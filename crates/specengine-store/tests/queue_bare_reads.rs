//! docs/features/health-code.md AC-7 and "Risks" (Fresh data directory),
//! the store half: Health's row 4 opens the queue with
//! `SqliteQueue::open_existing` on the slug's DB its own refresh just made,
//! an index DB without a queue table, and runs no schema step. There `get`
//! by an exact ID and `list_readable` over every repository
//! (`git_common_dir: None`) read an empty queue, `Ok`, and leave the DB
//! without a queue table, `user_version` 0; a newer `user_version` is still
//! refused (`SchemaTooNew`, the queue's cause of row 4 `cannot`).
//! `stored_rows` and `counts` there are `queue_state.rs`' already. The spec
//! ("Description and interactions", Crates) allows a fix inside `get` or
//! `list_readable` only if one errors without queue tables: this test
//! tells. Through the public API; the raw steps use the system's `sqlite3`.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use specengine_store::{ProposalFilter, ProposalQueue as _, QueueError, SqliteQueue};

const PROJECT: &str = "demo";

/// `sqlite3 <db> <sql>`'s stdout; it must succeed.
fn sqlite3_out(db: &Path, sql: &str) -> String {
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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// AC-7, the store's reads: an index DB (no queue table) opened with
/// `open_existing` answers `get("PR-0001")`, `get("PR-7")` with `None` and
/// `list_readable` (every repository, every state) with nothing, no
/// unreadable row; no queue table and `user_version` 0 afterwards. A
/// `user_version` above this build's: `SchemaTooNew`. M: `get` or
/// `list_readable` erroring without the queue's tables.
#[test]
fn get_and_list_readable_read_an_index_db_as_an_empty_queue() {
    let scratch = Scratch::new("qb-bare");
    let db = scratch.db("index-only");
    let root = scratch.join("root");
    std::fs::create_dir_all(&root).expect("root");
    drop(common::open(&db, &root));
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file exists");
    assert_eq!(queue.get("PR-0001").expect("get"), None);
    assert_eq!(queue.get("PR-7").expect("get"), None);
    let list = queue
        .list_readable(&ProposalFilter::default())
        .expect("list_readable");
    assert!(
        list.proposals.is_empty() && list.unreadable.is_empty(),
        "{list:?}"
    );
    drop(queue);
    assert_eq!(
        sqlite3_out(
            &db,
            "SELECT name FROM sqlite_master WHERE name IN ('proposals', 'tasks', 'runs', \
             'events');"
        ),
        "",
        "reading made no queue table"
    );
    assert_eq!(sqlite3_out(&db, "PRAGMA user_version;"), "0\n");

    sqlite3_out(&db, "PRAGMA user_version = 99;");
    match SqliteQueue::open_existing(&db, PROJECT) {
        Err(QueueError::SchemaTooNew { found }) => assert_eq!(found, 99),
        other => panic!("a newer queue is refused: {other:?}"),
    }
}

/// Iteration 2's target G1 (review 1's first minor, owner pre-approved):
/// the empty read is an index-only DB's alone (`user_version` 0). A DB
/// whose queue steps ran (`user_version` ≥ 1) and then lost `proposals`
/// still errors on `get` and `list_readable`, as it did before AC-7's fix.
/// Red on iteration 1 by design (`get` → `None`, `list_readable` empty).
#[test]
fn a_schema_run_db_without_proposals_still_errors() {
    let scratch = Scratch::new("qb-lost");
    let db = scratch.db("lost");
    drop(SqliteQueue::open(&db, PROJECT).expect("the queue's steps run"));
    sqlite3_out(&db, "DROP TABLE proposals;");
    assert_ne!(
        sqlite3_out(&db, "PRAGMA user_version;"),
        "0\n",
        "the queue's steps ran"
    );
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file exists");
    assert!(
        queue.get("PR-0001").is_err(),
        "`get` without `proposals` after the steps ran: an error"
    );
    assert!(
        queue.list_readable(&ProposalFilter::default()).is_err(),
        "`list_readable` without `proposals` after the steps ran: an error"
    );
}
