//! `spec verify`'s judge over a code tree already walked
//! (canon `spec-verify`, "Rows", "States", "Cascade", "Base"): the
//! rows of one side (the working tree, or a commit), each judged by core's pure [`judge_row`] over facts gathered
//! here, and, given a base, which rows are Pre-existing. It never calls
//! [`crate::code_walk::read_tree`]: the caller walks once and hands the
//! tree in ([`judge_walk`]); a commit's side is read by [`read_head`],
//! which takes the walk to reuse.
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
//! - **Cascade**: a bound row with an entry compares the file's blob OID
//!   (on disk one `hash-object` for every such file; at a commit its
//!   tree's OID); equal: unchanged, nothing parsed; else a `module` or
//!   `ron` changed, an item's file re-parsed from the walk's text and its
//!   whole item hashed (`hash_bound_items`). The judge gets the outcome.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use specengine_code::{HashState, Levels, MOD_KIND, RustParser, hash_bound_items, markers_in};
use specengine_core::check::{CheckFile, CheckInput, Mode};
use specengine_core::lock::{LOCK_FILE, Lock, LockKind, Verified, parse_lock};
use specengine_core::verify::{
    CodeFact, Compared, Comparison, Judgement, MarkerFact, MarkerRev, NodeFact, RowBinding,
    RowFacts, RowKey, Unbound, is_pre_existing, judge_row, verify_from_toml,
};
use specengine_core::{CodeConfig, norm_input, revision};
use specengine_store::{CommitEntryKind, CommitFile, CommitListing, VerifyGit, b3_hash};

use crate::code_walk::{CodeTree, MANIFEST, ManifestSeen, tree_of_blobs};
use crate::lock::{Target, build_recipes, is_code_path, is_manifest, relation_of};
use crate::project::ProjectRoot;
use crate::propose::canonical_id;
use crate::refs::{CANNOT_VERIFY, Corpus, IdState, NodeKey, UNANCHORED};

/// Run causes, by their place in "Verdict and exit"'s order.
pub(crate) const CAUSE_WALK: u8 = 3;
pub(crate) const CAUSE_GIT: u8 = 4;
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
    /// The commit's lock, parsed (absent: empty).
    pub lock: Lock,
    pub mode: HeadMode,
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
    pub entry: Option<Verified>,
    /// The row names one node.
    pub resolved: bool,
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
    if let Some(head) = base
        && let Some(listing) = &head.listing
    {
        let side = Side {
            input: &head.input,
            tree: head.tree.as_ref().unwrap_or(walked.tree),
            lock: &head.lock,
            files: Files::Commit(listing),
        };
        let (head_rows, _) = judge_side(&side, walked.project)?;
        let at_head: BTreeMap<RowKey, Judgement> = head_rows
            .into_iter()
            .map(|row| (row.key, row.judgement))
            .collect();
        for row in &mut rows {
            row.pre_existing = is_pre_existing(&row.key, &row.judgement, &at_head);
        }
    }
    Ok(Judged { rows, skipped })
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
    /// A key's binding, not ambiguous: the item (Rust file and item
    /// index) when it is one.
    Bound {
        item: Option<(usize, usize)>,
    },
    Ambiguous(&'static str),
    Unanchored(&'static str),
    ItemError(&'static str),
}

/// A row before it is judged.
struct Building {
    node: Option<NodeRef>,
    code: Option<CodeRef>,
    markers: Vec<MarkerFact>,
    entry: Option<Verified>,
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
            let code_ref = code_ref(&target, Some(index));
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
    for file in &side.tree.ron {
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
            let code_ref = code_ref(&target, None);
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
            .or_insert_with(|| Building {
                node: None,
                code: None,
                markers: Vec::new(),
                entry: None,
            })
            .entry = Some(verified.clone());
    }

    let comparisons = cascade(side, &building)?;
    let canonical = canonical_nodes(&corpus, project);
    let mut spec_side: BTreeMap<NodeKey, (Option<u32>, Option<String>)> = BTreeMap::new();
    let mut rows = Vec::with_capacity(building.len());
    for (key, row) in building {
        let node = match row.node {
            Some(node) => node,
            None => match canonical.get(&key.node).map(Vec::as_slice) {
                Some([node]) => NodeRef::Found(*node),
                Some([_, _, ..]) => NodeRef::Ambiguous,
                _ => {
                    // No node has the entry's ID.
                    let facts = RowFacts {
                        node: NodeFact::Gone,
                        entry: row.entry.clone(),
                        markers: Vec::new(),
                        code: CodeFact::Unbound(unbound(side, &key, &keyless_broken)),
                        key,
                    };
                    rows.push(judged(facts, None, false));
                    continue;
                }
            },
        };
        let (node_fact, node_rev) = match node {
            NodeRef::Found(at) => {
                let (rev, hash) = spec_side
                    .entry(at)
                    .or_insert_with(|| node_side(&corpus, at))
                    .clone();
                (NodeFact::Found { rev, hash }, rev)
            }
            NodeRef::Ambiguous => (NodeFact::Ambiguous, None),
            NodeRef::Homoglyph => (NodeFact::Homoglyph, None),
            NodeRef::Dangling => (NodeFact::Dangling, None),
        };
        let resolved = matches!(node, NodeRef::Found(_));
        let code_fact = match &row.code {
            Some(CodeRef::Ambiguous(reason)) => CodeFact::Ambiguous((*reason).to_owned()),
            Some(CodeRef::Unanchored(category)) => CodeFact::Unanchored((*category).to_owned()),
            Some(CodeRef::ItemError(category)) => CodeFact::ItemError((*category).to_owned()),
            Some(CodeRef::Bound { .. }) => CodeFact::Bound(
                comparisons
                    .get(&key)
                    .cloned()
                    .unwrap_or(Comparison::NotHashed),
            ),
            None => CodeFact::Unbound(unbound(side, &key, &keyless_broken)),
        };
        let facts = RowFacts {
            key,
            node: node_fact,
            entry: row.entry,
            markers: row.markers,
            code: code_fact,
        };
        rows.push(judged(facts, node_rev, resolved));
    }
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    Ok((rows, skipped))
}

/// A row judged.
fn judged(facts: RowFacts, node_rev: Option<u32>, resolved: bool) -> Row {
    let judgement = judge_row(&facts);
    Row {
        key: facts.key,
        judgement,
        pre_existing: false,
        markers: facts.markers,
        node_rev,
        entry: facts.entry,
        resolved,
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
    let row = building.entry(key).or_insert_with(|| Building {
        node: None,
        code: None,
        markers: Vec::new(),
        entry: None,
    });
    if row.node.is_none() {
        row.node = Some(node);
    }
    // An ambiguous binding of any marker makes the row's.
    match (&row.code, &code) {
        (None, _) | (Some(CodeRef::Bound { .. }), CodeRef::Ambiguous(_)) => row.code = Some(code),
        _ => {}
    }
    let marker = found.marker;
    let token = marker.token(found.comment);
    let rev = if token.len() == marker.id.len() {
        MarkerRev::Unversioned
    } else {
        marker.rev.map_or(MarkerRev::Bad, MarkerRev::Written)
    };
    row.markers.push(MarkerFact {
        line: found.line,
        rev,
        bad_levels: matches!(marker.levels, Levels::Invalid(_)),
    });
}

/// What a target gives a row's code side; `rust`: the Rust file's index.
fn code_ref(target: &Target, rust: Option<usize>) -> CodeRef {
    if let Some(reason) = target.ambiguity() {
        return CodeRef::Ambiguous(reason);
    }
    match target {
        Target::Item { index, .. } => CodeRef::Bound {
            item: rust.map(|file| (file, *index)),
        },
        Target::Module { .. } | Target::Ron { .. } => CodeRef::Bound { item: None },
        Target::Keyless { state, category } if *state == UNANCHORED => {
            CodeRef::Unanchored(category)
        }
        Target::Keyless { category, .. } => CodeRef::ItemError(category),
    }
}

/// The canonical cited ID of the node at `at`; `None` when it has no ID.
fn node_id(corpus: &Corpus<'_>, project: &ProjectRoot, at: NodeKey) -> Option<String> {
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

/// A bound row to compare: its key, its entry, its item (Rust file and
/// item index) when it binds one.
type Compare<'a> = (&'a RowKey, &'a Verified, Option<(usize, usize)>);

/// The cascade: each bound row with an entry compared with it, by the
/// file's blob, then for a changed item by its whole hash.
fn cascade(
    side: &Side<'_>,
    building: &BTreeMap<RowKey, Building>,
) -> Result<BTreeMap<RowKey, Comparison>, RunCause> {
    let compared: Vec<Compare<'_>> = building
        .iter()
        .filter_map(|(key, row)| match (&row.code, &row.entry) {
            (Some(CodeRef::Bound { item }), Some(entry)) => Some((key, entry, *item)),
            _ => None,
        })
        .collect();
    let files: BTreeSet<&str> = compared.iter().map(|(key, ..)| key.file.as_str()).collect();
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
    let mut out = BTreeMap::new();
    let mut to_hash: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (key, entry, item) in &compared {
        let Some(blob) = blobs.get(key.file.as_str()) else {
            out.insert((*key).clone(), Comparison::NotHashed);
            continue;
        };
        if *blob == entry.file_blob {
            out.insert((*key).clone(), Comparison::Equal(Compared::Blob));
            continue;
        }
        match item {
            None => {
                out.insert((*key).clone(), Comparison::Changed(Compared::Blob));
            }
            Some((file, index)) => {
                to_hash.entry(*file).or_default().insert(*index);
            }
        }
    }
    // Level 3: only the bound files whose blob changed are parsed again
    // (the walk keeps no tree), only their compared items hashed.
    let mut hashes: BTreeMap<(usize, usize), Option<HashState>> = BTreeMap::new();
    if !to_hash.is_empty() {
        let mut parser = RustParser::new().map_err(|error| {
            RunCause::new(
                CAUSE_WALK,
                format!("the code walk is incomplete: the Rust grammar cannot be loaded: {error}"),
            )
        })?;
        for (file, indexes) in &to_hash {
            let text = &side.tree.rust[*file].text;
            let indexes: Vec<usize> = indexes.iter().copied().collect();
            let states = match parser.parse(text) {
                Some(tree) => hash_bound_items(&tree, text, &indexes),
                None => vec![None; indexes.len()],
            };
            for (index, state) in indexes.into_iter().zip(states) {
                hashes.insert((*file, index), state);
            }
        }
    }
    for (key, entry, item) in compared {
        if out.contains_key(key) {
            continue;
        }
        let Some(at) = item else {
            continue;
        };
        let comparison = match hashes.get(&at) {
            Some(Some(HashState::Hashed(digest))) => {
                let now = format!("b3:{}", digest.to_hex());
                if Some(&now) == entry.ast_hash.as_ref() {
                    Comparison::Equal(Compared::Ast)
                } else {
                    Comparison::Changed(Compared::Ast)
                }
            }
            Some(Some(state @ HashState::CannotVerify { .. })) => {
                Comparison::CannotVerify(first_category(state))
            }
            _ => Comparison::NotHashed,
        };
        out.insert(key.clone(), comparison);
    }
    Ok(out)
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
/// parser fails on (7).
pub(crate) fn read_head(
    walked: &Walked<'_>,
    git: &mut VerifyGit,
    config_path: Option<&str>,
) -> Result<HeadSide, RunCause> {
    let git_cause = |cause: specengine_core::check::Cause| RunCause::new(CAUSE_GIT, cause.message);
    let Some(listing) = git.list_commit("HEAD").map_err(git_cause)? else {
        return Ok(HeadSide {
            listing: None,
            input: CheckInput::default(),
            tree: None,
            lock: Lock::empty(build_recipes()),
            mode: HeadMode::Absent,
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
    let mut read = git.read_files(&listing, wanted).map_err(git_cause)?;

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

    // The lock at HEAD, parsed against this build's recipes.
    let lock = match read.remove(LOCK_FILE) {
        None => Lock::empty(build_recipes()),
        Some(CommitFile::Unreadable(why)) => {
            return Err(RunCause::new(
                CAUSE_BASE,
                format!("HEAD's {LOCK_FILE} cannot be read: {why}"),
            ));
        }
        Some(CommitFile::Bytes(bytes)) => {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| RunCause::new(CAUSE_BASE, format!("HEAD's {LOCK_FILE}: not UTF-8")))?;
            parse_lock(text, Some(&build_recipes()))
                .map_err(|error| RunCause::new(CAUSE_BASE, format!("HEAD's {error}")))?
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
    })
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
        let head = read_head(&walked, &mut session, Some("specengine.toml")).expect("HEAD");
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
