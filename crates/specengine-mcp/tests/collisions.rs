//! docs/features/collision-pass.md (RM-0060) "Acceptance criteria", the MCP
//! half: the real `specengine-mcp` binary over stdio on scratch git copies of
//! `fixtures/spec-a` with the example's `endurance.md` committed, each call
//! against its library twin (`spec collisions …`, `spec collisions mark …`:
//! `content` its `2>&1`, `structuredContent` its `--json`) in another
//! `HOME`, and a hub server over a seam that runs every turn for real, in
//! memory.
//!
//! - CP8: the stdio `tools/list` holds 32; `get_collisions` and `mark_pairs`
//!   answer as their twins in both eras, conforming to their output schemas
//!   (exact key sets); the input schemas: `get_collisions` flat, `mark_pairs`'
//!   `pairs` items closed with `verdict` the enum `distinct`, `reported`; the
//!   read's annotations and `_meta` are `get_tree`'s, the mark's
//!   `ask_question`'s; the descriptions ASCII within 2 048 bytes with the
//!   determinism sentence and the spec's facts; `INSTRUCTIONS` as "MCP"
//!   edits it (2 039 bytes) and `HUB_INSTRUCTIONS` byte-equal to before; a
//!   hub lists 18 and answers `get_collisions` and `mark_pairs` as unknown
//!   tools. M: the tools registered on the hub.
//! - CP5, the MCP part: a blank `author_role` is refused as its twin, the
//!   ledger and the last event `seq` unchanged; a form error (`verdict`
//!   `maybe`, an unknown key in a pair, no `author_role`) is rmcp's argument
//!   error result, nothing written; a valid mark writes one row, no event.
//!
//! docs/features/collision-names.md "Acceptance criteria", the MCP half:
//!
//! - CN7: over a copy whose `stamina-tuning.md` gains a `{#AC-01}` section
//!   and a new feature document `endurance-tuning.md` holds one (both
//!   `Regen floor holds`), `get_collisions` names the pair
//!   `endurance-tuning/AC-01` and `stamina-tuning/AC-01` and `mark_pairs`
//!   refuses a bare `AC-01` (CN3), each `content` byte-equal to its twin's
//!   `2>&1` and `structuredContent` to its `--json`, nothing written; the
//!   whole `tools/list` of both eras hashes as at collision-pass (no
//!   description, schema, annotation or `_meta` byte changed).
//!
//! The names (a stamina pool) are the spec's example; every expected value
//! is the twin's or worked out from the spec.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::{BTreeSet, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::blake3::blake3_hex;
use common::read::{
    BAD_ARGUMENTS, ERAS, Era, Expected, Session, TOOLS, cli_env, conforms, content_text,
    flat_input_problems, library, output_schemas,
};
use common::*;
use git::Sandbox;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer, TurnFuture,
};

/// `INSTRUCTIONS` as collision-pass edits them (`mcp_code.rs`' RD-5 builds the
/// text and steps back to department-split's), and `HUB_INSTRUCTIONS` as
/// they were (D3: no hub change).
const INSTRUCTIONS_B3: &str = "72797a127dbf98ef66644c92d46870dacb4c072d9cc10819654c38fd82d95fe4";
const HUB_INSTRUCTIONS_B3: &str =
    "a1762b8eee9cc29a6a6590e12a346f249927e106b01cfc9aea2b28be34734fa1";

/// "MCP": the line `INSTRUCTIONS` gains after the review line.
const COLLISIONS_LINE: &str = "- get_collisions, mark_pairs = spec collisions, collisions mark.\n";
const REVIEW_LINE: &str =
    "- review_proposal = review --brief after the owner's form; only when the owner asks.\n";

const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

const ENDURANCE: &str = "docs/spec/movement/endurance.md";
const ENDURANCE_TEXT: &str = "---\nid: MEC-ENDURANCE\ntitle: Stamina pool\nlinks:\n  \
                              depends_on: [MEC-SPRINT]\n---\n\n# Stamina pool\n\nThe pool a \
                              keeper draws on to run.\n";

/// The read's document keys, a pair's and a side's (sorted: the schema's
/// `properties` as `conforms` sees them).
const DOCUMENT_KEYS: [&str; 8] = [
    "any_kind", "kinds", "left", "limit", "notes", "pairs", "project", "skipped",
];
const PAIR_KEYS: [&str; 8] = [
    "a",
    "b",
    "marked",
    "reason",
    "score",
    "shared",
    "shared_targets",
    "titles",
];
const SIDE_KEYS: [&str; 6] = ["id", "kind", "norm_hash", "ord", "path", "title"];
const MARKED_KEYS: [&str; 3] = ["at", "proposal", "verdict"];
const MARK_KEYS: [&str; 3] = ["marked", "notes", "replaced"];
const ITEM_KEYS: [&str; 6] = ["a", "a_norm", "b", "b_norm", "proposal", "verdict"];

/// A scratch git repository of spec-a with `endurance.md` committed, and a
/// data `HOME` for the server and another for the twin.
struct Repo {
    _scratch: Scratch,
    _git: Sandbox,
    root: PathBuf,
    home: PathBuf,
    twin_home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        Self::with(label, |root| write(root, ENDURANCE, ENDURANCE_TEXT))
    }

    /// The copy as `edit` leaves it, committed.
    fn with(label: &str, edit: impl FnOnce(&Path)) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("spec-a", "main");
        edit(&root);
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture and the example");
        let home = scratch.home("h");
        let twin_home = scratch.home("twin");
        Self {
            _scratch: scratch,
            _git: git,
            root,
            home,
            twin_home,
        }
    }

    fn session(&self, era: Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }

    /// The tool's twin, the library in the twin's `HOME`.
    fn twin(&self, tool: &str, args: &Value) -> Expected {
        library(
            tool,
            args,
            &cli_env(&self.root, Some(&self.twin_home)),
            &Globals::default(),
        )
    }

    /// `sqlite3 <home's db> sql`, trimmed; a missing table reads as `None`.
    fn sql(&self, home: &Path, sql: &str) -> Option<String> {
        let db = data_dir(home).join("lantern-keep.db");
        let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
            .into_iter()
            .find(|candidate| Path::new(candidate).exists())
            .expect("a sqlite3 client");
        let output = Process::new(client)
            .arg(&db)
            .arg(sql)
            .stdin(Stdio::null())
            .output()
            .expect("sqlite3 runs");
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// The server's ledger rows and its last event `seq`.
    fn state(&self) -> (Option<String>, Option<String>) {
        (
            self.sql(&self.home, "SELECT a, b, verdict FROM collision_marks;"),
            self.sql(&self.home, "SELECT coalesce(max(seq), 0) FROM events;"),
        )
    }
}

/// The call's result, compared with its twin: content, `structuredContent`,
/// `isError`.
fn assert_twin(reply: &Value, want: &Expected, label: &str) {
    let got = result(reply);
    assert_eq!(content_text(got), want.text, "{label}: content");
    assert_eq!(
        got.get("structuredContent"),
        want.document.as_ref(),
        "{label}: structuredContent"
    );
    assert_eq!(
        got["isError"] == json!(true),
        want.is_error,
        "{label}: isError: {reply}"
    );
}

/// An object schema's property names, sorted.
fn keys(schema: &Value) -> Vec<String> {
    schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("no properties: {schema}"))
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The object schema among `schema` and its `anyOf`/`oneOf` options.
fn object_of(schema: &Value) -> &Value {
    if schema.get("properties").is_some() {
        return schema;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(found) = schema[key].as_array().and_then(|options| {
            options
                .iter()
                .find(|option| option.get("properties").is_some())
        }) {
            return found;
        }
    }
    panic!("no object schema: {schema}")
}

/// The norm of `id` the read lists (any pair's side named `id`).
fn norm_in(document: &Value, id: &str) -> String {
    for pair in document["pairs"].as_array().expect("pairs") {
        for side in ["a", "b"] {
            if pair[side]["id"] == id {
                return pair[side]["norm_hash"].as_str().expect("a norm").to_owned();
            }
        }
    }
    panic!("{id} is no side: {document}")
}

// ------------------------------------------------------------------- CP8

/// CP8: 32 tools; `get_collisions` with and without `kinds` and `limit`,
/// then `mark_pairs` of the example's pair, then the read again (the pair
/// hidden), each the twin's content and document, conforming; both eras.
#[test]
fn cp8_the_two_tools_answer_as_their_twins() {
    for era in ERAS {
        let repo = Repo::new(&format!("mcp-cp8-{era:?}"));
        let mut session = repo.session(era);
        let list = session.tools();
        assert_eq!(tool_names(&list).len(), 32, "{era:?}");
        assert_eq!(tool_names(&list), TOOLS, "{era:?}");
        let schemas = output_schemas(&list);
        let called = |session: &mut Session, tool: &str, args: Value| -> Value {
            let reply = session.call(tool, args.clone());
            let want = repo.twin(tool, &args);
            assert_twin(&reply, &want, &format!("{era:?} {tool} {args}"));
            let got = result(&reply);
            if let Some(document) = got.get("structuredContent") {
                conforms(&schemas[tool], document, tool)
                    .unwrap_or_else(|problem| panic!("{era:?} {tool}: {problem}"));
            }
            got.clone()
        };
        let read = called(
            &mut session,
            "get_collisions",
            json!({"kinds": ["mechanic"]}),
        );
        let document = &read["structuredContent"];
        assert_eq!(document["pairs"][0]["score"], json!(936), "{document}");
        assert_eq!(document["pairs"][0]["a"]["id"], json!("MEC-ENDURANCE"));
        called(&mut session, "get_collisions", json!({}));
        called(
            &mut session,
            "get_collisions",
            json!({"kinds": ["mechanic"], "any_kind": false, "limit": 1}),
        );
        called(&mut session, "get_collisions", json!({"any_kind": true}));
        // A limit outside 1 to 200: the CLI's line, no document.
        let refused = called(&mut session, "get_collisions", json!({"limit": 201}));
        assert_eq!(refused["isError"], json!(true));
        assert_eq!(
            content_text(&refused),
            "spec: --limit 201: the limit is 1 to 200\n"
        );

        let e = norm_in(document, "MEC-ENDURANCE");
        let s = norm_in(document, "MEC-STAMINA");
        let marked = called(
            &mut session,
            "mark_pairs",
            json!({"pairs": [{"a": "MEC-ENDURANCE", "a_norm": e, "b": "MEC-STAMINA",
                "b_norm": s, "verdict": "distinct", "proposal": null}],
                "author_role": "reviewer"}),
        );
        assert_eq!(
            marked["structuredContent"],
            json!({"marked": 1, "replaced": 0, "notes": []})
        );
        assert_eq!(
            content_text(&marked),
            "marked 1 pair(s), 0 replacing an earlier mark\n"
        );
        let after = called(
            &mut session,
            "get_collisions",
            json!({"kinds": ["mechanic"]}),
        );
        let pairs = after["structuredContent"]["pairs"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            pairs.iter().all(|pair| pair["a"]["id"] != "MEC-ENDURANCE"),
            "{era:?}: the marked pair is hidden: {pairs:?}"
        );
        assert_eq!(pairs.len(), 1, "{era:?}: SPRINT-STAMINA stays");
        drop(session.finish());
    }
}

/// CP8: the input schemas (`get_collisions` flat, its three keys;
/// `mark_pairs`' four, `pairs`' items closed with the six keys and
/// `verdict` an enum), the output schemas' exact key sets, the annotations,
/// `_meta`, the descriptions and `INSTRUCTIONS`.
#[test]
fn cp8_the_schemas_texts_and_instructions() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let get = tool(&list, "get_collisions").clone();
    let mark = tool(&list, "mark_pairs").clone();

    let input = &get["inputSchema"];
    assert_eq!(
        flat_input_problems("get_collisions", input),
        Vec::<String>::new()
    );
    assert_eq!(keys(input), ["any_kind", "kinds", "limit"]);
    assert!(input["kinds"]["items"].get("enum").is_none());

    let input = &mark["inputSchema"];
    assert_eq!(input["type"], json!("object"));
    assert_eq!(input["additionalProperties"], json!(false));
    assert_eq!(keys(input), ["author_model", "author_role", "pairs", "run"]);
    let required: BTreeSet<&str> = input["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(required, BTreeSet::from(["author_role", "pairs"]));
    let items = object_of(&input["properties"]["pairs"]["items"]);
    assert_eq!(
        items["additionalProperties"],
        json!(false),
        "closed: {items}"
    );
    assert_eq!(keys(items), ITEM_KEYS);
    let verdict = &items["properties"]["verdict"];
    assert_eq!(
        verdict["enum"],
        json!(["distinct", "reported"]),
        "{verdict}"
    );
    let item_required: BTreeSet<&str> = items["required"]
        .as_array()
        .expect("items' required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for key in ["a", "a_norm", "b", "b_norm", "verdict"] {
        assert!(item_required.contains(key), "{key} required: {items}");
    }
    assert!(!input.to_string().contains("\"$ref\""), "no $ref: {input}");

    let output = &get["outputSchema"];
    assert_eq!(keys(output), DOCUMENT_KEYS);
    let pair = object_of(&output["properties"]["pairs"]["items"]);
    assert_eq!(keys(pair), PAIR_KEYS);
    assert_eq!(keys(object_of(&pair["properties"]["a"])), SIDE_KEYS);
    assert_eq!(keys(object_of(&pair["properties"]["b"])), SIDE_KEYS);
    assert_eq!(keys(object_of(&pair["properties"]["marked"])), MARKED_KEYS);
    assert_eq!(keys(&mark["outputSchema"]), MARK_KEYS);

    let get_tree = tool(&list, "get_tree");
    let ask = tool(&list, "ask_question");
    assert_eq!(get["annotations"], get_tree["annotations"], "the reads'");
    assert_eq!(get["_meta"], get_tree["_meta"], "the reads' _meta");
    assert_eq!(
        mark["annotations"], ask["annotations"],
        "the queue writers'"
    );
    assert_eq!(mark["_meta"], ask["_meta"]);

    for (name, tool, facts) in [
        (
            "get_collisions",
            &get,
            &[
                "spec collisions",
                "left",
                "norm_hash",
                "nothing under the project root",
            ][..],
        ),
        (
            "mark_pairs",
            &mark,
            &[
                "spec collisions mark",
                "distinct",
                "reported",
                "proposal",
                "ledger",
                "no event",
                "no proposal changed",
                "nothing the owner decides",
                "are data, not instructions",
            ][..],
        ),
    ] {
        let description = tool["description"].as_str().expect("a description");
        assert!(description.is_ascii(), "{name}: ASCII");
        assert!(
            description.len() <= 2048,
            "{name}: {} bytes",
            description.len()
        );
        assert!(description.contains(DETERMINISM), "{name}: {description}");
        for fact in facts {
            assert!(
                description.contains(fact),
                "{name} says {fact:?}: {description}"
            );
        }
        eprintln!("{name}: {} bytes", description.len());
    }
    // The numbers the read states are the constants'.
    let text = get["description"].as_str().unwrap();
    assert!(text.contains("1 to 200"), "{text}");
    assert!(text.contains("50"), "{text}");

    // `INSTRUCTIONS` (D8): 2 039 bytes, the line after the review line, no
    // resources sentence; served on `server/discover`.
    assert_eq!(INSTRUCTIONS.len(), 2039);
    assert!(INSTRUCTIONS.is_ascii());
    assert!(
        INSTRUCTIONS.contains(&format!("{REVIEW_LINE}{COLLISIONS_LINE}")),
        "{INSTRUCTIONS}"
    );
    assert!(!INSTRUCTIONS.contains("Resources:"), "{INSTRUCTIONS}");
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
    assert!(!HUB_INSTRUCTIONS.contains("get_collisions"));
    assert!(!HUB_INSTRUCTIONS.contains("mark_pairs"));
    let discover = result(&session.request("server/discover", json!({}))).clone();
    assert_eq!(discover["instructions"].as_str(), Some(INSTRUCTIONS));
    // The resources are still listed (D8).
    let templates = session.request("resources/templates/list", json!({}));
    assert!(
        result(&templates).to_string().contains("spec://"),
        "{templates}"
    );
    drop(session.finish());
}

// ------------------------------------------------------------------- CP5

/// CP5 over MCP: a blank `author_role` is refused as its twin (exit 1: the
/// document, the reason its last note), nothing written; `verdict` `maybe`
/// and an unknown key in a pair are rmcp's argument error (an error result,
/// no document), nothing written.
#[test]
fn cp5_mcp_refusals_write_nothing() {
    let repo = Repo::new("mcp-cp5");
    let mut session = repo.session(Era::Stateless);
    // A question first: the queue and its events exist.
    let asked = session.call(
        "ask_question",
        json!({"node_ids": ["MEC-STAMINA"], "text": "Is the pool refilled at rest?",
            "working_answer": "Yes.", "price_of_other": "One scene.", "author_role": "writer",
            "distinct_from": ["Q-031", "A-101", "DEC-0023", "DEC-0007", "Q-032", "A-102"]}),
    );
    assert_ne!(result(&asked)["isError"], json!(true), "{asked}");
    let read = session.call("get_collisions", json!({"kinds": ["mechanic"]}));
    let document = result(&read)["structuredContent"].clone();
    let e = norm_in(&document, "MEC-ENDURANCE");
    let s = norm_in(&document, "MEC-STAMINA");
    let pair = json!({"a": "MEC-ENDURANCE", "a_norm": e, "b": "MEC-STAMINA", "b_norm": s,
        "verdict": "distinct"});
    let before = repo.state();
    assert_ne!(before.1, Some("0".to_owned()), "premise: events");

    let blank = json!({"pairs": [pair.clone()], "author_role": " "});
    let reply = session.call("mark_pairs", blank.clone());
    let want = repo.twin("mark_pairs", &blank);
    assert_twin(&reply, &want, "a blank author_role");
    assert_eq!(result(&reply)["isError"], json!(true));
    assert_eq!(result(&reply)["structuredContent"]["marked"], json!(0));
    assert_eq!(repo.state(), before, "nothing written, no event");

    let mut maybe = pair.clone();
    maybe["verdict"] = json!("maybe");
    let mut extra = pair.clone();
    extra["why"] = json!("an unknown key");
    for (what, args) in [
        ("maybe", json!({"pairs": [maybe], "author_role": "writer"})),
        (
            "an unknown key",
            json!({"pairs": [extra], "author_role": "writer"}),
        ),
        ("no author_role", json!({"pairs": [pair.clone()]})),
    ] {
        // A schema-invalid argument is a tool result, never a JSON-RPC
        // error (the reads' AC-05): rmcp's line, no document.
        let reply = session.call("mark_pairs", args);
        let got = result(&reply);
        assert_eq!(got["isError"], json!(true), "{what}: {reply}");
        assert!(
            content_text(got).starts_with(BAD_ARGUMENTS),
            "{what}: {reply}"
        );
        assert!(got.get("structuredContent").is_none(), "{what}: {reply}");
        assert_eq!(repo.state(), before, "{what}: nothing written");
    }

    // A valid mark: one row, no event.
    let reply = session.call(
        "mark_pairs",
        json!({"pairs": [pair], "author_role": "writer"}),
    );
    assert_eq!(
        result(&reply)["structuredContent"],
        json!({"marked": 1, "replaced": 0, "notes": []})
    );
    let after = repo.state();
    assert_eq!(
        after.0,
        Some("MEC-ENDURANCE|MEC-STAMINA|distinct".to_owned())
    );
    assert_eq!(after.1, before.1, "no event");
    drop(session.finish());
}

// ------------------------------------------------------------- the hub

/// A seam whose requests hold the login `ann` and whose turn runs each
/// command at once, for real, on one project.
struct Ann {
    env: Env,
    globals: Globals,
}

impl HubSeam for Ann {
    fn login(&self, _extensions: &Extensions) -> Option<String> {
        Some("ann".to_owned())
    }

    fn turn(&self, command: Command) -> TurnFuture {
        command.run(&self.env, &self.globals);
        Box::pin(async {})
    }
}

/// A transport in memory: the client's messages queued, the server's kept;
/// it closes once `expected` responses have come.
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
            while responses(&outbound) < expected && started.elapsed() < Duration::from_secs(90) {
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

/// The hub server's whole answers over `root`: `tools/list` first, then
/// each `(tool, arguments)` of `calls`, a result or an error each.
async fn hub_messages(root: &Path, home: &Path, calls: &[(&str, Value)]) -> Vec<Value> {
    let seam = Ann {
        env: Env {
            cwd: root.to_path_buf(),
            home: Some(home.as_os_str().to_os_string()),
            xdg_data_home: None,
        },
        globals: Globals {
            root: Some(root.to_path_buf()),
            config: None,
        },
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
    let _ = tokio::time::timeout(Duration::from_secs(120), running.waiting()).await;
    let messages = outbound.lock().expect("the server's messages").clone();
    (1..=calls.len() + 1)
        .map(|id| {
            messages
                .iter()
                .find(|message| message.get("id") == Some(&json!(id)))
                .unwrap_or_else(|| panic!("no answer for {id}: {messages:?}"))
                .clone()
        })
        .collect()
}

/// An unknown tool's answer on a hub: a JSON-RPC error, or an error result
/// naming the tool; never a document.
fn is_unknown_tool(message: &Value, name: &str) -> bool {
    if let Some(error) = message.get("error") {
        return error.to_string().contains(name) || error["code"] == json!(INVALID_PARAMS);
    }
    let result = &message["result"];
    result["isError"] == json!(true)
        && result.get("structuredContent").is_none()
        && result.to_string().contains(name)
}

/// CP8, D3: a hub lists its 18 tools, neither of the two, and answers
/// `get_collisions` and `mark_pairs` as unknown tools, nothing written;
/// `get_tree` there still answers. M: the tools registered on the hub.
#[tokio::test]
async fn cp8_a_hub_lists_18_and_knows_neither_tool() {
    let repo = Repo::new("mcp-cp8-hub");
    let pair = json!({"a": "MEC-ENDURANCE", "a_norm": format!("b3:{}", "0".repeat(64)),
        "b": "MEC-STAMINA", "b_norm": format!("b3:{}", "1".repeat(64)), "verdict": "distinct"});
    let calls = [
        ("get_collisions", json!({})),
        (
            "mark_pairs",
            json!({"pairs": [pair], "author_role": "writer"}),
        ),
        ("get_tree", json!({})),
    ];
    let hub = hub_messages(&repo.root, &repo.home, &calls).await;
    let listed = tool_names(&hub[0]["result"]);
    assert_eq!(listed.len(), 18, "{listed:?}");
    assert_eq!(HUB_TOOLS.len(), 18);
    for name in ["get_collisions", "mark_pairs"] {
        assert!(!listed.iter().any(|tool| tool == name), "{name} listed");
        assert!(!HUB_TOOLS.contains(&name), "{name} in HUB_TOOLS");
    }
    assert!(
        is_unknown_tool(&hub[1], "get_collisions"),
        "get_collisions: {}",
        hub[1]
    );
    assert!(
        is_unknown_tool(&hub[2], "mark_pairs"),
        "mark_pairs: {}",
        hub[2]
    );
    assert_ne!(hub[3]["result"]["isError"], json!(true), "{}", hub[3]);
    let ledger = repo.sql(&repo.home, "SELECT count(*) FROM collision_marks;");
    assert!(
        matches!(ledger.as_deref(), None | Some("0")),
        "no ledger row written: {ledger:?}"
    );
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
}

// ------------------------------------------------- collision-names CN7

/// `value` as canonical JSON: object keys in byte order at every depth, no
/// whitespace, arrays in order (`plugin_files.rs`' `canonical`).
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let mut keys: Vec<&String> = object.keys().collect();
            keys.sort();
            let fields: Vec<String> = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("a key"),
                        canonical(&object[key])
                    )
                })
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        scalar => serde_json::to_string(scalar).expect("a scalar"),
    }
}

/// The whole `tools/list` result a fresh stdio server gives in each era
/// (every tool's name, description, schemas, annotations and `_meta`), as
/// collision-pass shipped it at 8f04ae24: collision-names changes no byte
/// of it.
const TOOLS_LIST_B3: [(Era, &str); 2] = [
    (
        Era::Legacy,
        "ce3aabdec5915c109e8c04c14620c7fc056d3f612616c50bdb95122fe29da1f3",
    ),
    (
        Era::Stateless,
        "c59811a2f6e1aac18349256f547dd648e110805d08a3d0c3f3f7a5c0e7005b38",
    ),
];

/// The same listing since docs/features/collision-pairs.md PP7: the review
/// document's output schema gains `pair`, nothing else changes.
const TOOLS_LIST_PAIR_B3: [(Era, &str); 2] = [
    (
        Era::Legacy,
        "26f63aaf19a727cef9dc65b7d2fce7d6543633fa00edaef87375b19d7d426aee",
    ),
    (
        Era::Stateless,
        "6a8007bac542d36f2b1d0157fc8cec43bb1396857a8d53f014a4d1edbeb0b109",
    ),
];

/// The same listing since docs/features/usage-orphans.md (RM-0111) UO6:
/// `get_node`'s links gain `usage`, nothing else changes.
const TOOLS_LIST_USAGE_B3: [(Era, &str); 2] = [
    (
        Era::Legacy,
        "2e2c06dd221fea7722d510b5abb39bd544395d0747379cb6a80515cc0707068d",
    ),
    (
        Era::Stateless,
        "d6168077af38d5bc925b5f1e6f1f318bf94a5b9f173fa69ba69622436836e679",
    ),
];

/// `list` as collision-pairs listed it: no `usage` in any links block of
/// an output schema (every object schema holding `outgoing`, `incoming` and
/// `usage`; docs/features/usage-orphans.md UO6 adds it to `get_node`'s);
/// the tools whose schema held one.
fn without_usage(list: &mut Value) -> Vec<String> {
    fn strip(value: &mut Value) -> usize {
        let mut found = 0;
        let links = value
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| {
                ["outgoing", "incoming", "usage"]
                    .iter()
                    .all(|key| properties.contains_key(*key))
            });
        if links {
            if let Some(properties) = value.get_mut("properties").and_then(Value::as_object_mut) {
                properties.remove("usage");
            }
            if let Some(required) = value.get_mut("required").and_then(Value::as_array_mut) {
                required.retain(|key| key != "usage");
            }
            found += 1;
        }
        match value {
            Value::Object(object) => {
                for (_, inner) in object.iter_mut() {
                    found += strip(inner);
                }
            }
            Value::Array(items) => {
                for inner in items {
                    found += strip(inner);
                }
            }
            _ => {}
        }
        found
    }
    let mut held = Vec::new();
    for tool in list["tools"].as_array_mut().expect("tools") {
        let name = tool["name"].as_str().unwrap_or_default().to_owned();
        if let Some(schema) = tool.get_mut("outputSchema")
            && strip(schema) > 0
        {
            held.push(name);
        }
    }
    held
}

/// `list` as collision-pass listed it: no `pair` in any tool's output
/// schema (docs/features/collision-pairs.md PP7 adds it to the review
/// document's); the tools whose schema held one.
fn without_pair(list: &mut Value) -> Vec<String> {
    let mut held = Vec::new();
    for tool in list["tools"].as_array_mut().expect("tools") {
        let name = tool["name"].as_str().unwrap_or_default().to_owned();
        let Some(schema) = tool.get_mut("outputSchema") else {
            continue;
        };
        let removed = schema
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .is_some_and(|properties| properties.remove("pair").is_some());
        if removed {
            if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
                required.retain(|key| key != "pair");
            }
            held.push(name);
        }
    }
    held
}

/// docs/features/collision-names.md CN7, the listing half: the stdio
/// `tools/list` of both eras hashes as at 8f04ae24 (no description, schema,
/// annotation or `_meta` byte changed), and `INSTRUCTIONS` and
/// `HUB_INSTRUCTIONS` as collision-pass pinned them. Since
/// docs/features/collision-pairs.md PP7 the listing is
/// [`TOOLS_LIST_PAIR_B3`], and without the review document's `pair` it
/// is still 8f04ae24's byte for byte. Since docs/features/usage-orphans.md
/// UO6 the listing is [`TOOLS_LIST_USAGE_B3`], and without `get_node`'s
/// links `usage` it is collision-pairs' byte for byte.
#[test]
fn cn7_the_tools_list_is_byte_equal_to_collision_pass() {
    assert_eq!(TOOLS_LIST_B3.map(|(era, _)| era), ERAS);
    assert_eq!(TOOLS_LIST_PAIR_B3.map(|(era, _)| era), ERAS);
    assert_eq!(TOOLS_LIST_USAGE_B3.map(|(era, _)| era), ERAS);
    for (((era, pinned), (_, paired)), (_, used)) in TOOLS_LIST_B3
        .into_iter()
        .zip(TOOLS_LIST_PAIR_B3)
        .zip(TOOLS_LIST_USAGE_B3)
    {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let mut list = session.tools();
        assert_eq!(
            blake3_hex(canonical(&list).as_bytes()),
            used,
            "{era:?}: tools/list changed"
        );
        let usage = without_usage(&mut list);
        assert_eq!(usage, ["get_node"], "{era:?}: get_node's links alone");
        assert_eq!(
            blake3_hex(canonical(&list).as_bytes()),
            paired,
            "{era:?}: tools/list without `usage` is collision-pairs'"
        );
        let held = without_pair(&mut list);
        assert!(
            held.contains(&"get_proposal".to_owned()),
            "{era:?}: get_proposal returns pair: {held:?}"
        );
        assert_eq!(
            blake3_hex(canonical(&list).as_bytes()),
            pinned,
            "{era:?}: tools/list without `pair` is collision-pass'"
        );
        drop(session.finish());
    }
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
}

// ------------------------------------------------- collision-names CN7, twins

const STAMINA_TUNING: &str = "docs/features/stamina-tuning.md";
const ENDURANCE_TUNING: &str = "docs/features/endurance-tuning.md";
const STAMINA_AC: &str = "stamina-tuning/AC-01";
const ENDURANCE_AC: &str = "endurance-tuning/AC-01";

/// spec-a whose `stamina-tuning.md` gains a `{#AC-01}` section and whose
/// new feature document `endurance-tuning.md` holds one, both `Regen floor
/// holds`.
fn floors(label: &str) -> Repo {
    Repo::with(label, |root| {
        let stamina = std::fs::read_to_string(root.join(STAMINA_TUNING)).expect("the feature");
        write(
            root,
            STAMINA_TUNING,
            format!(
                "{stamina}\n### Regen floor holds {{#AC-01}}\n\nAt zero stamina the pool still \
                 regains one point a second.\n"
            ),
        );
        write(
            root,
            ENDURANCE_TUNING,
            "---\nclass: spec\nstatus: draft\nscope: [movement]\nref: owner request, endurance \
             tuning\n---\n\n# Endurance tuning\n\nWhat the tuning measures.\n\n## Acceptance \
             criteria {#acceptance}\n\n### Regen floor holds {#AC-01}\n\nA keeper resting on a \
             landing never drops below ten points.\n",
        );
    })
}

/// CN7 over CN1 and CN3: `get_collisions {kinds: [criterion]}` names the
/// one pair by its canonical names; `mark_pairs` naming `AC-01` bare is
/// refused (`isError`, the resolver's reason the document's last note),
/// nothing written, no event; the canonical names mark (one row keyed by
/// them) and the read hides the pair; every call its twin's content and
/// document, conforming; both eras.
#[test]
fn cn7_canonical_names_answer_as_their_twins() {
    let reason = format!(
        "pairs[0].a: `AC-01` is feature-scoped: cite it as `{ENDURANCE_AC}` or `{STAMINA_AC}`"
    );
    for era in ERAS {
        let repo = floors(&format!("mcp-cn7-{era:?}"));
        let mut session = repo.session(era);
        let schemas = output_schemas(&session.tools());
        let called = |session: &mut Session, tool: &str, args: Value| -> Value {
            let reply = session.call(tool, args.clone());
            let want = repo.twin(tool, &args);
            assert_twin(&reply, &want, &format!("{era:?} {tool} {args}"));
            let got = result(&reply);
            if let Some(document) = got.get("structuredContent") {
                conforms(&schemas[tool], document, tool)
                    .unwrap_or_else(|problem| panic!("{era:?} {tool}: {problem}"));
            }
            got.clone()
        };
        // A question first: the queue and its events exist.
        let asked = session.call(
            "ask_question",
            json!({"node_ids": ["MEC-STAMINA"], "text": "Does the regen floor hold on stairs?",
                "working_answer": "Yes.", "price_of_other": "One scene.", "author_role": "writer",
                "distinct_from": ["Q-031", "A-101", "DEC-0023", "DEC-0007", "Q-032", "A-102"]}),
        );
        assert_ne!(result(&asked)["isError"], json!(true), "{asked}");

        let read = called(
            &mut session,
            "get_collisions",
            json!({"kinds": ["criterion"]}),
        );
        let document = read["structuredContent"].clone();
        let names: Vec<(&Value, &Value)> = document["pairs"]
            .as_array()
            .expect("pairs")
            .iter()
            .map(|pair| (&pair["a"]["id"], &pair["b"]["id"]))
            .collect();
        assert_eq!(
            names,
            [(&json!(ENDURANCE_AC), &json!(STAMINA_AC))],
            "{era:?}: {document}"
        );
        assert!(
            content_text(&read).contains(&format!("1000 | {ENDURANCE_AC} | {STAMINA_AC} | ")),
            "{era:?}: {read}"
        );
        let e = norm_in(&document, ENDURANCE_AC);
        let s = norm_in(&document, STAMINA_AC);

        let before = repo.state();
        let bare = json!({"pairs": [{"a": "AC-01", "a_norm": e, "b": STAMINA_AC, "b_norm": s,
            "verdict": "distinct", "proposal": null}], "author_role": "reviewer"});
        let refused = called(&mut session, "mark_pairs", bare);
        assert_eq!(refused["isError"], json!(true), "{era:?}: {refused}");
        let notes = &refused["structuredContent"]["notes"];
        assert_eq!(
            notes.as_array().and_then(|notes| notes.last()),
            Some(&json!(reason)),
            "{era:?}: {refused}"
        );
        assert_eq!(refused["structuredContent"]["marked"], json!(0));
        assert_eq!(repo.state(), before, "{era:?}: nothing written, no event");

        let marked = called(
            &mut session,
            "mark_pairs",
            json!({"pairs": [{"a": ENDURANCE_AC, "a_norm": e, "b": STAMINA_AC, "b_norm": s,
                "verdict": "distinct", "proposal": null}], "author_role": "reviewer"}),
        );
        assert_eq!(
            marked["structuredContent"],
            json!({"marked": 1, "replaced": 0, "notes": []})
        );
        assert_eq!(
            repo.state(),
            (
                Some(format!("{ENDURANCE_AC}|{STAMINA_AC}|distinct")),
                before.1.clone()
            ),
            "{era:?}: one row by the canonical names, no event"
        );
        let after = called(
            &mut session,
            "get_collisions",
            json!({"kinds": ["criterion"]}),
        );
        assert_eq!(
            after["structuredContent"]["pairs"],
            json!([]),
            "{era:?}: the pair hidden"
        );
        drop(session.finish());
    }
}
