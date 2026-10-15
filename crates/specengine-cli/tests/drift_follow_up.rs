//! docs/features/drift-follow-up.md (4c-2), the CLI's half: `spec reject PR
//! --reason T --follow-up code|spec` on a drift item. TE-1 and the named
//! RD-1, RD-2, RD-3 and RD-5 tests, over scratch repositories of real
//! fixtures made through `tests/drift_common/` (`drifted`, `ecs_drifted`,
//! `golden`):
//!
//! - `fixtures/code-a` with its golden lock: `PIECE-COUNTER`'s item
//!   (a `spec_ahead marker_behind` row) and, `CAP` changed, `LAW-BOUND`'s
//!   (`code_ahead code_changed`);
//! - `fixtures/verify-ecs`, an ECS-style game crate (systems, a plugin,
//!   RON tuning) with its four nodes accepted, `PERK-REGEN`'s system
//!   changed: a `code_ahead` item of another project (ADR-0008).
//!
//! The expected title and goal are built here from the spec's "Title and
//! goal" (`Texts`); the rows' lines are `spec review`'s (the CLI's one row
//! formatter), never the follow-up's own output. The follow-up runs
//! through the library seam Q8 names (`reject_follow_up`, `FollowUp`) with
//! every question recorded, or as the binary on a pseudo-terminal where
//! the exit code, stdout or stderr is the subject. The layout the spec
//! leaves open is read loosely (the goal's parts each on lines of their
//! own, the prompt holding the title, the goal's lines and the question in
//! that order) except in one golden of the implementation's bytes
//! (`follow_up_task_is_draft`: the goal's rows indented two spaces, the
//! prompt's `draft task:` and `goal:` lines). The readings the
//! implementation adds to the spec (rust-developer, iteration 1,
//! "Deviations") are pinned in their own tests, each named in its doc.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::fs;
use std::path::{Path, PathBuf};

use drift_common::{
    Asked, COUNTER, COUNTER_MD, DECIDED, RAISED, STAMINA, cap_changed, drifted, dump, ecs_drifted,
    proposal, queue_of, raise_at, raised, reject_at, review_of, sql,
};
use lock_common::{DECIDER, LockRepo, RepoState};
use serde_json::{Value, json};
use specengine_cli::{
    CliError, Exit, FollowUp, GOAL_MAX, GitEnv, Globals, LockAcceptRequest, LockPrompt, Outcome,
    ProposalOutcome, QuestionRequest, RejectRequest, StageBody, StageRequest, TITLE_MAX,
    TaskDecisionRequest, TaskShowRequest, TaskStatus, lock_accept, propose_question,
    reject_follow_up, render_json, render_text, stage, task_approve, task_show,
};
use specengine_core::proposal::Author;
use specengine_store::{Decision, NewTask, ProposalQueue as _, ProposalStatus, Task};
use verify_common::{STAMINA_MD, golden};

const SLUG: &str = "code-a";
const ECS_SLUG: &str = "verify-ecs";

/// The owner's reason of every follow-up unless a test names another.
const REASON: &str = "The cap is the spec; the code follows it.";

/// The clock of the owner's task approve, after [`DECIDED`].
const LATER: &str = "2026-10-14T11:00:00Z";

/// A follow-up's kind (`--follow-up code|spec`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Code,
    Spec,
}

impl Kind {
    const BOTH: [Self; 2] = [Self::Code, Self::Spec];

    /// The flag's value.
    fn word(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Spec => "spec",
        }
    }

    /// The library's follow-up kind (Q8: `FollowUp`).
    fn api(self) -> FollowUp {
        match self {
            Self::Code => FollowUp::Code,
            Self::Spec => FollowUp::Spec,
        }
    }
}

// ---------------------------------------------------------- the seam

/// `spec reject ID --reason R --follow-up KIND` from `cwd` with the
/// caller's git `git`, at [`DECIDED`]; `consent` answers the question.
fn follow_up_with(
    repo: &LockRepo,
    cwd: &Path,
    git: GitEnv,
    id: &str,
    kind: Kind,
    reason: &str,
    consent: &mut dyn FnMut(&str) -> bool,
) -> Result<ProposalOutcome, CliError> {
    // The library seam (Q8: `reject_follow_up` beside `reject`).
    reject_follow_up(
        &repo.env(cwd),
        &Globals::default(),
        &RejectRequest {
            id: id.to_owned(),
            reason: Some(reason.to_owned()),
            now: DECIDED.to_owned(),
            git,
        },
        kind.api(),
        consent,
    )
}

/// [`follow_up_with`] from `cwd`, the owner answering `answer`: the
/// outcome and every question asked.
fn follow_up_at(
    repo: &LockRepo,
    cwd: &Path,
    id: &str,
    kind: Kind,
    reason: &str,
    answer: bool,
) -> Asked {
    let mut questions = Vec::new();
    let outcome = follow_up_with(
        repo,
        cwd,
        repo.git_env(cwd),
        id,
        kind,
        reason,
        &mut |q: &str| {
            questions.push(q.to_owned());
            answer
        },
    );
    Asked { outcome, questions }
}

/// A follow-up from the root with [`REASON`], `y`.
fn follow_up(repo: &LockRepo, id: &str, kind: Kind) -> Asked {
    follow_up_at(repo, &repo.root, id, kind, REASON, true)
}

// -------------------------------------------------- the expected texts

/// What the spec's "Title and goal" render for one item: its kind, the
/// node's ID, revision and path now (step 1), and the item's ID.
struct Texts<'a> {
    kind: Kind,
    node: &'a str,
    rev: u32,
    path: &'a str,
    id: &'a str,
}

impl Texts<'_> {
    fn title(&self) -> String {
        let (node, id) = (self.node, self.id);
        match self.kind {
            Kind::Code => format!("Bring the code bound to {node} in line with its spec ({id})"),
            Kind::Spec => format!("Propose a spec edit of {node} matching its bound code ({id})"),
        }
    }

    /// The goal's first sentence.
    fn head(&self) -> String {
        let (node, rev, path) = (self.node, self.rev, self.path);
        match self.kind {
            Kind::Code => format!(
                "{node} rev {rev} ({path}) is the spec: change the bound code below to match it \
                 and set its markers to @{rev}; the owner then locks them (`spec lock accept \
                 {node}`, or the approve of a new drift item)."
            ),
            Kind::Spec => format!(
                "The bound code below is right: propose an edit of {node} rev {rev} ({path}) \
                 describing it; once applied, set its markers to the new revision, and the owner \
                 accepts the lock."
            ),
        }
    }

    /// The goal's lines: the sentence, the owner's reason, `rows`, then
    /// `<more> more row(s): spec review <id>` when `more` is not 0.
    fn goal(&self, reason: &str, rows: &[String], more: usize) -> Vec<String> {
        let mut lines = vec![self.head(), format!("Owner's reason: {reason}")];
        lines.extend(rows.iter().cloned());
        if more > 0 {
            lines.push(more_line(more, self.id));
        }
        lines
    }

    /// Plain reject's question with ` and make a draft task (<kind>)`
    /// before `?` (step 2); the item recorded in `worktree` on `main`.
    fn question(&self, worktree: &Path, approved: bool) -> String {
        format!(
            "reject {}{} ({} in spec.lock on main in {}) and make a draft task ({})? [y/N]",
            if approved { "approved " } else { "" },
            self.id,
            self.node,
            worktree.display(),
            self.kind.word()
        )
    }
}

fn more_line(more: usize, id: &str) -> String {
    format!("{more} more row(s): spec review {id}")
}

/// Step 4's note naming the task made.
fn note_of(task: &str, id: &str) -> String {
    format!("created {task} (draft) for {id}: spec task approve {task} makes it ready")
}

/// Q11's note on a `Proposal:` commit that does not complete the item.
fn carried_note(commit: &str, id: &str, why: &str) -> String {
    format!(
        "the commit {commit} on `main` carries `Proposal: {id}` but does not complete it \
         ({why}); rejected all the same"
    )
}

/// `text`'s lines trimmed, blank ones left out.
fn lines_of(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Asserts `text` holds every part, each after the one before it.
fn assert_in_order(text: &str, parts: &[String], context: &str) {
    let mut from = 0;
    for part in parts {
        match text[from..].find(part.as_str()) {
            Some(at) => from += at + part.len(),
            None => panic!("{context}: {part:?} not found in order in:\n{text}"),
        }
    }
}

// ---------------------------------------------------------- reading

/// The ID of the one item `raised` created for `node`.
fn item_of(raised: &drift_common::Raised, node: &str) -> String {
    raised
        .created()
        .into_iter()
        .find(|(_, created)| created == node)
        .unwrap_or_else(|| panic!("no item for {node}: {}", raised.json_text()))
        .0
}

/// The item's rows as `spec review ID` prints them (the one row
/// formatter), unindented; its `… and <n> more` line left out.
fn review_rows(repo: &LockRepo, id: &str) -> Vec<String> {
    let (text, _, _) = review_of(repo, id);
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("drift: "))
        .unwrap_or_else(|| panic!("no drift line: {text}"));
    lines[at + 1..]
        .iter()
        .take_while(|line| line.starts_with("  "))
        .map(|line| line.trim())
        .filter(|line| !line.starts_with('\u{2026}'))
        .map(str::to_owned)
        .collect()
}

/// The project's tasks, by number.
fn tasks_of(repo: &LockRepo, slug: &str) -> Vec<Task> {
    queue_of(repo, slug)
        .map(|queue| queue.list_tasks(None).expect("tasks").tasks)
        .unwrap_or_default()
}

/// The one task of the project.
fn only_task(repo: &LockRepo, slug: &str) -> Task {
    let mut tasks = tasks_of(repo, slug);
    assert_eq!(tasks.len(), 1, "{tasks:?}");
    tasks.remove(0)
}

fn task_of(repo: &LockRepo, slug: &str, id: &str) -> Task {
    queue_of(repo, slug)
        .expect("a queue")
        .get_task(id)
        .expect("get_task")
        .unwrap_or_else(|| panic!("no task {id}"))
}

/// The queue's events, `(type, payload, at)`.
fn events(repo: &LockRepo, slug: &str) -> Vec<(String, Value, String)> {
    queue_of(repo, slug)
        .expect("a queue")
        .events()
        .expect("events")
        .into_iter()
        .map(|event| (event.event_type, event.payload, event.at))
        .collect()
}

/// Every `proposal.rejected` payload as stored, oldest first.
fn rejected_payloads(repo: &LockRepo, slug: &str) -> Vec<String> {
    sql(
        repo,
        slug,
        "SELECT payload FROM events WHERE type = 'proposal.rejected' ORDER BY seq;",
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// stdout as `spec reject` prints it.
fn printed(outcome: &ProposalOutcome) -> String {
    render_text(&Outcome::Proposal(Box::new(outcome.clone())))
}

/// The `--json` document's `notes`.
fn json_notes(outcome: &ProposalOutcome) -> Vec<String> {
    let document: Value =
        serde_json::from_str(&render_json(&Outcome::Proposal(Box::new(outcome.clone()))))
            .expect("the document");
    document["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .map(|note| note.as_str().expect("a note").to_owned())
        .collect()
}

/// Asserts `note` is on stderr (`note: …`) and in the `--json` notes.
fn assert_noted(asked: &Asked, note: &str, context: &str) {
    let outcome = asked.outcome.as_ref().expect("an answer");
    let line = format!("note: {note}");
    assert!(
        asked.stderr().contains(&line),
        "{context}: {line:?} not on stderr: {:?}",
        asked.stderr()
    );
    assert!(
        json_notes(outcome).iter().any(|text| text == note),
        "{context}: {note:?} not in notes: {:?}",
        json_notes(outcome)
    );
}

/// What a refused follow-up leaves as it was: the queue (proposals,
/// tasks, runs, events: its dump) and the repository at `dir` (`HEAD`, the
/// refs, `git status`, the index, every file outside `.git`).
fn frozen(repo: &LockRepo, slug: &str, dir: &Path) -> (String, RepoState) {
    let files = common::snapshot(dir)
        .into_iter()
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .collect();
    (
        dump(repo, slug),
        RepoState {
            head: repo.git.head(dir),
            refs: repo
                .git
                .git_text(dir, &["for-each-ref", "--format=%(refname) %(objectname)"]),
            status: repo.git.git(
                dir,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            ),
            index: repo.git.git_text(dir, &["ls-files", "-s"]),
            files,
        },
    )
}

// ------------------------------------------------------------- repos

/// A linked worktree `name` of `repo` on a new branch `name`, canonical.
fn worktree(repo: &LockRepo, name: &str) -> PathBuf {
    let path = repo.scratch.join(name);
    repo.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        name,
        path.to_str().expect("a UTF-8 path"),
    ]);
    fs::canonicalize(path).expect("the linked worktree")
}

/// `git add -A` and a commit without hooks in `dir`.
fn commit_in(repo: &LockRepo, dir: &Path, message: &str) {
    repo.git.add_all(dir);
    repo.git.commit(dir, message);
}

/// `T` added to the `[ids]` of `dir`'s `specengine.toml`.
fn add_t_prefix(dir: &Path) {
    let config = common::read_text(dir, "specengine.toml");
    common::write(
        dir,
        "specengine.toml",
        common::with_ids_line(&config, "T     = { kind = \"thing\", width = 2 }\n"),
    );
}

/// The lock commit a killed `spec approve` of `id` left: the library
/// accept of `node` naming the item and `main`, consent yes, not recorded.
fn killed_lock_commit(repo: &LockRepo, node: &str, id: &str) -> String {
    let mut consent = |_: &LockPrompt<'_>| true;
    let outcome = lock_accept(
        &repo.env(&repo.root),
        &Globals::default(),
        &LockAcceptRequest {
            reference: node.to_owned(),
            editorial: false,
            now: DECIDED.to_owned(),
            git: repo.git_env(&repo.root),
            proposal: Some(id.to_owned()),
            branch: Some("main".to_owned()),
        },
        &mut consent,
        &[],
    )
    .expect("the accept");
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.commit.expect("a commit")
}

/// `git commit` (`amend`: `--amend`) with `message` from a scratch file.
fn commit_message(repo: &LockRepo, message: &str, amend: bool) -> String {
    let file = repo.scratch.join("message.txt");
    fs::write(&file, message).expect("the message");
    let file = file.to_str().expect("a UTF-8 path").to_owned();
    let mut args = vec!["commit", "-q", "--cleanup=verbatim", "-F", file.as_str()];
    if amend {
        args.push("--amend");
    }
    repo.git(&args);
    repo.head()
}

/// A `Proposal: <id>` commit on `main` that does not complete the item
/// (Q11), and why: `regen` (its lock commit's `Lock-mode: regen`), `none`
/// (no `Lock-mode`), `deleted` (every trailer, `spec.lock` deleted).
fn non_completing(repo: &LockRepo, id: &str, variant: &str) -> (String, &'static str) {
    if variant == "deleted" {
        repo.git(&["rm", "-q", "--", "spec.lock"]);
        let commit = commit_message(
            repo,
            &format!(
                "spec: lock LAW-BOUND\n\nLock: LAW-BOUND\nLock-mode: accept\nDecided-by: \
                 {DECIDER}\nProposal: {id}\n"
            ),
            false,
        );
        return (commit, "has no spec.lock");
    }
    let made = killed_lock_commit(repo, "LAW-BOUND", id);
    let message = repo.message(&made);
    assert!(message.contains("\nLock-mode: accept\n"), "{message}");
    let rewritten = match variant {
        "regen" => message.replace("Lock-mode: accept\n", "Lock-mode: regen\n"),
        "none" => message.replace("Lock-mode: accept\n", ""),
        other => panic!("no variant {other}"),
    };
    let commit = commit_message(repo, &rewritten, true);
    assert_eq!(repo.changed(&commit), ["spec.lock"], "{variant}");
    (commit, "has no `Lock-mode: accept` or `editorial` trailer")
}

/// A question of an agent on `node`, stored (its hits declared distinct):
/// its ID.
fn question_on(repo: &LockRepo, node: &str, text: &str) -> String {
    let mut request = QuestionRequest {
        node_ids: vec![node.to_owned()],
        text: text.to_owned(),
        working_answer: "Keep the cap as it is.".to_owned(),
        price_of_other: "A new rule case.".to_owned(),
        severity: None,
        distinct_from: Vec::new(),
        author_role: Some("developer".to_owned()),
        author_model: Some("claude-opus-5-5".to_owned()),
        run: None,
        now: RAISED.to_owned(),
        git: repo.git_env(&repo.root),
    };
    let run = |request: &QuestionRequest| {
        propose_question(&repo.env(&repo.root), &Globals::default(), request)
            .unwrap_or_else(|error| panic!("ask: {error}"))
    };
    let mut outcome = run(&request);
    if outcome.refusal.is_none() && !outcome.document.created {
        request.distinct_from = common::decision::hit_names(&outcome);
        outcome = run(&request);
    }
    assert_eq!(outcome.exit(), Exit::Answered, "ask: {outcome:?}");
    assert!(outcome.document.created, "not stored: {outcome:?}");
    outcome.document.id.clone().expect("an ID")
}

/// The stored `drift` column of `id` replaced by `copies` copies of its
/// rows (each copy's `file` under its own `dNN/`) and `more`: at most
/// [`specengine_core::drift::DRIFT_ROWS_MAX`] rows, as a raise stores them.
fn restore_rows(repo: &LockRepo, id: &str, copies: usize, more: u64) {
    let stored = sql(
        repo,
        SLUG,
        &format!("SELECT drift FROM proposals WHERE id = '{id}';"),
    );
    let rows = stored
        .strip_prefix("{\"rows\":[")
        .and_then(|rest| rest.rsplit_once("],\"more\":"))
        .map(|(rows, _)| rows)
        .unwrap_or_else(|| panic!("not the stored shape: {stored}"));
    let copied: Vec<String> = (0..copies)
        .map(|n| rows.replace("\"file\":\"", &format!("\"file\":\"d{n:02}/")))
        .collect();
    let column = format!("{{\"rows\":[{}],\"more\":{more}}}", copied.join(","));
    sql(
        repo,
        SLUG,
        &format!(
            "UPDATE proposals SET drift = '{}' WHERE id = '{id}';",
            column.replace('\'', "''")
        ),
    );
}

// ------------------------------------------------------------------ TE-1

/// TE-1 / RD-1 / RD-3, `marker_behind` on a terminal: the golden
/// `PIECE-COUNTER` item (its `spec_ahead marker_behind` row, which approve
/// cannot accept) followed up with `code` by the binary: one question
/// showing the title, the goal (rev 3, `@3`, the reason, the rows as
/// `spec review` prints them) and `reject PR-0001 (PIECE-COUNTER in
/// spec.lock on main in <root>) and make a draft task (code)? [y/N]`;
/// stdout `rejected PR-0001` alone, the note on stderr; the item
/// `rejected` with the reason; a `draft` task `T-0001` of that title and
/// goal, `[PIECE-COUNTER]`, a human's; `proposal.rejected` with `task`,
/// then `task.created`. `spec task approve T-0001` makes it `ready`, `spec
/// task show T-0001 --json` carries the goal. M: the task `ready` at once;
/// the goal's rows formatted anew.
#[test]
fn te1_marker_behind_item_follows_up_with_code_on_a_terminal() {
    let repo = golden("df-te1-code");
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "PIECE-COUNTER");
    let rows = review_rows(&repo, &id);
    assert!(!rows.is_empty());
    // Its `marker_behind` row, beside the item's other stored findings.
    assert!(
        rows.iter()
            .any(|row| row.contains("| spec_ahead marker_behind |")),
        "{rows:?}"
    );
    let texts = Texts {
        kind: Kind::Code,
        node: "PIECE-COUNTER",
        rev: 3,
        path: COUNTER_MD,
        id: &id,
    };
    let goal = texts.goal(REASON, &rows, 0);
    let run = repo.spec_pty(
        &repo.root,
        &["reject", &id, "--reason", REASON, "--follow-up", "code"],
        Some("y"),
    );
    run.code(0);
    assert!(run.asked, "{}", run.show());
    assert_eq!(run.stdout, format!("rejected {id}\n"));
    let mut prompt = vec![texts.title()];
    prompt.extend(goal.iter().cloned());
    prompt.push(texts.question(&repo.root, false));
    assert_in_order(&run.stderr, &prompt, "the one question");
    assert_eq!(run.stderr.matches("[y/N]").count(), 1, "{}", run.show());
    assert!(
        run.stderr
            .contains(&format!("note: {}\n", note_of("T-0001", &id))),
        "{}",
        run.show()
    );

    let item = proposal(&repo, SLUG, &id);
    assert_eq!(item.status, ProposalStatus::Rejected);
    assert_eq!(item.decision_note.as_deref(), Some(REASON));
    let task = only_task(&repo, SLUG);
    assert_eq!(task.id, "T-0001");
    assert_eq!(task.status, TaskStatus::Draft);
    assert_eq!(task.title.as_deref(), Some(texts.title().as_str()));
    assert_eq!(lines_of(task.goal.as_deref().expect("a goal")), goal);
    assert_eq!(task.targets, ["PIECE-COUNTER"]);
    assert_eq!(task.author, Author::human());
    assert_eq!(task.git_common_dir, item.place.git_common_dir);
    let all = events(&repo, SLUG);
    let tail: Vec<(String, Value)> = all[all.len() - 2..]
        .iter()
        .map(|(kind, payload, _)| (kind.clone(), payload.clone()))
        .collect();
    assert_eq!(
        tail,
        [
            (
                "proposal.rejected".to_owned(),
                json!({"id": id, "reason": REASON, "task": "T-0001"})
            ),
            ("task.created".to_owned(), json!({"id": "T-0001"})),
        ]
    );

    // The owner's approve makes it ready; the package carries the goal.
    repo.spec_pty(&repo.root, &["task", "approve", "T-0001"], Some("y"))
        .code(0);
    assert_eq!(task_of(&repo, SLUG, "T-0001").status, TaskStatus::Ready);
    let shown = repo.spec_piped(&repo.root, &["task", "show", "T-0001", "--json"]);
    let package = shown.code(0).json();
    assert_eq!(package["status"], json!("ready"));
    assert_eq!(package["title"], json!(texts.title()));
    assert_eq!(package["goal"].as_str(), task.goal.as_deref());
}

/// TE-1, another project (ADR-0008): verify-ecs's `PERK-REGEN` system
/// changed (`code_ahead code_changed`), followed up with `spec --json` on a
/// terminal: stdout one JSON document whose `notes` name the task (and the
/// note on stderr); the task's title and goal the spec's (rev 1,
/// `docs/spec/stamina.md`), its rows the review's; approve then `ready`.
/// M: the note left out of `--json`; a goal naming the code-a project's
/// words.
#[test]
fn te1_ecs_code_ahead_item_follows_up_with_spec_on_a_terminal() {
    let repo = ecs_drifted("df-te1-ecs", |repo| {
        repo.replace(
            STAMINA,
            "(stamina.current + rate * dt).min(stamina.max)",
            "(stamina.current + rate * dt * 2.0).min(stamina.max)",
        );
    });
    let raised = raised(&repo, &[]);
    let id = item_of(&raised, "PERK-REGEN");
    let rows = review_rows(&repo, &id);
    assert!(
        rows.iter()
            .any(|row| row.contains("code_ahead code_changed")),
        "{rows:?}"
    );
    let texts = Texts {
        kind: Kind::Spec,
        node: "PERK-REGEN",
        rev: 1,
        path: STAMINA_MD,
        id: &id,
    };
    let goal = texts.goal(REASON, &rows, 0);
    let run = repo.spec_pty(
        &repo.root,
        &[
            "reject",
            &id,
            "--reason",
            REASON,
            "--follow-up",
            "spec",
            "--json",
        ],
        Some("y"),
    );
    run.code(0);
    let document = common::one_json_document(&run.stdout)
        .unwrap_or_else(|problem| panic!("{problem}\n{}", run.show()));
    assert_eq!(document["status"], json!("rejected"));
    let note = note_of("T-0001", &id);
    assert!(
        document["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|text| text == &json!(note)),
        "{document}"
    );
    assert!(
        run.stderr.contains(&format!("note: {note}\n")),
        "{}",
        run.show()
    );
    let mut prompt = vec![texts.title()];
    prompt.extend(goal.iter().cloned());
    prompt.push(texts.question(&repo.root, false));
    assert_in_order(&run.stderr, &prompt, "the one question");

    let task = only_task(&repo, ECS_SLUG);
    assert_eq!(task.status, TaskStatus::Draft);
    assert_eq!(task.title.as_deref(), Some(texts.title().as_str()));
    assert_eq!(lines_of(task.goal.as_deref().expect("a goal")), goal);
    assert_eq!(task.targets, ["PERK-REGEN"]);
    for word in ["code-a", "tally", "LAW-", "PIECE-"] {
        assert!(
            !task.goal.as_deref().unwrap_or_default().contains(word),
            "{word} in another project's goal"
        );
    }
    repo.spec_pty(&repo.root, &["task", "approve", "T-0001"], Some("y"))
        .code(0);
    assert_eq!(task_of(&repo, ECS_SLUG, "T-0001").status, TaskStatus::Ready);
}

// ------------------------------------------------------------------ RD-1

/// RD-1: the follow-up's task is `draft`; the package says so; only the
/// owner's `spec task approve` makes it `ready` (ADR-0012). The item
/// `rejected` by the committer with the reason; the task `T-0001` at
/// `now`, the item's repository, `[LAW-BOUND]`, a human's. M: the task
/// stored `ready`.
#[test]
fn follow_up_task_is_draft() {
    let repo = drifted("df-draft", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let rows = review_rows(&repo, &id);
    let asked = follow_up(&repo, &id, Kind::Code);
    asked.done("a follow-up");
    assert_eq!(asked.questions.len(), 1, "{:?}", asked.questions);
    let item = proposal(&repo, SLUG, &id);
    assert_eq!(item.status, ProposalStatus::Rejected);
    assert_eq!(item.decided_by.as_deref(), Some(DECIDER));
    assert_eq!(item.decision_note.as_deref(), Some(REASON));
    let task = only_task(&repo, SLUG);
    assert_eq!(task.id, "T-0001");
    assert_eq!(task.status, TaskStatus::Draft);
    assert_eq!(task.targets, ["LAW-BOUND"]);
    assert_eq!(task.author, Author::human());
    assert_eq!(task.git_common_dir, item.place.git_common_dir);
    assert_eq!(task.created_at, DECIDED);
    // The implementation's bytes (iteration 1): the goal's parts joined by
    // LF, no final LF, each row indented two spaces; the one question
    // `draft task: <title>`, `goal:`, each goal line indented, then plain
    // reject's question with its tail.
    let texts = Texts {
        kind: Kind::Code,
        node: "LAW-BOUND",
        rev: 2,
        path: COUNTER_MD,
        id: &id,
    };
    let mut goal = format!("{}\nOwner's reason: {REASON}", texts.head());
    for row in &rows {
        goal.push_str(&format!("\n  {row}"));
    }
    assert_eq!(task.goal.as_deref(), Some(goal.as_str()));
    assert_eq!(task.title.as_deref(), Some(texts.title().as_str()));
    let indented: String = goal.lines().map(|line| format!("  {line}\n")).collect();
    assert_eq!(
        asked.question(),
        format!(
            "draft task: {}\ngoal:\n{indented}{}",
            texts.title(),
            texts.question(&repo.root, false)
        )
    );

    let shown = task_show(
        &repo.env(&repo.root),
        &Globals::default(),
        &TaskShowRequest {
            id: Some("T-0001".to_owned()),
            next: false,
            git: repo.git_env(&repo.root),
        },
    )
    .expect("task show");
    let package = shown.package.expect("a package");
    assert_eq!(package.status, TaskStatus::Draft);
    assert_eq!(package.goal, task.goal);

    let approved = task_approve(
        &repo.env(&repo.root),
        &Globals::default(),
        &TaskDecisionRequest {
            id: "T-0001".to_owned(),
            note: None,
            now: LATER.to_owned(),
            git: repo.git_env(&repo.root),
        },
        &mut |_: &str| true,
    )
    .expect("task approve");
    assert_eq!(approved.exit(), Exit::Answered, "{approved:?}");
    assert_eq!(task_of(&repo, SLUG, "T-0001").status, TaskStatus::Ready);
}

/// RD-1 / "Data": the flag on another kind, staged or not, is exit 2
/// `--follow-up: only a drift item has a follow-up`, checked right after
/// the item is read, before its state: an open question, the same one
/// with a reject staged, a rejected one; never asked, nothing written; the
/// binary alike on a terminal. M: the kind check removed (a question's
/// reject making a task); the check after the state (a rejected question:
/// exit 1).
#[test]
fn follow_up_on_a_non_drift_kind_is_a_usage_error() {
    const ONLY: &str = "--follow-up: only a drift item has a follow-up";
    let repo = drifted("df-kind", cap_changed);
    let drift = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let open = question_on(&repo, "LAW-BOUND", "Does the cap count the last step?");
    let closed = question_on(&repo, "LAW-STEP", "Is a step always one?");
    assert_ne!(open, drift);
    reject_at(&repo, &repo.root, &closed).done("a plain reject of a question");
    let check = |id: &str, context: &str| {
        let before = frozen(&repo, SLUG, &repo.root);
        for kind in Kind::BOTH {
            let asked = follow_up(&repo, id, kind);
            assert_eq!(
                asked.cannot(context),
                format!("spec: {ONLY}"),
                "{context} ({kind:?})"
            );
            assert!(asked.questions.is_empty(), "{context}: asked");
            assert_eq!(frozen(&repo, SLUG, &repo.root), before, "{context}");
        }
    };
    check(&open, "an open question");
    check(&closed, "a rejected question");

    let updated_at = proposal(&repo, SLUG, &open).updated_at;
    let staged = stage(
        &repo.env(&repo.root),
        &Globals::default(),
        &StageRequest {
            id: open.clone(),
            body: StageBody::Reject {
                reason: "Not now.".to_owned(),
                updated_at,
            },
            now: DECIDED.to_owned(),
            git: repo.git_env(&repo.root),
        },
    )
    .expect("a stage");
    assert_eq!(staged.exit(), Exit::Answered, "{staged:?}");
    assert!(proposal(&repo, SLUG, &open).staged.is_some());
    check(&open, "a question with a reject staged");

    let before = frozen(&repo, SLUG, &repo.root);
    let run = repo.spec_pty(
        &repo.root,
        &["reject", &open, "--reason", REASON, "--follow-up", "spec"],
        None,
    );
    run.code(2);
    assert!(!run.asked, "{}", run.show());
    assert!(run.stderr.contains(ONLY), "{}", run.show());
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);
    assert!(tasks_of(&repo, SLUG).is_empty());
}

/// "Data": a value other than `code` or `spec` is a usage error (exit 2)
/// before anything is read: never asked, nothing written. M: an unknown
/// value taken as `code`.
#[test]
fn follow_up_takes_only_code_or_spec() {
    let repo = drifted("df-value", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let before = frozen(&repo, SLUG, &repo.root);
    for value in ["docs", "Code", ""] {
        let run = repo.spec_pty(
            &repo.root,
            &["reject", &id, "--reason", REASON, "--follow-up", value],
            None,
        );
        run.code(2);
        assert!(!run.asked, "{value:?}: {}", run.show());
        assert!(
            run.stderr.contains("--follow-up"),
            "{value:?}: {}",
            run.show()
        );
        assert_eq!(frozen(&repo, SLUG, &repo.root), before, "{value:?}");
    }
}

/// RD-1 / D8: a project whose `[ids]` takes `T` (core
/// `task_prefix_clash`) is exit 2 as `spec task new`, read from the
/// recorded root's config: `T` there refuses whether the follow-up runs
/// at that root or from another worktree; `T` only in the current
/// worktree's config does not. Never asked, nothing written; a plain
/// reject still settles the item. M: the clash read from the current
/// directory's config; not checked.
#[test]
fn follow_up_reads_the_t_clash_at_the_recorded_root() {
    let clashes = |asked: &Asked, context: &str| {
        let message = asked.cannot(context);
        assert!(
            message.contains("configures the prefix `T`"),
            "{context}: {message}"
        );
        assert!(asked.questions.is_empty(), "{context}: asked");
    };

    // `T` at the recorded root, the follow-up there.
    let repo = drifted("df-t-root", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    add_t_prefix(&repo.root);
    repo.commit_all("T in [ids]");
    let before = frozen(&repo, SLUG, &repo.root);
    clashes(&follow_up(&repo, &id, Kind::Code), "T at the root");
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);
    reject_at(&repo, &repo.root, &id).done("a plain reject with T in [ids]");

    // Raised in the linked worktree whose config takes `T`; followed up
    // from the main worktree, whose config does not.
    let repo = golden("df-t-linked");
    let linked = worktree(&repo, "t1");
    common::replace(&linked, COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    commit_in(&repo, &linked, "the edit on t1");
    let id = item_of(&raise_at(&repo, &linked, &[], RAISED), "LAW-BOUND");
    add_t_prefix(&linked);
    commit_in(&repo, &linked, "T in [ids] on t1");
    let before = (
        frozen(&repo, SLUG, &repo.root),
        frozen(&repo, SLUG, &linked),
    );
    clashes(
        &follow_up_at(&repo, &repo.root, &id, Kind::Spec, REASON, true),
        "T at the recorded root, run elsewhere",
    );
    assert_eq!(
        (
            frozen(&repo, SLUG, &repo.root),
            frozen(&repo, SLUG, &linked)
        ),
        before
    );

    // Recorded at the main worktree (no `T`); run from a worktree whose
    // config takes `T`: made.
    let repo = drifted("df-t-cwd", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let linked = worktree(&repo, "t1");
    add_t_prefix(&linked);
    commit_in(&repo, &linked, "T in [ids] on t1");
    follow_up_at(&repo, &linked, &id, Kind::Code, REASON, true)
        .done("T only in the current worktree");
    assert_eq!(only_task(&repo, SLUG).targets, ["LAW-BOUND"]);
}

/// RD-1 / step 1: the node resolved now at the recorded root; gone (its
/// section deleted and committed) is exit 1 `--follow-up: LAW-BOUND names
/// no node now; reject without it`, before the question, nothing written;
/// a plain reject then settles it. M: the stored target taken unresolved.
#[test]
fn follow_up_needs_a_node() {
    let repo = drifted("df-gone-node", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    repo.replace(
        COUNTER_MD,
        "## Bound {#LAW-BOUND rev=2}\n\nThe count never exceeds the cap.\n\n",
        "",
    );
    repo.commit_all("LAW-BOUND gone");
    let before = frozen(&repo, SLUG, &repo.root);
    for kind in Kind::BOTH {
        let asked = follow_up(&repo, &id, kind);
        assert_eq!(
            asked.refused("a gone node"),
            "--follow-up: LAW-BOUND names no node now; reject without it",
            "{kind:?}"
        );
        assert!(asked.questions.is_empty(), "{kind:?}: asked");
        assert_eq!(frozen(&repo, SLUG, &repo.root), before, "{kind:?}");
    }
    reject_at(&repo, &repo.root, &id).done("a plain reject of a gone node's item");
    assert!(tasks_of(&repo, SLUG).is_empty());
}

/// RD-1 / D8: a recorded worktree that no longer exists is exit 1
/// `--follow-up: the worktree <w> of PR-0001 no longer exists; reject
/// without it`; an orphan (its repository moved away) is exit 1 naming the
/// repository; before the question, nothing written; the orphan's plain
/// reject then settles it. M: the follow-up resolving the node in the
/// current worktree instead.
#[test]
fn follow_up_needs_the_recorded_worktree() {
    // The linked worktree it was raised in, removed.
    let repo = golden("df-gone-worktree");
    let linked = worktree(&repo, "t1");
    common::replace(&linked, COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
    commit_in(&repo, &linked, "the edit on t1");
    let id = item_of(&raise_at(&repo, &linked, &[], RAISED), "LAW-BOUND");
    let recorded = proposal(&repo, SLUG, &id).place.worktree;
    repo.git(&[
        "worktree",
        "remove",
        "--force",
        linked.to_str().expect("UTF-8"),
    ]);
    assert!(!linked.exists());
    let before = frozen(&repo, SLUG, &repo.root);
    let asked = follow_up(&repo, &id, Kind::Code);
    assert_eq!(
        asked.refused("a gone worktree"),
        format!("--follow-up: the worktree {recorded} of {id} no longer exists; reject without it")
    );
    assert!(asked.questions.is_empty());
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);

    // The whole repository moved: an orphan.
    let repo = drifted("df-orphan", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let place = proposal(&repo, SLUG, &id).place;
    let moved = repo.scratch.join("moved");
    fs::rename(&repo.root, &moved).expect("moved");
    let moved = fs::canonicalize(moved).expect("the moved root");
    let before = frozen(&repo, SLUG, &moved);
    let asked = follow_up_at(&repo, &moved, &id, Kind::Code, REASON, true);
    let reason = asked.refused("an orphan");
    // D8, "repository": the recorded repository's common dir (or its
    // worktree, the spec naming neither).
    assert!(
        [&place.git_common_dir, &place.worktree]
            .iter()
            .any(|path| reason
                == format!(
                    "--follow-up: the repository {path} of {id} no longer exists; reject without it"
                )),
        "{reason}"
    );
    assert!(asked.questions.is_empty());
    assert_eq!(frozen(&repo, SLUG, &moved), before);
    reject_at(&repo, &moved, &id).done("the orphan rejected without a follow-up");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Rejected);
    assert!(tasks_of(&repo, SLUG).is_empty());
}

/// RD-1 (the pin that the follow-up keeps 4c-1's RD-9 lookup): a
/// completing lock commit of the item on its branch refuses the follow-up
/// as a plain reject (exit 1 naming the commit and `spec approve`), before
/// the question, nothing written; one that lands while the owner is asked
/// refuses it after the answer, no task, no event. M: the follow-up's write
/// before reject's lookups; the lookup skipped after the prompt.
#[test]
fn follow_up_refuses_a_drift_item_with_its_lock_commit() {
    let repo = drifted("df-locked", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let commit = killed_lock_commit(&repo, "LAW-BOUND", &id);
    let before = frozen(&repo, SLUG, &repo.root);
    for kind in Kind::BOTH {
        let asked = follow_up(&repo, &id, kind);
        let reason = asked.refused("a completing lock commit");
        assert!(reason.contains(&commit), "{kind:?}: {reason}");
        assert!(
            reason.contains(&format!("`spec approve {id}` completes it")),
            "{kind:?}: {reason}"
        );
        assert!(asked.questions.is_empty(), "{kind:?}: asked");
        assert_eq!(frozen(&repo, SLUG, &repo.root), before, "{kind:?}");
    }

    // The commit lands while the owner is asked.
    let repo = drifted("df-locked-late", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let queue_before = dump(&repo, SLUG);
    let mut questions = Vec::new();
    let mut landed = None;
    let outcome = follow_up_with(
        &repo,
        &repo.root,
        repo.git_env(&repo.root),
        &id,
        Kind::Code,
        REASON,
        &mut |question: &str| {
            questions.push(question.to_owned());
            landed = Some(killed_lock_commit(&repo, "LAW-BOUND", &id));
            true
        },
    );
    let asked = Asked { outcome, questions };
    let reason = asked.refused("a lock commit made during the question");
    let commit = landed.expect("the commit landed");
    assert!(reason.contains(&commit), "{reason}");
    assert_eq!(asked.questions.len(), 1);
    assert_eq!(dump(&repo, SLUG), queue_before, "nothing stored");
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);
}

/// "Data" / plain reject's checks: an item no longer open or approved
/// (rejected) is refused by its state, exit 1, no task; an `approved` one
/// (a run killed before its commit) is followed up, its question `reject
/// approved PR-0001 (…) and make a draft task (code)? [y/N]`. M: the
/// state check skipped by the follow-up.
#[test]
fn follow_up_keeps_reject_states() {
    let repo = drifted("df-closed", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    reject_at(&repo, &repo.root, &id).done("a plain reject");
    let before = frozen(&repo, SLUG, &repo.root);
    let asked = follow_up(&repo, &id, Kind::Code);
    let reason = asked.refused("a rejected item");
    assert!(
        reason.starts_with(&format!(
            "`{id}` is rejected: only an open or approved proposal"
        )),
        "{reason}"
    );
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);

    let repo = drifted("df-approved", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    queue_of(&repo, SLUG)
        .expect("a queue")
        .approve(
            &id,
            &Decision {
                decided_by: DECIDER.to_owned(),
                note: None,
                staged_at: None,
            },
            RAISED,
        )
        .expect("approved");
    let asked = follow_up(&repo, &id, Kind::Code);
    asked.done("an approved item");
    let texts = Texts {
        kind: Kind::Code,
        node: "LAW-BOUND",
        rev: 2,
        path: COUNTER_MD,
        id: &id,
    };
    assert!(
        asked
            .question()
            .ends_with(&texts.question(&repo.root, true)),
        "{}",
        asked.question()
    );
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Rejected);
    assert_eq!(only_task(&repo, SLUG).status, TaskStatus::Draft);
}

/// "Exits": no git identity is exit 2 before the question, nothing
/// written. M: a task made with an empty `decided_by`.
#[test]
fn follow_up_without_identity_exits_2() {
    let repo = drifted("df-ident", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    repo.git(&["config", "user.useConfigOnly", "true"]);
    let git = repo
        .git_env(&repo.root)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let before = frozen(&repo, SLUG, &repo.root);
    let mut questions = Vec::new();
    let outcome = follow_up_with(
        &repo,
        &repo.root,
        git,
        &id,
        Kind::Code,
        REASON,
        &mut |q: &str| {
            questions.push(q.to_owned());
            true
        },
    );
    let asked = Asked { outcome, questions };
    let message = asked.cannot("no identity");
    assert!(message.contains("no git identity"), "{message}");
    assert!(asked.questions.is_empty());
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);
}

// ------------------------------------------------------------------ RD-2

/// RD-2 / Q3: the goal names the node's revision now (step 1), not the
/// one stored in the rows: `LAW-BOUND` raised at rev 2, then raised to rev
/// 3 and committed: the goal says `rev 3` and `@3`; its rows stay as
/// stored. M: the stored `node_rev` used.
#[test]
fn follow_up_goal_names_the_revision_now() {
    let repo = drifted("df-rev-now", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let stored = proposal(&repo, SLUG, &id).drift.expect("drift");
    assert!(
        stored
            .evidence
            .rows
            .iter()
            .all(|row| row.node_rev == Some(2)),
        "raised at rev 2"
    );
    let rows = review_rows(&repo, &id);
    repo.replace(COUNTER_MD, "{#LAW-BOUND rev=2}", "{#LAW-BOUND rev=3}");
    repo.commit_all("LAW-BOUND at rev 3");
    follow_up(&repo, &id, Kind::Code).done("a follow-up after the raise");
    let texts = Texts {
        kind: Kind::Code,
        node: "LAW-BOUND",
        rev: 3,
        path: COUNTER_MD,
        id: &id,
    };
    let task = only_task(&repo, SLUG);
    assert_eq!(
        lines_of(task.goal.as_deref().expect("a goal")),
        texts.goal(REASON, &rows, 0)
    );
}

/// "Rules and edge cases": one item in one state followed up from its
/// root, a subdirectory, a linked worktree whose spec holds `LAW-BOUND` at
/// another revision, and that worktree's subdirectory gives byte-identical
/// questions (title and goal of the recorded root: rev 2), both kinds;
/// declined, nothing written; then made from the linked worktree, the
/// stored title and goal are those shown. M: the node resolved at the
/// current directory's root.
#[test]
fn follow_up_texts_are_byte_identical_from_any_cwd() {
    let repo = drifted("df-cwd", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let rows = review_rows(&repo, &id);
    let linked = worktree(&repo, "t1");
    common::replace(
        &linked,
        COUNTER_MD,
        "{#LAW-BOUND rev=2}",
        "{#LAW-BOUND rev=5}",
    );
    commit_in(&repo, &linked, "LAW-BOUND at rev 5 on t1");
    let cwds = [
        repo.root.clone(),
        repo.root.join("docs").join("spec"),
        linked.clone(),
        linked.join("crates"),
    ];
    let queue_before = dump(&repo, SLUG);
    for kind in Kind::BOTH {
        let texts = Texts {
            kind,
            node: "LAW-BOUND",
            rev: 2,
            path: COUNTER_MD,
            id: &id,
        };
        let mut shown = Vec::new();
        for cwd in &cwds {
            let asked = follow_up_at(&repo, cwd, &id, kind, REASON, false);
            asked.refused("declined");
            shown.push(asked.question().to_owned());
        }
        for (cwd, question) in cwds.iter().zip(&shown) {
            assert_eq!(question, &shown[0], "{kind:?} from {}", cwd.display());
        }
        let mut prompt = vec![texts.title()];
        prompt.extend(texts.goal(REASON, &rows, 0));
        prompt.push(texts.question(&repo.root, false));
        assert_in_order(&shown[0], &prompt, "the question");
        assert_eq!(dump(&repo, SLUG), queue_before, "{kind:?}: declined");
    }
    let asked = follow_up_at(&repo, &linked, &id, Kind::Spec, REASON, true);
    asked.done("made from the linked worktree");
    let task = only_task(&repo, SLUG);
    let title = task.title.expect("a title");
    let goal = task.goal.expect("a goal");
    let mut shown = vec![title.clone()];
    shown.extend(lines_of(&goal));
    assert_in_order(asked.question(), &shown, "the stored texts, as shown");
    let texts = Texts {
        kind: Kind::Spec,
        node: "LAW-BOUND",
        rev: 2,
        path: COUNTER_MD,
        id: &id,
    };
    assert_eq!(title, texts.title());
    assert_eq!(lines_of(&goal), texts.goal(REASON, &rows, 0));
}

/// RD-2: a goal over `GOAL_MAX` cannot be: the stored rows (64, each under
/// a `dNN/` of its own) and `more` 3 make one; the reason (3 600 B of
/// three-byte characters) is cut at 1 024 B with `…` on a character
/// boundary, the whole kept as the item's `decision_note`; the rows shown
/// are the stored ones from the first, as many as fit (the next would
/// not); then `<k> more row(s): spec review PR-0001`, `<k>` the rows not
/// shown plus 3. M: every row appended; the reason uncut; `<k>` without
/// the item's `more`.
#[test]
fn follow_up_goal_is_capped() {
    let repo = drifted("df-goal-cap", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let stored = proposal(&repo, SLUG, &id).drift.expect("drift");
    let per_copy = stored.evidence.rows.len();
    let copies = specengine_core::drift::DRIFT_ROWS_MAX / per_copy;
    restore_rows(&repo, &id, copies, 3);
    let rows = review_rows(&repo, &id);
    assert_eq!(rows.len(), copies * per_copy);
    let reason = "\u{2014}".repeat(1200);
    assert_eq!(reason.len(), 3600);
    follow_up_at(&repo, &repo.root, &id, Kind::Spec, &reason, true).done("a long goal");
    let goal = only_task(&repo, SLUG).goal.expect("a goal");
    assert!(goal.len() <= GOAL_MAX, "{} bytes", goal.len());
    let lines = lines_of(&goal);
    let texts = Texts {
        kind: Kind::Spec,
        node: "LAW-BOUND",
        rev: 2,
        path: COUNTER_MD,
        id: &id,
    };
    assert_eq!(lines[0], texts.head());
    let shown = lines[1]
        .strip_prefix("Owner's reason: ")
        .unwrap_or_else(|| panic!("no reason line: {goal}"));
    let kept = shown
        .strip_suffix('\u{2026}')
        .unwrap_or_else(|| panic!("not cut with \u{2026}: {shown}"));
    assert!(reason.starts_with(kept));
    assert!(
        kept.len() <= 1024 && kept.len() + 6 >= 1024,
        "{} bytes kept",
        kept.len()
    );
    let shown_rows = &lines[2..lines.len() - 1];
    assert!(!shown_rows.is_empty(), "no row shown: {goal}");
    assert_eq!(shown_rows, &rows[..shown_rows.len()]);
    let left = rows.len() - shown_rows.len();
    assert!(left > 0, "every row fits: {goal}");
    assert_eq!(lines[lines.len() - 1], more_line(left + 3, &id));
    let next = &rows[shown_rows.len()];
    assert!(
        goal.len() + next.len() + 5 > GOAL_MAX,
        "a row that fits left out: {} + {}",
        goal.len(),
        next.len()
    );
    assert_eq!(
        proposal(&repo, SLUG, &id).decision_note.as_deref(),
        Some(reason.as_str())
    );
}

/// RD-2 / "Title and goal": an item with stored rows left out (`more`
/// 5) shows every stored row, then `5 more row(s): spec review PR-0001`.
/// M: the item's `more` dropped.
#[test]
fn follow_up_goal_counts_the_rows_left_out() {
    let repo = drifted("df-more", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    restore_rows(&repo, &id, 1, 5);
    let rows = review_rows(&repo, &id);
    follow_up(&repo, &id, Kind::Code).done("an item with more rows");
    let texts = Texts {
        kind: Kind::Code,
        node: "LAW-BOUND",
        rev: 2,
        path: COUNTER_MD,
        id: &id,
    };
    assert_eq!(
        lines_of(&only_task(&repo, SLUG).goal.expect("a goal")),
        texts.goal(REASON, &rows, 5)
    );
}

/// RD-2: a title over `TITLE_MAX` (a node of a 244-byte ID, locked, its
/// bound const changed) is cut with `…` to at most 256 bytes, from the
/// start of the whole title; the goal keeps the whole ID. M: the title
/// stored uncut (or refused).
#[test]
fn follow_up_title_is_capped() {
    let repo = golden("df-title-cap");
    let long = format!("LAW-{}", "LONG".repeat(60));
    assert_eq!(long.len(), 244);
    repo.append(
        COUNTER_MD,
        &format!("\n## Long {{#{long} rev=1}}\n\nA rule with a long name.\n"),
    );
    repo.append(
        COUNTER,
        &format!("\n// @implements {long}@1\npub const LONG: u32 = 1;\n"),
    );
    repo.commit_all("a long name");
    repo.accept_ok(&long);
    repo.replace(
        COUNTER,
        "pub const LONG: u32 = 1;",
        "pub const LONG: u32 = 2;",
    );
    repo.commit_all("the edit");
    let id = item_of(&raised(&repo, &[long.as_str()]), &long);
    follow_up(&repo, &id, Kind::Code).done("a long title");
    let texts = Texts {
        kind: Kind::Code,
        node: &long,
        rev: 1,
        path: COUNTER_MD,
        id: &id,
    };
    let full = texts.title();
    assert!(full.len() > TITLE_MAX);
    let task = only_task(&repo, SLUG);
    let title = task.title.expect("a title");
    assert!(title.len() <= TITLE_MAX, "{} bytes", title.len());
    let kept = title
        .strip_suffix('\u{2026}')
        .unwrap_or_else(|| panic!("not cut with \u{2026}: {title}"));
    assert!(full.starts_with(kept), "{title}");
    assert!(kept.len() + 6 >= TITLE_MAX, "{} bytes kept", kept.len());
    assert_eq!(
        lines_of(&task.goal.expect("a goal"))[0],
        texts.head(),
        "the goal keeps the whole ID"
    );
}

/// Step 2 ("controls escaped as the question's"): a reason holding an
/// escape sequence and a bidirectional override reaches the question
/// escaped (`\u{1b}`, `\u{202e}`), never raw. Iteration 1's reading
/// (deviation 6): the stored goal keeps them as written (escaped when a
/// terminal shows it). M: the title and goal joined to the question
/// unescaped.
#[test]
fn follow_up_question_escapes_controls() {
    let repo = drifted("df-escape", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let before = dump(&repo, SLUG);
    let reason = "Not now.\u{1b}[2J\u{202e}gnorw";
    let asked = follow_up_at(&repo, &repo.root, &id, Kind::Code, reason, false);
    asked.refused("declined");
    let question = asked.question();
    assert!(
        !question.contains('\u{1b}') && !question.contains('\u{202e}'),
        "raw controls: {question:?}"
    );
    assert!(
        question.contains("Not now.\\u{1b}[2J\\u{202e}gnorw"),
        "{question:?}"
    );
    assert_eq!(dump(&repo, SLUG), before);
    follow_up_at(&repo, &repo.root, &id, Kind::Code, reason, true).done("made");
    let goal = only_task(&repo, SLUG).goal.expect("a goal");
    assert!(
        goal.contains(&format!("Owner's reason: {reason}")),
        "{goal:?}"
    );
}

// ------------------------------------------------------------------ RD-3

/// RD-3: a follow-up logs `proposal.rejected` `{"id","reason","task"}` (its
/// payload as stored), then `task.created` `{"id"}`, both at `now`, and
/// nothing else. M: `task` missing; `task.created` first.
#[test]
fn follow_up_events_name_the_task() {
    let repo = drifted("df-events", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let before = events(&repo, SLUG).len();
    follow_up(&repo, &id, Kind::Spec).done("a follow-up");
    let added: Vec<(String, Value, String)> = events(&repo, SLUG).split_off(before);
    assert_eq!(
        added,
        [
            (
                "proposal.rejected".to_owned(),
                json!({"id": id, "reason": REASON, "task": "T-0001"}),
                DECIDED.to_owned()
            ),
            (
                "task.created".to_owned(),
                json!({"id": "T-0001"}),
                DECIDED.to_owned()
            ),
        ]
    );
    assert_eq!(
        rejected_payloads(&repo, SLUG),
        [format!(
            r#"{{"id":"{id}","reason":"{REASON}","task":"T-0001"}}"#
        )]
    );
}

/// RD-3 / Q1: a reject without a follow-up logs the payload 4c-1 logs,
/// byte for byte (`{"id","reason"}`, no `task` key, no `"task":null`), one
/// event, no task; stdout `rejected PR-0001`, no task named. M:
/// `"task":null` on every reject.
#[test]
fn plain_reject_event_has_no_task() {
    let repo = drifted("df-plain", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let before = events(&repo, SLUG).len();
    let asked = reject_at(&repo, &repo.root, &id);
    let outcome = asked.done("a plain reject");
    assert_eq!(
        rejected_payloads(&repo, SLUG),
        [format!(r#"{{"id":"{id}","reason":"Not drift."}}"#)]
    );
    assert_eq!(events(&repo, SLUG).len(), before + 1);
    assert!(tasks_of(&repo, SLUG).is_empty());
    assert_eq!(printed(outcome), format!("rejected {id}\n"));
    for line in asked.stderr().iter().chain(json_notes(outcome).iter()) {
        assert!(!line.contains("created T-"), "{line}");
    }
}

/// RD-3 / Q2: the task's ID is a note on stderr and in the `--json`
/// `notes`, stdout `rejected PR-0001` alone; with a task made before
/// (`T-0001`), the follow-up's is `T-0002`, named. M: no task line in
/// `notes`; the ID hard-wired.
#[test]
fn follow_up_note_names_the_task() {
    let repo = drifted("df-note", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let place = proposal(&repo, SLUG, &id).place;
    let earlier = queue_of(&repo, SLUG)
        .expect("a queue")
        .create_task(
            &NewTask {
                git_common_dir: place.git_common_dir,
                title: Some("Earlier work".to_owned()),
                goal: None,
                targets: vec!["LAW-STEP".to_owned()],
                author: Author::human(),
            },
            RAISED,
        )
        .expect("an earlier task");
    assert_eq!(earlier.id, "T-0001");
    let asked = follow_up(&repo, &id, Kind::Code);
    let outcome = asked.done("a follow-up");
    assert_eq!(printed(outcome), format!("rejected {id}\n"));
    assert_noted(&asked, &note_of("T-0002", &id), "the task made");
    assert_eq!(task_of(&repo, SLUG, "T-0002").status, TaskStatus::Draft);
    assert_eq!(tasks_of(&repo, SLUG).len(), 2);
}

// ------------------------------------------------------------------ RD-5

/// RD-5 / Q11: a drift item whose only `Proposal:` commit does not
/// complete it (its lock commit's `Lock-mode: regen`; no `Lock-mode`; every
/// trailer but `spec.lock` deleted) is rejected all the same, plain or
/// followed up, the note `the commit <sha> on `main` carries `Proposal:
/// PR-0001` but does not complete it (<why>); rejected all the same` on
/// stderr and in `notes`; the follow-up's task made. 4c-1's
/// `reject_refuses_a_drift_item_with_its_lock_commit` (a completing one)
/// stays as it is. M: the reject's lookup back to the trailer alone.
#[test]
fn reject_is_not_blocked_by_a_non_completing_lock_commit() {
    for variant in ["regen", "none", "deleted"] {
        // A plain reject.
        let repo = drifted(&format!("df-q11-plain-{variant}"), cap_changed);
        let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
        let (commit, why) = non_completing(&repo, &id, variant);
        let asked = reject_at(&repo, &repo.root, &id);
        asked.done(variant);
        assert_noted(&asked, &carried_note(&commit, &id, why), variant);
        assert_eq!(
            proposal(&repo, SLUG, &id).status,
            ProposalStatus::Rejected,
            "{variant}"
        );
        assert!(tasks_of(&repo, SLUG).is_empty(), "{variant}");

        // A follow-up.
        let repo = drifted(&format!("df-q11-follow-{variant}"), cap_changed);
        let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
        let (commit, why) = non_completing(&repo, &id, variant);
        let head = repo.head();
        let asked = follow_up(&repo, &id, Kind::Code);
        asked.done(variant);
        assert_noted(&asked, &carried_note(&commit, &id, why), variant);
        assert_noted(&asked, &note_of("T-0001", &id), variant);
        assert_eq!(
            proposal(&repo, SLUG, &id).status,
            ProposalStatus::Rejected,
            "{variant}"
        );
        assert_eq!(only_task(&repo, SLUG).status, TaskStatus::Draft);
        assert_eq!(repo.head(), head, "{variant}: nothing committed");
    }
}

// ------------------------------- readings beyond the spec (iteration 1)

/// Iteration 1's deviation 1: a node whose revision cannot be read now
/// (`rev=0`, written but no revision) is exit 1 `--follow-up: the revision
/// of LAW-BOUND in docs/spec/counter.md cannot be read now; reject without
/// it`, before the question, nothing written; a plain reject settles it.
/// M: an unreadable revision named as 0 (or 1) in the goal.
#[test]
fn follow_up_needs_a_readable_revision() {
    let repo = drifted("df-rev-unread", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    repo.replace(COUNTER_MD, "{#LAW-BOUND rev=2}", "{#LAW-BOUND rev=0}");
    repo.commit_all("LAW-BOUND's revision unreadable");
    let before = frozen(&repo, SLUG, &repo.root);
    for kind in Kind::BOTH {
        let asked = follow_up(&repo, &id, kind);
        assert_eq!(
            asked.refused("an unreadable revision"),
            format!(
                "--follow-up: the revision of LAW-BOUND in {COUNTER_MD} cannot be read now; \
                 reject without it"
            ),
            "{kind:?}"
        );
        assert!(asked.questions.is_empty(), "{kind:?}: asked");
        assert_eq!(frozen(&repo, SLUG, &repo.root), before, "{kind:?}");
    }
    reject_at(&repo, &repo.root, &id).done("a plain reject");
    assert!(tasks_of(&repo, SLUG).is_empty());
}

/// Iteration 1's deviation 2: a node that is there but is not one node
/// (`LAW-BOUND` declared in a second document) is exit 1 with the
/// resolver's reason, `--follow-up: <why>; reject without it`, not the
/// gone node's text; before the question, nothing written. M: any
/// unresolved node reported as gone.
#[test]
fn follow_up_needs_one_node() {
    let repo = drifted("df-two-nodes", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    repo.append(
        "docs/spec/limits.md",
        "\n## Bound twice {#LAW-BOUND}\n\nThe same ID a second time.\n",
    );
    repo.commit_all("LAW-BOUND declared twice");
    let before = frozen(&repo, SLUG, &repo.root);
    let asked = follow_up(&repo, &id, Kind::Code);
    let reason = asked.refused("two holders");
    assert!(
        reason.starts_with("--follow-up: ") && reason.ends_with("; reject without it"),
        "{reason}"
    );
    assert!(reason.contains("LAW-BOUND"), "{reason}");
    assert_ne!(
        reason,
        "--follow-up: LAW-BOUND names no node now; reject without it"
    );
    assert!(asked.questions.is_empty());
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);
}

/// Iteration 1's deviation 3: the recorded root's config, read for the
/// node and the `T` clash, naming another project (its slug changed) or
/// not reading as a config is exit 2 `spec: --follow-up: <why>`, before
/// the question, nothing written. M: the current directory's config read
/// instead (the follow-up made).
#[test]
fn follow_up_needs_the_recorded_config() {
    for (label, edit) in [
        (
            "df-config-slug",
            (|dir: &Path| {
                common::replace(
                    dir,
                    "specengine.toml",
                    "slug = \"code-a\"",
                    "slug = \"code-b\"",
                )
            }) as fn(&Path),
        ),
        ("df-config-broken", |dir: &Path| {
            common::write(dir, "specengine.toml", "[project\nslug = \"code-a\"\n")
        }),
    ] {
        let repo = golden(label);
        let linked = worktree(&repo, "t1");
        common::replace(&linked, COUNTER, "CAP: u32 = 10;", "CAP: u32 = 12;");
        commit_in(&repo, &linked, "the edit on t1");
        let id = item_of(&raise_at(&repo, &linked, &[], RAISED), "LAW-BOUND");
        edit(&linked);
        commit_in(&repo, &linked, "the config changed on t1");
        let before = (
            frozen(&repo, SLUG, &repo.root),
            frozen(&repo, SLUG, &linked),
        );
        let asked = follow_up(&repo, &id, Kind::Code);
        let message = asked.cannot(label);
        assert!(
            message.starts_with("spec: --follow-up: "),
            "{label}: {message}"
        );
        assert!(asked.questions.is_empty(), "{label}: asked");
        assert_eq!(
            (
                frozen(&repo, SLUG, &repo.root),
                frozen(&repo, SLUG, &linked)
            ),
            before,
            "{label}"
        );
    }
}

/// Iteration 1's deviation 4 (Q11, "git cannot tell"): a `Proposal:`
/// commit git cannot read (its parent's tree object gone, so its changed
/// paths cannot be listed) refuses the reject, plain or followed up, exit
/// 1 ``cannot tell whether `PR-0001` has its commit in history: the
/// commit <sha> on `main` cannot be read: …``, nothing written. M: an
/// unreadable commit taken as one that does not complete the item.
#[test]
fn reject_is_refused_by_an_unreadable_proposal_commit() {
    let repo = drifted("df-q11-unread", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let parent = repo.head();
    let (commit, _) = non_completing(&repo, &id, "regen");
    assert_eq!(repo.parents(&commit), std::slice::from_ref(&parent));
    let tree = repo.git_text(&["rev-parse", &format!("{parent}^{{tree}}")]);
    let object = repo
        .root
        .join(".git/objects")
        .join(&tree[..2])
        .join(&tree[2..]);
    fs::remove_file(&object)
        .unwrap_or_else(|e| panic!("the parent's tree {} loose: {e}", object.display()));
    let before = frozen(&repo, SLUG, &repo.root);
    let want = format!(
        "cannot tell whether `{id}` has its commit in history: the commit {commit} on `main` \
         cannot be read: "
    );
    let plain = reject_at(&repo, &repo.root, &id);
    let reason = plain.refused("a plain reject");
    assert!(reason.starts_with(&want), "{reason}");
    for kind in Kind::BOTH {
        let asked = follow_up(&repo, &id, kind);
        let reason = asked.refused("a follow-up");
        assert!(reason.starts_with(&want), "{kind:?}: {reason}");
        assert!(asked.questions.is_empty(), "{kind:?}: asked");
    }
    assert_eq!(frozen(&repo, SLUG, &repo.root), before);
    assert!(tasks_of(&repo, SLUG).is_empty());
}

/// Review 1's W17 (iteration 2): two `Proposal:` commits that do not
/// complete the item (its lock commit's `Lock-mode: regen`, then a commit
/// changing another file) give ONE note naming the newest, its why and
/// ` (1 older one(s) too)`, shown in the `[y/N]` question (as `noted(..)`
/// shows the `unknown` case) and once on stderr and in `notes`, a declined
/// answer included. The one-commit note's text is
/// `reject_is_not_blocked_by_a_non_completing_lock_commit`'s. M: a note
/// per commit; the note left out of the question; the count off by one.
#[test]
fn q11_one_note_names_the_newest_commit_in_the_question() {
    let repo = drifted("df-q11-newest", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let (older, _) = non_completing(&repo, &id, "regen");
    repo.write("notes.txt", "a note\n");
    repo.git(&["add", "--", "notes.txt"]);
    let newer = commit_message(&repo, &format!("a note\n\nProposal: {id}\n"), false);
    assert_eq!(repo.parents(&newer), std::slice::from_ref(&older));
    let carried = |notes: Vec<String>| -> Vec<String> {
        notes
            .into_iter()
            .filter(|note| note.ends_with("rejected all the same"))
            .collect()
    };
    let the_note = |notes: &[String], context: &str| -> String {
        assert_eq!(notes.len(), 1, "{context}: one note: {notes:?}");
        let note = notes[0].clone();
        assert!(
            note.starts_with(&format!(
                "the commit {newer} on `main` carries `Proposal: {id}`"
            )),
            "{context}: the newest named: {note}"
        );
        assert!(
            note.contains("(changes notes.txt)") && note.contains(" (1 older one(s) too)"),
            "{context}: {note}"
        );
        assert!(!note.contains(&older), "{context}: the older named: {note}");
        note
    };
    let declined = follow_up_at(&repo, &repo.root, &id, Kind::Code, REASON, false);
    declined.refused("declined");
    let outcome = declined.outcome.as_ref().expect("an answer");
    let note = the_note(&carried(json_notes(outcome)), "declined");
    assert!(
        declined.question().contains(&note),
        "the note in the question: {}",
        declined.question()
    );
    let asked = reject_at(&repo, &repo.root, &id);
    asked.done("a plain reject");
    assert!(
        asked.question().contains(&note),
        "the note in a plain reject's question: {}",
        asked.question()
    );
    let made = the_note(
        &carried(json_notes(asked.outcome.as_ref().expect("an answer"))),
        "made",
    );
    assert_eq!(made, note);
    assert_eq!(
        asked
            .stderr()
            .iter()
            .filter(|line| **line == format!("note: {note}"))
            .count(),
        1,
        "{:?}",
        asked.stderr()
    );
}

/// Iteration 1's deviation 6: the owner's reason stays one line of the
/// goal, its line breaks (LF, CRLF, CR) written as spaces; the whole
/// reason, breaks included, stays the item's `decision_note`. M: the
/// reason's lines spilled into the goal's.
#[test]
fn follow_up_goal_keeps_the_reason_on_one_line() {
    let repo = drifted("df-reason-lines", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let reason = "First line.\nSecond line.\r\nThird line.\rFourth.";
    follow_up_at(&repo, &repo.root, &id, Kind::Spec, reason, true).done("made");
    let goal = only_task(&repo, SLUG).goal.expect("a goal");
    let lines: Vec<&str> = goal
        .lines()
        .filter(|line| line.starts_with("Owner's reason:"))
        .collect();
    assert_eq!(
        lines,
        ["Owner's reason: First line. Second line. Third line. Fourth."]
    );
    assert!(!goal.contains('\r'), "{goal:?}");
    assert_eq!(
        proposal(&repo, SLUG, &id).decision_note.as_deref(),
        Some(reason)
    );
}

/// Makes a file unreadable (mode 000) and gives its mode back when dropped,
/// a failed assertion included.
struct Unreadable(PathBuf);

impl Unreadable {
    fn new(path: PathBuf) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("mode 000");
        Self(path)
    }
}

impl Drop for Unreadable {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o644));
    }
}

/// Review 1's W18 (iteration 2): a node whose holder's nodes cannot be
/// known now (`docs/spec/counter.md` unreadable, mode 000; or not UTF-8)
/// is not "gone": exit 1 with the resolver's reason, `--follow-up: <why>;
/// reject without it`, before the question, nothing written, the item
/// still `open`. Control: every file read and the node deleted, "names no
/// node now". Known limits, not pinned (iteration 2 W18a-c): the reason
/// does not name the file; a node in a directory that cannot be listed,
/// or a holder whose front-matter does not read, counts as known. M:
/// "gone" without the every-file-known condition.
#[test]
fn follow_up_a_holder_that_cannot_be_read_is_not_gone() {
    const GONE: &str = "--follow-up: LAW-BOUND names no node now; reject without it";
    let resolver_reason = |asked: &Asked, context: &str| {
        let reason = asked.refused(context);
        assert!(
            reason.starts_with("--follow-up: ") && reason.ends_with("; reject without it"),
            "{context}: {reason}"
        );
        assert!(reason.contains("LAW-BOUND"), "{context}: {reason}");
        assert_ne!(reason, GONE, "{context}");
        assert!(asked.questions.is_empty(), "{context}: asked");
    };

    // Not UTF-8, committed.
    let repo = drifted("df-holder-bytes", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let mut bytes = fs::read(repo.root.join(COUNTER_MD)).expect("the holder");
    bytes.extend_from_slice(b"\xff\xfe not UTF-8\n");
    repo.write(COUNTER_MD, &bytes);
    repo.commit_all("the holder not UTF-8");
    let before = frozen(&repo, SLUG, &repo.root);
    for kind in Kind::BOTH {
        resolver_reason(&follow_up(&repo, &id, kind), "not UTF-8");
        assert_eq!(frozen(&repo, SLUG, &repo.root), before, "{kind:?}");
    }
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);

    // Unreadable (mode 000) in the working tree.
    let repo = drifted("df-holder-mode", cap_changed);
    let id = item_of(&raised(&repo, &[]), "LAW-BOUND");
    let (queue, head) = (dump(&repo, SLUG), repo.head());
    {
        let _unreadable = Unreadable::new(repo.root.join(COUNTER_MD));
        resolver_reason(&follow_up(&repo, &id, Kind::Code), "unreadable");
        assert_eq!(dump(&repo, SLUG), queue, "nothing stored");
        assert_eq!(repo.head(), head);
    }
    assert_eq!(proposal(&repo, SLUG, &id).status, ProposalStatus::Open);

    // Control: every file read, the node deleted.
    repo.replace(
        COUNTER_MD,
        "## Bound {#LAW-BOUND rev=2}\n\nThe count never exceeds the cap.\n\n",
        "",
    );
    repo.commit_all("LAW-BOUND gone");
    let asked = follow_up(&repo, &id, Kind::Code);
    assert_eq!(asked.refused("gone"), GONE);
    assert!(tasks_of(&repo, SLUG).is_empty());
}
