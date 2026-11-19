//! docs/features/doc-library.md (RM-0127) "Acceptance criteria", DL13
//! [measure]: the library's hold bars. The tree and the 10 000-commit
//! history are DC-11's (`catalog_scale.rs`, rebuilt by `library_common`).
//!
//! - At `--nodes 10000` (about 2 000 documents, the live hub's size):
//!   - `spec library --json` (release), the median of five warm runs: at most
//!     2 000 ms;
//!   - a hub's `GET …/library`, its `run_ms` read from its request lines,
//!     warm at one `HEAD` (the second read on), the median of five: at most
//!     500 ms, each `index=trusted`;
//!   - the first read after one commit to a part file: `run_ms` at most
//!     2 000 ms (its `ms` reported).
//! - At `--nodes 50000` (about 10 000 documents): warm `run_ms` at most
//!   2 000 ms; the rest reported.
//! - At `--nodes 1000`: reported only.
//!
//! Bars never rise without the owner's word.
//!
//! Run: `cargo build --release -p specengine-cli -p specengine-http
//! --bins` (`--bin spec` with both packages builds no daemon: the spec's
//! command left `specengine-http` stale), then `cargo nextest run -p
//! specengine-eval --release --test library_scale --run-ignored only --no-capture` (one size:
//! append `dl13_measure_10k`).
//!
//! Every run writes under its scratch directory in `std::env::temp_dir()`,
//! removed on drop, with `HOME`s of its own; the hub dies with the test
//! (killed on drop, and by its own alarm).

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;
mod library_common;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use library_common::{COMMITS, Hub, LIBRARY_TEMPLATE, Tree, median, page_get, release_bin};
use serde_json::{Value, json};

/// Timed runs; the median is kept.
const RUNS: usize = 5;
/// DL13's bars.
const CLI_BAR_MS: u64 = 2_000;
const WARM_BAR_MS: u64 = 500;
const AFTER_COMMIT_BAR_MS: u64 = 2_000;
const WARM_BAR_50K_MS: u64 = 2_000;
/// The hub's self-kill alarm: past any run of this file.
const ALARM_SECS: u64 = 3_600;
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// What one size measured.
struct Measured {
    cli_ms: u64,
    warm_run_ms: u64,
    after_commit_run_ms: u64,
}

/// The measure at `nodes`, printed as one JSON line.
fn measure(nodes: usize) -> Measured {
    let started = Instant::now();
    let tree = Tree::new(&format!("scale-{nodes}"), nodes, COMMITS);
    let seeding = started.elapsed();
    let spec = release_bin("spec");

    // The CLI: one warm-up, then five timed runs.
    let cli_home = tree.scratch.join("cli-home");
    std::fs::create_dir_all(&cli_home).expect("a home");
    let vars = tree.spec_vars(&cli_home);
    let mut document = Value::Null;
    let mut cli_runs = Vec::new();
    for run in 0..=RUNS {
        let at = Instant::now();
        let output = Command::new(&spec)
            .env_clear()
            .envs(vars.iter().map(|(name, value)| (name, value)))
            .current_dir(&tree.root)
            .args(["library", "--json"])
            .stdin(Stdio::null())
            .output()
            .expect("spec runs");
        let ms = u64::try_from(at.elapsed().as_millis()).expect("ms");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if run > 0 {
            cli_runs.push(ms);
        }
        document = serde_json::from_slice(&output.stdout).expect("JSON");
    }
    let cli_ms = median(&cli_runs);
    let cli_shown = document["rows"].as_array().map_or(0, Vec::len);
    let cli_truncated = document["truncated"].clone();

    // The hub: the first read walks, then five warm reads at one HEAD.
    let hub = Hub::start(
        &tree.root,
        &tree.scratch.join("hub-home"),
        &tree.git,
        ALARM_SECS,
    );
    let route = format!("/api/projects/{}/library", tree.slug);
    let mut bodies = 0usize;
    let mut whole = Value::Null;
    for _ in 0..=RUNS {
        let answer = page_get(hub.port, &route, "reader", READ_TIMEOUT).expect("an answer");
        assert_eq!(answer.status, 200, "{}", answer.text());
        bodies = answer.body.len();
        whole = serde_json::from_slice(&answer.body).expect("JSON");
    }
    // The route is never cut: its rows are the documents; the CLI's JSON is
    // cut at the output cap, its counts over the rows it shows.
    let documents = whole["rows"].as_array().map_or(0, Vec::len);
    assert_eq!(whole["truncated"], false);
    let lines = hub.requests_of(LIBRARY_TEMPLATE, RUNS + 1);
    let warm: Vec<&library_common::Line> = lines[1..=RUNS].iter().collect();
    for line in &warm {
        assert_eq!(line.index, "trusted", "a warm read trusts: {}", line.raw);
    }
    let warm_run: Vec<u64> = warm
        .iter()
        .map(|line| {
            line.run_ms
                .unwrap_or_else(|| panic!("run_ms: {}", line.raw))
        })
        .collect();
    let warm_run_ms = median(&warm_run);

    // One commit, then the first read after it.
    tree.commit_part();
    let answer = page_get(hub.port, &route, "reader", READ_TIMEOUT).expect("an answer");
    assert_eq!(answer.status, 200, "{}", answer.text());
    let after = hub.requests_of(LIBRARY_TEMPLATE, RUNS + 2)[RUNS + 1].clone();
    let after_commit_run_ms = after
        .run_ms
        .unwrap_or_else(|| panic!("run_ms: {}", after.raw));
    let rss = hub.rss_kib();
    println!(
        "{}",
        json!({
            "test": "DL13",
            "nodes": nodes,
            "documents": documents,
            "commits": COMMITS,
            "seeding_s": seeding.as_secs(),
            "cli_ms": {"median": cli_ms, "runs": cli_runs},
            "cli_rows_shown": cli_shown,
            "cli_truncated": cli_truncated,
            "route_body_bytes": bodies,
            "first_read": {"ms": lines[0].ms, "run_ms": lines[0].run_ms, "index": lines[0].index},
            "warm_run_ms": {"median": warm_run_ms, "runs": warm_run},
            "warm_ms": warm.iter().map(|line| line.ms).collect::<Vec<_>>(),
            "after_commit": {"ms": after.ms, "run_ms": after.run_ms, "index": after.index},
            "hub_rss_kib": rss,
        })
    );
    Measured {
        cli_ms,
        warm_run_ms,
        after_commit_run_ms,
    }
}

#[test]
#[ignore = "DL13 [measure], release, reported only: cargo build --release -p \
            specengine-cli -p specengine-http --bins, then cargo nextest run -p specengine-eval \
            --release --test library_scale --run-ignored only --no-capture"]
fn dl13_measure_1k() {
    measure(1_000);
}

#[test]
#[ignore = "DL13 [measure], release: cargo build --release -p \
            specengine-cli -p specengine-http --bins, then cargo nextest run -p specengine-eval \
            --release --test library_scale --run-ignored only --no-capture"]
fn dl13_measure_10k() {
    let measured = measure(10_000);
    let mut broken = Vec::new();
    if measured.cli_ms > CLI_BAR_MS {
        broken.push(format!(
            "spec library --json median {} ms > {CLI_BAR_MS}",
            measured.cli_ms
        ));
    }
    if measured.warm_run_ms > WARM_BAR_MS {
        broken.push(format!(
            "warm run_ms median {} ms > {WARM_BAR_MS}",
            measured.warm_run_ms
        ));
    }
    if measured.after_commit_run_ms > AFTER_COMMIT_BAR_MS {
        broken.push(format!(
            "the first read after a commit, run_ms {} > {AFTER_COMMIT_BAR_MS}",
            measured.after_commit_run_ms
        ));
    }
    assert!(broken.is_empty(), "DL13's bars at 10k: {broken:?}");
}

#[test]
#[ignore = "DL13 [measure], release: cargo build --release -p \
            specengine-cli -p specengine-http --bins, then cargo nextest run -p specengine-eval \
            --release --test library_scale --run-ignored only --no-capture"]
fn dl13_measure_50k() {
    let measured = measure(50_000);
    assert!(
        measured.warm_run_ms <= WARM_BAR_50K_MS,
        "DL13's bar at 50k: warm run_ms median {} ms > {WARM_BAR_50K_MS}",
        measured.warm_run_ms
    );
}
