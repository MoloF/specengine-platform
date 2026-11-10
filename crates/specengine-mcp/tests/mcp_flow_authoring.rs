//! docs/features/flow-authoring.md (RM-0068) over MCP, the real binary over
//! stdio: the path the `write-flow` skill gives an agent, taken from the
//! skill's own file on every run. `plugin_skills.rs` AC-07 asks only that the
//! server take the arguments; here they do the work.
//!
//! - AC-01's agent half: each `json` block sent verbatim as `propose_change`
//!   into an empty git project of the skill's config, the first rehearsed
//!   with `dry_run` as the skill's second step says, the owner's walk through
//!   the library with a scripted terminal, `get_flow` ten rows `measured` 0.
//!   A block's `ref-dangling` on a document not yet sent is expected: a loop
//!   cannot avoid it, approval clears it.
//! - AC-03 at the agent's door: `dry_run` of a text with an unknown link type
//!   and of an ID under no prefix gives the teaching words in `diagnostics`.
//!
//! The setup's git runs in the CLI tests' sandbox (`common/git.rs`, included
//! by path); the server has a cleared environment and a scratch `HOME`.

#![cfg(all(unix, not(feature = "probes")))]

mod common;

#[path = "../../specengine-cli/tests/common/git.rs"]
mod git;

use std::path::PathBuf;

use common::read::{Era, Session, cli_env, content_text};
use common::*;
use git::Sandbox;
use serde_json::{Value, json};
use specengine_cli::{ApproveOpenRequest, GitEnv, Globals, RoundTerminal, approve_open};

/// The skill, root-relative: the file a model loads, never a copy of it.
const SKILL: &str = "plugin/specengine/skills/write-flow/SKILL.md";
const LATER: &str = "2026-10-06T08:00:00Z";
/// A dry run's first note (docs/canon/proposal-kinds.md "Propose").
const DRY_NOTE: &str = "dry run: nothing stored";

/// The ten rows, in the spec's table order.
const CODES: [&str; 10] = [
    "no-start",
    "two-starts",
    "unreachable",
    "no-way-to-end",
    "trap-cycle",
    "unjoined-parallel",
    "cycle-no-repeat",
    "definition-unused",
    "definition-once",
    "supply-unfed",
];

/// The body of each fence of `language` in `text`.
fn fences(text: &str, language: &str) -> Vec<String> {
    let open = format!("```{language}");
    let lines: Vec<&str> = text.lines().collect();
    let mut bodies = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if lines[at] == open {
            let close = lines[at + 1..]
                .iter()
                .position(|line| *line == "```")
                .unwrap_or_else(|| panic!("{SKILL}:{}: an unclosed fence", at + 1));
            bodies.push(lines[at + 1..at + 1 + close].join("\n"));
            at += close + 2;
        } else {
            at += 1;
        }
    }
    bodies
}

/// An empty git repository holding the skill's config, committed.
struct Project {
    _scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Project {
    fn new(label: &str, config: &str) -> Self {
        let scratch = Scratch::new(label);
        let git = Sandbox::new(scratch.path());
        let root = scratch.dir("project");
        git.init(&root);
        write(
            &root,
            "specengine.toml",
            format!("[project]\nslug = \"authoring\"\n\n{config}\n"),
        );
        git.add_all(&root);
        git.commit(&root, "the config");
        let home = scratch.home("h");
        Self {
            _scratch: scratch,
            git,
            root,
            home,
        }
    }

    fn session(&self) -> Session {
        Session::open(Era::Stateless, &[], Some(&self.root), Home::At(&self.home))
    }
}

/// The skill's config and its blocks, each a `propose_change` argument object.
fn the_example() -> (String, Vec<Value>) {
    let skill = read_text(&repository_root(), SKILL);
    let configs = fences(&skill, "toml");
    assert_eq!(configs.len(), 1, "one config block: {configs:?}");
    let blocks: Vec<Value> = fences(&skill, "json")
        .iter()
        .map(|body| serde_json::from_str(body).unwrap_or_else(|error| panic!("{body}: {error}")))
        .collect();
    assert_eq!(blocks.len(), 8, "eight blocks");
    let config = configs.into_iter().next().expect("one config");
    (config, blocks)
}

/// The document of an answer that is no error.
fn answered(reply: &Value, context: &str) -> Value {
    let result = result(reply);
    assert_ne!(result["isError"], json!(true), "{context}: {reply}");
    result["structuredContent"].clone()
}

/// The owner's terminal, every key `y`.
#[derive(Default)]
struct Owner {
    asked: usize,
}

impl RoundTerminal for Owner {
    fn report(&mut self, _text: &str) {}

    fn ask(&mut self, _question: &str) -> Option<bool> {
        self.asked += 1;
        Some(true)
    }

    fn tell(&mut self, _line: &str) {}
}

/// The skill's five steps, as an agent and its owner take them.
#[test]
fn the_skills_path_reads_ten_rows_at_zero() {
    let (config, blocks) = the_example();
    let project = Project::new("mfa-path", &config);
    let mut session = project.session();

    // Step 2: the first proposal rehearsed: nothing stored.
    let mut rehearsal = blocks[0].clone();
    rehearsal["dry_run"] = json!(true);
    let document = answered(&session.call("propose_change", rehearsal), "the rehearsal");
    assert_eq!(document["id"], Value::Null, "{document}");
    assert_eq!(document["notes"][0], DRY_NOTE, "{document}");

    // Step 3: one proposal per document, each block verbatim.
    let mut ids = Vec::new();
    for block in &blocks {
        let document = answered(
            &session.call("propose_change", block.clone()),
            block["target"].as_str().expect("a target"),
        );
        ids.push(document["id"].as_str().expect("a stored ID").to_owned());
    }
    assert_eq!(ids.len(), 8);
    assert_eq!(ids[0], "PR-0001", "the rehearsal held no ID");

    // Step 4: the owner's walk.
    let mut owner = Owner::default();
    let walk = approve_open(
        &cli_env(&project.root, Some(&project.home)),
        &Globals::default(),
        &ApproveOpenRequest {
            now: LATER.to_owned(),
            git: GitEnv::new(&project.root, project.git.vars()),
        },
        &mut owner,
    )
    .unwrap_or_else(|error| panic!("approve --open: {error}"));
    assert_eq!(walk.left, Vec::<String>::new(), "{walk:?}");
    assert_eq!(walk.applied.len(), 8, "{walk:?}");
    assert_eq!(owner.asked, 8, "one key each");

    // Step 5: the read.
    let reply = session.call("get_flow", json!({}));
    let document = answered(&reply, "get_flow");
    // docs/features/doc-modal.md DM-3: `titles`, parallel to `items`.
    for (gap, code) in document["gaps"].as_array().expect("gaps").iter().zip(CODES) {
        assert_eq!(
            *gap,
            json!({"code": code, "state": "measured", "count": 0, "items": [], "titles": [],
                "cause": null}),
        );
    }
    assert!(
        content_text(result(&reply)).starts_with("flow all | nodes 8, edges 10, depth -\n"),
        "{reply}"
    );
    drop(session.finish());
}

/// AC-03 where the trial needed it: the rehearsal's own `diagnostics` teach,
/// nothing stored and nothing refused (ADR-0012).
#[test]
fn a_rehearsal_gives_the_teaching_words() {
    let (config, _) = the_example();
    let project = Project::new("mfa-rehearsal", &config);
    let mut session = project.session();
    let args = |target: &str, text: &str| {
        json!({"kind": "create", "target": target, "text": text, "rationale": "rehearse",
            "author_role": "writer", "dry_run": true})
    };

    let text = "---\nid: NODE-a\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nlinks:\n  \
                relates: [NODE-a]\n---\n\n# A\n";
    let document = answered(
        &session.call("propose_change", args("docs/spec/a.md", text)),
        "a link type",
    );
    let found = document.to_string();
    assert!(
        found.contains("kept; the types: derived_from, depends_on, "),
        "{document}"
    );

    let text = "---\nid: TOPIC-one\nclass: canon\nowner: owner\nreviewed: 2026-01-01\n---\n\n\
                # A\n";
    let document = answered(
        &session.call("propose_change", args("docs/spec/b.md", text)),
        "an ID",
    );
    assert!(
        document.to_string().contains("add to [ids]: TOPIC = "),
        "{document}"
    );
    assert_eq!(
        project
            .git
            .git_text(&project.root, &["status", "--porcelain=v1"]),
        ""
    );
    drop(session.finish());
}
