//! Auto mode's side of the queue (`docs/canon/auto-mode.md` "Events"; task
//! specs `auto-rule` "Events and the apply seam", `auto-land`): four event
//! types in `events`, no table, no schema step, no backup format step (a
//! dump keeps them as any event).
//!
//! - **Events**, payload keys in the canon's order, `null` written, never
//!   skipped: `auto.started` (a pack, named by its `seq`), `auto.bound` (a
//!   task bound to a pack's line), `auto.held` (an item held, with its act,
//!   reason and detail) and `auto.landed`. [`SqliteQueue::record_auto`]
//!   writes one in one `Immediate` transaction; [`SqliteQueue::auto_events`]
//!   reads a repository's, by `seq`.
//! - **By rule**: a handle given a [`ByRule`] ([`SqliteQueue::set_by_rule`])
//!   writes `proposal.approved` and `proposal.applied` with today's keys,
//!   then `"by":"rule"`, `rule` and, locally, `pack`; a handle with none
//!   writes them as 0.46.1 did, byte for byte.
//!   [`SqliteQueue::approve_task_by_rule`] approves a task with
//!   `task.approved {id, by: "rule", pack, rule}`;
//!   [`SqliteQueue::rule_applies_on`] counts a day's delegated applies.
//! - **A line's binding**: [`SqliteQueue::create_task_bound`] stores a task
//!   as `draft` and, in the same transaction, binds it to a pack's line when
//!   that line exists and no earlier `auto.bound` names it.

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specengine_core::auto_toml::CanonicalRule;

use super::tasks::{
    TaskChange, TaskSeen, TaskSnapshot, existing_task, insert_draft, listed_tasks,
    new_task_problem, next_task_id,
};
use super::{NewTask, QueueError, SqliteQueue, Task, check_time, log_text, to_json};
use crate::error::Db;
use crate::same_repository;

/// `auto.started`: a pack, named by its `seq`.
pub const EVENT_AUTO_STARTED: &str = "auto.started";
/// `auto.bound`: a task bound to a pack's line.
pub const EVENT_AUTO_BOUND: &str = "auto.bound";
/// `auto.held`: an item held.
pub const EVENT_AUTO_HELD: &str = "auto.held";
/// `auto.landed`: a task landed on the rule's branch.
pub const EVENT_AUTO_LANDED: &str = "auto.landed";
/// The four, in the canon's order.
pub const AUTO_EVENTS: [&str; 4] = [
    EVENT_AUTO_STARTED,
    EVENT_AUTO_BOUND,
    EVENT_AUTO_HELD,
    EVENT_AUTO_LANDED,
];

/// `"rule"`: the `by` of a delegated act's event.
pub const BY_RULE: &str = "rule";

/// `auto.started`'s payload, keys in this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoStarted {
    /// The repository's canonical git common dir.
    pub repository: String,
    /// The pack's tasks, in the order given.
    pub tasks: Vec<String>,
    /// Its lines' texts, `<pack>/<n>` from 1 in this order.
    pub lines: Vec<String>,
    /// `--until`, `YYYY-MM-DDTHH:MM:SSZ`.
    pub until: Option<String>,
    pub branch: String,
    /// The commit the branch was at (or was made at) when the pack started.
    pub branch_at: String,
    /// The rule's hash at the start.
    pub rule: String,
    /// The rule's canonical form at the start.
    pub frozen: CanonicalRule,
    /// The owner's git identity, as `Decided-by` reads it.
    pub by: String,
}

/// `auto.bound`'s payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoBound {
    pub pack: i64,
    /// From 1.
    pub line: u32,
    pub task: String,
}

/// `auto.held`'s payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoHeld {
    /// The pack, locally; `None` (`null`) for a hub's hold.
    pub pack: Option<i64>,
    /// The held task; `None` for an unbound line.
    pub task: Option<String>,
    /// The held line's number, when the item is a line.
    pub line: Option<u32>,
    /// The held proposal of an `apply`.
    pub proposal: Option<String>,
    /// `approve`, `apply`, `land` or `run`.
    pub act: String,
    pub reason: String,
    pub detail: String,
    pub rule: String,
}

/// `auto.landed`'s payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoLanded {
    pub pack: i64,
    pub task: String,
    pub branch: String,
    /// The merge commit.
    pub commit: String,
    /// The code commit the land made; `None` when nothing was uncommitted.
    pub code_commit: Option<String>,
    pub applied: Vec<String>,
    pub left: Vec<String>,
    pub rule: String,
}

/// One `auto.*` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoEvent {
    Started(AutoStarted),
    Bound(AutoBound),
    Held(AutoHeld),
    Landed(AutoLanded),
}

impl AutoEvent {
    /// Its `type`.
    pub const fn event_type(&self) -> &'static str {
        match self {
            Self::Started(_) => EVENT_AUTO_STARTED,
            Self::Bound(_) => EVENT_AUTO_BOUND,
            Self::Held(_) => EVENT_AUTO_HELD,
            Self::Landed(_) => EVENT_AUTO_LANDED,
        }
    }

    /// Its payload as stored: compact JSON, keys in the canon's order.
    pub fn payload(&self) -> Result<String, QueueError> {
        match self {
            Self::Started(payload) => to_json(payload),
            Self::Bound(payload) => to_json(payload),
            Self::Held(payload) => to_json(payload),
            Self::Landed(payload) => to_json(payload),
        }
    }

    /// The event of `event_type` with `payload`; `None` for another type or
    /// a payload that does not decode.
    pub fn decode(event_type: &str, payload: &str) -> Option<Self> {
        match event_type {
            EVENT_AUTO_STARTED => serde_json::from_str(payload).ok().map(Self::Started),
            EVENT_AUTO_BOUND => serde_json::from_str(payload).ok().map(Self::Bound),
            EVENT_AUTO_HELD => serde_json::from_str(payload).ok().map(Self::Held),
            EVENT_AUTO_LANDED => serde_json::from_str(payload).ok().map(Self::Landed),
            _ => None,
        }
    }

    /// The pack it names: its own `seq` for `auto.started` (given); `None`
    /// for a hub's hold.
    pub fn pack(&self, seq: i64) -> Option<i64> {
        match self {
            Self::Started(_) => Some(seq),
            Self::Bound(bound) => Some(bound.pack),
            Self::Held(held) => held.pack,
            Self::Landed(landed) => Some(landed.pack),
        }
    }
}

/// One stored `auto.*` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAuto {
    pub seq: i64,
    pub at: String,
    pub event: AutoEvent,
}

/// What a delegated apply's `proposal.approved` and `proposal.applied` add
/// after today's keys: `"by":"rule"`, `rule`, and `pack` when set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByRule {
    /// The rule's hash.
    pub rule: String,
    /// The pack, locally; `None` on a hub.
    pub pack: Option<i64>,
}

/// A line to bind a new task to ([`SqliteQueue::create_task_bound`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoBind {
    pub pack: i64,
    /// From 1.
    pub line: u32,
}

/// `task.approved` by rule, keys in this order.
#[derive(Serialize)]
struct TaskApprovedByRule<'a> {
    id: &'a str,
    by: &'a str,
    pack: i64,
    rule: &'a str,
}

/// `payload` as stored, with `by` added after today's keys when given:
/// `"by":"rule"`, `rule`, `pack` (when set). `None`: today's text, byte for
/// byte.
pub(crate) fn ruled_text(payload: &Value, by: Option<&ByRule>) -> String {
    let text = payload.to_string();
    let Some(by) = by else {
        return text;
    };
    let Some(open) = text.strip_suffix('}') else {
        return text;
    };
    let rule = Value::String(by.rule.clone());
    let pack = by
        .pack
        .map_or_else(String::new, |pack| format!(",\"pack\":{pack}"));
    let comma = if open.len() > 1 { "," } else { "" };
    format!("{open}{comma}\"by\":\"{BY_RULE}\",\"rule\":{rule}{pack}}}")
}

impl SqliteQueue {
    /// Delegated acts' events from now on (see the module documentation);
    /// `None` gives back today's events. The CLI holds it for one run and
    /// resets it when the run's context drops (`apply.rs` `Delegated`).
    pub fn set_by_rule(&mut self, by: Option<ByRule>) {
        self.by_rule = by;
    }

    /// What [`Self::set_by_rule`] set.
    pub fn by_rule(&self) -> Option<&ByRule> {
        self.by_rule.as_ref()
    }

    /// Records `event` at `now` in one `Immediate` transaction: its `seq`.
    pub fn record_auto(&mut self, event: &AutoEvent, now: &str) -> Result<i64, QueueError> {
        check_time(now)?;
        let payload = event.payload()?;
        let project = self.project.clone();
        let tx = self.write()?;
        log_text(&tx, &project, event.event_type(), &payload, now)?;
        let seq = tx.last_insert_rowid();
        tx.commit().db()?;
        Ok(seq)
    }

    /// The project's `auto.*` events of the repository `repository` (its
    /// canonical git common dir), by `seq`, in one snapshot: each
    /// `auto.started` recorded there, every other event naming one of those
    /// packs, and a hub's `auto.held` (no pack). An event whose payload
    /// does not decode is skipped.
    pub fn auto_events(&self, repository: &str) -> Result<Vec<StoredAuto>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let events = auto_events_in(&tx, &self.project, Some(repository))?;
        tx.commit().db()?;
        Ok(events)
    }

    /// `review` → `ready` by the rule, `snapshot` frozen, as a
    /// compare-and-set on `seen` ([`SqliteQueue::change_task`]'s
    /// refusals), logging `task.approved {id, by: "rule", pack, rule}`, in
    /// one transaction.
    pub fn approve_task_by_rule(
        &mut self,
        id: &str,
        seen: &TaskSeen,
        snapshot: &TaskSnapshot,
        pack: i64,
        rule: &str,
        now: &str,
    ) -> Result<Task, QueueError> {
        let payload = to_json(&TaskApprovedByRule {
            id,
            by: BY_RULE,
            pack,
            rule,
        })?;
        self.change_task_logging(
            id,
            seen,
            &TaskChange::Approve {
                snapshot: snapshot.clone(),
            },
            Some(&payload),
            now,
        )
        .map(|(task, _)| task)
    }

    /// How many of the project's `proposal.applied` events say `by:
    /// "rule"` with an `at` on `date` (`YYYY-MM-DD`): the hub's cap.
    pub fn rule_applies_on(&self, date: &str) -> Result<u32, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        // A payload that is no JSON counts as none (`json_extract` would
        // fail the whole read on it).
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM main.events WHERE project = ?1 AND type = ?2 \
                 AND at LIKE ?3 || '%' \
                 AND CASE WHEN json_valid(payload) THEN json_extract(payload, '$.by') END = ?4",
                [self.project.as_str(), super::EVENT_APPLIED, date, BY_RULE],
                |row| row.get(0),
            )
            .db()?;
        tx.commit().db()?;
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }

    /// The payload of the proposal `id`'s last `proposal.applied` event of
    /// this project, in one read (a review's `delegated`, task spec
    /// `hub-auto`); `None` without one. A payload that is no JSON names no
    /// proposal.
    pub fn applied_payload(&self, id: &str) -> Result<Option<Value>, QueueError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred).db()?;
        let text: Option<String> = tx
            .query_row(
                "SELECT payload FROM main.events WHERE project = ?1 AND type = ?2 \
                 AND CASE WHEN json_valid(payload) THEN json_extract(payload, '$.id') END = ?3 \
                 ORDER BY seq DESC LIMIT 1",
                [self.project.as_str(), super::EVENT_APPLIED, id],
                |row| row.get(0),
            )
            .optional()
            .db()?
            .flatten();
        tx.commit().db()?;
        Ok(text.and_then(|text| serde_json::from_str(&text).ok()))
    }

    /// [`Self::record_auto`] of `event` only when `check`, given the
    /// repository `repository`'s `auto.*` events and tasks as read inside the
    /// same `Immediate` transaction, finds nothing against it: `Ok(Ok(seq))`
    /// recorded; `Ok(Err(why))` nothing written (`spec auto start`'s "a task
    /// is in at most one live pack", judged again under the write lock).
    pub fn record_auto_checked<F>(
        &mut self,
        event: &AutoEvent,
        repository: &str,
        now: &str,
        check: F,
    ) -> Result<Result<i64, String>, QueueError>
    where
        F: FnOnce(&[StoredAuto], &[Task]) -> Option<String>,
    {
        check_time(now)?;
        let payload = event.payload()?;
        let project = self.project.clone();
        let tx = self.write()?;
        let events = auto_events_in(&tx, &project, Some(repository))?;
        let tasks = listed_tasks(&tx, &project, Some(repository))?;
        if let Some(why) = check(&events, &tasks.tasks) {
            return Ok(Err(why));
        }
        log_text(&tx, &project, event.event_type(), &payload, now)?;
        let seq = tx.last_insert_rowid();
        tx.commit().db()?;
        Ok(Ok(seq))
    }

    /// [`Self::create_task`], and when `bind` is given, in the same
    /// transaction, the task bound to that line: its pack an `auto.started`
    /// of the task's repository holding line `bind.line`, no earlier
    /// `auto.bound` naming it; then `auto.bound {pack, line, task}` after
    /// `task.created`. The stored task, and the binding or why none was made
    /// (`no live pack <n> here`, `pack <n> has no line <k>`, `line <n>/<k> is
    /// bound to T-…`); the task is stored either way. Liveness and the
    /// rule's switches are the caller's.
    pub fn create_task_bound(
        &mut self,
        task: &NewTask,
        bind: Option<AutoBind>,
        now: &str,
    ) -> Result<(Task, Option<Result<AutoBound, String>>), QueueError> {
        check_time(now)?;
        new_task_problem(task)?;
        let project = self.project.clone();
        let tx = self.write()?;
        let id = next_task_id(&tx)?;
        insert_draft(&tx, &project, &id, task, now)?;
        let bound = match bind {
            None => None,
            Some(bind) => {
                let events = auto_events_in(&tx, &project, Some(&task.git_common_dir))?;
                let verdict = binding(&events, bind, &id);
                if let Ok(bound) = &verdict {
                    log_text(&tx, &project, EVENT_AUTO_BOUND, &to_json(bound)?, now)?;
                }
                Some(verdict)
            }
        };
        let stored = existing_task(&tx, &project, &id)?;
        tx.commit().db()?;
        Ok((stored, bound))
    }
}

/// Whether `bind` may bind `task` among the repository's `events`.
fn binding(events: &[StoredAuto], bind: AutoBind, task: &str) -> Result<AutoBound, String> {
    let AutoBind { pack, line } = bind;
    let started = events.iter().find_map(|stored| match &stored.event {
        AutoEvent::Started(started) if stored.seq == pack => Some(started),
        _ => None,
    });
    let Some(started) = started else {
        return Err(format!("no live pack {pack} here"));
    };
    let lines = u32::try_from(started.lines.len()).unwrap_or(u32::MAX);
    if line == 0 || line > lines {
        return Err(format!("pack {pack} has no line {line}"));
    }
    let taken = events.iter().find_map(|stored| match &stored.event {
        AutoEvent::Bound(bound) if bound.pack == pack && bound.line == line => {
            Some(bound.task.clone())
        }
        _ => None,
    });
    if let Some(other) = taken {
        return Err(format!("line {pack}/{line} is bound to {other}"));
    }
    Ok(AutoBound {
        pack,
        line,
        task: task.to_owned(),
    })
}

/// [`SqliteQueue::auto_events`] in the caller's snapshot; `repository`
/// `None`: every repository's.
fn auto_events_in(
    tx: &Transaction<'_>,
    project: &str,
    repository: Option<&str>,
) -> Result<Vec<StoredAuto>, QueueError> {
    let rows = {
        let mut statement = tx
            .prepare(
                "SELECT seq, type, payload, at FROM main.events WHERE project = ?1 \
                 AND type IN (?2, ?3, ?4, ?5) ORDER BY seq",
            )
            .db()?;
        statement
            .query_map(
                rusqlite::params![
                    project,
                    EVENT_AUTO_STARTED,
                    EVENT_AUTO_BOUND,
                    EVENT_AUTO_HELD,
                    EVENT_AUTO_LANDED
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .db()?
            .collect::<rusqlite::Result<Vec<_>>>()
            .db()?
    };
    let mut packs = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (seq, event_type, payload, at) in rows {
        let (Some(event_type), Some(payload), Some(at)) = (event_type, payload, at) else {
            continue;
        };
        let Some(event) = AutoEvent::decode(&event_type, &payload) else {
            continue;
        };
        let kept = match &event {
            AutoEvent::Started(started) => {
                let here = repository.is_none_or(|repository| {
                    same_repository(&started.repository, std::path::Path::new(repository))
                });
                if here {
                    packs.insert(seq);
                }
                here
            }
            // A hub's hold names no pack: the project's alone.
            other => other.pack(seq).is_none_or(|pack| packs.contains(&pack)),
        };
        if kept {
            out.push(StoredAuto { seq, at, event });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn ruled_text_appends_after_todays_keys() {
        let payload = json!({ "id": "PR-0001", "commit": "abc" });
        assert_eq!(ruled_text(&payload, None), payload.to_string());
        let by = ByRule {
            rule: "b3:00".to_owned(),
            pack: Some(812),
        };
        let text = ruled_text(&payload, Some(&by));
        assert!(
            text.ends_with(",\"by\":\"rule\",\"rule\":\"b3:00\",\"pack\":812}"),
            "{text}"
        );
        assert!(text.starts_with(&payload.to_string()[..payload.to_string().len() - 1]));
        let hub = ByRule {
            rule: "b3:00".to_owned(),
            pack: None,
        };
        assert!(ruled_text(&payload, Some(&hub)).ends_with("\"rule\":\"b3:00\"}"));
    }

    #[test]
    fn payloads_keep_the_canons_order() {
        let held = AutoEvent::Held(AutoHeld {
            pack: Some(3),
            task: None,
            line: Some(1),
            proposal: None,
            act: "run".to_owned(),
            reason: "denied-tool".to_owned(),
            detail: "Bash".to_owned(),
            rule: "b3:00".to_owned(),
        });
        let text = held.payload().unwrap();
        assert_eq!(
            text,
            "{\"pack\":3,\"task\":null,\"line\":1,\"proposal\":null,\"act\":\"run\",\"reason\":\"denied-tool\",\"detail\":\"Bash\",\"rule\":\"b3:00\"}"
        );
        assert_eq!(AutoEvent::decode(EVENT_AUTO_HELD, &text), Some(held));
    }
}
