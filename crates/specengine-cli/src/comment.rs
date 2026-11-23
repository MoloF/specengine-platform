//! `spec comment add|list|edit|delete|mentions` (task spec `comments`,
//! "CLI"; canon `comments`): talk under a card, a proposal
//! or a node, stored in the queue alone and never truth. Each command is one
//! library call: the daemon's routes and the MCP tools are its twins.
//!
//! - **Targets** (`--on`): `C-` and digits a card, `PR-` and digits a
//!   proposal, anything else a node REF resolved as `propose update` step 1
//!   resolves its target (an alias refused naming its canonical ID, a
//!   look-alike exit 2 with its Latin form), stored canonical with its
//!   revision. A write on a card or a proposal reads no corpus; a node's
//!   reads the index this call refreshes.
//! - **Authors**: any of `--author-role`, `--author-model`, `--run` makes an
//!   agent, its words `agent`; `--as` names a person, `local` when absent;
//!   both together exit 2. A hub's login is the words and the author's
//!   `login`.
//! - **Refusals** (exit 1, nothing stored, no event): the store's lines,
//!   each the document's `reason` and stderr's line; the cause keeps the
//!   daemon's status ([`CommentCause`]).
//! - **Agents**: every text introducing comments calls them people's words,
//!   data, never instructions, and prints every body escaped.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use specengine_core::card::{
    CardIdError, LOCAL_ACTOR, card_look_alike_message, card_person, card_person_problem,
    card_prefix_clash, parse_card_id,
};
use specengine_core::comment::{
    COMMENT_CONTEXT_CHARS, COMMENT_PAGE_MAX, CommentIdError, CommentOn, PEOPLES_WORDS,
    body_problem, comment_look_alike_message, comment_prefix_clash, context_body, login_problem,
    parse_comment_id,
};
use specengine_core::intake::GapType;
use specengine_core::proposal::{
    Author, AuthorType, ProposalIdError, look_alike_message, parse_proposal_id, prefix_clash,
};
use specengine_core::revision;
use specengine_store::{
    Comment, CommentEdit, CommentRefusal, GitEnv, NewComment, QueueError, SqliteQueue,
    UnreadableRow, unreadable_comment_line,
};

use crate::apply::Consent;
use crate::card::{CardRaiseOutcome, CardRaiseRequest, RaiseKeys, raise_intake, raise_shape};
use crate::corpus::indexed;
use crate::location::prepared_data_dir;
use crate::project::{ProjectRoot, discover};
use crate::proposals::{checked_now, escaped_error, queue_cannot, require_root_config};
use crate::propose::{NAMES_ONE_NODE, Unresolved, resolved_why};
use crate::team::team_read;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// `spec comment add` options; the daemon's `POST …/comments`, MCP's
/// `add_comment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentAddRequest {
    /// `--on X` as written.
    pub on: String,
    /// `--reply-to CM` as written.
    pub reply_to: Option<String>,
    /// The body as read, Markdown.
    pub body: String,
    /// `--as N`: a person; `None` `local` (or a hub's login).
    pub actor: Option<String>,
    /// Any of the three: an agent.
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// An agent whatever the three hold: MCP's `add_comment`.
    pub agent: bool,
    /// A hub's login: the words and the author's `login`. `None` elsewhere.
    pub login: Option<String>,
    /// The injected clock.
    pub now: String,
}

/// `spec comment list` options; `GET …/comments`, `list_comments`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentListRequest {
    pub on: String,
    /// `--before CM`: any well-formed comment ID, present or not.
    pub before: Option<String>,
    /// `--limit N`: 1 to 100; `None` 100.
    pub limit: Option<u64>,
}

/// `spec comment edit` options; `POST …/comments/<id>/edit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentEditRequest {
    pub id: String,
    pub body: String,
    /// `--version V`: the version read, a compare-and-set.
    pub version: u64,
    pub actor: Option<String>,
    pub login: Option<String>,
    pub now: String,
}

/// `spec comment delete` options; `DELETE …/comments/<id>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDeleteRequest {
    pub id: String,
    /// `--as N`; `None` `local` (or a hub's login).
    pub actor: Option<String>,
    /// A hub's login: the author's login, or a `[team]` `owner`, deletes.
    pub login: Option<String>,
    pub now: String,
}

/// `spec comment mentions` options; `GET …/mentions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentMentionsRequest {
    /// `--login L`; `None` every mention.
    pub login: Option<String>,
    pub before: Option<String>,
    pub limit: Option<u64>,
}

/// Why a comment command was refused, for the daemon's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentCause {
    /// A body the caps refuse: exit 1, a 400.
    Body,
    /// No such comment or target: a 404.
    Unknown,
    /// A rule: a 409.
    Refused,
    /// Not the person to make it: a 403.
    Forbidden,
}

impl From<CommentRefusal> for CommentCause {
    fn from(refusal: CommentRefusal) -> Self {
        match refusal {
            CommentRefusal::Unknown => Self::Unknown,
            CommentRefusal::Refused => Self::Refused,
            CommentRefusal::Forbidden => Self::Forbidden,
        }
    }
}

/// A refusal: `{id, reason}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentRefused {
    pub id: Option<String>,
    pub reason: String,
    #[serde(skip)]
    pub cause: CommentCause,
}

/// `on`: `{kind, id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentOnJson {
    pub kind: &'static str,
    pub id: String,
}

/// `deleted`: `{by, at}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentDeletedJson {
    pub by: String,
    pub at: String,
}

/// An item raised from a comment: `{id, kind, status}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentItemJson {
    pub id: String,
    pub kind: Option<String>,
    pub status: Option<String>,
}

/// The comment document, 13 keys (task spec `comments`, "The comment
/// document").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommentDocument {
    pub id: String,
    pub on: CommentOnJson,
    pub on_rev: Option<u64>,
    pub reply_to: Option<String>,
    pub actor: String,
    pub author: Author,
    pub version: u64,
    pub body: Option<String>,
    pub mentions: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted: Option<CommentDeletedJson>,
    pub items: Vec<CommentItemJson>,
}

impl CommentDocument {
    pub fn of(comment: &Comment) -> Self {
        Self {
            id: comment.id.clone(),
            on: CommentOnJson {
                kind: comment.on.as_str(),
                id: comment.on_id.clone(),
            },
            on_rev: comment.on_rev,
            reply_to: comment.reply_to.clone(),
            actor: comment.actor.clone(),
            author: comment.author.clone(),
            version: comment.version,
            body: comment.body.clone(),
            mentions: comment.mentions.clone(),
            created_at: comment.created_at.clone(),
            updated_at: comment.updated_at.clone(),
            deleted: comment.deleted.as_ref().map(|deleted| CommentDeletedJson {
                by: deleted.by.clone(),
                at: deleted.at.clone(),
            }),
            items: comment
                .items
                .iter()
                .map(|item| CommentItemJson {
                    id: item.id.clone(),
                    kind: item.kind.clone(),
                    status: item.status.clone(),
                })
                .collect(),
        }
    }
}

/// What `add`, `edit` and `delete` answered: the comment, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentOutcome {
    pub comment: Option<CommentDocument>,
    pub refused: Option<CommentRefused>,
    /// `added`, `edited` or `deleted`.
    pub verb: &'static str,
    pub messages: Vec<Message>,
}

impl CommentOutcome {
    pub fn exit(&self) -> Exit {
        if self.refused.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    /// Why it was refused, for the daemon; `None` when done.
    pub fn cause(&self) -> Option<CommentCause> {
        self.refused.as_ref().map(|refused| refused.cause)
    }

    fn refused(id: Option<&str>, reason: String, cause: CommentCause, verb: &'static str) -> Self {
        Self {
            comment: None,
            refused: Some(CommentRefused {
                id: id.map(str::to_owned),
                reason: one_line(&reason),
                cause,
            }),
            verb,
            messages: Vec::new(),
        }
    }

    fn done(comment: &Comment, verb: &'static str, messages: Vec<Message>) -> Self {
        Self {
            comment: Some(CommentDocument::of(comment)),
            refused: None,
            verb,
            messages,
        }
    }
}

impl Serialize for CommentOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match (&self.comment, &self.refused) {
            (Some(comment), _) => comment.serialize(serializer),
            (None, Some(refused)) => refused.serialize(serializer),
            (None, None) => serializer.serialize_none(),
        }
    }
}

/// What `list` answered: `{on, total, comments, next_before, notes}`, or
/// why there is no such target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentListOutcome {
    pub on: CommentOnJson,
    pub total: u64,
    pub comments: Vec<CommentDocument>,
    pub next_before: Option<String>,
    pub notes: Vec<String>,
    pub refused: Option<CommentRefused>,
    pub messages: Vec<Message>,
}

#[derive(Serialize)]
struct CommentListJson<'a> {
    on: &'a CommentOnJson,
    total: u64,
    comments: &'a [CommentDocument],
    next_before: Option<&'a str>,
    notes: &'a [String],
}

impl CommentListOutcome {
    pub fn exit(&self) -> Exit {
        if self.refused.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    pub fn cause(&self) -> Option<CommentCause> {
        self.refused.as_ref().map(|refused| refused.cause)
    }
}

impl Serialize for CommentListOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.refused {
            Some(refused) => refused.serialize(serializer),
            None => CommentListJson {
                on: &self.on,
                total: self.total,
                comments: &self.comments,
                next_before: self.next_before.as_deref(),
                notes: &self.notes,
            }
            .serialize(serializer),
        }
    }
}

/// What `mentions` answered: `{login, comments, next_before, notes}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MentionsOutcome {
    pub login: Option<String>,
    pub comments: Vec<CommentDocument>,
    pub next_before: Option<String>,
    pub notes: Vec<String>,
    #[serde(skip)]
    pub messages: Vec<Message>,
}

/// The project and its queue for a comment command, git never asked:
/// `--config` but the root's own and `CM` taken by `[ids]` exit 2.
struct OpenComments {
    project: ProjectRoot,
    queue: SqliteQueue,
}

fn open_comments(env: &Env, globals: &Globals) -> Result<OpenComments, CliError> {
    let project = discover(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    if let Some(clash) = comment_prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    let data_dir = prepared_data_dir(env, &project)?;
    let queue =
        SqliteQueue::open(data_dir.join(format!("{slug}.db")), &slug).map_err(queue_cannot)?;
    Ok(OpenComments { project, queue })
}

/// What `--on` reads as.
pub(crate) enum Written {
    Card(String),
    Proposal(String),
    Node(String),
}

/// `--on X` as written: a card, a proposal or a node REF; a look-alike card
/// or proposal ID exits 2 with its Latin form, and so does an `[ids]` taking
/// the prefix it reads as.
pub(crate) fn written_on(project: &ProjectRoot, written: &str) -> Result<Written, CliError> {
    let trimmed = written.trim();
    match parse_card_id(trimmed) {
        Ok(id) => {
            if let Some(clash) = card_prefix_clash(&project.config.scheme) {
                return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
            }
            return Ok(Written::Card(id));
        }
        Err(CardIdError::LookAlike { fix }) => {
            return Err(CliError::spec(format!(
                "on: {}",
                card_look_alike_message(trimmed, &fix)
            )));
        }
        Err(CardIdError::NotAnId) => {}
    }
    match parse_proposal_id(trimmed) {
        Ok(id) => {
            if let Some(clash) = prefix_clash(&project.config.scheme) {
                return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
            }
            return Ok(Written::Proposal(id));
        }
        Err(ProposalIdError::LookAlike { fix }) => {
            return Err(CliError::spec(format!(
                "on: {}",
                look_alike_message(trimmed, &fix)
            )));
        }
        Err(ProposalIdError::NotAnId) => {}
    }
    Ok(Written::Node(trimmed.to_owned()))
}

/// A node REF resolved over the index this call refreshes: its canonical
/// ID and revision, or why not (exit 1, the resolve's line).
pub(crate) fn resolved_node_rev(
    env: &Env,
    project: &ProjectRoot,
    written: &str,
    messages: &mut Vec<Message>,
) -> Result<Result<(String, Option<u64>), Unresolved>, CliError> {
    let input = indexed(env, project, messages, false)?;
    let (id, path, ord) = match resolved_why(project, &input, written)? {
        Ok(found) => found,
        Err(unresolved) => return Ok(Err(unresolved)),
    };
    let rev = input
        .files
        .iter()
        .find(|file| file.path == path)
        .and_then(|file| file.parsed.as_ref())
        .and_then(|parsed| revision(parsed, ord).number())
        .map(u64::from);
    Ok(Ok((id, rev)))
}

/// A node REF's refusal: one naming the node's canonical form (an alias, a
/// section through its document, a revision, a wiki link) or one naming
/// several nodes (held by several files, declared twice) is a rule, a 409;
/// anything else names no node, a 404.
pub(crate) fn resolve_cause(unresolved: &Unresolved) -> CommentCause {
    if unresolved.another_form || unresolved.ambiguous {
        CommentCause::Refused
    } else {
        CommentCause::Unknown
    }
}

/// A node REF's refusal line for a comment: `on: <reason>`, a target naming
/// several nodes worded for a comment.
fn on_line(unresolved: &Unresolved) -> String {
    on_line_of(unresolved, "a comment names one node")
}

/// A node REF's refusal line, `on: <reason>`, a target naming several
/// nodes worded `names` (a comment's, a reference's).
pub(crate) fn on_line_of(unresolved: &Unresolved, names: &str) -> String {
    let reason = if unresolved.ambiguous {
        unresolved.reason.replace(NAMES_ONE_NODE, names)
    } else {
        unresolved.reason.clone()
    };
    format!("on: {reason}")
}

/// A comment ID as written: a look-alike exits 2; what is none in any
/// script is `None` (exit 1 where a command names its comment).
fn written_comment(field: &str, written: &str) -> Result<Option<String>, CliError> {
    match parse_comment_id(written) {
        Ok(id) => Ok(Some(id)),
        Err(CommentIdError::LookAlike { fix }) => Err(CliError::spec(format!(
            "{field}{}",
            comment_look_alike_message(written.trim(), &fix)
        ))),
        Err(CommentIdError::NotAnId) => Ok(None),
    }
}

/// The line for what is no comment ID.
fn not_a_comment(written: &str) -> String {
    format!(
        "no comment `{}`: a comment ID is `CM-` and 4 or more digits",
        written.trim()
    )
}

/// `--before CM`: a well-formed comment ID, else exit 2.
fn before_of(written: Option<&str>) -> Result<Option<String>, CliError> {
    let Some(written) = written else {
        return Ok(None);
    };
    match written_comment("--before: ", written)? {
        Some(id) => Ok(Some(id)),
        None => Err(CliError::spec(format!(
            "--before: {}",
            not_a_comment(written)
        ))),
    }
}

/// `--limit N`: 1 to 100, else exit 2; `None` 100.
fn limit_of(written: Option<u64>) -> Result<usize, CliError> {
    match written {
        None => Ok(COMMENT_PAGE_MAX),
        Some(limit) if (1..=COMMENT_PAGE_MAX as u64).contains(&limit) => {
            Ok(usize::try_from(limit).unwrap_or(COMMENT_PAGE_MAX))
        }
        Some(limit) => Err(CliError::spec(format!(
            "--limit: {limit}; 1 to {COMMENT_PAGE_MAX}"
        ))),
    }
}

/// Why a page's query is no query a list or the mentions take (the
/// daemon's 400, nothing run): `before` no comment ID, `limit` outside 1 to
/// 100, `login` off the login grammar; the CLI's line. `None`: it is one.
pub fn comment_query_problem(
    before: Option<&str>,
    limit: Option<i64>,
    login: Option<&str>,
) -> Option<String> {
    let limit = match limit {
        None => None,
        Some(limit) => match u64::try_from(limit) {
            Ok(limit) => Some(limit),
            Err(_) => return Some(format!("limit: {limit}; 1 to {COMMENT_PAGE_MAX}")),
        },
    };
    if let Some(problem) = login.and_then(login_problem) {
        return Some(format!("login: {problem}"));
    }
    before_of(before)
        .and_then(|_| limit_of(limit))
        .err()
        .map(|error| {
            error
                .message
                .strip_prefix("spec: --")
                .unwrap_or(&error.message)
                .to_owned()
        })
}

/// The person's words of `--as`, trimmed: `local` when absent; off a card
/// person's grammar exit 2.
pub(crate) fn person_of(written: Option<&str>) -> Result<String, CliError> {
    let actor = written.unwrap_or(LOCAL_ACTOR);
    match card_person_problem(actor) {
        Some(problem) => Err(CliError::spec(format!("--as: {problem}"))),
        None => Ok(card_person(actor).to_owned()),
    }
}

/// A store refusal as an outcome; anything else exit 2.
fn store_refusal(
    error: QueueError,
    verb: &'static str,
    fallback: &str,
) -> Result<CommentOutcome, CliError> {
    match error {
        QueueError::CommentRefused { id, reason, cause } => {
            let id = if id.is_empty() { fallback } else { id.as_str() };
            let cause = if reason.starts_with("body: ") {
                CommentCause::Body
            } else {
                cause.into()
            };
            Ok(CommentOutcome::refused(Some(id), reason, cause, verb))
        }
        error => Err(queue_cannot(error)),
    }
}

/// `spec comment add`: one comment stored under its target (see the module
/// documentation), `comment.created` logged, nothing else written.
pub fn comment_add(
    env: &Env,
    globals: &Globals,
    request: &CommentAddRequest,
) -> Result<CommentOutcome, CliError> {
    run_add(env, globals, request).map_err(escaped_error)
}

fn run_add(
    env: &Env,
    globals: &Globals,
    request: &CommentAddRequest,
) -> Result<CommentOutcome, CliError> {
    const VERB: &str = "added";
    let now = checked_now(&request.now)?;
    let agent = request.agent
        || request.author_role.is_some()
        || request.author_model.is_some()
        || request.run.is_some();
    if agent && request.actor.is_some() && request.login.is_none() {
        return Err(CliError::spec(
            "`--as` names a person and `--author-role` an agent: give one",
        ));
    }
    let mut author = Author::new(
        request.author_role.clone(),
        request.author_model.clone(),
        request.run.clone(),
    )
    .map_err(CliError::spec)?;
    if agent {
        author.author_type = AuthorType::Agent;
    }
    author.login.clone_from(&request.login);
    let actor = match (&request.login, agent) {
        (Some(login), _) => login.clone(),
        (None, true) => specengine_core::card::AGENT_ACTOR.to_owned(),
        (None, false) => person_of(request.actor.as_deref())?,
    };
    let reply_to = match &request.reply_to {
        None => None,
        Some(written) => match written_comment("reply_to: ", written)? {
            Some(id) => Some(id),
            None => {
                return Ok(CommentOutcome::refused(
                    None,
                    format!("reply_to: {}", not_a_comment(written)),
                    CommentCause::Refused,
                    VERB,
                ));
            }
        },
    };
    let mut context = open_comments(env, globals)?;
    let mut messages = Vec::new();
    let written = written_on(&context.project, &request.on)?;
    // The body's caps before a node's target walks the corpus.
    if let Some(problem) = body_problem(&request.body) {
        return Ok(CommentOutcome::refused(
            None,
            problem,
            CommentCause::Body,
            VERB,
        ));
    }
    let (on, on_id, on_rev) = match written {
        Written::Card(id) => (CommentOn::Card, id, None),
        Written::Proposal(id) => (CommentOn::Proposal, id, None),
        Written::Node(written) => {
            match resolved_node_rev(env, &context.project, &written, &mut messages)? {
                Ok((id, rev)) => (CommentOn::Node, id, rev),
                Err(unresolved) => {
                    let mut outcome = CommentOutcome::refused(
                        Some(&written),
                        on_line(&unresolved),
                        resolve_cause(&unresolved),
                        VERB,
                    );
                    outcome.messages = messages;
                    return Ok(outcome);
                }
            }
        }
    };
    let new = NewComment {
        on,
        on_id: on_id.clone(),
        on_rev,
        reply_to,
        actor,
        author,
        body: request.body.clone(),
    };
    match context.queue.add_comment(&new, now) {
        Ok(comment) => Ok(CommentOutcome::done(&comment, VERB, messages)),
        Err(error) => {
            let mut outcome = store_refusal(error, VERB, &on_id)?;
            outcome.messages = messages;
            Ok(outcome)
        }
    }
}

/// `spec comment list`: one page of a target's comments, oldest first (see
/// the module documentation). A node that no longer resolves but has
/// comments stored under that exact ID lists them, with a note.
pub fn comment_list(
    env: &Env,
    globals: &Globals,
    request: &CommentListRequest,
) -> Result<CommentListOutcome, CliError> {
    run_list(env, globals, request).map_err(escaped_error)
}

fn run_list(
    env: &Env,
    globals: &Globals,
    request: &CommentListRequest,
) -> Result<CommentListOutcome, CliError> {
    let before = before_of(request.before.as_deref())?;
    let limit = limit_of(request.limit)?;
    let context = open_comments(env, globals)?;
    let mut messages = Vec::new();
    let mut notes = Vec::new();
    let refused = |id: &str, reason: String, cause: CommentCause, messages: Vec<Message>| {
        CommentListOutcome {
            on: CommentOnJson {
                kind: CommentOn::Node.as_str(),
                id: id.to_owned(),
            },
            total: 0,
            comments: Vec::new(),
            next_before: None,
            notes: Vec::new(),
            refused: Some(CommentRefused {
                id: Some(id.to_owned()),
                reason: one_line(&reason),
                cause,
            }),
            messages,
        }
    };
    let (on, on_id) = match written_on(&context.project, &request.on)? {
        Written::Card(id) => {
            if context.queue.get_card(&id).map_err(queue_cannot)?.is_none() {
                return Ok(refused(
                    &id,
                    format!("on: no card {id}"),
                    CommentCause::Unknown,
                    messages,
                ));
            }
            (CommentOn::Card, id)
        }
        Written::Proposal(id) => {
            use specengine_store::ProposalQueue as _;
            let found = match context.queue.get(&id) {
                Ok(found) => found.is_some(),
                // A row that does not decode is still a proposal.
                Err(QueueError::Store(error)) => return Err(CliError::spec(error)),
                Err(_) => true,
            };
            if !found {
                return Ok(refused(
                    &id,
                    format!("on: no proposal {id}"),
                    CommentCause::Unknown,
                    messages,
                ));
            }
            (CommentOn::Proposal, id)
        }
        Written::Node(written) => {
            match resolved_node_rev(env, &context.project, &written, &mut messages)? {
                Ok((id, _)) => (CommentOn::Node, id),
                Err(unresolved) => {
                    // A gone node's comments stay under its old ID.
                    let stored = context
                        .queue
                        .list_comments(CommentOn::Node, &written, None, 1)
                        .map_err(queue_cannot)?;
                    if stored.total == 0 {
                        let cause = resolve_cause(&unresolved);
                        let reason = on_line(&unresolved);
                        return Ok(refused(&written, reason, cause, messages));
                    }
                    notes.push(format!("{written} is no node now; its comments as stored"));
                    (CommentOn::Node, written)
                }
            }
        }
    };
    let page = context
        .queue
        .list_comments(on, &on_id, before.as_deref(), limit)
        .map_err(queue_cannot)?;
    notes.extend(page.unreadable.iter().map(unreadable_comment_line));
    messages.extend(notes.iter().cloned().map(Message::Note));
    Ok(CommentListOutcome {
        on: CommentOnJson {
            kind: on.as_str(),
            id: on_id,
        },
        total: page.total,
        comments: page.comments.iter().map(CommentDocument::of).collect(),
        next_before: page.next_before,
        notes,
        refused: None,
        messages,
    })
}

/// `spec comment edit`: the author's new body as version n + 1, the old
/// one kept (task spec `comments`, "Rules and edge cases", Edits).
pub fn comment_edit(
    env: &Env,
    globals: &Globals,
    request: &CommentEditRequest,
) -> Result<CommentOutcome, CliError> {
    run_edit(env, globals, request).map_err(escaped_error)
}

fn run_edit(
    env: &Env,
    globals: &Globals,
    request: &CommentEditRequest,
) -> Result<CommentOutcome, CliError> {
    const VERB: &str = "edited";
    let now = checked_now(&request.now)?;
    let actor = match &request.login {
        Some(login) => login.clone(),
        None => person_of(request.actor.as_deref())?,
    };
    let Some(id) = written_comment("", &request.id)? else {
        return Ok(CommentOutcome::refused(
            None,
            not_a_comment(&request.id),
            CommentCause::Unknown,
            VERB,
        ));
    };
    let mut context = open_comments(env, globals)?;
    if let Some(problem) = body_problem(&request.body) {
        return Ok(CommentOutcome::refused(
            Some(&id),
            problem,
            CommentCause::Body,
            VERB,
        ));
    }
    let edit = CommentEdit {
        id: id.clone(),
        body: request.body.clone(),
        version: request.version,
        actor,
        login: request.login.clone(),
    };
    match context.queue.edit_comment(&edit, now) {
        Ok(comment) => Ok(CommentOutcome::done(&comment, VERB, Vec::new())),
        Err(error) => store_refusal(error, VERB, &id),
    }
}

/// `spec comment delete`: on a hub the author's login or a `[team]`
/// `owner` (judged here, in the call, nothing read from a body; `[team]`
/// absent or bad: the author alone); elsewhere whoever is at the terminal
/// or the page, after `consent` (task spec `comments`, "Delete"). Every
/// version's text erased, a tombstone kept, `comment.deleted` logged.
pub fn comment_delete(
    env: &Env,
    globals: &Globals,
    request: &CommentDeleteRequest,
    consent: Consent<'_>,
) -> Result<CommentOutcome, CliError> {
    run_delete(env, globals, request, consent).map_err(escaped_error)
}

fn run_delete(
    env: &Env,
    globals: &Globals,
    request: &CommentDeleteRequest,
    consent: Consent<'_>,
) -> Result<CommentOutcome, CliError> {
    const VERB: &str = "deleted";
    let now = checked_now(&request.now)?;
    let by = match &request.login {
        Some(login) => login.clone(),
        None => person_of(request.actor.as_deref())?,
    };
    let Some(id) = written_comment("", &request.id)? else {
        return Ok(CommentOutcome::refused(
            None,
            not_a_comment(&request.id),
            CommentCause::Unknown,
            VERB,
        ));
    };
    let mut context = open_comments(env, globals)?;
    let Some(comment) = context.queue.comment_row(&id).map_err(queue_cannot)? else {
        return Ok(CommentOutcome::refused(
            Some(&id),
            format!("no comment {id}"),
            CommentCause::Unknown,
            VERB,
        ));
    };
    if comment.deleted.is_some() {
        return Ok(CommentOutcome::refused(
            Some(&id),
            format!("{id} is deleted: nothing changed"),
            CommentCause::Refused,
            VERB,
        ));
    }
    if let Some(login) = &request.login
        && comment.author.login.as_deref() != Some(login.as_str())
    {
        // `[team]` read now; absent or bad, the author alone deletes.
        let owner = team_read(env, globals)
            .ok()
            .flatten()
            .and_then(|team| team.entry(login).map(|entry| entry.role.deletes_comments()))
            .unwrap_or(false);
        if !owner {
            return Ok(CommentOutcome::refused(
                Some(&id),
                format!(
                    "{id} is {}'s: only its author or an `owner` deletes it",
                    comment.actor
                ),
                CommentCause::Forbidden,
                VERB,
            ));
        }
    }
    let versions = comment.version;
    let question = format!(
        "delete {id} ({}, on {}), erasing the text of its {versions} version{}? [y/N]",
        comment.actor,
        comment.on_id,
        if versions == 1 { "" } else { "s" }
    );
    if !consent(&escape_controls(&one_line(&question))) {
        return Ok(CommentOutcome::refused(
            Some(&id),
            format!("{id} not deleted: the answer was not `y`; nothing changed"),
            CommentCause::Refused,
            VERB,
        ));
    }
    match context.queue.delete_comment(&id, &by, now) {
        Ok(comment) => Ok(CommentOutcome::done(&comment, VERB, Vec::new())),
        Err(error) => store_refusal(error, VERB, &id),
    }
}

/// `spec comment mentions`: one page of the comments mentioning `--login`
/// (every mention when absent), newest first. The server keeps no reader
/// state.
pub fn comment_mentions(
    env: &Env,
    globals: &Globals,
    request: &CommentMentionsRequest,
) -> Result<MentionsOutcome, CliError> {
    run_mentions(env, globals, request).map_err(escaped_error)
}

fn run_mentions(
    env: &Env,
    globals: &Globals,
    request: &CommentMentionsRequest,
) -> Result<MentionsOutcome, CliError> {
    if let Some(problem) = request.login.as_deref().and_then(login_problem) {
        return Err(CliError::spec(format!("--login: {problem}")));
    }
    let before = before_of(request.before.as_deref())?;
    let limit = limit_of(request.limit)?;
    let context = open_comments(env, globals)?;
    let page = context
        .queue
        .comment_mentions(request.login.as_deref(), before.as_deref(), limit)
        .map_err(queue_cannot)?;
    let notes: Vec<String> = page
        .unreadable
        .iter()
        .map(unreadable_comment_line)
        .collect();
    Ok(MentionsOutcome {
        login: request.login.clone(),
        comments: page.comments.iter().map(CommentDocument::of).collect(),
        next_before: page.next_before,
        messages: notes.iter().cloned().map(Message::Note).collect(),
        notes,
    })
}

/// The body of `POST …/comments`: `{on, body, reply_to, as}`; any other
/// key refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentAddBody {
    pub on: String,
    pub body: String,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(rename = "as", default)]
    pub actor: Option<String>,
}

impl CommentAddBody {
    /// `bytes` decoded, `as` a card person (trimmed); else exit 2, nothing
    /// read or changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, "\"on\",\"body\",\"reply_to\",\"as\"")?;
        Ok(Self {
            actor: checked_as(body.actor)?,
            ..body
        })
    }

    /// The request at `now`, `login` a hub's.
    pub fn request(self, now: String, login: Option<String>) -> CommentAddRequest {
        CommentAddRequest {
            on: self.on,
            reply_to: self.reply_to,
            body: self.body,
            actor: self.actor,
            author_role: None,
            author_model: None,
            run: None,
            agent: false,
            login,
            now,
        }
    }
}

/// The body of `POST …/comments/<id>/edit`: `{body, version, as}`; a
/// missing `version` or any other key refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentEditBody {
    pub body: String,
    pub version: u64,
    #[serde(rename = "as", default)]
    pub actor: Option<String>,
}

impl CommentEditBody {
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, "\"body\",\"version\",\"as\"")?;
        Ok(Self {
            actor: checked_as(body.actor)?,
            ..body
        })
    }

    pub fn request(self, id: String, now: String, login: Option<String>) -> CommentEditRequest {
        CommentEditRequest {
            id,
            body: self.body,
            version: self.version,
            actor: self.actor,
            login,
            now,
        }
    }
}

/// The body of `POST …/comments/<id>/items` (`CommentRaiseBody`): the card
/// door's keys, a discrepancy's too (`crate::card::raise_shape`); any other
/// key refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentRaiseBody {
    /// `question`, `decision` or `discrepancy`.
    pub kind: String,
    pub node_ids: Vec<String>,
    pub text: String,
    pub working_answer: Option<String>,
    pub price_of_other: Option<String>,
    pub why: Option<String>,
    pub severity: Option<specengine_core::intake::IntakeSeverity>,
    pub distinct_from: Option<Vec<String>>,
    pub gap_type: Option<GapType>,
    pub observed: Option<String>,
    pub documented: Option<String>,
    #[serde(rename = "as")]
    pub actor: Option<String>,
}

impl CommentRaiseBody {
    /// `bytes` decoded, its kind with its keys and `as` a card person;
    /// else exit 2, nothing read or changed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = decoded(bytes, crate::card::RAISE_KEYS)?;
        raise_shape("a comment", &body.keys())?;
        Ok(Self {
            actor: checked_as(body.actor)?,
            ..body
        })
    }

    /// The keys [`raise_shape`] judges.
    fn keys(&self) -> RaiseKeys<'_> {
        RaiseKeys {
            kind: &self.kind,
            working_answer: self.working_answer.as_deref(),
            price_of_other: self.price_of_other.as_deref(),
            why: self.why.as_deref(),
            gap_type: self.gap_type,
            observed: self.observed.as_deref(),
            documented: self.documented.as_deref(),
        }
    }

    fn raise_request(
        &self,
        id: String,
        now: String,
        git: GitEnv,
        login: Option<String>,
    ) -> Result<CardRaiseRequest, CliError> {
        let kind = raise_shape("a comment", &self.keys())?;
        Ok(CardRaiseRequest {
            id,
            kind,
            node_ids: self.node_ids.clone(),
            text: self.text.clone(),
            working_answer: self.working_answer.clone(),
            price_of_other: self.price_of_other.clone(),
            why: self.why.clone(),
            severity: self.severity,
            distinct_from: self.distinct_from.clone().unwrap_or_default(),
            gap_type: self.gap_type,
            observed: self.observed.clone(),
            documented: self.documented.clone(),
            actor: self.actor.clone(),
            login,
            now,
            git,
        })
    }

    /// The request for the comment `id` (the path's, as written).
    pub fn request(
        self,
        id: String,
        now: String,
        git: GitEnv,
        login: Option<String>,
    ) -> Result<CommentRaiseRequest, CliError> {
        Ok(CommentRaiseRequest {
            raise: self.raise_request(id, now, git, login)?,
        })
    }
}

/// [`comment_raise`]'s options: a question, a decision or a discrepancy
/// raised from the comment `raise.id` by a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRaiseRequest {
    pub raise: CardRaiseRequest,
}

/// The daemon's comment door (task spec `comments`, "HTTP"): the intake of
/// the body's kind with `comment_id` the comment, a card comment's item
/// naming its card too, `as` trailing its `proposal.created`. No such
/// comment: `{id, reason}` before any intake rule (a 404); a deleted one is
/// refused by the intake, under its lock.
pub fn comment_raise(
    env: &Env,
    globals: &Globals,
    request: &CommentRaiseRequest,
) -> Result<CardRaiseOutcome, CliError> {
    run_raise(env, globals, request).map_err(escaped_error)
}

fn run_raise(
    env: &Env,
    globals: &Globals,
    request: &CommentRaiseRequest,
) -> Result<CardRaiseOutcome, CliError> {
    let raise = &request.raise;
    checked_now(&raise.now)?;
    let actor = match &raise.login {
        Some(login) => login.clone(),
        None => person_of(raise.actor.as_deref())?,
    };
    let Some(id) = written_comment("", &raise.id)? else {
        return Ok(CardRaiseOutcome::missing(None, not_a_comment(&raise.id)));
    };
    let context = open_comments(env, globals)?;
    let Some(comment) = context.queue.comment_row(&id).map_err(queue_cannot)? else {
        return Ok(CardRaiseOutcome::missing(
            Some(id.clone()),
            format!("no comment {id}"),
        ));
    };
    // The queue is opened again by the intake, under its own lock.
    drop(context);
    let card = (comment.on == CommentOn::Card).then(|| comment.on_id.clone());
    let intake = raise_intake(env, globals, raise, card, &id, Some(&id), &actor)?;
    Ok(CardRaiseOutcome::raised(intake))
}

/// A body's `as`, trimmed, a card person's; else exit 2.
pub(crate) fn checked_as(actor: Option<String>) -> Result<Option<String>, CliError> {
    match actor {
        None => Ok(None),
        Some(actor) => person_of(Some(&actor)).map(Some),
    }
}

/// `bytes` as the JSON of a comment route's body, `shape` its keys; else
/// exit 2 naming the defect.
fn decoded<T: serde::de::DeserializeOwned>(bytes: &[u8], shape: &str) -> Result<T, CliError> {
    serde_json::from_slice(bytes).map_err(|error| {
        escaped_error(CliError::spec(format!(
            "the comment's body is not `{{{shape}}}` as JSON: {error}; nothing changed"
        )))
    })
}

// ------------------------------------------------------------------ text

/// Who a line names: a person's words; an agent's `agent <role>` (`-` for
/// none) and ` for <login>` on a hub.
fn who(document: &CommentDocument) -> String {
    if document.author.author_type != AuthorType::Agent {
        return document.actor.clone();
    }
    let role = document.author.role.as_deref().unwrap_or("-");
    match &document.author.login {
        Some(login) => format!("agent {role} for {login}"),
        None => format!("agent {role}"),
    }
}

/// A comment's line (and body) as `list` prints it; `on` adds ` | on <ID>`
/// after the time (the mentions).
fn comment_text(out: &mut String, document: &CommentDocument, on: bool) {
    if let Some(deleted) = &document.deleted {
        let _ = writeln!(
            out,
            "{} | deleted by {} {}",
            document.id,
            escape_controls(&one_line(&deleted.by)),
            deleted.at
        );
        return;
    }
    let mut fields = vec![
        document.id.clone(),
        escape_controls(&one_line(&who(document))),
        document.created_at.clone(),
    ];
    if on {
        fields.push(format!(
            "on {}",
            escape_controls(&one_line(&document.on.id))
        ));
    }
    if document.version > 1 {
        fields.push(format!(
            "edited {}, v{}",
            document.updated_at, document.version
        ));
    }
    if let Some(reply) = &document.reply_to {
        fields.push(format!("reply to {reply}"));
    }
    if !document.items.is_empty() {
        let items: Vec<&str> = document.items.iter().map(|item| item.id.as_str()).collect();
        fields.push(format!("items {}", items.join(", ")));
    }
    let _ = writeln!(out, "{}", fields.join(" | "));
    for line in document.body.as_deref().unwrap_or_default().lines() {
        let _ = writeln!(out, "  {}", escape_controls(line));
    }
}

/// `list`'s text (task spec `comments`, "The comment document").
pub(crate) fn render_list(outcome: &CommentListOutcome) -> String {
    if outcome.refused.is_some() {
        return String::new();
    }
    let on = escape_controls(&one_line(&outcome.on.id));
    let mut out = if outcome.total == 0 {
        format!("# Comments on {on}: none\n")
    } else {
        format!("# Comments on {on}: {}; {PEOPLES_WORDS}\n", outcome.total)
    };
    for document in &outcome.comments {
        comment_text(&mut out, document, false);
    }
    if let Some(next) = &outcome.next_before {
        let _ = writeln!(out, "next: --before {next}");
    }
    out
}

/// `mentions`' text.
pub(crate) fn render_mentions(outcome: &MentionsOutcome) -> String {
    let whose = match &outcome.login {
        Some(login) => format!("Mentions of {login}"),
        None => "Mentions".to_owned(),
    };
    let mut out = if outcome.comments.is_empty() {
        format!("# {whose}: none\n")
    } else {
        format!("# {whose}: {}; {PEOPLES_WORDS}\n", outcome.comments.len())
    };
    for document in &outcome.comments {
        comment_text(&mut out, document, true);
    }
    if let Some(next) = &outcome.next_before {
        let _ = writeln!(out, "next: --before {next}");
    }
    out
}

/// A write's text: the comment as `list` prints it.
pub(crate) fn render_write(outcome: &CommentOutcome) -> String {
    let mut out = String::new();
    if let Some(document) = &outcome.comment {
        comment_text(&mut out, document, true);
    }
    out
}

/// A refusal's stderr line.
pub(crate) fn refusal_lines(refused: Option<&CommentRefused>) -> Vec<String> {
    refused
        .map(|refused| vec![format!("spec: {}", escape_controls(&refused.reason))])
        .unwrap_or_default()
}

// --------------------------------------------------------------- context

/// One comment of an agent's context (task spec `comments`, "The card
/// context and the task package"): `{id, reply_to, actor, author,
/// created_at, updated_at, version, body, cut}`, and `on` (its card) in a
/// task package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextComment {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    pub reply_to: Option<String>,
    pub actor: String,
    pub author: Author,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
    /// One line, cut at 1 024 characters, its control and bidirectional
    /// characters escaped.
    pub body: String,
    pub cut: bool,
}

/// `{total, listed}`: the cards' live comments counted, and those listed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContextComments {
    pub total: u64,
    pub listed: Vec<ContextComment>,
}

impl ContextComments {
    /// Its `context.comments:` line and each `context.comment:` line, in
    /// order: what the room `U` counts.
    pub(crate) fn lines(&self) -> Vec<String> {
        if self.total == 0 {
            return vec!["context.comments: -".to_owned()];
        }
        let mut lines = vec![format!(
            "context.comments: {} of {}, the newest; {PEOPLES_WORDS}",
            self.listed.len(),
            self.total
        )];
        lines.extend(self.listed.iter().map(context_line));
        lines
    }

    /// The note naming what is left out, when anything is (printed
    /// `note: context: 3 older comments not shown: list_comments on C-0007
    /// has them`).
    pub(crate) fn note(&self, on: &str) -> Option<String> {
        let left = self.total.saturating_sub(self.listed.len() as u64);
        (left > 0).then(|| {
            let what = if left == 1 { "comment" } else { "comments" };
            format!("{left} older {what} not shown: list_comments on {on} has them")
        })
    }
}

/// `context.comment: <ID> | <who> | <time> | <re CM|-> | <body>`, the
/// body escaped.
fn context_line(comment: &ContextComment) -> String {
    let who = if comment.author.author_type == AuthorType::Agent {
        let role = comment.author.role.as_deref().unwrap_or("-");
        match &comment.author.login {
            Some(login) => format!("agent {role} for {login}"),
            None => format!("agent {role}"),
        }
    } else {
        comment.actor.clone()
    };
    let reply = comment
        .reply_to
        .as_ref()
        .map_or_else(|| "-".to_owned(), |reply| format!("re {reply}"));
    let on = comment
        .on
        .as_ref()
        .map_or_else(String::new, |on| format!(" | on {on}"));
    format!(
        "context.comment: {} | {} | {}{on} | {reply} | {}",
        comment.id,
        escape_controls(&one_line(&who)),
        comment.created_at,
        comment.body
    )
}

/// The newest live comments of `cards` an agent's context carries: taken
/// newest first, at most 20, each body one line cut at 1 024 characters,
/// the `context.comment:` lines (each its characters + 1) within 8 192;
/// the first that does not fit and every older one left out; listed
/// oldest first. `with_on`: each names its card (a task package).
pub(crate) fn context_comments(
    queue: &SqliteQueue,
    cards: &[&str],
    with_on: bool,
) -> Result<(ContextComments, Vec<String>), CliError> {
    let read = queue.card_comment_context(cards).map_err(queue_cannot)?;
    let notes: Vec<String> = read
        .unreadable
        .iter()
        .map(|row: &UnreadableRow| unreadable_comment_line(row))
        .collect();
    let mut listed = Vec::new();
    let mut used = 0usize;
    for comment in &read.newest {
        let (body, cut) = context_body(comment.body.as_deref().unwrap_or_default());
        // Escaped in the agent's JSON too: no bidirectional or control
        // character of a person's words reaches an agent.
        let body = escape_controls(&body);
        let entry = ContextComment {
            id: comment.id.clone(),
            on: with_on.then(|| comment.on_id.clone()),
            reply_to: comment.reply_to.clone(),
            actor: comment.actor.clone(),
            author: comment.author.clone(),
            created_at: comment.created_at.clone(),
            updated_at: comment.updated_at.clone(),
            version: comment.version,
            body,
            cut,
        };
        let chars = context_line(&entry).chars().count() + 1;
        if used + chars > COMMENT_CONTEXT_CHARS {
            break;
        }
        used += chars;
        listed.push(entry);
    }
    listed.reverse();
    Ok((
        ContextComments {
            total: read.total,
            listed,
        },
        notes,
    ))
}
