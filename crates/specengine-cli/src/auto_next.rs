//! `spec auto next` and lanes (task spec `auto-land` "`spec auto next`",
//! "Lanes"): the runner's next act, read-only and deterministic, always one
//! JSON document.
//!
//! - **Packs**: the live ones whose own switches pass (a branch changed
//!   since a pack's start, or a frozen rule that does not read, skips it;
//!   none left: `stop`); past its `--until` a pack offers only its lands.
//! - **Running**: the live packs' `in_progress` tasks and their `done` ones,
//!   neither landed nor held (a held item is finished), plus each `--busy`
//!   item, a line too (it makes the end a `wait`). Two items overlap
//!   when their sets (targets and affected nodes, canonical, and their
//!   holder paths) meet; a line overlaps nothing.
//! - **Candidates**, the first wins, by action (`land`, `approve`,
//!   `implement`, `plan`, `plan-line`), then pack `seq`, then item order;
//!   `--skip-lane` passes over a lane's, `--slots` (default 1) at 0 leaves
//!   only `land`. None: `wait` when anything runs or a candidate was passed
//!   over only for an overlap or the slots; else `done`.
//! - **A lane** is the first `[[workspace]]`, in config order, whose
//!   `module` holds every target (a rooted module: its root is the target
//!   or one of its ancestors; a direction: the target's kind is one it
//!   lists); else, and for a line, `general`. Advisory, never a right.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use serde::Serialize;
use specengine_core::auto::{Reason, Surface, Switches, bare_id, judge_pack_switches};
use specengine_core::check::{CheckInput, NodeAt, SpecGraph};
use specengine_core::workspace_toml::WorkspaceConfig;
use specengine_model::TaskStatus;
use specengine_store::{GitEnv, Task};

use crate::auto::{AutoOutcome, json_text};
use crate::auto_pack::{AutoState, Item, Pack, auto_state, parse_line};
use crate::auto_read::task_facts;
use crate::corpus::indexed;
use crate::project::ProjectRoot;
use crate::proposals::{checked_now, escaped_error};
use crate::propose::resolved_at;
use crate::task::written_task;
use crate::{CliError, Env, Exit, Globals, Message};

/// A task outside every workspace's module, and every line.
pub const GENERAL_LANE: &str = "general";

/// The project's lanes as `[[workspace]]` declares them, judged once over
/// one index: each lane's held nodes, and where each canonical name sits.
pub(crate) struct Lanes {
    /// Each workspace with a served module, in config order: its name and
    /// the nodes its module holds.
    lanes: Vec<(String, BTreeSet<NodeAt>)>,
    /// A node ID or an id-less document's path to its node (the first
    /// written, as the resolver takes it).
    placed: BTreeMap<String, NodeAt>,
}

impl Lanes {
    /// The lanes of `project`'s own `specengine.toml` (every read's
    /// `[[workspace]]`), judged over `input`.
    pub(crate) fn read(project: &ProjectRoot, input: &CheckInput) -> Self {
        let text = fs::read_to_string(project.root.join("specengine.toml")).unwrap_or_default();
        let config = WorkspaceConfig::from_toml(&text);
        let served: Vec<(&str, &specengine_core::workspace_toml::WorkspaceModule)> = config
            .workspaces
            .iter()
            .filter_map(|workspace| {
                let name = workspace.module.as_ref()?;
                let module = config.modules.iter().find(|module| &module.name == name)?;
                Some((workspace.name.as_str(), module))
            })
            .collect();
        if served.is_empty() {
            return Self {
                lanes: Vec::new(),
                placed: BTreeMap::new(),
            };
        }
        let graph = SpecGraph::new(input, &project.config.scheme, &project.config.paths);
        let mut placed: BTreeMap<String, NodeAt> = BTreeMap::new();
        let mut every: Vec<NodeAt> = Vec::new();
        for (file, path) in graph.paths().iter().enumerate() {
            for (ord, node) in graph.nodes(file).iter().enumerate() {
                let at = NodeAt { file, ord };
                every.push(at);
                if ord == 0 {
                    placed.entry((*path).to_owned()).or_insert(at);
                }
                if let Some(id) = &node.id {
                    placed.entry(id.clone()).or_insert(at);
                }
            }
        }
        let resolved = |written: &str| -> Option<NodeAt> {
            match resolved_at(project, input, written) {
                Ok(Ok((_, path, ord))) => Some(NodeAt {
                    file: graph.file_of(&path)?,
                    ord,
                }),
                _ => None,
            }
        };
        let mut lanes = Vec::new();
        for (name, module) in served {
            let held: BTreeSet<NodeAt> = match (&module.root, &module.kinds) {
                (Some(root), _) => match resolved(root) {
                    Some(root) => every
                        .iter()
                        .copied()
                        .filter(|at| *at == root || graph.ancestors(*at).contains(&root))
                        .collect(),
                    None => BTreeSet::new(),
                },
                (None, Some(kinds)) => every
                    .iter()
                    .copied()
                    .filter(|at| {
                        graph
                            .node(*at)
                            .and_then(|node| node.kind.as_ref())
                            .is_some_and(|kind| kinds.contains(kind))
                    })
                    .collect(),
                (None, None) => continue,
            };
            lanes.push((name.to_owned(), held));
        }
        Self { lanes, placed }
    }

    /// Where a stored, canonical target sits: an ID, a `slug/ID` (its ID),
    /// or an id-less document's path.
    fn at(&self, target: &str) -> Option<NodeAt> {
        self.placed.get(target).copied().or_else(|| {
            let bare = bare_id(target);
            (bare != target)
                .then(|| self.placed.get(bare).copied())
                .flatten()
        })
    }

    /// The lane of `task`; `general` for a line (`None`) and a task no
    /// module holds whole.
    pub(crate) fn lane(&self, task: Option<&Task>) -> String {
        let Some(task) = task else {
            return GENERAL_LANE.to_owned();
        };
        if self.lanes.is_empty() || task.targets.is_empty() {
            return GENERAL_LANE.to_owned();
        }
        let targets: Vec<Option<NodeAt>> =
            task.targets.iter().map(|target| self.at(target)).collect();
        self.lanes
            .iter()
            .find(|(_, held)| {
                targets
                    .iter()
                    .all(|target| target.is_some_and(|at| held.contains(&at)))
            })
            .map_or_else(|| GENERAL_LANE.to_owned(), |(name, _)| name.clone())
    }
}

/// `spec auto next` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoNextRequest {
    /// `--busy ITEM`: a task ID or `<pack>/<n>` the runner works on.
    pub busy: Vec<String>,
    /// `--skip-lane D`.
    pub skip_lanes: Vec<String>,
    /// `--slots N`; default 1.
    pub slots: u32,
    pub now: String,
    pub git: GitEnv,
}

/// `spec auto next`'s document: every key present.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct NextAct {
    pub action: &'static str,
    pub reason: Option<String>,
    pub why: Option<String>,
    pub pack: Option<i64>,
    pub item: Option<String>,
    pub task: Option<String>,
    pub line: Option<u32>,
    pub text: Option<String>,
    pub title: Option<String>,
    pub lane: Option<String>,
    pub branch: Option<String>,
    pub base: Option<String>,
}

/// `spec auto next` (see the module documentation): exit 0 with its
/// document; 2 only when it cannot run.
pub fn auto_next(
    env: &Env,
    globals: &Globals,
    request: &AutoNextRequest,
) -> Result<AutoOutcome, CliError> {
    let act = run_next(env, globals, request).map_err(escaped_error)?;
    let json = json_text(&act);
    Ok(AutoOutcome {
        exit: Exit::Answered,
        text: json.clone(),
        json,
        stderr: Vec::new(),
    })
}

/// One candidate item, its set and lane.
struct Candidate<'s> {
    action: &'static str,
    pack: &'s Pack,
    item: &'s Item,
    task: Option<&'s Task>,
}

/// The `stop` document of a switch.
fn stop(reason: Reason, why: String) -> NextAct {
    NextAct {
        action: "stop",
        reason: Some(reason.as_str().to_owned()),
        why: Some(why),
        ..NextAct::default()
    }
}

fn run_next(env: &Env, globals: &Globals, request: &AutoNextRequest) -> Result<NextAct, CliError> {
    let now = checked_now(&request.now)?;
    let state = auto_state(env, globals, &request.git, Surface::Local)?;
    if let Some((reason, why)) = state.rule.switch() {
        return Ok(stop(reason, why));
    }
    let live: Vec<&Pack> = state
        .packs
        .iter()
        .filter(|pack| pack.live(&state.tasks))
        .collect();
    // A pack whose own switches hold (its branch changed since its start)
    // or whose frozen rule does not read gives no act; none left: `stop`.
    let switches = Switches {
        surface: Surface::Local,
        ..state.rule.switches()
    };
    let mut skipped: Option<(Reason, String)> = None;
    let mut kept: Vec<&Pack> = Vec::with_capacity(live.len());
    for pack in &live {
        let judged = match pack.facts() {
            Ok(facts) => judge_pack_switches(&switches, &facts),
            Err(why) => Err((
                Reason::Off,
                format!("pack {}'s frozen rule does not read: {why}", pack.seq),
            )),
        };
        match judged {
            Ok(()) => kept.push(pack),
            Err(hold) => {
                skipped.get_or_insert(hold);
            }
        }
    }
    if kept.is_empty()
        && let Some((reason, why)) = skipped
    {
        return Ok(stop(reason, why));
    }
    let mut messages: Vec<Message> = Vec::new();
    let input = indexed(env, &state.context.project, &mut messages, false)?;
    let lanes = Lanes::read(&state.context.project, &input);
    let busy = busy_items(&request.busy)?;
    let sets = Sets {
        state: &state,
        input: &input,
    };
    // Running: the live packs' in_progress tasks, their done ones neither
    // landed nor held, and each busy item (a line too).
    let mut running: Vec<BTreeSet<String>> = Vec::new();
    for pack in &live {
        for item in &pack.items {
            let Some(task) = item.task.as_ref().and_then(|id| state.tasks.get(id)) else {
                continue;
            };
            // A finished item (landed or held) no longer runs.
            let runs = matches!(task.status, TaskStatus::InProgress | TaskStatus::Done)
                && item.landed.is_none()
                && item.held.is_none();
            if runs {
                running.push(sets.of(task)?);
            }
        }
    }
    for written in &busy {
        if let Busy::Task(id) = written
            && let Some(task) = state.tasks.get(id)
        {
            running.push(sets.of(task)?);
        }
    }
    let is_busy = |pack: &Pack, item: &Item| {
        busy.iter().any(|busy| match busy {
            Busy::Task(id) => item.task.as_deref() == Some(id.as_str()),
            Busy::Line(seq, line) => *seq == pack.seq && item.line == Some(*line),
        })
    };
    let mut candidates: Vec<Candidate<'_>> = Vec::new();
    // 1. land: a kept pack's done task, its `until` aside.
    for pack in &kept {
        for item in &pack.items {
            let task = item.task.as_ref().and_then(|id| state.tasks.get(id));
            if task.is_some_and(|task| task.status == TaskStatus::Done)
                && item.landed.is_none()
                && item.held.is_none()
            {
                candidates.push(Candidate {
                    action: "land",
                    pack,
                    item,
                    task,
                });
            }
        }
    }
    // 2.-5. a kept pack's other acts, until its `--until` (past it, the
    // predicate's `until` holds an approve and an implement).
    for action in ["approve", "implement", "plan", "plan-line"] {
        for pack in kept.iter().filter(|pack| !pack.ended(now)) {
            for item in &pack.items {
                if item.landed.is_some() || item.held.is_some() {
                    continue;
                }
                let task = item.task.as_ref().and_then(|id| state.tasks.get(id));
                let wanted = match (action, task) {
                    ("approve", Some(task)) => task.status == TaskStatus::Review,
                    ("implement", Some(task)) => task.status == TaskStatus::Ready,
                    ("plan", Some(task)) => matches!(
                        task.status,
                        TaskStatus::Draft | TaskStatus::ChangesRequested
                    ),
                    ("plan-line", None) => item.line.is_some(),
                    _ => false,
                };
                if wanted {
                    candidates.push(Candidate {
                        action,
                        pack,
                        item,
                        task,
                    });
                }
            }
        }
    }
    // Anything running, a busy line included, makes the end a `wait`.
    let mut waited = !running.is_empty() || !busy.is_empty();
    for candidate in candidates {
        let lane = lanes.lane(candidate.task);
        if request.skip_lanes.contains(&lane) {
            continue;
        }
        if candidate.action != "land" {
            if request.slots == 0 {
                waited = true;
                continue;
            }
            if candidate.action != "approve" && is_busy(candidate.pack, candidate.item) {
                continue;
            }
            if matches!(candidate.action, "approve" | "implement")
                && let Some(task) = candidate.task
            {
                let set = sets.of(task)?;
                if running.iter().any(|other| !other.is_disjoint(&set)) {
                    waited = true;
                    continue;
                }
            }
        }
        return Ok(act_of(&state, &candidate, lane));
    }
    Ok(NextAct {
        action: if waited { "wait" } else { "done" },
        ..NextAct::default()
    })
}

/// The document of the chosen candidate.
fn act_of(state: &AutoState, candidate: &Candidate<'_>, lane: String) -> NextAct {
    let branch_of = |task: &Task| {
        let base = state.rule.rule.as_ref().map_or_else(
            || candidate.pack.started.branch.clone(),
            |rule| rule.branch.clone(),
        );
        (format!("{base}-{}", task.id), base)
    };
    let (branch, base) = match (candidate.action, candidate.task) {
        ("implement" | "approve", Some(task)) => {
            let (branch, base) = branch_of(task);
            (Some(branch), Some(base))
        }
        _ => (None, None),
    };
    NextAct {
        action: candidate.action,
        reason: None,
        why: None,
        pack: Some(candidate.pack.seq),
        item: Some(candidate.item.name(candidate.pack.seq)),
        task: candidate.item.task.clone(),
        line: candidate.item.line,
        text: candidate.item.text.clone(),
        title: candidate.task.and_then(|task| task.title.clone()),
        lane: Some(lane),
        branch,
        base,
    }
}

/// A `--busy` item.
enum Busy {
    Task(String),
    Line(i64, u32),
}

/// The `--busy` items as written: a task ID or `<pack>/<n>`, else exit 2.
fn busy_items(written: &[String]) -> Result<Vec<Busy>, CliError> {
    written
        .iter()
        .map(|item| {
            let item = item.trim();
            if let Some((pack, line)) = parse_line(item) {
                return Ok(Busy::Line(pack, line));
            }
            match written_task(item)? {
                Some(id) => Ok(Busy::Task(id)),
                None => Err(CliError::spec(format!(
                    "--busy `{item}` is no task ID and no `<pack>/<n>`"
                ))),
            }
        })
        .collect()
}

/// A task's overlap set: its targets and affected nodes, canonical, and
/// their holder paths.
struct Sets<'a> {
    state: &'a AutoState,
    input: &'a CheckInput,
}

impl Sets<'_> {
    fn of(&self, task: &Task) -> Result<BTreeSet<String>, CliError> {
        let facts = task_facts(&self.state.context.project, self.input, task, None)?;
        let mut set: BTreeSet<String> = BTreeSet::new();
        for id in facts.ids.iter().chain(&facts.unresolved) {
            set.insert(format!("id:{id}"));
        }
        for path in &facts.paths {
            set.insert(format!("path:{path}"));
        }
        Ok(set)
    }
}
