//! docs/features/verify-tests.md (8a-1) RD-6's signal half and TE-3: a
//! `SIGINT` to `spec` from step 8 to the end of step 9 is forwarded to the
//! runner's group as `SIGTERM` (then `SIGKILL`); the run is `interrupted`,
//! exit 2, the event written, the tests not started yet `not run |
//! interrupted`, and no process of the group outlives `spec`.
//!
//! The signal must reach `spec` and never this test's own process: the
//! test re-runs its own binary as the `spec` stand-in (`rerun`), and the
//! fake runner sends the `SIGINT` to its parent, the stand-in. A stand-in
//! that dies of the signal shows as a failed status here, and the
//! `Reaper` kills what it left. This binary holds this one test only, so
//! no other test shares a process with an installed handler.

mod common;
mod lock_common;
mod verify_tests_common;

use std::time::Duration;

use serde_json::json;
use verify_tests_common::{
    GLAZE, HOLD, Kiln, RAMP_STOPS, TEMPLATE, TWIN_TEST, child_role, rerun, survivors,
};

/// RD-6, TE-3: the runner of the first test forks a grandchild that
/// ignores `SIGTERM`, then sends `SIGINT` to `spec` and hangs (the
/// timeout is 30 s): `spec` forwards it, the group dies within seconds,
/// the verdict is `interrupted`, exit 2, the event records it; the later
/// tests never start. M: no forwarding (the run waits for the timeout, or
/// `spec` dies of the signal).
#[test]
fn interrupt_kills_the_runner_group() {
    if child_role() {
        return;
    }
    let kiln = Kiln::new("vt-interrupt");
    let _reaper = kiln.reaper();
    kiln.set_verify(&TEMPLATE, Some(30));
    kiln.plan(&[(TWIN_TEST, "interrupt")]);
    let child = rerun(
        &kiln,
        "interrupt_kills_the_runner_group",
        None,
        b"",
        Duration::from_secs(90),
    );
    assert!(
        child.status.success(),
        "spec did not survive the forwarded signal: {}",
        child.show()
    );
    assert_eq!(child.code, 2, "{}", child.show());
    assert!(
        child.elapsed < Duration::from_secs(20),
        "forwarded, not waited out: {:?}",
        child.elapsed
    );
    let doc = child.json();
    assert_eq!(doc["verdict"], "interrupted", "{}", child.json_text);
    for symbol in [RAMP_STOPS, GLAZE, HOLD] {
        let row = doc["tests"]
            .as_array()
            .expect("tests")
            .iter()
            .find(|row| row["symbol"] == symbol)
            .unwrap_or_else(|| panic!("a row {symbol}: {doc}"))
            .clone();
        assert_eq!(
            (&row["outcome"], &row["reason"]),
            (&json!("not_run"), &json!("interrupted")),
            "{row}"
        );
    }
    assert!(
        child
            .text
            .lines()
            .last()
            .is_some_and(|line| line.starts_with("verify --tests: interrupted")),
        "{}",
        child.text
    );
    let ran: Vec<Vec<String>> = kiln.test_invocations();
    assert_eq!(ran.len(), 1, "only the interrupted test started: {ran:?}");
    assert_eq!(kiln.one_event()["verdict"], "interrupted");
    let noted = kiln.noted_pids();
    let roles: Vec<&str> = noted.iter().map(|(role, _)| role.as_str()).collect();
    assert!(
        roles.contains(&"leader") && roles.contains(&"grandchild"),
        "{noted:?}"
    );
    let pids: Vec<u32> = noted.iter().map(|(_, pid)| *pid).collect();
    let left = survivors(&pids, Duration::from_secs(3));
    assert!(
        left.is_empty(),
        "no process of the group outlives spec: {left:?}"
    );
}
