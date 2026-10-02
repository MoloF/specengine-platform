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
//! within a budget of estimated tokens, named by its `bundle_hash`.
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
//! - [`render_text`], [`render_json`]: an [`Outcome`] as stdout (JSON: one
//!   document, every key present, absent = `null`; `check`: the report's
//!   own JSON), bounded by [`OUTPUT_CAP_CHARS`] (`check`: unbounded;
//!   `bundle`: its body fitted within it, never cut);
//!   [`Outcome::stderr_lines`]: its `note:` and `warning:` lines;
//! - [`Exit`]: 0 answered, 1 not found (`show`, `tree`, `graph`, `bundle`) or
//!   blocked (`check`), 2 could not run (`check`: could not check).
//!
//! One database state gives one stdout, byte for byte: nothing depends on
//! time, storage order or the absolute root (`check`: one tree, config,
//! baseline and date). No command writes under the project root but
//! `spec init`, which creates its one file, and `spec export index`, which
//! writes only `[paths] index`; `index`, `search`, `show`, `tree`, `graph`
//! and `bundle` write only the data directory, `check` nothing. Nothing found in the corpus is fatal
//! to the read commands: broken or unreadable files are indexed with their
//! diagnostics and never change an exit code.

mod bundle;
mod cap;
mod check;
mod corpus;
mod documents;
mod export;
mod graph;
mod init;
mod links;
mod location;
mod project;
mod refresh;
mod search;
mod show;
mod tree;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

pub use bundle::{
    BUNDLE_TAIL_LINES, Bundle, BundleItem, BundleOutcome, BundleRequest, DEFAULT_BUNDLE_BUDGET,
    ItemForm, TailEntry, WorkingAnswer, bundle, layer_heading, layer_key,
};
pub use cap::{OUTPUT_CAP_CHARS, SHOW_TAIL_NAMES};
pub use check::{CheckOutcome, CheckRequest, CheckedTree, check};
pub use corpus::LeftOut;
pub use documents::{DocumentEntry, documents};
pub use export::{ExportIndexRequest, ExportOutcome, ShardOutcome, export_index};
pub use graph::{FollowedType, GraphEdge, GraphNode, GraphOutcome, GraphRequest, graph};
pub use init::{InitOutcome, InitRequest, derive_slug, init};
pub use links::{ShownLink, ShownLinks};
pub use location::{OpenIndex, data_dir, db_path, open_index};
pub use project::{CONFIG_FILE, Located, ProjectRoot, discover, locate};
pub use refresh::{IndexOutcome, IndexRequest, index};
pub use search::{HitCut, SearchOutcome, SearchRequest, search};
pub use show::{NestedSection, ShowOutcome, ShowRequest, ShownNode, show};
pub use specengine_core::ProjectConfig;
/// `search`'s `--limit` bounds and default, its shortest term (the store's).
pub use specengine_store::{
    MIN_TERM_CHARS, SEARCH_LIMIT_DEFAULT, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN,
};
pub use tree::{TreeMark, TreeNode, TreeOutcome, TreeRequest, tree};

/// The exit code of a command (the verdict scheme of `spec check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Exit {
    /// The command answered, zero search hits included; `spec check`:
    /// clean or observed.
    Answered = 0,
    /// `spec show`, `spec tree`, `spec graph`, `spec bundle` found nothing: a dangling
    /// reference, no configured prefix, a `.md` path that is not indexed.
    /// `spec check`: blocked.
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
}

impl Outcome {
    /// 0, or 1 for a `show`, `tree`, `graph` or `bundle` that found nothing; `check`:
    /// its verdict's exit (1 blocked, 2 could not check).
    pub fn exit(&self) -> Exit {
        match self {
            Self::Show(show) if show.nodes.is_empty() => Exit::NotFound,
            Self::Tree(tree) if tree.reason.is_some() => Exit::NotFound,
            Self::Graph(graph) if graph.reason.is_some() => Exit::NotFound,
            Self::Bundle(bundle) if bundle.reason.is_some() => Exit::NotFound,
            Self::Check(check) => check.exit(),
            _ => Exit::Answered,
        }
    }

    /// The stderr lines: notes and warnings in the order they arose, then,
    /// for a `show`, `tree`, `graph` or `bundle` that found nothing,
    /// `spec: <reason>`.
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
        };
        let mut lines: Vec<String> = messages.iter().map(Message::line).collect();
        if let Some(reason) = reason {
            lines.push(format!("spec: {}", one_line(reason)));
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

/// A store error: exit 2.
pub(crate) fn store_error(error: specengine_store::StoreError) -> CliError {
    CliError::spec(error)
}

/// A header or table field on one line: line breaks become spaces.
pub(crate) fn one_line(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}
