//! `spec feed [--since SEQ|TIME] [--needs-me] [--task T] [--department D]
//! [--json]` (task spec `auto-feed`): what happened, in `seq` order, and
//! what needs the owner. A read: no event, no stored choice, no write but
//! the index refresh any read does, and that only when a `[[workspace]]`
//! names a module (a task's lane needs the tree). It decides nothing.
//!
//! One call reads the config, this repository's tasks and, with a module,
//! the index for their lanes (`spec auto next`'s, [`LaneBook`]), then the
//! queue in one snapshot ([`SqliteQueue::open_existing`]: no schema step,
//! nothing written; no database reads as no event;
//! [`specengine_store::FeedSnapshot`]), and judges the `auto` head as `spec
//! auto show` does for the surface the reader is on ([`FeedRequest::hub`]).
//!
//! The entries are the project's events (the slug's, as the daemon's tail
//! streams them): `--since SEQ` those above it, `--since TIME` those at or
//! after it, neither the newest [`FEED_DEFAULT`]; `--task T` keeps the
//! entries whose `task` is T and the `auto.started` listing T, `--department`
//! those of its lane; at most [`FEED_MAX`], the oldest first,
//! `more` true when one more was there (the next call's `--since` is
//! `last_seq`). `last_seq` is the last entry's `seq`, else the `--since`
//! SEQ given, else `null`.
//!
//! An entry's subject is what its event is about: a `proposal.*` event's
//! proposal; else the task, else the proposal, else the card, else a line
//! `<pack>/<n>`. Its `title` follows the subject (the task's title now,
//! else the proposal's `target_id`; the proposal's `target_id`; the card's
//! title), its `status` too (the task's or the proposal's state now). A
//! task is this repository's or gone: another repository's keeps its ID,
//! `title`, `status` and `lane` `null`.
//!
//! `--needs-me` adds five lists. `held` and `unlanded` judge every `auto.*`
//! event of this repository's packs (and a hub's, which name none) whatever
//! `--since`: an item (a task, else an unbound line
//! `<pack>/<n>`, else a hub's proposal) is held while its last `auto.*`
//! event is `auto.held`, until its task is cancelled or a proposal held alone
//! is no longer `open`; `unlanded`, a pack's task of this repository now
//! `done` whose last `auto.*` event is neither `auto.held` nor
//! `auto.landed`. `open` is this repository's `open` proposals by ID
//! number. `failed_runs` and `gate_observed` read the window `--since` cuts
//! (neither: the newest [`FEED_DEFAULT`] events of the project), never
//! narrowed by `--task` or `--department`.
//!
//! ```text
//! <seq> | <at> | <type> | <subject> | <title> | <words>
//! needs me: held
//!   <seq> | <at> | <item> | <act> | <reason>: <detail>
//! needs me: open
//!   <id> | <kind> | <target_id> | <task_id> | <created_at>
//! needs me: failed runs
//!   <task> | run <n> | <outcome> | <summary's first line> | <ended_at>
//! needs me: gate observed
//!   <seq> | <at> | <path> (<zone>) | <branch>
//! needs me: unlanded
//!   <task> | <status> | pack <pack>
//! ```
//!
//! An empty list is `needs me: <list>: none`. `-` for a value absent. Every
//! value is on one line and every line's control characters are escaped;
//! the JSON holds the values raw. The notes go to stderr as `note:` lines
//! and into the JSON's `notes`, in the order they arose. Exit 0; 2 for a
//! bad flag value, no project, a root in no git worktree, or a queue of a
//! newer build.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;
use specengine_core::auto::{Surface, bare_id, event_family};
use specengine_core::proposal::is_utc_timestamp;
use specengine_core::{WorkspaceConfig, WorkspaceSource};
use specengine_model::{RunOutcome, TaskStatus};
use specengine_store::{
    EVENT_APPLIED, EVENT_APPLY_FAILED, EVENT_GATE_OBSERVED, EVENT_REJECTED, EVENT_TASK_APPROVED,
    EVENT_TASK_RUN_REPORTED, FeedEvent, FeedProposal, FeedRun, FeedTask, FeedWindow, GitEnv,
    ProposalStatus, SqliteQueue, Task, same_repository,
};

// The lane of a task under no module's workspace, and of a line: `spec auto
// next`'s.
pub use crate::auto_next::GENERAL_LANE;
use crate::auto_next::Lanes;
use crate::auto_read::{EffectiveRule, load_auto};
use crate::corpus::indexed;
use crate::location::db_path;
use crate::project::{ProjectRoot, discover_with_text};
use crate::proposals::{queue_cannot, repository_of};
use crate::task::{not_a_task, written_task};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line};

/// The entries without `--since`: the newest this many.
pub const FEED_DEFAULT: usize = 100;
/// The most entries one call gives.
pub const FEED_MAX: usize = 512;
/// The most bytes of a run's summary in the JSON.
const SUMMARY_BYTES: usize = 512;
/// The most characters of a summary's first line in a text line.
const SUMMARY_CHARS: usize = 160;
/// What a text line shows for a value absent.
const ABSENT: &str = "-";

/// `spec feed`'s options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRequest {
    /// `SEQ` (a non-negative decimal) or `TIME` (`YYYY-MM-DDTHH:MM:SSZ`), as
    /// written.
    pub since: Option<String>,
    pub needs_me: bool,
    /// A task ID as written.
    pub task: Option<String>,
    /// A department (a `[[workspace]]` name) or `general`.
    pub department: Option<String>,
    /// The `auto` head judged for a hub's surface; a terminal's otherwise.
    pub hub: bool,
    pub git: GitEnv,
}

/// `--since`, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSince {
    Seq(i64),
    Time(String),
}

/// The auto head: `spec auto show`'s judging, its first four keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedAuto {
    /// `on` or `off`.
    pub state: String,
    /// `off`, `team`, `stopped`; `null` when on.
    pub reason: Option<String>,
    pub why: Option<String>,
    /// `b3:` and 64 hex; `null` when off.
    pub rule: Option<String>,
}

/// One entry, its keys in this order, every one present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedEntry {
    pub seq: i64,
    pub at: Option<String>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub task: Option<String>,
    pub proposal: Option<String>,
    pub card: Option<String>,
    /// `<pack>/<n>`.
    pub line: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    /// `rule` when the payload says so.
    pub by: Option<String>,
    pub outcome: Option<String>,
    /// At most [`SUMMARY_BYTES`].
    pub summary: Option<String>,
    pub commit: Option<String>,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub path: Option<String>,
    pub lane: Option<String>,
    /// The text line's words.
    #[serde(skip)]
    pub words: String,
    /// The text line's subject.
    #[serde(skip)]
    pub subject: String,
}

/// An item held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedHeld {
    pub seq: i64,
    pub at: Option<String>,
    pub pack: Option<i64>,
    pub task: Option<String>,
    pub line: Option<String>,
    pub proposal: Option<String>,
    pub act: Option<String>,
    pub reason: Option<String>,
    pub detail: Option<String>,
}

/// An open proposal of this repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedOpen {
    pub id: String,
    pub kind: Option<String>,
    pub target_id: Option<String>,
    pub task_id: Option<String>,
    pub created_at: Option<String>,
}

/// A run reported in the window that did not complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedFailedRun {
    pub task: String,
    pub run: i64,
    pub outcome: Option<String>,
    pub summary: Option<String>,
    pub ended_at: Option<String>,
}

/// A `gate.observed` in the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedGateObserved {
    pub seq: i64,
    pub at: Option<String>,
    pub path: Option<String>,
    /// `null`: the config itself.
    pub zone: Option<String>,
    pub branch: Option<String>,
}

/// A pack's task done, neither landed nor held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedUnlanded {
    pub task: String,
    pub status: String,
    pub pack: i64,
}

/// `--needs-me`'s five lists, keys in this order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FeedNeedsMe {
    pub held: Vec<FeedHeld>,
    pub open: Vec<FeedOpen>,
    pub failed_runs: Vec<FeedFailedRun>,
    pub gate_observed: Vec<FeedGateObserved>,
    pub unlanded: Vec<FeedUnlanded>,
}

/// What `spec feed` read: the JSON document, keys in this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeedOutcome {
    pub auto: FeedAuto,
    /// The `--since` SEQ given, else `null`.
    pub after: Option<i64>,
    pub last_seq: Option<i64>,
    pub more: bool,
    pub entries: Vec<FeedEntry>,
    /// `null` without `--needs-me`.
    pub needs_me: Option<FeedNeedsMe>,
    pub notes: Vec<String>,
}

impl FeedOutcome {
    /// One `note:` line per note, in order, escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        self.notes
            .iter()
            .map(|note| escape_controls(&Message::Note(note.clone()).line()))
            .collect()
    }
}

/// `--since` as written: a non-negative decimal is a `seq`, a
/// `YYYY-MM-DDTHH:MM:SSZ` a time; else exit 2 naming it. The daemon judges
/// its `since` by it before anything runs.
pub fn feed_since(written: &str) -> Result<FeedSince, CliError> {
    if !written.is_empty() && written.bytes().all(|byte| byte.is_ascii_digit()) {
        if let Ok(seq) = written.parse::<i64>() {
            return Ok(FeedSince::Seq(seq));
        }
    } else if is_utc_timestamp(written) {
        return Ok(FeedSince::Time(written.to_owned()));
    }
    Err(CliError::spec(format!(
        "`--since`: `{}` is neither an event's seq (a non-negative decimal) nor a time \
         YYYY-MM-DDTHH:MM:SSZ",
        escape_controls(&one_line(written))
    )))
}

/// `--task` as written: a task ID, else exit 2 (a look-alike names its
/// Latin form). The daemon judges its `task` by it before anything runs.
pub fn feed_task(written: &str) -> Result<String, CliError> {
    match written_task(written)? {
        Some(id) => Ok(id),
        None => Err(CliError::spec(format!(
            "`--task`: {}",
            escape_controls(&not_a_task(written))
        ))),
    }
}

/// `spec feed` (see the module documentation).
pub fn feed(env: &Env, globals: &Globals, request: &FeedRequest) -> Result<FeedOutcome, CliError> {
    let since = request.since.as_deref().map(feed_since).transpose()?;
    let task = request.task.as_deref().map(feed_task).transpose()?;
    let (project, text) = discover_with_text(env, globals)?;
    let slug = project.slug()?.to_owned();
    let (_, common_dir) = repository_of(&project, &request.git)?;
    let surface = if request.hub {
        Surface::Hub
    } else {
        Surface::Local
    };
    let auto = auto_head(&load_auto(env, &project, &request.git, surface)?);
    let mut notes = Vec::new();

    let window = match &since {
        Some(FeedSince::Seq(seq)) => FeedWindow::After(*seq),
        Some(FeedSince::Time(at)) => FeedWindow::From(at.clone()),
        None => FeedWindow::Newest(FEED_DEFAULT),
    };
    let after = match &since {
        Some(FeedSince::Seq(seq)) => Some(*seq),
        _ => None,
    };
    let workspaces = WorkspaceConfig::from_toml(&text);
    let department = match &request.department {
        Some(department) if !known_department(&workspaces, department) => {
            notes.push(format!(
                "`--department`: `{department}` is neither a [[workspace]] name nor \
                 `{GENERAL_LANE}`: no entry"
            ));
            Some(None)
        }
        Some(department) => Some(Some(department.as_str())),
        None => None,
    };

    let db = db_path(env, &slug)?;
    let Some(queue) = SqliteQueue::open_existing(&db, &slug).map_err(queue_cannot)? else {
        return Ok(FeedOutcome {
            auto,
            after,
            last_seq: after,
            more: false,
            entries: Vec::new(),
            needs_me: request.needs_me.then(FeedNeedsMe::default),
            notes,
        });
    };
    let ours =
        |recorded: Option<&str>| recorded.is_some_and(|dir| same_repository(dir, &common_dir));
    // Before the snapshot, on the same connection: a task's targets never
    // change once it is made, so its lane is the index's alone.
    let book = LaneBook::read(env, &project, &workspaces, &queue, &ours)?;
    let snapshot = queue.feed_snapshot().map_err(queue_cannot)?;
    let tasks: BTreeMap<String, FeedTask> = snapshot
        .tasks()
        .map_err(queue_cannot)?
        .into_iter()
        .filter(|task| ours(task.git_common_dir.as_deref()))
        .map(|task| (task.id.clone(), task))
        .collect();
    let runs: BTreeMap<(String, i64), FeedRun> = snapshot
        .runs()
        .map_err(queue_cannot)?
        .into_iter()
        .filter(|run| tasks.contains_key(&run.task_id))
        .map(|run| ((run.task_id.clone(), run.run), run))
        .collect();
    let proposals_listed = snapshot.proposals().map_err(queue_cannot)?;
    let proposals: BTreeMap<&str, &FeedProposal> = proposals_listed
        .iter()
        .map(|proposal| (proposal.id.as_str(), proposal))
        .collect();
    let cards: BTreeMap<String, Option<String>> = snapshot
        .cards()
        .map_err(queue_cannot)?
        .into_iter()
        .map(|card| (card.id, card.title))
        .collect();
    let rows = Rows {
        tasks: &tasks,
        runs: &runs,
        proposals: &proposals,
        cards: &cards,
        lanes: &book,
    };

    let mut unparsed = BTreeSet::new();
    let (events, more) = match department {
        Some(None) => (Vec::new(), false),
        _ => snapshot
            .scan(&window, FEED_MAX, |event| {
                let entry = rows.entry(event);
                task.as_ref()
                    .is_none_or(|task| names_task(event, &entry, task))
                    && department
                        .flatten()
                        .is_none_or(|department| entry.lane.as_deref() == Some(department))
            })
            .map_err(queue_cannot)?,
    };
    let entries: Vec<FeedEntry> = events
        .iter()
        .map(|event| {
            if event.payload.is_none() {
                unparsed.insert(event.seq);
            }
            rows.entry(event)
        })
        .collect();
    notes.extend(
        unparsed
            .iter()
            .map(|seq| format!("event {seq}: its payload is not JSON")),
    );
    let last_seq = entries.last().map(|entry| entry.seq).or(after);

    let needs_me = if request.needs_me {
        let auto_events = snapshot.events_typed("auto.").map_err(queue_cannot)?;
        let windowed = snapshot
            .events_in(&window, &[EVENT_TASK_RUN_REPORTED, EVENT_GATE_OBSERVED])
            .map_err(queue_cannot)?;
        let open = proposals_listed
            .iter()
            .filter(|proposal| {
                proposal.status.as_deref() == Some(ProposalStatus::Open.as_str())
                    && ours(proposal.git_common_dir.as_deref())
            })
            .map(|proposal| FeedOpen {
                id: proposal.id.clone(),
                kind: proposal.kind.clone(),
                target_id: proposal.target_id.clone(),
                task_id: proposal.task_id.clone(),
                created_at: proposal.created_at.clone(),
            })
            .collect();
        let (held, unlanded) = held_and_unlanded(&auto_events, &tasks, &proposals, &ours);
        Some(FeedNeedsMe {
            held,
            open,
            failed_runs: failed_runs(&windowed, &runs),
            gate_observed: gate_observed(&windowed),
            unlanded,
        })
    } else {
        None
    };
    drop(snapshot);

    Ok(FeedOutcome {
        auto,
        after,
        last_seq,
        more,
        entries,
        needs_me,
        notes: notes.iter().map(|note| one_line(note)).collect(),
    })
}

/// `--task T` keeps `entry`: its `task` is T (a proposal bound to T among
/// them), or it is an `auto.started` whose `tasks` list T.
fn names_task(event: &FeedEvent, entry: &FeedEntry, task: &str) -> bool {
    entry.task.as_deref() == Some(task)
        || (event.event_type == "auto.started"
            && event.payload.as_ref().is_some_and(|payload| {
                strings(payload, "tasks")
                    .iter()
                    .any(|listed| listed == task)
            }))
}

/// `department` names a served workspace, or is `general`.
fn known_department(workspaces: &WorkspaceConfig, department: &str) -> bool {
    department == GENERAL_LANE
        || (workspaces.source == WorkspaceSource::Config
            && workspaces
                .workspaces
                .iter()
                .any(|workspace| workspace.name == department))
}

/// The rows an entry is read against.
struct Rows<'a> {
    /// This repository's, by ID.
    tasks: &'a BTreeMap<String, FeedTask>,
    /// Of this repository's tasks, by (task, run).
    runs: &'a BTreeMap<(String, i64), FeedRun>,
    /// Every repository's, by ID.
    proposals: &'a BTreeMap<&'a str, &'a FeedProposal>,
    /// By ID: the title.
    cards: &'a BTreeMap<String, Option<String>>,
    /// This repository's tasks' lanes.
    lanes: &'a LaneBook,
}

/// What an entry is about, for its subject, title and status.
enum Subject {
    Task(String),
    Proposal(String),
    Card(String),
    Line(String),
    None,
}

impl Rows<'_> {
    /// The entry of `event` (see the module documentation).
    fn entry(&self, event: &FeedEvent) -> FeedEntry {
        let mut entry = FeedEntry {
            seq: event.seq,
            at: None,
            event_type: event.event_type.clone(),
            task: None,
            proposal: None,
            card: None,
            line: None,
            title: None,
            status: None,
            by: None,
            outcome: None,
            summary: None,
            commit: None,
            reason: None,
            detail: None,
            path: None,
            lane: None,
            words: ABSENT.to_owned(),
            subject: ABSENT.to_owned(),
        };
        let Some(payload) = &event.payload else {
            return entry;
        };
        entry.at.clone_from(&event.at);
        let kind = event.event_type.as_str();
        let family = event_family(kind);
        match family {
            "task" => entry.task = text(payload, "id"),
            "card" => entry.card = text(payload, "id"),
            "proposal" => {
                entry.proposal = text(payload, "id");
                entry.task = entry
                    .proposal
                    .as_deref()
                    .and_then(|id| self.proposals.get(id))
                    .and_then(|row| row.task_id.clone());
            }
            "auto" => {
                entry.task = text(payload, "task");
                if kind == "auto.held" {
                    entry.proposal = text(payload, "proposal");
                }
                if kind == "auto.held" || kind == "auto.bound" {
                    entry.line = line_of(payload);
                }
            }
            _ => {}
        }
        if text(payload, "by").as_deref() == Some("rule") {
            entry.by = Some("rule".to_owned());
        }
        match kind {
            EVENT_TASK_RUN_REPORTED => {
                let run = entry
                    .task
                    .as_ref()
                    .zip(integer(payload, "run"))
                    .and_then(|(task, run)| self.runs.get(&(task.clone(), run)));
                if let Some(run) = run {
                    entry.outcome.clone_from(&run.outcome);
                    entry.summary = run.summary.as_deref().map(within_bytes);
                }
            }
            EVENT_APPLIED | "auto.landed" => entry.commit = text(payload, "commit"),
            EVENT_REJECTED => entry.detail = text(payload, "reason"),
            EVENT_APPLY_FAILED => {
                entry.detail = Some(format!(
                    "step {}: {}",
                    word(payload, "step"),
                    word(payload, "reason")
                ));
            }
            "auto.held" => {
                entry.reason = text(payload, "reason");
                entry.detail = text(payload, "detail");
            }
            EVENT_GATE_OBSERVED => entry.path = text(payload, "path"),
            _ => {}
        }

        let subject = if family == "proposal" && entry.proposal.is_some() {
            entry.proposal.clone().map(Subject::Proposal)
        } else {
            None
        }
        .or_else(|| entry.task.clone().map(Subject::Task))
        .or_else(|| entry.proposal.clone().map(Subject::Proposal))
        .or_else(|| entry.card.clone().map(Subject::Card))
        .or_else(|| entry.line.clone().map(Subject::Line))
        .unwrap_or(Subject::None);
        let proposal_row = |id: &str| self.proposals.get(id).copied();
        match &subject {
            Subject::Task(id) => {
                if let Some(task) = self.tasks.get(id) {
                    entry.status.clone_from(&task.status);
                    entry.title = task.title.clone().or_else(|| {
                        entry
                            .proposal
                            .as_deref()
                            .and_then(proposal_row)
                            .and_then(|row| row.target_id.clone())
                    });
                }
                entry.subject.clone_from(id);
            }
            Subject::Proposal(id) => {
                if let Some(row) = proposal_row(id) {
                    entry.status.clone_from(&row.status);
                    entry.title.clone_from(&row.target_id);
                }
                entry.subject.clone_from(id);
            }
            Subject::Card(id) => {
                entry.title = self.cards.get(id).cloned().flatten();
                entry.subject.clone_from(id);
            }
            Subject::Line(line) => entry.subject.clone_from(line),
            Subject::None => {}
        }
        entry.lane = match &entry.task {
            Some(task) if self.tasks.contains_key(task) => self.lanes.lane(task),
            Some(_) => None,
            None => entry.line.as_ref().map(|_| GENERAL_LANE.to_owned()),
        };
        entry.words = words(kind, payload, &entry);
        entry
    }
}

/// The text line's words for an event of `kind`.
fn words(kind: &str, payload: &Value, entry: &FeedEntry) -> String {
    let words = match kind {
        EVENT_TASK_RUN_REPORTED => entry.outcome.as_ref().map(|outcome| {
            let first = entry
                .summary
                .as_deref()
                .and_then(|summary| summary.lines().next())
                .map_or_else(
                    || ABSENT.to_owned(),
                    |line| line.chars().take(SUMMARY_CHARS).collect(),
                );
            format!("{outcome}: {first}")
        }),
        EVENT_TASK_APPROVED => entry.by.as_ref().map(|_| "by rule".to_owned()),
        EVENT_APPLIED => {
            let applied = entry
                .commit
                .as_deref()
                .map(|commit| format!("as {}", short(commit)));
            match (applied, entry.by.is_some()) {
                (Some(applied), true) => Some(format!("{applied} by rule")),
                (Some(applied), false) => Some(applied),
                (None, true) => Some("by rule".to_owned()),
                (None, false) => None,
            }
        }
        EVENT_REJECTED => entry
            .detail
            .as_deref()
            .and_then(|reason| reason.lines().next())
            .map(str::to_owned),
        EVENT_APPLY_FAILED => entry.detail.clone(),
        "auto.started" => Some(format!(
            "{} task(s), {} line(s) on {}",
            count(payload, "tasks"),
            count(payload, "lines"),
            word(payload, "branch")
        )),
        "auto.bound" => entry.line.as_ref().map(|line| format!("line {line}")),
        "auto.held" => Some(format!(
            "{}: {}",
            entry.reason.as_deref().unwrap_or(ABSENT),
            entry.detail.as_deref().unwrap_or(ABSENT)
        )),
        "auto.landed" => Some(format!(
            "on {} as {}",
            word(payload, "branch"),
            entry.commit.as_deref().map_or(ABSENT, short)
        )),
        EVENT_GATE_OBSERVED => Some(match text(payload, "zone") {
            Some(zone) => format!("{} ({zone})", entry.path.as_deref().unwrap_or(ABSENT)),
            None => format!("{} (config)", entry.path.as_deref().unwrap_or(ABSENT)),
        }),
        _ => None,
    };
    words.unwrap_or_else(|| ABSENT.to_owned())
}

/// The state of an item after the `auto.*` events read so far.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Last {
    /// Listed in a pack or bound to its line.
    Member,
    /// Its last `auto.*` event is this `auto.held` (its place in the list).
    Held(usize),
    Landed,
}

/// `held` and `unlanded` from every `auto.*` event (see the module
/// documentation). A held item leaves `held` once nothing waits on it: a
/// task once landed (its last `auto.*` event) or cancelled, a proposal held
/// alone (a hub's) once its row is no longer `open`, whatever decided it.
fn held_and_unlanded(
    auto_events: &[FeedEvent],
    tasks: &BTreeMap<String, FeedTask>,
    proposals: &BTreeMap<&str, &FeedProposal>,
    ours: &dyn Fn(Option<&str>) -> bool,
) -> (Vec<FeedHeld>, Vec<FeedUnlanded>) {
    // The packs of this repository: another's items are not its owner's
    // here. A hub's holds name no pack.
    let packs: BTreeSet<i64> = auto_events
        .iter()
        .filter(|event| {
            event.event_type == "auto.started"
                && ours(
                    event
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.get("repository"))
                        .and_then(Value::as_str),
                )
        })
        .map(|event| event.seq)
        .collect();
    // An item: `T-…`, `<pack>/<n>`, or a hub's `PR-…`, each tagged so
    // none collides.
    let mut last: BTreeMap<(u8, String), Last> = BTreeMap::new();
    // A task's pack and place in it, its latest.
    let mut places: BTreeMap<String, (i64, usize)> = BTreeMap::new();
    let mut pack_sizes: BTreeMap<i64, usize> = BTreeMap::new();
    for (place, event) in auto_events.iter().enumerate() {
        let Some(payload) = &event.payload else {
            continue;
        };
        let pack = match event.event_type.as_str() {
            "auto.started" => Some(event.seq),
            _ => integer(payload, "pack"),
        };
        if pack.is_some_and(|pack| !packs.contains(&pack)) {
            continue;
        }
        match event.event_type.as_str() {
            "auto.started" => {
                let pack = event.seq;
                let listed = strings(payload, "tasks");
                pack_sizes.insert(pack, listed.len());
                for (order, task) in listed.into_iter().enumerate() {
                    places.insert(task.clone(), (pack, order));
                    last.insert((0, task), Last::Member);
                }
                for n in 1..=count(payload, "lines") {
                    last.insert((1, format!("{pack}/{n}")), Last::Member);
                }
            }
            "auto.bound" => {
                if let Some(task) = text(payload, "task") {
                    let pack = integer(payload, "pack").unwrap_or_default();
                    let line = line_number(payload).unwrap_or_default();
                    let order = pack_sizes.get(&pack).copied().unwrap_or_default()
                        + usize::try_from(line).unwrap_or_default();
                    places.insert(task.clone(), (pack, order));
                    last.insert((0, task), Last::Member);
                }
            }
            "auto.held" => {
                let item = text(payload, "task")
                    .map(|task| (0, task))
                    .or_else(|| line_of(payload).map(|line| (1, line)))
                    .or_else(|| text(payload, "proposal").map(|proposal| (2, proposal)));
                if let Some(item) = item {
                    last.insert(item, Last::Held(place));
                }
            }
            "auto.landed" => {
                if let Some(task) = text(payload, "task") {
                    last.insert((0, task), Last::Landed);
                }
            }
            _ => {}
        }
    }
    let mut held: Vec<usize> = last
        .values()
        .filter_map(|state| match state {
            Last::Held(place) => Some(*place),
            _ => None,
        })
        .collect();
    held.sort_unstable();
    let held = held
        .into_iter()
        .filter_map(|place| {
            let event = &auto_events[place];
            let payload = event.payload.as_ref()?;
            Some(FeedHeld {
                seq: event.seq,
                at: event.at.clone(),
                pack: integer(payload, "pack"),
                task: text(payload, "task"),
                line: line_of(payload),
                proposal: text(payload, "proposal"),
                act: text(payload, "act"),
                reason: text(payload, "reason"),
                detail: text(payload, "detail"),
            })
        })
        .filter(|held| still_held(held, tasks, proposals))
        .collect();
    let done = TaskStatus::Done.as_str();
    let mut unlanded: Vec<(i64, usize, FeedUnlanded)> = places
        .into_iter()
        .filter(|(task, _)| last.get(&(0, task.clone())) == Some(&Last::Member))
        .filter_map(|(task, (pack, order))| {
            let status = tasks.get(&task)?.status.clone()?;
            (status == done).then_some((pack, order, FeedUnlanded { task, status, pack }))
        })
        .collect();
    unlanded.sort_by(|a, b| (a.0, a.1, &a.2.task).cmp(&(b.0, b.1, &b.2.task)));
    (
        held,
        unlanded.into_iter().map(|(_, _, task)| task).collect(),
    )
}

/// Whether `held` still waits on the owner: its task not cancelled; a
/// proposal held alone still `open`. A row the queue does not hold (another
/// repository's task, a gone proposal) stays held.
fn still_held(
    held: &FeedHeld,
    tasks: &BTreeMap<String, FeedTask>,
    proposals: &BTreeMap<&str, &FeedProposal>,
) -> bool {
    match (&held.task, &held.proposal) {
        (Some(task), _) => tasks
            .get(task)
            .is_none_or(|task| task.status.as_deref() != Some(TaskStatus::Cancelled.as_str())),
        (None, Some(proposal)) => proposals
            .get(proposal.as_str())
            .is_none_or(|row| row.status.as_deref() == Some(ProposalStatus::Open.as_str())),
        (None, None) => true,
    }
}

/// The window's runs reported `failed`, `partial` or `abandoned`, by event.
fn failed_runs(
    windowed: &[FeedEvent],
    runs: &BTreeMap<(String, i64), FeedRun>,
) -> Vec<FeedFailedRun> {
    let failing = [
        RunOutcome::Failed.as_str(),
        RunOutcome::Partial.as_str(),
        RunOutcome::Abandoned.as_str(),
    ];
    windowed
        .iter()
        .filter(|event| event.event_type == EVENT_TASK_RUN_REPORTED)
        .filter_map(|event| {
            let payload = event.payload.as_ref()?;
            let run = runs.get(&(text(payload, "id")?, integer(payload, "run")?))?;
            failing
                .contains(&run.outcome.as_deref()?)
                .then(|| FeedFailedRun {
                    task: run.task_id.clone(),
                    run: run.run,
                    outcome: run.outcome.clone(),
                    summary: run.summary.as_deref().map(within_bytes),
                    ended_at: run.ended_at.clone(),
                })
        })
        .collect()
}

/// The window's `gate.observed`, by `seq`.
fn gate_observed(windowed: &[FeedEvent]) -> Vec<FeedGateObserved> {
    windowed
        .iter()
        .filter(|event| event.event_type == EVENT_GATE_OBSERVED)
        .filter_map(|event| {
            let payload = event.payload.as_ref()?;
            Some(FeedGateObserved {
                seq: event.seq,
                at: event.at.clone(),
                path: text(payload, "path"),
                zone: text(payload, "zone"),
                branch: text(payload, "branch"),
            })
        })
        .collect()
}

/// Each of this repository's tasks' lane (task spec `auto-land`
/// "Lanes"), as `spec auto next` judges it ([`Lanes`]), each judged once,
/// when an entry first asks. Without a `[[workspace]]` naming a module
/// every lane is [`GENERAL_LANE`] and nothing more is read; with one, the
/// index (refreshed, as any read does) and this repository's tasks as
/// stored (their targets never change once made).
pub(crate) struct LaneBook {
    /// `None`: no module, every task `general`.
    judged: Option<(Lanes, BTreeMap<String, Task>)>,
    memo: RefCell<BTreeMap<String, Option<String>>>,
}

impl LaneBook {
    /// Reads what [`Self::lane`] needs, outside any open read of `queue`.
    fn read(
        env: &Env,
        project: &ProjectRoot,
        workspaces: &WorkspaceConfig,
        queue: &SqliteQueue,
        ours: &dyn Fn(Option<&str>) -> bool,
    ) -> Result<Self, CliError> {
        let modules = workspaces.source == WorkspaceSource::Config
            && workspaces.workspaces.iter().any(|workspace| {
                workspace.module.as_ref().is_some_and(|name| {
                    workspaces.modules.iter().any(|module| &module.name == name)
                })
            });
        let judged = if modules {
            let listed = queue.list_existing_tasks().map_err(queue_cannot)?;
            let tasks: BTreeMap<String, Task> = listed
                .tasks
                .into_iter()
                .filter(|task| ours(Some(&task.git_common_dir)))
                .map(|task| (task.id.clone(), task))
                .collect();
            let mut messages = Vec::new();
            let input = indexed(env, project, &mut messages, false)?;
            Some((Lanes::read(project, &input), tasks))
        } else {
            None
        };
        Ok(Self {
            judged,
            memo: RefCell::new(BTreeMap::new()),
        })
    }

    /// The lane of this repository's task `id` (the caller asks for no
    /// other); `None` for one the index read does not hold.
    fn lane(&self, id: &str) -> Option<String> {
        let Some((lanes, tasks)) = &self.judged else {
            return Some(GENERAL_LANE.to_owned());
        };
        if let Some(known) = self.memo.borrow().get(id) {
            return known.clone();
        }
        let lane = tasks.get(id).map(|task| lanes.lane(Some(task)));
        self.memo.borrow_mut().insert(id.to_owned(), lane.clone());
        lane
    }
}

/// A payload's text under `key`.
fn text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// A payload's integer under `key`.
fn integer(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

/// A payload's value under `key` for a text line: a text as it is, any
/// other value as JSON writes it, `-` when absent or `null`.
fn word(payload: &Value, key: &str) -> String {
    match payload.get(key) {
        None | Some(Value::Null) => ABSENT.to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

/// How many entries the array under `key` holds; 0 when it is none.
fn count(payload: &Value, key: &str) -> usize {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// The texts of the array under `key`.
fn strings(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The line number of a payload's `line`: a number, or the `n` of a text
/// `<n>` or `<pack>/<n>`.
fn line_number(payload: &Value) -> Option<i64> {
    match payload.get("line")? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => bare_id(text).parse().ok(),
        _ => None,
    }
}

/// A payload's line as `<pack>/<n>`; `None` when it names none.
fn line_of(payload: &Value) -> Option<String> {
    match payload.get("line")? {
        Value::String(text) if bare_id(text) != text => Some(text.clone()),
        Value::Null => None,
        _ => {
            let n = line_number(payload)?;
            Some(match integer(payload, "pack") {
                Some(pack) => format!("{pack}/{n}"),
                None => n.to_string(),
            })
        }
    }
}

/// A commit's first 12 characters.
fn short(commit: &str) -> &str {
    commit
        .char_indices()
        .nth(12)
        .map_or(commit, |(end, _)| &commit[..end])
}

/// `text` cut to at most [`SUMMARY_BYTES`] bytes, at a character boundary.
fn within_bytes(text: &str) -> String {
    if text.len() <= SUMMARY_BYTES {
        return text.to_owned();
    }
    let mut end = SUMMARY_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// One text field: on one line, `-` when absent.
fn field(value: Option<&str>) -> String {
    one_line(value.unwrap_or(ABSENT))
}

/// The stdout text (see the module documentation).
pub(crate) fn render_text(outcome: &FeedOutcome) -> String {
    let mut lines: Vec<String> = outcome
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{} | {} | {} | {} | {} | {}",
                entry.seq,
                field(entry.at.as_deref()),
                one_line(&entry.event_type),
                one_line(&entry.subject),
                field(entry.title.as_deref()),
                one_line(&entry.words)
            )
        })
        .collect();
    if let Some(needs) = &outcome.needs_me {
        section(&mut lines, "held", needs.held.iter().map(held_line));
        section(
            &mut lines,
            "open",
            needs.open.iter().map(|open| {
                format!(
                    "{} | {} | {} | {} | {}",
                    one_line(&open.id),
                    field(open.kind.as_deref()),
                    field(open.target_id.as_deref()),
                    field(open.task_id.as_deref()),
                    field(open.created_at.as_deref())
                )
            }),
        );
        section(
            &mut lines,
            "failed runs",
            needs.failed_runs.iter().map(|run| {
                let first = run
                    .summary
                    .as_deref()
                    .and_then(|summary| summary.lines().next())
                    .map(|line| line.chars().take(SUMMARY_CHARS).collect::<String>());
                format!(
                    "{} | run {} | {} | {} | {}",
                    one_line(&run.task),
                    run.run,
                    field(run.outcome.as_deref()),
                    field(first.as_deref()),
                    field(run.ended_at.as_deref())
                )
            }),
        );
        section(
            &mut lines,
            "gate observed",
            needs.gate_observed.iter().map(|observed| {
                format!(
                    "{} | {} | {} ({}) | {}",
                    observed.seq,
                    field(observed.at.as_deref()),
                    field(observed.path.as_deref()),
                    one_line(observed.zone.as_deref().unwrap_or("config")),
                    field(observed.branch.as_deref())
                )
            }),
        );
        section(
            &mut lines,
            "unlanded",
            needs.unlanded.iter().map(|task| {
                format!(
                    "{} | {} | pack {}",
                    one_line(&task.task),
                    one_line(&task.status),
                    task.pack
                )
            }),
        );
    }
    let mut out = String::new();
    for line in lines {
        out.push_str(&escape_controls(&line));
        out.push('\n');
    }
    out
}

/// A held item's line.
fn held_line(held: &FeedHeld) -> String {
    let item = held
        .task
        .as_deref()
        .or(held.line.as_deref())
        .or(held.proposal.as_deref());
    format!(
        "{} | {} | {} | {} | {}: {}",
        held.seq,
        field(held.at.as_deref()),
        field(item),
        field(held.act.as_deref()),
        field(held.reason.as_deref()),
        field(held.detail.as_deref())
    )
}

/// `needs me: <name>` and its rows indented, or `needs me: <name>: none`.
fn section(lines: &mut Vec<String>, name: &str, rows: impl Iterator<Item = String>) {
    let rows: Vec<String> = rows.map(|row| format!("  {row}")).collect();
    if rows.is_empty() {
        lines.push(format!("needs me: {name}: none"));
    } else {
        lines.push(format!("needs me: {name}"));
        lines.extend(rows);
    }
}

/// The `auto` head: `spec auto show`'s judging of `rule`, its first keys.
fn auto_head(rule: &EffectiveRule) -> FeedAuto {
    let switch = rule.switch();
    let on = switch.is_none().then_some(rule.rule.as_ref()).flatten();
    FeedAuto {
        state: if on.is_some() { "on" } else { "off" }.to_owned(),
        reason: switch
            .as_ref()
            .map(|(reason, _)| reason.as_str().to_owned()),
        why: switch.map(|(_, why)| why),
        rule: on.map(|rule| rule.hash.clone()),
    }
}
