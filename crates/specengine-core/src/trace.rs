//! A card against its brief (task spec `card-trace`, "Snapshot and suspect",
//! "Rollup"): the frozen `snapshot` of a card's direct targets, what a read
//! of the reading root makes of it, and a node's computed state. Pure:
//! nothing is read or written here, and nothing computed here is stored. A
//! mark and a rollup hold for one read of one root.
//!
//! - **Suspect**: a direct target whose `within_hash` now is neither its
//!   entry's nor in its `also`, or which no longer resolves; an entry stored
//!   with no `within_hash` compares `norm_hash`, and [`mark_notes`] says what
//!   it cannot see (task spec `trace-truth`, "Rules and edge cases"). A
//!   `null` hash on either side cannot be compared: not suspect, and
//!   [`mark_notes`] says so; so does a card with no snapshot. Whitespace
//!   leaves both hashes equal, so it marks nothing. A mark stops at the file:
//!   nothing here reads `parent:`, `uses` or a sibling section.
//! - **Marks**, first match: departed (its discrepancy not settled), then
//!   suspect, which a live correction holds as correcting, then accepted or
//!   open.
//! - **Rollup**, first match: `no card`, `departed`, `suspect`, `in work`,
//!   `confirmed`. A cancelled card is never handed in here.
//!
//! Nothing here stops anything: a mark and a state are what a read says.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_model::{Card, CardFrozen, CardVerdict};

use crate::card::{AGENT_ACTOR, LOCAL_ACTOR, closed_refusal};
use crate::intake::{DiscrepancyInput, Evidence, GapType, IntakeOption, IntakeSeverity};

/// The most hashes an entry's `also` keeps: the last ones.
pub const ALSO_MAX: usize = 16;

/// A title a correction takes by default opens with this.
const CORRECT: &str = "Correct";

/// What both options of a departing accept's discrepancy cost.
pub const NOT_PRICED: &str = "not priced at acceptance";

/// What a direct target reads now. Absent from [`TargetNorms`]: it no
/// longer resolves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetNow {
    pub rev: Option<u32>,
    /// `None`: null, the node has no hash to compare.
    pub norm_hash: Option<String>,
    /// The node and the ID sections nested in it, per read and never stored;
    /// `None` when any of them reads null.
    pub within_hash: Option<String>,
}

/// Each direct target that resolves, as the reading root's index has it.
pub type TargetNorms = BTreeMap<String, TargetNow>;

/// Which of a node's two hashes an editorial change moved. On a node with no
/// sections they are one string, and it sends both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashKind {
    /// `norm_hash`, for an entry holding no `within_hash`.
    Own,
    /// `within_hash`, for an entry holding one.
    Within,
}

/// What a node's `within_hash` is taken from. Core holds no hash function, so
/// the caller hashes [`Self::Pairs`], as it does `norm_input`'s bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WithinInput {
    /// One member: `within_hash` is its `norm_hash`, the same string.
    Alone(String),
    /// Two or more: `<name>\0<norm_hash>\n` per member, by name.
    Pairs(Vec<u8>),
}

/// The input of a node's `within_hash` from its members' (name, `norm_hash`)
/// pairs: the node and the ID sections nested in it, `name` the node's ID,
/// else its file's path. `None` when any hash is null, or there is no member.
/// Sorted by name, then hash for a repeated ID, so a section moved in its
/// file is no change and a new section is one.
pub fn within_input(mut members: Vec<(String, Option<String>)>) -> Option<WithinInput> {
    if let [(_, hash)] = members.as_mut_slice() {
        return hash.take().map(WithinInput::Alone);
    }
    if members.is_empty() {
        return None;
    }
    members.sort_unstable();
    let mut input = Vec::new();
    for (name, hash) in &members {
        input.extend_from_slice(name.as_bytes());
        input.push(0);
        input.extend_from_slice(hash.as_ref()?.as_bytes());
        input.push(b'\n');
    }
    Some(WithinInput::Pairs(input))
}

fn entry_of(id: &str, now: Option<&TargetNow>) -> CardFrozen {
    CardFrozen {
        id: id.to_owned(),
        rev: now.and_then(|now| now.rev),
        norm_hash: now.and_then(|now| now.norm_hash.clone()),
        within_hash: now.and_then(|now| now.within_hash.clone()),
        also: Vec::new(),
    }
}

/// `targets` as they read now, in order: a new card's snapshot.
pub fn freeze(targets: &[String], norms: &TargetNorms) -> Vec<CardFrozen> {
    targets
        .iter()
        .map(|target| entry_of(target, norms.get(target)))
        .collect()
}

/// `edit --target`'s snapshot, `targets` first in their order: a target kept
/// keeps its entry and a new one is frozen now. A target dropped keeps its
/// entry after them, inert ([`suspect_on`] walks `targets`), so no edit, or
/// two, takes a mark away without a person and a reason: a target that
/// returns finds its old hash.
pub fn retargeted(
    stored: Option<&[CardFrozen]>,
    targets: &[String],
    norms: &TargetNorms,
) -> Vec<CardFrozen> {
    let stored = stored.unwrap_or_default();
    let mut snapshot: Vec<CardFrozen> = targets
        .iter()
        .map(|target| {
            stored
                .iter()
                .find(|entry| entry.id == *target)
                .cloned()
                .unwrap_or_else(|| entry_of(target, norms.get(target)))
        })
        .collect();
    snapshot.extend(
        stored
            .iter()
            .filter(|entry| !targets.contains(&entry.id))
            .cloned(),
    );
    snapshot
}

/// An accept's snapshot: every target with a hash now frozen anew. One that
/// no longer resolves, or reads null where its entry compares (a front-matter
/// broken at this moment), has nothing to freeze, so its entry stays: the
/// card stays comparable, and suspect on a gone target for a person to clear
/// with a reason.
pub fn refrozen(card: &Card, norms: &TargetNorms) -> Vec<CardFrozen> {
    card.targets
        .iter()
        .map(|target| {
            let now = norms.get(target);
            match entry(card, target) {
                Some(kept) if now.is_none_or(|now| compared_now(kept, now).is_none()) => {
                    kept.clone()
                }
                _ => entry_of(target, now),
            }
        })
        .collect()
}

fn entry<'c>(card: &'c Card, target: &str) -> Option<&'c CardFrozen> {
    card.snapshot
        .as_deref()?
        .iter()
        .find(|entry| entry.id == target)
}

/// The hash an entry is compared by: its `within_hash`, else, on an entry
/// holding none, its `norm_hash`.
fn compared(frozen: &CardFrozen) -> Option<&String> {
    frozen.within_hash.as_ref().or(frozen.norm_hash.as_ref())
}

/// The hash of `now` that [`compared`] of `frozen` meets, the same kind.
fn compared_now<'n>(frozen: &CardFrozen, now: &'n TargetNow) -> Option<&'n String> {
    if frozen.within_hash.is_some() {
        now.within_hash.as_ref()
    } else {
        now.norm_hash.as_ref()
    }
}

fn is_suspect(frozen: &CardFrozen, now: Option<&TargetNow>) -> bool {
    let Some(hash) = compared(frozen) else {
        return false;
    };
    let Some(now) = now else {
        return true;
    };
    compared_now(frozen, now).is_some_and(|new| new != hash && !frozen.also.contains(new))
}

/// The direct targets `card` is suspect on, in `targets` order.
pub fn suspect_on<'c>(card: &'c Card, norms: &TargetNorms) -> Vec<&'c str> {
    card.targets
        .iter()
        .filter(|target| {
            entry(card, target).is_some_and(|frozen| is_suspect(frozen, norms.get(*target)))
        })
        .map(String::as_str)
        .collect()
}

/// One target a clear moved: `card.cleared`'s `nodes`. `rev` the one it
/// reads now (the last it read, when it is gone), so a `Revision:` trailer
/// joins it to a commit: the target's own, so the join of a change in a
/// section walks `within`. `from` and `to` what each entry is compared by,
/// so they differ even when the target's own hash held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClearedNode {
    pub id: String,
    pub rev: Option<u32>,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// What clearing `card` over the targets `on` takes does: its snapshot with
/// each suspect entry among them frozen again (one that no longer resolves
/// left with a null hash, which marks nothing and is noted on every read),
/// and those entries as moved. Any other entry stays as it is, its mark too.
pub fn cleared(
    card: &Card,
    norms: &TargetNorms,
    on: impl Fn(&str) -> bool,
) -> (Vec<CardFrozen>, Vec<ClearedNode>) {
    let suspect = suspect_on(card, norms);
    let mut moved = Vec::with_capacity(suspect.len());
    let snapshot = card
        .snapshot
        .iter()
        .flatten()
        .map(|frozen| {
            if !suspect.contains(&frozen.id.as_str()) || !on(&frozen.id) {
                return frozen.clone();
            }
            let now = norms.get(&frozen.id);
            let mut again = entry_of(&frozen.id, now);
            if now.is_none() {
                again.rev = frozen.rev;
            }
            moved.push(ClearedNode {
                id: frozen.id.clone(),
                rev: again.rev,
                from: compared(frozen).cloned(),
                to: compared(&again).cloned(),
            });
            again
        })
        .collect();
    (snapshot, moved)
}

/// An editorial apply changed a node's hash of `kind` from `old` to `new`:
/// `new` joins the `also` of an entry holding `old`, the last [`ALSO_MAX`]
/// kept; `false` when the entry holds no `old`, already holds `new`, or
/// compares the other kind. An entry takes a within pair only if it holds a
/// `within_hash` and an own pair only if it holds none, judged by that key and
/// never by comparing strings, so its `also` keeps the kind it compares alone
/// and no own hash can read clean once a target's sections are gone.
pub fn vouched(entry: &mut CardFrozen, kind: HashKind, old: &str, new: &str) -> bool {
    if (kind == HashKind::Within) != entry.within_hash.is_some() {
        return false;
    }
    let holds = |entry: &CardFrozen, hash: &str| {
        compared(entry).is_some_and(|held| held == hash)
            || entry.also.iter().any(|also| also == hash)
    };
    if !holds(entry, old) || holds(entry, new) {
        return false;
    }
    entry.also.push(new.to_owned());
    let over = entry.also.len().saturating_sub(ALSO_MAX);
    entry.also.drain(..over);
    true
}

/// What a read says of a card, over the targets it was asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardMark {
    /// Not accepted, nothing marked.
    Open,
    /// Accepted, nothing marked.
    Accepted,
    /// A meaning changed under `on`.
    Suspect { on: String },
    /// An accept departs from `on`, `discrepancy` not settled (`None`: the
    /// row names none, which reads unsettled).
    Departed {
        on: String,
        discrepancy: Option<String>,
    },
    /// Suspect on `on`, held while `correction` lives.
    Correcting { on: String, correction: String },
}

impl CardMark {
    /// `open`, `accepted`, `suspect`, `departed` or `correcting`.
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::Suspect { .. } => "suspect",
            Self::Departed { .. } => "departed",
            Self::Correcting { .. } => "correcting",
        }
    }

    /// The target it is on.
    pub fn on(&self) -> Option<&str> {
        match self {
            Self::Open | Self::Accepted => None,
            Self::Suspect { on } | Self::Departed { on, .. } | Self::Correcting { on, .. } => {
                Some(on)
            }
        }
    }

    pub fn discrepancy(&self) -> Option<&str> {
        match self {
            Self::Departed { discrepancy, .. } => discrepancy.as_deref(),
            _ => None,
        }
    }

    pub fn correction(&self) -> Option<&str> {
        match self {
            Self::Correcting { correction, .. } => Some(correction),
            _ => None,
        }
    }

    /// As a line prints it: `suspect <ID>`, `departed <PR>`, `correcting
    /// <C>`, or the word alone.
    pub fn text(&self) -> String {
        match self {
            Self::Open | Self::Accepted => self.word().to_owned(),
            Self::Suspect { on } => format!("suspect {on}"),
            Self::Departed { discrepancy, .. } => {
                format!("departed {}", discrepancy.as_deref().unwrap_or("-"))
            }
            Self::Correcting { correction, .. } => format!("correcting {correction}"),
        }
    }
}

/// What a mark is read against, besides the card.
#[derive(Debug, Clone, Copy)]
pub struct MarkFacts<'a> {
    pub norms: &'a TargetNorms,
    /// The discrepancies `applied` or `rejected`.
    pub settled: &'a BTreeSet<String>,
    /// Each card a live correction corrects, and that correction.
    pub corrections: &'a BTreeMap<String, String>,
}

/// The mark of `card` over the targets `on`, its direct targets a node holds
/// or all of them, first match: departed, suspect (correcting while a
/// correction lives), accepted, open.
pub fn card_mark(card: &Card, on: &[String], facts: &MarkFacts<'_>) -> CardMark {
    if let Some(acceptance) = &card.acceptance
        && acceptance.verdict == CardVerdict::Departs
        && acceptance
            .discrepancy
            .as_ref()
            .is_none_or(|discrepancy| !facts.settled.contains(discrepancy))
        && let Some(target) = acceptance.departs.iter().find(|node| on.contains(node))
    {
        return CardMark::Departed {
            on: target.clone(),
            discrepancy: acceptance.discrepancy.clone(),
        };
    }
    let suspect = suspect_on(card, facts.norms)
        .into_iter()
        .find(|target| on.iter().any(|asked| asked == target));
    if let Some(target) = suspect {
        return match facts.corrections.get(&card.id) {
            Some(correction) => CardMark::Correcting {
                on: target.to_owned(),
                correction: correction.clone(),
            },
            None => CardMark::Suspect {
                on: target.to_owned(),
            },
        };
    }
    if card.acceptance.is_some() {
        CardMark::Accepted
    } else {
        CardMark::Open
    }
}

/// A node's computed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Rollup {
    #[serde(rename = "no card")]
    NoCard,
    #[serde(rename = "departed")]
    Departed,
    #[serde(rename = "suspect")]
    Suspect,
    #[serde(rename = "in work")]
    InWork,
    #[serde(rename = "confirmed")]
    Confirmed,
}

impl Rollup {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCard => "no card",
            Self::Departed => "departed",
            Self::Suspect => "suspect",
            Self::InWork => "in work",
            Self::Confirmed => "confirmed",
        }
    }
}

/// The state of a node from the marks of its cards over it, cancelled ones
/// left out by the caller: first match of `no card`, `departed`, `suspect`,
/// `in work`; else at least one accepted, none suspect or departed:
/// `confirmed`. A held mark is work under way.
pub fn rollup<'m>(marks: impl IntoIterator<Item = &'m CardMark>) -> Rollup {
    let marks: Vec<&CardMark> = marks.into_iter().collect();
    let any = |wanted: fn(&CardMark) -> bool| marks.iter().any(|mark| wanted(mark));
    if marks.is_empty() {
        Rollup::NoCard
    } else if any(|mark| matches!(mark, CardMark::Departed { .. })) {
        Rollup::Departed
    } else if any(|mark| matches!(mark, CardMark::Suspect { .. })) {
        Rollup::Suspect
    } else if any(|mark| matches!(mark, CardMark::Open | CardMark::Correcting { .. })) {
        Rollup::InWork
    } else {
        Rollup::Confirmed
    }
}

/// How many nodes stand in each state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RollupCounts {
    pub no_card: usize,
    pub in_work: usize,
    pub suspect: usize,
    pub departed: usize,
    pub confirmed: usize,
}

impl RollupCounts {
    pub const fn add(&mut self, state: Rollup) {
        match state {
            Rollup::NoCard => self.no_card += 1,
            Rollup::InWork => self.in_work += 1,
            Rollup::Suspect => self.suspect += 1,
            Rollup::Departed => self.departed += 1,
            Rollup::Confirmed => self.confirmed += 1,
        }
    }
}

/// What a read cannot compare of `card`, or finds gone, in `targets` order:
/// never a mark's silence.
pub fn mark_notes(card: &Card, norms: &TargetNorms) -> Vec<String> {
    let id = &card.id;
    if card.targets.is_empty() {
        return Vec::new();
    }
    if card.snapshot.is_none() {
        return vec![format!(
            "{id} froze no snapshot: it cannot be compared, so it reads not suspect"
        )];
    }
    card.targets
        .iter()
        .filter_map(|target| {
            let frozen = entry(card, target);
            let now = norms.get(target);
            match (frozen, now) {
                (None, _) => Some(format!(
                    "{id}: `{target}` has no entry in its snapshot, so it reads not suspect"
                )),
                (Some(frozen), None) if is_suspect(frozen, None) => {
                    Some(format!("{id}: `{target}` no longer resolves"))
                }
                (Some(frozen), now)
                    if compared(frozen).is_none()
                        || now.is_none_or(|now| compared_now(frozen, now).is_none()) =>
                {
                    Some(format!(
                        "{id}: `{target}` has a null hash: it cannot be compared, so it reads \
                         not suspect"
                    ))
                }
                (Some(frozen), Some(now))
                    if frozen.within_hash.is_none() && now.within_hash != now.norm_hash =>
                {
                    Some(format!(
                        "{id}: {target} froze its own text alone: a change in its sections marks \
                         nothing until an accept or a clear"
                    ))
                }
                _ => None,
            }
        })
        .collect()
}

/// Why `card` takes no clear: a cancelled card, one whose mark a correction
/// holds, or one that is not suspect. An accepted card takes one.
pub fn clear_refusal(card: &Card, suspect: bool, correction: Option<&str>) -> Option<String> {
    let id = &card.id;
    if card.cancellation.is_some() {
        return closed_refusal(card);
    }
    if !suspect {
        return Some(format!(
            "{id} is not suspect: there is no mark to clear; nothing changed"
        ));
    }
    correction.map(|correction| {
        format!(
            "{id} reads correcting {correction}: its mark is held until that card is accepted or \
             cancelled; nothing changed"
        )
    })
}

/// Why `name` cannot clear a mark: [`LOCAL_ACTOR`] and [`AGENT_ACTOR`] are
/// no one's name, and a clear is a person's act.
pub fn clearer_problem(name: &str) -> Option<String> {
    (name == LOCAL_ACTOR || name == AGENT_ACTOR)
        .then(|| format!("`{name}` clears no mark: a person answers one; nothing changed"))
}

/// Why `card` takes no correction: only an accepted one does.
pub fn correction_refusal(card: &Card) -> Option<String> {
    card.acceptance.is_none().then(|| {
        format!(
            "{} is not accepted: a correction answers an accepted card; nothing changed",
            card.id
        )
    })
}

/// The title a correction of `title` takes by default.
pub fn correction_title(title: &str) -> String {
    format!("{CORRECT} {title}")
}

/// What a departing accept says of the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Departure<'a> {
    /// The card accepted.
    pub card: &'a str,
    /// The direct targets it departs from.
    pub node_ids: &'a [String],
    pub summary: &'a str,
    pub observed: &'a str,
    pub documented: &'a str,
    /// `None`: `contradicts`.
    pub gap_type: Option<GapType>,
}

/// The discrepancy of `departure`: evidence named after the card, the two
/// ways to settle it, neither priced, the first recommended; `distinct_from`
/// the accept's own to write.
pub fn departure_input(departure: &Departure<'_>) -> DiscrepancyInput {
    let option = |label: &str, effect: &str| IntakeOption {
        label: label.to_owned(),
        effect: effect.to_owned(),
        price: NOT_PRICED.to_owned(),
    };
    DiscrepancyInput {
        node_ids: departure.node_ids.to_vec(),
        summary: departure.summary.to_owned(),
        gap_type: departure.gap_type.unwrap_or(GapType::Contradicts),
        severity: IntakeSeverity::Normal,
        evidence: vec![Evidence {
            file: departure.card.to_owned(),
            qpath: None,
            lines: None,
            observed: departure.observed.to_owned(),
            documented: departure.documented.to_owned(),
        }],
        options: vec![
            option(
                "Change the brief",
                "The brief is changed to say what was accepted.",
            ),
            option(
                "Rework the result",
                "The result is reworked to what the brief says.",
            ),
        ],
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from: None,
    }
}
