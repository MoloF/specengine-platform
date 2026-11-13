//! docs/features/usage-orphans.md (RM-0111) "Acceptance criteria", UO6, the
//! stdio half: `get_node` with `links` serves each node's `usage`
//! (`{used_by, mentioned_by}`) equal to the CLI library's document, in both
//! protocol eras and within the published `outputSchema`, on scratch copies
//! of `fixtures/spec-a` and `fixtures/spec-b` (two projects of different
//! nature, ADR-0008); `archive` widens In and never the counts; `get_node`
//! without `with` serves `links: null`, as before. M: the counts tallied over
//! admitted rows; `usage` missing from the mirror type.
//!
//! The values on spec-a are the spec's table ("Data"), worked out by hand.

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::read::{ERAS, Era, Session, checked_call, cli_env, output_schemas};
use common::{Home, Scratch};
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
        let scratch = Scratch::new(&format!("usage-{fixture}"));
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

    fn open(&self, era: Era) -> (Session, BTreeMap<String, Value>) {
        let mut session = Session::open(era, &[], Some(&self.root), Home::At(&self.home));
        let schemas = output_schemas(&session.tools());
        (session, schemas)
    }
}

/// `{used_by, mentioned_by}` as served.
fn usage(used_by: u32, mentioned_by: u32) -> Value {
    json!({"used_by": used_by, "mentioned_by": mentioned_by})
}

/// UO6: on spec-a, `get_node` with links serves the table's counts, equal to
/// the CLI library's document and within the schema, in both eras; with
/// `archive` the In list of DEC-0023 widens, its counts do not; without
/// `with`, `links` is `null`.
#[test]
fn uo6_get_node_serves_usage_equal_to_the_cli() {
    let project = Project::new("spec-a");
    let env = project.env();
    let globals = Globals::default();
    let table = [
        ("MEC-STAMINA", 2, 6),
        ("RULE-STAM-REGEN", 2, 3),
        ("A-102", 0, 0),
        ("R-12", 1, 6),
        ("DEC-0023", 1, 1),
        ("docs/features/stamina-tuning.md", 0, 1),
    ];
    for era in ERAS {
        let (mut session, schemas) = project.open(era);
        for (id, used_by, mentioned_by) in table {
            for archive in [false, true] {
                let args = json!({"id": id, "with": ["links"], "archive": archive});
                let result = checked_call(&mut session, &schemas, "get_node", args, &env, &globals);
                let links = &result["structuredContent"]["nodes"][0]["links"];
                assert_eq!(
                    links["usage"],
                    usage(used_by, mentioned_by),
                    "{era:?} {id} archive {archive}: {links}"
                );
            }
        }
        let live = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "DEC-0023", "with": ["links"]}),
            &env,
            &globals,
        );
        let archived = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "DEC-0023", "with": ["links"], "archive": true}),
            &env,
            &globals,
        );
        let incoming = |result: &Value| {
            result["structuredContent"]["nodes"][0]["links"]["incoming"]
                .as_array()
                .expect("incoming")
                .len()
        };
        assert_eq!(
            incoming(&archived),
            incoming(&live) + 1,
            "{era:?}: DEC-0007's mention joins In"
        );
        let plain = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": "MEC-STAMINA"}),
            &env,
            &globals,
        );
        assert_eq!(
            plain["structuredContent"]["nodes"][0]["links"],
            Value::Null,
            "{era:?}"
        );
        let schema = &schemas["get_node"];
        assert!(
            schema.to_string().contains("\"mentioned_by\""),
            "{era:?}: the schema describes `usage`"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

/// UO6 on spec-b: every node `get_tree` lists, through `get_node` with
/// links, equals the CLI library and carries `usage` (two projects of
/// different nature).
#[test]
fn uo6_get_node_usage_on_spec_b_equals_the_cli() {
    let project = Project::new("spec-b");
    let env = project.env();
    let globals = Globals::default();
    let (mut session, schemas) = project.open(Era::Legacy);
    let tree = checked_call(
        &mut session,
        &schemas,
        "get_tree",
        json!({}),
        &env,
        &globals,
    );
    let nodes = tree["structuredContent"]["nodes"]
        .as_array()
        .expect("nodes")
        .clone();
    assert!(nodes.len() > 3, "{tree}");
    let mut counted = 0;
    for node in &nodes {
        let reference = node["id"]
            .as_str()
            .or_else(|| node["path"].as_str())
            .expect("an ID or a path");
        let result = checked_call(
            &mut session,
            &schemas,
            "get_node",
            json!({"id": reference, "with": ["links"]}),
            &env,
            &globals,
        );
        let shown = &result["structuredContent"]["nodes"][0]["links"]["usage"];
        assert!(
            shown["used_by"].is_u64() && shown["mentioned_by"].is_u64(),
            "{reference}: {shown}"
        );
        counted +=
            shown["used_by"].as_u64().unwrap_or(0) + shown["mentioned_by"].as_u64().unwrap_or(0);
    }
    assert!(counted > 0, "premise: some node of spec-b is used");
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}
