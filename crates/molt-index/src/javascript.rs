//! Syntax-level JS/TS index. Top-level declarations and static imports only; no type resolution.
use anyhow::{Context, Result};
use graphql_parser::query::parse_query;
use graphql_parser::schema::{parse_schema, Definition, Type, TypeDefinition, TypeExtension};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, CallExpression, Declaration, Expression, Statement, TaggedTemplateExpression,
};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};
use std::path::Path;

pub(crate) struct Symbol {
    pub name: String,
    pub kind: &'static str,
    pub start_line: usize,
    pub end_line: usize,
}

pub(crate) struct Import {
    pub specifier: String,
    pub line: usize,
    pub kind: &'static str,
}

pub(crate) struct Contract {
    pub category: &'static str,
    pub method: String,
    pub route: String,
    pub handler: String,
    pub signature: String,
    pub line: usize,
}

pub(crate) struct Issue {
    pub category: &'static str,
    pub code: &'static str,
    pub line: Option<usize>,
}

pub(crate) struct Parsed {
    pub issues: Vec<Issue>,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub contracts: Vec<Contract>,
    pub contract_errors: usize,
    pub contract_skipped: usize,
    pub error_count: usize,
}

fn line(starts: &[usize], offset: u32) -> usize {
    starts.partition_point(|&start| start <= offset as usize)
}

fn symbol(name: &str, kind: &'static str, span: Span, starts: &[usize], out: &mut Vec<Symbol>) {
    out.push(Symbol {
        name: name.to_owned(),
        kind,
        start_line: line(starts, span.start),
        end_line: line(starts, span.end.saturating_sub(1)),
    });
}

fn declaration(decl: &Declaration<'_>, starts: &[usize], out: &mut Vec<Symbol>) {
    match decl {
        Declaration::FunctionDeclaration(f) => {
            if let Some(id) = &f.id {
                symbol(id.name.as_str(), "function", f.span, starts, out);
            }
        }
        Declaration::ClassDeclaration(c) => {
            if let Some(id) = &c.id {
                symbol(id.name.as_str(), "class", c.span, starts, out);
            }
        }
        Declaration::VariableDeclaration(v) => {
            for d in &v.declarations {
                if let Some(id) = d.id.get_binding_identifier() {
                    symbol(id.name.as_str(), "variable", d.span, starts, out);
                }
            }
        }
        Declaration::TSInterfaceDeclaration(i) => {
            symbol(i.id.name.as_str(), "interface", i.span, starts, out)
        }
        Declaration::TSTypeAliasDeclaration(t) => {
            symbol(t.id.name.as_str(), "type", t.span, starts, out)
        }
        Declaration::TSEnumDeclaration(e) => {
            symbol(e.id.name.as_str(), "enum", e.span, starts, out)
        }
        _ => {} // Other TS declarations and destructuring are not indexed yet.
    }
}

pub(crate) fn parse(path: &Path, text: &str) -> Result<Parsed> {
    let source_type = SourceType::from_path(path)
        .with_context(|| format!("unsupported JS/TS extension: {}", path.display()))?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, text, source_type).parse();
    let error_count = parsed
        .diagnostics
        .len()
        .max(usize::from(parsed.fatal_error));
    // Do not persist an incomplete AST as if it were trustworthy.
    if error_count > 0 || parsed.fatal_error {
        return Ok(Parsed {
            issues: vec![Issue {
                category: "javascript",
                code: "syntax_error",
                line: None,
            }],
            symbols: vec![],
            imports: vec![],
            contracts: vec![],
            contract_errors: 0,
            contract_skipped: 0,
            error_count,
        });
    }
    let mut starts = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    let mut result = Parsed {
        issues: vec![],
        symbols: vec![],
        imports: vec![],
        contracts: vec![],
        contract_errors: 0,
        contract_skipped: 0,
        error_count: 0,
    };
    // Extract syntax candidates from every parsed file; directory and binding names are not contracts.
    let mut visitor = ContractVisitor {
        starts: &starts,
        contracts: &mut result.contracts,
        errors: &mut result.contract_errors,
        skipped: &mut result.contract_skipped,
        issues: &mut result.issues,
    };
    visitor.visit_program(&parsed.program);
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(i) => result.imports.push(Import {
                specifier: i.source.value.to_string(),
                line: line(&starts, i.span.start),
                kind: "import",
            }),
            Statement::ExportFromDeclaration(e) => result.imports.push(Import {
                specifier: e.source.value.to_string(),
                line: line(&starts, e.span.start),
                kind: "reexport",
            }),
            Statement::ExportAllDeclaration(e) => result.imports.push(Import {
                specifier: e.source.value.to_string(),
                line: line(&starts, e.span.start),
                kind: "reexport",
            }),
            Statement::ExportDeclaration(e) => {
                declaration(&e.declaration, &starts, &mut result.symbols);
            }
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    symbol(
                        id.name.as_str(),
                        "function",
                        f.span,
                        &starts,
                        &mut result.symbols,
                    );
                }
            }
            Statement::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    symbol(
                        id.name.as_str(),
                        "class",
                        c.span,
                        &starts,
                        &mut result.symbols,
                    );
                }
            }
            Statement::VariableDeclaration(v) => {
                for d in &v.declarations {
                    if let Some(id) = d.id.get_binding_identifier() {
                        symbol(
                            id.name.as_str(),
                            "variable",
                            d.span,
                            &starts,
                            &mut result.symbols,
                        );
                    }
                }
            }
            Statement::TSInterfaceDeclaration(i) => symbol(
                i.id.name.as_str(),
                "interface",
                i.span,
                &starts,
                &mut result.symbols,
            ),
            Statement::TSTypeAliasDeclaration(t) => symbol(
                t.id.name.as_str(),
                "type",
                t.span,
                &starts,
                &mut result.symbols,
            ),
            Statement::TSEnumDeclaration(e) => symbol(
                e.id.name.as_str(),
                "enum",
                e.span,
                &starts,
                &mut result.symbols,
            ),
            _ => {}
        }
    }
    Ok(result)
}

struct ContractVisitor<'a> {
    starts: &'a [usize],
    contracts: &'a mut Vec<Contract>,
    errors: &'a mut usize,
    skipped: &'a mut usize,
    issues: &'a mut Vec<Issue>,
}

impl<'ast> Visit<'ast> for ContractVisitor<'_> {
    fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'ast>) {
        if matches!(&tagged.tag, Expression::Identifier(tag) if tag.name == "gql") {
            if !tagged.quasi.expressions.is_empty() || tagged.quasi.quasis.len() != 1 {
                *self.skipped += 1;
                self.issues.push(Issue {
                    category: "graphql",
                    code: "interpolated_template",
                    line: Some(line(self.starts, tagged.span.start)),
                });
            } else {
                let quasi = &tagged.quasi.quasis[0];
                extract_sdl(
                    quasi.value.raw.as_str(),
                    line(self.starts, quasi.span.start),
                    self.contracts,
                    self.errors,
                    self.issues,
                );
            }
        }
        walk::walk_tagged_template_expression(self, tagged);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'ast>) {
        if let Expression::StaticMemberExpression(member) = &call.callee {
            let verb = member.property.name.as_str();
            if matches!(verb, "get" | "post" | "put" | "delete" | "patch")
                && matches!(&member.object, Expression::ThisExpression(_))
            {
                if let Some(Argument::StringLiteral(path)) = call.arguments.first() {
                    let route = path.value.as_str();
                    if route.starts_with('/') && !route.contains(['?', '#']) {
                        self.contracts.push(Contract {
                            category: "outbound_http_candidate",
                            method: verb.to_ascii_uppercase(),
                            route: route.to_owned(),
                            handler: format!("this.{verb}"),
                            signature: String::new(),
                            line: line(self.starts, call.span.start),
                        });
                    }
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
}

fn type_name(ty: &Type<'_, String>) -> String {
    match ty {
        Type::NamedType(name) => name.to_string(),
        Type::ListType(inner) => format!("[{}]", type_name(inner)),
        Type::NonNullType(inner) => format!("{}!", type_name(inner)),
    }
}

/// Parse a standalone SDL document or a non-interpolated `gql` template.
/// Valid GraphQL client operations are not SDL contracts and are ignored.
pub(crate) fn extract_sdl(
    sdl: &str,
    origin: usize,
    contracts: &mut Vec<Contract>,
    errors: &mut usize,
    issues: &mut Vec<Issue>,
) {
    let Ok(doc) = parse_schema::<String>(sdl) else {
        if parse_query::<String>(sdl).is_err() {
            *errors += 1;
            issues.push(Issue {
                category: "graphql",
                code: "invalid_document",
                line: Some(origin),
            });
        }
        return;
    };
    let mut root_names = [
        "Query".to_owned(),
        "Mutation".to_owned(),
        "Subscription".to_owned(),
    ];
    for def in &doc.definitions {
        if let Definition::SchemaDefinition(schema) = def {
            if let Some(query) = &schema.query {
                root_names[0] = query.to_string();
            }
            if let Some(mutation) = &schema.mutation {
                root_names[1] = mutation.to_string();
            }
            if let Some(subscription) = &schema.subscription {
                root_names[2] = subscription.to_string();
            }
        }
    }
    for def in &doc.definitions {
        let (name, fields) = match def {
            Definition::TypeDefinition(TypeDefinition::Object(obj)) => (&obj.name, &obj.fields),
            Definition::TypeExtension(TypeExtension::Object(obj)) => (&obj.name, &obj.fields),
            _ => continue,
        };
        let Some(index) = root_names.iter().position(|root| root == name) else {
            continue;
        };
        let category = ["query", "mutation", "subscription"][index];
        for field in fields {
            let args = field
                .arguments
                .iter()
                .map(|a| format!("{}: {}", a.name, type_name(&a.value_type)))
                .collect::<Vec<_>>()
                .join(", ");
            contracts.push(Contract {
                category: "graphql",
                method: category.to_owned(),
                route: field.name.to_string(),
                handler: field.name.to_string(),
                signature: format!("({args}) -> {}", type_name(&field.field_type)),
                line: origin + field.position.line - 1,
            });
        }
    }
}
