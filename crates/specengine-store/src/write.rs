//! The write side: plan against a snapshot of the stored hashes, parse
//! outside any transaction, then apply in one `Immediate` transaction that
//! re-checks every hash it relies on.
//!
//! A file is re-parsed iff its `(path, BLAKE3)` differs from its row, or the
//! worktree's `[ids]` fingerprint or the DB's format stamp changed. A changed
//! file's rows are deleted and inserted again, never replaced in place:
//! `INSERT OR REPLACE` on `nodes`, under `recursive_triggers=OFF`, deletes
//! the old row without firing `nodes_fts_delete` and leaves the FTS index
//! malformed. A row that moved
//! between the snapshot and the transaction (another writer) is decided
//! again inside it, parsing in place when needed; a file edited after it was
//! read is caught by the next update.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};

use rusqlite::{Statement, Transaction, TransactionBehavior};
use specengine_model::IdScheme;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::worktree_id;
use crate::rows::{FileRows, hash_bytes, size_of};
use crate::source::{Source, is_clean_relative};
use crate::{UpdateReport, schema};

/// Which update.
#[derive(Clone, Copy)]
pub(crate) enum Mode<'a> {
    /// Walk everything.
    Walk,
    /// Probe these paths only.
    Paths(&'a [&'a str]),
    /// Walk everything, re-parse everything, replace every row.
    Rebuild,
}

/// The stored state the plan is made against.
struct Snapshot {
    format_current: bool,
    /// `(wt, scheme_fp)` of the worktree.
    worktree: Option<(i64, String)>,
    /// path → BLAKE3 (`None`: stored without one).
    files: BTreeMap<String, Option<String>>,
}

/// What to do with one listed file.
enum Plan {
    /// The stored row had this hash and so did the bytes.
    Unchanged(String),
    /// Rows to write (parsed, or unreadable).
    Write(FileRows),
}

/// What the walk or the probes decided.
struct Targets {
    /// Listed paths, unique, in the source's order.
    listed: Vec<String>,
    /// Paths whose rows go if stored (probed and unlisted); a walk deletes
    /// every stored path it did not list instead.
    unlisted: Vec<String>,
    walked_everything: bool,
}

pub(crate) fn run(
    index: &mut SqliteIndex,
    source: &dyn Source,
    scheme: &IdScheme,
    mode: Mode<'_>,
) -> Result<UpdateReport, StoreError> {
    index.check_root(source)?;
    let fingerprint = fingerprint(scheme)?;
    // A named path that is no clean root-relative path, or a directory (new
    // or stored: a rename), cannot be probed file by file: walk.
    let mode = match mode {
        Mode::Paths(paths)
            if paths
                .iter()
                .any(|path| !is_clean_relative(path) || source.is_dir(path)) =>
        {
            Mode::Walk
        }
        other => other,
    };
    match attempt(index, source, scheme, &fingerprint, mode)? {
        Some(report) => Ok(report),
        // The stamp or the fingerprint moved under `update_paths`: walk.
        None => attempt(index, source, scheme, &fingerprint, Mode::Walk)?
            .ok_or_else(|| StoreError::Sqlite("a full walk was asked to escalate".to_owned())),
    }
}

/// BLAKE3 of the JSON of the scheme's prefixes: an `[ids]` comment or
/// another table of `specengine.toml` leaves it.
fn fingerprint(scheme: &IdScheme) -> Result<String, StoreError> {
    let json = serde_json::to_vec(scheme.prefixes()).map_err(|error| {
        StoreError::Sqlite(format!("cannot encode the [ids] scheme as JSON: {error}"))
    })?;
    Ok(hash_bytes(&json))
}

/// One update; `None` when `update_paths` must escalate to a walk.
fn attempt(
    index: &mut SqliteIndex,
    source: &dyn Source,
    scheme: &IdScheme,
    fingerprint: &str,
    mode: Mode<'_>,
) -> Result<Option<UpdateReport>, StoreError> {
    let snapshot = snapshot(index)?;
    let stale = !snapshot.format_current
        || snapshot
            .worktree
            .as_ref()
            .is_some_and(|(_, stored)| stored != fingerprint);
    let rebuild = matches!(mode, Mode::Rebuild);
    let reparse_all = stale || rebuild;

    let mut report = UpdateReport::default();
    let targets = match mode {
        Mode::Paths(paths) if !stale && snapshot.worktree.is_some() => {
            probe_targets(source, paths, &snapshot)
        }
        Mode::Paths(_) | Mode::Walk | Mode::Rebuild => {
            let listing = source
                .list()
                .map_err(|error| StoreError::io(source.root(), error))?;
            report.missing_roots = listing.missing_roots;
            report.skipped_names = listing.skipped_names;
            report.unreadable_dirs = listing.unreadable_dirs;
            // A source's repeat is dropped; its order is kept (rows never
            // depend on it).
            let mut seen = BTreeSet::new();
            let mut paths = listing.paths;
            paths.retain(|path| seen.insert(path.clone()));
            Targets {
                listed: paths,
                unlisted: Vec::new(),
                walked_everything: true,
            }
        }
    };
    report.walked = targets.listed.len();

    // Read, hash and parse before any write lock is taken.
    let mut plans = Vec::with_capacity(targets.listed.len());
    for path in &targets.listed {
        let stored = if reparse_all {
            None
        } else {
            snapshot.files.get(path)
        };
        plans.push(plan_file(source, scheme, path, stored)?);
    }

    // `&mut` on the handle already rules out a nested transaction.
    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Immediate).db()?;
    let recreated = schema::recreate_if_stale(&tx)?;
    let current = worktree_row(&tx, index)?;
    let fingerprint_moved = current
        .as_ref()
        .is_some_and(|(_, stored)| stored != fingerprint);
    if !targets.walked_everything && (recreated || current.is_none() || fingerprint_moved) {
        drop(tx);
        return Ok(None);
    }
    let reparse_all = reparse_all || recreated || fingerprint_moved;
    report.reparsed_all = reparse_all;

    let wt = match current {
        Some((wt, stored)) => {
            if stored != fingerprint {
                tx.execute(
                    "UPDATE worktrees SET scheme_fp = ?1 WHERE wt = ?2",
                    (fingerprint, wt),
                )
                .db()?;
            }
            wt
        }
        None => {
            tx.execute(
                "INSERT INTO worktrees (project, root, scheme_fp) VALUES (?1, ?2, ?3)",
                (
                    index.project.as_str(),
                    index.root_text.as_str(),
                    fingerprint,
                ),
            )
            .db()?;
            tx.last_insert_rowid()
        }
    };
    let stored = stored_files(&tx, wt)?;
    let mut writer = Writer::new(&tx)?;

    let listed: BTreeSet<&str> = targets.listed.iter().map(String::as_str).collect();
    let mut gone: Vec<(&str, i64)> = Vec::new();
    if targets.walked_everything {
        for (path, (file_id, _)) in &stored {
            if !listed.contains(path.as_str()) {
                gone.push((path.as_str(), *file_id));
            }
        }
    } else {
        for path in &targets.unlisted {
            if let Some((file_id, _)) = stored.get(path) {
                gone.push((path.as_str(), *file_id));
            }
        }
    }
    for (_, file_id) in &gone {
        writer.delete(*file_id)?;
    }
    report.removed = gone.len();
    if rebuild {
        // Replace every remaining row too, in this same transaction.
        for (path, (file_id, _)) in &stored {
            if listed.contains(path.as_str()) {
                writer.delete(*file_id)?;
            }
        }
    }

    for (path, plan) in targets.listed.iter().zip(plans) {
        let row = if rebuild { None } else { stored.get(path) };
        let stored_hash = row.and_then(|(_, hash)| hash.as_deref());
        let rows = match plan {
            Plan::Unchanged(hash) if !reparse_all && stored_hash == Some(hash.as_str()) => {
                report.unchanged += 1;
                continue;
            }
            // The row moved since the snapshot, or everything must be
            // re-parsed now: parse in place.
            Plan::Unchanged(_) => match plan_file(source, scheme, path, None)? {
                Plan::Write(rows) => rows,
                Plan::Unchanged(_) => continue,
            },
            Plan::Write(rows) => {
                if !reparse_all && rows.blake3.is_some() && stored_hash == rows.blake3.as_deref() {
                    // Another writer stored these very bytes meanwhile.
                    report.unchanged += 1;
                    continue;
                }
                rows
            }
        };
        if let Some((file_id, _)) = row {
            writer.delete(*file_id)?;
        }
        writer.insert(wt, &rows)?;
        if rows.read_error.is_some() {
            report.unreadable += 1;
        } else {
            report.parsed += 1;
        }
    }
    drop(writer);
    tx.commit().db()?;
    Ok(Some(report))
}

fn snapshot(index: &SqliteIndex) -> Result<Snapshot, StoreError> {
    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Deferred).db()?;
    let format_current = schema::format_is_current(&tx)?;
    let worktree = if format_current {
        worktree_row(&tx, index)?
    } else {
        None
    };
    let files = match &worktree {
        Some((wt, _)) => stored_files(&tx, *wt)?
            .into_iter()
            .map(|(path, (_, hash))| (path, hash))
            .collect(),
        None => BTreeMap::new(),
    };
    tx.commit().db()?;
    Ok(Snapshot {
        format_current,
        worktree,
        files,
    })
}

fn worktree_row(
    tx: &Transaction<'_>,
    index: &SqliteIndex,
) -> Result<Option<(i64, String)>, StoreError> {
    let Some(wt) = worktree_id(tx, index)? else {
        return Ok(None);
    };
    let fingerprint = tx
        .query_row(
            "SELECT scheme_fp FROM worktrees WHERE wt = ?1",
            [wt],
            |row| row.get::<_, String>(0),
        )
        .db()?;
    Ok(Some((wt, fingerprint)))
}

/// path → (file_id, BLAKE3) of the worktree's stored files.
fn stored_files(
    tx: &Transaction<'_>,
    wt: i64,
) -> Result<BTreeMap<String, (i64, Option<String>)>, StoreError> {
    let mut statement = tx
        .prepare("SELECT path, file_id, blake3 FROM files WHERE wt = ?1")
        .db()?;
    let rows = statement
        .query_map([wt], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?),
            ))
        })
        .db()?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .db()?;
    Ok(rows)
}

/// The named paths, deduplicated and sorted, split by the walk rules; a
/// named path that was a stored directory (gone from disk: an existing one
/// escalates to a walk) brings the stored files under it.
fn probe_targets(source: &dyn Source, paths: &[&str], snapshot: &Snapshot) -> Targets {
    let mut named: BTreeSet<String> = BTreeSet::new();
    for path in paths {
        named.insert((*path).to_owned());
        let prefix = format!("{path}/");
        named.extend(
            snapshot
                .files
                .range(prefix.clone()..)
                .take_while(|(stored, _)| stored.starts_with(&prefix))
                .map(|(stored, _)| stored.clone()),
        );
    }
    let (listed, unlisted) = named.into_iter().partition(|path| source.probe(path));
    Targets {
        listed,
        unlisted,
        walked_everything: false,
    }
}

/// Reads, hashes and (unless the stored hash matches) parses one file.
fn plan_file(
    source: &dyn Source,
    scheme: &IdScheme,
    path: &str,
    stored: Option<&Option<String>>,
) -> Result<Plan, StoreError> {
    let bytes = match source.read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Ok(Plan::Write(FileRows::unreadable(
                path,
                0,
                error.to_string(),
            )));
        }
    };
    let hash = hash_bytes(&bytes);
    if stored.is_some_and(|stored| stored.as_deref() == Some(hash.as_str())) {
        return Ok(Plan::Unchanged(hash));
    }
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, &bytes, scheme)
    }));
    match parsed {
        Ok(parsed) => Ok(Plan::Write(FileRows::parsed(&bytes, &parsed)?)),
        Err(_) => Ok(Plan::Write(FileRows::unreadable(
            path,
            size_of(&bytes),
            "the spec parser panicked on this file".to_owned(),
        ))),
    }
}

/// The prepared statements of one write transaction.
struct Writer<'t> {
    insert_file: Statement<'t>,
    insert_node: Statement<'t>,
    insert_alias: Statement<'t>,
    insert_link: Statement<'t>,
    insert_anchor: Statement<'t>,
    insert_diagnostic: Statement<'t>,
    delete_nodes: Statement<'t>,
    delete_file: Statement<'t>,
}

impl<'t> Writer<'t> {
    fn new(tx: &'t Transaction<'_>) -> Result<Self, StoreError> {
        Ok(Self {
            insert_file: tx
                .prepare(
                    "INSERT INTO files (wt, path, blake3, size, read_error, shell)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .db()?,
            insert_node: tx
                .prepare(
                    "INSERT INTO nodes (file_id, ord, id, kind, title, parent_id, own_text, node)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .db()?,
            insert_alias: tx
                .prepare("INSERT INTO aliases (node_id, ord, alias) VALUES (?1, ?2, ?3)")
                .db()?,
            insert_link: tx
                .prepare(
                    "INSERT INTO links (file_id, ord, src, type, dst_id, dst_path, link)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .db()?,
            insert_anchor: tx
                .prepare("INSERT INTO anchors (file_id, ord, name, anchor) VALUES (?1, ?2, ?3, ?4)")
                .db()?,
            insert_diagnostic: tx
                .prepare(
                    "INSERT INTO diagnostics (file_id, ord, code, diagnostic)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .db()?,
            // Nodes go first and one by one through their FTS delete
            // trigger; the file's other rows follow it by cascade.
            delete_nodes: tx.prepare("DELETE FROM nodes WHERE file_id = ?1").db()?,
            delete_file: tx.prepare("DELETE FROM files WHERE file_id = ?1").db()?,
        })
    }

    fn delete(&mut self, file_id: i64) -> Result<(), StoreError> {
        self.delete_nodes.execute([file_id]).db()?;
        self.delete_file.execute([file_id]).db()?;
        Ok(())
    }

    fn insert(&mut self, wt: i64, rows: &FileRows) -> Result<(), StoreError> {
        let file_id = self
            .insert_file
            .insert((
                wt,
                rows.path.as_str(),
                rows.blake3.as_deref(),
                rows.size,
                rows.read_error.as_deref(),
                rows.shell.as_deref(),
            ))
            .db()?;
        for (ord, node) in rows.nodes.iter().enumerate() {
            let node_id = self
                .insert_node
                .insert((
                    file_id,
                    ord_value(ord),
                    node.id.as_deref(),
                    node.kind.as_deref(),
                    node.title.as_deref(),
                    node.parent_id.as_deref(),
                    node.own_text.as_str(),
                    node.node.as_str(),
                ))
                .db()?;
            for (alias_ord, alias) in node.aliases.iter().enumerate() {
                self.insert_alias
                    .execute((node_id, ord_value(alias_ord), alias.as_str()))
                    .db()?;
            }
        }
        for (ord, link) in rows.links.iter().enumerate() {
            self.insert_link
                .execute((
                    file_id,
                    ord_value(ord),
                    link.src.as_deref(),
                    link.link_type.as_str(),
                    link.dst_id.as_deref(),
                    link.dst_path.as_deref(),
                    link.link.as_str(),
                ))
                .db()?;
        }
        for (ord, anchor) in rows.anchors.iter().enumerate() {
            self.insert_anchor
                .execute((
                    file_id,
                    ord_value(ord),
                    anchor.name.as_str(),
                    anchor.anchor.as_str(),
                ))
                .db()?;
        }
        for (ord, diagnostic) in rows.diagnostics.iter().enumerate() {
            self.insert_diagnostic
                .execute((
                    file_id,
                    ord_value(ord),
                    diagnostic.code.as_str(),
                    diagnostic.diagnostic.as_str(),
                ))
                .db()?;
        }
        Ok(())
    }
}

fn ord_value(ord: usize) -> i64 {
    i64::try_from(ord).unwrap_or(i64::MAX)
}
