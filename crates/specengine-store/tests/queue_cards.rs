//! docs/features/card-store.md, the store half ("Store", "Backup", "Rules
//! and edge cases"): `SqliteQueue`'s cards through the public API, the flows
//! read from `fixtures/card-press/specengine.toml` by core's reader; the raw
//! steps (an older schema, a corrupt row) use the system's `sqlite3`.
//!
//! - Schema 9: `cards` as "Store" gives it, 24 columns in `CARD_COLUMNS`
//!   order, `STRICT`, `TEXT` but `position`; a schema-8 queue steps once, its
//!   rows kept, `events` and `proposals` unchanged.
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

/// The columns as "Store" lists them, written here.
const COLUMNS: [&str; 24] = [
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
];

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
    }
}

fn before<'f>(flow: &'f CardFlow, column: Option<&str>, card: &str) -> CardChange<'f> {
    CardChange::Move {
        flow: Some(flow),
        to: column.map(str::to_owned),
        before: Some(card.to_owned()),
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
/// the one `INTEGER`, `STRICT`; `CARD_COLUMNS` is that list, 24 of them. M: a
/// column moved or retyped.
#[test]
fn schema_9_makes_the_cards_table() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 9);
    assert_eq!(CARD_COLUMNS, COLUMNS);
    let scratch = Scratch::new("qc-schema");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "9");
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
/// an event in it) opens at 9, stepped once, every row byte for byte;
/// `cards` empty, a card made after. M: the step skipped.
#[test]
fn a_schema_8_queue_opens_at_9_with_its_rows_unchanged() {
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
    sqlite3(&db, "DROP TABLE cards; PRAGMA user_version = 8;");
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
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "9");
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
                       "assignee": "ana", "as": "writer"})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0002", "flow": "copy", "column": "draft", "position": 2,
                       "assignee": null, "as": "local"})
            ),
            (
                EVENT_CARD_CREATED.to_owned(),
                json!({"id": "C-0003", "flow": "plates", "column": "queued", "position": 1,
                       "assignee": null, "as": "local"})
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
            json!({"id": "C-0001", "from": "draft", "to": "edit", "position": 1, "as": "desk"})
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
                    "as": "local"})
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
    assert_eq!(
        card_events(&queue).last(),
        Some(&(
            EVENT_CARD_EDITED.to_owned(),
            json!({"id": "C-0001", "fields": ["title", "goal", "targets", "criteria", "estimate",
                                              "due", "parent", "corrects", "evidence"],
                   "as": "ana"})
        ))
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
