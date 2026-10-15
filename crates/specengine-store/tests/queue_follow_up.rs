//! docs/features/drift-follow-up.md (4c-2), the store's half: `SqliteQueue::
//! reject_with_task(id, &Seen, &Decision, &NewTask, now)` ("Data", step 3;
//! "Rules and edge cases"):
//!
//! - RD-1 (one transaction, injected): a task the store refuses (`targets`
//!   empty: `Invalid`), a task ID past the last one (a store failure inside
//!   the write), a lost compare-and-set (`Changed`), a closed item
//!   (`Status`): the item as it was, no task row, no event (the queue's
//!   dump byte for byte).
//! - Iteration 1's reading (deviation 8): another kind is `Invalid`,
//!   nothing written.
//! - RD-3: made, the item `rejected` with the decision and a `draft` task
//!   as `create_task` stores one; `proposal.rejected` gains `task` (its
//!   payload `{"id","reason","task"}` as stored), then `task.created`;
//!   `reject_from` alone logs no `task` key.
//!
//! Through the public API; the raw reads and the injected task row use the
//! system's `sqlite3` client. The named mutations are in each test's `M:`.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use serde_json::json;
use specengine_core::drift::{DriftEvidence, DriftLock, DriftMarker, DriftRow};
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_model::TaskStatus;
use specengine_store::{
    Decision, DriftResult, EVENT_REJECTED, EVENT_TASK_CREATED, Intake, NewDrift, NewIntake,
    NewTask, Place, ProposalKind, ProposalQueue as _, ProposalStatus, QueueError, Seen,
    SqliteQueue,
};

const T0: &str = "2026-10-15T09:00:00Z";
const T1: &str = "2026-10-15T09:10:00Z";
const PROJECT: &str = "code-a";
const COMMON: &str = "/r/.git";
const NODE: &str = "LAW-BOUND";
const REASON: &str = "The cap stays; bring the code back.";
const OWNER: &str = "Owner <owner@example.invalid>";

fn place() -> Place {
    Place {
        git_common_dir: COMMON.to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "main".to_owned(),
        base_commit: "a".repeat(40),
    }
}

fn agent() -> Author {
    Author::new(
        Some("developer".to_owned()),
        Some("claude-opus-5-5".to_owned()),
        None,
    )
    .expect("an agent")
}

fn owner() -> Decision {
    Decision {
        decided_by: OWNER.to_owned(),
        note: Some(REASON.to_owned()),
        staged_at: None,
    }
}

/// One stored row of `spec verify --json`'s shape: `CAP`'s `code_ahead`.
fn row() -> DriftRow {
    DriftRow {
        node: NODE.to_owned(),
        relation: "configures".to_owned(),
        file: "crates/tally/src/counter.rs".to_owned(),
        kind: Some("item".to_owned()),
        symbol: Some("crates/tally::counter::CAP".to_owned()),
        item_kind: Some("const_item".to_owned()),
        state: "code_ahead".to_owned(),
        category: Some("code_changed".to_owned()),
        detail: None,
        class: Some("check".to_owned()),
        pre_existing: true,
        markers: vec![DriftMarker {
            line: 8,
            rev: Some(2),
        }],
        node_rev: Some(2),
        lock: Some(DriftLock {
            rev: 2,
            commit: "8".repeat(40),
            verified_at: "2026-10-08".to_owned(),
        }),
        compared: Some("ast".to_owned()),
    }
}

fn drift() -> NewDrift {
    NewDrift {
        target_id: NODE.to_owned(),
        target_path: "spec.lock".to_owned(),
        place: place(),
        author: agent(),
        task_id: None,
        severity: IntakeSeverity::Normal,
        summary: "code_ahead code_changed".to_owned(),
        evidence: serde_json::to_string(&DriftEvidence {
            rows: vec![row()],
            more: 0,
        })
        .expect("JSON"),
        hash: format!("b3:{}", "1".repeat(64)),
    }
}

/// The follow-up's task as the CLI makes it (step 3): the item's
/// repository, `targets` as given, a human's.
fn follow_up_task(targets: &[&str]) -> NewTask {
    NewTask {
        git_common_dir: COMMON.to_owned(),
        title: Some(format!(
            "Bring the code bound to {NODE} in line with its spec (PR-0001)"
        )),
        goal: Some(format!("{NODE} rev 2 (docs/spec/counter.md) is the spec.")),
        targets: targets.iter().map(|id| (*id).to_owned()).collect(),
        author: Author::human(),
    }
}

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

/// A queue at `scratch`'s `q.db` holding one open drift item: the handle,
/// the database and the item's ID.
fn with_item(scratch: &Scratch) -> (SqliteQueue, std::path::PathBuf, String) {
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = match queue
        .create_drift(&[drift()], T0)
        .expect("create_drift")
        .remove(0)
    {
        DriftResult::Created { proposal, .. } => proposal.id.clone(),
        other => panic!("expected a new item, got {other:?}"),
    };
    (queue, db, id)
}

/// The item's state as read now: the key of the compare-and-set.
fn seen(queue: &SqliteQueue, id: &str) -> Seen {
    queue
        .get(id)
        .expect("get")
        .unwrap_or_else(|| panic!("no {id}"))
        .seen()
}

/// Asserts a refused `reject_with_task` wrote nothing: the dump (every
/// proposal, task, run and event) byte for byte as `dump`, so no task row
/// and no event; the item as it was (`open`, its time, undecided).
fn nothing_written(queue: &SqliteQueue, id: &str, dump: &str, context: &str) {
    assert_eq!(queue.dump().expect("dump"), dump, "{context}: the dump");
    let item = queue.get(id).expect("get").expect("the item");
    assert_eq!(item.status, ProposalStatus::Open, "{context}");
    assert_eq!(item.updated_at, T0, "{context}");
    assert!(
        item.decided_by.is_none() && item.decision_note.is_none(),
        "{context}"
    );
}

// ------------------------------------------------------------------ RD-1

/// RD-1 ("Rules and edge cases": one transaction; a failure leaves
/// neither, and no event): a task the store refuses (`targets` empty) is
/// `Invalid` and the item stays `open`, no event, no task row; a failure
/// inside the write (the queue holds the last task ID there is, so no task
/// ID follows it) leaves the item alike and the task row untouched.
/// M: the reject committed in a transaction of its own before the task's.
#[test]
fn follow_up_and_reject_are_one_transaction() {
    let scratch = Scratch::new("qfu-one");
    let (mut queue, db, id) = with_item(&scratch);
    let read = seen(&queue, &id);
    let dump = queue.dump().expect("dump");
    match queue.reject_with_task(&id, &read, &owner(), &follow_up_task(&[]), T1) {
        Err(QueueError::Invalid(_)) => {}
        other => panic!("a task naming no node: expected Invalid, got {other:?}"),
    }
    nothing_written(&queue, &id, &dump, "a task the store refuses");
    assert!(queue.list_tasks(None).expect("tasks").tasks.is_empty());

    // A failure inside the write: no task ID follows the last one.
    let held = queue
        .create_task(&follow_up_task(&["LAW-STEP"]), T0)
        .expect("a task");
    assert_eq!(held.id, "T-0001");
    sqlite3(
        &db,
        "UPDATE tasks SET id = 'T-18446744073709551615' WHERE id = 'T-0001';",
    );
    let read = seen(&queue, &id);
    let dump = queue.dump().expect("dump");
    let failed = queue.reject_with_task(&id, &read, &owner(), &follow_up_task(&[NODE]), T1);
    assert!(failed.is_err(), "past the last task ID: {failed:?}");
    nothing_written(&queue, &id, &dump, "no task ID left");
    assert_eq!(
        sqlite3(&db, "SELECT id FROM tasks ORDER BY id;"),
        "T-18446744073709551615",
        "no task row added"
    );
}

/// RD-1: the compare-and-set of `reject_from`: a state read before
/// another run's change (`Changed`), or an item already `rejected`
/// (`Status`), stores neither the reject nor the task, logs nothing.
/// M: the task stored before the compare-and-set, in its own transaction.
#[test]
fn reject_with_task_is_a_compare_and_set() {
    let scratch = Scratch::new("qfu-cas");
    let (mut queue, _db, id) = with_item(&scratch);
    let stale = Seen {
        updated_at: "2026-10-15T08:59:59Z".to_owned(),
        ..seen(&queue, &id)
    };
    let dump = queue.dump().expect("dump");
    match queue.reject_with_task(&id, &stale, &owner(), &follow_up_task(&[NODE]), T1) {
        Err(QueueError::Changed { .. }) => {}
        other => panic!("a stale read: expected Changed, got {other:?}"),
    }
    nothing_written(&queue, &id, &dump, "a stale read");
    assert!(queue.list_tasks(None).expect("tasks").tasks.is_empty());

    // Already rejected: refused by its state, no task.
    let read = seen(&queue, &id);
    queue
        .reject_from(&id, &read, &owner(), T1)
        .expect("a plain reject");
    let read = seen(&queue, &id);
    let dump = queue.dump().expect("dump");
    let again = queue.reject_with_task(&id, &read, &owner(), &follow_up_task(&[NODE]), T1);
    assert!(
        matches!(again, Err(QueueError::Status { .. })),
        "a rejected item: {again:?}"
    );
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    assert!(queue.list_tasks(None).expect("tasks").tasks.is_empty());
}

// ------------------------------------------------------------------ RD-3

/// RD-3 / step 3: made, the item `rejected` by the owner with the reason
/// at `now`; a `draft` task as `create_task` stores one (the next ID, the
/// given repository, title, goal, targets, a human's, `now`, revision 1,
/// no plan, snapshot or claim); `proposal.rejected` `{"id","reason",
/// "task"}` (its payload as stored), then `task.created` `{"id"}`, both at
/// `now`. M: `task` left out of the payload; `task.created` logged first;
/// the task stored `ready`.
#[test]
fn reject_with_task_rejects_and_makes_a_draft() {
    let scratch = Scratch::new("qfu-made");
    let (mut queue, db, id) = with_item(&scratch);
    let read = seen(&queue, &id);
    let before = queue.events().expect("events").len();
    let task = follow_up_task(&[NODE]);
    let made = queue.reject_with_task(&id, &read, &owner(), &task, T1);
    assert!(made.is_ok(), "{made:?}");

    let item = queue.get(&id).expect("get").expect("the item");
    assert_eq!(item.status, ProposalStatus::Rejected);
    assert_eq!(item.decided_by.as_deref(), Some(OWNER));
    assert_eq!(item.decided_at.as_deref(), Some(T1));
    assert_eq!(item.decision_note.as_deref(), Some(REASON));
    assert_eq!(item.updated_at, T1);

    let tasks = queue.list_tasks(None).expect("tasks");
    assert!(tasks.unreadable.is_empty(), "{:?}", tasks.unreadable);
    assert_eq!(tasks.tasks.len(), 1);
    let stored = &tasks.tasks[0];
    assert_eq!(stored.id, "T-0001");
    assert_eq!(stored.status, TaskStatus::Draft);
    assert_eq!(stored.project, PROJECT);
    assert_eq!(stored.git_common_dir, COMMON);
    assert_eq!(stored.title, task.title);
    assert_eq!(stored.goal, task.goal);
    assert_eq!(stored.targets, [NODE]);
    assert_eq!(stored.author, Author::human());
    assert_eq!(
        (stored.created_at.as_str(), stored.updated_at.as_str()),
        (T1, T1)
    );
    assert_eq!(stored.revision, 1);
    assert!(stored.plan.is_none() && stored.snapshot.is_none() && stored.claim.is_none());
    assert!(stored.runs.is_empty() && stored.owner_notes.is_empty());

    let events = queue.events().expect("events");
    let added: Vec<(&str, &serde_json::Value, &str)> = events[before..]
        .iter()
        .map(|event| (event.event_type.as_str(), &event.payload, event.at.as_str()))
        .collect();
    assert_eq!(added.len(), 2, "{added:?}");
    assert_eq!(
        (added[0].0, added[0].1, added[0].2),
        (
            EVENT_REJECTED,
            &json!({"id": id, "reason": REASON, "task": "T-0001"}),
            T1
        )
    );
    assert_eq!((added[1].0, added[1].2), (EVENT_TASK_CREATED, T1));
    assert_eq!(added[1].1["id"], json!("T-0001"), "{}", added[1].1);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT payload FROM events WHERE type = 'proposal.rejected' ORDER BY seq;"
        ),
        format!(r#"{{"id":"{id}","reason":"{REASON}","task":"T-0001"}}"#),
        "the payload as stored"
    );
}

/// RD-3 / Q1: the task ID is the queue's next (a task made before keeps
/// `T-0001`; the follow-up's is `T-0002`, named in the event); a plain
/// `reject_from` of another item logs `{"id","reason"}` with no `task`
/// key, byte for byte as 4c-1 stores it. M: `"task":null` on every
/// `proposal.rejected`.
#[test]
fn task_key_only_with_a_follow_up() {
    let scratch = Scratch::new("qfu-key");
    let (mut queue, db, id) = with_item(&scratch);
    queue
        .create_task(&follow_up_task(&["LAW-STEP"]), T0)
        .expect("an earlier task");
    let read = seen(&queue, &id);
    queue
        .reject_with_task(&id, &read, &owner(), &follow_up_task(&[NODE]), T1)
        .expect("made");
    assert!(
        queue
            .get_task("T-0002")
            .expect("get")
            .is_some_and(|task| task.status == TaskStatus::Draft && task.targets == [NODE]),
        "the follow-up's task is the next"
    );

    // Another item, a plain reject.
    let mut other = drift();
    other.target_id = "LAW-STEP".to_owned();
    other.hash = format!("b3:{}", "2".repeat(64));
    let plain = match queue
        .create_drift(&[other], T0)
        .expect("create_drift")
        .remove(0)
    {
        DriftResult::Created { proposal, .. } => proposal.id.clone(),
        result => panic!("expected a new item, got {result:?}"),
    };
    let read = seen(&queue, &plain);
    queue
        .reject_from(&plain, &read, &owner(), T1)
        .expect("a plain reject");
    let payloads = sqlite3(
        &db,
        "SELECT payload FROM events WHERE type = 'proposal.rejected' ORDER BY seq;",
    );
    assert_eq!(
        payloads.lines().collect::<Vec<_>>(),
        [
            format!(r#"{{"id":"{id}","reason":"{REASON}","task":"T-0002"}}"#),
            format!(r#"{{"id":"{plain}","reason":"{REASON}"}}"#),
        ]
    );
    assert_eq!(
        queue.list_tasks(None).expect("tasks").tasks.len(),
        2,
        "a plain reject makes no task"
    );
}

/// Iteration 1's deviation 8: the store refuses another kind itself (an
/// open question): `Invalid` ``…is a question: only a drift item is
/// rejected with a task``, nothing written (the CLI's exit 2 comes first).
/// M: the kind check dropped (a question's reject making a task).
#[test]
fn reject_with_task_takes_only_a_drift_item() {
    let scratch = Scratch::new("qfu-kind");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let question = NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/spec/counter.md".to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: vec![NODE.to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: "Does the cap count the last step?".to_owned(),
            working_answer: Some("It does.".to_owned()),
            price_of_other: Some("A new rule case.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
        },
    };
    let id = queue
        .create_intake(&question, &[], None, T0)
        .expect("create_intake")
        .created
        .expect("a question stored")
        .id;
    let read = seen(&queue, &id);
    let dump = queue.dump().expect("dump");
    match queue.reject_with_task(&id, &read, &owner(), &follow_up_task(&[NODE]), T1) {
        Err(QueueError::Invalid(message)) => assert_eq!(
            message,
            format!("`{id}` is a question: only a drift item is rejected with a task")
        ),
        other => panic!("a question: expected Invalid, got {other:?}"),
    }
    nothing_written(&queue, &id, &dump, "a question");
    assert!(queue.list_tasks(None).expect("tasks").tasks.is_empty());
}
