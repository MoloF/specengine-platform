//! docs/features/rev-discipline.md RD-5 ("Review document, stage, queue";
//! Q14): the approve stage's `editorial` (`true` or `null`, after `canon`),
//! refused on a kind writing no existing node; a staged editorial
//! confirmed by a bare `spec approve PR`, its staged line ending in
//! `--editorial`, its note kept; typed flags (`--editorial` too) replacing
//! it whole. Through the library over a scratch repository of
//! `fixtures/spec-a` (`common::proposal::Pair`); the stage stored and read
//! back with `sqlite3`. The schema-7 migration's store half is
//! `crates/specengine-store/tests/queue_stage.rs`
//! `schema_7_rewrites_open_approve_stages`; the daemon's POST is
//! `crates/specengine-http/tests/stage.rs` (TE-3).

#![cfg(unix)]

mod common;
mod rev_common;

use common::proposal::{Pair, edit};
use common::read_text;
use rev_common::{At, LATER, after_base, approve_at, propose_create_text, proposed};
use specengine_cli::{
    ApproveFlags, ApproveRequest, Exit, Globals, ProposalOutcome, StageBody, StageCause,
    StageOutcome, StageRequest, approve, approve_with,
};

/// The stage's clock (after the proposals' `NOW`).
const STAGED: &str = "2026-10-05T21:30:00Z";
const SPRINT: &str = "docs/spec/movement/sprint.md";
const EMPTY: &str = "EDGE-SPRINT-EMPTY";
const EMPTY_LINE: (&str, &str) = ("the sprint ends;", "the sprint ends at once;");

fn linked_at(pair: &Pair) -> At<'_> {
    At::new(&pair.linked, &pair.home, pair.git_env(&pair.linked))
}

fn main_at(pair: &Pair) -> At<'_> {
    At::new(&pair.main, &pair.home, pair.git_env(&pair.main))
}

/// An update of `EDGE-SPRINT-EMPTY` raised on `t1`: its ID.
fn update(pair: &Pair) -> String {
    let (hash, text) = pair.span(&pair.linked, EMPTY);
    proposed(
        &linked_at(pair),
        EMPTY,
        &hash,
        &edit(&text, EMPTY_LINE.0, EMPTY_LINE.1),
    )
}

/// The library's `stage` of an approve with `editorial` and `note` on `id`
/// from the main worktree at [`STAGED`], against its stored `updated_at`.
fn stage_editorial(
    pair: &Pair,
    id: &str,
    editorial: Option<bool>,
    note: Option<&str>,
) -> StageOutcome {
    specengine_cli::stage(
        &pair.env(&pair.main),
        &Globals::default(),
        &StageRequest {
            id: id.to_owned(),
            body: StageBody::Approve {
                option: None,
                answer: None,
                canon: None,
                editorial,
                note: note.map(str::to_owned),
                updated_at: pair.proposal(id).updated_at,
            },
            now: STAGED.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
    .unwrap_or_else(|error| panic!("stage {id}: {error}"))
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

/// A library approve from the main worktree with `note` and `flags` typed
/// (`editorial`: `--editorial` typed too): the outcome and the questions.
fn approve_typed(
    pair: &Pair,
    id: &str,
    note: Option<&str>,
    editorial: bool,
    answer: bool,
) -> (ProposalOutcome, Vec<String>) {
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        answer
    };
    let request = ApproveRequest {
        id: id.to_owned(),
        note: note.map(str::to_owned),
        now: LATER.to_owned(),
        git: pair.git_env(&pair.main),
    };
    let outcome = if editorial {
        specengine_cli::approve_editorial(
            &pair.env(&pair.main),
            &Globals::default(),
            &request,
            &ApproveFlags::default(),
            &mut consent,
        )
    } else if note.is_some() {
        approve_with(
            &pair.env(&pair.main),
            &Globals::default(),
            &request,
            &ApproveFlags::default(),
            &mut consent,
        )
    } else {
        approve(
            &pair.env(&pair.main),
            &Globals::default(),
            &request,
            &mut consent,
        )
    };
    (outcome.expect("approve runs"), questions)
}

/// The apply question of `id` on `t1`, `tail` after the preview.
fn question(pair: &Pair, id: &str, tail: &str) -> String {
    format!(
        "apply {id} to {SPRINT} on t1 in {} ({tail})? [y/N]",
        pair.linked.display()
    )
}

/// The note of typed flags over a stage made at `at`.
fn unused_note(at: &str) -> String {
    format!("note: the choice staged {at} is not used: the typed flags decide")
}

// ------------------------------------------------------------------ RD-5

/// RD-5: `editorial: true` on a kind writing no existing node (a new
/// file's create) is refused when staged as a usage defect (exit 2, the
/// daemon's 400), nothing stored, no event; typed, `spec approve PR
/// --editorial` on it exits 2 before the question, nothing changed.
/// M: `editorial` accepted on a new file.
#[test]
fn editorial_on_a_new_file_is_a_usage_error() {
    let pair = Pair::new("rd5-new-file", "spec-a");
    let text = "---\nclass: canon\nid: MEC-DASH\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
                # Dash\n\nA short burst.\n";
    let created = propose_create_text(&linked_at(&pair), "docs/spec/movement/dash.md", None, text)
        .expect("create")
        .document
        .id
        .clone()
        .expect("stored");
    let events = pair.events().len();
    let outcome = stage_editorial(&pair, &created, Some(true), None);
    assert_eq!(outcome.cause, Some(StageCause::Usage), "{outcome:?}");
    assert_eq!(outcome.exit(), Exit::CannotRun);
    assert_eq!(stage_columns(&pair, &created), "-|-", "nothing stored");
    assert_eq!(pair.events().len(), events, "no event");
    // `null` stages as ever.
    let outcome = stage_editorial(&pair, &created, None, None);
    assert_eq!(outcome.cause, None, "{outcome:?}");
    assert!(
        stage_columns(&pair, &created).starts_with(
            "{\"decision\":\"approve\",\"option\":null,\"answer\":null,\"canon\":null,\
             \"editorial\":null,\"note\":null,\"span_hash\":null}|"
        ),
        "{}",
        stage_columns(&pair, &created)
    );

    let before = pair.state();
    let asked = approve_at(&main_at(&pair), &created, true, true);
    let message = asked.cannot("--editorial on a new file");
    assert!(message.contains("--editorial"), "{message}");
    assert!(asked.questions.is_empty(), "before the question");
    assert_eq!(pair.state(), before, "nothing changed");
}

/// RD-5 (Q14): a staged `editorial: true` with a note is confirmed by a
/// bare `spec approve PR`: the staged line `staged <at>: spec approve PR
/// --note "typo" --editorial` above the question `(applies, editorial: …
/// stays 1, staged)`; declined, the stage kept; consented, no raise, the
/// note kept as the decision's, `.approved` carrying `staged_at`, the
/// trailer `Revision: … 1 (editorial)`. M: the stage's `editorial` not
/// read back (a meaning approve).
#[test]
fn staged_editorial_is_confirmed_by_a_bare_approve() {
    let pair = Pair::new("rd5-confirm", "spec-a");
    let id = update(&pair);
    let (span, _) = pair.span(&pair.linked, EMPTY);
    let outcome = stage_editorial(&pair, &id, Some(true), Some("typo"));
    assert_eq!(outcome.cause, None, "{outcome:?}");
    let stored = format!(
        "{{\"decision\":\"approve\",\"option\":null,\"answer\":null,\"canon\":null,\
         \"editorial\":true,\"note\":\"typo\",\"span_hash\":\"{span}\"}}|{STAGED}"
    );
    assert_eq!(stage_columns(&pair, &id), stored);
    let (review, _) = rev_common::review_at(&main_at(&pair), &id);
    assert!(review.contains("\"editorial\":true"), "{review}");

    let want = format!(
        "staged {STAGED}: spec approve {id} --note \"typo\" --editorial\n{}",
        question(
            &pair,
            &id,
            "applies, editorial: EDGE-SPRINT-EMPTY stays 1, staged"
        )
    );
    let before = read_text(&pair.linked, SPRINT);
    let (declined, questions) = approve_typed(&pair, &id, None, false, false);
    assert_eq!(declined.exit(), Exit::NotFound);
    assert_eq!(questions, std::slice::from_ref(&want));
    assert_eq!(stage_columns(&pair, &id), stored, "the stage kept");

    let (applied, questions) = approve_typed(&pair, &id, None, false, true);
    assert_eq!(applied.exit(), Exit::Answered, "{applied:?}");
    assert_eq!(questions, [want]);
    assert_eq!(
        read_text(&pair.linked, SPRINT),
        edit(&before, EMPTY_LINE.0, EMPTY_LINE.1),
        "no raise"
    );
    let row = pair.proposal(&id);
    assert_eq!(row.decision_note.as_deref(), Some("typo"));
    let approved = pair
        .events()
        .into_iter()
        .rev()
        .find(|event| event.event_type == "proposal.approved")
        .expect("approved");
    assert_eq!(approved.payload["staged_at"], STAGED);
    let message = pair.git_text(&pair.main, &["log", "-1", "--format=%B", "t1"]);
    assert_eq!(
        after_base(&message),
        ["Revision: EDGE-SPRINT-EMPTY 1 (editorial)"]
    );
}

/// RD-5 (Q14): typed flags replace a staged editorial whole, never merged:
/// a typed `--editorial` applies editorially with no note (the staged one
/// dropped), the unused-stage note above the question; a typed `--note`
/// alone over a staged editorial is a meaning approve (the raise) with the
/// typed note. M: a typed `--editorial` confirming the stage (its note
/// kept).
#[test]
fn typed_flags_replace_a_staged_editorial() {
    let pair = Pair::new("rd5-typed", "spec-a");
    let id = update(&pair);
    assert_eq!(
        stage_editorial(&pair, &id, Some(true), Some("typo")).cause,
        None
    );
    let (outcome, questions) = approve_typed(&pair, &id, None, true, true);
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "{}\n{}",
            unused_note(STAGED),
            question(&pair, &id, "applies, editorial: EDGE-SPRINT-EMPTY stays 1")
        )]
    );
    let row = pair.proposal(&id);
    assert_eq!(row.decision_note, None, "the staged note dropped");
    assert!(
        pair.events()
            .into_iter()
            .rev()
            .find(|event| event.event_type == "proposal.approved")
            .expect("approved")
            .payload
            .get("staged_at")
            .is_none(),
        "the stage not confirmed"
    );

    let pair = Pair::new("rd5-typed-note", "spec-a");
    let id = update(&pair);
    assert_eq!(
        stage_editorial(&pair, &id, Some(true), Some("typo")).cause,
        None
    );
    let (outcome, questions) = approve_typed(&pair, &id, Some("typed"), false, true);
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert_eq!(
        questions,
        [format!(
            "{}\n{}",
            unused_note(STAGED),
            question(&pair, &id, "applies; raises EDGE-SPRINT-EMPTY 1 -> 2")
        )]
    );
    assert!(read_text(&pair.linked, SPRINT).contains(".playtest rev=2}"));
    assert_eq!(pair.proposal(&id).decision_note.as_deref(), Some("typed"));
}
