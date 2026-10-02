//! AC-02–AC-05, AC-13 and AC-14 of docs/features/mcp-read.md: every read
//! tool answers what the CLI library answers for the same request and
//! `Env` (text = `stderr_lines` + `render_text`, exit 2 = the `CliError`
//! line(s); `structuredContent` = the parsed `render_json`, absent on exit
//! 2; `isError` on exit 1 and 2), in both protocol eras, on scratch copies
//! of `fixtures/spec-a` and `fixtures/spec-b`; schema-invalid arguments are
//! an error result with rmcp's `Parameters` message (no `structuredContent`,
//! no JSON-RPC error; AC-05 as amended) and the session goes on; input
//! schemas are flat (Data); every
//! `structuredContent` conforms to its tool's `outputSchema` (a test-local
//! walker: types, key sets, all required).
//!
//! AC-02's `lantern-keep/MEC-STAMINA` is not a valid REF (`slug/ID` names a
//! feature slug): it is checked for parity (the CLI's exit 1) and the
//! valid scoped form `stamina-tuning/AC-07` is used instead.
//!
//! M: links re-sorted, the warning dropped, a cut of MCP's own, the body
//! re-rendered, an MCP default budget, exit 1 as success, the session ended,
//! -32602 for a bad argument, a nested parameter struct, a CLI key missing
//! from a mirror type.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::blake3::blake3_hex;
use common::read::{
    ERAS, READ_TOOLS, Session, assert_bad_arguments, checked_call, cli_env, conforms, content_text,
    flat_input_problems, output_schemas,
};
use common::*;
use serde_json::{Value, json};
use specengine_cli::{Env, Globals};

/// A scratch copy of a fixture with its own `HOME`.
struct Project {
    _scratch: Scratch,
    home: PathBuf,
    root: PathBuf,
}

impl Project {
    fn new(fixture: &str) -> Self {
        let scratch = Scratch::new(&format!("read-{fixture}"));
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        Self {
            _scratch: scratch,
            home,
            root,
        }
    }

    fn env(&self) -> Env {
        cli_env(&self.root, Some(&self.home))
    }

    fn open(&self, era: common::read::Era) -> (Session, BTreeMap<String, Value>) {
        let mut session = Session::open(era, &[], Some(&self.root), Home::At(&self.home));
        let schemas = output_schemas(&session.tools());
        (session, schemas)
    }

    /// Every call of `calls` in both eras, checked against the library;
    /// the results by era, in call order.
    fn parity(&self, calls: &[(&str, Value)]) -> Vec<Vec<Value>> {
        let env = self.env();
        let globals = Globals::default();
        let mut by_era = Vec::new();
        for era in ERAS {
            let (mut session, schemas) = self.open(era);
            let results = calls
                .iter()
                .map(|(tool, args)| {
                    checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals)
                })
                .collect();
            let done = session.finish();
            assert!(done.status.success(), "{era:?}: {}", done.stderr);
            assert_eq!(done.stderr, "", "{era:?}: stderr");
            by_era.push(results);
        }
        by_era
    }
}

fn node(id: &str) -> (&'static str, Value) {
    ("get_node", json!({"id": id}))
}

fn links(id: &str, archive: Option<bool>) -> (&'static str, Value) {
    let mut args = json!({"id": id, "with": ["links"]});
    if let Some(archive) = archive {
        args["archive"] = json!(archive);
    }
    ("get_node", args)
}

// ------------------------------------------------------------------ AC-02

#[test]
fn ac02_get_node_matches_show_for_every_ref_form() {
    let project = Project::new("spec-a");
    let calls = [
        node("R-12"),
        node("MEC-STAMINA"),
        node("TERM-tired"),
        node("QST-031"),
        node("stamina-tuning/AC-07"),
        // Deviation 10: `slug/ID` is a feature slug; the CLI answers exit 1.
        node("lantern-keep/MEC-STAMINA"),
        node("MEC-STAMINA#RULE-STAM-REGEN"),
        node("MEC-STAMINA#RULE-STAM-REGEN@3"),
        node("docs/spec/movement/stamina.md"),
        node("DEC-0007"),
        links("MEC-STAMINA", None),
        links("MEC-STAMINA", Some(true)),
        links("MEC-STAMINA", Some(false)),
        links("R-12", Some(true)),
        links("DOM-MOVEMENT", None),
        ("get_node", json!({"id": "R-12", "with": []})),
    ];
    let results = project.parity(&calls);
    for era in &results {
        // The valid forms answer; the scoped form shows the section.
        for index in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
            assert_eq!(era[index]["isError"], json!(false), "{:?}", calls[index]);
        }
        assert_eq!(era[4]["structuredContent"]["nodes"][0]["line"], json!(24));
        assert_eq!(era[5]["isError"], json!(true), "lantern-keep/MEC-STAMINA");
        assert!(
            content_text(&era[10]).contains("MEC-SPRINT"),
            "links listed: {}",
            content_text(&era[10])
        );
    }
    assert_eq!(
        without_result_type(&results[0]),
        without_result_type(&results[1]),
        "both eras give the same results"
    );

    let project = Project::new("spec-b");
    project.parity(&[
        node("REQ-001"),
        node("\u{0422}\u{0420}\u{0411}-001"),
        node("\u{0412}\u{041e}\u{041f}-07"),
        node("dry-run/CRIT-01"),
        node("MOD-CLI#CMD-SYNC"),
        node("docs/records/ADR/ADR-0002.md"),
        links("REQ-001", None),
        links("REQ-001", Some(true)),
    ]);
}

#[test]
fn ac02_several_holders_keep_their_warning() {
    let project = Project::new("spec-a");
    write(
        &project.root,
        "docs/features/lantern-fuel.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Lantern fuel\n\n## Acceptance criteria\n\n\
         ### Fuel lasts the night {#AC-07}\n\nMeasured.\n",
    );
    let results = project.parity(&[node("AC-07"), links("AC-07", None)]);
    for era in &results {
        for result in era {
            let text = content_text(result);
            let first = text.lines().next().unwrap_or_default();
            assert!(
                first.starts_with("warning: ")
                    && first.contains("lantern-fuel/AC-07")
                    && first.contains("stamina-tuning/AC-07"),
                "the several-holders warning leads the text: {}",
                clip(text)
            );
            assert_eq!(
                result["structuredContent"]["nodes"]
                    .as_array()
                    .map(Vec::len),
                Some(2),
                "both holders"
            );
        }
    }
}

// ------------------------------------------------------------------ AC-03

#[test]
fn ac03_search_and_tree_match_the_cli() {
    let project = Project::new("spec-a");
    let results = project.parity(&[
        ("search", json!({"query": "stamina"})),
        (
            "search",
            json!({"query": "stamina", "kinds": ["requirement"]}),
        ),
        (
            "search",
            json!({"query": "stamina regenerates", "kinds": ["requirement", "mechanic"]}),
        ),
        ("search", json!({"query": "stamina", "limit": 1})),
        ("search", json!({"query": "stamina", "limit": 20})),
        ("search", json!({"query": "stamina", "limit": 200})),
        ("search", json!({"query": "stamina", "archive": true})),
        ("search", json!({"query": "regeneration", "archive": false})),
        ("search", json!({"query": "zzyzxquux"})),
        (
            "search",
            json!({"query": "stamina", "limit": null, "kinds": null}),
        ),
        ("get_tree", json!({})),
        ("get_tree", json!({"root": "DOM-MOVEMENT"})),
        ("get_tree", json!({"root": "MEC-STAMINA"})),
        ("get_tree", json!({"depth": 0})),
        ("get_tree", json!({"depth": 1})),
        ("get_tree", json!({"kinds": ["mechanic"]})),
        (
            "get_tree",
            json!({"kinds": ["rule", "edge-case"], "depth": 2}),
        ),
        ("get_tree", json!({"archive": true})),
        ("get_tree", json!({"root": "DOM-NOPE"})),
        ("get_tree", json!({"root": null, "depth": null})),
    ]);
    for era in &results {
        assert_eq!(
            era[3]["structuredContent"]["hits"].as_array().map(Vec::len),
            Some(1),
            "limit 1"
        );
        assert_eq!(era[18]["isError"], json!(true), "a root naming nothing");
        for (index, result) in era.iter().enumerate() {
            if index != 18 {
                assert_eq!(result["isError"], json!(false), "call {index}");
            }
        }
    }
    let project = Project::new("spec-b");
    project.parity(&[
        ("search", json!({"query": "worktree"})),
        (
            "search",
            json!({"query": "worktree", "kinds": ["term"], "limit": 2}),
        ),
        ("get_tree", json!({})),
        ("get_tree", json!({"root": "MOD-CLI", "depth": 1})),
        ("get_tree", json!({"kinds": ["command", "flag"]})),
    ]);
}

/// A corpus whose tree and search answers pass the 40 000-character cap.
fn wide_corpus(root: &Path) {
    let title_tail = "lantern wick ".repeat(20);
    for n in 0..600 {
        write(
            root,
            &format!("docs/spec/wide/W-{n:04}.md"),
            format!(
                "---\nclass: canon\nparent: DOM-GAME\n---\n\n# Quillfeather {n} {title_tail}\n\n\
                 The quillfeather paragraph {n}.\n"
            ),
        );
    }
}

#[test]
fn ac03_search_and_tree_are_cut_as_the_cli_cuts() {
    let project = Project::new("spec-a");
    wide_corpus(&project.root);
    let results = project.parity(&[
        ("get_tree", json!({})),
        ("get_tree", json!({"root": "DOM-GAME", "depth": 1})),
        ("search", json!({"query": "quillfeather", "limit": 200})),
        ("search", json!({"query": "quillfeather"})),
    ]);
    for era in &results {
        for index in [0, 1, 2] {
            let result = &era[index];
            assert_eq!(
                result["structuredContent"]["truncated"],
                json!(true),
                "call {index} is cut"
            );
            let text = content_text(result);
            assert!(
                text.lines().any(|line| line.starts_with("[truncated: ")),
                "call {index}: the CLI's tail: {}",
                clip(text)
            );
        }
        assert_eq!(era[3]["structuredContent"]["truncated"], json!(false));
    }
}

// ------------------------------------------------------------------ AC-04

#[test]
fn ac04_bundle_body_and_hash_are_the_clis() {
    let project = Project::new("spec-a");
    let results = project.parity(&[
        (
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA"], "budget": 10000}),
        ),
        ("get_context_bundle", json!({"node_ids": ["MEC-STAMINA"]})),
        (
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA", "QST-031", "stamina-tuning/AC-07"], "budget": 6000}),
        ),
    ]);
    for era in &results {
        let document = &era[0]["structuredContent"];
        let body = document["body"].as_str().expect("body");
        let hash = document["bundle_hash"].as_str().expect("bundle_hash");
        assert_eq!(hash, format!("b3:{}", blake3_hex(body.as_bytes())));
        let text = content_text(&era[0]);
        assert!(
            text.starts_with(body),
            "the text starts with the body byte for byte"
        );
        assert!(
            text.contains(&format!("\nbundle_hash {hash}\n")),
            "the text names the same hash"
        );
        assert_eq!(document["budget"], json!(10000));
        assert_eq!(
            era[1]["structuredContent"]["budget"],
            json!(2000),
            "default"
        );
    }

    // `[budgets] bundle_node` of the project's config, no MCP default.
    let config = read_text(&project.root, "specengine.toml");
    write(
        &project.root,
        "specengine.toml",
        format!("{config}\n[budgets]\nbundle_node = 4321\n"),
    );
    let results = project.parity(&[("get_context_bundle", json!({"node_ids": ["MEC-STAMINA"]}))]);
    for era in &results {
        assert_eq!(era[0]["structuredContent"]["budget"], json!(4321));
    }
}

// ------------------------------------------------------------------ AC-05

#[test]
fn ac05_errors_are_the_clis_and_the_session_goes_on() {
    let project = Project::new("spec-a");
    let env = project.env();
    let globals = Globals::default();
    let cannot_run = [
        // A Cyrillic look-alike letter: refused naming the Latin fix.
        (node("M\u{0415}C-STAMINA"), "MEC-STAMINA"),
        (node("\u{0410}-101"), "A-101"),
        (node("other:R-12"), "project:"),
        (("get_tree", json!({"depth": -1})), "depth"),
        (("search", json!({"query": "ab cd"})), "spec: "),
        (("search", json!({"query": "stamina", "limit": 0})), "limit"),
        (
            ("search", json!({"query": "stamina", "limit": 201})),
            "limit",
        ),
        (
            (
                "get_context_bundle",
                json!({"node_ids": ["MEC-STAMINA"], "budget": 0}),
            ),
            "spec: ",
        ),
        (
            (
                "get_context_bundle",
                json!({"node_ids": ["MEC-STAMINA"], "budget": 10}),
            ),
            "minimum",
        ),
        (("get_context_bundle", json!({"node_ids": []})), "spec: "),
        (
            ("get_node", json!({"id": "R-12", "archive": true})),
            "--links",
        ),
    ];
    // Schema-invalid arguments and what rmcp's message names.
    let invalid = [
        (("get_tree", json!({"depth": "x"})), "invalid type"),
        (
            ("get_node", json!({"id": "R-12", "bogus": 1})),
            "unknown field `bogus`",
        ),
        (
            ("get_node", json!({"id": "R-12", "with": ["bindings"]})),
            "unknown variant `bindings`",
        ),
        (("get_node", json!({})), "missing field `id`"),
        (("search", json!({"query": 7})), "invalid type"),
        (
            ("get_context_bundle", json!({"node_ids": "MEC-STAMINA"})),
            "invalid type",
        ),
        (("get_tree", json!({"kinds": "mechanic"})), "invalid type"),
    ];
    for era in ERAS {
        let (mut session, schemas) = project.open(era);
        // Exit 1: an error result with the reason and the exit-1 document.
        let result = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "MEC-NOPE"}),
            &env,
            &globals,
        );
        assert_eq!(result["isError"], json!(true), "{era:?} MEC-NOPE");
        assert!(
            content_text(&result).starts_with("spec: ")
                && content_text(&result).contains("MEC-NOPE"),
            "{era:?}: {result}"
        );
        assert_eq!(result["structuredContent"]["nodes"], json!([]));
        assert!(
            result["structuredContent"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("MEC-NOPE"))
        );
        checked_call(
            &mut session,
            &schemas,
            "get_context_bundle",
            json!({"node_ids": ["MEC-NOPE"]}),
            &env,
            &globals,
        );
        // Exit 2: an error result with the CLI's line, no structuredContent.
        for ((tool, args), needle) in &cannot_run {
            let result = checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals);
            assert_eq!(result["isError"], json!(true), "{era:?} {tool} {args}");
            assert!(
                result.get("structuredContent").is_none(),
                "{era:?} {tool} {args}"
            );
            let text = content_text(&result);
            assert!(
                text.contains(needle) && text.ends_with('\n'),
                "{era:?} {tool} {args}: want {needle:?} in {text:?}"
            );
            // The next call answers.
            let next = checked_call(
                &mut session,
                &schemas,
                "get_node",
                json!({"id": "R-12"}),
                &env,
                &globals,
            );
            assert_eq!(next["isError"], json!(false));
        }
        // Schema-invalid or unknown arguments (AC-05 as amended): an error
        // result with rmcp's `Parameters` message, no structuredContent, no
        // JSON-RPC error; then the next call answers.
        for ((tool, args), needle) in &invalid {
            let reply = session.call(tool, args.clone());
            assert_bad_arguments(&reply, needle, &format!("{era:?} {tool} {args}"));
            // (`ping` is no 2026-07-28 method: rmcp answers -32601.)
            let next = session.request("tools/list", json!({}));
            common::result(&next);
            let next = checked_call(
                &mut session,
                &schemas,
                "search",
                json!({"query": "stamina", "limit": 1}),
                &env,
                &globals,
            );
            assert_eq!(next["isError"], json!(false));
        }
        // An unknown tool is no read tool.
        let reply = session.call("get_nodes", json!({"id": "R-12"}));
        assert!(
            reply.get("error").is_some() || reply["result"]["isError"] == json!(true),
            "{era:?}: an unknown tool answered: {reply}"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}: stderr");
        assert!(done.messages.is_empty(), "{era:?}: {:?}", done.messages);
    }
}

/// Results without the stateless `resultType`, to compare the eras.
fn without_result_type(results: &[Value]) -> Vec<Value> {
    results
        .iter()
        .map(|result| {
            let mut result = result.clone();
            if let Some(object) = result.as_object_mut() {
                object.remove("resultType");
            }
            result
        })
        .collect()
}

// ------------------------------------------------------------------ AC-13

#[test]
fn ac13_every_input_schema_is_flat() {
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        for name in READ_TOOLS {
            let schema = &tool(&list, name)["inputSchema"];
            let problems = flat_input_problems(name, schema);
            assert!(problems.is_empty(), "{era:?}: {problems:?}\n{schema}");
        }
        let required = |name: &str| tool(&list, name)["inputSchema"]["required"].clone();
        assert_eq!(required("get_node"), json!(["id"]));
        assert_eq!(required("search"), json!(["query"]));
        assert_eq!(required("get_context_bundle"), json!(["node_ids"]));
        assert!(
            required("get_tree").is_null() || required("get_tree") == json!([]),
            "get_tree requires nothing"
        );
        let properties = |name: &str| -> Vec<String> {
            tool(&list, name)["inputSchema"]["properties"]
                .as_object()
                .expect("properties")
                .keys()
                .cloned()
                .collect()
        };
        assert_eq!(
            properties("get_tree"),
            ["archive", "depth", "kinds", "root"]
        );
        assert_eq!(properties("get_node"), ["archive", "id", "with"]);
        assert_eq!(properties("search"), ["archive", "kinds", "limit", "query"]);
        assert_eq!(properties("get_context_bundle"), ["budget", "node_ids"]);
        assert_eq!(
            tool(&list, "get_node")["inputSchema"]["properties"]["with"]["items"],
            json!({"type": "string", "enum": ["links"]})
        );
    }
}

// ------------------------------------------------------------------ AC-14

/// The walker itself discriminates: a missing key, an extra key, a wrong
/// type and a value outside an enum are refused.
#[test]
fn ac14_the_schema_walker_refuses_what_breaks_a_schema() {
    let project = Project::new("spec-a");
    let (mut session, schemas) = project.open(common::read::Era::Legacy);
    let reply = session.call("get_node", json!({"id": "MEC-STAMINA", "with": ["links"]}));
    let document = result(&reply)["structuredContent"].clone();
    let schema = &schemas["get_node"];
    conforms(schema, &document, "$").expect("the real document conforms");
    let mut missing = document.clone();
    missing.as_object_mut().unwrap().remove("notes");
    assert!(conforms(schema, &missing, "$").is_err(), "a missing key");
    let mut extra = document.clone();
    extra["extra"] = json!(1);
    assert!(conforms(schema, &extra, "$").is_err(), "an extra key");
    let mut wrong = document.clone();
    wrong["nodes"][0]["line"] = json!("one");
    assert!(conforms(schema, &wrong, "$").is_err(), "a wrong type");
    let mut nested = document.clone();
    nested["nodes"][0]
        .as_object_mut()
        .unwrap()
        .remove("tokens_est");
    assert!(
        conforms(schema, &nested, "$").is_err(),
        "a nested missing key"
    );
    for name in READ_TOOLS {
        assert!(
            !schemas[name].to_string().contains("\"$ref\""),
            "{name}: outputSchema with a $ref"
        );
    }
}

/// The key set of a JSON object.
fn key_set(value: &Value, context: &str) -> std::collections::BTreeSet<String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context}: not an object: {value}"))
        .keys()
        .cloned()
        .collect()
}

fn set_of(keys: &[&str]) -> std::collections::BTreeSet<String> {
    keys.iter().map(|key| (*key).to_owned()).collect()
}

/// AC-14: each key that is mostly `null` (`omitted`, `links`,
/// `working_answer`, `tail`) is non-null at least once here, in both eras,
/// each answer checked for parity and against its `outputSchema` (the
/// walker refuses a missing or an extra key), and its key set is exactly
/// the CLI's (named here, so a key dropped from both the mirror and the
/// CLI still fails). M: a CLI key missing from a mirror type.
#[test]
fn ac14_every_mostly_null_key_is_non_null_once_with_its_exact_keys() {
    // spec-a: a document cut at the cap (`omitted`), `links`, a bundle over
    // its budget (`tail`).
    let a = Project::new("spec-a");
    let mut huge = String::from("---\nclass: canon\n---\n\n# Huge\n\n");
    for n in 1..=200 {
        huge.push_str(&format!("## Part {n} {{#RULE-HUGE-{n}}}\n\n"));
        for k in 0..12 {
            huge.push_str(&format!(
                "Plain filler words for a long text, line {n}.{k}\n"
            ));
        }
        huge.push('\n');
    }
    write(&a.root, "docs/spec/huge.md", &huge);
    let results = a.parity(&[
        node("docs/spec/huge.md"),
        links("MEC-STAMINA", None),
        (
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA"], "budget": 300}),
        ),
    ]);
    for era in &results {
        let cut = &era[0]["structuredContent"]["nodes"][0];
        assert_eq!(cut["truncated"], json!(true), "{cut}");
        assert_eq!(
            key_set(&cut["omitted"], "omitted"),
            set_of(&[
                "lines",
                "sections",
                "sections_more",
                "holders",
                "holders_more"
            ])
        );
        assert!(cut["omitted"]["sections_more"].as_u64().unwrap() > 0);
        let linked = &era[1]["structuredContent"]["nodes"][0]["links"];
        assert_eq!(
            key_set(linked, "links"),
            set_of(&["outgoing", "incoming", "left_out", "omitted"])
        );
        assert_eq!(
            key_set(&linked["left_out"], "left_out"),
            set_of(&["generated", "tier3"])
        );
        let all: Vec<&Value> = linked["outgoing"]
            .as_array()
            .unwrap()
            .iter()
            .chain(linked["incoming"].as_array().unwrap())
            .collect();
        assert!(!all.is_empty());
        for link in all {
            assert_eq!(
                key_set(link, "link"),
                set_of(&[
                    "type", "origin", "at", "name", "written", "path", "line", "state", "reason"
                ])
            );
        }
        let tail = era[2]["structuredContent"]["tail"]
            .as_array()
            .expect("a non-null tail");
        assert!(!tail.is_empty(), "the budget leaves items out");
        for item in tail {
            assert_eq!(
                key_set(item, "tail item"),
                set_of(&["name", "title", "path", "line", "tokens_est", "layer"])
            );
        }
    }
    // spec-b: an open question with its working answer.
    let b = Project::new("spec-b");
    let results = b.parity(&[(
        "get_context_bundle",
        json!({"node_ids": ["REQ-001"], "budget": 10000}),
    )]);
    for era in &results {
        let questions = era[0]["structuredContent"]["layers"]["open_questions"]
            .as_array()
            .expect("open_questions");
        let answer = &questions
            .iter()
            .find(|item| item["name"] == "QN-07")
            .expect("QN-07 in the bundle")["working_answer"];
        assert_eq!(
            key_set(answer, "working_answer"),
            set_of(&["name", "written", "path", "line", "state"])
        );
        assert_eq!(answer["name"], json!("ASM-01"));
    }
}
