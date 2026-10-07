//! `spec task …` (canon `tasks`, "Commands", "Transitions", "Place"): a
//! task is a record of the project's queue that only the owner moves to
//! `ready`, on a terminal; an agent reads its package, plans it, claims it
//! in a worktree, reports its run and completes it. Nothing is ever
//! blocked by a discrepancy: a stale task is told, never refused.
//!
//! - `new --nodes REF… [--title T] [--goal T] [A]` → `created T-0001`
//!   (`draft`); the nodes resolved as the intake's `node_ids`, stored
//!   canonical, each once.
//! - `show T | --next` → the brief, `--json` the package (crate
//!   `package`); `list [--status S]…` → a line per task by number.
//! - Owner only, a terminal's `[y/N]` (`main` refuses a stdin that is no
//!   terminal before anything is read): `approve T` (the snapshot frozen
//!   from disk in the caller's worktree), `changes T --note T`, `cancel T`.
//! - Agent, no terminal: `plan T`, `claim T --role R --worktree DIR`,
//!   `report T --outcome O --summary S [--changed FILE]…`, `complete T` →
//!   `<verb> T-0001: <status>` (+ ` (run <n>)`); JSON `{id, status, run,
//!   notes}`.
//!
//! Exits as the queue's: 0 done; 1 refused (an unknown task, a transition
//! or run that does not allow it, a cap, a declined prompt, a lost
//! compare-and-set); 2 cannot run here (`T` in `[ids]`, a look-alike ID,
//! another repository's task, no terminal, a corrupt row, an unbound
//! place). The store writes the queue only; nothing under a root.

use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_core::intake::{AuthorInput, author_problem};
use specengine_core::proposal::{Author, author_field_problem};
use specengine_core::task::{
    AFFECTED_MAX, CHANGED_FILE_MAX, CHANGED_FILES_MAX, CRITERIA_MAX, CRITERION_MAX, GOAL_MAX,
    NODES_MAX, NOTE_MAX, PLAN_MAX, RUN_SUMMARY_MAX, SNAPSHOT_MAX, TITLE_MAX, TaskAction,
    TaskIdError, control_problem, needs_message, over_bytes, over_items, parse_task_id,
    run_outcome, task_look_alike_message, task_prefix_clash, transition,
};
use specengine_model::{SnapshotPlace, TaskClaim, TaskCriterion, TaskPackage, TaskStatus, grammar};
use specengine_store::{
    GitEnv, NewTask, QueueError, SnapshotEntry, StoredNote, Task, TaskChange, TaskSnapshot,
    WorktreeGit, claimed_elsewhere, same_repository,
};

use crate::apply::Consent;
use crate::corpus::indexed;
use crate::package::{NodeNow, Roots, assemble, node_now, render_brief, staleness};
use crate::proposals::{QueueContext, checked_now, escaped_error, open_context, queue_cannot};
use crate::propose::{ProposedText, is_path_target, read_text, resolved_node};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The repeated criteria a plan's note names, the rest counted.
const REPEATS_NAMED: usize = 10;

/// `spec task new` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNewRequest {
    /// `--nodes REF…` as given: 1 to [`NODES_MAX`].
    pub nodes: Vec<String>,
    pub title: Option<String>,
    pub goal: Option<String>,
    /// `--author-role`, `--author-model`, `--run`: any given → an agent.
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    pub now: String,
    pub git: GitEnv,
}

/// `spec task show T | --next` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskShowRequest {
    /// `T` as given; `None` with `next`.
    pub id: Option<String>,
    /// `--next`: the lowest-numbered `ready` task of this repository.
    pub next: bool,
    pub git: GitEnv,
}

/// `spec task list [--status S]…` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListRequest {
    /// Only these states; empty: all.
    pub statuses: Vec<TaskStatus>,
    pub git: GitEnv,
}

/// `spec task approve T`, `changes T --note N`, `cancel T`: the owner's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDecisionRequest {
    pub id: String,
    /// `changes`' `--note`.
    pub note: Option<String>,
    pub now: String,
    pub git: GitEnv,
}

/// `spec task plan T --plan-file F|- [--criterion C]… [--affected REF]…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPlanRequest {
    pub id: String,
    /// The plan, Markdown: at most [`PLAN_MAX`] bytes.
    pub plan: ProposedText,
    /// Each a reference (one ID, `slug/ID` or `.md` path) or free text.
    pub criteria: Vec<String>,
    /// `--affected REF…`: resolved as `--nodes`.
    pub affected: Vec<String>,
    pub now: String,
    pub git: GitEnv,
}

/// `spec task claim T --role R --worktree DIR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskClaimRequest {
    pub id: String,
    /// The project's own role name, the author grammar.
    pub role: String,
    /// A worktree of the task's repository, relative to the current
    /// directory.
    pub worktree: PathBuf,
    pub now: String,
    pub git: GitEnv,
}

/// `spec task report T --outcome O --summary S [--changed FILE]…`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReportRequest {
    pub id: String,
    pub outcome: String,
    pub summary: String,
    pub changed_files: Vec<String>,
    pub now: String,
    pub git: GitEnv,
}

/// `spec task complete T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCompleteRequest {
    pub id: String,
    pub now: String,
    pub git: GitEnv,
}

/// What a task-changing command answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOutcome {
    pub action: TaskAction,
    /// The task's ID; `null` when what was given is no task ID.
    pub id: Option<String>,
    /// Its state after the command (as read, when refused); `null` when
    /// there is no such task.
    pub status: Option<TaskStatus>,
    /// The run the command opened or closed.
    pub run: Option<u64>,
    /// What a reader should know; a refusal's reason last.
    pub notes: Vec<String>,
    /// Why the command was refused (exit 1).
    pub refusal: Option<String>,
    pub messages: Vec<Message>,
}

impl TaskOutcome {
    /// Exit 0, or 1 when refused.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    fn refused(
        action: TaskAction,
        id: Option<&str>,
        status: Option<TaskStatus>,
        reason: &str,
        messages: Vec<Message>,
    ) -> Self {
        let reason = one_line(reason);
        let mut notes: Vec<String> = messages
            .iter()
            .filter_map(|message| match message {
                Message::Note(note) => Some(one_line(note)),
                Message::Warning(_) => None,
            })
            .collect();
        notes.push(reason.clone());
        Self {
            action,
            id: id.map(str::to_owned),
            status,
            run: None,
            notes,
            refusal: Some(reason),
            messages,
        }
    }

    fn done(action: TaskAction, task: &Task, run: Option<u64>, messages: Vec<Message>) -> Self {
        let notes = messages
            .iter()
            .filter_map(|message| match message {
                Message::Note(note) => Some(one_line(note)),
                Message::Warning(_) => None,
            })
            .collect();
        Self {
            action,
            id: Some(task.id.clone()),
            status: Some(task.status),
            run,
            notes,
            refusal: None,
            messages,
        }
    }
}

#[derive(Serialize)]
struct TaskOutcomeJson<'a> {
    id: Option<&'a str>,
    status: Option<TaskStatus>,
    run: Option<u64>,
    notes: &'a [String],
}

/// `{id, status, run, notes}`.
impl Serialize for TaskOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        TaskOutcomeJson {
            id: self.id.as_deref(),
            status: self.status,
            run: self.run,
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// What `spec task show` answered: the package, or why there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskShowOutcome {
    pub package: Option<Box<TaskPackage>>,
    /// The task asked for (`null` for `--next`), when there is no package.
    pub id: Option<String>,
    /// Why there is no package (exit 1).
    pub reason: Option<String>,
    pub messages: Vec<Message>,
}

impl TaskShowOutcome {
    /// Exit 0, or 1 when there is no such task.
    pub fn exit(&self) -> Exit {
        if self.reason.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

#[derive(Serialize)]
struct NoTaskJson<'a> {
    id: Option<&'a str>,
    reason: Option<&'a str>,
}

/// The package, else exactly `{id, reason}`.
impl Serialize for TaskShowOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.package {
            Some(package) => package.serialize(serializer),
            None => NoTaskJson {
                id: self.id.as_deref(),
                reason: self.reason.as_deref(),
            }
            .serialize(serializer),
        }
    }
}

/// One line of `spec task list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaskListEntry {
    pub id: String,
    pub status: TaskStatus,
    pub title: Option<String>,
    pub targets: Vec<String>,
    /// As `spec task show` tells it.
    pub stale: Option<bool>,
    pub updated_at: String,
}

/// What `spec task list` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskListOutcome {
    pub tasks: Vec<TaskListEntry>,
    pub notes: Vec<String>,
    pub messages: Vec<Message>,
}

#[derive(Serialize)]
struct TaskListJson<'a> {
    tasks: &'a [TaskListEntry],
    notes: &'a [String],
}

/// `{tasks: [{id, status, title, targets, stale, updated_at}], notes}`.
impl Serialize for TaskListOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        TaskListJson {
            tasks: &self.tasks,
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

// ------------------------------------------------------------- context

/// The queue's context for a task command: as every queue command's, and
/// `T` not taken by the project's `[ids]` (exit 2).
fn open_tasks(env: &Env, globals: &Globals, git: &GitEnv) -> Result<QueueContext, CliError> {
    let context = open_context(env, globals, git)?;
    if let Some(clash) = task_prefix_clash(&context.project.config.scheme) {
        return Err(CliError::spec(format!(
            "{}: {clash}",
            context.project.config_label
        )));
    }
    Ok(context)
}

/// The task ID as written: `Some` when it is one, `None` when it is none
/// in any script (exit 1); a look-alike exits 2 naming the Latin form.
pub(crate) fn written_task(written: &str) -> Result<Option<String>, CliError> {
    match parse_task_id(written) {
        Ok(id) => Ok(Some(id)),
        Err(TaskIdError::LookAlike { fix }) => Err(CliError::spec(task_look_alike_message(
            written.trim(),
            &fix,
        ))),
        Err(TaskIdError::NotAnId) => Ok(None),
    }
}

/// `no task T-0099 in this repository`.
pub(crate) fn no_task(id: &str) -> String {
    format!("no task {id} in this repository")
}

/// Exit 1 for what is no task ID.
fn not_a_task(written: &str) -> String {
    format!(
        "no task `{}`: a task ID is `T-` and 4 or more digits, as `spec task list` lists it",
        written.trim()
    )
}

/// The task `id` of the current repository: `Ok(Err(reason))` when the
/// queue has none (exit 1); another repository's exits 2, naming it.
pub(crate) fn find_task(
    context: &QueueContext,
    id: &str,
) -> Result<Result<Task, String>, CliError> {
    let task = match context.queue.get_task(id) {
        Ok(Some(task)) => task,
        Ok(None) => return Ok(Err(no_task(id))),
        Err(error) => return Err(queue_cannot(error)),
    };
    if !same_repository(&task.git_common_dir, &context.common_dir) {
        return Err(CliError::spec(format!(
            "`{id}` belongs to another repository of the project `{}`: {}; this one is {}: run \
             the command there",
            context.slug,
            task.git_common_dir,
            context.common_dir.display()
        )));
    }
    Ok(Ok(task))
}

/// The task a command names: its context and the task, or the refused
/// outcome (exit 1).
fn named_task(
    env: &Env,
    globals: &Globals,
    git: &GitEnv,
    written: &str,
    action: TaskAction,
) -> Result<Result<(QueueContext, Task), TaskOutcome>, CliError> {
    let id = written_task(written)?;
    let context = open_tasks(env, globals, git)?;
    let Some(id) = id else {
        return Ok(Err(TaskOutcome::refused(
            action,
            None,
            None,
            &not_a_task(written),
            Vec::new(),
        )));
    };
    match find_task(&context, &id)? {
        Ok(task) => Ok(Ok((context, task))),
        Err(reason) => Ok(Err(TaskOutcome::refused(
            action,
            Some(&id),
            None,
            &reason,
            Vec::new(),
        ))),
    }
}

/// The transition refused before any work: its needs message.
fn transition_refusal(task: &Task, action: TaskAction) -> Option<String> {
    transition(Some(task.status), action)
        .err()
        .map(|needs| needs_message(&task.id, task.status, action, needs))
}

/// A store refusal (exit 1) as the outcome; anything else exit 2.
fn store_refusal(
    error: QueueError,
    action: TaskAction,
    task: &Task,
    messages: Vec<Message>,
) -> Result<TaskOutcome, CliError> {
    match error {
        QueueError::TaskRefused { reason, .. } => Ok(TaskOutcome::refused(
            action,
            Some(&task.id),
            Some(task.status),
            &reason,
            messages,
        )),
        other => Err(queue_cannot(other)),
    }
}

/// The nodes `written` names over `input`, canonical, each once (as the
/// intake's `node_ids`): `Ok(Err)` the refusal naming `field[i]`.
fn resolved_list(
    context: &QueueContext,
    input: &specengine_core::check::CheckInput,
    written: &[String],
    field: &str,
) -> Result<Result<Vec<String>, String>, CliError> {
    let mut ids: Vec<String> = Vec::with_capacity(written.len());
    for (index, reference) in written.iter().enumerate() {
        let at = format!("{field}[{index}]");
        let (id, _) = match resolved_node(&context.project, input, reference)
            .map_err(|error| prefixed(error, &at))?
        {
            Ok(found) => found,
            Err(reason) => return Ok(Err(format!("{at}: {reason}"))),
        };
        if let Some(first) = ids.iter().position(|known| *known == id) {
            return Ok(Err(format!(
                "{at}: `{}` names `{id}`, as {field}[{first}] does: name each node once",
                reference.trim()
            )));
        }
        ids.push(id);
    }
    Ok(Ok(ids))
}

/// An exit 2 of a field's reference, its line prefixed by the field.
fn prefixed(error: CliError, field: &str) -> CliError {
    match error.message.strip_prefix("spec: ") {
        Some(rest) => CliError::spec(format!("{field}: {rest}")),
        None => error,
    }
}

// ---------------------------------------------------------------- new

/// `spec task new`: stores a `draft` task of the current repository.
pub fn task_new(
    env: &Env,
    globals: &Globals,
    request: &TaskNewRequest,
) -> Result<TaskOutcome, CliError> {
    run_new(env, globals, request).map_err(escaped_error)
}

fn run_new(
    env: &Env,
    globals: &Globals,
    request: &TaskNewRequest,
) -> Result<TaskOutcome, CliError> {
    const ACTION: TaskAction = TaskAction::New;
    let now = checked_now(&request.now)?;
    if let Some(problem) = author_problem(AuthorInput {
        role: request.author_role.as_deref(),
        model: request.author_model.as_deref(),
        run: request.run.as_deref(),
    }) {
        return Err(CliError::spec(problem));
    }
    let author = Author::new(
        request.author_role.clone(),
        request.author_model.clone(),
        request.run.clone(),
    )
    .map_err(CliError::spec)?;
    let mut context = open_tasks(env, globals, &request.git)?;
    let refuse = |reason: String, messages: Vec<Message>| {
        Ok(TaskOutcome::refused(ACTION, None, None, &reason, messages))
    };
    let caps = [
        request
            .nodes
            .is_empty()
            .then(|| "nodes: name at least one node".to_owned()),
        over_items("nodes", request.nodes.len(), NODES_MAX),
        request
            .title
            .as_deref()
            .and_then(|title| over_bytes("title", title, TITLE_MAX)),
        request
            .goal
            .as_deref()
            .and_then(|goal| over_bytes("goal", goal, GOAL_MAX)),
    ];
    if let Some(problem) = caps.into_iter().flatten().next() {
        return refuse(problem, Vec::new());
    }
    let mut messages = Vec::new();
    let input = indexed(env, &context.project, &mut messages, false)?;
    let targets = match resolved_list(&context, &input, &request.nodes, "nodes")? {
        Ok(targets) => targets,
        Err(reason) => return refuse(reason, messages),
    };
    let created = context
        .queue
        .create_task(
            &NewTask {
                git_common_dir: context.common_dir.display().to_string(),
                title: request.title.clone(),
                goal: request.goal.clone(),
                targets,
                author,
            },
            now,
        )
        .map_err(queue_cannot)?;
    Ok(TaskOutcome::done(ACTION, &created, None, messages))
}

// --------------------------------------------------------------- show

/// `spec task show T | --next`: the package of a task of this repository.
pub fn task_show(
    env: &Env,
    globals: &Globals,
    request: &TaskShowRequest,
) -> Result<TaskShowOutcome, CliError> {
    run_show(env, globals, request).map_err(escaped_error)
}

fn run_show(
    env: &Env,
    globals: &Globals,
    request: &TaskShowRequest,
) -> Result<TaskShowOutcome, CliError> {
    let written = match (&request.id, request.next) {
        (Some(id), false) => Some(written_task(id)?.ok_or(id)),
        (None, true) => None,
        _ => {
            return Err(CliError::spec("give exactly one of a task ID and --next"));
        }
    };
    let context = open_tasks(env, globals, &request.git)?;
    let missing = |id: Option<String>, reason: String| TaskShowOutcome {
        package: None,
        id,
        reason: Some(reason),
        messages: Vec::new(),
    };
    let task = match written {
        Some(Err(written)) => return Ok(missing(None, not_a_task(written))),
        Some(Ok(id)) => match find_task(&context, &id)? {
            Ok(task) => task,
            Err(reason) => return Ok(missing(Some(id), reason)),
        },
        None => {
            let listed = context.queue.list_tasks(None).map_err(queue_cannot)?;
            let next = listed.tasks.into_iter().find(|task| {
                task.status == TaskStatus::Ready
                    && same_repository(&task.git_common_dir, &context.common_dir)
            });
            match next {
                Some(task) => task,
                None => {
                    return Ok(missing(None, "no ready task in this repository".to_owned()));
                }
            }
        }
    };
    let mut messages = Vec::new();
    let mut roots = Roots::default();
    let package = assemble(
        env,
        globals,
        &context,
        &task,
        &request.git,
        &mut roots,
        &mut messages,
    )?;
    messages.extend(package.notes.iter().cloned().map(Message::Note));
    Ok(TaskShowOutcome {
        package: Some(Box::new(package)),
        id: None,
        reason: None,
        messages,
    })
}

// --------------------------------------------------------------- list

/// `spec task list`: the current repository's tasks by number.
pub fn task_list(
    env: &Env,
    globals: &Globals,
    request: &TaskListRequest,
) -> Result<TaskListOutcome, CliError> {
    run_list(env, globals, request).map_err(escaped_error)
}

fn run_list(
    env: &Env,
    globals: &Globals,
    request: &TaskListRequest,
) -> Result<TaskListOutcome, CliError> {
    let context = open_tasks(env, globals, &request.git)?;
    let listed = context.queue.list_tasks(None).map_err(queue_cannot)?;
    let mut notes: Vec<String> = listed
        .unreadable
        .iter()
        .map(|row| one_line(&format!("{row}; not listed")))
        .collect();
    let (ours, others): (Vec<Task>, Vec<Task>) = listed
        .tasks
        .into_iter()
        .partition(|task| same_repository(&task.git_common_dir, &context.common_dir));
    let mut messages = Vec::new();
    let mut roots = Roots::default();
    let mut tasks = Vec::new();
    for task in ours {
        if !request.statuses.is_empty() && !request.statuses.contains(&task.status) {
            continue;
        }
        let compared = staleness(
            env,
            &context,
            &task,
            &request.git,
            &mut roots,
            &mut messages,
        )?;
        if compared.stale.is_none() {
            for note in &compared.notes {
                notes.push(one_line(&format!("{}: {note}", task.id)));
            }
        }
        tasks.push(TaskListEntry {
            id: task.id.clone(),
            status: task.status,
            title: task.title.clone(),
            targets: task.targets.clone(),
            stale: compared.stale,
            updated_at: task.updated_at.clone(),
        });
    }
    if !others.is_empty() {
        notes.push(format!(
            "{} task(s) of another repository of the project `{}` not listed: `spec task list` \
             lists the current repository's",
            others.len(),
            context.slug
        ));
    }
    messages.extend(notes.iter().cloned().map(Message::Note));
    Ok(TaskListOutcome {
        tasks,
        notes,
        messages,
    })
}

// ------------------------------------------------------------- owner

/// `spec task approve T`: the snapshot frozen from disk in the caller's
/// worktree (targets, criteria references, affected nodes), after the
/// owner's consent; `draft`, `review`, `changes_requested` or `ready`
/// (frozen again) → `ready`.
pub fn task_approve(
    env: &Env,
    globals: &Globals,
    request: &TaskDecisionRequest,
    consent: Consent<'_>,
) -> Result<TaskOutcome, CliError> {
    run_approve(env, globals, request, consent).map_err(escaped_error)
}

fn run_approve(
    env: &Env,
    globals: &Globals,
    request: &TaskDecisionRequest,
    consent: Consent<'_>,
) -> Result<TaskOutcome, CliError> {
    const ACTION: TaskAction = TaskAction::Approve;
    let now = checked_now(&request.now)?;
    let (mut context, task) = match named_task(env, globals, &request.git, &request.id, ACTION)? {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let refuse = |reason: &str, messages: Vec<Message>| {
        Ok(TaskOutcome::refused(
            ACTION,
            Some(&task.id),
            Some(task.status),
            reason,
            messages,
        ))
    };
    if let Some(reason) = transition_refusal(&task, ACTION) {
        return refuse(&reason, Vec::new());
    }
    let place = context.git.place(&context.project.root).map_err(|error| {
        CliError::spec(format!(
            "the project root {} cannot be the snapshot's place: {error}",
            context.project.root.display()
        ))
    })?;
    let by = context.git.committer_ident().map_err(|error| {
        CliError::spec(format!(
            "no git identity in {}: {error}; set user.name and user.email",
            place.worktree
        ))
    })?;
    let mut messages = Vec::new();
    let input = indexed(env, &context.project, &mut messages, false)?;
    // The snapshot's nodes: targets, criteria references, affected nodes,
    // each once, in that order.
    let mut wanted: Vec<(String, String)> = Vec::new();
    let references = task
        .criteria
        .iter()
        .enumerate()
        .filter_map(|(index, criterion)| {
            criterion
                .reference
                .as_ref()
                .map(|reference| (format!("criteria[{index}]"), reference.clone()))
        });
    for (field, id) in task
        .targets
        .iter()
        .enumerate()
        .map(|(index, id)| (format!("targets[{index}]"), id.clone()))
        .chain(references)
        .chain(
            task.affected_nodes
                .iter()
                .enumerate()
                .map(|(index, id)| (format!("affected_nodes[{index}]"), id.clone())),
        )
    {
        if !wanted.iter().any(|(_, known)| *known == id) {
            wanted.push((field, id));
        }
    }
    if let Some(problem) = over_items("snapshot", wanted.len(), SNAPSHOT_MAX) {
        return refuse(&format!("{problem} (nodes)"), messages);
    }
    let mut nodes = Vec::with_capacity(wanted.len());
    for (field, id) in &wanted {
        let node: NodeNow = match node_now(&context.project, &input, id)? {
            Ok(node) => node,
            Err(reason) => {
                return refuse(
                    &format!("{field}: {reason}; nothing can be frozen of it"),
                    messages,
                );
            }
        };
        nodes.push(SnapshotEntry {
            id: id.clone(),
            path: node.path,
            span_hash: node.span_hash,
            text: node.text,
        });
    }
    let question = format!(
        "approve {} ({}), freezing {} node(s) of {} on {} at {}? [y/N]",
        task.id,
        task.title.as_deref().unwrap_or("-"),
        nodes.len(),
        place.worktree,
        place.branch,
        place.base_commit
    );
    if !consent(&escape_controls(&one_line(&question))) {
        return refuse(
            &format!(
                "`{}` not approved: the answer was not `y`; nothing changed",
                task.id
            ),
            messages,
        );
    }
    let snapshot = TaskSnapshot {
        at: now.to_owned(),
        by,
        place: SnapshotPlace {
            worktree: place.worktree.clone(),
            root_rel: place.root_rel.clone(),
            branch: place.branch.clone(),
            commit: place.base_commit.clone(),
        },
        nodes,
    };
    match context.queue.change_task(
        &task.id,
        &task.seen(),
        &TaskChange::Approve { snapshot },
        now,
    ) {
        Ok((stored, run)) => Ok(TaskOutcome::done(ACTION, &stored, run, messages)),
        Err(error) => store_refusal(error, ACTION, &task, messages),
    }
}

/// `spec task changes T --note N`: `review` → `changes_requested`, the
/// owner's note kept (oldest first), after the owner's consent.
pub fn task_changes(
    env: &Env,
    globals: &Globals,
    request: &TaskDecisionRequest,
    consent: Consent<'_>,
) -> Result<TaskOutcome, CliError> {
    run_owner(env, globals, request, consent, TaskAction::Changes).map_err(escaped_error)
}

/// `spec task cancel T`: any state before `done` → `cancelled`, after the
/// owner's consent; its proposals stay as they are.
pub fn task_cancel(
    env: &Env,
    globals: &Globals,
    request: &TaskDecisionRequest,
    consent: Consent<'_>,
) -> Result<TaskOutcome, CliError> {
    run_owner(env, globals, request, consent, TaskAction::Cancel).map_err(escaped_error)
}

fn run_owner(
    env: &Env,
    globals: &Globals,
    request: &TaskDecisionRequest,
    consent: Consent<'_>,
    action: TaskAction,
) -> Result<TaskOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let (mut context, task) = match named_task(env, globals, &request.git, &request.id, action)? {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let refuse = |reason: &str| {
        Ok(TaskOutcome::refused(
            action,
            Some(&task.id),
            Some(task.status),
            reason,
            Vec::new(),
        ))
    };
    if let Some(reason) = transition_refusal(&task, action) {
        return refuse(&reason);
    }
    let (change, question, declined) = if action == TaskAction::Changes {
        let note = request.note.clone().unwrap_or_default();
        if note.trim().is_empty() {
            return refuse("note: blank; say what the plan must change");
        }
        if let Some(problem) = over_bytes("note", &note, NOTE_MAX) {
            return refuse(&problem);
        }
        let by = context.git.committer_ident().map_err(|error| {
            CliError::spec(format!(
                "no git identity in {}: {error}; set user.name and user.email",
                context.project.root.display()
            ))
        })?;
        (
            TaskChange::Changes {
                note: StoredNote {
                    at: now.to_owned(),
                    note,
                    by,
                },
            },
            format!(
                "request changes of {} ({})? [y/N]",
                task.id,
                task.title.as_deref().unwrap_or("-")
            ),
            "changes not requested",
        )
    } else {
        (
            TaskChange::Cancel,
            format!(
                "cancel {} ({}, {})? [y/N]",
                task.id,
                task.status,
                task.title.as_deref().unwrap_or("-")
            ),
            "not cancelled",
        )
    };
    if !consent(&escape_controls(&one_line(&question))) {
        return refuse(&format!(
            "`{}` {declined}: the answer was not `y`; nothing changed",
            task.id
        ));
    }
    match context
        .queue
        .change_task(&task.id, &task.seen(), &change, now)
    {
        Ok((stored, run)) => Ok(TaskOutcome::done(action, &stored, run, Vec::new())),
        Err(error) => store_refusal(error, action, &task, Vec::new()),
    }
}

// -------------------------------------------------------------- agent

/// `spec task plan T`: `draft` or `changes_requested` → `review`, the
/// plan, criteria and affected nodes replaced.
pub fn task_plan(
    env: &Env,
    globals: &Globals,
    request: &TaskPlanRequest,
) -> Result<TaskOutcome, CliError> {
    run_plan(env, globals, request).map_err(escaped_error)
}

fn run_plan(
    env: &Env,
    globals: &Globals,
    request: &TaskPlanRequest,
) -> Result<TaskOutcome, CliError> {
    const ACTION: TaskAction = TaskAction::Plan;
    let now = checked_now(&request.now)?;
    let (mut context, task) = match named_task(env, globals, &request.git, &request.id, ACTION)? {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let refuse = |reason: &str, messages: Vec<Message>| {
        Ok(TaskOutcome::refused(
            ACTION,
            Some(&task.id),
            Some(task.status),
            reason,
            messages,
        ))
    };
    let plan = match read_text(env, &request.plan)? {
        Ok(plan) => plan,
        Err(reason) => return refuse(&format!("plan_md: {reason}"), Vec::new()),
    };
    let mut caps = vec![
        over_bytes("plan_md", &plan, PLAN_MAX),
        over_items("criteria", request.criteria.len(), CRITERIA_MAX),
    ];
    caps.extend(
        request
            .criteria
            .iter()
            .enumerate()
            .map(|(index, criterion)| {
                let field = format!("criteria[{index}]");
                if criterion.trim().is_empty() {
                    Some(format!("{field}: blank"))
                } else {
                    over_bytes(&field, criterion, CRITERION_MAX)
                }
            }),
    );
    caps.push(over_items(
        "affected_nodes",
        request.affected.len(),
        AFFECTED_MAX,
    ));
    if let Some(problem) = caps.into_iter().flatten().next() {
        return refuse(&problem, Vec::new());
    }
    if let Some(reason) = transition_refusal(&task, ACTION) {
        return refuse(&reason, Vec::new());
    }
    let mut messages = Vec::new();
    let input = indexed(env, &context.project, &mut messages, false)?;
    let scheme = &context.project.config.scheme;
    // A criterion is a reference when it is exactly one (an ID, `slug/ID`,
    // a root-relative `.md` path), else free text, verbatim.
    let mut criteria = Vec::with_capacity(request.criteria.len());
    for (index, criterion) in request.criteria.iter().enumerate() {
        let written = criterion.trim();
        let reference =
            is_path_target(written) || grammar::parse_reference(written, 0, scheme).is_some();
        if !reference {
            criteria.push(TaskCriterion {
                reference: None,
                text: Some(criterion.clone()),
            });
            continue;
        }
        let field = format!("criteria[{index}]");
        match resolved_node(&context.project, &input, written)
            .map_err(|error| prefixed(error, &field))?
        {
            Ok((id, _)) => criteria.push(TaskCriterion {
                reference: Some(id),
                text: None,
            }),
            Err(reason) => return refuse(&format!("{field}: {reason}"), messages),
        }
    }
    // A criterion given twice (the same reference, canonical, or the same
    // text) is kept once, where it first stands.
    // Each kept criterion with its index in the plan as given: a repeat
    // names where its criterion first stands there.
    let mut kept: Vec<(usize, TaskCriterion)> = Vec::with_capacity(criteria.len());
    let mut repeats = Vec::new();
    for (index, criterion) in criteria.into_iter().enumerate() {
        match kept.iter().find(|(_, earlier)| *earlier == criterion) {
            Some((first, _)) => repeats.push(format!("criteria[{index}] (as criteria[{first}])")),
            None => kept.push((index, criterion)),
        }
    }
    let kept: Vec<TaskCriterion> = kept.into_iter().map(|(_, criterion)| criterion).collect();
    if !repeats.is_empty() {
        let mut named = repeats
            .iter()
            .take(REPEATS_NAMED)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if repeats.len() > REPEATS_NAMED {
            named.push_str(&format!(" … and {} more", repeats.len() - REPEATS_NAMED));
        }
        messages.push(Message::Note(format!(
            "criteria: {} repeat(s) kept once: {named}",
            repeats.len()
        )));
    }
    let criteria = kept;
    let affected = match resolved_list(&context, &input, &request.affected, "affected_nodes")? {
        Ok(affected) => affected,
        Err(reason) => return refuse(&reason, messages),
    };
    let change = TaskChange::Plan {
        plan,
        criteria,
        affected_nodes: affected,
    };
    match context
        .queue
        .change_task(&task.id, &task.seen(), &change, now)
    {
        Ok((stored, run)) => Ok(TaskOutcome::done(ACTION, &stored, run, messages)),
        Err(error) => store_refusal(error, ACTION, &task, messages),
    }
}

/// `spec task claim T --role R --worktree DIR`: `ready` → `in_progress`,
/// a run opened, the claim recording a canonical worktree of the task's
/// repository (`git worktree list`) and its branch.
pub fn task_claim(
    env: &Env,
    globals: &Globals,
    request: &TaskClaimRequest,
) -> Result<TaskOutcome, CliError> {
    run_claim(env, globals, request).map_err(escaped_error)
}

fn run_claim(
    env: &Env,
    globals: &Globals,
    request: &TaskClaimRequest,
) -> Result<TaskOutcome, CliError> {
    const ACTION: TaskAction = TaskAction::Claim;
    let now = checked_now(&request.now)?;
    let (mut context, task) = match named_task(env, globals, &request.git, &request.id, ACTION)? {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let refuse = |reason: &str| {
        Ok(TaskOutcome::refused(
            ACTION,
            Some(&task.id),
            Some(task.status),
            reason,
            Vec::new(),
        ))
    };
    let worktree_text = request.worktree.to_string_lossy();
    let caps = [
        author_field_problem(&request.role).map(|problem| format!("role: {problem}")),
        control_problem("worktree", &worktree_text),
    ];
    if let Some(problem) = caps.into_iter().flatten().next() {
        return refuse(&problem);
    }
    if let Some(reason) = transition_refusal(&task, ACTION) {
        return refuse(&reason);
    }
    let (worktree, branch) =
        match claimed_place(&context, &task, &request.worktree, &request.git, env)? {
            Ok(place) => place,
            Err(reason) => return refuse(&reason),
        };
    let author = Author::new(Some(request.role.clone()), None, None).map_err(CliError::spec)?;
    let change = TaskChange::Claim {
        claim: TaskClaim {
            at: now.to_owned(),
            role: request.role.clone(),
            worktree,
            branch,
        },
        author,
    };
    match context
        .queue
        .change_task(&task.id, &task.seen(), &change, now)
    {
        Ok((stored, run)) => Ok(TaskOutcome::done(ACTION, &stored, run, Vec::new())),
        Err(error) => store_refusal(error, ACTION, &task, Vec::new()),
    }
}

/// The claim's place: `dir` (current-directory relative) canonical, one of
/// the task's repository's worktrees (`git worktree list`, never a bare
/// entry), on a branch (git only reads); else why not (exit 1).
fn claimed_place(
    context: &QueueContext,
    task: &Task,
    dir: &Path,
    git_env: &GitEnv,
    env: &Env,
) -> Result<Result<(String, String), String>, CliError> {
    let given = env.cwd.join(dir);
    let Ok(canonical) = std::fs::canonicalize(&given) else {
        return Ok(Err(format!(
            "worktree: {} does not exist: a claim names a worktree of the task's repository",
            dir.display()
        )));
    };
    let listed = context.git.worktrees().map_err(|error| {
        CliError::spec(format!(
            "cannot list the worktrees of {}: {error}",
            task.git_common_dir
        ))
    })?;
    if !listed
        .iter()
        .any(|worktree| !worktree.bare && worktree.path == canonical)
    {
        return Ok(Err(format!(
            "worktree: {} is no worktree of the task's repository ({}): `git worktree list` \
             names them",
            canonical.display(),
            task.git_common_dir
        )));
    }
    let git = WorktreeGit::new(&canonical, git_env)
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    let common = git.common_dir().map_err(|error| {
        CliError::spec(format!(
            "cannot read the repository of {}: {error}",
            canonical.display()
        ))
    })?;
    if !same_repository(&task.git_common_dir, &common) {
        return Ok(Err(format!(
            "worktree: {} belongs to the repository {}, not the task's ({})",
            canonical.display(),
            common.display(),
            task.git_common_dir
        )));
    }
    let branch = git.branch().map_err(|error| {
        CliError::spec(format!(
            "cannot read the branch of {}: {error}",
            canonical.display()
        ))
    })?;
    let Some(branch) = branch else {
        return Ok(Err(format!(
            "worktree: {} has a detached HEAD: a claim records the branch it works on; check \
             out a branch there first",
            canonical.display()
        )));
    };
    let Some(worktree) = canonical.to_str() else {
        return Ok(Err(format!(
            "worktree: {} is no UTF-8 path",
            canonical.display()
        )));
    };
    Ok(Ok((worktree.to_owned(), branch)))
}

/// `spec task report T`: the open run closed with its outcome, summary
/// and changed files; the state kept.
pub fn task_report(
    env: &Env,
    globals: &Globals,
    request: &TaskReportRequest,
) -> Result<TaskOutcome, CliError> {
    run_report(env, globals, request).map_err(escaped_error)
}

fn run_report(
    env: &Env,
    globals: &Globals,
    request: &TaskReportRequest,
) -> Result<TaskOutcome, CliError> {
    const ACTION: TaskAction = TaskAction::Report;
    let now = checked_now(&request.now)?;
    let (mut context, task) = match named_task(env, globals, &request.git, &request.id, ACTION)? {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let refuse = |reason: &str| {
        Ok(TaskOutcome::refused(
            ACTION,
            Some(&task.id),
            Some(task.status),
            reason,
            Vec::new(),
        ))
    };
    let outcome = match run_outcome(request.outcome.trim()) {
        Ok(outcome) => outcome,
        Err(problem) => return refuse(&problem),
    };
    let mut caps = vec![
        request
            .summary
            .trim()
            .is_empty()
            .then(|| "summary: blank".to_owned()),
        over_bytes("summary", &request.summary, RUN_SUMMARY_MAX),
        over_items(
            "changed_files",
            request.changed_files.len(),
            CHANGED_FILES_MAX,
        ),
    ];
    for (index, file) in request.changed_files.iter().enumerate() {
        let field = format!("changed_files[{index}]");
        caps.push(over_bytes(&field, file, CHANGED_FILE_MAX));
        caps.push(control_problem(&field, file));
    }
    if let Some(problem) = caps.into_iter().flatten().next() {
        return refuse(&problem);
    }
    if let Some(reason) = transition_refusal(&task, ACTION) {
        return refuse(&reason);
    }
    if let Some(reason) = claimed_elsewhere_refusal(&context, &task, ACTION)? {
        return refuse(&reason);
    }
    let change = TaskChange::Report {
        outcome,
        summary: request.summary.clone(),
        changed_files: request.changed_files.clone(),
    };
    match context
        .queue
        .change_task(&task.id, &task.seen(), &change, now)
    {
        Ok((stored, run)) => Ok(TaskOutcome::done(ACTION, &stored, run, Vec::new())),
        Err(error) => store_refusal(error, ACTION, &task, Vec::new()),
    }
}

/// `spec task complete T`: `in_progress`, its run closed → `done`.
pub fn task_complete(
    env: &Env,
    globals: &Globals,
    request: &TaskCompleteRequest,
) -> Result<TaskOutcome, CliError> {
    run_complete(env, globals, request).map_err(escaped_error)
}

fn run_complete(
    env: &Env,
    globals: &Globals,
    request: &TaskCompleteRequest,
) -> Result<TaskOutcome, CliError> {
    const ACTION: TaskAction = TaskAction::Complete;
    let now = checked_now(&request.now)?;
    let (mut context, task) = match named_task(env, globals, &request.git, &request.id, ACTION)? {
        Ok(found) => found,
        Err(refused) => return Ok(refused),
    };
    let refused = match transition_refusal(&task, ACTION) {
        Some(reason) => Some(reason),
        None => claimed_elsewhere_refusal(&context, &task, ACTION)?,
    };
    if let Some(reason) = refused {
        return Ok(TaskOutcome::refused(
            ACTION,
            Some(&task.id),
            Some(task.status),
            &reason,
            Vec::new(),
        ));
    }
    match context
        .queue
        .change_task(&task.id, &task.seen(), &TaskChange::Complete, now)
    {
        Ok((stored, run)) => Ok(TaskOutcome::done(ACTION, &stored, run, Vec::new())),
        Err(error) => store_refusal(error, ACTION, &task, Vec::new()),
    }
}

/// `report` and `complete` run in the claimed worktree only, compared as
/// directories with the worktree of the project root (the binding rule's
/// place check, canon `tasks`, "Place"): elsewhere, the refusal naming the
/// claimed place.
fn claimed_elsewhere_refusal(
    context: &QueueContext,
    task: &Task,
    action: TaskAction,
) -> Result<Option<String>, CliError> {
    if task.claim.is_none() {
        return Ok(None);
    }
    let here = context.git.top().map_err(|error| {
        CliError::spec(format!(
            "cannot read the worktree of the project root {}: {error}",
            context.project.root.display()
        ))
    })?;
    Ok(claimed_elsewhere(task, &here).map(|claim| {
        format!(
            "`{}` is claimed in the worktree {} on `{}`: `{}` runs there, not in {}; nothing \
             recorded",
            task.id,
            claim.worktree,
            claim.branch,
            action.as_str(),
            here.display()
        )
    }))
}

// ----------------------------------------------------- bound proposals

/// A proposal's `--task T` before anything is checked: the ID taken
/// (a look-alike exits 2, `T` in `[ids]` exits 2), the task of this
/// repository read and the place checked as the store will check it in
/// the inserting transaction: `Ok(Err(reason))` refuses (exit 1).
pub(crate) fn bound_task(
    context: &QueueContext,
    written: &str,
    place: &specengine_store::Place,
) -> Result<Result<String, String>, CliError> {
    if let Some(clash) = task_prefix_clash(&context.project.config.scheme) {
        return Err(CliError::spec(format!(
            "{}: {clash}",
            context.project.config_label
        )));
    }
    let Some(id) = written_task(written)? else {
        return Ok(Err(format!("--task: {}", not_a_task(written))));
    };
    let task = match context.queue.get_task(&id) {
        Ok(Some(task)) => task,
        Ok(None) => return Ok(Err(format!("--task: {}", no_task(&id)))),
        Err(error) => return Err(queue_cannot(error)),
    };
    if let Some(reason) = specengine_store::binding_problem(&task, place) {
        return Ok(Err(format!("--task: {reason}")));
    }
    Ok(Ok(id))
}

// -------------------------------------------------------------- text

/// The past tense a command's line opens with.
const fn verb(action: TaskAction) -> &'static str {
    match action {
        TaskAction::New => "created",
        TaskAction::Plan => "planned",
        TaskAction::Approve => "approved",
        TaskAction::Changes => "returned",
        TaskAction::Claim => "claimed",
        TaskAction::Report => "reported",
        TaskAction::Complete => "completed",
        TaskAction::Cancel => "cancelled",
    }
}

/// `created T-0001`; `<verb> T-0001: <status>` (+ ` (run <n>)`); nothing
/// when refused.
pub(crate) fn render_text(outcome: &TaskOutcome) -> String {
    if outcome.refusal.is_some() {
        return String::new();
    }
    let id = outcome.id.as_deref().unwrap_or_default();
    let mut line = match (outcome.action, outcome.status) {
        (TaskAction::New, _) | (_, None) => format!("{} {id}", verb(outcome.action)),
        (action, Some(status)) => format!("{} {id}: {status}", verb(action)),
    };
    if let Some(run) = outcome.run {
        line.push_str(&format!(" (run {run})"));
    }
    line.push('\n');
    escape_controls(&line)
}

/// The brief, or nothing when there is no package.
pub(crate) fn render_show_text(outcome: &TaskShowOutcome) -> String {
    outcome
        .package
        .as_deref()
        .map(render_brief)
        .unwrap_or_default()
}

/// `<id> | <status> | <title or -> | <targets> | <updated_at>[ | stale]`.
pub(crate) fn render_list_text(outcome: &TaskListOutcome) -> String {
    let mut out = String::new();
    for task in &outcome.tasks {
        let mut line = format!(
            "{} | {} | {} | {} | {}",
            task.id,
            task.status,
            task.title
                .as_deref()
                .map_or_else(|| "-".to_owned(), one_line),
            task.targets.join(", "),
            task.updated_at
        );
        if task.stale == Some(true) {
            line.push_str(" | stale");
        }
        out.push_str(&line);
        out.push('\n');
    }
    escape_controls(&out)
}
