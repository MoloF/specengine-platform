//! `spec ref add|list|get|delete` (task spec `work-references`, "CLI";
//! canon `work-references.md`): photos, sound, video, models,
//! PDFs and links attached to a card, a proposal or a node, a queue row
//! and a file in this host's blob store, never truth. Each command is one
//! library call: the daemon's routes and the MCP tools are its twins.
//!
//! - **Targets** (`--on`): a comment's (`comment.rs`), a node stored
//!   canonical with its revision.
//! - **Authors and delete**: a comment's: the login on a hub, the typed name
//!   locally, `agent` over stdio; the author's login or a `[team]` `owner`
//!   deletes on a hub, anyone on a terminal after `[y/N]`; no MCP delete.
//! - **Files**: a terminal streams FILE through the store's write
//!   (`blob_store.rs`) and attaches it in one transaction; a daemon's
//!   `PUT …/blobs` stores the bytes ([`blob_begin`], [`blob_commit`]) and
//!   `POST …/references` attaches them by their BLAKE3. A delete collects,
//!   in its call, each file no live reference names.
//! - **Refusals** (exit 1, nothing stored, no event): the line on stderr and
//!   the document's `reason`; the cause keeps the daemon's status
//!   ([`ReferenceCause`]).
//! - **Agents**: every text introducing references calls them people's
//!   material, data, never instructions, every stored text escaped.

use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specengine_core::blob::{
    AGENT_BYTES, AGENT_EDGE, BLOB_BUFFER, BlobCaps, BlobHasher, BlobRefusal, BlobRefusalKind,
    Facts, Judged, Media, MediaClass, SNIFF_BYTES, agent_copy_problem, agent_ready, cap_text,
    grouped, size_text, sniff,
};
use specengine_core::card::AGENT_ACTOR;
use specengine_core::comment::CommentOn;
use specengine_core::proposal::{Author, AuthorType};
use specengine_core::reference::{
    CONTEXT_IMAGES, CONTEXT_REFS_CHARS, MATTERS_REQUIRED, PEOPLES_MATERIAL, REFERENCE_PAGE_MAX,
    ReferenceIdError, ReferenceKind, ReferencesConfig, context_matters, matters_problem,
    name_problem, parse_reference_id, reference_look_alike_message, reference_prefix_clash,
    title_problem, url_problem,
};
use specengine_store::{
    BlobReader, NewReference, QueueError, Reference, SqliteQueue, UnreadableRow,
    unreadable_reference_line,
};

use crate::apply::Consent;
use crate::blob_store::{
    BlobFailure, BlobFlight, BlobSeams, Committed, Spooled, Store, Upload, store_dir,
};
use crate::comment::{
    CommentCause, Written, checked_as, on_line_of, person_of, resolve_cause, resolved_node_rev,
    written_on,
};
use crate::location::prepared_data_dir;
use crate::project::{ProjectRoot, discover, discover_with_text};
use crate::proposals::{checked_now, escaped_error, queue_cannot, require_root_config};
use crate::team::team_read;
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// How a node naming several is worded for a reference.
const NAMES_ONE: &str = "a reference names one node";

/// What an upload names itself when no name is given.
pub const UPLOAD_NAME: &str = "the upload";

// ---------------------------------------------------------------- requests

/// `spec ref add` options; the daemon's `POST …/references`, MCP's
/// `add_reference`.
#[derive(Debug, Clone)]
pub struct ReferenceAddRequest {
    /// `--on X` as written.
    pub on: String,
    /// A stored file's BLAKE3 (the daemon's `b3`; a terminal's FILE once
    /// stored).
    pub b3: Option<String>,
    /// Its agent copy's.
    pub agent_b3: Option<String>,
    /// `--url U`: a link.
    pub url: Option<String>,
    /// `--name N`: a file's name as picked.
    pub name: Option<String>,
    pub title: Option<String>,
    pub matters: Option<String>,
    /// `--as N`: a person; `None` `local` (or a hub's login).
    pub actor: Option<String>,
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// An agent whatever the three hold: MCP's `add_reference`.
    pub agent: bool,
    /// A hub's login: the words and the author's `login`.
    pub login: Option<String>,
    /// The files judged before the turn (a daemon's [`reference_prepare`]);
    /// `None`: judged in the call.
    pub prepared: Option<Prepared>,
    /// The injected clock.
    pub now: String,
}

/// `spec ref add FILE`: the file and its agent copy streamed through the
/// store, then [`ReferenceAddRequest`] attaching them.
#[derive(Debug, Clone)]
pub struct ReferenceFileRequest {
    /// FILE, relative to the current directory.
    pub file: PathBuf,
    /// `--agent-copy SMALL`.
    pub agent_copy: Option<PathBuf>,
    pub add: ReferenceAddRequest,
    pub caps: BlobCaps,
    pub seams: BlobSeams,
}

/// `spec ref list` options; `GET …/references`, `list_references`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceListRequest {
    pub on: String,
    /// `--before RF`: any well-formed reference ID, present or not.
    pub before: Option<String>,
    /// `--limit N`: 1 to 100; `None` 100.
    pub limit: Option<u64>,
}

/// `spec ref get` options; `get_reference`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceGetRequest {
    pub id: String,
    /// `--out F`: write its file to a new F.
    pub out: Option<PathBuf>,
    /// `--agent-copy` with `--out`: its agent copy instead.
    pub agent_copy: bool,
    /// MCP's: read its agent-ready image, base64 once.
    pub image: bool,
}

/// `spec ref delete` options; `DELETE …/references/<id>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDeleteRequest {
    pub id: String,
    pub actor: Option<String>,
    /// A hub's login: the author's login, or a `[team]` `owner`, deletes.
    pub login: Option<String>,
    pub now: String,
}

/// The files a `POST …/references` names, judged before the turn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prepared {
    pub file: Option<Judged>,
    pub copy: Option<Judged>,
}

// ---------------------------------------------------------------- outcomes

/// Why a reference command was refused, for the daemon's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCause {
    /// A body refused: a cap, a missing "what matters", an `agent_b3`
    /// rule, a short body: a 400.
    Body,
    /// No such reference, target or blob: a 404.
    Unknown,
    /// A rule: a deleted reference, a node in another form: a 409.
    Refused,
    /// Not the person to make it: a 403.
    Forbidden,
    /// A cap, the quota, the file count, the floor: a 413.
    TooLarge,
    /// A type refused, or outside `allow`: a 415.
    Type,
}

impl From<CommentCause> for ReferenceCause {
    fn from(cause: CommentCause) -> Self {
        match cause {
            CommentCause::Body => Self::Body,
            CommentCause::Unknown => Self::Unknown,
            CommentCause::Refused => Self::Refused,
            CommentCause::Forbidden => Self::Forbidden,
        }
    }
}

impl From<BlobRefusalKind> for ReferenceCause {
    fn from(kind: BlobRefusalKind) -> Self {
        match kind {
            BlobRefusalKind::TooLarge => Self::TooLarge,
            BlobRefusalKind::Type => Self::Type,
            BlobRefusalKind::Body => Self::Body,
        }
    }
}

/// A refusal: `{id, reason}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceRefused {
    pub id: Option<String>,
    pub reason: String,
    #[serde(skip)]
    pub cause: ReferenceCause,
}

/// `on`: `{kind, id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceOnJson {
    pub kind: &'static str,
    pub id: String,
}

/// `deleted`: `{by, at}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceDeletedJson {
    pub by: String,
    pub at: String,
}

/// The reference document, 21 keys (task spec `work-references`,
/// "Documents").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReferenceDocument {
    pub id: String,
    pub on: ReferenceOnJson,
    pub on_rev: Option<u64>,
    pub actor: String,
    pub author: Author,
    pub kind: &'static str,
    pub title: Option<String>,
    pub matters: Option<String>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub b3: Option<String>,
    pub agent_b3: Option<String>,
    pub size: Option<u64>,
    pub media: Option<&'static str>,
    pub class: Option<&'static str>,
    pub inline: Option<bool>,
    pub meta: Option<Facts>,
    /// `stored` or `lost`; `null` for a link or a deleted reference.
    pub file: Option<&'static str>,
    pub warning: Option<&'static str>,
    pub created_at: String,
    pub deleted: Option<ReferenceDeletedJson>,
    /// What reaches an agent (text only, never JSON).
    #[serde(skip)]
    agent: AgentBytes,
}

/// What of a reference reaches an agent as an image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum AgentBytes {
    /// The file as it is: agent-ready and stored.
    Original {
        b3: String,
        media: Media,
    },
    /// Its agent copy, stored.
    Copy {
        b3: String,
    },
    /// None: why, for an image; `None` for any other.
    #[default]
    Nothing,
    NoImage(String),
}

impl AgentBytes {
    fn some(&self) -> bool {
        matches!(self, Self::Original { .. } | Self::Copy { .. })
    }
}

impl ReferenceDocument {
    fn of(reference: &Reference, store: &Store) -> Self {
        let live = reference.deleted.is_none();
        let file = match (reference.kind, &reference.b3) {
            (ReferenceKind::File, Some(b3)) if live => {
                Some(if store.holds(b3) { "stored" } else { "lost" })
            }
            _ => None,
        };
        let agent = if live && reference.kind == ReferenceKind::File {
            agent_bytes(reference, store, file == Some("stored"))
        } else {
            AgentBytes::Nothing
        };
        Self {
            id: reference.id.clone(),
            on: ReferenceOnJson {
                kind: reference.on.as_str(),
                id: reference.on_id.clone(),
            },
            on_rev: reference.on_rev,
            actor: reference.actor.clone(),
            author: reference.author.clone(),
            kind: reference.kind.as_str(),
            title: reference.title.clone(),
            matters: reference.matters.clone(),
            url: reference.url.clone(),
            name: reference.name.clone(),
            b3: reference.b3.clone(),
            agent_b3: reference.agent_b3.clone(),
            size: reference.size,
            media: reference.media.map(Media::as_str),
            class: reference.media.map(|media| media.class().as_str()),
            inline: reference.media.map(Media::inline),
            meta: if reference.kind == ReferenceKind::Link || !live {
                None
            } else {
                reference.meta
            },
            file,
            warning: reference.media.and_then(Media::warning),
            created_at: reference.created_at.clone(),
            deleted: reference
                .deleted
                .as_ref()
                .map(|deleted| ReferenceDeletedJson {
                    by: deleted.by.clone(),
                    at: deleted.at.clone(),
                }),
            agent,
        }
    }
}

/// What of `reference`'s file reaches an agent: the file itself when it
/// is an agent-ready image held here, else its stored agent copy, else
/// why none.
fn agent_bytes(reference: &Reference, store: &Store, stored: bool) -> AgentBytes {
    let (Some(media), Some(b3), Some(size)) = (reference.media, &reference.b3, reference.size)
    else {
        return AgentBytes::Nothing;
    };
    if media.class() != MediaClass::Image {
        return AgentBytes::Nothing;
    }
    let facts = reference.meta.unwrap_or_default();
    if agent_ready(media, &facts, size) {
        return if stored {
            AgentBytes::Original {
                b3: b3.clone(),
                media,
            }
        } else {
            AgentBytes::NoImage("its file is not on this host".to_owned())
        };
    }
    if let Some(copy) = &reference.agent_b3 {
        return if store.holds(copy) {
            AgentBytes::Copy { b3: copy.clone() }
        } else {
            AgentBytes::NoImage("its agent copy is not on this host".to_owned())
        };
    }
    let why = match (facts.width, facts.height) {
        (Some(width), Some(height)) if width.max(height) > AGENT_EDGE => format!(
            "{width}x{height} is over {} px and no smaller copy was made",
            grouped(AGENT_EDGE)
        ),
        (Some(_), Some(_)) => format!(
            "{} is over {} and no smaller copy was made",
            size_text(size),
            cap_text(AGENT_BYTES)
        ),
        _ => "its size could not be read and no smaller copy was made".to_owned(),
    };
    AgentBytes::NoImage(why)
}

/// An image for an agent: its bytes base64-encoded once, at most
/// [`AGENT_BYTES`] of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentImage {
    pub id: String,
    pub media: &'static str,
    pub data: String,
}

/// What `add`, `get` and `delete` answered: the reference, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceOutcome {
    pub reference: Option<ReferenceDocument>,
    pub refused: Option<ReferenceRefused>,
    /// `added`, `shown` or `deleted`.
    pub verb: &'static str,
    pub messages: Vec<Message>,
    /// `get`'s `  image: none for agents: …` line's reason.
    pub image_note: Option<String>,
    /// `get --out F`: the file written.
    pub written: Option<String>,
    /// `get` for an agent: the image, never in the JSON.
    pub images: Vec<AgentImage>,
}

impl ReferenceOutcome {
    pub fn exit(&self) -> Exit {
        if self.refused.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    /// Why it was refused, for the daemon; `None` when done.
    pub fn cause(&self) -> Option<ReferenceCause> {
        self.refused.as_ref().map(|refused| refused.cause)
    }

    fn refused(
        id: Option<&str>,
        reason: impl Into<String>,
        cause: ReferenceCause,
        verb: &'static str,
    ) -> Self {
        Self {
            reference: None,
            refused: Some(ReferenceRefused {
                id: id.map(str::to_owned),
                reason: one_line(&reason.into()),
                cause,
            }),
            verb,
            messages: Vec::new(),
            image_note: None,
            written: None,
            images: Vec::new(),
        }
    }

    fn done(document: ReferenceDocument, verb: &'static str, messages: Vec<Message>) -> Self {
        Self {
            reference: Some(document),
            refused: None,
            verb,
            messages,
            image_note: None,
            written: None,
            images: Vec::new(),
        }
    }
}

impl Serialize for ReferenceOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match (&self.reference, &self.refused) {
            (Some(reference), _) => reference.serialize(serializer),
            (None, Some(refused)) => refused.serialize(serializer),
            (None, None) => serializer.serialize_none(),
        }
    }
}

/// What `list` answered: `{on, total, references, next_before, notes}`, or
/// why there is no such target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceListOutcome {
    pub on: ReferenceOnJson,
    pub total: u64,
    pub references: Vec<ReferenceDocument>,
    pub next_before: Option<String>,
    pub notes: Vec<String>,
    pub refused: Option<ReferenceRefused>,
    pub messages: Vec<Message>,
}

#[derive(Serialize)]
struct ReferenceListJson<'a> {
    on: &'a ReferenceOnJson,
    total: u64,
    references: &'a [ReferenceDocument],
    next_before: Option<&'a str>,
    notes: &'a [String],
}

impl ReferenceListOutcome {
    pub fn exit(&self) -> Exit {
        if self.refused.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    pub fn cause(&self) -> Option<ReferenceCause> {
        self.refused.as_ref().map(|refused| refused.cause)
    }
}

impl Serialize for ReferenceListOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.refused {
            Some(refused) => refused.serialize(serializer),
            None => ReferenceListJson {
                on: &self.on,
                total: self.total,
                references: &self.references,
                next_before: self.next_before.as_deref(),
                notes: &self.notes,
            }
            .serialize(serializer),
        }
    }
}

/// A stored blob, 10 keys (`PUT …/blobs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlobStoredDocument {
    pub b3: String,
    pub size: u64,
    pub media: &'static str,
    pub class: &'static str,
    pub inline: bool,
    pub meta: Facts,
    pub agent_ready: bool,
    pub needs_matters: bool,
    pub warning: Option<&'static str>,
    /// `new` or `existing`.
    pub stored: &'static str,
}

impl BlobStoredDocument {
    fn of(committed: &Committed) -> Self {
        let media = committed.judged.media;
        Self {
            b3: committed.b3.clone(),
            size: committed.judged.size,
            media: media.as_str(),
            class: media.class().as_str(),
            inline: media.inline(),
            meta: committed.judged.facts,
            agent_ready: agent_ready(media, &committed.judged.facts, committed.judged.size),
            needs_matters: media.class().needs_matters(),
            warning: media.warning(),
            stored: committed.stored.as_str(),
        }
    }
}

/// What a `PUT …/blobs` answered: the stored blob, or why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobOutcome {
    pub stored: Option<BlobStoredDocument>,
    pub refused: Option<ReferenceRefused>,
}

impl BlobOutcome {
    pub fn exit(&self) -> Exit {
        if self.refused.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    pub fn cause(&self) -> Option<ReferenceCause> {
        self.refused.as_ref().map(|refused| refused.cause)
    }

    fn refusal(refusal: &BlobRefusal) -> Self {
        Self {
            stored: None,
            refused: Some(ReferenceRefused {
                id: None,
                reason: one_line(&refusal.line),
                cause: refusal.kind.into(),
            }),
        }
    }
}

impl Serialize for BlobOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match (&self.stored, &self.refused) {
            (Some(stored), _) => stored.serialize(serializer),
            (None, Some(refused)) => refused.serialize(serializer),
            (None, None) => serializer.serialize_none(),
        }
    }
}

// ------------------------------------------------------------------- open

/// The project, its queue and its store for a reference command, git never
/// asked: `--config` but the root's own and `RF` taken by `[ids]` exit 2.
struct OpenReferences {
    project: ProjectRoot,
    queue: SqliteQueue,
    store: Store,
}

/// The project and the clash `[ids]` may hold with `RF`, judged first.
fn found(env: &Env, globals: &Globals) -> Result<(ProjectRoot, String), CliError> {
    let project = discover(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    if let Some(clash) = reference_prefix_clash(&project.config.scheme) {
        return Err(CliError::spec(format!("{}: {clash}", project.config_label)));
    }
    Ok((project, slug))
}

fn open_references(
    env: &Env,
    globals: &Globals,
    caps: BlobCaps,
    seams: BlobSeams,
) -> Result<OpenReferences, CliError> {
    let (project, slug) = found(env, globals)?;
    let data_dir = prepared_data_dir(env, &project)?;
    let queue =
        SqliteQueue::open(data_dir.join(format!("{slug}.db")), &slug).map_err(queue_cannot)?;
    let store = Store::new(store_dir(&data_dir, &slug), caps, seams);
    Ok(OpenReferences {
        project,
        queue,
        store,
    })
}

/// `[references] allow`, read now: a bad table is exit 2 with its line,
/// refusing the writes that store a file (task spec `work-references` D21).
fn references_allow(env: &Env, globals: &Globals) -> Result<Option<ReferencesConfig>, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    ReferencesConfig::from_toml(&text)
        .map_err(|error| escaped_error(crate::project::config_error(&project.config_label, &error)))
}

/// A reference ID as written: a look-alike exits 2; none in any script is
/// `None`.
fn written_reference(field: &str, written: &str) -> Result<Option<String>, CliError> {
    match parse_reference_id(written) {
        Ok(id) => Ok(Some(id)),
        Err(ReferenceIdError::LookAlike { fix }) => Err(CliError::spec(format!(
            "{field}{}",
            reference_look_alike_message(written.trim(), &fix)
        ))),
        Err(ReferenceIdError::NotAnId) => Ok(None),
    }
}

fn not_a_reference(written: &str) -> String {
    format!(
        "no reference `{}`: a reference ID is `RF-` and 4 or more digits",
        written.trim()
    )
}

/// `--before RF`: a well-formed reference ID, else exit 2.
fn before_of(written: Option<&str>) -> Result<Option<String>, CliError> {
    let Some(written) = written else {
        return Ok(None);
    };
    match written_reference("--before: ", written)? {
        Some(id) => Ok(Some(id)),
        None => Err(CliError::spec(format!(
            "--before: {}",
            not_a_reference(written)
        ))),
    }
}

/// `--limit N`: 1 to 100, else exit 2; `None` 100.
fn limit_of(written: Option<u64>) -> Result<usize, CliError> {
    match written {
        None => Ok(REFERENCE_PAGE_MAX),
        Some(limit) if (1..=REFERENCE_PAGE_MAX as u64).contains(&limit) => {
            Ok(usize::try_from(limit).unwrap_or(REFERENCE_PAGE_MAX))
        }
        Some(limit) => Err(CliError::spec(format!(
            "--limit: {limit}; 1 to {REFERENCE_PAGE_MAX}"
        ))),
    }
}

/// Why a list's query is no query `GET …/references` takes (a 400, nothing
/// run): `before` no reference ID, `limit` outside 1 to 100.
pub fn reference_query_problem(before: Option<&str>, limit: Option<i64>) -> Option<String> {
    let limit = match limit {
        None => None,
        Some(limit) => match u64::try_from(limit) {
            Ok(limit) => Some(limit),
            Err(_) => return Some(format!("limit: {limit}; 1 to {REFERENCE_PAGE_MAX}")),
        },
    };
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

/// A target resolved: its kind, canonical ID and a node's revision.
type Target = (CommentOn, String, Option<u64>);

/// A target as written, resolved: a card or a proposal as written (the
/// store checks it again), a node over the index; else the refusal.
fn resolved_target(
    env: &Env,
    context: &OpenReferences,
    written: &str,
    messages: &mut Vec<Message>,
) -> Result<Result<Target, Box<ReferenceOutcome>>, CliError> {
    Ok(match written_on(&context.project, written)? {
        Written::Card(id) => Ok((CommentOn::Card, id, None)),
        Written::Proposal(id) => Ok((CommentOn::Proposal, id, None)),
        Written::Node(written) => {
            match resolved_node_rev(env, &context.project, &written, messages)? {
                Ok((id, rev)) => Ok((CommentOn::Node, id, rev)),
                Err(unresolved) => Err(Box::new(ReferenceOutcome::refused(
                    Some(&written),
                    on_line_of(&unresolved, NAMES_ONE),
                    resolve_cause(&unresolved).into(),
                    "added",
                ))),
            }
        }
    })
}

/// A card's or a proposal's existence, read now (before a file streams; the
/// store reads it again under its lock).
fn target_exists(queue: &SqliteQueue, on: CommentOn, id: &str) -> Result<bool, CliError> {
    Ok(match on {
        CommentOn::Card => queue.get_card(id).map_err(queue_cannot)?.is_some(),
        CommentOn::Proposal => {
            use specengine_store::ProposalQueue as _;
            match queue.get(id) {
                Ok(found) => found.is_some(),
                Err(QueueError::Store(error)) => return Err(CliError::spec(error)),
                Err(_) => true,
            }
        }
        CommentOn::Node => true,
    })
}

/// A store refusal as an outcome; anything else exit 2.
fn store_refusal(
    error: QueueError,
    verb: &'static str,
    fallback: &str,
) -> Result<ReferenceOutcome, CliError> {
    match error {
        QueueError::ReferenceRefused { id, reason, cause } => {
            let id = if id.is_empty() { fallback } else { id.as_str() };
            Ok(ReferenceOutcome::refused(
                Some(id),
                reason,
                CommentCause::from(cause).into(),
                verb,
            ))
        }
        error => Err(queue_cannot(error)),
    }
}

/// The author and words of an add (as a comment's).
fn author_of(request: &ReferenceAddRequest) -> Result<(Author, String), CliError> {
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
        (None, true) => AGENT_ACTOR.to_owned(),
        (None, false) => person_of(request.actor.as_deref())?,
    };
    Ok((author, actor))
}

/// The texts' and the shape's rules, before any file or target is read:
/// exactly one of a file and a link; `agent_b3` and `name` with a file
/// alone; each text's caps. `None`: they pass.
fn shape_problem(request: &ReferenceAddRequest) -> Option<String> {
    match (&request.b3, &request.url) {
        (Some(_), Some(_)) => return Some("give a file or a url, not both".to_owned()),
        (None, None) => return Some("give a file (b3) or a url".to_owned()),
        _ => {}
    }
    if request.url.is_some() && (request.agent_b3.is_some() || request.name.is_some()) {
        return Some("agent_b3 and name go with a file alone".to_owned());
    }
    if let Some(b3) = &request.b3
        && !specengine_core::blob::is_b3(b3)
    {
        return Some("b3: 64 lower-case hex digits".to_owned());
    }
    if let Some(copy) = &request.agent_b3
        && !specengine_core::blob::is_b3(copy)
    {
        return Some("agent_b3: 64 lower-case hex digits".to_owned());
    }
    request
        .title
        .as_deref()
        .and_then(title_problem)
        // A blank one is judged with the file's class ([`blank_matters`]).
        .or_else(|| {
            request
                .matters
                .as_deref()
                .filter(|matters| given(Some(matters)))
                .and_then(matters_problem)
        })
        .or_else(|| request.url.as_deref().and_then(url_problem))
        .or_else(|| request.name.as_deref().and_then(name_problem))
}

/// "What matters" was given: present and not blank.
fn given(matters: Option<&str>) -> bool {
    matters.is_some_and(|matters| !matters.trim().is_empty())
}

/// A blank "what matters" where its class does not require one (an image,
/// a link): refused as blank; where it does, the class's line says it is
/// required ([`MATTERS_REQUIRED`]).
fn blank_matters(matters: Option<&str>) -> Option<String> {
    matters
        .filter(|matters| matters.trim().is_empty())
        .and_then(matters_problem)
}

/// Why `file` (and `copy`) cannot be attached as given: "what matters"
/// missing for its class, an `agent_b3` rule. `None`: they can.
fn file_problem(
    file: &Judged,
    copy: Option<&Judged>,
    name: Option<&str>,
    matters: Option<&str>,
    allow: Option<&ReferencesConfig>,
) -> Option<(String, ReferenceCause)> {
    if let Some(allow) = allow
        && !allow.allows(file.media)
    {
        return Some((
            format!(
                "refused: {}: {} is not allowed in this project ([references] allow)",
                name.unwrap_or(UPLOAD_NAME),
                file.media.as_str()
            ),
            ReferenceCause::Type,
        ));
    }
    if file.media.class().needs_matters() && !given(matters) {
        return Some((MATTERS_REQUIRED.to_owned(), ReferenceCause::Body));
    }
    let copy = copy?;
    agent_copy_problem(file, copy).map(|problem| (problem, ReferenceCause::Body))
}

/// The files of `b3` and `agent_b3` judged from the store now (a daemon's
/// `POST`, before its turn): `None` each where the store holds no such
/// file.
pub fn reference_prepare(
    env: &Env,
    globals: &Globals,
    b3: Option<&str>,
    agent_b3: Option<&str>,
) -> Result<Prepared, CliError> {
    let (project, slug) = found(env, globals)?;
    let data_dir = crate::location::checked_data_dir(env, &project)?;
    let store = Store::new(
        store_dir(&data_dir, &slug),
        BlobCaps::default(),
        BlobSeams::default(),
    );
    Ok(Prepared {
        file: b3.and_then(|b3| store.judge(b3)),
        copy: agent_b3.and_then(|copy| store.judge(copy)),
    })
}

/// `spec ref add --url`, `POST …/references`, `add_reference`: one
/// reference attached (see the module documentation), `reference.created`
/// logged, nothing else written.
pub fn reference_add(
    env: &Env,
    globals: &Globals,
    request: &ReferenceAddRequest,
) -> Result<ReferenceOutcome, CliError> {
    run_add(env, globals, request).map_err(escaped_error)
}

fn run_add(
    env: &Env,
    globals: &Globals,
    request: &ReferenceAddRequest,
) -> Result<ReferenceOutcome, CliError> {
    const VERB: &str = "added";
    let now = checked_now(&request.now)?;
    let (author, actor) = author_of(request)?;
    let mut context = open_references(env, globals, BlobCaps::default(), BlobSeams::default())?;
    // Every add reads `[references]`: a bad table refuses it (exit 2).
    let allow = references_allow(env, globals)?;
    if let Some(problem) = shape_problem(request) {
        return Ok(ReferenceOutcome::refused(
            None,
            problem,
            ReferenceCause::Body,
            VERB,
        ));
    }
    let mut messages = Vec::new();
    let (on, on_id, on_rev) = match resolved_target(env, &context, &request.on, &mut messages)? {
        Ok(target) => target,
        Err(mut refused) => {
            refused.messages = messages;
            return Ok(*refused);
        }
    };
    let (file, copy) = match &request.b3 {
        None => (None, None),
        Some(b3) => {
            let prepared = request.prepared.clone().unwrap_or_else(|| Prepared {
                file: context.store.judge(b3),
                copy: request
                    .agent_b3
                    .as_deref()
                    .and_then(|copy| context.store.judge(copy)),
            });
            // Read again here: a delete may have collected it meanwhile.
            let held = |hash: &str, judged: &Option<Judged>| {
                judged.as_ref().is_some_and(|judged| {
                    context
                        .store
                        .open(hash)
                        .is_some_and(|file| file.size == judged.size)
                })
            };
            if !held(b3, &prepared.file) {
                return Ok(ReferenceOutcome::refused(
                    Some(b3),
                    format!("blob {b3} is not stored here: upload it again"),
                    ReferenceCause::Unknown,
                    VERB,
                ));
            }
            if let Some(copy) = &request.agent_b3
                && !held(copy, &prepared.copy)
            {
                return Ok(ReferenceOutcome::refused(
                    Some(copy),
                    format!("blob {copy} is not stored here: upload it again"),
                    ReferenceCause::Unknown,
                    VERB,
                ));
            }
            let (Some(file), copy) = (prepared.file, prepared.copy) else {
                return Ok(ReferenceOutcome::refused(
                    Some(b3),
                    format!("blob {b3} is not stored here: upload it again"),
                    ReferenceCause::Unknown,
                    VERB,
                ));
            };
            if let Some((problem, cause)) = file_problem(
                &file,
                copy.as_ref(),
                request.name.as_deref(),
                request.matters.as_deref(),
                allow.as_ref(),
            ) {
                return Ok(ReferenceOutcome::refused(None, problem, cause, VERB));
            }
            (Some(file), copy)
        }
    };
    if let Some(problem) = blank_matters(request.matters.as_deref()) {
        return Ok(ReferenceOutcome::refused(
            None,
            problem,
            ReferenceCause::Body,
            VERB,
        ));
    }
    let new = NewReference {
        on,
        on_id: on_id.clone(),
        on_rev,
        actor,
        author,
        kind: if request.url.is_some() {
            ReferenceKind::Link
        } else {
            ReferenceKind::File
        },
        title: request.title.clone(),
        matters: request.matters.clone(),
        url: request.url.clone(),
        name: request.b3.as_ref().and(request.name.clone()),
        b3: request.b3.clone(),
        agent_b3: copy.as_ref().and(request.agent_b3.clone()),
        size: file.as_ref().map(|file| file.size),
        media: file.as_ref().map(|file| file.media),
        meta: file.as_ref().map(|file| file.facts),
    };
    match context.queue.add_reference(&new, now) {
        Ok(reference) => Ok(ReferenceOutcome::done(
            ReferenceDocument::of(&reference, &context.store),
            VERB,
            messages,
        )),
        Err(error) => {
            let mut outcome = store_refusal(error, VERB, &on_id)?;
            outcome.messages = messages;
            Ok(outcome)
        }
    }
}

/// `spec ref add FILE`: the target, the texts and the file's type judged
/// first (nothing stored on a refusal); then FILE (and `--agent-copy`)
/// streamed through the store's write and attached in one transaction.
pub fn reference_add_file(
    env: &Env,
    globals: &Globals,
    request: &ReferenceFileRequest,
) -> Result<ReferenceOutcome, CliError> {
    run_add_file(env, globals, request).map_err(escaped_error)
}

fn run_add_file(
    env: &Env,
    globals: &Globals,
    request: &ReferenceFileRequest,
) -> Result<ReferenceOutcome, CliError> {
    const VERB: &str = "added";
    checked_now(&request.add.now)?;
    author_of(&request.add)?;
    let context = open_references(env, globals, request.caps, request.seams.clone())?;
    let allow = references_allow(env, globals)?;
    let path = env.cwd.join(&request.file);
    let name = match &request.add.name {
        Some(name) => name.clone(),
        None => request
            .file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let mut add = request.add.clone();
    add.name = Some(name.clone());
    // The shape with a stand-in hash: the file is not stored yet.
    let mut probe = add.clone();
    probe.b3 = Some("0".repeat(64));
    probe.agent_b3 = request.agent_copy.as_ref().map(|_| "0".repeat(64));
    if let Some(problem) = shape_problem(&probe) {
        return Ok(ReferenceOutcome::refused(
            None,
            problem,
            ReferenceCause::Body,
            VERB,
        ));
    }
    let mut messages = Vec::new();
    let (on, on_id, _) = match resolved_target(env, &context, &add.on, &mut messages)? {
        Ok(target) => target,
        Err(mut refused) => {
            refused.messages = messages;
            return Ok(*refused);
        }
    };
    if !target_exists(&context.queue, on, &on_id)? {
        let mut refused = ReferenceOutcome::refused(
            Some(&on_id),
            format!("on: no {} {on_id}", on.as_str()),
            ReferenceCause::Unknown,
            VERB,
        );
        refused.messages = messages;
        return Ok(refused);
    }
    // The type, the cap, "what matters" and the agent copy's rules judged
    // on the local files before a byte is stored.
    let judged = match judged_local(&path, &request.file, &name, &request.caps)? {
        Ok(judged) => judged,
        Err(refusal) => return Ok(blob_refused(&refusal, VERB)),
    };
    let media = judged.media;
    if let Some(allow) = &allow
        && !allow.allows(media)
    {
        return Ok(ReferenceOutcome::refused(
            None,
            format!(
                "refused: {name}: {} is not allowed in this project ([references] allow)",
                media.as_str()
            ),
            ReferenceCause::Type,
            VERB,
        ));
    }
    if media.class().needs_matters() && !given(add.matters.as_deref()) {
        return Ok(ReferenceOutcome::refused(
            None,
            MATTERS_REQUIRED,
            ReferenceCause::Body,
            VERB,
        ));
    }
    if let Some(problem) = blank_matters(add.matters.as_deref()) {
        return Ok(ReferenceOutcome::refused(
            None,
            problem,
            ReferenceCause::Body,
            VERB,
        ));
    }
    let copy_name = request.agent_copy.as_ref().map(|copy_path| {
        copy_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    if let (Some(copy_path), Some(copy_name)) = (&request.agent_copy, &copy_name) {
        let copy = match judged_local(
            &env.cwd.join(copy_path),
            copy_path,
            copy_name,
            &request.caps,
        )? {
            Ok(copy) => copy,
            Err(refusal) => return Ok(blob_refused(&refusal, VERB)),
        };
        if let Some(problem) = agent_copy_problem(&judged, &copy) {
            return Ok(ReferenceOutcome::refused(
                None,
                problem,
                ReferenceCause::Body,
                VERB,
            ));
        }
    }
    let copy = match (&request.agent_copy, &copy_name) {
        (Some(copy_path), Some(copy_name)) => {
            match store_file(&context, env, copy_path, copy_name, allow.clone())? {
                Ok(committed) => Some(committed),
                Err(refusal) => return Ok(blob_refused(&refusal, VERB)),
            }
        }
        _ => None,
    };
    let file = match store_file(&context, env, &request.file, &name, allow)? {
        Ok(committed) => committed,
        Err(refusal) => return Ok(blob_refused(&refusal, VERB)),
    };
    add.b3 = Some(file.b3.clone());
    add.agent_b3 = copy.as_ref().map(|copy| copy.b3.clone());
    add.prepared = Some(Prepared {
        file: Some(file.judged),
        copy: copy.map(|copy| copy.judged),
    });
    drop(context);
    // The attach resolves its target again (its own notes, once).
    drop(messages);
    run_add(env, globals, &add)
}

/// A refusal of the store's write as an outcome.
fn blob_refused(refusal: &BlobRefusal, verb: &'static str) -> ReferenceOutcome {
    ReferenceOutcome::refused(None, refusal.line.clone(), refusal.kind.into(), verb)
}

/// A local file judged as the store would: its type from its first bytes,
/// the file cap on its size, its facts; an unreadable file exits 2.
fn judged_local(
    path: &Path,
    shown: &Path,
    name: &str,
    caps: &BlobCaps,
) -> Result<Result<Judged, BlobRefusal>, CliError> {
    let cannot =
        |error: std::io::Error| CliError::cannot(format!("spec: {}: {error}", shown.display()));
    let mut file = File::open(path).map_err(cannot)?;
    let size = file.metadata().map_err(cannot)?.len();
    let mut head = Vec::with_capacity(SNIFF_BYTES);
    (&mut file)
        .take(SNIFF_BYTES as u64)
        .read_to_end(&mut head)
        .map_err(cannot)?;
    let media = match sniff(&head) {
        Ok(media) => media,
        Err(refused) => {
            return Ok(Err(BlobRefusal {
                kind: BlobRefusalKind::Type,
                line: refused.line(name),
            }));
        }
    };
    if let Some(refusal) = caps.file_refusal(name, size) {
        return Ok(Err(refusal));
    }
    Ok(Ok(Judged {
        media,
        facts: specengine_core::blob::facts(media, &mut file, size),
        size,
    }))
}

/// One file of a terminal streamed through the store's write: begun with
/// its size, read in 64 KiB pieces, put in place, swept when due.
fn store_file(
    context: &OpenReferences,
    env: &Env,
    file: &Path,
    name: &str,
    allow: Option<ReferencesConfig>,
) -> Result<Result<Committed, BlobRefusal>, CliError> {
    let shown = file.display().to_string();
    let cannot = |error: std::io::Error| CliError::cannot(format!("spec: {shown}: {error}"));
    let mut source = File::open(env.cwd.join(file)).map_err(cannot)?;
    let size = source.metadata().map_err(cannot)?.len();
    let flight = BlobFlight::default();
    let failed = |failure: BlobFailure| match failure {
        BlobFailure::Refused(refusal) => Ok(Err(refusal)),
        BlobFailure::Cannot(line) => Err(CliError::spec(line)),
    };
    let mut upload: Upload =
        match context
            .store
            .begin(name, size, allow, &flight, Some(&context.queue))
        {
            Ok(upload) => upload,
            Err(failure) => return failed(failure),
        };
    let mut buffer = vec![0u8; BLOB_BUFFER];
    loop {
        let read = source.read(&mut buffer).map_err(cannot)?;
        if read == 0 {
            break;
        }
        if let Err(failure) = upload.write(&buffer[..read]) {
            return failed(failure);
        }
    }
    let staged: Spooled = match upload.finish() {
        Ok(staged) => staged,
        Err(failure) => return failed(failure),
    };
    match context.store.commit(staged, Some(&context.queue)) {
        Ok(committed) => Ok(Ok(committed)),
        Err(failure) => failed(failure),
    }
}

// ------------------------------------------------------------------ blobs

/// A `PUT …/blobs`' start: `name` the upload's word in its lines.
#[derive(Debug, Clone)]
pub struct BlobPutRequest {
    pub name: Option<String>,
    /// `Content-Length`.
    pub declared: u64,
    pub caps: BlobCaps,
    pub seams: BlobSeams,
    /// The project's uploads in flight.
    pub flight: BlobFlight,
}

/// A `PUT …/blobs` begun, or refused before a byte is kept.
#[derive(Debug)]
pub enum BlobBegun {
    Upload(Box<Upload>),
    Refused(Box<BlobOutcome>),
}

/// `PUT …/blobs`' start, outside the turn: `[ids]` and `[references]`
/// judged (exit 2), the store's caps on the declared size, its walk and
/// a due sweep's candidates, the `.part` created (task spec
/// `work-references`, "The blob store" steps 1 and 2).
pub fn blob_begin(
    env: &Env,
    globals: &Globals,
    request: &BlobPutRequest,
) -> Result<BlobBegun, CliError> {
    let (project, slug) = found(env, globals)?;
    let allow = references_allow(env, globals)?;
    let data_dir = prepared_data_dir(env, &project)?;
    let store = Store::new(
        store_dir(&data_dir, &slug),
        request.caps,
        request.seams.clone(),
    );
    // A queue no command has made yet holds no reference.
    let queue = SqliteQueue::open_existing(data_dir.join(format!("{slug}.db")), &slug)
        .map_err(queue_cannot)?;
    let name = request.name.as_deref().unwrap_or(UPLOAD_NAME);
    match store.begin(
        name,
        request.declared,
        allow,
        &request.flight,
        queue.as_ref(),
    ) {
        Ok(upload) => Ok(BlobBegun::Upload(Box::new(upload))),
        Err(BlobFailure::Refused(refusal)) => {
            Ok(BlobBegun::Refused(Box::new(BlobOutcome::refusal(&refusal))))
        }
        Err(BlobFailure::Cannot(line)) => Err(CliError::spec(line)),
    }
}

/// A store failure while bytes stream: the refusal's outcome, or exit 2.
pub fn blob_failed(failure: BlobFailure) -> Result<BlobOutcome, CliError> {
    match failure {
        BlobFailure::Refused(refusal) => Ok(BlobOutcome::refusal(&refusal)),
        BlobFailure::Cannot(line) => Err(CliError::spec(line)),
    }
}

/// `PUT …/blobs`' end, in the project's turn: the staged file put in place
/// and the sweep run when due (step 4); the stored blob's document.
pub fn blob_commit(
    env: &Env,
    globals: &Globals,
    staged: Spooled,
    caps: BlobCaps,
    seams: BlobSeams,
) -> Result<BlobOutcome, CliError> {
    let context = open_references(env, globals, caps, seams)?;
    match context.store.commit(staged, Some(&context.queue)) {
        Ok(committed) => Ok(BlobOutcome {
            stored: Some(BlobStoredDocument::of(&committed)),
            refused: None,
        }),
        Err(failure) => blob_failed(failure),
    }
}

/// A stored file to serve.
#[derive(Debug)]
pub struct BlobRead {
    pub file: File,
    pub size: u64,
    /// The served `Content-Type`.
    pub media: &'static str,
    /// `attachment` (a model, a PDF) rather than `inline`.
    pub attachment: bool,
    /// The stored name of the reference serving it; `None` for an agent copy.
    pub name: Option<String>,
}

/// `GET …/blobs/:b3`, outside the turn: the project's lowest live
/// reference naming `b3` as its file or its agent copy, read on a
/// connection of its own, and the file opened; `None` (a 404) when no live
/// reference names it or the store holds no such file.
pub fn blob_read(env: &Env, globals: &Globals, b3: &str) -> Result<Option<BlobRead>, CliError> {
    if !specengine_core::blob::is_b3(b3) {
        return Ok(None);
    }
    let (project, slug) = found(env, globals)?;
    let data_dir = crate::location::checked_data_dir(env, &project)?;
    // A read-only connection of its own (never the turn's).
    let Some(reader) =
        BlobReader::open(&data_dir.join(format!("{slug}.db")), &slug).map_err(queue_cannot)?
    else {
        return Ok(None);
    };
    let Some(served) = reader.served(b3).map_err(queue_cannot)? else {
        return Ok(None);
    };
    drop(reader);
    let store = Store::new(
        store_dir(&data_dir, &slug),
        BlobCaps::default(),
        BlobSeams::default(),
    );
    let media = match served.media {
        Some(media) if !served.copy => Some(media),
        _ => store.media_of(b3),
    };
    let Some(media) = media else {
        return Ok(None);
    };
    let Some(file) = store.open(b3) else {
        return Ok(None);
    };
    Ok(Some(BlobRead {
        file: file.file,
        size: file.size,
        media: media.as_str(),
        attachment: !served.copy && !media.inline(),
        name: if served.copy { None } else { served.name },
    }))
}

// ------------------------------------------------------------------ reads

/// `spec ref list`: one page of a target's references, oldest first. A node
/// that no longer resolves but has references stored under that exact ID
/// lists them, with a note.
pub fn reference_list(
    env: &Env,
    globals: &Globals,
    request: &ReferenceListRequest,
) -> Result<ReferenceListOutcome, CliError> {
    run_list(env, globals, request).map_err(escaped_error)
}

fn run_list(
    env: &Env,
    globals: &Globals,
    request: &ReferenceListRequest,
) -> Result<ReferenceListOutcome, CliError> {
    let before = before_of(request.before.as_deref())?;
    let limit = limit_of(request.limit)?;
    let context = open_references(env, globals, BlobCaps::default(), BlobSeams::default())?;
    let mut messages = Vec::new();
    let mut notes = Vec::new();
    let refused = |id: &str, reason: String, cause: ReferenceCause, messages: Vec<Message>| {
        ReferenceListOutcome {
            on: ReferenceOnJson {
                kind: CommentOn::Node.as_str(),
                id: id.to_owned(),
            },
            total: 0,
            references: Vec::new(),
            next_before: None,
            notes: Vec::new(),
            refused: Some(ReferenceRefused {
                id: Some(id.to_owned()),
                reason: one_line(&reason),
                cause,
            }),
            messages,
        }
    };
    let (on, on_id) = match written_on(&context.project, &request.on)? {
        Written::Card(id) => {
            if !target_exists(&context.queue, CommentOn::Card, &id)? {
                return Ok(refused(
                    &id,
                    format!("on: no card {id}"),
                    ReferenceCause::Unknown,
                    messages,
                ));
            }
            (CommentOn::Card, id)
        }
        Written::Proposal(id) => {
            if !target_exists(&context.queue, CommentOn::Proposal, &id)? {
                return Ok(refused(
                    &id,
                    format!("on: no proposal {id}"),
                    ReferenceCause::Unknown,
                    messages,
                ));
            }
            (CommentOn::Proposal, id)
        }
        Written::Node(written) => {
            match resolved_node_rev(env, &context.project, &written, &mut messages)? {
                Ok((id, _)) => (CommentOn::Node, id),
                Err(unresolved) => {
                    // A gone node's references stay under its old ID.
                    let stored = context
                        .queue
                        .list_references(CommentOn::Node, &written, None, 1)
                        .map_err(queue_cannot)?;
                    if stored.total == 0 {
                        let cause = resolve_cause(&unresolved).into();
                        let reason = on_line_of(&unresolved, NAMES_ONE);
                        return Ok(refused(&written, reason, cause, messages));
                    }
                    notes.push(format!(
                        "{written} is no node now; its references as stored"
                    ));
                    (CommentOn::Node, written)
                }
            }
        }
    };
    let page = context
        .queue
        .list_references(on, &on_id, before.as_deref(), limit)
        .map_err(queue_cannot)?;
    notes.extend(page.unreadable.iter().map(unreadable_reference_line));
    messages.extend(notes.iter().cloned().map(Message::Note));
    Ok(ReferenceListOutcome {
        on: ReferenceOnJson {
            kind: on.as_str(),
            id: on_id,
        },
        total: page.total,
        references: page
            .references
            .iter()
            .map(|reference| ReferenceDocument::of(reference, &context.store))
            .collect(),
        next_before: page.next_before,
        notes,
        refused: None,
        messages,
    })
}

/// `spec ref get`: one reference; `--out F` writes its file (or its agent
/// copy) to a new F, hashing as it copies; an agent's call reads its
/// agent-ready image, at most 1 MiB, base64 once.
pub fn reference_get(
    env: &Env,
    globals: &Globals,
    request: &ReferenceGetRequest,
) -> Result<ReferenceOutcome, CliError> {
    run_get(env, globals, request).map_err(escaped_error)
}

fn run_get(
    env: &Env,
    globals: &Globals,
    request: &ReferenceGetRequest,
) -> Result<ReferenceOutcome, CliError> {
    const VERB: &str = "shown";
    let Some(id) = written_reference("", &request.id)? else {
        return Ok(ReferenceOutcome::refused(
            None,
            not_a_reference(&request.id),
            ReferenceCause::Unknown,
            VERB,
        ));
    };
    if let Some(out) = &request.out
        && fs::symlink_metadata(env.cwd.join(out)).is_ok()
    {
        return Err(CliError::spec(format!(
            "--out {}: it exists; nothing written",
            out.display()
        )));
    }
    let context = open_references(env, globals, BlobCaps::default(), BlobSeams::default())?;
    let Some(reference) = context.queue.get_reference(&id).map_err(queue_cannot)? else {
        return Ok(ReferenceOutcome::refused(
            Some(&id),
            format!("no reference {id}"),
            ReferenceCause::Unknown,
            VERB,
        ));
    };
    let document = ReferenceDocument::of(&reference, &context.store);
    let image_note = match &document.agent {
        AgentBytes::NoImage(why) => Some(why.clone()),
        _ => None,
    };
    let mut outcome = ReferenceOutcome::done(document, VERB, Vec::new());
    outcome.image_note = image_note;
    if let Some(out) = &request.out {
        if let Some(refused) = write_out(env, &context.store, &reference, out, request.agent_copy)?
        {
            return Ok(refused);
        }
        outcome.written = Some(out.display().to_string());
    }
    if request.image
        && let Some(document) = &outcome.reference
        && let Some(image) = read_image(&context.store, document)
    {
        outcome.images.push(image);
    }
    Ok(outcome)
}

/// `--out F`: the file (or its copy) copied to a new F in 64 KiB pieces,
/// hashed as it goes; a mismatch removes F. A refusal is the outcome.
fn write_out(
    env: &Env,
    store: &Store,
    reference: &Reference,
    out: &Path,
    copy: bool,
) -> Result<Option<ReferenceOutcome>, CliError> {
    const VERB: &str = "shown";
    let id = reference.id.as_str();
    let refused = |reason: String| {
        Ok(Some(ReferenceOutcome::refused(
            Some(id),
            reason,
            ReferenceCause::Refused,
            VERB,
        )))
    };
    if reference.deleted.is_some() {
        return refused(format!("{id} is deleted: it has no file"));
    }
    if reference.kind == ReferenceKind::Link {
        return refused(format!("{id} is a link: it has no file"));
    }
    let hash = if copy {
        match &reference.agent_b3 {
            Some(hash) => hash.clone(),
            None => return refused(format!("{id} has no agent copy")),
        }
    } else {
        match &reference.b3 {
            Some(hash) => hash.clone(),
            None => return refused(format!("{id} has no file")),
        }
    };
    let Some(mut source) = store.open(&hash) else {
        return refused(format!(
            "{id}'s file is not on this host: it was lost with the host it was uploaded to"
        ));
    };
    let path = env.cwd.join(out);
    let shown = out.display().to_string();
    let cannot = |error: std::io::Error| CliError::cannot(format!("spec: --out {shown}: {error}"));
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(cannot)?;
    let mut hasher = BlobHasher::new();
    let mut buffer = vec![0u8; BLOB_BUFFER];
    let copied = (|| -> std::io::Result<()> {
        loop {
            let read = source.file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            target.write_all(&buffer[..read])?;
        }
        target.sync_all()
    })();
    if let Err(error) = copied {
        let _ = fs::remove_file(&path);
        return Err(cannot(error));
    }
    if hasher.finalize() != hash {
        let _ = fs::remove_file(&path);
        return refused(format!(
            "{id}'s file does not match its BLAKE3: the store is damaged"
        ));
    }
    Ok(None)
}

/// The agent-ready image of `document`, read whole (at most
/// [`AGENT_BYTES`]) and base64-encoded once; `None` when none is held.
fn read_image(store: &Store, document: &ReferenceDocument) -> Option<AgentImage> {
    let (hash, media) = match &document.agent {
        AgentBytes::Original { b3, media } => (b3.clone(), *media),
        AgentBytes::Copy { b3 } => (b3.clone(), store.media_of(b3)?),
        _ => return None,
    };
    if !media.copyable() && media != Media::Gif {
        return None;
    }
    let file = store.open(&hash)?;
    if file.size > AGENT_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(file.size).ok()?);
    file.file
        .take(AGENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > AGENT_BYTES {
        return None;
    }
    Some(AgentImage {
        id: document.id.clone(),
        media: media.as_str(),
        data: base64(&bytes),
    })
}

/// The agent-ready images of the references `ids` (a card's
/// `context.images:`, in its order): each read whole, at most 1 MiB, and
/// base64-encoded once; one no longer held is left out.
pub fn reference_images(
    env: &Env,
    globals: &Globals,
    ids: &[String],
) -> Result<Vec<AgentImage>, CliError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let context = open_references(env, globals, BlobCaps::default(), BlobSeams::default())?;
    let mut images = Vec::new();
    for id in ids.iter().take(CONTEXT_IMAGES) {
        let Some(reference) = context.queue.get_reference(id).map_err(queue_cannot)? else {
            continue;
        };
        let document = ReferenceDocument::of(&reference, &context.store);
        if let Some(image) = read_image(&context.store, &document) {
            images.push(image);
        }
    }
    Ok(images)
}

/// Standard base64 with padding (RFC 4648 §4), as MCP's image content
/// carries it.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let triple = (b0 << 16) | (b1 << 8) | b2;
        let sextet = |shift: u32| char::from(ALPHABET[((triple >> shift) & 0x3F) as usize]);
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    out
}

// ----------------------------------------------------------------- delete

/// `spec ref delete`: on a hub the author's login or a `[team]` `owner`
/// (judged here, in the call; `[team]` absent or bad: the author alone);
/// elsewhere whoever is at the terminal or the page, after `consent`. The
/// texts, hashes and facts erased, a tombstone kept, `reference.deleted`
/// logged, and each of its files no live reference names unlinked in the
/// same call.
pub fn reference_delete(
    env: &Env,
    globals: &Globals,
    request: &ReferenceDeleteRequest,
    consent: Consent<'_>,
) -> Result<ReferenceOutcome, CliError> {
    run_delete(env, globals, request, consent).map_err(escaped_error)
}

fn run_delete(
    env: &Env,
    globals: &Globals,
    request: &ReferenceDeleteRequest,
    consent: Consent<'_>,
) -> Result<ReferenceOutcome, CliError> {
    const VERB: &str = "deleted";
    let now = checked_now(&request.now)?;
    let by = match &request.login {
        Some(login) => login.clone(),
        None => person_of(request.actor.as_deref())?,
    };
    let Some(id) = written_reference("", &request.id)? else {
        return Ok(ReferenceOutcome::refused(
            None,
            not_a_reference(&request.id),
            ReferenceCause::Unknown,
            VERB,
        ));
    };
    let mut context = open_references(env, globals, BlobCaps::default(), BlobSeams::default())?;
    let Some(reference) = context.queue.get_reference(&id).map_err(queue_cannot)? else {
        return Ok(ReferenceOutcome::refused(
            Some(&id),
            format!("no reference {id}"),
            ReferenceCause::Unknown,
            VERB,
        ));
    };
    if reference.deleted.is_some() {
        return Ok(ReferenceOutcome::refused(
            Some(&id),
            format!("{id} is deleted: nothing changed"),
            ReferenceCause::Refused,
            VERB,
        ));
    }
    if let Some(login) = &request.login
        && reference.author.login.as_deref() != Some(login.as_str())
    {
        // `[team]` read now; absent or bad, the author alone deletes.
        let owner = team_read(env, globals)
            .ok()
            .flatten()
            .and_then(|team| {
                team.entry(login)
                    .map(|entry| entry.role.deletes_references())
            })
            .unwrap_or(false);
        if !owner {
            return Ok(ReferenceOutcome::refused(
                Some(&id),
                format!(
                    "{id} is {}'s: only its author or an `owner` deletes it",
                    reference.actor
                ),
                ReferenceCause::Forbidden,
                VERB,
            ));
        }
    }
    let question = match (reference.kind, reference.media, reference.size) {
        (ReferenceKind::File, Some(media), Some(size)) => format!(
            "delete {id} ({}, on {}, {} {})? Its file goes when no other reference names it. \
             [y/N]",
            reference.actor,
            reference.on_id,
            media.as_str(),
            size_text(size)
        ),
        _ => format!(
            "delete {id} ({}, on {}, link)? [y/N]",
            reference.actor, reference.on_id
        ),
    };
    if !consent(&escape_controls(&one_line(&question))) {
        return Ok(ReferenceOutcome::refused(
            Some(&id),
            format!("{id} not deleted: the answer was not `y`; nothing changed"),
            ReferenceCause::Refused,
            VERB,
        ));
    }
    match context.queue.delete_reference(&id, &by, now) {
        Ok(done) => {
            // In the same call, still in a daemon's turn: no attach races it.
            context.store.unlink(&done.unnamed);
            Ok(ReferenceOutcome::done(
                ReferenceDocument::of(&done.reference, &context.store),
                VERB,
                Vec::new(),
            ))
        }
        Err(error) => store_refusal(error, VERB, &id),
    }
}

// ------------------------------------------------------------------ bodies

/// The body of `POST …/references`: `{on, b3, agent_b3, name, url, title,
/// matters, as}`; any other key refused.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceAddBody {
    pub on: String,
    #[serde(default)]
    pub b3: Option<String>,
    #[serde(default)]
    pub agent_b3: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub matters: Option<String>,
    #[serde(rename = "as", default)]
    pub actor: Option<String>,
}

impl ReferenceAddBody {
    /// `bytes` decoded, `as` a card person (trimmed), the shape and the
    /// texts' caps judged (exit 2, the daemon's 400, nothing read).
    pub fn from_json(bytes: &[u8]) -> Result<Self, CliError> {
        let body: Self = serde_json::from_slice(bytes).map_err(|error| {
            escaped_error(CliError::spec(format!(
                "the reference's body is not `{{\"on\",\"b3\",\"agent_b3\",\"name\",\"url\",\
                 \"title\",\"matters\",\"as\"}}` as JSON: {error}; nothing changed"
            )))
        })?;
        let body = Self {
            actor: checked_as(body.actor)?,
            ..body
        };
        let probe = body.clone().request(String::new(), None);
        if let Some(problem) = shape_problem(&probe) {
            return Err(escaped_error(CliError::spec(problem)));
        }
        Ok(body)
    }

    /// The request at `now`, `login` a hub's.
    pub fn request(self, now: String, login: Option<String>) -> ReferenceAddRequest {
        ReferenceAddRequest {
            on: self.on,
            b3: self.b3,
            agent_b3: self.agent_b3,
            url: self.url,
            name: self.name,
            title: self.title,
            matters: self.matters,
            actor: self.actor,
            author_role: None,
            author_model: None,
            run: None,
            agent: false,
            login,
            prepared: None,
            now,
        }
    }
}

// ------------------------------------------------------------------- text

/// Who a line names: a person's words; an agent's `agent <role>` (`-` for
/// none) and ` for <login>` on a hub.
fn who(author: &Author, actor: &str) -> String {
    if author.author_type != AuthorType::Agent {
        return actor.to_owned();
    }
    let role = author.role.as_deref().unwrap_or("-");
    match &author.login {
        Some(login) => format!("agent {role} for {login}"),
        None => format!("agent {role}"),
    }
}

/// A file's facts in words: `4032x3024`, `2.0 s`, `16 triangles`.
fn facts_words(meta: Option<&Facts>) -> Vec<String> {
    let mut words = Vec::new();
    let Some(meta) = meta else {
        return words;
    };
    if let (Some(width), Some(height)) = (meta.width, meta.height) {
        words.push(format!("{width}x{height}"));
    }
    if let Some(ms) = meta.duration_ms {
        words.push(format!("{}.{} s", ms / 1_000, (ms % 1_000) / 100));
    }
    if let Some(triangles) = meta.triangles {
        words.push(format!("{triangles} triangles"));
    }
    words
}

/// The kind field of a line: `link`, or a file's type, its size (when
/// `sized`), its facts, its agent image and a lost file.
fn kind_field(document: &ReferenceDocument, sized: bool) -> String {
    if document.kind == ReferenceKind::Link.as_str() {
        return "link".to_owned();
    }
    let mut words = vec![document.media.unwrap_or("-").to_owned()];
    if sized && let Some(size) = document.size {
        words.push(size_text(size));
    }
    words.extend(facts_words(document.meta.as_ref()));
    let mut field = words.join(" ");
    match &document.agent {
        AgentBytes::Original { .. } => field.push_str(", agent image"),
        AgentBytes::Copy { .. } => field.push_str(", agent copy"),
        _ => {}
    }
    if document.file == Some("lost") {
        field.push_str(", file lost");
    }
    field
}

fn escaped(text: &str) -> String {
    escape_controls(&one_line(text))
}

/// A reference's lines as `list` prints them.
fn reference_text(out: &mut String, document: &ReferenceDocument) {
    if let Some(deleted) = &document.deleted {
        let _ = writeln!(
            out,
            "{} | deleted by {} {}",
            document.id,
            escaped(&deleted.by),
            deleted.at
        );
        return;
    }
    let mut fields = vec![
        document.id.clone(),
        escaped(&who(&document.author, &document.actor)),
        document.created_at.clone(),
        kind_field(document, true),
    ];
    if let Some(title) = &document.title {
        fields.push(escaped(title));
    }
    let _ = writeln!(out, "{}", fields.join(" | "));
    if let Some(matters) = &document.matters {
        let _ = writeln!(out, "  matters: {}", escaped(matters));
    }
    if let Some(url) = &document.url {
        let _ = writeln!(out, "  url: {}", escaped(url));
    }
    if let Some(name) = &document.name {
        let _ = writeln!(out, "  file: {}", escaped(name));
    }
    if let Some(warning) = document.warning {
        let _ = writeln!(out, "  warning: {warning}");
    }
}

/// `list`'s text (task spec `work-references`, "Documents").
pub(crate) fn render_list(outcome: &ReferenceListOutcome) -> String {
    if outcome.refused.is_some() {
        return String::new();
    }
    let on = escaped(&outcome.on.id);
    let mut out = if outcome.total == 0 {
        format!("# References on {on}: none\n")
    } else {
        format!(
            "# References on {on}: {}; {PEOPLES_MATERIAL}\n",
            outcome.total
        )
    };
    for document in &outcome.references {
        reference_text(&mut out, document);
    }
    if let Some(next) = &outcome.next_before {
        let _ = writeln!(out, "next: --before {next}");
    }
    out
}

/// `add`'s, `get`'s and `delete`'s text: the reference as `list` prints
/// it, under its header; `get`'s image line when no image reaches agents.
pub(crate) fn render_one(outcome: &ReferenceOutcome) -> String {
    let mut out = String::new();
    let Some(document) = &outcome.reference else {
        return out;
    };
    let _ = writeln!(
        out,
        "# Reference on {}; {PEOPLES_MATERIAL}",
        escaped(&document.on.id)
    );
    reference_text(&mut out, document);
    if let Some(why) = &outcome.image_note {
        let _ = writeln!(out, "  image: none for agents: {why}");
    }
    if let Some(written) = &outcome.written {
        let _ = writeln!(out, "written: {}", escaped(written));
    }
    out
}

/// A `PUT`'s text: the stored blob.
pub(crate) fn render_blob(outcome: &BlobOutcome) -> String {
    let Some(stored) = &outcome.stored else {
        return String::new();
    };
    let mut words = vec![stored.media.to_owned(), size_text(stored.size)];
    words.extend(facts_words(Some(&stored.meta)));
    format!("{} | {} | {}\n", stored.b3, words.join(" "), stored.stored)
}

/// A refusal's stderr line.
pub(crate) fn refusal_lines(refused: Option<&ReferenceRefused>) -> Vec<String> {
    refused
        .map(|refused| {
            let reason = escape_controls(&refused.reason);
            if reason.starts_with("refused: ") {
                vec![reason]
            } else {
                vec![format!("spec: {reason}")]
            }
        })
        .unwrap_or_default()
}

// --------------------------------------------------------------- context

/// One reference of an agent's context (task spec `work-references`, "A
/// card's context and the task package"), and `on` (its card) in a task
/// package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextReference {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    pub actor: String,
    pub author: Author,
    pub created_at: String,
    pub kind: &'static str,
    /// Escaped, as every text below.
    pub title: Option<String>,
    /// One line, cut at 512 characters (`cut`).
    pub matters: Option<String>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub media: Option<&'static str>,
    pub class: Option<&'static str>,
    pub size: Option<u64>,
    pub meta: Option<Facts>,
    /// An image reaches the agent: the file as it is, or its copy.
    pub agent_image: bool,
    pub cut: bool,
    /// The kind field of its line (never JSON).
    #[serde(skip)]
    field: String,
}

/// `{total, listed, images}`: the cards' live references counted, those
/// listed, and the IDs whose images `get_card` attaches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContextReferences {
    pub total: u64,
    pub listed: Vec<ContextReference>,
    pub images: Vec<String>,
}

impl ContextReferences {
    /// Its `context.references:` line, each `context.reference:` line and
    /// the `context.images:` line, in order: what the room `U` counts.
    pub(crate) fn lines(&self) -> Vec<String> {
        if self.total == 0 {
            return vec!["context.references: -".to_owned()];
        }
        let mut lines = vec![format!(
            "context.references: {} of {}, the newest; {PEOPLES_MATERIAL}",
            self.listed.len(),
            self.total
        )];
        lines.extend(
            self.listed
                .iter()
                .map(|reference| context_line(reference, "context.reference: ")),
        );
        lines.push(if self.images.is_empty() {
            "context.images: -".to_owned()
        } else {
            format!(
                "context.images: {}; get_card attaches these images, in this order",
                self.images.join(", ")
            )
        });
        lines
    }

    /// The note naming what is left out, when anything is.
    pub(crate) fn note(&self, on: &str) -> Option<String> {
        let left = self.total.saturating_sub(self.listed.len() as u64);
        (left > 0).then(|| {
            if left == 1 {
                format!("1 older reference not shown: list_references on {on} has it")
            } else {
                format!("{left} older references not shown: list_references on {on} has them")
            }
        })
    }
}

/// `<prefix><ID> | <who> | <time>[ | on C] | <kind> [| title] [| url or
/// name] [| matters: …]`, every text escaped.
pub(crate) fn context_line(reference: &ContextReference, prefix: &str) -> String {
    let mut fields = vec![
        format!("{prefix}{}", reference.id),
        escaped(&who(&reference.author, &reference.actor)),
        reference.created_at.clone(),
    ];
    if let Some(on) = &reference.on {
        fields.push(format!("on {}", escaped(on)));
    }
    fields.push(reference.field.clone());
    fields.extend(reference.title.clone());
    fields.extend(reference.url.clone().or_else(|| reference.name.clone()));
    if let Some(matters) = &reference.matters {
        fields.push(format!("matters: {matters}"));
    }
    fields.join(" | ")
}

/// The newest live references of `cards` an agent's context carries: taken
/// newest first, at most 10, each "what matters" one line cut at 512
/// characters, the `context.reference:` lines (each its characters + 1)
/// within 4 096; the first that does not fit and every older one left out;
/// listed oldest first; the images the newest agent-ready ones, at most 3.
/// `with_on`: each names its card (a task package).
pub(crate) fn context_references(
    queue: &SqliteQueue,
    store: &Store,
    cards: &[&str],
    with_on: bool,
) -> Result<(ContextReferences, Vec<String>), CliError> {
    let read = queue.card_reference_context(cards).map_err(queue_cannot)?;
    let notes: Vec<String> = read
        .unreadable
        .iter()
        .map(|row: &UnreadableRow| unreadable_reference_line(row))
        .collect();
    let mut listed = Vec::new();
    let mut images = Vec::new();
    let mut used = 0usize;
    for reference in &read.newest {
        let document = ReferenceDocument::of(reference, store);
        let (matters, cut) = match document.matters.as_deref() {
            Some(matters) => {
                let (line, cut) = context_matters(matters);
                (Some(escape_controls(&line)), cut)
            }
            None => (None, false),
        };
        let entry = ContextReference {
            id: document.id.clone(),
            on: with_on.then(|| reference.on_id.clone()),
            actor: document.actor.clone(),
            author: document.author.clone(),
            created_at: document.created_at.clone(),
            kind: document.kind,
            title: document.title.as_deref().map(escaped),
            matters,
            url: document.url.as_deref().map(escaped),
            name: document.name.as_deref().map(escaped),
            media: document.media,
            class: document.class,
            size: document.size,
            meta: document.meta,
            agent_image: document.agent.some(),
            cut,
            field: kind_field(&document, false),
        };
        let chars = context_line(&entry, "context.reference: ").chars().count() + 1;
        if used + chars > CONTEXT_REFS_CHARS {
            break;
        }
        used += chars;
        if entry.agent_image && images.len() < CONTEXT_IMAGES {
            images.push(entry.id.clone());
        }
        listed.push(entry);
    }
    listed.reverse();
    Ok((
        ContextReferences {
            total: read.total,
            listed,
            images,
        },
        notes,
    ))
}

/// The store of a queue's project, for a context's read.
pub(crate) fn context_store(data_dir: &Path, slug: &str) -> Store {
    Store::new(
        store_dir(data_dir, slug),
        BlobCaps::default(),
        BlobSeams::default(),
    )
}
