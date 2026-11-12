//! docs/features/upstream-signal.md (RM-0105) the spec's scenario for the MCP
//! and daemon halves (`specengine-mcp`'s and `specengine-http`'s
//! `tests/upstream_signal.rs` include this file by path, beside
//! `common/git.rs` as their `git` module): a scratch git repository holding
//! `docs/tides.md` (`TIDE-BOOK` with `{#TIDE-SPRING}`, `{#TIDE-NEAP}`),
//! `docs/harbor.md` and the record `DEC-0030`, its cards and items written
//! through the CLI library at the scenario's clocks. The CLI half builds the
//! same scene with its own helpers (`tests/upstream_signal.rs`).
//!
//! C-0002 and C-0003 are made by `card new --parent` as the split of
//! the spec's split would make them: a section's kind is its ID prefix's, so a split
//! would target `TIDE-NEAP` beside `TIDE-SPRING`.

#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CardClearOutcome, CardClearRequest, CardEditRequest,
    CardMoveRequest, CardNewRequest, DecisionInput, DecisionRequest, Env, Exit, GitEnv, Globals,
    IntakeOutcome, Mandate, QuestionRequest, approve_with, card_clear, card_edit, card_move,
    card_new, propose_decision, propose_question,
};

use crate::git::Sandbox;

pub const SLUG: &str = "tide-table";
pub const TITLE: &str = "Write the tide table foreword";
pub const T_MADE: &str = "2026-11-12T08:00:00Z";
pub const T_EDIT: &str = "2026-11-12T09:00:00Z";
pub const T_APPLY: &str = "2026-11-12T10:00:00Z";
pub const ANN: &str = "Ann";
pub const OWNER_NAME: &str = "Ann Owner";
pub const OWNER_EMAIL: &str = "ann@example.org";

const CONFIG: &str = r#"[project]
slug = "tide-table"

[paths]
roots = ["docs"]

[ids]
TIDE   = { kind = "tide", shape = "name" }
HARBOR = { kind = "harbor", shape = "name" }
DEC    = { kind = "decision", width = 4 }

[decision_records]
prefix   = "DEC"
dir      = "docs/records/DEC"
template = "templates/decision.md"

[[card.flow]]
name = "desk"
columns = ["backlog", "review", "done"]
accept = { from = "review", to = "done" }

[[card.flow]]
name = "copy"
disciplines = ["copy"]
columns = ["backlog", "review", "done"]
accept = { from = "review", to = "done" }

[[card.flow]]
name = "plates"
disciplines = ["plates"]
columns = ["backlog", "review", "done"]
accept = { from = "review", to = "done" }

[[card.split]]
kind = "tide"
disciplines = ["copy", "plates"]
"#;

const TEMPLATE: &str = "---\nid: {{id}}\nclass: decision\nstatus: {{status}}\ndate: {{date}}\n\
                        title: {{title}}\ncanon: {{canon}}\nlinks:\n  answers: {{targets}}\n\
                        ref: {{proposal}}\nscope: [decisions]\n---\n\n# {{title}}\n\nChoice:\n\n\
                        {{choice}}\n\nEffect:\n\n{{effect}}\n\nCost:\n\n{{cost}}\n\nSummary:\n\n\
                        {{summary}}\n\nOptions:\n\n{{options}}\n\nEvidence:\n\n{{evidence}}\n\n\
                        Note:\n\n{{note}}\n\nDecided by:\n\n{{decided_by}}\n";

const NEAP: &str = "Neap tides run low at the half moon.";
const NEAP_EDITED: &str = "Neap tides run low at the half moon, and lower in March.";

fn tides(neap: &str) -> String {
    format!(
        "---\nid: TIDE-BOOK\nclass: canon\ntitle: The tide book\n---\n\n# The tide book\n\n\
         The tide tables for the year.\n\n## Spring tides {{#TIDE-SPRING}}\n\nSpring tides run \
         high at the full moon.\n\n## Neap tides {{#TIDE-NEAP}}\n\n{neap}\n"
    )
}

const HARBOR: &str = "---\nid: HARBOR-LIGHTS\nclass: canon\ntitle: The harbor lights\n---\n\n\
                      # The harbor lights\n\nThe lights are lit at dusk.\n";

const RECORD_30: &str = "---\nid: DEC-0030\nclass: decision\nstatus: accepted\ndate: \
                         2026-11-01\ntitle: Tables print weekly\nscope: [decisions]\n---\n\n\
                         # Tables print weekly\n\nThe tables print once a week.\n";

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// How far the scenario runs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The three cards, seqs 1 to 3.
    Split,
    /// Then the hand edit of `TIDE-NEAP` alone (US-7's scene).
    Marked,
    /// The whole of the spec's data: the edit (4), the decision applied (9), the
    /// hand edit.
    Data,
}

/// The scratch repository and its `HOME`, removed on drop.
pub struct Scene {
    dir: PathBuf,
    pub git: Sandbox,
    pub root: PathBuf,
    pub home: PathBuf,
}

impl Drop for Scene {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn put(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

impl Scene {
    /// The corpus committed; no queue yet.
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-us-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        let dir = fs::canonicalize(&dir).expect("canonical");
        let git = Sandbox::new(&dir);
        let root = dir.join("repo");
        git.init(&root);
        let root = fs::canonicalize(&root).expect("canonical");
        put(&root, "specengine.toml", CONFIG);
        put(&root, "templates/decision.md", TEMPLATE);
        put(&root, "docs/tides.md", &tides(NEAP));
        put(&root, "docs/harbor.md", HARBOR);
        put(&root, "docs/records/DEC/DEC-0030.md", RECORD_30);
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let scene = Self {
            dir,
            git,
            root,
            home,
        };
        scene.commit("the tide table");
        scene
    }

    /// the spec's scenario up to `stop`.
    pub fn scenario(label: &str, stop: Stop) -> Self {
        let scene = Self::new(label);
        scene.number_items_from_40();
        assert_eq!(scene.parent(T_MADE), "C-0001");
        assert_eq!(scene.subtask("copy", Mandate::Binding), "C-0002");
        assert_eq!(scene.subtask("plates", Mandate::Open), "C-0003");
        assert_eq!(scene.last_seq(), 3);
        match stop {
            Stop::Split => {}
            Stop::Marked => scene.hand_edit_neap(),
            Stop::Data => {
                scene.goal("C-0001", "A foreword for the spring tables.", T_EDIT);
                assert_eq!(scene.last_seq(), 4);
                let decision = scene.decision("TIDE-NEAP", "Neap tables print in bold");
                assert_eq!(decision, "PR-0040");
                scene.move_to("C-0001", "backlog", T_EDIT);
                scene.move_to("C-0001", "backlog", T_EDIT);
                assert_eq!(scene.decide("PR-0040", T_APPLY), "DEC-0031");
                assert_eq!(scene.last_seq(), 9, "the apply is seq 9");
                scene.hand_edit_neap();
            }
        }
        scene
    }

    pub fn commit(&self, message: &str) {
        self.git.add_all(&self.root);
        self.git.commit(&self.root, message);
    }

    pub fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    pub fn globals(&self) -> Globals {
        Globals {
            root: Some(self.root.clone()),
            config: None,
        }
    }

    pub fn git_env(&self) -> GitEnv {
        GitEnv::new(self.root.clone(), self.git.vars())
    }

    fn deciding(&self, now: &str) -> GitEnv {
        let date = format!("{}+0000", now.trim_end_matches('Z'));
        let mut env = self.git_env();
        for (name, value) in [
            ("GIT_AUTHOR_NAME", OWNER_NAME),
            ("GIT_AUTHOR_EMAIL", OWNER_EMAIL),
            ("GIT_AUTHOR_DATE", date.as_str()),
            ("GIT_COMMITTER_NAME", OWNER_NAME),
            ("GIT_COMMITTER_EMAIL", OWNER_EMAIL),
            ("GIT_COMMITTER_DATE", date.as_str()),
        ] {
            env = env.with_var(OsString::from(name), OsString::from(value));
        }
        env
    }

    /// The queue's file, as the host rule places it under `HOME`.
    pub fn db(&self) -> PathBuf {
        let data = if cfg!(target_os = "macos") {
            self.home
                .join("Library")
                .join("Application Support")
                .join("specengine")
        } else {
            self.home.join(".local").join("share").join("specengine")
        };
        data.join(format!("{SLUG}.db"))
    }

    pub fn sql(&self, sql: &str) -> String {
        let client = ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
            .into_iter()
            .find(|candidate| Path::new(candidate).exists())
            .expect("a sqlite3 client");
        let output = Command::new(client)
            .arg(self.db())
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

    pub fn last_seq(&self) -> i64 {
        self.sql("SELECT COALESCE(MAX(seq), 0) FROM events;")
            .parse()
            .expect("a seq")
    }

    pub fn card(&self, request: &CardNewRequest) -> String {
        let outcome = card_new(&self.env(), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("card new {}: {error}", request.title));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    pub fn parent(&self, now: &str) -> String {
        self.card(&CardNewRequest {
            title: TITLE.to_owned(),
            targets: vec!["TIDE-BOOK".to_owned()],
            reviewer: Some(ANN.to_owned()),
            now: now.to_owned(),
            ..CardNewRequest::default()
        })
    }

    pub fn subtask(&self, discipline: &str, mandate: Mandate) -> String {
        self.card(&CardNewRequest {
            title: TITLE.to_owned(),
            discipline: Some(discipline.to_owned()),
            targets: vec!["TIDE-SPRING".to_owned()],
            reviewer: Some(ANN.to_owned()),
            mandate: Some(mandate),
            parent: Some("C-0001".to_owned()),
            now: T_MADE.to_owned(),
            ..CardNewRequest::default()
        })
    }

    /// `card edit id --goal goal --as Ann` at `now`.
    pub fn goal(&self, id: &str, goal: &str, now: &str) {
        let outcome = card_edit(
            &self.env(),
            &Globals::default(),
            &CardEditRequest {
                id: id.to_owned(),
                title: None,
                goal: Some(goal.to_owned()),
                targets: Vec::new(),
                criteria: Vec::new(),
                estimate: None,
                due: None,
                parent: None,
                corrects: None,
                evidence: Vec::new(),
                tasks: Vec::new(),
                mandate: None,
                reason: None,
                revision: None,
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                tasks_here: true,
            },
        )
        .unwrap_or_else(|error| panic!("card edit {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    pub fn move_to(&self, id: &str, column: &str, now: &str) {
        let outcome = card_move(
            &self.env(),
            &Globals::default(),
            &CardMoveRequest {
                id: id.to_owned(),
                to: Some(column.to_owned()),
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                ..CardMoveRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("move {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    pub fn clear(&self, id: &str, now: &str) {
        let mut yes = |_: &str| true;
        let outcome = card_clear(
            &self.env(),
            &Globals::default(),
            &CardClearRequest {
                id: id.to_owned(),
                outcome: CardClearOutcome::NoEffect,
                reason: "Read again.".to_owned(),
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                ..CardClearRequest::default()
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("clear {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    }

    pub fn decision(&self, node: &str, text: &str) -> String {
        let mut request = DecisionRequest {
            input: DecisionInput {
                node_ids: vec![node.to_owned()],
                text: text.to_owned(),
                working_answer: format!("Yes: {text}"),
                price_of_other: "A table reprinted.".to_owned(),
                why: None,
                severity: None,
                proposed_patch: None,
                distinct_from: None,
            },
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: T_EDIT.to_owned(),
            git: self.git_env(),
        };
        let raise = |request: &DecisionRequest| {
            propose_decision(&self.env(), &Globals::default(), request)
                .unwrap_or_else(|error| panic!("decision {text}: {error}"))
        };
        let first = raise(&request);
        if first.document.created {
            return stored(&first);
        }
        request.input.distinct_from = Some(hit_names(&first));
        stored(&raise(&request))
    }

    pub fn question(&self, node: &str, text: &str) -> String {
        let request = QuestionRequest {
            node_ids: vec![node.to_owned()],
            text: text.to_owned(),
            working_answer: "Yes, at once.".to_owned(),
            price_of_other: "A table reprinted.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: T_MADE.to_owned(),
            git: self.git_env(),
        };
        stored(
            &propose_question(&self.env(), &Globals::default(), &request)
                .unwrap_or_else(|error| panic!("question {text}: {error}")),
        )
    }

    /// Approved at `now` as the owner: the record's ID.
    pub fn decide(&self, id: &str, now: &str) -> String {
        let mut yes = |_: &str| true;
        let outcome = approve_with(
            &self.env(),
            &Globals::default(),
            &ApproveRequest {
                id: id.to_owned(),
                note: None,
                now: now.to_owned(),
                git: self.deciding(now),
            },
            &ApproveFlags {
                option: None,
                answer: None,
                canon: None,
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
        let record = self.sql(&format!(
            "SELECT record_id FROM proposals WHERE id = '{id}' AND status = 'applied';"
        ));
        assert!(!record.is_empty(), "{id} applied with a record");
        record
    }

    /// The next item raised is `PR-0040`, and `seq` starts at 1.
    fn number_items_from_40(&self) {
        let id = self.question("HARBOR-LIGHTS", "Are the lights lit at dawn too?");
        assert_eq!(id, "PR-0001");
        self.sql("UPDATE proposals SET id = 'PR-0039' WHERE id = 'PR-0001'; DELETE FROM events;");
    }

    pub fn hand_edit_neap(&self) {
        put(&self.root, "docs/tides.md", &tides(NEAP_EDITED));
        self.commit("neap tides by hand");
    }
}

fn stored(outcome: &IntakeOutcome) -> String {
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

fn hit_names(outcome: &IntakeOutcome) -> Vec<String> {
    outcome
        .document
        .hits
        .iter()
        .map(|hit| hit.name().to_owned())
        .collect()
}

/// the spec's JSON object for C-0002 on the whole scenario.
pub fn data_upstream() -> Value {
    json!({"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK", "correction": null,
     "changes": [
      {"seq": 4, "at": "2026-11-12T09:00:00Z", "source": "edit", "by": "Ann", "fields": ["goal"],
       "item": null, "kind": null, "record": null, "on": null},
      {"seq": 9, "at": "2026-11-12T10:00:00Z", "source": "item", "by": "Ann Owner", "fields": [],
       "item": "PR-0040", "kind": "decision", "record": "DEC-0031", "on": "TIDE-NEAP"}],
     "more": 0})
}

/// The parent's mark alone (US-7's scene).
pub fn marked_upstream() -> Value {
    json!({"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK", "correction": null,
           "changes": [], "more": 0})
}
