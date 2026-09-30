//! The read side: one deferred (read-only) transaction per call, so a call
//! sees one snapshot even while another connection writes.

use std::collections::BTreeMap;

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params_from_iter};
use specengine_core::check::{CheckFile, CheckInput};
use specengine_model::Node;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::rows::{from_json, rebuild_parsed};
use crate::search::fts_query;
use crate::{
    IdHit, IndexedFile, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN, SearchHit, SearchQuery, SearchResults,
    schema,
};

/// Runs `read` over the handle's worktree in one read transaction;
/// [`StoreError::NotIndexed`] when the stamp is stale or the worktree has no
/// row.
pub(crate) fn with_worktree<T>(
    index: &SqliteIndex,
    read: impl FnOnce(&Transaction<'_>, i64) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Deferred).db()?;
    if !schema::format_is_current(&tx)? {
        return Err(StoreError::NotIndexed);
    }
    let wt = worktree_id(&tx, index)?.ok_or(StoreError::NotIndexed)?;
    let value = read(&tx, wt)?;
    tx.commit().db()?;
    Ok(value)
}

/// The worktree's surrogate key, when it has a row.
pub(crate) fn worktree_id(
    tx: &Transaction<'_>,
    index: &SqliteIndex,
) -> Result<Option<i64>, StoreError> {
    tx.query_row(
        "SELECT wt FROM worktrees WHERE project = ?1 AND root = ?2",
        [index.project.as_str(), index.root_text.as_str()],
        |row| row.get(0),
    )
    .optional()
    .db()
}

pub(crate) fn files(index: &SqliteIndex) -> Result<Vec<String>, StoreError> {
    with_worktree(index, |tx, wt| {
        let mut statement = tx
            .prepare("SELECT path FROM files WHERE wt = ?1 ORDER BY path")
            .db()?;
        let paths = statement
            .query_map([wt], |row| row.get::<_, String>(0))
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        Ok(paths)
    })
}

pub(crate) fn file(index: &SqliteIndex, path: &str) -> Result<Option<IndexedFile>, StoreError> {
    with_worktree(index, |tx, wt| {
        let row = tx
            .query_row(
                "SELECT file_id, blake3, size, read_error, shell FROM files
                 WHERE wt = ?1 AND path = ?2",
                (wt, path),
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .db()?;
        let Some((file_id, blake3, size, read_error, shell)) = row else {
            return Ok(None);
        };
        let parsed = match shell {
            None => None,
            Some(shell) => {
                let column = |sql: &str| -> Result<Vec<String>, StoreError> {
                    let mut statement = tx.prepare(sql).db()?;
                    let values = statement
                        .query_map([file_id], |row| row.get::<_, String>(0))
                        .db()?
                        .collect::<Result<Vec<_>, _>>()
                        .db()?;
                    Ok(values)
                };
                Some(rebuild_parsed(
                    path,
                    &shell,
                    &column("SELECT node FROM nodes WHERE file_id = ?1 ORDER BY ord")?,
                    &column("SELECT link FROM links WHERE file_id = ?1 ORDER BY ord")?,
                    &column("SELECT anchor FROM anchors WHERE file_id = ?1 ORDER BY ord")?,
                    &column("SELECT diagnostic FROM diagnostics WHERE file_id = ?1 ORDER BY ord")?,
                )?)
            }
        };
        Ok(Some(IndexedFile {
            parsed,
            blake3,
            size: u64::try_from(size).unwrap_or(0),
            read_error,
        }))
    })
}

pub(crate) fn lookup_id(index: &SqliteIndex, id: &str) -> Result<Vec<IdHit>, StoreError> {
    with_worktree(index, |tx, wt| {
        let mut statement = tx
            .prepare(
                "SELECT f.path, n.ord, n.node FROM nodes n
                 JOIN files f ON f.file_id = n.file_id
                 WHERE f.wt = ?1 AND n.id = ?2
                 ORDER BY f.path, n.ord",
            )
            .db()?;
        let rows = statement
            .query_map((wt, id), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        rows.into_iter()
            .map(|(path, ord, node)| {
                Ok(IdHit {
                    path,
                    ord: ord_of(ord),
                    node: from_json::<Node>(&node, "node")?,
                })
            })
            .collect()
    })
}

pub(crate) fn search(
    index: &SqliteIndex,
    query: &SearchQuery,
) -> Result<SearchResults, StoreError> {
    with_worktree(index, |tx, wt| {
        let Some(matching) = fts_query(&query.text) else {
            return Ok(SearchResults {
                hits: Vec::new(),
                short_query: true,
                tier3_left_out: 0,
            });
        };
        // `?1` the match, `?2` the worktree, then the kinds; the limit last.
        let mut values = vec![Value::Text(matching), Value::Integer(wt)];
        let mut filter = String::new();
        if !query.kinds.is_empty() {
            let placeholders: Vec<String> = (0..query.kinds.len())
                .map(|offset| format!("?{}", values.len() + 1 + offset))
                .collect();
            filter = format!(" AND n.kind IN ({})", placeholders.join(", "));
            values.extend(query.kinds.iter().cloned().map(Value::Text));
        }
        const MATCHES: &str = "FROM nodes_fts
             JOIN nodes n ON n.node_id = nodes_fts.rowid
             JOIN files f ON f.file_id = n.file_id
             WHERE nodes_fts MATCH ?1 AND f.wt = ?2";
        // Tier 3 files leave in the query itself, before the limit.
        let archive = if query.archive {
            ""
        } else {
            " AND f.tier3 = 0"
        };
        let tier3_left_out = if query.archive {
            0
        } else {
            let count: i64 = tx
                .query_row(
                    &format!("SELECT count(*) {MATCHES}{filter} AND f.tier3 = 1"),
                    params_from_iter(values.iter()),
                    |row| row.get(0),
                )
                .db()?;
            u32::try_from(count).unwrap_or(u32::MAX)
        };
        let limit = query.limit.clamp(SEARCH_LIMIT_MIN, SEARCH_LIMIT_MAX);
        let limit_param = values.len() + 1;
        values.push(Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
        let sql = format!(
            "SELECT f.path, n.ord, n.line, f.tier3, n.id, n.kind, n.title,
                    snippet(nodes_fts, -1, '**', '**', '…', 64)
             {MATCHES}{filter}{archive}
             ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0), f.path, n.ord
             LIMIT ?{limit_param}"
        );
        let mut statement = tx.prepare(&sql).db()?;
        let hits = statement
            .query_map(params_from_iter(values), |row| {
                Ok(SearchHit {
                    path: row.get(0)?,
                    ord: ord_of(row.get(1)?),
                    line: ord_of(row.get(2)?),
                    tier3: row.get::<_, i64>(3)? != 0,
                    id: row.get(4)?,
                    kind: row.get(5)?,
                    title: row.get(6)?,
                    snippet: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                })
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        Ok(SearchResults {
            hits,
            short_query: false,
            tier3_left_out,
        })
    })
}

pub(crate) fn indexed_input(index: &SqliteIndex) -> Result<CheckInput, StoreError> {
    with_worktree(index, |tx, wt| {
        // file_id → its rows' JSON in `ord` order, for one of the row tables.
        let rows_of =
            |table: &str, column: &str| -> Result<BTreeMap<i64, Vec<String>>, StoreError> {
                let mut statement = tx
                    .prepare(&format!(
                        "SELECT x.file_id, x.{column} FROM {table} x
                     JOIN files f ON f.file_id = x.file_id
                     WHERE f.wt = ?1
                     ORDER BY x.file_id, x.ord"
                    ))
                    .db()?;
                let mut grouped: BTreeMap<i64, Vec<String>> = BTreeMap::new();
                let rows = statement
                    .query_map([wt], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })
                    .db()?;
                for row in rows {
                    let (file_id, value) = row.db()?;
                    grouped.entry(file_id).or_default().push(value);
                }
                Ok(grouped)
            };
        let mut nodes = rows_of("nodes", "node")?;
        let mut links = rows_of("links", "link")?;
        let mut anchors = rows_of("anchors", "anchor")?;
        let mut diagnostics = rows_of("diagnostics", "diagnostic")?;

        let mut statement = tx
            .prepare(
                "SELECT file_id, path, size, read_error, shell FROM files
                 WHERE wt = ?1 ORDER BY path",
            )
            .db()?;
        let files = statement
            .query_map([wt], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        let mut input = CheckInput::default();
        for (file_id, path, size, read_error, shell) in files {
            let parsed = match shell {
                None => None,
                Some(shell) => Some(rebuild_parsed(
                    &path,
                    &shell,
                    &nodes.remove(&file_id).unwrap_or_default(),
                    &links.remove(&file_id).unwrap_or_default(),
                    &anchors.remove(&file_id).unwrap_or_default(),
                    &diagnostics.remove(&file_id).unwrap_or_default(),
                )?),
            };
            input.files.push(CheckFile {
                path,
                size: u64::try_from(size).unwrap_or(0),
                parsed,
                read_error,
                bytes: Vec::new(),
            });
        }
        Ok(input)
    })
}

fn ord_of(ord: i64) -> usize {
    usize::try_from(ord).unwrap_or(usize::MAX)
}
