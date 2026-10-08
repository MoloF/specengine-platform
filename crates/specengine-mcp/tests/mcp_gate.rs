//! docs/features/plugin-gate.md AC-01, the MCP half: a bad `[gate]` (or
//! `[zones]`) fails no read: `get_node`, `search` and `get_tree` answer
//! on a scratch git repository of `fixtures/spec-a` whose config holds
//! each AC-01 form, as with a valid gate. M: a bad `[gate]` fails the
//! reads. The default build, a cleared environment, a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;

use common::read::{Era, Session, content_text};
use common::*;
use git::Sandbox;
use serde_json::json;

#[test]
fn ac01_a_bad_gate_fails_no_mcp_read() {
    let scratch = Scratch::new("pg-mcp-ac01");
    let git = Sandbox::new(scratch.path());
    let main = scratch.copy("spec-a", "main");
    git.init(&main);
    git.add_all(&main);
    git.commit(&main, "the fixture");
    let home = scratch.home("h");
    let original = fs::read_to_string(main.join("specengine.toml")).unwrap();
    let selective = "mode = \"selective\"\nzones = [\"code\"]\n";
    for (label, tables) in [
        ("a valid gate", format!("[zones]\ncode = [\"src/**\"]\n\n[gate]\n{selective}")),
        ("an unknown key", format!("[zones]\ncode = [\"src/**\"]\n\n[gate]\n{selective}colour = \"red\"\n")),
        ("mode strict", "[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"strict\"\nzones = [\"code\"]\n".to_owned()),
        ("mode on", "[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"on\"\nzones = [\"code\"]\n".to_owned()),
        ("min_files", format!("[zones]\ncode = [\"src/**\"]\n\n[gate]\n{selective}min_files = 3\n")),
        ("an undeclared zone", "[zones]\ncode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"code\", \"docs\"]\n".to_owned()),
        ("a glob src/[", format!("[zones]\ncode = [\"src/[\"]\n\n[gate]\n{selective}")),
        ("a zone Code", "[zones]\nCode = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"Code\"]\n".to_owned()),
    ] {
        fs::write(main.join("specengine.toml"), format!("{original}\n{tables}")).unwrap();
        let mut session = Session::open(Era::Legacy, &[], Some(&main), Home::At(&home));
        let node = session.call("get_node", json!({"id": "MEC-STAMINA"}));
        let node = result(&node);
        assert_ne!(node["isError"], json!(true), "{label}: {node}");
        assert!(content_text(node).contains("Stamina"), "{label}");
        let hits = session.call("search", json!({"query": "stamina"}));
        assert_ne!(result(&hits)["isError"], json!(true), "{label}: search");
        let tree = session.call("get_tree", json!({}));
        assert_ne!(result(&tree)["isError"], json!(true), "{label}: get_tree");
        drop(session.finish());
    }
}
