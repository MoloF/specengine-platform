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
//!
//! The edge tables follow the file rows in the same transaction
//! (`crate::edges`): a full pass when the stamp was recreated, the
//! worktree is new, the `[ids]` or the `[paths]` fingerprint moved, or on
//! a rebuild; else an incremental pass over what the file rows changed. A
//! `[paths]` move alone re-parses nothing.
//!
//! The stat key (the `read-trust` spec, "The key and the racy rule"): each
//! file is `stat`ed before it is read. `update` does not read a file whose
//! row has a BLAKE3 and a stored key equal to the file's now, taken
//! [`RACY_NS`] or more after that key's last change; `update_paths` and
//! `rebuild` read every file. A read file keeps its rows when its hash is
//! unchanged, and its key is stored when it differs, when none is stored,
//! or when it ends the stored key's raciness. An update that changes
//! nothing (the plans stand, no key to store, no path gone, the listing's
//! notes as stored) returns before the write transaction, `committed`
//! false.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Statement, Transaction, TransactionBehavior};
use specengine_core::{Paths, is_clean_relative, is_under};
use specengine_model::IdScheme;

use crate::edges::{self, Changes};
use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::{walk_notes, worktree_id};
use crate::rows::{FileRows, hash_bytes, size_of};
use crate::source::{Probed, Source, StatKey};
use crate::{UpdateReport, WalkNotes, schema};

/// The racy window, in nanoseconds: a key taken less than this after its
/// file's last change (`max(mtime, ctime)`) is not trusted, since a write
/// within the same timestamp tick may not move it.
pub(crate) const RACY_NS: i64 = 2_000_000_000;

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
    worktree: Option<StoredWorktree>,
    files: BTreeMap<String, StoredFile>,
    /// The worktree's `walk_notes`, when stored and readable.
    notes: Option<WalkNotes>,
}

/// One stored file as the plan needs it.
struct StoredFile {
    /// `None`: stored without one (unreadable, or the parser panicked).
    blake3: Option<String>,
    size: i64,
    read_error: Option<String>,
    stat: Option<StoredStat>,
}

/// A `file_stats` row.
#[derive(Clone, Copy)]
struct StoredStat {
    key: StatKey,
    taken_ns: i64,
}

impl StoredStat {
    fn racy(&self) -> bool {
        racy(self.taken_ns, &self.key)
    }
}

/// `key` taken at `taken_ns` is within the racy window of its change.
fn racy(taken_ns: i64, key: &StatKey) -> bool {
    taken_ns.saturating_sub(key.mtime_ns.max(key.ctime_ns)) < RACY_NS
}

/// A worktree's row.
struct StoredWorktree {
    wt: i64,
    scheme_fp: String,
    paths_fp: String,
}

/// The two fingerprints of one update: `[ids]` and the `[paths]` keys
/// resolution reads.
struct Fingerprints {
    scheme: String,
    paths: String,
}

impl StoredWorktree {
    fn scheme_moved(&self, fingerprints: &Fingerprints) -> bool {
        self.scheme_fp != fingerprints.scheme
    }

    fn paths_moved(&self, fingerprints: &Fingerprints) -> bool {
        self.paths_fp != fingerprints.paths
    }
}

/// What to do with one listed file.
enum Plan {
    /// The stored row had this hash and so did the bytes, or its key was
    /// trusted and the file not read; `stat`: what its key row needs.
    Unchanged { hash: String, stat: StatWrite },
    /// Stored without a hash, and failing again with the same `read_error`
    /// and size: its rows stand.
    Kept,
    /// Rows to write (parsed, or unreadable), and the key taken before the
    /// read (stored only beside a BLAKE3); boxed, as most plans of a walk
    /// are unchanged.
    Write(Box<FileRows>, Option<StatKey>),
}

impl Plan {
    /// Writes nothing.
    fn stands(&self) -> bool {
        matches!(
            self,
            Self::Unchanged {
                stat: StatWrite::Keep,
                ..
            } | Self::Kept
        )
    }
}

/// What an unchanged file's `file_stats` row needs.
#[derive(Clone, Copy)]
enum StatWrite {
    Keep,
    Store(StatKey),
    Drop,
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
    // Read once, before the first `stat`.
    let taken_ns = now_ns();
    // Only `update` trusts a key: `update_paths` runs after a known change
    // or a mismatch, which is evidence a key lied.
    let trust_keys = matches!(mode, Mode::Walk);
    let fingerprints = Fingerprints {
        scheme: fingerprint(scheme)?,
        paths: paths_fingerprint(source.paths()),
    };
    // The named paths are probed in one batch. One that is no clean
    // root-relative path, or a directory (new or stored: a rename), cannot
    // be probed file by file: walk.
    let probed = match mode {
        Mode::Paths(paths) if paths.iter().all(|path| is_clean_relative(path)) => {
            let probed = source.probe_paths(paths);
            (!probed.contains(&Probed::Dir)).then_some(probed)
        }
        _ => None,
    };
    let mode = match (mode, &probed) {
        (Mode::Paths(_), None) => Mode::Walk,
        (other, _) => other,
    };
    let probed = probed.as_deref();
    let keys = Keys {
        trust: trust_keys,
        taken_ns,
    };
    match attempt(index, source, scheme, &fingerprints, mode, probed, keys)? {
        Some(report) => Ok(report),
        // `update_paths` could not probe: the stamp or a fingerprint moved,
        // the worktree is new, or a root may gain its first file or lose its
        // last (`roots_may_move`): walk.
        None => attempt(index, source, scheme, &fingerprints, Mode::Walk, None, keys)?
            .ok_or_else(|| StoreError::Sqlite("a full walk was asked to escalate".to_owned())),
    }
}

/// How an update treats stat keys: whether it trusts them, and its clock.
#[derive(Clone, Copy)]
struct Keys {
    trust: bool,
    taken_ns: i64,
}

/// The wall clock in nanoseconds from the Unix epoch, negative before it,
/// saturated past an `i64`.
fn now_ns() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |ns| -ns),
    }
}

/// BLAKE3 of the JSON of the scheme's prefixes: an `[ids]` comment or
/// another table of `specengine.toml` leaves it.
pub(crate) fn fingerprint(scheme: &IdScheme) -> Result<String, StoreError> {
    let json = serde_json::to_vec(scheme.prefixes()).map_err(|error| {
        StoreError::Sqlite(format!("cannot encode the [ids] scheme as JSON: {error}"))
    })?;
    Ok(hash_bytes(&json))
}

/// BLAKE3 of core's `Paths::resolution_json`: the `[paths]` keys
/// resolution reads (`features`, `link_base`, `roots`, `exclude`). When it
/// moves every edge is recomputed and nothing is re-parsed.
pub(crate) fn paths_fingerprint(paths: &Paths) -> String {
    hash_bytes(paths.resolution_json().as_bytes())
}

/// One update; `None` when `update_paths` must escalate to a walk (see
/// [`run`]).
/// `probed`: the named paths' probes, for [`Mode::Paths`].
fn attempt(
    index: &mut SqliteIndex,
    source: &dyn Source,
    scheme: &IdScheme,
    fingerprints: &Fingerprints,
    mode: Mode<'_>,
    probed: Option<&[Probed]>,
    keys: Keys,
) -> Result<Option<UpdateReport>, StoreError> {
    let snapshot = snapshot(index)?;
    let stale = !snapshot.format_current
        || snapshot
            .worktree
            .as_ref()
            .is_some_and(|stored| stored.scheme_moved(fingerprints));
    let paths_moved = snapshot
        .worktree
        .as_ref()
        .is_some_and(|stored| stored.paths_moved(fingerprints));
    let rebuild = matches!(mode, Mode::Rebuild);
    let reparse_all = stale || rebuild;

    let mut report = UpdateReport::default();
    let targets = match (mode, probed) {
        (Mode::Paths(paths), Some(probed))
            if !stale && !paths_moved && snapshot.worktree.is_some() =>
        {
            let targets = probe_targets(source, paths, probed, &snapshot);
            if roots_may_move(&targets, &snapshot, source.paths()) {
                // A root may gain its first file or lose its last: what the
                // listing says of it (`walk_notes`) only a walk records.
                return Ok(None);
            }
            targets
        }
        (Mode::Paths(_) | Mode::Walk | Mode::Rebuild, _) => {
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

    // Stat, read, hash and parse before any write lock is taken.
    let keys = Keys {
        trust: keys.trust && !reparse_all,
        ..keys
    };
    let mut plans = Vec::with_capacity(targets.listed.len());
    for path in &targets.listed {
        let stored = if reparse_all {
            None
        } else {
            snapshot.files.get(path)
        };
        plans.push(plan_file(
            source,
            scheme,
            path,
            stored,
            keys,
            &mut report.hashed,
        )?);
    }
    let notes = targets.walked_everything.then(|| WalkNotes::from(&report));

    // Nothing to write: no write lock, no transaction.
    let gone_none = if targets.walked_everything {
        // Every plan stands, so every listed path is stored: equal counts
        // mean no stored path went unlisted.
        snapshot.files.len() == targets.listed.len()
    } else {
        targets
            .unlisted
            .iter()
            .all(|path| !snapshot.files.contains_key(path))
    };
    if !stale
        && !paths_moved
        && !rebuild
        && snapshot.worktree.is_some()
        && plans.iter().all(Plan::stands)
        && gone_none
        && (notes.is_none() || notes == snapshot.notes)
    {
        for plan in &plans {
            match plan {
                Plan::Kept => report.unreadable += 1,
                _ => report.unchanged += 1,
            }
        }
        return Ok(Some(report));
    }

    // `&mut` on the handle already rules out a nested transaction.
    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Immediate).db()?;
    let recreated = schema::recreate_if_stale(&tx)?;
    let current = worktree_row(&tx, index)?;
    let fingerprint_moved = current
        .as_ref()
        .is_some_and(|stored| stored.scheme_moved(fingerprints));
    let paths_moved = current
        .as_ref()
        .is_some_and(|stored| stored.paths_moved(fingerprints));
    if !targets.walked_everything
        && (recreated || current.is_none() || fingerprint_moved || paths_moved)
    {
        drop(tx);
        return Ok(None);
    }
    let reparse_all = reparse_all || recreated || fingerprint_moved;
    report.reparsed_all = reparse_all;
    // Every edge is recomputed: the stamp was recreated, the worktree is
    // new, a fingerprint moved, or a rebuild (which re-parses everything).
    let resolve_all = reparse_all || paths_moved || current.is_none();

    let wt = match current {
        Some(stored) => {
            if stored.scheme_moved(fingerprints) || stored.paths_moved(fingerprints) {
                tx.execute(
                    "UPDATE worktrees SET scheme_fp = ?1, paths_fp = ?2 WHERE wt = ?3",
                    (
                        fingerprints.scheme.as_str(),
                        fingerprints.paths.as_str(),
                        stored.wt,
                    ),
                )
                .db()?;
            }
            stored.wt
        }
        None => {
            tx.execute(
                "INSERT INTO worktrees (project, root, scheme_fp, paths_fp)
                 VALUES (?1, ?2, ?3, ?4)",
                (
                    index.project.as_str(),
                    index.root_text.as_str(),
                    fingerprints.scheme.as_str(),
                    fingerprints.paths.as_str(),
                ),
            )
            .db()?;
            tx.last_insert_rowid()
        }
    };
    let stored = stored_files(&tx, wt)?;
    let mut writer = Writer::new(&tx, !resolve_all)?;

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
    for (path, file_id) in &gone {
        writer.delete(*file_id)?;
        writer.deleted(path);
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

    let fresh = Keys {
        trust: false,
        ..keys
    };
    for (path, plan) in targets.listed.iter().zip(plans) {
        let row = if rebuild { None } else { stored.get(path) };
        let stored_hash = row.and_then(|(_, hash)| hash.as_deref());
        let (rows, key) = match plan {
            Plan::Unchanged { hash, stat }
                if !reparse_all && stored_hash == Some(hash.as_str()) =>
            {
                if let Some((file_id, _)) = row {
                    writer.stat(*file_id, stat, keys.taken_ns)?;
                }
                report.unchanged += 1;
                continue;
            }
            Plan::Kept if !reparse_all && row.is_some() && stored_hash.is_none() => {
                report.unreadable += 1;
                continue;
            }
            // The row moved since the snapshot, or everything must be
            // re-parsed now: parse in place.
            Plan::Unchanged { .. } | Plan::Kept => {
                match plan_file(source, scheme, path, None, fresh, &mut report.hashed)? {
                    Plan::Write(rows, key) => (rows, key),
                    Plan::Unchanged { .. } | Plan::Kept => continue,
                }
            }
            Plan::Write(rows, key) => {
                if !reparse_all && rows.blake3.is_some() && stored_hash == rows.blake3.as_deref() {
                    // Another writer stored these very bytes meanwhile.
                    report.unchanged += 1;
                    continue;
                }
                (rows, key)
            }
        };
        if let Some((file_id, _)) = row {
            writer.delete(*file_id)?;
        }
        let file_id = writer.insert(wt, &rows)?;
        if rows.read_error.is_some() {
            report.unreadable += 1;
        } else {
            report.parsed += 1;
        }
        if rows.blake3.is_some()
            && let Some(key) = key
        {
            writer.stat(file_id, StatWrite::Store(key), keys.taken_ns)?;
        }
    }
    if let Some(notes) = &notes
        && walk_notes(&tx, wt)?.as_ref() != Some(notes)
    {
        store_notes(&tx, wt, notes)?;
    }
    let changes = writer.into_changes();
    let paths = source.paths();
    if resolve_all {
        report.resolved = edges::full_pass(&tx, wt, scheme, paths)?;
        report.resolved_all = true;
    } else {
        report.resolved = edges::incremental_pass(&tx, wt, scheme, paths, &changes)?;
    }
    tx.commit().db()?;
    report.committed = true;
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
    let (files, notes) = match &worktree {
        Some(stored) => (snapshot_files(&tx, stored.wt)?, walk_notes(&tx, stored.wt)?),
        None => (BTreeMap::new(), None),
    };
    tx.commit().db()?;
    Ok(Snapshot {
        format_current,
        worktree,
        files,
        notes,
    })
}

/// path → the stored file and its key, of the worktree's files.
fn snapshot_files(
    tx: &Transaction<'_>,
    wt: i64,
) -> Result<BTreeMap<String, StoredFile>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path, f.blake3, f.size, f.read_error,
                    s.size, s.mtime_ns, s.ctime_ns, s.ino, s.dev, s.taken_ns
             FROM files f LEFT JOIN file_stats s ON s.file_id = f.file_id
             WHERE f.wt = ?1",
        )
        .db()?;
    let rows = statement
        .query_map([wt], |row| {
            let stat = match (
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
            ) {
                (
                    Some(size),
                    Some(mtime_ns),
                    Some(ctime_ns),
                    Some(ino),
                    Some(dev),
                    Some(taken_ns),
                ) => Some(StoredStat {
                    key: StatKey {
                        size: from_bits(size),
                        mtime_ns,
                        ctime_ns,
                        ino: from_bits(ino),
                        dev: from_bits(dev),
                    },
                    taken_ns,
                }),
                _ => None,
            };
            Ok((
                row.get::<_, String>(0)?,
                StoredFile {
                    blake3: row.get(1)?,
                    size: row.get(2)?,
                    read_error: row.get(3)?,
                    stat,
                },
            ))
        })
        .db()?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .db()?;
    Ok(rows)
}

/// A `u64` stored as an `INTEGER`: its bits as an `i64`.
fn to_bits(value: u64) -> i64 {
    i64::from_ne_bytes(value.to_ne_bytes())
}

/// [`to_bits`] read back.
fn from_bits(value: i64) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

/// `notes` as the worktree's `walk_notes` row.
fn store_notes(tx: &Transaction<'_>, wt: i64, notes: &WalkNotes) -> Result<(), StoreError> {
    let json = |list: &Vec<String>| {
        serde_json::to_string(list).map_err(|error| {
            StoreError::Sqlite(format!("cannot encode the walk's notes as JSON: {error}"))
        })
    };
    tx.execute(
        "INSERT OR REPLACE INTO walk_notes (wt, missing_roots, unreadable_dirs, skipped_names)
         VALUES (?1, ?2, ?3, ?4)",
        (
            wt,
            json(&notes.missing_roots)?,
            json(&notes.unreadable_dirs)?,
            i64::try_from(notes.skipped_names).unwrap_or(i64::MAX),
        ),
    )
    .db()?;
    Ok(())
}

fn worktree_row(
    tx: &Transaction<'_>,
    index: &SqliteIndex,
) -> Result<Option<StoredWorktree>, StoreError> {
    let Some(wt) = worktree_id(tx, index)? else {
        return Ok(None);
    };
    let (scheme_fp, paths_fp) = tx
        .query_row(
            "SELECT scheme_fp, paths_fp FROM worktrees WHERE wt = ?1",
            [wt],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .db()?;
    Ok(Some(StoredWorktree {
        wt,
        scheme_fp,
        paths_fp,
    }))
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

/// The named paths, deduplicated and sorted, split by their probes
/// (`probed`, in `paths`' order, none a directory); a named path that was
/// a stored directory (gone from disk: an existing one escalates to a
/// walk) brings the stored files under it, probed in a second batch.
fn probe_targets(
    source: &dyn Source,
    paths: &[&str],
    probed: &[Probed],
    snapshot: &Snapshot,
) -> Targets {
    let mut named: BTreeMap<String, bool> = BTreeMap::new();
    let mut below: BTreeSet<String> = BTreeSet::new();
    for (path, probe) in paths.iter().zip(probed) {
        named
            .entry((*path).to_owned())
            .or_insert(*probe == Probed::Listed);
        let prefix = format!("{path}/");
        below.extend(
            snapshot
                .files
                .range(prefix.clone()..)
                .take_while(|(stored, _)| stored.starts_with(&prefix))
                .map(|(stored, _)| stored.clone()),
        );
    }
    let below: Vec<&str> = below
        .iter()
        .map(String::as_str)
        .filter(|path| !named.contains_key(*path))
        .collect();
    // A stored file is listed only as a file: a directory in its place is
    // unlisted, as `Source::probe` has it.
    let below_probed = source.probe_paths(&below);
    let below: Vec<(String, bool)> = below
        .into_iter()
        .zip(below_probed)
        .map(|(path, probe)| (path.to_owned(), probe == Probed::Listed))
        .collect();
    named.extend(below);
    let mut listed = Vec::new();
    let mut unlisted = Vec::new();
    for (path, is_listed) in named {
        if is_listed {
            listed.push(path);
        } else {
            unlisted.push(path);
        }
    }
    Targets {
        listed,
        unlisted,
        walked_everything: false,
    }
}

/// Some root of `paths` may change what a listing says of it: a probed
/// update lists a file under a root that holds no stored file, or unlists
/// every stored file under one. Such a root may stop or start being
/// missing, which only a walk's listing tells.
fn roots_may_move(targets: &Targets, snapshot: &Snapshot, paths: &Paths) -> bool {
    paths.roots.iter().any(|root| {
        let under = |path: &str| path == root.as_str() || is_under(path, root);
        let added = targets
            .listed
            .iter()
            .any(|path| under(path) && !snapshot.files.contains_key(path));
        let removed = targets
            .unlisted
            .iter()
            .filter(|path| under(path) && snapshot.files.contains_key(*path))
            .count();
        if !added && removed == 0 {
            return false;
        }
        // Stored files under the root, counted only as far as needed.
        let prefix = format!("{root}/");
        let stored = usize::from(snapshot.files.contains_key(root))
            + snapshot
                .files
                .range(prefix.clone()..)
                .take_while(|(path, _)| path.starts_with(&prefix))
                .take(removed + 1)
                .count();
        (added && stored == 0) || (removed > 0 && removed >= stored)
    })
}

/// Stats one file, then, unless its key is trusted, reads (counted in
/// `hashed`), hashes and (unless the stored hash matches) parses it.
fn plan_file(
    source: &dyn Source,
    scheme: &IdScheme,
    path: &str,
    stored: Option<&StoredFile>,
    keys: Keys,
    hashed: &mut usize,
) -> Result<Plan, StoreError> {
    let key = source.stat(path);
    if keys.trust
        && let Some(stored) = stored
        && let Some(hash) = &stored.blake3
        && let (Some(key), Some(stat)) = (key, stored.stat)
        && key == stat.key
        && !stat.racy()
    {
        return Ok(Plan::Unchanged {
            hash: hash.clone(),
            stat: StatWrite::Keep,
        });
    }
    *hashed += 1;
    let bytes = match source.read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            let rows = FileRows::unreadable(path, 0, error.to_string());
            return Ok(unreadable_plan(rows, stored));
        }
    };
    let hash = hash_bytes(&bytes);
    if let Some(stored) = stored
        && stored.blake3.as_deref() == Some(hash.as_str())
    {
        return Ok(Plan::Unchanged {
            hash,
            stat: stat_write(key, stored.stat, keys.taken_ns),
        });
    }
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(path, &bytes, scheme)
    }));
    match parsed {
        Ok(parsed) => Ok(Plan::Write(
            Box::new(FileRows::parsed(&bytes, &parsed)?),
            key,
        )),
        Err(_) => Ok(unreadable_plan(
            FileRows::unreadable(
                path,
                size_of(&bytes),
                "the spec parser panicked on this file".to_owned(),
            ),
            stored,
        )),
    }
}

/// An unreadable file's plan: its stored rows stand when they hold no hash
/// and the same `read_error` and size; else these rows, with no key.
fn unreadable_plan(rows: FileRows, stored: Option<&StoredFile>) -> Plan {
    let same = stored.is_some_and(|stored| {
        stored.blake3.is_none() && stored.read_error == rows.read_error && stored.size == rows.size
    });
    if same {
        Plan::Kept
    } else {
        Plan::Write(Box::new(rows), None)
    }
}

/// What an unchanged file's key row needs: `key` stored when it differs
/// from the stored one, when none is stored, or when it ends the stored
/// key's raciness; the row dropped when the file has no key now.
fn stat_write(key: Option<StatKey>, stored: Option<StoredStat>, taken_ns: i64) -> StatWrite {
    match (key, stored) {
        (None, None) => StatWrite::Keep,
        (None, Some(_)) => StatWrite::Drop,
        (Some(key), None) => StatWrite::Store(key),
        (Some(key), Some(stored)) if key != stored.key => StatWrite::Store(key),
        (Some(key), Some(stored)) if stored.racy() && !racy(taken_ns, &key) => {
            StatWrite::Store(key)
        }
        (Some(_), Some(_)) => StatWrite::Keep,
    }
}

/// The prepared statements of one write transaction, and what its file
/// rows changed (for the incremental edge pass; `None` before a full one).
struct Writer<'t> {
    insert_file: Statement<'t>,
    insert_name: Statement<'t>,
    select_names: Statement<'t>,
    changes: Option<Changes>,
    insert_node: Statement<'t>,
    insert_alias: Statement<'t>,
    insert_link: Statement<'t>,
    insert_anchor: Statement<'t>,
    insert_diagnostic: Statement<'t>,
    delete_nodes: Statement<'t>,
    delete_file: Statement<'t>,
    store_stat: Statement<'t>,
    delete_stat: Statement<'t>,
}

impl<'t> Writer<'t> {
    /// `track`: gather the [`Changes`] an incremental edge pass needs.
    fn new(tx: &'t Transaction<'_>, track: bool) -> Result<Self, StoreError> {
        Ok(Self {
            insert_name: tx
                .prepare("INSERT INTO edge_names (file_id, key) VALUES (?1, ?2)")
                .db()?,
            select_names: tx
                .prepare("SELECT key FROM edge_names WHERE file_id = ?1")
                .db()?,
            changes: track.then(Changes::default),
            insert_file: tx
                .prepare(
                    "INSERT INTO files (wt, path, blake3, size, read_error, shell, tier3)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .db()?,
            insert_node: tx
                .prepare(
                    "INSERT INTO nodes
                       (file_id, ord, line, id, kind, title, parent_id, own_text,
                        norm_hash, node)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            store_stat: tx
                .prepare(
                    "INSERT OR REPLACE INTO file_stats
                       (file_id, size, mtime_ns, ctime_ns, ino, dev, taken_ns)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .db()?,
            delete_stat: tx
                .prepare("DELETE FROM file_stats WHERE file_id = ?1")
                .db()?,
        })
    }

    /// A file's key row as `write` asks, the key taken at `taken_ns`.
    fn stat(&mut self, file_id: i64, write: StatWrite, taken_ns: i64) -> Result<(), StoreError> {
        match write {
            StatWrite::Keep => {}
            StatWrite::Store(key) => {
                self.store_stat
                    .execute((
                        file_id,
                        to_bits(key.size),
                        key.mtime_ns,
                        key.ctime_ns,
                        to_bits(key.ino),
                        to_bits(key.dev),
                        taken_ns,
                    ))
                    .db()?;
            }
            StatWrite::Drop => {
                self.delete_stat.execute([file_id]).db()?;
            }
        }
        Ok(())
    }

    /// A file's rows go; its names are read first, when tracked.
    fn delete(&mut self, file_id: i64) -> Result<(), StoreError> {
        if let Some(changes) = &mut self.changes {
            let names = self
                .select_names
                .query_map([file_id], |row| row.get::<_, String>(0))
                .db()?;
            for name in names {
                changes.names.insert(name.db()?);
            }
        }
        self.delete_nodes.execute([file_id]).db()?;
        self.delete_file.execute([file_id]).db()?;
        Ok(())
    }

    /// The deleted file at `path` is not written again.
    fn deleted(&mut self, path: &str) {
        if let Some(changes) = &mut self.changes {
            changes.deleted.insert(path.to_owned());
        }
    }

    /// What the file rows changed; nothing when untracked.
    fn into_changes(self) -> Changes {
        self.changes.unwrap_or_default()
    }

    /// The file's rows; its new `file_id`.
    fn insert(&mut self, wt: i64, rows: &FileRows) -> Result<i64, StoreError> {
        let file_id = self
            .insert_file
            .insert((
                wt,
                rows.path.as_str(),
                rows.blake3.as_deref(),
                rows.size,
                rows.read_error.as_deref(),
                rows.shell.as_deref(),
                i64::from(rows.tier3),
            ))
            .db()?;
        for (ord, node) in rows.nodes.iter().enumerate() {
            let node_id = self
                .insert_node
                .insert((
                    file_id,
                    ord_value(ord),
                    node.line,
                    node.id.as_deref(),
                    node.kind.as_deref(),
                    node.title.as_deref(),
                    node.parent_id.as_deref(),
                    node.own_text.as_str(),
                    node.norm_hash.as_deref(),
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
        for name in &rows.names {
            self.insert_name.execute((file_id, name.as_str())).db()?;
        }
        if let Some(changes) = &mut self.changes {
            changes.written.insert(rows.path.clone());
            changes.names.extend(rows.names.iter().cloned());
        }
        Ok(file_id)
    }
}

fn ord_value(ord: usize) -> i64 {
    i64::try_from(ord).unwrap_or(i64::MAX)
}
