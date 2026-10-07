//! State S of docs/features/ui-live-tasks.md "Acceptance criteria", made
//! through the CLI library (`specengine_cli::task_*`, the owner's three
//! with a consent callback that answers yes and records the question, as
//! the CLI's `tests/task_common`), never through a terminal: on a git
//! copy of `fixtures/spec-a` (A) or `fixtures/spec-b` (B) of the shared
//! harness (git auto-maintenance off), every git process of a call in the
//! harness's sandbox, an injected clock.
//!
//! - T-0001 `draft`;
//! - T-0002 approved (`ready`), its target then edited and committed:
//!   `stale` true, one `snapshot_diff` entry;
//! - T-0003 by an agent (`author` with role, model and run): planned with
//!   a reference criterion and a free-text one and an affected node,
//!   `changes --note`, re-planned, approved, an open question with a
//!   working answer on its target bound to it, claimed in the root's
//!   worktree (`in_progress`), a run reported (`completed`);
//! - T-0004 cancelled.
//!
//! Thirteen events: T-0001 one, T-0002 two, T-0003 seven `task.*` and the
//! question's `proposal.created`, T-0004 two. Only the test binaries of
//! the slice include this module (`mod task_state;`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use specengine_cli::{
    CliError, Env, Exit, GitEnv, Globals, ProposedText, QuestionRequest, TaskClaimRequest,
    TaskDecisionRequest, TaskNewRequest, TaskOutcome, TaskPlanRequest, TaskReportRequest,
    TaskStatus, propose_question_with_task, task_approve, task_cancel, task_changes, task_claim,
    task_new, task_plan, task_report,
};

use crate::common::{Git, Scratch, replace};

/// The clock of every call.
pub const CLOCK: &str = "2026-10-07T09:00:00Z";

/// What state S needs of one fixture.
pub struct Case {
    pub fixture: &'static str,
    pub slug: &'static str,
    /// The copy's branch.
    pub branch: &'static str,
    /// T-0001's and T-0004's target.
    pub draft: &'static str,
    /// T-0002's target, edited after its approval: in `path`, `from` (once
    /// in the file) becomes `to`.
    pub stale: &'static str,
    pub path: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    /// T-0003's target, the question's node.
    pub target: &'static str,
    /// T-0003's reference criterion and affected node.
    pub criterion: &'static str,
    pub affected: &'static str,
}

/// spec-a (a game).
pub const A: Case = Case {
    fixture: "spec-a",
    slug: "lantern-keep",
    branch: "main",
    draft: "MEC-SPRINT",
    stale: "EDGE-SPRINT-EMPTY",
    path: "docs/spec/movement/sprint.md",
    from: "the sprint ends;",
    to: "the sprint stops;",
    target: "MEC-STAMINA",
    criterion: "stamina-tuning/AC-07",
    affected: "RULE-STAM-REGEN",
};

/// spec-b (a command-line tool, Russian prose).
pub const B: Case = Case {
    fixture: "spec-b",
    slug: "zerkalo",
    branch: "trunk",
    draft: "CMD-STATUS",
    stale: "CMD-SYNC",
    path: "docs/spec/cli.md",
    from: "`sync`",
    to: "`sync --each`",
    target: "REQ-001",
    criterion: "dry-run/CRIT-01",
    affected: "MOD-CLI",
};

/// The library calls of one project: its root (also the current
/// directory and the worktree of a claim), the `HOME` of the data
/// directory, the git sandbox.
pub struct Calls {
    pub root: PathBuf,
    pub home: PathBuf,
    pub git: Git,
}

impl Calls {
    pub fn new(scratch: &Scratch, root: &Path, home: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            home: home.to_path_buf(),
            git: scratch.git(),
        }
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
        GitEnv::new(&self.root, self.git.vars())
    }

    /// `spec task new --nodes NODE --title TITLE [--author-role …]`: the
    /// new task's ID, which must be `draft`.
    pub fn new_task(&self, node: &str, title: &str, agent: bool) -> String {
        let outcome = task_new(
            &self.env(),
            &self.globals(),
            &TaskNewRequest {
                nodes: vec![node.to_owned()],
                title: Some(title.to_owned()),
                goal: Some(format!("{title}: make it so.")),
                author_role: agent.then(|| "developer".to_owned()),
                author_model: agent.then(|| "claude-opus-5-5".to_owned()),
                run: agent.then(|| "run-7".to_owned()),
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
        );
        let outcome = done(outcome, &format!("task new {node}"));
        assert_eq!(outcome.status, Some(TaskStatus::Draft), "{outcome:?}");
        outcome.id.expect("the new task's ID")
    }

    /// The owner's `approve`, `changes` (`note`) or `cancel`, consent yes,
    /// asked once; it must succeed into `want`.
    pub fn owner(&self, action: &str, id: &str, note: Option<&str>, want: TaskStatus) {
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            true
        };
        let request = TaskDecisionRequest {
            id: id.to_owned(),
            note: note.map(str::to_owned),
            now: CLOCK.to_owned(),
            git: self.git_env(),
        };
        let (env, globals) = (self.env(), self.globals());
        let outcome = match action {
            "approve" => task_approve(&env, &globals, &request, &mut consent),
            "changes" => task_changes(&env, &globals, &request, &mut consent),
            "cancel" => task_cancel(&env, &globals, &request, &mut consent),
            other => panic!("no owner command {other}"),
        };
        let outcome = done(outcome, &format!("task {action} {id}"));
        assert_eq!(
            outcome.status,
            Some(want),
            "task {action} {id}: {outcome:?}"
        );
        assert_eq!(questions.len(), 1, "task {action} {id}: {questions:?}");
    }

    /// `spec task plan ID` with `criteria` and `affected`: `review`.
    pub fn plan(&self, id: &str, criteria: &[&str], affected: &[&str]) {
        let outcome = task_plan(
            &self.env(),
            &self.globals(),
            &TaskPlanRequest {
                id: id.to_owned(),
                plan: ProposedText::Given(b"1. Read the target.\n2. Change it.\n".to_vec()),
                criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
                affected: affected.iter().map(|a| (*a).to_owned()).collect(),
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
        );
        let outcome = done(outcome, &format!("task plan {id}"));
        assert_eq!(outcome.status, Some(TaskStatus::Review), "{outcome:?}");
    }

    /// `spec task claim ID --role developer --worktree <root>`.
    pub fn claim(&self, id: &str) {
        let outcome = task_claim(
            &self.env(),
            &self.globals(),
            &TaskClaimRequest {
                id: id.to_owned(),
                role: "developer".to_owned(),
                worktree: self.root.clone(),
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
        );
        let outcome = done(outcome, &format!("task claim {id}"));
        assert_eq!(outcome.status, Some(TaskStatus::InProgress), "{outcome:?}");
        assert_eq!(outcome.run, Some(1), "{outcome:?}");
    }

    /// `spec task report ID --outcome completed …`: run 1 closed.
    pub fn report(&self, id: &str) {
        let outcome = task_report(
            &self.env(),
            &self.globals(),
            &TaskReportRequest {
                id: id.to_owned(),
                outcome: "completed".to_owned(),
                summary: "Done as planned.".to_owned(),
                changed_files: vec!["src/a.txt".to_owned()],
                now: CLOCK.to_owned(),
                git: self.git_env(),
            },
        );
        let outcome = done(outcome, &format!("task report {id}"));
        assert_eq!(outcome.status, Some(TaskStatus::InProgress), "{outcome:?}");
        assert_eq!(outcome.run, Some(1), "{outcome:?}");
    }

    /// A developer's question on `node` with a working answer, bound to
    /// `task`; asked again naming every hit when its first ask hits. Its
    /// ID.
    pub fn ask(&self, node: &str, task: Option<&str>) -> String {
        let mut request = QuestionRequest {
            node_ids: vec![node.to_owned()],
            text: "Does the change keep the old behaviour at zero?".to_owned(),
            working_answer: "Keep it until the owner answers.".to_owned(),
            price_of_other: "A new rule case.".to_owned(),
            severity: None,
            distinct_from: Vec::new(),
            author_role: Some("developer".to_owned()),
            author_model: Some("claude-opus-5-5".to_owned()),
            run: None,
            now: CLOCK.to_owned(),
            git: self.git_env(),
        };
        let ask = |request: &QuestionRequest| {
            propose_question_with_task(&self.env(), &self.globals(), request, task)
                .unwrap_or_else(|error| panic!("ask on {node}: {error}"))
        };
        let mut outcome = ask(&request);
        if outcome.refusal.is_none() && !outcome.document.created {
            request.distinct_from = outcome
                .document
                .hits
                .iter()
                .map(|hit| hit.name().to_owned())
                .collect();
            outcome = ask(&request);
        }
        assert_eq!(outcome.exit(), Exit::Answered, "ask on {node}: {outcome:?}");
        assert!(outcome.document.created, "not stored: {outcome:?}");
        outcome.document.id.clone().expect("the question's ID")
    }
}

/// The call succeeded (exit 0): its outcome.
pub fn done(outcome: Result<TaskOutcome, CliError>, context: &str) -> TaskOutcome {
    let outcome = outcome.unwrap_or_else(|error| panic!("{context}: exit 2: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
    outcome
}

/// State S on `root` (a committed git copy of `case.fixture`) under
/// `home`.
pub fn build(scratch: &Scratch, case: &Case, root: &Path, home: &Path) {
    let calls = Calls::new(scratch, root, home);
    assert_eq!(calls.new_task(case.draft, "A draft", false), "T-0001");

    assert_eq!(
        calls.new_task(case.stale, "An edited target", false),
        "T-0002"
    );
    calls.owner("approve", "T-0002", None, TaskStatus::Ready);
    replace(root, case.path, case.from, case.to);
    calls
        .git
        .run(root, &["commit", "-q", "-am", "edit the target"]);

    assert_eq!(
        calls.new_task(case.target, "The whole life", true),
        "T-0003"
    );
    calls.plan("T-0003", &[case.criterion, "The old behaviour holds."], &[]);
    calls.owner(
        "changes",
        "T-0003",
        Some("Name the affected node."),
        TaskStatus::ChangesRequested,
    );
    calls.plan(
        "T-0003",
        &[case.criterion, "The old behaviour holds."],
        &[case.affected],
    );
    calls.owner("approve", "T-0003", None, TaskStatus::Ready);
    assert_eq!(calls.ask(case.target, Some("T-0003")), "PR-0001");
    calls.claim("T-0003");
    calls.report("T-0003");

    assert_eq!(calls.new_task(case.draft, "Dropped", false), "T-0004");
    calls.owner("cancel", "T-0004", None, TaskStatus::Cancelled);
    assert_eq!(
        calls.git.run(root, &["status", "--porcelain", "--ignored"]),
        "",
        "S leaves the copy committed"
    );
}

/// A git copy of `case.fixture` at `<scratch>/<dir>` in state S under
/// `home`.
pub fn repo_in_s(scratch: &Scratch, case: &Case, dir: &str, home: &Path) -> PathBuf {
    let root = scratch.repo(case.fixture, dir, case.branch);
    build(scratch, case, &root, home);
    root
}
