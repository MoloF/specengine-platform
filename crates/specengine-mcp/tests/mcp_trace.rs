//! docs/features/card-trace.md, the tool: `get_trace {ref?}`, a read, answers
//! what the CLI library answers for the same request (text = `stderr_lines` +
//! `render_text`; exit 2 = the `CliError` line; `structuredContent` = the
//! parsed `render_json`, absent on exit 2; `isError` on exit 1 and 2), in both
//! protocol eras, over a scratch git repository of `fixtures/card-press` with
//! a confirmed, a suspect and a departed node, and every document meets the
//! tool's output schema under the strict walker of `common/read.rs`.
//!
//! - AC-09: suspect and departed rows are no error, the bytes the CLI's.
//! - AC-10: `tools/list` holds no clear; the sources hold no way to one.
//! - "Re-estimating": `edit_card` gains `reason?`, acting as `agent`.
//!
//! The door (`mcp_door.rs`), the genre scan (`mcp_genre.rs`) and the
//! `INSTRUCTIONS` pins hold the rest.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::read::{
    ERAS, Era, Session, TOOLS, checked_call, cli_env, conforms, content_text, output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    CardAcceptRequest, CardAssignRequest, CardCancelRequest, CardMoveRequest, CardNewRequest, Env,
    Globals, card_accept, card_assign, card_cancel, card_move, card_new,
};

const CLOCK: &str = "2026-10-30T09:00:00Z";

/// `get_trace`'s reads-line clause (docs/features/card-trace.md).
const TRACE_READ: &str = "; get_trace(ref?) = trace";

/// A scratch git repository of `fixtures/card-press`, committed.
struct Repo {
    /// Removed on drop, with the repository in it.
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
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

    fn session(&self, era: Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }

    /// A card of the `copy` flow on `targets`, at `proof`, `ben` its reviewer:
    /// its ID.
    fn in_proof(&self, title: &str, targets: &[&str]) -> String {
        let (env, globals) = (self.env(), Globals::default());
        let made = card_new(
            &env,
            &globals,
            &CardNewRequest {
                title: title.to_owned(),
                discipline: Some("copy".to_owned()),
                targets: targets.iter().map(|target| (*target).to_owned()).collect(),
                now: CLOCK.to_owned(),
                ..CardNewRequest::default()
            },
        )
        .expect("a card");
        let id = made.card.expect("a card").id;
        card_assign(
            &env,
            &globals,
            &CardAssignRequest {
                id: id.clone(),
                reviewer: Some("ben".to_owned()),
                now: CLOCK.to_owned(),
                ..CardAssignRequest::default()
            },
        )
        .expect("assigned");
        for (column, actor) in [("edit", "desk"), ("proof", "desk")] {
            card_move(
                &env,
                &globals,
                &CardMoveRequest {
                    id: id.clone(),
                    to: Some(column.to_owned()),
                    actor: Some(actor.to_owned()),
                    now: CLOCK.to_owned(),
                    ..CardMoveRequest::default()
                },
            )
            .expect("moved");
        }
        id
    }

    /// `id` accepted by `ben`, `request` the verdict.
    fn accept(&self, id: &str, request: CardAcceptRequest) {
        let outcome = card_accept(
            &self.env(),
            &Globals::default(),
            &CardAcceptRequest {
                id: id.to_owned(),
                actor: "ben".to_owned(),
                now: CLOCK.to_owned(),
                ..request
            },
            &mut |_| true,
        )
        .expect("the accept");
        assert_eq!(outcome.refusal, None, "{outcome:?}");
    }

    /// `from` for `to` in `path`, by hand, committed.
    fn hand_edit(&self, path: &str, from: &str, to: &str) {
        replace(&self.root, path, from, to);
        self.git.add_all(&self.root);
        self.git.commit(&self.root, "by hand");
    }

    /// `git status --porcelain --ignored`, `HEAD`.
    fn git_side(&self) -> (String, String) {
        (
            self.git
                .git_text(&self.root, &["status", "--porcelain", "--ignored"]),
            self.git.git_text(&self.root, &["rev-parse", "HEAD"]),
        )
    }

    /// The `card.*` events of the queue, `(type, payload)`.
    fn events(&self) -> Vec<(String, Value)> {
        let db = data_dir(&self.home).join("card-press.db");
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
}

/// A board with a confirmed node, a suspect one and a departed one.
fn marked(label: &str) -> Repo {
    let repo = Repo::new(label);
    let lead = repo.in_proof("Lead story", &["SHT-FRONT"]);
    repo.accept(
        &lead,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
    let notices = repo.in_proof("Notices", &["SHT-BACK"]);
    repo.accept(
        &notices,
        CardAcceptRequest {
            briefed: true,
            ..CardAcceptRequest::default()
        },
    );
    let long = repo.in_proof("Long read", &["SHT-INNER"]);
    repo.accept(
        &long,
        CardAcceptRequest {
            departs: vec!["SHT-INNER".to_owned()],
            summary: Some("The result left the brief.".to_owned()),
            observed: Some("The lead runs below the fold.".to_owned()),
            documented: Some("The lead runs above the fold.".to_owned()),
            ..CardAcceptRequest::default()
        },
    );
    repo.hand_edit("docs/spec/sht-back.md", "the tide table", "the moon table");
    repo
}

/// Every call of `calls` in both eras against the CLI and the schema; the
/// first era's results.
fn parity(repo: &Repo, calls: &[Value]) -> Vec<Value> {
    let env = repo.env();
    let globals = Globals::default();
    let mut first = Vec::new();
    for (round, era) in ERAS.into_iter().enumerate() {
        let mut session = repo.session(era);
        let schemas = output_schemas(&session.tools());
        for args in calls {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_trace",
                args.clone(),
                &env,
                &globals,
            );
            if round == 0 {
                first.push(result);
            }
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    first
}

/// AC-09: a whole read, a tree, each state's node, each form of `ref`: the
/// CLI's bytes, a schema-true document, suspect and departed rows no error;
/// git as it was: a read.
#[test]
fn ac09_get_trace_equals_the_cli_within_its_schema() {
    let repo = marked("mcp-trace");
    let git = repo.git_side();
    let results = parity(
        &repo,
        &[
            json!({}),
            json!({"ref": null}),
            json!({"ref": "PRN-WEEKLY"}),
            json!({"ref": "SHT-FRONT"}),
            json!({"ref": "SHT-BACK"}),
            json!({"ref": "SHT-INNER"}),
            json!({"ref": "SHT-FRONT-HEAD"}),
            json!({"ref": "docs/spec/sht-inner.md"}),
            json!({"ref": "SHT-FRONT#SHT-FRONT-HEAD"}),
        ],
    );
    assert_eq!(repo.git_side(), git, "a read touches no file");
    let whole = &results[2];
    assert_eq!(whole["isError"], json!(false), "{whole}");
    let text = content_text(whole);
    assert!(
        text.starts_with(
            "trace PRN-WEEKLY | nodes 5, cards 3 | no card 2, in work 0, suspect 1, departed 1, \
             confirmed 1\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("  card C-0002 | copy/filed | suspect SHT-BACK | Notices\n"),
        "{text}"
    );
    assert!(
        text.contains("  card C-0003 | copy/filed | departed PR-0001 | Long read\n"),
        "{text}"
    );
    assert_eq!(
        whole["structuredContent"]["counts"],
        json!({"no_card": 2, "in_work": 0, "suspect": 1, "departed": 1, "confirmed": 1})
    );
}

/// "Exit 1 only a REF naming nothing, 2 as `spec flow`": a REF naming nothing
/// is an error result with the document, what cannot run has none; a wrong
/// type or an unknown field is a bad argument.
#[test]
fn a_ref_naming_nothing_is_an_error_with_its_document() {
    let repo = Repo::new("mcp-trace-exits");
    let results = parity(
        &repo,
        &[
            json!({"ref": "SHT-NOPE"}),
            json!({"ref": "S\u{041d}T-FRONT"}),
        ],
    );
    assert_eq!(results[0]["isError"], json!(true), "{}", results[0]);
    assert_eq!(results[0]["structuredContent"]["ref"], json!("SHT-NOPE"));
    assert!(results[0]["structuredContent"]["reason"].is_string());
    assert_eq!(results[1]["isError"], json!(true), "{}", results[1]);
    assert!(results[1].get("structuredContent").is_none());
}

/// "Rules": no queue: `no card`, no table made, and the tool's document meets
/// its schema, which requires each key: a node without `rollup`, or a card
/// without `mark`, does not conform.
#[test]
fn no_queue_reads_no_card_within_the_schema() {
    let repo = Repo::new("mcp-trace-empty");
    let results = parity(&repo, &[json!({}), json!({"ref": "PRN-WEEKLY"})]);
    assert_eq!(
        results[0]["structuredContent"]["counts"],
        json!({"no_card": 6, "in_work": 0, "suspect": 0, "departed": 0, "confirmed": 0})
    );

    let repo = marked("mcp-trace-schema");
    let results = parity(&repo, &[json!({"ref": "SHT-FRONT"})]);
    let mut session = repo.session(ERAS[0]);
    let schemas = output_schemas(&session.tools());
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    let schema = &schemas["get_trace"];
    let served = &results[0]["structuredContent"];
    assert_eq!(conforms(schema, served, "get_trace"), Ok(()));
    let mut without = served.clone();
    assert!(
        without["nodes"][0]
            .as_object_mut()
            .expect("a node")
            .remove("rollup")
            .is_some()
    );
    assert!(
        conforms(schema, &without, "get_trace").is_err(),
        "{without}"
    );
    let mut without = served.clone();
    assert!(
        without["nodes"][0]["cards"][0]
            .as_object_mut()
            .expect("a card")
            .remove("mark")
            .is_some()
    );
    assert!(
        conforms(schema, &without, "get_trace").is_err(),
        "{without}"
    );
}

/// AC-10: `tools/list` holds no clear, in both eras, by name or description,
/// and `get_trace` is a read: `readOnlyHint`, one optional input, `ref`.
/// `INSTRUCTIONS` holds its clause on the reads' line, 2 040 bytes.
#[test]
fn ac10_no_tool_clears_and_get_trace_is_a_read() {
    assert_eq!(TOOLS.len(), 29);
    assert!(TOOLS.contains(&"get_trace"));
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert_eq!(instructions.len(), 2040);
    assert_eq!(
        instructions.matches(TRACE_READ).count(),
        1,
        "{instructions}"
    );
    assert!(
        !instructions.contains("evidence computed, never typed"),
        "cut for room"
    );
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}");
        for entry in list["tools"].as_array().expect("tools") {
            let name = entry["name"].as_str().expect("a name");
            assert!(!name.contains("clear"), "{era:?}: {name}");
            let description = entry["description"].as_str().unwrap_or_default();
            assert!(
                !description.contains("card clear"),
                "{era:?}: {name}: {description}"
            );
        }
        let trace = tool(&list, "get_trace");
        assert_eq!(trace["annotations"]["readOnlyHint"], json!(true), "{trace}");
        assert_eq!(
            trace["inputSchema"]["properties"]
                .as_object()
                .expect("properties")
                .keys()
                .collect::<Vec<_>>(),
            ["ref"],
            "{trace}"
        );
        assert_eq!(trace["inputSchema"].get("required"), None, "{trace}");
        drop(session.finish());
    }
}

/// AC-10: the sources of the MCP crate hold no way to a clear.
#[test]
fn ac10_the_mcp_sources_hold_no_clear() {
    let dir = repository_root().join("crates/specengine-mcp/src");
    let mut read = 0;
    for entry in fs::read_dir(dir).expect("src") {
        let path = entry.expect("an entry").path();
        let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        read += 1;
        for name in ["card_clear", "clear_card", "clear_cards", "CardClear"] {
            assert!(!text.contains(name), "{}: {name}", path.display());
        }
    }
    assert!(read > 5, "{read} sources read");
}

/// "Re-estimating": `edit_card` takes `reason`: a card holding an estimate
/// refuses a change without it, nothing else beside it, and records
/// `card.reestimated` as `agent`; a first estimate needs none.
#[test]
fn edit_card_takes_a_reason() {
    let repo = Repo::new("mcp-trace-estimate");
    let mut session = repo.session(Era::Stateless);
    let list = session.tools();
    let keys = tool(&list, "edit_card")["inputSchema"]["properties"]
        .as_object()
        .expect("properties")
        .clone();
    assert!(keys.contains_key("reason"), "{keys:?}");
    let reply = session.call(
        "create_card",
        json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    let reply = session.call("edit_card", json!({"card_id": "C-0001", "estimate": "2"}));
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    let refused = session.call("edit_card", json!({"card_id": "C-0001", "estimate": "3"}));
    assert_eq!(result(&refused)["isError"], json!(true), "{refused}");
    assert!(
        content_text(result(&refused)).contains("--reason"),
        "{refused}"
    );
    let beside = session.call(
        "edit_card",
        json!({"card_id": "C-0001", "estimate": "3", "reason": "Bigger.", "goal": "G."}),
    );
    assert_eq!(result(&beside)["isError"], json!(true), "{beside}");
    let reply = session.call(
        "edit_card",
        json!({"card_id": "C-0001", "estimate": "3", "reason": "Bigger."}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    drop(session.finish());
    assert_eq!(
        repo.events().pop().expect("an event"),
        (
            "card.reestimated".to_owned(),
            json!({"id": "C-0001", "old": "2", "new": "3", "reason": "Bigger.", "as": "agent"})
        )
    );
}

/// "Snapshot and suspect": two `edit_card` calls cannot clear a mark, a tool
/// having no clear: the target dropped and named again finds its old hash.
#[test]
fn two_edit_card_calls_cannot_clear_a_mark() {
    let repo = Repo::new("mcp-trace-retarget");
    let mut session = repo.session(Era::Stateless);
    let reply = session.call(
        "create_card",
        json!({"title": "Lead story", "discipline": "copy", "target": ["SHT-FRONT"]}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    repo.hand_edit(
        "docs/spec/sht-front.md",
        "the masthead and the lead story",
        "the masthead, the lead story and the weather",
    );
    let mark = |session: &mut Session| {
        let reply = session.call("get_trace", json!({"ref": "SHT-FRONT"}));
        result(&reply)["structuredContent"]["nodes"][0]["cards"][0]["mark"].clone()
    };
    assert_eq!(mark(&mut session), json!("suspect"));
    for target in ["SHT-BACK", "SHT-FRONT"] {
        let reply = session.call(
            "edit_card",
            json!({"card_id": "C-0001", "target": [target]}),
        );
        assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    }
    assert_eq!(mark(&mut session), json!("suspect"), "the mark stays");
    drop(session.finish());
}

/// "Accepting": a tool cannot reach an accept's verdict either: `move_card`
/// to `accept.to` stays refused and a cancelled card reads `no card`.
#[test]
fn no_tool_reaches_a_verdict() {
    let repo = Repo::new("mcp-trace-verdict");
    let id = repo.in_proof("Lead story", &["SHT-FRONT"]);
    let mut session = repo.session(Era::Stateless);
    let refused = session.call("move_card", json!({"card_id": id, "to": "filed"}));
    assert_eq!(result(&refused)["isError"], json!(true), "{refused}");
    card_cancel(
        &repo.env(),
        &Globals::default(),
        &CardCancelRequest {
            id: id.clone(),
            reason: "Pulled.".to_owned(),
            now: CLOCK.to_owned(),
            ..CardCancelRequest::default()
        },
    )
    .expect("cancelled");
    let reply = session.call("get_trace", json!({"ref": "SHT-FRONT"}));
    assert_eq!(
        result(&reply)["structuredContent"]["nodes"][0]["rollup"],
        json!("no card"),
        "{reply}"
    );
    drop(session.finish());
}
