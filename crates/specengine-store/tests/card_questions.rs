//! docs/features/card-questions.md, the store half (RM-0097, ADR-0077):
//! `SqliteQueue` through the public API; the raw steps (an older schema, a
//! hand-made column, a corrupt row) use the system's `sqlite3`.
//!
//! - S1: a new queue stands at `user_version` 11, `proposals` holding
//!   `PROPOSAL_COLUMNS` (48, `card_id` and `why` last); `proposal_columns`
//!   48 for 11, 46 for 8 to 10, `None` for 12; a schema-10 queue (rows of
//!   four kinds, a task, a card) steps to 11 at its first open, every row as
//!   it was but the two new `NULL` columns; a schema-9 queue reaches 11 in
//!   one open; a schema-9 queue whose `proposals` already has a hand-made
//!   `card_id` fails at step 11 and stays at 9 with no `cards.tasks`;
//!   `open_existing` on a schema-10 queue reads its rows, both `None`, and
//!   steps nothing. M: each step committed alone; `why` before `card_id`.
//! - S2: `create_intake` naming a card no row holds is `CardRefused` (`no
//!   card C-0099`), nothing written, no ID taken, before the dedup; a card in
//!   any state is named; `actor` is a trailing `as` on `proposal.created`
//!   alone. M: the existence check after the insert; a closed card refused.
//! - S3: `Intake.why` on a question or a discrepancy is `Invalid`, on a
//!   decision stored as given; by SQL, a `why` on a question, a `card_id`
//!   on an update and `X-7` on a question are corrupt, named by "Data"'s
//!   lines (`get` fails, `list_readable` skips); a blank or over-long `why`
//!   from a hand edit reads as stored. M: `why` accepted on a question.
//! - K1, the store half: `card_items` lists the items naming one card by
//!   ID number, a row naming it that does not decode as unreadable, no
//!   other card's and no update; `item_history` is the items' last
//!   `proposal.*` events, oldest first, a linked update's absent.
//!
//! Every expected value below is worked out from the spec, never read back
//! from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{Scratch, blake3_hex, fixture};
use specengine_core::card_toml::CardConfig;
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_store::{
    CARD_COLUMNS, CardChange, Intake, NewCard, NewIntake, NewProposal, NewTask, PROPOSAL_COLUMNS,
    Place, ProposalKind, ProposalQueue as _, QUEUE_SCHEMA_VERSION, QueueError, SqliteQueue,
    card_columns, patch_hash, proposal_columns,
};

const T0: &str = "2026-11-11T09:00:00Z";
const T1: &str = "2026-11-11T09:00:01Z";
const PROJECT: &str = "studio-lamp";

/// The two columns step 11 appends, in this order.
const STEP_11_COLUMNS: [&str; 2] = ["card_id", "why"];

/// "Data"'s `why`.
const WHY: &str = "Two playtests read instant regeneration as a bug.";

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

/// `PRAGMA user_version`, as printed.
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

/// `cards`' columns, in order.
fn card_table(db: &Path) -> Vec<String> {
    sqlite3(
        db,
        "SELECT name FROM pragma_table_info('cards') ORDER BY cid;",
    )
    .lines()
    .map(str::to_owned)
    .collect()
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

/// A question of `text` on `targets`, for `card`.
fn question(targets: &[&str], text: &str, card: Option<&str>) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/spec/scn-storm.md".to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some("The harbour bell, as in chapter one.".to_owned()),
            price_of_other: Some("Two scenes rewritten.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
        },
        card_id: card.map(str::to_owned),
        actor: None,
    }
}

/// A decision of `text` on `SCN-STORM` with `why`, for `card`.
fn decision(text: &str, why: Option<&str>, card: Option<&str>) -> NewIntake {
    let mut intake = question(&["SCN-STORM"], text, card);
    intake.kind = ProposalKind::Decision;
    intake.intake.working_answer = Some("Yes: whale oil, as the keeper's log says.".to_owned());
    intake.intake.price_of_other = Some("The lamp room is redrawn.".to_owned());
    intake.intake.why = why.map(str::to_owned);
    intake
}

/// A discrepancy on `SCN-STORM` with two options, for `card`.
fn discrepancy(summary: &str, card: Option<&str>) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    let mut intake = question(&["SCN-STORM"], summary, card);
    intake.kind = ProposalKind::Discrepancy;
    intake.intake.severity = IntakeSeverity::High;
    intake.intake.gap_type = Some(GapType::Contradicts);
    intake.intake.working_answer = None;
    intake.intake.price_of_other = None;
    intake.intake.evidence = vec![Evidence {
        file: "docs/spec/scn-storm.md".to_owned(),
        qpath: None,
        lines: Some("12-14".to_owned()),
        observed: "the lamp lights first".to_owned(),
        documented: "the lamp lights last".to_owned(),
    }];
    intake.intake.options = vec![option("scene"), option("spec")];
    intake.intake.recommendation = Some(0);
    intake
}

fn update(target: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/spec/scn-storm.md".to_owned(),
        place: place(),
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Why.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    }
}

/// studio-lamp's flows, through core's reader.
fn lamp() -> CardConfig {
    let text = fs::read_to_string(fixture("studio-lamp").join("specengine.toml")).expect("config");
    CardConfig::from_toml(&text).expect("the fixture's [card]")
}

/// A narrative card on `SCN-STORM`, made by `local`.
fn new_card(config: &CardConfig, title: &str) -> NewCard {
    let flow = config.flow("narrative").expect("the narrative flow");
    NewCard {
        flow: flow.name.clone(),
        column: flow.first_column().to_owned(),
        title: title.to_owned(),
        goal: None,
        discipline: Some("narrative".to_owned()),
        targets: vec!["SCN-STORM".to_owned()],
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        mandate: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: None,
        by: "local".to_owned(),
    }
}

/// The stored ID of an intake that must be stored.
fn created(queue: &mut SqliteQueue, intake: &NewIntake, now: &str) -> String {
    let result = queue
        .create_intake(intake, &[], None, now)
        .unwrap_or_else(|error| panic!("create_intake: {error}"));
    match result.created {
        Some(proposal) => proposal.id,
        None => panic!("not stored: hits {:?}", result.hits),
    }
}

/// The raw payload of every `proposal.created`, in `seq` order.
fn created_payloads(db: &Path) -> Vec<String> {
    sqlite3(
        db,
        "SELECT payload FROM events WHERE type = 'proposal.created' ORDER BY seq;",
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// A queue with a task, a card, an update, a question and a discrepancy
/// (both for the card), and a decision with a `why`: its rows as `SELECT
/// *` of each table prints them.
fn filled(db: &Path) {
    let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_task(
            &NewTask {
                git_common_dir: "/r/.git".to_owned(),
                title: Some("Light the lamp".to_owned()),
                goal: None,
                targets: vec!["SCN-STORM".to_owned()],
                author: Author::human(),
            },
            T0,
        )
        .expect("a task");
    queue
        .create_card(&new_card(&config, "The storm scene"), T0)
        .expect("a card");
    queue.create(&update("SCN-STORM"), T0).expect("an update");
    created(
        &mut queue,
        &question(&["SCN-STORM"], "Which bell rings at dusk?", Some("C-0001")),
        T0,
    );
    created(
        &mut queue,
        &discrepancy("The storm scene lights the lamp too early.", Some("C-0001")),
        T0,
    );
    created(
        &mut queue,
        &decision("Does the lamp burn oil?", Some(WHY), None),
        T0,
    );
}

/// `filled`'s queue set back to schema 10: step 11's two columns dropped.
fn at_schema_10(db: &Path) {
    // docs/features/task-mandate.md M5: a schema-10 queue holds no
    // `cards.mandate` either.
    sqlite3(
        db,
        "ALTER TABLE proposals DROP COLUMN why; ALTER TABLE proposals DROP COLUMN card_id; \
         ALTER TABLE cards DROP COLUMN mandate; PRAGMA user_version = 10;",
    );
}

/// Every table's rows, `proposals` by its first 46 columns.
fn rows_of_10(db: &Path) -> String {
    let columns = proposal_columns(10)
        .expect("schema 10's columns")
        .join(", ");
    sqlite3(
        db,
        &format!(
            "SELECT {columns} FROM proposals ORDER BY id; SELECT * FROM tasks; SELECT * FROM \
             runs; SELECT * FROM events ORDER BY seq;"
        ),
    )
}

// ------------------------------------------------------------------ S1

/// S1: a new queue stands at 11; `PRAGMA table_info(proposals)` is
/// `PROPOSAL_COLUMNS`, 48, step 11's two last in this order;
/// `proposal_columns` gives 48 for 11, 46 for 8 to 10, `None` for 12;
/// `card_columns` 25 for 10 and 11. M: `why` added before `card_id`.
#[test]
fn s1_a_new_queue_stands_at_11_with_48_columns() {
    // 12 since docs/features/task-mandate.md M5, `proposals` unchanged.
    assert_eq!(QUEUE_SCHEMA_VERSION, 12);
    let scratch = Scratch::new("cq-s1-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "12");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS);
    assert_eq!(PROPOSAL_COLUMNS.len(), 48);
    assert_eq!(PROPOSAL_COLUMNS[46..], STEP_11_COLUMNS);
    assert_eq!(proposal_columns(11).map(<[_]>::len), Some(48));
    assert_eq!(proposal_columns(11), Some(&PROPOSAL_COLUMNS[..]));
    for schema in 8..=10 {
        assert_eq!(
            proposal_columns(schema),
            Some(&PROPOSAL_COLUMNS[..46]),
            "schema {schema}"
        );
    }
    // docs/features/task-mandate.md M5: 48 at 12 too, `None` from 13;
    // `cards` 26 at 12.
    assert_eq!(proposal_columns(12), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(13), None);
    for schema in 10..=11 {
        assert_eq!(
            card_columns(schema).map(<[_]>::len),
            Some(25),
            "schema {schema}"
        );
    }
    assert_eq!(card_columns(12).map(<[_]>::len), Some(26));
    assert_eq!(card_columns(13), None);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT type FROM pragma_table_info('proposals') WHERE name IN ('card_id', 'why') \
             ORDER BY cid;"
        ),
        "TEXT\nTEXT"
    );
}

/// S1: a schema-10 queue (rows of four kinds, a task, a card, their
/// events) steps to 11 at its first `SqliteQueue::open`: `proposals` gains
/// the two columns last, `NULL` on every row; every row as it was in its
/// first 46 columns, `tasks`, `runs`, `cards` and `events` unchanged; a
/// second open steps nothing.
#[test]
fn s1_a_schema_10_queue_steps_to_11_at_its_first_open() {
    let scratch = Scratch::new("cq-s1-step");
    let db = scratch.db("q");
    filled(&db);
    at_schema_10(&db);
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS[..46], "premise");
    let before = rows_of_10(&db);
    let cards = sqlite3(&db, "SELECT * FROM cards;");
    let card_25 = format!("SELECT {} FROM cards;", CARD_COLUMNS[..25].join(", "));

    let queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    // 12 since docs/features/task-mandate.md M5: steps 11 and 12.
    assert_eq!(user_version(&db), "12");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS);
    assert_eq!(rows_of_10(&db), before, "every row as it was");
    assert_eq!(sqlite3(&db, &card_25), cards);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE card_id IS NULL AND why IS NULL;"
        ),
        "4",
        "both NULL on every stepped row"
    );
    for id in ["PR-0001", "PR-0002", "PR-0003", "PR-0004"] {
        let proposal = queue.get(id).expect("read").expect("the row");
        assert_eq!(proposal.card_id, None, "{id}");
        if let Some(intake) = &proposal.intake {
            assert_eq!(intake.why, None, "{id}");
        }
    }
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "12", "stepped once");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS, "no second column");
}

/// S1: a schema-9 queue (no `cards.tasks`, no step-11 columns) reaches 11
/// in one open: `cards` 25 columns, `proposals` 48.
#[test]
fn s1_a_schema_9_queue_reaches_11_in_one_open() {
    let scratch = Scratch::new("cq-s1-nine");
    let db = scratch.db("q");
    filled(&db);
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN why; ALTER TABLE proposals DROP COLUMN card_id; \
         ALTER TABLE cards DROP COLUMN mandate; ALTER TABLE cards DROP COLUMN tasks; \
         PRAGMA user_version = 9;",
    );
    let before = rows_of_10(&db);
    drop(SqliteQueue::open(&db, PROJECT).expect("open steps"));
    // 12 and 26 since docs/features/task-mandate.md M5.
    assert_eq!(user_version(&db), "12");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS);
    assert_eq!(card_table(&db).len(), 26);
    assert_eq!(card_table(&db).last().map(String::as_str), Some("mandate"));
    assert_eq!(rows_of_10(&db), before);
}

/// S1: a schema-9 queue whose `proposals` already has a hand-made
/// `card_id` column: the open fails (step 11 cannot add it) and the file
/// stays at 9 with no `cards.tasks`: every due step in one transaction. M:
/// each step committed alone (this case then reads 10).
#[test]
fn s1_a_failing_step_11_leaves_a_schema_9_queue_at_9() {
    let scratch = Scratch::new("cq-s1-fail");
    let db = scratch.db("q");
    filled(&db);
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN why; ALTER TABLE cards DROP COLUMN mandate; \
         ALTER TABLE cards DROP COLUMN tasks; PRAGMA user_version = 9;",
    );
    assert!(
        proposal_table(&db).contains(&"card_id".to_owned()),
        "premise"
    );
    let tables = card_table(&db);
    match SqliteQueue::open(&db, PROJECT) {
        Err(_) => {}
        Ok(_) => panic!("step 11 ran over a hand-made card_id"),
    }
    assert_eq!(user_version(&db), "9");
    assert_eq!(card_table(&db), tables, "no cards.tasks");
    assert!(!card_table(&db).contains(&"tasks".to_owned()));
}

/// S1: `open_existing` on a schema-10 queue reads its proposals, `card_id`
/// and `why` both `None`, holds no card's items, and steps nothing.
#[test]
fn s1_open_existing_reads_a_schema_10_queue_and_steps_nothing() {
    let scratch = Scratch::new("cq-s1-existing");
    let db = scratch.db("q");
    filled(&db);
    at_schema_10(&db);
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file");
    for id in ["PR-0001", "PR-0002", "PR-0003", "PR-0004"] {
        let proposal = queue.get(id).expect("read").expect("the row");
        assert_eq!(proposal.card_id, None, "{id}");
        if let Some(intake) = &proposal.intake {
            assert_eq!(intake.why, None, "{id}");
        }
    }
    let stored = queue.stored_rows().expect("stored rows");
    assert_eq!(stored.proposals.len(), 4);
    for row in &stored.proposals {
        assert_eq!(row.columns[46..], [None, None], "{row:?}");
    }
    let items = queue.card_items("C-0001").expect("card_items");
    assert!(items.items.is_empty() && items.unreadable.is_empty());
    drop(queue);
    assert_eq!(user_version(&db), "10", "stepped nothing");
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS[..46]);
}

// ------------------------------------------------------------------ S2

/// S2, the store: a question naming `C-0099` (no such row) is
/// `CardRefused {id: "C-0099", reason: "no card C-0099"}`, the queue's dump
/// and the next ID as before, even when the same question would be a hit
/// (the check before the dedup); `C-0001` open, then cancelled, is named;
/// `actor` adds a trailing `as` to `proposal.created`, its absence leaves
/// `{"id":…}`. M: the existence check after the insert; a closed card
/// refused.
#[test]
fn s2_a_missing_card_is_card_refused_and_a_card_in_any_state_is_named() {
    let scratch = Scratch::new("cq-s2-store");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_card(&new_card(&config, "The storm scene"), T0)
        .expect("a card");
    let bell = question(&["SCN-STORM"], "Which bell rings at dusk?", Some("C-0001"));
    assert_eq!(created(&mut queue, &bell, T0), "PR-0001");
    let dump = queue.dump().expect("dump");

    let mut missing = bell.clone();
    missing.card_id = Some("C-0099".to_owned());
    match queue.create_intake(&missing, &[], None, T1) {
        Err(QueueError::CardRefused { id, reason }) => {
            assert_eq!(id, "C-0099");
            assert_eq!(reason, "no card C-0099");
        }
        other => panic!("expected CardRefused, got {other:?}"),
    }
    let mut other_text = missing.clone();
    other_text.intake.summary = "Who meets the keeper at the pier?".to_owned();
    assert!(
        matches!(
            queue.create_intake(&other_text, &[], None, T1),
            Err(QueueError::CardRefused { .. })
        ),
        "a new text alike"
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");

    let mut next = bell.clone();
    next.intake.summary = "Who meets the keeper at the pier?".to_owned();
    next.actor = Some("dev".to_owned());
    assert_eq!(created(&mut queue, &next, T1), "PR-0002", "no ID taken");
    let proposal = queue.get("PR-0002").expect("read").expect("the row");
    assert_eq!(proposal.card_id.as_deref(), Some("C-0001"));

    queue
        .change_card(
            "C-0001",
            None,
            &CardChange::Cancel {
                reason: "Cut.".to_owned(),
            },
            "local",
            T1,
        )
        .expect("cancelled");
    let mut late = bell.clone();
    late.intake.summary = "Is the cove reachable at high tide?".to_owned();
    assert_eq!(created(&mut queue, &late, T1), "PR-0003");
    assert_eq!(
        queue.get("PR-0003").unwrap().unwrap().card_id.as_deref(),
        Some("C-0001"),
        "a cancelled card is named"
    );
    drop(queue);
    assert_eq!(
        created_payloads(&db),
        [
            "{\"id\":\"PR-0001\"}",
            "{\"id\":\"PR-0002\",\"as\":\"dev\"}",
            "{\"id\":\"PR-0003\"}"
        ]
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE card_id = 'C-0099';"
        ),
        "0"
    );
}

// ------------------------------------------------------------------ S3

/// S3, the store: `Intake.why` on a question or a discrepancy is
/// `Invalid` (`a question holds no `why``), nothing written; on a decision
/// stored as given and read back.
#[test]
fn s3_why_is_a_decisions_alone() {
    let scratch = Scratch::new("cq-s3-store");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let mut asked = question(&["SCN-STORM"], "Which bell rings at dusk?", None);
    asked.intake.why = Some(WHY.to_owned());
    match queue.create_intake(&asked, &[], None, T0) {
        Err(QueueError::Invalid(message)) => {
            assert!(message.contains("a question holds no `why`"), "{message}");
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
    let mut reported = discrepancy("The storm scene lights the lamp too early.", None);
    reported.intake.why = Some(WHY.to_owned());
    assert!(
        matches!(
            queue.create_intake(&reported, &[], None, T0),
            Err(QueueError::Invalid(_))
        ),
        "a discrepancy holds none either"
    );
    assert_eq!(queue.dump().expect("dump"), "", "nothing written");

    let id = created(
        &mut queue,
        &decision("Does the lamp burn oil?", Some(WHY), None),
        T0,
    );
    let read = queue.get(&id).expect("read").expect("the row");
    assert_eq!(
        read.intake
            .as_ref()
            .and_then(|intake| intake.why.as_deref()),
        Some(WHY)
    );
    assert_eq!(read.card_id, None);
}

/// S3, S4, the store: by SQL, a `why` on a question, a `card_id` on an
/// update, `X-7` on a question: `get` fails with "Data"'s line,
/// `list_readable` lists the row as unreadable naming the column; a blank
/// and an over-long `why` on a decision read as stored.
#[test]
fn s3_a_column_its_kind_holds_none_of_is_corrupt_and_named() {
    let scratch = Scratch::new("cq-s3-corrupt");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue.create(&update("SCN-STORM"), T0).expect("an update");
    created(
        &mut queue,
        &question(&["SCN-STORM"], "Which bell rings at dusk?", None),
        T0,
    );
    created(
        &mut queue,
        &decision("Does the lamp burn oil?", Some(WHY), None),
        T0,
    );
    drop(queue);
    for (sql, id, column, line) in [
        (
            "UPDATE proposals SET why = 'Because.' WHERE id = 'PR-0002';",
            "PR-0002",
            "why",
            "proposal PR-0002: the stored `why` cannot be read: it is set on a question, which \
             holds none",
        ),
        (
            "UPDATE proposals SET card_id = 'C-0001' WHERE id = 'PR-0001';",
            "PR-0001",
            "card_id",
            "proposal PR-0001: the stored `card_id` cannot be read: it is set on an update, \
             which names no card",
        ),
        (
            "UPDATE proposals SET card_id = 'X-7' WHERE id = 'PR-0002';",
            "PR-0002",
            "card_id",
            "proposal PR-0002: the stored `card_id` cannot be read: `X-7` is no card ID",
        ),
    ] {
        sqlite3(&db, sql);
        let queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let message = match queue.get(id) {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{sql}: read as {found:?}"),
        };
        assert!(message.contains(line), "{sql}: {message}");
        let listed = queue
            .list_readable(&specengine_store::ProposalFilter::default())
            .expect("list_readable");
        assert_eq!(listed.unreadable.len(), 1, "{sql}");
        assert_eq!(listed.unreadable[0].id, id, "{sql}");
        assert_eq!(listed.unreadable[0].column, column, "{sql}");
        assert_eq!(listed.unreadable[0].to_string(), line, "{sql}");
        assert_eq!(listed.proposals.len(), 2, "{sql}");
        drop(queue);
        sqlite3(
            &db,
            "UPDATE proposals SET card_id = NULL; UPDATE proposals SET why = NULL WHERE id != \
             'PR-0003';",
        );
    }

    // A hand edit's blank or over-long `why` on a decision reads as stored.
    for why in [" ".to_owned(), "a".repeat(4097)] {
        sqlite3(
            &db,
            &format!("UPDATE proposals SET why = '{why}' WHERE id = 'PR-0003';"),
        );
        let queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let read = queue.get("PR-0003").expect("read").expect("the row");
        assert_eq!(
            read.intake.as_ref().and_then(|intake| intake.why.clone()),
            Some(why.clone())
        );
    }
}

// ------------------------------------------------------------------ K1

/// K1, the store: `card_items("C-0001")` lists its question, discrepancy
/// and decision by ID number, not `C-0002`'s item, not the update; a row
/// naming it that does not decode is unreadable, the others still listed.
/// `item_history` of its items: their last `limit` `proposal.*` events,
/// oldest first, no card's event and no other item's.
#[test]
fn k1_card_items_and_item_history_read_one_cards_items() {
    let scratch = Scratch::new("cq-k1-store");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    for title in ["The storm scene", "The arrival"] {
        queue
            .create_card(&new_card(&config, title), T0)
            .expect("a card");
    }
    queue.create(&update("SCN-STORM"), T0).expect("PR-0001");
    created(
        &mut queue,
        &question(&["SCN-STORM"], "Which bell rings at dusk?", Some("C-0001")),
        T0,
    );
    created(
        &mut queue,
        &question(
            &["SCN-ARRIVAL"],
            "Who meets the keeper at the pier?",
            Some("C-0002"),
        ),
        T0,
    );
    created(
        &mut queue,
        &discrepancy("The storm scene lights the lamp too early.", Some("C-0001")),
        T0,
    );
    created(
        &mut queue,
        &decision("Does the lamp burn oil?", Some(WHY), Some("C-0001")),
        T1,
    );
    let items = queue.card_items("C-0001").expect("card_items");
    let ids: Vec<&str> = items.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["PR-0002", "PR-0004", "PR-0005"]);
    assert!(items.unreadable.is_empty());
    assert_eq!(
        queue.card_items("C-0099").expect("card_items").items.len(),
        0
    );

    let history = queue
        .item_history(&["PR-0002", "PR-0004", "PR-0005"], 64)
        .expect("item_history");
    let seen: Vec<(String, String)> = history
        .iter()
        .map(|event| {
            (
                event.event_type.clone(),
                event.payload["id"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("proposal.created".to_owned(), "PR-0002".to_owned()),
            ("proposal.created".to_owned(), "PR-0004".to_owned()),
            ("proposal.created".to_owned(), "PR-0005".to_owned()),
        ]
    );
    let last = queue
        .item_history(&["PR-0002", "PR-0004", "PR-0005"], 2)
        .expect("item_history");
    let last: Vec<&str> = last
        .iter()
        .map(|event| event.payload["id"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(last, ["PR-0004", "PR-0005"], "the newest, oldest first");
    assert!(
        queue
            .item_history(&[], 64)
            .expect("item_history")
            .is_empty()
    );
    drop(queue);

    sqlite3(
        &db,
        "UPDATE proposals SET why = 'Because.' WHERE id = 'PR-0002';",
    );
    let queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let items = queue.card_items("C-0001").expect("card_items");
    let ids: Vec<&str> = items.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["PR-0004", "PR-0005"]);
    assert_eq!(items.unreadable.len(), 1);
    assert_eq!(items.unreadable[0].id, "PR-0002");
    assert_eq!(items.unreadable[0].column, "why");
}
