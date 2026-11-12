//! docs/features/intake-similar.md (RM-0059) "Acceptance criteria", the MCP
//! crate's half: the real `specengine-mcp` binary over stdio on scratch git
//! copies of `fixtures/spec-a`, its library twin (`spec propose create …
//! --brief`, `content` its `2>&1`, `structuredContent` its `--json`) in
//! another `HOME`, and a hub server over a seam that runs every turn for
//! real, in memory (as `card_context.rs`'s).
//!
//! - IS6, the MCP part: `distinct_from` or `keywords` with an update, an
//!   interpretation or an amendment is -32602 naming the field and the kind,
//!   nothing stored; null is no value; the answers and `get_proposal` carry
//!   `similar: null`. M: `distinct_from` accepted on an update.
//! - IS8 `is8_*`: `propose_change {kind: "create", …, distinct_from,
//!   keywords}` answers as its twin, not stored, rehearsed and stored, each
//!   document conforming to the output schema; the input schema's two keys
//!   and the output's `similar` items with exact key sets; the description
//!   with "MCP"'s seven edits, 2 041 bytes; `INSTRUCTIONS` and
//!   `HUB_INSTRUCTIONS` byte-equal to before; a hub answers the IS1 call
//!   alike, its description 2 037 bytes, its `tools/list` 18 tools. M:
//!   `reason` dropped from the mirror's entry; `distinct_from` ignored on the
//!   hub's route.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::VecDeque;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::blake3::blake3_hex;
use common::read::{Era, Session, cli_env, conforms, content_text, expected, output_schemas};
use common::*;
use git::Sandbox;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{
    CreateRequest, Env, GitEnv, Globals, InboxRequest, Outcome, ProposedText, ShowRequest, inbox,
    show, utc_now,
};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer, TurnFuture,
};

/// The BLAKE3 of the stdio `INSTRUCTIONS` and `HUB_INSTRUCTIONS` before this
/// slice (`card_context.rs`'s pins, 3892f03's texts, unchanged since).
const INSTRUCTIONS_B3: &str = "7de2fdaa1885277df812c2e368a1fce532f8fc5aba6db0e6185ead5fbe87d351";
const HUB_INSTRUCTIONS_B3: &str =
    "a1762b8eee9cc29a6a6590e12a346f249927e106b01cfc9aea2b28be34734fa1";

/// The "Text" example's file and its note.
const ENDURANCE: &str = "docs/spec/movement/endurance.md";
const NOT_STORED: &str = "not stored: a similar entry gates; propose an update of it, or name \
                          each gating entry in `distinct_from` (--distinct-from) to store it \
                          anyway";

/// "The entry"'s example.
fn stamina_entry() -> Value {
    json!({"id": "MEC-STAMINA", "path": "docs/spec/movement/stamina.md", "title": "Stamina",
        "kind": "mechanic", "source": "title",
        "reason": "title equal to new `MEC-ENDURANCE`'s", "gates": true})
}

/// The entry's keys, in "The entry"'s order.
const ENTRY_KEYS: [&str; 7] = ["id", "path", "title", "kind", "source", "reason", "gates"];

/// The five sources, in the list's order.
const SOURCES: [&str; 5] = ["title", "alias", "queue", "search", "neighbours"];

/// "MCP"'s property docs.
const DISTINCT_DOC: &str = "create only: each gating similar entry you judge distinct, by its id, \
                            else its path, byte for byte.";
const KEYWORDS_DOC: &str = "create only: extra search words, one word each.";

/// "MCP"'s seven edits of both descriptions: `(before, after)`; the third's
/// `before` is the sentence's end, which the stdio and the hub texts share.
const EDITS: [(&str, &str); 7] = [
    (
        "(a section and its subsections, or a whole file)",
        "(a section with its subsections, or a file)",
    ),
    (
        "create: new ID sections below the node (text: its span with them), or a new file at a \
         free .md path (base null, text the whole file)",
        "create: new ID sections below the node (its span with them), or a new file at a free \
         .md path (base null)",
    ),
    (
        "refused naming the next free ID. ",
        "refused naming the next free ID; a similar entry with gates true stores nothing (id \
         null) until distinct_from names it. ",
    ),
    (
        "(not an alias, ID#SECTION or ID@rev), or a root-relative .md path (the whole file).",
        "(no alias, ID#SECTION or ID@rev), or a root-relative .md path.",
    ),
    ("why interpretation was needed", "why it was needed"),
    (
        "to live nodes, which stay unwritten.",
        "to live nodes, unwritten.",
    ),
    (
        "base: get_node's span_hash of the target; a stale",
        "base: get_node's span_hash; a stale",
    ),
];

/// The sentences the third edit extends: stdio's and a hub's.
const STDIO_SENTENCE: &str = "a taken one is refused naming the next free ID; a similar entry \
                              with gates true stores nothing (id null) until distinct_from \
                              names it.";
const HUB_SENTENCE: &str = "a taken or lower one is refused naming the next free ID; a similar \
                            entry with gates true stores nothing (id null) until distinct_from \
                            names it.";

/// A scratch git repository of spec-a: its main worktree (one commit on
/// `main`), a linked worktree on `t1`, a data `HOME`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    linked: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.copy("spec-a", "main");
        git.init(&main);
        git.add_all(&main);
        git.commit(&main, "the fixture");
        let linked = scratch.join("t1");
        git.git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "t1",
                linked.to_str().expect("a UTF-8 scratch path"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            main,
            linked,
            home,
        }
    }

    fn session(&self) -> Session {
        Session::open(
            Era::Stateless,
            &[],
            Some(&self.linked),
            Home::At(&self.home),
        )
    }

    /// `(span_hash, text)` of `reference` in the linked worktree.
    fn span(&self, reference: &str) -> (String, String) {
        let outcome = show(
            &cli_env(&self.linked, Some(&self.home)),
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                history: false,
                git: GitEnv::new(&self.linked, self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("show {reference}: {error}"));
        let node = outcome.nodes.into_iter().next().expect("one node");
        (node.span_hash, node.text)
    }

    /// The proposal IDs stored under `home`.
    fn stored(&self, home: &Path) -> Vec<String> {
        if !data_dir(home).join("lantern-keep.db").exists() {
            return Vec::new();
        }
        inbox(
            &cli_env(&self.main, Some(home)),
            &Globals::default(),
            &InboxRequest {
                all: true,
                git: GitEnv::new(&self.main, self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("inbox: {error}"))
        .proposals
        .into_iter()
        .map(|entry| entry.id)
        .collect()
    }
}

/// "Text"'s example: `endurance.md`, `title: STAMINA`, two link targets.
fn endurance() -> String {
    "---\nid: MEC-ENDURANCE\nclass: canon\ntier: 2\ntitle: STAMINA\nparent: DOM-MOVEMENT\n\
     status: accepted\nowner: owner\nreviewed: 2026-09-20\nlinks:\n  depends_on: [MEC-SPRINT]\n  \
     derived_from: [R-12]\n---\n\n# STAMINA\n\nA body of plain words.\n"
        .to_owned()
}

/// The IS1 call: a create of [`endurance`] with `keywords: ["drained"]`.
fn is1_call() -> Value {
    json!({"kind": "create", "target": ENDURANCE, "base": null, "text": endurance(),
        "rationale": "A new node.", "author_role": "writer", "keywords": ["drained"]})
}

/// The twin of a create call: the library's `--brief` (or `--dry-run
/// --brief`) with `HOME` `home` at the clock `now`.
fn twin(args: &Value, cwd: &Path, home: &Path, now: &str) -> common::read::Expected {
    let text = |key: &str| args[key].as_str().map(str::to_owned);
    let list = |key: &str| -> Vec<String> {
        args[key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| item.as_str().expect("a string").to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let request = CreateRequest {
        target: text("target").expect("a target"),
        base: text("base"),
        text: ProposedText::Given(text("text").expect("a text").into_bytes()),
        rationale: text("rationale").expect("a rationale"),
        author_role: text("author_role"),
        author_model: text("author_model"),
        run: text("run"),
        login: None,
        now: now.to_owned(),
        git: GitEnv::new(cwd, [("HOME", home.as_os_str())]),
        distinct_from: list("distinct_from"),
        keywords: list("keywords"),
    };
    let env = cli_env(cwd, Some(home));
    let outcome = if args["dry_run"] == json!(true) {
        specengine_cli::propose_create_dry_run_brief(&env, &Globals::default(), &request, None)
    } else {
        specengine_cli::propose_create_brief(&env, &Globals::default(), &request)
    };
    expected(outcome.map(|outcome| Outcome::Proposal(Box::new(outcome))))
}

/// The `similar:` lines of a text.
fn similar_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("similar: "))
        .collect()
}

/// A tool's input schema in a `tools/list` answer.
fn input_schema<'a>(list: &'a Value, name: &str) -> &'a Value {
    &tool(list, name)["inputSchema"]
}

// ------------------------------------------------------------------ IS6

/// IS6, the MCP part: each key with each other kind is invalid params
/// naming the field and the kind, nothing stored; `null` is no value (the
/// update stores, `similar: null`); `get_proposal` carries `similar: null`.
/// M: `distinct_from` accepted on an update.
#[test]
fn is6_the_keys_with_another_kind_are_invalid_params() {
    let repo = Repo::new("mis-is6");
    let (hash, span) = repo.span("EDGE-STAM-ZERO");
    let edited = span.replace("immediately", "at once");
    let update = json!({"kind": "update", "target": "EDGE-STAM-ZERO", "base": hash,
        "text": edited, "rationale": "Shorter.", "author_role": "writer"});
    let mut interpretation = update.clone();
    interpretation["kind"] = json!("interpretation");
    interpretation["decision"] = json!("DEC-0023");
    interpretation["how"] = json!("At once.");
    interpretation["why"] = json!("Rest.");
    let amendment = json!({"kind": "amendment", "target": "docs/features/rest.md", "base": null,
        "text": "---\nclass: spec\nstatus: draft\nscope: [keep]\nlinks:\n  amends: \
                 [EDGE-SPRINT-EMPTY]\n---\n\n# Rest\n\n## Wait {#AC-01}\n\nA wait.\n",
        "rationale": "A wait.", "author_role": "writer"});
    let mut session = repo.session();
    for (kind, word, args) in [
        ("update", "an update", &update),
        ("interpretation", "an interpretation", &interpretation),
        ("amendment", "an amendment", &amendment),
    ] {
        for (key, value) in [
            ("distinct_from", json!(["MEC-STAMINA"])),
            ("distinct_from", json!([])),
            ("keywords", json!(["drained"])),
        ] {
            let mut call = args.clone();
            call[key] = value.clone();
            let reply = session.call("propose_change", call);
            assert_eq!(error_code(&reply), INVALID_PARAMS, "{kind} {key}: {reply}");
            assert_eq!(
                reply["error"]["message"],
                json!(format!("{key}: only a create takes it, not {word}")),
                "{kind} {key} {value}"
            );
            assert_eq!(
                repo.stored(&repo.home),
                Vec::<String>::new(),
                "{kind} {key}"
            );
        }
    }
    let mut nulls = update.clone();
    nulls["distinct_from"] = Value::Null;
    nulls["keywords"] = Value::Null;
    let reply = session.call("propose_change", nulls);
    let structured = &result(&reply)["structuredContent"];
    assert_eq!(structured["id"], json!("PR-0001"), "{reply}");
    assert_eq!(structured["similar"], Value::Null, "{reply}");
    assert!(similar_lines(content_text(result(&reply))).is_empty());
    for (args, id) in [(&interpretation, "PR-0002"), (&amendment, "PR-0003")] {
        let reply = session.call("propose_change", args.clone());
        let structured = &result(&reply)["structuredContent"];
        assert_eq!(structured["id"], json!(id), "{reply}");
        assert_eq!(structured["similar"], Value::Null, "{reply}");
    }
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        let read = session.call("get_proposal", json!({"proposal_id": id}));
        assert_eq!(
            result(&read)["structuredContent"]["similar"],
            Value::Null,
            "{id}: {read}"
        );
        assert!(
            !content_text(result(&read))
                .lines()
                .any(|line| line.starts_with("similar")),
            "{id}: {read}"
        );
    }
}

// ------------------------------------------------------------------ IS8

/// IS8: not stored, rehearsed (gating, then named) and stored, each call's
/// `content` and `structuredContent` are its twin's; each document conforms
/// to the output schema; `get_proposal` of the stored one `similar: null`.
/// M: `reason` dropped from the mirror's entry.
#[test]
fn is8_propose_change_answers_as_its_twin() {
    let repo = Repo::new("mis-is8");
    let twin_home = repo.scratch.home("twin");
    let cwd = repo.linked.clone();
    let not_stored = is1_call();
    let mut rehearsed = is1_call();
    rehearsed["dry_run"] = json!(true);
    let mut rehearsed_named = rehearsed.clone();
    rehearsed_named["distinct_from"] = json!(["MEC-STAMINA"]);
    let mut stored = is1_call();
    stored["distinct_from"] = json!(["MEC-STAMINA"]);
    let mut session = repo.session();
    let list = session.tools();
    let schemas = output_schemas(&list);
    let schema = &schemas["propose_change"];
    let mut similar_of = Vec::new();
    for (label, args, id) in [
        ("not stored", &not_stored, None),
        ("rehearsed", &rehearsed, None),
        ("rehearsed, named", &rehearsed_named, None),
        ("stored", &stored, Some("PR-0001")),
    ] {
        let reply = session.call("propose_change", args.clone());
        let got = result(&reply);
        assert_ne!(got["isError"], json!(true), "{label}: {reply}");
        let structured = &got["structuredContent"];
        assert_eq!(structured["id"], json!(id), "{label}: {reply}");
        let now = match id {
            Some(id) => {
                let read = session.call("get_proposal", json!({"proposal_id": id}));
                let document = &result(&read)["structuredContent"];
                assert_eq!(document["similar"], Value::Null, "{label}: {read}");
                if let Err(problem) = conforms(&schemas["get_proposal"], document, "$") {
                    panic!("{label}: get_proposal breaks its outputSchema: {problem}");
                }
                document["created_at"]
                    .as_str()
                    .expect("created_at")
                    .to_owned()
            }
            None => utc_now(),
        };
        let want = twin(args, &cwd, &twin_home, &now);
        assert_eq!(content_text(got), want.text, "{label}: content");
        assert_eq!(
            Some(structured),
            want.document.as_ref(),
            "{label}: structuredContent"
        );
        if let Err(problem) = conforms(schema, structured, "$") {
            panic!("{label}: the document breaks the outputSchema: {problem}");
        }
        assert_eq!(structured["similar"][0], stamina_entry(), "{label}");
        for entry in structured["similar"].as_array().expect("a list") {
            assert_eq!(
                entry.as_object().unwrap().keys().count(),
                ENTRY_KEYS.len(),
                "{label}: {entry}"
            );
        }
        similar_of.push((label, structured["similar"].clone()));
        let text = content_text(got);
        match label {
            "not stored" => {
                assert_eq!(text.lines().next(), Some(NOT_STORED), "{label}: {text}");
                assert_eq!(structured["notes"][0], json!(NOT_STORED));
            }
            "rehearsed" => assert_eq!(
                structured["notes"].as_array().expect("notes")[..2],
                [json!("dry run: nothing stored"), json!(NOT_STORED)],
                "{label}: {structured}"
            ),
            "rehearsed, named" => assert_eq!(
                structured["notes"][0],
                json!("dry run: nothing stored"),
                "{label}"
            ),
            _ => assert_eq!(text.lines().next(), Some("PR-0001"), "{label}: {text}"),
        }
    }
    // One state, one list: the four lists equal.
    for (label, similar) in &similar_of[1..] {
        assert_eq!(similar, &similar_of[0].1, "{label}");
    }
    assert_eq!(repo.stored(&repo.home), ["PR-0001"]);
}

/// IS8, the schemas: `propose_change` takes `distinct_from` and `keywords`,
/// nullable string lists in "MCP"'s words, not required, under
/// `additionalProperties: false`; its output's, `get_proposal`'s (and
/// `review_proposal`'s, when listed) `similar` is null or a list of the
/// seven keys, `source` one of the five.
#[test]
fn is8_the_schemas_hold_the_keys() {
    let repo = Repo::new("mis-is8-schema");
    let mut session = repo.session();
    let list = session.tools();
    let input = input_schema(&list, "propose_change");
    assert_eq!(input["additionalProperties"], json!(false), "{input}");
    let required = input["required"].as_array().cloned().unwrap_or_default();
    for (key, doc) in [("distinct_from", DISTINCT_DOC), ("keywords", KEYWORDS_DOC)] {
        let property = &input["properties"][key];
        assert_eq!(
            property["type"],
            json!(["array", "null"]),
            "{key}: {property}"
        );
        assert_eq!(
            property["items"]["type"],
            json!("string"),
            "{key}: {property}"
        );
        assert_eq!(property["description"], json!(doc), "{key}");
        assert!(!required.contains(&json!(key)), "{key} is optional");
    }
    let schemas = output_schemas(&list);
    let mut seen = 0;
    for name in ["propose_change", "get_proposal", "review_proposal"] {
        let Some(schema) = schemas.get(name) else {
            continue;
        };
        seen += 1;
        let similar = &schema["properties"]["similar"];
        assert_eq!(
            similar["type"],
            json!(["array", "null"]),
            "{name}: {similar}"
        );
        let item = &similar["items"];
        let keys: Vec<&str> = item["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{name}: {similar}"))
            .keys()
            .map(String::as_str)
            .collect();
        let mut want = ENTRY_KEYS.to_vec();
        want.sort_unstable();
        assert_eq!(keys, want, "{name}");
        let mut required: Vec<&str> = item["required"]
            .as_array()
            .expect("required")
            .iter()
            .map(|key| key.as_str().unwrap())
            .collect();
        required.sort_unstable();
        assert_eq!(required, want, "{name}");
        assert_eq!(item["additionalProperties"], json!(false), "{name}");
        assert_eq!(
            item["properties"]["source"]["enum"],
            json!(SOURCES),
            "{name}: {item}"
        );
        assert_eq!(item["properties"]["gates"]["type"], json!("boolean"));
        // `similar` after `why`, before `notes`, in the declared order.
        let order: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
        assert!(order.iter().any(|key| *key == "similar"), "{name}");
    }
    assert!(seen >= 2, "{schemas:?}");
}

/// IS8, the texts: the stdio description takes "MCP"'s seven edits, ASCII,
/// 2 041 bytes, the sentence once; `INSTRUCTIONS` and `HUB_INSTRUCTIONS`
/// are byte-equal to before.
#[test]
fn is8_the_description_takes_the_edits_and_the_instructions_stay() {
    let repo = Repo::new("mis-is8-texts");
    let mut session = repo.session();
    let list = session.tools();
    let description = tool(&list, "propose_change")["description"]
        .as_str()
        .expect("a description");
    assert!(description.is_ascii());
    for (before, after) in EDITS {
        assert_eq!(
            description.matches(after).count(),
            1,
            "{after:?}: {description}"
        );
        if !after.contains(before) {
            assert!(!description.contains(before), "{before:?}: {description}");
        }
    }
    assert_eq!(
        description.matches(STDIO_SENTENCE).count(),
        1,
        "{description}"
    );
    assert_eq!(description.len(), 2041, "{description}");
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
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

/// A transport in memory, as `mcp_hub.rs`'s: the client's messages queued,
/// the server's kept; it closes once `expected` responses have come.
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

/// The hub server's answers over `root`: `tools/list` first, then each
/// `propose_change` of `calls`.
async fn hub_answers(root: &Path, home: &Path, calls: &[Value]) -> Vec<Value> {
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
    for (n, arguments) in calls.iter().enumerate() {
        inbound.push_back(client(json!({"jsonrpc": "2.0", "id": n + 2,
            "method": "tools/call", "params": {"name": "propose_change", "arguments": arguments}})));
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
                .and_then(|message| message.get("result"))
                .unwrap_or_else(|| panic!("no result for {id}: {messages:?}"))
                .clone()
        })
        .collect()
}

/// IS8, the hub: the IS1 call, and the IS1 call naming MEC-STAMINA, answer
/// on a hub as over stdio (not stored, then `PR-0001`, the same `similar`
/// and the same `similar:` lines); `tools/list` holds 18 tools; the hub's
/// description takes the edits at 2 037 bytes. M: `distinct_from` ignored
/// on the hub's route.
#[tokio::test]
async fn is8_a_hub_answers_the_is1_call_alike() {
    let local = Repo::new("mis-is8-stdio");
    let remote = Repo::new("mis-is8-hub");
    let mut named = is1_call();
    named["distinct_from"] = json!(["MEC-STAMINA"]);
    let calls = [is1_call(), named];
    let mut session = local.session();
    let stdio: Vec<Value> = calls
        .iter()
        .map(|args| result(&session.call("propose_change", args.clone())).clone())
        .collect();
    drop(session.finish());
    let hub = hub_answers(&remote.main, &remote.home, &calls).await;
    let listed = tool_names(&hub[0]);
    assert_eq!(listed.len(), 18, "{listed:?}");
    assert_eq!(HUB_TOOLS.len(), 18);
    for (n, (args, local)) in calls.iter().zip(&stdio).enumerate() {
        let remote = &hub[n + 1];
        assert_ne!(remote["isError"], json!(true), "{args}: {remote}");
        let (on_hub, here) = (&remote["structuredContent"], &local["structuredContent"]);
        assert_eq!(on_hub["id"], here["id"], "{args}: {on_hub}");
        assert_eq!(on_hub["similar"], here["similar"], "{args}");
        assert_eq!(on_hub["similar"][0], stamina_entry(), "{args}");
        assert_eq!(on_hub["notes"][0], here["notes"][0], "{args}");
        assert_eq!(
            similar_lines(content_text(remote)),
            similar_lines(content_text(local)),
            "{args}"
        );
    }
    assert_eq!(hub[1]["structuredContent"]["id"], Value::Null);
    assert_eq!(hub[2]["structuredContent"]["id"], json!("PR-0001"));
    assert_eq!(remote.stored(&remote.home), ["PR-0001"]);

    let description = tool(&hub[0], "propose_change")["description"]
        .as_str()
        .expect("a description");
    assert!(description.is_ascii());
    for (before, after) in EDITS {
        assert_eq!(description.matches(after).count(), 1, "{after:?}");
        if !after.contains(before) {
            assert!(!description.contains(before), "{before:?}");
        }
    }
    assert_eq!(
        description.matches(HUB_SENTENCE).count(),
        1,
        "{description}"
    );
    assert_eq!(description.len(), 2037, "{description}");
    let input = &tool(&hub[0], "propose_change")["inputSchema"];
    assert_eq!(
        input["properties"]["distinct_from"]["description"],
        json!(DISTINCT_DOC)
    );
    assert_eq!(
        input["properties"]["keywords"]["description"],
        json!(KEYWORDS_DOC)
    );
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
}
