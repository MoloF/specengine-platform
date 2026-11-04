//! The edge tables (`docs/features/edge-table.md` "Data"): core's
//! resolution of every file's links, stored per citing file and kept
//! current inside the update's one `Immediate` transaction, after the file
//! rows. Core decides every end, state, reason and key
//! (`specengine_core::check::resolve_files`, `given_keys`); this module
//! projects them onto rows and matches key text, nothing more.
//!
//! - **Full pass**: every edge recomputed from every stored file, when the
//!   stamp was recreated, the worktree is new, the `[ids]` or `[paths]`
//!   fingerprint moved, or on a rebuild.
//! - **Incremental pass**: the files whose recorded reads (`edge_deps`)
//!   meet a changed file's old or new names (`edge_names`), re-resolved
//!   over a subset closed by a fixpoint: a resolution equals the whole
//!   worktree's when every key it read has every holder in its input
//!   (the spec's R4).
//!
//! No statement orders by rowid: `ord` and `n` are core's orders and
//! decoded subsets are sorted by path (the spec's R8).

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Statement, Transaction};
use specengine_core::Paths;
use specengine_core::check::{
    CheckInput, Endpoint, FileEdges, LinkState, Parent, ResolvedFiles, resolve_files,
};
use specengine_model::{EdgeLabel, IdScheme, LinkOrigin};

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::{IndexSnapshot, decode_files, with_worktree};
use crate::rows::{from_json, to_json};

/// The columns of an edge row a lookup gives, `f` its citing file and `e`
/// the row: the path, then every column but `file_id`. A literal, for
/// `concat!`. A lookup by holder gives one row per end (`n`) of each edge,
/// where core holds the edge once: a read (RM-0079) groups rows by
/// `(path, ord)`.
macro_rules! edge_columns {
    () => {
        "f.path AS path, e.ord AS ord, e.n AS n, e.holder, e.type, e.origin,
         e.names_source, e.state, e.reason, e.label, e.at, e.end_path, e.end_ord"
    };
}

/// The edges a node holds: the worktree `?1`, the node's file `?2` and
/// position `?3`; the file by its `(wt, path)` key, its rows by
/// `edges_by_holder` (named: the primary key would serve the order and
/// read the whole file). Core's `held`, one row per end: grouped by
/// `(path, ord)`.
pub(crate) const HELD: &str = concat!(
    "SELECT ",
    edge_columns!(),
    " FROM files f CROSS JOIN edges e INDEXED BY edges_by_holder ON e.file_id = f.file_id
     WHERE f.wt = ?1 AND f.path = ?2 AND e.holder = ?3
     ORDER BY path, ord, n"
);

/// The edges a node (`?1` worktree, `?2` path, `?3` position) is the
/// resolved source of: held there and not naming their source, then those
/// whose written end, naming their source, lands there. Core's `from`
/// (`Edge::written_end`); the held part gives one row per end, grouped by
/// `(path, ord)`.
pub(crate) const OUTGOING: &str = concat!(
    "SELECT ",
    edge_columns!(),
    " FROM files f CROSS JOIN edges e INDEXED BY edges_by_holder ON e.file_id = f.file_id
     WHERE f.wt = ?1 AND f.path = ?2 AND e.holder = ?3 AND e.names_source = 0
     UNION ALL SELECT ",
    edge_columns!(),
    " FROM edges e CROSS JOIN files f ON f.file_id = e.file_id
     WHERE e.end_path = ?2 AND e.end_ord = ?3 AND e.names_source = 1 AND f.wt = ?1
     ORDER BY path, ord, n"
);

/// The edges a node (`?1` worktree, `?2` path, `?3` position) is the
/// resolved target of: those whose written end, naming their target, lands
/// there, then those held there naming their source. Core's `into`
/// (`index_edges`); the held part gives one row per end, grouped by
/// `(path, ord)`.
pub(crate) const INCOMING: &str = concat!(
    "SELECT ",
    edge_columns!(),
    " FROM edges e CROSS JOIN files f ON f.file_id = e.file_id
     WHERE e.end_path = ?2 AND e.end_ord = ?3 AND e.names_source = 0 AND f.wt = ?1
     UNION ALL SELECT ",
    edge_columns!(),
    " FROM files f CROSS JOIN edges e INDEXED BY edges_by_holder ON e.file_id = f.file_id
     WHERE f.wt = ?1 AND f.path = ?2 AND e.holder = ?3 AND e.names_source = 1
     ORDER BY path, ord, n"
);

/// The documents whose resolved `parent:` lands on a node (`?1` worktree,
/// `?2` path, `?3` position), by `edge_parents_by_end`.
// No read uses it yet: the `store-reads` subset reads none of it, and a
// subset `tree` or `flow` would. It is kept with its index, which a later
// format would otherwise have to bring back; the lookup tests plan it.
#[allow(dead_code)]
pub(crate) const PARENTED: &str =
    "SELECT f.path AS path, p.n AS n, p.state, p.reason, p.end_path, p.end_ord
     FROM edge_parents p CROSS JOIN files f ON f.file_id = p.file_id
     WHERE p.end_path = ?2 AND p.end_ord = ?3 AND f.wt = ?1
     ORDER BY path, n";

/// The files of the worktree `?1` whose resolution read the key `?2`
/// (`edge_deps_by_key`); a list of keys is run key by key.
pub(crate) const DEPENDENTS: &str = "SELECT f.path AS path
     FROM edge_deps d CROSS JOIN files f ON f.file_id = d.file_id
     WHERE d.key = ?2 AND f.wt = ?1
     ORDER BY path";

/// The files of the worktree `?1` that give the key `?2`
/// (`edge_names_by_key`); a list of keys is run key by key.
pub(crate) const HOLDERS: &str = "SELECT f.path AS path
     FROM edge_names k CROSS JOIN files f ON f.file_id = k.file_id
     WHERE k.key = ?2 AND f.wt = ?1
     ORDER BY path";

/// One edge whole, every row (end) of it: the worktree `?1`, the citing
/// file `?2` by its `(wt, path)` key, the edge's position `?3` by the
/// primary key (`docs/features/store-reads.md` "Data"). A row found by its
/// end holds that end alone; this completes it.
pub(crate) const EDGE: &str = concat!(
    "SELECT ",
    edge_columns!(),
    " FROM files f CROSS JOIN edges e ON e.file_id = f.file_id
     WHERE f.wt = ?1 AND f.path = ?2 AND e.ord = ?3 ORDER BY n"
);

/// A document's stored `parent:`: the worktree `?1`, its file `?2` by its
/// `(wt, path)` key, its rows by the primary key, in `n` order.
pub(crate) const PARENT: &str = "SELECT p.n, p.state, p.reason, p.end_path, p.end_ord
     FROM files f CROSS JOIN edge_parents p ON p.file_id = f.file_id
     WHERE f.wt = ?1 AND f.path = ?2 ORDER BY p.n";

/// The edge tables of the handle's worktree ([`SqliteIndex::stored_edges`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredEdges {
    /// By `(path, ord)`.
    pub edges: Vec<StoredEdge>,
    /// By path.
    pub parents: Vec<StoredParent>,
}

/// One stored edge: core's [`EdgeRow`](specengine_core::check::EdgeRow),
/// an `Edge` without what the citing file's bytes give (its written form,
/// its line); core owns the type, so a read builds its `GraphRows` from
/// the rows as read (`docs/features/store-reads.md` "Data").
pub use specengine_core::check::EdgeRow as StoredEdge;

/// One document's stored `parent:`, before cycle breaking: core's
/// [`ParentRow`](specengine_core::check::ParentRow).
pub use specengine_core::check::ParentRow as StoredParent;

impl SqliteIndex {
    /// The worktree's edge tables in one snapshot
    /// (`docs/features/edge-table.md` "Data"): for the tests and the reads
    /// of RM-0079; [`StoreError::NotIndexed`] as every read of the
    /// worktree.
    pub fn stored_edges(&self) -> Result<StoredEdges, StoreError> {
        with_worktree(self, |tx, wt| {
            Ok(StoredEdges {
                edges: read_edges(tx, wt)?,
                parents: read_parents(tx, wt)?,
            })
        })
    }
}

/// The keyed lookups of a subset read (`docs/features/store-reads.md`
/// "Data"), each a `SEARCH`, every row from the snapshot.
impl IndexSnapshot<'_> {
    /// The files giving `key` (`EdgeKey::text()`), by path ([`HOLDERS`]).
    pub fn holders(&self, key: &str) -> Result<Vec<String>, StoreError> {
        let mut statement = self.tx.prepare(HOLDERS).db()?;
        paths_by_key(&mut statement, self.wt, key)
    }

    /// The edges the node `(path, ord)` holds, each whole, by (path, `ord`),
    /// ends in `n` order ([`HELD`]).
    pub fn held(&self, path: &str, ord: usize) -> Result<Vec<StoredEdge>, StoreError> {
        let mut statement = self.tx.prepare(HELD).db()?;
        let mut rows = statement.query((self.wt, path, count(ord))).db()?;
        edges_of(&mut rows)
    }

    /// The edges with a row whose end is the node `(path, ord)`, either
    /// `names_source`: the by-end halves of [`OUTGOING`] and [`INCOMING`],
    /// each edge completed with all its rows by [`EDGE`]; by (path, `ord`).
    pub fn ending(&self, path: &str, ord: usize) -> Result<Vec<StoredEdge>, StoreError> {
        let at = count(ord);
        let mut found: BTreeSet<(String, i64)> = BTreeSet::new();
        for sql in [OUTGOING, INCOMING] {
            let mut statement = self.tx.prepare(sql).db()?;
            let mut rows = statement.query((self.wt, path, at)).db()?;
            while let Some(row) = rows.next().db()? {
                let end_path: Option<String> = row.get(11).db()?;
                let end_ord: Option<i64> = row.get(12).db()?;
                if end_path.as_deref() == Some(path) && end_ord == Some(at) {
                    found.insert((row.get(0).db()?, row.get(1).db()?));
                }
            }
        }
        let mut statement = self.tx.prepare(EDGE).db()?;
        let mut edges = Vec::with_capacity(found.len());
        for (citing, edge) in found {
            let mut rows = statement.query((self.wt, citing.as_str(), edge)).db()?;
            edges.extend(edges_of(&mut rows)?);
        }
        Ok(edges)
    }

    /// The stored `parent:` of the document at `path` ([`PARENT`]); `None`
    /// without a row.
    pub fn parent(&self, path: &str) -> Result<Option<StoredParent>, StoreError> {
        let mut statement = self.tx.prepare(PARENT).db()?;
        let mut rows = statement.query((self.wt, path)).db()?;
        let mut parent: Option<StoredParent> = None;
        while let Some(row) = rows.next().db()? {
            let end = end_of(row.get(3).db()?, row.get(4).db()?);
            match &mut parent {
                Some(parent) => parent.ends.extend(end),
                None => {
                    parent = Some(StoredParent {
                        path: path.to_owned(),
                        state: state_of(&row.get::<_, String>(1).db()?)?,
                        reason: row.get(2).db()?,
                        ends: end.into_iter().collect(),
                    });
                }
            }
        }
        Ok(parent)
    }
}

/// What one transaction's file rows changed: the incremental pass's
/// input, gathered as the rows are deleted and written.
#[derive(Debug, Default)]
pub(crate) struct Changes {
    /// Paths whose rows were written, fresh or in place of older ones.
    pub written: BTreeSet<String>,
    /// Paths whose rows were deleted and not written again.
    pub deleted: BTreeSet<String>,
    /// Every changed file's names: the `edge_names` of the rows deleted,
    /// read before their deletion, and those of the rows written.
    pub names: BTreeSet<String>,
}

/// Every edge recomputed: the worktree's `edges`, `edge_parents` and
/// `edge_deps` replaced from every stored file. The number of files.
pub(crate) fn full_pass(
    tx: &Transaction<'_>,
    wt: i64,
    scheme: &IdScheme,
    paths: &Paths,
) -> Result<usize, StoreError> {
    for table in ["edges", "edge_parents", "edge_deps"] {
        tx.execute(
            &format!(
                "DELETE FROM {table} WHERE file_id IN (SELECT file_id FROM files WHERE wt = ?1)"
            ),
            [wt],
        )
        .db()?;
    }
    let (input, ids) = decode_files(tx, wt, None)?;
    let resolved = resolve_files(&input, scheme, paths, None);
    let mut rows = EdgeRows::new(tx)?;
    for file in &resolved.files {
        rows.insert(&resolved, file, &ids)?;
    }
    Ok(resolved.files.len())
}

/// The files a change can reach, re-resolved
/// (`docs/features/edge-table.md` "Data", the incremental pass): A is the
/// written files and those whose reads meet a changed name; each round
/// resolves A over the subset S, then adds the holders of every key read,
/// until S holds every holder of every key A read. The size of A; 0 and no
/// edge row read or written when nothing changed (the spec's R5).
pub(crate) fn incremental_pass(
    tx: &Transaction<'_>,
    wt: i64,
    scheme: &IdScheme,
    paths: &Paths,
    changes: &Changes,
) -> Result<usize, StoreError> {
    if changes.written.is_empty() && changes.deleted.is_empty() {
        return Ok(0);
    }
    let mut dependents = tx.prepare(DEPENDENTS).db()?;
    let mut holders = tx.prepare(HOLDERS).db()?;
    // A: the deleted files' rows are gone with their reads, and the
    // written ones have no reads yet.
    let mut a: BTreeSet<String> = changes.written.clone();
    for key in &changes.names {
        a.extend(paths_by_key(&mut dependents, wt, key)?);
    }
    for path in &changes.deleted {
        a.remove(path);
    }

    // A's reads, then its rows go.
    let mut file_of = tx
        .prepare("SELECT file_id FROM files WHERE wt = ?1 AND path = ?2")
        .db()?;
    let mut reads_of = tx
        .prepare("SELECT key FROM edge_deps WHERE file_id = ?1 ORDER BY key")
        .db()?;
    let mut deletes = ["edges", "edge_parents", "edge_deps"]
        .into_iter()
        .map(|table| {
            tx.prepare(&format!("DELETE FROM {table} WHERE file_id = ?1"))
                .db()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut queried: BTreeSet<String> = BTreeSet::new();
    for path in &a {
        let file_id: i64 = file_of
            .query_row((wt, path.as_str()), |row| row.get(0))
            .db()?;
        let keys = reads_of
            .query_map([file_id], |row| row.get::<_, String>(0))
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        queried.extend(keys);
        for delete in &mut deletes {
            delete.execute([file_id]).db()?;
        }
    }

    // S0: A and the holders of what it read.
    let mut members: BTreeSet<String> = a.clone();
    for key in &queried {
        members.extend(paths_by_key(&mut holders, wt, key)?);
    }
    let only: BTreeSet<&str> = a.iter().map(String::as_str).collect();
    let mut input = CheckInput::default();
    let mut ids: BTreeMap<String, i64> = BTreeMap::new();
    loop {
        let fresh: Vec<&str> = members
            .iter()
            .map(String::as_str)
            .filter(|path| !ids.contains_key(*path))
            .collect();
        let (decoded, decoded_ids) = decode_files(tx, wt, Some(&fresh))?;
        input.files.extend(decoded.files);
        ids.extend(decoded_ids);
        let resolved = resolve_files(&input, scheme, paths, Some(&only));
        let mut grown = false;
        for file in &resolved.files {
            for key in &file.reads {
                let key = key.text();
                if queried.contains(&key) {
                    continue;
                }
                for holder in paths_by_key(&mut holders, wt, &key)? {
                    grown |= members.insert(holder);
                }
                queried.insert(key);
            }
        }
        if !grown {
            let mut rows = EdgeRows::new(tx)?;
            for file in &resolved.files {
                rows.insert(&resolved, file, &ids)?;
            }
            return Ok(a.len());
        }
    }
}

/// The paths [`DEPENDENTS`] or [`HOLDERS`], prepared, gives for one key.
fn paths_by_key(
    statement: &mut Statement<'_>,
    wt: i64,
    key: &str,
) -> Result<Vec<String>, StoreError> {
    statement
        .query_map((wt, key), |row| row.get::<_, String>(0))
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()
}

/// The insert statements of one pass.
struct EdgeRows<'t> {
    edge: Statement<'t>,
    parent: Statement<'t>,
    dep: Statement<'t>,
}

impl<'t> EdgeRows<'t> {
    fn new(tx: &'t Transaction<'_>) -> Result<Self, StoreError> {
        Ok(Self {
            edge: tx
                .prepare(
                    "INSERT INTO edges
                       (file_id, ord, n, holder, type, origin, names_source, state, reason,
                        label, at, end_path, end_ord)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                )
                .db()?,
            parent: tx
                .prepare(
                    "INSERT INTO edge_parents (file_id, n, state, reason, end_path, end_ord)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .db()?,
            dep: tx
                .prepare("INSERT INTO edge_deps (file_id, key) VALUES (?1, ?2)")
                .db()?,
        })
    }

    /// One file's edges, parent and reads; its `file_id` by path from
    /// `ids`.
    fn insert(
        &mut self,
        resolved: &ResolvedFiles<'_>,
        file: &FileEdges,
        ids: &BTreeMap<String, i64>,
    ) -> Result<(), StoreError> {
        let path = resolved.paths.get(file.file).copied().unwrap_or_default();
        let Some(&file_id) = ids.get(path) else {
            return Err(StoreError::Sqlite(format!(
                "the resolved file `{path}` has no row"
            )));
        };
        let end = |at: &specengine_core::check::NodeAt| {
            (
                resolved.paths.get(at.file).copied().unwrap_or_default(),
                count(at.ord),
            )
        };
        for (ord, edge) in file.edges.iter().enumerate() {
            let label = edge
                .edge
                .as_ref()
                .map(|label| to_json(label, "edge label"))
                .transpose()?;
            let ends: Vec<(Option<&str>, Option<i64>)> = match edge.written_end() {
                Endpoint::Nodes(nodes) => nodes
                    .iter()
                    .map(|node| {
                        let (path, ord) = end(node);
                        (Some(path), Some(ord))
                    })
                    .collect(),
                _ => vec![(None, None)],
            };
            for (n, (end_path, end_ord)) in ends.into_iter().enumerate() {
                self.edge
                    .execute((
                        file_id,
                        count(ord),
                        count(n),
                        count(edge.holder.ord),
                        edge.link_type.as_str(),
                        origin_text(edge.origin),
                        i64::from(edge.names_source),
                        edge.state().as_str(),
                        edge.reason(),
                        label.as_deref(),
                        count(edge.offset),
                        end_path,
                        end_ord,
                    ))
                    .db()?;
            }
        }
        match &file.parent {
            Some(Parent::Node { node, others }) => {
                for (n, node) in std::iter::once(node).chain(others).enumerate() {
                    let (end_path, end_ord) = end(node);
                    self.parent
                        .execute((
                            file_id,
                            count(n),
                            LinkState::Resolved.as_str(),
                            None::<&str>,
                            Some(end_path),
                            Some(end_ord),
                        ))
                        .db()?;
                }
            }
            Some(Parent::Dangling { reason, .. }) => {
                self.parent
                    .execute((
                        file_id,
                        0_i64,
                        LinkState::Dangling.as_str(),
                        Some(reason.as_str()),
                        None::<&str>,
                        None::<i64>,
                    ))
                    .db()?;
            }
            Some(Parent::Skipped { .. }) => {
                self.parent
                    .execute((
                        file_id,
                        0_i64,
                        LinkState::Skipped.as_str(),
                        None::<&str>,
                        None::<&str>,
                        None::<i64>,
                    ))
                    .db()?;
            }
            // No `parent:` (or a failed front-matter), no parse, and a
            // cycle (broken by the reads, never here): no row.
            Some(Parent::None | Parent::Cycle { .. }) | None => {}
        }
        for key in &file.reads {
            self.dep.execute((file_id, key.text())).db()?;
        }
        Ok(())
    }
}

/// The worktree's edges by `(path, ord)`, each with its ends.
fn read_edges(tx: &Transaction<'_>, wt: i64) -> Result<Vec<StoredEdge>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path, e.ord, e.n, e.holder, e.type, e.origin, e.names_source, e.state,
                    e.reason, e.label, e.at, e.end_path, e.end_ord
             FROM files f CROSS JOIN edges e ON e.file_id = f.file_id
             WHERE f.wt = ?1 ORDER BY f.path, e.ord, e.n",
        )
        .db()?;
    let mut rows = statement.query([wt]).db()?;
    edges_of(&mut rows)
}

/// The end a row's `end_path` and `end_ord` give; none unless resolved.
fn end_of(end_path: Option<String>, end_ord: Option<i64>) -> Option<(String, usize)> {
    match (end_path, end_ord) {
        (Some(end_path), Some(end_ord)) => Some((end_path, index_of(end_ord))),
        _ => None,
    }
}

/// The edges of rows in `edge_columns!`' order, sorted by (path, `ord`,
/// `n`): consecutive rows of one `(path, ord)` are one edge, its ends in
/// row order.
fn edges_of(rows: &mut rusqlite::Rows<'_>) -> Result<Vec<StoredEdge>, StoreError> {
    let mut edges: Vec<StoredEdge> = Vec::new();
    while let Some(row) = rows.next().db()? {
        let path: String = row.get(0).db()?;
        let ord = index_of(row.get(1).db()?);
        let end = end_of(row.get(11).db()?, row.get(12).db()?);
        if let Some(last) = edges.last_mut()
            && last.path == path
            && last.ord == ord
        {
            last.ends.extend(end);
            continue;
        }
        let label: Option<String> = row.get(9).db()?;
        edges.push(StoredEdge {
            path,
            ord,
            holder: index_of(row.get(3).db()?),
            link_type: row.get(4).db()?,
            origin: origin_of(&row.get::<_, String>(5).db()?)?,
            names_source: row.get::<_, i64>(6).db()? != 0,
            state: state_of(&row.get::<_, String>(7).db()?)?,
            reason: row.get(8).db()?,
            label: label
                .map(|label| from_json::<EdgeLabel>(&label, "edge label"))
                .transpose()?,
            at: index_of(row.get(10).db()?),
            ends: end.into_iter().collect(),
        });
    }
    Ok(edges)
}

/// The worktree's stored parents by path, each with its ends.
fn read_parents(tx: &Transaction<'_>, wt: i64) -> Result<Vec<StoredParent>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path, p.state, p.reason, p.end_path, p.end_ord
             FROM files f CROSS JOIN edge_parents p ON p.file_id = f.file_id
             WHERE f.wt = ?1 ORDER BY f.path, p.n",
        )
        .db()?;
    let mut rows = statement.query([wt]).db()?;
    let mut parents: Vec<StoredParent> = Vec::new();
    while let Some(row) = rows.next().db()? {
        let path: String = row.get(0).db()?;
        let end = match (
            row.get::<_, Option<String>>(3).db()?,
            row.get::<_, Option<i64>>(4).db()?,
        ) {
            (Some(end_path), Some(end_ord)) => Some((end_path, index_of(end_ord))),
            _ => None,
        };
        if let Some(last) = parents.last_mut()
            && last.path == path
        {
            last.ends.extend(end);
            continue;
        }
        parents.push(StoredParent {
            path,
            state: state_of(&row.get::<_, String>(1).db()?)?,
            reason: row.get(2).db()?,
            ends: end.into_iter().collect(),
        });
    }
    Ok(parents)
}

/// `frontmatter` or `inline`: the model's spelling.
fn origin_text(origin: LinkOrigin) -> &'static str {
    match origin {
        LinkOrigin::Frontmatter => "frontmatter",
        LinkOrigin::Inline => "inline",
    }
}

fn origin_of(text: &str) -> Result<LinkOrigin, StoreError> {
    match text {
        "frontmatter" => Ok(LinkOrigin::Frontmatter),
        "inline" => Ok(LinkOrigin::Inline),
        other => Err(StoreError::Sqlite(format!(
            "a stored edge origin `{other}` does not decode"
        ))),
    }
}

fn state_of(text: &str) -> Result<LinkState, StoreError> {
    [
        LinkState::Resolved,
        LinkState::Dangling,
        LinkState::Skipped,
        LinkState::Unchecked,
    ]
    .into_iter()
    .find(|state| state.as_str() == text)
    .ok_or_else(|| StoreError::Sqlite(format!("a stored edge state `{text}` does not decode")))
}

fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn index_of(value: i64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    //! docs/features/edge-table.md "Acceptance criteria" AC-12, the store's
    //! half: the lookups. As the symbol lookup's test (`symbols.rs`): each
    //! of the six statements is planned as a `SEARCH` with no `SCAN` of a
    //! table on an empty DB (no `ANALYZE`: SQLite plans for large tables);
    //! each one's cost at 100 000 SQL-filled `edges` rows (and as many key
    //! and parent rows) is at most three times its cost at 10 000, with a
    //! 2 ms floor, best of three runs of 20 rounds; and HELD, OUTGOING and
    //! INCOMING, run for every node of every fixture with a root
    //! `specengine.toml`, return exactly the edges whose holder, source or
    //! target in `SpecGraph::new`'s `edges()` holds that node (core's
    //! `held`, `from`, `into`). M: drop `edges_by_end` (INCOMING scans);
    //! drop `edge_deps_by_key`; OUTGOING ignoring `names_source`.

    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use specengine_core::check::{Endpoint, NodeAt, SpecGraph};
    use specengine_core::{IdSchemeToml, Paths};
    use specengine_model::IdScheme;

    use super::{DEPENDENTS, HELD, HOLDERS, INCOMING, OUTGOING, PARENTED};
    use crate::index::SqliteIndex;
    use crate::source::WorkingTree;
    use crate::{IndexWriter, SpecIndex};

    /// The six statements, by name.
    const STATEMENTS: [(&str, &str); 6] = [
        ("HELD", HELD),
        ("OUTGOING", OUTGOING),
        ("INCOMING", INCOMING),
        ("PARENTED", PARENTED),
        ("DEPENDENTS", DEPENDENTS),
        ("HOLDERS", HOLDERS),
    ];

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-store-edge-lookup-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("wt")).expect("scratch");
            std::fs::create_dir_all(path.join("data")).expect("scratch");
            Self(std::fs::canonicalize(path).expect("canonical"))
        }

        fn open(&self, name: &str) -> SqliteIndex {
            SqliteIndex::open(
                self.0.join(format!("data/{name}.db")),
                "plan",
                self.0.join("wt"),
            )
            .expect("open")
        }

        fn open_at(&self, name: &str, root: &Path) -> SqliteIndex {
            SqliteIndex::open(self.0.join(format!("data/{name}.db")), "plan", root).expect("open")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The plan's `detail` rows of `sql`, every parameter bound to a text.
    fn plan(index: &SqliteIndex, sql: &str) -> Vec<String> {
        let mut statement = index
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        let count = statement.parameter_count();
        let values: Vec<String> = (0..count).map(|n| format!("q{n}")).collect();
        statement
            .query_map(rusqlite::params_from_iter(values.iter()), |row| {
                row.get::<_, String>(3)
            })
            .expect("plan")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan rows")
    }

    #[test]
    fn every_edge_lookup_searches_an_index() {
        let scratch = Scratch::new("plan");
        let index = scratch.open("plan");
        let mut searched = Vec::new();
        for (name, sql) in STATEMENTS {
            let details = plan(&index, sql);
            assert!(
                details.iter().any(|detail| detail.starts_with("SEARCH")),
                "{name}: no search: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.starts_with("SCAN") && !detail.contains("CONSTANT ROW")),
                "{name}: a scan: {details:?}"
            );
            searched.extend(
                details
                    .into_iter()
                    .map(|detail| format!("{name}: {detail}")),
            );
        }
        for (name, index_name) in [
            ("INCOMING", "edges_by_end"),
            ("OUTGOING", "edges_by_end"),
            ("PARENTED", "edge_parents_by_end"),
            ("DEPENDENTS", "edge_deps_by_key"),
            ("HOLDERS", "edge_names_by_key"),
        ] {
            assert!(
                searched
                    .iter()
                    .any(|detail| detail.starts_with(&format!("{name}: SEARCH"))
                        && detail.contains(index_name)),
                "{name} searches no `{index_name}`: {searched:#?}"
            );
        }
    }

    /// `(name, type, not null, has a default, primary key position)` of
    /// every column of `table`.
    fn columns(index: &SqliteIndex, table: &str) -> Vec<(String, String, bool, bool, i64)> {
        let mut statement = index
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table_info");
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, Option<String>>(4)?.is_some(),
                    row.get::<_, i64>(5)?,
                ))
            })
            .expect("columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("column rows")
    }

    /// `WITH RECURSIVE n(i) … INSERT INTO <table> … SELECT … <from>`: the
    /// `given` columns as SQL expressions, every other required column a
    /// filler of its type, a sole integer key left to SQLite.
    fn fill(index: &SqliteIndex, table: &str, given: &[(&str, String)], count: usize, from: &str) {
        let columns = columns(index, table);
        assert!(!columns.is_empty(), "the schema has `{table}`");
        let keys = columns.iter().filter(|column| column.4 > 0).count();
        let mut names = Vec::new();
        let mut values = Vec::new();
        for (name, kind, not_null, has_default, key) in &columns {
            if let Some((_, value)) = given.iter().find(|(given, _)| given == name) {
                names.push(name.clone());
                values.push(value.clone());
                continue;
            }
            let rowid_alias = *key > 0 && keys == 1 && kind.eq_ignore_ascii_case("INTEGER");
            if rowid_alias || !*not_null || *has_default {
                continue;
            }
            names.push(name.clone());
            values.push(
                match kind.to_ascii_uppercase().as_str() {
                    "INTEGER" | "INT" => "0",
                    "REAL" => "0.0",
                    "BLOB" => "x''",
                    _ => "''",
                }
                .to_owned(),
            );
        }
        let sql = format!(
            "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i + 1 < {count}) \
             INSERT INTO {table} ({}) SELECT {} {from}",
            names.join(", "),
            values.join(", ")
        );
        index
            .conn
            .execute_batch(&sql)
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
    }

    /// Edge rows per citing file; a file's rows end on files `f(i % FILES)`.
    const PER_FILE: usize = 100;

    /// An index of `count` edges over `count / 100` files, each file with
    /// 100 edges held by 10 nodes (one in five naming its source), each
    /// ending on a node of another file, plus `count` `edge_deps`,
    /// `edge_names` and `edge_parents` rows, ten per key or end. Every
    /// lookup's answer has the same size at any `count`.
    fn populated(scratch: &Scratch, name: &str, count: usize) -> (SqliteIndex, i64) {
        let index = scratch.open(name);
        index
            .conn
            .execute(
                "INSERT INTO worktrees (project, root, scheme_fp, paths_fp) VALUES (?1, ?2, 'x', 'x')",
                rusqlite::params![index.project, index.root_text],
            )
            .expect("a worktree");
        let wt = index.conn.last_insert_rowid();
        let files = count / PER_FILE;
        fill(
            &index,
            "files",
            &[
                ("wt", wt.to_string()),
                ("path", "'p/f' || i || '.md'".to_owned()),
                ("size", "1".to_owned()),
                ("tier3", "0".to_owned()),
            ],
            files,
            "FROM n",
        );
        let file_of = |expression: &str| {
            format!(
                "(SELECT file_id FROM files WHERE wt = {wt} AND path = 'p/f' || ({expression}) || '.md')"
            )
        };
        fill(
            &index,
            "edges",
            &[
                ("file_id", file_of(&format!("i / {PER_FILE}"))),
                ("ord", format!("i % {PER_FILE}")),
                ("n", "0".to_owned()),
                ("holder", "i % 10".to_owned()),
                ("type", "'mentions'".to_owned()),
                ("origin", "'inline'".to_owned()),
                ("names_source", "(i % 5 = 0)".to_owned()),
                ("state", "'resolved'".to_owned()),
                ("at", "i".to_owned()),
                (
                    "end_path",
                    format!("'p/f' || ((i * 7 + 3) % {files}) || '.md'"),
                ),
                ("end_ord", "i % 10".to_owned()),
            ],
            count,
            "FROM n",
        );
        for table in ["edge_deps", "edge_names"] {
            fill(
                &index,
                table,
                &[
                    ("file_id", file_of(&format!("i % {files}"))),
                    ("key", "'name:K' || (i / 10)".to_owned()),
                ],
                count,
                "FROM n",
            );
        }
        fill(
            &index,
            "edge_parents",
            &[
                ("file_id", file_of(&format!("i % {files}"))),
                ("n", format!("i / {files}")),
                ("state", "'resolved'".to_owned()),
                (
                    "end_path",
                    format!("'p/f' || ((i * 7 + 3) % {files}) || '.md'"),
                ),
                ("end_ord", "i % 10".to_owned()),
            ],
            count,
            "FROM n",
        );
        let stored: i64 = index
            .conn
            .query_row("SELECT count(*) FROM edges", [], |row| row.get(0))
            .expect("count");
        assert_eq!(stored, i64::try_from(count).expect("i64"));
        (index, wt)
    }

    /// The rows one statement gives for `(wt, text, ord)`; DEPENDENTS and
    /// HOLDERS take `(wt, key)`.
    fn rows(statement: &mut rusqlite::Statement<'_>, wt: i64, text: &str, ord: i64) -> usize {
        let found = if statement.parameter_count() == 2 {
            statement.query(rusqlite::params![wt, text])
        } else {
            statement.query(rusqlite::params![wt, text, ord])
        };
        let mut found = found.expect("query");
        let mut count = 0;
        while found.next().expect("row").is_some() {
            count += 1;
        }
        count
    }

    /// The best of three runs of 20 rounds of `sql` over eight nodes or
    /// keys, the statement prepared once; the rows of one round.
    fn cost(index: &SqliteIndex, wt: i64, name: &str, sql: &str) -> (Duration, usize) {
        let keyed = matches!(name, "DEPENDENTS" | "HOLDERS");
        let mut statement = index.conn.prepare(sql).expect("prepare");
        let probes: Vec<(String, i64)> = (0..8_i64)
            .map(|k| {
                if keyed {
                    (format!("name:K{}", 3 + k * 11), 0)
                } else {
                    (format!("p/f{}.md", 3 + k), k % 10)
                }
            })
            .collect();
        let mut found = 0;
        let best = (0..3)
            .map(|_| {
                let started = Instant::now();
                for _ in 0..20 {
                    found = probes
                        .iter()
                        .map(|(text, ord)| rows(&mut statement, wt, text, *ord))
                        .sum();
                }
                started.elapsed()
            })
            .min()
            .expect("three runs");
        (best, found)
    }

    #[test]
    fn every_edge_lookup_costs_sublinearly() {
        let scratch = Scratch::new("cost");
        let (small, small_wt) = populated(&scratch, "10k", 10_000);
        let (large, large_wt) = populated(&scratch, "100k", 100_000);
        for (name, sql) in STATEMENTS {
            let (at_10k, small_rows) = cost(&small, small_wt, name, sql);
            let (at_100k, large_rows) = cost(&large, large_wt, name, sql);
            eprintln!(
                "{name}, best of three: 10k {at_10k:?} ({small_rows} rows), \
                 100k {at_100k:?} ({large_rows} rows)"
            );
            assert!(
                small_rows > 0 && large_rows > 0,
                "{name}: the probes find rows"
            );
            let floor = Duration::from_millis(2).max(at_10k);
            assert!(
                at_100k <= floor * 3,
                "{name}: 10x the edges cost {at_100k:?} against {at_10k:?}: a scan"
            );
        }
    }

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .canonicalize()
            .expect("fixtures")
    }

    /// `(path, ord)` of each edge a statement gives for one node, once each.
    fn edges_of(
        index: &SqliteIndex,
        sql: &str,
        wt: i64,
        path: &str,
        ord: usize,
    ) -> BTreeSet<(String, usize)> {
        let mut statement = index.conn.prepare(sql).expect("prepare");
        let ord = i64::try_from(ord).expect("i64");
        statement
            .query_map(rusqlite::params![wt, path, ord], |row| {
                Ok((row.get::<_, String>("path")?, row.get::<_, i64>("ord")?))
            })
            .expect("query")
            .map(|row| {
                let (path, ord) = row.expect("row");
                (path, usize::try_from(ord).expect("usize"))
            })
            .collect()
    }

    #[test]
    fn held_outgoing_and_incoming_are_cores_held_from_and_into_on_every_fixture() {
        let scratch = Scratch::new("fixtures");
        let mut fixtures: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
            .expect("fixtures")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.join("specengine.toml").is_file())
            .collect();
        fixtures.sort();
        assert!(fixtures.len() >= 18, "{fixtures:?}");
        let mut nodes = 0;
        let mut answered = [0usize; 3];
        for root in &fixtures {
            let name = root
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(root.join("specengine.toml")).expect("config");
            let scheme = IdScheme::from_toml(&text)
                .unwrap_or_else(|error| panic!("{name}: {}", error.at("")));
            let paths = Paths::from_toml(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
            let mut index = scratch.open_at(&name, root);
            let tree = WorkingTree::new(root, &paths).expect("working tree");
            index.update(&tree, &scheme).expect("update");
            let wt: i64 = index
                .conn
                .query_row(
                    "SELECT wt FROM worktrees WHERE project = 'plan'",
                    [],
                    |row| row.get(0),
                )
                .expect("the worktree");
            let input = index.indexed_input().expect("indexed_input");
            let graph = SpecGraph::new(&input, &scheme, &paths);
            let names = graph.paths();
            // Each edge's (path, position among its file's edges).
            let mut keys = Vec::new();
            let mut last: Option<(usize, usize)> = None;
            for edge in graph.edges() {
                let ord = match last {
                    Some((file, ord)) if file == edge.file => ord + 1,
                    _ => 0,
                };
                last = Some((edge.file, ord));
                keys.push((names[edge.file].to_owned(), ord));
            }
            for (file, &path) in names.iter().enumerate() {
                for ord in 0..graph.nodes(file).len() {
                    let node = NodeAt { file, ord };
                    nodes += 1;
                    let holds = |end: &Endpoint| matches!(end, Endpoint::Nodes(found) if found.contains(&node));
                    let want = |keep: &dyn Fn(&specengine_core::check::Edge) -> bool| -> BTreeSet<(String, usize)> {
                        graph
                            .edges()
                            .iter()
                            .zip(&keys)
                            .filter(|(edge, _)| keep(edge))
                            .map(|(_, key)| key.clone())
                            .collect()
                    };
                    for (slot, (statement, sql, expected)) in [
                        ("HELD", HELD, want(&|edge| edge.holder == node)),
                        ("OUTGOING", OUTGOING, want(&|edge| holds(&edge.source))),
                        ("INCOMING", INCOMING, want(&|edge| holds(&edge.target))),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let got = edges_of(&index, sql, wt, path, ord);
                        answered[slot] += got.len();
                        assert_eq!(got, expected, "{name}: {statement} of `{path}` #{ord}");
                    }
                }
            }
        }
        eprintln!("{nodes} nodes; rows held, outgoing, incoming: {answered:?}");
        assert!(answered.iter().all(|&count| count > 0), "{answered:?}");
    }

    // ------------------------------------------------------- store-reads (RM-0079)
    //
    // docs/features/store-reads.md AC-12, the
    // store's read half. `EDGE`, `PARENT` and the keyed decode's
    // `FILE_BY_PATH` plan as a `SEARCH` with no table `SCAN`, beside the
    // six above; on the SQL-filled rows at 10 000
    // and 100 000 edges, `EDGE`, `PARENT` and the snapshot lookups a subset
    // is built by (`holders`, `held`, `ending`, `parent`, the keyed
    // `decode` with its BLAKE3) each cost at most 3x more at 100 000, best
    // of three, 2 ms floor. M: `EDGE` without `f.path = ?2` (about 10x);
    // the keyed decode served by `decode_all`.

    /// The statements of the subset read, by name: one edge's rows, one
    /// document's `parent:` row, the keyed decode's file row.
    const READ_STATEMENTS: [(&str, &str); 3] = [
        ("EDGE statement", super::EDGE),
        ("PARENT statement", super::PARENT),
        ("FILE_BY_PATH statement", crate::read::FILE_BY_PATH),
    ];

    #[test]
    fn edge_and_parent_search_an_index() {
        let scratch = Scratch::new("plan-reads");
        let index = scratch.open("plan");
        for (name, sql) in READ_STATEMENTS {
            let details = plan(&index, sql);
            assert!(
                details.iter().any(|detail| detail.starts_with("SEARCH")),
                "{name}: no search: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.starts_with("SCAN") && !detail.contains("CONSTANT ROW")),
                "{name}: a scan: {details:?}"
            );
        }
    }

    /// The best of three runs of 20 rounds of `lookup` over eight probes;
    /// the answers of one round.
    fn cost_of(mut lookup: impl FnMut(usize) -> usize) -> (Duration, usize) {
        let mut found = 0;
        let best = (0..3)
            .map(|_| {
                let started = Instant::now();
                for _ in 0..20 {
                    found = (0..8).map(&mut lookup).sum();
                }
                started.elapsed()
            })
            .min()
            .expect("three runs");
        (best, found)
    }

    /// One snapshot lookup's cost at one size, over the probes `cost` uses.
    fn snapshot_cost(index: &SqliteIndex, name: &str) -> (Duration, usize) {
        let snapshot = index.snapshot().expect("a snapshot of the filled rows");
        let path = |k: usize| format!("p/f{}.md", 3 + k);
        match name {
            "holders" => cost_of(|k| {
                snapshot
                    .holders(&format!("name:K{}", 3 + k * 11))
                    .expect("holders")
                    .len()
            }),
            "held" => cost_of(|k| snapshot.held(&path(k), k % 10).expect("held").len()),
            "ending" => cost_of(|k| snapshot.ending(&path(k), k % 10).expect("ending").len()),
            "parent" => cost_of(|k| {
                snapshot
                    .parent(&path(k))
                    .expect("parent")
                    .map_or(0, |parent| parent.ends.len())
            }),
            "decode" => cost_of(|k| {
                let decoded = snapshot.decode(&[path(k).as_str()]).expect("decode");
                decoded.input.files.len() + decoded.blake3.len()
            }),
            other => panic!("no lookup {other}"),
        }
    }

    #[test]
    fn the_subset_lookups_cost_sublinearly() {
        let scratch = Scratch::new("cost-reads");
        let (small, small_wt) = populated(&scratch, "10k", 10_000);
        let (large, large_wt) = populated(&scratch, "100k", 100_000);
        let mut timed: Vec<(String, Duration, usize, Duration, usize)> = Vec::new();
        for (name, sql) in READ_STATEMENTS {
            let (at_10k, small_rows) = cost(&small, small_wt, name, sql);
            let (at_100k, large_rows) = cost(&large, large_wt, name, sql);
            timed.push((name.to_owned(), at_10k, small_rows, at_100k, large_rows));
        }
        for name in ["holders", "held", "ending", "parent", "decode"] {
            let (at_10k, small_rows) = snapshot_cost(&small, name);
            let (at_100k, large_rows) = snapshot_cost(&large, name);
            timed.push((name.to_owned(), at_10k, small_rows, at_100k, large_rows));
        }
        for (name, at_10k, small_rows, at_100k, large_rows) in timed {
            eprintln!(
                "{name}, best of three: 10k {at_10k:?} ({small_rows} rows), \
                 100k {at_100k:?} ({large_rows} rows)"
            );
            assert!(
                small_rows > 0 && small_rows == large_rows,
                "{name}: the probes find the same rows at both sizes"
            );
            let floor = Duration::from_millis(2).max(at_10k);
            assert!(
                at_100k <= floor * 3,
                "{name}: 10x the rows cost {at_100k:?} against {at_10k:?}: a scan"
            );
        }
    }
}
