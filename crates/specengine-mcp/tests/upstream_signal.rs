//! docs/features/upstream-signal.md (RM-0105) "Acceptance criteria", the
//! MCP half: `get_card` and `list_cards` on the default build's stdio
//! server, and on a hub server in memory, over the spec's scenario (the
//! shared scene of `specengine-cli`'s `tests/upstream_scene`).
//!
//! - US-10: stdio `get_card {card_id: "C-0002"}`'s content is the CLI
//!   library's `spec card show C-0002 --marks 2>&1`, its
//!   `structuredContent` the `--json` document, valid against the output
//!   schema, its `upstream` the spec's JSON object; `list_cards {marks: true}`
//!   likewise against `spec card list --marks`, the rows' flags `false`,
//!   `true`, `true`, `null` without `marks`. The schema walker's exact key
//!   sets: `Card` 36, a `list_cards` row 23, `upstream` 6, a change 9, and
//!   the four enums. A hub answers alike and lists 18 tools. M: `on`
//!   dropped from `CardUpstreamChangeShape`; `upstream` dropped from
//!   `CardRowShape`.
//! - US-11, the texts: `get_card`'s description 2 041 B and `list_cards`'
//!   1 588 B, ASCII, each with its sentence once and in its place;
//!   `INSTRUCTIONS` and `HUB_INSTRUCTIONS` the build before's. (`PINS`,
//!   `SCHEMA_PINS`, the version and `read-spec` are `plugin_files.rs`'s,
//!   the tag `specengine-http`'s `spec_ids_hub.rs`.)
//!
//! The size (US-10's last clause) is `mcp_size.rs`'s. Every server has a
//! cleared environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/upstream_scene/mod.rs"]
mod upstream_scene;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::blake3::blake3_hex;
use common::read::{Era, Session, assert_parity, conforms, content_text, library, output_schemas};
use common::*;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer, TurnFuture,
};
use upstream_scene::{Scene, Stop, data_upstream};

/// The BLAKE3 of the build before's `INSTRUCTIONS` and `HUB_INSTRUCTIONS`
/// (node-history's pins): US-11 keeps both byte-equal.
// Re-pinned by docs/features/collision-pass.md "Acceptance criteria", CP8
// (the collisions line in, the resources sentence out; `mcp_code.rs`' RD-5
// steps back to the value this slice pinned, `7de2fdaa…`).
const INSTRUCTIONS_B3: &str = "72797a127dbf98ef66644c92d46870dacb4c072d9cc10819654c38fd82d95fe4";
const HUB_INSTRUCTIONS_B3: &str =
    "a1762b8eee9cc29a6a6590e12a346f249927e106b01cfc9aea2b28be34734fa1";

/// US-11: `get_card`'s two edits and `list_cards`' sentence, as the spec's data
/// gives them.
const GET_CARD_ID: &str = "card_id: as spec card list writes it. history: true:";
const GET_SENTENCE: &str = "in one read. upstream: what changed above an open subtask, as \
                            list_cards names it, or null. context: true:";
const LIST_SENTENCE: &str = "a mark changes nothing. upstream, with marks: true when, since it \
                             was made, its parent's goal, criteria or targets were edited or a \
                             question, discrepancy or decision on their nodes was applied, or \
                             when the parent reads suspect or correcting; false on a closed card \
                             or one with no parent; an edit of its own goal, criteria or targets \
                             answers every change before it; null without marks; get_card lists \
                             what changed.";

/// The keys of the spec's JSON, sorted: the object's and a change's.
const UPSTREAM_KEYS: [&str; 6] = ["changes", "correction", "mark", "more", "on", "parent"];
const CHANGE_KEYS: [&str; 9] = [
    "at", "by", "fields", "item", "kind", "on", "record", "seq", "source",
];

/// The object schema's property names under `schema`, through `anyOf`.
fn properties(schema: &Value) -> Vec<String> {
    if let Some(properties) = schema["properties"].as_object() {
        let mut keys: Vec<String> = properties.keys().cloned().collect();
        keys.sort();
        return keys;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array() {
            for variant in variants {
                let found = properties(variant);
                if !found.is_empty() {
                    return found;
                }
            }
        }
    }
    Vec::new()
}

/// The object schema under `schema` (through `anyOf`, `items`).
fn object(schema: &Value) -> &Value {
    if schema["properties"].is_object() {
        return schema;
    }
    if schema["items"].is_object() {
        return object(&schema["items"]);
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array() {
            for variant in variants {
                if variant["properties"].is_object() || variant["items"].is_object() {
                    return object(variant);
                }
            }
        }
    }
    panic!("no object schema: {schema}")
}

/// The enum of a string schema (through `anyOf`, `items`), sorted.
fn enum_of(schema: &Value) -> Vec<String> {
    if let Some(values) = schema["enum"].as_array() {
        let mut values: Vec<String> = values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        values.sort();
        return values;
    }
    if schema["items"].is_object() {
        return enum_of(&schema["items"]);
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array() {
            for variant in variants {
                let found = enum_of(variant);
                if !found.is_empty() {
                    return found;
                }
            }
        }
    }
    Vec::new()
}

/// Whether the schema admits `null` (a nullable enum or object).
fn nullable(schema: &Value) -> bool {
    if schema["type"] == "null" {
        return true;
    }
    if let Some(types) = schema["type"].as_array() {
        return types.iter().any(|kind| kind == "null");
    }
    if schema["enum"]
        .as_array()
        .is_some_and(|values| values.contains(&Value::Null))
    {
        return true;
    }
    ["anyOf", "oneOf"].iter().any(|key| {
        schema[*key]
            .as_array()
            .is_some_and(|variants| variants.iter().any(nullable))
    })
}

/// `text` with every run of whitespace one space.
fn folded(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// Each row's `upstream`, by ID.
fn flags(document: &Value) -> Vec<(String, Value)> {
    document["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("an ID").to_owned(),
                row["upstream"].clone(),
            )
        })
        .collect()
}

// ------------------------------------------------------------------ US-10

/// US-10: `get_card` and `list_cards` are the CLI library's, content and
/// `structuredContent`, valid against their output schemas; C-0002's
/// `upstream` is the spec's JSON object and its text the spec's text lines; the rows
/// flag as `spec card list --marks` does, `null` without `marks`.
#[test]
fn us10_get_card_and_list_cards_are_the_cli() {
    let scene = Scene::scenario("mcp-us10", Stop::Data);
    let mut session = Session::open(
        Era::Stateless,
        &[],
        Some(&scene.root),
        Home::At(&scene.home),
    );
    let schemas = output_schemas(&session.tools());
    let own: Env = common::read::cli_env(&scene.root, Some(&scene.home));
    let calls = [
        ("get_card", json!({"card_id": "C-0002"})),
        ("get_card", json!({"card_id": "C-0002", "history": true})),
        ("get_card", json!({"card_id": "C-0001"})),
        ("list_cards", json!({"marks": true})),
        ("list_cards", json!({})),
    ];
    let mut replies = Vec::new();
    for (tool, args) in calls {
        let reply = session.call(tool, args.clone());
        let want = library(tool, &args, &own, &Globals::default());
        assert_parity(&reply, &want, &schemas[tool], &format!("{tool} {args}"));
        replies.push(result(&reply).clone());
    }
    drop(session.finish());

    assert_eq!(replies[0]["structuredContent"]["upstream"], data_upstream());
    assert_eq!(replies[0]["structuredContent"]["mark"], "open");
    let text = content_text(&replies[0]);
    assert!(
        text.contains(
            "correction: -\nupstream: parent C-0001 | suspect TIDE-BOOK\nupstream: 4 | edited \
             goal | Ann | 2026-11-12T09:00:00Z\nupstream: 9 | decision PR-0040 | record \
             DEC-0031 | on TIDE-NEAP | Ann Owner | 2026-11-12T10:00:00Z\nsnapshot: "
        ),
        "{text}"
    );
    assert_eq!(replies[1]["structuredContent"]["upstream"], data_upstream());
    assert_eq!(
        replies[2]["structuredContent"]["upstream"],
        Value::Null,
        "no parent"
    );
    assert!(content_text(&replies[2]).contains("\nupstream: -\n"));
    assert_eq!(
        flags(&replies[3]["structuredContent"]),
        [
            ("C-0001".to_owned(), json!(false)),
            ("C-0002".to_owned(), json!(true)),
            ("C-0003".to_owned(), json!(true)),
        ]
    );
    assert!(
        content_text(&replies[3]).contains(
            "C-0002 | copy/backlog | 1 | Write the tide table foreword | copy | - | - | \
             TIDE-SPRING | mandate binding | mark open | upstream\n"
        ),
        "{}",
        content_text(&replies[3])
    );
    for (id, flag) in flags(&replies[4]["structuredContent"]) {
        assert_eq!(flag, Value::Null, "{id} without marks");
    }
}

/// US-10: the output schemas' exact key sets: a card 36 with `upstream`,
/// a `list_cards` row 23 with `upstream` (a nullable boolean), `upstream`'s
/// 6 and a change's 9; the enums `mark` (nullable), `source`, `fields`'
/// items and `kind` (nullable). The walker takes the spec's JSON object whole and
/// refuses it without a change's `on`. M: `on` dropped from
/// `CardUpstreamChangeShape`; `upstream` dropped from `CardRowShape`.
#[test]
fn us10_the_schemas_hold_the_upstream_keys() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    drop(session.finish());
    let card = &tool(&list, "get_card")["outputSchema"];
    let card_keys = properties(card);
    assert_eq!(card_keys.len(), 36, "{card_keys:?}");
    assert!(card_keys.contains(&"upstream".to_owned()));
    let row = object(&tool(&list, "list_cards")["outputSchema"]["properties"]["cards"]);
    let row_keys = properties(row);
    assert_eq!(
        row_keys.len(),
        23,
        "M: upstream dropped from CardRowShape: {row_keys:?}"
    );
    let flag = &row["properties"]["upstream"];
    assert!(nullable(flag), "{flag}");
    assert!(
        flag["type"] == json!(["boolean", "null"])
            || flag["type"] == "boolean"
            || flag.to_string().contains("\"boolean\""),
        "{flag}"
    );

    let upstream_schema = &card["properties"]["upstream"];
    assert!(nullable(upstream_schema), "{upstream_schema}");
    let upstream = object(upstream_schema);
    assert_eq!(properties(upstream), UPSTREAM_KEYS, "{upstream}");
    let change = object(&upstream["properties"]["changes"]);
    assert_eq!(
        properties(change),
        CHANGE_KEYS,
        "M: on dropped from CardUpstreamChangeShape: {change}"
    );
    let mark = &upstream["properties"]["mark"];
    assert_eq!(enum_of(mark), ["correcting", "suspect"], "{mark}");
    assert!(nullable(mark), "{mark}");
    assert_eq!(enum_of(&change["properties"]["source"]), ["edit", "item"]);
    assert!(!nullable(&change["properties"]["source"]));
    assert_eq!(
        enum_of(&change["properties"]["fields"]),
        ["criteria", "goal", "targets"]
    );
    let kind = &change["properties"]["kind"];
    assert_eq!(
        enum_of(kind),
        ["decision", "discrepancy", "question"],
        "{kind}"
    );
    assert!(nullable(kind), "{kind}");
    for key in UPSTREAM_KEYS {
        assert!(
            upstream["required"]
                .as_array()
                .is_some_and(|required| required.contains(&json!(key))),
            "{key} required: {upstream}"
        );
    }

    let whole = data_upstream();
    conforms(upstream_schema, &whole, "$").unwrap_or_else(|problem| panic!("whole: {problem}"));
    conforms(upstream_schema, &Value::Null, "$")
        .unwrap_or_else(|problem| panic!("null: {problem}"));
    let mut no_on = whole.clone();
    no_on["changes"][1]
        .as_object_mut()
        .expect("a change")
        .remove("on");
    assert!(
        conforms(upstream_schema, &no_on, "$").is_err(),
        "on required"
    );
    let mut extra = whole.clone();
    extra["changes"][0]["note"] = json!("x");
    assert!(
        conforms(upstream_schema, &extra, "$").is_err(),
        "no other key"
    );
    let mut word = whole;
    word["mark"] = json!("departed");
    assert!(
        conforms(upstream_schema, &word, "$").is_err(),
        "a closed mark"
    );
}

// ---------------------------------------------------------------- the hub

/// A seam whose requests hold the login `mira` and whose turn runs each
/// command at once on one project.
struct Mira {
    env: Env,
    globals: Globals,
}

impl HubSeam for Mira {
    fn login(&self, _extensions: &Extensions) -> Option<String> {
        Some("mira".to_owned())
    }

    fn turn(&self, command: Command) -> TurnFuture {
        command.run(&self.env, &self.globals);
        Box::pin(async {})
    }
}

/// A transport in memory: the client's messages queued, the server's
/// kept; it closes once `expected` responses have come.
struct Memory {
    inbound: VecDeque<RxJsonRpcMessage<RoleServer>>,
    outbound: Arc<Mutex<Vec<Value>>>,
    expected: usize,
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
        let expected = self.expected;
        async move {
            if next.is_some() {
                return next;
            }
            let started = Instant::now();
            while responses(&outbound) < expected && started.elapsed() < Duration::from_secs(60) {
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

/// The hub server's answers to `calls` (`tools/list` first).
async fn hub_answers(scene: &Scene, calls: &[(&str, Value)]) -> Vec<Value> {
    let seam = Mira {
        env: common::read::cli_env(&scene.root, Some(&scene.home)),
        globals: scene.globals(),
    };
    let mut inbound = VecDeque::new();
    inbound.push_back(client(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {"protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}}}),
    ));
    inbound.push_back(client(
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    ));
    inbound.push_back(client(
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    ));
    for (n, (name, arguments)) in calls.iter().enumerate() {
        inbound.push_back(client(json!({"jsonrpc": "2.0", "id": n + 2,
            "method": "tools/call", "params": {"name": name, "arguments": arguments}})));
    }
    let outbound = Arc::new(Mutex::new(Vec::new()));
    let transport = Memory {
        inbound,
        outbound: Arc::clone(&outbound),
        expected: 2 + calls.len(),
    };
    let running = SpecEngineServer::hub(Arc::new(seam))
        .serve(transport)
        .await
        .expect("the handshake");
    let _ = tokio::time::timeout(Duration::from_secs(90), running.waiting()).await;
    let messages = outbound.lock().expect("the server's messages").clone();
    (1..=calls.len() + 1)
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

/// US-10, the hub: `get_card` and `list_cards {marks: true}` answer on a
/// hub as on stdio, content and `structuredContent`; `tools/list` holds 18
/// tools; US-11: the hub's two descriptions hold their sentences once, and
/// `HUB_INSTRUCTIONS` is the build before's.
#[tokio::test]
async fn us10_a_hub_answers_alike() {
    let scene = Scene::scenario("mcp-us10-hub", Stop::Data);
    let calls = [
        ("get_card", json!({"card_id": "C-0002"})),
        ("list_cards", json!({"marks": true})),
        ("get_card", json!({"card_id": "C-0003", "history": true})),
    ];
    let mut session = Session::open(
        Era::Stateless,
        &[],
        Some(&scene.root),
        Home::At(&scene.home),
    );
    let stdio: Vec<Value> = calls
        .iter()
        .map(|(name, args)| result(&session.call(name, args.clone())).clone())
        .collect();
    drop(session.finish());
    let hub = hub_answers(&scene, &calls).await;
    let listed = tool_names(&hub[0]);
    assert_eq!(listed.len(), 18, "{listed:?}");
    assert_eq!(HUB_TOOLS.len(), 18);
    for (n, ((name, args), local)) in calls.iter().zip(&stdio).enumerate() {
        let remote = &hub[n + 1];
        assert_ne!(remote["isError"], json!(true), "{name} {args}: {remote}");
        assert_eq!(
            remote["structuredContent"], local["structuredContent"],
            "{name} {args}"
        );
        assert_eq!(content_text(remote), content_text(local), "{name} {args}");
    }
    assert_eq!(hub[1]["structuredContent"]["upstream"], data_upstream());
    let get = folded(
        tool(&hub[0], "get_card")["description"]
            .as_str()
            .expect("text"),
    );
    assert_eq!(get.matches(GET_SENTENCE).count(), 1, "{get}");
    let list = folded(
        tool(&hub[0], "list_cards")["description"]
            .as_str()
            .expect("text"),
    );
    assert_eq!(list.matches(LIST_SENTENCE).count(), 1, "{list}");
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
}

// ------------------------------------------------------------------ US-11

/// US-11: stdio `get_card`'s description is 2 041 bytes of ASCII (2 047
/// since docs/features/plain-questions.md PQ10) with the
/// new `card_id` clause and the sentence once, right after `subtask_items`'
/// `…in one read.` and before `context: true:`; `list_cards`' is 1 588
/// bytes with its sentence once, right after `a mark changes nothing.`;
/// `INSTRUCTIONS` is the build before's. M: a sentence dropped.
#[test]
fn us11_the_descriptions_hold_the_sentences() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    drop(session.finish());
    let get = tool(&list, "get_card")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    assert!(get.is_ascii());
    assert_eq!(get.len(), 2047, "{get}");
    assert!(get.len() <= 2048);
    let flat = folded(&get);
    assert_eq!(flat.matches(GET_SENTENCE).count(), 1, "{get}");
    assert_eq!(flat.matches("upstream").count(), 1, "{get}");
    assert_eq!(flat.matches(GET_CARD_ID).count(), 1, "{get}");
    assert!(!flat.contains("C- and 4 or more digits"), "{get}");

    let cards = tool(&list, "list_cards")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    assert!(cards.is_ascii());
    assert_eq!(cards.len(), 1588, "{cards}");
    assert_eq!(folded(&cards).matches(LIST_SENTENCE).count(), 1, "{cards}");

    // 2 039 since docs/features/collision-pass.md CP8.
    assert_eq!(INSTRUCTIONS.len(), 2039);
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
}
