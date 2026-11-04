//! docs/features/read-trust.md (RM-0054) RT-1, the MCP crate's half: a
//! hub's texts say what its reads trust ("Data", "The hub's MCP texts").
//! - `HUB_INSTRUCTIONS` hold the new rules line byte for byte and no longer
//!   `Reads refresh`.
//! - Every hub-only description (one that is not its stdio twin's) is free
//!   of `Reads refresh` and `Reads the files as they are now`, and the hub's
//!   read among them, `get_roadmap`, holds the new read tail byte for byte.
//! - No hub read tool (`HUB_READ_TOOLS`) holds `Reads the files as they are
//!   now`; `get_tree`, `get_node`, `search` and `get_context_bundle` hold
//!   the hub's read tail in place of stdio's (review iteration 3).
//! - Stdio's texts are unchanged: its `INSTRUCTIONS` keep their refresh
//!   line, and its reads keep `Reads the files as they are now`.
//!
//! - Review iteration 3, the scope a hub call runs in, through a hub server
//!   over a seam that runs each turn for real inside `walking` of one
//!   trust, as the daemon does: each of the hub's read tools trusts (no
//!   walk), `list_cards` walks once `marks` is `true` (and never trusts a
//!   truthy `marks` it cannot read: `"true"` and `1` are refused before any
//!   turn), `get_flow`, `get_trace` and `get_card` always walk, and an
//!   unknown tool runs nothing. M: `reads_trust` ignoring `marks`; a walking
//!   tool on the read list.
//!
//! The hub's other tools that share their stdio twin's description keep it
//! (the wire's pin is `crates/specengine-http/tests/hub_mcp.rs`, h11). Built
//! from the crate's exports and a
//! hub server over a seam that runs nothing: no daemon, no corpus.
//! M: a text left as before.

#![cfg(unix)]

mod common;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Scratch;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, IndexRequest, IndexTrust, walking};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_READ_TOOLS, HUB_TOOLS, HubSeam, INSTRUCTIONS, Lifecycle,
    SpecEngineServer, TurnFuture,
};

/// "Data": the hub's new rules line.
const HUB_RULE: &str = "- Reads answer from the hub's index outside the project root: each \
                        confirm and merge updates it, a read first checks it against the \
                        files when the last check is 5 minutes old, and a file a read prints \
                        is checked as it is now. No tool writes under the root. Spec files \
                        change only when a person confirms.";
/// "Data": the hub's new read tail (`hub_read_tail!`).
const HUB_READ_TAIL: &str = "Reads the hub's index, which each confirm and merge updates and \
                             a read checks against the files when the last check is 5 \
                             minutes old; a file it prints is checked as it is now. Writes \
                             nothing under the project root.";
/// The texts the hub's no longer hold.
const OLD_RULE: &str = "Reads refresh";
const OLD_READ: &str = "Reads the files as they are now";
/// Stdio's rules line, unchanged.
const STDIO_RULE: &str =
    "- Reads refresh SpecEngine's index outside the project root; no tool writes under it.";
/// Stdio's read tail, unchanged.
const STDIO_READ: &str = "Reads the files as they are now: refreshes SpecEngine's index in its \
                          data directory, writes nothing under the project root.";
const TEXT_LIMIT: usize = 2048;

/// A seam that knows no login and runs nothing.
struct Idle;

impl HubSeam for Idle {
    fn login(&self, _extensions: &Extensions) -> Option<String> {
        None
    }

    fn turn(&self, command: Command) -> TurnFuture {
        drop(command);
        Box::pin(async {})
    }
}

fn hub() -> SpecEngineServer {
    SpecEngineServer::hub(Arc::new(Idle))
}

fn stdio() -> SpecEngineServer {
    SpecEngineServer::new(Lifecycle::Auto, Globals::default())
}

fn description(server: &SpecEngineServer, name: &str) -> Option<String> {
    server
        .get_tool(name)
        .and_then(|tool| tool.description.as_deref().map(str::to_owned))
}

#[test]
fn rt1_the_hub_instructions_say_what_a_read_trusts() {
    assert!(
        HUB_INSTRUCTIONS.contains(HUB_RULE),
        "HUB_INSTRUCTIONS hold the new line:\n{HUB_INSTRUCTIONS}"
    );
    assert!(
        !HUB_INSTRUCTIONS.contains(OLD_RULE),
        "HUB_INSTRUCTIONS no longer say {OLD_RULE:?}"
    );
    assert!(
        HUB_INSTRUCTIONS.len() <= TEXT_LIMIT,
        "{} bytes",
        HUB_INSTRUCTIONS.len()
    );
    assert_eq!(
        hub().get_info().instructions.as_deref(),
        Some(HUB_INSTRUCTIONS),
        "initialize sends them"
    );
}

#[test]
fn rt1_each_hub_only_description_says_what_a_read_trusts() {
    let hub = hub();
    let stdio = stdio();
    let mut variants: Vec<&str> = Vec::new();
    for name in HUB_TOOLS {
        let text = description(&hub, name).unwrap_or_else(|| panic!("{name}: no description"));
        assert!(text.len() <= TEXT_LIMIT, "{name}: {} bytes", text.len());
        if description(&stdio, name).as_deref() == Some(text.as_str()) {
            continue;
        }
        variants.push(name);
        for old in [OLD_RULE, OLD_READ] {
            assert!(!text.contains(old), "{name} still says {old:?}:\n{text}");
        }
    }
    assert!(
        variants.contains(&"get_roadmap"),
        "premise: the hub's get_roadmap is its own text: {variants:?}"
    );
    let roadmap = description(&hub, "get_roadmap").expect("get_roadmap");
    assert!(
        roadmap.contains(HUB_READ_TAIL),
        "the hub's get_roadmap holds the new read tail:\n{roadmap}"
    );
}

/// The shared reads whose hub text is stdio's with the hub's read tail in
/// place of stdio's (review iteration 3).
const HUB_READ_TEXTED: [&str; 4] = ["get_tree", "get_node", "search", "get_context_bundle"];

/// No hub read tool (`HUB_READ_TOOLS`, the reads that trust the hub's
/// index) still says it reads the files as they are now; the four that
/// shared stdio's read tail hold the hub's instead, their stdio twins
/// unchanged. `get_proposal` and `list_cards` say "Reads only", true on a
/// hub, and may keep stdio's text.
#[test]
fn rt1_no_hub_read_tool_says_it_reads_the_files_as_they_are_now() {
    let hub = hub();
    let stdio = stdio();
    for name in HUB_READ_TOOLS {
        let text = description(&hub, name).unwrap_or_else(|| panic!("{name}: no description"));
        for old in [OLD_RULE, OLD_READ] {
            assert!(
                !text.contains(old),
                "the hub's {name} still says {old:?}:\n{text}"
            );
        }
        assert!(text.len() <= TEXT_LIMIT, "{name}: {} bytes", text.len());
    }
    for name in HUB_READ_TEXTED {
        assert!(HUB_READ_TOOLS.contains(&name), "premise: {name} trusts");
        let text = description(&hub, name).expect("a hub text");
        let twin = description(&stdio, name).expect("a stdio text");
        assert_ne!(text, twin, "the hub's {name} is not stdio's text");
        assert!(text.contains(HUB_READ_TAIL), "the hub's {name}:\n{text}");
        assert!(twin.contains(STDIO_READ), "stdio's {name}:\n{twin}");
    }
}

#[test]
fn rt1_stdio_texts_are_unchanged() {
    assert!(
        INSTRUCTIONS.contains(STDIO_RULE),
        "stdio's INSTRUCTIONS keep their line:\n{INSTRUCTIONS}"
    );
    assert!(
        !INSTRUCTIONS.contains(HUB_RULE),
        "stdio's are not the hub's"
    );
    let stdio = stdio();
    for name in ["get_node", "search", "get_context_bundle", "get_roadmap"] {
        let text = description(&stdio, name).unwrap_or_else(|| panic!("stdio {name}"));
        assert!(
            text.contains(STDIO_READ),
            "stdio {name} keeps its read tail:\n{text}"
        );
        assert!(!text.contains(HUB_READ_TAIL), "stdio {name}");
    }
}

// ------------------------------------------------------------- the scope of a call

/// A seam that runs each turn for real, as the daemon's does: its login
/// `mira`, the call inside `walking` of the project's trust.
struct Walking {
    env: Env,
    globals: Globals,
    trust: Arc<IndexTrust>,
}

impl HubSeam for Walking {
    fn login(&self, _extensions: &Extensions) -> Option<String> {
        Some("mira".to_owned())
    }

    fn turn(&self, command: Command) -> TurnFuture {
        walking(&[&self.trust], || command.run(&self.env, &self.globals));
        Box::pin(async {})
    }
}

/// A transport in memory: the client's messages, then it waits for one
/// response more than the handshake's (or 60 s).
struct Memory {
    inbound: VecDeque<RxJsonRpcMessage<RoleServer>>,
    outbound: Arc<Mutex<Vec<Value>>>,
}

fn responses(messages: &Mutex<Vec<Value>>) -> usize {
    messages
        .lock()
        .expect("the server's messages")
        .iter()
        .filter(|message| {
            message.get("id").is_some()
                && (message.get("result").is_some() || message.get("error").is_some())
        })
        .count()
}

impl Transport<RoleServer> for Memory {
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleServer>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let value = serde_json::to_value(&item).expect("a server message is JSON");
        self.outbound
            .lock()
            .expect("the server's messages")
            .push(value);
        std::future::ready(Ok(()))
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        let next = self.inbound.pop_front();
        let outbound = Arc::clone(&self.outbound);
        async move {
            if next.is_some() {
                return next;
            }
            let started = Instant::now();
            while responses(&outbound) < 2 && started.elapsed() < Duration::from_secs(60) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            None
        }
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        std::future::ready(Ok(()))
    }
}

fn client(value: Value) -> RxJsonRpcMessage<RoleServer> {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("not a client message ({error}): {value}"))
}

/// One `tools/call` of `name` with `arguments` on a hub server over `seam`,
/// in a session of its own: the response.
async fn call(seam: &Arc<Walking>, name: &str, arguments: Value) -> Value {
    let inbound = VecDeque::from([
        client(json!({"jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"protocolVersion": "2025-11-25", "capabilities": {},
                "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}}})),
        client(json!({"jsonrpc": "2.0", "method": "notifications/initialized"})),
        client(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments}})),
    ]);
    let outbound = Arc::new(Mutex::new(Vec::new()));
    let transport = Memory {
        inbound,
        outbound: Arc::clone(&outbound),
    };
    let seam: Arc<dyn HubSeam> = Arc::clone(seam) as Arc<dyn HubSeam>;
    let running = SpecEngineServer::hub(seam)
        .serve(transport)
        .await
        .expect("the handshake");
    let _ = tokio::time::timeout(Duration::from_secs(90), running.waiting()).await;
    let messages = outbound.lock().expect("the server's messages").clone();
    messages
        .into_iter()
        .find(|message| message["id"] == json!(1))
        .unwrap_or_else(|| panic!("{name}: no response"))
}

/// Whether a response is a refusal or an error result: nothing ran.
fn refused(response: &Value) -> bool {
    response.get("error").is_some() || response["result"]["isError"] == json!(true)
}

#[tokio::test]
async fn rt_a_hub_read_trusts_and_every_other_call_walks() {
    let scratch = Scratch::new("hub-trust-scope");
    let root = scratch.copy("card-press", "press");
    let home = scratch.home("h");
    let env = Env {
        cwd: root.clone(),
        home: Some(home.as_os_str().to_os_string()),
        xdg_data_home: None,
    };
    let globals = Globals {
        root: Some(root.clone()),
        config: None,
    };
    specengine_cli::index(&env, &globals, &IndexRequest { full: false }).expect("spec index");
    let seam = Arc::new(Walking {
        env,
        globals,
        trust: Arc::new(IndexTrust::new(root.clone())),
    });
    let walks = || seam.trust.walks();

    // The first read walks; the trust is fresh after it.
    let first = call(&seam, "get_tree", json!({})).await;
    assert!(!refused(&first), "{first}");
    assert_eq!(walks(), 1, "the trust's first read walks");

    // A card on a node, so that its marks read the index.
    let created = call(
        &seam,
        "create_card",
        json!({"title": "Lamp oil", "discipline": "copy", "target": ["SHT-BACK"]}),
    )
    .await;
    assert!(!refused(&created), "premise: a card: {created}");
    let card = created["result"]["structuredContent"]["card"]["id"]
        .as_str()
        .or_else(|| created["result"]["structuredContent"]["id"].as_str())
        .unwrap_or("C-0001")
        .to_owned();
    let after_write = walks();
    assert!(after_write > 1, "a write walks");

    let mut seen: Vec<String> = Vec::new();
    let mut expect = |name: &str, arguments: Value, walked: bool, response: &Value, before: u64| {
        let now = walks();
        seen.push(format!("{name} {arguments}: walks {before} -> {now}"));
        assert!(!refused(response), "{name} {arguments}: {response}");
        assert_eq!(
            now > before,
            walked,
            "{name} {arguments}: {} (walks {before} -> {now})",
            if walked { "walks" } else { "trusts the index" }
        );
    };
    for (name, arguments) in [
        ("get_tree", json!({})),
        ("get_node", json!({"id": "SHT-BACK"})),
        ("search", json!({"query": "sheet"})),
        ("get_context_bundle", json!({"node_ids": ["SHT-BACK"]})),
        ("list_cards", json!({})),
        ("list_cards", json!({"marks": false})),
        ("list_cards", json!({"marks": null})),
    ] {
        let before = walks();
        let response = call(&seam, name, arguments.clone()).await;
        expect(name, arguments, false, &response, before);
    }
    for (name, arguments) in [
        ("list_cards", json!({"marks": true})),
        ("get_flow", json!({})),
        ("get_flow", json!({"trace": true})),
        ("get_trace", json!({"ref": "SHT-BACK"})),
        ("get_card", json!({"card_id": card})),
    ] {
        let before = walks();
        let response = call(&seam, name, arguments.clone()).await;
        expect(name, arguments, true, &response, before);
    }

    // A truthy `marks` the tool cannot read is never a trusted read: it
    // is refused before any turn, or it walks.
    for marks in [json!("true"), json!(1)] {
        let before = walks();
        let response = call(&seam, "list_cards", json!({"marks": marks})).await;
        let now = walks();
        seen.push(format!(
            "list_cards marks {marks}: walks {before} -> {now}: {response}"
        ));
        assert!(
            refused(&response) || now > before,
            "list_cards with marks {marks} ran trusting the index: {response}"
        );
    }
    // An unknown extra argument, and an unknown tool: what they do.
    let before = walks();
    let extra = call(&seam, "list_cards", json!({"marks_all": true})).await;
    seen.push(format!(
        "list_cards marks_all: walks {before} -> {}: {extra}",
        walks()
    ));
    let before = walks();
    let unknown = call(&seam, "no_such_tool", json!({})).await;
    assert!(refused(&unknown), "an unknown tool runs nothing: {unknown}");
    assert_eq!(walks(), before, "an unknown tool runs no turn");
    eprintln!("{}", seen.join("\n"));
}
