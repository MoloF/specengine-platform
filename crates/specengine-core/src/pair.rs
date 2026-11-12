//! A two-sided collision's pair, the pure half (task spec `collision-pairs`
//! PP1 to PP3): which item serves one, and which of its evidence cites each
//! side's file. Nothing is read, written, stored or resolved here: the
//! caller resolves the two targets now, and only when [`pair_applies`]
//! holds (PP6). No project's word appears (ADR-0008): a path is compared
//! byte for byte, never judged or normalised.
//!
//! - **Which item**: a `discrepancy` whose gap type is `contradicts` and
//!   whose stored targets are exactly two, any status (PP2).
//! - **Sides**: `left` the first stored target, `right` the second; each
//!   holds the positions, ascending, of the evidence whose `file` equals
//!   its `path`; a side with no path holds none. One path on both sides
//!   puts its evidence under both (PP3).
//! - **`other`**: every position on neither side, ascending, so every
//!   position is in `left`, `right` or `other` and the reader computes no
//!   remainder.

use serde::Serialize;

use crate::intake::{DISCREPANCY_KIND, GapType};

/// The stored targets a pair is served for.
pub const PAIR_TARGETS: usize = 2;

/// One target as the caller resolved it now: `id` `None` for a path
/// target, `title` `None` when gone or untitled, `path` its holder (`None`
/// when unknown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairTarget<'a> {
    pub id: Option<&'a str>,
    pub title: Option<&'a str>,
    pub path: Option<&'a str>,
}

/// One side of a pair: the target and the positions of the evidence citing
/// its file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairSide {
    pub id: Option<String>,
    pub title: Option<String>,
    pub path: Option<String>,
    /// Positions in the item's `evidence`, ascending.
    pub evidence: Vec<usize>,
}

/// A two-target `contradicts` discrepancy's pair: its two sides and the
/// evidence citing neither.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pair {
    pub left: PairSide,
    pub right: PairSide,
    /// Positions in the item's `evidence` on neither side, ascending.
    pub other: Vec<usize>,
}

/// Whether an item of `kind` with `gap_type` and `targets` stored targets
/// serves a pair: a `discrepancy`, `contradicts`, exactly
/// [`PAIR_TARGETS`] (PP2).
pub fn pair_applies(kind: &str, gap_type: Option<&str>, targets: usize) -> bool {
    kind == DISCREPANCY_KIND
        && gap_type == Some(GapType::Contradicts.as_str())
        && targets == PAIR_TARGETS
}

/// The pair of `left` (the first stored target) and `right` (the second),
/// `evidence_files` each evidence item's `file` in stored order: an item is
/// a side's when its file equals that side's path byte for byte; under both
/// when both paths are equal; under `other` when under neither.
pub fn collision_pair(
    left: PairTarget<'_>,
    right: PairTarget<'_>,
    evidence_files: &[&str],
) -> Pair {
    let mut on_left = Vec::new();
    let mut on_right = Vec::new();
    let mut other = Vec::new();
    for (index, file) in evidence_files.iter().enumerate() {
        let cites_left = left.path == Some(*file);
        let cites_right = right.path == Some(*file);
        if cites_left {
            on_left.push(index);
        }
        if cites_right {
            on_right.push(index);
        }
        if !cites_left && !cites_right {
            other.push(index);
        }
    }
    Pair {
        left: side(left, on_left),
        right: side(right, on_right),
        other,
    }
}

/// `target` owned, with its evidence positions.
fn side(target: PairTarget<'_>, evidence: Vec<usize>) -> PairSide {
    PairSide {
        id: target.id.map(str::to_owned),
        title: target.title.map(str::to_owned),
        path: target.path.map(str::to_owned),
        evidence,
    }
}
