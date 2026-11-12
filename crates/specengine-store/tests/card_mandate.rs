//! docs/features/task-mandate.md (RM-0104, v5 step 6d, ADR-0079)
//! "Acceptance criteria", the store half: `SqliteQueue` through the public
//! API; the raw steps (an older schema, a hand-made column, a corrupt row)
//! use the system's `sqlite3`. The canon is
//! docs/canon/card-mandate.md "The rule".
//!
//! - M5 (the store part): a new queue stands at `user_version` 12,
//!   `table_info(cards)` equal to `CARD_COLUMNS`, 26, `mandate` last;
//!   `card_columns` 24, 25, 25, 26 for 9 to 12 and `None` for 13; a
//!   schema-11 queue with three cards steps at its first open, every row as
//!   it was but `mandate` `NULL`; schema 9 reaches 12 in one `migrate`; a
//!   hand-made `mandate` fails step 12 and the file stays at 11, and a
//!   schema-10 file stays at 10 with no step-11 column; `open_existing` on
//!   11 reads `mandate` `None` and steps nothing. M: step 12 committed apart
//!   from the other due steps.
//! - M1 (the store part): `create_card` with a mandate and no reviewer is
//!   `CardRefused` with the first line, no row, no event, no ID taken;
//!   `card.created` carries `mandate` always, `null` when none; a stored
//!   `mandate` off the two words makes the row corrupt, naming `mandate`.
//!   M: the store not asking `mandate_problem`; `card.created` carrying
//!   `mandate` only when set.
//! - M3 (the store part): an edit naming a mandate stands only for the
//!   stored reviewer, judged under the lock whatever else the edit holds; a
//!   refusal writes nothing, its other fields included; `card.edited` gains
//!   `mandate` only when `fields` hold it; `assign` leaves the mandate. M:
//!   the compare reading `assignee`; the check skipped when the edit holds
//!   another field; `card.edited` without `mandate`.
//! - M2 (the store part): a split's children store their own mandates and
//!   `card.created` carries each; a new parent holds none.
//! - M6 (the store part): `subtask_items` lists every item naming one of a
//!   card's children by ID number, each with its `card_id`; not the card's
//!   own, not a grandchild's; a cancelled child's listed; a corrupt row
//!   unreadable. M: `subtask_items` taking grandchildren.
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

use common::{Scratch, fixture};
use serde_json::{Value, json};
use specengine_core::card_toml::CardConfig;
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_model::{CardFrozen, Mandate};
use specengine_store::{
    CARD_COLUMNS, CardChange, CardEdit, EVENT_CARD_CREATED, EVENT_CARD_EDITED, Intake, NewCard,
    NewIntake, NewSplit, NewSplitChild, PROPOSAL_COLUMNS, Place, ProposalKind, ProposalQueue as _,
    QUEUE_SCHEMA_VERSION, QueueError, SplitParent, SqliteQueue, card_columns,
};

const T0: &str = "2026-11-12T09:00:00Z";
const T1: &str = "2026-11-12T09:00:01Z";
const PROJECT: &str = "studio-lamp";

/// The reviewer of the spec's probes.
const ANA: &str = "Ana Ruiz";

/// Another person.
const BEN: &str = "Ben Ode";

/// The first line of "Messages".
const NO_REVIEWER: &str =
    "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer";

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

/// `cards`' columns as `PRAGMA table_info` lists them, in order.
fn card_table(db: &Path) -> Vec<String> {
    sqlite3(
        db,
        "SELECT name FROM pragma_table_info('cards') ORDER BY cid;",
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `proposals`' columns, in order.
fn proposal_table(db: &Path) -> Vec<String> {
    sqlite3(
        db,
        "SELECT name FROM pragma_table_info('proposals') ORDER BY cid;",
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

/// An item of `kind` on `SCN-STORM` for `card`: a question, or a decision
/// with no `why` (a decision needs none, docs/canon/card-questions.md).
fn item(kind: ProposalKind, text: &str, card: &str) -> NewIntake {
    NewIntake {
        kind,
        target_path: "docs/spec/scn-storm.md".to_owned(),
        place: place(),
        author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: vec!["SCN-STORM".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some("A low alto, as the brief has it.".to_owned()),
            price_of_other: Some("Two scenes re-recorded.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        card_id: Some(card.to_owned()),
        actor: None,
    }
}

/// The stored ID of an item that must be stored.
fn raised(queue: &mut SqliteQueue, intake: &NewIntake) -> String {
    let result = queue
        .create_intake(intake, &[], None, T0)
        .unwrap_or_else(|error| panic!("create_intake: {error}"));
    match result.created {
        Some(proposal) => proposal.id,
        None => panic!("not stored: hits {:?}", result.hits),
    }
}

/// studio-lamp's flows, through core's reader.
fn lamp() -> CardConfig {
    let text = fs::read_to_string(fixture("studio-lamp").join("specengine.toml")).expect("config");
    CardConfig::from_toml(&text).expect("the fixture's [card]")
}

/// A card of `discipline`'s flow on `SCN-STORM`, made by Ana Ruiz: no
/// reviewer, no mandate, no parent.
fn new_card(config: &CardConfig, discipline: &str, title: &str) -> NewCard {
    let flow = config
        .flow(discipline)
        .unwrap_or_else(|| panic!("the {discipline} flow"));
    NewCard {
        flow: flow.name.clone(),
        column: flow.first_column().to_owned(),
        title: title.to_owned(),
        goal: None,
        discipline: Some(discipline.to_owned()),
        targets: vec!["SCN-STORM".to_owned()],
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        mandate: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: None,
        by: ANA.to_owned(),
    }
}

/// A hand-off of `discipline` under `parent`, reviewed by Ana Ruiz.
fn hand_off(
    config: &CardConfig,
    discipline: &str,
    title: &str,
    parent: &str,
    mandate: Option<Mandate>,
) -> NewCard {
    NewCard {
        parent: Some(parent.to_owned()),
        reviewer: Some(ANA.to_owned()),
        mandate,
        ..new_card(config, discipline, title)
    }
}

fn refusal(result: Result<impl std::fmt::Debug, QueueError>) -> String {
    match result {
        Err(QueueError::CardRefused { reason, .. }) => reason,
        other => panic!("expected CardRefused, got {other:?}"),
    }
}

/// `(type, payload)` of every `card.*` event, in `seq` order.
fn card_events(queue: &SqliteQueue) -> Vec<(String, Value)> {
    queue
        .events()
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type.starts_with("card."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

fn edit(edit: CardEdit) -> CardChange<'static> {
    CardChange::Edit(Box::new(edit))
}

fn mandate_edit(mandate: Mandate) -> CardChange<'static> {
    edit(CardEdit {
        mandate: Some(mandate),
        ..CardEdit::default()
    })
}

/// A queue with three cards, the second and third handed off under the
/// first with mandates, and two items naming the second: the rows of a
/// schema-12 queue.
fn three_cards(db: &Path) {
    let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_card(&new_card(&config, "narrative", "Lamp room scene"), T0)
        .expect("C-0001");
    queue
        .create_card(
            &hand_off(
                &config,
                "sound",
                "Keeper's voice",
                "C-0001",
                Some(Mandate::Open),
            ),
            T0,
        )
        .expect("C-0002");
    queue
        .create_card(
            &hand_off(
                &config,
                "art",
                "Keeper's portrait",
                "C-0001",
                Some(Mandate::Binding),
            ),
            T0,
        )
        .expect("C-0003");
    raised(
        &mut queue,
        &item(ProposalKind::Question, "Which voice?", "C-0002"),
    );
    raised(
        &mut queue,
        &item(ProposalKind::Decision, "A low alto.", "C-0002"),
    );
}

/// Every table's rows, `cards` by its first 25 columns.
fn rows_of_11(db: &Path) -> String {
    let columns = CARD_COLUMNS[..25].join(", ");
    // `proposals` by its first 48 (docs/features/plain-questions.md PQ8:
    // step 13 adds two).
    let proposals = PROPOSAL_COLUMNS[..48].join(", ");
    sqlite3(
        db,
        &format!(
            "SELECT {columns} FROM cards ORDER BY id; SELECT {proposals} FROM proposals ORDER BY \
             id; SELECT * FROM tasks; SELECT * FROM runs; SELECT * FROM events ORDER BY seq;"
        ),
    )
}

// ------------------------------------------------------------------ M5

/// M5: a new queue stands at 12; `PRAGMA table_info(cards)` is
/// `CARD_COLUMNS`, 26, `mandate` last and `TEXT`; `card_columns` 24, 25, 25
/// and 26 for 9 to 12, `None` for 13 and 8.
#[test]
fn m5_a_new_queue_stands_at_12_with_26_card_columns() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 14);
    let scratch = Scratch::new("cm-m5-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "14");
    assert_eq!(card_table(&db), CARD_COLUMNS);
    assert_eq!(CARD_COLUMNS.len(), 26);
    assert_eq!(CARD_COLUMNS[25], "mandate");
    assert_eq!(CARD_COLUMNS[24], "tasks");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT type FROM pragma_table_info('cards') WHERE name = 'mandate';"
        ),
        "TEXT"
    );
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS, "proposals unchanged");
    assert_eq!(PROPOSAL_COLUMNS.len(), 50);
    for (schema, width) in [(9, 24), (10, 25), (11, 25), (12, 26)] {
        assert_eq!(
            card_columns(schema),
            Some(&CARD_COLUMNS[..width]),
            "schema {schema}"
        );
    }
    // 14 since docs/features/collision-pass.md D6: no card column changes.
    assert_eq!(card_columns(14).map(<[_]>::len), Some(26));
    assert_eq!(card_columns(15), None);
    assert_eq!(card_columns(8), None);
    // No index on the new column.
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM pragma_index_list('cards') AS l, pragma_index_info(l.name) AS i \
             WHERE i.name = 'mandate';"
        ),
        "0"
    );
}

/// M5: a schema-11 queue with three cards steps to 12 at its first open:
/// `cards` gains `mandate` last, `NULL` on every row; every other row and
/// column as it was; a second open steps nothing.
#[test]
fn m5_a_schema_11_queue_with_three_cards_steps_at_its_first_open() {
    let scratch = Scratch::new("cm-m5-step");
    let db = scratch.db("q");
    three_cards(&db);
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN matters; ALTER TABLE proposals DROP COLUMN gist; ALTER TABLE cards DROP COLUMN mandate; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 11;",
    );
    assert_eq!(card_table(&db), CARD_COLUMNS[..25], "premise");
    let before = rows_of_11(&db);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM cards;"), "3", "premise");

    let queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    assert_eq!(user_version(&db), "14");
    assert_eq!(card_table(&db), CARD_COLUMNS);
    assert_eq!(rows_of_11(&db), before, "every row as it was");
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM cards WHERE mandate IS NULL;"),
        "3"
    );
    for id in ["C-0001", "C-0002", "C-0003"] {
        let card = queue.get_card(id).expect("read").expect("the row");
        assert_eq!(card.mandate, None, "{id}");
    }
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "14", "stepped once");
    assert_eq!(card_table(&db), CARD_COLUMNS, "no second column");
}

/// M5: a schema-9 queue (no `cards.tasks`, no step-11 columns, no
/// `mandate`) reaches 12 in one open: `cards` 26 columns, `proposals` 48.
#[test]
fn m5_a_schema_9_queue_reaches_12_in_one_migrate() {
    let scratch = Scratch::new("cm-m5-nine");
    let db = scratch.db("q");
    three_cards(&db);
    sqlite3(
        &db,
        "UPDATE proposals SET card_id = NULL; ALTER TABLE proposals DROP COLUMN matters; ALTER TABLE proposals DROP COLUMN gist; ALTER TABLE proposals DROP COLUMN why; \
         ALTER TABLE proposals DROP COLUMN card_id; ALTER TABLE cards DROP COLUMN mandate; \
         ALTER TABLE cards DROP COLUMN tasks; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 9;",
    );
    let cards = sqlite3(
        &db,
        &format!(
            "SELECT {} FROM cards ORDER BY id;",
            CARD_COLUMNS[..24].join(", ")
        ),
    );
    drop(SqliteQueue::open(&db, PROJECT).expect("open steps"));
    assert_eq!(user_version(&db), "14");
    assert_eq!(card_table(&db), CARD_COLUMNS);
    assert_eq!(proposal_table(&db), PROPOSAL_COLUMNS);
    assert_eq!(
        sqlite3(
            &db,
            &format!(
                "SELECT {} FROM cards ORDER BY id;",
                CARD_COLUMNS[..24].join(", ")
            ),
        ),
        cards
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM cards WHERE mandate IS NULL AND tasks IS NULL;"
        ),
        "3"
    );
}

/// M5: a hand-made `mandate` fails step 12: a schema-11 file stays at 11;
/// a schema-10 file stays at 10 with no step-11 column, every due step in
/// one transaction. M: step 12 committed apart from the other due steps
/// (the second file then reads 11, `proposals` with `card_id`).
#[test]
fn m5_a_hand_made_mandate_column_fails_step_12() {
    let scratch = Scratch::new("cm-m5-fail");

    let eleven = scratch.db("eleven");
    three_cards(&eleven);
    sqlite3(
        &eleven,
        "ALTER TABLE proposals DROP COLUMN matters; ALTER TABLE proposals DROP COLUMN gist; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 11;",
    );
    assert!(card_table(&eleven).contains(&"mandate".to_owned()));
    if let Ok(queue) = SqliteQueue::open(&eleven, PROJECT) {
        panic!("step 12 ran over a hand-made mandate: {:?}", queue.dump());
    }
    assert_eq!(user_version(&eleven), "11");
    assert_eq!(card_table(&eleven), CARD_COLUMNS, "nothing added or lost");

    let ten = scratch.db("ten");
    three_cards(&ten);
    sqlite3(
        &ten,
        "UPDATE proposals SET card_id = NULL; ALTER TABLE proposals DROP COLUMN matters; ALTER TABLE proposals DROP COLUMN gist; ALTER TABLE proposals DROP COLUMN why; \
         ALTER TABLE proposals DROP COLUMN card_id; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 10;",
    );
    let proposals = proposal_table(&ten);
    assert_eq!(proposals, PROPOSAL_COLUMNS[..46], "premise");
    if let Ok(queue) = SqliteQueue::open(&ten, PROJECT) {
        panic!("step 12 ran over a hand-made mandate: {:?}", queue.dump());
    }
    assert_eq!(user_version(&ten), "10", "no step committed alone");
    assert_eq!(proposal_table(&ten), proposals, "no card_id, no why");
}

/// M5: `open_existing` on a schema-11 queue reads every card's `mandate`
/// `None` (`list_existing_cards`), `stored_rows` its 25 card columns with the 26th `None`, and
/// steps nothing; on schema 10 the same, and `subtask_items` reads none
/// below `CARD_ITEMS_SCHEMA`.
#[test]
fn m5_open_existing_reads_an_11_queue_mandate_none_and_steps_nothing() {
    let scratch = Scratch::new("cm-m5-existing");
    let db = scratch.db("q");
    three_cards(&db);
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN matters; ALTER TABLE proposals DROP COLUMN gist; ALTER TABLE cards DROP COLUMN mandate; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 11;",
    );
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file");
    let list = queue.list_existing_cards().expect("list_existing_cards");
    assert_eq!(list.cards.len(), 3);
    assert!(list.unreadable.is_empty(), "{:?}", list.unreadable);
    for card in &list.cards {
        assert_eq!(card.mandate, None, "{}", card.id);
    }
    let stored = queue.stored_rows().expect("stored rows");
    assert_eq!(stored.cards.len(), 3);
    for row in &stored.cards {
        assert_eq!(row.columns.len(), 25, "26 less `position`");
        assert_eq!(row.columns[24], None, "{row:?}");
    }
    let items = queue.subtask_items("C-0001").expect("subtask_items");
    assert_eq!(items.items.len(), 2, "schema 11 holds card_id");
    drop(queue);
    assert_eq!(user_version(&db), "11", "stepped nothing");
    assert_eq!(card_table(&db), CARD_COLUMNS[..25]);

    sqlite3(
        &db,
        "UPDATE proposals SET card_id = NULL; ALTER TABLE proposals DROP COLUMN why; \
         ALTER TABLE proposals DROP COLUMN card_id; DROP TABLE IF EXISTS collision_marks; PRAGMA user_version = 10;",
    );
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file");
    for row in &queue.stored_rows().expect("stored rows").cards {
        assert_eq!(row.columns[24], None, "{row:?}");
    }
    for card in &queue.list_existing_cards().expect("list").cards {
        assert_eq!(card.mandate, None, "{}", card.id);
    }
    let items = queue.subtask_items("C-0001").expect("subtask_items");
    assert!(items.items.is_empty() && items.unreadable.is_empty());
    drop(queue);
    assert_eq!(user_version(&db), "10", "stepped nothing");
}

// ------------------------------------------------------------------ M1

/// M1: `create_card` with a mandate and no reviewer is `CardRefused` with
/// the first line of "Messages": nothing stored, no event, and the next
/// card takes the ID the refused one would have. M: the store not asking
/// `mandate_problem` (the card is then stored).
#[test]
fn m1_create_card_with_a_mandate_and_no_reviewer_is_refused() {
    let scratch = Scratch::new("cm-m1-refused");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_card(&new_card(&config, "narrative", "Lamp room scene"), T0)
        .expect("C-0001");
    let dump = queue.dump().expect("dump");
    for mandate in [Mandate::Binding, Mandate::Open] {
        let reason = refusal(queue.create_card(
            &NewCard {
                mandate: Some(mandate),
                parent: Some("C-0001".to_owned()),
                ..new_card(&config, "sound", "Keeper's voice")
            },
            T1,
        ));
        assert_eq!(reason, NO_REVIEWER, "{mandate:?}");
        assert_eq!(queue.dump().expect("dump"), dump, "{mandate:?}: nothing");
    }
    let next = queue
        .create_card(&new_card(&config, "sound", "Keeper's voice"), T1)
        .expect("the next card");
    assert_eq!(next.id, "C-0002", "no ID taken");
}

/// M1: a hand-off with a mandate stores it; `card.created` carries
/// `mandate` on every card, `null` when none, compared as JSON. M:
/// `card.created` carrying `mandate` only when set.
#[test]
fn m1_card_created_carries_the_mandate_always() {
    let scratch = Scratch::new("cm-m1-created");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    let parent = queue
        .create_card(&new_card(&config, "narrative", "Lamp room scene"), T0)
        .expect("C-0001");
    assert_eq!(parent.mandate, None);
    let child = queue
        .create_card(
            &hand_off(
                &config,
                "sound",
                "Keeper's voice",
                "C-0001",
                Some(Mandate::Open),
            ),
            T0,
        )
        .expect("C-0002");
    assert_eq!(child.mandate, Some(Mandate::Open));
    assert_eq!(queue.get_card("C-0002").expect("get"), Some(child));
    let reviewed = queue
        .create_card(
            &hand_off(&config, "art", "Keeper's portrait", "C-0001", None),
            T0,
        )
        .expect("C-0003");
    assert_eq!(reviewed.mandate, None);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT id, ifnull(mandate, 'NULL') FROM cards ORDER BY id;"
        ),
        "C-0001|NULL\nC-0002|open\nC-0003|NULL"
    );
    assert_eq!(
        card_events(&queue),
        [
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0001", "flow": "narrative", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": null,
                       "mandate": null, "as": ANA})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0002", "flow": "sound", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": ANA,
                       "mandate": "open", "as": ANA})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0003", "flow": "art", "column": "waiting",
                       "position": 1, "assignee": null, "reviewer": ANA,
                       "mandate": null, "as": ANA})
            ),
        ]
    );
}

/// M1: a stored `mandate` that is neither `binding` nor `open` makes the
/// row corrupt, naming `mandate`: `get_card` fails, `list_cards` skips it
/// into `unreadable`, the other rows listed; a write on it changes nothing.
#[test]
fn m1_a_stored_mandate_off_the_two_words_is_corrupt() {
    for written in ["maybe", "Binding", " open", "", "-"] {
        let scratch = Scratch::new("cm-m1-corrupt");
        let db = scratch.db("q");
        three_cards(&db);
        sqlite3(
            &db,
            &format!("UPDATE cards SET mandate = '{written}' WHERE id = 'C-0002';"),
        );
        let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let list = queue.list_cards().expect("list");
        let listed: Vec<&str> = list.cards.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(listed, ["C-0001", "C-0003"], "{written:?}");
        assert_eq!(list.unreadable.len(), 1, "{written:?}");
        assert_eq!(list.unreadable[0].id, "C-0002", "{written:?}");
        assert_eq!(list.unreadable[0].column, "mandate", "{written:?}");
        let error = queue.get_card("C-0002").expect_err(written).to_string();
        assert!(
            error.contains("C-0002") && error.contains("mandate"),
            "{written:?}: {error}"
        );
        let dump = queue.dump().expect("dump");
        let tried = queue.change_card(
            "C-0002",
            None,
            &edit(CardEdit {
                title: Some("Renamed".to_owned()),
                ..CardEdit::default()
            }),
            ANA,
            T1,
        );
        assert!(tried.is_err(), "{written:?}");
        assert_eq!(queue.dump().expect("dump"), dump, "{written:?}");
    }
}

// ------------------------------------------------------------------ M3

/// M3, the store: Ana Ruiz, the stored reviewer, changes the mandate:
/// `revision` 2, one `card.edited` `{id, fields: ["mandate"], as, mandate}`;
/// Ben Ode is refused with the line under the lock, nothing written, the
/// title he names with it unwritten too; Ben's title alone stands with
/// today's payload; a title and a mandate by Ana name both fields; the
/// same mandate again is an edit like any. M: the compare reading
/// `assignee`; the check skipped when the edit holds another field;
/// `card.edited` without `mandate`.
#[test]
fn m3_only_the_reviewer_changes_the_mandate_under_the_lock() {
    let scratch = Scratch::new("cm-m3-reviewer");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_card(&new_card(&config, "narrative", "Lamp room scene"), T0)
        .expect("C-0001");
    // Ben Ode is the assignee: the compare must read the reviewer.
    queue
        .create_card(
            &NewCard {
                assignee: Some(BEN.to_owned()),
                ..hand_off(
                    &config,
                    "sound",
                    "Keeper's voice",
                    "C-0001",
                    Some(Mandate::Open),
                )
            },
            T0,
        )
        .expect("C-0002");
    let events_before = card_events(&queue).len();

    let other = format!(
        "mandate: only C-0002's reviewer `{ANA}` changes its mandate, not `{BEN}`; nothing changed"
    );
    let dump = queue.dump().expect("dump");
    assert_eq!(
        refusal(queue.change_card("C-0002", None, &mandate_edit(Mandate::Binding), BEN, T1)),
        other
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    assert_eq!(
        refusal(queue.change_card(
            "C-0002",
            Some(1),
            &edit(CardEdit {
                title: Some("Renamed".to_owned()),
                mandate: Some(Mandate::Binding),
                ..CardEdit::default()
            }),
            BEN,
            T1,
        )),
        other
    );
    assert_eq!(queue.dump().expect("dump"), dump, "the title unwritten too");
    for actor in ["local", "agent", "ana ruiz"] {
        let reason =
            refusal(queue.change_card("C-0002", None, &mandate_edit(Mandate::Binding), actor, T1));
        assert_eq!(
            reason,
            format!(
                "mandate: only C-0002's reviewer `{ANA}` changes its mandate, not `{actor}`; \
                 nothing changed"
            )
        );
    }
    assert_eq!(queue.dump().expect("dump"), dump);

    let card = queue
        .change_card("C-0002", Some(1), &mandate_edit(Mandate::Binding), ANA, T1)
        .expect("the reviewer's edit");
    assert_eq!(card.mandate, Some(Mandate::Binding));
    assert_eq!(card.revision, 2);
    let events = card_events(&queue);
    assert_eq!(events.len(), events_before + 1, "exactly one new event");
    assert_eq!(
        events[events_before],
        (
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": ANA, "mandate": "binding"})
        )
    );

    // Ben's title alone stands; its payload is today's exactly.
    let card = queue
        .change_card(
            "C-0002",
            None,
            &edit(CardEdit {
                title: Some("The keeper's voice".to_owned()),
                ..CardEdit::default()
            }),
            BEN,
            T1,
        )
        .expect("a title edit by anyone");
    assert_eq!(card.revision, 3);
    assert_eq!(card.mandate, Some(Mandate::Binding));
    assert_eq!(
        card_events(&queue).last().cloned(),
        Some((
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0002", "fields": ["title"], "as": BEN})
        ))
    );

    // A title and a mandate by the reviewer: both fields, in table order.
    let card = queue
        .change_card(
            "C-0002",
            None,
            &edit(CardEdit {
                title: Some("Keeper's voice".to_owned()),
                mandate: Some(Mandate::Open),
                ..CardEdit::default()
            }),
            ANA,
            T1,
        )
        .expect("the reviewer's two fields");
    assert_eq!(card.revision, 4);
    assert_eq!(
        card_events(&queue).last().cloned(),
        Some((
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0002", "fields": ["title", "mandate"], "as": ANA,
                   "mandate": "open"})
        ))
    );

    // The same mandate again: written as any edit.
    let card = queue
        .change_card("C-0002", None, &mandate_edit(Mandate::Open), ANA, T1)
        .expect("the same mandate again");
    assert_eq!(card.revision, 5);
    assert_eq!(card.mandate, Some(Mandate::Open));
    assert_eq!(
        card_events(&queue).last().cloned(),
        Some((
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0002", "fields": ["mandate"], "as": ANA, "mandate": "open"})
        ))
    );
}

/// M3, the store: `assign` leaves the mandate; with the reviewer cleared the
/// mandate stays and nobody changes it (the no-reviewer line); with Ben Ode
/// assigned, Ben's edit stands and Ana's is refused.
#[test]
fn m3_assign_leaves_the_mandate_and_moves_who_changes_it() {
    let scratch = Scratch::new("cm-m3-assign");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_card(&new_card(&config, "narrative", "Lamp room scene"), T0)
        .expect("C-0001");
    queue
        .create_card(
            &hand_off(
                &config,
                "sound",
                "Keeper's voice",
                "C-0001",
                Some(Mandate::Open),
            ),
            T0,
        )
        .expect("C-0002");
    let cleared = queue
        .change_card(
            "C-0002",
            None,
            &CardChange::Assign {
                assignee: None,
                reviewer: Some(None),
            },
            ANA,
            T1,
        )
        .expect("the reviewer cleared");
    assert_eq!(cleared.reviewer, None);
    assert_eq!(cleared.mandate, Some(Mandate::Open), "the mandate stays");
    let dump = queue.dump().expect("dump");
    assert_eq!(
        refusal(queue.change_card("C-0002", None, &mandate_edit(Mandate::Binding), ANA, T1)),
        "mandate: C-0002 has no reviewer, who alone changes its mandate: name one with `spec \
         card assign C-0002 --reviewer N` first; nothing changed"
    );
    assert_eq!(queue.dump().expect("dump"), dump);

    let assigned = queue
        .change_card(
            "C-0002",
            None,
            &CardChange::Assign {
                assignee: None,
                reviewer: Some(Some(BEN.to_owned())),
            },
            ANA,
            T1,
        )
        .expect("Ben the reviewer");
    assert_eq!(assigned.mandate, Some(Mandate::Open), "the mandate stays");
    assert!(
        queue
            .change_card("C-0002", None, &mandate_edit(Mandate::Binding), ANA, T1)
            .is_err()
    );
    let card = queue
        .change_card("C-0002", None, &mandate_edit(Mandate::Binding), BEN, T1)
        .expect("Ben's edit");
    assert_eq!(card.mandate, Some(Mandate::Binding));
}

// ------------------------------------------------------------------ M2

/// M2, the store: a split's children keep their own mandates, its new
/// parent none; `card.created` carries each, `null` on the parent.
#[test]
fn m2_a_splits_children_store_their_own_mandates() {
    let scratch = Scratch::new("cm-m2-split");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    let frozen = vec![CardFrozen {
        id: "SCN-STORM".to_owned(),
        rev: Some(1),
        norm_hash: Some("b3:frozen".to_owned()),
        within_hash: Some("b3:frozen".to_owned()),
        also: Vec::new(),
    }];
    let reviewed = |discipline: &str, mandate: Option<Mandate>| NewCard {
        reviewer: Some(ANA.to_owned()),
        mandate,
        ..new_card(&config, discipline, "A new keeper")
    };
    let split = queue
        .split_cards(
            &NewSplit {
                parent: SplitParent::New {
                    card: Box::new(reviewed("narrative", None)),
                    snapshot: frozen.clone(),
                },
                children: vec![
                    NewSplitChild {
                        card: reviewed("art", Some(Mandate::Binding)),
                        snapshot: frozen.clone(),
                    },
                    NewSplitChild {
                        card: reviewed("sound", Some(Mandate::Open)),
                        snapshot: frozen.clone(),
                    },
                    NewSplitChild {
                        card: reviewed("level-design", None),
                        snapshot: frozen,
                    },
                ],
            },
            T0,
        )
        .expect("the split");
    assert_eq!(split.parent.mandate, None);
    let mandates: Vec<(&str, Option<Mandate>)> = split
        .children
        .iter()
        .map(|child| (child.id.as_str(), child.mandate))
        .collect();
    assert_eq!(
        mandates,
        [
            ("C-0002", Some(Mandate::Binding)),
            ("C-0003", Some(Mandate::Open)),
            ("C-0004", None),
        ]
    );
    let created: Vec<Value> = card_events(&queue)
        .into_iter()
        .filter(|(kind, _)| kind == EVENT_CARD_CREATED)
        .map(|(_, payload)| payload["mandate"].clone())
        .collect();
    assert_eq!(
        created,
        [json!(null), json!("binding"), json!("open"), json!(null)]
    );
}

// ------------------------------------------------------------------ M6

/// M6, the store: `subtask_items("C-0001")` lists the items naming its
/// children `C-0002`, `C-0003` and the cancelled `C-0005`, by ID number,
/// each with its `card_id`; not `C-0001`'s own, not the grandchild
/// `C-0004`'s, not an update; a corrupt one goes into `unreadable`. M:
/// `subtask_items` taking grandchildren.
#[test]
fn m6_subtask_items_read_the_childrens_items() {
    let scratch = Scratch::new("cm-m6-store");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    queue
        .create_card(&new_card(&config, "narrative", "Lamp room scene"), T0)
        .expect("C-0001");
    for (discipline, parent, mandate) in [
        ("sound", "C-0001", Some(Mandate::Open)),
        ("art", "C-0001", Some(Mandate::Binding)),
        ("level-design", "C-0002", None),
        ("production", "C-0001", None),
    ] {
        queue
            .create_card(
                &hand_off(&config, discipline, "A part", parent, mandate),
                T0,
            )
            .unwrap_or_else(|error| panic!("{discipline}: {error}"));
    }
    queue
        .change_card(
            "C-0005",
            None,
            &CardChange::Cancel {
                reason: "Cut from the build".to_owned(),
            },
            ANA,
            T1,
        )
        .expect("C-0005 cancelled");

    let own = raised(
        &mut queue,
        &item(ProposalKind::Question, "Which lamp?", "C-0001"),
    );
    let art = raised(
        &mut queue,
        &item(ProposalKind::Decision, "A charcoal portrait.", "C-0003"),
    );
    let sound_question = raised(
        &mut queue,
        &item(ProposalKind::Question, "Which accent?", "C-0002"),
    );
    let grandchild = raised(
        &mut queue,
        &item(ProposalKind::Decision, "A narrow stair.", "C-0004"),
    );
    let cancelled = raised(
        &mut queue,
        &item(ProposalKind::Decision, "No ledger scene.", "C-0005"),
    );
    let sound_decision = raised(
        &mut queue,
        &item(ProposalKind::Decision, "A low alto.", "C-0002"),
    );
    assert_eq!(
        [
            own.as_str(),
            &art,
            &sound_question,
            &grandchild,
            &cancelled,
            &sound_decision
        ],
        [
            "PR-0001", "PR-0002", "PR-0003", "PR-0004", "PR-0005", "PR-0006"
        ],
        "premise"
    );

    let read = queue.subtask_items("C-0001").expect("subtask_items");
    let listed: Vec<(&str, Option<&str>)> = read
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.card_id.as_deref()))
        .collect();
    assert_eq!(
        listed,
        [
            ("PR-0002", Some("C-0003")),
            ("PR-0003", Some("C-0002")),
            ("PR-0005", Some("C-0005")),
            ("PR-0006", Some("C-0002")),
        ]
    );
    assert!(read.unreadable.is_empty());
    // The grandchild's parent reads its own child's items alone.
    let middle = queue.subtask_items("C-0002").expect("subtask_items");
    let ids: Vec<&str> = middle.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["PR-0004"]);
    for leaf in ["C-0003", "C-0004", "C-0099"] {
        let none = queue.subtask_items(leaf).expect("subtask_items");
        assert!(
            none.items.is_empty() && none.unreadable.is_empty(),
            "{leaf}"
        );
    }
    drop(queue);

    // A question holding a `why` does not decode: unreadable, the others
    // still listed.
    sqlite3(
        &db,
        "UPDATE proposals SET why = 'Because.' WHERE id = 'PR-0003';",
    );
    let queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let read = queue.subtask_items("C-0001").expect("subtask_items");
    let ids: Vec<&str> = read.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["PR-0002", "PR-0005", "PR-0006"]);
    assert_eq!(read.unreadable.len(), 1);
    assert_eq!(read.unreadable[0].id, "PR-0003");
    assert_eq!(read.unreadable[0].column, "why");
}
