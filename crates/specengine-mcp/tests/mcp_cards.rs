//! docs/features/card-store.md, the MCP half ("MCP", "Who acts"): the five
//! card tools of the default build, each answering as its `spec card` twin
//! on a twin queue (AC-10), on a scratch git repository of
//! `fixtures/card-press` and of `fixtures/spec-a`, `-b`, `-c`.
//!
//! - The tools: both reads read-only, the three writes annotated as
//!   `claim_task`; the twin's flags in snake case, `C` as `card_id`, no `as`;
//!   no tool assigns, cancels or accepts (AC-04); `INSTRUCTIONS` 2 048 bytes
//!   with the cards line.
//! - A tool acts as `agent` (AC-03): `by = ["desk"]` refuses it naming `desk`,
//!   a flow listing `agent` admits it, the event records it.
//! - `move_card` never reaches `accept.to` (AC-04's M).
//! - A refusal keeps its document, no such card and an exit 2 have none;
//!   nothing asked is an exit 2 (docs/features/board-ui.md: its `{id, reason}`
//!   is the daemon's alone).
//! - `copy`, `plates` and the default flow of three projects run one tool
//!   code (AC-10); no reply names a stack word.
//! - The MCP sources hold no `card_accept`, `card_assign`, `card_cancel`,
//!   `card_clear`.
//! - board-handoff (RM-0085), docs/canon/card-flows.md "MCP and hub": on
//!   `fixtures/studio-lamp`, `create_card {reviewer}`, `move_card {note}`,
//!   `list_cards {parent}` each equal to their twin; `reviewer: "agent"` and a
//!   move into `sent-back` without a note refused, nothing stored; the
//!   descriptions name the keys.
//!
//! - card-tasks (RM-0092), docs/canon/cards.md "MCP", CT-1
//!   and CT-2, on `fixtures/studio-lamp` with `T-0001`, `T-0002` in both
//!   queues: `edit_card {task}` equal to `spec card edit --task` on a twin
//!   queue, one `card.edited` naming `tasks` acting as `agent`, `["-"]`
//!   clearing; no such task, a repeat, no task ID and a look-alike refused as
//!   the CLI refuses them, nothing stored; `get_card` serving `tasks` and the
//!   task notes byte for byte as `spec card show --marks --json`; the stdio
//!   descriptions holding "Data"'s sentences.
//!
//! - departments (RM-0096), docs/features/departments.md DP-4, on
//!   `fixtures/card-press`: `list_cards`' `structuredContent` equals the
//!   CLI's document and validates against its `outputSchema`, whose flow
//!   object requires `title`, a string; `copy` titled "Copy desk", untitled
//!   `plates` by its name.
//!
//! Every server has a cleared environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::{Path, PathBuf};

use common::read::{
    CARD_TOOLS, ERAS, Era, STACK_WORDS, Session, TOOLS, assert_bad_arguments, assert_parity,
    card_library, checked_call, cli_env, content_text, has_word, output_schemas, server_git,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::Globals;

const CLOCK: &str = "2026-10-30T09:00:00Z";

/// "MCP": the cards line, its LF included, 112 bytes; 130 since
/// docs/features/department-split.md "Acceptance criteria", S8 (`split_card`
/// and `split` named, the orchestrator's decision).
const CARDS_LINE: &str = "- list_cards, get_card, create_card, split_card, edit_card, move_card = \
                          spec card list|show|new|split|edit|move; no tool accepts.\n";

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
/// `list_cards`'; `INSTRUCTIONS` 2 048 bytes holding the cards line once and
/// naming every tool. M: a tool taking `as`; a sixth tool.
#[test]
fn ac10_the_card_tools_and_their_schemas() {
    // docs/features/department-split.md "Acceptance criteria", S8: thirty.
    // 32 since docs/features/collision-pass.md CP8 (`get_collisions`,
    // `mark_pairs`).
    assert_eq!(TOOLS.len(), 32);
    for name in CARD_TOOLS {
        assert!(TOOLS.contains(&name), "{name}");
    }
    assert_eq!(CARDS_LINE.len(), 130);
    let instructions = specengine_mcp::INSTRUCTIONS;
    // 2 039 since docs/features/collision-pass.md CP8.
    assert_eq!(instructions.len(), 2039);
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
                    "marks",
                    "no_target",
                    // docs/canon/card-flows.md "Children".
                    "parent",
                    "with_cancelled",
                ],
                vec![],
            ),
            (
                "get_card",
                // docs/features/card-context.md CC7: `context`, `budget`.
                vec!["budget", "card_id", "context", "history"],
                vec!["card_id"],
            ),
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
                    // docs/features/task-mandate.md M7.
                    "mandate",
                    "parent",
                    // docs/canon/card-flows.md "Hand-off".
                    "reviewer",
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
                    // docs/features/task-mandate.md M7.
                    "mandate",
                    "parent",
                    "reason",
                    "revision",
                    "target",
                    // docs/canon/cards.md "MCP".
                    "task",
                    "title",
                ],
                vec!["card_id"],
            ),
            (
                "move_card",
                // docs/canon/card-flows.md "Return": `note`.
                vec!["before", "card_id", "note", "revision", "to"],
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
            json!({"id": "C-0001", "from": "draft", "to": "edit", "position": 1, "as": "agent",
                   "note": null})
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
/// no `card_accept`, `card_assign`, `card_cancel`, `card_clear`, in any
/// file, comments too. M: a tool calling one.
#[test]
fn ac04_the_mcp_sources_hold_no_persons_act() {
    let dir = repository_root().join("crates/specengine-mcp/src");
    let mut read = 0;
    for entry in fs::read_dir(dir).expect("src") {
        let path = entry.expect("an entry").path();
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        read += 1;
        for name in ["card_accept", "card_assign", "card_cancel", "card_clear"] {
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

/// docs/features/departments.md (RM-0096) DP-4: on card-press, in both eras,
/// `list_cards`' flow object requires `title`, a string, after `name`;
/// `structuredContent` equals the CLI's document (`spec card list --json`
/// through the library) and validates against the `outputSchema`, empty and
/// with a card. M: the mirror without `title`.
#[test]
fn departments_dp4_list_cards_serves_each_flow_s_title() {
    for era in ERAS {
        let repo = Repo::new("mc-titles", "card-press");
        let mut session = repo.session(era);
        let list = session.tools();
        let schemas = output_schemas(&list);
        let flow = &schemas["list_cards"]["properties"]["flows"]["items"];
        assert_eq!(
            flow["properties"]["title"]["type"],
            json!("string"),
            "{era:?}: {flow}"
        );
        let required: Vec<&str> = flow["required"]
            .as_array()
            .expect("required")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(required.contains(&"title"), "{era:?}: {required:?}");
        let names: Vec<&String> = flow["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        assert!(names.contains(&&"title".to_owned()), "{era:?}: {names:?}");
        let env = cli_env(&repo.root, Some(&repo.home));
        let globals = Globals::default();
        for step in ["empty", "a card"] {
            if step == "a card" {
                session.call("create_card", json!({"title": "One", "discipline": "copy"}));
            }
            let read = checked_call(
                &mut session,
                &schemas,
                "list_cards",
                json!({}),
                &env,
                &globals,
            );
            let flows = &read["structuredContent"]["flows"];
            assert_eq!(flows[0]["name"], "copy", "{era:?} {step}");
            assert_eq!(flows[0]["title"], "Copy desk", "{era:?} {step}");
            assert_eq!(flows[1]["title"], "plates", "{era:?} {step}: its name");
        }
        drop(session.finish());
    }
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

// -------------------------------------------------- board-handoff (RM-0085)

/// The refusal of a move into the return column with no note, as the
/// tool's `content` gives it (docs/canon/card-flows.md "Return").
fn needs_note(id: &str) -> String {
    format!(
        "spec: {id} enters `sent-back`, its flow's return column: say why with `--note`; nothing \
         changed\n"
    )
}

/// board-handoff BH-12, BH-14, BH-19 (docs/canon/card-flows.md "MCP and hub"), on
/// `fixtures/studio-lamp`: `create_card {reviewer}`, `move_card {note}` and
/// `list_cards {parent}` each equal to their `spec card` twin on a twin queue,
/// the repository clean after each: a hand-off stores its reviewer and
/// `card.created` carries it, the tool acting as `agent`; `reviewer: "agent"`
/// is refused with nothing stored; a move into `sent-back` without a note is
/// refused with the write document and no event, with one it moves and
/// `card.moved` holds the note; `parent` lists exactly the children, none
/// for a card with none, a look-alike exit 2's line with no document.
#[test]
fn board_handoff_the_card_tools_take_reviewer_note_and_parent() {
    let repo = Repo::new("mc-handoff", "studio-lamp");
    let mut session = repo.session(Era::Stateless);
    let schemas = output_schemas(&session.tools());
    let calls = [
        (
            "create_card",
            json!({"title": "Lamp room scene", "discipline": "narrative"}),
        ),
        (
            "create_card",
            json!({"title": "Lamp room dialogue", "discipline": "development",
                   "parent": "C-0001", "reviewer": "Ann"}),
        ),
        (
            "create_card",
            json!({"title": "Lamp glow", "discipline": "art", "parent": "C-0001",
                   "reviewer": "Ann"}),
        ),
        (
            "create_card",
            json!({"title": "No one's", "discipline": "development", "reviewer": "agent"}),
        ),
        (
            "create_card",
            json!({"title": "No one's", "discipline": "development", "reviewer": " local "}),
        ),
        (
            "create_card",
            json!({"title": "No one's", "discipline": "development", "reviewer": "a\u{2028}b"}),
        ),
        (
            "move_card",
            json!({"card_id": "C-0002", "to": "in-progress"}),
        ),
        ("move_card", json!({"card_id": "C-0002", "to": "sent-back"})),
        (
            "move_card",
            json!({"card_id": "C-0002", "to": "sent-back", "note": "   "}),
        ),
        (
            "move_card",
            json!({"card_id": "C-0002", "to": "sent-back", "note": "Two endings contradict SCN-LAMP-ROOM"}),
        ),
        (
            "move_card",
            json!({"card_id": "C-0003", "to": "sent-back", "note": "Too dark"}),
        ),
        // A reorder inside the return column (`to` its own column): no note.
        ("move_card", json!({"card_id": "C-0003", "to": "sent-back"})),
        (
            "move_card",
            json!({"card_id": "C-0002", "to": "ready", "note": "Fixed"}),
        ),
    ];
    for (tool, args) in calls {
        twin_call(&repo, &mut session, &schemas, tool, args);
        assert_eq!(repo.status(), "", "{tool}");
    }
    let events = repo.events();
    assert_eq!(
        events,
        [
            (
                "card.created".to_owned(),
                json!({"id": "C-0001", "flow": "narrative", "column": "backlog", "position": 1,
                       "assignee": null, "reviewer": null, "mandate": null, "as": "agent"})
            ),
            (
                "card.created".to_owned(),
                json!({"id": "C-0002", "flow": "development", "column": "backlog",
                       "position": 1, "assignee": null, "reviewer": "Ann", "mandate": null,
                       "as": "agent"})
            ),
            (
                "card.created".to_owned(),
                json!({"id": "C-0003", "flow": "art", "column": "waiting", "position": 1,
                       "assignee": null, "reviewer": "Ann", "mandate": null, "as": "agent"})
            ),
            (
                "card.moved".to_owned(),
                json!({"id": "C-0002", "from": "backlog", "to": "in-progress", "position": 1,
                       "as": "agent", "note": null})
            ),
            (
                "card.moved".to_owned(),
                json!({"id": "C-0002", "from": "in-progress", "to": "sent-back",
                       "position": 1, "as": "agent",
                       "note": "Two endings contradict SCN-LAMP-ROOM"})
            ),
            (
                "card.moved".to_owned(),
                json!({"id": "C-0003", "from": "waiting", "to": "sent-back", "position": 1,
                       "as": "agent", "note": "Too dark"})
            ),
            (
                "card.moved".to_owned(),
                json!({"id": "C-0003", "from": "sent-back", "to": "sent-back", "position": 1,
                       "as": "agent", "note": null})
            ),
            (
                "card.moved".to_owned(),
                json!({"id": "C-0002", "from": "sent-back", "to": "ready", "position": 1,
                       "as": "agent", "note": "Fixed"})
            ),
        ],
        "the refused calls stored nothing"
    );
    // The refusals' own words, beyond parity with the twin.
    let card = session.call("create_card", json!({"title": "T", "reviewer": "agent"}));
    let card = result(&card);
    assert_eq!(card["isError"], json!(true), "{card}");
    assert_eq!(
        content_text(card),
        "spec: reviewer: `agent` can be no reviewer: a person accepts a result\n"
    );
    let back = session.call("move_card", json!({"card_id": "C-0002", "to": "sent-back"}));
    let back = result(&back);
    assert_eq!(back["isError"], json!(true), "{back}");
    assert_eq!(content_text(back), needs_note("C-0002"));
    assert_eq!(
        back["structuredContent"]["column"],
        json!("ready"),
        "{back}"
    );
    assert_eq!(
        back["structuredContent"]["notes"]
            .as_array()
            .and_then(|notes| notes.last()),
        Some(&json!(
            needs_note("C-0002")
                .trim_start_matches("spec: ")
                .trim_end_matches('\n')
        )),
        "{back}"
    );
    assert_eq!(repo.events().len(), events.len(), "no event");

    // `parent`, on the server's own queue.
    let env = cli_env(&repo.root, Some(&repo.home));
    let globals = Globals::default();
    for (tool, args) in [
        ("list_cards", json!({"parent": "C-0001"})),
        ("list_cards", json!({"parent": "C-0002"})),
        ("list_cards", json!({"parent": "C-9999"})),
        ("list_cards", json!({"parent": "C-0001", "flow": "art"})),
        ("list_cards", json!({"parent": "\u{0421}-0001"})),
        ("list_cards", json!({"parent": "C-1"})),
    ] {
        checked_call(&mut session, &schemas, tool, args, &env, &globals);
    }
    let children = session.call("list_cards", json!({"parent": "C-0001"}));
    let ids: Vec<&str> = result(&children)["structuredContent"]["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|row| row["id"].as_str().expect("an ID"))
        .collect();
    assert_eq!(ids, ["C-0002", "C-0003"], "exactly its children");
    assert_eq!(
        result(&children)["structuredContent"]["flows"]
            .as_array()
            .map(Vec::len),
        // docs/features/department-split.md S9: the preset's six flows.
        Some(6),
        "every flow"
    );
    assert_eq!(
        result(&children)["structuredContent"]["flows"][1]["return"],
        json!("sent-back"),
        "a flow's return column"
    );
    let none = session.call("list_cards", json!({"parent": "C-0002"}));
    assert_eq!(result(&none)["isError"], json!(false), "{none}");
    assert_eq!(result(&none)["structuredContent"]["cards"], json!([]));
    let alike = session.call("list_cards", json!({"parent": "\u{0421}-0001"}));
    let alike = result(&alike);
    assert_eq!(alike["isError"], json!(true), "{alike}");
    assert!(alike.get("structuredContent").is_none(), "{alike}");
    assert_eq!(
        content_text(alike),
        "spec: `\u{0421}-0001` uses look-alike letters or digits; card IDs are Latin only: write \
         `C-0001`\n"
    );
    drop(session.finish());
    assert_eq!(repo.status(), "");
}

/// docs/canon/card-flows.md "MCP and hub": the stdio descriptions
/// name the new keys: `create_card`'s `reviewer`, `move_card`'s `note`,
/// `list_cards`' `parent` and a flow's `return`; `INSTRUCTIONS` stays at its
/// 2 048 bytes (the cards line unchanged). The hub's texts, ASCII and within
/// 2 048 B, are `specengine-http`'s `tests/hub_mcp.rs` (H11).
#[test]
fn board_handoff_the_descriptions_name_the_new_keys() {
    let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
    let list = session.tools();
    for (name, words) in [
        ("create_card", &["reviewer", "parent"][..]),
        ("move_card", &["note", "return"][..]),
        ("list_cards", &["parent", "return"][..]),
    ] {
        let description = tool(&list, name)["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: a description"));
        for word in words {
            assert!(
                has_word(description, word),
                "{name} names `{word}`: {description}"
            );
        }
    }
    // 2 039 since docs/features/collision-pass.md CP8.
    assert_eq!(specengine_mcp::INSTRUCTIONS.len(), 2039);
    drop(session.finish());
}

// ---------------------------------------------------- card-tasks (RM-0092)

/// The sentence `EDIT_DESCRIPTION` adds (card-tasks, CT-1).
const EDIT_TASK_SENTENCE: &str = "task: the T- tasks it links, each an existing task of this \
                                  queue, listed once, at most 32; it replaces the list, - clears \
                                  it. The link decides nothing: no move, accept or gate reads it.";

/// The sentence `GET_DESCRIPTION` adds, shared by stdio and a hub.
const GET_TASKS_SENTENCE: &str = "tasks: each linked task {id, status, title, stale}, read now as \
                                  spec task list reads it; status null, with a note that starts \
                                  with its ID, for a task this server cannot read: a corrupt row, \
                                  another repository, a hub.";

/// `spec task new --nodes node --title title` through the library on the
/// queue of `home` at [`CLOCK`]: its ID.
fn task_in(repo: &Repo, home: &Path, node: &str, title: &str) -> String {
    let env = cli_env(&repo.root, Some(home));
    let outcome = specengine_cli::task_new(
        &env,
        &Globals::default(),
        &specengine_cli::TaskNewRequest {
            nodes: vec![node.to_owned()],
            title: Some(title.to_owned()),
            goal: None,
            author_role: None,
            author_model: None,
            run: None,
            now: CLOCK.to_owned(),
            git: server_git(&env),
        },
    )
    .unwrap_or_else(|error| panic!("task new {node}: {error}"));
    outcome.id.expect("a task's ID")
}

/// CT-1 and CT-2, the tools' half: on studio-lamp, `T-0001` and `T-0002`
/// made in the server's queue and the twin's, `edit_card {task}` answers as
/// `spec card edit --task` on the twin (a refusal's document and line too)
/// and stores the same list and event: `fields: ["tasks"]` (`["goal",
/// "tasks"]` beside `goal`), acting as `agent`; `["-"]` clears. `get_card`'s
/// `content` and `structuredContent` are `spec card show C --marks --json`'s,
/// each `{id, status, title, stale}`, each task's note after `<id>: `. The
/// repository stays clean.
#[test]
fn card_tasks_edit_card_links_and_get_card_reads_each_task() {
    let repo = Repo::new("mc-tasks", "studio-lamp");
    for home in [&repo.home, &repo.twin] {
        assert_eq!(
            task_in(&repo, home, "ISL-LAMP", "Light the north lamp"),
            "T-0001"
        );
        assert_eq!(
            task_in(&repo, home, "ISL-LAMP-ROOM", "Wire the switch"),
            "T-0002"
        );
    }
    let mut session = repo.session(Era::Stateless);
    let schemas = output_schemas(&session.tools());
    let env = cli_env(&repo.root, Some(&repo.home));
    let globals = Globals::default();
    let writes = [
        (
            "create_card",
            json!({"title": "Light the lamp", "discipline": "development",
                   "target": ["ISL-LAMP"]}),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "task": ["T-0001", "T-0002"]}),
        ),
    ];
    for (tool, args) in writes {
        twin_call(&repo, &mut session, &schemas, tool, args);
        assert_eq!(repo.status(), "", "{tool}");
    }
    let shown = checked_call(
        &mut session,
        &schemas,
        "get_card",
        json!({"card_id": "C-0001"}),
        &env,
        &globals,
    );
    let document = &shown["structuredContent"];
    assert_eq!(
        document["tasks"],
        json!([
            {"id": "T-0001", "status": "draft", "title": "Light the north lamp", "stale": null},
            {"id": "T-0002", "status": "draft", "title": "Wire the switch", "stale": null},
        ]),
        "{shown}"
    );
    // The key order is the CLI's (`card_tasks.rs`); here `serde_json` sorts.
    let notes = document["notes"].as_array().expect("notes");
    for (index, id) in ["T-0001", "T-0002"].iter().enumerate() {
        assert!(
            notes[index]
                .as_str()
                .is_some_and(|note| note.starts_with(&format!("{id}: "))),
            "{notes:?}"
        );
    }

    for (tool, args) in [
        (
            "edit_card",
            json!({"card_id": "C-0001", "task": ["T-0002", "T-0001"],
                   "goal": "Before the storm."}),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "task": ["T-0099"]}),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "task": ["T-0001", "T-0001"]}),
        ),
        ("edit_card", json!({"card_id": "C-0001", "task": ["foo"]})),
        (
            "edit_card",
            json!({"card_id": "C-0001", "task": ["T-0001", "-"]}),
        ),
        (
            "edit_card",
            json!({"card_id": "C-0001", "task": ["\u{0422}-0001"]}),
        ),
    ] {
        twin_call(&repo, &mut session, &schemas, tool, args);
        assert_eq!(repo.status(), "", "{tool}");
    }
    let refused = session.call(
        "edit_card",
        json!({"card_id": "C-0001", "task": ["T-0099"]}),
    );
    let refused = result(&refused);
    assert_eq!(refused["isError"], json!(true), "{refused}");
    assert_eq!(
        content_text(refused),
        "spec: task: no task T-0099; nothing changed\n"
    );
    checked_call(
        &mut session,
        &schemas,
        "get_card",
        json!({"card_id": "C-0001", "history": true}),
        &env,
        &globals,
    );
    twin_call(
        &repo,
        &mut session,
        &schemas,
        "edit_card",
        json!({"card_id": "C-0001", "task": ["-"]}),
    );
    let cleared = checked_call(
        &mut session,
        &schemas,
        "get_card",
        json!({"card_id": "C-0001"}),
        &env,
        &globals,
    );
    assert_eq!(
        cleared["structuredContent"]["tasks"],
        json!([]),
        "{cleared}"
    );
    let edited: Vec<(String, Value)> = repo
        .events()
        .into_iter()
        .filter(|(kind, _)| kind == "card.edited")
        .collect();
    assert_eq!(
        edited,
        [
            (
                "card.edited".to_owned(),
                json!({"id": "C-0001", "fields": ["tasks"], "as": "agent"})
            ),
            (
                "card.edited".to_owned(),
                json!({"id": "C-0001", "fields": ["goal", "tasks"], "as": "agent"})
            ),
            (
                "card.edited".to_owned(),
                json!({"id": "C-0001", "fields": ["tasks"], "as": "agent"})
            ),
        ],
        "the refused calls stored nothing"
    );
    drop(session.finish());
    assert_eq!(repo.status(), "");
}

/// docs/canon/cards.md "MCP", D15: the stdio `edit_card`
/// names `task` and holds its sentence, `get_card` holds the `tasks`
/// sentence; both stay ASCII within 2 048 bytes, 29 tools (30 since
/// department-split, S8).
#[test]
fn card_tasks_the_stdio_descriptions_name_task() {
    // 32 since docs/features/collision-pass.md CP8 (`get_collisions`,
    // `mark_pairs`).
    assert_eq!(TOOLS.len(), 32);
    let mut session = Session::open(Era::Legacy, &[], None, Home::Fresh);
    let list = session.tools();
    for (name, word, sentence) in [
        ("edit_card", "task", EDIT_TASK_SENTENCE),
        ("get_card", "tasks", GET_TASKS_SENTENCE),
    ] {
        let description = tool(&list, name)["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: a description"));
        assert!(has_word(description, word), "{name}: {description}");
        assert!(
            description.contains(sentence),
            "{name} holds {sentence:?}: {description}"
        );
        assert!(description.is_ascii(), "{name}");
        assert!(description.len() <= 2048, "{name}: {}", description.len());
    }
    assert!(
        tool(&list, "edit_card")["inputSchema"]["properties"]
            .get("task")
            .is_some(),
        "edit_card takes task"
    );
    drop(session.finish());
}

// ---------------------------------------------- card-questions (RM-0097)

/// docs/features/card-questions.md L2, `get_card` over stdio on
/// `fixtures/studio-lamp`, both eras: a card with a question and a decision
/// raised for it serves `items`, by ID, each of `CardItemShape`'s 16 keys,
/// `author` an agent's; the result equals `spec card show --json` and is
/// valid against the tool's `outputSchema`, an item's `why` and the
/// question's `decision_note` included. M: `CardItemShape` without
/// `decision_note`.
#[test]
fn card_questions_get_card_serves_items_valid_against_its_schema() {
    for era in ERAS {
        let repo = Repo::new("mc-cq-items", "studio-lamp");
        let mut session = repo.session(era);
        let list = session.tools();
        let schemas = output_schemas(&list);
        let item = &schemas["get_card"]["properties"]["items"]["items"];
        let mut keys: Vec<&String> = item["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{era:?}: an item's properties: {item}"))
            .keys()
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "author",
                "created_at",
                "decided_at",
                "decided_by",
                "decision_note",
                // docs/features/plain-questions.md PQ10.
                "gist",
                "id",
                "kind",
                "matters",
                "record_id",
                "record_path",
                "record_title",
                "staged_at",
                "status",
                "summary",
                "updated_at",
                "why",
                "working_answer"
            ],
            "{era:?}"
        );
        let made = session.call(
            "create_card",
            json!({"title": "The storm scene", "discipline": "narrative",
                "target": ["SCN-STORM"]}),
        );
        assert_eq!(result(&made)["structuredContent"]["id"], json!("C-0001"));
        let asked = session.call(
            "ask_question",
            json!({"node_ids": ["SCN-STORM"], "text": "Which bell rings at dusk?",
                "working_answer": "The harbour bell.", "price_of_other": "Two scenes.",
                "card_id": "C-0001", "author_role": "writer"}),
        );
        assert_eq!(result(&asked)["structuredContent"]["id"], json!("PR-0001"));
        let decided = session.call(
            "report_decision",
            json!({"node_ids": ["SCN-STORM"], "text": "Does the lamp burn oil?",
                "working_answer": "Yes.", "price_of_other": "The lamp room redrawn.",
                "why": "The keeper's log says so.", "card_id": "C-0001",
                "author_role": "developer"}),
        );
        assert_eq!(
            result(&decided)["structuredContent"]["id"],
            json!("PR-0002")
        );
        let env = cli_env(&repo.root, Some(&repo.home));
        let read = checked_call(
            &mut session,
            &schemas,
            "get_card",
            json!({"card_id": "C-0001"}),
            &env,
            &Globals::default(),
        );
        let items = &read["structuredContent"]["items"];
        let ids: Vec<&str> = items
            .as_array()
            .expect("items")
            .iter()
            .map(|entry| entry["id"].as_str().expect("an ID"))
            .collect();
        assert_eq!(ids, ["PR-0001", "PR-0002"], "{era:?}");
        assert_eq!(items[1]["why"], json!("The keeper's log says so."));
        assert_eq!(items[0]["author"]["type"], json!("agent"));
        assert_eq!(items[0]["author"]["role"], json!("writer"));
        assert_eq!(items[0]["decision_note"], Value::Null);
        drop(session.finish());
    }
}
