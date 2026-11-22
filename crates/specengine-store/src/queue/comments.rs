//! Comments in the project's queue (task spec `comments`, "Data";
//! `docs/canon/comments.md`; ADR-0087): talk under a card, a proposal or a
//! node, never truth. Step 15 adds `comments`, `comment_versions` and a
//! proposal's `comment_id`. Every change is one `Immediate` transaction
//! holding its one row change and its one `comment.*` event, under the rules
//! of core's [`specengine_core::comment`], asked again here, under the lock.
//! No card, proposal, task or lock row is written here, and nothing outside
//! this file reads a comment table but the dump: no card move, accept, stage,
//! decision, gate or check (ADR-0012).
//!
//! - **IDs**: `CM-` and 4 or more digits, the highest of the table plus one
//!   in the inserting transaction; rows are never removed, so an ID is never
//!   reused. A delete is an `UPDATE`.
//! - **Versions**: an edit is a compare-and-set on `version`; the replaced
//!   body goes to `comment_versions` with the time it was written.
//! - **Delete**: `PRAGMA secure_delete` on the connection before the
//!   transaction, every version's text and the mentions erased, the row a
//!   tombstone (`deleted_by`, `deleted_at`).
//! - **Reads** step their rows one at a time and page at
//!   [`COMMENT_PAGE_MAX`]: a page, never the table, is held (task spec
//!   `mixed-load-memory`'s lesson). A row that does not decode is named and
//!   skipped ([`CommentPage::unreadable`]).

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, Row, Transaction};
use specengine_core::comment::{
    COMMENT_CONTEXT_MAX, COMMENT_PAGE_MAX, CommentOn, body_problem, comment_id, comment_number,
    is_login, mentions,
};
use specengine_core::proposal::{Author, AuthorType, author_field_problem, is_utc_timestamp};

use super::stage::{as_key, json_text};
use super::state::stored_text;
use super::{
    COMMENTS_SCHEMA, ID_ORDER, QueueError, SqliteQueue, UnreadableRow, check_time, corrupt,
    index_only, log_text, to_json, user_version,
};
use crate::error::{Db, StoreError};
use crate::schema;

/// `comment.created`, with `id`, `on` (the target's ID) and `as`.
pub const EVENT_COMMENT_CREATED: &str = "comment.created";
/// `comment.edited`, with `id`, `version` (the new one) and `as`.
pub const EVENT_COMMENT_EDITED: &str = "comment.edited";
/// `comment.deleted`, with `id` and `as`.
pub const EVENT_COMMENT_DELETED: &str = "comment.deleted";

/// Every column of `comments`, in table order: `on_rev` and `version`
/// `INTEGER`, the rest `TEXT`.
pub const COMMENT_COLUMNS: [&str; 15] = [
    "id",
    "project",
    "on_kind",
    "on_id",
    "on_rev",
    "reply_to",
    "actor",
    "author",
    "version",
    "body",
    "mentions",
    "created_at",
    "updated_at",
    "deleted_by",
    "deleted_at",
];

/// Every column of `comment_versions`, in table order: `version` `INTEGER`,
/// the rest `TEXT`.
pub const COMMENT_VERSION_COLUMNS: [&str; 5] =
    ["comment_id", "project", "version", "body", "written_at"];

/// `on_rev` among [`COMMENT_COLUMNS`].
const ON_REV_COLUMN: usize = 4;

/// `version` among [`COMMENT_COLUMNS`].
const VERSION_COLUMN: usize = 8;

/// `version` among [`COMMENT_VERSION_COLUMNS`].
const VERSIONS_VERSION_COLUMN: usize = 2;

/// The order of `comment_versions`: by comment ID number, then version.
pub(super) const VERSION_ORDER: &str = "ORDER BY length(comment_id), comment_id, version";

/// The select list of `comments`.
const SELECT: &str = "id, project, on_kind, on_id, on_rev, reply_to, actor, author, version, \
                      body, mentions, created_at, updated_at, deleted_by, deleted_at";

/// Rows of an ID below `?N`'s by number, every row when `?N` is empty:
/// `before`'s filter, `N` its slot.
fn below(slot: usize) -> String {
    format!(
        "(?{slot} = '' OR length(id) < length(?{slot}) \
         OR (length(id) = length(?{slot}) AND id < ?{slot}))"
    )
}

/// What a comment add stores; the queue adds the ID, project, version and
/// times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub on: CommentOn,
    /// Canonical: `C-0007`, `PR-0031`, `MEC-RAIL`, `docs/spec/rail.md`.
    pub on_id: String,
    /// A node's revision when written; `None` on a card or a proposal.
    pub on_rev: Option<u64>,
    /// A top-level comment on the same target, or none.
    pub reply_to: Option<String>,
    /// In words: the login on a hub, the typed name, `local`, `agent`.
    pub actor: String,
    pub author: Author,
    /// Markdown, stored as given ([`body_problem`]).
    pub body: String,
}

/// A delete's tombstone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDeleted {
    /// The deleter in words.
    pub by: String,
    pub at: String,
}

/// A proposal raised from a comment, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentItem {
    pub id: String,
    /// As stored.
    pub kind: Option<String>,
    /// As stored.
    pub status: Option<String>,
}

/// A stored comment, decoded, with the items raised from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub project: String,
    pub on: CommentOn,
    pub on_id: String,
    pub on_rev: Option<u64>,
    pub reply_to: Option<String>,
    pub actor: String,
    pub author: Author,
    /// 1, then + 1 per edit.
    pub version: u64,
    /// `None` once deleted.
    pub body: Option<String>,
    /// Sorted, each once; empty once deleted.
    pub mentions: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub deleted: Option<CommentDeleted>,
    /// The proposals whose `comment_id` names it, by ID number.
    pub items: Vec<CommentItem>,
}

impl Comment {
    /// An agent wrote it.
    pub fn by_agent(&self) -> bool {
        self.author.author_type == AuthorType::Agent
    }
}

/// One page of comments: a target's (oldest first) or a login's mentions
/// (newest first).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentPage {
    /// Every comment the read covers, deleted ones included (a target's);
    /// the page's own for the mentions.
    pub total: u64,
    pub comments: Vec<Comment>,
    /// The page's smallest ID when older comments exist.
    pub next_before: Option<String>,
    /// The rows of the page that do not decode, named and skipped.
    pub unreadable: Vec<UnreadableRow>,
}

/// What a card's context, or a task package, carries of its cards' talk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentContext {
    /// The live comments on the cards.
    pub total: u64,
    /// The newest live ones, newest first, at most [`COMMENT_CONTEXT_MAX`].
    pub newest: Vec<Comment>,
    pub unreadable: Vec<UnreadableRow>,
}

/// What an edit asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentEdit {
    pub id: String,
    pub body: String,
    /// The version the editor read: a compare-and-set.
    pub version: u64,
    /// The editor in words: the login on a hub, the typed name, `local`.
    pub actor: String,
    /// A hub's login: compared with the author's `login`; `None` elsewhere,
    /// where `actor` is compared with the row's.
    pub login: Option<String>,
}

/// Why a comment change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentRefusal {
    /// No such comment or target: a 404.
    Unknown,
    /// A rule: a stale version, a reply rule, a deleted comment: a 409.
    Refused,
    /// The person may not make it: a 403.
    Forbidden,
}

fn refusal(id: &str, cause: CommentRefusal, reason: String) -> QueueError {
    QueueError::CommentRefused {
        id: id.to_owned(),
        reason,
        cause,
    }
}

/// `no comment CM-0099`.
pub(super) fn no_comment(id: &str) -> String {
    format!("no comment {id}")
}

/// A `comments` row as stored, never decoded: `on_rev`, `version`, and the
/// `TEXT` columns around them in [`COMMENT_COLUMNS`] order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredComment {
    pub on_rev: Option<i64>,
    pub version: i64,
    pub columns: [Option<String>; COMMENT_COLUMNS.len() - 2],
}

impl StoredComment {
    /// The stored `id`.
    pub fn id(&self) -> Option<&str> {
        self.columns[0].as_deref()
    }

    /// The stored `project`.
    pub fn project(&self) -> Option<&str> {
        self.columns[1].as_deref()
    }
}

/// A `comment_versions` row as stored: `version`, and the `TEXT` columns
/// around it in [`COMMENT_VERSION_COLUMNS`] order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCommentVersion {
    pub version: i64,
    pub columns: [Option<String>; COMMENT_VERSION_COLUMNS.len() - 1],
}

impl StoredCommentVersion {
    /// The stored `comment_id`.
    pub fn comment_id(&self) -> Option<&str> {
        self.columns[0].as_deref()
    }

    /// The stored `project`.
    pub fn project(&self) -> Option<&str> {
        self.columns[1].as_deref()
    }
}

/// The comment tables as stored: `comments` by ID number, `comment_versions`
/// by comment ID number and version.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredComments {
    pub comments: Vec<StoredComment>,
    pub versions: Vec<StoredCommentVersion>,
}

impl StoredComments {
    /// Every row is `project`'s, else [`QueueError::Invalid`].
    pub(super) fn check_project(&self, project: &str) -> Result<(), QueueError> {
        if let Some(row) = self
            .comments
            .iter()
            .find(|row| row.project() != Some(project))
        {
            return Err(QueueError::Invalid(format!(
                "comment {}: not a row of the project `{project}`",
                row.id().unwrap_or("NULL")
            )));
        }
        if let Some(row) = self
            .versions
            .iter()
            .find(|row| row.project() != Some(project))
        {
            return Err(QueueError::Invalid(format!(
                "version {} of comment {}: not a row of the project `{project}`",
                row.version,
                row.comment_id().unwrap_or("NULL")
            )));
        }
        Ok(())
    }
}

/// The queue holds the comment tables (a handle of
/// [`SqliteQueue::open_existing`] may stand below step 15).
fn has_comments(conn: &Connection) -> Result<bool, QueueError> {
    if index_only(conn)? || user_version(conn)? < COMMENTS_SCHEMA {
        return Ok(false);
    }
    Ok(schema::has_table(conn, "comments")?)
}

/// Every row of both tables, as stored, inside `conn`'s open transaction.
pub(super) fn stored_in(conn: &Connection) -> Result<StoredComments, QueueError> {
    let mut stored = StoredComments::default();
    if !schema::has_table(conn, "comments")? {
        return Ok(stored);
    }
    let mut statement = conn
        .prepare(&format!(
            "SELECT {} FROM main.comments {ID_ORDER}",
            COMMENT_COLUMNS.join(", ")
        ))
        .db()?;
    let mut rows = statement.query([]).db()?;
    while let Some(row) = rows.next().db()? {
        stored.comments.push(stored_comment(row)?);
    }
    drop(rows);
    drop(statement);
    let mut statement = conn
        .prepare(&format!(
            "SELECT {} FROM main.comment_versions {VERSION_ORDER}",
            COMMENT_VERSION_COLUMNS.join(", ")
        ))
        .db()?;
    let mut rows = statement.query([]).db()?;
    while let Some(row) = rows.next().db()? {
        stored.versions.push(stored_version(row)?);
    }
    Ok(stored)
}

/// Either comment table holds a row (any project's).
pub(super) fn any_in(conn: &Connection) -> Result<bool, QueueError> {
    for table in ["comments", "comment_versions"] {
        if schema::has_table(conn, table)? {
            let found: Option<i64> = conn
                .query_row(&format!("SELECT 1 FROM main.{table} LIMIT 1"), [], |row| {
                    row.get(0)
                })
                .optional()
                .db()?;
            if found.is_some() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Inserts `stored`'s rows as given, inside the restore's transaction.
pub(super) fn insert_stored(
    tx: &Transaction<'_>,
    stored: &StoredComments,
) -> Result<(), QueueError> {
    let slots: Vec<String> = (1..=COMMENT_COLUMNS.len())
        .map(|number| format!("?{number}"))
        .collect();
    let mut insert = tx
        .prepare(&format!(
            "INSERT INTO main.comments ({}) VALUES ({})",
            COMMENT_COLUMNS.join(", "),
            slots.join(", ")
        ))
        .db()?;
    for row in &stored.comments {
        let mut values: Vec<rusqlite::types::Value> = row.columns.iter().map(text_value).collect();
        values.insert(
            ON_REV_COLUMN,
            row.on_rev.map_or(
                rusqlite::types::Value::Null,
                rusqlite::types::Value::Integer,
            ),
        );
        values.insert(VERSION_COLUMN, rusqlite::types::Value::Integer(row.version));
        insert.execute(rusqlite::params_from_iter(values)).db()?;
    }
    let mut insert = tx
        .prepare(&format!(
            "INSERT INTO main.comment_versions ({}) VALUES (?1, ?2, ?3, ?4, ?5)",
            COMMENT_VERSION_COLUMNS.join(", ")
        ))
        .db()?;
    for row in &stored.versions {
        let mut values: Vec<rusqlite::types::Value> = row.columns.iter().map(text_value).collect();
        values.insert(
            VERSIONS_VERSION_COLUMN,
            rusqlite::types::Value::Integer(row.version),
        );
        insert.execute(rusqlite::params_from_iter(values)).db()?;
    }
    Ok(())
}

fn text_value(value: &Option<String>) -> rusqlite::types::Value {
    match value {
        Some(text) => rusqlite::types::Value::Text(text.clone()),
        None => rusqlite::types::Value::Null,
    }
}

/// The name of a row's first column, as far as it reads.
fn row_name(row: &Row<'_>) -> Result<String, QueueError> {
    Ok(match row.get_ref(0).db()? {
        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        _ => "NULL".to_owned(),
    })
}

/// An integer column as stored: `None` for `NULL`, else what is no integer
/// fails, naming the row and the column.
fn stored_integer(
    row: &Row<'_>,
    index: usize,
    what: &str,
    column: &str,
) -> Result<Option<i64>, QueueError> {
    match row.get_ref(index).db()? {
        ValueRef::Integer(value) => Ok(Some(value)),
        ValueRef::Null => Ok(None),
        _ => Err(QueueError::Store(StoreError::Sqlite(format!(
            "{what}: the stored `{column}` is not an integer"
        )))),
    }
}

fn stored_comment(row: &Row<'_>) -> Result<StoredComment, QueueError> {
    let name = row_name(row)?;
    let what = format!("comment {name}");
    let on_rev = stored_integer(row, ON_REV_COLUMN, &what, "on_rev")?;
    let version = stored_integer(row, VERSION_COLUMN, &what, "version")?.ok_or_else(|| {
        QueueError::Store(StoreError::Sqlite(format!(
            "{what}: the stored `version` is NULL"
        )))
    })?;
    let mut columns: [Option<String>; COMMENT_COLUMNS.len() - 2] = Default::default();
    let texts = COMMENT_COLUMNS
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != ON_REV_COLUMN && *index != VERSION_COLUMN);
    for (slot, (index, column)) in columns.iter_mut().zip(texts) {
        *slot = stored_text(row, index)?.map_err(|why| {
            QueueError::Store(StoreError::Sqlite(format!(
                "{what}: the stored `{column}` cannot be read: {why}"
            )))
        })?;
    }
    Ok(StoredComment {
        on_rev,
        version,
        columns,
    })
}

fn stored_version(row: &Row<'_>) -> Result<StoredCommentVersion, QueueError> {
    let name = row_name(row)?;
    let what = format!("a version of comment {name}");
    let version =
        stored_integer(row, VERSIONS_VERSION_COLUMN, &what, "version")?.ok_or_else(|| {
            QueueError::Store(StoreError::Sqlite(format!(
                "{what}: the stored `version` is NULL"
            )))
        })?;
    let mut columns: [Option<String>; COMMENT_VERSION_COLUMNS.len() - 1] = Default::default();
    let texts = COMMENT_VERSION_COLUMNS
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != VERSIONS_VERSION_COLUMN);
    for (slot, (index, column)) in columns.iter_mut().zip(texts) {
        *slot = stored_text(row, index)?.map_err(|why| {
            QueueError::Store(StoreError::Sqlite(format!(
                "{what}: the stored `{column}` cannot be read: {why}"
            )))
        })?;
    }
    Ok(StoredCommentVersion { version, columns })
}

/// A `comments` row as read, before decoding: its texts, its two integers,
/// or the first column holding neither.
struct RawComment {
    id: String,
    text: Vec<Option<String>>,
    on_rev: Option<i64>,
    version: Option<i64>,
    broken: Option<(&'static str, &'static str)>,
}

impl RawComment {
    fn read(row: &Row<'_>) -> Result<Self, QueueError> {
        let id = row_name(row)?;
        let mut text = Vec::with_capacity(COMMENT_COLUMNS.len());
        let mut broken = None;
        let (mut on_rev, mut version) = (None, None);
        for (index, column) in COMMENT_COLUMNS.iter().enumerate() {
            let value = row.get_ref(index).db()?;
            if index == ON_REV_COLUMN || index == VERSION_COLUMN {
                let number = match value {
                    ValueRef::Integer(number) => Some(number),
                    ValueRef::Null => None,
                    _ => {
                        broken.get_or_insert((*column, "it is no integer"));
                        None
                    }
                };
                if index == ON_REV_COLUMN {
                    on_rev = number;
                } else {
                    version = number;
                }
                text.push(None);
                continue;
            }
            text.push(match value {
                ValueRef::Null => None,
                ValueRef::Text(bytes) => match std::str::from_utf8(bytes) {
                    Ok(text) => Some(text.to_owned()),
                    Err(_) => {
                        broken.get_or_insert((*column, "it is not UTF-8 text"));
                        None
                    }
                },
                _ => {
                    broken.get_or_insert((*column, "it is no text"));
                    None
                }
            });
        }
        Ok(Self {
            id,
            text,
            on_rev,
            version,
            broken,
        })
    }

    /// The comment, its items not read; else the row named.
    fn decode(self) -> Result<Comment, UnreadableRow> {
        let id = self.id;
        if let Some((column, why)) = self.broken {
            return Err(corrupt(&id, column, why));
        }
        let mut values = self.text.into_iter();
        let mut take = || values.next().flatten();
        let required = |value: Option<String>, column: &str| {
            value.ok_or_else(|| corrupt(&id, column, "it is NULL"))
        };
        if comment_number(&id).is_none() {
            return Err(corrupt(&id, "id", format!("`{id}` is no comment ID")));
        }
        let _id = take();
        let project = required(take(), "project")?;
        let kind_text = required(take(), "on_kind")?;
        let on = CommentOn::parse(&kind_text)
            .ok_or_else(|| corrupt(&id, "on_kind", format!("`{kind_text}` is no target kind")))?;
        let on_id = required(take(), "on_id")?;
        if on_id.is_empty() || on_id.chars().any(char::is_control) {
            return Err(corrupt(
                &id,
                "on_id",
                "it is empty or holds a control character",
            ));
        }
        let _on_rev = take();
        let on_rev = match (on, self.on_rev) {
            (CommentOn::Node, Some(rev)) => Some(
                u64::try_from(rev)
                    .ok()
                    .filter(|rev| *rev >= 1)
                    .ok_or_else(|| corrupt(&id, "on_rev", format!("{rev} is no revision")))?,
            ),
            (CommentOn::Node, None) => None,
            (_, Some(_)) => {
                return Err(corrupt(
                    &id,
                    "on_rev",
                    format!("it is set on a {}, which has no revision", on.as_str()),
                ));
            }
            (_, None) => None,
        };
        let reply_to = take();
        if let Some(reply) = &reply_to
            && comment_number(reply).is_none()
        {
            return Err(corrupt(
                &id,
                "reply_to",
                format!("`{reply}` is no comment ID"),
            ));
        }
        let actor = required(take(), "actor")?;
        let author_text = required(take(), "author")?;
        let author: Author =
            serde_json::from_str(&author_text).map_err(|error| corrupt(&id, "author", error))?;
        for field in [&author.role, &author.model, &author.run] {
            if let Some(problem) = field.as_deref().and_then(author_field_problem) {
                return Err(corrupt(&id, "author", problem));
            }
        }
        if let Some(problem) = author
            .login
            .as_deref()
            .and_then(specengine_core::card::hub_login_problem)
        {
            return Err(corrupt(&id, "author", problem));
        }
        let _version = take();
        let version = self
            .version
            .and_then(|version| u64::try_from(version).ok())
            .filter(|version| *version >= 1)
            .ok_or_else(|| corrupt(&id, "version", "it is below 1"))?;
        let body = take();
        let mentions_text = take();
        let created_at = required(take(), "created_at")?;
        let updated_at = required(take(), "updated_at")?;
        for (column, time) in [("created_at", &created_at), ("updated_at", &updated_at)] {
            if !is_utc_timestamp(time) {
                return Err(corrupt(
                    &id,
                    column,
                    format!("{time:?} is no UTC time stamp"),
                ));
            }
        }
        let deleted_by = take();
        let deleted_at = take();
        let deleted = match (deleted_by, deleted_at) {
            (Some(by), Some(at)) => Some(CommentDeleted { by, at }),
            (None, None) => None,
            (by, _) => {
                let column = if by.is_none() {
                    "deleted_by"
                } else {
                    "deleted_at"
                };
                return Err(corrupt(
                    &id,
                    column,
                    "it is NULL while the delete's other column is set",
                ));
            }
        };
        let mentions = match (&deleted, body.as_deref(), mentions_text) {
            (Some(_), Some(_), _) => {
                return Err(corrupt(&id, "body", "it is set on a deleted comment"));
            }
            (Some(_), None, Some(_)) => {
                return Err(corrupt(&id, "mentions", "it is set on a deleted comment"));
            }
            (Some(_), None, None) => Vec::new(),
            (None, None, _) => return Err(corrupt(&id, "body", "it is NULL")),
            (None, Some(_), None) => Vec::new(),
            (None, Some(_), Some(text)) => {
                let listed: Vec<String> =
                    serde_json::from_str(&text).map_err(|error| corrupt(&id, "mentions", error))?;
                if listed.is_empty()
                    || !listed.iter().all(|login| is_login(login))
                    || !listed.windows(2).all(|pair| pair[0] < pair[1])
                {
                    return Err(corrupt(
                        &id,
                        "mentions",
                        "not a sorted list of logins, each once",
                    ));
                }
                listed
            }
        };
        Ok(Comment {
            id,
            project,
            on,
            on_id,
            on_rev,
            reply_to,
            actor,
            author,
            version,
            body,
            mentions,
            created_at,
            updated_at,
            deleted,
            items: Vec::new(),
        })
    }
}

/// The row of the comment `id` of `project` as read, not decoded; `None`
/// when there is none.
fn raw_of(conn: &Connection, project: &str, id: &str) -> Result<Option<RawComment>, QueueError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT {SELECT} FROM main.comments WHERE id = ?1 AND project = ?2"
        ))
        .db()?;
    let mut rows = statement.query([id, project]).db()?;
    match rows.next().db()? {
        Some(row) => Ok(Some(RawComment::read(row)?)),
        None => Ok(None),
    }
}

/// Whether a read of one comment reads the items raised from it too: a
/// document's read does, a write's own check of the row does not.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Items {
    Read,
    Skip,
}

/// The comment `id` of `project` inside `conn`'s open transaction, its items
/// read when `items` says so (else none); `Ok(None)` when there is none; a
/// row that does not decode is [`QueueError::Invalid`] naming it.
fn select_comment(
    conn: &Connection,
    project: &str,
    id: &str,
    items: Items,
) -> Result<Option<Comment>, QueueError> {
    if !has_comments(conn)? {
        return Ok(None);
    }
    let mut statement = conn
        .prepare(&format!(
            "SELECT {SELECT} FROM main.comments WHERE id = ?1 AND project = ?2"
        ))
        .db()?;
    let mut rows = statement.query([id, project]).db()?;
    let Some(row) = rows.next().db()? else {
        return Ok(None);
    };
    let raw = RawComment::read(row)?;
    drop(rows);
    let mut comment = raw.decode().map_err(unreadable_error)?;
    if items == Items::Read {
        comment.items = items_of(conn, project, &[comment.id.as_str()])?
            .into_iter()
            .map(|(_, item)| item)
            .collect();
    }
    Ok(Some(comment))
}

/// A row that does not decode, as an error: exit 2, named.
fn unreadable_error(row: UnreadableRow) -> QueueError {
    QueueError::Invalid(unreadable_line(&row))
}

/// `CM-0012: its row does not read (<column>: <reason>); not listed`.
pub fn unreadable_line(row: &UnreadableRow) -> String {
    format!(
        "{}: its row does not read ({}: {}); not listed",
        row.id, row.column, row.reason
    )
}

/// The proposals whose `comment_id` is one of `ids`, by ID number, each with
/// the comment it names: one statement, the page's IDs bound.
fn items_of(
    conn: &Connection,
    project: &str,
    ids: &[&str],
) -> Result<Vec<(String, CommentItem)>, QueueError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let slots = (0..ids.len())
        .map(|at| format!("?{}", at + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = conn
        .prepare(&format!(
            "SELECT id, kind, status, comment_id FROM main.proposals \
             WHERE project = ?1 AND comment_id IN ({slots}) {ID_ORDER}"
        ))
        .db()?;
    let params = std::iter::once(project).chain(ids.iter().copied());
    let mut rows = statement.query(rusqlite::params_from_iter(params)).db()?;
    let mut items = Vec::new();
    while let Some(row) = rows.next().db()? {
        let (Some(id), Some(comment)) = (
            row.get::<_, Option<String>>(0).ok().flatten(),
            row.get::<_, Option<String>>(3).ok().flatten(),
        ) else {
            continue;
        };
        items.push((
            comment,
            CommentItem {
                id,
                kind: row.get::<_, Option<String>>(1).ok().flatten(),
                status: row.get::<_, Option<String>>(2).ok().flatten(),
            },
        ));
    }
    Ok(items)
}

/// Each of `comments` takes the items naming it, in their order.
fn attach_items(
    conn: &Connection,
    project: &str,
    comments: &mut [Comment],
) -> Result<(), QueueError> {
    let ids: Vec<&str> = comments.iter().map(|comment| comment.id.as_str()).collect();
    let items = items_of(conn, project, &ids)?;
    for (named, item) in items {
        if let Some(comment) = comments.iter_mut().find(|comment| comment.id == named) {
            comment.items.push(item);
        }
    }
    Ok(())
}

/// The comment `id` is a live row of `project`, read inside the caller's
/// write (an intake raised from it): else [`QueueError::CommentRefused`],
/// `no comment CM-0099` or `CM-0012 is deleted`.
pub(super) fn check_live(tx: &Transaction<'_>, project: &str, id: &str) -> Result<(), QueueError> {
    let found: Option<Option<String>> = if has_comments(tx)? {
        tx.query_row(
            "SELECT deleted_at FROM main.comments WHERE id = ?1 AND project = ?2",
            [id, project],
            |row| row.get(0),
        )
        .optional()
        .db()?
    } else {
        None
    };
    match found {
        None => Err(refusal(id, CommentRefusal::Unknown, no_comment(id))),
        Some(Some(_)) => Err(refusal(
            id,
            CommentRefusal::Refused,
            format!("{id} is deleted: an item is raised from a live comment"),
        )),
        Some(None) => Ok(()),
    }
}

/// The next comment ID: the highest number of any stored comment, plus one.
fn next_comment_id(tx: &Transaction<'_>) -> Result<String, QueueError> {
    let mut statement = tx.prepare("SELECT id FROM main.comments").db()?;
    let mut rows = statement.query([]).db()?;
    let mut highest = 0;
    while let Some(row) = rows.next().db()? {
        let id: Option<String> = row.get(0).db()?;
        if let Some(number) = id.as_deref().and_then(comment_number) {
            highest = highest.max(number);
        }
    }
    let next = highest.checked_add(1).ok_or_else(|| {
        QueueError::Store(StoreError::Sqlite(format!(
            "the queue holds `{}`: no comment ID follows it",
            comment_id(highest)
        )))
    })?;
    Ok(comment_id(next))
}

/// The stored `mentions` of `body`: a sorted JSON list, `None` when none.
fn mentions_column(body: &str) -> Result<Option<String>, QueueError> {
    let found = mentions(body);
    if found.is_empty() {
        Ok(None)
    } else {
        to_json(&found).map(Some)
    }
}

/// Why `author` is no comment author: a field off the queue's grammar, or a
/// login off a hub's.
fn author_problem(author: &Author) -> Option<String> {
    [&author.role, &author.model, &author.run]
        .into_iter()
        .flatten()
        .find_map(|field| author_field_problem(field))
        .or_else(|| {
            author
                .login
                .as_deref()
                .and_then(specengine_core::card::hub_login_problem)
        })
        .map(|problem| format!("author: {problem}"))
}

impl SqliteQueue {
    /// Stores `new` under the next ID (highest + 1, taken in this
    /// transaction), version 1, `created_at` = `updated_at` = `now`, its
    /// mentions computed from its body; logs `comment.created {id, on, as}`,
    /// no body. Its card or proposal must be a row of the project (any
    /// state), its `reply_to` a top-level comment on the same target (a
    /// deleted one too), each read under this lock: else
    /// [`QueueError::CommentRefused`], nothing written, no ID taken. A body
    /// [`body_problem`] refuses: the same. A node is not checked here: the
    /// caller resolved it.
    pub fn add_comment(&mut self, new: &NewComment, now: &str) -> Result<Comment, QueueError> {
        check_time(now)?;
        if let Some(problem) = body_problem(&new.body) {
            return Err(refusal("", CommentRefusal::Refused, problem));
        }
        if let Some(problem) = author_problem(&new.author) {
            return Err(QueueError::Invalid(problem));
        }
        if new.on_id.is_empty() || new.on_id.chars().any(char::is_control) {
            return Err(QueueError::Invalid(
                "on: an empty target, or one holding a control character".to_owned(),
            ));
        }
        if new.on != CommentOn::Node && new.on_rev.is_some() {
            return Err(QueueError::Invalid(format!(
                "on: a {} has no revision",
                new.on.as_str()
            )));
        }
        let on_rev = new
            .on_rev
            .map(|rev| {
                i64::try_from(rev)
                    .map_err(|_| QueueError::Invalid(format!("on: revision {rev} is too large")))
            })
            .transpose()?;
        let author = to_json(&new.author)?;
        let mentioned = mentions_column(&new.body)?;
        let project = self.project.clone();
        self.erasing()?;
        let tx = self.write()?;
        let target_found: Option<i64> = match new.on {
            CommentOn::Card => tx
                .query_row(
                    "SELECT 1 FROM main.cards WHERE id = ?1 AND project = ?2",
                    [new.on_id.as_str(), project.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .db()?,
            CommentOn::Proposal => tx
                .query_row(
                    "SELECT 1 FROM main.proposals WHERE id = ?1 AND project = ?2",
                    [new.on_id.as_str(), project.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .db()?,
            CommentOn::Node => Some(1),
        };
        if target_found.is_none() {
            return Err(refusal(
                &new.on_id,
                CommentRefusal::Unknown,
                format!("on: no {} {}", new.on.as_str(), new.on_id),
            ));
        }
        if let Some(reply) = &new.reply_to {
            let parent = select_comment(&tx, &project, reply, Items::Skip)?;
            let Some(parent) = parent else {
                return Err(refusal(
                    reply,
                    CommentRefusal::Refused,
                    format!("reply_to: {}", no_comment(reply)),
                ));
            };
            if parent.reply_to.is_some() {
                return Err(refusal(
                    reply,
                    CommentRefusal::Refused,
                    format!("reply_to: {reply} is a reply: a reply names a top-level comment"),
                ));
            }
            if parent.on != new.on || parent.on_id != new.on_id {
                return Err(refusal(
                    reply,
                    CommentRefusal::Refused,
                    format!(
                        "reply_to: {reply} is on {}, not {}",
                        parent.on_id, new.on_id
                    ),
                ));
            }
        }
        let id = next_comment_id(&tx)?;
        tx.execute(
            &format!(
                "INSERT INTO main.comments ({SELECT}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, \
                 ?9, ?10, ?11, ?11, NULL, NULL)"
            ),
            rusqlite::params![
                id,
                project,
                new.on.as_str(),
                new.on_id,
                on_rev,
                new.reply_to,
                new.actor,
                author,
                new.body,
                mentioned,
                now,
            ],
        )
        .db()?;
        let payload = format!(
            "{{\"id\":{},\"on\":{}{}}}",
            json_text(&id),
            json_text(&new.on_id),
            as_key(Some(&new.actor))
        );
        log_text(&tx, &project, EVENT_COMMENT_CREATED, &payload, now)?;
        // A new comment names no item yet.
        let stored = select_comment(&tx, &project, &id, Items::Skip)?
            .ok_or_else(|| refusal(&id, CommentRefusal::Unknown, no_comment(&id)))?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// An edit (task spec `comments`, "Rules and edge cases", Edits): one
    /// `Immediate` transaction. Judged under the lock, in order: the row
    /// ([`CommentRefusal::Unknown`]); an agent's, never edited, and another
    /// person's ([`CommentRefusal::Forbidden`]: on a hub its `login` against
    /// the author's, elsewhere `actor` against the row's); a deleted one, a
    /// stale `version`, a body [`body_problem`] refuses
    /// ([`CommentRefusal::Refused`]). Then the replaced body goes to
    /// `comment_versions` with the time it was written, the new body is
    /// version n + 1 with its mentions recomputed, and `comment.edited {id,
    /// version, as}` is logged.
    pub fn edit_comment(&mut self, edit: &CommentEdit, now: &str) -> Result<Comment, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        self.erasing()?;
        let tx = self.write()?;
        let id = edit.id.as_str();
        let Some(row) = select_comment(&tx, &project, id, Items::Skip)? else {
            return Err(refusal(id, CommentRefusal::Unknown, no_comment(id)));
        };
        if row.by_agent() {
            return Err(refusal(
                id,
                CommentRefusal::Forbidden,
                format!("{id} is an agent's: it is never edited, only deleted"),
            ));
        }
        let own = match &edit.login {
            Some(login) => row.author.login.as_deref() == Some(login.as_str()),
            None => row.actor == edit.actor,
        };
        if !own {
            return Err(refusal(
                id,
                CommentRefusal::Forbidden,
                format!("{id} is {}'s: only its author edits it", row.actor),
            ));
        }
        if row.deleted.is_some() {
            return Err(refusal(
                id,
                CommentRefusal::Refused,
                format!("{id} is deleted: nothing changed"),
            ));
        }
        if row.version != edit.version {
            return Err(refusal(
                id,
                CommentRefusal::Refused,
                format!(
                    "{id} is at version {}, not {}: nothing changed",
                    row.version, edit.version
                ),
            ));
        }
        if let Some(problem) = body_problem(&edit.body) {
            return Err(refusal(id, CommentRefusal::Refused, problem));
        }
        let version = row.version + 1;
        let stored_version = i64::try_from(version).map_err(|_| {
            QueueError::Invalid(format!("{id}: no version follows {}", row.version))
        })?;
        let old_version = i64::try_from(row.version).unwrap_or(i64::MAX);
        tx.execute(
            "INSERT INTO main.comment_versions (comment_id, project, version, body, written_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, project, old_version, row.body, row.updated_at],
        )
        .db()?;
        tx.execute(
            "UPDATE main.comments SET version = ?1, body = ?2, mentions = ?3, updated_at = ?4 \
             WHERE id = ?5 AND project = ?6",
            rusqlite::params![
                stored_version,
                edit.body,
                mentions_column(&edit.body)?,
                now,
                id,
                project
            ],
        )
        .db()?;
        let payload = format!(
            "{{\"id\":{},\"version\":{version}{}}}",
            json_text(id),
            as_key(Some(&edit.actor))
        );
        log_text(&tx, &project, EVENT_COMMENT_EDITED, &payload, now)?;
        let stored = select_comment(&tx, &project, id, Items::Read)?
            .ok_or_else(|| refusal(id, CommentRefusal::Unknown, no_comment(id)))?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// A delete by `by` (in words), whoever the caller judged may make it
    /// (task spec `comments`, "Delete"): `PRAGMA secure_delete` on this
    /// connection, as before every comment write, then one `Immediate`
    /// transaction that nulls the body,
    /// the mentions and every version's body, sets `deleted_by` and
    /// `deleted_at`, and logs `comment.deleted {id, as}`; once committed, a
    /// best-effort checkpoint truncates the WAL ([`Self::checkpoint_now`]).
    /// No such comment
    /// ([`CommentRefusal::Unknown`]), or one deleted already
    /// ([`CommentRefusal::Refused`]): nothing changed.
    pub fn delete_comment(&mut self, id: &str, by: &str, now: &str) -> Result<Comment, QueueError> {
        check_time(now)?;
        self.erasing()?;
        let project = self.project.clone();
        let tx = self.write()?;
        let Some(row) = select_comment(&tx, &project, id, Items::Skip)? else {
            return Err(refusal(id, CommentRefusal::Unknown, no_comment(id)));
        };
        if row.deleted.is_some() {
            return Err(refusal(
                id,
                CommentRefusal::Refused,
                format!("{id} is deleted: nothing changed"),
            ));
        }
        tx.execute(
            "UPDATE main.comments SET body = NULL, mentions = NULL, deleted_by = ?1, \
             deleted_at = ?2 WHERE id = ?3 AND project = ?4",
            [by, now, id, project.as_str()],
        )
        .db()?;
        tx.execute(
            "UPDATE main.comment_versions SET body = NULL WHERE comment_id = ?1 AND project = ?2",
            [id, project.as_str()],
        )
        .db()?;
        let payload = format!("{{\"id\":{}{}}}", json_text(id), as_key(Some(by)));
        log_text(&tx, &project, EVENT_COMMENT_DELETED, &payload, now)?;
        let stored = select_comment(&tx, &project, id, Items::Read)?
            .ok_or_else(|| refusal(id, CommentRefusal::Unknown, no_comment(id)))?;
        tx.commit().db()?;
        self.checkpoint_now();
        Ok(stored)
    }

    /// A best-effort `PRAGMA wal_checkpoint(TRUNCATE)` once a delete has
    /// committed, outside any transaction: the WAL's older frames hold the
    /// erased text until a checkpoint truncates them. It never waits: the
    /// connection's busy timeout is 0 for this call alone, then restored, so
    /// a reader or a writer active anywhere makes it give up silently (the
    /// next checkpoint, or the last connection's close, takes the frames);
    /// nothing it does or fails to do changes the delete.
    fn checkpoint_now(&self) {
        let Ok(timeout) = self
            .conn
            .pragma_query_value(None, "busy_timeout", |row| row.get::<_, i64>(0))
        else {
            return;
        };
        if self.conn.pragma_update(None, "busy_timeout", 0).is_err() {
            return;
        }
        let _ = self
            .conn
            .query_row("PRAGMA main.wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
        let _ = self.conn.pragma_update(None, "busy_timeout", timeout);
    }

    /// The project's comment `id` with its items, in one snapshot; `Ok(None)`
    /// when there is none (a queue below step 15 holds none); a row that
    /// does not decode is [`QueueError::Invalid`], named.
    pub fn get_comment(&self, id: &str) -> Result<Option<Comment>, QueueError> {
        self.in_snapshot(|conn| select_comment(conn, &self.project, id, Items::Read))
    }

    /// [`Self::get_comment`] without its items (`items` empty): what a
    /// caller judges a write or a raise on, no proposal read.
    pub fn comment_row(&self, id: &str) -> Result<Option<Comment>, QueueError> {
        self.in_snapshot(|conn| select_comment(conn, &self.project, id, Items::Skip))
    }

    /// `PRAGMA secure_delete = ON` on this handle's connection, outside any
    /// transaction, before every comment write (task spec `comments`,
    /// "Delete"): a byte a write frees, an edit's replaced body or a moved
    /// cell's old place, is zeroed in that write, so a later delete leaves no
    /// copy of any version in the file. Comment writes alone pay it: every
    /// handle is one call's, and no other write touches a comment's pages.
    fn erasing(&self) -> Result<(), QueueError> {
        Ok(self.conn.pragma_update(None, "secure_delete", "ON").db()?)
    }

    /// One page of the comments on (`on`, `on_id`), in one snapshot: the
    /// newest `limit` (at most [`COMMENT_PAGE_MAX`]) below `before` by ID
    /// number when given, printed oldest first, rows stepped one at a time;
    /// `total` every comment on the target, deleted ones included;
    /// `next_before` the page's smallest ID when older ones exist. A row that
    /// does not decode is named in [`CommentPage::unreadable`] and skipped.
    pub fn list_comments(
        &self,
        on: CommentOn,
        on_id: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Result<CommentPage, QueueError> {
        let limit = limit.clamp(1, COMMENT_PAGE_MAX);
        self.in_snapshot(|conn| {
            let mut page = CommentPage::default();
            if !has_comments(conn)? {
                return Ok(page);
            }
            let project = self.project.as_str();
            let total: i64 = conn
                .query_row(
                    "SELECT count(*) FROM main.comments \
                     WHERE project = ?1 AND on_kind = ?2 AND on_id = ?3",
                    [project, on.as_str(), on_id],
                    |row| row.get(0),
                )
                .db()?;
            page.total = u64::try_from(total).unwrap_or(0);
            let filter = below(4);
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {SELECT} FROM main.comments \
                     WHERE project = ?1 AND on_kind = ?2 AND on_id = ?3 AND {filter} \
                     ORDER BY length(id) DESC, id DESC LIMIT ?5"
                ))
                .db()?;
            let take = i64::try_from(limit + 1).unwrap_or(i64::MAX);
            let mut rows = statement
                .query(rusqlite::params![
                    project,
                    on.as_str(),
                    on_id,
                    before.unwrap_or_default(),
                    take
                ])
                .db()?;
            let mut stepped = 0;
            let mut smallest = None;
            while let Some(row) = rows.next().db()? {
                stepped += 1;
                if stepped > limit {
                    page.next_before.clone_from(&smallest);
                    break;
                }
                let raw = RawComment::read(row)?;
                smallest = Some(raw.id.clone());
                match raw.decode() {
                    Ok(comment) => page.comments.push(comment),
                    Err(unread) => page.unreadable.push(unread),
                }
            }
            drop(rows);
            page.comments.reverse();
            page.unreadable.reverse();
            attach_items(conn, project, &mut page.comments)?;
            Ok(page)
        })
    }

    /// One page of the comments whose mentions name `login` (every comment
    /// with a mention when `None`), newest first by ID number, below
    /// `before` when given, at most `limit` (at most [`COMMENT_PAGE_MAX`]),
    /// in one snapshot. The scan orders IDs and mentions alone, a stored
    /// list holding the login's JSON string a prefilter, each list then
    /// matched whole; only the page's rows are read whole, one at a time,
    /// so no body is held beyond the page (task spec `mixed-load-memory`'s
    /// lesson). `total` is the page's count. A deleted comment mentions
    /// nobody.
    pub fn comment_mentions(
        &self,
        login: Option<&str>,
        before: Option<&str>,
        limit: usize,
    ) -> Result<CommentPage, QueueError> {
        let limit = limit.clamp(1, COMMENT_PAGE_MAX);
        self.in_snapshot(|conn| {
            let mut page = CommentPage::default();
            if !has_comments(conn)? {
                return Ok(page);
            }
            let project = self.project.as_str();
            let quoted = login.map(json_text).unwrap_or_default();
            let filter = below(3);
            let mut statement = conn
                .prepare(&format!(
                    "SELECT id, mentions FROM main.comments \
                     WHERE project = ?1 AND mentions IS NOT NULL \
                     AND (?2 = '' OR instr(mentions, ?2) > 0) AND {filter} \
                     ORDER BY length(id) DESC, id DESC"
                ))
                .db()?;
            let mut rows = statement
                .query(rusqlite::params![
                    project,
                    quoted,
                    before.unwrap_or_default()
                ])
                .db()?;
            let mut ids: Vec<String> = Vec::new();
            while let Some(row) = rows.next().db()? {
                let id = row_name(row)?;
                // A list that does not decode is the row's defect: listed,
                // so its full read names it.
                let matches = match (login, row.get_ref(1).db()?) {
                    (Some(login), ValueRef::Text(bytes)) => {
                        serde_json::from_slice::<Vec<String>>(bytes)
                            .map_or(true, |listed| listed.iter().any(|named| named == login))
                    }
                    _ => true,
                };
                if !matches {
                    continue;
                }
                if ids.len() == limit {
                    page.next_before = ids.last().cloned();
                    break;
                }
                ids.push(id);
            }
            drop(rows);
            drop(statement);
            for id in &ids {
                match raw_of(conn, project, id)? {
                    Some(raw) => match raw.decode() {
                        Ok(comment) => page.comments.push(comment),
                        Err(unread) => page.unreadable.push(unread),
                    },
                    None => continue,
                }
            }
            page.total = page.comments.len() as u64;
            attach_items(conn, project, &mut page.comments)?;
            Ok(page)
        })
    }

    /// The talk on the cards `cards` an agent's context carries (task spec
    /// `comments`, "The card context and the task package"), in one
    /// snapshot: `total` their live comments, `newest` the newest live ones,
    /// at most [`COMMENT_CONTEXT_MAX`], newest first; the scan orders IDs
    /// alone and at most [`COMMENT_CONTEXT_MAX`] rows are read whole, one
    /// at a time; a row that does not decode is named and skipped, and
    /// counts among them, so no corrupt row makes it read more. A queue below
    /// step 15 holds none. A read only: no card rule asks it.
    pub fn card_comment_context(&self, cards: &[&str]) -> Result<CommentContext, QueueError> {
        self.in_snapshot(|conn| {
            let mut context = CommentContext::default();
            if cards.is_empty() || !has_comments(conn)? {
                return Ok(context);
            }
            let project = self.project.as_str();
            let slots = (0..cards.len())
                .map(|at| format!("?{}", at + 2))
                .collect::<Vec<_>>()
                .join(", ");
            let params = || std::iter::once(project).chain(cards.iter().copied());
            let total: i64 = conn
                .query_row(
                    &format!(
                        "SELECT count(*) FROM main.comments WHERE project = ?1 \
                         AND on_kind = 'card' AND on_id IN ({slots}) AND deleted_at IS NULL"
                    ),
                    rusqlite::params_from_iter(params()),
                    |row| row.get(0),
                )
                .db()?;
            context.total = u64::try_from(total).unwrap_or(0);
            let mut statement = conn
                .prepare(&format!(
                    "SELECT id FROM main.comments WHERE project = ?1 \
                     AND on_kind = 'card' AND on_id IN ({slots}) AND deleted_at IS NULL \
                     ORDER BY length(id) DESC, id DESC"
                ))
                .db()?;
            let mut rows = statement.query(rusqlite::params_from_iter(params())).db()?;
            // At most the context's count of rows is read whole, a corrupt
            // one among them: no row past them is read.
            let mut stepped = 0;
            while let Some(row) = rows.next().db()? {
                if stepped == COMMENT_CONTEXT_MAX {
                    break;
                }
                stepped += 1;
                let id = row_name(row)?;
                let Some(raw) = raw_of(conn, project, &id)? else {
                    continue;
                };
                match raw.decode() {
                    Ok(comment) => context.newest.push(comment),
                    Err(unread) => context.unreadable.push(unread),
                }
            }
            Ok(context)
        })
    }
}
