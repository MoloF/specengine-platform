//! The store half of docs/features/kind-decision.md (AC-R1, AC-R3, AC-R6,
//! AC-R12; "Data" Row, W17, W18):
//!
//! - AC-R1: `ProposalKind::Decision` is the sixth of `ALL`, named by core's
//!   `DECISION_KIND`; it `decides()` and does not `applies()`; `create`
//!   refuses it (`Invalid`, nothing written); `create_intake` stores it
//!   `open` with its linked update under the next ID, `linked` both ways,
//!   and `get` reads both back equal. Its row is a question's: the same
//!   columns, byte for byte, but `kind` (`gap_type`, `evidence`,
//!   `options`, `recommendation` NULL). M: `decides()` false; the row
//!   decoded by `== Question` (`intake_shape` putting a decision with a
//!   discrepancy).
//! - AC-R3, W18: a question and a decision are one dedup family, a
//!   discrepancy another: equal normalised text on a shared target hits
//!   across the family both ways (nothing stored, no ID taken; named in
//!   `distinct_from`: stored); other text is related; a discrepancy never
//!   meets either. M: dedup by exact kind.
//! - AC-R6: a decision's stage takes no `option` (`Invalid` naming it,
//!   nothing written, no event), takes no flag or an `answer`; a row whose
//!   stage or `choice` holds an option, or that holds a discrepancy's
//!   column, is corrupt, named by `get`, skipped and named by
//!   `list_readable`; `approve_record_from` with an option is `Invalid`.
//!   M: `stage.rs`'s arm `None`.
//! - AC-R12: a decision, its update and its record: `dump`, `restore`
//!   into a fresh queue, `dump` byte-identical, `stored_rows` equal. M: the
//!   decode refusing the kind.
//!
//! Through the public API only; a broken row is made by `stored_rows`,
//! one column edited, and `restore` (which takes rows raw).

#![cfg(unix)]

mod common;

use common::{Scratch, blake3_hex};
use specengine_core::intake::{
    DECISION_KIND, Evidence, GapType, IntakeOption, IntakeSeverity, QUESTION_KIND,
};
use specengine_core::proposal::Author;
use specengine_core::record::Choice;
use specengine_store::{
    Decision, Intake, NewIntake, NewProposal, PROPOSAL_COLUMNS, Place, ProposalFilter,
    ProposalKind, ProposalQueue as _, ProposalStatus, QueueError, RecordApproval, RecordSeries,
    SqliteQueue, Stage, StoredQueue, patch_hash,
};

const T0: &str = "2026-10-08T09:00:00Z";
const T1: &str = "2026-10-08T09:14:02Z";
const T2: &str = "2026-10-08T09:20:00Z";
const PROJECT: &str = "lantern-keep";
const STAMINA: &str = "docs/spec/movement/stamina.md";

/// "Data"'s example.
const TEXT: &str = "Does regeneration wait for rest?";
const WORKING: &str = "Yes: it starts 1.5 s after the last stamina use.";
const PRICE: &str = "Without the wait R-28 is rebalanced.";

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r/t1".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn agent() -> Author {
    Author::new(
        Some("developer".to_owned()),
        Some("claude-opus-5-5".to_owned()),
        None,
    )
    .expect("an agent")
}

/// An item of a question's shape: `kind` a question or a decision.
fn answered_kind(
    kind: ProposalKind,
    targets: &[&str],
    text: &str,
    distinct_from: &[&str],
) -> NewIntake {
    NewIntake {
        kind,
        target_path: STAMINA.to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some(WORKING.to_owned()),
            price_of_other: Some(PRICE.to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: distinct_from.iter().map(|id| (*id).to_owned()).collect(),
        },
    }
}

fn decision(targets: &[&str], text: &str, distinct_from: &[&str]) -> NewIntake {
    answered_kind(ProposalKind::Decision, targets, text, distinct_from)
}

fn question(targets: &[&str], text: &str, distinct_from: &[&str]) -> NewIntake {
    answered_kind(ProposalKind::Question, targets, text, distinct_from)
}

fn discrepancy(targets: &[&str], summary: &str, distinct_from: &[&str]) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: STAMINA.to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "src/stamina.rs".to_owned(),
                qpath: None,
                lines: Some("3-9".to_owned()),
                observed: "regenerates while sprinting".to_owned(),
                documented: "regenerates at rest".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(1),
            distinct_from: distinct_from.iter().map(|id| (*id).to_owned()).collect(),
        },
    }
}

/// The decision's proposed patch, as the CLI stores it: an update of
/// `target`.
fn patch(target: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: STAMINA.to_owned(),
        place: place(),
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Rest only.".to_owned(),
        author: agent(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    }
}

fn owner() -> Decision {
    Decision {
        decided_by: "Ann Owner <ann@example.org>".to_owned(),
        note: Some("Because.".to_owned()),
        staged_at: None,
    }
}

fn approval(choice: Choice) -> RecordApproval {
    RecordApproval {
        series: RecordSeries {
            prefix: "DEC".to_owned(),
            width: 4,
            corpus_max: 23,
        },
        preview: "DEC-0024".to_owned(),
        path: "docs/records/DEC/DEC-0024.md".to_owned(),
        title: TEXT.to_owned(),
        text: "---\nid: DEC-0024\n---\n".to_owned(),
        choice,
    }
}

fn column_at(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

fn invalid(result: Result<impl std::fmt::Debug, QueueError>, context: &str) -> String {
    match result {
        Err(QueueError::Invalid(reason)) => reason,
        other => panic!("{context}: expected Invalid, got {other:?}"),
    }
}

/// The IDs of `ids` matches.
fn ids(matches: &[specengine_store::QueueMatch]) -> Vec<&str> {
    matches.iter().map(|found| found.id.as_str()).collect()
}

/// The one row `id` of `state`, its columns as stored.
fn row<'a>(state: &'a StoredQueue, id: &str) -> &'a [Option<String>] {
    &state
        .proposals
        .iter()
        .find(|row| row.id() == Some(id))
        .unwrap_or_else(|| panic!("no row {id}"))
        .columns
}

// ------------------------------------------------------------------ AC-R1

/// AC-R1: the sixth kind, named by core's constant, decides and never
/// applies; the kinds that decide are exactly a question, a discrepancy
/// and a decision. M: `decides()` false.
#[test]
fn ac_r1_a_decision_decides_and_never_applies() {
    assert_eq!(DECISION_KIND, "decision");
    // Seven since docs/features/kind-amendment.md appends `amendment`.
    assert_eq!(ProposalKind::ALL.len(), 7);
    assert_eq!(ProposalKind::ALL[5], ProposalKind::Decision);
    let kind = ProposalKind::Decision;
    assert_eq!(kind.as_str(), DECISION_KIND);
    assert_eq!(ProposalKind::parse(DECISION_KIND), Some(kind));
    assert_eq!(kind.with_article(), "a decision");
    assert!(!kind.applies(), "a decision never applies");
    assert!(kind.decides(), "a decision decides");
    let deciding: Vec<&str> = ProposalKind::ALL
        .into_iter()
        .filter(|kind| kind.decides())
        .map(ProposalKind::as_str)
        .collect();
    assert_eq!(deciding, ["question", "discrepancy", "decision"]);
    // Every kind either applies or decides, never both.
    for kind in ProposalKind::ALL {
        assert_ne!(kind.applies(), kind.decides(), "{kind:?}");
    }
}

/// AC-R1: `create` refuses a decision (`Invalid` naming the kind and
/// `create_intake`), nothing written, no event, no ID taken.
#[test]
fn ac_r1_create_refuses_a_decision_unwritten() {
    let scratch = Scratch::new("qd-create");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let dump = queue.dump().expect("dump");
    let mut proposal = patch("RULE-STAM-REGEN");
    proposal.kind = ProposalKind::Decision;
    let message = invalid(queue.create(&proposal, T0), "create of a decision");
    assert!(
        message.contains("not a decision") && message.contains("create_intake"),
        "{message}"
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    assert!(queue.events().expect("events").is_empty(), "no event");
    // `create_intake` refuses a kind that applies, naming the three that
    // decide.
    let mut applying = decision(&["RULE-STAM-REGEN"], TEXT, &[]);
    applying.kind = ProposalKind::Update;
    let message = invalid(
        queue.create_intake(&applying, &[], None, T0),
        "create_intake of an update",
    );
    assert!(
        message.contains("question, discrepancy, decision") && message.contains("an update"),
        "{message}"
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    let stored = queue
        .create_intake(&decision(&["RULE-STAM-REGEN"], TEXT, &[]), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    assert_eq!(stored.id, "PR-0001", "no ID was taken");
}

/// AC-R1, "Data" Row: `create_intake` stores a decision `open` with its
/// linked update (`PR-0001`, `PR-0002`, `linked` both ways, a
/// `proposal.created` each); `get` reads both back equal, the intake as
/// given. M: the row decoded by `== Question`.
#[test]
fn ac_r1_a_decision_is_stored_with_its_linked_update_and_reads_back_equal() {
    let scratch = Scratch::new("qd-linked");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let given = decision(&["RULE-STAM-REGEN", "EDGE-STAM-ZERO"], TEXT, &[]);
    let result = queue
        .create_intake(&given, &[], Some(&patch("RULE-STAM-REGEN")), T0)
        .expect("intake");
    assert!(result.hits.is_empty() && result.related.is_empty());
    let stored = result.created.expect("stored");
    let linked = result.linked.expect("linked");
    assert_eq!(
        (stored.id.as_str(), linked.id.as_str()),
        ("PR-0001", "PR-0002")
    );
    assert_eq!(stored.kind, ProposalKind::Decision);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.target_id, "RULE-STAM-REGEN");
    assert_eq!(stored.linked.as_deref(), Some("PR-0002"));
    assert_eq!(linked.kind, ProposalKind::Update);
    assert_eq!(linked.linked.as_deref(), Some("PR-0001"));
    assert!(linked.intake.is_none());
    assert_eq!(stored.intake.as_ref(), Some(&given.intake));
    assert!(stored.record.is_none());
    assert_eq!(queue.get("PR-0001").unwrap().unwrap(), stored);
    assert_eq!(queue.get("PR-0002").unwrap().unwrap(), linked);
    let listed = queue.list(&ProposalFilter::default()).expect("list");
    assert_eq!(listed, [stored.clone(), linked.clone()]);
    let events: Vec<(String, String)> = queue
        .events()
        .expect("events")
        .into_iter()
        .map(|event| {
            (
                event.event_type,
                event.payload["id"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        events,
        [
            ("proposal.created".to_owned(), "PR-0001".to_owned()),
            ("proposal.created".to_owned(), "PR-0002".to_owned()),
        ]
    );
    // Reopened, the same.
    drop(queue);
    let queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("reopen");
    assert_eq!(queue.get("PR-0001").unwrap().unwrap(), stored);
    assert_eq!(queue.get("PR-0002").unwrap().unwrap(), linked);
}

/// "Data" Row, W17: a decision's row is a question's, byte for byte but
/// `kind`: `summary` the text, `working_answer`, `price_of_other` set,
/// `gap_type`, `evidence`, `options`, `recommendation` NULL. M: a
/// decision written by a discrepancy's columns.
#[test]
fn a_decision_row_holds_a_question_columns() {
    let scratch = Scratch::new("qd-row");
    let rows = |kind: ProposalKind, name: &str| {
        let mut queue = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
        queue
            .create_intake(
                &answered_kind(kind, &["RULE-STAM-REGEN"], TEXT, &["DEC-0023"]),
                &[],
                None,
                T0,
            )
            .expect("intake")
            .created
            .expect("stored");
        queue.stored_rows().expect("rows")
    };
    let decided = rows(ProposalKind::Decision, "decision");
    let asked = rows(ProposalKind::Question, "question");
    let decision_row = row(&decided, "PR-0001");
    let question_row = row(&asked, "PR-0001");
    assert_eq!(decision_row.len(), PROPOSAL_COLUMNS.len());
    for (at, name) in PROPOSAL_COLUMNS.iter().enumerate() {
        if *name == "kind" {
            assert_eq!(decision_row[at].as_deref(), Some(DECISION_KIND));
            assert_eq!(question_row[at].as_deref(), Some(QUESTION_KIND));
        } else {
            assert_eq!(decision_row[at], question_row[at], "column {name}");
        }
    }
    for name in [
        "gap_type",
        "evidence",
        "options",
        "recommendation",
        "linked",
    ] {
        assert_eq!(decision_row[column_at(name)], None, "{name} NULL");
    }
    assert_eq!(decision_row[column_at("summary")].as_deref(), Some(TEXT));
    assert_eq!(
        decision_row[column_at("working_answer")].as_deref(),
        Some(WORKING)
    );
    assert_eq!(
        decision_row[column_at("price_of_other")].as_deref(),
        Some(PRICE)
    );
    assert_eq!(
        decision_row[column_at("distinct_from")].as_deref(),
        Some("[\"DEC-0023\"]")
    );
}

// ------------------------------------------------------------------ AC-R3

/// AC-R3, W18: one family `{question, decision}`, another
/// `{discrepancy}`: a decision equal to an open question on a shared
/// target is a hit (nothing stored, no ID taken), stored when named; a
/// question after a decision hits it; other text on a shared target is
/// related; a discrepancy with the same summary meets neither, and
/// neither meets it. M: `queue_matches` by exact kind.
#[test]
fn ac_r3_a_question_and_a_decision_are_one_family_a_discrepancy_another() {
    let scratch = Scratch::new("qd-family");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let store = |queue: &mut SqliteQueue, item: &NewIntake| {
        queue.create_intake(item, &[], None, T0).expect("intake")
    };
    // PR-0001: an open question.
    let first = store(
        &mut queue,
        &question(&["RULE-STAM-REGEN", "EDGE-STAM-ZERO"], TEXT, &[]),
    );
    assert_eq!(first.created.expect("stored").id, "PR-0001");
    let dump = queue.dump().expect("dump");

    // A decision of the same normalised text on a shared target: a hit.
    let spaced = "  does   REGENERATION wait for rest? ";
    let hit = store(&mut queue, &decision(&["EDGE-STAM-ZERO"], spaced, &[]));
    assert!(hit.created.is_none(), "a hit, not stored: {hit:?}");
    assert_eq!(ids(&hit.hits), ["PR-0001"]);
    assert!(hit.related.is_empty());
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    // Named in `distinct_from`: stored as PR-0002 (no ID was taken).
    let named = store(
        &mut queue,
        &decision(&["EDGE-STAM-ZERO"], spaced, &["PR-0001"]),
    );
    assert_eq!(ids(&named.hits), ["PR-0001"]);
    assert_eq!(named.created.expect("stored").id, "PR-0002");

    // A question after a decision: a hit, both ways.
    let again = store(&mut queue, &question(&["EDGE-STAM-ZERO"], TEXT, &[]));
    assert!(again.created.is_none());
    assert_eq!(ids(&again.hits), ["PR-0001", "PR-0002"]);
    let only_decision = store(&mut queue, &question(&["SOLO-1"], "Solo?", &[]));
    assert_eq!(only_decision.created.expect("stored").id, "PR-0003");
    let decided = store(&mut queue, &decision(&["SOLO-2"], "Solo two?", &[]));
    assert_eq!(decided.created.expect("stored").id, "PR-0004");
    let asked = store(&mut queue, &question(&["SOLO-2"], "Solo two?", &[]));
    assert!(asked.created.is_none(), "a question after a decision hits");
    assert_eq!(ids(&asked.hits), ["PR-0004"]);

    // Other text on a shared target: related across the family.
    let other = store(
        &mut queue,
        &decision(&["RULE-STAM-REGEN"], "How long is the wait?", &[]),
    );
    assert!(other.hits.is_empty());
    assert_eq!(ids(&other.related), ["PR-0001"]);
    assert_eq!(other.created.expect("stored").id, "PR-0005");

    // A discrepancy of the same summary: neither a hit nor related.
    let found = store(&mut queue, &discrepancy(&["RULE-STAM-REGEN"], TEXT, &[]));
    assert!(
        found.hits.is_empty() && found.related.is_empty(),
        "{found:?}"
    );
    assert_eq!(found.created.expect("stored").id, "PR-0006");
    // And a decision or a question after it meets only its own family.
    let after = store(
        &mut queue,
        &decision(&["SOLO-3", "RULE-STAM-REGEN"], TEXT, &[]),
    );
    assert_eq!(ids(&after.hits), ["PR-0001"], "not the discrepancy");
    assert_eq!(ids(&after.related), ["PR-0005"]);
    assert!(after.created.is_none());
    // A discrepancy after a decision on its own target: nothing met.
    let alone = store(&mut queue, &discrepancy(&["SOLO-2"], "Solo two?", &[]));
    assert!(
        alone.hits.is_empty() && alone.related.is_empty(),
        "{alone:?}"
    );
    assert_eq!(alone.created.expect("stored").id, "PR-0007");
    // A second discrepancy of the same summary hits the first only.
    let twice = store(&mut queue, &discrepancy(&["RULE-STAM-REGEN"], TEXT, &[]));
    assert_eq!(ids(&twice.hits), ["PR-0006"]);
    assert!(twice.created.is_none());
}

// ------------------------------------------------------------------ AC-R6

/// AC-R6: a decision's stage takes no `option` (`Stage::problem` names it;
/// `stage_from` is `Invalid` naming the item and `option`, nothing
/// written, no event); no flag, or an `answer`, is staged and read back.
/// M: `stage.rs`'s arm `None`.
#[test]
fn ac_r6_a_decision_stage_takes_an_answer_never_an_option() {
    let approve = |option: Option<u64>, answer: Option<&str>| Stage::Approve {
        option,
        answer: answer.map(str::to_owned),
        canon: None,
        note: None,
        span_hash: None,
    };
    let kind = ProposalKind::Decision;
    assert_eq!(
        approve(Some(0), None).problem(kind, false).as_deref(),
        Some("a staged `option` on a decision, which takes none")
    );
    assert!(approve(None, None).problem(kind, false).is_none());
    assert!(approve(None, Some("No.")).problem(kind, false).is_none());
    // A deciding kind has no span.
    let spanned = Stage::Approve {
        option: None,
        answer: None,
        canon: None,
        note: None,
        span_hash: Some(format!("b3:{}", blake3_hex(b"span"))),
    };
    assert_eq!(
        spanned.problem(kind, false).as_deref(),
        Some("a staged `span_hash` on a decision that has no target span")
    );

    let scratch = Scratch::new("qd-stage");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    queue
        .create_intake(&decision(&["RULE-STAM-REGEN"], TEXT, &[]), &[], None, T0)
        .expect("intake");
    let dump = queue.dump().expect("dump");
    let events = queue.events().expect("events").len();
    let seen = queue.get("PR-0001").unwrap().unwrap().seen();
    let message = invalid(
        queue.stage_from("PR-0001", &seen, &approve(Some(0), None), T1),
        "an option staged on a decision",
    );
    assert!(
        message.contains("PR-0001") && message.contains("option"),
        "{message}"
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    assert_eq!(queue.events().expect("events").len(), events, "no event");

    for (stage, at) in [(approve(None, None), T1), (approve(None, Some("No.")), T2)] {
        let seen = queue.get("PR-0001").unwrap().unwrap().seen();
        let staged = queue
            .stage_from("PR-0001", &seen, &stage, at)
            .unwrap_or_else(|error| panic!("{stage:?}: {error}"));
        let held = staged.staged.expect("a stage");
        assert_eq!(held.at, at);
        assert_eq!(
            queue
                .get("PR-0001")
                .unwrap()
                .unwrap()
                .staged
                .expect("read back")
                .at,
            at
        );
    }
    let staged = queue.stored_rows().expect("rows");
    assert_eq!(
        row(&staged, "PR-0001")[column_at("staged")].as_deref(),
        Some(
            "{\"decision\":\"approve\",\"option\":null,\"answer\":\"No.\",\"canon\":null,\
             \"note\":null,\"span_hash\":null}"
        )
    );
}

/// AC-R1, AC-R6: `approve_record_from` takes the working answer or the
/// owner's answer (`{"working_answer":true}`, `{"answer":"…"}`), never an
/// option (`Invalid`, nothing written); the record reads back.
#[test]
fn ac_r6_a_decision_record_takes_its_working_answer_or_an_answer() {
    let scratch = Scratch::new("qd-record");
    for (index, choice, stored) in [
        (0, Choice::WorkingAnswer, "{\"working_answer\":true}"),
        (
            1,
            Choice::Answer("No: at once.".to_owned()),
            "{\"answer\":\"No: at once.\"}",
        ),
    ] {
        let mut queue = SqliteQueue::open(scratch.db(&format!("q{index}")), PROJECT).expect("open");
        queue
            .create_intake(
                &decision(&["RULE-STAM-REGEN"], TEXT, &[]),
                &[],
                Some(&patch("RULE-STAM-REGEN")),
                T0,
            )
            .expect("intake");
        let open = queue.get("PR-0001").unwrap().unwrap();
        let dump = queue.dump().expect("dump");
        let message = invalid(
            queue.approve_record_from(
                "PR-0001",
                &open.seen(),
                &approval(Choice::Option(0)),
                &owner(),
                T1,
            ),
            "an option on a decision",
        );
        assert!(
            message.contains("a decision") && message.contains("option"),
            "{message}"
        );
        assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
        let approved = queue
            .approve_record_from(
                "PR-0001",
                &open.seen(),
                &approval(choice.clone()),
                &owner(),
                T1,
            )
            .expect("recorded");
        assert_eq!(approved.status, ProposalStatus::Approved);
        let record = approved.record.clone().expect("a record");
        assert_eq!(record.id, "DEC-0024");
        assert_eq!(record.choice, choice);
        assert_eq!(queue.get("PR-0001").unwrap().unwrap(), approved);
        let rows = queue.stored_rows().expect("rows");
        assert_eq!(
            row(&rows, "PR-0001")[column_at("choice")].as_deref(),
            Some(stored)
        );
        // The linked update is untouched: decided on its own.
        assert_eq!(
            queue.get("PR-0002").unwrap().unwrap().status,
            ProposalStatus::Open
        );
    }
}

/// AC-R6, "Data" Row: corrupt as a question's row, plus an option: an
/// `option` stage, an option `choice`, a question's column NULL. Each broken row restored alone: `get` fails
/// naming `PR-0001` and the column, `list` fails, `list_readable` skips
/// it naming it; the unbroken rows read.
#[test]
fn ac_r6_a_corrupt_decision_row_is_named() {
    let scratch = Scratch::new("qd-corrupt");
    // An open decision with a stage; an approved one with its record.
    let mut queue = SqliteQueue::open(scratch.db("open"), PROJECT).expect("open");
    queue
        .create_intake(&decision(&["RULE-STAM-REGEN"], TEXT, &[]), &[], None, T0)
        .expect("intake");
    let seen = queue.get("PR-0001").unwrap().unwrap().seen();
    queue
        .stage_from(
            "PR-0001",
            &seen,
            &Stage::Approve {
                option: None,
                answer: Some("No.".to_owned()),
                canon: None,
                note: None,
                span_hash: None,
            },
            T1,
        )
        .expect("staged");
    let open = queue.stored_rows().expect("rows");
    drop(queue);
    let mut queue = SqliteQueue::open(scratch.db("approved"), PROJECT).expect("open");
    queue
        .create_intake(&decision(&["RULE-STAM-REGEN"], TEXT, &[]), &[], None, T0)
        .expect("intake");
    let seen = queue.get("PR-0001").unwrap().unwrap().seen();
    queue
        .approve_record_from(
            "PR-0001",
            &seen,
            &approval(Choice::WorkingAnswer),
            &owner(),
            T1,
        )
        .expect("recorded");
    let approved = queue.stored_rows().expect("rows");
    drop(queue);

    let stage = |body: &str| {
        Some(format!(
            "{{\"decision\":\"approve\",{body},\"canon\":null,\"note\":null,\"span_hash\":null}}"
        ))
    };
    type Case<'a> = (
        &'a str,
        &'a StoredQueue,
        Vec<(&'a str, Option<String>)>,
        &'a str,
    );
    let text = |value: &str| Some(value.to_owned());
    let cases: Vec<Case<'_>> = vec![
        (
            "an option staged",
            &open,
            vec![("staged", stage("\"option\":0,\"answer\":null"))],
            "staged",
        ),
        (
            "an option and an answer staged",
            &open,
            vec![("staged", stage("\"option\":1,\"answer\":\"No.\""))],
            "staged",
        ),
        (
            "an option chosen",
            &approved,
            vec![("choice", text("{\"option\":0}"))],
            "choice",
        ),
        (
            "two keys chosen",
            &approved,
            vec![("choice", text("{\"option\":0,\"answer\":\"x\"}"))],
            "choice",
        ),
        (
            "no working answer",
            &open,
            vec![("working_answer", None)],
            "working_answer",
        ),
        (
            "no price",
            &open,
            vec![("price_of_other", None)],
            "price_of_other",
        ),
        ("no text", &open, vec![("summary", None)], "summary"),
    ];
    for (index, (label, base, edits, column)) in cases.into_iter().enumerate() {
        let mut state = base.clone();
        state.events.clear();
        for (name, value) in edits {
            state.proposals[0].columns[column_at(name)] = value;
        }
        let db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&db, PROJECT).expect("open");
        broken.restore(&state).expect("restore takes rows raw");
        let message = match broken.get("PR-0001") {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        assert!(
            message.contains("PR-0001") && message.contains(&format!("`{column}`")),
            "{label}: {message}"
        );
        assert!(
            broken.list(&ProposalFilter::default()).is_err(),
            "{label}: list fails"
        );
        let listed = broken
            .list_readable(&ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].id, "PR-0001", "{label}");
        assert_eq!(listed.unreadable[0].column, column, "{label}");
    }
    // The unbroken rows read.
    for (name, state) in [("good-open", &open), ("good-approved", &approved)] {
        let mut good = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
        good.restore(state).expect("restore");
        let read = good.get("PR-0001").unwrap().unwrap();
        assert_eq!(read.kind, ProposalKind::Decision, "{name}");
    }
}

// ----------------------------------------------------------------- AC-R12

/// AC-R12: a decision, its linked update and its record (and a staged
/// decision beside them): `dump`, `restore` into a fresh queue, `dump`
/// byte-identical, `stored_rows` equal, each proposal read back equal. M:
/// the decode refusing the kind.
#[test]
fn ac_r12_a_decision_its_update_and_record_round_trip_byte_identical() {
    let scratch = Scratch::new("qd-backup");
    let mut queue = SqliteQueue::open(scratch.db("source"), PROJECT).expect("open");
    queue
        .create_intake(
            &decision(&["RULE-STAM-REGEN"], TEXT, &[]),
            &[],
            Some(&patch("RULE-STAM-REGEN")),
            T0,
        )
        .expect("intake");
    queue
        .create_intake(
            &decision(&["EDGE-STAM-ZERO"], "Is zero stamina a stop?", &[]),
            &[],
            None,
            T0,
        )
        .expect("intake");
    let open = queue.get("PR-0001").unwrap().unwrap();
    queue
        .approve_record_from(
            "PR-0001",
            &open.seen(),
            &approval(Choice::Answer("No: at once.".to_owned())),
            &owner(),
            T1,
        )
        .expect("recorded");
    let seen = queue.get("PR-0003").unwrap().unwrap().seen();
    queue
        .stage_from(
            "PR-0003",
            &seen,
            &Stage::Approve {
                option: None,
                answer: None,
                canon: Some("EDGE-STAM-ZERO".to_owned()),
                note: Some("Keep.".to_owned()),
                span_hash: None,
            },
            T2,
        )
        .expect("staged");
    let dump = queue.dump().expect("dump");
    assert!(dump.contains("\"decision\""), "the kind is in the dump");
    let rows = queue.stored_rows().expect("rows");
    let proposals = queue.list(&ProposalFilter::default()).expect("list");
    assert_eq!(proposals.len(), 3);
    drop(queue);

    let mut restored = SqliteQueue::open(scratch.db("restored"), PROJECT).expect("open");
    restored.restore(&rows).expect("restore");
    assert_eq!(restored.dump().expect("dump"), dump, "byte-identical");
    assert_eq!(restored.stored_rows().expect("rows"), rows);
    assert_eq!(
        restored.list(&ProposalFilter::default()).expect("list"),
        proposals
    );
    let record = restored
        .get("PR-0001")
        .unwrap()
        .unwrap()
        .record
        .expect("its record");
    assert_eq!(record.choice, Choice::Answer("No: at once.".to_owned()));
    // A third copy from the second: still the same bytes.
    let again = restored.stored_rows().expect("rows");
    let mut third = SqliteQueue::open(scratch.db("third"), PROJECT).expect("open");
    third.restore(&again).expect("restore");
    assert_eq!(third.dump().expect("dump"), dump);
}

// ------------------------------------------- iteration 2: the row's shape

/// The queue's rows of one stored item of `item`'s kind: a fresh queue at
/// `name`, the item stored as `PR-0001`.
fn rows_of(scratch: &Scratch, name: &str, item: &NewIntake) -> StoredQueue {
    let mut queue = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
    queue
        .create_intake(item, &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    queue.stored_rows().expect("rows")
}

/// The stricter row reading (iteration 2, `IntakeShape::null_columns`): a
/// question- or decision-shaped row holding a discrepancy's column
/// (`gap_type`, `evidence`, `options`, `recommendation`; an empty list
/// `[]` counts as set), and a discrepancy row holding `price_of_other`,
/// are corrupt, named: `get` and `list` fail with "proposal PR-0001: the
/// stored `<column>` cannot be read: it is set on <kind>, which holds
/// none", `list_readable` skips the row naming it and the column. The
/// unbroken rows read. M: a column dropped from `null_columns()`.
#[test]
fn a_row_holding_a_column_its_shape_lacks_is_named_corrupt() {
    let scratch = Scratch::new("qd-shape-rows");
    let option = "[{\"label\":\"a\",\"effect\":\"b\",\"price\":\"c\"}]";
    let evidence = "[{\"file\":\"src/a.rs\",\"qpath\":null,\"lines\":null,\"observed\":\"o\",\
                    \"documented\":\"d\"}]";
    let mut cases: Vec<(String, StoredQueue, &str, &str)> = Vec::new();
    for (kind, name) in [
        (ProposalKind::Question, "a question"),
        (ProposalKind::Decision, "a decision"),
    ] {
        let rows = rows_of(
            &scratch,
            kind.as_str(),
            &answered_kind(kind, &["RULE-STAM-REGEN"], TEXT, &[]),
        );
        for (column, value) in [
            ("gap_type", "contradicts"),
            ("evidence", "[]"),
            ("evidence", evidence),
            ("options", "[]"),
            ("options", option),
            ("recommendation", "0"),
        ] {
            let mut state = rows.clone();
            state.proposals[0].columns[column_at(column)] = Some(value.to_owned());
            cases.push((format!("{name} with {column} {value}"), state, column, name));
        }
    }
    let rows = rows_of(
        &scratch,
        "discrepancy",
        &discrepancy(&["RULE-STAM-REGEN"], TEXT, &[]),
    );
    for value in ["A rebalance.", ""] {
        let mut state = rows.clone();
        state.proposals[0].columns[column_at("price_of_other")] = Some(value.to_owned());
        cases.push((
            format!("a discrepancy with price_of_other {value:?}"),
            state,
            "price_of_other",
            "a discrepancy",
        ));
    }
    for (index, (label, mut state, column, kind)) in cases.into_iter().enumerate() {
        state.events.clear();
        let db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&db, PROJECT).expect("open");
        broken.restore(&state).expect("restore takes rows raw");
        let message = match broken.get("PR-0001") {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        assert!(
            message.ends_with(&format!(
                "proposal PR-0001: the stored `{column}` cannot be read: it is set on {kind}, \
                 which holds none"
            )),
            "{label}: {message}"
        );
        assert!(
            broken.list(&ProposalFilter::default()).is_err(),
            "{label}: list fails"
        );
        let listed = broken
            .list_readable(&ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].id, "PR-0001", "{label}");
        assert_eq!(listed.unreadable[0].column, column, "{label}");
    }
    // The rows as stored read: a question's and a decision's discrepancy
    // columns NULL, a discrepancy's `price_of_other` NULL.
    for name in ["question", "decision", "discrepancy"] {
        let queue = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
        assert!(queue.get("PR-0001").unwrap().is_some(), "{name}");
    }
}

/// `create_intake` refuses an item its row could not hold (iteration 2,
/// checked up front, before the dedup and the insert): `Invalid` with the
/// exact text, the queue's dump unchanged, no event, no ID taken; an item
/// of its shape (a decision's empty `evidence`, `options` are none) is
/// stored as `PR-0001`. M: the up-front check skipped.
#[test]
fn create_intake_refuses_an_item_of_another_shape_unwritten() {
    let scratch = Scratch::new("qd-shape-intake");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let dump = queue.dump().expect("dump");
    let evidence = Evidence {
        file: "src/a.rs".to_owned(),
        qpath: None,
        lines: None,
        observed: "o".to_owned(),
        documented: "d".to_owned(),
    };
    let option = IntakeOption {
        label: "a".to_owned(),
        effect: "b".to_owned(),
        price: "c".to_owned(),
    };
    let mut cases: Vec<(NewIntake, String)> = Vec::new();
    for (kind, name) in [
        (ProposalKind::Question, "a question"),
        (ProposalKind::Decision, "a decision"),
    ] {
        let item = || answered_kind(kind, &["RULE-STAM-REGEN"], TEXT, &[]);
        let mut gap = item();
        gap.intake.gap_type = Some(GapType::Contradicts);
        cases.push((gap, format!("{name} holds no `gap_type`")));
        let mut found = item();
        found.intake.evidence = vec![evidence.clone()];
        cases.push((found, format!("{name} holds no `evidence`")));
        let mut priced = item();
        priced.intake.options = vec![option.clone()];
        cases.push((priced, format!("{name} holds no `options`")));
        let mut recommended = item();
        recommended.intake.recommendation = Some(0);
        cases.push((recommended, format!("{name} holds no `recommendation`")));
        let mut unanswered = item();
        unanswered.intake.working_answer = None;
        cases.push((unanswered, format!("{name} lacks its `working_answer`")));
        let mut unpriced = item();
        unpriced.intake.price_of_other = None;
        cases.push((unpriced, format!("{name} lacks its `price_of_other`")));
    }
    let report = || discrepancy(&["RULE-STAM-REGEN"], TEXT, &[]);
    let mut priced = report();
    priced.intake.price_of_other = Some(PRICE.to_owned());
    cases.push((priced, "a discrepancy holds no `price_of_other`".to_owned()));
    let mut ungapped = report();
    ungapped.intake.gap_type = None;
    cases.push((ungapped, "a discrepancy lacks its `gap_type`".to_owned()));
    let mut unrecommended = report();
    unrecommended.intake.recommendation = None;
    cases.push((
        unrecommended,
        "a discrepancy lacks its `recommendation`".to_owned(),
    ));
    let mut outside = report();
    outside.intake.recommendation = Some(5);
    cases.push((
        outside,
        "the recommendation 5 of a discrepancy is no index into its 2 options".to_owned(),
    ));
    for (item, want) in &cases {
        for patched in [None, Some(patch("RULE-STAM-REGEN"))] {
            let message = invalid(queue.create_intake(item, &[], patched.as_ref(), T0), want);
            assert_eq!(&message, want);
            assert_eq!(queue.dump().expect("dump"), dump, "{want}: nothing written");
        }
    }
    assert!(queue.events().expect("events").is_empty(), "no event");
    // Empty lists are none: the item of its shape is stored, no ID taken
    // before it.
    let mut empty = decision(&["RULE-STAM-REGEN"], TEXT, &[]);
    empty.intake.evidence = Vec::new();
    empty.intake.options = Vec::new();
    let stored = queue
        .create_intake(&empty, &[], Some(&patch("RULE-STAM-REGEN")), T0)
        .expect("intake")
        .created
        .expect("stored");
    assert_eq!(stored.id, "PR-0001");
    let mut found = discrepancy(&["EDGE-STAM-ZERO"], TEXT, &[]);
    found.intake.working_answer = Some("code".to_owned());
    let stored = queue
        .create_intake(&found, &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    assert_eq!(
        stored.id, "PR-0003",
        "a discrepancy's working answer is optional"
    );
}
