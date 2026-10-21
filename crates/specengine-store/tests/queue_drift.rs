//! docs/features/drift-queue.md, the store's half (RD-1, RD-5's dedup,
//! RD-11's stage refusal, RD-12's backup; TE-4's matrix, TE-6):
//!
//! - RD-1: `ProposalKind::family()` puts every kind in exactly one
//!   `KindFamily`, `drift` the eighth kind and the only one that locks;
//!   `create`, `create_intake` and `approve_record_from` refuse a drift
//!   item (`Invalid`), only `create_drift` stores one.
//! - RD-5 / TE-4 (`docs/features/drift-queue.md` "Dedup"): an earlier item
//!   in {open, approved, applied, rejected} x {same, other evidence} x
//!   {same, other branch}: a live one is a hit (`same` telling the
//!   evidence), a rejected one with equal evidence settles (the newest
//!   named), an applied one never settles (named as reverted); another
//!   branch is another key. Two raises racing from two handles store one
//!   item.
//! - RD-11: a stage on a drift item is refused by the store (`Invalid`).
//! - RD-12 / TE-6 (`docs/features/drift-queue.md` "Queue schema 8"):
//!   genuine schema-6 and schema-7 databases (step 8's columns absent)
//!   step to 8 keeping their rows; a drift row with either column `NULL`,
//!   evidence off its shape, a bad hash or a stage, another kind with
//!   either column set: corrupt, named; a row's later keys (5b's `levels`,
//!   5c's `anchor`) are no defect; drift rows dump, restore and dump byte
//!   for byte.
//!
//! Through the public API; the raw reads and the schema surgery use the
//! system's `sqlite3` client. The named mutations are in each test's `M:`.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};

use common::Scratch;
use specengine_core::drift::{DriftEvidence, DriftLock, DriftMarker, DriftRow};
use specengine_core::intake::{DRIFT_KIND, IntakeSeverity, QUESTION_KIND};
use specengine_core::proposal::Author;
use specengine_core::record::Choice;
use specengine_store::{
    Decision, DriftResult, Intake, KindFamily, NewDrift, NewIntake, NewProposal, PROPOSAL_COLUMNS,
    Place, ProposalFilter, ProposalKind, ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION,
    QueueError, RecordApproval, RecordSeries, SqliteQueue, Stage, patch_hash, proposal_columns,
};

const T0: &str = "2026-10-14T09:00:00Z";
const T1: &str = "2026-10-14T09:10:00Z";
const T2: &str = "2026-10-14T09:20:00Z";
const T3: &str = "2026-10-14T09:30:00Z";
const PROJECT: &str = "code-a";
const COMMON: &str = "/r/.git";

fn place(branch: &str) -> Place {
    Place {
        git_common_dir: COMMON.to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: branch.to_owned(),
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
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: None,
        staged_at: None,
    }
}

/// One stored row of `spec verify --json`'s shape: `CAP`'s `code_ahead`.
fn row(node: &str) -> DriftRow {
    DriftRow {
        node: node.to_owned(),
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

/// The `drift` column of one row of `node`.
fn evidence(node: &str) -> String {
    serde_json::to_string(&DriftEvidence {
        rows: vec![row(node)],
        more: 0,
    })
    .expect("JSON")
}

/// A `drift_hash` of the digit `digit`.
fn hash(digit: char) -> String {
    format!("b3:{}", digit.to_string().repeat(64))
}

fn drift(node: &str, branch: &str, digit: char) -> NewDrift {
    NewDrift {
        target_id: node.to_owned(),
        target_path: "spec.lock".to_owned(),
        place: place(branch),
        author: agent(),
        task_id: None,
        severity: IntakeSeverity::Normal,
        summary: "code_ahead code_changed".to_owned(),
        evidence: evidence(node),
        hash: hash(digit),
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

fn column_at(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

/// The one item `create_drift` stored for `item`.
fn created(queue: &mut SqliteQueue, item: NewDrift, now: &str) -> String {
    match queue
        .create_drift(&[item], now)
        .expect("create_drift")
        .remove(0)
    {
        DriftResult::Created { proposal, .. } => proposal.id.clone(),
        other => panic!("expected a new item, got {other:?}"),
    }
}

fn invalid<T: std::fmt::Debug>(result: Result<T, QueueError>, context: &str) -> String {
    match result {
        Err(QueueError::Invalid(message)) => message,
        other => panic!("{context}: expected Invalid, got {other:?}"),
    }
}

// ------------------------------------------------------------------ RD-1

/// RD-1: every kind is in exactly one family, `drift` the eighth kind (its
/// name core's `DRIFT_KIND`) and the only one that locks; it adds no node
/// and amends nothing. M: `Drift` mapped to `Applies`; the old
/// `applies()`/`decides()` pin kept (a kind in no family or two).
#[test]
fn every_kind_has_exactly_one_family() {
    assert_eq!(DRIFT_KIND, "drift");
    assert_eq!(ProposalKind::ALL.len(), 8);
    assert_eq!(ProposalKind::ALL[7], ProposalKind::Drift, "appended last");
    assert_eq!(ProposalKind::Drift.as_str(), DRIFT_KIND);
    assert_eq!(ProposalKind::parse(DRIFT_KIND), Some(ProposalKind::Drift));
    assert_eq!(
        serde_json::to_string(&ProposalKind::Drift).expect("serialize"),
        "\"drift\""
    );
    let want = [
        (ProposalKind::Update, KindFamily::Applies),
        (ProposalKind::Question, KindFamily::Decides),
        (ProposalKind::Discrepancy, KindFamily::Decides),
        (ProposalKind::Create, KindFamily::Applies),
        (ProposalKind::Interpretation, KindFamily::Applies),
        (ProposalKind::Decision, KindFamily::Decides),
        (ProposalKind::Amendment, KindFamily::Applies),
        (ProposalKind::Drift, KindFamily::Locks),
    ];
    let got: Vec<(ProposalKind, KindFamily)> = ProposalKind::ALL
        .into_iter()
        .map(|kind| (kind, kind.family()))
        .collect();
    assert_eq!(got, want);
    for family in [KindFamily::Applies, KindFamily::Decides, KindFamily::Locks] {
        assert!(
            got.iter().any(|(_, found)| *found == family),
            "{family:?} names a kind"
        );
    }
    let locking: Vec<ProposalKind> = ProposalKind::ALL
        .into_iter()
        .filter(|kind| kind.family() == KindFamily::Locks)
        .collect();
    assert_eq!(locking, [ProposalKind::Drift]);
    assert!(!ProposalKind::Drift.adds_nodes());
    assert!(!ProposalKind::Drift.amends());
}

/// RD-1 ("Kind family"): `create`, `create_intake` and
/// `approve_record_from` refuse a drift item (`Invalid`), nothing written,
/// no event; only `create_drift` stores one. M: a drift taken by `create`.
#[test]
fn only_create_drift_stores_a_drift_item() {
    let scratch = Scratch::new("qdr-only");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let proposal = NewProposal {
        kind: ProposalKind::Drift,
        target_id: "LAW-BOUND".to_owned(),
        target_path: "spec.lock".to_owned(),
        place: place("main"),
        base_hash: None,
        base_text: None,
        new_text: "x".to_owned(),
        patch_hash: patch_hash("spec.lock", "", "x"),
        rationale: "lock it".to_owned(),
        author: agent(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    };
    invalid(queue.create(&proposal, T0), "create of a drift");
    let intake = NewIntake {
        kind: ProposalKind::Drift,
        target_path: "spec.lock".to_owned(),
        place: place("main"),
        author: agent(),
        intake: Intake {
            target_ids: vec!["LAW-BOUND".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: "drift".to_owned(),
            working_answer: Some("w".to_owned()),
            price_of_other: Some("p".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
        },
    };
    invalid(
        queue.create_intake(&intake, &[], None, T0),
        "create_intake of a drift",
    );
    assert!(queue.list(&ProposalFilter::default()).unwrap().is_empty());
    assert!(queue.events().unwrap().is_empty(), "no event");

    let id = created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    let stored = queue.get(&id).unwrap().unwrap();
    assert_eq!(stored.kind, ProposalKind::Drift);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.target_id, "LAW-BOUND");
    assert_eq!(stored.target_path, "spec.lock");
    let item = stored.drift.as_ref().expect("its evidence");
    assert_eq!(item.hash, hash('1'));
    assert_eq!(item.evidence.rows, [row("LAW-BOUND")]);
    assert_eq!(item.summary, "code_ahead code_changed");
    assert!(stored.intake.is_none() && stored.record.is_none());
    assert!(stored.interpretation.is_none() && stored.staged.is_none());
    // `approve_record_from` (a deciding kind's step 7) refuses it too.
    let seen = stored.seen();
    let approval = RecordApproval {
        series: RecordSeries {
            prefix: "DEC".to_owned(),
            width: 4,
            corpus_max: 0,
        },
        preview: "DEC-0001".to_owned(),
        path: "docs/records/DEC-0001.md".to_owned(),
        title: "A record".to_owned(),
        text: "---\nid: DEC-0001\n---\n".to_owned(),
        choice: Choice::WorkingAnswer,
    };
    invalid(
        queue.approve_record_from(&id, &seen, &approval, &owner(), T1),
        "approve_record_from of a drift",
    );
    assert_eq!(
        queue.get(&id).unwrap().unwrap().status,
        ProposalStatus::Open
    );
    // An intake kind's name is no drift's.
    assert_ne!(QUESTION_KIND, DRIFT_KIND);
}

// ------------------------------------------------------------------ RD-11

/// RD-11: the store refuses any stage on a drift item (`Invalid`, the
/// item unchanged, no event). M: `Stage::problem`'s drift refusal removed.
#[test]
fn drift_item_cannot_be_staged_by_the_store() {
    let scratch = Scratch::new("qdr-stage");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let id = created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    let events = queue.events().unwrap().len();
    for stage in [
        Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            editorial: false,
            note: None,
            span_hash: None,
        },
        Stage::Approve {
            option: None,
            answer: None,
            canon: None,
            editorial: true,
            note: Some("typo".to_owned()),
            span_hash: None,
        },
        Stage::Reject {
            reason: "not now".to_owned(),
        },
    ] {
        assert!(
            stage.problem(ProposalKind::Drift, false).is_some(),
            "{stage:?}"
        );
        let seen = queue.get(&id).unwrap().unwrap().seen();
        let message = invalid(queue.stage_from(&id, &seen, &stage, T1), "stage a drift");
        assert!(message.contains("drift"), "{message}");
    }
    let stored = queue.get(&id).unwrap().unwrap();
    assert!(stored.staged.is_none());
    assert_eq!(stored.updated_at, T0);
    assert_eq!(queue.events().unwrap().len(), events, "no event");
}

// ------------------------------------------------------------- RD-5, TE-4

/// The prior item of one TE-4 cell: stored on `main` with hash `1`, then
/// taken to `status`.
fn prior(queue: &mut SqliteQueue, status: ProposalStatus) -> String {
    let id = created(queue, drift("LAW-BOUND", "main", '1'), T0);
    match status {
        ProposalStatus::Open => {}
        ProposalStatus::Approved => {
            queue.approve(&id, &owner(), T1).expect("approved");
        }
        ProposalStatus::Applied => {
            queue.approve(&id, &owner(), T1).expect("approved");
            queue.applied(&id, &"c".repeat(40), T2).expect("applied");
        }
        ProposalStatus::Rejected => {
            let decision = Decision {
                note: Some("not drift".to_owned()),
                ..owner()
            };
            queue.reject(&id, &decision, T1).expect("rejected");
        }
    }
    assert_eq!(queue.get(&id).unwrap().unwrap().status, status);
    id
}

/// TE-4 / RD-5 ("Dedup"): the 16 cells {open, approved, applied, rejected}
/// x {same, other evidence} x {same, other branch}. Same branch: open and
/// approved are hits (`same` the hashes' equality), rejected with equal
/// evidence settles (a hit), with other evidence a new item; applied never
/// settles: a new item, named as reverted (its commit) when the evidence is
/// equal. Another branch: always a new item, nothing reverted. A hit writes
/// nothing (no row, no event). M: no dedup; settled ignored; `applied`
/// settling; branch dropped from the key.
#[test]
fn dedup_matrix_of_status_evidence_and_branch() {
    for status in [
        ProposalStatus::Open,
        ProposalStatus::Approved,
        ProposalStatus::Applied,
        ProposalStatus::Rejected,
    ] {
        for same_evidence in [true, false] {
            for same_branch in [true, false] {
                let label = format!(
                    "{status} x {} evidence x {} branch",
                    if same_evidence { "same" } else { "other" },
                    if same_branch { "same" } else { "other" }
                );
                let scratch = Scratch::new("qdr-matrix");
                let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
                let earlier = prior(&mut queue, status);
                let rows = queue.list(&ProposalFilter::default()).unwrap().len();
                let events = queue.events().unwrap().len();
                let item = drift(
                    "LAW-BOUND",
                    if same_branch { "main" } else { "t1" },
                    if same_evidence { '1' } else { '2' },
                );
                let result = queue
                    .create_drift(&[item], T3)
                    .unwrap_or_else(|error| panic!("{label}: {error}"))
                    .remove(0);
                let hit = same_branch
                    && (matches!(status, ProposalStatus::Open | ProposalStatus::Approved)
                        || (status == ProposalStatus::Rejected && same_evidence));
                if hit {
                    assert_eq!(
                        result,
                        DriftResult::Hit {
                            id: earlier.clone(),
                            status,
                            same: same_evidence,
                        },
                        "{label}"
                    );
                    assert_eq!(
                        queue.list(&ProposalFilter::default()).unwrap().len(),
                        rows,
                        "{label}: nothing stored"
                    );
                    assert_eq!(queue.events().unwrap().len(), events, "{label}: no event");
                    continue;
                }
                let DriftResult::Created { proposal, reverted } = result else {
                    panic!("{label}: expected a new item, got {result:?}");
                };
                assert_eq!(proposal.id, "PR-0002", "{label}");
                assert_eq!(proposal.status, ProposalStatus::Open, "{label}");
                let want_reverted =
                    (same_branch && same_evidence && status == ProposalStatus::Applied)
                        .then(|| (earlier.clone(), Some("c".repeat(40))));
                assert_eq!(reverted, want_reverted, "{label}");
                assert_eq!(
                    queue.events().unwrap().len(),
                    events + 1,
                    "{label}: one proposal.created"
                );
            }
        }
    }
}

/// RD-5: two rejected items of a key with equal evidence: the newest is
/// named; a live item wins over a settled one; another node or another
/// repository is another key. M: the oldest rejected named; the
/// repository dropped from the key.
#[test]
fn dedup_names_the_newest_and_keys_on_repository_branch_and_node() {
    let scratch = Scratch::new("qdr-newest");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let reject = |queue: &mut SqliteQueue, id: &str| {
        queue
            .reject(
                id,
                &Decision {
                    note: Some("no".to_owned()),
                    ..owner()
                },
                T1,
            )
            .expect("rejected");
    };
    let first = created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    reject(&mut queue, &first);
    // Another hash after it: a new item, then rejected too.
    let second = created(&mut queue, drift("LAW-BOUND", "main", '2'), T0);
    reject(&mut queue, &second);
    // Hash 1 again: a third, rejected with hash 1.
    let mut again = drift("LAW-BOUND", "main", '1');
    again.summary = "spec_ahead marker_behind".to_owned();
    let result = queue.create_drift(&[again], T2).unwrap().remove(0);
    assert_eq!(
        result,
        DriftResult::Hit {
            id: first.clone(),
            status: ProposalStatus::Rejected,
            same: true
        },
        "the only rejected item of hash 1"
    );
    // A later rejected item of hash 1: forced through another hash first.
    let third = created(&mut queue, drift("LAW-BOUND", "main", '3'), T2);
    reject(&mut queue, &third);
    sqlite3(
        &scratch.db("q"),
        &format!(
            "UPDATE proposals SET drift_hash = '{}' WHERE id = '{third}';",
            hash('1')
        ),
    );
    let result = queue
        .create_drift(&[drift("LAW-BOUND", "main", '1')], T3)
        .unwrap()
        .remove(0);
    assert_eq!(
        result,
        DriftResult::Hit {
            id: third.clone(),
            status: ProposalStatus::Rejected,
            same: true
        },
        "the newest rejected item named"
    );
    // A live item wins: an open one of hash 4 beside the settled ones.
    let open = created(&mut queue, drift("LAW-BOUND", "main", '4'), T3);
    let result = queue
        .create_drift(&[drift("LAW-BOUND", "main", '1')], T3)
        .unwrap()
        .remove(0);
    assert_eq!(
        result,
        DriftResult::Hit {
            id: open,
            status: ProposalStatus::Open,
            same: false
        }
    );
    // Another node, another repository: new items.
    let mut other_repository = drift("LAW-BOUND", "main", '4');
    other_repository.place.git_common_dir = "/elsewhere/.git".to_owned();
    let results = queue
        .create_drift(&[drift("GOAL-01", "main", '4'), other_repository], T3)
        .unwrap();
    assert!(
        results
            .iter()
            .all(|result| matches!(result, DriftResult::Created { reverted: None, .. })),
        "{results:?}"
    );
}

/// RD-5 ("Dedup": IDs in node byte order, one `proposal.created` each):
/// one call's items are stored in the order given, IDs rising; an item off
/// its shape refuses the whole call (`Invalid`), nothing written.
#[test]
fn one_raise_stores_its_items_in_order_or_nothing() {
    let scratch = Scratch::new("qdr-order");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let results = queue
        .create_drift(
            &[
                drift("GOAL-01", "main", '1'),
                drift("LAW-BOUND", "main", '2'),
                drift("PIECE-COUNTER", "main", '3'),
            ],
            T0,
        )
        .unwrap();
    let ids: Vec<(String, String)> = results
        .into_iter()
        .map(|result| match result {
            DriftResult::Created { proposal, .. } => (proposal.id.clone(), proposal.target_id),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        ids,
        [
            ("PR-0001".to_owned(), "GOAL-01".to_owned()),
            ("PR-0002".to_owned(), "LAW-BOUND".to_owned()),
            ("PR-0003".to_owned(), "PIECE-COUNTER".to_owned()),
        ]
    );
    assert_eq!(queue.events().unwrap().len(), 3);
    for (label, broken) in [
        ("a hash in upper case", {
            let mut item = drift("LAW-STEP", "main", '1');
            item.hash = format!("b3:{}", "A".repeat(64));
            item
        }),
        ("a hash without its prefix", {
            let mut item = drift("LAW-STEP", "main", '1');
            item.hash = "1".repeat(64);
            item
        }),
        ("evidence of no row", {
            let mut item = drift("LAW-STEP", "main", '1');
            item.evidence = "{\"rows\":[],\"more\":0}".to_owned();
            item
        }),
        ("evidence of an ok row", {
            let mut item = drift("LAW-STEP", "main", '1');
            item.evidence = item.evidence.replace("code_ahead", "ok");
            item
        }),
        ("a blank summary", {
            let mut item = drift("LAW-STEP", "main", '1');
            item.summary = " ".to_owned();
            item
        }),
    ] {
        invalid(
            queue.create_drift(&[drift("MORE-1", "main", '1'), broken], T1),
            label,
        );
        assert_eq!(
            queue.list(&ProposalFilter::default()).unwrap().len(),
            3,
            "{label}: nothing written"
        );
        assert_eq!(queue.events().unwrap().len(), 3, "{label}: no event");
    }
}

/// TE-4 / RD-5: two raises racing from two handles of one database (two
/// threads released together by a barrier) store one item; the other is a
/// hit on it. Self-terminating: each thread ends with its call. M: the
/// dedup read outside the inserting `BEGIN IMMEDIATE`.
#[test]
fn concurrent_raises_from_two_handles_store_one_item() {
    for round in 0..8 {
        let scratch = Scratch::new("qdr-race");
        let db = scratch.db("q");
        drop(SqliteQueue::open(&db, PROJECT).expect("open"));
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let db = db.clone();
                std::thread::spawn(move || {
                    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
                    barrier.wait();
                    queue
                        .create_drift(&[drift("LAW-BOUND", "main", '1')], T0)
                        .expect("create_drift")
                        .remove(0)
                })
            })
            .collect();
        let results: Vec<DriftResult> = handles
            .into_iter()
            .map(|handle| handle.join().expect("a raise"))
            .collect();
        let created = results
            .iter()
            .filter(|result| matches!(result, DriftResult::Created { .. }))
            .count();
        assert_eq!(created, 1, "round {round}: {results:?}");
        assert!(
            results.iter().any(|result| *result
                == DriftResult::Hit {
                    id: "PR-0001".to_owned(),
                    status: ProposalStatus::Open,
                    same: true
                }),
            "round {round}: {results:?}"
        );
        let queue = SqliteQueue::open(&db, PROJECT).expect("open");
        assert_eq!(queue.list(&ProposalFilter::default()).unwrap().len(), 1);
    }
}

// ------------------------------------------------------------------- TE-6

/// TE-6: genuine schema-6 and schema-7 databases (step 8's two columns
/// absent, a stored update and question) open as 8: `drift`, `drift_hash`
/// appended `TEXT`, the rows kept (`NULL` there), `user_version` 8, once;
/// `proposal_columns` reads 6 and 7 at 44 columns, 8 at 46, 9 none; a read
/// steps nothing. A drift item goes into the stepped queue. M: step 8
/// omitted; `proposal_columns` mapping 8 to 44.
#[test]
fn schema_6_and_7_step_to_8() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 8);
    assert_eq!(PROPOSAL_COLUMNS.len(), 46);
    assert_eq!(PROPOSAL_COLUMNS[44..], ["drift", "drift_hash"]);
    assert_eq!(proposal_columns(6), Some(&PROPOSAL_COLUMNS[..44]));
    assert_eq!(proposal_columns(7), Some(&PROPOSAL_COLUMNS[..44]));
    assert_eq!(proposal_columns(8), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(9), None);
    for version in [6, 7] {
        let scratch = Scratch::new(&format!("qdr-schema-{version}"));
        let db = scratch.db("q");
        let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let update = NewProposal {
            kind: ProposalKind::Update,
            target_id: "LAW-BOUND".to_owned(),
            target_path: "docs/spec/counter.md".to_owned(),
            place: place("main"),
            base_hash: Some("b3:base".to_owned()),
            base_text: Some("old\n".to_owned()),
            new_text: "new\n".to_owned(),
            patch_hash: patch_hash("LAW-BOUND", "old\n", "new\n"),
            rationale: "why".to_owned(),
            author: agent(),
            diagnostics: Vec::new(),
            new_ids: Vec::new(),
            interpretation: None,
        };
        queue.create(&update, T0).expect("an update");
        let made = queue.stored_rows().expect("rows");
        let dump = queue.dump().expect("dump");
        drop(queue);
        sqlite3(
            &db,
            &format!(
                "ALTER TABLE proposals DROP COLUMN drift_hash; \
                 ALTER TABLE proposals DROP COLUMN drift; PRAGMA user_version = {version};"
            ),
        );
        assert_eq!(
            sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
            "44",
            "schema {version}"
        );
        let read = SqliteQueue::open_existing(&db, PROJECT)
            .expect("open_existing")
            .expect("the file");
        assert_eq!(read.stored_rows().expect("rows"), made, "read at 46");
        drop(read);
        assert_eq!(sqlite3(&db, "PRAGMA user_version;"), version.to_string());

        let mut queue = SqliteQueue::open(&db, PROJECT).expect("opens, stepping to 8");
        assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8", "from {version}");
        assert_eq!(
            sqlite3(
                &db,
                "SELECT group_concat(name || ' ' || type, ',') FROM pragma_table_info('proposals') \
                 WHERE cid >= 44;"
            ),
            "drift TEXT,drift_hash TEXT"
        );
        assert_eq!(queue.dump().expect("dump"), dump, "rows kept");
        assert_eq!(
            sqlite3(
                &db,
                "SELECT count(*) FROM proposals WHERE drift IS NULL AND drift_hash IS NULL;"
            ),
            "1"
        );
        let id = created(&mut queue, drift("LAW-BOUND", "main", '1'), T1);
        assert_eq!(id, "PR-0002");
        drop(queue);
        drop(SqliteQueue::open(&db, PROJECT).expect("reopen"));
        assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8", "stepped once");
    }
}

type CorruptCase<'a> = (&'a str, &'a str, Vec<(&'a str, Option<String>)>, &'a str);

/// TE-6 ("Queue schema 8", Corrupt): a drift row with `drift` or
/// `drift_hash` `NULL`, evidence not JSON or off its shape (no row, an
/// `ok` row, a top-level key added, a row without `pre_existing`), a hash
/// in upper case or without `b3:`, a stage; an
/// update with either column: `get` and `list` fail naming the row and the
/// column, `list_readable` skips it naming it. A row's later keys (5b's
/// `levels`, 5c's `anchor`) read. M: either column unchecked.
#[test]
fn corrupt_drift_rows_are_named() {
    let scratch = Scratch::new("qdr-corrupt");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    let update = NewProposal {
        kind: ProposalKind::Update,
        target_id: "GOAL-01".to_owned(),
        target_path: "docs/records/GOAL-01.md".to_owned(),
        place: place("main"),
        base_hash: Some("b3:base".to_owned()),
        base_text: Some("old\n".to_owned()),
        new_text: "new\n".to_owned(),
        patch_hash: patch_hash("GOAL-01", "old\n", "new\n"),
        rationale: "why".to_owned(),
        author: agent(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    };
    queue.create(&update, T0).expect("an update");
    let state = queue.stored_rows().expect("rows");
    drop(queue);
    let row_of = |id: &str| {
        let mut one = state.clone();
        one.proposals.retain(|row| row.id() == Some(id));
        one.events.clear();
        one
    };
    let good = evidence("LAW-BOUND");
    let set = |text: &str| Some(text.to_owned());
    let stage = "{\"decision\":\"reject\",\"reason\":\"no\"}";
    let cases: Vec<CorruptCase<'_>> = vec![
        ("drift NULL", "PR-0001", vec![("drift", None)], "drift"),
        (
            "drift_hash NULL",
            "PR-0001",
            vec![("drift_hash", None)],
            "drift_hash",
        ),
        ("not JSON", "PR-0001", vec![("drift", set("rows"))], "drift"),
        (
            "no row",
            "PR-0001",
            vec![("drift", set("{\"rows\":[],\"more\":0}"))],
            "drift",
        ),
        (
            "an ok row",
            "PR-0001",
            vec![("drift", Some(good.replace("code_ahead", "ok")))],
            "drift",
        ),
        (
            "a top-level key added",
            "PR-0001",
            vec![(
                "drift",
                Some(good.replace("\"more\":0", "\"more\":0,\"x\":1")),
            )],
            "drift",
        ),
        (
            "a row without `pre_existing`",
            "PR-0001",
            vec![("drift", Some(good.replace("\"pre_existing\":true,", "")))],
            "drift",
        ),
        (
            "a hash in upper case",
            "PR-0001",
            vec![("drift_hash", Some(format!("b3:{}", "A".repeat(64))))],
            "drift_hash",
        ),
        (
            "a hash without b3:",
            "PR-0001",
            vec![("drift_hash", Some("1".repeat(64)))],
            "drift_hash",
        ),
        (
            "a stage",
            "PR-0001",
            vec![("staged", set(stage)), ("staged_at", set(T1))],
            "staged",
        ),
        (
            "an update with drift",
            "PR-0002",
            vec![("drift", Some(good.clone()))],
            "drift",
        ),
        (
            "an update with drift_hash",
            "PR-0002",
            vec![("drift_hash", Some(hash('1')))],
            "drift_hash",
        ),
    ];
    let mut failures = Vec::new();
    for (index, (label, id, edits, named)) in cases.into_iter().enumerate() {
        let mut one = row_of(id);
        for (name, value) in edits {
            one.proposals[0].columns[column_at(name)] = value;
        }
        let broken_db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&broken_db, PROJECT).expect("open");
        broken.restore(&one).expect("restore takes rows raw");
        let message = match broken.get(id) {
            Err(error) => error.to_string(),
            Ok(found) => {
                failures.push(format!(
                    "{label}: read as {:?}",
                    found.map(|proposal| proposal.kind)
                ));
                continue;
            }
        };
        if !(message.contains(id) && message.contains(&format!("`{named}`"))) {
            failures.push(format!("{label}: {message}"));
            continue;
        }
        assert!(
            broken.list(&ProposalFilter::default()).is_err(),
            "{label}: list fails"
        );
        let listed = broken
            .list_readable(&ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].column, named, "{label}");
    }
    assert!(
        failures.is_empty(),
        "rows not named corrupt:\n{}",
        failures.join("\n")
    );
    // A row's later keys are no defect.
    let later = good.replace(
        "\"compared\":\"ast\"}",
        "\"compared\":\"ast\",\"levels\":{\"lock\":[\"sig\"],\"markers\":[\"sig\"],\
         \"changed\":null},\"anchor\":{\"from\":null}}",
    );
    assert_ne!(later, good);
    let mut one = row_of("PR-0001");
    one.proposals[0].columns[column_at("drift")] = Some(later);
    let mut fine = SqliteQueue::open(scratch.db("later"), PROJECT).expect("open");
    fine.restore(&one).expect("restore");
    let read = fine
        .get("PR-0001")
        .expect("later keys read")
        .expect("the row");
    assert_eq!(read.drift.expect("drift").evidence.rows, [row("LAW-BOUND")]);
}

/// DEFECT (TE-6, `docs/features/drift-queue.md` "Queue schema 8",
/// Corrupt: "JSON off its shape"; core `DriftRow`: "key for key"): a
/// stored row missing one of its nullable keys (`lock`, `class`,
/// `compared`, `node_rev`) must be corrupt, naming `drift`; core
/// `DriftEvidence::parse` reads a missing `Option` key as `null` (serde's
/// default), so such a row decodes as if the key said `null`.
#[test]
fn drift_row_missing_a_key_is_corrupt() {
    let scratch = Scratch::new("qdr-missing");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    let state = queue.stored_rows().expect("rows");
    drop(queue);
    let good = evidence("LAW-BOUND");
    let lock = "\"lock\":{\"rev\":2,\"commit\":\"8888888888888888888888888888888888888888\",\
                \"verified_at\":\"2026-10-08\"},";
    assert!(good.contains(lock), "{good}");
    let mut read_back = Vec::new();
    for (index, (label, broken)) in [
        ("lock", good.replace(lock, "")),
        ("class", good.replace("\"class\":\"check\",", "")),
        ("compared", good.replace(",\"compared\":\"ast\"", "")),
        ("node_rev", good.replace("\"node_rev\":2,", "")),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(broken, good, "{label}: the key is in the row");
        let mut one = state.clone();
        one.events.clear();
        one.proposals[0].columns[column_at("drift")] = Some(broken);
        let mut opened =
            SqliteQueue::open(scratch.db(&format!("missing-{index}")), PROJECT).expect("open");
        opened.restore(&one).expect("restore takes rows raw");
        match opened.get("PR-0001") {
            Err(error) => {
                let message = error.to_string();
                assert!(message.contains("`drift`"), "{label}: {message}");
            }
            Ok(_) => read_back.push(label),
        }
    }
    assert!(
        read_back.is_empty(),
        "rows missing a key read back as a drift item (not corrupt): {read_back:?}"
    );
}

/// RD-12 / TE-6, the backup
/// (`docs/features/drift-queue.md` "Queue schema 8"): drift items (open,
/// applied, rejected, with `more` and a
/// control-free summary) dump, restore into a fresh queue and dump byte
/// for byte; each reads equal; the dump's proposal lines end with the two
/// columns. M: export or restore dropping either column.
#[test]
fn drift_rows_round_trip_byte_identical() {
    let scratch = Scratch::new("qdr-backup");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let mut many = drift("PIECE-COUNTER", "main", '3');
    many.evidence = serde_json::to_string(&DriftEvidence {
        rows: vec![row("PIECE-COUNTER"); 64],
        more: 5,
    })
    .unwrap();
    many.severity = IntakeSeverity::High;
    let results = queue
        .create_drift(
            &[
                drift("GOAL-01", "main", '1'),
                drift("LAW-BOUND", "main", '2'),
                many,
            ],
            T0,
        )
        .unwrap();
    assert_eq!(results.len(), 3);
    queue.approve("PR-0001", &owner(), T1).unwrap();
    queue.applied("PR-0001", &"c".repeat(40), T2).unwrap();
    queue
        .reject(
            "PR-0002",
            &Decision {
                note: Some("fine as it is".to_owned()),
                ..owner()
            },
            T2,
        )
        .unwrap();
    let state = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    for (line, (evidence, hash)) in dump
        .lines()
        .filter(|line| line.starts_with("proposals\t"))
        .zip([
            (evidence("GOAL-01"), hash('1')),
            (evidence("LAW-BOUND"), hash('2')),
        ])
    {
        let tail = format!(
            ",{},{}]",
            serde_json::Value::String(evidence),
            serde_json::Value::String(hash)
        );
        assert!(line.ends_with(&tail), "{line} ends with {tail}");
    }
    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("open");
    fresh.restore(&state).expect("restore");
    assert_eq!(fresh.dump().expect("dump"), dump, "byte-identical");
    assert_eq!(fresh.stored_rows().expect("rows"), state);
    for id in ["PR-0001", "PR-0002", "PR-0003"] {
        assert_eq!(
            fresh.get(id).unwrap().unwrap(),
            queue.get(id).unwrap().unwrap(),
            "{id}"
        );
    }
    let third = fresh.get("PR-0003").unwrap().unwrap();
    let item = third.drift.expect("drift");
    assert_eq!((item.evidence.rows.len(), item.evidence.more), (64, 5));
    assert_eq!(item.severity, IntakeSeverity::High);
}

/// `docs/features/drift-queue.md` "The item" (`target_path` `spec.lock`),
/// the store's own check: `create_drift` refuses an item whose target path
/// is anything but core's `LOCK_FILE` (`Invalid`, ``a drift item's target
/// path "<path>" is not `spec.lock` ``), the whole batch with it; nothing
/// stored, no event. M: `new_drift_problem`'s target path check removed.
#[test]
fn create_drift_refuses_another_target_path() {
    let scratch = Scratch::new("qdr-target-path");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    for path in [
        "docs/spec.lock",
        "sub/spec.lock",
        "./spec.lock",
        "spec.lock ",
        "SPEC.LOCK",
        "spec.lock/",
        "Cargo.lock",
        "",
    ] {
        let mut item = drift("LAW-BOUND", "main", '1');
        item.target_path = path.to_owned();
        let message = invalid(
            queue.create_drift(&[drift("GOAL-01", "main", '2'), item], T0),
            path,
        );
        assert_eq!(
            message,
            format!("a drift item's target path {path:?} is not `spec.lock`"),
            "{path:?}"
        );
    }
    assert!(queue.list(&ProposalFilter::default()).unwrap().is_empty());
    assert!(queue.events().unwrap().is_empty(), "no event");
    // `spec.lock` itself is stored.
    let id = created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    assert_eq!(queue.get(&id).unwrap().unwrap().target_path, "spec.lock");
}

/// `docs/features/drift-queue.md` "Queue schema 8" and "Review, inbox,
/// round, stage" (`drift` as stored), the store's read: a stored row whose
/// keys are out of order, with a later key (5b's `levels`) and a nested
/// object after `compared`, and a row whose nullable keys are `null`, read
/// back through `get` and `list`: `stored_rows` writes each row byte for
/// byte as stored, integers intact; the view reads past the later key.
/// M: `decode_drift` taking the rows from the view (`DriftRow`
/// re-serialized: keys reordered, later keys dropped).
#[test]
fn stored_rows_read_back_as_stored() {
    let scratch = Scratch::new("qdr-as-stored");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    created(&mut queue, drift("LAW-BOUND", "main", '1'), T0);
    let state = queue.stored_rows().expect("rows");
    drop(queue);
    let commit = "8".repeat(40);
    let later = format!(
        "{{\"state\":\"code_ahead\",\"node\":\"LAW-BOUND\",\"relation\":\"configures\",\
         \"file\":\"crates/tally/src/counter.rs\",\"symbol\":\"crates/tally::counter::CAP\",\
         \"kind\":\"item\",\"item_kind\":\"const_item\",\"category\":\"code_changed\",\
         \"detail\":null,\"class\":\"check\",\"pre_existing\":true,\
         \"markers\":[{{\"rev\":2,\"line\":8}},{{\"line\":11,\"rev\":null}}],\"node_rev\":2,\
         \"lock\":{{\"verified_at\":\"2026-10-08\",\"rev\":2,\"commit\":\"{commit}\"}},\
         \"compared\":\"ast\",\"levels\":{{\"lock\":[\"sig\",\"body\"],\
         \"markers\":[\"sig\",\"body\"],\"changed\":[\"body\"]}},\
         \"anchor\":{{\"to\":{{\"line\":12,\"rev\":3}},\"from\":null}}}}"
    );
    let nulls = "{\"node\":\"LAW-BOUND\",\"relation\":\"implements\",\"file\":\"src/a.rs\",\
                 \"kind\":null,\"symbol\":null,\"item_kind\":null,\"state\":\"broken_binding\",\
                 \"category\":\"file_gone\",\"detail\":null,\"class\":null,\
                 \"pre_existing\":false,\"markers\":[],\"node_rev\":null,\"lock\":null,\
                 \"compared\":null}";
    let column = format!("{{\"rows\":[{later},{nulls}],\"more\":7}}");
    let mut one = state.clone();
    one.events.clear();
    one.proposals[0].columns[column_at("drift")] = Some(column.clone());
    let mut opened = SqliteQueue::open(scratch.db("as-stored"), PROJECT).expect("open");
    opened.restore(&one).expect("restore");
    let got = opened.get("PR-0001").expect("get").expect("the row");
    let listed = opened
        .list(&ProposalFilter::default())
        .expect("list")
        .remove(0);
    for (how, proposal) in [("get", got), ("list", listed)] {
        let item = proposal.drift.expect("drift");
        let written: Vec<String> = item
            .stored_rows
            .iter()
            .map(|row| serde_json::to_string(row).unwrap())
            .collect();
        assert_eq!(written, [later.clone(), nulls.to_owned()], "{how}");
        let mut view = row("LAW-BOUND");
        view.markers.push(DriftMarker {
            line: 11,
            rev: None,
        });
        assert_eq!(item.evidence.rows[0], view, "{how}: the view");
        assert_eq!(item.evidence.rows[1].lock, None, "{how}");
        assert_eq!(item.evidence.more, 7, "{how}");
    }
    // The column itself is untouched by the read.
    assert_eq!(
        sqlite3(&scratch.db("as-stored"), "SELECT drift FROM proposals"),
        column
    );
}
