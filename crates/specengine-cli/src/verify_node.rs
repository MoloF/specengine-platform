//! `spec verify --node REF [--symbol QPATH]` (task spec `verify-node`;
//! canon `spec-verify-scope`, "`--node`"): plain verify's rows kept to one
//! node's bindings, judged only over the files the code index picks.
//!
//! In order: discovery (its failure: exit 2, stdout empty); REF resolved as
//! `spec show` resolves it, over the spec index refreshed (a REF naming
//! nothing, a look-alike, `project:`, `HOME` or the data directory: exit 2,
//! stdout empty, `--json` too; several holders: one `warning:`, each
//! verified); then plain's run causes in plain's order, the `cannot:` lines
//! of the printed report (exit 2): (1) also an unknown `[code] frameworks`
//! name (the code index's one reader of the key refuses it: plain never
//! reads it), (3) the refreshed code index's first gap anywhere in the
//! tree, its text the walk's, (8) the code index's refresh failing.
//!
//! With no cause, the selection, from one read snapshot of the refreshed
//! code index ([`SqliteIndex::code_selection`]): the files of the stored
//! markers whose ID `spec refs` lists under a holder, and of the lock
//! entries of the rows that can be the holders' (the lock is cut to them,
//! so no other node's entry is judged `file_gone`). Each selected file is
//! read again, its BLAKE3 compared with the stored one (a file edited since
//! the refresh: the code index is refreshed once more and the selection
//! read again; edited again: (3)); then bound or anchored afresh, its Rust
//! names the stored ones ([`partial_tree`]), and judged by plain's judge
//! (`judge_walk`: levels hashed afresh, never the index's columns; the
//! pairing per node, so the node's own files suffice). The rows kept are
//! "A row in a set" over the holders, both halves, never their sections or
//! parents; `--symbol` keeps those whose key's symbol is QPATH byte for
//! byte (a RON row's `<file>#<symbol>` too), or whose moved anchor's is.
//! Counts, cascade, `delta`, hints, verdict and exit are the kept rows';
//! `skipped` 0; no row: `clean`, with a note. Nothing is written but the
//! data directory (the index refreshes, as `spec refs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use specengine_code::{Ambiguity, Name};
use specengine_core::CodeConfig;
use specengine_core::check::CheckInput;
use specengine_core::lock::{Lock, LockKind};
use specengine_core::verify::RowBinding;
use specengine_store::{CodeRefs, CodeSelection, SqliteIndex, StoredName, StoredNames, VerifyGit};

use crate::code_index::{known_frameworks, refresh_code};
use crate::code_walk::{PartialGap, partial_tree};
use crate::corpus::indexed_in;
use crate::location::open_index;
use crate::node_set::NodeSet;
use crate::project::{ProjectRoot, discover_with_text};
use crate::proposals::escaped_error;
use crate::refs::{Corpus, NodeKey};
use crate::show::classify;
use crate::verify::{
    CAUSE_DATA, Gathered, PlainReads, VerifyNode, VerifyOutcome, VerifyRequest, concluded,
    plain_reads, row_set_nodes, unprefixed,
};
use crate::verify_rows::{
    Anchor, CAUSE_CONFIG, CAUSE_GIT, CAUSE_WALK, HeadMode, Judged, Row, RunCause, Walked,
    judge_walk,
};
use crate::{CliError, Env, Globals, Message, one_line, store_error};

/// Why a written `@rev` is ignored.
const REV_IGNORED: &str = "`spec verify --node` judges each binding against the node as it is now";

/// `spec verify --node REF [--symbol QPATH]`: `reference` is REF as
/// written; the request's other forms were refused before.
pub(crate) fn verify_node(
    env: &Env,
    globals: &Globals,
    reference: &str,
    request: &VerifyRequest,
) -> Result<VerifyOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let written = reference.trim();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();

    // REF before any run cause: its refusal exits 2, stdout empty.
    let target = classify(written, scheme, &mut messages, REV_IGNORED)
        .map_err(escaped_error)?
        .map_err(refused)?;
    let mut open = open_index(env, &project).map_err(escaped_error)?;
    let input =
        indexed_in(&mut open.index, &project, &mut messages, true).map_err(escaped_error)?;
    let (holders, ids) = {
        let corpus = Corpus::new(&input, scheme, &project.config.paths);
        // In holder order: (path, position).
        let holders: Vec<NodeKey> = corpus
            .target_nodes(&target, written)
            .map_err(escaped_error)?
            .map_err(refused)?
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let set = NodeSet::of(&corpus, scheme, holders.iter().copied());
        if holders.len() > 1 {
            let cited: Vec<String> = holders.iter().map(|&key| corpus.citation(key)).collect();
            messages.push(Message::Warning(format!(
                "`{written}` has {} holders, all verified: {}",
                holders.len(),
                cited.join(", ")
            )));
        }
        let ids: Vec<String> = holders
            .iter()
            .map(|&key| {
                set.canonical(key)
                    .map_or_else(|| corpus.citation(key), str::to_owned)
            })
            .collect();
        (holders, ids)
    };
    let node = VerifyNode {
        reference: written.to_owned(),
        ids,
        symbol: request.symbol.clone(),
    };

    // Plain's run causes; (1) also an unknown `[code] frameworks` name.
    let mut causes: Vec<RunCause> = Vec::new();
    let label = &project.config_label;
    let PlainReads {
        code,
        configured,
        git,
        lock,
    } = plain_reads(&project, &text, &request.git, &mut causes);
    let code = code.and_then(|code| match known_frameworks(&code, label) {
        Ok(()) => Some(code),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_CONFIG, unprefixed(&error)));
            None
        }
    });

    // (3), (8): the code index refreshed, its gap anywhere a cause.
    let notes_at = messages.len();
    let refreshed = match &code {
        Some(code) => refreshed_code(&mut open.index, &project, code, &mut messages)
            .map_err(|cause| causes.push(cause))
            .is_ok(),
        None => false,
    };

    let mut judged = None;
    match (causes.is_empty() && refreshed, code, lock, git) {
        (true, Some(code), Some(lock), Some(git)) => {
            let scope = Scope {
                project: &project,
                code: &code,
                input: &input,
                lock: &lock,
                holders: &holders,
            };
            match judge_node(&scope, &mut open.index, &git, &mut messages, notes_at) {
                Ok(rows) => judged = Some(rows),
                Err(cause) => causes.push(cause),
            }
            if let Err(cause) = git.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (.., Some(git)) => {
            let _ = git.finish();
        }
        _ => {}
    }
    if let Some(judged) = judged.as_mut().filter(|_| causes.is_empty()) {
        if let Some(symbol) = &node.symbol {
            judged.rows.retain(|row| on_symbol(row, symbol));
        }
        judged.skipped = 0;
        if judged.rows.is_empty() {
            let on = node
                .symbol
                .as_deref()
                .map(|symbol| format!(" on {symbol}"))
                .unwrap_or_default();
            messages.push(Message::Note(format!(
                "verify: {written} has no binding{on}"
            )));
        }
    }
    Ok(concluded(Gathered {
        configured,
        has_base: false,
        base: None,
        task: None,
        node: Some(node),
        head_mode: HeadMode::Absent,
        judged,
        causes,
        messages,
        label,
    }))
}

/// A REF that names nothing: exit 2, its reason on one line.
fn refused(reason: String) -> CliError {
    escaped_error(CliError::spec(one_line(&reason)))
}

/// What `--node` judges over: the project, its `[code]`, the spec corpus
/// read now, the whole lock, REF's holders.
struct Scope<'a> {
    project: &'a ProjectRoot,
    code: &'a CodeConfig,
    input: &'a CheckInput,
    lock: &'a Lock,
    holders: &'a [NodeKey],
}

/// The code index refreshed (`refresh_code`, as `spec refs` calls it), its
/// notes added to `messages`. Causes: its first gap (3), worded as the
/// walk's; the refresh failing (8).
fn refreshed_code(
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

/// The holders' rows: the selection read, its files read again and
/// checked against the stored BLAKE3 (a mismatch: one more refresh, its
/// notes in place of the first's from `notes_at`), the partial tree judged
/// against the cut lock, the rows kept to the holders.
fn judge_node(
    scope: &Scope<'_>,
    index: &mut SqliteIndex,
    git: &VerifyGit,
    messages: &mut Vec<Message>,
    notes_at: usize,
) -> Result<Judged, RunCause> {
    let project = scope.project;
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(scope.input, scheme, &project.config.paths);
    let set = NodeSet::of(&corpus, scheme, scope.holders.iter().copied());
    let cut = cut_lock(scope.lock, &set, &mut corpus);

    let mut again = false;
    let tree = loop {
        if again {
            messages.truncate(notes_at);
            refreshed_code(index, project, scope.code, messages)?;
        }
        let selection = index
            .code_selection(|refs| selected(refs, &cut, &set, &mut corpus))
            .map_err(|error| RunCause::new(CAUSE_DATA, unprefixed(&store_error(error))))?
            .unwrap_or_default();
        match fresh_tree(project, &selection) {
            Ok(tree) => break tree,
            Err(Fresh::Gap(gap)) => return Err(walk_gap(&gap)),
            Err(Fresh::Changed(path)) if again => {
                return Err(walk_gap(&format!(
                    "`{path}` changed while it was read; run it again"
                )));
            }
            Err(Fresh::Changed(_)) => again = true,
        }
    };
    let walked = Walked {
        project,
        code: scope.code,
        input: scope.input,
        tree: &tree,
        lock: &cut,
    };
    let mut judged = judge_walk(&walked, git, None)?;
    judged
        .rows
        .retain(|row| !row_set_nodes(row, &set, &mut corpus).is_empty());
    Ok(judged)
}

/// The lock cut to the entries whose row can be a holder's: its key's node
/// names a holder as a marker's ID would, or is a holder's canonical ID.
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
/// `spec refs` lists under a holder.
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
fn fresh_tree(
    project: &ProjectRoot,
    selection: &CodeSelection,
) -> Result<crate::code_walk::CodeTree, Fresh> {
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

/// Whether `--symbol`'s QPATH names a row: its key's symbol, byte for byte
/// (a RON row's `<file>#<symbol>` too), or its moved anchor's old one.
fn on_symbol(row: &Row, qpath: &str) -> bool {
    let names = |file: &str, kind: LockKind, symbol: &str| {
        symbol == qpath
            || (kind == LockKind::Ron
                && qpath
                    .strip_prefix(file)
                    .and_then(|rest| rest.strip_prefix('#'))
                    == Some(symbol))
    };
    let own = match &row.key.binding {
        RowBinding::Keyed { kind, symbol, .. } => names(&row.key.file, *kind, symbol),
        RowBinding::Keyless { .. } => false,
    };
    own || matches!(&row.anchor, Some(Anchor::Moved(from)) if names(&from.file, from.kind, &from.symbol))
}
