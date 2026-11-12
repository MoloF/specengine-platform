//! docs/features/card-store.md, the store half ("Store", "Backup", "Rules
//! and edge cases"): `SqliteQueue`'s cards through the public API, the flows
//! read from `fixtures/card-press/specengine.toml` by core's reader; the raw
//! steps (an older schema, a corrupt row) use the system's `sqlite3`.
//!
//! - Schema 9: `cards` as "Store" gives it, `STRICT`, `TEXT` but
//!   `position`; a schema-8 queue steps once, its rows kept, `events` and
//!   `proposals` unchanged. Schema 10 (docs/canon/cards.md "Store"):
//!   25 columns in `CARD_COLUMNS` order, `tasks` last.
//! - Writes: one `Immediate` transaction, one event, `revision` raised; the
//!   rules of core asked again under the lock (a CLI that skipped them, or a
//!   stale read, changes nothing); a lost compare-and-set; dense positions,
//!   a shifted neighbour taking neither `revision` nor `updated_at` nor an
//!   event; accepted and cancelled cards frozen.
//! - IDs: highest plus one in the transaction: eight writers on one file
//!   make forty distinct cards, one column dense.
//! - Corrupt rows: `get_card` fails naming card and column, `list_cards`
//!   skips them into `unreadable`, a write on one changes nothing.
//! - Backup: `stored_rows`, `counts`, `restore` hold cards; a card alone
//!   occupies a queue; another project's card is refused. M: `stored_rows`
//!   skipping `cards` (AC-07).
//! - The gate: `GateQueue::tasks` answers alike over a corrupt card row and
//!   with no `cards` table at all, and `queue/gate.rs` names no card (M:
//!   `GateQueue::tasks` reading `cards`).
//! - card-tasks (RM-0092, CT-4, docs/canon/cards.md "Store"): a
//!   schema-9 queue made with step 9's SQL, holding a card, opens at 10, the
//!   card linking none (`NULL`, read as `[]`); `CardEdit.tasks` replaces the
//!   list whole, `Some` of none clears it, `fields` names `tasks` last; core's
//!   grammar, repeat and cap asked again under the lock, while no task is
//!   looked up (D4: an ID no task holds is stored); a `tasks` that is no JSON
//!   array of task IDs makes the row unreadable naming `tasks`, a repeat or a
//!   long list a hand edit left reads as stored; the gate reads no link.
//!   M6: `STEP_10` left out of `migrate`.
//! - board-handoff (RM-0085, docs/canon/cards.md "Documents and events"):
//!   `card.created` carries `reviewer` and `card.moved` `note`, each `null`
//!   when none; the reviewer and the return column's note asked again under
//!   the lock, on `fixtures/studio-lamp`'s flows (each with `return =
//!   "sent-back"`).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Scratch, fixture, repository_root};
use serde_json::{Value, json};
use specengine_core::card_toml::{CardConfig, CardFlow};
use specengine_core::trace::TargetNorms;
use specengine_model::EvidenceKind;
use specengine_store::{
    CARD_COLUMNS, CardAccept, CardChange, CardEdit, EVENT_CARD_ACCEPTED, EVENT_CARD_ASSIGNED,
    EVENT_CARD_CANCELLED, EVENT_CARD_CREATED, EVENT_CARD_EDITED, EVENT_CARD_MOVED, NewCard,
    NewTask, ProposalQueue as _, QUEUE_SCHEMA_VERSION, QueueCounts, QueueError, Restore,
    SqliteQueue, StoredCard, StoredEvent, StoredQueue, open_for_gate,
};

const T0: &str = "2026-10-30T09:00:00Z";
const T1: &str = "2026-10-30T09:00:01Z";
const T2: &str = "2026-10-30T09:00:02Z";
const PROJECT: &str = "card-press";

/// The columns as "Store" lists them, written here: step 9's 24, then step
/// 10's `tasks` (docs/canon/cards.md "Store"), then step 12's `mandate`
/// (docs/features/task-mandate.md M5).
const COLUMNS: [&str; 26] = [
    "id",
    "project",
    "flow",
    "col",
    "position",
    "title",
    "goal",
    "discipline",
    "targets",
    "snapshot",
    "criteria",
    "assignee",
    "reviewer",
    "estimate",
    "due",
    "parent",
    "corrects",
    "evidence",
    "acceptance",
    "cancellation",
    "created_by",
    "created_at",
    "updated_at",
    "revision",
    "tasks",
    "mandate",
];

/// Step 9's SQL as `crates/specengine-store/src/queue.rs` holds it
/// (docs/features/card-store.md "Store"): the `cards` a schema-9 queue has.
const STEP_9: &str = "
CREATE TABLE cards (
  id TEXT PRIMARY KEY,
  project TEXT, flow TEXT, col TEXT, position INTEGER,
  title TEXT, goal TEXT, discipline TEXT, targets TEXT, snapshot TEXT, criteria TEXT,
  assignee TEXT, reviewer TEXT, estimate TEXT, due TEXT, parent TEXT, corrects TEXT,
  evidence TEXT, acceptance TEXT, cancellation TEXT,
  created_by TEXT, created_at TEXT, updated_at TEXT, revision TEXT
) STRICT;
";

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

/// The flows of `fixtures/card-press`, through core's reader.
fn press() -> CardConfig {
    let text = fs::read_to_string(fixture("card-press").join("specengine.toml")).expect("config");
    CardConfig::from_toml(&text).expect("the fixture's [card]")
}

fn copy(config: &CardConfig) -> &CardFlow {
    config.flow("copy").expect("the copy flow")
}

fn plates(config: &CardConfig) -> &CardFlow {
    config.flow("plates").expect("the plates flow")
}

fn new_card(flow: &CardFlow, title: &str) -> NewCard {
    NewCard {
        flow: flow.name.clone(),
        column: flow.first_column().to_owned(),
        title: title.to_owned(),
        goal: None,
        discipline: None,
        targets: Vec::new(),
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

fn to<'f>(flow: &'f CardFlow, column: &str) -> CardChange<'f> {
    CardChange::Move {
        flow: Some(flow),
        to: Some(column.to_owned()),
        before: None,
        note: None,
    }
}

fn before<'f>(flow: &'f CardFlow, column: Option<&str>, card: &str) -> CardChange<'f> {
    CardChange::Move {
        flow: Some(flow),
        to: column.map(str::to_owned),
        before: Some(card.to_owned()),
        note: None,
    }
}

/// The reason of a refused change.
fn refusal(result: Result<impl std::fmt::Debug, QueueError>) -> String {
    match result {
        Err(QueueError::CardRefused { reason, .. }) => reason,
        other => panic!("expected CardRefused, got {other:?}"),
    }
}

/// `(type, payload)` of every `card.*` event.
fn card_events(queue: &SqliteQueue) -> Vec<(String, Value)> {
    queue
        .events()
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type.starts_with("card."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// `(id, column, position)` of every card, by number.
fn board(queue: &SqliteQueue) -> Vec<(String, String, u64)> {
    queue
        .list_cards()
        .expect("list")
        .cards
        .into_iter()
        .map(|card| (card.id, card.column, card.position))
        .collect()
}

fn card(id: &str, column: &str, position: u64) -> (String, String, u64) {
    (id.to_owned(), column.to_owned(), position)
}

// --------------------------------------------------------------- schema

/// "Store": `cards` as `PRAGMA table_info` gives it, `id` the key, `position`
/// the one `INTEGER`, `STRICT`; `CARD_COLUMNS` is that list, 25 of them since
/// docs/features/card-tasks.md (`tasks` last, queue schema 10). M: a column
/// moved or retyped.
#[test]
fn schema_10_makes_the_cards_table() {
    // Schema 11 since docs/features/card-questions.md, `cards` unchanged;
    // 12 since docs/features/task-mandate.md M5, `mandate` last.
    assert_eq!(QUEUE_SCHEMA_VERSION, 12);
    assert_eq!(CARD_COLUMNS, COLUMNS);
    assert_eq!(CARD_COLUMNS.last(), Some(&"mandate"));
    let scratch = Scratch::new("qc-schema");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "12");
    let want: Vec<String> = COLUMNS
        .iter()
        .map(|column| {
            let kind = if *column == "position" {
                "INTEGER"
            } else {
                "TEXT"
            };
            format!("{column} {kind} {}", u8::from(*column == "id"))
        })
        .collect();
    assert_eq!(
        sqlite3(
            &db,
            "SELECT name || ' ' || type || ' ' || pk FROM pragma_table_info('cards') ORDER BY cid;"
        ),
        want.join("\n")
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT strict FROM pragma_table_list WHERE name = 'cards';"
        ),
        "1"
    );
}

/// AC-07, the store half: a schema-8 queue (a proposal's table, a task and
/// an event in it) opens at 11 in one open (card-tasks CT-4: steps 9 and
/// 10; card-questions: 11), every row byte for byte; `cards` empty, its 25
/// columns, a card made after. M: the step skipped.
#[test]
fn a_schema_8_queue_opens_at_10_with_its_rows_unchanged() {
    let scratch = Scratch::new("qc-step");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue
        .create_task(
            &NewTask {
                git_common_dir: "/r/.git".to_owned(),
                title: Some("A task".to_owned()),
                goal: None,
                targets: vec!["SHT-FRONT".to_owned()],
                author: specengine_core::proposal::Author::human(),
            },
            T0,
        )
        .expect("a task");
    let rows = queue.dump().expect("dump");
    drop(queue);
    // Schema 8's `proposals` holds no step-11 column either.
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN why; ALTER TABLE proposals DROP COLUMN card_id; \
         DROP TABLE cards; PRAGMA user_version = 8;",
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM sqlite_master WHERE name = 'cards';"
        ),
        "0"
    );
    let raw = |db: &Path| {
        sqlite3(
            db,
            "SELECT * FROM proposals; SELECT * FROM tasks; SELECT * FROM runs; SELECT * FROM \
             events;",
        )
    };
    let before_step = raw(&db);
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    // 12 since docs/features/task-mandate.md M5.
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "12");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT group_concat(name, ' ') FROM (SELECT name FROM pragma_table_info('cards') \
             ORDER BY cid);"
        ),
        COLUMNS.join(" "),
        "steps 9, 10 and 11 in one open"
    );
    assert_eq!(raw(&db), before_step, "rows unchanged");
    assert_eq!(queue.dump().expect("dump"), rows);
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM cards;"),
        "0",
        "the table made, empty"
    );
    let config = press();
    let made = queue
        .create_card(&new_card(copy(&config), "After the step"), T1)
        .expect("a card");
    assert_eq!(made.id, "C-0001");
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM cards;"),
        "1",
        "not stepped twice"
    );
}

// --------------------------------------------------------------- writes

/// "Store": a new card: `C-0001`, last in its column of its own flow,
/// revision 1, times the clock, `snapshot` `NULL`, one `card.created`.
#[test]
fn create_card_numbers_places_and_logs() {
    let scratch = Scratch::new("qc-create");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let first = queue
        .create_card(
            &NewCard {
                goal: Some("Ten inches.".to_owned()),
                discipline: Some("copy".to_owned()),
                targets: vec!["SHT-FRONT".to_owned()],
                criteria: vec!["Two sources.".to_owned()],
                assignee: Some("ana".to_owned()),
                estimate: Some("2-5".to_owned()),
                due: Some("2026-11-05".to_owned()),
                by: "writer".to_owned(),
                ..new_card(copy(&config), "Lead")
            },
            T0,
        )
        .expect("a card");
    assert_eq!(first.id, "C-0001");
    assert_eq!(first.project, PROJECT);
    assert_eq!(first.flow, "copy");
    assert_eq!(first.column, "draft");
    assert_eq!(first.position, 1);
    assert_eq!(first.revision, 1);
    assert_eq!(first.created_at, T0);
    assert_eq!(first.updated_at, T0);
    assert_eq!(first.created_by, "writer");
    assert_eq!(first.snapshot, None);
    assert_eq!(first.evidence, []);
    assert_eq!(queue.get_card("C-0001").expect("get"), Some(first));

    let second = queue
        .create_card(&new_card(copy(&config), "Second"), T0)
        .expect("a card");
    assert_eq!((second.id.as_str(), second.position), ("C-0002", 2));
    let other = queue
        .create_card(&new_card(plates(&config), "Other column"), T0)
        .expect("a card");
    assert_eq!((other.id.as_str(), other.position), ("C-0003", 1));
    assert_eq!(
        card_events(&queue),
        [
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0001", "flow": "copy", "column": "draft", "position": 1,
                       "assignee": "ana", "reviewer": null, "mandate": null,
                       "as": "writer"})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0002", "flow": "copy", "column": "draft", "position": 2,
                       "assignee": null, "reviewer": null, "mandate": null,
                       "as": "local"})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0003", "flow": "plates", "column": "queued", "position": 1,
                       "assignee": null, "reviewer": null, "mandate": null,
                       "as": "local"})
            ),
        ]
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT targets, criteria, evidence, snapshot IS NULL, acceptance IS NULL, \
             cancellation IS NULL, revision, typeof(revision) FROM cards WHERE id = 'C-0001';"
        ),
        "[\"SHT-FRONT\"]|[\"Two sources.\"]|[]|1|1|1|1|text"
    );

    // A link naming nothing, a bad clock: nothing stored.
    let dump = queue.dump().expect("dump");
    let parent = NewCard {
        parent: Some("C-0099".to_owned()),
        ..new_card(copy(&config), "Orphan")
    };
    assert!(refusal(queue.create_card(&parent, T1)).contains("no card C-0099"));
    let corrects = NewCard {
        corrects: Some("C-0099".to_owned()),
        ..new_card(copy(&config), "Orphan")
    };
    assert!(refusal(queue.create_card(&corrects, T1)).contains("no card C-0099"));
    assert!(
        queue
            .create_card(&new_card(copy(&config), "T"), "soon")
            .is_err()
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing stored");
}

/// "Rules and edge cases": the store asks core again under the lock: a move
/// outside `moves`, an actor outside `by`, `accept.to`: refused with the
/// spec's line, `dump()` unchanged. M: the store trusting the caller.
#[test]
fn the_store_asks_the_move_rules_again_under_the_lock() {
    let scratch = Scratch::new("qc-rules");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    queue
        .create_card(&new_card(flow, "Lead"), T0)
        .expect("a card");
    let dump = queue.dump().expect("dump");
    assert_eq!(
        refusal(queue.change_card("C-0001", None, &to(flow, "proof"), "desk", T1)),
        "C-0001 is in draft: no move to `proof`; allowed: `edit`; nothing changed"
    );
    assert_eq!(
        refusal(queue.change_card("C-0001", None, &to(flow, "filed"), "desk", T1)),
        "C-0001 is in draft: no move to `filed`; allowed: `edit`; nothing changed"
    );
    let reason = refusal(queue.change_card("C-0001", None, &to(flow, "edit"), "ana", T1));
    assert!(reason.contains("`desk`"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), dump);
    assert!(
        refusal(queue.change_card("C-0099", None, &to(flow, "edit"), "desk", T1))
            .contains("no card C-0099")
    );
    assert!(
        queue
            .change_card("C-0001", None, &to(flow, "edit"), "desk", "soon")
            .is_err(),
        "a bad clock"
    );
    assert_eq!(queue.dump().expect("dump"), dump);

    let moved = queue
        .change_card("C-0001", None, &to(flow, "edit"), "desk", T1)
        .expect("the desk's move");
    assert_eq!(moved.column, "edit");
    assert_eq!(moved.revision, 2);
    assert_eq!(moved.updated_at, T1);
    assert_eq!(moved.created_at, T0);
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_MOVED.to_owned(),
            json!({"id": "C-0001", "from": "draft", "to": "edit", "position": 1, "as": "desk",
                   "note": null})
        ))
    );
    let moved = queue
        .change_card("C-0001", None, &to(flow, "proof"), "local", T2)
        .expect("anyone's move");
    assert_eq!(moved.revision, 3);
    assert_eq!(
        refusal(queue.change_card("C-0001", None, &to(flow, "filed"), "desk", T2)),
        "C-0001 is in proof: no move to `filed`; allowed: `spec card accept`; nothing changed"
    );
}

/// AC-02: a change read before another's lost: the line with `in <column>`,
/// nothing written, within one second too; a second handle on one file is
/// what a second process is.
#[test]
fn a_lost_compare_and_set_changes_nothing() {
    let scratch = Scratch::new("qc-cas");
    let db = scratch.db("q");
    let mut one = SqliteQueue::open(&db, PROJECT).expect("open");
    let mut two = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    one.create_card(&new_card(flow, "Plate"), T0)
        .expect("a card");
    let seen = one.get_card("C-0001").unwrap().unwrap().revision;
    assert_eq!(seen, 1);
    two.change_card("C-0001", Some(seen), &to(flow, "blocked"), "local", T0)
        .expect("the first");
    let dump = one.dump().expect("dump");
    for change in [
        to(flow, "checked"),
        CardChange::Edit(Box::new(CardEdit {
            goal: Some(Some("G.".to_owned())),
            ..CardEdit::default()
        })),
        CardChange::Assign {
            assignee: Some(Some("ana".to_owned())),
            reviewer: None,
        },
        CardChange::Cancel {
            reason: "No.".to_owned(),
        },
    ] {
        assert_eq!(
            refusal(one.change_card("C-0001", Some(seen), &change, "local", T0)),
            format!(
                "`C-0001` changed since this run read it: it is in blocked since {T0}; nothing \
                 changed"
            ),
            "{change:?}"
        );
    }
    assert_eq!(one.dump().expect("dump"), dump);
    let held = one
        .change_card("C-0001", Some(2), &to(flow, "checked"), "local", T0)
        .expect("the current revision");
    assert_eq!(held.revision, 3);
}

/// "Data", Order: dense from 1 per (flow, `col`); `--before` takes the
/// place; a move across columns closes the one and opens the other; a
/// neighbour takes no `revision`, no `updated_at`, no event, so a read
/// before the shift holds its compare-and-set.
#[test]
fn order_is_dense_and_a_neighbour_is_not_touched() {
    let scratch = Scratch::new("qc-order");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    for title in ["A", "B", "C", "D"] {
        queue
            .create_card(&new_card(flow, title), T0)
            .expect("a card");
    }
    let events = queue.events().expect("events").len();
    let c1 = queue.get_card("C-0001").unwrap().unwrap();
    queue
        .change_card("C-0004", None, &before(flow, None, "C-0002"), "local", T1)
        .expect("a reorder");
    assert_eq!(
        board(&queue),
        [
            card("C-0001", "queued", 1),
            card("C-0002", "queued", 3),
            card("C-0003", "queued", 4),
            card("C-0004", "queued", 2),
        ]
    );
    assert_eq!(
        queue.events().expect("events").len(),
        events + 1,
        "one event"
    );
    assert_eq!(
        card_events(&queue).last().map(|event| event.1.clone()),
        Some(
            json!({"id": "C-0004", "from": "queued", "to": "queued", "position": 2,
                    "as": "local", "note": null})
        )
    );
    for id in ["C-0002", "C-0003"] {
        let neighbour = queue.get_card(id).unwrap().unwrap();
        assert_eq!(neighbour.revision, 1, "{id}");
        assert_eq!(neighbour.updated_at, T0, "{id}");
    }
    assert_eq!(queue.get_card("C-0001").unwrap().unwrap(), c1, "untouched");
    queue
        .change_card("C-0003", Some(1), &to(flow, "blocked"), "local", T1)
        .expect("the shifted neighbour's compare-and-set holds");
    assert_eq!(
        board(&queue),
        [
            card("C-0001", "queued", 1),
            card("C-0002", "queued", 3),
            card("C-0003", "blocked", 1),
            card("C-0004", "queued", 2),
        ]
    );
    queue
        .change_card(
            "C-0001",
            None,
            &before(flow, Some("blocked"), "C-0003"),
            "local",
            T1,
        )
        .expect("into a column, before its card");
    assert_eq!(
        board(&queue),
        [
            card("C-0001", "blocked", 1),
            card("C-0002", "queued", 2),
            card("C-0003", "blocked", 2),
            card("C-0004", "queued", 1),
        ],
        "both columns dense"
    );
    let dump = queue.dump().expect("dump");
    for (column, target) in [
        (None, "C-0003"),
        (Some("checked"), "C-0003"),
        (None, "C-0099"),
        // Before itself.
        (None, "C-0002"),
    ] {
        let reason =
            refusal(queue.change_card("C-0002", None, &before(flow, column, target), "local", T2));
        assert!(reason.contains("--before"), "{reason}");
    }
    assert_eq!(queue.dump().expect("dump"), dump);
}

/// "Rules and edge cases": `accept`: in `accept.from`, by its reviewer, never
/// its assignee, never `agent` or `local`, by the store again; then frozen;
/// `reviewer` takes neither name.
#[test]
fn accept_is_the_reviewers_and_freezes_the_card() {
    let scratch = Scratch::new("qc-accept");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    let norms = TargetNorms::new();
    let accept = CardAccept {
        flow: Some(flow),
        note: Some("Fine."),
        norms: &norms,
    };
    queue
        .create_card(&new_card(flow, "Plate"), T0)
        .expect("a card");
    queue
        .change_card(
            "C-0001",
            None,
            &CardChange::Assign {
                assignee: Some(Some("ana".to_owned())),
                reviewer: Some(Some("ben".to_owned())),
            },
            "desk",
            T0,
        )
        .expect("assigned");
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_ASSIGNED.to_owned(),
            json!({"id": "C-0001", "assignee": "ana", "reviewer": "ben", "as": "desk"})
        ))
    );
    let dump = queue.dump().expect("dump");
    assert!(
        refusal(queue.accept_card("C-0001", None, &accept, "ben", T1)).contains("checked"),
        "not in accept.from"
    );
    for name in ["agent", "local"] {
        let refused = refusal(queue.change_card(
            "C-0001",
            None,
            &CardChange::Assign {
                assignee: None,
                reviewer: Some(Some(name.to_owned())),
            },
            "desk",
            T1,
        ));
        assert!(refused.contains(name), "{refused}");
    }
    assert_eq!(queue.dump().expect("dump"), dump);
    queue
        .change_card("C-0001", None, &to(flow, "checked"), "local", T1)
        .expect("moved");
    let dump = queue.dump().expect("dump");
    for name in ["ana", "cid", "agent", "local"] {
        refusal(queue.accept_card("C-0001", None, &accept, name, T1));
        assert_eq!(queue.dump().expect("dump"), dump, "{name}");
    }
    let accepted = queue
        .accept_card("C-0001", Some(3), &accept, "ben", T2)
        .expect("the reviewer's")
        .card;
    assert_eq!(accepted.column, "struck");
    assert_eq!(accepted.position, 1);
    assert_eq!(accepted.revision, 4);
    let acceptance = accepted.acceptance.expect("acceptance");
    assert_eq!(acceptance.by, "ben");
    assert_eq!(acceptance.at, T2);
    assert_eq!(acceptance.note.as_deref(), Some("Fine."));
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_ACCEPTED.to_owned(),
            json!({"id": "C-0001", "from": "checked", "to": "struck", "position": 1, "as": "ben",
                "verdict": "briefed", "discrepancy": null})
        ))
    );
    assert_eq!(
        sqlite3(&db, "SELECT acceptance FROM cards;"),
        format!(
            "{{\"by\":\"ben\",\"at\":\"{T2}\",\"note\":\"Fine.\",\"verdict\":\"briefed\",\
             \"departs\":[],\"discrepancy\":null}}"
        ),
        "`{{by, at, note}}`"
    );

    let dump = queue.dump().expect("dump");
    let closed: [CardChange<'_>; 4] = [
        to(flow, "queued"),
        CardChange::Edit(Box::new(CardEdit {
            goal: Some(Some("G.".to_owned())),
            ..CardEdit::default()
        })),
        CardChange::Assign {
            assignee: Some(None),
            reviewer: None,
        },
        CardChange::Cancel {
            reason: "No.".to_owned(),
        },
    ];
    for change in &closed {
        let reason = refusal(queue.change_card("C-0001", None, change, "ben", T2));
        assert!(reason.contains("--corrects"), "{reason}");
    }
    let reason = refusal(queue.accept_card("C-0001", None, &accept, "ben", T2));
    assert!(reason.contains("--corrects"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), dump);
    assert_eq!(queue.get_card("C-0001").unwrap().unwrap().position, 1);
}

/// AC-11: `cancel`: `position` 0, the column closed up, one `card.cancelled`,
/// `cancellation` `{by, at, reason}`, frozen; the column's next card takes
/// the place a cancelled one left, never position 0's.
#[test]
fn cancel_leaves_the_column_and_freezes_the_card() {
    let scratch = Scratch::new("qc-cancel");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    for title in ["A", "B", "C"] {
        queue
            .create_card(&new_card(flow, title), T0)
            .expect("a card");
    }
    let cancelled = queue
        .change_card(
            "C-0002",
            Some(1),
            &CardChange::Cancel {
                reason: "Dropped.".to_owned(),
            },
            "desk",
            T1,
        )
        .expect("cancelled");
    assert_eq!(cancelled.position, 0);
    assert_eq!(cancelled.column, "queued");
    assert_eq!(cancelled.revision, 2);
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_CANCELLED.to_owned(),
            json!({"id": "C-0002", "column": "queued", "as": "desk"})
        ))
    );
    assert_eq!(
        sqlite3(&db, "SELECT cancellation FROM cards WHERE id = 'C-0002';"),
        format!("{{\"by\":\"desk\",\"at\":\"{T1}\",\"reason\":\"Dropped.\"}}")
    );
    assert_eq!(
        board(&queue),
        [
            card("C-0001", "queued", 1),
            card("C-0002", "queued", 0),
            card("C-0003", "queued", 2),
        ]
    );
    let next = queue.create_card(&new_card(flow, "D"), T1).expect("a card");
    assert_eq!(next.position, 3);
    let dump = queue.dump().expect("dump");
    let reason = refusal(queue.change_card("C-0002", None, &to(flow, "blocked"), "local", T1));
    assert!(reason.contains("--corrects"), "{reason}");
    let reason =
        refusal(queue.change_card("C-0003", None, &before(flow, None, "C-0002"), "local", T1));
    assert!(
        reason.contains("--before"),
        "a cancelled card stands in no column: {reason}"
    );
    assert_eq!(queue.dump().expect("dump"), dump);
}

/// "Data", Fields, Evidence: an edit writes what it names, `None` leaves a
/// field, `Some(None)` clears it; `--evidence` appends `{kind, value, by,
/// at}`, 32 at most; `fields` names what was asked; a `parent` cycle is
/// refused, a self-parent too.
#[test]
fn edits_write_what_they_name_and_keep_links_acyclic() {
    let scratch = Scratch::new("qc-edit");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    for title in ["A", "B"] {
        queue
            .create_card(&new_card(flow, title), T0)
            .expect("a card");
    }
    let edit = |edit: CardEdit| CardChange::Edit(Box::new(edit));
    let edited = queue
        .change_card(
            "C-0001",
            Some(1),
            &edit(CardEdit {
                title: Some("Longer".to_owned()),
                goal: Some(Some("G.".to_owned())),
                targets: Some(vec!["SHT-BACK".to_owned()]),
                criteria: Some(vec!["One.".to_owned()]),
                estimate: Some(Some("3".to_owned())),
                due: Some(Some("2026-11-05".to_owned())),
                parent: Some(Some("C-0002".to_owned())),
                corrects: Some(Some("C-0002".to_owned())),
                evidence: vec![(EvidenceKind::Note, "Read twice.".to_owned())],
                reason: None,
                norms: None,
                tasks: Some(vec!["T-0002".to_owned(), "T-0001".to_owned()]),
                mandate: None,
            }),
            "ana",
            T1,
        )
        .expect("edited");
    assert_eq!(edited.title, "Longer");
    assert_eq!(edited.parent.as_deref(), Some("C-0002"));
    assert_eq!(edited.evidence.len(), 1);
    assert_eq!(edited.evidence[0].by, "ana");
    assert_eq!(edited.evidence[0].at, T1);
    assert_eq!(edited.tasks, ["T-0002", "T-0001"], "as given");
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0001", "fields": ["title", "goal", "targets", "criteria", "estimate",
                                              "due", "parent", "corrects", "evidence", "tasks"],
                   "as": "ana"})
        ))
    );
    assert_eq!(
        sqlite3(&db, "SELECT tasks FROM cards WHERE id = 'C-0001';"),
        "[\"T-0002\",\"T-0001\"]"
    );
    assert_eq!(
        sqlite3(&db, "SELECT evidence FROM cards WHERE id = 'C-0001';"),
        format!("[{{\"kind\":\"note\",\"value\":\"Read twice.\",\"by\":\"ana\",\"at\":\"{T1}\"}}]")
    );
    let cleared = queue
        .change_card(
            "C-0001",
            Some(2),
            &edit(CardEdit {
                goal: Some(None),
                parent: Some(None),
                ..CardEdit::default()
            }),
            "ana",
            T2,
        )
        .expect("cleared");
    assert_eq!(cleared.goal, None);
    assert_eq!(cleared.parent, None);
    assert_eq!(cleared.title, "Longer", "left");
    assert_eq!(cleared.corrects.as_deref(), Some("C-0002"), "left");
    assert_eq!(cleared.evidence.len(), 1, "left");
    assert_eq!(cleared.tasks, ["T-0002", "T-0001"], "left");

    let linked = |parent: &str| {
        edit(CardEdit {
            parent: Some(Some(parent.to_owned())),
            ..CardEdit::default()
        })
    };
    queue
        .change_card("C-0002", None, &linked("C-0001"), "ana", T2)
        .expect("a parent");
    let dump = queue.dump().expect("dump");
    for (id, parent) in [("C-0001", "C-0002"), ("C-0001", "C-0001")] {
        let reason = refusal(queue.change_card(id, None, &linked(parent), "ana", T2));
        assert!(reason.contains("parent"), "{reason}");
    }
    assert!(
        refusal(queue.change_card("C-0001", None, &linked("C-0099"), "ana", T2))
            .contains("no card C-0099")
    );
    let corrects = edit(CardEdit {
        corrects: Some(Some("C-0001".to_owned())),
        ..CardEdit::default()
    });
    let reason = refusal(queue.change_card("C-0001", None, &corrects, "ana", T2));
    assert!(reason.contains("corrects"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), dump);

    // The 33rd entry.
    let entries = |count: usize| CardEdit {
        evidence: vec![(EvidenceKind::Build, "b".to_owned()); count],
        ..CardEdit::default()
    };
    queue
        .change_card("C-0002", None, &edit(entries(32)), "ana", T2)
        .expect("32");
    let dump = queue.dump().expect("dump");
    let reason = refusal(queue.change_card("C-0002", None, &edit(entries(1)), "ana", T2));
    assert!(reason.contains("32"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), dump);
}

/// "Data", IDs: past `C-9999`, the highest plus one, and the highest of
/// every row, an accepted or cancelled one too.
#[test]
fn ids_are_the_highest_plus_one() {
    let scratch = Scratch::new("qc-ids");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    queue.create_card(&new_card(flow, "A"), T0).expect("a card");
    queue.create_card(&new_card(flow, "B"), T0).expect("a card");
    queue
        .change_card(
            "C-0002",
            None,
            &CardChange::Cancel {
                reason: "No.".to_owned(),
            },
            "local",
            T0,
        )
        .expect("cancelled");
    let made = queue.create_card(&new_card(flow, "C"), T0).expect("a card");
    assert_eq!(made.id, "C-0003");
    sqlite3(&db, "UPDATE cards SET id = 'C-9999' WHERE id = 'C-0003';");
    let made = queue.create_card(&new_card(flow, "D"), T0).expect("a card");
    assert_eq!(made.id, "C-10000");
    let made = queue.create_card(&new_card(flow, "E"), T0).expect("a card");
    assert_eq!(made.id, "C-10001");
    let ids: Vec<String> = queue
        .list_cards()
        .expect("list")
        .cards
        .into_iter()
        .map(|card| card.id)
        .collect();
    assert_eq!(
        ids,
        ["C-0001", "C-0002", "C-9999", "C-10000", "C-10001"],
        "by number"
    );
}

/// "Data", IDs: the highest number in the transaction: eight handles on one
/// file make forty cards with distinct IDs, one dense column, one event a
/// card. M: the ID read outside the write lock.
#[test]
fn eight_writers_make_distinct_ids_and_a_dense_column() {
    const WRITERS: usize = 8;
    const EACH: usize = 5;
    let scratch = Scratch::new("qc-writers");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    let config = press();
    std::thread::scope(|scope| {
        for writer in 0..WRITERS {
            let db = &db;
            let config = &config;
            scope.spawn(move || {
                let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
                for index in 0..EACH {
                    queue
                        .create_card(&new_card(plates(config), &format!("{writer}-{index}")), T0)
                        .expect("a card");
                }
            });
        }
    });
    let queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let cards = queue.list_cards().expect("list").cards;
    assert_eq!(cards.len(), WRITERS * EACH);
    let ids: BTreeSet<&str> = cards.iter().map(|card| card.id.as_str()).collect();
    let want: BTreeSet<String> = (1..=WRITERS * EACH)
        .map(|number| format!("C-{number:04}"))
        .collect();
    assert_eq!(
        ids,
        want.iter().map(String::as_str).collect::<BTreeSet<_>>()
    );
    let positions: BTreeSet<u64> = cards.iter().map(|card| card.position).collect();
    assert_eq!(
        positions,
        (1..=(WRITERS * EACH) as u64).collect::<BTreeSet<_>>(),
        "dense"
    );
    assert_eq!(card_events(&queue).len(), WRITERS * EACH);
}

/// "Documents and events": `card_history`: the last `limit` of this card
/// alone, oldest first, the other card's and the task's left out.
#[test]
fn history_is_the_cards_own_last_events() {
    let scratch = Scratch::new("qc-history");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    queue.create_card(&new_card(flow, "A"), T0).expect("a card");
    queue.create_card(&new_card(flow, "B"), T0).expect("a card");
    let edit = |goal: &str| {
        CardChange::Edit(Box::new(CardEdit {
            goal: Some(Some(goal.to_owned())),
            ..CardEdit::default()
        }))
    };
    for index in 0..5 {
        queue
            .change_card("C-0001", None, &edit(&format!("G{index}")), "local", T1)
            .expect("edited");
        queue
            .change_card("C-0002", None, &edit("Other."), "local", T1)
            .expect("edited");
    }
    let history = queue.card_history("C-0001", 3).expect("history");
    assert_eq!(history.len(), 3);
    assert!(history.windows(2).all(|pair| pair[0].seq < pair[1].seq));
    assert!(history.iter().all(|event| event.payload["id"] == "C-0001"));
    assert_eq!(
        queue.card_history("C-0001", 64).expect("history").len(),
        6,
        "created and five edits"
    );
    assert_eq!(queue.card_history("C-0099", 64).expect("history"), []);
}

// ------------------------------------------------- board-handoff (RM-0085)

/// The flows of `fixtures/studio-lamp`, the gamedev preset's: each with the
/// return column `sent-back` (board-handoff, `scripts/presets/gamedev.toml`).
fn lamp() -> CardConfig {
    let text = fs::read_to_string(fixture("studio-lamp").join("specengine.toml")).expect("config");
    CardConfig::from_toml(&text).expect("the fixture's [card]")
}

/// A move of the card to `column` with `note`.
fn noted<'f>(flow: &'f CardFlow, column: &str, note: Option<&str>) -> CardChange<'f> {
    CardChange::Move {
        flow: Some(flow),
        to: Some(column.to_owned()),
        before: None,
        note: note.map(str::to_owned),
    }
}

/// board-handoff BH-12 (docs/canon/cards.md "Documents and events"):
/// `create_card` stores a
/// reviewer named at creation and `card.created` carries it, `null` when
/// none; the store asks core's reviewer rule again, so `agent` and `local`
/// are refused with nothing stored. M14's store half.
#[test]
fn board_handoff_create_card_stores_the_reviewer_and_card_created_carries_it() {
    let scratch = Scratch::new("qc-handoff-reviewer");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    let narrative = config.flow("narrative").expect("narrative");
    let development = config.flow("development").expect("development");
    let parent = queue
        .create_card(
            &NewCard {
                by: "Ann".to_owned(),
                ..new_card(narrative, "Lamp room scene")
            },
            T0,
        )
        .expect("a card");
    assert_eq!(parent.reviewer, None);
    let child = queue
        .create_card(
            &NewCard {
                parent: Some(parent.id.clone()),
                reviewer: Some("Ann".to_owned()),
                by: "Ann".to_owned(),
                ..new_card(development, "Lamp room dialogue")
            },
            T0,
        )
        .expect("a hand-off");
    assert_eq!(child.reviewer.as_deref(), Some("Ann"));
    assert_eq!(child.parent.as_deref(), Some("C-0001"));
    assert_eq!(child.column, "backlog");
    assert_eq!(queue.get_card("C-0002").expect("get"), Some(child));
    assert_eq!(
        card_events(&queue),
        [
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0001", "flow": "narrative", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": null, "mandate": null,
                       "as": "Ann"})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0002", "flow": "development", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": "Ann", "mandate": null,
                       "as": "Ann"})
            ),
        ]
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT reviewer, parent FROM cards WHERE id = 'C-0002';"
        ),
        "Ann|C-0001"
    );
    let dump = queue.dump().expect("dump");
    for name in ["agent", "local"] {
        let reason = refusal(queue.create_card(
            &NewCard {
                reviewer: Some(name.to_owned()),
                ..new_card(development, "No one's")
            },
            T1,
        ));
        assert!(
            reason.contains(&format!(
                "`{name}` can be no reviewer: a person accepts a result"
            )),
            "{name}: {reason}"
        );
        assert_eq!(queue.dump().expect("dump"), dump, "{name}: nothing stored");
    }
}

/// board-handoff BH-14, R3 (docs/canon/card-flows.md "The rule",
/// docs/canon/cards.md "Rules"): the store asks
/// core's note rule again under the lock: a move into the flow's `return`
/// column from another column with no note, or a blank one, is refused with
/// the line of docs/canon/card-flows.md "Return" and `dump()` unchanged; with a note it moves and
/// `card.moved` holds it, `null` when none; a reorder inside the column and a
/// move out of it need none. M16, M17's store half.
#[test]
fn board_handoff_the_store_asks_the_return_rule_again_under_the_lock() {
    let scratch = Scratch::new("qc-handoff-return");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = lamp();
    let flow = config.flow("development").expect("development");
    assert_eq!(flow.return_column.as_deref(), Some("sent-back"));
    queue
        .create_card(&new_card(flow, "Lamp room dialogue"), T0)
        .expect("a card");
    queue
        .create_card(&new_card(flow, "Lamp light shader"), T0)
        .expect("a card");
    queue
        .change_card("C-0001", None, &noted(flow, "in-progress", None), "Rob", T1)
        .expect("taken, no note");
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_MOVED.to_owned(),
            json!({"id": "C-0001", "from": "backlog", "to": "in-progress", "position": 1,
                   "as": "Rob", "note": null})
        ))
    );
    let dump = queue.dump().expect("dump");
    let line = "C-0001 enters `sent-back`, its flow's return column: say why with `--note`; \
                nothing changed";
    for note in [None, Some(""), Some("  \n")] {
        assert_eq!(
            refusal(queue.change_card("C-0001", None, &noted(flow, "sent-back", note), "Rob", T2)),
            line,
            "{note:?}"
        );
        assert_eq!(
            queue.dump().expect("dump"),
            dump,
            "{note:?}: nothing changed"
        );
    }
    let moved = queue
        .change_card(
            "C-0001",
            None,
            &noted(flow, "sent-back", Some("Two endings contradict")),
            "Rob",
            T2,
        )
        .expect("returned with a reason");
    assert_eq!(moved.column, "sent-back");
    assert_eq!(moved.revision, 3);
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_MOVED.to_owned(),
            json!({"id": "C-0001", "from": "in-progress", "to": "sent-back", "position": 1,
                   "as": "Rob", "note": "Two endings contradict"})
        ))
    );
    let history = queue.card_history("C-0001", 64).expect("history");
    assert_eq!(
        history.last().map(|event| event.payload["note"].clone()),
        Some(json!("Two endings contradict")),
        "the move's own history entry is the note's home"
    );
    // The first column straight in: refused too, then sent with a note.
    let dump = queue.dump().expect("dump");
    assert_eq!(
        refusal(queue.change_card("C-0002", None, &noted(flow, "sent-back", None), "Rob", T2)),
        line.replace("C-0001", "C-0002")
    );
    assert_eq!(queue.dump().expect("dump"), dump);
    queue
        .change_card(
            "C-0002",
            None,
            &noted(flow, "sent-back", Some("Too dark")),
            "Rob",
            T2,
        )
        .expect("returned");
    // A reorder inside the return column: no note asked.
    queue
        .change_card(
            "C-0002",
            None,
            &CardChange::Move {
                flow: Some(flow),
                to: None,
                before: Some("C-0001".to_owned()),
                note: None,
            },
            "local",
            T2,
        )
        .expect("a reorder needs no note");
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_MOVED.to_owned(),
            json!({"id": "C-0002", "from": "sent-back", "to": "sent-back", "position": 1,
                   "as": "local", "note": null})
        ))
    );
    // Out of it: none asked; a note on any move is kept.
    queue
        .change_card("C-0001", None, &noted(flow, "ready", None), "Rob", T2)
        .expect("leaving needs no note");
    queue
        .change_card(
            "C-0002",
            None,
            &noted(flow, "in-review", Some("Lighter now")),
            "Rob",
            T2,
        )
        .expect("a note on any move");
    assert_eq!(
        card_events(&queue)
            .iter()
            .rev()
            .take(2)
            .map(|(_, payload)| payload["note"].clone())
            .collect::<Vec<_>>(),
        [json!("Lighter now"), Value::Null]
    );
    // card-press names no return column: its `blocked` is a word.
    let press = press();
    let plates = plates(&press);
    assert_eq!(plates.return_column, None);
    queue
        .create_card(&new_card(plates, "Back page"), T2)
        .expect("a card");
    queue
        .change_card("C-0003", None, &to(plates, "blocked"), "local", T2)
        .expect("no return column, no note asked");
}

/// A flow no fixture holds: explicit `moves`, the move into its `return`
/// column `lead`'s alone, a move out of it anyone's.
const RELAY: &str = r#"
[[card.flow]]
name = "relay"
disciplines = ["relay"]
columns = ["open", "back", "work", "check", "shut"]
accept = { from = "check", to = "shut" }
return = "back"
moves = [
  { from = "open", to = "work" },
  { from = "work", to = "back", by = ["lead"] },
  { from = "back", to = "work" },
  { from = "work", to = "check" },
]
"#;

/// docs/canon/card-flows.md "Return" (its order of a move's checks): core's
/// `card_move` judges the flow's move, then its `by`, then the return note.
/// So a move into the return column by a name outside its `by` is refused
/// with `by`'s line, note or none; one the flow does not list is refused
/// naming what is allowed, never the note's line; only a listed move by its
/// `by` asks for the note. The store asks it under its lock, `dump()`
/// unchanged on each refusal. M: the note asked before `by`.
#[test]
fn board_handoff_by_and_the_flows_moves_come_before_the_return_note() {
    let scratch = Scratch::new("qc-handoff-by");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = CardConfig::from_toml(RELAY).expect("the relay flow");
    let flow = config.flow("relay").expect("relay");
    assert_eq!(flow.return_column.as_deref(), Some("back"));
    queue
        .create_card(&new_card(flow, "Relay one"), T0)
        .expect("a card");
    queue
        .create_card(&new_card(flow, "Relay two"), T0)
        .expect("a card");
    queue
        .change_card("C-0001", None, &noted(flow, "work", None), "rob", T1)
        .expect("anyone's move");
    let dump = queue.dump().expect("dump");
    let not_by = "C-0001 is in work: the move to `back` is `lead`'s, not `rob`'s; nothing changed";
    for note in [None, Some(""), Some("Two endings contradict")] {
        assert_eq!(
            refusal(queue.change_card("C-0001", None, &noted(flow, "back", note), "rob", T2)),
            not_by,
            "{note:?}: `by` first"
        );
        assert_eq!(
            queue.dump().expect("dump"),
            dump,
            "{note:?}: nothing changed"
        );
    }
    // Not listed at all: the flow's own line, note or none.
    for note in [None, Some("Why.")] {
        assert_eq!(
            refusal(queue.change_card("C-0002", None, &noted(flow, "back", note), "lead", T2)),
            "C-0002 is in open: no move to `back`; allowed: `work`; nothing changed",
            "{note:?}"
        );
        assert_eq!(queue.dump().expect("dump"), dump, "{note:?}");
    }
    // Its `by`, no note: now the note's line.
    assert_eq!(
        refusal(queue.change_card("C-0001", None, &noted(flow, "back", None), "lead", T2)),
        "C-0001 enters `back`, its flow's return column: say why with `--note`; nothing changed"
    );
    assert_eq!(queue.dump().expect("dump"), dump);
    let moved = queue
        .change_card(
            "C-0001",
            None,
            &noted(flow, "back", Some("Two endings contradict")),
            "lead",
            T2,
        )
        .expect("the lead's return");
    assert_eq!(moved.column, "back");
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_MOVED.to_owned(),
            json!({"id": "C-0001", "from": "work", "to": "back", "position": 1,
                   "as": "lead", "note": "Two endings contradict"})
        ))
    );
    queue
        .change_card("C-0001", None, &noted(flow, "work", None), "rob", T2)
        .expect("out of it: anyone's, no note");
}

// --------------------------------------------------------- corrupt rows

/// "Rules and edge cases": each column that does not decode: `get_card`
/// fails naming the card and the column; `list_cards` skips it into
/// `unreadable`, the sound rows listed; a write on it changes nothing.
#[test]
fn a_corrupt_row_is_named_and_skipped() {
    let config = press();
    for (sql, id, column) in [
        (
            "UPDATE cards SET evidence = 'not json' WHERE id = 'C-0001'",
            "C-0001",
            "evidence",
        ),
        (
            "UPDATE cards SET targets = 'not json' WHERE id = 'C-0001'",
            "C-0001",
            "targets",
        ),
        (
            "UPDATE cards SET criteria = '{}' WHERE id = 'C-0001'",
            "C-0001",
            "criteria",
        ),
        (
            "UPDATE cards SET acceptance = 'x' WHERE id = 'C-0001'",
            "C-0001",
            "acceptance",
        ),
        (
            "UPDATE cards SET cancellation = '[]' WHERE id = 'C-0001'",
            "C-0001",
            "cancellation",
        ),
        (
            "UPDATE cards SET revision = 'two' WHERE id = 'C-0001'",
            "C-0001",
            "revision",
        ),
        (
            "UPDATE cards SET title = NULL WHERE id = 'C-0001'",
            "C-0001",
            "title",
        ),
        (
            "UPDATE cards SET position = NULL WHERE id = 'C-0001'",
            "C-0001",
            "position",
        ),
        ("UPDATE cards SET id = 'X' WHERE id = 'C-0001'", "X", "id"),
        (
            "UPDATE cards SET goal = CAST(x'ff' AS TEXT) WHERE id = 'C-0001'",
            "C-0001",
            "goal",
        ),
        // docs/canon/cards.md "Store": bad JSON, no array of strings,
        // an entry that is no task ID.
        (
            "UPDATE cards SET tasks = 'not json' WHERE id = 'C-0001'",
            "C-0001",
            "tasks",
        ),
        (
            "UPDATE cards SET tasks = '{}' WHERE id = 'C-0001'",
            "C-0001",
            "tasks",
        ),
        (
            "UPDATE cards SET tasks = '[1]' WHERE id = 'C-0001'",
            "C-0001",
            "tasks",
        ),
        (
            "UPDATE cards SET tasks = '[\"T-0001\",\"foo\"]' WHERE id = 'C-0001'",
            "C-0001",
            "tasks",
        ),
        (
            "UPDATE cards SET tasks = '[\"\u{0422}-0001\"]' WHERE id = 'C-0001'",
            "C-0001",
            "tasks",
        ),
    ] {
        let scratch = Scratch::new("qc-corrupt");
        let db = scratch.db("q");
        let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
        for title in ["Bad", "Sound"] {
            queue
                .create_card(&new_card(copy(&config), title), T0)
                .expect("a card");
        }
        sqlite3(&db, sql);
        let list = queue.list_cards().expect("list");
        assert_eq!(list.cards.len(), 1, "{sql}");
        assert_eq!(list.cards[0].id, "C-0002", "{sql}");
        assert_eq!(list.unreadable.len(), 1, "{sql}");
        assert_eq!(list.unreadable[0].id, id, "{sql}");
        assert_eq!(list.unreadable[0].column, column, "{sql}");
        if id == "X" {
            continue;
        }
        let error = queue.get_card(id).expect_err(sql).to_string();
        assert!(
            error.contains(id) && error.contains(column),
            "{sql}: {error}"
        );
        let dump = queue.dump().expect("dump");
        let tried = queue.change_card(id, None, &to(copy(&config), "edit"), "desk", T1);
        assert!(tried.is_err(), "{sql}");
        assert_eq!(queue.dump().expect("dump"), dump, "{sql}");
    }
}

// --------------------------------------------------------------- backup

fn stored_card(id: &str, project: &str, position: i64) -> StoredCard {
    let mut row = StoredCard {
        position: Some(position),
        columns: Default::default(),
    };
    row.columns[0] = Some(id.to_owned());
    row.columns[1] = Some(project.to_owned());
    row
}

fn stored_event(seq: i64) -> StoredEvent {
    StoredEvent {
        seq,
        columns: [
            Some(PROJECT.to_owned()),
            Some("card.created".to_owned()),
            Some("{\"id\":\"C-0001\"}".to_owned()),
            Some(T0.to_owned()),
        ],
    }
}

/// AC-07, the store half: `stored_rows` holds `cards` by number, raw (an
/// unreadable row too, `position` `NULL` kept), after the runs; `counts`
/// counts them; `restore` inserts them as given, `dump()` equal, the next ID
/// the highest restored plus one; a card alone occupies a queue; another
/// project's card is `Invalid`, nothing written. M: `stored_rows` skipping
/// `cards`.
#[test]
fn stored_rows_counts_and_restore_hold_cards() {
    let scratch = Scratch::new("qc-state");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    for title in ["A", "B"] {
        queue
            .create_card(&new_card(flow, title), T0)
            .expect("a card");
    }
    sqlite3(
        &db,
        "UPDATE cards SET id = 'C-10000', position = NULL WHERE id = 'C-0002';",
    );
    queue.create_card(&new_card(flow, "C"), T0).expect("a card");
    let stored = queue.stored_rows().expect("stored rows");
    let ids: Vec<&str> = stored.cards.iter().filter_map(StoredCard::id).collect();
    assert_eq!(ids, ["C-0001", "C-10000", "C-10001"], "by number");
    assert_eq!(stored.cards[1].position, None, "raw");
    assert_eq!(stored.cards[0].position, Some(1));
    assert_eq!(stored.cards[0].project(), Some(PROJECT));
    assert_eq!(
        queue.counts().expect("counts"),
        QueueCounts {
            proposals: 0,
            tasks: 0,
            runs: 0,
            cards: 3,
            events: 3,
        }
    );
    assert_eq!(stored.counts().cards, 3);
    let dump = queue.dump().expect("dump");
    let tables: Vec<&str> = dump
        .lines()
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    assert_eq!(
        tables,
        ["cards", "cards", "cards", "events", "events", "events"],
        "cards before events"
    );

    let fresh = scratch.db("fresh");
    let mut restored = SqliteQueue::open(&fresh, PROJECT).expect("open");
    assert_eq!(
        restored.restore(&stored).expect("restore"),
        Restore::Restored
    );
    assert_eq!(restored.dump().expect("dump"), dump, "byte for byte");
    assert_eq!(restored.stored_rows().expect("stored rows"), stored);
    let next = restored
        .create_card(&new_card(flow, "After"), T1)
        .expect("a card");
    assert_eq!(next.id, "C-10002");

    // A second restore: occupied, and by a card alone too.
    assert_eq!(
        restored.restore(&stored).expect("restore"),
        Restore::Occupied(restored.counts().expect("counts"))
    );
    let lone = scratch.db("lone");
    let mut queue = SqliteQueue::open(&lone, PROJECT).expect("open");
    queue.create_card(&new_card(flow, "A"), T0).expect("a card");
    sqlite3(&lone, "DELETE FROM events;");
    assert!(!queue.counts().expect("counts").is_empty(), "a card alone");
    assert_eq!(
        queue
            .restore(&StoredQueue {
                events: vec![stored_event(1)],
                ..StoredQueue::default()
            })
            .expect("restore"),
        Restore::Occupied(QueueCounts {
            proposals: 0,
            tasks: 0,
            runs: 0,
            cards: 1,
            events: 0,
        })
    );

    // Another project's card, a repeated ID: nothing written.
    let other = scratch.db("other");
    let mut queue = SqliteQueue::open(&other, PROJECT).expect("open");
    let empty = queue.dump().expect("dump");
    let wrong = StoredQueue {
        cards: vec![stored_card("C-0001", "another", 1)],
        ..StoredQueue::default()
    };
    assert!(matches!(queue.restore(&wrong), Err(QueueError::Invalid(_))));
    let repeated = StoredQueue {
        cards: vec![
            stored_card("C-0001", PROJECT, 1),
            stored_card("C-0001", PROJECT, 2),
        ],
        ..StoredQueue::default()
    };
    assert!(queue.restore(&repeated).is_err());
    assert_eq!(queue.dump().expect("dump"), empty, "all or nothing");
}

/// "Backup": a schema-8 DB read through `open_existing` is not stepped and
/// holds no cards.
#[test]
fn a_schema_8_database_reads_no_cards_and_is_not_stepped() {
    let scratch = Scratch::new("qc-existing");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    queue
        .create_card(&new_card(copy(&config), "A"), T0)
        .expect("a card");
    drop(queue);
    sqlite3(&db, "DROP TABLE cards; PRAGMA user_version = 8;");
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open")
        .expect("the file");
    let stored = queue.stored_rows().expect("stored rows");
    assert_eq!(stored.cards, []);
    assert_eq!(stored.events.len(), 1, "the event stays");
    assert_eq!(queue.counts().expect("counts").cards, 0);
    assert_eq!(queue.schema_version().expect("version"), 8);
    drop(queue);
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8");
}

// ----------------------------------------------------------------- gate

/// AC-08, the store half: `GateQueue::tasks` answers the same over sound
/// cards, a corrupt row, and with no `cards` table; the gate's source names
/// no card. M: `GateQueue::tasks` reading `cards`.
#[test]
fn the_gate_reads_tasks_alone() {
    let scratch = Scratch::new("qc-gate");
    let common_dir = scratch.join("repo/.git");
    fs::create_dir_all(&common_dir).expect("a repository");
    let common_dir = fs::canonicalize(common_dir).expect("canonical");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue
        .create_task(
            &NewTask {
                git_common_dir: common_dir.to_str().unwrap().to_owned(),
                title: Some("A task".to_owned()),
                goal: None,
                targets: vec!["SHT-FRONT".to_owned()],
                author: specengine_core::proposal::Author::human(),
            },
            T0,
        )
        .expect("a task");
    let config = press();
    queue
        .create_card(&new_card(copy(&config), "A card"), T0)
        .expect("a card");
    drop(queue);
    let tasks = || {
        let gate = open_for_gate(&db, PROJECT, Instant::now() + Duration::from_secs(30))
            .expect("open")
            .expect("the file");
        let (read, unreadable) = gate.tasks(&common_dir).expect("tasks");
        (format!("{read:?}"), format!("{unreadable:?}"))
    };
    let sound = tasks();
    assert!(sound.0.contains("T-0001"), "{sound:?}");
    assert!(!sound.0.contains("C-0001"), "{sound:?}");
    assert_eq!(sound.1, "[]");
    sqlite3(&db, "UPDATE cards SET tasks = '[\"T-0001\"]';");
    assert_eq!(tasks(), sound, "a card linking T-0001 (card-tasks CT-3)");
    sqlite3(&db, "UPDATE cards SET tasks = 'not json';");
    assert_eq!(tasks(), sound, "a card whose tasks do not read");
    sqlite3(
        &db,
        "UPDATE cards SET evidence = 'not json', id = 'X', project = NULL;",
    );
    assert_eq!(tasks(), sound, "a corrupt card row");
    sqlite3(&db, "DROP TABLE cards;");
    assert_eq!(tasks(), sound, "no cards table");

    let source =
        fs::read_to_string(repository_root().join("crates/specengine-store/src/queue/gate.rs"))
            .expect("the gate's source");
    assert!(source.contains("main.tasks"), "the scan reads the gate");
    assert!(!source.contains("cards"), "queue/gate.rs names no card");
}

// ------------------------------------------------ card-tasks (RM-0092)

/// CT-4, the store half: a schema-9 queue made with step 9's SQL, holding
/// one card, opens at 11 in one open (card-questions' step 11 too): `PRAGMA table_info(cards)` is
/// `CARD_COLUMNS` (25, `tasks` last), the old row's `tasks` `NULL`, read as
/// none, every other column as it was; once more, no second step. M6:
/// `STEP_10` left out of `migrate`.
#[test]
fn card_tasks_a_schema_9_queue_opens_at_10_and_its_card_links_none() {
    let scratch = Scratch::new("qc-step-10");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    // Schema 9's `proposals` holds no step-11 column either.
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN why; ALTER TABLE proposals DROP COLUMN card_id; \
         DROP TABLE cards;",
    );
    sqlite3(&db, STEP_9);
    sqlite3(
        &db,
        &format!(
            "INSERT INTO cards (id, project, flow, col, position, title, goal, discipline, \
             targets, snapshot, criteria, assignee, reviewer, estimate, due, parent, corrects, \
             evidence, acceptance, cancellation, created_by, created_at, updated_at, revision) \
             VALUES ('C-0001', '{PROJECT}', 'copy', 'draft', 1, 'Before the step', NULL, \
             'copy', '[]', NULL, '[]', NULL, NULL, NULL, NULL, NULL, NULL, '[]', NULL, NULL, \
             'local', '{T0}', '{T0}', '1'); PRAGMA user_version = 9;"
        ),
    );
    let columns = |db: &Path| {
        sqlite3(
            db,
            "SELECT group_concat(name, ' ') FROM (SELECT name FROM pragma_table_info('cards') \
             ORDER BY cid);",
        )
    };
    assert_eq!(columns(&db), COLUMNS[..24].join(" "), "premise: schema 9");
    let row = |db: &Path| {
        sqlite3(
            db,
            &format!("SELECT {} FROM cards;", COLUMNS[..24].join(", ")),
        )
    };
    let before = row(&db);

    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    // 12 since docs/features/task-mandate.md M5.
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "12");
    assert_eq!(columns(&db), CARD_COLUMNS.join(" "));
    // 26 since docs/features/task-mandate.md M5 (`mandate`).
    assert_eq!(CARD_COLUMNS.len(), 26);
    assert_eq!(
        sqlite3(&db, "SELECT tasks IS NULL FROM cards WHERE id = 'C-0001';"),
        "1",
        "a stepped row holds NULL"
    );
    assert_eq!(row(&db), before, "every other column as it was");
    let card = queue.get_card("C-0001").expect("read").expect("the card");
    assert_eq!(card.tasks, Vec::<String>::new(), "NULL reads as none");
    assert_eq!(card.title, "Before the step");
    let listed = queue.list_cards().expect("list");
    assert_eq!(listed.cards.len(), 1);
    assert!(listed.unreadable.is_empty(), "{:?}", listed.unreadable);

    // A card made after the step holds `'[]'`; an edit writes the list.
    let config = press();
    let made = queue
        .create_card(&new_card(copy(&config), "After the step"), T1)
        .expect("a card");
    assert_eq!(made.tasks, Vec::<String>::new());
    assert_eq!(
        sqlite3(&db, "SELECT tasks FROM cards WHERE id = 'C-0002';"),
        "[]"
    );
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "12", "stepped once");
    assert_eq!(columns(&db), CARD_COLUMNS.join(" "), "no second column");
}

/// CT-1, the store half (D2-D5): `CardEdit.tasks` replaces the list whole in
/// the order given, `Some` of none clears it, `None` leaves it; each one
/// `card.edited` naming `tasks`, `revision` one higher even when the list is
/// unchanged. Core's grammar, repeat and cap are asked again under the lock
/// (a CLI that skipped them changes nothing), and no task is looked up: an
/// ID this queue holds no task of is stored (D4: the existence check is the
/// caller's). A repeat or a long list a hand edit left reads as stored.
#[test]
fn card_tasks_an_edit_replaces_the_list_and_the_store_asks_the_rules_again() {
    let scratch = Scratch::new("qc-tasks");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    queue
        .create_card(&new_card(copy(&config), "Masthead"), T0)
        .expect("a card");
    let edit = |tasks: Option<Vec<&str>>| {
        CardChange::Edit(Box::new(CardEdit {
            tasks: tasks.map(|tasks| tasks.into_iter().map(str::to_owned).collect()),
            ..CardEdit::default()
        }))
    };
    let stored = || sqlite3(&db, "SELECT tasks FROM cards WHERE id = 'C-0001';");
    assert_eq!(stored(), "[]");
    let mut events = card_events(&queue);
    for (tasks, want, revision) in [
        (vec!["T-0042", "T-0001"], "[\"T-0042\",\"T-0001\"]", 2),
        (vec!["T-0042", "T-0001"], "[\"T-0042\",\"T-0001\"]", 3),
        (vec![], "[]", 4),
    ] {
        let card = queue
            .change_card(
                "C-0001",
                Some(revision - 1),
                &edit(Some(tasks.clone())),
                "ana",
                T1,
            )
            .expect("edited");
        assert_eq!(card.tasks, tasks, "{tasks:?}");
        assert_eq!(card.revision, revision, "{tasks:?}");
        assert_eq!(stored(), want, "{tasks:?}");
        events.push((
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0001", "fields": ["tasks"], "as": "ana"}),
        ));
        assert_eq!(card_events(&queue), events, "{tasks:?}");
    }
    // `None` leaves it.
    queue
        .change_card("C-0001", None, &edit(Some(vec!["T-0007"])), "ana", T1)
        .expect("edited");
    let goal = CardChange::Edit(Box::new(CardEdit {
        goal: Some(Some("G.".to_owned())),
        ..CardEdit::default()
    }));
    let card = queue
        .change_card("C-0001", None, &goal, "ana", T2)
        .expect("edited");
    assert_eq!(card.tasks, ["T-0007"], "left");
    assert_eq!(stored(), "[\"T-0007\"]");

    // Under the lock: core's rules again.
    let dump = queue.dump().expect("dump");
    let many: Vec<String> = (1..=33).map(|n| format!("T-{n:04}")).collect();
    for (tasks, reason) in [
        (
            vec!["foo".to_owned()],
            "task: \"foo\" is no task ID; nothing changed",
        ),
        (
            vec!["T-0001".to_owned(), "-".to_owned()],
            "task: \"-\" is no task ID; nothing changed",
        ),
        (
            vec!["T-0001".to_owned(), "T-0001".to_owned()],
            "task: T-0001 is listed twice; nothing changed",
        ),
        (many.clone(), "task: 33 tasks, at most 32; nothing changed"),
    ] {
        let change = CardChange::Edit(Box::new(CardEdit {
            tasks: Some(tasks.clone()),
            ..CardEdit::default()
        }));
        assert_eq!(
            refusal(queue.change_card("C-0001", None, &change, "ana", T2)),
            reason,
            "{tasks:?}"
        );
        assert_eq!(
            queue.dump().expect("dump"),
            dump,
            "{tasks:?}: nothing changed"
        );
    }
    let card = queue
        .change_card(
            "C-0001",
            None,
            &CardChange::Edit(Box::new(CardEdit {
                tasks: Some(many[..32].to_vec()),
                ..CardEdit::default()
            })),
            "ana",
            T2,
        )
        .expect("32 stored");
    assert_eq!(card.tasks, many[..32]);

    // A hand edit's repeat and 33 entries read as stored.
    let thirty_three = serde_json::to_string(&many).expect("JSON");
    for written in ["[\"T-0001\",\"T-0001\"]".to_owned(), thirty_three] {
        sqlite3(
            &db,
            &format!("UPDATE cards SET tasks = '{written}' WHERE id = 'C-0001';"),
        );
        let card = queue.get_card("C-0001").expect("read").expect("the card");
        let want: Vec<String> = serde_json::from_str(&written).expect("JSON");
        assert_eq!(card.tasks, want, "read as stored");
        assert!(queue.list_cards().expect("list").unreadable.is_empty());
    }
    // `NULL` reads as none.
    sqlite3(&db, "UPDATE cards SET tasks = NULL WHERE id = 'C-0001';");
    let card = queue.get_card("C-0001").expect("read").expect("the card");
    assert_eq!(card.tasks, Vec::<String>::new());
}

/// R7, the store half: an accept leaves the links; a closed card takes no
/// link edit (the closed refusal); a correction starts with none.
#[test]
fn card_tasks_an_accept_keeps_the_links_and_a_closed_card_takes_no_link() {
    let scratch = Scratch::new("qc-tasks-closed");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    let mut card = new_card(flow, "Back page");
    card.reviewer = Some("ben".to_owned());
    queue.create_card(&card, T0).expect("a card");
    queue
        .change_card(
            "C-0001",
            None,
            &CardChange::Edit(Box::new(CardEdit {
                tasks: Some(vec!["T-0001".to_owned()]),
                ..CardEdit::default()
            })),
            "ana",
            T0,
        )
        .expect("linked");
    queue
        .change_card("C-0001", None, &to(flow, "checked"), "ana", T1)
        .expect("moved");
    let norms = TargetNorms::new();
    queue
        .accept_card(
            "C-0001",
            None,
            &CardAccept {
                flow: Some(flow),
                note: None,
                norms: &norms,
            },
            "ben",
            T1,
        )
        .expect("accepted");
    let accepted = queue.get_card("C-0001").expect("read").expect("the card");
    assert!(accepted.acceptance.is_some());
    assert_eq!(accepted.tasks, ["T-0001"], "the accept leaves the links");
    let dump = queue.dump().expect("dump");
    let reason = refusal(queue.change_card(
        "C-0001",
        None,
        &CardChange::Edit(Box::new(CardEdit {
            tasks: Some(Vec::new()),
            ..CardEdit::default()
        })),
        "ana",
        T2,
    ));
    assert!(reason.contains("--corrects"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), dump, "nothing changed");
    let mut correction = new_card(flow, "Back page again");
    correction.corrects = Some("C-0001".to_owned());
    let made = queue.create_card(&correction, T2).expect("a correction");
    assert_eq!(
        made.tasks,
        Vec::<String>::new(),
        "a correction starts with none"
    );
}

// ---------------------------------------------- upstream-signal (RM-0105)

/// docs/features/upstream-signal.md US-5, the store's half:
/// `events_of_types` over 200 mixed events (`card.created`, `.edited`,
/// `.moved`, `.assigned`, `proposal.created`, `.applied`, another project's
/// rows) returns exactly the asked types of this project, by `seq`,
/// uncapped, a payload that is no JSON passed over. The oracle is the raw
/// table through `sqlite3`. M: `card_history(…, 64)` (a cap); a type filter
/// dropped; the project unfiltered.
#[test]
fn upstream_events_of_types_is_every_asked_event_by_seq() {
    const ASKED: [&str; 3] = [EVENT_CARD_CREATED, EVENT_CARD_EDITED, "proposal.applied"];
    let scratch = Scratch::new("qc-upstream-events");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = plates(&config);
    for index in 0..10 {
        queue
            .create_card(&new_card(flow, &format!("Plate {index}")), T0)
            .expect("a card");
    }
    let edit = |goal: String| {
        CardChange::Edit(Box::new(CardEdit {
            goal: Some(Some(goal)),
            ..CardEdit::default()
        }))
    };
    let raw = |event_type: &str, payload: &str, project: &str| {
        sqlite3(
            &db,
            &format!(
                "INSERT INTO events (project, type, payload, at) VALUES ('{project}', \
                 '{event_type}', '{payload}', '{T1}');"
            ),
        );
    };
    let mut round = 0;
    while sqlite3(&db, "SELECT COUNT(*) FROM events;")
        .parse::<usize>()
        .expect("a count")
        < 200
    {
        let id = format!("C-{:04}", round % 10 + 1);
        queue
            .change_card(&id, None, &edit(format!("Goal {round}.")), "ana", T1)
            .expect("edited");
        queue
            .change_card(&id, None, &to(flow, "queued"), "ana", T1)
            .expect("reordered");
        let item = format!("PR-{:04}", round + 1);
        raw(
            "proposal.created",
            &format!("{{\"id\":\"{item}\"}}"),
            PROJECT,
        );
        raw(
            "proposal.applied",
            &format!("{{\"id\":\"{item}\"}}"),
            PROJECT,
        );
        raw(
            "card.edited",
            "{\"id\":\"C-0001\",\"fields\":[\"goal\"]}",
            "elsewhere",
        );
        round += 1;
    }
    let total: usize = sqlite3(&db, "SELECT COUNT(*) FROM events;")
        .parse()
        .expect("a count");
    assert!(total >= 200, "{total}");
    let broken: i64 = sqlite3(
        &db,
        &format!(
            "SELECT seq FROM events WHERE project = '{PROJECT}' AND type = 'card.edited' \
             ORDER BY seq LIMIT 1 OFFSET 7;"
        ),
    )
    .parse()
    .expect("a seq");
    sqlite3(
        &db,
        &format!("UPDATE events SET payload = 'not json {{' WHERE seq = {broken};"),
    );
    let expected: Vec<i64> = sqlite3(
        &db,
        &format!(
            "SELECT seq FROM events WHERE project = '{PROJECT}' AND type IN ('card.created', \
             'card.edited', 'proposal.applied') AND seq <> {broken} ORDER BY seq;"
        ),
    )
    .lines()
    .map(|seq| seq.parse().expect("a seq"))
    .collect();
    assert!(
        expected.len() > 64,
        "past the history cap: {}",
        expected.len()
    );

    let found = queue.events_of_types(&ASKED).expect("the events");
    let seqs: Vec<i64> = found.iter().map(|event| event.seq).collect();
    assert_eq!(seqs, expected, "every asked event of this project, by seq");
    assert!(
        found
            .iter()
            .all(|event| event.project == PROJECT && ASKED.contains(&event.event_type.as_str())),
        "only the asked types of this project"
    );
    assert!(
        found.iter().all(|event| event.payload["id"].is_string()),
        "payloads decoded"
    );
    assert!(
        queue
            .events_of_types(&["card.assigned"])
            .expect("none")
            .is_empty()
    );
    let moved = queue
        .events_of_types(&[EVENT_CARD_MOVED])
        .expect("the moves");
    assert_eq!(moved.len(), round, "one reorder a round");
}
