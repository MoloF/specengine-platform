//! docs/features/department-split.md (RM-0100, v5 step 6c, ADR-0078)
//! "Acceptance criteria", S8, the stdio half: `split_card` on the default
//! build's server over a scratch git repository of `fixtures/card-press` (a
//! print shop with two `[[card.split]]` rows), the canon
//! docs/canon/card-split.md "The rule".
//!
//! - `tools/list` holds thirty with `split_card`, its input
//!   `additionalProperties: false`, nine keys (`mandate` since task-mandate
//!   M7) and no `as`; its output the split document's five keys.
//! - A new parent with no `reviewer`: an error result naming `agent` (the
//!   chain ends at the acting name), the refusal's document, nothing stored.
//! - `reviewer` a person: stored, each `card.created` `as` `agent`, the
//!   document `spec card split --json`'s. M: the stdio tool acting as
//!   `local`.
//! - `dry_run` true: the plan, nothing stored. An unknown key and `as`:
//!   refused as bad arguments, nothing stored.
//! - The stdio and hub descriptions: ASCII, at most 2 048 bytes, the
//!   determinism sentence, the criterion's body; the hub's names none of
//!   the tools it leaves out. M: `split_card` missing from `HUB_TOOLS`.
//!
//! The wire's half on a hub (`/mcp` lists eighteen, the login acts) is
//! `specengine-http`'s `tests/card_split.rs`. Every server has a cleared
//! environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::path::PathBuf;
use std::sync::Arc;

use common::read::{
    Era, Session, TOOLS, assert_bad_arguments, assert_parity, card_library, cli_env, content_text,
    output_schemas,
};
use common::*;
use git::Sandbox;
use rmcp::ServerHandler;
use rmcp::model::Extensions;
use serde_json::{Value, json};
use specengine_cli::Globals;
use specengine_mcp::{Command, HUB_TOOLS, HubSeam, SpecEngineServer, TurnFuture};

const CLOCK: &str = "2026-11-11T09:00:00Z";

const ANA: &str = "Ana Ruiz";
const TITLE: &str = "The weekly run";
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";
const TEXT_LIMIT: usize = 2048;

/// The tools a hub leaves out (docs/canon/hub-mcp.md "Tools").
const LEFT_OUT: [&str; 12] = [
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

/// The body of `split_card`'s stdio text as the criterion words it, its
/// whitespace folded.
const BODY: &str = "Splits work to departments in one act: the same as `spec card split \
                    (--target REF... --title T [--goal T] [--discipline D] | --parent C) \
                    [--child D]... [--reviewer N] [--dry-run]`. It makes a parent card, or \
                    takes the stored card parent, and one child card per child discipline, \
                    each in the first column of the flow listing it, its parent the parent, \
                    its reviewer the split's. All cards or none. content is the command's \
                    lines, structuredContent its --json document {dry_run, parent, children, \
                    reviewer, notes}. target: at most 64 nodes, resolved as a task's. title: \
                    required without parent. parent: a stored card; then no target, title, \
                    goal or discipline. child: 1 to 32 disciplines, each once; none only with \
                    dry_run. reviewer: a person, never agent or local; absent: the parent \
                    card's. dry_run: plan only, nothing written; without child it plans every \
                    discipline. A child targets the nodes of the kinds a [[card.split]] row \
                    names for it, found in the targets or two levels under them, else the \
                    parent's targets; suggested marks a row's pick.";

/// `PRN-WEEKLY`'s scope as `spec tree PRN-WEEKLY --depth 2` serves it.
const SCOPE: [(&str, &str); 8] = [
    ("PRN-WEEKLY", "print-run"),
    ("PRN-WEEKLY-DEADLINE", "print-run"),
    ("PRN-PLATES", "print-run"),
    ("SHT-BACK", "broadsheet"),
    ("SHT-FRONT", "broadsheet"),
    ("SHT-FRONT-HEAD", "broadsheet"),
    ("SHT-INNER", "broadsheet"),
    ("SHT-INNER-FOLD", "broadsheet"),
];

fn of_kinds(kinds: &[&str]) -> Vec<&'static str> {
    SCOPE
        .iter()
        .filter(|(_, kind)| kinds.contains(kind))
        .map(|(id, _)| *id)
        .collect()
}

/// A scratch git repository of card-press, committed, and its `HOME`.
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
        let root = scratch.copy("card-press", "repo");
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

    /// Every event of the server's queue, `(type, payload)`; none before
    /// the queue exists.
    fn events(&self) -> Vec<(String, Value)> {
        let db = data_dir(&self.home).join("card-press.db");
        if !db.exists() {
            return Vec::new();
        }
        let output = std::process::Command::new("/usr/bin/sqlite3")
            .arg(&db)
            .arg("SELECT type || char(9) || payload FROM events ORDER BY seq;")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("sqlite3 runs");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout)
            .expect("UTF-8")
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

    /// The raw rows of `cards`, none before the queue exists.
    fn cards(&self) -> String {
        let db = data_dir(&self.home).join("card-press.db");
        if !db.exists() {
            return String::new();
        }
        let output = std::process::Command::new("/usr/bin/sqlite3")
            .arg(&db)
            .arg("SELECT * FROM cards ORDER BY id;")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("sqlite3 runs");
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).expect("UTF-8")
    }
}

/// `text` with every run of whitespace one space.
fn folded(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn split_args(extra: &Value) -> Value {
    let mut args = json!({"target": ["PRN-WEEKLY"], "title": TITLE, "discipline": "copy",
                          "child": ["copy", "plates"]});
    for (key, value) in extra.as_object().expect("an object") {
        args[key] = value.clone();
    }
    args
}

// --------------------------------------------------------------- tools

/// S8: stdio `tools/list` holds thirty, `split_card` among them; its input
/// is closed (`additionalProperties: false`), the eight keys of the
/// criterion and `mandate` (docs/features/task-mandate.md M7), each
/// nullable, none required, no `as`; it is no read; its
/// output the split document's keys.
#[test]
fn s8_tools_list_holds_split_card() {
    assert_eq!(TOOLS.len(), 30);
    assert!(TOOLS.contains(&"split_card"));
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    assert_eq!(tool_names(&list), TOOLS);
    let split = tool(&list, "split_card");
    let input = &split["inputSchema"];
    assert_eq!(input["additionalProperties"], json!(false), "{input}");
    let mut keys: Vec<&String> = input["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "child",
            "discipline",
            "dry_run",
            "goal",
            "mandate",
            "parent",
            "reviewer",
            "target",
            "title"
        ]
    );
    let required = input["required"].as_array().cloned().unwrap_or_default();
    assert!(required.is_empty(), "{required:?}");
    for (key, kind) in [
        ("target", "array"),
        ("child", "array"),
        ("title", "string"),
        ("goal", "string"),
        ("discipline", "string"),
        ("parent", "string"),
        ("reviewer", "string"),
        ("dry_run", "boolean"),
        // docs/features/task-mandate.md M7: each `D=M`.
        ("mandate", "array"),
    ] {
        assert_eq!(
            input["properties"][key]["type"],
            json!([kind, "null"]),
            "{key}: {input}"
        );
    }
    assert_ne!(
        split["annotations"]["readOnlyHint"],
        json!(true),
        "a write: {split}"
    );
    let mut out: Vec<&String> = split["outputSchema"]["properties"]
        .as_object()
        .expect("an output schema")
        .keys()
        .collect();
    out.sort();
    assert_eq!(out, ["children", "dry_run", "notes", "parent", "reviewer"]);
    drop(session.finish());
}

// --------------------------------------------------------------- calls

/// S8: a new parent with no `reviewer`: an error result naming `agent`, the
/// refusal's document, nothing stored; with a person the split is stored,
/// each `card.created` `as` `agent`, the document the split's. M: the
/// stdio tool acting as `local`.
#[test]
fn s8_a_stdio_split_acts_as_agent_and_needs_a_person() {
    let repo = Repo::new("mcp-split");
    let mut session = repo.session();
    let reply = session.call("split_card", split_args(&json!({})));
    let answer = result(&reply);
    assert_eq!(answer["isError"], json!(true), "{answer}");
    let reason = "reviewer: `agent` can be no reviewer: a person accepts a result";
    assert!(
        content_text(answer).contains(reason),
        "M: the stdio tool acting as `local`: {}",
        content_text(answer)
    );
    assert_eq!(
        answer["structuredContent"],
        json!({"dry_run": false, "parent": null, "children": [], "reviewer": null,
               "notes": [reason]})
    );
    assert!(repo.events().is_empty(), "nothing stored");
    assert_eq!(repo.cards(), "");

    let reply = session.call("split_card", split_args(&json!({"reviewer": ANA})));
    let answer = result(&reply);
    assert_ne!(answer["isError"], json!(true), "{answer}");
    assert_eq!(
        answer["structuredContent"],
        json!({
            "dry_run": false,
            "parent": {"id": "C-0001", "created": true, "title": TITLE, "discipline": "copy",
                       "flow": "copy", "flow_title": "Copy desk", "column": "draft",
                       "targets": ["PRN-WEEKLY"]},
            "children": [
                {"id": "C-0002", "discipline": "copy", "flow": "copy",
                 "flow_title": "Copy desk", "column": "draft",
                 "targets": of_kinds(&["broadsheet"]), "from": "kinds", "suggested": true, "mandate": null},
                {"id": "C-0003", "discipline": "plates", "flow": "plates",
                 "flow_title": "plates", "column": "queued",
                 "targets": of_kinds(&["broadsheet", "print-run"]), "from": "kinds",
                 "suggested": true, "mandate": null}
            ],
            "reviewer": ANA,
            "notes": []
        })
    );
    let events = repo.events();
    assert_eq!(events.len(), 3, "{events:?}");
    for ((kind, payload), id) in events.iter().zip(["C-0001", "C-0002", "C-0003"]) {
        assert_eq!(kind, "card.created");
        assert_eq!(payload["id"], json!(id));
        assert_eq!(payload["as"], json!("agent"), "{payload}");
        assert_eq!(payload["reviewer"], json!(ANA), "{payload}");
    }
    assert_eq!(repo.status(), "", "the repository untouched");
    drop(session.finish());
}

/// S8: `dry_run` true plans and stores nothing, the reviewer the chain's
/// unjudged; with no `child` it plans every discipline. An unknown key and
/// `as` are refused as bad arguments, nothing stored.
#[test]
fn s8_a_dry_run_and_a_bad_argument_store_nothing() {
    let repo = Repo::new("mcp-split-dry");
    let mut session = repo.session();
    let reply = session.call(
        "split_card",
        json!({"target": ["PRN-WEEKLY"], "title": TITLE, "discipline": "copy",
               "dry_run": true}),
    );
    let answer = result(&reply);
    assert_ne!(answer["isError"], json!(true), "{answer}");
    let document = &answer["structuredContent"];
    assert_eq!(document["dry_run"], json!(true), "{document}");
    assert_eq!(document["parent"]["id"], Value::Null);
    let planned: Vec<(&Value, &Value, &Value)> = document["children"]
        .as_array()
        .expect("children")
        .iter()
        .map(|child| (&child["id"], &child["discipline"], &child["suggested"]))
        .collect();
    assert_eq!(
        planned,
        [
            (&Value::Null, &json!("copy"), &json!(true)),
            (&Value::Null, &json!("plates"), &json!(true))
        ]
    );
    assert_eq!(document["reviewer"], json!("agent"), "unjudged: {document}");
    assert!(repo.events().is_empty(), "nothing stored");
    assert_eq!(repo.cards(), "");
    for extra in [
        json!({"reviewer": ANA, "owner": "ana"}),
        json!({"reviewer": ANA, "as": ANA}),
    ] {
        let reply = session.call("split_card", split_args(&extra));
        assert_bad_arguments(&reply, "", &format!("{extra}"));
        assert!(repo.events().is_empty(), "{extra}: nothing stored");
    }
    drop(session.finish());
}

/// S8: each call equals `spec card split --json` on a twin queue, content
/// and `structuredContent`, valid against the tool's `outputSchema`: a
/// refusal for want of a person, a dry run, a write, a split under the
/// stored parent, a child no flow takes, a repeated child.
#[test]
fn s8_each_call_equals_its_twin() {
    let repo = Repo::new("mcp-split-twin");
    let mut session = repo.session();
    let schemas = output_schemas(&session.tools());
    for args in [
        split_args(&json!({})),
        split_args(&json!({"dry_run": true, "child": null})),
        split_args(&json!({"reviewer": ANA})),
        json!({"parent": "C-0001", "child": ["plates"]}),
        json!({"parent": "C-0001", "child": ["plates", "nowhere"], "reviewer": ANA}),
        split_args(&json!({"reviewer": ANA, "child": ["copy", "copy"]})),
        json!({"parent": "C-0099", "child": ["plates"], "reviewer": ANA}),
    ] {
        let reply = session.call("split_card", args.clone());
        let want = card_library(
            "split_card",
            &args,
            &cli_env(&repo.root, Some(&repo.twin)),
            &Globals::default(),
            CLOCK,
        );
        assert_parity(&reply, &want, &schemas["split_card"], &format!("{args}"));
        assert_eq!(repo.status(), "", "{args}");
    }
    let created: Vec<String> = repo
        .events()
        .iter()
        .map(|(kind, payload)| format!("{kind} {}", payload["id"]))
        .collect();
    assert_eq!(
        created,
        [
            "card.created \"C-0001\"",
            "card.created \"C-0002\"",
            "card.created \"C-0003\"",
            "card.created \"C-0004\""
        ]
    );
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

/// S8: both `split_card` texts are ASCII, at most 2 048 bytes, hold the
/// determinism sentence and the criterion's body; the stdio text acts as
/// `agent`, the hub's as the login and names none of the twelve it leaves
/// out; `HUB_TOOLS` holds `split_card`, eighteen in all. M: `split_card`
/// missing from `HUB_TOOLS`.
#[test]
fn s8_both_texts_hold() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let described = |name: &str| {
        tool(&list, name)["description"]
            .as_str()
            .expect("a description")
            .to_owned()
    };
    let stdio = described("split_card");
    let stdio_create = described("create_card");
    drop(session.finish());
    assert_eq!(HUB_TOOLS.len(), 18);
    assert!(
        HUB_TOOLS.contains(&"split_card"),
        "M: split_card missing from HUB_TOOLS"
    );
    let hub = SpecEngineServer::hub(Arc::new(Idle));
    let hub_text = |name: &str| {
        hub.get_tool(name)
            .unwrap_or_else(|| panic!("the hub routes {name}"))
            .description
            .as_deref()
            .expect("a description")
            .to_owned()
    };
    let hubs = hub_text("split_card");
    let hub_create = hub_text("create_card");
    // The tails: each the card writes' own, from "You act as" on.
    let tail = |text: &str| {
        let at = text
            .find("You act as")
            .unwrap_or_else(|| panic!("no tail in {text}"));
        text[at..].to_owned()
    };
    assert_eq!(tail(&stdio), tail(&stdio_create), "the stdio tail");
    assert_eq!(tail(&hubs), tail(&hub_create), "the hub's tail");
    for (label, text) in [("stdio", &stdio), ("hub", &hubs)] {
        assert!(text.is_ascii(), "{label}");
        assert!(text.len() <= TEXT_LIMIT, "{label}: {} bytes", text.len());
        assert!(text.contains(DETERMINISM), "{label}");
        assert!(
            folded(text).starts_with(BODY),
            "{label}: the criterion's body:\n{text}"
        );
        eprintln!("{label} split_card: {} bytes", text.len());
    }
    assert!(stdio.contains("You act as `agent`"), "{stdio}");
    assert!(!hubs.contains("You act as `agent`"), "{hubs}");
    assert!(hubs.contains("You act as your"), "{hubs}");
    for name in LEFT_OUT {
        assert!(
            !hubs
                .split(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '_'))
                .any(|word| word == name),
            "the hub's text names {name}"
        );
    }
    assert_ne!(stdio, hubs);
    // The instructions (the orchestrator's decision, departing from the
    // criterion's "unchanged"): both cards lines name `split_card` and
    // `split`; the stdio text stays at its 2 048-byte cap.
    assert_eq!(specengine_mcp::INSTRUCTIONS.len(), TEXT_LIMIT);
    assert!(specengine_mcp::INSTRUCTIONS.contains(
        "- list_cards, get_card, create_card, split_card, edit_card, move_card = spec card \
         list|show|new|split|edit|move; no tool accepts.\n"
    ));
    assert!(specengine_mcp::HUB_INSTRUCTIONS.contains(
        "- list_cards, get_card, create_card, split_card, edit_card, move_card = spec card \
         list|show|new|split|edit|move;"
    ));
    assert!(specengine_mcp::HUB_INSTRUCTIONS.len() <= TEXT_LIMIT);
}
