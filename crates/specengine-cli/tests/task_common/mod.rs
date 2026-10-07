//! Helpers of the docs/features/task-package.md tests ("Acceptance
//! criteria", Setup): the pairs of `common::proposal` (a scratch git
//! repository of `fixtures/spec-a` or `fixtures/spec-b`, its main worktree
//! on `main`, a linked one on `t1`, a scratch `HOME`, every git process in
//! the sandbox of `common::git`, git auto-maintenance off), every `spec
//! task` command through the library at an injected clock, the owner's
//! three with a consent callback that records the question; the binary
//! only where it is the subject (the terminal check, `--json` on stdout).
//!
//! Only the test binaries of the slice include this module (`mod
//! task_common;`), so the other CLI test binaries are not rebuilt.

#![allow(dead_code)]

use std::path::Path;

use serde_json::Value;
use specengine_cli::{
    CliError, DiscrepancyInput, DiscrepancyRequest, Exit, Globals, IntakeOutcome, Outcome,
    ProposalOutcome, ProposedText, QuestionRequest, TaskClaimRequest, TaskCompleteRequest,
    TaskDecisionRequest, TaskListOutcome, TaskListRequest, TaskNewRequest, TaskOutcome,
    TaskPlanRequest, TaskReportRequest, TaskShowOutcome, TaskShowRequest,
    propose_discrepancy_with_task, propose_question_with_task, propose_with_task, render_json,
    render_text, task_approve, task_cancel, task_changes, task_claim, task_complete, task_list,
    task_new, task_plan, task_report, task_show,
};

use crate::common::decision::hit_names;
use crate::common::proposal::{NOW, Pair};

/// The clock of every task command unless a test names another.
pub const CLOCK: &str = "2026-10-07T09:00:00Z";

/// The package's 25 keys, in order (docs/canon/task-package.md
/// "Package").
pub const PACKAGE_KEYS: [&str; 25] = [
    "schema_version",
    "id",
    "project",
    "status",
    "title",
    "goal",
    "profile",
    "stale",
    "targets",
    "criteria",
    "affected_nodes",
    "plan",
    "assumptions",
    "open_proposals",
    "owner_notes",
    "bindings",
    "spec_snapshot",
    "snapshot_diff",
    "claim",
    "runs",
    "bundle",
    "author",
    "created_at",
    "updated_at",
    "notes",
];

/// A JSON value with every object's keys in the order written (this
/// crate's `serde_json` sorts a `Value`'s keys).
#[derive(Debug, Clone, PartialEq)]
pub enum Ordered {
    Object(Vec<(String, Ordered)>),
    Array(Vec<Ordered>),
    Scalar,
}

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Seen;
        impl<'de> serde::de::Visitor<'de> for Seen {
            type Value = Ordered;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("JSON")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Ordered, M::Error> {
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Ordered>()? {
                    entries.push((key, value));
                }
                Ok(Ordered::Object(entries))
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> Result<Ordered, S::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<Ordered>()? {
                    items.push(item);
                }
                Ok(Ordered::Array(items))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_i64<E>(self, _: i64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_u64<E>(self, _: u64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_f64<E>(self, _: f64) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_str<E>(self, _: &str) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Scalar)
            }
        }
        deserializer.deserialize_any(Seen)
    }
}

impl Ordered {
    /// Parsed from JSON text.
    pub fn of(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
    }

    /// Its keys, in the order written (an object only).
    pub fn keys(&self) -> Vec<String> {
        match self {
            Self::Object(entries) => entries.iter().map(|(key, _)| key.clone()).collect(),
            other => panic!("not an object: {other:?}"),
        }
    }

    /// The value of `key` (an object only).
    pub fn get(&self, key: &str) -> &Self {
        match self {
            Self::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("no key {key}")),
            other => panic!("not an object: {other:?}"),
        }
    }

    /// Item `index` (an array only).
    pub fn at(&self, index: usize) -> &Self {
        match self {
            Self::Array(items) => &items[index],
            other => panic!("not an array: {other:?}"),
        }
    }

    /// Every key of every object in it, as `a.b[].c` paths, in order.
    pub fn paths(&self, prefix: &str, out: &mut Vec<String>) {
        match self {
            Self::Object(entries) => {
                for (key, value) in entries {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    out.push(path.clone());
                    value.paths(&path, out);
                }
            }
            Self::Array(items) => {
                for item in items {
                    item.paths(&format!("{prefix}[]"), out);
                }
            }
            Self::Scalar => {}
        }
    }
}

/// The keys of the JSON object `text`, in the order written.
pub fn keys(text: &str) -> Vec<String> {
    Ordered::of(text).keys()
}

/// The `spec task` commands of one pair, at [`CLOCK`] unless given.
pub trait Tasks {
    fn pair(&self) -> &Pair;

    fn new_task_at(
        &self,
        cwd: &Path,
        nodes: &[&str],
        title: Option<&str>,
        goal: Option<&str>,
        now: &str,
    ) -> Result<TaskOutcome, CliError> {
        let pair = self.pair();
        task_new(
            &pair.env(cwd),
            &Globals::default(),
            &TaskNewRequest {
                nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
                title: title.map(str::to_owned),
                goal: goal.map(str::to_owned),
                author_role: None,
                author_model: None,
                run: None,
                now: now.to_owned(),
                git: pair.git_env(cwd),
            },
        )
    }

    fn new_task(
        &self,
        cwd: &Path,
        nodes: &[&str],
        title: Option<&str>,
        goal: Option<&str>,
    ) -> Result<TaskOutcome, CliError> {
        self.new_task_at(cwd, nodes, title, goal, CLOCK)
    }

    /// A `draft` task that must be made: its ID.
    fn new_ok(&self, cwd: &Path, nodes: &[&str]) -> String {
        let outcome = self
            .new_task(
                cwd,
                nodes,
                Some("Tune the regeneration"),
                Some("Make it so."),
            )
            .unwrap_or_else(|error| panic!("task new {nodes:?}: {error}"));
        assert_eq!(outcome.exit(), Exit::Answered, "task new: {outcome:?}");
        outcome.id.expect("an ID")
    }

    fn show(&self, cwd: &Path, id: &str) -> Result<TaskShowOutcome, CliError> {
        let pair = self.pair();
        task_show(
            &pair.env(cwd),
            &Globals::default(),
            &TaskShowRequest {
                id: Some(id.to_owned()),
                next: false,
                git: pair.git_env(cwd),
            },
        )
    }

    fn show_next(&self, cwd: &Path) -> Result<TaskShowOutcome, CliError> {
        let pair = self.pair();
        task_show(
            &pair.env(cwd),
            &Globals::default(),
            &TaskShowRequest {
                id: None,
                next: true,
                git: pair.git_env(cwd),
            },
        )
    }

    fn show_ok(&self, cwd: &Path, id: &str) -> TaskShowOutcome {
        let outcome = self
            .show(cwd, id)
            .unwrap_or_else(|error| panic!("task show {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "task show {id}: {outcome:?}"
        );
        outcome
    }

    /// `task show --json` as printed.
    fn package_text(&self, cwd: &Path, id: &str) -> String {
        render_json(&Outcome::TaskShow(Box::new(self.show_ok(cwd, id))))
    }

    /// `task show --json`, parsed.
    fn package(&self, cwd: &Path, id: &str) -> Value {
        serde_json::from_str(&self.package_text(cwd, id)).expect("one JSON document")
    }

    /// `task show` (the brief) as printed.
    fn brief(&self, cwd: &Path, id: &str) -> String {
        render_text(&Outcome::TaskShow(Box::new(self.show_ok(cwd, id))))
    }

    fn list(&self, cwd: &Path) -> TaskListOutcome {
        let pair = self.pair();
        task_list(
            &pair.env(cwd),
            &Globals::default(),
            &TaskListRequest {
                statuses: Vec::new(),
                git: pair.git_env(cwd),
            },
        )
        .unwrap_or_else(|error| panic!("task list: {error}"))
    }

    /// `task list --json`, parsed.
    fn list_json(&self, cwd: &Path) -> Value {
        serde_json::from_str(&render_json(&Outcome::TaskList(self.list(cwd)))).expect("JSON")
    }

    /// The owner's `approve`, `changes` (`note`) or `cancel` from `cwd`,
    /// consent `answer`: the outcome and the questions asked.
    fn owner(
        &self,
        cwd: &Path,
        action: &str,
        id: &str,
        note: Option<&str>,
        answer: bool,
        now: &str,
    ) -> (Result<TaskOutcome, CliError>, Vec<String>) {
        let pair = self.pair();
        let mut questions = Vec::new();
        let mut consent = |question: &str| {
            questions.push(question.to_owned());
            answer
        };
        let request = TaskDecisionRequest {
            id: id.to_owned(),
            note: note.map(str::to_owned),
            now: now.to_owned(),
            git: pair.git_env(cwd),
        };
        let (env, globals) = (pair.env(cwd), Globals::default());
        let outcome = match action {
            "approve" => task_approve(&env, &globals, &request, &mut consent),
            "changes" => task_changes(&env, &globals, &request, &mut consent),
            "cancel" => task_cancel(&env, &globals, &request, &mut consent),
            other => panic!("no owner command {other}"),
        };
        (outcome, questions)
    }

    /// Approve with consent yes; it must succeed.
    fn approve_task(&self, cwd: &Path, id: &str) -> TaskOutcome {
        let (outcome, questions) = self.owner(cwd, "approve", id, None, true, CLOCK);
        let outcome = outcome.unwrap_or_else(|error| panic!("task approve {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "task approve {id}: {outcome:?}"
        );
        assert_eq!(questions.len(), 1, "{questions:?}");
        outcome
    }

    fn plan_at(
        &self,
        cwd: &Path,
        id: &str,
        plan: &str,
        criteria: &[&str],
        affected: &[&str],
        now: &str,
    ) -> Result<TaskOutcome, CliError> {
        let pair = self.pair();
        task_plan(
            &pair.env(cwd),
            &Globals::default(),
            &TaskPlanRequest {
                id: id.to_owned(),
                plan: ProposedText::Given(plan.as_bytes().to_vec()),
                criteria: criteria.iter().map(|c| (*c).to_owned()).collect(),
                affected: affected.iter().map(|a| (*a).to_owned()).collect(),
                now: now.to_owned(),
                git: pair.git_env(cwd),
            },
        )
    }

    fn plan(
        &self,
        cwd: &Path,
        id: &str,
        plan: &str,
        criteria: &[&str],
        affected: &[&str],
    ) -> Result<TaskOutcome, CliError> {
        self.plan_at(cwd, id, plan, criteria, affected, CLOCK)
    }

    fn plan_ok(&self, cwd: &Path, id: &str, criteria: &[&str], affected: &[&str]) -> TaskOutcome {
        let outcome = self
            .plan(cwd, id, "1. Read.\n2. Change.\n", criteria, affected)
            .unwrap_or_else(|error| panic!("task plan {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "task plan {id}: {outcome:?}"
        );
        outcome
    }

    fn claim(
        &self,
        cwd: &Path,
        id: &str,
        role: &str,
        worktree: &Path,
    ) -> Result<TaskOutcome, CliError> {
        let pair = self.pair();
        task_claim(
            &pair.env(cwd),
            &Globals::default(),
            &TaskClaimRequest {
                id: id.to_owned(),
                role: role.to_owned(),
                worktree: worktree.to_path_buf(),
                now: CLOCK.to_owned(),
                git: pair.git_env(cwd),
            },
        )
    }

    fn claim_ok(&self, cwd: &Path, id: &str, worktree: &Path) -> TaskOutcome {
        let outcome = self
            .claim(cwd, id, "developer", worktree)
            .unwrap_or_else(|error| panic!("task claim {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "task claim {id}: {outcome:?}"
        );
        outcome
    }

    fn report(
        &self,
        cwd: &Path,
        id: &str,
        outcome: &str,
        summary: &str,
        changed: &[&str],
    ) -> Result<TaskOutcome, CliError> {
        let pair = self.pair();
        task_report(
            &pair.env(cwd),
            &Globals::default(),
            &TaskReportRequest {
                id: id.to_owned(),
                outcome: outcome.to_owned(),
                summary: summary.to_owned(),
                changed_files: changed.iter().map(|file| (*file).to_owned()).collect(),
                now: CLOCK.to_owned(),
                git: pair.git_env(cwd),
            },
        )
    }

    fn report_ok(&self, cwd: &Path, id: &str) -> TaskOutcome {
        let outcome = self
            .report(cwd, id, "completed", "Done as planned.", &["src/a.txt"])
            .unwrap_or_else(|error| panic!("task report {id}: {error}"));
        assert_eq!(
            outcome.exit(),
            Exit::Answered,
            "task report {id}: {outcome:?}"
        );
        outcome
    }

    fn complete(&self, cwd: &Path, id: &str) -> Result<TaskOutcome, CliError> {
        let pair = self.pair();
        task_complete(
            &pair.env(cwd),
            &Globals::default(),
            &TaskCompleteRequest {
                id: id.to_owned(),
                now: CLOCK.to_owned(),
                git: pair.git_env(cwd),
            },
        )
    }

    /// The queue's oracle: `dump()` (every row of every table).
    fn dump(&self) -> String {
        self.pair().queue().dump().expect("dump")
    }

    /// `(type, payload)` of every `task.*` event, in `seq` order.
    fn task_events(&self) -> Vec<(String, Value)> {
        self.pair()
            .events()
            .into_iter()
            .filter(|event| event.event_type.starts_with("task."))
            .map(|event| (event.event_type, event.payload))
            .collect()
    }
}

impl Tasks for Pair {
    fn pair(&self) -> &Pair {
        self
    }
}

/// The command was refused (exit 1): its reason (the last note too).
pub fn refusal(outcome: &Result<TaskOutcome, CliError>, context: &str) -> String {
    match outcome {
        Ok(outcome) => {
            assert_eq!(outcome.exit(), Exit::NotFound, "{context}: {outcome:?}");
            assert_eq!(outcome.exit().code(), 1, "{context}");
            let reason = outcome.refusal.clone().expect("a refusal");
            assert_eq!(
                outcome.notes.last(),
                Some(&reason),
                "{context}: {outcome:?}"
            );
            reason
        }
        Err(error) => panic!("{context}: exit {:?}: {error}", error.exit),
    }
}

/// The command succeeded (exit 0): the outcome.
pub fn done(outcome: Result<TaskOutcome, CliError>, context: &str) -> TaskOutcome {
    let outcome = outcome.unwrap_or_else(|error| panic!("{context}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{context}: {outcome:?}");
    outcome
}

/// The printed line of a task command.
pub fn line(outcome: &TaskOutcome) -> String {
    render_text(&Outcome::Task(Box::new(outcome.clone())))
}

/// The `--json` document of a task command, parsed.
pub fn document(outcome: &TaskOutcome) -> Value {
    serde_json::from_str(&render_json(&Outcome::Task(Box::new(outcome.clone())))).expect("JSON")
}

/// A question of a `developer` from `cwd` with `--task` `task` (none:
/// unbound): the outcome of the ask that stores it (when its first ask
/// hits, asked again naming every hit in `distinct_from`), or the refusal.
pub fn ask(
    pair: &Pair,
    cwd: &Path,
    targets: &[&str],
    text: &str,
    working_answer: &str,
    task: Option<&str>,
) -> IntakeOutcome {
    let mut request = QuestionRequest {
        node_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
        text: text.to_owned(),
        working_answer: working_answer.to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        now: NOW.to_owned(),
        git: pair.git_env(cwd),
    };
    let run = |request: &QuestionRequest| {
        propose_question_with_task(&pair.env(cwd), &Globals::default(), request, task)
            .unwrap_or_else(|error| panic!("ask: {error}"))
    };
    let first = run(&request);
    if first.refusal.is_some() || first.document.created {
        return first;
    }
    request.distinct_from = hit_names(&first);
    run(&request)
}

/// [`ask`] that must store: its ID.
pub fn ask_ok(
    pair: &Pair,
    cwd: &Path,
    targets: &[&str],
    text: &str,
    working_answer: &str,
    task: Option<&str>,
) -> String {
    let outcome = ask(pair, cwd, targets, text, working_answer, task);
    assert_eq!(outcome.exit(), Exit::Answered, "ask: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// A discrepancy of a `developer` from `cwd` with `--task` `task`, as
/// [`ask`] stores a question.
pub fn report_gap(
    pair: &Pair,
    cwd: &Path,
    input: DiscrepancyInput,
    task: Option<&str>,
) -> IntakeOutcome {
    let run = |input: DiscrepancyInput| {
        propose_discrepancy_with_task(
            &pair.env(cwd),
            &Globals::default(),
            &DiscrepancyRequest {
                input,
                author_role: Some("developer".to_owned()),
                author_model: Some("claude-opus-5-5".to_owned()),
                run: None,
                now: NOW.to_owned(),
                git: pair.git_env(cwd),
            },
            task,
        )
        .unwrap_or_else(|error| panic!("report: {error}"))
    };
    let first = run(input.clone());
    if first.refusal.is_some() || first.document.created {
        return first;
    }
    let mut named = input;
    named.distinct_from = Some(hit_names(&first));
    run(named)
}

/// An update of `target`'s span read in `cwd` with `from` replaced by
/// `to`, from `cwd`, with `--task` `task`.
pub fn propose_bound(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    from: &str,
    to: &str,
    task: Option<&str>,
) -> ProposalOutcome {
    let (hash, text) = pair.span(cwd, target);
    let request = pair.request(
        cwd,
        target,
        &hash,
        &crate::common::proposal::edit(&text, from, to),
    );
    propose_with_task(&pair.env(cwd), &Globals::default(), &request, task)
        .unwrap_or_else(|error| panic!("propose {target}: {error}"))
}

/// [`propose_bound`] that must store: its ID.
pub fn propose_bound_ok(
    pair: &Pair,
    cwd: &Path,
    target: &str,
    from: &str,
    to: &str,
    task: Option<&str>,
) -> String {
    let outcome = propose_bound(pair, cwd, target, from, to, task);
    assert_eq!(
        outcome.exit(),
        Exit::Answered,
        "propose {target}: {outcome:?}"
    );
    outcome.document.id.clone().expect("an ID")
}
