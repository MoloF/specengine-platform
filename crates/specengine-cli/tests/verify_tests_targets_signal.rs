//! docs/features/verify-tests.md (8a-1), review 1's V1 under a forwarded
//! signal (`vt-review-iter1.md`, deviation 4 rejected), a target written
//! before iteration 2's code: red by design on iteration 1.
//!
//! An `interrupted` run did not complete step 9, so no node gets a
//! verdict: `nodes: []` in the document and the event, no `node` line on
//! stdout. As in `verify_tests_signals.rs`, the test re-runs its own
//! binary as the `spec` stand-in (`rerun`) and the fake runner sends
//! `SIGINT` to its parent; this binary holds this one test only, and the
//! `Reaper` kills what is left even when an assertion fails.

mod common;
mod lock_common;
mod verify_tests_common;

use std::time::Duration;

use serde_json::json;
use verify_tests_common::{Kiln, TEMPLATE, TWIN_TEST, child_role, rerun, survivors};

/// V1, red by design on iteration 1: an `interrupted` run reports no node
/// verdict (`nodes: []`, document and event; no `node` line), and no
/// process of the group outlives `spec`. M: `failed` nodes when the run
/// was stopped.
#[test]
fn interrupted_run_reports_no_node_verdict() {
    if child_role() {
        return;
    }
    let kiln = Kiln::new("vt-v1-interrupt");
    let _reaper = kiln.reaper();
    kiln.set_verify(&TEMPLATE, Some(30));
    kiln.plan(&[(TWIN_TEST, "interrupt")]);
    let child = rerun(
        &kiln,
        "interrupted_run_reports_no_node_verdict",
        None,
        b"",
        Duration::from_secs(90),
    );
    assert!(child.status.success(), "{}", child.show());
    assert_eq!(child.code, 2, "{}", child.show());
    let doc = child.json();
    assert_eq!(doc["verdict"], "interrupted", "{}", child.json_text);
    assert_eq!(doc["nodes"], json!([]), "{}", child.json_text);
    assert_eq!(doc["counts"]["nodes"]["failed"], 0, "{}", doc["counts"]);
    assert!(
        !child.text.lines().any(|line| line.starts_with("node ")),
        "a node line though the run was stopped:\n{}",
        child.text
    );
    assert_eq!(kiln.one_event()["nodes"], json!([]));
    let pids: Vec<u32> = kiln.noted_pids().iter().map(|(_, pid)| *pid).collect();
    let left = survivors(&pids, Duration::from_secs(3));
    assert!(left.is_empty(), "no process outlives spec: {left:?}");
}
