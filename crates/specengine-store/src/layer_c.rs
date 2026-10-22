//! The layer C snapshot (`docs/features/layer-c-resolve.md` "Tables",
//! format 11): one advisory snapshot per worktree, what `spec resolve`
//! stored after its run, beside the code index of the same worktree.
//!
//! - **Rows**: `code_resolve` (the run's stamps and counts, keyed by the
//!   worktree, `code_worktrees` its parent), `code_resolve_files` (each
//!   requested file's BLAKE3 at the run and `in_crate`), `code_monikers`
//!   (each item by `(path, start)`: `end`, `kind`, `state`, the raw
//!   `moniker`), the last two children of `code_resolve`, every key
//!   cascading.
//! - **Writes**: only [`SqliteIndex::replace_layer_c`], one `Immediate`
//!   transaction replacing the worktree's rows in the three tables; nothing
//!   else writes them.
//! - **Lifetime**: the code index's refreshes never touch them (a `--full`
//!   refresh and a new code stamp update `code_worktrees` in place);
//!   [`SqliteIndex::clear_code`] drops them with their parent, an
//!   `INDEX_FORMAT` change with every table.
//! - **Strings only**: the store knows no protocol word, Cargo file or
//!   moniker syntax; it stores what the caller gives and reads it back by
//!   key.
//! - **Beside a read** ([`LayerCRead`], `SqliteIndex::code_snapshot_with_layer_c`):
//!   in the code snapshot's own transaction, the run's row, the code
//!   index's files and packages, the run's files, the rows whose raw
//!   moniker equals a given text (one search of `code_monikers_by_moniker`)
//!   and, for each file the caller picks, its rows and its symbols.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};

use crate::code::{CodePackageRow, StoredSymbol, stored_code_fp};
use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::{with_worktree, worktree_id};
use crate::schema;
use crate::symbols::files_symbols_in;

/// One worktree's layer C snapshot as `spec resolve` stores it and its
/// readers get it back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayerCSnapshot {
    /// The fingerprint of what the run saw (the caller's).
    pub tree_fp: String,
    /// The reply's stamps: its protocol, rust-analyzer release and host.
    pub protocol: u32,
    pub ra_ap: String,
    pub host: String,
    /// The workspaces' distinct known toolchains, byte-sorted.
    pub toolchains: Vec<String>,
    /// Any workspace's metadata degraded.
    pub degraded: bool,
    /// The workspaces the run loaded.
    pub workspaces: u32,
    /// Every requested file, by path.
    pub files: Vec<LayerCFile>,
    /// Every item of every file, by (path, `start`).
    pub items: Vec<LayerCItem>,
}

/// One requested file of a layer C run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCFile {
    pub path: String,
    /// The code index's BLAKE3 of the file at the run.
    pub blake3: Option<String>,
    pub in_crate: bool,
}

/// One item of a layer C run's file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCItem {
    pub path: String,
    pub start: u64,
    pub end: u64,
    pub kind: String,
    pub state: String,
    /// Raw, as the reply gave it.
    pub moniker: Option<String>,
}

/// What a freshness judgement reads of a stored snapshot: its fingerprint
/// and the stamps the fingerprint covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCStamp {
    pub tree_fp: String,
    pub protocol: u32,
    pub ra_ap: String,
    pub host: String,
}

/// One stored code file as layer C reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeTreeFile {
    pub path: String,
    pub lang: String,
    pub blake3: Option<String>,
    pub utf8: bool,
    pub read_error: Option<String>,
    /// Its package directory (`""` the root); `None` outside every
    /// package.
    pub package: Option<String>,
}

/// The code index's files and packages, read in one transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeTreeRows {
    /// By path.
    pub files: Vec<CodeTreeFile>,
    /// By directory.
    pub packages: Vec<CodePackageRow>,
}

/// The stored run's row: its stamps and what it counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCHead {
    pub stamp: LayerCStamp,
    /// Byte-sorted.
    pub toolchains: Vec<String>,
    pub degraded: bool,
    pub workspaces: u32,
    /// The requested files and their items, as stored.
    pub files: u64,
    pub items: u64,
}

/// One file of the snapshot as a read joins it: its stored items by
/// `start`, and its layer A symbols by `ord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCFileRows {
    pub path: String,
    pub items: Vec<LayerCItem>,
    pub symbols: Vec<StoredSymbol>,
}

/// What a read reads of the worktree's snapshot inside its code snapshot's
/// transaction (`SqliteIndex::code_snapshot_with_layer_c`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerCRead {
    pub head: LayerCHead,
    /// The code index's files and packages, as
    /// [`SqliteIndex::code_tree_rows`] reads them.
    pub tree: CodeTreeRows,
    /// Every requested file of the run, by path.
    pub files: Vec<LayerCFile>,
    /// The items whose raw moniker is the searched text, by (path,
    /// `start`); none without a search.
    pub found: Vec<LayerCItem>,
    /// Each file the caller picked, by path.
    pub joined: Vec<LayerCFileRows>,
}

impl SqliteIndex {
    /// Replaces the worktree's layer C snapshot with `snapshot` in one
    /// `Immediate` transaction: the old rows deleted (their children by
    /// cascade), the new ones inserted. The worktree must have code rows
    /// ([`StoreError::NotIndexed`] otherwise). Queue tables and the code
    /// index are never touched.
    pub fn replace_layer_c(&mut self, snapshot: &LayerCSnapshot) -> Result<(), StoreError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate).db()?;
        if !schema::format_is_current(&tx)? {
            return Err(StoreError::NotIndexed);
        }
        let wt = worktree_id(&tx, self)?.ok_or(StoreError::NotIndexed)?;
        if stored_code_fp(&tx, wt)?.is_none() {
            return Err(StoreError::NotIndexed);
        }
        tx.execute("DELETE FROM code_resolve WHERE wt = ?1", [wt])
            .db()?;
        let toolchains = serde_json::to_string(&snapshot.toolchains)
            .map_err(|error| StoreError::Sqlite(error.to_string()))?;
        let files = count(snapshot.files.len())?;
        let items = count(snapshot.items.len())?;
        tx.execute(
            "INSERT INTO code_resolve
               (wt, tree_fp, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                wt,
                snapshot.tree_fp,
                snapshot.protocol,
                snapshot.ra_ap,
                snapshot.host,
                toolchains,
                snapshot.degraded,
                snapshot.workspaces,
                files,
                items,
            ],
        )
        .db()?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO code_resolve_files (wt, path, blake3, in_crate)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .db()?;
            for file in &snapshot.files {
                insert
                    .execute(rusqlite::params![wt, file.path, file.blake3, file.in_crate])
                    .db()?;
            }
            let mut insert = tx
                .prepare(
                    "INSERT INTO code_monikers (wt, path, start, \"end\", kind, state, moniker)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .db()?;
            for item in &snapshot.items {
                insert
                    .execute(rusqlite::params![
                        wt,
                        item.path,
                        count_of(item.start)?,
                        count_of(item.end)?,
                        item.kind,
                        item.state,
                        item.moniker
                    ])
                    .db()?;
            }
        }
        tx.commit().db()
    }

    /// The stored snapshot's fingerprint and stamps; `Ok(None)` when the
    /// worktree has no snapshot.
    pub fn layer_c_stamp(&self) -> Result<Option<LayerCStamp>, StoreError> {
        with_worktree(self, stamp_in)
    }

    /// The whole stored snapshot, files by path and items by (path,
    /// `start`); `Ok(None)` when the worktree has none.
    pub fn layer_c_snapshot(&self) -> Result<Option<LayerCSnapshot>, StoreError> {
        with_worktree(self, |tx, wt| {
            let Some(head) = head_in(tx, wt)? else {
                return Ok(None);
            };
            let files = resolve_files_in(tx, wt)?;
            let items = {
                let mut statement = tx
                    .prepare(
                        "SELECT path, start, \"end\", kind, state, moniker FROM code_monikers
                         WHERE wt = ?1 ORDER BY path, start",
                    )
                    .db()?;
                statement
                    .query_map([wt], item_row)
                    .db()?
                    .collect::<Result<Vec<_>, _>>()
                    .db()?
            };
            let LayerCStamp {
                tree_fp,
                protocol,
                ra_ap,
                host,
            } = head.stamp;
            Ok(Some(LayerCSnapshot {
                tree_fp,
                protocol,
                ra_ap,
                host,
                toolchains: head.toolchains,
                degraded: head.degraded,
                workspaces: head.workspaces,
                files,
                items,
            }))
        })
    }

    /// Every stored code file (path, language, BLAKE3, UTF-8, read error,
    /// package) and package row of the worktree, in one transaction;
    /// `Ok(None)` when no code is indexed.
    pub fn code_tree_rows(&self) -> Result<Option<CodeTreeRows>, StoreError> {
        with_worktree(self, |tx, wt| {
            if stored_code_fp(tx, wt)?.is_none() {
                return Ok(None);
            }
            Ok(Some(tree_rows_in(tx, wt)?))
        })
    }
}

/// The code index's files and packages inside the caller's transaction.
fn tree_rows_in(tx: &Transaction<'_>, wt: i64) -> Result<CodeTreeRows, StoreError> {
    let files = {
        let mut statement = tx
            .prepare(
                "SELECT path, lang, blake3, utf8, read_error, package FROM code_files
                 WHERE wt = ?1 ORDER BY path",
            )
            .db()?;
        statement
            .query_map([wt], |row| {
                Ok(CodeTreeFile {
                    path: row.get(0)?,
                    lang: row.get(1)?,
                    blake3: row.get(2)?,
                    utf8: row.get(3)?,
                    read_error: row.get(4)?,
                    package: row.get(5)?,
                })
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?
    };
    let packages = {
        let mut statement = tx
            .prepare(
                "SELECT dir, manifest_b3, source, targets FROM code_packages
                 WHERE wt = ?1 ORDER BY dir",
            )
            .db()?;
        statement
            .query_map([wt], |row| {
                Ok(CodePackageRow {
                    dir: row.get(0)?,
                    manifest_b3: row.get(1)?,
                    source: row.get(2)?,
                    targets: row.get(3)?,
                })
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?
    };
    Ok(CodeTreeRows { files, packages })
}

/// The snapshot's stamp inside the caller's transaction.
fn stamp_in(tx: &Transaction<'_>, wt: i64) -> Result<Option<LayerCStamp>, StoreError> {
    tx.query_row(
        "SELECT tree_fp, protocol, ra_ap, host FROM code_resolve WHERE wt = ?1",
        [wt],
        |row| {
            Ok(LayerCStamp {
                tree_fp: row.get(0)?,
                protocol: row.get(1)?,
                ra_ap: row.get(2)?,
                host: row.get(3)?,
            })
        },
    )
    .optional()
    .db()
}

/// The run's row inside the caller's transaction; `None` without a
/// snapshot.
fn head_in(tx: &Transaction<'_>, wt: i64) -> Result<Option<LayerCHead>, StoreError> {
    let head = tx
        .query_row(
            "SELECT tree_fp, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items
             FROM code_resolve WHERE wt = ?1",
            [wt],
            |row| {
                Ok((
                    LayerCStamp {
                        tree_fp: row.get(0)?,
                        protocol: row.get(1)?,
                        ra_ap: row.get(2)?,
                        host: row.get(3)?,
                    },
                    row.get::<_, String>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, u32>(6)?,
                    offset(row, 7)?,
                    offset(row, 8)?,
                ))
            },
        )
        .optional()
        .db()?;
    let Some((stamp, toolchains, degraded, workspaces, files, items)) = head else {
        return Ok(None);
    };
    let toolchains: Vec<String> = serde_json::from_str(&toolchains)
        .map_err(|error| StoreError::Sqlite(format!("code_resolve.toolchains: {error}")))?;
    Ok(Some(LayerCHead {
        stamp,
        toolchains,
        degraded,
        workspaces,
        files,
        items,
    }))
}

/// Every requested file of the run, by path, inside the caller's
/// transaction.
fn resolve_files_in(tx: &Transaction<'_>, wt: i64) -> Result<Vec<LayerCFile>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT path, blake3, in_crate FROM code_resolve_files
             WHERE wt = ?1 ORDER BY path",
        )
        .db()?;
    let files = statement
        .query_map([wt], |row| {
            Ok(LayerCFile {
                path: row.get(0)?,
                blake3: row.get(1)?,
                in_crate: row.get(2)?,
            })
        })
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    Ok(files)
}

/// An item from a row of `path, start, "end", kind, state, moniker`.
fn item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LayerCItem> {
    Ok(LayerCItem {
        path: row.get(0)?,
        start: offset(row, 1)?,
        end: offset(row, 2)?,
        kind: row.get(3)?,
        state: row.get(4)?,
        moniker: row.get(5)?,
    })
}

/// The search of [`found_in`]: `?1` the raw moniker, `?2` the worktree;
/// it starts at `code_monikers_by_moniker`, never at the table's key.
pub(crate) const FOUND_SQL: &str = "SELECT path, start, \"end\", kind, state, moniker
     FROM code_monikers INDEXED BY code_monikers_by_moniker
     WHERE moniker = ?1 AND wt = ?2";

/// The statement of one file's items: `?1` the worktree, `?2` the path.
const FILE_ITEMS_SQL: &str = "SELECT path, start, \"end\", kind, state, moniker
     FROM code_monikers WHERE wt = ?1 AND path = ?2 ORDER BY start";

/// The items whose raw moniker is `raw`, by (path, `start`), inside the
/// caller's transaction.
fn found_in(tx: &Transaction<'_>, wt: i64, raw: &str) -> Result<Vec<LayerCItem>, StoreError> {
    let mut statement = tx.prepare(FOUND_SQL).db()?;
    let mut found = statement
        .query_map((raw, wt), item_row)
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    found.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
    Ok(found)
}

/// [`LayerCRead`] inside the caller's transaction, `None` without a
/// snapshot (and then no other statement runs): the run's row, the code
/// index's files and packages, the run's files, the items [`found_in`]
/// finds for `raw` (no search without one), then each path `pick` chooses
/// from them, in byte order, with its items and its symbols.
pub(crate) fn layer_c_read_in(
    tx: &Transaction<'_>,
    wt: i64,
    raw: Option<&str>,
    pick: impl FnOnce(&[LayerCItem]) -> BTreeSet<String>,
) -> Result<Option<LayerCRead>, StoreError> {
    let Some(head) = head_in(tx, wt)? else {
        return Ok(None);
    };
    let tree = tree_rows_in(tx, wt)?;
    let files = resolve_files_in(tx, wt)?;
    let found = match raw {
        Some(raw) => found_in(tx, wt, raw)?,
        None => Vec::new(),
    };
    let paths = pick(&found);
    let mut symbols: BTreeMap<String, Vec<StoredSymbol>> = BTreeMap::new();
    for symbol in files_symbols_in(tx, wt, paths.iter().map(String::as_str))? {
        symbols.entry(symbol.path.clone()).or_default().push(symbol);
    }
    let mut statement = tx.prepare(FILE_ITEMS_SQL).db()?;
    let mut joined = Vec::with_capacity(paths.len());
    for path in paths {
        let items = statement
            .query_map((wt, path.as_str()), item_row)
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        let symbols = symbols.remove(&path).unwrap_or_default();
        joined.push(LayerCFileRows {
            path,
            items,
            symbols,
        });
    }
    Ok(Some(LayerCRead {
        head,
        tree,
        files,
        found,
        joined,
    }))
}

/// A count or offset as a row stores it (SQLite's integers are signed).
fn count_of(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value)
        .map_err(|_| StoreError::Sqlite(format!("{value} exceeds an integer column")))
}

/// A length as a row stores it.
fn count(value: usize) -> Result<i64, StoreError> {
    count_of(u64::try_from(value).unwrap_or(u64::MAX))
}

/// A stored byte offset, never negative.
fn offset(row: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(at)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            at,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}
