//! docs/features/verify-tests.md (8a-1) RD-7's tree half and TE-6: the
//! whole tree clean before the run ("Order" step 4, P9's status: no
//! pathspec, untracked files too, `--no-renames`) and the same after it
//! (step 10). A modified, staged, renamed or untracked file is `dirty`,
//! exit 1, nothing run, no question, no event, the reason naming each
//! path and ending `commit them first`; a runner writing into the tree or
//! moving `HEAD` makes the run `changed`, exit 1, no node verdict, the
//! event written. Scratch repositories of `fixtures/verify-tests` (P10)
//! with the fake runner (TE-1). Each test names the mutation that turns it
//! red (`M:`).

mod common;
mod lock_common;
mod verify_tests_common;

use verify_tests_common::{COOL, Kiln, RAMP, RAMP_STOPS_TEST, TestsRun};

/// A `dirty` run: exit 1, the reason naming every path of `paths` and
/// ending `commit them first`, in the summary and the document; no
/// question, no runner, no event, no row. Its reason.
fn assert_dirty(kiln: &Kiln, context: &str, paths: &[&str]) -> String {
    let run: TestsRun = kiln.run();
    run.code(1);
    let doc = run.json();
    assert_eq!(doc["verdict"], "dirty", "{context}: {}", run.show());
    let reason = doc["reason"].as_str().expect("a reason").to_owned();
    for path in paths {
        assert!(reason.contains(path), "{context}: {path} named: {reason}");
    }
    assert!(reason.ends_with("commit them first"), "{context}: {reason}");
    assert_eq!(
        run.summary(),
        format!("verify --tests: dirty: {reason}"),
        "{context}"
    );
    assert!(run.prompts.is_empty(), "{context}: asked {:?}", run.prompts);
    assert!(
        run.rows().is_empty() && run.nodes().is_empty(),
        "{context}: nothing read after the status: {}",
        run.show()
    );
    kiln.assert_nothing_started(context);
    assert!(kiln.events().is_empty(), "{context}: no event");
    reason
}

/// RD-7, TE-6 (P9): an untracked file anywhere (outside the code roots
/// too) refuses the run as `dirty` with its status letters `??`. M:
/// untracked files ignored.
#[test]
fn untracked_file_refuses_the_run() {
    let kiln = Kiln::new("vt-untracked");
    kiln.repo.write("notes/firing-log.txt", "a note\n");
    let reason = assert_dirty(&kiln, "untracked", &["notes/firing-log.txt"]);
    assert!(reason.contains("??"), "the status letters: {reason}");
}

/// TE-6: a modified file, the same change staged, and a staged rename
/// (read with `--no-renames`: the old path deleted, the new one added) are
/// each `dirty`, naming their paths. M: the status read with a pathspec, or
/// a rename pair misread.
#[test]
fn modified_staged_or_renamed_file_refuses_the_run() {
    let kiln = Kiln::new("vt-modified");
    kiln.repo.append(RAMP, "\n// A comment.\n");
    assert_dirty(&kiln, "modified", &[RAMP]);

    kiln.repo.git(&["add", RAMP]);
    assert_dirty(&kiln, "staged", &[RAMP]);
    kiln.repo.git(&["reset", "-q", "--hard"]);

    kiln.repo.git(&["mv", COOL, "crates/kiln/src/chill.rs"]);
    assert_dirty(&kiln, "renamed", &[COOL, "crates/kiln/src/chill.rs"]);
}

/// RD-7, TE-6 ("Order" step 10, "Risks": tree writes): a runner that writes
/// a file into the tree, and one that commits (moving `HEAD`), make the
/// run `changed`, exit 1, `nodes` empty, the reason naming the written
/// path and ending `commit them first`; each run's event says `changed`.
/// M: no re-check.
#[test]
fn tree_changed_during_the_run_is_not_verified() {
    let kiln = Kiln::new("vt-changed");
    kiln.plan(&[(RAMP_STOPS_TEST, "write-tree")]);
    let run = kiln.run();
    run.code(1);
    let doc = run.json();
    assert_eq!(doc["verdict"], "changed", "{}", run.show());
    assert_eq!(doc["nodes"], serde_json::json!([]), "no node verdict");
    let reason = doc["reason"].as_str().expect("a reason");
    assert!(
        reason.contains("firing-notes.txt") && reason.ends_with("commit them first"),
        "{reason}"
    );
    assert!(
        run.summary().starts_with("verify --tests: changed: "),
        "{}",
        run.summary()
    );
    assert_eq!(kiln.one_event()["verdict"], "changed");

    std::fs::remove_file(kiln.root().join("firing-notes.txt")).expect("the written file");
    let head = kiln.repo.head();
    kiln.plan(&[(RAMP_STOPS_TEST, "commit")]);
    let run = kiln.run();
    run.code(1);
    assert_ne!(kiln.repo.head(), head, "the runner committed");
    let doc = run.json();
    assert_eq!(doc["verdict"], "changed", "{}", run.show());
    assert_eq!(doc["nodes"], serde_json::json!([]));
    let events = kiln.events();
    assert_eq!(events.len(), 2, "{events:#?}");
    let last: serde_json::Value = serde_json::from_str(&events[1]).expect("JSON");
    assert_eq!(last["verdict"], "changed");
}
