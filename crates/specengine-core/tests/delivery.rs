//! docs/features/delivery-analytics.md (RM-0107 a) "Acceptance criteria",
//! the judge's half: core `delivery` over inputs built here, the spec's
//! scenario among them, and `roadmap::item_groups` over a parsed plan. Pure:
//! no file, queue or clock.
//!
//! - The scenario: every row, the total and both late rows are the spec's
//!   JSON (`specengine-cli`'s `tests/delivery_scene/data.rs`, included by
//!   path), whatever order the cards come in.
//! - DA-1: 70 reorders before and after the take; the moves read by `seq`,
//!   never by `at`. M: `card_history(…, 64)` (the CLI half), `at` order.
//! - DA-2: a reorder is no take; an accept from the first column is one;
//!   only reorders: none. M: the first move of any kind.
//! - DA-3: the parent fallback, up two levels; the least group order of
//!   two targets in either written order; a done item's group
//!   (`item_groups`); pool, unplaced, none; a parent cycle and a parent not
//!   read end at none. M: the parent fallback dropped; the last target's
//!   group; done items ignored.
//! - DA-4: a flow in two departments, the implicit `all`; the total once.
//!   M: `total` as the sum of the departments.
//! - DA-5: `as_of` before, at and after `due`; a delivered card's lateness
//!   whatever `as_of`. M: `<=` for `<`.
//! - DA-6: no key holding `estimate` but a late row's; no person's key. M:
//!   an `estimate_sum` key; `assignee` on a late row.
//! - DA-8: each list's oldest 16 kept, the rest in `more`; a noted move
//!   and a reorder inside the return column are no returns; a flow no
//!   longer configured has none. M: any noted move a return; the lists
//!   uncapped.
//! - DA-9: the median the lower middle, `null` with none delivered; a
//!   time stamp that does not read is a note and no lead. M: the upper
//!   middle.
//! - Risk "Day arithmetic": `unix_days` inverts `date_from_unix_days` every
//!   day from 1970 to 2100.
//!
//! docs/features/delivery-timeline.md (RM-0107 b) "Acceptance criteria",
//! the judge's half, the flag on [`Facts::timeline`]:
//!
//! - DT1: the scenario's timeline is `data::TIMELINE_JSON` whatever the
//!   input order; rows by group order, then `created_at`, then ID number;
//!   `days_late` only on a late or overdue row; an accept from the first
//!   column is the take; a `created_at` that does not read, no row. M:
//!   cancelled cards as rows; rows by `created_at` alone; `days_late` on
//!   an on-time card.
//! - DT2: a marker per group holding a row, the least take and the greatest
//!   accept, `null` with none; a group of cancelled cards alone, none. M:
//!   the first accept as `last_accepted_at`; markers over cancelled cards.
//! - DT4: without the flag `timeline` is `None` and the rest is the same
//!   document. M: the timeline built whatever the flag.
//! - DT5: `from` the least `created_at` day; `to` 2026-11-20 at 11-20,
//!   2026-11-15 at 11-01, a later `due` or `as_of` when one is greatest,
//!   and never before a stored day (the axis covers every one); no row, both
//!   `null`. M: `to` as `as_of` alone.

#[path = "../../specengine-cli/tests/delivery_scene/data.rs"]
mod data;

use std::collections::BTreeMap;

use data::{DATA_JSON, TIMELINE_JSON};
use serde_json::{Value, json};
use specengine_core::IdSchemeToml as _;
use specengine_core::check::{CheckFile, CheckInput, date_from_unix_days};
use specengine_core::delivery::{
    CardItemRef, Delivery, DeliveryInput, Moved, REASONS_MAX, Reestimated, delivery, unix_days,
};
use specengine_core::roadmap::{GroupKind, GroupRef, item_groups};
use specengine_core::roadmap_toml::RoadmapConfig;
use specengine_core::workspace_toml::Workspace;
use specengine_model::{Card, CardAcceptance, CardCancellation, CardVerdict, IdScheme};

const AS_OF: &str = "2026-11-20";

fn at(day: u32) -> String {
    format!("2026-11-{day:02}T09:00:00Z")
}

/// A card of `flow` in `column`, made at `day`, nothing else set.
fn card(id: &str, title: &str, flow: &str, column: &str, day: u32) -> Card {
    Card {
        id: id.to_owned(),
        project: "tide-almanac".to_owned(),
        flow: flow.to_owned(),
        column: column.to_owned(),
        position: 1,
        title: title.to_owned(),
        goal: None,
        discipline: Some(flow.to_owned()),
        targets: Vec::new(),
        snapshot: None,
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        mandate: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: None,
        evidence: Vec::new(),
        acceptance: None,
        cancellation: None,
        created_by: "Ann".to_owned(),
        created_at: at(day),
        updated_at: at(day),
        revision: 1,
        tasks: Vec::new(),
    }
}

fn accepted(mut card: Card, when: &str) -> Card {
    card.acceptance = Some(CardAcceptance {
        by: "Bob".to_owned(),
        at: when.to_owned(),
        note: None,
        verdict: CardVerdict::default(),
        departs: Vec::new(),
        discrepancy: None,
    });
    card
}

fn cancelled(mut card: Card, when: &str, reason: &str) -> Card {
    card.cancellation = Some(CardCancellation {
        by: "Ann".to_owned(),
        at: when.to_owned(),
        reason: reason.to_owned(),
    });
    card.position = 0;
    card
}

fn due(mut card: Card, day: &str) -> Card {
    card.due = Some(day.to_owned());
    card
}

fn targets(mut card: Card, ids: &[&str]) -> Card {
    card.targets = ids.iter().map(|id| (*id).to_owned()).collect();
    card
}

fn parent(mut card: Card, id: &str) -> Card {
    card.parent = Some(id.to_owned());
    card
}

fn workspace(name: &str, title: &str, flows: Option<&[&str]>) -> Workspace {
    Workspace {
        name: name.to_owned(),
        title: title.to_owned(),
        landing: "overview".to_owned(),
        sections: vec!["board".to_owned()],
        flows: flows.map(|flows| flows.iter().map(|flow| (*flow).to_owned()).collect()),
        disciplines: Vec::new(),
        module: None,
    }
}

fn release(name: &str, order: usize) -> GroupRef {
    GroupRef {
        kind: GroupKind::Release,
        name: Some(name.to_owned()),
        order,
    }
}

/// A `card.moved` or `card.accepted`, owned: (seq, card, from, to, note,
/// at).
type OwnedMove = (i64, String, String, String, Option<String>, String);

/// A `card.reestimated`, owned: (seq, card, old, new, reason, at).
type OwnedReestimate = (i64, String, Option<String>, Option<String>, String, String);

/// One read's facts, owned; [`Facts::judge`] lends them to the judge.
struct Facts {
    as_of: String,
    cards: Vec<Card>,
    moves: Vec<OwnedMove>,
    reestimates: Vec<OwnedReestimate>,
    /// (id, card, kind, status)
    items: Vec<(String, String, String, String)>,
    groups: BTreeMap<String, GroupRef>,
    departments: Vec<Workspace>,
    returns: BTreeMap<String, String>,
    /// delivery-timeline's flag (DT4): `false` unless a case asks.
    timeline: bool,
    seq: i64,
}

impl Facts {
    fn new() -> Self {
        Self {
            as_of: AS_OF.to_owned(),
            cards: Vec::new(),
            moves: Vec::new(),
            reestimates: Vec::new(),
            items: Vec::new(),
            groups: BTreeMap::from([
                ("RM-0001".to_owned(), release("r1", 0)),
                ("RM-0002".to_owned(), release("r2", 1)),
            ]),
            departments: vec![
                workspace("words", "Words", Some(&["copy"])),
                workspace("print", "Print", Some(&["copy", "plates"])),
            ],
            returns: BTreeMap::from([("copy".to_owned(), "returned".to_owned())]),
            timeline: false,
            seq: 0,
        }
    }

    fn next(&mut self) -> i64 {
        self.seq += 1;
        self.seq
    }

    fn moved(&mut self, card: &str, from: &str, to: &str, note: Option<&str>, when: &str) {
        let seq = self.next();
        self.moves.push((
            seq,
            card.to_owned(),
            from.to_owned(),
            to.to_owned(),
            note.map(str::to_owned),
            when.to_owned(),
        ));
    }

    fn reestimated(
        &mut self,
        card: &str,
        old: Option<&str>,
        new: Option<&str>,
        reason: &str,
        when: &str,
    ) {
        let seq = self.next();
        self.reestimates.push((
            seq,
            card.to_owned(),
            old.map(str::to_owned),
            new.map(str::to_owned),
            reason.to_owned(),
            when.to_owned(),
        ));
    }

    /// The spec's scenario: C-0001 to C-0005 and PR-0040, events by `seq`.
    fn scenario() -> Self {
        let mut facts = Self::new();
        let mut c1 = targets(
            due(
                card("C-0001", "Write the tide table foreword", "copy", "done", 1),
                "2026-11-10",
            ),
            &["RM-0001"],
        );
        c1.assignee = Some("Ann".to_owned());
        c1.reviewer = Some("Bob".to_owned());
        c1.estimate = Some("5".to_owned());
        let c1 = accepted(c1, &at(15));
        let c2 = cancelled(
            parent(
                card("C-0002", "Draw the tide plates", "plates", "todo", 2),
                "C-0001",
            ),
            &at(9),
            "Plates reused from last year",
        );
        let c3 = targets(
            due(
                card("C-0003", "Engrave the harbor map", "plates", "drawing", 4),
                "2026-11-12",
            ),
            &["RM-0002"],
        );
        let c4 = accepted(card("C-0004", "Fix the index", "copy", "done", 2), &at(5));
        let c5 = card("C-0005", "Record the foghorn", "sound", "todo", 6);
        facts.cards = vec![c1, c2, c3, c4, c5];
        facts.seq = 5;
        facts.moved("C-0001", "todo", "todo", None, &at(1));
        facts.moved("C-0001", "todo", "todo", None, &at(2));
        facts.moved("C-0001", "todo", "doing", None, &at(3));
        facts.moved("C-0004", "todo", "review", None, &at(3));
        facts.moved("C-0004", "review", "done", None, &at(5));
        facts.moved("C-0003", "todo", "drawing", None, &at(5));
        facts.moved("C-0001", "doing", "review", None, &at(5));
        facts.moved("C-0001", "review", "returned", Some("Too long"), &at(6));
        facts.reestimated("C-0001", Some("3"), Some("5"), "The tables doubled", &at(7));
        facts.moved("C-0001", "returned", "review", None, &at(7));
        facts.moved("C-0001", "review", "returned", Some("Wrong tone"), &at(8));
        facts.moved("C-0001", "returned", "review", None, &at(10));
        facts.moved("C-0001", "review", "done", None, &at(15));
        facts.items.push((
            "PR-0040".to_owned(),
            "C-0001".to_owned(),
            "question".to_owned(),
            "applied".to_owned(),
        ));
        facts
    }

    fn judge(&self) -> Delivery {
        let moves: Vec<Moved<'_>> = self
            .moves
            .iter()
            .map(|(seq, card, from, to, note, at)| Moved {
                seq: *seq,
                card,
                from,
                to,
                note: note.as_deref(),
                at,
            })
            .collect();
        let reestimates: Vec<Reestimated<'_>> = self
            .reestimates
            .iter()
            .map(|(seq, card, old, new, reason, at)| Reestimated {
                seq: *seq,
                card,
                old: old.as_deref(),
                new: new.as_deref(),
                reason,
                at,
            })
            .collect();
        let items: Vec<CardItemRef<'_>> = self
            .items
            .iter()
            .map(|(id, card, kind, status)| CardItemRef {
                id,
                card,
                kind,
                status,
            })
            .collect();
        delivery(&DeliveryInput {
            as_of: &self.as_of,
            cards: &self.cards,
            moves: &moves,
            reestimates: &reestimates,
            items: &items,
            groups: &self.groups,
            departments: &self.departments,
            returns: &self.returns,
            timeline: self.timeline,
        })
    }
}

/// The judged document's served parts as JSON.
fn served(delivery: &Delivery) -> Value {
    json!({
        "as_of": delivery.as_of,
        "notes": delivery.notes,
        "releases": serde_json::to_value(&delivery.releases).expect("releases"),
        "departments": serde_json::to_value(&delivery.departments).expect("departments"),
        "total": serde_json::to_value(delivery.total).expect("total"),
        "late": serde_json::to_value(&delivery.late).expect("late"),
        "timeline": serde_json::to_value(&delivery.timeline).expect("timeline"),
    })
}

/// The spec's scenario JSON less the CLI's `truncated` and `omitted`.
fn pinned() -> Value {
    let mut value: Value = serde_json::from_str(DATA_JSON).expect("the spec's JSON");
    let map = value.as_object_mut().expect("an object");
    map.remove("truncated");
    map.remove("omitted");
    value
}

fn late<'d>(document: &'d Value, id: &str) -> &'d Value {
    document["late"]
        .as_array()
        .expect("late")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no late row {id}: {document}"))
}

fn late_ids(document: &Value) -> Vec<String> {
    document["late"]
        .as_array()
        .expect("late")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID").to_owned())
        .collect()
}

// ------------------------------------------------------------ the scenario

/// The scenario judged is the spec's JSON (but the CLI's cut keys), and the
/// same whatever order the cards, moves and items come in.
#[test]
fn the_scenario_is_the_specs_json() {
    let facts = Facts::scenario();
    assert_eq!(served(&facts.judge()), pinned());
    let mut shuffled = Facts::scenario();
    shuffled.cards.reverse();
    shuffled.moves.reverse();
    shuffled.reestimates.reverse();
    assert_eq!(served(&shuffled.judge()), pinned(), "input order");
    // Key order of the serialized rows: the spec's.
    let release = serde_json::to_string(&facts.judge().releases[0]).expect("a row");
    assert!(
        release.starts_with(
            "{\"group\":\"release\",\"release\":\"r1\",\"tasks\":1,\"delivered\":1,\
             \"delivered_late\":1,\"open\":0,\"overdue\":0,\"cancelled\":1,\"lead_days\":"
        ),
        "{release}"
    );
}

// ------------------------------------------------------------------ DA-1

/// DA-1: 70 reorders in `todo` before the take and 70 in `doing` after it:
/// `taken_at` is the move to `doing`, the document the scenario's.
#[test]
fn da1_many_reorders_change_nothing() {
    let mut facts = Facts::scenario();
    let take = facts
        .moves
        .iter()
        .position(|(_, card, from, to, _, _)| card == "C-0001" && from == "todo" && to == "doing")
        .expect("the take");
    let mut moves = facts.moves[..take].to_vec();
    let mut seq = 100;
    for _ in 0..70 {
        seq += 1;
        moves.push((
            seq,
            "C-0001".into(),
            "todo".into(),
            "todo".into(),
            None,
            at(2),
        ));
    }
    let mut taken = facts.moves[take].clone();
    seq += 1;
    taken.0 = seq;
    moves.push(taken);
    for _ in 0..70 {
        seq += 1;
        moves.push((
            seq,
            "C-0001".into(),
            "doing".into(),
            "doing".into(),
            None,
            at(4),
        ));
    }
    for mut rest in facts.moves[take + 1..].iter().cloned() {
        seq += 1;
        rest.0 = seq;
        moves.push(rest);
    }
    facts.moves = moves;
    for reestimate in &mut facts.reestimates {
        reestimate.0 += 1_000;
    }
    assert_eq!(served(&facts.judge()), pinned());
}

/// DA-1, the slice's rule: order is by `seq`, never by `at`: C-0001's first
/// move out of `todo` holds a later `at` than its next move; the take is
/// still the first by `seq`, and its returns are listed by `seq`.
#[test]
fn da1_seq_orders_never_at() {
    let mut facts = Facts::scenario();
    for moved in &mut facts.moves {
        if moved.1 == "C-0001" && moved.2 == "todo" && moved.3 == "doing" {
            moved.5 = "2026-11-05T23:00:00Z".to_owned();
        }
        if moved.1 == "C-0001" && moved.3 == "returned" && moved.4.as_deref() == Some("Too long") {
            moved.5 = at(9);
        }
    }
    let document = served(&facts.judge());
    let row = late(&document, "C-0001");
    assert_eq!(row["taken_at"], "2026-11-05T23:00:00Z");
    assert_eq!(row["returns"][0]["note"], "Too long", "by seq: {row}");
    assert_eq!(row["returns"][1]["note"], "Wrong tone");
}

// ------------------------------------------------------------------ DA-2

/// DA-2: a reorder is no take; an accept straight from the first column is
/// the take; a card only reordered has none. M: the first `card.moved` of
/// any kind.
#[test]
fn da2_a_reorder_is_no_take() {
    let mut facts = Facts::scenario();
    let bell = accepted(
        due(
            card("C-0006", "Tune the bell", "sound", "done", 1),
            "2026-11-05",
        ),
        &at(8),
    );
    let gun = due(
        card("C-0007", "Ring the noon gun", "copy", "todo", 1),
        "2026-11-10",
    );
    facts.cards.push(bell);
    facts.cards.push(gun);
    facts.moved("C-0007", "todo", "todo", None, &at(2));
    facts.moved("C-0006", "todo", "done", None, &at(8));
    facts.moved("C-0007", "todo", "todo", Some("Kept for the noon"), &at(3));
    let document = served(&facts.judge());
    assert_eq!(late(&document, "C-0001")["taken_at"], at(3));
    assert_eq!(late(&document, "C-0006")["taken_at"], at(8));
    assert_eq!(late(&document, "C-0006")["lead_days"], 7);
    assert_eq!(late(&document, "C-0006")["days_late"], 3);
    assert_eq!(late(&document, "C-0007")["taken_at"], Value::Null);
    assert_eq!(late(&document, "C-0007")["returns"], json!([]));
    assert_eq!(
        late_ids(&document),
        ["C-0007", "C-0003", "C-0001", "C-0006"]
    );
}

// ------------------------------------------------------------------ DA-3

/// The (group, release) of each late row.
fn placed(document: &Value, id: &str) -> (Value, Value) {
    let row = late(document, id);
    (row["group"].clone(), row["release"].clone())
}

/// DA-3 at the judge: C-0002 counts under r1 through its parent; a card on
/// RM-0002 and RM-0001, in either written order, is r1; the pool and the
/// unplaced groups; a grandparent's pool item; no item up the chain,
/// `none`; a parent cycle and a parent not read end at `none`. Rows in
/// group order, then `none`. M: the parent fallback dropped; the last
/// target's group.
#[test]
fn da3_a_cards_release_at_the_judge() {
    let mut facts = Facts::scenario();
    facts.groups.insert(
        "RM-0004".to_owned(),
        GroupRef {
            kind: GroupKind::Pool,
            name: None,
            order: 2,
        },
    );
    facts.groups.insert(
        "RM-0005".to_owned(),
        GroupRef {
            kind: GroupKind::Unplaced,
            name: None,
            order: 3,
        },
    );
    let overdue = |id: &str, title: &str| due(card(id, title, "copy", "todo", 1), "2026-11-10");
    facts.cards.extend([
        targets(overdue("C-0006", "Bind both"), &["RM-0002", "RM-0001"]),
        targets(
            overdue("C-0007", "Bind both again"),
            &["RM-0001", "RM-0002"],
        ),
        targets(
            overdue("C-0009", "Trim the lanterns"),
            &["RM-0004", "RM-0099"],
        ),
        targets(overdue("C-0010", "Time the ferry"), &["RM-0005"]),
        parent(overdue("C-0011", "Polish a lantern"), "C-0009"),
        parent(overdue("C-0012", "Polish its glass"), "C-0011"),
        parent(overdue("C-0013", "Chase a ghost"), "C-0404"),
        parent(overdue("C-0014", "Loop one"), "C-0015"),
        parent(overdue("C-0015", "Loop two"), "C-0014"),
        parent(overdue("C-0016", "Under the index"), "C-0004"),
    ]);
    let document = served(&facts.judge());
    let rows: Vec<(Value, Value)> = document["releases"]
        .as_array()
        .expect("releases")
        .iter()
        .map(|row| (row["group"].clone(), row["release"].clone()))
        .collect();
    assert_eq!(
        rows,
        [
            (json!("release"), json!("r1")),
            (json!("release"), json!("r2")),
            (json!("pool"), Value::Null),
            (json!("unplaced"), Value::Null),
            (json!("none"), Value::Null),
        ]
    );
    assert_eq!(document["releases"][0]["cancelled"], 1, "C-0002 via C-0001");
    assert_eq!(document["releases"][0]["tasks"], 3);
    assert_eq!(
        document["releases"][2]["tasks"], 3,
        "C-0009, C-0011, C-0012"
    );
    assert_eq!(
        document["releases"][4]["tasks"], 6,
        "C-0004, C-0005, C-0013 to C-0016"
    );
    for (id, group, release) in [
        ("C-0006", "release", json!("r1")),
        ("C-0007", "release", json!("r1")),
        ("C-0009", "pool", Value::Null),
        ("C-0010", "unplaced", Value::Null),
        ("C-0011", "pool", Value::Null),
        ("C-0012", "pool", Value::Null),
        ("C-0013", "none", Value::Null),
        ("C-0014", "none", Value::Null),
        ("C-0015", "none", Value::Null),
        ("C-0016", "none", Value::Null),
    ] {
        assert_eq!(placed(&document, id), (json!(group), release), "{id}");
    }
}

const IDS: &str = "[ids]\nRM = { kind = \"plan-item\", width = 4 }\n";
const ROADMAP: &str = "[roadmap]\nprefix = \"RM\"\ndir = \"docs/roadmap\"\nreleases = [\"r1\", \
                       \"r2\"]\npriorities = [\"must\", \"could\"]\n";

fn item_text(id: &str, release: Option<&str>, status: &str) -> String {
    let release = release.map_or(String::new(), |name| format!("release: {name}\n"));
    format!(
        "---\nclass: spec\nid: {id}\nstatus: {status}\nscope: [almanac]\n{release}---\n\n# Item \
         {id}\n\nThe goal.\n"
    )
}

/// DA-3, `item_groups`: every item, open or done, in its group: r1 and r2
/// with the releases' orders, a shipped and an abandoned item of r2 in r2,
/// the pool and the unplaced after the releases. M: done items ignored.
#[test]
fn da3_item_groups_hold_done_items() {
    let toml = format!("[project]\nslug = \"tide-almanac\"\n\n{IDS}\n{ROADMAP}");
    let scheme = IdScheme::from_toml(&toml).expect("[ids]");
    let config = RoadmapConfig::from_toml(&toml, &scheme)
        .expect("[roadmap]")
        .expect("a table");
    let files = [
        (
            "docs/roadmap/RM-0001.md",
            item_text("RM-0001", Some("r1"), "draft"),
        ),
        (
            "docs/roadmap/RM-0002.md",
            item_text("RM-0002", Some("r2"), "in-progress"),
        ),
        (
            "docs/roadmap/RM-0003.md",
            item_text("RM-0003", Some("r2"), "shipped"),
        ),
        (
            "docs/roadmap/RM-0004.md",
            item_text("RM-0004", None, "draft"),
        ),
        (
            "docs/roadmap/RM-0005.md",
            item_text("RM-0005", Some("r9"), "draft"),
        ),
        (
            "docs/roadmap/RM-0006.md",
            item_text("RM-0006", Some("r1"), "abandoned"),
        ),
        (
            "docs/roadmap/RM-0007.md",
            item_text("RM-0007", None, "shipped"),
        ),
    ];
    let input = CheckInput {
        files: files
            .iter()
            .map(|(path, text)| CheckFile::parse(*path, text.as_bytes().to_vec(), &scheme))
            .collect(),
        problems: Vec::new(),
    };
    let groups = item_groups(&input, &config);
    let release = |name: &str, order| GroupRef {
        kind: GroupKind::Release,
        name: Some(name.to_owned()),
        order,
    };
    let pool = GroupRef {
        kind: GroupKind::Pool,
        name: None,
        order: 2,
    };
    let unplaced = GroupRef {
        kind: GroupKind::Unplaced,
        name: None,
        order: 3,
    };
    assert_eq!(
        groups,
        BTreeMap::from([
            ("RM-0001".to_owned(), release("r1", 0)),
            ("RM-0002".to_owned(), release("r2", 1)),
            ("RM-0003".to_owned(), release("r2", 1)),
            ("RM-0004".to_owned(), pool.clone()),
            ("RM-0005".to_owned(), unplaced),
            ("RM-0006".to_owned(), release("r1", 0)),
            ("RM-0007".to_owned(), pool),
        ])
    );
    // A card on the done item RM-0003 counts in r2.
    let mut facts = Facts::scenario();
    facts.groups = groups;
    facts.cards.push(targets(
        due(
            card("C-0008", "Reprint the neap", "copy", "todo", 1),
            "2026-11-10",
        ),
        &["RM-0003"],
    ));
    let document = served(&facts.judge());
    assert_eq!(placed(&document, "C-0008"), (json!("release"), json!("r2")));
}

// ------------------------------------------------------------------ DA-4

/// DA-4: `words` counts C-0001 and C-0004, `print` C-0001 to C-0004 (the
/// cancelled C-0002 as `cancelled`), C-0005 only in `total`, which counts
/// C-0001 once. M: `total` as the sum of the departments.
#[test]
fn da4_departments_and_the_total() {
    let document = served(&Facts::scenario().judge());
    assert_eq!(document["departments"], pinned()["departments"]);
    assert_eq!(document["total"], pinned()["total"]);
    assert_eq!(document["total"]["tasks"], 4);
}

/// DA-4: the implicit workspace (`flows` `None`) is one row equal to the
/// total; a card of a flow no workspace lists counts only in `null` ones;
/// no department: no row, the total still counts.
#[test]
fn da4_the_implicit_department() {
    let mut facts = Facts::scenario();
    facts.departments = vec![Workspace::implicit()];
    let document = served(&facts.judge());
    let row = &document["departments"][0];
    assert_eq!(row["name"], "all");
    for (key, value) in document["total"].as_object().expect("total") {
        assert_eq!(&row[key], value, "{key}");
    }
    facts.departments = vec![
        workspace("words", "Words", Some(&["copy"])),
        workspace("all", "Everything", None),
    ];
    let document = served(&facts.judge());
    assert_eq!(late(&document, "C-0003")["departments"], json!(["all"]));
    assert_eq!(
        late(&document, "C-0001")["departments"],
        json!(["words", "all"])
    );
    facts.departments = Vec::new();
    let document = served(&facts.judge());
    assert_eq!(document["departments"], json!([]));
    assert_eq!(document["total"], pinned()["total"]);
}

// ------------------------------------------------------------------ DA-5

/// DA-5: C-0003 (due 11-12) is overdue at 11-20 by 8, at 11-13 by 1, not
/// at 11-01 nor at 11-12; C-0001 is late by 5 at every `as_of`. A card
/// with no `due` is neither; a cancelled card past its `due` is neither. M:
/// `<=` for `<`.
#[test]
fn da5_overdue_against_as_of() {
    for (as_of, days) in [
        ("2026-11-01", None),
        ("2026-11-12", None),
        ("2026-11-13", Some(1)),
        (AS_OF, Some(8)),
        ("2027-01-01", Some(50)),
    ] {
        let mut facts = Facts::scenario();
        facts.as_of = as_of.to_owned();
        let document = served(&facts.judge());
        assert_eq!(document["as_of"], as_of);
        let ids = late_ids(&document);
        match days {
            Some(days) => assert_eq!(late(&document, "C-0003")["days_late"], days, "{as_of}"),
            None => assert!(!ids.contains(&"C-0003".to_owned()), "{as_of}: {ids:?}"),
        }
        assert_eq!(
            document["total"]["overdue"],
            u64::from(days.is_some()),
            "{as_of}"
        );
        assert_eq!(late(&document, "C-0001")["days_late"], 5, "{as_of}");
        assert_eq!(document["total"]["delivered_late"], 1, "{as_of}");
    }
    let mut facts = Facts::scenario();
    facts.cards.push(cancelled(
        due(
            card("C-0006", "Drop the old chart", "copy", "todo", 1),
            "2026-11-02",
        ),
        &at(10),
        "Not needed",
    ));
    facts.cards.push(accepted(
        card("C-0007", "No date", "copy", "done", 1),
        &at(19),
    ));
    let document = served(&facts.judge());
    assert_eq!(late_ids(&document), ["C-0003", "C-0001"]);
    assert_eq!(document["total"]["cancelled"], 2);
    assert_eq!(document["total"]["overdue"], 1);
    assert_eq!(document["total"]["delivered_late"], 1);
}

/// DA-5: on time is no lateness: accepted on its `due` day is not late.
#[test]
fn da5_accepted_on_the_due_day_is_on_time() {
    let mut facts = Facts::scenario();
    facts.cards.push(accepted(
        due(
            card("C-0006", "On the day", "copy", "done", 1),
            "2026-11-15",
        ),
        "2026-11-15T23:59:59Z",
    ));
    let document = served(&facts.judge());
    assert!(!late_ids(&document).contains(&"C-0006".to_owned()));
    assert_eq!(document["total"]["delivered_late"], 1);
}

// ------------------------------------------------------------------ DA-6

fn keys(value: &Value, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                out.push((path.to_owned(), key.clone()));
                keys(inner, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(items) => {
            for inner in items {
                keys(inner, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// DA-6: every serialized key of the judged scenario: none holding
/// `estimate` but a late row's `estimate` and the `reestimates` lists; none
/// naming a person. M: an `estimate_sum` key; `assignee` on a late row.
#[test]
fn da6_no_sum_no_person() {
    let document = served(&Facts::scenario().judge());
    let mut found = Vec::new();
    keys(&document, "", &mut found);
    for (path, key) in &found {
        if key.contains("estimate") {
            assert!(
                (path == ".late[]" && (key == "estimate" || key == "reestimates"))
                    || (path == ".late[].more" && key == "reestimates"),
                "`{key}` at `{path}`"
            );
        }
        assert!(
            ![
                "as",
                "by",
                "assignee",
                "reviewer",
                "created_by",
                "decided_by"
            ]
            .contains(&key.as_str()),
            "`{key}` at `{path}`"
        );
    }
    let text = document.to_string();
    assert!(!text.contains("Ann") && !text.contains("Bob"), "{text}");
}

// ------------------------------------------------------------------ DA-8

/// DA-8: a noted move into another column and a noted reorder inside the
/// return column are no returns; a move out of it is none. M: any move with
/// a note taken as a return.
#[test]
fn da8_only_a_move_into_the_return_column_returns() {
    let mut facts = Facts::scenario();
    for moved in &mut facts.moves {
        if moved.1 == "C-0001" && moved.3 == "review" && moved.2 == "doing" {
            moved.4 = Some("Ready for a read".to_owned());
        }
        if moved.1 == "C-0001" && moved.2 == "returned" {
            moved.4 = Some("Shorter now".to_owned());
        }
    }
    facts.moved(
        "C-0001",
        "returned",
        "returned",
        Some("Still too long"),
        &at(6),
    );
    assert_eq!(served(&facts.judge()), pinned());
}

/// DA-8: a flow no longer configured has no return column, so no returns.
#[test]
fn da8_a_flow_gone_has_no_returns() {
    let mut facts = Facts::scenario();
    facts.returns.clear();
    let document = served(&facts.judge());
    assert_eq!(late(&document, "C-0001")["returns"], json!([]));
    assert_eq!(late(&document, "C-0001")["more"]["returns"], 0);
}

/// DA-8: twenty of each reason list the oldest `REASONS_MAX` (16), the rest
/// counted in `more`: returns and re-estimates by `seq`, cancelled
/// subtasks and items by ID number. M: the lists uncapped; the newest kept.
#[test]
fn da8_each_list_keeps_its_oldest_16() {
    assert_eq!(REASONS_MAX, 16);
    let mut facts = Facts::scenario();
    for n in 1..=20 {
        let when = format!("2026-11-1{}T{:02}:00:00Z", n / 10, n);
        facts.moved(
            "C-0001",
            "review",
            "returned",
            Some(&format!("Return {n}")),
            &when,
        );
        facts.moved("C-0001", "returned", "review", None, &when);
        facts.reestimated("C-0001", Some("5"), Some("5"), &format!("Again {n}"), &when);
        facts.cards.push(cancelled(
            parent(
                card(
                    &format!("C-{:04}", 100 + n),
                    &format!("Plate {n}"),
                    "plates",
                    "todo",
                    2,
                ),
                "C-0001",
            ),
            &at(9),
            &format!("Reason {n}"),
        ));
        facts.items.push((
            format!("PR-{:04}", 100 + n),
            "C-0001".to_owned(),
            "decision".to_owned(),
            "open".to_owned(),
        ));
    }
    let document = served(&facts.judge());
    let row = late(&document, "C-0001");
    let lists = ["returns", "reestimates", "cancelled_subtasks", "items"];
    for list in lists {
        assert_eq!(
            row[list].as_array().expect(list).len(),
            REASONS_MAX,
            "{list}"
        );
    }
    assert_eq!(
        row["more"],
        json!({"returns": 6, "reestimates": 5, "cancelled_subtasks": 5, "items": 5}),
        "2 + 20 returns, 1 + 20 re-estimates, 1 + 20 subtasks, 1 + 20 items"
    );
    assert_eq!(row["returns"][0]["note"], "Too long");
    assert_eq!(row["returns"][15]["note"], "Return 14");
    assert_eq!(row["reestimates"][0]["reason"], "The tables doubled");
    assert_eq!(row["reestimates"][15]["reason"], "Again 15");
    assert_eq!(row["cancelled_subtasks"][0]["id"], "C-0002");
    assert_eq!(row["cancelled_subtasks"][15]["id"], "C-0115");
    assert_eq!(row["items"][0]["id"], "PR-0040");
    assert_eq!(row["items"][15]["id"], "PR-0115");
}

// ------------------------------------------------------------------ DA-9

/// DA-9: the median is the lower of the two middles: over 3 and 14, 3;
/// over 1, 2, 3, 4, 2; over 1, 5, 9, 5; `null` with none delivered. M: the
/// upper middle.
#[test]
fn da9_the_median_is_the_lower_middle() {
    let lead = |leads: &[u32]| {
        let mut facts = Facts::new();
        for (n, days) in leads.iter().enumerate() {
            let id = format!("C-{:04}", n + 1);
            facts.cards.push(accepted(
                card(&id, "Done", "copy", "done", 1),
                &at(1 + days),
            ));
        }
        served(&facts.judge())["total"]["lead_days"].clone()
    };
    assert_eq!(lead(&[3, 14]), json!({"median": 3, "max": 14}));
    assert_eq!(lead(&[14, 3]), json!({"median": 3, "max": 14}));
    assert_eq!(lead(&[4, 1, 3, 2]), json!({"median": 2, "max": 4}));
    assert_eq!(lead(&[9, 1, 5]), json!({"median": 5, "max": 9}));
    assert_eq!(lead(&[0]), json!({"median": 0, "max": 0}));
    assert_eq!(lead(&[]), Value::Null);
}

/// DA-9 (notes): a delivered card whose `created_at` or `acceptance.at` is
/// no time stamp: its note, no lead; it still counts as delivered.
#[test]
fn da9_a_time_stamp_that_does_not_read_is_a_note() {
    let mut facts = Facts::scenario();
    let mut odd = accepted(card("C-0006", "Odd clock", "copy", "done", 1), &at(4));
    odd.created_at = "yesterday".to_owned();
    facts.cards.push(odd);
    let judged = facts.judge();
    assert_eq!(
        judged.notes,
        ["C-0006: yesterday is no timestamp; no lead time"]
    );
    let document = served(&judged);
    assert_eq!(document["total"]["delivered"], 3);
    assert_eq!(
        document["total"]["lead_days"],
        json!({"median": 3, "max": 14})
    );
}

// ------------------------------------------------- risk: day arithmetic

/// `unix_days` inverts `date_from_unix_days` on every day from 1970-01-01
/// to 2100-12-31, and reads a time stamp's date part.
#[test]
fn unix_days_round_trips_1970_to_2100() {
    let last = unix_days("2100-12-31").expect("a date");
    assert_eq!(unix_days("1970-01-01"), Some(0));
    for day in 0..=last {
        let date = date_from_unix_days(day);
        assert_eq!(unix_days(&date), Some(day), "{date}");
    }
    assert_eq!(unix_days("2026-11-20T09:00:00Z"), unix_days("2026-11-20"));
    assert_eq!(
        unix_days("2024-02-29").map(|d| d + 1),
        unix_days("2024-03-01")
    );
    for bad in [
        "2026-13-01",
        "2026-02-29",
        "2026-11-20 09:00",
        "",
        "20261120",
    ] {
        assert_eq!(unix_days(bad), None, "{bad:?}");
    }
}

// ------------------------------------------------- delivery-timeline

/// The judged document with the timeline asked, as JSON.
fn with_timeline(facts: &mut Facts) -> Value {
    facts.timeline = true;
    served(&facts.judge())
}

/// The timeline's rows' IDs, in served order.
fn row_ids(timeline: &Value) -> Vec<String> {
    timeline["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID").to_owned())
        .collect()
}

fn row<'d>(timeline: &'d Value, id: &str) -> &'d Value {
    timeline["tasks"]
        .as_array()
        .expect("tasks")
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no row {id}: {timeline}"))
}

/// The scenario's timeline, the spec's.
fn pinned_timeline() -> Value {
    serde_json::from_str(TIMELINE_JSON).expect("the spec's timeline")
}

/// DT1: the scenario's timeline is the spec's (C-0004 taken by its move to
/// `review`), whatever order the cards and events come in; no
/// C-0002 row; the rest of the document unchanged. M: cancelled cards as
/// rows; rows by `created_at` alone.
#[test]
fn dt1_the_scenarios_timeline() {
    let mut facts = Facts::scenario();
    let document = with_timeline(&mut facts);
    assert_eq!(document["timeline"], pinned_timeline(), "{document}");
    assert_eq!(
        row_ids(&document["timeline"]),
        ["C-0001", "C-0003", "C-0004", "C-0005"]
    );
    let mut part_a = document.clone();
    part_a["timeline"] = Value::Null;
    assert_eq!(part_a, pinned(), "the flag adds the timeline alone");
    let mut shuffled = Facts::scenario();
    shuffled.cards.reverse();
    shuffled.moves.reverse();
    shuffled.reestimates.reverse();
    assert_eq!(
        with_timeline(&mut shuffled)["timeline"],
        pinned_timeline(),
        "input order"
    );
    // Key order of a serialized row and marker: the spec's.
    let judged = facts.judge();
    let timeline = judged.timeline.as_ref().expect("asked");
    let text = serde_json::to_string(timeline).expect("a timeline");
    assert!(
        text.starts_with(
            "{\"from\":\"2026-11-01\",\"to\":\"2026-11-20\",\"markers\":[{\"group\":\"release\",\
             \"release\":\"r1\",\"first_taken_at\":\"2026-11-03T09:00:00Z\",\
             \"last_accepted_at\":\"2026-11-15T09:00:00Z\"},"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "\"tasks\":[{\"id\":\"C-0001\",\"title\":\"Write the tide table foreword\",\
             \"flow\":\"copy\",\"group\":\"release\",\"release\":\"r1\",\"departments\":\
             [\"words\",\"print\"],\"created_at\":\"2026-11-01T09:00:00Z\",\"taken_at\":\
             \"2026-11-03T09:00:00Z\",\"accepted_at\":\"2026-11-15T09:00:00Z\",\"due\":\
             \"2026-11-10\",\"days_late\":5}"
        ),
        "{text}"
    );
    assert_eq!(timeline.tasks.len(), judged.total.tasks);
}

/// DT1: rows by group order (plan order, then `none`), then `created_at`,
/// then ID number (C-9999 before C-10000 on one `created_at`); a cancelled
/// card, a subtask's too, has no row and counts as cancelled. M: rows by
/// `created_at` alone; cancelled cards as rows.
#[test]
fn dt1_rows_by_group_then_created_at_then_id_number() {
    let mut facts = Facts::scenario();
    facts.groups.insert(
        "RM-0004".to_owned(),
        GroupRef {
            kind: GroupKind::Pool,
            name: None,
            order: 2,
        },
    );
    facts.groups.insert(
        "RM-0005".to_owned(),
        GroupRef {
            kind: GroupKind::Unplaced,
            name: None,
            order: 3,
        },
    );
    facts.cards.extend([
        targets(
            card("C-10000", "Ten thousand", "copy", "todo", 3),
            &["RM-0004"],
        ),
        targets(card("C-9999", "Nine nine", "copy", "todo", 3), &["RM-0004"]),
        targets(
            card("C-0006", "Early pool", "copy", "todo", 1),
            &["RM-0004"],
        ),
        targets(card("C-0007", "Ferry", "copy", "todo", 1), &["RM-0005"]),
        targets(
            card("C-0008", "Late in r1", "copy", "todo", 9),
            &["RM-0001"],
        ),
        cancelled(
            targets(card("C-0009", "Dropped", "copy", "todo", 1), &["RM-0001"]),
            &at(4),
            "Not needed",
        ),
        cancelled(
            parent(card("C-0010", "Dropped child", "copy", "todo", 1), "C-0008"),
            &at(4),
            "Not needed",
        ),
    ]);
    let document = with_timeline(&mut facts);
    let timeline = &document["timeline"];
    assert_eq!(
        row_ids(timeline),
        [
            "C-0001", "C-0008", "C-0003", "C-0006", "C-9999", "C-10000", "C-0007", "C-0004",
            "C-0005"
        ],
        "{timeline}"
    );
    let groups: Vec<(Value, Value)> = timeline["markers"]
        .as_array()
        .expect("markers")
        .iter()
        .map(|marker| (marker["group"].clone(), marker["release"].clone()))
        .collect();
    assert_eq!(
        groups,
        [
            (json!("release"), json!("r1")),
            (json!("release"), json!("r2")),
            (json!("pool"), Value::Null),
            (json!("unplaced"), Value::Null),
            (json!("none"), Value::Null),
        ]
    );
    assert_eq!(row(timeline, "C-9999")["group"], "pool");
    assert_eq!(row(timeline, "C-9999")["release"], Value::Null);
    assert_eq!(document["total"]["tasks"], 9);
    assert_eq!(document["total"]["cancelled"], 3);
    assert_eq!(
        timeline["tasks"].as_array().expect("tasks").len(),
        9,
        "every card `total.tasks` counts"
    );
}

/// DT1: `days_late` is part a's step 6 and `null` on a row neither late
/// nor overdue: accepted before its due day or on it, open with its due
/// day at `as_of` or after it, no due day. M: `days_late` on an on-time
/// card.
#[test]
fn dt1_days_late_only_on_a_late_or_overdue_row() {
    let mut facts = Facts::scenario();
    facts.cards.extend([
        accepted(
            due(card("C-0006", "Early", "copy", "done", 1), "2026-11-15"),
            &at(12),
        ),
        accepted(
            due(
                card("C-0007", "On the day", "copy", "done", 1),
                "2026-11-15",
            ),
            "2026-11-15T23:59:59Z",
        ),
        due(card("C-0008", "Due at as of", "copy", "todo", 1), AS_OF),
        due(card("C-0009", "Due later", "copy", "todo", 1), "2026-12-01"),
        accepted(
            due(
                card("C-0010", "A day late", "copy", "done", 1),
                "2026-11-15",
            ),
            &at(16),
        ),
        due(
            card("C-0011", "A day over", "copy", "todo", 1),
            "2026-11-19",
        ),
    ]);
    let document = with_timeline(&mut facts);
    let timeline = &document["timeline"];
    for id in ["C-0004", "C-0005", "C-0006", "C-0007", "C-0008", "C-0009"] {
        assert_eq!(row(timeline, id)["days_late"], Value::Null, "{id}");
    }
    for (id, days) in [("C-0001", 5), ("C-0003", 8), ("C-0010", 1), ("C-0011", 1)] {
        assert_eq!(row(timeline, id)["days_late"], days, "{id}");
        assert_eq!(late(&document, id)["days_late"], days, "part a's, {id}");
    }
    assert_eq!(row(timeline, "C-0008")["due"], AS_OF);
}

/// DT1 (part a's DA-2): a card accepted straight from its first column
/// takes at its accept; its work segment is empty. A card whose
/// `created_at` does not read is counted, noted and has no row.
#[test]
fn dt1_an_accept_from_the_first_column_is_the_take() {
    let mut facts = Facts::scenario();
    facts.cards.push(accepted(
        card("C-0006", "Tune the bell", "sound", "done", 7),
        &at(8),
    ));
    facts.moved("C-0006", "todo", "done", None, &at(8));
    let mut odd = accepted(card("C-0007", "Odd clock", "copy", "done", 1), &at(4));
    odd.created_at = "yesterday".to_owned();
    facts.cards.push(odd);
    let judged = {
        facts.timeline = true;
        facts.judge()
    };
    let document = served(&judged);
    let bell = row(&document["timeline"], "C-0006");
    assert_eq!(bell["taken_at"], at(8));
    assert_eq!(bell["accepted_at"], at(8));
    assert_eq!(
        row_ids(&document["timeline"]),
        ["C-0001", "C-0003", "C-0004", "C-0005", "C-0006"]
    );
    assert_eq!(document["total"]["tasks"], 6, "C-0007 still counts");
    assert!(
        judged.notes.iter().any(|note| note.starts_with("C-0007: ")),
        "{:?}",
        judged.notes
    );
}

/// DT1 (iteration 2): asked, an open card whose `created_at` does not read
/// is noted `<C>: `<created_at>` is no timestamp; not on the timeline` and
/// has no row; a delivered one keeps part a's one note, `no lead time`,
/// alone; not asked, neither gets the timeline's note. Both still count.
/// M: the note dropped.
#[test]
fn dt1_an_open_card_off_the_timeline_is_noted() {
    let mut facts = Facts::scenario();
    let mut done = accepted(card("C-0006", "Odd clock", "copy", "done", 1), &at(4));
    done.created_at = "yesterday".to_owned();
    let mut open = card("C-0007", "Odd bell", "copy", "todo", 1);
    open.created_at = "soon".to_owned();
    facts.cards.extend([done, open]);
    let part_a = "C-0006: yesterday is no timestamp; no lead time";
    let off = "C-0007: `soon` is no timestamp; not on the timeline";
    let without = facts.judge();
    assert_eq!(without.notes, [part_a], "not asked");
    facts.timeline = true;
    let with = facts.judge();
    assert_eq!(with.notes, [part_a, off], "asked");
    let document = served(&with);
    assert_eq!(
        row_ids(&document["timeline"]),
        ["C-0001", "C-0003", "C-0004", "C-0005"]
    );
    assert_eq!(document["total"]["tasks"], 6, "both still count");
    assert_eq!(with.total, without.total);
    assert_eq!(with.late, without.late);
    assert_eq!(with.releases, without.releases);
}

/// DT2: r1 11-03 and 11-15, r2 11-05 and `null`, `none` 11-03 and 11-05
/// (C-0004 taken by its move to `review`); a release of two delivered rows keeps the
/// least take and the greatest accept, whichever row comes first; a pool
/// item's one open, untaken row `null` twice; a group of cancelled cards
/// alone no marker, and a cancelled card's earlier take counts nowhere. M:
/// the first accept as `last_accepted_at`; markers over cancelled cards.
#[test]
fn dt2_markers() {
    let mut facts = Facts::scenario();
    facts.groups.insert("RM-0003".to_owned(), release("r3", 2));
    facts.groups.insert(
        "RM-0004".to_owned(),
        GroupRef {
            kind: GroupKind::Pool,
            name: None,
            order: 3,
        },
    );
    facts.groups.insert(
        "RM-0005".to_owned(),
        GroupRef {
            kind: GroupKind::Unplaced,
            name: None,
            order: 4,
        },
    );
    // r3: C-0006 first by ID, `created_at` and accept `seq`, taken later
    // and accepted earlier than C-0007.
    facts.cards.extend([
        accepted(
            targets(
                card("C-0006", "Chart the shoals", "copy", "done", 1),
                &["RM-0003"],
            ),
            &at(8),
        ),
        accepted(
            targets(
                card("C-0007", "Chart the reefs", "copy", "done", 2),
                &["RM-0003"],
            ),
            &at(12),
        ),
        cancelled(
            targets(
                card("C-0008", "Chart the wrecks", "copy", "review", 1),
                &["RM-0003"],
            ),
            &at(9),
            "Charted last year",
        ),
        targets(
            card("C-0009", "Light the buoys", "copy", "todo", 3),
            &["RM-0004"],
        ),
        cancelled(
            targets(
                card("C-0010", "Time the ferry", "copy", "doing", 1),
                &["RM-0005"],
            ),
            &at(9),
            "No ferry",
        ),
    ]);
    facts.moved("C-0008", "todo", "review", None, &at(2));
    facts.moved("C-0010", "todo", "doing", None, &at(2));
    facts.moved("C-0007", "todo", "review", None, &at(3));
    facts.moved("C-0006", "todo", "review", None, &at(6));
    facts.moved("C-0006", "review", "done", None, &at(8));
    facts.moved("C-0007", "review", "done", None, &at(12));
    let document = with_timeline(&mut facts);
    let markers = document["timeline"]["markers"].clone();
    assert_eq!(
        markers,
        json!([
            {"group": "release", "release": "r1", "first_taken_at": at(3), "last_accepted_at": at(15)},
            {"group": "release", "release": "r2", "first_taken_at": at(5), "last_accepted_at": null},
            {"group": "release", "release": "r3", "first_taken_at": at(3), "last_accepted_at": at(12)},
            {"group": "pool", "release": null, "first_taken_at": null, "last_accepted_at": null},
            {"group": "none", "release": null, "first_taken_at": at(3), "last_accepted_at": at(5)},
        ]),
        "{markers}"
    );
    let rows = row_ids(&document["timeline"]);
    assert!(
        !rows.contains(&"C-0008".to_owned()) && !rows.contains(&"C-0010".to_owned()),
        "{rows:?}"
    );
    // Part a still counts the unplaced group's cancelled card.
    assert!(
        document["releases"]
            .as_array()
            .expect("releases")
            .iter()
            .any(|release| release["group"] == "unplaced" && release["cancelled"] == 1),
        "{}",
        document["releases"]
    );
}

/// DT4: without the flag `timeline` is `None` and the document is part a's
/// (the pinned JSON's `"timeline": null`); with it, the same document but
/// the timeline. M: the timeline built whatever the flag.
#[test]
fn dt4_no_timeline_unless_asked() {
    let facts = Facts::scenario();
    assert!(!facts.timeline);
    let judged = facts.judge();
    assert_eq!(judged.timeline, None);
    let document = served(&judged);
    assert_eq!(document["timeline"], Value::Null);
    assert_eq!(document, pinned());
    let mut empty = Facts::new();
    assert_eq!(empty.judge().timeline, None, "no card, no flag");
    empty.timeline = true;
    assert!(empty.judge().timeline.is_some(), "no card, the flag");
}

/// DT5: `from` the least `created_at` day; `to` the greatest of `as_of`,
/// the accepts and the due days: 2026-11-20 at 11-20, 2026-11-15 (C-0001's
/// accept) at 11-01, `as_of` past every date, a later due day past `as_of`.
/// The judge reads no clock: one input, one answer. M: `to` as `as_of`
/// alone.
#[test]
fn dt5_the_axis() {
    for (as_of, to) in [
        (AS_OF, AS_OF),
        ("2026-11-01", "2026-11-15"),
        ("2026-11-12", "2026-11-15"),
        ("2027-03-01", "2027-03-01"),
    ] {
        let mut facts = Facts::scenario();
        facts.as_of = as_of.to_owned();
        let first = with_timeline(&mut facts);
        assert_eq!(first["timeline"]["from"], "2026-11-01", "{as_of}");
        assert_eq!(first["timeline"]["to"], to, "{as_of}");
        assert_eq!(with_timeline(&mut facts), first, "{as_of}: twice");
    }
    let mut facts = Facts::scenario();
    facts.cards.push(due(
        card("C-0006", "Plan the spring", "copy", "todo", 7),
        "2026-12-24",
    ));
    let document = with_timeline(&mut facts);
    assert_eq!(document["timeline"]["to"], "2026-12-24");
    assert_eq!(document["timeline"]["from"], "2026-11-01");
}

/// DT5, the axis covering every stored date: `as_of` before a row's `created_at`: `to`
/// still covers every stored date (no `to` before `from`); no row: `from`,
/// `to` `null`, no marker. M: `to` as `as_of` alone.
#[test]
fn dt5_the_axis_covers_every_stored_date() {
    let mut facts = Facts::new();
    facts.as_of = "2026-10-01".to_owned();
    facts
        .cards
        .push(card("C-0001", "Sound the fog", "sound", "todo", 6));
    facts
        .cards
        .push(card("C-0002", "Ring the bell", "copy", "doing", 7));
    facts.moved("C-0002", "todo", "doing", None, &at(9));
    let document = with_timeline(&mut facts);
    let timeline = &document["timeline"];
    assert_eq!(timeline["from"], "2026-11-06", "{timeline}");
    let to = timeline["to"].as_str().expect("a day");
    for stored in ["2026-11-06", "2026-11-07", "2026-11-09"] {
        assert!(to >= stored, "`to` {to} before a stored day {stored}");
    }
    let mut empty = Facts::new();
    let document = with_timeline(&mut empty);
    assert_eq!(
        document["timeline"],
        json!({"from": null, "to": null, "markers": [], "tasks": []})
    );
    let mut cancelled_only = Facts::new();
    cancelled_only.cards.push(cancelled(
        card("C-0001", "Dropped", "copy", "todo", 1),
        &at(2),
        "Not needed",
    ));
    assert_eq!(
        with_timeline(&mut cancelled_only)["timeline"],
        json!({"from": null, "to": null, "markers": [], "tasks": []}),
        "a cancelled card alone is no task"
    );
}
