//! `spec verify --tests [--task T] [--json]` (task spec `verify-tests`):
//! the tests `@verifies` binds to the selected nodes, run by the project's
//! own command (`[verify] tests`, an argv template; `timeout` seconds per
//! invocation), one invocation per distinct argv, only after the owner's
//! one `[y/N]` (`main` refuses a stdin that is no terminal before anything
//! is read), over a clean committed tree; each run that reaches the probe
//! is one `verify.tests` event. Nothing here names a runner or parses its
//! output: exit 0 passes, anything else fails.
//!
//! In order ("Order"; a failure before discovery, an unknown task: exit 2,
//! stdout empty; any other cause after discovery: verdict `cannot`):
//!
//! 1. Unix only ([`crate::test_runner`]); the clock checked;
//! 2. discovery; `[verify]` (`tests` required), `[code]`;
//! 3. git: a worktree, `HEAD` born, on a branch, no operation in progress;
//! 4. the whole worktree clean (status, no pathspec, untracked files too):
//!    else `dirty`, nothing more read; `specengine.toml` a file of `HEAD`;
//! 5. the committer; the queue opened as `spec task` opens it; with
//!    `--task` the task ([`crate::node_set::task_set`]);
//! 6. the corpus, the code walk, the set, the tests ("Runnable") and the
//!    reported nodes ("Verifiable": accept's own predicate,
//!    [`crate::lock::committed_bindings`]); each planned program run from
//!    the root's tree a file of `HEAD`; nothing to run: `empty`;
//! 7. the listing and the question; anything but `y`: `declined`;
//! 8. the probes (`{test}` a test no project has): one exiting 0 or timing
//!    out, or a program that cannot start: `refused`;
//! 9. each distinct argv once, by its first test's (`file`, `symbol`);
//! 10. `HEAD`, the branch and the status read again: a difference is
//!     `changed`;
//! 11. the event, then stdout.
//!
//! Nodes get a verdict only under `verified`, `unverified` (step 9 done,
//! step 10 unchanged) and `empty`; every other end reports none.
//!
//! From step 8 to the end of step 9 a `SIGINT`, `SIGTERM` or `SIGHUP` to
//! `spec` is forwarded to the running group: `interrupted`, exit 2. Exit 1
//! means "not verified" (the owner's test run, outside verify's judge),
//! never a Fix finding. Writes only the data directory (the index refresh,
//! the queue when absent, the event); what the project's command writes is
//! its own (step 10 sees it).

// Off Unix only the refusal is built; the rest is the runner's.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use specengine_code::{
    FileRole, NamedFile, PackageTargets, PlacedNames, QPath, Relation, Target as CargoTarget,
    TargetIndex, UnitKind, markers_in, placed_name_file,
};
use specengine_core::CodeConfig;
use specengine_core::lock::Lock;
use specengine_core::verify::{
    NodeVerdict, PROBE_TEST, PlaceholderValues, RunVerdict, TestOutcome, TestTemplate,
    node_verdict, tests_verdict, verify_from_toml,
};
use specengine_model::IdScheme;
use specengine_store::{GitEnv, PlaceError, StatusEntry, Task, WorktreeGit};

use crate::apply::Consent;
use crate::code_walk::{CodeTree, FilePackage, read_tree, tree_packages};
use crate::corpus::indexed;
use crate::lock::{Facts, NodeAt, Repo, Target, build_recipes, committed_bindings, node_at_head};
use crate::node_set::{NodeSet, canonical_of, named_task, task_set};
use crate::project::{CONFIG_FILE, ProjectRoot, discover_with_text};
use crate::proposals::{QueueContext, checked_now, escaped_error, open_context};
use crate::refs::{Corpus, NodeKey, RON};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The refusal off Unix ("Order" step 1).
#[cfg(not(unix))]
const UNIX_ONLY: &str = "spec verify --tests runs on Unix only";

/// The probe's refusal when the command passed it ("Order" step 8).
const PROBE_PASSED: &str = "the [verify] tests command passed a test that does not exist; make \
                            it fail when no test matches";

/// The tree-sitter kind of a free function, the one runnable item.
const FUNCTION_ITEM: &str = "function_item";

/// A module binding's kind, as rows print it.
const MODULE: &str = "module";

/// An item binding's kind, as rows print it.
const ITEM: &str = "item";

/// Why a bound test is not run ("Runnable"), as rows print it.
const NOT_COMMITTED: &str = "not committed";
const NOT_RUN_MODULE: &str = "module";
const NOT_RUN_RON: &str = "ron";
const UNROOTED: &str = "unrooted";
const SHARED_UNIT: &str = "shared unit";
const MEMBER: &str = "member";
const NOT_A_FUNCTION: &str = "not a function";
const NODE_NOT_VERIFIABLE: &str = "node not verifiable";

/// A node whose `HEAD` revision cannot be read, or whose `norm_hash` is
/// null there (canon `spec-verify`, "States").
const REV_UNKNOWN: &str = "rev_unknown";
const HASH_NULL: &str = "hash_null";

/// `spec verify --tests` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyTestsRequest {
    /// `--task T` as given.
    pub task: Option<String>,
    /// The clock: the event's time, the predicate's day.
    pub now: String,
    /// Git's environment; also the runner's (its repository variables
    /// dropped, as git's own).
    pub git: GitEnv,
}

/// One bound test: a `@verifies` binding in the set, once per (`file`,
/// `symbol`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyTestsRow {
    pub file: String,
    /// `item`, `module`, `ron`, or a keyless binding's state.
    pub kind: String,
    /// The binding's name (a keyless binding's category).
    pub symbol: String,
    /// Its set nodes' canonical IDs, by bytes.
    pub nodes: Vec<String>,
    /// The expanded template, element 0 as written; `None` when not
    /// runnable.
    pub argv: Option<Vec<String>>,
    pub outcome: TestOutcome,
    pub exit: Option<i32>,
    pub signal: Option<i32>,
    /// Why it was not run.
    pub reason: Option<String>,
    /// The event's: `b3:` of the whole output of its run.
    pub output_hash: Option<String>,
    /// The event's: a failed or timed-out run's last bytes (lossy UTF-8).
    pub tail: Option<String>,
}

/// One reported node: a `@verifies` binding of the set names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifyTestsNode {
    pub id: String,
    #[serde(serialize_with = "node_verdict_name")]
    pub verdict: NodeVerdict,
    /// Not verifiable: the predicate's first refusal.
    pub state: Option<String>,
}

/// One probe that started (the event's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyTestsProbe {
    pub argv: Vec<String>,
    pub exit: Option<i32>,
    pub signal: Option<i32>,
    pub outcome: TestOutcome,
    pub output_hash: Option<String>,
}

/// What `spec verify --tests` found and ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyTestsOutcome {
    pub task: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    /// The template as written.
    pub command: Option<Vec<String>>,
    pub timeout: Option<u32>,
    pub verdict: RunVerdict,
    /// Why, for `dirty`, `empty`, `declined`, `changed`, `refused`,
    /// `interrupted`.
    pub reason: Option<String>,
    /// The probes that started (the event's).
    pub probes: Vec<VerifyTestsProbe>,
    /// The distinct probe argv the run plans, started or not (the
    /// document's `counts.probes`).
    pub probes_planned: usize,
    /// By (`file`, `symbol`) bytes.
    pub tests: Vec<VerifyTestsRow>,
    /// By ID bytes; only under `verified`, `unverified` and `empty`.
    pub nodes: Vec<VerifyTestsNode>,
    /// The cause of a `cannot`.
    pub cannot: Vec<String>,
    pub messages: Vec<Message>,
}

impl VerifyTestsOutcome {
    /// 0 verified; 1 not verified; 2 refused, interrupted, cannot.
    pub fn exit(&self) -> Exit {
        match self.verdict.exit_code() {
            0 => Exit::Answered,
            1 => Exit::NotFound,
            _ => Exit::CannotRun,
        }
    }

    fn new(task: Option<String>) -> Self {
        Self {
            task,
            branch: None,
            commit: None,
            command: None,
            timeout: None,
            verdict: RunVerdict::Cannot,
            reason: None,
            probes: Vec::new(),
            probes_planned: 0,
            tests: Vec::new(),
            nodes: Vec::new(),
            cannot: Vec::new(),
            messages: Vec::new(),
        }
    }

    /// `cannot`, with its cause.
    fn cannot(mut self, cause: String) -> Self {
        self.verdict = RunVerdict::Cannot;
        self.nodes.clear();
        self.cannot = vec![one_line(&cause)];
        self
    }

    /// A verdict with its reason.
    fn stopped(mut self, verdict: RunVerdict, reason: String) -> Self {
        self.verdict = verdict;
        self.reason = Some(one_line(&reason));
        self
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

/// `spec verify --tests` (see the module documentation): an exit 2 before
/// discovery, or for a task that cannot be named, is the error; every
/// other end is the outcome.
pub fn verify_tests(
    env: &Env,
    globals: &Globals,
    request: &VerifyTestsRequest,
    consent: Consent<'_>,
) -> Result<VerifyTestsOutcome, CliError> {
    #[cfg(unix)]
    {
        run(env, globals, request, consent).map_err(escaped_error)
    }
    #[cfg(not(unix))]
    {
        let _ = (env, globals, request, consent);
        Err(CliError::spec(UNIX_ONLY))
    }
}

/// What steps 2 to 6 found for the run.
struct Prepared {
    /// The project root, canonical: every invocation's directory.
    root: PathBuf,
    template: TestTemplate,
    timeout: u32,
    repo: Repo,
    by: String,
    context: QueueContext,
    /// Each distinct argv and the tests (indexes into `tests`) it runs,
    /// by its first test.
    runs: Vec<(Vec<String>, Vec<usize>)>,
    /// The probes' argv, distinct, by their first run.
    probes: Vec<Vec<String>>,
    /// Per reported node (by ID): verifiable, and its runnable tests.
    nodes: Vec<(String, Option<String>, Vec<usize>)>,
}

#[cfg(unix)]
fn run(
    env: &Env,
    globals: &Globals,
    request: &VerifyTestsRequest,
    consent: Consent<'_>,
) -> Result<VerifyTestsOutcome, CliError> {
    checked_now(&request.now)?;
    let mut outcome = VerifyTestsOutcome::new(request.task.clone());
    let prepared = match prepare(env, globals, request, &mut outcome)? {
        Ok(prepared) => prepared,
        Err(done) => return Ok(*done),
    };

    // 7: the listing and the question.
    let mut lines: Vec<String> = outcome.tests.iter().map(listing_line).collect();
    for node in &outcome.nodes {
        if let Some(state) = &node.state {
            lines.push(escape_controls(&one_line(&format!(
                "node {}: not verifiable {state}",
                node.id
            ))));
        }
    }
    let verifiable = prepared
        .nodes
        .iter()
        .filter(|(_, state, _)| state.is_none())
        .count();
    lines.push(escape_controls(&one_line(&format!(
        "run {} bound tests of {verifiable} nodes as {} (timeout {} s each) in {} on {} at {}? \
         [y/N]",
        prepared.runs.len(),
        shown_template(prepared.template.written()),
        prepared.timeout,
        prepared.repo.worktree,
        prepared.repo.branch,
        prepared.repo.head
    ))));
    // From here on a node gets a verdict only from a completed step 9 over a
    // tree step 10 finds unchanged ("Verdicts and event").
    outcome.nodes.clear();
    if !consent(&lines.join("\n")) {
        not_run(&mut outcome, "declined");
        return Ok(outcome.stopped(
            RunVerdict::Declined,
            "the owner declined; nothing ran".to_owned(),
        ));
    }

    // 8, 9: the probes, then the runs, signals forwarded.
    let ended = run_all(&prepared, &mut outcome);
    let mut prepared = prepared;
    match ended {
        Stop::Done => {
            // 10: the tree read again.
            match changed(&prepared.repo) {
                Some(reason) => {
                    outcome = outcome.stopped(RunVerdict::Changed, reason);
                }
                None => {
                    finish_nodes(&mut outcome, &prepared);
                    outcome.verdict = tests_verdict(outcome.nodes.iter().map(|node| node.verdict));
                }
            }
        }
        Stop::Refused(reason) => {
            not_run(&mut outcome, RunVerdict::Refused.as_str());
            outcome = outcome.stopped(RunVerdict::Refused, reason);
        }
        Stop::Interrupted => {
            not_run(&mut outcome, RunVerdict::Interrupted.as_str());
            outcome = outcome.stopped(
                RunVerdict::Interrupted,
                "a signal to spec stopped the run; the runner's processes were stopped".to_owned(),
            );
        }
    }

    // 11: the event, then stdout.
    let event = Event::of(&outcome, &prepared.repo.worktree, &prepared.by);
    if let Err(error) = prepared
        .context
        .queue
        .record_verify_tests(&event, &request.now)
    {
        outcome.messages.push(Message::Warning(format!(
            "verify.tests not recorded: {error}"
        )));
    }
    Ok(outcome)
}

/// Steps 2 to 6. `Ok(Err)`: the run ended there (`cannot`, `dirty`,
/// `empty`); `Err`: exit 2 with stdout empty (discovery, the task).
fn prepare(
    env: &Env,
    globals: &Globals,
    request: &VerifyTestsRequest,
    outcome: &mut VerifyTestsOutcome,
) -> Result<Result<Prepared, Box<VerifyTestsOutcome>>, CliError> {
    let end = |outcome: &mut VerifyTestsOutcome, cause: String| {
        Ok(Err(Box::new(
            std::mem::replace(outcome, VerifyTestsOutcome::new(None)).cannot(cause),
        )))
    };

    // 2: discovery, the config.
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let label = project.config_label.clone();
    let config = match verify_from_toml(&text) {
        Ok(config) => config,
        Err(error) => return end(outcome, error.at(&label)),
    };
    outcome.timeout = Some(config.timeout);
    let Some(template) = config.tests else {
        return end(outcome, format!("[verify] tests is not set in {label}"));
    };
    outcome.command = Some(template.written().to_vec());
    let code = match CodeConfig::from_toml(&text) {
        Ok(Some(code)) => code,
        Ok(None) => {
            return end(
                outcome,
                format!(
                    "{label} has no [code] table: the code layer is off (add [code] with roots)"
                ),
            );
        }
        Err(error) => return end(outcome, error.at(&label)),
    };

    // 3: git.
    let repo = match open_repo(&project, &request.git) {
        Ok(repo) => repo,
        Err(cause) => return end(outcome, cause),
    };
    outcome.branch = Some(repo.branch.clone());
    outcome.commit = Some(repo.head.clone());

    // 4: the whole tree clean.
    let status = match repo.git.status_all() {
        Ok(status) => status,
        Err(error) => return end(outcome, format!("git: {error}")),
    };
    if !status.is_empty() {
        let reason = format!(
            "the worktree differs from HEAD: {}; commit them first",
            status_list(&status)
        );
        let done = std::mem::replace(outcome, VerifyTestsOutcome::new(None));
        return Ok(Err(Box::new(done.stopped(RunVerdict::Dirty, reason))));
    }
    // The template's file: an ignored one is clean and still not committed.
    match repo.git.blob_oid(&repo.head, &repo.top(CONFIG_FILE)) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return end(
                outcome,
                format!(
                    "{CONFIG_FILE} is not a file of HEAD: [verify] tests runs only a committed \
                     template"
                ),
            );
        }
        Err(error) => return end(outcome, format!("git: {error}")),
    }

    // 5: the committer, the queue, the task.
    let by = match repo.git.committer_ident() {
        Ok(by) => by,
        Err(error) => {
            return end(
                outcome,
                format!("no committer identity (git var GIT_COMMITTER_IDENT: {error})"),
            );
        }
    };
    let context = match open_context(env, globals, &request.git) {
        Ok(context) => context,
        Err(error) => return end(outcome, unprefixed(&error)),
    };
    let task = match &request.task {
        Some(written) => Some(named_task(&context, written)?),
        None => None,
    };
    if let Some(task) = &task {
        outcome.task = Some(task.id.clone());
    }

    // 6: the corpus, the walk, the set, the tests, the nodes.
    let mut messages = Vec::new();
    let input = match indexed(env, &project, &mut messages, false) {
        Ok(input) => input,
        Err(error) => {
            outcome.messages.append(&mut messages);
            return end(outcome, unprefixed(&error));
        }
    };
    let tree = match read_tree(&project.root, &code, &mut messages) {
        Ok(tree) => tree,
        Err(error) => {
            outcome.messages.append(&mut messages);
            return end(outcome, unprefixed(&error));
        }
    };
    let scheme = &project.config.scheme;
    let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
    let selected = Selection {
        tree: &tree,
        scheme,
        repo: &repo,
        template: &template,
        now: &request.now,
    }
    .select(&mut corpus, task.as_ref(), &mut messages);
    outcome.messages.append(&mut messages);
    let selected = match selected {
        Ok(selected) => selected,
        Err(cause) => return end(outcome, cause),
    };
    outcome.tests = selected.rows;
    outcome.probes_planned = selected.probes.len();
    outcome.nodes = selected
        .nodes
        .iter()
        .map(|(id, state, _)| VerifyTestsNode {
            id: id.clone(),
            verdict: if state.is_some() {
                NodeVerdict::NotVerifiable
            } else {
                NodeVerdict::NoRunnableTest
            },
            state: state.clone(),
        })
        .collect();
    if selected.runs.is_empty() {
        let reason = if outcome.tests.is_empty() {
            "no @verifies binding names a node of the set".to_owned()
        } else {
            format!(
                "none of the {} bound tests can run (each says why)",
                outcome.tests.len()
            )
        };
        let mut done = std::mem::replace(outcome, VerifyTestsOutcome::new(None));
        finish_static_nodes(&mut done, &selected.nodes);
        return Ok(Err(Box::new(done.stopped(RunVerdict::Empty, reason))));
    }
    Ok(Ok(Prepared {
        root: project.root.clone(),
        template,
        timeout: config.timeout,
        repo,
        by,
        context,
        runs: selected.runs,
        probes: selected.probes,
        nodes: selected.nodes,
    }))
}

/// A `CliError`'s message without its `spec: ` prefix: a cause line.
fn unprefixed(error: &CliError) -> String {
    error
        .message
        .strip_prefix("spec: ")
        .unwrap_or(&error.message)
        .to_owned()
}

/// Git in the worktree holding the project ("Order" step 3): `HEAD` born,
/// on a branch, no operation in progress; else the cause.
fn open_repo(project: &ProjectRoot, env: &GitEnv) -> Result<Repo, String> {
    let at_root =
        WorktreeGit::new(&project.root, env).map_err(|error| format!("cannot run git: {error}"))?;
    let place = at_root.place(&project.root).map_err(|error| match error {
        PlaceError::Git(error) => format!(
            "the project root lies in no git worktree ({error}): the tests run over a commit"
        ),
        other => format!("cannot run the tests: {other}"),
    })?;
    let git = WorktreeGit::new(&place.worktree, env)
        .map_err(|error| format!("cannot run git: {error}"))?;
    match git.operation_in_progress() {
        Ok(None) => {}
        Ok(Some(operation)) => {
            return Err(format!(
                "a {operation} is in progress: finish or abort it first"
            ));
        }
        Err(error) => return Err(format!("git: {error}")),
    }
    Ok(Repo {
        git,
        worktree: place.worktree,
        root_rel: place.root_rel,
        branch: place.branch,
        head: place.base_commit,
    })
}

/// `XY path` of each entry, `, `-joined.
fn status_list(status: &[StatusEntry]) -> String {
    status
        .iter()
        .map(|entry| format!("{}{} {}", entry.index, entry.worktree, entry.path))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------- select

/// What step 6 reads from.
struct Selection<'a> {
    tree: &'a CodeTree,
    scheme: &'a IdScheme,
    repo: &'a Repo,
    template: &'a TestTemplate,
    now: &'a str,
}

/// What step 6 found.
struct Selected {
    rows: Vec<VerifyTestsRow>,
    runs: Vec<(Vec<String>, Vec<usize>)>,
    probes: Vec<Vec<String>>,
    nodes: Vec<(String, Option<String>, Vec<usize>)>,
}

/// A runnable test's argv, and its probe's.
struct Planned {
    argv: Vec<String>,
    probe: Vec<String>,
}

/// A `@verifies` binding before the set: where and what it binds, the IDs
/// its markers write.
struct Bound {
    /// The Rust file's index in the walk; `None` for a RON file.
    rust: Option<usize>,
    target: Target,
    markers: Vec<String>,
}

impl Selection<'_> {
    /// The set, the tests, their runnability, the reported nodes' state.
    fn select(
        &self,
        corpus: &mut Corpus<'_>,
        task: Option<&Task>,
        messages: &mut Vec<Message>,
    ) -> Result<Selected, String> {
        let tree = self.tree;
        // Every binding of a `@verifies` marker, by (file, symbol), a keyless
        // one also by its comment's offset (its own row); every marker file
        // of every node (any relation), as accept finds them.
        let mut bound: BTreeMap<(String, String, Option<usize>), Bound> = BTreeMap::new();
        let mut marker_files: BTreeMap<NodeKey, BTreeSet<String>> = BTreeMap::new();
        for (index, file) in tree.rust.iter().enumerate() {
            let names = tree.names.items.get(index).map_or(&[][..], Vec::as_slice);
            let module = tree.names.modules.get(index);
            for comment in &file.binding.comments {
                let Some(text) = file.text.get(comment.range.clone()) else {
                    continue;
                };
                for marker in markers_in(text) {
                    for key in corpus.judge(&marker.id).nodes {
                        marker_files
                            .entry(key)
                            .or_default()
                            .insert(file.path.clone());
                    }
                    if marker.relation != Relation::Verifies {
                        continue;
                    }
                    let target =
                        Target::of_rust(comment.binding, &file.binding.analysis, names, module);
                    let (_, symbol) = kind_and_symbol(&target);
                    let keyless = keyless_at(&target, comment.range.start);
                    bound
                        .entry((file.path.clone(), symbol, keyless))
                        .or_insert_with(|| Bound {
                            rust: Some(index),
                            target,
                            markers: Vec::new(),
                        })
                        .markers
                        .push(marker.id.clone());
                }
            }
        }
        for file in &tree.ron {
            for anchor in &file.anchors {
                let Some(text) = file.text.get(anchor.comment.clone()) else {
                    continue;
                };
                for marker in markers_in(text) {
                    for key in corpus.judge(&marker.id).nodes {
                        marker_files
                            .entry(key)
                            .or_default()
                            .insert(file.path.clone());
                    }
                    if marker.relation != Relation::Verifies {
                        continue;
                    }
                    let target = Target::of_ron(&anchor.anchor);
                    let (_, symbol) = kind_and_symbol(&target);
                    let keyless = keyless_at(&target, anchor.comment.start);
                    bound
                        .entry((file.path.clone(), symbol, keyless))
                        .or_insert_with(|| Bound {
                            rust: None,
                            target,
                            markers: Vec::new(),
                        })
                        .markers
                        .push(marker.id.clone());
                }
            }
        }

        // The set: the task's, else every node a `@verifies` binding names.
        let set = match task {
            Some(task) => task_set(task, corpus, self.scheme, messages),
            None => {
                let mut keys = BTreeSet::new();
                for binding in bound.values() {
                    for written in &binding.markers {
                        keys.extend(corpus.judge(written).nodes);
                    }
                }
                NodeSet::of(corpus, self.scheme, keys)
            }
        };

        // The tests in the set, each with its set nodes.
        let mut tests: Vec<(String, String, Bound, BTreeSet<NodeKey>)> = Vec::new();
        for ((file, symbol, _), binding) in bound {
            let markers: Vec<&str> = binding.markers.iter().map(String::as_str).collect();
            let nodes = set.row_nodes(corpus, &markers, None);
            if !nodes.is_empty() {
                tests.push((file, symbol, binding, nodes));
            }
        }
        let reported: BTreeSet<NodeKey> = tests
            .iter()
            .flat_map(|(_, _, _, nodes)| nodes.iter().copied())
            .collect();
        if let Some(task) = task {
            let without = set.keys().filter(|key| !reported.contains(key)).count();
            if without > 0 {
                messages.push(Message::Note(format!(
                    "verify --tests: {without} nodes of {} have no @verifies binding",
                    task.id
                )));
            }
        }

        // Each reported node's state ("Verifiable").
        let mut tracked = Tracked::new(self.repo);
        let empty_lock = Lock::empty(build_recipes());
        let mut states: BTreeMap<NodeKey, Option<String>> = BTreeMap::new();
        for &key in &reported {
            let files = marker_files.get(&key).cloned().unwrap_or_default();
            let state = self.refusal(corpus, &set, key, &files, &empty_lock, &mut tracked)?;
            states.insert(key, state);
        }

        // Each test's runnability and argv.
        let packages = tree_packages(tree);
        let mut placed: BTreeMap<usize, PlacedNames> = BTreeMap::new();
        let mut rows = Vec::with_capacity(tests.len());
        let mut argvs: Vec<Option<Planned>> = Vec::with_capacity(tests.len());
        for (file, symbol, binding, nodes) in &tests {
            let (kind, _) = kind_and_symbol(&binding.target);
            let mut ids: Vec<String> = nodes
                .iter()
                .map(|key| set.canonical(*key).unwrap_or_default().to_owned())
                .collect();
            ids.sort();
            let any_verifiable = nodes
                .iter()
                .any(|key| states.get(key).is_some_and(Option::is_none));
            let runnable = self.runnable(
                file,
                binding,
                &packages,
                &mut placed,
                &mut tracked,
                any_verifiable,
            )?;
            let (argv, reason) = match runnable {
                Ok(pair) => (Some(pair), None),
                Err(reason) => (None, Some(reason)),
            };
            rows.push(VerifyTestsRow {
                file: file.clone(),
                kind,
                symbol: symbol.clone(),
                nodes: ids,
                argv: argv.as_ref().map(|planned| planned.argv.clone()),
                outcome: TestOutcome::NotRun,
                exit: None,
                signal: None,
                reason,
                output_hash: None,
                tail: None,
            });
            argvs.push(argv);
        }

        // Each distinct argv once, by its first test; its probe; each
        // distinct program of either, in that order.
        let mut runs: Vec<(Vec<String>, Vec<usize>)> = Vec::new();
        let mut run_of: BTreeMap<&[String], usize> = BTreeMap::new();
        let mut probes: Vec<Vec<String>> = Vec::new();
        let mut probe_seen: BTreeSet<&[String]> = BTreeSet::new();
        let mut programs: Vec<&str> = Vec::new();
        let mut program_seen: BTreeSet<&str> = BTreeSet::new();
        for (index, argv) in argvs.iter().enumerate() {
            let Some(Planned { argv, probe }) = argv else {
                continue;
            };
            if let Some(&run) = run_of.get(argv.as_slice()) {
                runs[run].1.push(index);
                continue;
            }
            run_of.insert(argv, runs.len());
            runs.push((argv.clone(), vec![index]));
            if probe_seen.insert(probe) {
                probes.push(probe.clone());
            }
            for program in [program_word(argv), program_word(probe)] {
                if program_seen.insert(program) {
                    programs.push(program);
                }
            }
        }

        // Each program run from the root's tree: a file of `HEAD`.
        for program in programs {
            if joined_to_root(program) && !tracked.program_at_head(program)? {
                return Err(format!(
                    "the [verify] tests program `{program}` is not a file of HEAD: commit it first"
                ));
            }
        }

        // The reported nodes by ID, each with its runnable tests.
        let mut nodes: Vec<(String, Option<String>, Vec<usize>)> = reported
            .iter()
            .map(|key| {
                let id = set.canonical(*key).unwrap_or_default().to_owned();
                let state = states.get(key).cloned().flatten();
                let runnable = tests
                    .iter()
                    .enumerate()
                    .filter(|(index, (_, _, _, nodes))| {
                        nodes.contains(key) && argvs[*index].is_some()
                    })
                    .map(|(index, _)| index)
                    .collect();
                (id, state, runnable)
            })
            .collect();
        nodes.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(Selected {
            rows,
            runs,
            probes,
            nodes,
        })
    }

    /// The node's first refusal by accept's own predicate ("Verifiable");
    /// `None`: verifiable.
    fn refusal(
        &self,
        corpus: &mut Corpus<'_>,
        set: &NodeSet,
        key: NodeKey,
        marker_files: &BTreeSet<String>,
        empty_lock: &Lock,
        tracked: &mut Tracked<'_>,
    ) -> Result<Option<String>, String> {
        let path = corpus.resolver.paths()[key.file].to_owned();
        let Some(id) = corpus
            .files
            .get(key.file)
            .and_then(|file| file.parsed.as_ref())
            .and_then(|parsed| parsed.nodes.get(key.ord))
            .and_then(|node| node.id.clone())
        else {
            return Ok(Some(REV_UNKNOWN.to_owned()));
        };
        // Accept's "tracked and clean": a file `HEAD` lacks.
        if !tracked.at_head(&path)? {
            return Ok(Some(NOT_COMMITTED.to_owned()));
        }
        for file in marker_files {
            if !tracked.at_head(file)? {
                return Ok(Some(NOT_COMMITTED.to_owned()));
            }
        }
        let canonical = set
            .canonical(key)
            .map(str::to_owned)
            .or_else(|| canonical_of(corpus, self.scheme, key))
            .unwrap_or_default();
        let node = NodeAt {
            key,
            path,
            id,
            canonical,
        };
        let (rev, spec_hash) =
            match node_at_head(self.repo, self.scheme, &node).map_err(|e| unprefixed(&e))? {
                Ok(found) => found,
                Err(_) => {
                    let state = if corpus.revision(key).number().is_none() {
                        REV_UNKNOWN
                    } else {
                        HASH_NULL
                    };
                    return Ok(Some(state.to_owned()));
                }
            };
        let today = self.now.get(..10).unwrap_or(self.now);
        let facts = Facts {
            rev,
            spec_hash: &spec_hash,
            commit: &self.repo.head,
            today,
        };
        let bindings = committed_bindings(
            self.repo,
            self.tree,
            marker_files,
            corpus,
            &node,
            empty_lock,
            &facts,
        )
        .map_err(|error| unprefixed(&error))?;
        Ok(bindings
            .unlockable
            .into_values()
            .next()
            .or_else(|| bindings.keyless.into_iter().next().map(|(_, state)| state)))
    }

    /// The test's argv and its probe's, or why it is not run ("Runnable",
    /// the first reason that applies).
    fn runnable(
        &self,
        file: &str,
        binding: &Bound,
        packages: &[FilePackage],
        placed: &mut BTreeMap<usize, PlacedNames>,
        tracked: &mut Tracked<'_>,
        any_verifiable: bool,
    ) -> Result<Result<Planned, String>, String> {
        let not = |reason: &str| Ok(Err(reason.to_owned()));
        if !tracked.at_head(file)? {
            return not(NOT_COMMITTED);
        }
        if matches!(binding.target, Target::Module { .. }) {
            return not(NOT_RUN_MODULE);
        }
        let Some(index) = binding.rust else {
            return not(NOT_RUN_RON);
        };
        let tree = self.tree;
        let rust = &tree.rust[index];
        let package = &packages[index];
        let names = placed.entry(index).or_insert_with(|| {
            let index_of = package.table.as_ref().map(TargetIndex::new);
            placed_name_file(
                &NamedFile {
                    path: Path::new(&rust.path),
                    package: package.dir.as_deref(),
                    analysis: &rust.binding.analysis,
                },
                index_of.as_ref(),
            )
        });
        let unit = match &names.role {
            FileRole::Unrooted => return not(UNROOTED),
            role => role.unit(),
        };
        if unit.is_some_and(|unit| unit.kind == UnitKind::Shared) {
            return not(SHARED_UNIT);
        }
        let Target::Item { index: item, .. } = binding.target else {
            return not(NOT_A_FUNCTION);
        };
        let Some(record) = rust.binding.analysis.items.get(item) else {
            return not(NOT_A_FUNCTION);
        };
        if record.owner.is_some() {
            return not(MEMBER);
        }
        if record.kind != FUNCTION_ITEM {
            return not(NOT_A_FUNCTION);
        }
        let Some(path) = names.paths.get(item) else {
            return not(NOT_A_FUNCTION);
        };
        let (target_kind, target) = match unit {
            Some(unit) => (Some(unit.kind_str().to_owned()), Some(unit.name.clone())),
            None => match package.table.as_ref().and_then(primary_target) {
                Some(primary) => (Some(primary.kind.clone()), Some(primary.name.clone())),
                None => (None, None),
            },
        };
        let qpath = tree
            .names
            .items
            .get(index)
            .and_then(|names| names.get(item))
            .map_or_else(
                || path.to_string(),
                |name| name.undiscriminated().to_owned(),
            );
        let mut values = PlaceholderValues {
            file: Some(file.to_owned()),
            qpath: Some(qpath),
            package: package.name.clone(),
            target_kind,
            target,
            test: Some(test_name(path)),
        };
        let argv = match self.template.expand(&values) {
            Ok(argv) => argv,
            Err(why) => return Ok(Err(why.to_string())),
        };
        if !any_verifiable {
            return not(NODE_NOT_VERIFIABLE);
        }
        let probe_path = QPath {
            name: PROBE_TEST.to_owned(),
            ..path.clone()
        };
        values.qpath = Some(probe_path.to_string());
        values.test = Some(PROBE_TEST.to_owned());
        let probe = match self.template.expand(&values) {
            Ok(probe) => probe,
            Err(why) => return Ok(Err(why.to_string())),
        };
        Ok(Ok(Planned { argv, probe }))
    }
}

/// `item`, `module`, `ron` or a keyless binding's state, and the name it
/// binds (a keyless binding's category).
fn kind_and_symbol(target: &Target) -> (String, String) {
    match target {
        Target::Item { qpath, .. } => (ITEM.to_owned(), qpath.clone()),
        Target::Module { qpath, .. } => (MODULE.to_owned(), qpath.clone()),
        Target::Ron { path, .. } => (RON.to_owned(), path.clone()),
        Target::Keyless { state, category } => ((*state).to_owned(), (*category).to_owned()),
    }
}

/// `{test}`: the module segments and the name joined by `::` (libtest's
/// name), from the structured path, never the rendered one.
fn test_name(path: &QPath) -> String {
    let mut segments: Vec<&str> = path.module.iter().map(String::as_str).collect();
    segments.push(&path.name);
    segments.join("::")
}

/// The primary target of a package's table, as `specengine-code` names
/// files of it: the library, else the bin rooted at `src/main.rs`, else
/// the sole bin.
fn primary_target(table: &PackageTargets) -> Option<&CargoTarget> {
    const NOT_LIBRARY: [&str; 5] = ["bin", "example", "test", "bench", "custom-build"];
    if let Some(library) = table
        .targets
        .iter()
        .find(|target| !NOT_LIBRARY.contains(&target.kind.as_str()))
    {
        return Some(library);
    }
    let mut bins = table.targets.iter().filter(|target| target.kind == "bin");
    if let Some(default) = bins
        .clone()
        .find(|target| parts_of(&target.root).as_deref() == Some(&["src", "main.rs"][..]))
    {
        return Some(default);
    }
    match (bins.next(), bins.next()) {
        (Some(sole), None) => Some(sole),
        _ => None,
    }
}

/// The normal components of a relative path, `.` dropped; `None` when one
/// is not UTF-8 or not a plain name (`..`, a root), as `specengine-code`
/// compares a target's root.
fn parts_of(path: &Path) -> Option<Vec<&str>> {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect()
}

/// A keyless binding's own row key, its comment's offset; `None` for a
/// keyed one (one row per (`file`, `symbol`)).
fn keyless_at(target: &Target, offset: usize) -> Option<usize> {
    matches!(target, Target::Keyless { .. }).then_some(offset)
}

/// Whether root-relative paths are files of `HEAD`, each asked once.
struct Tracked<'a> {
    repo: &'a Repo,
    known: BTreeMap<String, bool>,
}

impl<'a> Tracked<'a> {
    fn new(repo: &'a Repo) -> Self {
        Self {
            repo,
            known: BTreeMap::new(),
        }
    }

    fn at_head(&mut self, path: &str) -> Result<bool, String> {
        let top = self.repo.top(path);
        self.at_top(&top)
    }

    /// A program joined to the root ([`joined_to_root`]), `.` dropped: a
    /// file of `HEAD`. A `..` climbs only the root's own segments, which
    /// are physical (the root is canonical); a `..` after a written name,
    /// which the kernel resolves through what is on disk, or above the
    /// top: none.
    fn program_at_head(&mut self, written: &str) -> Result<bool, String> {
        let mut parts: Vec<&str> = self
            .repo
            .root_rel
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        let mut named = false;
        for part in written.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if named || parts.pop().is_none() {
                        return Ok(false);
                    }
                }
                part => {
                    named = true;
                    parts.push(part);
                }
            }
        }
        if parts.is_empty() {
            return Ok(false);
        }
        self.at_top(&parts.join("/"))
    }

    /// A path from the worktree's top.
    fn at_top(&mut self, top: &str) -> Result<bool, String> {
        if let Some(&known) = self.known.get(top) {
            return Ok(known);
        }
        let at_head = self
            .repo
            .git
            .blob_oid(&self.repo.head, top)
            .map_err(|error| format!("git: {error}"))?
            .is_some();
        self.known.insert(top.to_owned(), at_head);
        Ok(at_head)
    }
}

// ------------------------------------------------------------------- run

/// How steps 8 and 9 ended.
enum Stop {
    Done,
    Refused(String),
    Interrupted,
}

/// The probes, then each distinct argv once; the rows take their runs'
/// outcomes.
#[cfg(unix)]
fn run_all(prepared: &Prepared, outcome: &mut VerifyTestsOutcome) -> Stop {
    use crate::test_runner::{Ended, Forwarding, Invocation, run};

    let vars: Vec<(OsString, OsString)> = prepared
        .repo
        .git
        .vars()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    let root = prepared.root.as_path();
    let timeout = Duration::from_secs(u64::from(prepared.timeout));
    let forwarding = Forwarding::hold();
    // Every argv holds element 0: a template has one, expansion keeps it.
    let invoke = |argv: &[String]| {
        let (written, args) = argv
            .split_first()
            .map_or(("", &[][..]), |(first, rest)| (first.as_str(), rest));
        let program = program_of(root, written);
        let invocation = Invocation {
            program: &program,
            args,
            dir: root,
            vars: &vars,
            timeout,
        };
        run(&invocation, &forwarding)
    };

    let mut stop = Stop::Done;
    for argv in &prepared.probes {
        let ran = invoke(argv);
        let (exit, signal, result) = match &ran.ended {
            Ended::Exited(0) => (Some(0), None, TestOutcome::Passed),
            Ended::Exited(code) => (Some(*code), None, TestOutcome::Failed),
            Ended::Signalled(signal) => (None, Some(*signal), TestOutcome::Failed),
            Ended::TimedOut => (None, None, TestOutcome::TimedOut),
            Ended::Interrupted | Ended::CannotStart(_) => (None, None, TestOutcome::NotRun),
        };
        if !matches!(ran.ended, Ended::CannotStart(_)) {
            outcome.probes.push(VerifyTestsProbe {
                argv: argv.clone(),
                exit,
                signal,
                outcome: result,
                output_hash: ran.output_hash.clone(),
            });
        }
        stop = match ran.ended {
            Ended::Exited(0) => Stop::Refused(PROBE_PASSED.to_owned()),
            Ended::TimedOut => Stop::Refused(format!(
                "the [verify] tests command timed out after {} s on a test that does not exist \
                 (the probe); make it end when no test matches",
                prepared.timeout
            )),
            Ended::CannotStart(error) => Stop::Refused(cannot_start(program_word(argv), &error)),
            Ended::Interrupted => Stop::Interrupted,
            Ended::Exited(_) | Ended::Signalled(_) if forwarding.interrupted() => Stop::Interrupted,
            Ended::Exited(_) | Ended::Signalled(_) => Stop::Done,
        };
        if !matches!(stop, Stop::Done) {
            return stop;
        }
    }

    for (argv, members) in &prepared.runs {
        let ran = invoke(argv);
        let (exit, signal, result) = match &ran.ended {
            Ended::Exited(0) => (Some(0), None, TestOutcome::Passed),
            Ended::Exited(code) => (Some(*code), None, TestOutcome::Failed),
            Ended::Signalled(signal) => (None, Some(*signal), TestOutcome::Failed),
            Ended::TimedOut => (None, None, TestOutcome::TimedOut),
            Ended::Interrupted => return Stop::Interrupted,
            Ended::CannotStart(error) => {
                return Stop::Refused(cannot_start(program_word(argv), error));
            }
        };
        let tail = matches!(result, TestOutcome::Failed | TestOutcome::TimedOut)
            .then(|| String::from_utf8_lossy(&ran.tail).into_owned());
        for &member in members {
            let row = &mut outcome.tests[member];
            row.outcome = result;
            row.exit = exit;
            row.signal = signal;
            row.output_hash.clone_from(&ran.output_hash);
            row.tail.clone_from(&tail);
        }
        if forwarding.interrupted() {
            stop = Stop::Interrupted;
            break;
        }
    }
    stop
}

/// Element 0 as the program: a relative path holding a `/` joined to the
/// root (std leaves a relative program under the child's directory
/// platform-specific); else as written (absolute, or looked up on `PATH`).
fn program_of(root: &Path, written: &str) -> OsString {
    if joined_to_root(written) {
        root.join(written).into_os_string()
    } else {
        OsStr::new(written).to_owned()
    }
}

/// Element 0 is run from the root's tree: relative, holding a `/`.
fn joined_to_root(written: &str) -> bool {
    written.contains('/') && Path::new(written).is_relative()
}

/// Element 0 as written.
fn program_word(argv: &[String]) -> &str {
    argv.first().map_or("", String::as_str)
}

/// A program that cannot start, named as written, with the error.
fn cannot_start(program: &str, error: &std::io::Error) -> String {
    format!("the [verify] tests program `{program}` cannot start: {error}")
}

/// Every runnable test not run yet: `not run`, `reason` the run's verdict.
fn not_run(outcome: &mut VerifyTestsOutcome, reason: &str) {
    for row in &mut outcome.tests {
        if row.argv.is_some() && row.outcome == TestOutcome::NotRun {
            row.reason = Some(reason.to_owned());
        }
    }
}

/// The node verdicts over the rows' outcomes.
fn finish_nodes(outcome: &mut VerifyTestsOutcome, prepared: &Prepared) {
    finish_static_nodes(outcome, &prepared.nodes);
}

/// [`finish_nodes`] over the reported nodes and their runnable tests.
fn finish_static_nodes(
    outcome: &mut VerifyTestsOutcome,
    nodes: &[(String, Option<String>, Vec<usize>)],
) {
    outcome.nodes = nodes
        .iter()
        .map(|(id, state, runnable)| {
            let outcomes: Vec<TestOutcome> = runnable
                .iter()
                .map(|&index| outcome.tests[index].outcome)
                .collect();
            VerifyTestsNode {
                id: id.clone(),
                verdict: node_verdict(state.is_none(), &outcomes),
                state: state.clone(),
            }
        })
        .collect();
}

/// Step 10: `HEAD`, the branch and the status read again; the reason of a
/// difference.
fn changed(repo: &Repo) -> Option<String> {
    let mut reasons = Vec::new();
    match repo.git.head() {
        Ok(Some(head)) if head == repo.head => {}
        Ok(Some(head)) => reasons.push(format!(
            "HEAD moved from {} to {head} during the run",
            repo.head
        )),
        Ok(None) => reasons.push("HEAD names no commit now".to_owned()),
        Err(error) => reasons.push(format!("HEAD cannot be read again (git: {error})")),
    }
    match repo.git.branch() {
        Ok(Some(branch)) if branch == repo.branch => {}
        Ok(Some(branch)) => reasons.push(format!(
            "the branch changed from {} to {branch} during the run",
            repo.branch
        )),
        Ok(None) => reasons.push("HEAD is detached now".to_owned()),
        Err(error) => reasons.push(format!("the branch cannot be read again (git: {error})")),
    }
    match repo.git.status_all() {
        Ok(status) if status.is_empty() => {}
        Ok(status) => reasons.push(format!(
            "the worktree changed during the run: {}; commit them first",
            status_list(&status)
        )),
        Err(error) => reasons.push(format!("the worktree cannot be read again (git: {error})")),
    }
    (!reasons.is_empty()).then(|| reasons.join("; "))
}

// ---------------------------------------------------------------- output

/// The template as the question shows it: each element a JSON string.
fn shown_template(written: &[String]) -> String {
    let elements: Vec<String> = written
        .iter()
        .map(|element| serde_json::to_string(element).unwrap_or_else(|_| element.clone()))
        .collect();
    format!("[{}]", elements.join(", "))
}

/// A test's listing line (step 7): `run | <file> | <kind> <symbol> |
/// <nodes>` or `not run | … | <reason>`.
fn listing_line(row: &VerifyTestsRow) -> String {
    let head = if row.argv.is_some() { "run" } else { "not run" };
    let mut line = format!(
        "{head} | {} | {} {} | {}",
        one_line(&row.file),
        row.kind,
        one_line(&row.symbol),
        row.nodes.join(", ")
    );
    if row.argv.is_none()
        && let Some(reason) = &row.reason
    {
        line.push_str(" | ");
        line.push_str(reason);
    }
    escape_controls(&line)
}

/// A test's stdout line: `<outcome> | <file> | <kind> <symbol> |
/// <nodes>[ | <reason>]`.
fn test_line(row: &VerifyTestsRow, timeout: Option<u32>) -> String {
    let reason = match row.outcome {
        TestOutcome::Passed => None,
        TestOutcome::Failed => match (row.exit, row.signal) {
            (Some(code), _) => Some(format!("exit {code}")),
            (None, Some(signal)) => Some(format!("signal {signal}")),
            (None, None) => None,
        },
        TestOutcome::TimedOut => timeout.map(|seconds| format!("after {seconds} s")),
        TestOutcome::NotRun => row.reason.clone(),
    };
    let mut line = format!(
        "{} | {} | {} {} | {}",
        row.outcome.words(),
        one_line(&row.file),
        row.kind,
        one_line(&row.symbol),
        row.nodes.join(", ")
    );
    if let Some(reason) = reason {
        line.push_str(" | ");
        line.push_str(&reason);
    }
    line
}

/// The counts of the summary and the JSON.
#[derive(Serialize)]
struct NodeCounts {
    verified: usize,
    failed: usize,
    not_verifiable: usize,
    no_runnable_test: usize,
}

#[derive(Serialize)]
struct TestCounts {
    passed: usize,
    failed: usize,
    timed_out: usize,
    not_run: usize,
}

#[derive(Serialize)]
struct CountsJson {
    nodes: NodeCounts,
    tests: TestCounts,
    probes: usize,
}

fn counts(outcome: &VerifyTestsOutcome) -> CountsJson {
    let nodes = |verdict: NodeVerdict| {
        outcome
            .nodes
            .iter()
            .filter(|node| node.verdict == verdict)
            .count()
    };
    let tests = |result: TestOutcome| {
        outcome
            .tests
            .iter()
            .filter(|row| row.outcome == result)
            .count()
    };
    CountsJson {
        nodes: NodeCounts {
            verified: nodes(NodeVerdict::Verified),
            failed: nodes(NodeVerdict::Failed),
            not_verifiable: nodes(NodeVerdict::NotVerifiable),
            no_runnable_test: nodes(NodeVerdict::NoRunnableTest),
        },
        tests: TestCounts {
            passed: tests(TestOutcome::Passed),
            failed: tests(TestOutcome::Failed),
            timed_out: tests(TestOutcome::TimedOut),
            not_run: tests(TestOutcome::NotRun),
        },
        probes: outcome.probes_planned,
    }
}

/// The text report: the test lines by (`file`, `symbol`), the node lines
/// by ID, the `cannot:` lines, the summary.
pub(crate) fn render_text(outcome: &VerifyTestsOutcome) -> String {
    let mut out = String::new();
    for row in &outcome.tests {
        out.push_str(&test_line(row, outcome.timeout));
        out.push('\n');
    }
    for node in &outcome.nodes {
        let mut line = format!("node {}: {}", node.id, node.verdict.words());
        if let Some(state) = &node.state {
            line.push(' ');
            line.push_str(state);
        }
        out.push_str(&one_line(&line));
        out.push('\n');
    }
    for cause in &outcome.cannot {
        out.push_str(&format!("cannot: {}\n", one_line(cause)));
    }
    let task = outcome
        .task
        .as_deref()
        .map(|task| format!(" --task {task}"))
        .unwrap_or_default();
    let tail = match &outcome.reason {
        Some(reason) => one_line(reason),
        None => {
            let counts = counts(outcome);
            format!(
                "{} nodes ({} verified, {} failed, {} not verifiable, {} no runnable test); {} \
                 tests ({} passed, {} failed, {} timed out, {} not run)",
                outcome.nodes.len(),
                counts.nodes.verified,
                counts.nodes.failed,
                counts.nodes.not_verifiable,
                counts.nodes.no_runnable_test,
                outcome.tests.len(),
                counts.tests.passed,
                counts.tests.failed,
                counts.tests.timed_out,
                counts.tests.not_run
            )
        }
    };
    out.push_str(&format!(
        "verify --tests{task}: {}: {tail}\n",
        outcome.verdict.as_str()
    ));
    escape_controls(&out)
}

fn node_verdict_name<S: serde::Serializer>(
    verdict: &NodeVerdict,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(verdict.as_str())
}

/// A test row of the document.
#[derive(Serialize)]
struct TestJson<'a> {
    file: &'a str,
    kind: &'a str,
    symbol: &'a str,
    nodes: &'a [String],
    argv: Option<&'a [String]>,
    outcome: &'static str,
    exit: Option<i32>,
    signal: Option<i32>,
    reason: Option<&'a str>,
}

#[derive(Serialize)]
struct VerifyTestsJson<'a> {
    task: Option<&'a str>,
    branch: Option<&'a str>,
    commit: Option<&'a str>,
    command: Option<&'a [String]>,
    timeout: Option<u32>,
    verdict: &'static str,
    reason: Option<&'a str>,
    counts: CountsJson,
    tests: Vec<TestJson<'a>>,
    nodes: &'a [VerifyTestsNode],
    lock: Option<()>,
    cannot: &'a [String],
    notes: Vec<String>,
}

/// One document, every key present (absent: `null`), no timing, output
/// hash, tail or worktree.
impl Serialize for VerifyTestsOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        VerifyTestsJson {
            task: self.task.as_deref(),
            branch: self.branch.as_deref(),
            commit: self.commit.as_deref(),
            command: self.command.as_deref(),
            timeout: self.timeout,
            verdict: self.verdict.as_str(),
            reason: self.reason.as_deref(),
            counts: counts(self),
            tests: self
                .tests
                .iter()
                .map(|row| TestJson {
                    file: &row.file,
                    kind: &row.kind,
                    symbol: &row.symbol,
                    nodes: &row.nodes,
                    argv: row.argv.as_deref(),
                    outcome: row.outcome.as_str(),
                    exit: row.exit,
                    signal: row.signal,
                    reason: row.reason.as_deref(),
                })
                .collect(),
            nodes: &self.nodes,
            lock: None,
            cannot: &self.cannot,
            notes: self.notes(),
        }
        .serialize(serializer)
    }
}

// ----------------------------------------------------------------- event

/// The `verify.tests` payload ("Event"), its keys in this order.
#[derive(Serialize)]
struct Event<'a> {
    task: Option<&'a str>,
    worktree: &'a str,
    branch: Option<&'a str>,
    commit: Option<&'a str>,
    command: Option<&'a [String]>,
    timeout: Option<u32>,
    verdict: &'static str,
    probes: Vec<ProbeEvent<'a>>,
    tests: Vec<TestEvent<'a>>,
    nodes: &'a [VerifyTestsNode],
    /// `null` until slice 8b writes the run's evidence.
    lock_commit: Option<()>,
    by: &'a str,
}

#[derive(Serialize)]
struct ProbeEvent<'a> {
    argv: &'a [String],
    exit: Option<i32>,
    signal: Option<i32>,
    outcome: &'static str,
    output_hash: Option<&'a str>,
}

#[derive(Serialize)]
struct TestEvent<'a> {
    file: &'a str,
    symbol: &'a str,
    nodes: &'a [String],
    argv: Option<&'a [String]>,
    exit: Option<i32>,
    signal: Option<i32>,
    outcome: &'static str,
    output_hash: Option<&'a str>,
    tail: Option<&'a str>,
}

impl<'a> Event<'a> {
    fn of(outcome: &'a VerifyTestsOutcome, worktree: &'a str, by: &'a str) -> Self {
        Self {
            task: outcome.task.as_deref(),
            worktree,
            branch: outcome.branch.as_deref(),
            commit: outcome.commit.as_deref(),
            command: outcome.command.as_deref(),
            timeout: outcome.timeout,
            verdict: outcome.verdict.as_str(),
            probes: outcome
                .probes
                .iter()
                .map(|probe| ProbeEvent {
                    argv: &probe.argv,
                    exit: probe.exit,
                    signal: probe.signal,
                    outcome: probe.outcome.as_str(),
                    output_hash: probe.output_hash.as_deref(),
                })
                .collect(),
            tests: outcome
                .tests
                .iter()
                .map(|row| TestEvent {
                    file: &row.file,
                    symbol: &row.symbol,
                    nodes: &row.nodes,
                    argv: row.argv.as_deref(),
                    exit: row.exit,
                    signal: row.signal,
                    outcome: row.outcome.as_str(),
                    output_hash: row.output_hash.as_deref(),
                    tail: row.tail.as_deref(),
                })
                .collect(),
            nodes: &outcome.nodes,
            lock_commit: None,
            by,
        }
    }
}

#[cfg(test)]
mod primary_target_tests {
    //! Task spec `verify-tests` (8a-1), review 1's minor on
    //! `primary_target` (`{target_kind}`, `{target}` of a primary-target
    //! file, canon "Placeholders"): the default bin is found by its root's
    //! components with `.` dropped, as `specengine-code` finds it, so a
    //! manifest's `path = "./src/main.rs"` beside another bin still names a
    //! primary target. A crafted table: the CLI's layout tables never carry
    //! a `./` root (`verify_tests_targets.rs`
    //! `primary_bin_resolves_beside_another_bin`). M: the root compared as a
    //! path (`root == "src/main.rs"`).

    use std::path::PathBuf;

    use specengine_code::{PackageTargets, Target as CargoTarget, TargetSource};

    use super::{parts_of, primary_target};

    fn target(kind: &str, name: &str, root: &str) -> CargoTarget {
        CargoTarget {
            kind: kind.to_owned(),
            name: name.to_owned(),
            root: PathBuf::from(root),
        }
    }

    fn table(targets: Vec<CargoTarget>) -> PackageTargets {
        PackageTargets {
            source: TargetSource::Metadata,
            targets,
        }
    }

    /// The name of the primary target, if any.
    fn primary(targets: Vec<CargoTarget>) -> Option<String> {
        primary_target(&table(targets)).map(|found| found.name.clone())
    }

    /// The review's case: `./src/main.rs` beside a second bin, in either
    /// order, is the default bin.
    #[test]
    fn dotted_default_bin_beside_another_bin_is_primary() {
        for root in ["./src/main.rs", "src/main.rs", "./src/./main.rs"] {
            let tool = target("bin", "kiln-tool", root);
            let extra = target("bin", "kiln-extra", "src/bin/extra.rs");
            assert_eq!(
                primary(vec![extra.clone(), tool.clone()]).as_deref(),
                Some("kiln-tool"),
                "{root}, listed second"
            );
            assert_eq!(
                primary(vec![tool, extra]).as_deref(),
                Some("kiln-tool"),
                "{root}, listed first"
            );
        }
    }

    /// A library target wins over any bin; non-library kinds never count
    /// as the library.
    #[test]
    fn library_wins_over_the_default_bin() {
        for kind in ["lib", "rlib", "proc-macro"] {
            assert_eq!(
                primary(vec![
                    target("bin", "kiln-tool", "./src/main.rs"),
                    target(kind, "kiln", "src/lib.rs"),
                ])
                .as_deref(),
                Some("kiln"),
                "{kind}"
            );
        }
        assert_eq!(
            primary(vec![
                target("test", "schedule", "tests/schedule.rs"),
                target("example", "demo", "examples/demo.rs"),
                target("bench", "speed", "benches/speed.rs"),
                target("custom-build", "build-script-build", "build.rs"),
                target("bin", "kiln-extra", "src/bin/extra.rs"),
            ])
            .as_deref(),
            Some("kiln-extra"),
            "no library: the sole bin"
        );
    }

    /// No default bin among several: none; a root that leaves the package
    /// (`..`) or is absolute is not `src/main.rs`; an example rooted at
    /// `src/main.rs` is no bin.
    #[test]
    fn no_primary_without_a_default_among_several_bins() {
        for root in [
            "../src/main.rs",
            "/src/main.rs",
            "src/main.rs/..",
            "main.rs",
        ] {
            assert_eq!(
                primary(vec![
                    target("bin", "kiln-tool", root),
                    target("bin", "kiln-extra", "src/bin/extra.rs"),
                ]),
                None,
                "{root}"
            );
        }
        assert_eq!(
            primary(vec![
                target("example", "demo", "src/main.rs"),
                target("bin", "kiln-tool", "src/bin/tool.rs"),
                target("bin", "kiln-extra", "src/bin/extra.rs"),
            ]),
            None,
            "an example is no bin"
        );
        assert_eq!(
            primary(vec![target("bin", "kiln-tool", "src/bin/tool.rs")]).as_deref(),
            Some("kiln-tool"),
            "a sole bin, wherever rooted"
        );
        assert_eq!(primary(Vec::new()), None, "no target");
    }

    /// `parts_of` drops `.` and refuses `..` and a root, as
    /// `specengine-code`'s does.
    #[test]
    fn parts_of_drops_dots_and_refuses_parents() {
        let parts = |path: &str| parts_of(std::path::Path::new(path)).map(|parts| parts.join("/"));
        assert_eq!(parts("./src/main.rs").as_deref(), Some("src/main.rs"));
        assert_eq!(parts("src/./main.rs").as_deref(), Some("src/main.rs"));
        assert_eq!(parts("src//main.rs").as_deref(), Some("src/main.rs"));
        assert_eq!(parts("../src/main.rs"), None);
        assert_eq!(parts("/src/main.rs"), None);
    }
}
