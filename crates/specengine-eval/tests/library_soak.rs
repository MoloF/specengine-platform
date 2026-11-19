//! docs/features/doc-library.md (RM-0127) "Acceptance criteria":
//!
//! - DL14 [soak]: library reads do not grow the daemon's memory. The tree
//!   is DC-11's at `--nodes 10000` (`library_common`); the daemon the
//!   release `specengine-http`, a hub over it with `--public-origin`,
//!   `--port 0`, `--slow-ms 0` and a scratch `HOME`. For `SOAK_SECS`
//!   (default 300, never less): 10 logins each read `GET …/library` back to
//!   back; every 10 s a commit to one part file in the served checkout, so
//!   new heads keep the cache full and churning; every 1 s a sample of the
//!   daemon's RSS (`ps -o rss= -p <pid>`). Budgets, each failing the run:
//!   the Theil-Sen slope of the samples from `SOAK_SECS/3` on at most 0.5
//!   MiB per minute; no read taking 10 000 ms or more (the client's clock
//!   and the line's `ms`) or answering other than 200; no hang (a request
//!   unanswered for 30 s runs `sample <pid> 5` on macOS into a file kept in
//!   the temp directory, prints its path, then the run fails);
//!   `index=walked` lines at most 1 + ⌊`SOAK_SECS`/300⌋ (a hand commit
//!   makes no library read walk: the read re-reads no file's bytes). The
//!   report is one stdout JSON line: the reads, `ms` and `run_ms` p50, p95,
//!   p99 and worst, the slope, the first and last RSS, the heads committed,
//!   the hangs. Two runs, each with a fresh daemon (and `HOME`), must both
//!   stay within budget. M: the cache's eviction removed. The judged window
//!   is readers and commits alone; after it, on the same hub, a 60 s probe
//!   phase adds one `ask_question` by `writer` 1 s after each commit, its
//!   numbers reported apart (`probe_phase_reported_only`: the writer's
//!   `wait_ms` for the turn, the first read at each new `HEAD`, the one
//!   that walks git inside the turn), never judged for memory; the 10 s bar,
//!   the 200 and the hang watchdog hold there too (`page()` fails a probe
//!   read at 10 000 ms or more).
//! - DL15 [measure]: the page under agents' writes, in TF14's shape: for
//!   120 s at `--nodes 10000`, 10 library readers, 8 logins each raising
//!   `ask_question` over `/mcp/<slug>` back to back (each text fresh
//!   syllables, so it has no hit), one login reading `GET …/inbox` every
//!   5 s. Every page request (library and inbox) answers 200 with `ms` under
//!   10 000, every call answers, nothing hangs. Memory is reported, not
//!   judged.
//!
//! Run: `cargo build --release -p specengine-cli -p specengine-http
//! --bins` (`--bin spec` with both packages builds no daemon: the spec's
//! command left `specengine-http` stale), then `cargo nextest run -p
//! specengine-eval --release --test library_soak --run-ignored only
//! --no-capture` (both, in turn: `--no-capture` runs them serially;
//! `SOAK_SECS=600` for a longer soak; one alone: append `dl14_` or
//! `dl15_`). About 2 × (`SOAK_SECS` + 60) + 120 s plus the seeding of the
//! tree for each test.
//!
//! The daemon lives under `perl -e 'alarm N'` (N past the run) and is
//! killed and reaped on drop; nothing outlives the test. Every run writes
//! under its scratch directory, removed on drop, but a hang's sample file.

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;
mod library_common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use library_common::{
    COMMITS, Hub, INBOX_TEMPLATE, InFlight, LIBRARY_TEMPLATE, Line, Tree, mcp_call, nonce_words,
    page_get, spread, theil_sen_mib_per_minute,
};
use serde_json::{Value, json};

const NODES: usize = 10_000;
const DEFAULT_SOAK_SECS: u64 = 300;
/// DL14's memory budget, MiB per minute.
const SLOPE_BAR: f64 = 0.5;
/// No page request takes this long.
const READ_BAR_MS: u64 = 10_000;
/// A request unanswered this long is a hang.
const HANG: Duration = Duration::from_secs(30);
/// The client's own give-up, past the hang's.
const READ_TIMEOUT: Duration = Duration::from_secs(120);
const READERS: usize = 10;
const ASKERS: usize = 8;
const COMMIT_EVERY: Duration = Duration::from_secs(10);
const SAMPLE_EVERY: Duration = Duration::from_secs(1);
const INBOX_EVERY: Duration = Duration::from_secs(5);
/// DL14's probe write, this long after each commit.
const PROBE_AFTER: Duration = Duration::from_secs(1);
/// DL14's probe phase, after the judged window.
const PROBE_SECS: u64 = 60;
const DL15_SECS: u64 = 120;

/// `SOAK_SECS`, default 300, never less.
fn soak_secs() -> u64 {
    let secs = std::env::var("SOAK_SECS")
        .ok()
        .map_or(DEFAULT_SOAK_SECS, |value| {
            value
                .parse()
                .unwrap_or_else(|_| panic!("SOAK_SECS={value:?}: seconds"))
        });
    assert!(
        secs >= DEFAULT_SOAK_SECS,
        "SOAK_SECS={secs}: DL14 soaks {DEFAULT_SOAK_SECS} s at least"
    );
    secs
}

/// What every thread of one run shares.
#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    hangs: AtomicUsize,
    heads: AtomicUsize,
    calls: AtomicU64,
    in_flight: InFlight,
    /// Each page request: (what, status or 0, the client's ms).
    pages: Mutex<Vec<(String, u16, u64)>>,
    /// Each MCP call: (answered, stored, its error if any).
    asks: Mutex<Vec<(bool, bool, Option<String>)>>,
    /// Each MCP call's client ms.
    write_ms: Mutex<Vec<u64>>,
    /// `(seconds since start, RSS KiB)`.
    rss: Mutex<Vec<(f64, u64)>>,
    failures: Mutex<Vec<String>>,
    samples: Mutex<Vec<String>>,
}

impl Shared {
    fn fail(&self, what: String) {
        self.failures.lock().expect("failures").push(what);
    }

    fn running(&self, deadline: Instant) -> bool {
        !self.stop.load(Ordering::SeqCst) && Instant::now() < deadline
    }
}

/// A page `GET path` by `login`, timed and tracked.
fn page(hub: &Hub, shared: &Shared, path: &str, login: &str) {
    let id = shared.in_flight.begin(&format!("GET {path} by {login}"));
    let answer = page_get(hub.port, path, login, READ_TIMEOUT);
    shared.in_flight.end(id);
    match answer {
        Ok(answer) => {
            let ms = u64::try_from(answer.elapsed.as_millis()).unwrap_or(u64::MAX);
            if answer.status != 200 || ms >= READ_BAR_MS {
                shared.fail(format!(
                    "GET {path} by {login}: {} in {ms} ms: {}",
                    answer.status,
                    answer.text().chars().take(300).collect::<String>()
                ));
            }
            shared
                .pages
                .lock()
                .expect("pages")
                .push((path.to_owned(), answer.status, ms));
        }
        Err(error) => {
            shared.fail(format!("GET {path} by {login}: no answer: {error}"));
            shared.pages.lock().expect("pages").push((
                path.to_owned(),
                0,
                READ_TIMEOUT.as_millis() as u64,
            ));
        }
    }
}

/// The hang watchdog: a request open for [`HANG`] samples the daemon once,
/// prints the file's path and stops the run.
fn watchdog(hub: &Hub, shared: &Shared, deadline: Instant, label: &str) {
    let mut seen = std::collections::BTreeSet::new();
    while shared.running(deadline + HANG) {
        for (id, what) in shared.in_flight.older_than(HANG) {
            if seen.insert(id) {
                shared.hangs.fetch_add(1, Ordering::SeqCst);
                let sample = hub.sample(label);
                let place = sample.as_ref().map_or_else(
                    || "no sample (not macOS)".to_owned(),
                    |path| path.display().to_string(),
                );
                println!("HANG {label}: {what} open {HANG:?}; sample {place}");
                shared.samples.lock().expect("samples").push(place);
                shared.fail(format!("a hang: {what}"));
                shared.stop.store(true, Ordering::SeqCst);
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// The RSS sampler, every [`SAMPLE_EVERY`].
fn sampler(hub: &Hub, shared: &Shared, start: Instant, deadline: Instant) {
    while shared.running(deadline) {
        if let Some(kib) = hub.rss_kib() {
            shared
                .rss
                .lock()
                .expect("rss")
                .push((start.elapsed().as_secs_f64(), kib));
        }
        thread::sleep(SAMPLE_EVERY);
    }
}

/// Sleeps `period` in short steps while the run runs; whether it still runs.
fn wait(shared: &Shared, deadline: Instant, period: Duration) -> bool {
    let until = Instant::now() + period;
    while Instant::now() < until {
        if !shared.running(deadline) {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    shared.running(deadline)
}

/// The library's request lines and the run's numbers, checked against the
/// lines' own `ms`.
fn library_lines(hub: &Hub, shared: &Shared) -> Vec<Line> {
    thread::sleep(Duration::from_secs(1));
    let lines: Vec<Line> = hub
        .requests()
        .into_iter()
        .filter(|line| line.route == LIBRARY_TEMPLATE || line.route == INBOX_TEMPLATE)
        .collect();
    for line in &lines {
        if line.ms >= READ_BAR_MS || line.status != "200" {
            shared.fail(format!("a page line: {}", line.raw));
        }
    }
    lines
}

/// One `ask_question` by `login` over `/mcp/<slug>`, its text fresh
/// syllables from `seed`, tracked and recorded: answered, stored, a tool
/// error; the client's ms.
fn ask(hub: &Hub, shared: &Shared, tree: &Tree, login: &str, seed: u64) {
    let call = shared.calls.fetch_add(1, Ordering::SeqCst);
    let arguments = json!({
        "node_ids": [tree.part_id(usize::try_from(call).unwrap_or(0))],
        "text": format!("{}?", nonce_words(seed, 6)),
        "working_answer": nonce_words(seed ^ 0xa11, 3),
        "price_of_other": nonce_words(seed ^ 0xb22, 3),
        "author_role": "drafter",
        "author_model": "m-1",
        "run": format!("r-{login}"),
    });
    let id = shared.in_flight.begin(&format!("ask_question by {login}"));
    let started = Instant::now();
    let answered = mcp_call(
        hub.port,
        &tree.slug,
        login,
        i64::try_from(call + 1).unwrap_or(1),
        "ask_question",
        arguments,
        READ_TIMEOUT,
    );
    shared.in_flight.end(id);
    shared
        .write_ms
        .lock()
        .expect("write_ms")
        .push(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
    let record = match answered {
        Ok((answer, Some(response))) if answer.status == 200 => {
            if let Some(error) = response.get("error") {
                (true, false, Some(error.to_string()))
            } else {
                let result = &response["result"];
                let stored = result["structuredContent"]["id"].is_string();
                let error = (result["isError"] == true)
                    .then(|| result.to_string().chars().take(300).collect());
                (true, stored, error)
            }
        }
        Ok((answer, _)) => {
            shared.fail(format!(
                "ask_question by {login}: {} with no response: {}",
                answer.status,
                answer.text().chars().take(300).collect::<String>()
            ));
            (false, false, None)
        }
        Err(error) => {
            shared.fail(format!("ask_question by {login}: no answer: {error}"));
            (false, false, None)
        }
    };
    shared.asks.lock().expect("asks").push(record);
}

/// The `wait_ms` of `lines` that have one.
fn waits<'a>(lines: impl Iterator<Item = &'a Line>) -> Vec<u64> {
    lines.filter_map(|line| line.wait_ms).collect()
}

/// The request lines of the `ask_question` calls.
fn ask_lines(hub: &Hub) -> Vec<Line> {
    hub.requests()
        .into_iter()
        .filter(|line| line.tool == "ask_question")
        .collect()
}

/// One phase on `hub`: [`READERS`] readers back to back and a commit to a
/// part file every [`COMMIT_EVERY`] for `secs`, the RSS sampled, the hang
/// watchdog; with `probe`, one `ask_question` by `writer`
/// [`PROBE_AFTER`] after each commit. `shared` gathers it; the phase's
/// start.
fn phase(hub: &Hub, tree: &Tree, shared: &Shared, secs: u64, probe: bool, label: &str) -> Instant {
    let route = format!("/api/projects/{}/library", tree.slug);
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    thread::scope(|scope| {
        for reader in 0..READERS {
            let route = &route;
            scope.spawn(move || {
                let login = format!("reader-{reader}");
                while shared.running(deadline) {
                    page(hub, shared, route, &login);
                }
            });
        }
        scope.spawn(|| {
            let mut n = 0u64;
            while wait(shared, deadline, COMMIT_EVERY) {
                tree.commit_part();
                shared.heads.fetch_add(1, Ordering::SeqCst);
                if probe && wait(shared, deadline, PROBE_AFTER) {
                    n += 1;
                    ask(hub, shared, tree, "writer", (u64::from(u32::MAX) << 32) | n);
                }
            }
        });
        scope.spawn(|| sampler(hub, shared, start, deadline));
        scope.spawn(|| watchdog(hub, shared, deadline, label));
    });
    start
}

/// The first read at each new HEAD among `library` (the one that walks git;
/// the rest at that HEAD are warm, `git` the warm count alone): its count,
/// the warm count, `run_ms` and `git_ms`.
fn first_reads_at_new_heads(library: &[&Line]) -> Value {
    let warm_git = library
        .iter()
        .filter_map(|line| line.git)
        .min()
        .unwrap_or(0);
    let new_head: Vec<&&Line> = library
        .iter()
        .filter(|line| line.git.is_some_and(|git| git > warm_git))
        .collect();
    json!({
        "reads": new_head.len(),
        "warm_git": warm_git,
        "run_ms": spread(&new_head.iter().filter_map(|line| line.run_ms).collect::<Vec<_>>()),
        "git_ms": spread(&new_head.iter().filter_map(|line| line.git_ms).collect::<Vec<_>>()),
    })
}

/// One DL14 run on a fresh hub: the judged window (readers and commits
/// alone, as the spec has it), then, on the same hub, a probe phase of
/// [`PROBE_SECS`] with a probe write after each commit, reported apart and
/// never judged for memory. Its report and its broken
/// budgets.
fn soak_run(tree: &Tree, run: usize, secs: u64) -> (Value, Vec<String>) {
    let hub = Hub::start(
        &tree.root,
        &tree.scratch.join(&format!("hub-home-{run}")),
        &tree.git,
        secs + PROBE_SECS + 900,
    );
    let shared = Shared::default();
    phase(&hub, tree, &shared, secs, false, &format!("dl14-run{run}"));
    let lines = library_lines(&hub, &shared);
    let judged_lines = hub.requests().len();
    let library: Vec<&Line> = lines
        .iter()
        .filter(|line| line.route == LIBRARY_TEMPLATE)
        .collect();
    let ms: Vec<u64> = library.iter().map(|line| line.ms).collect();
    let run_ms: Vec<u64> = library.iter().filter_map(|line| line.run_ms).collect();
    let walked = library.iter().filter(|line| line.index == "walked").count();
    let reader_wait: Vec<u64> = library.iter().filter_map(|line| line.wait_ms).collect();
    let client: Vec<u64> = shared
        .pages
        .lock()
        .expect("pages")
        .iter()
        .map(|(_, _, ms)| *ms)
        .collect();
    let rss = shared.rss.lock().expect("rss").clone();
    let from = secs as f64 / 3.0;
    let judged: Vec<(f64, u64)> = rss.iter().copied().filter(|(t, _)| *t >= from).collect();
    let slope = theil_sen_mib_per_minute(&judged);
    let hangs = shared.hangs.load(Ordering::SeqCst);
    let heads = shared.heads.load(Ordering::SeqCst);
    // The spec's cap: the first read, and one reconcile per 300 s.
    let walked_cap = 1 + usize::try_from(secs / 300).expect("a count");
    let samples = shared.samples.lock().expect("samples").clone();
    let mut broken = shared.failures.lock().expect("failures").clone();
    if slope > SLOPE_BAR {
        broken.push(format!(
            "memory grows {slope:.3} MiB/min over the last two thirds (> {SLOPE_BAR})"
        ));
    }
    if walked > walked_cap {
        broken.push(format!("{walked} index=walked lines (> {walked_cap})"));
    }
    if judged.len() < 10 {
        broken.push(format!("{} RSS samples judged: too few", judged.len()));
    }
    if library.is_empty() {
        broken.push("no library read answered".to_owned());
    }

    // The probe phase: reported only, but a hang or a failed answer there
    // is still one.
    let probe = Shared::default();
    if !broken.iter().any(|what| what.starts_with("a hang")) {
        phase(
            &hub,
            tree,
            &probe,
            PROBE_SECS,
            true,
            &format!("dl14-run{run}-probe"),
        );
    }
    thread::sleep(Duration::from_secs(1));
    let after: Vec<Line> = hub.requests().into_iter().skip(judged_lines).collect();
    let probe_library: Vec<&Line> = after
        .iter()
        .filter(|line| line.route == LIBRARY_TEMPLATE)
        .collect();
    let probes: Vec<&Line> = after
        .iter()
        .filter(|line| line.tool == "ask_question")
        .collect();
    let probes_answered = probe
        .asks
        .lock()
        .expect("asks")
        .iter()
        .filter(|(answered, _, _)| *answered)
        .count();
    broken.extend(
        probe
            .failures
            .lock()
            .expect("failures")
            .iter()
            .map(|what| format!("probe phase: {what}")),
    );
    let report = json!({
        "test": "DL14",
        "run": run,
        "soak_secs": secs,
        "nodes": NODES,
        "reads": library.len(),
        "client_ms": spread(&client),
        "ms": spread(&ms),
        "run_ms": spread(&run_ms),
        "walked": walked,
        "walked_cap": walked_cap,
        "reader_wait_ms": spread(&reader_wait),
        "first_read_at_new_head": first_reads_at_new_heads(&library),
        "slope_mib_per_min": (slope * 1000.0).round() / 1000.0,
        "slope_budget_mib_per_min": SLOPE_BAR,
        "rss_first_kib": rss.first().map(|(_, kib)| *kib),
        "rss_last_kib": rss.last().map(|(_, kib)| *kib),
        "rss_samples": rss.len(),
        "heads": heads,
        "hangs": hangs,
        "samples": samples,
        "probe_phase_reported_only": {
            "secs": PROBE_SECS,
            "heads": probe.heads.load(Ordering::SeqCst),
            "reads": probe_library.len(),
            "walked": probe_library.iter().filter(|line| line.index == "walked").count(),
            "first_read_at_new_head": first_reads_at_new_heads(&probe_library),
            "reader_wait_ms": spread(&waits(probe_library.iter().copied())),
            "writes": probes.len(),
            "answered": probes_answered,
            "writer_wait_ms": spread(&waits(probes.iter().copied())),
            "writer_ms": spread(&probes.iter().map(|line| line.ms).collect::<Vec<_>>()),
            "hangs": probe.hangs.load(Ordering::SeqCst),
        },
        "broken": broken,
    });
    println!("{report}");
    (report, broken)
}

#[test]
#[ignore = "DL14 [soak], release, about 2 x (SOAK_SECS + 60) + 120 s: cargo build --release -p \
            specengine-cli -p specengine-http --bins, then cargo nextest run -p specengine-eval \
            --release --test library_soak --run-ignored only --no-capture"]
fn dl14_library_reads_do_not_grow_the_daemons_memory() {
    let secs = soak_secs();
    let tree = Tree::new("soak", NODES, COMMITS);
    let mut all = Vec::new();
    for run in 1..=2 {
        let (_, broken) = soak_run(&tree, run, secs);
        all.extend(broken.into_iter().map(|what| format!("run {run}: {what}")));
    }
    assert!(all.is_empty(), "DL14's budgets: {all:#?}");
}

#[test]
#[ignore = "DL15 [measure], release, about 120 s: cargo build --release -p specengine-cli \
            -p specengine-http --bins, then cargo nextest run -p specengine-eval --release --test \
            library_soak --run-ignored only --no-capture"]
fn dl15_the_page_under_agents_writes() {
    let tree = Tree::new("writes", NODES, COMMITS);
    let hub = Hub::start(
        &tree.root,
        &tree.scratch.join("hub-home"),
        &tree.git,
        DL15_SECS + 900,
    );
    let library = format!("/api/projects/{}/library", tree.slug);
    let inbox = format!("/api/projects/{}/inbox", tree.slug);
    let shared = Shared::default();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(DL15_SECS);
    thread::scope(|scope| {
        for reader in 0..READERS {
            let (hub, shared, library) = (&hub, &shared, &library);
            scope.spawn(move || {
                let login = format!("reader-{reader}");
                while shared.running(deadline) {
                    page(hub, shared, library, &login);
                }
            });
        }
        for asker in 0..ASKERS {
            let (hub, shared, tree) = (&hub, &shared, &tree);
            scope.spawn(move || {
                let login = format!("asker-{asker}");
                let mut n = 0u64;
                while shared.running(deadline) {
                    n += 1;
                    ask(hub, shared, tree, &login, ((asker as u64) << 32) | n);
                }
            });
        }
        scope.spawn(|| {
            while shared.running(deadline) {
                page(&hub, &shared, &inbox, "owner");
                if !wait(&shared, deadline, INBOX_EVERY) {
                    break;
                }
            }
        });
        scope.spawn(|| sampler(&hub, &shared, start, deadline));
        scope.spawn(|| watchdog(&hub, &shared, deadline, "dl15"));
    });
    let lines = library_lines(&hub, &shared);
    let of = |template: &str| -> Vec<u64> {
        lines
            .iter()
            .filter(|line| line.route == template)
            .map(|line| line.ms)
            .collect()
    };
    let asks = shared.asks.lock().expect("asks").clone();
    let rss = shared.rss.lock().expect("rss").clone();
    let samples = shared.samples.lock().expect("samples").clone();
    let report = json!({
        "test": "DL15",
        "secs": DL15_SECS,
        "nodes": NODES,
        "library_ms": spread(&of(LIBRARY_TEMPLATE)),
        "inbox_ms": spread(&of(INBOX_TEMPLATE)),
        "library_reads": of(LIBRARY_TEMPLATE).len(),
        "inbox_reads": of(INBOX_TEMPLATE).len(),
        "calls": asks.len(),
        "writer_wait_ms": spread(&waits(ask_lines(&hub).iter())),
        "writer_ms": spread(&ask_lines(&hub).iter().map(|line| line.ms).collect::<Vec<_>>()),
        "reader_wait_ms": spread(&waits(
            lines.iter().filter(|line| line.route == LIBRARY_TEMPLATE)
        )),
        "answered": asks.iter().filter(|(answered, _, _)| *answered).count(),
        "stored": asks.iter().filter(|(_, stored, _)| *stored).count(),
        "tool_errors": asks.iter().filter(|(_, _, error)| error.is_some()).count(),
        "first_tool_error": asks.iter().find_map(|(_, _, error)| error.clone()),
        "rss_first_kib": rss.first().map(|(_, kib)| *kib),
        "rss_last_kib": rss.last().map(|(_, kib)| *kib),
        "slope_mib_per_min_reported": theil_sen_mib_per_minute(&rss),
        "hangs": shared.hangs.load(Ordering::SeqCst),
        "samples": samples,
    });
    println!("{report}");
    let broken = shared.failures.lock().expect("failures").clone();
    assert!(broken.is_empty(), "DL15: {broken:#?}");
    assert!(
        !of(LIBRARY_TEMPLATE).is_empty() && !of(INBOX_TEMPLATE).is_empty() && !asks.is_empty(),
        "every kind of request ran: {report}"
    );
    assert!(
        asks.iter().all(|(answered, _, _)| *answered),
        "every call answers: {report}"
    );
}
