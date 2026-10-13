//! Syntax-only Spring mapping extraction. No Java type attribution, inheritance or runtime route discovery.
use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};

pub(crate) struct Route {
    pub method: String,
    pub path: String,
    pub handler: String,
    pub line: usize,
}

pub(crate) struct Issue {
    pub code: &'static str,
    pub line: Option<usize>,
}

pub(crate) struct Parsed {
    pub issues: Vec<Issue>,
    pub routes: Vec<Route>,
    pub errors: usize,
    pub skipped: usize,
}

fn text<'a>(node: Node<'_>, source: &'a [u8]) -> Option<&'a str> {
    node.utf8_text(source).ok()
}

fn annotations<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut inner = child.walk();
            out.extend(
                child
                    .named_children(&mut inner)
                    .filter(|n| matches!(n.kind(), "annotation" | "marker_annotation")),
            );
        } else if matches!(child.kind(), "annotation" | "marker_annotation") {
            out.push(child);
        }
    }
    out
}

fn annotation_name<'a>(node: Node<'_>, source: &'a [u8]) -> Option<&'a str> {
    text(node.child_by_field_name("name")?, source).map(|n| n.rsplit('.').next().unwrap_or(n))
}

fn literal(node: Node<'_>, source: &[u8]) -> Option<String> {
    let s = text(node, source)?;
    if node.kind() != "string_literal"
        || !s.starts_with('"')
        || !s.ends_with('"')
        || s.contains('\\')
    {
        return None;
    }
    Some(s[1..s.len() - 1].to_owned())
}

fn paths(node: Node<'_>, source: &[u8]) -> Option<Vec<String>> {
    if let Some(value) = literal(node, source) {
        return Some(vec![value]);
    }
    if node.kind() != "element_value_array_initializer" {
        return None;
    }
    let mut cursor = node.walk();
    let values: Vec<_> = node
        .named_children(&mut cursor)
        .map(|n| literal(n, source))
        .collect();
    values.into_iter().collect()
}

// None means a non-literal/dynamic value; do not claim an endpoint in that case.
fn mapping_paths(annotation: Node<'_>, source: &[u8]) -> Option<Vec<String>> {
    let Some(args) = annotation.child_by_field_name("arguments") else {
        return Some(vec![String::new()]);
    };
    let mut cursor = args.walk();
    let nodes: Vec<_> = args.named_children(&mut cursor).collect();
    let path_node = nodes.iter().find_map(|n| {
        if n.kind() == "element_value_pair" {
            let key = n
                .child_by_field_name("key")
                .and_then(|key| text(key, source));
            if matches!(key, Some("path" | "value")) {
                return n.child_by_field_name("value");
            }
            None
        } else if matches!(
            n.kind(),
            "string_literal" | "element_value_array_initializer"
        ) {
            Some(*n)
        } else {
            None
        }
    });
    match path_node {
        Some(n) => paths(n, source),
        None => {
            if nodes.iter().any(|n| n.kind() != "element_value_pair") {
                None
            } else {
                Some(vec![String::new()])
            }
        }
    }
}

fn methods(annotation: Node<'_>, name: &str, source: &[u8]) -> Option<Vec<String>> {
    let method = match name {
        "GetMapping" => "GET",
        "PostMapping" => "POST",
        "PutMapping" => "PUT",
        "DeleteMapping" => "DELETE",
        "PatchMapping" => "PATCH",
        "RequestMapping" => "ANY",
        _ => return None,
    };
    if name != "RequestMapping" {
        return Some(vec![method.to_owned()]);
    }
    let Some(args) = annotation.child_by_field_name("arguments") else {
        return Some(vec![method.to_owned()]);
    };
    let mut cursor = args.walk();
    for pair in args
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "element_value_pair")
    {
        if pair
            .child_by_field_name("key")
            .and_then(|n| text(n, source))
            != Some("method")
        {
            continue;
        }
        let value = text(pair.child_by_field_name("value")?, source)?;
        let value = value.strip_prefix("RequestMethod.")?;
        return ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]
            .contains(&value)
            .then(|| vec![value.to_owned()]);
    }
    Some(vec![method.to_owned()])
}

fn join(prefix: &str, suffix: &str) -> String {
    let full = format!(
        "{}/{}",
        prefix.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    );
    if full == "/" {
        full
    } else {
        full.trim_end_matches('/').to_owned()
    }
}

pub(crate) fn parse(source: &str) -> Result<Parsed> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .context("loading Java grammar")?;
    let tree = parser
        .parse(source, None)
        .context("Java parser returned no syntax tree")?;
    let mut result = Parsed {
        issues: vec![],
        routes: vec![],
        errors: 0,
        skipped: 0,
    };
    if tree.root_node().has_error() {
        result.errors += 1;
        result.issues.push(Issue {
            code: "syntax_error",
            line: None,
        });
        return Ok(result);
    }
    let bytes = source.as_bytes();
    let mut cursor = tree.root_node().walk();
    for class in tree
        .root_node()
        .named_children(&mut cursor)
        .filter(|n| n.kind() == "class_declaration")
    {
        if !annotations(class)
            .into_iter()
            .any(|n| annotation_name(n, bytes) == Some("RestController"))
        {
            continue;
        }
        let Some(class_name) = class
            .child_by_field_name("name")
            .and_then(|n| text(n, bytes))
        else {
            continue;
        };
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        let prefix = annotations(class)
            .into_iter()
            .find(|n| annotation_name(*n, bytes) == Some("RequestMapping"));
        let prefixes =
            prefix.map_or_else(|| Some(vec![String::new()]), |n| mapping_paths(n, bytes));
        let class_verbs = prefix.map_or_else(
            || Some(vec!["ANY".to_owned()]),
            |n| methods(n, "RequestMapping", bytes),
        );
        if (prefixes.is_none() || class_verbs.is_none()) && prefix.is_some() {
            result.skipped += 1;
            result.issues.push(Issue {
                code: "nonliteral_mapping",
                line: prefix.map(|n| n.start_position().row + 1),
            });
        }
        let mut body_cursor = body.walk();
        for method in body
            .named_children(&mut body_cursor)
            .filter(|n| n.kind() == "method_declaration")
        {
            let Some(handler) = method
                .child_by_field_name("name")
                .and_then(|n| text(n, bytes))
            else {
                continue;
            };
            for annotation in annotations(method) {
                let Some(name) = annotation_name(annotation, bytes) else {
                    continue;
                };
                if !matches!(
                    name,
                    "GetMapping"
                        | "PostMapping"
                        | "PutMapping"
                        | "DeleteMapping"
                        | "PatchMapping"
                        | "RequestMapping"
                ) {
                    continue;
                }
                let Some(class_verbs) = class_verbs.as_ref() else {
                    continue;
                };
                if prefixes.is_none() {
                    continue;
                }
                let (Some(prefixes), Some(suffixes), Some(verbs)) = (
                    prefixes.as_ref(),
                    mapping_paths(annotation, bytes),
                    methods(annotation, name, bytes),
                ) else {
                    result.skipped += 1;
                    result.issues.push(Issue {
                        code: "nonliteral_mapping",
                        line: Some(annotation.start_position().row + 1),
                    });
                    continue;
                };
                for prefix in prefixes {
                    for suffix in &suffixes {
                        for verb in &verbs {
                            for class_verb in class_verbs {
                                if class_verb != "ANY" && verb != "ANY" && class_verb != verb {
                                    continue;
                                }
                                result.routes.push(Route {
                                    method: if verb == "ANY" {
                                        class_verb.clone()
                                    } else {
                                        verb.clone()
                                    },
                                    path: join(prefix, suffix),
                                    handler: format!("{class_name}.{handler}"),
                                    line: annotation.start_position().row + 1,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_literal_routes_without_matching_comments_or_dynamic_paths() -> Result<()> {
        let src = r#"
            @RestController
            @RequestMapping("/widgets")
            class WidgetController {
                // @PostMapping("/fake")
                @PostMapping(value = "/new")
                void create() {}
                @GetMapping(path = {"/v1/{id}", "/v2/{id}"})
                void detail() {}
                @PostMapping(PATH_CONST)
                void unknown() {}
            }
        "#;
        let parsed = parse(src)?;
        assert_eq!(parsed.routes.len(), 3);
        assert_eq!(parsed.routes[0].path, "/widgets/new");
        assert_eq!(parsed.routes[1].method, "GET");
        assert_eq!(parsed.skipped, 1);
        assert_eq!(
            (parsed.issues[0].code, parsed.issues[0].line),
            ("nonliteral_mapping", Some(10))
        );
        Ok(())
    }

    #[test]
    fn class_method_constraint_is_intersected_and_dynamic_prefix_is_reported() -> Result<()> {
        let parsed = parse(
            r#"@RestController
@RequestMapping(path="/tasks", method=RequestMethod.GET)
class TaskController {
  @RequestMapping("/open") void open() {}
  @GetMapping("/read") void read() {}
  @PostMapping("/write") void write() {}
}"#,
        )?;
        assert_eq!(parsed.routes.len(), 2);
        assert_eq!(
            (
                parsed.routes[0].method.as_str(),
                parsed.routes[0].path.as_str()
            ),
            ("GET", "/tasks/open")
        );
        assert_eq!(parsed.routes[1].method, "GET");
        assert!(parsed.issues.is_empty());

        let unknown = parse(
            r#"@RestController
@RequestMapping(PATH)
class TaskController { @GetMapping("/read") void read() {} }"#,
        )?;
        assert!(unknown.routes.is_empty());
        assert_eq!(
            (unknown.issues[0].code, unknown.issues[0].line),
            ("nonliteral_mapping", Some(2))
        );
        Ok(())
    }
}
