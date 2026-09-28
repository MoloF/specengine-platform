//! Shared stdio harness for the `specengine-mcp` integration tests: a raw
//! JSON-RPC client over the real binary's piped stdin/stdout.
//!
//! Every stdout line is parsed and checked as a JSON-RPC 2.0 message; a read
//! waits at most [`READ_TIMEOUT`]; the child is killed when the harness drops.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const BIN: &str = env!("CARGO_BIN_EXE_specengine-mcp");

/// Longest wait for one stdout line.
pub const READ_TIMEOUT: Duration = Duration::from_secs(15);

/// Longest wait for the process to exit after stdin is closed.
pub const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

pub const LEGACY_VERSION: &str = "2025-11-25";
pub const STATELESS_VERSION: &str = "2026-07-28";

/// JSON-RPC error codes asserted by the tests.
pub const INVALID_PARAMS: i64 = -32602;
pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;

/// A running `specengine-mcp` child with piped stdio.
pub struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    stderr: Option<JoinHandle<String>>,
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
    pub fn spawn(args: &[&str]) -> Self {
        Self::spawn_in(args, None)
    }

    pub fn spawn_in(args: &[&str], cwd: Option<&Path>) -> Self {
        let mut command = Command::new(BIN);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = cwd {
            command.current_dir(dir);
        }
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
        }
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
        match self.lines.recv_timeout(READ_TIMEOUT) {
            Ok(line) => parse_message(&line),
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
