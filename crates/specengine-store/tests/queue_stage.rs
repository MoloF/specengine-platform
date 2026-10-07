//! docs/features/decision-staging.md, the store's half ("Data": queue
//! schema 5, `staged`, `staged_at`, the store ops, the events, the corrupt
//! rows, the backup's rows; `docs/canon/decision-staging.md` "The stage",
//! "Queue"):
//!
//! - AC-02: a fresh queue stands at `user_version` 5, `proposals` holding
//!   the 43 columns of `PROPOSAL_COLUMNS` (`PRAGMA table_info`), `staged`,
//!   `staged_at` last; a schema-4 database is read unstepped (43 columns,
//!   the two `None`) and opens as 5, its rows kept, both `NULL`; schema 6
//!   is refused, left alone. M: the columns without the version bump.
//! - The ops: `stage_from` sets both (`updated_at` = `now`) and logs one
//!   `proposal.staged` `{id, staged, staged_at}` (the payload's bytes), a
//!   second replaces; `unstage_from` clears both and logs one
//!   `proposal.unstaged` `{id}`, nothing staged: `Ok(false)`, no write, no
//!   event; a lost compare-and-set, a state other than `open`, a flag the
//!   kind takes none of: refused, nothing written.
//! - AC-12: every op that leaves `open` clears both in its own
//!   transaction, no `proposal.unstaged`; `.approved`, `.rejected` carry
//!   `staged_at` only when the decision confirms a stage; a stage on
//!   another state, one column `NULL` alone, another shape, a flag of a
//!   kind that takes none: a corrupt row, named (`get`, `list` fail,
//!   `list_readable` skips it); `dump`, `restore`, `dump` byte-identical
//!   with stages; `stored_rows` reads tasks at schema 4 and 5. M: export
//!   drops `staged`; `stored_rows` keeps `== 4`.

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Scratch, blake3_hex};
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_store::{
    Decision, EVENT_STAGED, EVENT_UNSTAGED, Intake, NewIntake, NewProposal, NewTask,
    PROPOSAL_COLUMNS, Place, ProposalFilter, ProposalKind, ProposalQueue as _, ProposalStatus,
    QUEUE_SCHEMA_VERSION, QueueError, SqliteQueue, Stage, StagedChoice, StoredQueue, patch_hash,
    proposal_columns,
};

const T0: &str = "2026-10-07T09:00:00Z";
const T1: &str = "2026-10-07T09:14:02Z";
const T2: &str = "2026-10-07T09:20:00Z";
const T3: &str = "2026-10-07T09:30:00Z";
const PROJECT: &str = "lantern-keep";

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r/t1".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn author() -> Author {
    Author::new(Some("developer".to_owned()), None, None).expect("an agent")
}

fn update(target: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/spec/movement/stamina.md".to_owned(),
        place: place(),
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

fn new_file(path: &str, id: &str) -> NewProposal {
    let text = format!("---\nid: {id}\n---\n\n# New\n");
    NewProposal {
        kind: ProposalKind::Create,
        patch_hash: patch_hash(id, "", &text),
        target_id: id.to_owned(),
        target_path: path.to_owned(),
        place: place(),
        base_hash: None,
        base_text: None,
        new_text: text,
        rationale: "A new record.".to_owned(),
        author: author(),
        diagnostics: Vec::new(),
        new_ids: vec![id.to_owned()],
    }
}

fn question(target: &str) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/spec/movement/stamina.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: vec![target.to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: "Does a sprint end at zero?".to_owned(),
            working_answer: Some("Yes.".to_owned()),
            price_of_other: Some("A rebalance.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
        },
    }
}

fn discrepancy(target: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/spec/movement/stamina.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: vec![target.to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: "Walking drains stamina.".to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "src/stamina.rs".to_owned(),
                qpath: None,
                lines: Some("3-9".to_owned()),
                observed: "walks".to_owned(),
                documented: "rests".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(1),
            distinct_from: Vec::new(),
        },
    }
}

fn owner(staged_at: Option<&str>) -> Decision {
    Decision {
        decided_by: "Ann Owner <ann@example.org>".to_owned(),
        note: Some("Because.".to_owned()),
        staged_at: staged_at.map(str::to_owned),
    }
}

fn approve_stage(option: Option<u64>, note: Option<&str>, span_hash: Option<&str>) -> Stage {
    Stage::Approve {
        option,
        answer: None,
        canon: None,
        note: note.map(str::to_owned),
        span_hash: span_hash.map(str::to_owned),
    }
}

fn reject_stage(reason: &str) -> Stage {
    Stage::Reject {
        reason: reason.to_owned(),
    }
}

/// `sqlite3 <db> <sql>`, which must succeed: its stdout, trimmed.
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

/// `staged|staged_at` of `id` as stored (`NULL` as the empty text).
fn stage_columns(db: &Path, id: &str) -> String {
    sqlite3(
        db,
        &format!("SELECT staged, staged_at FROM proposals WHERE id = '{id}';"),
    )
}

/// `type|payload` of every event after `seq`, as stored.
fn events_after(db: &Path, seq: i64) -> Vec<String> {
    sqlite3(
        db,
        &format!("SELECT type || '|' || payload FROM events WHERE seq > {seq} ORDER BY seq;"),
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

fn last_seq(db: &Path) -> i64 {
    sqlite3(db, "SELECT coalesce(max(seq), 0) FROM events;")
        .parse()
        .expect("a seq")
}

fn column_at(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

/// A queue holding an update (`PR-0001`), a question (`PR-0002`) and a
/// discrepancy (`PR-0003`), all `open`.
fn three(db: &Path) -> SqliteQueue {
    let mut queue = SqliteQueue::open(db, PROJECT).expect("open");
    assert_eq!(
        queue.create(&update("EDGE-STAM-ZERO"), T0).unwrap().id,
        "PR-0001"
    );
    for (want, intake) in [
        ("PR-0002", question("MEC-SPRINT")),
        ("PR-0003", discrepancy("MEC-SPRINT")),
    ] {
        let made = queue
            .create_intake(&intake, &[], None, T0)
            .expect("intake")
            .created
            .expect("stored");
        assert_eq!(made.id, want);
    }
    queue
}

// -------------------------------------------------------------- schema

/// AC-02: schema 5, its 43 columns; a schema-4 database read unstepped,
/// then opened as 5 with its rows and both `NULL`; schema 6 refused.
#[test]
fn ac02_schema_5_holds_43_columns_and_a_schema_4_database_opens_as_5() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 5);
    assert_eq!(PROPOSAL_COLUMNS.len(), 43);
    assert_eq!(PROPOSAL_COLUMNS[40..], ["task_id", "staged", "staged_at"]);
    assert_eq!(proposal_columns(4), Some(&PROPOSAL_COLUMNS[..41]));
    assert_eq!(proposal_columns(5), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(6), None);

    let scratch = Scratch::new("qst-schema");
    let db = scratch.db("q");
    let mut queue = three(&db);
    let task = queue
        .create_task(
            &NewTask {
                git_common_dir: "/r/.git".to_owned(),
                title: Some("Tune stamina".to_owned()),
                goal: None,
                targets: vec!["MEC-STAMINA".to_owned()],
                author: Author::human(),
            },
            T0,
        )
        .expect("a task");
    let made = queue.stored_rows().expect("rows");
    drop(queue);
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "5");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT group_concat(name || ' ' || type, ',') FROM pragma_table_info('proposals');"
        ),
        PROPOSAL_COLUMNS
            .iter()
            .map(|name| format!("{name} TEXT"))
            .collect::<Vec<_>>()
            .join(","),
        "`PRAGMA table_info` is PROPOSAL_COLUMNS, `staged`, `staged_at` TEXT last"
    );

    // The database as task-package's build left it: schema 4, 41 columns.
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN staged_at; ALTER TABLE proposals DROP COLUMN staged; \
         PRAGMA user_version = 4;",
    );
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "41"
    );
    let read = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file exists");
    let unstepped = read.stored_rows().expect("rows of a schema-4 DB");
    drop(read);
    assert_eq!(
        sqlite3(&db, "PRAGMA user_version;"),
        "4",
        "a read steps nothing"
    );
    assert_eq!(unstepped, made, "read at 43 columns, the two None");
    assert_eq!(unstepped.tasks.len(), 1, "its task read at schema 4");
    assert_eq!(unstepped.tasks[0].id(), Some(task.id.as_str()));

    let queue = SqliteQueue::open(&db, PROJECT).expect("opens, stepping");
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "5");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT group_concat(name, ',') FROM pragma_table_info('proposals');"
        ),
        PROPOSAL_COLUMNS.join(",")
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE staged IS NULL AND staged_at IS NULL;"
        ),
        "3"
    );
    assert_eq!(queue.stored_rows().expect("rows"), made, "rows kept");
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        let proposal = queue.get(id).expect("get").expect("kept");
        assert_eq!(proposal.staged, None, "{id}");
        assert_eq!(proposal.status, ProposalStatus::Open, "{id}");
    }
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("reopen"));
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "5", "stepped once");

    // Schema 6: refused by a read and an open, left alone.
    sqlite3(&db, "PRAGMA user_version = 6;");
    let dumped = sqlite3(&db, "SELECT count(*) FROM proposals;");
    for result in [
        SqliteQueue::open(&db, PROJECT).map(|_| ()),
        SqliteQueue::open_existing(&db, PROJECT).map(|_| ()),
    ] {
        match result {
            Err(QueueError::SchemaTooNew { found: 6 }) => {}
            other => panic!("expected SchemaTooNew 6, got {other:?}"),
        }
    }
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "6");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), dumped);
}

// ------------------------------------------------------------ the ops

/// `stage_from`: both columns set, `updated_at` = `now`, one
/// `proposal.staged` with the stage's bytes; a second replaces it;
/// `unstage_from`: both `NULL`, one `proposal.unstaged` `{id}`; nothing
/// staged: `Ok(false)`, no write, no event.
#[test]
fn stage_sets_replaces_and_unstage_clears_with_one_event_each() {
    assert_eq!(EVENT_STAGED, "proposal.staged");
    assert_eq!(EVENT_UNSTAGED, "proposal.unstaged");
    let scratch = Scratch::new("qst-ops");
    let db = scratch.db("q");
    let mut queue = three(&db);
    let report = queue.get("PR-0003").unwrap().unwrap();
    assert_eq!(report.staged, None);
    assert_eq!(report.seen().staged, None);
    let seq = last_seq(&db);

    let first = approve_stage(Some(1), Some("keep the cap"), None);
    let staged = queue
        .stage_from("PR-0003", &report.seen(), &first, T1)
        .expect("staged");
    assert_eq!(
        staged.staged,
        Some(StagedChoice {
            stage: first.clone(),
            at: T1.to_owned()
        })
    );
    assert_eq!(staged.updated_at, T1);
    assert_eq!(
        staged.status,
        ProposalStatus::Open,
        "an attribute, not a status"
    );
    let approve_json = "{\"decision\":\"approve\",\"option\":1,\"answer\":null,\"canon\":null,\
                        \"note\":\"keep the cap\",\"span_hash\":null}";
    assert_eq!(first.to_json(), approve_json);
    assert_eq!(
        stage_columns(&db, "PR-0003"),
        format!("{approve_json}|{T1}")
    );
    assert_eq!(
        events_after(&db, seq),
        [format!(
            "proposal.staged|{{\"id\":\"PR-0003\",\"staged\":{approve_json},\"staged_at\":\"{T1}\"}}"
        )]
    );

    // A stale read: refused, nothing written.
    match queue.stage_from("PR-0003", &report.seen(), &reject_stage("no"), T2) {
        Err(QueueError::Changed { id, .. }) => assert_eq!(id, "PR-0003"),
        other => panic!("expected Changed, got {other:?}"),
    }
    assert_eq!(
        stage_columns(&db, "PR-0003"),
        format!("{approve_json}|{T1}")
    );
    assert_eq!(events_after(&db, seq).len(), 1);

    // A second stage replaces the first; its event the new one.
    let second = reject_stage("duplicate of \"PR-0002\" \\ see");
    let replaced = queue
        .stage_from("PR-0003", &staged.seen(), &second, T2)
        .expect("replaced");
    let reject_json =
        "{\"decision\":\"reject\",\"reason\":\"duplicate of \\\"PR-0002\\\" \\\\ see\"}";
    assert_eq!(second.to_json(), reject_json);
    assert_eq!(Stage::from_stored(reject_json), Some(second.clone()));
    assert_eq!(stage_columns(&db, "PR-0003"), format!("{reject_json}|{T2}"));
    assert_eq!(replaced.updated_at, T2);
    let events = events_after(&db, seq);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(
        events[1],
        format!(
            "proposal.staged|{{\"id\":\"PR-0003\",\"staged\":{reject_json},\"staged_at\":\"{T2}\"}}"
        )
    );

    // Unstage: both NULL, one `proposal.unstaged`.
    assert!(
        queue
            .unstage_from("PR-0003", &replaced.seen(), T3)
            .expect("unstaged")
    );
    assert_eq!(stage_columns(&db, "PR-0003"), "|");
    let cleared = queue.get("PR-0003").unwrap().unwrap();
    assert_eq!(cleared.staged, None);
    assert_eq!(cleared.updated_at, T3);
    let events = events_after(&db, seq);
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(events[2], "proposal.unstaged|{\"id\":\"PR-0003\"}");

    // Nothing staged: Ok(false), no write, no event.
    let later = "2026-10-07T10:00:00Z";
    assert!(
        !queue
            .unstage_from("PR-0003", &cleared.seen(), later)
            .expect("nothing to unstage")
    );
    assert_eq!(queue.get("PR-0003").unwrap().unwrap().updated_at, T3);
    assert_eq!(events_after(&db, seq).len(), 3, "no event");

    // An update's approve with its span hash; a section-form create too.
    let target = queue.get("PR-0001").unwrap().unwrap();
    let span = format!("b3:{}", blake3_hex(b"span"));
    queue
        .stage_from(
            "PR-0001",
            &target.seen(),
            &approve_stage(None, None, Some(&span)),
            T1,
        )
        .expect("an update staged with its span hash");
    assert_eq!(
        stage_columns(&db, "PR-0001"),
        format!(
            "{{\"decision\":\"approve\",\"option\":null,\"answer\":null,\"canon\":null,\
             \"note\":null,\"span_hash\":\"{span}\"}}|{T1}"
        )
    );
    // A bad clock: refused, nothing written.
    let asked = queue.get("PR-0002").unwrap().unwrap();
    match queue.stage_from("PR-0002", &asked.seen(), &reject_stage("no"), "now") {
        Err(QueueError::Invalid(_)) => {}
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(stage_columns(&db, "PR-0002"), "|");
}

/// What the store refuses to stage, nothing written, no event: a flag of a
/// kind that takes none, a span hash on a kind without a span (a question,
/// a create's new file), a blank reason, a state other than `open`, an
/// unknown ID.
#[test]
fn a_stage_the_kind_or_state_cannot_hold_is_refused_unwritten() {
    let scratch = Scratch::new("qst-refused");
    let db = scratch.db("q");
    let mut queue = three(&db);
    let made = queue
        .create(&new_file("docs/records/R/R-13.md", "R-13"), T0)
        .expect("a new file");
    assert_eq!(made.id, "PR-0004");
    let seq = last_seq(&db);
    let flagged = |option: Option<u64>, answer: Option<&str>, canon: Option<&str>| Stage::Approve {
        option,
        answer: answer.map(str::to_owned),
        canon: canon.map(str::to_owned),
        note: None,
        span_hash: None,
    };
    let span = Some("b3:00".to_owned());
    let cases: Vec<(&str, Stage, &str)> = vec![
        ("PR-0001", flagged(Some(0), None, None), "option"),
        ("PR-0001", flagged(None, Some("yes"), None), "answer"),
        ("PR-0001", flagged(None, None, Some("MEC-STAMINA")), "canon"),
        ("PR-0004", flagged(Some(1), None, None), "option"),
        ("PR-0002", flagged(Some(0), None, None), "option"),
        ("PR-0003", flagged(None, Some("yes"), None), "answer"),
        (
            "PR-0002",
            Stage::Approve {
                option: None,
                answer: Some("yes".to_owned()),
                canon: None,
                note: None,
                span_hash: span.clone(),
            },
            "span_hash",
        ),
        (
            "PR-0004",
            Stage::Approve {
                option: None,
                answer: None,
                canon: None,
                note: None,
                span_hash: span,
            },
            "span_hash",
        ),
        ("PR-0002", reject_stage("  "), "blank reason"),
    ];
    for (id, stage, named) in cases {
        let seen = queue.get(id).unwrap().unwrap().seen();
        match queue.stage_from(id, &seen, &stage, T1) {
            Err(QueueError::Invalid(message)) => {
                assert!(
                    message.contains(id) && message.contains(named),
                    "{id} {stage:?}: {message}"
                );
            }
            other => panic!("{id} {stage:?}: expected Invalid, got {other:?}"),
        }
        assert_eq!(stage_columns(&db, id), "|", "{id}: nothing written");
    }
    assert_eq!(events_after(&db, seq), Vec::<String>::new(), "no event");

    // Not open: refused (`Status`), for stage and unstage alike.
    let open = queue.get("PR-0002").unwrap().unwrap();
    let rejected = queue
        .reject_from("PR-0002", &open.seen(), &owner(None), T1)
        .expect("rejected");
    let seq = last_seq(&db);
    match queue.stage_from("PR-0002", &rejected.seen(), &reject_stage("again"), T2) {
        Err(QueueError::Status { status, .. }) => assert_eq!(status, ProposalStatus::Rejected),
        other => panic!("expected Status, got {other:?}"),
    }
    match queue.unstage_from("PR-0002", &rejected.seen(), T2) {
        Err(QueueError::Status { status, .. }) => assert_eq!(status, ProposalStatus::Rejected),
        other => panic!("expected Status, got {other:?}"),
    }
    match queue.stage_from("PR-0099", &rejected.seen(), &reject_stage("x"), T2) {
        Err(QueueError::Unknown { id }) => assert_eq!(id, "PR-0099"),
        other => panic!("expected Unknown, got {other:?}"),
    }
    assert_eq!(stage_columns(&db, "PR-0002"), "|");
    assert_eq!(events_after(&db, seq), Vec::<String>::new(), "no event");
}

// ------------------------------------------------------- leaving open

/// AC-12: approve, approve_from, approve_record_from, applied_with (a
/// completion of an `open` proposal), reject, reject_from, reject_orphan
/// each clear both columns, no `proposal.unstaged`; `.approved` and
/// `.rejected` carry `staged_at` exactly when the decision confirms the
/// stage; a compare-and-set op given a `staged_at` the row does not hold
/// is refused, nothing written.
#[test]
fn ac12_leaving_open_clears_the_stage_without_an_unstaged_event() {
    let scratch = Scratch::new("qst-leave");
    type Leave = fn(&mut SqliteQueue, &str, &specengine_store::Seen, &Decision) -> ProposalStatus;
    let ops: Vec<(&str, Leave, &str)> = vec![
        (
            "approve",
            |queue, id, _, decision| queue.approve(id, decision, T2).unwrap().status,
            "proposal.approved",
        ),
        (
            "approve_from",
            |queue, id, seen, decision| queue.approve_from(id, seen, decision, T2).unwrap().status,
            "proposal.approved",
        ),
        (
            "applied_with",
            |queue, id, _, decision| {
                queue
                    .applied_with(id, &"c".repeat(40), decision, T2)
                    .unwrap()
                    .status
            },
            "proposal.approved",
        ),
        (
            "reject",
            |queue, id, _, decision| queue.reject(id, decision, T2).unwrap().status,
            "proposal.rejected",
        ),
        (
            "reject_from",
            |queue, id, seen, decision| queue.reject_from(id, seen, decision, T2).unwrap().status,
            "proposal.rejected",
        ),
        (
            "reject_orphan",
            |queue, id, seen, decision| queue.reject_orphan(id, seen, decision, T2).unwrap().status,
            "proposal.rejected",
        ),
    ];
    for (index, (name, op, event)) in ops.into_iter().enumerate() {
        for confirms in [true, false] {
            let db = scratch.db(&format!("leave-{index}-{confirms}"));
            let mut queue = three(&db);
            let stage = if event == "proposal.rejected" {
                reject_stage("not wanted")
            } else {
                approve_stage(None, Some("fine"), Some("b3:00"))
            };
            let read = queue.get("PR-0001").unwrap().unwrap();
            let staged = queue
                .stage_from("PR-0001", &read.seen(), &stage, T1)
                .expect("staged");
            let seq = last_seq(&db);
            let decision = owner(confirms.then_some(T1));
            let status = op(&mut queue, "PR-0001", &staged.seen(), &decision);
            assert_ne!(status, ProposalStatus::Open, "{name}");
            assert_eq!(stage_columns(&db, "PR-0001"), "|", "{name}: both NULL");
            let after = queue.get("PR-0001").unwrap().unwrap();
            assert_eq!(after.staged, None, "{name}");
            let events = events_after(&db, seq);
            assert!(
                events
                    .iter()
                    .all(|line| !line.starts_with("proposal.unstaged")),
                "{name}: no unstaged event: {events:?}"
            );
            let decided = events
                .iter()
                .find(|line| line.starts_with(event))
                .unwrap_or_else(|| panic!("{name}: no {event}: {events:?}"));
            let payload: serde_json::Value =
                serde_json::from_str(decided.split_once('|').unwrap().1).unwrap();
            assert_eq!(
                payload.get("staged_at").and_then(|at| at.as_str()),
                confirms.then_some(T1),
                "{name}: {decided}"
            );
        }
    }

    // A compare-and-set op told it confirms a stage the row does not hold
    // (none, or another time): refused, nothing written.
    let db = scratch.db("mismatch");
    let mut queue = three(&db);
    let read = queue.get("PR-0001").unwrap().unwrap();
    let staged = queue
        .stage_from(
            "PR-0001",
            &read.seen(),
            &approve_stage(None, None, None),
            T1,
        )
        .expect("staged");
    let seq = last_seq(&db);
    let other = owner(Some("2026-10-07T09:14:03Z"));
    match queue.approve_from("PR-0001", &staged.seen(), &other, T2) {
        Err(QueueError::Invalid(message)) => assert!(message.contains("PR-0001"), "{message}"),
        other => panic!("expected Invalid, got {other:?}"),
    }
    match queue.reject_from("PR-0001", &staged.seen(), &other, T2) {
        Err(QueueError::Invalid(message)) => assert!(message.contains("PR-0001"), "{message}"),
        other => panic!("expected Invalid, got {other:?}"),
    }
    let asked = queue.get("PR-0002").unwrap().unwrap();
    match queue.reject_from("PR-0002", &asked.seen(), &owner(Some(T1)), T2) {
        Err(QueueError::Invalid(_)) => {}
        other => panic!("nothing staged: expected Invalid, got {other:?}"),
    }
    assert_eq!(events_after(&db, seq), Vec::<String>::new());
    assert_eq!(queue.get("PR-0001").unwrap().unwrap(), staged);
    // A compare-and-set read before the stage loses: the stage is part of
    // the state read (`Seen`).
    match queue.approve_from("PR-0001", &read.seen(), &owner(None), T2) {
        Err(QueueError::Changed { .. }) => {}
        other => panic!("expected Changed, got {other:?}"),
    }
}

// --------------------------------------------------------- corrupt rows

/// A corrupt-row case: its label, the ID, the edits (`column`, value), the
/// column the refusal names.
type CorruptCase<'a> = (&'a str, &'a str, Vec<(&'a str, Option<String>)>, &'a str);

/// AC-12: a stage on a proposal that is not `open`, one column `NULL`
/// alone, `staged` of another shape (a key missing, added, reordered; a
/// value of another type; not compact), `staged_at` no UTC time, a flag of
/// a kind that takes none, a span hash on a kind without a span, a blank
/// reason: corrupt, named. Each row restored alone into a fresh queue:
/// `get` and `list` fail naming the ID and the column, `list_readable`
/// skips it naming it. The good rows read.
#[test]
fn ac12_a_corrupt_stage_row_names_its_column() {
    let scratch = Scratch::new("qst-corrupt");
    let db = scratch.db("q");
    let mut queue = three(&db);
    let span = format!("b3:{}", blake3_hex(b"span"));
    for (id, stage) in [
        ("PR-0001", approve_stage(None, Some("n"), Some(&span))),
        ("PR-0002", reject_stage("asked before")),
        ("PR-0003", approve_stage(Some(1), Some("keep"), None)),
    ] {
        let seen = queue.get(id).unwrap().unwrap().seen();
        queue.stage_from(id, &seen, &stage, T1).expect("staged");
    }
    let state = queue.stored_rows().expect("rows");
    drop(queue);
    let row_of = |id: &str| {
        let mut one = state.clone();
        one.proposals.retain(|row| row.id() == Some(id));
        one.events.clear();
        one
    };
    let staged = |text: &str| Some(text.to_owned());
    let approve = |body: &str| format!("{{\"decision\":\"approve\",{body}}}");
    let cases: Vec<CorruptCase<'_>> = vec![
        (
            "staged on an approved row",
            "PR-0001",
            vec![
                ("status", staged("approved")),
                ("decided_by", staged("Ann <a@example.org>")),
                ("decided_at", staged(T2)),
            ],
            "staged",
        ),
        (
            "staged on a rejected row",
            "PR-0002",
            vec![
                ("status", staged("rejected")),
                ("decided_by", staged("Ann <a@example.org>")),
                ("decided_at", staged(T2)),
                ("decision_note", staged("No.")),
            ],
            "staged",
        ),
        (
            "staged NULL alone",
            "PR-0003",
            vec![("staged", None)],
            "staged",
        ),
        (
            "staged_at NULL alone",
            "PR-0003",
            vec![("staged_at", None)],
            "staged_at",
        ),
        (
            "staged_at no UTC time",
            "PR-0003",
            vec![("staged_at", staged("2026-10-07 09:14:02"))],
            "staged_at",
        ),
        (
            "a key missing",
            "PR-0003",
            vec![(
                "staged",
                Some(approve(
                    "\"option\":1,\"answer\":null,\"canon\":null,\"note\":\"keep\"",
                )),
            )],
            "staged",
        ),
        (
            "a key added",
            "PR-0003",
            vec![(
                "staged",
                Some(approve(
                    "\"option\":1,\"answer\":null,\"canon\":null,\"note\":\"keep\",\
                     \"span_hash\":null,\"consent\":true",
                )),
            )],
            "staged",
        ),
        (
            "keys reordered",
            "PR-0003",
            vec![(
                "staged",
                staged(
                    "{\"option\":1,\"decision\":\"approve\",\"answer\":null,\"canon\":null,\
                     \"note\":\"keep\",\"span_hash\":null}",
                ),
            )],
            "staged",
        ),
        (
            "an option of another type",
            "PR-0003",
            vec![(
                "staged",
                Some(approve(
                    "\"option\":\"1\",\"answer\":null,\"canon\":null,\"note\":\"keep\",\
                     \"span_hash\":null",
                )),
            )],
            "staged",
        ),
        (
            "not compact",
            "PR-0002",
            vec![(
                "staged",
                staged("{\"decision\": \"reject\", \"reason\": \"asked before\"}"),
            )],
            "staged",
        ),
        (
            "another decision",
            "PR-0002",
            vec![(
                "staged",
                staged("{\"decision\":\"defer\",\"reason\":\"later\"}"),
            )],
            "staged",
        ),
        (
            "an option on a question",
            "PR-0002",
            vec![(
                "staged",
                Some(approve(
                    "\"option\":0,\"answer\":null,\"canon\":null,\"note\":null,\
                     \"span_hash\":null",
                )),
            )],
            "staged",
        ),
        (
            "a canon on an update",
            "PR-0001",
            vec![(
                "staged",
                Some(approve(&format!(
                    "\"option\":null,\"answer\":null,\"canon\":\"MEC-STAMINA\",\"note\":null,\
                     \"span_hash\":\"{span}\""
                ))),
            )],
            "staged",
        ),
        (
            "an answer on a discrepancy",
            "PR-0003",
            vec![(
                "staged",
                Some(approve(
                    "\"option\":1,\"answer\":\"yes\",\"canon\":null,\"note\":null,\
                     \"span_hash\":null",
                )),
            )],
            "staged",
        ),
        (
            "a span hash on a question",
            "PR-0002",
            vec![(
                "staged",
                Some(approve(&format!(
                    "\"option\":null,\"answer\":\"yes\",\"canon\":null,\"note\":null,\
                     \"span_hash\":\"{span}\""
                ))),
            )],
            "staged",
        ),
        (
            "a blank staged reason",
            "PR-0002",
            vec![(
                "staged",
                staged("{\"decision\":\"reject\",\"reason\":\" \"}"),
            )],
            "staged",
        ),
    ];
    for (index, (label, id, edits, column)) in cases.into_iter().enumerate() {
        let mut one = row_of(id);
        for (name, value) in edits {
            one.proposals[0].columns[column_at(name)] = value;
        }
        let broken_db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&broken_db, PROJECT).expect("open");
        broken.restore(&one).expect("restore takes rows raw");
        let message = match broken.get(id) {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        assert!(
            message.contains(id) && message.contains(&format!("`{column}`")),
            "{label}: {message}"
        );
        assert!(
            broken.list(&ProposalFilter::default()).is_err(),
            "{label}: list fails"
        );
        let listed = broken
            .list_readable(&ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].id, id, "{label}");
        assert_eq!(listed.unreadable[0].column, column, "{label}");
    }
    // The staged rows themselves read, each its stage.
    let mut good = SqliteQueue::open(scratch.db("good"), PROJECT).expect("open");
    good.restore(&state).expect("restore");
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        let read = good.get(id).unwrap().unwrap();
        assert_eq!(
            read.staged.map(|staged| staged.at).as_deref(),
            Some(T1),
            "{id}"
        );
    }
}

// ------------------------------------------------------------- backup

/// AC-12: `dump`, `restore` into a fresh queue, `dump`: byte-identical
/// with three stages, the stage columns in the dump; `stored_rows` equal.
/// M: export drops `staged`.
#[test]
fn ac12_dump_restore_dump_keeps_the_stages_byte_for_byte() {
    let scratch = Scratch::new("qst-backup");
    let db = scratch.db("q");
    let mut queue = three(&db);
    let span = format!("b3:{}", blake3_hex(b"span"));
    for (id, stage) in [
        (
            "PR-0001",
            approve_stage(None, Some("ok \"quoted\""), Some(&span)),
        ),
        ("PR-0002", reject_stage("asked before")),
        ("PR-0003", approve_stage(Some(1), None, None)),
    ] {
        let seen = queue.get(id).unwrap().unwrap().seen();
        queue.stage_from(id, &seen, &stage, T1).expect("staged");
    }
    let state = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    drop(queue);
    assert_eq!(
        state
            .proposals
            .iter()
            .filter(|row| row.columns[column_at("staged")].is_some()
                && row.columns[column_at("staged_at")].as_deref() == Some(T1))
            .count(),
        3,
        "the three stages read raw"
    );
    // Each proposal line of the dump ends with its stage's two columns.
    let lines: Vec<&str> = dump
        .lines()
        .filter(|line| line.starts_with("proposals\t"))
        .collect();
    assert_eq!(lines.len(), 3, "{dump}");
    for (line, stage) in lines.iter().zip([
        approve_stage(None, Some("ok \"quoted\""), Some(&span)),
        reject_stage("asked before"),
        approve_stage(Some(1), None, None),
    ]) {
        let tail = format!(",{},\"{T1}\"]", serde_json::Value::String(stage.to_json()));
        assert!(line.ends_with(&tail), "{line} ends with {tail}");
    }

    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("open");
    fresh.restore(&state).expect("restore");
    assert_eq!(fresh.dump().expect("dump"), dump, "byte-identical");
    assert_eq!(fresh.stored_rows().expect("rows"), state);
    let report = fresh.get("PR-0003").unwrap().unwrap();
    assert_eq!(
        report.staged,
        Some(StagedChoice {
            stage: approve_stage(Some(1), None, None),
            at: T1.to_owned()
        })
    );
}

/// AC-12: `stored_rows` reads `tasks`, `runs` at schema 5 as at 4 (`>=`,
/// not `==`): a schema-5 queue's task is exported. M: `stored_rows` keeps
/// `== 4`.
#[test]
fn ac12_stored_rows_reads_the_tasks_of_a_schema_5_queue() {
    let scratch = Scratch::new("qst-tasks");
    let db = scratch.db("q");
    let mut queue = three(&db);
    let task = queue
        .create_task(
            &NewTask {
                git_common_dir: "/r/.git".to_owned(),
                title: Some("Tune stamina".to_owned()),
                goal: None,
                targets: vec!["MEC-STAMINA".to_owned()],
                author: Author::human(),
            },
            T0,
        )
        .expect("a task");
    let state: StoredQueue = queue.stored_rows().expect("rows");
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "5");
    assert_eq!(state.tasks.len(), 1, "the schema-5 queue's task: {state:?}");
    assert_eq!(state.tasks[0].id(), Some(task.id.as_str()));
    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("open");
    fresh.restore(&state).expect("restore");
    assert_eq!(fresh.dump().unwrap(), queue.dump().unwrap());
}
