//! docs/features/write-memory.md (RM-0132) WM1-WM4: the write soak, the
//! owner's stability rule over the hub daemon's write path.
//!
//! - **The hub** (`library_common`, as DL14/DL15 and GU16, so the numbers
//!   compare): DC-11's tree at `--nodes 10000` with its 10 000 seeded
//!   commits, the release `specengine-http` over it with `--public-origin`,
//!   `--port 0`, `--slow-ms 0`, a scratch `HOME`, under `perl -e 'alarm N'`
//!   (it dies by SIGALRM even if this process is killed; killed and reaped
//!   on drop). Each run a fresh daemon and `HOME`, so a fresh queue.
//! - **The variants** (`WRITE_SOAK_VARIANT`, default `W`): `W` 8 logins
//!   call `ask_question` over `/mcp/<slug>` back to back, each text fresh
//!   syllables (no hit, every call stored), no readers; `U` 8 logins call
//!   `propose_change` `update` back to back on one of [`U_TARGETS`] ID
//!   sections, its text the section's own plus a fresh sentence against its
//!   `span_hash` (each a new check-memo key; nothing is applied: no
//!   `[auto]`); `R` 10 readers cycle `get_node`, `search` and `GET
//!   /api/projects/<slug>/inbox`; `M` W's writers and R's readers together;
//!   `H` 8 logins ask one question, the same text on the same node, so all
//!   but the first call hit (reported, not judged); `CM` (docs/features/
//!   comments.md CM19) 8 logins call `add_comment` back to back on one of
//!   [`CM_CARDS`] cards, each body fresh syllables mentioning one of the
//!   [`CM_PEOPLE`] logins, beside [`READERS`] readers, those logins, each
//!   cycling `get_card {context: true}`, `GET .../comments?on=` and `GET
//!   .../mentions`; the queue first holds [`CM_PREFILL`] comments by those
//!   logins on those cards ([`comments_prefill`], a template `HOME` copied
//!   per run), judged as W; `RF`
//!   (docs/features/work-references.md "Soak RF", RF23) CM's writers and
//!   pre-fill, beside [`RF_FILES`] WAV files of
//!   [`RF_FILE_BYTES`] stored and attached to the cards (the template
//!   `HOME` holds their blob store), and from [`RF_BASELINE`] on
//!   [`RF_UPLOADERS`] uploaders (logins `uploader-N`) each cycling a fresh
//!   [`RF_UPLOAD_BYTES`] WAV `PUT` at [`RF_UPLOAD_MIBS`] (a valid header,
//!   counter bytes generated as sent, never held whole), its `POST
//!   .../references` with "what matters" and its `DELETE`, and [`READERS`]
//!   Range readers (logins `reader-N`) each reading [`RF_RANGES_PER_S`]
//!   random [`RF_WINDOW`] windows of the stored files a second, every
//!   answer's bytes checked (the rates a hub sees over the network; an
//!   override is a diagnosis, [`rf_knob`]); a page reader (`page-0`) reads
//!   `GET .../cards` and `GET .../comments?on=` every second from the start,
//!   its p95 after [`RF_BASELINE`] at most 1.5 x its p95 before + 50 ms;
//!   at the end the store holds exactly the stored files and no `.part`; a
//!   run refuses to start with less than [`RF_FREE_MIN`] free, naming it.
//! - **Memory** sampled every [`SAMPLE_EVERY`]: the resident set and, on
//!   macOS, `phys_footprint` (`proc_pid_rusage`, what `footprint` reports).
//! - **The quiet probe** (macOS): every [`QUIET_EVERY`] the clients are
//!   held, every call answers, after [`QUIET_SETTLE`] `vmmap --summary`
//!   reads the malloc zones' allocated bytes: the live heap between calls,
//!   without a call's working set or the pages the allocator keeps.
//! - **Judged** ([`judge`]): the quiet probe's live heap on macOS, the
//!   resident set elsewhere (`judged_on` `live_heap` or `rss`): the
//!   Theil-Sen slope (the median of all pairwise slopes) of its readings
//!   from `SOAK_SECS/3` on, against [`WRITE_BUDGET_MIB_PER_MIN`] (W, U, M)
//!   or [`READ_BUDGET_MIB_PER_MIN`] (R); fewer than [`MIN_QUIET`] judged
//!   quiet readings (or [`MIN_JUDGED_SAMPLES`] RSS samples) break the run; a
//!   breach fails the test (non-zero exit). Reported beside it: the RSS and
//!   footprint slopes, each whole, judged and by third (a plateau falls
//!   third by third, a leak stays positive in each), KiB kept per write.
//!   Why the probe (RM-0132's diagnosis): after each write's step libmalloc
//!   keeps the freed pages resident, so RSS judged every clean W run red;
//!   the footprint falls for minutes as those pages go back, which hid a
//!   2 KiB per write leak in 4 of 5 runs; the probe read that leak as
//!   2.16 MiB/min and a clean run as 0.0.
//! - **Judged beside it on macOS** ([`judge_footprint`], RM-0134 MX6): the
//!   footprint at the quiet probe's moments, memory malloc does not count
//!   (the allocator's kept pages, which the live heap never sees): its
//!   Theil-Sen slope from `SOAK_SECS/3` on against
//!   [`FOOTPRINT_BUDGET_MIB_PER_MIN`], every judged variant (W, U, R, M);
//!   fewer than [`MIN_QUIET`] such readings break the run. RM-0134's
//!   diagnosis: clean runs read at most 0 there, M over 2 000 pending 25.0
//!   and 12.5, WM8's M 73.
//! - **Every call answers** (200, a JSON-RPC response, no `isError`; a W or
//!   U write stored, an H repeat a hit, an R read with its document); a
//!   request open
//!   [`HANG`] runs `sample <pid> 5` on macOS into a kept temp file, prints
//!   its path and fails the run.
//! - **The report**, one stdout JSON line a run (WM3): variant, seconds,
//!   calls per minute, the client's `ms` spread, the first, middle and last
//!   samples, the judged slope and its budget, both slopes whole/judged/by
//!   third, KiB per write, threads, open fds and child processes at start,
//!   middle and end, the daemon binary's path, size and mtime, the write
//!   lines' cost fields.
//! - **A stale binary is refused** (WM4) before anything is seeded: each
//!   release binary is held to its own sources ([`stale_binaries`]), a
//!   `spec` or `specengine-http` older than a file under `crates/<c>/src`
//!   of a workspace crate in its own path-dependency closure, than
//!   `Cargo.lock`, or (the daemon) than `ui/dist` names the build command.
//!   An http-only change refuses the daemon alone: cargo does not relink
//!   `spec` after it.
//!
//! RM-0134's knobs (the mixed-load resident step): `WRITE_SOAK_READS`, a
//! comma list of `node`, `search` and `inbox` (default all three), the
//! reads each reader cycles; `WRITE_SOAK_PENDING=N` fills a template `HOME`
//! once with `N` stored questions through a hub of its own (then stopped),
//! and each run starts its fresh daemon on a copy of it. Each run reports
//! the pending count beside RSS and the footprint every
//! [`TIMELINE_EVERY`] (`timeline`) and each inbox read's response bytes
//! (`inbox`).
//!
//! Settings: `SOAK_SECS` (default and least 300); `WRITE_SOAK_RUNS`
//! (default 2); `WRITE_SOAK_SMOKE=1` lets `SOAK_SECS` go below 300 and
//! judges no memory (a harness check, never evidence);
//! `WRITE_SOAK_HEAP=1` starts the daemon with `MallocStackLogging=lite` and
//! keeps `leaks --outputGraph` memgraphs and `vmmap --summary` at 60 s and
//! at the end in the temp directory, for `heap --diffFrom` and
//! `malloc_history` (macOS; memory reported, not judged: the stack log
//! weighs on it).
//!
//! Run: `cargo build --release -p specengine-cli -p specengine-http
//! --bins`, then `WRITE_SOAK_VARIANT=W cargo nextest run -p specengine-eval
//! --release --test write_soak --run-ignored only --no-capture`. One soak at
//! a time; about `WRITE_SOAK_RUNS` × (`SOAK_SECS` + 20 s) plus the seeding
//! of the tree (about two minutes).

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;
mod library_common;
#[path = "../../specengine-cli/tests/work_refs_common/mod.rs"]
mod refs_common;

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use library_common::{
    COMMITS, Hub, InFlight, Tree, mcp_call, nonce_words, page_get, spread, theil_sen_mib_per_minute,
};
use serde_json::{Value, json};

const NODES: usize = 10_000;
/// The least soak (and the default).
const MIN_SOAK_SECS: u64 = 300;
/// WM6: the write soaks' memory budget (W, U, M), MiB per minute: the
/// Theil-Sen slope from `SOAK_SECS/3` on. At about 500 writes a minute, at
/// most about 0.5 KiB kept per write.
const WRITE_BUDGET_MIB_PER_MIN: f64 = 0.25;
/// The readers' budget (R), DL14's and DP17's, MiB per minute.
const READ_BUDGET_MIB_PER_MIN: f64 = 0.5;
/// RM-0134 MX6: the second bar on macOS, MiB per minute: the footprint at
/// the quiet probe's moments, its Theil-Sen slope from `SOAK_SECS/3` on,
/// every judged variant (W, U, R, M).
const FOOTPRINT_BUDGET_MIB_PER_MIN: f64 = 5.0;
/// A request open this long is a hang.
const HANG: Duration = Duration::from_secs(30);
/// The client's own give-up, past the hang's.
const TIMEOUT: Duration = Duration::from_secs(120);
const WRITERS: usize = 8;
const READERS: usize = 10;
const SAMPLE_EVERY: Duration = Duration::from_secs(1);
/// The ID sections U updates (and R reads), spread over the part files.
const U_TARGETS: usize = 200;
/// The words R searches for.
const SEARCH_WORDS: usize = 64;
/// CM19: the cards the comments go under.
const CM_CARDS: usize = 20;
/// CM19: the people who wrote the prefilled comments and read.
const CM_PEOPLE: usize = 10;
/// CM19: the comments the queue holds before each CM run.
const CM_PREFILL: usize = 2_000;
/// RF23 ("Soak RF"): the files stored and attached before each RF run.
const RF_FILES: usize = 20;
/// RF23: each stored file's bytes.
const RF_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// RF23: each upload's bytes.
const RF_UPLOAD_BYTES: u64 = 200 * 1024 * 1024;
/// RF23: the uploaders.
const RF_UPLOADERS: usize = 2;
/// RF23: a Range reader's window.
const RF_WINDOW: u64 = 1024 * 1024;
/// RF23: each uploader's pace, MiB a second (RF19's): uploads reach a hub
/// over the network through Traefik. Unthrottled on one laptop the
/// harness itself saturated it (load 11-16) and the page bar read the
/// machine, not the daemon.
const RF_UPLOAD_MIBS: f64 = 25.0;
/// RF23: each Range reader's pace, windows a second.
const RF_RANGES_PER_S: f64 = 10.0;
/// RF23: the first phase, CM's writers and the page reader alone.
const RF_BASELINE: Duration = Duration::from_secs(60);
/// RF23: a run refuses to start with less free space than this.
const RF_FREE_MIN: u64 = 6 * 1024 * 1024 * 1024;
/// When heap mode takes its first snapshot, seconds into the run.
const HEAP_FIRST_AT: f64 = 60.0;
/// How often the quiet probe reads the live heap (macOS): the clients held,
/// their calls drained, `vmmap --summary` suspends the daemon for its scan.
const QUIET_EVERY: Duration = Duration::from_secs(30);
/// The wait after the last call's answer, before the reading: a stored
/// update's delegated apply and the request lines finish in the daemon.
const QUIET_SETTLE: Duration = Duration::from_secs(1);
/// The least quiet readings a judged run takes from `SOAK_SECS/3` on
/// (macOS): a 300 s run gives 6.
const MIN_QUIET: usize = 5;
/// The least RSS samples a judged run takes from `SOAK_SECS/3` on (off
/// macOS): a 300 s run gives about 200.
const MIN_JUDGED_SAMPLES: usize = 60;
/// The release binaries a run uses and the package each is built from.
const BINARIES: [(&str, &str); 2] = [
    ("spec", "specengine-cli"),
    ("specengine-http", "specengine-http"),
];
/// How often a run's `timeline` takes RSS, the footprint and the pending
/// count (RM-0134).
const TIMELINE_EVERY: f64 = 10.0;
/// The release build WM4 names.
const BUILD: &str = "cargo build --release -p specengine-cli -p specengine-http --bins";

// ------------------------------------------------------------ settings

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    W,
    U,
    R,
    M,
    H,
    CM,
    RF,
}

impl Variant {
    fn from_env() -> Self {
        let value = std::env::var("WRITE_SOAK_VARIANT").unwrap_or_else(|_| "W".to_owned());
        match value.as_str() {
            "W" => Self::W,
            "U" => Self::U,
            "R" => Self::R,
            "M" => Self::M,
            "H" => Self::H,
            "CM" => Self::CM,
            "RF" => Self::RF,
            other => panic!("WRITE_SOAK_VARIANT={other:?}: one of W, U, R, M, H, CM, RF"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::W => "W",
            Self::U => "U",
            Self::R => "R",
            Self::M => "M",
            Self::H => "H",
            Self::CM => "CM",
            Self::RF => "RF",
        }
    }

    fn writers(self) -> usize {
        if self == Self::R { 0 } else { WRITERS }
    }

    fn readers(self) -> usize {
        if matches!(self, Self::R | Self::M | Self::CM | Self::RF) {
            READERS
        } else {
            0
        }
    }

    /// The budget it is judged against; H is reported, not judged.
    fn budget(self) -> Option<f64> {
        match self {
            Self::W | Self::U | Self::M | Self::CM | Self::RF => Some(WRITE_BUDGET_MIB_PER_MIN),
            Self::R => Some(READ_BUDGET_MIB_PER_MIN),
            Self::H => None,
        }
    }

    /// The footprint's bar at the quiet moments (macOS, RM-0134 MX6): every
    /// variant [`Self::budget`] judges.
    fn footprint_budget(self) -> Option<f64> {
        self.budget().map(|_| FOOTPRINT_BUDGET_MIB_PER_MIN)
    }
}

/// The reads a reader cycles (RM-0134), in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reads {
    node: bool,
    search: bool,
    inbox: bool,
}

impl Reads {
    const ALL: Self = Self {
        node: true,
        search: true,
        inbox: true,
    };

    /// `WRITE_SOAK_READS`'s comma list (`node`, `search`, `inbox`); unset
    /// or empty, all three.
    fn parse(text: Option<&str>) -> Result<Self, String> {
        let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
            return Ok(Self::ALL);
        };
        let mut reads = Self {
            node: false,
            search: false,
            inbox: false,
        };
        for word in text.split(',').map(str::trim) {
            match word {
                "node" => reads.node = true,
                "search" => reads.search = true,
                "inbox" => reads.inbox = true,
                other => {
                    return Err(format!(
                        "WRITE_SOAK_READS: {other:?} is none of node, search, inbox"
                    ));
                }
            }
        }
        Ok(reads)
    }

    fn names(self) -> Vec<&'static str> {
        [
            (self.node, "node"),
            (self.search, "search"),
            (self.inbox, "inbox"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect()
    }
}

#[derive(Debug, Clone, Copy)]
struct Settings {
    variant: Variant,
    secs: u64,
    runs: usize,
    smoke: bool,
    heap: bool,
    /// The reads each reader cycles (RM-0134).
    reads: Reads,
    /// Questions stored before each run, in a copied template `HOME`
    /// (RM-0134; 0: none).
    pending: usize,
}

fn flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value == "1")
}

impl Settings {
    fn from_env() -> Self {
        let smoke = flag("WRITE_SOAK_SMOKE");
        let secs = std::env::var("SOAK_SECS")
            .ok()
            .map_or(MIN_SOAK_SECS, |value| {
                value
                    .parse()
                    .unwrap_or_else(|_| panic!("SOAK_SECS={value:?}: seconds"))
            });
        assert!(
            smoke || secs >= MIN_SOAK_SECS,
            "SOAK_SECS={secs}: the write soak runs {MIN_SOAK_SECS} s at least \
             (WRITE_SOAK_SMOKE=1 for a harness check that judges no memory)"
        );
        let runs = std::env::var("WRITE_SOAK_RUNS").ok().map_or(2, |value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("WRITE_SOAK_RUNS={value:?}: a count"))
        });
        assert!((1..=5).contains(&runs), "WRITE_SOAK_RUNS={runs}: 1 to 5");
        let reads = Reads::parse(std::env::var("WRITE_SOAK_READS").ok().as_deref())
            .unwrap_or_else(|error| panic!("{error}"));
        let pending = std::env::var("WRITE_SOAK_PENDING").ok().map_or(0, |value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("WRITE_SOAK_PENDING={value:?}: a count"))
        });
        Self {
            variant: Variant::from_env(),
            secs,
            runs,
            smoke,
            heap: flag("WRITE_SOAK_HEAP"),
            reads,
            pending,
        }
    }

    /// Whether memory is judged: not in a smoke run, not under the stack
    /// log, not for H.
    fn judged(&self) -> bool {
        !self.smoke && !self.heap && self.variant.budget().is_some()
    }
}

// ------------------------------------------------------------ WM4

/// The newest file under `dir` (recursively), into `best`.
fn newest(dir: &Path, best: &mut Option<(SystemTime, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            newest(&path, best);
        } else {
            consider(&path, best);
        }
    }
}

fn consider(path: &Path, best: &mut Option<(SystemTime, PathBuf)>) {
    if let Ok(at) = fs::metadata(path).and_then(|meta| meta.modified())
        && best.as_ref().is_none_or(|(known, _)| at > *known)
    {
        *best = Some((at, path.to_path_buf()));
    }
}

/// The workspace crates `package` is built from: itself and, recursively,
/// every `path` crate of its `[dependencies]` and `[build-dependencies]`
/// (optional ones too), by directory name under `crates/`.
fn crate_closure(crates: &Path, package: &str) -> std::collections::BTreeSet<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut todo = vec![package.to_owned()];
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let manifest = crates.join(&name).join("Cargo.toml");
        let text = fs::read_to_string(&manifest)
            .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
        let table: toml::Table = toml::from_str(&text).expect("a manifest");
        for section in ["dependencies", "build-dependencies"] {
            let Some(deps) = table.get(section).and_then(toml::Value::as_table) else {
                continue;
            };
            for spec in deps.values() {
                if let Some(path) = spec.get("path").and_then(toml::Value::as_str)
                    && let Some(dir) = Path::new(path).file_name().and_then(|n| n.to_str())
                {
                    todo.push(dir.to_owned());
                }
            }
        }
    }
    seen
}

/// WM4: the refusals of the release binaries under `release`, each held to
/// its own sources under the workspace `root`: a file under `crates/<c>/src`
/// of each crate in its package's path-dependency closure, `Cargo.lock`,
/// and (the daemon, which embeds it) `ui/dist`. A binary older than the
/// newest of them, or missing, is refused naming [`BUILD`]. Cargo relinks
/// no binary a change does not reach, so one rule over every crate for
/// both would refuse `spec` forever after an http-only change.
fn stale_binaries(root: &Path, release: &Path) -> Vec<String> {
    let crates = root.join("crates");
    let mut refusals = Vec::new();
    for (name, package) in BINARIES {
        let mut best = None;
        for member in crate_closure(&crates, package) {
            newest(&crates.join(member).join("src"), &mut best);
        }
        consider(&root.join("Cargo.lock"), &mut best);
        if package == "specengine-http" {
            newest(&root.join("ui/dist"), &mut best);
        }
        let path = release.join(name);
        let Ok(built) = fs::metadata(&path).and_then(|meta| meta.modified()) else {
            refusals.push(format!(
                "{}: no release build; refusing to measure: {BUILD}",
                path.display()
            ));
            continue;
        };
        if let Some((at, source)) = &best
            && *at > built
        {
            refusals.push(format!(
                "{} is older than {}: refusing to measure a stale binary; rebuild: {BUILD}",
                path.display(),
                source.display()
            ));
        }
    }
    refusals
}

/// WM4 over this repository's release directory ([`stale_binaries`]),
/// before anything is seeded; the daemon's path.
fn fresh_binaries() -> PathBuf {
    let dir = library_common::release_dir();
    assert!(
        dir.file_name().is_some_and(|name| name == "release"),
        "{}: run the write soak with `--release` (it measures the release daemon)",
        dir.display()
    );
    let refusals = stale_binaries(&import_support::repository_root(), &dir);
    assert!(refusals.is_empty(), "WM4:\n{}", refusals.join("\n"));
    dir.join("specengine-http")
}

/// The binary's path, size and mtime (seconds since the epoch).
fn binary_facts(path: &Path) -> Value {
    let meta = fs::metadata(path).expect("the daemon's binary");
    let mtime = meta
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map(|since| since.as_secs());
    json!({"path": path.display().to_string(), "size": meta.len(), "mtime_unix": mtime})
}

// ------------------------------------------------------------ the process

/// macOS's per-process accounting, `libproc` (in libSystem).
#[cfg(target_os = "macos")]
mod darwin {
    use std::os::raw::{c_int, c_void};

    /// `struct rusage_info_v2` (`<sys/resource.h>`).
    #[repr(C)]
    #[derive(Default)]
    struct RusageInfoV2 {
        uuid: [u8; 16],
        user_time: u64,
        system_time: u64,
        pkg_idle_wkups: u64,
        interrupt_wkups: u64,
        pageins: u64,
        wired_size: u64,
        resident_size: u64,
        phys_footprint: u64,
        proc_start_abstime: u64,
        proc_exit_abstime: u64,
        child_user_time: u64,
        child_system_time: u64,
        child_pkg_idle_wkups: u64,
        child_interrupt_wkups: u64,
        child_pageins: u64,
        child_elapsed_abstime: u64,
        diskio_bytesread: u64,
        diskio_byteswritten: u64,
    }

    /// `struct proc_taskinfo` (`<sys/proc_info.h>`).
    #[repr(C)]
    #[derive(Default)]
    struct TaskInfo {
        virtual_size: u64,
        resident_size: u64,
        total_user: u64,
        total_system: u64,
        threads_user: u64,
        threads_system: u64,
        policy: i32,
        faults: i32,
        pageins: i32,
        cow_faults: i32,
        messages_sent: i32,
        messages_received: i32,
        syscalls_mach: i32,
        syscalls_unix: i32,
        csw: i32,
        threadnum: i32,
        numrunning: i32,
        priority: i32,
    }

    const RUSAGE_INFO_V2: c_int = 2;
    const PROC_PIDLISTFDS: c_int = 1;
    const PROC_PIDTASKINFO: c_int = 4;
    /// `struct proc_fdinfo`: two 32-bit fields.
    const FDINFO_SIZE: usize = 8;

    unsafe extern "C" {
        fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *mut c_void) -> c_int;
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut c_void,
            size: c_int,
        ) -> c_int;
    }

    /// (resident set, `phys_footprint`), bytes.
    pub fn memory(pid: u32) -> Option<(u64, u64)> {
        let mut info = RusageInfoV2::default();
        let pid = c_int::try_from(pid).ok()?;
        // SAFETY: `info` is a `rusage_info_v2`, the flavor asks for one.
        let done =
            unsafe { proc_pid_rusage(pid, RUSAGE_INFO_V2, (&raw mut info).cast::<c_void>()) };
        (done == 0).then_some((info.resident_size, info.phys_footprint))
    }

    pub fn threads(pid: u32) -> Option<u64> {
        let mut info = TaskInfo::default();
        let size = c_int::try_from(std::mem::size_of::<TaskInfo>()).ok()?;
        // SAFETY: the buffer is a `proc_taskinfo` of `size` bytes.
        let done = unsafe {
            proc_pidinfo(
                c_int::try_from(pid).ok()?,
                PROC_PIDTASKINFO,
                0,
                (&raw mut info).cast::<c_void>(),
                size,
            )
        };
        (done == size).then(|| u64::try_from(info.threadnum).unwrap_or(0))
    }

    pub fn open_fds(pid: u32) -> Option<u64> {
        let mut buffer = vec![0u8; 16_384 * FDINFO_SIZE];
        let size = c_int::try_from(buffer.len()).ok()?;
        // SAFETY: the buffer holds `size` bytes of `proc_fdinfo` entries.
        let done = unsafe {
            proc_pidinfo(
                c_int::try_from(pid).ok()?,
                PROC_PIDLISTFDS,
                0,
                buffer.as_mut_ptr().cast::<c_void>(),
                size,
            )
        };
        (done > 0).then(|| u64::try_from(done).unwrap_or(0) / FDINFO_SIZE as u64)
    }
}

/// One memory sample, KiB; `footprint` on macOS only.
#[derive(Debug, Clone, Copy)]
struct Sample {
    at: f64,
    rss_kib: u64,
    footprint_kib: Option<u64>,
}

impl Sample {
    fn json(self) -> Value {
        json!({
            "at_s": (self.at * 10.0).round() / 10.0,
            "rss_kib": self.rss_kib,
            "footprint_kib": self.footprint_kib,
        })
    }
}

/// The daemon's resident set and footprint, KiB.
fn memory(pid: u32) -> Option<(u64, Option<u64>)> {
    #[cfg(target_os = "macos")]
    {
        darwin::memory(pid).map(|(rss, footprint)| (rss / 1024, Some(footprint / 1024)))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        let rss = status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))?
            .trim()
            .trim_end_matches("kB")
            .trim()
            .parse()
            .ok()?;
        Some((rss, None))
    }
}

fn threads(pid: u32) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        darwin::threads(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        fs::read_to_string(format!("/proc/{pid}/status"))
            .ok()?
            .lines()
            .find_map(|line| line.strip_prefix("Threads:"))?
            .trim()
            .parse()
            .ok()
    }
}

fn open_fds(pid: u32) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        darwin::open_fds(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(fs::read_dir(format!("/proc/{pid}/fd")).ok()?.count() as u64)
    }
}

/// The daemon's live child processes (`ps -A -o ppid=`).
fn children(pid: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-A", "-o", "ppid="])
        .output()
        .ok()?;
    let wanted = pid.to_string();
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| line.trim() == wanted)
            .count() as u64,
    )
}

/// Threads, open fds and child processes now.
fn process_facts(pid: u32) -> Value {
    json!({
        "threads": threads(pid),
        "open_fds": open_fds(pid),
        "children": children(pid),
        "load_1m": load_1m(),
    })
}

/// The machine's 1-minute load average (`getloadavg`): a run under a
/// heavier load than its twin is rerun, not trusted.
fn load_1m() -> Option<f64> {
    unsafe extern "C" {
        fn getloadavg(loadavg: *mut f64, count: std::os::raw::c_int) -> std::os::raw::c_int;
    }
    let mut loads = [0f64; 3];
    // SAFETY: the buffer holds the 3 values asked for.
    let got = unsafe { getloadavg(loads.as_mut_ptr(), 3) };
    (got >= 1).then(|| rounded(loads[0]))
}

// ------------------------------------------------------------ heap mode

/// The directory heap mode keeps its files in, after the run.
/// A size as `vmmap` prints it (`2259K`, `16.2M`, `1.1G`, `512`), KiB.
fn vmmap_kib(text: &str) -> Option<f64> {
    let (number, scale) = match text.chars().last()? {
        'K' => (&text[..text.len() - 1], 1.0),
        'M' => (&text[..text.len() - 1], 1024.0),
        'G' => (&text[..text.len() - 1], 1024.0 * 1024.0),
        _ => (text, 1.0 / 1024.0),
    };
    number.parse::<f64>().ok().map(|value| value * scale)
}

/// The malloc zones' allocated bytes (the live heap), KiB, from `vmmap
/// --summary`'s MALLOC ZONE table's `TOTAL` row (its `BYTES ALLOCATED`
/// column, the sixth number), with the scan's ms; macOS only.
fn malloc_allocated(pid: u32) -> Option<(u64, u64)> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let started = Instant::now();
    let output = Command::new("/usr/bin/vmmap")
        .args(["--summary", &pid.to_string()])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let took = ms(started.elapsed());
    let text = String::from_utf8_lossy(&output.stdout);
    // The table's header line; "see MALLOC ZONE table below" comes before.
    let table = &text[text.rfind("\nMALLOC ZONE")?..];
    let total = table.lines().find(|line| line.starts_with("TOTAL"))?;
    let cells: Vec<&str> = total.split_whitespace().skip(1).collect();
    let kib = vmmap_kib(cells.get(5)?)?;
    Some((kib.round() as u64, took))
}

/// One quiet reading: when, the live heap and the footprint (KiB), the
/// drain's and the scan's ms.
#[derive(Debug, Clone, Copy)]
struct Quiet {
    at: f64,
    heap_kib: u64,
    footprint_kib: Option<u64>,
    drain_ms: u64,
    scan_ms: u64,
}

/// The quiet probe (macOS): every [`QUIET_EVERY`] it holds the clients,
/// waits for every call to answer and [`QUIET_SETTLE`], reads the malloc
/// zones' allocated bytes (what the daemon holds between calls, without
/// the allocator's kept pages or a call's working set), then lets the
/// clients go. A leak shows here call by call; the footprint's slow return
/// of a call's pages after the step does not.
fn quiet_probe(run: &Run<'_>) {
    let mut next = Instant::now() + QUIET_EVERY;
    while run.running() {
        if Instant::now() < next {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        next += QUIET_EVERY;
        let started = Instant::now();
        run.shared.pause.store(true, Ordering::SeqCst);
        while run.shared.active.load(Ordering::SeqCst) > 0
            && started.elapsed() < HANG
            && !run.shared.stop.load(Ordering::SeqCst)
        {
            thread::sleep(Duration::from_millis(5));
        }
        let drained = run.shared.active.load(Ordering::SeqCst) == 0;
        let drain_ms = ms(started.elapsed());
        if drained {
            thread::sleep(QUIET_SETTLE);
            let at = run.start.elapsed().as_secs_f64();
            let footprint_kib = memory(run.hub.pid).and_then(|(_, footprint)| footprint);
            if let Some((heap_kib, scan_ms)) = malloc_allocated(run.hub.pid) {
                run.shared.quiet.lock().expect("quiet").push(Quiet {
                    at,
                    heap_kib,
                    footprint_kib,
                    drain_ms,
                    scan_ms,
                });
            }
        }
        run.shared.pause.store(false, Ordering::SeqCst);
    }
    run.shared.pause.store(false, Ordering::SeqCst);
}

fn heap_dir() -> PathBuf {
    std::env::temp_dir().join("specengine-write-soak-heap")
}

/// `leaks --outputGraph` and `vmmap --summary` of the daemon, kept under
/// [`heap_dir`] as `<label>.*`; what was written.
fn heap_snapshot(pid: u32, label: &str) -> Value {
    if !cfg!(target_os = "macos") {
        return json!({"label": label, "skipped": "not macOS"});
    }
    let dir = heap_dir();
    fs::create_dir_all(&dir).expect("the heap directory");
    let started = Instant::now();
    let graph = dir.join(format!("{label}.memgraph"));
    let leaks_out = dir.join(format!("{label}.leaks.txt"));
    let leaks = Command::new("/usr/bin/leaks")
        .arg(pid.to_string())
        .arg(format!("--outputGraph={}", graph.display()))
        .stdout(fs::File::create(&leaks_out).expect("the leaks file"))
        .stderr(Stdio::null())
        .status()
        .ok()
        .and_then(|status| status.code());
    let vmmap_out = dir.join(format!("{label}.vmmap.txt"));
    let vmmap = Command::new("/usr/bin/vmmap")
        .args(["--summary", &pid.to_string()])
        .stdout(fs::File::create(&vmmap_out).expect("the vmmap file"))
        .stderr(Stdio::null())
        .status()
        .ok()
        .and_then(|status| status.code());
    let snapshot = json!({
        "label": label,
        "memgraph": graph.exists().then(|| graph.display().to_string()),
        "leaks_txt": leaks_out.display().to_string(),
        "leaks_status": leaks,
        "vmmap_txt": vmmap_out.display().to_string(),
        "vmmap_status": vmmap,
        "took_ms": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    });
    println!("write_soak heap snapshot: {snapshot}");
    snapshot
}

// ------------------------------------------------------------ the corpus

/// What the clients act on: ID sections and search words, from the files.
struct Corpus {
    /// ID sections spread over the part files.
    sections: Vec<String>,
    /// Words of the part files' titles.
    words: Vec<String>,
}

impl Corpus {
    fn of(tree: &Tree) -> Self {
        let step = (tree.parts.len() / U_TARGETS).max(1);
        let mut sections = Vec::new();
        let mut words = std::collections::BTreeSet::new();
        for (at, part) in tree.parts.iter().enumerate() {
            let text = fs::read_to_string(tree.root.join(part)).expect("a part file");
            if at % step == 0
                && sections.len() < U_TARGETS
                && let Some(id) = text
                    .split("{#")
                    .nth(1)
                    .and_then(|rest| rest.split('}').next())
            {
                sections.push(id.to_owned());
            }
            if words.len() < SEARCH_WORDS
                && let Some(title) = text.lines().find_map(|line| line.strip_prefix("# "))
            {
                for word in title.split_whitespace().filter(|word| word.len() >= 5) {
                    words.insert(word.to_lowercase());
                }
            }
        }
        assert!(
            sections.len() >= U_TARGETS / 2,
            "premise: {} ID sections",
            sections.len()
        );
        assert!(!words.is_empty(), "premise: title words to search");
        Self {
            sections,
            words: words.into_iter().take(SEARCH_WORDS).collect(),
        }
    }
}

/// A section U updates: its text and `span_hash`, read over `/mcp`.
#[derive(Debug, Clone)]
struct Target {
    id: String,
    text: String,
    span_hash: Value,
}

/// Each section's text and `span_hash` by `get_node` on the hub.
fn targets(hub: &Hub, slug: &str, corpus: &Corpus) -> Vec<Target> {
    corpus
        .sections
        .iter()
        .enumerate()
        .map(|(at, id)| {
            let (answer, response) = mcp_call(
                hub.port,
                slug,
                "agent-prefetch",
                i64::try_from(at + 1).unwrap_or(1),
                "get_node",
                json!({"id": id}),
                TIMEOUT,
            )
            .unwrap_or_else(|error| panic!("get_node {id}: {error}"));
            let node = response
                .as_ref()
                .map(|response| response["result"]["structuredContent"]["nodes"][0].clone())
                .unwrap_or(Value::Null);
            let text = node["text"]
                .as_str()
                .unwrap_or_else(|| panic!("get_node {id}: {} {response:?}", answer.status));
            assert!(node["span_hash"].is_string(), "get_node {id}: {node}");
            Target {
                id: id.clone(),
                text: text.to_owned(),
                span_hash: node["span_hash"].clone(),
            }
        })
        .collect()
}

// ------------------------------------------------------------ one run

/// One finished call.
#[derive(Debug, Clone)]
struct Call {
    write: bool,
    at: f64,
    ok: bool,
    stored: bool,
    hit: bool,
    ms: u64,
}

/// A `timeline` point (RM-0134): `(at_s, rss_kib, footprint_kib, pending)`.
type Point = (f64, u64, Option<u64>, usize);

/// What every thread of one run shares.
#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    hangs: AtomicUsize,
    next_id: AtomicU64,
    in_flight: InFlight,
    calls: Mutex<Vec<Call>>,
    samples: Mutex<Vec<Sample>>,
    middle: Mutex<Option<Value>>,
    heap: Mutex<Vec<Value>>,
    /// The quiet probe's readings (macOS).
    quiet: Mutex<Vec<Quiet>>,
    /// Set while the quiet probe holds the clients.
    pause: AtomicBool,
    /// Calls between their gate and their answer.
    active: AtomicUsize,
    failures: Mutex<Vec<String>>,
    hang_samples: Mutex<Vec<String>>,
    /// Writes stored so far in the run (RM-0134's pending count).
    stored: AtomicUsize,
    /// One point every [`TIMELINE_EVERY`].
    timeline: Mutex<Vec<Point>>,
    /// Each inbox read: `(at_s, response bytes, ms)`.
    inbox_reads: Mutex<Vec<(f64, usize, u64)>>,
    /// RF23: each page read's `(at_s, ms)`.
    page_ms: Mutex<Vec<(f64, u64)>>,
    /// RF23: uploads answered, Range reads checked.
    uploads: AtomicUsize,
    ranges: AtomicUsize,
}

impl Shared {
    fn fail(&self, what: String) {
        self.failures.lock().expect("failures").push(what);
    }

    fn running(&self, deadline: Instant) -> bool {
        !self.stop.load(Ordering::SeqCst) && Instant::now() < deadline
    }
}

fn ms(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

/// The run's clock and its hub.
struct Run<'a> {
    hub: &'a Hub,
    slug: &'a str,
    shared: &'a Shared,
    start: Instant,
    deadline: Instant,
    variant: Variant,
    reads: Reads,
    /// Questions pending before the run (RM-0134).
    prefill: usize,
    /// RF23: the stored files, `(b3, seed)`.
    files: &'a [(String, u64)],
}

impl Run<'_> {
    /// One `/mcp` call by `login`, timed and judged; a write must be stored
    /// unless the variant is H.
    fn call(&self, login: &str, tool: &str, arguments: Value, write: bool) {
        let _active = self.gate();
        let id = self.shared.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let what = format!("{tool} by {login}");
        let open = self.shared.in_flight.begin(&what);
        let started = Instant::now();
        let answered = mcp_call(
            self.hub.port,
            self.slug,
            login,
            i64::try_from(id).unwrap_or(1),
            tool,
            arguments,
            TIMEOUT,
        );
        self.shared.in_flight.end(open);
        let took = ms(started.elapsed());
        let at = started.duration_since(self.start).as_secs_f64();
        let (mut ok, mut stored, mut hit) = (false, false, false);
        match answered {
            Ok((answer, Some(response))) if answer.status == 200 => {
                let content = &response["result"]["structuredContent"];
                let error = response.get("error").is_some()
                    || response["result"]["isError"] == json!(true)
                    || !content.is_object();
                // A write's: CM19's `get_card` read answers with an `id` too.
                stored = write && content["id"].is_string();
                if stored {
                    self.shared.stored.fetch_add(1, Ordering::SeqCst);
                }
                hit = content["hits"]
                    .as_array()
                    .is_some_and(|hits| !hits.is_empty());
                // W, U, M: each write stored; H: stored once, then a hit.
                ok = !error && (!write || stored || (self.variant == Variant::H && hit));
                if !ok {
                    self.shared.fail(format!(
                        "{what}: {}",
                        response.to_string().chars().take(400).collect::<String>()
                    ));
                }
            }
            Ok((answer, _)) => self.shared.fail(format!(
                "{what}: {} with no response: {}",
                answer.status,
                answer.text().chars().take(200).collect::<String>()
            )),
            Err(error) => self.shared.fail(format!("{what}: no answer: {error}")),
        }
        self.shared.calls.lock().expect("calls").push(Call {
            write,
            at,
            ok,
            stored,
            hit,
            ms: took,
        });
    }

    /// `GET /api/projects/<slug>/inbox` by `login`.
    fn inbox(&self, login: &str) {
        let _active = self.gate();
        let path = format!("/api/projects/{}/inbox", self.slug);
        let what = format!("GET {path} by {login}");
        let open = self.shared.in_flight.begin(&what);
        let started = Instant::now();
        let answer = page_get(self.hub.port, &path, login, TIMEOUT);
        self.shared.in_flight.end(open);
        let at = started.duration_since(self.start).as_secs_f64();
        let took = ms(started.elapsed());
        if let Ok(answer) = &answer {
            self.shared.inbox_reads.lock().expect("inbox reads").push((
                at,
                answer.body.len(),
                took,
            ));
        }
        let ok = match answer {
            Ok(answer) if answer.status == 200 => true,
            Ok(answer) => {
                self.shared.fail(format!(
                    "{what}: {}: {}",
                    answer.status,
                    answer.text().chars().take(200).collect::<String>()
                ));
                false
            }
            Err(error) => {
                self.shared.fail(format!("{what}: no answer: {error}"));
                false
            }
        };
        self.shared.calls.lock().expect("calls").push(Call {
            write: false,
            at,
            ok,
            stored: false,
            hit: false,
            ms: took,
        });
    }

    /// A page's `GET path` by `login` (CM19's reads), timed and judged: a
    /// 200 with a JSON document.
    fn page(&self, login: &str, path: &str) {
        let _active = self.gate();
        let what = format!("GET {path} by {login}");
        let open = self.shared.in_flight.begin(&what);
        let started = Instant::now();
        let answer = page_get(self.hub.port, path, login, TIMEOUT);
        self.shared.in_flight.end(open);
        let at = started.duration_since(self.start).as_secs_f64();
        let took = ms(started.elapsed());
        let ok = match answer {
            Ok(answer)
                if answer.status == 200
                    && serde_json::from_slice::<Value>(&answer.body)
                        .is_ok_and(|v| v.is_object()) =>
            {
                true
            }
            Ok(answer) => {
                self.shared.fail(format!(
                    "{what}: {}: {}",
                    answer.status,
                    answer.text().chars().take(200).collect::<String>()
                ));
                false
            }
            Err(error) => {
                self.shared.fail(format!("{what}: no answer: {error}"));
                false
            }
        };
        self.shared.calls.lock().expect("calls").push(Call {
            write: false,
            at,
            ok,
            stored: false,
            hit: false,
            ms: took,
        });
    }

    fn running(&self) -> bool {
        self.shared.running(self.deadline)
    }

    /// Waits while the quiet probe holds the clients; the call counted
    /// active until the guard drops. Counted first, then the hold read: a
    /// call the probe did not see is one that saw the hold.
    fn gate(&self) -> Active<'_> {
        loop {
            self.shared.active.fetch_add(1, Ordering::SeqCst);
            if !self.shared.pause.load(Ordering::SeqCst) {
                return Active(&self.shared.active);
            }
            self.shared.active.fetch_sub(1, Ordering::SeqCst);
            while self.shared.pause.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// A call counted active; uncounted on drop.
struct Active<'a>(&'a AtomicUsize);

impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A writer: `login` writes back to back until the deadline.
fn writer(run: &Run<'_>, tree: &Tree, targets: &[Target], index: usize) {
    let login = format!("agent-{index}");
    let mut n = 0u64;
    while run.running() {
        n += 1;
        let seed = (u64::try_from(index).unwrap_or(0) << 32) | n;
        match run.variant {
            Variant::W | Variant::M => run.call(
                &login,
                "ask_question",
                json!({
                    "node_ids": [tree.part_id(usize::try_from(seed % 1_000_003).unwrap_or(0))],
                    "text": format!("{}?", nonce_words(seed, 6)),
                    "working_answer": nonce_words(seed ^ 0xa11, 3),
                    "price_of_other": nonce_words(seed ^ 0xb22, 3),
                    "author_role": "drafter",
                    "author_model": "m-1",
                    "run": format!("r-{login}"),
                }),
                true,
            ),
            Variant::H => run.call(
                &login,
                "ask_question",
                json!({
                    "node_ids": [tree.part_id(0)],
                    "text": "Does the soak's one question hold for every reader?",
                    "working_answer": "Yes, as written.",
                    "price_of_other": "A new rule case.",
                    "author_role": "drafter",
                    "author_model": "m-1",
                    "run": format!("r-{login}"),
                }),
                true,
            ),
            Variant::U => {
                let target =
                    &targets[usize::try_from(seed % 1_000_003).unwrap_or(0) % targets.len()];
                let mut text = target.text.clone();
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&format!("\nSoak {}.\n", nonce_words(seed, 8)));
                run.call(
                    &login,
                    "propose_change",
                    json!({
                        "kind": "update",
                        "target": target.id,
                        "base": target.span_hash,
                        "text": text,
                        "rationale": format!("Soak {login} {n}."),
                        "author_role": "drafter",
                        "author_model": "m-1",
                        "run": format!("r-{login}"),
                    }),
                    true,
                );
            }
            Variant::CM | Variant::RF => run.call(
                &login,
                "add_comment",
                json!({
                    "on": cm_card(usize::try_from(seed % 1_000_003).unwrap_or(0)),
                    "body": format!(
                        "{} @person-{}",
                        nonce_words(seed, 8),
                        seed % u64::try_from(CM_PEOPLE).unwrap_or(1)
                    ),
                    "author_role": "drafter",
                    "author_model": "m-1",
                    "run": format!("r-{login}"),
                }),
                true,
            ),
            Variant::R => unreachable!("R has no writers"),
        }
    }
}

/// CM19's card `at` (mod [`CM_CARDS`]): `C-0001` to `C-0020`, made in
/// this order by [`comments_prefill`] on a fresh queue.
fn cm_card(at: usize) -> String {
    format!("C-{:04}", at % CM_CARDS + 1)
}

/// CM19's reader `index`: the login `person-<index>`, cycling `get_card
/// {context: true}` over `/mcp`, the card's thread and its own mentions as
/// a page reads them, until the deadline.
fn cm_reader(run: &Run<'_>, index: usize) {
    let login = format!("person-{}", index % CM_PEOPLE);
    let mut n = index;
    while run.running() {
        let card = cm_card(n);
        run.call(
            &login,
            "get_card",
            json!({"card_id": card, "context": true}),
            false,
        );
        if !run.running() {
            break;
        }
        run.page(
            &login,
            &format!("/api/projects/{}/comments?on={card}", run.slug),
        );
        if !run.running() {
            break;
        }
        run.page(&login, &format!("/api/projects/{}/mentions", run.slug));
        n += 1;
    }
}

/// CM19: a template `HOME` holding [`CM_CARDS`] cards on the tree's parts
/// (`create_card` over `/mcp`) and [`CM_PREFILL`] comments on them, posted
/// by the [`CM_PEOPLE`] logins as a page posts them, each mentioning
/// another; the hub then stopped; each run copies it.
fn comments_prefill(tree: &Tree) -> PathBuf {
    let home = tree.scratch.join("hub-home-comments");
    let hub = Hub::start(&tree.root, &home, &tree.git, 3600);
    let started = Instant::now();
    for at in 0..CM_CARDS {
        let answered = mcp_call(
            hub.port,
            &tree.slug,
            "person-0",
            i64::try_from(at + 1).unwrap_or(1),
            "create_card",
            json!({"title": format!("Soak card {at}"), "target": [tree.part_id(at)]}),
            TIMEOUT,
        );
        let made = match answered {
            Ok((_, Some(response))) => response["result"]["structuredContent"]["id"].clone(),
            other => panic!("create_card {at}: {other:?}"),
        };
        assert_eq!(made, json!(cm_card(at)), "the cards in order");
    }
    let posted = AtomicUsize::new(0);
    thread::scope(|scope| {
        for person in 0..CM_PEOPLE {
            let (posted, hub) = (&posted, &hub);
            scope.spawn(move || {
                let login = format!("person-{person}");
                let mut headers = library_common::page_headers(&login);
                headers.push(("Origin".to_owned(), library_common::ORIGIN.to_owned()));
                headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
                for n in 0..CM_PREFILL / CM_PEOPLE {
                    let seed = (3u64 << 60) | (u64::try_from(person).unwrap_or(0) << 32) | n as u64;
                    let body = json!({
                        "on": cm_card(person * 7 + n),
                        "body": format!(
                            "{} @person-{}",
                            nonce_words(seed, 10),
                            (person + 1 + n) % CM_PEOPLE
                        ),
                    });
                    let answer = library_common::exchange(
                        hub.port,
                        "POST",
                        &format!("/api/projects/{}/comments", tree.slug),
                        &headers,
                        body.to_string().as_bytes(),
                        TIMEOUT,
                    )
                    .unwrap_or_else(|error| panic!("prefill {login} {n}: {error}"));
                    assert_eq!(answer.status, 200, "prefill {login} {n}: {}", answer.text());
                    posted.fetch_add(1, Ordering::SeqCst);
                }
            });
        }
    });
    drop(hub);
    let posted = posted.load(Ordering::SeqCst);
    assert_eq!(posted, CM_PREFILL);
    println!(
        "write_soak comments prefill: {CM_CARDS} cards, {posted} comments in {:.0} s",
        started.elapsed().as_secs_f64()
    );
    home
}

// ------------------------------------------------------------ RF23

/// The data directory under `home` (the CLI's README "Database").
fn data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/specengine")
    } else {
        home.join(".local/share/specengine")
    }
}

/// The free bytes of the file system holding `path` (`df -k`).
fn free_bytes(path: &Path) -> Option<u64> {
    let output = Command::new("/bin/df").arg("-k").arg(path).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let kib: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(kib * 1024)
}

/// A page's upload headers on the hub: its `Origin` and the binary type.
fn upload_headers(login: &str) -> Vec<(String, String)> {
    let mut headers = library_common::page_headers(login);
    headers.push(("Origin".to_owned(), library_common::ORIGIN.to_owned()));
    headers.push((
        "Content-Type".to_owned(),
        "application/octet-stream".to_owned(),
    ));
    headers
}

/// One of RF's paces, read once: `default` unless `name` is set, its
/// value then (0: unthrottled), `(pace, overridden)`, `None` unthrottled.
/// `WRITE_SOAK_RF_UPLOAD_MIBS` overrides [`RF_UPLOAD_MIBS`],
/// `WRITE_SOAK_RF_RANGES_PER_S` [`RF_RANGES_PER_S`]. A run with either set
/// is a diagnosis, never RF23's evidence (its report says `overridden`).
fn rf_knob(
    name: &'static str,
    default: f64,
    cell: &'static std::sync::OnceLock<(Option<f64>, bool)>,
) -> (Option<f64>, bool) {
    *cell.get_or_init(|| match std::env::var(name) {
        Ok(value) => {
            let pace = value
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("{name}={value:?}: a number (0: unthrottled)"));
            ((pace > 0.0).then_some(pace), true)
        }
        Err(_) => (Some(default), false),
    })
}

fn rf_upload_knob() -> (Option<f64>, bool) {
    static CELL: std::sync::OnceLock<(Option<f64>, bool)> = std::sync::OnceLock::new();
    rf_knob("WRITE_SOAK_RF_UPLOAD_MIBS", RF_UPLOAD_MIBS, &CELL)
}

fn rf_ranges_knob() -> (Option<f64>, bool) {
    static CELL: std::sync::OnceLock<(Option<f64>, bool)> = std::sync::OnceLock::new();
    rf_knob("WRITE_SOAK_RF_RANGES_PER_S", RF_RANGES_PER_S, &CELL)
}

/// Each uploader's MiB a second, `None` unthrottled.
fn rf_upload_mibs() -> Option<f64> {
    rf_upload_knob().0
}

/// Each Range reader's windows a second, `None` unthrottled.
fn rf_ranges_per_s() -> Option<f64> {
    rf_ranges_knob().0
}

/// `PUT /api/projects/<slug>/blobs` of a [`refs_common::SeekWav`] of
/// `total` bytes and `seed`, written in 64 KiB pieces as generated (never
/// held whole; at [`rf_upload_mibs`] unless unthrottled), the answer read
/// to its end.
fn put_stream(
    port: u16,
    slug: &str,
    login: &str,
    total: u64,
    seed: u64,
    timeout: Duration,
) -> Result<library_common::Answer, String> {
    let started = Instant::now();
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|error| format!("connect: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    let mut head = format!("PUT /api/projects/{slug}/blobs HTTP/1.1\r\n");
    for (name, value) in upload_headers(login) {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {total}\r\nConnection: close\r\n\r\n"
    ));
    stream
        .write_all(head.as_bytes())
        .map_err(|error| format!("write the head: {error}"))?;
    let mut body = refs_common::SeekWav::new(total, seed);
    let mut buffer = vec![0u8; 65_536];
    let rate = rf_upload_mibs().map(|mibs| mibs * 1_048_576.0);
    let mut sent = 0u64;
    loop {
        let n = body.read(&mut buffer).map_err(|error| error.to_string())?;
        if n == 0 {
            break;
        }
        stream
            .write_all(&buffer[..n])
            .map_err(|error| format!("write the body: {error}"))?;
        sent += n as u64;
        if let Some(rate) = rate {
            let due = Duration::from_secs_f64(sent as f64 / rate);
            if let Some(rest) = due.checked_sub(started.elapsed()) {
                thread::sleep(rest);
            }
        }
    }
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| format!("read after {:?}: {error}", started.elapsed()))?;
    let end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| format!("no head: {}", String::from_utf8_lossy(&raw)))?;
    let text = String::from_utf8_lossy(&raw[..end]).into_owned();
    let status = text
        .split("\r\n")
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("no status: {text}"))?;
    let body = raw[end + 4..].to_vec();
    let body = if text
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(&body)
    } else {
        body
    };
    Ok(library_common::Answer {
        status,
        headers: Vec::new(),
        body,
        elapsed: started.elapsed(),
    })
}

fn dechunk(mut rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(end) = rest.windows(2).position(|pair| pair == b"\r\n") {
        let size = usize::from_str_radix(
            String::from_utf8_lossy(&rest[..end])
                .split(';')
                .next()
                .unwrap_or("0")
                .trim(),
            16,
        )
        .unwrap_or(0);
        rest = &rest[end + 2..];
        if size == 0 || rest.len() < size {
            break;
        }
        out.extend_from_slice(&rest[..size]);
        rest = &rest[(size + 2).min(rest.len())..];
    }
    out
}

/// RF23's pre-fill on CM's template `HOME`: a hub over it stores
/// [`RF_FILES`] WAV files of [`RF_FILE_BYTES`] (seeds `1000 + n`) and
/// attaches each to a card with "what matters", then stops; each run copies
/// the `HOME`, its blob store with it. The files' `(b3, seed)`.
fn references_prefill(tree: &Tree, home: &Path) -> Vec<(String, u64)> {
    let hub = Hub::start(&tree.root, home, &tree.git, 3600);
    let started = Instant::now();
    let mut files = Vec::new();
    for at in 0..RF_FILES {
        let seed = 1000 + at as u64;
        let answer = put_stream(
            hub.port,
            &tree.slug,
            "person-0",
            RF_FILE_BYTES,
            seed,
            TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("prefill PUT {at}: {error}"));
        assert_eq!(answer.status, 200, "prefill PUT {at}: {}", answer.text());
        let blob: Value = serde_json::from_slice(&answer.body).expect("a stored blob");
        let b3 = blob["b3"].as_str().expect("a b3").to_owned();
        let mut headers = library_common::page_headers("person-0");
        headers.push(("Origin".to_owned(), library_common::ORIGIN.to_owned()));
        headers.push(("Content-Type".to_owned(), "application/json".to_owned()));
        let body = json!({
            "on": cm_card(at),
            "b3": b3,
            "name": format!("tide-{at}.wav"),
            "matters": "the swell under the hull",
        });
        let attached = library_common::exchange(
            hub.port,
            "POST",
            &format!("/api/projects/{}/references", tree.slug),
            &headers,
            body.to_string().as_bytes(),
            TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("prefill POST {at}: {error}"));
        assert_eq!(
            attached.status,
            200,
            "prefill POST {at}: {}",
            attached.text()
        );
        files.push((b3, seed));
    }
    drop(hub);
    println!(
        "write_soak references prefill: {} files of {RF_FILE_BYTES} bytes in {:.0} s",
        files.len(),
        started.elapsed().as_secs_f64()
    );
    files
}

/// Waits out [`RF_BASELINE`] (or the run's end).
fn wait_baseline(run: &Run<'_>) {
    while run.running() && run.start.elapsed() < RF_BASELINE {
        thread::sleep(Duration::from_millis(100));
    }
}

impl Run<'_> {
    /// One RF step by `login`, timed, gated and watched: `step` answers
    /// `Ok(stored)` or why it failed.
    fn step(
        &self,
        login: &str,
        what: &str,
        write: bool,
        step: impl FnOnce() -> Result<bool, String>,
    ) {
        let _active = self.gate();
        let what = format!("{what} by {login}");
        let open = self.shared.in_flight.begin(&what);
        let started = Instant::now();
        let outcome = step();
        self.shared.in_flight.end(open);
        let at = started.duration_since(self.start).as_secs_f64();
        let took = ms(started.elapsed());
        let (ok, stored) = match outcome {
            Ok(stored) => (true, stored),
            Err(why) => {
                self.shared.fail(format!("{what}: {why}"));
                (false, false)
            }
        };
        if stored {
            self.shared.stored.fetch_add(1, Ordering::SeqCst);
        }
        self.shared.calls.lock().expect("calls").push(Call {
            write,
            at,
            ok,
            stored,
            hit: false,
            ms: took,
        });
    }
}

/// An uploader from [`RF_BASELINE`] on: a fresh [`RF_UPLOAD_BYTES`] WAV
/// `PUT` (a new file), its `POST` with "what matters" on a card, its
/// `DELETE` (the file collected), a whole cycle once begun, so the store
/// ends with the pre-fill's files alone.
fn rf_uploader(run: &Run<'_>, index: usize) {
    let login = format!("uploader-{index}");
    wait_baseline(run);
    let mut n = 0u64;
    let headers = {
        let mut headers = library_common::page_headers(&login);
        headers.push(("Origin".to_owned(), library_common::ORIGIN.to_owned()));
        headers
    };
    while run.running() {
        n += 1;
        let seed = ((index as u64 + 1) << 40) | n;
        let mut b3 = None;
        run.step(&login, "PUT blobs", true, || {
            let answer = put_stream(
                run.hub.port,
                run.slug,
                &login,
                RF_UPLOAD_BYTES,
                seed,
                TIMEOUT,
            )?;
            if answer.status != 200 {
                return Err(format!("{}: {}", answer.status, answer.text()));
            }
            let blob: Value = serde_json::from_slice(&answer.body).map_err(|e| e.to_string())?;
            if blob["stored"] != json!("new") || blob["size"] != json!(RF_UPLOAD_BYTES) {
                return Err(format!("not a new file of its size: {blob}"));
            }
            b3 = blob["b3"].as_str().map(str::to_owned);
            run.shared.uploads.fetch_add(1, Ordering::SeqCst);
            Ok(false)
        });
        let Some(b3) = b3 else { continue };
        let mut id = None;
        run.step(&login, "POST references", true, || {
            let mut posting = headers.clone();
            posting.push(("Content-Type".to_owned(), "application/json".to_owned()));
            let body = json!({
                "on": cm_card(usize::try_from(seed % 1_000_003).unwrap_or(0)),
                "b3": b3,
                "name": format!("take-{index}-{n}.wav"),
                "matters": nonce_words(seed, 5),
            });
            let answer = library_common::exchange(
                run.hub.port,
                "POST",
                &format!("/api/projects/{}/references", run.slug),
                &posting,
                body.to_string().as_bytes(),
                TIMEOUT,
            )?;
            if answer.status != 200 {
                return Err(format!("{}: {}", answer.status, answer.text()));
            }
            let made: Value = serde_json::from_slice(&answer.body).map_err(|e| e.to_string())?;
            id = made["id"].as_str().map(str::to_owned);
            Ok(id.is_some())
        });
        let Some(id) = id else { continue };
        run.step(&login, "DELETE references", true, || {
            let answer = library_common::exchange(
                run.hub.port,
                "DELETE",
                &format!("/api/projects/{}/references/{id}", run.slug),
                &headers,
                b"",
                TIMEOUT,
            )?;
            if answer.status != 200 {
                return Err(format!("{}: {}", answer.status, answer.text()));
            }
            let gone: Value = serde_json::from_slice(&answer.body).map_err(|e| e.to_string())?;
            if gone["deleted"]["by"] != json!(login) {
                return Err(format!("not deleted by {login}: {gone}"));
            }
            Ok(true)
        });
    }
}

/// A Range reader from [`RF_BASELINE`] on: random [`RF_WINDOW`] windows of
/// the pre-filled files, each answer a 206 whose bytes are the window's.
fn rf_reader(run: &Run<'_>, index: usize) {
    let login = format!("reader-{index}");
    wait_baseline(run);
    let mut state = 0x5EED_0000_u64 + index as u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let pace = rf_ranges_per_s().map(|per_s| Duration::from_secs_f64(1.0 / per_s));
    while run.running() {
        let tick = Instant::now();
        let (b3, seed) = &run.files[usize::try_from(next() % RF_FILES as u64).unwrap_or(0)];
        let offset = next() % (RF_FILE_BYTES - RF_WINDOW);
        let last = offset + RF_WINDOW - 1;
        run.step(&login, "GET blobs Range", false, || {
            let mut headers = library_common::page_headers(&login);
            headers.push(("Range".to_owned(), format!("bytes={offset}-{last}")));
            let answer = library_common::exchange(
                run.hub.port,
                "GET",
                &format!("/api/projects/{}/blobs/{b3}", run.slug),
                &headers,
                b"",
                TIMEOUT,
            )?;
            if answer.status != 206 {
                return Err(format!("{}: {}", answer.status, answer.text()));
            }
            if answer.body != refs_common::wav_window(RF_FILE_BYTES, *seed, offset, RF_WINDOW) {
                return Err(format!("bytes {offset}-{last} of {b3} differ"));
            }
            run.shared.ranges.fetch_add(1, Ordering::SeqCst);
            Ok(false)
        });
        if let Some(rest) = pace.and_then(|pace| pace.checked_sub(tick.elapsed())) {
            thread::sleep(rest);
        }
    }
}

/// The page reader: every second `GET .../cards` and a card's `GET
/// .../comments?on=`, each timed into `page_ms`.
fn rf_page_reader(run: &Run<'_>) {
    let login = "page-0";
    let mut n = 0usize;
    while run.running() {
        let tick = Instant::now();
        for path in [
            format!("/api/projects/{}/cards", run.slug),
            format!("/api/projects/{}/comments?on={}", run.slug, cm_card(n)),
        ] {
            let mut took = 0;
            let mut at = 0.0;
            run.step(login, &format!("GET {path}"), false, || {
                let started = Instant::now();
                at = started.duration_since(run.start).as_secs_f64();
                let answer = page_get(run.hub.port, &path, login, TIMEOUT)?;
                took = ms(started.elapsed());
                if answer.status != 200 {
                    return Err(format!("{}: {}", answer.status, answer.text()));
                }
                Ok(false)
            });
            run.shared.page_ms.lock().expect("pages").push((at, took));
        }
        n += 1;
        if let Some(rest) = Duration::from_secs(1).checked_sub(tick.elapsed()) {
            thread::sleep(rest);
        }
    }
}

/// The page reads before [`RF_BASELINE`] and after it, ms.
fn page_phases(pages: &[(f64, u64)]) -> (Vec<u64>, Vec<u64>) {
    let line = RF_BASELINE.as_secs_f64();
    (
        pages
            .iter()
            .filter(|(at, _)| *at < line)
            .map(|(_, ms)| *ms)
            .collect(),
        pages
            .iter()
            .filter(|(at, _)| *at >= line)
            .map(|(_, ms)| *ms)
            .collect(),
    )
}

fn quantile_of(values: &[u64], q: f64) -> Option<u64> {
    (!values.is_empty()).then(|| library_common::quantile(values, q))
}

/// RF23's own bars: the page reader's p95 after the baseline at most 1.5 x
/// its p95 before + 50 ms; uploads and Range reads ran; the store holds
/// exactly the pre-filled files and no `.part`.
fn rf_verdict(shared: &Shared, home: &Path, slug: &str, files: &[(String, u64)]) -> Vec<String> {
    let mut broken = Vec::new();
    let pages = shared.page_ms.lock().expect("pages").clone();
    let (before, after) = page_phases(&pages);
    match (quantile_of(&before, 0.95), quantile_of(&after, 0.95)) {
        (Some(base), Some(late)) => {
            let bar = base as f64 * 1.5 + 50.0;
            if late as f64 > bar {
                broken.push(format!(
                    "the page reader's p95 {late} ms after the baseline > 1.5 x {base} + 50 = {bar} ms"
                ));
            }
        }
        _ => broken.push(format!(
            "the page reader read {} times before and {} after the baseline",
            before.len(),
            after.len()
        )),
    }
    if shared.uploads.load(Ordering::SeqCst) == 0 {
        broken.push("no upload ran".to_owned());
    }
    if shared.ranges.load(Ordering::SeqCst) == 0 {
        broken.push("no Range read ran".to_owned());
    }
    let store = data_dir(home).join("blobs").join(slug);
    let held = refs_common::store_files(&store);
    let mut want: Vec<String> = files.iter().map(|(b3, _)| b3.clone()).collect();
    want.sort();
    if held != want {
        broken.push(format!(
            "the store holds {} files, not the {} pre-filled ({} extra)",
            held.len(),
            want.len(),
            held.iter().filter(|name| !want.contains(name)).count()
        ));
    }
    let parts = refs_common::parts(&store);
    if !parts.is_empty() {
        broken.push(format!("{} .part files left: {parts:?}", parts.len()));
    }
    broken
}

/// A reader: `get_node`, `search`, the inbox, in turn (those
/// `WRITE_SOAK_READS` names), until the deadline.
fn reader(run: &Run<'_>, corpus: &Corpus, index: usize) {
    if run.variant == Variant::CM {
        cm_reader(run, index);
        return;
    }
    if run.variant == Variant::RF {
        rf_reader(run, index);
        return;
    }
    let login = format!("reader-{index}");
    let mut n = index;
    while run.running() {
        if run.reads.node {
            let id = &corpus.sections[n % corpus.sections.len()];
            run.call(&login, "get_node", json!({"id": id}), false);
            if !run.running() {
                break;
            }
        }
        if run.reads.search {
            let word = &corpus.words[n % corpus.words.len()];
            run.call(&login, "search", json!({"query": word, "limit": 20}), false);
            if !run.running() {
                break;
            }
        }
        if run.reads.inbox {
            run.inbox(&login);
        }
        n += 1;
    }
}

/// RM-0134: a template `HOME` holding at least `pending` stored questions,
/// filled by a hub of its own (8 logins `filler-N`, fresh text, as W asks),
/// the hub then stopped; each run copies it. With how many it holds (8
/// writers overshoot `pending` by up to 7).
fn prefill(tree: &Tree, pending: usize) -> (PathBuf, usize) {
    let home = tree.scratch.join("hub-home-template");
    let hub = Hub::start(&tree.root, &home, &tree.git, 3600);
    let started = Instant::now();
    let stored = fill(&hub, tree, pending, &AtomicU64::new(0));
    drop(hub);
    println!(
        "write_soak prefill: {stored} questions in {:.0} s",
        started.elapsed().as_secs_f64()
    );
    (home, stored)
}

/// RM-0134: `count` more questions stored through `hub` by 8 logins
/// `filler-N`, fresh text as W asks, `next` numbering them across calls;
/// how many were stored (at least `count`, else a panic).
fn fill(hub: &Hub, tree: &Tree, count: usize, next: &AtomicU64) -> usize {
    let stored = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    thread::scope(|scope| {
        for index in 0..WRITERS {
            let (stored, failed) = (&stored, &failed);
            scope.spawn(move || {
                let login = format!("filler-{index}");
                while stored.load(Ordering::SeqCst) < count && failed.load(Ordering::SeqCst) < 20
                {
                    let n = next.fetch_add(1, Ordering::SeqCst) + 1;
                    let seed = (1u64 << 62) | n;
                    let answered = mcp_call(
                        hub.port,
                        &tree.slug,
                        &login,
                        i64::try_from(n).unwrap_or(1),
                        "ask_question",
                        json!({
                            "node_ids": [tree.part_id(usize::try_from(seed % 1_000_003).unwrap_or(0))],
                            "text": format!("{}?", nonce_words(seed, 6)),
                            "working_answer": nonce_words(seed ^ 0xa11, 3),
                            "price_of_other": nonce_words(seed ^ 0xb22, 3),
                            "author_role": "drafter",
                            "author_model": "m-1",
                            "run": format!("r-{login}"),
                        }),
                        TIMEOUT,
                    );
                    match answered {
                        Ok((_, Some(response)))
                            if response["result"]["structuredContent"]["id"].is_string() =>
                        {
                            stored.fetch_add(1, Ordering::SeqCst);
                        }
                        _ => {
                            failed.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            });
        }
    });
    let stored = stored.load(Ordering::SeqCst);
    assert!(
        stored >= count,
        "the fill stored {stored} of {count} ({} failed)",
        failed.load(Ordering::SeqCst)
    );
    stored
}

/// One `spec` run under `/usr/bin/time -l` (macOS): its maximum resident
/// set and peak footprint (bytes), its stdout's bytes and its wall ms.
fn peak_of(
    spec: &Path,
    vars: &[(std::ffi::OsString, std::ffi::OsString)],
    root: &Path,
    args: &[&str],
) -> Value {
    let started = Instant::now();
    let output = Command::new("/usr/bin/time")
        .arg("-l")
        .arg(spec)
        .args(args)
        .env_clear()
        .envs(vars.iter().map(|(name, value)| (name, value)))
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .expect("spec under time");
    let took = ms(started.elapsed());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let field = |label: &str| -> Option<u64> {
        stderr
            .lines()
            .find(|line| line.trim_end().ends_with(label))
            .and_then(|line| line.split_whitespace().next())
            .and_then(|number| number.parse().ok())
    };
    assert!(output.status.success(), "spec {args:?}: {stderr}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    json!({
        "max_rss": field("maximum resident set size"),
        "peak_footprint": field("peak memory footprint"),
        "stdout_bytes": output.stdout.len(),
        "listed": document["proposals"].as_array().map(Vec::len),
        "ms": took,
    })
}

/// RM-0134: one inbox read's peak against the pending count. A hub over
/// the tree stores questions to 0, 1 000, 2 000, 3 000 and 4 000 pending;
/// at each level the release `spec inbox --json` runs three times on the
/// hub's `HOME` under `/usr/bin/time -l` (a process of its own: the read's
/// whole working set, not the daemon's kept pages). One JSON line per
/// level, then the judged one (RM-0134 MX3): each level's pending count
/// and each read's response bytes are 540db8ff's ([`HEAD_INBOX_BYTES`]), and
/// the median peak footprint's rise per pending item from the 1 000 level to
/// the 4 000 one is at most [`INBOX_PEAK_BYTES_PER_ITEM`] (540db8ff: about
/// 6 600). macOS only (`/usr/bin/time -l`); elsewhere it runs nothing.
#[test]
#[ignore = "RM-0134, release, about 12 min: cargo nextest run -p specengine-eval --release \
            --test write_soak rm0134_inbox_peak --run-ignored only --no-capture"]
fn rm0134_inbox_peak_by_pending() {
    if !cfg!(target_os = "macos") {
        println!("rm0134_inbox_peak_by_pending: macOS's /usr/bin/time -l only; not run");
        return;
    }
    fresh_binaries();
    let tree = Tree::new("inbox-peak", NODES, COMMITS);
    let home = tree.scratch.join("hub-home-peak");
    let hub = Hub::start(&tree.root, &home, &tree.git, 3600);
    let spec = library_common::release_bin("spec");
    let vars = tree.spec_vars(&home);
    let next = AtomicU64::new(0);
    let mut pending = 0;
    let mut medians: Vec<(usize, Option<u64>)> = Vec::new();
    let mut bytes: Vec<(usize, Vec<u64>)> = Vec::new();
    for level in [0usize, 1_000, 2_000, 3_000, 4_000] {
        if level > pending {
            pending += fill(&hub, &tree, level - pending, &next);
        }
        let reads: Vec<Value> = (0..3)
            .map(|_| peak_of(&spec, &vars, &tree.root, &["inbox", "--json"]))
            .collect();
        let mut peaks: Vec<u64> = reads
            .iter()
            .filter_map(|read| read["peak_footprint"].as_u64())
            .collect();
        peaks.sort_unstable();
        medians.push((pending, (peaks.len() == 3).then(|| peaks[1])));
        bytes.push((
            pending,
            reads
                .iter()
                .map(|read| read["stdout_bytes"].as_u64().unwrap_or(0))
                .collect(),
        ));
        println!(
            "{}",
            json!({"test": "rm0134_inbox_peak", "pending": pending, "reads": reads})
        );
    }
    drop(hub);
    let head: Vec<(usize, Vec<u64>)> = HEAD_INBOX_BYTES
        .iter()
        .map(|&(pending, answer)| (pending, vec![answer; 3]))
        .collect();
    assert_eq!(
        bytes, head,
        "each level's pending count and each read's response bytes are 540db8ff's"
    );
    let per_item = inbox_peak_per_item(&medians).expect("the 1 000 and 4 000 levels' peaks");
    println!(
        "{}",
        json!({"test": "rm0134_inbox_peak", "bytes_per_item": per_item,
               "bar": INBOX_PEAK_BYTES_PER_ITEM})
    );
    assert!(
        per_item <= INBOX_PEAK_BYTES_PER_ITEM,
        "one inbox read's peak footprint rises {per_item:.0} B per pending item > \
         {INBOX_PEAK_BYTES_PER_ITEM} (540db8ff: about 6 600)"
    );
}

/// RM-0134 MX3: the most one `spec inbox --json` read's peak footprint may
/// rise per pending item, bytes: a third of 540db8ff's 6.6 KB.
const INBOX_PEAK_BYTES_PER_ITEM: f64 = 2_200.0;

/// 540db8ff's `(pending, stdout bytes)` of `spec inbox --json` at each level
/// of [`rm0134_inbox_peak_by_pending`] (RM-0134's diagnosis; the fix's first
/// run read the same): the fill stores 7 past each level (8 writers in flight), the
/// questions' texts fixed by their numbers, so the answer's length is the
/// level's whatever order the turn stored them in.
const HEAD_INBOX_BYTES: [(usize, u64); 5] = [
    (0, 28),
    (1_007, 306_155),
    (2_007, 610_155),
    (3_007, 914_155),
    (4_007, 1_218_155),
];

/// The median peak's rise per pending item from the level nearest 1 000 to
/// the one nearest 4 000 (`(pending, median peak)` pairs), bytes.
fn inbox_peak_per_item(medians: &[(usize, Option<u64>)]) -> Option<f64> {
    let near = |level: usize| {
        medians
            .iter()
            .min_by_key(|(pending, _)| pending.abs_diff(level))
            .and_then(|(pending, peak)| peak.map(|peak| (*pending as f64, peak as f64)))
    };
    let ((low_n, low), (high_n, high)) = (near(1_000)?, near(4_000)?);
    (high_n > low_n).then(|| (high - low) / (high_n - low_n))
}

/// The diagnosis' curve (RM-0134, 540db8ff) reads about 6.6 KB per item,
/// red; a third of it passes.
#[test]
fn mx3_the_inbox_peak_is_judged_per_pending_item() {
    let mib = |m: u64| m * 1024 * 1024;
    let head = [
        (0, Some(mib(3))),
        (1_007, Some(mib(10))),
        (2_007, Some(mib(17))),
        (3_007, Some(mib(23))),
        (4_007, Some(mib(29))),
    ];
    let per_item = inbox_peak_per_item(&head).expect("a rise");
    assert!((per_item - 6_641.0).abs() < 1.0, "{per_item}");
    assert!(per_item > INBOX_PEAK_BYTES_PER_ITEM);
    let fixed = [(1_000, Some(10_000_000)), (4_000, Some(16_000_000))];
    assert_eq!(inbox_peak_per_item(&fixed), Some(2_000.0));
    assert_eq!(
        inbox_peak_per_item(&[(1_000, None), (4_000, Some(1))]),
        None
    );
}

/// `from` copied to `to`, every directory and file (a stopped hub's `HOME`:
/// its index and queue, their WAL files included).
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the copy's directory");
    for entry in fs::read_dir(from).expect("the template").flatten() {
        let (path, target) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().expect("a file type");
        if kind.is_dir() {
            copy_tree(&path, &target);
        } else if kind.is_file() {
            fs::write(&target, fs::read(&path).expect("a template file")).expect("the copy");
        }
    }
}

/// The hang watchdog: a request open for [`HANG`] samples the daemon once,
/// prints the file's path and stops the run.
fn watchdog(run: &Run<'_>, label: &str) {
    let mut seen = std::collections::BTreeSet::new();
    while run.shared.running(run.deadline + HANG) {
        for (id, what) in run.shared.in_flight.older_than(HANG) {
            if seen.insert(id) {
                run.shared.hangs.fetch_add(1, Ordering::SeqCst);
                let place = run.hub.sample(label).map_or_else(
                    || "no sample (not macOS)".to_owned(),
                    |path| path.display().to_string(),
                );
                println!("HANG {label}: {what} open {HANG:?}; sample {place}");
                run.shared.hang_samples.lock().expect("samples").push(place);
                run.shared.fail(format!("a hang: {what}"));
                run.shared.stop.store(true, Ordering::SeqCst);
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// The sampler: memory every [`SAMPLE_EVERY`], the process's facts once
/// past the middle.
fn sampler(run: &Run<'_>, secs: u64) {
    let pid = run.hub.pid;
    let mut next_point = 0.0;
    while run.running() {
        let at = run.start.elapsed().as_secs_f64();
        if let Some((rss_kib, footprint_kib)) = memory(pid) {
            run.shared.samples.lock().expect("samples").push(Sample {
                at,
                rss_kib,
                footprint_kib,
            });
            if at >= next_point {
                next_point += TIMELINE_EVERY;
                let pending = run.prefill + run.shared.stored.load(Ordering::SeqCst);
                run.shared.timeline.lock().expect("timeline").push((
                    at,
                    rss_kib,
                    footprint_kib,
                    pending,
                ));
            }
        }
        if at >= secs as f64 / 2.0 {
            let mut middle = run.shared.middle.lock().expect("middle");
            if middle.is_none() {
                *middle = Some(process_facts(pid));
            }
        }
        thread::sleep(SAMPLE_EVERY);
    }
}

/// `(seconds, KiB)` pairs of `samples` in `[from, to)`, by `pick`.
fn window(
    samples: &[Sample],
    from: f64,
    to: f64,
    pick: impl Fn(&Sample) -> Option<u64>,
) -> Vec<(f64, u64)> {
    samples
        .iter()
        .filter(|sample| sample.at >= from && sample.at < to)
        .filter_map(|sample| pick(sample).map(|kib| (sample.at, kib)))
        .collect()
}

fn rounded(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// Whole, judged (from a third on) and per-third slopes, MiB per minute.
fn slopes(samples: &[Sample], secs: u64, pick: impl Fn(&Sample) -> Option<u64> + Copy) -> Value {
    let points = window(samples, 0.0, f64::INFINITY, pick);
    point_slopes(&points, secs)
}

/// As [`slopes`], over `(seconds, KiB)` points.
fn point_slopes(points: &[(f64, u64)], secs: u64) -> Value {
    let third = secs as f64 / 3.0;
    let end = f64::INFINITY;
    let slope = |from: f64, to: f64| -> Option<f64> {
        let points: Vec<(f64, u64)> = points
            .iter()
            .copied()
            .filter(|(at, _)| *at >= from && *at < to)
            .collect();
        (points.len() >= 2).then(|| rounded(theil_sen_mib_per_minute(&points)))
    };
    json!({
        "whole": slope(0.0, end),
        "judged": slope(third, end),
        "thirds": [slope(0.0, third), slope(third, 2.0 * third), slope(2.0 * third, end)],
    })
}

/// What memory is judged on: the quiet probe's live heap on macOS, the
/// resident set elsewhere.
fn judged_on(macos: bool) -> &'static str {
    if macos { "live_heap" } else { "rss" }
}

/// The judged series' slope from `SOAK_SECS/3` on, MiB per minute, and
/// what it breaks against `budget` (`None`: reported, not judged): on macOS
/// the quiet readings' live heap, fewer than [`MIN_QUIET`] of them judged a
/// break; elsewhere the resident set, fewer than [`MIN_JUDGED_SAMPLES`] a
/// break. The other series never enter the verdict.
fn judge(
    samples: &[Sample],
    quiet: &[Quiet],
    secs: u64,
    macos: bool,
    budget: Option<f64>,
) -> (Option<f64>, Vec<String>) {
    let points: Vec<(f64, u64)> = if macos {
        quiet.iter().map(|q| (q.at, q.heap_kib)).collect()
    } else {
        samples.iter().map(|s| (s.at, s.rss_kib)).collect()
    };
    let slope = point_slopes(&points, secs)["judged"].as_f64();
    let mut broken = Vec::new();
    let Some(budget) = budget else {
        return (slope, broken);
    };
    let on = judged_on(macos);
    let third = secs as f64 / 3.0;
    let count = points.iter().filter(|(at, _)| *at >= third).count();
    let least = if macos { MIN_QUIET } else { MIN_JUDGED_SAMPLES };
    if count < least {
        broken.push(format!(
            "{count} {on} readings judged (from SOAK_SECS/3 on): fewer than {least}"
        ));
    }
    match slope {
        Some(slope) if slope > budget => broken.push(format!(
            "memory grows {slope:.3} MiB/min ({on}, from SOAK_SECS/3 on) > the budget {budget}"
        )),
        Some(_) => {}
        None => broken.push(format!("no {on} slope")),
    }
    (slope, broken)
}

/// RM-0134 MX6, the second judge (macOS; elsewhere `(None, [])`: RSS is
/// the first): the quiet readings' footprint, its slope from
/// `SOAK_SECS/3` on, MiB per minute, and what it breaks against `budget`
/// (`None`: reported, not judged); fewer than [`MIN_QUIET`] judged readings
/// with a footprint break the run.
fn judge_footprint(
    quiet: &[Quiet],
    secs: u64,
    macos: bool,
    budget: Option<f64>,
) -> (Option<f64>, Vec<String>) {
    if !macos {
        return (None, Vec::new());
    }
    let points: Vec<(f64, u64)> = quiet
        .iter()
        .filter_map(|q| q.footprint_kib.map(|kib| (q.at, kib)))
        .collect();
    let slope = point_slopes(&points, secs)["judged"].as_f64();
    let mut broken = Vec::new();
    let Some(budget) = budget else {
        return (slope, broken);
    };
    let third = secs as f64 / 3.0;
    let count = points.iter().filter(|(at, _)| *at >= third).count();
    if count < MIN_QUIET {
        broken.push(format!(
            "{count} quiet_footprint readings judged (from SOAK_SECS/3 on): fewer than {MIN_QUIET}"
        ));
    }
    match slope {
        Some(slope) if slope > budget => broken.push(format!(
            "the footprint grows {slope:.3} MiB/min (quiet_footprint, from SOAK_SECS/3 on) > \
             the budget {budget}"
        )),
        Some(_) => {}
        None => broken.push("no quiet_footprint slope".to_owned()),
    }
    (slope, broken)
}

/// The quiet probe's readings: the live heap between calls and the
/// footprint at the same moments, their slopes, KiB kept per write; `null`
/// off macOS (no probe), every key present on it, a run with no reading
/// included.
fn quiet_report(
    quiet: &[Quiet],
    secs: u64,
    macos: bool,
    judged_writes: f64,
    judged_minutes: f64,
) -> Value {
    if !macos {
        return Value::Null;
    }
    let heap: Vec<(f64, u64)> = quiet.iter().map(|q| (q.at, q.heap_kib)).collect();
    let footprint: Vec<(f64, u64)> = quiet
        .iter()
        .filter_map(|q| q.footprint_kib.map(|kib| (q.at, kib)))
        .collect();
    let heap_slopes = point_slopes(&heap, secs);
    let per_write = heap_slopes["judged"].as_f64().and_then(|slope| {
        (judged_writes > 0.0).then(|| rounded(slope * 1024.0 / (judged_writes / judged_minutes)))
    });
    json!({
        "count": quiet.len(),
        "heap_kib": heap.iter().map(|(at, kib)| json!([at.round(), kib])).collect::<Vec<_>>(),
        // RM-0134 MX6: the series `judge_footprint` judges.
        "footprint_kib": footprint
            .iter()
            .map(|(at, kib)| json!([at.round(), kib]))
            .collect::<Vec<_>>(),
        "heap_slope_mib_per_min": heap_slopes,
        "heap_kib_per_write": per_write,
        "footprint_slope_mib_per_min": point_slopes(&footprint, secs),
        "drain_ms": spread(&quiet.iter().map(|q| q.drain_ms).collect::<Vec<_>>()),
        "scan_ms": spread(&quiet.iter().map(|q| q.scan_ms).collect::<Vec<_>>()),
    })
}

/// One run on a fresh daemon and `HOME`: its report and what broke.
fn soak_run(
    settings: &Settings,
    tree: &Tree,
    corpus: &Corpus,
    daemon: &Path,
    run_no: usize,
    template: Option<(&Path, usize)>,
    files: &[(String, u64)],
) -> (Value, Vec<String>) {
    let variant = settings.variant;
    let secs = settings.secs;
    let macos = cfg!(target_os = "macos");
    let label = format!("wm-{}-run{run_no}", variant.name());
    let extra: Vec<(String, String)> = if settings.heap {
        vec![("MallocStackLogging".to_owned(), "lite".to_owned())]
    } else {
        Vec::new()
    };
    let home = tree.scratch.join(&format!("hub-home-{run_no}"));
    // The questions the template holds: the run's pending count at its start.
    let prefilled = template.map_or(0, |(_, stored)| stored);
    if variant == Variant::RF {
        let free = free_bytes(&tree.scratch.0);
        assert!(
            free.is_some_and(|free| free >= RF_FREE_MIN),
            "RF23: {} has {} bytes free, under {RF_FREE_MIN} (6 GiB): the RF run refuses to start",
            tree.scratch.0.display(),
            free.map_or_else(|| "unknown".to_owned(), |free| free.to_string())
        );
    }
    if let Some((template, _)) = template {
        copy_tree(template, &home);
    }
    let hub = Hub::start_with(&tree.root, &home, &tree.git, secs + 900, &extra);
    let targets = if variant == Variant::U {
        targets(&hub, &tree.slug, corpus)
    } else {
        Vec::new()
    };
    let shared = Shared::default();
    let at_start = process_facts(hub.pid);
    let start = Instant::now();
    let run = Run {
        hub: &hub,
        slug: &tree.slug,
        shared: &shared,
        start,
        deadline: start + Duration::from_secs(secs),
        variant,
        reads: settings.reads,
        prefill: prefilled,
        files,
    };
    thread::scope(|scope| {
        if variant == Variant::RF {
            for at in 0..RF_UPLOADERS {
                let run = &run;
                scope.spawn(move || rf_uploader(run, at));
            }
            scope.spawn(|| rf_page_reader(&run));
        }
        for at in 0..variant.writers() {
            let (run, targets) = (&run, &targets);
            scope.spawn(move || writer(run, tree, targets, at));
        }
        for at in 0..variant.readers() {
            let run = &run;
            scope.spawn(move || reader(run, corpus, at));
        }
        scope.spawn(|| sampler(&run, secs));
        if macos {
            scope.spawn(|| quiet_probe(&run));
        }
        scope.spawn(|| watchdog(&run, &label));
        if settings.heap {
            scope.spawn(|| {
                while run.running() && run.start.elapsed().as_secs_f64() < HEAP_FIRST_AT {
                    thread::sleep(Duration::from_millis(200));
                }
                if run.running() {
                    let snapshot = heap_snapshot(hub.pid, &format!("{label}-minute1"));
                    run.shared.heap.lock().expect("heap").push(snapshot);
                }
            });
        }
    });
    let at_end = process_facts(hub.pid);
    if settings.heap {
        let snapshot = heap_snapshot(hub.pid, &format!("{label}-end"));
        shared.heap.lock().expect("heap").push(snapshot);
    }
    thread::sleep(Duration::from_millis(500));
    let lines = hub.requests();
    drop(hub);
    let rf_broken = if variant == Variant::RF {
        rf_verdict(&shared, &home, &tree.slug, files)
    } else {
        Vec::new()
    };

    let calls = shared.calls.lock().expect("calls").clone();
    let samples = shared.samples.lock().expect("samples").clone();
    let minutes = secs as f64 / 60.0;
    let third = secs as f64 / 3.0;
    let writes: Vec<&Call> = calls.iter().filter(|call| call.write).collect();
    let reads: Vec<&Call> = calls.iter().filter(|call| !call.write).collect();
    let judged_minutes = (secs as f64 - third) / 60.0;
    let judged_writes = writes.iter().filter(|call| call.at >= third).count() as f64;
    let judged_calls = calls.iter().filter(|call| call.at >= third).count() as f64;
    let footprint = slopes(&samples, secs, |sample| sample.footprint_kib);
    let quiet = shared.quiet.lock().expect("quiet").clone();
    let rss = slopes(&samples, secs, |sample| Some(sample.rss_kib));
    let (judged_slope, judged_broken) = judge(
        &samples,
        &quiet,
        secs,
        macos,
        variant.budget().filter(|_| settings.judged()),
    );
    let (footprint_slope, footprint_broken) = judge_footprint(
        &quiet,
        secs,
        macos,
        variant.footprint_budget().filter(|_| settings.judged()),
    );
    let per = |slope: &Value, count: f64| -> Option<f64> {
        let slope = slope["judged"].as_f64()?;
        (count > 0.0).then(|| rounded(slope * 1024.0 / (count / judged_minutes)))
    };
    let write_lines: Vec<_> = lines
        .iter()
        .filter(|line| {
            matches!(
                line.tool.as_str(),
                "ask_question" | "propose_change" | "add_comment"
            )
        })
        .collect();
    let mut write_index: BTreeMap<String, usize> = BTreeMap::new();
    for line in &write_lines {
        *write_index.entry(line.index.clone()).or_default() += 1;
    }
    let pick = |from: f64| {
        samples
            .iter()
            .find(|sample| sample.at >= from)
            .copied()
            .map(Sample::json)
    };
    let mut broken = shared.failures.lock().expect("failures").clone();
    broken.extend(rf_broken.iter().cloned());
    broken.extend(judged_broken);
    broken.extend(footprint_broken);
    if variant.writers() > 0 && writes.is_empty() {
        broken.push("no write ran".to_owned());
    }
    if variant.readers() > 0 && reads.is_empty() {
        broken.push("no read ran".to_owned());
    }
    let inbox_reads = shared.inbox_reads.lock().expect("inbox reads").clone();
    let timeline = shared.timeline.lock().expect("timeline").clone();
    let stored_now = shared.stored.load(Ordering::SeqCst);
    let stored_count = writes.iter().filter(|call| call.stored).count();
    if variant == Variant::H && stored_count > 1 {
        broken.push(format!(
            "H stored {stored_count} items: the repeat must hit, not store"
        ));
    }
    let mut report = json!({
        "test": "write_soak",
        "variant": variant.name(),
        "run": run_no,
        "seconds": secs,
        "nodes": NODES,
        "writers": variant.writers(),
        "readers": variant.readers(),
        "smoke": settings.smoke,
        "heap_mode": settings.heap,
        "calls": calls.len(),
        "calls_per_min": rounded(calls.len() as f64 / minutes),
        "writes": writes.len(),
        "writes_per_min": rounded(writes.len() as f64 / minutes),
        "reads": reads.len(),
        "reads_per_min": rounded(reads.len() as f64 / minutes),
        "answered": calls.iter().filter(|call| call.ok).count(),
        "stored": writes.iter().filter(|call| call.stored).count(),
        "hits": writes.iter().filter(|call| call.hit).count(),
        "ms": spread(&calls.iter().map(|call| call.ms).collect::<Vec<_>>()),
        "write_ms": (!writes.is_empty())
            .then(|| spread(&writes.iter().map(|call| call.ms).collect::<Vec<_>>())),
        "read_ms": (!reads.is_empty())
            .then(|| spread(&reads.iter().map(|call| call.ms).collect::<Vec<_>>())),
        "samples": {
            "count": samples.len(),
            "first": samples.first().copied().map(Sample::json),
            "middle": pick(secs as f64 / 2.0),
            "judged_from": pick(third),
            "last": samples.last().copied().map(Sample::json),
        },
        "judged_on": judged_on(macos),
        "judged": settings.judged(),
        "slope_mib_per_min": judged_slope,
        "budget_mib_per_min": variant.budget(),
        "footprint_slope_mib_per_min": footprint,
        "quiet": quiet_report(&quiet, secs, macos, judged_writes, judged_minutes),
        "rss_slope_mib_per_min": rss,
        // `null` with no writer (R).
        "kib_per_write": (variant.writers() > 0).then(|| json!({
            "footprint": per(&footprint, judged_writes),
            "rss": per(&rss, judged_writes),
        })),
        "kib_per_call": {
            "footprint": per(&footprint, judged_calls),
            "rss": per(&rss, judged_calls),
        },
        "process": {
            "start": at_start,
            "middle": shared.middle.lock().expect("middle").clone(),
            "end": at_end,
        },
        "binary": binary_facts(daemon),
        "lines": {
            "writes": write_lines.len(),
            "write_index": write_index,
            "write_wait_ms": spread(&write_lines.iter().filter_map(|line| line.wait_ms).collect::<Vec<_>>()),
            "write_run_ms": spread(&write_lines.iter().filter_map(|line| line.run_ms).collect::<Vec<_>>()),
            "write_git_ms": spread(&write_lines.iter().filter_map(|line| line.git_ms).collect::<Vec<_>>()),
            "write_git": spread(&write_lines.iter().filter_map(|line| line.git).collect::<Vec<_>>()),
        },
        "hangs": shared.hangs.load(Ordering::SeqCst),
        "hang_samples": shared.hang_samples.lock().expect("samples").clone(),
        "heap": settings.heap.then(|| shared.heap.lock().expect("heap").clone()),
        "broken": broken.iter().take(20).collect::<Vec<_>>(),
        "broken_count": broken.len(),
    });
    // RM-0134 (beside the macro's recursion limit): the reads cycled, the
    // pending count, each inbox read's bytes, and RSS and the footprint
    // beside the pending count every TIMELINE_EVERY.
    report["reads_mix"] = json!(settings.reads.names());
    // CM19: the comments the run started over and the cards they sit on.
    report["references"] = if variant == Variant::RF {
        let pages = shared.page_ms.lock().expect("pages").clone();
        let (before, after) = page_phases(&pages);
        // The page reader's own daemon lines (`GET …/cards` is its alone;
        // its reads alternate cards, comments), split where its reads cross
        // RF_BASELINE: the turn's wait beside the handler's run tells a held
        // turn from a busy machine. Reported, not judged.
        let line = RF_BASELINE.as_secs_f64();
        let cards_before = pages.iter().step_by(2).filter(|(at, _)| *at < line).count();
        let cards: Vec<&library_common::Line> = lines
            .iter()
            .filter(|line| line.method == "GET" && line.route == "/api/projects/{p}/cards")
            .collect();
        let phase = |part: &[&library_common::Line]| {
            json!({
                "count": part.len(),
                "ms": spread(&part.iter().map(|line| line.ms).collect::<Vec<_>>()),
                "wait_ms": spread(&part.iter().filter_map(|line| line.wait_ms).collect::<Vec<_>>()),
                "run_ms": spread(&part.iter().filter_map(|line| line.run_ms).collect::<Vec<_>>()),
            })
        };
        let split = cards_before.min(cards.len());
        // Every kind of request by phase (its line's `at=` against the
        // first line's, before or after RF_BASELINE): who waits for the
        // turn and who holds it. Reported, not judged.
        let seconds_of = |line: &library_common::Line| -> Option<f64> {
            let at = line
                .raw
                .split(' ')
                .find_map(|pair| pair.strip_prefix("at="))?;
            let time = at.split_once('T')?.1.trim_end_matches('Z');
            let mut parts = time.split(':');
            let hours: f64 = parts.next()?.parse().ok()?;
            let minutes: f64 = parts.next()?.parse().ok()?;
            let seconds: f64 = parts.next()?.parse().ok()?;
            Some(hours * 3600.0 + minutes * 60.0 + seconds)
        };
        let first = lines
            .iter()
            .filter_map(seconds_of)
            .fold(f64::INFINITY, f64::min);
        let mut kinds: BTreeMap<String, [Vec<&library_common::Line>; 2]> = BTreeMap::new();
        for request in &lines {
            let Some(at) = seconds_of(request) else {
                continue;
            };
            let kind = if request.tool == "-" {
                format!("{} {}", request.method, request.route)
            } else {
                format!("mcp {}", request.tool)
            };
            kinds.entry(kind).or_default()[usize::from(at - first >= line)].push(request);
        }
        let by_phase: serde_json::Map<String, Value> = kinds
            .iter()
            .map(|(kind, [early, late])| {
                (
                    kind.clone(),
                    json!({"before": phase(early), "after": phase(late)}),
                )
            })
            .collect();
        json!({
            "upload_mib_s": rf_upload_mibs(),
            "ranges_per_s": rf_ranges_per_s(),
            // A pace set by hand: a diagnosis, never RF23's evidence.
            "overridden": rf_upload_knob().1 || rf_ranges_knob().1,
            "lines_by_phase": by_phase,
            "files": files.len(),
            "file_bytes": RF_FILE_BYTES,
            "upload_bytes": RF_UPLOAD_BYTES,
            "uploads": shared.uploads.load(Ordering::SeqCst),
            "ranges": shared.ranges.load(Ordering::SeqCst),
            "page_p50_before_ms": quantile_of(&before, 0.50),
            "page_p50_after_ms": quantile_of(&after, 0.50),
            "page_p95_before_ms": quantile_of(&before, 0.95),
            "page_p95_after_ms": quantile_of(&after, 0.95),
            "page_reads": pages.len(),
            "page_cards_lines": {
                "before": phase(&cards[..split]),
                "after": phase(&cards[split..]),
            },
            "broken": rf_broken,
        })
    } else {
        Value::Null
    };
    report["comments"] = if matches!(variant, Variant::CM | Variant::RF) {
        json!({"prefilled": CM_PREFILL, "cards": CM_CARDS, "people": CM_PEOPLE})
    } else {
        Value::Null
    };
    // RM-0134 MX6: the second judge, macOS's (`null` elsewhere).
    report["quiet_footprint_slope_mib_per_min"] = json!(footprint_slope);
    report["quiet_footprint_budget_mib_per_min"] =
        json!(variant.footprint_budget().filter(|_| macos));
    report["pending"] = json!({"start": prefilled, "end": prefilled + stored_now});
    report["inbox"] = if inbox_reads.is_empty() {
        Value::Null
    } else {
        json!({
            "count": inbox_reads.len(),
            "bytes_first": inbox_reads.first().map(|read| read.1),
            "bytes_last": inbox_reads.last().map(|read| read.1),
            "bytes_max": inbox_reads.iter().map(|read| read.1).max(),
            "ms": spread(&inbox_reads.iter().map(|read| read.2).collect::<Vec<_>>()),
        })
    };
    report["timeline"] = Value::Array(
        timeline
            .iter()
            .map(|(at, rss, footprint, pending)| json!([at.round(), rss, footprint, pending]))
            .collect(),
    );
    println!("{report}");
    (report, broken)
}

/// WM1-WM4: the write soak of the variant `WRITE_SOAK_VARIANT` names.
#[test]
#[ignore = "WM1-WM4 [soak], release, about WRITE_SOAK_RUNS x (SOAK_SECS + 20) s plus seeding: \
            cargo build --release -p specengine-cli -p specengine-http --bins, then \
            WRITE_SOAK_VARIANT=W cargo nextest run -p specengine-eval --release --test \
            write_soak --run-ignored only --no-capture"]
fn write_soak() {
    let settings = Settings::from_env();
    let daemon = fresh_binaries();
    let tree = Tree::new("write-soak", NODES, COMMITS);
    let corpus = Corpus::of(&tree);
    let mut files = Vec::new();
    let template = if matches!(settings.variant, Variant::CM | Variant::RF) {
        assert_eq!(
            settings.pending, 0,
            "WRITE_SOAK_PENDING is W's and M's: CM and RF fill their own comments"
        );
        let home = comments_prefill(&tree);
        if settings.variant == Variant::RF {
            files = references_prefill(&tree, &home);
        }
        Some((home, 0))
    } else {
        (settings.pending > 0).then(|| prefill(&tree, settings.pending))
    };
    let template = template
        .as_ref()
        .map(|(home, stored)| (home.as_path(), *stored));
    let mut all = Vec::new();
    for run_no in 1..=settings.runs {
        let (_, broken) = soak_run(&settings, &tree, &corpus, &daemon, run_no, template, &files);
        all.extend(
            broken
                .into_iter()
                .map(|what| format!("{} run {run_no}: {what}", settings.variant.name())),
        );
    }
    assert!(
        all.is_empty(),
        "the write soak's budgets ({} failures): {:#?}",
        all.len(),
        all.iter().take(40).collect::<Vec<_>>()
    );
}

/// WM4 alone on this repository: the refusal [`write_soak`] runs first,
/// without seeding anything (seconds, not minutes).
#[test]
#[ignore = "WM4 [stale binary], release: cargo nextest run -p specengine-eval --release \
            --test write_soak wm4_the_release_binaries --run-ignored only"]
fn wm4_the_release_binaries_are_fresh() {
    let daemon = fresh_binaries();
    println!("fresh: {}", binary_facts(&daemon));
}

/// A variant's bars, MiB per minute: the live heap's (WM6; Linux RSS) and
/// the footprint's at the quiet moments (MX6, macOS); `None`: not judged.
type Bars = (Option<f64>, Option<f64>);

/// One "Memory bar" cell's bars: parts split on `;`, each `reported only`
/// or `<number>, <measure>`, the measure starting `live heap` or
/// `footprint`, each at most once.
fn cell_bars(cell: &str) -> Bars {
    let (mut live, mut footprint) = (None, None);
    for part in cell.split(';').map(str::trim) {
        if part.starts_with("reported only") {
            continue;
        }
        let (number, measure) = part
            .split_once(',')
            .unwrap_or_else(|| panic!("a bar `<number>, <measure>`: {part:?}"));
        let value = number
            .trim()
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("a bar {part:?} in {cell:?}"));
        let measure = measure.trim();
        let slot = if measure.starts_with("live heap") {
            &mut live
        } else if measure.starts_with("footprint") {
            &mut footprint
        } else {
            panic!("{part:?}: a bar of the live heap or of the footprint");
        };
        assert!(
            slot.replace(value).is_none(),
            "two bars of one measure: {cell:?}"
        );
    }
    (live, footprint)
}

/// The `write-memory` rows of a canon's "Budgets" table: each variant its
/// bars ([`cell_bars`] of the "Memory bar" column). A row's first cell is
/// `write-memory <V>[, <V>…]`, optionally followed by ` over <N> pending`
/// (a prefilled run of the same variants): a variant in two rows holds one
/// pair of bars.
fn canon_budgets(text: &str) -> BTreeMap<String, Bars> {
    let section = text
        .split("\n## ")
        .find(|part| part.starts_with("Budgets\n"))
        .expect("a \"Budgets\" section");
    let rows: Vec<Vec<String>> = section
        .lines()
        .filter(|line| line.starts_with('|'))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect();
    let header = rows.first().expect("a table");
    let bar = header
        .iter()
        .position(|cell| cell.starts_with("Memory bar"))
        .unwrap_or_else(|| panic!("no \"Memory bar\" column: {header:?}"));
    let mut out = BTreeMap::new();
    for row in &rows {
        let Some(label) = row[0].strip_prefix("write-memory ") else {
            continue;
        };
        let variants = label.split(" over ").next().unwrap_or(label);
        let bars = cell_bars(&row[bar]);
        for variant in variants.split(',') {
            let variant = variant.trim().to_owned();
            if let Some(previous) = out.insert(variant.clone(), bars) {
                assert_eq!(previous, bars, "{variant} in two rows with other bars");
            }
        }
    }
    out
}

/// WM6 [budget] and RM-0134 MX6 [judge]: the harness's bars are the
/// numbers of `docs/canon/soak-runs.md` "Budgets", its `write-memory` rows,
/// variant by variant: the live heap's (`WRITE_BUDGET_MIB_PER_MIN`,
/// `READ_BUDGET_MIB_PER_MIN`) and the quiet footprint's
/// (`FOOTPRINT_BUDGET_MIB_PER_MIN`), e.g. `0.25, live heap (Linux RSS); 5,
/// footprint at the quiet moments (macOS)` (`WRITE_SOAK_CANON` reads
/// another copy, for the mutation: 6 in the table is red).
#[test]
fn wm6_the_budgets_are_the_canons() {
    let path = std::env::var_os("WRITE_SOAK_CANON").map_or_else(
        || import_support::repository_root().join("docs/canon/soak-runs.md"),
        PathBuf::from,
    );
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let canon = canon_budgets(&text);
    let harness: BTreeMap<String, Bars> = [
        Variant::W,
        Variant::U,
        Variant::R,
        Variant::M,
        Variant::H,
        Variant::CM,
        Variant::RF,
    ]
    .into_iter()
    .map(|variant| {
        (
            variant.name().to_owned(),
            (variant.budget(), variant.footprint_budget()),
        )
    })
    .collect();
    assert_eq!(
        canon,
        harness,
        "{}'s write-memory bars (live heap, footprint) against WRITE_BUDGET_MIB_PER_MIN \
         ({WRITE_BUDGET_MIB_PER_MIN}), READ_BUDGET_MIB_PER_MIN ({READ_BUDGET_MIB_PER_MIN}) and \
         FOOTPRINT_BUDGET_MIB_PER_MIN ({FOOTPRINT_BUDGET_MIB_PER_MIN})",
        path.display()
    );
}

/// The bars' cell grammar: both bars, one, none; a prefilled row's
/// variants; a repeated variant with other bars, an unknown measure and a
/// bar given twice refused.
#[test]
fn mx6_the_canons_cells_read_both_bars() {
    assert_eq!(
        cell_bars("0.25, live heap (Linux RSS); 5, footprint at the quiet moments (macOS)"),
        (Some(0.25), Some(5.0))
    );
    assert_eq!(cell_bars("0.5, live heap (Linux RSS)"), (Some(0.5), None));
    assert_eq!(cell_bars("reported only"), (None, None));
    let table = "x\n## Budgets\n\n| Soak | Test | Memory bar, MiB/min |\n|---|---|---|\n\
                 | write-memory W, M | t | 0.25, live heap (Linux RSS); 5, footprint (macOS) |\n\
                 | write-memory M over 2 000 pending | t | 0.25, live heap; 5, footprint |\n\
                 | write-memory H | t | reported only |\n| DL14 | t | 0.5, RSS |\n\n## Next\n";
    let read = canon_budgets(table);
    assert_eq!(
        read.into_iter().collect::<Vec<_>>(),
        [
            ("H".to_owned(), (None, None)),
            ("M".to_owned(), (Some(0.25), Some(5.0))),
            ("W".to_owned(), (Some(0.25), Some(5.0))),
        ]
    );
    for bad in [
        table.replace(
            "0.25, live heap; 5, footprint |",
            "0.25, live heap; 6, footprint |",
        ),
        table.replace(
            "0.25, live heap; 5, footprint |",
            "0.25, live heap; 5, heap |",
        ),
        table.replace(
            "0.25, live heap; 5, footprint |",
            "0.25, live heap; 5, live heap |",
        ),
    ] {
        assert!(
            std::panic::catch_unwind(|| canon_budgets(&bad)).is_err(),
            "refused: {bad}"
        );
    }
}

/// `vmmap --summary`'s sizes and its MALLOC ZONE total, as the live-heap
/// sampler reads them.
#[test]
fn vmmap_sizes_read() {
    assert_eq!(vmmap_kib("2259K"), Some(2259.0));
    assert_eq!(vmmap_kib("16.5M"), Some(16.5 * 1024.0));
    assert_eq!(vmmap_kib("1G"), Some(1024.0 * 1024.0));
    assert_eq!(vmmap_kib("2048"), Some(2.0));
    assert_eq!(vmmap_kib("-"), None);
}

/// The slopes' arithmetic (no daemon): a level series is 0 in every
/// window, a steady climb its own rate in each third.
#[test]
fn the_slopes_tell_a_plateau_from_a_climb() {
    let level: Vec<Sample> = (0..300)
        .map(|at| Sample {
            at: f64::from(at),
            rss_kib: 100_000,
            footprint_kib: Some(90_000),
        })
        .collect();
    let flat = slopes(&level, 300, |sample| sample.footprint_kib);
    assert_eq!(flat["judged"], json!(0.0), "{flat}");
    // 1 MiB a minute: 1024 KiB / 60 s.
    let climb: Vec<Sample> = (0..300)
        .map(|at| Sample {
            at: f64::from(at),
            rss_kib: 100_000 + u64::try_from(at * 1024 / 60).expect("positive"),
            footprint_kib: None,
        })
        .collect();
    let rising = slopes(&climb, 300, |sample| Some(sample.rss_kib));
    for third in rising["thirds"].as_array().expect("thirds") {
        let slope = third.as_f64().expect("a slope");
        assert!((slope - 1.0).abs() < 0.02, "{rising}");
    }
    // A plateau: climbs over the first third, level after.
    let plateau: Vec<Sample> = (0..300)
        .map(|at| Sample {
            at: f64::from(at),
            rss_kib: 100_000 + u64::try_from(at.min(100) * 1024).expect("positive"),
            footprint_kib: None,
        })
        .collect();
    let leveled = slopes(&plateau, 300, |sample| Some(sample.rss_kib));
    assert!(
        leveled["thirds"][0].as_f64().expect("a slope") > 10.0,
        "{leveled}"
    );
    assert_eq!(leveled["judged"], json!(0.0), "{leveled}");
}

// ------------------------------------------------------------ the judge

/// A 300 s run's quiet readings (macOS): every 30 s from 31 s, nine of
/// them, six from `SOAK_SECS/3` on; the live heap `heap(at)` KiB, the
/// footprint falling 2.5 MiB/min.
fn quiet_series(heap: impl Fn(f64) -> u64) -> Vec<Quiet> {
    (0..9)
        .map(|k| {
            let at = 31.0 + 30.0 * f64::from(k);
            Quiet {
                at,
                heap_kib: heap(at),
                footprint_kib: Some(per_minute(60_000, -2.5, at)),
                drain_ms: 700,
                scan_ms: 400,
            }
        })
        .collect()
}

/// `base` KiB moving `mib_per_min` MiB a minute, at `at` seconds.
fn per_minute(base: u64, mib_per_min: f64, at: f64) -> u64 {
    (base as f64 + mib_per_min * 1024.0 * at / 60.0).round() as u64
}

/// A 300 s run's samples, one a second: RSS moving `rss` MiB/min, the
/// footprint `footprint` MiB/min.
fn page_series(rss: f64, footprint: f64) -> Vec<Sample> {
    (0..300)
        .map(|at| {
            let at = f64::from(at);
            Sample {
                at,
                rss_kib: per_minute(90_000, rss, at),
                footprint_kib: Some(per_minute(60_000, footprint, at)),
            }
        })
        .collect()
}

/// The judge reads the live heap on macOS and RSS elsewhere: a clean W as
/// the diagnosis saw it (the live heap level, RSS climbing 0.5 MiB/min on
/// libmalloc's kept pages, the footprint falling) passes on macOS and is
/// red on RSS; a 2.16 MiB/min leak the falling footprint hides is red on
/// the live heap; unjudged (smoke, heap mode, H) breaks nothing.
#[test]
fn the_judge_reads_the_live_heap_on_macos_and_rss_elsewhere() {
    let budget = Some(WRITE_BUDGET_MIB_PER_MIN);
    let level = quiet_series(|_| 1750);
    let pages = page_series(0.5, -2.5);
    let (slope, broken) = judge(&pages, &level, 300, true, budget);
    assert_eq!((slope, broken.len()), (Some(0.0), 0), "{broken:?}");
    let (slope, broken) = judge(&pages, &level, 300, false, budget);
    assert!(
        slope.is_some_and(|slope| (slope - 0.5).abs() < 0.01),
        "{slope:?}"
    );
    assert!(
        broken.len() == 1 && broken[0].contains("(rss, from SOAK_SECS/3 on) > the budget 0.25"),
        "{broken:?}"
    );
    let leak = quiet_series(|at| per_minute(1750, 2.16, at));
    let (slope, broken) = judge(&page_series(0.0, -2.5), &leak, 300, true, budget);
    assert!(
        slope.is_some_and(|slope| (slope - 2.16).abs() < 0.01),
        "{slope:?}"
    );
    assert!(
        broken.len() == 1 && broken[0].contains("(live_heap, from SOAK_SECS/3 on)"),
        "{broken:?}"
    );
    let (slope, broken) = judge(&pages, &leak, 300, true, None);
    assert!(slope.is_some() && broken.is_empty(), "{broken:?}");
    let (_, broken) = judge(&pages, &[], 300, true, None);
    assert!(broken.is_empty(), "unjudged: {broken:?}");
    assert_eq!((judged_on(true), judged_on(false)), ("live_heap", "rss"));
}

/// Fewer than five quiet readings from `SOAK_SECS/3` on break a judged run
/// on macOS, none at all included (`vmmap` failing); six pass.
#[test]
fn fewer_than_five_judged_quiet_readings_break_the_run() {
    let budget = Some(READ_BUDGET_MIB_PER_MIN);
    let six = quiet_series(|_| 1750);
    assert_eq!(six.iter().filter(|q| q.at >= 100.0).count(), 6);
    let (_, broken) = judge(&[], &six, 300, true, budget);
    assert!(broken.is_empty(), "{broken:?}");
    let four: Vec<Quiet> = six
        .iter()
        .filter(|q| q.at < 100.0 || q.at > 160.0)
        .copied()
        .collect();
    let (_, broken) = judge(&page_series(0.0, 0.0), &four, 300, true, budget);
    assert_eq!(
        broken,
        vec!["4 live_heap readings judged (from SOAK_SECS/3 on): fewer than 5".to_owned()]
    );
    let (slope, broken) = judge(&page_series(0.0, 0.0), &[], 300, true, budget);
    assert_eq!(slope, None);
    assert_eq!(
        broken,
        vec![
            "0 live_heap readings judged (from SOAK_SECS/3 on): fewer than 5".to_owned(),
            "no live_heap slope".to_owned()
        ]
    );
    let (_, broken) = judge(&page_series(0.0, 0.0)[..150], &[], 300, false, budget);
    assert_eq!(
        broken,
        vec!["50 rss readings judged (from SOAK_SECS/3 on): fewer than 60".to_owned()]
    );
}

/// The `quiet` object (WM3): every key on macOS, a run with no reading
/// included; `null` elsewhere. KiB per write is the judged slope over the
/// judged writes per minute.
#[test]
fn the_quiet_object_holds_every_key() {
    let leak = quiet_series(|at| per_minute(1750, 2.16, at));
    // 600 writes a minute over the judged 200 s.
    let report = quiet_report(&leak, 300, true, 2000.0, 200.0 / 60.0);
    for key in [
        "count",
        "heap_kib",
        "footprint_kib",
        "heap_slope_mib_per_min",
        "heap_kib_per_write",
        "drain_ms",
        "scan_ms",
    ] {
        assert!(report.get(key).is_some(), "{key}: {report}");
    }
    assert_eq!(report["count"], json!(9));
    assert_eq!(
        report["heap_kib"][0],
        json!([31.0, per_minute(1750, 2.16, 31.0)])
    );
    assert_eq!(
        report["footprint_kib"][8],
        json!([271.0, per_minute(60_000, -2.5, 271.0)])
    );
    assert_eq!(report["footprint_kib"].as_array().map(Vec::len), Some(9));
    let per_write = report["heap_kib_per_write"].as_f64().expect("a number");
    assert!((per_write - 2.16 * 1024.0 / 600.0).abs() < 0.02, "{report}");
    for third in report["heap_slope_mib_per_min"]["thirds"]
        .as_array()
        .expect("thirds")
    {
        assert!(third.as_f64().is_some_and(|slope| slope > 2.0), "{report}");
    }
    let empty = quiet_report(&[], 300, true, 0.0, 200.0 / 60.0);
    assert_eq!(empty["count"], json!(0));
    assert_eq!(empty["heap_slope_mib_per_min"]["judged"], Value::Null);
    assert_eq!(empty["heap_kib_per_write"], Value::Null);
    assert_eq!(empty["footprint_kib"], json!([]));
    assert_eq!(quiet_report(&leak, 300, false, 2000.0, 3.0), Value::Null);
}

/// [`quiet_series`]'s moments with the live heap level and the footprint
/// moving `mib_per_min`.
fn footprint_series(mib_per_min: f64) -> Vec<Quiet> {
    quiet_series(|_| 1750)
        .into_iter()
        .map(|quiet| Quiet {
            footprint_kib: Some(per_minute(200_000, mib_per_min, quiet.at)),
            ..quiet
        })
        .collect()
}

/// RM-0134 MX6: the footprint at the quiet moments is judged on macOS
/// beside the live heap, against 5 MiB/min, for W, U, R and M (H and an
/// unjudged run report it): RM-0134's M (the live heap level, the footprint
/// +25 MiB/min) passes the live-heap judge and breaks this one; a clean
/// run's falling footprint passes; the bar is inclusive; off macOS it is
/// not read; fewer than five judged readings with a footprint break the
/// run.
#[test]
fn mx6_the_quiet_footprint_is_judged_on_macos() {
    let budget = Some(FOOTPRINT_BUDGET_MIB_PER_MIN);
    let m = footprint_series(25.0);
    let (_, live) = judge(
        &page_series(34.0, 32.0),
        &m,
        300,
        true,
        Some(WRITE_BUDGET_MIB_PER_MIN),
    );
    assert!(live.is_empty(), "the live heap is level: {live:?}");
    let (slope, broken) = judge_footprint(&m, 300, true, budget);
    assert!(
        slope.is_some_and(|slope| (slope - 25.0).abs() < 0.01),
        "{slope:?}"
    );
    assert_eq!(
        broken,
        vec![
            "the footprint grows 25.000 MiB/min (quiet_footprint, from SOAK_SECS/3 on) > the \
             budget 5"
                .to_owned()
        ]
    );
    for clean in [-5.0, -0.1, 0.0, 4.9, 5.0] {
        let (slope, broken) = judge_footprint(&footprint_series(clean), 300, true, budget);
        assert!(
            slope.is_some_and(|slope| (slope - clean).abs() < 0.01) && broken.is_empty(),
            "{clean}: {slope:?} {broken:?}"
        );
    }
    let (_, broken) = judge_footprint(&footprint_series(5.1), 300, true, budget);
    assert_eq!(broken.len(), 1, "{broken:?}");
    assert_eq!(judge_footprint(&m, 300, false, budget), (None, Vec::new()));
    let (slope, broken) = judge_footprint(&m, 300, true, None);
    assert!(slope.is_some() && broken.is_empty(), "unjudged: {broken:?}");
    let blind: Vec<Quiet> = m
        .iter()
        .map(|quiet| Quiet {
            footprint_kib: (quiet.at < 100.0 || quiet.at > 160.0).then_some(0),
            ..*quiet
        })
        .collect();
    let (_, broken) = judge_footprint(&blind, 300, true, budget);
    assert_eq!(
        broken.first().map(String::as_str),
        Some("4 quiet_footprint readings judged (from SOAK_SECS/3 on): fewer than 5")
    );
    let (_, broken) = judge_footprint(&[], 300, true, budget);
    assert_eq!(
        broken,
        vec![
            "0 quiet_footprint readings judged (from SOAK_SECS/3 on): fewer than 5".to_owned(),
            "no quiet_footprint slope".to_owned()
        ]
    );
    for variant in [Variant::W, Variant::U, Variant::R, Variant::M] {
        assert_eq!(variant.footprint_budget(), Some(5.0), "{variant:?}");
    }
    assert_eq!(Variant::H.footprint_budget(), None);
}

// ------------------------------------------------------------ WM4

/// A scratch workspace for WM4, removed on drop: `specengine-model` (no
/// dependency), `specengine-cli` on it, `specengine-mcp` on cli,
/// `specengine-http` on cli and mcp (and `specengine-eval` as a dev
/// dependency only), `specengine-eval` on nothing; `Cargo.lock`, `ui/dist`;
/// `release/spec` and `release/specengine-http`. Sources at [`Self::BASE`],
/// the binaries 100 s later.
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    const BASE: u64 = 1_790_000_000;
    const SOURCES: [&str; 7] = [
        "crates/specengine-model/src/lib.rs",
        "crates/specengine-cli/src/main.rs",
        "crates/specengine-mcp/src/lib.rs",
        "crates/specengine-http/src/main.rs",
        "crates/specengine-eval/src/lib.rs",
        "Cargo.lock",
        "ui/dist/assets/index.js",
    ];

    fn new(label: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("specengine-wm4-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let manifests = [
            ("specengine-model", ""),
            (
                "specengine-cli",
                "[dependencies]\nspecengine-model = { path = \"../specengine-model\" }\n",
            ),
            (
                "specengine-mcp",
                "[dependencies]\nspecengine-cli = { path = \"../specengine-cli\" }\n",
            ),
            (
                "specengine-http",
                "[dependencies]\nspecengine-cli = { path = \"../specengine-cli\" }\n\
                 specengine-mcp = { path = \"../specengine-mcp\" }\n\
                 [dev-dependencies]\nspecengine-eval = { path = \"../specengine-eval\" }\n",
            ),
            ("specengine-eval", ""),
        ];
        for (name, deps) in manifests {
            let dir = root.join("crates").join(name);
            fs::create_dir_all(dir.join("src")).expect("a crate");
            fs::write(
                dir.join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\n{deps}"),
            )
            .expect("a manifest");
        }
        let workspace = Self { root };
        for relative in Self::SOURCES
            .iter()
            .chain(&["release/spec", "release/specengine-http"])
        {
            let path = workspace.root.join(relative);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(&path, relative).expect("a file");
        }
        workspace.reset();
        workspace
    }

    /// `relative`'s mtime set `secs` past [`Self::BASE`].
    fn at(&self, relative: &str, secs: u64) {
        fs::File::options()
            .write(true)
            .open(self.root.join(relative))
            .and_then(|file| file.set_modified(UNIX_EPOCH + Duration::from_secs(Self::BASE + secs)))
            .unwrap_or_else(|error| panic!("{relative}: {error}"));
    }

    /// Every source at the base, the binaries 100 s later.
    fn reset(&self) {
        for relative in Self::SOURCES {
            self.at(relative, 0);
        }
        self.at("release/spec", 100);
        self.at("release/specengine-http", 100);
    }

    /// The binaries refused, by name, and the refusals.
    fn refused(&self) -> (Vec<&'static str>, Vec<String>) {
        let release = self.root.join("release");
        let refusals = stale_binaries(&self.root, &release);
        let names = BINARIES
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| {
                let path = release.join(name).display().to_string();
                refusals.iter().any(|refusal| {
                    refusal.starts_with(&format!("{path} "))
                        || refusal.starts_with(&format!("{path}:"))
                })
            })
            .collect();
        (names, refusals)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// WM4: each binary is held to its own sources. An http-only change (the
/// daemon's crate, a crate only it links, `ui/dist`) refuses the daemon
/// alone; a crate both link, or `Cargo.lock`, refuses both; a crate
/// neither links (a dev dependency included) refuses none; a missing
/// binary is refused. Each refusal names the binary, the newer file and
/// [`BUILD`].
/// M (the old rule, every crate for every binary): the http-only change
/// still refuses `spec`, and the unlinked crate refuses both: red.
#[test]
fn wm4_each_binary_is_held_to_its_own_sources() {
    let workspace = Workspace::new("own-sources");
    assert_eq!(workspace.refused(), (Vec::new(), Vec::new()), "fresh");
    for (changed, expected) in [
        (
            "crates/specengine-http/src/main.rs",
            &["specengine-http"][..],
        ),
        ("crates/specengine-mcp/src/lib.rs", &["specengine-http"]),
        ("ui/dist/assets/index.js", &["specengine-http"]),
        (
            "crates/specengine-cli/src/main.rs",
            &["spec", "specengine-http"],
        ),
        (
            "crates/specengine-model/src/lib.rs",
            &["spec", "specengine-http"],
        ),
        ("Cargo.lock", &["spec", "specengine-http"]),
        ("crates/specengine-eval/src/lib.rs", &[]),
    ] {
        workspace.reset();
        workspace.at(changed, 110);
        let (names, refusals) = workspace.refused();
        assert_eq!(names, expected, "after {changed}: {refusals:#?}");
        for refusal in &refusals {
            assert!(
                refusal.contains(&format!(
                    "is older than {}",
                    workspace.root.join(changed).display()
                )) && refusal.ends_with(&format!("rebuild: {BUILD}")),
                "{refusal}"
            );
        }
    }
    workspace.reset();
    fs::remove_file(workspace.root.join("release/spec")).expect("remove");
    let (names, refusals) = workspace.refused();
    assert_eq!(names, ["spec"], "{refusals:#?}");
    assert!(
        refusals[0].contains("no release build") && refusals[0].ends_with(BUILD),
        "{refusals:#?}"
    );
}

/// WM4 on this repository's manifests: `spec` is built from none of the
/// daemon's own crates, so cargo does not relink it after an http-only
/// change; the daemon is built from everything `spec` is.
#[test]
fn wm4_spec_is_not_built_from_the_daemons_crates() {
    let crates = import_support::repository_root().join("crates");
    let spec = crate_closure(&crates, "specengine-cli");
    let daemon = crate_closure(&crates, "specengine-http");
    for own in ["specengine-http", "specengine-mcp"] {
        assert!(
            !spec.contains(own) && daemon.contains(own),
            "{own}: {spec:?} {daemon:?}"
        );
    }
    assert!(spec.is_subset(&daemon), "{spec:?} {daemon:?}");
    assert!(spec.contains("specengine-store"), "{spec:?}");
}

/// RM-0134's `WRITE_SOAK_READS`: unset or empty, all three reads; a list
/// names its own, in the reader's order; an unknown word is refused.
#[test]
fn the_reads_knob_names_what_readers_cycle() {
    assert_eq!(Reads::parse(None), Ok(Reads::ALL));
    assert_eq!(Reads::parse(Some(" ")), Ok(Reads::ALL));
    assert_eq!(
        Reads::parse(Some("search, node")).map(Reads::names),
        Ok(vec!["node", "search"])
    );
    assert_eq!(
        Reads::parse(Some("inbox")).map(Reads::names),
        Ok(vec!["inbox"])
    );
    assert!(
        Reads::parse(Some("node,cards"))
            .is_err_and(|error| error.contains("\"cards\" is none of node, search, inbox"))
    );
}

/// docs/features/comments.md CM19: the `CM` variant writes with 8 logins
/// beside 10 readers and is judged as W, both bars; its cards are the 20 a
/// fresh queue numbers.
#[test]
fn cm19_the_comments_variant_is_judged_as_w() {
    let cm = Variant::CM;
    assert_eq!(cm.name(), "CM");
    assert_eq!(cm.writers(), WRITERS);
    assert_eq!(cm.readers(), READERS);
    assert_eq!(cm.budget(), Some(WRITE_BUDGET_MIB_PER_MIN));
    assert_eq!(cm.footprint_budget(), Some(FOOTPRINT_BUDGET_MIB_PER_MIN));
    assert_eq!(cm_card(0), "C-0001");
    assert_eq!(cm_card(19), "C-0020");
    assert_eq!(cm_card(20), "C-0001");
    assert_eq!(CM_PREFILL % CM_PEOPLE, 0, "each person posts as many");
    assert_eq!((CM_CARDS, CM_PEOPLE, CM_PREFILL), (20, 10, 2_000));
}
