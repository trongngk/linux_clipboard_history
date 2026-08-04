use crate::util::*;
use regex::Regex;
use rusqlite::{params, Connection, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;

/// Maximum number of entries kept (text and images share the budget). Pinned
/// entries do NOT count against it. Change with: cliphist set max-entries N
pub const DEFAULT_MAX_ENTRIES: usize = 20;
pub const MAX_TEXT_LEN: usize = 2_000_000;
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: i64,
    pub content: String,
    pub kind: String,
    pub path: Option<String>,
    pub meta: Option<String>,
    pub ts: f64,
    pub pinned: bool,
    pub hits: i64,
}

impl Entry {
    pub fn is_image(&self) -> bool {
        self.kind == "image"
    }

    /// The mime is stored at the front of `meta`: "image/png|1920x1080 - 245 KB"
    pub fn mime(&self) -> String {
        self.meta
            .as_deref()
            .and_then(|m| m.split('|').next())
            .filter(|m| m.starts_with("image/"))
            .unwrap_or("image/png")
            .to_string()
    }

    pub fn meta_text(&self) -> String {
        self.meta
            .as_deref()
            .map(|m| m.rsplit('|').next().unwrap_or(m).to_string())
            .unwrap_or_default()
    }

    pub fn label(&self, width: usize) -> String {
        if self.is_image() {
            format!("[image] {}", self.meta_text())
        } else {
            preview(&self.content, width)
        }
    }
}

pub fn open() -> Result<Connection> {
    let _ = ensure_dirs();
    let fresh = !db_path().exists();
    let con = Connection::open(db_path())?;
    con.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS entries (
             id      INTEGER PRIMARY KEY,
             hash    TEXT UNIQUE,
             content TEXT NOT NULL,
             ts      REAL NOT NULL,
             pinned  INTEGER NOT NULL DEFAULT 0,
             hits    INTEGER NOT NULL DEFAULT 1);
         CREATE INDEX IF NOT EXISTS idx_ts ON entries(ts DESC);",
    )?;

    // Migration: databases from older builds can be missing some columns.
    let mut have = HashSet::new();
    {
        let mut st = con.prepare("PRAGMA table_info(entries)")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            have.insert(r.get::<_, String>(1)?);
        }
    }
    for (name, ddl) in [
        ("kind", "kind TEXT NOT NULL DEFAULT 'text'"),
        ("path", "path TEXT"),
        ("meta", "meta TEXT"),
        // Very old (Python-era) databases may also lack these. Without `hash`
        // every INSERT below fails with "no such column" and the daemon can no
        // longer record anything at all.
        ("hash", "hash TEXT"),
        ("hits", "hits INTEGER NOT NULL DEFAULT 1"),
        ("pinned", "pinned INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !have.contains(name) {
            con.execute(&format!("ALTER TABLE entries ADD COLUMN {}", ddl), [])?;
        }
    }
    // A `hash` added by ALTER above carries no UNIQUE constraint. Restore the
    // dedupe index (harmless when the column was already UNIQUE on a fresh DB).
    // Best-effort: pre-existing duplicate hashes must not stop the daemon.
    let _ = con.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_hash ON entries(hash);");
    if fresh {
        chmod(&db_path(), 0o600);
    }
    Ok(con)
}

fn row_to_entry(r: &rusqlite::Row) -> Result<Entry> {
    Ok(Entry {
        id: r.get("id")?,
        content: r.get("content")?,
        kind: r.get("kind")?,
        path: r.get("path")?,
        meta: r.get("meta")?,
        ts: r.get("ts")?,
        pinned: r.get::<_, i64>("pinned")? != 0,
        hits: r.get("hits")?,
    })
}

pub fn fetch(con: &Connection, limit: i64) -> Result<Vec<Entry>> {
    let mut st = con
        .prepare("SELECT * FROM entries ORDER BY pinned DESC, ts DESC LIMIT ?1")?;
    let rows = st.query_map(params![limit], |r| row_to_entry(r))?;
    rows.collect()
}

pub fn get(con: &Connection, id: i64) -> Result<Option<Entry>> {
    let mut st = con.prepare("SELECT * FROM entries WHERE id=?1")?;
    let mut rows = st.query_map(params![id], |r| row_to_entry(r))?;
    match rows.next() {
        Some(e) => Ok(Some(e?)),
        None => Ok(None),
    }
}

fn upsert(
    con: &Connection,
    hash: &str,
    content: &str,
    kind: &str,
    path: Option<&str>,
    meta: Option<&str>,
) -> Result<()> {
    let n = con.execute(
        "UPDATE entries SET ts=?1, hits=hits+1 WHERE hash=?2",
        params![now(), hash],
    )?;
    if n == 0 {
        con.execute(
            "INSERT INTO entries(hash, content, ts, kind, path, meta) VALUES (?1,?2,?3,?4,?5,?6)",
            params![hash, content, now(), kind, path, meta],
        )?;
    }
    trim(con)
}

pub fn max_entries() -> usize {
    setting_usize("max_entries", DEFAULT_MAX_ENTRIES)
}

/// Keep the newest `max_entries` unpinned rows and drop the rest.
pub fn trim(con: &Connection) -> Result<()> {
    con.execute(
        "DELETE FROM entries WHERE pinned=0 AND id NOT IN (
             SELECT id FROM entries WHERE pinned=0 ORDER BY ts DESC LIMIT ?1)",
        params![max_entries() as i64],
    )?;
    prune_orphans(con)
}

/// Delete image files no database row points at any more.
pub fn prune_orphans(con: &Connection) -> Result<()> {
    let mut keep = HashSet::new();
    {
        let mut st =
            con.prepare("SELECT path FROM entries WHERE kind='image' AND path IS NOT NULL")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            keep.insert(PathBuf::from(r.get::<_, String>(0)?));
        }
    }
    if let Ok(dir) = std::fs::read_dir(image_dir()) {
        for f in dir.flatten() {
            let p = f.path();
            if p.is_file() && !keep.contains(&p) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
    Ok(())
}

pub fn store_text(con: &Connection, text: &str, rules: &[Regex]) -> Result<bool> {
    if text.trim().is_empty() || text.len() > MAX_TEXT_LEN {
        return Ok(false);
    }
    if rules.iter().any(|r| r.is_match(text)) {
        return Ok(false);
    }
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    upsert(con, &hash, text, "text", None, None)?;
    Ok(true)
}

pub fn store_image(con: &Connection, data: &[u8], mime: &str) -> Result<bool> {
    if data.is_empty() || data.len() > MAX_IMAGE_BYTES {
        return Ok(false);
    }
    let hash = format!("{:x}", Sha256::digest(data));
    let path = image_dir().join(format!("{}.{}", &hash[..20], ext_for(mime)));
    if !path.exists() {
        std::fs::write(&path, data).ok();
        chmod(&path, 0o600);
    }
    let meta = match image_size(data) {
        Some((w, h)) => format!("{}|{}x{} - {}", mime, w, h, human_bytes(data.len())),
        None => format!("{}|{} - {}", mime, mime, human_bytes(data.len())),
    };
    let p = path.to_string_lossy().to_string();
    upsert(con, &hash, &p, "image", Some(&p), Some(&meta))?;
    Ok(true)
}

pub fn delete(con: &Connection, id: i64) -> Result<()> {
    con.execute("DELETE FROM entries WHERE id=?1", params![id])?;
    prune_orphans(con)
}

pub fn set_pinned(con: &Connection, id: i64, pinned: bool) -> Result<()> {
    con.execute(
        "UPDATE entries SET pinned=?1 WHERE id=?2",
        params![if pinned { 1 } else { 0 }, id],
    )?;
    Ok(())
}

pub fn wipe(con: &Connection, all: bool) -> Result<()> {
    if all {
        con.execute("DELETE FROM entries", [])?;
    } else {
        con.execute("DELETE FROM entries WHERE pinned=0", [])?;
    }
    prune_orphans(con)
}

pub fn load_ignore_rules() -> Vec<Regex> {
    let mut out = Vec::new();
    if let Ok(text) = std::fs::read_to_string(ignore_file()) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match Regex::new(line) {
                Ok(r) => out.push(r),
                Err(e) => eprintln!("[{}] bad regex in ignore.txt: {:?} ({})", APP, line, e),
            }
        }
    }
    out
}
