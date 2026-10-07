//! docs/features/proposal-kinds.md over MCP, the real binary over stdio:
//!
//! - AC-10: `propose_change {kind: "create", target: <new .md path>, base:
//!   null, …}` answers as `spec propose create … --brief` (`content` its
//!   `2>&1`, `structuredContent` its `--json`, the library twin in another
//!   `HOME` with the clock the server stamped); `base` absent alike; a
//!   create's sections against the span's hash alike; `create` on a node
//!   with `base: null` is AC-03's refusal (an error result, its text the
//!   twin's); `{kind: "update"}` with `base` null or absent is an
//!   invalid-params error naming `base`, nothing stored; an unknown kind a
//!   bad argument. The input schema: `kind` `["update", "create"]`, core's
//!   constants in that order, `base` nullable and not required; the
//!   description names `create`. After every call `git status --porcelain`
//!   of both worktrees is empty and their files as they were. M: `base`
//!   required.
//! - AC-17, the wire half: `INSTRUCTIONS` hold "Data"'s `propose_change`
//!   line (146 bytes with its LF), 1 740 bytes as of this slice, 1 878
//!   since docs/features/task-package.md's tasks line (138 bytes); the
//!   `probes` build sends them with the probe paragraph, 2 035 bytes.
//!
//! The setup's git runs in the CLI tests' sandbox (`common/git.rs`,
//! included by path, its repositories with automatic maintenance off);
//! every server with a cleared environment and a scratch `HOME`.
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
    CREATE_KIND, CreateRequest, GitEnv, Globals, InboxRequest, Outcome, ProposedText, ShowRequest,
    UPDATE_KIND, inbox, show,
};

/// "Data"'s `propose_change` line of the instructions, with its LF.
const CHANGE_LINE: &str = "- propose_change = spec propose update|create: a node's new text \
against its span_hash; create: new ID sections in it, or a new file (base null).\n";

/// "Data"'s new file.
const R13: &str = "docs/records/R/R-13.md";

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

    /// `(span_hash, text)` of `reference` in the linked worktree.
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

/// T13: spec-a's `R-12.md` with `id: R-13` and its own H1.
fn t13() -> String {
    read_text(&fixture("spec-a"), "docs/records/R/R-12.md")
        .replacen("id: R-12\n", "id: R-13\n", 1)
        .replacen(
            "# Stamina regenerates only at rest\n",
            "# Sprint keeps a stamina reserve\n",
            1,
        )
}

/// A spec-a record with `id: <id>`.
fn record(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # A new record\n\nA body.\n"
    )
}

/// The twin of a `propose_change` of kind `create`: `spec propose create
/// TARGET [--base B] --text-file - --rationale R --author-role ROLE
/// --brief` through the library, with `HOME` `home` and the clock `now`.
fn create_twin(args: &Value, cwd: &Path, home: &Path, now: &str) -> common::read::Expected {
    let text = |key: &str| args[key].as_str().map(str::to_owned);
    let outcome = specengine_cli::propose_create_brief(
        &cli_env(cwd, Some(home)),
        &Globals::default(),
        &CreateRequest {
            target: text("target").expect("a target"),
            base: text("base"),
            text: ProposedText::Given(text("text").expect("a text").into_bytes()),
            rationale: text("rationale").expect("a rationale"),
            author_role: text("author_role"),
            author_model: text("author_model"),
            run: text("run"),
            now: now.to_owned(),
            git: GitEnv::new(cwd, [("HOME", home.as_os_str())]),
        },
    )
    .map(|outcome| Outcome::Proposal(Box::new(outcome)));
    expected(outcome)
}

/// `content` and `structuredContent` of `reply` equal the twin's, its
/// `isError` the twin's.
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

/// AC-10: each call answered as its twin (or the error named), the
/// worktrees untouched after every call. M: `base` required.
#[test]
fn ac10_propose_change_creates_as_its_twin() {
    let repo = Repo::new("mc-ac10");
    let twin_home = repo.scratch.home("twin");
    let cwd = repo.linked.clone();
    let (hash, span) = repo.span("RULE-STAM-REGEN");
    let section = format!("{span}\n\n### Rest delay {{#EDGE-STAM-REST}}\n- Waits 1.5 s.");
    let writer = |kind: &str, target: &str, text: &str| {
        json!({"kind": kind, "target": target, "text": text,
            "rationale": "sprint needs it", "author_role": "writer"})
    };
    let mut null_base = writer("create", R13, &t13());
    null_base["base"] = Value::Null;
    let absent = writer("create", "docs/records/R/R-14.md", &record("R-14"));
    let mut sections = writer("create", "RULE-STAM-REGEN", &section);
    sections["base"] = json!(hash);
    let mut on_node = writer("create", "RULE-STAM-REGEN", &section);
    on_node["base"] = Value::Null;
    let taken = writer("create", "docs/records/R/R-50.md", &record("R-13"));
    let before_files = repo.files();
    let mut session = Session::open(Era::Stateless, &[], Some(&cwd), Home::At(&repo.home));
    let schemas = output_schemas(&session.tools());
    let schema = &schemas["propose_change"];
    let mut stored = Vec::new();
    for (label, args, created) in [
        ("a new file, base null", &null_base, Some("PR-0001")),
        ("a new file, base absent", &absent, Some("PR-0002")),
        ("new sections", &sections, Some("PR-0003")),
        ("a node with base null", &on_node, None),
        ("a reserved ID", &taken, None),
    ] {
        let reply = session.call("propose_change", args.clone());
        let structured = &result(&reply)["structuredContent"];
        let now = match created {
            Some(id) => {
                assert_eq!(structured["id"], json!(id), "{label}: {reply}");
                assert_eq!(structured["kind"], json!(CREATE_KIND), "{label}");
                assert_eq!(structured["new_text"], Value::Null, "{label}: brief");
                let read = session.call("get_proposal", json!({"proposal_id": id}));
                stored.push(id.to_owned());
                result(&read)["structuredContent"]["created_at"]
                    .as_str()
                    .expect("created_at")
                    .to_owned()
            }
            None => "2026-10-06T12:00:00Z".to_owned(),
        };
        let want = create_twin(args, &cwd, &twin_home, &now);
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
    // AC-03's refusal and the reservation's, as the twin's text.
    let refused = session.call("propose_change", on_node.clone());
    assert!(
        content_text(result(&refused)).contains(
            "`RULE-STAM-REGEN` exists: a create never replaces a file; to add ID sections to \
             it, name its span_hash with --base"
        ),
        "{refused}"
    );
    let reserved = session.call("propose_change", taken.clone());
    assert!(
        content_text(result(&reserved)).contains("`R-13` is reserved by `PR-0001` (open)"),
        "{reserved}"
    );
    // An update without its base: invalid params naming `base`.
    let (update_hash, update_span) = repo.span("EDGE-STAM-ZERO");
    let mut update_null = writer("update", "EDGE-STAM-ZERO", &format!("{update_span} Now."));
    update_null["base"] = Value::Null;
    let update_absent = writer("update", "EDGE-STAM-ZERO", &format!("{update_span} Now."));
    for (label, args) in [("base null", update_null), ("base absent", update_absent)] {
        let reply = session.call("propose_change", args);
        assert_eq!(error_code(&reply), INVALID_PARAMS, "{label}: {reply}");
        let message = reply["error"]["message"].as_str().expect("a message");
        assert!(message.starts_with("base: "), "{label}: {message}");
        assert_eq!(repo.statuses(), (String::new(), String::new()), "{label}");
        assert_eq!(repo.stored(&repo.home), stored, "{label}: nothing stored");
    }
    // An unknown kind: a bad argument naming the two.
    let mut unknown = writer("delete", R13, &t13());
    unknown["base"] = Value::Null;
    let reply = session.call("propose_change", unknown);
    let text = reply["error"]["message"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| content_text(result(&reply)).to_owned());
    assert!(
        text.contains("delete") && text.contains(UPDATE_KIND) && text.contains(CREATE_KIND),
        "{reply}"
    );
    // An update with its base still stores.
    let mut update = writer("update", "EDGE-STAM-ZERO", &format!("{update_span} Now."));
    update["base"] = json!(update_hash);
    let reply = session.call("propose_change", update);
    assert_eq!(
        result(&reply)["structuredContent"]["kind"],
        json!(UPDATE_KIND),
        "{reply}"
    );
    stored.push("PR-0004".to_owned());
    assert_eq!(repo.stored(&repo.home), stored);
    assert_eq!(repo.statuses(), (String::new(), String::new()));
    assert_eq!(repo.files(), before_files);
    drop(session.finish());
}

/// AC-10, the schema: `kind` `["update", "create"]` (core's constants, in
/// that order), `base` nullable and not required; the description names
/// `create` and a new file's null base. M: `base` required.
#[test]
fn ac10_the_schema_takes_create_and_a_nullable_base() {
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let list = session.tools();
    let change = tool(&list, "propose_change");
    let schema = &change["inputSchema"];
    let kind = &schema["properties"]["kind"];
    assert_eq!(kind["enum"], json!([UPDATE_KIND, CREATE_KIND]), "{kind}");
    let base = &schema["properties"]["base"];
    let nullable = base["type"]
        .as_array()
        .is_some_and(|types| types.contains(&json!("null")) && types.contains(&json!("string")))
        || base["anyOf"]
            .as_array()
            .is_some_and(|options| options.iter().any(|option| option["type"] == json!("null")));
    assert!(nullable, "base nullable: {base}");
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required")
        .iter()
        .map(|name| name.as_str().expect("a name"))
        .collect();
    assert!(!required.contains(&"base"), "{required:?}");
    assert!(required.contains(&"kind"), "{required:?}");
    let description = change["description"].as_str().expect("a description");
    assert!(description.contains("create"), "{description}");
    assert!(description.contains("base null"), "{description}");
    assert!(description.chars().count() <= 2048);
    drop(session.finish());
}

/// AC-17: `INSTRUCTIONS` hold "Data"'s `propose_change` line (146 bytes
/// with its LF) and are 1 878 bytes (1 740 and docs/features/task-package.md's
/// 138-byte tasks line); the default build sends them as is.
#[test]
#[cfg(not(feature = "probes"))]
fn ac17_the_instructions_name_create_within_their_budget() {
    assert_eq!(CHANGE_LINE.len(), 146);
    assert!(specengine_mcp::INSTRUCTIONS.contains(CHANGE_LINE));
    assert_eq!(specengine_mcp::INSTRUCTIONS.len(), 1878);
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let discover = result(&session.request("server/discover", json!({}))).clone();
    assert_eq!(
        discover["instructions"].as_str(),
        Some(specengine_mcp::INSTRUCTIONS)
    );
    drop(session.finish());
}

/// AC-17, the `probes` build: the instructions on the wire are
/// `INSTRUCTIONS` and the probe paragraph of "Data" (a blank line, 157
/// bytes), 1 897 bytes as of this slice; 2 035 of 2 048 since
/// docs/features/task-package.md ("Description and interactions") adds
/// its 138-byte tasks line.
#[test]
#[cfg(feature = "probes")]
fn ac17_the_probes_build_sends_the_short_probe_paragraph() {
    let paragraph = "\n\nProbes build, owner's checklist only: review_proposal(proposal_id) asks \
                     the owner by form; probe_output(tokens) returns filler; \
                     probe_sleep(seconds) waits.";
    assert_eq!(paragraph.len(), 157);
    let mut session = Session::open(Era::Stateless, &[], None, Home::Fresh);
    let discover = result(&session.request("server/discover", json!({}))).clone();
    let sent = discover["instructions"].as_str().expect("instructions");
    assert_eq!(sent, format!("{}{paragraph}", specengine_mcp::INSTRUCTIONS));
    assert_eq!(sent.len(), 2035);
    assert!(sent.len() <= 2048);
    assert!(sent.contains(CHANGE_LINE));
    drop(session.finish());
}
