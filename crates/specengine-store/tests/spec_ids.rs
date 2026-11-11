//! docs/features/spec-ids.md (RM-0073), the store's half.
//! "Data", "The constant and the side": `id_side(dir)` is `Hub` when the data directory
//! holds a readable `proposal-floor`, whatever number it holds, `Terminal`
//! with none, `read_floor`'s error for an invalid one. "Data", "Records":
//! `next_record` and `approve_record_from` count only the issued
//! `record_id`s on the queue's side, then take `IdSide::next`; a terminal
//! past 100000 has no next number, `QueueError::Invalid` with the line of
//! "Records", nothing written.
//!
//! "Acceptance criteria" AC-3 (b), (c) and (e), the store's part. Mutations:
//! M1 (the hub's `next_number` without the range), M4 (the queue's issued IDs
//! not filtered by side).
//!
//! Through the public API only, each queue in its own data directory under a
//! scratch directory (the floor is that directory's `proposal-floor`, written
//! by `store_floor` as a hub's start writes it).

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::Scratch;
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::{Author, HUB_PROPOSAL_FLOOR};
use specengine_core::record::IdSide;
use specengine_store::{
    Choice, Decision, Intake, NewIntake, PROPOSAL_COLUMNS, PROPOSAL_FLOOR_FILE, Place,
    ProposalKind, ProposalQueue as _, QueueError, RecordApproval, RecordSeries, SqliteQueue,
    StoredQueue, id_side, read_floor, store_floor,
};

const T0: &str = "2026-11-04T09:00:00Z";
const T1: &str = "2026-11-04T12:00:00Z";
const PROJECT: &str = "demo";

/// The line of an invalid floor, after `<data dir>/proposal-floor: `.
const NOT_A_FLOOR: &str = "not a proposal floor (a decimal number from 1, then LF)";

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn discrepancy(summary: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/x.md".to_owned(),
        place: place(),
        author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: vec!["A-1".to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "src/a.rs".to_owned(),
                qpath: None,
                lines: Some("3-9".to_owned()),
                observed: "walks".to_owned(),
                documented: "rests".to_owned(),
            }],
            options: vec![option("code"), option("spec")],
            recommendation: Some(1),
            distinct_from: Vec::new(),
            why: None,
        },
        actor: None,
        card_id: None,
    }
}

fn series(prefix: &str, width: u32, corpus_max: u64) -> RecordSeries {
    RecordSeries {
        prefix: prefix.to_owned(),
        width,
        corpus_max,
    }
}

fn dec(corpus_max: u64) -> RecordSeries {
    series("DEC", 4, corpus_max)
}

fn decision() -> Decision {
    Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: None,
        staged_at: None,
    }
}

fn approval(preview: &str, corpus_max: u64) -> RecordApproval {
    RecordApproval {
        series: dec(corpus_max),
        preview: preview.to_owned(),
        path: format!("docs/records/DEC/{preview}.md"),
        title: "Spec".to_owned(),
        text: format!("---\nid: {preview}\n---\n"),
        choice: Choice::Option(1),
    }
}

/// A data directory `name` of the scratch, with no floor.
fn terminal_dir(scratch: &Scratch, name: &str) -> PathBuf {
    let dir = scratch.join(name);
    fs::create_dir_all(&dir).expect("a data directory");
    dir
}

/// A data directory `name` of the scratch holding `floor` as a hub's start
/// stores it.
fn hub_dir(scratch: &Scratch, name: &str, floor: u64) -> PathBuf {
    let dir = terminal_dir(scratch, name);
    assert_eq!(store_floor(&dir, floor).expect("the floor stored"), floor);
    dir
}

fn queue_in(dir: &Path) -> SqliteQueue {
    SqliteQueue::open(dir.join("q.db"), PROJECT).expect("the queue opens")
}

/// The rows of a queue holding one discrepancy approved with the record
/// `record` (its `record_id`, `record_path` and `record_text` set to it), as
/// a dump of another data directory restores them: issued on a terminal as
/// `DEC-0024`, then the record's three columns rewritten.
fn issued_rows(scratch: &Scratch, record: &str) -> StoredQueue {
    let dir = terminal_dir(scratch, &format!("issued-{record}"));
    let mut queue = queue_in(&dir);
    let item = queue
        .create_intake(&discrepancy("It departs."), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    queue
        .approve_record_from(
            &item.id,
            &item.seen(),
            &approval("DEC-0024", 23),
            &decision(),
            T1,
        )
        .expect("issued on a terminal");
    let mut rows = queue.stored_rows().expect("rows");
    let at = |name: &str| {
        PROPOSAL_COLUMNS
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    let row = &mut rows.proposals[0].columns;
    row[at("record_id")] = Some(record.to_owned());
    row[at("record_path")] = Some(format!("docs/records/DEC/{record}.md"));
    row[at("record_text")] = Some(format!("---\nid: {record}\n---\n"));
    rows
}

/// A queue in `dir` holding `rows`, restored into it empty.
fn restored(dir: &Path, rows: &StoredQueue) -> SqliteQueue {
    let mut queue = queue_in(dir);
    queue.restore(rows).expect("restored");
    queue
}

// ------------------------------------------------------------ the side

/// docs/features/spec-ids.md "Data", "The constant and the side" (D2, I2):
/// no file `Terminal`; `100001` and LF `Hub`; `5` and LF `Hub` too, the
/// stored number never the side; `x` and LF, an empty file and a directory
/// at the name are `read_floor`'s error naming the file. Reading the side
/// writes nothing.
#[test]
fn the_side_is_the_floor_files_presence_alone() {
    let scratch = Scratch::new("si-side");
    let none = terminal_dir(&scratch, "none");
    assert_eq!(id_side(&none).expect("no floor"), IdSide::Terminal);
    assert!(!none.join(PROPOSAL_FLOOR_FILE).exists(), "nothing written");
    assert!(fs::read_dir(&none).expect("listed").next().is_none());

    let hub = hub_dir(&scratch, "hub", HUB_PROPOSAL_FLOOR);
    assert_eq!(id_side(&hub).expect("a floor"), IdSide::Hub);
    let five = hub_dir(&scratch, "five", 5);
    assert_eq!(read_floor(&five).expect("five"), 5);
    assert_eq!(id_side(&five).expect("a floor of 5"), IdSide::Hub);

    for (name, bytes) in [("x", b"x\n".as_slice()), ("empty", b"".as_slice())] {
        let dir = terminal_dir(&scratch, name);
        fs::write(dir.join(PROPOSAL_FLOOR_FILE), bytes).expect("written");
        let error = id_side(&dir).expect_err(name).to_string();
        let floor_error = read_floor(&dir).expect_err(name).to_string();
        assert_eq!(error, floor_error, "{name}: read_floor's error");
        assert!(
            error.contains(PROPOSAL_FLOOR_FILE) && error.contains(NOT_A_FLOOR),
            "{name}: {error}"
        );
        assert_eq!(
            fs::read(dir.join(PROPOSAL_FLOOR_FILE)).expect("kept"),
            bytes,
            "{name}: the file as it was"
        );
    }
    let directory = terminal_dir(&scratch, "directory");
    fs::create_dir_all(directory.join(PROPOSAL_FLOOR_FILE)).expect("a directory");
    let error = id_side(&directory).expect_err("a directory").to_string();
    assert!(error.contains(NOT_A_FLOOR), "{error}");
}

// ------------------------------------------------------------ AC-3 (b)

/// docs/features/spec-ids.md "Acceptance criteria" AC-3 (b): with a floor,
/// `next_record` gives `DEC-100001` with `corpus_max` 0 and a restored row
/// issued `DEC-0030`, and `DEC-100008` with a row issued `DEC-100007`; a
/// spec-a-shaped `R` of width 2 gives `R-100001`. A floor of 5 mints spec
/// IDs from 100001 all the same (D1: the constant, never the stored
/// number). M1, M4.
#[test]
fn ac3b_a_hub_mints_its_records_from_the_start() {
    let scratch = Scratch::new("si-hub");
    let low = restored(
        &hub_dir(&scratch, "low", HUB_PROPOSAL_FLOOR),
        &issued_rows(&scratch, "DEC-0030"),
    );
    let dump = low.dump().expect("dump");
    assert_eq!(low.next_record(&dec(0)).unwrap(), "DEC-100001");
    assert_eq!(low.next_record(&dec(100_003)).unwrap(), "DEC-100004");
    assert_eq!(low.dump().expect("dump"), dump, "a preview writes nothing");

    let high = restored(
        &hub_dir(&scratch, "high", HUB_PROPOSAL_FLOOR),
        &issued_rows(&scratch, "DEC-100007"),
    );
    assert_eq!(high.next_record(&dec(0)).unwrap(), "DEC-100008");
    assert_eq!(high.next_record(&dec(100_010)).unwrap(), "DEC-100011");
    assert_eq!(
        high.next_record(&series("R", 2, 0)).unwrap(),
        "R-100001",
        "another prefix: none of its own issued"
    );

    let empty = queue_in(&hub_dir(&scratch, "empty", HUB_PROPOSAL_FLOOR));
    assert_eq!(empty.next_record(&series("R", 2, 0)).unwrap(), "R-100001");
    assert_eq!(empty.next_record(&series("X", 8, 0)).unwrap(), "X-00100001");
    assert_eq!(empty.next_record(&series("N", 1, 0)).unwrap(), "N-100001");

    let five = queue_in(&hub_dir(&scratch, "five", 5));
    assert_eq!(five.next_record(&dec(0)).unwrap(), "DEC-100001");
}

/// docs/features/spec-ids.md "Data", "Records": the issue at step 7 under
/// the write lock recomputes the number on the side, so a terminal's
/// preview on a hub's queue (its corpus maximum the side's, 0) is `Issued`
/// naming the hub's ID, nothing written; the hub's preview issues it, and
/// the next is one more.
#[test]
fn a_hubs_issue_recomputes_on_its_side() {
    let scratch = Scratch::new("si-issue");
    let dir = hub_dir(&scratch, "hub", HUB_PROPOSAL_FLOOR);
    let mut queue = queue_in(&dir);
    let item = queue
        .create_intake(&discrepancy("It departs."), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    assert_eq!(item.id, "PR-100001", "the floor's PR- range, unchanged");
    let dump = queue.dump().expect("dump");
    match queue.approve_record_from(
        &item.id,
        &item.seen(),
        &approval("DEC-0024", 0),
        &decision(),
        T1,
    ) {
        Err(QueueError::Issued { next }) => assert_eq!(next, "DEC-100001"),
        other => panic!("expected Issued, got {other:?}"),
    }
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    let approved = queue
        .approve_record_from(
            &item.id,
            &item.seen(),
            &approval("DEC-100001", 0),
            &decision(),
            T1,
        )
        .expect("issued");
    assert_eq!(approved.record.expect("the record").id, "DEC-100001");
    assert_eq!(queue.next_record(&dec(0)).unwrap(), "DEC-100002");
}

/// docs/features/spec-ids.md "Data", "Records" (I3), the contract of
/// `RecordSeries.corpus_max`: the caller passes the side's corpus maximum
/// (core `highest_record_number(.., side)`). One off the queue's side means
/// the floor came or went between the caller's read of the side and the
/// queue's: `QueueError::Invalid` naming the file, nothing written, on the
/// preview and on the issue, a hub's queue and a terminal's alike (the
/// developer's iteration 2).
#[test]
fn a_corpus_maximum_off_the_queues_side_is_refused_and_nothing_written() {
    let scratch = Scratch::new("si-off-side");
    for (name, floor, corpus_max, preview) in [
        ("hub", true, 23, "DEC-100001"),
        ("terminal", false, 100_003, "DEC-0001"),
    ] {
        let dir = if floor {
            hub_dir(&scratch, name, HUB_PROPOSAL_FLOOR)
        } else {
            terminal_dir(&scratch, name)
        };
        let mut queue = queue_in(&dir);
        let item = queue
            .create_intake(&discrepancy("It departs."), &[], None, T0)
            .expect("intake")
            .created
            .expect("stored");
        let dump = queue.dump().expect("dump");
        let refused = |result: Result<String, QueueError>| match result {
            Err(QueueError::Invalid(message)) => message,
            other => panic!("{name}: expected Invalid, got {other:?}"),
        };
        let message = refused(queue.next_record(&dec(corpus_max)));
        for fragment in [
            format!("`DEC`'s highest number {corpus_max} "),
            format!("`{PROPOSAL_FLOOR_FILE}`"),
            "run it again".to_owned(),
        ] {
            assert!(message.contains(&fragment), "{name}: {fragment}: {message}");
        }
        let issued = queue
            .approve_record_from(
                &item.id,
                &item.seen(),
                &approval(preview, corpus_max),
                &decision(),
                T1,
            )
            .map(|proposal| proposal.id);
        assert_eq!(refused(issued), message, "{name}: the issue alike");
        assert_eq!(queue.dump().expect("dump"), dump, "{name}: nothing written");
    }
}

// ------------------------------------------------------------ AC-3 (c)

/// docs/features/spec-ids.md "Acceptance criteria" AC-3 (c): with no floor,
/// `DEC-0025` with `corpus_max` 24 and a restored row issued `DEC-100004`;
/// with `corpus_max` 100000 the error of "Records", nothing written, the
/// preview's and the issue's alike. M4.
#[test]
fn ac3c_a_terminal_mints_below_the_start_and_stops_at_100000() {
    let scratch = Scratch::new("si-terminal");
    let dir = terminal_dir(&scratch, "terminal");
    let mut queue = restored(&dir, &issued_rows(&scratch, "DEC-100004"));
    assert_eq!(queue.next_record(&dec(24)).unwrap(), "DEC-0025");
    assert_eq!(queue.next_record(&dec(0)).unwrap(), "DEC-0001");
    assert_eq!(queue.next_record(&dec(99_999)).unwrap(), "DEC-100000");

    let line = "`DEC` numbers reach 100000, the last below a hub's 100001: no record ID follows";
    let dump = queue.dump().expect("dump");
    match queue.next_record(&dec(100_000)) {
        Err(QueueError::Invalid(message)) => assert_eq!(message, line),
        other => panic!("expected Invalid, got {other:?}"),
    }
    let item = queue
        .create_intake(&discrepancy("It departs again."), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    let dump_with_item = queue.dump().expect("dump");
    assert_ne!(dump_with_item, dump);
    match queue.approve_record_from(
        &item.id,
        &item.seen(),
        &approval("DEC-100001", 100_000),
        &decision(),
        T1,
    ) {
        Err(QueueError::Invalid(message)) => assert_eq!(message, line),
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(
        queue.dump().expect("dump"),
        dump_with_item,
        "nothing written"
    );

    // Another prefix's numbers past 100000 on the terminal: the same line,
    // its prefix named.
    let empty = queue_in(&terminal_dir(&scratch, "empty"));
    match empty.next_record(&series("R", 2, 100_000)) {
        Err(QueueError::Invalid(message)) => assert_eq!(
            message,
            "`R` numbers reach 100000, the last below a hub's 100001: no record ID follows"
        ),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

// ------------------------------------------------------------ AC-3 (e)

/// docs/features/spec-ids.md "Data", "The constant and the side" (D2) and
/// AC-3 (e), the store's part: an invalid `proposal-floor` is `read_floor`'s
/// error on the preview and on the issue, nothing written.
#[test]
fn ac3e_an_invalid_floor_mints_no_record() {
    let scratch = Scratch::new("si-invalid");
    let dir = terminal_dir(&scratch, "bad");
    let mut queue = queue_in(&dir);
    let item = queue
        .create_intake(&discrepancy("It departs."), &[], None, T0)
        .expect("intake")
        .created
        .expect("stored");
    fs::write(dir.join(PROPOSAL_FLOOR_FILE), b"x\n").expect("written");
    let dump = queue.dump().expect("dump");
    let error = queue
        .next_record(&dec(23))
        .expect_err("no side")
        .to_string();
    assert!(
        error.contains(PROPOSAL_FLOOR_FILE) && error.contains(NOT_A_FLOOR),
        "{error}"
    );
    let error = queue
        .approve_record_from(
            &item.id,
            &item.seen(),
            &approval("DEC-0024", 23),
            &decision(),
            T1,
        )
        .expect_err("no side")
        .to_string();
    assert!(error.contains(NOT_A_FLOOR), "{error}");
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
}
