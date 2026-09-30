//! The `spec` command line, pass 1 of the Phase 1 CLI (task spec
//! `spec-cli`): the agent read loop, `spec init`, `spec index`,
//! `spec search`, `spec show`.
//!
//! Every command lives here, below `main`: MCP stdio and the Phase 2 daemon
//! bridge call the same functions. `main.rs` only parses the arguments,
//! calls one command and prints what it gives.
//!
//! - [`discover`] finds the project root and reads its `specengine.toml`
//!   through core's one loader ([`ProjectConfig`]);
//! - [`data_dir`], [`db_path`], [`open_index`]: the index database lives
//!   outside the repository, one per project slug;
//! - [`init`], [`index`], [`search`], [`show`]: one function per command,
//!   each giving its outcome or a [`CliError`];
//! - [`render_text`], [`render_json`]: an [`Outcome`] as stdout (JSON: one
//!   document, every key present, absent = `null`), bounded by
//!   [`OUTPUT_CAP_CHARS`]; [`Outcome::stderr_lines`]: its `note:` and
//!   `warning:` lines;
//! - [`Exit`]: 0 answered, 1 not found (`show`), 2 could not run.
//!
//! One database state gives one stdout, byte for byte: nothing depends on
//! time, storage order or the absolute root. No command writes under the
//! project root but `spec init`, which creates its one file; `index`,
//! `search` and `show` write only the data directory. Nothing found in the
//! corpus is fatal: broken or unreadable files are indexed with their
//! diagnostics and never change an exit code.

mod cap;
mod init;
mod location;
mod project;
mod refresh;
mod search;
mod show;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

pub use cap::OUTPUT_CAP_CHARS;
pub use init::{InitOutcome, InitRequest, derive_slug, init};
pub use location::{OpenIndex, data_dir, db_path, open_index};
pub use project::{CONFIG_FILE, ProjectRoot, discover};
pub use refresh::{IndexOutcome, IndexRequest, index};
pub use search::{HitCut, SearchOutcome, SearchRequest, search};
pub use show::{NestedSection, ShowOutcome, ShowRequest, ShownNode, show};
pub use specengine_core::ProjectConfig;

/// The exit code of a command (the verdict scheme of `spec check`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Exit {
    /// The command answered, zero search hits included.
    Answered = 0,
    /// `spec show` found nothing: a dangling reference, no configured
    /// prefix, a `.md` path that is not indexed.
    NotFound = 1,
    /// The command could not run: usage, project, config, environment,
    /// database. Nothing is printed on stdout.
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
    /// The whole stderr line: `<config as given>:<line>: message` for a
    /// config error, else `spec: message`.
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
}

impl Outcome {
    /// 0, or 1 for a `show` that found nothing.
    pub fn exit(&self) -> Exit {
        match self {
            Self::Show(show) if show.nodes.is_empty() => Exit::NotFound,
            _ => Exit::Answered,
        }
    }

    /// The stderr lines: notes and warnings in the order they arose, then,
    /// for a `show` that found nothing, `spec: <reason>`.
    pub fn stderr_lines(&self) -> Vec<String> {
        let messages = match self {
            Self::Init(outcome) => &outcome.messages,
            Self::Index(outcome) => &outcome.messages,
            Self::Search(outcome) => &outcome.messages,
            Self::Show(outcome) => &outcome.messages,
        };
        let mut lines: Vec<String> = messages.iter().map(Message::line).collect();
        if let Self::Show(show) = self
            && let Some(reason) = &show.reason
        {
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
    }
}

/// The command's own document, as [`render_json`] prints it (the `show`
/// cap applied).
impl serde::Serialize for Outcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Init(outcome) => outcome.serialize(serializer),
            Self::Index(outcome) => outcome.serialize(serializer),
            Self::Search(outcome) => outcome.serialize(serializer),
            Self::Show(outcome) => outcome.serialize(serializer),
        }
    }
}

/// The outcome as one compact JSON document and a line end; every key
/// present, absent = `null`. Every value is plain data (strings, numbers,
/// lists), which `serde_json` always encodes.
pub fn render_json(outcome: &Outcome) -> String {
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
