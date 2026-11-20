//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! DP16 [measure]: the latency of a card's context at a realistic size.
//! The tree is `practices_common`'s: `scale --corpus-only --nodes N --seed
//! 1`, plus 20 practices × 30 rules of `art`, plus a card of `art` with 3
//! targets.
//!
//! - At `--nodes 10000`, warm, the median of five (one warm-up first):
//!   - the release `spec card show C --context --json`: at most 800 ms, and
//!     at most 200 ms over the same card (same title, goal, targets) whose
//!     discipline (`nothing-here`) matches no practice;
//!   - a hub's MCP `get_card {card_id, context: true}`, its `run_ms` read
//!     from its request lines (`--slow-ms 0`): at most 1 000 ms.
//! - At `--nodes 1000`: reported only.
//!
//! Bars never rise without the owner's word. Each run prints one JSON line.
//!
//! Run: `cargo build --release -p specengine-cli -p specengine-http -p
//! specengine-mcp --bins` (a `--bin` filter once built no daemon and gave
//! stale numbers; `library_common::release_bin` refuses a binary older
//! than the sources), then `cargo nextest run -p specengine-eval --release
//! --test practices_scale --run-ignored only --no-capture` (one size:
//! append `dp16_measure_10k`).
//!
//! Every run writes under its scratch directory, removed on drop, with
//! `HOME`s of its own; the hub dies with the test (killed on drop, and by
//! its own alarm).

#![cfg(unix)]

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
mod import_support;
mod library_common;
mod practices_common;

use std::time::Duration;

use library_common::{Hub, Line, mcp_call, median};
use practices_common::{
    BUILD, DISCIPLINE, NOTHING, PRACTICES, cli_card, hub_card, practice_tree, release_spec,
};
use serde_json::{Value, json};

/// Timed runs; the median is kept.
const RUNS: usize = 5;
/// DP16's bars.
const CLI_BAR_MS: u64 = 800;
const OVER_NOTHING_BAR_MS: u64 = 200;
const HUB_BAR_MS: u64 = 1_000;
/// The hub's self-kill alarm: past any run of this file.
const ALARM_SECS: u64 = 3_600;
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// What one size measured.
struct Measured {
    cli_ms: u64,
    nothing_ms: u64,
    hub_run_ms: u64,
}

/// `card show <card> --context --json` six times: the five warm runs' ms
/// and the last document.
fn cli_runs(tree: &library_common::Tree, home: &std::path::Path, card: &str) -> (Vec<u64>, Value) {
    let mut runs = Vec::new();
    let mut document = Value::Null;
    for run in 0..=RUNS {
        let (output, ms) = release_spec(tree, home, &["--json", "card", "show", card, "--context"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if run > 0 {
            runs.push(ms);
        }
        document = serde_json::from_slice(&output.stdout).expect("JSON");
    }
    (runs, document)
}

/// The measure at `nodes`, printed as one JSON line.
fn measure(nodes: usize) -> Measured {
    let started = std::time::Instant::now();
    let tree = practice_tree(&format!("scale-{nodes}"), nodes);
    let seeding = started.elapsed();

    // The CLI: two cards alike but for the discipline.
    let home = tree.scratch.join("cli-home");
    std::fs::create_dir_all(&home).expect("a home");
    let art = cli_card(&tree, &home, DISCIPLINE);
    let nothing = cli_card(&tree, &home, NOTHING);
    let (art_runs, document) = cli_runs(&tree, &home, &art);
    assert_eq!(
        document["practices"].as_array().map(Vec::len),
        Some(PRACTICES),
        "premise: the card lists the 20 practices: {}",
        document["notes"]
    );
    let placed = document["context"]["bundle"]["layers"]["practices"]
        .as_array()
        .map_or(0, Vec::len);
    assert!(placed > 0, "premise: layer 10 holds rules");
    let (nothing_runs, plain) = cli_runs(&tree, &home, &nothing);
    assert_eq!(plain["practices"], json!([]), "premise: nothing matches");
    let cli_ms = median(&art_runs);
    let nothing_ms = median(&nothing_runs);

    // The hub: a card made over /mcp, then six get_card calls.
    let hub = Hub::start(
        &tree.root,
        &tree.scratch.join("hub-home"),
        &tree.git,
        ALARM_SECS,
    );
    let card = hub_card(&hub, &tree, "lead");
    let mut bodies = Vec::new();
    for call in 0..=RUNS {
        let (answer, response) = mcp_call(
            hub.port,
            &tree.slug,
            "reader",
            i64::try_from(call + 10).expect("an id"),
            "get_card",
            json!({"card_id": card, "context": true}),
            CALL_TIMEOUT,
        )
        .expect("get_card answers");
        assert_eq!(answer.status, 200, "{}", answer.text());
        let response = response.expect("a response");
        assert_ne!(response["result"]["isError"], json!(true), "{response}");
        bodies.push(answer.body.len());
    }
    std::thread::sleep(Duration::from_secs(1));
    let lines: Vec<Line> = hub
        .requests()
        .into_iter()
        .filter(|line| line.tool == "get_card")
        .collect();
    assert_eq!(lines.len(), RUNS + 1, "{}", hub.stderr());
    let warm: Vec<u64> = lines[1..]
        .iter()
        .map(|line| {
            line.run_ms
                .unwrap_or_else(|| panic!("run_ms: {}", line.raw))
        })
        .collect();
    let hub_run_ms = median(&warm);
    println!(
        "{}",
        json!({
            "test": "DP16",
            "nodes": nodes,
            "practices": PRACTICES,
            "rules_placed": placed,
            "seeding_s": seeding.as_secs(),
            "build": BUILD,
            "cli_ms": {"median": cli_ms, "runs": art_runs},
            "nothing_ms": {"median": nothing_ms, "runs": nothing_runs},
            "over_nothing_ms": cli_ms.saturating_sub(nothing_ms),
            "hub_first": {"ms": lines[0].ms, "run_ms": lines[0].run_ms, "index": lines[0].index},
            "hub_warm_run_ms": {"median": hub_run_ms, "runs": warm},
            "hub_warm_ms": lines[1..].iter().map(|line| line.ms).collect::<Vec<_>>(),
            "body_bytes": bodies,
            "hub_rss_kib": hub.rss_kib(),
        })
    );
    Measured {
        cli_ms,
        nothing_ms,
        hub_run_ms,
    }
}

#[test]
#[ignore = "DP16 [measure], release, reported only: cargo build --release -p specengine-cli \
            -p specengine-http -p specengine-mcp --bins, then cargo nextest run -p \
            specengine-eval --release --test practices_scale --run-ignored only --no-capture"]
fn dp16_measure_1k() {
    measure(1_000);
}

#[test]
#[ignore = "DP16 [measure], release: cargo build --release -p specengine-cli -p \
            specengine-http -p specengine-mcp --bins, then cargo nextest run -p specengine-eval \
            --release --test practices_scale --run-ignored only --no-capture"]
fn dp16_measure_10k() {
    let measured = measure(10_000);
    let mut broken = Vec::new();
    if measured.cli_ms > CLI_BAR_MS {
        broken.push(format!(
            "spec card show --context --json median {} ms > {CLI_BAR_MS}",
            measured.cli_ms
        ));
    }
    let over = measured.cli_ms.saturating_sub(measured.nothing_ms);
    if over > OVER_NOTHING_BAR_MS {
        broken.push(format!(
            "{over} ms over the card whose discipline matches nothing (> {OVER_NOTHING_BAR_MS})"
        ));
    }
    if measured.hub_run_ms > HUB_BAR_MS {
        broken.push(format!(
            "the hub's get_card run_ms median {} ms > {HUB_BAR_MS}",
            measured.hub_run_ms
        ));
    }
    assert!(broken.is_empty(), "DP16's bars at 10k: {broken:?}");
}
