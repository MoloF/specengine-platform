//! docs/features/task-package.md, the MCP half ("Description and
//! interactions", "MCP"): the five task tools of the default build.
//!
//! - The tools and their schemas: thirteen tools, no owner tool
//!   (`approve`, `changes`, `cancel` have none: AC-02); `get_task`'s input
//!   both nullable with no root `oneOf`; the four moving tools' required
//!   arguments and `{id, status, run, notes}` documents; `task_id`
//!   optional on the three queue writers; `INSTRUCTIONS` hold the tasks
//!   line (AC-14).
//! - 07 s1.2 P2-2: the package's output schema is pinned for
//!   `schema_version` 1: a key removed or renamed without a bump fails.
//! - `get_task` with both, neither or `next: false`: an error result
//!   naming `task_id` and `next`; an unknown task: an error result with the
//!   reason, no `structuredContent` (AC-03).
//! - P2-1 and P2-12: a generic agent's run (`get_task` → `claim_task` →
//!   `submit_plan` refused → `report_run` → `complete_task`, a bound
//!   question) each answered as its CLI twin (`get_task` as `spec task
//!   show --json` on the same state; a moving tool as the same command on a
//!   twin queue in another `HOME`); `git status --porcelain` of both
//!   worktrees empty and the files as they were after every call (AC-09).
//! - AC-12: a 128-node stale package: two `get_task` calls byte-identical,
//!   `content` within 48 000 characters with the brief's tail.
//! - AC-09 (07 P2-9): two projects in one `HOME` never see each other's
//!   tasks.
//! - 07 s1.2 P2-3: the package, brief and task sources and `plugin/**`
//!   name no stack word or role of this repository.
//!
//! Scratch git repositories of `fixtures/spec-a` (`-b`) in the CLI tests'
//! sandbox (`common/git.rs`, automatic maintenance off); every server with
//! a cleared environment and a scratch `HOME`; the owner's commands
//! through the CLI library, consent yes.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use common::read::{
    ERAS, Era, STACK_WORDS, Session, TASK_TOOLS, TOOLS, assert_parity, checked_call, cli_env,
    content_text, has_word, output_schemas, queue_library, task_library,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    Exit, GitEnv, Globals, TaskDecisionRequest, TaskNewRequest, TaskPlanRequest, task_approve,
    task_new, task_plan,
};

const CLOCK: &str = "2026-10-07T09:00:00Z";

/// The tasks line of `INSTRUCTIONS` ("Description and interactions").
const TASKS_LINE: &str = "- get_task, claim_task, submit_plan, report_run, complete_task = spec \
                          task show|claim|plan|report|complete; the three above take task_id.\n";

/// The exactly-one error of `get_task`.
const EXACTLY_ONE: &str = "spec: get_task takes exactly one of `task_id` and `next: true`\n";

struct Repo {
    scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    linked: PathBuf,
    home: PathBuf,
}

impl Repo {
    /// A copy of `fixture` committed on `main`, a linked worktree on `t1`.
    fn new(label: &str, fixture: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let main = scratch.copy(fixture, "main");
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
                linked.to_str().unwrap(),
            ],
        );
        let linked = fs::canonicalize(linked).unwrap();
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            main,
            linked,
            home,
        }
    }

    fn git_env(&self, cwd: &Path) -> GitEnv {
        GitEnv::new(cwd, self.git.vars())
    }

    /// A task on `nodes` made and approved (the owner, consent yes) in the
    /// main worktree, its data in `home`; `affected` planned first when
    /// given: its ID.
    fn ready(&self, home: &Path, nodes: &[&str], affected: &[&str]) -> String {
        let env = cli_env(&self.main, Some(home));
        let made = task_new(
            &env,
            &Globals::default(),
            &TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: Some("Tune the regeneration".to_owned()),
                goal: Some("The delay as measured.".to_owned()),
                author_role: None,
                author_model: None,
                run: None,
                now: CLOCK.to_owned(),
                git: self.git_env(&self.main),
            },
        )
        .expect("task new");
        let id = made.id.clone().expect("an ID");
        if !affected.is_empty() {
            let planned = task_plan(
                &env,
                &Globals::default(),
                &TaskPlanRequest {
                    id: id.clone(),
                    plan: specengine_cli::ProposedText::Given(b"1. Do.\n".to_vec()),
                    criteria: Vec::new(),
                    affected: affected.iter().map(|node| (*node).to_owned()).collect(),
                    now: CLOCK.to_owned(),
                    git: self.git_env(&self.main),
                },
            )
            .expect("task plan");
            assert_eq!(planned.exit(), Exit::Answered, "{planned:?}");
        }
        let mut yes = |_: &str| true;
        let approved = task_approve(
            &env,
            &Globals::default(),
            &TaskDecisionRequest {
                id: id.clone(),
                note: None,
                now: CLOCK.to_owned(),
                git: self.git_env(&self.main),
            },
            &mut yes,
        )
        .expect("task approve");
        assert_eq!(approved.exit(), Exit::Answered, "{approved:?}");
        id
    }

    /// `git status --porcelain=v1 --untracked-files=all` of both worktrees.
    fn porcelain(&self) -> (String, String) {
        let status = |dir: &Path| {
            self.git
                .git_text(dir, &["status", "--porcelain=v1", "--untracked-files=all"])
        };
        (status(&self.main), status(&self.linked))
    }

    /// Both worktrees' files (`.git` aside).
    fn files(&self) -> (BTreeSet<String>, Vec<Option<Vec<u8>>>) {
        let mut names = BTreeSet::new();
        let mut bytes = Vec::new();
        for dir in [&self.main, &self.linked] {
            for (path, content) in snapshot(dir) {
                if path == ".git" || path.starts_with(".git/") {
                    continue;
                }
                names.insert(format!("{}:{path}", dir.display()));
                bytes.push(content);
            }
        }
        (names, bytes)
    }
}

// ------------------------------------------------------- tools, schemas

/// The input schema's property `name` of `tool`.
fn property<'a>(tool: &'a Value, name: &str) -> &'a Value {
    &tool["inputSchema"]["properties"][name]
}

/// A schema admits `null` (`type` holds it, or an `anyOf` branch is it).
fn nullable(schema: &Value) -> bool {
    match &schema["type"] {
        Value::String(name) => name == "null",
        Value::Array(names) => names.iter().any(|name| name == "null"),
        _ => schema["anyOf"]
            .as_array()
            .is_some_and(|branches| branches.iter().any(nullable)),
    }
}

/// The input schema's `required`, sorted.
fn required(tool: &Value) -> Vec<String> {
    let mut names: Vec<String> = tool["inputSchema"]["required"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .map(|name| name.as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// AC-02 (the MCP half), AC-14 and "Description and interactions": the
/// default build lists the thirteen tools in both eras, none of them an
/// owner's command; `get_task` takes `task_id` and `next`, both nullable,
/// none required, no root `oneOf`/`anyOf`/`allOf`; the moving tools
/// require their arguments (arrays where "Data" lists them) and answer
/// `{id, status, run, notes}`; `task_id` is an optional property of the
/// three queue writers; every description within 2 048 characters;
/// `INSTRUCTIONS` hold the tasks line once (138 bytes, under 140).
/// M: a 160-byte tasks line (the build's own assert).
#[test]
fn ac14_the_task_tools_and_their_schemas() {
    assert_eq!(TASKS_LINE.len(), 138);
    assert!(TASKS_LINE.len() < 140);
    assert_eq!(specengine_mcp::INSTRUCTIONS.matches(TASKS_LINE).count(), 1);
    assert_eq!(specengine_mcp::INSTRUCTIONS.len(), 1878);
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        let names = tool_names(&list);
        assert_eq!(names, TOOLS, "{era:?}");
        for name in &names {
            for owner in ["approve", "changes", "cancel", "decide", "review_"] {
                assert!(!name.contains(owner), "{era:?}: an owner's tool {name}");
            }
            let description = tool(&list, name)["description"].as_str().unwrap();
            assert!(description.chars().count() <= 2048, "{name}");
        }
        for name in TASK_TOOLS {
            assert!(names.iter().any(|listed| listed == name), "{name}");
        }

        let get = tool(&list, "get_task");
        let input = &get["inputSchema"];
        for key in ["oneOf", "anyOf", "allOf"] {
            assert!(input.get(key).is_none(), "{era:?}: a root {key}: {input}");
        }
        let properties: BTreeSet<&String> =
            input["properties"].as_object().unwrap().keys().collect();
        assert_eq!(
            properties.into_iter().cloned().collect::<Vec<_>>(),
            ["next", "task_id"]
        );
        assert!(nullable(property(get, "task_id")), "{input}");
        assert!(nullable(property(get, "next")), "{input}");
        assert!(required(get).is_empty(), "{input}");
        assert_eq!(get["annotations"]["readOnlyHint"], json!(true));

        for (name, want) in [
            ("claim_task", vec!["role", "task_id", "worktree"]),
            (
                "submit_plan",
                vec!["affected_nodes", "criteria", "plan_md", "task_id"],
            ),
            (
                "report_run",
                vec!["changed_files", "outcome", "summary", "task_id"],
            ),
            ("complete_task", vec!["task_id"]),
        ] {
            let moving = tool(&list, name);
            assert_eq!(required(moving), want, "{era:?} {name}");
            assert_eq!(moving["annotations"]["readOnlyHint"], json!(false));
            let output = &moving["outputSchema"];
            let mut keys: Vec<&String> = output["properties"].as_object().unwrap().keys().collect();
            keys.sort();
            assert_eq!(keys, ["id", "notes", "run", "status"], "{era:?} {name}");
        }
        for (name, array) in [
            ("submit_plan", "criteria"),
            ("submit_plan", "affected_nodes"),
            ("report_run", "changed_files"),
        ] {
            assert_eq!(
                property(tool(&list, name), array)["type"],
                json!("array"),
                "{name}.{array}"
            );
        }
        for name in ["propose_change", "ask_question", "report_discrepancy"] {
            let writer = tool(&list, name);
            assert!(
                writer["inputSchema"]["properties"].get("task_id").is_some(),
                "{name}: no task_id"
            );
            assert!(nullable(property(writer, "task_id")), "{name}");
            assert!(!required(writer).contains(&"task_id".to_owned()), "{name}");
        }
        if era == Era::Stateless {
            let discover = result(&session.request("server/discover", json!({}))).clone();
            assert_eq!(
                discover["instructions"].as_str(),
                Some(specengine_mcp::INSTRUCTIONS)
            );
        }
        drop(session.finish());
    }
}

// ----------------------------------------------------------------- P2-2

/// Every property path of a JSON Schema (`a.b[].c`), every branch of an
/// `anyOf`/`oneOf`/`allOf` followed.
fn schema_paths(schema: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    if let Some(properties) = schema["properties"].as_object() {
        for (key, value) in properties {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            out.insert(path.clone());
            schema_paths(value, &path, out);
        }
    }
    if schema.get("items").is_some() {
        schema_paths(&schema["items"], &format!("{prefix}[]"), out);
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = schema[key].as_array() {
            for branch in branches {
                schema_paths(branch, prefix, out);
            }
        }
    }
}

/// Every key path of a JSON value.
fn value_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (key, inner) in object {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                out.insert(path.clone());
                value_paths(inner, &path, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                value_paths(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

/// The package's key paths at `schema_version` 1
/// (docs/canon/task-package.md "Package", "Versioning"). A key may be added at version 1; a
/// key removed, renamed or re-meant raises the version, and this list is
/// re-pinned with it.
const PACKAGE_V1: &[&str] = &[
    "schema_version",
    "id",
    "project",
    "status",
    "title",
    "goal",
    "profile",
    "stale",
    "targets",
    "targets[].id",
    "targets[].path",
    "targets[].kind",
    "targets[].title",
    "criteria",
    "criteria[].ref",
    "criteria[].text",
    "affected_nodes",
    "plan",
    "assumptions",
    "assumptions[].proposal",
    "assumptions[].text",
    "open_proposals",
    "open_proposals[].id",
    "open_proposals[].kind",
    "open_proposals[].status",
    "open_proposals[].target_ids",
    "open_proposals[].task_id",
    "open_proposals[].summary",
    "owner_notes",
    "owner_notes[].at",
    "owner_notes[].note",
    "bindings",
    "spec_snapshot",
    "spec_snapshot.at",
    "spec_snapshot.place",
    "spec_snapshot.place.worktree",
    "spec_snapshot.place.root_rel",
    "spec_snapshot.place.branch",
    "spec_snapshot.place.commit",
    "spec_snapshot.nodes",
    "spec_snapshot.nodes[].id",
    "spec_snapshot.nodes[].path",
    "spec_snapshot.nodes[].span_hash",
    "snapshot_diff",
    "snapshot_diff[].id",
    "snapshot_diff[].path",
    "snapshot_diff[].span_hash",
    "snapshot_diff[].diff",
    "snapshot_diff[].cut",
    "claim",
    "claim.at",
    "claim.role",
    "claim.worktree",
    "claim.branch",
    "runs",
    "runs[].run",
    "runs[].role",
    "runs[].started_at",
    "runs[].ended_at",
    "runs[].outcome",
    "runs[].summary",
    "runs[].changed_files",
    "bundle",
    "bundle.node_ids",
    "bundle.budget",
    "bundle.bundle_hash",
    "author",
    "author.type",
    "author.role",
    "author.model",
    "author.run",
    "created_at",
    "updated_at",
    "notes",
];

/// 07 s1.2 P2-2: `get_task`'s output schema at `schema_version` 1 holds
/// every pinned key path (a key removed or renamed without a bump: red);
/// a real package (every part present) reports `schema_version` 1 and
/// holds no key path the schema lacks. M: a key renamed without a bump.
#[test]
fn p2_2_the_package_schema_is_pinned_at_version_1() {
    let repo = Repo::new("mt-p22", "spec-a");
    let id = repo.ready(&repo.home, &["MEC-STAMINA"], &["RULE-STAM-REGEN"]);
    replace(&repo.main, "docs/spec/movement/stamina.md", "1.5 s", "2 s");
    let mut session = Session::open(Era::Legacy, &[], Some(&repo.main), Home::At(&repo.home));
    let list = session.tools();
    let mut schema = BTreeSet::new();
    schema_paths(&tool(&list, "get_task")["outputSchema"], "", &mut schema);
    let missing: Vec<&&str> = PACKAGE_V1
        .iter()
        .filter(|path| !schema.contains(**path))
        .collect();
    assert!(
        missing.is_empty(),
        "keys of schema_version 1 gone from the schema (a bump re-pins them): {missing:?}"
    );
    let reply = session.call(
        "claim_task",
        json!({"task_id": id, "role": "developer", "worktree": repo.main.to_str().unwrap()}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    let reply = session.call(
        "report_run",
        json!({"task_id": id, "outcome": "partial", "summary": "Half.",
               "changed_files": ["a.txt"]}),
    );
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    let reply = session.call("get_task", json!({"task_id": id}));
    let package = &result(&reply)["structuredContent"];
    assert_eq!(package["schema_version"], json!(1), "{reply}");
    assert_eq!(package["stale"], json!(true));
    let mut held = BTreeSet::new();
    value_paths(package, "", &mut held);
    let unknown: Vec<&String> = held.difference(&schema).collect();
    assert!(unknown.is_empty(), "keys the schema lacks: {unknown:?}");
    drop(session.finish());
}

// ---------------------------------------------------------------- AC-03

/// AC-03: `get_task` with both, neither or `next: false` is an error
/// result naming `task_id` and `next`, without `structuredContent`; an
/// unknown task or no `ready` one is an error result with the reason, no
/// `structuredContent` (the output schema is the package's); a look-alike
/// ID its exit-2 line.
#[test]
fn ac03_get_task_takes_exactly_one_of_its_arguments() {
    let repo = Repo::new("mt-ac03", "spec-a");
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&repo.main), Home::At(&repo.home));
        for args in [
            json!({"task_id": "T-0001", "next": true}),
            json!({}),
            json!({"next": false}),
            json!({"task_id": null, "next": null}),
            json!({"task_id": "T-0001", "next": false}),
        ] {
            let reply = session.call("get_task", args.clone());
            let result = result(&reply);
            assert_eq!(result["isError"], json!(true), "{era:?} {args}");
            assert_eq!(content_text(result), EXACTLY_ONE, "{era:?} {args}");
            assert!(result.get("structuredContent").is_none(), "{era:?} {args}");
        }
        for (args, text) in [
            (
                json!({"task_id": "T-0099"}),
                "spec: no task T-0099 in this repository\n",
            ),
            (
                json!({"next": true}),
                "spec: no ready task in this repository\n",
            ),
        ] {
            let reply = session.call("get_task", args.clone());
            let result = result(&reply);
            assert_eq!(result["isError"], json!(true), "{era:?} {args}");
            assert_eq!(content_text(result), text, "{era:?} {args}");
            assert!(result.get("structuredContent").is_none(), "{era:?} {args}");
        }
        let reply = session.call("get_task", json!({"task_id": "\u{0422}-0001"}));
        let result = result(&reply);
        assert_eq!(result["isError"], json!(true));
        assert!(
            content_text(result).starts_with("spec: ") && content_text(result).contains("T-0001"),
            "{result}"
        );
        drop(session.finish());
    }
}

// ----------------------------------------------------- P2-1, P2-12, AC-09

/// P2-1, P2-12 and AC-09: an agent's run on a project without a profile,
/// in the linked worktree, every call answered as its twin; after each,
/// both worktrees' `git status --porcelain` empty and their files as
/// they were. A moving tool's twin is the same command on a twin queue
/// (another `HOME`, the same task made and approved at the same clock);
/// `get_task`'s is `spec task show --json` on the server's own queue.
#[test]
fn p2_1_an_agents_run_answers_as_its_twins_and_writes_nothing_under_the_roots() {
    let repo = Repo::new("mt-p21", "spec-a");
    let twin_home = repo.scratch.home("twin");
    let id = repo.ready(&repo.home, &["MEC-STAMINA"], &["RULE-STAM-REGEN"]);
    assert_eq!(
        repo.ready(&twin_home, &["MEC-STAMINA"], &["RULE-STAM-REGEN"]),
        id
    );
    let cwd = repo.linked.clone();
    let files = repo.files();
    let env = cli_env(&cwd, Some(&repo.home));
    let twin_env = cli_env(&cwd, Some(&twin_home));
    let twin_git = GitEnv::new(cwd.clone(), [("HOME", twin_home.as_os_str())]);
    let globals = Globals::default();
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let clean = |context: &str| {
        assert_eq!(
            repo.porcelain(),
            (String::new(), String::new()),
            "{context}"
        );
        assert!(repo.files() == files, "{context}: the files changed");
    };

    let read = |session: &mut Session, args: Value| {
        let context = format!("get_task {args}");
        let result = checked_call(session, &schemas, "get_task", args, &env, &globals);
        clean(&context);
        result
    };
    let first = read(&mut session, json!({"task_id": id}));
    assert_eq!(first["structuredContent"]["status"], json!("ready"));
    assert_eq!(first["structuredContent"]["profile"], Value::Null);
    let next = read(&mut session, json!({"next": true}));
    assert_eq!(next["structuredContent"], first["structuredContent"]);

    let moves: Vec<(&str, Value, &str, bool)> = vec![
        (
            "claim_task",
            json!({"task_id": id, "role": "developer", "worktree": "."}),
            "in_progress",
            false,
        ),
        (
            "submit_plan",
            json!({"task_id": id, "plan_md": "1. Again.", "criteria": [],
                   "affected_nodes": []}),
            "in_progress",
            true,
        ),
        ("complete_task", json!({"task_id": id}), "in_progress", true),
        (
            "report_run",
            json!({"task_id": id, "outcome": "completed", "summary": "The delay is 2 s.",
                   "changed_files": ["src/stamina.rs"]}),
            "in_progress",
            false,
        ),
        ("complete_task", json!({"task_id": id}), "done", false),
        (
            "claim_task",
            json!({"task_id": "T-0099", "role": "developer", "worktree": "."}),
            "",
            true,
        ),
    ];
    for (index, (tool, args, status, refused)) in moves.into_iter().enumerate() {
        let reply = session.call(tool, args.clone());
        let want = task_library(tool, &args, &twin_env, &globals, CLOCK, &twin_git);
        let context = format!("{tool} {args}");
        assert_parity(&reply, &want, &schemas[tool], &context);
        let document = &result(&reply)["structuredContent"];
        assert_eq!(result(&reply)["isError"], json!(refused), "{context}");
        if !status.is_empty() {
            assert_eq!(document["status"], json!(status), "{context}");
        }
        clean(&context);
        if index == 0 {
            // Claimed: a bound question raised here is stored; in the twin
            // too.
            let args = json!({"node_ids": ["MEC-STAMINA"], "text": "Is it per second?",
                "working_answer": "Yes.", "price_of_other": "A new case.",
                "author_role": "developer", "task_id": id, "distinct_from": ["DEC-0023"]});
            let reply = session.call("ask_question", args.clone());
            let stamped = {
                let created = result(&reply)["structuredContent"]["id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("not stored: {reply}"));
                let stored = session.call("get_proposal", json!({"proposal_id": created}));
                result(&stored)["structuredContent"]["created_at"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            };
            let want = queue_library(
                "ask_question",
                &args,
                &twin_env,
                &globals,
                &stamped,
                &twin_git,
            );
            assert_parity(&reply, &want, &schemas["ask_question"], "bound ask");
            let stored = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
            assert_eq!(
                result(&stored)["structuredContent"]["task_id"],
                json!(id),
                "{stored}"
            );
            clean("ask_question");
        }
    }
    let last = read(&mut session, json!({"task_id": id}));
    let package = &last["structuredContent"];
    assert_eq!(package["status"], json!("done"));
    assert_eq!(
        package["claim"]["worktree"],
        json!(cwd.display().to_string())
    );
    assert_eq!(package["runs"][0]["outcome"], json!("completed"));
    assert_eq!(package["open_proposals"][0]["task_id"], json!(id));
    assert_eq!(package["assumptions"][0]["text"], json!("Yes."));
    // A done task takes no bound proposal.
    let args = json!({"node_ids": ["MEC-STAMINA"], "text": "Later?", "working_answer": "No.",
        "price_of_other": "None.", "author_role": "developer", "task_id": id,
        "distinct_from": ["DEC-0023"]});
    let reply = session.call("ask_question", args.clone());
    let want = queue_library("ask_question", &args, &twin_env, &globals, CLOCK, &twin_git);
    assert_parity(&reply, &want, &schemas["ask_question"], "done task");
    assert!(
        content_text(result(&reply)).contains("is done: a proposal is bound only to a task"),
        "{reply}"
    );
    clean("ask_question on a done task");
    drop(session.finish());
    clean("after the session");
}

// ---------------------------------------------------------------- AC-12

/// A synthetic project of 130 sections (`ITEM-001` … `ITEM-130`, ten per
/// file, about 8 KiB each) in `dir`, committed.
fn ledger(repo: &Repo) -> PathBuf {
    ledger_sized(repo, 110)
}

/// [`ledger`] with `ITEM-001` … `ITEM-032` of `big` lines (37 bytes each).
fn ledger_sized(repo: &Repo, big: usize) -> PathBuf {
    let root = repo.scratch.dir("ledger");
    write(
        &root,
        "specengine.toml",
        "[project]\nslug = \"big-ledger\"\n\n[ids]\nITEM = { kind = \"item\", width = 3 }\n",
    );
    for file in 0..13 {
        let mut text = format!("# Ledger {file}\n\nEntries.\n");
        for item in 1..=10 {
            let number = file * 10 + item;
            text.push_str(&format!("\n## Item {number} {{#ITEM-{number:03}}}\n"));
            let lines = if number <= 32 { big } else { 110 };
            for line in 0..lines {
                text.push_str(&format!(
                    "Line {line:03} of item {number:03}: a plain entry.\n"
                ));
            }
        }
        write(&root, &format!("docs/spec/ledger-{file:02}.md"), text);
    }
    repo.git.init(&root);
    repo.git.add_all(&root);
    repo.git.commit(&root, "ledger");
    root
}

/// AC-12 (and 07 s1.2 P2-10): 128 snapshot nodes all edited: two
/// `get_task` calls give byte-identical replies; `content` (the brief)
/// stays within 48 000 characters and ends with the tail; the package's
/// diffs within 262 144 B, one note naming those left out; the CLI's
/// answer the same. M: the read time in the package.
#[test]
fn ac12_a_full_package_is_deterministic_and_its_content_capped() {
    let repo = Repo::new("mt-ac12", "spec-a");
    let root = ledger(&repo);
    let env = cli_env(&root, Some(&repo.home));
    let items: Vec<String> = (1..=128)
        .map(|number| format!("ITEM-{number:03}"))
        .collect();
    let made = task_new(
        &env,
        &Globals::default(),
        &TaskNewRequest {
            nodes: items[..64].to_vec(),
            title: Some("All of it".to_owned()),
            goal: None,
            author_role: None,
            author_model: None,
            run: None,
            now: CLOCK.to_owned(),
            git: repo.git_env(&root),
        },
    )
    .expect("task new");
    let id = made.id.clone().unwrap();
    let planned = task_plan(
        &env,
        &Globals::default(),
        &TaskPlanRequest {
            id: id.clone(),
            plan: specengine_cli::ProposedText::Given(b"All.\n".to_vec()),
            criteria: Vec::new(),
            affected: items[64..].to_vec(),
            now: CLOCK.to_owned(),
            git: repo.git_env(&root),
        },
    )
    .expect("plan");
    assert_eq!(planned.exit(), Exit::Answered, "{planned:?}");
    let mut yes = |_: &str| true;
    let approved = task_approve(
        &env,
        &Globals::default(),
        &TaskDecisionRequest {
            id: id.clone(),
            note: None,
            now: CLOCK.to_owned(),
            git: repo.git_env(&root),
        },
        &mut yes,
    )
    .expect("approve");
    assert_eq!(approved.exit(), Exit::Answered, "{approved:?}");
    for file in 0..13 {
        let path = format!("docs/spec/ledger-{file:02}.md");
        let text = read_text(&root, &path);
        write(&root, &path, text.replace("a plain entry", "a PLAIN entry"));
    }
    let mut session = Session::open(Era::Legacy, &[], Some(&root), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let first = session.call("get_task", json!({"task_id": id}));
    let second = session.call("get_task", json!({"task_id": id}));
    assert_eq!(first["result"], second["result"], "byte-identical");
    let result = result(&first);
    let content = content_text(result);
    assert!(
        content.chars().count() <= 48_000,
        "{}",
        content.chars().count()
    );
    let brief = content
        .lines()
        .rev()
        .find(|line| line.starts_with("[truncated: sections not shown: "))
        .expect("the tail");
    assert!(
        brief.ends_with("; spec task show T-0001 --json carries every key]"),
        "{brief}"
    );
    let package = &result["structuredContent"];
    let diffs = package["snapshot_diff"].as_array().unwrap();
    assert_eq!(diffs.len(), 128);
    let total: usize = diffs
        .iter()
        .filter_map(|diff| diff["diff"].as_str())
        .map(str::len)
        .sum();
    assert!(total <= 262_144, "{total}");
    let left_out = diffs.iter().filter(|diff| diff["diff"].is_null()).count();
    assert!(left_out > 0);
    assert!(
        package["notes"]
            .as_array()
            .unwrap()
            .contains(&json!(format!(
                "snapshot_diff: {left_out} diff(s) past 262144 B left out"
            )))
    );
    drop(session.finish());
    let mut session = Session::open(Era::Stateless, &[], Some(&root), Home::At(&repo.home));
    checked_call(
        &mut session,
        &schemas,
        "get_task",
        json!({"task_id": id}),
        &env,
        &Globals::default(),
    );
    drop(session.finish());
}

// --------------------------------------------------------------- P2-9

/// AC-09 (07 s1.2 P2-9), the MCP half: two projects in one `HOME`, each
/// server its own: spec-b's sees no task of spec-a (`get_task` by ID and
/// `next` error results), and deleting spec-a's database leaves spec-b's
/// task. M: no slug filter.
#[test]
fn ac09_two_projects_in_one_home_see_only_their_own_tasks() {
    let a = Repo::new("mt-p29-a", "spec-a");
    let b = Repo::new("mt-p29-b", "spec-b");
    let home = a.home.clone();
    let id = a.ready(&home, &["MEC-STAMINA"], &[]);
    let mut session = Session::open(Era::Legacy, &[], Some(&b.main), Home::At(&home));
    for args in [json!({"task_id": id}), json!({"next": true})] {
        let reply = session.call("get_task", args.clone());
        assert_eq!(result(&reply)["isError"], json!(true), "{args}: {reply}");
    }
    drop(session.finish());
    // b's own task, then a's database deleted.
    let env = cli_env(&b.main, Some(&home));
    let made = task_new(
        &env,
        &Globals::default(),
        &TaskNewRequest {
            nodes: vec!["CMD-SYNC".to_owned()],
            title: Some("Sync".to_owned()),
            goal: None,
            author_role: None,
            author_model: None,
            run: None,
            now: CLOCK.to_owned(),
            git: b.git_env(&b.main),
        },
    )
    .expect("b's task");
    assert_eq!(made.id.as_deref(), Some("T-0001"));
    let db = data_dir(&home).join("lantern-keep.db");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", db.display()));
    }
    let mut session = Session::open(Era::Legacy, &[], Some(&b.main), Home::At(&home));
    let reply = session.call("get_task", json!({"task_id": "T-0001"}));
    let package = &result(&reply)["structuredContent"];
    assert_eq!(package["project"], json!("zerkalo"), "{reply}");
    assert_eq!(package["title"], json!("Sync"));
    drop(session.finish());
    let mut session = Session::open(Era::Legacy, &[], Some(&a.main), Home::At(&home));
    let reply = session.call("get_task", json!({"task_id": "T-0001"}));
    assert_eq!(result(&reply)["isError"], json!(true), "a's queue is gone");
    drop(session.finish());
}

// ----------------------------------------------------------------- P2-3

/// The task package's, brief's and task commands' sources (07 s1.2 P2-3).
const TASK_SOURCES: [&str; 6] = [
    "crates/specengine-model/src/task.rs",
    "crates/specengine-core/src/task.rs",
    "crates/specengine-store/src/queue/tasks.rs",
    "crates/specengine-cli/src/task.rs",
    "crates/specengine-cli/src/package.rs",
    "crates/specengine-mcp/src/tasks.rs",
];

/// Every file under `dir`, recursively.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files_under(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// 07 s1.2 P2-3: the whole text (comments too) of the task sources and of
/// every file under `plugin/` holds none of the stack words or this
/// repository's role names, as whole words. M: `cargo` in the brief.
#[test]
fn p2_3_the_task_sources_and_the_plugin_name_no_stack_word() {
    let root = repository_root();
    let mut files: Vec<PathBuf> = TASK_SOURCES.iter().map(|path| root.join(path)).collect();
    files_under(&root.join("plugin"), &mut files);
    // Finder's own files are no plugin content.
    files.retain(|path| path.file_name().is_none_or(|name| name != ".DS_Store"));
    assert!(files.len() > TASK_SOURCES.len(), "{files:?}");
    let mut offenders = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        assert!(!text.is_empty(), "{path:?}");
        for (number, line) in text.lines().enumerate() {
            for word in STACK_WORDS {
                if has_word(line, word) {
                    offenders.push(format!("{}:{}: {word}", path.display(), number + 1));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
    // The scan sees what it must refuse.
    assert!(has_word("let brief = \"run cargo test\";", "cargo"));
    assert!(!has_word("a nested span", "nest"));
}

/// Iteration 2 (docs/canon/mcp-read.md "Size", docs/canon/task-package.md "Caps"):
/// every field at its cap, 32 reference criteria of 8 192-byte texts, 128
/// edited snapshot nodes, the claim and report at their caps: `get_task`
/// answers within `MAX_RESULT_CHARS` (500 000) — its `content` and its
/// `structuredContent` together — `content` within 48 000, the package's
/// notes naming what the budget left out. M: no package budget.
#[test]
fn get_task_at_every_cap_stays_within_max_result_chars() {
    let repo = Repo::new("mt-caps", "spec-a");
    let root = ledger_sized(&repo, 300);
    let env = cli_env(&root, Some(&repo.home));
    let git = repo.git_env(&root);
    let items = |range: std::ops::RangeInclusive<usize>| -> Vec<String> {
        range.map(|number| format!("ITEM-{number:03}")).collect()
    };
    let made = task_new(
        &env,
        &Globals::default(),
        &TaskNewRequest {
            nodes: items(1..=64),
            title: Some("T".repeat(256)),
            goal: Some("G".repeat(4096)),
            author_role: None,
            author_model: None,
            run: None,
            now: CLOCK.to_owned(),
            git: git.clone(),
        },
    )
    .expect("new");
    let id = made.id.clone().unwrap();
    let plan = |git: GitEnv| {
        let planned = task_plan(
            &env,
            &Globals::default(),
            &TaskPlanRequest {
                id: id.clone(),
                plan: specengine_cli::ProposedText::Given(
                    format!("{}\n", "p".repeat(16_383)).into_bytes(),
                ),
                criteria: items(1..=32),
                affected: items(65..=128),
                now: CLOCK.to_owned(),
                git,
            },
        )
        .expect("plan");
        assert_eq!(planned.exit(), Exit::Answered, "{planned:?}");
    };
    plan(git.clone());
    let mut yes = |_: &str| true;
    let returned = specengine_cli::task_changes(
        &env,
        &Globals::default(),
        &TaskDecisionRequest {
            id: id.clone(),
            note: Some("n".repeat(4096)),
            now: CLOCK.to_owned(),
            git: git.clone(),
        },
        &mut yes,
    )
    .expect("changes");
    assert_eq!(returned.exit(), Exit::Answered, "{returned:?}");
    plan(git.clone());
    let approved = task_approve(
        &env,
        &Globals::default(),
        &TaskDecisionRequest {
            id: id.clone(),
            note: None,
            now: CLOCK.to_owned(),
            git: git.clone(),
        },
        &mut yes,
    )
    .expect("approve");
    assert_eq!(approved.exit(), Exit::Answered, "{approved:?}");
    let mut session = Session::open(Era::Stateless, &[], Some(&root), Home::At(&repo.home));
    let claimed = session.call(
        "claim_task",
        json!({"task_id": id, "role": "r".repeat(128), "worktree": "."}),
    );
    assert_eq!(result(&claimed)["isError"], json!(false), "{claimed}");
    let file = "f".repeat(512);
    let reported = session.call(
        "report_run",
        json!({"task_id": id, "outcome": "abandoned", "summary": "s".repeat(4096),
               "changed_files": vec![file; 256]}),
    );
    assert_eq!(
        result(&reported)["isError"],
        json!(false),
        "{}",
        clip(&reported.to_string())
    );
    for file in 0..13 {
        let path = format!("docs/spec/ledger-{file:02}.md");
        let text = read_text(&root, &path);
        write(&root, &path, text.replace("a plain entry", "a PLAIN entry"));
    }
    let reply = session.call("get_task", json!({"task_id": id}));
    let answered = result(&reply);
    assert_eq!(answered["isError"], json!(false));
    let content = content_text(answered).chars().count();
    let structured = serde_json::to_string(&answered["structuredContent"])
        .unwrap()
        .chars()
        .count();
    assert!(content <= 48_000, "content {content}");
    assert!(
        content + structured <= 500_000,
        "content {content} + structuredContent {structured}"
    );
    let notes: Vec<&str> = answered["structuredContent"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        notes.iter().any(|note| note
            .ends_with(" more diff(s) left out to keep the package within 460000 characters")),
        "{notes:?}"
    );
    drop(session.finish());
}
