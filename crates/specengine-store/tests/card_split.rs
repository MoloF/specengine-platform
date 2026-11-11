//! docs/features/department-split.md (RM-0100, v5 step 6c, ADR-0078)
//! "Acceptance criteria", the store half: `SqliteQueue::split_cards` through
//! the public API, the flows read from `fixtures/card-press/specengine.toml`
//! by core's reader (docs/canon/card-split.md "The rule").
//!
//! - S3 (store): one transaction stores a new parent first, then each child
//!   in the order asked, each child's `parent` set to the parent's ID
//!   whatever the input held; one `card.created` each, `new`'s payload; a
//!   child's snapshot as given, an empty one stored as `create_card` stores
//!   none.
//! - S4 (store): a stored parent is read, never written: its row,
//!   `revision`, `updated_at` and history byte-equal, no event naming it, in
//!   any state.
//! - S5: a child refused (`reviewer` `agent`) rolls everything back: no row,
//!   no event, the next `create_card` takes `C-0001`; `Stored("C-0099")` and
//!   no child each `CardRefused`, nothing written. M: each card committed
//!   alone (the first stays); the children's `parent` taken from the input.
//!
//! The raw oracle of "nothing written" is the system's `sqlite3` over
//! `cards` and `events`. Every expected value is worked out from the spec,
//! never read back from a run.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{Scratch, fixture};
use serde_json::{Value, json};
use specengine_core::card_toml::{CardConfig, CardFlow};
use specengine_model::CardFrozen;
use specengine_store::{
    CardChange, EVENT_CARD_CREATED, NewCard, NewSplit, NewSplitChild, ProposalQueue as _,
    QueueError, SplitCards, SplitParent, SqliteQueue,
};

const T0: &str = "2026-11-11T09:00:00Z";
const T1: &str = "2026-11-11T09:00:01Z";
const PROJECT: &str = "card-press";
const REVIEWER: &str = "Ana Ruiz";

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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The flows of `fixtures/card-press`, through core's reader.
fn press() -> CardConfig {
    let text = fs::read_to_string(fixture("card-press").join("specengine.toml")).expect("config");
    CardConfig::from_toml(&text).expect("the fixture's [card]")
}

fn flow<'c>(config: &'c CardConfig, name: &str) -> &'c CardFlow {
    config
        .flow(name)
        .unwrap_or_else(|| panic!("the {name} flow"))
}

/// A card of `flow` as the split makes it: the parent's title and goal, the
/// split's reviewer, no assignee, criteria, estimate or due.
fn split_card(
    flow: &CardFlow,
    discipline: &str,
    targets: &[&str],
    parent: Option<&str>,
) -> NewCard {
    NewCard {
        flow: flow.name.clone(),
        column: flow.first_column().to_owned(),
        title: "The weekly run".to_owned(),
        goal: Some("Out by Thursday.".to_owned()),
        discipline: Some(discipline.to_owned()),
        targets: targets.iter().map(|target| (*target).to_owned()).collect(),
        criteria: Vec::new(),
        assignee: None,
        reviewer: Some(REVIEWER.to_owned()),
        estimate: None,
        due: None,
        parent: parent.map(str::to_owned),
        corrects: None,
        by: REVIEWER.to_owned(),
    }
}

fn frozen(targets: &[&str]) -> Vec<CardFrozen> {
    targets
        .iter()
        .map(|target| CardFrozen {
            id: (*target).to_owned(),
            rev: Some(1),
            norm_hash: Some("b3:frozen".to_owned()),
            within_hash: Some("b3:frozen".to_owned()),
            also: Vec::new(),
        })
        .collect()
}

fn child(card: NewCard, snapshot: Vec<CardFrozen>) -> NewSplitChild {
    NewSplitChild { card, snapshot }
}

/// A new parent on `PRN-WEEKLY` and two children: `copy` on the sheets, its
/// input `parent` none; `plates` on the run, its input `parent` a card that
/// is not there. The transaction sets both.
fn weekly(config: &CardConfig) -> NewSplit {
    NewSplit {
        parent: SplitParent::New {
            card: Box::new(split_card(
                flow(config, "copy"),
                "copy",
                &["PRN-WEEKLY"],
                None,
            )),
            snapshot: frozen(&["PRN-WEEKLY"]),
        },
        children: vec![
            child(
                split_card(
                    flow(config, "copy"),
                    "copy",
                    &["SHT-BACK", "SHT-FRONT"],
                    None,
                ),
                frozen(&["SHT-BACK", "SHT-FRONT"]),
            ),
            child(
                split_card(
                    flow(config, "plates"),
                    "plates",
                    &["PRN-WEEKLY", "PRN-PLATES"],
                    Some("C-0042"),
                ),
                Vec::new(),
            ),
        ],
    }
}

/// The reason of a refused split.
fn refusal(result: Result<SplitCards, QueueError>) -> String {
    match result {
        Err(QueueError::CardRefused { reason, .. }) => reason,
        other => panic!("expected CardRefused, got {other:?}"),
    }
}

/// `(type, payload)` of every event.
fn events(queue: &SqliteQueue) -> Vec<(String, Value)> {
    queue
        .events()
        .expect("events")
        .into_iter()
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The raw rows of `cards` and `events`: what a refusal must leave.
fn raw(db: &Path) -> String {
    sqlite3(
        db,
        "SELECT * FROM cards ORDER BY id; SELECT seq, type, payload FROM events ORDER BY seq;",
    )
}

fn created(id: &str, flow: &str, column: &str, position: u64) -> (String, Value) {
    (
        EVENT_CARD_CREATED.to_owned(),
        json!({"id": id, "flow": flow, "column": column, "position": position,
               "assignee": null, "reviewer": REVIEWER, "as": REVIEWER}),
    )
}

// ------------------------------------------------------------------ S3

/// S3, the store half: the new parent first, then the children in the
/// order asked, each child's `parent` the parent's ID (the input's none and
/// `C-0042` alike), three `card.created` in that order, each `new`'s
/// payload; a child's snapshot as given, an empty one stored as
/// `create_card` stores a card with none. M: the children's `parent` taken
/// from the input.
#[test]
fn s3_a_split_stores_the_parent_then_each_child() {
    let config = press();
    let scratch = Scratch::new("split-store");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let done = queue.split_cards(&weekly(&config), T0).expect("split");
    assert!(done.created, "a new parent");
    assert_eq!(done.parent.id, "C-0001");
    assert_eq!(done.parent.parent, None);
    assert_eq!(
        (done.parent.column.as_str(), done.parent.position),
        ("draft", 1)
    );
    let ids: Vec<&str> = done.children.iter().map(|card| card.id.as_str()).collect();
    assert_eq!(ids, ["C-0002", "C-0003"], "in the order asked");
    for card in &done.children {
        assert_eq!(card.parent.as_deref(), Some("C-0001"), "{}", card.id);
        assert_eq!(card.reviewer.as_deref(), Some(REVIEWER));
        assert_eq!(card.title, "The weekly run");
        assert_eq!(card.goal.as_deref(), Some("Out by Thursday."));
        assert_eq!(card.assignee, None);
        assert_eq!(card.revision, 1);
        assert_eq!(
            (card.created_at.as_str(), card.updated_at.as_str()),
            (T0, T0)
        );
    }
    assert_eq!(
        (
            done.children[0].column.as_str(),
            done.children[0].position,
            done.children[1].column.as_str(),
            done.children[1].position
        ),
        ("draft", 2, "queued", 1),
        "each last in its flow's first column"
    );
    assert_eq!(
        events(&queue),
        [
            created("C-0001", "copy", "draft", 1),
            created("C-0002", "copy", "draft", 2),
            created("C-0003", "plates", "queued", 1),
        ]
    );
    // As stored: the result is the row.
    for card in std::iter::once(&done.parent).chain(&done.children) {
        assert_eq!(
            queue.get_card(&card.id).expect("get").as_ref(),
            Some(card),
            "{}",
            card.id
        );
    }
    assert_eq!(
        done.children[0].snapshot.as_deref(),
        Some(&frozen(&["SHT-BACK", "SHT-FRONT"])[..])
    );
    // An empty snapshot is none, as `create_card` stores it.
    let plain = queue
        .create_card(
            &split_card(flow(&config, "plates"), "plates", &["PRN-PLATES"], None),
            T1,
        )
        .expect("a card");
    assert_eq!(plain.id, "C-0004");
    assert_eq!(done.children[1].snapshot, plain.snapshot);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT snapshot IS NULL FROM cards WHERE id = 'C-0003';"
        ),
        sqlite3(
            &db,
            "SELECT snapshot IS NULL FROM cards WHERE id = 'C-0004';"
        ),
        "the raw column too"
    );
}

/// S3 (D2): a child of the parent's own discipline is a child like any
/// other; the children land after the cards already in their column.
#[test]
fn s3_children_land_last_and_a_child_of_the_parents_discipline_is_one() {
    let config = press();
    let scratch = Scratch::new("split-last");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let earlier = queue
        .create_card(
            &split_card(flow(&config, "plates"), "plates", &[], None),
            T0,
        )
        .expect("a card");
    assert_eq!((earlier.id.as_str(), earlier.position), ("C-0001", 1));
    let done = queue.split_cards(&weekly(&config), T1).expect("split");
    assert_eq!(done.parent.id, "C-0002");
    assert_eq!(done.parent.discipline.as_deref(), Some("copy"));
    assert_eq!(done.children[0].discipline.as_deref(), Some("copy"));
    assert_eq!(done.children[0].parent.as_deref(), Some("C-0002"));
    assert_eq!(
        (done.children[1].id.as_str(), done.children[1].position),
        ("C-0004", 2),
        "after the plates card already queued"
    );
}

// ------------------------------------------------------------------ S4

/// S4, the store half: `Stored(C-0001)` takes the card as it is, in any
/// state: `created` false, the parent the stored row; its row, `revision`,
/// `updated_at` and history byte-equal; the one event added the child's.
/// M: the parent's `revision` raised.
#[test]
fn s4_a_stored_parent_is_read_never_written() {
    let config = press();
    let scratch = Scratch::new("split-stored");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue
        .create_card(
            &split_card(flow(&config, "copy"), "copy", &["PRN-WEEKLY"], None),
            T0,
        )
        .expect("a card");
    let one_child = |input_parent: Option<&str>| NewSplit {
        parent: SplitParent::Stored("C-0001".to_owned()),
        children: vec![child(
            split_card(
                flow(&config, "plates"),
                "plates",
                &["PRN-WEEKLY"],
                input_parent,
            ),
            Vec::new(),
        )],
    };
    for (round, state) in ["open", "cancelled"].into_iter().enumerate() {
        let before = queue.get_card("C-0001").expect("get").expect("C-0001");
        let row = sqlite3(&db, "SELECT * FROM cards WHERE id = 'C-0001';");
        let events_before = events(&queue);
        let done = queue.split_cards(&one_child(None), T1).expect("split");
        assert!(!done.created, "{state}");
        assert_eq!(done.parent, before, "{state}: the stored row");
        let id = format!("C-{:04}", round + 2);
        assert_eq!(done.children.len(), 1);
        assert_eq!(done.children[0].id, id);
        assert_eq!(done.children[0].parent.as_deref(), Some("C-0001"));
        assert_eq!(
            sqlite3(&db, "SELECT * FROM cards WHERE id = 'C-0001';"),
            row,
            "{state}: the parent's row byte-equal"
        );
        let after = events(&queue);
        assert_eq!(after[..events_before.len()], events_before[..]);
        let added = &after[events_before.len()..];
        assert_eq!(added.len(), 1, "{state}: {added:?}");
        assert_eq!(added[0].0, EVENT_CARD_CREATED);
        assert_eq!(added[0].1["id"], json!(id));
        assert!(
            added.iter().all(|(_, payload)| payload["id"] != "C-0001"),
            "{state}: no event names the parent"
        );
        // Any state is taken: the second round splits under a cancelled one.
        if round == 0 {
            queue
                .change_card(
                    "C-0001",
                    None,
                    &CardChange::Cancel {
                        reason: "Pulled.".to_owned(),
                    },
                    "desk",
                    T1,
                )
                .expect("cancelled");
        }
    }
}

// ------------------------------------------------------------------ S5

/// S5: the second child's reviewer `agent`: `CardRefused`, `reviewer: …`;
/// no card row, no event, and the next `create_card` takes `C-0001`. M:
/// each card committed alone (the first stays).
#[test]
fn s5_a_refused_child_rolls_the_whole_split_back() {
    let config = press();
    let scratch = Scratch::new("split-none");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let before = raw(&db);
    let mut split = weekly(&config);
    split.children[1].card.reviewer = Some("agent".to_owned());
    assert_eq!(
        refusal(queue.split_cards(&split, T0)),
        "reviewer: `agent` can be no reviewer: a person accepts a result"
    );
    assert_eq!(raw(&db), before, "no row, no event");
    assert!(queue.list_cards().expect("list").cards.is_empty());
    assert!(events(&queue).is_empty());
    let next = queue
        .create_card(&split_card(flow(&config, "copy"), "copy", &[], None), T1)
        .expect("a card");
    assert_eq!(next.id, "C-0001", "no ID taken");
}

/// S5: a stored parent that is not there, and a split naming no child,
/// each `CardRefused` with their reasons, nothing written.
#[test]
fn s5_no_parent_and_no_child_are_refused() {
    let config = press();
    let scratch = Scratch::new("split-refused");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let before = raw(&db);
    let mut split = weekly(&config);
    split.parent = SplitParent::Stored("C-0099".to_owned());
    assert_eq!(
        refusal(queue.split_cards(&split, T0)),
        "parent: no card C-0099"
    );
    assert_eq!(raw(&db), before);
    let mut split = weekly(&config);
    split.children.clear();
    assert_eq!(refusal(queue.split_cards(&split, T0)), "child: none named");
    assert_eq!(raw(&db), before, "no parent stored either");
    // A refused first child: the parent goes with it.
    let mut split = weekly(&config);
    split.children[0].card.reviewer = Some("local".to_owned());
    assert_eq!(
        refusal(queue.split_cards(&split, T0)),
        "reviewer: `local` can be no reviewer: a person accepts a result"
    );
    assert_eq!(raw(&db), before);
    let done = queue.split_cards(&weekly(&config), T1).expect("split");
    assert_eq!(done.parent.id, "C-0001", "the refused splits took no ID");
}
