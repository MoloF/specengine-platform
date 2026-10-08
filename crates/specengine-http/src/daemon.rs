//! The daemon's identity in its data directory (docs/features/spec-serve.md
//! "Data"): one daemon per data directory. `daemon.lock` is opened
//! (created if absent, never written or deleted) and locked exclusively
//! before the bind, held until the process ends (any end, `SIGKILL`
//! included, frees it); `daemon.json` is written after the bind, whole
//! (`daemon.json.tmp`, then a rename), never removed. No token, key or
//! secret is written.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use specengine_cli::{
    DAEMON_FILE, DAEMON_LOCK, DAEMON_POLL, DaemonFile, DaemonProject, read_daemon_file,
    unpublished_daemon, utc_now,
};

use crate::start::Project;

/// How long a start retries a held lock: a `spec serve` probe holds its
/// shared lock for microseconds.
const LOCK_WAIT: Duration = Duration::from_millis(500);

/// The discovery file before its rename.
const DAEMON_FILE_PARTIAL: &str = "daemon.json.tmp";

/// The held lock of the data directory; dropped (or the process ended),
/// it is free.
#[derive(Debug)]
pub(crate) struct Lock {
    _file: File,
    dir: PathBuf,
}

/// `daemon.lock` of the data directory `dir` locked exclusively, retried
/// every 50 ms up to 500 ms; still held: the reason naming the daemon
/// that holds it (its `daemon.json`, every text from it escaped and on one
/// line as the CLI's `daemon_text` prints it), or that its file is
/// unreadable.
pub(crate) fn lock(dir: &Path) -> Result<Lock, String> {
    let path = dir.join(DAEMON_LOCK);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => {
                return Ok(Lock {
                    _file: file,
                    dir: dir.to_path_buf(),
                });
            }
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(DAEMON_POLL);
            }
            Err(TryLockError::WouldBlock) => break,
            Err(TryLockError::Error(error)) => {
                return Err(format!("cannot lock {}: {error}", path.display()));
            }
        }
    }
    Err(match read_daemon_file(dir) {
        Ok(daemon) => format!(
            "a daemon already runs under {}: pid {}, {}, serving {}; stop it first",
            dir.display(),
            daemon.pid,
            daemon.shown_url(),
            daemon.serving()
        ),
        Err(reason) => unpublished_daemon(dir, &reason),
    })
}

/// `daemon.json` written whole beside the held lock: this process's pid,
/// the bound `port` and its `url`, the projects in `--root` order, the
/// start time.
pub(crate) fn publish(
    lock: &Lock,
    projects: &[Project],
    port: u16,
    url: &str,
) -> Result<(), String> {
    let daemon = DaemonFile {
        pid: std::process::id(),
        port,
        url: url.to_owned(),
        projects: projects
            .iter()
            .map(|project| DaemonProject {
                slug: project.slug.clone(),
                root: project.root.display().to_string(),
            })
            .collect(),
        started: utc_now(),
    };
    let text = daemon.to_text()?;
    let partial = lock.dir.join(DAEMON_FILE_PARTIAL);
    let path = lock.dir.join(DAEMON_FILE);
    fs::write(&partial, text)
        .and_then(|()| fs::rename(&partial, &path))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}
