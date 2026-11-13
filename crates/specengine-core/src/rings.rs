//! The scope rings, the pure half (task spec `scope-rings` SR2 to SR4): the
//! document `spec rings --json` and `GET …/rings` serve, a ring's members
//! once inner rings took theirs, and the note of a root that names no node.
//!
//! The caller reads the config ([`crate::MapConfig`]), the tree's lines per
//! ring and the trace's reading once for every sector; nothing is read,
//! written or stored here, and no clock. Rings are innermost first, in
//! config order: a node sits in the first ring listing it, at most once.
//! Nothing here names a ring, a kind or a state: the titles and kinds are
//! the project's config, the state the trace's [`Rollup`] (ADR-0008).

use std::collections::BTreeSet;

use serde::Serialize;

use crate::trace::Rollup;

/// One sector: a tree line's keys and its node's work state. The 8 keys,
/// in this order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct RingSector {
    /// Its ID, else its file's path: what a document view opens.
    pub name: String,
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// A document's `status:`, raw; `None` for a section.
    pub status: Option<String>,
    /// The trace's rollup of the node.
    pub state: Rollup,
}

/// One ring as served: its config and its sectors in the tree's order. The
/// 4 keys, in this order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Ring {
    pub title: String,
    pub root: Option<String>,
    pub kinds: Option<Vec<String>>,
    pub sectors: Vec<RingSector>,
}

/// The rings document: the notes, sorted, then the rings innermost first.
/// The 2 keys, in this order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize)]
pub struct RingsDocument {
    pub notes: Vec<String>,
    pub rings: Vec<Ring>,
}

/// Each ring's members (task spec `scope-rings` SR2): the nodes `listed`
/// gives it, in that order, less those an inner ring (an earlier entry)
/// holds and those it lists twice.
pub fn ring_members<K: Ord + Clone>(listed: Vec<Vec<K>>) -> Vec<Vec<K>> {
    let mut held: BTreeSet<K> = BTreeSet::new();
    listed
        .into_iter()
        .map(|nodes| {
            nodes
                .into_iter()
                .filter(|node| held.insert(node.clone()))
                .collect()
        })
        .collect()
}

/// A note of ring `index` (1-based) titled `title`: `text` after the ring
/// it is of (task spec `scope-rings` SR4), a root's reason for naming no
/// node among them.
pub fn ring_note(index: usize, title: &str, text: &str) -> String {
    format!("rings: ring {index} `{title}`: {text}")
}

/// The note of ring `index` (1-based) titled `title`, whose `root` is
/// located at no node (task spec `scope-rings` SR4): the ring is served
/// empty.
pub fn unresolved_root(index: usize, title: &str, root: &str) -> String {
    ring_note(index, title, &format!("`{root}` resolves to no node"))
}
