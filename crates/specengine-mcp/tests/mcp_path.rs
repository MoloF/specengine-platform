//! docs/features/queue-path-targets.md over MCP, the default build: AC-09
//! and the MCP half of AC-05. The queue tools take a root-relative `.md`
//! path wherever they take a target (`propose_change.target`,
//! `ask_question`/`report_discrepancy.node_ids`, `proposed_patch.target`)
//! and answer as their CLI twins: `content` byte-equal to `spec … 2>&1`,
//! `structuredContent` its `--json`, stores and refusals alike; `get_node`'s
//! `span_hash` of a path is the base; the three writers' descriptions name
//! the path form; `INSTRUCTIONS` are byte-unchanged. M: the MCP side
//! refusing paths.
//!
//! The server runs with a cleared environment and a scratch `HOME` in the
//! linked worktree of a scratch git repository of spec-a; the twin is the
//! library with the server's clock in another `HOME` (as `mcp_intake.rs`
//! AC-13). Compiles to nothing with `--features probes`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::blake3::blake3_hex;
use common::read::{
    Era, Session, WRITE_TOOLS, assert_parity, checked_call, cli_env, output_schemas, queue_library,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{GitEnv, Globals, ShowRequest, show};

const NOW: &str = "2026-10-05T21:14:03Z";
const TUNING: &str = "docs/features/stamina-tuning.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";

/// A scratch git repository of spec-a: its main worktree (one commit) and
/// a linked worktree on `t1`, a data `HOME`.
struct Repo {
    scratch: Scratch,
    linked: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.copy("spec-a", "main");
        git.init(&main);
        git.add_all(&main);
        git.commit(&main, "the fixture");
        let linked = scratch.join("t1");
        git.git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "t1",
                linked.to_str().expect("a UTF-8 scratch path"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        Self {
            scratch,
            linked,
            home,
        }
    }

    /// `(span_hash, text)` of `reference` by the library.
    fn span(&self, reference: &str) -> (String, String) {
        let outcome = show(
            &cli_env(&self.linked, Some(&self.home)),
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
            },
        )
        .unwrap_or_else(|error| panic!("show {reference}: {error}"));
        let node = outcome.nodes.into_iter().next().expect("one node");
        (node.span_hash, node.text)
    }
}

fn question_args(node_ids: &[&str], text: &str) -> Value {
    json!({"node_ids": node_ids, "text": text, "working_answer": "yes, 1.2 s",
        "price_of_other": "R-12 rebalanced", "author_role": "writer"})
}

fn change_args(target: &str, base: &str, text: &str) -> Value {
    json!({"kind": "update", "target": target, "base": base, "text": text,
        "rationale": "Delay: 1.2 s", "author_role": "writer", "run": "r-1"})
}

/// AC-09 and AC-05's MCP half: by path, `propose_change` (an id-less
/// document with `get_node`'s `span_hash` as base, a document with an
/// `id:`), `ask_question`, `report_discrepancy` with a patch by path and
/// `get_proposal` of each answer as their twins; the refusals (not clean:
/// `isError` with the exit-2 line and no `structuredContent`; not indexed,
/// immutable, stale, a node twice) too. M: the MCP side refusing paths.
#[test]
fn ac09_the_queue_tools_take_a_path_as_their_twins_do() {
    let repo = Repo::new("qpt-ac09");
    let cwd = repo.linked.clone();
    let twin_home = repo.scratch.home("twin");
    let globals = Globals::default();
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());

    // get_node of the path: its span_hash is the base, as `spec show`'s.
    let node = checked_call(
        &mut session,
        &schemas,
        "get_node",
        json!({"id": TUNING}),
        &cli_env(&cwd, Some(&repo.home)),
        &globals,
    );
    let base = node["structuredContent"]["nodes"][0]["span_hash"]
        .as_str()
        .unwrap_or_else(|| panic!("a span_hash: {node}"))
        .to_owned();
    let (shown_base, text) = repo.span(TUNING);
    assert_eq!(base, shown_base);
    let tuned = text
        .replacen("priority: high", "priority: low", 1)
        .replacen("tick 1.5 s later.", "tick 1.2 s later.", 1);
    assert_ne!(tuned, text);
    let (stamina_base, stamina_text) = repo.span(STAMINA);
    let regen = stamina_text.replacen(
        "delay after sprinting 1.5 s",
        "delay after sprinting 1.2 s",
        1,
    );
    let (r12_base, r12_text) = repo.span("docs/records/R/R-12.md");
    let absolute = cwd.join("docs/spec/game.md");
    let absolute = absolute.to_str().expect("a UTF-8 scratch path");

    let mut patched = json!({
        "node_ids": ["MEC-STAMINA"], "summary": "The delay is 1.2 s.", "gap_type": "contradicts",
        "severity": "high",
        "evidence": [{"file": "src/stamina.rs", "qpath": "stamina::regen", "lines": "3-9",
            "observed": "1.2 s", "documented": "1.5 s"}],
        "options": [{"label": "code", "effect": "fix the code", "price": "1 item"},
            {"label": "spec", "effect": "say 1.2 s", "price": "a rebalance"}],
        "recommendation": 1, "author_role": "writer"
    });
    patched["distinct_from"] = json!(["DEC-0023"]);
    patched["proposed_patch"] = json!({"target": STAMINA, "base": stamina_base,
        "text": regen, "rationale": "Say 1.2 s."});

    let calls: Vec<(&str, Value)> = vec![
        ("propose_change", change_args(TUNING, &base, &tuned)),
        (
            "propose_change",
            change_args(STAMINA, &stamina_base, &regen),
        ),
        (
            "propose_change",
            change_args("../spec-a/docs/spec/game.md", &base, &tuned),
        ),
        ("propose_change", change_args(absolute, &base, &tuned)),
        (
            "propose_change",
            change_args("docs/./spec/game.md", &base, &tuned),
        ),
        (
            "propose_change",
            change_args("docs/spec/missing.md", &base, &tuned),
        ),
        ("propose_change", change_args("NOTES.md", &base, &tuned)),
        (
            "propose_change",
            change_args("docs/SPEC/game.md", &base, &tuned),
        ),
        (
            "propose_change",
            change_args(
                "docs/records/R/R-12.md",
                &r12_base,
                &r12_text.replacen("a short delay", "a 1.2 s delay", 1),
            ),
        ),
        (
            "propose_change",
            change_args(TUNING, &format!("b3:{}", "0".repeat(64)), &tuned),
        ),
        (
            "ask_question",
            question_args(&[TUNING], "Is 1.2 s the delay?"),
        ),
        ("ask_question", question_args(&[STAMINA], "Rest?")),
        (
            "ask_question",
            question_args(&[STAMINA, "MEC-STAMINA"], "Twice?"),
        ),
        (
            "ask_question",
            question_args(&["docs//spec/game.md"], "Clean?"),
        ),
        (
            "ask_question",
            question_args(&["docs/spec/missing.md"], "Here?"),
        ),
        (
            "ask_question",
            question_args(&["docs/records/R/R-12.md"], "Binding?"),
        ),
        ("report_discrepancy", patched),
        ("get_proposal", json!({"proposal_id": "PR-0001"})),
        ("get_proposal", json!({"proposal_id": "PR-0002"})),
        ("get_proposal", json!({"proposal_id": "PR-0003"})),
        ("get_proposal", json!({"proposal_id": "PR-0004"})),
        ("get_proposal", json!({"proposal_id": "PR-0005"})),
        ("get_proposal", json!({"proposal_id": "PR-0006"})),
    ];
    let twin_env = cli_env(&cwd, Some(&twin_home));
    let twin_git = GitEnv::new(cwd.clone(), [("HOME", twin_home.as_os_str())]);
    let mut answers = Vec::new();
    for (tool, args) in &calls {
        let reply = session.call(tool, args.clone());
        let mut stamped = NOW.to_owned();
        if let Some(id) = result(&reply)["structuredContent"]["id"].as_str()
            && WRITE_TOOLS.contains(tool)
        {
            let stored = session.call("get_proposal", json!({"proposal_id": id}));
            stamped = result(&stored)["structuredContent"]["created_at"]
                .as_str()
                .expect("created_at")
                .to_owned();
        }
        let want = queue_library(tool, args, &twin_env, &globals, &stamped, &twin_git);
        assert_parity(&reply, &want, &schemas[*tool], &format!("{tool} {args}"));
        answers.push(result(&reply).clone());
    }

    // What the answers say, beyond parity.
    let id_of = |index: usize| answers[index]["structuredContent"]["id"].clone();
    assert_eq!(id_of(0), json!("PR-0001"));
    assert_eq!(id_of(1), json!("PR-0002"));
    // Exit 2 (not clean): no structuredContent.
    for answer in &answers[2..5] {
        assert_eq!(answer["isError"], json!(true), "{answer}");
        assert!(answer.get("structuredContent").is_none(), "{answer}");
    }
    // Exit 1 (not indexed, immutable, stale): nothing stored.
    for answer in &answers[5..10] {
        assert_eq!(answer["isError"], json!(true), "{answer}");
        assert_eq!(answer["structuredContent"]["id"], Value::Null, "{answer}");
    }
    assert_eq!(id_of(10), json!("PR-0003"));
    assert_eq!(
        answers[11]["structuredContent"]["hits"][0]["id"],
        json!("DEC-0023")
    );
    assert_eq!(answers[11]["structuredContent"]["created"], json!(false));
    assert_eq!(answers[12]["isError"], json!(true));
    assert_eq!(answers[13]["isError"], json!(true));
    assert_eq!(answers[14]["isError"], json!(true));
    assert_eq!(id_of(15), json!("PR-0004"));
    assert_eq!(id_of(16), json!("PR-0005"));
    assert_eq!(answers[16]["structuredContent"]["linked"], json!("PR-0006"));
    let target_of = |index: usize| {
        let document = &answers[index]["structuredContent"];
        (
            document["target_id"].clone(),
            document["target_path"].clone(),
            document["target_ids"].clone(),
        )
    };
    assert_eq!(
        target_of(17),
        (json!(TUNING), json!(TUNING), json!([TUNING]))
    );
    assert_eq!(
        target_of(18),
        (json!("MEC-STAMINA"), json!(STAMINA), json!(["MEC-STAMINA"]))
    );
    assert_eq!(target_of(19).2, json!([TUNING]));
    assert_eq!(target_of(20).2, json!(["R-12"]));
    assert_eq!(target_of(21).2, json!(["MEC-STAMINA"]));
    assert_eq!(
        (target_of(22).0, target_of(22).1),
        (json!("MEC-STAMINA"), json!(STAMINA))
    );
    let done = session.finish();
    assert!(done.status.success(), "{:?}: {}", done.status, done.stderr);
}

/// AC-09's descriptions: `propose_change`, `ask_question` and
/// `report_discrepancy` name the path form (description and input
/// schema), within 2 048 characters; `INSTRUCTIONS` byte-unchanged by this
/// slice and on the wire as is: 1 740 bytes, its BLAKE3 pinned, as
/// docs/features/proposal-kinds.md AC-17 re-pins them (1 878 since
/// docs/features/task-package.md's tasks line, re-pinned below).
#[test]
fn ac09_the_writers_name_the_path_form_and_instructions_stay() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let tool = |name: &str| {
        list["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("{name} listed"))
            .clone()
    };
    for (name, field) in [
        ("propose_change", "target"),
        ("ask_question", "node_ids"),
        ("report_discrepancy", "node_ids"),
    ] {
        let tool = tool(name);
        let description = tool["description"].as_str().expect("description");
        assert!(description.chars().count() <= 2048, "{name}");
        assert!(
            description.contains(".md path"),
            "{name} names the path form: {description}"
        );
        let schema = tool["inputSchema"]["properties"][field]["description"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}.{field} described"));
        assert!(schema.contains(".md"), "{name}.{field}: {schema}");
    }
    let discover = result(&session.request("server/discover", json!({}))).clone();
    assert_eq!(
        discover["instructions"].as_str(),
        Some(specengine_mcp::INSTRUCTIONS)
    );
    drop(session.finish());
    // docs/features/task-package.md re-pins them: its tasks line (138
    // bytes with its LF) under "Queue"; without it, the text this slice
    // pinned (1 740 bytes, `4b08078c…`).
    let tasks_line = "- get_task, claim_task, submit_plan, report_run, complete_task = spec task \
                      show|claim|plan|report|complete; the three above take task_id.\n";
    assert_eq!(tasks_line.len(), 138);
    assert_eq!(specengine_mcp::INSTRUCTIONS.matches(tasks_line).count(), 1);
    let before = specengine_mcp::INSTRUCTIONS.replacen(tasks_line, "", 1);
    assert_eq!(before.len(), 1740);
    assert_eq!(
        blake3_hex(before.as_bytes()),
        "4b08078cc4c40004f3dab85d8f1e2bc856c175424177514088a91a69ba7d1c0e"
    );
    assert_eq!(specengine_mcp::INSTRUCTIONS.len(), 1878);
    assert_eq!(
        blake3_hex(specengine_mcp::INSTRUCTIONS.as_bytes()),
        "ddfb9e41d87cdf36799384c35c4da847913ca4f28746b14babc1f76d283bbc7d"
    );
}
