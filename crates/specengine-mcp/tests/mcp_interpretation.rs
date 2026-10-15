//! docs/features/kind-interpretation.md over MCP, the real binary over
//! stdio (AC-R11, AC-T3, "MCP"):
//!
//! - `tools/list`: `propose_change`'s `kind` enum is core's
//!   `UPDATE_KIND`, `CREATE_KIND`, `INTERPRETATION_KIND` in that order;
//!   `decision`, `how`, `why` nullable strings, not required.
//! - An interpretation without `decision`, `how`, `why` or `base`: a
//!   -32602 error naming the field; one of the three on an update or a
//!   create: -32602 `<field>: only an interpretation takes it`; `null`
//!   on an update is no field given. Nothing stored by any.
//! - A valid call answers as its twin `spec propose interpretation …
//!   --brief` (the library in another `HOME`, the server's clock): content
//!   and `structuredContent`, `interpretation` "Data"'s object; a
//!   superseded decision is an error result naming the successor, nothing
//!   stored; `get_proposal` is `spec review PR --brief --json`, the key
//!   kept.
//! - AC-T3: after every call both worktrees' files are as committed and
//!   `git status --porcelain` empty. M: a file write in propose.
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
    AMENDMENT_KIND, CREATE_KIND, GitEnv, Globals, INTERPRETATION_KIND, InboxRequest,
    InterpretRequest, Outcome, ProposedText, ReviewRequest, ShowRequest, UPDATE_KIND, inbox, show,
};

const HOW: &str = "Rest is any speed under 40 % of walking.";
const WHY: &str = "DEC-0023 says rest; the rule names no speed.";

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

    fn statuses(&self) -> (String, String) {
        let status = |dir: &Path| {
            self.git
                .git_text(dir, &["status", "--porcelain=v1", "--untracked-files=all"])
        };
        (status(&self.main), status(&self.linked))
    }

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

/// The twin of an interpretation call: `spec propose interpretation …
/// --brief` through the library, `HOME` `home`, the clock `now`.
fn interpret_twin(args: &Value, cwd: &Path, home: &Path, now: &str) -> common::read::Expected {
    let text = |key: &str| args[key].as_str().map(str::to_owned);
    let outcome = specengine_cli::propose_interpretation_brief(
        &cli_env(cwd, Some(home)),
        &Globals::default(),
        &InterpretRequest {
            target: text("target").expect("a target"),
            base: text("base").expect("a base"),
            decision: text("decision").expect("a decision"),
            how: text("how").expect("how"),
            why: text("why").expect("why"),
            text: ProposedText::Given(text("text").expect("a text").into_bytes()),
            rationale: text("rationale").expect("a rationale"),
            author_role: text("author_role"),
            author_model: text("author_model"),
            run: text("run"),
            now: now.to_owned(),
            git: GitEnv::new(cwd, [("HOME", home.as_os_str())]),
        },
        None,
    )
    .map(|outcome| Outcome::Proposal(Box::new(outcome)));
    expected(outcome)
}

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

/// AC-R11: the input schema: the enum from core's constants, the three
/// fields nullable strings, none required.
#[test]
fn ac_r11_the_kind_enum_and_the_three_fields_are_listed() {
    assert_eq!(INTERPRETATION_KIND, "interpretation");
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
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
        ])
    );
    for field in ["decision", "how", "why"] {
        let property = &schema["properties"][field];
        assert_eq!(
            property["type"],
            json!(["string", "null"]),
            "{field}: {property}"
        );
        assert!(property.get("enum").is_none(), "{field}");
        assert!(
            !schema["required"]
                .as_array()
                .expect("required")
                .contains(&json!(field)),
            "{field} is not required"
        );
    }
    let description = change["description"].as_str().expect("a description");
    assert!(
        description.contains("update|create|interpretation")
            && description.contains("decision, how, why"),
        "{description}"
    );
    // The review document's schema holds the key.
    let schemas = output_schemas(&list);
    assert!(
        schemas["get_proposal"]["properties"]
            .get("interpretation")
            .is_some(),
        "{}",
        schemas["get_proposal"]
    );
    drop(session.finish());
}

/// AC-R11 and AC-T3: the form errors (-32602 naming the field), a valid
/// call as its twin with `interpretation`, a superseded decision refused
/// naming the successor, `get_proposal` as `spec review --brief --json`;
/// the worktrees untouched and `git status` empty after every call. M: a
/// kind literal in `specengine-mcp/src`; a file write in propose.
#[test]
fn ac_r11_propose_change_takes_an_interpretation_as_its_twin() {
    let repo = Repo::new("mi-r11");
    let twin_home = repo.scratch.home("twin");
    let cwd = repo.linked.clone();
    let (hash, span) = repo.span("RULE-STAM-REGEN");
    let text = span.replacen("Base rate 10 units/s,", "Base rate 10 units/s at rest,", 1);
    assert_ne!(text, span);
    let call = |kind: &str| {
        json!({"kind": kind, "target": "RULE-STAM-REGEN", "base": hash, "decision": "DEC-0023",
            "how": HOW, "why": WHY, "text": text, "rationale": "Carry DEC-0023 into the rule.",
            "author_role": "spec-author"})
    };
    let before = repo.files();
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());

    // The form: each of the four missing on an interpretation.
    for field in ["decision", "how", "why", "base"] {
        for absent in [true, false] {
            let mut args = call(INTERPRETATION_KIND);
            if absent {
                args.as_object_mut().unwrap().remove(field);
            } else {
                args[field] = Value::Null;
            }
            let reply = session.call("propose_change", args);
            assert_eq!(error_code(&reply), INVALID_PARAMS, "{field}: {reply}");
            let message = reply["error"]["message"].as_str().expect("a message");
            assert!(
                message.starts_with(&format!("{field}: ")),
                "{field}: {message}"
            );
            if field != "base" {
                assert_eq!(
                    message,
                    format!("{field}: required with kind interpretation")
                );
            }
            assert_eq!(repo.statuses(), (String::new(), String::new()), "{field}");
            assert!(
                repo.stored(&repo.home).is_empty(),
                "{field}: nothing stored"
            );
        }
    }
    // One of the three on an update or a create.
    for kind in [UPDATE_KIND, CREATE_KIND] {
        for field in ["decision", "how", "why"] {
            let mut args = call(kind);
            for other in ["decision", "how", "why"] {
                if other != field {
                    args.as_object_mut().unwrap().remove(other);
                }
            }
            let reply = session.call("propose_change", args);
            assert_eq!(
                error_code(&reply),
                INVALID_PARAMS,
                "{kind} {field}: {reply}"
            );
            assert_eq!(
                reply["error"]["message"],
                json!(format!("{field}: only an interpretation takes it")),
                "{kind} {field}"
            );
        }
    }
    assert!(repo.stored(&repo.home).is_empty(), "nothing stored");
    // `null` on an update is no field given: stored as an update.
    let mut update = call(UPDATE_KIND);
    for field in ["decision", "how", "why"] {
        update[field] = Value::Null;
    }
    let reply = session.call("propose_change", update);
    let structured = &result(&reply)["structuredContent"];
    assert_eq!(structured["kind"], json!(UPDATE_KIND), "{reply}");
    assert_eq!(structured["interpretation"], Value::Null, "{reply}");
    assert_eq!(repo.stored(&repo.home), ["PR-0001"]);
    // W7 over MCP: the update lies in DEC-0023's `canon:` section, a
    // note, never a refusal.
    let w7 = "RULE-STAM-REGEN is in the `canon:` section of DEC-0023: a change carrying one is \
              `spec propose interpretation RULE-STAM-REGEN --decision DEC-0023`";
    assert_eq!(result(&reply)["isError"], json!(false), "{reply}");
    assert!(
        structured["notes"]
            .as_array()
            .is_some_and(|notes| notes.contains(&json!(w7))),
        "{reply}"
    );
    assert!(
        content_text(result(&reply)).contains(&format!("note: {w7}")),
        "{reply}"
    );

    // A superseded decision: an error result naming its successor.
    let mut superseded = call(INTERPRETATION_KIND);
    superseded["decision"] = json!("DEC-0007");
    let reply = session.call("propose_change", superseded.clone());
    assert_eq!(result(&reply)["isError"], json!(true), "{reply}");
    assert!(
        content_text(result(&reply))
            .contains("--decision: DEC-0007 is superseded by DEC-0023: interpret DEC-0023"),
        "{reply}"
    );
    assert_twin(
        &reply,
        &interpret_twin(&superseded, &cwd, &twin_home, "2026-10-08T12:00:00Z"),
        "superseded",
    );
    assert_eq!(repo.stored(&repo.home), ["PR-0001"]);

    // A valid call: as its twin.
    let valid = call(INTERPRETATION_KIND);
    let reply = session.call("propose_change", valid.clone());
    let structured = result(&reply)["structuredContent"].clone();
    assert_eq!(structured["id"], json!("PR-0002"), "{reply}");
    assert_eq!(structured["kind"], json!(INTERPRETATION_KIND));
    assert_eq!(
        structured["interpretation"],
        json!({"decision": "DEC-0023", "how": HOW, "why": WHY})
    );
    assert_eq!(structured["new_text"], Value::Null, "brief");
    let created = structured["created_at"]
        .as_str()
        .expect("created_at")
        .to_owned();
    // The twin stores its own PR-0001 in its HOME (no update there): the
    // IDs differ, the rest is equal.
    let want = interpret_twin(&valid, &cwd, &twin_home, &created);
    let mut want_document = want.document.clone().expect("a document");
    want_document["id"] = json!("PR-0002");
    let mut got = structured.clone();
    got["id"] = json!("PR-0002");
    assert_eq!(got, want_document, "structuredContent as the twin's");
    assert_eq!(
        content_text(result(&reply)).replacen("PR-0002", "PR-0001", 1),
        want.text,
        "content as the twin's"
    );
    if let Err(problem) = common::read::conforms(&schemas["propose_change"], &structured, "$") {
        panic!("the document breaks the outputSchema: {problem}");
    }

    // `get_proposal` = `spec review PR-0002 --brief --json`.
    let read = session.call("get_proposal", json!({"proposal_id": "PR-0002"}));
    let document = result(&read)["structuredContent"].clone();
    let brief = specengine_cli::review_brief(
        &cli_env(&cwd, Some(&repo.home)),
        &Globals::default(),
        &ReviewRequest {
            id: "PR-0002".to_owned(),
            git: GitEnv::new(&cwd, repo.git.vars()),
        },
    )
    .expect("review --brief");
    let rendered = specengine_cli::render_json(&Outcome::Proposal(Box::new(brief)));
    let mirrored: Value = serde_json::from_str(&rendered).expect("JSON");
    assert_eq!(document["interpretation"], mirrored["interpretation"]);
    // 48 since docs/features/drift-queue.md added `drift`.
    assert_eq!(
        document.as_object().map(|object| object.len()),
        Some(48),
        "{document}"
    );
    if let Err(problem) = common::read::conforms(&schemas["get_proposal"], &document, "$") {
        panic!("get_proposal breaks its outputSchema: {problem}");
    }
    drop(session.finish());

    // AC-T3: the one door.
    assert_eq!(repo.statuses(), (String::new(), String::new()));
    assert_eq!(repo.files(), before, "nothing under either root written");
    assert_eq!(repo.stored(&repo.home), ["PR-0001", "PR-0002"]);
}
