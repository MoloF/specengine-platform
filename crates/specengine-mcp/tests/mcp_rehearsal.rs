//! docs/features/proposal-rehearsal.md (RM-0067) over MCP, the real binary
//! over stdio: `propose_change {dry_run: true}` and the `ref-dangling`
//! filter as an agent reads them.
//!
//! - AC-01: on a game and a command-line tool, a new file's dry run answers
//!   the real call's document: `id` and `status` `null`, the real
//!   `diagnostics` (the real call runs after it), `notes` opening with
//!   `dry run: nothing stored`, conforming to the unchanged output schema;
//!   nothing stored, nothing written; the real call then stores under the
//!   first ID. `dry_run: false` is the real call. M: the row stored;
//!   validation skipped.
//! - AC-02: a taken ID, a stale base and `class: generated`: an error result
//!   with the real call's reason, on `update` and both forms of `create`. M:
//!   returning before step 4.
//! - AC-03: `dry_run: true` with `interpretation` or `amendment` is -32602
//!   naming the field, nothing stored; `dry_run: false` with them is the real
//!   call. M: ignored, a row stored.
//! - AC-04: create A, then B `follows: [A]`: no `ref-dangling`, a note names
//!   A's PR; a reference to nothing keeps its finding. M: filtering on the
//!   code alone.
//! - "Schema": `dry_run` optional, in the spec's words; `propose_change`'s
//!   description as it was (the pin: `plugin_files.rs`).
//!
//! The setup's git runs in the CLI tests' sandbox (`common/git.rs`, included
//! by path); every server with a cleared environment and a scratch `HOME`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use common::read::{ERAS, Era, Session, cli_env, conforms, content_text, output_schemas};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{GitEnv, Globals, InboxRequest, ShowRequest, inbox, show};

/// A dry run's first note ("Data").
const DRY_NOTE: &str = "dry run: nothing stored";
/// "Schema"'s text of the property.
const DRY_RUN_TEXT: &str = "true: every refusal and the diagnostics of the real call, nothing stored; update and create only.";
/// A base no span hashes to.
const STALE: &str = "b3:0000000000000000000000000000000000000000000000000000000000000000";
const R13: &str = "docs/records/R/R-13.md";
const R14: &str = "docs/records/R/R-14.md";
const R15: &str = "docs/records/R/R-15.md";
const STAMINA: &str = "docs/spec/movement/stamina.md";
const TARGET: &str = "RULE-STAM-REGEN";
/// An amendment of `RULE-STAM-REGEN`.
const AMENDMENT: &str = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                         [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## \
                         Delay {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint.\n";
const AMENDMENT_PATH: &str = "docs/features/regen-delay.md";

/// A scratch git repository of a fixture: its main worktree (one commit on
/// `main`) and a linked worktree on `t1`, a data `HOME`.
struct Repo {
    /// Kept for its `Drop`.
    _scratch: Scratch,
    git: Sandbox,
    main: PathBuf,
    linked: PathBuf,
    home: PathBuf,
    slug: &'static str,
}

impl Repo {
    fn new(label: &str, fixture: &str, slug: &'static str) -> Self {
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
                linked.to_str().expect("a UTF-8 scratch path"),
            ],
        );
        let linked = fs::canonicalize(linked).expect("the linked worktree");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            git,
            main,
            linked,
            home,
            slug,
        }
    }

    /// spec-a, a game.
    fn game(label: &str) -> Self {
        Self::new(label, "spec-a", "lantern-keep")
    }

    /// spec-b, a command-line tool with Russian prose.
    fn tool(label: &str) -> Self {
        Self::new(label, "spec-b", "zerkalo")
    }

    /// A session of an agent in the linked worktree.
    fn session(&self) -> Session {
        Session::open(
            Era::Stateless,
            &[],
            Some(&self.linked),
            Home::At(&self.home),
        )
    }

    /// `(span_hash, text)` of `reference` in the linked worktree.
    fn span(&self, reference: &str) -> (String, String) {
        let outcome = show(
            &cli_env(&self.linked, Some(&self.home)),
            &Globals::default(),
            &ShowRequest {
                reference: reference.to_owned(),
                links: false,
                archive: false,
                bindings: false,
                git: GitEnv::new(&self.linked, self.git.vars()),
            },
        )
        .unwrap_or_else(|error| panic!("show {reference}: {error}"));
        let node = outcome.nodes.into_iter().next().expect("one node");
        (node.span_hash, node.text)
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

    /// The proposal IDs stored (`spec inbox --all`).
    fn stored(&self) -> Vec<String> {
        if !data_dir(&self.home)
            .join(format!("{}.db", self.slug))
            .exists()
        {
            return Vec::new();
        }
        inbox(
            &cli_env(&self.main, Some(&self.home)),
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

    /// Nothing written to either worktree, nothing queued but `stored`.
    fn assert_untouched(
        &self,
        before: &(BTreeMap<String, Option<Vec<u8>>>, String),
        stored: &[&str],
        context: &str,
    ) {
        assert_eq!(self.statuses(), (String::new(), String::new()), "{context}");
        assert_eq!(&self.files(), before, "{context}: nothing written");
        assert_eq!(self.stored(), stored, "{context}");
    }
}

/// `propose_change`'s arguments of an agent.
fn change(kind: &str, target: &str, text: &str) -> Value {
    json!({"kind": kind, "target": target, "text": text, "rationale": "Rehearse it.",
        "author_role": "writer"})
}

/// `args`, a dry run.
fn dry(args: &Value) -> Value {
    let mut args = args.clone();
    args["dry_run"] = json!(true);
    args
}

/// `args` against `base`.
fn based(mut args: Value, base: &str) -> Value {
    args["base"] = json!(base);
    args
}

/// A spec-a record `id`, `extra` closing its front-matter.
fn record(id: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n\
         {extra}---\n\n# Record {id}\n\nA body.\n"
    )
}

/// A spec-b record `id`, Russian prose, `extra` closing its front-matter.
fn record_b(id: &str, extra: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nkind: requirement\nowner: owner\nreviewed: 2026-09-20\n\
         {extra}---\n\n# \u{0421}\u{043b}\u{0438}\u{044f}\u{043d}\u{0438}\u{0435}\n\n\
         \u{0412}\u{0435}\u{0442}\u{043a}\u{0430} \u{0441}\u{043b}\u{0438}\u{0432}\u{0430}\u{0435}\u{0442}\u{0441}\u{044f}.\n"
    )
}

/// The document of an answer that is no error.
fn answered(reply: &Value, context: &str) -> Value {
    let result = result(reply);
    assert_ne!(result["isError"], json!(true), "{context}: {reply}");
    result["structuredContent"].clone()
}

/// The subjects of a document's findings of `code`.
fn subjects(document: &Value, code: &str) -> Vec<String> {
    document["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("diagnostics: {document}"))
        .iter()
        .filter(|finding| finding["code"] == code)
        .map(|finding| finding["subject"].as_str().expect("a subject").to_owned())
        .collect()
}

fn notes(document: &Value) -> Vec<String> {
    document["notes"]
        .as_array()
        .unwrap_or_else(|| panic!("notes: {document}"))
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

/// AC-01 on both genres: the dry run's document, then the real call's. M: the
/// row stored; validation skipped.
#[test]
fn ac01_a_dry_run_answers_the_real_document_and_stores_nothing() {
    for (label, repo, id, path, text, missing) in [
        (
            "spec-a",
            Repo::game("mr-ac01-a"),
            "R-13",
            R13,
            record("R-13", "links: {derived_from: [R-99]}\n"),
            "R-99",
        ),
        (
            "spec-b",
            Repo::tool("mr-ac01-b"),
            "REQ-900",
            "docs/records/REQ/REQ-900.md",
            record_b("REQ-900", "links: {derived_from: [REQ-999]}\n"),
            "REQ-999",
        ),
    ] {
        let before = repo.files();
        let mut session = repo.session();
        let schemas = output_schemas(&session.tools());
        let args = change("create", path, &text);

        let reply = session.call("propose_change", dry(&args));
        let document = answered(&reply, label);
        assert_eq!(document["id"], Value::Null, "{label}: {document}");
        assert_eq!(document["status"], Value::Null, "{label}: {document}");
        assert_eq!(document["kind"], json!("create"), "{label}");
        assert_eq!(document["target_id"], json!(id), "{label}");
        assert_eq!(document["target_path"], json!(path), "{label}");
        assert_eq!(document["notes"][0], json!(DRY_NOTE), "{label}: {document}");
        assert_eq!(subjects(&document, "ref-dangling"), [missing], "{label}");
        for key in ["base_commit", "preview", "revisions", "created_at"] {
            assert_eq!(document[key], Value::Null, "{label}: {key}");
        }
        if let Err(problem) = conforms(&schemas["propose_change"], &document, "$") {
            panic!("{label}: the document breaks the outputSchema: {problem}");
        }
        assert!(
            content_text(result(&reply)).contains(DRY_NOTE),
            "{label}: {reply}"
        );
        repo.assert_untouched(&before, &[], label);

        // One state, one answer.
        let again = session.call("propose_change", dry(&args));
        assert_eq!(answered(&again, label), document, "{label}");

        let reply = session.call("propose_change", args);
        let stored = answered(&reply, label);
        assert_eq!(stored["id"], json!("PR-0001"), "{label}: no ID burnt");
        assert_eq!(
            stored["diagnostics"], document["diagnostics"],
            "{label}: the real call's"
        );
        repo.assert_untouched(&before, &["PR-0001"], label);
        drop(session.finish());
    }
}

/// New sections: a dry run leaves their ID free, and `dry_run: false` is the
/// real call. M: `is_some` read as true.
#[test]
fn ac01_new_sections_hold_no_id_and_false_is_the_real_call() {
    let repo = Repo::game("mr-ac01-sections");
    let before = repo.files();
    let (hash, span) = repo.span(TARGET);
    let mut session = repo.session();
    let text = format!("{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Waits 1.5 s.");
    let args = based(change("create", TARGET, &text), &hash);
    let document = answered(&session.call("propose_change", dry(&args)), "sections");
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["target_path"], json!(STAMINA), "{document}");
    repo.assert_untouched(&before, &[], "sections");

    let mut real = args;
    real["dry_run"] = json!(false);
    let stored = answered(&session.call("propose_change", real), "false");
    assert_eq!(stored["id"], json!("PR-0001"), "{stored}");
    assert_eq!(stored["target_ids"], document["target_ids"], "{stored}");
    drop(session.finish());
}

/// AC-02 on `update` and both forms of `create`: the error result holds the
/// real call's reason; nothing stored. M: returning before step 4.
#[test]
fn ac02_a_taken_id_a_stale_base_and_a_generated_text_give_the_real_reason() {
    let repo = Repo::game("mr-ac02");
    let before = repo.files();
    let (hash, span) = repo.span(TARGET);
    let rate = span.replacen("Base rate 10 units/s,", "Base rate 11 units/s,", 1);
    assert_ne!(rate, span, "the fixture's line");
    let rest = format!("{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Waits 1.5 s.");
    let twice = format!("{span}\n\n### Twice {{#EDGE-SPRINT-EMPTY}}\n- Again.");
    let generated = record("R-13", "").replacen("class: canon\n", "class: generated\n", 1);
    let cases = [
        (
            "a taken ID, a file",
            change("create", "docs/records/R/R-50.md", &record("R-12", "")),
            "R-12",
        ),
        (
            "a taken ID, a section",
            based(change("create", TARGET, &twice), &hash),
            "EDGE-SPRINT-EMPTY",
        ),
        (
            "a stale base, an update",
            based(change("update", TARGET, &rate), STALE),
            hash.as_str(),
        ),
        (
            "a stale base, a section",
            based(change("create", TARGET, &rest), STALE),
            hash.as_str(),
        ),
        (
            "generated",
            change("create", R13, &generated),
            "class: generated",
        ),
    ];
    let mut session = repo.session();
    for (label, args, fragment) in cases {
        let real = session.call("propose_change", args.clone());
        assert_eq!(result(&real)["isError"], json!(true), "{label}: {real}");
        let reason = content_text(result(&real))
            .lines()
            .last()
            .expect("a reason")
            .to_owned();
        assert!(reason.contains(fragment), "{label}: {reason}");

        let rehearsal = session.call("propose_change", dry(&args));
        assert_eq!(
            result(&rehearsal)["isError"],
            json!(true),
            "{label}: {rehearsal}"
        );
        assert!(
            content_text(result(&rehearsal)).contains(&reason),
            "{label}: {rehearsal}"
        );
        repo.assert_untouched(&before, &[], label);
    }
    // `task_id`: "checked once the place is bound, as in the real call".
    let mut args = change("create", R13, &record("R-13", ""));
    args["task_id"] = json!("T-9999");
    let reply = session.call("propose_change", dry(&args));
    assert_eq!(result(&reply)["isError"], json!(true), "{reply}");
    assert!(content_text(result(&reply)).contains("T-9999"), "{reply}");
    repo.assert_untouched(&before, &[], "task");
    drop(session.finish());
}

/// AC-02, a generated target, on the command-line tool's glossary entry.
#[test]
fn ac02_a_generated_target_is_refused_alike() {
    let repo = Repo::tool("mr-ac02-generated");
    let before = repo.files();
    let target = "GLS-task-branch";
    let (hash, span) = repo.span(target);
    let more = format!("{span}\nMore.\n");
    let mut session = repo.session();
    let args = based(change("update", target, &more), &hash);
    let reply = session.call("propose_change", dry(&args));
    assert_eq!(result(&reply)["isError"], json!(true), "{reply}");
    assert!(
        content_text(result(&reply)).contains("generated"),
        "{reply}"
    );
    repo.assert_untouched(&before, &[], "generated");
    drop(session.finish());
}

/// AC-03: -32602 naming `dry_run`, nothing stored; `false` is the real call.
/// M: ignored, a row stored.
#[test]
fn ac03_dry_run_with_interpretation_or_amendment_is_a_form_error() {
    let repo = Repo::game("mr-ac03");
    let before = repo.files();
    let (hash, span) = repo.span(TARGET);
    let rate = span.replacen(
        "Base rate 10 units/s,",
        "Base rate 10 units/s at rest (under 40 % of walking),",
        1,
    );
    assert_ne!(rate, span, "the fixture's line");
    let mut interpretation = based(change("interpretation", TARGET, &rate), &hash);
    interpretation["decision"] = json!("DEC-0023");
    interpretation["how"] = json!("Rest is any speed under 40 % of walking.");
    interpretation["why"] = json!("DEC-0023 says rest; the rule names no speed.");
    let amendment = change("amendment", AMENDMENT_PATH, AMENDMENT);
    let mut session = repo.session();
    for (label, args) in [
        ("interpretation", &interpretation),
        ("amendment", &amendment),
    ] {
        let reply = session.call("propose_change", dry(args));
        assert_eq!(error_code(&reply), INVALID_PARAMS, "{label}: {reply}");
        let message = reply["error"]["message"].as_str().expect("a message");
        assert!(message.starts_with("dry_run: "), "{label}: {message}");
        repo.assert_untouched(&before, &[], label);
    }
    let mut real = amendment;
    real["dry_run"] = json!(false);
    let stored = answered(&session.call("propose_change", real), "false");
    assert_eq!(stored["kind"], json!("amendment"), "{stored}");
    assert_eq!(repo.stored(), ["PR-0001"]);
    drop(session.finish());
}

/// AC-04 as an agent reads it: create A, then B `follows: [A]`, rehearsed and
/// real: no `ref-dangling`, the note names A's PR; `parent: A` alike; a
/// reference to nothing keeps its finding. M: filtering on the code alone.
#[test]
fn ac04_a_reference_to_a_live_create_stores_no_ref_dangling() {
    let repo = Repo::game("mr-ac04");
    let mut session = repo.session();
    let reply = session.call("propose_change", change("create", R13, &record("R-13", "")));
    let holder = answered(&reply, "A")["id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    let note =
        format!("`R-13` is reserved by `{holder}` (open), a live create: no ref-dangling stored");

    let follows = record("R-14", "links: {follows: [R-13]}\n");
    let args = change("create", R14, &follows);
    let rehearsed = answered(&session.call("propose_change", dry(&args)), "rehearsed");
    assert_eq!(notes(&rehearsed)[0], DRY_NOTE, "{rehearsed}");
    assert!(notes(&rehearsed).contains(&note), "{rehearsed}");
    assert_eq!(subjects(&rehearsed, "ref-dangling"), Vec::<String>::new());
    let stored = answered(&session.call("propose_change", args), "follows");
    assert!(notes(&stored).contains(&note), "{stored}");
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());

    let child = record("R-15", "parent: R-13\n");
    let stored = answered(
        &session.call("propose_change", change("create", R15, &child)),
        "parent",
    );
    assert!(notes(&stored).contains(&note), "{stored}");
    assert_eq!(subjects(&stored, "ref-dangling"), Vec::<String>::new());

    let both = record("R-16", "links: {follows: [R-13, R-99]}\n");
    let stored = answered(
        &session.call(
            "propose_change",
            change("create", "docs/records/R/R-16.md", &both),
        ),
        "nothing",
    );
    assert_eq!(subjects(&stored, "ref-dangling"), ["R-99"], "{stored}");
    assert!(notes(&stored).contains(&note), "{stored}");
    drop(session.finish());
}

/// "Schema": `dry_run` an optional boolean in the spec's words, in both eras;
/// `propose_change`'s description stays as it was, 19 B under 2 048; 4 B
/// under since docs/features/spec-ids.md "Data", "The MCP texts" (2 044 B).
#[test]
fn the_input_schema_takes_dry_run_and_the_description_stays() {
    for era in ERAS {
        let mut session = Session::open(era, &[], None, Home::Fresh);
        let list = session.tools();
        let change = tool(&list, "propose_change");
        let schema = &change["inputSchema"];
        let property = &schema["properties"]["dry_run"];
        assert_eq!(property["description"], json!(DRY_RUN_TEXT), "{property}");
        assert!(property.to_string().contains("boolean"), "{property}");
        let required = schema["required"].as_array().expect("required");
        assert!(!required.contains(&json!("dry_run")), "{required:?}");
        let description = change["description"].as_str().expect("a description");
        assert!(!description.contains("dry_run"), "{description}");
        assert_eq!(description.len(), 2_048 - 4, "{description}");
        drop(session.finish());
    }
}
