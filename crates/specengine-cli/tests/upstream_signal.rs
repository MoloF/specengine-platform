//! docs/features/upstream-signal.md (RM-0105) "Acceptance criteria", the
//! CLI half: the `spec` binary over a scratch git repository built at run
//! time (an invented tide table: `docs/tides.md` holding `TIDE-BOOK` with
//! `{#TIDE-SPRING}` and `{#TIDE-NEAP}`, `docs/harbor.md` holding
//! `HARBOR-LIGHTS`, the record `DEC-0030` so the next is `DEC-0031`), every
//! card and item written through the library at fixed clocks, every
//! decision approved as `Ann Owner <ann@example.org>`.
//!
//! - US-1: `seq` orders, never clocks, through a real split. M: `at`
//!   compared instead of `seq`.
//! - US-2: only the parent's goal, targets and criteria edits count; its
//!   title, due, re-estimate, evidence, tasks, assign, move, reorder and
//!   mandate edit say nothing. M: any `card.edited` counted; `fields`
//!   unfiltered.
//! - US-3: applied questions, discrepancies and decisions on the reach; not
//!   open, rejected or staged ones, one off the reach, the child's own, one
//!   applied before the anchor, an applied update. M: open items counted;
//!   the reach without `within`; the child's own counted; the applied `seq`
//!   ignored.
//! - US-4: `spec trace`, the list's mark keys, `allowed` and `mark`
//!   byte-equal before the parent's edit and after the decision's apply. M:
//!   the signal folded into `card_mark`.
//! - US-5 (the CLI's part): uncapped past 140 reorders; twenty edits list
//!   the newest 16 and `upstream: more 4`. M: `card_history(…, 64)`; no
//!   cap; the oldest kept.
//! - US-6: how it ends (the subtask's own edits, accept, cancel, a
//!   re-parent). M: the child's edits ignored; any child edit ending it;
//!   the anchor from `card.created` alone.
//! - US-7: the parent's mark: suspect, cleared, correcting, departed. M:
//!   the mark source dropped; `departed` counted; the mark ended by the
//!   child's edit.
//! - US-8: the spec's text and JSON byte for byte on the scenario; `null` and
//!   `upstream: -` without `--marks`; list rows; an unreadable parent row;
//!   no `card.created` event. M: `upstream` read without `--marks`.
//!
//! The scenario's cards C-0002 and C-0003 are made by `card new --parent`,
//! not by `card split`: a section's kind is its ID prefix's, so a
//! `[[card.split]]` row naming `TIDE-SPRING`'s kind names `TIDE-NEAP`'s
//! too, and a split would target both (US-1 runs a real split, where the
//! targets do not matter). The item IDs `PR-0040` and the record
//! `DEC-0031` of the spec's data are reached by renumbering one open item to
//! `PR-0039` and the record `DEC-0030` in the corpus; the queue's events
//! are cleared once before the first card, so `seq` starts at 1.
//!
//! US-9's counts are `specengine-cli`'s `#[cfg(test)]` module; US-10 to
//! US-12 are `specengine-mcp`'s and `specengine-http`'s
//! `tests/upstream_signal.rs`. Every expected value is worked out from the
//! spec, never read back from a run of the read under test.

#![cfg(unix)]

mod common;

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::git::Sandbox;
use common::proposal::sqlite3;
use common::{Run, Scratch, data_dir, spec_with, write};
use serde_json::{Value, json};
use specengine_cli::{
    ApproveFlags, ApproveRequest, CardAcceptRequest, CardAssignRequest, CardCancelRequest,
    CardClearOutcome, CardClearRequest, CardEditRequest, CardMoveRequest, CardNewRequest,
    CardSplitRequest, DecisionInput, DecisionRequest, DiscrepancyInput, DiscrepancyRequest, Env,
    Exit, Globals, IntakeOutcome, Mandate, QuestionRequest, RejectRequest, StageBody, StageRequest,
    approve_with, card_accept, card_assign, card_cancel, card_clear, card_edit, card_move,
    card_new, card_split, propose_decision, propose_discrepancy, propose_question, reject, stage,
};
use specengine_store::{GitEnv, ProposalQueue as _, SqliteQueue};

// ------------------------------------------------------------- the scene

const SLUG: &str = "tide-table";

/// The parent's title, which a subtask takes.
const TITLE: &str = "Write the tide table foreword";

/// The clocks of the scenario: the cards, the parent's goal edit (seq 4),
/// the decision's apply (seq 9).
const T_MADE: &str = "2026-11-12T08:00:00Z";
const T_EDIT: &str = "2026-11-12T09:00:00Z";
const T_APPLY: &str = "2026-11-12T10:00:00Z";
const T_LATE: &str = "2026-11-12T11:00:00Z";

/// The parent's editor (an `as`), and the reviewer of every card.
const ANN: &str = "Ann";

/// The owner who decides: `decided_by` `Ann Owner <ann@example.org>`.
const OWNER_NAME: &str = "Ann Owner";
const OWNER_EMAIL: &str = "ann@example.org";

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

/// A record template outside the walk.
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

/// A scratch repository and its data directory's `HOME`.
struct Scene {
    scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
    commits: Cell<usize>,
}

impl Scene {
    /// The corpus committed on `main`; no queue yet.
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(&format!("us-{label}"));
        let git = Sandbox::new(scratch.path());
        let root = scratch.dir("repo");
        git.init(&root);
        write(&root, "specengine.toml", CONFIG);
        write(&root, "templates/decision.md", TEMPLATE);
        write(&root, "docs/tides.md", tides(NEAP));
        write(&root, "docs/harbor.md", HARBOR);
        write(&root, "docs/records/DEC/DEC-0030.md", RECORD_30);
        let home = scratch.home("h");
        let scene = Self {
            scratch,
            git,
            root,
            home,
            commits: Cell::new(0),
        };
        scene.commit("the tide table");
        scene
    }

    fn commit(&self, message: &str) {
        self.commits.set(self.commits.get() + 1);
        self.git.add_all(&self.root);
        self.git.commit(&self.root, message);
    }

    fn env(&self) -> Env {
        Env {
            cwd: self.root.clone(),
            home: Some(self.home.clone().into_os_string()),
            xdg_data_home: None,
        }
    }

    fn git_env(&self) -> GitEnv {
        GitEnv::new(self.root.clone(), self.git.vars())
    }

    /// Git committing as the owner at `now` (author and committer).
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

    /// `spec args` in the root, `HOME` the scene's.
    fn spec(&self, args: &[&str]) -> Run {
        self.spec_in(&self.root, &self.home, args)
    }

    fn spec_in(&self, cwd: &Path, home: &Path, args: &[&str]) -> Run {
        let mut vars: Vec<(String, OsString)> = self
            .git
            .vars()
            .into_iter()
            .map(|(name, value)| (name.to_string_lossy().into_owned(), value))
            .filter(|(name, _)| name != "HOME")
            .collect();
        vars.push(("HOME".to_owned(), home.as_os_str().to_owned()));
        let env: Vec<(&str, &OsStr)> = vars
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
            .collect();
        spec_with(cwd, args, &env)
    }

    /// Exit 0: stdout.
    fn ok(&self, args: &[&str]) -> String {
        let run = self.spec(args);
        run.code(0);
        run.stdout
    }

    /// `spec --json args`, exit 0: the document.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let run = self.spec(&all);
        run.code(0);
        run.json()
    }

    fn db(&self) -> PathBuf {
        data_dir(&self.home).join(format!("{SLUG}.db"))
    }

    fn queue(&self) -> SqliteQueue {
        SqliteQueue::open(self.db(), SLUG).expect("the queue opens")
    }

    fn sql(&self, sql: &str) -> String {
        let output = std::process::Command::new(sqlite3())
            .arg(self.db())
            .arg(sql)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("sqlite3 runs");
        assert!(
            output.status.success(),
            "sqlite3 {sql}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// The highest `seq` stored.
    fn last_seq(&self) -> i64 {
        self.sql("SELECT COALESCE(MAX(seq), 0) FROM events;")
            .parse()
            .expect("a seq")
    }

    // ------------------------------------------------------- card writes

    /// `card new`: its ID.
    fn card(&self, request: CardNewRequest) -> String {
        let outcome = card_new(&self.env(), &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("card new {}: {error}", request.title));
        assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
        outcome.id.expect("a card's ID")
    }

    /// C-0001 of the scenario: on `TIDE-BOOK`, reviewed by [`ANN`], in the
    /// `desk` flow (no discipline), made at `now`.
    fn parent(&self, now: &str) -> String {
        self.card(CardNewRequest {
            title: TITLE.to_owned(),
            targets: vec!["TIDE-BOOK".to_owned()],
            reviewer: Some(ANN.to_owned()),
            estimate: Some("2".to_owned()),
            now: now.to_owned(),
            ..CardNewRequest::default()
        })
    }

    /// A subtask of C-0001 on `TIDE-SPRING` as the spec's split makes
    /// one: the parent's title, `discipline`, reviewed by [`ANN`].
    fn subtask(&self, discipline: &str, mandate: Mandate, now: &str) -> String {
        self.card(CardNewRequest {
            title: TITLE.to_owned(),
            discipline: Some(discipline.to_owned()),
            targets: vec!["TIDE-SPRING".to_owned()],
            reviewer: Some(ANN.to_owned()),
            mandate: Some(mandate),
            parent: Some("C-0001".to_owned()),
            now: now.to_owned(),
            ..CardNewRequest::default()
        })
    }

    /// `card edit id` as `actor` at `now`, `change` setting what it edits.
    fn edit(&self, id: &str, actor: &str, now: &str, change: impl FnOnce(&mut CardEditRequest)) {
        let mut request = CardEditRequest {
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
            actor: Some(actor.to_owned()),
            now: now.to_owned(),
            tasks_here: true,
        };
        change(&mut request);
        let outcome = card_edit(&self.env(), &Globals::default(), &request)
            .unwrap_or_else(|error| panic!("card edit {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "edit {id}: {outcome:?}");
    }

    /// The goal of `id` set to `goal`.
    fn goal(&self, id: &str, goal: &str, actor: &str, now: &str) {
        self.edit(id, actor, now, |request| {
            request.goal = Some(goal.to_owned())
        });
    }

    /// `card move id --to column`.
    fn move_to(&self, id: &str, column: &str, now: &str) {
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
        assert_eq!(outcome.exit(), Exit::Answered, "move {id}: {outcome:?}");
    }

    /// A reorder: `card move id --to <its own column>`.
    fn reorder(&self, id: &str, now: &str) {
        let column = self.json(&["card", "show", id])["column"]
            .as_str()
            .expect("a column")
            .to_owned();
        self.move_to(id, &column, now);
    }

    /// `n` reorders of `id`, through the library in one loop.
    fn reorders(&self, id: &str, column: &str, n: usize) {
        for _ in 0..n {
            self.move_to(id, column, T_LATE);
        }
    }

    /// Moved to `review` and accepted as briefed by [`ANN`].
    fn accept(&self, id: &str, now: &str) {
        self.move_to(id, "review", now);
        self.accept_with(id, now, |_| {});
    }

    fn accept_with(&self, id: &str, now: &str, change: impl FnOnce(&mut CardAcceptRequest)) {
        let mut request = CardAcceptRequest {
            id: id.to_owned(),
            actor: ANN.to_owned(),
            now: now.to_owned(),
            briefed: true,
            ..CardAcceptRequest::default()
        };
        change(&mut request);
        let mut yes = |_: &str| true;
        let outcome = card_accept(&self.env(), &Globals::default(), &request, &mut yes)
            .unwrap_or_else(|error| panic!("accept {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "accept {id}: {outcome:?}");
    }

    fn cancel(&self, id: &str, now: &str) {
        let outcome = card_cancel(
            &self.env(),
            &Globals::default(),
            &CardCancelRequest {
                id: id.to_owned(),
                reason: "Not needed now.".to_owned(),
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                ..CardCancelRequest::default()
            },
        )
        .unwrap_or_else(|error| panic!("cancel {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "cancel {id}: {outcome:?}");
    }

    fn clear(&self, id: &str, outcome: CardClearOutcome, now: &str) {
        let mut yes = |_: &str| true;
        let done = card_clear(
            &self.env(),
            &Globals::default(),
            &CardClearRequest {
                id: id.to_owned(),
                outcome,
                reason: "Read again.".to_owned(),
                estimate: (outcome == CardClearOutcome::NeedsCorrection).then(|| "1".to_owned()),
                actor: Some(ANN.to_owned()),
                now: now.to_owned(),
                ..CardClearRequest::default()
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("clear {id}: {error}"));
        assert_eq!(done.exit(), Exit::Answered, "clear {id}: {done:?}");
    }

    // ------------------------------------------------------- item writes

    fn decision(&self, node: &str, text: &str, card: Option<&str>, now: &str) -> String {
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
            card: card.map(str::to_owned),
            author_role: Some("developer".to_owned()),
            author_model: Some("m".to_owned()),
            run: None,
            login: None,
            now: now.to_owned(),
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

    fn question(&self, node: &str, text: &str, now: &str) -> String {
        let mut request = QuestionRequest {
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
            now: now.to_owned(),
            git: self.git_env(),
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
        };
        let ask = |request: &QuestionRequest| {
            propose_question(&self.env(), &Globals::default(), request)
                .unwrap_or_else(|error| panic!("question {text}: {error}"))
        };
        let first = ask(&request);
        if first.document.created {
            return stored(&first);
        }
        request.distinct_from = hit_names(&first);
        stored(&ask(&request))
    }

    fn discrepancy(&self, node: &str, text: &str, now: &str) -> String {
        let mut input: DiscrepancyInput = serde_json::from_value(json!({
            "node_ids": [node],
            "summary": text,
            "gap_type": "contradicts",
            "severity": "high",
            "evidence": [{"file": "docs/tides.md", "qpath": null, "lines": "12-14",
                          "observed": "The table prints a later hour.",
                          "documented": "The book gives an earlier hour."}],
            "options": [{"label": "book", "effect": "The book stands.", "price": "1 table"},
                        {"label": "table", "effect": "The table stands.", "price": "1 page"}],
            "recommendation": 0,
        }))
        .expect("a discrepancy's input");
        let raise = |input: &DiscrepancyInput| {
            propose_discrepancy(
                &self.env(),
                &Globals::default(),
                &DiscrepancyRequest {
                    input: input.clone(),
                    card: None,
                    author_role: Some("developer".to_owned()),
                    author_model: Some("m".to_owned()),
                    run: None,
                    login: None,
                    now: now.to_owned(),
                    git: self.git_env(),
                },
            )
            .unwrap_or_else(|error| panic!("discrepancy {text}: {error}"))
        };
        let first = raise(&input);
        if first.document.created {
            return stored(&first);
        }
        input.distinct_from = Some(hit_names(&first));
        stored(&raise(&input))
    }

    /// Approves `id` at `now` as the owner, its working answer or first
    /// option: the record's ID.
    fn decide(&self, id: &str, now: &str) -> String {
        let kind = self.queue().get(id).expect("get").expect("stored").kind;
        let option = (kind.as_str() == "discrepancy").then_some(0);
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
                option,
                answer: None,
                canon: None,
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("approve {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "approve {id}: {outcome:?}");
        self.queue()
            .get(id)
            .expect("get")
            .expect("stored")
            .record
            .unwrap_or_else(|| panic!("{id} holds a record"))
            .id
    }

    fn turn_down(&self, id: &str, now: &str) {
        let mut yes = |_: &str| true;
        let outcome = reject(
            &self.env(),
            &Globals::default(),
            &RejectRequest {
                id: id.to_owned(),
                reason: Some("Not for this table.".to_owned()),
                now: now.to_owned(),
                git: self.deciding(now),
            },
            &mut yes,
        )
        .unwrap_or_else(|error| panic!("reject {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "reject {id}: {outcome:?}");
    }

    /// An approve staged, never confirmed.
    fn stage_approve(&self, id: &str, now: &str) {
        let updated_at = self
            .queue()
            .get(id)
            .expect("get")
            .expect("stored")
            .updated_at;
        let outcome = stage(
            &self.env(),
            &Globals::default(),
            &StageRequest {
                id: id.to_owned(),
                body: StageBody::Approve {
                    option: None,
                    answer: None,
                    canon: None,
                    editorial: None,
                    note: None,
                    updated_at,
                    actor: None,
                },
                now: now.to_owned(),
                git: self.git_env(),
                actor: None,
            },
        )
        .unwrap_or_else(|error| panic!("stage {id}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "stage {id}: {outcome:?}");
    }

    /// The next item raised takes `PR-0040` and the queue's events start
    /// at seq 1: one open question on `HARBOR-LIGHTS` raised and renumbered
    /// `PR-0039`, every event removed (nothing reads them before a card).
    fn number_items_from_40(&self) {
        let id = self.question("HARBOR-LIGHTS", "Are the lights lit at dawn too?", T_MADE);
        assert_eq!(id, "PR-0001");
        self.sql("UPDATE proposals SET id = 'PR-0039' WHERE id = 'PR-0001'; DELETE FROM events;");
        assert_eq!(self.last_seq(), 0);
    }

    /// A hand edit of `TIDE-NEAP`, committed: C-0001 reads `suspect
    /// TIDE-BOOK`.
    fn hand_edit_neap(&self) {
        write(&self.root, "docs/tides.md", tides(NEAP_EDITED));
        self.commit("neap tides by hand");
    }

    // -------------------------------------------------------------- reads

    /// `card show id --marks`: the text run's stdout.
    fn show_text(&self, id: &str) -> String {
        self.ok(&["card", "show", id, "--marks"])
    }

    /// `card show id --marks --json`: `upstream`.
    fn upstream(&self, id: &str) -> Value {
        self.json(&["card", "show", id, "--marks"])["upstream"].clone()
    }

    /// `card list --marks --json`: `upstream` of each row, by ID.
    fn flags(&self) -> Vec<(String, Value)> {
        rows(&self.json(&["card", "list", "--marks", "--with-cancelled"]))
            .into_iter()
            .map(|row| {
                (
                    row["id"].as_str().expect("an ID").to_owned(),
                    row["upstream"].clone(),
                )
            })
            .collect()
    }

    /// The list flag of `id`.
    fn flag(&self, id: &str) -> Value {
        self.flags()
            .into_iter()
            .find(|(row, _)| row == id)
            .unwrap_or_else(|| panic!("{id} is listed"))
            .1
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

fn rows(list: &Value) -> Vec<Value> {
    list["cards"].as_array().expect("cards").clone()
}

/// The `upstream:` lines of a `card show` text, checked to stand right
/// after `correction:` and before `snapshot:`.
fn upstream_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let after = lines
        .iter()
        .position(|line| line.starts_with("correction: "))
        .unwrap_or_else(|| panic!("a correction line:\n{text}"));
    let found: Vec<String> = lines[after + 1..]
        .iter()
        .take_while(|line| line.starts_with("upstream: "))
        .map(|line| (*line).to_owned())
        .collect();
    assert!(
        lines
            .get(after + 1 + found.len())
            .is_some_and(|line| line.starts_with("snapshot: ")),
        "the upstream lines end at snapshot:\n{text}"
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("upstream: "))
            .count(),
        found.len(),
        "no other upstream line:\n{text}"
    );
    found
}

/// The `seq`s an `upstream` lists.
fn seqs(upstream: &Value) -> Vec<i64> {
    upstream["changes"]
        .as_array()
        .map(|all| {
            all.iter()
                .map(|change| change["seq"].as_i64().expect("a seq"))
                .collect()
        })
        .unwrap_or_default()
}

/// The `item`s an `upstream` lists.
fn items(upstream: &Value) -> Vec<String> {
    upstream["changes"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter(|change| change["source"] == "item")
                .map(|change| change["item"].as_str().expect("an item").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The seq of `id`'s last event of `event_type`.
fn seq_of(scene: &Scene, event_type: &str, id: &str) -> i64 {
    scene
        .sql(&format!(
            "SELECT MAX(seq) FROM events WHERE type = '{event_type}' AND \
             json_extract(payload, '$.id') = '{id}';"
        ))
        .parse()
        .unwrap_or_else(|_| panic!("{id} has a {event_type}"))
}

/// The upstream object of an edit, as the spec's JSON gives it.
fn edit_change(seq: i64, at: &str, by: &str, fields: &[&str]) -> Value {
    json!({"seq": seq, "at": at, "source": "edit", "by": by, "fields": fields,
           "item": null, "kind": null, "record": null, "on": null})
}

// ------------------------------------------------------------ the scenario

/// How far the spec's scenario runs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The three cards, seqs 1 to 3.
    Split,
    /// Through the decision's apply, seq 9, no hand edit.
    Applied,
    /// Then the hand edit of `TIDE-NEAP`.
    HandEdit,
}

/// the spec's scenario: C-0001 on `TIDE-BOOK` (seq 1); C-0002 `copy`
/// binding and C-0003 `plates` open on `TIDE-SPRING` (seqs 2, 3); C-0001's
/// goal edited as `Ann` at 09:00 (seq 4); PR-0040, a decision on
/// `TIDE-NEAP` (seq 5); two reorders of C-0001 (seqs 6, 7); its approve
/// (seq 8) and apply (seq 9) at 10:00 with DEC-0031; then the hand edit.
fn scenario(label: &str, stop: Stop) -> Scene {
    let scene = Scene::new(label);
    scene.number_items_from_40();
    assert_eq!(scene.parent(T_MADE), "C-0001");
    assert_eq!(scene.subtask("copy", Mandate::Binding, T_MADE), "C-0002");
    assert_eq!(scene.subtask("plates", Mandate::Open, T_MADE), "C-0003");
    assert_eq!(scene.last_seq(), 3, "the cards are seqs 1 to 3");
    if stop == Stop::Split {
        return scene;
    }
    scene.goal("C-0001", "A foreword for the spring tables.", ANN, T_EDIT);
    assert_eq!(seq_of(&scene, "card.edited", "C-0001"), 4);
    let decision = scene.decision("TIDE-NEAP", "Neap tables print in bold", None, T_EDIT);
    assert_eq!(decision, "PR-0040");
    scene.move_to("C-0001", "backlog", T_EDIT);
    scene.move_to("C-0001", "backlog", T_EDIT);
    assert_eq!(scene.decide("PR-0040", T_APPLY), "DEC-0031");
    assert_eq!(
        seq_of(&scene, "proposal.applied", "PR-0040"),
        9,
        "the apply is seq 9"
    );
    if stop == Stop::Applied {
        return scene;
    }
    scene.hand_edit_neap();
    scene
}

/// the spec's JSON object, byte for byte as its text.
fn data_upstream() -> Value {
    json!({"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK", "correction": null,
     "changes": [
      {"seq": 4, "at": "2026-11-12T09:00:00Z", "source": "edit", "by": "Ann", "fields": ["goal"],
       "item": null, "kind": null, "record": null, "on": null},
      {"seq": 9, "at": "2026-11-12T10:00:00Z", "source": "item", "by": "Ann Owner", "fields": [],
       "item": "PR-0040", "kind": "decision", "record": "DEC-0031", "on": "TIDE-NEAP"}],
     "more": 0})
}

/// The upstream object's keys, in order, and a change's.
const UPSTREAM_KEYS: [&str; 6] = ["parent", "mark", "on", "correction", "changes", "more"];
const CHANGE_KEYS: [&str; 9] = [
    "seq", "at", "source", "by", "fields", "item", "kind", "record", "on",
];

/// The keys of the first object under `"upstream": {` in `text`, in order:
/// the outer six, then the first change's nine.
fn key_orders(text: &str) -> (Vec<String>, Vec<String>) {
    let start = text.find("\"upstream\":{").expect("an upstream object");
    let tail = &text[start + "\"upstream\":".len()..];
    let mut depth = 0usize;
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    let mut chars = tail.char_indices().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut key_start = None;
    let mut after_brace_or_comma = false;
    while let Some((at, c)) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
                if let Some(begin) = key_start.take()
                    && chars.peek().is_some_and(|(_, next)| *next == ':')
                {
                    let key = tail[begin..at].to_owned();
                    match depth {
                        1 => outer.push(key),
                        3 if inner.len() < CHANGE_KEYS.len() => inner.push(key),
                        _ => {}
                    }
                }
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                if after_brace_or_comma {
                    key_start = Some(at + 1);
                }
            }
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        after_brace_or_comma = matches!(c, '{' | ',');
    }
    (outer, inner)
}

// ------------------------------------------------------------------ US-8

/// US-8: on the scenario, `spec card show C-0002 --marks` prints the spec's text
/// three lines byte for byte right after `correction:`; `--json` serves
/// the spec's JSON object, its keys in order. M: `upstream` read without
/// `--marks`.
#[test]
fn us8_the_scenario_prints_as_pinned() {
    let scene = scenario("us8-pinned", Stop::HandEdit);
    assert_eq!(
        upstream_lines(&scene.show_text("C-0002")),
        [
            "upstream: parent C-0001 | suspect TIDE-BOOK",
            "upstream: 4 | edited goal | Ann | 2026-11-12T09:00:00Z",
            "upstream: 9 | decision PR-0040 | record DEC-0031 | on TIDE-NEAP | Ann Owner | \
             2026-11-12T10:00:00Z",
        ]
    );
    let run = scene.spec(&["--json", "card", "show", "C-0002", "--marks"]);
    run.code(0);
    let document = run.json();
    assert_eq!(document["upstream"], data_upstream());
    assert_eq!(document["mark"], "open", "its own mark");
    let (outer, inner) = key_orders(&run.stdout);
    assert_eq!(outer, UPSTREAM_KEYS);
    assert_eq!(inner, CHANGE_KEYS);
    let ordered = common_keys(&run.stdout);
    let at = ordered
        .iter()
        .position(|key| key == "correction")
        .expect("correction");
    assert_eq!(ordered.get(at + 1).map(String::as_str), Some("upstream"));
    assert_eq!(ordered.len(), 36, "the card document's keys: {ordered:?}");
}

/// The top-level keys of a JSON object text, in order.
fn common_keys(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = None;
    let bytes: Vec<char> = text.chars().collect();
    let mut index = 0;
    let mut expect_key = false;
    while index < bytes.len() {
        let c = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
                if let Some(begin) = start.take() {
                    let mut next = index + 1;
                    while next < bytes.len() && bytes[next].is_whitespace() {
                        next += 1;
                    }
                    if depth == 1 && bytes.get(next) == Some(&':') {
                        keys.push(bytes[begin..index].iter().collect());
                    }
                }
            }
        } else {
            match c {
                '"' => {
                    in_string = true;
                    if expect_key {
                        start = Some(index + 1);
                    }
                }
                '{' | '[' => depth += 1,
                '}' | ']' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if !c.is_whitespace() && c != '"' {
                expect_key = matches!(c, '{' | ',');
            }
        }
        index += 1;
    }
    keys
}

/// US-8: without `--marks`, `upstream: -` after `correction:` and `null`;
/// a list's rows `null` without `--marks`; with it C-0002 and C-0003
/// `true`, C-0001 `false`; C-0002's text row as pinned.
#[test]
fn us8_unasked_is_null_and_the_rows_flag() {
    let scene = scenario("us8-rows", Stop::HandEdit);
    let plain = scene.ok(&["card", "show", "C-0002"]);
    assert_eq!(upstream_lines(&plain), ["upstream: -"]);
    assert_eq!(
        scene.json(&["card", "show", "C-0002"])["upstream"],
        Value::Null
    );
    assert_eq!(
        scene.json(&["card", "show", "C-0002", "--history"])["upstream"],
        Value::Null,
        "history alone asks nothing"
    );
    for row in rows(&scene.json(&["card", "list"])) {
        assert_eq!(row["upstream"], Value::Null, "{row}");
    }
    let marked = rows(&scene.json(&["card", "list", "--marks"]));
    let flags: Vec<(String, Value)> = marked
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("id").to_owned(),
                row["upstream"].clone(),
            )
        })
        .collect();
    assert_eq!(
        flags,
        [
            ("C-0001".to_owned(), json!(false)),
            ("C-0002".to_owned(), json!(true)),
            ("C-0003".to_owned(), json!(true)),
        ]
    );
    let text = scene.ok(&["card", "list", "--marks"]);
    let line = text
        .lines()
        .find(|line| line.starts_with("C-0002 |"))
        .expect("C-0002's row");
    assert_eq!(
        line,
        "C-0002 | copy/backlog | 1 | Write the tide table foreword | copy | - | - | TIDE-SPRING \
         | mandate binding | mark open | upstream"
    );
    let first = text
        .lines()
        .find(|line| line.starts_with("C-0001 |"))
        .expect("C-0001's row");
    assert!(first.ends_with("| mark suspect TIDE-BOOK"), "{first}");
    let unmarked = scene.ok(&["card", "list"]);
    assert!(
        !unmarked.contains("| upstream"),
        "no flag without --marks:\n{unmarked}"
    );
}

/// US-8: a list row gains `upstream` right after `correction`, 23 keys.
#[test]
fn us8_a_row_holds_23_keys() {
    let scene = scenario("us8-row-keys", Stop::Split);
    let run = scene.spec(&["--json", "card", "list", "--marks"]);
    run.code(0);
    let start = run.stdout.find("\"cards\":[").expect("the rows") + "\"cards\":[".len();
    let first = &run.stdout[start..];
    let end = first
        .find("},{")
        .or_else(|| first.find("}]"))
        .expect("a row's end")
        + 1;
    let row = common_keys(&first[..end]);
    assert_eq!(row.len(), 23, "{row:?}");
    let at = row
        .iter()
        .position(|key| key == "correction")
        .expect("correction");
    assert_eq!(row[at + 1], "upstream");
    assert_eq!(at + 2, row.len(), "upstream is last");
}

/// US-8: a parent row made unreadable: `show` exits 0, `upstream` null and
/// the note; the list row `false` with the unreadable row's own note.
#[test]
fn us8_an_unreadable_parent_is_not_judged() {
    let scene = scenario("us8-unreadable", Stop::HandEdit);
    scene.sql("UPDATE cards SET targets = 'not json' WHERE id = 'C-0001';");
    let run = scene.spec(&["--json", "card", "show", "C-0002", "--marks"]);
    run.code(0);
    let document = run.json();
    assert_eq!(document["upstream"], Value::Null, "{document}");
    let note = "upstream: C-0002: its parent C-0001 does not read; not judged";
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|one| one == note),
        "{document}"
    );
    let text = scene.spec(&["card", "show", "C-0002", "--marks"]);
    text.code(0);
    assert_eq!(upstream_lines(&text.stdout), ["upstream: -"]);
    assert!(
        format!("{}{}", text.stdout, text.stderr).contains(&format!("note: {note}")),
        "{}",
        text.show()
    );
    let list = scene.spec(&["--json", "card", "list", "--marks"]);
    list.code(0);
    let listed = list.json();
    let row = rows(&listed)
        .into_iter()
        .find(|row| row["id"] == "C-0002")
        .expect("C-0002 listed");
    assert_eq!(row["upstream"], json!(false));
    assert!(
        listed["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|one| one.as_str().is_some_and(|text| text.contains("C-0001"))),
        "the unreadable row's own note: {listed}"
    );
}

/// US-8: no `card.created` event of the subtask: only its parent's mark is
/// read, with the note; edits and items are skipped.
#[test]
fn us8_no_created_event_reads_the_mark_alone() {
    let scene = scenario("us8-no-anchor", Stop::HandEdit);
    scene.sql(
        "DELETE FROM events WHERE type = 'card.created' AND json_extract(payload, '$.id') = \
         'C-0002';",
    );
    let document = scene.json(&["card", "show", "C-0002", "--marks"]);
    assert_eq!(
        document["upstream"],
        json!({"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK", "correction": null,
               "changes": [], "more": 0})
    );
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|one| one
                == "upstream: C-0002: no card.created event; only its parent's mark is read"),
        "{document}"
    );
}

// ------------------------------------------------------------------ US-1

/// US-1, through a real split: C-0001 made at T5, `card split --parent
/// C-0001` makes C-0002 and C-0003 at T9; the parent's goal edited at T5;
/// C-0004 made under C-0001 at T1. C-0002 and C-0003 each list the one
/// edit with `at` T5; C-0004's `upstream` is `null`. M: `at` compared
/// instead of `seq`.
#[test]
fn us1_seq_orders_never_the_clock() {
    const T1: &str = "2026-11-12T01:00:00Z";
    const T5: &str = "2026-11-12T05:00:00Z";
    const T9: &str = "2026-11-12T09:00:00Z";
    let scene = Scene::new("us1");
    assert_eq!(scene.parent(T5), "C-0001");
    let split = card_split(
        &scene.env(),
        &Globals::default(),
        &CardSplitRequest {
            targets: Vec::new(),
            title: None,
            goal: None,
            discipline: None,
            parent: Some("C-0001".to_owned()),
            children: Some(vec!["copy".to_owned(), "plates".to_owned()]),
            mandate: vec!["copy=binding".to_owned(), "plates=open".to_owned()],
            reviewer: None,
            dry_run: false,
            actor: Some(ANN.to_owned()),
            now: T9.to_owned(),
        },
    )
    .expect("the split");
    assert_eq!(split.exit(), Exit::Answered, "{split:?}");
    scene.goal("C-0001", "A foreword for the spring tables.", ANN, T5);
    let edit = seq_of(&scene, "card.edited", "C-0001");
    let late = scene.card(CardNewRequest {
        title: "Late subtask".to_owned(),
        discipline: Some("copy".to_owned()),
        parent: Some("C-0001".to_owned()),
        now: T1.to_owned(),
        ..CardNewRequest::default()
    });
    assert_eq!(late, "C-0004");
    for id in ["C-0002", "C-0003"] {
        let upstream = scene.upstream(id);
        assert_eq!(
            upstream,
            json!({"parent": "C-0001", "mark": null, "on": null, "correction": null,
                   "changes": [edit_change(edit, T5, ANN, &["goal"])], "more": 0}),
            "{id}"
        );
    }
    assert_eq!(scene.upstream("C-0004"), Value::Null);
    assert_eq!(scene.flag("C-0004"), json!(false));
}

// ------------------------------------------------------------------ US-2

/// US-2: on the parent, nothing signals for `--title`, `--due`, `--estimate
/// --reason` (`card.reestimated`), `--evidence`, `--task`, `assign`, a
/// move, a reorder, a mandate edit by its reviewer; then `edit --title X
/// --goal Y` lists one change, `fields: ["goal"]`, and `--target`
/// `["targets"]`. M: any `card.edited` counted; `fields` unfiltered.
#[test]
fn us2_goal_targets_criteria_only() {
    let scene = scenario("us2", Stop::Split);
    scene.edit("C-0001", ANN, T_EDIT, |r| {
        r.title = Some("The foreword".to_owned())
    });
    scene.edit("C-0001", ANN, T_EDIT, |r| {
        r.due = Some("2026-12-01".to_owned())
    });
    scene.edit("C-0001", ANN, T_EDIT, |r| {
        r.estimate = Some("3".to_owned());
        r.reason = Some("Longer than it looked.".to_owned());
    });
    scene.edit("C-0001", ANN, T_EDIT, |r| {
        r.evidence = vec!["note=First draft read aloud.".to_owned()];
    });
    scene.edit("C-0001", ANN, T_EDIT, |r| {
        r.tasks = vec!["T-0007".to_owned()];
        r.tasks_here = false;
    });
    scene.edit("C-0001", ANN, T_EDIT, |r| r.mandate = Some(Mandate::Open));
    let assigned = card_assign(
        &scene.env(),
        &Globals::default(),
        &CardAssignRequest {
            id: "C-0001".to_owned(),
            assignee: Some("Bea".to_owned()),
            actor: Some(ANN.to_owned()),
            now: T_EDIT.to_owned(),
            ..CardAssignRequest::default()
        },
    )
    .expect("assigned");
    assert_eq!(assigned.exit(), Exit::Answered, "{assigned:?}");
    scene.move_to("C-0001", "review", T_EDIT);
    scene.reorder("C-0001", T_EDIT);
    let types = scene.sql(
        "SELECT group_concat(type, ' ') FROM (SELECT type FROM events WHERE seq > 3 ORDER BY \
         seq);",
    );
    assert!(
        types.contains("card.reestimated")
            && types.contains("card.assigned")
            && types.contains("card.moved"),
        "{types}"
    );
    assert_eq!(scene.upstream("C-0002"), Value::Null, "nothing counted");
    assert_eq!(scene.flag("C-0002"), json!(false));

    scene.edit("C-0001", ANN, T_APPLY, |r| {
        r.title = Some("The spring foreword".to_owned());
        r.goal = Some("Spring first.".to_owned());
    });
    let goal = seq_of(&scene, "card.edited", "C-0001");
    scene.edit("C-0001", ANN, T_LATE, |r| {
        r.targets = vec!["TIDE-BOOK".to_owned(), "HARBOR-LIGHTS".to_owned()];
    });
    let targets = seq_of(&scene, "card.edited", "C-0001");
    assert_eq!(
        scene.upstream("C-0002")["changes"],
        json!([
            edit_change(goal, T_APPLY, ANN, &["goal"]),
            edit_change(targets, T_LATE, ANN, &["targets"]),
        ])
    );
    assert_eq!(scene.flag("C-0002"), json!(true));
    assert_eq!(scene.flag("C-0001"), json!(false), "no parent");
}

// ------------------------------------------------------------------ US-3

/// US-3: the scenario plus `HARBOR-LIGHTS`. C-0002 lists the applied
/// decision on `TIDE-NEAP` (kind, record, `on`, `by` "Ann Owner") and none
/// of: a question on `TIDE-SPRING` applied before the subtasks were made;
/// an open, a rejected and a staged decision on `TIDE-SPRING`; an applied
/// decision on `HARBOR-LIGHTS`; an applied decision on `TIDE-SPRING` whose
/// card is C-0002. An applied discrepancy on `TIDE-SPRING` raised against
/// the parent is listed by its nodes. M: open items counted; the reach
/// without `within` (the `TIDE-NEAP` decision lost); the child's own
/// counted; the applied `seq` ignored.
#[test]
fn us3_applied_items_on_the_reach() {
    let scene = Scene::new("us3");
    assert_eq!(scene.parent(T_MADE), "C-0001");
    let early = scene.question("TIDE-SPRING", "Do spring tables list the moon?", T_MADE);
    let early_record = scene.decide(&early, T_MADE);
    assert_eq!(scene.subtask("copy", Mandate::Binding, T_MADE), "C-0002");
    assert_eq!(scene.subtask("plates", Mandate::Open, T_MADE), "C-0003");
    let neap = scene.decision("TIDE-NEAP", "Neap tables print in bold", None, T_EDIT);
    let open = scene.decision("TIDE-SPRING", "Spring tables print in red", None, T_EDIT);
    let rejected = scene.decision("TIDE-SPRING", "Spring tables print in blue", None, T_EDIT);
    let staged = scene.decision("TIDE-SPRING", "Spring tables print twice", None, T_EDIT);
    let harbor = scene.decision("HARBOR-LIGHTS", "Lights burn till dawn", None, T_EDIT);
    let own = scene.decision(
        "TIDE-SPRING",
        "Spring copy runs one page",
        Some("C-0002"),
        T_EDIT,
    );
    let gap = scene.discrepancy("TIDE-SPRING", "Spring hours drift", T_EDIT);
    let neap_record = scene.decide(&neap, T_APPLY);
    scene.turn_down(&rejected, T_APPLY);
    scene.stage_approve(&staged, T_APPLY);
    scene.decide(&harbor, T_APPLY);
    scene.decide(&own, T_APPLY);
    let gap_record = scene.decide(&gap, T_LATE);
    let _ = (early_record, open);
    // An item whose `proposal.applied` stands while its row reads `open`
    // (set by SQL): the row's state decides, never the event alone.
    let reopened = scene.decision("TIDE-NEAP", "Neap tables print twice", None, T_EDIT);
    scene.decide(&reopened, T_LATE);
    scene.sql(&format!(
        "UPDATE proposals SET status = 'open', decided_by = NULL, decided_at = NULL, \
         applied_commit = NULL WHERE id = '{reopened}';"
    ));

    let upstream = scene.upstream("C-0002");
    assert_eq!(items(&upstream), [neap.clone(), gap.clone()], "{upstream}");
    let notes = notes_of(&scene, "C-0002");
    assert!(
        notes.iter().all(|note| !note.starts_with("upstream: ")),
        "every row reads, {reopened}'s too: {notes:?}"
    );
    let changes = upstream["changes"].as_array().expect("changes");
    assert_eq!(
        changes[0],
        json!({"seq": seq_of(&scene, "proposal.applied", &neap), "at": T_APPLY,
               "source": "item", "by": OWNER_NAME, "fields": [], "item": neap,
               "kind": "decision", "record": neap_record, "on": "TIDE-NEAP"})
    );
    assert_eq!(
        changes[1],
        json!({"seq": seq_of(&scene, "proposal.applied", &gap), "at": T_LATE,
               "source": "item", "by": OWNER_NAME, "fields": [], "item": gap,
               "kind": "discrepancy", "record": gap_record, "on": "TIDE-SPRING"})
    );
    assert_eq!(upstream["mark"], Value::Null);
    assert_eq!(
        items(&scene.upstream("C-0003")),
        [neap.clone(), own.clone(), gap.clone()],
        "C-0002's own decision is C-0003's news"
    );
}

// ------------------------------------------------------------------ US-4

/// What the marks read, each as one string: `spec trace` and `spec trace
/// TIDE-BOOK`, text and JSON; each list row's mark keys; each card's
/// `allowed` and `mark`.
fn mark_reads(scene: &Scene) -> Vec<String> {
    let mut reads = Vec::new();
    for args in TRACES {
        let run = scene.spec(args);
        run.code(0);
        reads.push(run.stdout);
    }
    for row in rows(&scene.json(&["card", "list", "--marks"])) {
        reads.push(format!(
            "{} {} {} {} {}",
            row["id"], row["mark"], row["on"], row["discrepancy"], row["correction"]
        ));
    }
    for id in ["C-0001", "C-0002", "C-0003"] {
        let card = scene.json(&["card", "show", id, "--marks"]);
        reads.push(format!("{id} {} {}", card["allowed"], card["mark"]));
    }
    reads
}

/// The four trace reads of [`mark_reads`], in its order.
const TRACES: [&[&str]; 4] = [
    &["trace"],
    &["--json", "trace"],
    &["trace", "TIDE-BOOK"],
    &["--json", "trace", "TIDE-BOOK"],
];

/// The record's row in `spec trace`'s text.
const RECORD_ROW: &str = "no card | DEC-0031 | Yes: Neap tables print in bold | \
                          docs/records/DEC/DEC-0031.md | cards 0";

/// US-4: over the scenario before seq 4 and after seq 9 (no hand edit),
/// the rows' mark keys, every card's `allowed` and `mark` and `spec trace
/// TIDE-BOOK` (text and JSON) are byte-equal, while C-0002 and C-0003 now
/// flag. `spec trace` of the whole corpus differs by the record the apply
/// wrote, `DEC-0031`, alone: its row reads `no card`, every other row is
/// byte-equal, and the header counts one node more, without a card. Across
/// the parent's goal edit alone every read, both traces whole, is
/// byte-equal. M: the signal folded into `card_mark` (C-0002 reads
/// `suspect`).
#[test]
fn us4_marks_untouched() {
    let scene = scenario("us4", Stop::Split);
    let before = mark_reads(&scene);
    scene.goal("C-0001", "A foreword for the spring tables.", ANN, T_EDIT);
    assert_eq!(
        scene.flag("C-0002"),
        json!(true),
        "the edit's signal is there"
    );
    let edited = mark_reads(&scene);
    assert_eq!(before.len(), edited.len());
    for (was, now) in before.iter().zip(&edited) {
        assert_eq!(was, now, "across the parent's goal edit");
    }

    let decision = scene.decision("TIDE-NEAP", "Neap tables print in bold", None, T_EDIT);
    assert_eq!(scene.decide(&decision, T_APPLY), "DEC-0031");
    assert_eq!(
        items(&scene.upstream("C-0002")),
        [decision],
        "the item's signal"
    );
    let after = mark_reads(&scene);
    assert_eq!(before.len(), after.len());
    for (index, (was, now)) in before.iter().zip(&after).enumerate().skip(2) {
        assert_eq!(was, now, "read {index} across the apply");
    }

    let text = &after[0];
    assert_eq!(
        text.lines().filter(|line| *line == RECORD_ROW).count(),
        1,
        "the record's row:\n{text}"
    );
    let without: Vec<&str> = text.lines().filter(|line| *line != RECORD_ROW).collect();
    let mut was = before[0].lines();
    let header = was.next().expect("a header");
    assert_eq!(
        without[0],
        header
            .replace("nodes 4,", "nodes 5,")
            .replace("no card 2,", "no card 3,"),
        "one node more, without a card"
    );
    assert_eq!(without[1..], was.collect::<Vec<&str>>()[..]);

    let (was, now): (Value, Value) = (
        serde_json::from_str(&before[1]).expect("JSON"),
        serde_json::from_str(&after[1]).expect("JSON"),
    );
    let (record, others): (Vec<Value>, Vec<Value>) = now["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .cloned()
        .partition(|node| node["id"] == "DEC-0031");
    assert_eq!(others, was["nodes"].as_array().expect("nodes")[..]);
    assert_eq!(record.len(), 1);
    assert_eq!(record[0]["rollup"], "no card");
    assert_eq!(record[0]["cards"], json!([]));
    let mut counts = was["counts"].clone();
    counts["no_card"] = json!(counts["no_card"].as_u64().expect("a count") + 1);
    assert_eq!(now["counts"], counts);
}

// ------------------------------------------------------------------ US-5

/// US-5 (the CLI's part): after the parent's goal edit, 70 reorders of the
/// parent and 70 of C-0002: the edit is still listed. M:
/// `card_history(…, 64)`.
#[test]
fn us5_uncapped_past_140_reorders() {
    let scene = scenario("us5-uncapped", Stop::Split);
    scene.goal("C-0001", "A foreword for the spring tables.", ANN, T_EDIT);
    scene.reorders("C-0001", "backlog", 70);
    scene.reorders("C-0002", "backlog", 70);
    assert!(scene.last_seq() >= 144);
    assert_eq!(seqs(&scene.upstream("C-0002")), [4]);
}

/// US-5: twenty qualifying parent edits list the newest 16, `more: 4`, the
/// text ending `upstream: more 4`. M: no cap; the oldest kept.
#[test]
fn us5_twenty_edits_list_the_newest_sixteen() {
    let scene = scenario("us5-cap", Stop::Split);
    for n in 0..20 {
        scene.goal("C-0001", &format!("Goal {n}."), ANN, T_EDIT);
    }
    let upstream = scene.upstream("C-0002");
    assert_eq!(seqs(&upstream), (8..24).collect::<Vec<i64>>(), "{upstream}");
    assert_eq!(upstream["more"], 4);
    let lines = upstream_lines(&scene.show_text("C-0002"));
    assert_eq!(lines.len(), 18, "{lines:?}");
    assert_eq!(lines[0], "upstream: parent C-0001 | -");
    assert_eq!(
        lines[1],
        "upstream: 8 | edited goal | Ann | 2026-11-12T09:00:00Z"
    );
    assert_eq!(lines.last().map(String::as_str), Some("upstream: more 4"));
}

// ------------------------------------------------------------------ US-6

/// US-6 (a), (b): C-0002's own goal edit after the parent's: `null`, its
/// row `false`; its own due edit ends nothing; a later parent criteria edit
/// lists that one alone. M: the child's edits ignored; any child edit
/// ending it.
#[test]
fn us6_ab_the_childs_own_edits() {
    let scene = scenario("us6-ab", Stop::Split);
    scene.goal("C-0001", "A foreword for the spring tables.", ANN, T_EDIT);
    scene.edit("C-0002", ANN, T_EDIT, |r| {
        r.due = Some("2026-12-01".to_owned())
    });
    assert_eq!(
        seqs(&scene.upstream("C-0002")),
        [4],
        "a due edit ends nothing"
    );
    scene.goal("C-0002", "Spring first, as asked.", "Bea", T_EDIT);
    assert_eq!(scene.upstream("C-0002"), Value::Null);
    assert_eq!(scene.flag("C-0002"), json!(false));
    assert_eq!(
        seqs(&scene.upstream("C-0003")),
        [4],
        "its sibling still flags"
    );
    scene.edit("C-0001", ANN, T_LATE, |r| {
        r.criteria = vec!["Names both tides.".to_owned()];
    });
    let criteria = seq_of(&scene, "card.edited", "C-0001");
    assert_eq!(
        scene.upstream("C-0002")["changes"],
        json!([edit_change(criteria, T_LATE, ANN, &["criteria"])])
    );
}

/// US-6 (c): an accepted and a cancelled subtask read `null`, rows `false`.
#[test]
fn us6_c_a_closed_subtask_reads_null() {
    let scene = scenario("us6-c", Stop::HandEdit);
    assert_eq!(scene.flag("C-0002"), json!(true));
    scene.accept("C-0002", T_LATE);
    scene.cancel("C-0003", T_LATE);
    for id in ["C-0002", "C-0003"] {
        assert_eq!(scene.upstream(id), Value::Null, "{id}");
        assert_eq!(scene.flag(id), json!(false), "{id}");
    }
}

/// US-6 (d): C-0005 made with no parent, the parent edited, then `edit
/// C-0005 --parent C-0001`: nothing; the parent edited again: that edit.
/// M: the anchor from `card.created` alone.
#[test]
fn us6_d_re_parenting_moves_the_anchor() {
    let scene = scenario("us6-d", Stop::Split);
    let loose = scene.card(CardNewRequest {
        title: "Loose foreword".to_owned(),
        discipline: Some("copy".to_owned()),
        now: T_MADE.to_owned(),
        ..CardNewRequest::default()
    });
    assert_eq!(loose, "C-0004");
    let fifth = scene.card(CardNewRequest {
        title: "Spare".to_owned(),
        now: T_MADE.to_owned(),
        ..CardNewRequest::default()
    });
    assert_eq!(fifth, "C-0005");
    scene.goal("C-0001", "A foreword for the spring tables.", ANN, T_EDIT);
    scene.edit("C-0005", ANN, T_EDIT, |r| {
        r.parent = Some("C-0001".to_owned())
    });
    assert_eq!(scene.upstream("C-0005"), Value::Null);
    assert_eq!(scene.flag("C-0005"), json!(false));
    scene.edit("C-0001", ANN, T_APPLY, |r| {
        r.targets = vec!["TIDE-BOOK".to_owned(), "HARBOR-LIGHTS".to_owned()];
    });
    let targets = seq_of(&scene, "card.edited", "C-0001");
    assert_eq!(
        scene.upstream("C-0005")["changes"],
        json!([edit_change(targets, T_APPLY, ANN, &["targets"])])
    );
}

// ------------------------------------------------------------------ US-7

/// US-7: a hand edit of `TIDE-NEAP`: C-0001 reads `suspect TIDE-BOOK`,
/// C-0002's own mark `open`, its `upstream` the mark alone, its row `true`;
/// its own goal edit leaves that; `card clear C-0001 --no-effect` gives
/// `null`, `false`. M: the mark source dropped; the mark ended by the
/// child's edit.
#[test]
fn us7_the_parents_suspect_mark() {
    let scene = scenario("us7-suspect", Stop::Split);
    scene.hand_edit_neap();
    let parent = scene.json(&["card", "show", "C-0001", "--marks"]);
    assert_eq!(
        (parent["mark"].clone(), parent["on"].clone()),
        (json!("suspect"), json!("TIDE-BOOK"))
    );
    let child = scene.json(&["card", "show", "C-0002", "--marks"]);
    assert_eq!(child["mark"], "open");
    let marked = json!({"parent": "C-0001", "mark": "suspect", "on": "TIDE-BOOK",
                        "correction": null, "changes": [], "more": 0});
    assert_eq!(child["upstream"], marked);
    assert_eq!(scene.flag("C-0002"), json!(true));
    assert_eq!(
        upstream_lines(&scene.show_text("C-0002")),
        ["upstream: parent C-0001 | suspect TIDE-BOOK"]
    );
    scene.goal("C-0002", "Spring first, as asked.", "Bea", T_EDIT);
    assert_eq!(scene.upstream("C-0002"), marked, "the mark line stands");
    scene.clear("C-0001", CardClearOutcome::NoEffect, T_APPLY);
    assert_eq!(scene.upstream("C-0002"), Value::Null);
    assert_eq!(scene.flag("C-0002"), json!(false));
}

/// US-7: a live correction holding the parent: `correcting` with its
/// correction, the text as a list row prints the mark.
#[test]
fn us7_a_live_correction_reads_correcting() {
    let scene = scenario("us7-correcting", Stop::Split);
    scene.accept("C-0001", T_EDIT);
    scene.hand_edit_neap();
    scene.clear("C-0001", CardClearOutcome::NeedsCorrection, T_APPLY);
    let parent = scene.json(&["card", "show", "C-0001", "--marks"]);
    assert_eq!(parent["mark"], "correcting", "{parent}");
    let correction = parent["correction"]
        .as_str()
        .expect("a correction")
        .to_owned();
    assert_eq!(correction, "C-0004");
    assert_eq!(
        scene.upstream("C-0002"),
        json!({"parent": "C-0001", "mark": "correcting", "on": "TIDE-BOOK",
               "correction": correction, "changes": [], "more": 0})
    );
    assert_eq!(
        upstream_lines(&scene.show_text("C-0002")),
        ["upstream: parent C-0001 | correcting C-0004"]
    );
    assert_eq!(scene.flag("C-0002"), json!(true));
}

/// US-7: a parent reading `departed` is no mark source. M: `departed`
/// counted.
#[test]
fn us7_departed_is_no_source() {
    let scene = scenario("us7-departed", Stop::Split);
    scene.move_to("C-0001", "review", T_EDIT);
    scene.accept_with("C-0001", T_EDIT, |request| {
        request.briefed = false;
        request.departs = vec!["TIDE-BOOK".to_owned()];
        request.summary = Some("The foreword skips the neap tables".to_owned());
        request.observed = Some("Only spring tides are named.".to_owned());
        request.documented = Some("The book names both tides.".to_owned());
    });
    let parent = scene.json(&["card", "show", "C-0001", "--marks"]);
    assert_eq!(parent["mark"], "departed", "{parent}");
    assert_eq!(scene.upstream("C-0002"), Value::Null);
    assert_eq!(scene.flag("C-0002"), json!(false));
}

// ------------------------------------------------------------ determinism

/// "Rules and edge cases": one corpus and queue state gives one text and
/// JSON at any root and `HOME`; the read writes nothing but the index.
#[test]
fn one_state_one_answer_at_any_root_and_home() {
    let scene = scenario("determinism", Stop::HandEdit);
    let text = scene.show_text("C-0002");
    let document = scene.json(&["card", "show", "C-0002", "--marks"]);
    let list = scene.ok(&["card", "list", "--marks"]);
    let dump = scene.queue().dump().expect("dump");
    let again = scene.show_text("C-0002");
    assert_eq!(text, again);
    assert_eq!(
        scene.queue().dump().expect("dump"),
        dump,
        "a read writes no row"
    );

    let other = scene.scratch.dir("elsewhere");
    let copy = other.join("repo");
    common::copy_dir(&scene.root, &copy);
    let home = scene.scratch.home("h2");
    let data = data_dir(&home);
    fs::create_dir_all(&data).expect("a data directory");
    fs::copy(scene.db(), data.join(format!("{SLUG}.db"))).expect("the queue copied");
    let moved = scene.spec_in(&copy, &home, &["card", "show", "C-0002", "--marks"]);
    moved.code(0);
    assert_eq!(moved.stdout, text);
    let moved_json = scene.spec_in(
        &copy,
        &home,
        &["--json", "card", "show", "C-0002", "--marks"],
    );
    moved_json.code(0);
    assert_eq!(moved_json.json(), document);
    let moved_list = scene.spec_in(&copy, &home, &["card", "list", "--marks"]);
    moved_list.code(0);
    assert_eq!(moved_list.stdout, list);
}

// ------------------------------------------------- notes (iteration 2)

/// The `notes` of `card show id --marks --json`.
fn notes_of(scene: &Scene, id: &str) -> Vec<String> {
    scene.json(&["card", "show", id, "--marks"])["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

/// US-8, an applied row that does not decode (its `card_id` set by SQL to
/// a value holding an escape): `show` names it, one note after the
/// no-anchor note's place, `upstream: <C>: proposal <PR>: the stored
/// `card_id` cannot be read: <reason>; not judged`, raw in the JSON's
/// `notes`, escaped on stderr; the row is never read as clean nor listed:
/// the edit and the mark stand. A list adds no note and keeps its flag. M:
/// the note dropped (the read reads as clean).
#[test]
fn us8_an_undecodable_applied_row_is_named_not_judged() {
    let scene = scenario("us8-undecodable", Stop::HandEdit);
    scene.sql("UPDATE proposals SET card_id = 'X' || char(27) || '7' WHERE id = 'PR-0040';");
    let raw = "upstream: C-0002: proposal PR-0040: the stored `card_id` cannot be read: `X\u{1b}7` \
               is no card ID; not judged";
    let document = scene.json(&["card", "show", "C-0002", "--marks"]);
    let notes: Vec<&str> = document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with("upstream: "))
        .collect();
    assert_eq!(notes, [raw], "{document}");
    let mut want = data_upstream();
    want["changes"].as_array_mut().expect("changes").truncate(1);
    assert_eq!(document["upstream"], want, "the edit and the mark stand");

    let text = scene.spec(&["card", "show", "C-0002", "--marks"]);
    text.code(0);
    let escaped = raw.replace('\u{1b}', "\\u{1b}");
    let all = format!("{}{}", text.stdout, text.stderr);
    assert!(all.contains(&format!("note: {escaped}")), "{}", text.show());
    assert!(!all.contains('\u{1b}'), "escaped: {}", text.show());

    let list = scene.json(&["card", "list", "--marks"]);
    assert!(
        list["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .all(|note| !note.as_str().unwrap_or_default().starts_with("upstream: ")),
        "a list adds no note: {list}"
    );
    assert_eq!(scene.flag("C-0002"), json!(true), "the flag unchanged");
    let undecoded = rows(&list)
        .into_iter()
        .find(|row| row["id"] == "C-0002")
        .expect("C-0002")["upstream"]
        .clone();
    assert_eq!(undecoded, json!(true));
}

/// Iteration 2: a subtask's read keeps its own notes: the parent's
/// discrepancy that is not in the queue is the parent's note on its own
/// read, never the child's; the child's mark and `upstream` read as before
/// (a departed parent is no source).
#[test]
fn the_parents_settled_notes_stay_its_own() {
    let scene = scenario("notes-settled", Stop::Split);
    scene.move_to("C-0001", "review", T_EDIT);
    scene.accept_with("C-0001", T_EDIT, |request| {
        request.briefed = false;
        request.departs = vec!["TIDE-BOOK".to_owned()];
        request.summary = Some("The foreword skips the neap tables".to_owned());
        request.observed = Some("Only spring tides are named.".to_owned());
        request.documented = Some("The book names both tides.".to_owned());
    });
    let discrepancy = scene.json(&["card", "show", "C-0001", "--marks"])["discrepancy"]
        .as_str()
        .expect("the parent's discrepancy")
        .to_owned();
    scene.sql(&format!(
        "DELETE FROM proposals WHERE id = '{discrepancy}';"
    ));
    let parent = notes_of(&scene, "C-0001");
    let settled =
        format!("C-0001: its discrepancy {discrepancy} is not in the queue; read as unsettled");
    assert!(parent.contains(&settled), "{parent:?}");
    let child = notes_of(&scene, "C-0002");
    assert!(
        child.iter().all(|note| !note.contains("C-0001:")),
        "the parent's note is its own: {child:?}"
    );
    assert_eq!(scene.upstream("C-0002"), Value::Null);
}

/// Iteration 3, the many-rows form: two applied rows that do not decode
/// (each `card_id` set by SQL) give exactly one note, `upstream: <C>: <k>
/// applied rows cannot be read, the first: <the lowest-numbered row>; not
/// judged`, in the JSON's `notes` and on stderr; neither row is listed, the
/// edit and the mark stand; a list adds no note. (One row keeps the single
/// form: `us8_an_undecodable_applied_row_is_named_not_judged`.) M: one note
/// per row.
#[test]
fn many_undecodable_applied_rows_are_one_note() {
    let scene = scenario("notes-many-undecodable", Stop::HandEdit);
    let second = scene.decision("TIDE-NEAP", "Neap tables print twice", None, T_LATE);
    assert_eq!(second, "PR-0041");
    scene.decide(&second, T_LATE);
    scene.sql("UPDATE proposals SET card_id = 'X-8' WHERE id = 'PR-0041';");
    scene.sql("UPDATE proposals SET card_id = 'X-7' WHERE id = 'PR-0040';");
    let note = "upstream: C-0002: 2 applied rows cannot be read, the first: proposal PR-0040: \
                the stored `card_id` cannot be read: `X-7` is no card ID; not judged";
    let document = scene.json(&["card", "show", "C-0002", "--marks"]);
    let notes: Vec<&str> = document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|note| note.starts_with("upstream: "))
        .collect();
    assert_eq!(notes, [note], "{document}");
    let mut want = data_upstream();
    want["changes"].as_array_mut().expect("changes").truncate(1);
    assert_eq!(document["upstream"], want, "the edit and the mark stand");

    let text = scene.spec(&["card", "show", "C-0002", "--marks"]);
    text.code(0);
    let all = format!("{}{}", text.stdout, text.stderr);
    assert_eq!(
        all.lines()
            .filter(|line| line.starts_with("note: upstream: "))
            .collect::<Vec<&str>>(),
        [format!("note: {note}").as_str()],
        "{}",
        text.show()
    );

    let list = scene.json(&["card", "list", "--marks"]);
    assert!(
        list["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .all(|note| !note.as_str().unwrap_or_default().starts_with("upstream: ")),
        "a list adds no note: {list}"
    );
}
