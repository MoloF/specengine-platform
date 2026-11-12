//! `spec delivery [--as-of D] [--json]` (task spec `delivery-analytics`):
//! what was delivered, what is overdue and why, per release and per
//! department, computed per read from the queue, the config and the plan
//! (core's [`specengine_core::delivery::delivery`]), never stored. It moves,
//! refuses and gates nothing.
//!
//! One call reads the config's `[roadmap]`, `[card]` and `[[workspace]]`,
//! each apart (a table that does not read is a note), the plan's groups
//! from the walked files (no index), then the queue once, as it is
//! ([`SqliteQueue::open_existing`]: no schema step, nothing written): one
//! card list, one uncapped `events_of_types` read, never a card's capped
//! history, and one item list (DA-1, DA-10). No git, no index, no write.
//! A queue with no card reads nothing more.
//!
//! `--as-of D` is the day the open cards are judged at, a calendar date
//! (else exit 2, nothing read); without it, today's UTC date. Given, no
//! clock is read: one queue, config and file state with one `as_of` gives
//! one text and one JSON (DA-5).
//!
//! ```text
//! delivery | as of <D> | <counts> | <lead>
//! <release R | pool | unplaced | no release> | <counts> | <lead>
//! department <name> | <title> | <counts> | <lead>
//! late <C> | <flow>/<column> | <R, pool, unplaced or -> | due <D> | <open or accepted <at>> | <overdue or late> <n>d | <title>
//!   returned <at> | from <column> | <note or ->
//!   re-estimated <at> | <old or -> -> <new or -> | <reason>
//!   subtask <C> cancelled <at> | <reason>
//!   item <id> | <kind> | <status>
//!   more returns <a>, re-estimates <b>, cancelled subtasks <c>, items <d>
//! [truncated: <k> of <n> late tasks not shown]
//! ```
//!
//! `<counts>`: `tasks <t>, delivered <d>, late <l>, open <o>, overdue <v>,
//! cancelled <c>`, `late` the delivered late; `<lead>`: `lead median <m>d,
//! max <x>d`, or `lead -` with none delivered. The `more` line names only
//! the lists past their cap. Every value is on one line, every line's
//! control characters escaped; the JSON holds the values raw. The header,
//! group and department lines print whole, then the late rows, each whole,
//! while everything before the tail is at most [`OUTPUT_CAP_CHARS`]; the
//! JSON holds the rows the text prints ([`delivery_with_view`] with the
//! daemon's [`View::Browser`]: nothing cut, `truncated` false, `omitted`
//! `null`). The notes go to stderr as `note:` lines and into the JSON's
//! `notes`, sorted, each once; none changes the exit. Exit 2: a bad
//! `--as-of`, no project, a config the discovery refuses, or a queue that
//! cannot be read (a newer build's).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;
use specengine_core::check::{CheckInput, ProblemKind, front_matter_failure, is_calendar_date};
use specengine_core::delivery::{
    CardItemRef, Delivery, DeliveryCounts, DeliveryDepartment, DeliveryGroup, DeliveryInput,
    DeliveryLate, DeliveryRelease, Moved, Reestimated,
};
use specengine_core::{
    GroupRef, Paths, RoadmapConfig, WorkspaceConfig, card_from_toml, is_under, item_groups,
};
use specengine_model::Card;
use specengine_store::{
    EVENT_CARD_ACCEPTED, EVENT_CARD_MOVED, EVENT_CARD_REESTIMATED, Event, Proposal, ProposalFilter,
    ProposalKind, ProposalQueue as _, SqliteQueue, WorkingTree, check_input, today_utc,
};

use crate::cap::{OUTPUT_CAP_CHARS, View};
use crate::location::db_path;
use crate::project::{ProjectRoot, discover_with_text};
use crate::proposals::queue_cannot;
use crate::subset::seams::{self, Event as Seam};
use crate::{CliError, Env, Globals, escape_controls, one_line, store_error};

/// The note of a config with no `[roadmap]`.
const NO_ROADMAP: &str = "delivery: no [roadmap] table: every task is in no release";

/// The note of a `[card]` that does not read.
const CARD_UNREAD: &str = "delivery: [card] does not read: no return is counted";

/// The note of a project with no database yet.
const NO_QUEUE: &str = "delivery: no queue yet: no card read";

/// The end of a plan gap's note on one file.
const ITEM_PLACES_NONE: &str = "its item places no task";

/// The end of a plan gap's note on a directory or a root.
const ITEMS_PLACE_NONE: &str = "its items place no task";

/// What a text line shows for no value.
const ABSENT: &str = "-";

/// `spec delivery` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveryRequest {
    /// `--as-of D`; `None`: today's UTC date.
    pub as_of: Option<String>,
}

/// What `spec delivery` computed and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryOutcome {
    /// Core's answer; its `notes` the config's and the queue's too, sorted,
    /// each once.
    pub delivery: Delivery,
    /// How many late rows print, the first ones.
    pub shown: usize,
    /// A late row does not print: the tail, and the JSON's `truncated`.
    pub truncated: bool,
}

impl DeliveryOutcome {
    /// The late rows that do not print (`k` of the tail).
    pub fn omitted(&self) -> usize {
        self.delivery.late.len().saturating_sub(self.shown)
    }

    /// The late rows that print.
    fn shown_late(&self) -> &[DeliveryLate] {
        &self.delivery.late[..self.shown.min(self.delivery.late.len())]
    }

    /// One `note:` line per note, in order.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        self.delivery
            .notes
            .iter()
            .map(|note| escape_controls(&format!("note: {}", one_line(note))))
            .collect()
    }
}

/// The JSON document: every key present, in this order.
#[derive(Serialize)]
struct DeliveryJson<'a> {
    as_of: &'a str,
    notes: &'a [String],
    releases: &'a [DeliveryRelease],
    departments: &'a [DeliveryDepartment],
    total: &'a DeliveryCounts,
    late: &'a [DeliveryLate],
    truncated: bool,
    /// The late rows cut; `null` uncut.
    omitted: Option<usize>,
}

impl Serialize for DeliveryOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let delivery = &self.delivery;
        DeliveryJson {
            as_of: &delivery.as_of,
            notes: &delivery.notes,
            releases: &delivery.releases,
            departments: &delivery.departments,
            total: &delivery.total,
            late: self.shown_late(),
            truncated: self.truncated,
            omitted: self.truncated.then(|| self.omitted()),
        }
        .serialize(serializer)
    }
}

/// `--as-of D` must be a calendar date `YYYY-MM-DD` (exit 2 before
/// anything is read). The daemon judges its `as_of` by it before anything
/// runs.
pub fn delivery_as_of(as_of: &str) -> Result<(), CliError> {
    if is_calendar_date(as_of) {
        return Ok(());
    }
    Err(CliError::spec(format!(
        "`--as-of`: `{}` is no calendar date, YYYY-MM-DD",
        escape_controls(&one_line(as_of))
    )))
}

/// `spec delivery`: the config, the plan's groups and the queue, read once,
/// totalled at `as_of`.
pub fn delivery(
    env: &Env,
    globals: &Globals,
    request: &DeliveryRequest,
) -> Result<DeliveryOutcome, CliError> {
    delivery_with_view(env, globals, request, View::Capped)
}

/// [`delivery`], its late rows bounded by `view` (the daemon's
/// [`View::Browser`]: every one shown, `truncated` false, `omitted` `null`).
pub fn delivery_with_view(
    env: &Env,
    globals: &Globals,
    request: &DeliveryRequest,
    view: View,
) -> Result<DeliveryOutcome, CliError> {
    if let Some(as_of) = &request.as_of {
        delivery_as_of(as_of)?;
    }
    let (project, text) = discover_with_text(env, globals)?;
    let slug = project.slug()?.to_owned();
    let mut notes = BTreeSet::new();
    let groups = plan_groups(&project, &text, &mut notes)?;
    let returns = return_columns(&text, &mut notes);
    let workspaces = WorkspaceConfig::from_toml(&text);
    // As `spec workspace` prints them.
    notes.extend(
        workspaces
            .notes_at(&project.config_label)
            .iter()
            .map(|note| escape_controls(&one_line(note))),
    );
    let facts = QueueFacts::read(env, &slug, &mut notes)?;
    let as_of = request.as_of.clone().unwrap_or_else(today_utc);

    let moves: Vec<Moved<'_>> = facts.events.iter().filter_map(moved).collect();
    let reestimates: Vec<Reestimated<'_>> = facts.events.iter().filter_map(reestimated).collect();
    let items: Vec<CardItemRef<'_>> = facts.items.iter().filter_map(item_ref).collect();
    let mut delivery = specengine_core::delivery::delivery(&DeliveryInput {
        as_of: &as_of,
        cards: &facts.cards,
        moves: &moves,
        reestimates: &reestimates,
        items: &items,
        groups: &groups,
        departments: &workspaces.workspaces,
        returns: &returns,
    });
    notes.extend(delivery.notes.iter().map(|note| one_line(note)));
    delivery.notes = notes.into_iter().collect();

    let shown = match view {
        View::Capped => fitting(&delivery),
        View::Browser => delivery.late.len(),
    };
    Ok(DeliveryOutcome {
        truncated: shown < delivery.late.len(),
        delivery,
        shown,
    })
}

/// Every plan item's group, from the walked files as `spec roadmap` plans
/// them (core's [`item_groups`]), and a note per place of the walk it did
/// not read ([`plan_gaps`]); none, with a note, without a `[roadmap]` or
/// with one that does not read.
fn plan_groups(
    project: &ProjectRoot,
    text: &str,
    notes: &mut BTreeSet<String>,
) -> Result<BTreeMap<String, GroupRef>, CliError> {
    let config = match RoadmapConfig::from_toml(text, &project.config.scheme) {
        Ok(Some(config)) => config,
        Ok(None) => {
            notes.insert(NO_ROADMAP.to_owned());
            return Ok(BTreeMap::new());
        }
        Err(error) => {
            notes.insert(one_line(&format!(
                "delivery: [roadmap] does not read: {}; every task is in no release",
                error.at(&project.config_label)
            )));
            return Ok(BTreeMap::new());
        }
    };
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    seams::note(Seam::DeliveryPlan);
    let input = check_input(&tree, &project.config.scheme);
    notes.extend(plan_gaps(&input, &config, &project.config.paths));
    Ok(item_groups(&input, &config))
}

/// A note per place of the plan's directory, `[roadmap] dir`, its walk did
/// not read, so a card whose item stands there, counted by its parent or in
/// no release, says why (task spec `delivery-analytics` DA-3): `delivery:
/// plan <path> does not read (<reason>); its item places no task` for a
/// file under it not read, not parsed or whose front matter does not read;
/// `...; its items place no task` for a place that holds it or lies in it
/// (the directory itself, one under it, or one above it, `.` the root):
/// not listed, a written `[paths] roots` entry missing, or names skipped for
/// not being UTF-8. A file or place elsewhere is no plan's: `spec check`
/// reports it, never this read.
fn plan_gaps(input: &CheckInput, config: &RoadmapConfig, paths: &Paths) -> Vec<String> {
    let dir = config.dir.as_str();
    let covers =
        |path: &str| path.is_empty() || path == dir || is_under(path, dir) || is_under(dir, path);
    let gap = |path: &str, reason: &str, end: &str| {
        let path = if path.is_empty() { "." } else { path };
        one_line(&format!(
            "delivery: plan {path} does not read ({reason}); {end}"
        ))
    };
    let mut gaps = Vec::new();
    for file in input.files.iter().filter(|file| is_under(&file.path, dir)) {
        let reason = match (&file.read_error, &file.parsed) {
            (Some(error), _) => error.clone(),
            (None, None) => "not parsed".to_owned(),
            (None, Some(parsed)) => match front_matter_failure(parsed) {
                Some(failure) => format!("line {}: {}", failure.line, failure.message),
                None => continue,
            },
        };
        gaps.push(gap(&file.path, &reason, ITEM_PLACES_NONE));
    }
    for problem in input
        .problems
        .iter()
        .filter(|problem| covers(&problem.path))
    {
        let reason = match problem.kind {
            ProblemKind::UnreadableDir => "the directory cannot be listed",
            ProblemKind::MissingRoot if paths.roots_written => {
                "the [paths] roots entry names no directory and no .md file"
            }
            ProblemKind::MissingRoot => continue,
            ProblemKind::SkippedName => "a name in it is not UTF-8",
        };
        gaps.push(gap(&problem.path, reason, ITEMS_PLACE_NONE));
    }
    gaps
}

/// Each flow's return column; none, with a note, when `[card]` does not
/// read.
fn return_columns(text: &str, notes: &mut BTreeSet<String>) -> BTreeMap<String, String> {
    match card_from_toml(text) {
        Ok(card) => card
            .flows
            .into_iter()
            .filter_map(|flow| Some((flow.name, flow.return_column?)))
            .collect(),
        Err(_) => {
            notes.insert(CARD_UNREAD.to_owned());
            BTreeMap::new()
        }
    }
}

/// The queue's facts, read once per call.
#[derive(Default)]
struct QueueFacts {
    cards: Vec<Card>,
    /// `card.moved`, `card.accepted` and `card.reestimated`, by `seq`.
    events: Vec<Event>,
    /// The readable items naming a card, of the kinds that ask or decide.
    items: Vec<Proposal>,
}

impl QueueFacts {
    /// The queue as it is: no database, a note and nothing; a card row
    /// that does not decode, a note each and not counted; no card, nothing
    /// more read. Else one uncapped `events_of_types` read and one item list,
    /// each item row that does not decode a note, not listed.
    fn read(env: &Env, slug: &str, notes: &mut BTreeSet<String>) -> Result<Self, CliError> {
        let db = db_path(env, slug)?;
        let Some(queue) = SqliteQueue::open_existing(&db, slug).map_err(queue_cannot)? else {
            notes.insert(NO_QUEUE.to_owned());
            return Ok(Self::default());
        };
        seams::note(Seam::DeliveryCards);
        let listed = queue.list_existing_cards().map_err(queue_cannot)?;
        notes.extend(listed.unreadable.iter().map(|row| {
            one_line(&format!(
                "{}: its row does not read ({}: {}); not counted",
                row.id, row.column, row.reason
            ))
        }));
        if listed.cards.is_empty() {
            return Ok(Self::default());
        }
        seams::note(Seam::DeliveryEvents);
        let events = queue
            .events_of_types(&[
                EVENT_CARD_MOVED,
                EVENT_CARD_ACCEPTED,
                EVENT_CARD_REESTIMATED,
            ])
            .map_err(queue_cannot)?;
        seams::note(Seam::DeliveryItems);
        let listed_items = queue
            .list_readable(&ProposalFilter {
                git_common_dir: None,
                statuses: Vec::new(),
            })
            .map_err(queue_cannot)?;
        // A row that does not decode may be an item of a late card: it is
        // named, never left out silently.
        notes.extend(listed_items.unreadable.iter().map(|row| {
            one_line(&format!(
                "{}: its row does not read ({}: {}); not listed",
                row.id, row.column, row.reason
            ))
        }));
        let items = listed_items
            .proposals
            .into_iter()
            .filter(|item| {
                item.card_id.is_some()
                    && matches!(
                        item.kind,
                        ProposalKind::Question | ProposalKind::Discrepancy | ProposalKind::Decision
                    )
            })
            .collect();
        Ok(Self {
            cards: listed.cards,
            events,
            items,
        })
    }
}

/// A payload's text under `key`.
fn text<'e>(event: &'e Event, key: &str) -> Option<&'e str> {
    event.payload.get(key).and_then(Value::as_str)
}

/// A payload's nullable text under `key`: `Some(None)` for `null`; `None`
/// when the key is missing or holds another type.
fn nullable<'e>(event: &'e Event, key: &str) -> Option<Option<&'e str>> {
    match event.payload.get(key)? {
        Value::Null => Some(None),
        Value::String(value) => Some(Some(value)),
        _ => None,
    }
}

/// A `card.moved` or `card.accepted` as the judge reads it; `None` for
/// another type or a payload missing `id`, `from` or `to`. A note that is
/// no text (an accept's, which holds none) reads as none.
fn moved(event: &Event) -> Option<Moved<'_>> {
    if event.event_type != EVENT_CARD_MOVED && event.event_type != EVENT_CARD_ACCEPTED {
        return None;
    }
    let note = text(event, "note");
    Some(Moved {
        seq: event.seq,
        card: text(event, "id")?,
        from: text(event, "from")?,
        to: text(event, "to")?,
        note,
        at: &event.at,
    })
}

/// A `card.reestimated` as the judge reads it; `None` for another type or
/// a payload missing a field.
fn reestimated(event: &Event) -> Option<Reestimated<'_>> {
    if event.event_type != EVENT_CARD_REESTIMATED {
        return None;
    }
    Some(Reestimated {
        seq: event.seq,
        card: text(event, "id")?,
        old: nullable(event, "old")?,
        new: nullable(event, "new")?,
        reason: text(event, "reason")?,
        at: &event.at,
    })
}

/// An item as the judge reads it: the card it names, its kind and status.
fn item_ref(item: &Proposal) -> Option<CardItemRef<'_>> {
    Some(CardItemRef {
        id: &item.id,
        card: item.card_id.as_deref()?,
        kind: item.kind.as_str(),
        status: item.status.as_str(),
    })
}

/// How many late rows print within [`OUTPUT_CAP_CHARS`] after the header,
/// group and department lines, each row whole.
fn fitting(delivery: &Delivery) -> usize {
    let cost =
        |lines: &[String]| -> usize { lines.iter().map(|line| line.chars().count() + 1).sum() };
    let mut used = cost(&head_lines(delivery));
    let mut shown = 0;
    for late in &delivery.late {
        let row = cost(&late_lines(late));
        if used + row > OUTPUT_CAP_CHARS {
            break;
        }
        used += row;
        shown += 1;
    }
    shown
}

/// `tasks <t>, delivered <d>, late <l>, open <o>, overdue <v>, cancelled
/// <c> | <lead>`.
fn counts_text(counts: &DeliveryCounts) -> String {
    let lead = counts.lead_days.map_or_else(
        || format!("lead {ABSENT}"),
        |lead| format!("lead median {}d, max {}d", lead.median, lead.max),
    );
    format!(
        "tasks {}, delivered {}, late {}, open {}, overdue {}, cancelled {} | {lead}",
        counts.tasks,
        counts.delivered,
        counts.delivered_late,
        counts.open,
        counts.overdue,
        counts.cancelled
    )
}

/// The header, then a line per release row and per department, escaped.
fn head_lines(delivery: &Delivery) -> Vec<String> {
    let mut lines = vec![format!(
        "delivery | as of {} | {}",
        one_line(&delivery.as_of),
        counts_text(&delivery.total)
    )];
    for release in &delivery.releases {
        let group = match release.group {
            DeliveryGroup::Release => format!(
                "release {}",
                release
                    .release
                    .as_deref()
                    .map_or(ABSENT.to_owned(), one_line)
            ),
            DeliveryGroup::NoRelease => "no release".to_owned(),
            DeliveryGroup::Pool | DeliveryGroup::Unplaced => release.group.as_str().to_owned(),
        };
        lines.push(format!("{group} | {}", counts_text(&release.counts)));
    }
    for department in &delivery.departments {
        lines.push(format!(
            "department {} | {} | {}",
            one_line(&department.name),
            one_line(&department.title),
            counts_text(&department.counts)
        ));
    }
    lines.iter().map(|line| escape_controls(line)).collect()
}

/// A late row's lines: its own, then each reason, then `more`, escaped.
fn late_lines(late: &DeliveryLate) -> Vec<String> {
    let held = |text: Option<&str>| text.map_or(ABSENT.to_owned(), one_line);
    let release = match late.group {
        DeliveryGroup::Release => held(late.release.as_deref()),
        DeliveryGroup::NoRelease => ABSENT.to_owned(),
        DeliveryGroup::Pool | DeliveryGroup::Unplaced => late.group.as_str().to_owned(),
    };
    let (standing, past) = match &late.accepted_at {
        Some(at) => (format!("accepted {}", one_line(at)), "late"),
        None => ("open".to_owned(), "overdue"),
    };
    let mut lines = vec![format!(
        "late {} | {}/{} | {release} | due {} | {standing} | {past} {}d | {}",
        one_line(&late.id),
        one_line(&late.flow),
        one_line(&late.column),
        one_line(&late.due),
        late.days_late,
        one_line(&late.title)
    )];
    for returned in &late.returns {
        lines.push(format!(
            "  returned {} | from {} | {}",
            one_line(&returned.at),
            one_line(&returned.from),
            held(returned.note.as_deref())
        ));
    }
    for reestimate in &late.reestimates {
        lines.push(format!(
            "  re-estimated {} | {} -> {} | {}",
            one_line(&reestimate.at),
            held(reestimate.old.as_deref()),
            held(reestimate.new.as_deref()),
            one_line(&reestimate.reason)
        ));
    }
    for subtask in &late.cancelled_subtasks {
        lines.push(format!(
            "  subtask {} cancelled {} | {}",
            one_line(&subtask.id),
            one_line(&subtask.at),
            one_line(&subtask.reason)
        ));
    }
    for item in &late.items {
        lines.push(format!(
            "  item {} | {} | {}",
            one_line(&item.id),
            one_line(&item.kind),
            one_line(&item.status)
        ));
    }
    let more = &late.more;
    let past: Vec<String> = [
        ("returns", more.returns),
        ("re-estimates", more.reestimates),
        ("cancelled subtasks", more.cancelled_subtasks),
        ("items", more.items),
    ]
    .into_iter()
    .filter(|&(_, count)| count > 0)
    .map(|(list, count)| format!("{list} {count}"))
    .collect();
    if !past.is_empty() {
        lines.push(format!("  more {}", past.join(", ")));
    }
    lines.iter().map(|line| escape_controls(line)).collect()
}

/// The header, release and department lines, the late rows that print,
/// then the tail when one does not.
pub(crate) fn render_text(outcome: &DeliveryOutcome) -> String {
    let mut lines = head_lines(&outcome.delivery);
    for late in outcome.shown_late() {
        lines.extend(late_lines(late));
    }
    if outcome.truncated {
        lines.push(format!(
            "[truncated: {} of {} late tasks not shown]",
            outcome.omitted(),
            outcome.delivery.late.len()
        ));
    }
    lines.iter().map(|line| format!("{line}\n")).collect()
}
