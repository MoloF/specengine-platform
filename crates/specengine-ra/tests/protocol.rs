//! Protocol v1 of the `specengine-ra` executable on the real binary:
//! `docs/features/layer-c.md` "Acceptance criteria", B2-B8, against the
//! request, reply and endings of `docs/features/layer-c.md` "Protocol v1"
//! and the scrub, load and watchdogs of `docs/features/layer-c.md` "Binary".
//!
//! One file on purpose: each integration test links `ra_ap`. Every test
//! hands the binary a scratch copy of `fixtures/ra-mini` (the fixture itself
//! is never named in a request) and starts it under `perl`'s `alarm`, as the
//! leader of a process group of its own, which the test kills on every exit
//! path. Toolchain shims, a `cargo` and a `rustc` first on `PATH` and in a
//! scratch `$CARGO_HOME/bin`, log each command the binary starts and pass it
//! on to the tool of the same name on the test's own `PATH`, with the test's
//! own `CARGO_HOME`; B6's shims instead fork a sleeping grandchild and wait
//! for it.
//!
//! `fixtures/layer-c/ra-mini.jsonl` is the reply to the whole fixture with
//! the stamped values masked ([`MASKED`]); `spec resolve`'s fakes replay it.
//! To regenerate it, run `reply_equals_the_masked_golden` with
//! `SPECENGINE_RA_GOLDEN_OUT` naming a file outside the repository, review
//! that file, then copy it over the golden.

#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;
use specengine_code::{KIND_MAP, RustParser, analyze_file, kind_matches};
use specengine_ra::ItemKind;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-ra");

/// argv of protocol v1.
const PROTOCOL_ARGS: [&str; 2] = ["--protocol", "1"];

/// `budget_ms` of a full run: generous for a debug build of `ra_ap_*`.
const BUDGET_MS: u64 = 600_000;

/// The binary's own backstop past `budget_ms`.
const GRACE: Duration = Duration::from_secs(10);

/// How long a test waits for a full run: past the binary's backstop
/// (`BUDGET_MS` + [`GRACE`]), so a hung load ends in its own watchdog first.
const RUN_BOUND: Duration = Duration::from_secs(630);

/// `alarm` of a full run, past [`RUN_BOUND`].
const RUN_ALARM_S: u64 = 660;

/// A refusal comes before any discovery: seconds, never a load.
const REFUSAL_BOUND: Duration = Duration::from_secs(120);

/// `alarm` of a refusal and of a B6 run, past [`REFUSAL_BOUND`] and
/// [`FIRST_COMMAND_BOUND`].
const SHORT_ALARM_S: u64 = 150;

/// "its process group SHALL be gone within 5 s" (`docs/features/layer-c.md` "Rules and edge cases").
const GONE_WITHIN: Duration = Duration::from_secs(5);

/// B6's budget.
const SHORT_BUDGET: Duration = Duration::from_millis(1000);

/// B6's slack past [`SHORT_BUDGET`] plus [`GRACE`].
const SLACK: Duration = Duration::from_secs(5);

/// How long B6's grandchild sleeps when nothing kills it: bounded, so a
/// missed kill still ends on its own.
const GRANDCHILD_SLEEP_S: u64 = 60;

/// Bound of the wait for B6's first toolchain command.
const FIRST_COMMAND_BOUND: Duration = Duration::from_secs(120);

/// Reading what is left in a pipe once its writers are gone.
const PIPE_BOUND: Duration = Duration::from_secs(10);

const POLL: Duration = Duration::from_millis(20);

/// Starts its arguments under `alarm`: `perl -e PERL_ALARM <seconds> <program> <args>...`.
/// The alarm survives the `exec`, so the binary itself is bounded.
const PERL_ALARM: &str =
    "alarm shift @ARGV; exec { $ARGV[0] } @ARGV or die \"exec $ARGV[0]: $!\\n\"";

/// The seven toolchain overrides the binary removes first in `main`.
const OVERRIDES: [&str; 7] = [
    "RUSTUP_TOOLCHAIN",
    "CARGO",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
];

/// `MonikerStatus::as_str`, the keys of `done.by_state`.
const STATES: [&str; 5] = ["moniker", "local", "none", "unresolved", "not_loaded"];

/// The values a reply stamps (time, memory, toolchain, sysroot, host) and
/// what the golden holds in their place: a number stays a number and a
/// string a string, so a fake can replay the golden.
const MASKED: [(&str, &str); 5] = [
    ("ms", "0"),
    ("peak_rss", "0"),
    ("sysroot_packages", "0"),
    ("toolchain", "\"masked\""),
    ("host", "\"masked\""),
];

const GOLDEN: &str = "fixtures/layer-c/ra-mini.jsonl";

/// Names a file `reply_equals_the_masked_golden` writes its masked reply to.
const GOLDEN_OUT: &str = "SPECENGINE_RA_GOLDEN_OUT";

/// A peak RSS below this is not in bytes: any load of rust-analyzer's
/// database is larger.
const RSS_FLOOR_BYTES: u64 = 16 * 1024 * 1024;

/// Bytes of stderr a failure message shows.
const STDERR_TAIL: usize = 6000;

/// B3's same-name binary, added to the scratch copy only: package `arena`
/// then holds a library and a binary both named `arena`.
const SAME_NAME_BIN: &str = "//! A binary named like the library of its package.\n\nfn main() {\n    println!(\"{}\", banner());\n}\n\nfn banner() -> &'static str {\n    arena::TITLE\n}\n";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture_dir() -> PathBuf {
    repository_root().join("fixtures").join("ra-mini")
}

// ---------------------------------------------------------------------------
// Scratch trees: a copy of the fixture, whole-tree snapshots.
// ---------------------------------------------------------------------------

/// A canonical scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("specengine-ra-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        let path = fs::canonicalize(path).expect("canonical scratch directory");
        let text = path.to_str().expect("a UTF-8 scratch path");
        assert!(
            !text.contains('\'') && !text.contains('\n'),
            "the shims quote paths in sh: {text}"
        );
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Dir,
    File {
        bytes: Vec<u8>,
        modified: SystemTime,
    },
    Link(PathBuf),
}

/// Every entry under `root` (hidden ones included, symlinks not followed).
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Entry> {
    let mut entries = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("readable directory") {
            let entry = entry.expect("directory entry");
            let path = entry.path();
            let relative = path.strip_prefix(root).expect("under root").to_path_buf();
            let file_type = entry.file_type().expect("file type");
            if file_type.is_symlink() {
                entries.insert(relative, Entry::Link(fs::read_link(&path).expect("link")));
            } else if file_type.is_dir() {
                entries.insert(relative, Entry::Dir);
                stack.push(path);
            } else {
                let metadata = entry.metadata().expect("metadata");
                entries.insert(
                    relative,
                    Entry::File {
                        bytes: fs::read(&path).expect("readable file"),
                        modified: metadata.modified().expect("mtime"),
                    },
                );
            }
        }
    }
    entries
}

fn assert_unchanged(root: &Path, before: &BTreeMap<PathBuf, Entry>, context: &str) {
    let after = snapshot(root);
    let added: Vec<&PathBuf> = after.keys().filter(|k| !before.contains_key(*k)).collect();
    let removed: Vec<&PathBuf> = before.keys().filter(|k| !after.contains_key(*k)).collect();
    let changed: Vec<&PathBuf> = before
        .iter()
        .filter(|(k, v)| after.get(*k).is_some_and(|now| now != *v))
        .map(|(k, _)| k)
        .collect();
    assert!(
        added.is_empty() && removed.is_empty() && changed.is_empty(),
        "{context}: {} changed: added {added:?}, removed {removed:?}, modified {changed:?}",
        root.display()
    );
}

/// A copy of `fixtures/ra-mini` at `to` (files and directories only).
fn copy_fixture(to: &Path) {
    for (relative, entry) in snapshot(&fixture_dir()) {
        let target = to.join(&relative);
        match entry {
            Entry::Dir => fs::create_dir_all(&target).expect("copy dir"),
            Entry::File { bytes, .. } => {
                fs::create_dir_all(target.parent().expect("parent")).expect("copy parent");
                fs::write(&target, bytes).expect("copy file");
            }
            Entry::Link(_) => panic!("the fixture holds no symlinks: {}", relative.display()),
        }
    }
}

/// `relative` with `/` separators.
fn slash(relative: &Path) -> String {
    relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .expect("a UTF-8 path")
                .to_owned()
        })
        .collect::<Vec<String>>()
        .join("/")
}

/// Every `.rs` file under `root`, root-relative, byte-sorted: the `files` a
/// caller sends for the whole tree.
fn rust_files(root: &Path) -> Vec<String> {
    let mut files: Vec<String> = snapshot(root)
        .into_iter()
        .filter(|(path, entry)| {
            matches!(entry, Entry::File { .. }) && path.extension().is_some_and(|ext| ext == "rs")
        })
        .map(|(path, _)| slash(&path))
        .collect();
    files.sort();
    files
}

/// Every directory named `target` under `root`.
fn target_dirs(root: &Path) -> Vec<PathBuf> {
    snapshot(root)
        .into_iter()
        .filter(|(path, entry)| {
            matches!(entry, Entry::Dir) && path.file_name().is_some_and(|name| name == "target")
        })
        .map(|(path, _)| path)
        .collect()
}

/// `text` with `from` (found exactly once) replaced by `to`.
fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "`{from}` occurs once");
    text.replacen(from, to, 1)
}

// ---------------------------------------------------------------------------
// Toolchain shims.
// ---------------------------------------------------------------------------

/// How the `cargo` and `rustc` shims behave once they have logged.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shims {
    /// Pass the command on to the real tool.
    Forward,
    /// Fork a sleeping grandchild, record its pid, wait for it (B6).
    Block,
}

/// The first executable `name` on the test's own `PATH`.
fn real_tool(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH is set");
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
        .unwrap_or_else(|| panic!("no `{name}` on PATH: the shims pass commands on to it"))
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && (metadata.permissions().mode() & 0o111) != 0)
}

/// `path` single-quoted for sh.
fn quoted(path: &Path) -> String {
    let text = path.to_str().expect("a UTF-8 path");
    assert!(!text.contains('\''), "a quote in a shim path: {text}");
    format!("'{text}'")
}

/// An executable sh script at `path`.
fn write_script(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("the script's directory");
    fs::write(path, format!("#!/bin/sh\n{body}")).expect("the script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod 755");
}

/// sh lines that give a passed-on command the test's own `CARGO_HOME` (its
/// registry and configuration), not the scratch one holding the shims.
fn restore_cargo_home() -> String {
    match std::env::var_os("CARGO_HOME") {
        Some(home) => format!(
            "CARGO_HOME={}\nexport CARGO_HOME\n",
            quoted(Path::new(&home))
        ),
        None => "unset CARGO_HOME\n".to_owned(),
    }
}

/// sh lines that log, into `<overrides>/log`, each of the seven overrides
/// still pointing under `overrides` when a toolchain command starts.
fn leak_check(tool: &str, overrides: &Path) -> String {
    let variables = OVERRIDES.join(" ");
    let prefix = quoted(overrides);
    let log = quoted(&overrides.join("log"));
    format!(
        "for variable in {variables}; do\n  eval \"value=\\${{$variable-}}\"\n  case \"$value\" in\n    {prefix}*) printf '%s\\n' \"{tool} started with $variable=$value\" >> {log} ;;\n  esac\ndone\n"
    )
}

/// A JSON string of `path`.
fn json_path(path: &Path) -> String {
    json_text(path.to_str().expect("a UTF-8 path"))
}

fn json_text(text: &str) -> String {
    serde_json::to_string(text).expect("a JSON string")
}

fn json_strings(texts: &[&str]) -> String {
    serde_json::to_string(texts).expect("a JSON array")
}

// ---------------------------------------------------------------------------
// The lab: a scratch copy, the request's directories, the shims.
// ---------------------------------------------------------------------------

struct Lab {
    scratch: Scratch,
    /// The scratch copy of `fixtures/ra-mini`: the request's `root`.
    root: PathBuf,
    target_dir: PathBuf,
    tmp_dir: PathBuf,
    /// First on the binary's `PATH`.
    shim_bin: PathBuf,
    /// The binary's `CARGO_HOME`; its `bin/` holds the same shims.
    cargo_home: PathBuf,
    shim_log: PathBuf,
    /// B6: the pids of the shims' sleeping grandchildren, one per line.
    grandchildren_file: PathBuf,
    /// B7: where the overrides point and their log.
    overrides: PathBuf,
}

impl Lab {
    fn new(name: &str, shims: Shims) -> Lab {
        let scratch = Scratch::new(name);
        let base = scratch.path().to_path_buf();
        let lab = Lab {
            root: base.join("root"),
            target_dir: base.join("ra").join("target"),
            tmp_dir: base.join("ra").join("tmp"),
            shim_bin: base.join("shims").join("bin"),
            cargo_home: base.join("shims").join("cargo-home"),
            shim_log: base.join("shims").join("log"),
            grandchildren_file: base.join("shims").join("grandchildren"),
            overrides: base.join("overrides"),
            scratch,
        };
        copy_fixture(&lab.root);
        fs::create_dir_all(&lab.target_dir).expect("target_dir");
        fs::create_dir_all(&lab.tmp_dir).expect("tmp_dir");
        for tool in ["cargo", "rustc"] {
            let body = lab.shim_body(tool, shims);
            write_script(&lab.shim_bin.join(tool), &body);
            write_script(&lab.cargo_home.join("bin").join(tool), &body);
        }
        lab
    }

    fn shim_body(&self, tool: &str, shims: Shims) -> String {
        let mut body = format!(
            "printf '%s\\n' \"{tool} $*\" >> {}\n",
            quoted(&self.shim_log)
        );
        body.push_str(&leak_check(tool, &self.overrides));
        match shims {
            Shims::Forward => {
                body.push_str(&restore_cargo_home());
                body.push_str(&format!("exec {} \"$@\"\n", quoted(&real_tool(tool))));
            }
            Shims::Block => body.push_str(&format!(
                "sleep {GRANDCHILD_SLEEP_S} &\nprintf '%s\\n' \"$!\" >> {}\nwait\n",
                quoted(&self.grandchildren_file)
            )),
        }
        body
    }

    fn path_variable(&self) -> OsString {
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs = vec![self.shim_bin.clone()];
        dirs.extend(std::env::split_paths(&inherited));
        std::env::join_paths(dirs).expect("PATH joins")
    }

    /// The binary with `args` under `alarm`, leading a process group of its
    /// own, the shims first on `PATH` and in `CARGO_HOME`, no override set
    /// (B7's scrub test sets them on the command).
    fn command(&self, args: &[&str], alarm_s: u64) -> Command {
        let mut command = Command::new("perl");
        command
            .arg("-e")
            .arg(PERL_ALARM)
            .arg(alarm_s.to_string())
            .arg(BIN)
            .args(args)
            .current_dir(self.scratch.path())
            .env("PATH", self.path_variable())
            .env("CARGO_HOME", &self.cargo_home)
            .env("TMPDIR", &self.tmp_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        for variable in OVERRIDES {
            command.env_remove(variable);
        }
        command
    }

    /// A full run of `request` with protocol v1's argv.
    fn run(&self, request: &Request) -> Reply {
        run_command(
            self.command(&PROTOCOL_ARGS, RUN_ALARM_S),
            &request.line(),
            RUN_BOUND,
            "a full run",
        )
    }

    fn shim_log_text(&self) -> String {
        fs::read_to_string(&self.shim_log).unwrap_or_default()
    }

    fn grandchildren(&self) -> Vec<libc::pid_t> {
        fs::read_to_string(&self.grandchildren_file)
            .unwrap_or_default()
            .lines()
            .filter_map(|line| line.trim().parse().ok())
            .collect()
    }

    /// B6: the grandchildren's pids once a shim has forked one, `bound` at
    /// most; the binary must still be running.
    fn wait_for_grandchildren(&self, running: &mut Running, bound: Duration) -> Vec<libc::pid_t> {
        let deadline = Instant::now() + bound;
        loop {
            let pids = self.grandchildren();
            if !pids.is_empty() {
                return pids;
            }
            if let Some(status) = running.poll() {
                panic!(
                    "the binary exited ({status:?}) before a toolchain command started; shim log:\n{}",
                    self.shim_log_text()
                );
            }
            assert!(
                Instant::now() < deadline,
                "no toolchain command within {bound:?}: no shim forked its grandchild"
            );
            thread::sleep(POLL);
        }
    }

    /// The text of root-relative `path`.
    fn source(&self, path: &str) -> String {
        fs::read_to_string(self.root.join(path))
            .unwrap_or_else(|error| panic!("{path} of the scratch copy: {error}"))
    }

    /// `from` (once) replaced by `to` in root-relative `path`.
    fn edit(&self, path: &str, from: &str, to: &str) {
        let text = replace_once(&self.source(path), from, to);
        fs::write(self.root.join(path), text).expect("the edited file");
    }

    /// B7: the seven overrides, each pointing under `overrides` at a logging
    /// shim that passes the command on (the toolchain at a directory of
    /// them, the two target directories at paths nothing creates), as a
    /// caller's `cargo run` or `cargo +toolchain` may leave them.
    fn override_values(&self) -> Vec<(&'static str, PathBuf)> {
        let log = self.overrides.join("log");
        let forward = |label: &str, real: &Path| {
            format!(
                "printf '%s\\n' \"ran {label} $*\" >> {}\n{}exec {} \"$@\"\n",
                quoted(&log),
                restore_cargo_home(),
                quoted(real)
            )
        };
        let (cargo, rustc) = (real_tool("cargo"), real_tool("rustc"));
        let toolchain = self.overrides.join("toolchain");
        let wrapper = self.overrides.join("wrapper");
        write_script(&self.overrides.join("cargo"), &forward("cargo", &cargo));
        write_script(&self.overrides.join("rustc"), &forward("rustc", &rustc));
        write_script(
            &toolchain.join("bin").join("cargo"),
            &forward("toolchain cargo", &cargo),
        );
        write_script(
            &toolchain.join("bin").join("rustc"),
            &forward("toolchain rustc", &rustc),
        );
        write_script(
            &wrapper,
            &format!(
                "printf '%s\\n' \"ran wrapper $*\" >> {}\nexec \"$@\"\n",
                quoted(&log)
            ),
        );
        vec![
            ("RUSTUP_TOOLCHAIN", toolchain),
            ("CARGO", self.overrides.join("cargo")),
            ("RUSTC", self.overrides.join("rustc")),
            ("RUSTC_WRAPPER", wrapper.clone()),
            ("RUSTC_WORKSPACE_WRAPPER", wrapper),
            ("CARGO_TARGET_DIR", self.overrides.join("target")),
            (
                "CARGO_BUILD_TARGET_DIR",
                self.overrides.join("build-target"),
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// The request.
// ---------------------------------------------------------------------------

/// A request line, key by key, each value raw JSON, in the order of
/// `docs/features/layer-c.md` "Protocol v1".
#[derive(Clone)]
struct Request(Vec<(&'static str, String)>);

impl Request {
    fn new(lab: &Lab, files: &[String]) -> Request {
        Request(vec![
            ("protocol", "1".to_owned()),
            ("root", json_path(&lab.root)),
            (
                "files",
                serde_json::to_string(files).expect("files as JSON"),
            ),
            ("target_dir", json_path(&lab.target_dir)),
            ("tmp_dir", json_path(&lab.tmp_dir)),
            ("budget_ms", BUDGET_MS.to_string()),
        ])
    }

    /// This request with `key` set to `raw` (added when absent).
    fn with(&self, key: &'static str, raw: String) -> Request {
        let mut next = self.clone();
        match next.0.iter_mut().find(|(name, _)| *name == key) {
            Some(field) => field.1 = raw,
            None => next.0.push((key, raw)),
        }
        next
    }

    fn without(&self, key: &str) -> Request {
        let mut next = self.clone();
        next.0.retain(|(name, _)| *name != key);
        next
    }

    fn line(&self) -> String {
        let fields: Vec<String> = self
            .0
            .iter()
            .map(|(key, raw)| format!("\"{key}\":{raw}"))
            .collect();
        format!("{{{}}}", fields.join(","))
    }
}

// ---------------------------------------------------------------------------
// A running binary and its reply.
// ---------------------------------------------------------------------------

/// The binary, started; its group killed on drop, whatever happened.
struct Running {
    child: Child,
    pgid: libc::pid_t,
    stdin: Option<ChildStdin>,
    /// The whole of stdout, read to its end ([`Running::start`]); `None`
    /// when it is read line by line ([`Running::start_lines`]).
    stdout: Option<mpsc::Receiver<Vec<u8>>>,
    stderr: mpsc::Receiver<Vec<u8>>,
    status: Option<ExitStatus>,
    started: Instant,
}

/// A pipe read to its end on a thread of its own.
fn drain(mut pipe: impl Read + Send + 'static) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    receiver
}

/// A pipe read line by line on a thread of its own, each line sent as it
/// comes.
fn line_by_line(pipe: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(pipe).lines() {
            let Ok(line) = line else {
                break;
            };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

fn take(pipe: &mpsc::Receiver<Vec<u8>>, name: &str) -> Vec<u8> {
    pipe.recv_timeout(PIPE_BOUND).unwrap_or_else(|_| {
        format!("<{name} still open {PIPE_BOUND:?} after the run: a descendant holds it>")
            .into_bytes()
    })
}

/// Whether a signal could reach `target` (a pid, or `-pgid` for a group).
fn signal_reaches(target: libc::pid_t) -> bool {
    // SAFETY: signal 0 only checks for existence; no pointers.
    if unsafe { libc::kill(target, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

fn group_alive(pgid: libc::pid_t) -> bool {
    signal_reaches(-pgid)
}

/// SIGKILL to the binary's group: never the test's own.
fn kill_group(pgid: libc::pid_t) {
    // SAFETY: no arguments.
    let own = unsafe { libc::getpgrp() };
    if pgid > 1 && pgid != own {
        // SAFETY: no pointers; the group is the binary's (`process_group(0)`).
        unsafe { libc::kill(-pgid, libc::SIGKILL) };
    }
}

impl Running {
    /// The binary, its stdout read to its end.
    fn start(command: Command) -> Running {
        let (mut running, stdout) = Running::spawn(command);
        running.stdout = Some(drain(stdout));
        running
    }

    /// The binary, its stdout read line by line.
    fn start_lines(command: Command) -> (Running, mpsc::Receiver<String>) {
        let (running, stdout) = Running::spawn(command);
        (running, line_by_line(stdout))
    }

    fn spawn(mut command: Command) -> (Running, ChildStdout) {
        let started = Instant::now();
        let mut child = command
            .spawn()
            .unwrap_or_else(|error| panic!("perl starts the binary under `alarm`: {error}"));
        let pgid = libc::pid_t::try_from(child.id()).expect("a pid fits pid_t");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = drain(child.stderr.take().expect("a stderr pipe"));
        let running = Running {
            child,
            pgid,
            stdin,
            stdout: None,
            stderr,
            status: None,
            started,
        };
        (running, stdout.expect("a stdout pipe"))
    }

    /// The exit status, `bound` from now at most: past it the test fails
    /// (and the drop kills the group).
    fn wait_exit(&mut self, bound: Duration, what: &str) -> ExitStatus {
        let deadline = Instant::now() + bound;
        loop {
            if let Some(status) = self.poll() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: the binary did not exit within {bound:?}"
            );
            thread::sleep(POLL);
        }
    }

    /// One line on stdin, which stays open.
    fn send(&mut self, line: &str) {
        if let Some(stdin) = self.stdin.as_mut() {
            // A binary that refused its argv may be gone already: a broken
            // pipe is its answer, read from its stdout.
            let _ = stdin.write_all(format!("{line}\n").as_bytes());
        }
    }

    fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// The exit status once the binary has exited (and been reaped).
    fn poll(&mut self) -> Option<ExitStatus> {
        if self.status.is_none() {
            self.status = self.child.try_wait().expect("try_wait on the binary");
        }
        self.status
    }

    /// Waits for the binary's exit, `bound` after its start at most, then
    /// [`GONE_WITHIN`] for its group to be gone; the reply. Either wait
    /// overrun kills the group and fails.
    fn finish(mut self, bound: Duration, what: &str) -> Reply {
        let deadline = self.started + bound;
        let mut overran = false;
        let status = loop {
            if let Some(status) = self.poll() {
                break status;
            }
            if Instant::now() >= deadline {
                overran = true;
                kill_group(self.pgid);
                let status = self.child.wait().expect("wait for the killed binary");
                self.status = Some(status);
                break status;
            }
            thread::sleep(POLL);
        };
        self.close_stdin();
        let gone_by = Instant::now() + GONE_WITHIN;
        let mut outlived = false;
        while group_alive(self.pgid) {
            if Instant::now() >= gone_by {
                outlived = true;
                kill_group(self.pgid);
                break;
            }
            thread::sleep(POLL);
        }
        let reply = Reply {
            status,
            stdout: String::from_utf8_lossy(
                &self
                    .stdout
                    .as_ref()
                    .map_or_else(Vec::new, |pipe| take(pipe, "stdout")),
            )
            .into_owned(),
            stderr: String::from_utf8_lossy(&take(&self.stderr, "stderr")).into_owned(),
        };
        assert!(
            !overran,
            "{what}: the binary did not exit within {bound:?}; its group was killed\n{}",
            reply.context()
        );
        assert!(
            !outlived,
            "{what}: the binary exited, but its process group outlived it by {GONE_WITHIN:?} (killed now)\n{}",
            reply.context()
        );
        reply
    }

    /// Polls until the binary has exited, its group is gone and so is every
    /// pid `grandchildren` names, or until `deadline`: what is alive then.
    fn wait_gone(
        &mut self,
        grandchildren: impl Fn() -> Vec<libc::pid_t>,
        deadline: Instant,
    ) -> Vec<String> {
        loop {
            let mut alive = Vec::new();
            if self.poll().is_none() {
                alive.push(format!("the binary ({})", self.pgid));
            }
            if group_alive(self.pgid) {
                alive.push(format!("its process group {}", self.pgid));
            }
            for pid in grandchildren() {
                if signal_reaches(pid) {
                    alive.push(format!("grandchild {pid}"));
                }
            }
            if alive.is_empty() || Instant::now() >= deadline {
                return alive;
            }
            thread::sleep(POLL);
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // Every exit path, a failed assertion included: nothing of the run
        // outlives its test.
        kill_group(self.pgid);
        if self.status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn run_command(command: Command, line: &str, bound: Duration, what: &str) -> Reply {
    let mut running = Running::start(command);
    running.send(line);
    running.finish(bound, what)
}

struct Reply {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl Reply {
    fn context(&self) -> String {
        format!(
            "{:?}\n--- stdout ---\n{}--- stderr, last {STDERR_TAIL} bytes ---\n{}",
            self.status,
            self.stdout,
            tail(&self.stderr, STDERR_TAIL)
        )
    }
}

fn tail(text: &str, bytes: usize) -> &str {
    let mut from = text.len().saturating_sub(bytes);
    while !text.is_char_boundary(from) {
        from += 1;
    }
    &text[from..]
}

/// Every stdout line as JSON, each opening with the key `t`.
fn parse_lines(reply: &Reply) -> Vec<Value> {
    reply
        .stdout
        .lines()
        .enumerate()
        .map(|(index, line)| {
            assert!(
                line.starts_with("{\"t\":\""),
                "line {} does not open with the key `t`: {line}\n{}",
                index + 1,
                reply.context()
            );
            serde_json::from_str::<Value>(line).unwrap_or_else(|error| {
                panic!(
                    "line {} is not JSON ({error}): {line}\n{}",
                    index + 1,
                    reply.context()
                )
            })
        })
        .collect()
}

/// A reply that ended in `done`.
struct Answer {
    lines: Vec<Value>,
}

/// The lines of a reply that exited 0 with `done` last.
fn completed(reply: &Reply) -> Answer {
    assert_eq!(
        reply.status.code(),
        Some(0),
        "a run that ends in `done` exits 0\n{}",
        reply.context()
    );
    let lines = parse_lines(reply);
    assert!(
        lines.last().is_some_and(|line| line["t"] == "done"),
        "`done` is the last line\n{}",
        reply.context()
    );
    Answer { lines }
}

impl Answer {
    fn files(&self) -> impl Iterator<Item = &Value> {
        self.lines.iter().filter(|line| line["t"] == "file")
    }

    /// The one `file` line of `path`.
    fn file(&self, path: &str) -> &Value {
        let found: Vec<&Value> = self.files().filter(|line| line["path"] == path).collect();
        assert_eq!(found.len(), 1, "one `file` line for {path}: {found:?}");
        found[0]
    }
}

/// The keys of a JSON object, sorted.
fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    keys.sort_unstable();
    keys
}

fn as_u64(value: &Value, what: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{what} is not a non-negative integer: {value}"))
}

fn as_usize(value: &Value) -> usize {
    usize::try_from(as_u64(value, "an offset")).expect("an offset fits usize")
}

fn items(file: &Value) -> &[Value] {
    file["items"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("`items` is not an array: {file}"))
}

fn item_starting_at(file: &Value, start: usize) -> &Value {
    items(file)
        .iter()
        .find(|item| item["start"] == start)
        .unwrap_or_else(|| {
            panic!(
                "{}: no item starts at byte {start}: {}",
                file["path"], file["items"]
            )
        })
}

/// The item of `file` starting where `needle` (found once in `source`)
/// does: the first token after its attributes and comments.
fn item_at<'a>(file: &'a Value, source: &str, needle: &str) -> &'a Value {
    let starts: Vec<usize> = source.match_indices(needle).map(|(at, _)| at).collect();
    assert_eq!(
        starts.len(),
        1,
        "`{needle}` occurs once in {}",
        file["path"]
    );
    item_starting_at(file, starts[0])
}

fn assert_state(item: &Value, state: &str, what: &str) {
    assert_eq!(item["state"], state, "{what}: {item}");
}

/// `(kind, state, moniker)` of every item, per file: offsets left out.
fn monikers(answer: &Answer) -> BTreeMap<String, Vec<(String, String, Value)>> {
    answer
        .files()
        .map(|file| {
            let path = file["path"].as_str().unwrap_or_default().to_owned();
            let entries: Vec<(String, String, Value)> = items(file)
                .iter()
                .map(|item| {
                    (
                        item["kind"].as_str().unwrap_or_default().to_owned(),
                        item["state"].as_str().unwrap_or_default().to_owned(),
                        item["moniker"].clone(),
                    )
                })
                .collect();
            (path, entries)
        })
        .collect()
}

fn starts(file: &Value) -> Vec<u64> {
    items(file)
        .iter()
        .map(|item| as_u64(&item["start"], "start"))
        .collect()
}

// ---------------------------------------------------------------------------
// The masked golden.
// ---------------------------------------------------------------------------

/// `reply` with every [`MASKED`] value replaced, line by line.
fn masked(reply: &str) -> String {
    let mut out = String::with_capacity(reply.len());
    for line in reply.lines() {
        out.push_str(&mask_line(line));
        out.push('\n');
    }
    out
}

fn mask_line(line: &str) -> String {
    let mut out = line.to_owned();
    for (key, replacement) in MASKED {
        let needle = format!("\"{key}\":");
        let mut from = 0;
        while let Some(found) = out[from..].find(&needle) {
            let start = from + found + needle.len();
            let end = value_end(&out, start);
            out.replace_range(start..end, replacement);
            from = start + replacement.len();
        }
    }
    out
}

/// The end of the JSON scalar starting at byte `start` of `text`: a string
/// through its closing quote, anything else up to the next `,`, `}` or `]`.
fn value_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    if bytes.get(start) == Some(&b'"') {
        let mut at = start + 1;
        while let Some(&byte) = bytes.get(at) {
            match byte {
                b'\\' => at += 2,
                b'"' => return at + 1,
                _ => at += 1,
            }
        }
        panic!("an unterminated string at byte {start}: {text}");
    }
    bytes[start..]
        .iter()
        .position(|byte| matches!(byte, b',' | b'}' | b']'))
        .map_or(bytes.len(), |offset| start + offset)
}

/// The mask on the spec's example reply: the stamped values only, a `null`
/// toolchain and an escaped quote included; `items`, the counts and the
/// moniker untouched.
#[test]
fn masking_replaces_only_the_stamped_values() {
    let reply = concat!(
        "{\"t\":\"hello\",\"protocol\":1,\"ra_ap\":\"0.0.352\",\"host\":\"aarch64-macos\"}\n",
        "{\"t\":\"step\",\"workspace\":1,\"dir\":\".\",\"step\":\"discover\",\"ms\":2140,\"toolchain\":\"1.98.0\",\"sysroot_packages\":43,\"metadata_degraded\":false}\n",
        "{\"t\":\"step\",\"workspace\":2,\"dir\":\"b\",\"step\":\"discover\",\"ms\":7,\"toolchain\":null,\"sysroot_packages\":0,\"metadata_degraded\":true}\n",
        "{\"t\":\"step\",\"workspace\":1,\"dir\":\".\",\"step\":\"database\",\"ms\":6120}\n",
        "{\"t\":\"file\",\"path\":\"arena/src/lib.rs\",\"in_crate\":true,\"items\":[{\"start\":318,\"end\":402,\"kind\":\"impl\",\"state\":\"moniker\",\"moniker\":\"arena impl#[Health]\"}]}\n",
        "{\"t\":\"done\",\"files\":6,\"items\":29,\"by_state\":{\"moniker\":26,\"local\":0,\"none\":0,\"unresolved\":2,\"not_loaded\":1},\"peak_rss\":2914000000,\"ms\":9100}\n",
        "{\"t\":\"hello\",\"protocol\":1,\"ra_ap\":\"0.0.352\",\"host\":\"a\\\"b,c}\"}\n",
    );
    let want = concat!(
        "{\"t\":\"hello\",\"protocol\":1,\"ra_ap\":\"0.0.352\",\"host\":\"masked\"}\n",
        "{\"t\":\"step\",\"workspace\":1,\"dir\":\".\",\"step\":\"discover\",\"ms\":0,\"toolchain\":\"masked\",\"sysroot_packages\":0,\"metadata_degraded\":false}\n",
        "{\"t\":\"step\",\"workspace\":2,\"dir\":\"b\",\"step\":\"discover\",\"ms\":0,\"toolchain\":\"masked\",\"sysroot_packages\":0,\"metadata_degraded\":true}\n",
        "{\"t\":\"step\",\"workspace\":1,\"dir\":\".\",\"step\":\"database\",\"ms\":0}\n",
        "{\"t\":\"file\",\"path\":\"arena/src/lib.rs\",\"in_crate\":true,\"items\":[{\"start\":318,\"end\":402,\"kind\":\"impl\",\"state\":\"moniker\",\"moniker\":\"arena impl#[Health]\"}]}\n",
        "{\"t\":\"done\",\"files\":6,\"items\":29,\"by_state\":{\"moniker\":26,\"local\":0,\"none\":0,\"unresolved\":2,\"not_loaded\":1},\"peak_rss\":0,\"ms\":0}\n",
        "{\"t\":\"hello\",\"protocol\":1,\"ra_ap\":\"0.0.352\",\"host\":\"masked\"}\n",
    );
    assert_eq!(masked(reply), want);
}

// ---------------------------------------------------------------------------
// B2: protocol v1 and the masked golden.
// ---------------------------------------------------------------------------

/// B2: `hello` first, then the `step` lines; one `file` line per requested
/// path, in request order, its items by `start`; `done` last, its counts
/// those of the `file` lines, exit 0; every line opens with `t` and has
/// exactly its keys; the copy stays byte-identical, with no `target/`.
/// M: `done.items` counted before the `scan_unloaded` files.
#[test]
fn reply_follows_protocol_v1() {
    let lab = Lab::new("protocol", Shims::Forward);
    let files = rust_files(&lab.root);
    assert_eq!(
        files.len(),
        6,
        "the .rs files of fixtures/ra-mini: {files:?}"
    );
    let before = snapshot(&lab.root);
    let reply = lab.run(&Request::new(&lab, &files));
    let answer = completed(&reply);
    let context = reply.context();
    let lines = &answer.lines;
    let types: Vec<&str> = lines
        .iter()
        .map(|line| line["t"].as_str().unwrap_or_default())
        .collect();

    let hello = &lines[0];
    assert_eq!(types[0], "hello", "`hello` first\n{context}");
    assert_eq!(keys(hello), ["host", "protocol", "ra_ap", "t"], "{hello}");
    assert_eq!(hello["protocol"], 1, "{hello}");
    assert_eq!(hello["ra_ap"], "0.0.352", "{hello}");
    assert_eq!(hello["ra_ap"], specengine_ra::RA_AP_VERSION, "{hello}");
    let host = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    assert_eq!(hello["host"], host, "{hello}");

    let first_file = types
        .iter()
        .position(|t| *t == "file")
        .unwrap_or_else(|| panic!("no `file` line\n{context}"));
    let done_at = lines.len() - 1;
    assert!(
        first_file > 1 && types[1..first_file].iter().all(|t| *t == "step"),
        "after `hello`, the `step` lines and only them: {types:?}"
    );
    assert!(
        types[first_file..done_at].iter().all(|t| *t == "file"),
        "after the steps, the `file` lines and only them, then `done`: {types:?}"
    );

    // ra-mini is one workspace, `.`: discovered, then loaded.
    let steps = &lines[1..first_file];
    let order: Vec<(u64, &str, &str)> = steps
        .iter()
        .map(|step| {
            (
                step["workspace"].as_u64().unwrap_or_default(),
                step["dir"].as_str().unwrap_or_default(),
                step["step"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        order,
        [(1, ".", "discover"), (1, ".", "database")],
        "{context}"
    );
    let discover = &steps[0];
    assert_eq!(
        keys(discover),
        [
            "dir",
            "metadata_degraded",
            "ms",
            "step",
            "sysroot_packages",
            "t",
            "toolchain",
            "workspace"
        ],
        "{discover}"
    );
    as_u64(&discover["ms"], "discover.ms");
    as_u64(&discover["sysroot_packages"], "discover.sysroot_packages");
    assert!(
        discover["toolchain"].is_null()
            || discover["toolchain"]
                .as_str()
                .is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit())),
        "{discover}"
    );
    assert_eq!(
        discover["metadata_degraded"], false,
        "ra-mini's own lock resolves offline: {discover}"
    );
    let database = &steps[1];
    assert_eq!(
        keys(database),
        ["dir", "ms", "step", "t", "workspace"],
        "{database}"
    );
    as_u64(&database["ms"], "database.ms");

    let file_lines = &lines[first_file..done_at];
    let paths: Vec<&str> = file_lines
        .iter()
        .map(|file| file["path"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        paths, files,
        "one `file` line per requested path, in request order"
    );
    let kinds: BTreeSet<&str> = ItemKind::ALL.iter().map(|kind| kind.as_str()).collect();
    let mut tally: BTreeMap<&str, u64> = STATES.iter().map(|state| (*state, 0)).collect();
    let mut total = 0_u64;
    for file in file_lines {
        assert_eq!(keys(file), ["in_crate", "items", "path", "t"], "{file}");
        assert!(file["in_crate"].is_boolean(), "{file}");
        let path = file["path"].as_str().unwrap_or_default();
        let length = u64::try_from(lab.source(path).len()).expect("a file length");
        let mut previous: Option<u64> = None;
        for item in items(file) {
            assert_eq!(
                keys(item),
                ["end", "kind", "moniker", "start", "state"],
                "{path}: {item}"
            );
            let start = as_u64(&item["start"], "start");
            let end = as_u64(&item["end"], "end");
            assert!(
                start < end && end <= length,
                "{path}: {start}..{end} in {length} bytes: {item}"
            );
            assert!(
                previous.is_none_or(|before| before < start),
                "{path}: items not by `start`: {}",
                file["items"]
            );
            previous = Some(start);
            let kind = item["kind"].as_str().unwrap_or_default();
            assert!(
                kinds.contains(kind),
                "{path}: `{kind}` is no `ItemKind::as_str`: {item}"
            );
            let state = item["state"].as_str().unwrap_or_default();
            let Some(count) = tally.get_mut(state) else {
                panic!("{path}: `{state}` is no `MonikerStatus::as_str`: {item}");
            };
            *count += 1;
            total += 1;
            if state == "moniker" {
                assert!(
                    item["moniker"]
                        .as_str()
                        .is_some_and(|text| !text.is_empty()),
                    "{path}: a `moniker` item without its moniker: {item}"
                );
            } else {
                assert!(
                    item["moniker"].is_null(),
                    "{path}: a moniker on a `{state}` item: {item}"
                );
            }
        }
    }

    let done = &lines[done_at];
    assert_eq!(
        keys(done),
        ["by_state", "files", "items", "ms", "peak_rss", "t"],
        "{done}"
    );
    assert_eq!(done["files"], file_lines.len(), "done.files: {done}");
    assert_eq!(
        done["items"], total,
        "done.items against the items of the `file` lines: {done}"
    );
    let mut states = STATES.to_vec();
    states.sort_unstable();
    assert_eq!(keys(&done["by_state"]), states, "{done}");
    for (state, count) in &tally {
        assert_eq!(
            done["by_state"][*state], *count,
            "done.by_state.{state} against the `file` lines: {done}"
        );
    }
    let peak = as_u64(&done["peak_rss"], "done.peak_rss");
    assert!(
        peak >= RSS_FLOOR_BYTES,
        "done.peak_rss {peak} is below {RSS_FLOOR_BYTES}: not bytes? {done}"
    );
    as_u64(&done["ms"], "done.ms");

    assert_unchanged(&lab.root, &before, "a run on the scratch copy");
    assert_eq!(
        target_dirs(&lab.root),
        Vec::<PathBuf>::new(),
        "a `target/` under the root"
    );
}

/// B2, A6: the reply to the whole fixture, its stamped values masked
/// ([`MASKED`]), equals `fixtures/layer-c/ra-mini.jsonl` byte for byte, and
/// holds no path of the scratch tree. Two runs (any two invocations of this
/// test) give the same bytes. M: items in moniker order.
#[test]
fn reply_equals_the_masked_golden() {
    let lab = Lab::new("golden", Shims::Forward);
    let files = rust_files(&lab.root);
    let reply = lab.run(&Request::new(&lab, &files));
    completed(&reply);
    let masked = masked(&reply.stdout);
    let scratch = lab.scratch.path().to_str().expect("a UTF-8 scratch path");
    assert!(
        !masked.contains(scratch),
        "the reply names a path of the scratch tree:\n{masked}"
    );
    if let Some(out) = std::env::var_os(GOLDEN_OUT) {
        fs::write(out, &masked).expect("the masked reply for review");
    }
    let golden = fs::read_to_string(repository_root().join(GOLDEN)).unwrap_or_else(|error| {
        panic!(
            "{GOLDEN}: {error}; generate it with {GOLDEN_OUT}=<a file outside the repository>, review, copy\nmasked reply:\n{masked}"
        )
    });
    if masked != golden {
        let line = masked
            .lines()
            .zip(golden.lines())
            .position(|(now, then)| now != then)
            .unwrap_or_else(|| masked.lines().count().min(golden.lines().count()))
            + 1;
        panic!(
            "the masked reply differs from {GOLDEN} from line {line} on\n--- masked reply ---\n{masked}--- golden ---\n{golden}"
        );
    }
}

// ---------------------------------------------------------------------------
// B3: the fixture's states.
// ---------------------------------------------------------------------------

/// B3: `stray_helper` (outside the module tree) and `windows_overlay`
/// (`cfg`-disabled) `unresolved`; `tools/probe.rs` `in_crate` false, every
/// item `not_loaded`; `mod tests` `moniker` (`cfg(test)` on); the two
/// `impl Health` share one raw moniker; a library and a same-name binary
/// (added to the scratch copy only) both answered. M: `set_test: false`.
#[test]
fn ra_mini_states_match_the_fixture() {
    let lab = Lab::new("states", Shims::Forward);
    fs::write(lab.root.join("arena/src/main.rs"), SAME_NAME_BIN).expect("the same-name binary");
    let files = rust_files(&lab.root);
    assert!(
        files.iter().any(|file| file == "arena/src/main.rs"),
        "{files:?}"
    );
    let reply = lab.run(&Request::new(&lab, &files));
    let answer = completed(&reply);

    let lib = lab.source("arena/src/lib.rs");
    let lib_line = answer.file("arena/src/lib.rs");
    assert_eq!(lib_line["in_crate"], true, "{lib_line}");
    assert_state(
        item_at(lib_line, &lib, "pub fn windows_overlay"),
        "unresolved",
        "a cfg-disabled function",
    );

    let stray = lab.source("arena/src/stray.rs");
    assert_state(
        item_at(
            answer.file("arena/src/stray.rs"),
            &stray,
            "pub fn stray_helper",
        ),
        "unresolved",
        "a function outside every module tree",
    );

    let probe = answer.file("tools/probe.rs");
    assert_eq!(probe["in_crate"], false, "{probe}");
    assert!(!items(probe).is_empty(), "{probe}");
    for item in items(probe) {
        assert_state(item, "not_loaded", "a file outside every package");
        assert!(item["moniker"].is_null(), "{item}");
    }

    assert_state(
        item_at(lib_line, &lib, "mod tests {"),
        "moniker",
        "`mod tests` under cfg(test)",
    );
    assert_state(
        item_at(lib_line, &lib, "fn labels_every_component"),
        "moniker",
        "a test function under cfg(test)",
    );

    let impls: Vec<&Value> = lib
        .match_indices("impl Health {")
        .map(|(at, _)| item_starting_at(lib_line, at))
        .collect();
    assert_eq!(impls.len(), 2, "two `impl Health` blocks in lib.rs");
    for block in &impls {
        assert_eq!(block["kind"], "impl", "{block}");
        assert_state(block, "moniker", "an inherent impl");
    }
    assert_eq!(
        impls[0]["moniker"], impls[1]["moniker"],
        "the two `impl Health` share one raw moniker"
    );

    let health = item_at(lib_line, &lib, "pub struct Health");
    assert_state(health, "moniker", "the library's struct");
    assert!(
        health["moniker"]
            .as_str()
            .is_some_and(|moniker| moniker.starts_with("arena ")),
        "a moniker opens with its crate: {health}"
    );
    let main = lab.source("arena/src/main.rs");
    let main_line = answer.file("arena/src/main.rs");
    assert_eq!(main_line["in_crate"], true, "{main_line}");
    for needle in ["fn main()", "fn banner()"] {
        let item = item_at(main_line, &main, needle);
        assert_state(item, "moniker", "the same-name binary's function");
        assert!(
            item["moniker"]
                .as_str()
                .is_some_and(|moniker| moniker.starts_with("arena ")),
            "the binary's crate is `arena` too: {item}"
        );
    }
}

// ---------------------------------------------------------------------------
// B4: item bytes against layer A.
// ---------------------------------------------------------------------------

/// The layer A kinds an item kind of the reply stands for, restated from
/// `docs/features/layer-c.md` "Kind map" as this test's oracle.
fn spec_kinds(kind: &str) -> Vec<String> {
    let kinds: &[&str] = match kind {
        "method" => &["function_item", "function_signature_item"],
        "type_alias" => &["type_item"],
        "mod" => &["mod_item"],
        "macro_rules" | "macro_def" => &["macro_definition"],
        other => return vec![format!("{other}_item")],
    };
    kinds.iter().map(|kind| (*kind).to_owned()).collect()
}

/// A doc comment, a marker line and a derive above a struct, a doc comment
/// and an attribute above a method, a marker line above a function: on the
/// scratch copy only.
fn annotate(lab: &Lab) {
    let path = "arena/src/systems.rs";
    lab.edit(
        path,
        "pub struct Regeneration {",
        "/// Health regained per second.\n// @implements REGEN-RATE@1\n#[derive(Clone, Copy, Debug)]\npub struct Regeneration {",
    );
    lab.edit(
        path,
        "    fn tick(&mut self, _dt: f32) {",
        "    /// One point per tick.\n    #[inline]\n    fn tick(&mut self, _dt: f32) {",
    );
    lab.edit(
        path,
        "pub fn regenerate(",
        "// @implements REGEN-TICK@1\npub fn regenerate(",
    );
}

/// Every `.rs` file of `root` with CRLF line ends.
fn to_crlf(root: &Path) {
    for path in rust_files(root) {
        let file = root.join(&path);
        let text = fs::read_to_string(&file).expect("a source file");
        assert!(!text.contains('\r'), "{path} has CR already");
        fs::write(&file, text.replace('\n', "\r\n")).expect("the CRLF copy");
    }
}

/// Up to 32 characters of `source` from byte `at`, for a failure message.
fn snippet(source: &str, at: usize) -> String {
    source
        .get(at..)
        .unwrap_or_default()
        .chars()
        .take(32)
        .collect()
}

/// B4: each item's `(start, end, kind)` equals layer A's row on the same
/// bytes (the kind map of `docs/features/layer-c.md` "Kind map"), over the
/// fixture's `#[derive]` and `///`, a marker line above an item and a doc
/// comment and attribute above a method, on an LF copy and on a CRLF copy.
/// `specengine-code`'s `KIND_MAP` and `kind_matches` are the spec's map.
/// M: `start` from the syntax node's start.
#[test]
fn item_bytes_equal_layer_a() {
    let ordered: Vec<&str> = ItemKind::ALL.iter().map(|kind| kind.as_str()).collect();
    let rows: Vec<&str> = KIND_MAP.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(rows, ordered, "KIND_MAP's rows follow ItemKind::ALL");
    for (kind, layer_a) in KIND_MAP {
        let row: Vec<String> = layer_a.iter().map(|kind| (*kind).to_owned()).collect();
        assert_eq!(row, spec_kinds(kind), "the kind map's row of `{kind}`");
    }

    let mut parser = RustParser::new().expect("the tree-sitter Rust grammar loads");
    for crlf in [false, true] {
        let label = if crlf { "crlf" } else { "lf" };
        let lab = Lab::new(&format!("bytes-{label}"), Shims::Forward);
        annotate(&lab);
        if crlf {
            to_crlf(&lab.root);
        }
        let files = rust_files(&lab.root);
        let reply = lab.run(&Request::new(&lab, &files));
        let answer = completed(&reply);
        let mut mismatches = Vec::new();
        let mut compared = 0;
        for path in &files {
            let source = lab.source(path);
            assert_eq!(source.contains('\r'), crlf, "{label}: {path}");
            let analysis = analyze_file(&mut parser, &source)
                .unwrap_or_else(|| panic!("{label}: layer A parses {path}"));
            let mut layer_a: Vec<(usize, usize, String)> = analysis
                .items
                .iter()
                .map(|record| {
                    (
                        record.byte_range.start,
                        record.byte_range.end,
                        record.kind.clone(),
                    )
                })
                .collect();
            layer_a.sort();
            let mut layer_c: Vec<(usize, usize, String)> = items(answer.file(path))
                .iter()
                .map(|item| {
                    (
                        as_usize(&item["start"]),
                        as_usize(&item["end"]),
                        item["kind"].as_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect();
            layer_c.sort();
            if layer_a.len() != layer_c.len() {
                mismatches.push(format!(
                    "{path}: layer A has {} items, the reply {}\n  A {layer_a:?}\n  C {layer_c:?}",
                    layer_a.len(),
                    layer_c.len()
                ));
                continue;
            }
            compared += layer_c.len();
            for (a, c) in layer_a.iter().zip(&layer_c) {
                let in_map = spec_kinds(&c.2).contains(&a.2);
                assert_eq!(
                    kind_matches(&c.2, &a.2),
                    in_map,
                    "specengine-code's kind_matches against the spec's map on `{}` / `{}`",
                    c.2,
                    a.2
                );
                if a.0 != c.0 || a.1 != c.1 || !in_map {
                    mismatches.push(format!(
                        "{path}: layer A {} {}..{} {:?}, the reply {} {}..{} {:?}",
                        a.2,
                        a.0,
                        a.1,
                        snippet(&source, a.0),
                        c.2,
                        c.0,
                        c.1,
                        snippet(&source, c.0)
                    ));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "{label}: items whose bytes or kind differ from layer A's:\n{}\n{}",
            mismatches.join("\n"),
            reply.context()
        );
        assert!(
            compared >= 25,
            "{label}: only {compared} items compared\n{}",
            reply.context()
        );
    }
}

// ---------------------------------------------------------------------------
// B5: refusals before discovery.
// ---------------------------------------------------------------------------

/// What is wrong with `reply` as a refusal, if anything: exit 2 and exactly
/// one line, `refused` with the binary's own protocol and a reason.
fn refusal_problem(reply: &Reply) -> Option<String> {
    if reply.status.code() != Some(2) {
        return Some(format!("{:?}, not exit 2", reply.status));
    }
    let lines: Vec<&str> = reply.stdout.lines().collect();
    if lines.len() != 1 {
        return Some(format!(
            "{} stdout lines, not one `refused` line",
            lines.len()
        ));
    }
    let line = lines[0];
    if !line.starts_with("{\"t\":\"refused\"") {
        return Some(format!("not a `refused` line with `t` first: {line}"));
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Some(format!("not JSON: {line}"));
    };
    if keys(&value) != ["protocol", "reason", "t"] {
        return Some(format!("keys {:?}: {line}", keys(&value)));
    }
    if value["protocol"] != 1 {
        return Some(format!(
            "protocol {} is not its own 1: {line}",
            value["protocol"]
        ));
    }
    if !value["reason"]
        .as_str()
        .is_some_and(|reason| !reason.trim().is_empty())
    {
        return Some(format!("no reason: {line}"));
    }
    None
}

/// B5: a bad argv (`--protocol 2`, none, an extra argument) or a bad
/// request (files unsorted, repeated, absolute, with `..` or `.`; an
/// unknown or a missing key; another protocol; not JSON; a relative root;
/// `target_dir` or `tmp_dir` the root or under it, spelled plainly, through
/// `..` or through a symlink; a relative `target_dir`): each one `refused`
/// line, exit 2, and the logging shims' log empty: nothing was discovered.
/// The root is left byte-identical. M: the request checked after discovery.
#[test]
fn bad_request_is_refused_before_discovery() {
    let lab = Lab::new("refused", Shims::Forward);
    let files = rust_files(&lab.root);
    let base = Request::new(&lab, &files);
    let link = lab.scratch.path().join("link");
    symlink(&lab.root, &link).expect("a symlink to the root");
    let absolute = format!(
        "{}/arena/src/lib.rs",
        lab.root.to_str().expect("a UTF-8 root")
    );
    let around = lab
        .scratch
        .path()
        .join("ra")
        .join("..")
        .join("root")
        .join("target");
    let before = snapshot(&lab.root);
    let cases: &[(&str, &[&str], String)] = &[
        ("argv `--protocol 2`", &["--protocol", "2"], base.line()),
        ("argv without `--protocol`", &[], base.line()),
        (
            "argv with an extra argument",
            &["--protocol", "1", "--verbose"],
            base.line(),
        ),
        (
            "files unsorted",
            &PROTOCOL_ARGS,
            base.with(
                "files",
                json_strings(&["tools/probe.rs", "arena/src/lib.rs"]),
            )
            .line(),
        ),
        (
            "files repeated",
            &PROTOCOL_ARGS,
            base.with(
                "files",
                json_strings(&["arena/src/lib.rs", "arena/src/lib.rs"]),
            )
            .line(),
        ),
        (
            "an absolute file",
            &PROTOCOL_ARGS,
            base.with("files", json_strings(&[absolute.as_str()]))
                .line(),
        ),
        (
            "a file with `..`",
            &PROTOCOL_ARGS,
            base.with("files", json_strings(&["arena/../arena/src/lib.rs"]))
                .line(),
        ),
        (
            "a file with `.`",
            &PROTOCOL_ARGS,
            base.with("files", json_strings(&["./arena/src/lib.rs"]))
                .line(),
        ),
        (
            "an unknown key",
            &PROTOCOL_ARGS,
            base.with("verbose", "true".to_owned()).line(),
        ),
        (
            "a missing key",
            &PROTOCOL_ARGS,
            base.without("budget_ms").line(),
        ),
        (
            "protocol 2 in the request",
            &PROTOCOL_ARGS,
            base.with("protocol", "2".to_owned()).line(),
        ),
        ("not JSON", &PROTOCOL_ARGS, "protocol 1".to_owned()),
        (
            "a relative root",
            &PROTOCOL_ARGS,
            base.with("root", json_text("root")).line(),
        ),
        (
            "target_dir under the root",
            &PROTOCOL_ARGS,
            base.with("target_dir", json_path(&lab.root.join("target")))
                .line(),
        ),
        (
            "tmp_dir under the root",
            &PROTOCOL_ARGS,
            base.with("tmp_dir", json_path(&lab.root.join("tmp")))
                .line(),
        ),
        (
            "target_dir the root itself",
            &PROTOCOL_ARGS,
            base.with("target_dir", json_path(&lab.root)).line(),
        ),
        (
            "target_dir under the root through `..`",
            &PROTOCOL_ARGS,
            base.with("target_dir", json_path(&around)).line(),
        ),
        (
            "tmp_dir under the root through a symlink",
            &PROTOCOL_ARGS,
            base.with("tmp_dir", json_path(&link.join("tmp"))).line(),
        ),
        (
            "a relative target_dir",
            &PROTOCOL_ARGS,
            base.with("target_dir", json_text("ra/target")).line(),
        ),
    ];
    let mut failures = Vec::new();
    for (case, args, line) in cases {
        let _ = fs::remove_file(&lab.shim_log);
        let reply = run_command(lab.command(args, SHORT_ALARM_S), line, REFUSAL_BOUND, case);
        if let Some(problem) = refusal_problem(&reply) {
            failures.push(format!("{case}: {problem}\n{}", reply.context()));
        }
        let log = lab.shim_log_text();
        if !log.is_empty() {
            failures.push(format!(
                "{case}: a toolchain command ran before the refusal:\n{log}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} problem(s) over {} bad argv or requests:\n\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
    assert_unchanged(&lab.root, &before, "the refused runs");
}

// ---------------------------------------------------------------------------
// B6: the watchdogs.
// ---------------------------------------------------------------------------

/// How a watchdog ends a binary that leads its group: by SIGKILL
/// (`kill(0, SIGKILL)`), or by exit 3. On macOS a group SIGKILL may land
/// after `kill()` returns and the binary's own `exit(3)` wins; the group is
/// gone either way (`docs/canon/layer-c-protocol.md` "Endings", known limit;
/// fixed in 11a-2). The callers check the group gone on their own.
fn assert_ended_by_its_watchdog(status: Option<ExitStatus>) {
    let killed = status.and_then(|status| status.signal()) == Some(libc::SIGKILL);
    let exited_3 = status.and_then(|status| status.code()) == Some(3);
    assert!(
        killed || exited_3,
        "the binary ends its group, itself included, by SIGKILL (or exit 3 when its own exit wins the race): {status:?}"
    );
}

/// B6: under `alarm`, the shims fork a sleeping grandchild and wait, so the
/// binary hangs in discovery; once a grandchild exists, stdin is closed:
/// the binary, its group and every grandchild are gone within 5 s, the
/// binary by SIGKILL (`kill(0, SIGKILL)`, it leads its group) or exit 3
/// ([`assert_ended_by_its_watchdog`]). M: no stdin watchdog.
#[test]
fn binary_kills_its_group_when_stdin_closes() {
    let lab = Lab::new("stdin-eof", Shims::Block);
    let files = rust_files(&lab.root);
    let mut running = Running::start(lab.command(&PROTOCOL_ARGS, SHORT_ALARM_S));
    running.send(&Request::new(&lab, &files).line());
    let first = lab.wait_for_grandchildren(&mut running, FIRST_COMMAND_BOUND);
    running.close_stdin();
    let closed = Instant::now();
    let alive = running.wait_gone(|| lab.grandchildren(), closed + GONE_WITHIN);
    assert!(
        alive.is_empty(),
        "{:?} after stdin closed, still alive: {alive:?} (grandchildren seen first: {first:?}); shim log:\n{}",
        closed.elapsed(),
        lab.shim_log_text()
    );
    assert_ended_by_its_watchdog(running.status);
}

/// B6: as above with stdin held open and `budget_ms` 1000: the binary, its
/// group and every grandchild are gone by the budget plus the 10 s grace
/// plus 5 s of slack after the start, the binary by SIGKILL or exit 3
/// ([`assert_ended_by_its_watchdog`]). M: no budget watchdog.
#[test]
fn binary_kills_its_group_after_budget_and_grace() {
    let lab = Lab::new("budget", Shims::Block);
    let files = rust_files(&lab.root);
    let budget_ms = u64::try_from(SHORT_BUDGET.as_millis()).expect("a budget in ms");
    let request = Request::new(&lab, &files).with("budget_ms", budget_ms.to_string());
    let mut running = Running::start(lab.command(&PROTOCOL_ARGS, SHORT_ALARM_S));
    let deadline = running.started + SHORT_BUDGET + GRACE + SLACK;
    running.send(&request.line());
    // stdin stays open: only the budget can end this run.
    let first = lab.wait_for_grandchildren(&mut running, SHORT_BUDGET + GRACE);
    let alive = running.wait_gone(|| lab.grandchildren(), deadline);
    assert!(
        alive.is_empty(),
        "{:?} after the start (budget {SHORT_BUDGET:?}, grace {GRACE:?}, slack {SLACK:?}), still alive: {alive:?} (grandchildren seen first: {first:?}); shim log:\n{}",
        running.started.elapsed(),
        lab.shim_log_text()
    );
    assert_ended_by_its_watchdog(running.status);
}

// ---------------------------------------------------------------------------
// B7: read-only, no override, no wrapper.
// ---------------------------------------------------------------------------

/// `arena/build.rs` that leaves `sentinel` behind when it runs.
fn sentinel_build_script(sentinel: &Path) -> String {
    let sentinel = json_path(sentinel);
    format!(
        "fn main() {{\n    std::fs::write({sentinel}, \"ran\").expect(\"the sentinel\");\n    let out = std::env::var(\"OUT_DIR\").expect(\"OUT_DIR\");\n    std::fs::write(\n        std::path::Path::new(&out).join(\"tuning.rs\"),\n        \"pub fn regen_rate() -> u32 {{ 3 }}\\n\",\n    )\n    .expect(\"tuning.rs\");\n}}\n"
    )
}

/// B7: a sentinel `build.rs` never runs, no cargo `check` or `build` starts,
/// no `target/` appears under the root and the root stays byte-identical.
/// M: `proc_macros: true`.
#[test]
fn no_build_script_runs() {
    let lab = Lab::new("build-script", Shims::Forward);
    let sentinel = lab.scratch.path().join("build-script-ran");
    fs::write(
        lab.root.join("arena/build.rs"),
        sentinel_build_script(&sentinel),
    )
    .expect("the sentinel build.rs");
    let files = rust_files(&lab.root);
    let before = snapshot(&lab.root);
    let reply = lab.run(&Request::new(&lab, &files));
    completed(&reply);
    assert!(
        !sentinel.exists(),
        "the project's build script ran\n{}",
        reply.context()
    );
    let log = lab.shim_log_text();
    let builds: Vec<&str> = log
        .lines()
        .filter(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("cargo")
                && words
                    .find(|word| !word.starts_with('-') && !word.starts_with('+'))
                    .is_some_and(|command| ["check", "build"].contains(&command))
        })
        .collect();
    assert!(
        builds.is_empty(),
        "the binary started a cargo build: {builds:?}"
    );
    assert_eq!(
        target_dirs(&lab.root),
        Vec::<PathBuf>::new(),
        "a `target/` under the root"
    );
    assert_unchanged(&lab.root, &before, "a run with a sentinel build script");
}

/// B7: the seven overrides, each pointing at a logging shim, reach no
/// toolchain command: the override shims log nothing, every command the
/// binary starts (logged by the regular shims, so the check is not vacuous)
/// starts without one, and neither override target directory appears.
/// M: scrub off.
#[test]
fn toolchain_overrides_are_scrubbed() {
    let lab = Lab::new("scrub", Shims::Forward);
    let overrides = lab.override_values();
    let names: Vec<&str> = overrides.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, OVERRIDES, "the test sets exactly the seven");
    let files = rust_files(&lab.root);
    let mut command = lab.command(&PROTOCOL_ARGS, RUN_ALARM_S);
    for (name, value) in &overrides {
        command.env(name, value);
    }
    let reply = run_command(
        command,
        &Request::new(&lab, &files).line(),
        RUN_BOUND,
        "a run with the seven overrides set",
    );
    completed(&reply);
    let leaked = fs::read_to_string(lab.overrides.join("log")).unwrap_or_default();
    assert!(
        leaked.is_empty(),
        "an override reached a toolchain command:\n{leaked}\n{}",
        reply.context()
    );
    for (name, value) in overrides
        .iter()
        .filter(|(name, _)| name.ends_with("TARGET_DIR"))
    {
        assert!(
            !value.exists(),
            "{name} was used: {} exists",
            value.display()
        );
    }
    assert!(
        !lab.shim_log_text().is_empty(),
        "no toolchain command went through the shims: the check saw nothing\n{}",
        reply.context()
    );
}

/// B7 (L5): a project `.cargo/config.toml` naming a logging shim as
/// `build.rustc-wrapper` and `build.rustc-workspace-wrapper` never gets it
/// run, while cargo itself does run. M: the wrappers not set to `""`.
#[test]
fn project_wrappers_are_emptied() {
    let lab = Lab::new("wrappers", Shims::Forward);
    let log = lab.scratch.path().join("wrapper.log");
    let wrapper = lab.scratch.path().join("project-wrapper");
    write_script(
        &wrapper,
        &format!(
            "printf '%s\\n' \"wrapper $*\" >> {}\nexec \"$@\"\n",
            quoted(&log)
        ),
    );
    let config = lab.root.join(".cargo");
    fs::create_dir_all(&config).expect("the project's .cargo");
    let named = json_path(&wrapper);
    fs::write(
        config.join("config.toml"),
        format!("[build]\nrustc-wrapper = {named}\nrustc-workspace-wrapper = {named}\n"),
    )
    .expect("the project's cargo config");
    let files = rust_files(&lab.root);
    let reply = lab.run(&Request::new(&lab, &files));
    completed(&reply);
    let ran = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        ran.is_empty(),
        "a wrapper the project's cargo config names ran ({}):\n{ran}\n{}",
        log.display(),
        reply.context()
    );
    assert!(
        lab.shim_log_text()
            .lines()
            .any(|line| line.starts_with("cargo ")),
        "no cargo command ran: the check saw nothing\n{}",
        reply.context()
    );
}

// ---------------------------------------------------------------------------
// B8: monikers across an unrelated edit.
// ---------------------------------------------------------------------------

/// B8 (TE-3, binary level): every item's kind, state and moniker are the
/// same before and after unrelated `use` lines (an alias of an imported
/// type beside the impl that names it, a private import of a sibling
/// module's type); the edit did reach the binary (offsets moved).
/// M: a moniker from display text that follows imports (R2).
#[test]
fn monikers_survive_an_unrelated_use_edit() {
    let lab = Lab::new("use-edit", Shims::Forward);
    let files = rust_files(&lab.root);
    let first = lab.run(&Request::new(&lab, &files));
    let before = completed(&first);
    lab.edit(
        "arena/src/systems.rs",
        "use crate::{Health, Tick};\n",
        "use crate::{Health, Tick};\nuse crate::Health as Hitpoints;\n",
    );
    lab.edit(
        "arena/src/lib.rs",
        "use arena_derive::Label;\n",
        "use arena_derive::Label;\nuse crate::systems::Regeneration as Regen;\n",
    );
    let second = lab.run(&Request::new(&lab, &files));
    let after = completed(&second);
    let (was, now) = (monikers(&before), monikers(&after));
    let named = was
        .values()
        .flatten()
        .filter(|(_, state, _)| state == "moniker")
        .count();
    assert!(
        named >= 20,
        "only {named} monikers before the edit: {was:#?}"
    );
    let changed: Vec<&String> = was
        .keys()
        .filter(|path| was.get(*path) != now.get(*path))
        .collect();
    assert!(
        changed.is_empty(),
        "an unrelated `use` edit changed the answers of {changed:?}:\nbefore {was:#?}\nafter {now:#?}"
    );
    assert_ne!(
        starts(before.file("arena/src/systems.rs")),
        starts(after.file("arena/src/systems.rs")),
        "the edit did not reach the binary: systems.rs's offsets did not move"
    );
}

// ---------------------------------------------------------------------------
// The `error` ending (`docs/features/layer-c.md` "Protocol v1"; no named
// criterion) and iteration 2's changes (review of iteration 1).
// ---------------------------------------------------------------------------

/// A root manifest cargo cannot read: `cargo metadata` fails at once.
const BROKEN_MANIFEST: &str = "[workspace\nmembers = [\"arena\"\n";

/// Rounds of [`closing_stdin_after_error_exits_1`], each a reply of well
/// under a second (a metadata failure).
const GUARD_ROUNDS: usize = 20;

/// A reply that ends in one `error` line of `category` and `workspace`, its
/// keys `t`, `workspace` and `category` only: `hello` first, exit 1, no
/// `done`; the reason on stderr, no path of the root on stdout.
fn assert_error_ending(lab: &Lab, reply: &Reply, category: &str, workspace: u64) {
    assert_eq!(reply.status.code(), Some(1), "{}", reply.context());
    let lines = parse_lines(reply);
    assert!(
        lines.first().is_some_and(|line| line["t"] == "hello"),
        "{}",
        reply.context()
    );
    let last = lines.last().expect("a last line");
    assert_eq!(keys(last), ["category", "t", "workspace"], "{last}");
    assert_eq!(last["t"], "error", "{last}");
    assert_eq!(last["category"], category, "{last}");
    assert_eq!(last["workspace"], workspace, "{last}");
    assert!(
        !lines.iter().any(|line| line["t"] == "done"),
        "{}",
        reply.context()
    );
    let root = lab.root.to_str().expect("a UTF-8 root");
    assert!(
        !reply.stdout.contains(root),
        "stdout names a path: {}",
        reply.context()
    );
    assert!(
        !reply.stderr.trim().is_empty(),
        "the reason is on stderr: {}",
        reply.context()
    );
}

/// A workspace whose manifest cargo cannot read ends the reply with one
/// `error` line, category `metadata` (`LoadError`: the manifest or `cargo
/// metadata` failed), workspace 1, exit 1, no `done`.
#[test]
fn broken_manifest_ends_with_one_error_line() {
    let lab = Lab::new("broken-manifest", Shims::Forward);
    fs::write(lab.root.join("Cargo.toml"), BROKEN_MANIFEST).expect("a broken root manifest");
    let files = rust_files(&lab.root);
    let reply = lab.run(&Request::new(&lab, &files));
    assert_error_ending(&lab, &reply, "metadata", 1);
}

/// Iteration 2, change 3: a requested `.rs` file inside a loaded workspace,
/// outside every module tree, that is not UTF-8 (a Latin-1 byte) ends the
/// reply with one `error` line,
/// category `path`, its workspace 1, exit 1, no `done`, as a file under no
/// manifest does. M: the UTF-8 check in the loaded workspace's scan removed.
#[test]
fn non_utf8_file_in_a_loaded_workspace_ends_with_a_path_error() {
    let lab = Lab::new("latin1", Shims::Forward);
    fs::write(
        lab.root.join("arena/src/latin1.rs"),
        b"// caf\xe9: Latin-1, not UTF-8\npub fn latin() {}\n",
    )
    .expect("a Latin-1 source file");
    let files = rust_files(&lab.root);
    assert!(
        files.iter().any(|file| file == "arena/src/latin1.rs"),
        "{files:?}"
    );
    let reply = lab.run(&Request::new(&lab, &files));
    assert_error_ending(&lab, &reply, "path", 1);
}

/// Iteration 3: a requested module file that is not UTF-8 (`mod zlatin;` in
/// `arena/src/lib.rs`, a Latin-1 byte in `arena/src/zlatin.rs`), requested
/// after a sibling of its crate that sorts before it (`arena/src/lib.rs`):
/// every claimed file is read before any is scanned, so the reply ends in
/// one `error` line, category `path`, workspace 1, exit 1, no `done`; not
/// `database`, as when the sibling's scan reached the module's unset text
/// first. M: the pre-read reverted to read-then-scan per file.
#[test]
fn non_utf8_module_after_a_requested_sibling_ends_with_a_path_error() {
    let lab = Lab::new("latin1-module", Shims::Forward);
    lab.edit(
        "arena/src/lib.rs",
        "pub mod systems;\n",
        "pub mod systems;\nmod zlatin;\n",
    );
    fs::write(
        lab.root.join("arena/src/zlatin.rs"),
        b"// caf\xe9: Latin-1, not UTF-8\npub fn zlatin() {}\n",
    )
    .expect("a Latin-1 module file");
    let files = [
        "arena/src/lib.rs".to_owned(),
        "arena/src/zlatin.rs".to_owned(),
    ];
    let reply = lab.run(&Request::new(&lab, &files));
    assert_error_ending(&lab, &reply, "path", 1);
}

/// The lines of a reply up to and with its final line (`done` or `error`),
/// `bound` at most.
fn until_final_line(lines: &mpsc::Receiver<String>, bound: Duration) -> Vec<String> {
    let deadline = Instant::now() + bound;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) => {
                let last =
                    line.starts_with("{\"t\":\"done\"") || line.starts_with("{\"t\":\"error\"");
                seen.push(line);
                if last {
                    return seen;
                }
            }
            Err(error) => {
                panic!("no final line within {bound:?} ({error:?}); the lines so far: {seen:#?}")
            }
        }
    }
}

/// One caller that drops stdin the moment it has its answer: the request
/// sent, the final line read, stdin closed at once. The binary's stdin
/// watchdog then wakes while the binary may still be on its way out. The
/// final line seen and the exit status.
fn close_stdin_after_final_line(lab: &Lab, request: &str, bound: Duration) -> (String, ExitStatus) {
    let (mut running, lines) = Running::start_lines(lab.command(&PROTOCOL_ARGS, RUN_ALARM_S));
    running.send(request);
    let seen = until_final_line(&lines, bound);
    running.close_stdin();
    let status = running.wait_exit(GONE_WITHIN, "after its final line and a closed stdin");
    (seen.last().cloned().unwrap_or_default(), status)
}

/// Iteration 2, change 1 (the guard): once its final line is written, the
/// binary exits with that line's code whatever its stdin does. A caller that
/// reads `error` and drops stdin at once ([`close_stdin_after_final_line`])
/// gets exit 1, never exit 3 or a signal, in every one of [`GUARD_ROUNDS`]
/// rounds (a metadata failure each). Without the guard each round is a race
/// between the stdin watchdog and the binary's own exit: iteration 1's
/// binary lost 17 of 40 rounds, twice (exit 3 or SIGKILL), so 20 rounds
/// miss the mutation with a probability near 0.58^20, about 2e-5: red in
/// practice, not by construction. M: the guard removed.
#[test]
fn closing_stdin_after_error_exits_1() {
    let lab = Lab::new("guard-error", Shims::Forward);
    fs::write(lab.root.join("Cargo.toml"), BROKEN_MANIFEST).expect("a broken root manifest");
    let request = Request::new(&lab, &rust_files(&lab.root)).line();
    let mut wrong = Vec::new();
    for round in 1..=GUARD_ROUNDS {
        let (last, status) = close_stdin_after_final_line(&lab, &request, REFUSAL_BOUND);
        assert!(
            last.starts_with("{\"t\":\"error\""),
            "round {round}: the final line is not `error`: {last}"
        );
        if status.code() != Some(1) {
            wrong.push(format!("round {round}: {status:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {GUARD_ROUNDS} rounds did not exit 1 after their `error` line: {wrong:#?}",
        wrong.len()
    );
}

/// Rounds of [`closing_stdin_after_done_exits_0`], each an empty request
/// (`files: []`: `done` right after `hello`, no discovery, about 0.01 s).
const GUARD_DONE_ROUNDS: usize = 100;

/// Iteration 2, change 1, for `done`: a request naming no file is answered
/// by `hello` and `done` at once, the tightest race between the last line
/// and the binary's exit. A caller that reads `done` and drops stdin at once
/// ([`close_stdin_after_final_line`]) gets exit 0, never exit 3 or a signal,
/// in every one of [`GUARD_DONE_ROUNDS`] rounds. M: the guard disabled.
#[test]
fn closing_stdin_after_done_exits_0() {
    let lab = Lab::new("guard-done", Shims::Forward);
    let request = Request::new(&lab, &[]).line();
    let mut wrong = Vec::new();
    for round in 1..=GUARD_DONE_ROUNDS {
        let (last, status) = close_stdin_after_final_line(&lab, &request, REFUSAL_BOUND);
        assert!(
            last.starts_with("{\"t\":\"done\""),
            "round {round}: the final line is not `done`: {last}"
        );
        if status.code() != Some(0) {
            wrong.push(format!("round {round}: {status:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {GUARD_DONE_ROUNDS} rounds did not exit 0 after their `done` line: {wrong:#?}",
        wrong.len()
    );
    assert!(
        lab.shim_log_text().is_empty(),
        "an empty request started a toolchain command:\n{}",
        lab.shim_log_text()
    );
}
