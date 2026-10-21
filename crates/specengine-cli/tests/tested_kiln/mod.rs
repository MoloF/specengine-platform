//! Helpers of the docs/features/tested-evidence.md (8b-1) tests over 8a's
//! repositories (`verify_tests_common::Kiln`: a scratch copy of
//! `fixtures/verify-tests` with the fake runner committed): the fixture's
//! bindings and tests as "Data" "Fixture" names them, the golden's blocks
//! computed from the spec, a passing run and what its JSON `lock` says.
//!
//! Only the 8b-1 test binaries over the kiln include this module (`mod
//! tested_kiln;`, after `mod common;`, `mod lock_common;`, `mod
//! verify_common;`, `mod verify_tests_common;` and `mod tested_common;`).
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::path::Path;

use serde_json::Value;

use crate::tested_common::{Block, Bound, DAY, bound_hash, item, module};
use crate::verify_tests_common::{
    GLAZE, HOLD, KILN_MD, Kiln, RAMP, RAMP_STOPS, SCHEDULE, SCHEDULE_MODULE, TWIN_NOT_UNIX,
    TWIN_UNIX, TestsRun,
};

/// `next_temperature`'s qpath (`@implements KILN-RAMP`).
pub const NEXT: &str = "crates/kiln::ramp::next_temperature";

/// The fixture's own bindings of `node` (expected.json `nodes[].bindings`)
/// as `root`'s files are now: every relation, an alias marker under the
/// canonical ID, the module marker too.
pub fn kiln_bindings(root: &Path, node: &str) -> Vec<Bound> {
    match node {
        "CONE-01" => vec![
            item(
                root,
                node,
                "verifies",
                RAMP,
                TWIN_UNIX,
                "peak_is_capped",
                Some("test,unix"),
            ),
            item(
                root,
                node,
                "verifies",
                RAMP,
                TWIN_NOT_UNIX,
                "peak_is_capped",
                Some("test,not(unix)"),
            ),
        ],
        "KILN-HOLD" => vec![
            module(root, node, "verifies", SCHEDULE, SCHEDULE_MODULE),
            item(
                root,
                node,
                "verifies",
                SCHEDULE,
                HOLD,
                "hold_follows_the_ramp",
                None,
            ),
        ],
        "KILN-RAMP" => vec![
            item(
                root,
                node,
                "implements",
                RAMP,
                NEXT,
                "next_temperature",
                None,
            ),
            item(
                root,
                node,
                "verifies",
                RAMP,
                RAMP_STOPS,
                "ramp_stops_at_peak",
                None,
            ),
            item(
                root,
                node,
                "verifies",
                SCHEDULE,
                HOLD,
                "hold_follows_the_ramp",
                None,
            ),
        ],
        "glaze-check/GLZ-01" => vec![item(
            root,
            node,
            "verifies",
            SCHEDULE,
            GLAZE,
            "glaze_survives_the_hold",
            None,
        )],
        other => panic!("no fixture bindings for {other}"),
    }
}

/// The tests a passing full run lists in `node`'s block, by (file, symbol)
/// bytes (Data "Fixture").
pub fn kiln_tests(node: &str) -> Vec<(&'static str, &'static str)> {
    match node {
        "CONE-01" => vec![(RAMP, TWIN_NOT_UNIX), (RAMP, TWIN_UNIX)],
        "KILN-HOLD" => vec![(SCHEDULE, HOLD)],
        "KILN-RAMP" => vec![(RAMP, RAMP_STOPS), (SCHEDULE, HOLD)],
        "glaze-check/GLZ-01" => vec![(SCHEDULE, GLAZE)],
        other => panic!("no fixture tests for {other}"),
    }
}

/// The node's revision in the fixture (expected.json).
pub fn kiln_rev(node: &str) -> u32 {
    match node {
        "KILN-COOL" => 2,
        _ => 1,
    }
}

/// The node's document (expected.json).
pub fn kiln_doc(node: &str) -> &'static str {
    match node {
        "CONE-01" => "docs/records/CONE-01.md",
        "glaze-check/GLZ-01" => "docs/features/glaze-check.md",
        _ => KILN_MD,
    }
}

/// The blocks a passing run at `kiln`'s `HEAD` writes for `nodes`, built
/// from the spec: `rev` the fixture's, `spec_hash` `spec show`'s norm hash,
/// `bound_hash` over [`kiln_bindings`] of the files now, `tests`
/// [`kiln_tests`], `commit` `HEAD`, `verified_at` [`DAY`].
pub fn expected_blocks(kiln: &Kiln, nodes: &[&str]) -> Vec<Block> {
    let head = kiln.repo.head();
    nodes
        .iter()
        .map(|node| Block {
            node: (*node).to_owned(),
            rev: kiln_rev(node),
            spec_hash: kiln.repo.norm_hash(node),
            bound_hash: bound_hash(&kiln_bindings(kiln.root(), node)),
            tests: kiln_tests(node)
                .into_iter()
                .map(|(file, symbol)| (file.to_owned(), symbol.to_owned()))
                .collect(),
            commit: head.clone(),
            verified_at: DAY.to_owned(),
        })
        .collect()
}

// ----------------------------------------------------------------- runs

/// A passing run (every test passes, consent `y`) that must commit its
/// evidence: the run, the evidence commit.
pub fn pass(kiln: &Kiln) -> (TestsRun, String) {
    let before = kiln.repo.head();
    let run = kiln.run();
    let commit = kiln.repo.head();
    assert_ne!(commit, before, "the evidence committed:\n{}", run.show());
    assert_eq!(
        run.json()["lock"]["commit"],
        Value::String(commit.clone()),
        "{}",
        run.show()
    );
    (run, commit)
}

/// The run's JSON `lock` rows as `(node, change)`.
pub fn lock_rows(run: &TestsRun) -> Vec<(String, String)> {
    run.json()["lock"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("`lock.rows` is an array:\n{}", run.show()))
        .iter()
        .map(|row| {
            (
                row["node"].as_str().expect("a node").to_owned(),
                row["change"].as_str().expect("a change").to_owned(),
            )
        })
        .collect()
}

/// stdout's `lock | <change> | <node>` lines.
pub fn lock_lines(run: &TestsRun) -> Vec<String> {
    run.lines()
        .into_iter()
        .filter(|line| line.starts_with("lock | "))
        .collect()
}

/// `(node, change)` pairs from `&str`s.
pub fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(node, change)| ((*node).to_owned(), (*change).to_owned()))
        .collect()
}

// ------------------------------------------------------------------ RON

/// The RON file a test adds to the kiln: a firing's tuning, its `peak`
/// bound to KILN-RAMP (`@configures`), `hold_minutes` its sibling.
pub const FIRING_RON: &str = "crates/kiln/data/firing.ron";

/// [`FIRING_RON`]'s bytes: the anchored comment on line 3.
pub const FIRING_TEXT: &str = "// Firing tuning.\nFiring(\n    peak: 1200, // @configures KILN-RAMP\n    hold_minutes: 60,\n)\n";

/// [`FIRING_RON`] written and committed.
pub fn add_firing(kiln: &Kiln) {
    kiln.repo.write(FIRING_RON, FIRING_TEXT);
    kiln.repo.commit_all("the firing's tuning");
}

/// `next_temperature` as rustfmt would never leave it: the same tokens on
/// one line.
pub const RAMP_SQUEEZED: (&str, &str) = (
    "pub fn next_temperature(now: u32, rate: u32, peak: u32) -> u32 {\n    (now + rate).min(peak)\n}",
    "pub fn next_temperature(now:u32,rate:u32,peak:u32)->u32{(now+rate).min(peak)}",
);

/// A comment far from every binding of `ramp.rs`.
pub const RAMP_COMMENT: &str = "\n// A note on ramps, bound to nothing.\n";
