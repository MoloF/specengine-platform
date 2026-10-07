//! docs/features/task-package.md, the task's life in the queue: AC-01
//! (the transition table on both fixtures, every refused pair), AC-02
//! (the owner's three only on a terminal, consent `y`), AC-04 (a
//! discrepancy's linked update bound to the same task), AC-07 (no
//! blocking: open proposals on a `ready` task's node), AC-08 (the claim's
//! place; a bound proposal from another worktree), AC-10 (the review
//! document's `task_id`), AC-11 (the backup: format 2 with tasks and
//! runs), the store's corrupt rows as the CLI meets them, and
//! `git status` empty after every command (AC-09, the CLI half).
//!
//! Setup ("Acceptance criteria"): scratch git repositories of
//! `fixtures/spec-a` and `fixtures/spec-b` (`common::proposal::Pair`: a
//! main worktree on `main`, a linked one on `t1`, the sandbox's git, git
//! auto-maintenance off), a scratch `HOME`, a fixed clock and identity;
//! the owner's commands through the library with consent yes, the
//! terminal checks through the binary on a pipe and on a pseudo-terminal.
//! The oracle of "nothing changed" is `SqliteQueue::dump()` (every row of
//! every table).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod task_common;

use std::path::Path;

use common::proposal::{Pair, cannot, json_of};
use serde_json::{Value, json};
use specengine_cli::{Exit, TaskOutcome};
use task_common::{CLOCK, Tasks, document, done, line, refusal};

// ------------------------------------------------------------- AC-01

/// The transition table of "Data", written out here (not read from
/// core): `(action, the states it starts from, the state it leaves)`.
const TABLE: [(&str, &[&str], &str); 7] = [
    ("plan", &["draft", "changes_requested"], "review"),
    (
        "approve",
        &["draft", "review", "changes_requested", "ready"],
        "ready",
    ),
    ("changes", &["review"], "changes_requested"),
    ("claim", &["ready"], "in_progress"),
    ("report", &["in_progress"], "in_progress"),
    ("complete", &["in_progress"], "done"),
    (
        "cancel",
        &[
            "draft",
            "review",
            "changes_requested",
            "ready",
            "in_progress",
        ],
        "cancelled",
    ),
];

/// `a`, `a or b`, `a, b or c`, as the refusal names the states.
fn states(list: &[&str]) -> String {
    match list {
        [one] => (*one).to_owned(),
        [head @ .., last] => format!("{} or {last}", head.join(", ")),
        [] => unreachable!(),
    }
}

/// Runs `action` on `id` from `cwd` with valid arguments (claim in the
/// linked worktree): the outcome, and the questions an owner command
/// asked.
fn act(
    pair: &Pair,
    cwd: &Path,
    action: &str,
    id: &str,
) -> (Result<TaskOutcome, specengine_cli::CliError>, Vec<String>) {
    match action {
        "plan" => (
            pair.plan(cwd, id, "1. Do it.\n", &["Holds."], &[]),
            Vec::new(),
        ),
        "approve" | "cancel" => pair.owner(cwd, action, id, None, true, CLOCK),
        "changes" => pair.owner(cwd, action, id, Some("Say more."), true, CLOCK),
        "claim" => (pair.claim(cwd, id, "developer", &pair.linked), Vec::new()),
        // Report and complete run where the task is claimed (every claim
        // here is in the linked worktree): iteration 2, m3.
        "report" => (
            pair.report(&pair.linked, id, "completed", "Done.", &["a.txt"]),
            Vec::new(),
        ),
        "complete" => (pair.complete(&pair.linked, id), Vec::new()),
        other => panic!("no action {other}"),
    }
}

/// The events of task `id`: `(type, payload)` in `seq` order.
fn events_of(pair: &Pair, id: &str) -> Vec<(String, Value)> {
    pair.task_events()
        .into_iter()
        .filter(|(_, payload)| payload["id"] == id)
        .collect()
}

/// One fixture's lifecycle: every row of the table in turn on `T-0001`
/// (plan, changes, plan, approve from `review`, approve again from
/// `ready`, claim, report, complete), approve from `draft` and from
/// `changes_requested`, cancel from each of the five open states; each a
/// line, a `{id, status, run, notes}` document and exactly one event of
/// the action's name (`run` in claim's and report's). Then every pair the
/// table does not hold, on a task in each reachable state (`in_progress`
/// twice: its run open and closed): exit 1 with the "Data" message,
/// `dump()` unchanged, no consent asked. M: claim accepts `review`.
fn lifecycle(label: &str, fixture: &str, target: &str) {
    let pair = Pair::new(label, fixture);
    let main = pair.main.clone();
    let mut seen = 0usize;
    let mut one_event = |pair: &Pair, id: &str, kind: &str, run: Option<u64>| {
        let events = pair.task_events();
        assert_eq!(events.len(), seen + 1, "{kind}: one event: {events:?}");
        seen = events.len();
        let (event_type, payload) = events.last().unwrap();
        assert_eq!(event_type, &format!("task.{kind}"), "{events:?}");
        let mut want = json!({ "id": id });
        if let Some(run) = run {
            want["run"] = json!(run);
        }
        assert_eq!(payload, &want, "{kind}");
    };

    let created = done(
        pair.new_task(&main, &[target], Some("Tune it"), Some("A goal.")),
        "new",
    );
    assert_eq!(line(&created), "created T-0001\n");
    assert_eq!(
        document(&created),
        json!({"id": "T-0001", "status": "draft", "run": null, "notes": []})
    );
    one_event(&pair, "T-0001", "created", None);

    let steps: [(&str, &str, &str, Option<u64>, &str); 8] = [
        ("plan", "review", "planned", None, "planned"),
        (
            "changes",
            "changes_requested",
            "returned",
            None,
            "changes_requested",
        ),
        ("plan", "review", "planned", None, "planned"),
        ("approve", "ready", "approved", None, "approved"),
        ("approve", "ready", "approved", None, "approved"),
        ("claim", "in_progress", "claimed", Some(1), "claimed"),
        ("report", "in_progress", "reported", Some(1), "run_reported"),
        ("complete", "done", "completed", None, "completed"),
    ];
    for (action, status, verb, run, event) in steps {
        let (outcome, questions) = act(&pair, &main, action, "T-0001");
        let outcome = done(outcome, action);
        let suffix = run.map_or_else(String::new, |run| format!(" (run {run})"));
        assert_eq!(
            line(&outcome),
            format!("{verb} T-0001: {status}{suffix}\n"),
            "{action}"
        );
        assert_eq!(
            document(&outcome),
            json!({"id": "T-0001", "status": status, "run": run, "notes": []}),
            "{action}"
        );
        let owner = matches!(action, "approve" | "changes" | "cancel");
        assert_eq!(
            questions.len(),
            usize::from(owner),
            "{action}: {questions:?}"
        );
        one_event(&pair, "T-0001", event, run);
    }
    assert_eq!(
        events_of(&pair, "T-0001")
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<Vec<_>>(),
        [
            "task.created",
            "task.planned",
            "task.changes_requested",
            "task.planned",
            "task.approved",
            "task.approved",
            "task.claimed",
            "task.run_reported",
            "task.completed"
        ]
    );

    // Approve from `draft` (T-0002) and from `changes_requested` (T-0003).
    assert_eq!(pair.new_ok(&main, &[target]), "T-0002");
    one_event(&pair, "T-0002", "created", None);
    let outcome = done(act(&pair, &main, "approve", "T-0002").0, "approve draft");
    assert_eq!(line(&outcome), "approved T-0002: ready\n");
    one_event(&pair, "T-0002", "approved", None);
    assert_eq!(pair.new_ok(&main, &[target]), "T-0003");
    one_event(&pair, "T-0003", "created", None);
    for (action, event) in [
        ("plan", "planned"),
        ("changes", "changes_requested"),
        ("approve", "approved"),
    ] {
        done(act(&pair, &main, action, "T-0003").0, action);
        one_event(&pair, "T-0003", event, None);
    }

    // Cancel from each open state: T-0004 draft, T-0005 review, T-0006
    // changes_requested, T-0007 ready, T-0008 in_progress.
    let reach: [&[&str]; 5] = [
        &[],
        &["plan"],
        &["plan", "changes"],
        &["approve"],
        &["approve", "claim"],
    ];
    for (number, path) in (4..).zip(reach) {
        let id = pair.new_ok(&main, &[target]);
        assert_eq!(id, format!("T-{number:04}"));
        one_event(&pair, &id, "created", None);
        for action in path {
            let outcome = done(act(&pair, &main, action, &id).0, action);
            let run = (*action == "claim").then_some(1);
            let event = match *action {
                "plan" => "planned",
                "changes" => "changes_requested",
                "approve" => "approved",
                _ => "claimed",
            };
            assert_eq!(outcome.run, run);
            one_event(&pair, &id, event, run);
        }
        let from = pair.show_ok(&main, &id).package.unwrap().status;
        let (outcome, questions) = act(&pair, &main, "cancel", &id);
        let outcome = done(outcome, "cancel");
        assert_eq!(line(&outcome), format!("cancelled {id}: cancelled\n"));
        assert_eq!(questions.len(), 1);
        assert!(
            questions[0].starts_with(&format!("cancel {id} ({from}, Tune the regeneration)? ")),
            "{questions:?}"
        );
        one_event(&pair, &id, "cancelled", None);
    }

    // A task in each reachable state; in_progress twice.
    let mut at: Vec<(String, &str, bool)> = vec![
        ("T-0001".to_owned(), "done", false),
        ("T-0004".to_owned(), "cancelled", false),
    ];
    let shapes: [(&str, &[&str], bool); 6] = [
        ("draft", &[], false),
        ("review", &["plan"], false),
        ("changes_requested", &["plan", "changes"], false),
        ("ready", &["approve"], false),
        ("in_progress", &["approve", "claim"], true),
        ("in_progress", &["approve", "claim", "report"], false),
    ];
    for (status, path, open) in shapes {
        let id = pair.new_ok(&main, &[target]);
        for action in path {
            done(act(&pair, &main, action, &id).0, action);
        }
        at.push((id, status, open));
    }
    let mut refused = 0;
    for (id, status, open) in &at {
        for (action, from, _) in TABLE {
            let allowed = from.contains(status)
                && match action {
                    "report" => *open,
                    "complete" => !*open,
                    _ => true,
                };
            if allowed {
                continue;
            }
            let before = pair.dump();
            let (outcome, questions) = act(&pair, &main, action, id);
            let context = format!("{action} on {id} ({status}, run open {open})");
            let reason = refusal(&outcome, &context);
            let want = match (*status, action) {
                ("in_progress", "report") => format!(
                    "{id} is in_progress, no run open: `report` needs its open run (`claim` opens \
                     one); nothing changed"
                ),
                ("in_progress", "complete") => format!(
                    "{id} is in_progress, its run 1 open: `complete` needs it closed (`report`); \
                     nothing changed"
                ),
                _ => format!(
                    "{id} is {status}: `{action}` needs {}; nothing changed",
                    states(from)
                ),
            };
            assert_eq!(reason, want, "{context}");
            let refused_outcome = outcome.unwrap();
            assert_eq!(
                refused_outcome.id.as_deref(),
                Some(id.as_str()),
                "{context}"
            );
            assert_eq!(
                refused_outcome.status.map(|status| status.as_str()),
                Some(*status),
                "{context}"
            );
            assert_eq!(line(&refused_outcome), "", "{context}: no stdout line");
            assert!(
                questions.is_empty(),
                "{context}: no consent asked: {questions:?}"
            );
            assert_eq!(pair.dump(), before, "{context}: nothing changed");
            refused += 1;
        }
    }
    // 8 tasks x 7 actions, minus the 16 pairs the table (and the run) holds.
    assert_eq!(refused, 8 * 7 - 16);
    assert_eq!(pair.porcelain(&main), "", "nothing written under the root");
    assert_eq!(pair.porcelain(&pair.linked), "");
}

#[test]
fn ac01_the_transition_table_on_spec_a() {
    lifecycle("tk-ac01-a", "spec-a", "MEC-STAMINA");
}

#[test]
fn ac01_the_transition_table_on_spec_b() {
    lifecycle("tk-ac01-b", "spec-b", "CMD-SYNC");
}

// ------------------------------------------------------------- AC-02

/// AC-02, the library half: the owner's three ask once and, on any answer
/// but yes, refuse (exit 1) with "Data"'s reason, `dump()` unchanged, no
/// event; approve's question names the task, its title, the nodes it
/// freezes and the place. The binary half below.
#[test]
fn ac02_a_declined_owner_command_changes_nothing() {
    let pair = Pair::new("tk-ac02-lib", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA", "RULE-SPRINT-COST"]);
    let head = pair.rev(&main, "HEAD");
    let before = pair.dump();
    let (outcome, questions) = pair.owner(&main, "approve", &id, None, false, CLOCK);
    assert_eq!(
        refusal(&outcome, "approve declined"),
        "`T-0001` not approved: the answer was not `y`; nothing changed"
    );
    assert_eq!(
        questions,
        [format!(
            "approve T-0001 (Tune the regeneration), freezing 2 node(s) of {} on main at {head}? \
             [y/N]",
            main.display()
        )]
    );
    assert_eq!(pair.dump(), before);

    done(pair.plan(&main, &id, "1. Do.\n", &[], &[]), "plan");
    let before = pair.dump();
    let (outcome, questions) = pair.owner(&main, "changes", &id, Some("More."), false, CLOCK);
    assert_eq!(
        refusal(&outcome, "changes declined"),
        "`T-0001` changes not requested: the answer was not `y`; nothing changed"
    );
    assert_eq!(
        questions,
        ["request changes of T-0001 (Tune the regeneration)? [y/N]"]
    );
    assert_eq!(pair.dump(), before);
    let (outcome, questions) = pair.owner(&main, "cancel", &id, None, false, CLOCK);
    assert_eq!(
        refusal(&outcome, "cancel declined"),
        "`T-0001` not cancelled: the answer was not `y`; nothing changed"
    );
    assert_eq!(
        questions,
        ["cancel T-0001 (review, Tune the regeneration)? [y/N]"]
    );
    assert_eq!(pair.dump(), before);
    // A blank note is refused before the question.
    let (outcome, questions) = pair.owner(&main, "changes", &id, Some("  "), true, CLOCK);
    assert_eq!(
        refusal(&outcome, "blank note"),
        "note: blank; say what the plan must change"
    );
    assert!(questions.is_empty());
    assert_eq!(pair.dump(), before);
}

/// AC-02, the binary half: `spec task approve|changes|cancel` with a piped
/// stdin holding `y` exit 2 naming the terminal, the queue unchanged; on a
/// pseudo-terminal `n` exits 1, no event, and `y` moves the task (one
/// event). M: the terminal check removed (the piped `y` approves).
#[test]
fn ac02_the_owner_commands_run_only_on_a_terminal() {
    let pair = Pair::new("tk-ac02-tty", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    done(pair.plan(&main, &id, "1. Do.\n", &[], &[]), "plan");
    let before = pair.dump();
    for args in [
        vec!["task", "approve", "T-0001"],
        vec!["task", "changes", "T-0001", "--note", "More."],
        vec!["task", "cancel", "T-0001"],
    ] {
        let run = pair.spec_piped(&main, &args, b"y\n");
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        let command = args[..2].join(" ");
        assert_eq!(
            run.stderr,
            format!(
                "spec: `spec {command}` asks the owner for consent on a terminal, and stdin is \
                 not one (a pipe, a script or an agent's shell): run it in a terminal; nothing \
                 changed\n"
            ),
            "{}",
            run.show()
        );
        assert_eq!(pair.dump(), before, "{args:?}: nothing changed");
    }

    let declined = pair.spec_pty(&main, &["task", "approve", "T-0001"], Some("n"));
    assert!(declined.asked, "{}", declined.output);
    assert_eq!(declined.code, 1, "{}", declined.output);
    assert!(
        declined
            .output
            .contains("`T-0001` not approved: the answer was not `y`; nothing changed"),
        "{}",
        declined.output
    );
    assert_eq!(pair.dump(), before, "no event");
    let approved = pair.spec_pty(&main, &["task", "approve", "T-0001"], Some("y"));
    assert_eq!(approved.code, 0, "{}", approved.output);
    assert!(
        approved.output.contains("approved T-0001: ready\n"),
        "{}",
        approved.output
    );
    let events = events_of(&pair, &id);
    assert_eq!(
        events.last().map(|(kind, _)| kind.as_str()),
        Some("task.approved")
    );
    assert_eq!(events.len(), 3, "created, planned, approved: {events:?}");
    let cancelled = pair.spec_pty(&main, &["task", "cancel", "T-0001"], Some("yes"));
    assert_eq!(cancelled.code, 0, "{}", cancelled.output);
    assert!(cancelled.output.contains("cancelled T-0001: cancelled\n"));
    assert_eq!(pair.porcelain(&main), "");
}

// ------------------------------------------------------------- AC-04

/// A discrepancy of `RULE-STAM-REGEN` with a patch of its span read in
/// `cwd` (the delay `1.5 s` → `2 s`).
fn gap_with_patch(pair: &Pair, cwd: &Path) -> specengine_cli::DiscrepancyInput {
    let (hash, text) = pair.span(cwd, "RULE-STAM-REGEN");
    let mut input = common::decision::discrepancy(
        &["RULE-STAM-REGEN"],
        "The delay is 2 s in the code.",
        &common::decision::THREE,
    );
    input.proposed_patch = Some(specengine_cli::ProposedPatch {
        target: "RULE-STAM-REGEN".to_owned(),
        base: hash,
        text: common::proposal::edit(&text, "1.5 s", "2 s"),
        rationale: "The code's delay.".to_owned(),
    });
    input
}

/// AC-04: `propose discrepancy … --task T-0001` with a patch stores the
/// discrepancy and its linked update both bound (`task_id` `T-0001`, in
/// the review documents too); the package lists both, the discrepancy's
/// recommended option as an assumption. M: the update's `task_id` NULL.
#[test]
fn ac04_a_discrepancys_linked_update_is_bound_to_the_same_task() {
    let pair = Pair::new("tk-ac04", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    let outcome = task_common::report_gap(
        &pair,
        &pair.linked,
        gap_with_patch(&pair, &pair.linked),
        Some(&id),
    );
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    let reported = outcome.document.id.clone().expect("stored");
    let linked = pair.proposal(&reported).linked.expect("a linked update");
    for proposal in [&reported, &linked] {
        assert_eq!(
            pair.proposal(proposal).task_id.as_deref(),
            Some("T-0001"),
            "{proposal}"
        );
        let review = json_of(&common::proposal::printed(&pair.review_ok(&main, proposal)).1);
        assert_eq!(review["task_id"], json!("T-0001"), "{proposal}");
    }
    let package = pair.package(&main, &id);
    let listed: Vec<(&str, &str, &str)> = package["open_proposals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["id"].as_str().unwrap(),
                item["kind"].as_str().unwrap(),
                item["task_id"].as_str().unwrap_or("null"),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            (reported.as_str(), "discrepancy", "T-0001"),
            (linked.as_str(), "update", "T-0001")
        ]
    );
    assert_eq!(
        package["open_proposals"][0]["summary"],
        json!("The delay is 2 s in the code.")
    );
    assert_eq!(
        package["assumptions"],
        json!([{"proposal": reported, "text": "Keep the spec"}])
    );
    // An unbound one stays unbound; cancel retires the task, not them.
    let free = task_common::ask_ok(&pair, &main, &["EDGE-STAM-ZERO"], "Zero?", "Yes.", None);
    assert_eq!(pair.proposal(&free).task_id, None);
    let (outcome, _) = pair.owner(&main, "cancel", &id, None, true, CLOCK);
    done(outcome, "cancel");
    for proposal in [&reported, &linked] {
        let stored = pair.proposal(proposal);
        assert_eq!(stored.status.as_str(), "open", "{proposal}");
        assert_eq!(stored.task_id.as_deref(), Some("T-0001"), "{proposal}");
    }
    // A cancelled task takes no new proposal; an unknown one neither.
    let before = pair.dump();
    for (task, want) in [
        (
            "T-0001",
            "--task: `T-0001` is cancelled: a proposal is bound only to a task that is neither \
             done nor cancelled",
        ),
        ("T-0099", "--task: no task T-0099 in this repository"),
    ] {
        let outcome = task_common::ask(
            &pair,
            &main,
            &["EDGE-STAM-ZERO"],
            "Later?",
            "No.",
            Some(task),
        );
        assert_eq!(outcome.exit(), Exit::NotFound, "{task}: {outcome:?}");
        assert_eq!(outcome.refusal.as_deref(), Some(want), "{task}");
        let refused = task_common::propose_bound(
            &pair,
            &main,
            "EDGE-SPRINT-EMPTY",
            "the sprint ends;",
            "the sprint stops;",
            Some(task),
        );
        assert_eq!(
            refused.refusal.as_deref(),
            Some(want),
            "{task}: {refused:?}"
        );
        assert_eq!(pair.dump(), before, "{task}: nothing stored");
    }
    assert_eq!(pair.porcelain(&main), "");
    assert_eq!(pair.porcelain(&pair.linked), "");
}

// ------------------------------------------------------------- AC-07

/// AC-07: a `ready` task's node with two open questions, one bound: the
/// status stays `ready`, approve (again) and claim succeed, the package
/// lists both open proposals (`task_id` set on the bound one only) and
/// both working answers as assumptions. M: approve refuses.
#[test]
fn ac07_open_proposals_never_block_a_task() {
    let pair = Pair::new("tk-ac07", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    pair.approve_task(&main, &id);
    let bound = task_common::ask_ok(
        &pair,
        &main,
        &["MEC-STAMINA"],
        "Does stamina drain while climbing?",
        "Yes, at the sprint rate.",
        Some(&id),
    );
    let free = task_common::ask_ok(
        &pair,
        &pair.linked,
        &["MEC-STAMINA", "RULE-STAM-REGEN"],
        "Is regeneration paused in water?",
        "No.",
        None,
    );
    assert_eq!(
        pair.show_ok(&main, &id).package.unwrap().status.as_str(),
        "ready"
    );
    let again = pair.approve_task(&main, &id);
    assert_eq!(line(&again), "approved T-0001: ready\n");
    let claimed = pair.claim_ok(&main, &id, &pair.linked);
    assert_eq!(line(&claimed), "claimed T-0001: in_progress (run 1)\n");
    let package = pair.package(&main, &id);
    assert_eq!(
        package["open_proposals"],
        json!([
            {"id": bound, "kind": "question", "status": "open", "target_ids": ["MEC-STAMINA"],
             "task_id": "T-0001", "summary": "Does stamina drain while climbing?"},
            {"id": free, "kind": "question", "status": "open",
             "target_ids": ["MEC-STAMINA", "RULE-STAM-REGEN"], "task_id": null,
             "summary": "Is regeneration paused in water?"}
        ])
    );
    assert_eq!(
        package["assumptions"],
        json!([
            {"proposal": bound, "text": "Yes, at the sprint rate."},
            {"proposal": free, "text": "No."}
        ])
    );
}

// ------------------------------------------------------------- AC-08

/// AC-08: a claim naming a worktree of another repository, a plain
/// directory, a missing one, a detached `HEAD`, a control character, or a
/// role that is no author field → exit 1 naming why, nothing recorded;
/// then claimed in `t1`, a bound proposal from the main worktree → exit 1
/// naming `t1`, from `t1` stored. Another repository's task (same slug,
/// same `HOME`) cannot run there (exit 2). M: any worktree.
#[test]
fn ac08_a_claim_names_a_worktree_of_the_tasks_repository_on_a_branch() {
    let pair = Pair::new("tk-ac08", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    pair.approve_task(&main, &id);

    // Another repository of the same fixture and slug.
    let other = pair.scratch.copy("spec-a", "other");
    pair.git.init(&other);
    pair.git.add_all(&other);
    pair.git.commit(&other, "another repository");
    // A detached worktree of this repository; a plain directory.
    let detached = pair.scratch.join("t2");
    pair.git.git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            detached.to_str().unwrap(),
        ],
    );
    let plain = pair.scratch.dir("plain");
    let control = pair.linked.join("a\u{1}b");

    let before = pair.dump();
    let cases: [(&str, &Path, &str, &str); 6] = [
        (
            "another repository",
            &other,
            "developer",
            "is no worktree of the task's repository",
        ),
        (
            "a plain directory",
            &plain,
            "developer",
            "is no worktree of the task's repository",
        ),
        (
            "a missing directory",
            &pair.scratch.join("gone"),
            "developer",
            "does not exist",
        ),
        (
            "a detached HEAD",
            &detached,
            "developer",
            "has a detached HEAD",
        ),
        (
            "a control character",
            &control,
            "developer",
            "worktree: holds a control character",
        ),
        ("a role with a space", &pair.linked, "lead dev", "role: "),
    ];
    for (name, dir, role, want) in cases {
        let outcome = pair.claim(&main, &id, role, dir);
        let reason = refusal(&outcome, name);
        assert!(reason.contains(want), "{name}: {reason}");
        assert_eq!(pair.dump(), before, "{name}: nothing recorded");
    }
    let claimed = pair.claim_ok(&main, &id, &pair.linked);
    assert_eq!(claimed.run, Some(1));
    let claim = &pair.package(&main, &id)["claim"];
    assert_eq!(claim["worktree"], json!(pair.linked.display().to_string()));
    assert_eq!(claim["branch"], json!("t1"));
    assert_eq!(claim["role"], json!("developer"));

    let before = pair.dump();
    let want = format!(
        "--task: `T-0001` is claimed in the worktree {}: a proposal bound to it is raised there, \
         not in {}",
        pair.linked.display(),
        main.display()
    );
    let outcome = task_common::ask(&pair, &main, &["MEC-STAMINA"], "Climb?", "Yes.", Some(&id));
    assert_eq!(
        outcome.refusal.as_deref(),
        Some(want.as_str()),
        "{outcome:?}"
    );
    let refused = task_common::propose_bound(
        &pair,
        &main,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
        Some(&id),
    );
    assert_eq!(refused.refusal.as_deref(), Some(want.as_str()));
    assert_eq!(pair.dump(), before, "nothing stored");
    let bound = task_common::ask_ok(
        &pair,
        &pair.linked,
        &["MEC-STAMINA"],
        "Climb?",
        "Yes.",
        Some(&id),
    );
    assert_eq!(pair.proposal(&bound).task_id.as_deref(), Some("T-0001"));

    // The other repository: the same slug, `HOME` and queue.
    let shown = pair.show(&other, &id);
    let message = cannot(&shown, "another repository's task");
    assert!(
        message.contains("`T-0001` belongs to another repository of the project `lantern-keep`"),
        "{message}"
    );
    let listed = pair.list(&other);
    assert!(listed.tasks.is_empty(), "{listed:?}");
    assert_eq!(
        listed.notes,
        [
            "1 task(s) of another repository of the project `lantern-keep` not listed: `spec task \
          list` lists the current repository's"
        ]
    );
    for dir in [&main, &pair.linked, &other] {
        assert_eq!(pair.porcelain(dir), "", "{}", dir.display());
    }
}

// ------------------------------------------------------------- AC-10

/// AC-10: `review --json` holds `task_id` right after `choice`, before
/// `notes` (43 keys; 45 since docs/features/decision-staging.md, `staged`,
/// `staged_at` between `task_id` and `notes`; `T-0001` bound, `null`
/// unbound), its text a `task_id:` line; inbox entries keep their eleven
/// keys (twelve with `staged_at`), none `task_id`. M: `task_id` after
/// `notes`.
#[test]
fn ac10_the_review_document_names_its_task_and_the_inbox_does_not() {
    let pair = Pair::new("tk-ac10", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["EDGE-SPRINT-EMPTY"]);
    let bound = task_common::propose_bound_ok(
        &pair,
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
        Some(&id),
    );
    let free = task_common::propose_bound_ok(
        &pair,
        &pair.linked,
        "RULE-SPRINT-COST",
        "12 units/s",
        "14 units/s",
        None,
    );
    for (proposal, want) in [(&bound, json!("T-0001")), (&free, Value::Null)] {
        let (text, raw) = common::proposal::printed(&pair.review_ok(&main, proposal));
        let value = json_of(&raw);
        let keys = task_common::keys(&raw);
        assert_eq!(keys.len(), 45, "{keys:?}");
        assert_eq!(
            keys[40..],
            ["choice", "task_id", "staged", "staged_at", "notes"],
            "{keys:?}"
        );
        assert_eq!(value["task_id"], want, "{proposal}");
        let line = format!("\ntask_id: {}\n", want.as_str().unwrap_or("-"));
        assert!(text.contains(&line), "{proposal}: {text}");
        let at = |key: &str| text.find(&format!("\n{key}:")).expect(key);
        assert!(
            at("choice") < at("task_id") && at("task_id") < at("notes"),
            "{text}"
        );
    }
    let inbox = pair.inbox(&main, true).expect("inbox");
    let listed = task_common::Ordered::of(&common::proposal::printed_inbox(&inbox).1);
    for index in 0..2 {
        let keys = listed.get("proposals").at(index).keys();
        assert_eq!(keys.len(), 12, "{keys:?}");
        assert!(!keys.contains(&"task_id".to_owned()), "{keys:?}");
    }
}

// ------------------------------------------------------------- AC-11

fn env_at(home: &Path, cwd: &Path) -> specengine_cli::Env {
    specengine_cli::Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

fn export(pair: &Pair, home: &Path, out: &Path) -> specengine_cli::ExportStateOutcome {
    specengine_cli::export_state(
        &env_at(home, &pair.main),
        &specengine_cli::Globals::default(),
        &specengine_cli::ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export state: {error}"))
}

fn import(
    pair: &Pair,
    home: &Path,
    file: &Path,
) -> (
    Result<specengine_cli::ImportStateOutcome, specengine_cli::CliError>,
    Vec<String>,
) {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    let outcome = specengine_cli::import_state(
        &env_at(home, &pair.main),
        &specengine_cli::Globals::default(),
        &specengine_cli::ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    );
    (outcome, questions)
}

fn dump_in(home: &Path, slug: &str) -> String {
    specengine_store::SqliteQueue::open(common::data_dir(home).join(format!("{slug}.db")), slug)
        .expect("open")
        .dump()
        .expect("dump")
}

fn rendered(outcome: specengine_cli::Outcome) -> (String, Value) {
    (
        specengine_cli::render_text(&outcome),
        json_of(&specengine_cli::render_json(&outcome)),
    )
}

/// A queue of every task shape: `T-0001` done (one run, reported), `T-0002`
/// in progress (its run open), `T-0003` returned with a note, `T-0004`
/// cancelled, `T-0005` a draft; a bound question and an unbound update.
fn tasks_queue(pair: &Pair) {
    let main = pair.main.clone();
    for _ in 0..5 {
        pair.new_ok(&main, &["MEC-STAMINA"]);
    }
    pair.approve_task(&main, "T-0001");
    pair.claim_ok(&main, "T-0001", &pair.linked);
    pair.report_ok(&pair.linked, "T-0001");
    done(pair.complete(&pair.linked, "T-0001"), "complete");
    pair.approve_task(&main, "T-0002");
    pair.claim_ok(&main, "T-0002", &main);
    pair.plan_ok(
        &main,
        "T-0003",
        &["MEC-STAMINA", "Free text."],
        &["RULE-STAM-REGEN"],
    );
    done(
        pair.owner(&main, "changes", "T-0003", Some("Split it."), true, CLOCK)
            .0,
        "changes",
    );
    done(
        pair.owner(&main, "cancel", "T-0004", None, true, CLOCK).0,
        "cancel",
    );
    task_common::ask_ok(
        pair,
        &main,
        &["MEC-STAMINA"],
        "Climb?",
        "Yes.",
        Some("T-0002"),
    );
    task_common::propose_bound_ok(
        pair,
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
        None,
    );
}

/// AC-11: export (format 2: the seven-key header, proposals, tasks by
/// number, runs by task and number, events), import into a fresh queue
/// (the prompt and every count line as "Data"), export again:
/// byte-identical, `dump()` equal; the import's and export's `--json`
/// carry `tasks` and `runs`. M: tasks left out.
#[test]
fn ac11_tasks_and_runs_round_trip_through_a_format_2_backup() {
    let pair = Pair::new("tk-ac11", "spec-a");
    tasks_queue(&pair);
    let before = pair.dump();
    let events = pair.events().len();
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    let exported = export(&pair, &pair.home, &file);
    assert_eq!(
        (
            exported.proposals,
            exported.tasks,
            exported.runs,
            exported.events
        ),
        (2, 5, 2, events as u64)
    );
    let (text, document) = rendered(specengine_cli::Outcome::StateExport(exported));
    assert_eq!(
        text,
        format!(
            "wrote {}: 2 proposal(s), 5 task(s), 2 run(s), {events} event(s)\n",
            file.display()
        )
    );
    assert_eq!(
        document,
        json!({"path": file.display().to_string(), "proposals": 2, "tasks": 5, "runs": 2,
               "events": events})
    );
    let bytes = std::fs::read(&file).expect("the dump");
    let lines: Vec<&str> = std::str::from_utf8(&bytes).unwrap().lines().collect();
    assert_eq!(
        lines[0],
        format!(
            "{{\"format\":2,\"queue_schema\":5,\"project\":\"lantern-keep\",\"proposals\":2,\
             \"tasks\":5,\"runs\":2,\"events\":{events}}}"
        )
    );
    assert_eq!(lines.len(), 1 + 2 + 5 + 2 + events);
    let table = |line: &str| -> String { task_common::keys(line)[0].clone() };
    let tables: Vec<String> = lines[1..].iter().map(|line| table(line)).collect();
    let mut want = vec!["proposals"; 2];
    want.extend(["tasks"; 5]);
    want.extend(["runs"; 2]);
    want.extend(vec!["events"; events]);
    assert_eq!(tables, want);
    for (index, line) in lines[3..8].iter().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            task_common::Ordered::of(line).get("tasks").keys(),
            specengine_store::TASK_COLUMNS,
            "{line}"
        );
        assert_eq!(row["tasks"]["id"], json!(format!("T-{:04}", index + 1)));
    }
    for (line, task) in lines[8..10].iter().zip(["T-0001", "T-0002"]) {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            task_common::Ordered::of(line).get("runs").keys(),
            specengine_store::RUN_COLUMNS
        );
        assert_eq!(row["runs"]["task_id"], json!(task));
        assert_eq!(row["runs"]["run"], json!(1), "a number");
    }
    let bound: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(bound["proposals"]["task_id"], json!("T-0002"));

    let fresh = pair.scratch.home("fresh");
    let (outcome, questions) = import(&pair, &fresh, &file);
    let outcome = outcome.unwrap_or_else(|error| panic!("import: {error}"));
    let db = common::data_dir(&fresh).join("lantern-keep.db");
    assert_eq!(
        questions,
        [format!(
            "restore 2 proposal(s), 5 task(s), 2 run(s) and {events} event(s) of lantern-keep \
             from {} into {}? [y/N]",
            file.display(),
            db.display()
        )]
    );
    let (text, document) = rendered(specengine_cli::Outcome::StateImport(outcome));
    assert_eq!(
        text,
        format!(
            "restored 2 proposal(s), 5 task(s), 2 run(s), {events} event(s) into {}\n",
            db.display()
        )
    );
    assert_eq!(
        document,
        json!({"db": db.display().to_string(), "proposals": 2, "tasks": 5, "runs": 2,
               "events": events})
    );
    assert_eq!(dump_in(&fresh, "lantern-keep"), before, "dump() equal");
    let again = dumps.join("again.jsonl");
    export(&pair, &fresh, &again);
    assert_eq!(std::fs::read(&again).unwrap(), bytes, "byte-identical");
    // The restored queue reads the same packages (the same tree).
    let restored =
        task_common::Tasks::package_text(&RestoredHome(&pair, &fresh), &pair.main, "T-0001");
    assert_eq!(restored, pair.package_text(&pair.main, "T-0001"));
    // A queue holding the tasks refuses a second import, naming them.
    let (outcome, questions) = import(&pair, &fresh, &file);
    let message = cannot(&outcome, "occupied");
    assert!(
        message.contains(&format!(
            "holds 2 proposal(s), 5 task(s), 2 run(s), {events} event(s)"
        )),
        "{message}"
    );
    assert!(questions.is_empty());
}

/// The pair's commands against another data directory `HOME`.
struct RestoredHome<'a>(&'a Pair, &'a Path);

impl task_common::Tasks for RestoredHome<'_> {
    fn pair(&self) -> &Pair {
        self.0
    }

    fn show(
        &self,
        cwd: &Path,
        id: &str,
    ) -> Result<specengine_cli::TaskShowOutcome, specengine_cli::CliError> {
        specengine_cli::task_show(
            &env_at(self.1, cwd),
            &specengine_cli::Globals::default(),
            &specengine_cli::TaskShowRequest {
                id: Some(id.to_owned()),
                next: false,
                git: self.0.git_env(cwd),
            },
        )
    }
}

/// AC-11: format-1 dumps of queue schemas 1, 2 and 3 restore, the later
/// columns `NULL`, and re-export as format 2 (schema 3 here; 1 and 2:
/// `intake_state.rs`, `decision_apply.rs`); a database still at schema 3
/// (no `tasks`, `runs`, `task_id`, stage) exports as format 2, schema 5
/// (docs/features/decision-staging.md "Backup") without stepping.
/// Refused, naming line 1, nothing made: a format-2 header of six keys or
/// of `queue_schema` 3, a format-1 header of seven keys; a format-1 row
/// holding `task_id`; a dump into a queue holding only a task. M: tasks
/// left out.
#[test]
fn ac11_older_formats_restore_and_bad_headers_are_refused() {
    let pair = Pair::new("tk-ac11-old", "spec-a");
    task_common::propose_bound_ok(
        &pair,
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
        None,
    );
    let dumps = pair.scratch.dir("dumps");
    let current = dumps.join("v4.jsonl");
    export(&pair, &pair.home, &current);
    let v4 = std::fs::read_to_string(&current).unwrap();
    let v4_lines: Vec<String> = v4.lines().map(str::to_owned).collect();
    assert_eq!(
        v4_lines[0],
        "{\"format\":2,\"queue_schema\":5,\"project\":\"lantern-keep\",\"proposals\":1,\
         \"tasks\":0,\"runs\":0,\"events\":1}"
    );
    // Format 1, schema 3: the header's five keys, rows without `task_id`
    // and the stage's two.
    let later = ",\"task_id\":null,\"staged\":null,\"staged_at\":null}}";
    let mut v3_lines = v4_lines.clone();
    v3_lines[0] = "{\"format\":1,\"queue_schema\":3,\"project\":\"lantern-keep\",\"proposals\":1,\
                   \"events\":1}"
        .to_owned();
    assert!(v3_lines[1].ends_with(later), "{}", v3_lines[1]);
    v3_lines[1] = format!("{}}}}}", &v3_lines[1][..v3_lines[1].len() - later.len()]);
    let write = |name: &str, lines: &[String]| {
        let file = dumps.join(name);
        std::fs::write(&file, format!("{}\n", lines.join("\n"))).unwrap();
        file
    };
    let v3 = write("v3.jsonl", &v3_lines);
    let home = pair.scratch.home("v3");
    let (outcome, _) = import(&pair, &home, &v3);
    let outcome = outcome.unwrap_or_else(|error| panic!("import v3: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered);
    assert_eq!(dump_in(&home, "lantern-keep"), pair.dump(), "task_id NULL");
    let again = dumps.join("v3-again.jsonl");
    export(&pair, &home, &again);
    assert_eq!(
        std::fs::read_to_string(&again).unwrap(),
        v4,
        "re-exported as format 2"
    );

    // Refused headers and rows: line 1 (the header) or 2 (the row).
    let mut six = v4_lines.clone();
    six[0] = six[0].replacen("\"runs\":0,", "", 1);
    let mut schema_3 = v4_lines.clone();
    schema_3[0] = schema_3[0].replacen("\"queue_schema\":5,", "\"queue_schema\":3,", 1);
    let mut seven = v4_lines.clone();
    seven[0] = seven[0].replacen("\"format\":2,", "\"format\":1,", 1);
    let mut v3_row_4 = v3_lines.clone();
    v3_row_4[1] = v4_lines[1].clone();
    for (name, lines, line) in [
        ("six-keys", six, 1),
        ("format-2-schema-3", schema_3, 1),
        ("format-1-seven-keys", seven, 1),
        ("format-1-row-with-task-id", v3_row_4, 2),
    ] {
        let file = write(&format!("{name}.jsonl"), &lines);
        let home = pair.scratch.home(name);
        let (outcome, questions) = import(&pair, &home, &file);
        let message = cannot(&outcome, name);
        assert!(
            message.contains(&format!("{}:{line}: ", file.display())),
            "{name}: {message}"
        );
        assert!(questions.is_empty(), "{name}");
        assert!(!common::data_dir(&home).exists(), "{name}: no queue made");
    }

    // A database still at queue schema 3.
    pair.sql(
        "ALTER TABLE proposals DROP COLUMN staged_at; ALTER TABLE proposals DROP COLUMN staged; \
         ALTER TABLE proposals DROP COLUMN task_id; DROP TABLE tasks; DROP TABLE runs; \
         PRAGMA user_version = 3;",
    );
    let from_3 = dumps.join("from-3.jsonl");
    export(&pair, &pair.home, &from_3);
    assert_eq!(std::fs::read_to_string(&from_3).unwrap(), v4);
    assert_eq!(
        pair.sql("PRAGMA user_version;").trim(),
        "3",
        "nothing stepped"
    );

    // Only a task (its event deleted): the queue is occupied.
    let held = pair.scratch.home("held");
    let other = Pair::new("tk-ac11-held", "spec-a");
    let held_pair = HeldHome(&other, &held);
    let made = done(
        task_common::Tasks::new_task(&held_pair, &other.main, &["MEC-STAMINA"], None, None),
        "new",
    );
    assert_eq!(made.id.as_deref(), Some("T-0001"));
    let db = common::data_dir(&held).join("lantern-keep.db");
    let sql = std::process::Command::new(common::proposal::sqlite3())
        .arg(&db)
        .arg("DELETE FROM events;")
        .output()
        .expect("sqlite3");
    assert!(sql.status.success());
    let before = dump_in(&held, "lantern-keep");
    assert!(
        before.starts_with("tasks\t") && !before.contains("events\t"),
        "{before}"
    );
    let (outcome, questions) = import(&other, &held, &current);
    let message = cannot(&outcome, "only a task");
    assert!(
        message.contains("holds 0 proposal(s), 1 task(s), 0 run(s), 0 event(s)"),
        "{message}"
    );
    assert!(questions.is_empty());
    assert_eq!(dump_in(&held, "lantern-keep"), before);
}

/// A pair's commands with another data directory `HOME`.
struct HeldHome<'a>(&'a Pair, &'a Path);

impl task_common::Tasks for HeldHome<'_> {
    fn pair(&self) -> &Pair {
        self.0
    }

    fn new_task_at(
        &self,
        cwd: &Path,
        nodes: &[&str],
        title: Option<&str>,
        goal: Option<&str>,
        now: &str,
    ) -> Result<TaskOutcome, specengine_cli::CliError> {
        specengine_cli::task_new(
            &env_at(self.1, cwd),
            &specengine_cli::Globals::default(),
            &specengine_cli::TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: title.map(str::to_owned),
                goal: goal.map(str::to_owned),
                author_role: None,
                author_model: None,
                run: None,
                now: now.to_owned(),
                git: self.0.git_env(cwd),
            },
        )
    }
}

// ------------------------------------------------------- corrupt rows

/// "Data": a task row whose JSON does not decode or whose state is
/// unknown, or a run with an unknown outcome, is corrupt: `task show`
/// exits 2 naming the task and column; `task list` skips it with a note
/// naming it, the other tasks listed. M: a corrupt row listed.
#[test]
fn a_corrupt_task_or_run_row_is_named_and_skipped() {
    let pair = Pair::new("tk-corrupt", "spec-a");
    let main = pair.main.clone();
    for _ in 0..4 {
        pair.new_ok(&main, &["MEC-STAMINA"]);
    }
    pair.approve_task(&main, "T-0004");
    pair.claim_ok(&main, "T-0004", &main);
    pair.sql(
        "UPDATE tasks SET targets = '[1,' WHERE id = 'T-0001'; \
         UPDATE tasks SET status = 'paused' WHERE id = 'T-0002'; \
         UPDATE runs SET outcome = 'stalled' WHERE task_id = 'T-0004';",
    );
    for (id, column) in [
        ("T-0001", "targets"),
        ("T-0002", "status"),
        ("T-0004", "runs[1].outcome"),
    ] {
        let message = cannot(&pair.show(&main, id), id);
        assert!(
            message.contains(&format!("task {id}: the stored `{column}` cannot be read")),
            "{id}: {message}"
        );
    }
    let listed = pair.list(&main);
    assert_eq!(
        listed
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["T-0003"]
    );
    // The three corrupt rows, then the listed draft's unknown staleness.
    assert_eq!(listed.notes.len(), 4, "{:?}", listed.notes);
    for (note, id) in listed.notes.iter().zip(["T-0001", "T-0002", "T-0004"]) {
        assert!(
            note.starts_with(&format!("task {id}: the stored `")) && note.ends_with("; not listed"),
            "{note}"
        );
    }
    assert_eq!(
        listed.notes[3],
        "T-0003: stale unknown: no snapshot (the owner's `spec task approve` freezes one)"
    );
}

// ----------------------------------------------- AC-09, two projects

/// AC-09 (07 P2-9), the CLI half: two projects (`lantern-keep`, `zerkalo`)
/// in one `HOME` never see each other's tasks — `list` empty and without
/// a note, `show T-0001` "no task" — and deleting one's database leaves
/// the other's task intact; nothing is written under either root. M: no
/// slug filter.
#[test]
fn ac09_two_projects_in_one_home_keep_their_tasks_apart() {
    let a = Pair::new("tk-p29-a", "spec-a");
    let b = Pair::new("tk-p29-b", "spec-b");
    let home = a.home.clone();
    let b_here = HeldHome(&b, &home);
    let made = done(
        task_common::Tasks::new_task(&b_here, &b.main, &["CMD-SYNC"], Some("Sync"), None),
        "new in b",
    );
    assert_eq!(made.id.as_deref(), Some("T-0001"));
    let id = a.new_ok(&a.main, &["MEC-STAMINA"]);
    assert_eq!(id, "T-0001", "each project numbers its own");
    // Approved: its staleness known, so `list` has no note of its own.
    a.approve_task(&a.main, &id);
    let b_show = specengine_cli::task_show(
        &env_at(&home, &b.main),
        &specengine_cli::Globals::default(),
        &specengine_cli::TaskShowRequest {
            id: Some("T-0002".to_owned()),
            next: false,
            git: b.git_env(&b.main),
        },
    )
    .expect("show");
    assert_eq!(
        b_show.reason.as_deref(),
        Some("no task T-0002 in this repository")
    );
    let a_list = a.list(&a.main);
    assert_eq!(a_list.tasks.len(), 1);
    assert!(a_list.notes.is_empty(), "{:?}", a_list.notes);
    assert_eq!(
        a.package(&a.main, "T-0001")["project"],
        json!("lantern-keep")
    );
    let b_package = specengine_cli::task_show(
        &env_at(&home, &b.main),
        &specengine_cli::Globals::default(),
        &specengine_cli::TaskShowRequest {
            id: Some("T-0001".to_owned()),
            next: false,
            git: b.git_env(&b.main),
        },
    )
    .expect("show b")
    .package
    .expect("b's package");
    assert_eq!(b_package.project, "zerkalo");
    assert_eq!(b_package.title.as_deref(), Some("Sync"));
    // Deleting a's database leaves b's task.
    let a_db = common::data_dir(&home).join("lantern-keep.db");
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", a_db.display()));
    }
    assert!(dump_in(&home, "zerkalo").contains("\"T-0001\",\"zerkalo\""));
    assert!(a.list(&a.main).tasks.is_empty(), "a's queue is new");
    for dir in [&a.main, &a.linked, &b.main, &b.linked] {
        assert_eq!(a.porcelain(dir), "", "{}", dir.display());
    }
}

// ------------------------------------------------ list, plan, the text

/// "Description and interactions": `spec task list` prints a line per
/// task by number (`<id> | <status> | <title or -> | <targets> |
/// <updated_at>[ | stale]`), `--status` (repeatable) keeps those states,
/// `--json` is `{tasks: [{id, status, title, targets, stale,
/// updated_at}], notes}`; `spec task plan --plan-file -` reads the plan
/// from stdin; agent-written text reaches the terminal escaped, the JSON
/// verbatim; `spec task new` prints `created T-0001`.
#[test]
fn list_lines_the_status_filter_a_piped_plan_and_escaped_text() {
    let pair = Pair::new("tk-list", "spec-a");
    let main = pair.main.clone();
    let run = pair.spec_piped(
        &main,
        &[
            "task",
            "new",
            "--nodes",
            "MEC-STAMINA",
            "RULE-STAM-REGEN",
            "--title",
            "Red \u{1b}[31mtitle",
        ],
        b"",
    );
    run.code(0);
    assert_eq!(run.stdout, "created T-0001\n");
    let run = pair.spec_piped(&main, &["task", "new", "--nodes", "EDGE-STAM-ZERO"], b"");
    run.code(0);
    assert_eq!(run.stdout, "created T-0002\n");
    let run = pair.spec_piped(
        &main,
        &[
            "task",
            "plan",
            "T-0002",
            "--plan-file",
            "-",
            "--criterion",
            "Holds.",
        ],
        b"1. From stdin.\n",
    );
    run.code(0);
    assert_eq!(run.stdout, "planned T-0002: review\n");
    assert_eq!(
        pair.package(&main, "T-0002")["plan"],
        json!("1. From stdin.\n")
    );
    pair.approve_task(&main, "T-0001");

    let run = pair.spec_piped(&main, &["task", "list"], b"");
    run.code(0);
    let updated_1 = pair.package(&main, "T-0001")["updated_at"].clone();
    let updated_2 = pair.package(&main, "T-0002")["updated_at"].clone();
    assert_eq!(
        run.stdout,
        format!(
            "T-0001 | ready | Red \\u{{1b}}[31mtitle | MEC-STAMINA, RULE-STAM-REGEN | {}\n\
             T-0002 | review | - | EDGE-STAM-ZERO | {}\n",
            updated_1.as_str().unwrap(),
            updated_2.as_str().unwrap()
        )
    );
    assert!(!run.stdout.contains('\u{1b}'), "no raw escape");
    let run = pair.spec_piped(
        &main,
        &[
            "--json", "task", "list", "--status", "review", "--status", "done",
        ],
        b"",
    );
    run.code(0);
    assert_eq!(
        task_common::keys(&run.stdout),
        ["tasks", "notes"],
        "{}",
        run.stdout
    );
    let listed = task_common::Ordered::of(&run.stdout);
    assert_eq!(
        listed.get("tasks").at(0).keys(),
        ["id", "status", "title", "targets", "stale", "updated_at"]
    );
    let value = run.json();
    assert_eq!(value["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(value["tasks"][0]["id"], json!("T-0002"));
    assert_eq!(value["tasks"][0]["stale"], Value::Null);
    assert_eq!(
        value["notes"],
        json!(["T-0002: stale unknown: no snapshot (the owner's `spec task approve` freezes one)"])
    );
    // The brief escapes, the package keeps the title verbatim.
    let brief = pair.brief(&main, "T-0001");
    assert!(
        brief.starts_with("T-0001 | ready | Red \\u{1b}[31mtitle\n"),
        "{brief}"
    );
    assert!(!brief.contains('\u{1b}'));
    assert_eq!(
        pair.package(&main, "T-0001")["title"],
        json!("Red \u{1b}[31mtitle")
    );
    let run = pair.spec_piped(&main, &["task", "list", "--status", "paused"], b"");
    run.code(2);
    assert_eq!(pair.porcelain(&main), "");
}

/// `T` in `[ids]`: the owner's three (through the library) and a
/// proposal's `--task` exit 2 naming the config, nothing changed.
#[test]
fn t_in_ids_stops_the_owner_commands_and_bound_proposals() {
    let pair = Pair::new("tk-t-ids", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    let config = common::read_text(&main, "specengine.toml");
    common::write(
        &main,
        "specengine.toml",
        common::with_ids_line(&config, "T = { kind = \"ticket\", width = 4 }"),
    );
    let before = pair.dump();
    for action in ["approve", "changes", "cancel"] {
        let (outcome, questions) = pair.owner(&main, action, &id, Some("Note."), true, CLOCK);
        let message = cannot(&outcome, action);
        assert!(message.contains("specengine.toml"), "{action}: {message}");
        assert!(questions.is_empty(), "{action}");
    }
    let (hash, text) = pair.span(&main, "EDGE-SPRINT-EMPTY");
    let request = pair.request(
        &main,
        "EDGE-SPRINT-EMPTY",
        &hash,
        &common::proposal::edit(&text, "the sprint ends;", "the sprint stops;"),
    );
    let outcome = specengine_cli::propose_with_task(
        &pair.env(&main),
        &specengine_cli::Globals::default(),
        &request,
        Some(&id),
    );
    let message = cannot(&outcome, "--task");
    assert!(message.contains("specengine.toml"), "{message}");
    assert_eq!(pair.dump(), before);
}

// ------------------------------------------------------- iteration 2

/// Iteration 2, m3: a claimed task's `report` and `complete` run only in
/// its claimed worktree (its git top, a subdirectory too): from another
/// worktree exit 1 naming the claim, its branch and the action, `dump()`
/// unchanged. M: the place check removed.
#[test]
fn m3_report_and_complete_run_only_in_the_claimed_worktree() {
    let pair = Pair::new("tk-m3", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    pair.approve_task(&main, &id);
    pair.claim_ok(&main, &id, &pair.linked);
    let refused = |action: &str| {
        format!(
            "`T-0001` is claimed in the worktree {} on `t1`: `{action}` runs there, not in {}; \
             nothing recorded",
            pair.linked.display(),
            main.display()
        )
    };
    let before = pair.dump();
    let outcome = pair.report(&main, &id, "completed", "Done.", &[]);
    assert_eq!(refusal(&outcome, "report from main"), refused("report"));
    assert_eq!(pair.dump(), before);
    let deeper = pair.linked.join("docs/spec");
    pair.report_ok(&deeper, &id);
    let before = pair.dump();
    let outcome = pair.complete(&main, &id);
    assert_eq!(refusal(&outcome, "complete from main"), refused("complete"));
    assert_eq!(pair.dump(), before);
    let outcome = done(pair.complete(&pair.linked, &id), "complete in t1");
    assert_eq!(line(&outcome), "completed T-0001: done\n");
    assert_eq!(pair.porcelain(&main), "");
    assert_eq!(pair.porcelain(&pair.linked), "");
}

/// Iteration 2, n1: cancelling an `in_progress` task ends its open run in
/// the same transaction (`ended_at` the cancel's time, `outcome` and
/// `summary` `null`); the brief's run line shows it ended. M: the run left
/// open.
#[test]
fn n1_cancel_closes_the_open_run() {
    let pair = Pair::new("tk-n1", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    pair.approve_task(&main, &id);
    pair.claim_ok(&main, &id, &pair.linked);
    let later = "2026-10-07T11:30:00Z";
    done(
        pair.owner(&main, "cancel", &id, None, true, later).0,
        "cancel",
    );
    let package = pair.package(&main, &id);
    assert_eq!(package["status"], json!("cancelled"));
    assert_eq!(
        package["runs"],
        json!([{"run": 1, "role": "developer", "started_at": CLOCK, "ended_at": later,
                "outcome": null, "summary": null, "changed_files": []}])
    );
    let brief = pair.brief(&main, &id);
    assert!(
        brief.contains(&format!(
            "\n  run 1 | developer | {CLOCK} | {later} | - | -\n"
        )),
        "{brief}"
    );
    let events = events_of(&pair, &id);
    assert_eq!(
        events.last().map(|(kind, _)| kind.as_str()),
        Some("task.cancelled")
    );
}

/// The dump's text with every path through an alias of the scratch
/// directory: the system's `/tmp` when its canonical form (another
/// directory on macOS) holds the scratch, else a symlink `alias` to the
/// scratch.
fn aliased(pair: &Pair, text: &str) -> String {
    let real = pair.scratch.path().display().to_string();
    let tmp = Path::new("/tmp");
    let canonical_tmp = std::fs::canonicalize(tmp).ok();
    let through_tmp = canonical_tmp
        .filter(|canonical| canonical.as_path() != tmp)
        .and_then(|canonical| {
            pair.scratch
                .path()
                .strip_prefix(&canonical)
                .ok()
                .map(|rest| tmp.join(rest).display().to_string())
        });
    let alias = through_tmp.unwrap_or_else(|| {
        let link = pair.scratch.join("alias");
        std::os::unix::fs::symlink(pair.scratch.path(), &link).expect("a symlink");
        link.display().to_string()
    });
    assert_ne!(alias, real);
    assert!(
        std::fs::canonicalize(&alias).unwrap() == pair.scratch.path(),
        "{alias} names {real}"
    );
    text.replace(&format!("{real}/"), &format!("{alias}/"))
}

/// Iteration 2, n2: repositories and worktrees compare as directories. A
/// queue restored from a dump whose paths go through an alias of the
/// scratch (`/tmp` for its canonical form) still takes, from the claimed
/// worktree as git names it, a bound question, `report` and `complete`;
/// `task show` reads it as this repository's. M: paths compared as
/// strings.
#[test]
fn n2_paths_compare_as_directories() {
    let pair = Pair::new("tk-n2", "spec-a");
    let main = pair.main.clone();
    let id = pair.new_ok(&main, &["MEC-STAMINA"]);
    pair.approve_task(&main, &id);
    pair.claim_ok(&main, &id, &pair.linked);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    export(&pair, &pair.home, &file);
    let text = std::fs::read_to_string(&file).unwrap();
    let rewritten = aliased(&pair, &text);
    assert_ne!(rewritten, text);
    std::fs::write(&file, &rewritten).unwrap();
    let fresh = pair.scratch.home("fresh");
    let (outcome, _) = import(&pair, &fresh, &file);
    assert_eq!(outcome.expect("import").exit(), Exit::Answered);
    let here = RestoredHome(&pair, &fresh);
    let stored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .unwrap()
    .get_task(&id)
    .unwrap()
    .unwrap();
    assert_ne!(
        stored.git_common_dir,
        format!("{}/.git", main.display()),
        "stored through the alias"
    );
    assert_ne!(
        stored.claim.as_ref().unwrap().worktree,
        pair.linked.display().to_string(),
        "claimed through the alias"
    );
    let shown = here.show_ok(&pair.linked, &id);
    assert_eq!(shown.package.unwrap().status.as_str(), "in_progress");

    let ask = specengine_cli::propose_question_with_task(
        &env_at(&fresh, &pair.linked),
        &specengine_cli::Globals::default(),
        &specengine_cli::QuestionRequest {
            node_ids: vec!["MEC-STAMINA".to_owned()],
            text: "Is it per second?".to_owned(),
            working_answer: "Yes.".to_owned(),
            price_of_other: "A new case.".to_owned(),
            severity: None,
            distinct_from: vec!["DEC-0023".to_owned()],
            author_role: Some("developer".to_owned()),
            author_model: None,
            run: None,
            now: common::proposal::NOW.to_owned(),
            git: pair.git_env(&pair.linked),
        },
        Some(&id),
    )
    .expect("ask");
    assert_eq!(ask.exit(), Exit::Answered, "{ask:?}");
    assert!(ask.document.created, "{ask:?}");
    let report = specengine_cli::task_report(
        &env_at(&fresh, &pair.linked),
        &specengine_cli::Globals::default(),
        &specengine_cli::TaskReportRequest {
            id: id.clone(),
            outcome: "completed".to_owned(),
            summary: "Done.".to_owned(),
            changed_files: Vec::new(),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.linked),
        },
    );
    done(report, "report through the alias");
    let complete = specengine_cli::task_complete(
        &env_at(&fresh, &pair.linked),
        &specengine_cli::Globals::default(),
        &specengine_cli::TaskCompleteRequest {
            id: id.clone(),
            now: CLOCK.to_owned(),
            git: pair.git_env(&pair.linked),
        },
    );
    assert_eq!(done(complete, "complete").status.unwrap().as_str(), "done");
}

/// Iteration 2: the task's `revision` (its compare-and-set key, the last
/// column) is in the dump as text, raised by every change: `T-0001` of
/// [`tasks_queue`] at 5 (new, approve, claim, report, complete), `T-0005`
/// (new only) at 1; restored as dumped.
#[test]
fn the_revision_is_dumped_and_restored() {
    let pair = Pair::new("tk-revision", "spec-a");
    tasks_queue(&pair);
    let dumps = pair.scratch.dir("dumps");
    let file = dumps.join("q.jsonl");
    export(&pair, &pair.home, &file);
    let text = std::fs::read_to_string(&file).unwrap();
    let revisions: Vec<(String, String)> = text
        .lines()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            // The header's `tasks` is a count; a row's an object.
            let task = value.get("tasks").filter(|task| task.is_object())?;
            Some((
                task["id"].as_str().unwrap().to_owned(),
                task["revision"]
                    .as_str()
                    .expect("revision is text")
                    .to_owned(),
            ))
        })
        .collect();
    assert_eq!(
        revisions,
        [
            ("T-0001".to_owned(), "5".to_owned()),
            ("T-0002".to_owned(), "3".to_owned()),
            ("T-0003".to_owned(), "3".to_owned()),
            ("T-0004".to_owned(), "2".to_owned()),
            ("T-0005".to_owned(), "1".to_owned())
        ]
    );
    for line in text.lines().filter(|line| line.starts_with("{\"tasks\":")) {
        assert_eq!(
            task_common::Ordered::of(line)
                .get("tasks")
                .keys()
                .last()
                .map(String::as_str),
            Some("revision"),
            "{line}"
        );
    }
    let fresh = pair.scratch.home("fresh");
    let (outcome, _) = import(&pair, &fresh, &file);
    assert_eq!(outcome.expect("import").exit(), Exit::Answered);
    let restored = specengine_store::SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .unwrap();
    assert_eq!(restored.get_task("T-0001").unwrap().unwrap().revision, 5);
    assert_eq!(restored.get_task("T-0005").unwrap().unwrap().revision, 1);
}
