//! `spec gate`: the task gate's verdict on one agent write
//! (canon `gate`, "Verdict", "Hook"; task spec `plugin-gate`). The plugin's
//! `PreToolUse` hook runs it through its wrapper with `--stdin`; `--path`
//! asks the same question from a terminal.
//!
//! Each step runs only when the one before decided nothing (canon
//! "Verdict"):
//!
//! 1. **Input**: the hook's JSON read to its end within the deadline, only
//!    `cwd` and `tool_input.file_path` (else `.notebook_path`) kept, every
//!    other value skipped unread ([`read_hook_input`]); a leading `~`
//!    expanded against `HOME`; the path joined to the session's directory
//!    when relative and folded. An input that names no file (not JSON, no
//!    path, a relative `cwd`, not closed in time, `~` without `HOME`) is
//!    decided by the session's project instead ([`GateRequest::session`]):
//!    no config, no `[gate]` or `off` pass; a gated or broken `[gate]` is no
//!    verdict (`observe`: a pass, noted). The hook input never refuses a
//!    write outside a gated project.
//! 2. **Candidates**: the path with its last component as written (its
//!    directory canonical), and the canonical path (symbolic links and the
//!    on-disk case resolved; a dangling final link followed with
//!    `read_link`, at most [`LINK_HOPS`] hops, else no verdict under a gated
//!    root). Each is judged on its own (steps 3-5); the write is controlled
//!    when either is.
//! 3. **Config**: the first `specengine.toml` walking up from the
//!    candidate's nearest existing directory (none: pass); unreadable: no
//!    verdict.
//! 4. **Table**: `[gate]` absent or `off`: pass; an error: no verdict.
//! 5. **Controlled?** (core [`GateConfig::controlled`]): a path whose last
//!    component is `specengine.toml` in any ASCII case, at any depth, or one
//!    in a listed zone.
//! 6. **Git**, reads only, every call bounded by the deadline
//!    ([`WorktreeGit::new_until`]), from the controlled candidate's
//!    directory.
//! 7. **Queue**, read-only ([`open_for_gate`]): an `in_progress` task
//!    claimed in the file's worktree on its branch opens (core [`opens`]);
//!    else a corrupt row of the repository is no verdict, `selective`
//!    refuses and `observe` passes and records `gate.observed`
//!    ([`record_gate_observed`], best-effort).
//!
//! Nothing else is read: no proposal, `stale`, check or index (the reads'
//! update never runs), no title or goal. Every failure is a
//! [`GateOutcome`], never a [`crate::CliError`]: exit 0 pass, 2 refuse or
//! no verdict ([`GATE_DEADLINE`] from the start, below the hook's
//! timeout); `observe` turns a no verdict past the table into a pass. What
//! the verdict knows so far is shared with a watchdog ([`GateProgress`]).

use std::fs;
use std::io::{self, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specengine_core::gate::{GateConfig, GateMode, GateTask, fold, mentions_gate, opens};
use specengine_model::TaskStatus;
use specengine_store::{
    GateObserved, GitEnv, GitError, QueueError, StoreError, WorktreeGit, open_for_gate,
    record_gate_observed, same_dir,
};

use crate::project::{CONFIG_FILE, find_config_dir};
use crate::{Env, data_dir};

/// How long a verdict may take from the start of the process: below the
/// hook's timeout, past which Claude Code lets the write through.
pub const GATE_DEADLINE: Duration = Duration::from_secs(3);

/// The most tasks a refusal lists.
pub const GATE_TASKS_MAX: usize = 8;

/// The most dangling symbolic links followed by hand, one after another.
pub const LINK_HOPS: usize = 16;

/// The hint of a version skew between `spec` and the plugin.
const REINSTALL: &str = "Reinstall spec and the plugin from one commit.";

/// One verdict asked for.
#[derive(Debug, Clone)]
pub struct GateRequest {
    pub input: GateInput,
    /// The session's project directory, which decides when the input names
    /// no file: `CLAUDE_PROJECT_DIR`, else the process's current directory.
    pub session: Option<PathBuf>,
    /// The process's variables for git (its directory is not used).
    pub git: GitEnv,
    /// The clock's UTC time stamp, for a `gate.observed` record.
    pub now: String,
    /// [`GATE_DEADLINE`] after the process started.
    pub deadline: Instant,
    /// What the verdict knows so far, shared with a watchdog.
    pub progress: GateProgress,
}

/// What names the written file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateInput {
    /// `--worktree DIR --path FILE`: `dir` the session's directory
    /// (relative: to the current one), a relative `path` joined to it.
    Path { dir: PathBuf, path: PathBuf },
    /// `--stdin`: the two fields of Claude Code's `PreToolUse` JSON the
    /// gate reads.
    Hook(HookInput),
    /// `--stdin` that could not be read or parsed: why; `deadline` when it
    /// was not closed in time.
    Unread { cause: String, deadline: bool },
}

/// The hook input's fields the gate reads, as written: `cwd`, and
/// `tool_input.file_path` else `tool_input.notebook_path` (a value that is
/// no string is absent).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookInput {
    pub cwd: Option<String>,
    pub file: Option<String>,
}

/// What a verdict in progress knows, from the least to the most closed; a
/// watchdog acts on it when the verdict stalls past the deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum GateStage {
    /// No config read yet.
    Unknown = 0,
    /// No config, no `[gate]` or `off`: a stall passes.
    Open = 1,
    /// `observe`: a stall passes, noted.
    Observe = 2,
    /// `selective`, or a `[gate]` that cannot be read: a stall is no
    /// verdict.
    Closed = 3,
}

/// The [`GateStage`] a verdict reached, shared between threads; it only
/// rises.
#[derive(Debug, Clone, Default)]
pub struct GateProgress(Arc<AtomicU8>);

impl GateProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// The most closed stage reached so far.
    pub fn stage(&self) -> GateStage {
        match self.0.load(Ordering::SeqCst) {
            1 => GateStage::Open,
            2 => GateStage::Observe,
            3 => GateStage::Closed,
            _ => GateStage::Unknown,
        }
    }

    fn raise(&self, stage: GateStage) {
        self.0.fetch_max(stage as u8, Ordering::SeqCst);
    }
}

/// The verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GateVerdict {
    Pass,
    Refuse,
    Unavailable,
}

/// Why the verdict is what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GateReason {
    /// Pass: no `specengine.toml` at or above the file.
    NoConfig,
    /// Pass: no `[gate]`, or `mode = "off"`.
    Off,
    /// Pass: the path is in no listed zone and is not the config.
    NotControlled,
    /// Pass: an `in_progress` task claimed here opens it.
    Claimed,
    /// Pass under `observe`: uncovered, or no verdict past the table.
    Observed,
    /// Refuse under `selective`: no task claimed here.
    NoClaim,
    /// No verdict: the input.
    Input,
    /// No verdict: the config.
    Config,
    /// No verdict: `HOME`.
    Home,
    /// No verdict: git.
    Git,
    /// No verdict: the queue (a newer schema, a corrupt row, unreadable).
    Queue,
    /// No verdict: the deadline passed, or the queue stayed locked.
    Deadline,
}

impl GateReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoConfig => "no-config",
            Self::Off => "off",
            Self::NotControlled => "not-controlled",
            Self::Claimed => "claimed",
            Self::Observed => "observed",
            Self::NoClaim => "no-claim",
            Self::Input => "input",
            Self::Config => "config",
            Self::Home => "home",
            Self::Git => "git",
            Self::Queue => "queue",
            Self::Deadline => "deadline",
        }
    }
}

/// A `ready` or `in_progress` task a refusal lists: its ID, state and
/// claim place, never its title or goal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateTaskEntry {
    pub id: String,
    pub status: TaskStatus,
    pub worktree: Option<String>,
    pub branch: Option<String>,
}

/// A verdict: the JSON document's keys in order, every one present
/// (absent: `null`), and the stderr lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateOutcome {
    pub verdict: GateVerdict,
    pub reason: GateReason,
    /// The mode in effect once the config is read.
    pub mode: Option<&'static str>,
    /// Root-relative once a root is found, else the folded absolute path.
    pub path: Option<String>,
    /// The project root, canonical.
    pub root: Option<String>,
    /// The zone that controls the path; `None` for the config file or an
    /// uncontrolled path.
    pub zone: Option<String>,
    /// The file's worktree top, once git was read.
    pub worktree: Option<String>,
    /// Its branch; `None` detached or unread.
    pub branch: Option<String>,
    /// The lowest-numbered task that opens the write.
    pub task: Option<String>,
    /// On a refusal only: the repository's `ready` and `in_progress`
    /// tasks by number, at most [`GATE_TASKS_MAX`].
    pub tasks: Vec<GateTaskEntry>,
    /// The no-verdict cause (also an `observe` pass's).
    pub cause: Option<String>,
    /// The stderr line of exit 2.
    #[serde(skip)]
    pub message: Option<String>,
    /// The `note:` lines of exit 0.
    #[serde(skip)]
    pub notes: Vec<String>,
}

impl GateOutcome {
    fn new() -> Self {
        Self {
            verdict: GateVerdict::Pass,
            reason: GateReason::NoConfig,
            mode: None,
            path: None,
            root: None,
            zone: None,
            worktree: None,
            branch: None,
            task: None,
            tasks: Vec::new(),
            cause: None,
            message: None,
            notes: Vec::new(),
        }
    }

    /// 0 pass, 2 refuse or no verdict: never another code.
    pub const fn exit_code(&self) -> u8 {
        match self.verdict {
            GateVerdict::Pass => 0,
            GateVerdict::Refuse | GateVerdict::Unavailable => 2,
        }
    }

    /// stderr: the message on 2, the `note:` lines on 0.
    pub fn stderr_lines(&self) -> Vec<String> {
        match &self.message {
            Some(message) if self.exit_code() != 0 => vec![message.clone()],
            _ => self.notes.clone(),
        }
    }

    /// `--path`'s stdout without `--json`: `pass: <reason>` on 0, nothing
    /// on 2.
    pub fn stdout_text(&self) -> String {
        match self.verdict {
            GateVerdict::Pass => format!("pass: {}\n", self.reason.as_str()),
            GateVerdict::Refuse | GateVerdict::Unavailable => String::new(),
        }
    }

    /// `--path --json`: one compact document, on 0 and on 2.
    pub fn to_json(&self) -> String {
        // Plain data: encoding it cannot fail; were it to, the document
        // still says no verdict.
        let mut text = serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"verdict":"unavailable","reason":"input","mode":null,"path":null,"root":null,"zone":null,"worktree":null,"branch":null,"task":null,"tasks":[],"cause":"the verdict could not be encoded"}"#.to_owned()
        });
        text.push('\n');
        text
    }

    fn pass(mut self, reason: GateReason) -> Self {
        self.verdict = GateVerdict::Pass;
        self.reason = reason;
        self
    }

    /// No verdict: exit 2 naming the cause, never the refusal's text.
    fn unavailable(mut self, reason: GateReason, cause: String, hint: Option<String>) -> Self {
        let cause = one_line(&cause);
        let mut message = format!("specengine gate: no verdict ({cause}): the write is refused.");
        if let Some(hint) = hint {
            message.push(' ');
            message.push_str(&hint);
        }
        self.verdict = GateVerdict::Unavailable;
        self.reason = reason;
        self.cause = Some(cause);
        self.message = Some(message);
        self.tasks.clear();
        self
    }

    /// No verdict past the table: `observe` passes, noting it; else
    /// [`Self::unavailable`].
    fn no_verdict(
        self,
        mode: GateMode,
        reason: GateReason,
        cause: String,
        hint: Option<String>,
    ) -> Self {
        if mode != GateMode::Observe {
            return self.unavailable(reason, cause, hint);
        }
        let cause = one_line(&cause);
        let mut outcome = self.pass(GateReason::Observed);
        outcome.notes.push(format!(
            "note: specengine gate (observe): no verdict ({cause}); the write goes on, nothing \
             recorded"
        ));
        outcome.cause = Some(cause);
        outcome
    }
}

/// Reads Claude Code's hook input from `reader` (the process's stdin) on
/// its own thread, to its end, keeping only `cwd` and
/// `tool_input.{file_path,notebook_path}` (every other value is skipped,
/// never held: a large `Write` costs no memory), and waits for it no later
/// than `deadline`. Whatever a parse error leaves unread is still read to
/// the end, so the writer never meets a closed pipe. Not closed in time,
/// unreadable or not the expected JSON: [`GateInput::Unread`] (a thread
/// still reading is left to the process's exit).
pub fn read_hook_input<R: Read + Send + 'static>(reader: R, deadline: Instant) -> GateInput {
    let (sender, receiver) = mpsc::channel();
    let spawned = thread::Builder::new().spawn(move || {
        let mut reader = BufReader::new(reader);
        let parsed = parse_hook(&mut reader);
        let drained = io::copy(&mut reader, &mut io::sink());
        let result = match (parsed, drained) {
            (Err(cause), _) => Err(cause),
            (Ok(_), Err(error)) => Err(format!("the hook input cannot be read: {error}")),
            (Ok(fields), Ok(_)) => Ok(fields),
        };
        let _ = sender.send(result);
    });
    if let Err(error) = spawned {
        return GateInput::Unread {
            cause: format!("the hook input cannot be read: {error}"),
            deadline: false,
        };
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(fields)) => GateInput::Hook(fields),
        Ok(Err(cause)) => GateInput::Unread {
            cause,
            deadline: false,
        },
        Err(_) => GateInput::Unread {
            cause: format!(
                "the hook input was not read to its end within {} s",
                GATE_DEADLINE.as_secs()
            ),
            deadline: true,
        },
    }
}

/// The hook input's two fields, the rest skipped as it streams by.
fn parse_hook(reader: &mut impl Read) -> Result<HookInput, String> {
    let not_json =
        |error: serde_json::Error| format!("the hook input is not the expected JSON: {error}");
    let mut deserializer = serde_json::Deserializer::from_reader(reader);
    let raw = RawHook::deserialize(&mut deserializer).map_err(not_json)?;
    deserializer.end().map_err(not_json)?;
    let text = |value: Option<Value>| match value {
        Some(Value::String(text)) => Some(text),
        _ => None,
    };
    let tool = raw.tool_input.unwrap_or_default();
    Ok(HookInput {
        cwd: text(raw.cwd),
        file: text(tool.file_path).or_else(|| text(tool.notebook_path)),
    })
}

/// The hook input as the gate reads it; every other key is skipped.
#[derive(Deserialize)]
struct RawHook {
    #[serde(default)]
    cwd: Option<Value>,
    #[serde(default)]
    tool_input: Option<RawTool>,
}

#[derive(Deserialize, Default)]
struct RawTool {
    #[serde(default)]
    file_path: Option<Value>,
    #[serde(default)]
    notebook_path: Option<Value>,
}

/// The verdict on `request` (see the module documentation).
pub fn gate(env: &Env, request: &GateRequest) -> GateOutcome {
    let mut outcome = GateOutcome::new();
    let progress = &request.progress;
    let session = request.session.as_deref();

    // 1. Input.
    let file = match input_path(env, &request.input) {
        Ok(file) => file,
        Err((reason, cause)) => return by_session(outcome, session, reason, cause, progress),
    };
    let folded = fold(&file);
    outcome.path = Some(folded.to_string_lossy().into_owned());
    if past(request.deadline) {
        return by_session(
            outcome,
            session,
            GateReason::Deadline,
            late("the path"),
            progress,
        );
    }

    // 2.-5. Each candidate on its own.
    let written = as_written(&folded);
    let canonical = match canonical(&folded) {
        Ok(canonical) => canonical,
        Err(cause) => return unfollowed(outcome, classify(&written, progress), cause),
    };
    let mut classes = vec![classify(&canonical, progress)];
    if written.path != canonical.path {
        classes.push(classify(&written, progress));
    }
    // A no verdict that closes first, then a controlled candidate, then
    // an `observe` no verdict, then the canonical path's pass.
    let closing = classes.iter().position(
        |class| matches!(class, Class::NoVerdict { mode, .. } if *mode != Some(GateMode::Observe)),
    );
    let controlled = classes
        .iter()
        .position(|class| matches!(class, Class::Controlled(_)));
    let observed = classes
        .iter()
        .position(|class| matches!(class, Class::NoVerdict { .. }));
    let chosen = closing.or(controlled).or(observed).unwrap_or(0);
    match classes.swap_remove(chosen) {
        Class::Controlled(chosen) => decide(outcome, env, request, *chosen),
        class => settle(outcome, class),
    }
}

/// The verdict of the session's project alone, for a stall past the
/// deadline before any config was read (the watchdog's): no config, no
/// `[gate]` or `off` pass; `observe` passes, noted; else no verdict.
pub fn gate_session(session: Option<&Path>, cause: String, progress: &GateProgress) -> GateOutcome {
    by_session(
        GateOutcome::new(),
        session,
        GateReason::Deadline,
        cause,
        progress,
    )
}

/// An input that names no file, decided by the session's project.
fn by_session(
    mut outcome: GateOutcome,
    session: Option<&Path>,
    reason: GateReason,
    cause: String,
    progress: &GateProgress,
) -> GateOutcome {
    outcome.path = None;
    let Some(dir) = session else {
        progress.raise(GateStage::Open);
        return outcome.pass(GateReason::NoConfig);
    };
    let dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    match load(&dir, progress) {
        Loaded::NoConfig => outcome.pass(GateReason::NoConfig),
        Loaded::Off { root } => {
            outcome.root = Some(display(&root));
            off(outcome)
        }
        Loaded::Broken {
            root,
            cause: broken,
            hint,
        } => {
            outcome.root = Some(display(&root));
            outcome.unavailable(
                reason,
                format!(
                    "{cause}; the session's project has a gate config that cannot be read: {broken}"
                ),
                Some(hint),
            )
        }
        Loaded::Gated { root, config, .. } => {
            outcome.root = Some(display(&root));
            outcome.mode = Some(config.mode.as_str());
            outcome.no_verdict(
                config.mode,
                reason,
                format!("{cause}; the session's project {} is gated", root.display()),
                None,
            )
        }
    }
}

/// The final link could not be followed: the path as written decides
/// whether that matters (under a gated root it is no verdict).
fn unfollowed(outcome: GateOutcome, written: Class, cause: String) -> GateOutcome {
    match written {
        Class::Pass {
            reason: GateReason::NoConfig | GateReason::Off,
            ..
        }
        | Class::NoVerdict { .. } => settle(outcome, written),
        Class::Pass {
            root, path, mode, ..
        } => settle(
            outcome,
            Class::NoVerdict {
                root,
                path,
                mode,
                reason: GateReason::Input,
                cause,
                hint: None,
            },
        ),
        Class::Controlled(chosen) => settle(
            outcome,
            Class::NoVerdict {
                root: Some(chosen.root),
                path: chosen.rel,
                mode: Some(chosen.config.mode),
                reason: GateReason::Input,
                cause,
                hint: None,
            },
        ),
    }
}

/// One candidate judged through the config and the controlled check.
enum Class {
    /// Nothing controls it.
    Pass {
        reason: GateReason,
        root: Option<PathBuf>,
        /// Root-relative under a root, else absolute.
        path: String,
        mode: Option<GateMode>,
    },
    /// No verdict (under `observe`: a pass, noted).
    NoVerdict {
        root: Option<PathBuf>,
        path: String,
        mode: Option<GateMode>,
        reason: GateReason,
        cause: String,
        hint: Option<String>,
    },
    /// Controlled: git and the queue decide.
    Controlled(Box<Chosen>),
}

/// A controlled candidate.
struct Chosen {
    root: PathBuf,
    label: String,
    config: GateConfig,
    rel: String,
    zone: Option<String>,
    /// Its nearest existing directory: where git runs.
    dir: PathBuf,
}

fn classify(candidate: &Resolved, progress: &GateProgress) -> Class {
    let absolute = display(&candidate.path);
    let (root, label, config) = match load(&candidate.dir, progress) {
        Loaded::NoConfig => {
            return Class::Pass {
                reason: GateReason::NoConfig,
                root: None,
                path: absolute,
                mode: None,
            };
        }
        Loaded::Off { root } => {
            let path = relative(&candidate.path, &root).unwrap_or(absolute);
            return Class::Pass {
                reason: GateReason::Off,
                root: Some(root),
                path,
                mode: Some(GateMode::Off),
            };
        }
        Loaded::Broken { root, cause, hint } => {
            let path = relative(&candidate.path, &root).unwrap_or(absolute);
            return Class::NoVerdict {
                root: Some(root),
                path,
                mode: None,
                reason: GateReason::Config,
                cause,
                hint: Some(hint),
            };
        }
        Loaded::Gated {
            root,
            label,
            config,
        } => (root, label, config),
    };
    let mode = Some(config.mode);
    let Some(rel) = relative(&candidate.path, &root) else {
        return Class::NoVerdict {
            cause: format!("{absolute} is no UTF-8 path under {}", root.display()),
            root: Some(root),
            path: absolute,
            mode,
            reason: GateReason::Input,
            hint: None,
        };
    };
    match config.controlled(&rel, CONFIG_FILE) {
        None => Class::Pass {
            reason: GateReason::NotControlled,
            root: Some(root),
            path: rel,
            mode,
        },
        Some(control) => Class::Controlled(Box::new(Chosen {
            root,
            label,
            config,
            rel,
            zone: control.zone,
            dir: candidate.dir.clone(),
        })),
    }
}

/// A candidate's class that needs neither git nor the queue, as the
/// outcome.
fn settle(mut outcome: GateOutcome, class: Class) -> GateOutcome {
    match class {
        Class::Pass {
            reason,
            root,
            path,
            mode,
        } => {
            outcome.root = root.as_deref().map(display);
            outcome.path = Some(path);
            outcome.mode = mode.map(GateMode::as_str);
            outcome.pass(reason)
        }
        Class::NoVerdict {
            root,
            path,
            mode,
            reason,
            cause,
            hint,
        } => {
            outcome.root = root.as_deref().map(display);
            outcome.path = Some(path);
            outcome.mode = mode.map(GateMode::as_str);
            match mode {
                Some(mode) => outcome.no_verdict(mode, reason, cause, hint),
                None => outcome.unavailable(reason, cause, hint),
            }
        }
        Class::Controlled(chosen) => {
            // Not reached: `gate` sends a controlled candidate to `decide`.
            outcome.root = Some(display(&chosen.root));
            outcome.path = Some(chosen.rel);
            outcome.unavailable(
                GateReason::Input,
                "a controlled path reached no verdict".to_owned(),
                None,
            )
        }
    }
}

/// A config as the gate reads it.
enum Loaded {
    NoConfig,
    /// No `[gate]`, `off`, or a file that is not TOML and names no
    /// `[gate]`.
    Off {
        root: PathBuf,
    },
    /// Unreadable, or a `[gate]` error: no verdict.
    Broken {
        root: PathBuf,
        cause: String,
        hint: String,
    },
    /// `observe` or `selective`.
    Gated {
        root: PathBuf,
        label: String,
        config: GateConfig,
    },
}

/// The first config at or above `dir`, read and its `[gate]` checked; the
/// stage it shows is raised in `progress`.
fn load(dir: &Path, progress: &GateProgress) -> Loaded {
    let Some(root) = find_config_dir(dir) else {
        progress.raise(GateStage::Open);
        return Loaded::NoConfig;
    };
    let file = root.join(CONFIG_FILE);
    let label = display(&file);
    let broken = |cause: String, hint: String| {
        progress.raise(GateStage::Closed);
        Loaded::Broken {
            root: root.clone(),
            cause,
            hint,
        }
    };
    let text = match fs::read(&file) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                if mentions_gate(&String::from_utf8_lossy(error.as_bytes())) {
                    return broken(
                        format!("{label}: the file is not UTF-8, and a line of it names `[gate]`"),
                        format!("Fix {label}."),
                    );
                }
                progress.raise(GateStage::Open);
                return Loaded::Off { root };
            }
        },
        Err(error) => {
            return broken(
                format!("{label}: {error}"),
                format!("Make {label} readable."),
            );
        }
    };
    match GateConfig::from_toml(&text) {
        Ok(Some(config)) if config.mode != GateMode::Off => {
            progress.raise(if config.mode == GateMode::Observe {
                GateStage::Observe
            } else {
                GateStage::Closed
            });
            Loaded::Gated {
                root,
                label,
                config,
            }
        }
        Ok(_) => {
            progress.raise(GateStage::Open);
            Loaded::Off { root }
        }
        Err(error) => {
            let hint = match error.line {
                Some(line) => format!("Fix {label}:{line}."),
                None => format!("Fix {label}."),
            };
            broken(error.at(&label), hint)
        }
    }
}

/// Steps 6 and 7 for the controlled candidate.
fn decide(
    mut outcome: GateOutcome,
    env: &Env,
    request: &GateRequest,
    chosen: Chosen,
) -> GateOutcome {
    let deadline = request.deadline;
    let Chosen {
        root,
        label,
        config,
        rel,
        zone,
        dir,
    } = chosen;
    let mode = config.mode;
    outcome.root = Some(display(&root));
    outcome.path = Some(rel.clone());
    outcome.mode = Some(mode.as_str());
    outcome.zone = zone;
    if past(deadline) {
        return outcome.no_verdict(mode, GateReason::Deadline, late("the config"), None);
    }

    // 6. Git.
    let place = match place(&dir, &request.git, deadline) {
        Ok(place) => place,
        Err(error) => {
            let reason = match error {
                GitError::Deadline => GateReason::Deadline,
                _ => GateReason::Git,
            };
            let cause = format!("git in {}: {error}", dir.display());
            return outcome.no_verdict(mode, reason, cause, None);
        }
    };
    outcome.worktree = Some(display(&place.top));
    outcome.branch.clone_from(&place.branch);

    // 7. Queue.
    let Some(slug) = config.slug.as_deref() else {
        return outcome.no_verdict(
            mode,
            GateReason::Config,
            format!("{label}: no `[project] slug`"),
            None,
        );
    };
    let db = match data_dir(env) {
        Ok(dir) => dir.join(format!("{slug}.db")),
        Err(error) => {
            let cause = error
                .message
                .strip_prefix("spec: ")
                .unwrap_or(&error.message)
                .to_owned();
            return outcome.no_verdict(mode, GateReason::Home, cause, None);
        }
    };
    let read = open_for_gate(&db, slug, deadline).and_then(|queue| match queue {
        Some(queue) => queue.tasks(&place.common),
        None => Ok((Vec::new(), Vec::new())),
    });
    let (tasks, corrupt) = match read {
        Ok(read) => read,
        Err(error) => {
            let (reason, cause, hint) = queue_failure(&db, &error);
            return outcome.no_verdict(mode, reason, cause, hint);
        }
    };
    let top = place.top.clone();
    if let Some(task) = opens(&tasks, place.branch.as_deref(), |worktree| {
        same_dir(Path::new(worktree), &top)
    }) {
        outcome.task = Some(task.id.clone());
        return outcome.pass(GateReason::Claimed);
    }
    if let Some(row) = corrupt.first() {
        return outcome.no_verdict(
            mode,
            GateReason::Queue,
            row.to_string(),
            Some(format!("Run `spec task show {}` on a terminal.", row.id)),
        );
    }
    if past(deadline) {
        return outcome.no_verdict(mode, GateReason::Deadline, late("the queue"), None);
    }

    // Uncovered.
    match mode {
        GateMode::Observe => observe(outcome, &db, slug, &rel, &place, request),
        GateMode::Off | GateMode::Selective => refuse(outcome, mode, &rel, &place, &tasks),
    }
}

/// `observe`: pass, and record the uncovered write; a failure is a note.
fn observe(
    mut outcome: GateOutcome,
    db: &Path,
    slug: &str,
    rel: &str,
    place: &Place,
    request: &GateRequest,
) -> GateOutcome {
    let observed = GateObserved {
        path: rel.to_owned(),
        zone: outcome.zone.clone(),
        worktree: place.top.to_string_lossy().into_owned(),
        branch: place.branch.clone(),
    };
    let what = match &observed.zone {
        Some(zone) => format!("{rel} in the zone `{zone}`"),
        None => format!("{rel} (a project config)"),
    };
    let failure = match record_gate_observed(db, slug, &observed, &request.now, request.deadline) {
        Ok(true) => None,
        Ok(false) => Some(format!(
            "no queue at this build's schema in {}",
            db.display()
        )),
        Err(error) => Some(queue_failure(db, &error).1),
    };
    if let Some(why) = failure {
        outcome.notes.push(format!(
            "note: specengine gate (observe): {what} is not covered by a claimed task and was not \
             recorded ({}); the write goes on",
            one_line(&why)
        ));
    }
    outcome.pass(GateReason::Observed)
}

/// `selective`: refuse, naming the path, zone, mode, the next step and
/// the repository's open tasks by ID and state.
fn refuse(
    mut outcome: GateOutcome,
    mode: GateMode,
    rel: &str,
    place: &Place,
    tasks: &[GateTask],
) -> GateOutcome {
    let open: Vec<&GateTask> = tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Ready | TaskStatus::InProgress))
        .collect();
    outcome.tasks = open
        .iter()
        .take(GATE_TASKS_MAX)
        .map(|task| GateTaskEntry {
            id: task.id.clone(),
            status: task.status,
            worktree: task.claim.as_ref().map(|claim| claim.worktree.clone()),
            branch: task.claim.as_ref().map(|claim| claim.branch.clone()),
        })
        .collect();
    let what = match &outcome.zone {
        Some(zone) => format!("{rel} is in the zone `{zone}`"),
        None => format!("{rel} would be a project config"),
    };
    let top = place.top.display();
    let here = match &place.branch {
        Some(branch) => format!("no in_progress task is claimed in {top} on `{branch}`"),
        None => format!("nothing opens on the detached HEAD of {top} (a claim names a branch)"),
    };
    let listed = if outcome.tasks.is_empty() {
        "none ready or in_progress".to_owned()
    } else {
        let mut items: Vec<String> = outcome
            .tasks
            .iter()
            .map(|task| match (&task.worktree, &task.branch) {
                (Some(worktree), Some(branch)) => format!(
                    "{} {}, claimed in {worktree} on `{branch}`",
                    task.id, task.status
                ),
                _ => format!("{} {}", task.id, task.status),
            })
            .collect();
        if open.len() > GATE_TASKS_MAX {
            items.push(format!("and {} more", open.len() - GATE_TASKS_MAX));
        }
        items.join("; ")
    };
    outcome.verdict = GateVerdict::Refuse;
    outcome.reason = GateReason::NoClaim;
    outcome.message = Some(one_line(&format!(
        "specengine gate ({mode}): {what} and {here}. Tasks here: {listed}. Claim a ready task in \
         this worktree with claim_task, or ask the owner to approve one (`spec task approve T` on \
         a terminal). Nothing was written."
    )));
    outcome
}

/// No `[gate]`, `off`, or a file that is not TOML naming no `[gate]`.
fn off(mut outcome: GateOutcome) -> GateOutcome {
    outcome.mode = Some(GateMode::Off.as_str());
    outcome.pass(GateReason::Off)
}

/// The file's absolute path from the input, else why not (the session's
/// project then decides).
fn input_path(env: &Env, input: &GateInput) -> Result<PathBuf, (GateReason, String)> {
    let input_error = |cause: String| (GateReason::Input, cause);
    match input {
        GateInput::Unread { cause, deadline } => Err((
            if *deadline {
                GateReason::Deadline
            } else {
                GateReason::Input
            },
            cause.clone(),
        )),
        GateInput::Path { dir, path } => {
            let dir = if dir.is_absolute() {
                dir.clone()
            } else if env.cwd.is_absolute() {
                env.cwd.join(dir)
            } else {
                return Err(input_error(format!(
                    "--worktree {} is relative and the current directory cannot be read",
                    dir.display()
                )));
            };
            if path.as_os_str().is_empty() {
                return Err(input_error("--path is empty".to_owned()));
            }
            let path = match path.to_str() {
                Some(text) => expand_home(env, text).map_err(input_error)?,
                None => path.clone(),
            };
            Ok(dir.join(path))
        }
        GateInput::Hook(hook) => {
            let file = hook
                .file
                .as_deref()
                .filter(|file| !file.is_empty())
                .ok_or_else(|| {
                    input_error(
                        "the hook input names no `tool_input.file_path` or \
                         `tool_input.notebook_path`"
                            .to_owned(),
                    )
                })?;
            if file.contains('\0') {
                return Err(input_error(
                    "the hook input's file path holds NUL".to_owned(),
                ));
            }
            let file = expand_home(env, file).map_err(input_error)?;
            if file.is_absolute() {
                return Ok(file);
            }
            match hook.cwd.as_deref() {
                Some(cwd) if Path::new(cwd).is_absolute() => Ok(Path::new(cwd).join(file)),
                Some(cwd) => Err(input_error(format!(
                    "the hook input's `cwd` {cwd:?} is not absolute"
                ))),
                None => Err(input_error(format!(
                    "the hook input's file path {} is relative and it has no `cwd`",
                    file.display()
                ))),
            }
        }
    }
}

/// `text` with a leading `~` or `~/` expanded against `HOME`; `~user` is
/// left as written.
fn expand_home(env: &Env, text: &str) -> Result<PathBuf, String> {
    let rest = match text.strip_prefix('~') {
        Some("") => "",
        Some(rest) if rest.starts_with('/') => rest.trim_start_matches('/'),
        _ => return Ok(PathBuf::from(text)),
    };
    match env.home.as_deref().map(Path::new) {
        Some(home) if home.is_absolute() => Ok(home.join(rest)),
        _ => Err(format!(
            "the file path {text:?} starts with `~`, and HOME is not an absolute path"
        )),
    }
}

/// A folded path resolved on disk: its nearest existing ancestor
/// canonical (symbolic links, the on-disk case), the rest appended.
struct Resolved {
    path: PathBuf,
    /// The nearest existing directory: where the config walk and git
    /// start.
    dir: PathBuf,
}

fn resolve(folded: &Path) -> Resolved {
    let mut rest = Vec::new();
    let mut at = folded;
    loop {
        if let Ok(canonical) = fs::canonicalize(at) {
            let dir = if canonical.is_dir() {
                canonical.clone()
            } else {
                canonical
                    .parent()
                    .map_or_else(|| canonical.clone(), Path::to_path_buf)
            };
            let mut path = canonical;
            for name in rest.iter().rev() {
                path.push(name);
            }
            return Resolved { path, dir };
        }
        match (at.file_name(), at.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_owned());
                at = parent;
            }
            _ => {
                return Resolved {
                    path: folded.to_path_buf(),
                    dir: folded.to_path_buf(),
                };
            }
        }
    }
}

/// `path` relative to `root`, `/`-joined; `None` outside it or not UTF-8.
fn relative(path: &Path, root: &Path) -> Option<String> {
    let inner = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in inner.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// The file's worktree as git reads it.
struct Place {
    /// Canonical.
    top: PathBuf,
    common: PathBuf,
    /// `None`: detached.
    branch: Option<String>,
}

fn place(dir: &Path, env: &GitEnv, deadline: Instant) -> Result<Place, GitError> {
    let git = WorktreeGit::new_until(dir, env, deadline)?;
    let (top, common) = git.top_and_common_dir()?;
    Ok(Place {
        top,
        common,
        branch: git.branch()?,
    })
}

/// A queue failure's reason, cause and hint.
fn queue_failure(db: &Path, error: &QueueError) -> (GateReason, String, Option<String>) {
    match error {
        QueueError::Store(StoreError::Busy) => (
            GateReason::Deadline,
            format!(
                "the queue {} stayed locked past the deadline ({} s)",
                db.display(),
                GATE_DEADLINE.as_secs()
            ),
            None,
        ),
        QueueError::SchemaTooNew { .. } => (
            GateReason::Queue,
            format!("the queue {}: {error}", db.display()),
            Some(REINSTALL.to_owned()),
        ),
        // The path is in the message already.
        QueueError::Store(StoreError::Io { .. }) => {
            (GateReason::Queue, format!("the queue: {error}"), None)
        }
        _ => (
            GateReason::Queue,
            format!("the queue {}: {error}", db.display()),
            None,
        ),
    }
}

fn past(deadline: Instant) -> bool {
    Instant::now() >= deadline
}

fn late(what: &str) -> String {
    format!("no answer within {} s, at {what}", GATE_DEADLINE.as_secs())
}

fn one_line(text: &str) -> String {
    crate::one_line(text)
}

/// The path with its last component as written, its directory resolved:
/// a final symbolic link not followed.
fn as_written(folded: &Path) -> Resolved {
    match (folded.parent(), folded.file_name()) {
        (Some(parent), Some(name)) => {
            let directory = resolve(parent);
            Resolved {
                path: directory.path.join(name),
                dir: directory.dir,
            }
        }
        _ => resolve(folded),
    }
}

/// The canonical path: [`resolve`], a dangling final link followed by
/// hand (`read_link`, relative to the link's directory), at most
/// [`LINK_HOPS`] hops; else why not.
fn canonical(folded: &Path) -> Result<Resolved, String> {
    let mut at = folded.to_path_buf();
    for _ in 0..LINK_HOPS {
        let link = fs::symlink_metadata(&at).is_ok_and(|meta| meta.file_type().is_symlink());
        if !link || fs::metadata(&at).is_ok() {
            return Ok(resolve(&at));
        }
        let target = fs::read_link(&at).map_err(|error| {
            format!("the symbolic link {} cannot be read: {error}", at.display())
        })?;
        let parent = at.parent().map(Path::to_path_buf).unwrap_or_default();
        let base = fs::canonicalize(&parent).unwrap_or(parent);
        at = fold(&base.join(target));
    }
    Err(format!(
        "{} is a chain of more than {LINK_HOPS} dangling symbolic links",
        folded.display()
    ))
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
