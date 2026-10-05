//! AC-01 of docs/features/mcp-read.md: the default build of
//! `specengine-mcp` (no feature `probes`) lists exactly its tools in both
//! protocol eras — the four read tools and, since task spec `agent-intake`
//! (AC-02, "Tools"), `get_proposal` (read-only too) and the three queue
//! writers (`readOnlyHint`, `destructiveHint`, `idempotentHint`,
//! `openWorldHint` all `false`, no `requiresUserInteraction`) — each with
//! an `outputSchema`, `_meta["anthropic/maxResultSizeChars"]` =
//! `MAX_RESULT_CHARS` and a description of at most 2 048 characters holding
//! the determinism sentence; `instructions` are at most 2 048 bytes, name
//! the eight and mention neither `review_proposal` nor a probe. The probe
//! tools are absent (Phase 0 behaviour kept), and `probes` is no default
//! feature of the manifest.
//!
//! M: the demo in the default build (`default = ["probes"]` in the
//! manifest: `probes_is_not_a_default_feature` turns red, the list tests
//! compile out); a 2 049-character description (the build's const assert
//! fails; without it `default_build_lists_exactly_its_tools`); a queue
//! writer with `readOnlyHint: true` (agent-intake AC-12).
//!
//! The list tests compile to nothing with `--features probes`, where
//! `mcp_stdio.rs` checks the measurement build.

mod common;

use common::*;

#[cfg(not(feature = "probes"))]
mod default_build {
    use super::common::read::{ERAS, Era, READ_TOOLS, Session, TOOLS, WRITE_TOOLS};
    use super::common::*;
    use serde_json::{Value, json};

    const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

    fn assert_tool(tool: &Value, era: Era) {
        let name = tool["name"].as_str().expect("name");
        let annotations = if WRITE_TOOLS.contains(&name) {
            json!({
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false
            })
        } else {
            json!({"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false})
        };
        assert_eq!(
            tool["annotations"], annotations,
            "{era:?} {name}: annotations"
        );
        assert_eq!(
            tool["outputSchema"]["type"],
            json!("object"),
            "{era:?} {name}: outputSchema"
        );
        assert_eq!(
            tool["_meta"],
            json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS}),
            "{era:?} {name}: _meta"
        );
        assert_eq!(
            specengine_mcp::MAX_RESULT_CHARS,
            500_000,
            "the declared cap (Data)"
        );
        let description = tool["description"].as_str().expect("description");
        assert!(
            description.chars().count() <= 2048,
            "{era:?} {name}: description of {} characters",
            description.chars().count()
        );
        assert!(
            description.contains(DETERMINISM),
            "{era:?} {name}: no determinism sentence"
        );
    }

    fn assert_instructions(instructions: &str, context: &str) {
        assert!(
            instructions.len() <= 2048,
            "{context}: instructions of {} bytes",
            instructions.len()
        );
        for name in TOOLS {
            assert!(
                instructions.contains(name),
                "{context}: instructions do not name {name}"
            );
        }
        for absent in ["review_proposal", "probe_"] {
            assert!(
                !instructions.contains(absent),
                "{context}: instructions mention {absent}"
            );
        }
        assert_eq!(
            instructions,
            specengine_mcp::INSTRUCTIONS,
            "{context}: the wire text is INSTRUCTIONS"
        );
    }

    #[test]
    fn default_build_lists_exactly_its_tools() {
        for era in ERAS {
            let mut session = Session::open(era, &[], None, Home::Fresh);
            let list = session.tools();
            assert_eq!(tool_names(&list), TOOLS, "{era:?} tools/list");
            assert!(READ_TOOLS.iter().all(|name| TOOLS.contains(name)));
            for name in TOOLS {
                assert_tool(tool(&list, name), era);
            }
            match era {
                Era::Stateless => {
                    assert_eq!(list["ttlMs"], json!(0), "stateless ttlMs");
                    assert_eq!(list["cacheScope"], json!("public"), "stateless cacheScope");
                    let discover = result(&session.request("server/discover", json!({}))).clone();
                    assert_instructions(
                        discover["instructions"]
                            .as_str()
                            .expect("server/discover instructions"),
                        "server/discover",
                    );
                }
                Era::Legacy => {
                    assert!(list.get("ttlMs").is_none(), "legacy ttlMs: {list}");
                }
            }
            let call = session.call("probe_output", json!({"tokens": 3}));
            assert!(
                call.get("error").is_some() || call["result"]["isError"] == json!(true),
                "{era:?}: probe_output answered in the default build: {}",
                clip(&call.to_string())
            );
            let call = session.call("review_proposal", json!({"proposal_id": "P-0001"}));
            assert!(
                call.get("error").is_some() || call["result"]["isError"] == json!(true),
                "{era:?}: review_proposal answered in the default build: {}",
                clip(&call.to_string())
            );
            let done = session.finish();
            assert!(
                done.status.success(),
                "{era:?}: exit {:?}; stderr: {}",
                done.status,
                done.stderr
            );
            assert_eq!(done.stderr, "", "{era:?}: stderr");
        }

        let (_legacy, init) = Server::legacy(&[], json!({}));
        assert_instructions(
            init["instructions"].as_str().expect("instructions present"),
            "initialize",
        );
        assert_eq!(init["capabilities"]["resources"], json!({}), "{init}");
    }
}

/// The manifest keeps the demo out of the default build.
#[test]
fn probes_is_not_a_default_feature() {
    let manifest = read_text(&repository_root(), "crates/specengine-mcp/Cargo.toml");
    let mut in_features = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_features = line == "[features]";
            continue;
        }
        if in_features && line.starts_with("default") {
            assert!(
                !line.contains("probes"),
                "probes is a default feature: {line}"
            );
        }
    }
    assert!(
        manifest.contains("probes = [\"dep:getrandom\"]"),
        "the probes feature enables getrandom"
    );
}
