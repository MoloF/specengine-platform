//! `spec verify [--changed | --staged] [--json]`
//! (canon `spec-verify`, "Rows", "Verdict and exit"; canon
//! `spec-verify-output`, "Hints"): drift between `spec.lock`
//! and the tree, one row per lock key, each with a state and a class (Fix,
//! Check, Cannot), Pre-existing with `--changed` when `HEAD` gives the
//! same; the verdict and exit under `[verify] mode` (canon
//! `architecture-code`, "Drift: `spec verify`"). Read-only: only the data
//! directory is written (the index refresh); never `spec.lock`, the
//! worktree, a git object, a queue row or an event.
//!
//! In order: discovery (its failure: exit 2, stdout empty); then every
//! failure is a run cause of the printed report, exit 2 in every mode, its
//! `cannot:` lines in this order: (1) a config error, `[code]`'s or
//! `[verify]`'s; (2) no `[code]`; (3) a walk gap; (4) no worktree, no
//! `git`, a git failure; (5) `spec.lock` unmerged; (6) `spec.lock`
//! unreadable, not UTF-8, invalid, format 1 or of other recipes (core's
//! text, naming `spec lock rebase` but for another `norm_recipe`: the step
//! is the cause's text, no hint); (7) a partial base, `HEAD`'s lock
//! invalid, format 1 or of other recipes; (8) `HOME` or the data
//! directory. Every check
//! that can run does (the walk whenever `[code]` reads), so the first
//! `cannot:` line is the first cause in that order; the base is read only
//! when no other cause is present. A run with a cause judges no row.
//!
//! Each row with an entry shows its anchor (canon `spec-verify-anchors`,
//! "Output"): a moved row its old place, an ambiguous one its candidates,
//! in the text under the row and in the JSON, with `delta.bindings.moved`.
//!
//! The rows come from [`crate::verify_rows`]'s judge over the one walk;
//! `--changed` reads `HEAD` through the same git session, the stricter of
//! the current and `HEAD`'s modes applying. Plain, `enforce-introduced`
//! has no base and is judged as `enforce`, with a note. The mode shown,
//! text and JSON, is always the one the verdict used.
//!
//! `--staged` (task spec `rev-rule`; `--changed` excluded, a usage error)
//! judges what a commit records, through the one-commit seam
//! ([`crate::verify_rows::read_commit`], `judge_commit`) fed the git
//! index's listing (`GIT_INDEX_FILE` kept, read as `spec check --staged`
//! reads it): the index's config whole unless `--config` (disk), its
//! documents, code files, manifests and lock from its blobs; no disk walk,
//! no `hash-object`, no data-directory refresh. Its base is `HEAD`, read
//! over the index's side (a shared path and blob neither read nor parsed
//! again), Pre-existing as `--changed` marks it. An unmerged path is (5)
//! each; an index blob that cannot be read or parsed (3).
//!
//! `--task T` (task spec `verify-task`; `--staged` excluded, a usage
//! error): after discovery the task is resolved ([`named_task`], the queue
//! opened as `spec task show` opens it, `--config` only the root's own
//! file); any failure there exits 2, stdout empty. Then plain's run (with
//! `--changed` its base, Pre-existing marked over the whole tree), and,
//! with no run cause, its rows kept to the task's set from the same corpus
//! ([`task_set`], "A row in a set" both halves); `skipped` 0. Counts,
//! cascade, `delta`, hints, verdict and exit are the kept rows'; a run
//! cause is never filtered. A row naming no node now and a set node no
//! kept row reaches are each counted in a note; the task is read, never
//! changed.
//!
//! `--node REF [--symbol QPATH]` (task spec `verify-node`; `--changed`,
//! `--staged` and `--task` excluded, usage errors): [`crate::verify_node`]
//! judges only the files the code index picks; its text prints `ok` rows
//! too, cut at [`OUTPUT_CAP_CHARS`] as its JSON's `rows` are.
//!
//! `tested` (task spec `tested-evidence`, "Plain verify"): plain, `--task`
//! and `--node` judge each `[[tested]]` block of their nodes by core's one
//! `tested_state` ([`crate::tested`]), the JSON's 11th key and a text line
//! per block not `verified` after the hints (outside `--node`'s cut);
//! `null` under `--changed`, `--staged` and any run cause, and when the
//! blocks cannot be judged (a hashing failure: one note `tested: <cause>`,
//! never a cause). No finding, class, count, hint, verdict or exit depends
//! on it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde::Serialize;
use specengine_core::check::{
    CheckInput, Endpoint, Mode, NodeAt, Resolver, SpecGraph, canonical_id,
};
use specengine_core::drift::raises;
use specengine_core::lock::{
    EntryKey, LOCK_FILE, LockLevel, LockRelation, TestedState, key_problem,
};
use specengine_core::verify::{Finding, RowBinding, State, Verdict, verdict, verify_from_toml};
use specengine_core::{CodeConfig, ProjectConfig};
use specengine_store::{CommitFile, CommitListing, GitEnv, IndexRefusal, VerifyGit};

use crate::cap::OUTPUT_CAP_CHARS;
use crate::code_delta::{CodeDelta, DeltaJson, code_delta};
use crate::code_walk::CodeTree;
use crate::code_walk::read_tree;
use crate::corpus::indexed;
use crate::lock::{build_recipes, read_lock};
use crate::node_set::{Named, NodeSet, named_task, task_set};
use crate::project::{CONFIG_FILE, ProjectRoot, discover_with_text, locate};
use crate::proposals::{escaped_error, open_context};
use crate::refs::{Corpus, NodeKey};
use crate::tested::{TestedEntry, tested_entries, tested_note};
use crate::verify_node::verify_node;
use crate::verify_rows::{
    Anchor, CAUSE_CONFIG, CAUSE_GIT, CAUSE_LOCK, CAUSE_NO_CODE, CAUSE_WALK, CommitConfig, HeadMode,
    IndexShare, Judged, Row, RunCause, Walked, judge_commit, judge_walk, read_commit, read_head,
};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The run causes of "Verdict and exit", by place (the others are the
/// judge's, `verify_rows`).
pub(crate) const CAUSE_UNMERGED: u8 = 5;
pub(crate) const CAUSE_DATA: u8 = 8;

/// `--task` with `--staged`, as the library refuses it (`main`: clap's
/// conflict; task spec `verify-task`): a commit being recorded has no task.
const STAGED_TASK: &str = "the argument '--task <T>' cannot be used with '--staged'";

/// `--node` with a base or another scope, and `--symbol` without `--node`,
/// as the library refuses them (`main`: clap's conflicts and `requires`;
/// task spec `verify-node`): one node's rows, judged plain.
const NODE_CHANGED: &str = "the argument '--node <REF>' cannot be used with '--changed'";
const NODE_STAGED: &str = "the argument '--node <REF>' cannot be used with '--staged'";
const NODE_TASK: &str = "the argument '--node <REF>' cannot be used with '--task <T>'";
const SYMBOL_ALONE: &str = "the following required arguments were not provided: --node <REF>";

/// What `spec verify` judges, and whether against `HEAD`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerifiedTree {
    /// Plain: the working tree, no base.
    #[default]
    WorkingTree,
    /// `--changed`: the working tree, against `HEAD`.
    Changed,
    /// `--staged`: what the git index records, against `HEAD`.
    Staged,
}

/// `spec verify` options.
#[derive(Debug, Clone)]
pub struct VerifyRequest {
    /// Plain, `--changed` or `--staged`.
    pub tree: VerifiedTree,
    /// `--task T` as written: the rows kept to the task's nodes (task spec
    /// `verify-task`); refused with `--staged`.
    pub task: Option<String>,
    /// `--node REF` as written: the rows kept to its holders' bindings,
    /// judged over the files the code index picks (task spec
    /// `verify-node`); refused with `--changed`, `--staged` or `--task`.
    pub node: Option<String>,
    /// `--symbol QPATH` as written: `--node`'s rows kept to one symbol;
    /// refused without `--node`.
    pub symbol: Option<String>,
    /// Git's environment (`--staged`: its `GIT_INDEX_FILE` names the index).
    pub git: GitEnv,
}

/// `--node`'s scope as judged (task spec `verify-node`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyNode {
    /// REF as written, trimmed.
    pub reference: String,
    /// Its holders' canonical IDs (`slug/ID` when feature-scoped; a holder
    /// without an ID: its citation), in holder order.
    pub ids: Vec<String>,
    /// `--symbol`'s QPATH as written.
    pub symbol: Option<String>,
}

/// One marker of a row: its line and `@N` (`None`: unversioned or
/// malformed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyMarker {
    pub line: usize,
    pub rev: Option<u32>,
}

/// A row's lock entry, as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyLock {
    pub rev: u32,
    pub commit: String,
    pub verified_at: String,
}

/// A row's levels as judged (canon `spec-verify`, "States"): `lock` the
/// entry's, `markers` its markers' effective levels united, `changed` the
/// compared levels whose digest changed (`null` when no digest was
/// compared), each in canonical order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyLevels {
    pub lock: Vec<&'static str>,
    pub markers: Vec<&'static str>,
    pub changed: Option<Vec<&'static str>>,
}

/// Where a row's binding is (canon `spec-verify-anchors`,
/// "Anchors per row"): `state` `exact`, `moved`, `ambiguous` or `lost`;
/// `from` a moved row's old place, `candidates` an ambiguous row's, in key
/// order, else `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyAnchor {
    pub state: &'static str,
    pub from: Option<VerifyPlace>,
    pub candidates: Option<Vec<VerifyPlace>>,
}

/// A binding's place as an anchor names it: its key without the node and
/// relation, which are the row's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyPlace {
    pub file: String,
    pub kind: &'static str,
    pub symbol: String,
    pub item_kind: Option<String>,
}

/// The anchor states a row shows.
const EXACT: &str = "exact";
const MOVED: &str = "moved";
const AMBIGUOUS: &str = "ambiguous";
const LOST: &str = "lost";

/// One row: a lock key (keyless: `kind`, `symbol`, `item_kind` null), its
/// state, class and facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyRow {
    pub node: String,
    pub relation: &'static str,
    pub file: String,
    pub kind: Option<&'static str>,
    pub symbol: Option<String>,
    pub item_kind: Option<String>,
    #[serde(serialize_with = "state_name")]
    pub state: State,
    pub category: Option<String>,
    pub detail: Option<String>,
    /// `fix`, `check`, `cannot`; `None` for `ok`.
    pub class: Option<&'static str>,
    pub pre_existing: bool,
    pub markers: Vec<VerifyMarker>,
    /// The node's revision; `None` unknown or no node.
    pub node_rev: Option<u32>,
    pub lock: Option<VerifyLock>,
    /// `blob`, `ast`; `None` without an entry, decided earlier, or nothing
    /// to compare (both sides `[path]`).
    pub compared: Option<&'static str>,
    /// `None` without an entry or a bound marker, or with a marker's list
    /// malformed.
    pub levels: Option<VerifyLevels>,
    /// `None` without an entry, or when the row cannot be verified.
    pub anchor: Option<VerifyAnchor>,
    /// A keyless binding's word (`unanchored`, `cannot_verify`) and
    /// category, as the text prints them.
    #[serde(skip)]
    pub keyless: Option<(String, String)>,
    /// The row names one node.
    #[serde(skip)]
    pub resolved: bool,
    /// `key_problem`'s words, for the hint.
    #[serde(skip)]
    pub key_problem: Option<String>,
}

/// What `spec verify` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOutcome {
    /// The mode judged; `None` when `[verify]` cannot be read.
    pub mode: Option<Mode>,
    /// `None` plain; `HEAD`, `unborn`.
    pub base: Option<&'static str>,
    /// `--task`'s task ID; `None` without it.
    pub task: Option<String>,
    /// `--node`'s scope; `None` without it.
    pub node: Option<VerifyNode>,
    pub verdict: Verdict,
    /// Every row, `ok` too, in key order (`--task`, `--node`: those of its
    /// nodes).
    pub rows: Vec<VerifyRow>,
    /// Markers skipped: `queue` and `other_project` IDs (`--task`,
    /// `--node`: 0).
    pub skipped: usize,
    /// The run causes, in order.
    pub cannot: Vec<String>,
    pub messages: Vec<Message>,
    /// `delta.tests_reaching`'s candidates ([`ReachingTest`]), over every
    /// row the run judged (`--task`: before its filter); none with a run
    /// cause; `None` under `--node` (its JSON `null`).
    pub(crate) tests: Option<Vec<ReachingTest>>,
    /// `delta.symbols` (task spec `impact-since`): `--changed`'s code
    /// items `HEAD` and the working tree differ by, over all code, bound
    /// or not (`--task`: unfiltered); `None` in every other mode and with
    /// a run cause.
    pub(crate) symbols: Option<CodeDelta>,
    /// `tested` (task spec `tested-evidence`, "Plain verify"): each
    /// `[[tested]]` block's state now, by node bytes (plain: every block;
    /// `--task`: its set's; `--node`: its holders'); `None` under
    /// `--changed`, `--staged` and with a run cause (nothing hashed), and
    /// when the blocks cannot be judged (a note says why). Never a
    /// finding, a count, a hint, the verdict or the exit.
    pub tested: Option<Vec<TestedEntry>>,
}

/// A `verifies` row and the nodes it reaches, by canonical ID (task spec
/// `show-bindings`, `delta.tests_reaching`): its `node`, and each node a
/// node it names holds a resolved `verifies` link to (an `out verifies`
/// line of `spec show --links` for it: its own span, a document's
/// front-matter and sections).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReachingTest {
    pub row: VerifyRow,
    pub reaches: BTreeSet<String>,
}

/// The link type a node names what it verifies by (a member of the
/// model's shared [`specengine_model::LINK_TYPES`]).
const VERIFIES_LINK: &str = "verifies";

/// [`ReachingTest`]s of `rows` (any state, Pre-existing too), judged over
/// `input`: each `verifies` row, in row order, with what it reaches.
pub(crate) fn reaching_tests(
    rows: &[Row],
    input: &CheckInput,
    project: &ProjectRoot,
) -> Vec<ReachingTest> {
    let tests: Vec<&Row> = rows
        .iter()
        .filter(|row| row.key.relation == LockRelation::Verifies)
        .collect();
    if tests.is_empty() {
        return Vec::new();
    }
    let scheme = &project.config.scheme;
    let paths = &project.config.paths;
    let resolver = Resolver::new(input, scheme, paths);
    let graph = SpecGraph::new(input, scheme, paths);
    let canonical = |at: NodeAt| -> Option<String> {
        let path = graph.paths().get(at.file)?;
        let id = graph.node(at)?.id.as_deref()?;
        Some(canonical_id(&resolver, scheme, path, id, None))
    };
    // Every node with an ID, by its canonical cited ID.
    let mut named: BTreeMap<String, Vec<NodeAt>> = BTreeMap::new();
    for (file, _) in graph.paths().iter().enumerate() {
        for ord in 0..graph.nodes(file).len() {
            let at = NodeAt { file, ord };
            if let Some(id) = canonical(at) {
                named.entry(id).or_default().push(at);
            }
        }
    }
    // A named node's resolved `verifies` targets, its span's links.
    let mut targets: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut reaches_of = |node: &str| -> BTreeSet<String> {
        if let Some(found) = targets.get(node) {
            return found.clone();
        }
        let mut found = BTreeSet::new();
        for &at in named.get(node).into_iter().flatten() {
            for &(edge, _) in &graph.links(at, |_| true).outgoing {
                let edge = &graph.edges()[edge];
                if edge.link_type != VERIFIES_LINK || edge.names_source {
                    continue;
                }
                if let Endpoint::Nodes(ends) = &edge.target {
                    found.extend(ends.iter().filter_map(|&end| canonical(end)));
                }
            }
        }
        if let Some((key, _)) = named.get_key_value(node) {
            targets.insert(key.as_str(), found.clone());
        }
        found
    };
    tests
        .into_iter()
        .map(|row| {
            let mut reaches = reaches_of(&row.key.node);
            reaches.insert(row.key.node.clone());
            ReachingTest {
                row: shown(row),
                reaches,
            }
        })
        .collect()
}

impl VerifyOutcome {
    /// 0 clean or observed, 1 blocked, 2 cannot verify.
    pub fn exit(&self) -> Exit {
        match self.verdict {
            Verdict::Clean | Verdict::Observed => Exit::Answered,
            Verdict::Blocked => Exit::NotFound,
            Verdict::CannotVerify => Exit::CannotRun,
        }
    }

    /// The `note:` lines' texts, in order.
    fn notes(&self) -> Vec<String> {
        self.messages
            .iter()
            .filter_map(|message| match message {
                Message::Note(text) => Some(one_line(text)),
                Message::Warning(_) => None,
            })
            .collect()
    }
}

/// What a `spec verify` run renders (task spec `impact-since`): `--changed`
/// hashes the code items of `delta.symbols` for the `--json` document
/// alone, so a text run never pays for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyFormat {
    Text,
    Json,
}

/// `spec verify` as the `--json` document renders it ([`verify_as`] with
/// [`VerifyFormat::Json`]): the whole outcome, either rendering.
pub fn verify(
    env: &Env,
    globals: &Globals,
    request: &VerifyRequest,
) -> Result<VerifyOutcome, CliError> {
    verify_as(env, globals, request, VerifyFormat::Json)
}

/// `spec verify`: only discovery (and `--task`'s task, and `--node`'s REF
/// and its index) fails the call; every later failure is a run cause of
/// the outcome (exit 2). A [`VerifyFormat::Text`] outcome is never
/// rendered as JSON: `--changed`'s `delta.symbols` is not computed for it,
/// and nothing else differs.
pub fn verify_as(
    env: &Env,
    globals: &Globals,
    request: &VerifyRequest,
    format: VerifyFormat,
) -> Result<VerifyOutcome, CliError> {
    if let Some(reference) = &request.node {
        let refused = match request.tree {
            VerifiedTree::Changed => Some(NODE_CHANGED),
            VerifiedTree::Staged => Some(NODE_STAGED),
            VerifiedTree::WorkingTree => request.task.is_some().then_some(NODE_TASK),
        };
        if let Some(refused) = refused {
            return Err(CliError::spec(refused));
        }
        return verify_node(env, globals, reference, request);
    }
    if request.symbol.is_some() {
        return Err(CliError::spec(SYMBOL_ALONE));
    }
    if request.tree == VerifiedTree::Staged {
        if request.task.is_some() {
            return Err(CliError::spec(STAGED_TASK));
        }
        return verify_staged(env, globals, request);
    }
    let changed = request.tree == VerifiedTree::Changed;
    let symbols_wanted = changed && format == VerifyFormat::Json;
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    // The task before anything else is read: its refusal exits 2 with
    // stdout empty, never a run cause (task spec `verify-task`).
    let task = match &request.task {
        Some(written) => Some(
            open_context(env, globals, &request.git)
                .and_then(|context| named_task(&context, written))
                .map_err(escaped_error)?,
        ),
        None => None,
    };
    let mut messages = Vec::new();
    let mut causes: Vec<RunCause> = Vec::new();
    let label = &project.config_label;
    let PlainReads {
        code,
        configured,
        git,
        lock,
    } = plain_reads(&project, &text, &request.git, &mut causes);

    // (8): the spec corpus, the index refreshed, every file read again.
    let input = match indexed(env, &project, &mut messages, true) {
        Ok(input) => Some(input),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_DATA, unprefixed(&error)));
            None
        }
    };

    // (3): the one walk, whenever `[code]` reads (it only reads), so its
    // gap is listed in its place beside any other cause.
    let tree = match &code {
        Some(code) => match read_tree(&project.root, code, &mut messages) {
            Ok(tree) => Some(tree),
            Err(error) => {
                causes.push(RunCause::new(CAUSE_WALK, unprefixed(&error)));
                None
            }
        },
        None => None,
    };

    let mut base = changed.then_some("HEAD");
    let mut head_mode = HeadMode::Absent;
    let mut judged = None;
    let mut tests = Vec::new();
    let mut symbols = None;
    let mut tested = None;
    match (causes.is_empty(), code, tree, lock, input, git) {
        (true, Some(code), Some(tree), Some(lock), Some(input), Some(mut open)) => {
            let walked = Walked {
                project: &project,
                code: &code,
                input: &input,
                tree: &tree,
                lock: &lock,
            };
            match run(env, globals, &walked, &mut open, changed, symbols_wanted) {
                Ok(ran) => {
                    if ran.unborn {
                        base = Some("unborn");
                    }
                    head_mode = ran.head_mode;
                    judged = Some(ran.judged);
                    symbols = ran.symbols;
                    messages.extend(ran.lock_note.map(Message::Note));
                }
                Err(cause) => causes.push(cause),
            }
            // `tested`, plain and `--task` alone (a base hashes nothing):
            // every block, or the task's set's.
            // A failure to judge them is one note, `tested` null: never a
            // cause (hash-object runs apart from the session `finish`
            // ends).
            if !changed && causes.is_empty() {
                // The task's set only when a block can be kept by it.
                let set = task
                    .as_ref()
                    .filter(|_| !lock.tested.is_empty())
                    .map(|task| {
                        let scheme = &project.config.scheme;
                        let corpus = Corpus::new(&input, scheme, &project.config.paths);
                        let set = task_set(task, &corpus, scheme, &mut Vec::new());
                        set.keys()
                            .filter_map(|key| set.canonical(key).map(str::to_owned))
                            .collect::<BTreeSet<String>>()
                    });
                let keep = |node: &str| set.as_ref().is_none_or(|set| set.contains(node));
                match tested_entries(&project, &input, &tree, &lock, &open, &keep) {
                    Ok(entries) => tested = Some(entries),
                    Err(cause) => messages.push(tested_note(&cause)),
                }
            }
            if let Err(cause) = open.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
            // The tests reaching, over every row judged; then `--task`: the
            // rows kept to the task's nodes before the verdict; a run cause
            // is the whole run's, never filtered.
            if let (Some(rows), true) = (judged.as_ref(), causes.is_empty()) {
                tests = reaching_tests(&rows.rows, &input, &project);
            }
            if let (Some(task), Some(rows), true) = (&task, judged.as_mut(), causes.is_empty()) {
                keep_task_rows(rows, task, &input, &project, &mut messages);
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (.., Some(open)) => {
            let _ = open.finish();
        }
        _ => {}
    }
    Ok(concluded(Gathered {
        configured,
        has_base: changed,
        base,
        task: task.map(|task| task.id),
        node: None,
        head_mode,
        judged,
        causes,
        messages,
        label,
        tests: Some(tests),
        symbols,
        tested,
    }))
}

/// What plain verify reads before the spec corpus and the code: `[code]`,
/// `[verify] mode`, the git session and the lock, each `None` when its
/// cause is listed.
pub(crate) struct PlainReads {
    pub code: Option<CodeConfig>,
    pub configured: Option<Mode>,
    pub git: Option<VerifyGit>,
    pub lock: Option<specengine_core::lock::Lock>,
}

/// Plain's run causes (1), (2) (the config's `[code]` and `[verify]`),
/// (4), (5) (git, and the lock's unmerged stages) and (6) (the lock,
/// against this build's recipes), in that order, into `causes`.
pub(crate) fn plain_reads(
    project: &ProjectRoot,
    text: &str,
    git_env: &GitEnv,
    causes: &mut Vec<RunCause>,
) -> PlainReads {
    let label = &project.config_label;

    // (1), (2): the config's `[code]` and `[verify]`.
    let code = match CodeConfig::from_toml(text) {
        Ok(Some(code)) => Some(code),
        Ok(None) => {
            causes.push(RunCause::new(
                CAUSE_NO_CODE,
                format!(
                    "{label} has no [code] table: the code layer is off (add [code] with roots)"
                ),
            ));
            None
        }
        Err(error) => {
            causes.push(RunCause::new(CAUSE_CONFIG, error.at(label)));
            None
        }
    };
    let configured = match verify_from_toml(text) {
        Ok(config) => Some(config.mode),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_CONFIG, error.at(label)));
            None
        }
    };

    // (4), (5): git, and the lock's unmerged stages.
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

    // (6): the lock, against this build's recipes.
    let lock = match read_lock(&project.root, &build_recipes()) {
        Ok(read) => Some(read.lock),
        Err(error) => {
            causes.push(RunCause::new(CAUSE_LOCK, unprefixed(&error)));
            None
        }
    };
    PlainReads {
        code,
        configured,
        git,
        lock,
    }
}

/// `--task`'s filter (task spec `verify-task`): `judged`'s rows kept to
/// `task`'s set, built from the same corpus, by "A row in a set", both
/// halves. A row keeps no written marker ID: its key's node is the node its
/// markers resolve to (canonical), or their ID part as written, which the
/// judge reads as it read the markers; so the key's node is the marker half
/// when the row has a marker, and the entry half when it has an entry.
/// `skipped` is 0. Notes: the set's refs naming nothing, the rows naming
/// no node now (never kept), the set's nodes no kept row reaches.
fn keep_task_rows(
    judged: &mut Judged,
    task: &specengine_store::Task,
    input: &specengine_core::check::CheckInput,
    project: &ProjectRoot,
    messages: &mut Vec<Message>,
) {
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(input, scheme, &project.config.paths);
    let set = task_set(task, &corpus, scheme, messages);
    let named = Named::of(&corpus, scheme);
    let mut reached: BTreeSet<NodeKey> = BTreeSet::new();
    let mut nameless = 0usize;
    judged.rows.retain(|row| {
        let nodes = row_set_nodes(row, &set, &mut corpus);
        if nodes.is_empty() {
            let (markers, entry) = row_halves(row);
            if !named.any(&mut corpus, markers.as_slice(), entry) {
                nameless += 1;
            }
            return false;
        }
        reached.extend(nodes);
        true
    });
    judged.skipped = 0;
    if nameless > 0 {
        messages.push(Message::Note(format!(
            "verify --task: {nameless} rows name no node now; plain spec verify lists them"
        )));
    }
    let without = set.keys().filter(|key| !reached.contains(key)).count();
    if without > 0 {
        messages.push(Message::Note(format!(
            "verify --task: {without} nodes of {} have no binding",
            task.id
        )));
    }
}

/// "A row in a set" by its key's node (see [`keep_task_rows`]): the
/// marker half when the row has markers, the entry half when it has an
/// entry.
pub(crate) fn row_set_nodes(
    row: &Row,
    set: &NodeSet,
    corpus: &mut Corpus<'_>,
) -> BTreeSet<NodeKey> {
    key_set_nodes(
        &row.key.node,
        !row.markers.is_empty(),
        row.entry.is_some(),
        set,
        corpus,
    )
}

/// [`row_set_nodes`] of a row by its key's node, whether it has markers
/// and whether it has an entry (a row listed, never judged, too).
pub(crate) fn key_set_nodes(
    node: &str,
    markers: bool,
    entry: bool,
    set: &NodeSet,
    corpus: &mut Corpus<'_>,
) -> BTreeSet<NodeKey> {
    let (markers, entry) = halves(node, markers, entry);
    set.row_nodes(corpus, markers.as_slice(), entry)
}

/// A row's two halves as "A row in a set" reads them: its key's node as
/// its markers' ID (none without a marker), and as its entry's node (none
/// without an entry).
fn row_halves(row: &Row) -> (Vec<&str>, Option<&str>) {
    halves(&row.key.node, !row.markers.is_empty(), row.entry.is_some())
}

/// [`row_halves`] by a key's node and what the row has.
fn halves(node: &str, markers: bool, entry: bool) -> (Vec<&str>, Option<&str>) {
    let marker_half = if markers { vec![node] } else { Vec::new() };
    (marker_half, entry.then_some(node))
}

/// What a run gathered before its verdict.
pub(crate) struct Gathered<'a> {
    /// `[verify] mode`; `None` when it cannot be read.
    pub configured: Option<Mode>,
    /// `--changed` or `--staged`.
    pub has_base: bool,
    pub base: Option<&'static str>,
    /// `--task`'s task ID.
    pub task: Option<String>,
    /// `--node`'s scope.
    pub node: Option<VerifyNode>,
    pub head_mode: HeadMode,
    pub judged: Option<Judged>,
    pub causes: Vec<RunCause>,
    pub messages: Vec<Message>,
    /// The config as named.
    pub label: &'a str,
    /// `delta.tests_reaching`'s candidates over every row judged (none
    /// when none was); `None` under `--node`.
    pub tests: Option<Vec<ReachingTest>>,
    /// `delta.symbols`: `--changed`'s alone.
    pub symbols: Option<CodeDelta>,
    /// `tested`: plain's, `--task`'s, `--node`'s; `None` in a base mode.
    pub tested: Option<Vec<TestedEntry>>,
}

/// The outcome: the mode the verdict uses, shown as such whether rows were
/// judged or a run cause stopped them; the rows only when no cause.
pub(crate) fn concluded(gathered: Gathered<'_>) -> VerifyOutcome {
    let Gathered {
        configured,
        has_base,
        base,
        task,
        node,
        head_mode,
        judged,
        mut causes,
        mut messages,
        label,
        tests,
        symbols,
        tested,
    } = gathered;
    let mode = configured
        .map(|configured| judged_mode(configured, has_base, &head_mode, label, &mut messages));

    causes.sort_by_key(|cause| cause.order);
    let causes_empty = causes.is_empty();
    let (rows, skipped): (Vec<VerifyRow>, usize) = match judged {
        Some(judged) if causes.is_empty() => {
            (judged.rows.iter().map(shown).collect(), judged.skipped)
        }
        _ => (Vec::new(), 0),
    };
    let findings = rows.iter().map(|row| Finding {
        state: row.state,
        pre_existing: row.pre_existing,
    });
    let verdict = verdict(mode.unwrap_or(Mode::Observe), !causes.is_empty(), findings);
    VerifyOutcome {
        mode,
        base,
        task,
        node,
        verdict,
        rows,
        skipped,
        cannot: causes
            .into_iter()
            .map(|cause| one_line(&cause.message))
            .collect(),
        tests: tests.map(|tests| if causes_empty { tests } else { Vec::new() }),
        symbols: symbols.filter(|_| causes_empty),
        tested: tested.filter(|_| causes_empty),
        messages,
    }
}

/// `spec verify --staged`: discovery on disk (`--config` read there, as
/// plain's), then every failure a run cause: (4) the index cannot be
/// listed, (5) each unmerged path; (1), (2) the index's config (or
/// `--config`'s); the index's side through the one-commit seam, its
/// causes (3), (4), (6); `HEAD`'s side over it (7).
fn verify_staged(
    env: &Env,
    globals: &Globals,
    request: &VerifyRequest,
) -> Result<VerifyOutcome, CliError> {
    let located = locate(env, globals)?;
    let given = match &globals.config {
        Some(_) => Some(discover_with_text(env, globals)?.1),
        None => None,
    };
    let label = located.config_label.as_str();
    let mut messages = Vec::new();
    let mut causes: Vec<RunCause> = Vec::new();

    // (4), (5): the index git names.
    let mut opened = match VerifyGit::staged(&located.root, &request.git) {
        Ok(opened) => Some(opened),
        Err(IndexRefusal::Git(cause)) => {
            causes.push(RunCause::new(CAUSE_GIT, cause.message));
            None
        }
        Err(IndexRefusal::Unmerged(paths)) => {
            causes.extend(paths.iter().map(|path| {
                RunCause::new(
                    CAUSE_UNMERGED,
                    if path == LOCK_FILE {
                        format!("{LOCK_FILE} is unmerged: run spec lock regen")
                    } else {
                        format!(
                            "`{path}` is unmerged in the git index: the conflict is not resolved"
                        )
                    },
                )
            }));
            None
        }
    };

    // (1), (2): the index's config, whole, unless `--config`.
    let staged_text = match (&given, &mut opened) {
        (None, Some((git, listing))) => match staged_config(git, listing, label) {
            Ok(text) => Some(text),
            Err(cause) => {
                causes.push(cause);
                None
            }
        },
        _ => None,
    };
    let mut project = None;
    let mut code = None;
    let mut configured = None;
    if let Some(text) = given.as_deref().or(staged_text.as_deref()) {
        match ProjectConfig::from_toml(text) {
            Ok(config) => {
                project = Some(ProjectRoot {
                    root: located.root.clone(),
                    config_label: label.to_owned(),
                    config,
                });
            }
            Err(error) => causes.push(RunCause::new(CAUSE_CONFIG, error.at(label))),
        }
        match CodeConfig::from_toml(text) {
            Ok(Some(read)) => code = Some(read),
            Ok(None) => causes.push(RunCause::new(
                CAUSE_NO_CODE,
                format!(
                    "{label} has no [code] table: the code layer is off (add [code] with roots)"
                ),
            )),
            Err(error) => causes.push(RunCause::new(CAUSE_CONFIG, error.at(label))),
        }
        match verify_from_toml(text) {
            Ok(config) => configured = Some(config.mode),
            Err(error) => causes.push(RunCause::new(CAUSE_CONFIG, error.at(label))),
        }
    }

    let mut base = Some("HEAD");
    let mut head_mode = HeadMode::Absent;
    let mut judged = None;
    let mut tests = Vec::new();
    match (causes.is_empty(), project, code, opened) {
        (true, Some(project), Some(code), Some((mut git, listing))) => {
            let config = staged_text.as_deref().map(str::as_bytes);
            match judge_index(env, globals, &project, code, &mut git, listing, config) {
                Ok(index) => {
                    if index.unborn {
                        base = Some("unborn");
                    }
                    head_mode = index.head_mode;
                    judged = Some(index.judged);
                    tests = index.tests;
                    messages.extend(index.lock_note.map(Message::Note));
                }
                Err(cause) => causes.push(cause),
            }
            if let Err(cause) = git.finish() {
                causes.push(RunCause::new(CAUSE_GIT, cause.message));
            }
        }
        // The session's end is no cause of its own: one is listed already.
        (.., Some((git, _))) => {
            let _ = git.finish();
        }
        _ => {}
    }
    Ok(concluded(Gathered {
        configured,
        has_base: true,
        base,
        task: None,
        node: None,
        head_mode,
        judged,
        causes,
        messages,
        label,
        tests: Some(tests),
        symbols: None,
        tested: None,
    }))
}

/// The index's root `specengine.toml`, its text. Causes: git's (4); not
/// staged, not a regular file or its blob unreadable, not UTF-8 (1).
fn staged_config(
    git: &mut VerifyGit,
    listing: &CommitListing,
    label: &str,
) -> Result<String, RunCause> {
    let mut files = git
        .read_files(listing, [CONFIG_FILE])
        .map_err(|cause| RunCause::new(CAUSE_GIT, cause.message))?;
    match files.remove(CONFIG_FILE) {
        None => Err(RunCause::new(
            CAUSE_CONFIG,
            format!("{label}: not staged in the git index"),
        )),
        Some(CommitFile::Unreadable(why)) => Err(RunCause::new(
            CAUSE_CONFIG,
            format!("{label} in the index: {why}"),
        )),
        Some(CommitFile::Bytes(bytes)) => String::from_utf8(bytes)
            .map_err(|_| RunCause::new(CAUSE_CONFIG, format!("{label}: the file is not UTF-8"))),
    }
}

/// What `--staged`'s judge gives: the rows, their tests reaching (over
/// the index's documents), `HEAD`'s mode, whether it is unborn, and the
/// note when `HEAD`'s lock gave way to the index's.
struct IndexJudged {
    judged: Judged,
    tests: Vec<ReachingTest>,
    head_mode: HeadMode,
    unborn: bool,
    lock_note: Option<String>,
}

/// The index's side read through the one-commit seam, `HEAD`'s read over
/// it, and its rows judged against `HEAD`'s; `config` the index's config
/// bytes when it is the config judged.
fn judge_index(
    env: &Env,
    globals: &Globals,
    project: &ProjectRoot,
    code: CodeConfig,
    git: &mut VerifyGit,
    listing: CommitListing,
    config: Option<&[u8]>,
) -> Result<IndexJudged, RunCause> {
    let side = read_commit(project, git, CommitConfig::of_index(listing, code.clone()))?;
    let config_path = config_path(env, globals, project);
    let share = IndexShare {
        side: &side,
        config,
    };
    let head = read_head(
        &side.walked(project, &code),
        git,
        config_path.as_deref(),
        Some(&share),
    )?;
    let judged = judge_commit(&side, project, Some(&head))?;
    let tests = reaching_tests(&judged.rows, side.input(), project);
    Ok(IndexJudged {
        judged,
        tests,
        unborn: head.is_unborn(),
        head_mode: head.mode,
        lock_note: head.lock_note,
    })
}

/// A state as its name.
fn state_name<S: serde::Serializer>(state: &State, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(state.as_str())
}

/// What [`run`] gave: the rows, `HEAD`'s mode, whether it is unborn, the
/// note when `HEAD`'s lock gave way to the disk's, and `--changed`'s
/// `delta.symbols`.
struct Ran {
    judged: Judged,
    head_mode: HeadMode,
    unborn: bool,
    lock_note: Option<String>,
    symbols: Option<CodeDelta>,
}

/// The base when asked, and the judge over the one walk; with the base and
/// `symbols`, the code items it and the walk differ by (the record builder
/// and comparer `spec impact --since HEAD` runs; unborn: every item added).
fn run(
    env: &Env,
    globals: &Globals,
    walked: &Walked<'_>,
    git: &mut VerifyGit,
    changed: bool,
    symbols: bool,
) -> Result<Ran, RunCause> {
    let head = if changed {
        let config = config_path(env, globals, walked.project);
        Some(read_head(walked, git, config.as_deref(), None)?)
    } else {
        None
    };
    let judged = judge_walk(walked, git, head.as_ref())?;
    let symbols = head.as_ref().filter(|_| symbols).map(|head| {
        let unborn = CodeTree::empty();
        let old = head.code(walked.tree).unwrap_or(&unborn);
        code_delta(old, walked.tree).delta
    });
    let (head_mode, unborn, lock_note) = match head {
        Some(head) => {
            let unborn = head.is_unborn();
            (head.mode, unborn, head.lock_note)
        }
        None => (HeadMode::Absent, false, None),
    };
    Ok(Ran {
        judged,
        head_mode,
        unborn,
        lock_note,
        symbols,
    })
}

/// The config's root-relative path, for `HEAD`'s counterpart: the root's
/// own, or `--config`'s when it lies under the root.
fn config_path(
    env: &Env,
    globals: &Globals,
    project: &crate::project::ProjectRoot,
) -> Option<String> {
    let Some(given) = &globals.config else {
        return Some(CONFIG_FILE.to_owned());
    };
    let full = fs::canonicalize(env.cwd.join(given)).ok()?;
    let relative = full.strip_prefix(&project.root).ok()?;
    let parts: Option<Vec<&str>> = relative
        .components()
        .map(|component| match component {
            std::path::Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The mode judged: with a base (`--changed`, `--staged`) the stricter of
/// the configured one and `HEAD`'s (a note when `HEAD`'s is stricter or
/// unknown); plain, `enforce-introduced` judged as `enforce`, with a note.
fn judged_mode(
    configured: Mode,
    has_base: bool,
    head: &HeadMode,
    label: &str,
    messages: &mut Vec<Message>,
) -> Mode {
    if !has_base {
        if configured == Mode::EnforceIntroduced {
            messages.push(Message::Note(
                "mode `enforce-introduced` has no base without --changed: judged as `enforce`"
                    .to_owned(),
            ));
            return Mode::Enforce;
        }
        return configured;
    }
    match head {
        HeadMode::Absent => configured,
        HeadMode::Known(at_head) if *at_head > configured => {
            messages.push(Message::Note(format!(
                "HEAD's {label} sets mode `{at_head}`, stricter than `{configured}`: verify runs \
                 in `{at_head}`"
            )));
            *at_head
        }
        HeadMode::Known(_) => configured,
        HeadMode::Unknown(why) => {
            messages.push(Message::Note(format!(
                "{why}: HEAD's mode is unknown, so verify runs in `{configured}`"
            )));
            configured
        }
    }
}

/// A CLI error's message without its `spec: ` prefix.
pub(crate) fn unprefixed(error: &CliError) -> String {
    error
        .message
        .strip_prefix("spec: ")
        .unwrap_or(&error.message)
        .to_owned()
}

/// A judged row as shown (a drift item stores it too, task spec
/// `drift-queue`, "The item").
pub(crate) fn shown(row: &Row) -> VerifyRow {
    let (kind, symbol, item_kind, keyless) = match &row.key.binding {
        RowBinding::Keyed {
            kind,
            symbol,
            item_kind,
        } => (
            Some(kind.as_str()),
            Some(symbol.clone()),
            item_kind.clone(),
            None,
        ),
        RowBinding::Keyless { state, category } => {
            (None, None, None, Some((state.clone(), category.clone())))
        }
    };
    let judgement = &row.judgement;
    VerifyRow {
        node: row.key.node.clone(),
        relation: row.key.relation.as_str(),
        file: row.key.file.clone(),
        kind,
        symbol,
        item_kind,
        state: judgement.state,
        category: judgement.category.clone(),
        detail: judgement.detail.clone(),
        class: judgement.state.class().map(|class| class.as_str()),
        pre_existing: row.pre_existing,
        markers: row
            .markers
            .iter()
            .map(|marker| VerifyMarker {
                line: marker.line,
                rev: match marker.rev {
                    specengine_core::verify::MarkerRev::Written(n) => Some(n),
                    _ => None,
                },
            })
            .collect(),
        node_rev: row.node_rev,
        lock: row.entry.as_ref().map(|entry| VerifyLock {
            rev: entry.rev,
            commit: entry.commit.clone(),
            verified_at: entry.verified_at.clone(),
        }),
        compared: judgement.compared.map(|compared| compared.as_str()),
        levels: row.levels.as_ref().map(|levels| VerifyLevels {
            lock: level_names(&levels.lock),
            markers: level_names(&levels.markers),
            changed: levels.changed.as_ref().map(level_names),
        }),
        anchor: row.anchor.as_ref().map(|anchor| match anchor {
            Anchor::Exact => VerifyAnchor {
                state: EXACT,
                from: None,
                candidates: None,
            },
            Anchor::Moved(from) => VerifyAnchor {
                state: MOVED,
                from: Some(place(from)),
                candidates: None,
            },
            Anchor::Ambiguous(candidates) => VerifyAnchor {
                state: AMBIGUOUS,
                from: None,
                candidates: Some(candidates.iter().map(place).collect()),
            },
            Anchor::Lost => VerifyAnchor {
                state: LOST,
                from: None,
                candidates: None,
            },
        }),
        keyless,
        resolved: row.resolved,
        key_problem: row
            .key
            .entry_key()
            .as_ref()
            .and_then(key_problem)
            .map(|problem| problem.to_string()),
    }
}

/// A lock key's place, as an anchor shows it.
fn place(key: &EntryKey) -> VerifyPlace {
    VerifyPlace {
        file: key.file.clone(),
        kind: key.kind.as_str(),
        symbol: key.symbol.clone(),
        item_kind: key.item_kind.clone(),
    }
}

/// The row's anchor state.
fn anchor_state(row: &VerifyRow) -> Option<&'static str> {
    row.anchor.as_ref().map(|anchor| anchor.state)
}

/// Levels' names, in their order.
fn level_names<'a>(levels: impl IntoIterator<Item = &'a LockLevel>) -> Vec<&'static str> {
    levels.into_iter().map(|level| level.as_str()).collect()
}

// ---------------------------------------------------------------- output

/// The counts of the summary and the JSON.
struct Counts {
    rows: usize,
    ok: usize,
    fix: usize,
    check: usize,
    cannot: usize,
    pre_existing: usize,
    by_blob: usize,
    by_ast: usize,
}

fn counts(outcome: &VerifyOutcome) -> Counts {
    let class = |name: &str| {
        outcome
            .rows
            .iter()
            .filter(|row| row.class == Some(name))
            .count()
    };
    let compared = |name: &str| {
        outcome
            .rows
            .iter()
            .filter(|row| row.compared == Some(name))
            .count()
    };
    Counts {
        rows: outcome.rows.len(),
        ok: outcome
            .rows
            .iter()
            .filter(|row| row.class.is_none())
            .count(),
        fix: class("fix"),
        check: class("check"),
        cannot: class("cannot"),
        pre_existing: outcome.rows.iter().filter(|row| row.pre_existing).count(),
        by_blob: compared("blob"),
        by_ast: compared("ast"),
    }
}

/// A row's line ([`FindingLine`]); its binding a keyless row's words, else
/// [`keyed_binding`], else empty; an `ok` row's class `-` (only `--node`
/// prints one).
fn row_line(row: &VerifyRow) -> String {
    let binding = match &row.keyless {
        Some((word, category)) => format!("{word} {category}"),
        None => keyed_binding(row.kind, row.symbol.as_deref(), row.item_kind.as_deref())
            .unwrap_or_default(),
    };
    FindingLine {
        pre_existing: row.pre_existing,
        class: row.class.unwrap_or(OK_CLASS),
        state: row.state.as_str(),
        category: row.category.as_deref(),
        detail: row.detail.as_deref(),
        node: &row.node,
        relation: row.relation,
        file: &row.file,
        binding: &binding,
    }
    .render()
}

/// One row as `spec verify` prints it (a drift item's review prints its
/// stored rows so): `<class> | <state>[ <category>][ <detail>] | <node> |
/// <relation> | <file> | <binding>`, `pre-existing ` before the class;
/// each part on one line.
pub(crate) struct FindingLine<'a> {
    pub pre_existing: bool,
    pub class: &'a str,
    pub state: &'a str,
    pub category: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub node: &'a str,
    pub relation: &'a str,
    pub file: &'a str,
    /// `<kind> <symbol>[ <item_kind>]` ([`keyed_binding`]), or what the
    /// caller shows without one.
    pub binding: &'a str,
}

impl FindingLine<'_> {
    pub(crate) fn render(&self) -> String {
        let mut state = one_line(self.state);
        for part in [self.category, self.detail].into_iter().flatten() {
            state.push(' ');
            state.push_str(&one_line(part));
        }
        format!(
            "{}{} | {state} | {} | {} | {} | {}",
            if self.pre_existing {
                "pre-existing "
            } else {
                ""
            },
            one_line(self.class),
            one_line(self.node),
            one_line(self.relation),
            one_line(self.file),
            one_line(self.binding),
        )
    }
}

/// A keyed row's binding: `<kind> <symbol>[ <item_kind>]`; `None` without
/// a kind or a symbol.
pub(crate) fn keyed_binding(
    kind: Option<&str>,
    symbol: Option<&str>,
    item_kind: Option<&str>,
) -> Option<String> {
    let (kind, symbol) = (kind?, symbol?);
    let mut binding = format!("{kind} {}", one_line(symbol));
    if let Some(item_kind) = item_kind {
        binding.push(' ');
        binding.push_str(&one_line(item_kind));
    }
    Some(binding)
}

/// The next step for text changed at the locked revision (accept refuses
/// it): no proposal raises a revision, approval sets it.
const SAME_REVISION_STEP: &str = "the node's text changed at its locked revision: raise its \
     `rev` by hand (`spec bump` prints the line while the edit is uncommitted), commit, then \
     accept; or `--editorial` for an editorial change (`spec lock accept <ID> --editorial`); \
     accept refuses text changed at a revision";

/// The hint after the per-state ones when a row moved, `ok` too (canon
/// `spec-verify-anchors`, "Output"): accept re-keys its entry.
const MOVED_HINT: &str = "hint: moved: commit, then spec lock accept <ID> on a terminal locks the \
     new key and drops the old";

/// The hint after the others when a row is drift a queue item takes (task
/// spec `drift-queue`, "report_drift and texts").
const DRIFT_HINT: &str = "hint: drift: commit first; spec propose drift queues HEAD's drift for \
     the owner";

/// One hint per state and category present (`unlocked key_problem` apart):
/// the next step, as accept takes it; then, once, the moved hint when a row
/// moved; then, once, the drift hint when a row raises an item in a sweep
/// (`drift::raises`, no node named).
fn hints(rows: &[VerifyRow]) -> Vec<String> {
    let mut hints = state_hints(rows);
    if rows.iter().any(|row| anchor_state(row) == Some(MOVED)) {
        hints.push(MOVED_HINT.to_owned());
    }
    let drifting = rows
        .iter()
        .any(|row| raises(row.state, row.detail.as_deref(), false));
    if drifting {
        hints.push(DRIFT_HINT.to_owned());
    }
    hints
}

/// One hint per state and category present (`unlocked key_problem` apart):
/// the next step, as accept takes it.
fn state_hints(rows: &[VerifyRow]) -> Vec<String> {
    let mut present: BTreeSet<(State, Option<String>)> = BTreeSet::new();
    let mut key_problems: BTreeSet<String> = BTreeSet::new();
    // Conflict rows at the entry's revision, and at another.
    let mut conflicts = (false, false);
    for row in rows.iter().filter(|row| row.class.is_some()) {
        let state = row.state;
        if state == State::Conflict {
            let same = row
                .lock
                .as_ref()
                .is_some_and(|lock| row.node_rev == Some(lock.rev));
            if same {
                conflicts.0 = true;
            } else {
                conflicts.1 = true;
            }
        }
        let label = if row.detail.as_deref() == Some("key_problem") {
            key_problems.extend(row.key_problem.clone());
            Some("key_problem".to_owned())
        } else {
            row.category.clone()
        };
        present.insert((state, label));
    }
    present
        .into_iter()
        .map(|(state, label)| {
            let step = match (state, label.as_deref()) {
                (State::CannotVerify, Some("item_error")) => {
                    "fix the item's parse error, or move the marker to an item that parses; no \
                     lock vouches for it meanwhile (`spec refs <ID>`)"
                        .to_owned()
                }
                (State::CannotVerify, Some("ambiguous_symbol")) => {
                    "make the binding name one symbol (no `#[path]` target, a package for the \
                     file, no twin), or move the marker (`spec refs <ID>`)"
                        .to_owned()
                }
                (State::CannotVerify, Some("ambiguous_id")) => {
                    "the ID names several nodes: cite one (`slug/ID`, `ID#SECTION`) or make the \
                     IDs unique"
                        .to_owned()
                }
                (State::CannotVerify, Some("rev_unknown")) => {
                    "write the node's `rev` as a number from 1 (`spec show <ID>`)".to_owned()
                }
                (State::CannotVerify, Some("hash_null")) => {
                    "fix what nulls the node's meaning hash (`spec check` names it)".to_owned()
                }
                (State::CannotVerify, Some("unreadable")) => {
                    "make the file readable UTF-8 under [code], or restore the binding \
                     (`spec refs <ID>`)"
                        .to_owned()
                }
                (State::CannotVerify, Some("hidden_by_error")) => {
                    "fix the parse error before the symbol (`spec refs <ID>`)".to_owned()
                }
                (State::BrokenBinding, Some("node_gone")) => {
                    "commit first (the node is gone only when HEAD agrees), then `spec lock \
                     accept <old ID>` on a terminal drops its entries"
                        .to_owned()
                }
                (State::BrokenBinding | State::Unlocked | State::CodeAhead, _)
                    if label.as_deref() != Some("key_problem") =>
                {
                    "commit, then `spec lock accept <ID>` on a terminal".to_owned()
                }
                (State::Unlocked, _) => format!(
                    "spec.lock cannot hold the key ({}): rename what gives it; `spec lock \
                     accept` cannot lock it",
                    key_problems.iter().cloned().collect::<Vec<_>>().join("; ")
                ),
                (State::SpecAhead, Some("same_revision")) => SAME_REVISION_STEP.to_owned(),
                (State::Conflict, _) => {
                    // By R against the entry's revision: text changed at
                    // the locked revision, or a revision the marker never
                    // reached.
                    let mut steps = Vec::new();
                    if conflicts.0 {
                        steps.push(SAME_REVISION_STEP);
                    }
                    if conflicts.1 {
                        steps.push(
                            "at a revision the lock does not hold, the marker not raised: \
                             check the code against the node, set the marker to its \
                             revision, commit, then `spec lock accept <ID>` on a terminal",
                        );
                    }
                    steps.join("; ")
                }
                _ => "restore, raise or lower the marker (`spec refs <ID>`)".to_owned(),
            };
            let mut head = state.as_str().to_owned();
            if let Some(label) = label {
                head.push(' ');
                head.push_str(&label);
            }
            format!("hint: {head}: {step}")
        })
        .collect()
}

/// An `ok` row's class as `--node` prints it.
const OK_CLASS: &str = "-";

/// A printed row's block: its line, then its moved row's old place or its
/// ambiguous row's candidates, each line ended.
fn row_block(row: &VerifyRow) -> String {
    let mut out = row_line(row);
    out.push('\n');
    if let Some(anchor) = &row.anchor {
        for (word, place) in anchor.from.iter().map(|from| ("moved from", from)).chain(
            anchor
                .candidates
                .iter()
                .flatten()
                .map(|to| ("candidate", to)),
        ) {
            out.push_str(&place_line(word, place));
            out.push('\n');
        }
    }
    out
}

/// The rows printed, and how many are not: plain and `--task` print each
/// non-`ok` row, uncut; `--node` (task spec `verify-node`) every row, `ok`
/// too, while their blocks stay within [`OUTPUT_CAP_CHARS`] characters,
/// then none (the JSON's `rows` and `omitted`).
fn printed(outcome: &VerifyOutcome) -> (Vec<&VerifyRow>, usize) {
    if outcome.node.is_none() {
        let rows = outcome
            .rows
            .iter()
            .filter(|row| row.class.is_some())
            .collect();
        return (rows, 0);
    }
    let mut used = 0usize;
    let mut rows = Vec::new();
    for row in &outcome.rows {
        let block = row_block(row).chars().count();
        if used + block > OUTPUT_CAP_CHARS {
            break;
        }
        used += block;
        rows.push(row);
    }
    let omitted = outcome.rows.len() - rows.len();
    (rows, omitted)
}

/// The text report: the rows printed in key order, each followed by its
/// moved row's old place or its ambiguous row's candidates (`--node`: the
/// tail when cut), the `cannot:` lines, the hints, the summary.
pub(crate) fn render_text(outcome: &VerifyOutcome) -> String {
    let mut out = String::new();
    let (rows, omitted) = printed(outcome);
    for row in rows {
        out.push_str(&row_block(row));
    }
    if omitted > 0 {
        out.push_str(&format!(
            "[truncated: {omitted} rows not shown; narrow with --symbol QPATH]\n"
        ));
    }
    for cause in &outcome.cannot {
        out.push_str(&format!("cannot: {}\n", one_line(cause)));
    }
    for hint in hints(&outcome.rows) {
        out.push_str(&hint);
        out.push('\n');
    }
    // Each block not `verified` (task spec `tested-evidence`), outside
    // `--node`'s cut.
    for entry in outcome.tested.iter().flatten() {
        if entry.state == TestedState::Verified {
            continue;
        }
        let state = match entry.state.category() {
            Some(category) => format!("{} {category}", entry.state.as_str()),
            None => entry.state.as_str().to_owned(),
        };
        out.push_str(&format!(
            "tested | {} | {}\n",
            one_line(&state),
            one_line(&entry.node)
        ));
    }
    let counts = counts(outcome);
    let mode = outcome
        .mode
        .map(|mode| format!(" [{mode}]"))
        .unwrap_or_default();
    let pre_existing = if outcome.base.is_some() {
        format!(" ({} pre-existing)", counts.pre_existing)
    } else {
        String::new()
    };
    let moved = outcome
        .rows
        .iter()
        .filter(|row| row.class.is_none() && anchor_state(row) == Some(MOVED))
        .count();
    let moved = if moved > 0 {
        format!(" ({moved} moved)")
    } else {
        String::new()
    };
    // `--task`: `verify --task <T>: ` (task spec `verify-task`); `--node`:
    // `verify --node <REF>[ --symbol <QPATH>]: ` (task spec `verify-node`).
    let command = match (&outcome.task, &outcome.node) {
        (Some(task), _) => format!("verify --task {task}"),
        (None, Some(node)) => {
            let mut command = format!("verify --node {}", one_line(&node.reference));
            if let Some(symbol) = &node.symbol {
                command.push_str(" --symbol ");
                command.push_str(&one_line(symbol));
            }
            command
        }
        (None, None) => "verify".to_owned(),
    };
    out.push_str(&format!(
        "{command}: {}{mode}: {} fix, {} check{pre_existing}, {} cannot, {} ok{moved}; {} by \
         blob, {} by ast; {} skipped\n",
        outcome.verdict.as_str(),
        counts.fix,
        counts.check,
        counts.cannot,
        counts.ok,
        counts.by_blob,
        counts.by_ast,
        outcome.skipped
    ));
    escape_controls(&out)
}

/// A sub-line under a row: `  <word> <file> | <kind> <symbol>[
/// <item_kind>]`.
fn place_line(word: &str, place: &VerifyPlace) -> String {
    let binding = keyed_binding(
        Some(place.kind),
        Some(&place.symbol),
        place.item_kind.as_deref(),
    )
    .unwrap_or_default();
    format!("  {word} {} | {binding}", one_line(&place.file))
}

/// A row's key in the JSON's `delta`.
#[derive(Serialize)]
struct KeyJson<'a> {
    node: &'a str,
    relation: &'a str,
    file: &'a str,
    kind: Option<&'a str>,
    symbol: Option<&'a str>,
    item_kind: Option<&'a str>,
}

impl<'a> KeyJson<'a> {
    /// Its six fields, compared.
    fn fields(
        &self,
    ) -> (
        &'a str,
        &'a str,
        &'a str,
        Option<&'a str>,
        Option<&'a str>,
        Option<&'a str>,
    ) {
        (
            self.node,
            self.relation,
            self.file,
            self.kind,
            self.symbol,
            self.item_kind,
        )
    }

    fn of(row: &'a VerifyRow) -> Self {
        Self {
            node: &row.node,
            relation: row.relation,
            file: &row.file,
            kind: row.kind,
            symbol: row.symbol.as_deref(),
            item_kind: row.item_kind.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct CountsJson {
    rows: usize,
    ok: usize,
    fix: usize,
    check: usize,
    cannot: usize,
    pre_existing: usize,
    skipped: usize,
}

#[derive(Serialize)]
struct CascadeJson {
    by_blob: usize,
    by_ast: usize,
}

/// A moved row in the JSON's `delta`: its entry's key and its own.
#[derive(Serialize)]
struct MovedJson<'a> {
    from: KeyJson<'a>,
    to: KeyJson<'a>,
}

#[derive(Serialize)]
struct BindingsJson<'a> {
    moved: Vec<MovedJson<'a>>,
    ambiguous: Vec<KeyJson<'a>>,
    lost: Vec<KeyJson<'a>>,
    stale: Vec<KeyJson<'a>>,
}

#[derive(Serialize)]
struct VerifyDeltaJson<'a> {
    /// `--changed`'s code items (task spec `impact-since`), uncapped.
    symbols: Option<DeltaJson<'a>>,
    bindings: BindingsJson<'a>,
    affected_nodes: Vec<&'a str>,
    /// The keys of the `verifies` rows reaching an affected node; `null`
    /// under `--node`.
    tests_reaching: Option<Vec<KeyJson<'a>>>,
}

/// The rows `--node`'s cut leaves out (task spec `verify-node`).
#[derive(Serialize)]
struct OmittedJson {
    rows: usize,
}

/// One `[[tested]]` block's state in the JSON's `tested`.
#[derive(Serialize)]
struct TestedJson<'a> {
    node: &'a str,
    state: &'static str,
    category: Option<&'a str>,
    rev: u32,
    node_rev: Option<u32>,
    commit: &'a str,
    verified_at: &'a str,
}

#[derive(Serialize)]
struct VerifyJson<'a> {
    mode: Option<&'static str>,
    base: Option<&'static str>,
    /// `--task`'s ID, `null` without it (task spec `verify-task`).
    task: Option<&'a str>,
    /// `--node`'s holders' IDs and `--symbol`'s QPATH, `null` without
    /// `--node` (task spec `verify-node`).
    node: Option<&'a [String]>,
    symbol: Option<&'a str>,
    verdict: &'static str,
    counts: CountsJson,
    cascade: CascadeJson,
    /// The rows the text prints (`--node`) or every row, `ok` too.
    rows: Vec<&'a VerifyRow>,
    delta: VerifyDeltaJson<'a>,
    /// Each block's state by node bytes; `null` under a base or a run
    /// cause (task spec `tested-evidence`).
    tested: Option<Vec<TestedJson<'a>>>,
    cannot: &'a [String],
    notes: Vec<String>,
    /// `--node`'s rows not printed; `null` without `--node`.
    omitted: Option<OmittedJson>,
}

impl Serialize for VerifyOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let counts = counts(self);
        // Over the rows not Pre-existing.
        let introduced: Vec<&VerifyRow> =
            self.rows.iter().filter(|row| !row.pre_existing).collect();
        let keys = |wanted: &dyn Fn(&VerifyRow) -> bool| -> Vec<KeyJson<'_>> {
            introduced
                .iter()
                .filter(|row| wanted(row))
                .map(|row| KeyJson::of(row))
                .collect()
        };
        let affected: BTreeSet<&str> = introduced
            .iter()
            .filter(|row| row.class.is_some() && row.resolved)
            .map(|row| row.node.as_str())
            .collect();
        // Over every row judged (`--task`: before its filter): a test
        // whose node is affected or holds a `verifies` link into one.
        let tests_reaching = self.tests.as_ref().map(|tests| {
            let mut seen = BTreeSet::new();
            tests
                .iter()
                .filter(|test| {
                    test.reaches
                        .iter()
                        .any(|node| affected.contains(node.as_str()))
                })
                .map(|test| KeyJson::of(&test.row))
                // Distinct six-field keys: keyless rows of one place share one.
                .filter(|key| seen.insert(key.fields()))
                .collect()
        });
        // `--node`: the rows its text prints; plain and `--task`: every row.
        let (rows, omitted) = match &self.node {
            Some(_) => {
                let (rows, omitted) = printed(self);
                (rows, Some(OmittedJson { rows: omitted }))
            }
            None => (self.rows.iter().collect(), None),
        };
        let document = VerifyJson {
            mode: self.mode.map(Mode::as_str),
            base: self.base,
            task: self.task.as_deref(),
            node: self.node.as_ref().map(|node| node.ids.as_slice()),
            symbol: self.node.as_ref().and_then(|node| node.symbol.as_deref()),
            verdict: self.verdict.as_str(),
            counts: CountsJson {
                rows: counts.rows,
                ok: counts.ok,
                fix: counts.fix,
                check: counts.check,
                cannot: counts.cannot,
                pre_existing: counts.pre_existing,
                skipped: self.skipped,
            },
            cascade: CascadeJson {
                by_blob: counts.by_blob,
                by_ast: counts.by_ast,
            },
            rows,
            delta: VerifyDeltaJson {
                symbols: self
                    .symbols
                    .as_ref()
                    .map(|delta| DeltaJson::printed(delta, None)),
                bindings: BindingsJson {
                    moved: introduced
                        .iter()
                        .filter_map(|row| {
                            let from = row.anchor.as_ref()?.from.as_ref()?;
                            Some(MovedJson {
                                from: KeyJson {
                                    node: &row.node,
                                    relation: row.relation,
                                    file: &from.file,
                                    kind: Some(from.kind),
                                    symbol: Some(&from.symbol),
                                    item_kind: from.item_kind.as_deref(),
                                },
                                to: KeyJson::of(row),
                            })
                        })
                        .collect(),
                    ambiguous: keys(&|row| {
                        let symbol = row.state == State::CannotVerify
                            && row
                                .category
                                .as_deref()
                                .is_some_and(|category| category.starts_with("ambiguous_"));
                        symbol || anchor_state(row) == Some(AMBIGUOUS)
                    }),
                    lost: keys(&|row| {
                        row.state == State::BrokenBinding && anchor_state(row) != Some(AMBIGUOUS)
                    }),
                    stale: keys(&|row| {
                        row.state == State::CodeAhead || row.state == State::Conflict
                    }),
                },
                affected_nodes: affected.into_iter().collect(),
                tests_reaching,
            },
            tested: self.tested.as_ref().map(|entries| {
                entries
                    .iter()
                    .map(|entry| TestedJson {
                        node: &entry.node,
                        state: entry.state.as_str(),
                        category: entry.state.category(),
                        rev: entry.rev,
                        node_rev: entry.node_rev,
                        commit: &entry.commit,
                        verified_at: &entry.verified_at,
                    })
                    .collect()
            }),
            cannot: &self.cannot,
            notes: self.notes(),
            omitted,
        };
        document.serialize(serializer)
    }
}
