//! Cards in the project's queue (task spec `card-store`, "Store",
//! "Documents and events"; ADR-0056): the lighter record of the work of any
//! trade. Every change is one `Immediate` transaction holding the change and
//! its one event, under the rules of core's [`card_move`],
//! [`accept_refusal`] and [`closed_refusal`], asked again here, under the
//! lock. A `seen` revision makes a change a compare-and-set
//! ([`SqliteQueue::change_card`]). Nothing here reads a task, a proposal or a
//! lock, and the gate's [`crate::queue::GateQueue`] never reads a card: a
//! corrupt card row is named here and nowhere else.
//!
//! - **Order**: `position` dense from 1 per (`flow`, `col`). `new` and a move
//!   go last; a `before` takes that card's place; both columns stay dense. A
//!   shifted neighbour takes no `revision`, no `updated_at` and no event, so
//!   it loses no compare-and-set. A cancelled card leaves its column:
//!   `position` 0.
//! - **IDs**: `C-` and 4 or more digits, the highest plus one in the
//!   transaction, rows never deleted, so an ID is never reused.
//! - **Columns**: `targets`, `criteria` JSON string lists, `evidence`
//!   `[{kind, value, by, at}]`, `acceptance` `{by, at, note}`, `cancellation`
//!   `{by, at, reason}`, `snapshot` `NULL`. A value that does not decode is a
//!   corrupt row, named ([`UnreadableCard`]).

use std::collections::BTreeSet;
use std::fmt;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use specengine_core::card::{
    EVIDENCE_ENTRIES_MAX, accept_refusal, card_id, card_move, card_number, closed_refusal,
    reviewer_problem, stale_refusal,
};
use specengine_core::card_toml::CardFlow;
use specengine_model::{Card, CardAcceptance, CardCancellation, CardEvidence, EvidenceKind};

use super::state::stored_text;
use super::{CARD_COLUMNS, Event, ID_ORDER, QueueError, SqliteQueue, check_time, log, to_json};
use crate::error::{Db, StoreError};

/// `card.created`, with `flow`, `column`, `position`, `assignee`, `as`.
pub const EVENT_CARD_CREATED: &str = "card.created";
/// `card.moved`, with `from`, `to`, `position`, `as`.
pub const EVENT_CARD_MOVED: &str = "card.moved";
/// `card.edited`, with `fields`, `as`.
pub const EVENT_CARD_EDITED: &str = "card.edited";
/// `card.assigned`, with `assignee`, `reviewer`, `as`.
pub const EVENT_CARD_ASSIGNED: &str = "card.assigned";
/// `card.cancelled`, with `column`, `as`.
pub const EVENT_CARD_CANCELLED: &str = "card.cancelled";
/// `card.accepted`, with `from`, `to`, `position`, `as`.
pub const EVENT_CARD_ACCEPTED: &str = "card.accepted";

/// Every column of `cards` in table order, for a statement.
const CARD_SELECT: &str = "id, project, flow, col, position, title, goal, discipline, targets, \
     snapshot, criteria, assignee, reviewer, estimate, due, parent, corrects, evidence, \
     acceptance, cancellation, created_by, created_at, updated_at, revision";

/// `position` among the columns; the one that is no `TEXT`.
pub(super) const POSITION_COLUMN: usize = 4;

/// What `spec card new` stores; the queue adds the ID, project, position and
/// times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCard {
    pub flow: String,
    /// The flow's first column.
    pub column: String,
    pub title: String,
    pub goal: Option<String>,
    pub discipline: Option<String>,
    /// Canonical IDs or paths, each once; none on a manual card.
    pub targets: Vec<String>,
    pub criteria: Vec<String>,
    pub assignee: Option<String>,
    pub estimate: Option<String>,
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// Who acts: `created_by`.
    pub by: String,
}

/// What `spec card edit` changes: `None` leaves a field, `Some(None)` clears
/// it; `evidence` appends.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardEdit {
    pub title: Option<String>,
    pub goal: Option<Option<String>>,
    pub targets: Option<Vec<String>>,
    pub criteria: Option<Vec<String>>,
    pub estimate: Option<Option<String>>,
    pub due: Option<Option<String>>,
    pub parent: Option<Option<String>>,
    pub corrects: Option<Option<String>>,
    pub evidence: Vec<(EvidenceKind, String)>,
}

impl CardEdit {
    /// The columns this edit writes, in table order: `card.edited`'s
    /// `fields`.
    fn fields(&self) -> Vec<&'static str> {
        let written = [
            ("title", self.title.is_some()),
            ("goal", self.goal.is_some()),
            ("targets", self.targets.is_some()),
            ("criteria", self.criteria.is_some()),
            ("estimate", self.estimate.is_some()),
            ("due", self.due.is_some()),
            ("parent", self.parent.is_some()),
            ("corrects", self.corrects.is_some()),
            ("evidence", !self.evidence.is_empty()),
        ];
        written
            .into_iter()
            .filter(|(_, given)| *given)
            .map(|(name, _)| name)
            .collect()
    }
}

/// One change of [`SqliteQueue::change_card`]. A flow `None`: the card's
/// flow is gone from `[card]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardChange<'f> {
    /// To the column `to` (`None`: its own), last, or before the card
    /// `before`.
    Move {
        flow: Option<&'f CardFlow>,
        to: Option<String>,
        before: Option<String>,
    },
    Edit(CardEdit),
    /// `None` leaves a name, `Some(None)` clears it.
    Assign {
        assignee: Option<Option<String>>,
        reviewer: Option<Option<String>>,
    },
    /// Out of its column.
    Cancel {
        reason: String,
    },
    /// `accept.from` -> `accept.to`, by the reviewer.
    Accept {
        flow: Option<&'f CardFlow>,
        note: Option<String>,
    },
}

/// A stored card that does not decode: its ID (as stored), the column and
/// why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableCard {
    pub id: String,
    pub column: String,
    pub reason: String,
}

impl fmt::Display for UnreadableCard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "card {}: the stored `{}` cannot be read: {}",
            self.id, self.column, self.reason
        )
    }
}

impl From<UnreadableCard> for QueueError {
    fn from(row: UnreadableCard) -> Self {
        Self::Store(StoreError::Sqlite(row.to_string()))
    }
}

/// What [`SqliteQueue::list_cards`] gives: the cards by number, and the rows
/// skipped because they do not decode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardList {
    pub cards: Vec<Card>,
    pub unreadable: Vec<UnreadableCard>,
}

/// A refusal of the card `id`, exit 1.
fn refused(id: &str, reason: String) -> QueueError {
    QueueError::CardRefused {
        id: id.to_owned(),
        reason,
    }
}

/// `no card C-0099`.
fn no_card(id: &str) -> String {
    format!("no card {id}")
}

impl SqliteQueue {
    /// Stores `card` last in its column under the next ID (highest + 1,
    /// taken in this transaction), `created_at` = `updated_at` = `now`,
    /// revision 1; logs `card.created`. `parent` and `corrects` must name
    /// stored cards ([`QueueError::CardRefused`]).
    pub fn create_card(&mut self, card: &NewCard, now: &str) -> Result<Card, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let id = next_card_id(&tx)?;
        check_links(
            &tx,
            &project,
            &id,
            card.parent.as_deref(),
            card.corrects.as_deref(),
        )?;
        let position = last_position(&tx, &project, &card.flow, &card.column)? + 1;
        tx.execute(
            &format!(
                "INSERT INTO main.cards ({CARD_SELECT}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
                 ?9, NULL, ?10, ?11, NULL, ?12, ?13, ?14, ?15, '[]', NULL, NULL, ?16, ?17, ?17, \
                 '1')"
            ),
            rusqlite::params![
                id,
                project,
                card.flow,
                card.column,
                position_value(position),
                card.title,
                card.goal,
                card.discipline,
                to_json(&card.targets)?,
                to_json(&card.criteria)?,
                card.assignee,
                card.estimate,
                card.due,
                card.parent,
                card.corrects,
                card.by,
                now,
            ],
        )
        .db()?;
        log(
            &tx,
            &project,
            EVENT_CARD_CREATED,
            &json!({
                "id": id,
                "flow": card.flow,
                "column": card.column,
                "position": position,
                "assignee": card.assignee,
                "as": card.by,
            }),
            now,
        )?;
        let stored = existing_card(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// The project's card `id`, in one snapshot; `Ok(None)` when there is
    /// none; a row that does not decode fails, named.
    pub fn get_card(&self, id: &str) -> Result<Option<Card>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let card = select_card(&tx, &self.project, id)?;
        tx.commit().db()?;
        Ok(card)
    }

    /// The project's cards by number, in one snapshot; a row that does not
    /// decode is skipped into [`CardList::unreadable`].
    pub fn list_cards(&self) -> Result<CardList, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let raws = {
            let mut statement = tx
                .prepare(&format!(
                    "SELECT {CARD_SELECT} FROM main.cards WHERE project = ?1 {ID_ORDER}"
                ))
                .db()?;
            let mut rows = statement.query([&self.project]).db()?;
            let mut raws = Vec::new();
            while let Some(row) = rows.next().db()? {
                raws.push(raw_card(row)?);
            }
            raws
        };
        tx.commit().db()?;
        let mut list = CardList::default();
        for raw in raws {
            match decode_card(raw) {
                Ok(card) => list.cards.push(card),
                Err(row) => list.unreadable.push(row),
            }
        }
        Ok(list)
    }

    /// The last `limit` `card.*` events of the card `id`, oldest first. A
    /// payload that is no JSON names no card: it is passed over.
    pub fn card_history(&self, id: &str, limit: usize) -> Result<Vec<Event>, QueueError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT seq, project, type, payload, at FROM main.events \
                 WHERE project = ?1 AND type LIKE 'card.%' ORDER BY seq DESC",
            )
            .db()?;
        let mut rows = statement.query([&self.project]).db()?;
        let mut events = Vec::new();
        while events.len() < limit {
            let Some(row) = rows.next().db()? else {
                break;
            };
            let text: Option<String> = row.get(3).db()?;
            let Some(payload) = text.and_then(|text| serde_json::from_str::<Value>(&text).ok())
            else {
                continue;
            };
            if payload.get("id").and_then(Value::as_str) != Some(id) {
                continue;
            }
            events.push(Event {
                seq: row.get(0).db()?,
                project: row.get::<_, Option<String>>(1).db()?.unwrap_or_default(),
                event_type: row.get::<_, Option<String>>(2).db()?.unwrap_or_default(),
                payload,
                at: row.get::<_, Option<String>>(4).db()?.unwrap_or_default(),
            });
        }
        events.reverse();
        Ok(events)
    }

    /// One change of the card `id` made by `actor`, in one transaction with
    /// its one event: refused ([`QueueError::CardRefused`], nothing written)
    /// when the card is closed, when `seen` (a compare-and-set) is not its
    /// revision, or when core's rules refuse it under this lock. `updated_at`
    /// = `now`, `revision` one more; the stored card.
    pub fn change_card(
        &mut self,
        id: &str,
        seen: Option<u64>,
        change: &CardChange<'_>,
        actor: &str,
        now: &str,
    ) -> Result<Card, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = select_card(&tx, &project, id)?.ok_or_else(|| refused(id, no_card(id)))?;
        if let Some(reason) = closed_refusal(&current) {
            return Err(refused(id, reason));
        }
        if seen.is_some_and(|seen| seen != current.revision) {
            return Err(refused(id, stale_refusal(&current)));
        }
        let change_tx = Change {
            tx: &tx,
            project: &project,
            current: &current,
            actor,
            now,
        };
        match change {
            CardChange::Move { flow, to, before } => {
                change_tx.moved(*flow, to.as_deref(), before.as_deref())?;
            }
            CardChange::Edit(edit) => change_tx.edited(edit)?,
            CardChange::Assign { assignee, reviewer } => {
                change_tx.assigned(assignee.as_ref(), reviewer.as_ref())?;
            }
            CardChange::Cancel { reason } => change_tx.cancelled(reason)?,
            CardChange::Accept { flow, note } => change_tx.accepted(*flow, note.as_deref())?,
        }
        tx.execute(
            "UPDATE main.cards SET updated_at = ?1, revision = ?2 WHERE id = ?3 AND project = ?4",
            rusqlite::params![
                now,
                current.revision.saturating_add(1).to_string(),
                id,
                project
            ],
        )
        .db()?;
        let stored = existing_card(&tx, &project, id)?;
        tx.commit().db()?;
        Ok(stored)
    }
}

/// One change's parts, inside its transaction.
struct Change<'c> {
    tx: &'c Transaction<'c>,
    project: &'c str,
    current: &'c Card,
    actor: &'c str,
    now: &'c str,
}

impl Change<'_> {
    fn id(&self) -> &str {
        &self.current.id
    }

    /// The row's `position` 0 and its column closed up behind it.
    fn leave_column(&self) -> Result<(), QueueError> {
        let card = self.current;
        self.tx
            .execute(
                "UPDATE main.cards SET position = 0 WHERE id = ?1 AND project = ?2",
                [&card.id, &card.project],
            )
            .db()?;
        self.tx
            .execute(
                "UPDATE main.cards SET position = position - 1 \
                 WHERE project = ?1 AND flow = ?2 AND col = ?3 AND position > ?4",
                rusqlite::params![
                    self.project,
                    card.flow,
                    card.column,
                    position_value(card.position)
                ],
            )
            .db()?;
        Ok(())
    }

    /// The row in `column` at `position`.
    fn enter_column(&self, column: &str, position: u64) -> Result<(), QueueError> {
        self.tx
            .execute(
                "UPDATE main.cards SET col = ?1, position = ?2 WHERE id = ?3 AND project = ?4",
                rusqlite::params![column, position_value(position), self.id(), self.project],
            )
            .db()?;
        Ok(())
    }

    fn moved(
        &self,
        flow: Option<&CardFlow>,
        to: Option<&str>,
        before: Option<&str>,
    ) -> Result<(), QueueError> {
        let card = self.current;
        let id = self.id();
        let to = to.unwrap_or(&card.column);
        match flow {
            Some(flow) => card_move(flow, &card.column, to, self.actor).map_err(|refusal| {
                refused(id, refusal.message(id, &card.column, to, self.actor))
            })?,
            None if to != card.column => {
                return Err(refused(
                    id,
                    format!(
                        "{id} is in the flow `{}`, which `[card]` no longer holds: no move to \
                         `{to}`; allowed: nothing; nothing changed",
                        card.flow
                    ),
                ));
            }
            None => {}
        }
        if let Some(before) = before {
            self.before_problem(to, before)?;
        }
        self.leave_column()?;
        let position = match before {
            Some(before) => {
                // Read after the close-up: its place may have shifted.
                let position = position_of(self.tx, self.project, before)?;
                self.tx
                    .execute(
                        "UPDATE main.cards SET position = position + 1 \
                         WHERE project = ?1 AND flow = ?2 AND col = ?3 AND position >= ?4",
                        rusqlite::params![self.project, card.flow, to, position_value(position)],
                    )
                    .db()?;
                position
            }
            None => last_position(self.tx, self.project, &card.flow, to)? + 1,
        };
        self.enter_column(to, position)?;
        log(
            self.tx,
            self.project,
            EVENT_CARD_MOVED,
            &json!({
                "id": id,
                "from": card.column,
                "to": to,
                "position": position,
                "as": self.actor,
            }),
            self.now,
        )
    }

    /// `before` must stand in `column` of this card's flow, in it and not
    /// this card.
    fn before_problem(&self, column: &str, before: &str) -> Result<(), QueueError> {
        let id = self.id();
        if before == id {
            return Err(refused(
                id,
                format!("--before: `{before}` is the card moved; nothing changed"),
            ));
        }
        let place: Option<(Option<String>, Option<String>, Option<i64>)> = self
            .tx
            .query_row(
                "SELECT flow, col, position FROM main.cards WHERE id = ?1 AND project = ?2",
                [before, self.project],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .db()?;
        let Some((flow, col, position)) = place else {
            return Err(refused(id, format!("--before: {}", no_card(before))));
        };
        let in_column = flow.as_deref() == Some(self.current.flow.as_str())
            && col.as_deref() == Some(column)
            && position.is_some_and(|position| position >= 1);
        if !in_column {
            return Err(refused(
                id,
                format!("--before: `{before}` is not in `{column}`; nothing changed"),
            ));
        }
        Ok(())
    }

    fn edited(&self, edit: &CardEdit) -> Result<(), QueueError> {
        let card = self.current;
        let id = self.id();
        let keep = |field: Option<&Option<String>>, stored: &Option<String>| match field {
            Some(value) => value.clone(),
            None => stored.clone(),
        };
        check_links(
            self.tx,
            self.project,
            id,
            edit.parent.as_ref().and_then(Option::as_deref),
            edit.corrects.as_ref().and_then(Option::as_deref),
        )?;
        let mut evidence = card.evidence.clone();
        for (kind, value) in &edit.evidence {
            evidence.push(CardEvidence {
                kind: kind.clone(),
                value: value.clone(),
                by: self.actor.to_owned(),
                at: self.now.to_owned(),
            });
        }
        if evidence.len() > EVIDENCE_ENTRIES_MAX {
            return Err(refused(
                id,
                format!(
                    "evidence: {} entries; at most {EVIDENCE_ENTRIES_MAX}; nothing changed",
                    evidence.len()
                ),
            ));
        }
        self.tx
            .execute(
                "UPDATE main.cards SET title = ?1, goal = ?2, targets = ?3, criteria = ?4, \
                 estimate = ?5, due = ?6, parent = ?7, corrects = ?8, evidence = ?9 \
                 WHERE id = ?10 AND project = ?11",
                rusqlite::params![
                    edit.title.as_ref().unwrap_or(&card.title),
                    keep(edit.goal.as_ref(), &card.goal),
                    to_json(edit.targets.as_ref().unwrap_or(&card.targets))?,
                    to_json(edit.criteria.as_ref().unwrap_or(&card.criteria))?,
                    keep(edit.estimate.as_ref(), &card.estimate),
                    keep(edit.due.as_ref(), &card.due),
                    keep(edit.parent.as_ref(), &card.parent),
                    keep(edit.corrects.as_ref(), &card.corrects),
                    to_json(&evidence)?,
                    id,
                    self.project,
                ],
            )
            .db()?;
        log(
            self.tx,
            self.project,
            EVENT_CARD_EDITED,
            &json!({ "id": id, "fields": edit.fields(), "as": self.actor }),
            self.now,
        )
    }

    fn assigned(
        &self,
        assignee: Option<&Option<String>>,
        reviewer: Option<&Option<String>>,
    ) -> Result<(), QueueError> {
        let card = self.current;
        let id = self.id();
        let assignee = assignee.map_or_else(|| card.assignee.clone(), Clone::clone);
        let reviewer = reviewer.map_or_else(|| card.reviewer.clone(), Clone::clone);
        if let Some(problem) = reviewer.as_deref().and_then(reviewer_problem) {
            return Err(refused(id, format!("{id}: {problem}; nothing changed")));
        }
        self.tx
            .execute(
                "UPDATE main.cards SET assignee = ?1, reviewer = ?2 WHERE id = ?3 AND project = ?4",
                rusqlite::params![assignee, reviewer, id, self.project],
            )
            .db()?;
        log(
            self.tx,
            self.project,
            EVENT_CARD_ASSIGNED,
            &json!({
                "id": id,
                "assignee": assignee,
                "reviewer": reviewer,
                "as": self.actor,
            }),
            self.now,
        )
    }

    fn cancelled(&self, reason: &str) -> Result<(), QueueError> {
        let card = self.current;
        let id = self.id();
        self.leave_column()?;
        let cancellation = CardCancellation {
            by: self.actor.to_owned(),
            at: self.now.to_owned(),
            reason: reason.to_owned(),
        };
        self.tx
            .execute(
                "UPDATE main.cards SET cancellation = ?1 WHERE id = ?2 AND project = ?3",
                rusqlite::params![to_json(&cancellation)?, id, self.project],
            )
            .db()?;
        log(
            self.tx,
            self.project,
            EVENT_CARD_CANCELLED,
            &json!({ "id": id, "column": card.column, "as": self.actor }),
            self.now,
        )
    }

    fn accepted(&self, flow: Option<&CardFlow>, note: Option<&str>) -> Result<(), QueueError> {
        let card = self.current;
        let id = self.id();
        if let Some(reason) = accept_refusal(card, flow, self.actor) {
            return Err(refused(id, reason));
        }
        let Some(flow) = flow else {
            return Err(refused(id, no_card(id)));
        };
        self.leave_column()?;
        let to = flow.accept.to.as_str();
        let position = last_position(self.tx, self.project, &card.flow, to)? + 1;
        self.enter_column(to, position)?;
        let acceptance = CardAcceptance {
            by: self.actor.to_owned(),
            at: self.now.to_owned(),
            note: note.map(str::to_owned),
        };
        self.tx
            .execute(
                "UPDATE main.cards SET acceptance = ?1 WHERE id = ?2 AND project = ?3",
                rusqlite::params![to_json(&acceptance)?, id, self.project],
            )
            .db()?;
        log(
            self.tx,
            self.project,
            EVENT_CARD_ACCEPTED,
            &json!({
                "id": id,
                "from": card.column,
                "to": to,
                "position": position,
                "as": self.actor,
            }),
            self.now,
        )
    }
}

/// A position for a statement: the positions are small and counted here.
fn position_value(position: u64) -> i64 {
    i64::try_from(position).unwrap_or(i64::MAX)
}

/// The highest `position` of (`flow`, `column`), 0 when it holds no card.
fn last_position(
    conn: &Connection,
    project: &str,
    flow: &str,
    column: &str,
) -> Result<u64, QueueError> {
    let last: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), 0) FROM main.cards \
             WHERE project = ?1 AND flow = ?2 AND col = ?3",
            [project, flow, column],
            |row| row.get(0),
        )
        .db()?;
    Ok(u64::try_from(last).unwrap_or(0))
}

/// The stored `position` of the card `id`.
fn position_of(conn: &Connection, project: &str, id: &str) -> Result<u64, QueueError> {
    let position: Option<i64> = conn
        .query_row(
            "SELECT position FROM main.cards WHERE id = ?1 AND project = ?2",
            [id, project],
            |row| row.get(0),
        )
        .db()?;
    Ok(position
        .and_then(|position| u64::try_from(position).ok())
        .unwrap_or(0))
}

/// `parent` and `corrects`, when given, name stored cards, and `parent`
/// makes no cycle with `id`.
fn check_links(
    tx: &Transaction<'_>,
    project: &str,
    id: &str,
    parent: Option<&str>,
    corrects: Option<&str>,
) -> Result<(), QueueError> {
    let stored = |card: &str| -> Result<bool, QueueError> {
        let found: Option<String> = tx
            .query_row(
                "SELECT id FROM main.cards WHERE id = ?1 AND project = ?2",
                [card, project],
                |row| row.get(0),
            )
            .optional()
            .db()?;
        Ok(found.is_some())
    };
    if corrects == Some(id) {
        return Err(refused(
            id,
            format!("corrects: `{id}` is the card itself; nothing changed"),
        ));
    }
    if let Some(corrects) = corrects
        && !stored(corrects)?
    {
        return Err(refused(id, format!("corrects: {}", no_card(corrects))));
    }
    let Some(parent) = parent else {
        return Ok(());
    };
    if !stored(parent)? {
        return Err(refused(id, format!("parent: {}", no_card(parent))));
    }
    let mut walked = BTreeSet::new();
    let mut at = Some(parent.to_owned());
    while let Some(card) = at {
        if card == id {
            return Err(refused(
                id,
                format!("parent: `{parent}` descends from `{id}`: a cycle; nothing changed"),
            ));
        }
        at = tx
            .query_row(
                "SELECT parent FROM main.cards WHERE id = ?1 AND project = ?2",
                [card.as_str(), project],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .db()?
            .flatten();
        if !walked.insert(card) {
            break;
        }
    }
    Ok(())
}

/// The next card ID: the highest number of any stored card, plus one.
fn next_card_id(tx: &Transaction<'_>) -> Result<String, QueueError> {
    let mut statement = tx.prepare("SELECT id FROM main.cards").db()?;
    let mut rows = statement.query([]).db()?;
    let mut highest = 0;
    while let Some(row) = rows.next().db()? {
        let id: Option<String> = row.get(0).db()?;
        if let Some(number) = id.as_deref().and_then(card_number) {
            highest = highest.max(number);
        }
    }
    let next = highest.checked_add(1).ok_or_else(|| {
        QueueError::Store(StoreError::Sqlite(format!(
            "the queue holds `{}`: no card ID follows it",
            card_id(highest)
        )))
    })?;
    Ok(card_id(next))
}

/// A card row as read: `position` aside, its `TEXT` columns in table order,
/// the `id` as far as it reads, and the first column holding no text, so one
/// bad byte is that row's defect and never the read's.
struct RawCard {
    name: String,
    position: Option<i64>,
    text: Vec<Option<String>>,
    broken: Option<(&'static str, &'static str)>,
}

fn raw_card(row: &Row<'_>) -> Result<RawCard, QueueError> {
    let name = match row.get_ref(0).db()? {
        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        _ => "NULL".to_owned(),
    };
    let position = match row.get_ref(POSITION_COLUMN).db()? {
        ValueRef::Integer(position) => Some(position),
        _ => None,
    };
    let mut text = Vec::with_capacity(CARD_COLUMNS.len() - 1);
    let mut broken = None;
    for (index, column) in CARD_COLUMNS.iter().enumerate() {
        if index == POSITION_COLUMN {
            continue;
        }
        match stored_text(row, index)? {
            Ok(value) => text.push(value),
            Err(why) => {
                text.push(None);
                broken.get_or_insert((*column, why));
            }
        }
    }
    Ok(RawCard {
        name,
        position,
        text,
        broken,
    })
}

fn select_card(conn: &Connection, project: &str, id: &str) -> Result<Option<Card>, QueueError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT {CARD_SELECT} FROM main.cards WHERE id = ?1 AND project = ?2"
        ))
        .db()?;
    let mut rows = statement.query([id, project]).db()?;
    match rows.next().db()? {
        Some(row) => Ok(Some(decode_card(raw_card(row)?)?)),
        None => Ok(None),
    }
}

fn existing_card(tx: &Transaction<'_>, project: &str, id: &str) -> Result<Card, QueueError> {
    select_card(tx, project, id)?.ok_or_else(|| refused(id, no_card(id)))
}

fn unreadable(id: &str, column: &str, reason: impl fmt::Display) -> UnreadableCard {
    UnreadableCard {
        id: id.to_owned(),
        column: column.to_owned(),
        reason: reason.to_string(),
    }
}

/// The JSON of `column`, decoded; `NULL` is the type's empty value.
fn json_column<T: DeserializeOwned + Default>(
    id: &str,
    column: &str,
    value: Option<String>,
) -> Result<T, UnreadableCard> {
    match value {
        Some(text) => serde_json::from_str(&text).map_err(|error| unreadable(id, column, error)),
        None => Ok(T::default()),
    }
}

fn decode_card(raw: RawCard) -> Result<Card, UnreadableCard> {
    if let Some((column, why)) = raw.broken {
        return Err(unreadable(&raw.name, column, why));
    }
    let mut values = raw.text.into_iter();
    let mut take = || values.next().flatten();
    let id = take().unwrap_or_default();
    if card_number(&id).is_none() {
        return Err(unreadable(&id, "id", format!("{id:?} is no card ID")));
    }
    let required = |value: Option<String>, column: &str| {
        value.ok_or_else(|| unreadable(&id, column, "it is NULL"))
    };
    let project = required(take(), "project")?;
    let flow = required(take(), "flow")?;
    let column = required(take(), "col")?;
    let position = raw
        .position
        .and_then(|position| u64::try_from(position).ok())
        .ok_or_else(|| unreadable(&id, "position", "it is no position from 0"))?;
    let title = required(take(), "title")?;
    let goal = take();
    let discipline = take();
    let targets: Vec<String> = json_column(&id, "targets", take())?;
    let snapshot = take();
    let criteria: Vec<String> = json_column(&id, "criteria", take())?;
    let assignee = take();
    let reviewer = take();
    let estimate = take();
    let due = take();
    let parent = take();
    let corrects = take();
    let evidence: Vec<CardEvidence> = json_column(&id, "evidence", take())?;
    let acceptance: Option<CardAcceptance> = json_column(&id, "acceptance", take())?;
    let cancellation: Option<CardCancellation> = json_column(&id, "cancellation", take())?;
    let created_by = required(take(), "created_by")?;
    let created_at = required(take(), "created_at")?;
    let updated_at = required(take(), "updated_at")?;
    let revision_text = required(take(), "revision")?;
    let revision = revision_text
        .parse::<u64>()
        .ok()
        .filter(|revision| *revision >= 1 && revision.to_string() == revision_text)
        .ok_or_else(|| {
            unreadable(
                &id,
                "revision",
                format!("{revision_text:?} is no revision (1, 2, …)"),
            )
        })?;
    Ok(Card {
        id,
        project,
        flow,
        column,
        position,
        title,
        goal,
        discipline,
        targets,
        snapshot,
        criteria,
        assignee,
        reviewer,
        estimate,
        due,
        parent,
        corrects,
        evidence,
        acceptance,
        cancellation,
        created_by,
        created_at,
        updated_at,
        revision,
    })
}
