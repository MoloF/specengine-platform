//! `spec auto show|stop|resume` (task spec `auto-rule` "`spec auto show`"):
//! a dry run of the rule over this repository's packs and open proposals,
//! writing nothing; the stop file made and removed. The other `spec auto`
//! commands live beside it (`auto_pack`, `auto_approve`, `auto_land`,
//! `auto_next`); every one answers an [`AutoOutcome`].
//!
//! - **show**: the effective rule, its switches, every live pack's items
//!   (each with its act and verdict) and every `open` proposal of this
//!   repository by ID number, judged for `--surface`. A switch: `off`, every
//!   item held by it. Exit 0; 2 only when it cannot run.
//! - **stop**: `<data dir>/auto-stop` holding `stopped <now> by <identity>`
//!   and LF, made when absent; no project read. Already there: unchanged.
//! - **resume**: removes it after a terminal `[y/N]` (`main` refuses a
//!   stdin that is no terminal before anything is read).

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use serde::Serialize;
use specengine_core::auto::{
    PackFacts, Reason, Surface, Switches, TaskFacts, Verdict, judge_approve, judge_land_paths,
    judge_pack_act, judge_proposal,
};
use specengine_model::TaskStatus;
use specengine_store::{
    GitEnv, ProposalFilter, ProposalQueue as _, ProposalStatus, Task, WorktreeGit,
};

use crate::auto_land::land_paths;
use crate::auto_next::Lanes;
use crate::auto_pack::{AutoState, Item, Pack, auto_state, short};
use crate::auto_read::{proposal_facts, stop_file, task_facts};
use crate::corpus::indexed;
use crate::proposals::{checked_now, escaped_error, queue_cannot};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// What a `spec auto` command answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoOutcome {
    pub exit: Exit,
    /// stdout without `--json`.
    pub text: String,
    /// stdout with `--json`: one document, compact, LF-ended; empty when the
    /// command prints none.
    pub json: String,
    /// stderr lines, each one line.
    pub stderr: Vec<String>,
}

impl AutoOutcome {
    /// Exit 1, `spec: <reason>` on stderr, nothing on stdout: nothing
    /// written.
    pub fn refused(reason: impl Into<String>) -> Self {
        Self {
            exit: Exit::NotFound,
            text: String::new(),
            json: String::new(),
            stderr: vec![format!(
                "spec: {}",
                one_line(&escape_controls(&reason.into()))
            )],
        }
    }

    /// Exit 1 with `document` as the `--json` stdout and the reason on
    /// stderr.
    pub(crate) fn refused_with(reason: &str, document: String) -> Self {
        let mut refused = Self::refused(reason);
        refused.json = document;
        refused
    }

    /// stdout as `--json` asks.
    pub fn stdout(&self, json: bool) -> &str {
        if json { &self.json } else { &self.text }
    }
}

/// `value` as one compact JSON line, LF-ended.
pub(crate) fn json_text<T: Serialize>(value: &T) -> String {
    let mut text = serde_json::to_string(value).unwrap_or_default();
    text.push('\n');
    text
}

// -------------------------------------------------------------- show

/// `spec auto show [--surface local|hub]` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoShowRequest {
    pub surface: Surface,
    pub now: String,
    pub git: GitEnv,
}

/// The `auto` head of `spec auto show --json`: every key present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShownAuto {
    pub state: &'static str,
    pub reason: Option<String>,
    pub why: Option<String>,
    pub surface: &'static str,
    pub rule: Option<String>,
    pub mode: Option<String>,
    pub zones: Option<Vec<String>>,
    pub exclude_zones: Option<Vec<String>>,
    pub exclude_kinds: Option<Vec<String>>,
    pub branch: Option<String>,
    pub max_tasks: Option<u32>,
    pub max_applies: Option<u32>,
    pub logins: Option<usize>,
    pub stop_file: String,
    pub stopped: bool,
    pub applies_today: Option<u32>,
    pub gate: String,
    /// Why `[team]` does not read, for the text; not a key.
    #[serde(skip)]
    pub team_why: Option<String>,
}

/// One pack of `spec auto show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShownPack {
    pub pack: i64,
    pub at: String,
    pub by: String,
    pub until: Option<String>,
    pub branch: String,
    pub rule: String,
    pub items: Vec<ShownItem>,
}

/// One item of a shown pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShownItem {
    pub item: String,
    pub task: Option<String>,
    pub line: Option<u32>,
    pub text: Option<String>,
    pub status: String,
    pub act: String,
    pub verdict: String,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub commit: Option<String>,
    pub lane: String,
}

/// One open proposal of `spec auto show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShownProposal {
    pub id: String,
    pub kind: String,
    pub task_id: Option<String>,
    pub verdict: String,
    pub reason: Option<String>,
    pub detail: Option<String>,
}

/// `spec auto show`'s document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutoShowOutcome {
    pub auto: ShownAuto,
    pub packs: Vec<ShownPack>,
    pub proposals: Vec<ShownProposal>,
    pub notes: Vec<String>,
}

impl AutoShowOutcome {
    /// The text (see the module documentation), control characters escaped.
    pub fn text(&self) -> String {
        let auto = &self.auto;
        let mut out = String::new();
        match (auto.state, &auto.reason, &auto.why) {
            ("on", _, _) => {
                out.push_str(&format!(
                    "auto: on ({}), rule {}\n",
                    auto.mode.as_deref().unwrap_or_default(),
                    auto.rule.as_deref().unwrap_or_default()
                ));
                let list = |names: &Option<Vec<String>>| match names.as_deref() {
                    Some([]) | None => "none".to_owned(),
                    Some(names) => names.join(", "),
                };
                out.push_str(&format!(
                    "  zones: {}; excluded zones: {}; excluded kinds: {}; branch: {}; max_tasks \
                     {}; max_applies {}\n",
                    list(&auto.zones),
                    list(&auto.exclude_zones),
                    list(&auto.exclude_kinds),
                    auto.branch.as_deref().unwrap_or_default(),
                    auto.max_tasks.unwrap_or_default(),
                    auto.max_applies.unwrap_or_default()
                ));
            }
            (_, reason, why) => out.push_str(&format!(
                "auto: off ({}): {}\n",
                reason.as_deref().unwrap_or_default(),
                why.as_deref().unwrap_or_default()
            )),
        }
        match (auto.logins, &auto.team_why) {
            (_, Some(why)) => out.push_str(&format!("team: {why}\n")),
            (Some(0) | None, None) => out.push_str("team: none\n"),
            (Some(n), None) => out.push_str(&format!("team: {n} login(s)\n")),
        }
        out.push_str(&format!(
            "stop file: {} {}\n",
            auto.stop_file,
            if auto.stopped { "exists" } else { "absent" }
        ));
        for pack in &self.packs {
            out.push_str(&format!(
                "pack {}: started {} by {}, until {}, on {}, rule {}\n",
                pack.pack,
                pack.at,
                pack.by,
                pack.until.as_deref().unwrap_or("no end"),
                pack.branch,
                pack.rule
            ));
            for item in &pack.items {
                let verdict = match (&item.commit, item.verdict.as_str()) {
                    (Some(commit), "landed") => format!("landed {}", short(commit)),
                    (_, verdict) => verdict.to_owned(),
                };
                out.push_str(&format!(
                    "  {} | {} | {} | {verdict} | {}\n",
                    item.item,
                    item.status,
                    item.act,
                    reason_field(item.reason.as_deref(), item.detail.as_deref())
                ));
            }
        }
        out.push_str("proposals:\n");
        for proposal in &self.proposals {
            out.push_str(&format!(
                "  {} | {} | {} | {} | {}\n",
                proposal.id,
                proposal.kind,
                proposal.task_id.as_deref().unwrap_or("-"),
                proposal.verdict,
                reason_field(proposal.reason.as_deref(), proposal.detail.as_deref())
            ));
        }
        for note in &self.notes {
            out.push_str(&format!("note: {}\n", one_line(note)));
        }
        escape_controls(&out)
    }
}

/// `<reason>: <detail>`, `-` for none.
fn reason_field(reason: Option<&str>, detail: Option<&str>) -> String {
    match (reason, detail) {
        (Some(reason), Some(detail)) => format!("{reason}: {detail}"),
        (Some(reason), None) => reason.to_owned(),
        _ => "-".to_owned(),
    }
}

/// `spec auto show` (see the module documentation).
pub fn auto_show(
    env: &Env,
    globals: &Globals,
    request: &AutoShowRequest,
) -> Result<AutoShowOutcome, CliError> {
    run_show(env, globals, request).map_err(escaped_error)
}

fn run_show(
    env: &Env,
    globals: &Globals,
    request: &AutoShowRequest,
) -> Result<AutoShowOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let state = auto_state(env, globals, &request.git, request.surface)?;
    let rule = &state.rule;
    let switch = rule.switch();
    let applies_today = match request.surface {
        Surface::Hub => Some(
            state
                .context
                .queue
                .rule_applies_on(&now[..10])
                .map_err(queue_cannot)?,
        ),
        Surface::Local => None,
    };
    let on = switch.is_none().then_some(rule.rule.as_ref()).flatten();
    let auto = ShownAuto {
        state: if on.is_some() { "on" } else { "off" },
        reason: switch
            .as_ref()
            .map(|(reason, _)| reason.as_str().to_owned()),
        why: switch.as_ref().map(|(_, why)| why.clone()),
        surface: request.surface.as_str(),
        rule: on.map(|rule| rule.hash.clone()),
        mode: on.map(|rule| rule.mode.as_str().to_owned()),
        zones: on.map(|rule| owned(&rule.zone_names())),
        exclude_zones: on.map(|rule| owned(&rule.excluded_zone_names())),
        exclude_kinds: on.map(|rule| owned(&rule.excluded_kind_names())),
        branch: on.map(|rule| rule.branch.clone()),
        max_tasks: on.map(|rule| rule.max_tasks),
        max_applies: on.map(|rule| rule.max_applies),
        logins: rule.logins.as_ref().ok().map(Vec::len),
        stop_file: rule.stop_file.display().to_string(),
        stopped: rule.stopped.is_some(),
        applies_today,
        gate: rule.gate.as_str().to_owned(),
        team_why: rule.logins.as_ref().err().cloned(),
    };
    let mut messages: Vec<Message> = Vec::new();
    let input = indexed(env, &state.context.project, &mut messages, false)?;
    let lanes = Lanes::read(&state.context.project, &input);
    let local = Switches {
        surface: Surface::Local,
        ..rule.switches()
    };
    let mut packs = Vec::new();
    for pack in state.packs.iter().filter(|pack| pack.live(&state.tasks)) {
        let mut items = Vec::new();
        for item in &pack.items {
            items.push(item_row(
                &state,
                &request.git,
                &input,
                &lanes,
                &local,
                pack,
                item,
                now,
            )?);
        }
        packs.push(ShownPack {
            pack: pack.seq,
            at: pack.at.clone(),
            by: pack.started.by.clone(),
            until: pack.started.until.clone(),
            branch: pack.started.branch.clone(),
            rule: pack.started.rule.clone(),
            items,
        });
    }
    let switches = rule.switches();
    let mut proposals = Vec::new();
    let listed = state
        .context
        .queue
        .list_readable(&ProposalFilter {
            git_common_dir: Some(state.repository()),
            statuses: vec![ProposalStatus::Open],
        })
        .map_err(queue_cannot)?;
    for proposal in &listed.proposals {
        let pack = proposal.task_id.as_deref().and_then(|task| {
            crate::auto_pack::pack_of(&state.packs, task, &state.tasks, false).map(|(pack, _)| pack)
        });
        let facts = proposal_facts(
            env,
            &request.git,
            &state.context,
            &input,
            proposal,
            pack.map(|pack| pack.seq),
            &mut messages,
        );
        let verdict = match pack.map(Pack::facts) {
            Some(Err(why)) => frozen_off(why),
            Some(Ok(facts_of_pack)) => {
                judge_proposal(&switches, Some(&facts_of_pack), &facts, applies_today, now)
            }
            None => judge_proposal(&switches, None, &facts, applies_today, now),
        };
        let (verdict, reason, detail) = verdict_fields(&verdict);
        proposals.push(ShownProposal {
            id: proposal.id.clone(),
            kind: proposal.kind.as_str().to_owned(),
            task_id: proposal.task_id.clone(),
            verdict,
            reason,
            detail,
        });
    }
    let mut notes = rule.notes.clone();
    notes.extend(rule.gate_note());
    Ok(AutoShowOutcome {
        auto,
        packs,
        proposals,
        notes,
    })
}

/// The names as owned strings.
fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// A pack whose frozen rule does not read: held `off`.
pub(crate) fn frozen_off(why: String) -> Verdict {
    Verdict::Hold {
        reason: Reason::Off,
        detail: format!("the pack's frozen rule does not read: {why}"),
    }
}

/// `(verdict, reason, detail)` of a verdict.
fn verdict_fields(verdict: &Verdict) -> (String, Option<String>, Option<String>) {
    match verdict {
        Verdict::Apply => ("apply".to_owned(), None, None),
        Verdict::Hold { reason, detail } => (
            "hold".to_owned(),
            Some(reason.as_str().to_owned()),
            Some(detail.clone()),
        ),
    }
}

/// An item's act by its task's state (see the module documentation):
/// `None` for one no act takes.
pub(crate) fn act_of(item: &Item, task: Option<&Task>) -> &'static str {
    match (task, item.line) {
        (None, Some(_)) => "plan-line",
        (None, None) => "-",
        (Some(task), _) => match task.status {
            TaskStatus::Draft | TaskStatus::ChangesRequested => "plan",
            TaskStatus::Review => "approve",
            TaskStatus::Ready | TaskStatus::InProgress => "implement",
            TaskStatus::Done => "land",
            _ => "-",
        },
    }
}

/// One shown item.
#[allow(clippy::too_many_arguments)]
fn item_row(
    state: &AutoState,
    git: &GitEnv,
    input: &specengine_core::check::CheckInput,
    lanes: &Lanes,
    switches: &Switches<'_>,
    pack: &Pack,
    item: &Item,
    now: &str,
) -> Result<ShownItem, CliError> {
    let task = item.task.as_ref().and_then(|id| state.tasks.get(id));
    let status = match (&item.task, task) {
        (None, _) => "unbound".to_owned(),
        (Some(_), Some(task)) => task.status.as_str().to_owned(),
        (Some(_), None) => "missing".to_owned(),
    };
    let lane = lanes.lane(task);
    let row = |act: &str, verdict: &str, reason: Option<String>, detail: Option<String>, commit| {
        ShownItem {
            item: item.name(pack.seq),
            task: item.task.clone(),
            line: item.line,
            text: item.text.clone(),
            status: status.clone(),
            act: act.to_owned(),
            verdict: verdict.to_owned(),
            reason,
            detail,
            commit,
            lane: lane.clone(),
        }
    };
    if let Some(landed) = &item.landed {
        return Ok(row("-", "landed", None, None, Some(landed.commit.clone())));
    }
    if let Some(held) = &item.held {
        return Ok(row(
            "-",
            "held",
            Some(held.reason.clone()),
            Some(held.detail.clone()),
            None,
        ));
    }
    if task.is_some_and(|task| task.status == TaskStatus::Cancelled) {
        return Ok(row("-", "cancelled", None, None, None));
    }
    let act = act_of(item, task);
    let facts_of_pack = match pack.facts() {
        Ok(facts) => facts,
        Err(why) => {
            let (verdict, reason, detail) = verdict_fields(&frozen_off(why));
            return Ok(row(act, &verdict, reason, detail, None));
        }
    };
    let verdict = match (act, task) {
        ("approve", Some(task)) => {
            let facts = task_facts(&state.context.project, input, task, Some(facts_of_pack))?;
            judge_approve(switches, &facts, now)
        }
        ("land", Some(task)) => {
            let facts = task_facts(&state.context.project, input, task, Some(facts_of_pack))?;
            match land_paths(&state.rule, task, &facts_of_pack.frozen.branch, git) {
                Ok(changed) => judge_land_paths(switches, &facts, &changed),
                Err(detail) => match judge_pack_act(switches, &facts, now, false) {
                    Verdict::Apply => Verdict::Hold {
                        reason: Reason::Place,
                        detail,
                    },
                    hold => hold,
                },
            }
        }
        _ => {
            // Past its `--until` a pack offers only its lands (`spec auto
            // next`): a plan, an implement, a line's plan held `until`.
            let facts = line_facts(item, task, pack.seq, facts_of_pack);
            judge_pack_act(switches, &facts, now, true)
        }
    };
    let (verdict, reason, detail) = verdict_fields(&verdict);
    Ok(row(act, &verdict, reason, detail, None))
}

/// The facts of an item judged by its pack alone (a plan, an implement, a
/// line).
pub(crate) fn line_facts<'a>(
    item: &Item,
    task: Option<&Task>,
    pack: i64,
    facts: PackFacts<'a>,
) -> TaskFacts<'a> {
    TaskFacts {
        id: item.name(pack),
        status: task.map_or(TaskStatus::Draft, |task| task.status),
        pack: Some(facts),
        paths: Vec::new(),
        ids: Vec::new(),
        unresolved: Vec::new(),
        last_run: None,
    }
}

// -------------------------------------------------------- stop, resume

/// `spec auto stop` and `spec auto resume` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoStopRequest {
    pub now: String,
    pub git: GitEnv,
}

#[derive(Serialize)]
struct StopJson<'a> {
    path: &'a str,
    stopped: bool,
    notes: &'a [String],
}

/// `spec auto stop`: the stop file made (see the module documentation).
/// No project is read; exit 2 when the data directory cannot be written.
pub fn auto_stop(env: &Env, request: &AutoStopRequest) -> Result<AutoOutcome, CliError> {
    run_stop(env, request).map_err(escaped_error)
}

fn run_stop(env: &Env, request: &AutoStopRequest) -> Result<AutoOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let path = stop_file(env)?;
    let shown = path.display().to_string();
    let done = |text: String| AutoOutcome {
        exit: Exit::Answered,
        text: escape_controls(&text),
        json: json_text(&StopJson {
            path: &shown,
            stopped: true,
            notes: &[],
        }),
        stderr: Vec::new(),
    };
    if fs::symlink_metadata(&path).is_ok() {
        let since = first_line(&path);
        return Ok(done(format!("stopped: {shown} (since {since})\n")));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|error| CliError::spec(format!("cannot create {}: {error}", dir.display())))?;
    }
    let by = WorktreeGit::new(&env.cwd, &request.git)
        .ok()
        .and_then(|git| git.committer_ident().ok())
        .unwrap_or_else(|| "unknown".to_owned());
    let text = format!("stopped {now} by {}\n", one_line(&by));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut file| {
            file.write_all(text.as_bytes())
                .and_then(|()| file.sync_all())
        });
    match written {
        Ok(()) => Ok(done(format!("stopped: {shown}\n"))),
        // Another stop made it meanwhile: it is there, unchanged.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let since = first_line(&path);
            Ok(done(format!("stopped: {shown} (since {since})\n")))
        }
        Err(error) => Err(CliError::spec(format!("cannot write {shown}: {error}"))),
    }
}

/// The stop file's first line as shown, never parsed.
fn first_line(path: &Path) -> String {
    fs::read(path)
        .ok()
        .map(|bytes| {
            let text = String::from_utf8_lossy(&bytes);
            one_line(text.lines().next().unwrap_or_default())
        })
        .unwrap_or_default()
}

/// `spec auto resume`: `main` has refused a stdin that is no terminal.
/// Absent: exit 1; else `consent` asks, `y` removes it.
pub fn auto_resume(
    env: &Env,
    consent: &mut dyn FnMut(&str) -> bool,
) -> Result<AutoOutcome, CliError> {
    run_resume(env, consent).map_err(escaped_error)
}

fn run_resume(env: &Env, consent: &mut dyn FnMut(&str) -> bool) -> Result<AutoOutcome, CliError> {
    let path = stop_file(env)?;
    let shown = path.display().to_string();
    if fs::symlink_metadata(&path).is_err() {
        return Ok(AutoOutcome::refused(format!(
            "not stopped: {shown} is absent; nothing changed"
        )));
    }
    let since = first_line(&path);
    let question = format!("resume auto mode on this machine, removing {shown} ({since})? [y/N]");
    if !consent(&escape_controls(&question)) {
        return Ok(AutoOutcome::refused(
            "not resumed: the answer was not `y`; nothing changed",
        ));
    }
    match fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(CliError::spec(format!("cannot remove {shown}: {error}"))),
    }
    Ok(AutoOutcome {
        exit: Exit::Answered,
        text: escape_controls(&format!("resumed: {shown} removed\n")),
        json: json_text(&StopJson {
            path: &shown,
            stopped: false,
            notes: &[],
        }),
        stderr: Vec::new(),
    })
}
