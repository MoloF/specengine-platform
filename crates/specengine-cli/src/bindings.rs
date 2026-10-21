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
//! - **A read's judged set** ([`with_judged_set`], task spec `lock-diff`):
//!   the same code layer and judge, the judged set and the open git
//!   session handed to the read before the session ends; any cause, the
//!   session's end failing too, the read's one `bindings:` note.
//! - **A read's blocks** (task spec `tested-reads`): with the rows judged,
//!   the `[[tested]]` blocks of the set's nodes (by their canonical IDs),
//!   judged by `spec verify`'s one function ([`tested_entries`]) over the
//!   judge's tree, the uncut lock and the same git session, before it
//!   ends; nothing read when the lock holds no block. A failure to judge
//!   them is the read's note `tested: <cause>`, `tested` `None`; any cause
//!   (the session's end failing too) leaves `tested` `None` and only the
//!   `bindings:` note.
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
use specengine_model::{BindingAnchor, BindingPlace, BindingRow, TestedRow};
use specengine_store::{
    CodeFileState, CodeRefs, CodeSelection, GitEnv, SqliteIndex, StoredName, StoredNames,
    StoredSymbol, VerifyGit,
};

use crate::code_index::{known_frameworks, refresh_code};
use crate::code_walk::{CodeTree, PartialGap, partial_tree};
use crate::lock::{build_recipes, read_lock, read_lock_bytes};
use crate::node_set::NodeSet;
use crate::project::ProjectRoot;
use crate::refs::{CodeCount, Corpus, NodeKey, no_code_table, stored_count};
use crate::store_error;
use crate::tested::{tested_entries, tested_note_text};
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
/// set nodes (in step with `judged.rows`), the selected files' stored
/// symbols by path, and the walk the selection's snapshot holds.
pub(crate) struct SetJudged {
    pub judged: Judged,
    pub nodes: Vec<BTreeSet<NodeKey>>,
    pub symbols: BTreeMap<String, Vec<StoredSymbol>>,
    pub walk: SetWalk,
    /// The partial tree judged: every marker file of the set's nodes
    /// (`spec verify --node`'s `tested` reads their bindings in it).
    pub tree: CodeTree,
}

/// Every stored code file of the selection's snapshot (the walk's files,
/// by path) and, when the judge refreshed the code index once more (a file
/// edited since the first refresh), the names and files that refresh
/// skipped for not being UTF-8.
pub(crate) struct SetWalk {
    pub files: Vec<CodeFileState>,
    pub skipped: Option<usize>,
}

/// A read's rows of a node set and its one note (task spec
/// `show-bindings`, "Degrade"), and the set's `[[tested]]` blocks (task
/// spec `tested-reads`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetBindings {
    /// `None`: no row could be read (no `[code]`, causes (1), (3), (8));
    /// else every row of the set, in verify's row order.
    pub rows: Option<Vec<SetRow>>,
    /// The rows were judged; `false`: listed under a cause (4)-(6).
    pub judged: bool,
    /// `bindings: <cause>`, the first cause in verify's order; else, the
    /// rows judged, `tested: <cause>` when the blocks could not be.
    pub note: Option<String>,
    /// The code files the read's walk covered, as `spec refs` counts them;
    /// `None` when no row could be read.
    pub code: Option<CodeCount>,
    /// Each block of a set node, by node bytes, judged by `spec verify`'s
    /// one function over the judge's tree and the uncut lock; `None`
    /// unless the rows were judged, and when the blocks could not be (the
    /// note says why).
    pub tested: Option<Vec<SetTested>>,
}

/// One `[[tested]]` block of a read and the set nodes it is of (those
/// whose canonical ID is its node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetTested {
    pub row: TestedRow,
    pub nodes: BTreeSet<NodeKey>,
}

impl SetBindings {
    /// The blocks of the nodes `of` (`None` when not judged), by node
    /// bytes.
    pub(crate) fn tested_of(&self, of: &BTreeSet<NodeKey>) -> Option<Vec<TestedRow>> {
        self.tested.as_ref().map(|tested| tested_of(tested, of))
    }
}

/// The blocks of `tested` of the nodes `of`, in their order.
pub(crate) fn tested_of(tested: &[SetTested], of: &BTreeSet<NodeKey>) -> Vec<TestedRow> {
    tested
        .iter()
        .filter(|entry| !entry.nodes.is_disjoint(of))
        .map(|entry| entry.row.clone())
        .collect()
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
/// notes added to `messages`; the names and files it skipped for not being
/// UTF-8. Causes: its first gap (3), worded as the walk's; the refresh
/// failing (8).
pub(crate) fn refreshed_code(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    code: &CodeConfig,
    messages: &mut Vec<Message>,
) -> Result<usize, RunCause> {
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
        None => Ok(refreshed.report.skipped),
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
        walk: files.walk,
        tree: files.tree,
    })
}

/// The set's files as read: their partial tree, their stored symbols by
/// path, the lock cut to the set, and the selection's walk.
struct SetFiles {
    tree: CodeTree,
    symbols: BTreeMap<String, Vec<StoredSymbol>>,
    cut: Lock,
    walk: SetWalk,
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
    let read = read_selection(
        scope.project,
        scope.code,
        index,
        messages,
        notes_at,
        |refs| selected(refs, &cut, set, corpus),
    )?;
    Ok(SetFiles {
        tree: read.tree,
        symbols: read.symbols,
        cut,
        walk: read.walk,
    })
}

/// A selection's files as read: their partial tree, their stored symbols
/// by path, and the selection's walk.
struct ReadSelection {
    tree: CodeTree,
    symbols: BTreeMap<String, Vec<StoredSymbol>>,
    walk: SetWalk,
}

/// The files `select` picks, from one read snapshot of the code index,
/// each read again and checked against its stored BLAKE3 (a file edited
/// since the refresh: the code index refreshed once more, its notes in
/// place of the first's from `notes_at`, and the selection read again;
/// edited again: a walk gap), bound into a partial tree. Causes: a walk
/// gap (3), the data directory (8).
fn read_selection(
    project: &ProjectRoot,
    code: &CodeConfig,
    index: &mut SqliteIndex,
    messages: &mut Vec<Message>,
    notes_at: usize,
    mut select: impl FnMut(&CodeRefs) -> BTreeSet<String>,
) -> Result<ReadSelection, RunCause> {
    let mut again = false;
    let mut skipped = None;
    loop {
        if again {
            messages.truncate(notes_at);
            skipped = Some(refreshed_code(index, project, code, messages)?);
        }
        let selection = index
            .code_selection(&mut select)
            .map_err(|error| RunCause::new(CAUSE_DATA, unprefixed(&store_error(error))))?
            .unwrap_or_default();
        match fresh_tree(project, &selection) {
            Ok(tree) => {
                let symbols = selection
                    .files
                    .into_iter()
                    .map(|file| (file.path, file.symbols))
                    .collect();
                return Ok(ReadSelection {
                    tree,
                    symbols,
                    walk: SetWalk {
                        files: selection.refs.files,
                        skipped,
                    },
                });
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

// ------------------------------------------------------------ every key

/// What the whole-key judge reads (task spec `health-drift`): the project,
/// its `[code]`, the spec corpus read now (every file read again), the
/// lock, uncut.
pub(crate) struct KeyScope<'a> {
    pub project: &'a ProjectRoot,
    pub code: &'a CodeConfig,
    pub input: &'a CheckInput,
    pub lock: &'a Lock,
}

/// Every key's rows as plain verify judges them, and the partial tree
/// judged: every lock entry's file and every stored marker's file
/// (`spec verify`'s `tested` reads their bindings in it).
pub(crate) struct KeysJudged {
    pub judged: Judged,
    pub tree: CodeTree,
}

/// Plain verify's rows over the code index (task spec `health-drift`):
/// [`judge_set`]'s seam with every key selected, from one read snapshot
/// of the refreshed code index: each lock entry's file and each stored
/// marker's file, whatever its ID names (a `queue` or `other_project` ID's
/// marker is counted `skipped` as plain counts it). Each file is read
/// again and checked against its stored BLAKE3 ([`read_selection`]: a file
/// edited since the refresh, one more refresh, its notes in place of the
/// first's from `notes_at`; edited again: (3)), then judged by plain's
/// judge against the uncut lock (`judge_walk`, no base). Every row is kept,
/// as plain keeps it: a dangling key, a bare `""` one, a `node_gone` entry.
pub(crate) fn judge_every_key(
    scope: &KeyScope<'_>,
    index: &mut SqliteIndex,
    git: &VerifyGit,
    messages: &mut Vec<Message>,
    notes_at: usize,
) -> Result<KeysJudged, RunCause> {
    let lock = scope.lock;
    let read = read_selection(
        scope.project,
        scope.code,
        index,
        messages,
        notes_at,
        |refs| every_key_file(refs, lock),
    )?;
    let walked = Walked {
        project: scope.project,
        code: scope.code,
        input: scope.input,
        tree: &read.tree,
        lock,
    };
    let judged = judge_walk(&walked, git, None)?;
    Ok(KeysJudged {
        judged,
        tree: read.tree,
    })
}

/// Every lock entry's file and every stored marker's file.
fn every_key_file(refs: &CodeRefs, lock: &Lock) -> BTreeSet<String> {
    lock.entries
        .keys()
        .map(|key| key.file.clone())
        .chain(refs.markers.iter().map(|marker| marker.path.clone()))
        .collect()
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
    // The judge reads entries alone: a `[[tested]]` block is no row (a
    // read's blocks are judged apart, over the uncut lock: `set_tested`).
    Lock {
        format: lock.format,
        recipes: lock.recipes.clone(),
        entries,
        tested: BTreeMap::new(),
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
    set_bindings_with(project, text, git_env, index, input, keys, &[])
}

/// [`set_bindings`], with `tested_also`: more nodes whose blocks the read
/// shows (the package's criteria refs). They join the judge's keys only
/// when the lock holds a block: an evidence-free read judges `keys` alone.
/// A row's set nodes may then hold them: a read keeps its rows by its own
/// set.
pub(crate) fn set_bindings_with(
    project: &ProjectRoot,
    text: &str,
    git_env: &GitEnv,
    index: &mut SqliteIndex,
    input: &CheckInput,
    keys: &[NodeKey],
    tested_also: &[NodeKey],
) -> SetBindings {
    let unknown = |cause: RunCause| SetBindings {
        rows: None,
        judged: false,
        note: Some(note(&cause)),
        code: None,
        tested: None,
    };
    let mut messages = Vec::new();
    let ReadOpened {
        code,
        skipped,
        git,
        lock,
        mut causes,
    } = match open_read(project, text, git_env, index, &mut messages) {
        Ok(opened) => opened,
        Err(cause) => return unknown(cause),
    };
    // The latest refresh's skipped names and files: the judge's own when
    // it refreshed again.
    let count = |walk: SetWalk| {
        let refs = CodeRefs {
            files: walk.files,
            ..CodeRefs::default()
        };
        stored_count(&refs, walk.skipped.unwrap_or(skipped))
    };

    let mut tested = None;
    let mut tested_cause = None;
    let judged = match (causes.is_empty(), &lock, &git) {
        (true, Some(lock), Some(open)) => {
            let joined: Vec<NodeKey>;
            let keys = if lock.tested.is_empty() || tested_also.is_empty() {
                keys
            } else {
                joined = keys
                    .iter()
                    .chain(tested_also)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                &joined
            };
            let scope = SetScope {
                project,
                code: &code,
                input,
                lock,
                keys,
            };
            match judge_set(&scope, index, open, &mut messages, 0) {
                Ok(judged) => {
                    // `tested` (task spec `tested-reads`): the set's blocks
                    // by verify's one function, over the judge's tree, the
                    // uncut lock and the open session; never a second judge.
                    match set_tested(project, input, &judged.tree, lock, open, keys) {
                        Ok(entries) => tested = Some(entries),
                        Err(cause) => tested_cause = Some(tested_note_text(&cause)),
                    }
                    Some(judged)
                }
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
            note: tested_cause,
            code: judged.map(|judged| count(judged.walk)),
            tested,
        };
    }

    // (4)-(6) alone: the keys listed, unjudged; with the lock unreadable,
    // its entries as far as its bytes parse.
    let mut rows = None;
    let mut counted = None;
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
            Ok((listed, walk)) => {
                rows = Some(listed);
                counted = Some(count(walk));
            }
            Err(cause) => causes.push(cause),
        }
    }
    // Any cause, the session's end failing too: `tested` not judged, its
    // note dropped (the `bindings:` note covers the read).
    causes.sort_by_key(|cause| cause.order);
    SetBindings {
        rows,
        judged: false,
        note: causes.first().map(note),
        code: counted,
        tested: None,
    }
}

/// What a read's code layer opened before its judge: `[code]`, the
/// refresh's skipped names and files, the git session, the lock parsed
/// with this build's recipes, and the causes (4)-(6) found, in the order
/// found.
struct ReadOpened {
    code: CodeConfig,
    skipped: usize,
    git: Option<VerifyGit>,
    lock: Option<Lock>,
    causes: Vec<RunCause>,
}

/// A read's code layer opened ("Degrade"): `Err`, a cause that leaves no
/// row: no `[code]` (2), a bad `[code]` or an unknown `frameworks` name
/// (1), a walk gap (3), the data directory (8); the code index's notes
/// added to `messages`. Then git, the lock's unmerged stages and the lock,
/// each failure a cause (4)-(6). `[verify]` is never read.
fn open_read(
    project: &ProjectRoot,
    text: &str,
    git_env: &GitEnv,
    index: &mut SqliteIndex,
    messages: &mut Vec<Message>,
) -> Result<ReadOpened, RunCause> {
    let label = &project.config_label;
    // (2), (1): `[code]` alone.
    let code = match CodeConfig::from_toml(text) {
        Ok(Some(code)) => code,
        Ok(None) => {
            let sentence = unprefixed(&no_code_table(label));
            return Err(RunCause::new(CAUSE_NO_CODE, sentence));
        }
        Err(error) => return Err(RunCause::new(CAUSE_CONFIG, error.at(label))),
    };
    if let Err(error) = known_frameworks(&code, label) {
        return Err(RunCause::new(CAUSE_CONFIG, unprefixed(&error)));
    }

    // (3), (8): the code index refreshed, its gap anywhere a cause.
    let skipped = refreshed_code(index, project, &code, messages)?;

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
    Ok(ReadOpened {
        code,
        skipped,
        git,
        lock,
        causes,
    })
}

/// The set `keys` judged as a read's rows are ([`set_bindings`]'s code
/// layer and judge, task spec `lock-diff`), and `then` run on the judged
/// set and the still open git session (its blobs by OID); the session
/// ends after it. `Err`: the read's note `bindings: <cause>`, the first
/// cause in plain verify's order, any cause (1)-(8) or the session's end
/// failing; the code index's own notes are the refresh's, never the
/// read's.
pub(crate) fn with_judged_set<T>(
    project: &ProjectRoot,
    text: &str,
    git_env: &GitEnv,
    index: &mut SqliteIndex,
    input: &CheckInput,
    keys: &[NodeKey],
    then: impl FnOnce(&SetJudged, &mut VerifyGit) -> T,
) -> Result<T, String> {
    let mut messages = Vec::new();
    let ReadOpened {
        code,
        mut git,
        lock,
        mut causes,
        ..
    } = open_read(project, text, git_env, index, &mut messages).map_err(|cause| note(&cause))?;
    let mut answer = None;
    if causes.is_empty()
        && let (Some(lock), Some(open)) = (&lock, git.as_mut())
    {
        let scope = SetScope {
            project,
            code: &code,
            input,
            lock,
            keys,
        };
        match judge_set(&scope, index, open, &mut messages, 0) {
            Ok(judged) => answer = Some(then(&judged, open)),
            Err(cause) => causes.push(cause),
        }
    }
    // The session's end is no cause of its own when one is listed already.
    if let Some(open) = git
        && let Err(cause) = open.finish()
        && causes.is_empty()
    {
        causes.push(RunCause::new(CAUSE_GIT, cause.message));
    }
    causes.sort_by_key(|cause| cause.order);
    match (causes.first(), answer) {
        (None, Some(answer)) => Ok(answer),
        (Some(cause), _) => Err(note(cause)),
        // No cause leaves git and the lock both read: not reached.
        (None, None) => Err(note(&RunCause::new(
            CAUSE_GIT,
            "the code layer was not judged".to_owned(),
        ))),
    }
}

/// The set's blocks (`keys` kept by their canonical IDs, as
/// [`tested_entries`] names a block's node), each with the set nodes it is
/// of; none read when the lock holds no block.
fn set_tested(
    project: &ProjectRoot,
    input: &CheckInput,
    tree: &CodeTree,
    lock: &Lock,
    git: &VerifyGit,
    keys: &[NodeKey],
) -> Result<Vec<SetTested>, String> {
    if lock.tested.is_empty() {
        return Ok(Vec::new());
    }
    let scheme = &project.config.scheme;
    let corpus = Corpus::new(input, scheme, &project.config.paths);
    let set = NodeSet::of(&corpus, scheme, keys.iter().copied());
    drop(corpus);
    let mut of: BTreeMap<&str, BTreeSet<NodeKey>> = BTreeMap::new();
    for key in set.keys() {
        if let Some(id) = set.canonical(key) {
            of.entry(id).or_default().insert(key);
        }
    }
    let keep = |node: &str| of.contains_key(node);
    let entries = tested_entries(project, input, tree, lock, git, &keep)?;
    Ok(entries
        .into_iter()
        .map(|row| SetTested {
            nodes: of.get(row.node.as_str()).cloned().unwrap_or_default(),
            row,
        })
        .collect())
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
/// cut lock's entries, kept to the set; with the selection's walk. Its
/// causes: a walk gap (3), the data directory (8).
fn listed_rows(
    scope: &SetScope<'_>,
    index: &mut SqliteIndex,
    messages: &mut Vec<Message>,
) -> Result<(Vec<SetRow>, SetWalk), RunCause> {
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
    Ok((rows, files.walk))
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

/// A row's binding from the row alone (the brief's Bindings section, task
/// spec `bundle-bindings`): `<kind> <symbol>[ <item_kind>]`; a keyless
/// row's `<sync> <category>` when judged (`<sync>` alone without a
/// category), else `-` (its key's state and category are no keys of the
/// row).
pub(crate) fn row_binding(row: &BindingRow) -> String {
    keyed_binding(
        row.kind.as_deref(),
        row.symbol.as_deref(),
        row.item_kind.as_deref(),
    )
    .or_else(|| {
        let sync = one_line(row.sync.as_deref()?);
        Some(match row.category.as_deref() {
            Some(category) => format!("{sync} {}", one_line(category)),
            None => sync,
        })
    })
    .unwrap_or_else(|| "-".to_owned())
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

#[cfg(test)]
mod tests {
    //! The brief's binding of a row, from the row alone (`row_binding`):
    //! its key, else its judgement, else `-`.

    use specengine_model::BindingRow;

    use super::row_binding;

    /// A row of GOAL-01 in `src/lib.rs`, keyed or keyless as given.
    fn row(
        kind: Option<&str>,
        symbol: Option<&str>,
        sync: Option<&str>,
        category: Option<&str>,
    ) -> BindingRow {
        BindingRow {
            node: "GOAL-01".to_owned(),
            relation: "implements".to_owned(),
            file: "src/lib.rs".to_owned(),
            kind: kind.map(str::to_owned),
            symbol: symbol.map(str::to_owned),
            item_kind: kind.map(|_| "function_item".to_owned()),
            line: None,
            end_line: None,
            signature: None,
            layer: None,
            sync: sync.map(str::to_owned),
            category: category.map(str::to_owned),
            detail: None,
            anchor: None,
        }
    }

    /// Keyed: `<kind> <symbol> <item_kind>`, whatever its judgement; a
    /// keyless row judged: `<sync> <category>`, `<sync>` alone without a
    /// category (iteration 2's (b)); never judged: `-`. M: a judged keyless
    /// row without a category printed `-`.
    #[test]
    fn a_rows_brief_binding_is_its_key_else_its_judgement() {
        let keyed = row(Some("item"), Some("lib::step"), Some("ok"), None);
        assert_eq!(row_binding(&keyed), "item lib::step function_item");
        let unjudged_keyed = row(Some("item"), Some("lib::step"), None, None);
        assert_eq!(row_binding(&unjudged_keyed), "item lib::step function_item");
        let keyless = row(None, None, Some("bad_marker"), Some("unanchored"));
        assert_eq!(row_binding(&keyless), "bad_marker unanchored");
        let no_category = row(None, None, Some("cannot_verify"), None);
        assert_eq!(row_binding(&no_category), "cannot_verify");
        let unjudged = row(None, None, None, None);
        assert_eq!(row_binding(&unjudged), "-");
        let category_alone = row(None, None, None, Some("unanchored"));
        assert_eq!(
            row_binding(&category_alone),
            "-",
            "a category without a sync"
        );
    }
}
