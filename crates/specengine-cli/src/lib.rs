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
//! the decision record of a question, a discrepancy or a decision created
//! there, canon `decision-record`, "Steps") and `spec reject`; the queue's backup (canon
//! `queue-backup`, "Commands"): `spec export state` and `spec import-state`; the agent
//! intake (canon `agent-intake`, "Tools"): `spec propose question`, `spec
//! propose discrepancy`, `spec propose decision` (task spec
//! `kind-decision`) and the `--brief` answers of `spec propose update`
//! and `spec review` (MCP's intake tools call these); task spec
//! `proposal-kinds`: `spec propose create`, a new spec file or new `{#ID}`
//! sections through the queue, applied as one commit; task spec
//! `kind-amendment`: `spec propose amendment`, a new spec or decision
//! document amending live nodes, applied as a create's new file; task spec
//! `health-left`: `spec health`, the unapplied amendments of the working
//! tree; `spec lock accept` ([`lock_accept`], the accept, its editorial
//! refresh and a gone node's drop, one function taking the owner's
//! [`LockConsent`] and extra [`LockTrailer`]s: canon
//! `spec-lock-accept`, "Steps"), `spec lock regen` ([`lock_regen`]:
//! canon `spec-lock`, "`spec lock regen`") and `spec lock rebase`
//! ([`lock_rebase`]: task spec `lock-rebase`); `spec verify [--changed |
//! --staged]` ([`verify`]: canon `spec-verify`, "Rows", "Verdict and
//! exit"; `--task T [--changed]`: task spec `verify-task`; `--node REF`:
//! task spec `verify-node`), drift between
//! `spec.lock` and the tree, read-only; task spec
//! `drift-queue`: `spec propose drift` ([`propose_drift`]), the drift of
//! `HEAD`'s commit queued as `drift` items for the owner, approved by the
//! same lock accept; task spec `rev-rule`: `spec bump` ([`bump`]), the
//! nodes changed without a raise, read-only (the rule `rev-not-raised`
//! itself runs inside `spec check --staged` and `--changed`).
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
//! - [`health`]: the working tree's unapplied amendments (live documents
//!   that name a node they amend) over the walk `check` reads, no index;
//!   then rows 4-5 ([`CodeRows`]: `@assumes` and queue markers, unbound
//!   nodes) over the code index the call refreshes, as `refs` reads it;
//! - [`documents`]: the indexed live documents (neither `class: generated`
//!   nor Tier 3), by path (MCP's `resources/list`; no command prints it);
//! - [`tree_with_view`], [`show_with_view`], [`search_with_view`],
//!   [`graph_with_view`], [`roadmap_with_view`]: those reads bounded by a
//!   [`View`], the daemon's [`View::Browser`] never cut (task specs
//!   `daemon-read`, `ui-live`, `roadmap-surfaces`);
//!   [`project_entry`], [`events_after`]: a
//!   served project's entry and the queue's events after a `seq`, which
//!   only the daemon reads (no command prints them); [`EventsTail`] the
//!   latter with its connection kept between a live tail's polls;
//! - [`propose`], [`propose_create`], [`inbox`], [`review`], [`approve`],
//!   [`reject`]: the queue's commands; all but `inbox` answer with the
//!   review document
//!   ([`ProposalDocument`]); `approve` and `reject` take the owner's
//!   [`Consent`] (`main`: a terminal and a `[y/N]` prompt; [`approve_with`]
//!   takes a decision's [`ApproveFlags`], [`approve_editorial`]
//!   `--editorial`; an apply raises
//!   the revision of each node whose meaning it changes, or keeps it and
//!   refreshes the vouching `spec.lock` entries in its commit: task spec
//!   `rev-discipline`; [`reject_follow_up`] a drift item's reject making a
//!   draft task, [`FollowUp`]: task spec `drift-follow-up`), and every
//!   request carries the caller's git environment ([`process_git`]) and,
//!   where the queue records a time, the clock's `now` ([`utc_now`]);
//!   [`propose_brief`], [`propose_create_brief`], [`review_brief`]: their
//!   brief answers;
//! - [`stage`], [`unstage`]: the owner's choice staged on an `open`
//!   proposal, or cleared (canon `decision-staging`; no command: the
//!   daemon's decision route calls them), confirmed only by `approve` or
//!   `reject` on a terminal ([`StageOutcome`]);
//! - [`round_new`], [`round_answer`], [`round_stage`]: a question round
//!   (canon `round`): the open questions, discrepancies and decisions on
//!   one read-only sheet ([`RoundSheet`]); the returned answers checked,
//!   staged and each confirmed by its item's own question through the
//!   owner's [`RoundTerminal`] ([`RoundRun`]);
//! - [`round_stage_answers`], [`round_limit`] (no command; the daemon's
//!   round routes): the Questions screen's structured answers
//!   ([`RoundAnswersBody`]) checked as a returned file's are and staged,
//!   each refused alone, nothing decided ([`RoundAnswersOutcome`]); the
//!   sheet's `--limit` judged before anything runs;
//! - [`propose_question`], [`propose_discrepancy`], [`propose_decision`]:
//!   an agent's question, discrepancy or decision taken during work stored
//!   as a queue record (approved into a decision record, or rejected with
//!   the answer), unless what is decided or asked already answers it
//!   ([`IntakeDocument`]);
//! - [`gate`]: the task gate's verdict on one agent write (canon `gate`,
//!   "Verdict"): exit 0 pass, 2 refuse or no verdict, never a [`CliError`];
//!   [`read_hook_input`] reads the hook's stdin within the deadline;
//! - [`export_state`], [`import_state`]: the queue's dump ([`STATE_FORMAT`])
//!   written outside the worktree, and restored into an empty queue after
//!   the owner's [`Consent`];
//! - [`render_text`], [`render_json`]: an [`Outcome`] as stdout (JSON: one
//!   document, every key present, absent = `null`; `check`: the report's
//!   own JSON), bounded by [`OUTPUT_CAP_CHARS`] (`check`: unbounded;
//!   `bundle`: its body fitted within it, never cut);
//!   [`Outcome::stderr_lines`]: its `note:` and `warning:` lines;
//! - [`serve()`]: `spec serve`'s decision (task spec `spec-serve`): the
//!   roots, the data directory guarded against each, the daemon's binary
//!   found, a running daemon probed through `daemon.lock` (nothing
//!   created): its lines ([`Serve::Running`]) or the daemon to exec
//!   ([`Serve::Start`], `main` execs it); [`DaemonFile`] is `daemon.json`,
//!   which only the daemon writes ([`prepared_data_dir`], [`checked_data_dir`]
//!   its data directory, created and guarded);
//! - [`verify`]: `spec verify [--changed] [--task T]`, a state per lock key
//!   and marker binding, Fix, Check or Pre-existing findings, the verdict
//!   under `[verify] mode`; its run causes are in the report (exit 2);
//!   `--task` keeps the rows of a task's nodes (task spec `verify-task`);
//!   `--node REF [--symbol QPATH]` one node's, judged over the files the
//!   code index picks ([`VerifyNode`]; task spec `verify-node`);
//!   [`verify_tests`]: `spec verify --tests [--task T]` (task spec
//!   `verify-tests`), the `@verifies` tests run by the project's own
//!   command after the owner's [`Consent`], Unix only; a completed run's
//!   evidence committed to `spec.lock` under that one consent (task spec
//!   `tested-evidence`), judged on every plain verify and shown by the
//!   reads of a node's code (task spec `tested-reads`;
//!   [`specengine_model::TestedRow`]);
//! - [`symbols`], [`unmapped`]: `spec symbols` (task spec `symbol-reads`:
//!   a QUERY's items in tiers, a node's, a file's or a line's targets,
//!   each with its markers; a `--node` naming nothing is an outcome with
//!   its reason, exit 1) and `spec unmapped` (the units a marker maps, per
//!   directory and code file), read from the code index; for the bridges
//!   with [`SymbolsForm`] and [`SYMBOLS_LIMIT_MIN`], [`SYMBOLS_LIMIT_MAX`],
//!   [`SYMBOLS_LIMIT_DEFAULT`], [`symbols_limit_error`], no clap;
//! - [`resolve`]: `spec resolve` (task spec `layer-c-resolve`), the one
//!   command that spawns the layer C executable (Unix only), its reply
//!   checked and stored as the worktree's advisory snapshot after `done`
//!   over an unchanged tree; [`layer_c_state`] that snapshot's freshness;
//! - [`roadmap()`]: `spec roadmap [--release R | --pool]` (task spec
//!   `roadmap-items`), the plan of the `[roadmap]` items computed from the
//!   repository's files over the index it updates, joined with this
//!   repository's open tasks read as the queue holds them (task spec
//!   `roadmap-position`: no schema step, a queue that cannot be read a
//!   note, never an exit); a `--release` naming no release is an outcome
//!   with its reason, exit 1; [`roadmap_flags`] its usage check, for the
//!   daemon;
//! - [`Exit`]: 0 answered, 1 not found (`show`, `tree`, `graph`, `bundle`,
//!   `refs`, `symbols`, `roadmap`), blocked (`check`, `verify`) or refused by the
//!   proposal or its target (the queue's commands), 2 could not run
//!   (`check`: could not check; `verify`: could not verify).
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
//! new file, or its decision record) and commits it in the recorded worktree
//! (`round answer` too, one record per item the owner confirms), `spec lock
//! accept`, which writes only `spec.lock` and commits it alone, and `spec
//! lock regen`, which writes only `spec.lock` (canon `spec-lock`,
//! "Travel and writes"); `index`, `verify`, `refs`, `symbols`, `unmapped`,
//! `search`, `show`, `tree`, `graph`, `bundle`, `roadmap`, `propose`, `inbox`,
//! `review`, `reject`, `import-state`, `round new` and `resolve` write only
//! the data directory (`propose question`, `discrepancy` and `decision`
//! too), `export state` only its dump (never inside the worktree), `check`,
//! `health` and `serve` nothing (the daemon `serve` execs writes its data
//! directory).
//! Nothing found in the corpus is fatal to the indexed reads (`search`,
//! `show`, `tree`, `graph`, `bundle`, `roadmap`): broken or unreadable files are
//! indexed with their diagnostics and never change their exit code. The
//! walked reads differ: `check` cannot check, and `health` cannot run
//! (exit 2), when a file cannot be read.

mod amend;
mod apply;
mod bindings;
mod bump;
mod bundle;
mod cap;
mod check;
mod code_delta;
mod code_index;
mod code_walk;
mod corpus;
mod create;
mod decide;
mod documents;
mod drift;
mod drift_apply;
mod events;
mod export;
mod follow_up;
mod gate;
mod graph;
#[cfg(unix)]
mod group;
mod health;
mod health_code;
mod impact;
mod inbox;
mod init;
mod intake;
mod interpret;
mod layer_c;
#[cfg(unix)]
mod layer_c_reply;
#[cfg(unix)]
mod layer_c_run;
mod links;
mod location;
mod lock;
mod lock_rebase;
mod lock_regen;
mod node_set;
mod package;
mod preflight;
mod project;
mod proposals;
mod propose;
mod refresh;
mod refs;
mod review;
mod revisions;
mod roadmap;
mod roles;
mod round;
mod search;
mod serve;
mod show;
mod stage;
mod state;
mod state_file;
mod symbols;
mod task;
#[cfg(unix)]
mod test_runner;
mod tested;
mod tree;
mod unmapped;
mod verify;
mod verify_node;
mod verify_rows;
mod verify_tests;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

pub use amend::{AmendmentRequest, propose_amendment, propose_amendment_brief};
pub use apply::{
    ApproveFlags, ApproveRequest, Consent, RejectRequest, approve, approve_editorial, approve_with,
    reject, reject_follow_up,
};
pub use bump::{BumpOutcome, BumpRequest, bump};
pub use bundle::{
    BUNDLE_TAIL_LINES, Bundle, BundleItem, BundleOutcome, BundleRequest, BundleRow,
    DEFAULT_BUNDLE_BUDGET, ItemForm, TailEntry, WorkingAnswer, bundle, layer_heading, layer_key,
};
pub use cap::{OUTPUT_CAP_CHARS, SHOW_TAIL_NAMES, View};
pub use check::{CheckOutcome, CheckRequest, CheckedTree, check};
pub use code_index::{CodeRefresh, known_frameworks, listed_code_files, refresh_code};
pub use corpus::LeftOut;
pub use create::{
    CreateRequest, propose_create, propose_create_brief, propose_create_brief_with_task,
    propose_create_with_task,
};
pub use documents::{DocumentEntry, documents};
pub use drift::{
    DriftCreated, DriftDocument, DriftHit, DriftJudged, DriftOutcome, DriftRequest, propose_drift,
};
pub use events::{EVENTS_PAGE_MAX, EventLine, EventsPage, EventsTail, events_after};
pub use export::{ExportIndexRequest, ExportOutcome, ShardOutcome, export_index};
pub use gate::{
    GATE_DEADLINE, GATE_TASKS_MAX, GateInput, GateOutcome, GateProgress, GateReason, GateRequest,
    GateStage, GateTaskEntry, GateVerdict, HookInput, LINK_HOPS, gate, gate_session,
    read_hook_input,
};
pub use graph::{
    FollowedType, GraphEdge, GraphNode, GraphOutcome, GraphRequest, graph, graph_with_view,
};
pub use health::{HealthOutcome, UnappliedAmendment, health};
pub use health_code::{
    AssumedNode, AssumedProposal, AssumedState, AssumesCounts, AssumesRow, CodeRow, CodeRows,
    RowState, UnboundCounts, UnboundMarker, UnboundRow,
};
pub use impact::{ImpactOutcome, ImpactRequest, ImpactRow, ImpactTask, SeedSymbol, impact};
pub use inbox::{INBOX_RATIONALE_CHARS, InboxEntry, InboxOutcome, InboxRequest, inbox};
pub use init::{InitOutcome, InitRequest, derive_slug, init};
pub use intake::{
    DecisionRequest, DiscrepancyRequest, INTAKE_INPUT_MAX_BYTES, INTAKE_MATCHES_MAX,
    IntakeDocument, IntakeMatch, IntakeOutcome, IntakeSource, MatchSource, QuestionRequest,
    propose_decision, propose_decision_with_task, propose_discrepancy,
    propose_discrepancy_with_task, propose_question, propose_question_with_task,
    read_decision_input, read_discrepancy_input,
};
pub use interpret::{InterpretRequest, propose_interpretation, propose_interpretation_brief};
pub use layer_c::{
    LayerCState, RESOLVE_TIMEOUT_DEFAULT, RESOLVE_TIMEOUT_MAX, RESOLVE_TIMEOUT_MIN, ResolveByState,
    ResolveOutcome, ResolveRequest, ResolvedWorkspace, layer_c_state, resolve,
};
pub use links::{ShownLink, ShownLinks};
pub use location::{OpenIndex, checked_data_dir, data_dir, db_path, open_index, prepared_data_dir};
pub use lock::{
    LockAcceptRequest, LockConsent, LockMode, LockOutcome, LockPrompt, LockRow, LockTrailer,
    LockWritten, lock_accept,
};
pub use lock_rebase::{
    LockRebaseRequest, RebaseCounts, RebaseOutcome, RebaseRow, RebaseSide, lock_rebase,
};
pub use lock_regen::{LockRegenRequest, RegenOutcome, RegenRow, RegenTestedRow, lock_regen};
pub use project::{
    CONFIG_FILE, Located, ProjectEntry, ProjectRoot, discover, locate, project_entry,
};
pub use proposals::{Preview, ProposalDocument, ProposalDrift, ProposalOutcome, QueueCommand};
pub use propose::{
    ProposeRequest, ProposedText, TEXT_MAX_BYTES, propose, propose_brief, propose_brief_with_task,
    propose_with_task,
};
pub use refresh::{IndexOutcome, IndexRequest, index};
pub use refs::{
    BindingRow, CodeCount, MarkerRow, MentionRow, RefsNode, RefsOutcome, RefsRequest, refs,
};
pub use review::{ReviewRequest, review, review_brief};
pub use roadmap::{RoadmapOutcome, RoadmapRequest, roadmap, roadmap_flags, roadmap_with_view};
pub use roles::{RoleCounts, SubjectCounts, role_counts};
pub use round::{
    AnswerOutcome, RoundAnswer, RoundAnswerBody, RoundAnswerEntry, RoundAnswerRequest,
    RoundAnswersBody, RoundAnswersOutcome, RoundAnswersRequest, RoundItem, RoundNewRequest,
    RoundRun, RoundSheet, RoundStageRequest, RoundStep, RoundTarget, RoundTask, RoundTerminal,
    WalkOutcome, round_answer, round_limit, round_new, round_stage, round_stage_answers,
};
pub use search::{HitCut, SearchOutcome, SearchRequest, search, search_with_view};
pub use serve::{
    DAEMON_BIN, DAEMON_FILE, DAEMON_LOCK, DAEMON_POLL, DAEMON_PORT, DAEMON_PUBLISH_WAIT,
    DaemonFile, DaemonProject, Serve, ServeRequest, daemon_text, read_daemon_file, serve,
    unpublished_daemon,
};
pub use show::{
    NestedSection, ShowOutcome, ShowRequest, ShownBindings, ShownNode, show, show_with_view,
};
pub use specengine_core::ProjectConfig;
/// The intake's input types, enums and caps (core's), for the bridges;
/// the names of the kinds `propose_change` takes and of a decision, and a
/// kind's name with its article.
pub use specengine_core::intake::{
    AMENDMENT_KIND, ANSWER_MAX, CREATE_KIND, DECISION_KIND, DISTINCT_ITEM_MAX, DISTINCT_MAX,
    DRIFT_KIND, DecisionInput, DiscrepancyInput, EVIDENCE_MAX, EVIDENCE_TEXT_MAX, Evidence,
    GapType, INTERPRETATION_FIELDS, INTERPRETATION_KIND, INTERPRETATION_TEXT_MAX, IntakeOption,
    IntakeSeverity, Interpretation, LABEL_MAX, LINE_LIMIT, LOCATION_MAX, NODE_IDS_MAX,
    OPTION_TEXT_MAX, OPTIONS_MAX, OPTIONS_MIN, ProposedPatch, RATIONALE_MAX, SUMMARY_MAX,
    UPDATE_KIND, with_article,
};
/// The most bytes of an author's `role`, `model` or `run`.
pub use specengine_core::proposal::AUTHOR_FIELD_MAX;
/// A drift item's follow-up kind (`spec reject PR --follow-up code|spec`;
/// core's).
pub use specengine_core::task::FollowUp;
/// The task's caps and the outcomes a run reports (core's).
pub use specengine_core::task::{
    AFFECTED_MAX, CHANGED_FILE_MAX, CHANGED_FILES_MAX, CRITERIA_MAX, CRITERION_MAX,
    CRITERION_TEXT_MAX, DEFAULT_TASK_BUDGET, DIFF_MAX, DIFFS_TOTAL_MAX, GOAL_MAX, NODES_MAX,
    NOTE_MAX, PACKAGE_BINDINGS_MAX, PACKAGE_BUDGET, PLAN_MAX, RUN_SUMMARY_MAX, SNAPSHOT_MAX,
    TITLE_MAX,
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
pub use symbols::{
    SYMBOLS_LIMIT_DEFAULT, SYMBOLS_LIMIT_MAX, SYMBOLS_LIMIT_MIN, SymbolBinding, SymbolRole,
    SymbolTarget, SymbolsForm, SymbolsOutcome, SymbolsRequest, symbols, symbols_limit_error,
};
pub use task::{
    TaskClaimRequest, TaskCompleteRequest, TaskDecisionRequest, TaskListEntry, TaskListOutcome,
    TaskListRequest, TaskNewRequest, TaskOutcome, TaskPlanRequest, TaskReportRequest,
    TaskShowOutcome, TaskShowRequest, task_approve, task_cancel, task_changes, task_claim,
    task_complete, task_list, task_new, task_plan, task_report, task_show,
};
pub use tree::{TreeMark, TreeNode, TreeOutcome, TreeRequest, tree, tree_with_view};
pub use unmapped::{UnmappedItem, UnmappedOutcome, UnmappedRequest, UnmappedRow, unmapped};
pub use verify::{
    VerifiedTree, VerifyAnchor, VerifyFormat, VerifyLevels, VerifyLock, VerifyMarker, VerifyNode,
    VerifyOutcome, VerifyPlace, VerifyRequest, VerifyRow, verify, verify_as,
};
pub use verify_tests::{
    VerifyTestsLock, VerifyTestsNode, VerifyTestsOutcome, VerifyTestsProbe, VerifyTestsRequest,
    VerifyTestsRow, verify_tests,
};

/// The exit code of a command (the verdict scheme of `spec check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Exit {
    /// The command answered, zero search hits included; `spec check`:
    /// clean or observed.
    Answered = 0,
    /// `spec show`, `spec tree`, `spec graph`, `spec bundle`, `spec refs`,
    /// `spec symbols --node`, `spec impact` found nothing: a dangling
    /// reference, no configured prefix, a `.md` path that is not indexed,
    /// a `--symbol` naming no code item.
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
    /// `bump`: the nodes changed without a raise.
    Bump(Box<BumpOutcome>),
    /// `health`: the unapplied amendments of the working tree.
    Health(HealthOutcome),
    /// `roadmap`: the plan of the `[roadmap]` items.
    Roadmap(Box<RoadmapOutcome>),
    /// `impact`: what a change of a node or a code item reaches.
    Impact(Box<ImpactOutcome>),
    /// `refs`: what in the code names a node.
    Refs(Box<RefsOutcome>),
    /// `symbols`: the code index's targets of one form.
    Symbols(Box<SymbolsOutcome>),
    /// `unmapped`: how much of the code the spec maps.
    Unmapped(Box<UnmappedOutcome>),
    /// `resolve`: the layer C snapshot stored, and its figures.
    Resolve(Box<ResolveOutcome>),
    /// `lock accept`: the node's entries of `spec.lock`, committed.
    Lock(Box<LockOutcome>),
    /// `lock regen`: a conflict's stages of `spec.lock` merged.
    LockRegen(Box<RegenOutcome>),
    /// `lock rebase`: `spec.lock` carried into this build's format and
    /// recipes, committed.
    LockRebase(Box<RebaseOutcome>),
    /// `verify`: drift between `spec.lock` and the tree.
    Verify(Box<VerifyOutcome>),
    /// `verify --tests`: the bound tests run by the project's command.
    VerifyTests(Box<VerifyTestsOutcome>),
    Export(ExportOutcome),
    /// `propose`, `review`, `approve`, `reject`: the review document.
    Proposal(Box<ProposalOutcome>),
    /// `propose question`, `propose discrepancy`: the intake document.
    Intake(Box<IntakeOutcome>),
    /// `propose drift`: the drift document.
    Drift(Box<DriftOutcome>),
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
    /// `round new`: the sheet.
    RoundSheet(Box<RoundSheet>),
    /// `round answer`: the answers, staged, and the walk.
    Round(Box<RoundRun>),
    /// [`round_stage_answers`] (no command; the daemon's): each structured
    /// answer staged or refused.
    RoundAnswers(Box<RoundAnswersOutcome>),
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
            Self::Index(index) if index.code_error.is_some() => Exit::CannotRun,
            Self::Refs(refs) if refs.reason.is_some() => Exit::NotFound,
            Self::Symbols(symbols) if symbols.reason.is_some() => Exit::NotFound,
            Self::Roadmap(roadmap) if roadmap.reason.is_some() => Exit::NotFound,
            Self::Impact(impact) if impact.reason.is_some() => Exit::NotFound,
            Self::Lock(lock) => lock.exit(),
            Self::LockRegen(regen) => regen.exit(),
            Self::LockRebase(rebase) => rebase.exit(),
            Self::Verify(verify) => verify.exit(),
            Self::VerifyTests(run) => run.exit(),
            Self::Check(check) => check.exit(),
            Self::Bump(bump) => bump.exit(),
            Self::Proposal(proposal) => proposal.exit(),
            Self::Intake(intake) => intake.exit(),
            Self::Drift(drift) => drift.exit(),
            Self::Stage(stage) => stage.exit(),
            Self::StateImport(import) => import.exit(),
            Self::Task(task) => task.exit(),
            Self::TaskShow(show) => show.exit(),
            Self::Round(run) => run.exit(),
            Self::RoundAnswers(answers) => answers.exit(),
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
            Self::Bump(outcome) => (&outcome.messages, None),
            Self::Health(outcome) => (&outcome.messages, None),
            Self::Roadmap(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Impact(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Refs(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Symbols(outcome) => (&outcome.messages, outcome.reason.as_deref()),
            Self::Unmapped(outcome) => (&outcome.messages, None),
            Self::Resolve(outcome) => (&outcome.messages, None),
            Self::Lock(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::LockRegen(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::LockRebase(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::Verify(outcome) => (&outcome.messages, None),
            Self::VerifyTests(outcome) => (&outcome.messages, None),
            Self::Export(outcome) => (&outcome.messages, None),
            Self::Proposal(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::Intake(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
            Self::Drift(outcome) => (&outcome.messages, outcome.refusal.as_deref()),
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
            Self::RoundSheet(outcome) => (&outcome.messages, None),
            Self::Round(outcome) => (&outcome.messages, None),
            Self::RoundAnswers(outcome) => (&outcome.messages, None),
        };
        let mut lines: Vec<String> = messages.iter().map(Message::line).collect();
        if let Some(reason) = reason {
            lines.push(format!("spec: {}", one_line(reason)));
        }
        // A `[code]` that does not read: its config line, as `spec refs`
        // prints it, after the spec index was written.
        if let Self::Index(outcome) = self
            && let Some(error) = &outcome.code_error
        {
            lines.push(one_line(error));
        }
        // Health's rows 4-5: the refreshes' notes, a `cannot` row's causes.
        if let Self::Health(outcome) = self {
            lines.extend(outcome.code_rows.messages.iter().map(Message::line));
        }
        // The queue's commands quote agent-written text.
        // So do the lock's commands: a path, a `qpath`, an ID.
        if matches!(
            self,
            Self::Proposal(_)
                | Self::Bump(_)
                | Self::Lock(_)
                | Self::LockRegen(_)
                | Self::LockRebase(_)
                | Self::Verify(_)
                | Self::VerifyTests(_)
                | Self::Intake(_)
                | Self::Drift(_)
                | Self::Stage(_)
                | Self::Inbox(_)
                | Self::StateExport(_)
                | Self::StateImport(_)
                | Self::Task(_)
                | Self::TaskShow(_)
                | Self::TaskList(_)
                | Self::RoundSheet(_)
                | Self::Round(_)
                | Self::RoundAnswers(_)
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
        Outcome::Bump(outcome) => bump::render_text(outcome),
        Outcome::Health(outcome) => health::render_text(outcome),
        Outcome::Roadmap(outcome) => roadmap::render_text(outcome),
        Outcome::Impact(outcome) => impact::render_text(outcome),
        Outcome::Refs(outcome) => refs::render_text(outcome),
        Outcome::Symbols(outcome) => symbols::render_text(outcome),
        Outcome::Unmapped(outcome) => unmapped::render_text(outcome),
        Outcome::Resolve(outcome) => layer_c::render_text(outcome),
        Outcome::Lock(outcome) => lock::render_text(outcome),
        Outcome::LockRegen(outcome) => lock_regen::render_text(outcome),
        Outcome::LockRebase(outcome) => lock_rebase::render_text(outcome),
        Outcome::Verify(outcome) => verify::render_text(outcome),
        Outcome::VerifyTests(outcome) => verify_tests::render_text(outcome),
        Outcome::Export(outcome) => export::render_text(outcome),
        Outcome::Proposal(outcome) => proposals::render_text(outcome),
        Outcome::Intake(outcome) => intake::render_text(outcome),
        Outcome::Drift(outcome) => drift::render_text(outcome),
        Outcome::Stage(outcome) => proposals::render_text(&outcome.proposal),
        Outcome::Inbox(outcome) => inbox::render_text(outcome),
        Outcome::StateExport(outcome) => state::render_export_text(outcome),
        Outcome::StateImport(outcome) => state::render_import_text(outcome),
        Outcome::Task(outcome) => task::render_text(outcome),
        Outcome::TaskShow(outcome) => task::render_show_text(outcome),
        Outcome::TaskList(outcome) => task::render_list_text(outcome),
        Outcome::RoundSheet(outcome) => round::sheet_text(outcome),
        Outcome::Round(outcome) => round::render_text(outcome),
        Outcome::RoundAnswers(outcome) => round::answers_outcome_text(outcome),
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
            Self::Bump(outcome) => outcome.serialize(serializer),
            Self::Health(outcome) => outcome.serialize(serializer),
            Self::Roadmap(outcome) => outcome.serialize(serializer),
            Self::Impact(outcome) => outcome.serialize(serializer),
            Self::Refs(outcome) => outcome.serialize(serializer),
            Self::Symbols(outcome) => outcome.serialize(serializer),
            Self::Unmapped(outcome) => outcome.serialize(serializer),
            Self::Resolve(outcome) => outcome.serialize(serializer),
            Self::Lock(outcome) => outcome.serialize(serializer),
            Self::LockRegen(outcome) => outcome.serialize(serializer),
            Self::LockRebase(outcome) => outcome.serialize(serializer),
            Self::Verify(outcome) => outcome.serialize(serializer),
            Self::VerifyTests(outcome) => outcome.serialize(serializer),
            Self::Export(outcome) => outcome.serialize(serializer),
            Self::Proposal(outcome) => outcome.serialize(serializer),
            Self::Intake(outcome) => outcome.serialize(serializer),
            Self::Drift(outcome) => outcome.serialize(serializer),
            Self::Stage(outcome) => outcome.serialize(serializer),
            Self::Inbox(outcome) => outcome.serialize(serializer),
            Self::StateExport(outcome) => outcome.serialize(serializer),
            Self::StateImport(outcome) => outcome.serialize(serializer),
            Self::Task(outcome) => outcome.serialize(serializer),
            Self::TaskShow(outcome) => outcome.serialize(serializer),
            Self::TaskList(outcome) => outcome.serialize(serializer),
            Self::RoundSheet(outcome) => outcome.serialize(serializer),
            Self::Round(outcome) => outcome.serialize(serializer),
            Self::RoundAnswers(outcome) => outcome.serialize(serializer),
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

/// `top_path` (from the worktree's top) as one shell word of a printed git
/// command: a literal pathspec, as a commit passes it (no glob, no magic),
/// single-quoted with an embedded `'` written `'\''`, so the pasted command
/// names exactly that file whatever its name holds (an editorial apply's
/// restore, `spec lock accept`'s `Undo`).
pub(crate) fn pathspec_word(top_path: &str) -> String {
    format!("':(literal){}'", top_path.replace('\'', r"'\''"))
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
