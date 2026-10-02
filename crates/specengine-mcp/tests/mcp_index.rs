//! AC-18 of docs/features/mcp-read.md, the MCP side: after every read tool
//! and resource request, in both eras, the database's tables are the ones
//! `spec index` (the CLI library) created: no table, index or trigger is
//! added (the `CREATE` statements in the database files, read as bytes as
//! the CLI's `bundle_config.rs` does: no SQLite library in this crate's
//! tests). `INDEX_FORMAT` 6, `format_history.txt` and the CLI's JSON key
//! sets are pinned by the CLI's own tests (`bundle_config.rs`
//! `ac16_index_format_stays_six`, the key-set tests of `tree.rs`,
//! `search.rs`/`bounds.rs`, `show.rs`, `bundle.rs`).
//!
//! M: a log table.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::read::{ERAS, Session, cli_env, index_with_library};
use common::*;
use serde_json::json;
use specengine_cli::Globals;

/// The `CREATE …` statement heads in the data directory's files.
fn schema_statements(home: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(data_dir(home)).expect("the data directory") {
        let bytes = fs::read(entry.expect("entry").path()).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        for (at, _) in text.match_indices("CREATE ") {
            let statement: String = text[at..]
                .chars()
                .take_while(|c| *c != '(' && *c != '\0' && !c.is_control())
                .take(120)
                .collect();
            found.insert(statement.trim().to_owned());
        }
    }
    found
}

#[test]
fn ac18_reads_add_no_table() {
    let scratch = Scratch::new("tables");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index_with_library(&cli_env(&root, Some(&home)), &Globals::default());
    let schema = schema_statements(&home);
    assert!(
        schema
            .iter()
            .any(|statement| statement.starts_with("CREATE TABLE")),
        "{schema:?}"
    );
    let calls = [
        ("get_tree", json!({})),
        (
            "get_tree",
            json!({"root": "DOM-MOVEMENT", "depth": 1, "archive": true}),
        ),
        (
            "get_node",
            json!({"id": "MEC-STAMINA", "with": ["links"], "archive": true}),
        ),
        ("get_node", json!({"id": "MEC-NOPE"})),
        ("search", json!({"query": "stamina", "archive": true})),
        (
            "search",
            json!({"query": "stamina", "kinds": ["requirement"], "limit": 1}),
        ),
        (
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA"], "budget": 10000}),
        ),
        (
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA"], "budget": 0}),
        ),
    ];
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        for (tool, args) in &calls {
            let reply = session.call(tool, args.clone());
            assert!(reply.get("result").is_some(), "{era:?} {tool}: {reply}");
            assert_eq!(
                schema_statements(&home),
                schema,
                "{era:?}: the tables changed after {tool} {args}"
            );
        }
        for page in session.all_resources() {
            for resource in page["resources"].as_array().unwrap() {
                result(&session.read(resource["uri"].as_str().unwrap()));
            }
        }
        result(&session.read("spec://lantern-keep/node/MEC-STAMINA%23RULE-STAM-REGEN"));
        assert_eq!(
            schema_statements(&home),
            schema,
            "{era:?}: the tables changed after the resources"
        );
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}
