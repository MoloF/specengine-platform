//! The canonical dump (`crates/specengine-store/README.md`, "Rows"): every
//! index table in column order with its surrogate keys (`wt`, `file_id`,
//! `node_id`) replaced by `(project, root)`, `path`, `(path, ord)`, rows
//! sorted, plus the FTS5 vocabulary. Two DBs with equal dumps hold the same
//! index; storage order and rowids never show.

use rusqlite::types::ValueRef;
use rusqlite::{Transaction, TransactionBehavior};
use serde_json::Value;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::with_worktree;
use crate::schema;

/// The owner of a row, surrogates replaced (LEFT JOINs: an orphan row shows
/// NULLs instead of vanishing).
const FILE_OWNER: &str = "LEFT JOIN files f ON f.file_id = x.file_id
     LEFT JOIN worktrees w ON w.wt = f.wt";

/// `(table, SELECT … FROM <table> x <joins>)`; `w`, `f`, `n` are the owning
/// worktree, file and node; `?1`, where present, is the worktree or NULL.
fn tables() -> Vec<(&'static str, String)> {
    vec![
        (
            "index_meta",
            "SELECT x.key, x.value FROM index_meta x".to_owned(),
        ),
        (
            "worktrees",
            "SELECT x.project, x.root, x.scheme_fp FROM worktrees x
             WHERE ?1 IS NULL OR x.wt = ?1"
                .to_owned(),
        ),
        (
            "files",
            "SELECT w.project, w.root, x.path, x.blake3, x.size, x.read_error, x.shell
             FROM files x LEFT JOIN worktrees w ON w.wt = x.wt
             WHERE ?1 IS NULL OR x.wt = ?1"
                .to_owned(),
        ),
        (
            "nodes",
            format!(
                "SELECT w.project, w.root, f.path, x.ord, x.id, x.kind, x.title, x.parent_id,
                        x.own_text, x.node
                 FROM nodes x {FILE_OWNER}
                 WHERE ?1 IS NULL OR f.wt = ?1"
            ),
        ),
        (
            "aliases",
            "SELECT w.project, w.root, f.path, n.ord, x.ord, x.alias
             FROM aliases x
             LEFT JOIN nodes n ON n.node_id = x.node_id
             LEFT JOIN files f ON f.file_id = n.file_id
             LEFT JOIN worktrees w ON w.wt = f.wt
             WHERE ?1 IS NULL OR f.wt = ?1"
                .to_owned(),
        ),
        (
            "links",
            format!(
                "SELECT w.project, w.root, f.path, x.ord, x.src, x.type, x.dst_id, x.dst_path,
                        x.link
                 FROM links x {FILE_OWNER}
                 WHERE ?1 IS NULL OR f.wt = ?1"
            ),
        ),
        (
            "anchors",
            format!(
                "SELECT w.project, w.root, f.path, x.ord, x.name, x.anchor
                 FROM anchors x {FILE_OWNER}
                 WHERE ?1 IS NULL OR f.wt = ?1"
            ),
        ),
        (
            "diagnostics",
            format!(
                "SELECT w.project, w.root, f.path, x.ord, x.code, x.diagnostic
                 FROM diagnostics x {FILE_OWNER}
                 WHERE ?1 IS NULL OR f.wt = ?1"
            ),
        ),
    ]
}

/// Every worktree, and the DB-wide vocabulary (`fts5vocab` `row`).
pub(crate) fn whole(index: &SqliteIndex) -> Result<String, StoreError> {
    create_vocabularies(index)?;
    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Deferred).db()?;
    if !schema::format_is_current(&tx)? {
        return Err(StoreError::NotIndexed);
    }
    let mut out = String::new();
    for (table, sql) in tables() {
        dump_table(&tx, table, &sql, None, &mut out)?;
    }
    dump_table(
        &tx,
        "fts5vocab_row",
        "SELECT x.term, x.doc, x.cnt FROM temp.nodes_fts_vocab_row x",
        None,
        &mut out,
    )?;
    tx.commit().db()?;
    Ok(out)
}

/// The handle's worktree, and the vocabulary of its own nodes
/// (`fts5vocab` `instance`).
pub(crate) fn worktree(index: &SqliteIndex) -> Result<String, StoreError> {
    create_vocabularies(index)?;
    with_worktree(index, |tx, wt| {
        let mut out = String::new();
        for (table, sql) in tables() {
            dump_table(tx, table, &sql, Some(wt), &mut out)?;
        }
        dump_table(
            tx,
            "fts5vocab_instance",
            "SELECT x.term, f.path, n.ord, x.col, x.offset
             FROM temp.nodes_fts_vocab_instance x
             JOIN nodes n ON n.node_id = x.doc
             JOIN files f ON f.file_id = n.file_id
             WHERE f.wt = ?1",
            Some(wt),
            &mut out,
        )?;
        Ok(out)
    })
}

/// Per-connection views of the FTS5 index, in the temp schema (nothing is
/// written to the DB file).
fn create_vocabularies(index: &SqliteIndex) -> Result<(), StoreError> {
    index
        .conn
        .execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS temp.nodes_fts_vocab_row
               USING fts5vocab(main, nodes_fts, row);
             CREATE VIRTUAL TABLE IF NOT EXISTS temp.nodes_fts_vocab_instance
               USING fts5vocab(main, nodes_fts, instance);",
        )
        .db()
}

/// Appends `<table>\t<JSON array>` per row, rows sorted.
fn dump_table(
    tx: &Transaction<'_>,
    table: &str,
    sql: &str,
    wt: Option<i64>,
    out: &mut String,
) -> Result<(), StoreError> {
    let mut statement = tx.prepare(sql).db()?;
    let columns = statement.column_count();
    let mut lines: Vec<String> = Vec::new();
    // `?1` is the worktree (NULL: every worktree) where the query has one.
    let mut rows = if statement.parameter_count() == 0 {
        statement.query([]).db()?
    } else {
        statement.query([wt]).db()?
    };
    while let Some(row) = rows.next().db()? {
        let mut values = Vec::with_capacity(columns);
        for column in 0..columns {
            values.push(match row.get_ref(column).db()? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(value) => Value::from(value),
                ValueRef::Real(value) => Value::from(value),
                ValueRef::Text(bytes) => Value::from(String::from_utf8_lossy(bytes).into_owned()),
                ValueRef::Blob(bytes) => Value::from(blake_hex(bytes)),
            });
        }
        lines.push(Value::Array(values).to_string());
    }
    lines.sort();
    for line in lines {
        out.push_str(table);
        out.push('\t');
        out.push_str(&line);
        out.push('\n');
    }
    Ok(())
}

/// A blob shows as the BLAKE3 of its bytes (the index stores none).
fn blake_hex(bytes: &[u8]) -> String {
    format!("blob:{}", blake3::hash(bytes).to_hex())
}
