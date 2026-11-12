//! docs/features/plain-questions.md (RM-0088, the question half)
//! "Acceptance criteria", the MCP crate's half: the real `specengine-mcp`
//! binary over stdio on scratch git copies of `fixtures/spec-a`, its
//! library twin (`spec propose question …`: `content` its `2>&1`,
//! `structuredContent` its `--json`) in another `HOME`, and a hub server
//! over a seam that runs every turn for real, in memory (as
//! `intake_similar.rs`'s). The form's half of PQ7 is in `mcp_review_form.rs`
//! (its `Repo` and `Client`).
//!
//! The "Data" example names its hit DEC-0023 in `distinct_from`: spec-a's
//! accepted decision answers both targets (the CLI's `plain_questions.rs`).
//!
//! - PQ1, the MCP part: `ask_question` with the four fields stores
//!   `PR-0001`; `get_proposal` returns `gist`, `matters`, both options and
//!   `recommendation` 0 byte-equal, conforming to its output schema.
//! - PQ2, the MCP part: each cap is an error result (`isError`) with the
//!   CLI's line and the document, as its twin; nothing stored. M: 4
//!   options accepted.
//! - PQ9: the call's content byte-equal to the twin's `2>&1`,
//!   `structuredContent` to its `--json`, the note included; the input
//!   schema's four nullable keys in "MCP"'s words (no digit), `options`'
//!   items closed; the outputs' `gist` and `matters`; the descriptions with
//!   "MCP"'s six edits (2 031 and 2 044 bytes, ASCII, the numbers 160, 512,
//!   2 and 3 present); `INSTRUCTIONS` and `HUB_INSTRUCTIONS` byte-equal to
//!   before; 30 tools on stdio, 18 on a hub, whose `ask_question` stores
//!   the fields. M: `gist` dropped from the mirror; the hub's args ignoring
//!   `options`.
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
use common::read::{
    Era, Expected, Session, cli_env, conforms, content_text, expected, output_schemas,
};
use common::*;
use git::Sandbox;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{
    Env, GIST_MAX, GitEnv, Globals, IntakeOption, MATTERS_MAX, OPTIONS_MIN, Outcome,
    QUESTION_OPTIONS_MAX, QuestionRequest, propose_question, utc_now,
};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer, TurnFuture,
};

/// The BLAKE3 of the stdio `INSTRUCTIONS` and `HUB_INSTRUCTIONS` before this
/// slice (`intake_similar.rs`'s pins, unchanged since 3892f03).
// Re-pinned by docs/features/collision-pass.md "Acceptance criteria", CP8
// (the collisions line in, the resources sentence out; `mcp_code.rs`' RD-5
// steps back to the value this slice pinned, `7de2fdaa…`).
const INSTRUCTIONS_B3: &str = "72797a127dbf98ef66644c92d46870dacb4c072d9cc10819654c38fd82d95fe4";
const HUB_INSTRUCTIONS_B3: &str =
    "a1762b8eee9cc29a6a6590e12a346f249927e106b01cfc9aea2b28be34734fa1";

/// The "Data" example's fields.
const TEXT: &str =
    "Does RULE-STAM-REGEN's delay restart when stamina_regen() is called mid-sprint?";
const WORKING: &str = "yes, the 1.5 s delay restarts";
const PRICE: &str = "R-28 rebalanced";
const GIST: &str = "Should resting restart the wait before stamina refills?";
const MATTERS: &str = "Sprint tuning and two tasks wait on it.";

/// "MCP"'s property docs.
const GIST_DOC: &str =
    "The question in one line of plain words for the owner: no IDs, paths or code names.";
const MATTERS_DOC: &str = "Why it matters to the owner, in plain words.";
const OPTIONS_DOC: &str =
    "Two or three answers the owner may pick, each with its effect and price.";
const RECOMMENDATION_DOC: &str = "The index of the option you recommend, from 0.";

/// "MCP"'s edits shared by both descriptions: `(before, after)`; an empty
/// `before` is an insertion.
const EDITS: [(&str, &str); 4] = [
    (
        "[--severity S] [--distinct-from X]",
        "[--severity S] [--gist G] [--matters M] [--options F] [--recommendation N] \
         [--distinct-from X]",
    ),
    (
        "node_ids: 1 to 16 IDs (or slug/ID) it is about, or root-relative .md paths naming their \
         files' documents, each node once;",
        "node_ids: 1 to 16 IDs (or slug/ID) or root-relative .md paths it is about, each once;",
    ),
    (
        "A hit: an accepted decision document linked to a node (its id, path and title as the \
         answer), or",
        "A hit: an accepted decision linked to a node (its id, path, title as the answer), or",
    ),
    (
        " Related: decision documents only mentioning a node, queued items with other text.",
        "",
    ),
];

/// Edit 2: the line after `text:`, stdio's and a hub's.
const STDIO_LEAD: &str = "text: the question, at most 1024 bytes.\ngist (160 bytes), matters \
                          (512), options (2-3) and recommendation: what the owner reads first; \
                          plain words, no IDs, paths or code names.\n";
const HUB_LEAD: &str = "text: the question, at most 1024 bytes.\ngist (160 bytes), matters (512), \
                        options (2-3) and recommendation: what a person reads first; plain \
                        words, no IDs, paths or code names.\n";

/// The example's two options.
fn options() -> Value {
    json!([{"label": "Restart the wait", "effect": "Short rests do not refill",
        "price": "none: as built"},
        {"label": "Keep the wait running", "effect": "Tapping rest refills faster",
        "price": "R-28 rebalanced"}])
}

/// The "Data" example as `ask_question`'s arguments, its hit named.
fn example() -> Value {
    json!({"node_ids": ["MEC-STAMINA", "RULE-STAM-REGEN"], "text": TEXT,
        "working_answer": WORKING, "price_of_other": PRICE, "gist": GIST, "matters": MATTERS,
        "options": options(), "recommendation": 0, "distinct_from": ["DEC-0023"],
        "author_role": "developer"})
}

/// A scratch git repository of spec-a: its main worktree (one commit on
/// `main`), a linked worktree on `t1`, a data `HOME`.
struct Repo {
    scratch: Scratch,
    _git: Sandbox,
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
            _git: git,
            main,
            linked,
            home,
        }
    }

    fn session_in(&self, cwd: &Path, home: &Path) -> Session {
        Session::open(Era::Stateless, &[], Some(cwd), Home::At(home))
    }

    fn session(&self) -> Session {
        self.session_in(&self.linked, &self.home)
    }

    /// The twin of an `ask_question` call: the library in `t1` with `HOME`
    /// `home`.
    fn twin(&self, args: &Value, home: &Path) -> Expected {
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
        let request = QuestionRequest {
            node_ids: list("node_ids"),
            text: text("text").expect("a text"),
            working_answer: text("working_answer").expect("a working answer"),
            price_of_other: text("price_of_other").expect("a price"),
            gist: text("gist"),
            matters: text("matters"),
            options: (!args["options"].is_null()).then(|| {
                serde_json::from_value::<Vec<IntakeOption>>(args["options"].clone())
                    .expect("options")
            }),
            recommendation: args["recommendation"].as_u64(),
            severity: None,
            distinct_from: list("distinct_from"),
            card: None,
            author_role: text("author_role"),
            author_model: None,
            run: None,
            login: None,
            now: utc_now(),
            git: GitEnv::new(&self.linked, [("HOME", home.as_os_str())]),
        };
        expected(
            propose_question(
                &cli_env(&self.linked, Some(home)),
                &Globals::default(),
                &request,
            )
            .map(|outcome| Outcome::Intake(Box::new(outcome))),
        )
    }
}

/// The call's result, compared with its twin: content and
/// `structuredContent`, `isError`.
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

/// A tool's input schema in a `tools/list` answer.
fn input_schema<'a>(list: &'a Value, name: &str) -> &'a Value {
    &tool(list, name)["inputSchema"]
}

// ------------------------------------------------------------------ PQ1

/// PQ1, PQ9: the example over stdio answers as its twin (`PR-0001`, its
/// hit), `get_proposal` returns the four fields byte-equal, conforming;
/// a gist that reads as code gets the note in both. M: `gist` dropped from
/// the mirror.
#[test]
fn pq1_pq9_ask_question_answers_as_its_twin_and_get_proposal_returns_the_fields() {
    let repo = Repo::new("mpq-pq1");
    let twin_home = repo.scratch.home("twin");
    let mut session = repo.session();
    let list = session.tools();
    // 32 since docs/features/collision-pass.md CP8.
    assert_eq!(tool_names(&list).len(), 32, "stdio keeps 32 tools");
    let schemas = output_schemas(&list);

    let reply = session.call("ask_question", example());
    let want = repo.twin(&example(), &twin_home);
    assert_twin(&reply, &want, "the example");
    let document = &result(&reply)["structuredContent"];
    assert_eq!(document["id"], json!("PR-0001"), "{reply}");
    if let Err(problem) = conforms(&schemas["ask_question"], document, "$") {
        panic!("ask_question breaks its outputSchema: {problem}");
    }

    let read = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
    let proposal = &result(&read)["structuredContent"];
    assert_eq!(proposal["gist"], json!(GIST), "{read}");
    assert_eq!(proposal["matters"], json!(MATTERS));
    assert_eq!(proposal["options"], options());
    assert_eq!(proposal["recommendation"], json!(0));
    assert_eq!(proposal["summary"], json!(TEXT));
    if let Err(problem) = conforms(&schemas["get_proposal"], proposal, "$") {
        panic!("get_proposal breaks its outputSchema: {problem}");
    }
    let shown = content_text(result(&read));
    assert!(
        shown.contains(&format!("\ngist: {GIST}\nmatters: {MATTERS}\n")),
        "{shown}"
    );

    // A gist naming an ID: the note, as the twin's, last.
    let mut coded = example();
    coded["text"] = json!("Does the wait restart on a short rest?");
    coded["gist"] = json!("Does MEC-STAMINA restart its wait?");
    let reply = session.call("ask_question", coded.clone());
    let want = repo.twin(&coded, &twin_home);
    assert_twin(&reply, &want, "a coded gist");
    assert_eq!(
        result(&reply)["structuredContent"]["notes"]
            .as_array()
            .and_then(|notes| notes.last()),
        Some(&json!(
            "gist: `MEC-STAMINA` reads as code to the owner: say it in plain words; IDs belong \
             in node_ids"
        ))
    );

    // Without the fields: as before, `get_proposal`'s two keys null.
    let bare = json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Does the lamp stay lit?",
        "working_answer": "yes", "price_of_other": "a rule", "author_role": "developer"});
    let reply = session.call("ask_question", bare.clone());
    assert_twin(&reply, &repo.twin(&bare, &twin_home), "bare");
    let id = result(&reply)["structuredContent"]["id"].clone();
    let read = session.call("get_proposal", json!({"proposal_id": id}));
    let proposal = &result(&read)["structuredContent"];
    assert_eq!(
        (&proposal["gist"], &proposal["matters"]),
        (&Value::Null, &Value::Null)
    );
    assert_eq!(proposal["options"], json!([]));
    assert_eq!(proposal["recommendation"], Value::Null);
    drop(session.finish());
}

// ------------------------------------------------------------------ PQ2

/// PQ2, the MCP part: each cap is an error result naming the field, its
/// content and document the twin's; nothing stored. M: 4 options accepted.
#[test]
fn pq2_each_cap_is_an_error_result_with_the_document() {
    let repo = Repo::new("mpq-pq2");
    let twin_home = repo.scratch.home("twin");
    let mut session = repo.session();
    let option = |label: String| json!({"label": label, "effect": "e", "price": "p"});
    let with = |key: &str, value: Value| {
        let mut args = example();
        args[key] = value;
        args
    };
    let mut no_options = example();
    no_options.as_object_mut().unwrap().remove("options");
    let mut long_label = example();
    long_label["options"][0]["label"] = json!("l".repeat(129));
    let cases: Vec<(&str, Value, &str)> = vec![
        (
            "4 options",
            with(
                "options",
                Value::Array((0..4).map(|n| option(format!("Way {n}"))).collect()),
            ),
            "options: 4 option(s); give 2 to 3, each priced",
        ),
        (
            "1 option",
            with("options", json!([option("Way".to_owned())])),
            "options: 1 option(s); give 2 to 3, each priced",
        ),
        (
            "[]",
            {
                let mut args = with("options", json!([]));
                args["recommendation"] = Value::Null;
                args
            },
            "options: 0 option(s); give 2 to 3, each priced",
        ),
        (
            "a 161-byte gist",
            with("gist", json!("g".repeat(161))),
            "gist: 161 bytes; at most 160",
        ),
        (
            "a gist holding LF",
            with("gist", json!("Should resting\nrestart it?")),
            "gist: holds a line break; give one line",
        ),
        (
            "a blank gist",
            with("gist", json!("  ")),
            "gist: blank: give a value",
        ),
        (
            "a 513-byte matters",
            with("matters", json!("m".repeat(513))),
            "matters: 513 bytes; at most 512",
        ),
        (
            "recommendation 2 of 2",
            with("recommendation", json!(2)),
            "recommendation: 2 is no index into the 2 options (0 to 1)",
        ),
        (
            "a recommendation without options",
            no_options,
            "recommendation: given without options",
        ),
        (
            "a 129-byte label",
            long_label,
            "options[0].label: 129 bytes; at most 128",
        ),
    ];
    for (label, args, line) in &cases {
        let reply = session.call("ask_question", args.clone());
        let got = result(&reply);
        assert_eq!(got["isError"], json!(true), "{label}: {reply}");
        assert_eq!(
            got["structuredContent"]["notes"],
            json!([line]),
            "{label}: {reply}"
        );
        assert_eq!(got["structuredContent"]["id"], Value::Null, "{label}");
        assert_twin(&reply, &repo.twin(args, &twin_home), label);
        assert!(content_text(got).contains(line), "{label}");
    }
    assert!(
        !data_dir(&repo.home).join("lantern-keep.db").exists()
            || session
                .call("get_proposal", json!({"proposal_id": "PR-0001"}))
                .pointer("/result/isError")
                == Some(&json!(true)),
        "nothing stored"
    );
    drop(session.finish());
}

// ------------------------------------------------------------------ PQ9

/// PQ9, the schemas: `ask_question`'s input gains `gist`, `matters`
/// (nullable strings), `options` (nullable, its items closed `{label,
/// effect, price}`) and `recommendation` (a nullable integer), none
/// required, each documented in "MCP"'s words with no digit; the review
/// document's output (`get_proposal`, `review_proposal`) gains `gist` and
/// `matters`, nullable strings; the other raises take none of the four.
#[test]
fn pq9_the_schemas_hold_the_keys() {
    let repo = Repo::new("mpq-pq9-schema");
    let mut session = repo.session();
    let list = session.tools();
    let input = input_schema(&list, "ask_question");
    assert_eq!(input["additionalProperties"], json!(false), "{input}");
    let required = input["required"].as_array().cloned().unwrap_or_default();
    for (key, kind, doc) in [
        ("gist", "string", GIST_DOC),
        ("matters", "string", MATTERS_DOC),
        ("options", "array", OPTIONS_DOC),
        ("recommendation", "integer", RECOMMENDATION_DOC),
    ] {
        let property = &input["properties"][key];
        assert_eq!(property["type"], json!([kind, "null"]), "{key}: {property}");
        assert_eq!(property["description"], json!(doc), "{key}");
        assert!(
            !doc.chars().any(|c| c.is_ascii_digit()) || key == "recommendation",
            "{key}: a digit"
        );
        assert!(!required.contains(&json!(key)), "{key} is optional");
    }
    let item = &input["properties"]["options"]["items"];
    let resolved = if let Some(reference) = item["$ref"].as_str() {
        let name = reference.rsplit('/').next().unwrap();
        input["$defs"][name].clone()
    } else {
        item.clone()
    };
    assert_eq!(resolved["additionalProperties"], json!(false), "{resolved}");
    let mut keys: Vec<&str> = resolved["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{resolved}"))
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["effect", "label", "price"]);
    let mut needed: Vec<&str> = resolved["required"]
        .as_array()
        .expect("required")
        .iter()
        .map(|key| key.as_str().unwrap())
        .collect();
    needed.sort_unstable();
    assert_eq!(needed, ["effect", "label", "price"]);
    for name in ["report_decision", "report_discrepancy"] {
        for key in ["gist", "matters"] {
            assert!(
                input_schema(&list, name)["properties"].get(key).is_none(),
                "{name} takes no {key}"
            );
        }
    }
    let schemas = output_schemas(&list);
    let mut seen = 0;
    for name in ["get_proposal", "review_proposal"] {
        let Some(schema) = schemas.get(name) else {
            continue;
        };
        seen += 1;
        for key in ["gist", "matters"] {
            assert_eq!(
                schema["properties"][key]["type"],
                json!(["string", "null"]),
                "{name}.{key}: {schema}"
            );
        }
    }
    assert!(seen >= 1, "get_proposal's output schema");
    drop(session.finish());
}

/// Asserts "MCP"'s edits on `description`, `lead` its edit 2.
fn assert_edits(description: &str, lead: &str, context: &str) {
    assert!(description.is_ascii(), "{context}");
    for (before, after) in EDITS {
        if !after.is_empty() {
            assert_eq!(
                description.matches(after).count(),
                1,
                "{context}: {after:?} in {description}"
            );
        }
        if after.is_empty() || !after.contains(before) {
            assert!(
                !description.contains(before),
                "{context}: {before:?} in {description}"
            );
        }
    }
    assert_eq!(
        description.matches(lead).count(),
        1,
        "{context}: {description}"
    );
    // The numbers the description names are the caps'.
    for (number, cap) in [
        ("160", GIST_MAX),
        ("512", MATTERS_MAX),
        ("2-3", OPTIONS_MIN),
        ("2-3", QUESTION_OPTIONS_MAX),
    ] {
        assert!(description.contains(number), "{context}: {number}");
        assert!(
            number.contains(&cap.to_string()),
            "{context}: {number} is {cap}"
        );
    }
}

/// PQ9, the texts: the stdio description takes "MCP"'s edits, 2 031 bytes;
/// its sixth edit drops the decision-record clause; `INSTRUCTIONS` and
/// `HUB_INSTRUCTIONS` byte-equal to before.
#[test]
fn pq9_the_description_takes_the_edits_and_the_instructions_stay() {
    let repo = Repo::new("mpq-pq9-texts");
    let mut session = repo.session();
    let list = session.tools();
    let description = tool(&list, "ask_question")["description"]
        .as_str()
        .expect("a description");
    assert_edits(description, STDIO_LEAD, "stdio");
    assert!(
        !description
            .contains(" (a decision record: a queue hit's record, path, title as the answer)"),
        "{description}"
    );
    assert!(
        description.contains(
            "The owner decides with `spec approve PR` or answers with `spec reject PR --reason \
             <answer>`."
        ),
        "{description}"
    );
    assert_eq!(description.len(), 2031, "{description}");
    assert_eq!(blake3_hex(INSTRUCTIONS.as_bytes()), INSTRUCTIONS_B3);
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
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

/// The hub server's answers over `root`: `tools/list` first, then each
/// `(tool, arguments)` of `calls`.
async fn hub_answers(root: &Path, home: &Path, calls: &[(&str, Value)]) -> Vec<Value> {
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
                .and_then(|message| message.get("result"))
                .unwrap_or_else(|| panic!("no result for {id}: {messages:?}"))
                .clone()
        })
        .collect()
}

/// PQ9, the hub: `tools/list` holds 18 tools, its `ask_question` the four
/// keys and the hub's description with "MCP"'s edits (2 044 bytes, `with
/// Confirm on the hub`); the example stores `PR-0001` with the fields, read
/// back by `get_proposal`. M: the hub's args ignoring `options`.
#[tokio::test]
async fn pq9_a_hub_stores_the_fields() {
    let remote = Repo::new("mpq-pq9-hub");
    let calls = [
        ("ask_question", example()),
        ("get_proposal", json!({"proposal_id": "PR-0001"})),
    ];
    let hub = hub_answers(&remote.main, &remote.home, &calls).await;
    let listed = tool_names(&hub[0]);
    assert_eq!(listed.len(), 18, "{listed:?}");
    assert_eq!(HUB_TOOLS.len(), 18);
    let asked = &hub[1];
    assert_ne!(asked["isError"], json!(true), "{asked}");
    assert_eq!(
        asked["structuredContent"]["id"],
        json!("PR-0001"),
        "{asked}"
    );
    let read = &hub[2]["structuredContent"];
    assert_eq!(read["gist"], json!(GIST), "{read}");
    assert_eq!(read["matters"], json!(MATTERS));
    assert_eq!(read["options"], options());
    assert_eq!(read["recommendation"], json!(0));

    let input = input_schema(&hub[0], "ask_question");
    for (key, doc) in [
        ("gist", GIST_DOC),
        ("matters", MATTERS_DOC),
        ("options", OPTIONS_DOC),
        ("recommendation", RECOMMENDATION_DOC),
    ] {
        assert_eq!(input["properties"][key]["description"], json!(doc), "{key}");
    }
    let description = tool(&hub[0], "ask_question")["description"]
        .as_str()
        .expect("a description");
    assert_edits(description, HUB_LEAD, "hub");
    assert!(
        !description.contains(" (a queue hit's record, path, title as the answer)"),
        "{description}"
    );
    assert!(
        description.contains("with Confirm on the hub"),
        "{description}"
    );
    assert!(
        !description.contains("with a Confirm on the hub"),
        "{description}"
    );
    assert_eq!(description.len(), 2044, "{description}");
    assert_eq!(blake3_hex(HUB_INSTRUCTIONS.as_bytes()), HUB_INSTRUCTIONS_B3);
}
