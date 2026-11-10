//! Measurement `scale` (`docs/features/scale-eval.md`): today's read cost
//! on a seeded synthetic corpus, one size after another.
//!
//! The refusals (the flags, `--out` against this repository, a symbolic
//! link at `<out>/scale` or `<out>/scale/<label>`) run before anything is
//! written. Then `<out>/scale/<label>/` is emptied and each `--nodes` size
//! N gets `n<N>/`: the tree [`generate`] builds, written into `tree/` and
//! left past the store's racy window ([`RACY_WAIT`]); its index built cold
//! and its check run through the CLI library; the four reads MCP's
//! `read.rs` makes (`show` without and with links,
//! `search`, `bundle`), each warmed once and timed three times, the best
//! kept; the five whole reads
//! (`docs/features/flow-scope.md` "The timed reads": `flow` at Top, of the
//! first directory's head at depth 1 and of the first stage, `trace
//! --summary`, `tree` at depth 1), timed alike, with no step replay, no
//! trusted twin and no bar, since on a hub the flow and the trace walk;
//! each step the four reads take replayed alone through the public API
//! it calls; the same four reads, and the hub's two, again inside the CLI's
//! `trusted` scope of an `IndexTrust` the first of them walked, as a hub's
//! daemon runs them (`docs/features/read-trust.md` "Eval"), each answer
//! compared with the walking read's; W_after by `w`'s formula
//! (`docs/canon/w-measurement.md` "W_after") over 20 targets; `size.json`.
//! `--corpus-only` stops after the tree.
//!
//! The CLI library runs with an [`Env`] and a [`GitEnv`] of its own: the
//! tree as its current directory, the `home` directory beside it as its
//! home, so the database lives under `--out` and no call sees the
//! process's `HOME`.
//!
//! stdout carries numbers only; IDs, query words and answers' hashes go
//! only to `size.json`, and stderr carries counts and times.

mod generate;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::hint::black_box;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use specengine_cli::{
    BundleRequest, CheckRequest, CheckedTree, Env, FlowRequest, GitEnv, Globals, IndexRequest,
    IndexTrust, Outcome, ProjectRoot, SearchRequest, ShowRequest, TraceRequest, TreeRequest,
    render_text, trusted,
};
use specengine_core::check::{CheckFile, CheckInput, Resolver, SpecGraph};
use specengine_model::IdScheme;
use specengine_store::{
    IndexWriter as _, SEARCH_LIMIT_DEFAULT, SearchQuery, Source as _, SpecIndex as _, WorkingTree,
    b3_hash,
};

use self::generate::{CorpusCounts, Synthetic, Target};
use crate::ScaleArgs;
use crate::census::write_json;
use crate::harness::{self, HarnessError};
use crate::layout;
use crate::w::nearest_rank;

/// The measurement's name and its directory under `--out`.
pub const MEASUREMENT: &str = "scale";

/// The label without `--label`.
const DEFAULT_LABEL: &str = "synthetic";
/// The sizes without `--nodes`, in part nodes.
const DEFAULT_NODES: [usize; 3] = [1_000, 10_000, 100_000];
/// The smallest and the largest size.
const NODES_MIN: usize = 500;
const NODES_MAX: usize = 200_000;
/// The budget without `--budget`, estimated tokens (as `w`).
const DEFAULT_BUDGET: u32 = 10_000;
/// The W targets timed by every read.
const TIMED_TARGETS: usize = 3;
/// How long a written tree waits before its cold build: past the store's
/// racy window of 2 s, so the build stores keys a warm update trusts, as a
/// long-lived tree's are (`docs/features/read-trust.md`,
/// "The key and the racy rule").
const RACY_WAIT: Duration = Duration::from_millis(2_100);
/// The floor of a trusted read's median in `growth_trusted`, microseconds:
/// a millisecond read's growth matters to no one.
const TRUSTED_FLOOR_US: u64 = 10_000;
/// Timed calls of a read or a step; the best is kept.
const TIMED_CALLS: usize = 3;
/// A size's tree, beside its home and `size.json`.
const TREE: &str = "tree";
/// A size's home: its data directory and so its database.
const HOME: &str = "home";
/// A size's detail.
const SIZE_FILE: &str = "size.json";

/// The keys of the reads, as MCP names its tools.
const GET_NODE: &str = "get_node";
const GET_NODE_LINKS: &str = "get_node_links";
const SEARCH: &str = "search";
const GET_CONTEXT_BUNDLE: &str = "get_context_bundle";
const GET_FLOW_TOP: &str = "get_flow_top";
const GET_FLOW_ROOT: &str = "get_flow_root";
const GET_FLOW_STAGE: &str = "get_flow_stage";
const GET_TRACE_SUMMARY: &str = "get_trace_summary";
const GET_TREE: &str = "get_tree";

/// How `calls` names the target of a whole read with no `ROOT`, as
/// `spec flow`'s header names it.
const WHOLE: &str = "all";

// ---------------------------------------------------------------------------
// The flags and the guards.

/// A run the guards let through.
pub struct Setup {
    label: String,
    /// Absolute.
    out: PathBuf,
    sizes: Vec<usize>,
    seed: u64,
    budget: u32,
    /// `flag` or `default`.
    budget_source: &'static str,
    corpus_only: bool,
}

/// The refusals of `scale`, in order: `--label`, `--nodes`, `--budget`,
/// then `--out` against this repository and the symbolic links under it
/// (`layout::check_out`, the repository in the corpus's place; without the
/// repository on disk, the links alone). Any of them: exit 2, nothing
/// written. Then `--out` is created. The label goes to the envelope.
pub fn prepare(args: &ScaleArgs) -> Result<(String, Setup), HarnessError> {
    let refused = |message: String| HarnessError::Refused(format!("{message}; nothing written"));
    let label = args.label.as_deref().unwrap_or(DEFAULT_LABEL).to_owned();
    if !harness::is_single_component(&label) {
        return Err(refused(format!(
            "--label {label:?} is not a single plain path component (it names a directory under --out)"
        )));
    }
    let sizes = parse_nodes(args.nodes.as_deref()).map_err(refused)?;
    let (budget, budget_source) = match args.budget {
        None => (DEFAULT_BUDGET, "default"),
        Some(budget) => match u32::try_from(budget) {
            Ok(tokens) if tokens >= 1 => (tokens, "flag"),
            _ => {
                return Err(refused(format!(
                    "--budget {budget}: the budget is a whole number of estimated tokens from 1 to {}",
                    u32::MAX
                )));
            }
        },
    };
    let out = harness::absolutize(&args.out);
    match harness::repository_root() {
        Some(repository) => layout::check_out(&repository, &out, MEASUREMENT, &label)
            .map_err(HarnessError::Refused)?,
        None => refuse_links(&out, &label)?,
    }
    fs::create_dir_all(&out).map_err(|error| {
        HarnessError::Internal(format!("cannot create --out {}: {error}", out.display()))
    })?;
    Ok((
        label.clone(),
        Setup {
            label,
            out,
            sizes,
            seed: args.seed,
            budget,
            budget_source,
            corpus_only: args.corpus_only,
        },
    ))
}

/// `--nodes`: one size or more, comma-separated, each [`NODES_MIN`] to
/// [`NODES_MAX`], strictly ascending; absent, [`DEFAULT_NODES`].
fn parse_nodes(text: Option<&str>) -> Result<Vec<usize>, String> {
    let Some(text) = text else {
        return Ok(DEFAULT_NODES.to_vec());
    };
    if text.trim().is_empty() {
        return Err("--nodes is empty: give one size or more, such as 1000,10000".to_owned());
    }
    let mut sizes: Vec<usize> = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        let size: usize = part
            .parse()
            .map_err(|_| format!("--nodes {text:?}: {part:?} is not a whole number"))?;
        if !(NODES_MIN..=NODES_MAX).contains(&size) {
            return Err(format!(
                "--nodes {text:?}: {size} is outside {NODES_MIN} to {NODES_MAX} part nodes"
            ));
        }
        if sizes.last().is_some_and(|&last| size <= last) {
            return Err(format!(
                "--nodes {text:?}: the sizes must be strictly ascending, each once"
            ));
        }
        sizes.push(size);
    }
    Ok(sizes)
}

/// The symbolic-link half of `layout::check_out`, for a binary whose
/// repository is not on disk: nothing to compare `--out` with, the links
/// still refused.
fn refuse_links(out: &Path, label: &str) -> Result<(), HarnessError> {
    let measurement = out.join(MEASUREMENT);
    for dir in [measurement.join(label), measurement] {
        if fs::symlink_metadata(&dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(HarnessError::Refused(format!(
                "{} is a symbolic link; nothing written",
                dir.display()
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The result (SE-5: exactly these keys, numbers only).

#[derive(Serialize)]
pub struct ScaleResult {
    seed: u64,
    budget: BudgetOut,
    corpus_only: bool,
    sizes: Vec<SizeEntry>,
}

#[derive(Serialize)]
struct BudgetOut {
    tokens: u32,
    source: &'static str,
}

/// One size: `nodes` and `corpus`, then, unless `--corpus-only`, the
/// measured fields.
#[derive(Serialize)]
struct SizeEntry {
    nodes: usize,
    corpus: CorpusCounts,
    #[serde(flatten)]
    measured: Option<Measured>,
}

#[derive(Serialize)]
struct Measured {
    index: IndexOut,
    check: CheckOut,
    reads: ReadsOut,
    growth: GrowthOut,
    /// The four reads inside a hub's trust.
    reads_trusted: TrustedReads,
    /// As `growth`, each median floored at [`TRUSTED_FLOOR_US`].
    growth_trusted: Growth,
    steps: Steps,
    other_us: OtherUs,
    w_after: WAfter,
    hub: HubOut,
    nondeterministic: usize,
    /// The (read, target) pairs whose trusted answers' BLAKE3 differ from
    /// the walking answer's.
    trusted_differs: usize,
}

#[derive(Serialize)]
struct IndexOut {
    files: usize,
    nodes: usize,
    build_us: u64,
}

#[derive(Serialize)]
struct CheckOut {
    errors: usize,
    warnings: usize,
    us: u64,
}

/// `reads`: the four reads of targets, then the five whole reads.
#[derive(Clone, Copy, Serialize)]
struct ReadsOut {
    #[serde(flatten)]
    targeted: Reads,
    #[serde(flatten)]
    whole: WholeReads,
}

/// The four reads of the timed targets and the queries.
#[derive(Clone, Copy, Serialize)]
struct Reads {
    get_node: Spread,
    get_node_links: Spread,
    search: SearchSpread,
    get_context_bundle: Spread,
}

/// The five whole reads (`docs/features/flow-scope.md` "The timed reads"),
/// each of one target: both figures are its best.
#[derive(Clone, Copy, Serialize)]
struct WholeReads {
    get_flow_top: Spread,
    get_flow_root: Spread,
    get_flow_stage: Spread,
    get_trace_summary: Spread,
    get_tree: Spread,
}

/// Over the timed targets (or the queries), each at its best of three.
#[derive(Clone, Copy, Serialize)]
struct Spread {
    median_us: u64,
    max_us: u64,
}

/// The four reads inside a hub's trust, as [`Reads`] without the hits.
#[derive(Clone, Copy, Serialize)]
struct TrustedReads {
    get_node: Spread,
    get_node_links: Spread,
    search: Spread,
    get_context_bundle: Spread,
}

/// A size's medians, the next size's growth is over.
#[derive(Clone, Copy)]
struct Previous {
    reads: ReadsOut,
    trusted: TrustedReads,
}

#[derive(Clone, Copy, Serialize)]
struct SearchSpread {
    median_us: u64,
    max_us: u64,
    hits_min: usize,
    hits_max: usize,
}

/// `growth`: [`Growth`] of the four reads, then of the five whole reads.
#[derive(Serialize)]
struct GrowthOut {
    #[serde(flatten)]
    targeted: Growth,
    #[serde(flatten)]
    whole: WholeGrowth,
}

/// `median_us` over the previous size's, two decimals; `null` at the first
/// size or over a 0.
#[derive(Serialize)]
struct Growth {
    get_node: Option<f64>,
    get_node_links: Option<f64>,
    search: Option<f64>,
    get_context_bundle: Option<f64>,
}

/// As [`Growth`], of the five whole reads.
#[derive(Serialize)]
struct WholeGrowth {
    get_flow_top: Option<f64>,
    get_flow_root: Option<f64>,
    get_flow_stage: Option<f64>,
    get_trace_summary: Option<f64>,
    get_tree: Option<f64>,
}

/// Each step of the reads replayed alone, at its best of three.
#[derive(Clone, Copy, Serialize)]
struct Steps {
    open_us: u64,
    walk_us: u64,
    hash_us: u64,
    update_us: u64,
    update_parsed: usize,
    /// The most files a warm update read: 0 when every key was trusted.
    update_hashed: usize,
    decode_us: u64,
    reparse_us: u64,
    resolver_us: u64,
    graph_us: u64,
    fts_us: u64,
}

/// Each read's median less the steps it takes, never below 0 (SE-8;
/// [`other_us`]).
#[derive(Serialize)]
struct OtherUs {
    get_node: u64,
    get_node_links: u64,
    search: u64,
    get_context_bundle: u64,
}

#[derive(Serialize)]
struct WAfter {
    median: usize,
    p90: usize,
    max: usize,
    tier0: usize,
    tier1_max: usize,
}

#[derive(Serialize)]
struct HubOut {
    degree: u32,
    links_shown: usize,
    get_node_links_us: u64,
    get_context_bundle_us: u64,
    w_after: usize,
    /// The hub's two reads inside a hub's trust.
    trusted: HubTrusted,
}

#[derive(Serialize)]
struct HubTrusted {
    get_node_links_us: u64,
    get_context_bundle_us: u64,
}

/// `size.json`: the size's entry, then what names the corpus.
#[derive(Serialize)]
struct SizeDetail<'a> {
    #[serde(flatten)]
    entry: &'a SizeEntry,
    digest: String,
    targets: Vec<&'a str>,
    queries: &'a [String],
    hubs: Vec<&'a str>,
    /// By read, then by target ID (by query word for `search`).
    #[serde(skip_serializing_if = "Option::is_none")]
    calls: Option<BTreeMap<&'static str, BTreeMap<String, Call>>>,
    /// By target ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    w_after_bytes: Option<BTreeMap<String, WBytes>>,
}

/// One read of one target: the three timings and the four answers'
/// BLAKE3 (the warm call's first).
#[derive(Serialize)]
struct Call {
    us: Vec<u64>,
    answers: Vec<String>,
}

#[derive(Serialize)]
struct WBytes {
    tier1: usize,
    bundle: usize,
    total: usize,
}

// ---------------------------------------------------------------------------
// The run.

pub fn run(setup: Setup) -> Result<ScaleResult, String> {
    let dir = setup.out.join(MEASUREMENT).join(&setup.label);
    layout::clear(&dir).map_err(|error| format!("cannot clear {}: {error}", dir.display()))?;
    let mut sizes: Vec<SizeEntry> = Vec::with_capacity(setup.sizes.len());
    let mut previous: Option<Previous> = None;
    for &nodes in &setup.sizes {
        let size_dir = dir.join(format!("n{nodes}"));
        let entry = measure_size(&setup, nodes, &size_dir, previous.as_ref())?;
        summarize(&entry);
        previous = entry.measured.as_ref().map(|measured| Previous {
            reads: measured.reads,
            trusted: measured.reads_trusted,
        });
        sizes.push(entry);
    }
    Ok(ScaleResult {
        seed: setup.seed,
        budget: BudgetOut {
            tokens: setup.budget,
            source: setup.budget_source,
        },
        corpus_only: setup.corpus_only,
        sizes,
    })
}

/// One size, its `size.json` written last.
fn measure_size(
    setup: &Setup,
    nodes: usize,
    dir: &Path,
    previous: Option<&Previous>,
) -> Result<SizeEntry, String> {
    let synthetic = generate::generate(setup.seed, nodes);
    let tree = dir.join(TREE);
    generate::write(&synthetic, &tree)?;
    let written = Instant::now();
    if setup.corpus_only {
        let entry = SizeEntry {
            nodes,
            corpus: synthetic.corpus.clone(),
            measured: None,
        };
        write_json(
            &dir.join(SIZE_FILE),
            &size_detail(&synthetic, &entry, None, None),
        )?;
        return Ok(entry);
    }
    let home = dir.join(HOME);
    fs::create_dir_all(&home)
        .map_err(|error| format!("cannot create {}: {error}", home.display()))?;
    std::thread::sleep(RACY_WAIT.saturating_sub(written.elapsed()));
    let session = Session::new(&tree, &home);
    let mut measuring = Measuring::new(&session, setup, nodes, &synthetic);
    let entry = SizeEntry {
        nodes,
        corpus: synthetic.corpus.clone(),
        measured: Some(measuring.all(previous)?),
    };
    let Measuring {
        calls,
        w_after_bytes,
        ..
    } = measuring;
    write_json(
        &dir.join(SIZE_FILE),
        &size_detail(&synthetic, &entry, Some(calls), Some(w_after_bytes)),
    )?;
    Ok(entry)
}

/// `size.json` of `entry`: the corpus's digest and samples beside it.
fn size_detail<'a>(
    synthetic: &'a Synthetic,
    entry: &'a SizeEntry,
    calls: Option<BTreeMap<&'static str, BTreeMap<String, Call>>>,
    w_after_bytes: Option<BTreeMap<String, WBytes>>,
) -> SizeDetail<'a> {
    let samples = &synthetic.samples;
    SizeDetail {
        entry,
        digest: synthetic.digest(),
        targets: samples.targets.iter().map(|t| t.id.as_str()).collect(),
        queries: &samples.queries,
        hubs: samples.hubs.iter().map(|t| t.id.as_str()).collect(),
        calls,
        w_after_bytes,
    }
}

/// The stderr line of a size: counts and times, never a path, an ID or a
/// word of the corpus.
fn summarize(entry: &SizeEntry) {
    let corpus = &entry.corpus;
    let Some(measured) = &entry.measured else {
        eprintln!(
            "{MEASUREMENT}: {} part nodes: {} files, {} nodes, {} bytes written (corpus only)",
            entry.nodes, corpus.files, corpus.nodes, corpus.bytes
        );
        return;
    };
    let ms = |us: u64| format!("{:.1} ms", Duration::from_micros(us).as_secs_f64() * 1000.0);
    let reads = &measured.reads.targeted;
    let trusted = &measured.reads_trusted;
    let parsed = match measured.steps.update_parsed {
        0 => String::new(),
        parsed => format!("; the warm update parsed {parsed} files"),
    };
    eprintln!(
        "{MEASUREMENT}: {} part nodes: {} files, {} nodes; index {}, check {} ({} errors, {} warnings); \
         medians get_node {}, get_node_links {}, search {}, get_context_bundle {}; \
         trusted medians get_node {}, get_node_links {}, search {}, get_context_bundle {}; \
         W_after median {} bytes; {} nondeterministic, {} trusted differ{parsed}",
        entry.nodes,
        corpus.files,
        corpus.nodes,
        ms(measured.index.build_us),
        ms(measured.check.us),
        measured.check.errors,
        measured.check.warnings,
        ms(reads.get_node.median_us),
        ms(reads.get_node_links.median_us),
        ms(reads.search.median_us),
        ms(reads.get_context_bundle.median_us),
        ms(trusted.get_node.median_us),
        ms(trusted.get_node_links.median_us),
        ms(trusted.search.median_us),
        ms(trusted.get_context_bundle.median_us),
        measured.w_after.median,
        measured.nondeterministic,
        measured.trusted_differs,
    );
}

/// What every call of a size takes: `Globals {root: tree}`, `Env {cwd:
/// tree, home: <n>/home}` and git's environment with only `PATH` and that
/// home (never `Env::from_process` or `process_git`).
struct Session {
    env: Env,
    globals: Globals,
    git: GitEnv,
}

impl Session {
    fn new(tree: &Path, home: &Path) -> Self {
        let home = OsString::from(home.as_os_str());
        let mut vars: Vec<(OsString, OsString)> = Vec::new();
        if let Some(path) = std::env::var_os("PATH") {
            vars.push((OsString::from("PATH"), path));
        }
        vars.push((OsString::from("HOME"), home.clone()));
        Self {
            env: Env {
                cwd: tree.to_path_buf(),
                home: Some(home),
                xdg_data_home: None,
            },
            globals: Globals {
                root: Some(tree.to_path_buf()),
                config: None,
            },
            git: GitEnv::new(tree.to_path_buf(), vars),
        }
    }

    /// `get_node` (`links` false) or `get_node_links` (`links` true), as
    /// `read.rs` asks it: no bindings, no archive.
    fn show_request(&self, reference: &str, links: bool) -> ShowRequest {
        ShowRequest {
            reference: reference.to_owned(),
            links,
            archive: false,
            bindings: false,
            git: self.git.clone(),
        }
    }

    /// `search` as `read.rs` asks it: one word, no kinds, the default
    /// limit, no archive.
    fn search_request(word: &str) -> SearchRequest {
        SearchRequest {
            terms: vec![word.to_owned()],
            kinds: Vec::new(),
            limit: None,
            archive: false,
        }
    }

    /// `get_context_bundle` as `read.rs` asks it, at the run's budget.
    fn bundle_request(&self, reference: &str, budget: u32) -> BundleRequest {
        BundleRequest {
            references: vec![reference.to_owned()],
            budget: Some(i64::from(budget)),
            git: self.git.clone(),
        }
    }

    /// The answer rendered, and the links it lists; an error or a
    /// `reason` is the failure.
    fn show(&self, request: &ShowRequest) -> Result<(String, usize), String> {
        let outcome = specengine_cli::show(&self.env, &self.globals, request)
            .map_err(|error| error.message)?;
        if let Some(reason) = &outcome.reason {
            return Err(reason.clone());
        }
        let links = outcome
            .nodes
            .iter()
            .filter_map(|node| node.links.as_ref())
            .map(|links| links.len())
            .sum();
        Ok((render_text(&Outcome::Show(outcome)), links))
    }

    /// `get_flow` as `read.rs` asks it, `trace` true: the CLI's capped
    /// read.
    fn flow_request(root: Option<&str>, depth: Option<i64>, stage: Option<&str>) -> FlowRequest {
        FlowRequest {
            root: root.map(str::to_owned),
            depth,
            stage: stage.map(str::to_owned),
            trace: true,
            in_flow: false,
        }
    }

    /// The answer rendered, and its nodes (`Timed::extra`, which no output
    /// reads); an error or a `reason` is the failure.
    fn flow(&self, request: &FlowRequest) -> Result<(String, usize), String> {
        let outcome = specengine_cli::flow(&self.env, &self.globals, request)
            .map_err(|error| error.message)?;
        if let Some(reason) = &outcome.reason {
            return Err(reason.clone());
        }
        let nodes = outcome.nodes.len();
        Ok((render_text(&Outcome::Flow(Box::new(outcome))), nodes))
    }

    /// `get_trace {summary: true}`: the counts and the lanes, no node row.
    /// The answer rendered, and the nodes it lists (`Timed::extra`, unread).
    fn trace_summary(&self) -> Result<(String, usize), String> {
        let request = TraceRequest {
            reference: None,
            stage: None,
            summary: true,
        };
        let outcome = specengine_cli::trace(&self.env, &self.globals, &request)
            .map_err(|error| error.message)?;
        if let Some(reason) = &outcome.reason {
            return Err(reason.clone());
        }
        let listed = outcome.listed;
        Ok((render_text(&Outcome::Trace(Box::new(outcome))), listed))
    }

    /// `get_tree {depth: 1}`, no kinds, no archive. The answer rendered,
    /// and its lines (`Timed::extra`, unread).
    fn tree(&self) -> Result<(String, usize), String> {
        let request = TreeRequest {
            root: None,
            depth: Some(1),
            kinds: Vec::new(),
            archive: false,
        };
        let outcome = specengine_cli::tree(&self.env, &self.globals, &request)
            .map_err(|error| error.message)?;
        if let Some(reason) = &outcome.reason {
            return Err(reason.clone());
        }
        let lines = outcome.nodes.len();
        Ok((render_text(&Outcome::Tree(outcome)), lines))
    }

    /// The answer rendered, and its hits.
    fn search(&self, request: &SearchRequest) -> Result<(String, usize), String> {
        let outcome = specengine_cli::search(&self.env, &self.globals, request)
            .map_err(|error| error.message)?;
        let hits = outcome.hits.len();
        Ok((render_text(&Outcome::Search(outcome)), hits))
    }

    /// The answer rendered, and `Bundle.bytes`.
    fn bundle(&self, request: &BundleRequest) -> Result<(String, usize), String> {
        let outcome = specengine_cli::bundle(&self.env, &self.globals, request)
            .map_err(|error| error.message)?;
        if let Some(reason) = &outcome.reason {
            return Err(reason.clone());
        }
        let bytes = outcome.bundle.as_ref().map_or(0, |bundle| bundle.bytes);
        Ok((render_text(&Outcome::Bundle(outcome)), bytes))
    }
}

/// One read of one target four times: an unrecorded warm call, then three
/// timed calls (the call and its rendering), each answer hashed after its
/// timer stops.
struct Timed {
    best: u64,
    us: Vec<u64>,
    answers: Vec<String>,
    /// The warm answer's links, hits or bytes.
    extra: usize,
}

impl Timed {
    fn run(mut call: impl FnMut() -> Result<(String, usize), String>) -> Result<Self, String> {
        let (warm, extra) = call()?;
        let mut answers = vec![b3_hash(warm.as_bytes())];
        let mut us = Vec::with_capacity(TIMED_CALLS);
        for _ in 0..TIMED_CALLS {
            let started = Instant::now();
            let (text, _) = call()?;
            us.push(micros(started.elapsed()));
            answers.push(b3_hash(text.as_bytes()));
        }
        let best = us.iter().copied().min().unwrap_or(0);
        Ok(Self {
            best,
            us,
            answers,
            extra,
        })
    }

    fn deterministic(&self) -> bool {
        self.answers.windows(2).all(|pair| pair[0] == pair[1])
    }
}

/// One size's measuring, after its tree is written.
struct Measuring<'a> {
    session: &'a Session,
    nodes: usize,
    budget: u32,
    synthetic: &'a Synthetic,
    calls: BTreeMap<&'static str, BTreeMap<String, Call>>,
    w_after_bytes: BTreeMap<String, WBytes>,
    nondeterministic: usize,
    /// The reads now run inside a trust: compared, not recorded.
    in_trust: bool,
    trusted_differs: usize,
}

impl<'a> Measuring<'a> {
    fn new(session: &'a Session, setup: &Setup, nodes: usize, synthetic: &'a Synthetic) -> Self {
        Self {
            session,
            nodes,
            budget: setup.budget,
            synthetic,
            calls: BTreeMap::new(),
            w_after_bytes: BTreeMap::new(),
            nondeterministic: 0,
            in_trust: false,
            trusted_differs: 0,
        }
    }

    /// Index, check, reads, the whole reads, steps, W_after and the hub, in
    /// that order.
    fn all(&mut self, previous: Option<&Previous>) -> Result<Measured, String> {
        let index = self.index()?;
        let check = self.check()?;
        let (targeted, hub_reads) = self.reads()?;
        let whole = self.whole_reads()?;
        let reads = ReadsOut { targeted, whole };
        let (reads_trusted, hub_trusted) = self.trusted_reads()?;
        let steps = self.steps()?;
        let (w_after, hub_w_after) = self.w_after()?;
        let other_us = other_us(&reads.targeted, &steps);
        let samples = &self.synthetic.samples;
        let before = previous.map(|previous| &previous.reads);
        Ok(Measured {
            index,
            check,
            growth: GrowthOut {
                targeted: growth(&reads.targeted, before.map(|before| &before.targeted)),
                whole: whole_growth(&reads.whole, before.map(|before| &before.whole)),
            },
            growth_trusted: growth_trusted(
                &reads_trusted,
                previous.map(|previous| &previous.trusted),
            ),
            reads,
            reads_trusted,
            steps,
            other_us,
            w_after,
            hub: HubOut {
                degree: samples.hub_degrees.first().copied().unwrap_or(0),
                links_shown: hub_reads.links_shown,
                get_node_links_us: hub_reads.get_node_links_us,
                get_context_bundle_us: hub_reads.get_context_bundle_us,
                w_after: hub_w_after,
                trusted: HubTrusted {
                    get_node_links_us: hub_trusted.get_node_links_us,
                    get_context_bundle_us: hub_trusted.get_context_bundle_us,
                },
            },
            nondeterministic: self.nondeterministic,
            trusted_differs: self.trusted_differs,
        })
    }

    /// [`Self::reads`] again inside the CLI's `trusted` scope of a new
    /// `IndexTrust` of the tree, as a hub's daemon runs them: the first
    /// warm call walks, the others trust. Each answer is compared with the
    /// walking read's, never recorded.
    fn trusted_reads(&mut self) -> Result<(TrustedReads, HubReads), String> {
        let project = self.project()?;
        let trust = IndexTrust::new(project.root.clone());
        self.in_trust = true;
        let read = trusted(&[&trust], || self.reads());
        self.in_trust = false;
        let (reads, hub) = read?;
        let search = reads.search;
        Ok((
            TrustedReads {
                get_node: reads.get_node,
                get_node_links: reads.get_node_links,
                search: Spread {
                    median_us: search.median_us,
                    max_us: search.max_us,
                },
                get_context_bundle: reads.get_context_bundle,
            },
            hub,
        ))
    }

    /// The cold build through `specengine_cli::index`, timed; the stored
    /// files and nodes.
    fn index(&self) -> Result<IndexOut, String> {
        let session = self.session;
        let started = Instant::now();
        let built = specengine_cli::index(
            &session.env,
            &session.globals,
            &IndexRequest { full: false },
        )
        .map_err(|error| self.failed("index", &error.message))?;
        let build_us = micros(started.elapsed());
        let project = self.project()?;
        let open = specengine_cli::open_index(&session.env, &project)
            .map_err(|error| self.failed("index", &error.message))?;
        let input = open
            .index
            .indexed_input()
            .map_err(|error| self.failed("index", &error.to_string()))?;
        let nodes = input
            .files
            .iter()
            .filter_map(|file| file.parsed.as_ref())
            .map(|parsed| parsed.nodes.len())
            .sum();
        Ok(IndexOut {
            files: built.report.walked,
            nodes,
            build_us,
        })
    }

    /// `specengine_cli::check` over the working tree, no baseline, timed:
    /// its errors and warnings, whatever they are.
    fn check(&self) -> Result<CheckOut, String> {
        let session = self.session;
        let request = CheckRequest {
            tree: CheckedTree::WorkingTree,
            baseline: None,
            debt: false,
            json: false,
        };
        let started = Instant::now();
        let checked = specengine_cli::check(&session.env, &session.globals, &request)
            .map_err(|error| self.failed("check", &error.message))?;
        let us = micros(started.elapsed());
        Ok(CheckOut {
            errors: checked.report.counts.errors,
            warnings: checked.report.counts.warnings,
            us,
        })
    }

    /// The four reads on the timed targets and the queries, then the hub's
    /// two reads.
    fn reads(&mut self) -> Result<(Reads, HubReads), String> {
        let session = self.session;
        let synthetic = self.synthetic;
        let samples = &synthetic.samples;
        let timed_targets = &samples.targets[..TIMED_TARGETS.min(samples.targets.len())];
        let mut node_us = Vec::with_capacity(TIMED_TARGETS);
        let mut links_us = Vec::with_capacity(TIMED_TARGETS);
        let mut bundle_us = Vec::with_capacity(TIMED_TARGETS);
        for (ordinal, target) in timed_targets.iter().enumerate() {
            let what = format!("target {}", ordinal + 1);
            let request = session.show_request(&target.id, false);
            let timed = self.timed(GET_NODE, &target.id, &what, || session.show(&request))?;
            node_us.push(timed);
            let request = session.show_request(&target.id, true);
            let timed = self.timed(GET_NODE_LINKS, &target.id, &what, || session.show(&request))?;
            links_us.push(timed);
            let request = session.bundle_request(&target.id, self.budget);
            let timed = self.timed(GET_CONTEXT_BUNDLE, &target.id, &what, || {
                session.bundle(&request)
            })?;
            bundle_us.push(timed);
        }
        let mut search_us = Vec::with_capacity(samples.queries.len());
        let mut hits: Vec<usize> = Vec::with_capacity(samples.queries.len());
        for (ordinal, word) in samples.queries.iter().enumerate() {
            let what = format!("query {}", ordinal + 1);
            let request = Session::search_request(word);
            let (best, found) = {
                let timed = Timed::run(|| session.search(&request))
                    .map_err(|reason| self.failed(&format!("{SEARCH}, {what}"), &reason))?;
                let best = timed.best;
                let found = timed.extra;
                self.record(SEARCH, word, timed);
                (best, found)
            };
            search_us.push(best);
            hits.push(found);
        }
        let hub = samples
            .hubs
            .first()
            .ok_or_else(|| format!("no hub at {} nodes", self.nodes))?;
        let request = session.show_request(&hub.id, true);
        let (hub_links_us, links_shown) = {
            let timed = Timed::run(|| session.show(&request))
                .map_err(|reason| self.failed(&format!("{GET_NODE_LINKS}, the hub"), &reason))?;
            let pair = (timed.best, timed.extra);
            self.record(GET_NODE_LINKS, &hub.id, timed);
            pair
        };
        let request = session.bundle_request(&hub.id, self.budget);
        let hub_bundle_us = self.timed(GET_CONTEXT_BUNDLE, &hub.id, "the hub", || {
            session.bundle(&request)
        })?;
        let search_spread = spread(&search_us);
        Ok((
            Reads {
                get_node: spread(&node_us),
                get_node_links: spread(&links_us),
                search: SearchSpread {
                    median_us: search_spread.median_us,
                    max_us: search_spread.max_us,
                    hits_min: hits.iter().copied().min().unwrap_or(0),
                    hits_max: hits.iter().copied().max().unwrap_or(0),
                },
                get_context_bundle: spread(&bundle_us),
            },
            HubReads {
                links_shown,
                get_node_links_us: hub_links_us,
                get_context_bundle_us: hub_bundle_us,
            },
        ))
    }

    /// The five whole reads, as MCP calls them (`Capped`), each warmed once
    /// and timed three times like the four, recorded in `calls` (by the
    /// head's ID, the stage word, else [`WHOLE`]). Never inside a trust.
    fn whole_reads(&mut self) -> Result<WholeReads, String> {
        let session = self.session;
        let samples = &self.synthetic.samples;
        let top = Session::flow_request(None, Some(0), None);
        let flow_top = self.timed(GET_FLOW_TOP, WHOLE, "the whole flow", || session.flow(&top))?;
        let root = Session::flow_request(Some(&samples.flow_root), Some(1), None);
        let flow_root = self.timed(GET_FLOW_ROOT, &samples.flow_root, "the head", || {
            session.flow(&root)
        })?;
        let stage = Session::flow_request(None, None, Some(&samples.stage));
        let flow_stage = self.timed(GET_FLOW_STAGE, &samples.stage, "the stage", || {
            session.flow(&stage)
        })?;
        let trace_summary = self.timed(GET_TRACE_SUMMARY, WHOLE, "the summary", || {
            session.trace_summary()
        })?;
        let tree = self.timed(GET_TREE, WHOLE, "depth 1", || session.tree())?;
        Ok(WholeReads {
            get_flow_top: spread(&[flow_top]),
            get_flow_root: spread(&[flow_root]),
            get_flow_stage: spread(&[flow_stage]),
            get_trace_summary: spread(&[trace_summary]),
            get_tree: spread(&[tree]),
        })
    }

    /// [`Timed::run`] of `read` on `target` (named `what` on failure),
    /// recorded; its best.
    fn timed(
        &mut self,
        read: &'static str,
        target: &str,
        what: &str,
        call: impl FnMut() -> Result<(String, usize), String>,
    ) -> Result<u64, String> {
        let timed =
            Timed::run(call).map_err(|reason| self.failed(&format!("{read}, {what}"), &reason))?;
        let best = timed.best;
        self.record(read, target, timed);
        Ok(best)
    }

    fn record(&mut self, read: &'static str, target: &str, timed: Timed) {
        if self.in_trust {
            let walking = self
                .calls
                .get(read)
                .and_then(|calls| calls.get(target))
                .and_then(|call| call.answers.first());
            if timed.answers.iter().any(|answer| Some(answer) != walking) {
                self.trusted_differs += 1;
            }
            return;
        }
        if !timed.deterministic() {
            self.nondeterministic += 1;
        }
        self.calls.entry(read).or_default().insert(
            target.to_owned(),
            Call {
                us: timed.us,
                answers: timed.answers,
            },
        );
    }

    /// Each step of the reads replayed alone on the warm index, at its
    /// best of three (SE-8): discovery and the
    /// open, the walk, the reading and hashing, the update, the decoding,
    /// the re-parse (CLI `corpus.rs`'s `read_bytes`), the resolver, the
    /// graph and the full-text query.
    fn steps(&self) -> Result<Steps, String> {
        let session = self.session;
        let fail = |error: String| self.failed("the steps", &error);
        let project = self.project()?;
        let scheme = &project.config.scheme;
        let paths = &project.config.paths;

        let open_us = best(|| {
            let project = specengine_cli::discover(&session.env, &session.globals)
                .map_err(|error| error.message)?;
            specengine_cli::open_index(&session.env, &project).map_err(|error| error.message)
        })
        .map_err(fail)?;
        let mut open = specengine_cli::open_index(&session.env, &project)
            .map_err(|error| fail(error.message))?;

        let walk_us = best(|| {
            let tree = WorkingTree::new(&project.root, paths).map_err(|error| error.to_string())?;
            tree.list().map_err(|error| error.to_string())
        })
        .map_err(fail)?;
        let tree =
            WorkingTree::new(&project.root, paths).map_err(|error| fail(error.to_string()))?;
        let listing = tree.list().map_err(|error| fail(error.to_string()))?;

        let hash_us = best(|| {
            for path in &listing.paths {
                let bytes = tree
                    .read(path)
                    .map_err(|error| format!("cannot read `{path}`: {error}"))?;
                black_box(b3_hash(&bytes));
            }
            Ok(())
        })
        .map_err(fail)?;

        let mut update_parsed = 0;
        let mut update_hashed = 0;
        let update_us = best(|| {
            let tree = WorkingTree::new(&project.root, paths).map_err(|error| error.to_string())?;
            let report = open
                .index
                .update(&tree, scheme)
                .map_err(|error| error.to_string())?;
            update_parsed = update_parsed.max(report.parsed);
            update_hashed = update_hashed.max(report.hashed);
            Ok(report)
        })
        .map_err(fail)?;

        let mut decode_us = u64::MAX;
        let mut resolver_us = u64::MAX;
        let mut reparse_us = u64::MAX;
        let mut graph_us = u64::MAX;
        for _ in 0..TIMED_CALLS {
            let started = Instant::now();
            let mut input = open
                .index
                .indexed_input()
                .map_err(|error| fail(error.to_string()))?;
            decode_us = decode_us.min(micros(started.elapsed()));

            let started = Instant::now();
            let resolver = Resolver::new(&input, scheme, paths);
            resolver_us = resolver_us.min(micros(started.elapsed()));
            black_box(resolver);

            let started = Instant::now();
            read_bytes(&mut input, &tree, scheme);
            reparse_us = reparse_us.min(micros(started.elapsed()));

            let started = Instant::now();
            let graph = SpecGraph::new(&input, scheme, paths);
            graph_us = graph_us.min(micros(started.elapsed()));
            black_box(graph);
        }

        let query = SearchQuery {
            text: self
                .synthetic
                .samples
                .queries
                .first()
                .cloned()
                .unwrap_or_default(),
            kinds: Vec::new(),
            limit: SEARCH_LIMIT_DEFAULT,
            archive: false,
        };
        let fts_us =
            best(|| open.index.search(&query).map_err(|error| error.to_string())).map_err(fail)?;

        Ok(Steps {
            open_us,
            walk_us,
            hash_us,
            update_us,
            update_parsed,
            update_hashed,
            decode_us,
            reparse_us,
            resolver_us,
            graph_us,
            fts_us,
        })
    }

    /// The 20 targets' and the hub's W_after (`docs/canon/w-measurement.md`
    /// "W_after"): Tier 0 + the target's Tier 1 + `Bundle.bytes` at the
    /// budget; the statistics by `w`'s nearest rank.
    fn w_after(&mut self) -> Result<(WAfter, usize), String> {
        let synthetic = self.synthetic;
        let tier0 = synthetic.tier0_bytes;
        let mut totals = Vec::with_capacity(synthetic.samples.targets.len());
        let mut tier1_max = 0;
        for (ordinal, target) in synthetic.samples.targets.iter().enumerate() {
            let what = format!("W_after, target {}", ordinal + 1);
            let (tier1, bundle, total) = self.w_bytes(target, &what)?;
            tier1_max = tier1_max.max(tier1);
            totals.push(total);
            self.w_after_bytes.insert(
                target.id.clone(),
                WBytes {
                    tier1,
                    bundle,
                    total,
                },
            );
        }
        let hub = synthetic
            .samples
            .hubs
            .first()
            .ok_or_else(|| format!("no hub at {} nodes", self.nodes))?;
        let (_, _, hub_total) = self.w_bytes(hub, "W_after, the hub")?;
        totals.sort_unstable();
        let at = |p: usize| {
            nearest_rank(totals.len(), p)
                .and_then(|rank| totals.get(rank))
                .copied()
                .unwrap_or(0)
        };
        Ok((
            WAfter {
                median: at(50),
                p90: at(90),
                max: at(100),
                tier0,
                tier1_max,
            },
            hub_total,
        ))
    }

    /// One bundle of `target` for W_after: its Tier 1, its bundle and the
    /// total with Tier 0.
    fn w_bytes(&self, target: &Target, what: &str) -> Result<(usize, usize, usize), String> {
        let session = self.session;
        let request = session.bundle_request(&target.id, self.budget);
        let (_, bundle) = session
            .bundle(&request)
            .map_err(|reason| self.failed(&format!("{GET_CONTEXT_BUNDLE}, {what}"), &reason))?;
        let tier1 = self
            .synthetic
            .tier1_bytes
            .get(target.area)
            .copied()
            .unwrap_or(0);
        Ok((tier1, bundle, self.synthetic.tier0_bytes + tier1 + bundle))
    }

    fn project(&self) -> Result<ProjectRoot, String> {
        specengine_cli::discover(&self.session.env, &self.session.globals)
            .map_err(|error| self.failed("discover", &error.message))
    }

    /// The failure of `what` at this size, the reason as answered.
    fn failed(&self, what: &str, reason: &str) -> String {
        format!("{what} at {} nodes: {reason}", self.nodes)
    }
}

/// The hub's two reads.
struct HubReads {
    links_shown: usize,
    get_node_links_us: u64,
    get_context_bundle_us: u64,
}

/// The best of [`TIMED_CALLS`] runs of `step`, its value kept past the
/// timer (and so its drop).
fn best<T>(mut step: impl FnMut() -> Result<T, String>) -> Result<u64, String> {
    let mut best = u64::MAX;
    for _ in 0..TIMED_CALLS {
        let started = Instant::now();
        let value = step()?;
        best = best.min(micros(started.elapsed()));
        black_box(value);
    }
    Ok(best)
}

/// CLI `corpus.rs`'s private `read_bytes`, replayed: every file with a
/// parse read again through the tree and parsed again by core, into
/// `CheckFile::parsed`; a file that cannot be read or parsed keeps its
/// decoded parse (the CLI's warning is not built).
fn read_bytes(input: &mut CheckInput, tree: &WorkingTree, scheme: &IdScheme) {
    for file in &mut input.files {
        if file.parsed.is_none() {
            continue;
        }
        let Ok(bytes) = tree.read(&file.path) else {
            continue;
        };
        let path = file.path.clone();
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(&path, &bytes, scheme)
        }));
        if let Ok(parsed) = parsed {
            *file = CheckFile::parsed(path, bytes, parsed);
        }
    }
}

/// Median (nearest rank) and max of `values`.
fn spread(values: &[u64]) -> Spread {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let at = |p: usize| {
        nearest_rank(sorted.len(), p)
            .and_then(|rank| sorted.get(rank))
            .copied()
            .unwrap_or(0)
    };
    Spread {
        median_us: at(50),
        max_us: at(100),
    }
}

/// `other_us` (`docs/features/scale-eval.md` "Acceptance criteria",
/// SE-8): each read's median less the steps it takes today, never below
/// 0. `get_node`, `get_node_links` and `get_context_bundle` read a subset
/// of the index since RM-0079 (`docs/features/store-reads.md` "Eval"):
/// open and update alone; `search`: open, update, the full-text query. The
/// other steps are still replayed and reported: what the whole reads cost.
/// `update_us` covers `walk_us` and `hash_us`, never added again.
fn other_us(reads: &Reads, steps: &Steps) -> OtherUs {
    let opened = steps.open_us + steps.update_us;
    OtherUs {
        get_node: reads.get_node.median_us.saturating_sub(opened),
        get_node_links: reads.get_node_links.median_us.saturating_sub(opened),
        search: reads.search.median_us.saturating_sub(opened + steps.fts_us),
        get_context_bundle: reads.get_context_bundle.median_us.saturating_sub(opened),
    }
}

/// `now` over `before`, two decimals; `None` with no `before` or a 0.
fn ratio(now: u64, before: Option<u64>) -> Option<f64> {
    let before = before.filter(|&before| before > 0)?;
    Some((now as f64 / before as f64 * 100.0).round() / 100.0)
}

fn growth(reads: &Reads, previous: Option<&Reads>) -> Growth {
    Growth {
        get_node: ratio(
            reads.get_node.median_us,
            previous.map(|p| p.get_node.median_us),
        ),
        get_node_links: ratio(
            reads.get_node_links.median_us,
            previous.map(|p| p.get_node_links.median_us),
        ),
        search: ratio(reads.search.median_us, previous.map(|p| p.search.median_us)),
        get_context_bundle: ratio(
            reads.get_context_bundle.median_us,
            previous.map(|p| p.get_context_bundle.median_us),
        ),
    }
}

/// [`growth`] of the five whole reads.
fn whole_growth(reads: &WholeReads, previous: Option<&WholeReads>) -> WholeGrowth {
    WholeGrowth {
        get_flow_top: ratio(
            reads.get_flow_top.median_us,
            previous.map(|p| p.get_flow_top.median_us),
        ),
        get_flow_root: ratio(
            reads.get_flow_root.median_us,
            previous.map(|p| p.get_flow_root.median_us),
        ),
        get_flow_stage: ratio(
            reads.get_flow_stage.median_us,
            previous.map(|p| p.get_flow_stage.median_us),
        ),
        get_trace_summary: ratio(
            reads.get_trace_summary.median_us,
            previous.map(|p| p.get_trace_summary.median_us),
        ),
        get_tree: ratio(
            reads.get_tree.median_us,
            previous.map(|p| p.get_tree.median_us),
        ),
    }
}

/// `growth_trusted`: each median and its previous size's floored at
/// [`TRUSTED_FLOOR_US`], their ratio to two decimals; `null` at the first
/// size.
fn growth_trusted(reads: &TrustedReads, previous: Option<&TrustedReads>) -> Growth {
    let ratio = |now: u64, before: Option<u64>| -> Option<f64> {
        let before = before?.max(TRUSTED_FLOOR_US);
        let now = now.max(TRUSTED_FLOOR_US);
        Some((now as f64 / before as f64 * 100.0).round() / 100.0)
    };
    Growth {
        get_node: ratio(
            reads.get_node.median_us,
            previous.map(|p| p.get_node.median_us),
        ),
        get_node_links: ratio(
            reads.get_node_links.median_us,
            previous.map(|p| p.get_node_links.median_us),
        ),
        search: ratio(reads.search.median_us, previous.map(|p| p.search.median_us)),
        get_context_bundle: ratio(
            reads.get_context_bundle.median_us,
            previous.map(|p| p.get_context_bundle.median_us),
        ),
    }
}

/// Whole microseconds, saturating.
fn micros(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
}
