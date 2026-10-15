//! docs/features/kind-amendment.md over MCP, the real binary over stdio
//! (AC-T1 over AC-R10's `get_proposal` and AC-R11):
//!
//! - `propose_change {kind: "amendment", target: PATH, base: null, text,
//!   rationale, author_role}` answers as `spec propose amendment PATH
//!   --text-file - --rationale R --author-role ROLE --brief` (`content` its
//!   `2>&1`, `structuredContent` its `--json`, the library twin in another
//!   `HOME` with the clock the server stamped); `base` absent alike; a
//!   refusal (an unknown amended ID, an ID target) is the twin's error
//!   result; `get_proposal` is `spec review --brief`'s twin, the review's
//!   48 keys (drift-queue's `drift` the 48th), `kind` `amendment`.
//! - The form: a `base` string is -32602 `base: an amendment is a new
//!   file: null or absent`; `decision`, `how`, `why` -32602 `<field>: only
//!   an interpretation takes it`; nothing stored.
//! - The schema: `kind` the enum of four from core's constants; the
//!   description and the instructions name `amendment`, each within 2 048
//!   bytes.
//!
//! After every call both worktrees' `git status --porcelain` is empty and
//! their files and refs are as before (the single door).
//!
//! M: a kind literal in `specengine-mcp/src` (`mcp_genre.rs`); the base
//! rule dropped.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::read::{Era, Session, cli_env, content_text, expected, output_schemas};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{
    AMENDMENT_KIND, AmendmentRequest, CREATE_KIND, GitEnv, Globals, INTERPRETATION_KIND,
    InboxRequest, Outcome, ProposedText, ReviewRequest, UPDATE_KIND, inbox, review_brief,
};

/// "Data"'s path and text.
const PATH: &str = "docs/features/regen-delay.md";
const TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                    [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
                    {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint, not after any \
                    stamina use.\n";

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

    /// `git status --porcelain` of both worktrees, untracked files listed.
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

    /// The proposal IDs stored under `home` (`spec inbox --all`).
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
}

/// "Data"'s text with its `amends` list written `list` and its section ID
/// `section`.
fn amending(list: &str, section: &str) -> String {
    TEXT.replacen("amends: [RULE-STAM-REGEN]", &format!("amends: [{list}]"), 1)
        .replacen("{#AC-01}", &format!("{{#{section}}}"), 1)
}

/// The twin of a `propose_change` of kind `amendment`: `spec propose
/// amendment PATH --text-file - --rationale R --author-role ROLE --brief`
/// through the library, with `HOME` `home` and the clock `now`.
fn amend_twin(args: &Value, cwd: &Path, home: &Path, now: &str) -> common::read::Expected {
    let text = |key: &str| args[key].as_str().map(str::to_owned);
    let outcome = specengine_cli::propose_amendment_brief(
        &cli_env(cwd, Some(home)),
        &Globals::default(),
        &AmendmentRequest {
            target: text("target").expect("a target"),
            text: ProposedText::Given(text("text").expect("a text").into_bytes()),
            rationale: text("rationale").expect("a rationale"),
            author_role: text("author_role"),
            author_model: text("author_model"),
            run: text("run"),
            now: now.to_owned(),
            git: GitEnv::new(cwd, [("HOME", home.as_os_str())]),
        },
        text("task_id").as_deref(),
    )
    .map(|outcome| Outcome::Proposal(Box::new(outcome)));
    expected(outcome)
}

/// `content`, `isError` and `structuredContent` of `reply` equal the
/// twin's.
fn assert_twin(reply: &Value, want: &common::read::Expected, context: &str) {
    let result = result(reply);
    assert_eq!(content_text(result), want.text, "{context}: content");
    assert_eq!(
        result["isError"],
        json!(want.is_error),
        "{context}: isError"
    );
    match &want.document {
        Some(document) => assert_eq!(
            &result["structuredContent"], document,
            "{context}: structuredContent"
        ),
        None => assert!(result.get("structuredContent").is_none(), "{context}"),
    }
}

/// An agent's `propose_change` of kind `amendment`.
fn writer(target: &str, text: &str) -> Value {
    json!({"kind": AMENDMENT_KIND, "target": target, "text": text,
        "rationale": "Regeneration waits after a sprint.", "author_role": "spec-author",
        "author_model": "claude-opus-5-5"})
}

/// AC-R11: `amendment` with `base` null, and absent, is stored as its
/// twin's (`kind` `amendment`, `target_ids` the file and its own new ID,
/// the brief document); a refusal (an unknown amended ID, an ID target) is
/// the twin's error result; `get_proposal` is `spec review --brief`'s twin
/// (47 keys); the worktrees untouched after every call. M: the base rule
/// dropped.
#[test]
fn ac_r11_propose_change_amends_as_its_twin() {
    let repo = Repo::new("ma-twin");
    let twin_home = repo.scratch.home("twin");
    let cwd = repo.linked.clone();
    let mut null_base = writer(PATH, TEXT);
    null_base["base"] = Value::Null;
    let absent = writer(
        "docs/features/zero-stop.md",
        &amending("EDGE-STAM-ZERO", "AC-02"),
    );
    let unknown = writer("docs/features/nope.md", &amending("RULE-NOPE", "AC-03"));
    let mut on_node = writer("RULE-STAM-REGEN", TEXT);
    on_node["base"] = Value::Null;
    let before_files = repo.files();
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let schema = &schemas["propose_change"];
    let review_schema = &schemas["get_proposal"];
    let mut stored = Vec::new();
    for (label, args, created, targets) in [
        (
            "base null",
            &null_base,
            Some("PR-0001"),
            json!([PATH, "regen-delay/AC-01"]),
        ),
        (
            "base absent",
            &absent,
            Some("PR-0002"),
            json!(["docs/features/zero-stop.md", "zero-stop/AC-02"]),
        ),
        ("an unknown amended ID", &unknown, None, Value::Null),
        ("an ID target", &on_node, None, Value::Null),
    ] {
        let reply = session.call("propose_change", args.clone());
        let structured = &result(&reply)["structuredContent"];
        let now = match created {
            Some(id) => {
                assert_eq!(structured["id"], json!(id), "{label}: {reply}");
                assert_eq!(structured["kind"], json!(AMENDMENT_KIND), "{label}");
                assert_eq!(structured["target_ids"], targets, "{label}");
                assert_eq!(structured["base_hash"], Value::Null, "{label}");
                assert_eq!(structured["new_text"], Value::Null, "{label}: brief");
                let read = session.call("get_proposal", json!({"proposal_id": id}));
                let review = result(&read);
                let twin = expected(
                    review_brief(
                        &cli_env(&cwd, Some(&repo.home)),
                        &Globals::default(),
                        &ReviewRequest {
                            id: id.to_owned(),
                            git: GitEnv::new(&cwd, [("HOME", repo.home.as_os_str())]),
                        },
                    )
                    .map(|outcome| Outcome::Proposal(Box::new(outcome))),
                );
                assert_twin(&read, &twin, &format!("{label}: get_proposal"));
                let document = &review["structuredContent"];
                assert_eq!(
                    document.as_object().map(serde_json::Map::len),
                    Some(48),
                    "{label}: the review's 48 keys (docs/features/rev-discipline.md, \
                     docs/features/drift-queue.md)"
                );
                assert_eq!(document["kind"], json!(AMENDMENT_KIND));
                assert_eq!(document["preview"], json!("applies"));
                if let Err(problem) = common::read::conforms(review_schema, document, "$") {
                    panic!("{label}: get_proposal breaks its outputSchema: {problem}");
                }
                stored.push(id.to_owned());
                document["created_at"]
                    .as_str()
                    .expect("created_at")
                    .to_owned()
            }
            None => "2026-10-08T12:00:00Z".to_owned(),
        };
        let want = amend_twin(args, &cwd, &twin_home, &now);
        assert_twin(&reply, &want, label);
        if let Some(document) = &want.document
            && let Err(problem) = common::read::conforms(schema, document, "$")
        {
            panic!("{label}: the document breaks the outputSchema: {problem}");
        }
        assert_eq!(repo.statuses(), (String::new(), String::new()), "{label}");
        assert_eq!(repo.files(), before_files, "{label}: nothing written");
        assert_eq!(repo.stored(&repo.home), stored, "{label}");
    }
    let refused = session.call("propose_change", unknown.clone());
    assert!(
        content_text(result(&refused))
            .contains("amends `RULE-NOPE`: no node RULE-NOPE; nothing stored"),
        "{refused}"
    );
    assert_eq!(result(&refused)["isError"], json!(true));
    drop(session.finish());
    assert_eq!(repo.stored(&repo.home), ["PR-0001", "PR-0002"]);
}

/// AC-R11, the form: with kind `amendment` a `base` string is -32602
/// `base: an amendment is a new file: null or absent`; `decision`, `how`,
/// `why` each -32602 `<field>: only an interpretation takes it`; nothing
/// stored, nothing written. M: the base rule dropped.
#[test]
fn ac_r11_a_base_or_an_interpretation_field_is_invalid_params() {
    let repo = Repo::new("ma-form");
    let cwd = repo.linked.clone();
    let before_files = repo.files();
    let mut session = Session::open(Era::Legacy, &[], Some(&cwd), Home::At(&repo.home));
    let mut based = writer(PATH, TEXT);
    based["base"] = json!("b3:00");
    let reply = session.call("propose_change", based);
    assert_eq!(error_code(&reply), INVALID_PARAMS, "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("base: an amendment is a new file: null or absent")
    );
    for field in ["decision", "how", "why"] {
        let mut args = writer(PATH, TEXT);
        args[field] = json!("x");
        let reply = session.call("propose_change", args);
        assert_eq!(error_code(&reply), INVALID_PARAMS, "{field}: {reply}");
        assert_eq!(
            reply["error"]["message"],
            json!(format!("{field}: only an interpretation takes it")),
            "{field}"
        );
        // Explicit nulls are absent.
        let mut args = writer(PATH, TEXT);
        args[field] = Value::Null;
        args["target"] = json!(format!("docs/features/null-{field}.md"));
        args["text"] = json!(TEXT.replacen("{#AC-01}", "{#AC-09}", 1));
        let reply = session.call("propose_change", args);
        assert_eq!(
            result(&reply)["structuredContent"]["kind"],
            json!(AMENDMENT_KIND),
            "{field} null: {reply}"
        );
    }
    drop(session.finish());
    assert_eq!(repo.statuses(), (String::new(), String::new()));
    assert_eq!(repo.files(), before_files);
    assert_eq!(repo.stored(&repo.home), ["PR-0001", "PR-0002", "PR-0003"]);
}

/// AC-R11, the schema and the texts: `kind` is the enum of four from
/// core's constants, in order; `base` not required; `propose_change`'s
/// description names `amendment` and `links: {amends: [ID]}`, within 2 048
/// bytes; `INSTRUCTIONS` name it in the `propose_change` line, within
/// 2 048 bytes (the `probes` build: `mcp_create.rs`).
#[test]
fn ac_r11_the_enum_of_four_and_the_texts_name_amendment() {
    assert_eq!(
        [
            UPDATE_KIND,
            CREATE_KIND,
            INTERPRETATION_KIND,
            AMENDMENT_KIND
        ],
        ["update", "create", "interpretation", "amendment"]
    );
    for era in [Era::Legacy, Era::Stateless] {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        let change = tool(&list, "propose_change");
        let schema = &change["inputSchema"];
        assert_eq!(
            schema["properties"]["kind"]["enum"],
            json!([
                UPDATE_KIND,
                CREATE_KIND,
                INTERPRETATION_KIND,
                AMENDMENT_KIND
            ]),
            "{era:?}"
        );
        let required: Vec<&str> = schema["required"]
            .as_array()
            .expect("required")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(!required.contains(&"base"), "{required:?}");
        let description = change["description"].as_str().expect("a description");
        assert!(description.len() <= 2048, "{} bytes", description.len());
        assert!(
            description.contains("amendment: a new spec or decision file (base null)"),
            "{description}"
        );
        assert!(description.contains("amends: [ID]"), "{description}");
        assert!(
            description.contains("update|create|interpretation|amendment"),
            "{description}"
        );
        drop(session.finish());
    }
    let instructions = specengine_mcp::INSTRUCTIONS;
    assert!(instructions.len() <= 2048, "{}", instructions.len());
    assert!(
        instructions
            .contains("- propose_change = spec propose update|create|interpretation|amendment:"),
        "{instructions}"
    );
}
