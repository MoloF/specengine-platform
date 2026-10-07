//! docs/features/decision-apply.md over MCP, the default build, as the real
//! binary answers over stdio:
//!
//! - AC-12, the tools' half: a decided item (a spec-a discrepancy applied
//!   as `DEC-0024`, its record committed on `t1`); every one of the eight
//!   tools called on it in both eras answers; afterwards `git status
//!   --porcelain` of both worktrees is empty, every ref and file as it was;
//!   `tools/list` is the same eight. M: a tool added.
//! - AC-11, the MCP half: `get_proposal` of the decided item is `spec
//!   review --brief --json` (content and `structuredContent`, its
//!   `outputSchema`), the record's keys set, `record_text` null; the same
//!   summary reported again answers with the record (`created: false`).
//! - AC-16, the descriptions: `ask_question`, `report_discrepancy` and
//!   `get_proposal` name `spec approve`; `INSTRUCTIONS` byte-unchanged
//!   (its length and BLAKE3 as `mcp_path.rs` pins them, re-pinned by
//!   docs/features/proposal-kinds.md AC-17). M: no description edited.
//!
//! The setup's git and the library's approve run in the CLI tests' sandbox
//! (`common/git.rs`, included by path); every server with a cleared
//! environment and the scratch `HOME`. Compiles to nothing with
//! `--features probes` (the ACs name the default build).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::PathBuf;

use common::blake3::blake3_hex;
use common::read::{ERAS, Era, Session, TOOLS, checked_call, cli_env, output_schemas};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, DiscrepancyInput, DiscrepancyRequest, Evidence, Exit, GapType,
    GitEnv, Globals, IntakeOption, IntakeSeverity, approve_with, propose_discrepancy,
};

const NOW: &str = "2026-10-05T09:00:00Z";
const CLOCK: &str = "2026-10-05T12:00:00Z";
const SUMMARY: &str = "Regeneration starts while sprinting in the code";

/// A scratch git repository of spec-a (its main worktree, one commit on
/// `main`, a linked worktree on `t1`), a data `HOME`, and `PR-0001`: a
/// discrepancy on `RULE-STAM-REGEN` raised in `t1`, decided by the
/// library's approve (`--option 1`) into `DEC-0024`.
struct Decided {
    _scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    linked: PathBuf,
    home: PathBuf,
}

fn input(distinct_from: Option<Vec<String>>) -> DiscrepancyInput {
    let option = |label: &str, effect: &str| IntakeOption {
        label: label.to_owned(),
        effect: effect.to_owned(),
        price: "one item".to_owned(),
    };
    DiscrepancyInput {
        node_ids: vec!["RULE-STAM-REGEN".to_owned()],
        summary: SUMMARY.to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::Normal,
        evidence: vec![Evidence {
            file: "src/stamina.rs".to_owned(),
            qpath: None,
            lines: Some("10-20".to_owned()),
            observed: "regenerates while sprinting".to_owned(),
            documented: "regenerates only at rest".to_owned(),
        }],
        options: vec![
            option("Keep the spec", "Fix the code"),
            option("Change the spec", "Regeneration also while sprinting"),
        ],
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from,
    }
}

impl Decided {
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
                linked.to_str().expect("UTF-8"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        let decided = Self {
            _scratch: scratch,
            git,
            main,
            linked,
            home,
        };
        let outcome = propose_discrepancy(
            &cli_env(&decided.linked, Some(&decided.home)),
            &Globals::default(),
            &DiscrepancyRequest {
                input: input(Some(vec!["DEC-0023".to_owned()])),
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: decided.git_env(&decided.linked),
            },
        )
        .expect("reported");
        assert_eq!(
            outcome.document.id.as_deref(),
            Some("PR-0001"),
            "{outcome:?}"
        );
        let mut consent = |_: &str| true;
        let outcome = approve_with(
            &cli_env(&decided.main, Some(&decided.home)),
            &Globals::default(),
            &ApproveRequest {
                id: "PR-0001".to_owned(),
                note: None,
                now: CLOCK.to_owned(),
                git: decided.git_env(&decided.main),
            },
            &ApproveFlags {
                option: Some(1),
                answer: None,
                canon: None,
            },
            &mut consent,
        )
        .expect("approved");
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        assert_eq!(outcome.document.record_id.as_deref(), Some("DEC-0024"));
        decided
    }

    fn git_env(&self, cwd: &std::path::Path) -> GitEnv {
        GitEnv::new(cwd, self.git.vars())
    }

    /// Everything the tools must leave: both worktrees' files, every ref,
    /// both `git status --porcelain`.
    fn state(&self) -> (String, String, String, impl PartialEq + std::fmt::Debug) {
        let status = |dir: &std::path::Path| {
            String::from_utf8(
                self.git
                    .git(dir, &["status", "--porcelain", "--untracked-files=all"]),
            )
            .expect("UTF-8")
        };
        (
            status(&self.main),
            status(&self.linked),
            self.git.git_text(
                &self.main,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            ),
            (snapshot(&self.main), snapshot(&self.linked)),
        )
    }
}

/// The structured document of a tool result that answered.
fn answered(reply: &Value, context: &str) -> Value {
    let result = result(reply);
    assert_eq!(
        result["isError"],
        json!(false),
        "{context}: {}",
        clip(&result.to_string())
    );
    result["structuredContent"].clone()
}

/// AC-12 (the tools' half) and AC-11 (the MCP half): see the module
/// documentation.
#[test]
fn ac12_every_tool_on_a_decided_item_leaves_git_and_the_tool_list_as_they_were() {
    let decided = Decided::new("da-mcp");
    let before = decided.state();
    assert_eq!(before.0, "", "main clean");
    assert_eq!(before.1, "", "t1 clean: the record committed");
    let env = cli_env(&decided.linked, Some(&decided.home));
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&decided.linked), Home::At(&decided.home));
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}: no tool added");
        let schemas = output_schemas(&list);
        for (tool, args) in [
            ("get_tree", json!({})),
            ("get_node", json!({"id": "DEC-0024", "with": ["links"]})),
            ("search", json!({"query": "sprinting"})),
            ("get_context_bundle", json!({"node_ids": ["DEC-0024"]})),
            ("get_proposal", json!({"proposal_id": "PR-0001"})),
        ] {
            let result = checked_call(
                &mut session,
                &schemas,
                tool,
                args,
                &env,
                &Globals::default(),
            );
            assert_eq!(result["isError"], json!(false), "{era:?} {tool}");
        }
        let node = answered(
            &session.call("get_node", json!({"id": "DEC-0024"})),
            "get_node",
        );
        assert_eq!(
            node["nodes"][0]["path"],
            json!("docs/records/DEC/DEC-0024.md"),
            "{node}"
        );
        let brief = answered(
            &session.call("get_proposal", json!({"proposal_id": "PR-0001"})),
            "get_proposal",
        );
        assert_eq!(brief["status"], json!("applied"));
        assert_eq!(brief["record_id"], json!("DEC-0024"));
        assert_eq!(brief["record_path"], json!("docs/records/DEC/DEC-0024.md"));
        assert_eq!(brief["record_title"], json!("Change the spec"));
        assert!(brief["record_text"].is_null(), "{brief}");
        assert_eq!(brief["choice"], json!({"option": 1}));

        let again = json!({
            "node_ids": ["RULE-STAM-REGEN"], "summary": SUMMARY, "gap_type": "contradicts",
            "severity": "normal",
            "evidence": [{"file": "src/stamina.rs", "lines": "10-20",
                "observed": "regenerates while sprinting",
                "documented": "regenerates only at rest"}],
            "options": [
                {"label": "Keep the spec", "effect": "Fix the code", "price": "one item"},
                {"label": "Change the spec", "effect": "Regeneration also while sprinting",
                    "price": "one item"}],
            "recommendation": 0, "author_role": "developer"
        });
        let document = answered(&session.call("report_discrepancy", again), "again");
        assert_eq!(document["created"], json!(false), "{document}");
        let hits = document["hits"].as_array().expect("hits");
        assert!(
            hits.contains(
                &json!({"id": "PR-0001", "source": "queue", "status": "applied",
                "path": "docs/records/DEC/DEC-0024.md", "answer": "Change the spec",
                "record": "DEC-0024"})
            ),
            "{document}"
        );
        assert!(
            hits.iter()
                .any(|hit| hit["id"] == json!("DEC-0024") && hit["record"] == json!("DEC-0024")),
            "{document}"
        );
        let document = answered(
            &session.call(
                "ask_question",
                json!({"node_ids": ["RULE-STAM-REGEN"], "text": format!("Asked in {era:?}?"),
                    "working_answer": "at rest", "price_of_other": "a rule",
                    "author_role": "developer", "distinct_from": ["DEC-0023", "DEC-0024"]}),
            ),
            "ask",
        );
        assert_eq!(document["created"], json!(true), "{document}");
        let base = answered(
            &session.call("get_node", json!({"id": "EDGE-STAM-ZERO"})),
            "base",
        )["nodes"][0]["span_hash"]
            .as_str()
            .expect("span_hash")
            .to_owned();
        let document = answered(
            &session.call(
                "propose_change",
                json!({"kind": "update", "target": "EDGE-STAM-ZERO", "base": base,
                    "text": "## Depletion {#EDGE-STAM-ZERO}\n- At zero: `Exhausted`.\n",
                    "rationale": "Shorter.", "author_role": "developer"}),
            ),
            "change",
        );
        assert!(document["id"].is_string(), "{document}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert!(
            decided.state() == before,
            "{era:?}: git and the files as they were"
        );
    }
}

/// AC-16, the descriptions: the three queue tools that meet a decided
/// item name `spec approve`; `INSTRUCTIONS` byte-unchanged.
#[test]
fn ac16_the_descriptions_name_spec_approve_and_instructions_stay() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    for name in ["ask_question", "report_discrepancy", "get_proposal"] {
        let description = tool(&list, name)["description"]
            .as_str()
            .expect("a description");
        assert!(
            description.contains("`spec approve"),
            "{name}: {description}"
        );
        assert!(description.chars().count() <= 2048, "{name}");
    }
    assert!(
        tool(&list, "get_proposal")["description"]
            .as_str()
            .unwrap()
            .contains("record_id, record_path, record_title, choice"),
        "get_proposal names the record's keys"
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
