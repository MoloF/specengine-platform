//! docs/features/tested-evidence.md (8b-1) TE-1: 08 AC-8 end to end over
//! one history of a scratch copy of `fixtures/verify-tests` with 8a's fake
//! runner (`verify_tests_common`): an agent's `report_run` writes no
//! evidence; the owner's passing `spec verify --tests --task T` writes
//! KILN-RAMP's block in one commit that passes accept's four checks, and
//! plain `spec verify` reads it `verified`; then each later state of the
//! node, its text and its bound code reads as "The state" says, every
//! step's mutation named (`M:`); a failing run removes the block. KILN-RAMP's
//! markers are pinned at `@1` and a RON value is bound to it
//! (`@configures`) before the first run, so a raise makes them lag and a
//! RON edit reaches it.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use serde_json::json;
use tested_common::{block_nodes, block_of, plain, state_of, tested};
use tested_kiln::{FIRING_RON, FIRING_TEXT, RAMP_COMMENT, RAMP_SQUEEZED};
use verify_tests_common::{EVENT, KILN_MD, Kiln, RAMP, RAMP_STOPS_TEST, SCHEDULE};

/// `(state, category)` of KILN-RAMP's entry in plain verify now.
fn ramp_now(kiln: &Kiln) -> (String, String) {
    state_of(&plain(&kiln.repo, &[]), "KILN-RAMP")
}

fn pair(state: &str, category: &str) -> (String, String) {
    (state.to_owned(), category.to_owned())
}

/// The files the steps edit, as committed before the first run.
struct Saved {
    ramp: String,
    schedule: String,
    kiln_md: String,
    firing: String,
}

impl Saved {
    fn take(kiln: &Kiln) -> Self {
        Self {
            ramp: kiln.repo.read(RAMP),
            schedule: kiln.repo.read(SCHEDULE),
            kiln_md: kiln.repo.read(KILN_MD),
            firing: kiln.repo.read(FIRING_RON),
        }
    }

    /// Every edited file back to its committed bytes: the tree clean.
    fn restore(&self, kiln: &Kiln) {
        kiln.repo.write(RAMP, &self.ramp);
        kiln.repo.write(SCHEDULE, &self.schedule);
        kiln.repo.write(KILN_MD, &self.kiln_md);
        kiln.repo.write(FIRING_RON, &self.firing);
        assert!(kiln.repo.porcelain().is_empty(), "the tree is clean again");
    }
}

#[test]
fn ac_8_end_to_end() {
    let kiln = Kiln::new("te-ac8");
    // KILN-RAMP's markers pinned at its revision 1, a RON value bound.
    for (file, from, to) in [
        (
            RAMP,
            "// @implements KILN-RAMP\n",
            "// @implements KILN-RAMP@1\n",
        ),
        (
            RAMP,
            "    // @verifies KILN-RAMP\n",
            "    // @verifies KILN-RAMP@1\n",
        ),
        (
            SCHEDULE,
            "// @verifies KILN-RAMP\n",
            "// @verifies KILN-RAMP@1\n",
        ),
    ] {
        kiln.repo.replace(file, from, to);
    }
    kiln.repo.write(
        FIRING_RON,
        FIRING_TEXT.replace("@configures KILN-RAMP", "@configures KILN-RAMP@1"),
    );
    kiln.repo
        .commit_all("KILN-RAMP pinned at 1, its peak tuned");
    let saved = Saved::take(&kiln);

    // Step 1 (M: `report_run` writing evidence): an agent's report of a
    // run changes nothing in git: no lock, no block, no `verify.tests`
    // event; plain verify lists no block.
    let task = kiln.new_task(&["KILN-RAMP"]);
    kiln.approve_task(&task);
    kiln.claim_task(&task);
    let head = kiln.repo.head();
    kiln.report_task(&task);
    assert_eq!(kiln.repo.head(), head, "report_run commits nothing");
    assert!(kiln.repo.lock().is_none(), "report_run writes no lock");
    assert!(!kiln.event_types().contains(&EVENT.to_owned()));
    assert_eq!(
        tested(&plain(&kiln.repo, &[])),
        Vec::<serde_json::Value>::new()
    );

    // Step 2 (M: no block, or a commit failing a check): the owner's
    // passing run of the task writes KILN-RAMP's block alone in one commit:
    // its parent the old HEAD, `spec.lock` alone changed, `Lock:
    // KILN-RAMP`, its raw blob the bytes written; plain verify `verified`.
    let run = kiln.run_task(&task);
    run.code(0);
    assert_eq!(run.verdict(), "verified");
    let commit = kiln.repo.head();
    assert_eq!(kiln.repo.parents(&commit), std::slice::from_ref(&head));
    assert_eq!(kiln.repo.changed(&commit), ["spec.lock"]);
    let trailers = kiln.repo.git_text(&[
        "log",
        "-1",
        "--format=%(trailers:key=Lock,valueonly)",
        &commit,
    ]);
    assert_eq!(trailers.trim(), "KILN-RAMP");
    let lock = kiln.repo.lock_text();
    assert_eq!(kiln.repo.show_blob(&commit, "spec.lock"), lock.as_bytes());
    assert_eq!(block_nodes(&lock), ["KILN-RAMP"]);
    let block = block_of(&lock, "KILN-RAMP").expect("KILN-RAMP's block");
    assert!(
        block.contains(&format!("\ncommit = \"{head}\"\n")),
        "{block}"
    );
    let entry = tested_common::tested_of(&plain(&kiln.repo, &[]), "KILN-RAMP");
    assert_eq!(
        entry,
        json!({
            "node": "KILN-RAMP",
            "state": "verified",
            "category": null,
            "rev": 1,
            "node_rev": 1,
            "commit": head,
            "verified_at": tested_common::DAY,
        })
    );

    // Step 3 (M: `spec_hash` ignored): the text edited, unraised: `stale
    // spec_changed`.
    kiln.repo.replace(
        KILN_MD,
        "The temperature rises by the ramp rate each hour and never passes the peak.",
        "The temperature rises by the ramp rate each hour and stops at the peak.",
    );
    assert_eq!(ramp_now(&kiln), pair("stale", "spec_changed"));

    // Step 4 (M: `stale` before `unverifiable`): raised to 2, its markers
    // still `@1`: `unverifiable spec_ahead`, `node_rev` 2.
    kiln.repo.replace(
        KILN_MD,
        "## Ramp {#KILN-RAMP}",
        "## Ramp {#KILN-RAMP rev=2}",
    );
    let run = plain(&kiln.repo, &[]);
    assert_eq!(
        state_of(&run, "KILN-RAMP"),
        pair("unverifiable", "spec_ahead")
    );
    assert_eq!(
        tested_common::tested_of(&run, "KILN-RAMP")["node_rev"],
        json!(2)
    );
    run.has_line("tested | unverifiable spec_ahead | KILN-RAMP");
    saved.restore(&kiln);
    assert_eq!(ramp_now(&kiln), pair("verified", ""));

    // Step 5 (M: the bound hash not compared, or a relation left out):
    // the implementation, the test, the bound RON value edited, or a
    // `@verifies` added: each alone `stale code_changed`.
    let edits: [(&str, &str, &str, &str); 4] = [
        (
            "the implementation",
            RAMP,
            "(now + rate).min(peak)",
            "now.saturating_add(rate).min(peak)",
        ),
        (
            "the test",
            RAMP,
            "assert_eq!(next_temperature(1190, 50, 1200), 1200);",
            "assert_eq!(next_temperature(1180, 50, 1200), 1200);",
        ),
        (
            "the bound RON value",
            FIRING_RON,
            "peak: 1200,",
            "peak: 1250,",
        ),
        (
            "a @verifies added",
            SCHEDULE,
            "// @verifies glaze-check/GLZ-01\n",
            "// @verifies glaze-check/GLZ-01\n// @verifies KILN-RAMP@1\n",
        ),
    ];
    for (what, file, from, to) in edits {
        kiln.repo.replace(file, from, to);
        assert_eq!(ramp_now(&kiln), pair("stale", "code_changed"), "{what}");
        saved.restore(&kiln);
    }

    // Step 6 (M: `file_blob` hashed, or a sibling value bound): a comment
    // elsewhere, a rustfmt-like respacing, a sibling RON value: `verified`.
    kiln.repo.append(RAMP, RAMP_COMMENT);
    kiln.repo.replace(RAMP, RAMP_SQUEEZED.0, RAMP_SQUEEZED.1);
    kiln.repo
        .replace(FIRING_RON, "hold_minutes: 60,", "hold_minutes: 90,");
    let run = plain(&kiln.repo, &[]);
    assert_eq!(state_of(&run, "KILN-RAMP"), pair("verified", ""));
    assert!(
        tested_common::tested_lines(&run).is_empty(),
        "no line for verified"
    );
    saved.restore(&kiln);

    // Step 7 (M5: a failed node keeps its block): a failing run of the task
    // removes the block; the lock has no block left: format 2 again.
    kiln.plan(&[(RAMP_STOPS_TEST, "fail")]);
    let run = kiln.run_task(&task);
    run.code(1);
    assert_eq!(run.node("KILN-RAMP")["verdict"], "failed");
    assert_eq!(
        run.json()["lock"]["rows"],
        json!([{"node": "KILN-RAMP", "change": "removed"}])
    );
    let lock = kiln.repo.lock_text();
    assert!(block_nodes(&lock).is_empty(), "{lock}");
    assert_eq!(
        lock.lines().nth(1),
        Some("format = 2"),
        "no block: format 2"
    );
    assert_eq!(
        tested(&plain(&kiln.repo, &[])),
        Vec::<serde_json::Value>::new()
    );
}
