//! docs/features/card-store.md, the MCP half ("MCP", "Who acts"): the five
//! card tools of the default build, each answering as its `spec card` twin
//! on a twin queue (AC-10), on a scratch git repository of
//! `fixtures/card-press` and of `fixtures/spec-a`, `-b`, `-c`.
//!
//! - The tools: both reads read-only, the three writes annotated as
//!   `claim_task`; the twin's flags in snake case, `C` as `card_id`, no `as`;
//!   no tool assigns, cancels or accepts (AC-04); `INSTRUCTIONS` 2 047 bytes
//!   with the cards line.
//! - A tool acts as `agent` (AC-03): `by = ["desk"]` refuses it naming `desk`,
//!   a flow listing `agent` admits it, the event records it.
//! - `move_card` never reaches `accept.to` (AC-04's M).
//! - A refusal keeps its document, no such card and an exit 2 have none;
//!   nothing asked is an exit 2 (docs/features/board-ui.md: its `{id, reason}`
//!   is the daemon's alone).
//! - `copy`, `plates` and the default flow of three projects run one tool
//!   code (AC-10); no reply names a stack word.
//! - The MCP sources hold no `card_accept`, `card_assign`, `card_cancel`.
//!
//! Every server has a cleared environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::read::{
    CARD_TOOLS, ERAS, Era, STACK_WORDS, Session, TOOLS, assert_bad_arguments, assert_parity,
    card_library, checked_call, cli_env, content_text, has_word, output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::Globals;

const CLOCK: &str = "2026-10-30T09:00:00Z";

/// "MCP": the cards line, its LF included, 112 bytes.
const CARDS_LINE: &str = "- list_cards, get_card, create_card, edit_card, move_card = spec card \
                          list|show|new|edit|move; no tool accepts.\n";

/// A scratch git repository of `fixture`, committed, and two `HOME`s.
struct Repo {
    /// Removed on drop, with the repository in it.
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
    twin: PathBuf,
}

impl Repo {
    fn new(label: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy(fixture, "repo");
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

    /// `git status --porcelain --ignored`.
    fn status(&self) -> String {
        self.git
            .git_text(&self.root, &["status", "--porcelain", "--ignored"])
    }

    fn session(&self, era: Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }

    /// The `card.*` events of the server's queue, `(type, payload)`.
    fn events(&self) -> Vec<(String, Value)> {
        let db = data_dir(&self.home).join(format!("{}.db", self.slug()));
        let output = std::process::Command::new("/usr/bin/sqlite3")
            .arg(&db)
            .arg("SELECT type || char(9) || payload FROM events WHERE type LIKE 'card.%' ORDER BY seq;")
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

    /// The `[project] slug` of the config, as written.
    fn slug(&self) -> String {
        let text = fs::read_to_string(self.root.join("specengine.toml")).expect("config");
        let line = text
            .lines()
            .find(|line| line.starts_with("slug"))
            .expect("a slug line");
        line.split('"').nth(1).expect("a quoted slug").to_owned()
    }
}

/// One tool call, the answer of its twin on the twin queue, checked; the
/// tool result.
fn twin_call(
    repo: &Repo,
    session: &mut Session,
    schemas: &std::collections::BTreeMap<String, Value>,
    tool: &str,
    args: Value,
) -> Value {
    let reply = session.call(tool, args.clone());
    let want = card_library(
        tool,
        &args,
        &cli_env(&repo.root, Some(&repo.twin)),
        &Globals::default(),
        CLOCK,
    );
    assert_parity(
        &reply,
        &want,
        &schemas[tool],
        &format!("{:?} {tool} {args}", session.era),
    );
    result(&reply).clone()
}

// ---------------------------------------------------------------- tools

/// The input property `name` of `tool`.
fn input_keys(list: &Value, name: &str) -> Vec<String> {
    let mut keys: Vec<String> = tool(list, name)["inputSchema"]["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn required(list: &Value, name: &str) -> Vec<String> {
    tool(list, name)["inputSchema"]["required"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().expect("a name").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// "MCP": five tools, the twin's flags in snake case, `card_id`, no `as`, no
/// tool for `assign`, `cancel`, `accept`; reads read-only, writes annotated
/// as `claim_task`; the write document's seven keys, `get_card`'s and
/// `list_cards`'; `INSTRUCTIONS` 2 047 bytes holding the cards line once and
/// naming every tool. M: a tool taking `as`; a sixth tool.
#[test]
fn ac10_the_card_tools_and_their_schemas() {
    assert_eq!(TOOLS.len(), 28);
    for name in CARD_TOOLS {
        assert!(TOOLS.contains(&name), "{name}");
    }
    assert_eq!(CARDS_LINE.len(), 112);
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert_eq!(instructions.len(), 2047);
    assert_eq!(instructions.matches(CARDS_LINE).count(), 1);
    assert!(
        instructions.contains(&format!(
            "show|claim|plan|report|complete.\n{CARDS_LINE}\nREF:"
        )),
        "after the tasks line"
    );
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        let names = tool_names(&list);
        assert_eq!(names, TOOLS, "{era:?}");
        for name in &names {
            assert!(
                !name.contains("accept") && !name.contains("assign") && !name.contains("cancel"),
                "{era:?}: a tool for the person's acts: {name}"
            );
        }
        for (name, want, needed) in [
            (
                "list_cards",
                vec![
                    "assignee",
                    "column",
                    "discipline",
                    "flow",
                    "no_target",
                    "with_cancelled",
                ],
                vec![],
            ),
            ("get_card", vec!["card_id", "history"], vec!["card_id"]),
            (
                "create_card",
                vec![
                    "assignee",
                    "corrects",
                    "criterion",
                    "discipline",
                    "due",
                    "estimate",
                    "goal",
                    "parent",
                    "target",
                    "title",
                ],
                vec!["title"],
            ),
            (
                "edit_card",
                vec![
                    "card_id",
                    "corrects",
                    "criterion",
                    "due",
                    "estimate",
                    "evidence",
                    "goal",
                    "parent",
                    "revision",
                    "target",
                    "title",
                ],
                vec!["card_id"],
            ),
            (
                "move_card",
                vec!["before", "card_id", "revision", "to"],
                vec!["card_id"],
            ),
        ] {
            assert_eq!(input_keys(&list, name), want, "{era:?} {name}");
            assert_eq!(required(&list, name), needed, "{era:?} {name}");
        }
        for name in ["list_cards", "get_card"] {
            assert_eq!(
                tool(&list, name)["annotations"]["readOnlyHint"],
                json!(true),
                "{name}"
            );
        }
        let claim = tool(&list, "claim_task")["annotations"].clone();
        for name in ["create_card", "edit_card", "move_card"] {
            assert_eq!(tool(&list, name)["annotations"], claim, "{era:?} {name}");
            let mut keys: Vec<&String> = tool(&list, name)["outputSchema"]["properties"]
                .as_object()
                .expect("properties")
                .keys()
                .collect();
            keys.sort();
            assert_eq!(
                keys,
                [
                    "allowed", "column", "flow", "id", "notes", "position", "revision"
                ],
                "{era:?} {name}"
            );
        }
        let mut keys: Vec<&String> = tool(&list, "list_cards")["outputSchema"]["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            ["cards", "flows", "notes", "omitted", "truncated"],
            "{era:?}"
        );
        drop(session.finish());
    }
}

/// `tool` with `arguments` a bad-arguments error, in the session's
/// protocol.
fn assert_bad(session: &mut Session, tool: &str, arguments: Value) {
    let context = format!("{tool} {arguments}");
    let reply = session.call(tool, arguments);
    assert_bad_arguments(&reply, "", &context);
}

/// "MCP": `as` and a person's flags are no arguments of a tool: a bad call,
/// nothing stored.
#[test]
fn ac03_no_tool_takes_as_or_a_persons_flag() {
    let repo = Repo::new("mc-args", "card-press");
    let mut session = repo.session(Era::Stateless);
    let made = session.call("create_card", json!({"title": "One", "discipline": "copy"}));
    assert_eq!(result(&made)["isError"], json!(false), "{made}");
    assert_bad(
        &mut session,
        "move_card",
        json!({"card_id": "C-0001", "to": "edit", "as": "desk"}),
    );
    assert_bad(
        &mut session,
        "edit_card",
        json!({"card_id": "C-0001", "goal": "G.", "as": "desk"}),
    );
    assert_bad(
        &mut session,
        "create_card",
        json!({"title": "T", "as": "desk"}),
    );
    assert_bad(
        &mut session,
        "edit_card",
        json!({"card_id": "C-0001", "assignee": "ana"}),
    );
    assert_bad(
        &mut session,
        "edit_card",
        json!({"card_id": "C-0001", "reviewer": "ben"}),
    );
    drop(session.finish());
    assert_eq!(repo.events().len(), 1, "only the first call wrote");
}

/// AC-10: on `card-press`, `create_card`, `get_card`, `list_cards`,
/// `edit_card`, `move_card`, refused and answered, each equal to its twin on
/// a twin queue in another `HOME`: `content` the command's `2>&1`,
/// `structuredContent` its `--json` document, `isError` its exit; the
/// repository clean after each. M: a tool dropping a flag.
#[test]
fn ac10_each_tool_equals_its_twin() {
    let repo = Repo::new("mc-twin", "card-press");
    let mut session = repo.session(Era::Stateless);
    let schemas = output_schemas(&session.tools());
    let calls = [
        (
            "create_card",
            json!({"title": "Lead", "goal": "Ten inches.", "discipline": "copy",
                   "target": ["SHT-FRONT", "docs/spec/sht-back.md"], "criterion": ["Two."],
                   "assignee": "ana", "estimate": "2-5", "due": "2026-11-05"}),
        ),
        (
            "create_card",
            json!({"title": "Plate", "discipline": "plates", "parent": "C-0001",
                   "corrects": "C-0001"}),
        ),
        ("create_card", json!({"title": "No trade"})),
        (
            "create_card",
            json!({"title": "Bad", "due": "soon", "discipline": "copy"}),
        ),
        (
            "create_card",
            json!({"title": "Bad", "target": ["SHT-NOPE"], "discipline": "copy"}),
        ),
        (
            "create_card",
            json!({"title": "Bad", "parent": "\u{0421}-0001"}),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "goal": "Nine.", "evidence": ["note=Read."]}),
        ),
        ("edit_card", json!({"card_id": "C-0001", "goal": "-"})),
        (
            "edit_card",
            json!({"card_id": "C-0001", "evidence": ["file=x"]}),
        ),
        ("edit_card", json!({"card_id": "C-0001"})),
        ("edit_card", json!({"card_id": "C-0099", "goal": "G."})),
        (
            "edit_card",
            json!({"card_id": "C-0001", "goal": "G.", "revision": "3"}),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "goal": "G.", "revision": "x"}),
        ),
        ("move_card", json!({"card_id": "C-0002", "to": "blocked"})),
        ("move_card", json!({"card_id": "C-0002", "to": "struck"})),
        ("move_card", json!({"card_id": "C-0001", "to": "edit"})),
        ("move_card", json!({"card_id": "C-0001", "to": "proof"})),
        ("move_card", json!({"card_id": "C-0001", "to": "filed"})),
        ("move_card", json!({"card_id": "C-0001"})),
        ("move_card", json!({"card_id": "C-0099", "to": "edit"})),
        (
            "move_card",
            json!({"card_id": "\u{0421}-0001", "to": "edit"}),
        ),
        (
            "move_card",
            json!({"card_id": "C-0002", "before": "C-0001"}),
        ),
    ];
    for (tool, args) in calls {
        twin_call(&repo, &mut session, &schemas, tool, args);
        assert_eq!(repo.status(), "", "{tool}");
    }
    // A lost compare-and-set names the row's own clock, the server's here.
    let card = session.call("get_card", json!({"card_id": "C-0002"}));
    let at = result(&card)["structuredContent"]["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_owned();
    let stale = session.call(
        "move_card",
        json!({"card_id": "C-0002", "to": "queued", "revision": "1"}),
    );
    let stale = result(&stale);
    assert_eq!(stale["isError"], json!(true), "{stale}");
    assert_eq!(
        content_text(stale),
        format!(
            "spec: `C-0002` changed since this run read it: it is in blocked since {at}; nothing \
             changed\n"
        )
    );
    // The reads, on the server's own queue.
    let env = cli_env(&repo.root, Some(&repo.home));
    let globals = Globals::default();
    for (tool, args) in [
        ("list_cards", json!({})),
        ("list_cards", json!({"flow": "plates"})),
        (
            "list_cards",
            json!({"column": "draft", "discipline": "copy"}),
        ),
        ("list_cards", json!({"assignee": "ana", "no_target": true})),
        ("list_cards", json!({"with_cancelled": true})),
        ("list_cards", json!({"flow": "nope"})),
        ("get_card", json!({"card_id": "C-0001"})),
        ("get_card", json!({"card_id": "C-0002", "history": true})),
        ("get_card", json!({"card_id": "C-0099"})),
        ("get_card", json!({"card_id": "\u{0421}-0001"})),
    ] {
        checked_call(&mut session, &schemas, tool, args, &env, &globals);
    }
    drop(session.finish());
    assert_eq!(repo.status(), "");
}

/// "CLI", "MCP": a refusal keeps the write's document in both results; no
/// such card has no `structuredContent` and the reason; a look-alike is exit
/// 2's one line naming the Latin form.
#[test]
fn refusals_read_as_the_clis() {
    let repo = Repo::new("mc-refused", "card-press");
    let mut session = repo.session(Era::Legacy);
    session.call(
        "create_card",
        json!({"title": "Lead", "discipline": "copy"}),
    );
    let refused = session.call("move_card", json!({"card_id": "C-0001", "to": "proof"}));
    let refused = result(&refused);
    assert_eq!(refused["isError"], json!(true));
    assert_eq!(
        content_text(refused),
        "spec: C-0001 is in draft: no move to `proof`; allowed: `edit`; nothing changed\n"
    );
    assert_eq!(refused["structuredContent"]["column"], json!("draft"));
    assert_eq!(refused["structuredContent"]["revision"], json!("1"));
    assert_eq!(
        refused["structuredContent"]["allowed"],
        json!([{"to": "edit", "by": ["desk"]}])
    );

    let none = session.call("get_card", json!({"card_id": "C-0099"}));
    let none = result(&none);
    assert_eq!(none["isError"], json!(true));
    assert!(none.get("structuredContent").is_none(), "{none}");
    assert!(content_text(none).contains("no card C-0099"), "{none}");

    let alike = session.call("get_card", json!({"card_id": "\u{0421}-0001"}));
    let alike = result(&alike);
    assert_eq!(alike["isError"], json!(true));
    assert!(alike.get("structuredContent").is_none(), "{alike}");
    assert!(content_text(alike).contains("`C-0001`"), "{alike}");
    drop(session.finish());
}

/// docs/features/board-ui.md "Files in scope": nothing asked of `move_card`
/// or `edit_card` is an error result with the CLI's one line and no
/// `structuredContent`: no tool's output schema holds `{id, reason}`.
#[test]
fn nothing_asked_has_no_document() {
    let repo = Repo::new("mc-nothing", "card-press");
    let mut session = repo.session(Era::Legacy);
    session.call(
        "create_card",
        json!({"title": "Lead", "discipline": "copy"}),
    );
    for (tool, args, line) in [
        (
            "move_card",
            json!({"card_id": "C-0001"}),
            "spec: `spec card move` asks nothing: give --to K, --before C or both\n",
        ),
        (
            "move_card",
            json!({"card_id": "C-0001", "revision": "1"}),
            "spec: `spec card move` asks nothing: give --to K, --before C or both\n",
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001"}),
            "spec: `spec card edit` asks nothing: give a field to change or --evidence\n",
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "revision": "1"}),
            "spec: `spec card edit` asks nothing: give a field to change or --evidence\n",
        ),
    ] {
        let reply = session.call(tool, args);
        let reply = result(&reply);
        assert_eq!(reply["isError"], json!(true), "{tool}: {reply}");
        assert!(reply.get("structuredContent").is_none(), "{tool}: {reply}");
        assert_eq!(content_text(reply), line, "{tool}");
    }
    let shown = session.call("get_card", json!({"card_id": "C-0001"}));
    assert_eq!(
        result(&shown)["structuredContent"]["revision"],
        json!("1"),
        "nothing changed"
    );
    drop(session.finish());
}

// -------------------------------------------------------------- who acts

/// AC-03: a tool acts as `agent`: `by = ["desk"]` refuses `move_card` naming
/// `desk`, the row and the events as they were; a flow listing `agent`
/// admits it; each event records `agent`. M: `by` ignored.
#[test]
fn ac03_a_tool_acts_as_agent_against_by() {
    let repo = Repo::new("mc-by", "card-press");
    let mut session = repo.session(Era::Stateless);
    session.call(
        "create_card",
        json!({"title": "Lead", "discipline": "copy"}),
    );
    assert_eq!(repo.events()[0].1["as"], "agent");
    let refused = session.call("move_card", json!({"card_id": "C-0001", "to": "edit"}));
    let refused = result(&refused);
    assert_eq!(refused["isError"], json!(true), "{refused}");
    assert!(content_text(refused).contains("`desk`"), "{refused}");
    assert_eq!(repo.events().len(), 1, "no event");
    drop(session.finish());

    let config = repo.root.join("specengine.toml");
    let text = fs::read_to_string(&config).expect("config");
    fs::write(
        &config,
        text.replace("by = [\"desk\"]", "by = [\"desk\", \"agent\"]"),
    )
    .expect("config");
    let mut session = repo.session(Era::Stateless);
    let moved = session.call("move_card", json!({"card_id": "C-0001", "to": "edit"}));
    assert_eq!(result(&moved)["isError"], json!(false), "{moved}");
    assert_eq!(content_text(result(&moved)), "moved C-0001: copy/edit\n");
    drop(session.finish());
    let events = repo.events();
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[1],
        (
            "card.moved".to_owned(),
            json!({"id": "C-0001", "from": "draft", "to": "edit", "position": 1, "as": "agent"})
        )
    );
}

/// AC-04: no tool reaches `accept.to`: `move_card` to it exits 1 with `spec
/// card accept` named, in the flow with `moves` and in the one without; the
/// card stays. M: `move_card` reaching `accept.to`.
#[test]
fn ac04_move_card_never_reaches_accept_to() {
    let repo = Repo::new("mc-accept", "card-press");
    let mut session = repo.session(Era::Stateless);
    session.call(
        "create_card",
        json!({"title": "Plate", "discipline": "plates"}),
    );
    let moved = session.call("move_card", json!({"card_id": "C-0001", "to": "checked"}));
    assert_eq!(result(&moved)["isError"], json!(false), "{moved}");
    let refused = session.call("move_card", json!({"card_id": "C-0001", "to": "struck"}));
    let refused = result(&refused);
    assert_eq!(refused["isError"], json!(true), "{refused}");
    assert_eq!(
        content_text(refused),
        "spec: C-0001 is in checked: no move to `struck`; allowed: `queued`, `blocked`, `spec \
         card accept`; nothing changed\n"
    );
    let card = session.call("get_card", json!({"card_id": "C-0001"}));
    let card = &result(&card)["structuredContent"];
    assert_eq!(card["column"], json!("checked"));
    assert_eq!(card["acceptance"], Value::Null);
    let allowed: Vec<&str> = card["allowed"]
        .as_array()
        .expect("allowed")
        .iter()
        .map(|entry| entry["to"].as_str().expect("a column"))
        .collect();
    assert_eq!(
        allowed,
        ["queued", "blocked"],
        "moves only, never accept.to"
    );
    drop(session.finish());
}

/// AC-04: the sources of the MCP crate hold no way to the person's acts:
/// no `card_accept`, `card_assign`, `card_cancel`, in any file, comments
/// too. M: a tool calling one.
#[test]
fn ac04_the_mcp_sources_hold_no_persons_act() {
    let dir = repository_root().join("crates/specengine-mcp/src");
    let mut read = 0;
    for entry in fs::read_dir(dir).expect("src") {
        let path = entry.expect("an entry").path();
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        read += 1;
        for name in ["card_accept", "card_assign", "card_cancel"] {
            assert!(!text.contains(name), "{}: {name}", path.display());
        }
    }
    assert!(read > 5, "{read} sources read");
}

// --------------------------------------------------------------- genres

/// AC-10: `copy`, `plates`, `spec-a`, `-b`, `-c`: one walk through the tools,
/// up to `accept.from`, the same lines; `list_cards` shows it; no reply names
/// a stack word. M: a branch on a fixture's word.
#[test]
fn ac10_five_flows_run_one_tool_code() {
    for (label, fixture, discipline, flow, open, target) in [
        (
            "copy",
            "card-press",
            Some("copy"),
            "copy",
            &["edit", "proof"][..],
            "SHT-FRONT",
        ),
        (
            "plates",
            "card-press",
            Some("plates"),
            "plates",
            &["blocked", "checked"][..],
            "SHT-BACK",
        ),
        (
            "spec-a",
            "spec-a",
            None,
            "plain",
            &["ready", "in-progress", "review"][..],
            "MEC-STAMINA",
        ),
        (
            "spec-b",
            "spec-b",
            None,
            "plain",
            &["ready", "in-progress", "review"][..],
            "REQ-001",
        ),
        (
            "spec-c",
            "spec-c",
            None,
            "plain",
            &["ready", "in-progress", "review"][..],
            "FEE-RIDE",
        ),
    ] {
        let repo = Repo::new(&format!("mc-genre-{label}"), fixture);
        if label == "copy" {
            let config = repo.root.join("specengine.toml");
            let text = fs::read_to_string(&config).expect("config");
            fs::write(&config, text.replace("by = [\"desk\"]", "by = [\"agent\"]"))
                .expect("config");
            repo.git.add_all(&repo.root);
            repo.git.commit(&repo.root, "tools move the copy");
        }
        let mut session = repo.session(Era::Stateless);
        let schemas = output_schemas(&session.tools());
        let mut texts = Vec::new();
        let mut args = json!({"title": "One walk", "target": [target]});
        if let Some(discipline) = discipline {
            args["discipline"] = json!(discipline);
        }
        let made = twin_call(&repo, &mut session, &schemas, "create_card", args);
        assert_eq!(made["isError"], json!(false), "{label}: {made}");
        texts.push(made["content"].to_string());
        for column in open {
            let moved = twin_call(
                &repo,
                &mut session,
                &schemas,
                "move_card",
                json!({"card_id": "C-0001", "to": column}),
            );
            assert_eq!(
                content_text(&moved),
                format!("moved C-0001: {flow}/{column}\n"),
                "{label}"
            );
            texts.push(moved["content"].to_string());
        }
        let listed = session.call("list_cards", json!({}));
        let listed = result(&listed);
        assert_eq!(
            listed["structuredContent"]["cards"][0]["column"],
            json!(open.last().unwrap()),
            "{label}"
        );
        texts.push(listed["content"].to_string());
        for text in &texts {
            for word in STACK_WORDS {
                assert!(!has_word(text, word), "{label}: {word}: {text}");
            }
        }
        drop(session.finish());
        assert_eq!(repo.status(), "", "{label}");
        assert_eq!(repo.events().len(), 1 + open.len(), "{label}");
    }
}

/// AC-05, the tools' half: a bad `[card]` is an error result with its one
/// line for the card tools alone: `get_tree` answers.
#[test]
fn a_bad_card_table_stops_the_card_tools_alone() {
    let repo = Repo::new("mc-bad", "spec-a");
    let config = repo.root.join("specengine.toml");
    let mut text = fs::read_to_string(&config).expect("config");
    text.push_str("\n[[card.flow]]\nname = \"Bad\"\ncolumns = [\"a\", \"b\"]\n");
    fs::write(&config, text).expect("config");
    let mut session = repo.session(Era::Stateless);
    for (tool, args) in [
        ("list_cards", json!({})),
        ("get_card", json!({"card_id": "C-0001"})),
        ("create_card", json!({"title": "T"})),
        ("edit_card", json!({"card_id": "C-0001", "goal": "G."})),
        ("move_card", json!({"card_id": "C-0001", "to": "ready"})),
    ] {
        let reply = session.call(tool, args);
        let reply = result(&reply);
        assert_eq!(reply["isError"], json!(true), "{tool}: {reply}");
        assert!(reply.get("structuredContent").is_none(), "{tool}: {reply}");
        assert!(
            content_text(reply).contains("specengine.toml:"),
            "{tool}: {reply}"
        );
    }
    let tree = session.call("get_tree", json!({}));
    assert_eq!(result(&tree)["isError"], json!(false), "{tree}");
    drop(session.finish());
}

/// AC-10, one state, one result: `list_cards` and `get_card` twice give
/// byte-identical results; a read writes no event.
#[test]
fn reads_repeat_and_write_nothing() {
    let repo = Repo::new("mc-repeat", "card-press");
    let mut session = repo.session(Era::Stateless);
    session.call("create_card", json!({"title": "One", "discipline": "copy"}));
    let events = repo.events();
    for (tool, args) in [
        ("list_cards", json!({})),
        ("get_card", json!({"card_id": "C-0001", "history": true})),
    ] {
        let first = session.call(tool, args.clone());
        let second = session.call(tool, args);
        assert_eq!(result(&first), result(&second), "{tool}");
    }
    assert_eq!(repo.events(), events, "no event from a read");
    drop(session.finish());
}

/// "Who acts": a repository with no `.git` serves the tools too: no git.
#[test]
fn the_tools_need_no_git() {
    let scratch = Scratch::new("mc-nogit");
    let home = scratch.home("h");
    let root = scratch.copy("card-press", "copy");
    let mut session = Session::open(Era::Stateless, &[], Some(&root), Home::At(&home));
    let made = session.call(
        "create_card",
        json!({"title": "One", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    assert_eq!(result(&made)["isError"], json!(false), "{made}");
    assert_eq!(content_text(result(&made)), "created C-0001: copy/draft\n");
    let listed = session.call("list_cards", json!({}));
    assert_eq!(result(&listed)["isError"], json!(false), "{listed}");
    drop(session.finish());
    assert!(!root.join(".git").exists());
}
