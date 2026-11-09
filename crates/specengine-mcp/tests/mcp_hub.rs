//! docs/features/hub-mcp.md (RM-0053) H11 and H9, the MCP crate's half: the
//! hub server's texts and tool set, read from the crate's own exports
//! (`HUB_INSTRUCTIONS`, `HUB_TOOLS`) and from a hub server built over a seam
//! that runs nothing, so no daemon, no corpus and no call is needed. The wire's
//! half, both eras through a real hub, is `crates/specengine-http/tests/
//! hub_mcp.rs`.
//!
//! - `HUB_INSTRUCTIONS` is ASCII and at most 2 048 bytes; it names each of the
//!   seventeen, none of the twelve left out nor `task_id`, says a person
//!   decides with Confirm and that a card write acts as the person's login, in
//!   the order of "Data"'s texts; the stdio `INSTRUCTIONS` are another text.
//!   M: `claim_task` in `HUB_INSTRUCTIONS`.
//! - `HUB_TOOLS` are the seventeen; the hub server routes each and none of the
//!   twelve. M: `review_proposal` or `report_drift` in the hub set.
//! - Each hub description is ASCII, at most 2 048 bytes, holds the determinism
//!   sentence; the eight variants (iteration 2: `get_roadmap` joins the
//!   seven writes) name no `spec approve`, `spec reject`, `task_id`,
//!   `get_task`, ``You act as `agent` `` or the owner's decision on a
//!   terminal.
//! - The hub server's `get_info` sends `HUB_INSTRUCTIONS`.
//! - "Rules", **Identity**: a hub write that finds no login stores nothing and
//!   answers the fence's line as an error result, all seven, over a transport
//!   in memory (no HTTP crate: this crate has none) and a seam that runs every
//!   turn for real on a scratch `fixtures/spec-a`. M: a write falling back to
//!   a default actor.
//!
//! The 2 049-byte padding (H11's second mutation) is a build failure, not a
//! test: the compile-time asserts in `src/hub.rs`.

#![cfg(unix)]

mod common;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Scratch;
use rmcp::model::Extensions;
use rmcp::service::{RxJsonRpcMessage, TxJsonRpcMessage};
use rmcp::transport::Transport;
use rmcp::{RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};
use specengine_cli::{Env, Globals, events_after};
use specengine_mcp::{
    Command, HUB_INSTRUCTIONS, HUB_TOOLS, HubSeam, INSTRUCTIONS, SpecEngineServer, TurnFuture,
};

/// The hub's seventeen tools, sorted (docs/features/hub-mcp.md "Data").
const SEVENTEEN: [&str; 17] = [
    "ask_question",
    "create_card",
    "edit_card",
    "get_card",
    "get_context_bundle",
    "get_flow",
    "get_node",
    "get_proposal",
    "get_roadmap",
    "get_trace",
    "get_tree",
    "list_cards",
    "move_card",
    "propose_change",
    "report_decision",
    "report_discrepancy",
    "search",
];

/// The twelve the hub leaves out, sorted.
const TWELVE: [&str; 12] = [
    "check_binding",
    "claim_task",
    "complete_task",
    "find_symbols",
    "get_impact",
    "get_task",
    "refs",
    "report_drift",
    "report_run",
    "review_proposal",
    "submit_plan",
    "unmapped",
];

/// The eight whose descriptions are the hub's own (iteration 2: `get_roadmap`
/// joins the seven writes).
const VARIANTS: [&str; 8] = [
    "ask_question",
    "create_card",
    "edit_card",
    "get_roadmap",
    "move_card",
    "propose_change",
    "report_decision",
    "report_discrepancy",
];

const TEXT_LIMIT: usize = 2048;
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

/// A seam that knows no login and runs nothing: enough to build the server
/// and read its texts.
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

/// Whether `text` holds `word` whole: no letter, digit or `_` either side.
fn has_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let wordy = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    text.match_indices(word).any(|(at, _)| {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + word.len()).copied();
        !before.is_some_and(wordy) && !after.is_some_and(wordy)
    })
}

#[test]
fn the_word_reader_reads_whole_words() {
    assert!(has_word("a claim_task(b)", "claim_task"));
    assert!(!has_word("a claim_tasks b", "claim_task"));
    assert!(!has_word("prefs", "refs"));
    assert!(has_word("refs(id) = refs", "refs"));
}

/// H11: the instructions' budget, their seventeen and no twelfth, their words
/// for a person's decision and the login, in "Data"'s order.
#[test]
fn hub_instructions_hold_their_budget_and_name_the_seventeen_alone() {
    let text = HUB_INSTRUCTIONS;
    assert!(text.is_ascii(), "ASCII");
    assert!(text.len() <= TEXT_LIMIT, "{} bytes", text.len());
    for name in SEVENTEEN {
        assert!(has_word(text, name), "HUB_INSTRUCTIONS omit {name}");
    }
    for name in TWELVE.iter().chain(["task_id"].iter()) {
        assert!(!has_word(text, name), "HUB_INSTRUCTIONS name {name}");
    }
    let lower = text.to_ascii_lowercase();
    for words in [
        "a person decides with confirm, on this hub or on a terminal",
        "you act as your person's login",
    ] {
        assert!(lower.contains(words), "HUB_INSTRUCTIONS omit {words:?}");
    }
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("HUB_INSTRUCTIONS omit {needle:?}"))
    };
    let order = [
        at("get_tree"),
        at("propose_change"),
        at("create_card"),
        at("REF"),
        at("Deterministic"),
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "the reads, the queue, the cards, REF, the rules: {order:?}"
    );
    assert_ne!(text, INSTRUCTIONS, "the stdio text is another");
    assert_eq!(
        hub().get_info().instructions.as_deref(),
        Some(HUB_INSTRUCTIONS),
        "the hub server sends HUB_INSTRUCTIONS"
    );
}

/// H9 and H11: `HUB_TOOLS` are the seventeen; the hub server routes each, its
/// description within the budget and holding the determinism sentence, and
/// none of the twelve; the eight variants drop the terminal's words and
/// `get_task`, `get_roadmap`'s also the owner's decision on a terminal.
#[test]
fn hub_tools_are_the_seventeen_and_their_texts_hold() {
    let mut names: Vec<&str> = HUB_TOOLS.to_vec();
    names.sort_unstable();
    assert_eq!(names, SEVENTEEN, "HUB_TOOLS");
    let server = hub();
    for name in SEVENTEEN {
        let tool = server
            .get_tool(name)
            .unwrap_or_else(|| panic!("the hub routes no {name}"));
        let description = tool
            .description
            .as_deref()
            .unwrap_or_else(|| panic!("{name}: no description"));
        assert!(description.is_ascii(), "{name}");
        assert!(
            description.len() <= TEXT_LIMIT,
            "{name}: {} bytes",
            description.len()
        );
        assert!(description.contains(DETERMINISM), "{name}");
        if VARIANTS.contains(&name) {
            for word in [
                "spec approve",
                "spec reject",
                "task_id",
                "get_task",
                "You act as `agent`",
                "the owner's decision on a terminal",
            ] {
                assert!(!description.contains(word), "{name} holds {word:?}");
            }
        }
        let schema = serde_json::to_value(tool.input_schema.as_ref()).expect("a schema");
        for key in ["login", "as", "actor", "task_id"] {
            assert!(
                schema["properties"].get(key).is_none(),
                "{name}'s schema holds {key}"
            );
        }
    }
    for name in TWELVE {
        assert!(server.get_tool(name).is_none(), "the hub routes {name}");
    }
}

// ------------------------------------------------------------- no login

/// The fence's line a hub write answers when its request holds no login.
const NO_LOGIN: &str = "refused: this hub needs one Remote-User header from its proxy";

/// A seam whose requests hold no login and whose turn runs each command at
/// once, for real, on one project: a write that ran would store.
struct Anonymous {
    env: Env,
    globals: Globals,
    turns: Arc<AtomicUsize>,
}

impl HubSeam for Anonymous {
    fn login(&self, _extensions: &Extensions) -> Option<String> {
        None
    }

    fn turn(&self, command: Command) -> TurnFuture {
        self.turns.fetch_add(1, Ordering::SeqCst);
        command.run(&self.env, &self.globals);
        Box::pin(async {})
    }
}

/// A transport in memory: the client's messages queued, the server's kept;
/// it closes once `expected` responses have come (or after 60 s).
struct Memory {
    inbound: VecDeque<RxJsonRpcMessage<RoleServer>>,
    outbound: Arc<Mutex<Vec<Value>>>,
    expected: usize,
}

/// How many of `messages` are responses.
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

/// A client's message.
fn client(value: Value) -> RxJsonRpcMessage<RoleServer> {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("not a client message ({error}): {value}"))
}

/// "Rules", **Identity**: each of the seven hub writes, its arguments well
/// formed, with no login in its request: an error result holding the fence's
/// line alone, and nothing stored in the project's queue, though the seam runs
/// every turn it is given.
#[tokio::test]
async fn a_hub_write_with_no_login_stores_nothing_and_answers_the_fence_line() {
    let scratch = Scratch::new("hub-no-login");
    let root = scratch.copy("spec-a", "a");
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
    let turns = Arc::new(AtomicUsize::new(0));
    let seam = Anonymous {
        env: env.clone(),
        globals: globals.clone(),
        turns: Arc::clone(&turns),
    };
    let agent = json!({"author_role": "drafter", "author_model": "m-1", "run": "r-1"});
    let with_agent = |mut args: Value| {
        for (key, value) in agent.as_object().expect("an object") {
            args[key] = value.clone();
        }
        args
    };
    let writes = [
        (
            "propose_change",
            with_agent(json!({"kind": "update", "target": "EDGE-STAM-ZERO",
                "base": "0000000000000000", "text": "## Depletion {#EDGE-STAM-ZERO}\n- At zero.\n",
                "rationale": "Shorter."})),
        ),
        (
            "ask_question",
            with_agent(
                json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Does it stop?",
                "working_answer": "Yes.", "price_of_other": "A rule case."}),
            ),
        ),
        (
            "report_discrepancy",
            with_agent(
                json!({"node_ids": ["EDGE-STAM-ZERO"], "summary": "It departs.",
                "gap_type": "contradicts", "severity": "high",
                "evidence": [{"file": "notes/playtest.md", "lines": "3-9",
                    "observed": "regenerates while walking", "documented": "only at rest"}],
                "options": [{"label": "build", "effect": "follow the text", "price": "1 item"},
                    {"label": "text", "effect": "allow walking", "price": "a rebalance"}],
                "recommendation": 0}),
            ),
        ),
        (
            "report_decision",
            with_agent(
                json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Does zero end it?",
                "working_answer": "Yes.", "price_of_other": "A rule case."}),
            ),
        ),
        ("create_card", json!({"title": "Lamp oil"})),
        (
            "edit_card",
            json!({"card_id": "C-0001", "goal": "Fill it."}),
        ),
        ("move_card", json!({"card_id": "C-0001", "to": "ready"})),
    ];
    let mut inbound = VecDeque::new();
    inbound.push_back(client(
        json!({"jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {"protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "specengine-mcp-tests", "version": "0.0.0"}}}),
    ));
    inbound.push_back(client(
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    ));
    for (n, (name, arguments)) in writes.iter().enumerate() {
        inbound.push_back(client(json!({"jsonrpc": "2.0", "id": n + 1,
            "method": "tools/call", "params": {"name": name, "arguments": arguments}})));
    }
    let outbound = Arc::new(Mutex::new(Vec::new()));
    let transport = Memory {
        inbound,
        outbound: Arc::clone(&outbound),
        expected: 1 + writes.len(),
    };
    let running = SpecEngineServer::hub(Arc::new(seam))
        .serve(transport)
        .await
        .expect("the handshake");
    let _ = tokio::time::timeout(Duration::from_secs(90), running.waiting()).await;

    let messages = outbound.lock().expect("the server's messages").clone();
    for (n, (name, _)) in writes.iter().enumerate() {
        let response = messages
            .iter()
            .find(|message| message.get("id") == Some(&json!(n + 1)))
            .unwrap_or_else(|| panic!("{name}: no answer in {messages:?}"));
        let result = response
            .get("result")
            .unwrap_or_else(|| panic!("{name}: a JSON-RPC error: {response}"));
        assert_eq!(result["isError"], json!(true), "{name}: {result}");
        let text: String = result["content"]
            .as_array()
            .expect("content")
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect();
        assert_eq!(text.trim_end(), NO_LOGIN, "{name}: {result}");
    }
    let stored = events_after(&env, &globals, Some(0))
        .map(|page| page.events.len())
        .unwrap_or(0);
    assert_eq!(
        stored,
        0,
        "nothing stored ({} turns taken)",
        turns.load(Ordering::SeqCst)
    );
}

// ---------------------------------------------------- card-tasks (RM-0092)

/// The sentence the hub's `edit_card` text adds (card-tasks, CT-7).
const HUB_EDIT_TASK_SENTENCE: &str = "task: T- IDs as a terminal writes them, listed once, at most \
                                      32; it replaces the list, - clears it. This hub holds no \
                                      task: it checks none and get_card shows them without state.";

/// The sentence `GET_DESCRIPTION` adds, shared by stdio and a hub.
const GET_TASKS_SENTENCE: &str = "tasks: each linked task {id, status, title, stale}, read now as \
                                  spec task list reads it; status null, with a note that starts \
                                  with its ID, for a task this server cannot read: a corrupt row, \
                                  another repository, a hub.";

/// CT-7, the crate's half: the hub's `edit_card` names `task`, takes it, and
/// says the hub checks none; its `get_card` holds the shared `tasks`
/// sentence; each ASCII within 2 048 bytes, still among the seventeen.
#[test]
fn card_tasks_the_hub_card_texts_name_task_and_check_none() {
    assert_eq!(HUB_TOOLS.len(), 17);
    let server = hub();
    for (name, word, sentence) in [
        ("edit_card", "task", HUB_EDIT_TASK_SENTENCE),
        ("get_card", "tasks", GET_TASKS_SENTENCE),
    ] {
        let tool = server
            .get_tool(name)
            .unwrap_or_else(|| panic!("the hub routes no {name}"));
        let description = tool
            .description
            .as_deref()
            .unwrap_or_else(|| panic!("{name}: no description"));
        assert!(has_word(description, word), "{name}: {description}");
        assert!(
            description.contains(sentence),
            "{name} holds {sentence:?}: {description}"
        );
        assert!(description.is_ascii(), "{name}");
        assert!(
            description.len() <= TEXT_LIMIT,
            "{name}: {} bytes",
            description.len()
        );
    }
    let edit = server.get_tool("edit_card").expect("edit_card");
    let schema = serde_json::to_value(edit.input_schema.as_ref()).expect("a schema");
    assert!(
        schema["properties"].get("task").is_some(),
        "the hub's edit_card takes task: {schema}"
    );
}
