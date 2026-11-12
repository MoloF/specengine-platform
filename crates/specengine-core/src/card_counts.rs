//! A project's card counts, the pure half (task spec `department-overview`
//! DO2, DO8, DO9): the store's grouped rows in, the counts document out,
//! ordered by `[card]`. Nothing is read, written or stored here, and no clock
//! is read: an item's time is its stored `created_at`, never an age.
//!
//! - **Order**: the configured flows in config order, then the flows no
//!   `[card]` lists, by name. Inside a configured flow its columns in config
//!   order, zeros too, then the columns it no longer lists, by name; a flow
//!   no `[card]` lists has its stored columns alone, by name. A column it no
//!   longer lists carries no mark of its own: the served `flows` tell it.
//! - **`by_flow`**: one row per flow in that order, zeros too.
//! - **Agent tasks**: with `tasks_here`, each flow's distinct `T-` IDs in
//!   review, by number; without (a hub), `local_only` and every
//!   `review_agent_tasks` `null`, never an empty list (ADR-0047, ADR-0075).
//! - No name is compared with a literal: a flow, a column and a department
//!   are data (ADR-0008).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::card::card_number;
use crate::card_toml::CardFlow;
use crate::task::task_number;

/// One stored (flow, column)'s cards: each card in exactly one of the first
/// three (a cancellation set is cancelled, else an acceptance set is
/// accepted, else open), and the open ones with a parent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ColumnTally {
    pub open: u64,
    pub accepted: u64,
    pub cancelled: u64,
    pub open_subtasks: u64,
}

/// One flow's open items: the open questions, discrepancies and decisions on
/// its cards, whatever each card's state, and their least `created_at`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct OpenItems {
    pub count: u64,
    /// `None`: none, or none with a stored time.
    pub oldest: Option<String>,
}

/// What the store's one read of the counts gives, every name as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardCountRows {
    /// Per stored (flow, column).
    pub columns: BTreeMap<(String, String), ColumnTally>,
    /// Per parent's flow: its open parents with a child and none open.
    pub ready_parents: BTreeMap<String, u64>,
    /// Per card's flow.
    pub open_items: BTreeMap<String, OpenItems>,
    /// Per flow, the distinct `T-` IDs its open cards link whose task is in
    /// review; read only when agent tasks are counted here.
    pub review_tasks: BTreeMap<String, BTreeSet<String>>,
    /// The open cards whose `tasks` does not read: passed over, named.
    pub unread_tasks: BTreeSet<String>,
    /// The cards whose stored `flow` or `col` is no text, each counted
    /// under the name it reads as; a card's `flow` before its `col`.
    pub unread_names: Vec<UnreadName>,
}

/// A card whose stored `flow` or `col` is no text (task spec
/// `department-overview` DO9): it still counts, under `counted_as`, and is
/// named (ADR-0047).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnreadName {
    /// The card's ID as stored.
    pub card: String,
    /// The stored column: `flow` or `col`.
    pub column: String,
    /// Why it cannot be read: `it is NULL`, ….
    pub reason: String,
    /// The name it counts under.
    pub counted_as: String,
}

/// Whether the agent tasks were counted here: a hub reads no task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTasks {
    Counted,
    LocalOnly,
}

impl AgentTasks {
    /// The served word.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Counted => "counted",
            Self::LocalOnly => "local_only",
        }
    }
}

/// One (flow, column)'s row: the 6 keys, in this order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CardColumnCount {
    pub flow: String,
    pub column: String,
    pub open: u64,
    pub accepted: u64,
    pub cancelled: u64,
    pub open_subtasks: u64,
}

/// One flow's row: the 5 keys, in this order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CardFlowCount {
    pub flow: String,
    pub open_items: u64,
    pub oldest_open_item: Option<String>,
    pub ready_parents: u64,
    /// By number; `None` when the agent tasks are [`AgentTasks::LocalOnly`].
    pub review_agent_tasks: Option<Vec<String>>,
}

/// The counts document: the 7 keys, in this order. `flows` is
/// `spec card list --json`'s. [`counts_document`] gives it whole
/// (`truncated` false, `omitted` `None`); a cut is the caller's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardCounts {
    pub flows: Vec<CardFlow>,
    pub columns: Vec<CardColumnCount>,
    pub by_flow: Vec<CardFlowCount>,
    pub agent_tasks: AgentTasks,
    pub notes: Vec<String>,
    pub truncated: bool,
    /// The `columns` and `by_flow` rows cut; `None` uncut.
    pub omitted: Option<usize>,
}

/// The note of an open card whose `tasks` does not read.
pub fn unread_tasks_note(card: &str) -> String {
    format!("{card}: its `tasks` does not read; its agent tasks are not counted")
}

/// The note of a card whose stored `flow` or `col` cannot be read.
pub fn unread_name_note(name: &UnreadName) -> String {
    format!(
        "{}: the stored `{}` cannot be read: {}; counted under `{}`",
        name.card, name.column, name.reason, name.counted_as
    )
}

/// Where `id` sorts by number (`number` of it), any that is no such ID
/// after them by text.
fn number_key(id: &str, number: fn(&str) -> Option<u64>) -> ((u8, u64), String) {
    (
        number(id).map_or((1, 0), |number| (0, number)),
        id.to_owned(),
    )
}

/// `ids` by number ([`number_key`]).
fn by_number<'i>(
    ids: impl IntoIterator<Item = &'i String>,
    number: fn(&str) -> Option<u64>,
) -> Vec<String> {
    let mut sorted: Vec<String> = ids.into_iter().cloned().collect();
    sorted.sort_by_cached_key(|id| number_key(id, number));
    sorted
}

/// The document of `rows` under the configured `flows` (task spec
/// `department-overview` DO9), in the order the module documentation says.
/// `tasks_here` false: [`AgentTasks::LocalOnly`], every
/// `review_agent_tasks` `None` and no note of a card's links, whatever
/// `rows` holds.
pub fn counts_document(flows: &[CardFlow], rows: &CardCountRows, tasks_here: bool) -> CardCounts {
    let configured: BTreeSet<&str> = flows.iter().map(|flow| flow.name.as_str()).collect();
    let others: BTreeSet<&str> = rows
        .columns
        .keys()
        .map(|(flow, _)| flow)
        .chain(rows.ready_parents.keys())
        .chain(rows.open_items.keys())
        .chain(rows.review_tasks.keys())
        .map(String::as_str)
        .filter(|flow| !configured.contains(flow))
        .collect();
    let order: Vec<(&str, Option<&CardFlow>)> = flows
        .iter()
        .map(|flow| (flow.name.as_str(), Some(flow)))
        .chain(others.into_iter().map(|name| (name, None)))
        .collect();

    let mut columns = Vec::new();
    for &(name, flow) in &order {
        let row = |column: &str, tally: ColumnTally| CardColumnCount {
            flow: name.to_owned(),
            column: column.to_owned(),
            open: tally.open,
            accepted: tally.accepted,
            cancelled: tally.cancelled,
            open_subtasks: tally.open_subtasks,
        };
        if let Some(flow) = flow {
            for column in &flow.columns {
                let tally = rows
                    .columns
                    .get(&(name.to_owned(), column.id.clone()))
                    .copied()
                    .unwrap_or_default();
                columns.push(row(&column.id, tally));
            }
        }
        // The stored columns the flow does not list, by name: the map's
        // order within one flow.
        columns.extend(
            rows.columns
                .iter()
                .filter(|((stored, column), _)| {
                    stored == name && flow.is_none_or(|flow| !flow.holds(column))
                })
                .map(|((_, column), tally)| row(column, *tally)),
        );
    }

    let by_flow = order
        .iter()
        .map(|&(name, _)| {
            let items = rows.open_items.get(name);
            CardFlowCount {
                flow: name.to_owned(),
                open_items: items.map_or(0, |items| items.count),
                oldest_open_item: items.and_then(|items| items.oldest.clone()),
                ready_parents: rows.ready_parents.get(name).copied().unwrap_or(0),
                review_agent_tasks: tasks_here.then(|| {
                    rows.review_tasks
                        .get(name)
                        .map(|ids| by_number(ids, task_number))
                        .unwrap_or_default()
                }),
            }
        })
        .collect();

    // By card ID: a card's names first, `flow` before `col` as the store
    // gives them (a stable sort), then its links, read only here.
    let mut notes: Vec<(&str, u8, String)> = rows
        .unread_names
        .iter()
        .map(|name| (name.card.as_str(), 0, unread_name_note(name)))
        .collect();
    if tasks_here {
        notes.extend(
            rows.unread_tasks
                .iter()
                .map(|card| (card.as_str(), 1, unread_tasks_note(card))),
        );
    }
    notes.sort_by_cached_key(|&(card, kind, _)| (number_key(card, card_number), kind));
    let notes = notes.into_iter().map(|(_, _, note)| note).collect();

    CardCounts {
        flows: flows.to_vec(),
        columns,
        by_flow,
        agent_tasks: if tasks_here {
            AgentTasks::Counted
        } else {
            AgentTasks::LocalOnly
        },
        notes,
        truncated: false,
        omitted: None,
    }
}
