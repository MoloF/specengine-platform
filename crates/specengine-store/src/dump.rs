//! The canonical dump (`crates/specengine-store/README.md`, "Rows"): every
//! index table in column order (every column, read from the schema) with
//! its surrogate keys (`wt`, `file_id`, `node_id`) replaced by `(project,
//! root)`, `path`, `(path, ord)`, rows sorted, plus the FTS5 vocabulary.
//! Two DBs with equal dumps hold the same index; storage order and rowids
//! never show.

use rusqlite::types::ValueRef;
use rusqlite::{Transaction, TransactionBehavior};
use serde_json::Value;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::with_worktree;
use crate::schema;

/// The owner of a row, surrogates replaced (LEFT JOINs: an orphan row shows
/// NULLs instead of vanishing). Every index table is named `main.…`, so no
/// temp object of the connection can shadow it.
const FILE_OWNER: &str = "LEFT JOIN main.files f ON f.file_id = x.file_id
     LEFT JOIN main.worktrees w ON w.wt = f.wt";

/// How one index table is dumped: its owner's columns first (`w`, `f`, `n`
/// are the owning worktree, file and node), then every column of its own in
/// schema order but the surrogate keys `skip`; `?1`, where the filter has
/// it, is the worktree or NULL.
struct TableDump {
    table: &'static str,
    owner: &'static str,
    joins: &'static str,
    filter: &'static str,
    skip: &'static [&'static str],
}

/// Every index table, in dump order.
const TABLES: &[TableDump] = &[
    TableDump {
        table: "index_meta",
        owner: "",
        joins: "",
        filter: "",
        skip: &[],
    },
    TableDump {
        table: "worktrees",
        owner: "",
        joins: "",
        filter: "WHERE ?1 IS NULL OR x.wt = ?1",
        skip: &["wt"],
    },
    TableDump {
        table: "files",
        owner: "w.project, w.root",
        joins: "LEFT JOIN main.worktrees w ON w.wt = x.wt",
        filter: "WHERE ?1 IS NULL OR x.wt = ?1",
        skip: &["file_id", "wt"],
    },
    TableDump {
        table: "nodes",
        owner: "w.project, w.root, f.path",
        joins: FILE_OWNER,
        filter: "WHERE ?1 IS NULL OR f.wt = ?1",
        skip: &["node_id", "file_id"],
    },
    TableDump {
        table: "aliases",
        owner: "w.project, w.root, f.path, n.ord",
        joins: "LEFT JOIN main.nodes n ON n.node_id = x.node_id
             LEFT JOIN main.files f ON f.file_id = n.file_id
             LEFT JOIN main.worktrees w ON w.wt = f.wt",
        filter: "WHERE ?1 IS NULL OR f.wt = ?1",
        skip: &["node_id"],
    },
    TableDump {
        table: "links",
        owner: "w.project, w.root, f.path",
        joins: FILE_OWNER,
        filter: "WHERE ?1 IS NULL OR f.wt = ?1",
        skip: &["file_id"],
    },
    TableDump {
        table: "anchors",
        owner: "w.project, w.root, f.path",
        joins: FILE_OWNER,
        filter: "WHERE ?1 IS NULL OR f.wt = ?1",
        skip: &["file_id"],
    },
    TableDump {
        table: "diagnostics",
        owner: "w.project, w.root, f.path",
        joins: FILE_OWNER,
        filter: "WHERE ?1 IS NULL OR f.wt = ?1",
        skip: &["file_id"],
    },
];

/// `(table, SELECT … FROM main.<table> x <joins> <filter>)` for every index
/// table. A table's own columns are read from the database, never listed
/// here: `pragma_table_xinfo` of the `main` schema, in schema order, which
/// (unlike `pragma_table_info`) includes generated columns too. A column
/// added to the schema always enters the dump, so it always needs a new
/// format line.
fn tables(tx: &Transaction<'_>) -> Result<Vec<(&'static str, String)>, StoreError> {
    let mut statement = tx
        .prepare("SELECT name FROM pragma_table_xinfo(?1, 'main') ORDER BY cid")
        .db()?;
    let mut queries = Vec::with_capacity(TABLES.len());
    for dump in TABLES {
        let names = statement
            .query_map([dump.table], |row| row.get::<_, String>(0))
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        let mut columns: Vec<String> = Vec::new();
        if !dump.owner.is_empty() {
            columns.push(dump.owner.to_owned());
        }
        columns.extend(
            names
                .iter()
                .filter(|name| !dump.skip.contains(&name.as_str()))
                .map(|name| format!("x.{}", schema::quoted(name))),
        );
        queries.push((
            dump.table,
            format!(
                "SELECT {} FROM main.{} x {} {}",
                columns.join(", "),
                schema::quoted(dump.table),
                dump.joins,
                dump.filter
            ),
        ));
    }
    Ok(queries)
}

/// Every worktree, and the DB-wide vocabulary (`fts5vocab` `row`).
pub(crate) fn whole(index: &SqliteIndex) -> Result<String, StoreError> {
    create_vocabularies(index)?;
    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Deferred).db()?;
    if !schema::format_is_current(&tx)? {
        return Err(StoreError::NotIndexed);
    }
    let mut out = String::new();
    for (table, sql) in tables(&tx)? {
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
        for (table, sql) in tables(tx)? {
            dump_table(tx, table, &sql, Some(wt), &mut out)?;
        }
        dump_table(
            tx,
            "fts5vocab_instance",
            "SELECT x.term, f.path, n.ord, x.col, x.offset
             FROM temp.nodes_fts_vocab_instance x
             JOIN main.nodes n ON n.node_id = x.doc
             JOIN main.files f ON f.file_id = n.file_id
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
