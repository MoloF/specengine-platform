#![cfg(feature = "probes")]
//! AC-08 of docs/features/phase-0-spikes.md (04 §4, 07 §1.1-1.2), scripted:
//! a raw JSON-RPC client drives the real `specengine-mcp` binary (feature
//! `probes`) over stdio in both protocol eras — (a) legacy `initialize` +
//! `tools/list` + `tools/call`, (b) stateless 2026-07-28 requests, (c) the
//! `review_proposal` elicitation round trips, (d) the tool `_meta` and
//! annotations, (e) a 26 k-token output — plus the edge cases of the stdio
//! contract. Named mutation: `Lifecycle::Auto` pointed at
//! `LATEST_WITH_INITIALIZE` turns the (b) and stateless (c) tests red.
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

/// `tools/list` of the measurement build: the four read tools (task spec
/// `mcp-read`), the consent demo and the two probes, in wire order.
const PROBES_BUILD_TOOLS: [&str; 7] = [
    "get_context_bundle",
    "get_node",
    "get_tree",
    "probe_output",
    "probe_sleep",
    "review_proposal",
    "search",
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

/// (d): `review_proposal` carries the consent `_meta`, an `outputSchema` and
/// `readOnlyHint: false`; both probes are read-only.
fn assert_tool_surface(list: &Value, era: &str) {
    let review = tool(list, "review_proposal");
    assert_eq!(
        review["_meta"],
        json!({REVIEW_TOOL_META_KEY: true}),
        "{era}: review_proposal._meta"
    );
    let schema = &review["outputSchema"];
    assert_eq!(
        schema["type"],
        json!("object"),
        "{era}: outputSchema: {schema}"
    );
    for field in [
        "proposal_id",
        "action",
        "decision",
        "comment",
        "era",
        "protocol_version",
    ] {
        assert!(
            schema["properties"].get(field).is_some(),
            "{era}: outputSchema lacks {field}: {schema}"
        );
    }
    assert_eq!(
        review["annotations"]["readOnlyHint"],
        json!(false),
        "{era}: review_proposal.annotations"
    );
    for probe in ["probe_output", "probe_sleep"] {
        assert_eq!(
            tool(list, probe)["annotations"]["readOnlyHint"],
            json!(true),
            "{era}: {probe}.annotations"
        );
    }
}

/// The consent form: `decision` (required enum approve/reject), `comment` (string).
fn assert_review_form(params: &Value) {
    let schema = &params["requestedSchema"];
    assert_eq!(schema["type"], json!("object"), "form schema: {params}");
    assert_eq!(
        schema["properties"]["decision"]["enum"],
        json!(["approve", "reject"]),
        "form schema: {params}"
    );
    assert_eq!(
        schema["properties"]["comment"]["type"],
        json!("string"),
        "form schema: {params}"
    );
    assert_eq!(
        schema["required"],
        json!(["decision"]),
        "form schema: {params}"
    );
    assert!(
        params["message"].as_str().is_some_and(|m| !m.is_empty()),
        "form message: {params}"
    );
}

fn outcome(
    proposal_id: &str,
    action: &str,
    decision: Value,
    comment: Value,
    era: &str,
    version: &str,
) -> Value {
    json!({
        "proposal_id": proposal_id,
        "action": action,
        "decision": decision,
        "comment": comment,
        "era": era,
        "protocol_version": version
    })
}

/// Legacy: calls `review_proposal`, answers the server's `elicitation/create`
/// with `answer`, returns the tool result.
fn legacy_review(server: &mut Server, id: u64, proposal_id: &str, answer: Value) -> Value {
    server.send(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": call_params("review_proposal", review_args(proposal_id))
    }));
    let ask = server.recv();
    assert_eq!(
        ask["method"],
        json!("elicitation/create"),
        "expected the server's form request, got: {}",
        clip(&ask.to_string())
    );
    assert!(
        ask.get("id").is_some(),
        "elicitation/create must be a request: {ask}"
    );
    assert_review_form(&ask["params"]);
    assert!(
        ask["params"]["message"]
            .as_str()
            .is_some_and(|m| m.contains(proposal_id)),
        "the form names the proposal: {ask}"
    );
    server.send(&json!({"jsonrpc": "2.0", "id": ask["id"], "result": answer}));
    let reply = server.recv();
    assert_eq!(reply["id"], json!(id), "tool result id: {reply}");
    result(&reply).clone()
}

/// Stateless round 1 of `review_proposal`; returns the result.
fn stateless_ask(server: &mut Server, id: u64, meta: &Value, proposal_id: &str) -> Value {
    let reply = server.request(
        id,
        "tools/call",
        Some(with_meta(
            call_params("review_proposal", review_args(proposal_id)),
            meta,
        )),
    );
    result(&reply).clone()
}

/// Stateless round 1 that must ask; returns the sealed `requestState`.
fn stateless_request_state(
    server: &mut Server,
    id: u64,
    meta: &Value,
    proposal_id: &str,
) -> String {
    let first = stateless_ask(server, id, meta, proposal_id);
    assert_eq!(
        first["resultType"],
        json!("input_required"),
        "round 1: {first}"
    );
    first["requestState"]
        .as_str()
        .unwrap_or_else(|| panic!("round 1 without requestState: {first}"))
        .to_owned()
}

/// Stateless round 2 of `review_proposal`; returns the whole reply.
fn stateless_answer(
    server: &mut Server,
    id: u64,
    meta: &Value,
    proposal_id: &str,
    request_state: Option<&str>,
    input_responses: Option<Value>,
) -> Value {
    let mut params = with_meta(
        call_params("review_proposal", review_args(proposal_id)),
        meta,
    );
    if let Some(state) = request_state {
        params["requestState"] = json!(state);
    }
    if let Some(responses) = input_responses {
        params["inputResponses"] = responses;
    }
    server.request(id, "tools/call", Some(params))
}

fn text_of(result: &Value) -> &str {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text content: {}", clip(&result.to_string())))
}

/// Replaces the character at `index` of `text` with a different base64url one.
fn flip_char(text: &str, index: usize) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    chars[index] = if chars[index] == 'A' { 'B' } else { 'A' };
    chars.into_iter().collect()
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

// ------------------------------------------------------- (c) elicitation

#[test]
fn c_legacy_elicitation_accept_round_trip() {
    let (mut server, _) = Server::legacy(&[], form_capabilities());
    let answer = legacy_review(
        &mut server,
        5,
        "P-0001",
        json!({"action": "accept", "content": {"decision": "approve", "comment": "ok"}}),
    );
    assert_eq!(answer["isError"], json!(false), "{answer}");
    assert_eq!(
        answer["structuredContent"],
        outcome(
            "P-0001",
            "accept",
            json!("approve"),
            json!("ok"),
            "legacy",
            LEGACY_VERSION
        )
    );
    assert!(text_of(&answer).contains("P-0001"), "{answer}");

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
fn c_legacy_elicitation_decline_and_cancel() {
    let (mut server, _) = Server::legacy(&[], form_capabilities());
    for (id, action) in [(6, "decline"), (7, "cancel")] {
        let answer = legacy_review(&mut server, id, "P-0002", json!({"action": action}));
        assert_eq!(answer["isError"], json!(false), "{action}: {answer}");
        assert_eq!(
            answer["structuredContent"],
            outcome(
                "P-0002",
                action,
                Value::Null,
                Value::Null,
                "legacy",
                LEGACY_VERSION
            ),
            "{action}"
        );
    }
}

#[test]
fn c_legacy_without_form_elicitation_is_a_tool_error() {
    let (mut server, _) = Server::legacy(&[], json!({}));
    let reply = server.request(
        1,
        "tools/call",
        Some(call_params("review_proposal", review_args("P-0001"))),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(
        answer.get("structuredContent").is_none_or(Value::is_null),
        "no outcome without a form: {answer}"
    );
}

#[test]
fn c_stateless_elicitation_round_trip() {
    let mut server = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());

    let first = stateless_ask(&mut server, 1, &meta, "P-0001");
    assert_eq!(first["resultType"], json!("input_required"), "{first}");
    let ask = &first["inputRequests"]["owner_review"];
    assert_eq!(ask["method"], json!("elicitation/create"), "{first}");
    assert_review_form(&ask["params"]);
    let state = first["requestState"].as_str().expect("requestState");
    assert!(state.starts_with("rs1."), "requestState: {state}");

    let reply = stateless_answer(
        &mut server,
        2,
        &meta,
        "P-0001",
        Some(state),
        Some(json!({"owner_review": {"action": "accept", "content": {"decision": "reject"}}})),
    );
    let second = result(&reply);
    assert_eq!(second["resultType"], json!("complete"), "{second}");
    assert_eq!(second["isError"], json!(false), "{second}");
    assert_eq!(
        second["structuredContent"],
        outcome(
            "P-0001",
            "accept",
            json!("reject"),
            Value::Null,
            "stateless",
            STATELESS_VERSION
        )
    );

    // Decline through the same state: decision and comment null.
    let reply = stateless_answer(
        &mut server,
        3,
        &meta,
        "P-0001",
        Some(state),
        Some(json!({"owner_review": {"action": "decline"}})),
    );
    assert_eq!(
        result(&reply)["structuredContent"],
        outcome(
            "P-0001",
            "decline",
            Value::Null,
            Value::Null,
            "stateless",
            STATELESS_VERSION
        )
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
fn c_stateless_request_state_is_deterministic_per_process() {
    let meta = stateless_meta(form_capabilities());
    let mut server = Server::spawn(&[]);
    let one = stateless_request_state(&mut server, 1, &meta, "P-0001");
    let two = stateless_request_state(&mut server, 2, &meta, "P-0001");
    assert_eq!(
        one, two,
        "same proposal, same process, different requestState"
    );
    let other = stateless_request_state(&mut server, 3, &meta, "P-0002");
    assert_ne!(one, other, "two proposals share a requestState");
    drop(server);

    // The key is per process: a restart invalidates the old state.
    let mut restarted = Server::spawn(&[]);
    let fresh = stateless_request_state(&mut restarted, 1, &meta, "P-0001");
    assert_ne!(one, fresh, "the requestState key survived a restart");
    let reply = stateless_answer(
        &mut restarted,
        2,
        &meta,
        "P-0001",
        Some(&one),
        Some(json!({"owner_review": {"action": "accept", "content": {"decision": "approve"}}})),
    );
    assert_eq!(
        error_code(&reply),
        INVALID_PARAMS,
        "state of another process: {reply}"
    );
}

#[test]
fn c_stateless_tampered_request_state_is_invalid_params() {
    let meta = stateless_meta(form_capabilities());
    let mut server = Server::spawn(&[]);
    let state = stateless_request_state(&mut server, 1, &meta, "P-0001");
    let parts: Vec<&str> = state.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "requestState is rs1.<payload>.<mac>: {state}"
    );
    let payload_start = parts[0].len() + 1;
    let mac_start = payload_start + parts[1].len() + 1;
    // Middle characters: all six bits are significant, so the bytes change.
    let tampered = [
        flip_char(&state, payload_start + parts[1].len() / 2),
        flip_char(&state, mac_start + parts[2].len() / 2),
    ];
    let answer = json!({"owner_review": {"action": "accept", "content": {"decision": "approve"}}});
    for (index, bad) in tampered.iter().enumerate() {
        assert_ne!(bad, &state);
        let reply = stateless_answer(
            &mut server,
            10 + index as u64,
            &meta,
            "P-0001",
            Some(bad),
            Some(answer.clone()),
        );
        assert_eq!(
            error_code(&reply),
            INVALID_PARAMS,
            "tampered state {bad}: {reply}"
        );
    }
    // The untouched state still opens.
    let reply = stateless_answer(&mut server, 20, &meta, "P-0001", Some(&state), Some(answer));
    assert_eq!(result(&reply)["resultType"], json!("complete"), "{reply}");
}

#[test]
fn c_stateless_request_state_is_bound_to_its_proposal() {
    let meta = stateless_meta(form_capabilities());
    let mut server = Server::spawn(&[]);
    let state = stateless_request_state(&mut server, 1, &meta, "P-0001");
    let reply = stateless_answer(
        &mut server,
        2,
        &meta,
        "P-0002",
        Some(&state),
        Some(json!({"owner_review": {"action": "accept", "content": {"decision": "approve"}}})),
    );
    assert_eq!(
        error_code(&reply),
        INVALID_PARAMS,
        "state reused for another proposal: {reply}"
    );
}

#[test]
fn c_stateless_missing_input_responses_is_invalid_params() {
    let meta = stateless_meta(form_capabilities());
    let mut server = Server::spawn(&[]);
    let state = stateless_request_state(&mut server, 1, &meta, "P-0001");
    let reply = stateless_answer(&mut server, 2, &meta, "P-0001", Some(&state), None);
    assert_eq!(
        error_code(&reply),
        INVALID_PARAMS,
        "no inputResponses: {reply}"
    );
    let reply = stateless_answer(
        &mut server,
        3,
        &meta,
        "P-0001",
        Some(&state),
        Some(json!({"another_key": {"action": "accept", "content": {"decision": "approve"}}})),
    );
    assert_eq!(
        error_code(&reply),
        INVALID_PARAMS,
        "no owner_review answer: {reply}"
    );
}

#[test]
fn c_stateless_without_form_elicitation_is_a_tool_error() {
    let meta = stateless_meta(json!({}));
    let mut server = Server::spawn(&[]);
    let answer = stateless_ask(&mut server, 1, &meta, "P-0001");
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert_ne!(answer["resultType"], json!("input_required"), "{answer}");
    assert!(answer.get("requestState").is_none(), "{answer}");
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
fn homoglyph_proposal_id_is_a_tool_error_naming_the_code_point() {
    // CYRILLIC CAPITAL LETTER ER in place of the Latin P.
    let homoglyph = "\u{0420}-0001";
    let (mut legacy, _) = Server::legacy(&[], form_capabilities());
    let reply = legacy.request(
        1,
        "tools/call",
        Some(call_params("review_proposal", review_args(homoglyph))),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(text_of(answer).contains("U+0420"), "{answer}");
    drop(legacy);

    let mut stateless = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());
    let answer = stateless_ask(&mut stateless, 1, &meta, homoglyph);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(text_of(&answer).contains("U+0420"), "{answer}");
    assert!(
        answer.get("requestState").is_none(),
        "no form for a bad ID: {answer}"
    );
}

#[test]
fn malformed_proposal_ids_are_tool_errors_before_any_form() {
    let longest = format!("P{}", "0".repeat(63));
    let too_long = format!("P{}", "0".repeat(64));
    let (mut legacy, _) = Server::legacy(&[], form_capabilities());
    let mut id = 1;
    for (bad, needle) in [
        ("", "empty"),
        ("1-P", "must start with an ASCII letter"),
        ("P 1", "U+0020"),
        (too_long.as_str(), "at most 64"),
    ] {
        // A form request here would be answered by `recv` as the tool result
        // and fail the id check: an invalid ID must never reach the owner.
        let reply = legacy.request(
            id,
            "tools/call",
            Some(call_params("review_proposal", review_args(bad))),
        );
        let answer = result(&reply);
        assert_eq!(answer["isError"], json!(true), "{bad:?}: {answer}");
        assert!(text_of(answer).contains(needle), "{bad:?}: {answer}");
        id += 1;
    }
    // The 64-byte boundary itself is accepted: the form goes out.
    let answer = legacy_review(&mut legacy, id, &longest, json!({"action": "cancel"}));
    assert_eq!(
        answer["structuredContent"]["action"],
        json!("cancel"),
        "{answer}"
    );
}

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
    legacy_review(
        &mut legacy,
        4,
        "P-0001",
        json!({"action": "accept", "content": {"decision": "approve", "comment": "ok"}}),
    );
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
    let state = stateless_request_state(&mut stateless, 2, &meta, "P-0001");
    let reply = stateless_answer(
        &mut stateless,
        3,
        &meta,
        "P-0001",
        Some(&state),
        Some(json!({"owner_review": {"action": "accept", "content": {"decision": "approve"}}})),
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
