//! docs/features/reference-promotion.md (RM-0138, ADR-0089) "Acceptance
//! criteria", the MCP half: the default build's stdio server over scratch
//! git repositories of `fixtures/promote-mini`, its references written by
//! this build's `spec` binary (the build directory's, next to the server),
//! and the hub's constants. Every server has a cleared environment and a
//! scratch `HOME`.
//!
//! - PR1, MCP: `propose_change {kind: "reference"}` stores a `reference`
//!   row, writing nothing under the root; `tools/list` names no promote tool.
//! - PR16: `propose_change {kind: "reference", target, reference_id, text:
//!   "", rationale, author_role}` answers as its twin, `spec propose
//!   reference … --brief` on a twin repository (`2>&1`: stderr's lines, then
//!   stdout), its `structuredContent` the twin's `--json` document but the
//!   times; `reference_id` on an `update`, `name` on an `update`, `base`,
//!   `dry_run`, `decision`, `how`, `why`, `distinct_from` or `keywords` on a
//!   `reference`, no `reference_id`: -32602 naming the key, nothing stored;
//!   stdio 38 tools, a hub 24; `propose_change`'s description,
//!   `INSTRUCTIONS` and `HUB_INSTRUCTIONS` name the kind, ASCII within 2 048
//!   B. M: `reference_id` accepted on an `update`.
//!
//! The hub over `/mcp` is `specengine-http`'s `tests/reference_promotion.rs`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;
#[path = "../../specengine-cli/tests/work_refs_common/mod.rs"]
mod refs_common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use common::read::{Era, Session, content_text};
use common::*;
use git::Sandbox;
use refs_common::blob_fixture;
use serde_json::{Value, json};
use specengine_mcp::{HUB_INSTRUCTIONS, HUB_TOOLS, INSTRUCTIONS};

/// The build directory's `spec`, next to the server's binary.
fn spec_bin() -> PathBuf {
    let dir = Path::new(BIN).parent().expect("the server's directory");
    let path = dir.join("spec");
    assert!(
        path.is_file(),
        "{}: build it first (`cargo build --workspace --bins`)",
        path.display()
    );
    path
}

struct Ran {
    code: i32,
    stdout: String,
    stderr: String,
}

/// A scratch git repository of promote-mini, committed, its queue in `home`.
struct Repo {
    scratch: Scratch,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.copy("promote-mini", "repo");
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "the fixture");
        let home = scratch.home("h");
        Self {
            scratch,
            root,
            home,
        }
    }

    fn spec(&self, args: &[&str]) -> Ran {
        self.spec_at(&self.home, args)
    }

    /// `spec args` in the repository with `HOME` `home`: a twin's own data
    /// directory over the same worktree.
    fn spec_at(&self, home: &Path, args: &[&str]) -> Ran {
        let mut command = isolated_command(spec_bin(), args, &self.root, Some(home));
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = command.output().expect("spec ran");
        Ran {
            code: output.status.code().expect("an exit code"),
            stdout: String::from_utf8(output.stdout).expect("UTF-8"),
            stderr: String::from_utf8(output.stderr).expect("UTF-8"),
        }
    }

    fn doc(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let ran = self.spec(&all);
        assert_eq!(ran.code, 0, "{all:?}: {}{}", ran.stdout, ran.stderr);
        serde_json::from_str(&ran.stdout).expect("one JSON document")
    }

    /// C-0001 on LOC-HARBOUR with the fixture's harbour.png: RF-0001.
    fn scene(&self) {
        self.scene_at(&self.home);
    }

    /// [`Repo::scene`] in the data directory of `home`.
    fn scene_at(&self, home: &Path) {
        let file = blob_fixture("harbour.png");
        for (args, id) in [
            (
                vec![
                    "--json",
                    "card",
                    "new",
                    "--title",
                    "Light the harbour",
                    "--target",
                    "LOC-HARBOUR",
                ],
                "C-0001",
            ),
            (
                vec![
                    "--json",
                    "ref",
                    "add",
                    "--on",
                    "C-0001",
                    file.to_str().expect("UTF-8"),
                ],
                "RF-0001",
            ),
        ] {
            let ran = self.spec_at(home, &args);
            assert_eq!(ran.code, 0, "{args:?}: {}{}", ran.stdout, ran.stderr);
            let made: Value = serde_json::from_str(&ran.stdout).expect("JSON");
            assert_eq!(made["id"], json!(id));
        }
    }

    fn session(&self) -> Session {
        Session::open(Era::Stateless, &[], Some(&self.root), Home::At(&self.home))
    }

    /// The stored proposals' IDs.
    fn stored(&self) -> Vec<String> {
        let db = data_dir(&self.home).join("promote-mini.db");
        if !db.exists() {
            return Vec::new();
        }
        let output = std::process::Command::new("/usr/bin/sqlite3")
            .arg(&db)
            .arg("SELECT id FROM proposals ORDER BY id;")
            .output()
            .expect("sqlite3");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Every file of the worktree but `.git`'s.
    fn files(&self) -> Vec<String> {
        snapshot(&self.root)
            .into_iter()
            .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
            .map(|(path, bytes)| format!("{path} {:?}", bytes.map(|b| b.len())))
            .collect()
    }
}

/// The arguments of an agent's promotion of RF-0001 to LOC-HARBOUR.
fn promotion() -> Value {
    json!({"kind": "reference", "target": "LOC-HARBOUR", "reference_id": "RF-0001", "text": "",
           "rationale": "Approved by the art lead.", "author_role": "art-lead"})
}

/// `structuredContent` without the times a second run would move.
fn timeless(mut document: Value) -> Value {
    if let Some(map) = document.as_object_mut() {
        for key in ["created_at", "updated_at"] {
            if map.contains_key(key) {
                map.insert(key.to_owned(), Value::Null);
            }
        }
    }
    document
}

/// PR1 and PR16: `propose_change` of kind `reference` answers as its twin
/// on another repository of the same fixture, stores the row and writes
/// nothing under the root.
#[test]
fn pr16_propose_change_promotes_as_its_twin() {
    let repo = Repo::new("mcp-pr16-twin");
    repo.scene();
    // The twins: the same worktree, each its own data directory.
    let twin_text = repo.scratch.home("twin-text");
    repo.scene_at(&twin_text);
    let twin_json = repo.scratch.home("twin-json");
    repo.scene_at(&twin_json);
    let before = repo.files();
    let mut session = repo.session();
    let reply = session.call("propose_change", promotion());
    let made = result(&reply);
    assert_ne!(made["isError"], json!(true), "{made}");
    let document = &made["structuredContent"];
    assert_eq!(document["id"], json!("PR-0001"));
    assert_eq!(document["kind"], json!("reference"));
    assert_eq!(document["ref_id"], json!("RF-0001"));
    assert_eq!(repo.files(), before, "nothing under the root");
    assert_eq!(repo.stored(), ["PR-0001"]);

    let args = [
        "propose",
        "reference",
        "RF-0001",
        "--to",
        "LOC-HARBOUR",
        "--rationale",
        "Approved by the art lead.",
        "--author-role",
        "art-lead",
        "--brief",
    ];
    let cli = repo.spec_at(&twin_text, &args);
    assert_eq!(cli.code, 0, "{}{}", cli.stdout, cli.stderr);
    assert_eq!(
        content_text(made),
        format!("{}{}", cli.stderr, cli.stdout),
        "the content is `spec propose reference … --brief 2>&1`"
    );
    let mut with_json = vec!["--json"];
    with_json.extend_from_slice(&args);
    let ran = repo.spec_at(&twin_json, &with_json);
    assert_eq!(ran.code, 0, "{}{}", ran.stdout, ran.stderr);
    let twin_document: Value = serde_json::from_str(&ran.stdout).expect("JSON");
    assert_eq!(timeless(document.clone()), timeless(twin_document));
    drop(session.finish());
}

/// PR16: each misplaced key is -32602 naming it, nothing stored, nothing
/// written. M: `reference_id` accepted on an `update`.
#[test]
fn pr16_misplaced_keys_are_invalid_params() {
    let repo = Repo::new("mcp-pr16-form");
    repo.scene();
    let shown = repo.doc(&["show", "LOC-HARBOUR"]);
    let node = if shown["nodes"].is_array() {
        shown["nodes"][0].clone()
    } else {
        shown
    };
    let base = node["span_hash"].as_str().expect("a span hash").to_owned();
    let text = node["text"].as_str().expect("a text").to_owned();
    let update = json!({"kind": "update", "target": "LOC-HARBOUR", "base": base,
                        "text": text.replacen("before dawn", "before first light", 1),
                        "rationale": "Wording.", "author_role": "writer"});
    let before = repo.files();
    let mut session = repo.session();
    let mut cases: Vec<(String, Value)> = Vec::new();
    let mut with = |label: &str, mut args: Value, key: &str, value: Value| {
        args[key] = value;
        cases.push((label.to_owned(), args));
    };
    with(
        "reference_id on an update",
        update.clone(),
        "reference_id",
        json!("RF-0001"),
    );
    with(
        "name on an update",
        update.clone(),
        "name",
        json!("harbour.png"),
    );
    with("base on a reference", promotion(), "base", json!(base));
    with(
        "dry_run on a reference",
        promotion(),
        "dry_run",
        json!(true),
    );
    with(
        "decision on a reference",
        promotion(),
        "decision",
        json!("DEC-0001"),
    );
    with("how on a reference", promotion(), "how", json!("so"));
    with("why on a reference", promotion(), "why", json!("because"));
    with(
        "distinct_from on a reference",
        promotion(),
        "distinct_from",
        json!(["PR-0009"]),
    );
    with(
        "keywords on a reference",
        promotion(),
        "keywords",
        json!(["harbour"]),
    );
    let mut bare = promotion();
    bare.as_object_mut()
        .expect("an object")
        .remove("reference_id");
    cases.push(("no reference_id".to_owned(), bare));
    for (label, args) in cases {
        let key = label.split(' ').next().expect("a key").to_owned();
        let key = if key == "no" {
            "reference_id".to_owned()
        } else {
            key
        };
        let reply = session.call("propose_change", args);
        let error = reply
            .get("error")
            .unwrap_or_else(|| panic!("{label}: not an error: {reply}"));
        assert_eq!(error["code"], json!(INVALID_PARAMS), "{label}: {reply}");
        assert!(
            error["message"].as_str().is_some_and(|m| m.contains(&key)),
            "{label}: names `{key}`: {reply}"
        );
        assert_eq!(
            repo.stored(),
            Vec::<String>::new(),
            "{label}: nothing stored"
        );
        assert_eq!(repo.files(), before, "{label}: nothing written");
    }
    drop(session.finish());
}

/// PR16: stdio 38 tools, a hub 24, no promote tool; `propose_change`'s kind
/// list holds `reference`, its inputs `reference_id` and `name`; its
/// description and both instructions name the kind, ASCII within 2 048 B.
#[test]
fn pr16_the_tools_and_their_texts() {
    let repo = Repo::new("mcp-pr16-tools");
    let mut session = repo.session();
    let list = session.tools();
    let names = tool_names(&list);
    assert_eq!(names.len(), 38, "{names:?}");
    assert_eq!(HUB_TOOLS.len(), 24, "{HUB_TOOLS:?}");
    assert!(
        !names.iter().any(|name| name.contains("promot")),
        "PR1: no promote tool: {names:?}"
    );
    let propose = tool(&list, "propose_change");
    let schema = &propose["inputSchema"];
    let properties = schema["properties"].as_object().expect("properties");
    for key in ["reference_id", "name"] {
        assert!(properties.contains_key(key), "{key}: {schema}");
    }
    let kinds = &properties["kind"]["enum"];
    assert!(
        kinds
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == "reference")),
        "{kinds}"
    );
    for forbidden in ["as", "login", "b3", "file", "bytes"] {
        assert!(!properties.contains_key(forbidden), "{forbidden}: {schema}");
    }
    let description = propose["description"].as_str().expect("a description");
    for (label, text) in [
        ("propose_change", description),
        ("INSTRUCTIONS", INSTRUCTIONS),
        ("HUB_INSTRUCTIONS", HUB_INSTRUCTIONS),
    ] {
        assert!(text.is_ascii(), "{label}: ASCII");
        assert!(text.len() <= 2048, "{label}: {} B", text.len());
        assert!(text.contains("reference"), "{label} names the kind: {text}");
    }
    drop(session.finish());
}

/// PR1: a refused promotion over stdio (a link) is the twin's error result,
/// nothing stored; the files untouched.
#[test]
fn pr1_a_refused_promotion_stores_nothing() {
    let repo = Repo::new("mcp-pr1-link");
    repo.scene();
    let link = repo.doc(&[
        "ref",
        "add",
        "--on",
        "C-0001",
        "--url",
        "https://example.org/board",
    ]);
    let rf = link["id"].as_str().expect("an ID").to_owned();
    let before = repo.files();
    let mut session = repo.session();
    let mut args = promotion();
    args["reference_id"] = json!(rf);
    let reply = session.call("propose_change", args);
    let made = result(&reply);
    assert_eq!(made["isError"], json!(true), "{made}");
    assert!(
        content_text(made).contains(&format!("{rf} is a link: only a file is promoted into git")),
        "{}",
        content_text(made)
    );
    assert_eq!(repo.stored(), Vec::<String>::new());
    assert_eq!(repo.files(), before);
    let _ = fs::metadata(&repo.root);
    drop(session.finish());
}
