//! `spec check [--staged] [--baseline F] [--debt]`: the documentation
//! convention's check (core's `check::run`) over a fresh parse of the
//! working tree, no database, data directory, slug or `HOME`; nothing is
//! written.
//!
//! `--staged` checks what `git commit` would record instead: the git index
//! git names (`GIT_INDEX_FILE`, so `commit -a`, `-o` too), by the store's
//! `check_staged` (read-only plumbing, no fetch, no `HEAD`). Discovery is
//! unchanged, on disk; the config is the staged `specengine.toml` unless
//! `--config`, the baseline the staged `.spec-debt.toml` (when staged)
//! unless `--baseline`, never the working tree's. Git's failures are causes
//! of the report (exit 2), with fixed messages; git's own text is never
//! shown. Exit codes, streams, text and JSON are the plain check's: a fully
//! staged tree prints the same bytes.
//!
//! Root and config come from discovery (`--root`, `--config`); after it,
//! every failure is a `cannot` cause of the printed report, exit 2: the
//! config unreadable, not UTF-8 or invalid anywhere (`ProjectConfig` and the
//! check tables, a cause per distinct error), the baseline missing,
//! unreadable or invalid, the root unreadable, the walk's causes. The
//! baseline is `--baseline F` (relative to the current directory; it must
//! exist), else the root's `.spec-debt.toml` when an entry of that name
//! exists. The mode is `[check] mode` only; today is the UTC date at start.
//!
//! Names: the config as discovery gives it (`specengine.toml`, or
//! `--config` as typed), the default baseline `.spec-debt.toml`,
//! `--baseline` as typed, the root `.`: no output holds an absolute path
//! the caller did not type. One tree, config, baseline and date give the
//! same bytes on stdout and stderr, wherever the root lies.

use std::path::PathBuf;

use specengine_core::check::{Report, Verdict};
use specengine_store::{
    GitEnv, NamedBytes, check_staged, check_tree, default_baseline, load_check, today_utc,
};

use crate::project::locate;
use crate::{CliError, Env, Exit, Globals, Message, one_line};

/// `spec check` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckRequest {
    /// `--staged`: check the git index instead of the working tree, git run
    /// with this environment (the caller's current directory and
    /// variables; `main` passes the process's).
    pub staged: Option<GitEnv>,
    /// `--baseline F`: replaces the root's baseline; relative to the current
    /// directory, named as typed.
    pub baseline: Option<PathBuf>,
    /// `--debt`: the text lists every finding and the stale entries too.
    pub debt: bool,
    /// The answer is printed as JSON, which `--debt` does not change (a
    /// `note:` says so).
    pub json: bool,
}

/// What `spec check` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    /// The report, a `cannot-check` one included.
    pub report: Report,
    /// `--debt`.
    pub debt: bool,
    pub messages: Vec<Message>,
}

impl CheckOutcome {
    /// The verdict's exit: 0 clean or observed, 1 blocked, 2 cannot check.
    pub fn exit(&self) -> Exit {
        match self.report.verdict {
            Verdict::Clean | Verdict::Observed => Exit::Answered,
            Verdict::Blocked => Exit::NotFound,
            Verdict::CannotCheck => Exit::CannotRun,
        }
    }
}

/// `spec check`: judges the project's documents. Only a discovery failure
/// is an error; every later failure is in the report.
pub fn check(
    env: &Env,
    globals: &Globals,
    request: &CheckRequest,
) -> Result<CheckOutcome, CliError> {
    let today = today_utc();
    let located = locate(env, globals)?;
    let config = || NamedBytes::read(located.config_label.clone(), &located.config_file);
    let given_baseline = request
        .baseline
        .as_ref()
        .map(|path| NamedBytes::read(path.display().to_string(), &env.cwd.join(path)));
    let report = match &request.staged {
        Some(git) => check_staged(
            &located.root,
            globals.config.is_some().then(config),
            given_baseline,
            git,
            &today,
        ),
        None => {
            let baseline = given_baseline.or_else(|| default_baseline(&located.root));
            match load_check(&config(), baseline.as_ref()) {
                Ok(setup) => check_tree(&located.root, &setup, &today),
                Err(report) => *report,
            }
        }
    };
    let mut messages = Vec::new();
    if request.json && request.debt {
        messages.push(Message::Note(
            "--debt changes only the text report: the JSON lists every finding and stale entry"
                .to_owned(),
        ));
    }
    Ok(CheckOutcome {
        report,
        debt: request.debt,
        messages,
    })
}

/// `Report::lines`, each on one line.
pub(crate) fn render_text(outcome: &CheckOutcome) -> String {
    let mut out = String::new();
    for line in outcome.report.lines(outcome.debt) {
        out.push_str(&one_line(&line));
        out.push('\n');
    }
    out
}

/// `Report::to_json` and a line end, whatever `--debt`.
pub(crate) fn render_json(outcome: &CheckOutcome) -> String {
    let mut json = outcome.report.to_json();
    json.push('\n');
    json
}
