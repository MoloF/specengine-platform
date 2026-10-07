//! docs/features/agent-intake.md over MCP, the default build: the queue
//! tools `propose_change`, `ask_question`, `report_discrepancy` and
//! `get_proposal` as the real binary answers them over stdio.
//!
//! - AC-02: `open`, `approved` with its commit on its branch, `applied`,
//!   `rejected`: `get_proposal` and every write tool leave their rows and
//!   events, git and the files as they were. M: `get_proposal` via
//!   approve's completion lookup.
//! - AC-03: no `author_role` is rmcp's parameter error naming it, nothing
//!   stored; `"nest-developer"` alone is stored verbatim as an agent; `"a
//!   b"` refused. M: `author_role` optional, absent passed on.
//! - AC-05: 8 parallel identical `ask_question` (8 servers) and the twin:
//!   one row, every answer naming it. M: the queue read before `BEGIN
//!   IMMEDIATE`.
//! - AC-06 (the MCP half): an unknown argument and a missing `node_ids`
//!   are parameter errors; `node_ids: []` refused naming it; then a valid
//!   ask stores `PR-0001`.
//! - AC-12: "Tools" (annotations, `_meta`, descriptions; input schemas
//!   with no `$ref` and one level of closed inline objects; `kind` free in
//!   the output schemas); a 1 MiB `propose_change` and a discrepancy at
//!   every cap of control characters: each result at most
//!   `MAX_RESULT_CHARS`, `content` at most 48 000. M: `readOnlyHint: true`;
//!   the brief keeping `new_text`.
//! - AC-13: `content` byte-equal to the twin's `2>&1` and
//!   `structuredContent` its `--json` (the library with the same clock in
//!   another `HOME`); what the server made with `HOME` X is in `spec inbox`
//!   with X, not with another `HOME`. M: the server's own data directory.
//!
//! Every server runs with a cleared environment and a scratch `HOME`; the
//! setup's git (and the library's approve) runs in the CLI tests' sandbox
//! (`common/git.rs`, included by path). Compiles to nothing with
//! `--features probes` (the AC names the default build).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};

use common::read::{
    ERAS, Era, QUEUE_TOOLS, Session, WRITE_TOOLS, assert_bad_arguments, assert_parity, cli_env,
    content_text, output_schemas, queue_library,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    ApproveRequest, Env, ExportStateRequest, GitEnv, Globals, InboxRequest, ProposeRequest,
    ProposedText, QuestionRequest, RejectRequest, ShowRequest, TEXT_MAX_BYTES, approve,
    export_state, inbox, propose, propose_question, reject, show,
};

const NOW: &str = "2026-10-05T21:14:03Z";
const LATER: &str = "2026-10-06T08:00:00Z";
const DETERMINISM: &str = "Deterministic: one state, one result; no LLM inside.";

// ---------------------------------------------------------------- helpers

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

    fn env(&self, cwd: &Path, home: &Path) -> Env {
        cli_env(cwd, Some(home))
    }

    /// The sandbox's git environment (a fixed committer).
    fn git_env(&self, cwd: &Path) -> GitEnv {
        GitEnv::new(cwd, self.git.vars())
    }

    fn db(&self) -> PathBuf {
        data_dir(&self.home).join("lantern-keep.db")
    }

    /// `(span_hash, text)` of `reference` in `cwd`.
    fn span(&self, cwd: &Path, reference: &str) -> (String, String) {
        let outcome = show(
            &self.env(cwd, &self.home),
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

    /// Library `propose` of `reference`'s span with `from` → `to`, from the
    /// linked worktree: its ID.
    fn propose_edit(&self, reference: &str, from: &str, to: &str) -> String {
        let (base, text) = self.span(&self.linked, reference);
        assert_eq!(text.matches(from).count(), 1, "{from:?} in {text:?}");
        let outcome = propose(
            &self.env(&self.linked, &self.home),
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

    fn approve(&self, id: &str) {
        let mut consent = |_: &str| true;
        let outcome = approve(
            &self.env(&self.linked, &self.home),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: LATER.to_owned(),
                git: self.git_env(&self.linked),
            },
            &mut consent,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(
            outcome.document.status.as_deref(),
            Some("applied"),
            "{outcome:?}"
        );
    }

    fn reject(&self, id: &str, reason: &str) {
        let mut consent = |_: &str| true;
        let outcome = reject(
            &self.env(&self.linked, &self.home),
            &Globals::default(),
            &RejectRequest {
                id: id.to_owned(),
                reason: reason.to_owned(),
                now: LATER.to_owned(),
                git: self.git_env(&self.linked),
            },
            &mut consent,
        )
        .unwrap_or_else(|error| panic!("reject {id}: {error}"));
        assert_eq!(outcome.document.status.as_deref(), Some("rejected"));
    }

    /// The queue's whole backup (`spec export state`), its lines.
    fn dump(&self, home: &Path, name: &str) -> Vec<String> {
        let out = self.scratch.dir("dumps").join(format!("{name}.jsonl"));
        export_state(
            &self.env(&self.main, home),
            &Globals::default(),
            &ExportStateRequest {
                out: Some(out.clone()),
                now: NOW.to_owned(),
                git: self.git_env(&self.main),
            },
        )
        .unwrap_or_else(|error| panic!("export state: {error}"));
        fs::read_to_string(out)
            .expect("the dump")
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// `spec inbox --all` with `home`: the IDs listed.
    fn inbox(&self, home: &Path) -> Vec<String> {
        inbox(
            &self.env(&self.main, home),
            &Globals::default(),
            &InboxRequest {
                all: true,
                git: self.git_env(&self.main),
            },
        )
        .unwrap_or_else(|error| panic!("inbox: {error}"))
        .proposals
        .into_iter()
        .map(|entry| entry.id)
        .collect()
    }
}

/// `sqlite3 <db> <sql>`, which must succeed.
fn sqlite3(db: &Path, sql: &str) -> String {
    let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client");
    let output = Command::new(client)
        .arg(db)
        .arg(sql)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A question's arguments of a `writer` agent.
fn question_args(node_ids: &[&str], text: &str) -> Value {
    json!({"node_ids": node_ids, "text": text, "working_answer": "yes, 1.5 s",
        "price_of_other": "R-12 rebalanced", "author_role": "writer"})
}

/// A discrepancy's arguments on `node_ids`.
fn discrepancy_args(node_ids: &[&str], summary: &str) -> Value {
    json!({
        "node_ids": node_ids, "summary": summary, "gap_type": "contradicts",
        "severity": "high",
        "evidence": [{"file": "src/stamina.rs", "qpath": "stamina::regen", "lines": "3-9",
            "observed": "regenerates while walking", "documented": "only at rest"}],
        "options": [{"label": "code", "effect": "fix the code", "price": "1 item"},
            {"label": "spec", "effect": "allow walking", "price": "a rebalance"}],
        "recommendation": 0, "author_role": "writer"
    })
}

/// The structured document of a tool result that answered (no error).
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

/// A tool result that refused: its one content line.
fn refused_text(reply: &Value, context: &str) -> String {
    let result = result(reply);
    assert_eq!(
        result["isError"],
        json!(true),
        "{context}: {}",
        clip(&result.to_string())
    );
    content_text(result).to_owned()
}

// ------------------------------------------------------------------ AC-02

/// The lines of a dump about `ids`: their rows and their events.
fn about(lines: &[String], ids: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            let value: Value = serde_json::from_str(line).expect("a dump line");
            let id = value["proposals"]["id"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| {
                    value["events"]["payload"]
                        .as_str()
                        .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
                        .and_then(|payload| payload["id"].as_str().map(str::to_owned))
                });
            id.is_some_and(|id| ids.contains(&id.as_str()))
        })
        .cloned()
        .collect()
}

/// AC-02: `PR-0001` open, `PR-0002` applied, `PR-0003` approved with its
/// commit on its branch (applied, then set back), `PR-0004` rejected; in
/// both eras `get_proposal` reads each (its state as stored), and every
/// write tool stores its own item: the four rows and their events, both
/// worktrees' files and `.git`, every ref unchanged. M: `get_proposal` via
/// approve's completion lookup.
#[test]
fn ac02_the_queue_tools_change_no_state() {
    let repo = Repo::new("ai-ac02");
    assert_eq!(
        repo.propose_edit("EDGE-STAM-ZERO", "immediately", "at once"),
        "PR-0001"
    );
    let applied = repo.propose_edit("EDGE-SPRINT-EMPTY", "the sprint ends;", "it ends;");
    repo.approve(&applied);
    let held = repo.propose_edit("RULE-STAM-REGEN", "rate \u{d7} 0.5", "rate \u{d7} 0.4");
    repo.approve(&held);
    sqlite3(
        &repo.db(),
        "UPDATE proposals SET status = 'approved', applied_commit = NULL, \
         updated_at = decided_at WHERE id = 'PR-0003';",
    );
    let rejected = repo.propose_edit("MEC-SPRINT", "Hold the sprint key", "Hold the key");
    repo.reject(&rejected, "No.");
    let ids = ["PR-0001", "PR-0002", "PR-0003", "PR-0004"];
    let before = about(&repo.dump(&repo.home, "before"), &ids);
    assert_eq!(before.len(), 4 + 9, "four rows, nine events: {before:#?}");
    let trees = (snapshot(&repo.main), snapshot(&repo.linked));
    let refs = repo.git.git_text(
        &repo.main,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );

    let mut next = 5;
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&repo.linked), Home::At(&repo.home));
        for (id, status) in ids.iter().zip(["open", "applied", "approved", "rejected"]) {
            let reply = session.call("get_proposal", json!({"proposal_id": id}));
            let document = answered(&reply, id);
            assert_eq!(document["status"], json!(status), "{era:?} {id}");
            assert_eq!(document["kind"], json!("update"), "{era:?} {id}");
        }
        let reply = session.call("get_proposal", json!({"proposal_id": "PR-0003"}));
        let document = answered(&reply, "PR-0003");
        assert!(document["applied_commit"].is_null(), "{document}");
        assert_eq!(document["preview"], json!("unavailable"), "{document}");

        let reply = session.call(
            "ask_question",
            question_args(&["EDGE-STAM-ZERO"], &format!("Asked in {era:?}?")),
        );
        assert_eq!(
            answered(&reply, "ask")["id"],
            json!(format!("PR-{next:04}"))
        );
        next += 1;
        let base = |session: &mut Session, id: &str| {
            let reply = session.call("get_node", json!({"id": id}));
            result(&reply)["structuredContent"]["nodes"][0]["span_hash"]
                .as_str()
                .expect("span_hash")
                .to_owned()
        };
        let regen = base(&mut session, "RULE-STAM-REGEN");
        let mut args = discrepancy_args(&["RULE-STAM-REGEN"], &format!("Departs in {era:?}."));
        args["distinct_from"] = json!(["DEC-0023"]);
        args["proposed_patch"] = json!({"target": "RULE-STAM-REGEN", "base": regen,
            "text": "## Regeneration {#RULE-STAM-REGEN}\n- At rest only.\n",
            "rationale": "Rest."});
        let reply = session.call("report_discrepancy", args);
        let document = answered(&reply, "report");
        assert_eq!(document["id"], json!(format!("PR-{next:04}")), "{document}");
        assert_eq!(document["linked"], json!(format!("PR-{:04}", next + 1)));
        next += 2;
        let zero = base(&mut session, "EDGE-STAM-ZERO");
        let reply = session.call(
            "propose_change",
            json!({"kind": "update", "target": "EDGE-STAM-ZERO", "base": zero,
                "text": "## Depletion {#EDGE-STAM-ZERO}\n- At zero: `Exhausted`.\n",
                "rationale": "Shorter.", "author_role": "writer"}),
        );
        assert_eq!(
            answered(&reply, "change")["id"],
            json!(format!("PR-{next:04}"))
        );
        next += 1;
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(done.stderr, "", "{era:?}");
    }

    assert_eq!(
        about(&repo.dump(&repo.home, "after"), &ids),
        before,
        "the four rows and their events"
    );
    assert_eq!(snapshot(&repo.main), trees.0, "the main worktree and .git");
    assert_eq!(snapshot(&repo.linked), trees.1, "the linked worktree");
    assert_eq!(
        repo.git.git_text(
            &repo.main,
            &["for-each-ref", "--format=%(refname) %(objectname)"]
        ),
        refs,
        "no commit, no ref"
    );
    assert_eq!(
        sqlite3(
            &repo.db(),
            "SELECT status FROM proposals WHERE id = 'PR-0003';"
        ),
        "approved"
    );
}

// ------------------------------------------------------------------ AC-03

/// AC-03, the MCP half: a write tool without `author_role` is rmcp's
/// parameter error naming it, nothing stored; `"nest-developer"` alone is
/// stored `{"type":"agent","role":"nest-developer","model":null,"run":null}`;
/// `"a b"` is refused naming `author_role`, `author_model` `"m m"` and `run`
/// `""` naming theirs — `propose_change` too (exit 2, iteration 2). M:
/// `author_role` optional, absent passed on; `Author::new`'s flag naming
/// restored in `propose update`.
#[test]
fn ac03_every_write_names_its_agent() {
    let repo = Repo::new("ai-ac03");
    let mut session = Session::open(Era::Legacy, &[], Some(&repo.linked), Home::At(&repo.home));
    let (base, text) = repo.span(&repo.linked, "EDGE-STAM-ZERO");
    let change = json!({"kind": "update", "target": "EDGE-STAM-ZERO", "base": base,
        "text": text.replacen("immediately", "at once", 1), "rationale": "Shorter."});
    let mut question = question_args(&["EDGE-STAM-ZERO"], "Does it stop?");
    question.as_object_mut().unwrap().remove("author_role");
    let mut report = discrepancy_args(&["EDGE-STAM-ZERO"], "It departs.");
    report.as_object_mut().unwrap().remove("author_role");
    for (tool, args) in [
        ("propose_change", change.clone()),
        ("ask_question", question.clone()),
        ("report_discrepancy", report.clone()),
    ] {
        let reply = session.call(tool, args);
        assert_bad_arguments(&reply, "author_role", tool);
    }
    assert!(repo.inbox(&repo.home).is_empty(), "nothing stored");

    for (tool, mut args) in [
        ("propose_change", change),
        ("ask_question", question),
        ("report_discrepancy", report),
    ] {
        // Every tool names the field as its schema does (`author_role`,
        // `author_model`, `run`); `propose update` cannot run (exit 2: no
        // document), the intake refuses (exit 1: its document, the reason
        // its last note). Nothing stored either way.
        let stored = repo.inbox(&repo.home);
        for (field, value, problem) in [
            ("author_role", "a b", "printable ASCII without spaces only"),
            ("author_model", "m m", "printable ASCII without spaces only"),
            ("run", "", "empty; give a value or leave the option out"),
        ] {
            let mut bad = args.clone();
            if field != "author_role" {
                bad["author_role"] = json!("nest-developer");
            }
            bad[field] = json!(value);
            let reply = session.call(tool, bad);
            let context = format!("{tool} {field}");
            let text = refused_text(&reply, &context);
            assert_eq!(text, format!("spec: {field}: {problem}\n"), "{context}");
            let structured = &result(&reply)["structuredContent"];
            if tool == "propose_change" {
                assert!(
                    structured.is_null(),
                    "{context}: exit 2, no document: {structured}"
                );
            } else {
                assert_eq!(
                    structured["notes"],
                    json!([format!("{field}: {problem}")]),
                    "{context}"
                );
                assert!(structured["id"].is_null(), "{context}");
            }
            assert_eq!(repo.inbox(&repo.home), stored, "{context}: nothing stored");
        }
        args["author_role"] = json!("nest-developer");
        let document = answered(&session.call(tool, args), tool);
        let id = document["id"].as_str().expect("stored").to_owned();
        let reply = session.call("get_proposal", json!({"proposal_id": id}));
        assert_eq!(
            answered(&reply, &id)["author"],
            json!({"type": "agent", "role": "nest-developer", "model": null, "run": null}),
            "{tool}"
        );
    }
    assert_eq!(repo.inbox(&repo.home), ["PR-0001", "PR-0002", "PR-0003"]);
    drop(session.finish());
}

// ------------------------------------------------------------------ AC-05

/// AC-05: eight servers ask the same question at once and the twin (the
/// library, as `spec propose question`) with them: one row; every answer
/// names it — the one that stored it as its `id`, every other as its only
/// hit. Thirty rounds, a new question each (the race window of the
/// mutation is short: one round in ten or so catches it). M: the queue
/// read before `BEGIN IMMEDIATE`.
#[test]
fn ac05_eight_parallel_asks_and_the_twin_store_one_row() {
    let repo = Repo::new("ai-ac05");
    let mut sessions: Vec<Session> = (0..8)
        .map(|_| Session::open(Era::Legacy, &[], Some(&repo.main), Home::At(&repo.home)))
        .collect();
    for round in 0..30 {
        let text = format!("Does round {round} wait for rest?");
        let barrier = Arc::new(Barrier::new(9));
        let mut workers = Vec::new();
        for mut session in sessions.drain(..) {
            let barrier = Arc::clone(&barrier);
            let args = question_args(&["EDGE-STAM-ZERO"], &text);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                let reply = session.call("ask_question", args);
                (session, answered(&reply, "ask"))
            }));
        }
        let twin = {
            let barrier = Arc::clone(&barrier);
            let env = repo.env(&repo.main, &repo.home);
            let request = QuestionRequest {
                node_ids: vec!["EDGE-STAM-ZERO".to_owned()],
                text: text.clone(),
                working_answer: "yes, 1.5 s".to_owned(),
                price_of_other: "R-12 rebalanced".to_owned(),
                severity: None,
                distinct_from: Vec::new(),
                author_role: Some("writer".to_owned()),
                author_model: None,
                run: None,
                now: NOW.to_owned(),
                git: repo.git_env(&repo.main),
            };
            std::thread::spawn(move || {
                barrier.wait();
                let outcome =
                    propose_question(&env, &Globals::default(), &request).expect("the twin");
                serde_json::to_value(&outcome.document).expect("JSON")
            })
        };
        let mut documents = Vec::new();
        for worker in workers {
            let (session, document) = worker.join().expect("a session");
            sessions.push(session);
            documents.push(document);
        }
        documents.push(twin.join().expect("the twin"));
        let id = format!("PR-{:04}", round + 1);
        let stored: Vec<&Value> = documents
            .iter()
            .filter(|document| document["created"] == json!(true))
            .collect();
        assert_eq!(stored.len(), 1, "round {round}: {documents:#?}");
        assert_eq!(stored[0]["id"], json!(id));
        for document in &documents {
            if document["created"] == json!(false) {
                assert!(document["id"].is_null(), "{document}");
                assert_eq!(
                    document["hits"],
                    json!([{"id": id, "source": "queue", "status": "open", "path": null,
                        "answer": null, "record": null}]),
                    "round {round}"
                );
            }
        }
        let listed = repo.inbox(&repo.home);
        assert_eq!(listed.len(), round + 1, "round {round}: {listed:?}");
    }
    for session in sessions {
        let done = session.finish();
        assert!(done.status.success(), "{}", done.stderr);
        assert_eq!(done.stderr, "");
    }
}

// ------------------------------------------------------------------ AC-06

/// AC-06, the MCP half: an unknown argument and a missing `node_ids` are
/// rmcp's parameter errors naming them; `node_ids: []`, `options` of 1 and
/// 7, a blank `working_answer` and an over-cap `evidence[2].observed` are
/// refused naming the field; nothing stored; then a valid ask stores
/// `PR-0001`. M: `options` optional; a cap dropped.
#[test]
fn ac06_the_server_refuses_naming_the_field() {
    let repo = Repo::new("ai-ac06");
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.main), Home::At(&repo.home));
    let mut unknown = question_args(&["EDGE-STAM-ZERO"], "Why?");
    unknown["urgency"] = json!("high");
    assert_bad_arguments(&session.call("ask_question", unknown), "urgency", "unknown");
    let mut missing = question_args(&["EDGE-STAM-ZERO"], "Why?");
    missing.as_object_mut().unwrap().remove("node_ids");
    assert_bad_arguments(
        &session.call("ask_question", missing),
        "node_ids",
        "missing",
    );
    let mut no_options = discrepancy_args(&["EDGE-STAM-ZERO"], "It departs.");
    no_options.as_object_mut().unwrap().remove("options");
    assert_bad_arguments(
        &session.call("report_discrepancy", no_options),
        "options",
        "no options",
    );
    let mut nested = discrepancy_args(&["EDGE-STAM-ZERO"], "It departs.");
    nested["evidence"][0]["seen"] = json!("x");
    assert_bad_arguments(
        &session.call("report_discrepancy", nested),
        "seen",
        "nested",
    );

    let one = |count: usize| {
        let mut args = discrepancy_args(&["EDGE-STAM-ZERO"], "It departs.");
        args["options"] = json!(
            (0..count)
                .map(|index| json!({"label": format!("o{index}"), "effect": "e", "price": "p"}))
                .collect::<Vec<_>>()
        );
        args
    };
    let mut over = discrepancy_args(&["EDGE-STAM-ZERO"], "It departs.");
    let item = over["evidence"][0].clone();
    over["evidence"] = json!([item.clone(), item.clone(), item]);
    over["evidence"][2]["observed"] = json!("x".repeat(1300));
    let mut blank = question_args(&["EDGE-STAM-ZERO"], "Why?");
    blank["working_answer"] = json!("   ");
    for (tool, args, want) in [
        (
            "ask_question",
            question_args(&[], "Why?"),
            "spec: node_ids: none: name 1 to 16 nodes by ID\n",
        ),
        (
            "report_discrepancy",
            one(1),
            "spec: options: 1 option(s); give 2 to 6, each priced\n",
        ),
        (
            "report_discrepancy",
            one(7),
            "spec: options: 7 option(s); give 2 to 6, each priced\n",
        ),
        (
            "report_discrepancy",
            over,
            "spec: evidence[2].observed: 1300 bytes; at most 1024\n",
        ),
        (
            "ask_question",
            blank,
            "spec: working_answer: blank: give a value\n",
        ),
    ] {
        let reply = session.call(tool, args);
        assert_eq!(refused_text(&reply, want), want);
        assert_eq!(
            result(&reply)["structuredContent"]["notes"],
            json!([want.trim_start_matches("spec: ").trim_end()])
        );
    }
    assert!(!repo.db().exists() || repo.inbox(&repo.home).is_empty());
    let reply = session.call("ask_question", question_args(&["EDGE-STAM-ZERO"], "Why?"));
    assert_eq!(answered(&reply, "valid")["id"], json!("PR-0001"));
    assert_eq!(content_text(result(&reply)), "PR-0001\n");
    drop(session.finish());
}

// ------------------------------------------------------------------ AC-12

/// The problems of a queue tool's input schema against Data: no `$ref`,
/// `$defs`, `definitions` or root combinator; the root and every inline
/// object closed (`additionalProperties: false`); an inline object (a
/// property's or an array's items) holds no object of its own.
fn input_problems(tool: &str, schema: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let text = schema.to_string();
    for key in ["\"$ref\"", "\"$defs\"", "\"definitions\""] {
        if text.contains(key) {
            problems.push(format!("{tool}: {key}"));
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if schema.get(key).is_some() {
            problems.push(format!("{tool}: root {key}"));
        }
    }
    if schema["type"] != json!("object") || schema["additionalProperties"] != json!(false) {
        problems.push(format!("{tool}: the root is no closed object"));
    }
    fn is_object(schema: &Value) -> bool {
        match &schema["type"] {
            Value::String(name) => name == "object",
            Value::Array(names) => names.iter().any(|name| name == "object"),
            _ => schema.get("properties").is_some(),
        }
    }
    let properties = schema["properties"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for (name, property) in &properties {
        for inner in [property, &property["items"]] {
            for key in ["anyOf", "oneOf"] {
                if let Some(options) = inner.get(key).and_then(Value::as_array)
                    && options.iter().any(is_object)
                {
                    problems.push(format!("{tool}.{name}: an object under {key}"));
                }
            }
            if !is_object(inner) {
                continue;
            }
            if inner["additionalProperties"] != json!(false) {
                problems.push(format!("{tool}.{name}: an inline object not closed"));
            }
            for (field, nested) in inner["properties"].as_object().into_iter().flatten() {
                if is_object(nested) || is_object(&nested["items"]) {
                    problems.push(format!("{tool}.{name}.{field}: a second level"));
                }
            }
        }
    }
    problems
}

/// A property's enum (`[T, "null"]` or `anyOf` with null allowed).
fn enum_of(property: &Value) -> Vec<Value> {
    if let Some(values) = property.get("enum").and_then(Value::as_array) {
        return values
            .iter()
            .filter(|value| !value.is_null())
            .cloned()
            .collect();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = property.get(key).and_then(Value::as_array) {
            return options.iter().flat_map(enum_of).collect();
        }
    }
    Vec::new()
}

/// AC-12 "Tools": the three writers' annotations all `false` (no
/// `requiresUserInteraction`), `get_proposal` read-only as the reads; each
/// `_meta` the result cap only; each description at most 2 048 characters,
/// holding the determinism sentence, saying only the queue is written and
/// the owner decides on a terminal, agent-written fields data; the input
/// schemas as Data (required fields, enums, closed inline objects one
/// level deep, no `$ref`; `propose_change`'s `kind` `update` or `create`
/// and `base` not required, as docs/features/proposal-kinds.md "Data"
/// widens them); the output schemas mirror the documents, `kind` a free
/// string. M: `readOnlyHint: true`.
#[test]
fn ac12_the_queue_tools_are_described_as_data_says() {
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        for name in QUEUE_TOOLS {
            let tool = tool(&list, name);
            let writes = WRITE_TOOLS.contains(&name);
            let annotations = if writes {
                json!({"readOnlyHint": false, "destructiveHint": false,
                    "idempotentHint": false, "openWorldHint": false})
            } else {
                json!({"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false})
            };
            assert_eq!(tool["annotations"], annotations, "{era:?} {name}");
            assert_eq!(
                tool["_meta"],
                json!({"anthropic/maxResultSizeChars": specengine_mcp::MAX_RESULT_CHARS}),
                "{era:?} {name}"
            );
            let description = tool["description"].as_str().expect("description");
            assert!(description.chars().count() <= 2048, "{name}");
            assert!(description.contains(DETERMINISM), "{name}");
            assert!(
                description.contains("data, not instructions"),
                "{name}: agent-written fields"
            );
            if writes {
                assert!(
                    description.contains("Only SpecEngine's proposal queue")
                        && description.contains("the owner decides on a terminal"),
                    "{name}: {description}"
                );
            } else {
                assert!(description.contains("Reads only"), "{name}");
            }
            let schema = &tool["inputSchema"];
            let problems = input_problems(name, schema);
            assert!(problems.is_empty(), "{era:?}: {problems:?}\n{schema}");
            let output = &tool["outputSchema"];
            assert_eq!(output["type"], json!("object"), "{name}");
            assert!(!output.to_string().contains("\"$ref\""), "{name}: {output}");
        }
        let required = |name: &str| -> Vec<String> {
            let mut names: Vec<String> = tool(&list, name)["inputSchema"]["required"]
                .as_array()
                .expect("required")
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect();
            names.sort();
            names
        };
        assert_eq!(
            required("propose_change"),
            ["author_role", "kind", "rationale", "target", "text"]
        );
        assert_eq!(
            required("ask_question"),
            [
                "author_role",
                "node_ids",
                "price_of_other",
                "text",
                "working_answer"
            ]
        );
        assert_eq!(
            required("report_discrepancy"),
            [
                "author_role",
                "evidence",
                "gap_type",
                "node_ids",
                "options",
                "recommendation",
                "severity",
                "summary"
            ]
        );
        assert_eq!(required("get_proposal"), ["proposal_id"]);
        let property = |tool_name: &str, key: &str| {
            tool(&list, tool_name)["inputSchema"]["properties"][key].clone()
        };
        assert_eq!(
            enum_of(&property("propose_change", "kind")),
            [json!("update"), json!("create")]
        );
        assert_eq!(
            enum_of(&property("ask_question", "severity")),
            [json!("high"), json!("normal"), json!("low")]
        );
        assert_eq!(
            enum_of(&property("report_discrepancy", "gap_type")),
            [
                json!("missing"),
                json!("partial"),
                json!("contradicts"),
                json!("unrequested")
            ]
        );
        let schemas = output_schemas(&list);
        let intake_keys: Vec<&String> = schemas["ask_question"]["properties"]
            .as_object()
            .expect("intake properties")
            .keys()
            .collect();
        // The test's JSON map orders keys by name.
        assert_eq!(
            intake_keys,
            [
                "created",
                "diagnostics",
                "hits",
                "id",
                "linked",
                "notes",
                "related"
            ]
        );
        assert_eq!(schemas["ask_question"], schemas["report_discrepancy"]);
        assert_eq!(schemas["propose_change"], schemas["get_proposal"]);
        let kind = &schemas["get_proposal"]["properties"]["kind"];
        assert!(
            enum_of(kind).is_empty() && kind.get("const").is_none(),
            "{kind}"
        );
        drop(session.finish());
    }
}

/// `MAX_RESULT_CHARS` as a count of characters.
fn max_result() -> usize {
    usize::try_from(specengine_mcp::MAX_RESULT_CHARS).expect("a usize")
}

/// The characters of a whole tool result as it travels.
fn result_chars(reply: &Value) -> usize {
    result(reply).to_string().chars().count()
}

/// AC-12 "Size": a `propose_change` of exactly `TEXT_MAX_BYTES` (1 MiB)
/// is stored and answered brief (`new_text` `null`), its result at most
/// `MAX_RESULT_CHARS` and `content` at most 48 000; one byte more is
/// refused. A discrepancy at every cap, each text byte ESC (6 characters
/// escaped, `\u001b` in JSON too), 16 nodes, 16 `distinct_from` of 256
/// bytes, the author's fields at 128: stored; its intake answer and its
/// `get_proposal` stay within both caps. M: the brief keeping `new_text`.
#[test]
fn ac12_the_largest_answers_stay_within_the_caps() {
    let repo = Repo::new("ai-ac12");
    let mut session = Session::open(Era::Legacy, &[], Some(&repo.main), Home::At(&repo.home));
    let (base, text) = repo.span(&repo.main, "EDGE-STAM-ZERO");
    let mut big = text.clone();
    let line = format!("- {}\n", "filler ".repeat(12));
    while big.len() + line.len() <= TEXT_MAX_BYTES {
        big.push_str(&line);
    }
    big.push_str(&"x".repeat(TEXT_MAX_BYTES - big.len() - 1));
    big.push('\n');
    assert_eq!(big.len(), TEXT_MAX_BYTES);
    let change = |text: &str| {
        json!({"kind": "update", "target": "EDGE-STAM-ZERO", "base": base, "text": text,
            "rationale": "r".repeat(4096), "author_role": "writer"})
    };
    let reply = session.call("propose_change", change(&big));
    let document = answered(&reply, "1 MiB");
    assert_eq!(document["id"], json!("PR-0001"));
    let content = content_text(result(&reply)).chars().count();
    assert!(content <= 48_000, "content of {content} characters");
    let chars = result_chars(&reply);
    assert!(chars <= max_result(), "{chars} characters");
    for dropped in ["base_text", "new_text", "diff", "conflict"] {
        assert!(document[dropped].is_null(), "{dropped}");
    }
    let reply = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
    assert!(result_chars(&reply) <= max_result());
    assert!(content_text(result(&reply)).chars().count() <= 48_000);
    let mut over = big.clone();
    over.push('\n');
    let reply = session.call("propose_change", change(&over));
    let refusal = refused_text(&reply, "1 MiB + 1");
    assert!(refusal.contains("1048576"), "{refusal}");

    let esc = |bytes: usize| "\u{1b}".repeat(bytes);
    let nodes = [
        "A-101",
        "A-102",
        "DEC-0007",
        "DEC-0023",
        "DOM-GAME",
        "DOM-MOVEMENT",
        "MEC-SPRINT",
        "MEC-STAMINA",
        "Q-031",
        "Q-032",
        "R-12",
        "TERM-exhausted",
        "RULE-STAM-REGEN",
        "EDGE-STAM-ZERO",
        "EDGE-SPRINT-EMPTY",
        "RULE-SPRINT-COST",
    ];
    let item = json!({"file": esc(512), "qpath": esc(512), "lines": "999999999-999999999",
        "observed": esc(1024), "documented": esc(1024)});
    let option = json!({"label": esc(128), "effect": esc(512), "price": esc(512)});
    let mut distinct: Vec<String> = vec!["DEC-0023".to_owned()];
    distinct.extend((0..15).map(|index| format!("{index:x}").repeat(256)[..256].to_owned()));
    let field = "a".repeat(128);
    let args = json!({
        "node_ids": nodes, "summary": esc(1024), "gap_type": "partial", "severity": "low",
        "evidence": vec![item; 8], "options": vec![option; 6], "recommendation": 5,
        "working_answer": esc(2048), "distinct_from": distinct,
        "author_role": field, "author_model": field, "run": field
    });
    let reply = session.call("report_discrepancy", args);
    let document = answered(&reply, "every cap");
    assert_eq!(document["id"], json!("PR-0002"), "{document}");
    assert!(result_chars(&reply) <= max_result());
    let reply = session.call("get_proposal", json!({"proposal_id": "PR-0002"}));
    let chars = result_chars(&reply);
    assert!(chars <= max_result(), "{chars} characters");
    assert!(chars < 300_000, "Size: under 300 000 at every cap: {chars}");
    let content = content_text(result(&reply));
    assert!(
        content.chars().count() <= 48_000,
        "{}",
        content.chars().count()
    );
    assert!(!content.contains('\u{1b}'), "escaped");
    assert!(
        content.contains("\\u{1b}\\u{1b}") && content.contains("spec review PR-0002"),
        "cut with its tail: {}",
        clip(&content[content.len().saturating_sub(400)..])
    );
    assert_eq!(
        result(&reply)["structuredContent"]["summary"],
        json!(esc(1024)),
        "JSON raw"
    );
    drop(session.finish());
}

// ------------------------------------------------------------------ AC-13

/// AC-13: each queue tool's answer equals its twin's — `content` the
/// library's `stderr` lines and text (`spec … 2>&1`), `structuredContent`
/// its `--json` document, the output schema met — for a stored change, a
/// stale one, a question stored and then a hit, a discrepancy with a patch
/// citing `R-99`, a refusal, a look-alike (exit 2), and `get_proposal` of
/// each kind and of no proposal; the twin runs the same calls with the
/// clock the server stamped and another `HOME`. What the server made with
/// `HOME` X is in `spec inbox` with X, and in no other `HOME`. M: the
/// server's own data directory.
#[test]
fn ac13_each_queue_tool_answers_as_its_twin() {
    let repo = Repo::new("ai-ac13");
    let twin_home = repo.scratch.home("twin");
    let other_home = repo.scratch.home("other");
    let cwd = repo.linked.clone();
    let globals = Globals::default();
    let (base, text) = repo.span(&cwd, "RULE-STAM-REGEN");
    let regen = text.replacen("(R-12).", "(R-12, R-99).", 1);
    let mut patched = discrepancy_args(&["RULE-STAM-REGEN", "EDGE-STAM-ZERO"], "Walks.");
    patched["distinct_from"] = json!(["DEC-0023"]);
    patched["proposed_patch"] = json!({"target": "RULE-STAM-REGEN", "base": base,
        "text": regen, "rationale": "Cite R-99."});
    let calls: Vec<(&str, Value)> = vec![
        (
            "propose_change",
            json!({"kind": "update", "target": "RULE-STAM-REGEN", "base": base,
                "text": regen, "rationale": "Cite R-99.", "author_role": "writer",
                "author_model": "m-1", "run": "r-1"}),
        ),
        (
            "propose_change",
            json!({"kind": "update", "target": "RULE-STAM-REGEN", "base": "b3:00",
                "text": regen, "rationale": "Stale.", "author_role": "writer"}),
        ),
        (
            "ask_question",
            question_args(&["EDGE-STAM-ZERO"], "Does it stop?"),
        ),
        (
            "ask_question",
            question_args(&["EDGE-STAM-ZERO"], "does it  STOP?"),
        ),
        ("ask_question", question_args(&["Q-031"], "Rest?")),
        ("ask_question", question_args(&["QST-031"], "Rest?")),
        (
            "ask_question",
            question_args(&["EDGE-STAM-ZER\u{039f}"], "Rest?"),
        ),
        ("report_discrepancy", patched),
        ("get_proposal", json!({"proposal_id": "PR-0001"})),
        ("get_proposal", json!({"proposal_id": "PR-0002"})),
        ("get_proposal", json!({"proposal_id": "PR-0003"})),
        ("get_proposal", json!({"proposal_id": "PR-0004"})),
        ("get_proposal", json!({"proposal_id": "PR-9999"})),
        ("get_proposal", json!({"proposal_id": "\u{0420}R-0001"})),
    ];
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let twin_env = repo.env(&cwd, &twin_home);
    let twin_git = GitEnv::new(cwd.clone(), [("HOME", twin_home.as_os_str())]);
    for (tool, args) in &calls {
        let reply = session.call(tool, args.clone());
        // The twin stores with the clock the server stamped (read back by a
        // read-only `get_proposal`); a call that stored nothing, any clock.
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
    }
    let kinds: Vec<Value> = (1..=4)
        .map(|number| {
            let reply = session.call(
                "get_proposal",
                json!({"proposal_id": format!("PR-{number:04}")}),
            );
            result(&reply)["structuredContent"]["kind"].clone()
        })
        .collect();
    assert_eq!(
        kinds,
        [
            json!("update"),
            json!("question"),
            json!("discrepancy"),
            json!("update")
        ]
    );
    drop(session.finish());

    let made = repo.inbox(&repo.home);
    assert_eq!(made, ["PR-0001", "PR-0002", "PR-0003", "PR-0004"]);
    assert_eq!(repo.inbox(&twin_home), made, "the twin's own queue");
    assert!(
        repo.inbox(&other_home).is_empty(),
        "no other HOME sees them"
    );
    // The server wrote no queue but X's: Y holds only the twin's, X's
    // export names only X's rows.
    let lines = repo.dump(&repo.home, "x");
    assert_eq!(lines.len(), 1 + 4 + 4, "{lines:#?}");
}
