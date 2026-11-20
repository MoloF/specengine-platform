//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria" GU16
//! [measure]: no turn, no growth. The `library_common` harness, as
//! doc-library's DL15: DC-11's tree at `--nodes 10000` with its seeded
//! history, the release `specengine-http` as a hub over it (`--public-origin`,
//! `--port 0`, `--slow-ms 0`, a scratch `HOME`, under `perl -e 'alarm N'`).
//! For [`SECS`] (120): 10 logins each read the guide back to back, turn by
//! turn `get_guide` over `/mcp/<slug>` and `GET /api/guide/<topic>`, the
//! topic cycling through `GUIDE_ORDER` (and the index); beside them 8 logins
//! each raise `ask_question` over `/mcp/<slug>` back to back (fresh
//! syllables, so no hit); every 1 s a sample of the daemon's RSS.
//!
//! Budgets, each failing the run: every guide read answers (200, no
//! `isError`) under 1 000 ms by the client's clock and its line's `ms`;
//! every guide line takes no turn (`wait_ms=-`, `run_ms=-` for the route,
//! `wait_ms=-` for `tool=get_guide`); every write answers; no hang (a
//! request open 30 s samples the daemon with `sample <pid> 5` on macOS into
//! a kept temp file, prints its path and fails the run). Reported, not
//! judged: the RSS's Theil-Sen slope (over the run and over its second
//! half, the owner's measure of growth), its first, middle and last sample, the
//! spreads of the guide reads and the writes. The report is one stdout
//! JSON line.
//!
//! Measured 2026-11-20 on the owner's laptop (iteration 1 of the slice),
//! for the morning: two GU16 runs, about 1.27M route reads and 1.27M
//! `get_guide` calls each, worst client 118 and 113 ms, worst line 12 and
//! 11 ms, every guide line with no turn, about 1 000 writes all answered
//! (writer p50 about 960 ms), RSS 5 to 96 MB: whole-run slope 0.64 and
//! 0.73 MiB/min, second-half slope 1.68 MiB/min. The readers-only control,
//! two runs: RSS 9.8 to 13.7 MB, second-half slope 0.0. The growth is the
//! write path's (CLAUDE.md's open "memory growth" item), not the guide's.
//!
//! Run: `cargo build --release -p specengine-cli -p specengine-http
//! --bins`, then `cargo nextest run -p specengine-eval --release --test
//! guide_soak --run-ignored only --no-capture`. About 120 s plus the
//! seeding of the tree. The daemon dies by SIGALRM even if this process is
//! killed, and is killed and reaped on drop.

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
#[path = "../../specengine-cli/tests/guide_source/mod.rs"]
mod guide_source;
mod import_support;
mod library_common;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use guide_source::GUIDE_ORDER;
use library_common::{
    COMMITS, Hub, InFlight, Line, Tree, mcp_call, nonce_words, page_get, spread,
    theil_sen_mib_per_minute,
};
use serde_json::{Value, json};

const NODES: usize = 10_000;
const SECS: u64 = 120;
/// GU16's bar for a guide read, ms.
const GUIDE_BAR_MS: u64 = 1_000;
/// A request open this long is a hang.
const HANG: Duration = Duration::from_secs(30);
/// The client's own give-up, past the hang's.
const TIMEOUT: Duration = Duration::from_secs(120);
const READERS: usize = 10;
const WRITERS: usize = 8;
const SAMPLE_EVERY: Duration = Duration::from_secs(1);
/// The guide route as a slow line names a topic's read.
const TOPIC_TEMPLATE_PREFIX: &str = "/api/guide";

/// What every thread of the run shares.
#[derive(Default)]
struct Shared {
    stop: AtomicBool,
    hangs: AtomicUsize,
    calls: AtomicU64,
    in_flight: InFlight,
    /// Each guide read: (what, ok, the client's ms).
    reads: Mutex<Vec<(String, bool, u64)>>,
    /// Each write: (answered, the client's ms).
    writes: Mutex<Vec<(bool, u64)>>,
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

fn ms(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

/// One `GET /api/guide[/<topic>]` by `login`, timed and judged.
fn route_read(hub: &Hub, shared: &Shared, topic: Option<&str>, login: &str) {
    let path = topic.map_or_else(|| "/api/guide".to_owned(), |t| format!("/api/guide/{t}"));
    let id = shared.in_flight.begin(&format!("GET {path} by {login}"));
    let answer = page_get(hub.port, &path, login, TIMEOUT);
    shared.in_flight.end(id);
    let (ok, took) = match answer {
        Ok(answer) => {
            let took = ms(answer.elapsed);
            let ok = answer.status == 200 && took < GUIDE_BAR_MS;
            if !ok {
                shared.fail(format!(
                    "GET {path} by {login}: {} in {took} ms: {}",
                    answer.status,
                    answer.text().chars().take(200).collect::<String>()
                ));
            }
            (ok, took)
        }
        Err(error) => {
            shared.fail(format!("GET {path} by {login}: no answer: {error}"));
            (false, ms(TIMEOUT))
        }
    };
    shared.reads.lock().expect("reads").push((path, ok, took));
}

/// One `get_guide` over `/mcp/<slug>` by `login`, timed and judged.
fn tool_read(hub: &Hub, shared: &Shared, slug: &str, topic: Option<&str>, login: &str) {
    let call = shared.calls.fetch_add(1, Ordering::SeqCst);
    let arguments = topic.map_or_else(|| json!({}), |topic| json!({"topic": topic}));
    let what = format!("get_guide {arguments} by {login}");
    let id = shared.in_flight.begin(&what);
    let started = Instant::now();
    let answered = mcp_call(
        hub.port,
        slug,
        login,
        i64::try_from(call + 1).unwrap_or(1),
        "get_guide",
        arguments,
        TIMEOUT,
    );
    shared.in_flight.end(id);
    let took = ms(started.elapsed());
    let ok = match answered {
        Ok((answer, Some(response))) if answer.status == 200 => {
            let good = response.get("error").is_none()
                && response["result"]["isError"] != json!(true)
                && response["result"]["structuredContent"].is_object();
            if !good {
                shared.fail(format!(
                    "{what}: {}",
                    response.to_string().chars().take(300).collect::<String>()
                ));
            }
            good && took < GUIDE_BAR_MS
        }
        Ok((answer, _)) => {
            shared.fail(format!("{what}: {} with no response", answer.status));
            false
        }
        Err(error) => {
            shared.fail(format!("{what}: no answer: {error}"));
            false
        }
    };
    if took >= GUIDE_BAR_MS {
        shared.fail(format!("{what}: {took} ms"));
    }
    shared
        .reads
        .lock()
        .expect("reads")
        .push(("get_guide".to_owned(), ok, took));
}

/// One `ask_question` by `login` on a part of the tree, its text fresh
/// syllables from `seed`: answered or not, the client's ms.
fn write(hub: &Hub, shared: &Shared, tree: &Tree, login: &str, seed: u64) {
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
        TIMEOUT,
    );
    shared.in_flight.end(id);
    let took = ms(started.elapsed());
    let answered = match answered {
        Ok((answer, Some(_))) if answer.status == 200 => true,
        Ok((answer, _)) => {
            shared.fail(format!("ask_question by {login}: {}", answer.status));
            false
        }
        Err(error) => {
            shared.fail(format!("ask_question by {login}: no answer: {error}"));
            false
        }
    };
    shared.writes.lock().expect("writes").push((answered, took));
}

/// The hang watchdog: a request open for [`HANG`] samples the daemon once,
/// prints the file's path and stops the run.
fn watchdog(hub: &Hub, shared: &Shared, deadline: Instant) {
    let mut seen = std::collections::BTreeSet::new();
    while shared.running(deadline + HANG) {
        for (id, what) in shared.in_flight.older_than(HANG) {
            if seen.insert(id) {
                shared.hangs.fetch_add(1, Ordering::SeqCst);
                let place = hub.sample("gu16").map_or_else(
                    || "no sample (not macOS)".to_owned(),
                    |path| path.display().to_string(),
                );
                println!("HANG gu16: {what} open {HANG:?}; sample {place}");
                shared.samples.lock().expect("samples").push(place);
                shared.fail(format!("a hang: {what}"));
                shared.stop.store(true, Ordering::SeqCst);
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// The RSS sampler.
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

/// GU16 [measure], release.
#[test]
#[ignore = "GU16 [measure], release, about 120 s plus seeding: cargo build --release -p \
            specengine-cli -p specengine-http --bins, then cargo nextest run -p \
            specengine-eval --release --test guide_soak --run-ignored only --no-capture"]
fn gu16_guide_reads_take_no_turn_beside_agents_writes() {
    run(WRITERS, "GU16");
}

/// The control of GU16's memory report: the same readers with no writer,
/// so the RSS growth is the guide reads' alone (the owner's rule: a
/// hypothesis takes two runs a variant). Every bar of GU16 holds; nothing
/// more is judged.
#[test]
#[ignore = "GU16 control [measure], release, about 120 s plus seeding: as GU16, append \
            gu16_control"]
fn gu16_control_guide_reads_alone() {
    run(0, "GU16-control");
}

/// One GU16 run with `writers` agents writing beside the readers.
fn run(writers: usize, label: &str) {
    let tree = Tree::new("guide", NODES, COMMITS);
    let hub = Hub::start(
        &tree.root,
        &tree.scratch.join("hub-home"),
        &tree.git,
        SECS + 900,
    );
    let shared = Shared::default();
    let start = Instant::now();
    let deadline = start + Duration::from_secs(SECS);
    let topics: Vec<Option<&str>> = std::iter::once(None)
        .chain(GUIDE_ORDER.iter().map(|topic| Some(*topic)))
        .collect();
    thread::scope(|scope| {
        for reader in 0..READERS {
            let (hub, shared, tree, topics) = (&hub, &shared, &tree, &topics);
            scope.spawn(move || {
                let login = format!("reader-{reader}");
                let mut n = reader;
                while shared.running(deadline) {
                    let topic = topics[n % topics.len()];
                    tool_read(hub, shared, &tree.slug, topic, &login);
                    if !shared.running(deadline) {
                        break;
                    }
                    route_read(hub, shared, topic, &login);
                    n += 1;
                }
            });
        }
        for writer in 0..writers {
            let (hub, shared, tree) = (&hub, &shared, &tree);
            scope.spawn(move || {
                let login = format!("agent-{writer}");
                let mut n = 0u64;
                while shared.running(deadline) {
                    n += 1;
                    write(hub, shared, tree, &login, ((writer as u64) << 32) | n);
                }
            });
        }
        scope.spawn(|| sampler(&hub, &shared, start, deadline));
        scope.spawn(|| watchdog(&hub, &shared, deadline));
    });
    thread::sleep(Duration::from_secs(1));
    let lines = hub.requests();
    let route_lines: Vec<&Line> = lines
        .iter()
        .filter(|line| line.route.starts_with(TOPIC_TEMPLATE_PREFIX))
        .collect();
    let tool_lines: Vec<&Line> = lines
        .iter()
        .filter(|line| line.tool == "get_guide")
        .collect();
    let write_lines: Vec<&Line> = lines
        .iter()
        .filter(|line| line.tool == "ask_question")
        .collect();
    for line in &route_lines {
        if line.status != "200"
            || line.ms >= GUIDE_BAR_MS
            || line.wait_ms.is_some()
            || line.run_ms.is_some()
        {
            shared.fail(format!("a guide route line: {}", line.raw));
        }
    }
    for line in &tool_lines {
        if line.status != "200" || line.ms >= GUIDE_BAR_MS || line.wait_ms.is_some() {
            shared.fail(format!("a get_guide line: {}", line.raw));
        }
    }
    let reads = shared.reads.lock().expect("reads").clone();
    let writes = shared.writes.lock().expect("writes").clone();
    let rss = shared.rss.lock().expect("rss").clone();
    let samples = shared.samples.lock().expect("samples").clone();
    let read_ms = |kind: &str| -> Vec<u64> {
        reads
            .iter()
            .filter(|(what, _, _)| (kind == "get_guide") == (what == "get_guide"))
            .map(|(_, _, took)| *took)
            .collect()
    };
    let report: Value = json!({
        "test": label,
        "secs": SECS,
        "nodes": NODES,
        "route_reads": read_ms("route").len(),
        "tool_reads": read_ms("get_guide").len(),
        "route_ms": spread(&read_ms("route")),
        "tool_ms": spread(&read_ms("get_guide")),
        "route_line_ms": spread(&route_lines.iter().map(|line| line.ms).collect::<Vec<_>>()),
        "tool_line_ms": spread(&tool_lines.iter().map(|line| line.ms).collect::<Vec<_>>()),
        "writes": writes.len(),
        "writes_answered": writes.iter().filter(|(answered, _)| *answered).count(),
        "write_ms": spread(&writes.iter().map(|(_, took)| *took).collect::<Vec<_>>()),
        "writer_wait_ms": spread(
            &write_lines.iter().filter_map(|line| line.wait_ms).collect::<Vec<_>>()
        ),
        "rss_first_kib": rss.first().map(|(_, kib)| *kib),
        "rss_last_kib": rss.last().map(|(_, kib)| *kib),
        "slope_mib_per_min_reported": theil_sen_mib_per_minute(&rss),
        // The owner's rule: memory growth is the slope over the second half,
        // not the warm-up.
        "slope_second_half_mib_per_min_reported": theil_sen_mib_per_minute(
            &rss.iter()
                .copied()
                .filter(|(at, _)| *at >= SECS as f64 / 2.0)
                .collect::<Vec<_>>()
        ),
        "rss_mid_kib": rss
            .iter()
            .find(|(at, _)| *at >= SECS as f64 / 2.0)
            .map(|(_, kib)| *kib),
        "hangs": shared.hangs.load(Ordering::SeqCst),
        "samples": samples,
    });
    println!("{report}");
    let broken = shared.failures.lock().expect("failures").clone();
    assert!(broken.is_empty(), "GU16: {broken:#?}");
    assert!(
        !route_lines.is_empty() && !tool_lines.is_empty() && (writers == 0 || !writes.is_empty()),
        "every kind of request ran: {report}"
    );
    assert!(
        reads.iter().all(|(_, ok, _)| *ok),
        "every guide read answered within the bar: {report}"
    );
    assert!(
        writes.iter().all(|(answered, _)| *answered),
        "every write answered: {report}"
    );
}
