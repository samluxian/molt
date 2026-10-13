//! Local, rebuildable source and syntax-contract index; no type or runtime resolution.
mod java;
mod javascript;
mod resolution;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub status: &'static str,
    pub source: String,
    pub scanned: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub index: String,
    pub parsed: i64,
    pub parse_errors: i64,
    pub resolved_imports: i64,
    pub unresolved_imports: i64,
    pub graphql_files: i64,
    pub graphql_fields: i64,
    pub http_routes: i64,
    pub outbound_http_candidates: i64,
    pub contract_issues: i64,
}

#[derive(Debug, Serialize)]
pub struct FileRecord {
    pub path: String,
    pub language: String,
    pub content_hash: String,
    pub parse_status: String,
    pub error_count: i64,
    pub contract_status: String,
    pub contract_errors: i64,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub source: String,
    pub files: i64,
    pub javascript: i64,
    pub typescript: i64,
    pub java: i64,
    pub graphql: i64,
    pub parsed: i64,
    pub parse_errors: i64,
    pub resolved_imports: i64,
    pub unresolved_imports: i64,
    pub graphql_fields: i64,
    pub http_routes: i64,
    pub outbound_http_candidates: i64,
    pub contract_issues: i64,
}

#[derive(Debug, Serialize)]
pub struct SymbolRecord {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub start_line: i64,
    pub end_line: i64,
}

#[derive(Debug, Serialize)]
pub struct ImportRecord {
    pub path: String,
    pub specifier: String,
    pub kind: String,
    pub line: i64,
    pub resolution: String,
    pub target_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DependentRecord {
    pub path: String,
    pub specifier: String,
    pub kind: String,
    pub line: i64,
}

#[derive(Debug, Serialize)]
pub struct ContractRecord {
    pub path: String,
    pub category: String,
    pub method: String,
    pub route: String,
    pub handler: String,
    pub signature: String,
    pub line: i64,
}

#[derive(Debug, Serialize)]
pub struct ContractIssue {
    pub path: String,
    pub category: String,
    pub code: String,
    /// Start line of the construct when known; not necessarily the exact error position.
    pub line: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct HttpLink {
    pub caller_path: String,
    pub caller_line: i64,
    pub method: String,
    pub route: String,
    pub backend_path: String,
    pub backend_line: i64,
    pub status: &'static str,
}

fn language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "tsx" | "mts" | "cts" => Some("typescript"),
        "java" => Some("java"),
        "graphql" | "gql" => Some("graphql"),
        _ => None,
    }
}

fn skip_dir(entry: &walkdir::DirEntry) -> bool {
    !entry.file_type().is_dir()
        || !matches!(
            entry.file_name().to_str(),
            Some(
                ".git"
                    | ".molt"
                    | ".deengineer"
                    | "node_modules"
                    | "target"
                    | "dist"
                    | "build"
                    | ".next"
            )
        )
}

fn root(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("source does not exist: {}", path.display()))?;
    if !path.is_dir() {
        bail!("source must be a directory: {}", path.display());
    }
    Ok(path)
}

fn db_path(root: &Path) -> PathBuf {
    root.join(".molt/intelligence.db")
}

fn open(root: &Path, create: bool) -> Result<Connection> {
    let path = db_path(root);
    if root
        .join(".molt")
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("refusing to use a symlinked .molt directory");
    }
    if path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        bail!("refusing to use a symlinked intelligence.db");
    }
    if create {
        std::fs::create_dir_all(root.join(".molt"))?;
    } else if !path.is_file() {
        bail!(
            "index missing; run `molt analyze --source {}` first",
            root.display()
        );
    }
    let mut conn =
        Connection::open(&path).with_context(|| format!("opening {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS source_files (
           path TEXT PRIMARY KEY, language TEXT NOT NULL,
           content_hash TEXT NOT NULL, last_seen INTEGER NOT NULL,
           parse_status TEXT NOT NULL DEFAULT 'unparsed', error_count INTEGER NOT NULL DEFAULT 0
         );
         CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS idx_source_files_language ON source_files(language);",
    )?;
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 0 {
        let has_status: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('source_files') WHERE name = 'parse_status')",
            [], |row| row.get(0),
        )?;
        if !has_status {
            conn.execute_batch("ALTER TABLE source_files ADD COLUMN parse_status TEXT NOT NULL DEFAULT 'unparsed';
                                ALTER TABLE source_files ADD COLUMN error_count INTEGER NOT NULL DEFAULT 0;")?;
        }
        conn.pragma_update(None, "user_version", 1)?;
    } else if version != 1 && version != 2 && version != 3 && version != 4 {
        bail!("unsupported index schema version {version}; use a compatible MOLT version");
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS symbols (
           path TEXT NOT NULL REFERENCES source_files(path) ON DELETE CASCADE,
           name TEXT NOT NULL, kind TEXT NOT NULL, start_line INTEGER NOT NULL, end_line INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
         CREATE INDEX IF NOT EXISTS idx_symbols_path ON symbols(path);
         CREATE TABLE IF NOT EXISTS imports (
           path TEXT NOT NULL REFERENCES source_files(path) ON DELETE CASCADE,
           specifier TEXT NOT NULL, kind TEXT NOT NULL, line INTEGER NOT NULL,
           resolution TEXT NOT NULL DEFAULT 'pending', target_path TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_imports_path ON imports(path);"
    )?;
    if version < 2 {
        let has_resolution: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('imports') WHERE name='resolution')",
            [],
            |row| row.get(0),
        )?;
        if !has_resolution {
            conn.execute_batch(
                "ALTER TABLE imports ADD COLUMN resolution TEXT NOT NULL DEFAULT 'pending';
                                ALTER TABLE imports ADD COLUMN target_path TEXT;",
            )?;
        }
        conn.pragma_update(None, "user_version", 2)?;
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_imports_target ON imports(target_path);")?;
    if version < 3 {
        let tx = conn.transaction()?;
        tx.execute_batch("ALTER TABLE source_files ADD COLUMN contract_status TEXT NOT NULL DEFAULT 'not_scanned';
                          ALTER TABLE source_files ADD COLUMN contract_errors INTEGER NOT NULL DEFAULT 0;")?;
        tx.pragma_update(None, "user_version", 3)?;
        tx.commit()?;
    }
    conn.execute_batch("CREATE TABLE IF NOT EXISTS contracts (
        path TEXT NOT NULL REFERENCES source_files(path) ON DELETE CASCADE,
        category TEXT NOT NULL, method TEXT NOT NULL, route TEXT NOT NULL,
        handler TEXT NOT NULL, signature TEXT NOT NULL, line INTEGER NOT NULL
    ); CREATE INDEX IF NOT EXISTS idx_contracts_category_method_route ON contracts(category,method,route);
       CREATE INDEX IF NOT EXISTS idx_contracts_path ON contracts(path);")?;
    if version < 4 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "CREATE TABLE contract_issues (
            path TEXT NOT NULL REFERENCES source_files(path) ON DELETE CASCADE,
            category TEXT NOT NULL, code TEXT NOT NULL, line INTEGER
        ); CREATE INDEX idx_contract_issues_path ON contract_issues(path);
        UPDATE source_files SET contract_status='not_scanned';",
        )?;
        tx.pragma_update(None, "user_version", 4)?;
        tx.commit()?;
    }
    Ok(conn)
}

/// Scan supported source files. The repository remains the source of truth; only hashes and paths are stored.
pub fn analyze(source: &Path) -> Result<ScanResult> {
    let root = root(source)?;
    // Finish all reads before modifying the index, so an I/O failure cannot remove existing rows.
    let mut files = Vec::new();
    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(skip_dir)
    {
        let entry = entry.context("walking source tree")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(lang) = language(entry.path()) else {
            continue;
        };
        let path = entry
            .path()
            .strip_prefix(&root)?
            .to_str()
            .context("source path is not UTF-8")?
            .replace('\\', "/");
        let file = std::fs::File::open(entry.path())
            .with_context(|| format!("opening {}", entry.path().display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let len = reader
                .read(&mut buffer)
                .with_context(|| format!("reading {}", entry.path().display()))?;
            if len == 0 {
                break;
            }
            hasher.update(&buffer[..len]);
        }
        let hash = format!("{:x}", hasher.finalize());
        files.push((path, lang, hash));
    }
    files.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut conn = open(&root, true)?;
    let tx = conn.transaction()?;
    let generation: i64 = tx.query_row(
        "SELECT COALESCE(MAX(last_seen), 0) + 1 FROM source_files",
        [],
        |row| row.get(0),
    )?;
    let parser_version = "oxc-0.154.0-syntax-v4-graphql-sdl-java-routes-http-candidates-v1";
    let previous_parser: Option<String> = tx
        .query_row(
            "SELECT value FROM metadata WHERE key='javascript_parser_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let refresh_parser = previous_parser.as_deref() != Some(parser_version);
    let mut changed = 0;
    for (path, lang, hash) in &files {
        let prior: Option<(String, String, String, String)> = tx
            .query_row(
                "SELECT language, content_hash, parse_status, contract_status FROM source_files WHERE path = ?1",
                [path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let needs_parse =
            prior
                .as_ref()
                .is_none_or(|(old_lang, old_hash, status, contract_status)| {
                    old_lang != lang
                        || old_hash != hash
                        || status == "unparsed"
                        || contract_status == "not_scanned"
                        || refresh_parser
                });
        if needs_parse {
            changed += 1;
            let bytes = std::fs::read(root.join(path))?;
            if format!("{:x}", Sha256::digest(&bytes)) != *hash {
                bail!("source changed during scan: {path}; retry analyze");
            }
            let mut symbols = vec![];
            let mut imports = vec![];
            let mut contracts = vec![];
            let mut issues: Vec<ContractIssue> = vec![];
            let (status, error_count, contract_status, contract_errors) =
                match String::from_utf8(bytes) {
                    Err(_) => {
                        issues.push(ContractIssue {
                            path: path.clone(),
                            category: "source".into(),
                            code: "invalid_utf8".into(),
                            line: None,
                        });
                        ("invalid_utf8", 1, "error", 1)
                    }
                    Ok(text) if *lang == "graphql" => {
                        let mut parsed = vec![];
                        let mut errors = 0;
                        let mut sdl_issues = vec![];
                        javascript::extract_sdl(
                            &text,
                            1,
                            &mut parsed,
                            &mut errors,
                            &mut sdl_issues,
                        );
                        issues.extend(sdl_issues.into_iter().map(|issue| ContractIssue {
                            path: path.clone(),
                            category: issue.category.into(),
                            code: issue.code.into(),
                            line: issue.line.map(|line| line as i64),
                        }));
                        contracts.extend(parsed.into_iter().map(|c| ContractRecord {
                            path: path.clone(),
                            category: c.category.into(),
                            method: c.method,
                            route: c.route,
                            handler: c.handler,
                            signature: c.signature,
                            line: c.line as i64,
                        }));
                        (
                            if errors == 0 {
                                "graphql_sdl"
                            } else {
                                "graphql_error"
                            },
                            errors as i64,
                            if errors == 0 { "complete" } else { "error" },
                            errors as i64,
                        )
                    }
                    Ok(text) if *lang == "java" => {
                        let parsed = java::parse(&text)?;
                        contracts.extend(parsed.routes.into_iter().map(|r| ContractRecord {
                            path: path.clone(),
                            category: "http".into(),
                            method: r.method,
                            route: r.path,
                            handler: r.handler,
                            signature: String::new(),
                            line: r.line as i64,
                        }));
                        let issue_count = (parsed.errors + parsed.skipped) as i64;
                        issues.extend(parsed.issues.into_iter().map(|issue| ContractIssue {
                            path: path.clone(),
                            category: "java".into(),
                            code: issue.code.into(),
                            line: issue.line.map(|line| line as i64),
                        }));
                        (
                            "unsupported",
                            0,
                            if parsed.errors > 0 {
                                "error"
                            } else if parsed.skipped > 0 {
                                "partial"
                            } else {
                                "complete"
                            },
                            issue_count,
                        )
                    }
                    Ok(text) => {
                        let parsed = javascript::parse(Path::new(path), &text)?;
                        let status = if parsed.error_count == 0 {
                            "parsed"
                        } else {
                            "parse_error"
                        };
                        let contract_status = if parsed.error_count + parsed.contract_errors > 0 {
                            "error"
                        } else if parsed.contract_skipped > 0 {
                            "partial"
                        } else {
                            "complete"
                        };
                        let issue_count =
                            (parsed.error_count + parsed.contract_errors + parsed.contract_skipped)
                                as i64;
                        issues.extend(parsed.issues.into_iter().map(|issue| ContractIssue {
                            path: path.clone(),
                            category: issue.category.into(),
                            code: issue.code.into(),
                            line: issue.line.map(|line| line as i64),
                        }));
                        contracts.extend(parsed.contracts.into_iter().map(|c| ContractRecord {
                            path: path.clone(),
                            category: c.category.into(),
                            method: c.method,
                            route: c.route,
                            handler: c.handler,
                            signature: c.signature,
                            line: c.line as i64,
                        }));
                        symbols = parsed.symbols;
                        imports = parsed.imports;
                        (
                            status,
                            parsed.error_count as i64,
                            contract_status,
                            issue_count,
                        )
                    }
                };
            tx.execute(
                "INSERT INTO source_files(path, language, content_hash, last_seen, parse_status, error_count, contract_status, contract_errors)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(path) DO UPDATE SET language=excluded.language,
                   content_hash=excluded.content_hash, last_seen=excluded.last_seen,
                   parse_status=excluded.parse_status, error_count=excluded.error_count,
                   contract_status=excluded.contract_status, contract_errors=excluded.contract_errors",
                params![path, lang, hash, generation, status, error_count, contract_status, contract_errors],
            )?;
            tx.execute("DELETE FROM symbols WHERE path = ?1", [path])?;
            tx.execute("DELETE FROM imports WHERE path = ?1", [path])?;
            tx.execute("DELETE FROM contracts WHERE path = ?1", [path])?;
            tx.execute("DELETE FROM contract_issues WHERE path = ?1", [path])?;
            for s in symbols {
                tx.execute("INSERT INTO symbols(path,name,kind,start_line,end_line) VALUES (?1,?2,?3,?4,?5)",
                    params![path, s.name, s.kind, s.start_line, s.end_line])?;
            }
            for i in imports {
                tx.execute(
                    "INSERT INTO imports(path,specifier,kind,line) VALUES (?1,?2,?3,?4)",
                    params![path, i.specifier, i.kind, i.line],
                )?;
            }
            for c in contracts {
                tx.execute("INSERT INTO contracts(path,category,method,route,handler,signature,line) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![c.path, c.category, c.method, c.route, c.handler, c.signature, c.line])?;
            }
            for issue in issues {
                tx.execute(
                    "INSERT INTO contract_issues(path,category,code,line) VALUES (?1,?2,?3,?4)",
                    params![issue.path, issue.category, issue.code, issue.line],
                )?;
            }
        } else {
            tx.execute(
                "UPDATE source_files SET last_seen = ?2 WHERE path = ?1",
                params![path, generation],
            )?;
        }
    }
    let removed = tx.execute(
        "DELETE FROM source_files WHERE last_seen != ?1",
        [generation],
    )?;
    tx.execute(
        "INSERT INTO metadata(key,value) VALUES ('source',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [root.to_string_lossy().as_ref()],
    )?;
    tx.execute(
        "INSERT INTO metadata(key,value) VALUES ('javascript_parser_version',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [parser_version],
    )?;
    // Resolve against the complete *current* inventory, even when importers are unchanged.
    // New/deleted targets can change edges without changing the importing file's hash.
    let paths: HashSet<String> = {
        let mut stmt = tx.prepare("SELECT path FROM source_files WHERE parse_status='parsed' AND language IN ('javascript','typescript')")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    {
        let mut stmt = tx.prepare("SELECT rowid,path,specifier FROM imports")?;
        let imports: Vec<(i64, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, importer, specifier) in imports {
            let result = resolution::resolve(&importer, &specifier, &paths);
            tx.execute(
                "UPDATE imports SET resolution=?2,target_path=?3 WHERE rowid=?1",
                params![id, result.status, result.target],
            )?;
        }
    }
    let (parsed, parse_errors, resolved_imports, unresolved_imports): (i64, i64, i64, i64) = tx.query_row(
        "SELECT
            (SELECT COUNT(*) FROM source_files WHERE parse_status='parsed'),
            (SELECT COUNT(*) FROM source_files WHERE parse_status IN ('parse_error','invalid_utf8')),
            (SELECT COUNT(*) FROM imports WHERE resolution='resolved'),
            (SELECT COUNT(*) FROM imports WHERE resolution NOT IN ('resolved','non_relative'))",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let (graphql_fields, http_routes, outbound_http_calls, contract_issues) = contract_counts(&tx)?;
    tx.commit()?;
    Ok(ScanResult {
        status: "completed",
        source: root.display().to_string(),
        scanned: files.len(),
        changed,
        unchanged: files.len() - changed,
        removed,
        index: db_path(&root).display().to_string(),
        parsed,
        parse_errors,
        resolved_imports,
        unresolved_imports,
        graphql_files: files
            .iter()
            .filter(|(_, lang, _)| *lang == "graphql")
            .count() as i64,
        graphql_fields,
        http_routes,
        outbound_http_candidates: outbound_http_calls,
        contract_issues,
    })
}

fn contract_counts(conn: &Connection) -> Result<(i64, i64, i64, i64)> {
    Ok(conn.query_row(
        "SELECT (SELECT COUNT(*) FROM contracts WHERE category='graphql'),
                (SELECT COUNT(*) FROM contracts WHERE category='http'),
                (SELECT COUNT(*) FROM contracts WHERE category='outbound_http_candidate'),
                (SELECT COUNT(*) FROM source_files WHERE contract_status IN ('error','partial'))",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?)
}

pub fn summary(source: &Path) -> Result<Summary> {
    let root = root(source)?;
    let conn = open(&root, false)?;
    let (
        files,
        javascript,
        typescript,
        java,
        graphql,
        parsed,
        parse_errors,
        resolved_imports,
        unresolved_imports,
    ) = conn.query_row(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE language='javascript'),
                COUNT(*) FILTER (WHERE language='typescript'),
                COUNT(*) FILTER (WHERE language='java'),
                COUNT(*) FILTER (WHERE language='graphql'),
                COUNT(*) FILTER (WHERE parse_status='parsed'),
                COUNT(*) FILTER (WHERE parse_status IN ('parse_error','invalid_utf8')),
                (SELECT COUNT(*) FROM imports WHERE resolution='resolved'),
                (SELECT COUNT(*) FROM imports WHERE resolution NOT IN ('resolved','non_relative'))
                FROM source_files",
        [],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        },
    )?;
    let (graphql_fields, http_routes, outbound_http_calls, contract_issues) =
        contract_counts(&conn)?;
    Ok(Summary {
        source: root.display().to_string(),
        files,
        javascript,
        typescript,
        java,
        graphql,
        parsed,
        parse_errors,
        resolved_imports,
        unresolved_imports,
        graphql_fields,
        http_routes,
        outbound_http_candidates: outbound_http_calls,
        contract_issues,
    })
}

/// Return at most 100 file records; offset allows bounded agent queries.
pub fn files(source: &Path, limit: u32, offset: u32) -> Result<Vec<FileRecord>> {
    validate_limit(limit)?;
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path, language, content_hash, parse_status, error_count, contract_status, contract_errors FROM source_files ORDER BY path LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(FileRecord {
            path: row.get(0)?,
            language: row.get(1)?,
            content_hash: row.get(2)?,
            parse_status: row.get(3)?,
            error_count: row.get(4)?,
            contract_status: row.get(5)?,
            contract_errors: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Query top-level JS/TS declarations by exact name, optionally restricting to a relative path.
pub fn symbols(
    source: &Path,
    name: &str,
    path: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<SymbolRecord>> {
    validate_limit(limit)?;
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path,name,kind,start_line,end_line FROM symbols
         WHERE name=?1 AND (?2 IS NULL OR path=?2) ORDER BY path,start_line LIMIT ?3 OFFSET ?4",
    )?;
    let rows = stmt.query_map(params![name, path, limit, offset], |r| {
        Ok(SymbolRecord {
            path: r.get(0)?,
            name: r.get(1)?,
            kind: r.get(2)?,
            start_line: r.get(3)?,
            end_line: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Query static imports and re-exports for one indexed file; only relative file-level edges are resolved.
pub fn imports(source: &Path, path: &str, limit: u32, offset: u32) -> Result<Vec<ImportRecord>> {
    validate_limit(limit)?;
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path,specifier,kind,line,resolution,target_path FROM imports WHERE path=?1 ORDER BY line,specifier LIMIT ?2 OFFSET ?3"
    )?;
    let rows = stmt.query_map(params![path, limit, offset], |r| {
        Ok(ImportRecord {
            path: r.get(0)?,
            specifier: r.get(1)?,
            kind: r.get(2)?,
            line: r.get(3)?,
            resolution: r.get(4)?,
            target_path: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// List files with a resolved relative import to this exact indexed relative path.
pub fn dependents(
    source: &Path,
    path: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<DependentRecord>> {
    validate_limit(limit)?;
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path,specifier,kind,line FROM imports WHERE target_path=?1 AND resolution='resolved'
         ORDER BY path,line,specifier LIMIT ?2 OFFSET ?3"
    )?;
    let rows = stmt.query_map(params![path, limit, offset], |r| {
        Ok(DependentRecord {
            path: r.get(0)?,
            specifier: r.get(1)?,
            kind: r.get(2)?,
            line: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Bounded syntax-level contract inventory; query is not a runtime compatibility check.
pub fn contracts(
    source: &Path,
    category: &str,
    path: Option<&str>,
    method: Option<&str>,
    route: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ContractRecord>> {
    validate_limit(limit)?;
    if !matches!(category, "graphql" | "http" | "outbound_http_candidate") {
        bail!("category must be graphql, http or outbound_http_candidate");
    }
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path,category,method,route,handler,signature,line FROM contracts
         WHERE category=?1 AND (?2 IS NULL OR path=?2) AND (?3 IS NULL OR method=?3)
         AND (?4 IS NULL OR route=?4) ORDER BY path,line,method,route LIMIT ?5 OFFSET ?6",
    )?;
    let rows = stmt.query_map(params![category, path, method, route, limit, offset], |r| {
        Ok(ContractRecord {
            path: r.get(0)?,
            category: r.get(1)?,
            method: r.get(2)?,
            route: r.get(3)?,
            handler: r.get(4)?,
            signature: r.get(5)?,
            line: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Bounded reasons for incomplete syntax coverage. Line is a construct start, if available.
pub fn contract_issues(
    source: &Path,
    path: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ContractIssue>> {
    validate_limit(limit)?;
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path,category,code,line FROM contract_issues
         WHERE (?1 IS NULL OR path=?1) ORDER BY path,line,category,code LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(params![path, limit, offset], |r| {
        Ok(ContractIssue {
            path: r.get(0)?,
            category: r.get(1)?,
            code: r.get(2)?,
            line: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Candidate matches between literal outbound calls and literal Spring mappings.
/// These do not establish reachability, authentication, base URL or runtime behavior.
pub fn http_links(
    source: &Path,
    route: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<HttpLink>> {
    validate_limit(limit)?;
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT o.path,o.line,o.method,o.route,i.path,i.line FROM contracts o JOIN contracts i
         ON i.category='http' AND o.method=i.method AND o.route=i.route
         WHERE o.category='outbound_http_candidate' AND (?1 IS NULL OR o.route=?1)
         ORDER BY o.path,o.line,i.path,i.line LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(params![route, limit, offset], |r| {
        Ok(HttpLink {
            caller_path: r.get(0)?,
            caller_line: r.get(1)?,
            method: r.get(2)?,
            route: r.get(3)?,
            backend_path: r.get(4)?,
            backend_line: r.get(5)?,
            status: "path_match_unverified",
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn validate_limit(limit: u32) -> Result<()> {
    if !(1..=100).contains(&limit) {
        bail!("limit must be between 1 and 100");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_scan_tracks_changes_deletions_and_skips_generated_dirs() -> Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(dir.path().join("app.tsx"), "export default 1")?;
        std::fs::create_dir(dir.path().join("node_modules"))?;
        std::fs::write(dir.path().join("node_modules/ignored.js"), "ignore")?;
        let first = analyze(dir.path())?;
        assert_eq!((first.scanned, first.changed), (1, 1));
        let second = analyze(dir.path())?;
        assert_eq!((second.changed, second.unchanged), (0, 1));
        std::fs::write(dir.path().join("app.tsx"), "export default 2")?;
        assert_eq!(analyze(dir.path())?.changed, 1);
        std::fs::remove_file(dir.path().join("app.tsx"))?;
        assert_eq!(analyze(dir.path())?.removed, 1);
        assert_eq!(summary(dir.path())?.files, 0);
        Ok(())
    }

    #[test]
    fn ast_index_is_incremental_and_does_not_keep_stale_symbols() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("entry.tsx");
        std::fs::write(&file, "import X from './other';\nexport function Hello() { return <p/>; }\nexport interface Item {}\n")?;
        analyze(dir.path())?;
        assert_eq!(symbols(dir.path(), "Hello", None, 10, 0)?[0].start_line, 2);
        assert_eq!(
            imports(dir.path(), "entry.tsx", 10, 0)?[0].specifier,
            "./other"
        );
        assert_eq!(analyze(dir.path())?.changed, 0);
        std::fs::write(&file, "export const Renamed = 1;\n")?;
        analyze(dir.path())?;
        assert!(symbols(dir.path(), "Hello", None, 10, 0)?.is_empty());
        assert!(imports(dir.path(), "entry.tsx", 10, 0)?.is_empty());
        assert_eq!(symbols(dir.path(), "Renamed", None, 10, 0)?.len(), 1);
        std::fs::write(&file, "export const = ;\n")?;
        assert_eq!(analyze(dir.path())?.parse_errors, 1);
        assert!(symbols(dir.path(), "Renamed", None, 10, 0)?.is_empty());
        assert!(symbols(dir.path(), "Renamed", None, 101, 0).is_err());
        std::fs::write(&file, "export class Restored {}")?;
        analyze(dir.path())?;
        std::fs::remove_file(&file)?;
        assert_eq!(analyze(dir.path())?.removed, 1);
        assert!(symbols(dir.path(), "Restored", None, 10, 0)?.is_empty());
        Ok(())
    }

    #[test]
    fn contracts_are_bounded_and_incremental_with_source_evidence() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let schema = dir.path().join("schema.ts");
        let controller = dir.path().join("Cards.java");
        let client_dir = dir.path().join("unrelated/modules");
        std::fs::create_dir_all(&client_dir)?;
        let client = client_dir.join("AnyClient.ts");
        std::fs::write(&client, "class AnyClient {\n  async create() { return this.post('/widgets/new'); }\n  async ignored(id: string) { return this.post(`/widgets/${id}`); }\n}\n")?;
        std::fs::write(&schema, "import { gql } from 'graphql-tag';\nfunction define() { return gql`\n  type Query { widget(id: ID!): String }\n  extend type Mutation { createWidget: Boolean! }\n`; }\n")?;
        std::fs::write(&controller, "@RestController\n@RequestMapping(\"/widgets\")\nclass WidgetController {\n  @PostMapping(\"/new\") void createWidget() {}\n}\n")?;
        let first = analyze(dir.path())?;
        assert_eq!(
            (
                first.graphql_fields,
                first.http_routes,
                first.outbound_http_candidates
            ),
            (2, 1, 1)
        );
        let links = http_links(dir.path(), Some("/widgets/new"), 10, 0)?;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].caller_line, 2);
        assert_eq!(links[0].backend_line, 4);
        assert_eq!(links[0].status, "path_match_unverified");
        assert!(http_links(dir.path(), None, 101, 0).is_err());
        assert_eq!(
            contracts(
                dir.path(),
                "outbound_http_candidate",
                None,
                Some("POST"),
                None,
                10,
                0
            )?
            .len(),
            1
        );
        let gql = contracts(
            dir.path(),
            "graphql",
            None,
            Some("query"),
            Some("widget"),
            10,
            0,
        )?;
        assert_eq!(
            (gql[0].line, gql[0].signature.as_str()),
            (3, "(id: ID!) -> String")
        );
        let http = contracts(
            dir.path(),
            "http",
            None,
            Some("POST"),
            Some("/widgets/new"),
            10,
            0,
        )?;
        assert_eq!(http[0].handler, "WidgetController.createWidget");
        assert!(contracts(dir.path(), "bad", None, None, None, 1, 0).is_err());
        assert!(contracts(dir.path(), "http", None, None, None, 101, 0).is_err());
        assert_eq!(analyze(dir.path())?.changed, 0);
        std::fs::write(&controller, "@RestController class WidgetController {}")?;
        assert_eq!(analyze(dir.path())?.http_routes, 0);
        assert!(http_links(dir.path(), None, 10, 0)?.is_empty());
        std::fs::remove_file(client)?;
        assert_eq!(analyze(dir.path())?.outbound_http_candidates, 0);
        std::fs::remove_file(schema)?;
        assert_eq!(analyze(dir.path())?.graphql_fields, 0);
        Ok(())
    }

    #[test]
    fn graphql_sdl_files_and_nonstandard_js_bindings_are_indexed_without_business_names(
    ) -> Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::create_dir(dir.path().join("arbitrary"))?;
        let sdl = dir.path().join("arbitrary/schema.graphql");
        let js = dir.path().join("arbitrary/operations.ts");
        std::fs::write(
            &sdl,
            "schema { query: Root }\ntype Root { items: [String!]! }\n",
        )?;
        std::fs::write(&js, "const mySchema = gql`type Mutation { addItem: ID }`;\nconst clientQuery = gql`query ReadItems { items }`;\n")?;
        let result = analyze(dir.path())?;
        assert_eq!(
            (
                result.graphql_files,
                result.graphql_fields,
                result.contract_issues
            ),
            (1, 2, 0)
        );
        let fields = contracts(dir.path(), "graphql", None, None, None, 10, 0)?;
        assert_eq!((fields[0].route.as_str(), fields[0].line), ("addItem", 1));
        assert_eq!((fields[1].route.as_str(), fields[1].line), ("items", 2));
        assert_eq!(summary(dir.path())?.graphql, 1);
        std::fs::write(&sdl, "type Root { broken( }")?;
        let updated = analyze(dir.path())?;
        assert_eq!((updated.graphql_fields, updated.contract_issues), (1, 1));
        assert_eq!(files(dir.path(), 10, 0)?[1].contract_status, "error");
        let issue = &contract_issues(dir.path(), Some("arbitrary/schema.graphql"), 10, 0)?[0];
        assert_eq!(
            (issue.category.as_str(), issue.code.as_str(), issue.line),
            ("graphql", "invalid_document", Some(1))
        );
        assert!(contract_issues(dir.path(), None, 101, 0).is_err());
        assert!(contract_issues(dir.path(), None, 1, 1)?.is_empty());
        std::fs::write(&sdl, "type Query { working: String }")?;
        analyze(dir.path())?;
        assert!(contract_issues(dir.path(), None, 10, 0)?.is_empty());
        std::fs::remove_file(js)?;
        std::fs::remove_file(sdl)?;
        assert_eq!(analyze(dir.path())?.graphql_fields, 0);
        Ok(())
    }

    #[test]
    fn issue_evidence_tracks_partial_syntax_and_file_deletion() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let controller = dir.path().join("Other.java");
        let client = dir.path().join("schema.ts");
        std::fs::write(&controller, "@RestController\n@RequestMapping(PATH)\nclass Other { @GetMapping(\"/items\") void list() {} }")?;
        std::fs::write(&client, "const schema = gql`type Query { ${fields} }`;")?;
        assert_eq!(analyze(dir.path())?.contract_issues, 2);
        let issues = contract_issues(dir.path(), None, 10, 0)?;
        assert_eq!(issues.len(), 2);
        assert_eq!(
            (
                issues[0].path.as_str(),
                issues[0].code.as_str(),
                issues[0].line
            ),
            ("Other.java", "nonliteral_mapping", Some(2))
        );
        assert_eq!(
            (
                issues[1].path.as_str(),
                issues[1].code.as_str(),
                issues[1].line
            ),
            ("schema.ts", "interpolated_template", Some(1))
        );
        assert_eq!(
            contract_issues(dir.path(), None, 1, 1)?[0].path,
            "schema.ts"
        );
        std::fs::write(&client, "const schema = gql`type Query { items: String }`;")?;
        std::fs::remove_file(controller)?;
        assert_eq!(analyze(dir.path())?.contract_issues, 0);
        assert!(contract_issues(dir.path(), None, 10, 0)?.is_empty());
        Ok(())
    }

    #[test]
    fn relative_edges_update_when_target_changes_without_reparsing_importer() -> Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::create_dir(dir.path().join("app"))?;
        std::fs::write(
            dir.path().join("app/main.ts"),
            "import './part';\nimport '../elsewhere';\nimport React from 'react';\n",
        )?;
        let first = analyze(dir.path())?;
        assert_eq!(first.unresolved_imports, 2);
        assert_eq!(
            imports(dir.path(), "app/main.ts", 10, 0)?[0].resolution,
            "missing"
        );
        std::fs::write(dir.path().join("app/part.ts"), "export const part = 1")?;
        let second = analyze(dir.path())?;
        assert_eq!(second.changed, 1);
        assert_eq!(second.resolved_imports, 1);
        assert_eq!(
            dependents(dir.path(), "app/part.ts", 10, 0)?[0].path,
            "app/main.ts"
        );
        std::fs::write(dir.path().join("app/part.js"), "export const part = 2")?;
        assert_eq!(analyze(dir.path())?.resolved_imports, 0);
        assert_eq!(
            imports(dir.path(), "app/main.ts", 10, 0)?[0].resolution,
            "ambiguous"
        );
        std::fs::remove_file(dir.path().join("app/part.js"))?;
        assert_eq!(analyze(dir.path())?.resolved_imports, 1);
        std::fs::remove_file(dir.path().join("app/part.ts"))?;
        assert_eq!(analyze(dir.path())?.resolved_imports, 0);
        assert!(dependents(dir.path(), "app/part.ts", 10, 0)?.is_empty());
        assert!(dependents(dir.path(), "app/part.ts", 101, 0).is_err());
        Ok(())
    }

    #[test]
    fn upgrades_previous_inventory_schema_and_marks_java_unsupported() -> Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::create_dir(dir.path().join(".molt"))?;
        let conn = Connection::open(db_path(dir.path()))?;
        conn.execute_batch("CREATE TABLE source_files (
            path TEXT PRIMARY KEY, language TEXT NOT NULL, content_hash TEXT NOT NULL, last_seen INTEGER NOT NULL
        ); INSERT INTO source_files VALUES ('old.ts','typescript','old',1);
        CREATE TABLE imports (path TEXT NOT NULL, specifier TEXT NOT NULL, kind TEXT NOT NULL, line INTEGER NOT NULL);")?;
        drop(conn);
        std::fs::write(dir.path().join("old.ts"), "export class Card {}")?;
        std::fs::write(dir.path().join("Main.java"), "class Main {}")?;
        let result = analyze(dir.path())?;
        assert_eq!((result.scanned, result.changed, result.parsed), (2, 2, 1));
        let records = files(dir.path(), 10, 0)?;
        assert_eq!(records[0].parse_status, "unsupported");
        assert_eq!(symbols(dir.path(), "Card", None, 10, 0)?.len(), 1);
        let version: i64 = Connection::open(db_path(dir.path()))?.pragma_query_value(
            None,
            "user_version",
            |row| row.get(0),
        )?;
        assert_eq!(version, 4);
        Ok(())
    }

    #[test]
    fn upgrades_v3_index_and_refreshes_coverage_evidence() -> Result<()> {
        let dir = tempfile::tempdir()?;
        std::fs::write(
            dir.path().join("schema.ts"),
            "const s = gql`type Query { ${fields} }`;",
        )?;
        assert_eq!(analyze(dir.path())?.contract_issues, 1);
        let conn = Connection::open(db_path(dir.path()))?;
        conn.execute_batch(
            "DROP TABLE contract_issues; PRAGMA user_version=3;
            UPDATE source_files SET contract_status='complete', contract_errors=0;",
        )?;
        drop(conn);
        let refreshed = analyze(dir.path())?;
        assert_eq!((refreshed.changed, refreshed.contract_issues), (1, 1));
        assert_eq!(
            contract_issues(dir.path(), None, 10, 0)?[0].code,
            "interpolated_template"
        );
        assert_eq!(analyze(dir.path())?.changed, 0);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_index_directory() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".molt"))?;
        assert!(analyze(dir.path()).is_err());
        assert!(!outside.path().join("intelligence.db").exists());
        Ok(())
    }

    #[test]
    fn query_is_bounded_and_requires_index() -> Result<()> {
        let dir = tempfile::tempdir()?;
        assert!(summary(dir.path()).is_err());
        for name in ["a.java", "b.js", "c.ts"] {
            std::fs::write(dir.path().join(name), name)?;
        }
        analyze(dir.path())?;
        assert_eq!(summary(dir.path())?.java, 1);
        assert_eq!(files(dir.path(), 1, 1)?[0].path, "b.js");
        assert!(files(dir.path(), 101, 0).is_err());
        Ok(())
    }
}
