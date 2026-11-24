//! References in the project's queue (task spec `work-references`, "Data";
//! `docs/canon/work-references.md`; ADR-0088): files and links attached to
//! a card, a proposal or a node, people's material and never truth. Step 16
//! adds `refs`. Every change is one `Immediate` transaction holding its one
//! row change and its one `reference.*` event; a delete is an `UPDATE` that
//! erases the row's texts, hashes and facts under `secure_delete`, as a
//! comment's. No card, proposal, task or lock row is written here, and
//! nothing outside this file reads `refs` but the dump: no card move,
//! accept, stage, decision, gate or check (ADR-0012).
//!
//! - **IDs**: `RF-` and 4 or more digits, the highest of the table plus one
//!   in the inserting transaction; rows are never removed, so an ID is never
//!   reused.
//! - **Files**: a row names its file's BLAKE3 (`b3`) and its agent copy's
//!   (`agent_b3`); the store's files themselves are the CLI's
//!   (`blob_store.rs`). [`SqliteQueue::blob_named`] is the indexed lookup a
//!   sweep and a delete ask under the turn.
//! - **Reads** step their rows one at a time and page at
//!   [`REFERENCE_PAGE_MAX`]; a row that does not decode is named and
//!   skipped.

use rusqlite::types::ValueRef;
use std::path::Path;

use rusqlite::config::DbConfig;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, TransactionBehavior};
use specengine_core::blob::{Facts, Media, is_b3};
use specengine_core::comment::CommentOn;
use specengine_core::proposal::{Author, author_field_problem, is_utc_timestamp};
use specengine_core::reference::{
    CONTEXT_REFS, REFERENCE_PAGE_MAX, ReferenceKind, reference_id, reference_number,
};

use super::comments::CommentRefusal;
use super::stage::{as_key, json_text};
use super::state::stored_text;
use super::{
    EVENT_CREATED, ID_ORDER, NewProposal, Proposal, ProposalKind, QUEUE_SCHEMA_VERSION, QueueError,
    REFS_SCHEMA, SqliteQueue, UnreadableRow, check_time, corrupt, existing, index_only,
    insert_update, log_text, new_proposal_problem, tasks, to_json, user_version,
};
use crate::error::{Db, StoreError};
use crate::floor::read_floor;
use crate::schema;

/// `reference.created`, with `id`, `on` (the target's ID) and `as`.
pub const EVENT_REFERENCE_CREATED: &str = "reference.created";
/// `reference.deleted`, with `id` and `as`.
pub const EVENT_REFERENCE_DELETED: &str = "reference.deleted";

/// Every column of `refs`, in table order: `on_rev` and `size` `INTEGER`,
/// the rest `TEXT`.
pub const REF_COLUMNS: [&str; 20] = [
    "id",
    "project",
    "on_kind",
    "on_id",
    "on_rev",
    "actor",
    "author",
    "kind",
    "title",
    "matters",
    "url",
    "name",
    "b3",
    "agent_b3",
    "size",
    "media",
    "meta",
    "created_at",
    "deleted_by",
    "deleted_at",
];

/// `on_rev` among [`REF_COLUMNS`].
const ON_REV_COLUMN: usize = 4;

/// `size` among [`REF_COLUMNS`].
const SIZE_COLUMN: usize = 14;

/// The select list of `refs`.
const SELECT: &str = "id, project, on_kind, on_id, on_rev, actor, author, kind, title, matters, \
                      url, name, b3, agent_b3, size, media, meta, created_at, deleted_by, \
                      deleted_at";

/// Rows of an ID below `?N`'s by number, every row when `?N` is empty.
fn below(slot: usize) -> String {
    format!(
        "(?{slot} = '' OR length(id) < length(?{slot}) \
         OR (length(id) = length(?{slot}) AND id < ?{slot}))"
    )
}

/// What an attach stores; the queue adds the ID, project and time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewReference {
    pub on: CommentOn,
    /// Canonical: `C-0007`, `PR-0031`, `MEC-RAIL`, `docs/spec/rail.md`.
    pub on_id: String,
    /// A node's revision when attached; `None` on a card or a proposal.
    pub on_rev: Option<u64>,
    /// In words: the login on a hub, the typed name, `local`, `agent`.
    pub actor: String,
    pub author: Author,
    pub kind: ReferenceKind,
    pub title: Option<String>,
    pub matters: Option<String>,
    /// A link's.
    pub url: Option<String>,
    /// A file's: its name, BLAKE3, agent copy, size, type and facts.
    pub name: Option<String>,
    pub b3: Option<String>,
    pub agent_b3: Option<String>,
    pub size: Option<u64>,
    pub media: Option<Media>,
    pub meta: Option<Facts>,
}

/// A delete's tombstone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDeleted {
    pub by: String,
    pub at: String,
}

/// A stored reference, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub id: String,
    pub project: String,
    pub on: CommentOn,
    pub on_id: String,
    pub on_rev: Option<u64>,
    pub actor: String,
    pub author: Author,
    pub kind: ReferenceKind,
    /// `None` once deleted.
    pub title: Option<String>,
    pub matters: Option<String>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub b3: Option<String>,
    pub agent_b3: Option<String>,
    pub size: Option<u64>,
    pub media: Option<Media>,
    pub meta: Option<Facts>,
    pub created_at: String,
    pub deleted: Option<ReferenceDeleted>,
}

/// One page of a target's references, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferencePage {
    /// Every reference on the target, deleted ones included.
    pub total: u64,
    pub references: Vec<Reference>,
    /// The page's smallest ID when older references exist.
    pub next_before: Option<String>,
    pub unreadable: Vec<UnreadableRow>,
}

/// What a card's context, or a task package, carries of its cards'
/// references.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferenceContext {
    /// The live references on the cards.
    pub total: u64,
    /// The newest live ones, newest first, at most [`CONTEXT_REFS`].
    pub newest: Vec<Reference>,
    pub unreadable: Vec<UnreadableRow>,
}

/// A delete done: the tombstone, and each of the row's former files no
/// live reference of the project names now, for the caller to unlink in
/// the same turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceDelete {
    pub reference: Reference,
    pub unnamed: Vec<String>,
}

/// A `refs` row as stored, never decoded: `on_rev`, `size`, and the `TEXT`
/// columns around them in [`REF_COLUMNS`] order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRef {
    pub on_rev: Option<i64>,
    pub size: Option<i64>,
    pub columns: [Option<String>; REF_COLUMNS.len() - 2],
}

impl StoredRef {
    /// The stored `id`.
    pub fn id(&self) -> Option<&str> {
        self.columns[0].as_deref()
    }

    /// The stored `project`.
    pub fn project(&self) -> Option<&str> {
        self.columns[1].as_deref()
    }
}

fn refusal(id: &str, cause: CommentRefusal, reason: String) -> QueueError {
    QueueError::ReferenceRefused {
        id: id.to_owned(),
        reason,
        cause,
    }
}

/// `no reference RF-0099`.
fn no_reference(id: &str) -> String {
    format!("no reference {id}")
}

/// The queue holds `refs` (a handle of [`SqliteQueue::open_existing`] may
/// stand below step 16).
pub(super) fn has_refs(conn: &Connection) -> Result<bool, QueueError> {
    if index_only(conn)? || user_version(conn)? < REFS_SCHEMA {
        return Ok(false);
    }
    Ok(schema::has_table(conn, "refs")?)
}

/// Every row of `refs`, as stored, by ID number, inside `conn`'s open
/// transaction.
pub(super) fn stored_in(conn: &Connection) -> Result<Vec<StoredRef>, QueueError> {
    let mut stored = Vec::new();
    if !schema::has_table(conn, "refs")? {
        return Ok(stored);
    }
    let mut statement = conn
        .prepare(&format!(
            "SELECT {} FROM main.refs {ID_ORDER}",
            REF_COLUMNS.join(", ")
        ))
        .db()?;
    let mut rows = statement.query([]).db()?;
    while let Some(row) = rows.next().db()? {
        stored.push(stored_ref(row)?);
    }
    Ok(stored)
}

/// `refs` holds a row (any project's).
pub(super) fn any_in(conn: &Connection) -> Result<bool, QueueError> {
    if !schema::has_table(conn, "refs")? {
        return Ok(false);
    }
    let found: Option<i64> = conn
        .query_row("SELECT 1 FROM main.refs LIMIT 1", [], |row| row.get(0))
        .optional()
        .db()?;
    Ok(found.is_some())
}

/// Inserts `stored` as given, inside the restore's transaction.
pub(super) fn insert_stored(tx: &Transaction<'_>, stored: &[StoredRef]) -> Result<(), QueueError> {
    let slots: Vec<String> = (1..=REF_COLUMNS.len())
        .map(|number| format!("?{number}"))
        .collect();
    let mut insert = tx
        .prepare(&format!(
            "INSERT INTO main.refs ({}) VALUES ({})",
            REF_COLUMNS.join(", "),
            slots.join(", ")
        ))
        .db()?;
    let integer = |value: Option<i64>| {
        value.map_or(
            rusqlite::types::Value::Null,
            rusqlite::types::Value::Integer,
        )
    };
    for row in stored {
        let mut values: Vec<rusqlite::types::Value> = row
            .columns
            .iter()
            .map(|value| match value {
                Some(text) => rusqlite::types::Value::Text(text.clone()),
                None => rusqlite::types::Value::Null,
            })
            .collect();
        values.insert(ON_REV_COLUMN, integer(row.on_rev));
        values.insert(SIZE_COLUMN, integer(row.size));
        insert.execute(rusqlite::params_from_iter(values)).db()?;
    }
    Ok(())
}

/// The name of a row's first column, as far as it reads.
fn row_name(row: &Row<'_>) -> Result<String, QueueError> {
    Ok(match row.get_ref(0).db()? {
        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        _ => "NULL".to_owned(),
    })
}

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

fn stored_ref(row: &Row<'_>) -> Result<StoredRef, QueueError> {
    let name = row_name(row)?;
    let what = format!("reference {name}");
    let on_rev = stored_integer(row, ON_REV_COLUMN, &what, "on_rev")?;
    let size = stored_integer(row, SIZE_COLUMN, &what, "size")?;
    let mut columns: [Option<String>; REF_COLUMNS.len() - 2] = Default::default();
    let texts = REF_COLUMNS
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != ON_REV_COLUMN && *index != SIZE_COLUMN);
    for (slot, (index, column)) in columns.iter_mut().zip(texts) {
        *slot = stored_text(row, index)?.map_err(|why| {
            QueueError::Store(StoreError::Sqlite(format!(
                "{what}: the stored `{column}` cannot be read: {why}"
            )))
        })?;
    }
    Ok(StoredRef {
        on_rev,
        size,
        columns,
    })
}

/// A `refs` row as read, before decoding.
struct RawRef {
    id: String,
    text: Vec<Option<String>>,
    on_rev: Option<i64>,
    size: Option<i64>,
    broken: Option<(&'static str, &'static str)>,
}

impl RawRef {
    fn read(row: &Row<'_>) -> Result<Self, QueueError> {
        let id = row_name(row)?;
        let mut text = Vec::with_capacity(REF_COLUMNS.len());
        let mut broken = None;
        let (mut on_rev, mut size) = (None, None);
        for (index, column) in REF_COLUMNS.iter().enumerate() {
            let value = row.get_ref(index).db()?;
            if index == ON_REV_COLUMN || index == SIZE_COLUMN {
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
                    size = number;
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
            size,
            broken,
        })
    }

    /// The reference; else the row named.
    fn decode(self) -> Result<Reference, UnreadableRow> {
        let id = self.id;
        if let Some((column, why)) = self.broken {
            return Err(corrupt(&id, column, why));
        }
        let mut values = self.text.into_iter();
        let mut take = || values.next().flatten();
        let required = |value: Option<String>, column: &str| {
            value.ok_or_else(|| corrupt(&id, column, "it is NULL"))
        };
        if reference_number(&id).is_none() {
            return Err(corrupt(&id, "id", format!("`{id}` is no reference ID")));
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
            (_, Some(_)) => {
                return Err(corrupt(
                    &id,
                    "on_rev",
                    format!("it is set on a {}, which has no revision", on.as_str()),
                ));
            }
            (_, None) => None,
        };
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
        let kind_text = required(take(), "kind")?;
        let kind = ReferenceKind::parse(&kind_text)
            .ok_or_else(|| corrupt(&id, "kind", format!("`{kind_text}` is no reference kind")))?;
        let title = take();
        let matters = take();
        let url = take();
        let name = take();
        let b3 = take();
        let agent_b3 = take();
        for (column, hash) in [("b3", &b3), ("agent_b3", &agent_b3)] {
            if let Some(hash) = hash
                && !is_b3(hash)
            {
                return Err(corrupt(&id, column, "it is no BLAKE3 of 64 lower-case hex"));
            }
        }
        let _size = take();
        let size = match self.size {
            None => None,
            Some(size) => Some(
                u64::try_from(size)
                    .map_err(|_| corrupt(&id, "size", format!("{size} is no size")))?,
            ),
        };
        let media = match take() {
            None => None,
            Some(text) => Some(
                Media::parse(&text)
                    .ok_or_else(|| corrupt(&id, "media", format!("`{text}` is no stored type")))?,
            ),
        };
        let meta = match take() {
            None => None,
            Some(text) => Some(
                serde_json::from_str::<Facts>(&text)
                    .map_err(|error| corrupt(&id, "meta", error))?,
            ),
        };
        let created_at = required(take(), "created_at")?;
        if !is_utc_timestamp(&created_at) {
            return Err(corrupt(
                &id,
                "created_at",
                format!("{created_at:?} is no UTC time stamp"),
            ));
        }
        let deleted = match (take(), take()) {
            (Some(by), Some(at)) => Some(ReferenceDeleted { by, at }),
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
        if deleted.is_none() {
            let held = match kind {
                ReferenceKind::Link => url.is_some() && b3.is_none(),
                ReferenceKind::File => {
                    b3.is_some() && url.is_none() && size.is_some() && media.is_some()
                }
            };
            if !held {
                return Err(corrupt(
                    &id,
                    "kind",
                    format!("a live {} lacks its columns", kind.as_str()),
                ));
            }
        } else if [&title, &matters, &url, &name, &b3, &agent_b3]
            .iter()
            .any(|value| value.is_some())
            || meta.is_some()
        {
            return Err(corrupt(
                &id,
                "deleted_at",
                "a deleted reference keeps a text",
            ));
        }
        Ok(Reference {
            id,
            project,
            on,
            on_id,
            on_rev,
            actor,
            author,
            kind,
            title,
            matters,
            url,
            name,
            b3,
            agent_b3,
            size,
            media,
            meta,
            created_at,
            deleted,
        })
    }
}

/// The row of the reference `id` of `project`, not decoded.
fn raw_of(conn: &Connection, project: &str, id: &str) -> Result<Option<RawRef>, QueueError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT {SELECT} FROM main.refs WHERE id = ?1 AND project = ?2"
        ))
        .db()?;
    let mut rows = statement.query([id, project]).db()?;
    match rows.next().db()? {
        Some(row) => Ok(Some(RawRef::read(row)?)),
        None => Ok(None),
    }
}

/// The reference `id` of `project`; a row that does not decode is
/// [`QueueError::Invalid`] naming it.
fn select_reference(
    conn: &Connection,
    project: &str,
    id: &str,
) -> Result<Option<Reference>, QueueError> {
    if !has_refs(conn)? {
        return Ok(None);
    }
    match raw_of(conn, project, id)? {
        Some(raw) => raw
            .decode()
            .map(Some)
            .map_err(|row| QueueError::Invalid(unreadable_line(&row))),
        None => Ok(None),
    }
}

/// `RF-0012: its row does not read (<column>: <reason>); not listed`.
pub fn unreadable_line(row: &UnreadableRow) -> String {
    format!(
        "{}: its row does not read ({}: {}); not listed",
        row.id, row.column, row.reason
    )
}

/// The next reference ID: the highest number stored, plus one.
fn next_reference_id(tx: &Transaction<'_>) -> Result<String, QueueError> {
    let mut statement = tx.prepare("SELECT id FROM main.refs").db()?;
    let mut rows = statement.query([]).db()?;
    let mut highest = 0;
    while let Some(row) = rows.next().db()? {
        let id: Option<String> = row.get(0).db()?;
        if let Some(number) = id.as_deref().and_then(reference_number) {
            highest = highest.max(number);
        }
    }
    let next = highest.checked_add(1).ok_or_else(|| {
        QueueError::Store(StoreError::Sqlite(format!(
            "the queue holds `{}`: no reference ID follows it",
            reference_id(highest)
        )))
    })?;
    Ok(reference_id(next))
}

/// A live reference of `project` names `hash`, as its file or its copy.
fn named_in(conn: &Connection, project: &str, hash: &str) -> Result<bool, QueueError> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM main.refs WHERE project = ?1 AND deleted_at IS NULL \
             AND (b3 = ?2 OR agent_b3 = ?2) LIMIT 1",
            [project, hash],
            |row| row.get(0),
        )
        .optional()
        .db()?;
    Ok(found.is_some())
}

/// What a blob read finds of a file: the reference that serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobServed {
    /// The lowest live reference naming the file.
    pub id: String,
    /// It names the file as its agent copy, not as its file.
    pub copy: bool,
    pub media: Option<Media>,
    pub name: Option<String>,
}

/// What serves `hash` (task spec `work-references`, "Serving"): the
/// project's lowest live reference naming it as `b3` or `agent_b3`, its type
/// and name when it names the file, `copy` when it names the agent copy;
/// `None` when no live reference names it (or the queue holds no `refs`).
fn served_in(
    conn: &Connection,
    project: &str,
    hash: &str,
) -> Result<Option<BlobServed>, QueueError> {
    if !has_refs(conn)? {
        return Ok(None);
    }
    let mut statement = conn
        .prepare(
            "SELECT id, b3, media, name FROM main.refs WHERE project = ?1 \
             AND deleted_at IS NULL AND (b3 = ?2 OR agent_b3 = ?2) \
             ORDER BY length(id), id LIMIT 1",
        )
        .db()?;
    let mut rows = statement.query([project, hash]).db()?;
    let Some(row) = rows.next().db()? else {
        return Ok(None);
    };
    let id: String = row.get(0).db()?;
    let b3: Option<String> = row.get(1).db()?;
    let media: Option<String> = row.get(2).db()?;
    let name: Option<String> = row.get(3).db()?;
    let copy = b3.as_deref() != Some(hash);
    Ok(Some(BlobServed {
        id,
        copy,
        media: if copy {
            None
        } else {
            media.as_deref().and_then(Media::parse)
        },
        name: if copy { None } else { name },
    }))
}

/// A blob read's lookup (task spec `work-references`, "Serving"): a
/// read-only connection of its own, opened outside the project's turn,
/// nothing created, no schema step, no checkpoint when closed.
pub struct BlobReader {
    conn: Connection,
    project: String,
}

impl BlobReader {
    /// Opens `db` read-only; `None` when there is no such file. A newer
    /// build's queue is [`QueueError::SchemaTooNew`].
    pub fn open(db: &Path, project: &str) -> Result<Option<Self>, QueueError> {
        match std::fs::metadata(db) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(StoreError::io(db, error).into()),
        }
        let conn = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .db()?;
        conn.pragma_update(None, "busy_timeout", schema::BUSY_TIMEOUT_MS)
            .db()?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
            .db()?;
        let version = user_version(&conn)?;
        if version > QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        Ok(Some(Self {
            conn,
            project: project.to_owned(),
        }))
    }

    /// What serves `hash`, in one read transaction.
    pub fn served(&self, hash: &str) -> Result<Option<BlobServed>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let served = served_in(&tx, &self.project, hash)?;
        tx.commit().db()?;
        Ok(served)
    }
}

impl SqliteQueue {
    /// Stores `new` under the next ID, `created_at` `now`; logs
    /// `reference.created {id, on, as}`, no title, URL, name, hash or "what
    /// matters". Its card or proposal must be a row of the project (any
    /// state), read under this lock: else [`QueueError::ReferenceRefused`],
    /// nothing written, no ID taken. A node is not checked here: the caller
    /// resolved it. The caller judged the texts and the file before.
    pub fn add_reference(
        &mut self,
        new: &NewReference,
        now: &str,
    ) -> Result<Reference, QueueError> {
        check_time(now)?;
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
        if let Some(problem) = [&new.author.role, &new.author.model, &new.author.run]
            .into_iter()
            .flatten()
            .find_map(|field| author_field_problem(field))
        {
            return Err(QueueError::Invalid(format!("author: {problem}")));
        }
        let on_rev = new
            .on_rev
            .map(|rev| {
                i64::try_from(rev)
                    .map_err(|_| QueueError::Invalid(format!("on: revision {rev} is too large")))
            })
            .transpose()?;
        let size = new
            .size
            .map(|size| {
                i64::try_from(size)
                    .map_err(|_| QueueError::Invalid(format!("size: {size} is too large")))
            })
            .transpose()?;
        let author = to_json(&new.author)?;
        let meta = new.meta.as_ref().map(to_json).transpose()?;
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
        let id = next_reference_id(&tx)?;
        tx.execute(
            &format!(
                "INSERT INTO main.refs ({SELECT}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
                 ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, NULL, NULL)"
            ),
            rusqlite::params![
                id,
                project,
                new.on.as_str(),
                new.on_id,
                on_rev,
                new.actor,
                author,
                new.kind.as_str(),
                new.title,
                new.matters,
                new.url,
                new.name,
                new.b3,
                new.agent_b3,
                size,
                new.media.map(Media::as_str),
                meta,
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
        log_text(&tx, &project, EVENT_REFERENCE_CREATED, &payload, now)?;
        let stored = select_reference(&tx, &project, &id)?
            .ok_or_else(|| refusal(&id, CommentRefusal::Unknown, no_reference(&id)))?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// A delete by `by` (in words), whoever the caller judged may make it:
    /// `PRAGMA secure_delete`, then one `Immediate` transaction that nulls
    /// the row's title, "what matters", URL, name, hashes and facts, sets
    /// `deleted_by` and `deleted_at`, logs `reference.deleted {id, as}`,
    /// and reads which of its former files no live reference of the project
    /// names now ([`ReferenceDelete::unnamed`], for the caller to unlink in
    /// the same turn); once committed, a best-effort WAL truncate. No such
    /// reference ([`CommentRefusal::Unknown`]) or one deleted already
    /// ([`CommentRefusal::Refused`]): nothing changed.
    pub fn delete_reference(
        &mut self,
        id: &str,
        by: &str,
        now: &str,
    ) -> Result<ReferenceDelete, QueueError> {
        check_time(now)?;
        self.erasing()?;
        let project = self.project.clone();
        let tx = self.write()?;
        let Some(row) = select_reference(&tx, &project, id)? else {
            return Err(refusal(id, CommentRefusal::Unknown, no_reference(id)));
        };
        if row.deleted.is_some() {
            return Err(refusal(
                id,
                CommentRefusal::Refused,
                format!("{id} is deleted: nothing changed"),
            ));
        }
        tx.execute(
            "UPDATE main.refs SET title = NULL, matters = NULL, url = NULL, name = NULL, \
             b3 = NULL, agent_b3 = NULL, meta = NULL, deleted_by = ?1, deleted_at = ?2 \
             WHERE id = ?3 AND project = ?4",
            [by, now, id, project.as_str()],
        )
        .db()?;
        let payload = format!("{{\"id\":{}{}}}", json_text(id), as_key(Some(by)));
        log_text(&tx, &project, EVENT_REFERENCE_DELETED, &payload, now)?;
        let mut unnamed = Vec::new();
        for hash in [&row.b3, &row.agent_b3].into_iter().flatten() {
            if !unnamed.contains(hash) && !named_in(&tx, &project, hash)? {
                unnamed.push(hash.clone());
            }
        }
        let stored = select_reference(&tx, &project, id)?
            .ok_or_else(|| refusal(id, CommentRefusal::Unknown, no_reference(id)))?;
        tx.commit().db()?;
        self.checkpoint_now();
        Ok(ReferenceDelete {
            reference: stored,
            unnamed,
        })
    }

    /// Stores `proposal`, a `reference` (task spec `reference-promotion`,
    /// "Queue schema 17"), as `open` under the next ID with `ref_id`
    /// `reference`, bound to `task` when given, and logs `proposal.created`,
    /// in one `Immediate` transaction. Read inside it: the reference, which
    /// must be the project's, live and a file
    /// ([`QueueError::ReferenceRefused`]: `no reference RF-0099`, `RF-0012
    /// is deleted: nothing to promote`, `RF-0010 is a link: only a file is
    /// promoted into git`), and the live twin: an `open` or `approved`
    /// promotion of the same reference to the same document refuses
    /// (`RF-0012 is already proposed for `<ID>` by PR-0042 (open)`).
    /// Nothing written on any refusal.
    #[allow(clippy::too_many_arguments)]
    pub fn create_promotion(
        &mut self,
        proposal: &NewProposal,
        reference: &str,
        task: Option<&str>,
        actor: Option<&str>,
        now: &str,
    ) -> Result<Proposal, QueueError> {
        check_time(now)?;
        if proposal.kind != ProposalKind::Reference {
            return Err(QueueError::Invalid(format!(
                "`create_promotion` stores a reference, not {}",
                proposal.kind.with_article()
            )));
        }
        if reference_number(reference).is_none() {
            return Err(QueueError::Invalid(format!(
                "`{reference}` is no reference ID"
            )));
        }
        if let Some(problem) = new_proposal_problem(proposal) {
            return Err(QueueError::Invalid(problem));
        }
        let project = self.project.clone();
        let dir = self.dir.clone();
        let tx = self.write()?;
        if let Some(task) = task {
            tasks::check_binding(&tx, &project, task, &proposal.place)?;
        }
        let stored = select_reference(&tx, &project, reference)?;
        let Some(stored) = stored else {
            return Err(refusal(
                reference,
                CommentRefusal::Unknown,
                no_reference(reference),
            ));
        };
        if stored.deleted.is_some() {
            return Err(refusal(
                reference,
                CommentRefusal::Refused,
                format!("{reference} is deleted: nothing to promote"),
            ));
        }
        if stored.kind != ReferenceKind::File {
            return Err(refusal(
                reference,
                CommentRefusal::Refused,
                format!("{reference} is a link: only a file is promoted into git"),
            ));
        }
        if stored.b3.is_none() {
            return Err(refusal(
                reference,
                CommentRefusal::Refused,
                format!("{reference}'s file is not on this host: nothing to promote"),
            ));
        }
        let twin: Option<(String, String)> = tx
            .query_row(
                &format!(
                    "SELECT id, status FROM main.proposals WHERE project = ?1 AND ref_id = ?2 \
                     AND target_path = ?3 AND status IN ('open', 'approved') {ID_ORDER} LIMIT 1"
                ),
                (&project, reference, &proposal.target_path),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .db()?;
        if let Some((twin, status)) = twin {
            return Err(refusal(
                reference,
                CommentRefusal::Refused,
                format!(
                    "{reference} is already proposed for `{}` by {twin} ({status})",
                    proposal.target_id
                ),
            ));
        }
        let floor = read_floor(&dir)?;
        let id = insert_update(
            &tx,
            &project,
            proposal,
            None,
            task,
            Some(reference),
            floor,
            now,
        )?;
        // A page's promotion names its person as a raise's `as` does.
        let payload = format!("{{\"id\":{}{}}}", json_text(&id), as_key(actor));
        log_text(&tx, &project, EVENT_CREATED, &payload, now)?;
        let created = existing(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok(created)
    }

    /// The project's reference `id`, in one snapshot; `Ok(None)` when there
    /// is none (a queue below step 16 holds none).
    pub fn get_reference(&self, id: &str) -> Result<Option<Reference>, QueueError> {
        self.in_snapshot(|conn| select_reference(conn, &self.project, id))
    }

    /// A live reference of the project names `hash` as its file or its agent
    /// copy: the indexed lookup a sweep and a delete ask in the turn.
    pub fn blob_named(&self, hash: &str) -> Result<bool, QueueError> {
        self.in_snapshot(|conn| {
            if !has_refs(conn)? {
                return Ok(false);
            }
            named_in(conn, &self.project, hash)
        })
    }

    /// What serves `hash` ([`served_in`]) through this handle, in one
    /// snapshot. A daemon's blob read uses [`BlobReader`] instead.
    pub fn blob_served(&self, hash: &str) -> Result<Option<BlobServed>, QueueError> {
        self.in_snapshot(|conn| served_in(conn, &self.project, hash))
    }

    /// Each live row's `b3` and `agent_b3` of the project handed to `seen`,
    /// rows stepped one at a time in one snapshot (a sweep's one read).
    pub fn each_live_blob(&self, mut seen: impl FnMut(&str)) -> Result<(), QueueError> {
        self.in_snapshot(|conn| {
            if !has_refs(conn)? {
                return Ok(());
            }
            let mut statement = conn
                .prepare(
                    "SELECT b3, agent_b3 FROM main.refs WHERE project = ?1 \
                     AND deleted_at IS NULL",
                )
                .db()?;
            let mut rows = statement.query([self.project.as_str()]).db()?;
            while let Some(row) = rows.next().db()? {
                for index in 0..2 {
                    if let ValueRef::Text(bytes) = row.get_ref(index).db()?
                        && let Ok(hash) = std::str::from_utf8(bytes)
                    {
                        seen(hash);
                    }
                }
            }
            Ok(())
        })
    }

    /// One page of the references on (`on`, `on_id`), in one snapshot: the
    /// newest `limit` (at most [`REFERENCE_PAGE_MAX`]) below `before` by ID
    /// number when given, printed oldest first, rows stepped one at a time;
    /// `total` every reference on the target, deleted ones included.
    pub fn list_references(
        &self,
        on: CommentOn,
        on_id: &str,
        before: Option<&str>,
        limit: usize,
    ) -> Result<ReferencePage, QueueError> {
        let limit = limit.clamp(1, REFERENCE_PAGE_MAX);
        self.in_snapshot(|conn| {
            let mut page = ReferencePage::default();
            if !has_refs(conn)? {
                return Ok(page);
            }
            let project = self.project.as_str();
            let total: i64 = conn
                .query_row(
                    "SELECT count(*) FROM main.refs \
                     WHERE project = ?1 AND on_kind = ?2 AND on_id = ?3",
                    [project, on.as_str(), on_id],
                    |row| row.get(0),
                )
                .db()?;
            page.total = u64::try_from(total).unwrap_or(0);
            let filter = below(4);
            let mut statement = conn
                .prepare(&format!(
                    "SELECT {SELECT} FROM main.refs \
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
                let raw = RawRef::read(row)?;
                smallest = Some(raw.id.clone());
                match raw.decode() {
                    Ok(reference) => page.references.push(reference),
                    Err(unread) => page.unreadable.push(unread),
                }
            }
            drop(rows);
            page.references.reverse();
            page.unreadable.reverse();
            Ok(page)
        })
    }

    /// The references on the cards `cards` an agent's context carries, in
    /// one snapshot: `total` their live references, `newest` the newest
    /// live ones, at most [`CONTEXT_REFS`], newest first; the scan orders
    /// IDs alone and at most [`CONTEXT_REFS`] rows are read whole. A read
    /// only: no card rule asks it.
    pub fn card_reference_context(&self, cards: &[&str]) -> Result<ReferenceContext, QueueError> {
        self.in_snapshot(|conn| {
            let mut context = ReferenceContext::default();
            if cards.is_empty() || !has_refs(conn)? {
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
                        "SELECT count(*) FROM main.refs WHERE project = ?1 \
                         AND on_kind = 'card' AND on_id IN ({slots}) AND deleted_at IS NULL"
                    ),
                    rusqlite::params_from_iter(params()),
                    |row| row.get(0),
                )
                .db()?;
            context.total = u64::try_from(total).unwrap_or(0);
            let mut statement = conn
                .prepare(&format!(
                    "SELECT id FROM main.refs WHERE project = ?1 \
                     AND on_kind = 'card' AND on_id IN ({slots}) AND deleted_at IS NULL \
                     ORDER BY length(id) DESC, id DESC"
                ))
                .db()?;
            let mut rows = statement.query(rusqlite::params_from_iter(params())).db()?;
            let mut stepped = 0;
            while let Some(row) = rows.next().db()? {
                if stepped == CONTEXT_REFS {
                    break;
                }
                stepped += 1;
                let id = row_name(row)?;
                let Some(raw) = raw_of(conn, project, &id)? else {
                    continue;
                };
                match raw.decode() {
                    Ok(reference) => context.newest.push(reference),
                    Err(unread) => context.unreadable.push(unread),
                }
            }
            Ok(context)
        })
    }
}
