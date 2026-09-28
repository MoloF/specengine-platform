//! The hidden `ra-worker` subcommand: one load of one mode in its own
//! process (and process group), so its peak RSS is its own and a budget
//! overrun can kill it together with every cargo, rustc and proc-macro
//! server process it started.
//!
//! Self-terminating on two watchdogs: stdin reaching EOF (the parent is gone)
//! and the budget plus a grace period (the parent failed to act). Either
//! SIGKILLs the worker's own process group.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use clap::Args;
use serde::Serialize;
use specengine_ra::{
    FileScan, ItemKind, LoadOptions, MonikerStatus, Project, Workspace, scan_unloaded,
};

use super::Mode;
use super::events::{
    BuildScriptsStep, DatabaseStep, DoneStep, Event, KindCounts, LoadFailedStep, MetadataStep,
    PassCounts, PassStep, TimedStep,
};
use super::sys;
use crate::harness;

/// rust-analyzer's own threads run on 16 MiB stacks; the worker gives its
/// analysis thread more headroom (reserved, not resident).
const STACK_BYTES: usize = 64 * 1024 * 1024;

/// Past the budget the parent kills the worker; this much later the worker
/// kills itself.
const GRACE: Duration = Duration::from_secs(10);

/// The in-memory edits of the warm step (appended to one file, never written).
const WARM_EDIT_FILE: &str = "\n\nfn specengine_eval_warm_probe_file() {}\n";
const WARM_EDIT_FULL: &str = "\n\nfn specengine_eval_warm_probe_full() {}\n";

#[derive(Args, Clone)]
pub struct WorkerArgs {
    /// Canonical corpus root.
    #[arg(long, value_name = "DIR")]
    pub root: PathBuf,
    #[arg(long, value_name = "DIR")]
    pub cargo_target_dir: PathBuf,
    /// Directory of this mode's detail files.
    #[arg(long, value_name = "DIR")]
    pub detail: PathBuf,
    #[arg(long, value_enum)]
    pub mode: Mode,
    /// Budget in seconds (the parent enforces it; this is the backstop).
    #[arg(long, value_name = "SECONDS")]
    pub budget: u64,
}

pub fn main(args: WorkerArgs) -> ExitCode {
    let root = match fs::canonicalize(&args.root) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("ra-worker: refused: corpus unreadable: {error}");
            return ExitCode::from(2);
        }
    };
    for (flag, path) in [
        ("--cargo-target-dir", &args.cargo_target_dir),
        ("--detail", &args.detail),
    ] {
        if harness::absolutize(path).starts_with(&root) {
            eprintln!("ra-worker: refused: {flag} lies under the corpus; nothing written");
            return ExitCode::from(2);
        }
    }
    start_watchdogs(Duration::from_secs(args.budget));
    let work = thread::Builder::new()
        .name("ra-worker".to_owned())
        .stack_size(STACK_BYTES)
        .spawn(move || run(&args, &root));
    match work.map(thread::JoinHandle::join) {
        Ok(Ok(Ok(()))) => ExitCode::SUCCESS,
        Ok(Ok(Err(message))) => {
            eprintln!("ra-worker: internal failure: {message}");
            ExitCode::from(1)
        }
        Ok(Err(_)) => {
            eprintln!("ra-worker: internal failure: the analysis thread panicked");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("ra-worker: internal failure: cannot start the analysis thread: {error}");
            ExitCode::from(1)
        }
    }
}

fn start_watchdogs(budget: Duration) {
    thread::spawn(|| {
        let mut stdin = io::stdin();
        let mut sink = [0_u8; 64];
        loop {
            match stdin.read(&mut sink) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        terminate("stdin closed");
    });
    thread::spawn(move || {
        thread::sleep(budget.saturating_add(GRACE));
        terminate("budget exceeded");
    });
}

/// One write, so a line racing the parent's SIGKILL appears whole or not at all.
fn terminate(reason: &str) -> ! {
    let line = format!("ra-worker: {reason}; stopping every process of this load\n");
    let _ = io::stderr().write_all(line.as_bytes());
    sys::kill_own_group();
    std::process::exit(3)
}

/// Load-step error texts; they may name paths, so they go to `errors.json` only.
#[derive(Serialize, Default)]
struct Errors {
    load: Option<String>,
    metadata: Option<String>,
    sysroot: Option<String>,
    build_scripts: Option<String>,
}

/// One item line of `items.json`.
#[derive(Serialize)]
struct ItemDetail<'a> {
    path: &'a str,
    line: usize,
    kind: &'static str,
    name: Option<&'a str>,
    status: &'static str,
    moniker: Option<&'a str>,
}

fn run(args: &WorkerArgs, root: &Path) -> Result<(), String> {
    fs::create_dir_all(&args.detail)
        .map_err(|error| format!("cannot create {}: {error}", args.detail.display()))?;
    let with = args.mode == Mode::With;
    let label = args.mode.as_str();
    let options = LoadOptions {
        root: root.to_path_buf(),
        target_dir: harness::absolutize(&args.cargo_target_dir),
        proc_macros: with,
    };
    let mut errors = Errors::default();

    let started = Instant::now();
    let mut project = match Project::discover(&options) {
        Ok(project) => project,
        Err(error) => return load_failed(args, errors, &error),
    };
    let summary = project.summary();
    emit(&Event::Metadata(MetadataStep {
        ms: millis(started),
        toolchain: summary.toolchain.clone(),
        packages: summary.packages,
        metadata_degraded: summary.metadata_error.is_some(),
        sysroot_packages: summary.sysroot_packages,
        sysroot_error: summary.sysroot_error.is_some(),
    }))?;
    eprintln!("ra {label}: metadata in {:.1} s", seconds(started));
    errors.metadata = summary.metadata_error;
    errors.sysroot = summary.sysroot_error;

    if with {
        let started = Instant::now();
        let scripts = match project.run_build_scripts() {
            Ok(scripts) => scripts,
            Err(error) => return load_failed(args, errors, &error),
        };
        emit(&Event::BuildScripts(BuildScriptsStep {
            ms: millis(started),
            ran: scripts.ran,
            errors: scripts.error.is_some(),
        }))?;
        eprintln!("ra {label}: build scripts in {:.1} s", seconds(started));
        errors.build_scripts = scripts.error;
    }

    let started = Instant::now();
    let mut workspace = match project.into_workspace() {
        Ok(workspace) => workspace,
        Err(error) => return load_failed(args, errors, &error),
    };
    let ms = millis(started);
    let proc_macro_crates = workspace.proc_macro_crates();
    emit(&Event::Database(DatabaseStep {
        ms,
        proc_macro_server: workspace.proc_macro_server().as_str().to_owned(),
        proc_macro_crates_loaded: proc_macro_crates.loaded,
        proc_macro_crates_not_loaded: proc_macro_crates.not_loaded,
        peak_rss_mb: sys::peak_rss_bytes().map(sys::mib),
    }))?;
    eprintln!("ra {label}: database in {:.1} s", seconds(started));
    write_json(&args.detail.join("errors.json"), &errors)?;

    let files = harness::rust_files(root)
        .map_err(|error| format!("cannot list the corpus's .rs files: {error}"))?;
    let started = Instant::now();
    let first = scan_all(&workspace, root, &files);
    emit(&Event::FirstPass(PassStep {
        ms: millis(started),
        counts: first.counts.clone(),
    }))?;
    eprintln!(
        "ra {label}: first pass in {:.1} s ({} items)",
        seconds(started),
        first.counts.items
    );
    write_items(&args.detail.join("items.json"), &first)?;

    let target = warm_target(&workspace, &first);
    if let Some(path) = &target
        && let Some(file) = workspace.file(path)
    {
        let started = Instant::now();
        workspace.append(file, WARM_EDIT_FILE)?;
        let _ = catch(|| workspace.scan(file));
        emit(&Event::WarmFile(TimedStep {
            ms: millis(started),
        }))?;
        let started = Instant::now();
        workspace.append(file, WARM_EDIT_FULL)?;
        let _ = scan_all(&workspace, root, &files);
        emit(&Event::Warm(TimedStep {
            ms: millis(started),
        }))?;
        eprintln!("ra {label}: warm pass in {:.1} s", seconds(started));
        write_json(
            &args.detail.join("warm.json"),
            &BTreeMap::from([("edited_file", relative_string(path))]),
        )?;
    }

    // The database and the client both hold the proc-macro server: only once
    // both are dropped is it killed and reaped, so its peak RSS reaches
    // `RUSAGE_CHILDREN` below.
    workspace.close();
    emit(&Event::Done(DoneStep {
        peak_rss_mb: sys::peak_rss_bytes().map(sys::mib),
        children_peak_rss_mb: sys::children_peak_rss_bytes().map(sys::mib),
        no_warm_target: target.is_none(),
    }))
}

fn load_failed(
    args: &WorkerArgs,
    mut errors: Errors,
    error: &specengine_ra::LoadError,
) -> Result<(), String> {
    eprintln!(
        "ra {}: load failed: {}",
        args.mode.as_str(),
        error.category()
    );
    errors.load = Some(error.to_string());
    write_json(&args.detail.join("errors.json"), &errors)?;
    emit(&Event::LoadFailed(LoadFailedStep {
        category: error.category().to_owned(),
    }))
}

/// A full pass: every item of every file, with its status.
struct Pass {
    files: Vec<(PathBuf, FileScan)>,
    counts: PassCounts,
}

fn scan_all(workspace: &Workspace, root: &Path, files: &[PathBuf]) -> Pass {
    let mut scanned = Vec::with_capacity(files.len());
    let mut counts = PassCounts {
        files: files.len(),
        ..PassCounts::default()
    };
    for path in files {
        let scan = match workspace.file(path) {
            Some(file) => {
                counts.files_loaded += 1;
                match catch(|| workspace.scan(file)) {
                    Some(scan) => scan,
                    None => {
                        counts.files_panicked += 1;
                        continue;
                    }
                }
            }
            None => match fs::read_to_string(root.join(path)) {
                Ok(text) => scan_unloaded(&text),
                Err(_) => {
                    counts.files_unreadable += 1;
                    continue;
                }
            },
        };
        if scan.in_crate {
            counts.files_in_crate += 1;
        }
        scanned.push((path.clone(), scan));
    }
    let mut monikers: HashMap<&str, usize> = HashMap::new();
    for (_, scan) in &scanned {
        for item in &scan.items {
            if let MonikerStatus::Moniker(moniker) = &item.status {
                *monikers.entry(moniker.as_str()).or_default() += 1;
            }
        }
    }
    for kind in ItemKind::ALL {
        counts
            .by_kind
            .insert(kind.as_str().to_owned(), KindCounts::default());
    }
    for (_, scan) in &scanned {
        for item in &scan.items {
            counts.items += 1;
            *counts
                .by_status
                .entry(item.status.as_str().to_owned())
                .or_default() += 1;
            let kind = counts
                .by_kind
                .entry(item.kind.as_str().to_owned())
                .or_default();
            kind.items += 1;
            if let MonikerStatus::Moniker(moniker) = &item.status {
                kind.moniker += 1;
                if monikers.get(moniker.as_str()).copied().unwrap_or(0) > 1 {
                    kind.duplicate += 1;
                    counts.duplicate_moniker_items += 1;
                }
            }
        }
    }
    counts.by_kind.retain(|_, kind| kind.items > 0);
    Pass {
        files: scanned,
        counts,
    }
}

/// The loaded file with the most items that have a moniker (the first in
/// path order on a tie): a central module of some crate, so an edit there
/// invalidates real analysis. `None` when no item has a moniker.
fn warm_target(workspace: &Workspace, pass: &Pass) -> Option<PathBuf> {
    let mut best: Option<(&PathBuf, usize)> = None;
    for (path, scan) in &pass.files {
        let monikers = scan
            .items
            .iter()
            .filter(|item| matches!(item.status, MonikerStatus::Moniker(_)))
            .count();
        let better = best.is_none_or(|(_, most)| monikers > most);
        if monikers > 0 && better && workspace.file(path).is_some() {
            best = Some((path, monikers));
        }
    }
    best.map(|(path, _)| path.clone())
}

/// Runs a rust-analyzer query; a panic (a crash of the analyser on this
/// input) becomes `None` and is counted, the pass goes on.
fn catch<T>(query: impl FnOnce() -> T) -> Option<T> {
    panic::catch_unwind(AssertUnwindSafe(query)).ok()
}

fn emit(event: &Event) -> Result<(), String> {
    let line = serde_json::to_string(event).map_err(|error| format!("cannot encode: {error}"))?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("cannot report to the parent: {error}"))
}

fn write_items(path: &Path, pass: &Pass) -> Result<(), String> {
    let paths: Vec<String> = pass
        .files
        .iter()
        .map(|(path, _)| relative_string(path))
        .collect();
    let mut items = Vec::new();
    for ((_, scan), file) in pass.files.iter().zip(&paths) {
        for item in &scan.items {
            items.push(ItemDetail {
                path: file,
                line: item.line,
                kind: item.kind.as_str(),
                name: item.name.as_deref(),
                status: item.status.as_str(),
                moniker: match &item.status {
                    MonikerStatus::Moniker(moniker) => Some(moniker),
                    _ => None,
                },
            });
        }
    }
    write_json(path, &items)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, json).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn seconds(started: Instant) -> f64 {
    started.elapsed().as_secs_f64()
}
