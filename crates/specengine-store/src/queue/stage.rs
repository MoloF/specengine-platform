//! A proposal's staged choice
//! (`docs/canon/decision-staging.md` "The stage", "Queue"): queue step
//! 4 → 5 adds `staged` (compact JSON, one of
//! the two shapes, keys in their order) and `staged_at` (the UTC time it was
//! staged), both `NULL` when nothing is staged. Step 6 → 7 (task spec
//! `rev-discipline`) adds the approve shape's `editorial` (`true` or
//! `null`) after `canon`: an open approve stage of schema 6 is rewritten
//! with `"editorial":null` ([`upgrade_stage`]). A stage is set, replaced and
//! cleared only on an `open` proposal ([`ProposalQueue::stage_from`],
//! [`ProposalQueue::unstage_from`]: one `Immediate` transaction with its
//! event, a compare-and-set on the [`Seen`] read); every op that leaves
//! `open` clears both columns in its own transaction. A stage writes no
//! file and changes no state: it is an attribute of an `open` proposal.
//!
//! Read back, the two columns are checked: one `NULL` alone, a stage on a
//! proposal that is not `open`, a `staged_at` that is no UTC time, a
//! `staged` of another shape (a key missing, added or reordered, a value of
//! another type, not compact), or a decision flag of a kind that takes none
//! make the row corrupt, named.
//!
//! [`ProposalQueue::stage_from`]: super::ProposalQueue::stage_from
//! [`ProposalQueue::unstage_from`]: super::ProposalQueue::unstage_from

use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use specengine_core::proposal::{EDITORIAL, is_utc_timestamp};

use super::{
    Proposal, ProposalKind, ProposalStatus, QueueError, Seen, SqliteQueue, UnreadableRow, changed,
    check_time, corrupt, existing, log_text, status_error,
};
use crate::error::Db;

/// `proposal.staged`, with `staged` (the stage, an object) and `staged_at`.
pub const EVENT_STAGED: &str = "proposal.staged";
/// `proposal.unstaged`.
pub const EVENT_UNSTAGED: &str = "proposal.unstaged";

/// The owner's choice staged on an `open` proposal, the flags `spec
/// approve|reject` would take
/// (`docs/canon/decision-staging.md` "The stage"). Stored and serialized
/// as one of
/// `{"decision":"approve","option":…,"answer":…,"canon":…,"editorial":…,"note":…,"span_hash":…}`
/// (`editorial` `true` or `null`) and `{"decision":"reject","reason":"…"}`,
/// compact, keys in this order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// `spec approve PR` with these flags; `None` when absent.
    Approve {
        /// `--option N`: a discrepancy's.
        option: Option<u64>,
        /// `--answer T`: a question's or a decision's.
        answer: Option<String>,
        /// `--canon REF`: a kind that decides.
        canon: Option<String>,
        /// `--editorial`: the revisions kept, the vouching `spec.lock`
        /// entries refreshed (task spec `rev-discipline`); only on a kind
        /// writing an existing node. Stored `true`, else `null`.
        editorial: bool,
        /// `--note T`.
        note: Option<String>,
        /// An `update`'s or a section-form `create`'s target span hash when
        /// staged, when it could be read; `None` for any other kind.
        span_hash: Option<String>,
    },
    /// `spec reject PR --reason T`.
    Reject { reason: String },
}

/// A stage and when it was made: a proposal's `staged` and `staged_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedChoice {
    pub stage: Stage,
    /// `YYYY-MM-DDTHH:MM:SSZ`.
    pub at: String,
}

/// The approve shape, keys in order.
#[derive(Serialize)]
struct ApproveShape<'a> {
    decision: &'static str,
    option: Option<u64>,
    answer: Option<&'a str>,
    canon: Option<&'a str>,
    /// `true`, or `null` (never `false`).
    editorial: Option<bool>,
    note: Option<&'a str>,
    span_hash: Option<&'a str>,
}

/// The reject shape, keys in order.
#[derive(Serialize)]
struct RejectShape<'a> {
    decision: &'static str,
    reason: &'a str,
}

impl Serialize for Stage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Approve {
                option,
                answer,
                canon,
                editorial,
                note,
                span_hash,
            } => ApproveShape {
                decision: "approve",
                option: *option,
                answer: answer.as_deref(),
                canon: canon.as_deref(),
                editorial: editorial.then_some(true),
                note: note.as_deref(),
                span_hash: span_hash.as_deref(),
            }
            .serialize(serializer),
            Self::Reject { reason } => RejectShape {
                decision: "reject",
                reason,
            }
            .serialize(serializer),
        }
    }
}

impl Stage {
    /// The stored text: compact JSON, keys in the shape's order (a struct
    /// serialized field by field, whatever a JSON map's order is).
    pub fn to_json(&self) -> String {
        // Strings, integers and nulls only: `serde_json` always encodes them.
        serde_json::to_string(self).unwrap_or_default()
    }

    /// The stage `text` stores, when it is exactly the compact JSON of one
    /// of the two shapes ([`Self::to_json`] gives it back byte for byte);
    /// `None` for anything else.
    pub fn from_stored(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        let text_of = |key: &str| -> Option<Option<String>> {
            match object.get(key)? {
                Value::Null => Some(None),
                Value::String(text) => Some(Some(text.clone())),
                _ => None,
            }
        };
        let stage = match object.get("decision")?.as_str()? {
            "approve" => Self::Approve {
                option: match object.get("option")? {
                    Value::Null => None,
                    Value::Number(number) => Some(number.as_u64()?),
                    _ => return None,
                },
                answer: text_of("answer")?,
                canon: text_of("canon")?,
                editorial: match object.get(EDITORIAL)? {
                    Value::Null => false,
                    Value::Bool(true) => true,
                    _ => return None,
                },
                note: text_of("note")?,
                span_hash: text_of("span_hash")?,
            },
            "reject" => Self::Reject {
                reason: text_of("reason")??,
            },
            _ => return None,
        };
        // A key added or reordered, or a text not compact: not its bytes.
        (stage.to_json() == text).then_some(stage)
    }

    /// `true` for a staged reject.
    pub fn rejects(&self) -> bool {
        matches!(self, Self::Reject { .. })
    }

    /// Why a proposal of `kind` (`new_file`: a new file, a create's or an
    /// amendment's) cannot hold this stage: a decision flag of a kind that
    /// takes none (a kind that applies takes none; a question or a decision
    /// no `--option`; a discrepancy no `--answer`), a span hash or
    /// `editorial` on a kind that has no span (a deciding kind, a new file:
    /// it writes no existing node), a blank reason; `None`: it can.
    pub fn problem(&self, kind: ProposalKind, new_file: bool) -> Option<String> {
        match self {
            Self::Reject { reason } => reason
                .trim()
                .is_empty()
                .then(|| "a staged reject with a blank reason".to_owned()),
            Self::Approve {
                option,
                answer,
                canon,
                editorial,
                span_hash,
                ..
            } => {
                // Keyed on what the kind does: one that applies takes no
                // decision flag; a deciding kind is judged by name, every
                // kind listed, so a new one must be judged here.
                let flag = if kind.applies() {
                    if option.is_some() {
                        Some("option")
                    } else if answer.is_some() {
                        Some("answer")
                    } else if canon.is_some() {
                        Some("canon")
                    } else {
                        None
                    }
                } else {
                    match kind {
                        // Answered by the working answer or the owner's.
                        ProposalKind::Question | ProposalKind::Decision => {
                            option.is_some().then_some("option")
                        }
                        ProposalKind::Discrepancy => answer.is_some().then_some("answer"),
                        // Judged above: they apply.
                        ProposalKind::Update
                        | ProposalKind::Create
                        | ProposalKind::Interpretation
                        | ProposalKind::Amendment => None,
                    }
                };
                if let Some(flag) = flag {
                    return Some(format!(
                        "a staged `{flag}` on {}, which takes none",
                        kind.with_article()
                    ));
                }
                let spanless = kind.decides() || new_file;
                if spanless && *editorial {
                    return Some(format!(
                        "a staged `{EDITORIAL}` on {}, which writes no existing node",
                        kind.with_article()
                    ));
                }
                (spanless && span_hash.is_some()).then(|| {
                    format!(
                        "a staged `span_hash` on {} that has no target span",
                        kind.with_article()
                    )
                })
            }
        }
    }
}

/// The stage of a row's `staged`, `staged_at` (see the module
/// documentation): `None` when both are `NULL`; else checked, a defect
/// named as a corrupt row.
pub(super) fn decode(
    id: &str,
    kind: ProposalKind,
    status: ProposalStatus,
    new_file: bool,
    columns: [Option<String>; 2],
) -> Result<Option<StagedChoice>, UnreadableRow> {
    let (text, at) = match columns {
        [None, None] => return Ok(None),
        [Some(text), Some(at)] => (text, at),
        [None, Some(_)] => {
            return Err(corrupt(id, "staged", "it is NULL while `staged_at` is set"));
        }
        [Some(_), None] => {
            return Err(corrupt(id, "staged_at", "it is NULL while `staged` is set"));
        }
    };
    if status != ProposalStatus::Open {
        return Err(corrupt(
            id,
            "staged",
            format!("a stage on an {status} proposal (only an open one holds one)"),
        ));
    }
    if !is_utc_timestamp(&at) {
        return Err(corrupt(
            id,
            "staged_at",
            format!("{at:?} is no UTC time stamp"),
        ));
    }
    let stage = Stage::from_stored(&text).ok_or_else(|| {
        corrupt(
            id,
            "staged",
            "not the compact `{\"decision\":\"approve\",\"option\":…,\"answer\":…,\"canon\":…,\
             \"editorial\":…,\"note\":…,\"span_hash\":…}` or `{\"decision\":\"reject\",\"reason\":\
             \"…\"}`, keys in this order",
        )
    })?;
    if let Some(problem) = stage.problem(kind, new_file) {
        return Err(corrupt(id, "staged", problem));
    }
    Ok(Some(StagedChoice { stage, at }))
}

/// Queue schema 6's approve shape, keys in order, without `editorial`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApproveShape6 {
    decision: String,
    option: Option<u64>,
    answer: Option<String>,
    canon: Option<String>,
    note: Option<String>,
    span_hash: Option<String>,
}

/// A stage stored by queue schema 6 in its approve shape, byte for byte,
/// rewritten in schema 7's (`"editorial":null` after `canon`); `None` for
/// anything else (a reject, schema 7's shape, a corrupt text), which stays
/// as it is.
pub(super) fn upgrade_stage(text: &str) -> Option<String> {
    let old: ApproveShape6 = serde_json::from_str(text).ok()?;
    if old.decision != "approve" {
        return None;
    }
    let stage = Stage::Approve {
        option: old.option,
        answer: old.answer,
        canon: old.canon,
        editorial: false,
        note: old.note,
        span_hash: old.span_hash,
    };
    let upgraded = stage.to_json();
    // A string value holds no bare `"`: the first match is the key.
    let removed = upgraded.replacen(&format!(",\"{EDITORIAL}\":null"), "", 1);
    (removed == text).then_some(upgraded)
}

/// Queue step 6 → 7: every open approve stage in schema 6's shape
/// rewritten in schema 7's ([`upgrade_stage`]); `staged_at` and
/// `updated_at` kept, no event. Also run on a restored dump, so a dump of
/// schema 6 restores its stages in this build's shape.
pub(super) fn upgrade_stages(conn: &rusqlite::Connection) -> Result<(), QueueError> {
    let mut rows: Vec<(String, String, String)> = Vec::new();
    {
        let mut statement = conn
            .prepare(
                "SELECT id, project, staged FROM main.proposals WHERE staged IS NOT NULL AND \
                 status = ?1",
            )
            .db()?;
        let mut found = statement.query([ProposalStatus::Open.as_str()]).db()?;
        while let Some(row) = found.next().db()? {
            let staged: Option<String> = row.get(2).ok();
            if let (Ok(id), Ok(project), Some(staged)) = (row.get(0), row.get(1), staged) {
                rows.push((id, project, staged));
            }
        }
    }
    for (id, project, staged) in rows {
        if let Some(upgraded) = upgrade_stage(&staged) {
            conn.execute(
                "UPDATE main.proposals SET staged = ?1 WHERE id = ?2 AND project = ?3",
                [upgraded.as_str(), id.as_str(), project.as_str()],
            )
            .db()?;
        }
    }
    Ok(())
}

/// `{"id":…,"staged":{…},"staged_at":…}`, keys in this order.
fn staged_payload(id: &str, stage: &Stage, at: &str) -> String {
    let text = |value: &str| Value::String(value.to_owned()).to_string();
    format!(
        "{{\"id\":{},\"staged\":{},\"staged_at\":{}}}",
        text(id),
        stage.to_json(),
        text(at)
    )
}

impl SqliteQueue {
    /// [`ProposalQueue::stage_from`] (`stage` given) and
    /// [`ProposalQueue::unstage_from`] (`None`): the stored proposal, and
    /// whether anything was written.
    ///
    /// [`ProposalQueue::stage_from`]: super::ProposalQueue::stage_from
    /// [`ProposalQueue::unstage_from`]: super::ProposalQueue::unstage_from
    pub(super) fn stage_if(
        &mut self,
        id: &str,
        seen: &Seen,
        stage: Option<&Stage>,
        now: &str,
    ) -> Result<(Proposal, bool), QueueError> {
        check_time(now)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let current = existing(&tx, &project, id)?;
        if current.status != ProposalStatus::Open {
            return Err(status_error(current));
        }
        if current.seen() != *seen {
            return Err(changed(current));
        }
        match stage {
            Some(stage) => {
                if let Some(problem) = stage.problem(current.kind, current.new_file()) {
                    return Err(QueueError::Invalid(format!("`{id}`: {problem}")));
                }
                tx.execute(
                    "UPDATE main.proposals SET staged = ?1, staged_at = ?2, updated_at = ?2 \
                     WHERE id = ?3 AND project = ?4",
                    [stage.to_json().as_str(), now, id, project.as_str()],
                )
                .db()?;
                log_text(
                    &tx,
                    &project,
                    EVENT_STAGED,
                    &staged_payload(id, stage, now),
                    now,
                )?;
            }
            None if current.staged.is_none() => {
                // Nothing staged: nothing written, no event.
                drop(tx);
                return Ok((current, false));
            }
            None => {
                tx.execute(
                    "UPDATE main.proposals SET staged = NULL, staged_at = NULL, updated_at = ?1 \
                     WHERE id = ?2 AND project = ?3",
                    [now, id, project.as_str()],
                )
                .db()?;
                let payload = Value::Object(
                    [("id".to_owned(), Value::String(id.to_owned()))]
                        .into_iter()
                        .collect(),
                );
                log_text(&tx, &project, EVENT_UNSTAGED, &payload.to_string(), now)?;
            }
        }
        let stored = existing(&tx, &project, id)?;
        tx.commit().db()?;
        Ok((stored, true))
    }
}
