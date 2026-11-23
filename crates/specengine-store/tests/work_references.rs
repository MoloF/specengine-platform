//! docs/features/work-references.md (RM-0137, ADR-0088) "Acceptance
//! criteria", the store's half of RF1: `SqliteQueue::open` through the public
//! API, the older schema and the hand-made objects through the system's
//! `sqlite3`.
//!
//! - RF1: `QUEUE_SCHEMA_VERSION` 16; a new queue stands at 16 with `refs`
//!   (20 columns) as "Queue schema 16" writes it, `PRAGMA table_info` pinned
//!   column by column (name, type, NOT NULL, key), STRICT, the three indexes
//!   `refs_on`, `refs_b3`, `refs_agent_b3` on their columns; `REF_COLUMNS`
//!   equals `PRAGMA table_info`; `PROPOSAL_COLUMNS` stays 51. A schema-15
//!   queue holding proposals, cards, comments, a comment version and events
//!   steps to 16 at its first open, every row as it was, `refs` there and
//!   empty; a second open steps nothing. A failure inside the step (a
//!   hand-made index named `refs_agent_b3`: the step's last statement) leaves
//!   the queue at 15 with no `refs` table: one transaction; a schema-14 queue
//!   failing at step 16 stays at 14 without step 15's tables: every due step
//!   in one transaction. Schema 17 is `SchemaTooNew`, the file left alone.
//!   M: a column dropped from `STEP_16`.
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
    COMMENT_COLUMNS, PROPOSAL_COLUMNS, QUEUE_SCHEMA_VERSION, QueueError, REF_COLUMNS, SqliteQueue,
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

/// "Queue schema 16"'s DDL as `PRAGMA table_info` reads it, `cid name type
/// notnull pk`, "keyed as `comments` is" (`id TEXT PRIMARY KEY`, which a
/// STRICT table reads NOT NULL).
const REFS_INFO: &str = "0 id TEXT 1 1\n1 project TEXT 1 0\n2 on_kind TEXT 1 0\n\
3 on_id TEXT 1 0\n4 on_rev INTEGER 0 0\n5 actor TEXT 1 0\n6 author TEXT 1 0\n\
7 kind TEXT 1 0\n8 title TEXT 0 0\n9 matters TEXT 0 0\n10 url TEXT 0 0\n\
11 name TEXT 0 0\n12 b3 TEXT 0 0\n13 agent_b3 TEXT 0 0\n14 size INTEGER 0 0\n\
15 media TEXT 0 0\n16 meta TEXT 0 0\n17 created_at TEXT 1 0\n18 deleted_by TEXT 0 0\n\
19 deleted_at TEXT 0 0";

/// The names of [`REFS_INFO`], in order.
fn ddl_names() -> Vec<String> {
    REFS_INFO
        .lines()
        .map(|line| line.split(' ').nth(1).expect("a name").to_owned())
        .collect()
}

/// RF1: 16; `refs` as the DDL writes it, STRICT, its three indexes; the
/// column list equals `PRAGMA table_info`; `PROPOSAL_COLUMNS` stays 51. M: a
/// column dropped from `STEP_16`.
#[test]
fn rf1_a_new_queue_stands_at_16_with_refs_pinned() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 16);
    let scratch = Scratch::new("rf-rf1-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "16");

    assert_eq!(table_info(&db, "refs"), REFS_INFO);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT name || ' ' || pk FROM pragma_table_info('comments') WHERE pk > 0;"
        ),
        "id 1",
        "keyed as comments is"
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT strict FROM pragma_table_list WHERE name = 'refs';"
        ),
        "1",
        "refs is STRICT"
    );
    for (index, columns) in [
        ("refs_on", "project\non_kind\non_id"),
        ("refs_b3", "project\nb3"),
        ("refs_agent_b3", "project\nagent_b3"),
    ] {
        assert_eq!(
            index_of(&db, index),
            ("refs".to_owned(), columns.to_owned()),
            "the index {index}"
        );
    }
    assert_eq!(REF_COLUMNS.len(), 20);
    assert_eq!(column_names(&db, "refs"), REF_COLUMNS);
    assert_eq!(ddl_names(), REF_COLUMNS);
    assert_eq!(PROPOSAL_COLUMNS.len(), 51, "PROPOSAL_COLUMNS stays 51");
    assert_eq!(column_names(&db, "proposals"), PROPOSAL_COLUMNS);
    assert_eq!(COMMENT_COLUMNS.len(), 15);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM refs;"), "0");
}

/// Rows of every queue table as this build's schema holds them, written raw:
/// two proposals, a card, a task, a run, two comments (one on a node), a
/// comment version and four events.
fn some_rows(db: &Path) {
    sqlite3(
        db,
        "INSERT INTO proposals (id, project, kind, status, target_id, target_path, author, \
         created_at, updated_at, summary, card_id) VALUES ('PR-0001', 'lantern-keep', \
         'question', 'open', 'MEC-SPRINT', 'docs/spec/movement/sprint.md', \
         '{\"type\":\"agent\",\"role\":\"developer\",\"model\":null,\"run\":null}', \
         '2026-11-22T09:00:00Z', '2026-11-22T09:00:00Z', 'Does a sprint end at zero?', 'C-0001'); \
         INSERT INTO proposals (id, project, kind, status, target_id, base_text, new_text, \
         created_at, updated_at, comment_id) VALUES ('PR-0002', 'lantern-keep', 'update', \
         'rejected', 'MEC-STAMINA', 'old', 'new', '2026-11-22T09:01:00Z', \
         '2026-11-22T09:02:00Z', 'CM-0001'); \
         INSERT INTO cards (id, project, flow, col, position, title, targets, created_by, \
         created_at, updated_at, revision, tasks) VALUES ('C-0001', 'lantern-keep', 'plain', \
         'backlog', 1, 'Write the sprint foreword', '[\"MEC-SPRINT\"]', 'local', \
         '2026-11-22T08:59:00Z', '2026-11-22T08:59:00Z', '2', '[\"T-0001\"]'); \
         INSERT INTO tasks (id, project, status, title, targets, created_at, updated_at, \
         revision) VALUES ('T-0001', 'lantern-keep', 'draft', 'Tune the sprint', \
         '[\"MEC-SPRINT\"]', '2026-11-22T08:58:00Z', '2026-11-22T08:58:00Z', '1'); \
         INSERT INTO runs (task_id, run, role, outcome) VALUES ('T-0001', 1, 'developer', \
         'done'); \
         INSERT INTO comments (id, project, on_kind, on_id, on_rev, reply_to, actor, author, \
         version, body, mentions, created_at, updated_at) VALUES ('CM-0001', 'lantern-keep', \
         'card', 'C-0001', NULL, NULL, 'mira', \
         '{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}', 2, \
         'Keep the rail at 1.1 m?', '[]', '2026-11-22T09:03:00Z', '2026-11-22T09:04:00Z'), \
         ('CM-0002', 'lantern-keep', 'node', 'MEC-SPRINT', 1, NULL, 'lev', \
         '{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}', 1, \
         'The sprint ends at zero.', '[]', '2026-11-22T09:05:00Z', '2026-11-22T09:05:00Z'); \
         INSERT INTO comment_versions (comment_id, project, version, body, written_at) VALUES \
         ('CM-0001', 'lantern-keep', 1, 'Keep the rail at 1.2 m?', '2026-11-22T09:03:00Z'); \
         INSERT INTO events (seq, project, type, payload, at) VALUES \
         (1, 'lantern-keep', 'card.created', '{\"id\":\"C-0001\"}', '2026-11-22T08:59:00Z'), \
         (2, 'lantern-keep', 'proposal.created', '{\"id\":\"PR-0001\"}', \
         '2026-11-22T09:00:00Z'), \
         (3, 'lantern-keep', 'comment.created', \
         '{\"id\":\"CM-0001\",\"on\":\"C-0001\",\"as\":\"mira\"}', '2026-11-22T09:03:00Z'), \
         (4, 'lantern-keep', 'proposal.rejected', '{\"id\":\"PR-0002\"}', \
         '2026-11-22T09:02:00Z');",
    );
}

/// A schema-15 queue holding [`some_rows`]: this build's tables with step
/// 16's undone (the indexes go with their table).
fn schema_15(db: &Path) {
    drop(SqliteQueue::open(db, PROJECT).expect("open"));
    some_rows(db);
    sqlite3(db, "DROP TABLE refs; PRAGMA user_version = 15;");
    assert_eq!(
        sqlite3(
            db,
            "SELECT count(*) FROM sqlite_master WHERE name LIKE 'refs%';"
        ),
        "0",
        "premise: schema 15"
    );
}

/// Every row of the seven queue tables of schema 15, in a stable order.
fn rows_of_15(db: &Path) -> String {
    sqlite3(
        db,
        "SELECT * FROM proposals ORDER BY id; SELECT * FROM cards ORDER BY id; \
         SELECT * FROM tasks ORDER BY id; SELECT * FROM runs ORDER BY task_id, run; \
         SELECT * FROM comments ORDER BY id; \
         SELECT * FROM comment_versions ORDER BY comment_id, version; \
         SELECT * FROM events ORDER BY seq;",
    )
}

/// RF1: a schema-15 queue steps at its first open: every row as it was,
/// `refs` there with its indexes and empty; a second open steps nothing.
#[test]
fn rf1_a_schema_15_queue_steps_keeping_every_row() {
    let scratch = Scratch::new("rf-rf1-step");
    let db = scratch.db("q");
    schema_15(&db);
    let before = rows_of_15(&db);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), "2");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM comments;"), "2");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM events;"), "4");

    drop(SqliteQueue::open(&db, PROJECT).expect("open steps"));
    assert_eq!(user_version(&db), "16");
    assert_eq!(rows_of_15(&db), before, "every row as it was");
    assert_eq!(table_info(&db, "refs"), REFS_INFO);
    assert_eq!(column_names(&db, "refs"), REF_COLUMNS);
    for (index, columns) in [
        ("refs_on", "project\non_kind\non_id"),
        ("refs_b3", "project\nb3"),
        ("refs_agent_b3", "project\nagent_b3"),
    ] {
        assert_eq!(
            index_of(&db, index),
            ("refs".to_owned(), columns.to_owned())
        );
    }
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM refs;"), "0");
    let stepped = tables(&db);

    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "16", "stepped once");
    assert_eq!(rows_of_15(&db), before);
    assert_eq!(tables(&db), stepped);
}

/// RF1: a failure inside the step: a hand-made index named `refs_agent_b3`
/// (on another table), the step's last statement, fails it after its
/// `CREATE TABLE`; the file stays at 15 with no `refs` table: one
/// transaction.
#[test]
fn rf1_a_failure_inside_the_step_leaves_15() {
    let scratch = Scratch::new("rf-rf1-fail");
    let db = scratch.db("q");
    schema_15(&db);
    sqlite3(&db, "CREATE INDEX refs_agent_b3 ON cards (title);");
    let before = rows_of_15(&db);
    let tables_before = tables(&db);
    if let Ok(queue) = SqliteQueue::open(&db, PROJECT) {
        panic!("step 16 ran over a hand-made index: {:?}", queue.dump());
    }
    assert_eq!(user_version(&db), "15");
    assert_eq!(rows_of_15(&db), before, "nothing added or lost");
    assert_eq!(tables(&db), tables_before, "no `refs` table left behind");
    for index in ["refs_on", "refs_b3"] {
        assert_eq!(
            index_of(&db, index),
            (String::new(), String::new()),
            "no {index} left behind"
        );
    }
}

/// RF1: every due step runs in one transaction: a schema-14 queue whose step
/// 16 fails stays at 14, step 15's tables undone too.
#[test]
fn rf1_a_14_queue_failing_at_16_stays_at_14() {
    let scratch = Scratch::new("rf-rf1-fourteen");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    sqlite3(
        &db,
        "DROP TABLE refs; DROP TABLE comment_versions; DROP TABLE comments; \
         DROP INDEX IF EXISTS proposals_comment; ALTER TABLE proposals DROP COLUMN comment_id; \
         PRAGMA user_version = 14; CREATE TABLE refs (x TEXT);",
    );
    let before = tables(&db);
    assert!(SqliteQueue::open(&db, PROJECT).is_err(), "step 16 fails");
    assert_eq!(user_version(&db), "14");
    assert_eq!(tables(&db), before, "step 15's tables undone with 16");
    assert_eq!(
        column_names(&db, "proposals").len(),
        50,
        "no comment_id either"
    );
}

/// RF1: schema 17 (this build's 16 + 1) is `SchemaTooNew` for `open` and
/// `open_existing`, the file left alone.
#[test]
fn rf1_a_newer_schema_is_refused() {
    let scratch = Scratch::new("rf-rf1-newer");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    let newer = QUEUE_SCHEMA_VERSION + 1;
    assert_eq!(newer, 17);
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
    assert_eq!(user_version(&db), "17", "left alone");
}
