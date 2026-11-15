//! docs/features/auto-rule.md RL1 (b), manual mode byte for byte: on
//! `fixtures/auto-mini` with no `[auto]` table, then again with
//! `rules/off.toml` committed, the manual commands give equal stdout (text
//! and `--json`), refusals and exits, through the library at one injected
//! clock (`spec task new` with no `SPECENGINE_AUTO_LINE`, `plan`, the
//! owner's `approve`, `claim`, `report`, `complete`, `spec propose update`,
//! `spec approve`, `spec review --json`, `spec inbox --json`, `spec export
//! state`); the two scratch repositories' paths and commit IDs are
//! normalized (both hold one commit more than the fixture's: the table, or
//! a comment). The dump holds no `auto.*` event, `task.approved` is
//! exactly `{"id":"T-0001"}`, `proposal.applied` exactly
//! `{"id":"PR-0001","commit":"<sha>"}` and the commit has no
//! `Delegated-by:`; `spec auto show --json` says `off` with the why.
//!
//! Mutations: a missing table read as `mode = "both"`; `by` always written.

#![cfg(unix)]

mod auto_common;
mod common;
mod task_common;

use std::fs;

use auto_common::{Auto, normalized};
use serde_json::{Value, json};
use specengine_cli::{
    ExportStateRequest, Globals, Outcome, export_state, render_json, render_text,
};
use task_common::{CLOCK, Tasks as _, document, line};

use common::proposal::{printed, printed_inbox};

/// The outputs of the manual night, labelled, in order.
fn manual_night(auto: &Auto) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let mut push = |label: &'static str, text: String| out.push((label, text));
    let (main, linked) = (auto.main.clone(), auto.linked.clone());

    let new = auto
        .new_task(
            &main,
            &["R-02"],
            Some("Fix the door timer"),
            Some("Close it."),
        )
        .expect("task new");
    push("task new", line(&new));
    push("task new --json", document(&new).to_string());
    let id = new.id.clone().expect("an ID");
    let plan = auto
        .plan(&main, &id, "1. Change the timer.\n", &["R-02"], &[])
        .expect("plan");
    push("task plan", line(&plan));
    push("task plan --json", document(&plan).to_string());
    let (approved, questions) = auto.owner(&main, "approve", &id, None, true, CLOCK);
    let approved = approved.expect("task approve");
    push("task approve", line(&approved));
    push("task approve --json", document(&approved).to_string());
    push("task approve: the question", questions.join("\n"));
    let claim = auto
        .claim(&linked, &id, "developer", &linked)
        .expect("claim");
    push("task claim", line(&claim));
    push("task claim --json", document(&claim).to_string());
    let report = auto
        .report(
            &linked,
            &id,
            "completed",
            "The timer resets.",
            &["src/lib.rs"],
        )
        .expect("report");
    push("task report", line(&report));
    let complete = auto.complete(&linked, &id).expect("complete");
    push("task complete", line(&complete));
    push("task complete --json", document(&complete).to_string());

    let pr = auto.propose_edit(&linked, "R-01", "three save slots", "four save slots");
    let (applied, questions) = auto.approve_answer(&main, &pr, true, auto.git_env(&main));
    let applied = applied.expect("approve");
    let (text, json) = printed(&applied);
    push("approve", text);
    push("approve --json", json);
    push("approve: the question", questions.join("\n"));
    let (text, json) = printed(&auto.review_ok(&main, &pr));
    push("review", text);
    push("review --json", json);
    let (text, json) = printed_inbox(&auto.inbox(&main, true).expect("inbox"));
    push("inbox --all", text);
    push("inbox --all --json", json);

    let dump = auto.scratch.join("dump.jsonl");
    let exported = export_state(
        &auto.env(&main),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(dump.clone()),
            now: CLOCK.to_owned(),
            git: auto.git_env(&main),
        },
    )
    .expect("export state");
    let outcome = Outcome::StateExport(exported);
    push("export state", render_text(&outcome));
    push("export state --json", render_json(&outcome));
    push("the dump", fs::read_to_string(&dump).expect("the dump"));
    out
}

/// RL1 (b). M: a missing table read as `mode = "both"`; `by` always
/// written.
#[test]
fn rl1_manual_mode_is_byte_for_byte_with_no_table_or_an_off_rule() {
    // No table: one more commit than the fixture's, a comment.
    let none = Auto::new("am-none");
    none.write_config(&format!("{}\n# no auto table\n", none.config().trim_end()));
    none.commit_all(&none.main, "auto: none");
    let off = Auto::with_rule("am-off", "off.toml");

    let a = manual_night(&none);
    let b = manual_night(&off);
    assert_eq!(a.len(), b.len());
    let mut failures = Vec::new();
    for ((label, left), (_, right)) in a.iter().zip(&b) {
        let left = normalized(left, &[none.scratch.path()]);
        let right = normalized(right, &[off.scratch.path()]);
        if left != right {
            failures.push(format!("{label}:\n  none: {left}\n  off:  {right}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    for auto in [&none, &off] {
        let events = auto.raw_events();
        assert!(
            events.iter().all(|event| !event.kind.starts_with("auto.")),
            "{events:?}"
        );
        let approved: Vec<Value> = auto
            .events_of_kind("task.approved")
            .iter()
            .map(auto_common::RawEvent::json)
            .collect();
        assert_eq!(approved, [json!({"id": "T-0001"})]);
        let applied = auto.events_of_kind("proposal.applied");
        assert_eq!(applied.len(), 1);
        let commit = auto.proposal("PR-0001").applied_commit.expect("applied");
        assert_eq!(
            applied[0].json(),
            json!({"id": "PR-0001", "commit": commit})
        );
        for event in auto.raw_events() {
            assert!(!event.payload.contains("\"by\":\"rule\""), "{event:?}");
        }
        let message = auto.message(&auto.linked, &commit);
        assert!(!message.contains("Delegated-by:"), "{message}");
    }
    let shown = none.json(&["auto", "show"], 0);
    assert_eq!(shown["auto"]["state"], "off");
    assert_eq!(
        shown["auto"]["why"],
        "no [auto] table in specengine.toml@HEAD"
    );
    let shown = off.json(&["auto", "show"], 0);
    assert_eq!(shown["auto"]["state"], "off");
    assert_eq!(shown["auto"]["why"], "[auto] mode is off");
}
