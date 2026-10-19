//! The runner of `spec verify --tests` (task spec `verify-tests`,
//! "Runner"), Unix only: one invocation of the project's own command at a
//! time, through std's `Command` and three POSIX calls std does not expose
//! (`killpg`, `sigaction`, `waitid`), the CLI's only `unsafe`, each block
//! with its `SAFETY` note (after eval's `ra/sys.rs` `kill_group`, which the
//! CLI may not depend on).
//!
//! - **Process**: its own process group (`process_group(0)`: the leader's
//!   pid is the group's ID), the given directory, stdin null, exactly the
//!   given variables; stdout and stderr one pipe, read on a thread to the
//!   end: each piece copied at once to `spec`'s stderr, fed to one
//!   streaming BLAKE3 ([`B3Stream`]) and to the last [`TAIL_BYTES`] kept;
//!   nothing reaches `spec`'s stdout, no file is written.
//! - **Every exit**: the leader's exit is seen without reaping it (`waitid`
//!   with `WNOWAIT`), then the whole group gets `SIGKILL`, then the leader
//!   is reaped: the group ID signalled is never a reused one. The pipe is
//!   read to its end, or [`DRAIN`] more (a process that left the group may
//!   hold it).
//! - **Overrun**: past the timeout, `SIGTERM` to the group, [`GRACE`], then
//!   `SIGKILL`.
//! - **Signals** ([`Forwarding`]): while one is held, `SIGINT`, `SIGTERM`
//!   and `SIGHUP` to `spec` only set a flag (`sigaction`, `SA_RESTART`);
//!   the running invocation's group then gets `SIGTERM` and the same
//!   escalation. The dispositions before are restored when the last
//!   holder drops it.
//!
//! A `setsid` or double-forked daemon leaves the group and is not reached;
//! a `spec` killed by `SIGKILL` leaves the group running (task spec
//! `verify-tests`, "Risks").

use std::ffi::{OsStr, OsString};
use std::io::{self, PipeReader, Read as _, Write as _};
use std::mem::MaybeUninit;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use specengine_store::B3Stream;

/// The bytes of a run's output its tail keeps.
pub(crate) const TAIL_BYTES: usize = 4096;

/// Between `SIGTERM` and `SIGKILL` to an overrun or interrupted group.
const GRACE: Duration = Duration::from_secs(5);

/// How long the pipe is read after the group is gone.
const DRAIN: Duration = Duration::from_secs(5);

/// How often a running invocation is looked at.
const POLL: Duration = Duration::from_millis(10);

/// The pipe's read size.
const CHUNK: usize = 64 * 1024;

/// The signals forwarded to the running group.
const FORWARDED: [libc::c_int; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

/// Set by the handler; read by the waiting loop.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// The holders of [`Forwarding`] and the dispositions they replaced.
static INSTALLED: Mutex<Installed> = Mutex::new(Installed {
    holders: 0,
    before: Vec::new(),
});

struct Installed {
    holders: usize,
    before: Vec<(libc::c_int, libc::sigaction)>,
}

/// One invocation of the project's command.
pub(crate) struct Invocation<'a> {
    /// Element 0, as `Command` takes it (a relative path with a `/`
    /// already joined to the root).
    pub(crate) program: &'a OsStr,
    /// The elements after it.
    pub(crate) args: &'a [String],
    pub(crate) dir: &'a Path,
    /// Every variable it gets, and no other.
    pub(crate) vars: &'a [(OsString, OsString)],
    pub(crate) timeout: Duration,
}

/// How an invocation ended.
#[derive(Debug)]
pub(crate) enum Ended {
    /// Its own exit, with the code.
    Exited(i32),
    /// Killed by a signal not sent here.
    Signalled(i32),
    /// Still running at the timeout.
    TimedOut,
    /// A forwarded signal stopped it.
    Interrupted,
    /// The program could not be started.
    CannotStart(io::Error),
}

/// An invocation's end and output.
#[derive(Debug)]
pub(crate) struct Ran {
    pub(crate) ended: Ended,
    /// `b3:` of the whole output; `None` when nothing started.
    pub(crate) output_hash: Option<String>,
    /// The output's last [`TAIL_BYTES`].
    pub(crate) tail: Vec<u8>,
}

/// The signal forwarding of steps 8 and 9: installed while held.
pub(crate) struct Forwarding {
    _held: (),
}

impl Forwarding {
    /// Installs the handlers (the first holder clears the flag).
    pub(crate) fn hold() -> Self {
        let mut installed = INSTALLED.lock().unwrap_or_else(PoisonError::into_inner);
        if installed.holders == 0 {
            INTERRUPTED.store(false, Ordering::SeqCst);
            for signal in FORWARDED {
                if let Some(before) = install(signal) {
                    installed.before.push((signal, before));
                }
            }
        }
        installed.holders += 1;
        Self { _held: () }
    }

    /// A forwarded signal arrived since the first holder installed them.
    pub(crate) fn interrupted(&self) -> bool {
        INTERRUPTED.load(Ordering::SeqCst)
    }
}

impl Drop for Forwarding {
    fn drop(&mut self) {
        let mut installed = INSTALLED.lock().unwrap_or_else(PoisonError::into_inner);
        installed.holders = installed.holders.saturating_sub(1);
        if installed.holders == 0 {
            for (signal, before) in installed.before.drain(..) {
                restore(signal, &before);
            }
        }
    }
}

/// The handler: only an atomic store, which is async-signal-safe.
extern "C" fn on_signal(_signal: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// [`on_signal`] made `signal`'s handler; the disposition it replaced.
fn install(signal: libc::c_int) -> Option<libc::sigaction> {
    // SAFETY: an all-zero `sigaction` is a valid value of `libc`'s layout
    // (integers and an empty mask: `SIG_DFL`, no flags).
    let mut action: libc::sigaction = unsafe { MaybeUninit::zeroed().assume_init() };
    // SAFETY: the same, for the disposition `sigaction` writes back.
    let mut before: libc::sigaction = unsafe { MaybeUninit::zeroed().assume_init() };
    // SAFETY: `sigemptyset` writes only the mask it is given, a valid
    // `sigset_t` of `action`.
    unsafe {
        libc::sigemptyset(&mut action.sa_mask);
    }
    action.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    action.sa_flags = libc::SA_RESTART;
    // SAFETY: both pointers point to valid `sigaction` values of this
    // target's layout; the handler is an `extern "C" fn(c_int)` that only
    // stores to an atomic; `sigaction` reads the first, writes the second.
    let status = unsafe { libc::sigaction(signal, &action, &mut before) };
    (status == 0).then_some(before)
}

/// `before` made `signal`'s disposition again.
fn restore(signal: libc::c_int, before: &libc::sigaction) {
    // SAFETY: `before` is a disposition `sigaction` itself gave; the old
    // one is not asked for (a null pointer).
    unsafe {
        libc::sigaction(signal, before, std::ptr::null_mut());
    }
}

/// `signal` to every process of `group` (a child spawned with
/// `process_group(0)` and not reaped yet). Never group 0 (the caller's
/// own) or 1.
fn signal_group(group: libc::pid_t, signal: libc::c_int) {
    if group <= 1 {
        return;
    }
    // SAFETY: a plain system call without pointers; the group's leader is
    // this process's child, not reaped yet, so the ID is still its group's.
    unsafe {
        libc::killpg(group, signal);
    }
}

/// The child `pid` has exited, seen without reaping it (`waitid` with
/// `WEXITED | WNOHANG | WNOWAIT`).
fn leader_exited(pid: libc::pid_t) -> io::Result<bool> {
    let id = libc::id_t::try_from(pid).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    loop {
        let mut info = MaybeUninit::<libc::siginfo_t>::zeroed();
        // SAFETY: `info` is writable storage for one `siginfo_t` of this
        // target's layout; `waitid` writes at most one into it.
        let status = unsafe {
            libc::waitid(
                libc::P_PID,
                id,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if status != 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        // SAFETY: the storage was all zero (a valid `siginfo_t`: integers
        // and a null pointer) and `waitid` writes only a whole one.
        let info = unsafe { info.assume_init() };
        // No child ready: `waitid` leaves the zeroed `si_pid` as it is.
        return Ok(exited_pid(&info) != 0);
    }
}

/// The pid `waitid` filled in.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn exited_pid(info: &libc::siginfo_t) -> libc::pid_t {
    // SAFETY: `waitid` with `WEXITED` fills the child fields of the union,
    // which `si_pid` reads (zero when it filled nothing).
    unsafe { info.si_pid() }
}

/// The pid `waitid` filled in.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn exited_pid(info: &libc::siginfo_t) -> libc::pid_t {
    info.si_pid
}

/// Runs one invocation to its end (see the module documentation).
pub(crate) fn run(invocation: &Invocation<'_>, forwarding: &Forwarding) -> Ran {
    let cannot = |error: io::Error| Ran {
        ended: Ended::CannotStart(error),
        output_hash: None,
        tail: Vec::new(),
    };
    let (reader, writer) = match io::pipe() {
        Ok(pipe) => pipe,
        Err(error) => return cannot(error),
    };
    let writer_too = match writer.try_clone() {
        Ok(writer) => writer,
        Err(error) => return cannot(error),
    };
    let mut command = Command::new(invocation.program);
    command
        .args(invocation.args)
        .current_dir(invocation.dir)
        .env_clear()
        .envs(invocation.vars.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(writer_too)
        .process_group(0);
    let spawned = command.spawn();
    // The command holds this process's copies of the pipe's write end: its
    // end of the output is the end of every writer the group has.
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return cannot(error),
    };
    let copy = Copy::start(reader);
    let ended = wait(&mut child, invocation.timeout, forwarding);
    let (output_hash, tail) = copy.finish();
    Ran {
        ended,
        output_hash: Some(output_hash),
        tail,
    }
}

/// Waits for the leader, escalating on an overrun or a forwarded signal;
/// the group gets `SIGKILL` once the leader's exit is seen, then the leader
/// is reaped.
fn wait(child: &mut Child, timeout: Duration, forwarding: &Forwarding) -> Ended {
    // A pid outside `pid_t` cannot be: group 0 is never signalled.
    let group = libc::pid_t::try_from(child.id()).unwrap_or(0);
    let started = Instant::now();
    // Why the group is being stopped, and when `SIGKILL` is due.
    let mut stopping: Option<(Ended, Instant)> = None;
    let mut killed = false;
    loop {
        let reaped = match leader_exited(group) {
            Ok(true) => {
                signal_group(group, libc::SIGKILL);
                Some(child.wait())
            }
            Ok(false) => None,
            // `waitid` cannot look: std's wait, which reaps (the group is
            // then no longer signalled). If that fails too the leader may be
            // reaped already: its pid is never signalled.
            Err(_) => match child.try_wait() {
                Ok(None) => None,
                Ok(Some(status)) => Some(Ok(status)),
                Err(error) => Some(Err(error)),
            },
        };
        if let Some(status) = reaped {
            if let Some((why, _)) = stopping {
                return why;
            }
            return match status {
                Ok(status) => match (status.code(), status.signal()) {
                    (Some(code), _) => Ended::Exited(code),
                    (None, Some(signal)) => Ended::Signalled(signal),
                    (None, None) => Ended::Exited(-1),
                },
                Err(error) => Ended::CannotStart(error),
            };
        }
        let now = Instant::now();
        match &stopping {
            None if forwarding.interrupted() => {
                signal_group(group, libc::SIGTERM);
                stopping = Some((Ended::Interrupted, now + GRACE));
            }
            None if now.duration_since(started) >= timeout => {
                signal_group(group, libc::SIGTERM);
                stopping = Some((Ended::TimedOut, now + GRACE));
            }
            Some((_, due)) if !killed && now >= *due => {
                signal_group(group, libc::SIGKILL);
                killed = true;
            }
            _ => {}
        }
        thread::sleep(POLL);
    }
}

/// What the copy has read so far.
struct Copied {
    hash: B3Stream,
    tail: Vec<u8>,
}

/// The pipe read on its own thread.
struct Copy {
    copied: Arc<Mutex<Copied>>,
    done: mpsc::Receiver<()>,
}

impl Copy {
    /// Starts reading `reader` to its end.
    fn start(mut reader: PipeReader) -> Self {
        let copied = Arc::new(Mutex::new(Copied {
            hash: B3Stream::new(),
            tail: Vec::new(),
        }));
        let (sender, done) = mpsc::channel();
        let shared = Arc::clone(&copied);
        thread::spawn(move || {
            let mut buffer = vec![0u8; CHUNK];
            loop {
                let read = match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                let piece = &buffer[..read];
                // A closed stderr is no failure of the run.
                let _ = io::stderr().lock().write_all(piece);
                let mut copied = shared.lock().unwrap_or_else(PoisonError::into_inner);
                copied.hash.update(piece);
                copied.tail.extend_from_slice(piece);
                if copied.tail.len() > 2 * TAIL_BYTES {
                    let cut = copied.tail.len() - TAIL_BYTES;
                    copied.tail.drain(..cut);
                }
            }
            let _ = sender.send(());
        });
        Self { copied, done }
    }

    /// The hash and the tail once the pipe ended, or after [`DRAIN`] (what
    /// was read by then).
    fn finish(self) -> (String, Vec<u8>) {
        let _ = self.done.recv_timeout(DRAIN);
        let copied = self.copied.lock().unwrap_or_else(PoisonError::into_inner);
        let start = copied.tail.len().saturating_sub(TAIL_BYTES);
        (copied.hash.finish(), copied.tail[start..].to_vec())
    }
}
