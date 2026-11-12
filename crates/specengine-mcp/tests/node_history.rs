//! docs/features/node-history.md (RM-0035) "Acceptance criteria", the MCP
//! half: `get_node` on the default build's stdio server over a scratch git
//! repository built at run time (an invented rules book: `docs/rules.md`
//! holding `{#R-12}` and `{#R-13}`; a hand commit creating both, an apply
//! raising R-12 written as `commit_message_raising` writes it, a hand
//! edit), and on a hub server over the same project, in memory.
//!
//! - NH6: `get_node {id, with: ["history"]}`'s content is `spec show REF
//!   --history 2>&1` and its `structuredContent` the `--json` document (the
//!   library twin), valid against the output schema; the same with all
//!   three `with` values, on the document, and in a `--depth 1` clone (the
//!   note in the content, `history: null`). The output schema's `history`
//!   holds exactly `{entries, truncated, omitted}` and an entry the 15
//!   keys, `source` and `change` closed enums; the input enum has three
//!   values. A hub answers alike and lists 18 tools. M: `rationale` dropped
//!   from `ShowHistoryEntry`.
//! - NH11, the texts: `get_node`'s description is 1 909 bytes and the
//!   hub's 2 000, ASCII, each with the new head and the history sentence
//!   once; `INSTRUCTIONS` and `HUB_INSTRUCTIONS` are the build before's.
//!   M: the sentence dropped. (`PINS`, `SCHEMA_PINS`, the version and
//!   `read-spec` are `plugin_files.rs`'s and `plugin_skills.rs`'s.)
//!
//! The CLI half is `specengine-cli`'s `tests/node_history.rs`; the size
//! (NH14) `mcp_size.rs`. Every server has a cleared environment and a
//! scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::blake3::blake3_hex;
use common::read::{
    Era, Session, assert_parity, cli_env, conforms, content_text, library, output_schemas,
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

/// The BLAKE3 of the build before's `INSTRUCTIONS` and `HUB_INSTRUCTIONS`
/// (card-context's pins, unchanged by it): NH11 keeps both byte-equal.
const INSTRUCTIONS_B3: &str = "7de2fdaa1885277df812c2e368a1fce532f8fc5aba6db0e6185ead5fbe87d351";
const HUB_INSTRUCTIONS_B3: &str =
    "a1762b8eee9cc29a6a6590e12a346f249927e106b01cfc9aea2b28be34734fa1";

/// NH11: the description's new head and its sentence, as the spec gives
/// them (NH11).
const NODE_HEAD: &str = "`spec show REF [--links] [--bindings] [--history] [--archive]`";
const NODE_SENTENCE: &str = "with: [\"history\"] lists the commits that changed each node's \
                             text, then the cards accepted on it and the items decided on it, \
                             newest first, at most 64, trailers as written; no git history \
                             gives a note and null, never an error.";

/// `HISTORY_MAX` as the spec fixes it.
const HISTORY_MAX: usize = 64;

/// The 15 keys of an entry (NH6, NH12), sorted.
const ENTRY_KEYS: [&str; 15] = [
    "at",
    "by",
    "by_name",
    "change",
    "created",
    "id",
    "proposal",
    "proposed_by",
    "proposer",
    "rationale",
    "record",
    "rev",
    "rev_before",
    "source",
    "title",
];

const CONFIG: &str = "[project]\nslug = \"quill-rules\"\n\n[paths]\nroots = [\"docs\"]\n\n\
                      [ids]\nR = { kind = \"requirement\", width = 2 }\n";

const RULES: &str = "docs/rules.md";

fn rules(window: &str) -> String {
    format!(
        "# Rules\n\nThe house rules.\n\n{window}\n\n## Door {{#R-13}}\n\nThe door shuts at dusk.\n"
    )
}

// ------------------------------------------------------------- the scene

/// A scratch git repository built here, its queue in `home`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    /// A hand commit creating both sections (Bob Hand, 2026-01-01), an
    /// apply of PR-0001 raising R-12 1 -> 2 decided by Ann Owner
    /// (2026-01-02), a hand edit of R-12 (2026-01-04).
    fn book(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.dir("repo");
        git.init(&root);
        let home = scratch.home("h");
        let repo = Self {
            scratch,
            git,
            root,
            home,
        };
        write(&repo.root, "specengine.toml", CONFIG);
        write(
            &repo.root,
            RULES,
            rules("## Window {#R-12}\n\nThe window opens at noon."),
        );
        let a = repo.commit(
            ("Bob Hand", "bob@example.org"),
            "2026-01-01T10:00:00+0000",
            "Add the rules\n",
        );
        write(
            &repo.root,
            RULES,
            rules("## Window {#R-12 rev=2}\n\nThe window opens at noon for ten minutes."),
        );
        repo.commit(
            ("Cleo Commit", "cleo@example.org"),
            "2026-01-02T10:00:00+0000",
            &format!(
                "spec: apply PR-0001\n\nTighten the window.\n\nProposal: PR-0001\nDecided-by: Ann \
                 Owner <ann@example.org>\nProposed-by: agent role=spec-writer model=m run=r\n\
                 Base-commit: {a}\nRevision: R-12 1 -> 2\n"
            ),
        );
        write(
            &repo.root,
            RULES,
            rules("## Window {#R-12 rev=2}\n\nThe window opens at noon, for ten minutes."),
        );
        repo.commit(
            ("Bob Hand", "bob@example.org"),
            "2026-01-04T10:00:00+0000",
            "Fix a typo in the window rule\n",
        );
        repo
    }

    fn commit(&self, (name, email): (&str, &str), date: &str, message: &str) -> String {
        let file = self.scratch.join("message.txt");
        fs::write(&file, message).expect("message");
        self.git.add_all(&self.root);
        let env: Vec<(&str, OsString)> = vec![
            ("GIT_AUTHOR_NAME", name.into()),
            ("GIT_AUTHOR_EMAIL", email.into()),
            ("GIT_AUTHOR_DATE", date.into()),
            ("GIT_COMMITTER_NAME", name.into()),
            ("GIT_COMMITTER_EMAIL", email.into()),
            ("GIT_COMMITTER_DATE", date.into()),
        ];
        let extra: Vec<(&str, &OsStr)> = env.iter().map(|(k, v)| (*k, v.as_os_str())).collect();
        self.git.git_env(
            &self.root,
            &[
                "commit",
                "-q",
                "--no-verify",
                "--cleanup=verbatim",
                "-F",
                file.to_str().expect("UTF-8"),
            ],
            &extra,
        );
        self.git.git_text(&self.root, &["rev-parse", "HEAD"])
    }

    fn env_at(&self, root: &Path, home: &Path) -> Env {
        cli_env(root, Some(home))
    }
}

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

/// The enum of a string schema (through `anyOf`), sorted.
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

/// `text` with every run of whitespace one space.
fn folded(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

// ------------------------------------------------------------------ NH6

/// NH6: content and `structuredContent` are the library twin's for
/// `with: ["history"]`, all three values, the document, and not asked;
/// the history holds the three commits, newest first.
#[test]
fn nh6_get_node_is_spec_show_with_history() {
    let repo = Repo::book("mcp-nh6");
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let own = repo.env_at(&repo.root, &repo.home);
    let calls = [
        json!({"id": "R-12", "with": ["history"]}),
        json!({"id": "R-12", "with": ["links", "bindings", "history"]}),
        json!({"id": RULES, "with": ["history"]}),
        json!({"id": "R-12"}),
        json!({"id": "R-13", "with": ["history"], "archive": false}),
    ];
    let mut replies = Vec::new();
    for args in calls {
        let reply = session.call("get_node", args.clone());
        let want = library("get_node", &args, &own, &Globals::default());
        assert_parity(
            &reply,
            &want,
            &schemas["get_node"],
            &format!("get_node {args}"),
        );
        replies.push(result(&reply).clone());
    }
    drop(session.finish());

    let history = &replies[0]["structuredContent"]["nodes"][0]["history"];
    let entries = history["entries"].as_array().expect("entries");
    let changes: Vec<&str> = entries
        .iter()
        .map(|entry| entry["change"].as_str().expect("change"))
        .collect();
    assert_eq!(changes, ["hand", "raised", "hand"], "{history}");
    assert_eq!(entries[1]["by_name"], json!("Ann Owner"));
    assert_eq!(entries[1]["proposal"], json!("PR-0001"));
    assert_eq!(entries[2]["created"], json!(true));
    assert_eq!(history["truncated"], json!(false));
    assert!(
        content_text(&replies[0]).contains("\n  history 3\n"),
        "{}",
        content_text(&replies[0])
    );
    assert_eq!(
        replies[1]["structuredContent"]["nodes"][0]["history"], *history,
        "the same history beside links and bindings"
    );
    let document_entries = replies[2]["structuredContent"]["nodes"][0]["history"]["entries"]
        .as_array()
        .expect("entries")
        .len();
    assert_eq!(document_entries, 3, "every commit of the file");
    assert_eq!(
        replies[3]["structuredContent"]["nodes"][0]["history"],
        Value::Null,
        "not asked"
    );
    assert!(!content_text(&replies[3]).contains("history"));
}

/// NH6 in NH5's clone: a `--depth 1` clone answers with the note in its
/// content and `history: null`, never `isError`, as the twin does.
#[test]
fn nh6_a_shallow_clone_is_a_note_over_mcp() {
    let repo = Repo::book("mcp-nh6-shallow");
    let clone = repo.scratch.join("clone");
    let url = format!("file://{}", repo.root.display());
    repo.git.git(
        repo.scratch.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            &url,
            clone.to_str().expect("UTF-8"),
        ],
    );
    let home = repo.scratch.home("clone");
    let mut session = Session::open(Era::Stateless, &[], Some(&clone), Home::At(&home));
    let schemas = output_schemas(&session.tools());
    let own = repo.env_at(&clone, &home);
    for args in [
        json!({"id": "R-12", "with": ["history"]}),
        json!({"id": "R-12", "with": ["links", "bindings", "history"]}),
    ] {
        let reply = session.call("get_node", args.clone());
        let want = library("get_node", &args, &own, &Globals::default());
        assert_parity(&reply, &want, &schemas["get_node"], &format!("{args}"));
        let answer = result(&reply);
        assert_ne!(answer["isError"], json!(true), "{answer}");
        assert_eq!(
            answer["structuredContent"]["nodes"][0]["history"],
            Value::Null
        );
        assert!(
            content_text(answer)
                .contains("note: history: docs/rules.md: not read: a shallow clone\n"),
            "{}",
            content_text(answer)
        );
    }
    drop(session.finish());
}

/// NH6: the output schema's `history` holds exactly `{entries, omitted,
/// truncated}`, an entry the 15 keys, `source` and `change` closed; the
/// input's `with` enum the three values. M: `rationale` dropped from
/// `ShowHistoryEntry`.
#[test]
fn nh6_the_schemas_hold_the_history_keys() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    drop(session.finish());
    let get = tool(&list, "get_node");
    let node = object(&get["outputSchema"]["properties"]["nodes"]);
    let history = &node["properties"]["history"];
    assert_eq!(
        properties(history),
        ["entries", "omitted", "truncated"],
        "{history}"
    );
    let shape = object(history);
    let entry = object(&shape["properties"]["entries"]);
    assert_eq!(
        properties(entry),
        ENTRY_KEYS,
        "M: rationale dropped: {entry}"
    );
    assert_eq!(
        enum_of(&entry["properties"]["source"]),
        ["card", "commit", "item"],
        "source closed"
    );
    assert_eq!(
        enum_of(&entry["properties"]["change"]),
        [
            "accepted",
            "applied",
            "decision",
            "discrepancy",
            "editorial",
            "hand",
            "question",
            "raised",
        ],
        "change closed"
    );
    let required: Vec<&str> = node["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(required.contains(&"history"), "{required:?}");

    let with = &get["inputSchema"]["properties"]["with"];
    let items = if with["items"].is_object() {
        with["items"].clone()
    } else {
        with["anyOf"]
            .as_array()
            .and_then(|variants| variants.iter().find(|v| v["items"].is_object()))
            .map(|variant| variant["items"].clone())
            .unwrap_or_else(|| panic!("with's items: {with}"))
    };
    assert_eq!(
        items,
        json!({"type": "string", "enum": ["links", "bindings", "history"]}),
        "the inlined enum"
    );
    let doc = with["description"].as_str().expect("with's description");
    assert!(
        doc.contains("[\\\"history\\\"]") || doc.contains("[\"history\"]"),
        "{doc}"
    );
    assert!(doc.contains("each node's changes, newest first."), "{doc}");

    // The walker's exact key sets (`docs/canon/mcp-read.md` "Tools", its
    // schemas paragraph): a whole entry conforms, one with `rationale`
    // absent does not.
    let mut broken = json!({"source": "commit", "at": "2026-01-02T10:00:00Z",
        "id": "0".repeat(40), "change": "raised", "created": false, "rev": 2,
        "rev_before": 1, "by": "Ann Owner <ann@example.org>", "by_name": "Ann Owner",
        "proposal": "PR-0001", "proposed_by": "agent role=spec-writer model=m run=r",
        "proposer": "agent spec-writer", "record": null, "title": null,
        "rationale": "Tighten the window."});
    conforms(entry, &broken, "$").unwrap_or_else(|problem| panic!("a whole entry: {problem}"));
    broken
        .as_object_mut()
        .expect("an object")
        .remove("rationale");
    assert!(conforms(entry, &broken, "$").is_err(), "rationale required");
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

/// The hub server's answers to `get_node` `calls` (`tools/list` first).
async fn hub_answers(repo: &Repo, calls: &[Value]) -> Vec<Value> {
    let seam = Mira {
        env: repo.env_at(&repo.root, &repo.home),
        globals: Globals {
            root: Some(repo.root.clone()),
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
            "method": "tools/call", "params": {"name": "get_node", "arguments": arguments}})));
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

/// NH6, the hub: `get_node` with `history` answers on a hub as on stdio,
/// content and `structuredContent`; `tools/list` holds 18 tools; NH11, the
/// hub's `get_node` description is 2 000 bytes with the sentence once and
/// `HUB_INSTRUCTIONS` the build before's.
#[tokio::test]
async fn nh6_a_hub_answers_alike_and_nh11_its_texts() {
    let repo = Repo::book("mcp-nh6-hub");
    let calls = [
        json!({"id": "R-12", "with": ["history"]}),
        json!({"id": "R-12", "with": ["links", "bindings", "history"]}),
        json!({"id": "R-12"}),
    ];
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    let stdio: Vec<Value> = calls
        .iter()
        .map(|args| result(&session.call("get_node", args.clone())).clone())
        .collect();
    drop(session.finish());
    let hub = hub_answers(&repo, &calls).await;
    let listed = tool_names(&hub[0]);
    assert_eq!(listed.len(), 18, "{listed:?}");
    assert_eq!(HUB_TOOLS.len(), 18);
    for (n, (args, local)) in calls.iter().zip(&stdio).enumerate() {
        let remote = &hub[n + 1];
        assert_ne!(remote["isError"], json!(true), "{args}: {remote}");
        assert_eq!(
            remote["structuredContent"], local["structuredContent"],
            "{args}"
        );
        assert_eq!(content_text(remote), content_text(local), "{args}");
    }
    assert_ne!(
        hub[1]["structuredContent"]["nodes"][0]["history"],
        Value::Null,
        "the hub reads the history"
    );
    let text = tool(&hub[0], "get_node")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    assert!(text.is_ascii());
    assert_eq!(text.len(), 2000, "the hub's get_node description: {text}");
    assert!(text.len() <= 2048);
    let flat = folded(&text);
    assert!(flat.contains(NODE_HEAD), "{text}");
    assert_eq!(flat.matches(NODE_SENTENCE).count(), 1, "{text}");
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
}

// ----------------------------------------------------------------- NH11

/// NH11: stdio `get_node`'s description is 1 909 bytes of ASCII with the
/// new head, and the sentence once, before `archive:`, "at most 64" the
/// spec's `HISTORY_MAX`; `INSTRUCTIONS` are the build before's. M: the
/// sentence dropped.
#[test]
fn nh11_the_description_holds_the_history_sentence() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    drop(session.finish());
    let text = tool(&list, "get_node")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    assert!(text.is_ascii());
    assert_eq!(text.len(), 1909, "{text}");
    let flat = folded(&text);
    assert!(
        flat.contains(&format!("The same as {NODE_HEAD}:")),
        "{text}"
    );
    assert_eq!(
        flat.matches(NODE_SENTENCE).count(),
        1,
        "M: the sentence dropped: {text}"
    );
    assert!(
        flat.contains(&format!("{NODE_SENTENCE} archive:")),
        "the sentence before `archive:`: {text}"
    );
    assert!(NODE_SENTENCE.contains(&format!("at most {HISTORY_MAX},")));
    assert_eq!(INSTRUCTIONS.len(), 2048);
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
}
