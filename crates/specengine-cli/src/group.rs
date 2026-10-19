//! The process-group core of `spec verify --tests` (task spec
//! `verify-tests`, "Runner") and `spec resolve` (task spec
//! `layer-c-resolve`, "Group core"), Unix only: a prepared `Command`
//! spawned as the leader of a process group of its own and waited for,
//! through three POSIX calls std does not expose (`killpg`, `sigaction`,
//! `waitid`), the CLI's only `unsafe`, each block with its `SAFETY` note
//! (after eval's `ra/sys.rs` `kill_group`, which the CLI may not depend
//! on). The I/O is each caller's.
//!
//! - **Process**: [`spawn`] gives the command its own process group
//!   (`process_group(0)`: the leader's pid is the group's ID).
//! - **Every exit**: [`wait`] sees the leader's exit without reaping it
//!   (`waitid` with `WNOWAIT`), then the whole group gets `SIGKILL`, then
//!   the leader is reaped: the group ID signalled is never a reused one.
//! - **Triggers**: past the timeout (an overrun), a forwarded signal
//!   ([`Forwarding`]) or the caller's stop (a flag it sets, such as a
//!   failed check of what the group writes): `SIGTERM` to the group,
//!   [`GRACE`], then `SIGKILL`. The first trigger is the outcome: it wins
//!   over any exit seen afterwards.
//! - **Signals** ([`Forwarding`]): while one is held, `SIGINT`, `SIGTERM`
//!   and `SIGHUP` to `spec` only set a flag (`sigaction`, `SA_RESTART`);
//!   the running group then gets `SIGTERM` and the same escalation. The
//!   dispositions before are restored when the last holder drops it.
//!
//! A `setsid` or double-forked daemon leaves the group and is not reached;
//! a `spec` killed by `SIGKILL` leaves the group running (task spec
//! `verify-tests`, "Risks").

use std::io;
use std::mem::MaybeUninit;
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Between `SIGTERM` and `SIGKILL` to an overrun, interrupted or stopped
/// group.
pub(crate) const GRACE: Duration = Duration::from_secs(5);

/// How often a running group is looked at.
const POLL: Duration = Duration::from_millis(10);

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

/// How a group's leader ended.
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
    /// The caller's stop flag stopped it.
    Stopped,
    /// Its exit could not be read (std's wait failed).
    WaitFailed(io::Error),
}

/// The signal forwarding: installed while held.
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

/// `signal` to every process of `group` (a child spawned by [`spawn`] and
/// not reaped yet). Never group 0 (the caller's own) or 1.
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

/// Spawns `command` as the leader of a process group of its own; its
/// stdio, directory and variables are the caller's.
pub(crate) fn spawn(command: &mut Command) -> io::Result<Child> {
    command.process_group(0).spawn()
}

/// Waits for `child`'s group (spawned by [`spawn`]), escalating on the
/// first trigger: an overrun of `timeout` since the call, a forwarded
/// signal, or `stop` set by the caller; the group gets `SIGKILL` once the
/// leader's exit is seen, then the leader is reaped.
pub(crate) fn wait(
    child: &mut Child,
    timeout: Duration,
    forwarding: &Forwarding,
    stop: &AtomicBool,
) -> Ended {
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
                Err(error) => Ended::WaitFailed(error),
            };
        }
        let now = Instant::now();
        match &stopping {
            None if forwarding.interrupted() => {
                signal_group(group, libc::SIGTERM);
                stopping = Some((Ended::Interrupted, now + GRACE));
            }
            None if stop.load(Ordering::SeqCst) => {
                signal_group(group, libc::SIGTERM);
                stopping = Some((Ended::Stopped, now + GRACE));
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
