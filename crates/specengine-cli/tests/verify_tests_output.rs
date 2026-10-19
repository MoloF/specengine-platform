//! docs/features/verify-tests.md (8a-1) RD-8, TE-7 and TE-1's oracle: what
//! `spec verify --tests` prints ("Output": the text rows by (`file`,
//! `symbol`), the node lines by ID, the summary; the JSON document, every
//! key present, no timing, output hash, tail or worktree), the step-7
//! listing and question, and the `verify.tests` payload's keys ("Event"),
//! against `fixtures/verify-tests/expected.json` (built from the spec,
//! never from the command's output); byte-identical from two roots,
//! another cwd and a copy made in reverse creation order. Scratch
//! repositories with the fake runner (TE-1), the library with a consent
//! closure. Each test names the mutation that turns it red (`M:`).

mod common;
mod lock_common;
mod verify_tests_common;

use serde_json::{Value, json};
use specengine_store::b3_hash;
use verify_tests_common::{
    COMMITTER, COUNTS_KEYS, EVENT_KEYS, EVENT_PROBE_KEYS, EVENT_TEST_KEYS, Kiln, NODE_COUNT_KEYS,
    NODE_KEYS, Ordered, RAMP_STOPS, RAMP_STOPS_TEST, TEMPLATE, TEST_COUNT_KEYS, TEST_KEYS,
    TOP_KEYS, TestsRun, expected, owned, strings,
};

/// The JSON document's keys at every level, in order, against the spec's.
fn assert_document_keys(run: &TestsRun, context: &str) {
    let text = run.json_text();
    let doc = Ordered::of(&text);
    assert_eq!(doc.keys(), owned(&TOP_KEYS), "{context}: {text}");
    let counts = doc.get("counts");
    assert_eq!(counts.keys(), owned(&COUNTS_KEYS), "{context}");
    assert_eq!(
        counts.get("nodes").keys(),
        owned(&NODE_COUNT_KEYS),
        "{context}"
    );
    assert_eq!(
        counts.get("tests").keys(),
        owned(&TEST_COUNT_KEYS),
        "{context}"
    );
    for row in doc.get("tests").items() {
        assert_eq!(row.keys(), owned(&TEST_KEYS), "{context}: a test row");
    }
    for node in doc.get("nodes").items() {
        assert_eq!(node.keys(), owned(&NODE_KEYS), "{context}: a node row");
    }
}

/// TE-1's oracle ("Output", "Order" step 7): the fixture run with every
/// runnable test passing gives `expected.json`'s listing, question,
/// stdout, counts, nodes and rows: each test's file, kind, symbol, set
/// nodes, argv (the template expanded from the structured name) and
/// outcome or reason; `task` null, `branch` `main`, `commit` HEAD, the
/// template and the default timeout, `lock` null, no cause, no note. M:
/// any row, line or count off the spec.
#[test]
fn fixture_run_matches_expected_json() {
    let kiln = Kiln::new("vt-oracle");
    let expected = expected();
    let all = &expected["all_passed"];
    let run = kiln.run();
    run.code(i32::try_from(all["exit"].as_i64().expect("an exit")).expect("small"));

    assert_eq!(run.listing(), strings(&expected["listing"]), "the listing");
    let question = run.question();
    let template = expected["question"].as_str().expect("the question");
    let (head, rest) = template.split_once("<template>").expect("<template>");
    let tail = rest
        .replace("<worktree>", &kiln.root().display().to_string())
        .replace("<commit>", &kiln.repo.head());
    assert!(
        question.starts_with(head) && question.ends_with(&tail),
        "{question}\nwanted {head}…{tail}"
    );

    assert_eq!(
        run.lines(),
        strings(&all["stdout"]),
        "stdout:\n{}",
        run.text()
    );

    let doc = run.json();
    assert_eq!(
        (
            &doc["task"],
            &doc["branch"],
            &doc["commit"],
            &doc["command"],
            &doc["timeout"],
            &doc["verdict"],
            &doc["reason"],
            &doc["lock"],
            &doc["cannot"],
            &doc["notes"],
        ),
        (
            &Value::Null,
            &json!("main"),
            &json!(kiln.repo.head()),
            &expected["command"],
            &expected["timeout"],
            &all["verdict"],
            &Value::Null,
            &Value::Null,
            &json!([]),
            &json!([]),
        )
    );
    assert_eq!(doc["counts"], all["counts"]);
    assert_eq!(doc["nodes"], all["nodes"]);
    let rows = doc["tests"].as_array().expect("tests");
    let wanted = expected["tests"].as_array().expect("tests");
    assert_eq!(rows.len(), wanted.len(), "{rows:#?}");
    for (row, want) in rows.iter().zip(wanted) {
        let context = want["symbol"].to_string();
        for key in ["file", "kind", "symbol", "nodes"] {
            assert_eq!(row[key], want[key], "{context} {key}");
        }
        if want["argv"].is_null() {
            assert_eq!(
                (&row["outcome"], &row["reason"], &row["exit"]),
                (&json!("not_run"), &want["reason"], &Value::Null),
                "{context}"
            );
        } else {
            assert_eq!(row["argv"], want["argv"], "{context}");
            assert_eq!(
                (
                    &row["outcome"],
                    &row["exit"],
                    &row["signal"],
                    &row["reason"]
                ),
                (&json!("passed"), &json!(0), &Value::Null, &Value::Null),
                "{context}"
            );
        }
    }
    assert_document_keys(&run, "the oracle run");
}

/// RD-8 (S14): every verdict's document has every key, in the spec's
/// order, at every level (absent: `null`): `verified`, `unverified`,
/// `declined`, `dirty`, `empty`, `changed`, `refused`, and `cannot` after
/// discovery (exit 2 still prints one document). M: a key dropped.
#[test]
fn verify_tests_json_every_key_present() {
    let kiln = Kiln::new("vt-keys");
    let task = kiln.new_task(&["KILN-RAMP"]);
    let cool = kiln.new_task(&["KILN-COOL"]);
    let mut seen = Vec::new();
    let mut check = |run: TestsRun, verdict: &str| {
        assert_eq!(run.verdict(), verdict, "{}", run.show());
        assert_document_keys(&run, verdict);
        seen.push(verdict.to_owned());
    };
    check(kiln.run_task(&task), "verified");
    check(kiln.run(), "unverified");
    check(kiln.run_with(kiln.root(), None, false, &[]), "declined");
    check(kiln.run_task(&cool), "empty");
    kiln.plan(&[("*probe", "pass")]);
    check(kiln.run(), "refused");
    kiln.plan(&[(RAMP_STOPS_TEST, "write-tree")]);
    check(kiln.run(), "changed");
    check(kiln.run(), "dirty");
    std::fs::remove_file(kiln.root().join("firing-notes.txt")).expect("the written file");
    kiln.set_verify_table("[verify]\nmode = \"observe\"\n", "no tests key");
    let run = kiln.run();
    run.code(2);
    check(run, "cannot");
    assert_eq!(seen.len(), 8);
}

/// RD-8 ("Output": no timing): a run whose test takes 1.5 s and one whose
/// test takes no time print the same bytes, text and JSON; no key names a
/// duration. M: timing in the document.
#[test]
fn verify_tests_json_has_no_timing() {
    let kiln = Kiln::new("vt-timing");
    kiln.plan(&[(RAMP_STOPS_TEST, "slow")]);
    let slow = kiln.run();
    kiln.plan(&[]);
    let fast = kiln.run();
    assert_eq!(slow.text(), fast.text(), "text");
    assert_eq!(slow.json_text(), fast.json_text(), "JSON");
    let lower = fast.json_text().to_lowercase();
    for word in ["duration", "elapsed", "_ms", "started", "ended", "seconds"] {
        assert!(!lower.contains(word), "{word} in {lower}");
    }
}

/// TE-7 ("Rules": two roots, another cwd, reverse creation order): stdout
/// and JSON byte-identical, the listing too; the question differs only by
/// the worktree it names; no absolute path printed. M: an absolute path or
/// a walk order in the output.
#[test]
fn verify_tests_output_is_byte_identical_across_roots_and_order() {
    let first = Kiln::new("vt-det-a");
    let second = Kiln::at("vt-det-b", "elsewhere/deeper");
    let third = Kiln::reversed("vt-det-c");
    assert_eq!(second.repo.head(), first.repo.head(), "the same commits");
    assert_eq!(third.repo.head(), first.repo.head(), "the same commits");
    let reference = first.run();
    let question = |kiln: &Kiln, run: &TestsRun| {
        run.question()
            .replace(&kiln.root().display().to_string(), "<worktree>")
    };
    let reference_question = question(&first, &reference);
    let runs = [
        ("a repeat run", &first, first.run()),
        ("another root", &second, second.run()),
        (
            "another cwd",
            &second,
            second.run_with(&second.root().join("crates").join("kiln"), None, true, &[]),
        ),
        ("reverse creation order", &third, third.run()),
    ];
    for (context, kiln, run) in &runs {
        assert_eq!(run.text(), reference.text(), "{context}: text");
        assert_eq!(run.json_text(), reference.json_text(), "{context}: JSON");
        assert_eq!(run.listing(), reference.listing(), "{context}: listing");
        assert_eq!(
            question(kiln, run),
            reference_question,
            "{context}: question"
        );
        assert_eq!(
            run.stderr_lines(),
            reference.stderr_lines(),
            "{context}: stderr"
        );
    }
    let root = first.root().display().to_string();
    assert!(
        !reference.text().contains(&root) && !reference.json_text().contains(&root),
        "no absolute path printed"
    );
    assert!(
        !reference.json_text().contains("worktree"),
        "the document names no worktree"
    );
}

/// RD-7 ("Event"): the fixture run's `verify.tests` payload has the spec's
/// keys in order at every level; each run test's `output_hash` is the
/// BLAKE3 of its whole (here empty) output and its `tail` null; a test
/// not run has no hash; the node verdicts are the document's; `by` the
/// committer, `worktree` the root. M: a key dropped or the order changed.
#[test]
fn event_payload_keys_follow_the_spec() {
    let kiln = Kiln::new("vt-event-keys");
    let run = kiln.run();
    run.code(1);
    let events = kiln.events();
    assert_eq!(events.len(), 1, "{events:#?}");
    let ordered = Ordered::of(&events[0]);
    assert_eq!(ordered.keys(), owned(&EVENT_KEYS));
    for probe in ordered.get("probes").items() {
        assert_eq!(probe.keys(), owned(&EVENT_PROBE_KEYS));
    }
    for test in ordered.get("tests").items() {
        assert_eq!(test.keys(), owned(&EVENT_TEST_KEYS));
    }
    for node in ordered.get("nodes").items() {
        assert_eq!(node.keys(), owned(&NODE_KEYS));
    }
    let event: Value = serde_json::from_str(&events[0]).expect("JSON");
    assert_eq!(
        (
            &event["worktree"],
            &event["command"],
            &event["by"],
            &event["verdict"],
            &event["lock_commit"],
        ),
        (
            &json!(kiln.root().display().to_string()),
            &json!(TEMPLATE),
            &json!(COMMITTER),
            &json!("unverified"),
            &Value::Null,
        )
    );
    assert_eq!(event["nodes"], run.json()["nodes"]);
    assert_eq!(event["probes"].as_array().map(Vec::len), Some(2));
    for test in event["tests"].as_array().expect("tests") {
        if test["outcome"] == "passed" {
            assert_eq!(test["output_hash"], json!(b3_hash(b"")), "{test}");
            assert_eq!(test["tail"], Value::Null, "{test}");
        } else {
            assert_eq!(test["outcome"], "not_run", "{test}");
            assert_eq!(
                (&test["output_hash"], &test["exit"]),
                (&Value::Null, &Value::Null),
                "{test}"
            );
        }
    }
    let ramp = event["tests"]
        .as_array()
        .expect("tests")
        .iter()
        .find(|test| test["symbol"] == RAMP_STOPS)
        .expect("the ramp test")
        .clone();
    assert_eq!(
        (&ramp["exit"], &ramp["nodes"]),
        (&json!(0), &json!(["KILN-RAMP"]))
    );
}
