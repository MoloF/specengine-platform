//! AC-19 of docs/features/mcp-read.md, the read path in either build: over
//! broken and non-UTF-8 files (broken front-matter, an unknown key, a
//! mixed-script ID, raw `\xff` bytes, an unclosed front-matter, a binary
//! `.md`, an empty file) every tool and resource answers, stdout carries
//! JSON-RPC only (the harness parses every line), stderr stays empty, and
//! every answer is the CLI library's. The probes-build checks (demo and
//! probe tests, `--lifecycle legacy` refusing stateless, empty stdin exit
//! 0, a bad first message exit 1 with one stderr line) are `mcp_stdio.rs`.
//!
//! M: a `println!` or the default panic hook on the read path (the hook
//! shows only with a panic: the mutation injects one into `get_tree` at
//! `depth: 99`, answered here without a parity check).

#![cfg(unix)]

mod common;

use common::read::{ERAS, Session, checked_call, cli_env, output_schemas};
use common::*;
use serde_json::json;
use specengine_cli::Globals;

#[test]
fn broken_and_non_utf8_files_leave_stdout_json_rpc_only_and_stderr_empty() {
    let scratch = Scratch::new("quiet");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    write(
        &root,
        "docs/spec/broken.md",
        "---\nid: [unclosed\nclass: canon\n---\n\n# Broken\n",
    );
    write(
        &root,
        "docs/spec/unknown-key.md",
        "---\nid: RULE-ODD\nclass: canon\nflavour: salty\n---\n\n# Odd key\n\nBody.\n",
    );
    write(
        &root,
        "docs/spec/mixed.md",
        "---\nid: M\u{0415}C-MIXED\nclass: canon\n---\n\n# Mixed script\n",
    );
    write(
        &root,
        "docs/spec/raw.md",
        b"# Raw \xff\xfe bytes\n\nstamina \xc3\x28\n".as_slice(),
    );
    write(
        &root,
        "docs/spec/unclosed.md",
        "---\nid: RULE-OPEN\nclass: canon\n\n# Never closed\n",
    );
    write(
        &root,
        "docs/spec/binary.md",
        [0u8, 159, 146, 150, 0, 1, 2, 255].as_slice(),
    );
    write(&root, "docs/spec/empty.md", "");
    let env = cli_env(&root, Some(&home));
    let globals = Globals::default();
    let calls = [
        ("get_tree", json!({})),
        ("get_tree", json!({"archive": true, "depth": 1})),
        ("get_node", json!({"id": "docs/spec/raw.md"})),
        ("get_node", json!({"id": "docs/spec/binary.md"})),
        (
            "get_node",
            json!({"id": "docs/spec/broken.md", "with": ["links"]}),
        ),
        ("get_node", json!({"id": "docs/spec/empty.md"})),
        ("get_node", json!({"id": "RULE-ODD"})),
        ("get_node", json!({"id": "M\u{0415}C-MIXED"})),
        ("search", json!({"query": "stamina", "limit": 200})),
        ("search", json!({"query": "bytes"})),
        (
            "get_context_bundle",
            json!({"node_ids": ["docs/spec/raw.md"], "budget": 10000}),
        ),
        (
            "get_context_bundle",
            json!({"node_ids": ["MEC-STAMINA", "RULE-ODD"]}),
        ),
    ];
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&root), Home::At(&home));
        let schemas = output_schemas(&session.tools());
        for (tool, args) in &calls {
            checked_call(&mut session, &schemas, tool, args.clone(), &env, &globals);
        }
        // Answered, whatever it holds: with a panic injected on the read
        // path (the mutation's trigger) the quiet hook keeps stderr empty.
        let reply = session.call("get_tree", json!({"depth": 99}));
        assert!(reply.get("result").is_some(), "{era:?}: {reply}");
        for page in session.all_resources() {
            for resource in page["resources"].as_array().unwrap() {
                let reply = session.read(resource["uri"].as_str().unwrap());
                assert!(reply.get("result").is_some(), "{era:?}: {reply}");
            }
        }
        let done = session.finish();
        assert_eq!(done.status.code(), Some(0), "{era:?}: {}", done.stderr);
        assert!(done.messages.is_empty(), "{era:?}: {:?}", done.messages);
        assert_eq!(done.stderr, "", "{era:?}: stderr");
    }
}
