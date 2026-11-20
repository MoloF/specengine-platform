//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! what practices never change and what one state always gives, over the
//! scene (`tests/practices_scene`, a scratch git copy of
//! `fixtures/studio-lamp` with the practices; the fixture is only read).
//!
//! - DP15: twin scenes, identical but for every practice file deleted from
//!   the second between the task's plan and its approve: the owner's
//!   approve, the claim, every move of `C-0001` to `accept.from`, the report,
//!   the completion and the reviewer's accept answer alike (exit, line,
//!   document, scratch paths as `<scratch>`), and end `done` and accepted.
//!   M: a move, accept, claim or completion that reads a practice.
//! - DP20: the scene at two roots under two `HOME`s, its files created in
//!   opposite orders (the second indexed half-way, so its index rows are
//!   not in path order): `card show` (`--practices`, `--context`,
//!   `--history`), `task show`, `bundle --discipline` and `review` give
//!   byte-equal text and JSON, and a repeat gives the same; the review
//!   document's `worktree` names its root, so the root's path is written
//!   `<root>` there alone (`cited` compared as served). M: practices in
//!   index row order.
//!
//! The guards of DP20 (no discipline, kind or trade literal in the
//! sources) are `specengine-eval`'s `workspace_names.rs` and
//! `specengine-http`'s `no_domain.rs`; `doc_pointers.rs` runs as it is.

#![cfg(unix)]

mod common;
mod practices_scene;

use std::ffi::OsStr;
use std::path::Path;

#[allow(unused_imports)]
use common::git;
use common::{Run, spec_with};
use practices_scene::{CLOCK, PRACTICE_FILES, REVIEWER, Scene};
use serde_json::{Value, json};
use specengine_cli::{
    CardAcceptRequest, CardMoveRequest, CliError, Globals, Outcome, ProposedText, TaskClaimRequest,
    TaskCompleteRequest, TaskDecisionRequest, TaskOutcome, TaskPlanRequest, TaskReportRequest,
    card_accept, card_move, render_json, render_text, task_approve, task_claim, task_complete,
    task_plan, task_report,
};

fn spec_at(scene: &Scene, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    let vars = scene.vars_with(home);
    let env: Vec<(&str, &OsStr)> = vars
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_os_str()))
        .collect();
    spec_with(cwd, args, &env)
}

fn spec(scene: &Scene, args: &[&str]) -> Run {
    spec_at(scene, &scene.root, &scene.home, args)
}

// ----------------------------------------------------------------- DP15

/// One step's answer: `(step, exit, text, JSON)`, the twin's scratch path
/// written `<scratch>`.
type Answer = (String, i32, String, String);

fn placed(scene: &Scene, text: &str) -> String {
    text.replace(
        scene.scratch().to_str().expect("a UTF-8 scratch"),
        "<scratch>",
    )
}

fn task_answer(scene: &Scene, step: &str, outcome: Result<TaskOutcome, CliError>) -> Answer {
    let outcome = outcome.unwrap_or_else(|error| panic!("{step}: {error}"));
    let code = i32::from(outcome.exit().code());
    let outcome = Outcome::Task(Box::new(outcome));
    (
        step.to_owned(),
        code,
        placed(scene, &render_text(&outcome)),
        placed(scene, &render_json(&outcome)),
    )
}

fn card_answer(
    scene: &Scene,
    step: &str,
    outcome: Result<specengine_cli::CardOutcome, CliError>,
) -> Answer {
    let outcome = outcome.unwrap_or_else(|error| panic!("{step}: {error}"));
    let code = i32::from(outcome.exit().code());
    let outcome = Outcome::Card(Box::new(outcome));
    (
        step.to_owned(),
        code,
        placed(scene, &render_text(&outcome)),
        placed(scene, &render_json(&outcome)),
    )
}

/// The steps of DP15 on `scene`; `delete` removes every practice file
/// after the plan, before the approve.
fn steps(scene: &Scene, delete: bool) -> Vec<Answer> {
    let env = scene.env();
    let globals = Globals::default();
    let git = Scene::git_env;
    let mut out = Vec::new();
    out.push(task_answer(
        scene,
        "plan",
        task_plan(
            &env,
            &globals,
            &TaskPlanRequest {
                id: "T-0001".to_owned(),
                plan: ProposedText::Given(b"1. Model.\n2. Land.\n".to_vec()),
                criteria: vec!["The boat lands.".to_owned()],
                affected: Vec::new(),
                now: CLOCK.to_owned(),
                git: git(scene),
            },
        ),
    ));
    if delete {
        for file in PRACTICE_FILES {
            scene.remove(file);
        }
        assert!(
            PRACTICE_FILES.iter().all(|file| !scene.path(file).exists()),
            "every practice deleted"
        );
    }
    let mut questions = Vec::new();
    let mut consent = |question: &str| {
        questions.push(question.to_owned());
        true
    };
    out.push(task_answer(
        scene,
        "approve",
        task_approve(
            &env,
            &globals,
            &TaskDecisionRequest {
                id: "T-0001".to_owned(),
                note: None,
                now: CLOCK.to_owned(),
                git: git(scene),
            },
            &mut consent,
        ),
    ));
    out.push((
        "approve asked".to_owned(),
        0,
        placed(scene, &questions.join(" | ")),
        String::new(),
    ));
    out.push(task_answer(
        scene,
        "claim",
        task_claim(
            &env,
            &globals,
            &TaskClaimRequest {
                id: "T-0001".to_owned(),
                role: "modeller".to_owned(),
                worktree: scene.root.clone(),
                now: CLOCK.to_owned(),
                git: git(scene),
            },
        ),
    ));
    for column in ["ready", "in-progress", "pending-review", "approved"] {
        out.push(card_answer(
            scene,
            &format!("move {column}"),
            card_move(
                &env,
                &globals,
                &CardMoveRequest {
                    id: "C-0001".to_owned(),
                    to: Some(column.to_owned()),
                    before: None,
                    note: None,
                    revision: None,
                    actor: None,
                    now: CLOCK.to_owned(),
                },
            ),
        ));
    }
    out.push(task_answer(
        scene,
        "report",
        task_report(
            &env,
            &globals,
            &TaskReportRequest {
                id: "T-0001".to_owned(),
                outcome: "completed".to_owned(),
                summary: "The boat lands.".to_owned(),
                changed_files: Vec::new(),
                now: CLOCK.to_owned(),
                git: git(scene),
            },
        ),
    ));
    out.push(task_answer(
        scene,
        "complete",
        task_complete(
            &env,
            &globals,
            &TaskCompleteRequest {
                id: "T-0001".to_owned(),
                now: CLOCK.to_owned(),
                git: git(scene),
            },
        ),
    ));
    let mut asked = Vec::new();
    let mut yes = |question: &str| {
        asked.push(question.to_owned());
        true
    };
    out.push(card_answer(
        scene,
        "accept",
        card_accept(
            &env,
            &globals,
            &CardAcceptRequest {
                id: "C-0001".to_owned(),
                actor: REVIEWER.to_owned(),
                note: None,
                revision: None,
                now: CLOCK.to_owned(),
                briefed: true,
                departs: Vec::new(),
                summary: None,
                observed: None,
                documented: None,
                gap_type: None,
                login: None,
            },
            &mut yes,
        ),
    ));
    out.push((
        "accept asked".to_owned(),
        0,
        placed(scene, &asked.join(" | ")),
        String::new(),
    ));
    out
}

/// DP15: the twins answer every step alike; each step answers; the card
/// ends accepted in `in-engine` and the task `done`. M: a step that reads
/// a practice (the twin without them answers otherwise).
#[test]
fn dp15_nothing_is_gated_by_a_practice() {
    let kept = Scene::new("dp15-kept");
    let gone = Scene::new("dp15-gone");
    let a = steps(&kept, false);
    let b = steps(&gone, true);
    for (answer, other) in a.iter().zip(&b) {
        assert_eq!(answer, other, "the step answers alike with no practice");
    }
    assert_eq!(a.len(), b.len());
    for (step, code, _, _) in &a {
        assert_eq!(*code, 0, "{step} answered");
    }
    for scene in [&kept, &gone] {
        let card = spec(scene, &["--json", "card", "show", "C-0001"]);
        card.code(0);
        let card = card.json();
        assert_eq!(card["column"], "in-engine", "{card}");
        assert!(card["acceptance"].is_object(), "{card}");
        let task = spec(scene, &["--json", "task", "show", "T-0001"]);
        task.code(0);
        assert_eq!(task.json()["status"], "done");
    }
}

// ----------------------------------------------------------------- DP20

/// The reads of DP20, each `(args, review)`: a review's JSON names its
/// worktree.
const READS: [(&[&str], bool); 16] = [
    (&["card", "show", "C-0001", "--practices"], false),
    (&["--json", "card", "show", "C-0001", "--practices"], false),
    (&["card", "show", "C-0001", "--context"], false),
    (&["--json", "card", "show", "C-0001", "--context"], false),
    (
        &[
            "--json",
            "card",
            "show",
            "C-0002",
            "--practices",
            "--history",
        ],
        false,
    ),
    (&["--json", "card", "show", "C-0003", "--practices"], false),
    (&["task", "show", "T-0001"], false),
    (&["--json", "task", "show", "T-0001"], false),
    (&["--json", "task", "show", "T-0002"], false),
    (&["bundle", "SCN-ARRIVAL", "--discipline", "art"], false),
    (
        &["--json", "bundle", "SCN-ARRIVAL", "--discipline", "art"],
        false,
    ),
    (
        &[
            "--json",
            "bundle",
            "SCN-ARRIVAL",
            "--discipline",
            "art",
            "--discipline",
            "development",
            "--budget",
            "3000",
        ],
        false,
    ),
    (
        &["--json", "bundle", "SCN-STORM", "--discipline", "sound"],
        false,
    ),
    (&["review", "PR-0001"], true),
    (&["--json", "review", "PR-0001"], true),
    (&["--json", "library"], false),
];

/// Every read of [`READS`] on `scene`: `(exit, stdout, stderr)`, a review's
/// root written `<root>`.
fn play(scene: &Scene) -> Vec<(i32, String, String)> {
    let root = scene.root.to_str().expect("a UTF-8 root").to_owned();
    READS
        .iter()
        .map(|(args, review)| {
            let run = spec(scene, args);
            assert_eq!(run.code, 0, "{args:?}: {}", run.show());
            let mask = |text: &str| {
                if *review {
                    text.replace(&root, "<root>")
                } else {
                    text.to_owned()
                }
            };
            (run.code, mask(&run.stdout), mask(&run.stderr))
        })
        .collect()
}

/// DP20: two roots, two `HOME`s, opposite file orders, the second indexed
/// half-way: every read byte-equal, twice. M: practices in index row order
/// (the half-way index writes ghost's and shared's rows before art's).
#[test]
fn dp20_one_state_gives_one_answer_at_any_root_and_home() {
    let first = Scene::files("dp20-a", false);
    // Reversed, the practices come last, `shared.md` and `ghost.md` first of
    // them: indexed right after ghost's, their rows precede art's.
    let mut order: Vec<String> = practices_scene::scene_files()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    order.reverse();
    let half = order
        .iter()
        .position(|path| path == practices_scene::GHOST)
        .expect("ghost.md")
        + 1;
    let second = Scene::files_with("dp20-b", true, |root, home, n| {
        if n == half {
            let env: [(&str, &OsStr); 2] = [
                ("HOME", home.as_os_str()),
                ("PATH", OsStr::new("/usr/bin:/bin")),
            ];
            let run = spec_with(root, &["index"], &env);
            run.code(0);
        }
    });
    for scene in [&first, &second] {
        scene.with_queue();
        assert_eq!(scene.propose_cited(), "PR-0001");
    }
    assert_ne!(first.root, second.root);
    assert_ne!(first.home, second.home);
    let a = play(&first);
    let b = play(&second);
    for ((args, _), (one, other)) in READS.iter().zip(a.iter().zip(&b)) {
        assert_eq!(one, other, "{args:?}: one state, one answer");
    }
    assert_eq!(play(&first), a, "a repeat gives the same");
    // A premise: the reads carry what the slice adds.
    let (_, card, _) = &a[1];
    let card: Value = serde_json::from_str(card).expect("JSON");
    assert_eq!(card["cited"][0]["name"], json!("PRA-ART-LOD"));
    let (_, review, _) = &a[14];
    let review: Value = serde_json::from_str(review).expect("JSON");
    assert_eq!(review["cited"][0]["name"], json!("PRA-ART-NORMALS"));
    assert_eq!(
        review["worktree"],
        json!("<root>"),
        "the one masked value is the worktree: {review}"
    );
}

/// DP20, "Rules and edge cases": nothing is cached across calls: a rule
/// deleted between two reads is gone from the second, and a practice added
/// appears.
#[test]
fn nothing_is_cached_across_reads() {
    let scene = Scene::new("dp20-fresh");
    let names = |scene: &Scene| -> Vec<String> {
        let run = spec(scene, &["--json", "card", "show", "C-0001", "--practices"]);
        run.code(0);
        run.json()["practices"]
            .as_array()
            .expect("practices")
            .iter()
            .map(|entry| entry["name"].as_str().expect("a name").to_owned())
            .collect()
    };
    assert_eq!(names(&scene), ["PRA-ART-MODELS", "PRA-SHARED"]);
    scene.remove(practices_scene::SHARED);
    assert_eq!(
        names(&scene),
        ["PRA-ART-MODELS"],
        "a deleted practice is gone"
    );
    scene.put(
        "docs/spec/practices/zz/late.md",
        &practices_scene::practice_text(
            "PRA-ZZ",
            "Late rules",
            &["art"],
            &[(
                "PRA-ZZ-ONE".to_owned(),
                "One".to_owned(),
                "Late.\n".to_owned(),
            )],
        ),
    );
    assert_eq!(
        names(&scene),
        ["PRA-ART-MODELS", "PRA-ZZ"],
        "a new one appears"
    );
    let cited = spec(&scene, &["--json", "card", "show", "C-0001", "--practices"]);
    cited.code(0);
    assert_eq!(cited.json()["cited"][0]["name"], json!("PRA-ART-LOD"));
    scene.edit(
        practices_scene::ART,
        "## Level of detail {#PRA-ART-LOD}",
        "## Level of detail",
    );
    let again = spec(&scene, &["--json", "card", "show", "C-0001", "--practices"]);
    again.code(0);
    assert_eq!(
        again.json()["cited"],
        json!([]),
        "a rule deleted reads as text on the next read"
    );
}
