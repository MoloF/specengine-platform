//! docs/features/plain-questions.md (RM-0088, the question half)
//! "Acceptance criteria", the store's half: `SqliteQueue` through the
//! public API; the raw steps (an older schema, a hand-edited row) through
//! `restore` and the system's `sqlite3`.
//!
//! - PQ5: `create_intake` refuses (`Invalid`, nothing written, no event, no
//!   ID taken) a decision holding `options`, `gist` or `matters` and a
//!   discrepancy holding `gist` or `matters`; a question holding all four
//!   is stored and reads back byte-equal. A hand-edited row reads corrupt
//!   with its column named: a decision with `options`, a discrepancy with
//!   `matters`, a question's `recommendation` without options, a
//!   question's `choice {"option":5}` over two options. A question and a
//!   decision with equal text on a shared target still hit each other. A
//!   question with options takes `Choice::Option` in range, its working
//!   answer and an answer; a decision still takes no option. M: one shape
//!   for a question and a decision (a decision's options stored); a dedup
//!   family per shape (the question and the decision no longer hit).
//! - PQ8, the store part: a new queue stands at `user_version` 13,
//!   `PRAGMA table_info(proposals)` equal to `PROPOSAL_COLUMNS`, 50, `gist`
//!   and `matters` last; `proposal_columns` 48 for 12 and 50 for 13, `None`
//!   for 14; a schema-12 queue with rows of every deciding kind steps at its
//!   first open, every row kept, both columns `NULL`; `open_existing` on 12
//!   reads both `None` and steps nothing; schema 14 is `SchemaTooNew`.
//!
//! The names the tests add (a lighthouse's lamp, its oil, a keeper's rest)
//! are invented here. Every expected value is worked out from the spec,
//! never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_core::record::Choice;
use specengine_store::{
    Decision, Intake, NewIntake, PROPOSAL_COLUMNS, Place, ProposalFilter, ProposalKind,
    ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION, QueueError, RecordApproval,
    RecordSeries, SqliteQueue, Stage, StoredQueue, proposal_columns,
};

const T0: &str = "2026-11-12T09:00:00Z";
const T1: &str = "2026-11-12T09:30:00Z";
const PROJECT: &str = "lantern-keep";
const LAMP: &str = "docs/spec/lamp.md";

/// The item's text, as asked.
const TEXT: &str = "Does LAMP-OIL's refill wait for KEEP-REST when lamp_refill() runs at dusk?";
const WORKING: &str = "Yes, the refill waits for the keeper's rest.";
const PRICE: &str = "Two dusk scenes re-timed.";
const GIST: &str = "Should the lamp wait for the keeper to rest before it is refilled?";
const MATTERS: &str = "The dusk scenes and one task wait on it.";

fn sqlite3(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn user_version(db: &Path) -> String {
    sqlite3(db, "PRAGMA user_version;")
}

/// `proposals`' columns as `PRAGMA table_info` lists them, in order.
fn proposal_table(db: &Path) -> Vec<String> {
    sqlite3(
        db,
        "SELECT name FROM pragma_table_info('proposals') ORDER BY cid;",
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

fn column_at(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "main".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn agent() -> Author {
    Author::new(Some("developer".to_owned()), None, None).expect("an agent")
}

fn option(label: &str) -> IntakeOption {
    IntakeOption {
        label: label.to_owned(),
        effect: format!("{label}: the lamp changes"),
        price: "one scene".to_owned(),
    }
}

/// An item of a question's fields on `LAMP-OIL`: a question or a decision,
/// none of the four new fields.
fn answered(kind: ProposalKind, text: &str) -> NewIntake {
    NewIntake {
        kind,
        target_path: LAMP.to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: vec!["LAMP-OIL".to_owned(), "KEEP-REST".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some(WORKING.to_owned()),
            price_of_other: Some(PRICE.to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        card_id: None,
        actor: None,
    }
}

/// A question holding all four fields: two options, the first recommended.
fn plain_question(text: &str) -> NewIntake {
    let mut item = answered(ProposalKind::Question, text);
    item.intake.gist = Some(GIST.to_owned());
    item.intake.matters = Some(MATTERS.to_owned());
    item.intake.options = vec![option("Wait for rest"), option("Refill at once")];
    item.intake.recommendation = Some(0);
    item
}

/// A discrepancy on `LAMP-OIL`: its evidence, two options, the second
/// recommended.
fn discrepancy(summary: &str) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: LAMP.to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: vec!["LAMP-OIL".to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "src/lamp.rs".to_owned(),
                qpath: Some("lamp::refill".to_owned()),
                lines: Some("4-9".to_owned()),
                observed: "refills at once".to_owned(),
                documented: "refills after rest".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(1),
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        card_id: None,
        actor: None,
    }
}

fn invalid(result: Result<impl std::fmt::Debug, QueueError>, context: &str) -> String {
    match result {
        Err(QueueError::Invalid(reason)) => reason,
        other => panic!("{context}: expected Invalid, got {other:?}"),
    }
}

fn stored(queue: &mut SqliteQueue, item: &NewIntake) -> String {
    let result = queue
        .create_intake(item, &[], None, T0)
        .unwrap_or_else(|error| panic!("create_intake: {error}"));
    match result.created {
        Some(proposal) => proposal.id,
        None => panic!("not stored: hits {:?}", result.hits),
    }
}

fn owner() -> Decision {
    Decision {
        decided_by: "Ann Owner <ann@example.org>".to_owned(),
        note: None,
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
        title: "Refill at once".to_owned(),
        text: "---\nid: DEC-0024\n---\n".to_owned(),
        choice,
    }
}

/// The rows of a queue holding `item` alone, as `PR-0001`.
fn rows_of(scratch: &Scratch, name: &str, item: &NewIntake) -> StoredQueue {
    let mut queue = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
    assert_eq!(stored(&mut queue, item), "PR-0001", "{name}");
    queue.stored_rows().expect("rows")
}

/// The rows of a queue holding `item` alone as `PR-0001`, approved into
/// its record with `choice`.
fn decided_rows(scratch: &Scratch, name: &str, item: &NewIntake, choice: Choice) -> StoredQueue {
    let mut queue = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
    assert_eq!(stored(&mut queue, item), "PR-0001", "{name}");
    let open = queue.get("PR-0001").unwrap().unwrap();
    queue
        .approve_record_from("PR-0001", &open.seen(), &approval(choice), &owner(), T1)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    queue.stored_rows().expect("rows")
}

// ------------------------------------------------------------------ PQ5

/// PQ5: a decision with `options`, `gist` or `matters`, a discrepancy with
/// `gist` or `matters`: `Invalid` naming the field, the dump unchanged, no
/// event, no ID taken. M: one shape for a question and a decision (the
/// decision's options and gist stored).
#[test]
fn pq5_a_decision_or_a_discrepancy_holding_the_new_fields_is_invalid() {
    let scratch = Scratch::new("pq-pq5-invalid");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let dump = queue.dump().expect("dump");
    let decision = || answered(ProposalKind::Decision, TEXT);
    let mut cases: Vec<(NewIntake, &str)> = Vec::new();
    let mut priced = decision();
    priced.intake.options = vec![option("Wait for rest"), option("Refill at once")];
    cases.push((priced, "a decision holds no `options`"));
    let mut gist = decision();
    gist.intake.gist = Some(GIST.to_owned());
    cases.push((gist, "a decision holds no `gist`"));
    let mut matters = decision();
    matters.intake.matters = Some(MATTERS.to_owned());
    cases.push((matters, "a decision holds no `matters`"));
    let mut recommended = decision();
    recommended.intake.recommendation = Some(0);
    cases.push((recommended, "a decision holds no `recommendation`"));
    let mut found = discrepancy(TEXT);
    found.intake.gist = Some(GIST.to_owned());
    cases.push((found, "a discrepancy holds no `gist`"));
    let mut why = discrepancy(TEXT);
    why.intake.matters = Some(MATTERS.to_owned());
    cases.push((why, "a discrepancy holds no `matters`"));
    for (item, want) in &cases {
        let message = invalid(queue.create_intake(item, &[], None, T0), want);
        assert_eq!(&message, want);
        assert_eq!(queue.dump().expect("dump"), dump, "{want}: nothing written");
    }
    assert!(queue.events().expect("events").is_empty(), "no event");
    // No ID was taken: the next item is PR-0001.
    assert_eq!(stored(&mut queue, &decision()), "PR-0001");
}

/// PQ5, PQ1's store half: a question holding all four fields is stored and
/// reads back byte-equal (`get`, `list`); its columns hold the text raw and
/// the options as JSON; a question without them stores the four `NULL`.
#[test]
fn pq5_a_question_holds_the_four_fields_and_reads_them_back() {
    let scratch = Scratch::new("pq-pq5-question");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let item = plain_question(TEXT);
    let id = stored(&mut queue, &item);
    assert_eq!(id, "PR-0001");
    let read = queue.get(&id).expect("read").expect("the row");
    assert_eq!(read.kind, ProposalKind::Question);
    assert_eq!(read.intake.as_ref(), Some(&item.intake));
    let rows = queue.stored_rows().expect("rows");
    let row = &rows.proposals[0].columns;
    assert_eq!(row[column_at("gist")].as_deref(), Some(GIST));
    assert_eq!(row[column_at("matters")].as_deref(), Some(MATTERS));
    assert_eq!(
        row[column_at("options")].as_deref(),
        Some(
            "[{\"label\":\"Wait for rest\",\"effect\":\"Wait for rest: the lamp changes\",\
             \"price\":\"one scene\"},{\"label\":\"Refill at once\",\"effect\":\"Refill at \
             once: the lamp changes\",\"price\":\"one scene\"}]"
        )
    );
    assert_eq!(row[column_at("recommendation")].as_deref(), Some("0"));
    assert_eq!(row[column_at("evidence")], None, "a question holds none");
    assert_eq!(row[column_at("gap_type")], None);

    let bare = answered(ProposalKind::Question, "Is the lamp lit at dawn?");
    let bare_id = stored(&mut queue, &bare);
    let rows = queue.stored_rows().expect("rows");
    let row = &rows
        .proposals
        .iter()
        .find(|row| row.id() == Some(bare_id.as_str()))
        .expect("the bare row")
        .columns;
    for column in ["gist", "matters", "options", "recommendation"] {
        assert_eq!(row[column_at(column)], None, "{column} NULL");
    }
    assert_eq!(
        queue.get(&bare_id).unwrap().unwrap().intake.as_ref(),
        Some(&bare.intake)
    );
}

/// PQ5: hand-edited rows read corrupt with the column named, `list`
/// failing and `list_readable` naming the row: a decision with `options`,
/// a discrepancy with `matters`, a question's `recommendation` without
/// options (the spec's line), a question's `choice {"option":5}` over two
/// options. The rows as stored read.
#[test]
fn pq5_a_hand_edited_row_reads_corrupt_naming_its_column() {
    let scratch = Scratch::new("pq-pq5-rows");
    let options = "[{\"label\":\"a\",\"effect\":\"b\",\"price\":\"c\"},{\"label\":\"d\",\
                   \"effect\":\"e\",\"price\":\"f\"}]";
    let decision = rows_of(
        &scratch,
        "decision",
        &answered(ProposalKind::Decision, TEXT),
    );
    let report = rows_of(&scratch, "discrepancy", &discrepancy(TEXT));
    let asked = rows_of(&scratch, "question", &plain_question(TEXT));
    let bare = rows_of(
        &scratch,
        "bare",
        &answered(ProposalKind::Question, "Is the lamp lit at dawn?"),
    );
    let chose = decided_rows(&scratch, "chose", &plain_question(TEXT), Choice::Option(1));
    let took = decided_rows(
        &scratch,
        "took",
        &answered(ProposalKind::Question, "Is the lamp lit at dawn?"),
        Choice::WorkingAnswer,
    );
    let set = |rows: &StoredQueue, edits: &[(&str, Option<&str>)]| {
        let mut rows = rows.clone();
        rows.events.clear();
        for (column, value) in edits {
            rows.proposals[0].columns[column_at(column)] = value.map(str::to_owned);
        }
        rows
    };
    let held = |kind: &str| format!("it is set on {kind}, which holds none");
    let cases: Vec<(&str, StoredQueue, &str, Option<String>)> = vec![
        (
            "a decision with options",
            set(&decision, &[("options", Some(options))]),
            "options",
            Some(held("a decision")),
        ),
        (
            "a decision with a gist",
            set(&decision, &[("gist", Some("A line."))]),
            "gist",
            Some(held("a decision")),
        ),
        (
            "a discrepancy with matters",
            set(&report, &[("matters", Some("It matters."))]),
            "matters",
            Some(held("a discrepancy")),
        ),
        (
            "a question's recommendation without options",
            set(&bare, &[("recommendation", Some("0"))]),
            "recommendation",
            Some("it is set without options".to_owned()),
        ),
        (
            "a question's recommendation out of its options",
            set(&asked, &[("recommendation", Some("2"))]),
            "recommendation",
            None,
        ),
        (
            "a question's options not JSON",
            set(&asked, &[("options", Some("[{"))]),
            "options",
            None,
        ),
        (
            "a question's choice {\"option\":5} over two options",
            set(&chose, &[("choice", Some("{\"option\":5}"))]),
            "choice",
            None,
        ),
        (
            "an option-less question's choice {\"option\":0}",
            set(&took, &[("choice", Some("{\"option\":0}"))]),
            "choice",
            None,
        ),
    ];
    for (index, (label, state, column, reason)) in cases.into_iter().enumerate() {
        let db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&db, PROJECT).expect("open");
        broken.restore(&state).expect("restore takes rows raw");
        let message = match broken.get("PR-0001") {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        let head = format!("proposal PR-0001: the stored `{column}` cannot be read: ");
        assert!(message.contains(&head), "{label}: {message}");
        if let Some(reason) = reason {
            assert!(
                message.ends_with(&format!("{head}{reason}")),
                "{label}: {message}"
            );
        }
        assert!(
            broken.list(&ProposalFilter::default()).is_err(),
            "{label}: list fails"
        );
        let listed = broken
            .list_readable(&ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].column, column, "{label}");
    }
    // The rows as stored read, a question's choice of an option in range
    // too.
    for name in [
        "decision",
        "discrepancy",
        "question",
        "bare",
        "chose",
        "took",
    ] {
        let queue = SqliteQueue::open(scratch.db(name), PROJECT).expect("open");
        assert!(queue.get("PR-0001").unwrap().is_some(), "{name}");
    }
    assert_eq!(
        chose.proposals[0].columns[column_at("choice")].as_deref(),
        Some("{\"option\":1}")
    );
}

/// PQ5: a question and a decision with equal text on a shared target hit
/// each other both ways (one family, `Asks`); a discrepancy with that text
/// hits neither (`Reports`). M: a dedup family per shape.
#[test]
fn pq5_a_question_and_a_decision_with_equal_text_still_hit() {
    let scratch = Scratch::new("pq-pq5-dedup");
    let ids = |result: &specengine_store::IntakeResult| -> Vec<String> {
        result.hits.iter().map(|hit| hit.id.clone()).collect()
    };
    // A plain question first, then a decision with the same text.
    let mut queue = SqliteQueue::open(scratch.db("a"), PROJECT).expect("open");
    assert_eq!(stored(&mut queue, &plain_question(TEXT)), "PR-0001");
    let decided = queue
        .create_intake(&answered(ProposalKind::Decision, TEXT), &[], None, T0)
        .expect("intake");
    assert!(decided.created.is_none(), "the decision is not stored");
    assert_eq!(ids(&decided), ["PR-0001"]);
    let reported = queue
        .create_intake(&discrepancy(TEXT), &[], None, T0)
        .expect("intake");
    assert!(reported.hits.is_empty(), "a discrepancy is another family");
    assert!(reported.created.is_some());
    // A decision first, then the plain question; text differing only in
    // case and whitespace hits too, the gist not compared.
    let mut queue = SqliteQueue::open(scratch.db("b"), PROJECT).expect("open");
    assert_eq!(
        stored(&mut queue, &answered(ProposalKind::Decision, TEXT)),
        "PR-0001"
    );
    let spaced = TEXT.to_uppercase().replace(' ', "  ");
    let mut asked = plain_question(&spaced);
    asked.intake.gist = Some("Another gist altogether.".to_owned());
    let result = queue.create_intake(&asked, &[], None, T0).expect("intake");
    assert!(result.created.is_none(), "the question is not stored");
    assert_eq!(ids(&result), ["PR-0001"]);
    // Two questions differing only in the gist hit.
    let mut queue = SqliteQueue::open(scratch.db("c"), PROJECT).expect("open");
    assert_eq!(stored(&mut queue, &plain_question(TEXT)), "PR-0001");
    let mut other = plain_question(TEXT);
    other.intake.gist = Some("The same, put another way.".to_owned());
    let result = queue.create_intake(&other, &[], None, T0).expect("intake");
    assert_eq!(ids(&result), ["PR-0001"]);
}

/// PQ4's store half: a question with options takes `Choice::Option` in
/// range (stored `{"option":1}`), and out of range is `Invalid`; a decision
/// still takes no option. M: `--option` on a question ignored.
#[test]
fn pq5_a_question_with_options_takes_an_option_a_decision_none() {
    let scratch = Scratch::new("pq-pq5-choice");
    for (index, choice, stored_as) in [
        (0, Choice::Option(1), "{\"option\":1}"),
        (1, Choice::WorkingAnswer, "{\"working_answer\":true}"),
        (
            2,
            Choice::Answer("At dusk.".to_owned()),
            "{\"answer\":\"At dusk.\"}",
        ),
    ] {
        let mut queue = SqliteQueue::open(scratch.db(&format!("q{index}")), PROJECT).expect("open");
        assert_eq!(stored(&mut queue, &plain_question(TEXT)), "PR-0001");
        let open = queue.get("PR-0001").unwrap().unwrap();
        let dump = queue.dump().expect("dump");
        invalid(
            queue.approve_record_from(
                "PR-0001",
                &open.seen(),
                &approval(Choice::Option(2)),
                &owner(),
                T1,
            ),
            "option 2 of two",
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
            .unwrap_or_else(|error| panic!("{choice:?}: {error}"));
        assert_eq!(approved.status, ProposalStatus::Approved);
        assert_eq!(approved.record.as_ref().expect("a record").choice, choice);
        let rows = queue.stored_rows().expect("rows");
        assert_eq!(
            rows.proposals[0].columns[column_at("choice")].as_deref(),
            Some(stored_as)
        );
    }
    let mut queue = SqliteQueue::open(scratch.db("decision"), PROJECT).expect("open");
    assert_eq!(
        stored(&mut queue, &answered(ProposalKind::Decision, TEXT)),
        "PR-0001"
    );
    let open = queue.get("PR-0001").unwrap().unwrap();
    invalid(
        queue.approve_record_from(
            "PR-0001",
            &open.seen(),
            &approval(Choice::Option(0)),
            &owner(),
            T1,
        ),
        "an option on a decision",
    );
}

// ------------------------------------------------------------------ PQ8

/// PQ8: a new queue stands at 13; `PRAGMA table_info(proposals)` is
/// `PROPOSAL_COLUMNS`, 50, `gist` and `matters` last and `TEXT`;
/// `proposal_columns` 48 for 12, 50 for 13, `None` for 14.
#[test]
fn pq8_a_new_queue_stands_at_13_with_50_proposal_columns() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 14);
    let scratch = Scratch::new("pq-pq8-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "14");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS);
    assert_eq!(PROPOSAL_COLUMNS.len(), 50);
    assert_eq!(PROPOSAL_COLUMNS[48..], ["gist", "matters"]);
    assert_eq!(PROPOSAL_COLUMNS[46..48], ["card_id", "why"]);
    for column in ["gist", "matters"] {
        assert_eq!(
            sqlite3(
                &db,
                &format!(
                    "SELECT type FROM pragma_table_info('proposals') WHERE name = '{column}';"
                )
            ),
            "TEXT",
            "{column}"
        );
    }
    assert_eq!(proposal_columns(12), Some(&PROPOSAL_COLUMNS[..48]));
    assert_eq!(proposal_columns(11), Some(&PROPOSAL_COLUMNS[..48]));
    assert_eq!(proposal_columns(13), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(14), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(15), None);
}

/// A schema-12 queue: one item of each deciding kind (a plain question, a
/// decision, a discrepancy) stored by this build, `gist` and `matters`
/// dropped (the plain question's set back first: a schema-12 row holds
/// none), `user_version` 12.
fn schema_12(db: &Path) {
    let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
    stored(&mut queue, &plain_question(TEXT));
    stored(
        &mut queue,
        &answered(ProposalKind::Decision, "Is the lamp lit at dawn?"),
    );
    stored(&mut queue, &discrepancy("The lamp refills at once."));
    drop(queue);
    sqlite3(
        db,
        "UPDATE proposals SET gist = NULL, matters = NULL; ALTER TABLE proposals DROP COLUMN \
         matters; ALTER TABLE proposals DROP COLUMN gist; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 12;",
    );
}

/// Every table's rows, `proposals` by its first 48 columns.
fn rows_of_12(db: &Path) -> String {
    let columns = PROPOSAL_COLUMNS[..48].join(", ");
    sqlite3(
        db,
        &format!(
            "SELECT {columns} FROM proposals ORDER BY id; SELECT * FROM cards; SELECT * FROM \
             tasks; SELECT * FROM runs; SELECT * FROM events ORDER BY seq;"
        ),
    )
}

/// PQ8: a schema-12 queue with rows of every deciding kind steps to 13 at
/// its first open: `proposals` gains `gist` and `matters` last, `NULL` on
/// every row; every other row and column as it was; each row reads; a
/// second open steps nothing. M: the step dropping a row.
#[test]
fn pq8_a_schema_12_queue_with_rows_steps_at_its_first_open() {
    let scratch = Scratch::new("pq-pq8-step");
    let db = scratch.db("q");
    schema_12(&db);
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS[..48], "premise");
    let before = rows_of_12(&db);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), "3");

    let queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    assert_eq!(user_version(&db), "14");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS);
    assert_eq!(rows_of_12(&db), before, "every row as it was");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE gist IS NULL AND matters IS NULL;"
        ),
        "3"
    );
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        let read = queue.get(id).expect("read").expect("the row");
        let intake = read.intake.expect("an intake");
        assert_eq!((intake.gist, intake.matters), (None, None), "{id}");
    }
    // The question's options and recommendation (schema 12 had the
    // columns) read as stored.
    let question = queue.get("PR-0001").unwrap().unwrap().intake.unwrap();
    assert_eq!(question.options.len(), 2);
    assert_eq!(question.recommendation, Some(0));
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "14", "stepped once");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS, "no second column");
}

/// PQ8: a hand-made `gist` fails step 13: the schema-12 file stays at 12,
/// nothing added or lost.
#[test]
fn pq8_a_hand_made_gist_column_fails_step_13() {
    let scratch = Scratch::new("pq-pq8-fail");
    let db = scratch.db("q");
    schema_12(&db);
    sqlite3(&db, "ALTER TABLE proposals ADD COLUMN gist TEXT;");
    let columns = proposal_table(&db);
    if let Ok(queue) = SqliteQueue::open(&db, PROJECT) {
        panic!("step 13 ran over a hand-made gist: {:?}", queue.dump());
    }
    assert_eq!(user_version(&db), "12");
    assert_eq!(proposal_table(&db), columns, "nothing added or lost");
}

/// PQ8: `open_existing` on a schema-12 queue reads `gist` and `matters`
/// `None` and steps nothing.
#[test]
fn pq8_open_existing_reads_a_12_queue_and_steps_nothing() {
    let scratch = Scratch::new("pq-pq8-existing");
    let db = scratch.db("q");
    schema_12(&db);
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("a queue");
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        let intake = queue.get(id).unwrap().unwrap().intake.unwrap();
        assert_eq!((intake.gist, intake.matters), (None, None), "{id}");
    }
    let rows = queue.stored_rows().expect("rows");
    for row in &rows.proposals {
        assert_eq!(row.columns[column_at("gist")], None);
        assert_eq!(row.columns[column_at("matters")], None);
    }
    drop(queue);
    assert_eq!(user_version(&db), "12", "nothing stepped");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS[..48]);
}

/// PQ8: a newer schema (15 since docs/features/collision-pass.md D6, this
/// build's 14; 14 over 13 before it) is `SchemaTooNew` for `open` and
/// `open_existing`, the file left alone. The path an older build meets at
/// 13 (a build set to 12): recorded with the HEAD-of-slice binary in this
/// slice's report.
#[test]
fn pq8_a_newer_schema_is_refused() {
    let scratch = Scratch::new("pq-pq8-newer");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    let newer = QUEUE_SCHEMA_VERSION + 1;
    assert_eq!(newer, 15);
    sqlite3(&db, &format!("PRAGMA user_version = {newer};"));
    for result in [
        SqliteQueue::open(&db, PROJECT).map(|_| ()),
        SqliteQueue::open_existing(&db, PROJECT).map(|_| ()),
    ] {
        match result {
            Err(QueueError::SchemaTooNew { found }) => assert_eq!(found, newer),
            other => panic!("expected SchemaTooNew {newer}, got {other:?}"),
        }
    }
    assert_eq!(user_version(&db), "15", "left alone");
}

// --------------------------------------------------------- D3, the stage

fn approve_option(option: u64) -> Stage {
    Stage::Approve {
        option: Some(option),
        answer: None,
        canon: None,
        editorial: false,
        note: None,
        span_hash: None,
    }
}

/// D3 ("Approve, the stage, the form, the round and the UI choose answers
/// by the item's fields, never its kind"), the store's stage: an option on
/// a question holding none is no stage (`Invalid` naming `option`, nothing
/// written), as on a decision; on a question holding two, option 1 stages.
/// Moved here from `queue_stage.rs`, where a question took no option by its
/// kind.
#[test]
fn pq5_an_option_on_a_question_without_options_is_no_stage() {
    let scratch = Scratch::new("pq-d3-stage");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let bare = stored(
        &mut queue,
        &answered(ProposalKind::Question, "Is the lamp lit at dawn?"),
    );
    let priced = stored(&mut queue, &plain_question(TEXT));
    let staged = |queue: &SqliteQueue, id: &str| {
        queue
            .stored_rows()
            .expect("rows")
            .proposals
            .iter()
            .find(|row| row.id() == Some(id))
            .expect("the row")
            .columns[column_at("staged")]
        .clone()
    };
    let seen = queue.get(&priced).unwrap().unwrap().seen();
    queue
        .stage_from(&priced, &seen, &approve_option(1), T1)
        .expect("option 1 of two stages");
    assert!(staged(&queue, &priced).is_some());
    let seen = queue.get(&bare).unwrap().unwrap().seen();
    let message = invalid(
        queue.stage_from(&bare, &seen, &approve_option(0), T1),
        "an option on a question holding none",
    );
    assert!(
        message.contains(&bare)
            && message.contains("a staged `option` on a question, which holds no options"),
        "{message}"
    );
    assert_eq!(staged(&queue, &bare), None, "nothing written");
    // Out of range on a question holding two (iteration 3's line).
    let seen = queue.get(&priced).unwrap().unwrap().seen();
    let message = invalid(
        queue.stage_from(&priced, &seen, &approve_option(5), T1),
        "option 5 of two",
    );
    assert!(
        message.contains("a staged `option` 5 on a question, which holds options 0-1"),
        "{message}"
    );
}

/// D3, the stored stage: a question holding no options whose `staged`
/// names an option reads corrupt, `staged` named, as a decision's would;
/// on a question holding two, option 1 reads.
#[test]
fn pq5_a_stored_option_stage_on_a_question_without_options_is_corrupt() {
    let scratch = Scratch::new("pq-d3-staged");
    let stage = "{\"decision\":\"approve\",\"option\":0,\"answer\":null,\"canon\":null,\
                 \"editorial\":null,\"note\":null,\"span_hash\":null}";
    for (name, item, readable) in [
        (
            "bare",
            answered(ProposalKind::Question, "Is the lamp lit at dawn?"),
            false,
        ),
        ("priced", plain_question(TEXT), true),
        ("decision", answered(ProposalKind::Decision, TEXT), false),
    ] {
        let mut rows = rows_of(&scratch, name, &item);
        rows.events.clear();
        rows.proposals[0].columns[column_at("staged")] = Some(stage.to_owned());
        rows.proposals[0].columns[column_at("staged_at")] = Some(T1.to_owned());
        let mut broken =
            SqliteQueue::open(scratch.db(&format!("{name}-staged")), PROJECT).expect("open");
        broken.restore(&rows).expect("restore takes rows raw");
        match (broken.get("PR-0001"), readable) {
            (Ok(Some(_)), true) => {}
            (Err(error), false) => {
                let message = error.to_string();
                assert!(
                    message.contains("the stored `staged` cannot be read"),
                    "{name}: {message}"
                );
                if name == "bare" {
                    assert!(
                        message
                            .ends_with("a staged `option` on a question, which holds no options"),
                        "{name}: {message}"
                    );
                }
            }
            (other, _) => panic!("{name}: {other:?}"),
        }
    }
}

/// Iteration 2: a staged `option` and `answer` together on a question is no
/// stage (`Invalid`, nothing written), and a stored one reads corrupt, both
/// with the store's line.
#[test]
fn pq5_an_option_and_an_answer_staged_together_on_a_question_are_refused() {
    const LINE: &str = "a staged `option` and `answer` on a question, which takes one of them";
    let scratch = Scratch::new("pq-it2-both");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let id = stored(&mut queue, &plain_question(TEXT));
    let both = Stage::Approve {
        option: Some(1),
        answer: Some("At dusk.".to_owned()),
        canon: None,
        editorial: false,
        note: None,
        span_hash: None,
    };
    let seen = queue.get(&id).unwrap().unwrap().seen();
    let message = invalid(queue.stage_from(&id, &seen, &both, T1), "both");
    assert!(message.contains(LINE), "{message}");
    assert_eq!(
        queue.stored_rows().unwrap().proposals[0].columns[column_at("staged")],
        None,
        "nothing written"
    );
    let mut rows = rows_of(&scratch, "row", &plain_question(TEXT));
    rows.events.clear();
    rows.proposals[0].columns[column_at("staged")] = Some(
        "{\"decision\":\"approve\",\"option\":1,\"answer\":\"At dusk.\",\"canon\":null,\
         \"editorial\":null,\"note\":null,\"span_hash\":null}"
            .to_owned(),
    );
    rows.proposals[0].columns[column_at("staged_at")] = Some(T1.to_owned());
    let mut broken = SqliteQueue::open(scratch.db("broken"), PROJECT).expect("open");
    broken.restore(&rows).expect("restore takes rows raw");
    let message = broken.get("PR-0001").expect_err("corrupt").to_string();
    assert!(
        message.ends_with(&format!(
            "proposal PR-0001: the stored `staged` cannot be read: {LINE}"
        )),
        "{message}"
    );
}
