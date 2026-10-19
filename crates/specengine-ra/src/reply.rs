//! The reply of protocol v1 (`docs/features/layer-c.md` "Protocol v1"):
//! JSON lines on stdout, the key `t` first, each written whole and flushed.
//!
//! `hello`; per workspace its `step` lines; one `file` line per requested
//! path, in request order, items by `start`; `done` last. Or an `error`
//! (exit 1) after any of them, or a lone `refused` (exit 2) before any
//! discovery.

use std::io::{self, Write};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use specengine_ra::MonikerStatus;

/// The one protocol this binary speaks.
pub const PROTOCOL: u32 = 1;

#[derive(Serialize)]
pub struct Hello<'a> {
    t: &'static str,
    protocol: u32,
    ra_ap: &'a str,
    host: &'a str,
}

impl<'a> Hello<'a> {
    pub fn new(ra_ap: &'a str, host: &'a str) -> Hello<'a> {
        Hello {
            t: "hello",
            protocol: PROTOCOL,
            ra_ap,
            host,
        }
    }
}

/// The `discover` step: `cargo metadata` of the workspace and the sysroot.
#[derive(Serialize)]
pub struct DiscoverStep<'a> {
    t: &'static str,
    workspace: usize,
    dir: &'a str,
    step: &'static str,
    ms: u64,
    toolchain: Option<&'a str>,
    sysroot_packages: usize,
    metadata_degraded: bool,
}

impl<'a> DiscoverStep<'a> {
    pub fn new(
        workspace: usize,
        dir: &'a str,
        ms: u64,
        toolchain: Option<&'a str>,
        sysroot_packages: usize,
        metadata_degraded: bool,
    ) -> DiscoverStep<'a> {
        DiscoverStep {
            t: "step",
            workspace,
            dir,
            step: "discover",
            ms,
            toolchain,
            sysroot_packages,
            metadata_degraded,
        }
    }
}

/// The `database` step: files and crate graph into rust-analyzer's database.
#[derive(Serialize)]
pub struct DatabaseStep<'a> {
    t: &'static str,
    workspace: usize,
    dir: &'a str,
    step: &'static str,
    ms: u64,
}

impl<'a> DatabaseStep<'a> {
    pub fn new(workspace: usize, dir: &'a str, ms: u64) -> DatabaseStep<'a> {
        DatabaseStep {
            t: "step",
            workspace,
            dir,
            step: "database",
            ms,
        }
    }
}

#[derive(Serialize)]
pub struct FileLine<'a> {
    t: &'static str,
    path: &'a str,
    in_crate: bool,
    items: Vec<ItemLine<'a>>,
}

impl<'a> FileLine<'a> {
    pub fn new(path: &'a str, in_crate: bool, items: Vec<ItemLine<'a>>) -> FileLine<'a> {
        FileLine {
            t: "file",
            path,
            in_crate,
            items,
        }
    }
}

/// One item of a `file` line; `moniker` is `null` unless `state` is `moniker`.
#[derive(Serialize)]
pub struct ItemLine<'a> {
    pub start: usize,
    pub end: usize,
    pub kind: &'static str,
    pub state: &'static str,
    pub moniker: Option<&'a str>,
}

/// Items per `MonikerStatus::as_str`, every state present.
#[derive(Serialize, Default)]
pub struct ByState {
    pub moniker: usize,
    pub local: usize,
    pub none: usize,
    pub unresolved: usize,
    pub not_loaded: usize,
}

impl ByState {
    pub fn count(&mut self, status: &MonikerStatus) {
        match status {
            MonikerStatus::Moniker(_) => self.moniker += 1,
            MonikerStatus::Local => self.local += 1,
            MonikerStatus::NoMoniker => self.none += 1,
            MonikerStatus::Unresolved => self.unresolved += 1,
            MonikerStatus::NotLoaded => self.not_loaded += 1,
        }
    }
}

#[derive(Serialize)]
pub struct Done {
    t: &'static str,
    files: usize,
    items: usize,
    by_state: ByState,
    peak_rss: Option<u64>,
    ms: u64,
}

impl Done {
    pub fn new(
        files: usize,
        items: usize,
        by_state: ByState,
        peak_rss: Option<u64>,
        ms: u64,
    ) -> Done {
        Done {
            t: "done",
            files,
            items,
            by_state,
            peak_rss,
            ms,
        }
    }
}

/// A failed load step (`LoadError::category`), or a file the binary could
/// not read (`path`). `workspace` 0: a file under no manifest.
#[derive(Serialize)]
pub struct ErrorLine {
    t: &'static str,
    workspace: usize,
    category: &'static str,
}

impl ErrorLine {
    pub fn new(workspace: usize, category: &'static str) -> ErrorLine {
        ErrorLine {
            t: "error",
            workspace,
            category,
        }
    }
}

#[derive(Serialize)]
pub struct Refused<'a> {
    t: &'static str,
    protocol: u32,
    reason: &'a str,
}

impl<'a> Refused<'a> {
    pub fn new(reason: &'a str) -> Refused<'a> {
        Refused {
            t: "refused",
            protocol: PROTOCOL,
            reason,
        }
    }
}

/// One line, written in one call and flushed, so a reader never sees half
/// of it. An error means stdout is gone: the caller with it.
pub fn emit(line: &impl Serialize) -> io::Result<()> {
    let mut text = serde_json::to_string(line).map_err(io::Error::other)?;
    text.push('\n');
    let mut stdout = io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()
}

/// How the reply of an accepted request ended: its last line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// `done` written: exit 0.
    Done,
    /// `error` written: exit 1.
    Error,
}

impl Ending {
    /// The process's exit code once this line is written.
    pub const fn exit_code(self) -> u8 {
        match self {
            Ending::Done => 0,
            Ending::Error => 1,
        }
    }
}

/// Where the reply's last line stands, for a watchdog that fires at the end
/// of a run ([`last_line`]): a caller that reads `done` and then waits for
/// the process closes its stdin.
static LAST_LINE: AtomicU8 = AtomicU8::new(LAST_NOT_BEGUN);
/// Neither `done` nor `error` begun, or its write failed.
const LAST_NOT_BEGUN: u8 = 0;
/// `done` or `error` being written: a reader may already hold it whole.
const LAST_WRITING: u8 = 1;
const DONE_WRITTEN: u8 = 2;
const ERROR_WRITTEN: u8 = 3;

/// How often [`last_line`] looks again while the last line is being written.
const LAST_LINE_POLL: Duration = Duration::from_millis(1);

/// `done`, the reply's last line.
pub fn emit_done(line: &Done) -> io::Result<Ending> {
    emit_last(line, Ending::Done)
}

/// `error`, the reply's last line.
pub fn emit_error(line: &ErrorLine) -> io::Result<Ending> {
    emit_last(line, Ending::Error)
}

/// The last line. The order is what keeps a watchdog from separating the
/// line from its mark:
/// 1. `LAST_WRITING` is stored before the first byte is written;
/// 2. the line is written and flushed (`emit`, under the stdout lock);
/// 3. `DONE_WRITTEN` or `ERROR_WRITTEN` is stored once the flush returns,
///    `LAST_NOT_BEGUN` again when the write failed.
///
/// A reader holding the whole line therefore finds the state at step 1 or
/// 3, never before it; [`last_line`] waits out step 2 for a bounded time.
/// The stdout lock itself is not shared with the watchdogs: std offers no
/// `try_lock` on it, and a write blocked on a full pipe must not hold up the
/// budget watchdog's kill.
fn emit_last(line: &impl Serialize, ending: Ending) -> io::Result<Ending> {
    LAST_LINE.store(LAST_WRITING, Ordering::SeqCst);
    let written = emit(line);
    let state = match (&written, ending) {
        (Err(_), _) => LAST_NOT_BEGUN,
        (Ok(()), Ending::Done) => DONE_WRITTEN,
        (Ok(()), Ending::Error) => ERROR_WRITTEN,
    };
    LAST_LINE.store(state, Ordering::SeqCst);
    written.map(|()| ending)
}

/// The reply's last line once it is written and flushed; while it is being
/// written, waited for up to `patience`. `None`: not begun, failed, or still
/// being written after `patience` (stdout blocked): the run is unfinished,
/// and the watchdog ends it as before.
pub fn last_line(patience: Duration) -> Option<Ending> {
    let waiting = Instant::now();
    loop {
        match LAST_LINE.load(Ordering::SeqCst) {
            DONE_WRITTEN => return Some(Ending::Done),
            ERROR_WRITTEN => return Some(Ending::Error),
            LAST_WRITING if waiting.elapsed() < patience => thread::sleep(LAST_LINE_POLL),
            _ => return None,
        }
    }
}
