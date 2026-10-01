//! `spec check [--staged] [--baseline F] [--debt]`: the documentation
//! convention's check (core's `check::run`) over a fresh parse of the
//! working tree, no database, data directory, slug or `HOME`; nothing is
//! written.
//!
//! `--staged` checks what `git commit` would record instead: the git index
//! git names (`GIT_INDEX_FILE`, so `commit -a`, `-o` too), by the store's
//! `check_staged_with_notes` (read-only plumbing, no fetch), judged against
//! `HEAD`, its base (task spec `spec-cli-introduced`): each finding
//! says whether it is introduced, new baseline entries are new debt, and
//! the stricter of the checked and `HEAD`'s modes applies. Discovery is
//! unchanged, on disk; the config is the staged `specengine.toml` unless
//! `--config`, the baseline the staged `.spec-debt.toml` (when staged)
//! unless `--baseline`, never the working tree's. Git's failures are causes
//! of the report (exit 2), with fixed messages; git's own text is never
//! shown. Exit codes and streams are the plain check's; the text and JSON
//! add the base's fields. The store's notes (`HEAD`'s mode or baseline
//! unknown, its stricter mode applied) go to stderr as `note:` lines,
//! never into the JSON. A plain run has no base: `enforce-introduced` is
//! judged and shown as `enforce`, with a note.
//!
//! Root and config come from discovery (`--root`, `--config`); after it,
//! every failure is a `cannot` cause of the printed report, exit 2: the
//! config unreadable, not UTF-8 or invalid anywhere (`ProjectConfig` and the
//! check tables, a cause per distinct error), the baseline missing,
//! unreadable or invalid, the root unreadable, the walk's causes. The
//! baseline is `--baseline F` (relative to the current directory; it must
//! exist), else the root's `.spec-debt.toml` when an entry of that name
//! exists. The mode is `[check] mode` (with `--staged`, the stricter of it
//! and `HEAD`'s); today is the UTC date at start.
//!
//! Names: the config as discovery gives it (`specengine.toml`, or
//! `--config` as typed), the default baseline `.spec-debt.toml`,
//! `--baseline` as typed, the root `.`: no output holds an absolute path
//! the caller did not type. One tree, config, baseline and date (and with
//! `--staged` one `HEAD` tree) give the same bytes on stdout and stderr,
//! wherever the root lies.

use std::path::PathBuf;

use specengine_core::check::{Mode, Report, Verdict};
use specengine_store::{
    GitEnv, GivenFile, NamedBytes, check_staged_with_notes, check_tree, default_baseline,
    load_check, today_utc,
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
    let given_baseline = request.baseline.as_ref().map(|path| {
        let full = env.cwd.join(path);
        GivenFile {
            bytes: NamedBytes::read(path.display().to_string(), &full),
            path: Some(full),
        }
    });
    let mut messages = Vec::new();
    let report = match &request.staged {
        Some(git) => {
            let given_config = globals.config.is_some().then(|| GivenFile {
                bytes: config(),
                path: Some(located.config_file.clone()),
            });
            let checked =
                check_staged_with_notes(&located.root, given_config, given_baseline, git, &today);
            messages.extend(checked.notes.into_iter().map(Message::Note));
            checked.report
        }
        None => {
            let baseline = given_baseline
                .map(|given| given.bytes)
                .or_else(|| default_baseline(&located.root));
            let report = match load_check(&config(), baseline.as_ref()) {
                Ok(setup) => check_tree(&located.root, &setup, &today),
                Err(report) => *report,
            };
            if report.mode == Mode::EnforceIntroduced {
                messages.push(Message::Note(
                    "mode `enforce-introduced` has no base without --staged: judged as `enforce`"
                        .to_owned(),
                ));
            }
            report.without_base()
        }
    };
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
