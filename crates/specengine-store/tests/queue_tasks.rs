//! docs/features/task-package.md, the store half ("Data": queue schema 4,
//! the transitions, the store's rows; docs/canon/tasks.md "Task-bound proposals", the
//! snapshot's refresh): `SqliteQueue`'s tasks and runs through the public
//! API only (`api.rs`: no test but `format.rs` names the SQL crate); the
//! raw steps (an older schema, a corrupt row) use the system's `sqlite3`.
//!
//! - Schema 4: a fresh queue makes `tasks` (16 `TEXT` columns), `runs`
//!   (`run` `INTEGER`, the rest `TEXT`, key `(task_id, run)`), both
//!   `STRICT`, and `proposals.task_id`; a database at queue schema 1, 2 or
//!   3 steps to 4 in one transaction, its rows kept, `task_id` `NULL`.
//! - IDs: the highest number plus one, `T-0001` first, past `T-9999`.
//! - Changes: the transition table, the run checks, one event each
//!   (`task.claimed`, `task.run_reported` with `run`), a compare-and-set on
//!   `{status, updated_at}` — refused within the same second too.
//! - Corrupt rows: `get_task` fails naming the task and column;
//!   `list_tasks` skips them into `unreadable`.
//! - Bound proposals: checked in the inserting transaction (another
//!   repository, a closed task, another worktree after the claim); a
//!   discrepancy's linked update bound with it; a task alone makes the
//!   queue occupied; the refresh of `applied_refreshing`.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Scratch, blake3_hex};
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_model::{RunOutcome, SnapshotPlace, TaskClaim, TaskCriterion, TaskStatus};
use specengine_store::{
    Decision, EVENT_TASK_APPROVED, EVENT_TASK_CANCELLED, EVENT_TASK_CHANGES_REQUESTED,
    EVENT_TASK_CLAIMED, EVENT_TASK_COMPLETED, EVENT_TASK_CREATED, EVENT_TASK_PLANNED,
    EVENT_TASK_REFRESHED, EVENT_TASK_RUN_REPORTED, Intake, NewIntake, NewProposal, NewTask,
    PROPOSAL_COLUMNS, Place, ProposalKind, ProposalQueue as _, QUEUE_SCHEMA_VERSION, QueueCounts,
    QueueError, RUN_COLUMNS, RefreshEntry, Restore, SnapshotEntry, SqliteQueue, StoredNote,
    TASK_COLUMNS, TaskChange, TaskRefresh, TaskSnapshot, patch_hash,
};

const T0: &str = "2026-10-07T09:00:00Z";
const T1: &str = "2026-10-07T09:00:01Z";
const PROJECT: &str = "demo";
const COMMON: &str = "/r/.git";

fn sqlite3(db: &Path, sql: &str) -> String {
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

fn new_task(targets: &[&str]) -> NewTask {
    NewTask {
        git_common_dir: COMMON.to_owned(),
        title: Some("Tune it".to_owned()),
        goal: None,
        targets: targets.iter().map(|id| (*id).to_owned()).collect(),
        author: Author::human(),
    }
}

fn place(worktree: &str) -> Place {
    Place {
        git_common_dir: COMMON.to_owned(),
        worktree: worktree.to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn update(target: &str, worktree: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/x.md".to_owned(),
        place: place(worktree),
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Why.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
    }
}

fn discrepancy(target: &str, worktree: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/x.md".to_owned(),
        place: place(worktree),
        author: Author::new(Some("writer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: vec![target.to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: "It departs.".to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "src/a.rs".to_owned(),
                qpath: None,
                lines: None,
                observed: "walks".to_owned(),
                documented: "rests".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(0),
            distinct_from: Vec::new(),
        },
    }
}

fn snapshot(nodes: &[(&str, &str)]) -> TaskSnapshot {
    TaskSnapshot {
        at: T0.to_owned(),
        by: "Owner <owner@example.invalid>".to_owned(),
        place: SnapshotPlace {
            worktree: "/r".to_owned(),
            root_rel: String::new(),
            branch: "t1".to_owned(),
            commit: "1".repeat(40),
        },
        nodes: nodes
            .iter()
            .map(|(id, hash)| SnapshotEntry {
                id: (*id).to_owned(),
                path: "docs/x.md".to_owned(),
                span_hash: (*hash).to_owned(),
                text: format!("text of {id}"),
            })
            .collect(),
    }
}

fn claim(worktree: &str) -> TaskChange {
    TaskChange::Claim {
        claim: TaskClaim {
            at: T0.to_owned(),
            role: "nest-developer".to_owned(),
            worktree: worktree.to_owned(),
            branch: "t1".to_owned(),
        },
        author: Author::new(Some("nest-developer".to_owned()), None, None).expect("a role"),
    }
}

fn report() -> TaskChange {
    TaskChange::Report {
        outcome: RunOutcome::Partial,
        summary: "Half.".to_owned(),
        changed_files: vec!["a.txt".to_owned()],
    }
}

/// `change` on `id` as read now (its `seen`), at `now`.
fn change(
    queue: &mut SqliteQueue,
    id: &str,
    change: &TaskChange,
    now: &str,
) -> Result<(specengine_store::Task, Option<u64>), QueueError> {
    let seen = queue.get_task(id).expect("get").expect("a task").seen();
    queue.change_task(id, &seen, change, now)
}

fn refusal(result: Result<impl std::fmt::Debug, QueueError>) -> String {
    match result {
        Err(QueueError::TaskRefused { reason, .. }) => reason,
        other => panic!("expected TaskRefused, got {other:?}"),
    }
}

/// `(type, payload)` of every `task.*` event.
fn task_events(queue: &SqliteQueue) -> Vec<(String, serde_json::Value)> {
    queue
        .events()
        .expect("events")
        .into_iter()
        .filter(|event| event.event_type.starts_with("task."))
        .map(|event| (event.event_type, event.payload))
        .collect()
}

// --------------------------------------------------------------- schema

/// Schema 4: `tasks` and `runs` as `PRAGMA table_info` gives them (names
/// in [`TASK_COLUMNS`] and [`RUN_COLUMNS`] order, `run` the one `INTEGER`,
/// the key `id`, `(task_id, run)`), both `STRICT`; `proposals` 41 columns,
/// `task_id` last. M: a column moved or retyped.
#[test]
fn schema_4_makes_the_tasks_and_runs_tables() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 4);
    let scratch = Scratch::new("qt-schema");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "4");
    let info = |table: &str| {
        sqlite3(
            &db,
            &format!(
                "SELECT name || ' ' || type || ' ' || pk FROM pragma_table_info('{table}') \
                 ORDER BY cid;"
            ),
        )
    };
    let mut want: Vec<String> = TASK_COLUMNS
        .iter()
        .map(|column| format!("{column} TEXT {}", u8::from(*column == "id")))
        .collect();
    assert_eq!(info("tasks"), want.join("\n"));
    want = RUN_COLUMNS
        .iter()
        .map(|column| {
            let kind = if *column == "run" { "INTEGER" } else { "TEXT" };
            let key = match *column {
                "task_id" => 1,
                "run" => 2,
                _ => 0,
            };
            format!("{column} {kind} {key}")
        })
        .collect();
    assert_eq!(info("runs"), want.join("\n"));
    assert_eq!(
        sqlite3(
            &db,
            "SELECT name, strict FROM pragma_table_list WHERE name IN ('tasks', 'runs') \
             ORDER BY name;"
        ),
        "runs|1\ntasks|1"
    );
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "41"
    );
    assert_eq!(PROPOSAL_COLUMNS[40], "task_id");
    // Iteration 2: the task's compare-and-set key, `revision`, its last
    // column, `TEXT` (decimal from `1`), 17 in all.
    assert_eq!(TASK_COLUMNS.len(), 17);
    assert_eq!(TASK_COLUMNS[16], "revision");
    assert!(
        info("tasks").ends_with("\nrevision TEXT 0"),
        "{}",
        info("tasks")
    );
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let task = queue.create_task(&new_task(&["A-1"]), T0).unwrap();
    assert_eq!(task.revision, 1);
    assert_eq!(
        sqlite3(&db, "SELECT revision, typeof(revision) FROM tasks;"),
        "1|text"
    );
}

/// A database at queue schema 1, 2 or 3 (a real queue's later columns and
/// its `tasks`, `runs` dropped) steps to 4 when opened: its row kept and
/// readable, `task_id` `NULL`, both tables made and empty, and a task
/// created after; stepped once. M: step 4 skipped from 1 or 2.
#[test]
fn schema_1_to_3_databases_step_to_4() {
    for (version, kept) in [(1, 24), (2, 35), (3, 40)] {
        let scratch = Scratch::new(&format!("qt-step-{version}"));
        let db = scratch.db("q");
        let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let made = queue.create(&update("A-1", "/r"), T0).expect("create");
        drop(queue);
        let drops: Vec<String> = PROPOSAL_COLUMNS[kept..]
            .iter()
            .map(|column| format!("ALTER TABLE proposals DROP COLUMN {column};"))
            .collect();
        sqlite3(
            &db,
            &format!(
                "{} DROP TABLE tasks; DROP TABLE runs; PRAGMA user_version = {version};",
                drops.join(" ")
            ),
        );
        assert_eq!(
            sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
            kept.to_string()
        );
        let mut queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
        assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "4", "from {version}");
        assert_eq!(
            sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
            "41"
        );
        let kept_row = queue.get(&made.id).expect("get").expect("kept");
        assert_eq!(kept_row.task_id, None, "from {version}");
        assert_eq!(kept_row.new_text, "new");
        assert_eq!(queue.list_tasks(None).expect("tasks").tasks, []);
        let task = queue.create_task(&new_task(&["A-1"]), T1).expect("a task");
        assert_eq!(task.id, "T-0001", "from {version}");
        drop(queue);
        drop(SqliteQueue::open(&db, PROJECT).expect("reopen"));
        assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "4");
    }
}

// ------------------------------------------------------------------- IDs

/// IDs: `T-0001` first, the highest number plus one (a gap kept; past
/// `T-9999` five digits), never another project's; `task.created`
/// `{id}`; the stored task `draft` with `[]` lists and `NULL` parts.
#[test]
fn task_ids_are_the_highest_plus_one() {
    let scratch = Scratch::new("qt-ids");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let first = queue.create_task(&new_task(&["A-1"]), T0).expect("create");
    assert_eq!(first.id, "T-0001");
    assert_eq!(first.status, TaskStatus::Draft);
    assert_eq!(first.project, PROJECT);
    assert_eq!(first.targets, ["A-1"]);
    assert_eq!(
        (first.created_at.as_str(), first.updated_at.as_str()),
        (T0, T0)
    );
    assert!(first.plan.is_none() && first.snapshot.is_none() && first.claim.is_none());
    assert!(first.criteria.is_empty() && first.affected_nodes.is_empty());
    assert!(first.owner_notes.is_empty() && first.runs.is_empty());
    assert_eq!(
        task_events(&queue),
        [(
            EVENT_TASK_CREATED.to_owned(),
            serde_json::json!({"id": "T-0001"})
        )]
    );
    assert!(matches!(
        queue.create_task(&new_task(&[]), T0),
        Err(QueueError::Invalid(_))
    ));
    sqlite3(&db, "UPDATE tasks SET id = 'T-9999' WHERE id = 'T-0001';");
    let next = queue.create_task(&new_task(&["A-2"]), T0).expect("create");
    assert_eq!(next.id, "T-10000");
    // Another project's handle on the same database numbers past them.
    let mut other = SqliteQueue::open(&db, "other").expect("open other");
    assert_eq!(
        other
            .create_task(&new_task(&["B-1"]), T0)
            .expect("other")
            .id,
        "T-10001"
    );
    assert!(
        other.get_task("T-9999").expect("get").is_none(),
        "not its own"
    );
    assert_eq!(
        queue
            .list_tasks(None)
            .expect("list")
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["T-9999", "T-10000"]
    );
}

// --------------------------------------------------------------- changes

/// The transition table through `change_task`: plan, changes (the note
/// kept, oldest first), plan, approve, claim (run 1 opened, the claim
/// set), report (closed with its outcome), complete; one event each,
/// `task.claimed` and `task.run_reported` with `run`; refused pairs name
/// the states and change nothing; report with no run open and complete
/// with the run open are refused.
#[test]
fn every_change_is_one_event_and_refused_pairs_change_nothing() {
    let scratch = Scratch::new("qt-changes");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = queue
        .create_task(&new_task(&["A-1"]), T0)
        .expect("create")
        .id;
    let plan = TaskChange::Plan {
        plan: "1. Do.".to_owned(),
        criteria: vec![TaskCriterion {
            reference: None,
            text: Some("Holds.".to_owned()),
        }],
        affected_nodes: vec!["A-2".to_owned()],
    };
    let note = |text: &str, at: &str| TaskChange::Changes {
        note: StoredNote {
            at: at.to_owned(),
            note: text.to_owned(),
            by: "Owner <owner@example.invalid>".to_owned(),
        },
    };
    let before = queue.dump().unwrap();
    for (bad, want) in [
        (
            claim("/r"),
            "T-0001 is draft: `claim` needs ready; nothing changed",
        ),
        (
            report(),
            "T-0001 is draft: `report` needs in_progress; nothing changed",
        ),
        (
            TaskChange::Complete,
            "T-0001 is draft: `complete` needs in_progress; nothing changed",
        ),
        (
            note("x", T0),
            "T-0001 is draft: `changes` needs review; nothing changed",
        ),
    ] {
        assert_eq!(refusal(change(&mut queue, &id, &bad, T0)), want);
        assert_eq!(queue.dump().unwrap(), before);
    }
    let steps: Vec<(TaskChange, TaskStatus, &str, Option<u64>)> = vec![
        (plan.clone(), TaskStatus::Review, EVENT_TASK_PLANNED, None),
        (
            note("First.", T0),
            TaskStatus::ChangesRequested,
            EVENT_TASK_CHANGES_REQUESTED,
            None,
        ),
        (plan.clone(), TaskStatus::Review, EVENT_TASK_PLANNED, None),
        (
            note("Second.", T1),
            TaskStatus::ChangesRequested,
            EVENT_TASK_CHANGES_REQUESTED,
            None,
        ),
        (plan, TaskStatus::Review, EVENT_TASK_PLANNED, None),
        (
            TaskChange::Approve {
                snapshot: snapshot(&[("A-1", "b3:01")]),
            },
            TaskStatus::Ready,
            EVENT_TASK_APPROVED,
            None,
        ),
        (
            claim("/r"),
            TaskStatus::InProgress,
            EVENT_TASK_CLAIMED,
            Some(1),
        ),
        (
            report(),
            TaskStatus::InProgress,
            EVENT_TASK_RUN_REPORTED,
            Some(1),
        ),
        (
            TaskChange::Complete,
            TaskStatus::Done,
            EVENT_TASK_COMPLETED,
            None,
        ),
    ];
    for (number, (step, status, event, run)) in steps.into_iter().enumerate() {
        let now = format!("2026-10-07T10:00:{number:02}Z");
        let (stored, opened) = change(&mut queue, &id, &step, &now).expect("a change");
        assert_eq!(stored.status, status, "{event}");
        assert_eq!(stored.updated_at, now, "{event}");
        assert_eq!(opened, run, "{event}");
        let (kind, payload) = task_events(&queue).pop().unwrap();
        assert_eq!(kind, event);
        let mut want = serde_json::json!({"id": id});
        if let Some(run) = run {
            want["run"] = serde_json::json!(run);
        }
        assert_eq!(payload, want, "{event}");
        if event == EVENT_TASK_RUN_REPORTED {
            // Complete with no open run passes; report again is refused.
            assert_eq!(
                refusal(change(&mut queue, &id, &report(), &now)),
                "T-0001 is in_progress, no run open: `report` needs its open run (`claim` \
                 opens one); nothing changed"
            );
        }
        if event == EVENT_TASK_CLAIMED {
            assert_eq!(
                refusal(change(&mut queue, &id, &TaskChange::Complete, &now)),
                "T-0001 is in_progress, its run 1 open: `complete` needs it closed (`report`); \
                 nothing changed"
            );
        }
    }
    let done = queue.get_task(&id).unwrap().unwrap();
    assert_eq!(
        done.owner_notes
            .iter()
            .map(|note| note.note.as_str())
            .collect::<Vec<_>>(),
        ["First.", "Second."],
        "oldest first"
    );
    assert_eq!(done.runs.len(), 1);
    let run = &done.runs[0];
    assert_eq!((run.run, run.role.as_str()), (1, "nest-developer"));
    assert_eq!(run.outcome, Some(RunOutcome::Partial));
    assert_eq!(run.ended_at.as_deref(), Some("2026-10-07T10:00:07Z"));
    assert_eq!(run.changed_files, ["a.txt"]);
    assert_eq!(done.claim.as_ref().unwrap().role, "nest-developer");
    assert_eq!(task_events(&queue).len(), 10, "created + nine changes");
    let before = queue.dump().unwrap();
    assert_eq!(
        refusal(change(&mut queue, &id, &TaskChange::Cancel, T1)),
        "T-0001 is done: `cancel` needs draft, review, changes_requested, ready or in_progress; \
         nothing changed"
    );
    assert_eq!(queue.dump().unwrap(), before);
    let cancelled = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    let (stored, _) = change(&mut queue, &cancelled, &TaskChange::Cancel, T1).unwrap();
    assert_eq!(stored.status, TaskStatus::Cancelled);
    assert_eq!(task_events(&queue).pop().unwrap().0, EVENT_TASK_CANCELLED);
}

/// The compare-and-set: two runs read the same task in the same second;
/// the first changes it, the second's change (on what it read) is
/// refused naming the state since, nothing written — `updated_at` equal,
/// `status` not. A missing task is refused too.
#[test]
fn a_change_on_a_stale_read_is_refused_within_the_same_second() {
    let scratch = Scratch::new("qt-cas");
    let db = scratch.db("q");
    let mut first = SqliteQueue::open(&db, PROJECT).expect("open");
    let mut second = SqliteQueue::open(&db, PROJECT).expect("open again");
    let id = first.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    let seen = second.get_task(&id).unwrap().unwrap().seen();
    first
        .change_task(
            &id,
            &first.get_task(&id).unwrap().unwrap().seen(),
            &TaskChange::Approve {
                snapshot: snapshot(&[("A-1", "b3:01")]),
            },
            T0,
        )
        .expect("the first run");
    let before = first.dump().unwrap();
    let reason = refusal(second.change_task(&id, &seen, &TaskChange::Cancel, T0));
    assert_eq!(
        reason,
        format!("`T-0001` changed since this run read it: it is ready since {T0}; nothing changed")
    );
    assert_eq!(first.dump().unwrap(), before);
    assert_eq!(
        refusal(second.change_task("T-0042", &seen, &TaskChange::Cancel, T0)),
        "no task `T-0042` in this project's queue"
    );
    // The time is checked as every queue time.
    assert!(matches!(
        first.change_task(&id, &seen, &TaskChange::Cancel, "now"),
        Err(QueueError::Invalid(_))
    ));
}

// --------------------------------------------------------- corrupt rows

/// A task row whose JSON does not decode, a `NULL` required column, an
/// unknown state, a run with an unknown outcome or a run number below 1:
/// `get_task` fails naming the task and column; `list_tasks` skips each
/// into `unreadable`, the readable ones listed. M: a corrupt row listed.
#[test]
fn a_corrupt_task_or_run_row_is_named() {
    let scratch = Scratch::new("qt-corrupt");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    for _ in 0..6 {
        queue.create_task(&new_task(&["A-1"]), T0).unwrap();
    }
    for id in ["T-0004", "T-0005"] {
        change(
            &mut queue,
            id,
            &TaskChange::Approve {
                snapshot: snapshot(&[("A-1", "b3:01")]),
            },
            T0,
        )
        .unwrap();
        change(&mut queue, id, &claim("/r"), T0).unwrap();
    }
    sqlite3(
        &db,
        "UPDATE tasks SET criteria = '{' WHERE id = 'T-0001'; \
         UPDATE tasks SET updated_at = NULL WHERE id = 'T-0002'; \
         UPDATE tasks SET status = 'paused' WHERE id = 'T-0003'; \
         UPDATE runs SET outcome = 'stalled' WHERE task_id = 'T-0004'; \
         UPDATE runs SET run = 0 WHERE task_id = 'T-0005';",
    );
    for (id, column) in [
        ("T-0001", "criteria"),
        ("T-0002", "updated_at"),
        ("T-0003", "status"),
        ("T-0004", "runs[1].outcome"),
        ("T-0005", "runs.run"),
    ] {
        match queue.get_task(id) {
            Err(error) => {
                let message = error.to_string();
                assert!(
                    message.contains(&format!("task {id}: the stored `{column}` cannot be read")),
                    "{id}: {message}"
                );
            }
            Ok(task) => panic!("{id} read: {task:?}"),
        }
    }
    let listed = queue.list_tasks(None).expect("list");
    assert_eq!(
        listed
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        ["T-0006"]
    );
    assert_eq!(
        listed
            .unreadable
            .iter()
            .map(|row| (row.id.as_str(), row.column.as_str()))
            .collect::<Vec<_>>(),
        [
            ("T-0001", "criteria"),
            ("T-0002", "updated_at"),
            ("T-0003", "status"),
            ("T-0004", "runs[1].outcome"),
            ("T-0005", "runs.run")
        ]
    );
    // A proposal row whose `task_id` is no task ID is corrupt, named.
    let made = queue.create(&update("A-1", "/r"), T0).unwrap();
    sqlite3(
        &db,
        &format!(
            "UPDATE proposals SET task_id = 'X-1' WHERE id = '{}';",
            made.id
        ),
    );
    let message = queue.get(&made.id).unwrap_err().to_string();
    assert!(message.contains("task_id"), "{message}");
}

// ----------------------------------------------------- bound proposals

/// A proposal bound to a task: checked in the inserting transaction —
/// another repository's task, a closed one, an unknown one, or (after the
/// claim) a place in another worktree refuses, nothing stored; bound, its
/// `task_id` stored; a discrepancy's linked update bound with it in the
/// same transaction. M: the linked update's `task_id` NULL.
#[test]
fn a_bound_proposal_is_checked_where_it_is_stored() {
    let scratch = Scratch::new("qt-bound");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    let before = queue.dump().unwrap();
    let mut foreign = update("A-1", "/r");
    foreign.place.git_common_dir = "/other/.git".to_owned();
    assert!(
        refusal(queue.create_with_task(&foreign, Some(&id), T0))
            .contains("is a task of another repository")
    );
    assert_eq!(
        refusal(queue.create_with_task(&update("A-1", "/r"), Some("T-0042"), T0)),
        "no task `T-0042` in this project's queue"
    );
    assert_eq!(queue.dump().unwrap(), before, "nothing stored");

    let bound = queue
        .create_with_task(&update("A-1", "/r/t2"), Some(&id), T0)
        .expect("bound before the claim, anywhere");
    assert_eq!(bound.task_id.as_deref(), Some("T-0001"));
    let result = queue
        .create_intake_with_task(
            &discrepancy("A-1", "/r/t2"),
            &[],
            Some(&update("A-1", "/r/t2")),
            Some(&id),
            T0,
        )
        .expect("intake");
    let stored = result.created.expect("stored");
    let linked = result.linked.expect("linked");
    assert_eq!(stored.task_id.as_deref(), Some("T-0001"));
    assert_eq!(
        linked.task_id.as_deref(),
        Some("T-0001"),
        "the linked update"
    );
    assert_eq!(
        queue.get(&linked.id).unwrap().unwrap().task_id.as_deref(),
        Some("T-0001")
    );

    change(
        &mut queue,
        &id,
        &TaskChange::Approve {
            snapshot: snapshot(&[("A-1", "b3:01")]),
        },
        T0,
    )
    .unwrap();
    change(&mut queue, &id, &claim("/r/t1"), T0).unwrap();
    let before = queue.dump().unwrap();
    let reason = refusal(queue.create_with_task(&update("A-1", "/r/t2"), Some(&id), T0));
    assert!(
        reason.contains("is claimed in the worktree /r/t1") && reason.contains("not in /r/t2"),
        "{reason}"
    );
    assert!(
        refusal(queue.create_intake_with_task(
            &discrepancy("A-1", "/r/t2"),
            &[],
            None,
            Some(&id),
            T0
        ))
        .contains("is claimed in the worktree /r/t1")
    );
    assert_eq!(queue.dump().unwrap(), before);
    queue
        .create_with_task(&update("A-1", "/r/t1"), Some(&id), T0)
        .expect("from the claimed worktree");
    change(&mut queue, &id, &report(), T0).unwrap();
    change(&mut queue, &id, &TaskChange::Complete, T0).unwrap();
    assert!(
        refusal(queue.create_with_task(&update("A-1", "/r/t1"), Some(&id), T0)).contains(
            "is done: a proposal is bound only to a task that is neither done nor \
                       cancelled"
        )
    );
}

/// `applied_refreshing`: a proposal bound to the refresh's task records
/// its commit and, in that transaction, each snapshot node whose `was` is
/// still its frozen hash takes the applied text and hash (one
/// `task.refreshed` `{id, proposal, node}` each, the task's `updated_at`
/// now); a node frozen at another hash is left; a refresh naming another
/// task refreshes nothing and the commit is still recorded. M: every
/// apply refreshes.
#[test]
fn applied_refreshing_refreshes_only_nodes_still_frozen() {
    let scratch = Scratch::new("qt-refresh");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    let other = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    for task in [&id, &other] {
        change(
            &mut queue,
            task,
            &TaskChange::Approve {
                snapshot: snapshot(&[("A-1", "b3:01"), ("A-2", "b3:02"), ("A-3", "b3:03")]),
            },
            T0,
        )
        .unwrap();
    }
    let decision = Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: None,
    };
    let bound = queue
        .create_with_task(&update("A-2", "/r"), Some(&id), T0)
        .unwrap();
    queue.approve(&bound.id, &decision, T0).unwrap();
    let entry = |node: &str, was: &str| RefreshEntry {
        id: node.to_owned(),
        was: was.to_owned(),
        span_hash: format!("b3:new-{node}"),
        text: format!("new text of {node}"),
    };
    let refresh = TaskRefresh {
        task_id: id.clone(),
        entries: vec![
            entry("A-1", "b3:01"),
            entry("A-2", "b3:02"),
            entry("A-3", "b3:99"),
        ],
    };
    let (applied, refreshed) = queue
        .applied_refreshing(&bound.id, &"c".repeat(40), None, &refresh, T1)
        .expect("applied");
    assert_eq!(applied.status.as_str(), "applied");
    assert_eq!(refreshed, ["A-1", "A-2"]);
    let task = queue.get_task(&id).unwrap().unwrap();
    let nodes = &task.snapshot.as_ref().unwrap().nodes;
    assert_eq!(
        nodes
            .iter()
            .map(|node| (
                node.id.as_str(),
                node.span_hash.as_str(),
                node.text.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            ("A-1", "b3:new-A-1", "new text of A-1"),
            ("A-2", "b3:new-A-2", "new text of A-2"),
            ("A-3", "b3:03", "text of A-3")
        ]
    );
    assert_eq!(task.updated_at, T1);
    assert_eq!(task.revision, 3, "a refresh raises the revision");
    let events: Vec<serde_json::Value> = task_events(&queue)
        .into_iter()
        .filter(|(kind, _)| kind == EVENT_TASK_REFRESHED)
        .map(|(_, payload)| payload)
        .collect();
    assert_eq!(
        events,
        [
            serde_json::json!({"id": id, "proposal": bound.id, "node": "A-1"}),
            serde_json::json!({"id": id, "proposal": bound.id, "node": "A-2"})
        ]
    );
    // The other task's snapshot untouched.
    let untouched = queue.get_task(&other).unwrap().unwrap();
    assert_eq!(untouched.revision, 2);
    assert_eq!(untouched.snapshot.unwrap().nodes[0].span_hash, "b3:01");

    // An unbound proposal with a refresh: recorded, nothing refreshed.
    let free = queue.create(&update("A-1", "/r"), T0).unwrap();
    queue.approve(&free.id, &decision, T0).unwrap();
    let refresh = TaskRefresh {
        task_id: other.clone(),
        entries: vec![entry("A-1", "b3:01")],
    };
    let (applied, refreshed) = queue
        .applied_refreshing(&free.id, &"d".repeat(40), None, &refresh, T1)
        .expect("applied");
    assert_eq!(applied.status.as_str(), "applied");
    assert!(refreshed.is_empty());
    assert_eq!(
        queue
            .get_task(&other)
            .unwrap()
            .unwrap()
            .snapshot
            .unwrap()
            .nodes[0]
            .span_hash,
        "b3:01"
    );
}

/// A task alone (no proposal, its event deleted) makes the queue
/// occupied: `restore` answers `Occupied` with `tasks` 1, nothing
/// inserted; `counts` names tasks and runs.
#[test]
fn a_task_alone_makes_the_queue_occupied() {
    let scratch = Scratch::new("qt-occupied");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    change(
        &mut queue,
        &id,
        &TaskChange::Approve {
            snapshot: snapshot(&[("A-1", "b3:01")]),
        },
        T0,
    )
    .unwrap();
    change(&mut queue, &id, &claim("/r"), T0).unwrap();
    sqlite3(&db, "DELETE FROM events;");
    assert_eq!(
        queue.counts().unwrap(),
        QueueCounts {
            proposals: 0,
            tasks: 1,
            runs: 1,
            events: 0
        }
    );
    let state = queue.stored_rows().unwrap();
    assert_eq!((state.tasks.len(), state.runs.len()), (1, 1));
    let before = queue.dump().unwrap();
    assert_eq!(
        queue.restore(&state).unwrap(),
        Restore::Occupied(QueueCounts {
            proposals: 0,
            tasks: 1,
            runs: 1,
            events: 0
        })
    );
    assert_eq!(queue.dump().unwrap(), before);
    // Restored into an empty queue: the same rows; a run of no task of the
    // state is invalid, nothing written.
    let fresh_db = scratch.db("fresh");
    let mut fresh = SqliteQueue::open(&fresh_db, PROJECT).expect("open fresh");
    let mut orphan = state.clone();
    orphan.tasks.clear();
    assert!(matches!(
        fresh.restore(&orphan),
        Err(QueueError::Invalid(_))
    ));
    assert!(fresh.counts().unwrap().is_empty());
    assert_eq!(fresh.restore(&state).unwrap(), Restore::Restored);
    assert_eq!(fresh.stored_rows().unwrap(), state);
    assert_eq!(fresh.get_task(&id).unwrap(), queue.get_task(&id).unwrap());
    assert_eq!(
        fresh.create_task(&new_task(&["A-1"]), T1).unwrap().id,
        "T-0002",
        "the next ID follows the highest restored"
    );
}

// ------------------------------------------------------- iteration 2

/// The `revision`: `1` at creation, raised by one with every change (a
/// refused one leaves it); two runs that read the same `ready` task make
/// the same-status change (approve again) in the same second: the second
/// is refused — `status` and `updated_at` alike, `revision` not —
/// nothing written; likewise a `complete` read before a `report` landed.
/// M: the revision never raised.
#[test]
fn the_revision_refuses_a_second_same_status_change() {
    let scratch = Scratch::new("qt-revision");
    let db = scratch.db("q");
    let mut first = SqliteQueue::open(&db, PROJECT).expect("open");
    let mut second = SqliteQueue::open(&db, PROJECT).expect("open again");
    let id = first.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    let approve = TaskChange::Approve {
        snapshot: snapshot(&[("A-1", "b3:01")]),
    };
    let (task, _) = change(&mut first, &id, &approve, T0).unwrap();
    assert_eq!((task.status, task.revision), (TaskStatus::Ready, 2));

    let seen_a = first.get_task(&id).unwrap().unwrap().seen();
    let seen_b = second.get_task(&id).unwrap().unwrap().seen();
    assert_eq!(seen_a, seen_b);
    let (task, _) = first
        .change_task(&id, &seen_a, &approve, T0)
        .expect("the first re-approval");
    assert_eq!(
        (task.status, task.updated_at.as_str(), task.revision),
        (TaskStatus::Ready, T0, 3)
    );
    let before = first.dump().unwrap();
    assert_eq!(
        refusal(second.change_task(&id, &seen_b, &approve, T0)),
        format!("`T-0001` changed since this run read it: it is ready since {T0}; nothing changed")
    );
    assert_eq!(first.dump().unwrap(), before, "nothing written");

    // in_progress twice: a complete read before the report landed.
    change(&mut first, &id, &claim("/r"), T0).unwrap();
    let stale = second.get_task(&id).unwrap().unwrap().seen();
    change(&mut first, &id, &report(), T0).unwrap();
    let after_report = first.get_task(&id).unwrap().unwrap();
    assert_eq!(
        (
            after_report.status,
            after_report.updated_at.as_str(),
            after_report.revision
        ),
        (TaskStatus::InProgress, T0, 5)
    );
    assert_eq!(
        (stale.status, stale.updated_at.as_str()),
        (TaskStatus::InProgress, T0),
        "the same status and time as read"
    );
    assert!(
        refusal(second.change_task(&id, &stale, &TaskChange::Complete, T0))
            .contains("changed since this run read it")
    );
    // A refused change leaves the revision.
    let _ = change(&mut first, &id, &report(), T0);
    assert_eq!(first.get_task(&id).unwrap().unwrap().revision, 5);
    let (done, _) = change(&mut first, &id, &TaskChange::Complete, T0).unwrap();
    assert_eq!(done.revision, 6);
    // The dump carries it as text, last; a restore keeps it; the next
    // change raises it from there.
    let dump = first.dump().unwrap();
    let row = dump
        .lines()
        .find(|line| line.starts_with("tasks\t"))
        .expect("a task row");
    assert!(row.ends_with(",\"6\"]"), "{row}");
    let state = first.stored_rows().unwrap();
    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("fresh");
    assert_eq!(fresh.restore(&state).unwrap(), Restore::Restored);
    assert_eq!(fresh.get_task(&id).unwrap().unwrap().revision, 6);
    let other = fresh.create_task(&new_task(&["A-1"]), T1).unwrap().id;
    let (cancelled, _) = change(&mut fresh, &other, &TaskChange::Cancel, T1).unwrap();
    assert_eq!(cancelled.revision, 2);
}

/// A `revision` that is no decimal from 1 (`0`, `01`, `x`, `NULL`) is a
/// corrupt row named by its column.
#[test]
fn a_bad_revision_is_a_corrupt_row() {
    let scratch = Scratch::new("qt-bad-revision");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    for _ in 0..4 {
        queue.create_task(&new_task(&["A-1"]), T0).unwrap();
    }
    sqlite3(
        &db,
        "UPDATE tasks SET revision = '0' WHERE id = 'T-0001'; \
         UPDATE tasks SET revision = '01' WHERE id = 'T-0002'; \
         UPDATE tasks SET revision = 'x' WHERE id = 'T-0003'; \
         UPDATE tasks SET revision = NULL WHERE id = 'T-0004';",
    );
    for id in ["T-0001", "T-0002", "T-0003", "T-0004"] {
        let message = queue.get_task(id).unwrap_err().to_string();
        assert!(
            message.contains(&format!("task {id}: the stored `revision` cannot be read")),
            "{id}: {message}"
        );
    }
    assert_eq!(queue.list_tasks(None).unwrap().unreadable.len(), 4);
}

/// Iteration 2, n1: cancelling an `in_progress` task ends its open run in
/// the same transaction: `ended_at` the cancel's time, `outcome` and
/// `summary` `None`; a reported run keeps its end. M: the run left open.
#[test]
fn cancel_ends_the_open_run() {
    let scratch = Scratch::new("qt-cancel-run");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
    change(
        &mut queue,
        &id,
        &TaskChange::Approve {
            snapshot: snapshot(&[("A-1", "b3:01")]),
        },
        T0,
    )
    .unwrap();
    change(&mut queue, &id, &claim("/r"), T0).unwrap();
    let (cancelled, run) = change(&mut queue, &id, &TaskChange::Cancel, T1).unwrap();
    assert_eq!(run, None);
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert!(cancelled.open_run().is_none());
    let ended = &cancelled.runs[0];
    assert_eq!(ended.ended_at.as_deref(), Some(T1));
    assert_eq!((ended.outcome, ended.summary.as_deref()), (None, None));
    assert_eq!(task_events(&queue).pop().unwrap().0, EVENT_TASK_CANCELLED);
}

/// Iteration 2, n4: `applied_refreshing` on a bound proposal whose task is
/// `cancelled` or `done` records the commit and refreshes nothing: no
/// event, the snapshot and `revision` as they were. M: closed tasks
/// refreshed.
#[test]
fn a_closed_tasks_snapshot_is_never_refreshed() {
    let scratch = Scratch::new("qt-closed-refresh");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let decision = Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: None,
    };
    for closing in [TaskChange::Cancel, TaskChange::Complete] {
        let id = queue.create_task(&new_task(&["A-1"]), T0).unwrap().id;
        change(
            &mut queue,
            &id,
            &TaskChange::Approve {
                snapshot: snapshot(&[("A-1", "b3:01")]),
            },
            T0,
        )
        .unwrap();
        let bound = queue
            .create_with_task(&update("A-1", "/r"), Some(&id), T0)
            .unwrap();
        queue.approve(&bound.id, &decision, T0).unwrap();
        if closing == TaskChange::Complete {
            change(&mut queue, &id, &claim("/r"), T0).unwrap();
            change(&mut queue, &id, &report(), T0).unwrap();
        }
        change(&mut queue, &id, &closing, T0).unwrap();
        let before = queue.get_task(&id).unwrap().unwrap();
        let refresh = TaskRefresh {
            task_id: id.clone(),
            entries: vec![RefreshEntry {
                id: "A-1".to_owned(),
                was: "b3:01".to_owned(),
                span_hash: "b3:new".to_owned(),
                text: "new".to_owned(),
            }],
        };
        let (applied, refreshed) = queue
            .applied_refreshing(&bound.id, &"c".repeat(40), None, &refresh, T1)
            .expect("applied");
        assert_eq!(applied.status.as_str(), "applied");
        assert!(refreshed.is_empty(), "{closing:?}");
        assert_eq!(queue.get_task(&id).unwrap().unwrap(), before, "{closing:?}");
    }
    assert!(
        task_events(&queue)
            .iter()
            .all(|(kind, _)| kind != EVENT_TASK_REFRESHED)
    );
}
