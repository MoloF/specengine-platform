//! A direction's catalog, the pure half (task spec `direction-catalog`,
//! `docs/features/direction-catalog.md` "Acceptance criteria", DC-3,
//! DC-4): a row's last change from the newest commit naming its file, read
//! as node-history's commit entry, and each node's open items from one
//! read's open proposals.
//!
//! Nothing is read, written or stored here: the store walks git, the CLI
//! reads the tree, the trace's reading and the queue and joins them per
//! read. No clock: a change's time is its commit's.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::history::{applied_trailers, identity_name};
use crate::proposal::utc_timestamp;

/// First-parent commits one walk for last changes reads at most (DC-11):
/// a file untouched since before them reads no last change, with a note.
pub const LAST_CHANGE_SCAN_MAX: usize = 20_000;

/// One commit as the store's log reads it: `%H`, `%an`, `%ae`, `%ct` and
/// the raw message `%B`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitFacts<'a> {
    pub oid: &'a str,
    pub author_name: &'a str,
    pub author_email: &'a str,
    /// The committer time, seconds since 1970-01-01T00:00:00Z.
    pub committed_at: i64,
    pub message: &'a str,
}

/// A row's last change: node-history's commit entry's `at`, `by`,
/// `by_name`, `id` (here `commit`) and `proposal`, and whether it is its
/// file's rather than the node's own (a section's row). The 6 keys, in
/// this order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct LastChange {
    /// UTC, `YYYY-MM-DDTHH:MM:SSZ`: the committer time.
    pub at: String,
    /// As written: the `Decided-by` trailer of an apply commit, else the
    /// author `Name <email>`.
    pub by: String,
    /// [`identity_name`] of `by`.
    pub by_name: String,
    /// The full OID.
    pub commit: String,
    /// An apply commit's `Proposal:` trailer; `None` for a hand commit.
    pub proposal: Option<String>,
    /// The change is the file's: the row is a section.
    pub of_file: bool,
}

/// `commit` as a row's last change (DC-4): one commit gives one value,
/// exactly node-history's commit entry's.
pub fn last_change(commit: &CommitFacts<'_>, of_file: bool) -> LastChange {
    let applied = applied_trailers(commit.message);
    let by = applied
        .as_ref()
        .and_then(|applied| applied.decided_by.clone())
        .unwrap_or_else(|| format!("{} <{}>", commit.author_name, commit.author_email));
    LastChange {
        at: utc_timestamp(commit.committed_at),
        by_name: identity_name(&by).to_owned(),
        by,
        commit: commit.oid.to_owned(),
        proposal: applied.map(|applied| applied.proposal),
        of_file,
    }
}

/// An open proposal as the catalog counts it: the node it targets and the
/// nodes it names (an item's targets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenItem<'a> {
    /// `None` or empty: it targets no node.
    pub target_id: Option<&'a str>,
    pub node_ids: &'a [String],
}

/// Each node's count of the `items` naming it, as target or among its
/// nodes (DC-3): an item counts once per node it names, however often it
/// names it. A node no item names is absent.
pub fn open_items_by_node(items: &[OpenItem<'_>]) -> BTreeMap<String, u64> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for item in items {
        let named: BTreeSet<&str> = item
            .target_id
            .into_iter()
            .chain(item.node_ids.iter().map(String::as_str))
            .filter(|id| !id.is_empty())
            .collect();
        for id in named {
            *counts.entry(id.to_owned()).or_default() += 1;
        }
    }
    counts
}
