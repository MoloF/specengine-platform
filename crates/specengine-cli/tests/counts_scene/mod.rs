//! docs/features/department-overview.md (RM-0086), the scenario every
//! counts test reads ("the scenario" of its acceptance criteria): a scratch
//! git repository holding the plan items `RM-0001` and `RM-0002`, the flows
//! `copy` "Copy", `plates` "Plates" and `sound` "Sound", the departments
//! `words`, `print` and `press`, and its seven cards, four items and two
//! agent tasks written through the CLI library at the scenario's clocks,
//! every time `T09:00:00Z`. `specengine-http`'s `tests/card_counts.rs` and
//! `tests/daemon_keys.rs` include this file by path, beside `common/git.rs`
//! as their `git` module.
//!
//! C-0007 stands in `copy`/`proofing`: the config lists `proofing` while
//! the card is moved there, then a second commit drops it, so the column is
//! one "the config no longer lists".
//!
//! Every name here is invented.

#![allow(dead_code)]

mod data;

// Each including crate uses its own part of the pinned data.
#[allow(unused_imports)]
pub use data::{DATA_JSON, DATA_TEXT, compact, with_flows};

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_cli::{
    ApproveFlags, ApproveRequest, CardAcceptRequest, CardCancelRequest, CardEditRequest,
    CardMoveRequest, CardNewRequest, DecisionRequest, Env, Exit, GitEnv, Globals, IntakeSource,
    ProposedText, QuestionRequest, TaskDecisionRequest, TaskNewRequest, TaskPlanRequest,
    approve_with, card_accept, card_cancel, card_edit, card_move, card_new, propose_decision,
    propose_question, read_decision_input, task_approve, task_new, task_plan,
};

use crate::git::Sandbox;

pub const SLUG: &str = "atlas-bindery";
pub const ANN: &str = "Ann";
pub const BOB: &str = "Bob";
pub const OWNER_NAME: &str = "Cora Owner";
pub const OWNER_EMAIL: &str = "cora@example.org";

pub const HEAD: &str = r#"[project]
slug = "atlas-bindery"

[paths]
roots = ["docs"]

[ids]
RM  = { kind = "plan-item", width = 4 }
DEC = { kind = "decision", width = 4 }

[decision_records]
prefix   = "DEC"
dir      = "docs/records/DEC"
template = "templates/decision.md"

[roadmap]
prefix     = "RM"
dir        = "docs/roadmap"
releases   = ["r1", "r2"]
priorities = ["must", "could"]
"#;

/// The flow `copy` as "Data" configures it.
pub const COPY: &str = r#"
[[card.flow]]
name = "copy"
title = "Copy"
disciplines = ["copy"]
columns = ["todo", "doing", "review", "returned", "done"]
accept = { from = "review", to = "done" }
return = "returned"
"#;

/// `copy` while C-0007 is moved: `proofing` listed.
pub const COPY_PROOFING: &str = r#"
[[card.flow]]
name = "copy"
title = "Copy"
disciplines = ["copy"]
columns = ["todo", "doing", "review", "returned", "proofing", "done"]
accept = { from = "review", to = "done" }
return = "returned"
"#;

pub const PLATES: &str = r#"
[[card.flow]]
name = "plates"
title = "Plates"
disciplines = ["plates"]
columns = ["todo", "drawing", "done"]
accept = { from = "drawing", to = "done" }
"#;

pub const SOUND: &str = r#"
[[card.flow]]
name = "sound"
title = "Sound"
disciplines = ["sound"]
columns = ["todo", "check", "done"]
accept = { from = "check", to = "done" }
"#;

pub const DEPARTMENTS: &str = r#"
[[workspace]]
name = "words"
title = "Words"
sections = ["flow", "board", "questions", "inbox"]
flows = ["copy"]

[[workspace]]
name = "print"
title = "Print"
sections = ["board", "health", "inbox"]
flows = ["copy", "plates"]

[[workspace]]
name = "press"
title = "Press room"
sections = ["inbox", "board", "roadmap", "flow"]
"#;

/// The scenario's whole config, as it stands once C-0007 is in `proofing`.
pub fn config() -> String {
    format!("{HEAD}{COPY}{PLATES}{SOUND}{DEPARTMENTS}")
}

/// The config while C-0007 is moved.
pub fn config_proofing() -> String {
    format!("{HEAD}{COPY_PROOFING}{PLATES}{SOUND}{DEPARTMENTS}")
}

const TEMPLATE: &str = "---\nid: {{id}}\nclass: decision\nstatus: {{status}}\ndate: {{date}}\n\
                        title: {{title}}\ncanon: {{canon}}\nlinks:\n  answers: {{targets}}\n\
                        ref: {{proposal}}\nscope: [decisions]\n---\n\n# {{title}}\n\nChoice:\n\n\
                        {{choice}}\n\nEffect:\n\n{{effect}}\n\nCost:\n\n{{cost}}\n\nSummary:\n\n\
                        {{summary}}\n\nOptions:\n\n{{options}}\n\nEvidence:\n\n{{evidence}}\n\n\
                        Note:\n\n{{note}}\n\nDecided by:\n\n{{decided_by}}\n";

/// A plan item's document.
pub fn item(id: &str, title: &str, release: &str) -> String {
    format!(
        "---\nclass: spec\nid: {id}\nstatus: draft\nscope: [atlas]\nrelease: {release}\npriority: \
         must\n---\n\n# {title}\n\nThe goal of {title}.\n"
    )
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// November 2026's `day` at the scenario's hour.
pub fn at(day: u32) -> String {
    format!("2026-11-{day:02}T09:00:00Z")
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

pub fn put(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

/// A new card's request: the title and discipline, made at `day`.
pub fn new_card(title: &str, discipline: &str, day: u32) -> CardNewRequest {
    CardNewRequest {
        title: title.to_owned(),
        discipline: Some(discipline.to_owned()),
        now: at(day),
        ..CardNewRequest::default()
    }
}

/// An edit of `id` at `now` as `Ann` that changes nothing yet.
pub fn edit(id: &str, now: &str) -> CardEditRequest {
    CardEditRequest {
        id: id.to_owned(),
        title: None,
        goal: None,
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
    }
}

impl Scene {
    /// `config` and the two plan items committed; no queue yet.
    pub fn with_config(label: &str, config: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-do-{label}-{}-{}",
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
        put(&root, "specengine.toml", config);
        put(&root, "templates/decision.md", TEMPLATE);
        put(
            &root,
            "docs/roadmap/RM-0001.md",
            &item("RM-0001", "The coastal atlas", "r1"),
        );
        put(
            &root,
            "docs/roadmap/RM-0002.md",
            &item("RM-0002", "The bell recordings", "r2"),
        );
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let scene = Self {
            dir,
            git,
            root,
            home,
        };
        scene.commit("the atlas");
        scene
    }

    /// The scenario's config; no card yet.
    pub fn new(label: &str) -> Self {
        Self::with_config(label, &config())
    }

    /// The whole of the spec's scenario: C-0001 to C-0007, PR-0001 to
    /// PR-0004, T-0001 and T-0002; the config committed as "Data" gives it.
    pub fn scenario(label: &str) -> Self {
        let scene = Self::with_config(label, &config_proofing());
        scene.play();
        scene.set_config(&config());
        scene
    }

    /// The scenario's writes, in order.
    pub fn play(&self) {
        let c1 = self.card(&CardNewRequest {
            targets: vec!["RM-0001".to_owned()],
            reviewer: Some(BOB.to_owned()),
            ..new_card("Write the foreword", "copy", 1)
        });
        assert_eq!(c1, "C-0001");
        let c2 = self.card(&CardNewRequest {
            parent: Some("C-0001".to_owned()),
            reviewer: Some(BOB.to_owned()),
            ..new_card("Draw the plates", "plates", 1)
        });
        assert_eq!(c2, "C-0002");
        let c3 = self.card(&CardNewRequest {
            parent: Some("C-0001".to_owned()),
            ..new_card("Cut the index", "copy", 1)
        });
        assert_eq!(c3, "C-0003");
        let c4 = self.card(&new_card("Proof the map", "copy", 1));
        assert_eq!(c4, "C-0004");
        let c5 = self.card(&CardNewRequest {
            targets: vec!["RM-0001".to_owned()],
            ..new_card("Print the atlas", "plates", 1)
        });
        assert_eq!(c5, "C-0005");
        let c6 = self.card(&CardNewRequest {
            targets: vec!["RM-0002".to_owned()],
            ..new_card("Record the bell", "sound", 1)
        });
        assert_eq!(c6, "C-0006");
        let c7 = self.card(&new_card("Set the type", "copy", 1));
        assert_eq!(c7, "C-0007");

        self.move_to("C-0001", "doing", None, &at(2));
        self.move_to("C-0001", "review", None, &at(2));
        self.move_to("C-0002", "drawing", None, &at(3));
        self.accept("C-0002", &at(3));
        self.cancel("C-0003", "The index moves to the back", &at(3));
        self.move_to("C-0004", "review", None, &at(4));
        self.move_to(
            "C-0004",
            "returned",
            Some("The coast is mislabelled"),
            &at(4),
        );
        self.edited(&CardEditRequest {
            parent: Some("C-0005".to_owned()),
            ..edit("C-0004", &at(4))
        });
        self.move_to("C-0007", "proofing", None, &at(4));

        let t1 = self.task("RM-0001", "Typeset the foreword", &at(5));
        assert_eq!(t1, "T-0001");
        let t2 = self.task("RM-0001", "Ink the plates", &at(5));
        assert_eq!(t2, "T-0002");
        self.plan(&t1, &at(5));
        self.plan(&t2, &at(5));
        self.approve_task(&t2, &at(6));
        self.edited(&CardEditRequest {
            tasks: vec![t1.clone()],
            ..edit("C-0001", &at(6))
        });
        self.edited(&CardEditRequest {
            tasks: vec![t1, t2],
            ..edit("C-0005", &at(6))
        });

        let q1 = self.question(
            "RM-0001",
            "Does the foreword thank the surveyors?",
            "C-0001",
            &at(2),
        );
        assert_eq!(q1, "PR-0001");
        let q2 = self.question(
            "RM-0002",
            "Which harbor names does the map keep?",
            "C-0004",
            &at(5),
        );
        assert_eq!(q2, "PR-0002");
        let d3 = self.decision("RM-0001", "Is the foreword set in a serif face?", "C-0001");
        assert_eq!(d3, "PR-0003");
        self.decide("PR-0003", &at(6));
        let q4 = self.question(
            "RM-0002",
            "Is the old index kept as an appendix?",
            "C-0003",
            &at(7),
        );
        assert_eq!(q4, "PR-0004");
    }

    pub fn commit(&self, message: &str) {
        self.git.add_all(&self.root);
        self.git.commit(&self.root, message);
    }

    /// `specengine.toml` replaced by `config` and committed.
    pub fn set_config(&self, config: &str) {
        put(&self.root, "specengine.toml", config);
        self.commit("the config");
    }

    pub fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
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
        db_under(&self.home)
    }

    pub fn sql(&self, sql: &str) -> String {
        sql_at(&self.db(), sql)
    }

    pub fn card(&self, request: &CardNewRequest) -> String {
        let outcome = card_new(&self.env(), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("card new {}: {error}", request.title));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    /// `card move id --to column [--note note]` at `now`, as `Ann`.
    pub fn move_to(&self, id: &str, column: &str, note: Option<&str>, now: &str) {
        let outcome = card_move(
            &self.env(),
            &Globals::default(),
            &CardMoveRequest {
                id: id.to_owned(),
                to: Some(column.to_owned()),
                note: note.map(str::to_owned),
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                ..CardMoveRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("move {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "move {id} to {column}: {outcome:?}"
        );
    }

    /// `card accept id --as Bob --briefed` at `now`.
    pub fn accept(&self, id: &str, now: &str) {
        let mut yes = |_: &str| true;
        let outcome = card_accept(
            &self.env(),
            &Globals::default(),
            &CardAcceptRequest {
                id: id.to_owned(),
                actor: BOB.to_owned(),
                briefed: true,
                now: now.to_owned(),
                ..CardAcceptRequest::default()
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("accept {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "accept {id}: {outcome:?}");
    }

    /// `card cancel id --reason reason --as Ann` at `now`.
    pub fn cancel(&self, id: &str, reason: &str, now: &str) {
        let outcome = card_cancel(
            &self.env(),
            &Globals::default(),
            &CardCancelRequest {
                id: id.to_owned(),
                reason: reason.to_owned(),
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                ..CardCancelRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("cancel {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "cancel {id}: {outcome:?}");
    }

    /// `card edit` as given; it must answer.
    pub fn edited(&self, request: &CardEditRequest) {
        let outcome = card_edit(&self.env(), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("edit {}: {error}", request.id));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "edit {}: {outcome:?}",
            request.id
        );
    }

    /// A `draft` agent task on `node` titled `title`: its ID.
    pub fn task(&self, node: &str, title: &str, now: &str) -> String {
        let outcome = task_new(
            &self.env(),
            &Globals::default(),
            &TaskNewRequest {
                nodes: vec![node.to_owned()],
                title: Some(title.to_owned()),
                goal: None,
                author_role: None,
                author_model: None,
                run: None,
                now: now.to_owned(),
                git: self.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("task new {title}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a task's ID")
    }

    /// The task's plan submitted: `review`.
    pub fn plan(&self, id: &str, now: &str) {
        let outcome = task_plan(
            &self.env(),
            &Globals::default(),
            &TaskPlanRequest {
                id: id.to_owned(),
                plan: ProposedText::Given(b"1. Read.\n2. Set.\n".to_vec()),
                criteria: vec!["The page reads.".to_owned()],
                affected: Vec::new(),
                now: now.to_owned(),
                git: self.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("task plan {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "plan {id}: {outcome:?}");
    }

    /// The owner approves the task's plan: `ready`.
    pub fn approve_task(&self, id: &str, now: &str) {
        let mut yes = |_: &str| true;
        let outcome = task_approve(
            &self.env(),
            &Globals::default(),
            &TaskDecisionRequest {
                id: id.to_owned(),
                note: None,
                now: now.to_owned(),
                git: self.deciding(now),
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("task approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
    }

    /// A question on `node` for `card`, made at `now`: its ID.
    pub fn question(&self, node: &str, text: &str, card: &str, now: &str) -> String {
        let request = QuestionRequest {
            node_ids: vec![node.to_owned()],
            text: text.to_owned(),
            working_answer: "Yes, as the first edition did.".to_owned(),
            price_of_other: "A page reprinted.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: Some(card.to_owned()),
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: now.to_owned(),
            git: self.git_env(),
        };
        let outcome = propose_question(&self.env(), &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("question {text}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        assert!(outcome.document.created, "not stored: {outcome:?}");
        outcome.document.id.clone().expect("an ID")
    }

    /// A decision on `node` for `card`, made at 2026-11-06: its ID.
    pub fn decision(&self, node: &str, text: &str, card: &str) -> String {
        let input = serde_json::json!({
            "node_ids": [node], "text": text,
            "working_answer": "Yes, the house serif.",
            "price_of_other": "The foreword is reset."
        })
        .to_string();
        let input = read_decision_input(&self.env(), &IntakeSource::Given(input.into_bytes()))
            .unwrap_or_else(|error| panic!("decision input: {error}"));
        let outcome = propose_decision(
            &self.env(),
            &Globals::default(),
            &DecisionRequest {
                input,
                card: Some(card.to_owned()),
                author_role: Some("developer".to_owned()),
                author_model: Some("m".to_owned()),
                run: None,
                login: None,
                now: at(6),
                git: self.git_env(),
            },
        )
        .unwrap_or_else(|error| panic!("decision {text}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        assert!(outcome.document.created, "not stored: {outcome:?}");
        outcome.document.id.clone().expect("an ID")
    }

    /// Approved at `now` as the owner: the item `applied`.
    pub fn decide(&self, id: &str, now: &str) {
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
        assert_eq!(
            self.sql(&format!("SELECT status FROM proposals WHERE id = '{id}';")),
            "applied"
        );
    }
}

/// The queue's file under `home`, as the host rule places it.
pub fn db_under(home: &Path) -> PathBuf {
    let data = if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join("specengine")
    } else {
        home.join(".local").join("share").join("specengine")
    };
    data.join(format!("{SLUG}.db"))
}

/// `sqlite3 db sql`, which must answer.
pub fn sql_at(db: &Path, sql: &str) -> String {
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
