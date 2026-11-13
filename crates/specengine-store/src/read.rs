//! The read side: one deferred (read-only) transaction per call, so a call
//! sees one snapshot even while another connection writes.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Statement, Transaction, TransactionBehavior, params_from_iter};
use specengine_core::Paths;
use specengine_core::check::{CheckFile, CheckInput, DocClass};
use specengine_core::collisions::{CollisionNode, CollisionRows, CollisionTarget};
use specengine_model::{IdScheme, MENTIONS, Node};

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::rows::{from_json, rebuild_parsed};
use crate::search::fts_query;
use crate::{
    IdHit, IndexedFile, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN, SearchHit, SearchQuery, SearchResults,
    Snippet, WalkNotes, schema, write,
};

/// One node's `norm_hash` as the index stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeNorm {
    pub path: String,
    /// 0 the document, then its sections in source order.
    pub ord: usize,
    /// `None`: null (`docs/canon/node-revision.md` "Null").
    pub norm_hash: Option<String>,
}

impl SqliteIndex {
    /// Every node of the handle's worktree with its stored `norm_hash`, by
    /// (path, position), in one snapshot; [`StoreError::NotIndexed`] as every
    /// read of the worktree.
    pub fn node_norms(&self) -> Result<Vec<NodeNorm>, StoreError> {
        with_worktree(self, |tx, wt| {
            let mut statement = tx
                .prepare(
                    "SELECT f.path, n.ord, n.norm_hash FROM nodes n
                     JOIN files f ON f.file_id = n.file_id
                     WHERE f.wt = ?1 ORDER BY f.path, n.ord",
                )
                .db()?;
            statement
                .query_map([wt], |row| {
                    Ok(NodeNorm {
                        path: row.get(0)?,
                        ord: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(usize::MAX),
                        norm_hash: row.get(2)?,
                    })
                })
                .db()?
                .collect::<Result<Vec<_>, _>>()
                .db()
        })
    }
}

/// The universe of the collision pass, `f` the file and `n` the node: the
/// worktree `?1`'s documents and ID sections with a kind, in files neither
/// Tier 3 nor generated (the latter left out by the caller).
const COLLISION_NODES: &str = "SELECT f.file_id, f.path, n.ord, n.id, n.kind, n.title, n.norm_hash
     FROM files f JOIN nodes n ON n.file_id = f.file_id
     WHERE f.wt = ?1 AND f.tier3 = 0 AND n.kind IS NOT NULL AND (n.ord = 0 OR n.id IS NOT NULL)
     ORDER BY f.path, n.ord";

/// The worktree `?1`'s generated files that are not Tier 3: their document's
/// front-matter `class` is `?2`.
const GENERATED_FILES: &str = "SELECT f.file_id FROM files f JOIN nodes d ON d.file_id = f.file_id
     WHERE f.wt = ?1 AND f.tier3 = 0 AND d.ord = 0
       AND json_extract(d.node, '$.fields.class') = ?2";

/// The IDs the worktree `?1`'s nodes outside the universe hold, in a Tier 3
/// file or with no kind; a generated file's are read with the universe's rows
/// (task spec `release-hardening` RH-R6).
const OUTSIDE_IDS: &str = "SELECT DISTINCT n.id FROM files f JOIN nodes n ON n.file_id = f.file_id
     WHERE f.wt = ?1 AND n.id IS NOT NULL AND (f.tier3 <> 0 OR n.kind IS NULL)";

/// The resolved ends of the edges a node of a live file holds, the holder
/// their source, any type but `?2`, each with its end's ID when it has one.
const COLLISION_EDGES: &str = "SELECT f.path, e.holder, e.end_path, e.end_ord, tn.id
     FROM files f CROSS JOIN edges e ON e.file_id = f.file_id
     LEFT JOIN files tf ON tf.wt = f.wt AND tf.path = e.end_path
     LEFT JOIN nodes tn ON tn.file_id = tf.file_id AND tn.ord = e.end_ord
     WHERE f.wt = ?1 AND f.tier3 = 0 AND e.names_source = 0 AND e.type <> ?2
       AND e.end_path IS NOT NULL AND e.end_ord IS NOT NULL
     ORDER BY f.path, e.holder, e.end_path, e.end_ord";

/// The resolved `parent:` ends of a live file's document, each with its
/// end's ID when it has one.
const COLLISION_PARENTS: &str = "SELECT f.path, p.end_path, p.end_ord, tn.id
     FROM files f CROSS JOIN edge_parents p ON p.file_id = f.file_id
     LEFT JOIN files tf ON tf.wt = f.wt AND tf.path = p.end_path
     LEFT JOIN nodes tn ON tn.file_id = tf.file_id AND tn.ord = p.end_ord
     WHERE f.wt = ?1 AND f.tier3 = 0 AND p.end_path IS NOT NULL AND p.end_ord IS NOT NULL
     ORDER BY f.path, p.end_path, p.end_ord";

/// [`crate::SpecIndex::collision_input`]: in live files (neither Tier 3 nor
/// generated: `docs/canon/spec-cli-graph.md` "Live sources").
pub(crate) fn collision_input(index: &SqliteIndex) -> Result<CollisionRows, StoreError> {
    with_worktree(index, |tx, wt| {
        let generated: BTreeSet<i64> = {
            let mut statement = tx.prepare(GENERATED_FILES).db()?;
            statement
                .query_map((wt, DocClass::Generated.as_str()), |row| row.get(0))
                .db()?
                .collect::<Result<_, _>>()
                .db()?
        };
        let mut rows = CollisionRows::default();
        let mut statement = tx.prepare(COLLISION_NODES).db()?;
        let mut found = statement.query([wt]).db()?;
        while let Some(row) = found.next().db()? {
            if generated.contains(&row.get::<_, i64>(0).db()?) {
                // Outside the universe, yet a holder the resolver counts.
                if let Some(id) = row.get::<_, Option<String>>(3).db()? {
                    rows.outside_ids.insert(id);
                }
                continue;
            }
            let node = CollisionNode {
                path: row.get(1).db()?,
                ord: ord_of(row.get(2).db()?),
                id: row.get(3).db()?,
                kind: row.get(4).db()?,
                title: row.get(5).db()?,
                norm_hash: row.get(6).db()?,
            };
            rows.skipped += usize::from(node.norm_hash.is_none());
            rows.nodes.push(node);
        }
        drop(found);
        let mut statement = tx.prepare(OUTSIDE_IDS).db()?;
        let mut found = statement.query([wt]).db()?;
        while let Some(row) = found.next().db()? {
            rows.outside_ids.insert(row.get(0).db()?);
        }
        drop(found);
        let held: BTreeSet<(&str, usize)> = rows
            .nodes
            .iter()
            .map(|node| (node.path.as_str(), node.ord))
            .collect();
        let mut targets = Vec::new();
        let mut statement = tx.prepare(COLLISION_EDGES).db()?;
        let mut found = statement.query((wt, MENTIONS)).db()?;
        while let Some(row) = found.next().db()? {
            let holder_path: String = row.get(0).db()?;
            let holder_ord = ord_of(row.get(1).db()?);
            if !held.contains(&(holder_path.as_str(), holder_ord)) {
                continue;
            }
            let end_path: String = row.get(2).db()?;
            let end_id: Option<String> = row.get(4).db()?;
            targets.push(CollisionTarget {
                holder_path,
                holder_ord,
                end_name: end_id.unwrap_or_else(|| end_path.clone()),
                end_path,
                end_ord: ord_of(row.get(3).db()?),
            });
        }
        drop(found);
        let mut statement = tx.prepare(COLLISION_PARENTS).db()?;
        let mut found = statement.query([wt]).db()?;
        while let Some(row) = found.next().db()? {
            let holder_path: String = row.get(0).db()?;
            if !held.contains(&(holder_path.as_str(), 0)) {
                continue;
            }
            let end_path: String = row.get(1).db()?;
            let end_id: Option<String> = row.get(3).db()?;
            targets.push(CollisionTarget {
                holder_path,
                holder_ord: 0,
                end_name: end_id.unwrap_or_else(|| end_path.clone()),
                end_path,
                end_ord: ord_of(row.get(2).db()?),
            });
        }
        drop(found);
        rows.targets = targets;
        Ok(rows)
    })
}

impl SqliteIndex {
    /// The last walk's notes, when the index can be trusted to answer as
    /// that walk left it (the `read-trust` spec, "Store API"): one deferred
    /// transaction, never a write. `Some` when the stamp is this build's,
    /// the handle's worktree has a row whose `[ids]` and `[paths]`
    /// fingerprints are `scheme`'s and `paths`', and its `walk_notes` row
    /// exists (and reads); else `None`.
    pub fn trusted_notes(
        &self,
        scheme: &IdScheme,
        paths: &Paths,
    ) -> Result<Option<WalkNotes>, StoreError> {
        let scheme_fp = write::fingerprint(scheme)?;
        let paths_fp = write::paths_fingerprint(paths);
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let notes = if schema::format_is_current(&tx)? {
            let stored = tx
                .query_row(
                    "SELECT wt, scheme_fp, paths_fp FROM worktrees
                     WHERE project = ?1 AND root = ?2",
                    [self.project.as_str(), self.root_text.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()
                .db()?;
            match stored {
                Some((wt, scheme, paths)) if scheme == scheme_fp && paths == paths_fp => {
                    walk_notes(&tx, wt)?
                }
                _ => None,
            }
        } else {
            None
        };
        tx.commit().db()?;
        Ok(notes)
    }
}

/// The worktree's `walk_notes` row; `None` when absent, or when it does not
/// read back (a walk then writes it again).
pub(crate) fn walk_notes(tx: &Transaction<'_>, wt: i64) -> Result<Option<WalkNotes>, StoreError> {
    let row = tx
        .query_row(
            "SELECT missing_roots, unreadable_dirs, skipped_names FROM walk_notes WHERE wt = ?1",
            [wt],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .db()?;
    let Some((missing_roots, unreadable_dirs, skipped_names)) = row else {
        return Ok(None);
    };
    let list = |text: &str| serde_json::from_str::<Vec<String>>(text).ok();
    Ok(
        match (
            list(&missing_roots),
            list(&unreadable_dirs),
            usize::try_from(skipped_names).ok(),
        ) {
            (Some(missing_roots), Some(unreadable_dirs), Some(skipped_names)) => Some(WalkNotes {
                missing_roots,
                skipped_names,
                unreadable_dirs,
            }),
            _ => None,
        },
    )
}

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
                    snippet(nodes_fts, -1, char(2), char(3), char(4), 64)
             {MATCHES}{filter}{archive}
             ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0), f.path, n.ord
             LIMIT ?{limit_param}"
        );
        let mut statement = tx.prepare(&sql).db()?;
        let hits = statement
            .query_map(params_from_iter(values), |row| {
                let snippet_parts =
                    Snippet::from_marks(&row.get::<_, Option<String>>(7)?.unwrap_or_default());
                Ok(SearchHit {
                    path: row.get(0)?,
                    ord: ord_of(row.get(1)?),
                    line: ord_of(row.get(2)?),
                    tier3: row.get::<_, i64>(3)? != 0,
                    id: row.get(4)?,
                    kind: row.get(5)?,
                    title: row.get(6)?,
                    snippet: snippet_parts.marked(),
                    snippet_parts,
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
    with_worktree(index, |tx, wt| Ok(decode_files(tx, wt, None)?.0))
}

/// One deferred read transaction over a handle's worktree
/// (`docs/features/store-reads.md` "Data", T3): every row read through it
/// comes from one snapshot, even while another connection writes. It ends
/// when dropped; the handle cannot write until then.
pub struct IndexSnapshot<'i> {
    pub(crate) tx: Transaction<'i>,
    pub(crate) wt: i64,
}

/// Stored files decoded as the check's input, with their stored hashes.
#[derive(Debug, Clone, Default)]
pub struct Decoded {
    /// In path order, `bytes` empty, no walk `problems`.
    pub input: CheckInput,
    /// Path → `files.blake3`: lower-case hex; `None` for a read error or a
    /// parser panic.
    pub blake3: BTreeMap<String, Option<String>>,
}

impl SqliteIndex {
    /// One deferred read transaction over the handle's worktree (T3);
    /// [`StoreError::NotIndexed`] as every read of the worktree
    /// ([`with_worktree`]).
    pub fn snapshot(&self) -> Result<IndexSnapshot<'_>, StoreError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        if !schema::format_is_current(&tx)? {
            return Err(StoreError::NotIndexed);
        }
        let wt = worktree_id(&tx, self)?.ok_or(StoreError::NotIndexed)?;
        Ok(IndexSnapshot { tx, wt })
    }
}

impl IndexSnapshot<'_> {
    /// The stored files among `paths` (the others left out), in path order,
    /// each by its `(wt, path)` key.
    pub fn decode(&self, paths: &[&str]) -> Result<Decoded, StoreError> {
        Ok(decode_some(&self.tx, self.wt, paths)?.decoded())
    }

    /// Every stored file, each row table read once: [`SpecIndex::indexed_input`]'s
    /// input with its hashes.
    ///
    /// [`SpecIndex::indexed_input`]: crate::SpecIndex::indexed_input
    pub fn decode_all(&self) -> Result<Decoded, StoreError> {
        Ok(decode_all(&self.tx, self.wt)?.decoded())
    }
}

/// What a decode gives: the input, each file's `file_id` and hash.
struct DecodedRows {
    input: CheckInput,
    ids: BTreeMap<String, i64>,
    blake3: BTreeMap<String, Option<String>>,
}

impl DecodedRows {
    fn decoded(self) -> Decoded {
        Decoded {
            input: self.input,
            blake3: self.blake3,
        }
    }
}

/// The stored files of the worktree `wt` as the check's input, in path
/// order, and each one's `file_id`: every file (`None`), else the stored
/// ones among `only`. [`indexed_input`]'s decode, shared by the edge
/// passes inside their write transaction
/// (`docs/features/edge-table.md` "Data").
pub(crate) fn decode_files(
    tx: &Transaction<'_>,
    wt: i64,
    only: Option<&[&str]>,
) -> Result<(CheckInput, BTreeMap<String, i64>), StoreError> {
    let rows = match only {
        None => decode_all(tx, wt),
        Some(paths) => decode_some(tx, wt, paths),
    }?;
    Ok((rows.input, rows.ids))
}

/// Every file of the worktree, each row table read once.
fn decode_all(tx: &Transaction<'_>, wt: i64) -> Result<DecodedRows, StoreError> {
    // file_id → its rows' JSON in `ord` order, for one of the row tables.
    let rows_of = |table: &str, column: &str| -> Result<BTreeMap<i64, Vec<String>>, StoreError> {
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
            "SELECT file_id, path, size, read_error, shell, blake3 FROM files
                 WHERE wt = ?1 ORDER BY path",
        )
        .db()?;
    let files = statement
        .query_map([wt], file_row)
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    let mut input = CheckInput::default();
    let mut ids = BTreeMap::new();
    let mut blake3 = BTreeMap::new();
    for file in files {
        let parts = FileParts {
            nodes: nodes.remove(&file.file_id).unwrap_or_default(),
            links: links.remove(&file.file_id).unwrap_or_default(),
            anchors: anchors.remove(&file.file_id).unwrap_or_default(),
            diagnostics: diagnostics.remove(&file.file_id).unwrap_or_default(),
        };
        ids.insert(file.path.clone(), file.file_id);
        blake3.insert(file.path.clone(), file.blake3.clone());
        input.files.push(file.check_file(&parts)?);
    }
    Ok(DecodedRows { input, ids, blake3 })
}

/// The keyed decode's file row: the worktree `?1`, the path `?2`, by the
/// `(wt, path)` key; its hash included ([`IndexSnapshot::decode`]).
pub(crate) const FILE_BY_PATH: &str =
    "SELECT file_id, path, size, read_error, shell, blake3 FROM files
             WHERE wt = ?1 AND path = ?2";

/// The stored files among `paths`, in path order, each by its
/// `(wt, path)` key and its rows by `(file_id, ord)`.
fn decode_some(tx: &Transaction<'_>, wt: i64, paths: &[&str]) -> Result<DecodedRows, StoreError> {
    let mut sorted: Vec<&str> = paths.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut file = tx.prepare(FILE_BY_PATH).db()?;
    let statement_of = |table: &str, column: &str| {
        tx.prepare(&format!(
            "SELECT {column} FROM {table} WHERE file_id = ?1 ORDER BY ord"
        ))
        .db()
    };
    let mut nodes = statement_of("nodes", "node")?;
    let mut links = statement_of("links", "link")?;
    let mut anchors = statement_of("anchors", "anchor")?;
    let mut diagnostics = statement_of("diagnostics", "diagnostic")?;
    let mut input = CheckInput::default();
    let mut ids = BTreeMap::new();
    let mut blake3 = BTreeMap::new();
    for path in sorted {
        let Some(found) = file.query_row((wt, path), file_row).optional().db()? else {
            continue;
        };
        let parts = FileParts {
            nodes: texts_of(&mut nodes, found.file_id)?,
            links: texts_of(&mut links, found.file_id)?,
            anchors: texts_of(&mut anchors, found.file_id)?,
            diagnostics: texts_of(&mut diagnostics, found.file_id)?,
        };
        ids.insert(found.path.clone(), found.file_id);
        blake3.insert(found.path.clone(), found.blake3.clone());
        input.files.push(found.check_file(&parts)?);
    }
    Ok(DecodedRows { input, ids, blake3 })
}

/// The one text column `statement` gives for `file_id`, in its order.
fn texts_of(statement: &mut Statement<'_>, file_id: i64) -> Result<Vec<String>, StoreError> {
    statement
        .query_map([file_id], |row| row.get::<_, String>(0))
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()
}

/// A `files` row as the decode reads it.
struct FileRow {
    file_id: i64,
    path: String,
    size: i64,
    read_error: Option<String>,
    shell: Option<String>,
    blake3: Option<String>,
}

/// One file's row JSON, each table in `ord` order.
struct FileParts {
    nodes: Vec<String>,
    links: Vec<String>,
    anchors: Vec<String>,
    diagnostics: Vec<String>,
}

fn file_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRow> {
    Ok(FileRow {
        file_id: row.get(0)?,
        path: row.get(1)?,
        size: row.get(2)?,
        read_error: row.get(3)?,
        shell: row.get(4)?,
        blake3: row.get(5)?,
    })
}

impl FileRow {
    /// The check's file: the parse rebuilt from `parts` (none without a
    /// shell), no bytes.
    fn check_file(self, parts: &FileParts) -> Result<CheckFile, StoreError> {
        let parsed = match &self.shell {
            None => None,
            Some(shell) => Some(rebuild_parsed(
                &self.path,
                shell,
                &parts.nodes,
                &parts.links,
                &parts.anchors,
                &parts.diagnostics,
            )?),
        };
        Ok(CheckFile {
            path: self.path,
            size: u64::try_from(self.size).unwrap_or(0),
            parsed,
            read_error: self.read_error,
            bytes: Vec::new(),
        })
    }
}

fn ord_of(ord: i64) -> usize {
    usize::try_from(ord).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod collision_input_tests {
    //! docs/features/collision-pass.md "Acceptance criteria" CP1, CP2, the
    //! store's unit: [`collision_input`] over a small corpus this module
    //! writes to a scratch directory (neutral words, no project's prefix or
    //! kind). The universe is the live documents and ID sections with a
    //! kind, by `(path, ord)`: an ID-less section, a kindless document, a
    //! Tier 3 and a generated file are not in it; a node without a
    //! `norm_hash` is, counted in `skipped`. The targets are a node's
    //! resolved edges of any type but `mentions` and its document's
    //! `parent:`, each named by its end's ID, else its path. M: `mentions`
    //! read; the Tier 3 or generated filter dropped.

    use std::path::PathBuf;

    use specengine_core::{IdSchemeToml, Paths};
    use specengine_model::IdScheme;

    use super::collision_input;
    use crate::IndexWriter;
    use crate::index::SqliteIndex;
    use crate::source::WorkingTree;

    /// One name-shaped prefix of a neutral kind.
    const CONFIG: &str = "[project]\nname = \"knots\"\nslug = \"knots\"\n\n[ids]\n\
                          KNT = { kind = \"knot\", shape = \"name\" }\n";

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The corpus under `docs/spec/`, indexed.
    fn indexed(name: &str) -> (Scratch, SqliteIndex) {
        let dir = std::env::temp_dir().join(format!(
            "specengine-store-collisions-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("wt");
        let spec = root.join("docs/spec");
        std::fs::create_dir_all(&spec).expect("scratch");
        std::fs::write(root.join("specengine.toml"), CONFIG).expect("config");
        let files = [
            (
                "base.md",
                "---\nid: KNT-BASE\n---\n\n# Base\n\nThe base knot.\n",
            ),
            (
                "bend.md",
                "---\nid: KNT-BEND\nlinks:\n  uses: [KNT-BASE]\n---\n\n# Bend\n\n\
                 Tied after KNT-HITCH.\n",
            ),
            (
                "child.md",
                "---\nkind: knot\nparent: KNT-BASE\n---\n\n# Child\n\nA child knot.\n",
            ),
            (
                "hitch.md",
                "---\nid: KNT-HITCH\n---\n\n# Hitch\n\nA hitch.\n\n## Round turn {#KNT-ROUND}\n\n\
                 One turn.\n\n## Notes\n\nNo ID here.\n",
            ),
            (
                "loose.md",
                "---\ntitle: Loose\n---\n\n# Loose\n\nNo kind.\n",
            ),
            (
                "old.md",
                "---\nid: KNT-OLD\nclass: decision\nstatus: superseded\nlinks:\n  uses: \
                 [KNT-BASE]\n---\n\n# Old\n\nGone.\n",
            ),
            (
                "made.md",
                "---\nid: KNT-MADE\nclass: generated\nlinks:\n  uses: [KNT-BASE]\n---\n\n\
                 # Made\n\nGenerated.\n",
            ),
            (
                "torn.md",
                "---\nid: KNT-TORN\nrev: \"3\"\n---\n\n# Torn\n\nA torn knot.\n",
            ),
        ];
        for (file, text) in files {
            std::fs::write(spec.join(file), text).expect("a document");
        }
        let root = root.canonicalize().expect("canonical");
        let scheme = IdScheme::from_toml(CONFIG).expect("the scheme");
        let paths = Paths::from_toml(CONFIG).expect("the paths");
        let mut index = SqliteIndex::open(dir.join("index.db"), "knots", &root).expect("open");
        index
            .update(&WorkingTree::new(&root, &paths).expect("a tree"), &scheme)
            .expect("update");
        (Scratch(dir), index)
    }

    #[test]
    fn the_universe_and_its_targets() {
        let (_scratch, index) = indexed("input");
        let rows = collision_input(&index).expect("the input");
        let nodes: Vec<(&str, usize, Option<&str>, &str, bool)> = rows
            .nodes
            .iter()
            .map(|node| {
                (
                    node.path.as_str(),
                    node.ord,
                    node.id.as_deref(),
                    node.kind.as_str(),
                    node.norm_hash.is_some(),
                )
            })
            .collect();
        assert_eq!(
            nodes,
            [
                ("docs/spec/base.md", 0, Some("KNT-BASE"), "knot", true),
                ("docs/spec/bend.md", 0, Some("KNT-BEND"), "knot", true),
                ("docs/spec/child.md", 0, None, "knot", true),
                ("docs/spec/hitch.md", 0, Some("KNT-HITCH"), "knot", true),
                ("docs/spec/hitch.md", 1, Some("KNT-ROUND"), "knot", true),
                ("docs/spec/torn.md", 0, Some("KNT-TORN"), "knot", false),
            ]
        );
        assert_eq!(rows.skipped, 1);
        assert_eq!(rows.nodes[1].title.as_deref(), Some("Bend"));
        let targets: Vec<(&str, usize, &str)> = rows
            .targets
            .iter()
            .map(|target| {
                (
                    target.holder_path.as_str(),
                    target.holder_ord,
                    target.end_name.as_str(),
                )
            })
            .collect();
        assert_eq!(
            targets,
            [
                ("docs/spec/bend.md", 0, "KNT-BASE"),
                ("docs/spec/child.md", 0, "KNT-BASE"),
            ],
            "uses and parent; no mention, no Tier 3 or generated holder"
        );
        assert_eq!(rows.targets[0].end_path, "docs/spec/base.md");
        assert_eq!(rows.targets[0].end_ord, 0);
    }
}
