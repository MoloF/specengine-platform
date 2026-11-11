//! docs/features/task-mandate.md (RM-0104, v5 step 6d, ADR-0079)
//! "Acceptance criteria", M7 and M6, the stdio half: `create_card`,
//! `edit_card`, `split_card`, `get_card` and `list_cards` on the default
//! build's server over a scratch git repository of `fixtures/studio-lamp`
//! (the preset's flows), the canon docs/canon/card-mandate.md "The rule".
//!
//! - M7: the three inputs hold `mandate` as "Data" gives it, each closed
//!   (`additionalProperties: false`); the outputs `get_card`'s `mandate`
//!   and `subtask_items`, `list_cards`' rows' `mandate`, `split_card`'s
//!   children's `mandate`.
//! - M7: `create_card {…, reviewer: "Ana Ruiz", mandate: "open"}` stores
//!   it with `as` `agent`; without a reviewer the first line; `edit_card
//!   {card_id, mandate}` an error result naming `agent`, nothing changed;
//!   `mandate: "maybe"` refused as a bad argument.
//! - M6, M7: each call equals `spec card` on a twin queue, content and
//!   `structuredContent`, valid against its `outputSchema`: `get_card` of a
//!   parent whose children hold items. M: `CardSubtaskItemShape` without
//!   `card_id`.
//! - M7: every card tool's text, stdio and hub, ASCII and at most 2 048
//!   bytes, the determinism sentence kept and "Data"'s sentence added;
//!   `INSTRUCTIONS` and `HUB_INSTRUCTIONS` unchanged.
//!
//! The wire's half on a hub (`/mcp`'s `edit_card` acting as the login) is
//! `specengine-http`'s `tests/card_mandate.rs`. Every server has a cleared
//! environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::path::PathBuf;
use std::sync::Arc;

use common::read::{
    Era, Session, assert_bad_arguments, assert_parity, card_library, cli_env, conforms,
    content_text, output_schemas,
};
use common::*;
use git::Sandbox;
use rmcp::ServerHandler;
use rmcp::model::Extensions;
use serde_json::{Value, json};
use specengine_cli::Globals;
use specengine_mcp::{Command, HubSeam, SpecEngineServer, TurnFuture};

const CLOCK: &str = "2026-11-12T09:00:00Z";

const ANA: &str = "Ana Ruiz";
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";
const TEXT_LIMIT: usize = 2048;

/// The first line of "Messages".
const NO_REVIEWER: &str =
    "mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer";

/// "Data"'s sentences, per tool, stdio.
const CREATE_TEXT: &str = "mandate: binding (a departure from the brief goes back as a return \
                           with a question) or open (the department decides and says why); \
                           needs reviewer; null: none. Nothing reads it to allow or refuse.";
const EDIT_TEXT: &str =
    "mandate: binding or open; only the card's reviewer changes it, never agent.";
const EDIT_HUB_TEXT: &str = "only the card's reviewer, your login, changes it.";
const SPLIT_TEXT: &str = "mandate: D=binding or D=open per child discipline, each once; a \
                          child without one holds none.";
const GET_TEXT: &str = "mandate: binding, open or null. subtask_items: the items naming one \
                        of its subtasks, each with card_id first, by ID, in one read.";

/// A scratch git repository of studio-lamp, committed, and its `HOME`.
struct Repo {
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
    /// The twin queue's `HOME`: the CLI library's side.
    twin: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("studio-lamp", "repo");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        let twin = scratch.home("twin");
        Self {
            _scratch: scratch,
            git,
            root,
            home,
            twin,
        }
    }

    fn session(&self) -> Session {
        Session::open(Era::Stateless, &[], Some(&self.root), Home::At(&self.home))
    }

    fn status(&self) -> String {
        self.git
            .git_text(&self.root, &["status", "--porcelain", "--ignored"])
    }

    fn sqlite(&self, sql: &str) -> String {
        let db = data_dir(&self.home).join("studio-lamp.db");
        if !db.exists() {
            return String::new();
        }
        let output = std::process::Command::new("/usr/bin/sqlite3")
            .arg(&db)
            .arg(sql)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("sqlite3 runs");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).expect("UTF-8")
    }

    /// Every event of the server's queue, `(type, payload)`.
    fn events(&self) -> Vec<(String, Value)> {
        self.sqlite("SELECT type || char(9) || payload FROM events ORDER BY seq;")
            .lines()
            .map(|line| {
                let (kind, payload) = line.split_once('\t').expect("two fields");
                (
                    kind.to_owned(),
                    serde_json::from_str(payload).expect("a payload"),
                )
            })
            .collect()
    }

    /// The raw rows of `cards`.
    fn cards(&self) -> String {
        self.sqlite("SELECT * FROM cards ORDER BY id;")
    }
}

/// M1's parent and its hand-off as tool arguments.
fn parent_args() -> Value {
    json!({"title": "Lamp room scene", "discipline": "narrative", "target": ["SCN-ARRIVAL"]})
}

fn hand_off_args(extra: &Value) -> Value {
    let mut args = json!({"title": "Keeper's voice", "discipline": "sound",
                          "target": ["SCN-ARRIVAL"], "parent": "C-0001"});
    for (key, value) in extra.as_object().expect("an object") {
        args[key] = value.clone();
    }
    args
}

/// `tool`'s input property `key`.
fn property<'a>(list: &'a Value, tool_name: &str, key: &str) -> &'a Value {
    &tool(list, tool_name)["inputSchema"]["properties"][key]
}

// --------------------------------------------------------------- tools

/// M7: `create_card` and `edit_card` take `mandate`, `["string", "null"]`,
/// enum `binding`, `open`; `split_card` takes `mandate`, `["array",
/// "null"]` of strings; each input closed. The outputs name `mandate` and
/// `subtask_items`.
#[test]
fn m7_the_inputs_and_outputs_hold_mandate() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    for name in ["create_card", "edit_card"] {
        let input = &tool(&list, name)["inputSchema"];
        assert_eq!(input["additionalProperties"], json!(false), "{name}");
        let mandate = property(&list, name, "mandate");
        assert_eq!(
            mandate["type"],
            json!(["string", "null"]),
            "{name}: {mandate}"
        );
        let words: Vec<&Value> = mandate["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}: an enum: {mandate}"))
            .iter()
            .filter(|word| !word.is_null())
            .collect();
        assert_eq!(words, [&json!("binding"), &json!("open")], "{name}");
        // The hand-written schema of the tools' `MandateArg` (the model's
        // `Mandate` takes no `schemars`): the two words and `null`, no
        // other key but its description.
        assert_eq!(mandate["enum"], json!(["binding", "open", null]), "{name}");
        let mut keys: Vec<&String> = mandate.as_object().expect("a schema").keys().collect();
        keys.sort();
        assert_eq!(keys, ["description", "enum", "type"], "{name}: {mandate}");
        let required = input["required"].as_array().cloned().unwrap_or_default();
        assert!(
            !required.contains(&json!("mandate")),
            "{name}: {required:?}"
        );
    }
    let split = &tool(&list, "split_card")["inputSchema"];
    assert_eq!(split["additionalProperties"], json!(false));
    let mandate = property(&list, "split_card", "mandate");
    assert_eq!(mandate["type"], json!(["array", "null"]), "{mandate}");
    assert_eq!(mandate["items"]["type"], json!("string"), "{mandate}");

    let get = &tool(&list, "get_card")["outputSchema"]["properties"];
    assert!(get.get("mandate").is_some(), "{get}");
    assert!(get.get("subtask_items").is_some(), "{get}");
    let text = tool(&list, "list_cards")["outputSchema"].to_string();
    assert!(text.contains("\"mandate\""), "list_cards' rows: {text}");
    let text = tool(&list, "split_card")["outputSchema"].to_string();
    assert!(
        text.contains("\"mandate\""),
        "split_card's children: {text}"
    );
    let text = tool(&list, "get_card")["outputSchema"].to_string();
    assert!(
        text.contains("\"card_id\""),
        "subtask_items' entries: {text}"
    );
    drop(session.finish());
}

// --------------------------------------------------------------- calls

/// M7: `create_card` with a reviewer and `mandate: "open"` stores it, its
/// `card.created` `as` `agent`; without a reviewer an error result with the
/// first line, nothing stored; `"maybe"` a bad argument. `edit_card
/// {card_id, mandate}` is an error result naming `agent`, nothing changed.
#[test]
fn m7_stdio_creates_with_a_mandate_and_never_edits_one() {
    let repo = Repo::new("mcp-mandate");
    let mut session = repo.session();
    let reply = session.call("create_card", parent_args());
    assert_ne!(result(&reply)["isError"], json!(true), "{reply}");

    let before = (repo.events(), repo.cards());
    let reply = session.call("create_card", hand_off_args(&json!({"mandate": "binding"})));
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(content_text(answer).contains(NO_REVIEWER), "{answer}");
    assert_eq!((repo.events(), repo.cards()), before, "nothing stored");
    for word in ["maybe", "Open"] {
        let reply = session.call(
            "create_card",
            hand_off_args(&json!({"reviewer": ANA, "mandate": word})),
        );
        assert_bad_arguments(&reply, "", word);
        assert_eq!((repo.events(), repo.cards()), before, "{word}");
    }

    let reply = session.call(
        "create_card",
        hand_off_args(&json!({"reviewer": ANA, "mandate": "open"})),
    );
    let answer = result(&reply);
    assert_ne!(answer["isError"], json!(true), "{answer}");
    assert_eq!(answer["structuredContent"]["id"], json!("C-0002"));
    assert_eq!(
        repo.events().last().cloned(),
        Some((
            "card.created".to_owned(),
            json!({"id": "C-0002", "flow": "sound", "column": "backlog", "position": 1,
                   "assignee": null, "reviewer": ANA, "mandate": "open", "as": "agent"})
        ))
    );
    let shown = session.call("get_card", json!({"card_id": "C-0002"}));
    assert_eq!(
        result(&shown)["structuredContent"]["mandate"],
        json!("open")
    );

    let before = (repo.events(), repo.cards());
    let reply = session.call(
        "edit_card",
        json!({"card_id": "C-0002", "mandate": "binding"}),
    );
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    assert!(
        content_text(answer).contains(
            "mandate: only C-0002's reviewer `Ana Ruiz` changes its mandate, not `agent`; \
             nothing changed"
        ),
        "{}",
        content_text(answer)
    );
    assert_eq!((repo.events(), repo.cards()), before, "nothing changed");
    let reply = session.call(
        "edit_card",
        json!({"card_id": "C-0002", "mandate": "maybe"}),
    );
    assert_bad_arguments(&reply, "", "maybe");
    assert_eq!(repo.status(), "", "the repository untouched");
    drop(session.finish());
}

/// M6, M7: each call equals `spec card` on a twin queue, content and
/// `structuredContent`, valid against its `outputSchema`: creates with and
/// without a mandate, a refused one, a refused edit, splits with mandates,
/// a refused split, items raised on the children, `get_card` of the parent
/// with its `subtask_items`, `list_cards`. M: `CardSubtaskItemShape`
/// without `card_id`.
#[test]
fn m7_each_call_equals_its_twin() {
    let repo = Repo::new("mcp-mandate-twin");
    let mut session = repo.session();
    let schemas = output_schemas(&session.tools());
    let twin = cli_env(&repo.root, Some(&repo.twin));
    let calls: Vec<(&str, Value)> = vec![
        ("create_card", parent_args()),
        (
            "create_card",
            hand_off_args(&json!({"reviewer": ANA, "mandate": "open"})),
        ),
        ("create_card", hand_off_args(&json!({"mandate": "binding"}))),
        (
            "create_card",
            hand_off_args(&json!({"reviewer": ANA, "mandate": null, "discipline": "art"})),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0002", "mandate": "binding"}),
        ),
        (
            "split_card",
            json!({"parent": "C-0001", "child": ["level-design", "production"],
                   "mandate": ["production=binding"], "reviewer": ANA}),
        ),
        (
            "split_card",
            json!({"parent": "C-0001", "child": ["production"],
                   "mandate": ["voice=open"], "reviewer": ANA}),
        ),
        (
            "split_card",
            json!({"parent": "C-0001", "child": ["production"], "mandate": ["production=open"],
                   "reviewer": ANA, "dry_run": true}),
        ),
    ];
    for (tool_name, args) in calls {
        let reply = session.call(tool_name, args.clone());
        let want = card_library(tool_name, &args, &twin, &Globals::default(), CLOCK);
        assert_parity(
            &reply,
            &want,
            &schemas[tool_name],
            &format!("{tool_name} {args}"),
        );
    }
    // Items on two children, then the parent's read: one read, each with
    // its `card_id`, valid against the schema.
    for (card, text) in [
        ("C-0003", "Is the keeper's coat oilskin?"),
        ("C-0002", "Which accent does the keeper speak?"),
    ] {
        let args = json!({"node_ids": ["SCN-STORM"], "text": text,
                          "working_answer": "Yes.", "price_of_other": "A scene redrawn.",
                          "card_id": card, "author_role": "developer"});
        let reply = session.call("ask_question", args.clone());
        let answer = result(&reply);
        assert_ne!(answer["isError"], json!(true), "{answer}");
    }
    // The reads, on the server's own queue.
    let own = cli_env(&repo.root, Some(&repo.home));
    for (tool_name, args) in [
        ("get_card", json!({"card_id": "C-0001"})),
        ("get_card", json!({"card_id": "C-0001", "history": true})),
        ("get_card", json!({"card_id": "C-0002"})),
        ("list_cards", json!({})),
    ] {
        let reply = session.call(tool_name, args.clone());
        let want = card_library(tool_name, &args, &own, &Globals::default(), CLOCK);
        assert_parity(
            &reply,
            &want,
            &schemas[tool_name],
            &format!("{tool_name} {args}"),
        );
    }
    let reply = session.call("get_card", json!({"card_id": "C-0001"}));
    let answer = result(&reply);
    assert_ne!(answer["isError"], json!(true), "{answer}");
    let document = &answer["structuredContent"];
    let listed: Vec<(&Value, &Value)> = document["subtask_items"]
        .as_array()
        .expect("subtask_items")
        .iter()
        .map(|item| (&item["card_id"], &item["id"]))
        .collect();
    assert_eq!(
        listed,
        [
            (&json!("C-0003"), &json!("PR-0001")),
            (&json!("C-0002"), &json!("PR-0002"))
        ],
        "{document}"
    );
    if let Err(problem) = conforms(&schemas["get_card"], document, "$") {
        panic!("M: CardSubtaskItemShape without card_id: {problem}");
    }
    assert_eq!(repo.status(), "");
    drop(session.finish());
}

// ---------------------------------------------------------------- texts

/// A seam that knows no login and runs nothing: enough to read the hub's
/// texts.
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

/// `text` with every run of whitespace one space.
fn folded(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// M7: the four tools' texts, stdio and hub: ASCII, at most 2 048 bytes,
/// the determinism sentence kept, "Data"'s sentence added (the hub's
/// `edit_card` naming the login); the instructions unchanged. The byte
/// counts are printed for "Implementation".
#[test]
fn m7_the_texts_hold_the_mandate() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let stdio = |name: &str| {
        tool(&list, name)["description"]
            .as_str()
            .expect("a description")
            .to_owned()
    };
    let hub = SpecEngineServer::hub(Arc::new(Idle));
    let hubs = |name: &str| {
        hub.get_tool(name)
            .unwrap_or_else(|| panic!("the hub routes {name}"))
            .description
            .as_deref()
            .expect("a description")
            .to_owned()
    };
    for (name, sentence) in [
        ("create_card", CREATE_TEXT),
        ("edit_card", EDIT_TEXT),
        ("split_card", SPLIT_TEXT),
        ("get_card", GET_TEXT),
    ] {
        for (label, text) in [("stdio", stdio(name)), ("hub", hubs(name))] {
            assert!(text.is_ascii(), "{label} {name}");
            assert!(
                text.len() <= TEXT_LIMIT,
                "{label} {name}: {} bytes",
                text.len()
            );
            assert!(text.contains(DETERMINISM), "{label} {name}");
            let folded = folded(&text);
            if name == "edit_card" && label == "hub" {
                assert!(
                    folded.contains("mandate: binding or open;") && folded.contains(EDIT_HUB_TEXT),
                    "{label} {name}: {text}"
                );
                assert!(!folded.contains("never agent"), "{label} {name}: {text}");
            } else {
                assert!(
                    folded.contains(sentence),
                    "{label} {name}: {sentence:?} not in {text}"
                );
            }
            eprintln!("{label} {name}: {} bytes", text.len());
        }
    }
    drop(session.finish());
    assert_eq!(specengine_mcp::INSTRUCTIONS.len(), TEXT_LIMIT);
    assert!(!specengine_mcp::INSTRUCTIONS.contains("mandate"));
    assert!(!specengine_mcp::HUB_INSTRUCTIONS.contains("mandate"));
}
