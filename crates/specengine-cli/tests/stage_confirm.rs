//! docs/features/decision-staging.md, the CLI's half ("Terminal",
//! "Staleness", "Documents", "Backup"; `docs/canon/decision-staging.md`
//! "Terminal"): a stage made through the CLI library's `stage` (the
//! daemon's call, an injected clock), confirmed by `spec approve|reject`
//! through the library with a consent callback, or through the binary on a
//! pseudo-terminal where the terminal is the subject.
//!
//! - AC-07: `{option: 1, note: "n"}` staged on a discrepancy: `spec approve
//!   PR` on a terminal prints `staged <at>:`, the staged command, the
//!   option's line and the question marked `staged`; `n` -> exit 1, the
//!   stage kept; `y` -> the record of option 1, note `n`, the stage NULL,
//!   `.approved` with `staged_at`, no `.unstaged`; no terminal -> exit 2,
//!   unread. M: the stage not read.
//! - AC-08: option 1 staged, typed `--option 2`: the unused-stage note,
//!   option 2, an empty note; typed `--note` alone on an update, a reject
//!   staged under `spec approve`: left unused, named. M: the staged note
//!   merged.
//! - AC-09: a reject staged: `spec reject PR` (library and terminal)
//!   stores its reason; nothing staged: exit 2 naming `--reason`; an
//!   approve staged: exit 2 naming `spec approve`. M: an empty reason
//!   stored.
//! - AC-10: the stage replaced or removed between the question and `y`, in
//!   the same second, at step 7 (an update, a record), a reject or a
//!   completion: exit 1 `changed since the question`, no commit, still
//!   `open`. M: compare-and-set on status and `updated_at` only.
//! - AC-11: staged at `H`, the target changed after: the note in review
//!   and at the prompt, applied as it rebases; changed only before: no
//!   note; a file-form create's `span_hash` null, a section-form create's
//!   the target's. M: the note from `preview != applies`.
//! - AC-16: an `open` update staged, completed by its own commit: the
//!   question ends ` (staged)? [y/N]`; `y` -> the staged note recorded,
//!   `.approved` with `staged_at`. M: `applied_with` given no flags.
//! - AC-02, the CLI's half: a queue at schema 6: exit 2 naming 6 and 5.
//! - AC-12, the CLI's half: review and inbox show the stage (text and
//!   JSON); export, import, export byte-identical with stages; a format-2
//!   schema-4 dump restores, stages NULL; format 1 at schema 4 and format
//!   2 at schema 3 refused naming what each holds; a schema-4 database
//!   exports as 5 with its task, unstepped.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::Path;

use common::decision::{CLOCK, THREE, discrepancy, flags, option};
use common::proposal::{
    CASES, LATER, Pair, cannot, edit, json_of, printed, printed_inbox, refused,
};
use common::{read_text, replace};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CreateRequest, Exit, ExportStateRequest, Globals,
    ImportStateRequest, Message, Preview, ProposalOutcome, ProposedText, RejectRequest, StageBody,
    StageOutcome, StageRequest, approve_with, export_state, import_state, propose_create, reject,
};
use specengine_store::{
    NewTask, ProposalQueue as _, ProposalStatus, SqliteQueue, Stage, StagedChoice,
};

/// The stage's injected clock (after the proposals' `NOW`).
const STAGED: &str = "2026-10-05T21:30:00Z";
const SUMMARY: &str = "Regeneration starts while sprinting in the code";

fn approve_body(
    option: Option<u64>,
    answer: Option<&str>,
    note: Option<&str>,
) -> impl FnOnce(String) -> StageBody {
    let answer = answer.map(str::to_owned);
    let note = note.map(str::to_owned);
    move |updated_at| StageBody::Approve {
        option,
        answer,
        canon: None,
        note,
        updated_at,
    }
}

fn reject_body(reason: &str) -> impl FnOnce(String) -> StageBody {
    let reason = reason.to_owned();
    move |updated_at| StageBody::Reject { reason, updated_at }
}

/// The library's `stage` of `id` from `cwd` at `now`, against its stored
/// `updated_at`; it must be stored.
fn stage(
    pair: &Pair,
    cwd: &Path,
    id: &str,
    body: impl FnOnce(String) -> StageBody,
    now: &str,
) -> StageOutcome {
    let updated_at = pair.proposal(id).updated_at;
    let outcome = specengine_cli::stage(
        &pair.env(cwd),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body: body(updated_at),
            now: now.to_owned(),
            git: pair.git_env(cwd),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"));
    assert_eq!(outcome.cause, None, "stage {id}: {outcome:?}");
    assert_eq!(outcome.exit(), Exit::Answered);
    outcome
}

/// `staged|staged_at` of `id` as stored (`-` for `NULL`).
fn stage_columns(pair: &Pair, id: &str) -> String {
    pair.sql(&format!(
        "SELECT coalesce(staged, '-') || '|' || coalesce(staged_at, '-') FROM proposals \
         WHERE id = '{id}';"
    ))
    .trim()
    .to_owned()
}

/// The payload of `id`'s last event of `kind`.
fn last_event(pair: &Pair, id: &str, kind: &str) -> Value {
    pair.events()
        .into_iter()
        .rev()
        .find(|event| event.event_type == kind && event.payload["id"] == id)
        .unwrap_or_else(|| panic!("no {kind} of {id}"))
        .payload
}

fn kinds_of(pair: &Pair, id: &str) -> Vec<String> {
    pair.events_of(id)
        .into_iter()
        .map(|(kind, _)| kind)
        .collect()
}

/// AC-01's discrepancy of decision-apply on `RULE-STAM-REGEN`, three
/// options, raised in `t1`.
fn item(pair: &Pair) -> String {
    pair.raise_discrepancy(
        &pair.linked,
        discrepancy(&["RULE-STAM-REGEN"], SUMMARY, &THREE),
    )
}

/// Library approve from the main worktree at `LATER`, consent `answer`,
/// `note` and `flags` typed: the outcome and the questions.
fn approve(
    pair: &Pair,
    id: &str,
    flags: &ApproveFlags,
    note: Option<&str>,
    consent: &mut dyn FnMut(&str) -> bool,
) -> Result<ProposalOutcome, specengine_cli::CliError> {
    approve_with(
        &pair.env(&pair.main),
        &Globals::default(),
        &ApproveRequest {
            id: id.to_owned(),
            note: note.map(str::to_owned),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        flags,
        consent,
    )
}

fn reject_with(
    pair: &Pair,
    id: &str,
    reason: Option<&str>,
    consent: &mut dyn FnMut(&str) -> bool,
) -> Result<ProposalOutcome, specengine_cli::CliError> {
    reject(
        &pair.env(&pair.main),
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: reason.map(str::to_owned),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
        consent,
    )
}

fn unused_note(at: &str) -> String {
    format!("the choice staged {at} is not used: the typed flags decide")
}

fn has_note(outcome: &ProposalOutcome, text: &str) -> bool {
    outcome
        .messages
        .iter()
        .any(|message| matches!(message, Message::Note(note) if note == text))
}

// ------------------------------------------------------------------ AC-07

#[test]
fn ac07_a_staged_approve_is_shown_and_confirmed_on_a_terminal() {
    let pair = Pair::new("ds-ac07", "spec-a");
    let id = item(&pair);
    assert_eq!(id, "PR-0001");
    let staged = stage(
        &pair,
        &pair.main,
        &id,
        approve_body(Some(1), None, Some("n")),
        STAGED,
    );
    assert_eq!(
        staged.proposal.document.staged,
        Some(Stage::Approve {
            option: Some(1),
            answer: None,
            canon: None,
            note: Some("n".to_owned()),
            span_hash: None,
        })
    );
    let stored = "{\"decision\":\"approve\",\"option\":1,\"answer\":null,\"canon\":null,\
                  \"note\":\"n\",\"span_hash\":null}";
    assert_eq!(stage_columns(&pair, &id), format!("{stored}|{STAGED}"));
    let base = pair.rev(&pair.linked, "t1");

    // No terminal: exit 2 before reading, nothing changed.
    let events = pair.events();
    let run = pair.spec_piped(&pair.main, &["approve", &id], b"y\n");
    run.code(2);
    assert!(!run.stderr.contains("[y/N]"), "{}", run.show());
    assert_eq!(stage_columns(&pair, &id), format!("{stored}|{STAGED}"));
    assert_eq!(pair.events(), events);

    // `n`: exit 1, the stage kept.
    let block = format!(
        "staged {STAGED}: spec approve {id} --option 1 --note \"n\"\n  [1] Change the spec | \
         Regeneration also runs while sprinting | a new rule case and a test\napply {id} as \
         DEC-0024 (option 1, staged) on t1 in {}? [y/N]",
        pair.linked.display()
    );
    let run = pair.spec_pty(&pair.main, &["approve", &id], Some("n"));
    assert_eq!(run.code, 1, "{}", run.output);
    assert!(run.asked, "{}", run.output);
    assert!(run.output.contains(&block), "{block}\n---\n{}", run.output);
    assert!(
        run.output
            .contains("record DEC-0024 at docs/records/DEC/DEC-0024.md:\n"),
        "the record shown first: {}",
        run.output
    );
    assert_eq!(
        stage_columns(&pair, &id),
        format!("{stored}|{STAGED}"),
        "kept"
    );
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    assert_eq!(pair.rev(&pair.linked, "t1"), base, "no commit");

    // `y`: the staged flags decide.
    let run = pair.spec_pty(&pair.main, &["approve", &id], Some("y"));
    assert_eq!(run.code, 0, "{}", run.output);
    assert!(run.output.contains(&block), "{}", run.output);
    let decided = pair.proposal(&id);
    assert_eq!(decided.status, ProposalStatus::Applied);
    assert_eq!(decided.decision_note.as_deref(), Some("n"));
    let record = decided.record.expect("the record");
    assert_eq!(record.id, "DEC-0024");
    assert_eq!(record.choice, specengine_store::Choice::Option(1));
    assert_eq!(decided.staged, None);
    assert_eq!(stage_columns(&pair, &id), "-|-", "both NULL");
    let approved = last_event(&pair, &id, "proposal.approved");
    assert_eq!(approved["staged_at"], json!(STAGED), "{approved}");
    assert!(
        !kinds_of(&pair, &id).contains(&"proposal.unstaged".to_owned()),
        "leaving open logs no unstage"
    );
    let text = read_text(&pair.linked, "docs/records/DEC/DEC-0024.md");
    assert!(text.contains("Change the spec"), "{text}");
}

// ------------------------------------------------------------------ AC-08

#[test]
fn ac08_typed_flags_win_whole_and_the_unused_stage_is_named() {
    let pair = Pair::new("ds-ac08", "spec-a");
    let id = item(&pair);
    stage(
        &pair,
        &pair.main,
        &id,
        approve_body(Some(1), None, Some("staged note")),
        STAGED,
    );
    let mut questions = Vec::new();
    let outcome = approve(&pair, &id, &option(2), None, &mut |question: &str| {
        questions.push(question.to_owned());
        true
    })
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        has_note(&outcome, &unused_note(STAGED)),
        "{:?}",
        outcome.messages
    );
    assert_eq!(questions.len(), 1);
    assert!(
        questions[0].contains(&format!("note: {}\n", unused_note(STAGED))),
        "{}",
        questions[0]
    );
    assert!(
        !questions[0].contains(&format!("staged {STAGED}: spec approve")),
        "the unused stage is not shown as the choice: {}",
        questions[0]
    );
    assert!(
        questions[0].ends_with(&format!(
            "apply {id} as DEC-0024 (option 2) on t1 in {}? [y/N]",
            pair.linked.display()
        )),
        "{}",
        questions[0]
    );
    let decided = pair.proposal(&id);
    assert_eq!(
        decided.record.expect("record").choice,
        specengine_store::Choice::Option(2)
    );
    assert_eq!(decided.decision_note, None, "the staged note not merged");
    assert_eq!(stage_columns(&pair, &id), "-|-");
    let approved = last_event(&pair, &id, "proposal.approved");
    assert!(approved.get("staged_at").is_none(), "{approved}");

    // An update: a typed `--note` alone wins over a staged approve; a
    // reject staged under `spec approve` is left unused too.
    let case = &CASES[0];
    let first = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    stage(
        &pair,
        &pair.main,
        &first,
        approve_body(None, None, Some("staged")),
        STAGED,
    );
    let outcome = approve(
        &pair,
        &first,
        &ApproveFlags::default(),
        Some("typed"),
        &mut |_| true,
    )
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        has_note(&outcome, &unused_note(STAGED)),
        "{:?}",
        outcome.messages
    );
    let applied = pair.proposal(&first);
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(applied.decision_note.as_deref(), Some("typed"));
    assert!(
        last_event(&pair, &first, "proposal.approved")
            .get("staged_at")
            .is_none()
    );

    let second = pair.propose_edit(&pair.linked, case.target, case.second.0, case.second.1);
    stage(
        &pair,
        &pair.main,
        &second,
        reject_body("not wanted"),
        STAGED,
    );
    let mut asked = Vec::new();
    let outcome = approve(
        &pair,
        &second,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            asked.push(question.to_owned());
            true
        },
    )
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        has_note(&outcome, &unused_note(STAGED)),
        "{:?}",
        outcome.messages
    );
    assert!(!asked[0].contains(", staged)"), "{}", asked[0]);
    let applied = pair.proposal(&second);
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(applied.decision_note, None);
    assert!(
        last_event(&pair, &second, "proposal.approved")
            .get("staged_at")
            .is_none()
    );
}

// ------------------------------------------------------------------ AC-09

#[test]
fn ac09_a_staged_reject_gives_spec_reject_its_reason() {
    let pair = Pair::new("ds-ac09", "spec-a");
    let case = &CASES[0];
    let first = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let second = pair.propose_edit(&pair.linked, case.target, case.second.0, case.second.1);
    let third = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", "immediately", "at once");
    let reason = "duplicate of \"PR-0002\" \\ see";
    stage(&pair, &pair.main, &first, reject_body(reason), STAGED);

    // Nothing staged: exit 2 naming `--reason`; an approve staged: exit 2
    // naming `spec approve`.
    let message = cannot(
        &reject_with(&pair, &second, None, &mut |_| true),
        "nothing staged",
    );
    assert!(
        message.contains(&format!("`spec reject {second}` needs `--reason T`")),
        "{message}"
    );
    stage(
        &pair,
        &pair.main,
        &third,
        approve_body(None, None, None),
        STAGED,
    );
    let message = cannot(
        &reject_with(&pair, &third, None, &mut |_| true),
        "approve staged",
    );
    assert!(
        message.contains(&format!("`spec approve {third}` confirms it")),
        "{message}"
    );
    assert_eq!(pair.proposal(&second).status, ProposalStatus::Open);
    assert!(pair.proposal(&third).staged.is_some(), "kept");

    // The staged reject: shown, marked, its reason stored.
    let mut questions = Vec::new();
    let outcome = reject_with(&pair, &first, None, &mut |question: &str| {
        questions.push(question.to_owned());
        true
    })
    .expect("reject");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(questions.len(), 1);
    let worktree = pair.linked.display().to_string();
    assert_eq!(
        questions[0],
        format!(
            "staged {STAGED}: spec reject {first} --reason \"duplicate of \\\"PR-0002\\\" \\\\ \
             see\"\nreject {first} ({} in {} on t1 in {worktree}, staged)? [y/N]",
            case.target, case.path
        )
    );
    let rejected = pair.proposal(&first);
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.decision_note.as_deref(), Some(reason));
    assert_eq!(stage_columns(&pair, &first), "-|-");
    let event = last_event(&pair, &first, "proposal.rejected");
    assert_eq!(event["staged_at"], json!(STAGED));
    assert_eq!(event["reason"], json!(reason));

    // A typed reason over a staged reject: typed wins, the stage named.
    stage(
        &pair,
        &pair.main,
        &second,
        reject_body("staged reason"),
        STAGED,
    );
    let outcome = reject_with(&pair, &second, Some("typed reason"), &mut |_| true).expect("reject");
    assert!(
        has_note(&outcome, &unused_note(STAGED)),
        "{:?}",
        outcome.messages
    );
    assert_eq!(
        pair.proposal(&second).decision_note.as_deref(),
        Some("typed reason")
    );
    assert!(
        last_event(&pair, &second, "proposal.rejected")
            .get("staged_at")
            .is_none()
    );
}

/// AC-09 through the binary: `spec reject PR` without `--reason` on a
/// terminal, `y`: the staged reason stored; piped: exit 2 unread; with
/// nothing staged: exit 2 naming `--reason`.
#[test]
fn ac09_spec_reject_without_reason_on_a_terminal_takes_the_staged_one() {
    let pair = Pair::new("ds-ac09-tty", "spec-a");
    let case = &CASES[0];
    let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let run = pair.spec_pty(&pair.main, &["reject", &id], None);
    assert_eq!(run.code, 2, "{}", run.output);
    assert!(run.output.contains("needs `--reason T`"), "{}", run.output);
    stage(&pair, &pair.main, &id, reject_body("not now"), STAGED);
    let run = pair.spec_piped(&pair.main, &["reject", &id], b"y\n");
    run.code(2);
    assert!(!run.stderr.contains("[y/N]"), "{}", run.show());
    let run = pair.spec_pty(&pair.main, &["reject", &id], Some("y"));
    assert_eq!(run.code, 0, "{}", run.output);
    assert!(
        run.output.contains(&format!(
            "staged {STAGED}: spec reject {id} --reason \"not now\"\nreject {id} ("
        )),
        "{}",
        run.output
    );
    let rejected = pair.proposal(&id);
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.decision_note.as_deref(), Some("not now"));
}

// ------------------------------------------------------------------ AC-10

/// In the consent callback: `id`'s stage replaced by `stage` (or removed)
/// at [`STAGED`], the same second it was made in: `updated_at` unchanged.
fn swap(pair: &Pair, id: &str, stage: Option<Stage>) {
    let mut queue = SqliteQueue::open(pair.db(), &pair.slug).expect("the queue opens");
    let read = queue.get(id).expect("get").expect("stored");
    let updated_at = read.updated_at.clone();
    match stage {
        Some(stage) => {
            queue
                .stage_from(id, &read.seen(), &stage, STAGED)
                .expect("replaced");
        }
        None => {
            assert!(
                queue
                    .unstage_from(id, &read.seen(), STAGED)
                    .expect("removed")
            );
        }
    }
    let after = queue.get(id).unwrap().unwrap();
    assert_eq!(after.updated_at, updated_at, "the same second");
}

fn changed_since(id: &str) -> String {
    format!(
        "{id} changed since the question: its staged choice was replaced or removed; nothing changed"
    )
}

#[test]
fn ac10_a_stage_replaced_between_the_question_and_y_is_refused() {
    let pair = Pair::new("ds-ac10", "spec-a");
    let case = &CASES[0];
    let base = pair.rev(&pair.linked, "t1");
    // Step 7 of an update: replaced, then removed.
    for replacement in [
        Some(Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            note: Some("second".to_owned()),
            span_hash: None,
        }),
        None,
    ] {
        let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
        stage(
            &pair,
            &pair.main,
            &id,
            approve_body(None, None, Some("first")),
            STAGED,
        );
        let replaced = replacement.clone();
        let outcome = approve(
            &pair,
            &id,
            &ApproveFlags::default(),
            None,
            &mut |question: &str| {
                assert!(question.ends_with(", staged)? [y/N]"), "{question}");
                swap(&pair, &id, replaced.clone());
                true
            },
        );
        let reason = refused(&outcome, "replaced at step 7");
        assert_eq!(
            reason,
            format!("`{id}` not applied (step 7): {}", changed_since(&id)),
            "{replacement:?}"
        );
        assert_eq!(pair.rev(&pair.linked, "t1"), base, "no commit");
        let proposal = pair.proposal(&id);
        assert_eq!(proposal.status, ProposalStatus::Open);
        assert_eq!(
            proposal.staged.map(|staged| staged.stage),
            replacement,
            "the replacement kept"
        );
        // Rejected to leave the next round its own proposal.
        let (outcome, _) = pair.reject_answer(&pair.main, &id, "next round", true);
        assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    }

    // A record's step 7.
    let report = item(&pair);
    stage(
        &pair,
        &pair.main,
        &report,
        approve_body(Some(1), None, None),
        STAGED,
    );
    let outcome = approve(&pair, &report, &ApproveFlags::default(), None, &mut |_| {
        swap(
            &pair,
            &report,
            Some(Stage::Approve {
                option: Some(2),
                answer: None,
                canon: None,
                note: None,
                span_hash: None,
            }),
        );
        true
    });
    let reason = refused(&outcome, "a record");
    assert!(reason.ends_with(&changed_since(&report)), "{reason}");
    assert_eq!(pair.rev(&pair.linked, "t1"), base, "no record committed");
    assert_eq!(pair.proposal(&report).status, ProposalStatus::Open);
    assert!(pair.proposal(&report).record.is_none());

    // A reject.
    let id = pair.propose_edit(&pair.linked, case.target, case.second.0, case.second.1);
    stage(&pair, &pair.main, &id, reject_body("first reason"), STAGED);
    let outcome = reject_with(&pair, &id, None, &mut |_| {
        swap(
            &pair,
            &id,
            Some(Stage::Reject {
                reason: "second reason".to_owned(),
            }),
        );
        true
    });
    let reason = refused(&outcome, "a reject");
    assert!(reason.ends_with(&changed_since(&id)), "{reason}");
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Open);
    assert_eq!(pair.proposal(&id).decision_note, None);
}

#[test]
fn ac10_a_stage_replaced_before_a_completion_records_nothing() {
    let pair = Pair::new("ds-ac10-complete", "spec-a");
    let case = &CASES[0];
    let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let commit = commit_by_hand(&pair, &id);
    stage(
        &pair,
        &pair.main,
        &id,
        approve_body(None, None, Some("by hand")),
        STAGED,
    );
    let outcome = approve(
        &pair,
        &id,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            assert!(question.ends_with(" (staged)? [y/N]"), "{question}");
            swap(
                &pair,
                &id,
                Some(Stage::Approve {
                    option: None,
                    answer: None,
                    canon: None,
                    note: Some("by someone else".to_owned()),
                    span_hash: None,
                }),
            );
            true
        },
    );
    let reason = refused(&outcome, "a completion");
    assert!(reason.ends_with(&changed_since(&id)), "{reason}");
    let proposal = pair.proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Open);
    assert_eq!(proposal.applied_commit, None);
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    assert!(!kinds_of(&pair, &id).contains(&"proposal.applied".to_owned()));
}

// ------------------------------------------------------------------ AC-16

/// The proposal's edit committed by hand on `t1` with its `Proposal:`
/// trailer: the commit.
fn commit_by_hand(pair: &Pair, id: &str) -> String {
    let case = &CASES[0];
    replace(&pair.linked, case.path, case.first.0, case.first.1);
    pair.git.git(
        &pair.linked,
        &[
            "commit",
            "-q",
            "-m",
            "The sprint ends at once.",
            "-m",
            &format!("Proposal: {id}"),
            "--",
            case.path,
        ],
    );
    pair.rev(&pair.linked, "t1")
}

#[test]
fn ac16_a_staged_approve_completed_by_its_own_commit_records_the_staged_flags() {
    let pair = Pair::new("ds-ac16", "spec-a");
    let case = &CASES[0];
    let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let commit = commit_by_hand(&pair, &id);
    stage(
        &pair,
        &pair.main,
        &id,
        approve_body(None, None, Some("by hand")),
        STAGED,
    );
    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &id,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "staged {STAGED}: spec approve {id} --note \"by hand\"\ncomplete {id} by its commit \
             {commit} on t1 in {} (staged)? [y/N]",
            pair.linked.display()
        )]
    );
    let applied = pair.proposal(&id);
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(applied.applied_commit.as_deref(), Some(commit.as_str()));
    assert_eq!(applied.decision_note.as_deref(), Some("by hand"));
    assert_eq!(applied.staged, None);
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");
    let approved = last_event(&pair, &id, "proposal.approved");
    assert_eq!(approved["staged_at"], json!(STAGED), "{approved}");
    assert!(!kinds_of(&pair, &id).contains(&"proposal.unstaged".to_owned()));
}

// ------------------------------------------------------------------ AC-11

fn stale(then: &str, now: &str) -> String {
    format!("staged against {then}; the target is now {now}: the change applies as it rebases")
}

#[test]
fn ac11_a_target_changed_after_the_stage_is_noted_and_applied_as_it_rebases() {
    let pair = Pair::new("ds-ac11", "spec-a");
    let case = &CASES[0];
    let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let (then, _) = pair.span(&pair.linked, case.target);
    let staged = stage(
        &pair,
        &pair.main,
        &id,
        approve_body(None, None, None),
        STAGED,
    );
    assert_eq!(
        staged.proposal.document.staged,
        Some(Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            note: None,
            span_hash: Some(then.clone()),
        }),
        "an update's stage carries its target's span hash"
    );
    // The target changed after the stage, on another line.
    replace(&pair.linked, case.path, case.second.0, case.second.1);
    pair.commit_all(&pair.linked, "Another line.");
    let (now, _) = pair.span(&pair.linked, case.target);
    assert_ne!(then, now);
    let note = stale(&then, &now);
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(review.document.preview, Some(Preview::Rebases));
    assert!(
        review.document.notes.contains(&note),
        "{:?}",
        review.document.notes
    );
    let (text, json) = printed(&review);
    assert!(text.contains(&note), "{text}");
    assert!(
        json_of(&json)["notes"]
            .as_array()
            .unwrap()
            .contains(&json!(note))
    );

    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &id,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "staged {STAGED}: spec approve {id}\nnote: {note}\napply {id} to {} on t1 in {} \
             (rebases, staged)? [y/N]",
            case.path,
            pair.linked.display()
        )]
    );
    assert_eq!(pair.proposal(&id).status, ProposalStatus::Applied);
    let file = read_text(&pair.linked, case.path);
    assert!(
        file.contains(case.first.1) && file.contains(case.second.1),
        "both edits: {file}"
    );
}

#[test]
fn ac11_a_target_changed_only_before_the_stage_gives_no_note() {
    let pair = Pair::new("ds-ac11-before", "spec-a");
    let case = &CASES[0];
    let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    replace(&pair.linked, case.path, case.second.0, case.second.1);
    pair.commit_all(&pair.linked, "Another line.");
    let (now, _) = pair.span(&pair.linked, case.target);
    let staged = stage(
        &pair,
        &pair.main,
        &id,
        approve_body(None, None, None),
        STAGED,
    );
    match &staged.proposal.document.staged {
        Some(Stage::Approve { span_hash, .. }) => {
            assert_eq!(span_hash.as_deref(), Some(now.as_str()))
        }
        other => panic!("{other:?}"),
    }
    let review = pair.review_ok(&pair.main, &id);
    assert_eq!(review.document.preview, Some(Preview::Rebases));
    assert!(
        !review
            .document
            .notes
            .iter()
            .any(|note| note.starts_with("staged against")),
        "{:?}",
        review.document.notes
    );
    let mut questions = Vec::new();
    approve(
        &pair,
        &id,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .expect("approve");
    assert_eq!(
        questions,
        [format!(
            "staged {STAGED}: spec approve {id}\napply {id} to {} on t1 in {} (rebases, \
             staged)? [y/N]",
            case.path,
            pair.linked.display()
        )]
    );
}

#[test]
fn ac11_a_file_form_create_stages_no_span_hash_and_a_section_form_its_targets() {
    let pair = Pair::new("ds-ac11-create", "spec-a");
    let r12 = read_text(&common::fixture("spec-a"), "docs/records/R/R-12.md");
    let t13 = edit(&r12, "id: R-12\n", "id: R-13\n");
    let create = |target: &str, base: Option<&str>, text: &str| {
        let outcome = propose_create(
            &pair.env(&pair.linked),
            &Globals::default(),
            &CreateRequest {
                target: target.to_owned(),
                base: base.map(str::to_owned),
                text: ProposedText::Given(text.as_bytes().to_vec()),
                rationale: "A create.".to_owned(),
                author_role: Some("writer".to_owned()),
                author_model: None,
                run: None,
                now: CLOCK.to_owned(),
                git: pair.git_env(&pair.linked),
            },
        )
        .expect("propose create");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.document.id.expect("an ID")
    };
    let file = create("docs/records/R/R-13.md", None, &t13);
    let (hash, span) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    let sections = create(
        "RULE-STAM-REGEN",
        Some(&hash),
        &format!(
            "{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Regeneration waits 1.5 s after the \
             last sprint."
        ),
    );
    let staged = stage(
        &pair,
        &pair.main,
        &file,
        approve_body(None, None, Some("ok")),
        STAGED,
    );
    assert_eq!(
        staged.proposal.document.staged,
        Some(Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            note: Some("ok".to_owned()),
            span_hash: None,
        })
    );
    let staged = stage(
        &pair,
        &pair.main,
        &sections,
        approve_body(None, None, None),
        STAGED,
    );
    match &staged.proposal.document.staged {
        Some(Stage::Approve { span_hash, .. }) => {
            assert_eq!(span_hash.as_deref(), Some(hash.as_str()));
        }
        other => panic!("{other:?}"),
    }
    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &file,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "staged {STAGED}: spec approve {file} --note \"ok\"\napply {file} to \
             docs/records/R/R-13.md on t1 in {} (new file, staged)? [y/N]",
            pair.linked.display()
        )]
    );
    assert_eq!(pair.proposal(&file).decision_note.as_deref(), Some("ok"));
}

// ------------------------------------------------------------------ AC-02

#[test]
fn ac02_a_queue_of_a_newer_schema_exits_2_naming_both() {
    let pair = Pair::new("ds-ac02", "spec-a");
    let case = &CASES[0];
    pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    pair.sql("PRAGMA user_version = 6;");
    for args in [vec!["inbox"], vec!["review", "PR-0001"]] {
        let run = pair.spec_piped(&pair.main, &args, b"");
        run.code(2);
        assert!(
            run.stderr.contains("schema version 6, this build knows 5"),
            "{args:?}: {}",
            run.show()
        );
    }
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "6", "left alone");
}

// ------------------------------------------------------------------ AC-12

fn export(pair: &Pair, home: &Path, file: &Path) -> Vec<u8> {
    let outcome = export_state(
        &specengine_cli::Env {
            cwd: pair.main.clone(),
            home: Some(home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        &Globals::default(),
        &ExportStateRequest {
            out: Some(file.to_path_buf()),
            now: LATER.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("export: {error}"));
    let _ = outcome;
    fs::read(file).expect("the dump")
}

fn import(
    pair: &Pair,
    home: &Path,
    file: &Path,
) -> Result<specengine_cli::ImportStateOutcome, specengine_cli::CliError> {
    import_state(
        &specengine_cli::Env {
            cwd: pair.main.clone(),
            home: Some(home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        &Globals::default(),
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut |_: &str| true,
    )
}

fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8(bytes.to_vec())
        .expect("UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn ac12_review_and_inbox_show_the_stage_and_backups_keep_it() {
    let pair = Pair::new("ds-ac12", "spec-a");
    let case = &CASES[0];
    let update = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let question = pair.raise_question(&pair.linked, &["EDGE-STAM-ZERO"], "Stop?", "yes", "a rule");
    let report = item(&pair);
    stage(
        &pair,
        &pair.main,
        &update,
        approve_body(None, None, Some("ok \"q\"")),
        STAGED,
    );
    stage(
        &pair,
        &pair.main,
        &question,
        reject_body("asked before"),
        STAGED,
    );
    stage(
        &pair,
        &pair.main,
        &report,
        approve_body(Some(1), None, None),
        STAGED,
    );

    // Review: `staged` (compact JSON) and `staged_at`, text and JSON.
    let review = pair.review_ok(&pair.main, &question);
    let (text, json) = printed(&review);
    assert!(
        text.contains(&format!(
            "\nstaged: {{\"decision\":\"reject\",\"reason\":\"asked before\"}}\nstaged_at: \
             {STAGED}\n"
        )),
        "{text}"
    );
    assert!(
        json.contains(&format!(
            "\"staged\":{{\"decision\":\"reject\",\"reason\":\"asked before\"}},\"staged_at\":\
             \"{STAGED}\",\"notes\":"
        )),
        "{json}"
    );
    // Inbox: `open (staged)`, `staged_at` after `record_id`.
    let (text, json) = printed_inbox(&pair.inbox(&pair.main, false).expect("inbox"));
    for line in text.lines() {
        assert!(line.contains(" | open (staged) | "), "{line}");
    }
    let listed = json_of(&json);
    for entry in listed["proposals"].as_array().unwrap() {
        assert_eq!(entry["staged_at"], json!(STAGED), "{entry}");
        assert!(entry.get("staged").is_none(), "{entry}");
    }
    assert!(
        json.contains(&format!("\"record_id\":null,\"staged_at\":\"{STAGED}\"}}")),
        "{json}"
    );

    // Export, import, export: byte-identical with the three stages.
    let dumps = pair.scratch.dir("dumps");
    let bytes = export(&pair, &pair.home, &dumps.join("q.jsonl"));
    let rows = lines(&bytes);
    assert!(
        rows[0].starts_with("{\"format\":2,\"queue_schema\":5,"),
        "{}",
        rows[0]
    );
    let staged_rows = rows
        .iter()
        .filter(|row| row.contains(&format!("\"staged_at\":\"{STAGED}\"}}}}")))
        .count();
    assert_eq!(staged_rows, 3, "{rows:?}");
    let fresh = pair.scratch.home("fresh");
    let outcome = import(&pair, &fresh, &dumps.join("q.jsonl")).expect("import");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        export(&pair, &fresh, &dumps.join("again.jsonl")),
        bytes,
        "byte-identical"
    );
    let restored = SqliteQueue::open(
        common::data_dir(&fresh).join("lantern-keep.db"),
        "lantern-keep",
    )
    .unwrap();
    assert_eq!(
        restored.get(&question).unwrap().unwrap().staged,
        Some(StagedChoice {
            stage: Stage::Reject {
                reason: "asked before".to_owned()
            },
            at: STAGED.to_owned()
        })
    );
}

#[test]
fn ac12_older_dumps_restore_and_formats_hold_their_schemas() {
    let pair = Pair::new("ds-ac12-old", "spec-a");
    let case = &CASES[0];
    pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let dumps = pair.scratch.dir("dumps");
    let current = export(&pair, &pair.home, &dumps.join("v5.jsonl"));
    let v5 = lines(&current);
    assert_eq!(
        v5[0],
        "{\"format\":2,\"queue_schema\":5,\"project\":\"lantern-keep\",\"proposals\":1,\
         \"tasks\":0,\"runs\":0,\"events\":1}"
    );
    let tail = ",\"staged\":null,\"staged_at\":null}}";
    assert!(v5[1].ends_with(tail), "{}", v5[1]);
    let write = |name: &str, rows: &[String]| {
        let file = dumps.join(name);
        fs::write(&file, format!("{}\n", rows.join("\n"))).unwrap();
        file
    };
    // Format 2, schema 4: its 41 columns; restores, the stage NULL, and
    // re-exports as the current dump.
    let mut v4 = v5.clone();
    v4[0] = v4[0].replacen("\"queue_schema\":5,", "\"queue_schema\":4,", 1);
    v4[1] = format!("{}}}}}", &v4[1][..v4[1].len() - tail.len()]);
    let file = write("v4.jsonl", &v4);
    let home = pair.scratch.home("v4");
    let outcome = import(&pair, &home, &file).expect("a format-2 schema-4 dump");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(export(&pair, &home, &dumps.join("v4-again.jsonl")), current);
    // Format 1 at schema 4, format 2 at schema 3: refused, naming what each
    // format holds.
    let mut v1_at_4 = v4.clone();
    v1_at_4[0] = "{\"format\":1,\"queue_schema\":4,\"project\":\"lantern-keep\",\"proposals\":1,\
                  \"events\":1}"
        .to_owned();
    let mut v2_at_3 = v4.clone();
    v2_at_3[0] = v2_at_3[0].replacen("\"queue_schema\":4,", "\"queue_schema\":3,", 1);
    for (name, rows, held) in [
        (
            "format-1-schema-4",
            v1_at_4,
            "(it holds queue schemas 1 to 3)",
        ),
        (
            "format-2-schema-3",
            v2_at_3,
            "(it holds queue schemas 4 to 5)",
        ),
    ] {
        let file = write(&format!("{name}.jsonl"), &rows);
        let home = pair.scratch.home(name);
        let message = cannot(&import(&pair, &home, &file), name);
        assert!(
            message.contains(&format!("{}:1: ", file.display())) && message.contains(held),
            "{name}: {message}"
        );
        assert!(!common::data_dir(&home).exists(), "{name}: no queue made");
    }

    // A schema-4 database holding a task: exported as 5 with its task,
    // unstepped.
    let mut queue = pair.queue();
    queue
        .create_task(
            &NewTask {
                git_common_dir: pair.proposal("PR-0001").place.git_common_dir.clone(),
                title: Some("Tune the sprint".to_owned()),
                goal: None,
                targets: vec![case.target.to_owned()],
                author: specengine_core::proposal::Author::human(),
            },
            LATER,
        )
        .expect("a task");
    drop(queue);
    pair.sql(
        "ALTER TABLE proposals DROP COLUMN staged_at; ALTER TABLE proposals DROP COLUMN staged; \
         PRAGMA user_version = 4;",
    );
    let bytes = export(&pair, &pair.home, &dumps.join("from-4.jsonl"));
    let rows = lines(&bytes);
    assert!(
        rows[0].starts_with("{\"format\":2,\"queue_schema\":5,\"project\":\"lantern-keep\",\"proposals\":1,\"tasks\":1,"),
        "{}",
        rows[0]
    );
    assert!(rows[1].ends_with(tail), "{}", rows[1]);
    assert!(
        rows.iter().any(|row| row.starts_with("{\"tasks\":")),
        "{rows:?}"
    );
    assert_eq!(
        pair.sql("PRAGMA user_version;").trim(),
        "4",
        "export steps nothing"
    );
}

/// The staged choice's events and states end to end through the library:
/// a stage made, read back by `review`, confirmed: one `proposal.staged`,
/// no `proposal.unstaged`, `.approved` with `staged_at`.
#[test]
fn a_question_staged_with_an_answer_and_canon_is_recorded_with_them() {
    let pair = Pair::new("ds-question", "spec-a");
    let id = pair.raise_question(
        &pair.linked,
        &["EDGE-STAM-ZERO"],
        "Does a sprint end at zero?",
        "yes",
        "a rule",
    );
    let body = |updated_at: String| StageBody::Approve {
        option: None,
        answer: Some("No, it waits.".to_owned()),
        canon: Some("EDGE-STAM-ZERO".to_owned()),
        note: None,
        updated_at,
    };
    stage(&pair, &pair.main, &id, body, STAGED);
    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &id,
        &flags(None, None, None),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            true
        },
    )
    .expect("approve");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(
        questions[0].contains(&format!(
            "staged {STAGED}: spec approve {id} --answer \"No, it waits.\" --canon \
             \"EDGE-STAM-ZERO\"\napply {id} as DEC-0024 ("
        )),
        "{}",
        questions[0]
    );
    assert!(
        questions[0].contains(", staged) on t1 in "),
        "{}",
        questions[0]
    );
    let decided = pair.proposal(&id);
    assert_eq!(
        decided.record.expect("record").choice,
        specengine_store::Choice::Answer("No, it waits.".to_owned())
    );
    assert_eq!(
        kinds_of(&pair, &id),
        [
            "proposal.created",
            "proposal.staged",
            "proposal.approved",
            "proposal.applied"
        ]
    );
}

/// AC-12: a stage on a proposal that is not `open` (written behind the
/// queue's back) is a corrupt row: `spec inbox --all` skips it with a note
/// naming the row and `staged`, `spec review` of it cannot run, naming
/// them; the other rows listed.
#[test]
fn ac12_a_stage_on_another_state_is_a_corrupt_row_named_by_inbox_and_review() {
    let pair = Pair::new("ds-ac12-corrupt", "spec-a");
    let case = &CASES[0];
    let applied = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let open = pair.propose_edit(&pair.linked, "EDGE-STAM-ZERO", "immediately", "at once");
    pair.approve_ok(&pair.main, &applied);
    pair.sql(&format!(
        "UPDATE proposals SET staged = '{{\"decision\":\"reject\",\"reason\":\"late\"}}', \
         staged_at = '{STAGED}' WHERE id = '{applied}';"
    ));
    let inbox = pair.inbox(&pair.main, true).expect("inbox");
    let ids: Vec<&str> = inbox
        .proposals
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, [open.as_str()], "{inbox:?}");
    assert!(
        inbox
            .notes
            .iter()
            .any(|note| note.contains(&format!("proposal {applied}"))
                && note.contains("`staged`")
                && note.ends_with("; not listed")),
        "{:?}",
        inbox.notes
    );
    let message = cannot(&pair.review(&pair.main, &applied), "a corrupt row");
    assert!(
        message.contains(&applied) && message.contains("`staged`"),
        "{message}"
    );
}

// ------------------------------------------- one line whatever a value holds

/// The lines of `question` that start `staged `: exactly one, and the
/// question's own line (starting `starts`) directly after it; that line.
fn one_staged_line(question: &str, starts: &str) -> String {
    let lines: Vec<&str> = question.lines().collect();
    let staged: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("staged "))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(staged.len(), 1, "one `staged` line: {question:?}");
    let at = staged[0];
    assert!(
        lines
            .get(at + 1)
            .is_some_and(|next| next.starts_with(starts)),
        "the question right after the staged line: {question:?}"
    );
    assert_eq!(
        at + 2,
        lines.len(),
        "nothing after the question: {question:?}"
    );
    lines[at].to_owned()
}

/// A staged note, answer or reason holding a line feed, a carriage return
/// (stored behind the CLI's character check: any local process can write
/// the queue) or a tab prints as one `staged …` line, the escapes literal
/// (`\n`, `\r`, `\t`), the question directly after it, through the library
/// and on a terminal. M: `quoted()` without the line-feed case.
#[test]
fn a_staged_value_with_lf_cr_or_tab_is_shown_on_one_line() {
    let pair = Pair::new("ds-lf", "spec-a");
    let case = &CASES[0];
    let worktree = pair.linked.display().to_string();

    // An update's note: LF and TAB, staged through the CLI library.
    let update = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    stage(
        &pair,
        &pair.main,
        &update,
        approve_body(None, None, Some("line one\nline two\tend")),
        STAGED,
    );
    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &update,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            false
        },
    );
    refused(&outcome, "declined");
    assert_eq!(
        questions,
        [format!(
            "staged {STAGED}: spec approve {update} --note \"line one\\nline two\\tend\"\napply \
             {update} to {} on t1 in {worktree} (applies, staged)? [y/N]",
            case.path
        )]
    );
    one_staged_line(&questions[0], &format!("apply {update} to "));
    // On a terminal: the same one line, then the question.
    let run = pair.spec_pty(&pair.main, &["approve", &update], Some("n"));
    assert_eq!(run.code, 1, "{}", run.output);
    assert!(
        run.output.contains(&format!(
            "staged {STAGED}: spec approve {update} --note \"line one\\nline two\\tend\"\napply \
             {update} to "
        )),
        "{}",
        run.output
    );

    // A question's answer: LF and TAB; the record text above, the staged
    // line right before the question.
    let question = pair.raise_question(&pair.linked, &["EDGE-STAM-ZERO"], "Stop?", "yes", "a rule");
    stage(
        &pair,
        &pair.main,
        &question,
        approve_body(None, Some("yes\nreally\tso"), None),
        STAGED,
    );
    let mut asked = Vec::new();
    let outcome = approve(
        &pair,
        &question,
        &ApproveFlags::default(),
        None,
        &mut |text: &str| {
            asked.push(text.to_owned());
            false
        },
    );
    refused(&outcome, "declined");
    let line = one_staged_line(&asked[0], &format!("apply {question} as DEC-0024 ("));
    assert_eq!(
        line,
        format!("staged {STAGED}: spec approve {question} --answer \"yes\\nreally\\tso\"")
    );

    // A reject's reason with CR LF and TAB, written behind the CLI's
    // check (the store takes any text).
    let rejected = pair.propose_edit(&pair.linked, case.target, case.second.0, case.second.1);
    let mut queue = pair.queue();
    let read = queue.get(&rejected).unwrap().unwrap();
    queue
        .stage_from(
            &rejected,
            &read.seen(),
            &Stage::Reject {
                reason: "first\r\nsecond\tthird".to_owned(),
            },
            STAGED,
        )
        .expect("stored");
    drop(queue);
    let mut asked = Vec::new();
    let outcome = reject_with(&pair, &rejected, None, &mut |text: &str| {
        asked.push(text.to_owned());
        false
    });
    refused(&outcome, "declined");
    assert_eq!(
        asked,
        [format!(
            "staged {STAGED}: spec reject {rejected} --reason \"first\\r\\nsecond\\tthird\"\n\
             reject {rejected} ({} in {} on t1 in {worktree}, staged)? [y/N]",
            case.target, case.path
        )]
    );
}

// --------------------------------------------- a completion's compare

/// A completion compares the stage it read, whatever it was: an `open`
/// proposal completed by its own commit with nothing staged, a stage
/// stored during the question → exit 1 `a choice was staged on it
/// meanwhile`, still `open`, no `.approved` or `.applied`, the stage kept;
/// a stage left unused (`--note` typed) and replaced during the question →
/// exit 1 `its staged choice was replaced or removed`. M: the compare only
/// when a stage was shown.
#[test]
fn a_completion_refuses_a_stage_that_changed_during_the_question() {
    let pair = Pair::new("ds-complete-sym", "spec-a");
    let case = &CASES[0];
    let id = pair.propose_edit(&pair.linked, case.target, case.first.0, case.first.1);
    let commit = commit_by_hand(&pair, &id);
    let staged_meanwhile = Stage::Approve {
        option: None,
        answer: None,
        canon: None,
        note: Some("from a page".to_owned()),
        span_hash: None,
    };
    let stored = staged_meanwhile.clone();
    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &id,
        &ApproveFlags::default(),
        None,
        &mut |question: &str| {
            questions.push(question.to_owned());
            let mut queue = pair.queue();
            let read = queue.get(&id).unwrap().unwrap();
            queue
                .stage_from(&id, &read.seen(), &stored, STAGED)
                .expect("staged meanwhile");
            true
        },
    );
    assert_eq!(
        questions,
        [format!(
            "complete {id} by its commit {commit} on t1 in {}? [y/N]",
            pair.linked.display()
        )]
    );
    let reason = refused(&outcome, "staged meanwhile");
    assert!(
        reason.ends_with(&format!(
            "{id} changed since the question: a choice was staged on it meanwhile; nothing changed"
        )),
        "{reason}"
    );
    let proposal = pair.proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Open);
    assert_eq!(proposal.applied_commit, None);
    assert_eq!(
        proposal.staged.map(|staged| staged.stage),
        Some(staged_meanwhile),
        "the stage kept"
    );
    let kinds = kinds_of(&pair, &id);
    assert!(
        !kinds.contains(&"proposal.approved".to_owned())
            && !kinds.contains(&"proposal.applied".to_owned()),
        "{kinds:?}"
    );
    assert_eq!(pair.rev(&pair.linked, "t1"), commit, "no new commit");

    // A stage left unused (`--note` typed), replaced during the question.
    let replacement = Stage::Reject {
        reason: "changed my mind".to_owned(),
    };
    let stored = replacement.clone();
    let mut questions = Vec::new();
    let outcome = approve(
        &pair,
        &id,
        &ApproveFlags::default(),
        Some("typed"),
        &mut |question: &str| {
            questions.push(question.to_owned());
            let mut queue = pair.queue();
            let read = queue.get(&id).unwrap().unwrap();
            queue
                .stage_from(&id, &read.seen(), &stored, STAGED)
                .expect("replaced");
            true
        },
    );
    assert_eq!(questions.len(), 1, "{questions:?}");
    assert!(
        questions[0].starts_with(&format!("note: {}\n", unused_note(STAGED)))
            && questions[0].ends_with(&format!(
                "complete {id} by its commit {commit} on t1 in {}? [y/N]",
                pair.linked.display()
            )),
        "{}",
        questions[0]
    );
    let reason = refused(&outcome, "an unused stage replaced");
    assert!(
        reason.ends_with(&format!(
            "{id} changed since the question: its staged choice was replaced or removed; nothing \
             changed"
        )),
        "{reason}"
    );
    let proposal = pair.proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Open);
    assert_eq!(proposal.decision_note, None);
    assert_eq!(
        proposal.staged.map(|staged| staged.stage),
        Some(replacement)
    );
    let kinds = kinds_of(&pair, &id);
    assert!(!kinds.contains(&"proposal.applied".to_owned()), "{kinds:?}");
}
