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
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use common::read::{
    ERAS, Session, assert_bad_arguments, checked_call, cli_env, conforms, content_text,
    flat_input_problems, output_schemas,
};
use common::*;
use serde_json::{Value, json};
use specengine_cli::Globals;

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
        assert_eq!(names, ["depth", "root"], "{era:?}: {input}");
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
        instructions.contains("get_flow(root?, depth?) = flow"),
        "{instructions}"
    );
    assert!(instructions.len() <= 2048, "{}", instructions.len());
    assert!(instructions.is_ascii());
}
