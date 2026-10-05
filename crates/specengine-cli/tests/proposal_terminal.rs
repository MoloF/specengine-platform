//! docs/features/proposal-apply.md AC-06 through the `spec` binary: with a
//! piped stdin (never a terminal) `spec approve` and `spec reject` exit 2
//! before anything — nothing stored, written or committed, no event —
//! even when the pipe carries `y`. And the queue's other commands as the
//! binary prints them: `propose update --text-file -` (the text from
//! stdin), its refusal (exit 1, `spec: …` on stderr), `review`, `inbox`.
//!
//! Every `spec` run gets the sandbox's variables and a scratch `HOME`.

#![cfg(unix)]

mod common;

use common::proposal::{NOW, Pair, edit};

/// AC-06: piped stdin: `approve`, `reject` exit 2, refused, no event, no
/// prompt. M: terminal check removed.
#[test]
fn ac06_approve_and_reject_on_a_piped_stdin_exit_2() {
    let pair = Pair::new("pa-ac06", "spec-a");
    let id = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let before = pair.state();
    let events = pair.events();
    for args in [
        vec!["approve", id.as_str()],
        vec!["approve", id.as_str(), "--note", "fine"],
        vec!["reject", id.as_str(), "--reason", "x"],
        vec!["--json", "approve", id.as_str()],
    ] {
        let run = pair.spec_piped(&pair.main, &args, b"y\nyes\n");
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: nothing on stdout");
        assert!(
            run.stderr.starts_with("spec: ") && run.stderr.contains("terminal"),
            "{args:?}: {}",
            run.show()
        );
        assert!(
            !run.stderr.contains("[y/N]"),
            "{args:?}: no prompt: {}",
            run.show()
        );
        assert_eq!(run.stderr.lines().count(), 1, "{}", run.show());
        assert_eq!(pair.state(), before, "{args:?}: refused");
        assert_eq!(pair.events(), events, "{args:?}: no event");
    }
    assert_eq!(pair.proposal(&id).status.as_str(), "open");
}

/// `spec propose update --text-file -` reads the text from stdin and prints
/// the ID and the count; a stale base exits 1 with the reason on stderr;
/// `review` and `inbox` print what the library renders.
#[test]
fn the_binary_prints_the_queues_documents() {
    let pair = Pair::new("pa-binary", "spec-a");
    let (hash, text) = pair.span(&pair.linked, "EDGE-SPRINT-EMPTY");
    let new_text = edit(&text, "the sprint ends;", "the sprint ends at once;");
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "update",
            "EDGE-SPRINT-EMPTY",
            "--base",
            &hash,
            "--text-file",
            "-",
            "--rationale",
            "From stdin.",
            "--author-role",
            "spec-writer",
        ],
        new_text.as_bytes(),
    );
    run.code(0);
    assert_eq!(run.stdout, "PR-0001\nintroduced: 0\n", "{}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(pair.proposal("PR-0001").new_text, new_text);
    assert_eq!(
        pair.proposal("PR-0001").author.provenance(),
        "agent role=spec-writer model=unknown run=unknown"
    );

    let before = pair.state();
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "update",
            "EDGE-SPRINT-EMPTY",
            "--base",
            "b3:0000",
            "--text-file",
            "-",
            "--rationale",
            "Stale.",
        ],
        new_text.as_bytes(),
    );
    run.code(1);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(
        run.stderr.starts_with("spec: ") && run.stderr.contains(&hash),
        "{}",
        run.show()
    );
    assert_eq!(pair.state(), before);

    let run = pair.spec_piped(&pair.main, &["review", "PR-0001"], b"");
    run.code(0);
    assert!(
        run.stdout
            .starts_with("id: PR-0001\nproject: lantern-keep\nkind: update\nstatus: open\n"),
        "{}",
        run.show()
    );
    assert!(
        run.stdout.contains("\npreview: applies\n"),
        "{}",
        run.show()
    );
    let run = pair.spec_piped(&pair.main, &["--json", "review", "PR-0001"], b"");
    run.code(0);
    assert_eq!(run.json()["status"], "open");
    let run = pair.spec_piped(&pair.main, &["review", "PR-0009"], b"");
    run.code(1);
    assert_eq!(run.stdout, "");
    let run = pair.spec_piped(&pair.main, &["review", "\u{0420}R-0001"], b"");
    run.code(2);
    assert!(run.stderr.contains("`PR-0001`"), "{}", run.show());

    let run = pair.spec_piped(&pair.main, &["inbox"], b"");
    run.code(0);
    let fields: Vec<&str> = run.stdout.trim_end().split(" | ").collect();
    assert_eq!(fields.len(), 7, "{}", run.show());
    assert_eq!(
        fields[..5],
        ["PR-0001", "update", "open", "EDGE-SPRINT-EMPTY", "t1"]
    );
    assert_eq!(fields[6], "From stdin.");
    // The binary's own clock: a UTC time stamp.
    let at = fields[5].as_bytes();
    assert!(
        at.len() == NOW.len() && at[4] == b'-' && at[10] == b'T' && at[19] == b'Z',
        "{}",
        run.show()
    );
    let run = pair.spec_piped(&pair.main, &["--json", "inbox", "--all"], b"");
    run.code(0);
    assert_eq!(run.json()["proposals"][0]["id"], "PR-0001");
}

/// AC-18, the CLI half: `spec index --full` (and a plain `spec index`)
/// leave the queue's tables byte for byte (the store's `format.rs` covers
/// an `INDEX_FORMAT` stamp change). M: a queue table in the drop list.
#[test]
fn ac18_spec_index_full_keeps_the_queue() {
    let pair = Pair::new("pa-ac18", "spec-b");
    pair.propose_edit(&pair.linked, "CMD-SYNC", "`sync`", "`sync --all`");
    pair.propose_edit(
        &pair.linked,
        "CMD-STATUS",
        "ADR-0001#history",
        "ADR-0001#history (old)",
    );
    let (outcome, _) = pair.reject_answer(&pair.main, "PR-0002", "No.", true);
    assert_eq!(outcome.unwrap().exit(), specengine_cli::Exit::Answered);
    let dump = pair.queue().dump().unwrap();
    assert_eq!(dump.lines().count(), 5, "{dump}");
    for args in [
        &["index", "--full"][..],
        &["index"][..],
        &["--json", "index", "--full"][..],
    ] {
        for cwd in [&pair.main, &pair.linked] {
            let run = pair.spec_piped(cwd, args, b"");
            run.code(0);
            assert_eq!(
                pair.queue().dump().unwrap(),
                dump,
                "{args:?} in {}",
                cwd.display()
            );
        }
    }
}
