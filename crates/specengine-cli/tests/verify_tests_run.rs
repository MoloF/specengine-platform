//! docs/features/verify-tests.md (8a-1) RD-4, RD-5, RD-7's event half,
//! TE-2 and TE-5: the argv each distinct test gets ("Placeholder values",
//! "Rules": bytes as one element, never a shell), the probe ("Order" step
//! 8), the runs (step 9: in the order of their first test, from the root,
//! each its own process group, `spec` its parent), the verdicts
//! ("Verdicts"), a program that cannot start, the consent (`declined`), and
//! the `verify.tests` event ("Event": one per run that reached step 8,
//! never a task transition or a `runs` row). Scratch repositories of
//! `fixtures/verify-tests` (P10) with the fake runner (TE-1); the library
//! with a consent closure. Each test names the mutation that turns it red
//! (`M:`).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;
mod lock_common;
mod verify_tests_common;

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use verify_tests_common::{
    COMMITTER, EVENT, EVENT_KEYS, EVENT_PROBE_KEYS, EVENT_TEST_KEYS, GLAZE, GLAZE_TEST, HOLD,
    HOLD_TEST, Kiln, Ordered, PROBE, PROBE_PASSED, RAMP_STOPS, RAMP_STOPS_TEST, TEMPLATE,
    TWIN_TEST, TWIN_UNIX, args_of, expected, owned, strings, survivors,
};

/// The `{test}` (last argument) of every test invocation.
fn tests_run(kiln: &Kiln) -> Vec<String> {
    kiln.test_invocations()
        .into_iter()
        .map(|args| args.last().cloned().unwrap_or_default())
        .collect()
}

/// `b3:` and 64 lower-case hex digits.
fn is_b3(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        text.strip_prefix("b3:").is_some_and(|hex| {
            hex.len() == 64
                && hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
    })
}

// -------------------------------------------------------------------- RD-4

/// RD-4, TE-5 ("Rules"): a value holding spaces, `;`, `$()`, quotes and
/// `*` (a module file named so) reaches the runner as one argv element
/// each, byte for byte, through `{file}`, `{qpath}` and `{test}`; nothing a
/// shell would do happens (no `pwned` file anywhere). M: a `sh -c` join.
#[test]
fn placeholder_value_never_becomes_shell_syntax() {
    let kiln = Kiln::new("vt-shell");
    let name = "odd name; $(touch pwned) 'q' \"d\" *";
    let file = format!("crates/kiln/src/{name}.rs");
    kiln.repo.write(
        &file,
        "#[cfg(test)]\nmod tests {\n    // @verifies KILN-RAMP\n    #[test]\n    fn shell_check() {}\n}\n",
    );
    kiln.repo.commit_all("an odd module name");
    kiln.set_verify(&["scripts/run-test", "{file}", "{qpath}", "{test}"], None);
    let run = kiln.run();
    let symbol = format!("crates/kiln::{name}::tests::shell_check");
    let test = format!("{name}::tests::shell_check");
    let row = run.row(&symbol);
    let wanted = owned(&[file.as_str(), symbol.as_str(), test.as_str()]);
    assert_eq!(args_of(&row), wanted, "{row}");
    assert_eq!(row["outcome"], "passed", "{row}");
    let calls: Vec<Vec<String>> = kiln
        .test_invocations()
        .into_iter()
        .filter(|args| {
            args.last()
                .is_some_and(|last| last.ends_with("shell_check"))
        })
        .collect();
    assert_eq!(calls, vec![wanted], "one element each, verbatim");
    for dir in [kiln.root(), kiln.vt.as_path(), kiln.repo.scratch.path()] {
        assert!(
            !dir.join("pwned").exists(),
            "a shell ran `$(touch pwned)` in {}",
            dir.display()
        );
    }
}

/// RD-4 (P5): every placeholder comes from the structured name, never
/// from splitting the rendered qpath: in a module file `src/a #1.rs`
/// (whose module is `a #1`), `{test}` is `a #1::tests::…` and `{qpath}`
/// keeps ` #1`; `{package}` is the manifest's name (`kiln-core`, not the
/// directory), `{target}` the layout's (`kiln_core`, `schedule`),
/// `{target_kind}` `lib` or `test`; `{{`/`}}` are literal braces; the
/// probe's `{qpath}` is the test's with the probe as last segment. M:
/// `{test}` cut from the rendered qpath.
#[test]
fn placeholders_come_from_the_structured_name() {
    let kiln = Kiln::new("vt-names");
    kiln.repo.write(
        "crates/kiln/src/a #1.rs",
        "#[cfg(test)]\nmod tests {\n    // @verifies KILN-RAMP\n    #[test]\n    fn hashed_module_check() {}\n}\n",
    );
    kiln.repo.commit_all("a module file with ` #` in its name");
    kiln.set_verify(
        &[
            "scripts/run-test",
            "lit{{}}",
            "{file}",
            "{qpath}",
            "{package}",
            "{target_kind}",
            "{target}",
            "{test}",
        ],
        None,
    );
    let run = kiln.run();
    let cases: [(&str, [&str; 7]); 3] = [
        (
            "crates/kiln::a #1::tests::hashed_module_check",
            [
                "lit{}",
                "crates/kiln/src/a #1.rs",
                "crates/kiln::a #1::tests::hashed_module_check",
                "kiln-core",
                "lib",
                "kiln_core",
                "a #1::tests::hashed_module_check",
            ],
        ),
        (
            TWIN_UNIX,
            [
                "lit{}",
                "crates/kiln/src/ramp.rs",
                "crates/kiln::ramp::tests::peak_is_capped",
                "kiln-core",
                "lib",
                "kiln_core",
                TWIN_TEST,
            ],
        ),
        (
            HOLD,
            [
                "lit{}",
                "crates/kiln/tests/schedule.rs",
                HOLD,
                "kiln-core",
                "test",
                "schedule",
                HOLD_TEST,
            ],
        ),
    ];
    let calls = kiln.test_invocations();
    for (symbol, args) in cases {
        let wanted = owned(&args);
        assert_eq!(args_of(&run.row(symbol)), wanted, "{symbol}");
        assert_eq!(
            calls.iter().filter(|call| **call == wanted).count(),
            1,
            "{symbol} run once: {calls:?}"
        );
    }
    let probe_qpath = format!("crates/kiln::a #1::tests::{PROBE}");
    let probe = owned(&[
        "lit{}",
        "crates/kiln/src/a #1.rs",
        probe_qpath.as_str(),
        "kiln-core",
        "lib",
        "kiln_core",
        PROBE,
    ]);
    assert!(
        kiln.probe_invocations().contains(&probe),
        "the probe keeps the test's other values: {:?}",
        kiln.probe_invocations()
    );
}

/// TE-5 ("Runnable"): a value holding a control character (a module file
/// whose name holds a newline) is `not run | … | control character`, never
/// started. M: the value passed (or the binding dropped).
#[test]
fn newline_in_a_value_is_not_run() {
    let kiln = Kiln::new("vt-newline");
    kiln.repo.write(
        "docs/spec/coil.md",
        "---\nclass: canon\nid: KILN-COIL\nowner: owner\nreviewed: 2026-10-16\n---\n\n# Coil\n\nThe coil heats the kiln.\n",
    );
    kiln.repo.write(
        "crates/kiln/src/new\nline.rs",
        "#[cfg(test)]\nmod tests {\n    // @verifies KILN-COIL\n    #[test]\n    fn newline_check() {}\n}\n",
    );
    kiln.repo
        .commit_all("a module file with a newline in its name");
    let run = kiln.run();
    let row = run.row("crates/kiln::new\nline::tests::newline_check");
    assert_eq!(
        (&row["outcome"], &row["reason"], &row["nodes"]),
        (
            &json!("not_run"),
            &json!("control character"),
            &json!(["KILN-COIL"])
        ),
        "{row}"
    );
    assert!(
        !tests_run(&kiln)
            .iter()
            .any(|test| test.contains("newline_check")),
        "{:?}",
        kiln.log()
    );
}

/// RD-4 ("Order" step 8): a runner exiting 0 for the probe (a test that
/// does not exist) is `refused`, exit 2, with the spec's text, after the
/// question; no test starts; every runnable row reads `not run | … |
/// refused`. M: the probe skipped.
#[test]
fn runner_passing_an_empty_match_is_refused() {
    let kiln = Kiln::new("vt-probe-passes");
    kiln.plan(&[("*probe", "pass")]);
    let run = kiln.run();
    run.code(2);
    let doc = run.json();
    assert_eq!(
        (&doc["verdict"], &doc["reason"]),
        (&json!("refused"), &json!(PROBE_PASSED)),
        "{}",
        run.show()
    );
    assert_eq!(
        run.summary(),
        format!("verify --tests: refused: {PROBE_PASSED}")
    );
    assert_eq!(run.prompts.len(), 1, "the question comes first");
    assert!(!kiln.probe_invocations().is_empty(), "{:?}", kiln.log());
    assert!(
        kiln.test_invocations().is_empty(),
        "no test after a passing probe: {:?}",
        kiln.log()
    );
    for symbol in [RAMP_STOPS, HOLD, GLAZE, TWIN_UNIX] {
        let row = run.row(symbol);
        assert_eq!(
            (&row["outcome"], &row["reason"]),
            (&json!("not_run"), &json!("refused")),
            "{row}"
        );
    }
}

/// "Order" step 8: a probe that outlives `timeout` is `refused`, exit 2,
/// naming the time-out; no test starts and the probe's group is gone. M: a
/// timed-out probe read as a failing one (the tests run).
#[test]
fn probe_timing_out_is_refused() {
    let kiln = Kiln::new("vt-probe-hangs");
    let reaper = kiln.reaper();
    kiln.set_verify(&TEMPLATE, Some(2));
    kiln.plan(&[("*probe", "hang")]);
    let started = Instant::now();
    let run = kiln.run();
    run.code(2);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    let doc = run.json();
    assert_eq!(doc["verdict"], "refused", "{}", run.show());
    let reason = doc["reason"].as_str().expect("a reason").to_lowercase();
    assert!(
        reason.contains("timed out") || reason.contains("time-out") || reason.contains("timeout"),
        "{reason}"
    );
    assert!(kiln.test_invocations().is_empty(), "{:?}", kiln.log());
    let pids: Vec<u32> = kiln.log().iter().map(|call| call.pid).collect();
    let left = survivors(&pids, Duration::from_secs(3));
    assert!(left.is_empty(), "the probe is gone: {left:?}");
    drop(reaper);
}

/// "Runner": a program that cannot start is `refused`, exit 2, with
/// ``the [verify] tests program `<elem0>` cannot start: <error>``; the run is
/// recorded (it reached step 8). `ENOENT`: a program looked up on `PATH`
/// (no `/`, so step 6's check of programs at `HEAD` does not apply) that
/// no directory holds; `EACCES`: a committed `scripts/locked-runner`
/// without its execute bit. A relative program with a `/` that `HEAD`
/// lacks is step 6's `cannot` (`verify_tests_targets.rs`). M: a spawn
/// failure read as a failed test.
#[test]
fn runner_that_cannot_start_is_refused() {
    let kiln = Kiln::new("vt-cannot-start");
    kiln.write_runner("scripts/locked-runner", 0o644);
    kiln.repo.commit_all("a runner that is not executable");
    for (program, error) in [
        (
            "specengine-no-such-runner",
            ["No such file or directory", "os error 2"],
        ),
        (
            "scripts/locked-runner",
            ["Permission denied", "os error 13"],
        ),
    ] {
        kiln.set_verify(&[program, "{test}"], None);
        let before = kiln.events().len();
        let run = kiln.run();
        run.code(2);
        let doc = run.json();
        assert_eq!(doc["verdict"], "refused", "{program}: {}", run.show());
        let reason = doc["reason"].as_str().expect("a reason");
        assert!(
            reason.starts_with(&format!(
                "the [verify] tests program `{program}` cannot start: "
            )) && error.iter().all(|words| reason.contains(words)),
            "{program}: {reason}"
        );
        kiln.assert_nothing_started(program);
        assert_eq!(kiln.events().len(), before + 1, "{program}: recorded");
    }
}

// -------------------------------------------------------------------- RD-5

/// RD-5 ("Verdicts"): a test exiting non-zero or killed by a signal is
/// `failed` (`exit 101`, `signal 9`), its nodes `failed`, the run
/// `unverified`, exit 1; the counts say so. M: exit ignored.
#[test]
fn failing_test_never_verifies() {
    let kiln = Kiln::new("vt-failing");
    kiln.plan(&[(RAMP_STOPS_TEST, "fail"), (GLAZE_TEST, "killed")]);
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.verdict(), "unverified");
    let ramp = run.row(RAMP_STOPS);
    assert_eq!(
        (&ramp["outcome"], &ramp["exit"], &ramp["signal"]),
        (&json!("failed"), &json!(101), &Value::Null),
        "{ramp}"
    );
    let glaze = run.row(GLAZE);
    assert_eq!(
        (&glaze["outcome"], &glaze["exit"], &glaze["signal"]),
        (&json!("failed"), &Value::Null, &json!(9)),
        "{glaze}"
    );
    run.has_line(
        "failed | crates/kiln/src/ramp.rs | item crates/kiln::ramp::tests::ramp_stops_at_peak | KILN-RAMP | exit 101",
    );
    run.has_line(
        "failed | crates/kiln/tests/schedule.rs | item crates/kiln::test:schedule::glaze_survives_the_hold | glaze-check/GLZ-01 | signal 9",
    );
    run.has_line("node KILN-RAMP: failed");
    run.has_line("node glaze-check/GLZ-01: failed");
    run.has_line("node CONE-01: verified");
    assert_eq!(
        run.summary(),
        "verify --tests: unverified: 5 nodes (2 verified, 2 failed, 1 not verifiable, 0 no runnable test); 7 tests (3 passed, 2 failed, 0 timed out, 2 not run)"
    );
}

/// RD-5 ("Verdicts", "Runner": overrun): a test that outlives `timeout` is
/// `timed out | … | after 2 s`, never `passed`; its node `failed`; the
/// runner's process is gone. M: a time-out as a pass.
#[test]
fn timed_out_test_is_not_passed() {
    let kiln = Kiln::new("vt-timeout");
    let reaper = kiln.reaper();
    kiln.set_verify(&TEMPLATE, Some(2));
    kiln.plan(&[(RAMP_STOPS_TEST, "hang")]);
    let started = Instant::now();
    let run = kiln.run();
    run.code(1);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    let row = run.row(RAMP_STOPS);
    assert_eq!(row["outcome"], "timed_out", "{row}");
    run.has_line(
        "timed out | crates/kiln/src/ramp.rs | item crates/kiln::ramp::tests::ramp_stops_at_peak | KILN-RAMP | after 2 s",
    );
    assert_eq!(run.node("KILN-RAMP")["verdict"], "failed");
    assert_eq!(run.verdict(), "unverified");
    assert_eq!(run.json()["counts"]["tests"]["timed_out"], 1);
    let pids: Vec<u32> = kiln.noted_pids().into_iter().map(|(_, pid)| pid).collect();
    assert!(!pids.is_empty(), "the hanging runner noted itself");
    let left = survivors(&pids, Duration::from_secs(3));
    assert!(left.is_empty(), "the runner is gone: {left:?}");
    drop(reaper);
}

// -------------------------------------------------------------- Order, TE-2

/// "Order" steps 8-9: the probes run first, one per distinct probe argv;
/// then each distinct argv once, in the order of its first test by
/// (`file`, `symbol`) bytes (`expected.json` `probes`, `run_order`); every
/// invocation from the root, its own process group, `spec` its parent (no
/// shell between). M: the probes after a test, or a run order by node.
#[test]
fn runs_follow_the_order_of_their_first_test() {
    let kiln = Kiln::new("vt-order");
    let run = kiln.run();
    run.code(1);
    let expected = expected();
    let log = kiln.log();
    let probes: Vec<Vec<String>> = expected["probes"]
        .as_array()
        .expect("probes")
        .iter()
        .map(|argv| strings(argv)[1..].to_vec())
        .collect();
    let first_test = log
        .iter()
        .position(|call| !call.is_probe())
        .expect("a test ran");
    let mut seen: Vec<Vec<String>> = log[..first_test]
        .iter()
        .map(|call| call.args.clone())
        .collect();
    seen.sort();
    let mut wanted = probes.clone();
    wanted.sort();
    assert_eq!(seen, wanted, "every probe, once, before any test");
    assert!(log[first_test..].iter().all(|call| !call.is_probe()));
    let order: Vec<Vec<String>> = expected["run_order"]
        .as_array()
        .expect("run_order")
        .iter()
        .map(|argv| strings(argv)[1..].to_vec())
        .collect();
    assert_eq!(kiln.test_invocations(), order);
    assert_eq!(run.json()["counts"]["probes"], json!(probes.len()));
    let root = kiln.root().display().to_string();
    for call in &log {
        assert_eq!(call.cwd, root, "{call:?}");
        assert_eq!(call.pgid, call.pid, "its own process group: {call:?}");
        assert_eq!(call.ppid, std::process::id(), "spec started it: {call:?}");
    }
}

/// TE-2 (AC-8, docs/features/tested-evidence.md RD-5): a task's tests that
/// all pass give `verified`, exit 0, the event `verified`; the run commits
/// its evidence: one commit on top of `HEAD` changing `spec.lock` alone,
/// KILN-RAMP's block in it (no other node's), JSON `lock` its row and the
/// commit, the event's `lock_commit` that commit, the tree clean. M: no
/// lock written, or a passing set not `verified`.
#[test]
fn passing_task_commits_its_evidence() {
    let kiln = Kiln::new("vt-verified");
    let task = kiln.new_task(&["KILN-RAMP"]);
    let head = kiln.repo.head();
    let run = kiln.run_task(&task);
    run.code(0);
    assert_eq!(run.verdict(), "verified");
    assert_eq!(run.node("KILN-RAMP")["verdict"], "verified");
    let commit = kiln.repo.head();
    assert_ne!(commit, head, "the evidence committed");
    assert_eq!(kiln.repo.parents(&commit), [head]);
    assert_eq!(kiln.repo.changed(&commit), ["spec.lock"]);
    let lock = kiln.repo.lock_text();
    assert_eq!(lock.lines().nth(1), Some("format = 3"));
    assert_eq!(lock.matches("\n[[tested]]\n").count(), 1, "{lock}");
    assert!(
        lock.contains("\n[[tested]]\nnode = \"KILN-RAMP\"\n"),
        "{lock}"
    );
    assert!(!lock.contains("[[binding]]"), "no entry");
    assert_eq!(
        run.json()["lock"],
        json!({
            "rows": [{"node": "KILN-RAMP", "change": "added"}],
            "commit": commit,
            "unchanged": false,
            "refusal": null,
        })
    );
    assert!(kiln.repo.porcelain().is_empty(), "the tree is clean");
    let event = kiln.one_event();
    assert_eq!(
        (&event["verdict"], &event["task"], &event["lock_commit"]),
        (&json!("verified"), &json!(task), &json!(commit))
    );
}

/// TE-2: the owner's `n` is `declined`, exit 1: nothing started, no event;
/// the listing and the question were shown; every runnable row reads `not
/// run | … | declined`. M: a run or an event after `n`.
#[test]
fn declined_run_starts_nothing_and_records_nothing() {
    let kiln = Kiln::new("vt-declined");
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    run.code(1);
    assert_eq!(run.verdict(), "declined");
    assert!(
        run.summary().starts_with("verify --tests: declined"),
        "{}",
        run.summary()
    );
    assert_eq!(run.prompts.len(), 1);
    assert!(run.question().ends_with("? [y/N]"), "{}", run.question());
    kiln.assert_nothing_started("declined");
    assert!(kiln.events().is_empty(), "no event: {:?}", kiln.events());
    for symbol in [RAMP_STOPS, HOLD] {
        assert_eq!(run.row(symbol)["reason"], "declined");
    }
}

/// TE-2 (07:38): an agent's `report_run` (`spec task report`) records a
/// run of the task and nothing else: no `verify.tests` event, no runner
/// started. M: `report_run` running or recording tests.
#[test]
fn report_run_records_no_test_run() {
    let kiln = Kiln::new("vt-report-run");
    let task = kiln.new_task(&["KILN-RAMP"]);
    kiln.approve_task(&task);
    kiln.claim_task(&task);
    kiln.report_task(&task);
    let types = kiln.event_types();
    assert!(types.contains(&"task.run_reported".to_owned()), "{types:?}");
    assert!(!types.contains(&EVENT.to_owned()), "{types:?}");
    kiln.assert_nothing_started("report_run");
}

// -------------------------------------------------------------------- RD-7

/// RD-7 ("Event"): a refused run (the probe passed) is recorded: one
/// `verify.tests`, its keys in the spec's order, `verdict` `refused`, the
/// probes with `exit` 0 and a `b3:` output hash, no test run, `lock_commit`
/// null, `by` the committer, `worktree` the root; the queue's dump keeps
/// it. M: none on a refused probe.
#[test]
fn refused_run_is_recorded() {
    let kiln = Kiln::new("vt-refused-event");
    kiln.plan(&[("*probe", "pass")]);
    let run = kiln.run();
    run.code(2);
    let events = kiln.events();
    assert_eq!(events.len(), 1, "{events:#?}");
    let ordered = Ordered::of(&events[0]);
    assert_eq!(ordered.keys(), owned(&EVENT_KEYS), "{}", events[0]);
    let event: Value = serde_json::from_str(&events[0]).expect("JSON");
    assert_eq!(
        (
            &event["task"],
            &event["worktree"],
            &event["branch"],
            &event["commit"],
            &event["command"],
            &event["timeout"],
            &event["verdict"],
            &event["lock_commit"],
            &event["by"],
        ),
        (
            &Value::Null,
            &json!(kiln.root().display().to_string()),
            &json!("main"),
            &json!(kiln.repo.head()),
            &json!(TEMPLATE),
            &json!(600),
            &json!("refused"),
            &Value::Null,
            &json!(COMMITTER),
        )
    );
    let probes = event["probes"].as_array().expect("probes");
    assert!(!probes.is_empty(), "{event}");
    for (probe, keys) in probes.iter().zip(ordered.get("probes").items()) {
        assert_eq!(keys.keys(), owned(&EVENT_PROBE_KEYS), "{probe}");
        assert_eq!(
            (&probe["exit"], &probe["signal"]),
            (&json!(0), &Value::Null)
        );
        assert!(is_b3(&probe["output_hash"]), "{probe}");
        assert_eq!(
            strings(&probe["argv"]).last().map(String::as_str),
            Some(PROBE)
        );
    }
    for (test, keys) in event["tests"]
        .as_array()
        .expect("tests")
        .iter()
        .zip(ordered.get("tests").items())
    {
        assert_eq!(keys.keys(), owned(&EVENT_TEST_KEYS), "{test}");
        assert_eq!(test["exit"], Value::Null, "no test ran: {test}");
    }
    assert!(
        kiln.dump()
            .lines()
            .any(|line| line.starts_with("events\t") && line.contains(EVENT)),
        "the dump keeps the event"
    );
}

/// RD-7 ("Rules": nothing blocks; ADR-0044): a run with `--task` of a
/// claimed task moves nothing: the task's status, revision, snapshot,
/// claim and its `runs` rows are the dump's as before; the only new row is
/// one `verify.tests` event naming the task. M: a transition or `runs` row.
#[test]
fn test_run_moves_no_task() {
    let kiln = Kiln::new("vt-no-transition");
    let task = kiln.new_task(&["KILN-RAMP"]);
    kiln.approve_task(&task);
    kiln.claim_task(&task);
    let rows = |dump: &str| -> Vec<String> {
        dump.lines()
            .filter(|line| !line.starts_with("events\t"))
            .map(str::to_owned)
            .collect()
    };
    let events = |dump: &str| -> Vec<String> {
        dump.lines()
            .filter(|line| line.starts_with("events\t"))
            .map(str::to_owned)
            .collect()
    };
    let before = kiln.dump();
    let run = kiln.run_task(&task);
    run.code(0);
    let after = kiln.dump();
    assert_eq!(
        rows(&after),
        rows(&before),
        "no task, run or proposal row changed"
    );
    let (old, new) = (events(&before), events(&after));
    assert_eq!(new.len(), old.len() + 1, "{new:#?}");
    assert_eq!(new[..old.len()], old[..]);
    assert!(new[old.len()].contains(EVENT), "{}", new[old.len()]);
    assert_eq!(kiln.one_event()["task"], json!(task));
    assert!(tests_run(&kiln).contains(&HOLD_TEST.to_owned()));
}
