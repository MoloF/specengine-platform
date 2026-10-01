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
//!
//! The staged check ([`check_staged_with_notes`]) and the working tree's
//! against `HEAD` ([`check_changed_with_notes`]) are judged against their
//! base, `HEAD` (docs/features/spec-cli-introduced.md,
//! docs/features/spec-cli-changed.md; `crate::base`), and return notes
//! beside their report.

use std::fs;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use specengine_core::check::{
    self, Base, Baseline, Cause, CheckConfig, CheckFile, CheckInput, Mode, Problem, ProblemKind,
    Report,
};
use specengine_core::{IdSchemeToml, Paths, ProjectConfig};
use specengine_model::IdScheme;

use crate::base::{Head, HeadWalk, Placed};
use crate::error::StoreError;
use crate::git::{GitEnv, GitFailure, Staged};
use crate::source::{GitIndex, IndexWalk, Listing, Source, WorkingTree};

/// The project's config, at its root (the working tree's for discovery,
/// the index entry for [`check_staged`]).
pub const CONFIG_FILE: &str = "specengine.toml";

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

/// A file the caller passes instead of the root's (`--config`,
/// `--baseline`): its bytes as read, and where it lies, so the staged
/// check can tell whether it lies below the root and find `HEAD`'s file at
/// its root-relative path.
#[derive(Debug)]
pub struct GivenFile {
    /// Named as typed.
    pub bytes: NamedBytes,
    /// Its path on disk (absolute, or relative to the process's current
    /// directory); `None` when unknown: read as outside the root.
    pub path: Option<PathBuf>,
}

/// The answer of a check against `HEAD` (`--staged`, `--changed`): the
/// report and the notes beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedCheck {
    pub report: Report,
    /// `note:` lines' text, never part of the report, in order: `HEAD`'s
    /// mode unknown, `HEAD`'s stricter mode applied, `HEAD`'s baseline
    /// unknown (each at most once).
    pub notes: Vec<String>,
}

impl StagedCheck {
    fn bare(report: Report) -> Self {
        Self {
            report,
            notes: Vec::new(),
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
    let (paths, problems) = split_listing(listing);
    input.problems = problems;
    for path in paths {
        let file = match source.read(&path) {
            Ok(bytes) => parse_file(path, bytes, scheme),
            Err(error) => CheckFile::unreadable(path, error.to_string()),
        };
        input.files.push(file);
    }
    input
}

/// A listing's paths, and its problems in the check's order: missing
/// roots, directories that could not be listed, skipped names.
pub(crate) fn split_listing(listing: Listing) -> (Vec<String>, Vec<Problem>) {
    let mut problems = Vec::new();
    for root in listing.missing_roots {
        problems.push(Problem {
            kind: ProblemKind::MissingRoot,
            path: root,
        });
    }
    for dir in listing.unreadable_dirs {
        problems.push(Problem {
            kind: ProblemKind::UnreadableDir,
            path: dir,
        });
    }
    for _ in 0..listing.skipped_names {
        problems.push(Problem {
            kind: ProblemKind::SkippedName,
            path: String::new(),
        });
    }
    (listing.paths, problems)
}

/// A read file parsed; a parser panic is a read error of that file.
pub(crate) fn parse_file(path: String, bytes: Vec<u8>, scheme: &IdScheme) -> CheckFile {
    let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
        specengine_core::parse(&path, &bytes, scheme)
    }));
    match parsed {
        Ok(parsed) => CheckFile::parsed(path, bytes, parsed),
        Err(_) => CheckFile::unreadable(path, "the spec parser panicked on this file"),
    }
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

/// [`check_staged_with_notes`]'s report: a given config or baseline is of
/// unknown location, read as outside the root.
pub fn check_staged(
    root: &Path,
    config: Option<NamedBytes>,
    baseline: Option<NamedBytes>,
    git: &GitEnv,
    today: &str,
) -> Report {
    let unplaced = |bytes| GivenFile { bytes, path: None };
    check_staged_with_notes(
        root,
        config.map(unplaced),
        baseline.map(unplaced),
        git,
        today,
    )
    .report
}

/// The whole check of what `git commit` would record under `root`
/// (docs/features/spec-cli-staged.md), judged against `HEAD`
/// (docs/features/spec-cli-introduced.md): the index git names (with
/// `git`'s environment: `GIT_INDEX_FILE`, `GIT_DIR`), walked by
/// [`GitIndex`]. The config is `config` when given (read from disk by the
/// caller), else the index entry [`CONFIG_FILE`] at the root; the baseline
/// `baseline` when given, else the entry [`BASELINE_FILE`] when staged;
/// never the working tree's. `today` as `YYYY-MM-DD`.
///
/// The base: `HEAD`'s tree under the root (none when unborn), walked and
/// checked under the checked config, its causes dropped; `HEAD`'s config
/// at the checked config's root-relative path gives its mode (the stricter
/// applies), `HEAD`'s baseline at the checked baseline's the new-debt
/// rule; a given file outside the root, or `HEAD`'s file unreadable or
/// invalid, leaves the mode the checked one or lifts the rule, with a
/// note. One `cat-file --batch` session reads, each OID once: the staged
/// config and baseline, `HEAD`'s, the staged documents, then `HEAD`'s
/// documents whose (path, OID) the index lacks.
///
/// Every failure is a `cannot-check` report, without notes: before the
/// config is read (no repository, `git` not runnable or failing,
/// `GIT_INDEX_FILE` naming no file: cause `.`; unmerged paths under the
/// root: a cause each) in `config`'s mode, else `enforce`; the config not
/// staged or not a regular blob ([`CONFIG_FILE`]), the baseline not a
/// regular blob ([`BASELINE_FILE`]), either invalid, as [`load_check`]
/// reports them; git failing on the base, in the checked config's mode
/// (cause `.`); a document's blob missing, at its path; a document `HEAD`
/// lists whose blob is missing or whose object is not a blob (a partial
/// base), at its path, in the checked config's mode, with the checked
/// run's causes and no findings.
/// `HEAD`'s config or baseline unreadable is a note, never a cause.
pub fn check_staged_with_notes(
    root: &Path,
    config: Option<GivenFile>,
    baseline: Option<GivenFile>,
    git: &GitEnv,
    today: &str,
) -> StagedCheck {
    let (config, config_path) = match config {
        Some(given) => (Some(given.bytes), Some(given.path)),
        None => (None, None),
    };
    let (baseline, baseline_path) = match baseline {
        Some(given) => (Some(given.bytes), Some(given.path)),
        None => (None, None),
    };
    let mut staged = match Staged::read(root, git) {
        Ok(staged) => staged,
        Err(causes) => {
            return StagedCheck::bare(Report::cannot(early_mode(config.as_ref()), causes));
        }
    };
    let config = match config {
        Some(config) => config,
        None => match staged_file(&mut staged, CONFIG_FILE) {
            Ok(Some(config)) => config,
            Ok(None) => NamedBytes {
                name: CONFIG_FILE.to_owned(),
                bytes: Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "not staged in the git index",
                )),
            },
            Err(failure) => {
                return StagedCheck::bare(Report::cannot(Mode::default(), vec![failure.cause()]));
            }
        },
    };
    let baseline = match baseline {
        Some(baseline) => Some(baseline),
        None => match staged_file(&mut staged, BASELINE_FILE) {
            Ok(baseline) => baseline,
            Err(failure) => {
                return StagedCheck::bare(Report::cannot(
                    early_mode(Some(&config)),
                    vec![failure.cause()],
                ));
            }
        },
    };
    let setup = match load_check(&config, baseline.as_ref()) {
        Ok(setup) => setup,
        Err(report) => return StagedCheck::bare(*report),
    };
    let checked_mode = setup.config.mode;
    let config_place = place(
        &staged.root,
        config_path,
        "--config",
        &config.name,
        CONFIG_FILE,
    );
    let baseline_name = baseline
        .as_ref()
        .map_or(BASELINE_FILE, |given| given.name.as_str());
    let baseline_place = place(
        &staged.root,
        baseline_path,
        "--baseline",
        baseline_name,
        BASELINE_FILE,
    );
    match read_base(&mut staged, &setup, &config_place, &baseline_place) {
        Ok((head_walk, head, walk)) => {
            let notes = head.notes(&config_place, checked_mode);
            let mut index = GitIndex::from_walk(staged, walk);
            let input = check_input(&index, &setup.project.scheme);
            let report = check::run(
                &input,
                &setup.project.scheme,
                &setup.project.paths,
                &setup.config,
                &setup.baseline,
                today,
            );
            let findings = match head_walk.findings(&mut index, input, &setup, today) {
                Ok(findings) => findings,
                Err(mut causes) => {
                    // A partial base judges nothing: the checked run's
                    // causes and the base's, no findings, no notes.
                    causes.extend(report.cannot_check);
                    return StagedCheck::bare(Report::cannot(checked_mode, causes));
                }
            };
            let base = Base {
                findings,
                baseline: head.baseline,
                mode: head.mode,
            };
            StagedCheck {
                report: check::judge(report, &setup.baseline, &base),
                notes,
            }
        }
        Err(failure) => StagedCheck::bare(Report::cannot(checked_mode, vec![failure.cause()])),
    }
}

/// The whole check of the working tree under `root`, walked as
/// [`check_tree`] walks it (untracked and ignored files included, the git
/// index never read), judged against `HEAD`
/// (docs/features/spec-cli-changed.md): `spec check --changed`. The config
/// is `config` when given (read from disk by the caller), else
/// [`CONFIG_FILE`] at the root on disk; the baseline `baseline` when
/// given, else [`default_baseline`]. `today` as `YYYY-MM-DD`.
///
/// The config and the baseline are validated first, before git runs: their
/// causes as [`load_check`] reports them. The base is
/// [`check_staged_with_notes`]'s: `HEAD`'s tree under the root, walked and
/// checked under the checked config, its causes dropped; `HEAD`'s config
/// at the checked config's root-relative path gives its mode (the stricter
/// applies), `HEAD`'s baseline at the checked baseline's (the root's
/// [`BASELINE_FILE`] when none is given, even absent on disk) the new-debt
/// rule; the same notes. Git, in the root: `rev-parse`, when born
/// `ls-tree`, one `cat-file --batch` session reading, each OID once,
/// `HEAD`'s config and baseline, then every blob `HEAD` lists under the
/// checked `[paths]`. A `HEAD` file reuses the checked parse only when the
/// file at its path was read without error and holds exactly its bytes.
///
/// Every failure after the config is a `cannot-check` report in the
/// checked config's mode, without notes: the root not in a git working
/// tree, `git` not runnable or failing (one cause at `.`; never the plain
/// check instead); a listed `HEAD` object missing or not a blob (a partial
/// base), at its path, with the checked run's causes and no findings.
pub fn check_changed_with_notes(
    root: &Path,
    config: Option<GivenFile>,
    baseline: Option<GivenFile>,
    git: &GitEnv,
    today: &str,
) -> StagedCheck {
    let (config, config_path) = match config {
        Some(given) => (given.bytes, Some(given.path)),
        None => (NamedBytes::read(CONFIG_FILE, &root.join(CONFIG_FILE)), None),
    };
    let (baseline, baseline_path) = match baseline {
        Some(given) => (Some(given.bytes), Some(given.path)),
        None => (default_baseline(root), None),
    };
    let setup = match load_check(&config, baseline.as_ref()) {
        Ok(setup) => setup,
        Err(report) => return StagedCheck::bare(*report),
    };
    let checked_mode = setup.config.mode;
    let mut session = match Staged::head_only(root, git) {
        Ok(session) => session,
        Err(cause) => return StagedCheck::bare(Report::cannot(checked_mode, vec![cause])),
    };
    let config_place = place(
        &session.root,
        config_path,
        "--config",
        &config.name,
        CONFIG_FILE,
    );
    let baseline_name = baseline
        .as_ref()
        .map_or(BASELINE_FILE, |given| given.name.as_str());
    let baseline_place = place(
        &session.root,
        baseline_path,
        "--baseline",
        baseline_name,
        BASELINE_FILE,
    );
    let (head_walk, head) = match read_head(&mut session, &setup, &config_place, &baseline_place) {
        Ok(read) => read,
        Err(failure) => {
            return StagedCheck::bare(Report::cannot(checked_mode, vec![failure.cause()]));
        }
    };
    let tree = match WorkingTree::new(root, &setup.project.paths) {
        Ok(tree) => tree,
        Err(error) => {
            return StagedCheck::bare(Report::cannot(checked_mode, vec![root_cause(&error)]));
        }
    };
    let input = check_input(&tree, &setup.project.scheme);
    let report = check::run(
        &input,
        &setup.project.scheme,
        &setup.project.paths,
        &setup.config,
        &setup.baseline,
        today,
    );
    let findings = match head_walk.findings_by_bytes(session.objects, input, &setup, today) {
        Ok(findings) => findings,
        Err(mut causes) => {
            // A partial base judges nothing: the checked run's causes and
            // the base's, no findings, no notes.
            causes.extend(report.cannot_check);
            return StagedCheck::bare(Report::cannot(checked_mode, causes));
        }
    };
    let notes = head.notes(&config_place, checked_mode);
    let base = Base {
        findings,
        baseline: head.baseline,
        mode: head.mode,
    };
    StagedCheck {
        report: check::judge(report, &setup.baseline, &base),
        notes,
    }
}

/// Where a checked config or baseline lies, so `HEAD`'s counterpart can be
/// found: given under `flag` (`Some`, its path on disk known or not) at
/// [`Placed::given`] against `root` (canonical), named `name`; else the
/// root's own `file`.
fn place(
    root: &Path,
    given: Option<Option<PathBuf>>,
    flag: &'static str,
    name: &str,
    file: &str,
) -> Placed {
    match given {
        Some(path) => Placed::given(root, flag, name, path.as_deref()),
        None => Placed::root_file(file),
    }
}

/// Every git read of a `--changed` check, in the session's order: `HEAD`
/// probed and listed, `HEAD`'s config and baseline, every blob `HEAD`
/// lists under the checked `[paths]`; then the session ends.
fn read_head(
    session: &mut Staged,
    setup: &CheckSetup,
    config: &Placed,
    baseline: &Placed,
) -> Result<(HeadWalk, Head), GitFailure> {
    let head = Head::read(session, config, baseline, setup.config.mode)?;
    let head_walk = head.walk_by(&setup.project.paths.walk_scope());
    head_walk.read(session)?;
    session.finish()?;
    Ok((head_walk, head))
}

/// Every git read of a staged check after its config and baseline, in the
/// session's order: `HEAD` probed and listed, `HEAD`'s config and
/// baseline, the index's listed blobs, `HEAD`'s listed blobs the index
/// lacks at their path; then the session ends.
fn read_base(
    staged: &mut Staged,
    setup: &CheckSetup,
    config: &Placed,
    baseline: &Placed,
) -> Result<(HeadWalk, Head, IndexWalk), GitFailure> {
    let head = Head::read(staged, config, baseline, setup.config.mode)?;
    let walk = IndexWalk::new(&staged.entries, &setup.project.paths);
    walk.read(staged)?;
    let head_walk = head.walk(&walk);
    head_walk.read(staged)?;
    staged.finish()?;
    Ok((head_walk, head, walk))
}

/// The mode of a failure before the config is read: `config`'s when it
/// can be read, else `enforce`.
fn early_mode(config: Option<&NamedBytes>) -> Mode {
    config.map_or(Mode::default(), |config| match load_config(config) {
        Ok((_, check_config)) => check_config.mode,
        Err(report) => report.mode,
    })
}

/// The index entry `name` at the root as the loader's input: `None` when
/// not staged; a symlink, a gitlink or a missing blob as a read error.
fn staged_file(staged: &mut Staged, name: &str) -> Result<Option<NamedBytes>, GitFailure> {
    let Some(entry) = staged.entry(name.as_bytes()) else {
        return Ok(None);
    };
    let (kind, oid) = (entry.kind, entry.oid.clone());
    let bytes = match kind.not_regular() {
        Some(reason) => Err(io::Error::other(reason)),
        None => staged.blob(&oid)?.bytes(),
    };
    Ok(Some(NamedBytes {
        name: name.to_owned(),
        bytes,
    }))
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
