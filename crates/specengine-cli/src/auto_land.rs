//! `spec auto land T` (task spec `auto-land` "`spec auto land`"): a pack's
//! `done` task landed on the rule's branch, in eleven steps, the first
//! failing one ending it: membership; the task's state and last run; the
//! lock `<data dir>/<slug>.auto-land`; the claim's place; the paths changed
//! since the merge base, judged; a code commit of what is uncommitted; the
//! task's open `update` and `create` rows of this worktree applied by the
//! rule; a `--no-ff` merge in a scratch worktree; `spec check` on the
//! merged tree against the branch's old tip; the branch moved by
//! compare-and-set; `auto.landed`.
//!
//! It never pushes, never moves a ref but the rule's branch and the claim
//! branch, never checks out a branch, never writes `spec.lock`. A hold at
//! steps 2 and 4-10 logs one `auto.held` (`act` `land`); the scratch
//! worktree and the lock are removed in every outcome. Commits made at
//! steps 6-7 stay on the task's branch, so a land run again continues.
//!
//! - **Paths** (step 5) are root-relative; one outside the project root is
//!   written `../…` and lies in no zone. Each changed spec document (a `.md`
//!   the index walks) is read as the merge brings it and as the merge base
//!   held it, and an ID either declares of an excluded kind holds
//!   `excluded-kind` (file-level on purpose: deleting or stripping such
//!   content is held too).
//! - **Nothing new** (step 8): a claim tip already on the rule's branch is
//!   a recovery when a land merge of this task has it as its second parent
//!   (a land that stopped between its `update-ref` and its `auto.landed`):
//!   that merge is recorded and the outcome says so; else a hold `place`,
//!   nothing to land. The branch's old tip is never recorded as a merge.
//! - **Notes**: each row step 7 judged applicable but could not apply
//!   (refused at a step, or moved since it was judged) is one note, with
//!   why, in the outcome, landed or held.
//! - **Signals**: `SIGINT`, `SIGTERM` and `SIGHUP` are held from the lock to
//!   the end; one arriving stops the land before its next step (before the
//!   branch moves), the lock and the scratch worktree removed, nothing
//!   recorded. A lock whose pid is gone is taken over atomically (one land
//!   at a time: `LandLock::take_over`); under the lock, stale `land-*`
//!   scratch entries are removed first.

use std::ffi::OsStr;
use std::fs;
use std::fs::TryLockError;
use std::io::{self, Write as _};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_core::WalkScope;
use specengine_core::auto::{
    DELEGATED_BY_TRAILER, Reason, Surface, Switches, Verdict, judge_land, judge_land_paths,
    judge_proposal, root_relative,
};
use specengine_core::check::CheckInput;
use specengine_core::check::Verdict as CheckVerdict;
use specengine_model::Severity;
use specengine_store::{
    AutoEvent, AutoHeld, AutoLanded, GitEnv, ProposalFilter, ProposalKind, ProposalQueue as _,
    ProposalStatus, Task, WorktreeGit, same_dir, same_repository,
};

use crate::apply::Delegation;
use crate::auto::{AutoOutcome, frozen_off, json_text};
use crate::auto_apply::{DelegatedApproveRequest, DelegatedOutcome, delegated_approve};
use crate::auto_pack::{AutoState, Pack, auto_state, checked_out_in, pack_of, short};
use crate::auto_read::{EffectiveRule, joined, proposal_facts, task_facts};
use crate::check::{CheckRequest, CheckedTree, check};
use crate::corpus::indexed;
use crate::proposals::{checked_now, escaped_error, queue_cannot};
use crate::task::{find_task, not_a_task, written_task};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// Conflicting paths a `merge-conflict` detail names before `and <k> more`.
const CONFLICTS_NAMED: usize = 8;

/// `spec auto land T` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoLandRequest {
    /// `T` as given.
    pub id: String,
    pub now: String,
    pub git: GitEnv,
}

#[derive(Serialize)]
struct LandJson<'a> {
    id: &'a str,
    landed: bool,
    branch: &'a str,
    commit: Option<&'a str>,
    code_commit: Option<&'a str>,
    applied: &'a [String],
    left: &'a [String],
    rule: &'a str,
    reason: Option<&'a str>,
    detail: Option<&'a str>,
    notes: &'a [String],
}

/// The paths a land judges (step 5): `git diff --name-only -z --no-renames
/// <merge base>` in the claim's worktree (committed and uncommitted) and
/// its untracked files, each once, in byte order, root-relative; a path
/// outside the project root written `../…` from it, which no zone covers.
/// `Err`: why they cannot be read (a `place` detail).
pub(crate) fn land_paths(
    rule: &EffectiveRule,
    task: &Task,
    branch: &str,
    git_env: &GitEnv,
) -> Result<Vec<String>, String> {
    land_changes(rule, task, branch, git_env).map(|(_, paths)| paths)
}

/// [`land_paths`] and the merge base they are read against.
fn land_changes(
    rule: &EffectiveRule,
    task: &Task,
    branch: &str,
    git_env: &GitEnv,
) -> Result<(String, Vec<String>), String> {
    let Some(claim) = &task.claim else {
        return Err(format!("`{}` has no claim: no worktree to land", task.id));
    };
    let git = WorktreeGit::new(&claim.worktree, git_env).map_err(|error| error.to_string())?;
    let base = merge_base(&git, &claim.branch, branch)?;
    let diff = git_ok(
        &git,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--end-of-options",
            &base,
        ],
        true,
    )?;
    let untracked = git_ok(
        &git,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        true,
    )?;
    let mut paths: Vec<String> = Vec::new();
    for raw in diff
        .split(|&byte| byte == 0)
        .chain(untracked.split(|&byte| byte == 0))
    {
        if raw.is_empty() {
            continue;
        }
        paths.push(root_relative(&rule.root_rel, &String::from_utf8_lossy(raw)));
    }
    paths.sort();
    paths.dedup();
    Ok((base, paths))
}

/// `git merge-base <a> <b>` (branches as `refs/heads/`).
fn merge_base(git: &WorktreeGit, a: &str, b: &str) -> Result<String, String> {
    let out = git_ok(
        git,
        &[
            "merge-base",
            "--end-of-options",
            &format!("refs/heads/{a}"),
            &format!("refs/heads/{b}"),
        ],
        true,
    )?;
    Ok(String::from_utf8_lossy(&out).trim().to_owned())
}

/// `git <args>` to success: its stdout; else git's words.
fn git_ok(git: &WorktreeGit, args: &[&str], read_only: bool) -> Result<Vec<u8>, String> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let output = git
        .run(&args, read_only)
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let words = String::from_utf8_lossy(&output.stderr);
        let words = words.trim();
        Err(one_line(if words.is_empty() {
            "git failed"
        } else {
            words
        }))
    }
}

/// `git merge-base --is-ancestor <a> <b>`: exit 0 yes, 1 no, else git's
/// words.
fn is_ancestor(git: &WorktreeGit, a: &str, b: &str) -> Result<bool, String> {
    let words = ["merge-base", "--is-ancestor", "--end-of-options", a, b];
    let args: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    let output = git.run(&args, true).map_err(|error| error.to_string())?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(one_line(String::from_utf8_lossy(&output.stderr).trim())),
    }
}

/// `spec auto land` (see the module documentation): exit 0 landed, 1 held
/// or refused, 2 when it cannot run or a signal stopped it.
pub fn auto_land(
    env: &Env,
    globals: &Globals,
    request: &AutoLandRequest,
) -> Result<AutoOutcome, CliError> {
    run_land(env, globals, request).map_err(escaped_error)
}

/// `SIGINT`, `SIGTERM` and `SIGHUP` held: a flag, never the process's end
/// (`crate::group`'s forwarding).
#[cfg(unix)]
struct Signals(crate::group::Forwarding);

#[cfg(unix)]
impl Signals {
    fn hold() -> Self {
        Self(crate::group::Forwarding::hold())
    }

    fn interrupted(&self) -> bool {
        self.0.interrupted()
    }
}

/// No signal is held off Unix.
#[cfg(not(unix))]
struct Signals;

#[cfg(not(unix))]
impl Signals {
    fn hold() -> Self {
        Self
    }

    fn interrupted(&self) -> bool {
        false
    }
}

/// A process holding the lock is alive (off Unix: always taken as alive).
fn lock_holder_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        crate::group::alive(pid)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// What a land carries from step to step.
struct Land<'s> {
    task: &'s Task,
    pack: &'s Pack,
    branch: String,
    rule: String,
    now: &'s str,
    /// The queue's database, for the events a step records.
    db: PathBuf,
    slug: String,
    /// Held from step 3; `None` before it.
    signals: Option<&'s Signals>,
}

impl Land<'_> {
    /// A hold's outcome: exit 1, `<T> held (<reason>): <detail>; <branch>
    /// unmoved`, JSON `landed` false.
    fn held_outcome(&self, reason: Reason, detail: &str, done: &Done<'_>) -> AutoOutcome {
        let line = format!(
            "{} held ({reason}): {detail}; {} unmoved",
            self.task.id, self.branch
        );
        let mut outcome = AutoOutcome::refused_with(
            &line,
            json_text(&LandJson {
                id: &self.task.id,
                landed: false,
                branch: &self.branch,
                commit: None,
                code_commit: done.code_commit,
                applied: done.applied,
                left: done.left,
                rule: &self.rule,
                reason: Some(reason.as_str()),
                detail: Some(detail),
                notes: done.notes,
            }),
        );
        // The notes before the refusal's own line.
        let mut stderr: Vec<String> = done
            .notes
            .iter()
            .map(|note| escape_controls(&Message::Note(note.clone()).line()))
            .collect();
        stderr.append(&mut outcome.stderr);
        outcome.stderr = stderr;
        outcome
    }

    /// Exit 2 when a held signal arrived: the land stops before its next
    /// step, nothing recorded.
    fn check(&self) -> Result<(), CliError> {
        if self.signals.is_some_and(Signals::interrupted) {
            return Err(CliError::spec(format!(
                "interrupted: `{}` not landed, `{}` unmoved; the lock and the scratch worktree \
                 removed, nothing recorded",
                self.task.id, self.branch
            )));
        }
        Ok(())
    }

    /// One event recorded in the project's queue.
    fn record(&self, event: &AutoEvent) -> Result<(), CliError> {
        let mut queue =
            specengine_store::SqliteQueue::open(&self.db, &self.slug).map_err(queue_cannot)?;
        queue.record_auto(event, self.now).map_err(queue_cannot)?;
        Ok(())
    }

    /// One `auto.held` (`act` `land`), then the hold's outcome; a signal
    /// that arrived meanwhile stops the land instead (a step it cut short
    /// is no hold of the task).
    fn hold(&self, reason: Reason, detail: &str, done: &Done<'_>) -> Result<AutoOutcome, CliError> {
        self.check()?;
        self.record(&AutoEvent::Held(AutoHeld {
            pack: Some(self.pack.seq),
            task: Some(self.task.id.clone()),
            line: None,
            proposal: None,
            act: "land".to_owned(),
            reason: reason.as_str().to_owned(),
            detail: detail.to_owned(),
            rule: self.rule.clone(),
        }))?;
        Ok(self.held_outcome(reason, detail, done))
    }
}

/// What a land did before a hold: its rows applied and left, its code
/// commit, and its notes (each row step 7 could not apply, with why).
#[derive(Default)]
struct Done<'a> {
    applied: &'a [String],
    left: &'a [String],
    code_commit: Option<&'a str>,
    notes: &'a [String],
}

fn run_land(
    env: &Env,
    globals: &Globals,
    request: &AutoLandRequest,
) -> Result<AutoOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let written = written_task(&request.id)?;
    let state = auto_state(env, globals, &request.git, Surface::Local)?;
    let Some(id) = written else {
        return Ok(AutoOutcome::refused(not_a_task(&request.id)));
    };
    if let Some(line) = state.rule.off_line() {
        return Ok(AutoOutcome::refused(line));
    }
    let Some(now_rule) = state.rule.rule.clone() else {
        return Ok(AutoOutcome::refused("auto is off".to_owned()));
    };
    let task = match find_task(&state.context, &id)? {
        Ok(task) => task,
        Err(reason) => return Ok(AutoOutcome::refused(format!("{reason}; nothing changed"))),
    };
    // Step 1: membership.
    let Some((pack, item)) = pack_of(&state.packs, &id, &state.tasks, true) else {
        return Ok(AutoOutcome::refused(format!(
            "{id} held (not-in-pack): `{id}` is in no live pack; {} unmoved",
            now_rule.branch
        )));
    };
    if let Some(landed) = &item.landed {
        return Ok(AutoOutcome::refused(format!(
            "{id} is landed: {} on {}; nothing changed",
            short(&landed.commit),
            landed.branch
        )));
    }
    let mut land = Land {
        task: &task,
        pack,
        branch: now_rule.branch.clone(),
        rule: now_rule.hash.clone(),
        now,
        db: state
            .context
            .data_dir
            .join(format!("{}.db", state.context.slug)),
        slug: state.context.slug.clone(),
        signals: None,
    };
    let mut messages: Vec<Message> = Vec::new();
    let input = indexed(env, &state.context.project, &mut messages, false)?;
    let switches = Switches {
        surface: Surface::Local,
        ..state.rule.switches()
    };
    let facts_of_pack = match pack.facts() {
        Ok(facts) => facts,
        Err(why) => {
            let (reason, detail) = match frozen_off(why) {
                Verdict::Hold { reason, detail } => (reason, detail),
                Verdict::Apply => (Reason::Off, String::new()),
            };
            return Ok(AutoOutcome::refused(format!(
                "{id} held ({reason}): {detail}; nothing changed"
            )));
        }
    };
    let facts = task_facts(&state.context.project, &input, &task, Some(facts_of_pack))?;
    // Step 2: the state and the last run (no paths yet).
    match judge_land_paths(&switches, &facts, &[]) {
        Verdict::Apply => {}
        Verdict::Hold { reason, detail } if reason.is_switch() => {
            return Ok(AutoOutcome::refused(format!("auto is off: {detail}")));
        }
        Verdict::Hold {
            reason: reason @ (Reason::Status | Reason::NotInPack),
            detail,
        } => {
            return Ok(AutoOutcome::refused(format!(
                "{id} held ({reason}): {detail}; {} unmoved",
                land.branch
            )));
        }
        Verdict::Hold { reason, detail } => {
            return land.hold(reason, &detail, &Done::default());
        }
    }
    // Steps 3-11 hold the signals: the lock and the scratch worktree are
    // removed whatever stops the land.
    let signals = Signals::hold();
    land.signals = Some(&signals);
    // Step 3: the lock; a lock whose pid is gone is stale.
    let lock_path = state
        .context
        .data_dir
        .join(format!("{}.auto-land", state.context.slug));
    let _lock = match LandLock::take(&lock_path, now) {
        Ok(lock) => lock,
        Err(Taken::Held(text)) => {
            return Ok(AutoOutcome::refused(format!(
                "another land holds {} since {text}; if none runs, remove it; nothing changed",
                lock_path.display()
            )));
        }
        Err(Taken::Cannot(error)) => {
            return Err(CliError::spec(format!(
                "cannot make {}: {error}",
                lock_path.display()
            )));
        }
    };
    let scratch_root = state
        .context
        .data_dir
        .join("auto")
        .join(&state.context.slug);
    fs::create_dir_all(&scratch_root).map_err(|error| {
        CliError::spec(format!("cannot create {}: {error}", scratch_root.display()))
    })?;
    // Under the lock no other land runs: every `land-*` entry is stale.
    sweep(&scratch_root, &state.rule.main_worktree, &request.git);
    land.check()?;
    steps(
        env,
        globals,
        request,
        &state,
        &land,
        &input,
        &switches,
        &scratch_root,
    )
}

/// The stale `land-<pid>` scratch worktrees, their check homes and message
/// files of `scratch_root`, removed (`git worktree remove --force`, then
/// `git worktree prune` in the main worktree); a failure leaves them.
fn sweep(scratch_root: &Path, main: &Path, git_env: &GitEnv) {
    let Ok(entries) = fs::read_dir(scratch_root) else {
        return;
    };
    let git = WorktreeGit::new(main, git_env).ok();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix("land-") else {
            continue;
        };
        let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
        let path = entry.path();
        if digits(rest) {
            if let Some(git) = &git {
                let text = path.display().to_string();
                let _ = git_ok(git, &["worktree", "remove", "--force", &text], false);
            }
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            }
        } else if rest.strip_suffix("-home").is_some_and(digits) && path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        } else if rest.strip_suffix(".msg").is_some_and(digits) {
            let _ = fs::remove_file(&path);
        }
    }
    if let Some(git) = &git {
        let _ = git_ok(git, &["worktree", "prune"], false);
    }
}

/// The IDs each changed spec document declares, before and after the
/// change: for each path of `changed` the index walks (`[paths]`), the IDs
/// of the file as the merge brings it (the claim's worktree), then those it
/// held at the merge base `base` that it no longer declares (a document
/// deleted, a section stripped). A file gone, unread or not UTF-8 on either
/// side declares none there.
fn declared_ids(
    state: &AutoState,
    git: &WorktreeGit,
    worktree: &Path,
    base: &str,
    changed: &[String],
) -> Vec<(String, Vec<String>)> {
    let config = &state.context.project.config;
    let scope = WalkScope::new(&config.paths);
    let root_rel = state.rule.root_rel.as_str();
    let root = worktree.join(root_rel);
    let ids_of = |path: &str, bytes: &[u8]| -> Vec<String> {
        panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, bytes, &config.scheme)
        }))
        .map(|parsed| {
            parsed
                .nodes
                .iter()
                .filter_map(|node| node.id.clone())
                .collect()
        })
        .unwrap_or_default()
    };
    let mut out = Vec::new();
    for path in changed.iter().filter(|path| scope.in_walk_scope(path)) {
        let mut ids = fs::read(root.join(path))
            .map(|bytes| ids_of(path, &bytes))
            .unwrap_or_default();
        // A missing blob at the base (a new document) declares none.
        if let Ok(bytes) = git.blob_at(base, &joined(root_rel, path)) {
            for id in ids_of(path, &bytes) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        if !ids.is_empty() {
            out.push((path.clone(), ids));
        }
    }
    out
}

/// A land merge of `task` on `branch` whose second parent is `tip`: a land
/// that stopped after moving the branch and before recording it.
fn earlier_merge(git: &WorktreeGit, branch: &str, task: &str, tip: &str) -> Option<String> {
    let grep = format!("--grep=^Task: {task}$");
    let listed = git_ok(
        git,
        &[
            "log",
            "--merges",
            "--format=%H %P",
            &grep,
            "--end-of-options",
            &format!("refs/heads/{branch}"),
        ],
        true,
    )
    .ok()?;
    String::from_utf8_lossy(&listed).lines().find_map(|line| {
        let mut shas = line.split(' ');
        let merge = shas.next()?;
        let _first = shas.next()?;
        (shas.next()? == tip).then(|| merge.to_owned())
    })
}

/// Steps 4 to 11, the lock held.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn steps(
    env: &Env,
    globals: &Globals,
    request: &AutoLandRequest,
    state: &AutoState,
    land: &Land<'_>,
    input: &CheckInput,
    switches: &Switches<'_>,
    scratch_root: &Path,
) -> Result<AutoOutcome, CliError> {
    let task = land.task;
    let hold = |reason: Reason, detail: &str| land.hold(reason, detail, &Done::default());
    // Step 4: the claim's place.
    let Some(claim) = &task.claim else {
        return hold(
            Reason::Place,
            &format!("`{}` has no claim: no worktree to land", task.id),
        );
    };
    let worktree = PathBuf::from(&claim.worktree);
    if !worktree.is_dir() {
        return hold(
            Reason::Place,
            &format!("the claim's worktree {} is gone", claim.worktree),
        );
    }
    let git = match WorktreeGit::new(&worktree, &request.git) {
        Ok(git) => git,
        Err(error) => return hold(Reason::Place, &one_line(&error.to_string())),
    };
    match git.common_dir() {
        Ok(common) if same_repository(&common.display().to_string(), &state.context.common_dir) => {
        }
        Ok(_) => {
            return hold(
                Reason::Place,
                &format!("{} is no worktree of this repository", claim.worktree),
            );
        }
        Err(error) => return hold(Reason::Place, &one_line(&error.to_string())),
    }
    match git.branch() {
        Ok(Some(branch)) if branch == claim.branch => {}
        Ok(Some(branch)) => {
            return hold(
                Reason::Place,
                &format!(
                    "{} is on `{branch}`, not the claim's `{}`",
                    claim.worktree, claim.branch
                ),
            );
        }
        Ok(None) => {
            return hold(
                Reason::Place,
                &format!(
                    "{} has a detached HEAD, not the claim's `{}`",
                    claim.worktree, claim.branch
                ),
            );
        }
        Err(error) => return hold(Reason::Place, &one_line(&error.to_string())),
    }
    match git.operation_in_progress() {
        Ok(None) => {}
        Ok(Some(operation)) => {
            return hold(
                Reason::Place,
                &format!("{} has {operation:?} in progress", claim.worktree),
            );
        }
        Err(error) => return hold(Reason::Place, &one_line(&error.to_string())),
    }
    land.check()?;
    // Step 5: the paths, and the IDs the changed spec documents declare,
    // judged.
    let (base, changed) = match land_changes(&state.rule, task, &land.branch, &request.git) {
        Ok(changes) => changes,
        Err(detail) => return hold(Reason::Place, &detail),
    };
    let facts_of_pack = land.pack.facts().map_err(CliError::spec)?;
    let facts = task_facts(&state.context.project, input, task, Some(facts_of_pack))?;
    let declared = declared_ids(state, &git, &worktree, &base, &changed);
    if let Verdict::Hold { reason, detail } = judge_land(switches, &facts, &changed, &declared) {
        if reason.is_switch() {
            return Ok(AutoOutcome::refused(format!("auto is off: {detail}")));
        }
        return hold(reason, &detail);
    }
    land.check()?;
    let pid = std::process::id();
    let message_file = scratch_root.join(format!("land-{pid}.msg"));
    let _message_guard = Removed(message_file.clone());
    // Step 6: the code commit, only when something is uncommitted.
    let mut code_commit: Option<String> = None;
    let dirty = match git_ok(
        &git,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        true,
    ) {
        Ok(status) => !status.is_empty(),
        Err(detail) => return hold(Reason::Place, &detail),
    };
    if dirty {
        let run = task.runs.iter().rev().find(|run| run.outcome.is_some());
        let title = task.title.clone().unwrap_or_else(|| task.id.clone());
        let summary = run.and_then(|run| run.summary.clone()).unwrap_or_default();
        let message = format!(
            "auto: {} {title}\n\n{summary}\n\nTask: {}\nRun: {}\nPack: {}\n{DELEGATED_BY_TRAILER}: \
             auto {}\n",
            task.id,
            task.id,
            run.map_or(0, |run| run.run),
            land.pack.seq,
            land.rule
        );
        write_message(&message_file, &message)?;
        let file = message_file.display().to_string();
        let committed = git_ok(&git, &["add", "-A"], false)
            .and_then(|_| git_ok(&git, &["commit", "--cleanup=verbatim", "-F", &file], false));
        if let Err(detail) = committed {
            let _ = git_ok(&git, &["reset", "-q"], false);
            return hold(Reason::Place, &detail);
        }
        match git.head() {
            Ok(Some(head)) => code_commit = Some(head),
            Ok(None) => return hold(Reason::Place, "no commit after the code commit"),
            Err(error) => return hold(Reason::Place, &one_line(&error.to_string())),
        }
    }
    land.check()?;
    // Step 7: the task's open `update` and `create` rows raised in this
    // worktree, each judged and applied by the rule; `left` lists every
    // row of the task still open, whichever worktree raised it.
    let mut applied: Vec<String> = Vec::new();
    let mut refusals: Vec<String> = Vec::new();
    let mut messages: Vec<Message> = Vec::new();
    let open_rows = || -> Result<Vec<specengine_store::Proposal>, CliError> {
        let listed = state
            .context
            .queue
            .list_readable(&ProposalFilter {
                git_common_dir: Some(state.repository()),
                statuses: vec![ProposalStatus::Open],
            })
            .map_err(queue_cannot)?;
        Ok(listed
            .proposals
            .into_iter()
            .filter(|proposal| proposal.task_id.as_deref() == Some(task.id.as_str()))
            .collect())
    };
    for proposal in open_rows()? {
        if !matches!(proposal.kind, ProposalKind::Update | ProposalKind::Create)
            || !same_dir(Path::new(&proposal.place.worktree), &worktree)
        {
            continue;
        }
        land.check()?;
        let facts = proposal_facts(
            env,
            &request.git,
            &state.context,
            input,
            &proposal,
            Some(land.pack.seq),
            &mut messages,
        );
        match judge_proposal(switches, Some(&facts_of_pack), &facts, None, land.now) {
            Verdict::Apply => {
                let outcome = delegated_approve(
                    env,
                    globals,
                    &DelegatedApproveRequest {
                        id: proposal.id.clone(),
                        now: land.now.to_owned(),
                        git: request.git.clone(),
                        delegation: Delegation {
                            rule: land.rule.clone(),
                            pack: Some(land.pack.seq),
                        },
                        identity: None,
                        expected: Some(proposal.seen()),
                    },
                )?;
                match &outcome {
                    DelegatedOutcome::Applied(_) => applied.push(proposal.id.clone()),
                    // Refused or moved: still open, told apart from a row
                    // the rule left undecided.
                    DelegatedOutcome::Refused(_) | DelegatedOutcome::Moved { .. } => {
                        refusals.push(format!(
                            "`{}` not applied by the rule: {}",
                            proposal.id,
                            outcome.reason().unwrap_or_default()
                        ));
                    }
                }
            }
            Verdict::Hold { reason, detail } => {
                land.record(&AutoEvent::Held(AutoHeld {
                    pack: Some(land.pack.seq),
                    task: Some(task.id.clone()),
                    line: None,
                    proposal: Some(proposal.id.clone()),
                    act: "apply".to_owned(),
                    reason: reason.as_str().to_owned(),
                    detail,
                    rule: land.rule.clone(),
                }))?;
            }
        }
    }
    let left: Vec<String> = open_rows()?
        .into_iter()
        .map(|proposal| proposal.id)
        .collect();
    let hold_after = |reason: Reason, detail: &str| {
        let done = Done {
            applied: &applied,
            left: &left,
            code_commit: code_commit.as_deref(),
            notes: &refusals,
        };
        land.hold(reason, detail, &done)
    };
    land.check()?;
    // Step 8: the merge, in a scratch worktree; nothing new, never merged.
    let old = match git.commit_of(&format!("refs/heads/{}", land.branch)) {
        Ok(Some(old)) => old,
        Ok(None) => {
            return hold_after(Reason::Place, &format!("`{}` names no commit", land.branch));
        }
        Err(error) => return hold_after(Reason::Place, &one_line(&error.to_string())),
    };
    let claim_ref = format!("refs/heads/{}", claim.branch);
    let tip = match git.commit_of(&claim_ref) {
        Ok(Some(tip)) => tip,
        Ok(None) => {
            return hold_after(
                Reason::Place,
                &format!("`{}` names no commit", claim.branch),
            );
        }
        Err(error) => return hold_after(Reason::Place, &one_line(&error.to_string())),
    };
    match is_ancestor(&git, &tip, &old) {
        Ok(false) => {}
        Ok(true) => {
            return match earlier_merge(&git, &land.branch, &task.id, &tip) {
                Some(merge) => recovered(
                    land,
                    &merge,
                    &applied,
                    &left,
                    code_commit,
                    messages,
                    refusals,
                ),
                None => hold_after(
                    Reason::Place,
                    &format!(
                        "nothing to land: `{}` has no commit `{}` lacks",
                        claim.branch, land.branch
                    ),
                ),
            };
        }
        Err(detail) => return hold_after(Reason::Place, &detail),
    }
    let scratch = scratch_root.join(format!("land-{pid}"));
    let scratch_home = scratch_root.join(format!("land-{pid}-home"));
    let _scratch_guard = Scratch {
        git: git.clone(),
        dir: scratch.clone(),
        home: scratch_home.clone(),
    };
    let scratch_text = scratch.display().to_string();
    if let Err(detail) = git_ok(
        &git,
        &[
            "worktree",
            "add",
            "--detach",
            "--end-of-options",
            &scratch_text,
            &old,
        ],
        false,
    ) {
        return hold_after(Reason::Place, &detail);
    }
    let merger = match WorktreeGit::new(&scratch, &request.git) {
        Ok(merger) => merger,
        Err(error) => return hold_after(Reason::Place, &one_line(&error.to_string())),
    };
    let merge_message = format!(
        "auto: land {} into {}\n\nTask: {}\nPack: {}\n{DELEGATED_BY_TRAILER}: auto {}\n",
        task.id, land.branch, task.id, land.pack.seq, land.rule
    );
    write_message(&message_file, &merge_message)?;
    let file = message_file.display().to_string();
    if let Err(detail) = git_ok(
        &merger,
        &[
            "merge",
            "--no-ff",
            "--cleanup=verbatim",
            "-F",
            &file,
            "--end-of-options",
            &tip,
        ],
        false,
    ) {
        let conflicts = git_ok(
            &merger,
            &["diff", "--name-only", "--diff-filter=U", "-z"],
            true,
        )
        .map(|out| {
            out.split(|&byte| byte == 0)
                .filter(|raw| !raw.is_empty())
                .map(|raw| String::from_utf8_lossy(raw).into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        let _ = git_ok(&merger, &["merge", "--abort"], false);
        if conflicts.is_empty() {
            return hold_after(Reason::Place, &detail);
        }
        return hold_after(Reason::MergeConflict, &conflict_detail(&conflicts));
    }
    let merge = match merger.head() {
        Ok(Some(merge)) if merge != old => merge,
        Ok(Some(_)) => {
            return hold_after(
                Reason::Place,
                &format!(
                    "nothing to land: the merge of `{}` made no commit on `{}`",
                    claim.branch, land.branch
                ),
            );
        }
        Ok(None) => return hold_after(Reason::Place, "no commit after the merge"),
        Err(error) => return hold_after(Reason::Place, &one_line(&error.to_string())),
    };
    land.check()?;
    // Step 9: the check of the merged tree, in a scratch data directory,
    // against the branch's old tip: the scratch worktree's detached `HEAD`
    // set back to `old` (index and files the merge's), so debt the base's
    // baseline lacks is new debt. Anything but a clean verdict holds.
    let checked_root = scratch.join(&state.rule.root_rel);
    let check_env = Env {
        cwd: checked_root.clone(),
        home: Some(scratch_home.clone().into_os_string()),
        xdg_data_home: None,
    };
    if let Err(error) = fs::create_dir_all(&scratch_home) {
        return hold_after(Reason::Check, &format!("cannot run the check: {error}"));
    }
    if let Err(detail) = git_ok(&merger, &["reset", "-q", "--soft", &old], false) {
        return hold_after(Reason::Check, &format!("cannot run the check: {detail}"));
    }
    let checked = check(
        &check_env,
        &Globals {
            root: Some(checked_root),
            config: None,
        },
        &CheckRequest {
            tree: CheckedTree::Changed(request.git.clone()),
            ..CheckRequest::default()
        },
    );
    let _ = git_ok(&merger, &["reset", "-q", "--soft", &merge], false);
    match checked {
        Ok(outcome) => {
            let report = &outcome.report;
            if let Some(cause) = report.cannot_check.first() {
                let detail = if cause.path.is_empty() {
                    cause.message.clone()
                } else {
                    format!("{}: {}", cause.path, cause.message)
                };
                return hold_after(
                    Reason::Check,
                    &one_line(&format!("spec check cannot run: {detail}")),
                );
            }
            if report.verdict != CheckVerdict::Clean {
                // An error outside live debt (expired debt too), else new
                // debt: what keeps the verdict from clean.
                let errors: Vec<_> = report
                    .findings
                    .iter()
                    .filter(|finding| {
                        finding.severity == Severity::Error && !finding.is_live_debt()
                    })
                    .collect();
                let detail = match errors.first() {
                    Some(first) => format!(
                        "spec check: {} error(s); the first {}:{}: {}",
                        errors.len(),
                        first.path,
                        first.line,
                        first.code
                    ),
                    None => format!(
                        "spec check: {}: debt the base's baseline lacks",
                        report.verdict.as_str()
                    ),
                };
                return hold_after(Reason::Check, &detail);
            }
        }
        Err(error) => {
            return hold_after(
                Reason::Check,
                &one_line(&format!("spec check cannot run: {}", error.message)),
            );
        }
    }
    land.check()?;
    // Step 10: the branch moved, by compare-and-set, unless checked out.
    match checked_out_in(&git, &land.branch) {
        Ok(Some(worktree)) => {
            return hold_after(
                Reason::BranchCheckedOut,
                &format!("`{}` is checked out in {worktree}", land.branch),
            );
        }
        Ok(None) => {}
        Err(detail) => return hold_after(Reason::Place, &detail),
    }
    let branch_ref = format!("refs/heads/{}", land.branch);
    if git_ok(&git, &["update-ref", &branch_ref, &merge, &old], false).is_err() {
        return hold_after(
            Reason::BranchMoved,
            &format!(
                "`{}` moved from {} since this land read it",
                land.branch,
                short(&old)
            ),
        );
    }
    // Step 11: the branch moved; recorded whatever arrives now.
    landed(
        land,
        &merge,
        &applied,
        &left,
        code_commit,
        messages,
        refusals,
    )
}

/// Step 11: `auto.landed` recorded, the outcome; `notes` (stderr and JSON
/// `notes`): each row step 7 could not apply, with why, and a recovery's
/// line.
fn landed(
    land: &Land<'_>,
    merge: &str,
    applied: &[String],
    left: &[String],
    code_commit: Option<String>,
    messages: Vec<Message>,
    notes: Vec<String>,
) -> Result<AutoOutcome, CliError> {
    let task = land.task;
    land.record(&AutoEvent::Landed(AutoLanded {
        pack: land.pack.seq,
        task: task.id.clone(),
        branch: land.branch.clone(),
        commit: merge.to_owned(),
        code_commit: code_commit.clone(),
        applied: applied.to_vec(),
        left: left.to_vec(),
        rule: land.rule.clone(),
    }))?;
    let named = |list: &[String]| {
        if list.is_empty() {
            "none".to_owned()
        } else {
            list.join(", ")
        }
    };
    let text = format!(
        "landed {} on {} as {} (applied {}; left {})\n",
        task.id,
        land.branch,
        short(merge),
        named(applied),
        named(left)
    );
    let mut stderr: Vec<String> = messages.iter().map(Message::line).collect();
    stderr.extend(notes.iter().map(|note| Message::Note(note.clone()).line()));
    Ok(AutoOutcome {
        exit: Exit::Answered,
        text: escape_controls(&text),
        json: json_text(&LandJson {
            id: &task.id,
            landed: true,
            branch: &land.branch,
            commit: Some(merge),
            code_commit: code_commit.as_deref(),
            applied,
            left,
            rule: &land.rule,
            reason: None,
            detail: None,
            notes: &notes,
        }),
        stderr: stderr.iter().map(|line| escape_controls(line)).collect(),
    })
}

/// A recovery: `merge`, an earlier land's merge of the claim tip, recorded
/// as this task's, the outcome saying so.
fn recovered(
    land: &Land<'_>,
    merge: &str,
    applied: &[String],
    left: &[String],
    code_commit: Option<String>,
    messages: Vec<Message>,
    mut notes: Vec<String>,
) -> Result<AutoOutcome, CliError> {
    notes.push(format!(
        "recovered: {} merged {} into {} before, unrecorded; recorded now",
        short(merge),
        land.task.id,
        land.branch
    ));
    landed(land, merge, applied, left, code_commit, messages, notes)
}

/// `<n> path(s) conflict: <p1>, …` (8 named, then `and <k> more`).
fn conflict_detail(paths: &[String]) -> String {
    let named: Vec<&str> = paths
        .iter()
        .take(CONFLICTS_NAMED)
        .map(String::as_str)
        .collect();
    let mut detail = format!("{} path(s) conflict: {}", paths.len(), named.join(", "));
    if paths.len() > CONFLICTS_NAMED {
        detail.push_str(&format!(", and {} more", paths.len() - CONFLICTS_NAMED));
    }
    detail
}

/// `text` written to the scratch file `path`.
fn write_message(path: &Path, text: &str) -> Result<(), CliError> {
    fs::write(path, text)
        .map_err(|error| CliError::spec(format!("cannot write {}: {error}", path.display())))
}

/// The land's lock, removed when dropped.
struct LandLock(PathBuf);

/// Why the lock was not taken.
enum Taken {
    /// Another land holds it: its text.
    Held(String),
    Cannot(io::Error),
}

impl LandLock {
    /// Made with create-new, holding `<pid> <now>`; a lock whose pid names
    /// no process is stale, taken over atomically ([`Self::take_over`]).
    fn take(path: &Path, now: &str) -> Result<Self, Taken> {
        let text = format!("{} {now}\n", std::process::id());
        match Self::make(path, &text) {
            Err(Taken::Held(held)) if stale(&held) => Self::take_over(path, &held, &text),
            other => other,
        }
    }

    /// The lock made with create-new, holding `text`; else who holds it.
    fn make(path: &Path, text: &str) -> Result<Self, Taken> {
        let made = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path);
        match made {
            Ok(mut file) => {
                if let Err(error) = file.write_all(text.as_bytes()) {
                    let _ = fs::remove_file(path);
                    return Err(Taken::Cannot(error));
                }
                Ok(Self(path.to_path_buf()))
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                Err(Taken::Held(lock_text(path).unwrap_or_default()))
            }
            Err(error) => Err(Taken::Cannot(error)),
        }
    }

    /// A stale lock (`held`, its pid gone) replaced by this land's, at most
    /// one land at a time: under an exclusive `flock` of `<lock>.takeover`
    /// (another holding it: given up, `held` named), the lock read again
    /// (gone: an ordinary make; another text: given up), `<lock>.<pid>`
    /// written with create-new and renamed over it, then read again (not
    /// this land's text: given up, nothing removed). The lock's path is never
    /// empty meanwhile, so an ordinary make cannot slip in; the `flock` ends
    /// with the process, so a takeover never goes stale.
    fn take_over(path: &Path, held: &str, text: &str) -> Result<Self, Taken> {
        let gate_path = beside(path, ".takeover");
        let gate = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&gate_path)
            .map_err(Taken::Cannot)?;
        match gate.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(Taken::Held(held.to_owned())),
            Err(TryLockError::Error(error)) => return Err(Taken::Cannot(error)),
        }
        match lock_text(path) {
            None => return Self::make(path, text),
            Some(current) if current != held => return Err(Taken::Held(current)),
            Some(_) => {}
        }
        let mine = beside(path, &format!(".{}", std::process::id()));
        let _ = fs::remove_file(&mine);
        let written = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&mine)
            .and_then(|mut file| file.write_all(text.as_bytes()));
        if let Err(error) = written.and_then(|()| fs::rename(&mine, path)) {
            let _ = fs::remove_file(&mine);
            return Err(Taken::Cannot(error));
        }
        match lock_text(path) {
            Some(current) if current == one_line(text.trim()) => Ok(Self(path.to_path_buf())),
            current => Err(Taken::Held(current.unwrap_or_default())),
        }
    }
}

/// `path` with `suffix` appended to its file name.
fn beside(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// The lock's text as shown (one line, trimmed); `None` when there is no
/// lock; unreadable: empty.
fn lock_text(path: &Path) -> Option<String> {
    match fs::read(path) {
        Ok(bytes) => Some(one_line(String::from_utf8_lossy(&bytes).trim())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(_) => Some(String::new()),
    }
}

/// A lock's text `<pid> <time>` whose pid names no process.
fn stale(text: &str) -> bool {
    text.split(' ')
        .next()
        .and_then(|pid| pid.parse::<u32>().ok())
        .is_some_and(|pid| !lock_holder_alive(pid))
}

impl Drop for LandLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// A scratch file removed when dropped.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// The scratch worktree and its check's home, removed when dropped
/// (`git worktree remove --force`, `git worktree prune`).
struct Scratch {
    git: WorktreeGit,
    dir: PathBuf,
    home: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.dir.exists() {
            let dir = self.dir.display().to_string();
            let _ = git_ok(&self.git, &["worktree", "remove", "--force", &dir], false);
        }
        let _ = git_ok(&self.git, &["worktree", "prune"], false);
        if self
            .home
            .starts_with(self.dir.parent().unwrap_or(&self.dir))
            && self.home.exists()
        {
            let _ = fs::remove_dir_all(&self.home);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflicts_name_eight() {
        let paths: Vec<String> = (0..10).map(|n| format!("p{n}")).collect();
        assert_eq!(
            conflict_detail(&paths),
            "10 path(s) conflict: p0, p1, p2, p3, p4, p5, p6, p7, and 2 more"
        );
        assert_eq!(conflict_detail(&paths[..1]), "1 path(s) conflict: p0");
    }

    #[test]
    fn paths_outside_the_root_leave_it() {
        assert_eq!(root_relative("", "src/a.rs"), "src/a.rs");
        assert_eq!(root_relative("sub", "sub/src/a.rs"), "src/a.rs");
        assert_eq!(root_relative("sub", "src/a.rs"), "../src/a.rs");
    }

    /// A scratch directory of this test, removed when dropped.
    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "specengine-auto-land-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_stale_lock_is_taken_over_once() {
        let dir = Dir::new("takeover");
        let lock = dir.0.join("p.auto-land");
        let dead = "4194303999 2026-11-15T00:00:00Z";
        fs::write(&lock, format!("{dead}\n")).unwrap();
        // A takeover under way elsewhere (its gate locked): given up.
        let gate = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(beside(&lock, ".takeover"))
            .unwrap();
        gate.try_lock().unwrap();
        match LandLock::take(&lock, "2026-11-16T00:00:00Z") {
            Err(Taken::Held(text)) => assert_eq!(text, dead),
            _ => panic!("taken while another takeover held the gate"),
        }
        drop(gate);
        let mine = format!("{} 2026-11-16T00:00:00Z", std::process::id());
        let held = match LandLock::take(&lock, "2026-11-16T00:00:00Z") {
            Ok(held) => held,
            Err(_) => panic!("a stale lock not taken over"),
        };
        assert_eq!(lock_text(&lock).as_deref(), Some(mine.as_str()));
        assert!(!beside(&lock, &format!(".{}", std::process::id())).exists());
        // Taken now: a second land refuses, naming this one.
        match LandLock::take(&lock, "2026-11-16T00:00:01Z") {
            Err(Taken::Held(text)) => assert_eq!(text, mine),
            _ => panic!("a live lock taken"),
        }
        drop(held);
        assert!(!lock.exists());
    }

    #[test]
    fn a_lock_of_a_gone_pid_is_stale() {
        assert!(!stale(&format!(
            "{} 2026-11-15T00:00:00Z",
            std::process::id()
        )));
        assert!(!stale("not-a-pid 2026-11-15T00:00:00Z"));
        #[cfg(unix)]
        assert!(stale("4194303999 2026-11-15T00:00:00Z"));
    }
}
