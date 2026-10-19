//! One run of `specengine-ra` for `spec resolve` (task spec
//! `layer-c-resolve`, "spec resolve" steps 4 and 5, "Stdin, stdout,
//! stderr", "Output and exits"), Unix only, on the CLI's process-group
//! core ([`crate::group`]):
//!
//! - **Spawn**: the program made absolute against `spec`'s current
//!   directory (one found through a relative `PATH` entry), exactly
//!   `--protocol 1`, the given directory, the leader of its own group;
//!   `spec`'s environment unchanged (the binary scrubs it).
//! - **Stdin**: the request written on its own thread while the budget
//!   runs (a binary that never reads it is an overrun); a broken pipe is
//!   not reported; held open until the reply's last line is read, a fault
//!   found or the group gone, then closed.
//! - **Stdout**: read as lines on a thread, each judged as it arrives
//!   ([`Reply::line`]); a line over [`REPLY_LINE_CAP`] is a fault at once;
//!   a last line cut off by the end is not judged. A fault sets the group
//!   core's stop: `SIGTERM`, grace, `SIGKILL`, stdin closed.
//! - **Stderr**: the binary's own, `spec`'s stderr (copied live).
//! - **Ending**: the group core's first trigger wins (an overrun, a
//!   forwarded signal, a fault); else the last line and the exit together
//!   ([`Ended`]'s table). Nothing is written here: the caller stores only
//!   a complete reply.

use std::io::{self, Read as _, Write as _};
use std::path::Path;
use std::process::{ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use crate::group::{self, Forwarding};
use crate::layer_c::LAYER_C_BIN;
use crate::layer_c_reply::{Fault, Last, PROTOCOL, REPLY_LINE_CAP, Reply};

/// How long stdout is read after the group is gone (a process that left
/// the group may hold it).
const DRAIN: Duration = Duration::from_secs(5);

/// Stdout's read size.
const CHUNK: usize = 64 * 1024;

/// The binary's one argument naming the protocol.
const PROTOCOL_ARGUMENT: &str = "--protocol";

/// What one run is given.
pub(crate) struct Run<'a> {
    /// The binary found.
    pub(crate) program: &'a Path,
    /// Its working directory.
    pub(crate) dir: &'a Path,
    /// The request line, its line end included.
    pub(crate) request: String,
    /// The request's `files`, in order.
    pub(crate) requested: Vec<String>,
    /// The budget, S.
    pub(crate) seconds: u64,
}

/// How a run ended without a complete reply: the reason `spec resolve`
/// gives.
pub(crate) type Failure = String;

/// The reply read and what the reader found.
struct Read {
    reply: Reply,
    fault: Option<Fault>,
}

/// Runs the binary to its end: the complete reply after `done` and exit 0,
/// else the reason (the table of "Output and exits").
pub(crate) fn run(run: Run<'_>) -> Result<Reply, Failure> {
    // A program found through a relative `PATH` entry is relative to this
    // process's directory, not to `run.dir`, where the child execs.
    let program = std::path::absolute(run.program).map_err(|error| cannot_start(&error))?;
    let forwarding = Forwarding::hold();
    let mut command = Command::new(&program);
    command
        .arg(PROTOCOL_ARGUMENT)
        .arg(PROTOCOL.to_string())
        .current_dir(run.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let spawned = group::spawn(&mut command);
    drop(command);
    let mut child = spawned.map_err(|error| cannot_start(&error))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();

    // Stdin: written, then held until a close is sent (or every sender is
    // gone).
    let (close, held) = mpsc::channel::<()>();
    let request = run.request;
    thread::spawn(move || {
        if let Some(mut stdin) = stdin {
            // A broken pipe is not reported: the reply and the exit decide.
            let _ = stdin.write_all(request.as_bytes());
            let _ = stdin.flush();
            let _ = held.recv();
        }
    });

    let stop = Arc::new(AtomicBool::new(false));
    let read = Arc::new(Mutex::new(Read {
        reply: Reply::new(run.requested),
        fault: None,
    }));
    let (finished, reader_done) = mpsc::channel::<()>();
    if let Some(stdout) = stdout {
        let read = Arc::clone(&read);
        let stop = Arc::clone(&stop);
        let close = close.clone();
        thread::spawn(move || {
            read_reply(stdout, &read, &stop, &close);
            let _ = close.send(());
            let _ = finished.send(());
        });
    } else {
        drop(finished);
    }

    let budget = Duration::from_secs(run.seconds);
    let ended = group::wait(&mut child, budget, &forwarding, &stop);
    // The group is gone: stdin is closed whatever the reader saw.
    let _ = close.send(());
    let _ = reader_done.recv_timeout(DRAIN);
    drop(forwarding);
    let mut read = read.lock().unwrap_or_else(PoisonError::into_inner);
    let fault = read.fault.take();
    let reply = std::mem::replace(&mut read.reply, Reply::new(Vec::new()));
    ending(ended, fault, reply, run.seconds)
}

/// The reason, or the complete reply, from the group's ending, the
/// reader's fault and the reply's last line.
fn ending(
    ended: group::Ended,
    fault: Option<Fault>,
    reply: Reply,
    seconds: u64,
) -> Result<Reply, Failure> {
    let exit = match ended {
        group::Ended::TimedOut => return Err(format!("over the budget of {seconds} s")),
        group::Ended::Interrupted => return Err("interrupted".to_owned()),
        group::Ended::WaitFailed(error) => {
            return Err(format!("{LAYER_C_BIN} cannot be waited for: {error}"));
        }
        group::Ended::Stopped => {
            return Err(fault.map_or_else(|| "malformed reply".to_owned(), |fault| reason(&fault)));
        }
        group::Ended::Exited(code) => Exit::Code(code),
        group::Ended::Signalled(signal) => Exit::Signal(signal),
    };
    if let Some(fault) = fault {
        return Err(reason(&fault));
    }
    match (&reply.last, exit) {
        (Some(Last::Done(_)), Exit::Code(0)) => Ok(reply),
        (
            Some(Last::Error {
                workspace,
                category,
            }),
            Exit::Code(1),
        ) => Err(if *workspace == 0 {
            format!("load failed: {category} (files under no manifest)")
        } else {
            format!("load failed: {category} (workspace {workspace})")
        }),
        (Some(Last::Refused { reason }), Exit::Code(2)) => {
            Err(format!("{LAYER_C_BIN} refused the request: {reason}"))
        }
        (_, exit) => {
            let when = if reply.done() { "after" } else { "before" };
            Err(match exit {
                Exit::Code(code) => format!("{LAYER_C_BIN} ended (exit {code}) {when} done"),
                Exit::Signal(signal) => {
                    format!("{LAYER_C_BIN} ended (signal {signal}) {when} done")
                }
            })
        }
    }
}

/// The binary's own ending.
#[derive(Debug, Clone, Copy)]
enum Exit {
    Code(i32),
    Signal(i32),
}

/// A fault's reason.
fn reason(fault: &Fault) -> Failure {
    match fault {
        Fault::Malformed(line) => format!("malformed reply at line {line}"),
        Fault::Mismatch(protocol) => format!(
            "{LAYER_C_BIN} speaks protocol {protocol}, spec needs {PROTOCOL}: install both from \
             one commit"
        ),
    }
}

/// The spawn failed.
fn cannot_start(error: &io::Error) -> Failure {
    format!("{LAYER_C_BIN} cannot be started: {error}")
}

/// Reads stdout as lines to its end or the first fault, judging each
/// complete line; the reply's last line, a fault or the end sends `close`.
fn read_reply(
    mut stdout: ChildStdout,
    read: &Mutex<Read>,
    stop: &AtomicBool,
    close: &mpsc::Sender<()>,
) {
    let mut line: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let size = match stdout.read(&mut buffer) {
            Ok(0) => return,
            Ok(size) => size,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        let mut piece = &buffer[..size];
        while !piece.is_empty() {
            let end = piece.iter().position(|&byte| byte == b'\n');
            let taken = end.unwrap_or(piece.len());
            if line.len() + taken > REPLY_LINE_CAP {
                let mut read = read.lock().unwrap_or_else(PoisonError::into_inner);
                let fault = read.reply.oversized();
                fail(&mut read, fault, stop, close);
                return;
            }
            line.extend_from_slice(&piece[..taken]);
            let Some(end) = end else {
                break;
            };
            piece = &piece[end + 1..];
            let mut read = read.lock().unwrap_or_else(PoisonError::into_inner);
            match read.reply.line(&line) {
                Ok(()) => {
                    if read.reply.complete() {
                        let _ = close.send(());
                    }
                }
                Err(fault) => {
                    fail(&mut read, fault, stop, close);
                    return;
                }
            }
            line.clear();
        }
    }
}

/// Records the fault, stops the group, closes stdin.
fn fail(read: &mut Read, fault: Fault, stop: &AtomicBool, close: &mpsc::Sender<()>) {
    read.fault = Some(fault);
    stop.store(true, Ordering::SeqCst);
    let _ = close.send(());
}
