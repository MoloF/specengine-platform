#![cfg(feature = "probes")]
//! AC-08 of docs/features/phase-0-spikes.md
//! (`crates/specengine-mcp/README.md` "Claude Code client"; 07 §1.1-1.2),
//! scripted: a raw JSON-RPC client drives the real `specengine-mcp` binary
//! (feature `probes`) over stdio in both protocol eras — (a) legacy `initialize` +
//! `tools/list` + `tools/call`, (b) stateless 2026-07-28 requests, (c) the
//! owner's form `review_proposal` in the measurement build, (d) the tool
//! `_meta` and annotations, (e) a 26 k-token output — plus the edge cases
//! of the stdio contract. Named mutation: `Lifecycle::Auto` pointed at
//! `LATEST_WITH_INITIALIZE` turns the (b) tests red.
//!
//! docs/features/review-form.md retires the Phase 0 consent demo of the
//! same name (its sealed `requestState`, `getrandom`): the measurement
//! build lists the default build's twenty-two tools and the two probes,
//! twenty-four; the form's round trips, refusals and cancellation are
//! `mcp_review_form.rs`' (every build), here only that the probes build
//! serves the same tool (its `_meta`, its read before any form).
//!
//! Run: `cargo nextest run -p specengine-mcp --features probes --test mcp_stdio`.
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::*;
use serde_json::{Value, json};

const REVIEW_TOOL_META_KEY: &str = "anthropic/requiresUserInteraction";

/// `tools/list` of the measurement build: the default build's
/// [`common::read::TOOLS`] (twenty-two since docs/features/review-form.md)
/// and the two probes, in wire order.
const PROBES_BUILD_TOOLS: [&str; 24] = [
    "ask_question",
    "check_binding",
    "claim_task",
    "complete_task",
    "find_symbols",
    "get_context_bundle",
    "get_impact",
    "get_node",
    "get_proposal",
    "get_roadmap",
    "get_task",
    "get_tree",
    "probe_output",
    "probe_sleep",
    "propose_change",
    "refs",
    "report_decision",
    "report_discrepancy",
    "report_drift",
    "report_run",
    "review_proposal",
    "search",
    "submit_plan",
    "unmapped",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after 1970")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "specengine-mcp-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }

    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.0)
            .expect("read scratch dir")
            .map(|entry| {
                entry
                    .expect("scratch entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn call_params(name: &str, arguments: Value) -> Value {
    json!({"name": name, "arguments": arguments})
}

fn review_args(proposal_id: &str) -> Value {
    json!({"proposal_id": proposal_id})
}

/// (d): `review_proposal` carries the owner's `_meta` (the size cap and
/// `requiresUserInteraction`), the writers' hints and `get_proposal`'s
/// output schema; both probes are read-only and ask nobody.
fn assert_tool_surface(list: &Value, era: &str) {
    let review = tool(list, "review_proposal");
    assert_eq!(
        review["_meta"],
        json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS,
            REVIEW_TOOL_META_KEY: true}),
        "{era}: review_proposal._meta"
    );
    assert_eq!(
        review["outputSchema"],
        tool(list, "get_proposal")["outputSchema"],
        "{era}: review_proposal's output schema is get_proposal's"
    );
    assert_eq!(
        review["annotations"],
        json!({"readOnlyHint": false, "destructiveHint": false,
            "idempotentHint": false, "openWorldHint": false}),
        "{era}: review_proposal.annotations"
    );
    for probe in ["probe_output", "probe_sleep"] {
        assert_eq!(
            tool(list, probe)["annotations"]["readOnlyHint"],
            json!(true),
            "{era}: {probe}.annotations"
        );
        assert!(
            tool(list, probe)["_meta"]
                .get(REVIEW_TOOL_META_KEY)
                .is_none(),
            "{era}: {probe} asks the human"
        );
    }
}

fn text_of(result: &Value) -> &str {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content: {}", clip(&result.to_string())))
}

// ---------------------------------------------------------------- (a) legacy

#[test]
fn a_legacy_initialize_list_and_call() {
    let (mut server, init) = Server::legacy(&[], form_capabilities());
    assert_eq!(init["protocolVersion"], json!(LEGACY_VERSION), "{init}");
    assert_eq!(
        init["serverInfo"]["name"],
        json!("specengine-mcp"),
        "{init}"
    );
    let instructions = init["instructions"].as_str().expect("instructions present");
    assert!(
        instructions.chars().count() <= 2048,
        "instructions are {} characters, the client keeps 2048",
        instructions.chars().count()
    );

    let list = server.request(1, "tools/list", None);
    assert_eq!(
        tool_names(result(&list)),
        PROBES_BUILD_TOOLS,
        "legacy tools/list"
    );
    // docs/features/review-form.md D1: `probes` adds only the two probes.
    let without: Vec<&str> = PROBES_BUILD_TOOLS
        .iter()
        .copied()
        .filter(|name| !name.starts_with("probe_"))
        .collect();
    assert_eq!(without, common::read::TOOLS, "the default build's tools");

    let call = server.request(
        2,
        "tools/call",
        Some(call_params("probe_output", json!({"tokens": 3}))),
    );
    let call = result(&call);
    assert_eq!(call["isError"], json!(false), "{call}");
    assert_eq!(text_of(call), "tok tok tok\n");

    let done = server.finish();
    assert!(
        done.status.success(),
        "exit {:?}; stderr: {}",
        done.status,
        done.stderr
    );
    assert!(
        done.messages.is_empty(),
        "unsolicited output: {:?}",
        done.messages
    );
    assert_eq!(done.stderr, "", "stderr of a clean session");
}

// ------------------------------------------------------------- (b) stateless

#[test]
fn b_stateless_discover_and_list() {
    let mut server = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());

    let discover = server.request(1, "server/discover", Some(json!({"_meta": meta})));
    let discover = result(&discover);
    assert_eq!(discover["resultType"], json!("complete"), "{discover}");
    let versions = discover["supportedVersions"]
        .as_array()
        .unwrap_or_else(|| panic!("no supportedVersions: {discover}"));
    assert!(
        versions.contains(&json!(STATELESS_VERSION)),
        "supportedVersions lacks {STATELESS_VERSION}: {versions:?}"
    );

    let list = server.request(2, "tools/list", Some(json!({"_meta": meta})));
    let list = result(&list);
    assert_eq!(
        list["resultType"],
        json!("complete"),
        "{}",
        clip(&list.to_string())
    );
    assert_eq!(list["ttlMs"], json!(0), "{}", clip(&list.to_string()));
    assert_eq!(
        list["cacheScope"],
        json!("public"),
        "{}",
        clip(&list.to_string())
    );
    assert_eq!(tool_names(list), PROBES_BUILD_TOOLS, "stateless tools/list");

    let bare = server.request(3, "tools/list", None);
    assert_eq!(
        error_code(&bare),
        INVALID_PARAMS,
        "tools/list without _meta: {bare}"
    );

    let done = server.finish();
    assert!(
        done.status.success(),
        "exit {:?}; stderr: {}",
        done.status,
        done.stderr
    );
    assert!(
        done.messages.is_empty(),
        "unsolicited output: {:?}",
        done.messages
    );
}

#[test]
fn b_legacy_lifecycle_refuses_stateless_requests() {
    // Discrimination of (b): the same messages against `--lifecycle legacy`.
    let mut server = Server::spawn(&["--lifecycle", "legacy"]);
    let meta = stateless_meta(form_capabilities());
    for (id, method) in [(1, "server/discover"), (2, "tools/list")] {
        let reply = server.request(id, method, Some(json!({"_meta": meta})));
        assert_eq!(
            error_code(&reply),
            UNSUPPORTED_PROTOCOL_VERSION,
            "{method} under --lifecycle legacy: {reply}"
        );
        let supported = reply["error"]["data"]["supported"]
            .as_array()
            .unwrap_or_else(|| panic!("error.data.supported missing: {reply}"));
        assert!(
            !supported.contains(&json!(STATELESS_VERSION)),
            "legacy lifecycle still advertises {STATELESS_VERSION}: {reply}"
        );
        assert!(
            supported.contains(&json!(LEGACY_VERSION)),
            "legacy lifecycle must keep {LEGACY_VERSION}: {reply}"
        );
    }
    drop(server);

    // The legacy handshake still works under the same flag.
    let (mut server, init) = Server::legacy(&["--lifecycle", "legacy"], form_capabilities());
    assert_eq!(init["protocolVersion"], json!(LEGACY_VERSION), "{init}");
    let list = server.request(1, "tools/list", None);
    assert_eq!(tool_names(result(&list)), PROBES_BUILD_TOOLS);
}

// ------------------------------------------------------- (c) the owner's form

/// (c): the measurement build serves the owner's form as every build
/// does: outside a project its read before any form refuses with the
/// CLI's `spec init` line — no `elicitation/create`, no `input_required`,
/// no `requestState` — in both eras, a form-capable client.
#[test]
fn c_review_proposal_reads_before_any_form_in_both_eras() {
    let (mut legacy, _) = Server::legacy(&[], form_capabilities());
    let reply = legacy.request(
        1,
        "tools/call",
        Some(call_params("review_proposal", review_args("PR-0001"))),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(text_of(answer).contains("spec init"), "{answer}");
    let done = legacy.finish();
    assert!(done.status.success(), "{}", done.stderr);
    assert!(
        done.messages.is_empty(),
        "no form went out: {:?}",
        done.messages
    );

    let mut stateless = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());
    let reply = stateless.request(
        1,
        "tools/call",
        Some(with_meta(
            call_params("review_proposal", review_args("PR-0001")),
            &meta,
        )),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert_ne!(answer["resultType"], json!("input_required"), "{answer}");
    assert!(answer.get("requestState").is_none(), "{answer}");
    assert!(text_of(answer).contains("spec init"), "{answer}");
}

// ------------------------------------------------ (d) tool _meta, annotations

#[test]
fn d_tool_surface_in_both_eras() {
    let (mut legacy, _) = Server::legacy(&[], form_capabilities());
    let list = legacy.request(1, "tools/list", None);
    assert_tool_surface(result(&list), "legacy");
    drop(legacy);

    let mut stateless = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());
    let list = stateless.request(1, "tools/list", Some(json!({"_meta": meta})));
    assert_tool_surface(result(&list), "stateless");
}

// ------------------------------------------------------ (e) large output

#[test]
fn e_output_of_26k_tokens_without_server_error() {
    let (mut legacy, _) = Server::legacy(&[], form_capabilities());
    let reply = legacy.request(
        1,
        "tools/call",
        Some(call_params("probe_output", json!({"tokens": 26_000}))),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(false));
    assert_eq!(text_of(answer).len(), 104_000, "26 000 tokens x 4 bytes");

    let reply = legacy.request(
        2,
        "tools/call",
        Some(call_params("probe_output", json!({"tokens": 200_001}))),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(text_of(answer).contains("200000"), "{answer}");
    drop(legacy);

    let mut stateless = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());
    let reply = stateless.request(
        1,
        "tools/call",
        Some(with_meta(
            call_params("probe_output", json!({"tokens": 26_000})),
            &meta,
        )),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(false));
    assert_eq!(
        text_of(answer).len(),
        104_000,
        "stateless: 26 000 tokens x 4 bytes"
    );
}

// ------------------------------------------------------------------ extras

#[test]
fn cancelled_probe_sleep_gets_no_response() {
    let (mut server, _) = Server::legacy(&[], form_capabilities());
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": call_params("probe_sleep", json!({"seconds": 30}))
    }));
    // The server reads in order: once `ping` is answered, the call is in flight.
    let ping = server.request(8, "ping", None);
    result(&ping);
    server.notify(
        "notifications/cancelled",
        Some(json!({"requestId": 7, "reason": "test"})),
    );
    let ping = server.request(9, "ping", None);
    result(&ping);

    // On stdin EOF rmcp 3.5.0 drains in-flight handlers for up to 5 s and then
    // drops them unanswered, so "no response" alone does not show the
    // cancellation: a still-sleeping handler holds the exit for those 5 s, a
    // cancelled one does not.
    let done = server.finish();
    assert!(
        done.status.success(),
        "exit {:?}; stderr: {}",
        done.status,
        done.stderr
    );
    assert!(
        done.messages
            .iter()
            .all(|message| message["id"] != json!(7)),
        "the cancelled call was answered: {:?}",
        done.messages
    );
    assert!(
        done.exit_after < Duration::from_secs(4),
        "exit took {:?} after stdin closed: the cancelled probe_sleep kept running",
        done.exit_after
    );
}

#[test]
fn cancelled_short_probe_sleep_stays_unanswered_after_it_would_have_ended() {
    // The session outlives the 1 s sleep: an uncancelled call would be answered
    // before the last `ping`.
    let (mut server, _) = Server::legacy(&[], form_capabilities());
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": call_params("probe_sleep", json!({"seconds": 1}))
    }));
    result(&server.request(8, "ping", None));
    server.notify(
        "notifications/cancelled",
        Some(json!({"requestId": 7, "reason": "test"})),
    );
    std::thread::sleep(Duration::from_millis(2_500));
    result(&server.request(9, "ping", None));
    let done = server.finish();
    assert!(
        done.status.success(),
        "exit {:?}; stderr: {}",
        done.status,
        done.stderr
    );
    assert!(
        done.messages
            .iter()
            .all(|message| message["id"] != json!(7)),
        "the cancelled call was answered: {:?}",
        done.messages
    );
}

#[test]
fn empty_stdin_exits_zero_silently() {
    let done = Server::spawn(&[]).finish();
    assert_eq!(done.status.code(), Some(0), "stderr: {}", done.stderr);
    assert!(done.messages.is_empty(), "{:?}", done.messages);
    assert_eq!(done.stderr, "");
}

#[test]
fn notification_first_exits_one_with_one_stderr_line() {
    let mut server = Server::spawn(&[]);
    server.notify("notifications/initialized", None);
    let done = server.finish();
    assert_eq!(done.status.code(), Some(1), "stderr: {}", done.stderr);
    let lines: Vec<&str> = done.stderr.lines().collect();
    assert_eq!(lines.len(), 1, "stderr: {:?}", done.stderr);
    assert!(
        lines[0].starts_with("specengine-mcp: "),
        "stderr: {:?}",
        done.stderr
    );
    assert!(done.messages.is_empty(), "{:?}", done.messages);
}

#[test]
fn full_sessions_leave_the_working_directory_empty() {
    // "One door": no MCP tool writes files.
    let scratch = Scratch::new("door");

    let mut legacy = Server::spawn_in(&[], Some(&scratch.0));
    legacy.initialize(form_capabilities());
    result(&legacy.request(1, "tools/list", None));
    result(&legacy.request(
        2,
        "tools/call",
        Some(call_params("probe_output", json!({"tokens": 100}))),
    ));
    result(&legacy.request(
        3,
        "tools/call",
        Some(call_params("probe_sleep", json!({"seconds": 0}))),
    ));
    result(&legacy.request(
        4,
        "tools/call",
        Some(call_params("review_proposal", review_args("PR-0001"))),
    ));
    let done = legacy.finish();
    assert!(
        done.status.success(),
        "legacy exit {:?}: {}",
        done.status,
        done.stderr
    );

    let mut stateless = Server::spawn_in(&[], Some(&scratch.0));
    let meta = stateless_meta(form_capabilities());
    result(&stateless.request(1, "server/discover", Some(json!({"_meta": meta}))));
    let reply = stateless.request(
        2,
        "tools/call",
        Some(with_meta(
            call_params("review_proposal", review_args("PR-0001")),
            &meta,
        )),
    );
    assert_eq!(result(&reply)["resultType"], json!("complete"));
    let done = stateless.finish();
    assert!(
        done.status.success(),
        "stateless exit {:?}: {}",
        done.status,
        done.stderr
    );

    assert_eq!(
        scratch.entries(),
        Vec::<String>::new(),
        "files appeared in the working directory"
    );
}

#[test]
fn mcp_json_fixture_is_the_specified_config() {
    // The owner's AC-16 launcher (docs/features/mcp-read.md): the Phase 0
    // command, plus a scratch `HOME` so no read reaches the owner's data
    // directory, and the real toolchain homes so `cargo` still finds its
    // toolchain. Claude Code expands `${VAR}` and `${VAR:-default}` from its
    // own environment: `HOME` is the per-user temp dir (`$TMPDIR` on macOS,
    // `/tmp` elsewhere), the toolchain homes the defaults under the real
    // `HOME`. Unexpanded, the `HOME` is relative and every read refuses it
    // (exit 2), never a write elsewhere.
    let path = repository_root()
        .join("fixtures")
        .join("mcp")
        .join("mcp.json");
    let text = fs::read_to_string(&path).expect("fixtures/mcp/mcp.json exists");
    let config: Value = serde_json::from_str(&text).expect("fixtures/mcp/mcp.json is JSON");
    assert_eq!(
        config,
        json!({"mcpServers": {"specengine": {
            "type": "stdio",
            "command": "cargo",
            "args": ["run", "-q", "-p", "specengine-mcp", "--features", "probes"],
            "env": {
                "HOME": "${TMPDIR:-/tmp}/specengine-mcp-home",
                "CARGO_HOME": "${HOME}/.cargo",
                "RUSTUP_HOME": "${HOME}/.rustup"
            }
        }}})
    );
    let home = config["mcpServers"]["specengine"]["env"]["HOME"]
        .as_str()
        .expect("HOME");
    assert!(
        !home.contains("${HOME}") && !home.contains("Application Support"),
        "the launcher's HOME must not be the owner's: {home}"
    );
}
