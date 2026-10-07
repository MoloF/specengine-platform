//! `spec export state [--out PATH]` and `spec import-state FILE` (canon
//! `queue-backup`, "Commands"; tasks and runs: canon `tasks`, "Backup"):
//! the backup and restore of the queue, the one state git cannot rebuild.
//! The dump's format: [`crate::state_file`].
//!
//! - **Export** reads every row of the queue's tables (proposals, tasks,
//!   runs, events) as stored, every repository
//!   of the root's slug, in one read transaction (never `list_readable`: an
//!   unreadable row goes into the dump as it is); a row of another project
//!   is exit 2 naming it. No consent, no index refresh. The destination,
//!   `--out` (relative to the current directory, its parent existing) or
//!   `<data dir>/backups/<slug>-<YYYYMMDDTHHMMSSZ>.jsonl` (UTC, the injected
//!   clock; `backups/` made 0700), never lies inside the repository: the
//!   root's worktree top, every worktree of its repository, its common dir
//!   (git finding no repository from the root: the root; any other git
//!   failure: exit 2), judged by its nearest existing ancestor, canonical,
//!   by path and by device and inode (a firmlink, a bind mount). The dump
//!   goes to `<path>.partial` (created new, 0600), synced, then takes
//!   `<path>` only while no such name exists (a hard link, never replacing
//!   one; a file system without hard links: a rename after the check), the
//!   partial removed, the directory synced; any failure removes the
//!   partial and the directories this run made. An exit 2 writes nothing.
//! - **Import**, in order, nothing written before step 5: (1) `main`
//!   refuses a stdin that is no terminal before the file is opened; (2)
//!   the whole file, a regular one (else exit 2 unread), is read and
//!   checked; (3) every table must be empty (any project's rows; a task
//!   alone makes it occupied); (4) one
//!   `[y/N]` question; (5) one `Immediate`
//!   transaction re-checks (3) and inserts every row as given, logging no
//!   event of its own. Nothing else is touched: no worktree, no git, no
//!   completion or apply (an `approved` row stays `approved` until `spec
//!   approve` completes it), no index row, never the DB file itself.
//!
//! Both read only the root's own `specengine.toml` (`--config` another:
//! exit 2) and neither writes under the project root.

use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_store::{
    GitEnv, GitError, QueueCounts, Restore, SqliteQueue, StoredQueue, WorktreeGit,
};

use crate::apply::Consent;
use crate::export::identity;
use crate::location::{checked_data_dir, prepared_data_dir, resolve_nonexistent};
use crate::project::{ProjectRoot, discover};
use crate::proposals::{checked_now, escaped_error, queue_cannot, require_root_config};
use crate::state_file::{parse, render};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// The default destination's directory under the data directory.
const BACKUPS_DIR: &str = "backups";

/// `spec export state` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportStateRequest {
    /// `--out PATH`, relative to the current directory; `None`:
    /// `<data dir>/backups/<slug>-<YYYYMMDDTHHMMSSZ>.jsonl`.
    pub out: Option<PathBuf>,
    /// The injected clock, `YYYY-MM-DDTHH:MM:SSZ`: the default file's name.
    pub now: String,
    /// The caller's environment: git finds the root's worktree top without
    /// its local `GIT_*` variables.
    pub git: GitEnv,
}

/// What `spec export state` wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportStateOutcome {
    /// The dump written, absolute.
    pub path: String,
    pub proposals: u64,
    pub tasks: u64,
    pub runs: u64,
    pub events: u64,
    pub messages: Vec<Message>,
}

/// `spec import-state` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportStateRequest {
    /// `FILE`, relative to the current directory.
    pub file: PathBuf,
}

/// What `spec import-state` restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportStateOutcome {
    /// The database restored into.
    pub db: String,
    /// The rows restored: none when refused.
    pub proposals: u64,
    pub tasks: u64,
    pub runs: u64,
    pub events: u64,
    /// Why nothing was restored (exit 1): the answer was not `y`.
    pub refusal: Option<String>,
    pub messages: Vec<Message>,
}

impl ImportStateOutcome {
    /// Exit 0, or 1 when the owner declined.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }
}

/// `spec export state`: writes the dump of the root slug's whole queue.
pub fn export_state(
    env: &Env,
    globals: &Globals,
    request: &ExportStateRequest,
) -> Result<ExportStateOutcome, CliError> {
    run_export(env, globals, request).map_err(escaped_error)
}

fn run_export(
    env: &Env,
    globals: &Globals,
    request: &ExportStateRequest,
) -> Result<ExportStateOutcome, CliError> {
    let project = discover(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    let data_dir = checked_data_dir(env, &project)?;
    let path = match &request.out {
        Some(out) => env.cwd.join(out),
        None => {
            let stamp: String = checked_now(&request.now)?
                .chars()
                .filter(|c| !matches!(c, '-' | ':'))
                .collect();
            data_dir
                .join(BACKUPS_DIR)
                .join(format!("{slug}-{stamp}.jsonl"))
        }
    };
    let bounds = bounds(&project, &request.git, &path)?;
    let resolved = resolve_nonexistent(&path).map_err(|error| {
        CliError::spec(format!(
            "cannot resolve the destination {}: {error}; nothing written",
            path.display()
        ))
    })?;
    if let Some(bound) = bound_holding(&resolved, &bounds) {
        return Err(CliError::spec(format!(
            "the destination {} lies inside {}: a dump holds proposal texts, paths and \
             identities and never goes into the repository; nothing written",
            path.display(),
            bound.what
        )));
    }
    if fs::symlink_metadata(&path).is_ok() {
        return Err(exists(&path));
    }
    let parent = path.parent().unwrap_or(Path::new("/"));
    if request.out.is_some() && !fs::metadata(parent).is_ok_and(|meta| meta.is_dir()) {
        return Err(CliError::spec(format!(
            "the destination's directory {} does not exist: `--out` names a new file in an \
             existing directory; nothing written",
            parent.display()
        )));
    }
    let db = data_dir.join(format!("{slug}.db"));
    let state = match SqliteQueue::open_existing(&db, &slug).map_err(queue_cannot)? {
        Some(queue) => queue.stored_rows().map_err(queue_cannot)?,
        None => StoredQueue::default(),
    };
    foreign_row(&state, &slug, &db)?;
    let dump = render(&slug, &state).map_err(|error| {
        CliError::spec(format!("cannot render the dump: {error}; nothing written"))
    })?;
    let made = if request.out.is_none() {
        make_backups_dir(parent)?
    } else {
        Vec::new()
    };
    let messages = write_new(&path, &dump).map_err(|error| unmade(error, &made))?;
    let counts = state.counts();
    Ok(ExportStateOutcome {
        path: path.display().to_string(),
        proposals: counts.proposals,
        tasks: counts.tasks,
        runs: counts.runs,
        events: counts.events,
        messages,
    })
}

/// A directory a dump never goes inside.
struct Bound {
    /// Canonical where it exists.
    path: PathBuf,
    /// The message's words for it.
    what: String,
    /// Its device and inode, when it exists (Unix).
    identity: Option<(u64, u64)>,
}

impl Bound {
    fn new(path: PathBuf, what: impl FnOnce(&Path) -> String) -> Self {
        let path = fs::canonicalize(&path).unwrap_or(path);
        let identity = fs::metadata(&path).ok().as_ref().and_then(identity);
        Self {
            what: what(&path),
            path,
            identity,
        }
    }

    /// Whether `other` is the same directory: one canonical path, or one
    /// device and inode.
    fn is(&self, other: &Bound) -> bool {
        self.path == other.path || self.identity.is_some_and(|id| other.identity == Some(id))
    }
}

/// The directories a dump to `path` never goes inside (never inside the
/// repository: canon `queue-backup`, "Export", step 1), the root's own
/// worktree top first: then every worktree of its repository (the main
/// one: a `.git` common dir's parent when not bare; every entry of `git
/// worktree list`, one that is the common dir named the git directory),
/// then the common dir. When git finds no repository from the root: the
/// root alone. Any other git failure (a dubious owner, no `git` to run):
/// exit 2 naming it.
fn bounds(project: &ProjectRoot, git_env: &GitEnv, path: &Path) -> Result<Vec<Bound>, CliError> {
    let cannot = |error: GitError| {
        CliError::spec(format!(
            "cannot tell whether the destination {} lies inside the repository of the project \
             root {}: {error}; nothing written",
            path.display(),
            project.root.display()
        ))
    };
    let of_root = |top: &Path| format!("the worktree {} of the project root", top.display());
    let git = WorktreeGit::new(&project.root, git_env).map_err(cannot)?;
    let Some(top) = git.top_if_repository().map_err(cannot)? else {
        return Ok(vec![Bound::new(project.root.clone(), of_root)]);
    };
    let common = git.common_dir().map_err(cannot)?;
    let listed = git.worktrees().map_err(cannot)?;
    let worktree = |dir: &Path| {
        format!(
            "the worktree {} of the project root's repository",
            dir.display()
        )
    };
    let git_dir = |dir: &Path| {
        format!(
            "the git directory {} of the project root's repository",
            dir.display()
        )
    };
    let mut bounds = vec![Bound::new(top, of_root)];
    // A `.git` directory's parent: git's own main worktree. Another name
    // (`--separate-git-dir`, a submodule's `modules/<name>`) says nothing
    // of its parent, which may be the home directory.
    if listed.first().is_some_and(|main| !main.bare)
        && common.file_name() == Some(OsStr::new(".git"))
        && let Some(main) = common.parent()
    {
        bounds.push(Bound::new(main.to_path_buf(), worktree));
    }
    let common = Bound::new(common, git_dir);
    for entry in listed {
        let mut bound = Bound::new(entry.path, worktree);
        // A bare main entry is its git directory; so is a main entry git
        // prints as the common dir itself (`--separate-git-dir`: git knows
        // no other place of the main checkout, from any worktree; a bare
        // repository's entry git did not flag bare): no files checked out
        // there, and the message names it so.
        if entry.bare || bound.is(&common) {
            bound.what = git_dir(&bound.path);
        }
        bounds.push(bound);
    }
    bounds.push(common);
    Ok(bounds)
}

/// The first of `bounds` holding `resolved` (the destination, its nearest
/// existing ancestor canonical): by path, or by one of its existing
/// ancestors being that directory, one device and inode (a macOS firmlink
/// through the Data volume, a bind mount: another path to it).
fn bound_holding<'a>(resolved: &Path, bounds: &'a [Bound]) -> Option<&'a Bound> {
    let ancestors: Vec<(u64, u64)> = resolved
        .ancestors()
        .filter_map(|dir| fs::metadata(dir).ok())
        .filter_map(|meta| identity(&meta))
        .collect();
    bounds.iter().find(|bound| {
        resolved.starts_with(&bound.path)
            || bound.identity.is_some_and(|id| ancestors.contains(&id))
    })
}

/// Exit 2: `path` exists (a file, a directory, a symlink); its bytes
/// untouched.
fn exists(path: &Path) -> CliError {
    CliError::spec(format!(
        "the destination {} exists: a dump never replaces a file; name a new one; nothing \
         written",
        path.display()
    ))
}

/// Exit 2 naming the first row of another project than `slug` (the
/// database is the slug's own: such a row was not written by this queue).
fn foreign_row(state: &StoredQueue, slug: &str, db: &Path) -> Result<(), CliError> {
    let shown = |project: Option<&str>| match project {
        Some(project) => format!("of the project `{project}`"),
        None => "of no project (NULL)".to_owned(),
    };
    let foreign = state
        .proposals
        .iter()
        .find(|row| row.project() != Some(slug))
        .map(|row| {
            format!(
                "proposal `{}` in {} is {}",
                row.id().unwrap_or("NULL"),
                db.display(),
                shown(row.project())
            )
        })
        .or_else(|| {
            state
                .tasks
                .iter()
                .find(|row| row.project() != Some(slug))
                .map(|row| {
                    format!(
                        "task `{}` in {} is {}",
                        row.id().unwrap_or("NULL"),
                        db.display(),
                        shown(row.project())
                    )
                })
        })
        .or_else(|| {
            state
                .runs
                .iter()
                .find(|row| {
                    !state
                        .tasks
                        .iter()
                        .any(|task| task.id().is_some() && task.id() == row.task_id())
                })
                .map(|row| {
                    format!(
                        "run {} of `{}` in {} belongs to no task of the project",
                        row.run,
                        row.task_id().unwrap_or("NULL"),
                        db.display()
                    )
                })
        })
        .or_else(|| {
            state
                .events
                .iter()
                .find(|row| row.project() != Some(slug))
                .map(|row| {
                    format!(
                        "event {} in {} is {}",
                        row.seq,
                        db.display(),
                        shown(row.project())
                    )
                })
        });
    match foreign {
        Some(row) => Err(CliError::spec(format!(
            "{row}, not `{slug}`'s: the dump holds one project's queue; nothing written"
        ))),
        None => Ok(()),
    }
}

/// The default destination's `backups/` (0700) and, where absent, the
/// data directory above it (as `create_dir_all` makes it); the directories
/// this run made, outermost first. On failure, nothing made is left.
fn make_backups_dir(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    let missing: Vec<&Path> = dir
        .ancestors()
        .take_while(|ancestor| !ancestor.as_os_str().is_empty() && fs::metadata(ancestor).is_err())
        .collect();
    let mut made = Vec::new();
    for &missing_dir in missing.iter().rev() {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        if missing_dir == dir {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        match builder.create(missing_dir) {
            Ok(()) => made.push(missing_dir.to_path_buf()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && missing_dir.is_dir() => {}
            Err(error) => {
                let what = if missing_dir == dir {
                    format!("the backup directory {}", dir.display())
                } else {
                    format!(
                        "{} for the backup directory {}",
                        missing_dir.display(),
                        dir.display()
                    )
                };
                let error =
                    CliError::spec(format!("cannot create {what}: {error}; nothing written"));
                return Err(unmade(error, &made));
            }
        }
    }
    Ok(made)
}

/// `error`, the directories this run `made` removed first (innermost
/// first, only while empty); one that stays is named.
fn unmade(mut error: CliError, made: &[PathBuf]) -> CliError {
    for dir in made.iter().rev() {
        if let Err(cause) = fs::remove_dir(dir) {
            error.message.push_str(&format!(
                " (the directory {} this run made could not be removed: {cause})",
                dir.display()
            ));
        }
    }
    error
}

/// Writes `bytes` to the new file `path`: `<path>.partial` created new
/// (0600), written and synced, then linked to `path` only while no such
/// name exists (`link` never replaces one), and removed. On any failure
/// the partial is removed and `path`, when it exists, untouched. A file
/// system without hard links (exFAT, some network shares) gets a rename
/// after the check instead. The directory is synced after, so the name
/// lasts. The warnings: the dump is written, but its partial could not be
/// removed, or its directory synced.
fn write_new(path: &Path, bytes: &[u8]) -> Result<Vec<Message>, CliError> {
    let mut partial = OsString::from(path.as_os_str());
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&partial).map_err(|error| {
        let hint = if error.kind() == io::ErrorKind::AlreadyExists {
            " (left by an export that stopped: remove it)"
        } else {
            ""
        };
        CliError::spec(format!(
            "cannot create {}: {error}{hint}; nothing written",
            partial.display()
        ))
    })?;
    let synced = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let placed = match synced {
        Err(error) => Err(CliError::spec(format!(
            "cannot write {}: {error}; nothing written",
            partial.display()
        ))),
        Ok(()) => place(&partial, path),
    };
    let removed = match fs::remove_file(&partial) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Some(error),
        _ => None,
    };
    placed?;
    let mut warnings: Vec<Message> = removed
        .map(|error| {
            Message::Warning(format!(
                "cannot remove {} (the dump {} is whole): {error}",
                partial.display(),
                path.display()
            ))
        })
        .into_iter()
        .collect();
    let dir = path.parent().unwrap_or(Path::new("/"));
    if let Err(error) = fs::File::open(dir).and_then(|dir| dir.sync_all()) {
        warnings.push(Message::Warning(format!(
            "cannot sync the directory {} (the dump {} is whole; its name may not outlast a \
             power loss): {error}",
            dir.display(),
            path.display()
        )));
    }
    Ok(warnings)
}

/// Gives the written `partial` the name `path`, only while no such name
/// exists.
fn place(partial: &Path, path: &Path) -> Result<(), CliError> {
    let cannot = |error: io::Error| {
        CliError::spec(format!(
            "cannot move {} to {}: {error}; nothing written",
            partial.display(),
            path.display()
        ))
    };
    match fs::hard_link(partial, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(exists(path)),
        Err(_) if fs::symlink_metadata(path).is_ok() => Err(exists(path)),
        Err(_) => fs::rename(partial, path).map_err(cannot),
    }
}

/// `spec import-state`: restores the dump `FILE` into the root slug's
/// empty queue after the owner's `consent` (`main` has refused a stdin that
/// is no terminal).
pub fn import_state(
    env: &Env,
    globals: &Globals,
    request: &ImportStateRequest,
    consent: Consent<'_>,
) -> Result<ImportStateOutcome, CliError> {
    run_import(env, globals, request, consent).map_err(escaped_error)
}

fn run_import(
    env: &Env,
    globals: &Globals,
    request: &ImportStateRequest,
    consent: Consent<'_>,
) -> Result<ImportStateOutcome, CliError> {
    let project = discover(env, globals)?;
    require_root_config(env, globals, &project)?;
    let slug = project.slug()?.to_owned();
    // Step 2: the whole file.
    let label = request.file.display().to_string();
    let bytes = read_regular(&env.cwd.join(&request.file), &label)?;
    let state = parse(&label, &bytes, &slug)?;
    let counts = state.counts();
    // Step 3: an empty queue; nothing created yet.
    let data_dir = checked_data_dir(env, &project)?;
    let db = data_dir.join(format!("{slug}.db"));
    let held = match SqliteQueue::open_existing(&db, &slug).map_err(queue_cannot)? {
        Some(queue) => queue.counts().map_err(queue_cannot)?,
        None => QueueCounts::default(),
    };
    if !held.is_empty() {
        return Err(occupied(&slug, &db, held));
    }
    // Step 4: the owner's answer.
    let question = format!(
        "restore {} proposal(s), {} task(s), {} run(s) and {} event(s) of {slug} from {label} \
         into {}? [y/N]",
        counts.proposals,
        counts.tasks,
        counts.runs,
        counts.events,
        db.display()
    );
    if !consent(&escape_controls(&one_line(&question))) {
        return Ok(ImportStateOutcome {
            db: db.display().to_string(),
            proposals: 0,
            tasks: 0,
            runs: 0,
            events: 0,
            refusal: Some("not restored: the answer was not `y`; nothing changed".to_owned()),
            messages: Vec::new(),
        });
    }
    // Step 5: one transaction, the emptiness checked again under its lock.
    prepared_data_dir(env, &project)?;
    let mut queue = SqliteQueue::open(&db, &slug).map_err(queue_cannot)?;
    match queue.restore(&state).map_err(queue_cannot)? {
        Restore::Restored => Ok(ImportStateOutcome {
            db: db.display().to_string(),
            proposals: counts.proposals,
            tasks: counts.tasks,
            runs: counts.runs,
            events: counts.events,
            refusal: None,
            messages: Vec::new(),
        }),
        Restore::Occupied(held) => Err(occupied(&slug, &db, held)),
    }
}

/// The whole regular file `path` (`label` as given; symlinks followed).
/// Anything else — a directory, a FIFO (whose open would block), a device
/// (`/dev/zero` never ends) — is exit 2, unread; so is a name swapped for
/// one between the check and the open.
fn read_regular(path: &Path, label: &str) -> Result<Vec<u8>, CliError> {
    let cannot = |error: io::Error| CliError::spec(format!("cannot read {label}: {error}"));
    let regular = |meta: fs::Metadata| {
        if meta.is_file() {
            Ok(())
        } else {
            Err(CliError::spec(format!(
                "cannot read {label}: not a regular file (a dump is one file)"
            )))
        }
    };
    regular(fs::metadata(path).map_err(cannot)?)?;
    let mut file = fs::File::open(path).map_err(cannot)?;
    regular(file.metadata().map_err(cannot)?)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(cannot)?;
    Ok(bytes)
}

/// Exit 2: the queue holds rows (any project's).
fn occupied(slug: &str, db: &Path, held: QueueCounts) -> CliError {
    CliError::spec(format!(
        "the queue of `{slug}` in {db} holds {} proposal(s), {} task(s), {} run(s), {} event(s): \
         import-state restores only into an empty queue (a fresh data directory, or {db} moved \
         aside); nothing changed",
        held.proposals,
        held.tasks,
        held.runs,
        held.events,
        db = db.display()
    ))
}

/// `wrote <path>: <p> proposal(s), <t> task(s), <r> run(s), <e> event(s)`.
pub(crate) fn render_export_text(outcome: &ExportStateOutcome) -> String {
    escape_controls(&format!(
        "{}\n",
        one_line(&format!(
            "wrote {}: {} proposal(s), {} task(s), {} run(s), {} event(s)",
            outcome.path, outcome.proposals, outcome.tasks, outcome.runs, outcome.events
        ))
    ))
}

/// `restored <p> proposal(s), <t> task(s), <r> run(s), <e> event(s) into
/// <db>`; nothing when declined.
pub(crate) fn render_import_text(outcome: &ImportStateOutcome) -> String {
    if outcome.refusal.is_some() {
        return String::new();
    }
    escape_controls(&format!(
        "{}\n",
        one_line(&format!(
            "restored {} proposal(s), {} task(s), {} run(s), {} event(s) into {}",
            outcome.proposals, outcome.tasks, outcome.runs, outcome.events, outcome.db
        ))
    ))
}

#[derive(Serialize)]
struct ExportJson<'a> {
    path: &'a str,
    proposals: u64,
    tasks: u64,
    runs: u64,
    events: u64,
}

/// `{path, proposals, tasks, runs, events}`.
impl Serialize for ExportStateOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ExportJson {
            path: &self.path,
            proposals: self.proposals,
            tasks: self.tasks,
            runs: self.runs,
            events: self.events,
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct ImportJson<'a> {
    db: &'a str,
    proposals: u64,
    tasks: u64,
    runs: u64,
    events: u64,
}

/// `{db, proposals, tasks, runs, events}`: the rows restored (all 0 when
/// declined).
impl Serialize for ImportStateOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ImportJson {
            db: &self.db,
            proposals: self.proposals,
            tasks: self.tasks,
            runs: self.runs,
            events: self.events,
        }
        .serialize(serializer)
    }
}
