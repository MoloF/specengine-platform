//! docs/features/upstream-signal.md (RM-0105) "Acceptance criteria", the
//! judge's half: core's pure `upstream` over cards, events and items built
//! by hand, no store and no index.
//!
//! - US-1: `seq` alone orders: an edit logged at an earlier clock than the
//!   subtask's creation, but after it by `seq`, is listed; a card made later
//!   by `seq` at an earlier clock lists nothing. M: `at` compared instead of
//!   `seq`.
//! - US-2: only `goal`, `targets` and `criteria` count, `fields` keeping
//!   those in the event's order. M: any `card.edited` counted; `fields`
//!   unfiltered.
//! - US-6: the subtask's own goal, targets or criteria edit answers every
//!   change before it, its other edits end nothing; accepted or cancelled,
//!   `None`; a re-parenting edit moves the anchor. M: the child's edits
//!   ignored; any child edit ending it; the anchor from `card.created`
//!   alone.
//! - The judge's other steps (the spec's core data): items by the reach, not the
//!   child's own, applied past the floor, the last apply's `seq`; `on` the
//!   first meeting node in the item's order; `by` the decider's name, never
//!   the email; the parent's mark only when suspect or correcting; the cap
//!   of 16, the newest, oldest first, `more` the rest; no anchor: the mark
//!   alone.
//!
//! Every expected value is worked out from the spec, never read back from a
//! run.

use std::collections::BTreeSet;

use specengine_core::trace::CardMark;
use specengine_core::upstream::{
    Decided, Logged, LoggedType, UPSTREAM_FIELDS, UPSTREAM_MAX, Upstream, UpstreamChange,
    UpstreamSource, upstream,
};
use specengine_model::{Card, CardAcceptance, CardCancellation, CardVerdict};

const PARENT: &str = "C-0001";
const CHILD: &str = "C-0002";
const SIBLING: &str = "C-0003";

/// Clocks, in an order that differs from the `seq` order they are logged in.
const T1: &str = "2026-11-12T01:00:00Z";
const T5: &str = "2026-11-12T05:00:00Z";
const T9: &str = "2026-11-12T09:00:00Z";

// ------------------------------------------------------------- builders

/// An open card `id` under `parent`.
fn card(id: &str, parent: Option<&str>) -> Card {
    Card {
        id: id.to_owned(),
        project: "tide-table".to_owned(),
        flow: "copy".to_owned(),
        column: "backlog".to_owned(),
        position: 1,
        title: "Write the tide table foreword".to_owned(),
        goal: None,
        discipline: None,
        targets: vec!["TIDE-SPRING".to_owned()],
        snapshot: None,
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        mandate: None,
        estimate: None,
        due: None,
        parent: parent.map(str::to_owned),
        corrects: None,
        evidence: Vec::new(),
        acceptance: None,
        cancellation: None,
        created_by: "local".to_owned(),
        created_at: T1.to_owned(),
        updated_at: T1.to_owned(),
        revision: 1,
        tasks: Vec::new(),
    }
}

fn parent() -> Card {
    let mut parent = card(PARENT, None);
    parent.targets = vec!["TIDE-BOOK".to_owned()];
    parent
}

fn child() -> Card {
    card(CHILD, Some(PARENT))
}

fn accepted(mut card: Card) -> Card {
    card.acceptance = Some(CardAcceptance {
        by: "Ann".to_owned(),
        at: T9.to_owned(),
        note: None,
        verdict: CardVerdict::Briefed,
        departs: Vec::new(),
        discrepancy: None,
    });
    card
}

fn cancelled(mut card: Card) -> Card {
    card.cancellation = Some(CardCancellation {
        by: "Ann".to_owned(),
        at: T9.to_owned(),
        reason: "Not needed.".to_owned(),
    });
    card
}

fn created(seq: i64, id: &'static str, at: &'static str) -> Logged<'static> {
    Logged {
        seq,
        kind: LoggedType::Created,
        id,
        fields: Vec::new(),
        by: Some("local"),
        at,
    }
}

fn edited(
    seq: i64,
    id: &'static str,
    fields: &[&'static str],
    by: Option<&'static str>,
    at: &'static str,
) -> Logged<'static> {
    Logged {
        seq,
        kind: LoggedType::Edited,
        id,
        fields: fields.to_vec(),
        by,
        at,
    }
}

fn applied(seq: i64, id: &'static str, at: &'static str) -> Logged<'static> {
    Logged {
        seq,
        kind: LoggedType::Applied,
        id,
        fields: Vec::new(),
        by: None,
        at,
    }
}

/// An applied item's row: `nodes` in its order.
struct Item {
    id: &'static str,
    kind: &'static str,
    card: Option<&'static str>,
    nodes: Vec<String>,
    record: Option<&'static str>,
    decided_by: Option<&'static str>,
}

fn item(id: &'static str, kind: &'static str, nodes: &[&str]) -> Item {
    Item {
        id,
        kind,
        card: None,
        nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
        record: Some("DEC-0031"),
        decided_by: Some("Ann Owner <ann@example.org>"),
    }
}

fn decided(items: &[Item]) -> Vec<Decided<'_>> {
    items
        .iter()
        .map(|item| Decided {
            id: item.id,
            kind: item.kind,
            card: item.card,
            nodes: &item.nodes,
            record: item.record,
            decided_by: item.decided_by,
        })
        .collect()
}

/// The reach of the scenario: the parent's `TIDE-BOOK` with its nested
/// sections, the child's `TIDE-SPRING`.
fn reach() -> BTreeSet<String> {
    ["TIDE-BOOK", "TIDE-SPRING", "TIDE-NEAP"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// The judge with no mark, no item and the scenario's reach.
fn edits_only(child: &Card, log: &[Logged<'_>]) -> Option<Upstream> {
    upstream(child, &parent(), None, log, &[], &reach())
}

/// A change as one line: `seq|at|source|by|fields|item|kind|record|on`,
/// `-` for none.
fn line(change: &UpstreamChange) -> String {
    let source = match change.source {
        UpstreamSource::Edit => "edit",
        UpstreamSource::Item => "item",
    };
    let held = |value: &Option<String>| value.clone().unwrap_or_else(|| "-".to_owned());
    format!(
        "{}|{}|{source}|{}|{}|{}|{}|{}|{}",
        change.seq,
        change.at,
        held(&change.by),
        change.fields.join(","),
        held(&change.item),
        held(&change.kind),
        held(&change.record),
        held(&change.on),
    )
}

fn lines(found: &Upstream) -> Vec<String> {
    found.changes.iter().map(line).collect()
}

/// The seqs listed, oldest first.
fn seqs(found: Option<&Upstream>) -> Vec<i64> {
    found.map_or_else(Vec::new, |found| {
        found.changes.iter().map(|change| change.seq).collect()
    })
}

fn suspect() -> CardMark {
    CardMark::Suspect {
        on: "TIDE-BOOK".to_owned(),
    }
}

// ------------------------------------------------------------- constants

/// The spec's data: the cap and the counted fields, in the table's order.
#[test]
fn the_cap_and_the_counted_fields() {
    assert_eq!(UPSTREAM_MAX, 16);
    assert_eq!(UPSTREAM_FIELDS, ["goal", "targets", "criteria"]);
}

// ------------------------------------------------------------------ US-1

/// US-1: C-0001 made at T5; the split's C-0002 and C-0003 at T9 (seqs 2,
/// 3); the parent's goal edited at T5 (seq 4); C-0004 made under C-0001 at
/// T1 (seq 5). The subtasks each list the one edit, `at` T5; C-0004 lists
/// nothing. M: `at` compared instead of `seq` (C-0002 loses it, C-0004
/// gains it).
#[test]
fn us1_seq_orders_never_the_clock() {
    let log = [
        created(1, PARENT, T5),
        created(2, CHILD, T9),
        created(3, SIBLING, T9),
        edited(4, PARENT, &["goal"], Some("Ann"), T5),
        created(5, "C-0004", T1),
    ];
    for id in [CHILD, SIBLING] {
        let found = edits_only(&card(id, Some(PARENT)), &log)
            .unwrap_or_else(|| panic!("{id} lists the parent's edit"));
        assert_eq!(found.parent, PARENT);
        assert_eq!(found.mark, None);
        assert_eq!(found.more, 0);
        assert_eq!(
            lines(&found),
            [format!("4|{T5}|edit|Ann|goal|-|-|-|-")],
            "{id}"
        );
    }
    assert!(
        edits_only(&card("C-0004", Some(PARENT)), &log).is_none(),
        "C-0004 was made after the edit by seq"
    );
}

// ------------------------------------------------------------------ US-2

/// US-2: an edit of the title, due, estimate, evidence, tasks or mandate
/// says nothing; `--title X --goal Y` lists one change with `fields:
/// ["goal"]`, `--target` `["targets"]`, and several counted fields keep the
/// event's order. M: any `card.edited` counted; `fields` unfiltered.
#[test]
fn us2_goal_targets_criteria_only() {
    let mut log = vec![created(1, PARENT, T1), created(2, CHILD, T1)];
    let mut seq = 3;
    for field in ["title", "due", "estimate", "evidence", "tasks", "mandate"] {
        log.push(edited(seq, PARENT, &[field], Some("Ann"), T1));
        seq += 1;
    }
    log.push(edited(
        seq,
        PARENT,
        &["title", "due", "parent"],
        Some("Ann"),
        T1,
    ));
    assert!(
        edits_only(&child(), &log).is_none(),
        "no counted field was edited"
    );

    log.push(edited(20, PARENT, &["title", "goal"], Some("Ann"), T5));
    log.push(edited(21, PARENT, &["targets"], None, T5));
    log.push(edited(
        22,
        PARENT,
        &["criteria", "due", "goal"],
        Some("Bea"),
        T9,
    ));
    let found = edits_only(&child(), &log).expect("three counted edits");
    assert_eq!(
        lines(&found),
        [
            format!("20|{T5}|edit|Ann|goal|-|-|-|-"),
            format!("21|{T5}|edit|-|targets|-|-|-|-"),
            format!("22|{T9}|edit|Bea|criteria,goal|-|-|-|-"),
        ]
    );
}

/// "Rules and edge cases": a sibling's edit of its goal is no source, and
/// an applied event of an item no row describes counts nothing.
#[test]
fn a_siblings_edit_is_no_source() {
    let log = [
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        created(3, SIBLING, T1),
        edited(4, SIBLING, &["goal", "targets"], Some("Ann"), T1),
        applied(5, "PR-0009", T1),
    ];
    assert!(edits_only(&child(), &log).is_none());
}

// ------------------------------------------------------------------ US-6

/// US-6 (a): the subtask's own goal edit after the parent's answers it:
/// `None`; a later parent criteria edit lists that one alone. M: the
/// child's edits ignored.
#[test]
fn us6_a_the_childs_own_edit_answers_what_came_before() {
    let mut log = vec![
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        edited(3, PARENT, &["goal"], Some("Ann"), T1),
        edited(4, CHILD, &["goal"], Some("Bea"), T5),
    ];
    assert!(edits_only(&child(), &log).is_none());
    log.push(edited(5, PARENT, &["criteria"], Some("Ann"), T9));
    let found = edits_only(&child(), &log).expect("the later edit");
    assert_eq!(lines(&found), [format!("5|{T9}|edit|Ann|criteria|-|-|-|-")]);
}

/// US-6 (a), the other counted fields: the child's own targets or
/// criteria edit answers too; an item applied before it is answered.
#[test]
fn us6_a_targets_or_criteria_answer_too() {
    let items = [item("PR-0040", "decision", &["TIDE-NEAP"])];
    for field in ["targets", "criteria"] {
        let log = [
            created(1, PARENT, T1),
            created(2, CHILD, T1),
            edited(3, PARENT, &["goal"], Some("Ann"), T1),
            applied(4, "PR-0040", T1),
            edited(5, CHILD, &["title", field], Some("Bea"), T5),
        ];
        assert!(
            upstream(&child(), &parent(), None, &log, &decided(&items), &reach()).is_none(),
            "{field}"
        );
    }
}

/// US-6 (b): the child's own due, title, estimate or mandate edit ends
/// nothing. M: any child edit ending it.
#[test]
fn us6_b_the_childs_other_edits_end_nothing() {
    let log = [
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        edited(3, PARENT, &["goal"], Some("Ann"), T1),
        edited(4, CHILD, &["due"], Some("Bea"), T5),
        edited(5, CHILD, &["title", "estimate", "mandate"], Some("Bea"), T5),
        edited(6, CHILD, &["evidence", "tasks"], Some("Bea"), T5),
    ];
    let found = edits_only(&child(), &log).expect("still listed");
    assert_eq!(seqs(Some(&found)), [3]);
}

/// US-6 (c): an accepted and a cancelled subtask read `None`, over edits,
/// items and the parent's mark alike.
#[test]
fn us6_c_a_closed_subtask_reads_none() {
    let log = [
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        edited(3, PARENT, &["goal"], Some("Ann"), T1),
        applied(4, "PR-0040", T1),
    ];
    let items = [item("PR-0040", "decision", &["TIDE-NEAP"])];
    let mark = suspect();
    for closed in [accepted(child()), cancelled(child())] {
        assert!(
            upstream(
                &closed,
                &parent(),
                Some(&mark),
                &log,
                &decided(&items),
                &reach()
            )
            .is_none()
        );
    }
}

/// US-6 (d): C-0005 made with no parent, the parent edited, then C-0005
/// re-parented under it: nothing; the parent edited again: that edit. M:
/// the anchor from `card.created` alone.
#[test]
fn us6_d_re_parenting_moves_the_anchor() {
    let reparented = card("C-0005", Some(PARENT));
    let mut log = vec![
        created(1, PARENT, T1),
        created(2, "C-0005", T1),
        edited(3, PARENT, &["goal"], Some("Ann"), T1),
        edited(4, "C-0005", &["parent"], Some("Bea"), T5),
    ];
    assert!(edits_only(&reparented, &log).is_none());
    log.push(edited(5, PARENT, &["targets"], Some("Ann"), T9));
    assert_eq!(seqs(edits_only(&reparented, &log).as_ref()), [5]);
}

/// The judge, step 2: with no `card.created` or parent edit of the child in
/// the log, edits and items are skipped and the mark alone is judged.
#[test]
fn no_anchor_judges_the_mark_alone() {
    let log = [
        created(1, PARENT, T1),
        edited(3, PARENT, &["goal"], Some("Ann"), T1),
        applied(4, "PR-0040", T1),
    ];
    let items = [item("PR-0040", "decision", &["TIDE-NEAP"])];
    assert!(upstream(&child(), &parent(), None, &log, &decided(&items), &reach()).is_none());
    let mark = suspect();
    let found = upstream(
        &child(),
        &parent(),
        Some(&mark),
        &log,
        &decided(&items),
        &reach(),
    )
    .expect("the mark alone");
    assert_eq!(found.mark, Some(suspect()));
    assert!(found.changes.is_empty());
    assert_eq!(found.more, 0);
}

// ----------------------------------------------------------- the items

/// US-3 at the judge: an applied decision on a node of the reach is listed
/// with its kind, record, first meeting node and the decider's name; not
/// listed: one off the reach, the child's own, one applied before the
/// anchor, one with no applied event. M: the child's own items counted; the
/// applied `seq` ignored; open items counted.
#[test]
fn us3_items_on_the_reach_applied_after_the_anchor() {
    let mut own = item("PR-0046", "decision", &["TIDE-SPRING"]);
    own.card = Some(CHILD);
    let mut on_parent = item("PR-0047", "discrepancy", &["HARBOR-LIGHTS", "TIDE-SPRING"]);
    on_parent.card = Some(PARENT);
    on_parent.record = None;
    on_parent.decided_by = None;
    let items = [
        item("PR-0041", "question", &["TIDE-SPRING"]),
        item(
            "PR-0040",
            "decision",
            &["HARBOR-LIGHTS", "TIDE-NEAP", "TIDE-BOOK"],
        ),
        item("PR-0042", "decision", &["TIDE-SPRING"]),
        item("PR-0045", "decision", &["HARBOR-LIGHTS"]),
        own,
        on_parent,
    ];
    let log = [
        created(1, PARENT, T1),
        applied(2, "PR-0041", T1),
        created(3, CHILD, T1),
        applied(9, "PR-0040", T9),
        applied(10, "PR-0045", T9),
        applied(11, "PR-0046", T9),
        applied(12, "PR-0047", T9),
    ];
    let found =
        upstream(&child(), &parent(), None, &log, &decided(&items), &reach()).expect("two items");
    assert_eq!(
        lines(&found),
        [
            format!("9|{T9}|item|Ann Owner||PR-0040|decision|DEC-0031|TIDE-NEAP"),
            format!("12|{T9}|item|-||PR-0047|discrepancy|-|TIDE-SPRING"),
        ],
        "PR-0042 has no applied event; PR-0041 came before the anchor; PR-0045 is off the \
         reach; PR-0046 is the child's own"
    );
}

/// The judge, step 5: an item applied twice is placed by its last apply, and
/// a decider written without an email is the whole value.
#[test]
fn an_item_takes_its_last_apply() {
    let mut plain = item("PR-0040", "question", &["TIDE-BOOK"]);
    plain.decided_by = Some("Ann");
    let items = [plain];
    let log = [
        created(1, PARENT, T1),
        applied(2, "PR-0040", T1),
        created(3, CHILD, T1),
        applied(7, "PR-0040", T5),
    ];
    let found = upstream(&child(), &parent(), None, &log, &decided(&items), &reach())
        .expect("the second apply");
    assert_eq!(
        lines(&found),
        [format!(
            "7|{T5}|item|Ann||PR-0040|question|DEC-0031|TIDE-BOOK"
        )]
    );
}

/// The judge, steps 4 to 6: edits and items interleave by `seq`.
#[test]
fn edits_and_items_sort_by_seq() {
    let items = [item("PR-0040", "decision", &["TIDE-NEAP"])];
    let log = [
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        edited(4, PARENT, &["goal"], Some("Ann"), T9),
        applied(6, "PR-0040", T1),
        edited(8, PARENT, &["criteria"], Some("Ann"), T5),
    ];
    let found = upstream(&child(), &parent(), None, &log, &decided(&items), &reach())
        .expect("three changes");
    assert_eq!(seqs(Some(&found)), [4, 6, 8]);
}

// ------------------------------------------------------------ the mark

/// US-7 at the judge: the parent's mark when suspect or correcting, with
/// its keys; open, accepted and departed say nothing. M: the mark source
/// dropped; `departed` counted; the mark ended by the child's edit.
#[test]
fn us7_the_parents_mark() {
    let log = [
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        edited(3, CHILD, &["goal", "targets"], Some("Bea"), T5),
    ];
    for mark in [
        suspect(),
        CardMark::Correcting {
            on: "TIDE-BOOK".to_owned(),
            correction: "C-0009".to_owned(),
        },
    ] {
        let found = upstream(&child(), &parent(), Some(&mark), &log, &[], &reach())
            .unwrap_or_else(|| panic!("{} stands over the child's edit", mark.word()));
        assert_eq!(found.parent, PARENT);
        assert_eq!(found.mark, Some(mark.clone()));
        assert!(found.changes.is_empty());
        assert_eq!(found.more, 0);
    }
    for mark in [
        CardMark::Open,
        CardMark::Accepted,
        CardMark::Departed {
            on: "TIDE-BOOK".to_owned(),
            discrepancy: Some("PR-0050".to_owned()),
        },
    ] {
        assert!(
            upstream(&child(), &parent(), Some(&mark), &log, &[], &reach()).is_none(),
            "{}",
            mark.word()
        );
    }
}

/// "Rules and edge cases": a cancelled parent has no mark; its edits made
/// before the cancel still count.
#[test]
fn a_cancelled_parents_edits_still_count() {
    let log = [
        created(1, PARENT, T1),
        created(2, CHILD, T1),
        edited(3, PARENT, &["goal"], Some("Ann"), T1),
    ];
    let mark = suspect();
    let found = upstream(
        &child(),
        &cancelled(parent()),
        Some(&mark),
        &log,
        &[],
        &reach(),
    )
    .expect("the edit before the cancel");
    assert_eq!(found.mark, None, "a cancelled parent has none");
    assert_eq!(seqs(Some(&found)), [3]);
}

// ------------------------------------------------------------- the cap

/// US-5 at the judge: twenty counted parent edits list the newest 16,
/// oldest first, `more` 4. M: no cap; the oldest kept.
#[test]
fn us5_the_newest_sixteen_oldest_first() {
    let mut log = vec![created(1, PARENT, T1), created(2, CHILD, T1)];
    for seq in 10..30 {
        log.push(edited(seq, PARENT, &["goal"], Some("Ann"), T1));
    }
    let found = edits_only(&child(), &log).expect("twenty edits");
    assert_eq!(seqs(Some(&found)), (14..30).collect::<Vec<i64>>());
    assert_eq!(found.more, 4);

    let mut sixteen = vec![created(1, PARENT, T1), created(2, CHILD, T1)];
    for seq in 10..26 {
        sixteen.push(edited(seq, PARENT, &["goal"], Some("Ann"), T1));
    }
    let found = edits_only(&child(), &sixteen).expect("sixteen edits");
    assert_eq!(found.changes.len(), 16);
    assert_eq!(found.more, 0);
}
