//! Measurement `ra` (layer C of 05 §5.1, ADR-0020; loader:
//! `crates/specengine-ra/README.md`): `specengine-ra` loads the corpus with
//! and without the proc-macro server; per load: cold time, peak RSS, warm
//! re-analysis after an in-memory edit, and the share of source items with a
//! moniker.
//!
//! Each load runs in a worker process of its own (`ra-worker`, hidden), in
//! its own process group, under the per-load budget `--timeout`: on overrun
//! the whole group — worker, cargo, rustc, proc-macro server — is killed and
//! every field the worker had not reported is `"timeout"`. While the load
//! runs, the harness samples the summed resident size of the whole group
//! (the verdict's peak RSS, see [`ModeResult::group_peak_rss_mb`]).
//!
//! Read-only: every cargo invocation writes to `--cargo-target-dir` (default
//! `<out>/ra/<label>/target`, refused under the corpus) with `--locked
//! --offline`; the worker's temporary files go to `<out>/ra/<label>/<mode>/tmp`.
//! Build scripts and proc macros of the corpus run only in the `with` load:
//! they are the corpus's own code, so pilots are measured on scratch copies.
//!
//! stdout: counts and times only. Per load, `--out/ra/<label>/<mode>/`
//! receives `items.json` (path, line, kind, name, status, moniker per item),
//! `errors.json` (load-step error texts) and `warm.json` (the edited file).

mod events;
mod sys;
mod worker;

use std::fs;
use std::io::{self, BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use clap::ValueEnum;
use serde::Serialize;

use crate::harness::{self, Corpus};
use events::{BuildScriptsStep, DatabaseStep, Event, MetadataStep, PassCounts, PassStep};

pub use worker::{WorkerArgs, main as worker_main};

/// Fixture directory (relative to `fixtures/`) used without `--pilot`: a
/// small Cargo workspace of its own with a `Cargo.lock`, a build script and a
/// path proc-macro crate, so both loads run in full.
pub const FIXTURE: &str = "ra-mini";

/// Slack of the harness's outer budget over the per-load budgets; the outer
/// one only fires if the supervisor itself hangs.
const OUTER_SLACK_S: u64 = 60;

/// How long a worker that sent its last report (`done` or `load_failed`)
/// may take to exit before its process group is killed; not part of the
/// load's budget.
const AFTER_REPORT_GRACE: Duration = Duration::from_secs(10);

/// Pause between two samples of the process group's resident size.
const GROUP_RSS_SAMPLE: Duration = Duration::from_millis(100);

/// How long, after the group is killed and the worker reaped, the stdout
/// reader may take to see the pipe close; a descendant that left the group
/// and still holds the pipe must not hang the harness (the reader is then
/// left blocked until that process closes it).
const READER_JOIN_GRACE: Duration = Duration::from_secs(2);

/// Which loads to measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Modes {
    Both,
    With,
    Without,
}

/// One load: with or without build scripts and the proc-macro server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    With,
    Without,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::With => "with",
            Mode::Without => "without",
        }
    }
}

impl Modes {
    fn list(self) -> &'static [Mode] {
        match self {
            Modes::Both => &[Mode::Without, Mode::With],
            Modes::With => &[Mode::With],
            Modes::Without => &[Mode::Without],
        }
    }
}

/// The harness's outer budget for `modes` loads of `per_load_s` seconds each.
pub fn outer_budget(per_load_s: u64, modes: Modes) -> u64 {
    let loads = u64::try_from(modes.list().len()).unwrap_or(2);
    per_load_s
        .saturating_mul(loads)
        .saturating_add(OUTER_SLACK_S)
}

/// Checked before anything is written (a refusal, exit 2 on error).
pub struct Setup {
    cargo_target_dir: Option<PathBuf>,
}

/// The corpus must be a Cargo project; an explicit `--cargo-target-dir` must
/// not lie under it; every deadline of the run (at most twice the outer
/// budget from now) must be representable by the system clock.
pub fn prepare(
    root: &Path,
    cargo_target_dir: Option<&Path>,
    per_load_s: u64,
    modes: Modes,
) -> Result<Setup, String> {
    let horizon = Duration::from_secs(outer_budget(per_load_s, modes)).saturating_mul(2);
    if Instant::now().checked_add(horizon).is_none() {
        return Err(format!(
            "--timeout {per_load_s} s is beyond the range of the system clock"
        ));
    }
    if !root.join("Cargo.toml").is_file() {
        return Err(format!(
            "{}: no Cargo.toml at the corpus root",
            root.display()
        ));
    }
    let cargo_target_dir = cargo_target_dir.map(harness::absolutize);
    if let Some(dir) = &cargo_target_dir
        && dir.starts_with(root)
    {
        return Err(format!(
            "--cargo-target-dir {} lies under the corpus {}; nothing written",
            dir.display(),
            root.display()
        ));
    }
    Ok(Setup { cargo_target_dir })
}

/// The `result` object: one object per load, `null` for a load not requested.
#[derive(Serialize)]
pub struct RaResult {
    /// The `ra_ap_*` release the harness was built with.
    pub ra_ap: &'static str,
    /// Build profile of the harness: `release`, or `debug` (debug assertions
    /// on; the analysis runs many times slower, so its times are not the
    /// measurement).
    pub profile: &'static str,
    /// Budget of each load (`--timeout`), seconds.
    pub timeout_s: u64,
    pub without: Option<ModeResult>,
    pub with: Option<ModeResult>,
}

/// The rows of the "Results" table for one load.
#[derive(Serialize)]
pub struct ModeResult {
    /// `ok`; `timeout` (budget exceeded, the load's processes killed);
    /// `crashed` (the worker died on its own); `load_failed` (a load step
    /// failed outright, category in `detail.load_error`).
    pub status: &'static str,
    /// Metadata + build scripts (with) + database + first pass.
    pub cold_ms: Field<u64>,
    /// Peak RSS of the analysing process, MiB.
    pub peak_rss_mb: Field<f64>,
    /// The measure of the RSS verdict threshold, MiB: the largest sum of the
    /// resident sizes of every live process of the load's process group —
    /// the worker, cargo, rustc and build scripts, the proc-macro server —
    /// sampled by the harness every `detail.group_rss_sample_ms` from the
    /// worker's start until the load ends (its last report, its stdout
    /// closing, or the budget). Pages shared between processes count once
    /// per process; a spike shorter than the interval can be missed (see
    /// `group_peak_rss_floor_mb`). `"timeout"` when the budget ran out: the
    /// peak of an unfinished load is not known (the floor keeps the peak
    /// sampled until the kill). `null`: no group reading on this system
    /// (only macOS and Linux have one) or not one sample read — never 0.
    pub group_peak_rss_mb: Option<Field<f64>>,
    /// A guaranteed lower bound of the group's true peak, MiB: the largest
    /// of the sampled group peak (on `"timeout"`, the peak sampled until the
    /// kill), `peak_rss_mb` and `detail.children_peak_rss_mb` — each of them
    /// is at most the true peak. A sanity bound on `group_peak_rss_mb`, not
    /// the verdict's measure; `null` when none of the three is known.
    pub group_peak_rss_floor_mb: Option<f64>,
    /// An in-memory edit of one file (a function appended) + a full pass.
    pub warm_ms: Field<u64>,
    /// Items with a (non-local) moniker, percent of `items`.
    pub moniker_pct: Field<f64>,
    pub items: Field<usize>,
    pub items_with_moniker: Field<usize>,
    /// Files whose scan panicked (rust-analyzer crashes the pass survived).
    pub panics: Field<usize>,
    pub detail: ModeDetail,
}

/// A measured value, or the status string of the load that did not reach it.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Field<T> {
    Value(T),
    Status(&'static str),
}

/// Side information of one load: every step the worker reported.
#[derive(Serialize, Default)]
pub struct ModeDetail {
    pub metadata: Option<MetadataStep>,
    pub build_scripts: Option<BuildScriptsStep>,
    pub database: Option<DatabaseStep>,
    pub first_pass: Option<PassStep>,
    /// The edit + a pass over the edited file only.
    pub warm_file_ms: Option<u64>,
    pub children_peak_rss_mb: Option<f64>,
    /// Pause between two samples of the group's resident size
    /// (`group_peak_rss_mb`), ms; the first sample is taken at the worker's
    /// start. `null` where there is no group reading.
    pub group_rss_sample_ms: Option<u64>,
    /// The cargo target directory held no build output before this load:
    /// absent, empty, or only the files cargo writes without building
    /// ([`TARGET_BOOKKEEPING`], which the `without` load leaves), so the
    /// `with` load built its build scripts and proc macros from scratch.
    /// `false`: earlier output was reused and `cold_ms` understates a cold
    /// build. `null`: the directory could not be read.
    pub target_dir_fresh: Option<bool>,
    /// No file had an item with a moniker: no warm step.
    pub no_warm_target: bool,
    /// Category of the load step that failed (`status` = `load_failed`).
    pub load_error: Option<String>,
    /// How the worker ended when not cleanly: `code N` or `signal N` — a
    /// crashed worker, or one killed after it outlived its last report by
    /// the grace period (its reported values stand).
    pub exit: Option<String>,
}

pub fn run(
    corpus: &Corpus,
    setup: Setup,
    modes: Modes,
    per_load_s: u64,
) -> Result<RaResult, String> {
    let base = corpus.out.join("ra").join(&corpus.label);
    let target_dir = setup
        .cargo_target_dir
        .unwrap_or_else(|| base.join("target"));
    let mut result = RaResult {
        ra_ap: specengine_ra::RA_AP_VERSION,
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        timeout_s: per_load_s,
        without: None,
        with: None,
    };
    for &mode in modes.list() {
        let measured = run_load(corpus, &base, &target_dir, mode, per_load_s)?;
        if measured.status == "timeout" {
            eprintln!(
                "ra {}: timeout after {per_load_s} s; every process of the load killed",
                mode.as_str()
            );
        } else {
            eprintln!("ra {}: {}", mode.as_str(), measured.status);
        }
        match mode {
            Mode::With => result.with = Some(measured),
            Mode::Without => result.without = Some(measured),
        }
    }
    Ok(result)
}

/// Runs one worker under the budget and folds its events into a [`ModeResult`].
fn run_load(
    corpus: &Corpus,
    base: &Path,
    target_dir: &Path,
    mode: Mode,
    budget_s: u64,
) -> Result<ModeResult, String> {
    let detail = base.join(mode.as_str());
    let tmp = detail.join("tmp");
    let target_dir_fresh = holds_no_build_output(target_dir);
    fs::create_dir_all(&tmp)
        .map_err(|error| format!("cannot create {}: {error}", tmp.display()))?;
    fs::create_dir_all(target_dir)
        .map_err(|error| format!("cannot create {}: {error}", target_dir.display()))?;
    let exe = std::env::current_exe()
        .map_err(|error| format!("cannot locate this binary for the worker: {error}"))?;
    let mut command = Command::new(exe);
    command
        .arg("ra-worker")
        .arg("--root")
        .arg(&corpus.root)
        .arg("--cargo-target-dir")
        .arg(target_dir)
        .arg("--detail")
        .arg(&detail)
        .arg("--mode")
        .arg(mode.as_str())
        .arg("--budget")
        .arg(budget_s.to_string())
        .env("TMPDIR", &tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0);
    // The corpus is analysed with its own toolchain, not with the one that
    // happened to launch this binary (`cargo +toolchain run` sets these).
    for variable in [
        "RUSTUP_TOOLCHAIN",
        "CARGO",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET_DIR",
    ] {
        command.env_remove(variable);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start the ra worker: {error}"))?;
    let Some(stdout) = child.stdout.take() else {
        sys::kill_group(child.id());
        let _ = child.wait();
        return Err("the ra worker has no stdout pipe".to_owned());
    };
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else {
                break;
            };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    // `prepare` checked that the clock represents every deadline of the run.
    let Some(deadline) = Instant::now().checked_add(Duration::from_secs(budget_s)) else {
        sys::kill_group(child.id());
        let _ = child.wait();
        return Err(format!(
            "--timeout {budget_s} s is beyond the range of the system clock"
        ));
    };
    let sampler = GroupSampler::start(child.id());
    let mut events = Vec::new();
    let mut timed_out = false;
    let mut reported = false;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            timed_out = true;
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(line) => {
                if let Ok(event) = serde_json::from_str::<Event>(&line) {
                    reported = matches!(event, Event::Done(_) | Event::LoadFailed(_));
                    events.push(event);
                    if reported {
                        break;
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                timed_out = true;
                break;
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // The load has ended (reported, its stdout closed, or out of budget):
    // what the group does from here on is not the load's.
    if let Some(sampler) = &sampler {
        sampler.stop();
    }
    // After its last report the worker only exits: that is not the load's
    // work, so the budget no longer applies — a short grace for its stdout
    // to close (the worker and whatever inherited it gone).
    let lingered = reported && {
        let until = Instant::now().checked_add(AFTER_REPORT_GRACE);
        loop {
            let remaining = until.map_or(Duration::ZERO, |until| {
                until.saturating_duration_since(Instant::now())
            });
            if remaining.is_zero() {
                break true;
            }
            match receiver.recv_timeout(remaining) {
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => break true,
                Err(RecvTimeoutError::Disconnected) => break false,
            }
        }
    };
    // The whole group, before reaping: on overrun the worker and everything
    // it started; after a normal exit any straggler (usually none).
    sys::kill_group(child.id());
    let exit = child
        .wait()
        .map_err(|error| format!("cannot reap the ra worker: {error}"))?;
    drop(child.stdin.take());
    join_bounded(reader, READER_JOIN_GRACE);
    let sample_ms = sampler
        .as_ref()
        .map(|_| u64::try_from(GROUP_RSS_SAMPLE.as_millis()).unwrap_or(u64::MAX));
    let group_peak = sampler.and_then(GroupSampler::peak);
    let mut result = fold(events, timed_out, lingered, exit, group_peak);
    result.detail.target_dir_fresh = target_dir_fresh;
    result.detail.group_rss_sample_ms = sample_ms;
    Ok(result)
}

/// Joins `thread` if it ends within `grace`; otherwise leaves it running
/// (detached) and returns.
fn join_bounded(thread: thread::JoinHandle<()>, grace: Duration) {
    let until = Instant::now().checked_add(grace);
    while !thread.is_finished() {
        let remaining = until.map_or(Duration::ZERO, |until| {
            until.saturating_duration_since(Instant::now())
        });
        if remaining.is_zero() {
            return;
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
    let _ = thread.join();
}

/// Samples the summed resident size of the worker's process group on a
/// thread of its own — every [`GROUP_RSS_SAMPLE`], sleeping in between —
/// and keeps the largest sum; the load's own control flow (budget, kill,
/// reap) never waits on it.
struct GroupSampler {
    stop: mpsc::Sender<()>,
    thread: thread::JoinHandle<Option<u64>>,
}

impl GroupSampler {
    /// Starts sampling group `leader` at once; `None` on a system without
    /// a group reading, or when the thread cannot start.
    fn start(leader: u32) -> Option<Self> {
        let mut group = sys::GroupRss::new(leader)?;
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = thread::Builder::new()
            .name("ra-group-rss".to_owned())
            .spawn(move || {
                let mut peak: Option<u64> = None;
                loop {
                    if let Some(bytes) = group.resident_bytes() {
                        peak = Some(peak.map_or(bytes, |peak| peak.max(bytes)));
                    }
                    match stopped.recv_timeout(GROUP_RSS_SAMPLE) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break peak,
                    }
                }
            })
            .ok()?;
        Some(Self { stop, thread })
    }

    /// No sample after the one in progress, if any.
    fn stop(&self) {
        let _ = self.stop.send(());
    }

    /// The largest sum sampled, bytes; `None` when not one sample was read.
    fn peak(self) -> Option<u64> {
        drop(self.stop);
        self.thread.join().ok().flatten()
    }
}

/// What cargo writes to a target directory without building anything
/// (`cargo metadata` and the toolchain queries of a load).
const TARGET_BOOKKEEPING: [&str; 2] = [".rustc_info.json", "CACHEDIR.TAG"];

/// `Some(true)` when `dir` does not exist or holds nothing but
/// [`TARGET_BOOKKEEPING`], `Some(false)` when it holds anything else, `None`
/// when it cannot be read.
fn holds_no_build_output(dir: &Path) -> Option<bool> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Some(true),
        Err(_) => return None,
    };
    for entry in entries {
        let name = entry.ok()?.file_name();
        if !TARGET_BOOKKEEPING.iter().any(|kept| name == *kept) {
            return Some(false);
        }
    }
    Some(true)
}

/// Folds the worker's events. `lingered`: the worker sent its last report
/// but was still running (or its stdout still open) after the grace period.
/// `group_peak`: the largest sampled resident size of the group, bytes.
fn fold(
    events: Vec<Event>,
    timed_out: bool,
    lingered: bool,
    exit: ExitStatus,
    group_peak: Option<u64>,
) -> ModeResult {
    let mut detail = ModeDetail::default();
    let mut warm_ms = None;
    let mut peak_rss_mb = None;
    let mut done = false;
    for event in events {
        match event {
            Event::Metadata(step) => detail.metadata = Some(step),
            Event::BuildScripts(step) => detail.build_scripts = Some(step),
            Event::Database(step) => detail.database = Some(step),
            Event::FirstPass(step) => detail.first_pass = Some(step),
            Event::WarmFile(step) => detail.warm_file_ms = Some(step.ms),
            Event::Warm(step) => warm_ms = Some(step.ms),
            Event::Done(step) => {
                done = true;
                peak_rss_mb = step.peak_rss_mb;
                detail.children_peak_rss_mb = step.children_peak_rss_mb;
                detail.no_warm_target = step.no_warm_target;
            }
            Event::LoadFailed(step) => detail.load_error = Some(step.category),
        }
    }
    let status = if timed_out {
        "timeout"
    } else if detail.load_error.is_some() {
        "load_failed"
    } else if done && (exit.success() || lingered) {
        "ok"
    } else {
        "crashed"
    };
    if !timed_out && !exit.success() {
        detail.exit = Some(describe(exit));
    }
    let cold_ms = cold(&detail);
    let counts: Option<&PassCounts> = detail.first_pass.as_ref().map(|pass| &pass.counts);
    let items = counts.map(|counts| counts.items);
    let with_moniker = counts.map(|counts| counts.by_status.get("moniker").copied().unwrap_or(0));
    let moniker_pct = items
        .zip(with_moniker)
        .map(|(items, with)| harness::percent(with, items));
    let panics = counts.map(|counts| counts.files_panicked);
    let warm = match (warm_ms, done && detail.no_warm_target) {
        (Some(ms), _) => Field::Value(ms),
        (None, true) => Field::Status("no_warm_target"),
        (None, false) => Field::Status(status),
    };
    let group_mb = group_peak.map(sys::mib);
    let floor = [group_mb, peak_rss_mb, detail.children_peak_rss_mb]
        .into_iter()
        .flatten()
        .reduce(f64::max);
    ModeResult {
        status,
        cold_ms: reached(cold_ms, status),
        peak_rss_mb: match (done, peak_rss_mb) {
            (true, Some(mb)) => Field::Value(mb),
            (true, None) => Field::Status("unknown"),
            (false, _) => Field::Status(status),
        },
        group_peak_rss_mb: group_mb.map(|mb| {
            if timed_out {
                Field::Status(status)
            } else {
                Field::Value(mb)
            }
        }),
        group_peak_rss_floor_mb: floor,
        warm_ms: warm,
        moniker_pct: reached(moniker_pct, status),
        items: reached(items, status),
        items_with_moniker: reached(with_moniker, status),
        panics: reached(panics, status),
        detail,
    }
}

/// The value, or the status of the load that never reported it.
fn reached<T>(value: Option<T>, status: &'static str) -> Field<T> {
    value.map_or(Field::Status(status), Field::Value)
}

/// Sum of the load steps and the first pass, once all of them finished.
fn cold(detail: &ModeDetail) -> Option<u64> {
    let metadata = detail.metadata.as_ref()?.ms;
    let build_scripts = detail.build_scripts.as_ref().map_or(0, |step| step.ms);
    let database = detail.database.as_ref()?.ms;
    let first_pass = detail.first_pass.as_ref()?.ms;
    Some(
        metadata
            .saturating_add(build_scripts)
            .saturating_add(database)
            .saturating_add(first_pass),
    )
}

fn describe(exit: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    match (exit.code(), exit.signal()) {
        (Some(code), _) => format!("code {code}"),
        (None, Some(signal)) => format!("signal {signal}"),
        (None, None) => "unknown".to_owned(),
    }
}
