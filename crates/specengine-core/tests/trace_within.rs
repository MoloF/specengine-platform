//! docs/features/trace-truth.md, core's half: suspect against `within_hash`,
//! the note of an entry with no key, what a clear moves, what `vouched` takes,
//! through the public `trace` module over cards built by hand, and the stored
//! shape of an entry through serde.
//!
//! - "Rules and edge cases": an entry holding `within_hash` compares it; a
//!   null now reads not suspect, with the note; an entry with none compares
//!   `norm_hash`, no false mark, a note only where the target's two hashes
//!   differ. M: an absent key read as a change.
//! - "`card.cleared`": `from` and `to` the `within_hash`, else `norm_hash` on
//!   an entry with none. M: `cleared` freezing `norm_hash` alone.
//! - `vouched`: an entry's `also` holds only the kind it compares, judged by
//!   the `within_hash` key, never by comparing strings. M: `vouched` ignoring
//!   the kind.
//! - "Data": `within_hash` defaults to `None` on a stored row, and is present
//!   in what is written.

use serde_json::json;
use specengine_core::trace::{
    ALSO_MAX, HashKind, TargetNorms, TargetNow, cleared, freeze, mark_notes, refrozen, suspect_on,
    vouched,
};
use specengine_model::{Card, CardFrozen};

const TARGET: &str = "STEP-SET";
const OWN: &str = "b3:own";
const OWN_NEW: &str = "b3:own-new";
const WITHIN: &str = "b3:within";
const WITHIN_NEW: &str = "b3:within-new";

/// An entry on [`TARGET`], `also` empty.
fn frozen(norm: Option<&str>, within: Option<&str>) -> CardFrozen {
    CardFrozen {
        id: TARGET.to_owned(),
        rev: Some(1),
        norm_hash: norm.map(str::to_owned),
        within_hash: within.map(str::to_owned),
        also: Vec::new(),
    }
}

/// [`TARGET`] reading these hashes.
fn now(norm: Option<&str>, within: Option<&str>) -> TargetNorms {
    TargetNorms::from([(
        TARGET.to_owned(),
        TargetNow {
            rev: Some(1),
            norm_hash: norm.map(str::to_owned),
            within_hash: within.map(str::to_owned),
        },
    )])
}

/// A card on [`TARGET`] holding `entry`.
fn card(entry: CardFrozen) -> Card {
    Card {
        id: "C-0001".to_owned(),
        project: "p".to_owned(),
        flow: "f".to_owned(),
        column: "c".to_owned(),
        position: 1,
        title: "t".to_owned(),
        goal: None,
        discipline: None,
        targets: vec![TARGET.to_owned()],
        snapshot: Some(vec![entry]),
        criteria: Vec::new(),
        assignee: None,
        reviewer: None,
        estimate: None,
        due: None,
        parent: None,
        corrects: None,
        evidence: Vec::new(),
        acceptance: None,
        cancellation: None,
        created_by: "local".to_owned(),
        created_at: "2026-10-31T09:00:00Z".to_owned(),
        updated_at: "2026-10-31T09:00:00Z".to_owned(),
        revision: 1,
        // docs/features/card-tasks.md: the `T-` tasks it links, none here.
        tasks: Vec::new(),
    }
}

fn is_suspect(card: &Card, norms: &TargetNorms) -> bool {
    !suspect_on(card, norms).is_empty()
}

/// An entry holding `within_hash` compares it: the own hash may hold or move,
/// only `within_hash` and `also` decide. M: `within_hash` over the target
/// alone reads as the second row.
#[test]
fn an_entry_with_the_key_compares_within_hash() {
    let held = card(frozen(Some(OWN), Some(WITHIN)));
    assert!(!is_suspect(&held, &now(Some(OWN), Some(WITHIN))));
    assert!(
        is_suspect(&held, &now(Some(OWN), Some(WITHIN_NEW))),
        "own held"
    );
    assert!(
        !is_suspect(&held, &now(Some(OWN_NEW), Some(WITHIN))),
        "`within_hash` is the whole comparison"
    );

    let mut vouching = frozen(Some(OWN), Some(WITHIN));
    vouching.also.push(WITHIN_NEW.to_owned());
    assert!(!is_suspect(
        &card(vouching),
        &now(Some(OWN), Some(WITHIN_NEW))
    ));

    assert!(is_suspect(&held, &TargetNorms::new()), "a target gone");
}

/// "A null now reads not suspect, with the note `mark_notes` gives today."
#[test]
fn a_null_now_reads_not_suspect_with_the_note() {
    let held = card(frozen(Some(OWN), Some(WITHIN)));
    let norms = now(None, None);
    assert!(!is_suspect(&held, &norms));
    assert_eq!(
        mark_notes(&held, &norms),
        ["C-0001: `STEP-SET` has a null hash: it cannot be compared, so it reads not suspect"]
    );
}

/// "WHEN an entry holds no `within_hash`, suspect SHALL compare `norm_hash` as
/// today: no false mark, and a note only where the target's `within_hash` now
/// differs from its `norm_hash`." M: an absent key read as a change.
#[test]
fn an_entry_with_no_key_compares_its_own_hash() {
    let legacy = card(frozen(Some(OWN), None));
    let sectioned = now(Some(OWN), Some(WITHIN));
    assert!(!is_suspect(&legacy, &sectioned), "no false mark");
    assert_eq!(
        mark_notes(&legacy, &sectioned),
        [
            "C-0001: STEP-SET froze its own text alone: a change in its sections marks nothing \
             until an accept or a clear"
        ]
    );
    assert!(
        is_suspect(&legacy, &now(Some(OWN_NEW), Some(WITHIN))),
        "its own text marks"
    );

    let plain = now(Some(OWN), Some(OWN));
    assert!(!is_suspect(&legacy, &plain));
    assert_eq!(mark_notes(&legacy, &plain), Vec::<String>::new(), "no note");
}

/// "`from` and `to` its `within_hash` (else `norm_hash` on an entry with
/// none), so they differ even when the own hash held"; the entry is frozen
/// anew, both hashes, `also` empty. M: `cleared` freezing `norm_hash` alone.
#[test]
fn a_clear_moves_within_hash() {
    let mut entry = frozen(Some(OWN), Some(WITHIN));
    entry.also.push("b3:vouched".to_owned());
    let held = card(entry);
    let norms = now(Some(OWN), Some(WITHIN_NEW));
    let (snapshot, moved) = cleared(&held, &norms, |_| true);
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!(moved[0].from.as_deref(), Some(WITHIN));
    assert_eq!(moved[0].to.as_deref(), Some(WITHIN_NEW));
    assert_eq!(snapshot, [frozen(Some(OWN), Some(WITHIN_NEW))]);
    assert!(!is_suspect(&card(snapshot[0].clone()), &norms));

    let legacy = card(frozen(Some(OWN), None));
    let (snapshot, moved) = cleared(&legacy, &now(Some(OWN_NEW), Some(WITHIN)), |_| true);
    assert_eq!(moved[0].from.as_deref(), Some(OWN), "else `norm_hash`");
    assert_eq!(moved[0].to.as_deref(), Some(WITHIN), "the key written");
    assert_eq!(snapshot, [frozen(Some(OWN_NEW), Some(WITHIN))]);
}

/// A new card and an accept freeze both hashes; an accept over a null now
/// keeps the entry: nothing to freeze. An entry with no key takes it.
#[test]
fn a_freeze_writes_the_key() {
    let norms = now(Some(OWN), Some(WITHIN));
    assert_eq!(
        freeze(&[TARGET.to_owned()], &norms),
        [frozen(Some(OWN), Some(WITHIN))]
    );
    assert_eq!(
        refrozen(&card(frozen(Some(OWN), None)), &norms),
        [frozen(Some(OWN), Some(WITHIN))],
        "the key written"
    );
    let held = frozen(Some(OWN), Some(WITHIN));
    assert_eq!(
        refrozen(&card(held.clone()), &now(None, None)),
        [held],
        "a null now keeps the entry"
    );
}

/// "An entry compares one kind of hash and its `also` holds only that kind,
/// judged by the presence of the `within_hash` key": a keyed entry takes a
/// within pair and refuses an own one, a legacy entry the other way. On a
/// plain target's entry the two kinds are one string and the key still
/// decides. The last [`ALSO_MAX`]. M: `vouched` ignoring the kind.
#[test]
fn vouched_takes_the_kind_an_entry_compares() {
    let mut entry = frozen(Some(OWN), Some(WITHIN));
    assert!(
        !vouched(&mut entry, HashKind::Own, WITHIN, OWN_NEW),
        "never an own pair, its old hash held or not"
    );
    assert_eq!(entry.also, Vec::<String>::new());
    assert!(vouched(&mut entry, HashKind::Within, WITHIN, WITHIN_NEW));
    assert_eq!(entry.also, [WITHIN_NEW]);
    assert!(
        !vouched(&mut entry, HashKind::Within, WITHIN, WITHIN_NEW),
        "already held: no second change"
    );
    assert!(
        vouched(&mut entry, HashKind::Within, WITHIN_NEW, "b3:third"),
        "held by `also`"
    );
    assert_eq!(entry.also, [WITHIN_NEW, "b3:third"]);

    let mut plain = frozen(Some(OWN), Some(OWN));
    assert!(
        !vouched(&mut plain, HashKind::Own, OWN, OWN_NEW),
        "one string, and the key decides"
    );
    assert!(vouched(&mut plain, HashKind::Within, OWN, OWN_NEW));
    assert_eq!(plain.also, [OWN_NEW]);

    let mut legacy = frozen(Some(OWN), None);
    assert!(
        !vouched(&mut legacy, HashKind::Within, OWN, OWN_NEW),
        "no `within_hash` key"
    );
    assert!(vouched(&mut legacy, HashKind::Own, OWN, OWN_NEW));
    assert_eq!(legacy.also, [OWN_NEW]);

    let mut full = frozen(Some(OWN), Some(WITHIN));
    let mut last = WITHIN.to_owned();
    for number in 0..=ALSO_MAX {
        let next = format!("b3:{number}");
        assert!(
            vouched(&mut full, HashKind::Within, &last, &next),
            "{number}"
        );
        last = next;
    }
    assert_eq!(full.also.len(), ALSO_MAX);
    assert_eq!(full.also.first().map(String::as_str), Some("b3:1"));
    assert_eq!(full.also.last(), Some(&last));
}

/// "`within_hash` `#[serde(default)]`, so a stored row with no key reads
/// `None`"; a written entry holds the key, in the order of "Data", `null` when
/// no hash could be frozen.
#[test]
fn the_stored_shape() {
    let stored: CardFrozen = serde_json::from_value(json!({
        "id": TARGET, "rev": 1, "norm_hash": OWN, "also": [],
    }))
    .expect("a row with no key");
    assert_eq!(stored, frozen(Some(OWN), None));

    let written = serde_json::to_string(&frozen(None, None)).expect("JSON");
    assert_eq!(
        written,
        r#"{"id":"STEP-SET","rev":1,"norm_hash":null,"within_hash":null,"also":[]}"#
    );
}
