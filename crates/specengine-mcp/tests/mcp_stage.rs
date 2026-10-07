//! docs/features/decision-staging.md over MCP, the default build (AC-15,
//! the tools' half; "Description and interactions" 2: agents read a stage,
//! no tool writes one):
//!
//! - `tools/list` is the thirteen of `TOOLS`, none of them a stage tool:
//!   no name, description or input property that stages, approves or
//!   rejects.
//! - `get_proposal` of an update staged with its span hash, a question
//!   staged as a reject, a discrepancy staged with an option and a note,
//!   and an unstaged update: `spec review --brief --json` (content and
//!   `structuredContent` byte-equal, the `outputSchema` met with its closed
//!   objects), `staged` the stage and `staged_at` its time, `staged`,
//!   `staged_at` after `task_id`; `null` both when nothing is staged.
//! - The output schema's `staged`: the two closed shapes or `null`.
//!
//! The setup's git and the library's `stage` run in the CLI tests' sandbox
//! (`common/git.rs`, included by path); every server with a cleared
//! environment and the scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::{Path, PathBuf};

use common::read::{ERAS, Session, TOOLS, checked_call, cli_env, output_schemas};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    DiscrepancyInput, DiscrepancyRequest, Evidence, GapType, GitEnv, Globals, IntakeOption,
    IntakeSeverity, ProposeRequest, ProposedText, QuestionRequest, ShowRequest, StageBody,
    StageRequest, propose, propose_discrepancy, propose_question, show,
};

const NOW: &str = "2026-10-05T21:14:03Z";
const STAGED: &str = "2026-10-05T21:30:00Z";

struct Repo {
    _scratch: Scratch,
    git: Sandbox,
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
                linked.to_str().expect("UTF-8"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            git,
            linked,
            home,
        }
    }

    fn git_env(&self, cwd: &Path) -> GitEnv {
        GitEnv::new(cwd, self.git.vars())
    }

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

    fn propose_edit(&self, reference: &str, from: &str, to: &str) -> String {
        let (base, text) = self.span(reference);
        let outcome = propose(
            &cli_env(&self.linked, Some(&self.home)),
            &Globals::default(),
            &ProposeRequest {
                target: reference.to_owned(),
                base,
                text: ProposedText::Given(text.replacen(from, to, 1).into_bytes()),
                rationale: format!("Edit {reference}."),
                author_role: Some("writer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("propose {reference}: {error}"));
        outcome.document.id.clone().expect("stored")
    }

    fn stage(&self, id: &str, body: impl FnOnce(String) -> StageBody) {
        let updated_at = specengine_cli::review(
            &cli_env(&self.linked, Some(&self.home)),
            &Globals::default(),
            &specengine_cli::ReviewRequest {
                id: id.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("review {id}: {error}"))
        .document
        .updated_at
        .expect("updated_at");
        let outcome = specengine_cli::stage(
            &cli_env(&self.linked, Some(&self.home)),
            &Globals::default(),
            &StageRequest {
                id: id.to_owned(),
                body: body(updated_at),
                now: STAGED.to_owned(),
                git: self.git_env(&self.linked),
            },
        )
        .unwrap_or_else(|error| panic!("stage {id}: {error}"));
        assert_eq!(outcome.cause, None, "stage {id}: {outcome:?}");
    }
}

fn discrepancy() -> DiscrepancyInput {
    let option = |label: &str, effect: &str| IntakeOption {
        label: label.to_owned(),
        effect: effect.to_owned(),
        price: "one item".to_owned(),
    };
    DiscrepancyInput {
        node_ids: vec!["RULE-STAM-REGEN".to_owned()],
        summary: "Regeneration starts while sprinting in the code".to_owned(),
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
        distinct_from: None,
    }
}

/// The names of a schema's properties, in order.
fn property_names(schema: &Value) -> Vec<String> {
    schema["properties"]
        .as_object()
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

/// Every object schema reachable through `anyOf`/`oneOf` of `schema`.
fn objects_in(schema: &Value, out: &mut Vec<Value>) {
    if schema.get("properties").is_some() {
        out.push(schema.clone());
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = schema.get(key).and_then(Value::as_array) {
            for option in options {
                objects_in(option, out);
            }
        }
    }
}

#[test]
fn ac15_get_proposal_shows_the_stage_and_no_tool_stages() {
    let repo = Repo::new("mcp-stage");
    let update = repo.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "the sprint stops;");
    let unstaged = repo.propose_edit("EDGE-STAM-ZERO", "immediately", "at once");
    let ask = |distinct_from: Vec<String>| {
        propose_question(
            &cli_env(&repo.linked, Some(&repo.home)),
            &Globals::default(),
            &QuestionRequest {
                node_ids: vec!["MEC-SPRINT".to_owned()],
                text: "Does a sprint end at zero?".to_owned(),
                working_answer: "yes".to_owned(),
                price_of_other: "a rule".to_owned(),
                severity: None,
                distinct_from,
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: repo.git_env(&repo.linked),
            },
        )
        .expect("asked")
    };
    let first = ask(Vec::new());
    let asked = if first.document.created {
        first
    } else {
        ask(first
            .document
            .hits
            .iter()
            .map(|hit| hit.name().to_owned())
            .collect())
    };
    let question = asked.document.id.clone().expect("stored");
    let report_with = |distinct_from: Option<Vec<String>>| {
        let mut input = discrepancy();
        input.distinct_from = distinct_from;
        propose_discrepancy(
            &cli_env(&repo.linked, Some(&repo.home)),
            &Globals::default(),
            &DiscrepancyRequest {
                input,
                author_role: Some("developer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: repo.git_env(&repo.linked),
            },
        )
        .expect("reported")
    };
    let first = report_with(None);
    let reported = if first.document.created {
        first
    } else {
        report_with(Some(
            first
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect(),
        ))
    };
    let report = reported.document.id.clone().expect("stored");
    let (span, _) = repo.span("EDGE-SPRINT-EMPTY");
    repo.stage(&update, |updated_at| StageBody::Approve {
        option: None,
        answer: None,
        canon: None,
        note: Some("ok".to_owned()),
        updated_at,
    });
    repo.stage(&question, |updated_at| StageBody::Reject {
        reason: "asked before".to_owned(),
        updated_at,
    });
    repo.stage(&report, |updated_at| StageBody::Approve {
        option: Some(1),
        answer: None,
        canon: None,
        note: Some("keep the cap".to_owned()),
        updated_at,
    });
    let expected = [
        (
            update.clone(),
            json!({"decision": "approve", "option": null, "answer": null, "canon": null,
                "note": "ok", "span_hash": span}),
            json!(STAGED),
        ),
        (
            question.clone(),
            json!({"decision": "reject", "reason": "asked before"}),
            json!(STAGED),
        ),
        (
            report.clone(),
            json!({"decision": "approve", "option": 1, "answer": null, "canon": null,
                "note": "keep the cap", "span_hash": null}),
            json!(STAGED),
        ),
        (unstaged.clone(), Value::Null, Value::Null),
    ];

    let env = cli_env(&repo.linked, Some(&repo.home));
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&repo.linked), Home::At(&repo.home));
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}: thirteen tools");
        for tool in list["tools"].as_array().expect("tools") {
            let name = tool["name"].as_str().unwrap_or_default();
            for word in ["stage", "approve", "reject", "decide"] {
                assert!(!name.contains(word), "{era:?}: a tool named {name}");
            }
            let inputs = property_names(&tool["inputSchema"]);
            for property in ["staged", "staged_at", "decision", "consent"] {
                assert!(
                    !inputs.iter().any(|input| input == property),
                    "{era:?}: {name} takes `{property}`: {inputs:?}"
                );
            }
        }
        let schemas = output_schemas(&list);
        let schema = &schemas["get_proposal"];
        let names = property_names(schema);
        for key in ["task_id", "staged", "staged_at", "notes"] {
            assert!(names.iter().any(|name| name == key), "{key}: {names:?}");
        }
        let mut shapes = Vec::new();
        objects_in(&schema["properties"]["staged"], &mut shapes);
        // The schema's maps are sorted: the shapes compared as key sets,
        // each closed and every key required.
        let mut keys: Vec<Vec<String>> = shapes
            .iter()
            .map(|shape| {
                assert_eq!(shape["additionalProperties"], json!(false), "{shape}");
                let mut names = property_names(shape);
                names.sort();
                let mut required: Vec<String> = shape["required"]
                    .as_array()
                    .expect("required")
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
                    .collect();
                required.sort();
                assert_eq!(required, names, "{shape}");
                names
            })
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                vec!["answer", "canon", "decision", "note", "option", "span_hash"],
                vec!["decision", "reason"],
            ],
            "{era:?}: the two closed shapes: {}",
            schema["properties"]["staged"]
        );
        assert!(
            schema["properties"]["staged"]
                .to_string()
                .contains("{\"type\":\"null\"}"),
            "{era:?}: `staged` may be null"
        );
        for (id, staged, staged_at) in &expected {
            let result = checked_call(
                &mut session,
                &schemas,
                "get_proposal",
                json!({"proposal_id": id}),
                &env,
                &Globals::default(),
            );
            assert_eq!(result["isError"], json!(false), "{era:?} {id}");
            let brief = &result["structuredContent"];
            assert_eq!(&brief["staged"], staged, "{era:?} {id}: {brief}");
            assert_eq!(&brief["staged_at"], staged_at, "{era:?} {id}");
            assert_eq!(brief["status"], json!("open"), "{era:?} {id}");
            let text = result["content"][0]["text"].as_str().expect("content text");
            let line = match staged_at.as_str() {
                Some(at) => format!("\nstaged_at: {at}\n"),
                None => "\nstaged_at: -\n".to_owned(),
            };
            assert!(text.contains(&line), "{era:?} {id}: {text}");
            // In the text, as in `review`: `staged`, `staged_at` right
            // after `task_id`.
            let at = |key: &str| {
                text.find(&format!("\n{key}:"))
                    .unwrap_or_else(|| panic!("{key}: {text}"))
            };
            assert!(
                at("task_id") < at("staged") && at("staged") < at("staged_at"),
                "{era:?} {id}: {text}"
            );
        }
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
    }
}

/// AC-15, the plugin's half: `ask-owner` "Whose words count" says a staged
/// choice is no answer (any local process can stage one); the plugin is
/// 0.1.6 (`PINS` in `plugin_files.rs` pins its bytes).
#[test]
fn ac15_ask_owner_names_a_staged_choice_no_answer() {
    let skill =
        fs::read_to_string(repository_root().join("plugin/specengine/skills/ask-owner/SKILL.md"))
            .expect("the skill");
    let section = skill
        .split("\n## ")
        .find(|section| section.starts_with("Whose words count"))
        .expect("its \"Whose words count\" section");
    assert!(
        section.contains("`staged`") && section.contains("is not an answer"),
        "{section}"
    );
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(repository_root().join("plugin/specengine/.claude-plugin/plugin.json"))
            .expect("plugin.json"),
    )
    .expect("JSON");
    assert_eq!(manifest["version"], json!("0.1.6"));
}
