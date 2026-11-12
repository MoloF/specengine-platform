//! docs/features/collision-pairs.md (RM-0088, the pair half)
//! "Acceptance criteria", the MCP crate's half: the real `specengine-mcp`
//! binary over stdio on scratch git copies of `fixtures/spec-a` (the example
//! raised by `report_discrepancy`, its hit DEC-0023 named), and the CLI
//! library's `review_brief` in the same `HOME` as the twin.
//!
//! - PP1, the MCP part: `get_proposal`'s `structuredContent` holds the
//!   spec's `pair`, conforming to its output schema, the whole answer its
//!   library twin's (`spec review PR --brief`, the same `HOME`); its
//!   `content` the four text lines after the evidence lines; a three-target
//!   item's `pair` null and no `pair` line.
//! - PP7: `get_proposal`'s and `review_proposal`'s (when listed) output
//!   schemas hold `pair` after `evidence`: `Pair` closed (`left, right,
//!   other`), `PairSide` closed (`id, title, path, evidence`), `evidence`
//!   and `other` integer arrays, every key required; 32 tools on stdio, 18
//!   on a hub. M: `pair` an open object.
//! - Risks, "Size": the review's bound re-measured with a pair: a
//!   two-target `contradicts` discrepancy at every cap, its evidence all
//!   ESC (in `other`) and all on the two sides' files (each under its
//!   side), is a result under 300 000 characters and `MAX_RESULT_CHARS`,
//!   `content` at most 48 000; the figures are printed. The bound:
//!   `docs/canon/agent-intake-documents.md` "Escaping and size".
//!
//! The plugin's 0.45.0 pins are `plugin_files.rs`'s, `plugin_skills.rs`'s
//! and `plugin_roles.rs`'; the CLI's half `specengine-cli`'s
//! `collision_pairs.rs`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::read::{
    Era, Expected, Session, cli_env, conforms, content_text, expected, output_schemas,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{GitEnv, Globals, Outcome, ReviewRequest, review_brief};
use specengine_mcp::HUB_TOOLS;

const SPRINT: &str = "docs/spec/movement/sprint.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";

/// The four pair lines of the spec's text.
const PAIR_LINES: &str = "pair:\n  left RULE-SPRINT-COST | Cost | docs/spec/movement/sprint.md | \
                          evidence 0\n  right RULE-STAM-REGEN | Regeneration | \
                          docs/spec/movement/stamina.md | evidence 1\n  other | evidence 2\n";

/// A scratch git repository of spec-a (one commit) and a data `HOME`.
struct Repo {
    scratch: Scratch,
    _git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("spec-a", "main");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let root = fs::canonicalize(root).expect("the root");
        let home = scratch.home("h");
        Self {
            scratch,
            _git: git,
            root,
            home,
        }
    }

    fn session(&self) -> Session {
        Session::open(Era::Stateless, &[], Some(&self.root), Home::At(&self.home))
    }

    /// The library twin of `get_proposal`: `spec review ID --brief` in the
    /// root, the same `HOME` (`content` its text, `structuredContent` its
    /// `--json`).
    fn twin(&self, id: &str) -> Expected {
        expected(
            review_brief(
                &cli_env(&self.root, Some(&self.home)),
                &Globals::default(),
                &ReviewRequest {
                    id: id.to_owned(),
                    git: GitEnv::new(&self.root, [("HOME", self.home.as_os_str())]),
                },
            )
            .map(|outcome| Outcome::Proposal(Box::new(outcome))),
        )
    }
}

/// The example's evidence.
fn evidence() -> Value {
    json!([
        {"file": SPRINT, "lines": "18-20", "observed": "Sprinting pauses regeneration",
            "documented": "Cost is drained per second"},
        {"file": STAMINA, "lines": "21-23", "observed": "Regeneration never pauses",
            "documented": "Refill starts after 1.5 s"},
        {"file": "src/stamina.rs", "qpath": "stamina::regen", "lines": "40-52",
            "observed": "regen runs during sprint", "documented": "a pause is specified"}
    ])
}

/// `report_discrepancy`'s arguments: the example over `targets`, its hit
/// named.
fn example(targets: &[&str], summary: &str) -> Value {
    json!({"node_ids": targets, "gap_type": "contradicts", "severity": "normal",
        "summary": summary, "evidence": evidence(),
        "options": [{"label": "Pause while sprinting", "effect": "Sprint drains faster",
                "price": "Regeneration rewritten"},
            {"label": "Never pause", "effect": "Sprint lasts longer",
                "price": "Sprint cost rewritten"}],
        "recommendation": 0, "distinct_from": ["DEC-0023"], "author_role": "developer"})
}

fn data_pair() -> Value {
    json!({"left": {"id": "RULE-SPRINT-COST", "title": "Cost", "path": SPRINT, "evidence": [0]},
        "right": {"id": "RULE-STAM-REGEN", "title": "Regeneration", "path": STAMINA,
            "evidence": [1]},
        "other": [2]})
}

/// The stored ID of a `report_discrepancy` reply.
fn stored(reply: &Value) -> String {
    let result = result(reply);
    assert_eq!(
        result["isError"],
        json!(false),
        "{}",
        clip(&reply.to_string())
    );
    result["structuredContent"]["id"]
        .as_str()
        .unwrap_or_else(|| panic!("not stored: {}", clip(&reply.to_string())))
        .to_owned()
}

/// PP1, the MCP part: `get_proposal` serves the spec's `pair`, equal to the
/// CLI's review, conforming; its content the four lines; a three-target
/// item `null`, no line.
#[test]
fn pp1_get_proposal_serves_the_pair() {
    let repo = Repo::new("mcp-cpair-pp1");
    let mut session = repo.session();
    let list = session.tools();
    assert_eq!(tool_names(&list).len(), 32, "stdio keeps 32 tools");
    let schemas = output_schemas(&list);
    let id = stored(&session.call(
        "report_discrepancy",
        example(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN"],
            "Sprint cost and regeneration disagree on when stamina refills",
        ),
    ));
    let three = stored(&session.call(
        "report_discrepancy",
        example(
            &["RULE-SPRINT-COST", "RULE-STAM-REGEN", "MEC-SPRINT"],
            "Three documents disagree on when stamina refills",
        ),
    ));
    let read = session.call("get_proposal", json!({"proposal_id": id}));
    let document = &result(&read)["structuredContent"];
    assert_eq!(document["pair"], data_pair(), "{}", clip(&read.to_string()));
    if let Err(problem) = conforms(&schemas["get_proposal"], document, "$") {
        panic!("get_proposal breaks its outputSchema: {problem}");
    }
    let shown = content_text(result(&read));
    assert!(
        shown.contains(&format!("\n{PAIR_LINES}options: 2\n")),
        "{shown}"
    );
    let other = session.call("get_proposal", json!({"proposal_id": three}));
    let other_document = &result(&other)["structuredContent"];
    assert_eq!(other_document["pair"], Value::Null);
    if let Err(problem) = conforms(&schemas["get_proposal"], other_document, "$") {
        panic!("get_proposal breaks its outputSchema with a null pair: {problem}");
    }
    assert!(!content_text(result(&other)).contains("pair:"));
    // The twin of each read: the CLI's review over the same queue, its
    // document and text.
    for (reply, id) in [(&read, &id), (&other, &three)] {
        let want = repo.twin(id);
        assert_eq!(
            result(reply).get("structuredContent"),
            want.document.as_ref(),
            "{id}: structuredContent"
        );
        assert_eq!(content_text(result(reply)), want.text, "{id}: content");
    }
    drop(session.finish());
}

/// The object variant of a nullable schema: itself when its `type` names
/// `object`, else the `anyOf`/`oneOf` variant that does.
fn object_variant<'a>(schema: &'a Value, at: &str) -> &'a Value {
    let names_object = |schema: &Value| match &schema["type"] {
        Value::String(name) => name == "object",
        Value::Array(names) => names.contains(&json!("object")),
        _ => false,
    };
    if names_object(schema) {
        return schema;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array()
            && let Some(object) = variants.iter().find(|variant| names_object(variant))
        {
            return object;
        }
    }
    panic!("{at}: no object schema: {schema}")
}

/// Whether a nullable schema admits `null`.
fn admits_null(schema: &Value) -> bool {
    match &schema["type"] {
        Value::Array(names) if names.contains(&json!("null")) => true,
        _ => ["anyOf", "oneOf"].iter().any(|key| {
            schema[key]
                .as_array()
                .is_some_and(|variants| variants.iter().any(|v| v["type"] == json!("null")))
        }),
    }
}

/// Asserts `schema` is a closed object of exactly `keys`, each required.
fn assert_closed(schema: &Value, keys: &[&str], at: &str) {
    assert_eq!(
        schema["additionalProperties"],
        json!(false),
        "{at} is closed: {schema}"
    );
    let declared: Vec<&str> = schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{at}: no properties: {schema}"))
        .keys()
        .map(String::as_str)
        .collect();
    let mut want = keys.to_vec();
    want.sort_unstable();
    let mut got = declared.clone();
    got.sort_unstable();
    assert_eq!(got, want, "{at}: its keys");
    let mut required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{at}: no required: {schema}"))
        .iter()
        .map(|key| key.as_str().expect("a key"))
        .collect();
    required.sort_unstable();
    assert_eq!(required, want, "{at}: every key required");
}

/// Asserts `schema` is an array of integers (no negative bound).
fn assert_positions(schema: &Value, at: &str) {
    assert!(
        schema["type"] == json!("array") || schema["type"] == json!(["array"]),
        "{at}: an array: {schema}"
    );
    let items = &schema["items"];
    assert!(
        items["type"] == json!("integer") || items["type"] == json!(["integer"]),
        "{at}: integer items: {items}"
    );
    if let Some(minimum) = items.get("minimum") {
        assert_eq!(minimum.as_f64(), Some(0.0), "{at}: {items}");
    }
}

/// PP7: the review document's output schemas hold a closed `Pair` and
/// `PairSide` after `evidence`, the positions integer arrays; the tool
/// counts unchanged. M: `pair` an open object.
#[test]
fn pp7_the_schema_holds_a_closed_pair() {
    let repo = Repo::new("mcp-cpair-pp7");
    let mut session = repo.session();
    let list = session.tools();
    assert_eq!(tool_names(&list).len(), 32, "stdio keeps 32 tools");
    assert_eq!(HUB_TOOLS.len(), 18, "a hub keeps 18 tools");
    let schemas = output_schemas(&list);
    let mut seen = 0;
    for name in ["get_proposal", "review_proposal", "propose_change"] {
        let Some(schema) = schemas.get(name) else {
            continue;
        };
        let Some(pair) = schema["properties"].get("pair") else {
            assert_ne!(name, "get_proposal", "get_proposal returns pair");
            continue;
        };
        seen += 1;
        let keys: Vec<&String> = schema["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .collect();
        // The schema's property order is not kept by the tests' `serde_json`;
        // the served order is the CLI's and the daemon's to pin.
        assert!(
            keys.iter().any(|key| *key == "evidence"),
            "{name}: evidence"
        );
        assert_eq!(keys.len(), 54, "{name}: 54 keys");
        assert!(
            schema["required"]
                .as_array()
                .is_some_and(|required| required.contains(&json!("pair"))),
            "{name}: pair required"
        );
        assert!(admits_null(pair), "{name}: pair nullable: {pair}");
        let object = object_variant(pair, name);
        assert_closed(object, &["left", "right", "other"], &format!("{name}.pair"));
        assert_positions(
            &object["properties"]["other"],
            &format!("{name}.pair.other"),
        );
        for side in ["left", "right"] {
            let at = format!("{name}.pair.{side}");
            let side = object_variant(&object["properties"][side], &at);
            assert_closed(side, &["id", "title", "path", "evidence"], &at);
            for key in ["id", "title", "path"] {
                assert_eq!(
                    side["properties"][key]["type"],
                    json!(["string", "null"]),
                    "{at}.{key}"
                );
            }
            assert_positions(&side["properties"]["evidence"], &format!("{at}.evidence"));
        }
        assert!(
            !pair.to_string().contains("\"$ref\""),
            "{name}: inlined, no $ref"
        );
    }
    assert!(seen >= 1, "get_proposal's output schema holds pair");
    drop(session.finish());
}

/// Risks, "Size": a two-target `contradicts` discrepancy at every cap, its
/// eight evidence items ESC-filled (all in `other`) and then on the two
/// sides' files (each under its side), stays under 300 000 characters and
/// `MAX_RESULT_CHARS`, `content` at most 48 000.
#[test]
fn size_a_pair_at_every_cap_stays_within_the_bound() {
    let repo = Repo::new("mcp-cpair-size");
    let mut session = repo.session();
    let esc = |bytes: usize| "\u{1b}".repeat(bytes);
    let max_result = usize::try_from(specengine_mcp::MAX_RESULT_CHARS).expect("a usize");
    let option = json!({"label": esc(128), "effect": esc(512), "price": esc(512)});
    // 16 `distinct_from` entries: the hits (DEC-0023, then the first item
    // for the second), the rest 256-byte fillers.
    let distinct_after = |hits: &[&str]| -> Vec<String> {
        let mut distinct: Vec<String> = hits.iter().map(|hit| (*hit).to_owned()).collect();
        distinct.extend(
            (0..16 - hits.len()).map(|index| format!("{index:x}").repeat(256)[..256].to_owned()),
        );
        distinct
    };
    let field = "a".repeat(128);
    let mut figures = Vec::new();
    for (label, hits, files) in [
        ("other", vec!["DEC-0023"], vec![esc(512); 8]),
        (
            "sides",
            vec!["DEC-0023", "PR-0001"],
            [SPRINT, STAMINA]
                .iter()
                .cycle()
                .take(8)
                .map(|file| (*file).to_owned())
                .collect(),
        ),
    ] {
        let evidence: Vec<Value> = files
            .iter()
            .map(|file| {
                json!({"file": file, "qpath": esc(512), "lines": "999999999-999999999",
                    "observed": esc(1024), "documented": esc(1024)})
            })
            .collect();
        let args = json!({
            "node_ids": ["RULE-SPRINT-COST", "RULE-STAM-REGEN"], "summary": esc(1024),
            "gap_type": "contradicts", "severity": "low", "evidence": evidence,
            "options": vec![option.clone(); 6], "recommendation": 5,
            "working_answer": esc(2048), "distinct_from": distinct_after(&hits),
            "author_role": field, "author_model": field, "run": field
        });
        let reply = session.call("report_discrepancy", args);
        let id = stored(&reply);
        let read = session.call("get_proposal", json!({"proposal_id": id}));
        let document = &result(&read)["structuredContent"];
        assert!(document["pair"].is_object(), "{label}: a pair");
        let chars = result(&read).to_string().chars().count();
        let content = content_text(result(&read)).chars().count();
        assert!(chars <= max_result, "{label}: {chars} characters");
        assert!(chars < 300_000, "{label}: under 300 000: {chars}");
        assert!(content <= 48_000, "{label}: content {content}");
        figures.push(format!("{label}: result {chars}, content {content}"));
    }
    eprintln!("collision-pairs size: {}", figures.join("; "));
    drop(session.finish());
    drop(repo.scratch);
}
