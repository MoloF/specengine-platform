//! A proposal's staged choice
//! (`docs/canon/decision-staging.md` "The stage", "Queue"): queue step
//! 4 → 5 adds `staged` (compact JSON, one of
//! the two shapes, keys in their order) and `staged_at` (the UTC time it was
//! staged), both `NULL` when nothing is staged. A stage is set, replaced and
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

use serde::{Serialize, Serializer};
use serde_json::Value;
use specengine_core::proposal::is_utc_timestamp;

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
/// `{"decision":"approve","option":…,"answer":…,"canon":…,"note":…,"span_hash":…}`
/// and `{"decision":"reject","reason":"…"}`, compact, keys in this order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stage {
    /// `spec approve PR` with these flags; `None` when absent.
    Approve {
        /// `--option N`: a discrepancy's.
        option: Option<u64>,
        /// `--answer T`: a question's.
        answer: Option<String>,
        /// `--canon REF`: a question's or a discrepancy's.
        canon: Option<String>,
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
                note,
                span_hash,
            } => ApproveShape {
                decision: "approve",
                option: *option,
                answer: answer.as_deref(),
                canon: canon.as_deref(),
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

    /// Why a proposal of `kind` (`new_file`: a create's new file) cannot
    /// hold this stage: a decision flag of a kind that takes none (an
    /// `update` or a `create` takes none; a question no `--option`; a
    /// discrepancy no `--answer`), a span hash on a kind that has no span
    /// (a deciding kind, a create's new file), a blank reason; `None`: it
    /// can.
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
                span_hash,
                ..
            } => {
                let flag = match kind {
                    ProposalKind::Update | ProposalKind::Create => {
                        if option.is_some() {
                            Some("option")
                        } else if answer.is_some() {
                            Some("answer")
                        } else if canon.is_some() {
                            Some("canon")
                        } else {
                            None
                        }
                    }
                    ProposalKind::Question => option.is_some().then_some("option"),
                    ProposalKind::Discrepancy => answer.is_some().then_some("answer"),
                };
                if let Some(flag) = flag {
                    return Some(format!(
                        "a staged `{flag}` on a {}, which takes none",
                        kind.as_str()
                    ));
                }
                let spanless = kind.decides() || new_file;
                (spanless && span_hash.is_some()).then(|| {
                    format!(
                        "a staged `span_hash` on a {} that has no target span",
                        kind.as_str()
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
             \"note\":…,\"span_hash\":…}` or `{\"decision\":\"reject\",\"reason\":\"…\"}`, keys \
             in this order",
        )
    })?;
    if let Some(problem) = stage.problem(kind, new_file) {
        return Err(corrupt(id, "staged", problem));
    }
    Ok(Some(StagedChoice { stage, at }))
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
