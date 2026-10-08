//! docs/features/plugin-gate.md, the store half ("Data", Library and
//! Event; canon `docs/canon/gate.md` "Verdict" step 6, "Event"): the
//! queue as the task gate reads it, through the public API only (the raw
//! steps -- a newer or older schema, a corrupt row, a held lock -- use the
//! system's `sqlite3`).
//!
//! - `open_for_gate`: no file is `None` and nothing is created (no file,
//!   no directory, no side file); an existing queue is opened read-only:
//!   its bytes, `user_version` and modification time kept, no schema step
//!   on an older queue (a queue without `tasks` holds none); a newer one is
//!   `SchemaTooNew`.
//! - `GateQueue::tasks`: the project's rows of the repository (compared as
//!   directories: a symlinked path is the same repository; another
//!   repository's rows and another project's are not read) by number, only
//!   `id`, `status`, `claim`; a corrupt row of the repository (`claim` not
//!   JSON, a `status` that is no state, `in_progress` without a claim, a
//!   NULL `git_common_dir`, an `id` that is no task ID) is named with its
//!   column, never skipped silently; another repository's corrupt row is
//!   not named.
//! - `record_gate_observed`: only into a queue at this build's schema
//!   (5): one `gate.observed` whose stored payload keys are `path, zone,
//!   worktree, branch` in this order (`null` for the config file's zone
//!   and a detached branch), `project` the slug; no database or another
//!   schema: `Ok(false)`, nothing created or stepped; a bad clock refused.
//! - The deadline: the queue under another process's exclusive lock gives
//!   `StoreError::Busy` at the deadline, not after an unbounded wait, for
//!   the read and the record; a deadline already past opens nothing.

#![cfg(unix)]

mod common;

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

use common::{Scratch, blake3_hex};
use specengine_core::proposal::Author;
use specengine_model::{SnapshotPlace, TaskClaim, TaskCriterion, TaskStatus};
use specengine_store::{
    EVENT_GATE_OBSERVED, GateObserved, NewTask, ProposalQueue as _, QUEUE_SCHEMA_VERSION,
    QueueError, SnapshotEntry, SqliteQueue, StoreError, TaskChange, TaskSnapshot, open_for_gate,
    record_gate_observed,
};

const T0: &str = "2026-10-08T09:00:00Z";
const PROJECT: &str = "lantern-keep";

fn sqlite3_client() -> &'static str {
    ["/usr/bin/sqlite3", "/bin/sqlite3", "/usr/local/bin/sqlite3"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("a sqlite3 client")
}

fn sqlite3(db: &Path, sql: &str) -> String {
    let output = Command::new(sqlite3_client())
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

fn later() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

/// Two repositories' common dirs, real directories (compared as such).
struct Repos {
    scratch: Scratch,
    db: PathBuf,
    one: PathBuf,
    other: PathBuf,
}

impl Repos {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let one = scratch.join("one/.git");
        let other = scratch.join("other/.git");
        fs::create_dir_all(&one).unwrap();
        fs::create_dir_all(&other).unwrap();
        let one = fs::canonicalize(one).unwrap();
        let other = fs::canonicalize(other).unwrap();
        let db = scratch.join("data/lantern-keep.db");
        fs::create_dir_all(db.parent().unwrap()).unwrap();
        Self {
            scratch,
            db,
            one,
            other,
        }
    }

    fn queue(&self, project: &str) -> SqliteQueue {
        SqliteQueue::open(&self.db, project).expect("open the queue")
    }
}

fn new_task(common: &Path) -> NewTask {
    NewTask {
        git_common_dir: common.to_str().unwrap().to_owned(),
        title: Some("A unique title 7f3a".to_owned()),
        goal: Some("A unique goal 9c1e".to_owned()),
        targets: vec!["MEC-LANTERN".to_owned()],
        author: Author::human(),
    }
}

fn claim_of(worktree: &str, branch: &str) -> TaskClaim {
    TaskClaim {
        at: T0.to_owned(),
        role: "implementer".to_owned(),
        worktree: worktree.to_owned(),
        branch: branch.to_owned(),
    }
}

fn change(queue: &mut SqliteQueue, id: &str, change: &TaskChange) {
    let seen = queue.get_task(id).expect("get").expect("a task").seen();
    queue
        .change_task(id, &seen, change, T0)
        .unwrap_or_else(|error| panic!("{id}: {error}"));
}

/// A task of `common` moved to `status` through the real transitions
/// (`ready` approved with a snapshot; `in_progress` claimed in `worktree`
/// on `branch`).
fn task_in(
    queue: &mut SqliteQueue,
    common: &Path,
    status: TaskStatus,
    worktree: &str,
    branch: &str,
) -> String {
    let id = queue.create_task(&new_task(common), T0).expect("create").id;
    if status == TaskStatus::Draft {
        return id;
    }
    change(
        queue,
        &id,
        &TaskChange::Plan {
            plan: "1. Do.".to_owned(),
            criteria: vec![TaskCriterion {
                reference: None,
                text: Some("Holds.".to_owned()),
            }],
            affected_nodes: Vec::new(),
        },
    );
    if status == TaskStatus::Review {
        return id;
    }
    if status == TaskStatus::Cancelled {
        change(queue, &id, &TaskChange::Cancel);
        return id;
    }
    change(
        queue,
        &id,
        &TaskChange::Approve {
            snapshot: TaskSnapshot {
                at: T0.to_owned(),
                by: "Owner <owner@example.invalid>".to_owned(),
                place: SnapshotPlace {
                    worktree: worktree.to_owned(),
                    root_rel: String::new(),
                    branch: branch.to_owned(),
                    commit: "1".repeat(40),
                },
                nodes: vec![SnapshotEntry {
                    id: "MEC-LANTERN".to_owned(),
                    path: "docs/x.md".to_owned(),
                    span_hash: format!("b3:{}", blake3_hex(b"x")),
                    text: "text".to_owned(),
                }],
            },
        },
    );
    if status == TaskStatus::Ready {
        return id;
    }
    change(
        queue,
        &id,
        &TaskChange::Claim {
            claim: claim_of(worktree, branch),
            author: Author::new(Some("implementer".to_owned()), None, None).expect("a role"),
        },
    );
    if status == TaskStatus::InProgress {
        return id;
    }
    assert_eq!(status, TaskStatus::Done, "a status this helper makes");
    change(
        queue,
        &id,
        &TaskChange::Report {
            outcome: specengine_model::RunOutcome::Completed,
            summary: "Done.".to_owned(),
            changed_files: vec!["src/a.rs".to_owned()],
        },
    );
    change(queue, &id, &TaskChange::Complete);
    id
}

/// Every file under `dir`, with its bytes.
fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap_or_default();
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                bytes,
            )
        })
        .collect();
    out.sort();
    out
}

// --------------------------------------------------------------- the open

#[test]
fn no_database_is_none_and_nothing_is_created() {
    let repos = Repos::new("qg-none");
    let data = repos.db.parent().unwrap();
    assert!(
        open_for_gate(&repos.db, PROJECT, later())
            .unwrap()
            .is_none()
    );
    assert_eq!(files(data), Vec::new(), "no file, no side file");
    // No data directory at all: still none, still nothing made.
    let missing = repos.scratch.join("absent/dir/lantern-keep.db");
    assert!(open_for_gate(&missing, PROJECT, later()).unwrap().is_none());
    assert!(!repos.scratch.join("absent").exists());
    // The record makes nothing either.
    let observed = GateObserved {
        path: "src/a.rs".to_owned(),
        zone: Some("code".to_owned()),
        worktree: "/w".to_owned(),
        branch: Some("main".to_owned()),
    };
    assert!(!record_gate_observed(&repos.db, PROJECT, &observed, T0, later()).unwrap());
    assert_eq!(files(data), Vec::new());
    assert!(!missing.exists());
}

#[test]
fn the_tasks_of_the_repository_by_number_only_id_status_claim() {
    let repos = Repos::new("qg-tasks");
    let one = repos.one.to_str().unwrap().to_owned();
    {
        let mut queue = repos.queue(PROJECT);
        for status in [
            TaskStatus::Draft,
            TaskStatus::Review,
            TaskStatus::Ready,
            TaskStatus::InProgress,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ] {
            task_in(&mut queue, &repos.one, status, "/w1", "main");
        }
        // Another repository's task and another project's.
        task_in(
            &mut queue,
            &repos.other,
            TaskStatus::InProgress,
            "/w9",
            "main",
        );
        let mut foreign = repos.queue("another-project");
        task_in(
            &mut foreign,
            &repos.one,
            TaskStatus::InProgress,
            "/w1",
            "main",
        );
    }
    let queue = open_for_gate(&repos.db, PROJECT, later())
        .unwrap()
        .expect("the queue");
    let (tasks, corrupt) = queue.tasks(&repos.one).unwrap();
    assert!(corrupt.is_empty(), "{corrupt:?}");
    let seen: Vec<(&str, TaskStatus)> = tasks
        .iter()
        .map(|task| (task.id.as_str(), task.status))
        .collect();
    assert_eq!(
        seen,
        [
            ("T-0001", TaskStatus::Draft),
            ("T-0002", TaskStatus::Review),
            ("T-0003", TaskStatus::Ready),
            ("T-0004", TaskStatus::InProgress),
            ("T-0005", TaskStatus::Done),
            ("T-0006", TaskStatus::Cancelled),
        ]
    );
    assert_eq!(tasks[3].claim, Some(claim_of("/w1", "main")));
    assert_eq!(tasks[2].claim, None);
    // The repository compared as a directory: through a symlink, the same.
    let link = repos.scratch.join("one-link");
    symlink(repos.one.parent().unwrap(), &link).unwrap();
    let (through, _) = queue.tasks(&link.join(".git")).unwrap();
    assert_eq!(through, tasks);
    // The other repository sees only its own.
    let (others, _) = queue.tasks(&repos.other).unwrap();
    let ids: Vec<&str> = others.iter().map(|task| task.id.as_str()).collect();
    assert_eq!(ids, ["T-0007"]);
    // A repository with no task.
    let none = repos.scratch.join("none/.git");
    fs::create_dir_all(&none).unwrap();
    assert_eq!(queue.tasks(&none).unwrap(), (Vec::new(), Vec::new()));
    let _ = one;
}

#[test]
fn the_read_changes_nothing_on_disk() {
    let repos = Repos::new("qg-readonly");
    {
        let mut queue = repos.queue(PROJECT);
        task_in(
            &mut queue,
            &repos.one,
            TaskStatus::InProgress,
            "/w1",
            "main",
        );
    }
    // A checkpointed, closed queue: only the database file is left.
    sqlite3(&repos.db, "PRAGMA wal_checkpoint(TRUNCATE);");
    let data = repos.db.parent().unwrap();
    let before_bytes = fs::read(&repos.db).unwrap();
    let before_mtime = fs::metadata(&repos.db).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    for _ in 0..5 {
        let queue = open_for_gate(&repos.db, PROJECT, later())
            .unwrap()
            .expect("the queue");
        let (tasks, _) = queue.tasks(&repos.one).unwrap();
        assert_eq!(tasks.len(), 1);
    }
    assert_eq!(fs::read(&repos.db).unwrap(), before_bytes, "the bytes kept");
    assert_eq!(
        fs::metadata(&repos.db).unwrap().modified().unwrap(),
        before_mtime,
        "the modification time kept"
    );
    for (name, bytes) in files(data) {
        if name.ends_with("-wal") {
            assert!(bytes.is_empty(), "nothing written to the WAL");
        }
    }
    assert_eq!(
        sqlite3(&repos.db, "PRAGMA user_version;"),
        QUEUE_SCHEMA_VERSION.to_string()
    );
}

#[test]
fn an_older_queue_is_not_stepped_and_one_without_tasks_holds_none() {
    let repos = Repos::new("qg-older");
    // A queue of schema 3: no `tasks` table.
    sqlite3(
        &repos.db,
        "CREATE TABLE events (seq INTEGER PRIMARY KEY, project TEXT, type TEXT, payload TEXT, at TEXT); PRAGMA user_version = 3;",
    );
    let before = fs::read(&repos.db).unwrap();
    let queue = open_for_gate(&repos.db, PROJECT, later())
        .unwrap()
        .expect("the queue");
    assert_eq!(queue.tasks(&repos.one).unwrap(), (Vec::new(), Vec::new()));
    drop(queue);
    assert_eq!(sqlite3(&repos.db, "PRAGMA user_version;"), "3", "no step");
    assert_eq!(fs::read(&repos.db).unwrap(), before);
    // Nor recorded into: another schema records nothing, steps nothing.
    let observed = GateObserved {
        path: "src/a.rs".to_owned(),
        zone: Some("code".to_owned()),
        worktree: "/w".to_owned(),
        branch: Some("main".to_owned()),
    };
    assert!(!record_gate_observed(&repos.db, PROJECT, &observed, T0, later()).unwrap());
    assert_eq!(sqlite3(&repos.db, "SELECT count(*) FROM events;"), "0");
    assert_eq!(sqlite3(&repos.db, "PRAGMA user_version;"), "3");
}

#[test]
fn a_newer_queue_is_schema_too_new() {
    let repos = Repos::new("qg-newer");
    {
        let mut queue = repos.queue(PROJECT);
        task_in(
            &mut queue,
            &repos.one,
            TaskStatus::InProgress,
            "/w1",
            "main",
        );
    }
    let newer = QUEUE_SCHEMA_VERSION + 1;
    sqlite3(&repos.db, &format!("PRAGMA user_version = {newer};"));
    let queue = open_for_gate(&repos.db, PROJECT, later())
        .unwrap()
        .expect("opened");
    match queue.tasks(&repos.one) {
        Err(QueueError::SchemaTooNew { found }) => assert_eq!(found, newer),
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
    drop(queue);
    assert_eq!(
        sqlite3(&repos.db, "PRAGMA user_version;"),
        newer.to_string(),
        "left as it was"
    );
    // The record: nothing into a newer queue.
    let observed = GateObserved {
        path: "src/a.rs".to_owned(),
        zone: None,
        worktree: "/w".to_owned(),
        branch: None,
    };
    assert!(!record_gate_observed(&repos.db, PROJECT, &observed, T0, later()).unwrap());
    assert_eq!(
        sqlite3(
            &repos.db,
            &format!("SELECT count(*) FROM events WHERE type = '{EVENT_GATE_OBSERVED}';")
        ),
        "0"
    );
}

// ------------------------------------------------------------ corrupt rows

#[test]
fn a_corrupt_row_of_the_repository_is_named_with_its_column() {
    for (label, sql, column) in [
        (
            "claim not JSON",
            "UPDATE tasks SET claim = '{' WHERE id = 'T-0002'",
            "claim",
        ),
        (
            "a claim without a branch",
            "UPDATE tasks SET claim = '{\"at\":\"x\",\"role\":\"r\",\"worktree\":\"/w1\"}' WHERE id = 'T-0002'",
            "claim",
        ),
        (
            "a status that is no state",
            "UPDATE tasks SET status = 'bogus' WHERE id = 'T-0002'",
            "status",
        ),
        (
            "a NULL status",
            "UPDATE tasks SET status = NULL WHERE id = 'T-0002'",
            "status",
        ),
        (
            "in_progress without a claim",
            "UPDATE tasks SET claim = NULL WHERE id = 'T-0002'",
            "claim",
        ),
        (
            "an id that is no task ID",
            "UPDATE tasks SET id = 'X-0002' WHERE id = 'T-0002'",
            "id",
        ),
    ] {
        let repos = Repos::new("qg-corrupt");
        {
            let mut queue = repos.queue(PROJECT);
            task_in(&mut queue, &repos.one, TaskStatus::Ready, "/w1", "main");
            task_in(
                &mut queue,
                &repos.one,
                TaskStatus::InProgress,
                "/w1",
                "main",
            );
        }
        sqlite3(&repos.db, sql);
        let queue = open_for_gate(&repos.db, PROJECT, later())
            .unwrap()
            .expect("the queue");
        let (tasks, corrupt) = queue.tasks(&repos.one).unwrap();
        assert_eq!(corrupt.len(), 1, "{label}: {corrupt:?}");
        assert_eq!(corrupt[0].column, column, "{label}: {corrupt:?}");
        let named = corrupt[0].to_string();
        assert!(named.contains(&format!("`{column}`")), "{label}: {named}");
        // The readable row is still read.
        let ids: Vec<&str> = tasks.iter().map(|task| task.id.as_str()).collect();
        assert_eq!(ids, ["T-0001"], "{label}");
    }
}

#[test]
fn a_null_repository_is_named_and_another_repositorys_corrupt_row_is_not() {
    let repos = Repos::new("qg-corrupt-repo");
    {
        let mut queue = repos.queue(PROJECT);
        task_in(&mut queue, &repos.one, TaskStatus::Ready, "/w1", "main");
        task_in(
            &mut queue,
            &repos.other,
            TaskStatus::InProgress,
            "/w9",
            "main",
        );
    }
    sqlite3(
        &repos.db,
        "UPDATE tasks SET claim = '{' WHERE id = 'T-0002'",
    );
    let queue = open_for_gate(&repos.db, PROJECT, later())
        .unwrap()
        .expect("the queue");
    let (tasks, corrupt) = queue.tasks(&repos.one).unwrap();
    assert!(corrupt.is_empty(), "another repository's row: {corrupt:?}");
    assert_eq!(tasks.len(), 1);
    drop(queue);
    sqlite3(
        &repos.db,
        "UPDATE tasks SET git_common_dir = NULL WHERE id = 'T-0002'",
    );
    let queue = open_for_gate(&repos.db, PROJECT, later())
        .unwrap()
        .expect("the queue");
    let (_, corrupt) = queue.tasks(&repos.one).unwrap();
    assert_eq!(corrupt.len(), 1, "{corrupt:?}");
    assert_eq!(corrupt[0].id, "T-0002");
    assert_eq!(corrupt[0].column, "git_common_dir");
}

// ------------------------------------------------------------- the record

#[test]
fn the_record_is_one_event_with_its_four_keys_in_order() {
    let repos = Repos::new("qg-record");
    let before = {
        let queue = repos.queue(PROJECT);
        queue.events().unwrap().len()
    };
    for observed in [
        GateObserved {
            path: "src/a.rs".to_owned(),
            zone: Some("code".to_owned()),
            worktree: "/r/w1".to_owned(),
            branch: Some("main".to_owned()),
        },
        GateObserved {
            path: "specengine.toml".to_owned(),
            zone: None,
            worktree: "/r/w1".to_owned(),
            branch: None,
        },
    ] {
        assert!(record_gate_observed(&repos.db, PROJECT, &observed, T0, later()).unwrap());
    }
    let stored = sqlite3(
        &repos.db,
        &format!(
            "SELECT project || ' ' || payload || ' ' || at FROM events WHERE type = '{EVENT_GATE_OBSERVED}' ORDER BY seq;"
        ),
    );
    assert_eq!(
        stored.lines().collect::<Vec<_>>(),
        [
            "lantern-keep {\"path\":\"src/a.rs\",\"zone\":\"code\",\"worktree\":\"/r/w1\",\"branch\":\"main\"} 2026-10-08T09:00:00Z",
            "lantern-keep {\"path\":\"specengine.toml\",\"zone\":null,\"worktree\":\"/r/w1\",\"branch\":null} 2026-10-08T09:00:00Z",
        ]
    );
    let queue = repos.queue(PROJECT);
    let events = queue.events().unwrap();
    assert_eq!(events.len(), before + 2, "two events, nothing else");
    assert_eq!(EVENT_GATE_OBSERVED, "gate.observed");
    // The schema is the build's: the record stepped nothing.
    assert_eq!(
        sqlite3(&repos.db, "PRAGMA user_version;"),
        QUEUE_SCHEMA_VERSION.to_string()
    );
    // A bad clock is refused, nothing stored.
    let observed = GateObserved {
        path: "src/b.rs".to_owned(),
        zone: Some("code".to_owned()),
        worktree: "/r/w1".to_owned(),
        branch: Some("main".to_owned()),
    };
    assert!(record_gate_observed(&repos.db, PROJECT, &observed, "now", later()).is_err());
    assert_eq!(queue.events().unwrap().len(), before + 2);
}

#[test]
fn the_record_goes_only_into_schema_six() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 6, "this build's queue schema");
    let repos = Repos::new("qg-record-4");
    drop(repos.queue(PROJECT));
    // A queue at schema 5 (as a build before kind-interpretation left it).
    sqlite3(&repos.db, "PRAGMA user_version = 5;");
    let observed = GateObserved {
        path: "src/a.rs".to_owned(),
        zone: Some("code".to_owned()),
        worktree: "/w".to_owned(),
        branch: Some("main".to_owned()),
    };
    assert!(!record_gate_observed(&repos.db, PROJECT, &observed, T0, later()).unwrap());
    assert_eq!(sqlite3(&repos.db, "SELECT count(*) FROM events;"), "0");
    assert_eq!(sqlite3(&repos.db, "PRAGMA user_version;"), "5");
}

// ------------------------------------------------------------- the deadline

/// Another process holding the queue: `sqlite3` with its lock taken
/// (`exclusive`: `locking_mode=EXCLUSIVE` and a write, so even a reader
/// waits; else `BEGIN IMMEDIATE`, writers only), released and reaped on
/// drop.
struct Holder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl Holder {
    fn hold(db: &Path, exclusive: bool) -> Self {
        let mut child = Command::new(sqlite3_client())
            .arg(db)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sqlite3 runs");
        let mut stdin = child.stdin.take().expect("stdin");
        // `.timeout`: the holder waits out the probe below instead of
        // losing its `BEGIN` to it.
        let lock: &[u8] = if exclusive {
            b".timeout 5000\nPRAGMA locking_mode=EXCLUSIVE;\nBEGIN EXCLUSIVE;\nINSERT INTO events (project, type, payload, at) VALUES ('x', 'x', '{}', 'x');\n"
        } else {
            b".timeout 5000\nBEGIN IMMEDIATE;\nINSERT INTO events (project, type, payload, at) VALUES ('x', 'x', '{}', 'x');\n"
        };
        stdin.write_all(lock).expect("lock");
        stdin.flush().expect("flush");
        let holder = Self {
            child,
            stdin: Some(stdin),
        };
        // Held: a writer with no busy wait is refused (bounded: 5 s).
        let started = Instant::now();
        loop {
            let probe = Command::new(sqlite3_client())
                .args(["-cmd", ".timeout 0"])
                .arg(db)
                .arg("BEGIN IMMEDIATE; ROLLBACK;")
                .stdin(Stdio::null())
                .output()
                .expect("probe");
            if !probe.status.success() {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "the lock was never taken"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        holder
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"ROLLBACK;\n.quit\n");
        }
        let started = Instant::now();
        loop {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            if started.elapsed() > Duration::from_secs(5) {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn is_busy(error: &QueueError) -> bool {
    matches!(error, QueueError::Store(StoreError::Busy))
}

#[test]
fn a_locked_queue_is_busy_at_the_deadline_for_the_read_and_the_record() {
    let repos = Repos::new("qg-busy");
    {
        let mut queue = repos.queue(PROJECT);
        task_in(
            &mut queue,
            &repos.one,
            TaskStatus::InProgress,
            "/w1",
            "main",
        );
    }
    let observed = GateObserved {
        path: "src/a.rs".to_owned(),
        zone: Some("code".to_owned()),
        worktree: "/w1".to_owned(),
        branch: Some("main".to_owned()),
    };
    {
        let _held = Holder::hold(&repos.db, true);
        let started = Instant::now();
        let deadline = started + Duration::from_millis(600);
        let read = open_for_gate(&repos.db, PROJECT, deadline)
            .and_then(|queue| queue.expect("the queue").tasks(&repos.one));
        let took = started.elapsed();
        match read {
            Err(error) => assert!(is_busy(&error), "{error:?}"),
            Ok(read) => panic!("read under an exclusive lock: {read:?}"),
        }
        assert!(
            took >= Duration::from_millis(500),
            "gave up early: {took:?}"
        );
        assert!(
            took < Duration::from_millis(1500),
            "unbounded wait: {took:?}"
        );

        let started = Instant::now();
        let recorded = record_gate_observed(
            &repos.db,
            PROJECT,
            &observed,
            T0,
            started + Duration::from_millis(600),
        );
        let took = started.elapsed();
        match recorded {
            Err(error) => assert!(is_busy(&error), "{error:?}"),
            Ok(stored) => panic!("recorded under a lock: {stored}"),
        }
        assert!(
            took < Duration::from_millis(1500),
            "unbounded wait: {took:?}"
        );
    }
    // A writer's lock alone (`BEGIN IMMEDIATE`): the read goes on, the
    // record waits until the deadline.
    {
        let _held = Holder::hold(&repos.db, false);
        let queue = open_for_gate(&repos.db, PROJECT, later())
            .unwrap()
            .expect("the queue");
        assert_eq!(queue.tasks(&repos.one).unwrap().0.len(), 1);
        drop(queue);
        let started = Instant::now();
        let recorded = record_gate_observed(
            &repos.db,
            PROJECT,
            &observed,
            T0,
            started + Duration::from_millis(400),
        );
        let took = started.elapsed();
        assert!(
            matches!(&recorded, Err(error) if is_busy(error)),
            "{recorded:?}"
        );
        assert!(took >= Duration::from_millis(300), "{took:?}");
        assert!(took < Duration::from_millis(1300), "{took:?}");
    }
    // Released: the record goes in.
    assert!(record_gate_observed(&repos.db, PROJECT, &observed, T0, later()).unwrap());
    // A deadline already past: busy before any wait.
    let past = Instant::now();
    match open_for_gate(&repos.db, PROJECT, past) {
        Err(QueueError::Store(StoreError::Busy)) => {}
        other => panic!(
            "expected Busy, got {:?}",
            other.map(|queue| queue.is_some())
        ),
    }
    assert!(matches!(
        record_gate_observed(&repos.db, PROJECT, &observed, T0, past),
        Err(QueueError::Store(StoreError::Busy))
    ));
}
