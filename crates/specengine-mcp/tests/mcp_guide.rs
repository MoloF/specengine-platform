//! docs/features/specengine-guide.md (RM-0129) "Acceptance criteria" GU5,
//! the stdio server and the hub's server in process: the real
//! `specengine-mcp` binary over scratch directories (an empty one, a copy of
//! `fixtures/spec-a`), each with its own `HOME` (or none), and
//! `SpecEngineServer::hub` over an in-memory transport with a seam that
//! counts its turns. Expectations are the CLI library's answer
//! (`specengine_cli::guide`, the code `spec guide` prints) and the pages of
//! `docs/guide/` as `guide_source` reads them by "Data".
//!
//! - `get_guide` with no arguments, `{topic}` and `{topic, rule}` (every
//!   topic, every rule): content `spec guide …`'s bytes, `structuredContent`
//!   `--json`'s document, within its output schema; in both eras, with no
//!   project, with `HOME` unset, and inside spec-a; nothing written.
//! - `rule` without `topic`, an unknown topic, an unknown rule: `isError`,
//!   the CLI's line; a mistyped argument is refused as arguments.
//! - `tools/list` is [`guide_source::KNOWN_TOOLS`] (thirty-three), the
//!   description within 2 048 B naming "where does X go", the input `{topic?,
//!   rule?}`; the output schema is pinned in `plugin_files.rs`'s
//!   `SCHEMA_PINS`.
//! - The resource template `guide://{topic}` (`name` `guide`, `text/markdown`)
//!   and `resources/read guide://proposals`, with and without a project;
//!   `resources/list` names no `guide://`.
//! - `INSTRUCTIONS` and `HUB_INSTRUCTIONS` name `get_guide`.
//! - The hub: `HUB_TOOLS` is 19 with `get_guide`, which is not in
//!   `HUB_READ_TOOLS`; the hub server answers every form as stdio does and
//!   its seam's `turn` is never called for it. M (named): the hub tool
//!   taking the turn.
//!
//! The wire half (`/mcp/<slug>`, its slow line's `wait_ms=-`) is http
//! `guide.rs`.

mod common;

#[path = "../../specengine-cli/tests/guide_source/mod.rs"]
mod guide_source;

use std::collections::VecDeque;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::read::{
    ERAS, Era, Expected, Session, assert_parity, cli_env, conforms, content_text, expected,
    flat_input_problems, output_schemas,
};
use common::*;
use guide_source::{Form, KNOWN_TOOLS, forms, source_pages, unknown_topic};
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, GuideRequest, Outcome};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_READ_TOOLS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer,
    TurnFuture,
};

/// The tool under test.
const TOOL: &str = "get_guide";

/// The CLI library's answer to `get_guide`'s arguments: `spec guide
/// [TOPIC[#ID]]`.
fn cli_answer(arguments: &Value) -> Expected {
    let topic = arguments["topic"].as_str();
    let rule = arguments["rule"].as_str();
    let reference = match (topic, rule) {
        (Some(topic), Some(rule)) => Some(format!("{topic}#{rule}")),
        (Some(topic), None) => Some(topic.to_owned()),
        (None, _) => None,
    };
    expected(specengine_cli::guide(&GuideRequest { reference }).map(Outcome::Guide))
}

/// Every form against the CLI library and against the source, on one
/// session.
fn assert_every_form(session: &mut Session, forms: &[Form], context: &str) {
    let list = session.tools();
    let schemas = output_schemas(&list);
    let schema = schemas
        .get(TOOL)
        .unwrap_or_else(|| panic!("{TOOL} has no outputSchema"));
    for form in forms {
        let reply = session.call(TOOL, form.arguments.clone());
        let want = cli_answer(&form.arguments);
        let at = format!("{context} {}", form.arguments);
        assert_parity(&reply, &want, schema, &at);
        let answer = result(&reply);
        assert_ne!(answer["isError"], json!(true), "{at}: {answer}");
        assert!(content_text(answer) == form.text, "{at}: the source's text");
        assert_eq!(
            answer["structuredContent"], form.json,
            "{at}: the source's JSON"
        );
    }
}

/// GU5: with no project (an empty directory, a fresh `HOME`), in both eras,
/// every form is the CLI's and the source's; with `HOME` unset too.
#[test]
fn gu5_stdio_answers_with_no_project() {
    let pages = source_pages();
    let all = forms(&pages);
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        assert_every_form(&mut session, &all, &format!("{era:?} no project"));
        drop(session.finish());
    }
    let few: Vec<Form> = all.iter().take(3).cloned().collect();
    let mut session = Session::open(Era::Stateless, &[], None, Home::Unset);
    assert_every_form(&mut session, &few, "HOME unset");
    drop(session.finish());
}

/// GU5: inside a copy of spec-a, in both eras, every form is the same;
/// the copy and the `HOME` are byte-equal after (no index built, nothing
/// written).
#[test]
fn gu5_stdio_answers_inside_a_project_and_writes_nothing() {
    let pages = source_pages();
    let all = forms(&pages);
    let scratch = Scratch::new("mcp-gu5-project");
    let root = scratch.copy("spec-a", "spec-a");
    let home = scratch.home("h");
    let before = snapshot(scratch.path());
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        assert_every_form(&mut session, &all, &format!("{era:?} spec-a"));
        drop(session.finish());
    }
    assert_eq!(
        snapshot(scratch.path()),
        before,
        "the project and the HOME unchanged"
    );
    assert!(!data_dir(&home).exists(), "no data directory");
}

/// GU5: `rule` without `topic` is `isError`; an unknown topic and an
/// unknown rule are `isError` with the CLI's line; a mistyped argument is
/// refused before anything runs.
#[test]
fn gu5_refusals() {
    let pages = source_pages();
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let schemas = output_schemas(&list);
    let schema = &schemas[TOOL];
    let alone = session.call(TOOL, json!({"rule": "GD-PROPOSALS-ONE-DOOR"}));
    let answer = result(&alone);
    assert_eq!(
        answer["isError"],
        json!(true),
        "rule without topic: {answer}"
    );
    assert!(
        answer.get("structuredContent").is_none() || answer["structuredContent"].is_object(),
        "{answer}"
    );
    for arguments in [
        json!({"topic": "nope"}),
        json!({"topic": "proposals", "rule": "GD-PROPOSALS-NOPE"}),
    ] {
        let reply = session.call(TOOL, arguments.clone());
        let want = cli_answer(&arguments);
        assert!(want.is_error, "the CLI refuses {arguments}");
        assert_parity(&reply, &want, schema, &format!("{arguments}"));
    }
    let nope = session.call(TOOL, json!({"topic": "nope"}));
    assert_eq!(
        content_text(result(&nope)),
        format!("{}\n", unknown_topic("nope")),
        "\"Data\"'s line"
    );
    let proposals = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .expect("proposals");
    let rule = session.call(
        TOOL,
        json!({"topic": "proposals", "rule": "GD-PROPOSALS-NOPE"}),
    );
    assert_eq!(
        content_text(result(&rule)),
        format!(
            "{}\n",
            guide_source::unknown_rule(proposals, "GD-PROPOSALS-NOPE")
        )
    );
    let mistyped = session.call(TOOL, json!({"topic": 3}));
    assert!(
        mistyped.get("error").is_some() || result(&mistyped)["isError"] == json!(true),
        "a number for `topic` is refused: {mistyped}"
    );
    drop(session.finish());
}

/// GU5: `tools/list` (both eras) is the thirty-three tools, `get_guide`
/// among them; its description is within 2 048 bytes and names "where
/// does X go"; its input is `{topic?, rule?}`, two strings, flat.
#[test]
fn gu5_the_tool_is_listed_and_described() {
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        let mut names = tool_names(&list);
        names.sort();
        assert_eq!(names, KNOWN_TOOLS, "{era:?}: thirty-eight tools");
        let guide = tool(&list, TOOL);
        let description = guide["description"].as_str().expect("a description");
        assert!(
            description.len() <= 2048,
            "{} bytes: {description}",
            description.len()
        );
        assert!(
            description
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .contains("where does X go"),
            "names \"where does X go\": {description}"
        );
        let input = &guide["inputSchema"];
        let mut properties: Vec<&String> = input["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        properties.sort();
        assert_eq!(properties, ["rule", "topic"], "{input}");
        // An optional input is typed `["string", "null"]`, as every optional
        // input of the server is.
        for key in ["topic", "rule"] {
            let kind = &input["properties"][key]["type"];
            assert!(
                *kind == json!("string") || *kind == json!(["string", "null"]),
                "{key}: {input}"
            );
        }
        assert!(
            input
                .get("required")
                .is_none_or(|required| required.as_array().is_some_and(Vec::is_empty)),
            "nothing required: {input}"
        );
        assert_eq!(
            flat_input_problems(TOOL, input),
            Vec::<String>::new(),
            "{input}"
        );
        assert!(guide["outputSchema"].is_object(), "an output schema");
        drop(session.finish());
    }
}

/// GU5: the resource template `guide://{topic}` beside the node template;
/// `resources/read guide://proposals` is the page's text, `text/markdown`,
/// with and without a project; `resources/list` names no `guide://`; an
/// unknown topic is an error.
#[test]
fn gu5_the_resource_template_reads_the_page() {
    let pages = source_pages();
    let proposals = pages
        .iter()
        .find(|page| page.topic == "proposals")
        .expect("proposals");
    let scratch = Scratch::new("mcp-gu5-resource");
    let root = scratch.copy("spec-a", "spec-a");
    let home = scratch.home("h");
    let template =
        json!({"uriTemplate": "guide://{topic}", "name": "guide", "mimeType": "text/markdown"});
    for (label, cwd) in [("no project", None), ("spec-a", Some(root.as_path()))] {
        for era in ERAS {
            let mut session = Session::open(era, &[], cwd, Home::At(&home));
            let templates = result(&session.request("resources/templates/list", json!({}))).clone();
            let listed = templates["resourceTemplates"]
                .as_array()
                .expect("resourceTemplates");
            assert!(listed.contains(&template), "{label} {era:?}: {templates}");
            assert!(
                listed
                    .iter()
                    .any(|entry| entry["uriTemplate"] == "spec://{project}/node/{id}"),
                "the node template kept: {templates}"
            );
            let read = session.read("guide://proposals");
            let contents = result(&read)["contents"]
                .as_array()
                .expect("contents")
                .clone();
            assert_eq!(contents.len(), 1, "{read}");
            assert_eq!(contents[0]["uri"], "guide://proposals");
            assert_eq!(contents[0]["mimeType"], "text/markdown");
            assert!(
                contents[0]["text"].as_str() == Some(proposals.text.as_str()),
                "{label} {era:?}: the page's text"
            );
            if cwd.is_some() {
                let uris: Vec<String> = session
                    .all_resources()
                    .iter()
                    .flat_map(|page| {
                        page["resources"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|entry| entry["uri"].as_str().unwrap_or_default().to_owned())
                    })
                    .collect();
                assert!(
                    uris.iter().all(|uri| !uri.starts_with("guide://")),
                    "resources/list unchanged: {uris:?}"
                );
            }
            let unknown = session.read("guide://nope");
            assert!(unknown.get("error").is_some(), "{unknown}");
            drop(session.finish());
        }
    }
}

/// GU5: both instruction texts name the tool.
#[test]
fn gu5_the_instructions_name_the_tool() {
    assert!(INSTRUCTIONS.contains("get_guide"), "{INSTRUCTIONS}");
    assert!(HUB_INSTRUCTIONS.contains("get_guide"), "{HUB_INSTRUCTIONS}");
}

// ------------------------------------------------------------------ the hub

/// A seam whose requests hold the login `mira`, whose turn runs each
/// command at once and counts itself.
struct Counting {
    env: Env,
    globals: Globals,
    turns: Arc<AtomicUsize>,
}

impl HubSeam for Counting {
    fn login(&self, _extensions: &Extensions) -> Option<String> {
        Some("mira".to_owned())
    }

    fn turn(&self, _login: &str, command: Command) -> TurnFuture {
        self.turns.fetch_add(1, Ordering::SeqCst);
        command.run(&self.env, &self.globals);
        Box::pin(async {})
    }
}

/// A transport in memory, as `card_context.rs`'s: the client's messages
/// queued, the server's kept; it closes once `expected` responses have
/// come.
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

/// The hub server's answers over spec-a: `tools/list` (id 1), then each of
/// `calls` (`(tool, arguments)`, ids from 2); and the seam's turn count.
async fn hub_answers(root: PathBuf, home: PathBuf, calls: &[(&str, Value)]) -> (Vec<Value>, usize) {
    let turns = Arc::new(AtomicUsize::new(0));
    let seam = Counting {
        env: cli_env(&root, Some(&home)),
        globals: Globals {
            root: Some(root.clone()),
            config: None,
        },
        turns: Arc::clone(&turns),
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
    let answers = (1..=calls.len() + 1)
        .map(|id| {
            messages
                .iter()
                .find(|message| message.get("id") == Some(&json!(id)))
                .and_then(|message| message.get("result"))
                .unwrap_or_else(|| panic!("no result for {id}: {messages:?}"))
                .clone()
        })
        .collect();
    (answers, turns.load(Ordering::SeqCst))
}

/// GU5, the hub: `HUB_TOOLS` is 19 with `get_guide`, outside
/// `HUB_READ_TOOLS`; the hub lists it; every form answers as on stdio
/// (the CLI's and the source's), `rule` alone is `isError`, and the seam's
/// turn is never taken; a `get_tree` beside it takes one (the count is
/// live). M (named): the hub tool taking the turn.
#[tokio::test]
async fn gu5_a_hub_answers_alike_and_takes_no_turn() {
    // 21 since docs/features/comments.md CM15; 24 since
    // docs/features/work-references.md RF20.
    assert_eq!(HUB_TOOLS.len(), 24, "{HUB_TOOLS:?}");
    assert!(HUB_TOOLS.contains(&TOOL), "{HUB_TOOLS:?}");
    assert!(!HUB_READ_TOOLS.contains(&TOOL), "it reads no index");
    let pages = source_pages();
    let all = forms(&pages);
    let scratch = Scratch::new("mcp-gu5-hub");
    let root = scratch.copy("spec-a", "spec-a");
    let home = scratch.home("h");
    let mut calls: Vec<(&str, Value)> = all
        .iter()
        .map(|form| (TOOL, form.arguments.clone()))
        .collect();
    calls.push((TOOL, json!({"rule": "GD-PROPOSALS-ONE-DOOR"})));
    let (answers, turns) = hub_answers(root.clone(), home.clone(), &calls).await;
    assert_eq!(turns, 0, "M: get_guide took the project's turn");
    let listed = tool_names(&answers[0]);
    // 21 since docs/features/comments.md CM15; 24 since
    // docs/features/work-references.md RF20.
    assert_eq!(listed.len(), 24, "{listed:?}");
    assert!(listed.iter().any(|name| name == TOOL), "{listed:?}");
    let schemas = output_schemas(&answers[0]);
    for (n, form) in all.iter().enumerate() {
        let answer = &answers[n + 1];
        let at = format!("hub {}", form.arguments);
        assert_ne!(answer["isError"], json!(true), "{at}: {answer}");
        assert!(content_text(answer) == form.text, "{at}: text");
        assert_eq!(answer["structuredContent"], form.json, "{at}: JSON");
        if let Err(problem) = conforms(&schemas[TOOL], &answer["structuredContent"], "$") {
            panic!("{at}: {problem}");
        }
        let want = cli_answer(&form.arguments);
        assert_eq!(content_text(answer), want.text, "{at}: the CLI's text");
    }
    let alone = answers.last().expect("the last answer");
    assert_eq!(alone["isError"], json!(true), "rule without topic: {alone}");
    let (beside, counted) =
        hub_answers(root, home, &[("get_tree", json!({})), (TOOL, json!({}))]).await;
    assert_ne!(beside[1]["isError"], json!(true), "{}", beside[1]);
    assert_eq!(counted, 1, "the read takes its turn, the guide none");
}
