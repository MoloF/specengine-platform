//! The runner of `spec verify --tests` (task spec `verify-tests`,
//! "Runner"), Unix only: one invocation of the project's own command at a
//! time, through std's `Command` on the CLI's process-group core
//! ([`crate::group`]: its own group, the waiting, the escalation, the
//! signal forwarding); the I/O is this module's.
//!
//! - **Process**: the given directory, stdin null, exactly the given
//!   variables; stdout and stderr one pipe, read on a thread to the end:
//!   each piece copied at once to `spec`'s stderr, fed to one streaming
//!   BLAKE3 ([`B3Stream`]) and to the last [`TAIL_BYTES`] kept; nothing
//!   reaches `spec`'s stdout, no file is written.
//! - **Every exit**: the group core's (the leader seen without reaping it,
//!   the whole group `SIGKILL`ed, the leader reaped). The pipe is read to
//!   its end, or [`DRAIN`] more (a process that left the group may hold
//!   it).
//! - **Overrun**: past the timeout, `SIGTERM` to the group,
//!   [`crate::group::GRACE`], then `SIGKILL`.
//! - **Signals** ([`Forwarding`]): the running invocation's group gets
//!   `SIGTERM` and the same escalation.

use std::ffi::{OsStr, OsString};
use std::io::{self, PipeReader, Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use specengine_store::B3Stream;

use crate::group;
pub(crate) use crate::group::Forwarding;

/// The bytes of a run's output its tail keeps.
pub(crate) const TAIL_BYTES: usize = 4096;

/// How long the pipe is read after the group is gone.
const DRAIN: Duration = Duration::from_secs(5);

/// The pipe's read size.
const CHUNK: usize = 64 * 1024;

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
        .stderr(writer_too);
    let spawned = group::spawn(&mut command);
    // The command holds this process's copies of the pipe's write end: its
    // end of the output is the end of every writer the group has.
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return cannot(error),
    };
    let copy = Copy::start(reader);
    // No stop of its own: only the overrun and a forwarded signal.
    let never = AtomicBool::new(false);
    let ended = ended_of(group::wait(
        &mut child,
        invocation.timeout,
        forwarding,
        &never,
    ));
    let (output_hash, tail) = copy.finish();
    Ran {
        ended,
        output_hash: Some(output_hash),
        tail,
    }
}

/// The group core's ending as this runner's.
fn ended_of(ended: group::Ended) -> Ended {
    match ended {
        group::Ended::Exited(code) => Ended::Exited(code),
        group::Ended::Signalled(signal) => Ended::Signalled(signal),
        group::Ended::TimedOut => Ended::TimedOut,
        // `Stopped` never comes: this runner sets no stop flag.
        group::Ended::Interrupted | group::Ended::Stopped => Ended::Interrupted,
        group::Ended::WaitFailed(error) => Ended::CannotStart(error),
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
