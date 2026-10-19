//! `spec serve` and the daemon's identity in the data directory (task spec
//! `spec-serve`): one daemon per data directory, beside the index
//! databases, never under a root.
//!
//! - `daemon.lock`, empty: its `flock` is the one liveness signal. The
//!   daemon (`specengine-http`) takes it exclusively for its whole life;
//!   the launcher cannot (Rust opens files close-on-exec: a lock taken
//!   before the exec would die with it), and a direct start is covered too.
//! - `daemon.json` ([`DaemonFile`]): where the daemon listens, written by
//!   it after its bind, never removed, trusted only while the lock is held.
//!   Readers take its five keys and ignore others; a missing or mistyped
//!   one makes the file unreadable.
//!
//! [`serve`] is the launcher's decision, nothing started: the roots (the
//! project any command finds, or each DIR as `--root` takes it), the guard
//! (the data directory outside every root), the lookup of the daemon's
//! binary, the probe (a shared lock tried on the existing `daemon.lock`).
//! It creates nothing, neither file nor directory; it runs no thread, no
//! socket, no async runtime. A daemon serving every requested root is
//! answered with its lines; one serving others is refused with the restart
//! command; else `main` execs the daemon ([`Serve::Start`]).

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::location::checked_data_dir;
use crate::project::{ProjectRoot, discover_without_config_hint};
use crate::{CliError, Env, Globals, escape_controls, one_line};

/// The daemon's binary, found beside `spec` or on `PATH`.
pub const DAEMON_BIN: &str = "specengine-http";
/// The lock file in the data directory.
pub const DAEMON_LOCK: &str = "daemon.lock";
/// The discovery file in the data directory.
pub const DAEMON_FILE: &str = "daemon.json";
/// The port when none is given.
pub const DAEMON_PORT: u16 = 7777;
/// The pause between two tries of the lock or two reads of the discovery
/// file.
pub const DAEMON_POLL: Duration = Duration::from_millis(50);
/// How long `spec serve` waits for a locked daemon's discovery file (its
/// winner writes it after the bind).
pub const DAEMON_PUBLISH_WAIT: Duration = Duration::from_secs(2);

/// `daemon.json`: the daemon's pid, its bound port and URL, the projects
/// it serves and when it started; these five keys, in this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonFile {
    /// The daemon's `std::process::id()`.
    pub pid: u32,
    /// The bound port (`--port 0`: the one chosen).
    pub port: u16,
    /// The `listening` line's URL.
    pub url: String,
    /// In `--root` order.
    pub projects: Vec<DaemonProject>,
    /// `YYYY-MM-DDTHH:MM:SSZ`, UTC.
    pub started: String,
}

/// A served project in `daemon.json`: exactly its slug and canonical root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonProject {
    pub slug: String,
    /// Canonical, as `/api/projects` gives it.
    pub root: String,
}

impl DaemonFile {
    /// The file's text: pretty JSON (two-space indent), then a LF.
    pub fn to_text(&self) -> Result<String, String> {
        let mut text = serde_json::to_string_pretty(self)
            .map_err(|error| format!("cannot encode {DAEMON_FILE}: {error}"))?;
        text.push('\n');
        Ok(text)
    }

    /// `<slug> <root>` per served project, joined by `, `, each as
    /// [`daemon_text`] prints it.
    pub fn serving(&self) -> String {
        self.projects
            .iter()
            .map(|project| {
                format!(
                    "{} {}",
                    daemon_text(&project.slug),
                    daemon_text(&project.root)
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The URL as [`daemon_text`] prints it.
    pub fn shown_url(&self) -> String {
        daemon_text(&self.url)
    }
}

/// A text read from `daemon.json` (or a reason quoting it) as printed:
/// control characters and bidirectional marks escaped (the CLI's terminal
/// escaping), on one line.
pub fn daemon_text(text: &str) -> String {
    one_line(&escape_controls(text))
}

/// `daemon.json` of the data directory `dir`, else why it is unreadable
/// (missing, not JSON, a key missing or mistyped).
pub fn read_daemon_file(dir: &Path) -> Result<DaemonFile, String> {
    let bytes = fs::read(dir.join(DAEMON_FILE)).map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

/// The line of a held lock whose discovery file cannot be read (no
/// prefix): `a daemon holds <lock>, but <file> is unreadable: <reason>`,
/// the reason as [`daemon_text`] prints it.
pub fn unpublished_daemon(dir: &Path, reason: &str) -> String {
    format!(
        "a daemon holds {}, but {} is unreadable: {}",
        dir.join(DAEMON_LOCK).display(),
        dir.join(DAEMON_FILE).display(),
        daemon_text(reason)
    )
}

/// What `spec serve` is given besides the globals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeRequest {
    /// The positional DIRs, in order; none: the project any command finds.
    pub dirs: Vec<PathBuf>,
    /// The daemon's port (`main`: [`DAEMON_PORT`] when not given).
    pub port: u16,
    /// This `spec`'s path (`std::env::current_exe()`), if known: the
    /// daemon's binary is looked for beside it first.
    pub spec_path: Option<PathBuf>,
    /// `PATH`, searched next.
    pub path: Option<OsString>,
}

/// The launcher's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Serve {
    /// A daemon already serves every requested root: its lines for stdout
    /// (`already running: pid <pid>`, a `serving <slug> <root>` per
    /// requested root, `listening <url>`), exit 0, nothing started.
    Running(String),
    /// No daemon: `main` execs `program` with `args` (`--root <root>`
    /// per canonical root, in order, then `--port <N>`), environment and
    /// current directory unchanged.
    Start {
        program: PathBuf,
        args: Vec<OsString>,
    },
}

/// `spec serve`: the roots, the guard, the lookup, the probe (see the
/// module documentation). Exit 2 for `--config`, for `--root` beside a
/// DIR, for a root that is no project, for a data directory inside a root
/// (the CLI's lines; a DIR's as `spec serve: DIR <given>: <line>`), for no
/// daemon binary, for a running daemon serving other roots or one whose
/// discovery file stays unreadable.
pub fn serve(env: &Env, globals: &Globals, request: &ServeRequest) -> Result<Serve, CliError> {
    if globals.config.is_some() {
        return Err(refused(
            "--config is not supported: each root's specengine.toml is read",
        ));
    }
    if globals.root.is_some() && !request.dirs.is_empty() {
        return Err(refused("give the roots as DIR... or as --root, not both"));
    }
    let projects = roots(env, globals, &request.dirs)?;
    let mut requested = Vec::with_capacity(projects.len());
    let mut data_dir = None;
    for (given, project) in &projects {
        requested.push(DaemonProject {
            slug: project
                .slug()
                .map_err(|error| named(given.as_deref(), error))?
                .to_owned(),
            root: project.root.display().to_string(),
        });
    }
    for (given, project) in &projects {
        data_dir =
            Some(checked_data_dir(env, project).map_err(|error| named(given.as_deref(), error))?);
    }
    let Some(data_dir) = data_dir else {
        return Err(refused("no project root to serve"));
    };
    let program = find_daemon(request.spec_path.as_deref(), request.path.as_deref())
        .ok_or_else(|| no_binary(request.spec_path.as_deref()))?;
    match probe(&data_dir)? {
        Some(daemon) => answer(&daemon, &requested),
        None => {
            let mut args = Vec::with_capacity(projects.len() * 2 + 2);
            for (_, project) in &projects {
                args.push(OsString::from("--root"));
                args.push(project.root.clone().into_os_string());
            }
            args.push(OsString::from("--port"));
            args.push(OsString::from(request.port.to_string()));
            Ok(Serve::Start { program, args })
        }
    }
}

/// Exit 2, `spec serve: <message>`.
fn refused(message: impl std::fmt::Display) -> CliError {
    CliError::cannot(format!("spec serve: {message}"))
}

/// A DIR's error: `spec serve: DIR <given>: <the CLI's line>`; without a
/// DIR (the walk, or `--root`), the CLI's line as it is.
fn named(given: Option<&Path>, error: CliError) -> CliError {
    match given {
        Some(dir) => refused(format!("DIR {}: {}", dir.display(), error.message)),
        None => error,
    }
}

/// No DIR: the project any command finds (none given). DIRs: each as
/// `--root` takes it, in order, a repeated canonical root once, each with
/// the DIR as given. Each config read and its slug required in that order,
/// with the CLI's lines ([`named`]); a root without a config is not told
/// to pass `--config`, which `spec serve` refuses.
fn roots(
    env: &Env,
    globals: &Globals,
    dirs: &[PathBuf],
) -> Result<Vec<(Option<PathBuf>, ProjectRoot)>, CliError> {
    if dirs.is_empty() {
        let project = discover_without_config_hint(env, globals)?;
        project.slug()?;
        return Ok(vec![(None, project)]);
    }
    let mut projects: Vec<(Option<PathBuf>, ProjectRoot)> = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let globals = Globals {
            root: Some(dir.clone()),
            config: None,
        };
        let project =
            discover_without_config_hint(env, &globals).map_err(|error| named(Some(dir), error))?;
        project.slug().map_err(|error| named(Some(dir), error))?;
        if !projects.iter().any(|(_, found)| found.root == project.root) {
            projects.push((Some(dir.clone()), project));
        }
    }
    Ok(projects)
}

/// `specengine-http` beside `spec_path`, else in the first `PATH` entry
/// holding an executable regular file of that name (an empty entry
/// skipped).
fn find_daemon(spec_path: Option<&Path>, path: Option<&OsStr>) -> Option<PathBuf> {
    find_beside_or_on_path(DAEMON_BIN, spec_path, path)
}

/// The executable `binary` (the platform's suffix added) beside
/// `spec_path`, else in the first `PATH` entry holding an executable
/// regular file of that name (an empty entry skipped): the launcher's
/// lookup, `spec resolve`'s too.
pub(crate) fn find_beside_or_on_path(
    binary: &str,
    spec_path: Option<&Path>,
    path: Option<&OsStr>,
) -> Option<PathBuf> {
    let name = format!("{binary}{}", std::env::consts::EXE_SUFFIX);
    let beside = spec_path.and_then(Path::parent).map(|dir| dir.join(&name));
    let on_path = path
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(&name));
    beside
        .into_iter()
        .chain(on_path)
        .find(|candidate| is_executable_file(candidate))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn no_binary(spec_path: Option<&Path>) -> CliError {
    let spec = spec_path.map_or_else(|| "spec".to_owned(), |path| path.display().to_string());
    refused(format!(
        "{DAEMON_BIN} not found beside {spec} or on PATH; install it: \
         cargo install --locked --path <clone>/crates/{DAEMON_BIN}"
    ))
}

/// The running daemon of the data directory `dir`, if any, creating
/// nothing: no directory or no `daemon.lock` -> none; else the lock file
/// opened read-only and a shared lock tried: acquired (released at once,
/// the file closed) -> none, a `daemon.json` there stale; held -> its
/// `daemon.json`, re-read every [`DAEMON_POLL`] within
/// [`DAEMON_PUBLISH_WAIT`] while missing or unreadable, the shared lock
/// tried again before each read (a holder that ended meanwhile: none).
fn probe(dir: &Path) -> Result<Option<DaemonFile>, CliError> {
    let lock = dir.join(DAEMON_LOCK);
    let file = match File::open(&lock) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(refused(format!("cannot open {}: {error}", lock.display())));
        }
    };
    let deadline = Instant::now() + DAEMON_PUBLISH_WAIT;
    loop {
        match file.try_lock_shared() {
            Ok(()) => return Ok(None),
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(error)) => {
                return Err(refused(format!("cannot lock {}: {error}", lock.display())));
            }
        }
        match read_daemon_file(dir) {
            Ok(daemon) => return Ok(Some(daemon)),
            Err(reason) if Instant::now() >= deadline => {
                return Err(refused(unpublished_daemon(dir, &reason)));
            }
            Err(_) => thread::sleep(DAEMON_POLL),
        }
    }
}

/// A running daemon serving every requested (slug, root) pair: its lines;
/// else exit 2 naming it and the command that serves them all (its roots
/// in order, then the new ones; `--port` only when not the default).
///
/// Every text from `daemon.json` (and the requested pairs alike) is
/// printed as [`daemon_text`] gives it; a root in the command is then
/// shell-quoted.
fn answer(daemon: &DaemonFile, requested: &[DaemonProject]) -> Result<Serve, CliError> {
    let url = daemon.shown_url();
    if requested
        .iter()
        .all(|wanted| daemon.projects.contains(wanted))
    {
        let mut lines = format!("already running: pid {}\n", daemon.pid);
        for project in requested {
            lines.push_str(&format!(
                "serving {} {}\n",
                daemon_text(&project.slug),
                daemon_text(&project.root)
            ));
        }
        lines.push_str(&format!("listening {url}\n"));
        return Ok(Serve::Running(lines));
    }
    let mut roots: Vec<&str> = Vec::new();
    for root in daemon
        .projects
        .iter()
        .chain(requested)
        .map(|project| project.root.as_str())
    {
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    let mut command = String::from("spec serve");
    for root in roots {
        command.push(' ');
        command.push_str(&shell_word(&daemon_text(root)));
    }
    if daemon.port != DAEMON_PORT {
        command.push_str(&format!(" --port {}", daemon.port));
    }
    Err(refused(format!(
        "a daemon (pid {}, {url}) serves {}; stop it, then run: {command}",
        daemon.pid,
        daemon.serving()
    )))
}

/// `word` as one shell word: as written when it holds only
/// `A-Za-z0-9/._-`, else single-quoted (a `'` as `'\''`).
fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'));
    if plain {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}
