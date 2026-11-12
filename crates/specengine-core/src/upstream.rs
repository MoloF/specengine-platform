//! What changed above an open subtask, the pure half (task spec
//! `upstream-signal`, US-1 to US-7): its direct parent's goal, targets or
//! criteria edited, a question, discrepancy or decision applied on the
//! nodes both cards reach, and the parent's own suspect or correcting mark.
//!
//! Nothing is read, written or stored here: the store reads the queue's
//! events and items, the CLI reads the index and the parent's mark and
//! prints. The judge orders by `seq` alone, never by a clock, and is per
//! read: it moves, refuses and gates nothing, and no card's mark is
//! touched (`docs/features/upstream-signal.md` "Acceptance criteria",
//! US-4).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde::ser::SerializeStruct;
use specengine_model::Card;

use crate::history::identity_name;
use crate::trace::CardMark;

/// Changes an [`Upstream`] lists at most, the newest (US-5).
pub const UPSTREAM_MAX: usize = 16;

/// The fields of a `card.edited` that count, in table order (US-2).
pub const UPSTREAM_FIELDS: [&str; 3] = ["goal", "targets", "criteria"];

/// The field of a `card.edited` that moves a card's anchor (US-6 d).
const PARENT_FIELD: &str = "parent";

/// What a logged event is to the judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoggedType {
    /// `card.created`.
    Created,
    /// `card.edited`.
    Edited,
    /// `proposal.applied`.
    Applied,
}

/// One event of the queue's log as the judge reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged<'a> {
    pub seq: i64,
    pub kind: LoggedType,
    /// The payload's `id`: a card's, or an applied item's.
    pub id: &'a str,
    /// A `card.edited`'s `fields`, in the event's order; empty otherwise.
    pub fields: Vec<&'a str>,
    /// The payload's `as`.
    pub by: Option<&'a str>,
    pub at: &'a str,
}

/// One applied question, discrepancy or decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided<'a> {
    pub id: &'a str,
    /// `question`, `discrepancy` or `decision`.
    pub kind: &'a str,
    /// The card it was raised for.
    pub card: Option<&'a str>,
    /// The nodes it names, canonical, in its order.
    pub nodes: &'a [String],
    /// Its decision record's ID.
    pub record: Option<&'a str>,
    /// Who decided, `Name <email>`.
    pub decided_by: Option<&'a str>,
}

/// Where a change comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamSource {
    /// The parent's own edit.
    Edit,
    /// An applied item on the reach.
    Item,
}

impl UpstreamSource {
    /// `edit` or `item`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Item => "item",
        }
    }
}

/// One change above a subtask, keys in the document's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpstreamChange {
    /// Its event's: an edit's `card.edited`, an item's last
    /// `proposal.applied`.
    pub seq: i64,
    pub at: String,
    pub source: UpstreamSource,
    /// An edit's `as`; an item's decider's name, never an email.
    pub by: Option<String>,
    /// An edit's counted fields, in the event's order; empty on an item.
    pub fields: Vec<&'static str>,
    pub item: Option<String>,
    pub kind: Option<String>,
    pub record: Option<String>,
    /// The first of an item's nodes the reach holds.
    pub on: Option<String>,
}

/// What changed above an open subtask: the parent's mark when it says
/// something, then the changes, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    /// The parent's ID.
    pub parent: String,
    /// The parent's mark, `Suspect` or `Correcting` only.
    pub mark: Option<CardMark>,
    /// At most [`UPSTREAM_MAX`], the newest, oldest first.
    pub changes: Vec<UpstreamChange>,
    /// The changes older than those listed.
    pub more: usize,
}

/// `{parent, mark, on, correction, changes, more}`: the parent's mark word
/// and its keys, all `null` without one.
impl Serialize for Upstream {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mark = self.mark.as_ref();
        let mut document = serializer.serialize_struct("Upstream", 6)?;
        document.serialize_field("parent", &self.parent)?;
        document.serialize_field("mark", &mark.map(CardMark::word))?;
        document.serialize_field("on", &mark.and_then(CardMark::on))?;
        document.serialize_field("correction", &mark.and_then(CardMark::correction))?;
        document.serialize_field("changes", &self.changes)?;
        document.serialize_field("more", &self.more)?;
        document.end()
    }
}

/// The child's anchor: the `seq` of its latest `card.created`, or of its
/// latest `card.edited` holding `parent`; `None` when `log` holds neither
/// (US-6 d).
pub fn anchor_of(child: &str, log: &[Logged<'_>]) -> Option<i64> {
    log.iter()
        .filter(|event| {
            event.id == child
                && match event.kind {
                    LoggedType::Created => true,
                    LoggedType::Edited => event.fields.contains(&PARENT_FIELD),
                    LoggedType::Applied => false,
                }
        })
        .map(|event| event.seq)
        .max()
}

/// The fields of `event` that count, in its order, as the constants.
fn counted(event: &Logged<'_>) -> Vec<&'static str> {
    event
        .fields
        .iter()
        .filter_map(|field| UPSTREAM_FIELDS.iter().find(|known| *known == field))
        .copied()
        .collect()
}

/// What changed above `child` since its anchor, by `seq` (US-1 to US-7):
/// `parent` its direct parent, `parent_mark` the parent's mark over all its
/// targets, `log` the queue's `card.created`, `card.edited` and
/// `proposal.applied` events, `decided` its applied questions,
/// discrepancies and decisions, `reach` the names both cards' targets reach.
/// `None` when the child is accepted or cancelled, or when nothing is said.
pub fn upstream(
    child: &Card,
    parent: &Card,
    parent_mark: Option<&CardMark>,
    log: &[Logged<'_>],
    decided: &[Decided<'_>],
    reach: &BTreeSet<String>,
) -> Option<Upstream> {
    if child.acceptance.is_some() || child.cancellation.is_some() {
        return None;
    }
    let mut changes = Vec::new();
    if let Some(anchor) = anchor_of(&child.id, log) {
        // The child's own edit of goal, targets or criteria answers every
        // change before it (US-6 a); its other edits end nothing (US-6 b).
        let floor = log
            .iter()
            .filter(|event| {
                event.id == child.id
                    && event.kind == LoggedType::Edited
                    && event.seq > anchor
                    && !counted(event).is_empty()
            })
            .map(|event| event.seq)
            .fold(anchor, i64::max);
        let mut applied: BTreeMap<&str, (i64, &str)> = BTreeMap::new();
        for event in log {
            match event.kind {
                LoggedType::Edited if event.id == parent.id && event.seq > floor => {
                    let fields = counted(event);
                    if !fields.is_empty() {
                        changes.push(UpstreamChange {
                            seq: event.seq,
                            at: event.at.to_owned(),
                            source: UpstreamSource::Edit,
                            by: event.by.map(str::to_owned),
                            fields,
                            item: None,
                            kind: None,
                            record: None,
                            on: None,
                        });
                    }
                }
                LoggedType::Applied => {
                    let last = applied.entry(event.id).or_insert((event.seq, event.at));
                    if event.seq > last.0 {
                        *last = (event.seq, event.at);
                    }
                }
                _ => {}
            }
        }
        for item in decided {
            if item.card == Some(child.id.as_str()) {
                continue;
            }
            let Some(&(seq, at)) = applied.get(item.id) else {
                continue;
            };
            if seq <= floor {
                continue;
            }
            let Some(on) = item.nodes.iter().find(|node| reach.contains(*node)) else {
                continue;
            };
            changes.push(UpstreamChange {
                seq,
                at: at.to_owned(),
                source: UpstreamSource::Item,
                by: item
                    .decided_by
                    .map(|decider| identity_name(decider).to_owned()),
                fields: Vec::new(),
                item: Some(item.id.to_owned()),
                kind: Some(item.kind.to_owned()),
                record: item.record.map(str::to_owned),
                on: Some(on.clone()),
            });
        }
    }
    changes.sort_by(|one, other| {
        (one.seq, one.source, &one.item).cmp(&(other.seq, other.source, &other.item))
    });
    let more = changes.len().saturating_sub(UPSTREAM_MAX);
    changes.drain(..more);
    // A cancelled parent has no mark; its edits before the cancel still
    // count (US-7).
    let mark = parent_mark
        .filter(|_| parent.cancellation.is_none())
        .filter(|mark| matches!(mark, CardMark::Suspect { .. } | CardMark::Correcting { .. }))
        .cloned();
    if mark.is_none() && changes.is_empty() {
        return None;
    }
    Some(Upstream {
        parent: parent.id.clone(),
        mark,
        changes,
        more,
    })
}
