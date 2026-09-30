//! The fresh-parse loader of `spec check` (docs/canon/spec-check.md; its API
//! in this crate's README): the config and the baseline validated from
//! their bytes ([`load_config`], [`load_check`]), a [`Source`] walked by
//! `[paths]` and every listed file read and parsed into a [`CheckInput`]
//! ([`check_input`]), and [`specengine_core::check::run`] over it
//! ([`check_source`], [`check_tree`]). No database and no daemon: CI and
//! the pre-commit hook can run it. Nothing is written.
//!
//! The loader takes the config and the baseline as [`NamedBytes`]: the name
//! every cause carries, and the bytes or why they could not be read. The
//! working tree, a caller with its own discovery (`spec check`) and a git
//! index all feed it the same way; no cause names a path the caller did
//! not pass (the root is `.`, the root's baseline [`BASELINE_FILE`]).

use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use specengine_core::check::{
    self, Baseline, Cause, CheckConfig, CheckFile, CheckInput, Mode, Problem, ProblemKind, Report,
};
use specengine_core::{IdSchemeToml, Paths, ProjectConfig};
use specengine_model::IdScheme;

use crate::error::StoreError;
use crate::source::{Source, WorkingTree};

/// The baseline read from the worktree root when none is passed.
pub const BASELINE_FILE: &str = ".spec-debt.toml";

/// A file handed to the loader: the name its causes carry (as the caller
/// shows it: `specengine.toml`, a path as typed, a git path), and its bytes
/// or why they could not be read.
#[derive(Debug)]
pub struct NamedBytes {
    pub name: String,
    pub bytes: io::Result<Vec<u8>>,
}

impl NamedBytes {
    /// `path` read from disk, named `name`.
    pub fn read(name: impl Into<String>, path: &Path) -> Self {
        Self {
            name: name.into(),
            bytes: fs::read(path),
        }
    }
}

/// What a check runs with: the whole config, validated, and the baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSetup {
    /// The top level, `[project]`, `[ids]`, `[paths]` (core's one loader).
    pub project: ProjectConfig,
    /// `[budgets]`, `[classes]`, `[check]`, `[[generators]]`.
    pub config: CheckConfig,
    pub baseline: Baseline,
}

/// The baseline at the root, [`BASELINE_FILE`], when an entry of that name
/// exists there (a dangling symlink or a directory is then unreadable);
/// `None` when there is none, or when the root cannot be listed to tell
/// (the walk then reports the root itself).
pub fn default_baseline(root: &Path) -> Option<NamedBytes> {
    let path = root.join(BASELINE_FILE);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        // The entry cannot be looked up (a root without search permission):
        // the root's listing decides whether it exists.
        Err(error) => listed(root, BASELINE_FILE).then(|| NamedBytes {
            name: BASELINE_FILE.to_owned(),
            bytes: Err(error),
        }),
        Ok(_) => Some(NamedBytes::read(BASELINE_FILE, &path)),
    }
}

/// `dir` can be listed and holds an entry named exactly `name`.
fn listed(dir: &Path, name: &str) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name() == name)
    })
}

/// The whole config validated: [`ProjectConfig`] (the top level,
/// `[project]`, `[ids]`, `[paths]`) and [`CheckConfig`]. Else a
/// `cannot-check` report with one cause per distinct error, each
/// `<name>:<line>` (or `<name>`), in the mode `[check] mode` gives when it
/// can be read, else `enforce`.
pub fn load_config(config: &NamedBytes) -> Result<(ProjectConfig, CheckConfig), Box<Report>> {
    let name = config.name.as_str();
    let bytes = match &config.bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            return Err(cannot(
                Mode::default(),
                cause(name, None, format!("cannot read the config: {error}")),
            ));
        }
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Err(cannot(
            Mode::default(),
            cause(name, None, "the file is not UTF-8".to_owned()),
        ));
    };
    let project = ProjectConfig::from_toml(text);
    let check_config = CheckConfig::from_toml(text);
    let mode = check_config
        .as_ref()
        .map_or(Mode::default(), |config| config.mode);
    match (project, check_config) {
        (Ok(project), Ok(check_config)) => Ok((project, check_config)),
        (project, check_config) => {
            // `ProjectConfig` stops at its first error; `[ids]` and
            // `[paths]` read on their own may name more. The same error
            // twice is one cause (`Report::cannot` drops repeats).
            let mut causes = Vec::new();
            if let Err(error) = project {
                causes.push(cause(name, error.line, error.message));
            }
            if let Err(error) = IdScheme::from_toml(text) {
                causes.push(cause(name, error.line, error.message));
            }
            if let Err(error) = Paths::from_toml(text) {
                causes.push(cause(name, error.line, error.message));
            }
            if let Err(error) = check_config {
                causes.push(cause(name, error.line, error.message));
            }
            Err(Box::new(Report::cannot(mode, causes)))
        }
    }
}

/// [`load_config`], then the baseline when one is given (none: no debt).
/// A config error stops before the baseline; a baseline that cannot be
/// read or is invalid gives a `cannot-check` report in the config's mode.
pub fn load_check(
    config: &NamedBytes,
    baseline: Option<&NamedBytes>,
) -> Result<CheckSetup, Box<Report>> {
    let (project, check_config) = load_config(config)?;
    let baseline = match baseline {
        None => Baseline::empty(),
        Some(baseline) => {
            read_baseline(baseline).map_err(|cause| cannot(check_config.mode, cause))?
        }
    };
    Ok(CheckSetup {
        project,
        config: check_config,
        baseline,
    })
}

/// A `cannot-check` report of one cause.
fn cannot(mode: Mode, cause: Cause) -> Box<Report> {
    Box::new(Report::cannot(mode, vec![cause]))
}

fn read_baseline(baseline: &NamedBytes) -> Result<Baseline, Cause> {
    let name = baseline.name.as_str();
    let bytes = baseline
        .bytes
        .as_ref()
        .map_err(|error| cause(name, None, format!("cannot read the baseline: {error}")))?;
    let text = std::str::from_utf8(bytes).map_err(|_| {
        cause(
            name,
            None,
            "cannot read the baseline: the file is not UTF-8".to_owned(),
        )
    })?;
    Baseline::from_toml(text).map_err(|error| cause(name, error.line, error.message))
}

/// Lists `source`, reads and parses every listed file (a parser panic is
/// caught per file and reported as a read error), and records the walk's
/// problems: missing roots, directories that could not be listed (`""`:
/// the root itself), names skipped for not being UTF-8.
pub fn check_input(source: &dyn Source, scheme: &IdScheme) -> CheckInput {
    let mut input = CheckInput::default();
    let listing = match source.list() {
        Ok(listing) => listing,
        Err(_) => {
            input.problems.push(Problem {
                kind: ProblemKind::UnreadableDir,
                path: String::new(),
            });
            return input;
        }
    };
    for root in listing.missing_roots {
        input.problems.push(Problem {
            kind: ProblemKind::MissingRoot,
            path: root,
        });
    }
    for dir in listing.unreadable_dirs {
        input.problems.push(Problem {
            kind: ProblemKind::UnreadableDir,
            path: dir,
        });
    }
    for _ in 0..listing.skipped_names {
        input.problems.push(Problem {
            kind: ProblemKind::SkippedName,
            path: String::new(),
        });
    }
    for path in listing.paths {
        let file = match source.read(&path) {
            Ok(bytes) => {
                let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
                    specengine_core::parse(&path, &bytes, scheme)
                }));
                match parsed {
                    Ok(parsed) => CheckFile::parsed(path, bytes, parsed),
                    Err(_) => CheckFile::unreadable(path, "the spec parser panicked on this file"),
                }
            }
            Err(error) => CheckFile::unreadable(path, error.to_string()),
        };
        input.files.push(file);
    }
    input
}

/// The check of `source` with a loaded setup, `today` as `YYYY-MM-DD`.
pub fn check_source(source: &dyn Source, setup: &CheckSetup, today: &str) -> Report {
    let input = check_input(source, &setup.project.scheme);
    check::run(
        &input,
        &setup.project.scheme,
        &setup.project.paths,
        &setup.config,
        &setup.baseline,
        today,
    )
}

/// [`check_source`] over the working tree under `root`; a root that cannot
/// be read gives a `cannot-check` report whose cause is `.`.
pub fn check_tree(root: &Path, setup: &CheckSetup, today: &str) -> Report {
    match WorkingTree::new(root, &setup.project.paths) {
        Ok(tree) => check_source(&tree, setup, today),
        Err(error) => Report::cannot(setup.config.mode, vec![root_cause(&error)]),
    }
}

/// The whole check of the working tree under `root`: the config from the
/// file `config` (named by its file name), the baseline from `baseline`
/// (named as passed), else [`BASELINE_FILE`] at the root when an entry
/// exists there, `today` as `YYYY-MM-DD`. A missing root, an unreadable or
/// invalid config or baseline gives a `cannot-check` report (exit 2)
/// without walking. [`load_check`] and [`check_tree`].
pub fn check_worktree(root: &Path, config: &Path, baseline: Option<&Path>, today: &str) -> Report {
    let config_name = config.file_name().map_or_else(
        || config.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let config = NamedBytes::read(config_name, config);
    let baseline = match baseline {
        Some(path) => Some(NamedBytes::read(path.display().to_string(), path)),
        None => default_baseline(root),
    };
    match load_check(&config, baseline.as_ref()) {
        Ok(setup) => check_tree(root, &setup, today),
        Err(report) => *report,
    }
}

/// The root cannot be read: the cause `.`, the error without its path.
fn root_cause(error: &StoreError) -> Cause {
    let message = match error {
        StoreError::Io { source, .. } => source.to_string(),
        other => other.to_string(),
    };
    cause(".", None, format!("the root cannot be read: {message}"))
}

/// `file:line` (or `file`) and the message.
fn cause(file: &str, line: Option<usize>, message: String) -> Cause {
    Cause {
        path: match line {
            Some(line) => format!("{file}:{line}"),
            None => file.to_owned(),
        },
        message,
    }
}

/// Today's UTC date as `YYYY-MM-DD`, for callers that are not given one.
pub fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() / 86_400);
    check::date_from_unix_days(i64::try_from(days).unwrap_or(0))
}
