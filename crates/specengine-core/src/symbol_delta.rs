//! The code items changed between two sides (task spec `impact-since`,
//! canon `spec-impact`): `spec impact --since REV` compares a commit with
//! the files on disk, `spec verify --changed` `HEAD` with them, both
//! through [`compare`]. Pure: no I/O, no hash made here, no code or node
//! type. Each side comes as [`SymbolRecord`]s the caller made afresh from
//! the bytes of the files it compares: an item's `sig` and `body` digests
//! (a file compared whole gives one record, its digest in both), opaque
//! strings compared for equality only.
//!
//! - **Key**: (`file`, `qpath`, `item_kind`). Records sharing a key on one
//!   side pair with the other side's in (`line`, given order); the surplus
//!   is added or removed.
//! - **Added**: a record of the new side alone; **removed**: of the old side
//!   alone; **changed**: a pair whose `sig` or `body` differs, or with a
//!   `None` on either side (a digest that could not be made is never read
//!   as unchanged).
//! - **Order**: each list by (file bytes, line, qpath, item kind), a pair's
//!   line the new side's; then the input position.

use std::cmp::Ordering;
use std::collections::BTreeMap;

/// One item of one side as the caller made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecord {
    /// Root-relative, `/`-separated.
    pub file: String,
    pub qpath: String,
    /// The item's kind as the caller names it: opaque here.
    pub item_kind: String,
    /// 1-based.
    pub line: usize,
    /// `None`: no digest could be made.
    pub sig: Option<String>,
    pub body: Option<String>,
}

impl SymbolRecord {
    fn key(&self) -> (&str, &str, &str) {
        (&self.file, &self.qpath, &self.item_kind)
    }

    /// The pair's digests are all made and equal.
    fn same_as(&self, other: &Self) -> bool {
        self.sig.is_some()
            && self.body.is_some()
            && self.sig == other.sig
            && self.body == other.body
    }
}

/// What [`compare`] found: indexes into the sides given, each list in the
/// module's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compared {
    /// Into `new`.
    pub added: Vec<usize>,
    /// Into `old`.
    pub removed: Vec<usize>,
    /// (into `old`, into `new`).
    pub changed: Vec<(usize, usize)>,
}

impl Compared {
    /// Nothing added, removed or changed.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }
}

/// The module's order of two records, ties by position.
fn order(a: (&SymbolRecord, usize), b: (&SymbolRecord, usize)) -> Ordering {
    let (left, at) = a;
    let (right, other) = b;
    left.file
        .as_bytes()
        .cmp(right.file.as_bytes())
        .then(left.line.cmp(&right.line))
        .then(left.qpath.as_bytes().cmp(right.qpath.as_bytes()))
        .then(left.item_kind.as_bytes().cmp(right.item_kind.as_bytes()))
        .then(at.cmp(&other))
}

/// `old` against `new`: what was added, removed and changed (see the
/// module documentation).
pub fn compare(old: &[SymbolRecord], new: &[SymbolRecord]) -> Compared {
    // Each key's records on each side, by (line, position).
    type Key<'a> = (&'a str, &'a str, &'a str);
    let mut keyed: BTreeMap<Key<'_>, (Vec<usize>, Vec<usize>)> = BTreeMap::new();
    for (at, record) in old.iter().enumerate() {
        keyed.entry(record.key()).or_default().0.push(at);
    }
    for (at, record) in new.iter().enumerate() {
        keyed.entry(record.key()).or_default().1.push(at);
    }
    let mut compared = Compared::default();
    for (_, (mut olds, mut news)) in keyed {
        olds.sort_by_key(|&at| (old[at].line, at));
        news.sort_by_key(|&at| (new[at].line, at));
        let paired = olds.len().min(news.len());
        for (&before, &after) in olds.iter().zip(&news) {
            if !old[before].same_as(&new[after]) {
                compared.changed.push((before, after));
            }
        }
        compared.removed.extend_from_slice(&olds[paired..]);
        compared.added.extend_from_slice(&news[paired..]);
    }
    compared
        .added
        .sort_by(|&a, &b| order((&new[a], a), (&new[b], b)));
    compared
        .removed
        .sort_by(|&a, &b| order((&old[a], a), (&old[b], b)));
    compared
        .changed
        .sort_by(|&(_, a), &(_, b)| order((&new[a], a), (&new[b], b)));
    compared
}
