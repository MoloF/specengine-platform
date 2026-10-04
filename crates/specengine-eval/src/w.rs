//! Measurement `w` (`docs/canon/w-measurement.md`): task W of every task
//! document, before and after, over the same targets.
//!
//! One process, read-only on the corpus. The refusals (`--budget`, `--out`
//! against the corpus, `layout`'s, the before scheme's `[project] slug`, the
//! tasks config) run before anything is written. Then `<out>/w/<label>/` is
//! emptied, the after-tree written into its `tree/` by `layout`'s own step
//! ([`layout::write_tree`]), read back by core, and each task measured:
//! W_before from the corpus's bytes over the pilot's reading protocol,
//! W_after from `specengine_cli::bundle` on the tree, the follow-up reads
//! from `specengine_cli::show`; every bundle computed a second time, in
//! reverse task order, on a fresh data directory (AC-07).
//!
//! The CLI library runs with an [`Env`] of its own: the tree as its
//! current directory, `home-1/` or `home-2/` beside the tree as its home,
//! so every database lives under `--out` (AC-08), never the process's.
//!
//! stdout carries counts and statistics only (AC-06); paths, IDs, REFs and
//! messages go only to `tasks.json` and `bundles/` beside the tree.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specengine_cli::{
    Bundle, BundleOutcome, BundleRequest, CliError, Env, Exit, Globals, ItemForm, Outcome,
    ShowRequest, layer_key, render_text,
};
use specengine_core::check::{
    BundleLayer, CheckConfig, Edge, Endpoint, NodeAt, Resolver, SpecGraph, Standing,
};
use specengine_core::{DOCUMENT_EXTENSION, Paths, WalkScope, is_clean_relative};
use specengine_import::import::Role;
use specengine_import::layout::Emitted;
use specengine_model::{CanonTarget, FmValue, IdScope, LinkOrigin, LinkTarget, Node, PathTarget};
use specengine_store::{CONFIG_FILE, WorkingTree, check_input};
use toml::Spanned;

use crate::WArgs;
use crate::census::write_json;
use crate::harness::{self, Corpus};
use crate::layout;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "pilot-w/one";

/// The measurement's name and its directory under `--out`.
pub const MEASUREMENT: &str = "w";

/// The tasks config at the corpus root without `--tasks`.
const DEFAULT_TASKS: &str = "tasks.toml";
/// The budget without `--budget` (07 §5 `bundle_task`), estimated tokens.
const DEFAULT_BUDGET: u32 = 10_000;
/// The after-tree's directory beside the detail files.
const TREE: &str = "tree";
/// The data directories' homes of pass 1 and pass 2.
const HOMES: [&str; 2] = ["home-1", "home-2"];
/// Pass 1's bundle bodies, one file per bundled task.
const BUNDLES: &str = "bundles";
/// The per-task detail.
const TASKS_FILE: &str = "tasks.json";
/// A task needs a third step past this many documents, and is incomplete
/// past this many follow-ups (A1; 05 §6).
const THRESHOLD: usize = 3;
/// The two refusals of the bundle's frame, by the CLI's own wording
/// (`spec bundle`'s fitting step 1; AC-05): any other CLI error ends the
/// run.
const BELOW_MINIMUM: &str = "below this bundle's minimum";
const OVER_CEILING: &str = "-character ceiling";
/// The tail line of a `spec show` output its cap cut.
const TRUNCATED: &str = "[truncated: ";
/// A wiki link's brackets: core makes no link of them (AC-02).
const WIKI_OPEN: &str = "[[";
const WIKI_CLOSE: &str = "]]";
/// The words a statistic prints when it lands on a task without bytes.
const REFUSED: &str = "refused";
const FAILED: &str = "failed";
const BUNDLED: &str = "bundled";
/// The name stem of a task on stdout and in the detail.
const TASK_STEM: &str = "task";
/// Slot names of the detail.
const TIER0_SLOT: &str = "tier0";
const TIER1_SLOT: &str = "tier1";
const INDEX_SLOT: &str = "index";
const SHARD_SLOT: &str = "shard";
/// Where a slot's bytes were read.
const SIDE_BEFORE: &str = "before";
const SIDE_AFTER: &str = "after";
const READ_CORPUS: &str = "corpus";
const READ_TREE: &str = "tree";

// ---------------------------------------------------------------------------
// The tasks config.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTasksConfig {
    tasks: RawTasks,
    tier1: Option<RawTier1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTasks {
    include: Spanned<Vec<Spanned<String>>>,
    #[serde(default)]
    exclude: Vec<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTier1 {
    key: Option<Spanned<String>>,
    default: Option<Spanned<String>>,
    paths: Option<Spanned<BTreeMap<String, Spanned<String>>>>,
}

/// The tasks config, checked against the corpus.
pub struct TasksConfig {
    /// Census globs over source paths; at least one.
    include: Vec<String>,
    exclude: Vec<String>,
    /// The front-matter key naming a task's area, read in the after-tree.
    key: Option<String>,
    /// The Tier 1 README of a task no key value maps.
    default: Option<String>,
    /// Key value → the Tier 1 README of that area.
    paths: BTreeMap<String, String>,
}

/// Reads the tasks config at `path`; every error is `<path>:<line>:
/// message` (AC-01). `sources` are the walked source documents, `root` the
/// corpus.
fn load_tasks(path: &Path, sources: &[&str], root: &Path) -> Result<TasksConfig, String> {
    let name = path.display().to_string();
    let text = fs::read_to_string(path)
        .map_err(|error| format!("{name}:1: cannot read the tasks config: {error}"))?;
    let at = |span: Option<Range<usize>>, message: String| {
        let line = span.map_or(1, |span| line_of(&text, span.start));
        format!("{name}:{line}: {message}")
    };
    let raw: RawTasksConfig = toml::from_str(&text)
        .map_err(|error| at(error.span(), error.message().trim().to_owned()))?;
    if raw.tasks.include.get_ref().is_empty() {
        return Err(at(
            Some(raw.tasks.include.span()),
            "`[tasks] include` is empty: name at least one glob over the task documents' paths"
                .to_owned(),
        ));
    }
    for (table, glob) in raw
        .tasks
        .include
        .get_ref()
        .iter()
        .map(|glob| ("`[tasks] include`", glob))
        .chain(
            raw.tasks
                .exclude
                .iter()
                .map(|glob| ("`[tasks] exclude`", glob)),
        )
    {
        let scope = glob_scope(std::slice::from_ref(glob.get_ref()));
        if !sources.iter().any(|source| scope.is_excluded(source)) {
            return Err(at(
                Some(glob.span()),
                format!(
                    "{table} glob `{}` matches no walked source document",
                    glob.get_ref()
                ),
            ));
        }
    }
    let mut config = TasksConfig {
        include: raw
            .tasks
            .include
            .get_ref()
            .iter()
            .map(|glob| glob.get_ref().clone())
            .collect(),
        exclude: raw
            .tasks
            .exclude
            .iter()
            .map(|glob| glob.get_ref().clone())
            .collect(),
        key: None,
        default: None,
        paths: BTreeMap::new(),
    };
    let Some(tier1) = raw.tier1 else {
        return Ok(config);
    };
    let corpus_file = |key: &str, value: &Spanned<String>| -> Result<String, String> {
        let path = value.get_ref();
        if !is_clean_relative(path) {
            return Err(at(
                Some(value.span()),
                format!("`{key}` `{path}` is not a clean path relative to the corpus root"),
            ));
        }
        if !root.join(path).is_file() {
            return Err(at(
                Some(value.span()),
                format!("`{key}` `{path}` is not a file of the corpus"),
            ));
        }
        Ok(path.clone())
    };
    if let Some(paths) = &tier1.paths {
        if tier1.key.is_none() {
            return Err(at(
                Some(paths.span()),
                "`[tier1.paths]` needs `[tier1] key`: the front-matter key whose values it maps"
                    .to_owned(),
            ));
        }
        for (value, path) in paths.get_ref() {
            config
                .paths
                .insert(value.clone(), corpus_file("[tier1.paths]", path)?);
        }
    }
    if let Some(default) = &tier1.default {
        config.default = Some(corpus_file("[tier1] default", default)?);
    }
    config.key = tier1.key.map(Spanned::into_inner);
    Ok(config)
}

/// The census globs `globs` through core's matcher: a path matches one iff
/// [`WalkScope::is_excluded`] holds over a `Paths` carrying them.
fn glob_scope(globs: &[String]) -> WalkScope {
    Paths {
        exclude: globs.to_vec(),
        ..Paths::default()
    }
    .walk_scope()
}

/// The 1-based line of byte `offset` in `text`.
fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

// ---------------------------------------------------------------------------
// Setup.

/// What the run needs, read and emitted before anything is written.
pub struct Setup {
    layout: layout::Setup,
    tasks: TasksConfig,
    budget: u32,
    /// `flag` or `default`.
    budget_source: &'static str,
}

/// The refusals of `w`, in order: `--budget`, `--out` against the corpus
/// and the symlinks under it, `layout`'s (its configs, its start rules, its
/// emitter run in memory), the before scheme's `[project] slug`, the tasks
/// config. Any error is a refusal (exit 2, nothing written).
pub fn prepare(root: &Path, args: &WArgs, run_label: &str) -> Result<Setup, String> {
    let (budget, budget_source) = match args.budget {
        None => (DEFAULT_BUDGET, "default"),
        Some(budget) => match u32::try_from(budget) {
            Ok(tokens) if tokens >= 1 => (tokens, "flag"),
            _ => {
                return Err(format!(
                    "--budget {budget}: the budget is a whole number of estimated tokens from 1 to {}; nothing written",
                    u32::MAX
                ));
            }
        },
    };
    let common = &args.layout.common;
    layout::check_out(root, &common.out, MEASUREMENT, run_label)?;
    let label = common.label.as_deref();
    let setup = layout::prepare(
        root,
        common.config.as_deref(),
        args.layout.scheme.as_deref(),
        args.layout.today.as_deref(),
        label,
    )?;
    if let Err(error) = setup.before.project.slug() {
        let scheme = harness::resolve_file(
            "--scheme",
            args.layout.scheme.as_deref(),
            harness::PILOT_SCHEME,
            label,
            root,
            CONFIG_FILE,
        )?;
        return Err(format!(
            "{}; nothing written",
            error.at(&scheme.display().to_string())
        ));
    }
    let tasks_path = harness::resolve_file(
        "--tasks",
        args.tasks.as_deref(),
        harness::PILOT_TASKS,
        label,
        root,
        DEFAULT_TASKS,
    )?;
    let sources: Vec<&str> = setup
        .layout
        .emission
        .documents
        .iter()
        .map(|document| document.source.as_str())
        .collect();
    let tasks = load_tasks(&tasks_path, &sources, root)?;
    Ok(Setup {
        layout: setup,
        tasks,
        budget,
        budget_source,
    })
}

// ---------------------------------------------------------------------------
// The result (AC-06: exactly this whitelist).

#[derive(Debug, Serialize)]
pub struct WResult {
    pub tasks: usize,
    pub budget: BudgetOut,
    pub w_before: Statistic,
    pub w_after: Statistic,
    pub w_after_followups: Statistic,
    pub docs_needed: Statistic,
    pub third_step: usize,
    pub worst: Worst,
    pub slots: SlotCounts,
    pub targets: TargetCounts,
    pub citations: CitationCounts,
    pub followups: FollowupCounts,
    pub refused: usize,
    pub failed: usize,
    pub nondeterministic: usize,
    pub fill_percent: Fill,
    pub bytes_per_token: f64,
    pub bundle_ms: Millis,
}

#[derive(Debug, Serialize)]
pub struct BudgetOut {
    pub tokens: u32,
    pub source: &'static str,
}

/// Nearest-rank statistics: a number of bytes, or the word of the task it
/// lands on.
#[derive(Debug, Serialize)]
pub struct Statistic {
    pub median: Value,
    pub p90: Value,
    pub max: Value,
}

#[derive(Debug, Serialize)]
pub struct Worst {
    pub w_before: Option<String>,
    pub w_after: Option<String>,
    pub w_after_followups: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct SlotCounts {
    pub tier0: u64,
    pub index: u64,
    pub shard: usize,
    pub tier1: Tier1Counts,
    pub unmapped: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct Tier1Counts {
    pub key: usize,
    pub default: usize,
    pub none: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct TargetCounts {
    pub refs: usize,
    pub text: usize,
    pub outline: usize,
    pub header: usize,
    pub not_included: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct CitationCounts {
    pub unresolved: usize,
    pub unchecked: usize,
    pub wiki_links: usize,
}

#[derive(Debug, Default, Serialize)]
pub struct FollowupCounts {
    pub refs: usize,
    pub bytes: usize,
    pub truncated: usize,
    pub incomplete: usize,
}

#[derive(Debug, Serialize)]
pub struct Fill {
    pub median: f64,
    pub max: f64,
}

#[derive(Debug, Serialize)]
pub struct Millis {
    pub median: u128,
    pub p90: u128,
}

// ---------------------------------------------------------------------------
// The detail (`tasks.json`).

#[derive(Debug, Serialize)]
struct TaskDetail {
    task: String,
    source: String,
    after: String,
    refs: Vec<String>,
    targets: Vec<TargetDetail>,
    citations: CitationsDetail,
    slots: Vec<SlotDetail>,
    w_before: u64,
    w_after: Value,
    w_after_followups: Value,
    docs_needed: usize,
    outcome: &'static str,
    bundle: Option<BundleDetail>,
    message: Option<String>,
    followups: FollowupsDetail,
    bundle_hash_2: Option<String>,
    bundle_ms: Option<u128>,
    bundle_2_ms: Option<u128>,
    show_ms: u128,
}

#[derive(Debug, Serialize)]
struct TargetDetail {
    reference: String,
    path: String,
    line: usize,
    sources: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct CitationsDetail {
    unresolved: Vec<Written>,
    unchecked: Vec<Written>,
    /// Each `[[…]]` pair.
    wiki_links: Vec<WikiLine>,
}

#[derive(Debug, Serialize)]
struct Written {
    written: String,
    line: usize,
    /// The record file it is written in, when not D's after file.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

/// A `[[…]]` pair: its line in D's after file, or its record file and
/// line.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum WikiLine {
    Line(usize),
    At { path: String, line: usize },
}

#[derive(Debug, Serialize)]
struct SlotDetail {
    slot: &'static str,
    side: &'static str,
    path: String,
    bytes: u64,
    /// Where the bytes were read: the corpus or the tree.
    read: &'static str,
}

#[derive(Debug, Serialize)]
struct BundleDetail {
    budget: u32,
    tokens: u32,
    chars: usize,
    bytes: usize,
    not_included: usize,
    more: usize,
    bundle_hash: String,
    /// Items placed per layer, by the layer's JSON key.
    layers: BTreeMap<&'static str, usize>,
    /// The targets layer: each item's form.
    forms: Vec<FormDetail>,
}

#[derive(Debug, Serialize)]
struct FormDetail {
    name: String,
    path: String,
    line: usize,
    form: ItemForm,
}

#[derive(Debug, Default, Serialize)]
struct FollowupsDetail {
    refs: Vec<FollowupDetail>,
    bytes: usize,
}

#[derive(Debug, Serialize)]
struct FollowupDetail {
    reference: String,
    bytes: usize,
    truncated: bool,
    /// `spec show` did not answer it (the task: `failed`); bytes 0.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    not_answered: bool,
}

// ---------------------------------------------------------------------------
// The run.

/// One task before its bundle: its document, targets, REFs and slots.
struct Task {
    name: String,
    source: String,
    after: String,
    /// The document node; `None`: failed before any call.
    document: Option<NodeAt>,
    refs: Vec<String>,
    targets: Vec<TargetDetail>,
    citations: CitationsDetail,
    slots: Vec<SlotDetail>,
    tier1: Tier1Choice,
    shard: bool,
    unmapped: usize,
    w_before: u64,
    docs_needed: usize,
    /// Tier 0 and Tier 1 on the after side.
    after_slots: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier1Choice {
    Key,
    Default,
    None,
}

/// What one bundle call answered.
#[derive(Clone, PartialEq, Eq)]
enum Called {
    Bundled(Box<Bundle>),
    /// Exit 2 naming the frame's minimum or the ceiling.
    Refused(String),
    /// Exit 1: a REF names nothing.
    Failed(String),
}

impl Called {
    /// The outcome compared across the two passes (AC-07).
    fn key(&self) -> (&'static str, &str) {
        match self {
            Self::Bundled(bundle) => (BUNDLED, bundle.bundle_hash.as_str()),
            Self::Refused(message) => (REFUSED, message.as_str()),
            Self::Failed(message) => (FAILED, message.as_str()),
        }
    }
}

/// A figure of one task: bytes, or why there are none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Figure {
    Bytes(u64),
    Refused,
    Failed,
}

impl Figure {
    fn value(self) -> Value {
        match self {
            Self::Bytes(bytes) => Value::from(bytes),
            Self::Refused => Value::from(REFUSED),
            Self::Failed => Value::from(FAILED),
        }
    }
}

/// One `spec show` follow-up read.
#[derive(Clone)]
enum Shown {
    Read { bytes: usize, truncated: bool },
    NotAnswered(String),
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<WResult, String> {
    let Setup {
        layout: layout_setup,
        tasks: config,
        budget,
        budget_source,
    } = setup;
    let out_dir = corpus.out.join(MEASUREMENT).join(&corpus.label);
    layout::clear(&out_dir)
        .map_err(|error| format!("cannot clear {}: {error}", out_dir.display()))?;
    let tree = out_dir.join(TREE);
    let homes = HOMES.map(|home| out_dir.join(home));
    let bundles_dir = out_dir.join(BUNDLES);
    for dir in [&tree, &homes[0], &homes[1], &bundles_dir] {
        fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    }
    let written = layout::write_tree(&tree, &layout_setup)?;
    let emitted = &written.emitted;

    // The tree as core reads it.
    let tree_source =
        WorkingTree::new(&tree, &emitted.project.paths).map_err(|error| error.to_string())?;
    let input = check_input(&tree_source, &emitted.project.scheme);
    let graph = SpecGraph::new(&input, &emitted.project.scheme, &emitted.project.paths);
    let resolver = Resolver::new(&input, &emitted.project.scheme, &emitted.project.paths);

    let mut bytes = Bytes::new(&corpus.root, &tree);
    let links = CorpusLinks::new(&layout_setup);
    let protocol = Protocol::new(&layout_setup, &corpus.root, &links, &mut bytes);
    let tasks = plan_tasks(
        &layout_setup,
        &config,
        &protocol,
        &links,
        &graph,
        &resolver,
        &mut bytes,
    );

    // Pass 1, in task order: the bundle, then its follow-ups.
    let globals = Globals {
        root: Some(tree.clone()),
        config: None,
    };
    let env_of = |home: &Path| Env {
        cwd: tree.clone(),
        home: Some(OsString::from(home.as_os_str())),
        xdg_data_home: None,
    };
    let first = env_of(&homes[0]);
    let mut pass1: Vec<Option<(Called, u128)>> = Vec::with_capacity(tasks.len());
    for task in &tasks {
        pass1.push(match task.document {
            Some(_) => Some(call_bundle(&first, &globals, task, budget)?),
            None => None,
        });
    }
    let mut shown: BTreeMap<String, Shown> = BTreeMap::new();
    let mut followups: Vec<(Vec<String>, u128)> = Vec::with_capacity(tasks.len());
    for (task, called) in tasks.iter().zip(&pass1) {
        let refs = match called {
            Some((Called::Bundled(bundle), _)) => followup_refs(bundle, &graph, &resolver),
            Some((Called::Refused(_), _)) => task.refs.clone(),
            Some((Called::Failed(_), _)) | None => Vec::new(),
        };
        let started = Instant::now();
        for reference in &refs {
            if !shown.contains_key(reference) {
                let read = show(&first, &globals, reference);
                shown.insert(reference.clone(), read);
            }
        }
        followups.push((refs, started.elapsed().as_millis()));
    }

    // Pass 2, in reverse task order, on a fresh data directory.
    let second = env_of(&homes[1]);
    let mut pass2: Vec<Option<(Called, u128)>> = vec![None; tasks.len()];
    for (index, task) in tasks.iter().enumerate().rev() {
        if task.document.is_some() {
            pass2[index] = Some(call_bundle(&second, &globals, task, budget)?);
        }
    }

    // Per task: the figures and the detail.
    let mut details = Vec::with_capacity(tasks.len());
    let mut figures = Figures::default();
    let mut result_counts = Counts::default();
    for (index, task) in tasks.into_iter().enumerate() {
        let (followup_refs, show_ms) = &followups[index];
        let mut followup = FollowupsDetail::default();
        let mut not_answered = None;
        for reference in followup_refs {
            match shown.get(reference) {
                Some(Shown::Read { bytes, truncated }) => {
                    followup.bytes += bytes;
                    followup.refs.push(FollowupDetail {
                        reference: reference.clone(),
                        bytes: *bytes,
                        truncated: *truncated,
                        not_answered: false,
                    });
                }
                Some(Shown::NotAnswered(message)) => {
                    not_answered.get_or_insert_with(|| {
                        format!("`spec show {reference}` did not answer: {message}")
                    });
                    followup.refs.push(FollowupDetail {
                        reference: reference.clone(),
                        bytes: 0,
                        truncated: false,
                        not_answered: true,
                    });
                }
                None => {}
            }
        }
        let called = pass1[index].as_ref();
        let again = pass2[index].as_ref();
        if let (Some((one, _)), Some((two, _))) = (called, again)
            && one.key() != two.key()
        {
            result_counts.nondeterministic += 1;
        }
        let (outcome, message, w_after, w_after_followups) = match (called, &not_answered) {
            (None, _) => (
                FAILED,
                Some("no document node in the after-tree".to_owned()),
                Figure::Failed,
                Figure::Failed,
            ),
            (Some((Called::Failed(reason), _)), _) => {
                (FAILED, Some(reason.clone()), Figure::Failed, Figure::Failed)
            }
            (Some(_), Some(message)) => (
                FAILED,
                Some(message.clone()),
                Figure::Failed,
                Figure::Failed,
            ),
            (Some((Called::Refused(message), _)), None) => (
                REFUSED,
                Some(message.clone()),
                Figure::Refused,
                Figure::Bytes(task.after_slots + followup.bytes as u64),
            ),
            (Some((Called::Bundled(bundle), _)), None) => {
                let after = task.after_slots + bundle.bytes as u64;
                (
                    BUNDLED,
                    None,
                    Figure::Bytes(after),
                    Figure::Bytes(after + followup.bytes as u64),
                )
            }
        };
        match outcome {
            REFUSED => result_counts.refused += 1,
            FAILED => result_counts.failed += 1,
            _ => {}
        }
        let bundle_detail = match called {
            Some((Called::Bundled(bundle), _)) => {
                let body = bundles_dir.join(format!("{}.txt", task.name));
                fs::write(&body, &bundle.body)
                    .map_err(|error| format!("cannot write {}: {error}", body.display()))?;
                // Every bundled task, a follow-up not answering too: its
                // forms, fill and bytes per token.
                tally_forms(bundle, &mut result_counts.targets);
                figures.bundled.push((bundle.tokens, bundle.bytes, budget));
                Some(bundle_detail(bundle))
            }
            _ => None,
        };
        if let Some((_, ms)) = called {
            figures.bundle_ms.push(*ms);
        }

        // Counters over every task.
        result_counts.targets.refs += task.refs.len();
        result_counts.citations.unresolved += task.citations.unresolved.len();
        result_counts.citations.unchecked += task.citations.unchecked.len();
        result_counts.citations.wiki_links += task.citations.wiki_links.len();
        result_counts.followups.refs += followup.refs.len();
        result_counts.followups.bytes += followup.bytes;
        result_counts.followups.truncated += followup.refs.iter().filter(|f| f.truncated).count();
        if followup_refs.len() > THRESHOLD {
            result_counts.followups.incomplete += 1;
        }
        if task.docs_needed > THRESHOLD {
            result_counts.third_step += 1;
        }
        if task.shard {
            result_counts.slots.shard += 1;
        }
        match task.tier1 {
            Tier1Choice::Key => result_counts.slots.tier1.key += 1,
            Tier1Choice::Default => result_counts.slots.tier1.default += 1,
            Tier1Choice::None => result_counts.slots.tier1.none += 1,
        }
        result_counts.slots.unmapped += task.unmapped;

        figures.w_before.push(Figure::Bytes(task.w_before));
        figures.w_after.push(w_after);
        figures.w_after_followups.push(w_after_followups);
        figures
            .docs_needed
            .push(Figure::Bytes(task.docs_needed as u64));

        details.push(TaskDetail {
            task: task.name,
            source: task.source,
            after: task.after,
            refs: task.refs,
            targets: task.targets,
            citations: task.citations,
            slots: task.slots,
            w_before: task.w_before,
            w_after: w_after.value(),
            w_after_followups: w_after_followups.value(),
            docs_needed: task.docs_needed,
            outcome,
            bundle: bundle_detail,
            message,
            followups: followup,
            bundle_hash_2: match again {
                Some((Called::Bundled(bundle), _)) => Some(bundle.bundle_hash.clone()),
                _ => None,
            },
            bundle_ms: called.map(|(_, ms)| *ms),
            bundle_2_ms: again.map(|(_, ms)| *ms),
            show_ms: *show_ms,
        });
    }
    write_json(&out_dir.join(TASKS_FILE), &details)?;

    result_counts.slots.tier0 = protocol.tier0.as_ref().map_or(0, |(_, bytes)| *bytes);
    result_counts.slots.index = protocol.index.as_ref().map_or(0, |(_, bytes)| *bytes);
    let names: Vec<&str> = details.iter().map(|detail| detail.task.as_str()).collect();
    let (w_before, worst_before) = statistic(&figures.w_before, &names);
    let (w_after, worst_after) = statistic(&figures.w_after, &names);
    let (w_after_followups, worst_followups) = statistic(&figures.w_after_followups, &names);
    let (docs_needed, _) = statistic(&figures.docs_needed, &names);
    let result = WResult {
        tasks: details.len(),
        budget: BudgetOut {
            tokens: budget,
            source: budget_source,
        },
        w_before,
        w_after,
        w_after_followups,
        docs_needed,
        third_step: result_counts.third_step,
        worst: Worst {
            w_before: worst_before,
            w_after: worst_after,
            w_after_followups: worst_followups,
        },
        slots: result_counts.slots,
        targets: result_counts.targets,
        citations: result_counts.citations,
        followups: result_counts.followups,
        refused: result_counts.refused,
        failed: result_counts.failed,
        nondeterministic: result_counts.nondeterministic,
        fill_percent: fill(&figures.bundled),
        bytes_per_token: bytes_per_token(&figures.bundled),
        bundle_ms: millis(&figures.bundle_ms),
    };
    summarize(&result, &corpus.label, &out_dir, &written);
    Ok(result)
}

/// The counters of the run, filled per task.
#[derive(Default)]
struct Counts {
    third_step: usize,
    slots: SlotCounts,
    targets: TargetCounts,
    citations: CitationCounts,
    followups: FollowupCounts,
    refused: usize,
    failed: usize,
    nondeterministic: usize,
}

/// The figures of every task, in task order.
#[derive(Default)]
struct Figures {
    w_before: Vec<Figure>,
    w_after: Vec<Figure>,
    w_after_followups: Vec<Figure>,
    docs_needed: Vec<Figure>,
    /// Each bundled task's (tokens, bytes, budget).
    bundled: Vec<(u32, usize, u32)>,
    /// Pass 1's bundle calls.
    bundle_ms: Vec<u128>,
}

/// One `spec bundle` call over the task's REFs at `budget`, timed: its
/// outcome, or the error that ends the run (a CLI error other than the
/// frame's two refusals).
fn call_bundle(
    env: &Env,
    globals: &Globals,
    task: &Task,
    budget: u32,
) -> Result<(Called, u128), String> {
    let request = BundleRequest {
        references: task.refs.clone(),
        budget: Some(i64::from(budget)),
    };
    let started = Instant::now();
    let answer = specengine_cli::bundle(env, globals, &request);
    let ms = started.elapsed().as_millis();
    let called = match answer {
        Ok(BundleOutcome {
            bundle: Some(bundle),
            ..
        }) => Called::Bundled(Box::new(bundle)),
        Ok(BundleOutcome { reason, .. }) => Called::Failed(reason.unwrap_or_default()),
        Err(error) if is_frame_refusal(&error) => Called::Refused(error.message),
        Err(error) => {
            return Err(format!(
                "{}: `spec bundle` could not run: {}",
                task.name, error.message
            ));
        }
    };
    Ok((called, ms))
}

/// Exit 2 naming the frame's minimum or the ceiling (AC-05).
fn is_frame_refusal(error: &CliError) -> bool {
    error.exit == Exit::CannotRun
        && (error.message.contains(BELOW_MINIMUM) || error.message.contains(OVER_CEILING))
}

/// One `spec show REF` as `spec show` prints it: its bytes and whether its
/// cap cut it.
fn show(env: &Env, globals: &Globals, reference: &str) -> Shown {
    let request = ShowRequest {
        reference: reference.to_owned(),
        links: false,
        archive: false,
    };
    match specengine_cli::show(env, globals, &request) {
        Ok(outcome) => match &outcome.reason {
            Some(reason) => Shown::NotAnswered(reason.clone()),
            None => {
                let text = render_text(&Outcome::Show(outcome));
                Shown::Read {
                    bytes: text.len(),
                    truncated: text.lines().any(|line| line.starts_with(TRUNCATED)),
                }
            }
        },
        Err(error) => Shown::NotAnswered(error.message),
    }
}

/// The follow-ups of a bundled task: its targets layer's outlined and
/// header-only items, as REFs, once each in print order.
fn followup_refs(bundle: &Bundle, graph: &SpecGraph<'_>, resolver: &Resolver<'_>) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    for (layer, items) in &bundle.layers {
        if *layer != BundleLayer::Targets {
            continue;
        }
        for item in items {
            if !matches!(item.form, ItemForm::Outline | ItemForm::Header) {
                continue;
            }
            let reference = node_of(graph, &item.path, item.line, &item.name)
                .map_or_else(|| item.name.clone(), |at| ref_name(graph, resolver, at));
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
    }
    refs
}

/// The node at `path` starting on `line` named `name`.
fn node_of(graph: &SpecGraph<'_>, path: &str, line: usize, name: &str) -> Option<NodeAt> {
    let file = graph.file_of(path)?;
    (0..graph.nodes(file).len())
        .map(|ord| NodeAt { file, ord })
        .find(|&at| graph.line(at) == line && graph.name(at) == name)
}

/// The targets layer's items by form; merged targets are inside theirs.
fn tally_forms(bundle: &Bundle, counts: &mut TargetCounts) {
    for (layer, items) in &bundle.layers {
        if *layer != BundleLayer::Targets {
            continue;
        }
        for item in items {
            match item.form {
                ItemForm::Text => counts.text += 1,
                ItemForm::Outline => counts.outline += 1,
                ItemForm::Header | ItemForm::Summary => counts.header += 1,
            }
        }
    }
    counts.not_included += bundle.not_included();
}

fn bundle_detail(bundle: &Bundle) -> BundleDetail {
    BundleDetail {
        budget: bundle.budget,
        tokens: bundle.tokens,
        chars: bundle.chars,
        bytes: bundle.bytes,
        not_included: bundle.not_included(),
        more: bundle.more,
        bundle_hash: bundle.bundle_hash.clone(),
        layers: bundle
            .layers
            .iter()
            .map(|(layer, items)| (layer_key(*layer), items.len()))
            .collect(),
        forms: bundle
            .layers
            .iter()
            .filter(|(layer, _)| *layer == BundleLayer::Targets)
            .flat_map(|(_, items)| items)
            .map(|item| FormDetail {
                name: item.name.clone(),
                path: item.path.clone(),
                line: item.line,
                form: item.form,
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// The tasks, their targets and the reading protocol.

/// Corpus and tree bytes, each path read once.
struct Bytes {
    root: PathBuf,
    tree: PathBuf,
    corpus: BTreeMap<String, u64>,
    after: BTreeMap<String, Option<u64>>,
}

impl Bytes {
    fn new(root: &Path, tree: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            tree: tree.to_path_buf(),
            corpus: BTreeMap::new(),
            after: BTreeMap::new(),
        }
    }

    /// The corpus file's bytes; 0 when it is missing.
    fn corpus(&mut self, path: &str) -> u64 {
        if let Some(&bytes) = self.corpus.get(path) {
            return bytes;
        }
        let bytes = file_bytes(&self.root, path).unwrap_or(0);
        self.corpus.insert(path.to_owned(), bytes);
        bytes
    }

    /// The after side of a slot (A4): the tree's file when the tree holds
    /// the path, else the corpus's.
    fn after(&mut self, path: &str) -> (u64, &'static str) {
        let tree = match self.after.get(path) {
            Some(&bytes) => bytes,
            None => {
                let bytes = file_bytes(&self.tree, path);
                self.after.insert(path.to_owned(), bytes);
                bytes
            }
        };
        match tree {
            Some(bytes) => (bytes, READ_TREE),
            None => (self.corpus(path), READ_CORPUS),
        }
    }
}

/// The size of the regular file `path` under `root`; `None` when it is
/// missing or not clean and relative.
fn file_bytes(root: &Path, path: &str) -> Option<u64> {
    if !is_clean_relative(path) {
        return None;
    }
    fs::metadata(root.join(path))
        .ok()
        .filter(fs::Metadata::is_file)
        .map(|meta| meta.len())
}

/// The pilot's reading protocol on the untouched corpus: its Tier 0, its
/// index root, its index shards and their links, the Tier 1 rule.
struct Protocol {
    /// `[paths] tier0` of the before scheme and its corpus bytes.
    tier0: Option<(String, u64)>,
    /// `[paths] index` of the before scheme and its corpus bytes.
    index: Option<(String, u64)>,
    /// The shards, by path: the source documents their file links resolve
    /// to.
    shards: Vec<(String, BTreeSet<String>)>,
}

impl Protocol {
    fn new(setup: &layout::Setup, root: &Path, links: &CorpusLinks<'_>, bytes: &mut Bytes) -> Self {
        let paths = &setup.before.project.paths;
        let tier0 = paths
            .tier0
            .as_ref()
            .map(|path| (path.clone(), bytes.corpus(path)));
        let index = paths
            .index
            .as_ref()
            .map(|path| (path.clone(), bytes.corpus(path)));
        let mut shards: Vec<(String, BTreeSet<String>)> =
            shard_paths(&setup.before.config, paths.index.as_deref(), root)
                .into_iter()
                .map(|shard| {
                    let linked = linked(root, &shard, &setup.before.project.scheme, links);
                    (shard, linked)
                })
                .collect();
        shards.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            tier0,
            index,
            shards,
        }
    }

    /// The first shard, in path order, linking to `source`.
    fn shard_of(&self, source: &str) -> Option<&str> {
        self.shards
            .iter()
            .find(|(_, linked)| linked.contains(source))
            .map(|(path, _)| path.as_str())
    }
}

/// The shards of the before scheme: the `index = true` generator's; without
/// one, the other outputs of the generator whose `writes` holds `[paths]
/// index` (config order: the first), those a corpus file stands at (a
/// registered generator core never runs); else none.
fn shard_paths(config: &CheckConfig, index: Option<&str>, root: &Path) -> Vec<String> {
    if let Some(generator) = config.index_generator() {
        return generator
            .shards
            .iter()
            .map(|shard| shard.path.clone())
            .collect();
    }
    let Some(index) = index else {
        return Vec::new();
    };
    config
        .generators
        .iter()
        .flatten()
        .find(|generator| generator.writes.iter().any(|path| path == index))
        .map(|generator| {
            generator
                .writes
                .iter()
                // docs/features/pilot-w.md AC-03: a shard is a Markdown
                // document; any other output is not read as one.
                .filter(|path| {
                    *path != index
                        && path.ends_with(DOCUMENT_EXTENSION)
                        && file_bytes(root, path).is_some()
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Core's resolution of a Markdown file link on the untouched corpus
/// (`resolve_link_path` and `file_link` of core's link check): the walked
/// source documents, the before scheme's `[paths] link_base` and its walk
/// scope.
struct CorpusLinks<'a> {
    walked: BTreeSet<&'a str>,
    link_base: Option<&'a str>,
    scope: WalkScope,
}

/// How a Markdown file link fares on the untouched corpus, by core's link
/// rules.
enum CorpusEnd {
    /// The walked source document it names.
    Walked(String),
    /// It names none, and a tried candidate lies in the before walk scope
    /// (core's `link-dangling`).
    Dangling,
    /// Core never checks it: not a document path, or no tried candidate
    /// lies in the before walk scope.
    Unchecked,
}

impl<'a> CorpusLinks<'a> {
    fn new(setup: &'a layout::Setup) -> Self {
        let paths = &setup.before.project.paths;
        Self {
            walked: setup
                .layout
                .emission
                .documents
                .iter()
                .map(|document| document.source.as_str())
                .collect(),
            link_base: paths.link_base.as_deref(),
            scope: paths.walk_scope(),
        }
    }

    /// The walked source document a file link's destination path names,
    /// written in `from` ([`CorpusLinks::classify`]).
    fn resolve(&self, from: &str, written: &str) -> Option<String> {
        match self.classify(from, written) {
            CorpusEnd::Walked(named) => Some(named),
            CorpusEnd::Dangling | CorpusEnd::Unchecked => None,
        }
    }

    /// A file link's destination path written in `from`: percent-decoded,
    /// checked only when it ends in the document extension; a `/`-led path
    /// is one candidate from the root, else `from`'s directory and, when
    /// that names no walked document, `link_base`. The first candidate
    /// naming a walked document resolves; with none, it dangles when a
    /// candidate (one leaving the root dropped) lies in the before walk
    /// scope, else it is unchecked (docs/features/pilot-w.md AC-02).
    fn classify(&self, from: &str, written: &str) -> CorpusEnd {
        let path = percent_decode(written);
        if !path.ends_with(DOCUMENT_EXTENSION) {
            return CorpusEnd::Unchecked;
        }
        let mut tried: Vec<Option<String>> = Vec::with_capacity(2);
        if let Some(from_root) = path.strip_prefix('/') {
            tried.push(normalise("", from_root));
        } else {
            let directory = from.rsplit_once('/').map_or("", |(directory, _)| directory);
            tried.push(normalise(directory, &path));
            if let Some(base) = self.link_base {
                tried.push(normalise(base, &path));
            }
        }
        let mut in_scope = false;
        for candidate in tried.into_iter().flatten() {
            if self.walked.contains(candidate.as_str()) {
                return CorpusEnd::Walked(candidate);
            }
            in_scope |= self.scope.in_walk_scope(&candidate);
        }
        if in_scope {
            CorpusEnd::Dangling
        } else {
            CorpusEnd::Unchecked
        }
    }
}

/// The source documents the Markdown file links of the corpus file `path`
/// resolve to ([`CorpusLinks::resolve`]); none when it is missing.
fn linked(
    root: &Path,
    path: &str,
    scheme: &specengine_model::IdScheme,
    links: &CorpusLinks<'_>,
) -> BTreeSet<String> {
    let Some(bytes) = is_clean_relative(path)
        .then(|| fs::read(root.join(path)).ok())
        .flatten()
    else {
        return BTreeSet::new();
    };
    let parsed = specengine_core::parse(path, &bytes, scheme);
    parsed
        .links
        .iter()
        .filter(|link| link.origin == LinkOrigin::Inline)
        .filter_map(|link| match &link.dst {
            LinkTarget::Path(target) => links.resolve(path, &target.path),
            _ => None,
        })
        .collect()
}

/// `path` relative to `directory`, `.` and empty components dropped, `..`
/// popped; `None` when it climbs above the root.
fn normalise(directory: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// `%XX` (two hex digits) → that byte, as core's link check: a malformed
/// `%` stays; a result that is not UTF-8 gives the text as written.
fn percent_decode(text: &str) -> String {
    if !text.contains('%') {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(at + 1).and_then(|&byte| hex_value(byte)),
                bytes.get(at + 2).and_then(|&byte| hex_value(byte)),
            )
        {
            decoded.push((high << 4) | low);
            at += 3;
            continue;
        }
        decoded.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| text.to_owned())
}

/// The value of one hex digit; `None` for any other byte (a sign too).
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// The tasks in source-path order: every walked (emitted) document whose
/// source path an `include` glob matches and no `exclude` glob does, but a
/// tree file of class `generated` (AC-01: Tier 3, shipped and documents
/// whose header core rejects are tasks; `spec bundle` admits a named Tier 3
/// target), each with its targets, REFs, citations and before slots.
///
/// Targets N: D, each record layout moved out of D into another after file
/// (its node), and the written end of every link written in D's after file
/// or those record files; a link the tree marks dangling or unchecked that
/// resolves on the corpus from D's source is mapped through the emission
/// map, else counted as core would on the corpus (dangling → unresolved,
/// never checked → unchecked); an anchor the tree misses that names a
/// definition layout moved out of the landed document → its record.
fn plan_tasks(
    setup: &layout::Setup,
    config: &TasksConfig,
    protocol: &Protocol,
    links: &CorpusLinks<'_>,
    graph: &SpecGraph<'_>,
    resolver: &Resolver<'_>,
    bytes: &mut Bytes,
) -> Vec<Task> {
    let include = glob_scope(&config.include);
    let exclude = glob_scope(&config.exclude);
    let emission = &setup.layout.emission;
    // Sources of a node (A5): every definition with its after path and ID,
    // else the document with that after path.
    let mut by_definition: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    for emitted in &emission.definitions {
        if let Some(after) = &emitted.after {
            by_definition
                .entry((after.as_str(), emitted.id.as_str()))
                .or_default()
                .insert(emitted.path.as_str());
        }
    }
    let mut by_document: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for document in &emission.documents {
        by_document
            .entry(document.after.as_str())
            .or_default()
            .insert(document.source.as_str());
    }
    let sources_of = |at: NodeAt| -> Vec<String> {
        let path = graph.paths()[at.file];
        let defined = graph
            .node(at)
            .and_then(|node| node.id.as_deref())
            .and_then(|id| by_definition.get(&(path, id)))
            .filter(|sources| !sources.is_empty());
        defined
            .or_else(|| by_document.get(path))
            .map(|sources| sources.iter().map(|source| (*source).to_owned()).collect())
            .unwrap_or_default()
    };

    // Per source document: its after path, and the definitions layout
    // moved out of it into another after file (N: their records).
    let after_of: BTreeMap<&str, &str> = emission
        .documents
        .iter()
        .map(|document| (document.source.as_str(), document.after.as_str()))
        .collect();
    let mut moved: BTreeMap<&str, Vec<&Emitted>> = BTreeMap::new();
    for emitted in &emission.definitions {
        if let Some(after) = emitted.after.as_deref()
            && after_of
                .get(emitted.path.as_str())
                .is_some_and(|own| *own != after)
        {
            moved
                .entry(emitted.path.as_str())
                .or_default()
                .push(emitted);
        }
    }
    // A definition's legacy aliases, by (source path, line, ID): the
    // import's definitions, which `emission.definitions` lists in order.
    let mut aliases: BTreeMap<(&str, usize, &str), BTreeSet<&str>> = BTreeMap::new();
    for record in setup
        .import
        .records
        .iter()
        .filter(|record| record.role == Role::Definition)
    {
        aliases
            .entry((record.path.as_str(), record.line, record.id.as_str()))
            .or_default()
            .extend(record.aliases.iter().map(String::as_str));
    }
    // docs/features/pilot-w.md AC-02: a link whose anchor names nothing in
    // the tree (core lands on the document `landed`) and is the ID or an
    // alias of a definition layout moved out of that document reads, on
    // the corpus, that definition: its record's node.
    let moved_anchor = |edge: &Edge, landed: NodeAt| -> Option<NodeAt> {
        if edge.note.is_none() || graph.document(landed.file) != Some(landed) {
            return None;
        }
        let anchor = percent_decode(path_target(graph, edge)?.anchor.as_deref()?);
        by_document
            .get(graph.paths()[landed.file])?
            .iter()
            .flat_map(|source| moved.get(source).into_iter().flatten())
            .filter(|emitted| {
                emitted.id == anchor
                    || aliases
                        .get(&(emitted.path.as_str(), emitted.line, emitted.id.as_str()))
                        .is_some_and(|names| names.contains(anchor.as_str()))
            })
            .find_map(|emitted| record_node(graph, emitted))
    };

    let mut documents: Vec<_> = emission
        .documents
        .iter()
        .filter(|document| {
            include.is_excluded(&document.source) && !exclude.is_excluded(&document.source)
        })
        .collect();
    documents.sort_by(|a, b| a.source.cmp(&b.source));
    let mut tasks = Vec::new();
    for document in documents {
        let file = graph.file_of(&document.after);
        if file.is_some_and(|file| graph.standing(file) == Standing::Generated) {
            continue;
        }
        let name = format!("{TASK_STEM}-{}", tasks.len() + 1);
        let at = file.and_then(|file| graph.document(file));
        let mut targets: BTreeSet<NodeAt> = BTreeSet::new();
        let mut citations = CitationsDetail::default();
        if let Some(at) = at {
            targets.insert(at);
            // Read as written in D: D's after file and the record file of
            // each definition moved out of D.
            let mut files = BTreeSet::from([at.file]);
            for emitted in moved.get(document.source.as_str()).into_iter().flatten() {
                if let Some(record) = record_node(graph, emitted) {
                    targets.insert(record);
                    files.insert(record.file);
                }
            }
            for edge in graph
                .edges()
                .iter()
                .filter(|edge| files.contains(&edge.file))
            {
                let written = || Written {
                    written: edge.written.clone(),
                    line: edge.line,
                    path: (edge.file != at.file).then(|| graph.paths()[edge.file].to_owned()),
                };
                match edge.written_end() {
                    Endpoint::Nodes(nodes) => match nodes.as_slice() {
                        [landed] => {
                            targets.insert(moved_anchor(edge, *landed).unwrap_or(*landed));
                        }
                        nodes => targets.extend(nodes.iter().copied()),
                    },
                    Endpoint::Dangling(_) => {
                        match relocated(graph, links, &after_of, edge, &document.source) {
                            Relocated::Node(node) => {
                                targets.insert(node);
                            }
                            Relocated::NoNode | Relocated::Unchecked => {
                                citations.unchecked.push(written());
                            }
                            Relocated::Dangling => citations.unresolved.push(written()),
                        }
                    }
                    Endpoint::Unchecked => {
                        match relocated(graph, links, &after_of, edge, &document.source) {
                            Relocated::Node(node) => {
                                targets.insert(node);
                            }
                            Relocated::NoNode | Relocated::Unchecked | Relocated::Dangling => {
                                citations.unchecked.push(written());
                            }
                        }
                    }
                    Endpoint::Skipped => citations.unchecked.push(written()),
                }
            }
            for &file in &files {
                let Some(read) = graph.file(file) else {
                    continue;
                };
                let lines = wiki_links(&read.bytes).into_iter();
                if file == at.file {
                    citations.wiki_links.extend(lines.map(WikiLine::Line));
                } else {
                    citations.wiki_links.extend(lines.map(|line| WikiLine::At {
                        path: graph.paths()[file].to_owned(),
                        line,
                    }));
                }
            }
        }

        // REFs, once each, in (path, position) order.
        let mut refs: Vec<String> = Vec::new();
        let mut details = Vec::new();
        let mut sources: BTreeSet<String> = BTreeSet::new();
        let mut unmapped = 0;
        for &target in &targets {
            let reference = ref_name(graph, resolver, target);
            let target_sources = sources_of(target);
            if target_sources.is_empty() {
                unmapped += 1;
            }
            sources.extend(target_sources.iter().cloned());
            details.push(TargetDetail {
                reference: reference.clone(),
                path: graph.paths()[target.file].to_owned(),
                line: graph.line(target),
                sources: target_sources,
            });
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
        if at.is_none() {
            sources.insert(document.source.clone());
        }

        // The before slots, then W_before over the distinct paths.
        let (tier1, tier1_choice) = tier1_of(config, at.and_then(|at| graph.node(at)), bytes);
        let shard = protocol.shard_of(&document.source).map(str::to_owned);
        let mut slot_paths: BTreeSet<String> = BTreeSet::new();
        let mut slots = Vec::new();
        let mut before_slot = |slot: &'static str, path: &str, bytes: &mut Bytes| {
            slot_paths.insert(path.to_owned());
            slots.push(SlotDetail {
                slot,
                side: SIDE_BEFORE,
                path: path.to_owned(),
                bytes: bytes.corpus(path),
                read: READ_CORPUS,
            });
        };
        if let Some((path, _)) = &protocol.tier0 {
            before_slot(TIER0_SLOT, path, bytes);
        }
        if let Some(path) = &tier1 {
            before_slot(TIER1_SLOT, path, bytes);
        }
        if let Some((path, _)) = &protocol.index {
            before_slot(INDEX_SLOT, path, bytes);
        }
        if let Some(path) = &shard {
            before_slot(SHARD_SLOT, path, bytes);
        }
        let mut after_slots = 0;
        for (slot, path) in [
            (TIER0_SLOT, protocol.tier0.as_ref().map(|(path, _)| path)),
            (TIER1_SLOT, tier1.as_ref()),
        ] {
            if let Some(path) = path {
                let (after, read) = bytes.after(path);
                after_slots += after;
                slots.push(SlotDetail {
                    slot,
                    side: SIDE_AFTER,
                    path: path.clone(),
                    bytes: after,
                    read,
                });
            }
        }
        let docs_needed = sources
            .iter()
            .filter(|source| !slot_paths.contains(*source))
            .count();
        let w_before = slot_paths
            .iter()
            .chain(sources.iter())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|path| bytes.corpus(path))
            .sum();
        tasks.push(Task {
            name,
            source: document.source.clone(),
            after: document.after.clone(),
            document: at,
            refs,
            targets: details,
            citations,
            slots,
            tier1: tier1_choice,
            shard: shard.is_some(),
            unmapped,
            w_before,
            docs_needed,
            after_slots,
        });
    }
    tasks
}

/// The node of a definition layout moved out of its document: in its after
/// file, the first node holding its ID (a record file's document node),
/// else that file's document; `None` when the tree has no such document.
fn record_node(graph: &SpecGraph<'_>, emitted: &Emitted) -> Option<NodeAt> {
    let file = graph.file_of(emitted.after.as_deref()?)?;
    graph
        .nodes(file)
        .iter()
        .position(|node| node.id.as_deref() == Some(emitted.id.as_str()))
        .map(|ord| NodeAt { file, ord })
        .or_else(|| graph.document(file))
}

/// A link the tree marks dangling or unchecked, read on the untouched
/// corpus.
enum Relocated {
    /// A file link naming a walked source document from D's source: the
    /// document node at its after path (layout moved an end).
    Node(NodeAt),
    /// It names one, but the tree has no document at its after path.
    NoNode,
    /// A file link core never checks on the corpus: not a document path,
    /// or no tried candidate in the before walk scope.
    Unchecked,
    /// A file link dangling on the corpus too (a tried candidate in the
    /// before walk scope), or not a file link.
    Dangling,
}

/// A link the tree marks dangling or unchecked (layout moved an end, or D
/// itself and the link leaves the tree's walk scope), written in D or in a
/// record moved out of it: a Markdown file link read on the corpus from D's
/// source (`source`) as core would ([`CorpusLinks::classify`]); one naming
/// a walked source document is mapped through the emission map to its
/// after path (docs/features/pilot-w.md AC-02).
fn relocated(
    graph: &SpecGraph<'_>,
    links: &CorpusLinks<'_>,
    after_of: &BTreeMap<&str, &str>,
    edge: &Edge,
    source: &str,
) -> Relocated {
    let Some(target) = path_target(graph, edge).filter(|_| edge.origin == LinkOrigin::Inline)
    else {
        return Relocated::Dangling;
    };
    let named = match links.classify(source, &target.path) {
        CorpusEnd::Walked(named) => named,
        CorpusEnd::Unchecked => return Relocated::Unchecked,
        CorpusEnd::Dangling => return Relocated::Dangling,
    };
    match after_of
        .get(named.as_str())
        .and_then(|after| graph.file_of(after))
        .and_then(|file| graph.document(file))
    {
        Some(node) => Relocated::Node(node),
        None => Relocated::NoNode,
    }
}

/// The `path[#anchor]` an edge is written as: the inline Markdown file
/// link, or the path-form `canon:`, at the edge's offset in its file;
/// `None` for a reference.
fn path_target<'g>(graph: &SpecGraph<'g>, edge: &Edge) -> Option<&'g PathTarget> {
    let parsed = graph.file(edge.file)?.parsed.as_ref()?;
    let written_at =
        |target: &PathTarget| target.span.is_some_and(|span| span.start == edge.offset);
    match edge.origin {
        LinkOrigin::Inline => parsed.links.iter().find_map(|link| match &link.dst {
            LinkTarget::Path(target) if link.origin == LinkOrigin::Inline && written_at(target) => {
                Some(target)
            }
            _ => None,
        }),
        LinkOrigin::Frontmatter => match parsed.document()?.fields.as_ref()?.canon.as_ref()? {
            CanonTarget::Path(target) if written_at(target) => Some(target),
            CanonTarget::Path(_) | CanonTarget::Reference(_) => None,
        },
    }
}

/// The REF a node is asked by (AC-02): `<slug>/ID` for a feature-scoped ID
/// in a feature document (bare, it names every feature's holder), else its
/// ID, else its path; never the form written in the task.
fn ref_name(graph: &SpecGraph<'_>, resolver: &Resolver<'_>, at: NodeAt) -> String {
    if let Some(id) = graph.node(at).and_then(|node| node.id.as_deref())
        && is_feature_scoped(graph, id)
        && let Some(slug) = resolver.feature_slug(graph.paths()[at.file])
    {
        return format!("{slug}/{id}");
    }
    graph.name(at)
}

/// The ID's prefix (or legacy alias) is scoped to a feature.
fn is_feature_scoped(graph: &SpecGraph<'_>, id: &str) -> bool {
    let scheme = graph.scheme();
    id.split_once('-')
        .and_then(|(prefix, _)| scheme.prefix(prefix).or_else(|| scheme.alias(prefix)))
        .is_some_and(|spec| spec.scope == IdScope::Feature)
}

/// The line of every `[[…]]` pair: an opening pair closed later on the same
/// line, counted once per pair.
fn wiki_links(bytes: &[u8]) -> Vec<usize> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(open) = rest.find(WIKI_OPEN) {
            let after = &rest[open + WIKI_OPEN.len()..];
            let Some(close) = after.find(WIKI_CLOSE) else {
                break;
            };
            lines.push(index + 1);
            rest = &after[close + WIKI_CLOSE.len()..];
        }
    }
    lines
}

/// The task's Tier 1 README: the values of `key` in the document's
/// front-matter that `[tier1.paths]` maps (several: the largest in corpus
/// bytes, path order on a tie), else `default`, else none.
fn tier1_of(
    config: &TasksConfig,
    document: Option<&Node>,
    bytes: &mut Bytes,
) -> (Option<String>, Tier1Choice) {
    if let (Some(key), Some(node)) = (&config.key, document) {
        let mapped: BTreeSet<&String> = front_strings(node, key)
            .iter()
            .filter_map(|value| config.paths.get(value))
            .collect();
        let mut best: Option<(&String, u64)> = None;
        for path in mapped {
            let size = bytes.corpus(path);
            if best.is_none_or(|(_, largest)| size > largest) {
                best = Some((path, size));
            }
        }
        if let Some((path, _)) = best {
            return (Some(path.clone()), Tier1Choice::Key);
        }
    }
    match &config.default {
        Some(path) => (Some(path.clone()), Tier1Choice::Default),
        None => (None, Tier1Choice::None),
    }
}

/// The strings of the front-matter key `key`: a string, or each string of
/// a list, typed (`Node.fields`) or not (`Node.extra`).
fn front_strings(node: &Node, key: &str) -> Vec<String> {
    let mut values = Vec::new();
    if let Some(fields) = &node.fields
        && let Ok(Value::Object(map)) = serde_json::to_value(fields)
        && let Some(value) = map.get(key)
    {
        match value {
            Value::String(text) => values.push(text.clone()),
            Value::Array(items) => {
                values.extend(items.iter().filter_map(Value::as_str).map(str::to_owned))
            }
            _ => {}
        }
    }
    for entry in node.extra.iter().flatten().filter(|entry| entry.key == key) {
        match &entry.value {
            FmValue::Str(text) => values.push(text.clone()),
            FmValue::Seq(items) => values.extend(items.iter().filter_map(|item| match item {
                FmValue::Str(text) => Some(text.clone()),
                _ => None,
            })),
            _ => {}
        }
    }
    values
}

// ---------------------------------------------------------------------------
// Aggregates (AC-06).

/// Nearest rank over every task, ascending: p = v[⌈p·n/100⌉], median =
/// p50, max = v[n]; a task without bytes ranks above every number, in task
/// order. Also the max's task.
fn statistic(figures: &[Figure], names: &[&str]) -> (Statistic, Option<String>) {
    let mut ranked: Vec<(usize, Figure)> = figures.iter().copied().enumerate().collect();
    ranked.sort_by_key(|&(task, figure)| match figure {
        Figure::Bytes(bytes) => (0, bytes, task),
        Figure::Refused | Figure::Failed => (1, 0, task),
    });
    let at = |p: usize| -> Value {
        nearest_rank(ranked.len(), p)
            .and_then(|rank| ranked.get(rank))
            .map_or(Value::from(0), |(_, figure)| figure.value())
    };
    let worst = ranked
        .last()
        .and_then(|(task, _)| names.get(*task))
        .map(|name| (*name).to_owned());
    (
        Statistic {
            median: at(50),
            p90: at(90),
            max: at(100),
        },
        worst,
    )
}

/// The 0-based index of the nearest-rank `p`-th percentile of `n` values.
fn nearest_rank(n: usize, p: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    Some((p * n).div_ceil(100).max(1) - 1)
}

/// The fill of each bundled task, tokens × 100 / budget, one decimal:
/// median and max.
fn fill(bundled: &[(u32, usize, u32)]) -> Fill {
    let mut tenths: Vec<u64> = bundled
        .iter()
        .map(|&(tokens, _, budget)| {
            (u64::from(tokens) * 1000 + u64::from(budget) / 2) / u64::from(budget.max(1))
        })
        .collect();
    tenths.sort_unstable();
    let at = |p: usize| {
        nearest_rank(tenths.len(), p)
            .and_then(|rank| tenths.get(rank))
            .map_or(0.0, |&tenths| tenths as f64 / 10.0)
    };
    Fill {
        median: at(50),
        max: at(100),
    }
}

/// The bytes over the tokens of the bundled tasks, summed; one decimal.
fn bytes_per_token(bundled: &[(u32, usize, u32)]) -> f64 {
    let tokens: u64 = bundled
        .iter()
        .map(|&(tokens, _, _)| u64::from(tokens))
        .sum();
    let bytes: u64 = bundled.iter().map(|&(_, bytes, _)| bytes as u64).sum();
    (bytes * 10 + tokens / 2)
        .checked_div(tokens)
        .map_or(0.0, |tenths| tenths as f64 / 10.0)
}

/// Median and p90 of pass 1's bundle calls.
fn millis(values: &[u128]) -> Millis {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let at = |p: usize| {
        nearest_rank(sorted.len(), p)
            .and_then(|rank| sorted.get(rank))
            .copied()
            .unwrap_or(0)
    };
    Millis {
        median: at(50),
        p90: at(90),
    }
}

fn summarize(result: &WResult, label: &str, out_dir: &Path, written: &layout::WrittenTree) {
    let word = |value: &Value| value.to_string();
    eprintln!(
        "w [{label}]: {} tasks at {} tokens; W_before median {}, W_after median {}, W_after_followups median {}",
        result.tasks,
        result.budget.tokens,
        word(&result.w_before.median),
        word(&result.w_after.median),
        word(&result.w_after_followups.median),
    );
    eprintln!(
        "  third step {}, incomplete {}, refused {}, failed {}, nondeterministic {}; tree files {}, paths skipped {}",
        result.third_step,
        result.followups.incomplete,
        result.refused,
        result.failed,
        result.nondeterministic,
        written.written,
        written.notes.len(),
    );
    eprintln!("  detail: {}", out_dir.display());
}
