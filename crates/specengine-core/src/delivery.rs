//! What was delivered, what is overdue and why, per release and per
//! department: the pure half (task spec `delivery-analytics`,
//! `docs/features/delivery-analytics.md` "Acceptance criteria", DA-1 to
//! DA-9). The caller reads the queue's cards, their `card.moved`,
//! `card.accepted` and `card.reestimated` events and the items raised on
//! them once, the plan's groups and the served workspaces; [`delivery`]
//! totals them against one `as_of` day.
//!
//! Nothing is read, written or stored here, and no clock: `as_of` is an
//! input, judging open cards only, never rewinding the queue. Events are
//! ordered by `seq`, never by `at`. A card's estimate is shown as written
//! and never summed, averaged or compared; no key names a person (DA-6).
//! It moves, refuses and gates nothing.
//!
//! 1. **Release**: the card's targets the plan's groups hold, the least
//!    group order winning; none, its parent's, up the chain, a repeat or a
//!    parent not read ending it at no release (DA-3).
//! 2. **Departments**: every served workspace whose `flows` is `None` or
//!    holds the card's flow, in served order (DA-4).
//! 3. **Taken**: the card's first move or accept whose `from` differs from
//!    its `to`, a reorder never one (DA-1, DA-2); **returns** its moves from
//!    another column into its flow's return column (DA-8).
//! 4. **Counts**: `tasks` not cancelled, `delivered` accepted,
//!    `delivered_late` accepted on a day after `due`, `open` neither,
//!    `overdue` open with `due` before `as_of`, `cancelled`; lead days over
//!    the delivered ones, the median the lower middle (DA-9).
//! 5. **Rows**: a release row per group holding a card, in plan order, then
//!    no release; a department row per served workspace, zeros too; the
//!    total over every card once (DA-4).
//! 6. **Late**: every card delivered late or overdue, by days late
//!    descending, then ID number; each list of its reasons keeps its oldest
//!    [`REASONS_MAX`], the rest counted (DA-8).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_model::Card;

use crate::card::card_number;
use crate::check::is_calendar_date;
use crate::proposal::{is_utc_timestamp, proposal_number};
use crate::roadmap::{GroupKind, GroupRef};
use crate::workspace_toml::Workspace;

/// The most entries of one reason list of a late task, the oldest kept.
pub const REASONS_MAX: usize = 16;

/// The days since 1970-01-01 of a calendar date `YYYY-MM-DD`, or of a UTC
/// time stamp's date part; `None` for any other text. The inverse of
/// [`crate::check::date_from_unix_days`] (the days-from-civil algorithm).
pub fn unix_days(date: &str) -> Option<i64> {
    let day = if is_calendar_date(date) {
        date
    } else if is_utc_timestamp(date) {
        date.get(..10)?
    } else {
        return None;
    };
    let number = |range: std::ops::Range<usize>| day.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// What [`delivery`] totals: one read's facts.
#[derive(Debug, Clone, Copy)]
pub struct DeliveryInput<'a> {
    /// A calendar date, judged by the caller.
    pub as_of: &'a str,
    /// Every readable card row, in any state.
    pub cards: &'a [Card],
    /// `card.moved` and `card.accepted`, by `seq`.
    pub moves: &'a [Moved<'a>],
    /// `card.reestimated`, by `seq`.
    pub reestimates: &'a [Reestimated<'a>],
    /// The question, discrepancy and decision rows naming a card.
    pub items: &'a [CardItemRef<'a>],
    /// Every plan item by ID, with its group ([`crate::roadmap::item_groups`]).
    pub groups: &'a BTreeMap<String, GroupRef>,
    /// The served workspaces, in served order.
    pub departments: &'a [Workspace],
    /// Each flow's return column.
    pub returns: &'a BTreeMap<String, String>,
}

/// A `card.moved` or `card.accepted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved<'a> {
    pub seq: i64,
    pub card: &'a str,
    pub from: &'a str,
    pub to: &'a str,
    /// A move's note; an accept has none.
    pub note: Option<&'a str>,
    pub at: &'a str,
}

/// A `card.reestimated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reestimated<'a> {
    pub seq: i64,
    pub card: &'a str,
    pub old: Option<&'a str>,
    /// `None` on a clear.
    pub new: Option<&'a str>,
    pub reason: &'a str,
    pub at: &'a str,
}

/// A question, discrepancy or decision raised on a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardItemRef<'a> {
    pub id: &'a str,
    pub card: &'a str,
    pub kind: &'a str,
    pub status: &'a str,
}

/// The group a row or a late task counts in: a plan group's, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum DeliveryGroup {
    #[serde(rename = "release")]
    Release,
    #[serde(rename = "pool")]
    Pool,
    #[serde(rename = "unplaced")]
    Unplaced,
    /// No plan item up the card's chain.
    #[serde(rename = "none")]
    NoRelease,
}

impl DeliveryGroup {
    /// `release`, `pool`, `unplaced`, `none`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Pool => "pool",
            Self::Unplaced => "unplaced",
            Self::NoRelease => "none",
        }
    }

    fn of(kind: GroupKind) -> Self {
        match kind {
            GroupKind::Release => Self::Release,
            GroupKind::Pool => Self::Pool,
            GroupKind::Unplaced => Self::Unplaced,
        }
    }
}

/// The lead days of a row's delivered cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct LeadDays {
    /// The lower of the two middles.
    pub median: i64,
    pub max: i64,
}

/// A row's counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryCounts {
    /// Not cancelled.
    pub tasks: usize,
    pub delivered: usize,
    pub delivered_late: usize,
    pub open: usize,
    pub overdue: usize,
    pub cancelled: usize,
    /// `None` with none delivered.
    pub lead_days: Option<LeadDays>,
}

/// One release row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryRelease {
    pub group: DeliveryGroup,
    /// The release's name, for [`DeliveryGroup::Release`] alone.
    pub release: Option<String>,
    #[serde(flatten)]
    pub counts: DeliveryCounts,
}

/// One department row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryDepartment {
    pub name: String,
    pub title: String,
    #[serde(flatten)]
    pub counts: DeliveryCounts,
}

/// A return of a late task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryReturn {
    pub at: String,
    pub from: String,
    pub note: Option<String>,
}

/// A re-estimate of a late task, as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryReestimate {
    pub at: String,
    pub old: Option<String>,
    /// `None` on a clear.
    pub new: Option<String>,
    pub reason: String,
}

/// A cancelled subtask of a late task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryCancelled {
    pub id: String,
    pub title: String,
    pub at: String,
    pub reason: String,
}

/// An item raised on a late task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryItem {
    pub id: String,
    pub kind: String,
    pub status: String,
}

/// How many entries of each reason list are past [`REASONS_MAX`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryMore {
    pub returns: usize,
    pub reestimates: usize,
    pub cancelled_subtasks: usize,
    pub items: usize,
}

/// A task delivered late or overdue, with its reasons.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeliveryLate {
    pub id: String,
    pub title: String,
    pub flow: String,
    pub column: String,
    pub parent: Option<String>,
    pub group: DeliveryGroup,
    pub release: Option<String>,
    /// The departments' names, in served order.
    pub departments: Vec<String>,
    pub created_at: String,
    pub taken_at: Option<String>,
    pub accepted_at: Option<String>,
    pub due: String,
    pub days_late: i64,
    pub lead_days: Option<i64>,
    /// As written.
    pub estimate: Option<String>,
    pub returns: Vec<DeliveryReturn>,
    pub reestimates: Vec<DeliveryReestimate>,
    pub cancelled_subtasks: Vec<DeliveryCancelled>,
    pub items: Vec<DeliveryItem>,
    pub more: DeliveryMore,
}

/// What [`delivery`] found.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Delivery {
    pub as_of: String,
    /// Sorted, each once.
    pub notes: Vec<String>,
    pub releases: Vec<DeliveryRelease>,
    pub departments: Vec<DeliveryDepartment>,
    pub total: DeliveryCounts,
    pub late: Vec<DeliveryLate>,
}

/// Where a card stands at `as_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Cancelled,
    /// Accepted; `late` its days past `due`, when after it.
    Delivered {
        late: Option<i64>,
    },
    /// Neither; `overdue` its days past `due`, when before `as_of`.
    Open {
        overdue: Option<i64>,
    },
}

/// One card as judged.
struct Judged<'c> {
    card: &'c Card,
    group: Option<&'c GroupRef>,
    standing: Standing,
    /// Accepted, both days read.
    lead: Option<i64>,
}

impl Judged<'_> {
    /// Days late, delivered late or overdue.
    fn days_late(&self) -> Option<i64> {
        match self.standing {
            Standing::Delivered { late } => late,
            Standing::Open { overdue } => overdue,
            Standing::Cancelled => None,
        }
    }
}

/// A row's running counts.
#[derive(Default)]
struct Tally {
    counts: DeliveryCounts,
    leads: Vec<i64>,
}

impl Tally {
    fn add(&mut self, judged: &Judged<'_>) {
        let counts = &mut self.counts;
        match judged.standing {
            Standing::Cancelled => counts.cancelled += 1,
            Standing::Delivered { late } => {
                counts.tasks += 1;
                counts.delivered += 1;
                if late.is_some() {
                    counts.delivered_late += 1;
                }
                if let Some(lead) = judged.lead {
                    self.leads.push(lead);
                }
            }
            Standing::Open { overdue } => {
                counts.tasks += 1;
                counts.open += 1;
                if overdue.is_some() {
                    counts.overdue += 1;
                }
            }
        }
    }

    fn counts(mut self) -> DeliveryCounts {
        self.leads.sort_unstable();
        let mut counts = self.counts;
        counts.lead_days = self.leads.last().map(|&max| LeadDays {
            median: self.leads[(self.leads.len() - 1) / 2],
            max,
        });
        counts
    }
}

/// The facts of one read, grouped by card once.
/// The cards of one read by ID, for the release walk up the chain.
struct ByCard<'i> {
    cards: BTreeMap<&'i str, &'i Card>,
}

impl<'i> ByCard<'i> {
    fn new(input: &DeliveryInput<'i>) -> Self {
        Self {
            cards: input
                .cards
                .iter()
                .map(|card| (card.id.as_str(), card))
                .collect(),
        }
    }

    /// The group of `card`'s first plan item, else its parent's, up the
    /// chain; `None` past a repeat or a parent not read.
    fn group_of(
        &self,
        card: &'i Card,
        groups: &'i BTreeMap<String, GroupRef>,
    ) -> Option<&'i GroupRef> {
        let mut seen = BTreeSet::new();
        let mut at = card;
        loop {
            if !seen.insert(at.id.as_str()) {
                return None;
            }
            let first = at
                .targets
                .iter()
                .filter_map(|target| groups.get(target))
                .min_by_key(|group| group.order);
            if first.is_some() {
                return first;
            }
            at = self.cards.get(at.parent.as_deref()?)?;
        }
    }
}

/// The reasons of the late cards alone, grouped once after the judge, so
/// a read of many on-time cards groups none of their events.
struct Reasons<'i> {
    /// Each late card's moves and accepts, by `seq`.
    moves: BTreeMap<&'i str, Vec<&'i Moved<'i>>>,
    /// Each late card's re-estimates, by `seq`.
    reestimates: BTreeMap<&'i str, Vec<&'i Reestimated<'i>>>,
    /// Each late card's items, by ID number.
    items: BTreeMap<&'i str, Vec<&'i CardItemRef<'i>>>,
    /// Each late card's cancelled subtasks, by ID number.
    cancelled: BTreeMap<&'i str, Vec<&'i Card>>,
}

impl<'i> Reasons<'i> {
    fn new(input: &DeliveryInput<'i>, late: &BTreeSet<&str>) -> Self {
        let mut moves: BTreeMap<&str, Vec<&Moved<'_>>> = BTreeMap::new();
        for moved in input.moves.iter().filter(|moved| late.contains(moved.card)) {
            moves.entry(moved.card).or_default().push(moved);
        }
        for list in moves.values_mut() {
            list.sort_by_key(|moved| moved.seq);
        }
        let mut reestimates: BTreeMap<&str, Vec<&Reestimated<'_>>> = BTreeMap::new();
        for reestimated in input
            .reestimates
            .iter()
            .filter(|reestimated| late.contains(reestimated.card))
        {
            reestimates
                .entry(reestimated.card)
                .or_default()
                .push(reestimated);
        }
        for list in reestimates.values_mut() {
            list.sort_by_key(|reestimated| reestimated.seq);
        }
        let mut items: BTreeMap<&str, Vec<&CardItemRef<'_>>> = BTreeMap::new();
        for item in input.items.iter().filter(|item| late.contains(item.card)) {
            items.entry(item.card).or_default().push(item);
        }
        for list in items.values_mut() {
            list.sort_by_key(|item| (proposal_number(item.id).unwrap_or(u64::MAX), item.id));
        }
        let mut cancelled: BTreeMap<&str, Vec<&Card>> = BTreeMap::new();
        for card in input
            .cards
            .iter()
            .filter(|card| card.cancellation.is_some())
        {
            if let Some(parent) = card
                .parent
                .as_deref()
                .filter(|parent| late.contains(parent))
            {
                cancelled.entry(parent).or_default().push(card);
            }
        }
        for list in cancelled.values_mut() {
            list.sort_by_key(|card| id_key(&card.id));
        }
        Self {
            moves,
            reestimates,
            items,
            cancelled,
        }
    }
}

/// A card ID's order: its number, then the ID (one written otherwise last).
fn id_key(id: &str) -> (u64, &str) {
    (card_number(id).unwrap_or(u64::MAX), id)
}

/// The note of a card's time stamp that does not read.
fn no_timestamp(card: &Card, value: &str) -> String {
    format!("{}: {value} is no timestamp; no lead time", card.id)
}

/// Where `card` stands at `as_of` (days `as_of_days`), and its lead days;
/// a time stamp of a delivered card that does not read is noted.
fn standing(
    card: &Card,
    as_of_days: Option<i64>,
    notes: &mut BTreeSet<String>,
) -> (Standing, Option<i64>) {
    if card.cancellation.is_some() {
        return (Standing::Cancelled, None);
    }
    let due = card.due.as_deref().and_then(unix_days);
    let Some(acceptance) = &card.acceptance else {
        let overdue = match (due, as_of_days) {
            (Some(due), Some(as_of)) if due < as_of => Some(as_of - due),
            _ => None,
        };
        return (Standing::Open { overdue }, None);
    };
    let accepted = unix_days(&acceptance.at);
    if accepted.is_none() {
        notes.insert(no_timestamp(card, &acceptance.at));
    }
    let created = unix_days(&card.created_at);
    if created.is_none() {
        notes.insert(no_timestamp(card, &card.created_at));
    }
    let late = match (accepted, due) {
        (Some(accepted), Some(due)) if accepted > due => Some(accepted - due),
        _ => None,
    };
    let lead = accepted
        .zip(created)
        .map(|(accepted, created)| accepted - created);
    (Standing::Delivered { late }, lead)
}

/// The first `REASONS_MAX` of `list`, and how many are past them.
fn kept<T>(mut list: Vec<T>) -> (Vec<T>, usize) {
    let more = list.len().saturating_sub(REASONS_MAX);
    list.truncate(REASONS_MAX);
    (list, more)
}

/// The group and release name a card counts under.
fn placed(group: Option<&GroupRef>) -> (DeliveryGroup, Option<String>) {
    match group {
        Some(group) => {
            let kind = DeliveryGroup::of(group.kind);
            let release = match kind {
                DeliveryGroup::Release => group.name.clone(),
                _ => None,
            };
            (kind, release)
        }
        None => (DeliveryGroup::NoRelease, None),
    }
}

/// The delivery of `input` (see the module documentation).
pub fn delivery(input: &DeliveryInput<'_>) -> Delivery {
    let by = ByCard::new(input);
    let as_of_days = unix_days(input.as_of);
    let mut notes = BTreeSet::new();
    let judged: Vec<Judged<'_>> = input
        .cards
        .iter()
        .map(|card| {
            let (standing, lead) = standing(card, as_of_days, &mut notes);
            Judged {
                card,
                group: by.group_of(card, input.groups),
                standing,
                lead,
            }
        })
        .collect();
    let in_department = |department: &Workspace, card: &Card| {
        department
            .flows
            .as_ref()
            .is_none_or(|flows| flows.contains(&card.flow))
    };

    let mut groups: BTreeMap<usize, (&GroupRef, Tally)> = BTreeMap::new();
    let mut none: Option<Tally> = None;
    let mut departments: Vec<Tally> = input.departments.iter().map(|_| Tally::default()).collect();
    let mut total = Tally::default();
    for card in &judged {
        match card.group {
            Some(group) => groups
                .entry(group.order)
                .or_insert_with(|| (group, Tally::default()))
                .1
                .add(card),
            None => none.get_or_insert_with(Tally::default).add(card),
        }
        for (department, tally) in input.departments.iter().zip(&mut departments) {
            if in_department(department, card.card) {
                tally.add(card);
            }
        }
        total.add(card);
    }

    let mut releases: Vec<DeliveryRelease> = groups
        .into_values()
        .map(|(group, tally)| {
            let (group, release) = placed(Some(group));
            DeliveryRelease {
                group,
                release,
                counts: tally.counts(),
            }
        })
        .collect();
    if let Some(tally) = none {
        releases.push(DeliveryRelease {
            group: DeliveryGroup::NoRelease,
            release: None,
            counts: tally.counts(),
        });
    }
    let departments = input
        .departments
        .iter()
        .zip(departments)
        .map(|(department, tally)| DeliveryDepartment {
            name: department.name.clone(),
            title: department.title.clone(),
            counts: tally.counts(),
        })
        .collect();

    let late_ids: BTreeSet<&str> = judged
        .iter()
        .filter(|card| card.days_late().is_some())
        .map(|card| card.card.id.as_str())
        .collect();
    let reasons = Reasons::new(input, &late_ids);
    let mut late: Vec<DeliveryLate> = judged
        .iter()
        .filter_map(|card| {
            let days_late = card.days_late()?;
            Some(late_row(input, &reasons, card, days_late, &in_department))
        })
        .collect();
    late.sort_by(|a, b| {
        b.days_late
            .cmp(&a.days_late)
            .then_with(|| id_key(&a.id).cmp(&id_key(&b.id)))
    });

    Delivery {
        as_of: input.as_of.to_owned(),
        notes: notes.into_iter().collect(),
        releases,
        departments,
        total: total.counts(),
        late,
    }
}

/// The late row of `judged`, `days_late` past its `due`, with its reasons.
fn late_row(
    input: &DeliveryInput<'_>,
    by: &Reasons<'_>,
    judged: &Judged<'_>,
    days_late: i64,
    in_department: &dyn Fn(&Workspace, &Card) -> bool,
) -> DeliveryLate {
    let card = judged.card;
    let id = card.id.as_str();
    let moves = by.moves.get(id).map_or(&[][..], Vec::as_slice);
    let taken_at = moves
        .iter()
        .find(|moved| moved.from != moved.to)
        .map(|moved| moved.at.to_owned());
    let return_column = input.returns.get(&card.flow);
    let (returns, more_returns) = kept(
        moves
            .iter()
            .filter(|moved| {
                moved.from != moved.to && return_column.is_some_and(|column| column == moved.to)
            })
            .map(|moved| DeliveryReturn {
                at: moved.at.to_owned(),
                from: moved.from.to_owned(),
                note: moved.note.map(str::to_owned),
            })
            .collect(),
    );
    let (reestimates, more_reestimates) = kept(
        by.reestimates
            .get(id)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|reestimated| DeliveryReestimate {
                at: reestimated.at.to_owned(),
                old: reestimated.old.map(str::to_owned),
                new: reestimated.new.map(str::to_owned),
                reason: reestimated.reason.to_owned(),
            })
            .collect(),
    );
    let (cancelled_subtasks, more_cancelled) = kept(
        by.cancelled
            .get(id)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|child| {
                let cancellation = child.cancellation.as_ref()?;
                Some(DeliveryCancelled {
                    id: child.id.clone(),
                    title: child.title.clone(),
                    at: cancellation.at.clone(),
                    reason: cancellation.reason.clone(),
                })
            })
            .collect(),
    );
    let (items, more_items) = kept(
        by.items
            .get(id)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|item| DeliveryItem {
                id: item.id.to_owned(),
                kind: item.kind.to_owned(),
                status: item.status.to_owned(),
            })
            .collect(),
    );
    let (group, release) = placed(judged.group);
    DeliveryLate {
        id: card.id.clone(),
        title: card.title.clone(),
        flow: card.flow.clone(),
        column: card.column.clone(),
        parent: card.parent.clone(),
        group,
        release,
        departments: input
            .departments
            .iter()
            .filter(|department| in_department(department, card))
            .map(|department| department.name.clone())
            .collect(),
        created_at: card.created_at.clone(),
        taken_at,
        accepted_at: card
            .acceptance
            .as_ref()
            .map(|acceptance| acceptance.at.clone()),
        due: card.due.clone().unwrap_or_default(),
        days_late,
        lead_days: judged.lead,
        estimate: card.estimate.clone(),
        returns,
        reestimates,
        cancelled_subtasks,
        items,
        more: DeliveryMore {
            returns: more_returns,
            reestimates: more_reestimates,
            cancelled_subtasks: more_cancelled,
            items: more_items,
        },
    }
}
