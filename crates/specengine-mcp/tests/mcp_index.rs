//! AC-18 of docs/features/mcp-read.md, the MCP side: after every read tool
//! and resource request, in both eras, the database's tables are the ones
//! `spec index` (the CLI library) created: no table, index or trigger is
//! added (the `CREATE` statements in the database files, read as bytes as
//! the CLI's `bundle_config.rs` does: no SQLite library in this crate's
//! tests). `INDEX_FORMAT` 9 (the code tables' format,
//! docs/features/symbol-index.md TE-3), `format_history.txt` and the CLI's
//! JSON key sets are pinned by the CLI's own tests (`bundle_config.rs`
//! `ac16_index_format_is_nine`, the key-set tests of `tree.rs`,
//! `search.rs`/`bounds.rs`, `show.rs`, `bundle.rs`, `index.rs`). The code
//! tables are created with the schema, so no MCP read adds them either.
//!
//! M: a log table.
//!
//! docs/features/code-tools.md TE-1 (`mcp_index`): the three code tools
//! refresh the code index inside the CLI read (6b-1), and over a scratch
//! copy of `fixtures/code-index` every form, exit and tool, in both eras,
//! leaves the `CREATE` statements `spec index` made. M: a table or index
//! of MCP's own (a cache of symbols between calls).
//!
//! docs/features/roadmap-surfaces.md RD-8 (M12, Q-G): `get_roadmap` joins
//! the calls (on spec-a, no `[roadmap]`: exit 2), and over a scratch git
//! repository of `fixtures/roadmap-a` whose `HOME` holds the index alone
//! (no queue table) its every request, exit 0, 1 and 2, in both eras,
//! leaves the `CREATE` statements `spec index` made: the queue is read with
//! no schema step. M: the read opening the queue with its schema steps.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

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
    assert!(
        schema
            .iter()
            .any(|statement| statement.contains("symbol_suffixes")),
        "the code tables come with the schema: {schema:?}"
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
        // docs/features/roadmap-surfaces.md: no `[roadmap]` here (exit 2).
        ("get_roadmap", json!({})),
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

    // docs/features/roadmap-surfaces.md (M12): a plan in a git
    // repository, the index alone in its `HOME`.
    let sandbox = git::Sandbox::new(scratch.path());
    let plan = scratch.copy("roadmap-a", "plan");
    sandbox.init(&plan);
    sandbox.add_all(&plan);
    sandbox.commit(&plan, "the plan");
    let plan_home = scratch.home("plan-home");
    index_with_library(&cli_env(&plan, Some(&plan_home)), &Globals::default());
    let plan_schema = schema_statements(&plan_home);
    for queue in ["tasks", "runs", "proposals", "events"] {
        assert!(
            !plan_schema
                .iter()
                .any(|statement| statement == &format!("CREATE TABLE {queue}")),
            "no queue table before the reads ({queue}): {plan_schema:?}"
        );
    }
    let plan_calls = [
        (json!({}), false),
        (json!({"release": "beta"}), false),
        (json!({"pool": true}), false),
        (json!({"release": null, "pool": false}), false),
        (json!({"release": "someday"}), true),
        (json!({"release": "beta", "pool": true}), true),
    ];
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&plan), Home::At(&plan_home));
        for (args, error) in &plan_calls {
            let reply = session.call("get_roadmap", args.clone());
            assert_eq!(
                result(&reply)["isError"],
                json!(error),
                "{era:?} get_roadmap {args}: {reply}"
            );
            assert_eq!(
                schema_statements(&plan_home),
                plan_schema,
                "{era:?}: the tables changed after get_roadmap {args}"
            );
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

/// docs/features/code-tools.md TE-1: the code tools over a code-index
/// copy add no table, index or trigger to what `spec index` created,
/// answering every form and exit (0, 1, 2) in both eras.
#[test]
fn code_tools_add_no_table() {
    let scratch = Scratch::new("code-tables");
    let home = scratch.home("h");
    let root = scratch.copy("code-index", "copy");
    index_with_library(&cli_env(&root, Some(&home)), &Globals::default());
    let schema = schema_statements(&home);
    assert!(
        schema
            .iter()
            .any(|statement| statement.contains("symbol_suffixes")),
        "the code tables come with the schema: {schema:?}"
    );
    let calls = [
        ("find_symbols", json!({"query": "separator"})),
        ("find_symbols", json!({"query": "run", "limit": 1})),
        ("find_symbols", json!({"node": "TENET-WIDTH"})),
        ("find_symbols", json!({"node": "QUILL-99"})),
        (
            "find_symbols",
            json!({"file": "crates/inkwell/src/twins.rs"}),
        ),
        (
            "find_symbols",
            json!({"at": "crates/inkwell/src/lib.rs:19"}),
        ),
        ("find_symbols", json!({"at": "crates/inkwell/src/lib.rs:0"})),
        ("find_symbols", json!({})),
        ("refs", json!({"id": "TENET-WIDTH"})),
        ("refs", json!({"id": "QUILL-99"})),
        ("unmapped", json!({})),
        ("unmapped", json!({"path": "crates/inkwell/src/twins.rs"})),
        ("unmapped", json!({"path": "docs"})),
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
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}: stderr");
    }
}
