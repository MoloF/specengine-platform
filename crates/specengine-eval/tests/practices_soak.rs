//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! DP17 [soak]: repeated `get_card {context: true}` does not grow the
//! daemon's memory. The tree is DP16's (`practices_common`: `scale
//! --corpus-only --nodes 10000 --seed 1`, 20 practices × 30 rules of `art`);
//! the daemon the release `specengine-http`, a hub over it with
//! `--public-origin`, `--port 0`, `--slow-ms 0` and a scratch `HOME`
//! (`library_common::Hub`), a card of `art` with 3 targets made on it by
//! `create_card`.
//!
//! For `SOAK_SECS` (default 300, never less) 10 logins call `get_card
//! {card_id, context: true}` back to back over `/mcp/<slug>`, and one page
//! login reads `GET …/cards/<id>?practices=true` back to back beside them;
//! every 1 s a sample of the daemon's RSS (`ps -o rss= -p <pid>`). Budgets
//! (the spec as the orchestrator re-specified it in iteration 3), each
//! failing the run:
//! - the Theil-Sen slope of the samples from `SOAK_SECS/3` on at most 0.5
//!   MiB per minute;
//! - no call or page read taking 10 000 ms or more (the client's clock and
//!   the line's `ms`), none answering other than 200 with practices served;
//! - no hang: a request unanswered for 30 s runs `sample <pid> 5` on macOS
//!   into `$TMPDIR/specengine-practices-soak-samples/` (kept, its path
//!   printed), then the run fails;
//! - over `/mcp`, at most one walk per `get_card {context: true}` call:
//!   one line per call, `walked` or `trusted` (the line carries no count:
//!   one walk exactly, never two, is `specengine-mcp`'s
//!   `mcp_hub_trust.rs` `dp17_a_hub_get_card_with_context_walks_once`, on
//!   the hub's own trust);
//! - over the page route, `index=walked` lines at most 1 + ⌊`SOAK_SECS`/300⌋.
//!
//! Two runs, each with a fresh daemon (and `HOME`), must both stay within
//! budget; each prints one JSON line (the calls and page reads, their `ms`
//! and `run_ms` p50, p95, p99 and worst, the walked lines per call and on
//! the page route, the slope, the first and last RSS, the hangs), the
//! report that goes with the task. M: a per-process map of practices keyed
//! per call, never evicted (the slope passes the budget); the context
//! bundle walking again (the in-memory count: two walks per call).
//!
//! Run: `cargo build --release -p specengine-cli -p specengine-http -p
//! specengine-mcp --bins`, then `cargo nextest run -p specengine-eval
//! --release --test practices_soak --run-ignored only --no-capture`
//! (`SOAK_SECS=600` for a longer soak). About 2 × `SOAK_SECS` plus the
//! seeding of the tree.
//!
//! The daemon lives under `perl -e 'alarm N'` (N past the run) and is
//! killed and reaped on drop; nothing outlives the test. Every run writes
//! under its scratch directory, removed on drop, but a hang's sample file.

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;
mod library_common;
mod practices_common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use library_common::{
    Hub, InFlight, Line, Tree, mcp_call, page_get, spread, theil_sen_mib_per_minute,
};
use practices_common::{BUILD, hub_card, practice_tree, sample};
use serde_json::{Value, json};

const NODES: usize = 10_000;
const DEFAULT_SOAK_SECS: u64 = 300;
/// DP17's memory budget, MiB per minute.
const SLOPE_BAR: f64 = 0.5;
/// No call takes this long.
const CALL_BAR_MS: u64 = 10_000;
/// A call unanswered this long is a hang.
const HANG: Duration = Duration::from_secs(30);
/// The client's own give-up, past the hang's.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
const LOGINS: usize = 10;
/// The card route as a slow line names it.
const CARD_TEMPLATE: &str = "/api/projects/{p}/cards/{id}";
const SAMPLE_EVERY: Duration = Duration::from_secs(1);

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
        "SOAK_SECS={secs}: DP17 soaks {DEFAULT_SOAK_SECS} s at least"
    );
    secs
}

/// What every thread of one run shares.
#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    hangs: AtomicUsize,
    calls: AtomicU64,
    in_flight: InFlight,
    /// Each call's client ms.
    client_ms: Mutex<Vec<u64>>,
    /// Each page read's client ms.
    page_ms: Mutex<Vec<u64>>,
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

/// One `get_card {card_id, context: true}` by `login`, timed and tracked.
fn call(hub: &Hub, shared: &Shared, tree: &Tree, card: &str, login: &str) {
    let n = shared.calls.fetch_add(1, Ordering::SeqCst);
    let id = shared.in_flight.begin(&format!("get_card by {login}"));
    let started = Instant::now();
    let answered = mcp_call(
        hub.port,
        &tree.slug,
        login,
        i64::try_from(n + 100).unwrap_or(100),
        "get_card",
        json!({"card_id": card, "context": true}),
        CALL_TIMEOUT,
    );
    shared.in_flight.end(id);
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    shared.client_ms.lock().expect("client_ms").push(ms);
    match answered {
        Ok((answer, Some(response))) if answer.status == 200 => {
            let result = &response["result"];
            if response.get("error").is_some() || result["isError"] == json!(true) {
                shared.fail(format!(
                    "get_card by {login}: {}",
                    response.to_string().chars().take(300).collect::<String>()
                ));
            } else if result["structuredContent"]["practices"]
                .as_array()
                .is_none_or(Vec::is_empty)
            {
                shared.fail(format!("get_card by {login}: no practices served"));
            }
            if ms >= CALL_BAR_MS {
                shared.fail(format!("get_card by {login}: {ms} ms"));
            }
        }
        Ok((answer, _)) => shared.fail(format!(
            "get_card by {login}: {} with no response: {}",
            answer.status,
            answer.text().chars().take(300).collect::<String>()
        )),
        Err(error) => shared.fail(format!("get_card by {login}: no answer: {error}")),
    }
}

/// One page read of `GET …/cards/<card>?practices=true`, timed and tracked.
fn page(hub: &Hub, shared: &Shared, tree: &Tree, card: &str) {
    let path = format!("/api/projects/{}/cards/{card}?practices=true", tree.slug);
    let id = shared.in_flight.begin(&format!("GET {path}"));
    let started = Instant::now();
    let answer = page_get(hub.port, &path, "page", CALL_TIMEOUT);
    shared.in_flight.end(id);
    let ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    shared.page_ms.lock().expect("page_ms").push(ms);
    match answer {
        Ok(answer) if answer.status == 200 => {
            let served: Value = serde_json::from_slice(&answer.body).unwrap_or(Value::Null);
            if served["practices"].as_array().is_none_or(Vec::is_empty) {
                shared.fail(format!("GET {path}: no practices served"));
            }
            if ms >= CALL_BAR_MS {
                shared.fail(format!("GET {path}: {ms} ms"));
            }
        }
        Ok(answer) => shared.fail(format!(
            "GET {path}: {}: {}",
            answer.status,
            answer.text().chars().take(300).collect::<String>()
        )),
        Err(error) => shared.fail(format!("GET {path}: no answer: {error}")),
    }
}

/// The hang watchdog: a call open for [`HANG`] samples the daemon once,
/// prints the file's path and stops the run.
fn watchdog(hub: &Hub, shared: &Shared, deadline: Instant, label: &str) {
    let mut seen = std::collections::BTreeSet::new();
    while shared.running(deadline + HANG) {
        for (id, what) in shared.in_flight.older_than(HANG) {
            if seen.insert(id) {
                shared.hangs.fetch_add(1, Ordering::SeqCst);
                let place = sample(hub, label).map_or_else(
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

/// One DP17 run on a fresh hub: its report and its broken budgets.
fn soak_run(tree: &Tree, run: usize, secs: u64) -> (Value, Vec<String>) {
    let hub = Hub::start(
        &tree.root,
        &tree.scratch.join(&format!("hub-home-{run}")),
        &tree.git,
        secs + 900,
    );
    let card = hub_card(&hub, tree, "lead");
    let label = format!("dp17-run{run}");
    let shared = Shared::default();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    thread::scope(|scope| {
        for login in 0..LOGINS {
            let (hub, shared, card) = (&hub, &shared, &card);
            scope.spawn(move || {
                let login = format!("reader-{login}");
                while shared.running(deadline) {
                    call(hub, shared, tree, card, &login);
                }
            });
        }
        {
            let (hub, shared, card) = (&hub, &shared, &card);
            scope.spawn(move || {
                while shared.running(deadline) {
                    page(hub, shared, tree, card);
                }
            });
        }
        scope.spawn(|| sampler(&hub, &shared, start, deadline));
        scope.spawn(|| watchdog(&hub, &shared, deadline, &label));
    });
    thread::sleep(Duration::from_secs(1));
    let all = hub.requests();
    let lines: Vec<Line> = all
        .iter()
        .filter(|line| line.tool == "get_card")
        .cloned()
        .collect();
    let pages: Vec<Line> = all
        .iter()
        .filter(|line| line.route == CARD_TEMPLATE)
        .cloned()
        .collect();
    let mut broken = shared.failures.lock().expect("failures").clone();
    for line in lines.iter().chain(&pages) {
        if line.ms >= CALL_BAR_MS || line.status != "200" {
            broken.push(format!("a line: {}", line.raw));
        }
    }
    let ms: Vec<u64> = lines.iter().map(|line| line.ms).collect();
    let run_ms: Vec<u64> = lines.iter().filter_map(|line| line.run_ms).collect();
    let page_line_ms: Vec<u64> = pages.iter().map(|line| line.ms).collect();
    let page_run_ms: Vec<u64> = pages.iter().filter_map(|line| line.run_ms).collect();
    // One line per call: at most one walk shows per call.
    let calls = usize::try_from(shared.calls.load(Ordering::SeqCst)).expect("a count");
    let walked = lines.iter().filter(|line| line.index == "walked").count();
    let page_walked = pages.iter().filter(|line| line.index == "walked").count();
    let walked_cap = 1 + usize::try_from(secs / 300).expect("a count");
    let client = shared.client_ms.lock().expect("client_ms").clone();
    let page_client = shared.page_ms.lock().expect("page_ms").clone();
    let rss = shared.rss.lock().expect("rss").clone();
    let from = secs as f64 / 3.0;
    let judged: Vec<(f64, u64)> = rss.iter().copied().filter(|(t, _)| *t >= from).collect();
    let slope = theil_sen_mib_per_minute(&judged);
    if slope > SLOPE_BAR {
        broken.push(format!(
            "memory grows {slope:.3} MiB/min over the last two thirds (> {SLOPE_BAR})"
        ));
    }
    if lines.len() > calls {
        broken.push(format!("{} get_card lines for {calls} calls", lines.len()));
    }
    if walked > lines.len() {
        broken.push(format!("{walked} walked lines over {} calls", lines.len()));
    }
    if page_walked > walked_cap {
        broken.push(format!(
            "{page_walked} index=walked page lines (> {walked_cap})"
        ));
    }
    if pages.is_empty() {
        broken.push("no page read answered".to_owned());
    }
    if judged.len() < 10 {
        broken.push(format!("{} RSS samples judged: too few", judged.len()));
    }
    if lines.is_empty() {
        broken.push("no get_card call answered".to_owned());
    }
    let report = json!({
        "test": "DP17",
        "run": run,
        "soak_secs": secs,
        "nodes": NODES,
        "build": BUILD,
        "calls": lines.len(),
        "client_ms": spread(&client),
        "ms": spread(&ms),
        "run_ms": spread(&run_ms),
        "walked": walked,
        "walked_per_call": if lines.is_empty() { 0.0 } else { walked as f64 / lines.len() as f64 },
        "page_reads": pages.len(),
        "page_client_ms": spread(&page_client),
        "page_ms": spread(&page_line_ms),
        "page_run_ms": spread(&page_run_ms),
        "page_walked": page_walked,
        "page_walked_cap": walked_cap,
        "slope_mib_per_min": (slope * 1000.0).round() / 1000.0,
        "slope_budget_mib_per_min": SLOPE_BAR,
        "rss_first_kib": rss.first().map(|(_, kib)| *kib),
        "rss_last_kib": rss.last().map(|(_, kib)| *kib),
        "rss_samples": rss.len(),
        "hangs": shared.hangs.load(Ordering::SeqCst),
        "samples": shared.samples.lock().expect("samples").clone(),
        "broken": broken,
    });
    println!("{report}");
    (report, broken)
}

#[test]
#[ignore = "DP17 [soak], release, about 2 x SOAK_SECS plus the seeding: cargo build --release \
            -p specengine-cli -p specengine-http -p specengine-mcp --bins, then cargo nextest run \
            -p specengine-eval --release --test practices_soak --run-ignored only --no-capture"]
fn dp17_card_context_reads_do_not_grow_the_daemons_memory() {
    let secs = soak_secs();
    let tree = practice_tree("soak", NODES);
    let mut all = Vec::new();
    for run in 1..=2 {
        let (_, broken) = soak_run(&tree, run, secs);
        all.extend(broken.into_iter().map(|what| format!("run {run}: {what}")));
    }
    assert!(all.is_empty(), "DP17's budgets: {all:#?}");
}

/// The `key=<n>` value of a slow line's raw text.
fn field(line: &Line, key: &str) -> Option<u64> {
    line.raw
        .split(' ')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")))
        .and_then(|value| value.parse().ok())
}

/// A probe of how a call's walks show on its line: on the DP16 tree's hub,
/// three `get_card {card_id}` calls (the mark's walk alone), then three
/// with `context: true`, each line's `index`, `git` and `index_ms` printed.
#[test]
#[ignore = "DP17 probe, release: prints what one walk costs on a get_card line"]
fn dp17_probe_walks_per_call() {
    let tree = practice_tree("probe", 1_000);
    let hub = Hub::start(&tree.root, &tree.scratch.join("hub-home"), &tree.git, 600);
    let card = hub_card(&hub, &tree, "lead");
    for (n, context) in [
        (1, false),
        (2, false),
        (3, false),
        (4, true),
        (5, true),
        (6, true),
    ] {
        let args = if context {
            json!({"card_id": card, "context": true})
        } else {
            json!({"card_id": card})
        };
        let (answer, _) = mcp_call(
            hub.port,
            &tree.slug,
            "reader",
            100 + n,
            "get_card",
            args,
            CALL_TIMEOUT,
        )
        .expect("an answer");
        assert_eq!(answer.status, 200);
    }
    thread::sleep(Duration::from_secs(1));
    for line in hub
        .requests()
        .into_iter()
        .filter(|line| line.tool == "get_card")
    {
        println!(
            "index={} git={:?} index_ms={:?} run_ms={:?}",
            line.index,
            line.git,
            field(&line, "index_ms"),
            line.run_ms
        );
    }
}
