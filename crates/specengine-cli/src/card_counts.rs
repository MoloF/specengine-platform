//! `spec card counts [--json]` (task spec `department-overview` DO1, DO5,
//! DO9, DO11): the project's cards counted per (flow, column) and per flow,
//! from the queue as it is, by the store's grouped statements in one read
//! transaction ([`SqliteQueue::card_counts`]), ordered by core's
//! [`counts_document`]. No count comes from `spec card list`, which is cut,
//! and none is stored. It moves, refuses and gates nothing: nothing is
//! blocked by it.
//!
//! One call reads `[card]` once, then opens the queue once with
//! [`SqliteQueue::open_existing`]: no schema step, no lock, nothing written;
//! no git, no index. No clock is read: one queue and config state gives one
//! text and one JSON at any root and `HOME`. No queue yet: every configured
//! column at 0. The flows are [`card_from_toml`]'s, the board's own: no
//! `[[card.flow]]` written reads core's default flow, as `spec card list`
//! serves it, so the counts and the board agree. `tasks_here` is whether the
//! agent tasks are read here: the command always reads them, a hub never
//! (`local_only`, every `review_agent_tasks` `null`: agent tasks are
//! local).
//!
//! ```text
//! card counts | flows <n> | cards <t>: open <o>, accepted <a>, cancelled <c> | agent tasks <counted or local_only>
//! flow <name> | <title or -> | open items <n>, oldest <at or -> | ready parents <r> | agent tasks in review <T, ... or - or local_only>[ | not configured]
//!   <flow>/<column> | open <o>, accepted <a>, cancelled <c>, open subtasks <s>[ | not in the flow]
//! [truncated: <k> of <n> lines not shown]
//! ```
//!
//! `flows` counts the configured flows; `cards` every card counted, a
//! column or flow `[card]` no longer lists included. A flow no `[card]`
//! lists has no title and every one of its columns is not in the flow. Each
//! value is on one line and every line's control characters are escaped;
//! the JSON holds the values raw. The header prints whole, then the flow and
//! column lines in order, each whole, while all of them are at most
//! [`OUTPUT_CAP_CHARS`](crate::OUTPUT_CAP_CHARS); `<n>` counts every line,
//! the header too. The JSON
//! holds the `columns` and `by_flow` rows the text prints, `omitted` the
//! rows cut ([`CardCountsOutcome::in_view`] with the daemon's
//! [`View::Browser`]: nothing cut, `truncated` false, `omitted` `null`). The
//! notes go to stderr as `note:` lines and into the JSON's `notes`, by card
//! ID; none changes the exit. Exit 2: no project, a bad `[card]`
//! (`<config>:<line>: <message>`, as every `spec card` command), `C` in
//! `[ids]`, `--config` but the root's own, or a queue that cannot be read (a
//! newer build's).

use serde::Serialize;
use specengine_core::card::card_prefix_clash;
use specengine_core::card_counts::{
    AgentTasks, CardColumnCount, CardCountRows, CardCounts, CardFlowCount, counts_document,
};
use specengine_core::card_toml::card_from_toml;
use specengine_store::SqliteQueue;

use crate::cap::{View, lines_within};
use crate::location::db_path;
use crate::project::{config_error, discover_with_text};
use crate::proposals::{escaped_error, queue_cannot, require_root_config};
use crate::{CliError, Env, Globals, escape_controls, one_line};

/// What a text line shows for no value.
const ABSENT: &str = "-";

/// `spec card counts` options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardCountsRequest {
    /// Read the agent tasks the open cards link (the command: always); a
    /// hub passes `false` and reads none.
    pub tasks_here: bool,
}

/// What `spec card counts` counted and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardCountsOutcome {
    /// Core's document, whole: its `truncated` false, its `omitted` `None`.
    pub counts: CardCounts,
    /// How many flow and column lines print, the first ones, after the
    /// header.
    pub shown: usize,
}

/// A line after the header: a `by_flow` row or a `columns` row, by index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowLine {
    Flow(usize),
    Column(usize),
}

impl CardCountsOutcome {
    /// The counts as `view` bounds them: [`View::Browser`] prints every
    /// line (`truncated` false, `omitted` `null`, the same keys);
    /// [`View::Capped`] the lines within
    /// [`OUTPUT_CAP_CHARS`](crate::OUTPUT_CAP_CHARS).
    #[must_use]
    pub fn in_view(mut self, view: View) -> Self {
        self.shown = match view {
            View::Browser => row_lines(&self.counts).len(),
            View::Capped => fitting(&self.counts),
        };
        self
    }

    /// The flow and column lines that do not print.
    fn omitted(&self) -> usize {
        row_lines(&self.counts).len().saturating_sub(self.shown)
    }

    /// One `note:` line per note, in order, escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        self.counts
            .notes
            .iter()
            .map(|note| escape_controls(&format!("note: {}", one_line(note))))
            .collect()
    }
}

/// Core's document with the rows the text prints: `{flows, columns,
/// by_flow, agent_tasks, notes, truncated, omitted}`.
impl Serialize for CardCountsOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let lines = row_lines(&self.counts);
        let shown = &lines[..self.shown.min(lines.len())];
        let flows = shown
            .iter()
            .filter(|line| matches!(line, RowLine::Flow(_)))
            .count();
        let omitted = self.omitted();
        let mut document = self.counts.clone();
        document.by_flow.truncate(flows);
        document.columns.truncate(shown.len() - flows);
        document.truncated = omitted > 0;
        document.omitted = (omitted > 0).then_some(omitted);
        document.serialize(serializer)
    }
}

/// `spec card counts`: `[card]` and the queue, each read once, counted.
pub fn card_counts(
    env: &Env,
    globals: &Globals,
    request: &CardCountsRequest,
) -> Result<CardCountsOutcome, CliError> {
    run_counts(env, globals, request).map_err(escaped_error)
}

fn run_counts(
    env: &Env,
    globals: &Globals,
    request: &CardCountsRequest,
) -> Result<CardCountsOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    if let Some(clash) = card_prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    // The board's flows, as `spec card list` reads them: no flow written is
    // core's default one.
    let flows = card_from_toml(&text)
        .map_err(|error| config_error(&project.config_label, &error))?
        .flows;
    let db = db_path(env, &slug)?;
    let rows = match SqliteQueue::open_existing(&db, &slug).map_err(queue_cannot)? {
        Some(queue) => queue
            .card_counts(request.tasks_here)
            .map_err(queue_cannot)?,
        None => CardCountRows::default(),
    };
    let mut counts = counts_document(&flows, &rows, request.tasks_here);
    counts.notes = counts.notes.iter().map(|note| one_line(note)).collect();
    Ok(CardCountsOutcome { counts, shown: 0 }.in_view(View::Capped))
}

/// The lines after the header, in order: each flow's line, then its
/// column lines (core gives `columns` grouped by flow in `by_flow`'s order),
/// any column of no listed flow last.
fn row_lines(counts: &CardCounts) -> Vec<RowLine> {
    let mut lines = Vec::with_capacity(counts.by_flow.len() + counts.columns.len());
    let mut next = 0;
    for (index, flow) in counts.by_flow.iter().enumerate() {
        lines.push(RowLine::Flow(index));
        while counts
            .columns
            .get(next)
            .is_some_and(|column| column.flow == flow.flow)
        {
            lines.push(RowLine::Column(next));
            next += 1;
        }
    }
    lines.extend((next..counts.columns.len()).map(RowLine::Column));
    lines
}

/// How many lines after the header print within
/// [`OUTPUT_CAP_CHARS`](crate::OUTPUT_CAP_CHARS), each whole, the header's
/// characters counted first.
fn fitting(counts: &CardCounts) -> usize {
    let cost = |line: String| line.chars().count() + 1;
    let (shown, _) = lines_within(
        row_lines(counts)
            .into_iter()
            .map(|line| cost(row_text(counts, line))),
        cost(header(counts)),
        false,
    );
    shown
}

/// `card counts | flows <n> | cards <t>: open <o>, accepted <a>, cancelled
/// <c> | agent tasks <word>`, escaped.
fn header(counts: &CardCounts) -> String {
    let (open, accepted, cancelled) =
        counts
            .columns
            .iter()
            .fold((0_u64, 0_u64, 0_u64), |(open, accepted, cancelled), row| {
                (
                    open + row.open,
                    accepted + row.accepted,
                    cancelled + row.cancelled,
                )
            });
    escape_controls(&format!(
        "card counts | flows {} | cards {}: open {open}, accepted {accepted}, cancelled \
         {cancelled} | agent tasks {}",
        counts.flows.len(),
        open + accepted + cancelled,
        counts.agent_tasks.as_str()
    ))
}

/// One line after the header, escaped.
fn row_text(counts: &CardCounts, line: RowLine) -> String {
    let text = match line {
        RowLine::Flow(index) => flow_text(counts, &counts.by_flow[index]),
        RowLine::Column(index) => column_text(counts, &counts.columns[index]),
    };
    escape_controls(&text)
}

/// `flow <name> | <title> | open items <n>, oldest <at> | ready parents <r>
/// | agent tasks in review <ids>[ | not configured]`.
fn flow_text(counts: &CardCounts, row: &CardFlowCount) -> String {
    let configured = counts.flows.iter().find(|flow| flow.name == row.flow);
    let title = configured.map_or(ABSENT.to_owned(), |flow| one_line(&flow.title));
    let oldest = row
        .oldest_open_item
        .as_deref()
        .map_or(ABSENT.to_owned(), one_line);
    let review = match &row.review_agent_tasks {
        None => AgentTasks::LocalOnly.as_str().to_owned(),
        Some(ids) if ids.is_empty() => ABSENT.to_owned(),
        Some(ids) => one_line(&ids.join(", ")),
    };
    let mut text = format!(
        "flow {} | {title} | open items {}, oldest {oldest} | ready parents {} | agent tasks in \
         review {review}",
        one_line(&row.flow),
        row.open_items,
        row.ready_parents
    );
    if configured.is_none() {
        text.push_str(" | not configured");
    }
    text
}

/// `  <flow>/<column> | open <o>, accepted <a>, cancelled <c>, open subtasks
/// <s>[ | not in the flow]`.
fn column_text(counts: &CardCounts, row: &CardColumnCount) -> String {
    let listed = counts
        .flows
        .iter()
        .any(|flow| flow.name == row.flow && flow.holds(&row.column));
    let mut text = format!(
        "  {}/{} | open {}, accepted {}, cancelled {}, open subtasks {}",
        one_line(&row.flow),
        one_line(&row.column),
        row.open,
        row.accepted,
        row.cancelled,
        row.open_subtasks
    );
    if !listed {
        text.push_str(" | not in the flow");
    }
    text
}

/// The header, the flow and column lines that print, then the tail when one
/// does not.
pub(crate) fn render_text(outcome: &CardCountsOutcome) -> String {
    let counts = &outcome.counts;
    let lines = row_lines(counts);
    let mut out = header(counts);
    out.push('\n');
    for line in lines.iter().take(outcome.shown) {
        out.push_str(&row_text(counts, *line));
        out.push('\n');
    }
    let omitted = outcome.omitted();
    if omitted > 0 {
        out.push_str(&format!(
            "[truncated: {omitted} of {} lines not shown]\n",
            lines.len() + 1
        ));
    }
    out
}
