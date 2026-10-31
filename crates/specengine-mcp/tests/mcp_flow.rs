//! docs/features/flow-read.md AC-07, the tool: `get_flow` answers what the
//! CLI library answers for the same request (text = `stderr_lines` +
//! `render_text`; exit 2 = the `CliError` line; `structuredContent` = the
//! parsed `render_json`, absent on exit 2; `isError` on exit 1 and 2), in
//! both protocol eras, over scratch copies of `fixtures/flow-gaps`,
//! `fixtures/flow-relay` and `fixtures/spec-c`, and every document meets the
//! tool's output schema under the strict walker of `common/read.rs` (exact
//! keys, every key required).
//!
//! The door (`mcp_door.rs`), the size (`mcp_size.rs`) and the genre scan
//! (`mcp_genre.rs`) hold the rest of AC-07 and AC-06.
//!
//! docs/features/flow-canvas.md AC-14: `FlowNode.gaps` reaches the agent in
//! both the text and the document, and the schema requires it. The count
//! itself is `specengine-cli`'s `flow_read.rs`.
//!
//! docs/features/trace-views.md AC-03: `get_flow`'s `trace`, over a scratch
//! copy of `fixtures/trace-almanac` with cards cut through the library: the
//! CLI's bytes within the schema either way, `null` with no flag, a node's
//! values `get_trace`'s, folded or not; a queue of another schema a note.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use common::read::{
    ERAS, Session, assert_bad_arguments, checked_call, cli_env, conforms, content_text,
    flat_input_problems, output_schemas,
};
use common::*;
use serde_json::{Value, json};
use specengine_cli::{
    CardAcceptRequest, CardAssignRequest, CardMoveRequest, CardNewRequest, Globals, card_accept,
    card_assign, card_move, card_new,
};

/// A scratch copy of a fixture with its own `HOME`.
struct Project {
    _scratch: Scratch,
    home: std::path::PathBuf,
    root: std::path::PathBuf,
}

impl Project {
    fn new(fixture: &str) -> Self {
        let scratch = Scratch::new("mcp-flow");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }

    fn session(&self, era: common::read::Era) -> Session {
        Session::open(era, &[], Some(&self.root), Home::At(&self.home))
    }
}

/// Every call of `calls` in both eras against the CLI and the schema;
/// returns the first era's results.
fn parity(project: &Project, calls: &[Value]) -> Vec<Value> {
    let env = cli_env(&project.root, Some(&project.home));
    let globals = Globals::default();
    let mut first = Vec::new();
    for (round, era) in ERAS.into_iter().enumerate() {
        let mut session = project.session(era);
        let schemas = output_schemas(&session.tools());
        for args in calls {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_flow",
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

/// Fixed clock of the cards below.
const CLOCK: &str = "2026-10-30T09:00:00Z";

/// A card of the almanac's one flow on `target`, its ID.
fn cut(project: &Project, title: &str, target: &str) -> String {
    let made = card_new(
        &cli_env(&project.root, Some(&project.home)),
        &Globals::default(),
        &CardNewRequest {
            title: title.to_owned(),
            discipline: Some("editing".to_owned()),
            targets: vec![target.to_owned()],
            now: CLOCK.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .expect("a card");
    made.card.expect("a card").id
}

/// `id` taken to `review` and accepted `--briefed` by `ben`.
fn accepted(project: &Project, id: &str) {
    let (env, globals) = (
        cli_env(&project.root, Some(&project.home)),
        Globals::default(),
    );
    card_assign(
        &env,
        &globals,
        &CardAssignRequest {
            id: id.to_owned(),
            assignee: Some("ana".to_owned()),
            reviewer: Some("ben".to_owned()),
            now: CLOCK.to_owned(),
            ..CardAssignRequest::default()
        },
    )
    .expect("assigned");
    card_move(
        &env,
        &globals,
        &CardMoveRequest {
            id: id.to_owned(),
            to: Some("review".to_owned()),
            now: CLOCK.to_owned(),
            ..CardMoveRequest::default()
        },
    )
    .expect("moved");
    let outcome = card_accept(
        &env,
        &globals,
        &CardAcceptRequest {
            id: id.to_owned(),
            actor: "ben".to_owned(),
            briefed: true,
            now: CLOCK.to_owned(),
            ..CardAcceptRequest::default()
        },
        &mut |_| true,
    )
    .expect("the accept");
    assert_eq!(outcome.refusal, None, "{outcome:?}");
}

/// The node `id` of a document.
fn node_of<'a>(document: &'a Value, id: &str) -> &'a Value {
    document["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == id)
        .unwrap_or_else(|| panic!("no node {id}: {document}"))
}

/// AC-03, the tool: `trace` absent, `false` or `null` is the plain read, each
/// node's `rollup` and `counts` `null`, its `stage` its own; `true` gives
/// `get_trace`'s values at every depth, and folding moves no count. Each call
/// the CLI's bytes within the schema, in both eras. M: `rollup` filled without
/// the flag.
#[test]
fn trace_views_get_flow_takes_trace() {
    let project = Project::new("trace-almanac");
    let call = cut(&project, "Notice posted", "STEP-CALL");
    accepted(&project, &call);
    cut(&project, "Pages set", "STEP-SET");
    let results = parity(
        &project,
        &[
            json!({}),
            json!({"trace": false}),
            json!({"trace": null}),
            json!({"root": "BOOK-ALMANAC", "depth": 1}),
            json!({"trace": true}),
            json!({"root": "BOOK-ALMANAC", "depth": 1, "trace": true}),
            json!({"root": "BOOK-ALMANAC", "depth": 0, "trace": true}),
        ],
    );
    for result in &results[..4] {
        assert_eq!(result["isError"], json!(false), "{result}");
        let document = &result["structuredContent"];
        for node in document["nodes"].as_array().expect("nodes") {
            assert_eq!(node["rollup"], Value::Null, "{node}");
            assert_eq!(node["counts"], Value::Null, "{node}");
        }
        assert!(!content_text(result).contains(" | rollup "), "{result}");
    }
    assert_eq!(
        results[0]["structuredContent"], results[1]["structuredContent"],
        "`false` is the plain read"
    );
    let plain = &results[0]["structuredContent"];
    assert_eq!(node_of(plain, "STEP-CALL")["stage"], "gather", "{plain}");
    assert_eq!(node_of(plain, "STEP-READ")["stage"], Value::Null, "{plain}");

    let flat = &results[4]["structuredContent"];
    assert_eq!(node_of(flat, "STEP-CALL")["rollup"], "confirmed", "{flat}");
    assert_eq!(node_of(flat, "STEP-SET")["rollup"], "in work", "{flat}");
    assert_eq!(node_of(flat, "STEP-BIND")["rollup"], "no card", "{flat}");
    assert!(
        content_text(&results[4]).contains(" | rollup confirmed\n"),
        "{}",
        content_text(&results[4])
    );

    let folded = &results[5]["structuredContent"];
    let gather = json!({"no_card": 1, "in_work": 1, "suspect": 0, "departed": 0, "confirmed": 1});
    assert_eq!(node_of(flat, "PART-GATHER")["counts"], gather, "{flat}");
    assert_eq!(node_of(folded, "PART-GATHER")["counts"], gather, "{folded}");
    assert_eq!(node_of(folded, "PART-GATHER")["start"], json!(true));
    assert_eq!(node_of(folded, "PART-PRINT")["end"], json!(true));
    let book = &results[6]["structuredContent"];
    assert_eq!(
        node_of(book, "BOOK-ALMANAC")["counts"],
        json!({"no_card": 6, "in_work": 1, "suspect": 0, "departed": 0, "confirmed": 1}),
        "{book}"
    );
}

/// "Rules and edge cases": `trace` with a newer queue is a note, `rollup` and
/// `counts` `null`, within the schema and never an error result; a wrong type
/// is a bad argument.
#[test]
fn trace_views_a_newer_queue_is_a_note() {
    let project = Project::new("trace-almanac");
    cut(&project, "Notice posted", "STEP-CALL");
    let db = data_dir(&project.home).join("trace-almanac.db");
    let done = std::process::Command::new("/usr/bin/sqlite3")
        .arg(&db)
        .arg("PRAGMA user_version = 9999;")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(done.status.success(), "{done:?}");
    let mut session = project.session(ERAS[0]);
    let schemas = output_schemas(&session.tools());
    let env = cli_env(&project.root, Some(&project.home));
    let result = checked_call(
        &mut session,
        &schemas,
        "get_flow",
        json!({"trace": true}),
        &env,
        &Globals::default(),
    );
    assert_eq!(result["isError"], json!(false), "{result}");
    let document = &result["structuredContent"];
    assert_eq!(node_of(document, "STEP-CALL")["rollup"], Value::Null);
    assert_eq!(node_of(document, "STEP-CALL")["counts"], Value::Null);
    assert_eq!(
        document["notes"].as_array().map(Vec::len),
        Some(1),
        "{document}"
    );
    let reply = session.call("get_flow", json!({"trace": "yes"}));
    assert_bad_arguments(&reply, "", "get_flow trace yes");
    let finished = session.finish();
    assert!(finished.status.success(), "{}", finished.stderr);
}

/// AC-07: the planted corpus: a whole read, a ROOT, a fold, each form of
/// `root` and `depth`, the CLI's bytes and a schema-true document.
#[test]
fn get_flow_equals_the_cli_within_its_schema() {
    let project = Project::new("flow-gaps");
    let results = parity(
        &project,
        &[
            json!({}),
            json!({"root": "WING-BROKEN"}),
            json!({"root": "WING-LEND"}),
            json!({"root": "WING-HEADLESS"}),
            json!({"root": "WING-LIBRARY", "depth": 0}),
            json!({"root": "WING-LIBRARY", "depth": 1}),
            json!({"root": "WING-LIBRARY", "depth": 2}),
            json!({"root": "LND-ASK#LND-ASK-LINE"}),
            json!({"root": "docs/spec/wing-lend.md"}),
            json!({"root": null, "depth": null}),
        ],
    );
    // The planted row reached the agent, in both the text and the document.
    let broken = &results[1];
    assert_eq!(broken["isError"], json!(false));
    assert!(
        content_text(broken).contains("gap two-starts | measured | 1 | LND-A + LND-B\n"),
        "{}",
        content_text(broken)
    );
    assert_eq!(
        broken["structuredContent"]["gaps"][1],
        json!({"code": "two-starts", "state": "measured", "count": 1,
            "items": [["LND-A", "LND-B"]], "cause": null})
    );
    // A lifted edge carries its count.
    let lifted = &results[5];
    assert_eq!(
        lifted["structuredContent"]["edges"],
        json!([{"from": "WING-LEND", "to": "WING-BROKEN", "type": "follows",
            "kind": "alternative", "when": null, "count": 2,
            "path": "docs/spec/lnd-a.md", "line": 10}])
    );
}

/// docs/features/flow-canvas.md AC-14: `get_flow` serves `gaps` 1, 12, 2,
/// 0 at depth 1, its sum 15 at every depth, the text line ending ` | gaps
/// <g>` above 0, each document within the tool's schema, which requires the
/// key as a count: a node without it, or holding a word, does not conform.
#[test]
fn ac14_get_flow_serves_each_nodes_gaps_within_its_schema() {
    let project = Project::new("flow-gaps");
    let results = parity(
        &project,
        &[
            json!({"root": "WING-LIBRARY", "depth": 0}),
            json!({"root": "WING-LIBRARY", "depth": 1}),
            json!({"root": "WING-LIBRARY", "depth": 2}),
            json!({"root": "WING-LIBRARY"}),
        ],
    );
    let gaps = |result: &Value, id: &str| {
        let nodes = result["structuredContent"]["nodes"]
            .as_array()
            .expect("nodes");
        let node = nodes.iter().find(|node| node["id"] == id);
        node.unwrap_or_else(|| panic!("no node {id}: {result}"))["gaps"].clone()
    };
    for (id, want) in [
        ("WING-LEND", 1),
        ("WING-BROKEN", 12),
        ("WING-HEADLESS", 2),
        ("WING-LIBRARY", 0),
    ] {
        assert_eq!(gaps(&results[1], id), json!(want), "{id}");
    }
    for result in &results {
        let nodes = result["structuredContent"]["nodes"]
            .as_array()
            .expect("nodes");
        let sum: u64 = nodes
            .iter()
            .map(|node| node["gaps"].as_u64().expect("a count"))
            .sum();
        assert_eq!(sum, 15, "{result}");
    }
    let text = content_text(&results[1]);
    assert!(
        text.contains(
            "- WING-HEADLESS | wing | The back room | docs/spec/wing-headless.md:1 | group \
             WING-LIBRARY | holds 2, inner 1 | gaps 2\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("- WING-LIBRARY | wing | The public library | docs/spec/wing-library.md:1\n"),
        "0 prints no field: {text}"
    );

    let mut session = project.session(ERAS[0]);
    let schemas = output_schemas(&session.tools());
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    let schema = &schemas["get_flow"];
    let served = &results[1]["structuredContent"];
    assert_eq!(conforms(schema, served, "get_flow"), Ok(()));
    let mut without = served.clone();
    let node = without["nodes"][0].as_object_mut().expect("a node");
    assert!(node.remove("gaps").is_some(), "gaps served");
    assert!(conforms(schema, &without, "get_flow").is_err(), "{without}");
    let mut worded = served.clone();
    worded["nodes"][0]["gaps"] = json!("twelve");
    assert!(conforms(schema, &worded, "get_flow").is_err(), "{worded}");
}

/// AC-07 and "Exits" of the spec: a gap is no error; a ROOT naming nothing
/// is an error result with the document; what cannot run has none.
#[test]
fn a_gap_is_no_error_and_a_root_naming_nothing_is_one() {
    let project = Project::new("flow-gaps");
    let results = parity(
        &project,
        &[
            json!({"root": "LND-NOPE"}),
            json!({"root": "LND-\u{0410}SK"}),
            json!({"depth": -1}),
            json!({"root": "other:LND-ASK"}),
        ],
    );
    let nothing = &results[0];
    assert_eq!(nothing["isError"], json!(true));
    assert!(
        nothing["structuredContent"]["reason"]
            .as_str()
            .expect("a reason")
            .contains("LND-NOPE"),
        "{nothing}"
    );
    for refused in &results[1..] {
        assert_eq!(refused["isError"], json!(true), "{refused}");
        assert!(refused.get("structuredContent").is_none(), "{refused}");
    }
    assert!(
        content_text(&results[1]).contains("`LND-ASK`"),
        "the Latin fix: {}",
        content_text(&results[1])
    );
}

/// AC-06, AC-07: the tool answers on a project with no `[flow]`, on a
/// bad `[flow]`, and on a project holding no link at all.
#[test]
fn get_flow_answers_on_every_project() {
    let project = Project::new("flow-relay");
    let results = parity(&project, &[json!({}), json!({"depth": 0})]);
    assert_eq!(
        results[0]["structuredContent"]["gaps"][0]["state"],
        "not_configured"
    );
    assert_eq!(
        results[0]["structuredContent"]["gaps"][0]["count"],
        Value::Null,
        "never 0"
    );

    let project = Project::new("spec-c");
    let results = parity(&project, &[json!({})]);
    assert_eq!(results[0]["isError"], json!(false));
    assert_eq!(results[0]["structuredContent"]["nodes"], json!([]));

    let project = Project::new("flow-gaps");
    let config = read_text(&project.root, "specengine.toml");
    write(
        &project.root,
        "specengine.toml",
        format!("{config}\n[[flow.definition]]\n"),
    );
    let results = parity(&project, &[json!({"root": "WING-BROKEN"})]);
    assert_eq!(results[0]["isError"], json!(false), "a bad table is exit 0");
    let row = &results[0]["structuredContent"]["gaps"][7];
    assert_eq!(row["state"], "cannot", "{row}");
    assert!(
        row["cause"]
            .as_str()
            .expect("a cause")
            .contains("specengine.toml:"),
        "{row}"
    );
}

/// The tool is a read: listed with a reader's hints, flat in `root` and
/// `depth`, both optional; a wrong type or an unknown field is the bad
/// arguments result; a call writes nothing under the project.
#[test]
fn the_tool_is_a_flat_read_that_writes_nothing() {
    let project = Project::new("flow-gaps");
    let before = snapshot(&project.root);
    for era in ERAS {
        let mut session = project.session(era);
        let list = session.tools();
        let listed = tool(&list, "get_flow");
        assert_eq!(
            listed["annotations"],
            json!({"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false}),
            "{era:?}"
        );
        let input = &listed["inputSchema"];
        let mut names: Vec<&String> = input["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        names.sort();
        assert_eq!(names, ["depth", "root", "trace"], "{era:?}: {input}");
        assert_eq!(
            input
                .get("required")
                .and_then(Value::as_array)
                .map(Vec::len),
            input.get("required").map(|_| 0),
            "neither is required: {input}"
        );
        assert_eq!(
            flat_input_problems("get_flow", input),
            Vec::<String>::new(),
            "{era:?}"
        );
        let description = listed["description"].as_str().expect("a description");
        assert!(description.len() <= 2048, "{}", description.len());
        assert!(description.is_ascii(), "{era:?}");

        for args in [
            json!({"root": 7}),
            json!({"depth": "1"}),
            json!({"type": "follows"}),
            json!({"archive": true}),
            json!({"bogus": 1}),
        ] {
            let reply = session.call("get_flow", args.clone());
            assert_bad_arguments(&reply, "", &format!("{era:?} get_flow {args}"));
        }
        assert_eq!(
            result(&session.call("get_flow", json!({"root": "WING-LEND"})))["isError"],
            json!(false),
            "{era:?}: the session goes on"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
    assert_eq!(snapshot(&project.root), before, "no tool writes under it");
    assert!(
        snapshot(&project.home).values().any(Option::is_some),
        "the read's index stayed outside the project"
    );
}

/// `INSTRUCTIONS` name the tool as the spec writes it, within the limit.
#[test]
fn the_instructions_name_get_flow_within_their_limit() {
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert!(
        instructions.contains("get_flow(root?, depth?, trace?) = flow"),
        "{instructions}"
    );
    assert!(instructions.len() <= 2048, "{}", instructions.len());
    assert!(instructions.is_ascii());
}
