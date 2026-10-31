//! `spec card …` (canon `cards`; task spec `card-store`): a card is the
//! lighter record of the work of any trade, a row of the project's queue
//! beside its tasks, never a file or a commit. The project's `[card]` gives
//! each discipline its columns, its moves and who makes them; no code here
//! reads a column's name.
//!
//! - `new --title T …` → `created C-0001: plain/backlog`; `list` → a line per
//!   flow, then per card; `show C [--history]` → `key: value` lines.
//! - `move C [--to K] [--before C]`, `edit C …`, `assign C …`, `cancel C
//!   --reason T` → `<verb> C-0001: <flow>/<column>`; JSON `{id, flow, column,
//!   position, revision, allowed, notes}`, a refusal too; no such card `{id,
//!   reason}`. In `edit`, `assign` a value `-` clears.
//! - `accept C --as N`, a person's act on a terminal (`main` refuses a stdin
//!   that is no terminal before anything is read), with a `Consent`.
//!
//! `--as N` is self-declared, recorded and compared with a move's `by`, never
//! verified; absent: `local`. A tool acts as `agent`.
//!
//! Exits as the queue's: 0 done; 1 refused (no such card, a move outside its
//! flow, a closed card, a cap, a declined prompt, a stale `--revision`); 2
//! cannot run (`C` in `[ids]`, a look-alike ID, a bad `[card]`, no terminal,
//! a corrupt row, nothing asked). Nothing here opens git, so any root works;
//! nothing reads a proposal, a lock or a task, and nothing is written under
//! the root: the queue is in the data directory.

use serde::Serialize;
use specengine_core::card::{
    AGENT_ACTOR, AllowedMove, CLEAR, CardIdError, EVIDENCE_ENTRIES_MAX, HISTORY_MAX, LOCAL_ACTOR,
    accept_refusal, allowed_moves, card_look_alike_message, card_move as decide_move, card_number,
    card_prefix_clash, closed_refusal, due_problem, estimate_problem, parse_card_id,
    parse_evidence, reviewer_problem, stale_refusal,
};
use specengine_core::card_toml::{CardConfig, CardFlow, name_problem};
use specengine_core::proposal::author_field_problem;
use specengine_core::task::{
    CRITERIA_MAX, CRITERION_MAX, GOAL_MAX, NODES_MAX, NOTE_MAX, TITLE_MAX, over_bytes, over_items,
};
use specengine_model::{Card, CardAcceptance, CardCancellation, CardEvidence};
use specengine_store::{CardChange, CardEdit, Event, NewCard, QueueError, SqliteQueue};

use crate::apply::Consent;
use crate::cap::lines_within;
use crate::corpus::indexed;
use crate::location::prepared_data_dir;
use crate::project::{ProjectRoot, config_error, discover_with_text};
use crate::proposals::{checked_now, escaped_error, queue_cannot, require_root_config};
use crate::search::notes;
use crate::task::resolved_list;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// How a line prints an absent field.
const ABSENT: &str = "-";

/// Who a tool acts as (MCP's card tools).
pub const TOOL_ACTOR: &str = AGENT_ACTOR;

/// `spec card new` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardNewRequest {
    pub title: String,
    pub goal: Option<String>,
    pub discipline: Option<String>,
    /// `--target REF…`, resolved as `task new --nodes`; none: a manual card.
    pub targets: Vec<String>,
    pub criteria: Vec<String>,
    pub assignee: Option<String>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// `--as N`; `None`: `local`.
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card list` options: filters ANDed, exact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardListRequest {
    pub flow: Option<String>,
    pub column: Option<String>,
    pub discipline: Option<String>,
    pub assignee: Option<String>,
    /// `--no-target`: manual cards only.
    pub no_target: bool,
    /// `--with-cancelled`: cancelled cards too, last in their column.
    pub with_cancelled: bool,
}

/// `spec card show C [--history]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardShowRequest {
    pub id: String,
    /// The card's last `card.*` events.
    pub history: bool,
}

/// `spec card move C [--to K] [--before C]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardMoveRequest {
    pub id: String,
    /// A column of its flow; `None`: its own, with `before`.
    pub to: Option<String>,
    /// The card whose place it takes; `None`: last.
    pub before: Option<String>,
    /// `--revision R`: a compare-and-set on what the caller read.
    pub revision: Option<String>,
    /// `--as N`; `None`: `local`.
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card edit C …`: a value `-` clears; `targets`, `criteria` replace,
/// `evidence` appends.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardEditRequest {
    pub id: String,
    pub title: Option<String>,
    pub goal: Option<String>,
    pub targets: Vec<String>,
    pub criteria: Vec<String>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// Each `KIND=VALUE`.
    pub evidence: Vec<String>,
    pub revision: Option<String>,
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card assign C [--assignee N] [--reviewer N]`: a value `-` clears.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardAssignRequest {
    pub id: String,
    pub assignee: Option<String>,
    pub reviewer: Option<String>,
    pub revision: Option<String>,
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card cancel C --reason T`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardCancelRequest {
    pub id: String,
    pub reason: String,
    pub revision: Option<String>,
    pub actor: Option<String>,
    pub now: String,
}

/// `spec card accept C --as N [--note T]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardAcceptRequest {
    pub id: String,
    /// `--as N`, required: the card's reviewer.
    pub actor: String,
    pub note: Option<String>,
    pub revision: Option<String>,
    pub now: String,
}

/// What a card-changing command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardAction {
    New,
    Move,
    Edit,
    Assign,
    Cancel,
    Accept,
}

impl CardAction {
    /// The past tense its line opens with.
    const fn verb(&self) -> &'static str {
        match self {
            Self::New => "created",
            Self::Move => "moved",
            Self::Edit => "edited",
            Self::Assign => "assigned",
            Self::Cancel => "cancelled",
            Self::Accept => "accepted",
        }
    }
}

/// Why a command was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardCause {
    /// There is no such card: the document is `{id, reason}`.
    Unknown,
    /// A rule, a cap, a stale revision, a declined prompt.
    Refused,
}

/// What a card-changing command answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardOutcome {
    pub action: CardAction,
    /// The card's ID; `null` when what was given is none, or none was made.
    pub id: Option<String>,
    /// The card after the command (as read, when refused).
    pub card: Option<Box<Card>>,
    /// What it may do next.
    pub allowed: Vec<AllowedMove>,
    /// What a reader should know; a refusal's reason last.
    pub notes: Vec<String>,
    /// Why the command was refused (exit 1).
    pub refusal: Option<String>,
    pub cause: Option<CardCause>,
    pub messages: Vec<Message>,
}

impl CardOutcome {
    /// Exit 0, or 1 when refused.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    fn done(action: CardAction, card: Card, config: &CardConfig, messages: Vec<Message>) -> Self {
        let allowed = allowed_moves(config.flow(&card.flow), &card);
        Self {
            action,
            id: Some(card.id.clone()),
            card: Some(Box::new(card)),
            allowed,
            notes: notes(&messages),
            refusal: None,
            cause: None,
            messages,
        }
    }

    fn refused(
        action: CardAction,
        id: Option<&str>,
        card: Option<(&Card, &CardConfig)>,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        let reason = one_line(reason);
        let mut listed = notes(&messages);
        listed.push(reason.clone());
        Self {
            action,
            id: id.map(str::to_owned),
            card: card.map(|(card, _)| Box::new(card.clone())),
            allowed: card
                .map(|(card, config)| allowed_moves(config.flow(&card.flow), card))
                .unwrap_or_default(),
            notes: listed,
            refusal: Some(reason),
            cause: Some(CardCause::Refused),
            messages,
        }
    }

    fn unknown(action: CardAction, id: Option<&str>, reason: String) -> Self {
        Self {
            cause: Some(CardCause::Unknown),
            ..Self::refused(action, id, None, &reason, Vec::new())
        }
    }
}

#[derive(Serialize)]
struct CardWriteJson<'a> {
    id: Option<&'a str>,
    flow: Option<&'a str>,
    column: Option<&'a str>,
    position: Option<u64>,
    revision: Option<String>,
    allowed: &'a [AllowedMove],
    notes: &'a [String],
}

#[derive(Serialize)]
struct NoCardJson<'a> {
    id: Option<&'a str>,
    reason: Option<&'a str>,
}

/// `{id, flow, column, position, revision, allowed, notes}`, else exactly
/// `{id, reason}`.
impl Serialize for CardOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.cause == Some(CardCause::Unknown) {
            return NoCardJson {
                id: self.id.as_deref(),
                reason: self.refusal.as_deref(),
            }
            .serialize(serializer);
        }
        let card = self.card.as_deref();
        CardWriteJson {
            id: self.id.as_deref(),
            flow: card.map(|card| card.flow.as_str()),
            column: card.map(|card| card.column.as_str()),
            position: card.map(|card| card.position),
            revision: card.map(|card| card.revision.to_string()),
            allowed: &self.allowed,
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// What `spec card show` answered: the card, or why there is none.
#[derive(Debug, Clone, PartialEq)]
pub struct CardShowOutcome {
    pub card: Option<Box<Card>>,
    pub allowed: Vec<AllowedMove>,
    /// `--history`'s events, oldest first; `null` without it.
    pub history: Option<Vec<Event>>,
    pub notes: Vec<String>,
    /// The card asked for, when there is none.
    pub id: Option<String>,
    /// Why there is none (exit 1).
    pub reason: Option<String>,
    pub messages: Vec<Message>,
}

impl CardShowOutcome {
    /// Exit 0, or 1 when there is no such card.
    pub fn exit(&self) -> Exit {
        if self.reason.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

#[derive(Serialize)]
struct EventJson<'a> {
    seq: i64,
    #[serde(rename = "type")]
    event_type: &'a str,
    at: &'a str,
    payload: &'a serde_json::Value,
}

/// The columns in order, less `project`, then `allowed`, `history`, `notes`.
#[derive(Serialize)]
struct CardJson<'a> {
    id: &'a str,
    flow: &'a str,
    column: &'a str,
    position: u64,
    title: &'a str,
    goal: Option<&'a str>,
    discipline: Option<&'a str>,
    targets: &'a [String],
    snapshot: Option<&'a str>,
    criteria: &'a [String],
    assignee: Option<&'a str>,
    reviewer: Option<&'a str>,
    estimate: Option<&'a str>,
    due: Option<&'a str>,
    parent: Option<&'a str>,
    corrects: Option<&'a str>,
    evidence: &'a [CardEvidence],
    acceptance: Option<&'a CardAcceptance>,
    cancellation: Option<&'a CardCancellation>,
    created_by: &'a str,
    created_at: &'a str,
    updated_at: &'a str,
    revision: String,
    allowed: &'a [AllowedMove],
    history: Option<Vec<EventJson<'a>>>,
    notes: &'a [String],
}

impl Serialize for CardShowOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Some(card) = self.card.as_deref() else {
            return NoCardJson {
                id: self.id.as_deref(),
                reason: self.reason.as_deref(),
            }
            .serialize(serializer);
        };
        CardJson {
            id: &card.id,
            flow: &card.flow,
            column: &card.column,
            position: card.position,
            title: &card.title,
            goal: card.goal.as_deref(),
            discipline: card.discipline.as_deref(),
            targets: &card.targets,
            snapshot: card.snapshot.as_deref(),
            criteria: &card.criteria,
            assignee: card.assignee.as_deref(),
            reviewer: card.reviewer.as_deref(),
            estimate: card.estimate.as_deref(),
            due: card.due.as_deref(),
            parent: card.parent.as_deref(),
            corrects: card.corrects.as_deref(),
            evidence: &card.evidence,
            acceptance: card.acceptance.as_ref(),
            cancellation: card.cancellation.as_ref(),
            created_by: &card.created_by,
            created_at: &card.created_at,
            updated_at: &card.updated_at,
            revision: card.revision.to_string(),
            allowed: &self.allowed,
            history: self.history.as_ref().map(|events| {
                events
                    .iter()
                    .map(|event| EventJson {
                        seq: event.seq,
                        event_type: &event.event_type,
                        at: &event.at,
                        payload: &event.payload,
                    })
                    .collect()
            }),
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// What `spec card list` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardListOutcome {
    pub flows: Vec<CardFlow>,
    /// `flow <name> | <column> <n> | ...`, one per flow: the cards listed
    /// standing in each column, before the cut.
    pub flow_lines: Vec<String>,
    /// Every card listed, in list order.
    pub cards: Vec<Card>,
    /// How many of `cards` are printed (text and JSON alike).
    pub shown: usize,
    pub notes: Vec<String>,
    pub messages: Vec<Message>,
}

impl CardListOutcome {
    fn omitted(&self) -> usize {
        self.cards.len().saturating_sub(self.shown)
    }

    fn shown_cards(&self) -> &[Card] {
        &self.cards[..self.shown.min(self.cards.len())]
    }
}

/// A card as `list` gives it.
#[derive(Serialize)]
struct CardRowJson<'a> {
    id: &'a str,
    flow: &'a str,
    column: &'a str,
    position: u64,
    title: &'a str,
    discipline: Option<&'a str>,
    targets: &'a [String],
    assignee: Option<&'a str>,
    reviewer: Option<&'a str>,
    estimate: Option<&'a str>,
    due: Option<&'a str>,
    parent: Option<&'a str>,
    corrects: Option<&'a str>,
    updated_at: &'a str,
    revision: String,
    accepted: bool,
    cancelled: bool,
}

#[derive(Serialize)]
struct CardListJson<'a> {
    flows: &'a [CardFlow],
    cards: Vec<CardRowJson<'a>>,
    notes: &'a [String],
    truncated: bool,
    omitted: Option<usize>,
}

/// `{flows, cards, notes, truncated, omitted}`.
impl Serialize for CardListOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let omitted = self.omitted();
        CardListJson {
            flows: &self.flows,
            cards: self
                .shown_cards()
                .iter()
                .map(|card| CardRowJson {
                    id: &card.id,
                    flow: &card.flow,
                    column: &card.column,
                    position: card.position,
                    title: &card.title,
                    discipline: card.discipline.as_deref(),
                    targets: &card.targets,
                    assignee: card.assignee.as_deref(),
                    reviewer: card.reviewer.as_deref(),
                    estimate: card.estimate.as_deref(),
                    due: card.due.as_deref(),
                    parent: card.parent.as_deref(),
                    corrects: card.corrects.as_deref(),
                    updated_at: &card.updated_at,
                    revision: card.revision.to_string(),
                    accepted: card.acceptance.is_some(),
                    cancelled: card.cancellation.is_some(),
                })
                .collect(),
            notes: &self.notes,
            truncated: omitted > 0,
            omitted: (omitted > 0).then_some(omitted),
        }
        .serialize(serializer)
    }
}

// ------------------------------------------------------------- context

/// Where `spec card` runs: the project, its flows and its queue.
struct CardContext {
    project: ProjectRoot,
    config: CardConfig,
    queue: SqliteQueue,
}

/// The queue's context for a card command, git never asked: `--config` but
/// the root's own, `C` taken by `[ids]` and a bad `[card]` exit 2.
fn open_cards(env: &Env, globals: &Globals) -> Result<CardContext, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    if let Some(clash) = card_prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    let config = CardConfig::from_toml(&text)
        .map_err(|error| config_error(&project.config_label, &error))?;
    let data_dir = prepared_data_dir(env, &project)?;
    let queue =
        SqliteQueue::open(data_dir.join(format!("{slug}.db")), &slug).map_err(queue_cannot)?;
    Ok(CardContext {
        project,
        config,
        queue,
    })
}

/// The card ID as written: `Some` when it is one, `None` when it is none in
/// any script (exit 1); a look-alike exits 2 naming the Latin form.
fn written_card(written: &str) -> Result<Option<String>, CliError> {
    match parse_card_id(written) {
        Ok(id) => Ok(Some(id)),
        Err(CardIdError::LookAlike { fix }) => Err(CliError::spec(card_look_alike_message(
            written.trim(),
            &fix,
        ))),
        Err(CardIdError::NotAnId) => Ok(None),
    }
}

/// `no card C-0099`.
fn no_card(id: &str) -> String {
    format!("no card {id}")
}

/// Exit 1 for what is no card ID.
fn not_a_card(written: &str) -> String {
    format!(
        "no card `{}`: a card ID is `C-` and 4 or more digits, as `spec card list` lists it",
        written.trim()
    )
}

/// `--as N`: `local` when absent; off `author_field_problem`'s grammar exit
/// 2, as `--author-role`.
fn actor_of(written: Option<&str>) -> Result<String, CliError> {
    let actor = written.unwrap_or(LOCAL_ACTOR);
    match author_field_problem(actor) {
        Some(problem) => Err(CliError::spec(format!("--as: {problem}"))),
        None => Ok(actor.to_owned()),
    }
}

/// `--revision R`, a compare-and-set key: exit 2 for what no revision can be.
fn revision_of(written: Option<&str>) -> Result<Option<u64>, CliError> {
    let Some(written) = written else {
        return Ok(None);
    };
    written
        .parse::<u64>()
        .ok()
        .filter(|revision| *revision >= 1 && revision.to_string() == written)
        .map(Some)
        .ok_or_else(|| {
            CliError::spec(format!(
                "--revision: `{written}` is no revision (1, 2, ...)"
            ))
        })
}

/// A card a field names: the ID, a look-alike exiting 2; `Ok(Err)` the
/// refusal when it is no card ID.
fn linked_card(field: &str, written: &str) -> Result<Result<String, String>, CliError> {
    Ok(match written_card(written)? {
        Some(id) => Ok(id),
        None => Err(format!("{field}: {}", not_a_card(written))),
    })
}

/// A field as `edit` and `assign` take it: absent, cleared (`-`), or a value.
fn field(written: Option<&str>) -> Option<Option<String>> {
    written.map(|value| (value != CLEAR).then(|| value.to_owned()))
}

/// The first problem of the fields `new` and `edit` share.
fn field_problem(
    title: Option<&str>,
    goal: Option<&str>,
    targets: usize,
    criteria: &[String],
    estimate: Option<&str>,
    due: Option<&str>,
) -> Option<String> {
    title
        .filter(|title| title.trim().is_empty())
        .map(|_| "title: blank; a card has a title".to_owned())
        .or_else(|| title.and_then(|title| over_bytes("title", title, TITLE_MAX)))
        .or_else(|| goal.and_then(|goal| over_bytes("goal", goal, GOAL_MAX)))
        .or_else(|| over_items("targets", targets, NODES_MAX))
        .or_else(|| over_items("criteria", criteria.len(), CRITERIA_MAX))
        .or_else(|| {
            criteria
                .iter()
                .enumerate()
                .find_map(|(at, text)| over_bytes(&format!("criteria[{at}]"), text, CRITERION_MAX))
        })
        .or_else(|| estimate.and_then(estimate_problem))
        .or_else(|| due.and_then(due_problem))
}

/// Why `name` cannot be a person a card names.
fn person_problem(key: &str, name: &str) -> Option<String> {
    author_field_problem(name).map(|problem| format!("{key}: {problem}"))
}

/// The card a command names: `Ok(Err)` the refused outcome (exit 1).
fn named_card(
    context: &CardContext,
    written: &str,
    id: Option<String>,
    action: CardAction,
) -> Result<Result<Card, Box<CardOutcome>>, CliError> {
    let Some(id) = id else {
        return Ok(Err(Box::new(CardOutcome::unknown(
            action,
            None,
            not_a_card(written),
        ))));
    };
    match context.queue.get_card(&id).map_err(queue_cannot)? {
        Some(card) => Ok(Ok(card)),
        None => Ok(Err(Box::new(CardOutcome::unknown(
            action,
            Some(&id),
            no_card(&id),
        )))),
    }
}

/// A store refusal (exit 1) as the outcome, the card as it stands now;
/// anything else exit 2.
fn store_refusal(
    error: QueueError,
    action: CardAction,
    read: &Card,
    context: &CardContext,
) -> Result<CardOutcome, CliError> {
    let QueueError::CardRefused { reason, .. } = error else {
        return Err(queue_cannot(error));
    };
    let now = context
        .queue
        .get_card(&read.id)
        .map_err(queue_cannot)?
        .unwrap_or_else(|| read.clone());
    Ok(CardOutcome::refused(
        action,
        Some(&now.id),
        Some((&now, &context.config)),
        &reason,
        Vec::new(),
    ))
}

/// One change of `card` through the store, under its lock: the answer.
fn changed(
    context: &mut CardContext,
    action: CardAction,
    card: &Card,
    seen: Option<u64>,
    change: &CardChange<'_>,
    actor: &str,
    now: &str,
) -> Result<CardOutcome, CliError> {
    match context
        .queue
        .change_card(&card.id, seen, change, actor, now)
    {
        Ok(stored) => Ok(CardOutcome::done(
            action,
            stored,
            &context.config,
            Vec::new(),
        )),
        Err(error) => store_refusal(error, action, card, context),
    }
}

// ---------------------------------------------------------------- new

/// `spec card new`: stores a card, first column of its discipline's flow.
pub fn card_new(
    env: &Env,
    globals: &Globals,
    request: &CardNewRequest,
) -> Result<CardOutcome, CliError> {
    run_new(env, globals, request).map_err(escaped_error)
}

fn run_new(
    env: &Env,
    globals: &Globals,
    request: &CardNewRequest,
) -> Result<CardOutcome, CliError> {
    let refuse = |reason: String, messages: Vec<Message>| {
        Ok(CardOutcome::refused(
            CardAction::New,
            None,
            None,
            &reason,
            messages,
        ))
    };
    let now = checked_now(&request.now)?;
    let actor = actor_of(request.actor.as_deref())?;
    let parent = match request.parent.as_deref() {
        Some(written) => Some(linked_card("parent", written)?),
        None => None,
    };
    let corrects = match request.corrects.as_deref() {
        Some(written) => Some(linked_card("corrects", written)?),
        None => None,
    };
    let mut context = open_cards(env, globals)?;
    let (parent, corrects) = match (parent.transpose(), corrects.transpose()) {
        (Ok(parent), Ok(corrects)) => (parent, corrects),
        (Err(reason), _) | (_, Err(reason)) => return refuse(reason, Vec::new()),
    };
    let problem = field_problem(
        Some(&request.title),
        request.goal.as_deref(),
        request.targets.len(),
        &request.criteria,
        request.estimate.as_deref(),
        request.due.as_deref(),
    )
    .or_else(|| {
        request
            .discipline
            .as_deref()
            .and_then(name_problem)
            .map(|problem| format!("discipline: {problem}"))
    })
    .or_else(|| {
        request.assignee.as_deref().and_then(|name| {
            (name == CLEAR)
                .then(|| format!("assignee: `{CLEAR}` clears a name: a new card has none"))
                .or_else(|| person_problem("assignee", name))
        })
    });
    if let Some(problem) = problem {
        return refuse(problem, Vec::new());
    }
    let flow = match context.config.flow_for(request.discipline.as_deref()) {
        Ok(flow) => flow,
        Err(reason) => return refuse(reason, Vec::new()),
    };
    let (flow, column) = (flow.name.clone(), flow.first_column().to_owned());
    let mut messages = Vec::new();
    let targets = match resolved_targets(env, &context, &request.targets, &mut messages)? {
        Ok(targets) => targets,
        Err(reason) => return refuse(reason, messages),
    };
    let made = context.queue.create_card(
        &NewCard {
            flow,
            column,
            title: request.title.clone(),
            goal: request.goal.clone(),
            discipline: request.discipline.clone(),
            targets,
            criteria: request.criteria.clone(),
            assignee: request.assignee.clone(),
            estimate: request.estimate.clone(),
            due: request.due.clone(),
            parent,
            corrects,
            by: actor,
        },
        now,
    );
    match made {
        Ok(card) => Ok(CardOutcome::done(
            CardAction::New,
            card,
            &context.config,
            messages,
        )),
        Err(QueueError::CardRefused { reason, .. }) => refuse(reason, messages),
        Err(error) => Err(queue_cannot(error)),
    }
}

/// The nodes `--target` names, canonical, each once; a manual card reads no
/// index.
fn resolved_targets(
    env: &Env,
    context: &CardContext,
    written: &[String],
    messages: &mut Vec<Message>,
) -> Result<Result<Vec<String>, String>, CliError> {
    if written.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let input = indexed(env, &context.project, messages, false)?;
    resolved_list(&context.project, &input, written, "target")
}

// -------------------------------------------------------------- reads

/// `spec card show C [--history]`.
pub fn card_show(
    env: &Env,
    globals: &Globals,
    request: &CardShowRequest,
) -> Result<CardShowOutcome, CliError> {
    run_show(env, globals, request).map_err(escaped_error)
}

fn run_show(
    env: &Env,
    globals: &Globals,
    request: &CardShowRequest,
) -> Result<CardShowOutcome, CliError> {
    let missing = |id: Option<String>, reason: String| CardShowOutcome {
        card: None,
        allowed: Vec::new(),
        history: None,
        notes: Vec::new(),
        id,
        reason: Some(reason),
        messages: Vec::new(),
    };
    let id = written_card(&request.id)?;
    let context = open_cards(env, globals)?;
    let Some(id) = id else {
        return Ok(missing(None, not_a_card(&request.id)));
    };
    let Some(card) = context.queue.get_card(&id).map_err(queue_cannot)? else {
        return Ok(missing(Some(id.clone()), no_card(&id)));
    };
    let history = if request.history {
        Some(
            context
                .queue
                .card_history(&id, HISTORY_MAX)
                .map_err(queue_cannot)?,
        )
    } else {
        None
    };
    let notes: Vec<String> = stranded(&context.config, &card)
        .map(|why| format!("{}: {why}", card.id))
        .into_iter()
        .collect();
    Ok(CardShowOutcome {
        allowed: allowed_moves(context.config.flow(&card.flow), &card),
        card: Some(Box::new(card)),
        history,
        messages: notes.iter().cloned().map(Message::Note).collect(),
        notes,
        id: None,
        reason: None,
    })
}

/// Why `[card]` no longer places `card`: its flow or its column is gone.
fn stranded(config: &CardConfig, card: &Card) -> Option<String> {
    match config.flow(&card.flow) {
        None => Some(format!(
            "the flow `{}` is no longer in `[card]`: served last, no move open",
            card.flow
        )),
        Some(flow) if !flow.holds(&card.column) => Some(format!(
            "the column `{}` is no longer in the flow `{}`: served last",
            card.column, card.flow
        )),
        Some(_) => None,
    }
}

/// One note per (flow, column) `[card]` no longer places, its cards counted
/// and the first named: `cards` in list order, so a pair's cards stand
/// together.
fn stranded_notes(config: &CardConfig, cards: &[Card]) -> Vec<String> {
    cards
        .chunk_by(|one, next| one.flow == next.flow && one.column == next.column)
        .filter_map(|group| {
            let first = group.first()?;
            stranded(config, first).map(|why| {
                format!(
                    "{} card(s) in {}/{}, from {}: {why}",
                    group.len(),
                    first.flow,
                    first.column,
                    first.id
                )
            })
        })
        .collect()
}

/// `spec card list`: by flow, column (config order), position; a card
/// `[card]` no longer places last, cancelled ones last in their column.
pub fn card_list(
    env: &Env,
    globals: &Globals,
    request: &CardListRequest,
) -> Result<CardListOutcome, CliError> {
    run_list(env, globals, request).map_err(escaped_error)
}

fn run_list(
    env: &Env,
    globals: &Globals,
    request: &CardListRequest,
) -> Result<CardListOutcome, CliError> {
    let context = open_cards(env, globals)?;
    let listed = context.queue.list_cards().map_err(queue_cannot)?;
    let mut notes: Vec<String> = listed
        .unreadable
        .iter()
        .map(|row| one_line(&format!("{row}; not listed")))
        .collect();
    let exact = |asked: &Option<String>, value: Option<&str>| {
        asked.as_deref().is_none_or(|asked| Some(asked) == value)
    };
    let mut cards: Vec<Card> = listed
        .cards
        .into_iter()
        .filter(|card| {
            exact(&request.flow, Some(&card.flow))
                && exact(&request.column, Some(&card.column))
                && exact(&request.discipline, card.discipline.as_deref())
                && exact(&request.assignee, card.assignee.as_deref())
                && (!request.no_target || card.targets.is_empty())
                && (request.with_cancelled || card.cancellation.is_none())
        })
        .collect();
    let config = &context.config;
    cards.sort_by_cached_key(|card| list_key(config, card));
    notes.extend(stranded_notes(config, &cards));
    let flow_lines: Vec<String> = config
        .flows
        .iter()
        .map(|flow| flow_line(flow, &cards))
        .collect();
    let used = flow_lines
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum::<usize>();
    let (shown, _) = lines_within(
        cards.iter().map(|card| card_line(card).chars().count() + 1),
        used,
        false,
    );
    Ok(CardListOutcome {
        flows: config.flows.clone(),
        flow_lines,
        cards,
        shown,
        messages: notes.iter().cloned().map(Message::Note).collect(),
        notes,
    })
}

/// Where a name stands in `names`: its place, or past them all by name.
fn place_of<'n>(names: impl Iterator<Item = &'n str>, name: &str) -> (usize, String) {
    let mut count = 0;
    for known in names {
        if known == name {
            return (count, String::new());
        }
        count += 1;
    }
    (count, name.to_owned())
}

/// A card's place in the list. Ties by ID, by number.
fn list_key(config: &CardConfig, card: &Card) -> impl Ord + use<> {
    let flow = config.flow(&card.flow);
    let flow_place = place_of(
        config.flows.iter().map(|flow| flow.name.as_str()),
        &card.flow,
    );
    let column_place = match flow {
        Some(flow) => place_of(
            flow.columns.iter().map(|column| column.id.as_str()),
            &card.column,
        ),
        None => (0, card.column.clone()),
    };
    (
        flow_place,
        column_place,
        card.cancellation.is_some(),
        card.position,
        card_number(&card.id),
    )
}

/// `flow <name> | <column> <n> | ...`: the cards listed standing in each
/// column (a cancelled card has left it).
fn flow_line(flow: &CardFlow, cards: &[Card]) -> String {
    let mut line = format!("flow {}", flow.name);
    for column in &flow.columns {
        let count = cards
            .iter()
            .filter(|card| {
                card.flow == flow.name && card.column == column.id && card.cancellation.is_none()
            })
            .count();
        line.push_str(&format!(" | {} {count}", column.id));
    }
    line
}

/// `<id> | <flow>/<column> | <position> | <title> | <discipline> | <assignee>
/// | <due> | <targets>[ | cancelled]`, no line end.
fn card_line(card: &Card) -> String {
    let optional = |value: &Option<String>| value.as_deref().map_or(ABSENT.to_owned(), one_line);
    let targets = if card.targets.is_empty() {
        ABSENT.to_owned()
    } else {
        one_line(&card.targets.join(", "))
    };
    let mut line = format!(
        "{} | {}/{} | {} | {} | {} | {} | {} | {targets}",
        card.id,
        card.flow,
        card.column,
        card.position,
        one_line(&card.title),
        optional(&card.discipline),
        optional(&card.assignee),
        optional(&card.due),
    );
    if card.cancellation.is_some() {
        line.push_str(" | cancelled");
    }
    line
}

// ------------------------------------------------------------- writes

/// `spec card move C [--to K] [--before C]`: the flow's rule, a reorder
/// within a column none; last, or at `--before`'s place.
pub fn card_move(
    env: &Env,
    globals: &Globals,
    request: &CardMoveRequest,
) -> Result<CardOutcome, CliError> {
    run_move(env, globals, request).map_err(escaped_error)
}

fn run_move(
    env: &Env,
    globals: &Globals,
    request: &CardMoveRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Move;
    let now = checked_now(&request.now)?;
    if request.to.is_none() && request.before.is_none() {
        return Err(CliError::spec(
            "`spec card move` asks nothing: give --to K, --before C or both",
        ));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let before = match request.before.as_deref() {
        Some(written) => Some(linked_card("--before", written)?),
        None => None,
    };
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let refuse = |reason: &str, context: &CardContext| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            Vec::new(),
        ))
    };
    let before = match before.transpose() {
        Ok(before) => before,
        Err(reason) => return refuse(&reason, &context),
    };
    if let Some(reason) = move_refusal(&context, &card, seen, request.to.as_deref(), &actor) {
        return refuse(&reason, &context);
    }
    let config = context.config.clone();
    let change = CardChange::Move {
        flow: config.flow(&card.flow),
        to: request.to.clone(),
        before,
    };
    changed(&mut context, ACTION, &card, seen, &change, &actor, now)
}

/// What the flow says of this move before the store says it again under
/// its lock, in the store's order: a closed card, a stale revision, core's
/// decision.
fn move_refusal(
    context: &CardContext,
    card: &Card,
    seen: Option<u64>,
    to: Option<&str>,
    actor: &str,
) -> Option<String> {
    if let Some(reason) = closed_refusal(card) {
        return Some(reason);
    }
    if seen.is_some_and(|seen| seen != card.revision) {
        return Some(stale_refusal(card));
    }
    let flow = context.config.flow(&card.flow)?;
    let to = to.unwrap_or(&card.column);
    decide_move(flow, &card.column, to, actor)
        .err()
        .map(|refusal| refusal.message(&card.id, &card.column, to, actor))
}

/// `spec card edit C …`.
pub fn card_edit(
    env: &Env,
    globals: &Globals,
    request: &CardEditRequest,
) -> Result<CardOutcome, CliError> {
    run_edit(env, globals, request).map_err(escaped_error)
}

/// `--target -` or `--criterion -` alone: the list cleared.
fn clears(values: &[String]) -> bool {
    matches!(values, [value] if value == CLEAR)
}

fn run_edit(
    env: &Env,
    globals: &Globals,
    request: &CardEditRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Edit;
    let now = checked_now(&request.now)?;
    let asked = request.title.is_some()
        || request.goal.is_some()
        || !request.targets.is_empty()
        || !request.criteria.is_empty()
        || request.estimate.is_some()
        || request.due.is_some()
        || request.parent.is_some()
        || request.corrects.is_some()
        || !request.evidence.is_empty();
    if !asked {
        return Err(CliError::spec(
            "`spec card edit` asks nothing: give a field to change or --evidence",
        ));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let parent = link_field("parent", request.parent.as_deref())?;
    let corrects = link_field("corrects", request.corrects.as_deref())?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let mut messages = Vec::new();
    let refuse = |reason: &str, context: &CardContext, messages: Vec<Message>| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            messages,
        ))
    };
    if let Some(reason) = closed_refusal(&card) {
        return refuse(&reason, &context, messages);
    }
    let (parent, corrects) = match (parent.transpose(), corrects.transpose()) {
        (Ok(parent), Ok(corrects)) => (parent, corrects),
        (Err(reason), _) | (_, Err(reason)) => return refuse(&reason, &context, messages),
    };
    let criteria = if clears(&request.criteria) {
        &[][..]
    } else {
        &request.criteria[..]
    };
    let goal = field(request.goal.as_deref());
    let estimate = field(request.estimate.as_deref());
    let due = field(request.due.as_deref());
    let cleared_title = request.title.as_deref() == Some(CLEAR);
    let problem = cleared_title
        .then(|| "title: a card has one; `-` clears no title".to_owned())
        .or_else(|| {
            field_problem(
                request.title.as_deref(),
                goal.as_ref().and_then(Option::as_deref),
                request.targets.len(),
                criteria,
                estimate.as_ref().and_then(Option::as_deref),
                due.as_ref().and_then(Option::as_deref),
            )
        })
        .or_else(|| over_items("evidence", request.evidence.len(), EVIDENCE_ENTRIES_MAX));
    if let Some(problem) = problem {
        return refuse(&problem, &context, messages);
    }
    let mut evidence = Vec::with_capacity(request.evidence.len());
    for written in &request.evidence {
        match parse_evidence(written) {
            Ok(entry) => evidence.push(entry),
            Err(reason) => return refuse(&reason, &context, messages),
        }
    }
    let targets = if clears(&request.targets) {
        Some(Vec::new())
    } else if request.targets.is_empty() {
        None
    } else {
        match resolved_targets(env, &context, &request.targets, &mut messages)? {
            Ok(targets) => Some(targets),
            Err(reason) => return refuse(&reason, &context, messages),
        }
    };
    let edit = CardEdit {
        title: request.title.clone(),
        goal,
        targets,
        criteria: (!request.criteria.is_empty()).then(|| criteria.to_vec()),
        estimate,
        due,
        parent,
        corrects,
        evidence,
    };
    let change = CardChange::Edit(edit);
    let mut answer = changed(&mut context, ACTION, &card, seen, &change, &actor, now)?;
    answer.notes.splice(0..0, notes(&messages));
    answer.messages.splice(0..0, messages);
    Ok(answer)
}

/// A card field of `edit` as written: absent, cleared, a card; a look-alike
/// exits 2, `Err` inside the refusal when it is no card ID.
fn link_field(
    key: &str,
    written: Option<&str>,
) -> Result<Option<Result<Option<String>, String>>, CliError> {
    match field(written) {
        None => Ok(None),
        Some(None) => Ok(Some(Ok(None))),
        Some(Some(value)) => Ok(Some(linked_card(key, &value)?.map(Some))),
    }
}

/// `spec card assign C [--assignee N] [--reviewer N]`.
pub fn card_assign(
    env: &Env,
    globals: &Globals,
    request: &CardAssignRequest,
) -> Result<CardOutcome, CliError> {
    run_assign(env, globals, request).map_err(escaped_error)
}

fn run_assign(
    env: &Env,
    globals: &Globals,
    request: &CardAssignRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Assign;
    let now = checked_now(&request.now)?;
    if request.assignee.is_none() && request.reviewer.is_none() {
        return Err(CliError::spec(
            "`spec card assign` asks nothing: give --assignee N, --reviewer N or both",
        ));
    }
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let assignee = field(request.assignee.as_deref());
    let reviewer = field(request.reviewer.as_deref());
    let problem = closed_refusal(&card)
        .or_else(|| {
            assignee
                .as_ref()
                .and_then(Option::as_deref)
                .and_then(|name| person_problem("assignee", name))
        })
        .or_else(|| {
            reviewer
                .as_ref()
                .and_then(Option::as_deref)
                .and_then(|name| {
                    person_problem("reviewer", name)
                        .or_else(|| reviewer_problem(name).map(|text| format!("reviewer: {text}")))
                })
        });
    if let Some(problem) = problem {
        return Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            &problem,
            Vec::new(),
        ));
    }
    let change = CardChange::Assign { assignee, reviewer };
    changed(&mut context, ACTION, &card, seen, &change, &actor, now)
}

/// `spec card cancel C --reason T`: the card leaves its column.
pub fn card_cancel(
    env: &Env,
    globals: &Globals,
    request: &CardCancelRequest,
) -> Result<CardOutcome, CliError> {
    run_cancel(env, globals, request).map_err(escaped_error)
}

fn run_cancel(
    env: &Env,
    globals: &Globals,
    request: &CardCancelRequest,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Cancel;
    let now = checked_now(&request.now)?;
    let actor = actor_of(request.actor.as_deref())?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let problem = closed_refusal(&card)
        .or_else(|| {
            request
                .reason
                .trim()
                .is_empty()
                .then(|| "reason: blank; say why the card is cancelled".to_owned())
        })
        .or_else(|| over_bytes("reason", &request.reason, NOTE_MAX));
    if let Some(problem) = problem {
        return Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            &problem,
            Vec::new(),
        ));
    }
    let change = CardChange::Cancel {
        reason: request.reason.clone(),
    };
    changed(&mut context, ACTION, &card, seen, &change, &actor, now)
}

/// `spec card accept C --as N`: the card in `accept.from`, `N` its reviewer
/// and not its assignee, after the person's consent; to `accept.to`, which
/// nothing else reaches.
pub fn card_accept(
    env: &Env,
    globals: &Globals,
    request: &CardAcceptRequest,
    consent: Consent<'_>,
) -> Result<CardOutcome, CliError> {
    run_accept(env, globals, request, consent).map_err(escaped_error)
}

fn run_accept(
    env: &Env,
    globals: &Globals,
    request: &CardAcceptRequest,
    consent: Consent<'_>,
) -> Result<CardOutcome, CliError> {
    const ACTION: CardAction = CardAction::Accept;
    let now = checked_now(&request.now)?;
    let actor = actor_of(Some(&request.actor))?;
    let seen = revision_of(request.revision.as_deref())?;
    let id = written_card(&request.id)?;
    let mut context = open_cards(env, globals)?;
    let card = match named_card(&context, &request.id, id, ACTION)? {
        Ok(card) => card,
        Err(refused) => return Ok(*refused),
    };
    let refuse = |reason: &str, context: &CardContext| {
        Ok(CardOutcome::refused(
            ACTION,
            Some(&card.id),
            Some((&card, &context.config)),
            reason,
            Vec::new(),
        ))
    };
    let config = context.config.clone();
    let flow = config.flow(&card.flow);
    let problem = accept_refusal(&card, flow, &actor)
        .or_else(|| {
            seen.filter(|seen| *seen != card.revision)
                .map(|_| stale_refusal(&card))
        })
        .or_else(|| {
            request
                .note
                .as_deref()
                .and_then(|note| over_bytes("note", note, NOTE_MAX))
        });
    if let Some(problem) = problem {
        return refuse(&problem, &context);
    }
    let question = format!("accept {} ({}) as {actor}? [y/N]", card.id, card.title);
    if !consent(&escape_controls(&one_line(&question))) {
        return refuse(
            &format!(
                "`{}` not accepted: the answer was not `y`; nothing changed",
                card.id
            ),
            &context,
        );
    }
    let change = CardChange::Accept {
        flow,
        note: request.note.clone(),
    };
    changed(
        &mut context,
        ACTION,
        &card,
        Some(card.revision),
        &change,
        &actor,
        now,
    )
}

// -------------------------------------------------------------- text

/// `<verb> C-0001: <flow>/<column>`; `assigned C-0001: <assignee>, reviewer
/// <reviewer>`; nothing when refused.
pub(crate) fn render_text(outcome: &CardOutcome) -> String {
    let (None, Some(card)) = (&outcome.refusal, outcome.card.as_deref()) else {
        return String::new();
    };
    let verb = outcome.action.verb();
    let name = |value: &Option<String>| value.as_deref().unwrap_or(ABSENT).to_owned();
    let line = if outcome.action == CardAction::Assign {
        format!(
            "{verb} {}: {}, reviewer {}\n",
            card.id,
            name(&card.assignee),
            name(&card.reviewer)
        )
    } else {
        format!("{verb} {}: {}/{}\n", card.id, card.flow, card.column)
    };
    escape_controls(&line)
}

/// The `key: value` lines of a card, cut at [`crate::OUTPUT_CAP_CHARS`];
/// nothing when there is no card.
pub(crate) fn render_show_text(outcome: &CardShowOutcome) -> String {
    let Some(card) = outcome.card.as_deref() else {
        return String::new();
    };
    let lines = show_lines(card, outcome);
    let (shown, _) = lines_within(lines.iter().map(|line| line.chars().count() + 1), 0, false);
    let mut out = String::new();
    for line in &lines[..shown] {
        out.push_str(line);
        out.push('\n');
    }
    if shown < lines.len() {
        out.push_str(&format!(
            "[truncated: {} of {} lines not shown]\n",
            lines.len() - shown,
            lines.len()
        ));
    }
    escape_controls(&out)
}

fn show_lines(card: &Card, outcome: &CardShowOutcome) -> Vec<String> {
    let optional = |value: &Option<String>| value.as_deref().map_or(ABSENT.to_owned(), one_line);
    let mut lines = vec![
        format!("id: {}", card.id),
        format!("flow: {}", card.flow),
        format!("column: {}", card.column),
        format!("position: {}", card.position),
        format!("title: {}", one_line(&card.title)),
        format!("goal: {}", optional(&card.goal)),
        format!("discipline: {}", optional(&card.discipline)),
    ];
    let list = |lines: &mut Vec<String>, key: &str, values: Vec<String>| {
        if values.is_empty() {
            lines.push(format!("{key}: {ABSENT}"));
        }
        lines.extend(values.iter().map(|value| format!("{key}: {value}")));
    };
    list(
        &mut lines,
        "targets",
        card.targets.iter().map(|text| one_line(text)).collect(),
    );
    lines.push(format!("snapshot: {}", optional(&card.snapshot)));
    list(
        &mut lines,
        "criteria",
        card.criteria.iter().map(|text| one_line(text)).collect(),
    );
    lines.extend([
        format!("assignee: {}", optional(&card.assignee)),
        format!("reviewer: {}", optional(&card.reviewer)),
        format!("estimate: {}", optional(&card.estimate)),
        format!("due: {}", optional(&card.due)),
        format!("parent: {}", optional(&card.parent)),
        format!("corrects: {}", optional(&card.corrects)),
    ]);
    list(
        &mut lines,
        "evidence",
        card.evidence
            .iter()
            .map(|entry| {
                format!(
                    "{} {} ({}, {})",
                    entry.kind.as_str(),
                    one_line(&entry.value),
                    entry.by,
                    entry.at
                )
            })
            .collect(),
    );
    lines.push(format!(
        "acceptance: {}",
        card.acceptance.as_ref().map_or(ABSENT.to_owned(), |done| {
            match &done.note {
                Some(note) => format!("{} {}: {}", done.by, done.at, one_line(note)),
                None => format!("{} {}", done.by, done.at),
            }
        })
    ));
    lines.push(format!(
        "cancellation: {}",
        card.cancellation
            .as_ref()
            .map_or(ABSENT.to_owned(), |done| {
                format!("{} {}: {}", done.by, done.at, one_line(&done.reason))
            })
    ));
    lines.extend([
        format!("created_by: {}", card.created_by),
        format!("created_at: {}", card.created_at),
        format!("updated_at: {}", card.updated_at),
        format!("revision: {}", card.revision),
    ]);
    list(
        &mut lines,
        "allowed",
        outcome
            .allowed
            .iter()
            .map(|allowed| match allowed.by.as_slice() {
                [] => allowed.to.clone(),
                by => format!("{} (by {})", allowed.to, by.join(", ")),
            })
            .collect(),
    );
    list(
        &mut lines,
        "history",
        outcome
            .history
            .iter()
            .flatten()
            .map(|event| {
                format!(
                    "{} {} {} {}",
                    event.seq, event.event_type, event.at, event.payload
                )
            })
            .collect(),
    );
    lines
}

/// The flow lines, then a line per card printed, then the tail.
pub(crate) fn render_list_text(outcome: &CardListOutcome) -> String {
    let mut out = String::new();
    for line in &outcome.flow_lines {
        out.push_str(line);
        out.push('\n');
    }
    for card in outcome.shown_cards() {
        out.push_str(&card_line(card));
        out.push('\n');
    }
    let omitted = outcome.omitted();
    if omitted > 0 {
        out.push_str(&format!(
            "[truncated: {omitted} of {} cards not shown]\n",
            outcome.cards.len()
        ));
    }
    escape_controls(&out)
}
