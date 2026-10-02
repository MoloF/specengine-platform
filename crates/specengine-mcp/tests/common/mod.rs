//! Shared stdio harness for the `specengine-mcp` integration tests: a raw
//! JSON-RPC client over the real binary's piped stdin/stdout.
//!
//! Every spawn runs with a **cleared environment** and a **fresh scratch
//! `HOME`** of its own under `std::env::temp_dir()` (the CLI's
//! `tests/common/mod.rs` pattern; task spec `mcp-read`, R2 and AC-09): the
//! binary needs nothing but `HOME`, so nothing else is passed, and no read
//! can reach the owner's data directory. Without an explicit `cwd` the child
//! runs in an empty scratch directory, never in this repository. The scratch
//! is removed when the [`Server`] drops.
//!
//! Every stdout line is parsed and checked as a JSON-RPC 2.0 message; a read
//! waits at most [`READ_TIMEOUT`]; the child is killed when the harness drops.

#![allow(dead_code)]

pub mod blake3;
pub mod read;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const BIN: &str = env!("CARGO_BIN_EXE_specengine-mcp");

/// Longest wait for one stdout line.
pub const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Longest wait for the process to exit after stdin is closed.
pub const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

pub const LEGACY_VERSION: &str = "2025-11-25";
pub const STATELESS_VERSION: &str = "2026-07-28";

/// JSON-RPC error codes asserted by the tests.
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
pub const RESOURCE_NOT_FOUND: i64 = -32002;
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

pub fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A scratch directory under the system temp dir, canonical, removed on
/// drop (permissions restored first).
pub struct Scratch {
    root: PathBuf,
}

impl Scratch {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-mcp-{name}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self {
            root: fs::canonicalize(&path).expect("canonical scratch directory"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// A new, empty `HOME` of its own at `<scratch>/homes/<name>`.
    pub fn home(&self, name: &str) -> PathBuf {
        let home = self.root.join("homes").join(name);
        fs::create_dir_all(&home).expect("home directory");
        fs::canonicalize(&home).expect("canonical home")
    }

    /// A copy of `fixtures/<name>` at `<scratch>/<dir>`, canonical.
    pub fn copy(&self, name: &str, dir: &str) -> PathBuf {
        let root = self.root.join(dir);
        copy_dir(&fixture(name), &root, false);
        fs::canonicalize(&root).expect("copy exists")
    }

    /// An empty directory `<scratch>/<dir>`, canonical.
    pub fn dir(&self, dir: &str) -> PathBuf {
        let path = self.root.join(dir);
        fs::create_dir_all(&path).expect("directory");
        fs::canonicalize(&path).expect("canonical directory")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        restore_permissions(&self.root);
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn restore_permissions(dir: &Path) {
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            restore_permissions(&path);
        } else {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o644));
        }
    }
}

/// Copies every directory and regular file under `from` to `to`, files in
/// name order, or in reverse name order when `reversed`.
pub fn copy_dir(from: &Path, to: &Path, reversed: bool) {
    fs::create_dir_all(to).expect("copy target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("readable fixture directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    if reversed {
        entries.reverse();
    }
    for entry in entries {
        let kind = entry.file_type().expect("file type");
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_dir(&entry.path(), &target, reversed);
        } else if kind.is_file() {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// Writes `bytes` to `root/relative`, creating its directories.
pub fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("parent directory");
    fs::write(&path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

pub fn read_text(root: &Path, relative: &str) -> String {
    let path = root.join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Replaces the one occurrence of `from` in `root/relative` by `to`.
pub fn replace(root: &Path, relative: &str, from: &str, to: &str) {
    let text = read_text(root, relative);
    assert_eq!(
        text.matches(from).count(),
        1,
        "{relative}: {from:?} must occur exactly once"
    );
    write(root, relative, text.replacen(from, to, 1));
}

/// The directory the host rule puts the index databases in, under `home`.
pub fn data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    }
}

/// Every entry under `dir`, root-relative: directories as `None`, files as
/// their bytes, symlinks as `link -> target`.
pub fn snapshot(dir: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .into_owned();
            let kind = entry.file_type().expect("file type");
            if kind.is_symlink() {
                let target = fs::read_link(&path).expect("link");
                out.insert(
                    relative,
                    Some(format!("link -> {}", target.display()).into_bytes()),
                );
            } else if kind.is_dir() {
                out.insert(relative, None);
                walk(root, &path, out);
            } else {
                out.insert(relative, Some(fs::read(&path).unwrap_or_default()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The `HOME` a spawn gets.
#[derive(Debug, Clone, Copy)]
pub enum Home<'a> {
    /// A new, empty scratch `HOME` (the default).
    Fresh,
    /// This directory.
    At(&'a Path),
    /// No `HOME` at all.
    Unset,
}

/// The command every spawn runs: `program args` in `cwd`, the environment
/// cleared, `HOME` set to `home` (when given) and nothing else.
pub fn isolated_command(
    program: impl AsRef<OsStr>,
    args: &[&str],
    cwd: &Path,
    home: Option<&Path>,
) -> Command {
    let mut command = Command::new(program);
    command.env_clear().current_dir(cwd).args(args);
    if let Some(home) = home {
        command.env("HOME", home);
    }
    command
}

/// A running `specengine-mcp` child with piped stdio.
pub struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    stderr: Option<JoinHandle<String>>,
    home: Option<PathBuf>,
    cwd: PathBuf,
    /// The scratch holding a fresh `HOME` and the default `cwd`; removed
    /// after the child has exited.
    scratch: Option<Scratch>,
}

/// What a finished session left behind.
pub struct Finished {
    pub status: ExitStatus,
    /// Every stdout message not consumed by [`Server::recv`], parsed.
    pub messages: Vec<Value>,
    pub stderr: String,
    /// Time from closing stdin to the process exit.
    pub exit_after: Duration,
}

impl Server {
    /// A fresh scratch `HOME`, an empty scratch working directory.
    pub fn spawn(args: &[&str]) -> Self {
        Self::spawn_with(args, None, Home::Fresh)
    }

    /// A fresh scratch `HOME`, the working directory `cwd` (else an empty
    /// scratch one).
    pub fn spawn_in(args: &[&str], cwd: Option<&Path>) -> Self {
        Self::spawn_with(args, cwd, Home::Fresh)
    }

    pub fn spawn_with(args: &[&str], cwd: Option<&Path>, home: Home) -> Self {
        let needs_scratch = cwd.is_none() || matches!(home, Home::Fresh);
        let scratch = needs_scratch.then(|| Scratch::new("spawn"));
        let home = match home {
            Home::Fresh => Some(scratch.as_ref().expect("scratch").home("h")),
            Home::At(path) => Some(path.to_path_buf()),
            Home::Unset => None,
        };
        let cwd = match cwd {
            Some(dir) => dir.to_path_buf(),
            None => scratch.as_ref().expect("scratch").dir("cwd"),
        };
        let mut command = isolated_command(BIN, args, &cwd, home.as_deref());
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn specengine-mcp");
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("piped stdout");
        let mut stderr_pipe = child.stderr.take().expect("piped stderr");
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut buffer = Vec::new();
                match reader.read_until(b'\n', &mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if buffer.last() == Some(&b'\n') {
                            buffer.pop();
                        }
                        let line = String::from_utf8(buffer)
                            .unwrap_or_else(|error| format!("<non-UTF-8 stdout line: {error}>"));
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let stderr = thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr_pipe.read_to_string(&mut text);
            text
        });
        Self {
            child,
            stdin,
            lines,
            stderr: Some(stderr),
            home,
            cwd,
            scratch,
        }
    }

    /// The child's `HOME`, `None` when unset.
    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    /// The child's working directory.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// The child's process id.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// A legacy session: `initialize` with `capabilities`, then
    /// `notifications/initialized`. Returns the `initialize` result.
    pub fn legacy(args: &[&str], capabilities: Value) -> (Self, Value) {
        let mut server = Self::spawn(args);
        let init = server.initialize(capabilities);
        (server, init)
    }

    pub fn initialize(&mut self, capabilities: Value) -> Value {
        let reply = self.request(
            0,
            "initialize",
            Some(json!({
                "protocolVersion": LEGACY_VERSION,
                "capabilities": capabilities,
                "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}
            })),
        );
        let init = result(&reply).clone();
        self.notify("notifications/initialized", None);
        init
    }

    pub fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin is still open");
        let mut line = serde_json::to_string(message).expect("serialize message");
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .expect("write to specengine-mcp stdin");
        stdin.flush().expect("flush specengine-mcp stdin");
    }

    pub fn notify(&mut self, method: &str, params: Option<Value>) {
        let mut message = json!({"jsonrpc": "2.0", "method": method});
        if let Some(params) = params {
            message["params"] = params;
        }
        self.send(&message);
    }

    /// The next stdout message, checked as JSON-RPC 2.0.
    pub fn recv(&mut self) -> Value {
        parse_message(&self.recv_line())
    }

    /// The next stdout line as written (checked as JSON-RPC 2.0).
    pub fn recv_line(&mut self) -> String {
        match self.lines.recv_timeout(READ_TIMEOUT) {
            Ok(line) => {
                parse_message(&line);
                line
            }
            Err(RecvTimeoutError::Timeout) => {
                panic!("no stdout message from specengine-mcp within {READ_TIMEOUT:?}")
            }
            Err(RecvTimeoutError::Disconnected) => {
                let status = self.child.wait().ok();
                let stderr = self
                    .stderr
                    .take()
                    .and_then(|handle| handle.join().ok())
                    .unwrap_or_default();
                panic!("specengine-mcp closed stdout (exit {status:?}); stderr: {stderr}")
            }
        }
    }

    /// Sends a request and returns the next line as written, which must
    /// answer it.
    pub fn request_line(&mut self, id: u64, method: &str, params: Option<Value>) -> String {
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if let Some(params) = params {
            message["params"] = params;
        }
        self.send(&message);
        let line = self.recv_line();
        let reply = parse_message(&line);
        assert_eq!(
            reply.get("id"),
            Some(&json!(id)),
            "expected the answer to {method} (id {id}), got: {}",
            clip(&line)
        );
        line
    }

    /// Sends a request and returns the next message, which must answer it.
    pub fn request(&mut self, id: u64, method: &str, params: Option<Value>) -> Value {
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        if let Some(params) = params {
            message["params"] = params;
        }
        self.send(&message);
        let reply = self.recv();
        assert_eq!(
            reply.get("id"),
            Some(&json!(id)),
            "expected the answer to {method} (id {id}), got: {}",
            clip(&reply.to_string())
        );
        reply
    }

    /// Closes stdin, waits for the exit, drains stdout and stderr.
    pub fn finish(mut self) -> Finished {
        drop(self.stdin.take());
        let closed = Instant::now();
        let status = loop {
            match self.child.try_wait().expect("poll specengine-mcp") {
                Some(status) => break status,
                None if closed.elapsed() > EXIT_TIMEOUT => {
                    let _ = self.child.kill();
                    panic!("specengine-mcp did not exit within {EXIT_TIMEOUT:?} of stdin closing");
                }
                None => thread::sleep(Duration::from_millis(20)),
            }
        };
        let exit_after = closed.elapsed();
        let mut messages = Vec::new();
        loop {
            match self.lines.recv_timeout(READ_TIMEOUT) {
                Ok(line) => messages.push(parse_message(&line)),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    panic!("stdout still open {READ_TIMEOUT:?} after specengine-mcp exited")
                }
            }
        }
        let stderr = self
            .stderr
            .take()
            .expect("stderr reader")
            .join()
            .expect("stderr reader thread");
        Finished {
            status,
            messages,
            stderr,
            exit_after,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        // `scratch` drops after this body: the child is gone by then.
    }
}

/// Parses one stdout line and checks the JSON-RPC 2.0 envelope.
pub fn parse_message(line: &str) -> Value {
    let message: Value = serde_json::from_str(line)
        .unwrap_or_else(|error| panic!("stdout line is not JSON ({error}): {}", clip(line)));
    assert_eq!(
        message.get("jsonrpc"),
        Some(&json!("2.0")),
        "stdout line is not JSON-RPC 2.0: {}",
        clip(line)
    );
    if message.get("method").is_some() {
        assert!(
            message["method"].is_string(),
            "`method` is not a string: {}",
            clip(line)
        );
    } else {
        assert!(
            message.get("id").is_some(),
            "a response without `id`: {}",
            clip(line)
        );
        let has_result = message.get("result").is_some();
        let has_error = message.get("error").is_some();
        assert!(
            has_result ^ has_error,
            "a response needs exactly one of `result` / `error`: {}",
            clip(line)
        );
        if has_error {
            assert!(
                message["error"]["code"].is_i64() && message["error"]["message"].is_string(),
                "malformed JSON-RPC error: {}",
                clip(line)
            );
        }
    }
    message
}

/// The `result` of a reply; panics on a JSON-RPC error.
pub fn result(reply: &Value) -> &Value {
    assert!(
        reply.get("error").is_none(),
        "unexpected JSON-RPC error: {}",
        clip(&reply.to_string())
    );
    &reply["result"]
}

/// The JSON-RPC error code of a reply; panics on a result.
pub fn error_code(reply: &Value) -> i64 {
    reply["error"]["code"].as_i64().unwrap_or_else(|| {
        panic!(
            "expected a JSON-RPC error, got: {}",
            clip(&reply.to_string())
        )
    })
}

/// The three 2026-07-28 request `_meta` fields.
pub fn stateless_meta(capabilities: Value) -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": STATELESS_VERSION,
        "io.modelcontextprotocol/clientCapabilities": capabilities,
        "io.modelcontextprotocol/clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}
    })
}

/// `params` with `_meta` added.
pub fn with_meta(mut params: Value, meta: &Value) -> Value {
    params["_meta"] = meta.clone();
    params
}

/// Client capabilities declaring form elicitation.
pub fn form_capabilities() -> Value {
    json!({"elicitation": {"form": {}}})
}

/// Tool names of a `tools/list` result, in wire order.
pub fn tool_names(list: &Value) -> Vec<String> {
    list["tools"]
        .as_array()
        .expect("`tools` is an array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name").to_owned())
        .collect()
}

/// One tool of a `tools/list` result by name.
pub fn tool<'a>(list: &'a Value, name: &str) -> &'a Value {
    list["tools"]
        .as_array()
        .expect("`tools` is an array")
        .iter()
        .find(|tool| tool["name"] == name)
        .unwrap_or_else(|| panic!("tool {name} missing from tools/list"))
}

/// A long line cut for an assertion message.
pub fn clip(text: &str) -> String {
    const MAX: usize = 400;
    if text.len() <= MAX {
        return text.to_owned();
    }
    let mut end = MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &text[..end], text.len())
}
