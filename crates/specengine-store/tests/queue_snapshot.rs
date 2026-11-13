//! docs/features/release-hardening.md "Acceptance criteria" RH-R5, the
//! store's half: `SqliteQueue::snapshot`, one deferred read transaction
//! over the queue, holds the delivery read's three reads (the cards, the
//! events of types, the items) in the snapshot its first read takes, while a
//! second connection commits. Rows are written through the store with fixed
//! `now`s on a scratch queue of an invented bindery (`copy` and `plates`).
//!
//! - After the handle's card read, a second connection commits a
//!   `card.accepted` and a question naming the card: the handle's events
//!   and items reads show neither; a new handle shows both, and the card
//!   accepted. M: each read on its own transaction.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run.

#![cfg(unix)]

mod common;

use common::Scratch;
use specengine_core::card_toml::CardConfig;
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_core::trace::TargetNorms;
use specengine_store::{
    CardAccept, EVENT_CARD_ACCEPTED, Intake, NewCard, NewIntake, Place, ProposalFilter,
    ProposalKind, ProposalQueue as _, SqliteQueue,
};

const PROJECT: &str = "atlas-bindery";
const REVIEWER: &str = "bob";

/// One flow, its accept from `review` to `done`.
const FLOWS: &str = r#"
[[card.flow]]
name = "copy"
title = "Copy"
disciplines = ["copy"]
columns = ["todo", "review", "done"]
accept = { from = "review", to = "done" }
"#;

fn at(day: u32) -> String {
    format!("2026-11-{day:02}T09:00:00Z")
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

/// The card in `review`, its reviewer [`REVIEWER`].
fn card() -> NewCard {
    NewCard {
        flow: "copy".to_owned(),
        column: "review".to_owned(),
        title: "Write the foreword".to_owned(),
        goal: None,
        discipline: Some("copy".to_owned()),
        targets: Vec::new(),
        criteria: Vec::new(),
        assignee: None,
        reviewer: Some(REVIEWER.to_owned()),
        mandate: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: None,
        by: "local".to_owned(),
    }
}

/// A question on `ATL-FOREWORD` for `card`, as an agent raises it.
fn question(card: &str) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/atlas.md".to_owned(),
        place: place(),
        author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: vec!["ATL-FOREWORD".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: "Does the foreword name the printer?".to_owned(),
            working_answer: Some("As the first edition did.".to_owned()),
            price_of_other: Some("A page reprinted.".to_owned()),
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

/// Every item, any status, any repository.
fn every_item() -> ProposalFilter {
    ProposalFilter {
        git_common_dir: None,
        statuses: Vec::new(),
    }
}

/// RH-R5: the handle's three reads share the snapshot of its first; a
/// second connection's commit after the card read is seen by a new handle
/// alone. M: each read on its own transaction.
#[test]
fn rh_r5_delivery_reads_one_snapshot() {
    let scratch = Scratch::new("rh-r5-snapshot");
    let db = scratch.join("queue.db");
    let config = CardConfig::from_toml(FLOWS).expect("the bindery's [card]");
    let copy = config.flow("copy").expect("copy").clone();
    let mut writer = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = writer.create_card(&card(), &at(1)).expect("the card").id;
    assert_eq!(id, "C-0001");

    let reader = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open the existing queue")
        .expect("a queue");
    let snapshot = reader.snapshot().expect("a snapshot");
    let cards = snapshot.existing_cards().expect("the card read");
    assert_eq!(cards.cards.len(), 1);
    assert_eq!(cards.cards[0].column, "review", "open, before the accept");

    // A second connection commits a `card.accepted` and a question naming
    // the card.
    let norms = TargetNorms::new();
    writer
        .accept_card(
            &id,
            None,
            &CardAccept {
                flow: Some(&copy),
                note: None,
                norms: &norms,
            },
            REVIEWER,
            &at(2),
        )
        .expect("the accept commits");
    let stored = writer
        .create_intake(&question(&id), &[], None, &at(2))
        .expect("the question commits")
        .created
        .expect("stored")
        .id;
    assert_eq!(stored, "PR-0001");

    let events = snapshot
        .events_of_types(&[EVENT_CARD_ACCEPTED])
        .expect("the events read");
    assert_eq!(events, Vec::new(), "the handle sees no accept");
    let items = snapshot
        .list_readable(&every_item())
        .expect("the items read");
    assert_eq!(
        items.proposals.len(),
        0,
        "the handle sees no item: {:?}",
        items.proposals
    );
    drop(snapshot);

    let fresh = reader.snapshot().expect("a new snapshot");
    let cards = fresh.existing_cards().expect("the card read");
    assert_eq!(cards.cards[0].column, "done", "accepted");
    let events = fresh
        .events_of_types(&[EVENT_CARD_ACCEPTED])
        .expect("the events read");
    assert_eq!(events.len(), 1, "{events:?}");
    let items = fresh.list_readable(&every_item()).expect("the items read");
    let ids: Vec<&str> = items
        .proposals
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(ids, ["PR-0001"]);
}
