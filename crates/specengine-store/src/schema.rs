//! The schema, the connection PRAGMAs and the format stamp
//! (`crates/specengine-store/README.md`, "Rows", "Cache key and format stamp",
//! "Connection, location, concurrency"; 05 §8).
//!
//! Every table is `STRICT`; kinds are free strings (no `CHECK`); no key on
//! `id` (a node is keyed by `(file, ord)`, uniqueness is `spec check`'s).
//! FTS5 is external-content over `nodes` with triggers, trigram-tokenised,
//! case-folded, no stemmer. The stamp is `INDEX_FORMAT` in `index_meta`;
//! `user_version` belongs to the proposal queue's own schema steps
//! (`crate::queue`), whose tables no list here names.

use rusqlite::{Connection, OptionalExtension, Transaction};

use crate::INDEX_FORMAT;
use crate::error::{Db, StoreError};

/// `index_meta` key of the format stamp.
pub(crate) const FORMAT_KEY: &str = "format";

/// Milliseconds a statement waits for another connection's lock.
pub(crate) const BUSY_TIMEOUT_MS: i64 = 5000;
/// Bytes a WAL file is truncated to after a checkpoint.
pub(crate) const JOURNAL_SIZE_LIMIT: i64 = 67_108_864;

/// The index tables of the current format.
const SCHEMA: &str = "
CREATE TABLE index_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE worktrees (
  wt INTEGER PRIMARY KEY,
  project TEXT NOT NULL,
  root TEXT NOT NULL,
  scheme_fp TEXT NOT NULL,
  UNIQUE (project, root)
) STRICT;
CREATE TABLE files (
  file_id INTEGER PRIMARY KEY,
  wt INTEGER NOT NULL REFERENCES worktrees ON DELETE CASCADE,
  path TEXT NOT NULL,
  blake3 TEXT,
  size INTEGER NOT NULL,
  read_error TEXT,
  shell TEXT,
  tier3 INTEGER NOT NULL,
  UNIQUE (wt, path)
) STRICT;
CREATE TABLE nodes (
  node_id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  line INTEGER NOT NULL,
  id TEXT,
  kind TEXT,
  title TEXT,
  parent_id TEXT,
  own_text TEXT NOT NULL,
  norm_hash TEXT,
  node TEXT NOT NULL,
  UNIQUE (file_id, ord)
) STRICT;
CREATE TABLE aliases (
  node_id INTEGER NOT NULL REFERENCES nodes ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  alias TEXT NOT NULL,
  PRIMARY KEY (node_id, ord)
) STRICT;
CREATE TABLE links (
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  src TEXT,
  type TEXT NOT NULL,
  dst_id TEXT,
  dst_path TEXT,
  link TEXT NOT NULL,
  PRIMARY KEY (file_id, ord)
) STRICT;
CREATE TABLE anchors (
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  name TEXT NOT NULL,
  anchor TEXT NOT NULL,
  PRIMARY KEY (file_id, ord)
) STRICT;
CREATE TABLE diagnostics (
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  code TEXT NOT NULL,
  diagnostic TEXT NOT NULL,
  PRIMARY KEY (file_id, ord)
) STRICT;
CREATE TABLE code_worktrees (
  wt INTEGER PRIMARY KEY REFERENCES worktrees ON DELETE CASCADE,
  code_fp TEXT NOT NULL
) STRICT;
CREATE TABLE code_packages (
  wt INTEGER NOT NULL REFERENCES code_worktrees ON DELETE CASCADE,
  dir TEXT NOT NULL,
  manifest_b3 TEXT,
  source TEXT NOT NULL,
  targets TEXT NOT NULL,
  PRIMARY KEY (wt, dir)
) STRICT;
CREATE TABLE code_files (
  file_id INTEGER PRIMARY KEY,
  wt INTEGER NOT NULL REFERENCES code_worktrees ON DELETE CASCADE,
  path TEXT NOT NULL,
  lang TEXT NOT NULL,
  blake3 TEXT,
  size INTEGER NOT NULL,
  read_error TEXT,
  utf8 INTEGER NOT NULL,
  package TEXT,
  role TEXT,
  module TEXT,
  orphan_errors INTEGER NOT NULL,
  targets_fp TEXT,
  declared_by TEXT,
  UNIQUE (wt, path)
) STRICT;
CREATE TABLE symbols (
  file INTEGER NOT NULL REFERENCES code_files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  item_kind TEXT NOT NULL,
  name TEXT NOT NULL,
  owner TEXT,
  qpath TEXT NOT NULL,
  discriminator TEXT,
  parent_ord INTEGER,
  start_line INTEGER NOT NULL,
  end_line INTEGER NOT NULL,
  start_byte INTEGER NOT NULL,
  end_byte INTEGER NOT NULL,
  signature TEXT,
  level_state TEXT NOT NULL,
  level_categories TEXT NOT NULL,
  sig_hash TEXT,
  body_hash TEXT,
  deps_hash TEXT,
  skeleton_hash TEXT,
  quote_hash TEXT,
  body_bytes INTEGER,
  layer TEXT NOT NULL,
  twin_files INTEGER NOT NULL,
  PRIMARY KEY (file, ord)
) STRICT;
CREATE TABLE symbol_suffixes (
  file INTEGER NOT NULL REFERENCES code_files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  depth INTEGER NOT NULL,
  suffix TEXT NOT NULL,
  PRIMARY KEY (file, ord, depth)
) STRICT;
CREATE TABLE code_decls (
  file INTEGER NOT NULL REFERENCES code_files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  target TEXT NOT NULL,
  line INTEGER NOT NULL,
  PRIMARY KEY (file, ord)
) STRICT;
CREATE TABLE code_markers (
  file INTEGER NOT NULL REFERENCES code_files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  relation TEXT NOT NULL,
  token TEXT NOT NULL,
  id TEXT NOT NULL,
  rev INTEGER,
  levels TEXT,
  levels_state TEXT NOT NULL,
  note TEXT,
  line INTEGER NOT NULL,
  \"column\" INTEGER NOT NULL,
  binding TEXT NOT NULL,
  symbol_ord INTEGER,
  anchor TEXT,
  PRIMARY KEY (file, ord)
) STRICT;
CREATE TABLE code_mentions (
  file INTEGER NOT NULL REFERENCES code_files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  written TEXT NOT NULL,
  line INTEGER NOT NULL,
  \"column\" INTEGER NOT NULL,
  binding TEXT NOT NULL,
  symbol_ord INTEGER,
  anchor TEXT,
  PRIMARY KEY (file, ord)
) STRICT;
CREATE TABLE code_roles (
  file INTEGER NOT NULL REFERENCES code_files ON DELETE CASCADE,
  ord INTEGER NOT NULL,
  word TEXT NOT NULL,
  source TEXT NOT NULL,
  form TEXT,
  origin TEXT NOT NULL,
  group_text TEXT,
  symbol_ord INTEGER,
  subject TEXT,
  subject_name TEXT,
  certainty TEXT NOT NULL,
  category TEXT,
  line INTEGER NOT NULL,
  PRIMARY KEY (file, ord)
) STRICT;
CREATE TABLE code_resolve (
  wt INTEGER PRIMARY KEY REFERENCES code_worktrees ON DELETE CASCADE,
  tree_fp TEXT NOT NULL,
  protocol INTEGER NOT NULL,
  ra_ap TEXT NOT NULL,
  host TEXT NOT NULL,
  toolchains TEXT NOT NULL,
  degraded INTEGER NOT NULL,
  workspaces INTEGER NOT NULL,
  files INTEGER NOT NULL,
  items INTEGER NOT NULL
) STRICT;
CREATE TABLE code_resolve_files (
  wt INTEGER NOT NULL REFERENCES code_resolve ON DELETE CASCADE,
  path TEXT NOT NULL,
  blake3 TEXT,
  in_crate INTEGER NOT NULL,
  PRIMARY KEY (wt, path)
) STRICT;
CREATE TABLE code_monikers (
  wt INTEGER NOT NULL REFERENCES code_resolve ON DELETE CASCADE,
  path TEXT NOT NULL,
  start INTEGER NOT NULL,
  \"end\" INTEGER NOT NULL,
  kind TEXT NOT NULL,
  state TEXT NOT NULL,
  moniker TEXT,
  PRIMARY KEY (wt, path, start)
) STRICT;
CREATE INDEX nodes_by_id ON nodes (id);
CREATE INDEX links_by_dst_id ON links (dst_id);
CREATE INDEX aliases_by_alias ON aliases (alias);
CREATE INDEX anchors_by_name ON anchors (name);
CREATE INDEX symbols_by_qpath ON symbols (qpath);
CREATE INDEX symbols_by_name ON symbols (name);
CREATE INDEX symbol_suffixes_by_suffix ON symbol_suffixes (suffix);
CREATE INDEX code_decls_by_target ON code_decls (target);
CREATE INDEX code_roles_by_subject_name ON code_roles (subject_name);
CREATE INDEX code_monikers_by_moniker ON code_monikers (moniker);
CREATE VIRTUAL TABLE nodes_fts USING fts5(
  id, title, own_text,
  content='nodes', content_rowid='node_id',
  tokenize='trigram case_sensitive 0'
);
CREATE TRIGGER nodes_fts_insert AFTER INSERT ON nodes BEGIN
  INSERT INTO nodes_fts (rowid, id, title, own_text)
    VALUES (new.node_id, new.id, new.title, new.own_text);
END;
CREATE TRIGGER nodes_fts_delete AFTER DELETE ON nodes BEGIN
  INSERT INTO nodes_fts (nodes_fts, rowid, id, title, own_text)
    VALUES ('delete', old.node_id, old.id, old.title, old.own_text);
END;
CREATE TRIGGER nodes_fts_update AFTER UPDATE ON nodes BEGIN
  INSERT INTO nodes_fts (nodes_fts, rowid, id, title, own_text)
    VALUES ('delete', old.node_id, old.id, old.title, old.own_text);
  INSERT INTO nodes_fts (rowid, id, title, own_text)
    VALUES (new.node_id, new.id, new.title, new.own_text);
END;
";

/// Every trigger of every index format, dropped before a recreation. A later
/// format appends its names here and never removes one, so any older DB
/// loses exactly the index's objects and nothing of Phase 2's own tables.
const FORMAT_TRIGGERS: &[&str] = &["nodes_fts_insert", "nodes_fts_delete", "nodes_fts_update"];

/// Every table of every index format but `index_meta`, children first; a
/// later format inserts its tables where their parents ask and never
/// removes one, as [`FORMAT_TRIGGERS`]. The code tables of format 9
/// (`docs/canon/symbol-index.md` "Tables") come before `worktrees`, which
/// `code_worktrees` references; format 10's `code_roles` before
/// `code_files`, which it references; format 11's layer C snapshot
/// (`code_monikers`, `code_resolve_files`, then `code_resolve`) before
/// `code_worktrees`, which `code_resolve` references.
const FORMAT_TABLES: &[&str] = &[
    "nodes_fts",
    "aliases",
    "links",
    "anchors",
    "diagnostics",
    "nodes",
    "files",
    "symbol_suffixes",
    "symbols",
    "code_decls",
    "code_markers",
    "code_mentions",
    "code_roles",
    "code_files",
    "code_packages",
    "code_monikers",
    "code_resolve_files",
    "code_resolve",
    "code_worktrees",
    "worktrees",
];

/// Per-connection settings; the handle applies them on every open.
pub(crate) fn configure(conn: &Connection) -> Result<(), StoreError> {
    conn.pragma_update(None, "busy_timeout", BUSY_TIMEOUT_MS)
        .db()?;
    conn.pragma_update(None, "foreign_keys", "ON").db()?;
    conn.pragma_update(None, "synchronous", "NORMAL").db()?;
    conn.pragma_update(None, "journal_size_limit", JOURNAL_SIZE_LIMIT)
        .db()?;
    conn.pragma_update(None, "trusted_schema", "OFF").db()?;
    conn.pragma_update(None, "recursive_triggers", "OFF").db()?;
    Ok(())
}

/// Creates the index tables when the DB has none. On a DB without any table
/// (a new file) the creation PRAGMAs come first: `auto_vacuum` before any
/// table exists, then WAL. A DB that has other tables but no `index_meta`
/// gets the index tables beside them (nothing is dropped); a name collision
/// is an error.
pub(crate) fn ensure(conn: &mut Connection) -> Result<(), StoreError> {
    if has_table(conn, "index_meta")? {
        return Ok(());
    }
    prepare_new(conn)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .db()?;
    if !has_table(&tx, "index_meta")? {
        create(&tx)?;
    }
    tx.commit().db()
}

/// On a DB without any table (a new file), the creation PRAGMAs: first
/// `auto_vacuum=INCREMENTAL` (before any table exists), then WAL. A DB that
/// has a table is left as it is. Shared by the index and the queue
/// (`crate::queue`), whichever makes the first table.
pub(crate) fn prepare_new(conn: &Connection) -> Result<(), StoreError> {
    if has_any_table(conn)? {
        return Ok(());
    }
    conn.pragma_update(None, "auto_vacuum", "INCREMENTAL")
        .db()?;
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .db()?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Sqlite(format!(
            "the database refused WAL mode (journal_mode stays {mode})"
        )));
    }
    Ok(())
}

/// The stamp stored in the DB; `None` when absent or unreadable.
pub(crate) fn stored_format(conn: &Connection) -> Result<Option<String>, StoreError> {
    if !has_table(conn, "index_meta")? {
        return Ok(None);
    }
    conn.query_row(
        "SELECT value FROM index_meta WHERE key = ?1",
        [FORMAT_KEY],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .db()
}

/// The stored stamp is this build's.
pub(crate) fn format_is_current(conn: &Connection) -> Result<bool, StoreError> {
    Ok(stored_format(conn)?.as_deref() == Some(INDEX_FORMAT.to_string().as_str()))
}

/// Inside a write transaction: when the stamp is not this build's, drops
/// every index object of any format and creates the current ones. `true`
/// when it did.
pub(crate) fn recreate_if_stale(tx: &Transaction<'_>) -> Result<bool, StoreError> {
    if format_is_current(tx)? {
        return Ok(false);
    }
    // Dropping a parent table deletes its rows first; the checks wait for
    // the commit, when no child table is left.
    tx.pragma_update(None, "defer_foreign_keys", "ON").db()?;
    for trigger in FORMAT_TRIGGERS {
        tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {}", quoted(trigger)))
            .db()?;
    }
    for table in FORMAT_TABLES {
        tx.execute_batch(&format!("DROP TABLE IF EXISTS {}", quoted(table)))
            .db()?;
    }
    tx.execute_batch("DROP TABLE IF EXISTS index_meta").db()?;
    create(tx)?;
    Ok(true)
}

fn create(tx: &Transaction<'_>) -> Result<(), StoreError> {
    tx.execute_batch(SCHEMA).db()?;
    tx.execute(
        "INSERT INTO index_meta (key, value) VALUES (?1, ?2)",
        [FORMAT_KEY, INDEX_FORMAT.to_string().as_str()],
    )
    .db()?;
    Ok(())
}

pub(crate) fn has_table(conn: &Connection, name: &str) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .db()
}

fn has_any_table(conn: &Connection) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .db()
}

/// An SQL identifier in double quotes.
pub(crate) fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
