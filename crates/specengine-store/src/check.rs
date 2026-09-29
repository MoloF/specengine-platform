//! The fresh-parse loader of `spec check` (docs/canon/spec-check.md; its API
//! in this crate's README): walk a [`Source`] by `[paths]`, read and
//! parse every listed file into a [`CheckInput`], and run
//! [`specengine_core::check::run`]. No database and no daemon: CI and the
//! pre-commit hook can run it. Nothing is written.

use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use specengine_core::check::{
    self, Baseline, Cause, CheckConfig, CheckFile, CheckInput, Mode, Problem, ProblemKind, Report,
};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;

use crate::source::{Source, WorkingTree};

/// The baseline read from the worktree root when none is passed.
pub const BASELINE_FILE: &str = ".spec-debt.toml";

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

/// The whole check of the working tree under `root`: `[ids]`, `[paths]`
/// and the check tables from the file `config`, the baseline from
/// `baseline` (else [`BASELINE_FILE`] at the root when it exists), `today`
/// as `YYYY-MM-DD`. A missing root, an unreadable or invalid config or
/// baseline gives a `cannot-check` report (exit 2) without walking.
pub fn check_worktree(root: &Path, config: &Path, baseline: Option<&Path>, today: &str) -> Report {
    let config_name = config.display().to_string();
    let text = match fs::read_to_string(config) {
        Ok(text) => text,
        Err(error) => {
            return Report::cannot(
                Mode::default(),
                vec![cause(
                    &config_name,
                    None,
                    format!("cannot read the config: {error}"),
                )],
            );
        }
    };
    let check_config = CheckConfig::from_toml(&text);
    let mode = check_config
        .as_ref()
        .map_or(Mode::default(), |config| config.mode);
    let scheme =
        IdScheme::from_toml(&text).map_err(|error| cause(&config_name, error.line, error.message));
    let paths =
        Paths::from_toml(&text).map_err(|error| cause(&config_name, error.line, error.message));
    let check_config = check_config.map_err(|error| cause(&config_name, error.line, error.message));
    let (scheme, paths, check_config) = match (scheme, paths, check_config) {
        (Ok(scheme), Ok(paths), Ok(config)) => (scheme, paths, config),
        (scheme, paths, config) => {
            let causes = [scheme.err(), paths.err(), config.err()]
                .into_iter()
                .flatten()
                .collect();
            return Report::cannot(mode, causes);
        }
    };

    let baseline = match read_baseline(root, baseline) {
        Ok(baseline) => baseline,
        Err(cause) => return Report::cannot(mode, vec![cause]),
    };
    let tree = match WorkingTree::new(root, &paths) {
        Ok(tree) => tree,
        Err(error) => {
            return Report::cannot(
                mode,
                vec![cause(
                    &root.display().to_string(),
                    None,
                    format!("the root cannot be read: {error}"),
                )],
            );
        }
    };
    let input = check_input(&tree, &scheme);
    check::run(&input, &scheme, &paths, &check_config, &baseline, today)
}

/// The baseline passed (it must exist), else the root's when present, else
/// none.
fn read_baseline(root: &Path, passed: Option<&Path>) -> Result<Baseline, Cause> {
    let path = match passed {
        Some(path) => path.to_path_buf(),
        None => {
            let path = root.join(BASELINE_FILE);
            match fs::symlink_metadata(&path) {
                Ok(_) => path,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Baseline::empty());
                }
                Err(error) => {
                    return Err(cause(
                        &path.display().to_string(),
                        None,
                        format!("cannot read the baseline: {error}"),
                    ));
                }
            }
        }
    };
    let name = path.display().to_string();
    let text = fs::read_to_string(&path)
        .map_err(|error| cause(&name, None, format!("cannot read the baseline: {error}")))?;
    Baseline::from_toml(&text).map_err(|error| cause(&name, error.line, error.message))
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
