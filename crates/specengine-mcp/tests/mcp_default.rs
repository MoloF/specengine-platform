#![cfg(not(feature = "probes"))]
//! The default build of `specengine-mcp` (no feature `probes`;
//! `crates/specengine-mcp/README.md`, "Feature `probes`"): the probe tools
//! are absent in both protocol eras and `instructions` do not mention them.
//!
//! Runs under `cargo nextest run --workspace` (default features); compiles to
//! nothing with `--features probes`, where `mcp_stdio.rs` runs instead.

mod common;

use common::*;
use serde_json::json;

#[test]
fn default_build_lists_only_review_proposal() {
    let (mut legacy, init) = Server::legacy(&[], form_capabilities());
    let instructions = init["instructions"].as_str().expect("instructions present");
    assert!(
        !instructions.contains("probe_"),
        "default-build instructions mention the probes: {instructions}"
    );
    let list = legacy.request(1, "tools/list", None);
    assert_eq!(tool_names(result(&list)), ["review_proposal"], "legacy");
    let call = legacy.request(
        2,
        "tools/call",
        Some(json!({"name": "probe_output", "arguments": {"tokens": 3}})),
    );
    assert!(
        call.get("error").is_some() || call["result"]["isError"] == json!(true),
        "probe_output answered in the default build: {}",
        clip(&call.to_string())
    );
    let done = legacy.finish();
    assert!(
        done.status.success(),
        "exit {:?}; stderr: {}",
        done.status,
        done.stderr
    );

    let mut stateless = Server::spawn(&[]);
    let meta = stateless_meta(form_capabilities());
    let list = stateless.request(1, "tools/list", Some(json!({"_meta": meta})));
    assert_eq!(tool_names(result(&list)), ["review_proposal"], "stateless");
    let review = tool(result(&list), "review_proposal");
    assert_eq!(
        review["_meta"],
        json!({"anthropic/requiresUserInteraction": true})
    );
}
