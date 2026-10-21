//! `spec roadmap [--release R | --pool] [--json]` (canon `spec-roadmap`,
//! "Command", "Position"): the plan of the project's `[roadmap]` items,
//! computed per read from the repository's files alone (core's
//! [`specengine_core::plan`]) over the index the call updates, as `spec
//! tree` reads it, then joined with this repository's open tasks (core's
//! [`specengine_core::plan_tasks`]), read as the queue holds them: no
//! schema step, no table added, never an exit. It writes nothing under the
//! project root.
//!
//! `--release R` lists that release's group, `--pool` the pool and the
//! unplaced items, neither every group; the first line and the JSON's
//! top-level keys are always the whole plan's.
//!
//! ```text
//! roadmap: <o> open, <d> done; current <release or none>[; to <cut> <sum>]; total <sum>
//! release <name>[ | current][ | mvp cut] | <o> open, <d> done | <sum>
//!   <position>. <ID> | <title or -> | <status or -> | <priority or -> | <estimate or ->[ | waits on <ID>, …][ | cycle] | <path>
//! pool | <o> open, <d> done | <sum>
//!   - <ID> | <title or -> | <status or -> | <priority or -> | <estimate or ->[ | waits on <ID>, …] | <path>
//! unplaced | <o> open, <d> done | <sum>
//!   - <ID> | … as pool … | release <as written, or mistyped>
//! [truncated: <k> of <n> items not shown; add --release R or --pool]
//! ```
//!
//! An item some open task executes ends its line with `| task <T> <state>,
//! …`, by number; the open tasks executing no open item print after the
//! pool's items (`--pool` or neither flag), `  - <T> | <state> | <title or
//! -> | <targets or ->`, and count as items in the cut. The view never
//! reads the queue: the cell and the task lines are this command's alone.
//!
//! `<sum>`: `<min>-<max>` (`<min>` when equal), ` <unit>` when set, `, <u>
//! unestimated` when some are. The lines are core's ([`Plan::summary`],
//! `PlanGroup::header`, `PlanItem::line`), shared with the index entry's
//! roadmap view. Everything before the tail is at most
//! [`OUTPUT_CAP_CHARS`] characters: lines print whole, in order, until the
//! next would pass it; the tail only when an item is not shown
//! (canon `spec-roadmap`, "Cut"); the JSON holds what the text prints
//! ([`roadmap_with_view`] with the daemon's [`View::Browser`]: nothing cut,
//! `truncated` false, `omitted` 0). The plan's
//! notes go to stderr as `note:` lines and into the JSON's `notes`; none
//! changes the exit. The queue's notes follow them on stderr alone (a
//! queue that cannot be read, a row that does not decode, another
//! repository's open tasks); no queue table, no task row or a root in no git
//! repository adds none.
//! Exit 1: `--release R` names no release (stderr, and the JSON with
//! `reason` and no group); exit 2: no project, a config error, no or a bad
//! `[roadmap]`, an index that cannot be updated.

use std::path::PathBuf;

use serde::Serialize;
use specengine_core::proposal::prefix_clash;
use specengine_core::task::task_prefix_clash;
use specengine_core::{
    GroupKind, Plan, PlanItem, PlanTask, PlanTasks, RoadmapConfig, Sum, plan, plan_tasks,
};
use specengine_model::TaskStatus;
use specengine_store::{GitEnv, SqliteQueue, Task, TaskList, WorktreeGit, same_repository};

use crate::cap::{OUTPUT_CAP_CHARS, View};
use crate::corpus::indexed;
use crate::location::db_path;
use crate::project::{ProjectRoot, config_error, discover_with_text};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line};

/// `spec roadmap` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapRequest {
    /// `--release R`: that release's group only.
    pub release: Option<String>,
    /// `--pool`: the pool and the unplaced items only.
    pub pool: bool,
    /// The caller's environment: git finds the reading repository without
    /// its local `GIT_*` variables.
    pub git: GitEnv,
}

/// What `spec roadmap` computed and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapOutcome {
    /// `--release R` as given.
    pub release: Option<String>,
    /// `--pool`.
    pub pool: bool,
    /// Why `--release R` lists nothing (exit 1).
    pub reason: Option<String>,
    /// The index update's notes and warnings, the plan's notes, then the
    /// queue's.
    pub messages: Vec<Message>,
    /// `[roadmap] prefix`.
    pub prefix: String,
    /// `[roadmap] dir`.
    pub dir: String,
    /// `[roadmap] unit`.
    pub unit: Option<String>,
    /// The whole plan.
    pub plan: Plan,
    /// The groups the request lists, as indexes into `plan.groups`.
    pub listed: Vec<usize>,
    /// The groups whose header prints, each with how many of its items
    /// print, in order.
    pub shown: Vec<(usize, usize)>,
    /// A listed item or pool task does not print (`k` > 0): the tail and
    /// the JSON's `truncated`.
    pub truncated: bool,
    /// The plan's open tasks of this repository (core's
    /// [`specengine_core::plan_tasks`]); none when the queue holds none or
    /// was not read.
    pub tasks: PlanTasks,
    /// How many of [`PlanTasks::pool`] print (none with `--release R`).
    pub shown_pool_tasks: usize,
    /// The queue's notes, also in `messages` after the plan's.
    pub task_notes: Vec<String>,
}

impl RoadmapOutcome {
    /// The items the request lists, its pool tasks counted as items (`n`
    /// of the tail).
    pub fn requested(&self) -> usize {
        self.listed
            .iter()
            .map(|&group| {
                let group = &self.plan.groups[group];
                let tasks = if group.kind == GroupKind::Pool {
                    self.tasks.pool.len()
                } else {
                    0
                };
                group.items.len() + tasks
            })
            .sum()
    }

    /// The listed items and pool tasks that do not print (`k` of the tail).
    pub fn omitted(&self) -> usize {
        let shown: usize = self.shown.iter().map(|&(_, items)| items).sum();
        self.requested()
            .saturating_sub(shown + self.shown_pool_tasks)
    }

    /// The item's line with its task cell ([`task_cell`]).
    fn item_line(&self, item: &PlanItem, kind: GroupKind) -> String {
        let mut line = item.line(kind);
        line.push_str(&task_cell(self.tasks.of(&item.id)));
        line
    }

    /// The pool tasks that print, by number.
    fn printed_pool_tasks(&self) -> &[PlanTask] {
        let pool = &self.tasks.pool;
        &pool[..self.shown_pool_tasks.min(pool.len())]
    }
}

/// `spec roadmap`: reads `[roadmap]`, updates the index, computes the plan
/// and joins this repository's open tasks to it.
pub fn roadmap(
    env: &Env,
    globals: &Globals,
    request: &RoadmapRequest,
) -> Result<RoadmapOutcome, CliError> {
    roadmap_with_view(env, globals, request, View::Capped)
}

/// `--release R` with `--pool`: exit 2, `spec: --release and --pool cannot
/// be used together`, before anything is read. The daemon judges its
/// `release` and `pool` by it before anything runs.
pub fn roadmap_flags(release: Option<&str>, pool: bool) -> Result<(), CliError> {
    if release.is_some() && pool {
        return Err(CliError::spec(
            "--release and --pool cannot be used together",
        ));
    }
    Ok(())
}

/// [`roadmap`], its answer bounded by `view` (the daemon's
/// [`View::Browser`]: every listed item and pool task shown, `truncated`
/// false, `omitted` 0).
pub fn roadmap_with_view(
    env: &Env,
    globals: &Globals,
    request: &RoadmapRequest,
    view: View,
) -> Result<RoadmapOutcome, CliError> {
    roadmap_flags(request.release.as_deref(), request.pool)?;
    let (project, text) = discover_with_text(env, globals)?;
    let config = RoadmapConfig::from_toml(&text, &project.config.scheme)
        .map_err(|error| config_error(&project.config_label, &error))?
        .ok_or_else(|| CliError::spec(format!("{} has no [roadmap]", project.config_label)))?;
    let slug = project.slug()?.to_owned();
    let mut messages = Vec::new();
    let input = indexed(env, &project, &mut messages, true)?;
    let plan = plan(&input, &config);
    messages.extend(plan.notes.iter().cloned().map(Message::Note));
    let (rows, task_notes) = queue_tasks(env, &project, &slug, &request.git);
    let tasks = plan_tasks(&plan, &rows);
    messages.extend(task_notes.iter().cloned().map(Message::Note));

    let mut outcome = RoadmapOutcome {
        release: request.release.clone(),
        pool: request.pool,
        reason: None,
        messages,
        prefix: config.prefix.clone(),
        dir: config.dir.clone(),
        unit: config.unit.clone(),
        listed: Vec::new(),
        shown: Vec::new(),
        truncated: false,
        plan,
        tasks,
        shown_pool_tasks: 0,
        task_notes,
    };
    let groups = &outcome.plan.groups;
    outcome.listed = match &request.release {
        Some(release) => {
            let found = groups.iter().position(|group| {
                group.kind == GroupKind::Release && group.name.as_ref() == Some(release)
            });
            match found {
                Some(group) => vec![group],
                None => {
                    outcome.reason = Some(format!(
                        "no release \"{release}\" in [roadmap] releases ({})",
                        config.releases.join(", ")
                    ));
                    return Ok(outcome);
                }
            }
        }
        None if request.pool => (0..groups.len())
            .filter(|&group| groups[group].kind != GroupKind::Release)
            .collect(),
        None => (0..groups.len()).collect(),
    };
    match view {
        View::Capped => cut(&mut outcome),
        View::Browser => uncut(&mut outcome),
    }
    Ok(outcome)
}

/// Every listed group, item and pool task shown (the browser view).
fn uncut(outcome: &mut RoadmapOutcome) {
    let groups = &outcome.plan.groups;
    outcome.shown = outcome
        .listed
        .iter()
        .map(|&at| (at, groups[at].items.len()))
        .collect();
    outcome.shown_pool_tasks = if outcome
        .listed
        .iter()
        .any(|&at| groups[at].kind == GroupKind::Pool)
    {
        outcome.tasks.pool.len()
    } else {
        0
    };
    outcome.truncated = false;
}

/// The lines that print within [`OUTPUT_CAP_CHARS`] after the first, an
/// item's task cell and the pool's task lines counted; truncated only
/// when a listed item or pool task does not print (an empty group's header
/// past the cap alone is no truncation).
fn cut(outcome: &mut RoadmapOutcome) {
    let unit = outcome.unit.as_deref();
    let mut used = summary(outcome).chars().count() + 1;
    let mut shown = Vec::new();
    let mut shown_pool_tasks = 0;
    'groups: for &at in &outcome.listed {
        let group = &outcome.plan.groups[at];
        let cost = group.header(unit).chars().count() + 1;
        if used + cost > OUTPUT_CAP_CHARS {
            break;
        }
        used += cost;
        let mut items = 0;
        for item in &group.items {
            let cost = outcome.item_line(item, group.kind).chars().count() + 1;
            if used + cost > OUTPUT_CAP_CHARS {
                shown.push((at, items));
                break 'groups;
            }
            used += cost;
            items += 1;
        }
        shown.push((at, items));
        if group.kind == GroupKind::Pool {
            for task in &outcome.tasks.pool {
                let cost = pool_task_line(task).chars().count() + 1;
                if used + cost > OUTPUT_CAP_CHARS {
                    break 'groups;
                }
                used += cost;
                shown_pool_tasks += 1;
            }
        }
    }
    outcome.shown = shown;
    outcome.shown_pool_tasks = shown_pool_tasks;
    outcome.truncated = outcome.omitted() > 0;
}

/// The first line: the whole plan's (core [`Plan::summary`]).
fn summary(outcome: &RoadmapOutcome) -> String {
    outcome.plan.summary(outcome.unit.as_deref())
}

/// ` | task <T> <state>, …`: the open tasks executing an item, by number;
/// nothing when none does.
fn task_cell(tasks: &[PlanTask]) -> String {
    if tasks.is_empty() {
        return String::new();
    }
    let names: Vec<String> = tasks
        .iter()
        .map(|task| format!("{} {}", one_line(&task.id), task.status))
        .collect();
    escape_controls(&format!(" | task {}", names.join(", ")))
}

/// `  - <T> | <state> | <title or -> | <targets or ->`: a pool task's
/// line, its targets as stored.
fn pool_task_line(task: &PlanTask) -> String {
    let title = task
        .title
        .as_deref()
        .map_or_else(|| "-".to_owned(), one_line);
    let targets = if task.targets.is_empty() {
        "-".to_owned()
    } else {
        one_line(&task.targets.join(", "))
    };
    escape_controls(&format!(
        "  - {} | {} | {title} | {targets}",
        one_line(&task.id),
        task.status
    ))
}

/// This repository's task rows for the join (canon `spec-roadmap`,
/// "Position"), read as the queue holds them ([`open_tasks`]): no schema
/// step, nothing created or written, never an exit; with the queue's notes,
/// a git failure naming the root, then one note naming how many open tasks
/// of another repository of the project are not shown.
fn queue_tasks(
    env: &Env,
    project: &ProjectRoot,
    slug: &str,
    git: &GitEnv,
) -> (Vec<PlanTask>, Vec<String>) {
    let read = open_tasks(env, project, slug, git, GitPlace::Root);
    let mut notes = read.notes;
    let rows = read
        .here
        .into_iter()
        .map(|task| PlanTask {
            id: task.id,
            status: task.status,
            title: task.title,
            targets: task.targets,
        })
        .collect();
    if let Some(note) = others_note(read.others.len(), slug, "spec roadmap") {
        notes.push(note);
    }
    (rows, notes)
}

/// The open tasks of a project's queue (`PlanTask::is_open`), read as the
/// queue holds them: those of this repository, those of another, and the
/// queue's notes. `spec roadmap`'s join and `spec impact`'s tasks.
pub(crate) struct OpenTasks {
    /// This repository's, by number.
    pub(crate) here: Vec<Task>,
    /// Another repository's of the project, by number.
    pub(crate) others: Vec<Task>,
    pub(crate) notes: Vec<String>,
}

/// How the queue's notes name a failure: `spec roadmap`'s detailed texts
/// (git's at the project root's path, the queue's own error), or texts
/// that carry no path at all (a read whose answer holds no root and no
/// data directory), leaving the cause to `spec roadmap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitPlace {
    Root,
    Unnamed,
}

/// [`GitPlace::Unnamed`]'s note on a data directory or a queue that cannot
/// be read.
const QUEUE_UNREAD: &str = "the queue cannot be read (spec roadmap names the cause)";

/// [`GitPlace::Unnamed`]'s note on git that does not run or fails.
const GIT_FAILED: &str = "git in the project root failed (spec roadmap names the cause)";

/// The project's open tasks, read as the queue holds them: no schema step,
/// nothing created or written, never an exit.
///
/// - No task row (no database, no queue table, an empty queue) or a
///   project root in no git repository: none, no note.
/// - A queue that cannot be read (a newer build's, a database error),
///   `[ids]` taking the proposal or the task prefix, git that does not
///   run: none, one note `tasks not shown: <why>`; under
///   [`GitPlace::Unnamed`] the queue's and git's `<why>` name no path.
/// - A row that does not decode: one note each, `<row>; not shown`.
pub(crate) fn open_tasks(
    env: &Env,
    project: &ProjectRoot,
    slug: &str,
    git: &GitEnv,
    place: GitPlace,
) -> OpenTasks {
    let none = |notes| OpenTasks {
        here: Vec::new(),
        others: Vec::new(),
        notes,
    };
    let not_shown = |why: &str| {
        let note = escape_controls(&one_line(&format!("tasks not shown: {why}")));
        none(vec![note])
    };
    let unread = |why: &str| match place {
        GitPlace::Root => not_shown(why),
        GitPlace::Unnamed => not_shown(QUEUE_UNREAD),
    };
    let db = match db_path(env, slug) {
        Ok(db) => db,
        Err(error) => return unread(cause(&error)),
    };
    let read = SqliteQueue::open_existing(&db, slug).and_then(|queue| match queue {
        Some(queue) => queue.list_existing_tasks(),
        None => Ok(TaskList::default()),
    });
    let listed = match read {
        Ok(listed) => listed,
        Err(error) => return unread(&error.to_string()),
    };
    if listed.tasks.is_empty() && listed.unreadable.is_empty() {
        return none(Vec::new());
    }
    let common_dir = match repository(project, git, place) {
        Ok(Some(dir)) => dir,
        Ok(None) => return none(Vec::new()),
        Err(why) => return not_shown(&why),
    };
    let scheme = &project.config.scheme;
    if let Some(clash) = prefix_clash(scheme).or_else(|| task_prefix_clash(scheme)) {
        return not_shown(&format!("{}: {clash}", project.config_label));
    }
    let notes: Vec<String> = listed
        .unreadable
        .iter()
        .map(|row| escape_controls(&one_line(&format!("{row}; not shown"))))
        .collect();
    let mut here = Vec::new();
    let mut others = Vec::new();
    for task in listed.tasks {
        if !is_open(task.status) {
            continue;
        }
        if same_repository(&task.git_common_dir, &common_dir) {
            here.push(task);
        } else {
            others.push(task);
        }
    }
    OpenTasks {
        here,
        others,
        notes,
    }
}

/// Not `done`, `accepted` or `cancelled` (core's `PlanTask::is_open`).
fn is_open(status: TaskStatus) -> bool {
    PlanTask {
        id: String::new(),
        status,
        title: None,
        targets: Vec::new(),
    }
    .is_open()
}

/// `<n> task(s) of another repository of the project `<slug>` not shown:
/// `<command>` shows the current repository's`; none for none.
pub(crate) fn others_note(others: usize, slug: &str, command: &str) -> Option<String> {
    (others > 0).then(|| {
        escape_controls(&one_line(&format!(
            "{others} task(s) of another repository of the project `{slug}` not shown: \
             `{command}` shows the current repository's"
        )))
    })
}

/// The canonical git common dir of the project root's repository; `None`
/// when git finds no repository there; `Err` the note's reason when git
/// does not run or fails otherwise: git's error at the root's path, or
/// under [`GitPlace::Unnamed`] a text with no path.
fn repository(
    project: &ProjectRoot,
    git: &GitEnv,
    place: GitPlace,
) -> Result<Option<PathBuf>, String> {
    let failed = |error: &dyn std::fmt::Display| match place {
        GitPlace::Root => format!("git in {}: {error}", project.root.display()),
        GitPlace::Unnamed => GIT_FAILED.to_owned(),
    };
    let git = WorktreeGit::new(&project.root, git).map_err(|error| failed(&error))?;
    if git
        .top_if_repository()
        .map_err(|error| failed(&error))?
        .is_none()
    {
        return Ok(None);
    }
    git.common_dir().map(Some).map_err(|error| failed(&error))
}

/// A refusal's message without its `spec: ` prefix.
fn cause(error: &CliError) -> &str {
    error
        .message
        .strip_prefix("spec: ")
        .unwrap_or(&error.message)
}

pub(crate) fn render_text(outcome: &RoadmapOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let unit = outcome.unit.as_deref();
    let mut out = summary(outcome);
    out.push('\n');
    for &(at, items) in &outcome.shown {
        let group = &outcome.plan.groups[at];
        out.push_str(&group.header(unit));
        out.push('\n');
        for item in &group.items[..items.min(group.items.len())] {
            out.push_str(&outcome.item_line(item, group.kind));
            out.push('\n');
        }
        if group.kind == GroupKind::Pool {
            for task in outcome.printed_pool_tasks() {
                out.push_str(&pool_task_line(task));
                out.push('\n');
            }
        }
    }
    if outcome.truncated {
        out.push_str(&format!(
            "[truncated: {} of {} items not shown; add --release R or --pool]\n",
            outcome.omitted(),
            outcome.requested()
        ));
    }
    out
}

#[derive(Serialize)]
struct RoadmapJson<'a> {
    release: Option<&'a str>,
    pool: bool,
    reason: Option<&'a str>,
    prefix: &'a str,
    dir: &'a str,
    unit: Option<&'a str>,
    current: Option<&'a str>,
    counts: CountsJson,
    cut: Option<CutJson<'a>>,
    total: SumJson,
    groups: Vec<GroupJson<'a>>,
    pool_tasks: Vec<PoolTaskJson<'a>>,
    notes: &'a [String],
    truncated: bool,
    omitted: usize,
}

#[derive(Serialize)]
struct CountsJson {
    open: usize,
    done: usize,
}

#[derive(Serialize)]
struct CutJson<'a> {
    release: &'a str,
    min: u64,
    max: u64,
    unestimated: u64,
}

#[derive(Serialize)]
struct SumJson {
    min: u64,
    max: u64,
    unestimated: u64,
}

impl From<Sum> for SumJson {
    fn from(sum: Sum) -> Self {
        Self {
            min: sum.min,
            max: sum.max,
            unestimated: sum.unestimated,
        }
    }
}

#[derive(Serialize)]
struct GroupJson<'a> {
    group: &'static str,
    name: Option<&'a str>,
    current: bool,
    mvp_cut: bool,
    open: usize,
    done: usize,
    sum: SumJson,
    items: Vec<ItemJson<'a>>,
}

#[derive(Serialize)]
struct ItemJson<'a> {
    position: Option<usize>,
    id: &'a str,
    title: Option<&'a str>,
    path: &'a str,
    status: Option<&'a str>,
    release: Option<&'a str>,
    priority: Option<&'a str>,
    estimate: Option<EstimateJson>,
    waits_on: &'a [String],
    cycle: bool,
    tasks: Vec<ItemTaskJson<'a>>,
}

#[derive(Serialize)]
struct EstimateJson {
    min: u64,
    max: u64,
}

/// An open task executing an item.
#[derive(Serialize)]
struct ItemTaskJson<'a> {
    id: &'a str,
    status: TaskStatus,
}

/// An open task executing no open item.
#[derive(Serialize)]
struct PoolTaskJson<'a> {
    id: &'a str,
    status: TaskStatus,
    title: Option<&'a str>,
    targets: &'a [String],
}

fn view(outcome: &RoadmapOutcome) -> RoadmapJson<'_> {
    let plan = &outcome.plan;
    let groups = if outcome.reason.is_some() {
        Vec::new()
    } else {
        outcome
            .shown
            .iter()
            .map(|&(at, items)| {
                let group = &plan.groups[at];
                GroupJson {
                    group: group.kind.as_str(),
                    name: group.name.as_deref(),
                    current: group.current,
                    mvp_cut: group.mvp_cut,
                    open: group.open,
                    done: group.done,
                    sum: group.sum.into(),
                    items: group.items[..items.min(group.items.len())]
                        .iter()
                        .map(|item| ItemJson {
                            position: item.position,
                            id: &item.id,
                            title: item.title.as_deref(),
                            path: &item.path,
                            status: item.status.as_deref(),
                            release: item.release.as_deref(),
                            priority: item.priority.as_deref(),
                            estimate: item.estimate.map(|estimate| EstimateJson {
                                min: estimate.min,
                                max: estimate.max,
                            }),
                            waits_on: &item.waits_on,
                            cycle: item.cycle,
                            tasks: outcome
                                .tasks
                                .of(&item.id)
                                .iter()
                                .map(|task| ItemTaskJson {
                                    id: &task.id,
                                    status: task.status,
                                })
                                .collect(),
                        })
                        .collect(),
                }
            })
            .collect()
    };
    RoadmapJson {
        release: outcome.release.as_deref(),
        pool: outcome.pool,
        reason: outcome.reason.as_deref(),
        prefix: &outcome.prefix,
        dir: &outcome.dir,
        unit: outcome.unit.as_deref(),
        current: plan.current.as_deref(),
        counts: CountsJson {
            open: plan.open,
            done: plan.done,
        },
        cut: plan.cut.as_ref().map(|cut| CutJson {
            release: &cut.release,
            min: cut.sum.min,
            max: cut.sum.max,
            unestimated: cut.sum.unestimated,
        }),
        total: plan.total.into(),
        groups,
        pool_tasks: outcome
            .printed_pool_tasks()
            .iter()
            .map(|task| PoolTaskJson {
                id: &task.id,
                status: task.status,
                title: task.title.as_deref(),
                targets: &task.targets,
            })
            .collect(),
        notes: &plan.notes,
        truncated: outcome.truncated,
        omitted: outcome.omitted(),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for RoadmapOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
