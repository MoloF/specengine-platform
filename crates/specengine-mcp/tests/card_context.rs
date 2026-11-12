//! docs/features/card-context.md (RM-0108) "Acceptance criteria", the MCP
//! half: `get_card` on the default build's stdio server over a scratch git
//! repository of `fixtures/spec-a` (a game, `[decision_records]`), its queue
//! made through the library at fixed clocks (the CLI half's scene: `C-0001`
//! with two targets, a decision applied with its record `DEC-0024`, an open
//! question and a rejected decision), and on a hub server over the same
//! project, in memory.
//!
//! - CC2: `get_card {card_id}` is the read before plus `context: -` and
//!   `"context": null`; `context: false` and `null` read alike.
//! - CC4: a gone target is a note on an answer, never `isError`; `budget`
//!   without `context` is `isError` with the library's line, no
//!   `structuredContent`.
//! - CC7: each call's content is `spec card show C-0001 --marks …`'s `2>&1`
//!   and its `structuredContent` the `--json` document (the library twin),
//!   valid against the output schema, whose `context` holds exactly
//!   `{parent, bundle, notes}`, `parent` `{id, title, goal}`, `bundle` the
//!   bundle document's 13; the input schema `card_id`, `history`, `context`,
//!   `budget`, closed. A hub answers the same call alike, lists 18 tools,
//!   and its `HUB_INSTRUCTIONS` are 3892f03's. M: `notes` dropped from
//!   `CardContextShape`; `context` ignored on the hub's route.
//! - CC9: `get_card`'s description is 1 987 bytes (2 041 since
//!   docs/features/upstream-signal.md US-11), ASCII, with the spec's
//!   head and sentence; the stdio `INSTRUCTIONS` are 3892f03's. M: the
//!   sentence dropped.
//!
//! The CLI half and the text's arithmetic are `specengine-cli`'s
//! `tests/card_context.rs`; the size (CC11) `mcp_size.rs`. Every server has
//! a cleared environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::VecDeque;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::blake3::blake3_hex;
use common::read::{
    Era, Session, assert_parity, card_library, cli_env, conforms, content_text, output_schemas,
};
use common::*;
use git::Sandbox;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CardNewRequest, DecisionInput, DecisionRequest, Env, Exit,
    GitEnv, Globals, IntakeOutcome, QuestionRequest, RejectRequest, approve_with, card_new,
    propose_decision, propose_question, reject,
};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer, TurnFuture,
};

const CLOCK: &str = "2026-11-12T09:00:00Z";
const RAISED: &str = "2026-11-12T08:00:00Z";
const DECIDED: &str = "2026-11-12T10:00:00Z";

const T1: &str = "MEC-SPRINT";
const T2: &str = "MEC-STAMINA";
const T2_FILE: &str = "docs/spec/movement/stamina.md";
const R1: &str = "DEC-0024";
const TITLE: &str = "Write the stamina foreword";
const G1: &str = "Readers learn stamina before they sprint.";

/// The BLAKE3 of 3892f03's `INSTRUCTIONS` and `HUB_INSTRUCTIONS`: CC7 and
/// CC9 keep both byte-equal (the stdio text was compared with 3892f03's
/// `specengine-mcp` `initialize` answer when this pin was written).
// Re-pinned by docs/features/collision-pass.md "Acceptance criteria", CP8
// (the collisions line in, the resources sentence out; `mcp_code.rs`' RD-5
// steps back to the value this slice pinned, `7de2fdaa…`).
const INSTRUCTIONS_B3: &str = "72797a127dbf98ef66644c92d46870dacb4c072d9cc10819654c38fd82d95fe4";
const HUB_INSTRUCTIONS_B3: &str =
    "a1762b8eee9cc29a6a6590e12a346f249927e106b01cfc9aea2b28be34734fa1";

/// CC9: the description's head and its sentence, as the spec gives them.
const GET_HEAD: &str = "One card: the same as `spec card show C --marks [--history] [--context \
                        [--budget N]]`.";
const GET_CONTEXT: &str = "context: true: {parent {id, title, goal} or null, bundle, notes}: \
                           bundle is get_context_bundle's document over its targets, then the \
                           records of its applied items, at budget (else the project's \
                           bundle_task, else 10000), fitted into the room its lines leave of \
                           the 40000 characters, its body printed after them; a bundle that \
                           cannot be made is null with a note, never an error. budget needs \
                           context. Without context: null.";

/// CC4: the library's line of `budget` without `context`.
const NEEDS_CONTEXT: &str = "`--budget` needs `--context`";

/// The bundle document's 13 keys (docs/canon/spec-cli-bundle.md "Output").
const BUNDLE_KEYS: [&str; 13] = [
    "body",
    "budget",
    "bundle_hash",
    "bytes",
    "chars",
    "layers",
    "more",
    "notes",
    "reason",
    "refs",
    "tail",
    "task",
    "tokens",
];

// ------------------------------------------------------------- the scene

/// A scratch git repository of spec-a, committed, its queue in `home`.
struct Repo {
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("spec-a", "repo");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            git,
            root,
            home,
        }
    }

    fn env(&self) -> Env {
        cli_env(&self.root, Some(&self.home))
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(self.root.clone(), self.git.vars())
    }

    fn session(&self) -> Session {
        Session::open(Era::Stateless, &[], Some(&self.root), Home::At(&self.home))
    }

    /// CC1's scene: `C-0001` (T1, T2, G1); `PR-0001` a decision approved as
    /// [`R1`], `PR-0002` an open question, `PR-0003` a rejected decision,
    /// each naming `C-0001`.
    fn foreword(label: &str) -> Self {
        let repo = Self::new(label);
        let made = card_new(
            &repo.env(),
            &Globals::default(),
            &CardNewRequest {
                title: TITLE.to_owned(),
                goal: Some(G1.to_owned()),
                discipline: None,
                targets: vec![T1.to_owned(), T2.to_owned()],
                criteria: vec!["One page, read before the first sprint.".to_owned()],
                assignee: None,
                reviewer: None,
                mandate: None,
                estimate: None,
                due: None,
                parent: None,
                corrects: None,
                actor: None,
                now: CLOCK.to_owned(),
            },
        )
        .expect("card new");
        assert_eq!(made.id.as_deref(), Some("C-0001"), "{made:?}");
        let first = repo.decision(
            "RULE-STAM-REGEN",
            "Does the foreword name the regeneration delay?",
        );
        repo.approve(&first);
        let open = repo.question("EDGE-STAM-ZERO", "Does the foreword show an empty tank?");
        let gone = repo.decision(
            "EDGE-SPRINT-EMPTY",
            "Does the foreword quote the sprint table?",
        );
        assert_eq!([&first, &open, &gone], ["PR-0001", "PR-0002", "PR-0003"]);
        let mut yes = |_: &str| true;
        let rejected = reject(
            &repo.env(),
            &Globals::default(),
            &RejectRequest {
                id: gone.clone(),
                reason: Some("Not for this foreword.".to_owned()),
                now: DECIDED.to_owned(),
                git: repo.git_env(),
            },
            &mut yes,
        )
        .expect("reject");
        assert_eq!(rejected.exit(), Exit::Answered, "{rejected:?}");
        repo
    }

    fn decision(&self, node: &str, text: &str) -> String {
        let mut request = DecisionRequest {
            input: DecisionInput {
                node_ids: vec![node.to_owned()],
                text: text.to_owned(),
                working_answer: format!("Yes: {text}"),
                price_of_other: "A page rewritten.".to_owned(),
                why: None,
                severity: None,
                proposed_patch: None,
                distinct_from: None,
            },
            card: Some("C-0001".to_owned()),
            author_role: Some("developer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            login: None,
            now: RAISED.to_owned(),
            git: self.git_env(),
        };
        let raise = |request: &DecisionRequest| {
            propose_decision(&self.env(), &Globals::default(), request).expect("a decision")
        };
        let first = raise(&request);
        if first.document.created {
            return stored(&first);
        }
        request.input.distinct_from = Some(hits(&first));
        stored(&raise(&request))
    }

    fn question(&self, node: &str, text: &str) -> String {
        let mut request = QuestionRequest {
            node_ids: vec![node.to_owned()],
            text: text.to_owned(),
            working_answer: "Yes, at once.".to_owned(),
            price_of_other: "A new edge case.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: Some("C-0001".to_owned()),
            author_role: Some("developer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            login: None,
            now: RAISED.to_owned(),
            git: self.git_env(),
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
        };
        let ask = |request: &QuestionRequest| {
            propose_question(&self.env(), &Globals::default(), request).expect("a question")
        };
        let first = ask(&request);
        if first.document.created {
            return stored(&first);
        }
        request.distinct_from = hits(&first);
        stored(&ask(&request))
    }

    /// Approves `id` with its working answer: one record, committed.
    fn approve(&self, id: &str) {
        let mut yes = |_: &str| true;
        let outcome = approve_with(
            &self.env(),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: DECIDED.to_owned(),
                git: self.git_env(),
            },
            &ApproveFlags {
                option: None,
                answer: None,
                canon: None,
            },
            &mut yes,
        )
        .expect("approve");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }
}

fn stored(outcome: &IntakeOutcome) -> String {
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "{outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

fn hits(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect()
}

/// The object schema's property names under `schema`, through `anyOf`.
fn properties(schema: &Value) -> Vec<String> {
    if let Some(properties) = schema["properties"].as_object() {
        return properties.keys().cloned().collect();
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

/// The object schema under `schema` (through `anyOf`).
fn object(schema: &Value) -> &Value {
    if schema["properties"].is_object() {
        return schema;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array() {
            for variant in variants {
                if variant["properties"].is_object() {
                    return variant;
                }
            }
        }
    }
    panic!("no object schema: {schema}")
}

// ------------------------------------------------------------- CC2, CC7

/// CC7: `get_card {card_id, context: true, budget: 3000}`, with `history:
/// true`, and without `budget`: content and `structuredContent` the library
/// twin's (`spec card show C-0001 --marks --context … 2>&1`, `--json`),
/// valid against the output schema; the bundle's REFs `[T1, T2, R1]`. CC2:
/// `{card_id}`, `context: false` and `context: null` alike, the last line
/// `context: -`, `"context": null`. M: `notes` dropped from
/// `CardContextShape`.
#[test]
fn cc7_get_card_is_the_cli_read_with_context() {
    let repo = Repo::foreword("mcp-cc7");
    let mut session = repo.session();
    let schemas = output_schemas(&session.tools());
    let own = repo.env();
    let calls = [
        json!({"card_id": "C-0001", "context": true, "budget": 3000}),
        json!({"card_id": "C-0001", "context": true, "budget": 3000, "history": true}),
        json!({"card_id": "C-0001", "context": true}),
        json!({"card_id": "C-0001"}),
        json!({"card_id": "C-0001", "context": false}),
        json!({"card_id": "C-0001", "context": null}),
        json!({"card_id": "C-0001", "history": true}),
    ];
    let mut replies = Vec::new();
    for args in calls {
        let reply = session.call("get_card", args.clone());
        let want = card_library("get_card", &args, &own, &Globals::default(), CLOCK);
        assert_parity(
            &reply,
            &want,
            &schemas["get_card"],
            &format!("get_card {args}"),
        );
        replies.push(result(&reply).clone());
    }
    let asked = &replies[0];
    let document = &asked["structuredContent"];
    assert_eq!(
        document["context"]["bundle"]["refs"],
        json!([T1, T2, R1]),
        "{document}"
    );
    assert_eq!(document["context"]["bundle"]["budget"], json!(3000));
    assert_eq!(
        replies[2]["structuredContent"]["context"]["bundle"]["budget"],
        json!(10000)
    );
    let text = content_text(asked);
    let hash = document["context"]["bundle"]["bundle_hash"]
        .as_str()
        .expect("a hash");
    let body = document["context"]["bundle"]["body"]
        .as_str()
        .expect("a body");
    assert!(
        text.contains(&format!(
            "history: -\ncontext.parent: -\ncontext.parent.goal: -\ncontext.bundle: {hash}\n{body}"
        )),
        "{text}"
    );
    if let Err(problem) = conforms(&schemas["get_card"], document, "$") {
        panic!("M: a CLI key without its mirror: {problem}");
    }
    // CC2: not asked.
    let plain = &replies[3];
    assert!(
        content_text(plain).ends_with("\nhistory: -\ncontext: -\n"),
        "{}",
        content_text(plain)
    );
    assert_eq!(plain["structuredContent"]["context"], Value::Null);
    assert_eq!(replies[4], *plain, "context: false reads as absent");
    assert_eq!(replies[5], *plain, "context: null reads as absent");
    assert_eq!(
        replies[6]["structuredContent"]["context"],
        Value::Null,
        "history alone asks no context"
    );
    assert_eq!(
        repo.git
            .git_text(&repo.root, &["status", "--porcelain", "--ignored"]),
        "",
        "nothing written under the root"
    );
    drop(session.finish());
}

/// CC7: the output schema's `context` holds exactly `{parent, bundle,
/// notes}`, `parent` `{id, title, goal}`, `bundle` the bundle document's
/// 13; the input schema `budget`, `card_id`, `context`, `history`, closed,
/// `card_id` alone required. M: `notes` dropped from `CardContextShape`.
#[test]
fn cc7_the_schemas_hold_the_context_keys() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let get = tool(&list, "get_card");
    let output = &get["outputSchema"];
    let context = &output["properties"]["context"];
    let mut keys = properties(context);
    keys.sort();
    assert_eq!(keys, ["bundle", "notes", "parent"], "M: notes dropped");
    let shape = object(context);
    let mut parent = properties(&shape["properties"]["parent"]);
    parent.sort();
    assert_eq!(parent, ["goal", "id", "title"]);
    let mut bundle = properties(&shape["properties"]["bundle"]);
    bundle.sort();
    assert_eq!(bundle, BUNDLE_KEYS);
    // `context` sits between `history` and `notes` in the document, and the
    // schema requires it.
    let required: Vec<&str> = output["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(required.contains(&"context"), "{required:?}");

    let input = &get["inputSchema"];
    assert_eq!(input["additionalProperties"], json!(false), "{input}");
    let mut taken = properties(input);
    taken.sort();
    assert_eq!(taken, ["budget", "card_id", "context", "history"]);
    assert_eq!(input["required"], json!(["card_id"]));
    drop(session.finish());
}

// ------------------------------------------------------------------ CC4

/// CC4 (a) over MCP: T2's file deleted: an answer, not `isError`, the
/// card's document with `bundle` `null` and the one note naming T2, the
/// content the library twin's. CC4: `budget` without `context` (absent,
/// `false`) is `isError` with the library's line and no
/// `structuredContent`; `budget: 0` with `context` the range line.
#[test]
fn cc4_a_gone_target_is_a_note_and_budget_needs_context() {
    let repo = Repo::foreword("mcp-cc4");
    fs::remove_file(repo.root.join(T2_FILE)).expect("remove T2's file");
    let mut session = repo.session();
    let schemas = output_schemas(&session.tools());
    let own = repo.env();
    let args = json!({"card_id": "C-0001", "context": true});
    let reply = session.call("get_card", args.clone());
    let want = card_library("get_card", &args, &own, &Globals::default(), CLOCK);
    assert_parity(&reply, &want, &schemas["get_card"], "a gone target");
    let answer = result(&reply);
    assert_ne!(answer["isError"], json!(true), "{answer}");
    let context = &answer["structuredContent"]["context"];
    assert_eq!(context["bundle"], Value::Null, "{context}");
    let notes = context["notes"].as_array().expect("notes");
    assert_eq!(notes.len(), 1, "{context}");
    let note = notes[0].as_str().expect("a note");
    assert!(
        note.starts_with("no bundle: ") && note.contains(T2),
        "{note}"
    );
    assert!(
        content_text(answer).contains(&format!("note: context: {note}\n")),
        "{}",
        content_text(answer)
    );

    for args in [
        json!({"card_id": "C-0001", "budget": 5}),
        json!({"card_id": "C-0001", "context": false, "budget": 5}),
    ] {
        let reply = session.call("get_card", args.clone());
        let want = card_library("get_card", &args, &own, &Globals::default(), CLOCK);
        assert_parity(&reply, &want, &schemas["get_card"], &format!("{args}"));
        let answer = result(&reply);
        assert_eq!(answer["isError"], json!(true), "{args}: {answer}");
        assert!(answer.get("structuredContent").is_none(), "{answer}");
        assert!(
            content_text(answer).contains(NEEDS_CONTEXT),
            "{args}: {}",
            content_text(answer)
        );
    }
    let args = json!({"card_id": "C-0001", "context": true, "budget": 0});
    let reply = session.call("get_card", args.clone());
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(
        content_text(answer).contains(
            "--budget 0: the budget is a whole number of estimated tokens from 1 to 4294967295"
        ),
        "{}",
        content_text(answer)
    );
    drop(session.finish());
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

/// The hub server's answers to `calls` (`tools/list` first), by request.
async fn hub_answers(repo: &Repo, calls: &[Value]) -> Vec<Value> {
    let seam = Mira {
        env: repo.env(),
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
            "method": "tools/call", "params": {"name": "get_card", "arguments": arguments}})));
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

/// CC7, the hub: `get_card {card_id, context: true, budget: 3000}` and with
/// `history: true` answer on a hub as on stdio, content and
/// `structuredContent` (the card links no task, so the hub's and the
/// terminal's reads agree); `tools/list` holds the 18 tools; the hub's and
/// the stdio texts are 3892f03's. M: `context` ignored on the hub's route.
#[tokio::test]
async fn cc7_a_hub_answers_alike_and_keeps_its_texts() {
    let repo = Repo::foreword("mcp-cc7-hub");
    let calls = [
        json!({"card_id": "C-0001", "context": true, "budget": 3000}),
        json!({"card_id": "C-0001", "context": true, "budget": 3000, "history": true}),
        json!({"card_id": "C-0001"}),
    ];
    let mut session = repo.session();
    let stdio: Vec<Value> = calls
        .iter()
        .map(|args| result(&session.call("get_card", args.clone())).clone())
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
            "M: context ignored on the hub's route: {args}"
        );
        assert_eq!(content_text(remote), content_text(local), "{args}");
    }
    assert_ne!(
        hub[1]["structuredContent"]["context"],
        Value::Null,
        "the hub builds the context"
    );
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
}

// ------------------------------------------------------------------ CC9

/// `text` with every run of whitespace one space.
fn folded(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// CC9: `get_card`'s description is 1 987 bytes of ASCII (2 041 since
/// docs/features/upstream-signal.md US-11, 2 047 since
/// docs/features/plain-questions.md PQ10, the items' `gist` and `matters`),
/// starts with the spec's head,
/// and holds its sentence once, after "in one read." (and, since
/// upstream-signal, its `upstream` sentence) and before "Reads only:"; the stdio `INSTRUCTIONS` are 3892f03's (2 048
/// bytes). The two numbers are tied by compile-time asserts in `src/`. M:
/// the sentence dropped.
#[test]
fn cc9_the_description_holds_the_context_sentence() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let text = tool(&list, "get_card")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    drop(session.finish());
    assert!(text.is_ascii());
    assert_eq!(text.len(), 2047, "{text}");
    assert!(folded(&text).starts_with(GET_HEAD), "{text}");
    let folded_text = folded(&text);
    assert_eq!(
        folded_text.matches(GET_CONTEXT).count(),
        1,
        "M: the sentence dropped: {text}"
    );
    assert!(
        folded_text.contains(&format!(
            "in one read. upstream: what changed above an open subtask, as list_cards names it, \
             or null. {GET_CONTEXT} Reads only:"
        )),
        "{text}"
    );
    // 2 039 since docs/features/collision-pass.md CP8.
    assert_eq!(INSTRUCTIONS.len(), 2039);
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
    assert!(!INSTRUCTIONS.contains("context: true"));
}
