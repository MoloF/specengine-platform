//! docs/features/kind-decision.md over MCP, the real binary over stdio
//! (AC-R10, AC-T3, "Data" MCP and `INSTRUCTIONS`):
//!
//! - `tools/list` (both eras): the fourteen of `TOOLS`; `report_decision`
//!   takes exactly "Data"'s arguments (`node_ids`, `text`,
//!   `working_answer`, `price_of_other`, `author_role` required; no field
//!   named `decision`, no `options`), its annotations, `_meta` and output
//!   schema `report_discrepancy`'s; its description ASCII, within 2 048
//!   bytes, holding the determinism line and naming the decision's
//!   fields; `INSTRUCTIONS` hold the queue's new line exactly (202 bytes)
//!   and stay within 2 048 bytes. M: `readOnlyHint` true; a `decision`
//!   field.
//! - A call with a patch answers as its twin `spec propose decision
//!   --input -` (content and `structuredContent` equal, the schema kept):
//!   `PR-0001` a decision, `linked` `PR-0002`; with `task_id` both rows
//!   bound (the twin's too); the same call again is a hit, not stored; a
//!   question and a decision hit each other over MCP. A `decision`, an
//!   `options` or a look-alike key, or no `working_answer`: a bad argument
//!   naming it, nothing stored. AC-T3: after every call both worktrees'
//!   files are as committed and `git status` empty. M: a file write in
//!   propose.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::read::{
    ERAS, Era, Session, TOOLS, assert_bad_arguments, assert_parity, cli_env, output_schemas,
    queue_library,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    GitEnv, Globals, InboxRequest, ShowRequest, TaskNewRequest, inbox, show, task_new,
};

const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";
/// "Data"'s `INSTRUCTIONS` queue line, its LF aside.
const QUEUE_LINE: &str = "- ask_question, report_discrepancy, report_decision = spec propose \
                          question|discrepancy|decision: hits are what is decided or asked; \
                          stored only if distinct_from names each; work on your working answer.";
const TEXT: &str = "Does regeneration wait for rest?";
const WORKING: &str = "Yes: it starts 1.5 s after the last stamina use.";
const PRICE: &str = "Without the wait R-28 is rebalanced.";

/// A scratch git repository of spec-a: its main worktree (one commit on
/// `main`) and a linked worktree on `t1`, a data `HOME`.
struct Repo {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
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
            git,
            main,
            linked,
            home,
        }
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

    /// Both worktrees' `git status --porcelain`.
    fn statuses(&self) -> (String, String) {
        let status = |dir: &Path| {
            self.git
                .git_text(dir, &["status", "--porcelain=v1", "--untracked-files=all"])
        };
        (status(&self.main), status(&self.linked))
    }

    /// Both worktrees' files (`.git` aside) and every ref.
    fn files(&self) -> (BTreeMap<String, Option<Vec<u8>>>, String) {
        let mut files = BTreeMap::new();
        for (name, dir) in [("main", &self.main), ("t1", &self.linked)] {
            for (path, bytes) in snapshot(dir) {
                if path != ".git" && !path.starts_with(".git/") {
                    files.insert(format!("{name}/{path}"), bytes);
                }
            }
        }
        let refs = self.git.git_text(
            &self.main,
            &["for-each-ref", "--format=%(refname) %(objectname)"],
        );
        (files, refs)
    }

    /// The IDs `home`'s queue holds for this project.
    fn stored(&self, home: &Path) -> Vec<String> {
        if !data_dir(home).join("lantern-keep.db").exists() {
            return Vec::new();
        }
        inbox(
            &cli_env(&self.main, Some(home)),
            &Globals::default(),
            &InboxRequest {
                all: true,
                git: GitEnv::new(&self.main, self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("inbox: {error}"))
        .proposals
        .into_iter()
        .map(|entry| entry.id)
        .collect()
    }

    /// A draft task on `MEC-STAMINA` in `home`'s queue: its ID.
    fn task(&self, home: &Path) -> String {
        let outcome = task_new(
            &cli_env(&self.main, Some(home)),
            &Globals::default(),
            &TaskNewRequest {
                nodes: vec!["MEC-STAMINA".to_owned()],
                title: Some("Tune the regeneration".to_owned()),
                goal: Some("Make it so.".to_owned()),
                author_role: None,
                author_model: None,
                run: None,
                now: "2026-10-07T09:00:00Z".to_owned(),
                git: GitEnv::new(&self.main, self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("task new: {error}"));
        outcome.id.expect("a task ID")
    }
}

/// The sorted names of a JSON array of strings.
fn sorted(value: &Value) -> Vec<String> {
    let mut names: Vec<String> = value
        .as_array()
        .unwrap_or_else(|| panic!("an array: {value}"))
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect();
    names.sort();
    names
}

fn keys(value: &Value) -> Vec<String> {
    let mut names: Vec<String> = value
        .as_object()
        .unwrap_or_else(|| panic!("an object: {value}"))
        .keys()
        .cloned()
        .collect();
    names.sort();
    names
}

/// AC-R10: fourteen tools in both eras; `report_decision`'s input schema
/// is "Data"'s, no field named `decision`; annotations, `_meta` and the
/// output schema `report_discrepancy`'s; the description and
/// `INSTRUCTIONS` within their caps, holding "Data"'s lines. M:
/// `readOnlyHint` true; a `decision` field.
#[test]
fn ac_r10_report_decision_is_listed_as_data_says() {
    assert_eq!(QUEUE_LINE.len(), 202);
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert_eq!(
        instructions.matches(&format!("{QUEUE_LINE}\n")).count(),
        1,
        "{instructions}"
    );
    assert!(instructions.len() <= 2048, "{}", instructions.len());
    assert!(instructions.is_ascii());
    assert!(!instructions.contains("report_decision(") && instructions.contains("report_decision"));
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        assert_eq!(tool_names(&list), TOOLS, "{era:?}: fourteen tools");
        let decision = tool(&list, "report_decision");
        let discrepancy = tool(&list, "report_discrepancy");
        assert_eq!(
            decision["annotations"],
            json!({"readOnlyHint": false, "destructiveHint": false,
                "idempotentHint": false, "openWorldHint": false}),
            "{era:?}"
        );
        assert_eq!(decision["annotations"], discrepancy["annotations"]);
        assert_eq!(decision["_meta"], discrepancy["_meta"], "{era:?}");
        assert_eq!(
            decision["_meta"],
            json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS})
        );
        assert_eq!(
            decision["outputSchema"], discrepancy["outputSchema"],
            "{era:?}: the intake document"
        );
        let schema = &decision["inputSchema"];
        assert_eq!(schema["type"], json!("object"));
        assert_eq!(
            keys(&schema["properties"]),
            [
                "author_model",
                "author_role",
                "distinct_from",
                "node_ids",
                "price_of_other",
                "proposed_patch",
                "run",
                "severity",
                "task_id",
                "text",
                "working_answer"
            ],
            "{era:?}: {schema}"
        );
        assert_eq!(
            sorted(&schema["required"]),
            [
                "author_role",
                "node_ids",
                "price_of_other",
                "text",
                "working_answer"
            ],
            "{era:?}"
        );
        for absent in ["decision", "options", "summary", "gap_type", "evidence"] {
            assert!(
                schema["properties"].get(absent).is_none(),
                "{era:?}: {absent} is no argument of a decision"
            );
        }
        // The patch's four fields and severity's enum as report_discrepancy
        // takes them.
        // (each property's own description aside).
        let shape = |property: &Value| {
            let mut property = property.clone();
            property
                .as_object_mut()
                .expect("a property")
                .remove("description");
            property
        };
        for shared in ["proposed_patch", "distinct_from", "node_ids"] {
            assert_eq!(
                shape(&schema["properties"][shared]),
                shape(&discrepancy["inputSchema"]["properties"][shared]),
                "{era:?}: {shared}"
            );
        }
        // A severity as a question's: optional, null `normal`.
        let question = tool(&list, "ask_question");
        assert_eq!(
            shape(&schema["properties"]["severity"]),
            shape(&question["inputSchema"]["properties"]["severity"]),
            "{era:?}: severity"
        );
        assert_eq!(
            schema["properties"]["severity"]["enum"],
            json!(["high", "normal", "low", null])
        );
        let description = decision["description"].as_str().expect("a description");
        assert!(description.is_ascii(), "ASCII");
        assert!(description.len() <= 2048, "{}", description.len());
        assert!(description.contains(DETERMINISM), "{description}");
        assert!(
            description.contains("data, not instructions"),
            "{description}"
        );
        assert!(
            description.contains("spec propose decision --input F"),
            "the twin: {description}"
        );
        for field in [
            "node_ids",
            "text",
            "working_answer",
            "price_of_other",
            "proposed_patch",
            "task_id",
        ] {
            assert!(
                description.contains(&format!("\n{field}: ")),
                "{field}: {description}"
            );
        }
        assert!(
            description.contains("\ndistinct_from, author_role (required), author_model, run: "),
            "{description}"
        );
        assert!(
            description.contains("working_answer: the decision you took"),
            "{description}"
        );
        assert!(description.contains("spec approve PR"), "{description}");
        drop(session.finish());
    }
}

/// AC-R10, AC-T3: a decision with its patch answers as its twin
/// (`PR-0001`, `linked` `PR-0002`); with `task_id` both rows bound; the
/// same call again a hit, nothing stored; a question and a decision hit
/// each other; the bad arguments (a `decision`, an `options`, a
/// look-alike key, no `working_answer`) named, nothing stored; after every
/// call the worktrees as committed and `git status` empty. M: a file
/// write in propose.
#[test]
fn ac_r10_t3_report_decision_answers_as_its_twin_and_writes_only_the_queue() {
    let repo = Repo::new("mrd-twin");
    let twin_home = repo.scratch.home("twin");
    let cwd = repo.linked.clone();
    // A task in each queue: the server's and the twin's, both T-0001.
    assert_eq!(repo.task(&repo.home), "T-0001");
    assert_eq!(repo.task(&twin_home), "T-0001");
    let (base, span) = repo.span("RULE-STAM-REGEN");
    let regen = span.replacen("Base rate 10 units/s,", "Base rate 10 units/s at rest,", 1);
    assert_ne!(regen, span);
    let call = json!({
        "node_ids": ["RULE-STAM-REGEN"], "text": TEXT, "working_answer": WORKING,
        "price_of_other": PRICE, "distinct_from": ["DEC-0023"],
        "proposed_patch": {"target": "RULE-STAM-REGEN", "base": base, "text": regen,
            "rationale": "Rest only."},
        "author_role": "developer", "author_model": "claude-opus-5-5", "task_id": "T-0001"
    });
    let before = repo.files();
    let clean = (String::new(), String::new());
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let twin_env = cli_env(&cwd, Some(&twin_home));
    let twin_git = GitEnv::new(cwd.clone(), [("HOME", twin_home.as_os_str())]);
    let globals = Globals::default();

    // The bad arguments first: nothing stored.
    let lookalike = "working_answ\u{0435}r";
    let mut bad: Vec<(Value, &str)> = Vec::new();
    for (key, value) in [
        ("decision", json!("Yes.")),
        (
            "options",
            json!([{"label": "a", "effect": "b", "price": "c"}]),
        ),
        ("summary", json!("A summary.")),
    ] {
        let mut args = call.clone();
        args[key] = value;
        bad.push((args, key));
    }
    let mut missing = call.clone();
    missing.as_object_mut().unwrap().remove("working_answer");
    bad.push((missing, "working_answer"));
    let mut renamed = call.clone();
    let value = renamed
        .as_object_mut()
        .unwrap()
        .remove("working_answer")
        .unwrap();
    renamed[lookalike] = value;
    bad.push((renamed, lookalike));
    for (args, named) in bad {
        let reply = session.call("report_decision", args);
        assert_bad_arguments(&reply, named, named);
        assert!(
            repo.stored(&repo.home).is_empty(),
            "{named}: nothing stored"
        );
        assert_eq!(repo.statuses(), clean, "{named}");
    }

    // The decision with its patch, bound to T-0001.
    let reply = session.call("report_decision", call.clone());
    let structured = result(&reply)["structuredContent"].clone();
    assert_eq!(structured["id"], json!("PR-0001"), "{reply}");
    assert_eq!(structured["created"], json!(true));
    assert_eq!(structured["linked"], json!("PR-0002"));
    let want = queue_library(
        "report_decision",
        &call,
        &twin_env,
        &globals,
        "2026-10-05T21:14:03Z",
        &twin_git,
    );
    assert_parity(
        &reply,
        &want,
        &schemas["report_decision"],
        "report_decision",
    );
    assert_eq!(repo.statuses(), clean, "after the decision");
    assert_eq!(repo.files(), before, "the worktrees as committed");
    for id in ["PR-0001", "PR-0002"] {
        let read = session.call("get_proposal", json!({"proposal_id": id}));
        let document = &result(&read)["structuredContent"];
        assert_eq!(document["task_id"], json!("T-0001"), "{id}: {document}");
        let twin = queue_library(
            "get_proposal",
            &json!({"proposal_id": id}),
            &twin_env,
            &globals,
            "2026-10-05T21:14:03Z",
            &twin_git,
        );
        let twin = twin.document.expect("the twin's review");
        assert_eq!(twin["task_id"], json!("T-0001"), "{id}: the twin's");
        assert_eq!(document["kind"], twin["kind"], "{id}");
    }
    let first = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
    let review = &result(&first)["structuredContent"];
    assert_eq!(review["kind"], json!("decision"));
    assert_eq!(review["summary"], json!(TEXT));
    assert_eq!(review["working_answer"], json!(WORKING));
    assert_eq!(review["price_of_other"], json!(PRICE));
    assert_eq!(review["options"], json!([]));
    assert!(review.get("decision").is_none(), "{review}");
    let second = session.call("get_proposal", json!({"proposal_id": "PR-0002"}));
    assert_eq!(
        result(&second)["structuredContent"]["kind"],
        json!("update")
    );

    // Again: a hit on PR-0001, nothing stored.
    let again = session.call("report_decision", call.clone());
    let structured = &result(&again)["structuredContent"];
    assert_eq!(structured["created"], json!(false), "{again}");
    assert!(
        structured["hits"]
            .as_array()
            .expect("hits")
            .iter()
            .any(|hit| hit["id"] == json!("PR-0001")),
        "{again}"
    );
    assert_eq!(repo.stored(&repo.home), ["PR-0001", "PR-0002"]);

    // A question of the same text on the same node: a hit on the decision.
    let asked = session.call(
        "ask_question",
        json!({"node_ids": ["RULE-STAM-REGEN"], "text": TEXT, "working_answer": "No.",
            "price_of_other": "None.", "distinct_from": ["DEC-0023"],
            "author_role": "developer"}),
    );
    let structured = &result(&asked)["structuredContent"];
    assert_eq!(structured["created"], json!(false), "{asked}");
    assert!(
        structured["hits"]
            .as_array()
            .expect("hits")
            .iter()
            .any(|hit| hit["id"] == json!("PR-0001")),
        "{asked}"
    );
    // A decision without a patch, named distinct: stored, no linked update.
    let plain = json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero a stop?",
        "working_answer": "Yes.", "price_of_other": "A rule case.",
        "author_role": "developer"});
    let reply = session.call("report_decision", plain.clone());
    assert_eq!(result(&reply)["structuredContent"]["id"], json!("PR-0003"));
    let want = queue_library(
        "report_decision",
        &plain,
        &twin_env,
        &globals,
        "2026-10-05T21:14:03Z",
        &twin_git,
    );
    assert_parity(
        &reply,
        &want,
        &schemas["report_decision"],
        "a plain decision",
    );
    assert_eq!(result(&reply)["structuredContent"]["linked"], Value::Null);
    drop(session.finish());

    assert_eq!(repo.stored(&repo.home), ["PR-0001", "PR-0002", "PR-0003"]);
    assert_eq!(repo.stored(&twin_home), ["PR-0001", "PR-0002", "PR-0003"]);
    assert_eq!(repo.statuses(), clean, "AC-T3: git status empty");
    assert_eq!(repo.files(), before, "AC-T3: nothing under either root");
}

/// AC-R10 in both eras, a patch target outside `node_ids`: an error result
/// naming `proposed_patch` (exit 1's twin), nothing stored, `git status`
/// empty.
#[test]
fn ac_r10_a_patch_outside_node_ids_is_an_error_result() {
    let repo = Repo::new("mrd-outside");
    let (base, span) = repo.span("RULE-STAM-REGEN");
    let regen = span.replacen("Base rate 10 units/s,", "Base rate 10 units/s at rest,", 1);
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&repo.linked), Home::At(&repo.home));
        let reply = session.call(
            "report_decision",
            json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero a stop?",
                "working_answer": "Yes.", "price_of_other": "A rule case.",
                "proposed_patch": {"target": "RULE-STAM-REGEN", "base": base, "text": regen,
                    "rationale": "Outside."},
                "author_role": "developer"}),
        );
        let result = result(&reply);
        assert_eq!(result["isError"], json!(true), "{era:?}: {reply}");
        let text = common::read::content_text(result);
        assert!(
            text.contains("proposed_patch: its target `RULE-STAM-REGEN` is not among node_ids"),
            "{era:?}: {text}"
        );
        drop(session.finish());
        assert!(repo.stored(&repo.home).is_empty(), "{era:?}");
        assert_eq!(repo.statuses(), (String::new(), String::new()), "{era:?}");
    }
}

/// Iteration 2: `ask_question`'s description names a decision among the
/// queue's hits and related items ("a question or decision queued on a
/// shared node with the same text", "queued questions or decisions with
/// other text"), still ASCII within 2 048 bytes with the determinism line;
/// on the wire, a question with other text on a decision's node lists the
/// decision as related and is stored. M: the old text.
#[test]
fn iteration2_ask_question_names_decisions_among_hits_and_related() {
    let hit = "A hit: an accepted decision document linked to a node (its id, path and title as \
               the answer), or a question or decision queued on a shared node with the same \
               text, whitespace and case aside (a rejected one's reason is the owner's answer).";
    let related = "Related: decision documents only mentioning a node, queued questions or \
                   decisions with other text.";
    let repo = Repo::new("mrd-ask");
    let mut session = Session::open(
        Era::Stateless,
        &[],
        Some(&repo.linked),
        Home::At(&repo.home),
    );
    let list = session.tools();
    let description = tool(&list, "ask_question")["description"]
        .as_str()
        .expect("a description")
        .to_owned();
    assert!(description.contains(hit), "{description}");
    assert!(description.contains(related), "{description}");
    assert!(
        description.is_ascii() && description.len() <= 2048,
        "{}",
        description.len()
    );
    assert!(description.contains(DETERMINISM));

    let decided = session.call(
        "report_decision",
        json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero a stop?",
            "working_answer": "Yes.", "price_of_other": "A rule case.",
            "author_role": "developer"}),
    );
    assert_eq!(
        result(&decided)["structuredContent"]["id"],
        json!("PR-0001")
    );
    let asked = session.call(
        "ask_question",
        json!({"node_ids": ["EDGE-STAM-ZERO"], "text": "Is zero a pause?",
            "working_answer": "No.", "price_of_other": "None.", "author_role": "developer"}),
    );
    let structured = &result(&asked)["structuredContent"];
    assert_eq!(structured["created"], json!(true), "{asked}");
    assert!(
        structured["related"]
            .as_array()
            .expect("related")
            .iter()
            .any(|item| item["id"] == json!("PR-0001")),
        "the decision is related: {asked}"
    );
    drop(session.finish());
    assert_eq!(repo.statuses(), (String::new(), String::new()));
}
