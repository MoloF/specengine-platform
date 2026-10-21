//! The bindings of a node set (task spec `show-bindings`; canon
//! `spec-verify-scope`, "`--node`"): one judge for `spec verify --node`
//! and for the reads that show a node's code, so a read's `sync` is the
//! state `spec verify --node` gives by construction.
//!
//! - **The judge** ([`judge_set`]): from one read snapshot of the refreshed
//!   code index ([`SqliteIndex::code_selection`]), the files of the stored
//!   markers whose ID `spec refs` lists under a set node, and of the lock
//!   entries of the rows that can be the set's (the lock cut to them, so no
//!   other node's entry is judged `file_gone`). Each selected file is read
//!   again, its BLAKE3 compared with the stored one (a file edited since
//!   the refresh: the code index is refreshed once more and the selection
//!   read again; edited again: a walk gap); then bound or anchored afresh,
//!   its Rust names the stored ones ([`partial_tree`]), and judged by plain
//!   verify's judge (`judge_walk`: levels hashed afresh, never the index's
//!   columns). The rows kept are "A row in a set" over the set's nodes,
//!   both halves; each comes with the set nodes it belongs to and the
//!   selected files' stored symbols (read in the selection's transaction).
//! - **A read's rows** ([`set_bindings`], "Degrade"): never a failure of
//!   the read. No `[code]` (2), a bad `[code]` (an unknown `frameworks`
//!   name too) (1), a walk gap (3) or the data directory (8): no row read
//!   (`null`), whatever else is present. Otherwise no worktree, no git or a
//!   git failure (4), `spec.lock` unmerged (5), unreadable or invalid (6):
//!   the rows' keys listed from the selected files' markers and the cut
//!   lock's entries when its bytes parse at all, never judged (`sync` and
//!   the judgement `null`). Each read one note, `bindings: <cause>`, the
//!   first cause in plain verify's order, its text plain verify's
//!   `cannot:` line. `[verify]` is never read: a bad `mode` changes
//!   nothing.
//! - **A row** ([`BindingRow`]): the lock key, an item's stored symbol's
//!   lines, signature and layer (the symbol at the key's qpath and item
//!   kind in the BLAKE3-checked file), a RON row its first marker's line;
//!   `sync` the state word, `category`, `detail`, `anchor` verify's.
//!
//! Nothing is written but the data directory (the index refreshes, as
//! `spec refs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use specengine_code::{Ambiguity, Name};
use specengine_core::CodeConfig;
use specengine_core::check::CheckInput;
use specengine_core::lock::{LOCK_FILE, Lock, LockKind, parse_lock};
use specengine_core::verify::{Class, RowBinding, RowKey};
use specengine_model::{BindingAnchor, BindingPlace, BindingRow};
use specengine_store::{
    CodeRefs, CodeSelection, GitEnv, SqliteIndex, StoredName, StoredNames, StoredSymbol, VerifyGit,
};

use crate::code_index::{known_frameworks, refresh_code};
use crate::code_walk::{CodeTree, PartialGap, partial_tree};
use crate::lock::{build_recipes, read_lock, read_lock_bytes};
use crate::node_set::NodeSet;
use crate::project::ProjectRoot;
use crate::refs::{Corpus, NodeKey, no_code_table};
use crate::store_error;
use crate::verify::{
    CAUSE_DATA, CAUSE_UNMERGED, VerifyAnchor, VerifyPlace, key_set_nodes, keyed_binding, shown,
    unprefixed,
};
use crate::verify_rows::{
    CAUSE_CONFIG, CAUSE_GIT, CAUSE_LOCK, CAUSE_NO_CODE, CAUSE_WALK, Judged, Row, RunCause, Walked,
    judge_walk, list_rows,
};
use crate::{Message, one_line};

/// What a set's judge reads: the project, its `[code]`, the spec corpus
/// read now (every file read again), the lock, the set's nodes.
pub(crate) struct SetScope<'a> {
    pub project: &'a ProjectRoot,
    pub code: &'a CodeConfig,
    pub input: &'a CheckInput,
    pub lock: &'a Lock,
    pub keys: &'a [NodeKey],
}

/// The set's rows as judged: the judge's rows kept to the set, each one's
/// set nodes (in step with `judged.rows`), and the selected files' stored
/// symbols by path.
pub(crate) struct SetJudged {
    pub judged: Judged,
    pub nodes: Vec<BTreeSet<NodeKey>>,
    pub symbols: BTreeMap<String, Vec<StoredSymbol>>,
}

/// A read's rows of a node set and its one note (task spec
/// `show-bindings`, "Degrade").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetBindings {
    /// `None`: no row could be read (no `[code]`, causes (1), (3), (8));
    /// else every row of the set, in verify's row order.
    pub rows: Option<Vec<SetRow>>,
    /// The rows were judged; `false`: listed under a cause (4)-(6).
    pub judged: bool,
    /// `bindings: <cause>`, the first cause in verify's order.
    pub note: Option<String>,
}

/// One row of a read, the set nodes it belongs to, its binding as verify
/// prints it and its class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetRow {
    pub row: BindingRow,
    pub nodes: BTreeSet<NodeKey>,
    /// `<kind> <symbol>[ <item_kind>]`, a keyless row's `<word>
    /// <category>`.
    pub binding: String,
    /// Verify's class of its state; `None` for `ok` and a row not judged.
    pub class: Option<&'static str>,
}

/// The code index refreshed (`refresh_code`, as `spec refs` calls it), its
/// notes added to `messages`. Causes: its first gap (3), worded as the
/// walk's; the refresh failing (8).
pub(crate) fn refreshed_code(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    code: &CodeConfig,
    messages: &mut Vec<Message>,
) -> Result<(), RunCause> {
    let refreshed = refresh_code(
        index,
        &project.root,
        code,
        &project.config_label,
        &project.config.scheme,
        false,
    )
    .map_err(|error| RunCause::new(CAUSE_DATA, unprefixed(&error)))?;
    messages.extend(refreshed.messages.iter().cloned());
    match refreshed.gap() {
        Some(gap) => Err(walk_gap(gap)),
        None => Ok(()),
    }
}

/// The walk's gap as plain verify words it.
fn walk_gap(gap: &str) -> RunCause {
    RunCause::new(CAUSE_WALK, format!("the code walk is incomplete: {gap}"))
}

/// The set's rows (see the module documentation): the selection read, its
/// files read again and checked against the stored BLAKE3 (a mismatch: one
/// more refresh, its notes in place of the first's from `notes_at`), the
/// partial tree judged against the cut lock, the rows kept to the set.
pub(crate) fn judge_set(
    scope: &SetScope<'_>,
    index: &mut SqliteIndex,
    git: &VerifyGit,
    messages: &mut Vec<Message>,
    notes_at: usize,
) -> Result<SetJudged, RunCause> {
    let project = scope.project;
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(scope.input, scheme, &project.config.paths);
    let set = NodeSet::of(&corpus, scheme, scope.keys.iter().copied());
    let files = set_files(scope, &set, &mut corpus, index, messages, notes_at)?;
    let walked = Walked {
        project,
        code: scope.code,
        input: scope.input,
        tree: &files.tree,
        lock: &files.cut,
    };
    let judged = judge_walk(&walked, git, None)?;
    let mut nodes = Vec::with_capacity(judged.rows.len());
    let mut rows = Vec::with_capacity(judged.rows.len());
    for row in judged.rows {
        let of = key_set_nodes(
            &row.key.node,
            !row.markers.is_empty(),
            row.entry.is_some(),
            &set,
            &mut corpus,
        );
        if !of.is_empty() {
            nodes.push(of);
            rows.push(row);
        }
    }
    Ok(SetJudged {
        judged: Judged {
            rows,
            skipped: judged.skipped,
        },
        nodes,
        symbols: files.symbols,
    })
}

/// The set's files as read: their partial tree, their stored symbols by
/// path, and the lock cut to the set.
struct SetFiles {
    tree: CodeTree,
    symbols: BTreeMap<String, Vec<StoredSymbol>>,
    cut: Lock,
}

/// The selection of the set's files read and checked (see [`judge_set`]).
fn set_files(
    scope: &SetScope<'_>,
    set: &NodeSet,
    corpus: &mut Corpus<'_>,
    index: &mut SqliteIndex,
    messages: &mut Vec<Message>,
    notes_at: usize,
) -> Result<SetFiles, RunCause> {
    let cut = cut_lock(scope.lock, set, corpus);
    let mut again = false;
    loop {
        if again {
            messages.truncate(notes_at);
            refreshed_code(index, scope.project, scope.code, messages)?;
        }
        let selection = index
            .code_selection(|refs| selected(refs, &cut, set, corpus))
            .map_err(|error| RunCause::new(CAUSE_DATA, unprefixed(&store_error(error))))?
            .unwrap_or_default();
        match fresh_tree(scope.project, &selection) {
            Ok(tree) => {
                let symbols = selection
                    .files
                    .into_iter()
                    .map(|file| (file.path, file.symbols))
                    .collect();
                return Ok(SetFiles { tree, symbols, cut });
            }
            Err(Fresh::Gap(gap)) => return Err(walk_gap(&gap)),
            Err(Fresh::Changed(path)) if again => {
                return Err(walk_gap(&format!(
                    "`{path}` changed while it was read; run it again"
                )));
            }
            Err(Fresh::Changed(_)) => again = true,
        }
    }
}

/// The lock cut to the entries whose row can be a set node's: its key's
/// node names one as a marker's ID would, or is one's canonical ID.
fn cut_lock(lock: &Lock, set: &NodeSet, corpus: &mut Corpus<'_>) -> Lock {
    let entries = lock
        .entries
        .iter()
        .filter(|(key, _)| {
            let node = key.node.as_str();
            !set.row_nodes(corpus, &[node], Some(node)).is_empty()
        })
        .map(|(key, verified)| (key.clone(), verified.clone()))
        .collect();
    Lock {
        format: lock.format,
        recipes: lock.recipes.clone(),
        entries,
    }
}

/// The files judged: each cut entry's, and each stored marker's whose ID
/// `spec refs` lists under a set node.
fn selected(
    refs: &CodeRefs,
    cut: &Lock,
    set: &NodeSet,
    corpus: &mut Corpus<'_>,
) -> BTreeSet<String> {
    let mut paths: BTreeSet<String> = cut.entries.keys().map(|key| key.file.clone()).collect();
    for marker in &refs.markers {
        if !paths.contains(&marker.path)
            && !set
                .row_nodes(corpus, &[marker.id.as_str()], None)
                .is_empty()
        {
            paths.insert(marker.path.clone());
        }
    }
    paths
}

/// Why the selected files gave no tree.
enum Fresh {
    /// The walk's gap: a file that cannot be parsed.
    Gap(String),
    /// A file whose bytes, or whose names, are not those the code index
    /// holds: edited since the refresh.
    Changed(String),
}

/// The selection's stored files read again, each checked against its
/// stored BLAKE3, and bound into a partial tree named by the stored names.
fn fresh_tree(project: &ProjectRoot, selection: &CodeSelection) -> Result<CodeTree, Fresh> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut names: BTreeMap<&str, (Name, Vec<Name>)> = BTreeMap::new();
    for file in &selection.files {
        let changed = || Fresh::Changed(file.path.clone());
        let bytes = fs::read(project.root.join(&file.path)).map_err(|_| changed())?;
        if !file.holds(&bytes) {
            return Err(changed());
        }
        if let Some(stored) = &file.names {
            names.insert(&file.path, file_names(stored).ok_or_else(changed)?);
        }
        files.insert(file.path.clone(), bytes);
    }
    partial_tree(files, |path| names.remove(path)).map_err(|gap| match gap {
        PartialGap::Walk(gap) => Fresh::Gap(gap),
        PartialGap::Names(path) => Fresh::Changed(path),
    })
}

/// A file's stored names as the walk's: its module's, then its items';
/// `None` when an ambiguity is not one the walk names.
fn file_names(stored: &StoredNames) -> Option<(Name, Vec<Name>)> {
    let module = name(&stored.module)?;
    let items = stored.items.iter().map(name).collect::<Option<Vec<_>>>()?;
    Some((module, items))
}

/// One stored name as the walk's.
fn name(stored: &StoredName) -> Option<Name> {
    let ambiguity = match stored.ambiguity.as_deref() {
        None => None,
        Some(word) => Some(
            Ambiguity::ALL
                .into_iter()
                .find(|ambiguity| ambiguity.as_str() == word)?,
        ),
    };
    Some(Name {
        qpath: stored.qpath.clone(),
        ambiguity,
        discriminator: stored.discriminator.clone(),
    })
}

// ------------------------------------------------------------ a read's rows

/// A read's rows of the set `keys` (see the module documentation) and its
/// one note; the code index's own notes are the refresh's, never the
/// read's. `text` is the config's; `input` the spec corpus with every file
/// read again, from the same handle `index`.
pub(crate) fn set_bindings(
    project: &ProjectRoot,
    text: &str,
    git_env: &GitEnv,
    index: &mut SqliteIndex,
    input: &CheckInput,
    keys: &[NodeKey],
) -> SetBindings {
    let unknown = |cause: RunCause| SetBindings {
        rows: None,
        judged: false,
        note: Some(note(&cause)),
    };
    let label = &project.config_label;
    // (2), (1): `[code]` alone; `[verify]` is never read.
    let code = match CodeConfig::from_toml(text) {
        Ok(Some(code)) => code,
        Ok(None) => {
            let sentence = unprefixed(&no_code_table(label));
            return unknown(RunCause::new(CAUSE_NO_CODE, sentence));
        }
        Err(error) => return unknown(RunCause::new(CAUSE_CONFIG, error.at(label))),
    };
    if let Err(error) = known_frameworks(&code, label) {
        return unknown(RunCause::new(CAUSE_CONFIG, unprefixed(&error)));
    }

    // (3), (8): the code index refreshed, its gap anywhere a cause.
    let mut messages = Vec::new();
    if let Err(cause) = refreshed_code(index, project, &code, &mut messages) {
        return unknown(cause);
    }

    // (4), (5), (6): git, the lock's unmerged stages, the lock.
    let mut causes: Vec<RunCause> = Vec::new();
    let mut git = match VerifyGit::open(&project.root, git_env) {
        Ok(git) => Some(git),
        Err(cause) => {
            causes.push(RunCause::new(CAUSE_GIT, cause.message));
            None
        }
    };
    if let Some(open) = &mut git {
        match open.is_unmerged(LOCK_FILE) {
            Ok(false) => {}
            Ok(true) => causes.push(RunCause::new(
                CAUSE_UNMERGED,
                format!("{LOCK_FILE} is unmerged: run spec lock regen"),
            )),
            Err(cause) => causes.push(RunCause::new(CAUSE_GIT, cause.message)),
        }
    }
    let lock = match read_lock(&project.root, &build_recipes()) {
        Ok(read) => Some(read.lock),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_LOCK, unprefixed(&error)));
            None
        }
    };

    let judged = match (causes.is_empty(), &lock, &git) {
        (true, Some(lock), Some(open)) => {
            let scope = SetScope {
                project,
                code: &code,
                input,
                lock,
                keys,
            };
            match judge_set(&scope, index, open, &mut messages, 0) {
                Ok(judged) => Some(judged),
                Err(cause) => {
                    causes.push(cause);
                    None
                }
            }
        }
        _ => None,
    };
    // The session's end is no cause of its own when one is listed already
    // (as `verify --node`'s).
    if let Some(open) = git
        && let Err(cause) = open.finish()
        && causes.is_empty()
    {
        causes.push(RunCause::new(CAUSE_GIT, cause.message));
    }
    if causes.is_empty() {
        return SetBindings {
            rows: Some(judged.as_ref().map(judged_rows).unwrap_or_default()),
            judged: true,
            note: None,
        };
    }

    // (4)-(6) alone: the keys listed, unjudged; with the lock unreadable,
    // its entries as far as its bytes parse.
    let mut rows = None;
    if !causes.iter().any(leaves_no_row) {
        let listing = match lock {
            Some(lock) => lock,
            None => any_lock(project),
        };
        let scope = SetScope {
            project,
            code: &code,
            input,
            lock: &listing,
            keys,
        };
        match listed_rows(&scope, index, &mut messages) {
            Ok(listed) => rows = Some(listed),
            Err(cause) => causes.push(cause),
        }
    }
    causes.sort_by_key(|cause| cause.order);
    SetBindings {
        rows,
        judged: false,
        note: causes.first().map(note),
    }
}

/// Whether a cause leaves no row at all: (1), (2), (3), (8).
fn leaves_no_row(cause: &RunCause) -> bool {
    matches!(
        cause.order,
        CAUSE_CONFIG | CAUSE_NO_CODE | CAUSE_WALK | CAUSE_DATA
    )
}

/// The note of a read whose config has no `[code]` (cause (2)): the code
/// layer is not read at all.
pub(crate) fn no_code_note(project: &ProjectRoot) -> String {
    let sentence = unprefixed(&no_code_table(&project.config_label));
    note(&RunCause::new(CAUSE_NO_CODE, sentence))
}

/// The note's text: `bindings: <cause>`, the cause's `cannot:` line.
fn note(cause: &RunCause) -> String {
    format!("bindings: {}", one_line(&cause.message))
}

/// The lock as far as it parses at all (either format, recipes never
/// compared), for its keys; else none.
fn any_lock(project: &ProjectRoot) -> Lock {
    read_lock_bytes(&project.root)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| parse_lock(&text, None).ok())
        .unwrap_or_else(|| Lock::empty(build_recipes()))
}

/// The set's rows listed, never judged: the selected files' tree and the
/// cut lock's entries, kept to the set. Its causes: a walk gap (3), the
/// data directory (8).
fn listed_rows(
    scope: &SetScope<'_>,
    index: &mut SqliteIndex,
    messages: &mut Vec<Message>,
) -> Result<Vec<SetRow>, RunCause> {
    let project = scope.project;
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(scope.input, scheme, &project.config.paths);
    let set = NodeSet::of(&corpus, scheme, scope.keys.iter().copied());
    let files = set_files(scope, &set, &mut corpus, index, messages, 0)?;
    let rows = list_rows(scope.input, &files.tree, &files.cut, project)
        .into_iter()
        .filter_map(|listed| {
            let nodes = key_set_nodes(
                &listed.key.node,
                !listed.markers.is_empty(),
                listed.entry,
                &set,
                &mut corpus,
            );
            (!nodes.is_empty()).then(|| {
                let first_line = listed.markers.iter().map(|marker| marker.line).min();
                SetRow {
                    row: key_row(&listed.key, first_line, &files.symbols),
                    nodes,
                    binding: binding_text(&listed.key),
                    class: None,
                }
            })
        })
        .collect();
    Ok(rows)
}

/// The judged rows as a read shows them.
fn judged_rows(judged: &SetJudged) -> Vec<SetRow> {
    judged
        .judged
        .rows
        .iter()
        .zip(&judged.nodes)
        .map(|(row, nodes)| SetRow {
            row: binding_row(row, &judged.symbols),
            nodes: nodes.clone(),
            binding: binding_text(&row.key),
            class: row.judgement.state.class().map(Class::as_str),
        })
        .collect()
}

/// A row's binding as verify prints it: `<kind> <symbol>[ <item_kind>]`,
/// a keyless row's `<word> <category>`.
fn binding_text(key: &RowKey) -> String {
    match &key.binding {
        RowBinding::Keyed {
            kind,
            symbol,
            item_kind,
        } => keyed_binding(Some(kind.as_str()), Some(symbol), item_kind.as_deref())
            .unwrap_or_default(),
        RowBinding::Keyless { state, category } => format!("{state} {category}"),
    }
}

/// A judged row as a read shows it: its key and place, verify's state,
/// category, detail and anchor.
pub(crate) fn binding_row(row: &Row, symbols: &BTreeMap<String, Vec<StoredSymbol>>) -> BindingRow {
    let first_line = row.markers.iter().map(|marker| marker.line).min();
    let mut binding = key_row(&row.key, first_line, symbols);
    let verified = shown(row);
    binding.sync = Some(verified.state.as_str().to_owned());
    binding.category = verified.category;
    binding.detail = verified.detail;
    binding.anchor = verified.anchor.as_ref().map(anchor_of);
    binding
}

/// A row's key and place, nothing judged: an item's stored symbol (the
/// one at the key's qpath and item kind in the file) gives its lines,
/// signature and layer; a RON value its first marker's line (`first_line`).
fn key_row(
    key: &RowKey,
    first_line: Option<usize>,
    symbols: &BTreeMap<String, Vec<StoredSymbol>>,
) -> BindingRow {
    let mut row = BindingRow {
        node: key.node.clone(),
        relation: key.relation.as_str().to_owned(),
        file: key.file.clone(),
        kind: None,
        symbol: None,
        item_kind: None,
        line: None,
        end_line: None,
        signature: None,
        layer: None,
        sync: None,
        category: None,
        detail: None,
        anchor: None,
    };
    let RowBinding::Keyed {
        kind,
        symbol,
        item_kind,
    } = &key.binding
    else {
        return row;
    };
    row.kind = Some(kind.as_str().to_owned());
    row.symbol = Some(symbol.clone());
    row.item_kind.clone_from(item_kind);
    match kind {
        LockKind::Item => {
            let stored = symbols.get(&key.file).and_then(|stored| {
                stored.iter().find(|candidate| {
                    candidate.qpath == *symbol
                        && Some(candidate.item_kind.as_str()) == item_kind.as_deref()
                })
            });
            if let Some(stored) = stored {
                row.line = Some(u64::from(stored.start_line));
                row.end_line = Some(u64::from(stored.end_line));
                row.signature.clone_from(&stored.signature);
                row.layer = Some(stored.layer.clone());
            }
        }
        LockKind::Ron => row.line = first_line.and_then(|line| u64::try_from(line).ok()),
        LockKind::Module => {}
    }
    row
}

/// Verify's anchor as a row's.
fn anchor_of(anchor: &VerifyAnchor) -> BindingAnchor {
    BindingAnchor {
        state: anchor.state.to_owned(),
        from: anchor.from.as_ref().map(place_of),
        candidates: anchor
            .candidates
            .as_ref()
            .map(|candidates| candidates.iter().map(place_of).collect()),
    }
}

/// Verify's place as a row's.
fn place_of(place: &VerifyPlace) -> BindingPlace {
    BindingPlace {
        file: place.file.clone(),
        kind: place.kind.to_owned(),
        symbol: place.symbol.clone(),
        item_kind: place.item_kind.clone(),
    }
}
