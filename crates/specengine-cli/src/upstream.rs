//! What changed above an open subtask (task spec `upstream-signal`): the
//! reads behind core's judge, once per call. One `events_of_types` read of
//! the queue's `card.created`, `card.edited` and `proposal.applied` events,
//! uncapped; one read of the applied questions, discrepancies and decisions,
//! every repository's (a card read runs no git); the reach from the marks'
//! own index refresh and graph (`trace::fresh_norms_reach`). A call with no
//! open subtask to judge reads none of them (US-9).
//!
//! The signal is its own key, never a mark: no mark, move, accept, clear or
//! gate reads it (US-4). Per read, never stored.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_core::trace::CardMark;
use specengine_core::upstream::{
    Decided, Logged, LoggedType, Upstream, UpstreamSource, anchor_of, upstream,
};
use specengine_model::Card;
use specengine_store::{
    EVENT_APPLIED, EVENT_CARD_CREATED, EVENT_CARD_EDITED, Event, Proposal, ProposalFilter,
    ProposalKind, ProposalQueue as _, ProposalStatus, SqliteQueue, UnreadableRow,
};

use crate::proposals::queue_cannot;
use crate::subset::seams::{self, Event as Seam};
use crate::{CliError, one_line};

/// What each asked target reaches: its stored text, then the names of its
/// first node and the ID sections nested in it (`SpecGraph::within`, the
/// mark's own reach), an ID-less document by its path.
pub(crate) type Reaches = BTreeMap<String, BTreeSet<String>>;

/// The parent `card` is judged under: an open card's, `None` for an
/// accepted or cancelled one, which reads nothing (US-6 c).
pub(crate) fn judged_parent(card: &Card) -> Option<&str> {
    if card.acceptance.is_some() || card.cancellation.is_some() {
        return None;
    }
    card.parent.as_deref()
}

/// The `show` note of a parent that does not read: no judge, `null`.
pub(crate) fn parent_unread(card: &str, parent: &str) -> String {
    one_line(&format!(
        "upstream: {card}: its parent {parent} does not read; not judged"
    ))
}

/// The `show` note of a subtask with no anchor in the log: its parent's
/// mark alone is judged.
pub(crate) fn no_anchor(card: &str) -> String {
    format!("upstream: {card}: no card.created event; only its parent's mark is read")
}

/// [`Reaches`] of `reach_of` over `places` (`trace::target_places`'s): a
/// target that no longer resolves meets only its stored text.
pub(crate) fn reaches(
    graph: &SpecGraph<'_>,
    places: &BTreeMap<String, Vec<NodeAt>>,
    reach_of: &BTreeSet<&String>,
) -> Reaches {
    reach_of
        .iter()
        .map(|target| {
            let mut names = BTreeSet::from([(*target).clone()]);
            if let Some(&at) = places.get(*target).and_then(|nodes| nodes.first()) {
                names.extend(graph.within(at).into_iter().map(|node| graph.name(node)));
            }
            ((*target).clone(), names)
        })
        .collect()
}

/// The names `targets` reach together: each one's [`Reaches`] entry, its
/// stored text alone when it has none.
pub(crate) fn reach_of<'t>(
    targets: impl IntoIterator<Item = &'t String>,
    reaches: &Reaches,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for target in targets {
        match reaches.get(target) {
            Some(reached) => names.extend(reached.iter().cloned()),
            None => {
                names.insert(target.clone());
            }
        }
    }
    names
}

/// The queue's facts the judge reads, once per call.
pub(crate) struct UpstreamFacts {
    events: Vec<Event>,
    items: Vec<Proposal>,
    /// The applied rows that do not decode: the judge cannot tell whether
    /// they meet the reach, so `show` names each (US-8).
    unreadable: Vec<UnreadableRow>,
}

impl UpstreamFacts {
    /// One `events_of_types` read and one read of the applied questions,
    /// discrepancies and decisions, of any repository; a row that does not
    /// decode is kept aside in `unreadable`, never read as clean.
    pub(crate) fn read(queue: &SqliteQueue) -> Result<Self, CliError> {
        seams::note(Seam::UpstreamEvents);
        let events = queue
            .events_of_types(&[EVENT_CARD_CREATED, EVENT_CARD_EDITED, EVENT_APPLIED])
            .map_err(queue_cannot)?;
        seams::note(Seam::UpstreamItems);
        let listed = queue
            .list_readable(&ProposalFilter {
                git_common_dir: None,
                statuses: vec![ProposalStatus::Applied],
            })
            .map_err(queue_cannot)?;
        let items = listed
            .proposals
            .into_iter()
            .filter(|item| {
                item.status == ProposalStatus::Applied
                    && matches!(
                        item.kind,
                        ProposalKind::Question | ProposalKind::Discrepancy | ProposalKind::Decision
                    )
            })
            .collect();
        Ok(Self {
            events,
            items,
            unreadable: listed.unreadable,
        })
    }

    /// `show`'s one note for the applied rows that do not decode, for
    /// `card`, whatever their count (a row cannot say which nodes it names,
    /// so none is judged on the reach): `upstream: <C>: <row>; not judged`
    /// for one, `upstream: <C>: <k> applied rows cannot be read, the first:
    /// <row>; not judged` for more, the first by ID number; none for none.
    pub(crate) fn unread_note(&self, card: &str) -> Option<String> {
        let first = self.unreadable.first()?;
        Some(one_line(&match self.unreadable.len() {
            1 => format!("upstream: {card}: {first}; not judged"),
            count => format!(
                "upstream: {card}: {count} applied rows cannot be read, the first: {first}; \
                 not judged"
            ),
        }))
    }
}

/// [`UpstreamFacts`] as core's judge takes them, grouped once, so a list's
/// many subtasks each pass the judge only what can concern it.
pub(crate) struct Judge<'f> {
    /// Each card's `card.created` and `card.edited`, by card ID.
    cards: BTreeMap<&'f str, Vec<Logged<'f>>>,
    /// Each item's `proposal.applied`, by item ID.
    applied: BTreeMap<&'f str, Vec<Logged<'f>>>,
    decided: Vec<Decided<'f>>,
}

impl<'f> Judge<'f> {
    pub(crate) fn new(facts: &'f UpstreamFacts) -> Self {
        let mut cards: BTreeMap<&str, Vec<Logged<'_>>> = BTreeMap::new();
        let mut applied: BTreeMap<&str, Vec<Logged<'_>>> = BTreeMap::new();
        for logged in facts.events.iter().filter_map(logged) {
            let group = if logged.kind == LoggedType::Applied {
                &mut applied
            } else {
                &mut cards
            };
            group.entry(logged.id).or_default().push(logged);
        }
        let decided = facts
            .items
            .iter()
            .map(|item| Decided {
                id: &item.id,
                kind: item.kind.as_str(),
                card: item.card_id.as_deref(),
                nodes: item
                    .intake
                    .as_ref()
                    .map_or(&[][..], |intake| intake.target_ids.as_slice()),
                record: item.record.as_ref().map(|record| record.id.as_str()),
                decided_by: item.decided_by.as_deref(),
            })
            .collect();
        Self {
            cards,
            applied,
            decided,
        }
    }

    /// Whether the log holds `child`'s anchor (else a `show` note).
    pub(crate) fn anchored(&self, child: &str) -> bool {
        self.cards
            .get(child)
            .is_some_and(|log| anchor_of(child, log).is_some())
    }

    /// Core's judge of `child` under `parent`: the log of the two cards and
    /// of the items whose nodes meet `reach`, the only ones it can list.
    pub(crate) fn judge(
        &self,
        child: &Card,
        parent: &Card,
        parent_mark: Option<&CardMark>,
        reach: &BTreeSet<String>,
    ) -> Option<Upstream> {
        let decided: Vec<Decided<'f>> = self
            .decided
            .iter()
            .filter(|item| item.nodes.iter().any(|node| reach.contains(node)))
            .cloned()
            .collect();
        let of = |group: &BTreeMap<&str, Vec<Logged<'f>>>, id: &str| {
            group.get(id).map_or(&[][..], Vec::as_slice).to_vec()
        };
        let mut log = of(&self.cards, &child.id);
        if parent.id != child.id {
            log.extend(of(&self.cards, &parent.id));
        }
        for item in &decided {
            log.extend(of(&self.applied, item.id));
        }
        upstream(child, parent, parent_mark, &log, &decided, reach)
    }
}

/// `event` as the judge reads it; `None` for a type it does not take or a
/// payload naming no `id`.
fn logged(event: &Event) -> Option<Logged<'_>> {
    let kind = match event.event_type.as_str() {
        EVENT_CARD_CREATED => LoggedType::Created,
        EVENT_CARD_EDITED => LoggedType::Edited,
        EVENT_APPLIED => LoggedType::Applied,
        _ => return None,
    };
    let text = |key: &str| event.payload.get(key).and_then(Value::as_str);
    let fields = match kind {
        LoggedType::Edited => event
            .payload
            .get("fields")
            .and_then(Value::as_array)
            .map(|fields| fields.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default(),
        LoggedType::Created | LoggedType::Applied => Vec::new(),
    };
    Some(Logged {
        seq: event.seq,
        kind,
        id: text("id")?,
        fields,
        by: match kind {
            LoggedType::Applied => None,
            LoggedType::Created | LoggedType::Edited => text("as"),
        },
        at: &event.at,
    })
}

/// `show`'s lines for `upstream`, after `correction:`: `upstream: -`, or the
/// parent's line, a line per change, oldest first, then `more` (US-8).
pub(crate) fn upstream_lines(upstream: Option<&Upstream>) -> Vec<String> {
    const ABSENT: &str = "-";
    let Some(upstream) = upstream else {
        return vec![format!("upstream: {ABSENT}")];
    };
    let held = |text: Option<&str>| text.map_or(ABSENT.to_owned(), one_line);
    let mut lines = vec![format!(
        "upstream: parent {} | {}",
        one_line(&upstream.parent),
        upstream
            .mark
            .as_ref()
            .map_or(ABSENT.to_owned(), |mark| one_line(&mark.text()))
    )];
    for change in &upstream.changes {
        let by = held(change.by.as_deref());
        lines.push(match change.source {
            UpstreamSource::Edit => format!(
                "upstream: {} | edited {} | {by} | {}",
                change.seq,
                change.fields.join(", "),
                one_line(&change.at)
            ),
            UpstreamSource::Item => format!(
                "upstream: {} | {} {} | record {} | on {} | {by} | {}",
                change.seq,
                held(change.kind.as_deref()),
                held(change.item.as_deref()),
                held(change.record.as_deref()),
                held(change.on.as_deref()),
                one_line(&change.at)
            ),
        });
    }
    if upstream.more > 0 {
        lines.push(format!("upstream: more {}", upstream.more));
    }
    lines
}
