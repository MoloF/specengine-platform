//! docs/features/proposal-apply.md, "Data": the proposal queue through its
//! public API only (`ProposalQueue`, `SqliteQueue`): IDs `PR-NNNN` (the
//! highest plus one, never reused), the states `open → approved → applied`,
//! `open → rejected` with every other change refused, one event per state
//! change in its transaction (AC-19's store half), `reopen`'s rule for
//! `apply_failed`, the repository filter, the injected clock, the
//! `patch_hash`, the dump; AC-18's index half (the queue's tables untouched
//! by an index update and rebuild in the same database file); the
//! compare-and-set changes of iteration 2 (`approve_from`, `reopen_from`,
//! `reject_orphan`: a run changes only the state it read); iteration 3's
//! `applied_with` (step 10 and the completion from `open`), `log_failure`
//! (a run that holds nothing changes no state) and `reject_from` (`open`
//! or `approved`, as read). The raw-SQL
//! halves (an `INDEX_FORMAT` stamp change, a newer `user_version`, IDs past
//! `PR-9999`) live in `format.rs`, the one test allowed a raw connection.

#![cfg(unix)]

mod common;

use common::{Corpus, Scratch, blake3_hex};
use specengine_core::proposal::Author;
use specengine_model::Severity;
use specengine_store::{
    APPLY_VERIFY_STEP, ApplyFailure, Decision, EVENT_APPLIED, EVENT_APPLY_FAILED, EVENT_APPROVED,
    EVENT_CREATED, EVENT_REJECTED, IndexWriter as _, NewProposal, Place, ProposalFilter,
    ProposalFinding, ProposalKind, ProposalQueue, ProposalStatus, QueueError, Seen, SqliteQueue,
    StoreError, patch_hash,
};

const T0: &str = "2026-10-05T21:14:03Z";
const T1: &str = "2026-10-05T21:15:00Z";
const T2: &str = "2026-10-05T21:16:00Z";

fn place(common_dir: &str) -> Place {
    Place {
        git_common_dir: common_dir.to_owned(),
        worktree: "/w/t1".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "0".repeat(40),
    }
}

fn new_proposal(target: &str, common_dir: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/x.md".to_owned(),
        place: place(common_dir),
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Why.\nSecond line.".to_owned(),
        author: Author::human(),
        diagnostics: vec![ProposalFinding {
            code: "mention-dangling".to_owned(),
            severity: Severity::Warning,
            path: "docs/x.md".to_owned(),
            line: 3,
            subject: "X-1".to_owned(),
            message: "no such ID".to_owned(),
        }],
        new_ids: Vec::new(),
    }
}

fn owner(note: Option<&str>) -> Decision {
    Decision {
        decided_by: "Ann Owner <ann@example.org>".to_owned(),
        note: note.map(str::to_owned),
        staged_at: None,
    }
}

fn kinds(queue: &SqliteQueue) -> Vec<(String, String)> {
    queue
        .events()
        .unwrap()
        .into_iter()
        .map(|event| {
            (
                event.event_type,
                event.payload["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn assert_status_error<T: std::fmt::Debug>(result: Result<T, QueueError>, context: &str) {
    match result {
        Err(QueueError::Status { .. }) => {}
        other => panic!("{context}: expected a status error, got {other:?}"),
    }
}

#[test]
fn ids_rise_from_pr_0001_and_every_change_logs_one_event() {
    let scratch = Scratch::new("queue-ids");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, "demo").unwrap();
    assert!(queue.list(&ProposalFilter::default()).unwrap().is_empty());
    assert!(queue.events().unwrap().is_empty());

    let first = queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    assert_eq!(first.id, "PR-0001");
    assert_eq!(first.project, "demo");
    assert_eq!(first.status, ProposalStatus::Open);
    assert_eq!(
        (first.created_at.as_str(), first.updated_at.as_str()),
        (T0, T0)
    );
    assert_eq!(first.diagnostics.len(), 1);
    assert_eq!(first.rationale, "Why.\nSecond line.");
    assert_eq!(first.author, Author::human());
    let second = queue.create(&new_proposal("R-2", "/r/.git"), T0).unwrap();
    assert_eq!(second.id, "PR-0002");
    assert_eq!(queue.get("PR-0002").unwrap().unwrap(), second);
    assert!(queue.get("PR-0003").unwrap().is_none());

    // open → approved (event), approved → approved (decision replaced, no
    // event), approved → applied (event with the commit).
    let approved = queue.approve("PR-0001", &owner(None), T1).unwrap();
    assert_eq!(approved.status, ProposalStatus::Approved);
    assert_eq!(approved.decided_at.as_deref(), Some(T1));
    let again = queue.approve("PR-0001", &owner(Some("ok")), T2).unwrap();
    assert_eq!(again.decision_note.as_deref(), Some("ok"));
    assert_eq!(again.decided_at.as_deref(), Some(T2));
    let applied = queue.applied("PR-0001", "abc123", T2).unwrap();
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(applied.applied_commit.as_deref(), Some("abc123"));
    assert_eq!(applied.updated_at, T2);
    // open → rejected (event with the reason).
    let rejected = queue.reject("PR-0002", &owner(Some("no")), T1).unwrap();
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.decision_note.as_deref(), Some("no"));

    let events = queue.events().unwrap();
    assert_eq!(
        kinds(&queue),
        [
            (EVENT_CREATED.to_owned(), "PR-0001".to_owned()),
            (EVENT_CREATED.to_owned(), "PR-0002".to_owned()),
            (EVENT_APPROVED.to_owned(), "PR-0001".to_owned()),
            (EVENT_APPLIED.to_owned(), "PR-0001".to_owned()),
            (EVENT_REJECTED.to_owned(), "PR-0002".to_owned()),
        ]
    );
    assert!(events.windows(2).all(|pair| pair[0].seq < pair[1].seq));
    assert_eq!(events[3].payload["commit"], "abc123");
    assert_eq!(events[4].payload["reason"], "no");
    assert_eq!(
        [
            EVENT_CREATED,
            EVENT_APPROVED,
            EVENT_APPLIED,
            EVENT_REJECTED,
            EVENT_APPLY_FAILED
        ],
        [
            "proposal.created",
            "proposal.approved",
            "proposal.applied",
            "proposal.rejected",
            "proposal.apply_failed"
        ]
    );

    // Every other change is refused, nothing logged.
    let count = events.len();
    assert_status_error(
        queue.approve("PR-0001", &owner(None), T2),
        "approve applied",
    );
    assert_status_error(
        queue.approve("PR-0002", &owner(None), T2),
        "approve rejected",
    );
    assert_status_error(queue.applied("PR-0002", "x", T2), "applied from rejected");
    assert_status_error(
        queue.reject("PR-0001", &owner(Some("x")), T2),
        "reject applied",
    );
    assert_status_error(
        queue.reject("PR-0002", &owner(Some("x")), T2),
        "reject twice",
    );
    let failure = ApplyFailure {
        step: 3,
        reason: "dirty".to_owned(),
    };
    assert_status_error(queue.reopen("PR-0001", &failure, T2), "reopen applied");
    assert_status_error(queue.reopen("PR-0002", &failure, T2), "reopen rejected");
    match queue.approve("PR-0001", &owner(None), T2) {
        Err(QueueError::Status {
            status,
            applied_commit,
            ..
        }) => {
            assert_eq!(status, ProposalStatus::Applied);
            assert_eq!(
                applied_commit.as_deref(),
                Some("abc123"),
                "names its commit"
            );
        }
        other => panic!("{other:?}"),
    }
    let third = queue.create(&new_proposal("R-3", "/r/.git"), T0).unwrap();
    assert_status_error(queue.applied(&third.id, "x", T2), "applied from open");
    match queue.approve("PR-0099", &owner(None), T2) {
        Err(QueueError::Unknown { id }) => assert_eq!(id, "PR-0099"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        queue.events().unwrap().len(),
        count + 1,
        "only the creation"
    );
}

/// `reopen`: one `apply_failed` (`step`, `reason`) per refused attempt; an
/// `approved` proposal back to `open`, its decision cleared, but at step 10
/// (a commit exists); an `open` one stays.
#[test]
fn reopen_logs_apply_failed_and_keeps_approved_only_at_step_10() {
    let scratch = Scratch::new("queue-reopen");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let id = queue
        .create(&new_proposal("R-1", "/r/.git"), T0)
        .unwrap()
        .id;
    let reopened = queue
        .reopen(
            &id,
            &ApplyFailure {
                step: 2,
                reason: "elsewhere".to_owned(),
            },
            T1,
        )
        .unwrap();
    assert_eq!(reopened.status, ProposalStatus::Open);
    queue.approve(&id, &owner(Some("go")), T1).unwrap();
    let reopened = queue
        .reopen(
            &id,
            &ApplyFailure {
                step: 9,
                reason: "hook".to_owned(),
            },
            T2,
        )
        .unwrap();
    assert_eq!(reopened.status, ProposalStatus::Open);
    assert!(reopened.decided_by.is_none() && reopened.decided_at.is_none());
    assert!(reopened.decision_note.is_none());
    queue.approve(&id, &owner(None), T2).unwrap();
    let kept = queue
        .reopen(
            &id,
            &ApplyFailure {
                step: APPLY_VERIFY_STEP,
                reason: "verify".to_owned(),
            },
            T2,
        )
        .unwrap();
    assert_eq!(APPLY_VERIFY_STEP, 10);
    assert_eq!(kept.status, ProposalStatus::Approved);
    assert!(kept.decided_by.is_some());
    for step in [0, 1, 11] {
        match queue.reopen(
            &id,
            &ApplyFailure {
                step,
                reason: "x".to_owned(),
            },
            T2,
        ) {
            Err(QueueError::Invalid(_)) => {}
            other => panic!("step {step}: {other:?}"),
        }
    }
    let failed: Vec<_> = queue
        .events()
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == EVENT_APPLY_FAILED)
        .map(|event| {
            (
                event.payload["step"].as_u64().unwrap(),
                event.payload["reason"].clone(),
            )
        })
        .collect();
    assert_eq!(
        failed,
        [
            (2, serde_json::json!("elsewhere")),
            (9, serde_json::json!("hook")),
            (10, serde_json::json!("verify"))
        ]
    );
    assert_eq!(
        kinds(&queue)
            .iter()
            .filter(|(kind, _)| kind == EVENT_APPROVED)
            .count(),
        2
    );
}

/// The clock is injected and checked; the filter by repository and state;
/// another handle and another project's handle; `patch_hash`.
#[test]
fn time_stamps_filters_handles_and_the_patch_hash() {
    let scratch = Scratch::new("queue-filter");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, "demo").unwrap();
    for bad in [
        "2026-10-05 21:14:03",
        "2026-10-05T21:14:03+00:00",
        "",
        "yesterday",
    ] {
        match queue.create(&new_proposal("R-1", "/r/.git"), bad) {
            Err(QueueError::Invalid(_)) => {}
            other => panic!("{bad:?}: {other:?}"),
        }
    }
    assert!(
        queue.events().unwrap().is_empty(),
        "nothing stored or logged"
    );
    queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    queue
        .create(&new_proposal("R-2", "/other/.git"), T0)
        .unwrap();
    queue.create(&new_proposal("R-3", "/r/.git"), T0).unwrap();
    queue.reject("PR-0003", &owner(Some("no")), T1).unwrap();

    let ids = |filter: &ProposalFilter, queue: &SqliteQueue| -> Vec<String> {
        queue
            .list(filter)
            .unwrap()
            .into_iter()
            .map(|proposal| proposal.id)
            .collect()
    };
    let ours = ProposalFilter {
        git_common_dir: Some("/r/.git".to_owned()),
        statuses: Vec::new(),
    };
    assert_eq!(ids(&ours, &queue), ["PR-0001", "PR-0003"]);
    let open = ProposalFilter {
        git_common_dir: None,
        statuses: vec![ProposalStatus::Open, ProposalStatus::Approved],
    };
    assert_eq!(ids(&open, &queue), ["PR-0001", "PR-0002"]);

    // A second handle on the file sees the same queue; another project's
    // handle sees none of it and continues the numbering.
    let again = SqliteQueue::open(&db, "demo").unwrap();
    assert_eq!(
        ids(&ProposalFilter::default(), &again),
        ["PR-0001", "PR-0002", "PR-0003"]
    );
    assert_eq!(again.dump().unwrap(), queue.dump().unwrap());
    let mut other = SqliteQueue::open(&db, "elsewhere").unwrap();
    assert!(ids(&ProposalFilter::default(), &other).is_empty());
    assert!(other.get("PR-0001").unwrap().is_none());
    assert!(other.events().unwrap().is_empty());
    match other.reject("PR-0001", &owner(Some("x")), T1) {
        Err(QueueError::Unknown { .. }) => {}
        result => panic!("{result:?}"),
    }
    assert_eq!(other.project(), "elsewhere");

    assert_eq!(
        patch_hash("R-1", "b3:aa", "line\n"),
        format!("b3:{}", blake3_hex(b"R-1\nb3:aa\nline\n"))
    );

    // A database directory that does not exist is the store's error.
    match SqliteQueue::open(scratch.join("missing/q.db"), "demo") {
        Err(QueueError::Store(StoreError::DbDirMissing { .. })) => {}
        other => panic!("{other:?}"),
    }
}

/// AC-18, the index half: the queue lives in the index's database file;
/// an index update, `update_paths` and a rebuild (`spec index --full`)
/// leave the queue's dump byte for byte. M: a queue table in the drop list.
#[test]
fn ac18_the_queue_survives_index_updates_and_a_rebuild() {
    let scratch = Scratch::new("queue-index");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let db = scratch.db("lantern-keep");
    let mut queue = SqliteQueue::open(&db, "lantern-keep").unwrap();
    queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    queue.create(&new_proposal("R-2", "/r/.git"), T0).unwrap();
    queue.approve("PR-0001", &owner(None), T1).unwrap();
    queue.reject("PR-0002", &owner(Some("no")), T1).unwrap();
    let dump = queue.dump().unwrap();
    assert_eq!(
        dump.lines()
            .filter(|line| line.starts_with("proposals\t"))
            .count(),
        2
    );
    assert_eq!(
        dump.lines()
            .filter(|line| line.starts_with("events\t"))
            .count(),
        4
    );

    let mut index = corpus.open(&db);
    corpus.update(&mut index);
    assert_eq!(queue.dump().unwrap(), dump, "after the first index");
    index
        .update_paths(&corpus.tree(), &corpus.scheme, &["docs/spec/game.md"])
        .unwrap();
    index.rebuild(&corpus.tree(), &corpus.scheme).unwrap();
    assert_eq!(queue.dump().unwrap(), dump, "after a rebuild");
    drop(index);
    drop(queue);
    let queue = SqliteQueue::open(&db, "lantern-keep").unwrap();
    assert_eq!(queue.dump().unwrap(), dump, "reopened");
    // The index opens a DB the queue made first, too.
    let other = scratch.db("queue-first");
    let mut first = SqliteQueue::open(&other, "lantern-keep").unwrap();
    first.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    let before = first.dump().unwrap();
    let mut index = corpus.open(&other);
    corpus.update(&mut index);
    assert!(!index.dump().unwrap().is_empty());
    assert_eq!(first.dump().unwrap(), before);
}

/// IDs are taken inside the inserting transaction: four handles creating
/// at once get twenty distinct, gapless IDs and twenty `created` events.
#[test]
fn concurrent_creations_get_distinct_gapless_ids() {
    let scratch = Scratch::new("queue-concurrent");
    let db = scratch.db("q");
    SqliteQueue::open(&db, "demo").unwrap();
    let handles: Vec<_> = (0..4)
        .map(|worker| {
            let db = db.clone();
            std::thread::spawn(move || {
                let mut queue = SqliteQueue::open(&db, "demo").unwrap();
                (0..5)
                    .map(|n| {
                        queue
                            .create(&new_proposal(&format!("R-{worker}{n}"), "/r/.git"), T0)
                            .unwrap()
                            .id
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut ids: Vec<String> = handles
        .into_iter()
        .flat_map(|handle| handle.join().unwrap())
        .collect();
    ids.sort();
    let want: Vec<String> = (1..=20).map(|n| format!("PR-{n:04}")).collect();
    assert_eq!(ids, want);
    let queue = SqliteQueue::open(&db, "demo").unwrap();
    assert_eq!(queue.events().unwrap().len(), 20);
}

const T3: &str = "2026-10-05T21:17:00Z";

fn failure(step: u8, reason: &str) -> ApplyFailure {
    ApplyFailure {
        step,
        reason: reason.to_owned(),
    }
}

fn seen(status: ProposalStatus, updated_at: &str) -> Seen {
    Seen {
        status,
        updated_at: updated_at.to_owned(),
        staged: None,
    }
}

fn assert_changed<T: std::fmt::Debug>(
    result: Result<T, QueueError>,
    status: ProposalStatus,
    updated_at: &str,
    context: &str,
) {
    match result {
        Err(error @ QueueError::Changed { .. }) => {
            let text = error.to_string();
            let QueueError::Changed {
                status: found,
                updated_at: at,
                id,
            } = error
            else {
                unreachable!()
            };
            assert_eq!((found, at.as_str()), (status, updated_at), "{context}");
            assert!(
                text.contains(id.as_str()) && text.contains(updated_at),
                "{context}: {text}"
            );
        }
        other => panic!("{context}: expected Changed, got {other:?}"),
    }
}

/// Apply step 7 as a compare-and-set (review iteration 1, "two concurrent
/// approvals both pass step 7"): two runs read `open`; the first
/// `approve_from` wins, the second gets `Changed` naming the state and
/// time, nothing written, no event. `approved` → `approved` only for the
/// state read and only to a later time (else `Invalid`), the decision
/// replaced, no event; a stale `approved` key is `Changed`; `applied` and
/// `rejected` stay `Status` even with their own key. M: the key compare
/// dropped (the second run approves too).
#[test]
fn approve_from_changes_only_the_state_this_run_read() {
    let scratch = Scratch::new("queue-approve-from");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let created = queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    let read = created.seen();
    assert_eq!(read, seen(ProposalStatus::Open, T0));

    // Run A and run B both read `open`; B approves first.
    let b = queue
        .approve_from(&created.id, &read, &owner(Some("b")), T1)
        .unwrap();
    assert_eq!(b.status, ProposalStatus::Approved);
    assert_eq!(b.seen(), seen(ProposalStatus::Approved, T1));
    assert_eq!(b.decision_note.as_deref(), Some("b"));
    let events = queue.events().unwrap().len();
    let dump = queue.dump().unwrap();
    assert_changed(
        queue.approve_from(&created.id, &read, &owner(Some("a")), T2),
        ProposalStatus::Approved,
        T1,
        "run A after run B",
    );
    assert_eq!(queue.dump().unwrap(), dump, "nothing written");
    assert_eq!(queue.events().unwrap().len(), events, "no event");

    // A run that started `approved` (its key B's state): not at the same
    // second, then later with the decision replaced and no event.
    match queue.approve_from(&created.id, &b.seen(), &owner(Some("again")), T1) {
        Err(QueueError::Invalid(message)) => {
            assert!(message.contains(&created.id), "{message}");
        }
        other => panic!("approved again at its own second: {other:?}"),
    }
    assert_eq!(queue.dump().unwrap(), dump, "nothing written");
    let again = queue
        .approve_from(&created.id, &b.seen(), &owner(Some("again")), T2)
        .unwrap();
    assert_eq!(again.seen(), seen(ProposalStatus::Approved, T2));
    assert_eq!(again.decision_note.as_deref(), Some("again"));
    assert_eq!(again.decided_at.as_deref(), Some(T2));
    assert_eq!(
        queue.events().unwrap().len(),
        events,
        "approved → approved logs nothing"
    );
    // B's key is stale now.
    assert_changed(
        queue.approve_from(&created.id, &b.seen(), &owner(None), T3),
        ProposalStatus::Approved,
        T2,
        "a stale approved key",
    );

    // Applied and rejected: Status even with their own key.
    let applied = queue.applied(&created.id, "abc123", T3).unwrap();
    assert_status_error(
        queue.approve_from(&created.id, &applied.seen(), &owner(None), T3),
        "applied",
    );
    let other = queue.create(&new_proposal("R-2", "/r/.git"), T0).unwrap();
    let rejected = queue.reject(&other.id, &owner(Some("no")), T1).unwrap();
    assert_status_error(
        queue.approve_from(&other.id, &rejected.seen(), &owner(None), T2),
        "rejected",
    );
    match queue.approve_from("PR-0099", &read, &owner(None), T2) {
        Err(QueueError::Unknown { id }) => assert_eq!(id, "PR-0099"),
        result => panic!("{result:?}"),
    }
    let third = queue.create(&new_proposal("R-3", "/r/.git"), T0).unwrap();
    match queue.approve_from(&third.id, &third.seen(), &owner(None), "now") {
        Err(QueueError::Invalid(_)) => {}
        result => panic!("{result:?}"),
    }
    assert_eq!(queue.get(&third.id).unwrap().unwrap(), third);
    assert_eq!(
        kinds(&queue)
            .iter()
            .filter(|(kind, _)| kind == EVENT_APPROVED)
            .count(),
        1,
        "one approval event in all"
    );
}

/// A refused run's `reopen_from` (review iteration 1: "never reopen a
/// proposal another run holds"): with a key that is not the stored state
/// it logs one `apply_failed` and leaves the proposal as it is (another
/// run's `approved` stays, its decision kept); with its own hold it
/// reopens (decision cleared), except at step 10; an `open` proposal stays
/// `open`; `applied` and `rejected` are `Status` with nothing logged; a
/// step outside 2–10 is `Invalid`. M: the hold ignored (the other run's
/// approval reopened).
#[test]
fn reopen_from_reopens_only_its_own_hold() {
    let scratch = Scratch::new("queue-reopen-from");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let created = queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    let id = created.id.clone();
    let read_by_a = created.seen();
    let held_by_b = queue
        .approve_from(&id, &read_by_a, &owner(Some("b")), T1)
        .unwrap()
        .seen();

    // Run A, refused at step 7 against B's approval: logged, not reopened.
    let kept = queue
        .reopen_from(&id, &read_by_a, &failure(7, "changed"), T2)
        .unwrap();
    assert_eq!(kept.status, ProposalStatus::Approved, "B's hold stays");
    assert_eq!(kept.seen(), held_by_b, "untouched, updated_at too");
    assert_eq!(kept.decision_note.as_deref(), Some("b"));
    assert_eq!(
        kinds(&queue).last().unwrap(),
        &(EVENT_APPLY_FAILED.to_owned(), id.clone())
    );

    // Run B, refused at step 9: its own hold reopens.
    let reopened = queue
        .reopen_from(&id, &held_by_b, &failure(9, "hook"), T2)
        .unwrap();
    assert_eq!(reopened.seen(), seen(ProposalStatus::Open, T2));
    assert!(reopened.decided_by.is_none() && reopened.decided_at.is_none());
    assert!(reopened.decision_note.is_none());
    // A stale key on an open proposal: it stays open, logged.
    let still = queue
        .reopen_from(&id, &held_by_b, &failure(8, "late"), T3)
        .unwrap();
    assert_eq!(still.seen(), seen(ProposalStatus::Open, T2));

    // Step 10 keeps even its own hold approved.
    let held = queue
        .approve_from(&id, &still.seen(), &owner(None), T3)
        .unwrap()
        .seen();
    let verify = queue
        .reopen_from(&id, &held, &failure(APPLY_VERIFY_STEP, "verify"), T3)
        .unwrap();
    assert_eq!(verify.seen(), held);
    assert!(verify.decided_by.is_some());

    for step in [0, 1, 11] {
        match queue.reopen_from(&id, &held, &failure(step, "x"), T3) {
            Err(QueueError::Invalid(_)) => {}
            other => panic!("step {step}: {other:?}"),
        }
    }
    let logged = queue.events().unwrap().len();
    let applied = queue.applied(&id, "abc123", T3).unwrap();
    assert_status_error(
        queue.reopen_from(&id, &applied.seen(), &failure(9, "x"), T3),
        "applied",
    );
    let other = queue.create(&new_proposal("R-2", "/r/.git"), T0).unwrap();
    let rejected = queue.reject(&other.id, &owner(Some("no")), T1).unwrap();
    assert_status_error(
        queue.reopen_from(&other.id, &rejected.seen(), &failure(9, "x"), T3),
        "rejected",
    );
    assert_eq!(
        queue.events().unwrap().len(),
        logged + 3,
        "applied, created and rejected only: the refused reopens log nothing"
    );

    let steps: Vec<u64> = queue
        .events()
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == EVENT_APPLY_FAILED)
        .map(|event| event.payload["step"].as_u64().unwrap())
        .collect();
    assert_eq!(steps, [7, 9, 8, 10]);
}

/// The orphan rule's store half (review iteration 1: a gone repository's
/// proposals must be able to leave the inbox): `reject_orphan` takes
/// `open` and `approved` (plain `reject` refuses `approved`), as a
/// compare-and-set on the state read (a stale key is `Changed`, nothing
/// written), logging one `proposal.rejected` with the reason; `applied`
/// and `rejected` are `Status`. M: `approved` refused; the key ignored.
#[test]
fn reject_orphan_takes_open_or_approved_as_read() {
    let scratch = Scratch::new("queue-orphan");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let open = queue
        .create(&new_proposal("R-1", "/gone/.git"), T0)
        .unwrap();
    let rejected = queue
        .reject_orphan(&open.id, &open.seen(), &owner(Some("gone")), T1)
        .unwrap();
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.decision_note.as_deref(), Some("gone"));
    assert_eq!(rejected.decided_at.as_deref(), Some(T1));

    let approved_id = queue
        .create(&new_proposal("R-2", "/gone/.git"), T0)
        .unwrap()
        .id;
    let approved = queue.approve(&approved_id, &owner(None), T1).unwrap();
    assert_status_error(
        queue.reject(&approved_id, &owner(Some("x")), T2),
        "plain reject of an approved proposal",
    );
    let dump = queue.dump().unwrap();
    assert_changed(
        queue.reject_orphan(
            &approved_id,
            &seen(ProposalStatus::Open, T0),
            &owner(Some("stale")),
            T2,
        ),
        ProposalStatus::Approved,
        T1,
        "a stale key",
    );
    assert_eq!(queue.dump().unwrap(), dump, "nothing written");
    let done = queue
        .reject_orphan(&approved_id, &approved.seen(), &owner(Some("moved")), T2)
        .unwrap();
    assert_eq!(done.status, ProposalStatus::Rejected);
    assert_eq!(done.decision_note.as_deref(), Some("moved"));
    assert_eq!(done.updated_at, T2);

    assert_status_error(
        queue.reject_orphan(&approved_id, &done.seen(), &owner(Some("x")), T3),
        "rejected",
    );
    let applied_id = queue
        .create(&new_proposal("R-3", "/gone/.git"), T0)
        .unwrap()
        .id;
    queue.approve(&applied_id, &owner(None), T1).unwrap();
    let applied = queue.applied(&applied_id, "abc123", T2).unwrap();
    match queue.reject_orphan(&applied_id, &applied.seen(), &owner(Some("x")), T3) {
        Err(QueueError::Status { applied_commit, .. }) => {
            assert_eq!(applied_commit.as_deref(), Some("abc123"))
        }
        other => panic!("applied: {other:?}"),
    }
    match queue.reject_orphan("PR-0099", &open.seen(), &owner(Some("x")), T3) {
        Err(QueueError::Unknown { .. }) => {}
        other => panic!("{other:?}"),
    }
    let reasons: Vec<_> = queue
        .events()
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == EVENT_REJECTED)
        .map(|event| {
            (
                event.payload["id"].as_str().unwrap().to_owned(),
                event.payload["reason"].clone(),
            )
        })
        .collect();
    assert_eq!(
        reasons,
        [
            (open.id.clone(), serde_json::json!("gone")),
            (approved_id, serde_json::json!("moved"))
        ]
    );
}

/// The `(type, id, step, commit, reason)` of every event after the first
/// `from`.
fn events_since(
    queue: &SqliteQueue,
    from: usize,
) -> Vec<(String, String, Option<u64>, Option<String>)> {
    queue
        .events()
        .unwrap()
        .into_iter()
        .skip(from)
        .map(|event| {
            (
                event.event_type,
                event.payload["id"].as_str().unwrap().to_owned(),
                event.payload["step"].as_u64(),
                event.payload["commit"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

/// Iteration 3, step 10 and the completion (`applied_with`): an `open`
/// proposal (a completion the owner consented to, or step 10 after the
/// hold was reopened meanwhile) goes to `applied` in one transaction with
/// the decision given (`decided_by`, `decided_at` = now, the note) and its
/// commit, logging `proposal.approved` then `proposal.applied` (`commit`);
/// an `approved` one keeps its own decision (the given one ignored), one
/// `proposal.applied`; `applied` (with its commit named) and `rejected` are
/// `Status`, nothing written or logged; an unknown ID `Unknown`, a bad
/// clock `Invalid`. M: `open` refused; the decision not stored; the
/// approved decision replaced.
#[test]
fn applied_with_takes_open_with_the_decision_and_approved_as_decided() {
    let scratch = Scratch::new("queue-applied-with");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let open = queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    let logged = queue.events().unwrap().len();
    let applied = queue
        .applied_with(&open.id, "c0ffee1", &owner(Some("by hand")), T1)
        .unwrap();
    assert_eq!(applied.status, ProposalStatus::Applied);
    assert_eq!(applied.applied_commit.as_deref(), Some("c0ffee1"));
    assert_eq!(
        applied.decided_by.as_deref(),
        Some("Ann Owner <ann@example.org>")
    );
    assert_eq!(applied.decided_at.as_deref(), Some(T1));
    assert_eq!(applied.decision_note.as_deref(), Some("by hand"));
    assert_eq!(applied.updated_at, T1);
    assert_eq!(queue.get(&open.id).unwrap().unwrap(), applied, "as stored");
    assert_eq!(
        events_since(&queue, logged),
        [
            (EVENT_APPROVED.to_owned(), open.id.clone(), None, None),
            (
                EVENT_APPLIED.to_owned(),
                open.id.clone(),
                None,
                Some("c0ffee1".to_owned())
            ),
        ],
        "approved then applied, in order"
    );
    assert!(
        queue
            .events()
            .unwrap()
            .iter()
            .skip(logged)
            .all(|event| event.at == T1),
        "both at now"
    );

    // Approved: its decision kept, one event.
    let second = queue.create(&new_proposal("R-2", "/r/.git"), T0).unwrap();
    let approved = queue
        .approve(&second.id, &owner(Some("first decision")), T1)
        .unwrap();
    let logged = queue.events().unwrap().len();
    let other = Decision {
        decided_by: "Bob Other <bob@example.org>".to_owned(),
        note: Some("ignored".to_owned()),
        staged_at: None,
    };
    let done = queue
        .applied_with(&second.id, "beef002", &other, T2)
        .unwrap();
    assert_eq!(done.status, ProposalStatus::Applied);
    assert_eq!(done.applied_commit.as_deref(), Some("beef002"));
    assert_eq!(done.decided_by, approved.decided_by, "its own decision");
    assert_eq!(done.decided_at.as_deref(), Some(T1));
    assert_eq!(done.decision_note.as_deref(), Some("first decision"));
    assert_eq!(done.updated_at, T2);
    assert_eq!(
        events_since(&queue, logged),
        [(
            EVENT_APPLIED.to_owned(),
            second.id.clone(),
            None,
            Some("beef002".to_owned())
        )]
    );

    // Applied and rejected: Status, nothing written.
    let dump = queue.dump().unwrap();
    let logged = queue.events().unwrap().len();
    match queue.applied_with(&second.id, "beef002", &owner(None), T3) {
        Err(QueueError::Status {
            status,
            applied_commit,
            ..
        }) => {
            assert_eq!(status, ProposalStatus::Applied);
            assert_eq!(applied_commit.as_deref(), Some("beef002"), "names it");
        }
        other => panic!("applied again: {other:?}"),
    }
    let third = queue.create(&new_proposal("R-3", "/r/.git"), T0).unwrap();
    queue.reject(&third.id, &owner(Some("no")), T1).unwrap();
    let dump_rejected = queue.dump().unwrap();
    let logged_rejected = queue.events().unwrap().len();
    assert_status_error(
        queue.applied_with(&third.id, "abc", &owner(None), T3),
        "rejected",
    );
    assert_eq!(queue.dump().unwrap(), dump_rejected, "nothing written");
    assert_eq!(queue.events().unwrap().len(), logged_rejected);
    assert_ne!(dump, dump_rejected);
    assert_eq!(logged + 2, logged_rejected, "created and rejected only");
    match queue.applied_with("PR-0099", "abc", &owner(None), T3) {
        Err(QueueError::Unknown { id }) => assert_eq!(id, "PR-0099"),
        other => panic!("{other:?}"),
    }
    let fourth = queue.create(&new_proposal("R-4", "/r/.git"), T0).unwrap();
    match queue.applied_with(&fourth.id, "abc", &owner(None), "later") {
        Err(QueueError::Invalid(_)) => {}
        other => panic!("{other:?}"),
    }
    assert_eq!(queue.get(&fourth.id).unwrap().unwrap(), fourth, "untouched");
}

/// Iteration 3, the reopen rule's store half (`log_failure`): a run that
/// holds nothing (refused before its own step 7) logs one
/// `proposal.apply_failed` with its step and reason and changes no state:
/// `open` stays `open`, `approved` (another or a stopped run's) stays
/// `approved` with its decision and `updated_at`, at every step 2–10;
/// `applied` and `rejected` are `Status` with nothing logged; a step
/// outside 2–10 is `Invalid`. M: the approved proposal reopened.
#[test]
fn log_failure_logs_one_event_and_changes_no_state() {
    let scratch = Scratch::new("queue-log-failure");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let open = queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    let logged = queue.events().unwrap().len();
    let kept = queue
        .log_failure(&open.id, &failure(3, "dirty"), T1)
        .unwrap();
    assert_eq!(kept, open, "open, untouched (updated_at too)");
    let event = queue.events().unwrap().pop().unwrap();
    assert_eq!(event.event_type, EVENT_APPLY_FAILED);
    assert_eq!(
        (
            event.payload["id"].as_str(),
            event.payload["step"].as_u64(),
            event.payload["reason"].as_str()
        ),
        (Some(open.id.as_str()), Some(3), Some("dirty"))
    );
    assert_eq!(event.at, T1);

    let approved = queue.approve(&open.id, &owner(Some("held")), T1).unwrap();
    for step in 2..=APPLY_VERIFY_STEP {
        let kept = queue
            .log_failure(&open.id, &failure(step, "refused"), T2)
            .unwrap();
        assert_eq!(kept, approved, "step {step}: approved, its decision kept");
    }
    assert_eq!(
        events_since(&queue, logged)
            .iter()
            .map(|(kind, _, step, _)| (kind.as_str(), *step))
            .collect::<Vec<_>>(),
        [
            (EVENT_APPLY_FAILED, Some(3)),
            (EVENT_APPROVED, None),
            (EVENT_APPLY_FAILED, Some(2)),
            (EVENT_APPLY_FAILED, Some(3)),
            (EVENT_APPLY_FAILED, Some(4)),
            (EVENT_APPLY_FAILED, Some(5)),
            (EVENT_APPLY_FAILED, Some(6)),
            (EVENT_APPLY_FAILED, Some(7)),
            (EVENT_APPLY_FAILED, Some(8)),
            (EVENT_APPLY_FAILED, Some(9)),
            (EVENT_APPLY_FAILED, Some(10)),
        ]
    );
    for step in [0, 1, 11] {
        match queue.log_failure(&open.id, &failure(step, "x"), T3) {
            Err(QueueError::Invalid(_)) => {}
            other => panic!("step {step}: {other:?}"),
        }
    }
    let applied = queue.applied(&open.id, "abc123", T3).unwrap();
    let rejected_id = queue
        .create(&new_proposal("R-2", "/r/.git"), T0)
        .unwrap()
        .id;
    queue.reject(&rejected_id, &owner(Some("no")), T1).unwrap();
    let dump = queue.dump().unwrap();
    let logged = queue.events().unwrap().len();
    assert_status_error(
        queue.log_failure(&applied.id, &failure(9, "x"), T3),
        "applied",
    );
    assert_status_error(
        queue.log_failure(&rejected_id, &failure(9, "x"), T3),
        "rejected",
    );
    assert_eq!(queue.dump().unwrap(), dump, "nothing written");
    assert_eq!(queue.events().unwrap().len(), logged, "nothing logged");
}

/// Iteration 3, the owner's rejection (`reject_from`): `open` and
/// `approved` → `rejected` with the decision (the note the reason),
/// `decided_at` = `updated_at` = now, one `proposal.rejected` with the
/// reason — each a compare-and-set on the state read: a stale key (the
/// proposal approved, reopened or re-dated since) is `Changed` naming the
/// stored state, nothing written or logged; `applied` (its commit named)
/// and `rejected` are `Status`. M: the key ignored; `approved` refused.
#[test]
fn reject_from_rejects_open_or_approved_only_as_read() {
    let scratch = Scratch::new("queue-reject-from");
    let mut queue = SqliteQueue::open(scratch.db("q"), "demo").unwrap();
    let open = queue.create(&new_proposal("R-1", "/r/.git"), T0).unwrap();
    // Read open; approved and reopened since (the same status, a new time).
    let approved = queue.approve(&open.id, &owner(None), T1).unwrap();
    let reopened = queue.reopen(&open.id, &failure(9, "hook"), T2).unwrap();
    assert_eq!(reopened.status, ProposalStatus::Open);
    let dump = queue.dump().unwrap();
    let logged = queue.events().unwrap().len();
    assert_changed(
        queue.reject_from(&open.id, &open.seen(), &owner(Some("stale")), T3),
        ProposalStatus::Open,
        T2,
        "open, re-dated since it was read",
    );
    assert_changed(
        queue.reject_from(&open.id, &approved.seen(), &owner(Some("stale")), T3),
        ProposalStatus::Open,
        T2,
        "read approved, reopened since",
    );
    assert_eq!(queue.dump().unwrap(), dump, "nothing written");
    assert_eq!(queue.events().unwrap().len(), logged, "nothing logged");
    let rejected = queue
        .reject_from(&open.id, &reopened.seen(), &owner(Some("not now")), T3)
        .unwrap();
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.decision_note.as_deref(), Some("not now"));
    assert_eq!(
        rejected.decided_by.as_deref(),
        Some("Ann Owner <ann@example.org>")
    );
    assert_eq!(rejected.decided_at.as_deref(), Some(T3));
    assert_eq!(rejected.updated_at, T3);

    // Approved, as read.
    let second = queue.create(&new_proposal("R-2", "/r/.git"), T0).unwrap();
    let held = queue.approve(&second.id, &owner(Some("held")), T1).unwrap();
    let dump = queue.dump().unwrap();
    assert_changed(
        queue.reject_from(&second.id, &second.seen(), &owner(Some("stale")), T2),
        ProposalStatus::Approved,
        T1,
        "read open, approved since",
    );
    assert_eq!(queue.dump().unwrap(), dump, "nothing written");
    let done = queue
        .reject_from(&second.id, &held.seen(), &owner(Some("dropped")), T2)
        .unwrap();
    assert_eq!(done.status, ProposalStatus::Rejected);
    assert_eq!(done.decision_note.as_deref(), Some("dropped"));
    assert_eq!(done.decided_at.as_deref(), Some(T2));
    assert!(done.applied_commit.is_none());

    // Rejected and applied: Status even with their own key.
    let dump = queue.dump().unwrap();
    let logged = queue.events().unwrap().len();
    assert_status_error(
        queue.reject_from(&second.id, &done.seen(), &owner(Some("x")), T3),
        "rejected",
    );
    let third = queue.create(&new_proposal("R-3", "/r/.git"), T0).unwrap();
    queue.approve(&third.id, &owner(None), T1).unwrap();
    let applied = queue.applied(&third.id, "abc123", T2).unwrap();
    let dump_applied = queue.dump().unwrap();
    let logged_applied = queue.events().unwrap().len();
    match queue.reject_from(&third.id, &applied.seen(), &owner(Some("x")), T3) {
        Err(QueueError::Status { applied_commit, .. }) => {
            assert_eq!(applied_commit.as_deref(), Some("abc123"));
        }
        other => panic!("applied: {other:?}"),
    }
    assert_eq!(queue.dump().unwrap(), dump_applied, "nothing written");
    assert_eq!(queue.events().unwrap().len(), logged_applied);
    assert_ne!(dump, dump_applied);
    assert_eq!(
        logged + 3,
        logged_applied,
        "created, approved, applied only"
    );

    let reasons: Vec<_> = queue
        .events()
        .unwrap()
        .into_iter()
        .filter(|event| event.event_type == EVENT_REJECTED)
        .map(|event| {
            (
                event.payload["id"].as_str().unwrap().to_owned(),
                event.payload["reason"].clone(),
                event.at,
            )
        })
        .collect();
    assert_eq!(
        reasons,
        [
            (open.id.clone(), serde_json::json!("not now"), T3.to_owned()),
            (
                second.id.clone(),
                serde_json::json!("dropped"),
                T2.to_owned()
            )
        ],
        "one rejected event each, with the reason"
    );
}
