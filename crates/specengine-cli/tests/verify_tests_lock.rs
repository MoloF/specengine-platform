//! docs/features/tested-evidence.md (8b-1) RD-5 ("The run's write"), TE-2
//! (determinism) and TE-3 (failure paths): what `spec verify --tests`
//! writes into `spec.lock` after a completed run, in its own commit under
//! its one `[y/N]`. Scratch repositories of `fixtures/verify-tests` (P10)
//! with 8a's fake runner (`verify_tests_common`); the run through the
//! library with a consent closure. The expected blocks, bytes and commit
//! message are built from "Data" (`tested_common`, `tested_kiln`), never
//! from the command's output. Each test names the mutation that turns it
//! red (`M:`).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use std::time::Duration;

use serde_json::{Value, json};
use tested_common::{
    BLOCK_NODES, IGNORED, LOCK_KEYS, QUESTION_TAIL, block_nodes, block_of, block_value,
    evidence_message, golden_text, lock_with, template_of, tested_blocks, without_blocks,
};
use tested_kiln::{expected_blocks, lock_lines, lock_rows, pass, rows};
use verify_tests_common::{
    COMMITTER, GLAZE_TEST, HOLD_TEST, KILN_MD, Kiln, Ordered, RAMP, RAMP_STOPS, RAMP_STOPS_TEST,
    TEMPLATE, TWIN_TEST, TestsRun, child_role, rerun,
};

use crate::lock_common::{RepoState, Variant};

const LOCK: &str = "spec.lock";

/// KILN-RAMP's own sentence and an edit of its meaning.
const RAMP_TEXT: (&str, &str) = (
    "The temperature rises by the ramp rate each hour and never passes the peak.",
    "The temperature rises by the ramp rate each hour and stops at the peak.",
);

/// `next_temperature`'s body and a code edit of it.
const RAMP_BODY: (&str, &str) = (
    "(now + rate).min(peak)",
    "now.saturating_add(rate).min(peak)",
);

/// `spec verify --tests` through the library from the root at clock `now`,
/// consent `y`.
fn run_at(kiln: &Kiln, now: &str) -> TestsRun {
    kiln.run_at(kiln.root(), None, true, &[], now)
}

/// The state every failure path must leave as it found it: status
/// (`--porcelain=v1 -z --untracked-files=all`), `HEAD`, the refs, the
/// index and every file.
fn unchanged(kiln: &Kiln, before: &RepoState, context: &str) {
    let after = kiln.repo.state();
    assert_eq!(after.head, before.head, "{context}: HEAD");
    assert_eq!(after.refs, before.refs, "{context}: refs");
    assert_eq!(after.status, before.status, "{context}: status");
    assert_eq!(after.index, before.index, "{context}: index");
    assert_eq!(after.files, before.files, "{context}: files");
}

// -------------------------------------------------------------------- RD-5

/// RD-1's golden and RD-5 (Data "Fixture", "The run's write"): the golden
/// is what "Data" says a first passing run writes (every block computed
/// here: `rev`, `spec_hash` from `spec show`, `bound_hash` over the
/// fixture's bindings, `commit` the run's `HEAD`, `verified_at` the clock's
/// day); the run writes it byte for byte in one commit on top of `HEAD`
/// changing `spec.lock` alone, its message the spec's (subject capped at
/// three nodes, a `Lock:` per node, `Lock-mode: tests`, `Decided-by:`,
/// `Test-command:` the question's template), the question ending `, then
/// commit their evidence to spec.lock? [y/N]`; stdout after 8a's summary
/// gives `lock | added | <node>` by node bytes, then `committed <oid>`; the
/// JSON `lock` is `{rows, commit, unchanged, refusal}`; the event's
/// `lock_commit` the commit; the tree clean. M: the golden's bytes or
/// order changed; no commit; the event's `lock_commit` null.
#[test]
fn first_passing_run_writes_the_golden() {
    let kiln = Kiln::new("tl-golden");
    let want = lock_with(&[], &expected_blocks(&kiln, &BLOCK_NODES));
    assert_eq!(golden_text(), want, "the golden is the spec's:\n{want}");
    let head = kiln.repo.head();
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.verdict(), "unverified", "KILN-COOL is not verifiable");
    assert!(
        run.question().ends_with(QUESTION_TAIL),
        "{}",
        run.question()
    );
    assert_eq!(kiln.repo.lock_text(), want, "the golden, byte for byte");

    let commit = kiln.repo.head();
    assert_eq!(kiln.repo.parents(&commit), std::slice::from_ref(&head));
    assert_eq!(kiln.repo.changed(&commit), [LOCK]);
    assert_eq!(
        kiln.repo.show_blob(&commit, LOCK),
        want.as_bytes(),
        "committed as written"
    );
    assert_eq!(
        kiln.repo.message(&commit).trim_end(),
        evidence_message(&BLOCK_NODES, COMMITTER, &template_of(&run.question())).trim_end(),
    );
    assert!(kiln.repo.porcelain().is_empty(), "the tree is clean");

    let lines = run.lines();
    let summary = lines
        .iter()
        .position(|line| line.starts_with("verify --tests: "))
        .expect("8a's summary");
    assert_eq!(
        lines[summary + 1..].to_vec(),
        [
            "lock | added | CONE-01".to_owned(),
            "lock | added | KILN-HOLD".to_owned(),
            "lock | added | KILN-RAMP".to_owned(),
            "lock | added | glaze-check/GLZ-01".to_owned(),
            format!("committed {commit}"),
        ],
        "{}",
        run.show()
    );
    let document = run.json();
    assert_eq!(
        document["lock"],
        json!({
            "rows": [
                {"node": "CONE-01", "change": "added"},
                {"node": "KILN-HOLD", "change": "added"},
                {"node": "KILN-RAMP", "change": "added"},
                {"node": "glaze-check/GLZ-01", "change": "added"},
            ],
            "commit": commit,
            "unchanged": false,
            "refusal": null,
        })
    );
    let ordered = Ordered::of(&run.json_text());
    assert_eq!(ordered.get("lock").keys(), LOCK_KEYS);
    for row in ordered.get("lock").get("rows").items() {
        assert_eq!(row.keys(), ["node", "change"]);
    }
    let event = kiln.one_event();
    assert_eq!(event["lock_commit"], json!(commit), "{event}");
    assert_eq!(event["commit"], json!(head), "the run's HEAD");
}

/// RD-5 ("Order" step 7): `--task T` writes only its set's nodes:
/// KILN-RAMP's block alone, `spec: verify tests KILN-RAMP`, one `Lock:`;
/// the run `verified`, exit 0 (8a's TE-2, AC-8 met). M: every node's block
/// written under a task.
#[test]
fn task_run_writes_its_set_only() {
    let kiln = Kiln::new("tl-task");
    let task = kiln.new_task(&["KILN-RAMP"]);
    let run = kiln.run_task(&task);
    run.code(0);
    assert_eq!(run.verdict(), "verified");
    assert_eq!(block_nodes(&kiln.repo.lock_text()), ["KILN-RAMP"]);
    assert_eq!(lock_rows(&run), rows(&[("KILN-RAMP", "added")]));
    let commit = kiln.repo.head();
    assert_eq!(
        kiln.repo.message(&commit).trim_end(),
        evidence_message(&["KILN-RAMP"], COMMITTER, &template_of(&run.question())).trim_end()
    );
    assert_eq!(kiln.one_event()["lock_commit"], json!(commit));
}

/// RD-5 **M5** (a failed or timed-out node keeps its block): after a pass,
/// a failing test removes its node's block (`removed`), the others
/// `unchanged` byte for byte, `spec: verify tests KILN-RAMP`; a timed-out
/// test removes its node's too; a failed node without a block gets no row.
#[test]
fn failing_test_removes_evidence() {
    let kiln = Kiln::new("tl-failing");
    let _reaper = kiln.reaper();
    pass(&kiln);
    let before = kiln.repo.lock_text();

    kiln.plan(&[(RAMP_STOPS_TEST, "fail")]);
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.node("KILN-RAMP")["verdict"], "failed");
    assert_eq!(
        lock_rows(&run),
        rows(&[
            ("CONE-01", "unchanged"),
            ("KILN-HOLD", "unchanged"),
            ("KILN-RAMP", "removed"),
            ("glaze-check/GLZ-01", "unchanged"),
        ]),
        "{}",
        run.show()
    );
    let after = kiln.repo.lock_text();
    assert_eq!(
        block_nodes(&after),
        ["CONE-01", "KILN-HOLD", "glaze-check/GLZ-01"]
    );
    for node in ["CONE-01", "KILN-HOLD", "glaze-check/GLZ-01"] {
        assert_eq!(block_of(&after, node), block_of(&before, node), "{node}");
    }
    let commit = kiln.repo.head();
    assert_eq!(
        kiln.repo.message(&commit).trim_end(),
        evidence_message(&["KILN-RAMP"], COMMITTER, &template_of(&run.question())).trim_end()
    );
    run.has_line("lock | removed | KILN-RAMP");

    // A time-out, KILN-RAMP still failing (no block now: no row).
    kiln.set_verify(&TEMPLATE, Some(1));
    kiln.plan(&[(RAMP_STOPS_TEST, "fail"), (GLAZE_TEST, "hang")]);
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.node("glaze-check/GLZ-01")["verdict"], "failed");
    assert_eq!(
        lock_rows(&run),
        rows(&[
            ("CONE-01", "unchanged"),
            ("KILN-HOLD", "unchanged"),
            ("glaze-check/GLZ-01", "removed"),
        ]),
        "{}",
        run.show()
    );
    assert_eq!(
        block_nodes(&kiln.repo.lock_text()),
        ["CONE-01", "KILN-HOLD"]
    );
}

/// RD-5 (M: an equal block rewritten): a second passing run over the same
/// `HEAD`'s content finds every block equal on `rev`, `spec_hash`,
/// `bound_hash` and `tests`: every row `unchanged`, the bytes kept (the
/// first run's `commit` and `verified_at` too, at a later clock and after
/// an unrelated commit), no commit, JSON `lock.commit` null and
/// `unchanged` true, no `committed` line, the event's `lock_commit` null.
#[test]
fn equal_evidence_makes_no_commit() {
    let kiln = Kiln::new("tl-equal");
    pass(&kiln);
    let lock = kiln.repo.lock_text();
    kiln.repo.write("notes/firing.txt", "an unrelated note\n");
    kiln.repo.commit_all("an unrelated note");
    let head = kiln.repo.head();
    let run = run_at(&kiln, "2026-10-17T23:59:59Z");
    run.code(1);
    assert_eq!(kiln.repo.head(), head, "no commit");
    assert_eq!(kiln.repo.lock_text(), lock, "the bytes kept");
    assert_eq!(
        lock_rows(&run),
        rows(&[
            ("CONE-01", "unchanged"),
            ("KILN-HOLD", "unchanged"),
            ("KILN-RAMP", "unchanged"),
            ("glaze-check/GLZ-01", "unchanged"),
        ])
    );
    let document = run.json();
    assert_eq!(
        (
            &document["lock"]["commit"],
            &document["lock"]["unchanged"],
            &document["lock"]["refusal"]
        ),
        (&Value::Null, &json!(true), &Value::Null)
    );
    assert!(
        !run.lines()
            .iter()
            .any(|line| line.starts_with("committed ")),
        "{}",
        run.show()
    );
    assert_eq!(lock_lines(&run).len(), 4);
    let events = kiln.events();
    let last: Value = serde_json::from_str(events.last().expect("an event")).expect("JSON");
    assert_eq!(last["lock_commit"], Value::Null);
    assert!(kiln.repo.porcelain().is_empty());
}

/// RD-5 ("The write", `refreshed`): after a code edit of KILN-RAMP's
/// implementation, committed, a passing run refreshes its block alone (new
/// `bound_hash`, `commit` the new `HEAD`, the new day), the others
/// `unchanged`; `spec: verify tests KILN-RAMP`. M: a changed block kept.
#[test]
fn changed_code_refreshes_the_block() {
    let kiln = Kiln::new("tl-refresh");
    pass(&kiln);
    let before = kiln.repo.lock_text();
    kiln.repo.replace(RAMP, RAMP_BODY.0, RAMP_BODY.1);
    kiln.repo.commit_all("the ramp saturates");
    let head = kiln.repo.head();
    let run = run_at(&kiln, "2026-10-18T08:00:00Z");
    assert_eq!(
        lock_rows(&run),
        rows(&[
            ("CONE-01", "unchanged"),
            ("KILN-HOLD", "unchanged"),
            ("KILN-RAMP", "refreshed"),
            ("glaze-check/GLZ-01", "unchanged"),
        ]),
        "{}",
        run.show()
    );
    let after = kiln.repo.lock_text();
    let ramp = block_of(&after, "KILN-RAMP").expect("KILN-RAMP's block");
    let old = block_of(&before, "KILN-RAMP").expect("the old block");
    assert_ne!(
        block_value(&ramp, "bound_hash"),
        block_value(&old, "bound_hash")
    );
    assert_eq!(
        block_value(&ramp, "spec_hash"),
        block_value(&old, "spec_hash")
    );
    assert_eq!(block_value(&ramp, "commit"), head);
    assert_eq!(block_value(&ramp, "verified_at"), "2026-10-18");
    let want = expected_blocks(&kiln, &["KILN-RAMP"]).remove(0);
    assert_eq!(block_value(&ramp, "bound_hash"), want.bound_hash);
    let commit = kiln.repo.head();
    assert_eq!(kiln.repo.parents(&commit), [head]);
    assert_eq!(
        kiln.repo.message(&commit).trim_end(),
        evidence_message(&["KILN-RAMP"], COMMITTER, &template_of(&run.question())).trim_end()
    );
}

/// RD-5 (M: a `[[binding]]` or the header touched): over a lock holding
/// KILN-RAMP's accepted entries, the run adds blocks and `format = 3` and
/// nothing else: the lock without its blocks, `format` back to 2, is the
/// lock before byte for byte (header, recipes, every entry).
#[test]
fn test_run_never_touches_bindings() {
    let kiln = Kiln::new("tl-bindings");
    kiln.repo.accept_ok("KILN-RAMP");
    let before = kiln.repo.lock_text();
    assert!(before.contains("\n[[binding]]\n"), "KILN-RAMP's entries");
    pass(&kiln);
    let after = kiln.repo.lock_text();
    assert_eq!(after.lines().nth(1), Some("format = 3"));
    assert_eq!(block_nodes(&after), BLOCK_NODES);
    assert_eq!(without_blocks(&after), before, "entries and header kept");
}

/// RD-5 (M: a write after `changed`, `interrupted`, `refused` or
/// `declined`): only a completed run writes; each of these leaves no lock,
/// no evidence commit, JSON `lock` null, the event's `lock_commit` null.
#[test]
fn no_evidence_without_a_completed_run() {
    if child_role() {
        return;
    }
    // Declined.
    let kiln = Kiln::new("tl-none-declined");
    let head = kiln.repo.head();
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    assert_eq!(run.verdict(), "declined");
    assert_eq!(run.json()["lock"], Value::Null);
    assert!(kiln.repo.lock().is_none(), "declined: no lock");
    assert_eq!(kiln.repo.head(), head);

    // Refused: the probe passes.
    let kiln = Kiln::new("tl-none-refused");
    let head = kiln.repo.head();
    kiln.plan(&[("*probe", "pass")]);
    let run = kiln.run();
    assert_eq!(run.verdict(), "refused", "{}", run.show());
    assert_eq!(run.json()["lock"], Value::Null);
    assert!(kiln.repo.lock().is_none(), "refused: no lock");
    assert_eq!(kiln.repo.head(), head);
    assert_eq!(kiln.one_event()["lock_commit"], Value::Null);

    // Changed: the runner commits.
    let kiln = Kiln::new("tl-none-changed");
    let head = kiln.repo.head();
    kiln.plan(&[(RAMP_STOPS_TEST, "commit")]);
    let run = kiln.run();
    assert_eq!(run.verdict(), "changed", "{}", run.show());
    assert_eq!(run.json()["lock"], Value::Null);
    assert!(kiln.repo.lock().is_none(), "changed: no lock");
    let moved = kiln.repo.head();
    assert_eq!(
        kiln.repo.parents(&moved),
        [head],
        "the runner's commit only"
    );
    assert_eq!(kiln.one_event()["lock_commit"], Value::Null);

    // Interrupted: a child `spec` gets SIGINT from its runner.
    let kiln = Kiln::new("tl-none-interrupted");
    let _reaper = kiln.reaper();
    let head = kiln.repo.head();
    kiln.set_verify(&TEMPLATE, Some(30));
    let head_after_config = kiln.repo.head();
    assert_ne!(head_after_config, head);
    kiln.plan(&[(TWIN_TEST, "interrupt")]);
    let child = rerun(
        &kiln,
        "no_evidence_without_a_completed_run",
        None,
        b"",
        Duration::from_secs(90),
    );
    assert_eq!(child.code, 2, "{}", child.show());
    let doc = child.json();
    assert_eq!(doc["verdict"], "interrupted", "{}", child.json_text);
    assert_eq!(doc["lock"], Value::Null);
    assert!(kiln.repo.lock().is_none(), "interrupted: no lock");
    assert_eq!(kiln.repo.head(), head_after_config);
}

/// RD-5 (M: a not-verifiable node's block removed): after a pass, a
/// second `@verifies KILN-RAMP` marker with an invalid level list makes
/// KILN-RAMP `not verifiable bad_levels`: its block stays byte for byte,
/// no row names it; its shared test still runs for KILN-HOLD.
#[test]
fn unverifiable_node_keeps_its_block() {
    let kiln = Kiln::new("tl-unverifiable");
    pass(&kiln);
    let before = kiln.repo.lock_text();
    kiln.repo.write(
        "crates/kiln/tests/lid.rs",
        "// @verifies KILN-RAMP [Sig]\n#[test]\nfn lid_holds() {}\n",
    );
    kiln.repo.commit_all("a check under an invalid list");
    let head = kiln.repo.head();
    let run = kiln.run();
    assert_eq!(
        run.node("KILN-RAMP"),
        json!({"id": "KILN-RAMP", "verdict": "not_verifiable", "state": "bad_levels"})
    );
    assert!(
        !lock_rows(&run).iter().any(|(node, _)| node == "KILN-RAMP"),
        "{}",
        run.show()
    );
    assert_eq!(kiln.repo.lock_text(), before, "every block kept");
    assert_eq!(kiln.repo.head(), head, "all else unchanged: no commit");
    assert!(
        kiln.test_invocations()
            .iter()
            .any(|args| args.last().map(String::as_str) == Some(HOLD_TEST)),
        "KILN-HOLD's test ran"
    );
}

/// RD-5 (Q-C; M: a block written over text changed at R): after a pass,
/// KILN-RAMP's text edited without a raise and committed: the run calls it
/// `not verifiable text_changed` before the question (its block at R holds
/// another `spec_hash`), its own test `not run | … | node not verifiable`,
/// the shared `hold_follows_the_ramp` run for KILN-HOLD; its block stays.
#[test]
fn text_changed_at_r_is_not_verifiable() {
    let kiln = Kiln::new("tl-text-changed");
    pass(&kiln);
    let before = kiln.repo.lock_text();
    kiln.repo.replace(KILN_MD, RAMP_TEXT.0, RAMP_TEXT.1);
    kiln.repo.commit_all("the ramp's text, unraised");
    let run = kiln.run();
    assert!(
        run.listing()
            .contains(&"node KILN-RAMP: not verifiable text_changed".to_owned()),
        "{:?}",
        run.listing()
    );
    assert_eq!(
        run.node("KILN-RAMP"),
        json!({"id": "KILN-RAMP", "verdict": "not_verifiable", "state": "text_changed"})
    );
    let row = run.row(RAMP_STOPS);
    assert_eq!(
        (&row["outcome"], &row["reason"]),
        (&json!("not_run"), &json!("node not verifiable")),
        "{row}"
    );
    assert!(
        kiln.test_invocations()
            .iter()
            .any(|args| args.last().map(String::as_str) == Some(HOLD_TEST)),
        "the shared test ran for KILN-HOLD"
    );
    assert!(
        !kiln
            .test_invocations()
            .iter()
            .any(|args| args.last().map(String::as_str) == Some(RAMP_STOPS_TEST)),
        "KILN-RAMP's own test never ran"
    );
    assert_eq!(
        block_of(&kiln.repo.lock_text(), "KILN-RAMP"),
        block_of(&before, "KILN-RAMP"),
        "its block stays"
    );
}

/// RD-5, TE-3 (Q-D (i); M: the question asked or a test run first): a lock
/// the run cannot write into is `cannot`, exit 2, before the question,
/// nothing started, nothing changed: an unreadable lock, a format-1 lock
/// and one of another `code_recipe` (verify's cause (6) texts); a first
/// lock git ignores (`spec.lock is ignored by git: …`).
#[test]
fn unreadable_lock_is_cannot_before_the_question() {
    let cases: Vec<(&str, String, String)> = vec![
        (
            "unreadable",
            "format = = 2\n".to_owned(),
            "spec.lock:".to_owned(),
        ),
        (
            "format 1",
            Variant::Format1.text(&lock_common::lock_of(&[])),
            Variant::Format1.message(),
        ),
        (
            "another code_recipe",
            Variant::OtherCode.text(&lock_common::lock_of(&[])),
            Variant::OtherCode.message(),
        ),
    ];
    for (label, text, want) in cases {
        let kiln = Kiln::new(&format!("tl-cannot-{}", label.replace(' ', "-")));
        kiln.repo.write(LOCK, &text);
        kiln.repo.commit_all(label);
        let before = kiln.repo.state();
        let run = kiln.run();
        let message = run.cannot_text(label);
        assert!(message.contains(&want), "{label}: {message}\nwant {want}");
        assert!(run.prompts.is_empty(), "{label}: asked");
        kiln.assert_nothing_started(label);
        unchanged(&kiln, &before, label);
    }

    // TE-3: an ignored first lock.
    let kiln = Kiln::new("tl-cannot-ignored");
    kiln.repo.write(".gitignore", "spec.lock\n");
    kiln.repo.commit_all("ignore the lock");
    let before = kiln.repo.state();
    let run = kiln.run();
    let message = run.cannot_text("an ignored first lock");
    assert!(message.contains(IGNORED), "{message}");
    assert!(run.prompts.is_empty(), "asked");
    kiln.assert_nothing_started("an ignored first lock");
    unchanged(&kiln, &before, "an ignored first lock");
    assert!(kiln.events().is_empty(), "nothing ran, nothing recorded");
}

/// RD-5, TE-3 (Q-D (ii); M: the restore skipped; the event missing): a
/// `pre-commit` hook refusing the evidence commit keeps every verdict
/// (`verified`), sets `lock.refusal`, leaves `lock.commit` and the event's
/// `lock_commit` null, writes the event, exits 1; the lock's bytes, status,
/// `HEAD`, refs and the index as before (a first lock: absent again; a
/// lock with blocks: its bytes). stdout ends `lock: <refusal>`.
#[test]
fn refused_evidence_commit_exits_1() {
    let kiln = Kiln::new("tl-refused-commit");
    let task = kiln.new_task(&["KILN-RAMP"]);
    kiln.repo.hook("pre-commit", "exit 1");
    let before = kiln.repo.state();
    let run = kiln.run_task(&task);
    run.code(1);
    assert_eq!(run.verdict(), "verified", "the verdict kept");
    assert_eq!(run.node("KILN-RAMP")["verdict"], "verified");
    let document = run.json();
    let refusal = document["lock"]["refusal"]
        .as_str()
        .unwrap_or_else(|| panic!("a refusal: {}", run.show()))
        .to_owned();
    assert!(!refusal.is_empty());
    assert_eq!(document["lock"]["commit"], Value::Null);
    assert_eq!(
        run.lines().last().cloned(),
        Some(format!("lock: {refusal}")),
        "{}",
        run.show()
    );
    let event = kiln.one_event();
    assert_eq!(
        (&event["verdict"], &event["lock_commit"]),
        (&json!("verified"), &Value::Null)
    );
    assert!(kiln.repo.lock().is_none(), "the first lock removed again");
    unchanged(&kiln, &before, "a refused first lock");

    // Over a lock with blocks: its bytes restored.
    let kiln = Kiln::new("tl-refused-commit-2");
    pass(&kiln);
    kiln.repo.replace(RAMP, RAMP_BODY.0, RAMP_BODY.1);
    kiln.repo.commit_all("the ramp saturates");
    kiln.repo.hook("pre-commit", "exit 1");
    let before = kiln.repo.state();
    let run = kiln.run();
    run.code(1);
    assert!(run.json()["lock"]["refusal"].is_string(), "{}", run.show());
    unchanged(&kiln, &before, "a refused refresh");
}

// -------------------------------------------------------------------- TE-2

/// The lock, stdout and JSON of one passing run, the root's path and the
/// `HEAD`s' OIDs masked (two roots differ in path; a SHA-256 repository
/// in its OIDs).
fn outputs(kiln: &Kiln, run: &TestsRun, mask_oids: bool) -> (String, String, String) {
    let root = kiln.root().display().to_string();
    let mut texts = (kiln.repo.lock_text(), run.text(), run.json_text());
    let mut oids = vec![kiln.repo.head()];
    oids.extend(kiln.repo.parents(&kiln.repo.head()));
    for text in [&mut texts.0, &mut texts.1, &mut texts.2] {
        *text = text.replace(&root, "<root>");
        if mask_oids {
            for oid in &oids {
                *text = text.replace(oid.as_str(), "<oid>");
            }
        }
    }
    texts
}

/// TE-2 (M: a path or a time of day in a block): two roots, a run from
/// another cwd, the fixture written in reverse creation order, a
/// `core.autocrlf=true` checkout and another time of day give the same
/// lock, stdout and JSON byte for byte (the OIDs too: equal content, equal
/// dates); a SHA-256 repository the same with its OIDs masked, and the
/// same `bound_hash` and `spec_hash` in every block.
#[test]
fn evidence_is_deterministic() {
    let one = Kiln::new("tl-det-one");
    let (run, _) = pass(&one);
    let want = outputs(&one, &run, false);
    let want_masked = outputs(&one, &run, true);

    let two = Kiln::at("tl-det-two", "elsewhere");
    let (run, _) = pass(&two);
    assert_eq!(outputs(&two, &run, false), want, "another root");

    let cwd = Kiln::new("tl-det-cwd");
    let sub = cwd.root().join("crates/kiln");
    let before = cwd.repo.head();
    let run = cwd.run_with(&sub, None, true, &[]);
    assert_ne!(cwd.repo.head(), before, "{}", run.show());
    assert_eq!(outputs(&cwd, &run, false), want, "another cwd");

    let reversed = Kiln::reversed("tl-det-reversed");
    let (run, _) = pass(&reversed);
    assert_eq!(outputs(&reversed, &run, false), want, "reverse order");

    let crlf = Kiln::new("tl-det-crlf");
    crlf.repo.git(&["config", "core.autocrlf", "true"]);
    for file in [RAMP, "crates/kiln/tests/schedule.rs", KILN_MD] {
        std::fs::remove_file(crlf.root().join(file)).expect("a checked-out file");
        crlf.repo.git(&["checkout", "--", file]);
    }
    assert!(crlf.repo.read(RAMP).contains("\r\n"), "a CRLF checkout");
    assert!(crlf.repo.porcelain().is_empty(), "clean under autocrlf");
    let (run, _) = pass(&crlf);
    assert_eq!(outputs(&crlf, &run, false), want, "core.autocrlf=true");

    let late = Kiln::new("tl-det-late");
    let run = run_at(&late, "2026-10-16T23:59:59Z");
    assert_eq!(outputs(&late, &run, false), want, "another time of day");

    let sha256 = Kiln::bare(
        lock_common::LockRepo::of(
            "tl-det-sha256",
            verify_tests_common::FIXTURE,
            "repo",
            &["--object-format=sha256"],
        ),
        verify_tests_common::SLUG,
    );
    sha256.write_runner(verify_tests_common::RUNNER, 0o755);
    sha256.repo.commit_all("the runner");
    assert_eq!(sha256.repo.head().len(), 64, "a SHA-256 repository");
    let (run, _) = pass(&sha256);
    assert_eq!(
        outputs(&sha256, &run, true),
        want_masked,
        "SHA-256, OIDs masked"
    );
    let blocks_of = |kiln: &Kiln| -> Vec<(String, String, String)> {
        tested_blocks(&kiln.repo.lock_text())
            .into_iter()
            .map(|(node, block)| {
                (
                    node,
                    block_value(&block, "spec_hash"),
                    block_value(&block, "bound_hash"),
                )
            })
            .collect()
    };
    assert_eq!(blocks_of(&sha256), blocks_of(&one), "the hashes alike");
}

// -------------------------------------------------------------------- TE-3

/// TE-3 (M: a write after a moved `HEAD`): a runner that commits moves
/// `HEAD` during the run: `changed`, nothing written; the only new commit
/// is the runner's, the tree clean, the index `HEAD`'s.
#[test]
fn moved_head_writes_nothing() {
    let kiln = Kiln::new("tl-moved");
    pass(&kiln);
    let lock = kiln.repo.lock_text();
    kiln.repo.replace(RAMP, RAMP_BODY.0, RAMP_BODY.1);
    kiln.repo.commit_all("the ramp saturates");
    let head = kiln.repo.head();
    kiln.plan(&[(RAMP_STOPS_TEST, "commit")]);
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.verdict(), "changed");
    assert_eq!(run.json()["lock"], Value::Null);
    let moved = kiln.repo.head();
    assert_eq!(
        kiln.repo.parents(&moved),
        [head],
        "the runner's commit only"
    );
    assert_eq!(
        kiln.repo.changed(&moved),
        Vec::<String>::new(),
        "an empty commit"
    );
    assert_eq!(kiln.repo.lock_text(), lock, "the lock as it was");
    assert!(kiln.repo.porcelain().is_empty(), "clean");
    let index = kiln.repo.git_text(&["diff", "--cached", "--name-only"]);
    assert_eq!(index, "", "the index is HEAD's");
}

/// TE-3 (accept's table, "Restoring … only while the file still holds
/// what the run wrote"): a `pre-commit` hook that edits the lock, then
/// refuses: exit 1, the verdicts kept, `lock.refusal` saying `spec.lock was
/// changed meanwhile, so it is left as it is, not restored`, the hook's
/// bytes left, `HEAD` and the refs as before, the event written with
/// `lock_commit` null. M: the hook's bytes overwritten; the event missing.
#[test]
fn hook_editing_the_lock_then_refusing_is_left() {
    let kiln = Kiln::new("tl-hook-edits");
    pass(&kiln);
    kiln.repo.replace(RAMP, RAMP_BODY.0, RAMP_BODY.1);
    kiln.repo.commit_all("the ramp saturates");
    kiln.repo.hook(
        "pre-commit",
        "printf '# hook was here\\n' >> spec.lock\nexit 1",
    );
    let before = kiln.repo.state();
    let events = kiln.events().len();
    let run = kiln.run();
    run.code(1);
    let refusal = run.json()["lock"]["refusal"]
        .as_str()
        .unwrap_or_else(|| panic!("a refusal: {}", run.show()))
        .to_owned();
    assert!(
        refusal.contains("spec.lock was changed meanwhile, so it is left as it is, not restored"),
        "{refusal}"
    );
    assert!(
        kiln.repo.lock_text().ends_with("# hook was here\n"),
        "the hook's bytes left"
    );
    let after = kiln.repo.state();
    assert_eq!(after.head, before.head, "HEAD");
    assert_eq!(after.refs, before.refs, "refs");
    let all = kiln.events();
    assert_eq!(all.len(), events + 1, "the event written");
    let last: Value = serde_json::from_str(all.last().expect("an event")).expect("JSON");
    assert_eq!(last["lock_commit"], Value::Null);
}
