//! docs/features/card-trace.md, the store half ("Accepting", "Clearing",
//! "Snapshot and suspect", "Data"): the queue's card-trace API through the
//! public surface, the flows read from `fixtures/card-press/specengine.toml`
//! by core's reader, what a target reads now given as core's `TargetNorms`;
//! raw steps use the system's `sqlite3`.
//!
//! - `accept_departing`: one `Immediate` transaction, `proposal.created` then
//!   `card.accepted`; a refusal or a failure after the insert leaves neither
//!   (AC-04's M: the insert in its own transaction); hits never refuse, the
//!   item names the first 64 (AC-05's M: the dedup's refusal kept).
//! - `clear_card`, `clear_cards`: judged under the lock, one event each, each
//!   suspect entry frozen again and no other; a correction leaves the original
//!   alone (AC-06's M: re-freezing at the clear); a group is one transaction.
//! - `accept_card`: a correction's accept re-freezes what it corrects.
//! - `applied_vouching`: `also` of every entry holding the old hash, one
//!   `card.refreshed` each, the last 16, a corrupt row left alone.
//! - `list_existing_cards`, `node_norms`: reads, no schema step, no table.
//! - docs/features/trace-truth.md "Editorial refresh": one event per (card,
//!   node), `revision` once, `also` the kind the entry compares.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{Corpus, Scratch, fixture};
use serde_json::{Value, json};
use specengine_core::card_toml::{CardConfig, CardFlow};
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_core::trace::{ALSO_MAX, HashKind, TargetNorms, TargetNow};
use specengine_model::{CardFrozen, CardVerdict};
use specengine_store::{
    CardAccept, CardChange, CardClear, CardClearing, CardDeparture, CardEdit, CardRefresh,
    Decision, Intake, NewCard, NewIntake, NewProposal, Place, ProposalKind, ProposalQueue as _,
    ProposalStatus, QueueError, SqliteQueue, b3_hash, patch_hash,
};

const T0: &str = "2026-10-30T09:00:00Z";
const T1: &str = "2026-10-30T09:00:01Z";
const T2: &str = "2026-10-30T09:00:02Z";
const PROJECT: &str = "card-press";
const OLD: &str = "b3:old";
const NEW: &str = "b3:new";
const INNER: &str = "SHT-INNER";

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

/// `targets` reading `hash` now, at `rev`.
fn norms(targets: &[&str], rev: u32, hash: &str) -> TargetNorms {
    targets
        .iter()
        .map(|target| {
            (
                (*target).to_owned(),
                TargetNow {
                    rev: Some(rev),
                    norm_hash: Some(hash.to_owned()),
                    within_hash: Some(hash.to_owned()),
                },
            )
        })
        .collect()
}

/// `targets` frozen at `hash`, `also` empty.
fn frozen(targets: &[&str], hash: &str) -> Vec<CardFrozen> {
    targets
        .iter()
        .map(|target| CardFrozen {
            id: (*target).to_owned(),
            rev: Some(1),
            norm_hash: Some(hash.to_owned()),
            within_hash: Some(hash.to_owned()),
            also: Vec::new(),
        })
        .collect()
}

fn new_card(flow: &CardFlow, title: &str, targets: &[&str], corrects: Option<&str>) -> NewCard {
    NewCard {
        flow: flow.name.clone(),
        column: flow.first_column().to_owned(),
        title: title.to_owned(),
        goal: None,
        discipline: Some("copy".to_owned()),
        targets: targets.iter().map(|target| (*target).to_owned()).collect(),
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: corrects.map(str::to_owned),
        by: "local".to_owned(),
    }
}

/// A card on `targets` frozen at [`OLD`]: its ID.
fn make(queue: &mut SqliteQueue, title: &str, targets: &[&str]) -> String {
    let config = press();
    queue
        .create_card_frozen(
            &new_card(copy(&config), title, targets, None),
            &frozen(targets, OLD),
            T0,
        )
        .expect("a card")
        .id
}

/// The card walked to `proof`, `ben` its reviewer.
fn to_proof(queue: &mut SqliteQueue, id: &str) {
    let config = press();
    let flow = copy(&config);
    queue
        .change_card(
            id,
            None,
            &CardChange::Assign {
                assignee: None,
                reviewer: Some(Some("ben".to_owned())),
            },
            "desk",
            T0,
        )
        .expect("assigned");
    for column in ["edit", "proof"] {
        queue
            .change_card(
                id,
                None,
                &CardChange::Move {
                    flow: Some(flow),
                    to: Some(column.to_owned()),
                    before: None,
                    note: None,
                },
                "desk",
                T0,
            )
            .expect("moved");
    }
}

/// The card at `proof`, accepted as briefed against `now`.
fn accepted(queue: &mut SqliteQueue, id: &str, now: &TargetNorms) {
    to_proof(queue, id);
    let config = press();
    queue
        .accept_card(
            id,
            None,
            &CardAccept {
                flow: Some(copy(&config)),
                note: None,
                norms: now,
            },
            "ben",
            T1,
        )
        .expect("accepted");
}

/// A departure's item from `targets`.
fn departure(targets: &[&str], summary: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "not priced at acceptance".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/spec/sht-inner.md".to_owned(),
        place: Place {
            git_common_dir: "/r/.git".to_owned(),
            worktree: "/r".to_owned(),
            root_rel: String::new(),
            branch: "main".to_owned(),
            base_commit: "1".repeat(40),
        },
        author: Author::human(),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            severity: IntakeSeverity::Normal,
            gap_type: Some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "C-0001".to_owned(),
                qpath: None,
                lines: None,
                observed: "below the fold".to_owned(),
                documented: "above the fold".to_owned(),
            }],
            options: vec![option("change the brief"), option("rework the result")],
            recommendation: Some(0),
            distinct_from: Vec::new(),
            why: None,
        },
        actor: None,
        card_id: None,
    }
}

/// `accept_departing` of `id` from `targets`, the corpus's `hits`.
fn departing(
    queue: &mut SqliteQueue,
    id: &str,
    seen: Option<u64>,
    targets: &[&str],
    hits: &[String],
    now: &TargetNorms,
) -> Result<specengine_store::CardAccepted, QueueError> {
    let config = press();
    let item = departure(targets, "The result left the brief.");
    let departs: Vec<String> = targets.iter().map(|id| (*id).to_owned()).collect();
    queue.accept_departing(
        id,
        seen,
        &CardAccept {
            flow: Some(copy(&config)),
            note: Some("See the fold."),
            norms: now,
        },
        &CardDeparture {
            intake: &item,
            corpus_hits: hits,
            departs: &departs,
        },
        "ben",
        T1,
    )
}

/// `ben`'s clear for `reason` against `now`.
fn by_ben<'a>(reason: &'a str, now: &'a TargetNorms) -> CardClear<'a> {
    CardClear {
        reason,
        norms: now,
        actor: "ben",
    }
}

/// `(type, payload)` of every event of the queue.
fn all_events(queue: &SqliteQueue) -> Vec<(String, Value)> {
    queue
        .events()
        .expect("events")
        .into_iter()
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The payloads of `kind`.
fn payloads(queue: &SqliteQueue, kind: &str) -> Vec<Value> {
    all_events(queue)
        .into_iter()
        .filter(|(event, _)| event == kind)
        .map(|(_, payload)| payload)
        .collect()
}

/// The reason of a refused change.
fn refusal(result: Result<impl std::fmt::Debug, QueueError>) -> String {
    match result {
        Err(QueueError::CardRefused { reason, .. }) => reason,
        other => panic!("expected CardRefused, got {other:?}"),
    }
}

// --------------------------------------------------------- accept_departing

/// "Accepting": the item and the accept together, `proposal.created` then
/// `card.accepted`, the row `{by, at, note, verdict, departs, discrepancy}`,
/// the snapshot frozen anew, the item `open`.
#[test]
fn accept_departing_writes_both_in_order() {
    let scratch = Scratch::new("qt-departs");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Long read", &[INNER]);
    to_proof(&mut queue, &id);
    let now = norms(&[INNER], 2, NEW);
    let done = departing(&mut queue, &id, None, &[INNER], &[], &now).expect("accepted");
    let card = &done.card;
    assert_eq!(card.column, "filed");
    let acceptance = card.acceptance.as_ref().expect("an acceptance");
    assert_eq!(acceptance.by, "ben");
    assert_eq!(acceptance.at, T1);
    assert_eq!(acceptance.note.as_deref(), Some("See the fold."));
    assert_eq!(acceptance.verdict, CardVerdict::Departs);
    assert_eq!(acceptance.departs, [INNER]);
    assert_eq!(acceptance.discrepancy.as_deref(), Some("PR-0001"));
    let entry = &card.snapshot.as_ref().expect("a snapshot")[0];
    assert_eq!(entry.norm_hash.as_deref(), Some(NEW), "frozen anew");
    assert_eq!(entry.rev, Some(2));

    let result = done.intake.as_ref().expect("the item");
    let item = result.created.as_ref().expect("stored");
    assert_eq!(item.id, "PR-0001");
    assert_eq!(item.status, ProposalStatus::Open);
    assert_eq!(item.kind, ProposalKind::Discrepancy);

    let tail: Vec<String> = all_events(&queue)
        .into_iter()
        .rev()
        .take(2)
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(tail, ["card.accepted", "proposal.created"], "last first");
    let event = payloads(&queue, "card.accepted").pop().expect("the event");
    assert_eq!(event["verdict"], "departs", "{event}");
    assert_eq!(event["discrepancy"], "PR-0001", "{event}");
}

/// AC-04's "failure injected after the insert": a trigger refusing the
/// `card.accepted` event, the transaction's last write: an error, no item,
/// no event, the card's row as it was; the next call takes the same ID. M: the
/// insert in its own transaction.
#[test]
fn a_failure_after_the_insert_leaves_neither() {
    let scratch = Scratch::new("qt-failure");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Long read", &[INNER]);
    to_proof(&mut queue, &id);
    let rows = queue.dump().expect("dump");
    sqlite3(
        &db,
        "CREATE TRIGGER injected BEFORE INSERT ON events WHEN NEW.type = 'card.accepted' BEGIN \
         SELECT RAISE(ABORT, 'injected'); END;",
    );
    let now = norms(&[INNER], 2, NEW);
    let failed = departing(&mut queue, &id, None, &[INNER], &[], &now);
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), "0");
    assert_eq!(queue.dump().expect("dump"), rows, "every row as it was");

    sqlite3(&db, "DROP TRIGGER injected;");
    let done = departing(&mut queue, &id, None, &[INNER], &[], &now).expect("accepted");
    assert_eq!(
        done.card.acceptance.expect("an acceptance").discrepancy,
        Some("PR-0001".to_owned()),
        "no ID burnt"
    );
}

/// "Accepting", refused as `accept` is: a stale `seen`, a name that is no
/// reviewer's, a card not at `accept.from`, a closed card: nothing stored.
#[test]
fn a_refused_departure_leaves_neither() {
    let scratch = Scratch::new("qt-refused");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let now = norms(&[INNER], 2, NEW);
    let early = make(&mut queue, "Early", &[INNER]);
    let rows = queue.dump().expect("dump");
    refusal(departing(&mut queue, &early, None, &[INNER], &[], &now));
    assert_eq!(queue.dump().expect("dump"), rows, "not at `proof`");

    to_proof(&mut queue, &early);
    let rows = queue.dump().expect("dump");
    refusal(departing(&mut queue, &early, Some(1), &[INNER], &[], &now));
    assert_eq!(queue.dump().expect("dump"), rows, "stale");

    departing(&mut queue, &early, None, &[INNER], &[], &now).expect("accepted");
    let rows = queue.dump().expect("dump");
    refusal(departing(&mut queue, &early, None, &[INNER], &[], &now));
    assert_eq!(queue.dump().expect("dump"), rows, "closed");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), "1");
}

/// AC-05: hits never refuse: the corpus's first, then the queue's, named by
/// the accept itself; `create_intake` still refuses the same item. M: the
/// dedup's refusal kept.
#[test]
fn hits_are_named_by_the_accept() {
    let scratch = Scratch::new("qt-hits");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let first = make(&mut queue, "First", &[INNER]);
    let second = make(&mut queue, "Second", &[INNER]);
    to_proof(&mut queue, &first);
    to_proof(&mut queue, &second);
    let now = norms(&[INNER], 2, NEW);
    departing(&mut queue, &first, None, &[INNER], &[], &now).expect("accepted");

    // The same item through the intake door: a hit, nothing stored.
    let result = queue
        .create_intake(
            &departure(&[INNER], "The result left the brief."),
            &[],
            None,
            T2,
        )
        .expect("an answer");
    assert_eq!(result.created, None, "the intake's own dedup stands");
    assert_eq!(result.hits.len(), 1);

    let corpus = ["STY-0001".to_owned()];
    let done = departing(&mut queue, &second, None, &[INNER], &corpus, &now).expect("accepted");
    let result = done.intake.expect("the item");
    let item = result.created.expect("stored");
    assert_eq!(
        item.intake.expect("an intake").distinct_from,
        ["STY-0001", "PR-0001"],
        "the corpus's first"
    );
    assert_eq!(
        result
            .hits
            .iter()
            .map(|hit| hit.id.as_str())
            .collect::<Vec<_>>(),
        ["PR-0001"],
        "the caller lists them"
    );
}

/// "The dedup never refuses a departure": past `DISTINCT_MAX` the first 64
/// are named; a name the column cannot hold is passed over, never a failure.
#[test]
fn past_the_cap_the_item_names_the_first_sixty_four() {
    let scratch = Scratch::new("qt-cap");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Long read", &[INNER]);
    to_proof(&mut queue, &id);
    let mut corpus: Vec<String> = vec!["x".repeat(257), "bad\nname".to_owned()];
    corpus.extend((0..70).map(|number| format!("STY-{number:04}")));
    let now = norms(&[INNER], 2, NEW);
    let done = departing(&mut queue, &id, None, &[INNER], &corpus, &now).expect("accepted");
    let item = done.intake.expect("the item").created.expect("stored");
    let named = item.intake.expect("an intake").distinct_from;
    assert_eq!(named.len(), 64, "{named:?}");
    assert_eq!(named[0], "STY-0000", "what cannot be named is passed over");
    assert_eq!(named[63], "STY-0063");
}

// ------------------------------------------------------------- clear_card

/// "No effect": each suspect entry frozen again and no other, the event naming
/// exactly those, `revision` one more; the mark is gone and a later change
/// marks again.
#[test]
fn a_clear_freezes_each_suspect_entry_and_no_other() {
    let scratch = Scratch::new("qt-clear");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Two sheets", &["SHT-FRONT", "SHT-BACK"]);
    let mut now = norms(&["SHT-FRONT"], 2, NEW);
    now.extend(norms(&["SHT-BACK"], 1, OLD));
    let revision = queue.get_card(&id).expect("read").expect("a card").revision;
    let done = queue
        .clear_card(
            &id,
            Some(revision),
            &CardClearing::NoEffect,
            &by_ben("Only the weather.", &now),
            T1,
        )
        .expect("cleared");
    assert_eq!(done.correction, None);
    assert_eq!(done.card.revision, revision + 1);
    let snapshot = done.card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].norm_hash.as_deref(), Some(NEW));
    assert_eq!(snapshot[0].rev, Some(2));
    assert_eq!(snapshot[1].norm_hash.as_deref(), Some(OLD));
    assert_eq!(snapshot[1].rev, Some(1), "the clean entry as it was");
    assert_eq!(
        payloads(&queue, "card.cleared"),
        [json!({
            "id": id,
            "outcome": "no_effect",
            "nodes": [{"id": "SHT-FRONT", "rev": 2, "from": OLD, "to": NEW}],
            "correction": null,
            "reason": "Only the weather.",
            "as": "ben",
        })]
    );

    let reason = refusal(queue.clear_card(
        &id,
        None,
        &CardClearing::NoEffect,
        &by_ben("Again.", &now),
        T2,
    ));
    assert!(reason.contains("not suspect"), "{reason}");
}

/// "Clearing", judged under the lock: a stale `seen`, a cancelled card, no
/// such card and a mark a live correction holds are refused; nothing changes.
#[test]
fn a_clear_is_judged_under_the_lock() {
    let scratch = Scratch::new("qt-clear-refused");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    let id = make(&mut queue, "Lead story", &[INNER]);
    let now = norms(&[INNER], 2, NEW);
    let rows = queue.dump().expect("dump");
    let try_clear = |queue: &mut SqliteQueue, card: &str, seen: Option<u64>| {
        queue.clear_card(card, seen, &CardClearing::NoEffect, &by_ben("R.", &now), T1)
    };
    refusal(try_clear(&mut queue, &id, Some(99)));
    refusal(try_clear(&mut queue, "C-0099", None));
    assert_eq!(queue.dump().expect("dump"), rows);

    let gone = make(&mut queue, "Pulled", &[INNER]);
    queue
        .change_card(
            &gone,
            None,
            &CardChange::Cancel {
                reason: "Pulled.".to_owned(),
            },
            "desk",
            T0,
        )
        .expect("cancelled");
    let rows = queue.dump().expect("dump");
    refusal(try_clear(&mut queue, &gone, None));
    assert_eq!(queue.dump().expect("dump"), rows, "cancelled");

    // A live correction holds the mark.
    queue
        .create_card_frozen(
            &new_card(flow, "By hand", &[INNER], Some(&id)),
            &frozen(&[INNER], NEW),
            T0,
        )
        .expect("a correction");
    let rows = queue.dump().expect("dump");
    let reason = refusal(try_clear(&mut queue, &id, None));
    assert!(reason.contains("correcting C-0003"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), rows, "held");
}

/// AC-06: needs correction, accepted cards only: `card.created`, then
/// `card.cleared` naming it; the original's row untouched, its snapshot, its
/// revision. M: re-freezing at the clear.
#[test]
fn a_correction_leaves_the_original_alone() {
    let scratch = Scratch::new("qt-correction");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    let id = make(&mut queue, "Lead story", &[INNER]);
    let fix = new_card(flow, "Correct Lead story", &[INNER], Some(&id));
    let now = norms(&[INNER], 2, NEW);
    let rows = queue.dump().expect("dump");
    refusal(queue.clear_card(
        &id,
        None,
        &CardClearing::NeedsCorrection(&fix),
        &by_ben("R.", &now),
        T1,
    ));
    assert_eq!(queue.dump().expect("dump"), rows, "not accepted");

    accepted(&mut queue, &id, &norms(&[INNER], 1, OLD));
    let before = queue.get_card(&id).expect("read").expect("a card");
    let done = queue
        .clear_card(
            &id,
            None,
            &CardClearing::NeedsCorrection(&fix),
            &by_ben("The lead moved.", &now),
            T2,
        )
        .expect("cleared");
    let correction = done.correction.expect("the correction");
    assert_eq!(correction.corrects.as_deref(), Some(id.as_str()));
    assert_eq!(
        correction.snapshot.expect("a snapshot")[0]
            .norm_hash
            .as_deref(),
        Some(NEW),
        "a new card frozen now"
    );
    let after = queue.get_card(&id).expect("read").expect("a card");
    assert_eq!(after, before, "the original's row untouched");

    let kinds: Vec<String> = all_events(&queue)
        .into_iter()
        .rev()
        .take(2)
        .map(|(kind, _)| kind)
        .collect();
    assert_eq!(kinds, ["card.cleared", "card.created"], "last first");
    let event = payloads(&queue, "card.cleared").pop().expect("the event");
    assert_eq!(event["outcome"], "needs_correction");
    assert_eq!(event["correction"], correction.id.as_str());
    assert_eq!(
        event["nodes"],
        json!([{"id": INNER, "rev": 2, "from": OLD, "to": NEW}])
    );
}

/// "Accepted, it re-freezes the original", one `card.cleared` of `corrected`;
/// nothing moved, no event; a correction's departing accept too.
#[test]
fn accepting_a_correction_refreezes_what_it_corrects() {
    let scratch = Scratch::new("qt-corrected");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    let id = make(&mut queue, "Lead story", &[INNER]);
    accepted(&mut queue, &id, &norms(&[INNER], 1, OLD));
    let now = norms(&[INNER], 2, NEW);
    let fix = queue
        .create_card_frozen(
            &new_card(flow, "Correct it", &[INNER], Some(&id)),
            &frozen(&[INNER], NEW),
            T1,
        )
        .expect("a correction")
        .id;
    to_proof(&mut queue, &fix);
    let done = queue
        .accept_card(
            &fix,
            None,
            &CardAccept {
                flow: Some(flow),
                note: Some("Fixed."),
                norms: &now,
            },
            "ben",
            T2,
        )
        .expect("accepted");
    assert_eq!(done.corrected.as_deref(), Some(id.as_str()));
    let original = queue.get_card(&id).expect("read").expect("a card");
    assert_eq!(
        original.snapshot.expect("a snapshot")[0]
            .norm_hash
            .as_deref(),
        Some(NEW)
    );
    let event = payloads(&queue, "card.cleared").pop().expect("the event");
    assert_eq!(event["id"], id.as_str());
    assert_eq!(event["outcome"], "corrected");
    assert_eq!(event["correction"], fix.as_str());
    assert_eq!(event["as"], "ben");

    // Nothing to move: no event.
    let count = payloads(&queue, "card.cleared").len();
    let later = queue
        .create_card_frozen(
            &new_card(flow, "Another", &[INNER], Some(&id)),
            &frozen(&[INNER], NEW),
            T1,
        )
        .expect("a correction")
        .id;
    to_proof(&mut queue, &later);
    let done = queue
        .accept_card(
            &later,
            None,
            &CardAccept {
                flow: Some(flow),
                note: None,
                norms: &now,
            },
            "ben",
            T2,
        )
        .expect("accepted");
    assert_eq!(done.corrected, None);
    assert_eq!(payloads(&queue, "card.cleared").len(), count);
}

/// AC-07: `clear_cards`: one event each, in the order asked; one card refused
/// leaves every card as it was, whichever comes first.
#[test]
fn a_group_is_one_transaction() {
    let scratch = Scratch::new("qt-group");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let first = make(&mut queue, "First", &[INNER]);
    let second = make(&mut queue, "Second", &[INNER]);
    let revision =
        |queue: &SqliteQueue, id: &str| queue.get_card(id).expect("read").expect("a card").revision;
    let asked = vec![
        (first.clone(), revision(&queue, &first)),
        (second.clone(), revision(&queue, &second) + 7),
    ];
    let now = norms(&[INNER], 2, NEW);
    let rows = queue.dump().expect("dump");
    let scope = BTreeSet::from([INNER.to_owned()]);
    refusal(queue.clear_cards(&asked, &scope, &by_ben("R.", &now), T1));
    assert_eq!(queue.dump().expect("dump"), rows, "the first put back");

    let asked = vec![
        (first.clone(), revision(&queue, &first)),
        (second.clone(), revision(&queue, &second)),
    ];
    let cleared = queue
        .clear_cards(&asked, &scope, &by_ben("A reprint.", &now), T1)
        .expect("cleared");
    assert_eq!(cleared.len(), 2);
    assert_eq!(
        payloads(&queue, "card.cleared")
            .iter()
            .map(|event| event["id"].as_str().expect("an ID").to_owned())
            .collect::<Vec<_>>(),
        [first, second]
    );
}

/// AC-07, "Rules and edge cases": a live correction row with bad bytes, in its
/// `corrects` or its `id`, stops no group clear: it holds no mark, and the
/// cards it does not hold clear, one event each.
#[test]
fn a_correction_row_with_bad_bytes_stops_no_group() {
    let scratch = Scratch::new("qt-bad-bytes");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Lead story", &[INNER]);
    let bad_corrects = make(&mut queue, "Bad corrects", &[INNER]);
    let bad_id = make(&mut queue, "Bad id", &[INNER]);
    sqlite3(
        &db,
        &format!(
            "UPDATE cards SET corrects = CAST(X'FF' AS TEXT) WHERE id = '{bad_corrects}'; \
             UPDATE cards SET corrects = '{id}', id = CAST(X'FE' AS TEXT) WHERE id = '{bad_id}';"
        ),
    );
    let revision = queue.get_card(&id).expect("read").expect("a card").revision;
    let now = norms(&[INNER], 2, NEW);
    let scope = BTreeSet::from([INNER.to_owned()]);
    let cleared = queue
        .clear_cards(
            &[(id.clone(), revision)],
            &scope,
            &by_ben("A reprint.", &now),
            T1,
        )
        .expect("the bad rows stop nothing");
    assert_eq!(cleared.len(), 1);
    assert_eq!(payloads(&queue, "card.cleared")[0]["id"], id.as_str());
}

/// AC-07, "a mark outside it stays": `clear_cards` over `scope`: a card suspect
/// only outside refuses the whole group, under the lock; a card suspect on
/// both re-freezes its in-scope entry alone, the event naming that one, and
/// `clear_card` reaches the rest.
#[test]
fn a_group_reaches_its_scope_alone() {
    let scratch = Scratch::new("qt-scope");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let both = make(&mut queue, "Both", &[INNER, "SHT-BACK"]);
    let outside = make(&mut queue, "Outside", &["SHT-BACK"]);
    let now = norms(&[INNER, "SHT-BACK"], 2, NEW);
    let scope = BTreeSet::from([INNER.to_owned()]);
    let seen =
        |queue: &SqliteQueue, id: &str| queue.get_card(id).expect("read").expect("a card").revision;
    let asked = vec![
        (both.clone(), seen(&queue, &both)),
        (outside.clone(), seen(&queue, &outside)),
    ];
    let rows = queue.dump().expect("dump");
    let reason = refusal(queue.clear_cards(&asked, &scope, &by_ben("R.", &now), T1));
    assert!(reason.contains("not suspect"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), rows, "the first put back");

    let asked = vec![(both.clone(), seen(&queue, &both))];
    queue
        .clear_cards(&asked, &scope, &by_ben("R.", &now), T1)
        .expect("cleared");
    let card = queue.get_card(&both).expect("read").expect("a card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].norm_hash.as_deref(), Some(NEW), "in scope");
    assert_eq!(snapshot[1].norm_hash.as_deref(), Some(OLD), "outside");
    assert_eq!(
        payloads(&queue, "card.cleared")[0]["nodes"],
        json!([{"id": INNER, "rev": 2, "from": OLD, "to": NEW}])
    );

    queue
        .clear_card(
            &both,
            None,
            &CardClearing::NoEffect,
            &by_ben("The rest.", &now),
            T2,
        )
        .expect("the rest");
    let card = queue.get_card(&both).expect("read").expect("a card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[1].norm_hash.as_deref(), Some(NEW));
}

/// "A person's act", under the lock too: `agent` and `local` clear nothing,
/// the words a clear's, nothing changed.
#[test]
fn a_clear_is_a_persons_under_the_lock() {
    let scratch = Scratch::new("qt-person");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Lead story", &[INNER]);
    let now = norms(&[INNER], 2, NEW);
    let rows = queue.dump().expect("dump");
    for name in ["agent", "local"] {
        let clear = CardClear {
            reason: "R.",
            norms: &now,
            actor: name,
        };
        let reason = refusal(queue.clear_card(&id, None, &CardClearing::NoEffect, &clear, T1));
        assert_eq!(
            reason,
            format!("{id}: `{name}` clears no mark: a person answers one; nothing changed")
        );
    }
    assert_eq!(queue.dump().expect("dump"), rows);
}

/// "Either re-freezes", the reading of a target with nothing to freeze: a null
/// hash now (a front matter broken at this moment) and a target gone keep
/// their entries, so the card stays comparable; one that reads takes its hash.
#[test]
fn an_accept_freezes_only_what_has_a_hash() {
    let scratch = Scratch::new("qt-refrozen");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Three", &[INNER, "SHT-BACK", "SHT-FRONT"]);
    let mut now = norms(&["SHT-FRONT"], 2, NEW);
    now.insert(
        INNER.to_owned(),
        TargetNow {
            rev: Some(2),
            norm_hash: None,
            within_hash: None,
        },
    );
    accepted(&mut queue, &id, &now);
    let card = queue.get_card(&id).expect("read").expect("a card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].norm_hash.as_deref(), Some(OLD), "a null hash");
    assert_eq!(snapshot[0].rev, Some(1));
    assert_eq!(snapshot[1].norm_hash.as_deref(), Some(OLD), "gone");
    assert_eq!(snapshot[2].norm_hash.as_deref(), Some(NEW), "reads");
}

/// A correction's accept moves what its read holds: a target of the original
/// the read does not hold (gone, or taken after the read) is left as it is,
/// its mark too.
#[test]
fn a_correction_refreezes_only_what_it_read() {
    let scratch = Scratch::new("qt-partial");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    let id = make(&mut queue, "Two", &[INNER, "SHT-BACK"]);
    accepted(&mut queue, &id, &norms(&[INNER, "SHT-BACK"], 1, OLD));
    let now = norms(&[INNER], 2, NEW);
    let fix = queue
        .create_card_frozen(
            &new_card(flow, "Correct it", &[INNER], Some(&id)),
            &frozen(&[INNER], NEW),
            T1,
        )
        .expect("a correction")
        .id;
    to_proof(&mut queue, &fix);
    queue
        .accept_card(
            &fix,
            None,
            &CardAccept {
                flow: Some(flow),
                note: None,
                norms: &now,
            },
            "ben",
            T2,
        )
        .expect("accepted");
    let card = queue.get_card(&id).expect("read").expect("a card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].norm_hash.as_deref(), Some(NEW), "read");
    assert_eq!(snapshot[1].norm_hash.as_deref(), Some(OLD), "not read");
    assert_eq!(
        payloads(&queue, "card.cleared")[0]["nodes"],
        json!([{"id": INNER, "rev": 2, "from": OLD, "to": NEW}])
    );
}

// ------------------------------------------------------------ the refresh

/// docs/features/trace-truth.md AC-05, "Editorial refresh": a document whose
/// own hash and `within_hash` both moved sends a pair of each kind: an entry
/// holding a `within_hash` takes the within pair and logs one `card.refreshed`,
/// `revision` one more; a legacy entry, which holds no `within_hash`, takes the
/// own pair the same way.
#[test]
fn two_changes_of_one_node_log_one_event() {
    let scratch = Scratch::new("qt-two-changes");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    let entry = |within: Option<&str>| CardFrozen {
        id: INNER.to_owned(),
        rev: Some(1),
        norm_hash: Some(OLD.to_owned()),
        within_hash: within.map(str::to_owned),
        also: Vec::new(),
    };
    let current = queue
        .create_card_frozen(
            &new_card(flow, "Current", &[INNER], None),
            &[entry(Some("b3:within-old"))],
            T0,
        )
        .expect("a card");
    let legacy = queue
        .create_card_frozen(
            &new_card(flow, "Legacy", &[INNER], None),
            &[entry(None)],
            T0,
        )
        .expect("a card");
    let id = update(&mut queue, INNER, "new");
    let refresh = [
        CardRefresh {
            node: INNER.to_owned(),
            kind: HashKind::Own,
            old: OLD.to_owned(),
            new: NEW.to_owned(),
        },
        CardRefresh {
            node: INNER.to_owned(),
            kind: HashKind::Within,
            old: "b3:within-old".to_owned(),
            new: "b3:within-new".to_owned(),
        },
    ];
    let commit = "c".repeat(40);
    queue
        .applied_vouching(&id, &commit, Some(&owner()), None, &refresh, T1)
        .expect("recorded");
    assert_eq!(
        payloads(&queue, "card.refreshed"),
        [
            json!({"id": current.id, "node": INNER, "proposal": id}),
            json!({"id": legacy.id, "node": INNER, "proposal": id}),
        ]
    );
    let card = queue.get_card(&current.id).expect("read").expect("a card");
    assert_eq!(card.revision, current.revision + 1, "once per card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].also, ["b3:within-new"], "the kind it compares");
    assert_eq!(snapshot[0].within_hash.as_deref(), Some("b3:within-old"));
    let card = queue.get_card(&legacy.id).expect("read").expect("a card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].also, [NEW], "own hashes on a legacy entry");
    assert_eq!(snapshot[0].within_hash, None);
}

/// "one `card.refreshed` per (card, node)", held where two changes of a node
/// both join one entry's `also`: an entry takes one kind, and a pass sends one
/// pair of it per holder of the ID, so a duplicated `{#ID}` does it through a
/// real pass (the CLI's `ac05_a_duplicated_id_logs_one_event`), and so does a
/// refresh chained as here, the second change's old hash the first's new one.
/// M: `nodes` not deduplicated.
#[test]
fn a_chain_of_changes_of_one_node_logs_one_event() {
    let scratch = Scratch::new("qt-chain");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let card = make(&mut queue, "Chained", &[INNER]);
    let before = queue.get_card(&card).expect("read").expect("a card");
    let id = update(&mut queue, INNER, "new");
    let refresh = [
        CardRefresh {
            node: INNER.to_owned(),
            kind: HashKind::Within,
            old: OLD.to_owned(),
            new: NEW.to_owned(),
        },
        CardRefresh {
            node: INNER.to_owned(),
            kind: HashKind::Within,
            old: NEW.to_owned(),
            new: "b3:newer".to_owned(),
        },
    ];
    let commit = "c".repeat(40);
    queue
        .applied_vouching(&id, &commit, Some(&owner()), None, &refresh, T1)
        .expect("recorded");
    assert_eq!(
        payloads(&queue, "card.refreshed"),
        [json!({"id": card, "node": INNER, "proposal": id})]
    );
    let after = queue.get_card(&card).expect("read").expect("a card");
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(
        after.snapshot.expect("a snapshot")[0].also,
        [NEW, "b3:newer"]
    );
}

/// A decision `applied_vouching` records an `open` update under.
fn owner() -> Decision {
    Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: None,
        staged_at: None,
    }
}

/// An `open` update the refresh's recording completes, its `text` its own.
fn update(queue: &mut SqliteQueue, target: &str, text: &str) -> String {
    let base_hash = b3_hash(b"base");
    let place = departure(&[INNER], "A place.").place;
    queue
        .create(
            &NewProposal {
                kind: ProposalKind::Update,
                target_id: target.to_owned(),
                target_path: "docs/spec/sht-inner.md".to_owned(),
                place,
                patch_hash: patch_hash(target, &base_hash, text),
                base_hash: Some(base_hash),
                base_text: Some("base".to_owned()),
                new_text: text.to_owned(),
                rationale: "Why.".to_owned(),
                author: Author::human(),
                diagnostics: Vec::new(),
                new_ids: Vec::new(),
                interpretation: None,
            },
            T0,
        )
        .expect("an update")
        .id
}

/// AC-02, the store: the new hash joins `also` of every entry holding the old
/// one, one `card.refreshed` each, `revision` one more; an entry at another
/// hash, a card with no snapshot, a cancelled card and a corrupt row are
/// left; the recording stands.
#[test]
fn applied_vouching_adds_to_also() {
    let scratch = Scratch::new("qt-vouch");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let config = press();
    let flow = copy(&config);
    let first = make(&mut queue, "First", &[INNER]);
    let second = make(&mut queue, "Second", &[INNER, "SHT-BACK"]);
    let other = queue
        .create_card_frozen(
            &new_card(flow, "Elsewhere", &[INNER], None),
            &frozen(&[INNER], "b3:elsewhere"),
            T0,
        )
        .expect("a card")
        .id;
    let bare = queue
        .create_card(&new_card(flow, "No snapshot", &[INNER], None), T0)
        .expect("a card")
        .id;
    let bad = make(&mut queue, "Bad", &[INNER]);
    sqlite3(
        &db,
        &format!("UPDATE cards SET evidence = 'not json' WHERE id = '{bad}';"),
    );
    let gone = make(&mut queue, "Pulled", &[INNER]);
    queue
        .change_card(
            &gone,
            None,
            &CardChange::Cancel {
                reason: "Pulled.".to_owned(),
            },
            "desk",
            T0,
        )
        .expect("cancelled");
    let before = queue
        .get_card(&first)
        .expect("read")
        .expect("a card")
        .revision;
    let id = update(&mut queue, INNER, "new");
    let refresh = [CardRefresh {
        node: INNER.to_owned(),
        kind: HashKind::Within,
        old: OLD.to_owned(),
        new: NEW.to_owned(),
    }];
    let commit = "c".repeat(40);
    let (applied, _, changed) = queue
        .applied_vouching(&id, &commit, Some(&owner()), None, &refresh, T1)
        .expect("recorded");
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(changed, [first.clone(), second.clone()]);
    assert_eq!(
        payloads(&queue, "card.refreshed"),
        [
            json!({"id": first, "node": INNER, "proposal": id}),
            json!({"id": second, "node": INNER, "proposal": id}),
        ]
    );
    let card = queue.get_card(&first).expect("read").expect("a card");
    assert_eq!(card.revision, before + 1);
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[0].norm_hash.as_deref(), Some(OLD), "kept");
    assert_eq!(snapshot[0].also, [NEW]);
    let card = queue.get_card(&second).expect("read").expect("a card");
    let snapshot = card.snapshot.expect("a snapshot");
    assert_eq!(snapshot[1].also, Vec::<String>::new(), "the other target's");
    assert_eq!(
        queue
            .get_card(&other)
            .expect("read")
            .expect("a card")
            .snapshot
            .expect("a snapshot")[0]
            .also,
        Vec::<String>::new(),
        "it does not hold the old hash"
    );
    assert_eq!(
        queue
            .get_card(&bare)
            .expect("read")
            .expect("a card")
            .snapshot,
        None
    );
    assert_eq!(
        queue
            .get_card(&gone)
            .expect("read")
            .expect("a card")
            .snapshot
            .expect("a snapshot")[0]
            .also,
        Vec::<String>::new(),
        "a cancelled card is ignored"
    );
}

/// "`also` keeps the last 16".
#[test]
fn also_keeps_the_last_sixteen() {
    assert_eq!(ALSO_MAX, 16);
    let scratch = Scratch::new("qt-also");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Lead story", &[INNER]);
    let mut last = OLD.to_owned();
    for number in 0..17 {
        let new = format!("b3:{number}");
        let proposal = update(&mut queue, INNER, &new);
        queue
            .applied_vouching(
                &proposal,
                &"c".repeat(40),
                Some(&owner()),
                None,
                &[CardRefresh {
                    node: INNER.to_owned(),
                    kind: HashKind::Within,
                    old: last.clone(),
                    new: new.clone(),
                }],
                T1,
            )
            .expect("recorded");
        last = new;
    }
    let card = queue.get_card(&id).expect("read").expect("a card");
    let also = &card.snapshot.expect("a snapshot")[0].also;
    assert_eq!(also.len(), 16, "{also:?}");
    assert_eq!(also[0], "b3:1");
    assert_eq!(also[15], "b3:16");
}

// ---------------------------------------------------------------- reads

/// "Data": `list_existing_cards` over `open_existing`: no `cards` table reads
/// empty, none added, `user_version` kept; a newer build's queue is refused.
#[test]
fn list_existing_cards_runs_no_schema_step() {
    let scratch = Scratch::new("qt-existing");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    make(&mut queue, "Lead story", &[INNER]);
    drop(queue);
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open")
        .expect("a queue");
    assert_eq!(queue.list_existing_cards().expect("cards").cards.len(), 1);
    drop(queue);

    let version = sqlite3(&db, "PRAGMA user_version;");
    sqlite3(&db, "DROP TABLE cards; PRAGMA user_version = 8;");
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open")
        .expect("a queue");
    let list = queue.list_existing_cards().expect("cards");
    assert_eq!(list.cards.len(), 0);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM sqlite_master WHERE name = 'cards';"
        ),
        "0",
        "no table added"
    );
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8");
    drop(queue);

    sqlite3(
        &db,
        &format!(
            "PRAGMA user_version = {};",
            version.parse::<u32>().unwrap() + 1
        ),
    );
    let queue = SqliteQueue::open_existing(&db, PROJECT);
    let read = queue.and_then(|queue| queue.expect("a queue").list_existing_cards());
    assert!(
        matches!(read, Err(QueueError::SchemaTooNew { .. })),
        "{read:?}"
    );
}

/// "Store": `node_norms`, the first read of `nodes.norm_hash`: every node by
/// (path, position), as `spec show` hashes it, a stale index refused as any
/// read; null where the file's front matter fails.
#[test]
fn node_norms_reads_the_stored_column() {
    let scratch = Scratch::new("qt-norms");
    let corpus = Corpus::copy_of("card-press", &scratch, "wt");
    let index = corpus.fresh(&scratch);
    let norms = index.node_norms().expect("the norms");
    let front: Vec<_> = norms
        .iter()
        .filter(|row| row.path == "docs/spec/sht-front.md")
        .collect();
    assert_eq!(
        front.iter().map(|row| row.ord).collect::<Vec<_>>(),
        [0, 1],
        "the document, then its section"
    );
    let sorted = {
        let mut rows: Vec<(String, usize)> = norms
            .iter()
            .map(|row| (row.path.clone(), row.ord))
            .collect();
        rows.sort();
        rows
    };
    assert_eq!(
        norms
            .iter()
            .map(|row| (row.path.clone(), row.ord))
            .collect::<Vec<_>>(),
        sorted,
        "by (path, position)"
    );
    for row in &norms {
        let bytes = corpus.bytes(&row.path);
        let parsed = specengine_core::parse(&row.path, &bytes, &corpus.scheme);
        let want = specengine_core::norm_input(&parsed, &bytes, row.ord)
            .map(|input| specengine_store::b3_hash(&input));
        assert_eq!(row.norm_hash, want, "{}#{}", row.path, row.ord);
    }

    corpus.replace("docs/spec/sht-back.md", "id: SHT-BACK\n", "id: [SHT-BACK\n");
    let mut index = index;
    corpus.update(&mut index);
    let rows = index.node_norms().expect("the norms");
    let back = rows
        .iter()
        .find(|row| row.path == "docs/spec/sht-back.md")
        .expect("a row");
    assert_eq!(back.norm_hash, None, "a front matter that fails");
}

/// "Re-estimating", the store: `card.reestimated` on a card holding an
/// estimate, `reason` required under the lock; a first estimate `card.edited`.
#[test]
fn a_re_estimate_is_its_own_event() {
    let scratch = Scratch::new("qt-estimate");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = make(&mut queue, "Lead story", &[INNER]);
    let edit = |estimate: &str, reason: Option<&str>| CardEdit {
        estimate: Some(Some(estimate.to_owned())),
        reason: reason.map(str::to_owned),
        ..CardEdit::default()
    };
    queue
        .change_card(
            &id,
            None,
            &CardChange::Edit(Box::new(edit("2", None))),
            "ana",
            T0,
        )
        .expect("a first estimate");
    assert_eq!(all_events(&queue).pop().expect("an event").0, "card.edited");

    let rows = queue.dump().expect("dump");
    let reason = refusal(queue.change_card(
        &id,
        None,
        &CardChange::Edit(Box::new(edit("3", None))),
        "ana",
        T1,
    ));
    assert!(reason.contains("--reason"), "{reason}");
    assert_eq!(queue.dump().expect("dump"), rows);

    queue
        .change_card(
            &id,
            None,
            &CardChange::Edit(Box::new(edit("3", Some("Bigger.")))),
            "ana",
            T1,
        )
        .expect("a re-estimate");
    assert_eq!(
        all_events(&queue).pop().expect("an event"),
        (
            "card.reestimated".to_owned(),
            json!({"id": id, "old": "2", "new": "3", "reason": "Bigger.", "as": "ana"})
        )
    );
}
