//! docs/features/collision-pass.md (RM-0060) "Acceptance criteria", the
//! store's half: `SqliteQueue` through the public API, the raw steps (an
//! older schema, a hand-written row) through the system's `sqlite3`.
//!
//! - CP6: `QUEUE_SCHEMA_VERSION` 14; a new queue stands at 14 with
//!   `collision_marks` pinned to `PRAGMA table_info` (nine columns, the key
//!   `(project, a, b)`, STRICT) and `PROPOSAL_COLUMNS` unchanged (50); a
//!   schema 13 queue holding a proposal, a card, a task, a run and events
//!   steps at its first open with every row kept and an empty ledger;
//!   `open_existing` on 13 reads no marks and steps nothing; a hand-made
//!   `collision_marks` fails step 14, the file left at 13; schema 15 is
//!   `SchemaTooNew`. `dump()`, `stored_rows()` and `counts()` are equal with
//!   marks and without, and a restore leaves the ledger empty, a queue
//!   holding only marks restored into too (the ledger cleared). M: the
//!   ledger dumped.
//! - CP3, CP5, the store's part: `mark_collisions` writes `a` before `b`
//!   with each norm beside its name, the author's JSON and the clock, and
//!   no event; a second mark of the pair in either order replaces it
//!   (`replaced` 1); a pair of one node, a bad norm, a pair given twice, a
//!   `reported` pair without a proposal or a `distinct` one with one is
//!   `Invalid` with nothing written; another project's rows are not read; a
//!   hand-written row that does not read is ignored with its note. M: the
//!   batch written in part; an event emitted.
//!
//! The names (a lamp's wick and hood) are invented here; every expected
//! value is worked out from the spec, never read back from a run.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use specengine_core::collisions::CollisionVerdict;
use specengine_core::intake::IntakeSeverity;
use specengine_core::proposal::Author;
use specengine_store::{
    CollisionMark, Intake, NewCollisionMark, NewIntake, PROPOSAL_COLUMNS, Place, ProposalKind,
    ProposalQueue as _, QUEUE_SCHEMA_VERSION, QueueError, Restore, SqliteQueue,
};

const T0: &str = "2026-11-12T09:00:00Z";
const T1: &str = "2026-11-12T09:30:00Z";
const PROJECT: &str = "lantern-keep";
const WICK: &str = "MEC-WICK";
const HOOD: &str = "MEC-HOOD";

fn norm(digit: char) -> String {
    format!("b3:{}", digit.to_string().repeat(64))
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

fn user_version(db: &Path) -> String {
    sqlite3(db, "PRAGMA user_version;")
}

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "main".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn agent() -> Author {
    Author::new(Some("reviewer".to_owned()), None, None).expect("an agent")
}

/// A question on the wick, stored: its ID.
fn question(queue: &mut SqliteQueue, text: &str) -> String {
    let item = NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/spec/lamp.md".to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: vec![WICK.to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some("Trim it first.".to_owned()),
            price_of_other: Some("One scene re-timed.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        card_id: None,
        actor: None,
    };
    let result = queue
        .create_intake(&item, &[], None, T0)
        .unwrap_or_else(|error| panic!("create_intake: {error}"));
    result.created.expect("stored").id
}

fn mark(
    a: &str,
    a_norm: &str,
    b: &str,
    b_norm: &str,
    verdict: CollisionVerdict,
    proposal: Option<&str>,
) -> NewCollisionMark {
    NewCollisionMark {
        a: a.to_owned(),
        a_norm: a_norm.to_owned(),
        b: b.to_owned(),
        b_norm: b_norm.to_owned(),
        verdict,
        proposal: proposal.map(str::to_owned),
    }
}

/// Every table's rows but the ledger's, in a stable order.
fn rows_but_the_ledger(db: &Path) -> String {
    sqlite3(
        db,
        "SELECT * FROM proposals ORDER BY id; SELECT * FROM cards ORDER BY id; SELECT * FROM \
         tasks; SELECT * FROM runs; SELECT * FROM events ORDER BY seq;",
    )
}

fn event_count(db: &Path) -> String {
    sqlite3(db, "SELECT count(*) FROM events;")
}

// ------------------------------------------------------------------- CP6

/// CP6: 14; `collision_marks` as `PRAGMA table_info` lists it (cid, name,
/// type, notnull, pk), STRICT; `proposals` still the 50 columns.
#[test]
fn cp6_a_new_queue_stands_at_14_with_the_ledger_pinned() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 14);
    let scratch = Scratch::new("cm-cp6-new");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    assert_eq!(user_version(&db), "14");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT cid || ' ' || name || ' ' || type || ' ' || \"notnull\" || ' ' || pk \
             FROM pragma_table_info('collision_marks') ORDER BY cid;"
        ),
        "0 project TEXT 1 1\n1 a TEXT 1 2\n2 a_norm TEXT 1 0\n3 b TEXT 1 3\n4 b_norm TEXT 1 0\n\
         5 verdict TEXT 1 0\n6 proposal TEXT 0 0\n7 author TEXT 1 0\n8 at TEXT 1 0"
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT strict FROM pragma_table_list WHERE name = 'collision_marks';"
        ),
        "1",
        "STRICT"
    );
    assert_eq!(PROPOSAL_COLUMNS.len(), 50, "the proposals are unchanged");
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "50"
    );
}

/// A schema 13 queue: a question, a card, events (as this build stores
/// them), the ledger dropped, `user_version` 13.
fn schema_13(db: &Path) {
    let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
    question(&mut queue, "Is the wick trimmed before dusk?");
    question(&mut queue, "Does the hood close in rain?");
    drop(queue);
    sqlite3(db, "DROP TABLE collision_marks; PRAGMA user_version = 13;");
}

/// CP6: a schema 13 queue with rows steps to 14 at its first open: every
/// row of the five tables as it was, the ledger there and empty; a second
/// open steps nothing.
#[test]
fn cp6_a_schema_13_queue_steps_keeping_every_row() {
    let scratch = Scratch::new("cm-cp6-step");
    let db = scratch.db("q");
    schema_13(&db);
    let before = rows_but_the_ledger(&db);
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), "2");
    assert_ne!(event_count(&db), "0", "premise: events");

    let queue = SqliteQueue::open(&db, PROJECT).expect("open steps");
    assert_eq!(user_version(&db), "14");
    assert_eq!(rows_but_the_ledger(&db), before, "every row as it was");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM collision_marks;"), "0");
    assert_eq!(queue.collision_marks().expect("read").marks, Vec::new());
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("open again"));
    assert_eq!(user_version(&db), "14", "stepped once");
    assert_eq!(rows_but_the_ledger(&db), before);
}

/// CP6: `open_existing` on a schema 13 queue reads no marks and steps
/// nothing.
#[test]
fn cp6_open_existing_reads_a_13_queue_as_no_marks() {
    let scratch = Scratch::new("cm-cp6-existing");
    let db = scratch.db("q");
    schema_13(&db);
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("a queue");
    let ledger = queue.collision_marks().expect("read");
    assert_eq!(ledger.marks, Vec::new());
    assert_eq!(ledger.unreadable, Vec::<String>::new());
    drop(queue);
    assert_eq!(user_version(&db), "13", "nothing stepped");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM sqlite_master WHERE name = 'collision_marks';"
        ),
        "0"
    );
}

/// CP6: a hand-made `collision_marks` fails step 14: the file stays at 13,
/// nothing added or lost.
#[test]
fn cp6_a_hand_made_ledger_fails_step_14() {
    let scratch = Scratch::new("cm-cp6-fail");
    let db = scratch.db("q");
    schema_13(&db);
    sqlite3(&db, "CREATE TABLE collision_marks (x TEXT);");
    let before = rows_but_the_ledger(&db);
    if let Ok(queue) = SqliteQueue::open(&db, PROJECT) {
        panic!("step 14 ran over a hand-made table: {:?}", queue.dump());
    }
    assert_eq!(user_version(&db), "13");
    assert_eq!(rows_but_the_ledger(&db), before, "nothing added or lost");
}

/// CP6: schema 15 (this build's 14) is `SchemaTooNew` for `open` and
/// `open_existing`, the file left alone.
#[test]
fn cp6_a_newer_schema_is_refused() {
    let scratch = Scratch::new("cm-cp6-newer");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    let newer = QUEUE_SCHEMA_VERSION + 1;
    assert_eq!(newer, 15);
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
    assert_eq!(user_version(&db), "15", "left alone");
}

/// CP6: marks change neither `dump()`, `stored_rows()` nor `counts()`; the
/// rows restored into a fresh queue leave its ledger empty. M: the ledger
/// dumped.
#[test]
fn cp6_backups_never_read_the_ledger() {
    let scratch = Scratch::new("cm-cp6-backup");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = question(&mut queue, "Is the wick trimmed before dusk?");
    let dump = queue.dump().expect("dump");
    let rows = queue.stored_rows().expect("rows");
    let counts = queue.counts().expect("counts");
    queue
        .mark_collisions(
            &[
                mark(
                    WICK,
                    &norm('1'),
                    HOOD,
                    &norm('2'),
                    CollisionVerdict::Distinct,
                    None,
                ),
                mark(
                    "MEC-OIL",
                    &norm('3'),
                    HOOD,
                    &norm('4'),
                    CollisionVerdict::Reported,
                    Some(&id),
                ),
            ],
            &agent(),
            T1,
        )
        .expect("marked");
    assert_eq!(queue.collision_marks().expect("read").marks.len(), 2);
    assert_eq!(queue.dump().expect("dump"), dump, "dump()");
    assert_eq!(queue.stored_rows().expect("rows"), rows, "stored_rows()");
    assert_eq!(queue.counts().expect("counts"), counts, "counts()");
    let marked_rows = queue.stored_rows().expect("rows");
    drop(queue);

    let fresh = scratch.db("fresh");
    let mut restored = SqliteQueue::open(&fresh, PROJECT).expect("open fresh");
    assert_eq!(
        restored.restore(&marked_rows).expect("restore"),
        Restore::Restored
    );
    assert_eq!(restored.collision_marks().expect("read").marks, Vec::new());
    assert_eq!(
        sqlite3(&fresh, "SELECT count(*) FROM collision_marks;"),
        "0"
    );
    assert_eq!(restored.dump().expect("dump"), dump, "the same queue");
    drop(restored);

    // A queue holding only a mark is empty to a restore (the five tables
    // alone are read): restored, and the ledger starts empty (D1; the
    // orchestrator's iteration-2 ruling: a restore clears it).
    let marked_only = scratch.db("marked-only");
    let mut target = SqliteQueue::open(&marked_only, PROJECT).expect("open");
    target
        .mark_collisions(
            &[mark(
                WICK,
                &norm('1'),
                HOOD,
                &norm('2'),
                CollisionVerdict::Distinct,
                None,
            )],
            &agent(),
            T0,
        )
        .expect("marked");
    assert_eq!(
        target.restore(&marked_rows).expect("restore"),
        Restore::Restored,
        "the ledger is no table a restore finds occupied"
    );
    assert_eq!(target.dump().expect("dump"), dump);
    assert_eq!(target.collision_marks().expect("read").marks, Vec::new());
    assert_eq!(
        sqlite3(&marked_only, "SELECT count(*) FROM collision_marks;"),
        "0"
    );
}

// -------------------------------------------------------------- CP3, CP5

/// CP3, CP5: a mark given `b` before `a` is stored `a` < `b`, each norm
/// with its name, the author's JSON, the clock; no event; the same pair
/// again, in the input's other order, replaces it; a `reported` pair keeps
/// its proposal.
#[test]
fn cp5_a_mark_is_one_row_per_pair_and_no_event() {
    let scratch = Scratch::new("cm-cp5-row");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = question(&mut queue, "Is the wick trimmed before dusk?");
    let events = event_count(&db);
    let result = queue
        .mark_collisions(
            &[mark(
                WICK,
                &norm('1'),
                HOOD,
                &norm('2'),
                CollisionVerdict::Distinct,
                None,
            )],
            &agent(),
            T0,
        )
        .expect("marked");
    assert_eq!((result.marked, result.replaced), (1, 0));
    assert_eq!(
        sqlite3(
            &db,
            "SELECT project, a, a_norm, b, b_norm, verdict, coalesce(proposal, '<null>'), \
             author, at FROM collision_marks;"
        ),
        format!(
            "{PROJECT}|{HOOD}|{}|{WICK}|{}|distinct|<null>|{{\"type\":\"agent\",\"role\":\
             \"reviewer\",\"model\":null,\"run\":null}}|{T0}",
            norm('2'),
            norm('1')
        )
    );
    assert_eq!(event_count(&db), events, "no event");
    assert_eq!(
        queue.collision_marks().expect("read").marks,
        [CollisionMark {
            a: HOOD.to_owned(),
            a_norm: norm('2'),
            b: WICK.to_owned(),
            b_norm: norm('1'),
            verdict: CollisionVerdict::Distinct,
            proposal: None,
            author: "{\"type\":\"agent\",\"role\":\"reviewer\",\"model\":null,\"run\":null}"
                .to_owned(),
            at: T0.to_owned(),
        }]
    );

    let result = queue
        .mark_collisions(
            &[mark(
                HOOD,
                &norm('5'),
                WICK,
                &norm('6'),
                CollisionVerdict::Reported,
                Some(&id),
            )],
            &Author::human(),
            T1,
        )
        .expect("marked again");
    assert_eq!((result.marked, result.replaced), (1, 1));
    let marks = queue.collision_marks().expect("read").marks;
    assert_eq!(marks.len(), 1, "one row per pair");
    assert_eq!(marks[0].verdict, CollisionVerdict::Reported);
    assert_eq!(marks[0].proposal.as_deref(), Some(id.as_str()));
    assert_eq!(
        (marks[0].a_norm.as_str(), marks[0].b_norm.as_str()),
        (norm('5').as_str(), norm('6').as_str())
    );
    assert_eq!(
        marks[0].author,
        "{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}"
    );
    assert_eq!(marks[0].at, T1);
    assert_eq!(event_count(&db), events, "no event");
}

/// CP5: each refusal `Invalid`, nothing written, no event: a pair of one
/// node, a bad norm (short, upper-case, no `b3:`), a pair given twice (in
/// either order), `reported` without a proposal, `distinct` with one; a
/// batch whose last pair is bad writes none of it.
#[test]
fn cp5_a_bad_pair_writes_nothing() {
    let scratch = Scratch::new("cm-cp5-refused");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let id = question(&mut queue, "Is the wick trimmed before dusk?");
    let events = event_count(&db);
    let good = mark(
        WICK,
        &norm('1'),
        HOOD,
        &norm('2'),
        CollisionVerdict::Distinct,
        None,
    );
    let upper = format!("b3:{}", "A".repeat(64));
    let cases: Vec<(&str, Vec<NewCollisionMark>)> = vec![
        (
            "one node",
            vec![mark(
                WICK,
                &norm('1'),
                WICK,
                &norm('1'),
                CollisionVerdict::Distinct,
                None,
            )],
        ),
        (
            "a short norm",
            vec![mark(
                WICK,
                "b3:12",
                HOOD,
                &norm('2'),
                CollisionVerdict::Distinct,
                None,
            )],
        ),
        (
            "an upper-case norm",
            vec![mark(
                WICK,
                &norm('1'),
                HOOD,
                &upper,
                CollisionVerdict::Distinct,
                None,
            )],
        ),
        (
            "no b3:",
            vec![mark(
                WICK,
                &"1".repeat(67),
                HOOD,
                &norm('2'),
                CollisionVerdict::Distinct,
                None,
            )],
        ),
        (
            "twice",
            vec![
                good.clone(),
                mark(
                    HOOD,
                    &norm('2'),
                    WICK,
                    &norm('1'),
                    CollisionVerdict::Distinct,
                    None,
                ),
            ],
        ),
        (
            "reported without a proposal",
            vec![mark(
                WICK,
                &norm('1'),
                HOOD,
                &norm('2'),
                CollisionVerdict::Reported,
                None,
            )],
        ),
        (
            "distinct with a proposal",
            vec![mark(
                WICK,
                &norm('1'),
                HOOD,
                &norm('2'),
                CollisionVerdict::Distinct,
                Some(&id),
            )],
        ),
        (
            "the last bad",
            vec![
                good.clone(),
                mark(
                    "MEC-OIL",
                    &norm('3'),
                    "MEC-OIL",
                    &norm('3'),
                    CollisionVerdict::Distinct,
                    None,
                ),
            ],
        ),
    ];
    for (what, marks) in cases {
        match queue.mark_collisions(&marks, &agent(), T0) {
            Err(QueueError::Invalid(_)) => {}
            other => panic!("{what}: expected Invalid, got {other:?}"),
        }
        assert_eq!(
            sqlite3(&db, "SELECT count(*) FROM collision_marks;"),
            "0",
            "{what}: nothing written"
        );
        assert_eq!(event_count(&db), events, "{what}: no event");
    }
    match queue.mark_collisions(&[good], &agent(), "noon") {
        Err(QueueError::Invalid(_)) => {}
        other => panic!("a bad clock: expected Invalid, got {other:?}"),
    }
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM collision_marks;"), "0");
}

/// CP6, D1: a restore empties the bound project's ledger and leaves
/// another project's rows in the same queue DB as they were (the
/// orchestrator's iteration 3: the clear is scoped to the project). M: the
/// clear unscoped.
#[test]
fn cp6_a_restore_leaves_another_projects_ledger_alone() {
    let scratch = Scratch::new("cm-cp6-other");
    let source = scratch.db("source");
    let mut queue = SqliteQueue::open(&source, PROJECT).expect("open");
    question(&mut queue, "Is the wick trimmed before dusk?");
    let rows = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    drop(queue);

    let db = scratch.db("shared");
    let mut theirs = SqliteQueue::open(&db, "dock-hire").expect("open theirs");
    theirs
        .mark_collisions(
            &[mark(
                WICK,
                &norm('7'),
                HOOD,
                &norm('8'),
                CollisionVerdict::Distinct,
                None,
            )],
            &agent(),
            T0,
        )
        .expect("theirs");
    drop(theirs);
    let mut ours = SqliteQueue::open(&db, PROJECT).expect("open ours");
    ours.mark_collisions(
        &[mark(
            WICK,
            &norm('1'),
            HOOD,
            &norm('2'),
            CollisionVerdict::Distinct,
            None,
        )],
        &agent(),
        T0,
    )
    .expect("ours");
    assert_eq!(ours.restore(&rows).expect("restore"), Restore::Restored);
    assert_eq!(ours.dump().expect("dump"), dump);
    assert_eq!(
        ours.collision_marks().expect("read").marks,
        Vec::new(),
        "ours cleared"
    );
    drop(ours);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT project || '|' || a || '|' || a_norm FROM collision_marks ORDER BY project;"
        ),
        format!("dock-hire|{HOOD}|{}", norm('8')),
        "theirs left alone"
    );
    let theirs = SqliteQueue::open(&db, "dock-hire").expect("open theirs");
    assert_eq!(theirs.collision_marks().expect("read").marks.len(), 1);
}

/// The ledger is the bound project's: another project's rows are not read
/// (nor written over).
#[test]
fn cp5_the_ledger_is_per_project() {
    let scratch = Scratch::new("cm-cp5-project");
    let db = scratch.db("q");
    let mut ours = SqliteQueue::open(&db, PROJECT).expect("open");
    ours.mark_collisions(
        &[mark(
            WICK,
            &norm('1'),
            HOOD,
            &norm('2'),
            CollisionVerdict::Distinct,
            None,
        )],
        &agent(),
        T0,
    )
    .expect("ours");
    drop(ours);
    let mut theirs = SqliteQueue::open(&db, "dock-hire").expect("open theirs");
    assert_eq!(theirs.collision_marks().expect("read").marks, Vec::new());
    let result = theirs
        .mark_collisions(
            &[mark(
                WICK,
                &norm('7'),
                HOOD,
                &norm('8'),
                CollisionVerdict::Distinct,
                None,
            )],
            &agent(),
            T1,
        )
        .expect("theirs");
    assert_eq!(
        (result.marked, result.replaced),
        (1, 0),
        "not ours replaced"
    );
    drop(theirs);
    let ours = SqliteQueue::open(&db, PROJECT).expect("open ours");
    let marks = ours.collision_marks().expect("read").marks;
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].a_norm, norm('2'));
}

/// The "Store" rule: a hand-written row that does not read (`a` not before
/// `b`, a bad norm, an unknown verdict, `reported` without a proposal) is
/// ignored and noted `collision mark <a> <b>: unreadable (<column>:
/// <reason>); ignored`, by `(a, b)`; the readable row still reads.
#[test]
fn cp3_an_unreadable_row_is_ignored_with_a_note() {
    let scratch = Scratch::new("cm-unreadable");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue
        .mark_collisions(
            &[mark(
                WICK,
                &norm('1'),
                HOOD,
                &norm('2'),
                CollisionVerdict::Distinct,
                None,
            )],
            &agent(),
            T0,
        )
        .expect("marked");
    let (one, two) = (norm('1'), norm('2'));
    let insert = |a: &str, a_norm: &str, b: &str, verdict: &str, proposal: &str| {
        format!(
            "INSERT INTO collision_marks VALUES ('{PROJECT}', '{a}', '{a_norm}', '{b}', '{two}', \
             '{verdict}', {proposal}, '{{}}', '{T0}');"
        )
    };
    sqlite3(
        &db,
        &[
            insert("Z-B", &one, "Z-A", "distinct", "NULL"),
            insert("Z-C", "b3:short", "Z-D", "distinct", "NULL"),
            insert("Z-E", &one, "Z-F", "maybe", "NULL"),
            insert("Z-G", &one, "Z-H", "reported", "NULL"),
        ]
        .concat(),
    );
    let ledger = queue.collision_marks().expect("read");
    assert_eq!(ledger.marks.len(), 1, "{ledger:?}");
    assert_eq!(ledger.marks[0].a, HOOD);
    assert_eq!(ledger.unreadable.len(), 4, "{ledger:?}");
    for (index, (a, b, column)) in [
        ("Z-B", "Z-A", "(a: "),
        ("Z-C", "Z-D", "(a_norm: "),
        ("Z-E", "Z-F", "(verdict: "),
        ("Z-G", "Z-H", "(proposal: "),
    ]
    .into_iter()
    .enumerate()
    {
        let note = &ledger.unreadable[index];
        assert!(
            note.starts_with(&format!("collision mark {a} {b}: unreadable {column}"))
                && note.ends_with("); ignored"),
            "{note}"
        );
    }
}
