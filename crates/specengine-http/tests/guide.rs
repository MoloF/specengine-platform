//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria", the
//! daemon's and the wire's half: the real `specengine-http`, `spec` and
//! `specengine-mcp` binaries of this build over scratch git copies of
//! `fixtures/spec-a`, each daemon with a `HOME` of its own; the fixture is
//! only read.
//!
//! - GU6 `gu6_*`: on a local daemon, `GET /api/guide`, `/api/guide/<topic>`
//!   for every topic and `/api/guide/<topic>%23<ID>` for every rule are
//!   `spec guide [REF] --json` less its LF byte for byte; `/api/guide/nope`
//!   and an unknown rule are 404s with the CLI's stderr line; another query
//!   is a 400; nothing runs a project call. On a hub (`--public-origin`,
//!   the test proxy), any login reads them and their request lines take no
//!   turn (`wait_ms=- run_ms=-`, no turn line). The key sets are
//!   `daemon_keys.rs`'s (GU6, `fixtures/daemon-keys.json`).
//! - GU5 `gu5_*`, the wire: on a hub over `/mcp/<slug>`, `get_guide` with no
//!   arguments, `{topic}` and `{topic, rule}` is the CLI's text and JSON;
//!   `rule` alone is `isError`; `tools/list` holds 19 tools with
//!   `get_guide`; its slow line has `tool=get_guide` and `wait_ms=-`, and no
//!   turn line names it (M, named: the hub tool taking the turn). On stdio
//!   (`specengine-mcp` spawned with and without a project), the same forms
//!   are the `spec` binary's bytes.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/guide_source/mod.rs"]
mod guide_source;

use std::io::{BufRead, BufReader, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use common::hub::{ORIGIN, Proxied, hub_args, through};
use common::mcp_http::{Stateless, content_text, is_error, result_of, tool_names};
use common::slow::{requests, slow_lines, slow_ms, turns, wait_for};
use common::{Scratch, Server, beyond_daemon, mcp_bin, spec};
use guide_source::{Form, GUIDE_ORDER, forms, source_pages, unknown_topic};
use serde_json::{Value, json};

const SLUG: &str = "lantern-keep";

/// `spec guide [reference] [--json]` in `cwd` under `home`: its run.
fn cli(home: &Path, cwd: &Path, reference: Option<&str>, json: bool) -> common::Run {
    let mut args = vec!["guide"];
    args.extend(reference);
    if json {
        args.push("--json");
    }
    spec(home, cwd, &args)
}

/// A route's path for a REF: the topic as is, `#` percent-encoded.
fn route(reference: Option<&str>) -> String {
    match reference {
        None => "/api/guide".to_owned(),
        Some(reference) => format!("/api/guide/{}", reference.replace('#', "%23")),
    }
}

/// A git copy of spec-a, a `HOME` and a working directory.
struct Served {
    scratch: Scratch,
    root: std::path::PathBuf,
    home: std::path::PathBuf,
    cwd: std::path::PathBuf,
}

impl Served {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = scratch.repo("spec-a", "served", "main");
        let home = scratch.home("h");
        let cwd = scratch.dir("cwd");
        Self {
            scratch,
            root,
            home,
            cwd,
        }
    }
}

// ------------------------------------------------------------------ GU6

/// GU6: a local daemon's two routes are the CLI's JSON less its LF, for the
/// index, every topic and every rule; the routes run no project call (the
/// fresh `HOME` holds the daemon's own files alone).
#[test]
fn gu6_the_routes_are_the_clis_documents() {
    let pages = source_pages();
    let served = Served::new("http-gu6-local");
    let server = Server::serve(&served.home, &served.cwd, &[&served.root]);
    let cli_home = served.scratch.home("cli");
    for form in forms(&pages) {
        let reference = form.reference.as_deref();
        let run = cli(&cli_home, &served.cwd, reference, true);
        run.code(0);
        let path = route(reference);
        let reply = server.get(&path);
        assert_eq!(reply.status, 200, "{path}: {}", reply.text());
        assert!(
            reply.text() == run.document(),
            "{path}: the CLI's JSON less its LF"
        );
        let body: Value = serde_json::from_str(reply.text()).expect("JSON");
        assert_eq!(body, form.json, "{path}: the source's document");
    }
    assert!(
        beyond_daemon(&served.home).is_empty(),
        "a guide read runs no project call: {:?}",
        beyond_daemon(&served.home)
    );
}

/// GU6: an unknown topic and an unknown rule are 404s whose message is the
/// CLI's stderr line; a query on either route is a 400.
#[test]
fn gu6_unknown_and_queried() {
    let served = Served::new("http-gu6-refused");
    let server = Server::serve(&served.home, &served.cwd, &[&served.root]);
    let cli_home = served.scratch.home("cli");
    for reference in [
        "nope",
        "proposals#GD-PROPOSALS-NOPE",
        "nope#GD-PROPOSALS-ONE-DOOR",
    ] {
        let run = cli(&cli_home, &served.cwd, Some(reference), false);
        run.code(1);
        let path = route(Some(reference));
        let reply = server.get(&path);
        assert_eq!(reply.status, 404, "{path}: {}", reply.text());
        assert_eq!(
            reply.error_message(),
            run.stderr.trim_end_matches('\n'),
            "{path}: the CLI's line"
        );
    }
    let nope = server.get("/api/guide/nope");
    assert_eq!(nope.error_message(), unknown_topic("nope"));
    for path in [
        "/api/guide?topic=proposals",
        "/api/guide/proposals?json=true",
        "/api/guide/proposals?x",
    ] {
        let reply = server.get(path);
        assert_eq!(reply.status, 400, "{path}: {}", reply.text());
    }
    assert!(
        beyond_daemon(&served.home).is_empty(),
        "{:?}",
        beyond_daemon(&served.home)
    );
}

/// The request lines of the guide's routes on `server`, once `n` of them
/// are on stderr.
fn guide_lines(server: &Server, n: usize) -> Vec<common::slow::Request> {
    wait_for(
        server,
        Duration::from_secs(60),
        &format!("{n} request lines of /api/guide"),
        |lines| {
            (requests(lines)
                .iter()
                .filter(|line| line.route.starts_with("/api/guide"))
                .count()
                >= n)
                .then_some(())
        },
    );
    std::thread::sleep(Duration::from_millis(500));
    requests(&slow_lines(&server.stderr()))
        .into_iter()
        .filter(|line| line.route.starts_with("/api/guide"))
        .collect()
}

/// GU6 on a hub at `--slow-ms 0`: logins with no role and an owner alike
/// read the index, a page and a rule (the CLI's bodies); each request line
/// takes no turn (`wait_ms=-`, `run_ms=-`) and no turn line is the
/// guide's. M: the route served inside the project's turn.
#[test]
fn gu6_a_hub_serves_any_login_with_no_turn() {
    let pages = source_pages();
    let served = Served::new("http-gu6-hub");
    let hub_home = served.scratch.dir("hub-home");
    let server = Server::start(
        &hub_home,
        &served.cwd,
        &slow_ms(hub_args(&[&served.root], ORIGIN), "0"),
    );
    let cli_home = served.scratch.home("cli");
    let rule = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .and_then(|page| page.rules.first())
        .expect("a rule");
    let reference = format!("proposals#{}", rule.id);
    let reads = [None, Some("proposals"), Some(reference.as_str())];
    let mut asked = 0;
    for login in ["mira", "zed-unknown", "owner"] {
        for reference in reads {
            let path = route(reference);
            let reply = through(&server, "GET", &path, &Proxied::fetch(login), b"");
            assert_eq!(reply.status, 200, "{login} {path}: {}", reply.text());
            let run = cli(&cli_home, &served.cwd, reference, true);
            assert!(reply.text() == run.document(), "{login} {path}: the CLI's");
            asked += 1;
        }
    }
    let lines = guide_lines(&server, asked);
    assert_eq!(lines.len(), asked, "{lines:#?}");
    for line in &lines {
        assert_eq!(line.status, "200", "{}", line.line);
        assert_eq!(line.wait_ms, None, "no turn: {}", line.line);
        assert_eq!(line.run_ms, None, "no turn: {}", line.line);
    }
    let held: Vec<_> = turns(&slow_lines(&server.stderr()))
        .into_iter()
        .filter(|turn| turn.by.starts_with("/api/guide"))
        .collect();
    assert!(held.is_empty(), "M: a turn for a guide read: {held:#?}");
}

// ------------------------------------------------------------------ GU5 hub

/// The CLI's text (`2>&1`) and JSON of a form, run by `spec`.
fn cli_pair(home: &Path, cwd: &Path, form: &Form) -> (String, Value) {
    let text = cli(home, cwd, form.reference.as_deref(), false);
    let json = cli(home, cwd, form.reference.as_deref(), true);
    text.code(0);
    json.code(0);
    (
        format!("{}{}", text.stdout, text.stderr),
        serde_json::from_str(json.document()).expect("JSON"),
    )
}

/// GU5 on a hub over `/mcp/<slug>`: every form is the CLI's text and JSON;
/// `rule` alone is `isError`; `tools/list` holds 19 tools with `get_guide`;
/// each call's slow line names `get_guide` with `wait_ms=-`, and no turn
/// line names the tool. M (named): the hub tool taking the turn.
#[test]
fn gu5_a_hub_over_mcp_is_the_cli_and_takes_no_turn() {
    let pages = source_pages();
    let served = Served::new("http-gu5-hub");
    let hub_home = served.scratch.dir("hub-home");
    let server = Server::start(
        &hub_home,
        &served.cwd,
        &slow_ms(hub_args(&[&served.root], ORIGIN), "0"),
    );
    let cli_home = served.scratch.home("cli");
    let mut agent = Stateless::new(server.port, SLUG, "mira");
    let list = agent.tools();
    let names = tool_names(&list);
    assert_eq!(names.len(), 19, "{names:?}");
    assert!(names.iter().any(|name| name == "get_guide"), "{names:?}");
    let all = forms(&pages);
    let picked: Vec<&Form> = all
        .iter()
        .filter(|form| {
            form.reference.is_none()
                || form.reference.as_deref() == Some("proposals")
                || form
                    .reference
                    .as_deref()
                    .is_some_and(|reference| reference.starts_with("records#"))
        })
        .collect();
    for form in &picked {
        let response = agent.call("get_guide", form.arguments.clone());
        let result = result_of(&response, "get_guide");
        assert!(!is_error(result), "{}: {result}", form.arguments);
        let (text, json) = cli_pair(&cli_home, &served.cwd, form);
        assert!(
            content_text(result) == text,
            "{}: the CLI's text",
            form.arguments
        );
        assert_eq!(result["structuredContent"], json, "{}", form.arguments);
    }
    let alone = agent.call("get_guide", json!({"rule": "GD-PROPOSALS-ONE-DOOR"}));
    assert!(is_error(result_of(&alone, "get_guide")), "{alone}");
    let calls = picked.len() + 1;
    wait_for(
        &server,
        Duration::from_secs(60),
        "the get_guide lines",
        |lines| {
            (requests(lines)
                .iter()
                .filter(|line| line.tool == "get_guide")
                .count()
                >= calls)
                .then_some(())
        },
    );
    std::thread::sleep(Duration::from_millis(500));
    let stderr = server.stderr();
    let lines: Vec<_> = requests(&slow_lines(&stderr))
        .into_iter()
        .filter(|line| line.tool == "get_guide")
        .collect();
    assert_eq!(lines.len(), calls, "{lines:#?}");
    for line in &lines {
        assert_eq!(line.route, "/mcp/{p}", "{}", line.line);
        assert_eq!(line.wait_ms, None, "M: the turn taken: {}", line.line);
    }
    let held: Vec<_> = turns(&slow_lines(&stderr))
        .into_iter()
        .filter(|turn| turn.tool == "get_guide")
        .collect();
    assert!(held.is_empty(), "M: a turn for get_guide: {held:#?}");
}

// ------------------------------------------------------------------ GU5 stdio

/// The stdio server in `cwd` under `home`: `initialize`, then one
/// `tools/call get_guide` per `arguments`; stdin closed, every response
/// read until each is answered. Each call's `result`, in order.
fn stdio_calls(cwd: &Path, home: &Path, arguments: &[Value]) -> Vec<Value> {
    let mut child = Command::new(mcp_bin())
        .env_clear()
        .env("HOME", home)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn specengine-mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut lines = vec![
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                "clientInfo": {"name": "specengine-http-tests", "version": "0.0.0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    ];
    for (n, args) in arguments.iter().enumerate() {
        lines.push(
            json!({"jsonrpc": "2.0", "id": n + 1, "method": "tools/call",
            "params": {"name": "get_guide", "arguments": args}}),
        );
    }
    let stdout = child.stdout.take().expect("stdout");
    let (sender, received) = std::sync::mpsc::channel::<Value>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Ok(value) = serde_json::from_str::<Value>(&line)
                && sender.send(value).is_err()
            {
                break;
            }
        }
    });
    for line in &lines {
        writeln!(stdin, "{line}").expect("write a message");
    }
    stdin.flush().expect("flush");
    // Every call answered (or a minute gone), then stdin closed: the
    // server exits on EOF.
    let mut messages: Vec<Value> = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let answered = |messages: &[Value]| {
        (1..=arguments.len()).all(|id| {
            messages
                .iter()
                .any(|message| message.get("id") == Some(&json!(id)))
        })
    };
    while !answered(&messages) && std::time::Instant::now() < deadline {
        if let Ok(message) = received.recv_timeout(Duration::from_millis(100)) {
            messages.push(message);
        }
    }
    drop(stdin);
    let exit_deadline = std::time::Instant::now() + Duration::from_secs(10);
    while child.try_wait().ok().flatten().is_none() && std::time::Instant::now() < exit_deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = reader.join();
    messages.extend(received.try_iter());
    (1..=arguments.len())
        .map(|id| {
            messages
                .iter()
                .find(|message| message.get("id") == Some(&json!(id)))
                .and_then(|message| message.get("result"))
                .unwrap_or_else(|| panic!("no result for {id}: {messages:?}"))
                .clone()
        })
        .collect()
}

/// GU5 on stdio, against the `spec` binary itself: with no project and
/// inside spec-a, the index, every topic and `proposals`'s rules are the
/// CLI's text and JSON.
#[test]
fn gu5_stdio_is_the_spec_binary() {
    let pages = source_pages();
    let served = Served::new("http-gu5-stdio");
    let empty = served.scratch.dir("empty");
    let cli_home = served.scratch.home("cli");
    let all = forms(&pages);
    let picked: Vec<&Form> = all
        .iter()
        .filter(|form| {
            form.reference.as_deref().is_none_or(|reference| {
                GUIDE_ORDER.contains(&reference) || reference.starts_with("proposals#")
            })
        })
        .collect();
    let arguments: Vec<Value> = picked.iter().map(|form| form.arguments.clone()).collect();
    for (label, cwd) in [("no project", &empty), ("spec-a", &served.root)] {
        let home = served
            .scratch
            .home(&format!("mcp-{}", label.replace(' ', "-")));
        let results = stdio_calls(cwd, &home, &arguments);
        for (form, result) in picked.iter().zip(&results) {
            assert!(!is_error(result), "{label} {}: {result}", form.arguments);
            let (text, json) = cli_pair(&cli_home, &served.cwd, form);
            assert!(
                content_text(result) == text,
                "{label} {}: the spec binary's text",
                form.arguments
            );
            assert_eq!(
                result["structuredContent"], json,
                "{label} {}",
                form.arguments
            );
        }
    }
}
