//! TE-7 of docs/features/drift-queue.md over MCP, the real binary over
//! stdio ("report_drift and texts", "Review, inbox, round, stage"; RD-4's
//! first half, RD-13):
//!
//! - `report_drift {node_ids?, task_id?, author_role, author_model?,
//!   run?}` answers as `spec propose drift [REF…] --author-role R --json`:
//!   `content` its `2>&1`, `structuredContent` its drift document, the
//!   library twin in another `HOME`; a second raise is a hit there too.
//! - Its input schema has no field for evidence: exactly those five
//!   properties, `author_role` required, closed; a call naming `rows`,
//!   `drift`, `drift_hash` or `evidence` is a bad argument, nothing stored;
//!   `node_ids: []` is -32602.
//! - `get_proposal` of a drift item is `spec review --brief`'s twin: the
//!   review's 48 keys, `drift` `{hash, rows, more}` conforming to the
//!   output schema (which names `drift`), `preview` `null` and the note
//!   naming `spec approve`.
//! - AC-3: after every call `git status --porcelain=v1
//!   --untracked-files=all`, the files, `HEAD` and every ref are as before.
//!
//! The repository: `fixtures/code-a` committed, its golden lock
//! (`fixtures/spec-lock/code-a.lock`) committed, `CAP` edited and
//! committed (`LAW-BOUND` drifts `code_ahead code_changed`).
//!
//! M: MCP takes `rows` (a field for evidence in `DriftArgs`); a write
//! under the root.

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use common::read::{
    ERAS, Era, Session, assert_bad_arguments, assert_parity, cli_env, expected, output_schemas,
    queue_library,
};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    GitEnv, Globals, Outcome, ProposeRequest, ProposedText, ReviewRequest, ShowRequest, propose,
    review_brief, show,
};

struct Repo {
    scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        Self::with_lock(label, "code-a.lock")
    }

    /// The repository with `fixtures/spec-lock/<lock>` as its lock (the
    /// format-2 golden, or `code-a.format-1.lock`: a project not yet
    /// rebased, docs/features/lock-levels.md).
    fn with_lock(label: &str, lock: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("code-a", "repo");
        let _ = fs::remove_file(root.join("expected.json"));
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        copy_file(fixture("spec-lock").join(lock), root.join("spec.lock"));
        git.add_all(&root);
        git.commit(&root, "the lock");
        replace(
            &root,
            "crates/tally/src/counter.rs",
            "CAP: u32 = 10;",
            "CAP: u32 = 12;",
        );
        git.add_all(&root);
        git.commit(&root, "the cap");
        let home = scratch.home("h");
        Self {
            scratch,
            git,
            root,
            home,
        }
    }

    /// AC-3's state: `git status` (untracked files listed), every file but
    /// `.git`'s, and every ref with `HEAD`.
    fn state(&self) -> (String, BTreeMap<String, Option<Vec<u8>>>, String) {
        let status = self.git.git_text(
            &self.root,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        );
        let files = snapshot(&self.root)
            .into_iter()
            .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
            .collect();
        let refs = self.git.git_text(
            &self.root,
            &["for-each-ref", "--format=%(refname) %(objectname)"],
        ) + &self.git.git_text(&self.root, &["rev-parse", "HEAD"]);
        (status, files, refs)
    }
}

/// RD-4 (MCP half), RD-13: the input schema names no evidence: exactly
/// `node_ids`, `task_id`, `author_role`, `author_model`, `run`, closed,
/// `author_role` required, `node_ids` an array of strings; the writers'
/// annotations; a call naming a field for evidence is a bad argument and
/// `node_ids: []` -32602, nothing stored. M: `DriftArgs` takes `rows`.
#[test]
fn report_drift_takes_no_evidence() {
    let repo = Repo::new("mcp-drift-schema");
    let before = repo.state();
    for era in ERAS {
        let mut session = Session::open(era, &[], Some(&repo.root), Home::At(&repo.home));
        let list = session.tools();
        let drift = tool(&list, "report_drift");
        let schema = &drift["inputSchema"];
        let mut properties: Vec<&str> = schema["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .map(String::as_str)
            .collect();
        properties.sort_unstable();
        assert_eq!(
            properties,
            ["author_model", "author_role", "node_ids", "run", "task_id"],
            "{era:?}: {schema}"
        );
        assert_eq!(schema["required"], json!(["author_role"]), "{era:?}");
        assert_eq!(schema["additionalProperties"], json!(false), "{era:?}");
        assert_eq!(
            schema["properties"]["node_ids"]["items"]["type"],
            json!("string"),
            "{era:?}: {schema}"
        );
        assert_eq!(
            drift["annotations"],
            json!({"readOnlyHint": false, "destructiveHint": false,
                "idempotentHint": false, "openWorldHint": false}),
            "{era:?}"
        );
        let output = &drift["outputSchema"]["properties"];
        let mut keys: Vec<&str> = output
            .as_object()
            .expect("output properties")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["branch", "commit", "created", "hits", "judged", "notes"],
            "{era:?}: the drift document"
        );
        for field in ["rows", "drift", "drift_hash", "evidence"] {
            let reply = session.call("report_drift", json!({"author_role": "tester", field: []}));
            assert_bad_arguments(&reply, field, &format!("{era:?} {field}"));
        }
        let reply = session.call(
            "report_drift",
            json!({"author_role": "tester", "node_ids": []}),
        );
        assert_eq!(error_code(&reply), -32602, "{era:?}: {reply}");
        assert!(
            reply["error"]["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("node_ids: ")),
            "{reply}"
        );
        // Nothing stored.
        let reply = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
        assert_eq!(result(&reply)["isError"], json!(true), "{era:?}: {reply}");
        let done = session.finish();
        assert!(done.status.success(), "{era:?}: {}", done.stderr);
        assert_eq!(repo.state(), before, "{era:?}: AC-3");
    }
}

/// TE-7: `report_drift` is `spec propose drift`'s twin (a sweep, then
/// `LAW-BOUND` named: a hit); `get_proposal` of the item `spec review
/// --brief`'s: 48 keys, `drift` as stored, `preview` `null` with the note;
/// every `structuredContent` conforms to its output schema; AC-3 after
/// every call. M: a write under the root; `drift` missing from the review.
#[test]
fn report_drift_answers_as_its_twin_and_writes_only_the_queue() {
    let repo = Repo::new("mcp-drift-twin");
    let twin_home = repo.scratch.home("twin");
    let globals = Globals::default();
    let twin_env = cli_env(&repo.root, Some(&twin_home));
    let twin_git = GitEnv::new(repo.root.clone(), [("HOME", twin_home.as_os_str())]);
    let before = repo.state();
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    assert!(
        schemas["get_proposal"]["properties"].get("drift").is_some(),
        "get_proposal's output schema names `drift`: {}",
        schemas["get_proposal"]
    );
    let calls = [
        json!({"author_role": "tester"}),
        json!({"node_ids": ["LAW-BOUND"], "author_role": "tester", "author_model": "m-1",
            "run": "r-1"}),
    ];
    let mut documents = Vec::new();
    for args in &calls {
        let reply = session.call("report_drift", args.clone());
        let document = result(&reply)["structuredContent"].clone();
        // The twin stores with the clock the server stamped.
        let stamped = document["created"]
            .as_array()
            .and_then(|created| created.first())
            .and_then(|first| first["id"].as_str())
            .map(|id| {
                let stored = session.call("get_proposal", json!({"proposal_id": id}));
                result(&stored)["structuredContent"]["created_at"]
                    .as_str()
                    .expect("created_at")
                    .to_owned()
            })
            .unwrap_or_else(|| "2026-10-14T09:00:00Z".to_owned());
        let want = queue_library(
            "report_drift",
            args,
            &twin_env,
            &globals,
            &stamped,
            &twin_git,
        );
        assert_parity(
            &reply,
            &want,
            &schemas["report_drift"],
            &format!("report_drift {args}"),
        );
        assert_eq!(repo.state(), before, "AC-3 after report_drift {args}");
        documents.push(document);
    }
    let created: Vec<(&str, &str)> = documents[0]["created"]
        .as_array()
        .expect("created")
        .iter()
        .map(|item| {
            (
                item["id"].as_str().expect("id"),
                item["node"].as_str().expect("node"),
            )
        })
        .collect();
    assert_eq!(
        created,
        [("PR-0001", "LAW-BOUND"), ("PR-0002", "PIECE-COUNTER")],
        "{}",
        documents[0]
    );
    assert_eq!(documents[1]["created"], json!([]));
    assert_eq!(
        documents[1]["hits"],
        json!([{"id": "PR-0001", "node": "LAW-BOUND", "status": "open", "same": true}])
    );

    // `get_proposal` of the drift item: the review's brief twin.
    let reply = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
    let want = expected(
        review_brief(
            &twin_env,
            &globals,
            &ReviewRequest {
                id: "PR-0001".to_owned(),
                git: twin_git.clone(),
            },
        )
        .map(|outcome| Outcome::Proposal(Box::new(outcome))),
    );
    assert_parity(
        &reply,
        &want,
        &schemas["get_proposal"],
        "get_proposal PR-0001",
    );
    let document = &result(&reply)["structuredContent"];
    assert_eq!(document.as_object().map(serde_json::Map::len), Some(48));
    assert_eq!(document["kind"], json!("drift"));
    assert_eq!(document["preview"], Value::Null);
    let drift = document["drift"].as_object().expect("a drift object");
    let mut keys: Vec<&str> = drift.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["hash", "more", "rows"]);
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .contains(&json!("spec approve PR-0001 shows the rows before asking")),
        "{document}"
    );
    assert_eq!(repo.state(), before, "AC-3 after get_proposal");
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    assert_eq!(done.stderr, "");
    assert_eq!(repo.state(), before, "AC-3 at the end");
}

/// A row as a later build could store it: keys out of verify's and out of
/// alphabetical order, 5b's `levels` and 5c's `anchor` (a nested object)
/// after `compared`, integers (`line`, `rev`, `node_rev`, a negative one)
/// as written.
const LATER_ROW: &str = "{\"state\":\"code_ahead\",\"node\":\"LAW-BOUND\",\
    \"relation\":\"configures\",\"file\":\"crates/tally/src/counter.rs\",\
    \"symbol\":\"crates/tally::counter::CAP\",\"kind\":\"item\",\"item_kind\":\"const_item\",\
    \"category\":\"code_changed\",\"detail\":null,\"class\":\"check\",\"pre_existing\":true,\
    \"markers\":[{\"rev\":2,\"line\":8}],\"node_rev\":2,\"lock\":{\"verified_at\":\"2026-10-08\",\
    \"rev\":2,\"commit\":\"8888888888888888888888888888888888888888\"},\"compared\":\"ast\",\
    \"levels\":[\"sig\",\"body\"],\"anchor\":{\"to\":{\"line\":12,\"rev\":3},\"from\":null,\
    \"n\":-1}}";

/// The same row with every nullable key `null` (valid: a known key `null`).
const NULL_ROW: &str = "{\"node\":\"LAW-BOUND\",\"relation\":\"implements\",\
    \"file\":\"src/a.rs\",\"kind\":null,\"symbol\":null,\"item_kind\":null,\
    \"state\":\"broken_binding\",\"category\":\"file_gone\",\"detail\":null,\"class\":null,\
    \"pre_existing\":false,\"markers\":[],\"node_rev\":null,\"lock\":null,\"compared\":null}";

/// `repo`'s `PR-0001` raised by `report_drift`, its `drift` column then
/// rewritten to [`LATER_ROW`] and [`NULL_ROW`] through the system's
/// `sqlite3`; the session, and the column.
fn later_build_item(repo: &Repo) -> (Session, String) {
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    let reply = session.call("report_drift", json!({"author_role": "tester"}));
    assert_eq!(
        result(&reply)["structuredContent"]["created"][0]["id"],
        json!("PR-0001"),
        "{reply}"
    );
    let column = format!("{{\"rows\":[{LATER_ROW},{NULL_ROW}],\"more\":4}}");
    let db = data_dir(&repo.home).join("code-a.db");
    let output = std::process::Command::new("/usr/bin/sqlite3")
        .env_clear()
        .args(["-cmd", ".timeout 2000"])
        .arg(&db)
        .arg(format!(
            "UPDATE proposals SET drift = '{column}' WHERE id = 'PR-0001';"
        ))
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (session, column)
}

/// RD-12 over MCP ("Review, inbox, round, stage": `drift` as stored; Q-A):
/// `get_proposal` of a drift item whose stored rows carry later keys (a
/// nested object after `compared`) and a known key `null` gives each row
/// whole: the later keys kept, every value and integer as stored, `more`
/// as stored. The key order is not kept here: the server reads the
/// `--json` document into a `serde_json::Value`, which sorts keys (the
/// spec's accepted deviation 6, "MCP still sorts"); `spec review --json`
/// keeps it (`drift_approve.rs` `review_gives_the_rows_as_stored`). M:
/// `StoredJson` dropping a key; the review's rows taken from the view
/// (`DriftRow`).
#[test]
fn get_proposal_keeps_a_later_builds_rows() {
    let repo = Repo::new("mcp-drift-later");
    let (mut session, _) = later_build_item(&repo);
    let reply = session.call("get_proposal", json!({"proposal_id": "PR-0001"}));
    let drift = &result(&reply)["structuredContent"]["drift"];
    let stored: Vec<Value> = [LATER_ROW, NULL_ROW]
        .iter()
        .map(|row| serde_json::from_str(row).expect("a row"))
        .collect();
    assert_eq!(drift["rows"], json!(stored), "{drift}");
    assert_eq!(drift["more"], json!(4));
    assert_eq!(drift["rows"][0]["anchor"]["to"]["line"], json!(12));
    assert_eq!(drift["rows"][0]["anchor"]["n"], json!(-1));
    assert_eq!(drift["rows"][0]["levels"], json!(["sig", "body"]));
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}

// ------------------------------------------------ lock-levels.md RD-2, TE-1

/// The one text every writer and code judge refuses a format-1 lock with
/// (docs/features/lock-levels.md "Readers", Q27 (3)).
const FORMAT_1: &str = "spec.lock:2: format 1 (\"specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15\"), \
    this build's is 2 (\"specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15\"): run spec lock rebase \
    on a terminal";

/// lock-levels.md RD-2 **M4**, MCP's case: `report_drift` reads `HEAD`'s
/// lock with the build's recipes (`read_commit`), so over the format-1
/// golden it is `spec propose drift`'s exit 2: `isError`, the content
/// naming the one text with its step, no document, nothing stored in
/// either queue, nothing written (AC-3). M: the step missing from one
/// caller.
#[test]
fn report_drift_refuses_a_format_1_lock() {
    let repo = Repo::with_lock("mcp-drift-format-1", "code-a.format-1.lock");
    let twin_home = repo.scratch.home("twin");
    let globals = Globals::default();
    let twin_env = cli_env(&repo.root, Some(&twin_home));
    let twin_git = GitEnv::new(repo.root.clone(), [("HOME", twin_home.as_os_str())]);
    let before = repo.state();
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let args = json!({"author_role": "tester"});
    let reply = session.call("report_drift", args.clone());
    let want = queue_library(
        "report_drift",
        &args,
        &twin_env,
        &globals,
        "2026-10-14T09:00:00Z",
        &twin_git,
    );
    assert_parity(
        &reply,
        &want,
        &schemas["report_drift"],
        "report_drift over format 1",
    );
    let result = result(&reply);
    assert_eq!(result["isError"], json!(true), "{reply}");
    let text = common::read::content_text(result);
    assert!(text.contains(FORMAT_1), "{text}");
    assert_eq!(repo.state(), before, "AC-3");
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
    for home in [&repo.home, &twin_home] {
        let db = data_dir(home).join("code-a.db");
        if db.exists() {
            let output = std::process::Command::new("/usr/bin/sqlite3")
                .env_clear()
                .args(["-cmd", ".timeout 2000"])
                .arg(&db)
                .arg("SELECT count(*) FROM proposals;")
                .output()
                .expect("sqlite3 runs");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "0",
                "nothing stored"
            );
        }
    }
}

/// lock-levels.md TE-1 (Q10): `get_proposal` is a reader of the spec side
/// alone: over the format-1 golden an update of `PIECE-COUNTER` (raised
/// through the library, never through a lock) gives `spec review
/// --brief`'s twin, its revision row counting the four entries format 1
/// holds; nothing written.
#[test]
fn get_proposal_counts_format_1_entries() {
    let repo = Repo::with_lock("mcp-drift-format-1-review", "code-a.format-1.lock");
    let env = cli_env(&repo.root, Some(&repo.home));
    let globals = Globals::default();
    let git = GitEnv::new(repo.root.clone(), [("HOME", repo.home.as_os_str())]);
    let shown = show(
        &env,
        &globals,
        &ShowRequest {
            reference: "PIECE-COUNTER".to_owned(),
            links: false,
            archive: false,
            bindings: false,
            git: git.clone(),
        },
    )
    .expect("show PIECE-COUNTER");
    let node = shown.nodes.into_iter().next().expect("one node");
    let text = node.text.replacen(
        "grows one step at a time",
        "grows a single step at a time",
        1,
    );
    assert_ne!(text, node.text, "the edit applies");
    let proposed = propose(
        &env,
        &globals,
        &ProposeRequest {
            target: "PIECE-COUNTER".to_owned(),
            base: node.span_hash.clone(),
            text: ProposedText::Given(text.into_bytes()),
            rationale: "Update PIECE-COUNTER.".to_owned(),
            author_role: Some("spec-writer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: "2026-10-14T09:00:00Z".to_owned(),
            git: git.clone(),
        },
    )
    .expect("proposed");
    let id = proposed.document.id.clone().expect("an ID");
    let before = repo.state();
    let mut session = Session::open(Era::Stateless, &[], Some(&repo.root), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let reply = session.call("get_proposal", json!({"proposal_id": id}));
    let want = expected(
        review_brief(
            &env,
            &globals,
            &ReviewRequest {
                id: id.clone(),
                git: git.clone(),
            },
        )
        .map(|outcome| Outcome::Proposal(Box::new(outcome))),
    );
    assert_parity(
        &reply,
        &want,
        &schemas["get_proposal"],
        "get_proposal over format 1",
    );
    let document = &result(&reply)["structuredContent"];
    assert_eq!(
        document["revisions"],
        json!([{"id": "PIECE-COUNTER", "rev": 3, "to": 4, "lock_entries": 4, "refused": null}]),
        "{document}"
    );
    assert_eq!(repo.state(), before, "AC-3");
    let done = session.finish();
    assert!(done.status.success(), "{}", done.stderr);
}
