//! docs/features/collision-pairs.md (RM-0088, the pair half)
//! "Acceptance criteria", the core's half: the pure rule `pair_applies` and
//! `collision_pair` over evidence files written here. Every expected value
//! is worked out from the spec's rule (an evidence item is a side's when its
//! `file` equals that side's `path` byte for byte; one path on both sides
//! puts it under both; a side with no path holds none; `other` the rest;
//! every list ascends), never read back from a run.
//!
//! - PP1, the rule: the example's three files give `left [0]`, `right [1]`,
//!   `other [2]`; the same files in another order give the indices of their
//!   positions, so grouping by position is red. `left` is the first target
//!   given, `right` the second. M: evidence grouped by position, not
//!   `file`; `left` and `right` swapped.
//! - PP2, the rule: only `discrepancy` with `gap_type` `contradicts` and two
//!   targets applies; three, one, none, another gap type, no gap type and
//!   another kind do not. M: served for any two-target discrepancy.
//! - PP3, the rule: one file on both sides is under both and not under
//!   `other`; a side with `path` `None` (a gone second target) holds none
//!   and keeps its ID; `./docs/a.md` is not `docs/a.md`. M: a shared file's
//!   evidence under `left` alone.
//! - Every index of `evidence` is in `left`, `right` or `other`, each list
//!   ascending, `other` disjoint from both sides; the rule is pure (a second
//!   call equal).
//!
//! The paths are a lamp keeper's (invented here) beside spec-a's two
//! movement files, so two projects of different nature give the rule its
//! inputs (ADR-0008).

use specengine_core::pair::{Pair, PairSide, PairTarget, collision_pair, pair_applies};

const SPRINT: &str = "docs/spec/movement/sprint.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";
const CODE: &str = "src/stamina.rs";

/// One side as plain values: `(id, title, path, evidence)`.
type Side = (Option<String>, Option<String>, Option<String>, Vec<usize>);

fn side(side: &PairSide) -> Side {
    (
        side.id.clone(),
        side.title.clone(),
        side.path.clone(),
        side.evidence.clone(),
    )
}

fn want(id: Option<&str>, title: Option<&str>, path: Option<&str>, evidence: &[usize]) -> Side {
    (
        id.map(str::to_owned),
        title.map(str::to_owned),
        path.map(str::to_owned),
        evidence.to_vec(),
    )
}

fn shape(pair: &Pair) -> (Side, Side, Vec<usize>) {
    (side(&pair.left), side(&pair.right), pair.other.clone())
}

fn target<'a>(
    id: Option<&'a str>,
    title: Option<&'a str>,
    path: Option<&'a str>,
) -> PairTarget<'a> {
    PairTarget { id, title, path }
}

fn cost() -> PairTarget<'static> {
    target(Some("RULE-SPRINT-COST"), Some("Cost"), Some(SPRINT))
}

fn regen() -> PairTarget<'static> {
    target(Some("RULE-STAM-REGEN"), Some("Regeneration"), Some(STAMINA))
}

/// Asserts the closing rule over one pair: every index of `0..n` in a side
/// or `other`, each list ascending without repeats, `other` holding none of
/// a side's.
fn assert_covers(pair: &Pair, n: usize, context: &str) {
    for (name, list) in [
        ("left", &pair.left.evidence),
        ("right", &pair.right.evidence),
        ("other", &pair.other),
    ] {
        assert!(
            list.windows(2).all(|w| w[0] < w[1]),
            "{context}: {name} ascends without repeats: {list:?}"
        );
        assert!(
            list.iter().all(|&index| index < n),
            "{context}: {name} within 0..{n}: {list:?}"
        );
    }
    for index in 0..n {
        let in_side = pair.left.evidence.contains(&index) || pair.right.evidence.contains(&index);
        let in_other = pair.other.contains(&index);
        assert!(
            in_side != in_other,
            "{context}: index {index} is in a side or in other, exactly one: {:?}",
            shape(pair)
        );
    }
}

// ------------------------------------------------------------------ PP2

/// PP2: `pair_applies` holds for a two-target `contradicts` discrepancy
/// alone. M: served for any two-target discrepancy (the `missing` row).
#[test]
fn pp2_only_a_two_target_contradicts_discrepancy_applies() {
    assert!(pair_applies("discrepancy", Some("contradicts"), 2));
    let not: [(&str, Option<&str>, usize); 12] = [
        ("discrepancy", Some("contradicts"), 3),
        ("discrepancy", Some("contradicts"), 1),
        ("discrepancy", Some("contradicts"), 0),
        ("discrepancy", Some("contradicts"), 16),
        ("discrepancy", Some("missing"), 2),
        ("discrepancy", Some("partial"), 2),
        ("discrepancy", Some("extra"), 2),
        ("discrepancy", None, 2),
        ("question", Some("contradicts"), 2),
        ("question", None, 2),
        ("update", None, 2),
        ("decision", Some("contradicts"), 2),
    ];
    for (kind, gap, targets) in not {
        assert!(
            !pair_applies(kind, gap, targets),
            "{kind} {gap:?} with {targets} target(s) takes no pair"
        );
    }
}

// ------------------------------------------------------------------ PP1

/// PP1: the example's three evidence files: `left [0]`, `right [1]`,
/// `other [2]`, each side the target's ID, title and path as given.
#[test]
fn pp1_the_example_groups_by_file() {
    let pair = collision_pair(cost(), regen(), &[SPRINT, STAMINA, CODE]);
    assert_eq!(
        shape(&pair),
        (
            want(Some("RULE-SPRINT-COST"), Some("Cost"), Some(SPRINT), &[0]),
            want(
                Some("RULE-STAM-REGEN"),
                Some("Regeneration"),
                Some(STAMINA),
                &[1]
            ),
            vec![2],
        )
    );
    assert_covers(&pair, 3, "the example");
    // Pure: a second call is equal.
    assert_eq!(
        shape(&collision_pair(cost(), regen(), &[SPRINT, STAMINA, CODE])),
        shape(&pair)
    );
}

/// PP1: the same files in another order give their positions' indices, so
/// a rule grouping by position (`[0]`, `[1]`, the rest) is red. M: evidence
/// grouped by position, not `file`.
#[test]
fn pp1_the_grouping_follows_the_file_not_the_position() {
    let pair = collision_pair(cost(), regen(), &[CODE, STAMINA, SPRINT]);
    assert_eq!(
        (
            pair.left.evidence.clone(),
            pair.right.evidence.clone(),
            pair.other.clone()
        ),
        (vec![2], vec![1], vec![0])
    );
    // Several items a side, interleaved: each list ascends.
    let files = [STAMINA, SPRINT, CODE, SPRINT, STAMINA, CODE, SPRINT];
    let pair = collision_pair(cost(), regen(), &files);
    assert_eq!(
        (
            pair.left.evidence.clone(),
            pair.right.evidence.clone(),
            pair.other.clone()
        ),
        (vec![1, 3, 6], vec![0, 4], vec![2, 5])
    );
    assert_covers(&pair, files.len(), "interleaved");
}

/// PP1: `left` is the first target given, `right` the second; swapping the
/// two targets swaps the sides whole. M: `left` and `right` swapped.
#[test]
fn pp1_left_is_the_first_target() {
    let pair = collision_pair(regen(), cost(), &[SPRINT, STAMINA, CODE]);
    assert_eq!(
        shape(&pair),
        (
            want(
                Some("RULE-STAM-REGEN"),
                Some("Regeneration"),
                Some(STAMINA),
                &[1]
            ),
            want(Some("RULE-SPRINT-COST"), Some("Cost"), Some(SPRINT), &[0]),
            vec![2],
        )
    );
}

// ------------------------------------------------------------------ PP3

/// PP3: two targets in one file (`MEC-STAMINA` and `RULE-STAM-REGEN`, both
/// in `stamina.md`): its evidence under both sides, never under `other`.
/// M: a shared file's evidence under `left` alone.
#[test]
fn pp3_one_file_on_both_sides_is_under_both() {
    let stamina = target(Some("MEC-STAMINA"), Some("Stamina"), Some(STAMINA));
    let pair = collision_pair(stamina, regen(), &[SPRINT, STAMINA, CODE]);
    assert_eq!(
        shape(&pair),
        (
            want(Some("MEC-STAMINA"), Some("Stamina"), Some(STAMINA), &[1]),
            want(
                Some("RULE-STAM-REGEN"),
                Some("Regeneration"),
                Some(STAMINA),
                &[1]
            ),
            vec![0, 2],
        )
    );
    assert_covers(&pair, 3, "one file");
}

/// PP3: a gone second target (`title` and `path` `None`, the ID kept) holds
/// no evidence, its file's items under `other`; a gone first target keeping
/// its stored path still groups by it. M: `null` for a gone side is the
/// caller's (the CLI's `collision_pairs.rs`).
#[test]
fn pp3_a_side_with_no_path_holds_none() {
    let gone = target(Some("RULE-STAM-REGEN"), None, None);
    let pair = collision_pair(cost(), gone, &[SPRINT, STAMINA, CODE]);
    assert_eq!(
        shape(&pair),
        (
            want(Some("RULE-SPRINT-COST"), Some("Cost"), Some(SPRINT), &[0]),
            want(Some("RULE-STAM-REGEN"), None, None, &[]),
            vec![1, 2],
        )
    );
    assert_covers(&pair, 3, "a gone right");
    // The first target gone: its stored `target_path` still names the file.
    let gone_left = target(Some("RULE-SPRINT-COST"), None, Some(SPRINT));
    let pair = collision_pair(gone_left, regen(), &[SPRINT, STAMINA, CODE]);
    assert_eq!(
        side(&pair.left),
        want(Some("RULE-SPRINT-COST"), None, Some(SPRINT), &[0])
    );
    assert_eq!(pair.other, vec![2]);
    // Both without a path: everything is other.
    let pair = collision_pair(
        target(Some("A-1"), None, None),
        target(Some("A-2"), None, None),
        &[SPRINT, STAMINA],
    );
    assert_eq!(
        (
            pair.left.evidence.clone(),
            pair.right.evidence.clone(),
            pair.other.clone()
        ),
        (vec![], vec![], vec![0, 1])
    );
}

/// PP3 ("Out of scope": no judging of `./` prefixes): a file is compared
/// byte for byte, so `./docs/a.md`, `docs/a.md/` and a case change are no
/// side's.
#[test]
fn pp3_the_file_is_compared_byte_for_byte() {
    let lamp = target(
        Some("LMP-OIL"),
        Some("Oil reserve"),
        Some("docs/lamp/oil.md"),
    );
    let wick = target(Some("LMP-WICK"), Some("Wick"), Some("docs/lamp/wick.md"));
    let files = [
        "./docs/lamp/oil.md",
        "docs/lamp/oil.md",
        "docs/lamp/Oil.md",
        "docs/lamp/wick.md ",
        "docs/lamp//wick.md",
        "docs/lamp/wick.md",
        "",
    ];
    let pair = collision_pair(lamp, wick, &files);
    assert_eq!(
        (
            pair.left.evidence.clone(),
            pair.right.evidence.clone(),
            pair.other.clone()
        ),
        (vec![1], vec![5], vec![0, 2, 3, 4, 6])
    );
    assert_covers(&pair, files.len(), "byte for byte");
}

/// A path target (`id` `None`): the side keeps the path and groups by it;
/// no evidence at all gives three empty lists.
#[test]
fn a_path_target_and_no_evidence() {
    let path_side = target(None, Some("Lamp room"), Some("docs/lamp/room.md"));
    let pair = collision_pair(path_side, regen(), &["docs/lamp/room.md", STAMINA]);
    assert_eq!(
        side(&pair.left),
        want(None, Some("Lamp room"), Some("docs/lamp/room.md"), &[0])
    );
    assert_eq!(pair.right.evidence, vec![1]);
    let pair = collision_pair(cost(), regen(), &[]);
    assert_eq!(
        (
            pair.left.evidence.clone(),
            pair.right.evidence.clone(),
            pair.other.clone()
        ),
        (vec![], vec![], vec![])
    );
}

/// The closing rule over every arrangement of up to eight items drawn from
/// four files (two sides', a shared one, a stray): each index lands where
/// the byte rule puts it.
#[test]
fn every_index_lands_where_the_rule_puts_it() {
    let pool = [SPRINT, STAMINA, CODE, "./docs/spec/movement/sprint.md"];
    for seed in 0..512usize {
        let n = seed % 9;
        let files: Vec<&str> = (0..n)
            .map(|i| pool[(seed / (i + 1) + i) % pool.len()])
            .collect();
        let shared = seed % 3 == 0;
        let right = if shared {
            target(Some("MEC-SPRINT"), Some("Sprint"), Some(SPRINT))
        } else {
            regen()
        };
        let right_path = if shared { SPRINT } else { STAMINA };
        let pair = collision_pair(cost(), right, &files);
        assert_covers(&pair, n, &format!("seed {seed}: {files:?}"));
        let left: Vec<usize> = (0..n).filter(|&i| files[i] == SPRINT).collect();
        let right: Vec<usize> = (0..n).filter(|&i| files[i] == right_path).collect();
        let other: Vec<usize> = (0..n)
            .filter(|&i| files[i] != SPRINT && files[i] != right_path)
            .collect();
        assert_eq!(
            (
                pair.left.evidence.clone(),
                pair.right.evidence.clone(),
                pair.other.clone()
            ),
            (left, right, other),
            "seed {seed}: {files:?}"
        );
    }
}
