//! docs/features/delivery-analytics.md (RM-0107 a), the scenario every
//! delivery test reads ("the scenario" of its acceptance criteria): a
//! scratch git repository holding the plan items `RM-0001` (release `r1`)
//! and `RM-0002` (release `r2`), the flows `copy`, `plates` and `sound`,
//! the departments `words` and `print`, and its five cards and one item
//! written through the CLI library at the scenario's clocks, every time
//! `T09:00:00Z`. `specengine-http`'s `tests/delivery.rs` and
//! `tests/daemon_keys.rs` include this file by path, beside
//! `common/git.rs` as their `git` module.
//!
//! The cards are made in ID order, so `seq` order is not the order of their
//! `created_at` (C-0004 is made after C-0003, two days earlier): the read
//! orders by `seq`, never by `at`.
//!
//! Every name here is invented; the people `Ann` and `Bob` are the
//! assignee and the reviewer no served key may name.

#![allow(dead_code)]

mod data;

// Each including crate uses its own part of the pinned data.
#[allow(unused_imports)]
pub use data::{DATA_JSON, DATA_TEXT, TIMELINE_JSON, TIMELINE_TEXT, compact, data_timeline_json};

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use specengine_cli::{
    ApproveFlags, ApproveRequest, CardAcceptRequest, CardCancelRequest, CardEditRequest,
    CardMoveRequest, CardNewRequest, Env, Exit, GitEnv, Globals, QuestionRequest, approve_with,
    card_accept, card_cancel, card_edit, card_move, card_new, propose_question,
};

use crate::git::Sandbox;

pub const SLUG: &str = "tide-almanac";
pub const AS_OF: &str = "2026-11-20";
pub const ANN: &str = "Ann";
pub const BOB: &str = "Bob";
pub const OWNER_NAME: &str = "Cora Owner";
pub const OWNER_EMAIL: &str = "cora@example.org";

pub const HEAD: &str = r#"[project]
slug = "tide-almanac"

[paths]
roots = ["docs"]

[ids]
RM  = { kind = "plan-item", width = 4 }
DEC = { kind = "decision", width = 4 }

[decision_records]
prefix   = "DEC"
dir      = "docs/records/DEC"
template = "templates/decision.md"
"#;

pub const ROADMAP: &str = r#"
[roadmap]
prefix     = "RM"
dir        = "docs/roadmap"
releases   = ["r1", "r2"]
priorities = ["must", "could"]
"#;

pub const FLOWS: &str = r#"
[[card.flow]]
name = "copy"
disciplines = ["copy"]
columns = ["todo", "doing", "review", "returned", "done"]
accept = { from = "review", to = "done" }
return = "returned"

[[card.flow]]
name = "plates"
disciplines = ["plates"]
columns = ["todo", "drawing", "done"]
accept = { from = "drawing", to = "done" }

[[card.flow]]
name = "sound"
disciplines = ["sound"]
columns = ["todo", "done"]
accept = { from = "todo", to = "done" }
"#;

pub const DEPARTMENTS: &str = r#"
[[workspace]]
name = "words"
title = "Words"
flows = ["copy"]

[[workspace]]
name = "print"
title = "Print"
flows = ["copy", "plates"]
"#;

/// The scenario's whole config.
pub fn config() -> String {
    format!("{HEAD}{ROADMAP}{FLOWS}{DEPARTMENTS}")
}

const TEMPLATE: &str = "---\nid: {{id}}\nclass: decision\nstatus: {{status}}\ndate: {{date}}\n\
                        title: {{title}}\ncanon: {{canon}}\nlinks:\n  answers: {{targets}}\n\
                        ref: {{proposal}}\nscope: [decisions]\n---\n\n# {{title}}\n\nChoice:\n\n\
                        {{choice}}\n\nEffect:\n\n{{effect}}\n\nCost:\n\n{{cost}}\n\nSummary:\n\n\
                        {{summary}}\n\nOptions:\n\n{{options}}\n\nEvidence:\n\n{{evidence}}\n\n\
                        Note:\n\n{{note}}\n\nDecided by:\n\n{{decided_by}}\n";

/// A plan item's document: `release` `None` is the pool.
pub fn item(id: &str, title: &str, release: Option<&str>, status: &str) -> String {
    let release = release.map_or(String::new(), |name| format!("release: {name}\n"));
    format!(
        "---\nclass: spec\nid: {id}\nstatus: {status}\nscope: [almanac]\n{release}priority: \
         must\n---\n\n# {title}\n\nThe goal of {title}.\n"
    )
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// `YYYY-MM-DD` of November 2026 at the scenario's hour.
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

impl Scene {
    /// `config` and the two plan items committed; no queue yet.
    pub fn with_config(label: &str, config: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "specengine-da-{label}-{}-{}",
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
            &item("RM-0001", "The spring almanac", Some("r1"), "draft"),
        );
        put(
            &root,
            "docs/roadmap/RM-0002.md",
            &item("RM-0002", "The harbor charts", Some("r2"), "draft"),
        );
        let home = dir.join("h");
        fs::create_dir_all(&home).expect("a home");
        let scene = Self {
            dir,
            git,
            root,
            home,
        };
        scene.commit("the almanac");
        scene
    }

    /// The scenario's config; no card yet.
    pub fn new(label: &str) -> Self {
        Self::with_config(label, &config())
    }

    /// The whole of the spec's scenario: C-0001 to C-0005 and PR-0040.
    pub fn scenario(label: &str) -> Self {
        let scene = Self::new(label);
        scene.play(&Extra::default());
        scene
    }

    /// The scenario with `extra` woven in, on the scenario's config.
    pub fn scenario_with(label: &str, extra: &Extra) -> Self {
        let scene = Self::new(label);
        scene.play(extra);
        scene
    }

    /// The scenario's writes, in `seq` order, `extra`'s at their places.
    pub fn play(&self, extra: &Extra) {
        self.number_items_from_40();
        let c1 = self.card(&CardNewRequest {
            targets: vec!["RM-0001".to_owned()],
            assignee: Some(ANN.to_owned()),
            reviewer: Some(BOB.to_owned()),
            estimate: Some("3".to_owned()),
            due: Some("2026-11-10".to_owned()),
            actor: Some(ANN.to_owned()),
            ..new_card("Write the tide table foreword", "copy", 1)
        });
        assert_eq!(c1, "C-0001");
        let c2 = self.card(&CardNewRequest {
            parent: Some("C-0001".to_owned()),
            ..new_card("Draw the tide plates", "plates", 2)
        });
        assert_eq!(c2, "C-0002");
        let c3 = self.card(&CardNewRequest {
            targets: vec!["RM-0002".to_owned()],
            due: Some("2026-11-12".to_owned()),
            ..new_card("Engrave the harbor map", "plates", 4)
        });
        assert_eq!(c3, "C-0003");
        let c4 = self.card(&CardNewRequest {
            reviewer: Some(BOB.to_owned()),
            ..new_card("Fix the index", "copy", 2)
        });
        assert_eq!(c4, "C-0004");
        let c5 = self.card(&new_card("Record the foghorn", "sound", 6));
        assert_eq!(c5, "C-0005");
        self.move_to("C-0001", "todo", None, &at(1));
        self.move_to("C-0001", "todo", None, &at(2));
        for _ in 0..extra.reorders_before_take {
            self.move_to("C-0001", "todo", None, &at(2));
        }
        self.move_to("C-0001", "doing", None, &at(3));
        for _ in 0..extra.reorders_after_take {
            self.move_to("C-0001", "doing", None, &at(4));
        }
        self.move_to("C-0004", "review", None, &at(3));
        self.accept("C-0004", &at(5));
        self.move_to("C-0003", "drawing", None, &at(5));
        self.move_to("C-0001", "review", extra.forward_note, &at(5));
        self.move_to("C-0001", "returned", Some("Too long"), &at(6));
        if let Some(note) = extra.reorder_in_return {
            self.move_to("C-0001", "returned", Some(note), &at(6));
        }
        self.reestimate("C-0001", Some("5"), "The tables doubled", &at(7));
        self.move_to("C-0001", "review", None, &at(7));
        self.move_to("C-0001", "returned", Some("Wrong tone"), &at(8));
        self.cancel("C-0002", "Plates reused from last year", &at(9));
        self.move_to("C-0001", "review", None, &at(10));
        let asked = self.question(
            "RM-0001",
            "Does the foreword name the tide tables?",
            Some("C-0001"),
        );
        assert_eq!(asked, "PR-0040");
        self.decide("PR-0040", &at(12));
        self.accept("C-0001", &at(15));
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

    /// `card edit id --estimate E --reason T --as Ann` at `now`; `None`
    /// clears it (`-`).
    pub fn reestimate(&self, id: &str, estimate: Option<&str>, reason: &str, now: &str) {
        let outcome = card_edit(
            &self.env(),
            &Globals::default(),
            &CardEditRequest {
                id: id.to_owned(),
                title: None,
                goal: None,
                targets: Vec::new(),
                criteria: Vec::new(),
                estimate: Some(estimate.unwrap_or("-").to_owned()),
                due: None,
                parent: None,
                corrects: None,
                evidence: Vec::new(),
                tasks: Vec::new(),
                mandate: None,
                reason: Some(reason.to_owned()),
                revision: None,
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                tasks_here: true,
            },
        )
        .unwrap_or_else(|error| panic!("reestimate {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "reestimate {id}: {outcome:?}"
        );
    }

    /// A question on `node`, asked for `card` if any: its ID.
    pub fn question(&self, node: &str, text: &str, card: Option<&str>) -> String {
        let request = QuestionRequest {
            node_ids: vec![node.to_owned()],
            text: text.to_owned(),
            working_answer: "Yes, each one by its harbor.".to_owned(),
            price_of_other: "A page reprinted.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: card.map(str::to_owned),
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: at(10),
            git: self.git_env(),
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
        };
        let outcome = propose_question(&self.env(), &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("question {text}: {error}"));
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

    /// The next item raised is `PR-0040`, and `seq` starts at 1: one
    /// question with no card, renumbered `PR-0039`, its event dropped.
    fn number_items_from_40(&self) {
        let request = QuestionRequest {
            node_ids: vec!["RM-0002".to_owned()],
            text: "Are the harbor charts drawn to one scale?".to_owned(),
            working_answer: "Yes, one scale.".to_owned(),
            price_of_other: "A chart redrawn.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            card: None,
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: at(1),
            git: self.git_env(),
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
        };
        let outcome = propose_question(&self.env(), &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("the first question: {error}"));
        assert_eq!(
            outcome.document.id.as_deref(),
            Some("PR-0001"),
            "{outcome:?}"
        );
        self.sql("UPDATE proposals SET id = 'PR-0039' WHERE id = 'PR-0001'; DELETE FROM events;");
    }
}

/// What a variant weaves into the scenario; the default adds nothing.
#[derive(Default)]
pub struct Extra {
    /// More reorders of C-0001 in `todo`, before its take (DA-1).
    pub reorders_before_take: usize,
    /// Reorders of C-0001 in `doing`, after its take (DA-1: the take then
    /// falls outside its last 64 `card.*` events).
    pub reorders_after_take: usize,
    /// A note on C-0001's move from `doing` to `review` (DA-8: no return).
    pub forward_note: Option<&'static str>,
    /// A reorder of C-0001 inside `returned`, with this note (DA-8: no
    /// return).
    pub reorder_in_return: Option<&'static str>,
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
