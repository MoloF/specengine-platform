//! docs/features/comments.md (RM-0136, ADR-0087) "Acceptance criteria", the
//! store's half of CM1: `SqliteQueue::open` through the public API, the
//! older schema and the hand-made tables through the system's `sqlite3`.
//!
//! - CM1: `QUEUE_SCHEMA_VERSION` 15; a new queue stands at 15 with
//!   `comments` (15 columns) and `comment_versions` (5) as "Queue schema 15"
//!   writes them, `PRAGMA table_info` pinned column by column (name, type,
//!   NOT NULL, key), both STRICT, the index `comments_on`; `COMMENT_COLUMNS`,
//!   `COMMENT_VERSION_COLUMNS` and `PROPOSAL_COLUMNS` (51, `comment_id`
//!   last; `proposal_columns(14)` the first 50) equal `PRAGMA table_info`;
//!   the index `proposals_comment` on `(project, comment_id)`. A
//!   schema-14 queue holding proposals, cards, a task, a run and events steps
//!   to 15 at its first open, every row as it was, both tables there and
//!   empty, every `comment_id` NULL; a second open steps nothing. A failure
//!   inside the step (a hand-made `comment_versions`: the step's second
//!   table) leaves the queue at 14 with no `comments` table either: one
//!   transaction; a schema-13 queue failing at step 15 stays at 13 without
//!   step 14's ledger: every due step in one transaction. Schema 16 is
//!   `SchemaTooNew`, the file left alone. M: a column dropped from `STEP_15`.
//!
//! The rows are written raw: the step reads no row, and the test needs no
//! struct of this build to hold them. Every expected value is worked out
//! from the spec's DDL, never read back from a run.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use specengine_store::{
    COMMENT_COLUMNS, COMMENT_VERSION_COLUMNS, PROPOSAL_COLUMNS, QUEUE_SCHEMA_VERSION, QueueError,
    SqliteQueue, proposal_columns,
};

const PROJECT: &str = "lantern-keep";

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

fn user_version(db: &Path) -> String {
    sqlite3(db, "PRAGMA user_version;")
}

/// `cid name type notnull pk` of every column of `table`, one per line.
fn table_info(db: &Path, table: &str) -> String {
    sqlite3(
        db,
        &format!(
            "SELECT cid || ' ' || name || ' ' || type || ' ' || \"notnull\" || ' ' || pk \
             FROM pragma_table_info('{table}') ORDER BY cid;"
        ),
    )
}

fn column_names(db: &Path, table: &str) -> Vec<String> {
    sqlite3(
        db,
        &format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid;"),
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `(table, columns)` of the index `name`, its columns one per line.
fn index_of(db: &Path, name: &str) -> (String, String) {
    (
        sqlite3(
            db,
            &format!(
                "SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = '{name}';"
            ),
        ),
        sqlite3(
            db,
            &format!("SELECT name FROM pragma_index_info('{name}') ORDER BY seqno;"),
        ),
    )
}

fn tables(db: &Path) -> String {
    sqlite3(
        db,
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name;",
    )
}

/// "Queue schema 15"'s DDL as `PRAGMA table_info` reads it: a STRICT table's
/// `PRIMARY KEY` column reads NOT NULL.
const COMMENTS_INFO: &str = "0 id TEXT 1 1\n1 project TEXT 1 0\n2 on_kind TEXT 1 0\n\
3 on_id TEXT 1 0\n4 on_rev INTEGER 0 0\n5 reply_to TEXT 0 0\n6 actor TEXT 1 0\n\
7 author TEXT 1 0\n8 version INTEGER 1 0\n9 body TEXT 0 0\n10 mentions TEXT 0 0\n\
11 created_at TEXT 1 0\n12 updated_at TEXT 1 0\n13 deleted_by TEXT 0 0\n\
14 deleted_at TEXT 0 0";

const COMMENT_VERSIONS_INFO: &str = "0 comment_id TEXT 1 1\n1 project TEXT 1 0\n\
2 version INTEGER 1 2\n3 body TEXT 0 0\n4 written_at TEXT 1 0";

/// CM1: 15 (16 since docs/features/work-references.md RF1); both tables as
/// the DDL writes them, STRICT, indexed; the three
/// column lists equal `PRAGMA table_info`. M: a column dropped from
/// `STEP_15`.
#[test]
fn cm1_a_new_queue_stands_at_15_with_both_tables_pinned() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 17);
    let scratch = Scratch::new("cm-cm1-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "17");

    assert_eq!(table_info(&db, "comments"), COMMENTS_INFO);
    assert_eq!(table_info(&db, "comment_versions"), COMMENT_VERSIONS_INFO);
    for table in ["comments", "comment_versions"] {
        assert_eq!(
            sqlite3(
                &db,
                &format!("SELECT strict FROM pragma_table_list WHERE name = '{table}';")
            ),
            "1",
            "{table} is STRICT"
        );
    }
    assert_eq!(
        sqlite3(
            &db,
            "SELECT name FROM pragma_index_info('comments_on') ORDER BY seqno;"
        ),
        "project\non_kind\non_id",
        "the index comments_on"
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = 'comments_on';"
        ),
        "comments"
    );

    // The review's index (docs/features/comments.md, review iteration): a
    // comment's items found by `comment_id`.
    assert_eq!(
        index_of(&db, "proposals_comment"),
        ("proposals".to_owned(), "project\ncomment_id".to_owned())
    );
    assert_eq!(COMMENT_COLUMNS.len(), 15);
    assert_eq!(COMMENT_VERSION_COLUMNS.len(), 5);
    assert_eq!(column_names(&db, "comments"), COMMENT_COLUMNS);
    assert_eq!(
        column_names(&db, "comment_versions"),
        COMMENT_VERSION_COLUMNS
    );

    assert_eq!(PROPOSAL_COLUMNS.len(), 52);
    // `comment_id` last but one since docs/features/reference-promotion.md
    // PR11 (`ref_id` last).
    assert_eq!(PROPOSAL_COLUMNS[50], "comment_id");
    assert_eq!(PROPOSAL_COLUMNS.last(), Some(&"ref_id"));
    assert_eq!(column_names(&db, "proposals"), PROPOSAL_COLUMNS);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT type || ' ' || \"notnull\" FROM pragma_table_info('proposals') \
             WHERE name = 'comment_id';"
        ),
        "TEXT 0"
    );
    assert_eq!(proposal_columns(15), Some(&PROPOSAL_COLUMNS[..51]));
    assert_eq!(proposal_columns(14), Some(&PROPOSAL_COLUMNS[..50]));
    assert_eq!(proposal_columns(13), Some(&PROPOSAL_COLUMNS[..50]));
    assert_eq!(proposal_columns(16), Some(&PROPOSAL_COLUMNS[..51]));
    // 52 since docs/features/reference-promotion.md: step 17 appends `ref_id`.
    assert_eq!(proposal_columns(17), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(18), None);
}

/// Rows of every queue table as this build's schema holds them, written raw
/// so no struct of the build is needed: two proposals (a question raised
/// for a card, an update), a card, a task, a run and three events.
fn some_rows(db: &Path) {
    sqlite3(
        db,
        "INSERT INTO proposals (id, project, kind, status, target_id, target_path, author, \
         created_at, updated_at, summary, card_id) VALUES ('PR-0001', 'lantern-keep', \
         'question', 'open', 'MEC-SPRINT', 'docs/spec/movement/sprint.md', \
         '{\"type\":\"agent\",\"role\":\"developer\",\"model\":null,\"run\":null}', \
         '2026-11-22T09:00:00Z', '2026-11-22T09:00:00Z', 'Does a sprint end at zero?', 'C-0001'); \
         INSERT INTO proposals (id, project, kind, status, target_id, base_text, new_text, \
         created_at, updated_at) VALUES ('PR-0002', 'lantern-keep', 'update', 'rejected', \
         'MEC-STAMINA', 'old', 'new', '2026-11-22T09:01:00Z', '2026-11-22T09:02:00Z'); \
         INSERT INTO cards (id, project, flow, col, position, title, targets, created_by, \
         created_at, updated_at, revision, tasks) VALUES ('C-0001', 'lantern-keep', 'plain', \
         'backlog', 1, 'Write the sprint foreword', '[\"MEC-SPRINT\"]', 'local', \
         '2026-11-22T08:59:00Z', '2026-11-22T08:59:00Z', '2', '[\"T-0001\"]'); \
         INSERT INTO tasks (id, project, status, title, targets, created_at, updated_at, \
         revision) VALUES ('T-0001', 'lantern-keep', 'draft', 'Tune the sprint', \
         '[\"MEC-SPRINT\"]', '2026-11-22T08:58:00Z', '2026-11-22T08:58:00Z', '1'); \
         INSERT INTO runs (task_id, run, role, outcome) VALUES ('T-0001', 1, 'developer', \
         'done'); \
         INSERT INTO events (seq, project, type, payload, at) VALUES \
         (1, 'lantern-keep', 'card.created', '{\"id\":\"C-0001\"}', '2026-11-22T08:59:00Z'), \
         (2, 'lantern-keep', 'proposal.created', '{\"id\":\"PR-0001\"}', \
         '2026-11-22T09:00:00Z'), \
         (3, 'lantern-keep', 'proposal.rejected', '{\"id\":\"PR-0002\"}', \
         '2026-11-22T09:02:00Z');",
    );
}

/// A schema-14 queue holding [`some_rows`]: this build's tables with step
/// 15's undone.
fn schema_14(db: &Path) {
    drop(SqliteQueue::open(db, PROJECT).expect("open"));
    some_rows(db);
    sqlite3(
        db,
        "DROP TABLE refs; DROP INDEX IF EXISTS proposals_ref; ALTER TABLE proposals DROP COLUMN ref_id; DROP TABLE comment_versions; DROP TABLE comments; \
         DROP INDEX IF EXISTS proposals_comment; ALTER TABLE proposals DROP COLUMN comment_id; PRAGMA user_version = 14;",
    );
    assert_eq!(
        column_names(db, "proposals").len(),
        50,
        "premise: schema 14"
    );
}

/// Every row of the five queue tables, the proposals by their first 50
/// columns, in a stable order.
fn rows_of_14(db: &Path) -> String {
    let columns = PROPOSAL_COLUMNS[..50].join(", ");
    sqlite3(
        db,
        &format!(
            "SELECT {columns} FROM proposals ORDER BY id; SELECT * FROM cards ORDER BY id; \
             SELECT * FROM tasks ORDER BY id; SELECT * FROM runs ORDER BY task_id, run; \
             SELECT * FROM events ORDER BY seq;"
        ),
    )
}

/// CM1: a schema-14 queue steps at its first open: every row as it was, the
/// two tables there and empty, every `comment_id` NULL; a second open steps
/// nothing.
#[test]
fn cm1_a_schema_14_queue_steps_keeping_every_row() {
    let scratch = Scratch::new("cm-cm1-step");
    let db = scratch.db("q");
    schema_14(&db);
    let before = rows_of_14(&db);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), "2");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM events;"), "3");

    drop(SqliteQueue::open(&db, PROJECT).expect("open steps"));
    assert_eq!(user_version(&db), "17");
    assert_eq!(rows_of_14(&db), before, "every row as it was");
    assert_eq!(table_info(&db, "comments"), COMMENTS_INFO);
    assert_eq!(table_info(&db, "comment_versions"), COMMENT_VERSIONS_INFO);
    assert_eq!(column_names(&db, "proposals"), PROPOSAL_COLUMNS);
    assert_eq!(
        index_of(&db, "proposals_comment"),
        ("proposals".to_owned(), "project\ncomment_id".to_owned())
    );
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM comments;"), "0");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM comment_versions;"), "0");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE comment_id IS NULL;"
        ),
        "2",
        "every comment_id NULL"
    );
    let stepped = tables(&db);

    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "17", "stepped once");
    assert_eq!(rows_of_14(&db), before);
    assert_eq!(tables(&db), stepped);
}

/// CM1: a failure inside the step: a hand-made `comment_versions`, the
/// step's second table, fails it after its first; the file stays at 14,
/// with no `comments` table and no `comment_id`: one transaction.
#[test]
fn cm1_a_failure_inside_the_step_leaves_14() {
    let scratch = Scratch::new("cm-cm1-fail");
    let db = scratch.db("q");
    schema_14(&db);
    sqlite3(&db, "CREATE TABLE comment_versions (x TEXT);");
    let before = rows_of_14(&db);
    let tables_before = tables(&db);
    if let Ok(queue) = SqliteQueue::open(&db, PROJECT) {
        panic!("step 15 ran over a hand-made table: {:?}", queue.dump());
    }
    assert_eq!(user_version(&db), "14");
    assert_eq!(rows_of_14(&db), before, "nothing added or lost");
    assert_eq!(
        tables(&db),
        tables_before,
        "no `comments` table left behind"
    );
    assert_eq!(
        index_of(&db, "proposals_comment"),
        (String::new(), String::new()),
        "no index left behind"
    );
    assert_eq!(column_names(&db, "proposals").len(), 50, "no comment_id");
}

/// CM1: every due step runs in one transaction: a schema-13 queue whose
/// step 15 fails stays at 13, step 14's ledger undone too.
#[test]
fn cm1_a_13_queue_failing_at_15_stays_at_13() {
    let scratch = Scratch::new("cm-cm1-thirteen");
    let db = scratch.db("q");
    schema_14(&db);
    sqlite3(
        &db,
        "DROP TABLE collision_marks; PRAGMA user_version = 13; \
         CREATE TABLE comments (x TEXT);",
    );
    let before = rows_of_14(&db);
    assert!(SqliteQueue::open(&db, PROJECT).is_err(), "step 15 fails");
    assert_eq!(user_version(&db), "13");
    assert_eq!(rows_of_14(&db), before);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM sqlite_master WHERE name = 'collision_marks';"
        ),
        "0",
        "step 14 undone with 15"
    );
}

/// CM1: schema 16 (this build's 15 + 1) is `SchemaTooNew` for `open` and
/// `open_existing`, the file left alone.
#[test]
fn cm1_a_newer_schema_is_refused() {
    let scratch = Scratch::new("cm-cm1-newer");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    let newer = QUEUE_SCHEMA_VERSION + 1;
    // 17 since docs/features/work-references.md RF1.
    assert_eq!(newer, 18);
    sqlite3(&db, &format!("PRAGMA user_version = {newer};"));
    for result in [
        SqliteQueue::open(&db, PROJECT).map(|_| ()),
        SqliteQueue::open_existing(&db, PROJECT).map(|_| ()),
    ] {
        match result {
            Err(QueueError::SchemaTooNew { found }) => assert_eq!(found, newer),
            other => panic!("expected SchemaTooNew {newer}, got {other:?}"),
        }
    }
    assert_eq!(user_version(&db), "18", "left alone");
}
