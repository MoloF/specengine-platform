//! Cards in the project's queue (task spec `card-store`, "Store",
//! "Documents and events"; ADR-0056): the lighter record of the work of any
//! trade. Every change is one `Immediate` transaction holding the change and
//! its one event, under the rules of core's [`card_move`],
//! [`accept_refusal`] and [`closed_refusal`], asked again here, under the
//! lock. A `seen` revision makes a change a compare-and-set
//! ([`SqliteQueue::change_card`], [`SqliteQueue::accept_card`],
//! [`SqliteQueue::clear_card`]). Nothing here reads a task or a lock, only
//! [`SqliteQueue::accept_departing`] stores a proposal, and the gate's
//! [`crate::queue::GateQueue`] never reads a card: a corrupt card row is named
//! here and nowhere else.
//!
//! - **Order**: `position` dense from 1 per (`flow`, `col`). `new` and a move
//!   go last; a `before` takes that card's place; both columns stay dense. A
//!   shifted neighbour takes no `revision`, no `updated_at` and no event, so
//!   it loses no compare-and-set. A cancelled card leaves its column:
//!   `position` 0.
//! - **IDs**: `C-` and 4 or more digits, the highest plus one in the
//!   transaction, rows never deleted, so an ID is never reused.
//! - **Columns**: `targets`, `criteria`, `tasks` JSON string lists (`tasks`
//!   the `T-` IDs a card links, `NULL` read as none, never read by a rule:
//!   `docs/canon/card-tasks.md` "The rule"), `evidence`
//!   `[{kind, value, by, at}]`, `acceptance` `{by, at, note, verdict, departs,
//!   discrepancy}` (a row without `verdict` reads `briefed`), `cancellation`
//!   `{by, at, reason}`, `snapshot` `[{id, rev, norm_hash, within_hash,
//!   also}]` or `NULL`, its entries `targets`' then any dropped target's. A
//!   value that does not decode is a corrupt row, named ([`UnreadableCard`]).
//! - **Trace** (task spec `card-trace`): a suspect mark and a rollup are
//!   computed per read by core's `trace` and never stored; what is stored is
//!   the snapshot they are computed from. An accept and a clear take the
//!   caller's `TargetNorms` and judge against them under this lock; a group's
//!   clear reaches its in-scope entries alone, so a mark outside it stays.
//!   [`SqliteQueue::accept_departing`] runs an intake's body inside an
//!   accept's transaction and never refuses for a hit: it names them itself.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use specengine_core::card::{
    EVIDENCE_ENTRIES_MAX, accept_refusal, card_id, card_move, card_number, closed_refusal,
    mandate_change_problem, mandate_problem, reviewer_problem, stale_refusal, task_links_problem,
};
use specengine_core::card_counts::{CardCountRows, UnreadName};
use specengine_core::card_toml::CardFlow;
use specengine_core::intake::{DISTINCT_MAX, distinct_entry_fits};
use specengine_core::task::task_number;
use specengine_core::trace::{
    ClearedNode, HashKind, TargetNorms, clear_refusal, cleared, clearer_problem,
    correction_refusal, freeze, refrozen, retargeted, suspect_on, vouched,
};
use specengine_model::{
    Card, CardAcceptance, CardCancellation, CardEvidence, CardFrozen, CardVerdict, EvidenceKind,
    Mandate, TaskStatus,
};

use super::state::{TASK_LINKS_SCHEMA, stored_text};
use super::{
    CARD_COLUMNS, CARD_ITEMS_SCHEMA, EVENT_CREATED, Event, ID_ORDER, IntakeResult, MANDATE_SCHEMA,
    NewIntake, Proposal, ProposalFilter, ProposalKind, ProposalList, ProposalStatus, QueueError,
    RawRow, SqliteQueue, UnreadableRow, check_time, columns_of, created_payload, existing,
    index_only, insert_intake, link_problem, log, log_text, queue_matches, raw_rows_in, readable,
    to_json, user_version,
};
use crate::error::{Db, StoreError};
use crate::floor::read_floor;
use crate::schema;

/// `card.created`, with `flow`, `column`, `position`, `assignee`,
/// `reviewer`, `mandate` (`null` when none), `as`.
pub const EVENT_CARD_CREATED: &str = "card.created";
/// `card.moved`, with `from`, `to`, `position`, `as`, `note` (`null` when
/// none): a move's own event is the one home of its note.
pub const EVENT_CARD_MOVED: &str = "card.moved";
/// `card.edited`, with `fields`, `as`, and `mandate` only when `fields`
/// hold it.
pub const EVENT_CARD_EDITED: &str = "card.edited";
/// `card.assigned`, with `assignee`, `reviewer`, `as`.
pub const EVENT_CARD_ASSIGNED: &str = "card.assigned";
/// `card.cancelled`, with `column`, `as`.
pub const EVENT_CARD_CANCELLED: &str = "card.cancelled";
/// `card.accepted`, with `from`, `to`, `position`, `as`, `verdict`,
/// `discrepancy`.
pub const EVENT_CARD_ACCEPTED: &str = "card.accepted";
/// `card.cleared`, with `outcome`, `nodes`, `correction`, `reason`, `as`.
pub const EVENT_CARD_CLEARED: &str = "card.cleared";
/// `card.reestimated`, with `old`, `new`, `reason`, `as`.
pub const EVENT_CARD_REESTIMATED: &str = "card.reestimated";
/// `card.refreshed`, with `node`, `proposal`.
pub const EVENT_CARD_REFRESHED: &str = "card.refreshed";

/// `card.cleared`'s `outcome`: a person's clear with no effect on the work.
const CLEARED_NO_EFFECT: &str = "no_effect";
/// `card.cleared`'s `outcome`: a person's clear opening a correction.
const CLEARED_NEEDS_CORRECTION: &str = "needs_correction";
/// `card.cleared`'s `outcome`: a correction accepted.
const CLEARED_CORRECTED: &str = "corrected";

/// The columns of `cards` step 9 made, in table order.
macro_rules! step_9_columns {
    () => {
        "id, project, flow, col, position, title, goal, discipline, targets, snapshot, criteria, \
         assignee, reviewer, estimate, due, parent, corrects, evidence, acceptance, \
         cancellation, created_by, created_at, updated_at, revision"
    };
}

/// Every column of `cards` in table order, for a statement.
const CARD_SELECT: &str = concat!(step_9_columns!(), ", tasks, mandate");

/// [`CARD_SELECT`] over a queue still at schema 9, no step run (a handle
/// of [`SqliteQueue::open_existing`]): `tasks` and `mandate` read `NULL`,
/// as on a row steps 10 and 12 stepped.
const CARD_SELECT_9: &str = concat!(step_9_columns!(), ", NULL, NULL");

/// [`CARD_SELECT`] over a queue still at schema 10 or 11, no step run:
/// `mandate` read `NULL`, as on a row step 12 stepped.
const CARD_SELECT_10: &str = concat!(step_9_columns!(), ", tasks, NULL");

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
    /// Who accepts its result, named at creation: a hand-off's asking side.
    pub reviewer: Option<String>,
    /// Who decides what its brief leaves open, named by the asking side:
    /// only with a reviewer, asked again under the lock
    /// (`docs/canon/card-mandate.md` "The rule").
    pub mandate: Option<Mandate>,
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
    /// Why an estimate the card holds changes: `card.reestimated` instead
    /// of `card.edited`, nothing else changed beside it.
    pub reason: Option<String>,
    /// With `targets`, what they read now: a target kept keeps its entry, a
    /// new one is frozen. `None`: the snapshot is left alone.
    pub norms: Option<TargetNorms>,
    /// The `T-` tasks it links, the whole list in the order given; `Some`
    /// of none clears it. Core's grammar, repeat and cap are asked again
    /// under the lock; whether a task exists is the caller's to ask, and no
    /// task is read here (`docs/canon/card-tasks.md` "The rule").
    pub tasks: Option<Vec<String>>,
    /// The mandate it takes; never cleared. It stands only when the acting
    /// name is the card's stored reviewer, judged under the lock
    /// (`docs/canon/card-mandate.md` "The rule").
    pub mandate: Option<Mandate>,
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
            ("tasks", self.tasks.is_some()),
            ("mandate", self.mandate.is_some()),
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
    /// `before`; `note`, why it moves, kept on its `card.moved`, required
    /// into the flow's `return` column.
    Move {
        flow: Option<&'f CardFlow>,
        to: Option<String>,
        before: Option<String>,
        note: Option<String>,
    },
    Edit(Box<CardEdit>),
    /// `None` leaves a name, `Some(None)` clears it.
    Assign {
        assignee: Option<Option<String>>,
        reviewer: Option<Option<String>>,
    },
    /// Out of its column.
    Cancel {
        reason: String,
    },
}

/// What an accept says and what it freezes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAccept<'a> {
    /// The card's flow; `None`: gone from `[card]`.
    pub flow: Option<&'a CardFlow>,
    pub note: Option<&'a str>,
    /// What the card's targets, and those of the card it corrects, read now.
    pub norms: &'a TargetNorms,
}

/// What a departing accept stores beside the accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardDeparture<'a> {
    /// The discrepancy; its `distinct_from` is the accept's to write.
    pub intake: &'a NewIntake,
    /// The corpus's hits, as `distinct_from` names them.
    pub corpus_hits: &'a [String],
    /// The direct targets it departs from.
    pub departs: &'a [String],
}

/// What an accept did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAccepted {
    pub card: Card,
    /// The card this correction re-froze.
    pub corrected: Option<String>,
    /// A departing accept's item, and its hits.
    pub intake: Option<IntakeResult>,
}

/// What a person says of one card's mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardClearing<'a> {
    /// The change has no effect on the work: each suspect entry is frozen
    /// again.
    NoEffect,
    /// The accepted work needs this new card; the original keeps its row.
    NeedsCorrection(&'a NewCard),
}

/// Who clears, why, against what the targets read now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardClear<'a> {
    pub reason: &'a str,
    pub norms: &'a TargetNorms,
    pub actor: &'a str,
}

/// What a clear did: the card, and the correction it made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardCleared {
    pub card: Card,
    pub correction: Option<Card>,
}

/// An editorial apply changed the hash of `kind` of `node` from `old` to
/// `new`: a reader that has either text stays clean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardRefresh {
    pub node: String,
    pub kind: HashKind,
    pub old: String,
    pub new: String,
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

/// A split's parent (`docs/canon/card-split.md` "The rule"): a new card and
/// its snapshot (empty: none), or a stored card, read and never written.
/// The new card is boxed: it outweighs a stored card's ID many times over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitParent {
    New {
        card: Box<NewCard>,
        snapshot: Vec<CardFrozen>,
    },
    Stored(String),
}

/// One subtask of a split and its snapshot (empty: none). Its `parent` is
/// set to the split's parent in the transaction, whatever it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSplitChild {
    pub card: NewCard,
    pub snapshot: Vec<CardFrozen>,
}

/// What [`SqliteQueue::split_cards`] stores: the parent first, then the
/// subtasks in the order given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSplit {
    pub parent: SplitParent,
    pub children: Vec<NewSplitChild>,
}

/// What a split did: the parent (made, or as stored), whether it was made,
/// and the subtasks by ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitCards {
    pub parent: Card,
    pub created: bool,
    pub children: Vec<Card>,
}

/// What [`SqliteQueue::card_items`] gives: the questions, discrepancies and
/// decisions naming one card, by ID number, and the rows naming it that do
/// not decode (`docs/canon/card-questions.md` "The rule").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardItems {
    pub items: Vec<Proposal>,
    pub unreadable: Vec<UnreadableRow>,
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
    /// revision 1, no snapshot; logs `card.created`. `parent` and `corrects`
    /// must name stored cards ([`QueueError::CardRefused`]).
    pub fn create_card(&mut self, card: &NewCard, now: &str) -> Result<Card, QueueError> {
        self.create_frozen(card, None, now)
    }

    /// [`Self::create_card`] with its snapshot: each direct target frozen.
    pub fn create_card_frozen(
        &mut self,
        card: &NewCard,
        snapshot: &[CardFrozen],
        now: &str,
    ) -> Result<Card, QueueError> {
        self.create_frozen(card, Some(snapshot), now)
    }

    fn create_frozen(
        &mut self,
        card: &NewCard,
        snapshot: Option<&[CardFrozen]>,
        now: &str,
    ) -> Result<Card, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let stored = insert_card(&tx, &project, card, snapshot, now)?;
        tx.commit().db()?;
        Ok(stored)
    }

    /// A split (`docs/canon/card-split.md` "The rule";
    /// `docs/features/department-split.md` "Acceptance criteria", S5): one
    /// `Immediate` transaction, all cards or none. A stored parent is read,
    /// never written, so its row, revision and history stay; a new one is
    /// stored first. Then each subtask in order, its `parent` set here to the
    /// parent's ID whatever the input holds, each stored as
    /// [`Self::create_card`] stores one: the next ID, last in its column, its
    /// reviewer and links asked again under the lock, one `card.created`.
    /// Any refusal ([`QueueError::CardRefused`]: no subtask, no such parent,
    /// a reviewer, a link) rolls everything back: no row, no event, no ID
    /// taken.
    pub fn split_cards(&mut self, split: &NewSplit, now: &str) -> Result<SplitCards, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        if split.children.is_empty() {
            let id = match &split.parent {
                SplitParent::Stored(id) => id.clone(),
                SplitParent::New { .. } => next_card_id(&tx)?,
            };
            return Err(refused(&id, "child: none named".to_owned()));
        }
        let (parent, created) = match &split.parent {
            SplitParent::Stored(id) => match select_card(&tx, &project, id)? {
                Some(card) => (card, false),
                None => return Err(refused(id, format!("parent: {}", no_card(id)))),
            },
            SplitParent::New { card, snapshot } => {
                (insert_card(&tx, &project, card, Some(snapshot), now)?, true)
            }
        };
        let mut children = Vec::with_capacity(split.children.len());
        for child in &split.children {
            let card = NewCard {
                parent: Some(parent.id.clone()),
                ..child.card.clone()
            };
            children.push(insert_card(
                &tx,
                &project,
                &card,
                Some(&child.snapshot),
                now,
            )?);
        }
        tx.commit().db()?;
        Ok(SplitCards {
            parent,
            created,
            children,
        })
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
        let list = listed_cards(&tx, &self.project)?;
        tx.commit().db()?;
        Ok(list)
    }

    /// [`Self::list_cards`] over a handle of [`Self::open_existing`]: a DB
    /// without the cards' table (an index-only one, or a queue before step
    /// 9) holds none; nothing is written, no table added. The `user_version`
    /// is read in the same snapshot: a newer build's queue is
    /// [`QueueError::SchemaTooNew`].
    pub fn list_existing_cards(&self) -> Result<CardList, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let list = existing_cards(&tx, &self.project)?;
        tx.commit().db()?;
        Ok(list)
    }

    /// One deferred read transaction over the queue ([`QueueSnapshot`]),
    /// as the index's `SqliteIndex::snapshot` is over the index.
    pub fn snapshot(&self) -> Result<QueueSnapshot<'_>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        Ok(QueueSnapshot {
            tx,
            project: &self.project,
        })
    }

    /// The project's card counts (task spec `department-overview` DO1, DO8,
    /// DO9, DO11), in one read transaction of grouped statements over the
    /// stored columns, no row decoded: a row [`Self::list_cards`] would skip
    /// as corrupt still counts where it stands, and nothing is cut. Per
    /// (`flow`, `col`) the open, accepted and cancelled cards and the open
    /// ones with a parent (`COUNT_COLUMNS`), a `flow` or `col` that is no
    /// text counted under the name it reads as and its card named
    /// (`COUNT_UNNAMED`); per parent's flow the open parents with a child
    /// and none open (`COUNT_READY`); per card's flow the open items on its
    /// cards and their least `created_at` (`COUNT_ITEMS`); with
    /// `tasks_here`, per flow the distinct `T-` IDs its open cards link
    /// whose task, in any repository of the slug, is in review
    /// (`COUNT_LINKS`, `COUNT_REVIEW`), an open card's `tasks` that does not
    /// read as a card's decoder reads it passed over and named. The
    /// statements are these six whatever the rows: none runs per card. A
    /// handle of
    /// [`Self::open_existing`] works: a table or column a queue's schema
    /// does not hold yet counts none, and nothing is written; a newer
    /// build's queue is [`QueueError::SchemaTooNew`].
    pub fn card_counts(&self, tasks_here: bool) -> Result<CardCountRows, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let version = user_version(&tx)?;
        if version > super::QUEUE_SCHEMA_VERSION {
            return Err(QueueError::SchemaTooNew { found: version });
        }
        let mut rows = CardCountRows::default();
        if schema::has_table(&tx, "cards")? {
            let project = self.project.as_str();
            count_columns(&tx, project, &mut rows)?;
            count_unnamed(&tx, project, &mut rows)?;
            count_ready(&tx, project, &mut rows)?;
            if version >= CARD_ITEMS_SCHEMA && schema::has_table(&tx, "proposals")? {
                count_items(&tx, project, &mut rows)?;
            }
            if tasks_here && version >= TASK_LINKS_SCHEMA && schema::has_table(&tx, "tasks")? {
                count_review_links(&tx, project, &mut rows)?;
            }
        }
        tx.commit().db()?;
        Ok(rows)
    }

    /// The last `limit` `card.*` events of the card `id`, oldest first. A
    /// payload that is no JSON names no card: it is passed over.
    pub fn card_history(&self, id: &str, limit: usize) -> Result<Vec<Event>, QueueError> {
        events_naming(&self.conn, &self.project, "card.%", &[id], limit)
    }

    /// Every event of this project whose type is one of `types`, oldest
    /// first by `seq`, uncapped, in one statement; a payload that is no
    /// JSON is passed over. No type asked: none, nothing read
    /// (`docs/features/upstream-signal.md` "Acceptance criteria", US-5).
    pub fn events_of_types(&self, types: &[&str]) -> Result<Vec<Event>, QueueError> {
        events_of_types(&self.conn, &self.project, types)
    }

    /// The questions, discrepancies and decisions whose `card_id` is the
    /// card `id`, by ID number, in one snapshot; a row naming it that does
    /// not decode is listed in [`CardItems::unreadable`]. A queue below
    /// step 11 (a handle of [`Self::open_existing`]) or an index-only DB
    /// holds none. A read only: no card rule asks it
    /// (`docs/canon/card-questions.md` "The rule").
    pub fn card_items(&self, id: &str) -> Result<CardItems, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let mut found = CardItems::default();
        if !index_only(&tx)? && user_version(&tx)? >= CARD_ITEMS_SCHEMA {
            // At schema 11 or 12 (no step run here), `gist` and `matters`
            // read `NULL`.
            let columns = columns_of(&tx)?;
            let raws: Vec<RawRow> = {
                let mut statement = tx
                    .prepare(&format!(
                        "SELECT {columns} FROM main.proposals \
                         WHERE project = ?1 AND card_id = ?2 {ID_ORDER}"
                    ))
                    .db()?;
                statement
                    .query_map([self.project.as_str(), id], RawRow::read)
                    .db()?
                    .collect::<rusqlite::Result<_>>()
                    .db()?
            };
            for raw in raws {
                match raw.decode() {
                    Ok(proposal) => found.items.push(proposal),
                    Err(row) => found.unreadable.push(row),
                }
            }
        }
        tx.commit().db()?;
        Ok(found)
    }

    /// The questions, discrepancies and decisions whose `card_id` is one of
    /// the cards whose `parent` is exactly `parent` (a cancelled child
    /// included, no grandchild), by ID number, in one snapshot and one
    /// statement; each names its child in `card_id`, and a row that does not
    /// decode is listed in [`CardItems::unreadable`]. Below step 11, or on an
    /// index-only DB, there is none, as [`Self::card_items`] reads them. A
    /// read only: no card rule asks it
    /// (`docs/canon/card-mandate.md` "The rule").
    pub fn subtask_items(&self, parent: &str) -> Result<CardItems, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let mut found = CardItems::default();
        if !index_only(&tx)? && user_version(&tx)? >= CARD_ITEMS_SCHEMA {
            let columns = columns_of(&tx)?;
            let raws: Vec<RawRow> = {
                let mut statement = tx
                    .prepare(&format!(
                        "SELECT {columns} FROM main.proposals \
                         WHERE project = ?1 AND card_id IN \
                         (SELECT id FROM main.cards WHERE project = ?1 AND parent = ?2) \
                         {ID_ORDER}"
                    ))
                    .db()?;
                statement
                    .query_map([self.project.as_str(), parent], RawRow::read)
                    .db()?
                    .collect::<rusqlite::Result<_>>()
                    .db()?
            };
            for raw in raws {
                match raw.decode() {
                    Ok(proposal) => found.items.push(proposal),
                    Err(row) => found.unreadable.push(row),
                }
            }
        }
        tx.commit().db()?;
        Ok(found)
    }

    /// The last `limit` `proposal.*` events whose payload's `id` is one of
    /// `ids` (a card's items), oldest first, as [`Self::card_history`]
    /// reads a card's.
    pub fn item_history(&self, ids: &[&str], limit: usize) -> Result<Vec<Event>, QueueError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        events_naming(&self.conn, &self.project, "proposal.%", ids, limit)
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
        let (card, ()) = self.written(id, seen, actor, now, |row| match change {
            CardChange::Move {
                flow,
                to,
                before,
                note,
            } => row.moved(*flow, to.as_deref(), before.as_deref(), note.as_deref()),
            CardChange::Edit(edit) => row.edited(edit),
            CardChange::Assign { assignee, reviewer } => {
                row.assigned(assignee.as_ref(), reviewer.as_ref())
            }
            CardChange::Cancel { reason } => row.cancelled(reason),
        })?;
        Ok(card)
    }

    /// The accept of the card `id` as briefed, in one transaction: its
    /// snapshot frozen anew from `accept.norms`; a correction's accept also
    /// re-freezes the card it corrects ([`EVENT_CARD_CLEARED`], `corrected`).
    /// Refusals as [`Self::change_card`]'s.
    pub fn accept_card(
        &mut self,
        id: &str,
        seen: Option<u64>,
        accept: &CardAccept<'_>,
        actor: &str,
        now: &str,
    ) -> Result<CardAccepted, QueueError> {
        let (card, corrected) = self.written(id, seen, actor, now, |row| {
            row.accepted(accept, &Verdict::Briefed)?;
            row.corrected(accept)
        })?;
        Ok(CardAccepted {
            card,
            corrected,
            intake: None,
        })
    }

    /// [`Self::accept_card`] that departs from the brief: the discrepancy
    /// and the accept, one `Immediate` transaction, events `proposal.created`
    /// then `card.accepted`; a refusal or a failure leaves neither. The hits
    /// are read under this lock as [`super::ProposalQueue::create_intake`]
    /// reads them, but they never refuse: the item's `distinct_from` names
    /// them itself, the corpus's first, the first [`DISTINCT_MAX`] that fit
    /// the column, and the caller lists them all.
    pub fn accept_departing(
        &mut self,
        id: &str,
        seen: Option<u64>,
        accept: &CardAccept<'_>,
        departure: &CardDeparture<'_>,
        actor: &str,
        now: &str,
    ) -> Result<CardAccepted, QueueError> {
        let intake = departure.intake;
        if intake.kind != ProposalKind::Discrepancy || intake.intake.target_ids.is_empty() {
            return Err(QueueError::Invalid(
                "a departing accept stores a discrepancy with its targets".to_owned(),
            ));
        }
        if let Some(problem) = intake
            .kind
            .intake_shape()
            .and_then(|shape| shape.problem(intake.kind, &intake.intake))
            .or_else(|| link_problem(intake))
        {
            return Err(QueueError::Invalid(problem));
        }
        let dir = self.dir.clone();
        let (card, (corrected, result)) = self.written(id, seen, actor, now, |row| {
            // The discrepancy names the card accepted, read under this
            // lock as any raise's card is.
            if let Some(card) = &intake.card_id {
                check_card(row.tx, row.project, card)?;
            }
            let (hits, related) = queue_matches(row.tx, row.project, intake)?;
            let mut item = intake.clone();
            item.intake.distinct_from = departure
                .corpus_hits
                .iter()
                .map(String::as_str)
                .chain(hits.iter().map(|hit| hit.id.as_str()))
                .filter(|name| distinct_entry_fits(name))
                .take(DISTINCT_MAX)
                .map(str::to_owned)
                .collect();
            let floor = read_floor(&dir)?;
            let made = insert_intake(row.tx, row.project, &item, None, floor, now)?;
            log_text(
                row.tx,
                row.project,
                EVENT_CREATED,
                &created_payload(&made, &item),
                now,
            )?;
            row.accepted(
                accept,
                &Verdict::Departs {
                    departs: departure.departs,
                    discrepancy: &made,
                },
            )?;
            let corrected = row.corrected(accept)?;
            let created = existing(row.tx, row.project, &made)?;
            Ok((
                corrected,
                IntakeResult {
                    hits,
                    related,
                    created: Some(created),
                    linked: None,
                },
            ))
        })?;
        Ok(CardAccepted {
            card,
            corrected,
            intake: Some(result),
        })
    }

    /// The transaction of one write of the card `id`: refused when it is
    /// closed or `seen` is not its revision; `write`, then `updated_at` =
    /// `now`, `revision` one more; the stored card and what `write` gave.
    fn written<T>(
        &mut self,
        id: &str,
        seen: Option<u64>,
        actor: &str,
        now: &str,
        write: impl FnOnce(&Change<'_>) -> Result<T, QueueError>,
    ) -> Result<(Card, T), QueueError> {
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
        let row = Change {
            tx: &tx,
            project: &project,
            current: &current,
            actor,
            now,
        };
        let done = write(&row)?;
        touched(&tx, &project, &current, now)?;
        let stored = existing_card(&tx, &project, id)?;
        tx.commit().db()?;
        Ok((stored, done))
    }

    /// A person's clear of the card `id`, in one transaction with its one
    /// `card.cleared`, judged again under this lock against `norms`: refused
    /// on a cancelled card, one not suspect, one whose mark a live correction
    /// holds, a lost `seen`. [`CardClearing::NoEffect`] freezes each suspect
    /// entry again; [`CardClearing::NeedsCorrection`], accepted cards only,
    /// makes the correction (`card.created` first) and leaves the row as it
    /// is.
    pub fn clear_card(
        &mut self,
        id: &str,
        seen: Option<u64>,
        clearing: &CardClearing<'_>,
        clear: &CardClear<'_>,
        now: &str,
    ) -> Result<CardCleared, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let held = live_correction(&tx, &project, id)?;
        let one = OneClear {
            id,
            seen,
            clearing,
            held: held.as_deref(),
            scope: None,
        };
        let cleared = clear_one(&tx, &project, &one, clear, now)?;
        tx.commit().db()?;
        Ok(cleared)
    }

    /// A group's clear, no effect only: each `(id, seen)` as
    /// [`Self::clear_card`] over `scope` alone, the direct targets inside the
    /// group: a card must be suspect on one of them, only those entries are
    /// frozen again, and a mark outside stays. One transaction, one event
    /// each; one refusal leaves every card as it was.
    pub fn clear_cards(
        &mut self,
        cards: &[(String, u64)],
        scope: &BTreeSet<String>,
        clear: &CardClear<'_>,
        now: &str,
    ) -> Result<Vec<Card>, QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let held = live_corrections(&tx, &project)?;
        let mut stored = Vec::with_capacity(cards.len());
        for (id, seen) in cards {
            let one = OneClear {
                id,
                seen: Some(*seen),
                clearing: &CardClearing::NoEffect,
                held: held.get(id).map(String::as_str),
                scope: Some(scope),
            };
            stored.push(clear_one(&tx, &project, &one, clear, now)?.card);
        }
        tx.commit().db()?;
        Ok(stored)
    }

    /// The first live correction of `id`, by number: a card correcting it,
    /// neither accepted nor cancelled.
    pub fn live_correction(&self, id: &str) -> Result<Option<String>, QueueError> {
        live_correction(&self.conn, &self.project, id)
    }
}

/// The last `limit` events of `project` whose type is `like` (`card.%`,
/// `proposal.%`) and whose payload's `id` is one of `ids`, scanned newest
/// first, then oldest first. A payload that is no JSON names nothing: it is
/// passed over.
fn events_naming(
    conn: &Connection,
    project: &str,
    like: &str,
    ids: &[&str],
    limit: usize,
) -> Result<Vec<Event>, QueueError> {
    let mut statement = conn
        .prepare(
            "SELECT seq, project, type, payload, at FROM main.events \
             WHERE project = ?1 AND type LIKE ?2 ORDER BY seq DESC",
        )
        .db()?;
    let mut rows = statement.query([project, like]).db()?;
    let mut events = Vec::new();
    while events.len() < limit {
        let Some(row) = rows.next().db()? else {
            break;
        };
        let text: Option<String> = row.get(3).db()?;
        let Some(payload) = text.and_then(|text| serde_json::from_str::<Value>(&text).ok()) else {
            continue;
        };
        if !payload
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|named| ids.contains(&named))
        {
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

/// The card `id` is a row of `project`, in any state, read inside the
/// caller's write: else [`QueueError::CardRefused`] `no card C-0099`. Its
/// columns are not decoded: a card names it whatever it holds.
pub(super) fn check_card(tx: &Transaction<'_>, project: &str, id: &str) -> Result<(), QueueError> {
    let found: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM main.cards WHERE id = ?1 AND project = ?2",
            [id, project],
            |row| row.get(0),
        )
        .optional()
        .db()?;
    found.map(|_| ()).ok_or_else(|| refused(id, no_card(id)))
}

/// One deferred read transaction over the queue (task spec
/// `release-hardening` RH-R5): every row read through it comes from the one
/// snapshot its first read takes, even while another connection writes, so
/// a card list, its events and its items agree. It ends when dropped,
/// nothing written.
pub struct QueueSnapshot<'q> {
    tx: Transaction<'q>,
    project: &'q str,
}

impl QueueSnapshot<'_> {
    /// [`SqliteQueue::list_existing_cards`] in this snapshot.
    pub fn existing_cards(&self) -> Result<CardList, QueueError> {
        existing_cards(&self.tx, self.project)
    }

    /// [`SqliteQueue::events_of_types`] in this snapshot.
    pub fn events_of_types(&self, types: &[&str]) -> Result<Vec<Event>, QueueError> {
        events_of_types(&self.tx, self.project, types)
    }

    /// `ProposalQueue::list_readable` in this snapshot.
    pub fn list_readable(&self, filter: &ProposalFilter) -> Result<ProposalList, QueueError> {
        Ok(readable(
            raw_rows_in(&self.tx, self.project, filter)?,
            filter,
        ))
    }
}

/// [`SqliteQueue::list_existing_cards`] inside the caller's read: a queue
/// with no cards' table holds none; a newer build's is
/// [`QueueError::SchemaTooNew`].
fn existing_cards(tx: &Transaction<'_>, project: &str) -> Result<CardList, QueueError> {
    let version = super::user_version(tx)?;
    if version > super::QUEUE_SCHEMA_VERSION {
        return Err(QueueError::SchemaTooNew { found: version });
    }
    if !schema::has_table(tx, "cards")? {
        return Ok(CardList::default());
    }
    // A queue at schema 9 holds no `tasks` yet: its cards link none; one
    // below 12 holds no `mandate`: its cards hold none.
    let select = if version < TASK_LINKS_SCHEMA {
        CARD_SELECT_9
    } else if version < MANDATE_SCHEMA {
        CARD_SELECT_10
    } else {
        CARD_SELECT
    };
    listed_from(tx, project, select)
}

/// [`SqliteQueue::events_of_types`] over `conn`, inside its caller's read
/// when one is open.
fn events_of_types(
    conn: &Connection,
    project: &str,
    types: &[&str],
) -> Result<Vec<Event>, QueueError> {
    if types.is_empty() {
        return Ok(Vec::new());
    }
    let marks = (0..types.len())
        .map(|place| format!("?{}", place + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let mut statement = conn
        .prepare(&format!(
            "SELECT seq, project, type, payload, at FROM main.events \
             WHERE project = ?1 AND type IN ({marks}) ORDER BY seq"
        ))
        .db()?;
    let values = std::iter::once(project).chain(types.iter().copied());
    let mut rows = statement.query(rusqlite::params_from_iter(values)).db()?;
    let mut events = Vec::new();
    while let Some(row) = rows.next().db()? {
        let text: Option<String> = row.get(3).db()?;
        let Some(payload) = text.and_then(|text| serde_json::from_str::<Value>(&text).ok()) else {
            continue;
        };
        events.push(Event {
            seq: row.get(0).db()?,
            project: row.get::<_, Option<String>>(1).db()?.unwrap_or_default(),
            event_type: row.get::<_, Option<String>>(2).db()?.unwrap_or_default(),
            payload,
            at: row.get::<_, Option<String>>(4).db()?.unwrap_or_default(),
        });
    }
    Ok(events)
}

/// `project`'s cards by number, inside the caller's read; a row that does
/// not decode into [`CardList::unreadable`].
fn listed_cards(tx: &Transaction<'_>, project: &str) -> Result<CardList, QueueError> {
    listed_from(tx, project, CARD_SELECT)
}

/// [`listed_cards`] by the columns `select`, [`CARD_SELECT`]'s,
/// [`CARD_SELECT_10`]'s or [`CARD_SELECT_9`]'s.
fn listed_from(tx: &Transaction<'_>, project: &str, select: &str) -> Result<CardList, QueueError> {
    let raws = {
        let mut statement = tx
            .prepare(&format!(
                "SELECT {select} FROM main.cards WHERE project = ?1 {ID_ORDER}"
            ))
            .db()?;
        let mut rows = statement.query([project]).db()?;
        let mut raws = Vec::new();
        while let Some(row) = rows.next().db()? {
            raws.push(raw_card(row)?);
        }
        raws
    };
    let mut list = CardList::default();
    for raw in raws {
        match decode_card(raw) {
            Ok(card) => list.cards.push(card),
            Err(row) => list.unreadable.push(row),
        }
    }
    Ok(list)
}

/// [`SqliteQueue::card_counts`]' first statement: per stored (`flow`,
/// `col`), the open, accepted and cancelled cards, each card in exactly one
/// (a cancellation set first, then an acceptance), and the open ones with a
/// parent, a card naming itself no child of its own, as in `COUNT_READY`.
const COUNT_COLUMNS: &str = "SELECT flow, col, \
     sum(cancellation IS NULL AND acceptance IS NULL), \
     sum(cancellation IS NULL AND acceptance IS NOT NULL), \
     sum(cancellation IS NOT NULL), \
     sum(cancellation IS NULL AND acceptance IS NULL AND parent IS NOT NULL \
     AND parent IS NOT id) \
     FROM main.cards WHERE project = ?1 GROUP BY flow, col";

/// The second: per parent's `flow`, the open parents with a child and no
/// open child, the children grouped over `parent` once, never per card; a
/// grandchild is no child, and a card naming itself is no child of its own,
/// as the board's rollup reads one
/// (`docs/canon/card-progress.md` "The model").
const COUNT_READY: &str = "SELECT parents.flow, count(*) FROM main.cards AS parents \
     JOIN (SELECT parent, sum(cancellation IS NULL AND acceptance IS NULL) AS open_children \
     FROM main.cards WHERE project = ?1 AND parent IS NOT NULL AND parent IS NOT id \
     GROUP BY parent) AS children ON children.parent = parents.id \
     WHERE parents.project = ?1 AND parents.cancellation IS NULL \
     AND parents.acceptance IS NULL AND children.open_children = 0 \
     GROUP BY parents.flow";

/// The third: per card's `flow`, the open items whose `card_id` names one
/// of its cards, in any state, and their least `created_at`; `?2` the open
/// status.
const COUNT_ITEMS: &str = "SELECT cards.flow, count(*), min(items.created_at) \
     FROM main.proposals AS items JOIN main.cards AS cards ON cards.id = items.card_id \
     WHERE items.project = ?1 AND cards.project = ?1 AND items.status = ?2 \
     GROUP BY cards.flow";

/// The fourth, with agent tasks counted: the open cards' links as stored.
const COUNT_LINKS: &str = "SELECT id, flow, tasks FROM main.cards \
     WHERE project = ?1 AND tasks IS NOT NULL AND cancellation IS NULL AND acceptance IS NULL";

/// The fifth, beside it: the tasks in review, of any repository of the
/// slug; `?2` the review status.
const COUNT_REVIEW: &str = "SELECT DISTINCT id FROM main.tasks WHERE project = ?1 AND status = ?2";

/// The sixth, beside the first: the cards whose stored `flow` or `col` is
/// no text, each named where it counts.
const COUNT_UNNAMED: &str = "SELECT id, flow, col FROM main.cards \
     WHERE project = ?1 AND (typeof(flow) IS NOT 'text' OR typeof(col) IS NOT 'text')";

/// Why a stored name is no text, as `card list` says it of a row; `None`
/// for text.
fn unread_why(value: ValueRef<'_>) -> Option<&'static str> {
    match value {
        ValueRef::Text(_) => None,
        ValueRef::Null => Some("it is NULL"),
        ValueRef::Integer(_) | ValueRef::Real(_) => Some("it is a number, not text"),
        ValueRef::Blob(_) => Some("it is a BLOB, not text"),
    }
}

/// A stored name as a count groups it: its text, an invalid byte replaced;
/// `NULL` when there is none, which no `[card]` name spells
/// (`COUNT_UNNAMED` names each such card).
fn stored_name(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            String::from_utf8_lossy(bytes).into_owned()
        }
        ValueRef::Integer(number) => number.to_string(),
        ValueRef::Real(number) => number.to_string(),
        ValueRef::Null => "NULL".to_owned(),
    }
}

/// The count at `index` of a grouped row: a sum or a `count(*)`, never
/// below 0.
fn count_at(row: &Row<'_>, index: usize) -> Result<u64, QueueError> {
    let value: Option<i64> = row.get(index).db()?;
    Ok(value
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0))
}

/// [`COUNT_COLUMNS`] into `rows`: two names read as one (an invalid byte
/// replaced in each) add up.
fn count_columns(
    tx: &Transaction<'_>,
    project: &str,
    rows: &mut CardCountRows,
) -> Result<(), QueueError> {
    let mut statement = tx.prepare(COUNT_COLUMNS).db()?;
    let mut found = statement.query([project]).db()?;
    while let Some(row) = found.next().db()? {
        let key = (
            stored_name(row.get_ref(0).db()?),
            stored_name(row.get_ref(1).db()?),
        );
        let tally = rows.columns.entry(key).or_default();
        tally.open += count_at(row, 2)?;
        tally.accepted += count_at(row, 3)?;
        tally.cancelled += count_at(row, 4)?;
        tally.open_subtasks += count_at(row, 5)?;
    }
    Ok(())
}

/// [`COUNT_UNNAMED`] into `rows`: each card's `flow`, then its `col`, that
/// is no text, with why and the name it counts under ([`stored_name`]).
fn count_unnamed(
    tx: &Transaction<'_>,
    project: &str,
    rows: &mut CardCountRows,
) -> Result<(), QueueError> {
    let mut statement = tx.prepare(COUNT_UNNAMED).db()?;
    let mut found = statement.query([project]).db()?;
    while let Some(row) = found.next().db()? {
        let card = stored_name(row.get_ref(0).db()?);
        for (index, column) in [(1, "flow"), (2, "col")] {
            let value = row.get_ref(index).db()?;
            if let Some(reason) = unread_why(value) {
                rows.unread_names.push(UnreadName {
                    card: card.clone(),
                    column: column.to_owned(),
                    reason: reason.to_owned(),
                    counted_as: stored_name(value),
                });
            }
        }
    }
    Ok(())
}

/// [`COUNT_READY`] into `rows`.
fn count_ready(
    tx: &Transaction<'_>,
    project: &str,
    rows: &mut CardCountRows,
) -> Result<(), QueueError> {
    let mut statement = tx.prepare(COUNT_READY).db()?;
    let mut found = statement.query([project]).db()?;
    while let Some(row) = found.next().db()? {
        let flow = stored_name(row.get_ref(0).db()?);
        *rows.ready_parents.entry(flow).or_default() += count_at(row, 1)?;
    }
    Ok(())
}

/// [`COUNT_ITEMS`] into `rows`: a `created_at` that is no text is none.
fn count_items(
    tx: &Transaction<'_>,
    project: &str,
    rows: &mut CardCountRows,
) -> Result<(), QueueError> {
    let mut statement = tx.prepare(COUNT_ITEMS).db()?;
    let mut found = statement
        .query([project, ProposalStatus::Open.as_str()])
        .db()?;
    while let Some(row) = found.next().db()? {
        let flow = stored_name(row.get_ref(0).db()?);
        let oldest = match row.get_ref(2).db()? {
            ValueRef::Text(bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
            _ => None,
        };
        let items = rows.open_items.entry(flow).or_default();
        items.count += count_at(row, 1)?;
        items.oldest = match (items.oldest.take(), oldest) {
            (Some(held), Some(read)) => Some(held.min(read)),
            (held, read) => held.or(read),
        };
    }
    Ok(())
}

/// [`COUNT_REVIEW`] and [`COUNT_LINKS`] into `rows`: each open card's
/// linked IDs whose task is in review, per its flow, once; a `tasks` that
/// does not read ([`task_links`]) passed over whole and its card named.
fn count_review_links(
    tx: &Transaction<'_>,
    project: &str,
    rows: &mut CardCountRows,
) -> Result<(), QueueError> {
    let mut review = BTreeSet::new();
    {
        let mut statement = tx.prepare(COUNT_REVIEW).db()?;
        let mut found = statement
            .query([project, TaskStatus::Review.as_str()])
            .db()?;
        while let Some(row) = found.next().db()? {
            review.insert(stored_name(row.get_ref(0).db()?));
        }
    }
    let mut statement = tx.prepare(COUNT_LINKS).db()?;
    let mut found = statement.query([project]).db()?;
    while let Some(row) = found.next().db()? {
        let card = stored_name(row.get_ref(0).db()?);
        let flow = stored_name(row.get_ref(1).db()?);
        let Some(links) = task_links(row.get_ref(2).db()?) else {
            rows.unread_tasks.insert(card);
            continue;
        };
        for link in links.into_iter().filter(|link| review.contains(link)) {
            rows.review_tasks
                .entry(flow.clone())
                .or_default()
                .insert(link);
        }
    }
    Ok(())
}

/// A stored `tasks`' IDs, read as a card's decoder reads them: `None`, so
/// passed over and named, when it is no JSON list of text or an entry is no
/// task ID; no entry is ever dropped (ADR-0047).
fn task_links(value: ValueRef<'_>) -> Option<Vec<String>> {
    let ValueRef::Text(bytes) = value else {
        return None;
    };
    let links: Vec<String> = serde_json::from_slice(bytes).ok()?;
    links
        .iter()
        .all(|link| task_number(link).is_some())
        .then_some(links)
}

/// What an accept records of its verdict.
enum Verdict<'a> {
    Briefed,
    Departs {
        departs: &'a [String],
        discrepancy: &'a str,
    },
}

/// `updated_at` = `now`, `revision` one more than `card`'s.
fn touched(tx: &Transaction<'_>, project: &str, card: &Card, now: &str) -> Result<(), QueueError> {
    tx.execute(
        "UPDATE main.cards SET updated_at = ?1, revision = ?2 WHERE id = ?3 AND project = ?4",
        rusqlite::params![
            now,
            card.revision.saturating_add(1).to_string(),
            card.id,
            project
        ],
    )
    .db()?;
    Ok(())
}

/// The row's `snapshot` = `snapshot`, its entries `targets`' then any dropped
/// target's: a card that never froze one holds none, and one with all its
/// targets dropped keeps their entries.
fn write_snapshot(
    tx: &Transaction<'_>,
    project: &str,
    id: &str,
    snapshot: &[CardFrozen],
) -> Result<(), QueueError> {
    tx.execute(
        "UPDATE main.cards SET snapshot = ?1 WHERE id = ?2 AND project = ?3",
        rusqlite::params![snapshot_column(snapshot)?, id, project],
    )
    .db()?;
    Ok(())
}

/// `snapshot` as its column holds it: an empty one is `NULL`.
fn snapshot_column(snapshot: &[CardFrozen]) -> Result<Option<String>, QueueError> {
    if snapshot.is_empty() {
        return Ok(None);
    }
    to_json(&snapshot).map(Some)
}

/// [`write_snapshot`], `updated_at` = `now`, `revision` one more.
fn refreeze(
    tx: &Transaction<'_>,
    project: &str,
    card: &Card,
    snapshot: &[CardFrozen],
    now: &str,
) -> Result<(), QueueError> {
    write_snapshot(tx, project, &card.id, snapshot)?;
    touched(tx, project, card, now)
}

/// One `card.cleared`.
struct ClearedEvent<'a> {
    id: &'a str,
    outcome: &'a str,
    nodes: &'a [ClearedNode],
    correction: Option<&'a str>,
    reason: Option<&'a str>,
    actor: &'a str,
}

impl ClearedEvent<'_> {
    fn log(&self, tx: &Transaction<'_>, project: &str, now: &str) -> Result<(), QueueError> {
        log(
            tx,
            project,
            EVENT_CARD_CLEARED,
            &json!({
                "id": self.id,
                "outcome": self.outcome,
                "nodes": self.nodes,
                "correction": self.correction,
                "reason": self.reason,
                "as": self.actor,
            }),
            now,
        )
    }
}

/// The first live correction of `id`, by number: a card correcting it,
/// neither accepted nor cancelled.
fn live_correction(
    conn: &Connection,
    project: &str,
    id: &str,
) -> Result<Option<String>, QueueError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT id FROM main.cards WHERE project = ?1 AND corrects = ?2 \
                 AND acceptance IS NULL AND cancellation IS NULL {ID_ORDER} LIMIT 1"
            ),
            [project, id],
            |row| row.get(0),
        )
        .optional()
        .db()?)
}

/// Every live correction of the project's cards, read once: each card
/// corrected and its first by number. A row that does not read holds nothing
/// here, so one bad row stops no group's clear.
fn live_corrections(
    conn: &Connection,
    project: &str,
) -> Result<BTreeMap<String, String>, QueueError> {
    let mut statement = conn
        .prepare(&format!(
            "SELECT id, corrects FROM main.cards WHERE project = ?1 AND corrects IS NOT NULL \
             AND acceptance IS NULL AND cancellation IS NULL {ID_ORDER}"
        ))
        .db()?;
    let mut rows = statement.query([project]).db()?;
    let mut held = BTreeMap::new();
    while let Some(row) = rows.next().db()? {
        let (Ok(Some(correction)), Ok(Some(original))) =
            (stored_text(row, 0)?, stored_text(row, 1)?)
        else {
            continue;
        };
        held.entry(original).or_insert(correction);
    }
    Ok(held)
}

/// One card of a clear.
struct OneClear<'a> {
    id: &'a str,
    seen: Option<u64>,
    clearing: &'a CardClearing<'a>,
    /// Its live correction, read under this lock.
    held: Option<&'a str>,
    /// The direct targets a group reaches; `None`: all of the card's.
    scope: Option<&'a BTreeSet<String>>,
}

/// One card's clear inside its transaction ([`SqliteQueue::clear_card`]).
fn clear_one(
    tx: &Transaction<'_>,
    project: &str,
    one: &OneClear<'_>,
    clear: &CardClear<'_>,
    now: &str,
) -> Result<CardCleared, QueueError> {
    let id = one.id;
    let norms = clear.norms;
    if let Some(problem) = clearer_problem(clear.actor) {
        return Err(refused(id, format!("{id}: {problem}")));
    }
    let card = select_card(tx, project, id)?.ok_or_else(|| refused(id, no_card(id)))?;
    let in_scope = |target: &str| one.scope.is_none_or(|scope| scope.contains(target));
    let suspect = suspect_on(&card, norms).into_iter().any(&in_scope);
    if let Some(refusal) = clear_refusal(&card, suspect, one.held) {
        return Err(refused(id, refusal));
    }
    if one.seen.is_some_and(|seen| seen != card.revision) {
        return Err(refused(id, stale_refusal(&card)));
    }
    let (snapshot, nodes) = cleared(&card, norms, in_scope);
    match one.clearing {
        CardClearing::NoEffect => {
            refreeze(tx, project, &card, &snapshot, now)?;
            ClearedEvent {
                id,
                outcome: CLEARED_NO_EFFECT,
                nodes: &nodes,
                correction: None,
                reason: Some(clear.reason),
                actor: clear.actor,
            }
            .log(tx, project, now)?;
            Ok(CardCleared {
                card: existing_card(tx, project, id)?,
                correction: None,
            })
        }
        CardClearing::NeedsCorrection(correction) => {
            if let Some(refusal) = correction_refusal(&card) {
                return Err(refused(id, refusal));
            }
            let frozen = freeze(&correction.targets, norms);
            let made = insert_card(tx, project, correction, Some(&frozen), now)?;
            ClearedEvent {
                id,
                outcome: CLEARED_NEEDS_CORRECTION,
                nodes: &nodes,
                correction: Some(&made.id),
                reason: Some(clear.reason),
                actor: clear.actor,
            }
            .log(tx, project, now)?;
            Ok(CardCleared {
                card,
                correction: Some(made),
            })
        }
    }
}

/// Stores `card` under the next ID, last in its column: [`SqliteQueue::create_card`]'s
/// row and its `card.created`, inside the caller's transaction.
fn insert_card(
    tx: &Transaction<'_>,
    project: &str,
    card: &NewCard,
    snapshot: Option<&[CardFrozen]>,
    now: &str,
) -> Result<Card, QueueError> {
    let id = next_card_id(tx)?;
    if let Some(problem) = card.reviewer.as_deref().and_then(reviewer_problem) {
        return Err(refused(&id, format!("reviewer: {problem}")));
    }
    if let Some(problem) = mandate_problem(card.mandate, card.reviewer.as_deref()) {
        return Err(refused(&id, problem));
    }
    check_links(
        tx,
        project,
        &id,
        card.parent.as_deref(),
        card.corrects.as_deref(),
    )?;
    let position = last_position(tx, project, &card.flow, &card.column)? + 1;
    let mandate = card.mandate.map(Mandate::as_str);
    tx.execute(
        &format!(
            "INSERT INTO main.cards ({CARD_SELECT}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, \
             ?9, ?10, ?11, ?12, ?19, ?13, ?14, ?15, ?16, '[]', NULL, NULL, ?17, ?18, ?18, \
             '1', '[]', ?20)"
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
            snapshot_column(snapshot.unwrap_or_default())?,
            to_json(&card.criteria)?,
            card.assignee,
            card.estimate,
            card.due,
            card.parent,
            card.corrects,
            card.by,
            now,
            card.reviewer,
            mandate,
        ],
    )
    .db()?;
    // `mandate` on every card's, `null` when none
    // (`docs/features/task-mandate.md` "Acceptance criteria", M1).
    log(
        tx,
        project,
        EVENT_CARD_CREATED,
        &json!({
            "id": id,
            "flow": card.flow,
            "column": card.column,
            "position": position,
            "assignee": card.assignee,
            "reviewer": card.reviewer,
            "mandate": mandate,
            "as": card.by,
        }),
        now,
    )?;
    existing_card(tx, project, &id)
}

/// The transaction recording an editorial apply `proposal`: each change of
/// `refresh` joins the `also` of every entry holding its old hash of that
/// kind, across the project's cards not cancelled, one `card.refreshed` per
/// (card, node), a node repeated in `refresh` logging one, `revision` one more
/// on a card it changes. A row that does not decode is left alone: recording
/// a commit never fails for a card's sake. The cards changed, by number.
pub(super) fn refresh_snapshots(
    tx: &Transaction<'_>,
    project: &str,
    proposal: &str,
    refresh: &[CardRefresh],
    now: &str,
) -> Result<Vec<String>, QueueError> {
    if refresh.is_empty() {
        return Ok(Vec::new());
    }
    let raws = {
        let mut statement = tx
            .prepare(&format!(
                "SELECT {CARD_SELECT} FROM main.cards WHERE project = ?1 AND snapshot IS NOT NULL \
                 AND cancellation IS NULL {ID_ORDER}"
            ))
            .db()?;
        let mut rows = statement.query([project]).db()?;
        let mut raws = Vec::new();
        while let Some(row) = rows.next().db()? {
            raws.push(raw_card(row)?);
        }
        raws
    };
    let mut changed = Vec::new();
    for card in raws.into_iter().filter_map(|raw| decode_card(raw).ok()) {
        let mut snapshot = card.snapshot.clone().unwrap_or_default();
        let mut nodes = Vec::new();
        for change in refresh {
            for entry in snapshot.iter_mut().filter(|entry| entry.id == change.node) {
                if vouched(entry, change.kind, &change.old, &change.new)
                    && !nodes.contains(&change.node.as_str())
                {
                    nodes.push(change.node.as_str());
                }
            }
        }
        if nodes.is_empty() {
            continue;
        }
        refreeze(tx, project, &card, &snapshot, now)?;
        for node in nodes {
            log(
                tx,
                project,
                EVENT_CARD_REFRESHED,
                &json!({ "id": card.id, "node": node, "proposal": proposal }),
                now,
            )?;
        }
        changed.push(card.id);
    }
    Ok(changed)
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
        note: Option<&str>,
    ) -> Result<(), QueueError> {
        let card = self.current;
        let id = self.id();
        let to = to.unwrap_or(&card.column);
        match flow {
            Some(flow) => {
                card_move(flow, &card.column, to, self.actor, note).map_err(|refusal| {
                    refused(id, refusal.message(id, &card.column, to, self.actor))
                })?
            }
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
                "note": note,
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
        // The reviewer compare, against the row under this lock, whatever
        // else the edit holds
        // (`docs/features/task-mandate.md` "Acceptance criteria", M3).
        if edit.mandate.is_some()
            && let Some(problem) = mandate_change_problem(id, card.reviewer.as_deref(), self.actor)
        {
            return Err(refused(id, problem));
        }
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
        // A change of an estimate the card holds, a clear too, says why, even
        // when a read before this lock saw none, and the record keeps `old`.
        let reestimated = edit.estimate.is_some() && card.estimate.is_some();
        if reestimated && edit.reason.is_none() {
            return Err(refused(
                id,
                format!("{id} holds an estimate: --reason says why it changes; nothing changed"),
            ));
        }
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
        if let Some(problem) = edit.tasks.as_deref().and_then(task_links_problem) {
            return Err(refused(id, problem));
        }
        let tasks = edit.tasks.as_ref().map(to_json).transpose()?;
        if let (Some(targets), Some(norms)) = (&edit.targets, &edit.norms) {
            let snapshot = retargeted(card.snapshot.as_deref(), targets, norms);
            write_snapshot(self.tx, self.project, id, &snapshot)?;
        }
        let mandate = edit.mandate.map(Mandate::as_str);
        // `tasks` written whole when given, else left as stored (a row
        // stepped from schema 9 keeps its `NULL`); `mandate` likewise, never
        // cleared.
        self.tx
            .execute(
                "UPDATE main.cards SET title = ?1, goal = ?2, targets = ?3, criteria = ?4, \
                 estimate = ?5, due = ?6, parent = ?7, corrects = ?8, evidence = ?9, \
                 tasks = COALESCE(?12, tasks), mandate = COALESCE(?13, mandate) \
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
                    tasks,
                    mandate,
                ],
            )
            .db()?;
        if reestimated {
            return log(
                self.tx,
                self.project,
                EVENT_CARD_REESTIMATED,
                &json!({
                    "id": id,
                    "old": card.estimate,
                    "new": edit.estimate.as_ref().and_then(Option::as_deref),
                    "reason": edit.reason,
                    "as": self.actor,
                }),
                self.now,
            );
        }
        // `mandate` only when the edit sets it: every other edit's payload
        // stays `{id, fields, as}`.
        let mut payload = json!({ "id": id, "fields": edit.fields(), "as": self.actor });
        if let (Some(mandate), Value::Object(entries)) = (mandate, &mut payload) {
            entries.insert("mandate".to_owned(), Value::from(mandate));
        }
        log(self.tx, self.project, EVENT_CARD_EDITED, &payload, self.now)
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

    /// The accept proper, its snapshot frozen anew.
    fn accepted(&self, accept: &CardAccept<'_>, verdict: &Verdict<'_>) -> Result<(), QueueError> {
        let CardAccept { flow, note, norms } = *accept;
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
        let (word, departs, discrepancy) = match verdict {
            Verdict::Briefed => (CardVerdict::Briefed, Vec::new(), None),
            Verdict::Departs {
                departs,
                discrepancy,
            } => (
                CardVerdict::Departs,
                departs.to_vec(),
                Some((*discrepancy).to_owned()),
            ),
        };
        let acceptance = CardAcceptance {
            by: self.actor.to_owned(),
            at: self.now.to_owned(),
            note: note.map(str::to_owned),
            verdict: word,
            departs,
            discrepancy: discrepancy.clone(),
        };
        self.tx
            .execute(
                "UPDATE main.cards SET acceptance = ?1 WHERE id = ?2 AND project = ?3",
                rusqlite::params![to_json(&acceptance)?, id, self.project],
            )
            .db()?;
        write_snapshot(self.tx, self.project, id, &refrozen(card, norms))?;
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
                "verdict": word.as_str(),
                "discrepancy": discrepancy,
            }),
            self.now,
        )
    }

    /// This card's accept re-freezes the card it corrects, when any entry
    /// of that card is suspect: a person answered its mark. A target absent
    /// from the read is left as it is: gone, or a target that card took after
    /// the read, it has nothing to freeze. That card's ID; `None` when
    /// nothing moved, it is cancelled, or its row does not read (the accept
    /// stands: nothing is stopped for a row's sake).
    fn corrected(&self, accept: &CardAccept<'_>) -> Result<Option<String>, QueueError> {
        let Some(original) = self.current.corrects.as_deref() else {
            return Ok(None);
        };
        let Ok(Some(card)) = select_card(self.tx, self.project, original) else {
            return Ok(None);
        };
        if card.cancellation.is_some() {
            return Ok(None);
        }
        let read = |target: &str| accept.norms.contains_key(target);
        let (snapshot, nodes) = cleared(&card, accept.norms, read);
        if nodes.is_empty() {
            return Ok(None);
        }
        refreeze(self.tx, self.project, &card, &snapshot, self.now)?;
        ClearedEvent {
            id: original,
            outcome: CLEARED_CORRECTED,
            nodes: &nodes,
            correction: Some(self.id()),
            reason: accept.note,
            actor: self.actor,
        }
        .log(self.tx, self.project, self.now)?;
        Ok(Some(card.id))
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
    let snapshot: Option<Vec<CardFrozen>> = json_column(&id, "snapshot", take())?;
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
    // `NULL` (a row stepped from schema 9) links none; every entry must be
    // a task ID, while a repeat or a long list a hand edit left reads as
    // stored (`docs/canon/cards.md` "Store").
    let tasks: Vec<String> = json_column(&id, "tasks", take())?;
    if let Some(entry) = tasks.iter().find(|entry| task_number(entry).is_none()) {
        return Err(unreadable(&id, "tasks", format!("{entry:?} is no task ID")));
    }
    // `NULL` (a row stepped from schema 11) holds none; any other word than
    // the two is a corrupt row (`docs/canon/cards.md` "Store").
    let mandate = match take() {
        None => None,
        Some(word) => Some(Mandate::parse(&word).ok_or_else(|| {
            unreadable(
                &id,
                "mandate",
                format!("{word:?} is not `binding` or `open`"),
            )
        })?),
    };
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
        mandate,
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
        tasks,
    })
}
