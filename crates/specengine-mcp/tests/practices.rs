//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! the MCP half: the default build's stdio server over the scene of
//! `specengine-cli`'s `tests/practices_scene` (a scratch git copy of
//! `fixtures/studio-lamp` with the practices, its queue made through the
//! library at fixed clocks), included by path with `common/git.rs`.
//!
//! - DP13: `get_proposal` of a proposal whose rationale cites
//!   `PRA-ART-NORMALS` and `PRA-NOPE-X` serves `cited` with the first
//!   alone, equal to `spec review PR --json`'s (the library twin, not the
//!   brief) and its own twin's whole.
//! - DP14: `get_card {context: true}` (and with `budget`, and C-0003),
//!   `get_task`, `get_context_bundle {disciplines}` and `get_proposal` each
//!   equal the CLI library's text and document ("Parity") and conform to
//!   their output schemas; `get_card` without `context` serves `practices`
//!   and `cited` `null`; the schemas' key sets (`Card` 38, `CardPractice`
//!   5, `Cited` 4, the package 28, bundle `layers` 10) and the input of
//!   `get_context_bundle` (`disciplines`, optional, strings); the three
//!   descriptions within 2 048 B, naming practices; `HUB_TOOLS` 18 and
//!   unchanged. Plugin 0.48.0 is `plugin_skills.rs`' pin; the description
//!   cap's mutation is `mcp_size.rs`'.
//!
//! Every server has a cleared environment and the scene's `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

#[path = "../../specengine-cli/tests/practices_scene/mod.rs"]
mod practices_scene;

use common::read::{
    Era, Session, assert_parity, card_library, cli_env, content_text, library, output_schemas,
    server_git, task_library,
};
use common::*;
use practices_scene::{ART, CLOCK, LOD, NAMING, NORMALS, REALISM, Scene};
use serde_json::{Value, json};
use specengine_cli::{Globals, Outcome, ReviewRequest, render_json, review};
use specengine_mcp::HUB_TOOLS;

/// The stdio server over the scene, its `HOME` the scene's.
fn session(scene: &Scene) -> Session {
    Session::open(
        Era::Stateless,
        &[],
        Some(&scene.root),
        Home::At(&scene.home),
    )
}

/// The object schema's property names under `schema`, through `anyOf`.
fn properties(schema: &Value) -> Vec<String> {
    if let Some(properties) = schema["properties"].as_object() {
        let mut keys: Vec<String> = properties.keys().cloned().collect();
        keys.sort();
        return keys;
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array() {
            for variant in variants {
                let found = properties(variant);
                if !found.is_empty() {
                    return found;
                }
            }
        }
    }
    Vec::new()
}

/// The item schema of an array schema under `schema` (through `anyOf`).
fn items(schema: &Value) -> Value {
    if schema["items"].is_object() {
        return schema["items"].clone();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(variants) = schema[key].as_array() {
            for variant in variants {
                if variant["items"].is_object() {
                    return variant["items"].clone();
                }
            }
        }
    }
    panic!("no array schema: {schema}")
}

fn sorted(keys: &[&str]) -> Vec<String> {
    let mut keys: Vec<String> = keys.iter().map(|key| (*key).to_owned()).collect();
    keys.sort();
    keys
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("an array: {list}"))
        .iter()
        .map(|entry| entry["name"].as_str().expect("a name").to_owned())
        .collect()
}

// ------------------------------------------------------------- DP14

/// DP14: each call's content and `structuredContent` are the library
/// twin's, valid against the schema; what the slice adds is there.
#[test]
fn dp14_the_reads_are_the_clis() {
    let scene = Scene::new("mcp-dp14");
    let proposal = scene.propose_cited();
    let mut session = session(&scene);
    let schemas = output_schemas(&session.tools());
    let env = cli_env(&scene.root, Some(&scene.home));
    let globals = Globals::default();

    let cards = [
        json!({"card_id": "C-0001", "context": true}),
        json!({"card_id": "C-0001", "context": true, "budget": 3000}),
        json!({"card_id": "C-0001", "context": true, "history": true}),
        json!({"card_id": "C-0003", "context": true}),
        json!({"card_id": "C-0001"}),
    ];
    let mut replies = Vec::new();
    for args in &cards {
        let reply = session.call("get_card", args.clone());
        let want = card_library("get_card", args, &env, &globals, CLOCK);
        assert_parity(
            &reply,
            &want,
            &schemas["get_card"],
            &format!("get_card {args}"),
        );
        replies.push(result(&reply)["structuredContent"].clone());
    }
    assert_eq!(
        names(&replies[0]["practices"]),
        ["PRA-ART-MODELS", "PRA-SHARED"]
    );
    assert_eq!(names(&replies[0]["cited"]), [LOD]);
    assert_eq!(
        names(&replies[0]["context"]["bundle"]["layers"]["practices"]),
        [REALISM, NORMALS, LOD, NAMING],
        "the context's bundle carries the card's discipline"
    );
    assert_eq!(replies[3]["practices"], json!([]));
    assert_eq!(
        replies[4]["practices"],
        Value::Null,
        "no context, no practices"
    );
    assert_eq!(replies[4]["cited"], Value::Null);

    let task = json!({"task_id": "T-0001"});
    let reply = session.call("get_task", task.clone());
    let want = task_library("get_task", &task, &env, &globals, CLOCK, &server_git(&env));
    assert_parity(&reply, &want, &schemas["get_task"], "get_task T-0001");
    let package = &result(&reply)["structuredContent"];
    assert_eq!(
        package["practices"]
            .as_array()
            .expect("practices")
            .iter()
            .map(|entry| entry["path"].as_str().expect("a path"))
            .collect::<Vec<_>>(),
        [ART, practices_scene::DEV, practices_scene::SHARED]
    );
    assert_eq!(
        package["bundle"]["disciplines"],
        json!(["art", "development"])
    );
    let lone = json!({"task_id": "T-0002"});
    let reply = session.call("get_task", lone.clone());
    let want = task_library("get_task", &lone, &env, &globals, CLOCK, &server_git(&env));
    assert_parity(&reply, &want, &schemas["get_task"], "get_task T-0002");

    for args in [
        json!({"node_ids": ["SCN-ARRIVAL"], "budget": 10000, "disciplines": ["art"]}),
        json!({"node_ids": ["SCN-ARRIVAL"], "disciplines": ["art", "development"]}),
        json!({"node_ids": ["SCN-ARRIVAL"], "budget": 10000}),
    ] {
        let reply = session.call("get_context_bundle", args.clone());
        let want = library("get_context_bundle", &args, &env, &globals);
        assert_parity(
            &reply,
            &want,
            &schemas["get_context_bundle"],
            &format!("get_context_bundle {args}"),
        );
    }
    let reply = session.call(
        "get_context_bundle",
        json!({"node_ids": ["SCN-ARRIVAL"], "budget": 10000, "disciplines": ["art"]}),
    );
    assert_eq!(
        names(&result(&reply)["structuredContent"]["layers"]["practices"]),
        [REALISM, NORMALS, LOD, NAMING]
    );
    // The same rules as `--discipline`: a bad name is the CLI's exit 2.
    let bad = json!({"node_ids": ["SCN-ARRIVAL"], "disciplines": ["Art"]});
    let reply = session.call("get_context_bundle", bad.clone());
    let want = library("get_context_bundle", &bad, &env, &globals);
    assert!(want.is_error, "premise: the library refuses `Art`");
    assert_parity(
        &reply,
        &want,
        &schemas["get_context_bundle"],
        "a bad discipline",
    );
    assert!(
        content_text(result(&reply)).contains("`Art` is no discipline name"),
        "{reply}"
    );

    let args = json!({"proposal_id": proposal});
    let reply = session.call("get_proposal", args.clone());
    let want = library("get_proposal", &args, &env, &globals);
    assert_parity(&reply, &want, &schemas["get_proposal"], "get_proposal");
    drop(session.finish());
}

/// DP13: `get_proposal`'s `cited` is `spec review PR --json`'s, and holds
/// `PRA-ART-NORMALS` alone. M: unresolved `PRA-NOPE-X` served.
#[test]
fn dp13_get_proposal_serves_cited() {
    let scene = Scene::new("mcp-dp13");
    let proposal = scene.propose_cited();
    let mut session = session(&scene);
    let reply = session.call("get_proposal", json!({"proposal_id": proposal}));
    let served = &result(&reply)["structuredContent"];
    let env = cli_env(&scene.root, Some(&scene.home));
    let reviewed = review(
        &env,
        &Globals::default(),
        &ReviewRequest {
            id: proposal.clone(),
            git: server_git(&env),
        },
    )
    .expect("review");
    let document: Value =
        serde_json::from_str(&render_json(&Outcome::Proposal(Box::new(reviewed)))).expect("JSON");
    assert_eq!(served["cited"], document["cited"], "{served}");
    assert_eq!(
        served["cited"],
        json!([{"written": NORMALS, "name": NORMALS, "title": "Normal maps", "path": ART}]),
        "M: unresolved served"
    );
    drop(session.finish());
}

/// DP14: the output schemas mirror the new keys; `get_context_bundle`
/// takes `disciplines`; the descriptions name practices within 2 048 B;
/// `HUB_TOOLS` is the 18 it was.
#[test]
fn dp14_the_schemas_and_descriptions() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let card = &tool(&list, "get_card")["outputSchema"];
    let card_keys = properties(card);
    assert_eq!(card_keys.len(), 38, "Card 38: {card_keys:?}");
    assert!(card_keys.contains(&"practices".to_owned()) && card_keys.contains(&"cited".to_owned()));
    assert_eq!(
        properties(&items(&card["properties"]["practices"])),
        sorted(&["name", "title", "path", "disciplines", "rules"]),
        "CardPractice 5"
    );
    assert_eq!(
        properties(&items(&card["properties"]["cited"])),
        sorted(&["written", "name", "title", "path"]),
        "Cited 4"
    );
    let required: Vec<&str> = card["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        required.contains(&"practices") && required.contains(&"cited"),
        "{required:?}"
    );

    let package = &tool(&list, "get_task")["outputSchema"];
    let package_keys = properties(package);
    // 29 since docs/features/comments.md (`comments`).
    assert_eq!(package_keys.len(), 29, "{package_keys:?}");
    assert_eq!(
        properties(&items(&package["properties"]["practices"])),
        sorted(&["name", "title", "path", "disciplines", "rules"])
    );
    assert!(
        properties(&package["properties"]["bundle"]).contains(&"disciplines".to_owned()),
        "the package's bundle names its disciplines"
    );

    let bundle = &tool(&list, "get_context_bundle");
    let layers = properties(&bundle["outputSchema"]["properties"]["layers"]);
    assert_eq!(layers.len(), 10, "{layers:?}");
    assert!(layers.contains(&"practices".to_owned()));
    let input = &bundle["inputSchema"];
    assert!(
        input["properties"]["disciplines"].is_object(),
        "get_context_bundle takes disciplines: {input}"
    );
    let kind = &input["properties"]["disciplines"]["type"];
    assert!(
        kind == "array"
            || kind
                .as_array()
                .is_some_and(|kinds| kinds.contains(&json!("array"))),
        "an array: {input}"
    );
    assert_eq!(
        input["properties"]["disciplines"]["items"]["type"], "string",
        "{input}"
    );
    let required: Vec<&str> = input["required"]
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(!required.contains(&"disciplines"), "optional: {input}");

    let proposal = properties(&tool(&list, "get_proposal")["outputSchema"]);
    assert!(proposal.contains(&"cited".to_owned()), "{proposal:?}");

    for name in ["get_card", "get_task", "get_context_bundle"] {
        let description = tool(&list, name)["description"]
            .as_str()
            .expect("a description");
        assert!(
            description.len() <= 2048,
            "{name}: {} bytes",
            description.len()
        );
        assert!(
            description.contains("practice"),
            "{name} names practices: {description}"
        );
    }

    assert_eq!(
        HUB_TOOLS,
        [
            "get_tree",
            "get_node",
            "search",
            "get_context_bundle",
            "get_flow",
            "get_trace",
            "get_proposal",
            "list_cards",
            "get_card",
            // docs/features/comments.md CM15: the comments' read.
            "list_comments",
            // docs/features/specengine-guide.md GU5: the guide's tool, the
            // last shared with stdio.
            "get_guide",
            "get_roadmap",
            "propose_change",
            "ask_question",
            "report_discrepancy",
            "report_decision",
            // docs/features/comments.md CM15: the comments' write.
            "add_comment",
            "create_card",
            "split_card",
            "edit_card",
            "move_card",
        ],
        "HUB_TOOLS unchanged by department-practices (get_guide since specengine-guide, \
         list_comments and add_comment since comments)"
    );
    drop(session.finish());
}
