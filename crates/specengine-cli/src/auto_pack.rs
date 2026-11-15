//! Packs (task spec `auto-land` "Packs", "`spec auto start`", "`spec auto
//! hold`"): a pack is its `auto.started` event, named by its `seq`; its
//! items are its tasks in the order given, then its lines `<pack>/<n>`. A
//! line is bound by the first `auto.bound` naming it.
//!
//! - **Finished**: an item whose task has an `auto.landed`, whose last
//!   `auto.*` event is `auto.held`, or whose task is `cancelled`; an unbound
//!   line, when its last `auto.*` event is `auto.held`.
//! - **Live**: a pack with an unfinished item, whatever its `until`: past
//!   it the pack stays live for its lands, and `until` holds its approves
//!   and implements (the predicate's `until`). A task belongs to a pack when
//!   listed in it or bound to one of its lines, and is in at most one live
//!   pack (judged again inside the transaction recording a start).
//!
//! [`auto_start`] asks the owner once, on a terminal, and records the pack;
//! [`auto_hold`] records the runner's hold of an item. Both write nothing
//! under a switch.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use specengine_core::auto::{
    Act, PACK_LINE_MAX, PackFacts, Reason, Surface, judge_task_paths, pack_lines, parse_pack_line,
};
use specengine_core::auto_toml::AutoRule;
use specengine_core::check::CheckInput;
use specengine_core::proposal::is_utc_timestamp;
use specengine_model::TaskStatus;
use specengine_store::{
    AutoBind, AutoEvent, AutoHeld, AutoLanded, AutoStarted, GitEnv, NewTask, StoredAuto, Task,
    WorktreeGit,
};

use crate::auto::{AutoOutcome, json_text};
use crate::auto_read::{EffectiveRule, load_auto, task_facts};
use crate::corpus::indexed;
use crate::proposals::{QueueContext, checked_now, escaped_error, queue_cannot};
use crate::task::{find_task, not_a_task, open_tasks, written_task};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The longest `--lines` line, in bytes (core's).
pub const LINE_MAX: usize = PACK_LINE_MAX;
/// The longest `spec auto hold --note`, in bytes.
pub const HOLD_NOTE_MAX: usize = 1024;

/// One pack, its items resolved from its events.
#[derive(Debug, Clone)]
pub(crate) struct Pack {
    pub seq: i64,
    pub at: String,
    pub started: AutoStarted,
    /// The rule its start froze, rebuilt; `Err` why it does not read.
    pub frozen: Result<AutoRule, String>,
    pub items: Vec<Item>,
}

/// One item of a pack: a task listed at start, or a line (bound or not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    /// The task: listed, or bound to the line.
    pub task: Option<String>,
    /// The line's number, from 1.
    pub line: Option<u32>,
    /// The line's text.
    pub text: Option<String>,
    /// Its last `auto.landed`.
    pub landed: Option<AutoLanded>,
    /// Its last `auto.*` event, when that is an `auto.held`.
    pub held: Option<AutoHeld>,
}

impl Item {
    /// `T-0042`, or `<pack>/<n>` for an unbound line.
    pub fn name(&self, pack: i64) -> String {
        match (&self.task, self.line) {
            (Some(task), _) => task.clone(),
            (None, Some(line)) => format!("{pack}/{line}"),
            (None, None) => String::new(),
        }
    }

    /// Finished (see the module documentation), its task's state read in
    /// `tasks`.
    pub fn finished(&self, tasks: &BTreeMap<String, Task>) -> bool {
        if self.landed.is_some() || self.held.is_some() {
            return true;
        }
        self.task
            .as_ref()
            .and_then(|task| tasks.get(task))
            .is_some_and(|task| task.status == TaskStatus::Cancelled)
    }
}

impl Pack {
    /// Live (see the module documentation): an item unfinished, its task's
    /// state read in `tasks`; `until` aside.
    pub fn live(&self, tasks: &BTreeMap<String, Task>) -> bool {
        self.items.iter().any(|item| !item.finished(tasks))
    }

    /// Its `--until` is not after `now`: only its lands still run.
    pub fn ended(&self, now: &str) -> bool {
        self.started
            .until
            .as_deref()
            .is_some_and(|until| until <= now)
    }

    /// The pack's facts for the predicate; `Err` why its frozen rule does
    /// not read.
    pub fn facts(&self) -> Result<PackFacts<'_>, String> {
        match &self.frozen {
            Ok(frozen) => Ok(PackFacts {
                seq: self.seq,
                until: self.started.until.as_deref(),
                frozen,
            }),
            Err(why) => Err(why.clone()),
        }
    }

    /// The item of `task`, listed or bound.
    pub fn item_of(&self, task: &str) -> Option<&Item> {
        self.items
            .iter()
            .find(|item| item.task.as_deref() == Some(task))
    }

    /// The line item `line`.
    pub fn line(&self, line: u32) -> Option<&Item> {
        self.items.iter().find(|item| item.line == Some(line))
    }
}

/// The packs of a repository's `auto.*` events, by `seq`.
pub(crate) fn packs_of(events: &[StoredAuto]) -> Vec<Pack> {
    let mut packs: Vec<Pack> = Vec::new();
    for stored in events {
        match &stored.event {
            AutoEvent::Started(started) => {
                let mut items: Vec<Item> = started
                    .tasks
                    .iter()
                    .map(|task| Item {
                        task: Some(task.clone()),
                        line: None,
                        text: None,
                        landed: None,
                        held: None,
                    })
                    .collect();
                items.extend(started.lines.iter().enumerate().map(|(at, text)| Item {
                    task: None,
                    line: Some(u32::try_from(at + 1).unwrap_or(u32::MAX)),
                    text: Some(text.clone()),
                    landed: None,
                    held: None,
                }));
                packs.push(Pack {
                    seq: stored.seq,
                    at: stored.at.clone(),
                    frozen: AutoRule::from_canonical(&started.frozen),
                    started: started.clone(),
                    items,
                });
            }
            AutoEvent::Bound(bound) => {
                if let Some(item) = pack_mut(&mut packs, bound.pack).and_then(|pack| {
                    pack.items
                        .iter_mut()
                        .find(|item| item.line == Some(bound.line))
                }) && item.task.is_none()
                {
                    item.task = Some(bound.task.clone());
                    item.held = None;
                }
            }
            AutoEvent::Held(held) => {
                if let Some(pack) = held.pack.and_then(|seq| pack_mut(&mut packs, seq)) {
                    let by_task = held.task.as_deref().and_then(|task| {
                        pack.items
                            .iter()
                            .position(|item| item.task.as_deref() == Some(task))
                    });
                    let by_line = || {
                        held.line.and_then(|line| {
                            pack.items.iter().position(|item| item.line == Some(line))
                        })
                    };
                    if let Some(at) = by_task.or_else(by_line) {
                        pack.items[at].held = Some(held.clone());
                    }
                }
            }
            AutoEvent::Landed(landed) => {
                if let Some(item) = pack_mut(&mut packs, landed.pack).and_then(|pack| {
                    pack.items
                        .iter_mut()
                        .find(|item| item.task.as_deref() == Some(landed.task.as_str()))
                }) {
                    item.landed = Some(landed.clone());
                    item.held = None;
                }
            }
        }
    }
    packs
}

fn pack_mut(packs: &mut [Pack], seq: i64) -> Option<&mut Pack> {
    packs.iter_mut().find(|pack| pack.seq == seq)
}

/// The pack `task` belongs to: the live one, else (`any`) the latest
/// holding it, with its item.
pub(crate) fn pack_of<'p>(
    packs: &'p [Pack],
    task: &str,
    tasks: &BTreeMap<String, Task>,
    any: bool,
) -> Option<(&'p Pack, &'p Item)> {
    let holding = || {
        packs
            .iter()
            .filter_map(move |pack| pack.item_of(task).map(|item| (pack, item)))
    };
    holding()
        .find(|(pack, _)| pack.live(tasks))
        .or_else(|| if any { holding().next_back() } else { None })
}

/// What every `spec auto` act reads in one snapshot: the queue, the rule,
/// the repository's packs and tasks.
pub(crate) struct AutoState {
    pub context: QueueContext,
    pub rule: EffectiveRule,
    pub packs: Vec<Pack>,
    /// The repository's tasks, by ID.
    pub tasks: BTreeMap<String, Task>,
}

impl AutoState {
    /// The repository's canonical git common dir, as stored.
    pub fn repository(&self) -> String {
        self.context.common_dir.display().to_string()
    }
}

/// Reads [`AutoState`] for `surface`: exit 2 when it cannot run.
pub(crate) fn auto_state(
    env: &Env,
    globals: &Globals,
    git: &GitEnv,
    surface: Surface,
) -> Result<AutoState, CliError> {
    let context = open_tasks(env, globals, git)?;
    let rule = load_auto(env, &context.project, git, surface)?;
    let repository = context.common_dir.display().to_string();
    let events = context
        .queue
        .auto_events(&repository)
        .map_err(queue_cannot)?;
    let listed = context
        .queue
        .list_tasks(Some(&repository))
        .map_err(queue_cannot)?;
    let tasks = listed
        .tasks
        .into_iter()
        .map(|task| (task.id.clone(), task))
        .collect();
    Ok(AutoState {
        context,
        rule,
        packs: packs_of(&events),
        tasks,
    })
}

// ------------------------------------------------------------- start

/// `spec auto start T... [--lines FILE] [--until TIME]` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoStartRequest {
    /// The tasks as written.
    pub tasks: Vec<String>,
    /// `--lines FILE`: a path (never `-`).
    pub lines: Option<PathBuf>,
    /// `--until TIME`.
    pub until: Option<String>,
    pub now: String,
    pub git: GitEnv,
}

/// The listing `spec auto start` shows before its question, and the
/// question; `main` writes both on stderr and reads one answer.
pub type StartConsent<'a> = &'a mut dyn FnMut(&str, &str) -> bool;

#[derive(Serialize)]
struct StartedJson<'a> {
    pack: i64,
    tasks: &'a [String],
    lines: &'a [String],
    branch: &'a str,
    branch_at: &'a str,
    until: Option<&'a str>,
    rule: &'a str,
    notes: &'a [String],
}

/// `spec auto start` (task spec `auto-land` "`spec auto start`"): `main`
/// has refused a stdin that is no terminal. Exit 1 with nothing written for
/// a refusal, a switch or a declined answer; exit 2 for what cannot run.
pub fn auto_start(
    env: &Env,
    globals: &Globals,
    request: &AutoStartRequest,
    consent: StartConsent<'_>,
) -> Result<AutoOutcome, CliError> {
    run_start(env, globals, request, consent).map_err(escaped_error)
}

fn run_start(
    env: &Env,
    globals: &Globals,
    request: &AutoStartRequest,
    consent: StartConsent<'_>,
) -> Result<AutoOutcome, CliError> {
    let now = checked_now(&request.now)?;
    if request.tasks.is_empty() && request.lines.is_none() {
        return Err(CliError::spec(
            "name at least one task or give --lines FILE; nothing started",
        ));
    }
    // Each task ID as written: a look-alike exits 2, as `tasks.md` reads.
    let mut ids: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for written in &request.tasks {
        let Some(id) = written_task(written)? else {
            // Not a task ID in any script: exit 1, as `tasks.md` "Exits".
            return Ok(AutoOutcome::refused(format!(
                "{}; nothing started",
                not_a_task(written)
            )));
        };
        if ids.contains(&id) {
            notes.push(format!("{id} is named twice: kept once"));
        } else {
            ids.push(id);
        }
    }
    if let Some(until) = &request.until
        && let Some(why) = until_problem(until, now)
    {
        return Err(CliError::spec(format!("--until: {why}")));
    }
    let lines = match &request.lines {
        None => Vec::new(),
        Some(path) => match read_lines(path)? {
            Ok(lines) => lines,
            Err(reason) => return Ok(AutoOutcome::refused(reason)),
        },
    };
    let state = auto_state(env, globals, &request.git, Surface::Local)?;
    let mut tasks: Vec<Task> = Vec::with_capacity(ids.len());
    for id in &ids {
        let task = match find_task(&state.context, id)? {
            Ok(task) => task,
            Err(reason) => return Ok(AutoOutcome::refused(format!("{reason}; nothing started"))),
        };
        if !matches!(
            task.status,
            TaskStatus::Draft
                | TaskStatus::Review
                | TaskStatus::ChangesRequested
                | TaskStatus::Ready
        ) {
            return Ok(AutoOutcome::refused(format!(
                "{id} is {}: a pack takes draft, review, changes_requested or ready; nothing \
                 started",
                task.status
            )));
        }
        if let Some((pack, _)) = pack_of(&state.packs, id, &state.tasks, false) {
            return Ok(AutoOutcome::refused(format!(
                "{id} is in live pack {}; nothing started",
                pack.seq
            )));
        }
        tasks.push(task);
    }
    let rule = &state.rule;
    let Some(now_rule) = rule.rule.as_ref().filter(|_| rule.switch().is_none()) else {
        let line = rule.off_line().unwrap_or_else(|| "auto is off".to_owned());
        return Ok(AutoOutcome::refused(line));
    };
    let items = tasks.len() + lines.len();
    let max = now_rule.max_tasks;
    if items == 0 || items > max as usize {
        return Ok(AutoOutcome::refused(format!(
            "the pack holds {items} items; the rule's max_tasks is {max}; nothing started"
        )));
    }
    // The owner's identity in the main worktree, as `spec task approve`.
    let main = WorktreeGit::new(&rule.main_worktree, &request.git)
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    let by = main.committer_ident().map_err(|error| {
        CliError::spec(format!(
            "no git identity in {}: {error}; set user.name and user.email",
            rule.main_worktree.display()
        ))
    })?;
    let head = rule.head.clone().unwrap_or_default();
    let branch = now_rule.branch.clone();
    let existing = main
        .commit_of(&format!("refs/heads/{branch}"))
        .map_err(|error| CliError::spec(format!("cannot read `{branch}`: {error}")))?;
    let mut messages: Vec<Message> = Vec::new();
    let input = indexed(env, &state.context.project, &mut messages, false)?;
    let listing = start_listing(
        &state,
        &input,
        &tasks,
        &lines,
        now_rule,
        existing.as_deref(),
        &head,
        request.until.as_deref(),
    )?;
    let mut shown = listing.join("\n");
    shown.push('\n');
    // The read's notes first (a working tree differing from `HEAD`).
    for note in rule
        .notes
        .iter()
        .cloned()
        .chain(rule.gate_note())
        .chain(checked_out_note(&main, &branch))
    {
        shown.push_str(&format!("note: {}\n", one_line(&note)));
    }
    let question = format!(
        "start this pack: its tasks approved and landed on `{branch}` by the rule, with no \
         further key? [y/N]"
    );
    if !consent(&escape_controls(&shown), &escape_controls(&question)) {
        return Ok(AutoOutcome::refused(
            "not started: the answer was not `y`; nothing changed".to_owned(),
        ));
    }
    let made_here = existing.is_none();
    let branch_at = match existing {
        Some(at) => at,
        None => {
            let made = main.run(
                &[
                    "branch".as_ref(),
                    "--end-of-options".as_ref(),
                    branch.as_ref(),
                    head.as_ref(),
                ],
                false,
            );
            match made {
                Ok(output) if output.status.success() => head.clone(),
                Ok(output) => {
                    return Err(CliError::spec(format!(
                        "cannot make the branch `{branch}` at {head}: {}; nothing started",
                        String::from_utf8_lossy(&output.stderr).trim()
                    )));
                }
                Err(error) => {
                    return Err(CliError::spec(format!(
                        "cannot make the branch `{branch}` at {head}: {error}; nothing started"
                    )));
                }
            }
        }
    };
    let started = AutoStarted {
        repository: state.repository(),
        tasks: ids.clone(),
        lines: lines.clone(),
        until: request.until.clone(),
        branch: branch.clone(),
        branch_at: branch_at.clone(),
        rule: now_rule.hash.clone(),
        frozen: now_rule.canonical_form(),
        by,
    };
    // "A task is in at most one live pack", judged again under the write
    // lock: another start may have taken one meanwhile.
    let repository = state.repository();
    let mut context = state.context;
    let recorded = context
        .queue
        .record_auto_checked(
            &AutoEvent::Started(started),
            &repository,
            now,
            |events, rows| {
                let packs = packs_of(events);
                let tasks: BTreeMap<String, Task> = rows
                    .iter()
                    .map(|task| (task.id.clone(), task.clone()))
                    .collect();
                ids.iter().find_map(|id| {
                    pack_of(&packs, id, &tasks, false).map(|(pack, _)| {
                        format!("{id} is in live pack {}; nothing started", pack.seq)
                    })
                })
            },
        )
        .map_err(queue_cannot)?;
    let seq = match recorded {
        Ok(seq) => seq,
        Err(why) => {
            // The branch this start made goes too, while still where it was
            // made: nothing started.
            if made_here {
                let _ = main.run(
                    &[
                        "update-ref".as_ref(),
                        "-d".as_ref(),
                        format!("refs/heads/{branch}").as_ref(),
                        branch_at.as_ref(),
                    ],
                    false,
                );
            }
            return Ok(AutoOutcome::refused(why));
        }
    };
    let text = format!(
        "started pack {seq}: {} task(s), {} line(s) on {branch}\n",
        ids.len(),
        lines.len()
    );
    let json = json_text(&StartedJson {
        pack: seq,
        tasks: &ids,
        lines: &lines,
        branch: &branch,
        branch_at: &branch_at,
        until: request.until.as_deref(),
        rule: &now_rule.hash,
        notes: &notes,
    });
    Ok(AutoOutcome {
        exit: Exit::Answered,
        text: escape_controls(&text),
        json,
        stderr: notes
            .iter()
            .map(|note| format!("note: {}", one_line(note)))
            .collect(),
    })
}

/// The listing's lines: the head, each task, each line.
#[allow(clippy::too_many_arguments)]
fn start_listing(
    state: &AutoState,
    input: &CheckInput,
    tasks: &[Task],
    lines: &[String],
    rule: &AutoRule,
    existing: Option<&str>,
    head: &str,
    until: Option<&str>,
) -> Result<Vec<String>, CliError> {
    let short = |sha: &str| sha.chars().take(12).collect::<String>();
    let on = match existing {
        Some(at) => short(at),
        None => format!("new at {}", short(head)),
    };
    let mut out = vec![format!(
        "auto pack: {} task(s), {} line(s); rule {}; on {} ({on}); until {}",
        tasks.len(),
        lines.len(),
        rule.hash,
        rule.branch,
        until.unwrap_or("no end")
    )];
    let switches = state.rule.switches();
    for task in tasks {
        // A hint, never a refusal: the path checks of its targets and
        // affected nodes now, judged as under the frozen rule to be.
        let pack = PackFacts {
            seq: 0,
            until: None,
            frozen: rule,
        };
        let facts = task_facts(&state.context.project, input, task, Some(pack))?;
        let mark = match judge_task_paths(&switches, &facts) {
            specengine_core::auto::Verdict::Apply => "apply".to_owned(),
            specengine_core::auto::Verdict::Hold { reason, detail } => {
                format!("hold ({reason}): {detail}")
            }
        };
        out.push(format!(
            "  {} | {} | {} | {mark}",
            task.id,
            task.status,
            task.title.as_deref().unwrap_or("-")
        ));
    }
    for (at, line) in lines.iter().enumerate() {
        out.push(format!("  line {} | {line}", at + 1));
    }
    Ok(out)
}

/// `` `<b>` is checked out in <w>: a land holds until it is not ``, when it
/// is.
fn checked_out_note(git: &WorktreeGit, branch: &str) -> Option<String> {
    checked_out_in(git, branch).ok().flatten().map(|worktree| {
        format!("`{branch}` is checked out in {worktree}: a land holds until it is not")
    })
}

/// The worktree that has `branch` checked out (`git worktree list
/// --porcelain -z`), when one does.
pub(crate) fn checked_out_in(git: &WorktreeGit, branch: &str) -> Result<Option<String>, String> {
    let output = git
        .run(
            &[
                "worktree".as_ref(),
                "list".as_ref(),
                "--porcelain".as_ref(),
                "-z".as_ref(),
            ],
            true,
        )
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    let wanted = format!("branch refs/heads/{branch}");
    let mut worktree: Option<String> = None;
    for field in output.stdout.split(|&byte| byte == 0) {
        let field = String::from_utf8_lossy(field);
        if let Some(path) = field.strip_prefix("worktree ") {
            worktree = Some(path.to_owned());
        } else if field == wanted {
            return Ok(worktree);
        }
    }
    Ok(None)
}

/// Why `until` is no time after `now`.
fn until_problem(until: &str, now: &str) -> Option<String> {
    if !is_utc_timestamp(until) {
        return Some(format!("`{until}` is no time `YYYY-MM-DDTHH:MM:SSZ`"));
    }
    (until <= now).then(|| format!("`{until}` is not after now ({now})"))
}

/// The kept lines of `path` (see the module documentation); exit 2 when
/// the file cannot be read, `Ok(Err)` the refusal (exit 1).
fn read_lines(path: &PathBuf) -> Result<Result<Vec<String>, String>, CliError> {
    if path.as_os_str() == "-" {
        return Err(CliError::spec(
            "--lines takes a file, never `-`; nothing started",
        ));
    }
    let bytes = fs::read(path).map_err(|error| {
        CliError::spec(format!("--lines: cannot read {}: {error}", path.display()))
    })?;
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(Err("--lines: the file is not UTF-8".to_owned()));
    };
    Ok(pack_lines(&text).map_err(|why| format!("--lines: {why}")))
}

// -------------------------------------------------------- a line's task

/// The variable binding `spec task new` to a pack's line.
pub(crate) const AUTO_LINE_VAR: &str = "SPECENGINE_AUTO_LINE";

/// `SPECENGINE_AUTO_LINE` of the caller's environment, when set and not
/// empty.
pub(crate) fn auto_line(git: &GitEnv) -> Option<String> {
    git.var(AUTO_LINE_VAR)
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty())
}

/// `task` stored as `spec task new` stores it, bound to the line `value`
/// names (task spec `auto-land` "`SPECENGINE_AUTO_LINE` and `spec task
/// new`") when it parses as `<pack>/<n>`, the pack is a live `auto.started`
/// of this repository holding line `n`, the line is unbound and the rule's
/// switches pass: the stored task and `bound to pack <p> line <n>`, else
/// why not (the task stored either way).
pub(crate) fn bind_new_task(
    env: &Env,
    context: &mut QueueContext,
    task: &NewTask,
    value: &str,
    now: &str,
    git: &GitEnv,
) -> Result<(Task, Result<String, String>), CliError> {
    let plain = |context: &mut QueueContext,
                 why: String|
     -> Result<(Task, Result<String, String>), CliError> {
        let created = context.queue.create_task(task, now).map_err(queue_cannot)?;
        Ok((created, Err(why)))
    };
    let Some((pack, line)) = parse_line(value.trim()) else {
        return plain(context, "no <pack>/<line>".to_owned());
    };
    let rule = load_auto(env, &context.project, git, Surface::Local)?;
    if let Some(off) = rule.off_line() {
        return plain(context, off);
    }
    let repository = context.common_dir.display().to_string();
    let events = context
        .queue
        .auto_events(&repository)
        .map_err(queue_cannot)?;
    let tasks: BTreeMap<String, Task> = context
        .queue
        .list_tasks(Some(&repository))
        .map_err(queue_cannot)?
        .tasks
        .into_iter()
        .map(|task| (task.id.clone(), task))
        .collect();
    let live = packs_of(&events)
        .iter()
        .any(|candidate| candidate.seq == pack && candidate.live(&tasks));
    if !live {
        return plain(context, format!("no live pack {pack} here"));
    }
    let (created, bound) = context
        .queue
        .create_task_bound(task, Some(AutoBind { pack, line }), now)
        .map_err(queue_cannot)?;
    let bound = match bound {
        Some(Ok(bound)) => Ok(format!("bound to pack {} line {}", bound.pack, bound.line)),
        Some(Err(why)) => Err(why),
        None => Err(format!("no live pack {pack} here")),
    };
    Ok((created, bound))
}

// -------------------------------------------------------------- hold

/// `spec auto hold ITEM --reason R [--note TEXT]` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoHoldRequest {
    /// A task ID or `<pack>/<n>`.
    pub item: String,
    pub reason: String,
    pub note: Option<String>,
    pub now: String,
    pub git: GitEnv,
}

#[derive(Serialize)]
struct HeldJson<'a> {
    item: &'a str,
    pack: Option<i64>,
    reason: &'a str,
    detail: &'a str,
    notes: &'a [String],
}

/// `spec auto hold` (task spec `auto-land` "`spec auto hold`"): the
/// runner's hold of an item of a pack, not landed: one `auto.held` (`act`
/// `run`), exit 0; in no pack or landed: exit 1, no event; a reason not the
/// runner's or a bad note: exit 2.
pub fn auto_hold(
    env: &Env,
    globals: &Globals,
    request: &AutoHoldRequest,
) -> Result<AutoOutcome, CliError> {
    run_hold(env, globals, request).map_err(escaped_error)
}

fn run_hold(
    env: &Env,
    globals: &Globals,
    request: &AutoHoldRequest,
) -> Result<AutoOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let reason = Reason::parse(&request.reason)
        .filter(|reason| Reason::RUNNER.contains(reason))
        .ok_or_else(|| {
            let names: Vec<&str> = Reason::RUNNER
                .iter()
                .map(|reason| reason.as_str())
                .collect();
            CliError::spec(format!(
                "--reason `{}` is none of {}; nothing held",
                request.reason,
                names.join(", ")
            ))
        })?;
    let note = request
        .note
        .clone()
        .unwrap_or_else(|| reason.as_str().to_owned());
    if note.len() > HOLD_NOTE_MAX {
        return Err(CliError::spec(format!(
            "--note: {} bytes; at most {HOLD_NOTE_MAX}; nothing held",
            note.len()
        )));
    }
    if note.chars().any(char::is_control) {
        return Err(CliError::spec(
            "--note holds a control character; nothing held",
        ));
    }
    let written = request.item.trim();
    let line = parse_line(written);
    let task_id = match line {
        Some(_) => None,
        None => match written_task(written)? {
            Some(id) => Some(id),
            None => {
                return Ok(AutoOutcome::refused(format!(
                    "`{written}` is no task ID and no `<pack>/<n>`; nothing held"
                )));
            }
        },
    };
    let state = auto_state(env, globals, &request.git, Surface::Local)?;
    if let Some(line) = state.rule.off_line() {
        return Ok(AutoOutcome::refused(line));
    }
    let rule_hash = state
        .rule
        .rule
        .as_ref()
        .map(|rule| rule.hash.clone())
        .unwrap_or_default();
    let found: Option<(&Pack, &Item)> = match (line, &task_id) {
        (Some((pack, n)), _) => state
            .packs
            .iter()
            .find(|candidate| candidate.seq == pack)
            .and_then(|pack| pack.line(n).map(|item| (pack, item))),
        (None, Some(id)) => pack_of(&state.packs, id, &state.tasks, true),
        (None, None) => None,
    };
    let Some((pack, item)) = found else {
        return Ok(AutoOutcome::refused(format!(
            "{written} is in no pack; nothing held"
        )));
    };
    if let Some(landed) = &item.landed {
        return Ok(AutoOutcome::refused(format!(
            "{written} is landed: {} on {}; nothing held",
            short(&landed.commit),
            landed.branch
        )));
    }
    let held = AutoHeld {
        pack: Some(pack.seq),
        task: item.task.clone(),
        line: item.line,
        proposal: None,
        act: Act::Run.as_str().to_owned(),
        reason: reason.as_str().to_owned(),
        detail: note.clone(),
        rule: rule_hash,
    };
    let pack_seq = pack.seq;
    let name = item.name(pack_seq);
    let mut context = state.context;
    context
        .queue
        .record_auto(&AutoEvent::Held(held), now)
        .map_err(queue_cannot)?;
    let notes: Vec<String> = Vec::new();
    Ok(AutoOutcome {
        exit: Exit::Answered,
        text: escape_controls(&format!("held {name} ({reason}): {note}\n")),
        json: json_text(&HeldJson {
            item: &name,
            pack: Some(pack_seq),
            reason: reason.as_str(),
            detail: &note,
            notes: &notes,
        }),
        stderr: Vec::new(),
    })
}

/// `<pack>/<n>` as two decimal numbers (core [`parse_pack_line`]).
pub(crate) fn parse_line(written: &str) -> Option<(i64, u32)> {
    parse_pack_line(written)
}

/// The first 12 characters of a commit.
pub(crate) fn short(sha: &str) -> String {
    sha.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use specengine_store::{AutoBound, AutoLanded};

    use super::*;

    fn started(tasks: &[&str], lines: &[&str], until: Option<&str>) -> AutoEvent {
        let text = "[project]\nslug = \"a\"\n[zones]\ncode = [\"src/**\"]\n[auto]\nmode = \"local\"\nzones = [\"code\"]\n";
        let rule = AutoRule::from_toml(text.as_bytes(), "specengine.toml")
            .rule
            .unwrap();
        AutoEvent::Started(AutoStarted {
            repository: "/r".to_owned(),
            tasks: tasks.iter().map(|task| (*task).to_owned()).collect(),
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
            until: until.map(str::to_owned),
            branch: "auto".to_owned(),
            branch_at: "0".repeat(40),
            rule: rule.hash.clone(),
            frozen: rule.canonical_form(),
            by: "O <o@x>".to_owned(),
        })
    }

    fn stored(seq: i64, event: AutoEvent) -> StoredAuto {
        StoredAuto {
            seq,
            at: "2026-11-15T00:00:00Z".to_owned(),
            event,
        }
    }

    #[test]
    fn items_bind_hold_and_land() {
        let events = vec![
            stored(5, started(&["T-0001"], &["make it sortable"], None)),
            stored(
                6,
                AutoEvent::Bound(AutoBound {
                    pack: 5,
                    line: 1,
                    task: "T-0002".to_owned(),
                }),
            ),
            stored(
                7,
                AutoEvent::Held(AutoHeld {
                    pack: Some(5),
                    task: Some("T-0002".to_owned()),
                    line: None,
                    proposal: None,
                    act: "run".to_owned(),
                    reason: "denied-tool".to_owned(),
                    detail: "Bash".to_owned(),
                    rule: String::new(),
                }),
            ),
        ];
        let packs = packs_of(&events);
        assert_eq!(packs.len(), 1);
        let pack = &packs[0];
        assert_eq!(pack.items.len(), 2);
        assert_eq!(pack.items[1].task.as_deref(), Some("T-0002"));
        assert!(pack.items[1].held.is_some());
        let tasks = BTreeMap::new();
        assert!(pack.live(&tasks));
        let mut more = events.clone();
        more.push(stored(
            8,
            AutoEvent::Landed(AutoLanded {
                pack: 5,
                task: "T-0001".to_owned(),
                branch: "auto".to_owned(),
                commit: "a".repeat(40),
                code_commit: None,
                applied: Vec::new(),
                left: Vec::new(),
                rule: String::new(),
            }),
        ));
        let packs = packs_of(&more);
        assert!(!packs[0].live(&tasks));
        assert!(pack_of(&packs, "T-0001", &tasks, false).is_none());
        assert!(pack_of(&packs, "T-0001", &tasks, true).is_some());
    }

    #[test]
    fn until_ends_a_pack() {
        let packs = packs_of(&[stored(
            1,
            started(&["T-0001"], &[], Some("2026-11-15T00:30:00Z")),
        )]);
        let tasks = BTreeMap::new();
        // Live past `until`, for its lands; ended for the rest.
        assert!(packs[0].live(&tasks));
        assert!(!packs[0].ended("2026-11-15T00:00:00Z"));
        assert!(packs[0].ended("2026-11-15T00:30:00Z"));
    }

    #[test]
    fn lines_parse() {
        assert_eq!(parse_line("812/1"), Some((812, 1)));
        assert_eq!(parse_line("812/"), None);
        assert_eq!(parse_line("T-0001"), None);
        assert_eq!(parse_line("a/1"), None);
    }
}
