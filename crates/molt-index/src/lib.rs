//! Local, rebuildable source-file inventory. No AST or semantic claims are made by this crate.
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
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
}

#[derive(Debug, Serialize)]
pub struct FileRecord {
    pub path: String,
    pub language: String,
    pub content_hash: String,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub source: String,
    pub files: i64,
    pub javascript: i64,
    pub typescript: i64,
    pub java: i64,
}

fn language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "tsx" | "mts" | "cts" => Some("typescript"),
        "java" => Some("java"),
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
    let conn = Connection::open(&path).with_context(|| format!("opening {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS source_files (
           path TEXT PRIMARY KEY, language TEXT NOT NULL,
           content_hash TEXT NOT NULL, last_seen INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS idx_source_files_language ON source_files(language);",
    )?;
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
    let mut changed = 0;
    for (path, lang, hash) in &files {
        let prior: Option<(String, String)> = tx
            .query_row(
                "SELECT language, content_hash FROM source_files WHERE path = ?1",
                [path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if prior.as_ref() != Some(&(lang.to_string(), hash.clone())) {
            changed += 1;
        }
        tx.execute(
            "INSERT INTO source_files(path, language, content_hash, last_seen) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET language=excluded.language,
               content_hash=excluded.content_hash, last_seen=excluded.last_seen",
            params![path, lang, hash, generation],
        )?;
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
    tx.commit()?;
    Ok(ScanResult {
        status: "completed",
        source: root.display().to_string(),
        scanned: files.len(),
        changed,
        unchanged: files.len() - changed,
        removed,
        index: db_path(&root).display().to_string(),
    })
}

pub fn summary(source: &Path) -> Result<Summary> {
    let root = root(source)?;
    let conn = open(&root, false)?;
    let (files, javascript, typescript, java) = conn.query_row(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE language='javascript'),
                COUNT(*) FILTER (WHERE language='typescript'),
                COUNT(*) FILTER (WHERE language='java') FROM source_files",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    Ok(Summary {
        source: root.display().to_string(),
        files,
        javascript,
        typescript,
        java,
    })
}

/// Return at most 100 file records; offset allows bounded agent queries.
pub fn files(source: &Path, limit: u32, offset: u32) -> Result<Vec<FileRecord>> {
    if !(1..=100).contains(&limit) {
        bail!("limit must be between 1 and 100");
    }
    let root = root(source)?;
    let conn = open(&root, false)?;
    let mut stmt = conn.prepare(
        "SELECT path, language, content_hash FROM source_files ORDER BY path LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        Ok(FileRecord {
            path: row.get(0)?,
            language: row.get(1)?,
            content_hash: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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
