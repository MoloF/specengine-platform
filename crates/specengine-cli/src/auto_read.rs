//! Reading the rule and the facts the predicate takes (task spec
//! `auto-rule` "The table and its reading", "Rules and edge cases"):
//!
//! - [`load_auto`] reads, in this order: the repository's main worktree
//!   (`git worktree list`'s first entry), `HEAD:<root_rel>/specengine.toml`
//!   there (no commit, no file: the switch `off`), [`AutoRule::from_toml`],
//!   `[team]` from the same bytes (one that does not read: the switch
//!   `team`), the stop file's existence, whether the main worktree's file
//!   differs from `HEAD`'s (a note, never a verdict), and `[gate] mode`
//!   from the same bytes (`off` absent). The working tree never decides.
//! - [`proposal_facts`] and [`task_facts`] gather one item's facts in one
//!   snapshot: the predicate ([`specengine_core::auto`]) reads nothing.
//!
//! Nothing here writes, but the index refresh any read does.

use std::fs;
use std::path::{Path, PathBuf};

use specengine_core::GateConfig;
use specengine_core::auto::{
    PackFacts, ProposalFacts, Reason, STOP_FILE, StagedAs, Surface, Switches, TaskFacts, TextState,
    judge_switches, slash_joined,
};
use specengine_core::auto_toml::{AutoRead, AutoRule};
use specengine_core::check::CheckInput;
use specengine_core::gate::GateMode;
use specengine_core::{TeamConfig, TeamRole};
use specengine_store::{GitEnv, KindFamily, Proposal, Stage, Task, WorktreeGit};

use crate::location::data_dir;
use crate::project::ProjectRoot;
use crate::proposals::{Preview, QueueContext};
use crate::propose::{is_path_target, resolved_node};
use crate::review::preview_run;
use crate::{CliError, Env, Message};

/// The project config's file name.
const CONFIG_FILE: &str = "specengine.toml";

/// The stop file of the data directory `env` places (`<data dir>/auto-stop`):
/// its presence holds every project of that directory (`stopped`).
pub fn stop_file(env: &Env) -> Result<PathBuf, CliError> {
    Ok(data_dir(env)?.join(STOP_FILE))
}

/// The rule as one surface reads it now, with what its switches take.
#[derive(Debug, Clone)]
pub struct EffectiveRule {
    pub surface: Surface,
    /// `specengine.toml` joined to `root_rel`, as notes and whys name it
    /// (`@HEAD` added there).
    pub path: String,
    /// The rule at `HEAD`; `None`: off ([`Self::off`] says why).
    pub rule: Option<AutoRule>,
    /// The switch `off`'s why when no rule reads.
    pub off: Option<String>,
    /// `HEAD`'s `specengine.toml` holds no `[auto]` table (a hub prints
    /// nothing for it, task spec `hub-auto`).
    pub absent: bool,
    /// `[team]`'s logins and roles, by login; `Err` the switch `team`'s why.
    pub logins: Result<Vec<(String, TeamRole)>, String>,
    /// `[team]` as read at `HEAD` (a hub's identity); `None` without one.
    pub team: Option<TeamConfig>,
    /// `<data dir>/auto-stop`.
    pub stop_file: PathBuf,
    /// [`Self::stop_file`] as shown, when it exists.
    pub stopped: Option<String>,
    /// `[gate] mode` at `HEAD`; `off` absent.
    pub gate: GateMode,
    /// The read's note (a table that does not read), then the note of a
    /// working tree differing from `HEAD`.
    pub notes: Vec<String>,
    /// The repository's main worktree, canonical.
    pub main_worktree: PathBuf,
    /// The project root inside its worktree, `/`-joined; `""` at its top.
    pub root_rel: String,
    /// The main worktree's `HEAD` commit; `None` unborn.
    pub head: Option<String>,
}

impl EffectiveRule {
    /// The predicate's switches.
    pub fn switches(&self) -> Switches<'_> {
        Switches {
            surface: self.surface,
            rule: match (&self.rule, &self.off) {
                (Some(rule), _) => Ok(rule),
                (None, Some(why)) => Err(why.as_str()),
                (None, None) => Err("[auto] does not read"),
            },
            logins: self.logins.as_deref().map_err(String::as_str),
            stopped: self.stopped.as_deref(),
        }
    }

    /// The first switch that holds, with its why.
    pub fn switch(&self) -> Option<(Reason, String)> {
        judge_switches(&self.switches()).err()
    }

    /// `auto is off: <why>`, the refusal of an act a switch holds.
    pub fn off_line(&self) -> Option<String> {
        self.switch().map(|(_, why)| format!("auto is off: {why}"))
    }

    /// The note a `[gate] mode` other than `selective` adds.
    pub fn gate_note(&self) -> Option<String> {
        (self.gate != GateMode::Selective).then(|| {
            format!(
                "[gate] mode is {}: an agent's uncovered write passes; spec feed --needs-me lists \
                 them",
                self.gate
            )
        })
    }
}

/// The rule of `project`'s repository as `surface` reads it (see the module
/// documentation). Exit 2 only when git cannot run.
pub fn load_auto(
    env: &Env,
    project: &ProjectRoot,
    git_env: &GitEnv,
    surface: Surface,
) -> Result<EffectiveRule, CliError> {
    let cannot = |error: &dyn std::fmt::Display| CliError::spec(format!("cannot run git: {error}"));
    let here = WorktreeGit::new(&project.root, git_env).map_err(|error| cannot(&error))?;
    let top = here.top().map_err(|error| {
        CliError::spec(format!(
            "the project root {} lies in no git worktree ({error}): auto mode reads the rule at \
             HEAD",
            project.root.display()
        ))
    })?;
    let root_rel = relative(&top, &project.root);
    let listed = here.worktrees().map_err(|error| cannot(&error))?;
    let main_worktree = listed
        .first()
        .map_or_else(|| top.clone(), |main| main.path.clone());
    let main = WorktreeGit::new(&main_worktree, git_env).map_err(|error| cannot(&error))?;
    let file = joined(&root_rel, CONFIG_FILE);
    let path = file.clone();
    let stop = stop_file(env)?;
    let stopped = fs::symlink_metadata(&stop)
        .is_ok()
        .then(|| stop.display().to_string());
    let mut out = EffectiveRule {
        surface,
        path: path.clone(),
        rule: None,
        off: None,
        absent: false,
        logins: Ok(Vec::new()),
        team: None,
        stop_file: stop,
        stopped,
        gate: GateMode::Off,
        notes: Vec::new(),
        main_worktree: main_worktree.clone(),
        root_rel,
        head: None,
    };
    let head = main.head().map_err(|error| cannot(&error))?;
    let Some(head) = head else {
        out.off = Some("no commit".to_owned());
        return Ok(out);
    };
    out.head = Some(head.clone());
    if !main
        .has_path(&head, &file)
        .map_err(|error| cannot(&error))?
    {
        out.off = Some(format!("no {CONFIG_FILE} in HEAD"));
        return Ok(out);
    }
    let bytes = main.blob_at(&head, &file).map_err(|error| cannot(&error))?;
    let AutoRead {
        rule,
        absent,
        notes,
    } = AutoRule::from_toml(&bytes, &path);
    if absent {
        out.absent = true;
        out.off = Some(format!("no [auto] table in {path}@HEAD"));
    } else if let Some(note) = notes.first() {
        out.off = Some(format!("[auto] does not read: {}", note.located()));
        out.notes.push(note.text());
    }
    out.rule = rule;
    let text = String::from_utf8_lossy(&bytes);
    match TeamConfig::from_toml(&text) {
        Ok(None) => {}
        Ok(Some(team)) => {
            out.logins = Ok(team
                .entries()
                .map(|entry| (entry.login.clone(), entry.role))
                .collect());
            out.team = Some(team);
        }
        Err(error) => {
            out.logins = Err(format!("`[team]` does not read: {}", error.message));
        }
    }
    out.gate = match GateConfig::from_toml(&text) {
        Ok(Some(gate)) => gate.mode,
        _ => GateMode::Off,
    };
    // An uncommitted edit changes no verdict: one note.
    let on_disk = fs::read(main_worktree.join(&file)).ok();
    if on_disk.as_deref() != Some(bytes.as_slice()) {
        out.notes.push(format!(
            "{path} differs from HEAD: auto reads HEAD's; commit it to change the rule"
        ));
    }
    Ok(out)
}

/// `inner` relative to `outer`, `/`-joined; `""` when equal (or outside).
fn relative(outer: &Path, inner: &Path) -> String {
    inner
        .strip_prefix(outer)
        .map(|rest| {
            slash_joined(
                rest.components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned()),
            )
        })
        .unwrap_or_default()
}

/// `name` below the `/`-joined `root_rel`.
pub(crate) fn joined(root_rel: &str, name: &str) -> String {
    if root_rel.is_empty() {
        name.to_owned()
    } else {
        format!("{root_rel}/{name}")
    }
}

/// A proposal's facts (task spec `auto-rule` "Rules and edge cases"):
/// `paths` its target path; `ids` the target ID when it is one, the target
/// file's own `id` (its document node in the index), a create's new IDs
/// beyond the target; `text` from the review's preview, a file-form create
/// [`TextState::FileExists`] when its file is on disk at the recorded place.
/// `input` is the reading root's index, used when the preview read no
/// recorded root.
pub(crate) fn proposal_facts(
    env: &Env,
    git_env: &GitEnv,
    context: &QueueContext,
    input: &CheckInput,
    proposal: &Proposal,
    pack: Option<i64>,
    messages: &mut Vec<Message>,
) -> ProposalFacts {
    let staged = proposal.staged.as_ref().map(|staged| {
        let which = match staged.stage {
            Stage::Approve { .. } => StagedAs::Approve,
            Stage::Reject { .. } => StagedAs::Reject,
        };
        (which, staged.at.clone())
    });
    let applies = proposal.kind.family() == KindFamily::Applies;
    let mut text = TextState::Applies;
    let mut document_id = None;
    if applies {
        let on_disk = Path::new(&proposal.place.worktree)
            .join(&proposal.place.root_rel)
            .join(&proposal.target_path);
        if proposal.new_file() && fs::symlink_metadata(&on_disk).is_ok() {
            text = TextState::FileExists;
        } else {
            let run = preview_run(env, git_env, context, proposal, messages);
            text = match run.preview() {
                Some(Preview::Applies) => TextState::Applies,
                Some(Preview::Rebases) => TextState::Rebases,
                Some(Preview::Conflicts) => TextState::Conflict,
                Some(Preview::Unavailable) | None => TextState::Gone,
            };
            document_id = run
                .corpus
                .as_ref()
                .and_then(|corpus| document_of(&corpus.input, &proposal.target_path));
        }
    }
    let document_id = document_id.or_else(|| document_of(input, &proposal.target_path));
    let mut ids: Vec<String> = Vec::new();
    let mut add = |id: &str| {
        if !id.is_empty() && !is_path_target(id) && !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    };
    add(&proposal.target_id);
    if let Some(id) = &document_id {
        add(id);
    }
    for id in proposal.target_ids() {
        add(&id);
    }
    ProposalFacts {
        id: proposal.id.clone(),
        kind: proposal.kind.as_str().to_owned(),
        staged,
        author_type: proposal.author.author_type,
        author_login: proposal.author.login.clone(),
        pack,
        paths: vec![proposal.target_path.clone()],
        ids,
        findings: proposal
            .diagnostics
            .iter()
            .map(|finding| {
                (
                    finding.code.clone(),
                    finding.path.clone(),
                    u32::try_from(finding.line).unwrap_or(u32::MAX),
                )
            })
            .collect(),
        text,
    }
}

/// The `id` of the document `path` holds in `input`.
fn document_of(input: &CheckInput, path: &str) -> Option<String> {
    input
        .files
        .iter()
        .find(|file| file.path == path)
        .and_then(|file| file.parsed.as_ref())
        .and_then(|parsed| parsed.document())
        .and_then(|node| node.id.clone())
}

/// A task's facts: its targets and affected nodes, each once, resolved to
/// their holder paths in `project` over `input` (one that does not resolve
/// kept by its ID, judged `uncovered`); its last closed run.
pub(crate) fn task_facts<'a>(
    project: &ProjectRoot,
    input: &CheckInput,
    task: &Task,
    pack: Option<PackFacts<'a>>,
) -> Result<TaskFacts<'a>, CliError> {
    let mut ids: Vec<String> = Vec::new();
    let mut paths: Vec<String> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for named in task.targets.iter().chain(&task.affected_nodes) {
        if seen.contains(&named.as_str()) {
            continue;
        }
        seen.push(named);
        if is_path_target(named) {
            if !paths.contains(named) {
                paths.push(named.clone());
            }
            continue;
        }
        ids.push(named.clone());
        match resolved_node(project, input, named)? {
            Ok((_, path)) => {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
            Err(_) => unresolved.push(named.clone()),
        }
    }
    let last_run = task.runs.iter().rev().find_map(|run| {
        run.outcome
            .map(|outcome| (u32::try_from(run.run).unwrap_or(u32::MAX), outcome))
    });
    Ok(TaskFacts {
        id: task.id.clone(),
        status: task.status,
        pack,
        paths,
        ids,
        unresolved,
        last_run,
    })
}
