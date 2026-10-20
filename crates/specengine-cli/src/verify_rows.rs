//! `spec verify`'s judge over a code tree already walked
//! (canon `spec-verify`, "Rows", "States", "Cascade", "Base"): the
//! rows of one side (the working tree, or a commit), each judged by core's pure [`judge_row`] over facts gathered
//! here, and, given a base, which rows are Pre-existing. It never calls
//! [`crate::code_walk::read_tree`]: the caller walks once and hands the
//! tree in ([`judge_walk`]); a commit's side is read by [`read_head`],
//! which takes the walk to reuse.
//!
//! One commit alone ([`read_commit`], [`judge_commit`]; task spec
//! `drift-queue`, `spec propose drift`, and `spec verify --staged`'s
//! seam): its documents, config, code files, manifests and `spec.lock`
//! read from the commit the caller names (never `HEAD` by name), its code
//! tree bound from its blobs with nothing reused, judged as the base is;
//! no working-tree file is read. `spec verify --staged` (task spec
//! `rev-rule`) feeds it the git index's listing ([`CommitConfig::of_index`]:
//! its blob causes (3), not (7)) and gives [`judge_commit`] `HEAD`'s side,
//! read by [`read_head`] over the index's side ([`IndexShare`]): a (path,
//! OID) the index shares is neither read nor parsed again.
//!
//! - **Rows**: one per lock key, from a lock entry or a marker binding;
//!   markers sharing a key are one row. Keys come from accept's own
//!   builder ([`Target::key`]): the canonical cited ID, never an alias; an
//!   unresolved ID (ambiguous, look-alike, dangling) keys on its ID part as
//!   written, its `@rev` left out. A
//!   keyless marker is one row per node, relation, file, state and
//!   category. `queue` and `other_project` IDs are skipped and counted.
//! - **Spec side**: R by [`revision`], H the `norm_hash` of a fresh parse
//!   of the bytes read (`norm_input`, `b3_hash`), never the index's
//!   column.
//! - **Cascade** (canon `spec-verify`, "States", "Cascade"): a bound row
//!   with an entry (lock format 2, the only one a code judge reads) is
//!   judged by its levels, L_lock the entry's, L_mark its markers' united,
//!   C their union without `path`. A marker's list malformed: not
//!   compared (`bad_marker bad_levels`). Then the file's blob OID (on disk
//!   one `hash-object` for every such file; at a commit its tree's OID);
//!   equal: nothing parsed, only the two sets compared. C empty: nothing
//!   parsed or compared. Else C's digests made by accept's own functions,
//!   each changed file parsed once from the walk's text: an item by
//!   `item_levels` at its index, an inline `mod` at its `mod_item`'s, the
//!   file's module by `module_levels`, a RON value by `value_hashes` paired
//!   by its comment's range, rendered by `lock.rs`' `level_code`; core's
//!   `changed_levels` names those that differ. The judge gets the outcome;
//!   the row keeps the levels judged, the current fact a changed digest was
//!   found by (`<level>:<digest>` per changed level, never a blob OID) and
//!   the node's H.
//! - **Anchors** (canon `spec-verify-anchors`, "The pairing",
//!   "The merged row"): on each side alone, an item's or a module's entry
//!   no marker gives any more (`file_gone`, `symbol_gone`) is paired by
//!   core's [`pair`] with a lockable marker binding of its node, relation,
//!   kind and item kind that has no entry, by the entry's fingerprints
//!   against the binding's made now, in the cascade's one parse; a binding
//!   in a file whose tree has an error is no candidate. A mutual unique
//!   match makes one row at the binding's key judged against the entry
//!   (`path` changed when either side names it). Nothing is written.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use specengine_code::{
    HashState, LevelState, MOD_KIND, RonValues, RustParser, TRIVIAL_BODY_BYTES, item_levels,
    markers_in, module_levels, value_hashes,
};
use specengine_core::anchor::{Follow, Print, pair};
use specengine_core::check::{CheckFile, CheckInput, Mode, canonical_id};
use specengine_core::lock::{
    EntryCode, EntryKey, LOCK_FILE, LevelCode, Lock, LockKind, LockLevel, LockRelation, Verified,
    key_problem, parse_lock,
};
use specengine_core::verify::{
    CodeFact, Compared, Comparison, FILE_GONE, Judgement, LevelDigests, MarkerFact, MarkerRev,
    NodeFact, RowBinding, RowFacts, RowKey, RowLevels, SYMBOL_GONE, State, Unbound, changed_levels,
    is_pre_existing, judge_row, verify_from_toml,
};
use specengine_core::{CodeConfig, norm_input, revision};
use specengine_store::{CommitEntryKind, CommitFile, CommitListing, VerifyGit, b3_hash};

use crate::code_walk::{CodeTree, MANIFEST, ManifestSeen, RustFile, tree_of_blobs};
use crate::lock::{
    Hashed, Target, build_recipes, is_code_path, is_manifest, level_code, lock_level, relation_of,
};
use crate::project::{CONFIG_FILE, ProjectRoot};
use crate::refs::{CANNOT_VERIFY, Corpus, IdState, NodeKey, UNANCHORED};

/// Run causes, by their place in "Verdict and exit"'s order.
pub(crate) const CAUSE_CONFIG: u8 = 1;
pub(crate) const CAUSE_NO_CODE: u8 = 2;
pub(crate) const CAUSE_WALK: u8 = 3;
pub(crate) const CAUSE_GIT: u8 = 4;
pub(crate) const CAUSE_LOCK: u8 = 6;
pub(crate) const CAUSE_BASE: u8 = 7;

/// A cause that stops the run (exit 2), with its place in the order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunCause {
    pub order: u8,
    pub message: String,
}

impl RunCause {
    pub(crate) fn new(order: u8, message: impl Into<String>) -> Self {
        Self {
            order,
            message: message.into(),
        }
    }
}

/// What a caller has already read: the project, its `[code]`, the spec
/// corpus as read now (every file read again: the spec side's bytes), the
/// walked code tree and the lock, parsed.
pub(crate) struct Walked<'a> {
    pub project: &'a ProjectRoot,
    pub code: &'a CodeConfig,
    pub input: &'a CheckInput,
    pub tree: &'a CodeTree,
    pub lock: &'a Lock,
}

/// `HEAD`'s mode, as its config gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadMode {
    /// No config at `HEAD`, or no base.
    Absent,
    Known(Mode),
    /// Why it is not known.
    Unknown(String),
}

/// The base: a commit's side, read by [`read_head`]; `listing` `None`:
/// unborn, every row introduced.
pub(crate) struct HeadSide {
    listing: Option<CommitListing>,
    input: CheckInput,
    /// `None`: the working tree's walk, equal to the commit's.
    tree: Option<CodeTree>,
    /// The commit's lock, parsed (absent: empty); the judged lock when
    /// `HEAD`'s is only unrebased ([`HeadSide::lock_note`]).
    pub lock: Lock,
    pub mode: HeadMode,
    /// `HEAD`'s lock gave way to the judged one: the note saying so
    /// (stderr and `--json` `notes`).
    pub lock_note: Option<String>,
}

impl HeadSide {
    /// `HEAD` names no commit.
    pub(crate) fn is_unborn(&self) -> bool {
        self.listing.is_none()
    }
}

/// One judged row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub key: RowKey,
    pub judgement: Judgement,
    pub pre_existing: bool,
    pub markers: Vec<MarkerFact>,
    /// R; `None` unknown or no node.
    pub node_rev: Option<u32>,
    /// H, the node's `norm_hash` of the side's bytes; `None` null or no
    /// node.
    pub norm_hash: Option<String>,
    pub entry: Option<Verified>,
    /// The row names one node.
    pub resolved: bool,
    /// The levels judged; `None` without an entry or a bound marker, or
    /// with a marker's list malformed.
    pub levels: Option<JudgedLevels>,
    /// The cascade's current fact on a row whose digests were compared
    /// (`compared` `ast`) and found changed, any kind and state:
    /// `<level>:<digest>` per changed level, this run's digest, `,`-joined
    /// in canonical order (`body:b3:…,deps:b3:…`); else `None`. Never a
    /// blob OID.
    pub code: Option<String>,
    /// Where its binding is; `None` without an entry or when the row
    /// cannot be verified.
    pub anchor: Option<Anchor>,
}

/// A bound row's levels as judged: L_lock, L_mark and, when C's digests
/// were compared (`compared` `ast`), those that changed, in canonical
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JudgedLevels {
    pub lock: BTreeSet<LockLevel>,
    pub markers: BTreeSet<LockLevel>,
    pub changed: Option<Vec<LockLevel>>,
}

/// The rows of a walk, in key order, and the markers skipped.
pub(crate) struct Judged {
    pub rows: Vec<Row>,
    pub skipped: usize,
}

/// The judge over an already walked tree: the working tree's rows against
/// its lock, and, given `base` (a commit's side), each row Pre-existing
/// when the commit gives the same key, state and category. Git: one
/// `hash-object` for the bound files whose rows have an entry.
pub(crate) fn judge_walk(
    walked: &Walked<'_>,
    git: &VerifyGit,
    base: Option<&HeadSide>,
) -> Result<Judged, RunCause> {
    let current = Side {
        input: walked.input,
        tree: walked.tree,
        lock: walked.lock,
        files: Files::Disk {
            root: &walked.project.root,
            git,
        },
    };
    let (mut rows, skipped) = judge_side(&current, walked.project)?;
    mark_pre_existing(&mut rows, base, walked.tree, walked.project)?;
    Ok(Judged { rows, skipped })
}

/// Given `base` (a commit's side; unborn: none), each of `rows` is
/// Pre-existing when the commit gives the same key, state and category;
/// `tree` is the base's code tree when it reused the current one.
fn mark_pre_existing(
    rows: &mut [Row],
    base: Option<&HeadSide>,
    tree: &CodeTree,
    project: &ProjectRoot,
) -> Result<(), RunCause> {
    if let Some(head) = base
        && let Some(listing) = &head.listing
    {
        let side = Side {
            input: &head.input,
            tree: head.tree.as_ref().unwrap_or(tree),
            lock: &head.lock,
            files: Files::Commit(listing),
        };
        let (head_rows, _) = judge_side(&side, project)?;
        let at_head: BTreeMap<RowKey, Judgement> = head_rows
            .into_iter()
            .map(|row| (row.key, row.judgement))
            .collect();
        for row in rows {
            row.pre_existing = is_pre_existing(&row.key, &row.judgement, &at_head);
        }
    }
    Ok(())
}

// ----------------------------------------------------------------- sides

/// Where a side's files are: on disk (hashed by git), or in a commit.
enum Files<'a> {
    Disk { root: &'a Path, git: &'a VerifyGit },
    Commit(&'a CommitListing),
}

/// One side to judge.
struct Side<'a> {
    input: &'a CheckInput,
    tree: &'a CodeTree,
    lock: &'a Lock,
    files: Files<'a>,
}

/// Which node a row's markers name.
#[derive(Debug, Clone, Copy)]
enum NodeRef {
    Found(NodeKey),
    Ambiguous,
    Homoglyph,
    Dangling,
}

/// What a row's markers bind.
#[derive(Debug, Clone)]
enum CodeRef {
    /// A key's binding, not ambiguous: the unit its digests are made of.
    Bound(Unit),
    Ambiguous(&'static str),
    Unanchored(&'static str),
    ItemError(&'static str),
}

/// A bound unit, one variant per kind, as accept hashes it.
#[derive(Debug, Clone)]
enum Unit {
    /// An item: its Rust file's index in the walk and its own.
    Item { file: usize, index: usize },
    /// An inline `mod`: its Rust file's index and its `mod_item`'s.
    InlineMod { file: usize, index: usize },
    /// A file's module: its Rust file's index.
    FileModule { file: usize },
    /// A RON value: its RON file's index and its comment's byte range, by
    /// which its hash is paired.
    Ron { file: usize, comment: Range<usize> },
}

/// A row before it is judged.
struct Building {
    node: Option<NodeRef>,
    code: Option<CodeRef>,
    markers: Vec<MarkerFact>,
    /// L_mark: the markers' effective levels united (a malformed list adds
    /// none; its marker's `bad_levels` stops the comparison).
    levels: BTreeSet<LockLevel>,
    entry: Option<Verified>,
}

impl Building {
    fn new() -> Self {
        Self {
            node: None,
            code: None,
            markers: Vec::new(),
            levels: BTreeSet::new(),
            entry: None,
        }
    }
}

/// Every row of one side, judged, and the markers skipped.
fn judge_side(side: &Side<'_>, project: &ProjectRoot) -> Result<(Vec<Row>, usize), RunCause> {
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(side.input, scheme, &project.config.paths);
    let mut building: BTreeMap<RowKey, Building> = BTreeMap::new();
    let mut skipped = 0;
    let mut keyless_broken: BTreeSet<(String, &'static str, String)> = BTreeSet::new();

    for (index, file) in side.tree.rust.iter().enumerate() {
        let names = side
            .tree
            .names
            .items
            .get(index)
            .map_or(&[][..], Vec::as_slice);
        let module = side.tree.names.modules.get(index);
        let lines = LineStarts::new(&file.text);
        for comment in &file.binding.comments {
            let Some(text) = file.text.get(comment.range.clone()) else {
                continue;
            };
            let markers = markers_in(text);
            if markers.is_empty() {
                continue;
            }
            let target = Target::of_rust(comment.binding, &file.binding.analysis, names, module);
            let code_ref = code_ref(&target, || match &target {
                Target::Item { index: item, .. } => Unit::Item {
                    file: index,
                    index: *item,
                },
                Target::Module {
                    item: Some(item), ..
                } => Unit::InlineMod {
                    file: index,
                    index: *item,
                },
                // The file's module: a Rust comment binds no RON value.
                _ => Unit::FileModule { file: index },
            });
            for marker in &markers {
                let found = MarkerAt {
                    marker,
                    comment: text,
                    line: lines.line(comment.range.start + marker.offset),
                    file: &file.path,
                };
                if let Some((row_key, node)) = marker_row(&mut corpus, project, &found, &target) {
                    if matches!(&row_key.binding, RowBinding::Keyless { state, .. } if state == CANNOT_VERIFY)
                    {
                        keyless_broken.insert((
                            row_key.node.clone(),
                            row_key.relation.as_str(),
                            row_key.file.clone(),
                        ));
                    }
                    add(&mut building, row_key, node, code_ref.clone(), &found);
                } else {
                    skipped += 1;
                }
            }
        }
    }
    for (index, file) in side.tree.ron.iter().enumerate() {
        let lines = LineStarts::new(&file.text);
        for anchor in &file.anchors {
            let Some(text) = file.text.get(anchor.comment.clone()) else {
                continue;
            };
            let markers = markers_in(text);
            if markers.is_empty() {
                continue;
            }
            let target = Target::of_ron(&anchor.anchor);
            let code_ref = code_ref(&target, || Unit::Ron {
                file: index,
                comment: anchor.comment.clone(),
            });
            for marker in &markers {
                let found = MarkerAt {
                    marker,
                    comment: text,
                    line: lines.line(anchor.comment.start + marker.offset),
                    file: &file.path,
                };
                if let Some((row_key, node)) = marker_row(&mut corpus, project, &found, &target) {
                    add(&mut building, row_key, node, code_ref.clone(), &found);
                } else {
                    skipped += 1;
                }
            }
        }
    }
    for (key, verified) in &side.lock.entries {
        building
            .entry(RowKey::of_entry(key))
            .or_insert_with(Building::new)
            .entry = Some(verified.clone());
    }

    // Each row's node as the spec reads it now. A bound row waits for the
    // cascade; every other row, an entry alone too, is judged at once.
    let canonical = canonical_nodes(&corpus, project);
    let mut spec_side: BTreeMap<NodeKey, (Option<u32>, Option<String>)> = BTreeMap::new();
    let mut rows = Vec::with_capacity(building.len());
    let mut waiting: Vec<(&RowKey, &Building, NodeSide)> = Vec::new();
    for (key, row) in &building {
        let node = node_of(row.node, key, &canonical, &corpus, &mut spec_side);
        let code = match &row.code {
            Some(CodeRef::Bound(_)) => {
                waiting.push((key, row, node));
                continue;
            }
            Some(CodeRef::Ambiguous(reason)) => CodeFact::Ambiguous((*reason).to_owned()),
            Some(CodeRef::Unanchored(category)) => CodeFact::Unanchored((*category).to_owned()),
            Some(CodeRef::ItemError(category)) => CodeFact::ItemError((*category).to_owned()),
            None => CodeFact::Unbound(unbound(side, key, &keyless_broken)),
        };
        let facts = RowFacts {
            key: key.clone(),
            node: node.fact,
            entry: row.entry.clone(),
            markers: row.markers.clone(),
            code,
        };
        rows.push(judged(facts, node.rev, node.hash, None, node.resolved));
    }

    // The pairing (canon `spec-verify-anchors`, "The pairing"): E, the
    // entries alone the judge reads `file_gone` or `symbol_gone`; B, the
    // bindings of the non-trivial E's groups with no entry, hashed with the
    // cascade's units in one pass; no non-trivial E, nothing beyond the
    // cascade's is hashed. A trivial E still goes to `pair`: `lost`.
    let lone: Vec<(usize, Print<'_, Group<'_>>)> = rows
        .iter()
        .enumerate()
        .filter_map(|(at, row)| lone_print(row).map(|print| (at, print)))
        .collect();
    let lone_keys: Vec<&RowKey> = lone
        .iter()
        .filter(|(_, print)| print.body_bytes >= TRIVIAL_BODY_BYTES)
        .map(|(at, _)| &rows[*at].key)
        .collect();
    let pairable = select_bindings(&lone_keys, &building);
    let units: Vec<&Unit> = pairable
        .iter()
        .filter_map(
            |key| match building.get(*key).and_then(|row| row.code.as_ref()) {
                Some(CodeRef::Bound(unit)) => Some(unit),
                _ => None,
            },
        )
        .collect();
    let (mut comparisons, made) = cascade(side, &building, &units)?;
    // Each binding of B as accept would record it now; one refused, not
    // hashed, or in a file whose tree has an error (`ERROR` or `MISSING`:
    // its place there is the parser's recovery, never followed) is no
    // candidate.
    let now: Vec<(&RowKey, LevelCode)> = pairable
        .iter()
        .filter_map(|key| {
            let row = building.get(*key)?;
            let Some(CodeRef::Bound(unit)) = &row.code else {
                return None;
            };
            if made.in_broken_file(unit) {
                return None;
            }
            match digests_of(&made, unit) {
                Ok(hashed @ Hashed::Levels(_)) => {
                    Some((*key, level_code(row.levels.clone(), &hashed)))
                }
                _ => None,
            }
        })
        .collect();
    let prints: Vec<(&RowKey, &LevelCode, Print<'_, Group<'_>>)> = now
        .iter()
        .filter_map(|(key, code)| binding_print(key, code).map(|print| (*key, code, print)))
        .collect();
    let entries: Vec<Print<'_, Group<'_>>> = lone.iter().map(|(_, print)| *print).collect();
    let bindings: Vec<Print<'_, Group<'_>>> = prints.iter().map(|(_, _, print)| *print).collect();
    let follows = pair(&entries, &bindings, TRIVIAL_BODY_BYTES);
    // b's key -> (e's row, b's code now); e's row -> its candidates' keys.
    let mut merged: BTreeMap<&RowKey, (usize, &LevelCode)> = BTreeMap::new();
    let mut contested: BTreeMap<usize, Vec<EntryKey>> = BTreeMap::new();
    for ((at, _), follow) in lone.iter().zip(follows) {
        match follow {
            Follow::Moved(binding) => {
                if let Some((key, code, _)) = prints.get(binding) {
                    merged.insert(*key, (*at, *code));
                }
            }
            Follow::Ambiguous(candidates) => {
                let keys = candidates
                    .iter()
                    .filter_map(|binding| prints.get(*binding))
                    .filter_map(|(key, _, _)| key.entry_key())
                    .collect();
                contested.insert(*at, keys);
            }
            Follow::Lost => {}
        }
    }

    let mut judged_bound = Vec::with_capacity(waiting.len());
    let mut gone: BTreeSet<usize> = BTreeSet::new();
    for (key, row, node) in waiting {
        // The merged row: b's key and markers judged against e's entry by
        // b's digests, no blob step.
        let moved = merged.get(key).and_then(|(at, now)| {
            let lone_row = &rows[*at];
            let entry = lone_row.entry.as_ref()?;
            let EntryCode::Levels(held) = &entry.code else {
                return None;
            };
            let cascaded = moved_cascade(held, now, &row.levels);
            Some((*at, entry, cascaded, lone_row.key.entry_key()))
        });
        let (entry, cascaded, from) = match moved {
            Some((at, entry, cascaded, from)) => {
                gone.insert(at);
                (Some(entry.clone()), Some(cascaded), from)
            }
            None => (row.entry.clone(), comparisons.remove(key), None),
        };
        let comparison = cascaded
            .as_ref()
            .map_or(Comparison::NotHashed, |found| found.comparison.clone());
        let facts = RowFacts {
            key: key.clone(),
            node: node.fact,
            entry,
            markers: row.markers.clone(),
            code: CodeFact::Bound(comparison),
        };
        let mut judged_row = judged(facts, node.rev, node.hash, cascaded, node.resolved);
        if let Some(from) = from
            && judged_row.anchor.is_some()
        {
            judged_row.anchor = Some(Anchor::Moved(from));
        }
        judged_bound.push(judged_row);
    }

    // e's row is gone once merged; a contested one names its candidates.
    for (at, keys) in contested {
        if let Some(row) = rows.get_mut(at)
            && row.anchor.is_some()
        {
            row.anchor = Some(Anchor::Ambiguous(keys));
        }
    }
    let mut rows: Vec<Row> = rows
        .into_iter()
        .enumerate()
        .filter(|(at, _)| !gone.contains(at))
        .map(|(_, row)| row)
        .chain(judged_bound)
        .collect();
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    Ok((rows, skipped))
}

/// A row's node as the spec reads it now: R, H, and whether it names one
/// node.
struct NodeSide {
    fact: NodeFact,
    rev: Option<u32>,
    hash: Option<String>,
    resolved: bool,
}

/// The node of a row: the one its markers name (`named`), else the one
/// whose canonical ID is its entry's; none such: gone.
fn node_of(
    named: Option<NodeRef>,
    key: &RowKey,
    canonical: &BTreeMap<String, Vec<NodeKey>>,
    corpus: &Corpus<'_>,
    spec_side: &mut BTreeMap<NodeKey, (Option<u32>, Option<String>)>,
) -> NodeSide {
    let node = match named {
        Some(node) => node,
        None => match canonical.get(&key.node).map(Vec::as_slice) {
            Some([node]) => NodeRef::Found(*node),
            Some([_, _, ..]) => NodeRef::Ambiguous,
            _ => {
                // No node has the entry's ID.
                return NodeSide {
                    fact: NodeFact::Gone,
                    rev: None,
                    hash: None,
                    resolved: false,
                };
            }
        },
    };
    let (fact, rev, hash) = match node {
        NodeRef::Found(at) => {
            let (rev, hash) = spec_side
                .entry(at)
                .or_insert_with(|| node_side(corpus, at))
                .clone();
            let norm_hash = hash.clone();
            (NodeFact::Found { rev, hash }, rev, norm_hash)
        }
        NodeRef::Ambiguous => (NodeFact::Ambiguous, None, None),
        NodeRef::Homoglyph => (NodeFact::Homoglyph, None, None),
        NodeRef::Dangling => (NodeFact::Dangling, None, None),
    };
    NodeSide {
        fact,
        rev,
        hash,
        resolved: matches!(node, NodeRef::Found(_)),
    }
}

// --------------------------------------------------------------- anchors

/// Where a row's binding is (canon `spec-verify-anchors`,
/// "Anchors per row"); a row without an entry, or one that cannot be
/// verified, has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Anchor {
    /// An entry and a marker binding at one key.
    Exact,
    /// The merged row: the key of the entry it was judged against.
    Moved(EntryKey),
    /// An entry alone whose match is not unique (several bindings, or one
    /// another entry matches too): the bindings' keys, in key order.
    Ambiguous(Vec<EntryKey>),
    /// Every other entry with no binding at its key.
    Lost,
}

/// A row's anchor before the pairing: `exact` with an entry and a marker,
/// `lost` with an entry alone, none without an entry or when the row
/// cannot be verified.
fn first_anchor(judged: &Judgement, entry: bool, bound: bool) -> Option<Anchor> {
    if !entry || judged.state == State::CannotVerify {
        None
    } else if bound {
        Some(Anchor::Exact)
    } else {
        Some(Anchor::Lost)
    }
}

/// A pairing group (canon `spec-verify-anchors`, "The pairing"): node,
/// relation, kind, item kind; opaque to core.
type Group<'a> = (&'a str, LockRelation, LockKind, Option<&'a str>);

/// A keyed row's group; `None` for a keyless one.
fn group_of(key: &RowKey) -> Option<Group<'_>> {
    match &key.binding {
        RowBinding::Keyed {
            kind, item_kind, ..
        } => Some((key.node.as_str(), key.relation, *kind, item_kind.as_deref())),
        RowBinding::Keyless { .. } => None,
    }
}

/// E's print: an entry alone (no marker at its key), of an item or a
/// module, the judge reading `broken_binding file_gone|symbol_gone`; its
/// entry's `body` digest and fingerprints. Any other row: `None` (a gone
/// node, a marker gone, a RON value, a fact that cannot be verified).
fn lone_print(row: &Row) -> Option<Print<'_, Group<'_>>> {
    let judged = &row.judgement;
    let gone = judged.state == State::BrokenBinding
        && matches!(judged.category.as_deref(), Some(FILE_GONE | SYMBOL_GONE));
    if !gone || !row.markers.is_empty() {
        return None;
    }
    let group = group_of(&row.key)?;
    if !matches!(group.2, LockKind::Item | LockKind::Module) {
        return None;
    }
    let EntryCode::Levels(held) = &row.entry.as_ref()?.code else {
        return None;
    };
    let fingerprints = held.fingerprints.as_ref()?;
    Some(Print {
        group,
        file: &row.key.file,
        body: &held.body_hash,
        skeleton: &fingerprints.skeleton_hash,
        quote: &fingerprints.quote_hash,
        body_bytes: fingerprints.body_bytes,
    })
}

/// B's print: a binding's `body` digest and fingerprints as accept would
/// record them now.
fn binding_print<'a>(key: &'a RowKey, now: &'a LevelCode) -> Option<Print<'a, Group<'a>>> {
    let fingerprints = now.fingerprints.as_ref()?;
    Some(Print {
        group: group_of(key)?,
        file: &key.file,
        body: &now.body_hash,
        skeleton: &fingerprints.skeleton_hash,
        quote: &fingerprints.quote_hash,
        body_bytes: fingerprints.body_bytes,
    })
}

/// The bindings B is drawn from (canon `spec-verify-anchors`,
/// "The pairing"), pure: of `building`, the keys of the rows with no entry
/// bound to an item, an inline `mod` or a file module, no marker's list
/// malformed, no `key_problem`, whose group holds a key of `lone` (the
/// non-trivial E's); in key order. `lone` empty: none.
fn select_bindings<'a>(
    lone: &[&RowKey],
    building: &'a BTreeMap<RowKey, Building>,
) -> Vec<&'a RowKey> {
    let groups: BTreeSet<Group<'_>> = lone.iter().filter_map(|key| group_of(key)).collect();
    if groups.is_empty() {
        return Vec::new();
    }
    building
        .iter()
        .filter(|(key, row)| {
            row.entry.is_none()
                && group_of(key).is_some_and(|group| groups.contains(&group))
                && matches!(
                    row.code,
                    Some(CodeRef::Bound(
                        Unit::Item { .. } | Unit::InlineMod { .. } | Unit::FileModule { .. }
                    ))
                )
                && !row.markers.iter().any(|marker| marker.bad_levels)
                && key
                    .entry_key()
                    .is_some_and(|entry| key_problem(&entry).is_none())
        })
        .map(|(key, _)| key)
        .collect()
}

/// The merged row's cascade: b's digests `now` (rendered over L_mark,
/// `markers`) against the entry's `held` over C, `path` changed first when
/// L_lock or L_mark names it (the binding moved); its current fact the
/// digest levels' only.
fn moved_cascade(held: &LevelCode, now: &LevelCode, markers: &BTreeSet<LockLevel>) -> Cascaded {
    let levels = RowLevels {
        lock: held.levels.clone(),
        markers: markers.clone(),
    };
    let mut changed = Vec::new();
    if levels.lock.contains(&LockLevel::Path) || levels.markers.contains(&LockLevel::Path) {
        changed.push(LockLevel::Path);
    }
    let digests = LevelDigests::of(now);
    changed.extend(changed_levels(&levels, &LevelDigests::of(held), &digests));
    let code = digest_fact(&changed, &digests);
    Cascaded {
        comparison: Comparison::Digests {
            levels: levels.clone(),
            changed,
        },
        levels,
        code,
    }
}

/// A row's current fact: `<level>:<digest>` per changed level with a
/// digest, this run's, `,`-joined in canonical order; `None` when no such
/// level changed (`path` alone has no digest).
fn digest_fact(changed: &[LockLevel], now: &LevelDigests<'_>) -> Option<String> {
    let parts: Vec<String> = changed
        .iter()
        .filter_map(|level| {
            now.level(*level)
                .map(|digest| format!("{}:{digest}", level.as_str()))
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(","))
}

/// A row judged; `cascaded` its cascade's outcome when it was bound and
/// had an entry to compare. Its changed levels and current fact are kept
/// only when the judge compared C's digests (`compared` `ast`): a state
/// decided before that compares nothing.
fn judged(
    facts: RowFacts,
    node_rev: Option<u32>,
    norm_hash: Option<String>,
    cascaded: Option<Cascaded>,
    resolved: bool,
) -> Row {
    let judgement = judge_row(&facts);
    let anchor = first_anchor(&judgement, facts.entry.is_some(), !facts.markers.is_empty());
    let by_digests = judgement.compared == Some(Compared::Ast);
    let (levels, code) = match cascaded {
        Some(Cascaded {
            comparison,
            levels,
            code,
        }) => {
            let changed = match comparison {
                Comparison::Digests { changed, .. } if by_digests => Some(changed),
                _ => None,
            };
            let levels = JudgedLevels {
                lock: levels.lock,
                markers: levels.markers,
                changed,
            };
            (Some(levels), code.filter(|_| by_digests))
        }
        None => (None, None),
    };
    Row {
        key: facts.key,
        judgement,
        pre_existing: false,
        markers: facts.markers,
        node_rev,
        norm_hash,
        entry: facts.entry,
        resolved,
        levels,
        code,
        anchor,
    }
}

/// One marker and where it lies.
struct MarkerAt<'a> {
    marker: &'a specengine_code::Marker,
    /// The comment's text.
    comment: &'a str,
    line: usize,
    file: &'a str,
}

/// The row a marker gives and the node it names; `None`: skipped (a
/// `queue` or `other_project` ID).
fn marker_row(
    corpus: &mut Corpus<'_>,
    project: &ProjectRoot,
    found: &MarkerAt<'_>,
    target: &Target,
) -> Option<(RowKey, NodeRef)> {
    let judged = corpus.judge(&found.marker.id);
    // An unresolved ID keys on its ID part as written, never its `@rev`:
    // `ID@2` and `ID@3` on one item are one row, and a gone node's leftover
    // marker shares the row of its entry.
    let written = found.marker.id.as_str();
    let (node, node_ref) = match judged.state {
        IdState::Queue | IdState::OtherProject => return None,
        IdState::Resolved | IdState::Alias => match judged.nodes.first() {
            Some(&at) => (
                node_id(corpus, project, at).unwrap_or_else(|| written.to_owned()),
                NodeRef::Found(at),
            ),
            None => (written.to_owned(), NodeRef::Dangling),
        },
        IdState::Ambiguous => (written.to_owned(), NodeRef::Ambiguous),
        IdState::Homoglyph => (written.to_owned(), NodeRef::Homoglyph),
        IdState::Dangling => (written.to_owned(), NodeRef::Dangling),
    };
    let relation = relation_of(found.marker);
    let key = match target.key(&node, relation, found.file) {
        Ok(key) => RowKey::of_entry(&key),
        Err(place) => RowKey {
            node,
            relation,
            file: place.file,
            binding: RowBinding::Keyless {
                state: place.kind,
                category: place.symbol,
            },
        },
    };
    Some((key, node_ref))
}

/// A marker added to its row.
fn add(
    building: &mut BTreeMap<RowKey, Building>,
    key: RowKey,
    node: NodeRef,
    code: CodeRef,
    found: &MarkerAt<'_>,
) {
    let row = building.entry(key).or_insert_with(Building::new);
    if row.node.is_none() {
        row.node = Some(node);
    }
    // An ambiguous binding of any marker makes the row's.
    match (&row.code, &code) {
        (None, _) | (Some(CodeRef::Bound(_)), CodeRef::Ambiguous(_)) => row.code = Some(code),
        _ => {}
    }
    let marker = found.marker;
    let token = marker.token(found.comment);
    let rev = if token.len() == marker.id.len() {
        MarkerRev::Unversioned
    } else {
        marker.rev.map_or(MarkerRev::Bad, MarkerRev::Written)
    };
    // Its effective levels (no list: `sig`, `body`) join L_mark; a
    // malformed list has none.
    let effective = marker.levels.effective();
    if let Some(levels) = effective {
        row.levels.extend(levels.iter().copied().map(lock_level));
    }
    row.markers.push(MarkerFact {
        line: found.line,
        rev,
        bad_levels: effective.is_none(),
    });
}

/// What a target gives a row's code side: its ambiguity, a keyless
/// binding's category, else the unit `unit` names.
fn code_ref(target: &Target, unit: impl FnOnce() -> Unit) -> CodeRef {
    if let Some(reason) = target.ambiguity() {
        return CodeRef::Ambiguous(reason);
    }
    match target {
        Target::Item { .. } | Target::Module { .. } | Target::Ron { .. } => CodeRef::Bound(unit()),
        Target::Keyless { state, category } if *state == UNANCHORED => {
            CodeRef::Unanchored(category)
        }
        Target::Keyless { category, .. } => CodeRef::ItemError(category),
    }
}

/// The canonical cited ID of the node at `at`; `None` when it has no ID.
pub(crate) fn node_id(corpus: &Corpus<'_>, project: &ProjectRoot, at: NodeKey) -> Option<String> {
    let path = corpus.resolver.paths().get(at.file)?;
    let id = corpus
        .files
        .get(at.file)?
        .parsed
        .as_ref()?
        .nodes
        .get(at.ord)?
        .id
        .as_deref()?;
    Some(canonical_id(
        &corpus.resolver,
        &project.config.scheme,
        path,
        id,
        None,
    ))
}

/// Every node with an ID, by its canonical cited ID.
fn canonical_nodes(corpus: &Corpus<'_>, project: &ProjectRoot) -> BTreeMap<String, Vec<NodeKey>> {
    let mut nodes: BTreeMap<String, Vec<NodeKey>> = BTreeMap::new();
    for (file, checked) in corpus.files.iter().enumerate() {
        let Some(parsed) = &checked.parsed else {
            continue;
        };
        for ord in 0..parsed.nodes.len() {
            let at = NodeKey { file, ord };
            if let Some(id) = node_id(corpus, project, at) {
                nodes.entry(id).or_default().push(at);
            }
        }
    }
    nodes
}

/// R and H of the node at `at`: its own revision, and the `norm_hash` of
/// the parse of the bytes read.
fn node_side(corpus: &Corpus<'_>, at: NodeKey) -> (Option<u32>, Option<String>) {
    let Some(file) = corpus.files.get(at.file) else {
        return (None, None);
    };
    let Some(parsed) = &file.parsed else {
        return (None, None);
    };
    let rev = revision(parsed, at.ord).number();
    let hash = norm_input(parsed, &file.bytes, at.ord).map(|input| b3_hash(&input));
    (rev, hash)
}

/// Why no marker gives an entry's key now.
fn unbound(
    side: &Side<'_>,
    key: &RowKey,
    keyless_broken: &BTreeSet<(String, &'static str, String)>,
) -> Unbound {
    let RowBinding::Keyed {
        kind,
        symbol,
        item_kind,
    } = &key.binding
    else {
        return Unbound::MarkerGone;
    };
    let tree = side.tree;
    if let Ok(index) = tree
        .rust
        .binary_search_by(|file| file.path.as_str().cmp(&key.file))
    {
        let file = &tree.rust[index];
        let names = tree.names.items.get(index).map_or(&[][..], Vec::as_slice);
        let module = tree.names.modules.get(index);
        if *kind == LockKind::Module && module.is_some_and(|name| name.qpath == *symbol) {
            return Unbound::MarkerGone;
        }
        let wanted_kind = match kind {
            LockKind::Module => Some(MOD_KIND),
            _ => item_kind.as_deref(),
        };
        let item = file
            .binding
            .analysis
            .items
            .iter()
            .zip(names)
            .find(|(item, name)| name.qpath == *symbol && Some(item.kind.as_str()) == wanted_kind);
        return match item {
            Some((item, _)) if item.has_error => {
                // Accept's `keep_broken`: a keyless `cannot_verify` marker
                // of the entry's node and relation in the file.
                let marked = keyless_broken.contains(&(
                    key.node.clone(),
                    key.relation.as_str(),
                    key.file.clone(),
                ));
                if marked {
                    Unbound::ItemError(first_category(&item.state))
                } else {
                    Unbound::MarkerGone
                }
            }
            Some(_) => Unbound::MarkerGone,
            None if file.binding.analysis.orphan_errors > 0 => Unbound::HiddenByError,
            None => Unbound::SymbolGone,
        };
    }
    if tree
        .ron
        .binary_search_by(|file| file.path.as_str().cmp(&key.file))
        .is_ok()
    {
        // A RON value's presence is known only through a comment bound to
        // it: the marker is gone either way.
        return Unbound::MarkerGone;
    }
    let outside = || Unbound::Unreadable("outside_code".to_owned());
    let not_utf8 = || Unbound::Unreadable("not_utf8".to_owned());
    match &side.files {
        Files::Disk { root, .. } => match fs::symlink_metadata(root.join(&key.file)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Unbound::FileGone,
            Err(_) => Unbound::Unreadable("unreadable".to_owned()),
            Ok(meta) if meta.is_dir() => Unbound::FileGone,
            Ok(meta) if meta.is_file() && tree.not_utf8.contains(&key.file) => not_utf8(),
            // On disk, but no file the walk reads: outside `[code]`, a
            // symbolic link, or under one.
            Ok(_) => outside(),
        },
        Files::Commit(listing) => match listing.entry(&key.file) {
            None => Unbound::FileGone,
            Some(entry)
                if entry.kind == CommitEntryKind::Regular && tree.not_utf8.contains(&key.file) =>
            {
                not_utf8()
            }
            Some(_) => outside(),
        },
    }
}

/// The first of an item's error categories (`other` when none is named).
fn first_category(state: &HashState) -> String {
    match state {
        HashState::CannotVerify { categories } => categories
            .first()
            .map_or("other", |category| category.as_str())
            .to_owned(),
        HashState::Hashed(_) | HashState::NotHashed => "other".to_owned(),
    }
}

/// A bound row to compare: its key, its entry, the entry's format-2 code,
/// the levels it is judged by and the unit it binds.
struct Compare<'a> {
    key: &'a RowKey,
    entry: &'a Verified,
    held: &'a LevelCode,
    levels: RowLevels,
    unit: &'a Unit,
}

/// A bound row's cascade: its comparison, the levels it was judged by, and
/// the current fact a changed digest was found by (`<level>:<digest>` per
/// changed level, this run's digest).
#[derive(Debug, Clone)]
struct Cascaded {
    comparison: Comparison,
    levels: RowLevels,
    code: Option<String>,
}

/// What one side's changed files gave, each parsed or read once: an item's
/// or an inline `mod`'s levels by (Rust file, item index), a file
/// module's by Rust file, a RON file's value hashes by RON file; `broken`
/// the Rust files whose tree has an `ERROR` or `MISSING` node.
#[derive(Default)]
struct Made {
    items: BTreeMap<(usize, usize), Option<LevelState>>,
    modules: BTreeMap<usize, LevelState>,
    ron: BTreeMap<usize, RonValues>,
    broken: BTreeSet<usize>,
}

impl Made {
    /// Whether `unit` lies in a Rust file parsed here whose tree has an
    /// error; a RON value never does.
    fn in_broken_file(&self, unit: &Unit) -> bool {
        match unit {
            Unit::Item { file, .. } | Unit::InlineMod { file, .. } | Unit::FileModule { file } => {
                self.broken.contains(file)
            }
            Unit::Ron { .. } => false,
        }
    }
}

/// The cascade (canon `spec-verify`, "States"): each bound row with a
/// format-2 entry judged by its levels, the first step that applies. (1)
/// A marker's list malformed: not compared (the judge reads `bad_marker
/// bad_levels`). (2) The file's blob equals `file_blob`: nothing parsed,
/// only L_lock and L_mark compared. (3) C empty: nothing parsed or
/// compared. (4) C's digests made as accept makes them, each changed file
/// parsed once (an item and an inline `mod` by `item_levels`, the file's
/// module by `module_levels`, a RON value by `value_hashes` paired by its
/// comment's range), rendered by `level_code`, compared by core's
/// `changed_levels`; refused: the unit's first error category; no tree or
/// no RON pair: not hashed. `extra` (B's units) is hashed in the same pass;
/// what was made comes back with the comparisons.
fn cascade(
    side: &Side<'_>,
    building: &BTreeMap<RowKey, Building>,
    extra: &[&Unit],
) -> Result<(BTreeMap<RowKey, Cascaded>, Made), RunCause> {
    let compared: Vec<Compare<'_>> = building
        .iter()
        .filter_map(|(key, row)| {
            let (Some(CodeRef::Bound(unit)), Some(entry)) = (&row.code, &row.entry) else {
                return None;
            };
            // Format 2 is the only lock a code judge reads.
            let EntryCode::Levels(held) = &entry.code else {
                return None;
            };
            // Step 1.
            if row.markers.iter().any(|marker| marker.bad_levels) {
                return None;
            }
            Some(Compare {
                key,
                entry,
                held,
                levels: RowLevels {
                    lock: held.levels.clone(),
                    markers: row.levels.clone(),
                },
                unit,
            })
        })
        .collect();
    let files: BTreeSet<&str> = compared
        .iter()
        .map(|compare| compare.key.file.as_str())
        .collect();
    let blobs: BTreeMap<&str, String> = match &side.files {
        Files::Disk { git, .. } => {
            let paths: Vec<&str> = files.iter().copied().collect();
            let oids = git
                .hash_files(&paths)
                .map_err(|cause| RunCause::new(CAUSE_GIT, cause.message))?;
            paths.into_iter().zip(oids).collect()
        }
        Files::Commit(listing) => files
            .iter()
            .filter_map(|path| listing.entry(path).map(|entry| (*path, entry.oid)))
            .collect(),
    };

    // Steps 2 and 3; the rest wait for their digests.
    let mut out = BTreeMap::new();
    let mut hashed: Vec<Compare<'_>> = Vec::new();
    for compare in compared {
        let comparison = match blobs.get(compare.key.file.as_str()) {
            None => Comparison::NotHashed,
            Some(blob) if *blob == compare.entry.file_blob => {
                Comparison::Blob(compare.levels.clone())
            }
            Some(_) if compare.levels.compared().is_empty() => Comparison::PathOnly,
            Some(_) => {
                hashed.push(compare);
                continue;
            }
        };
        out.insert(
            compare.key.clone(),
            Cascaded {
                comparison,
                levels: compare.levels,
                code: None,
            },
        );
    }

    // Step 4: only the bound files whose blob changed are parsed or read
    // again (the walk keeps no tree), only their compared units hashed, B's
    // with them.
    let made = made(
        side,
        hashed
            .iter()
            .map(|compare| compare.unit)
            .chain(extra.iter().copied()),
    )?;
    for compare in hashed {
        let (comparison, code) = match digests_of(&made, compare.unit) {
            Ok(hashed) => {
                // As accept would record it now: L_mark and every digest.
                let rendered = level_code(compare.levels.markers.clone(), &hashed);
                let now = LevelDigests::of(&rendered);
                let changed =
                    changed_levels(&compare.levels, &LevelDigests::of(compare.held), &now);
                let code = digest_fact(&changed, &now);
                (
                    Comparison::Digests {
                        levels: compare.levels.clone(),
                        changed,
                    },
                    code,
                )
            }
            Err(Some(category)) => (Comparison::CannotVerify(category), None),
            Err(None) => (Comparison::NotHashed, None),
        };
        out.insert(
            compare.key.clone(),
            Cascaded {
                comparison,
                levels: compare.levels,
                code,
            },
        );
    }
    Ok((out, made))
}

/// The levels and value hashes step 4 needs: each Rust file of `units`
/// parsed once (its items' and inline `mod`s' levels in one call, its
/// module's once), each RON file's values hashed once, each Rust file whose
/// tree has an error noted. A Rust file the parser gives no tree yields
/// nothing.
fn made<'u>(side: &Side<'_>, units: impl IntoIterator<Item = &'u Unit>) -> Result<Made, RunCause> {
    let mut items: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut modules: BTreeSet<usize> = BTreeSet::new();
    let mut ron: BTreeSet<usize> = BTreeSet::new();
    for unit in units {
        match unit {
            Unit::Item { file, index } | Unit::InlineMod { file, index } => {
                items.entry(*file).or_default().insert(*index);
            }
            Unit::FileModule { file } => {
                modules.insert(*file);
            }
            Unit::Ron { file, .. } => {
                ron.insert(*file);
            }
        }
    }
    let mut made = Made::default();
    let rust: BTreeSet<usize> = items
        .keys()
        .copied()
        .chain(modules.iter().copied())
        .collect();
    if !rust.is_empty() {
        let mut parser = RustParser::new().map_err(|error| {
            RunCause::new(
                CAUSE_WALK,
                format!("the code walk is incomplete: the Rust grammar cannot be loaded: {error}"),
            )
        })?;
        for file in rust {
            let Some(walked) = side.tree.rust.get(file) else {
                continue;
            };
            let text = &walked.text;
            let Some(tree) = parser.parse(text) else {
                continue;
            };
            if tree.root_node().has_error() {
                made.broken.insert(file);
            }
            if let Some(indexes) = items.get(&file) {
                let indexes: Vec<usize> = indexes.iter().copied().collect();
                let levels = item_levels(&tree, text, &indexes);
                for (index, levels) in indexes.into_iter().zip(levels) {
                    made.items.insert((file, index), levels);
                }
            }
            if modules.contains(&file) {
                made.modules.insert(file, module_levels(&tree, text));
            }
        }
    }
    for file in ron {
        if let Some(walked) = side.tree.ron.get(file) {
            made.ron.insert(file, value_hashes(&walked.text));
        }
    }
    Ok(made)
}

/// A unit's digests as `made` holds them, as accept pairs them (canon
/// `code-hash-ron`, "Entry point"); `Err`: refused with its first error
/// category, or none made (no tree, no RON pair).
fn digests_of<'a>(made: &'a Made, unit: &Unit) -> Result<Hashed<'a>, Option<String>> {
    let levels = match unit {
        Unit::Item { file, index } | Unit::InlineMod { file, index } => {
            made.items.get(&(*file, *index)).and_then(Option::as_ref)
        }
        Unit::FileModule { file } => made.modules.get(file),
        Unit::Ron { file, comment } => {
            return match made.ron.get(file) {
                Some(RonValues::Hashed(values)) => values
                    .iter()
                    .find(|(range, _)| range == comment)
                    .map(|(_, digest)| Hashed::Ron(*digest))
                    .ok_or(None),
                Some(RonValues::CannotVerify(category)) => Err(Some((*category).to_owned())),
                None => Err(None),
            };
        }
    };
    match levels {
        Some(LevelState::Hashed(levels)) => Ok(Hashed::Levels(levels)),
        Some(LevelState::CannotVerify { categories }) => Err(Some(
            categories
                .first()
                .map_or("other", |category| category.as_str())
                .to_owned(),
        )),
        None => Err(None),
    }
}

/// Line starts of one text: a byte offset's 1-based line.
struct LineStarts(Vec<usize>);

impl LineStarts {
    fn new(text: &str) -> Self {
        Self(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(at, _)| at + 1)
                .collect(),
        )
    }

    fn line(&self, offset: usize) -> usize {
        self.0.partition_point(|&start| start <= offset) + 1
    }
}

// ------------------------------------------------------------------ base

/// The base: `HEAD`'s side read through `git`'s one session
/// (canon `spec-verify`, "Base"): its documents as `spec check
/// --changed`'s base lists them, the `.rs` and `.ron` files under the code
/// roots, every `Cargo.toml` under a root or at or above one, `spec.lock`
/// and the config (`config_path`, root-relative: its mode only). Judged
/// with the current config, scheme and `[code]`; a document's parse and a
/// code file's binding are reused when its bytes are equal, the whole
/// walk when every code file and every manifest the naming read is. An
/// unborn `HEAD`: no side, every row introduced. Causes: git's (4); a
/// listed object missing or no blob, `HEAD`'s lock invalid, a file the
/// parser fails on (7). `HEAD`'s lock only unrebased (format 1, another
/// `code_recipe`) beside a judged lock of this build's: `HEAD`'s rows are
/// judged with the judged lock, under a note ([`HeadSide::lock_note`]).
///
/// Over the index's side (`index`: `spec verify --staged`; `walked` its
/// side), a `HEAD` file whose (path, OID) the index holds as a regular
/// file is not asked of git: its bytes are the index side's, so its parse
/// and binding are reused as equal bytes are; a shared `spec.lock` is the
/// index side's lock.
pub(crate) fn read_head(
    walked: &Walked<'_>,
    git: &mut VerifyGit,
    config_path: Option<&str>,
    index: Option<&IndexShare<'_>>,
) -> Result<HeadSide, RunCause> {
    let git_cause = |cause: specengine_core::check::Cause| RunCause::new(CAUSE_GIT, cause.message);
    let Some(listing) = git.list_commit("HEAD").map_err(git_cause)? else {
        return Ok(HeadSide {
            listing: None,
            input: CheckInput::default(),
            tree: None,
            lock: Lock::empty(build_recipes()),
            mode: HeadMode::Absent,
            lock_note: None,
        });
    };
    let project = walked.project;
    let code = walked.code;
    let documents = listing.documents(&project.config.paths.walk_scope());
    let mut code_files: Vec<String> = Vec::new();
    let mut manifests: Vec<String> = Vec::new();
    for (path, entry) in listing.entries() {
        if entry.kind != CommitEntryKind::Regular {
            continue;
        }
        if is_code_path(code, path) {
            code_files.push(path.to_owned());
        } else if is_manifest(code, path) {
            manifests.push(path.to_owned());
        }
    }
    let mut wanted: BTreeSet<&str> = BTreeSet::new();
    wanted.extend(documents.iter().map(String::as_str));
    wanted.extend(code_files.iter().map(String::as_str));
    wanted.extend(manifests.iter().map(String::as_str));
    wanted.insert(LOCK_FILE);
    if let Some(config) = config_path {
        wanted.insert(config);
    }
    // The index's side: what it shares is given, never read again.
    let mut given: Vec<(String, Vec<u8>)> = Vec::new();
    let mut shared_lock = None;
    if let Some(share) = index {
        wanted.retain(|path| {
            if !share.shares(&listing, path) {
                return true;
            }
            if *path == LOCK_FILE {
                shared_lock = Some(share.side.lock.clone());
                return false;
            }
            match share.bytes(path) {
                Some(bytes) => {
                    given.push(((*path).to_owned(), bytes));
                    false
                }
                None => true,
            }
        });
    }
    let mut read = git.read_files(&listing, wanted).map_err(git_cause)?;
    for (path, bytes) in given {
        read.insert(path, CommitFile::Bytes(bytes));
    }

    // A partial base judges nothing.
    let mut partial = Vec::new();
    for path in documents.iter().chain(&code_files).chain(&manifests) {
        if let Some(CommitFile::Unreadable(why)) = read.get(path) {
            partial.push(format!("HEAD's `{path}`: {why}"));
        }
    }
    if let Some(first) = partial.into_iter().next() {
        return Err(RunCause::new(CAUSE_BASE, first));
    }

    // The lock at HEAD (the index side's when it shares it), or the judged
    // lock in its place when HEAD's is only unrebased.
    let (lock, lock_note) = match (shared_lock, read.remove(LOCK_FILE)) {
        (Some(lock), _) => (lock, None),
        (None, None) => (Lock::empty(build_recipes()), None),
        (None, Some(CommitFile::Unreadable(why))) => {
            return Err(RunCause::new(
                CAUSE_BASE,
                format!("HEAD's {LOCK_FILE} cannot be read: {why}"),
            ));
        }
        (None, Some(CommitFile::Bytes(bytes))) => {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| RunCause::new(CAUSE_BASE, format!("HEAD's {LOCK_FILE}: not UTF-8")))?;
            head_lock(text, walked.lock)?
        }
    };

    let mode = match config_path.map(|path| read.remove(path)) {
        None => HeadMode::Unknown("the config lies outside the root".to_owned()),
        Some(None) => HeadMode::Absent,
        Some(Some(CommitFile::Unreadable(why))) => {
            HeadMode::Unknown(format!("HEAD's config cannot be read ({why})"))
        }
        Some(Some(CommitFile::Bytes(bytes))) => match std::str::from_utf8(&bytes) {
            Err(_) => HeadMode::Unknown("HEAD's config is not UTF-8".to_owned()),
            Ok(text) => match verify_from_toml(text) {
                Ok(config) => HeadMode::Known(config.mode),
                Err(error) => HeadMode::Unknown(match error.line {
                    Some(line) => format!("HEAD's config is not a valid config (line {line})"),
                    None => "HEAD's config is not a valid config".to_owned(),
                }),
            },
        },
    };

    // HEAD's documents: a working tree parse reused when its bytes are
    // equal.
    let scheme = &project.config.scheme;
    let current: BTreeMap<&str, &CheckFile> = walked
        .input
        .files
        .iter()
        .filter(|file| file.parsed.is_some() && file.read_error.is_none())
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut input = CheckInput::default();
    for path in &documents {
        let Some(CommitFile::Bytes(bytes)) = read.remove(path) else {
            continue;
        };
        if let Some(file) = current
            .get(path.as_str())
            .filter(|file| file.bytes == bytes)
        {
            input.files.push((*file).clone());
            continue;
        }
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, &bytes, scheme)
        }))
        .map_err(|_| {
            RunCause::new(
                CAUSE_BASE,
                format!("the spec parser failed on HEAD's `{path}`"),
            )
        })?;
        input
            .files
            .push(CheckFile::parsed(path.clone(), bytes, parsed));
    }

    // HEAD's code tree: the walk's when every code file and every
    // manifest the naming read are equal, else bound from HEAD's blobs.
    let mut blobs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for path in &code_files {
        if let Some(CommitFile::Bytes(bytes)) = read.remove(path) {
            blobs.insert(path.clone(), bytes);
        }
    }
    let mut seen: BTreeMap<String, ManifestSeen> = BTreeMap::new();
    for path in &manifests {
        let dir = path
            .strip_suffix(MANIFEST)
            .map(|dir| dir.strip_suffix('/').unwrap_or(dir))
            .unwrap_or_default();
        let manifest = match read.remove(path) {
            Some(CommitFile::Bytes(bytes)) => ManifestSeen::Bytes(bytes),
            _ => ManifestSeen::Unreadable,
        };
        seen.insert(dir.to_owned(), manifest);
    }
    let tree = walked.tree;
    let walked_paths = tree.rust.len() + tree.ron.len() + tree.not_utf8.len();
    let same_files = tree.not_utf8.is_empty()
        && blobs.len() == walked_paths
        && tree
            .rust
            .iter()
            .map(|file| (&file.path, &file.text))
            .chain(tree.ron.iter().map(|file| (&file.path, &file.text)))
            .all(|(path, text)| {
                blobs
                    .get(path)
                    .is_some_and(|bytes| bytes == text.as_bytes())
            });
    let same_manifests = tree
        .manifests
        .iter()
        .all(|(dir, walked)| seen.get(dir).unwrap_or(&ManifestSeen::Absent) == walked);
    let tree = if same_files && same_manifests {
        None
    } else {
        Some(tree_of_blobs(blobs, &seen, tree).map_err(|gap| {
            RunCause::new(CAUSE_BASE, format!("HEAD's code walk is incomplete: {gap}"))
        })?)
    };
    Ok(HeadSide {
        listing: Some(listing),
        input,
        tree,
        lock,
        mode,
        lock_note,
    })
}

/// `HEAD`'s lock, read in a rebase's order (task spec `lock-rebase`,
/// "The base"): this build's, as it is; else, at this build's
/// `norm_recipe`, a format-1 lock or one of another `code_recipe` gives way
/// to `judged` (the disk's lock, the index's under `--staged`, read at this
/// build's recipes), so a rebase's commit, which changes no key, `rev`,
/// `spec_hash` or `file_blob`, reads as at `HEAD`: `judged` and the note
/// saying so.
///
/// `HEAD`'s lock unreadable or of another `norm_recipe`: cause (7), the
/// strict reader's text.
fn head_lock(text: &str, judged: &Lock) -> Result<(Lock, Option<String>), RunCause> {
    let build = build_recipes();
    let strict = match parse_lock(text, Some(&build)) {
        Ok(lock) => return Ok((lock, None)),
        Err(error) => RunCause::new(CAUSE_BASE, format!("HEAD's {error}")),
    };
    let Ok(old) = parse_lock(text, None) else {
        return Err(strict);
    };
    if old.recipes.norm != build.norm {
        return Err(strict);
    }
    let note = format!(
        "HEAD's {LOCK_FILE} is format {} (\"{}\"): judged with the current lock",
        old.format.number(),
        old.recipes.code
    );
    Ok((judged.clone(), Some(note)))
}

// ------------------------------------------------------------ one commit

/// One commit's side, read alone ([`read_commit`]): its listing, its
/// documents parsed, its code tree bound from its blobs (under its own
/// `[code]`) and its lock.
pub(crate) struct CommitSide {
    listing: CommitListing,
    input: CheckInput,
    tree: CodeTree,
    /// Every `Cargo.toml` read, by directory: what [`IndexShare::bytes`]
    /// gives for a shared one.
    manifests: BTreeMap<String, ManifestSeen>,
    /// The commit's lock, parsed against this build's recipes (absent:
    /// empty).
    pub lock: Lock,
}

impl CommitSide {
    /// The commit's documents, parsed from its blobs: what a REF resolves
    /// over at the commit.
    pub(crate) fn input(&self) -> &CheckInput {
        &self.input
    }

    /// What [`read_head`] reuses of it: its project, `[code]`, documents,
    /// code tree and lock, as a walk gives them.
    pub(crate) fn walked<'a>(
        &'a self,
        project: &'a ProjectRoot,
        code: &'a CodeConfig,
    ) -> Walked<'a> {
        Walked {
            project,
            code,
            input: &self.input,
            tree: &self.tree,
            lock: &self.lock,
        }
    }
}

/// The git index's side ([`read_commit`] over [`CommitConfig::of_index`])
/// as [`read_head`] reuses it: a `HEAD` file at the same path with the
/// same OID is given these bytes, never read from git again.
pub(crate) struct IndexShare<'a> {
    pub side: &'a CommitSide,
    /// The index's config bytes, when the config judged is its root
    /// `specengine.toml` (not `--config`'s).
    pub config: Option<&'a [u8]>,
}

/// The stand-in bytes of a shared code file whose content is not UTF-8:
/// the binding skips it and counts it, as it does `HEAD`'s equal blob.
const NOT_UTF8: [u8; 1] = [0xFF];

impl IndexShare<'_> {
    /// `HEAD` (listed as `head`) and the index hold `path` as a regular
    /// file with the same OID.
    fn shares(&self, head: &CommitListing, path: &str) -> bool {
        match (head.entry(path), self.side.listing.entry(path)) {
            (Some(at_head), Some(staged)) => {
                at_head.kind == CommitEntryKind::Regular
                    && staged.kind == CommitEntryKind::Regular
                    && at_head.oid == staged.oid
            }
            _ => false,
        }
    }

    /// The index side's bytes of `path`: its config, a document, a code
    /// file, a manifest; `None` when the side kept none.
    fn bytes(&self, path: &str) -> Option<Vec<u8>> {
        if path == CONFIG_FILE
            && let Some(config) = self.config
        {
            return Some(config.to_vec());
        }
        let files = &self.side.input.files;
        if let Ok(at) = files.binary_search_by(|file| file.path.as_str().cmp(path)) {
            return Some(files[at].bytes.clone());
        }
        let tree = &self.side.tree;
        let text_at = |files: &[RustFile]| {
            files
                .binary_search_by(|file| file.path.as_str().cmp(path))
                .ok()
                .map(|at| files[at].text.clone().into_bytes())
        };
        if let Some(bytes) = text_at(&tree.rust) {
            return Some(bytes);
        }
        if let Ok(at) = tree
            .ron
            .binary_search_by(|file| file.path.as_str().cmp(path))
        {
            return Some(tree.ron[at].text.clone().into_bytes());
        }
        if tree.not_utf8.contains(path) {
            return Some(NOT_UTF8.to_vec());
        }
        // A manifest's directory as `is_manifest` derives it: the root's
        // is empty, any other ends in `/` (never `xCargo.toml`).
        let dir = path.strip_suffix(MANIFEST)?;
        let dir = if dir.is_empty() {
            dir
        } else {
            dir.strip_suffix('/')?
        };
        match self.side.manifests.get(dir) {
            Some(ManifestSeen::Bytes(bytes)) => Some(bytes.clone()),
            _ => None,
        }
    }
}

/// A commit listed and the `[code]` of its own root config: the first
/// half of [`read_commit`].
pub(crate) struct CommitConfig {
    listing: CommitListing,
    code: CodeConfig,
}

impl CommitConfig {
    /// The git index's listing ([`VerifyGit::staged`]) and the `[code]`
    /// the caller read (the index's config, or `--config`'s).
    pub(crate) fn of_index(listing: CommitListing, code: CodeConfig) -> Self {
        Self { listing, code }
    }
}

/// The commit `commit` (an OID the caller bound, never `HEAD` by name)
/// listed through `git`'s session, and its root config's `[code]` (none:
/// cause 2; invalid, unreadable or absent: cause 1). Causes: git's (4), a
/// commit it does not name.
pub(crate) fn commit_config(git: &mut VerifyGit, commit: &str) -> Result<CommitConfig, RunCause> {
    let git_cause = |cause: specengine_core::check::Cause| RunCause::new(CAUSE_GIT, cause.message);
    let listing = git
        .list_commit(commit)
        .map_err(git_cause)?
        .ok_or_else(|| RunCause::new(CAUSE_GIT, format!("{commit} names no commit")))?;
    let label = format!("{CONFIG_FILE} at {}", listing.label());
    let config = match git
        .read_files(&listing, [CONFIG_FILE])
        .map_err(git_cause)?
        .remove(CONFIG_FILE)
    {
        Some(CommitFile::Bytes(bytes)) => String::from_utf8(bytes)
            .map_err(|_| RunCause::new(CAUSE_CONFIG, format!("{label}: not UTF-8")))?,
        Some(CommitFile::Unreadable(why)) => {
            return Err(RunCause::new(CAUSE_CONFIG, format!("{label}: {why}")));
        }
        None => {
            return Err(RunCause::new(
                CAUSE_CONFIG,
                format!("{label}: no such file in the commit"),
            ));
        }
    };
    let code = match CodeConfig::from_toml(&config) {
        Ok(Some(code)) => code,
        Ok(None) => {
            return Err(RunCause::new(
                CAUSE_NO_CODE,
                format!(
                    "{label} has no [code] table: the code layer is off (add [code] with roots)"
                ),
            ));
        }
        Err(error) => return Err(RunCause::new(CAUSE_CONFIG, error.at(&label))),
    };
    Ok(CommitConfig { listing, code })
}

/// The side of a commit ([`commit_config`]) read through `git`'s session
/// alone: its documents as `spec check --changed`'s base lists them under
/// the project's `[paths]` (the caller checks the config committed and
/// clean, so the project's scheme and paths are the commit's), the `.rs`
/// and `.ron` files under its code roots, every `Cargo.toml` under a root
/// or at or above one and its `spec.lock`; each document parsed and each
/// code file bound from its blob, nothing reused from the working tree,
/// which is never read. Causes: git's (4), a listed object missing or no
/// blob (7), its lock invalid (6), a parser panic (7), a Rust file the
/// parser gives no tree (3). The git index's listing
/// ([`CommitConfig::of_index`]; the caller gives the project of the
/// index's config): the same reads, named `in the index`, a blob missing
/// or no blob and a parser panic (3).
pub(crate) fn read_commit(
    project: &ProjectRoot,
    git: &mut VerifyGit,
    config: CommitConfig,
) -> Result<CommitSide, RunCause> {
    let git_cause = |cause: specengine_core::check::Cause| RunCause::new(CAUSE_GIT, cause.message);
    let CommitConfig { listing, code } = config;
    let documents = listing.documents(&project.config.paths.walk_scope());
    let mut code_files: Vec<String> = Vec::new();
    let mut manifests: Vec<String> = Vec::new();
    for (path, entry) in listing.entries() {
        if entry.kind != CommitEntryKind::Regular {
            continue;
        }
        if is_code_path(&code, path) {
            code_files.push(path.to_owned());
        } else if is_manifest(&code, path) {
            manifests.push(path.to_owned());
        }
    }
    let mut wanted: BTreeSet<&str> = BTreeSet::new();
    wanted.extend(documents.iter().map(String::as_str));
    wanted.extend(code_files.iter().map(String::as_str));
    wanted.extend(manifests.iter().map(String::as_str));
    wanted.insert(LOCK_FILE);
    let mut read = git.read_files(&listing, wanted).map_err(git_cause)?;
    // A commit's blob or parse that fails is a partial side (7); the
    // index's, a file of the tree judged that cannot be read (3).
    let (at, unread) = if listing.is_index() {
        (format!("in {}", listing.label()), CAUSE_WALK)
    } else {
        (format!("at {}", listing.label()), CAUSE_BASE)
    };
    if let Some(first) = documents
        .iter()
        .chain(&code_files)
        .chain(&manifests)
        .find_map(|path| match read.get(path) {
            Some(CommitFile::Unreadable(why)) => Some(format!("`{path}` {at}: {why}")),
            _ => None,
        })
    {
        return Err(RunCause::new(unread, first));
    }
    let lock = match read.remove(LOCK_FILE) {
        None => Lock::empty(build_recipes()),
        Some(CommitFile::Unreadable(why)) => {
            return Err(RunCause::new(
                CAUSE_LOCK,
                format!("{LOCK_FILE} {at} cannot be read: {why}"),
            ));
        }
        Some(CommitFile::Bytes(bytes)) => {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| RunCause::new(CAUSE_LOCK, format!("{LOCK_FILE} {at}: not UTF-8")))?;
            parse_lock(text, Some(&build_recipes()))
                .map_err(|error| RunCause::new(CAUSE_LOCK, format!("{error} ({at})")))?
        }
    };
    let scheme = &project.config.scheme;
    let mut input = CheckInput::default();
    for path in &documents {
        let Some(CommitFile::Bytes(bytes)) = read.remove(path) else {
            continue;
        };
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, &bytes, scheme)
        }))
        .map_err(|_| RunCause::new(unread, format!("the spec parser failed on `{path}` {at}")))?;
        input
            .files
            .push(CheckFile::parsed(path.clone(), bytes, parsed));
    }
    let mut blobs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for path in &code_files {
        if let Some(CommitFile::Bytes(bytes)) = read.remove(path) {
            blobs.insert(path.clone(), bytes);
        }
    }
    let mut seen: BTreeMap<String, ManifestSeen> = BTreeMap::new();
    for path in &manifests {
        let dir = path
            .strip_suffix(MANIFEST)
            .map(|dir| dir.strip_suffix('/').unwrap_or(dir))
            .unwrap_or_default();
        let manifest = match read.remove(path) {
            Some(CommitFile::Bytes(bytes)) => ManifestSeen::Bytes(bytes),
            _ => ManifestSeen::Unreadable,
        };
        seen.insert(dir.to_owned(), manifest);
    }
    let tree = tree_of_blobs(blobs, &seen, &CodeTree::empty()).map_err(|gap| {
        RunCause::new(
            CAUSE_WALK,
            format!("the code walk {at} is incomplete: {gap}"),
        )
    })?;
    Ok(CommitSide {
        listing,
        input,
        tree,
        manifests: seen,
        lock,
    })
}

/// Every row of one commit's side ([`read_commit`]), judged as the base is
/// (its files the commit's, its blobs the commit's tree's), in key order;
/// given `base` (`HEAD`'s side, [`read_head`]), each Pre-existing as
/// [`judge_walk`] marks it, else none.
pub(crate) fn judge_commit(
    side: &CommitSide,
    project: &ProjectRoot,
    base: Option<&HeadSide>,
) -> Result<Judged, RunCause> {
    let at = Side {
        input: &side.input,
        tree: &side.tree,
        lock: &side.lock,
        files: Files::Commit(&side.listing),
    };
    let (mut rows, skipped) = judge_side(&at, project)?;
    mark_pre_existing(&mut rows, base, &side.tree, project)?;
    Ok(Judged { rows, skipped })
}

#[cfg(test)]
mod tests {
    //! Task spec `spec-verify` RD-10 (c), the seam `health-drift` (9b)
    //! reuses: [`judge_walk`] judges the code tree it is given and never
    //! walks itself. A scratch copy of `fixtures/code-a` with slice 3's
    //! golden lock is walked once; then a locked marker is removed on
    //! disk. Judged over the first walk the row keeps its marker and is
    //! `ok` (its item re-hashed from the walk's own text, the disk's blob
    //! differing); judged over a second walk the marker is gone. No node
    //! path is written here (the CLI's genre scan, `tests/genre.rs`, reads
    //! every source whole). M: the judge walks itself.

    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use specengine_core::CodeConfig;
    use specengine_core::verify::{Compared, RowBinding, State};
    use specengine_store::{GitEnv, VerifyGit};

    use super::{Judged, Walked, judge_walk, read_head};
    use crate::code_walk::read_tree;
    use crate::corpus::indexed;
    use crate::lock::{build_recipes, read_lock};
    use crate::project::discover_with_text;
    use crate::{Env, Globals};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// A scratch directory, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "specengine-verify-seam-{name}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("scratch");
            Self(fs::canonicalize(&path).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).expect("copy target");
        for entry in fs::read_dir(from).expect("fixture directory") {
            let entry = entry.expect("entry");
            let kind = entry.file_type().expect("file type");
            let target = to.join(entry.file_name());
            if kind.is_dir() {
                copy_dir(&entry.path(), &target);
            } else if kind.is_file() {
                fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
            }
        }
    }

    /// The sandbox's git variables: no global or system config, a fixed
    /// identity and date, the scratch as the ceiling.
    fn git_vars(scratch: &Path) -> Vec<(OsString, OsString)> {
        let mut vars: Vec<(OsString, OsString)> = [
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_AUTHOR_NAME", "Scratch Author"),
            ("GIT_AUTHOR_EMAIL", "author@example.invalid"),
            ("GIT_AUTHOR_DATE", "2026-10-08T09:00:00Z"),
            ("GIT_COMMITTER_NAME", "Scratch Committer"),
            ("GIT_COMMITTER_EMAIL", "committer@example.invalid"),
            ("GIT_COMMITTER_DATE", "2026-10-08T09:00:00Z"),
        ]
        .into_iter()
        .map(|(name, value)| (name.into(), value.into()))
        .collect();
        vars.push(("GIT_CEILING_DIRECTORIES".into(), scratch.into()));
        vars.push(("HOME".into(), scratch.join("git-home").into()));
        if let Some(path) = std::env::var_os("PATH") {
            vars.push(("PATH".into(), path));
        }
        vars
    }

    fn git(root: &Path, vars: &[(OsString, OsString)], args: &[&str]) {
        let status = Command::new("git")
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)))
            .current_dir(root)
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    /// The row of `node`/`symbol` in `judged`: its state, compared and
    /// marker lines.
    fn row(judged: &Judged, node: &str, symbol: &str) -> (State, Option<Compared>, Vec<usize>) {
        let found: Vec<_> = judged
            .rows
            .iter()
            .filter(|row| {
                row.key.node == node
                    && matches!(&row.key.binding, RowBinding::Keyed { symbol: s, .. } if s == symbol)
            })
            .collect();
        assert_eq!(found.len(), 1, "one row {node} {symbol}");
        (
            found[0].judgement.state,
            found[0].judgement.compared,
            found[0].markers.iter().map(|marker| marker.line).collect(),
        )
    }

    #[test]
    fn verify_judges_a_given_walk() {
        let scratch = Scratch::new("walk");
        let root = scratch.0.join("repo");
        copy_dir(&fixtures().join("code-a"), &root);
        fs::remove_file(root.join("expected.json")).expect("expected.json");
        fs::write(
            root.join("spec.lock"),
            fs::read(fixtures().join("spec-lock").join("code-a.lock")).expect("the golden lock"),
        )
        .expect("spec.lock");
        let vars = git_vars(&scratch.0);
        git(&root, &vars, &["init", "-q", "-b", "main"]);
        git(&root, &vars, &["add", "-A"]);
        git(
            &root,
            &vars,
            &["commit", "-q", "--no-verify", "-m", "the fixture"],
        );

        let home = scratch.0.join("home");
        fs::create_dir_all(&home).expect("home");
        let env = Env {
            cwd: root.clone(),
            home: Some(home.into_os_string()),
            xdg_data_home: None,
        };
        let (project, text) = discover_with_text(&env, &Globals::default()).expect("a project");
        let code = CodeConfig::from_toml(&text)
            .expect("a valid [code]")
            .expect("a [code] table");
        let lock = read_lock(&project.root, &build_recipes())
            .expect("the golden lock reads")
            .lock;
        let mut messages = Vec::new();
        let input = indexed(&env, &project, &mut messages, true).expect("the corpus");
        let first = read_tree(&project.root, &code, &mut messages).expect("the first walk");

        // The walk done, the disk changes: the locked `CAP` loses its marker.
        let file = "crates/tally/src/counter.rs";
        let path = project.root.join(file);
        let source = fs::read_to_string(&path).expect("the file");
        let marker = " // @configures LAW-BOUND@2";
        assert_eq!(source.matches(marker).count(), 1);
        fs::write(&path, source.replacen(marker, "", 1)).expect("edited");

        let git_env = GitEnv::new(&project.root, vars.clone());
        let mut session = VerifyGit::open(&project.root, &git_env).expect("git");
        let symbol = "crates/tally::counter::CAP";
        let walked = Walked {
            project: &project,
            code: &code,
            input: &input,
            tree: &first,
            lock: &lock,
        };
        let over_first = judge_walk(&walked, &session, None).expect("judged");
        assert_eq!(
            row(&over_first, "LAW-BOUND", symbol),
            (State::Ok, Some(Compared::Ast), vec![8]),
            "judged over the walk it was given"
        );

        // A base read with the walk reused: Pre-existing judged over it too.
        let head = read_head(&walked, &mut session, Some("specengine.toml"), None).expect("HEAD");
        let with_base = judge_walk(&walked, &session, Some(&head)).expect("judged");
        assert_eq!(row(&with_base, "LAW-BOUND", symbol).0, State::Ok);

        let second = read_tree(&project.root, &code, &mut messages).expect("the second walk");
        let over_second = judge_walk(
            &Walked {
                tree: &second,
                ..walked
            },
            &session,
            None,
        )
        .expect("judged");
        assert_eq!(
            row(&over_second, "LAW-BOUND", symbol),
            (State::BrokenBinding, None, vec![]),
            "a new walk sees the marker gone"
        );
        session.finish().expect("the session ends");
    }
}

#[cfg(test)]
mod selection_tests {
    //! B, the bindings a side hashes for the pairing
    //! (canon `spec-verify-anchors`, "The pairing"; task spec
    //! `anchor-states` RD-4, M8), are chosen by the pure
    //! [`select_bindings`], only rows with no entry, bound to an item, an
    //! inline `mod` or a file module, no marker's list malformed, whose
    //! group (node, relation, kind, item kind) holds a lone entry of E. No
    //! E: none, whatever the side's unlocked bindings. M8: the selection
    //! takes every unlocked binding.

    use std::collections::BTreeMap;

    use specengine_core::lock::{EntryCode, LockKind, LockRelation, Verified};
    use specengine_core::verify::{MarkerFact, MarkerRev, RowBinding, RowKey};

    use super::{Building, CodeRef, Unit, select_bindings};

    fn key(
        node: &str,
        relation: LockRelation,
        file: &str,
        kind: LockKind,
        symbol: &str,
        item_kind: Option<&str>,
    ) -> RowKey {
        RowKey {
            node: node.to_owned(),
            relation,
            file: file.to_owned(),
            binding: RowBinding::Keyed {
                kind,
                symbol: symbol.to_owned(),
                item_kind: item_kind.map(str::to_owned),
            },
        }
    }

    /// A function's key under `implements`.
    fn function(node: &str, file: &str, symbol: &str) -> RowKey {
        key(
            node,
            LockRelation::Implements,
            file,
            LockKind::Item,
            symbol,
            Some("function_item"),
        )
    }

    /// A row with no entry whose one marker (its list `bad_levels` or not)
    /// binds `code`.
    fn marked(code: CodeRef, bad_levels: bool) -> Building {
        let mut row = Building::new();
        row.code = Some(code);
        row.markers.push(MarkerFact {
            line: 3,
            rev: MarkerRev::Written(1),
            bad_levels,
        });
        row
    }

    fn item() -> CodeRef {
        CodeRef::Bound(Unit::Item { file: 0, index: 0 })
    }

    #[test]
    fn lone_entries_select_no_binding() {
        let near = function("N-1", "src/b.rs", "b::moved");
        let file_module = key(
            "N-1",
            LockRelation::Implements,
            "src/b.rs",
            LockKind::Module,
            "b",
            None,
        );
        let inline_mod = key(
            "N-1",
            LockRelation::Implements,
            "src/b.rs",
            LockKind::Module,
            "b::inner",
            None,
        );
        let mut building: BTreeMap<RowKey, Building> = BTreeMap::new();
        building.insert(near.clone(), marked(item(), false));
        building.insert(
            file_module.clone(),
            marked(CodeRef::Bound(Unit::FileModule { file: 0 }), false),
        );
        building.insert(
            inline_mod.clone(),
            marked(CodeRef::Bound(Unit::InlineMod { file: 0, index: 1 }), false),
        );
        // Of other groups: another node, another item kind, another relation.
        building.insert(
            function("N-2", "src/b.rs", "b::other"),
            marked(item(), false),
        );
        building.insert(
            key(
                "N-1",
                LockRelation::Implements,
                "src/b.rs",
                LockKind::Item,
                "b::Shape",
                Some("struct_item"),
            ),
            marked(item(), false),
        );
        building.insert(
            key(
                "N-1",
                LockRelation::Verifies,
                "src/b.rs",
                LockKind::Item,
                "b::check",
                Some("function_item"),
            ),
            marked(item(), false),
        );
        // Of N-1's function group, never B: a malformed list, an entry of
        // its own, an ambiguous binding.
        building.insert(function("N-1", "src/c.rs", "c::bad"), marked(item(), true));
        let mut locked = marked(item(), false);
        locked.entry = Some(Verified {
            rev: 1,
            spec_hash: format!("b3:{}", "0".repeat(64)),
            code: EntryCode::Whole(None),
            file_blob: "0".repeat(40),
            commit: "0".repeat(40),
            verified_at: "2026-10-20".to_owned(),
        });
        building.insert(function("N-1", "src/d.rs", "d::kept"), locked);
        building.insert(
            function("N-1", "src/e.rs", "e::twin"),
            marked(CodeRef::Ambiguous("duplicate"), false),
        );
        // A RON value of N-1.
        building.insert(
            key(
                "N-1",
                LockRelation::Configures,
                "data/x.ron",
                LockKind::Ron,
                "root.a",
                None,
            ),
            marked(
                CodeRef::Bound(Unit::Ron {
                    file: 0,
                    comment: 0..1,
                }),
                false,
            ),
        );

        assert!(
            select_bindings(&[], &building).is_empty(),
            "no lone entry: no binding"
        );
        let elsewhere = function("N-3", "src/a.rs", "a::gone");
        assert!(
            select_bindings(&[&elsewhere], &building).is_empty(),
            "a lone entry whose group holds no binding: none"
        );
        let ron_entry = key(
            "N-1",
            LockRelation::Configures,
            "data/x.ron",
            LockKind::Ron,
            "root.b",
            None,
        );
        assert!(
            select_bindings(&[&ron_entry], &building).is_empty(),
            "a RON value is never B"
        );
        let gone = function("N-1", "src/a.rs", "a::gone");
        assert_eq!(select_bindings(&[&gone], &building), vec![&near]);
        let module_entry = key(
            "N-1",
            LockRelation::Implements,
            "src/a.rs",
            LockKind::Module,
            "a",
            None,
        );
        let mut modules = vec![&file_module, &inline_mod];
        modules.sort();
        assert_eq!(select_bindings(&[&module_entry], &building), modules);
        let mut both = vec![&near, &file_module, &inline_mod];
        both.sort();
        assert_eq!(
            select_bindings(&[&module_entry, &gone], &building),
            both,
            "in key order"
        );
    }
}
