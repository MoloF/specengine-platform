//! The `spec` command line: pass 1 of the Phase 1 CLI (task spec
//! `spec-cli`), the agent read loop, `spec init`, `spec index`,
//! `spec search`, `spec show`; pass 2a.1 (task spec `spec-cli-check`),
//! `spec check` and `spec export index`; pass 2a.2 (task spec
//! `spec-cli-staged`), `spec check --staged` over the git index; task spec
//! `spec-cli-changed`, `spec check --changed`: the working tree against
//! `HEAD`; pass 3 (task spec `spec-cli-graph`), the graph reads: `spec
//! tree`, `spec graph` and `spec show --links`, over the index-fed spec
//! graph of core ([`specengine_core::check::SpecGraph`]); pass 4 (task spec
//! `spec-cli-bundle`), `spec bundle`: the context around named targets
//! within a budget of estimated tokens, named by its `bundle_hash`; task
//! spec `proposal-apply`, the proposal queue: `spec propose update`, `spec
//! inbox`, `spec review`, `spec approve` (the one write door: the target
//! file replaced in the proposal's recorded worktree and committed there;
//! a question's or a discrepancy's decision record created there, canon
//! `decision-record`, "Steps") and `spec reject`; the queue's backup (canon
//! `queue-backup`, "Commands"): `spec export state` and `spec import-state`; the agent
//! intake (canon `agent-intake`, "Tools"): `spec propose question`, `spec
//! propose discrepancy` and the `--brief` answers of `spec propose update`
//! and `spec review` (MCP's intake tools call these); task spec
//! `proposal-kinds`: `spec propose create`, a new spec file or new `{#ID}`
//! sections through the queue, applied as one commit.
//!
//! Every command lives here, below `main`: MCP stdio and the Phase 2 daemon
//! bridge call the same functions. `main.rs` only parses the arguments,
//! calls one command and prints what it gives.
//!
//! - [`discover`] finds the project root and reads its `specengine.toml`
//!   through core's one loader ([`ProjectConfig`]); `check` and
//!   `export index` read the whole config through the store's check loader
//!   instead; [`locate`] only finds the root and the config file, nothing
//!   read (its failure is "no project");
//! - [`data_dir`], [`db_path`], [`open_index`]: the index database lives
//!   outside the repository, one per project slug;
//! - [`init`], [`index`], [`search`], [`show`], [`tree`], [`graph`],
//!   [`bundle`], [`check`], [`export_index`]: one function per command,
//!   each giving its outcome or a [`CliError`];
//! - [`documents`]: the indexed live documents (neither `class: generated`
//!   nor Tier 3), by path (MCP's `resources/list`; no command prints it);
//! - [`tree_with_view`], [`show_with_view`], [`search_with_view`],
//!   [`graph_with_view`]: those reads bounded by a [`View`], the daemon's
//!   [`View::Browser`] never cut (task specs `daemon-read`, `ui-live`);
//!   [`project_entry`], [`events_after`]: a
//!   served project's entry and the queue's events after a `seq`, which
//!   only the daemon reads (no command prints them); [`EventsTail`] the
//!   latter with its connection kept between a live tail's polls;
//! - [`propose`], [`propose_create`], [`inbox`], [`review`], [`approve`],
//!   [`reject`]: the queue's commands; all but `inbox` answer with the
//!   review document
//!   ([`ProposalDocument`]); `approve` and `reject` take the owner's
//!   [`Consent`] (`main`: a terminal and a `[y/N]` prompt; [`approve_with`]
//!   takes a decision's [`ApproveFlags`]), and every
//!   request carries the caller's git environment ([`process_git`]) and,
//!   where the queue records a time, the clock's `now` ([`utc_now`]);
//!   [`propose_brief`], [`propose_create_brief`], [`review_brief`]: their
//!   brief answers;
//! - [`stage`], [`unstage`]: the owner's choice staged on an `open`
//!   proposal, or cleared (canon `decision-staging`; no command: the
//!   daemon's decision route calls them), confirmed only by `approve` or
//!   `reject` on a terminal ([`StageOutcome`]);
//! - [`propose_question`], [`propose_discrepancy`]: an agent's question or
//!   discrepancy stored as a queue record (approved into a decision record,
//!   or rejected with the answer), unless what is decided or asked already
//!   answers it ([`IntakeDocument`]);
//! - [`export_state`], [`import_state`]: the queue's dump ([`STATE_FORMAT`])
//!   written outside the worktree, and restored into an empty queue after
//!   the owner's [`Consent`];
//! - [`render_text`], [`render_json`]: an [`Outcome`] as stdout (JSON: one
//!   document, every key present, absent = `null`; `check`: the report's
//!   own JSON), bounded by [`OUTPUT_CAP_CHARS`] (`check`: unbounded;
//!   `bundle`: its body fitted within it, never cut);
//!   [`Outcome::stderr_lines`]: its `note:` and `warning:` lines;
//! - [`Exit`]: 0 answered, 1 not found (`show`, `tree`, `graph`, `bundle`),
//!   blocked (`check`) or refused by the proposal or its target (the queue's
//!   commands), 2 could not run (`check`: could not check).
//!
//! One database state gives one stdout, byte for byte: nothing depends on
//! time, storage order or the absolute root (`check`: one tree, config,
//! baseline and date; the queue's commands print stored times, never
//! relative ones, and their worktree's absolute path; `export state`: its
//! dump's bytes, and its default file name the injected clock's time). No
//! command writes
//! under the project root but `spec init`, which creates its one file,
//! `spec export index`, which writes only `[paths] index`, and `spec
//! approve`, which writes only the proposal's target file (or creates its
//! new file, or its decision record) and commits it in the recorded worktree; `index`, `search`, `show`, `tree`, `graph`,
//! `bundle`, `propose`, `inbox`, `review`, `reject` and `import-state` write
//! only the data directory (`propose question` and `propose discrepancy`
//! too), `export state` only its dump (never inside the worktree), `check`
//! nothing. Nothing found in the corpus is fatal
//! to the read commands: broken or unreadable files are indexed with their
//! diagnostics and never change an exit code.

mod apply;
mod bundle;
mod cap;
mod check;
mod corpus;
mod create;
mod decide;
mod documents;
mod events;
mod export;
mod graph;
mod inbox;
mod init;
mod intake;
mod links;
mod location;
mod package;
mod preflight;
mod project;
mod proposals;
mod propose;
mod refresh;
mod review;
mod search;
mod show;
mod stage;
mod state;
mod state_file;
mod task;
mod tree;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

pub use apply::{
    ApproveFlags, ApproveRequest, Consent, RejectRequest, approve, approve_with, reject,
};
pub use bundle::{
    BUNDLE_TAIL_LINES, Bundle, BundleItem, BundleOutcome, BundleRequest, DEFAULT_BUNDLE_BUDGET,
    ItemForm, TailEntry, WorkingAnswer, bundle, layer_heading, layer_key,
};
pub use cap::{OUTPUT_CAP_CHARS, SHOW_TAIL_NAMES, View};
pub use check::{CheckOutcome, CheckRequest, CheckedTree, check};
pub use corpus::LeftOut;
pub use create::{
    CreateRequest, propose_create, propose_create_brief, propose_create_brief_with_task,
    propose_create_with_task,
};
pub use documents::{DocumentEntry, documents};
pub use events::{EVENTS_PAGE_MAX, EventLine, EventsPage, EventsTail, events_after};
pub use export::{ExportIndexRequest, ExportOutcome, ShardOutcome, export_index};
pub use graph::{
    FollowedType, GraphEdge, GraphNode, GraphOutcome, GraphRequest, graph, graph_with_view,
};
pub use inbox::{INBOX_RATIONALE_CHARS, InboxEntry, InboxOutcome, InboxRequest, inbox};
pub use init::{InitOutcome, InitRequest, derive_slug, init};
pub use intake::{
    DiscrepancyRequest, INTAKE_INPUT_MAX_BYTES, INTAKE_MATCHES_MAX, IntakeDocument, IntakeMatch,
    IntakeOutcome, IntakeSource, MatchSource, QuestionRequest, propose_discrepancy,
    propose_discrepancy_with_task, propose_question, propose_question_with_task,
    read_discrepancy_input,
};
pub use links::{ShownLink, ShownLinks};
pub use location::{OpenIndex, data_dir, db_path, open_index};
pub use project::{
    CONFIG_FILE, Located, ProjectEntry, ProjectRoot, discover, locate, project_entry,
};
pub use proposals::{Preview, ProposalDocument, ProposalOutcome, QueueCommand};
pub use propose::{
    ProposeRequest, ProposedText, TEXT_MAX_BYTES, propose, propose_brief, propose_brief_with_task,
    propose_with_task,
};
pub use refresh::{IndexOutcome, IndexRequest, index};
pub use review::{ReviewRequest, review, review_brief};
pub use search::{HitCut, SearchOutcome, SearchRequest, search, search_with_view};
pub use show::{NestedSection, ShowOutcome, ShowRequest, ShownNode, show, show_with_view};
pub use specengine_core::ProjectConfig;
/// The intake's input types, enums and caps (core's), for the bridges;
/// the names of the kinds `propose_change` takes.
pub use specengine_core::intake::{
    ANSWER_MAX, CREATE_KIND, DISTINCT_ITEM_MAX, DISTINCT_MAX, DiscrepancyInput, EVIDENCE_MAX,
    EVIDENCE_TEXT_MAX, Evidence, GapType, IntakeOption, IntakeSeverity, LABEL_MAX, LINE_LIMIT,
    LOCATION_MAX, NODE_IDS_MAX, OPTION_TEXT_MAX, OPTIONS_MAX, OPTIONS_MIN, ProposedPatch,
    RATIONALE_MAX, SUMMARY_MAX, UPDATE_KIND,
};
/// The most bytes of an author's `role`, `model` or `run`.
pub use specengine_core::proposal::AUTHOR_FIELD_MAX;
/// The task's caps and the outcomes a run reports (core's).
pub use specengine_core::task::{
    AFFECTED_MAX, CHANGED_FILE_MAX, CHANGED_FILES_MAX, CRITERIA_MAX, CRITERION_MAX,
    CRITERION_TEXT_MAX, DEFAULT_TASK_BUDGET, DIFF_MAX, DIFFS_TOTAL_MAX, GOAL_MAX, NODES_MAX,
    NOTE_MAX, PACKAGE_BUDGET, PLAN_MAX, RUN_SUMMARY_MAX, SNAPSHOT_MAX, TITLE_MAX,
};
/// The task package and its parts (the model's), for the bridges.
pub use specengine_model::{RunOutcome, TaskPackage, TaskStatus};
/// The caller's git environment a queue request carries.
pub use specengine_store::GitEnv;
/// A staged choice as the review document carries it (the store's).
pub use specengine_store::Stage;
/// `search`'s `--limit` bounds and default, its shortest term (the store's).
pub use specengine_store::{
    MIN_TERM_CHARS, SEARCH_LIMIT_DEFAULT, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN,
};
/// A search hit's snippet as structure (the browser view's).
pub use specengine_store::{Snippet, SnippetSegment};
pub use stage::{
    StageBody, StageCause, StageOutcome, StageRequest, UnstageRequest, stage, unstage,
};
pub use state::{
    ExportStateOutcome, ExportStateRequest, ImportStateOutcome, ImportStateRequest, export_state,
    import_state,
};
pub use state_file::STATE_FORMAT;
pub use task::{
    TaskClaimRequest, TaskCompleteRequest, TaskDecisionRequest, TaskListEntry, TaskListOutcome,
    TaskListRequest, TaskNewRequest, TaskOutcome, TaskPlanRequest, TaskReportRequest,
    TaskShowOutcome, TaskShowRequest, task_approve, task_cancel, task_changes, task_claim,
    task_complete, task_list, task_new, task_plan, task_report, task_show,
};
pub use tree::{TreeMark, TreeNode, TreeOutcome, TreeRequest, tree, tree_with_view};

/// The exit code of a command (the verdict scheme of `spec check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Exit {
    /// The command answered, zero search hits included; `spec check`:
    /// clean or observed.
    Answered = 0,
    /// `spec show`, `spec tree`, `spec graph`, `spec bundle` found nothing: a dangling
    /// reference, no configured prefix, a `.md` path that is not indexed.
    /// `spec check`: blocked. The queue's commands: refused by the proposal
    /// or its target (nothing stored, written or committed).
    NotFound = 1,
    /// The command could not run: usage, project, config, environment,
    /// database. Nothing is printed on stdout, but `spec check`'s report
    /// of a check that could not vouch for the corpus.
    CannotRun = 2,
}

impl Exit {
    /// The process exit code.
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Why a command could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub exit: Exit,
    /// The whole stderr text: one line, `<config as given>:<line>: message`
    /// for a config error, else `spec: message`; `spec export index`'s
    /// config error: one such line per cause, joined by `\n`.
    pub message: String,
}

impl CliError {
    /// Exit 2 with `message` as written (already prefixed), on one line: a
    /// path or text it quotes cannot break it.
    pub fn cannot(message: impl Into<String>) -> Self {
        Self {
            exit: Exit::CannotRun,
            message: one_line(&message.into()),
        }
    }

    /// Exit 2 with several lines as written (already prefixed), each on
    /// one line.
    pub fn lines(lines: Vec<String>) -> Self {
        Self {
            exit: Exit::CannotRun,
            message: lines
                .iter()
                .map(|line| one_line(line))
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Exit 2, `spec: message`.
    pub(crate) fn spec(message: impl fmt::Display) -> Self {
        Self::cannot(format!("spec: {message}"))
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

/// What a command reads of its process: the current directory and the
/// variables that place the data directory. Passed in, so the bridge and
/// tests give their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    pub cwd: PathBuf,
    pub home: Option<OsString>,
    /// Read outside macOS only.
    pub xdg_data_home: Option<OsString>,
}

impl Env {
    /// The running process's current directory, `HOME` and `XDG_DATA_HOME`.
    pub fn from_process() -> Result<Self, CliError> {
        let cwd = std::env::current_dir().map_err(|error| {
            CliError::spec(format!("cannot read the current directory: {error}"))
        })?;
        Ok(Self {
            cwd,
            home: std::env::var_os("HOME"),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
        })
    }
}

/// The options every command takes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Globals {
    /// `--root DIR`: the project root, no walk up.
    pub root: Option<PathBuf>,
    /// `--config FILE`: read instead of `<root>/specengine.toml`; without
    /// `--root` the root is the current directory.
    pub config: Option<PathBuf>,
}

/// A `note:` or `warning:` line on stderr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Note(String),
    Warning(String),
}

impl Message {
    /// The stderr line: one line whatever path or text the message quotes.
    pub fn line(&self) -> String {
        match self {
            Self::Note(text) => format!("note: {}", one_line(text)),
            Self::Warning(text) => format!("warning: {}", one_line(text)),
        }
    }
}

/// What a command answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Init(InitOutcome),
    Index(IndexOutcome),
    Search(SearchOutcome),
    Show(ShowOutcome),
    Tree(TreeOutcome),
    Graph(GraphOutcome),
    Bundle(BundleOutcome),
    Check(CheckOutcome),
    Export(ExportOutcome),
    /// `propose`, `review`, `approve`, `reject`: the review document.
    Proposal(Box<ProposalOutcome>),
    /// `propose question`, `propose discrepancy`: the intake document.
    Intake(Box<IntakeOutcome>),
    /// [`stage`], [`unstage`] (no command; the daemon's): the review
    /// document, and why nothing was stored.
    Stage(Box<StageOutcome>),
    Inbox(InboxOutcome),
    /// `export state`: the dump written.
    StateExport(ExportStateOutcome),
    /// `import-state`: the rows restored, or the owner's refusal.
    StateImport(ImportStateOutcome),
    /// `task new|plan|approve|changes|claim|report|complete|cancel`.
    Task(Box<TaskOutcome>),
    /// `task show`: the package.
    TaskShow(Box<TaskShowOutcome>),
    /// `task list`.
    TaskList(TaskListOutcome),
}

impl Outcome {
    /// 0, or 1 for a `show`, `tree`, `graph` or `bundle` that found nothing
    /// or a refused queue command; `check`: its verdict's exit (1 blocked,
    /// 2 could not check).
    pub fn exit(&self) -> Exit {
        match self {
            Self::Show(show) if show.nodes.is_empty() => Exit::NotFound,
            Self::Tree(tree) if tree.reason.is_some() => Exit::NotFound,
            Self::Graph(graph) if graph.reason.is_some() => Exit::NotFound,
            Self::Bundle(bundle) if bundle.reason.is_some() => Exit::NotFound,
            Self::Check(check) => check.exit(),
            Self::Proposal(proposal) => proposal.exit(),
            Self::Intake(intake) => intake.exit(),
            Self::Stage(stage) => stage.exit(),
            Self::StateImport(import) => import.exit(),
            Self::Task(task) => task.exit(),
            Self::TaskShow(show) => show.exit(),
            _ => Exit::Answered,
        }
    }

    /// The stderr lines: notes and warnings in the order they arose, then,
    /// for a `show`, `tree`, `graph` or `bundle` that found nothing or a
    /// refused queue command, `spec: <reason>`.
    pub fn stderr_lines(&self) -> Vec<String> {
        let (messages, reason) = match self {
            Self::Init(outcome) => (&outcome.messages, None),
            Self::Index(outcome) => (&outcome.messages, None),
            Self::Search(outcome) => (&outcome.messages, None),
            Self::Show(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Tree(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Graph(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Bundle(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Check(outcome) => (&outcome.messages, None),
            Self::Export(outcome) => (&outcome.messages, None),
            Self::Proposal(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::Intake(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::Stage(outcome) => (
                &outcome.proposal.messages,
                outcome.proposal.refusal.as_deref(),
            ),
            Self::Inbox(outcome) => (&outcome.messages, None),
            Self::StateExport(outcome) => (&outcome.messages, None),
            Self::StateImport(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::Task(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::TaskShow(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::TaskList(outcome) => (&outcome.messages, None),
        };
        let mut lines: Vec<String> = messages.iter().map(Message::line).collect();
        if let Some(reason) = reason {
            lines.push(format!("spec: {}", one_line(reason)));
        }
        // The queue's commands quote agent-written text.
        if matches!(
            self,
            Self::Proposal(_)
                | Self::Intake(_)
                | Self::Stage(_)
                | Self::Inbox(_)
                | Self::StateExport(_)
                | Self::StateImport(_)
                | Self::Task(_)
                | Self::TaskShow(_)
                | Self::TaskList(_)
        ) {
            for line in &mut lines {
                *line = escape_controls(line);
            }
        }
        lines
    }
}

/// The outcome as stdout text; results only.
pub fn render_text(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Init(outcome) => init::render_text(outcome),
        Outcome::Index(outcome) => refresh::render_text(outcome),
        Outcome::Search(outcome) => search::render_text(outcome),
        Outcome::Show(outcome) => cap::render_text(outcome),
        Outcome::Tree(outcome) => tree::render_text(outcome),
        Outcome::Graph(outcome) => graph::render_text(outcome),
        Outcome::Bundle(outcome) => bundle::render_text(outcome),
        Outcome::Check(outcome) => check::render_text(outcome),
        Outcome::Export(outcome) => export::render_text(outcome),
        Outcome::Proposal(outcome) => proposals::render_text(outcome),
        Outcome::Intake(outcome) => intake::render_text(outcome),
        Outcome::Stage(outcome) => proposals::render_text(&outcome.proposal),
        Outcome::Inbox(outcome) => inbox::render_text(outcome),
        Outcome::StateExport(outcome) => state::render_export_text(outcome),
        Outcome::StateImport(outcome) => state::render_import_text(outcome),
        Outcome::Task(outcome) => task::render_text(outcome),
        Outcome::TaskShow(outcome) => task::render_show_text(outcome),
        Outcome::TaskList(outcome) => task::render_list_text(outcome),
    }
}

/// The command's own document, as [`render_json`] prints it (the `show`
/// cap applied; `check`: the report).
impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Init(outcome) => outcome.serialize(serializer),
            Self::Index(outcome) => outcome.serialize(serializer),
            Self::Search(outcome) => outcome.serialize(serializer),
            Self::Show(outcome) => outcome.serialize(serializer),
            Self::Tree(outcome) => outcome.serialize(serializer),
            Self::Graph(outcome) => outcome.serialize(serializer),
            Self::Bundle(outcome) => outcome.serialize(serializer),
            Self::Check(outcome) => outcome.report.serialize(serializer),
            Self::Export(outcome) => outcome.serialize(serializer),
            Self::Proposal(outcome) => outcome.serialize(serializer),
            Self::Intake(outcome) => outcome.serialize(serializer),
            Self::Stage(outcome) => outcome.serialize(serializer),
            Self::Inbox(outcome) => outcome.serialize(serializer),
            Self::StateExport(outcome) => outcome.serialize(serializer),
            Self::StateImport(outcome) => outcome.serialize(serializer),
            Self::Task(outcome) => outcome.serialize(serializer),
            Self::TaskShow(outcome) => outcome.serialize(serializer),
            Self::TaskList(outcome) => outcome.serialize(serializer),
        }
    }
}

/// The outcome as one compact JSON document and a line end; every key
/// present, absent = `null`. Every value is plain data (strings, numbers,
/// lists), which `serde_json` always encodes. `check`: the report's own
/// JSON, byte for byte (`Report::to_json`: unset keys absent).
pub fn render_json(outcome: &Outcome) -> String {
    if let Outcome::Check(outcome) = outcome {
        return check::render_json(outcome);
    }
    let mut json = serde_json::to_string(outcome).unwrap_or_else(|error| {
        serde_json::json!({ "error": format!("cannot encode the outcome: {error}") }).to_string()
    });
    json.push('\n');
    json
}

/// The clock of a queue request: now, UTC, `YYYY-MM-DDTHH:MM:SSZ` (`main`
/// and the MCP server; tests inject their own `now`).
pub fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        });
    specengine_core::proposal::utc_timestamp(seconds)
}

/// The git environment of a queue request from this process: `env`'s
/// directory and the process's variables (the library drops the local
/// `GIT_*` ones before running git in a proposal's worktree).
pub fn process_git(env: &Env) -> GitEnv {
    GitEnv::new(env.cwd.clone(), std::env::vars_os())
}

/// A store error: exit 2.
pub(crate) fn store_error(error: specengine_store::StoreError) -> CliError {
    CliError::spec(error)
}

/// A header or table field on one line: line breaks become spaces.
pub(crate) fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// `text` for a terminal: every C0 control character but LF, TAB and the
/// CR of a CRLF pair, DEL, every C1 control character and the Unicode
/// bidirectional marks, embeddings, overrides and isolates (U+061C,
/// U+200E, U+200F, U+202A–U+202E, U+2066–U+2069) written as `\u{XX}`
/// (lower-case hex), so agent-written text cannot move the cursor, erase,
/// recolour or reorder what the owner reads. Everything else is kept (a
/// CRLF file's lines read as they are).
pub(crate) fn escape_controls(text: &str) -> String {
    if !text.chars().any(is_escaped) {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let crlf = c == '\r' && chars.peek() == Some(&'\n');
        if is_escaped(c) && !crlf {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// A character [`escape_controls`] escapes (a CR followed by LF aside):
/// core's test, the one a decision record refuses too.
fn is_escaped(c: char) -> bool {
    specengine_core::record::is_escaped(c)
}
