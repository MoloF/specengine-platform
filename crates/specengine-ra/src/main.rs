//! `specengine-ra --protocol 1`: layer C as its own executable
//! (`docs/features/layer-c.md`, ADR-0049). A pure function of the request
//! and the disk: it reads one request line on stdin, answers in JSON lines
//! on stdout ([`reply`]), runs no build script, proc macro or `cargo check`,
//! and writes only into the request's `target_dir` and `tmp_dir`.
//!
//! Exits: 0 after `done`, 1 after `error`, 2 after `refused` (bad argv or
//! request, before anything is discovered). Outside the protocol: 3 when a
//! watchdog or a closed stdout ends a run that does not lead its process
//! group, or on an internal failure; no line is written then, the reason
//! goes to stderr.
//!
//! Its caller (`spec resolve`) spawns it as a process group of its own and
//! keeps stdin open. Two watchdogs then end the whole group, cargo and rustc
//! included: stdin reaching EOF (the caller is gone) and `budget_ms` plus
//! [`GRACE`] (the caller failed to act). Whether it leads its group is
//! checked, not assumed: started by an interactive shell with job control,
//! alone (`specengine-ra --protocol 1 < request.json`) or as the first
//! command of a pipeline, it leads its job's group, and a watchdog kills
//! that job; in a pipeline, unless it is the first command, or under a shell
//! without job control the group is another process's, and a watchdog ends
//! this process alone. Once `done` or `error` is written the reply is whole:
//! a watchdog firing then (a caller's wait closes stdin) exits with that
//! line's code, 0 or 1, and kills nothing.

mod reply;
mod request;
mod run;
mod sys;

use std::io::{self, Read, Write};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use crate::reply::{Hello, Refused};
use crate::request::Request;

/// The toolchain overrides a caller's environment may carry (`cargo run`,
/// `cargo +toolchain`, a wrapper, a target directory): removed so the tree
/// is analysed with its own toolchain and nothing is built through a
/// wrapper or into a directory the request did not name. The same seven
/// `specengine-eval ra` removes when it spawns its worker.
const TOOLCHAIN_OVERRIDES: [&str; 7] = [
    "RUSTUP_TOOLCHAIN",
    "CARGO",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
];

/// Set to the empty string after the removal: cargo then runs no wrapper,
/// over `build.rustc-wrapper` and `build.rustc-workspace-wrapper` of a
/// project's cargo configuration too.
const EMPTIED_WRAPPERS: [&str; 2] = ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"];

/// Past `budget_ms` the caller kills the group; this much later the binary
/// kills it itself.
const GRACE: Duration = Duration::from_secs(10);

/// A watchdog that fires while `done` or `error` is being written waits this
/// long for the write to finish before it treats the run as unfinished.
const LAST_LINE_PATIENCE: Duration = Duration::from_secs(1);

/// rust-analyzer's own threads run on 16 MiB stacks; the analysis thread
/// gets more headroom (reserved, not resident), as `specengine-eval ra`'s.
const STACK_BYTES: usize = 64 * 1024 * 1024;

/// Exit code of a run ended outside the protocol (no line written).
const EXIT_OUTSIDE_PROTOCOL: u8 = 3;

fn main() -> ExitCode {
    let started = Instant::now();
    scrub_toolchain_overrides();
    if let Err(reason) = request::check_argv(std::env::args_os().skip(1)) {
        return refuse(&reason);
    }
    let request = match request::read_line(io::stdin().lock())
        .and_then(|line| request::parse(&line))
        .and_then(|request| request::create_dirs(&request).map(|()| request))
    {
        Ok(request) => request,
        Err(reason) => return refuse(&reason),
    };
    set_tmpdir(&request);
    start_watchdogs(started, request.budget);

    let host = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    if reply::emit(&Hello::new(specengine_ra::RA_AP_VERSION, &host)).is_err() {
        terminate("stdout closed");
    }
    let work = thread::Builder::new()
        .name("specengine-ra".to_owned())
        .stack_size(STACK_BYTES)
        .spawn(move || run::run(&request, started));
    match work.map(thread::JoinHandle::join) {
        Ok(Ok(Ok(ending))) => ExitCode::from(ending.exit_code()),
        Ok(Ok(Err(_))) => terminate("stdout closed"),
        Ok(Err(_)) => {
            eprintln!("specengine-ra: internal failure: the analysis thread panicked");
            ExitCode::from(EXIT_OUTSIDE_PROTOCOL)
        }
        Err(error) => {
            eprintln!("specengine-ra: internal failure: cannot start the analysis thread: {error}");
            ExitCode::from(EXIT_OUTSIDE_PROTOCOL)
        }
    }
}

/// First in `main`, while the process has one thread: the seven overrides
/// removed, both wrappers set to the empty string. Every cargo and rustc
/// rust-analyzer starts inherits the result.
fn scrub_toolchain_overrides() {
    for variable in TOOLCHAIN_OVERRIDES {
        // SAFETY: no other thread exists yet (this runs first in `main`,
        // before any thread is spawned), so nothing reads the environment
        // concurrently.
        unsafe { std::env::remove_var(variable) };
    }
    for variable in EMPTIED_WRAPPERS {
        // SAFETY: as above, still single-threaded.
        unsafe { std::env::set_var(variable, "") };
    }
}

/// `TMPDIR` is the request's `tmp_dir`: rust-analyzer copies the lock file
/// there (`cargo metadata --locked` resolves against the copy), never
/// beside the project's own.
fn set_tmpdir(request: &Request) {
    // SAFETY: still single-threaded: the watchdogs and the analysis thread
    // start after this.
    unsafe { std::env::set_var("TMPDIR", &request.tmp_dir) };
}

fn refuse(reason: &str) -> ExitCode {
    // A refusal is the whole reply; a caller already gone cannot read it.
    let _ = reply::emit(&Refused::new(reason));
    ExitCode::from(2)
}

fn start_watchdogs(started: Instant, budget: Duration) {
    thread::spawn(|| {
        let mut stdin = io::stdin();
        let mut sink = [0_u8; 64];
        loop {
            match stdin.read(&mut sink) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        terminate("stdin closed");
    });
    thread::spawn(move || {
        let deadline = budget.saturating_add(GRACE);
        thread::sleep(deadline.saturating_sub(started.elapsed()));
        terminate("over the budget and its grace");
    });
}

/// Ends the run: the whole process group when this process leads it
/// (cargo, rustc and anything they started die with it), else this process
/// alone. One write, so the stderr line is whole or absent. After the last
/// line (`done` or `error`) the run has already ended: the process exits
/// with that line's code, silently, and kills nothing.
fn terminate(reason: &str) -> ! {
    if let Some(ending) = reply::last_line(LAST_LINE_PATIENCE) {
        std::process::exit(i32::from(ending.exit_code()));
    }
    let leads = sys::leads_its_group();
    let outcome = if leads {
        "stopping every process of this run"
    } else {
        "exiting alone (not leading a process group)"
    };
    let line = format!("specengine-ra: {reason}; {outcome}\n");
    let _ = io::stderr().write_all(line.as_bytes());
    if leads {
        sys::kill_own_group();
    }
    std::process::exit(i32::from(EXIT_OUTSIDE_PROTOCOL))
}
