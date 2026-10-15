//! docs/features/drift-queue.md "Dedup" through `spec propose drift`
//! (RD-5's named tests, TE-4's CLI half): scratch repositories of
//! `fixtures/code-a` with the golden lock committed, `CAP` changed and
//! committed (`LAW-BOUND` drifts), and of `fixtures/verify-ecs` (a node
//! that reads all `ok` again). The store's 16-cell matrix is
//! `crates/specengine-store/tests/queue_drift.rs`; here each cell is
//! reached through the command: the text lines `queued PR | node | status
//! | same evidence` / `evidence changed since it was raised`, `settled PR |
//! node | rejected | same evidence`, and the notes.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod verify_common;

use std::fs;
use std::sync::{Arc, Barrier};

use drift_common::{
    COUNTER, DECIDED, STAMINA, approve_yes, cap_changed, data_dir_of, drifted, dump, ecs_drifted,
    proposal, proposals, raised, reject_at,
};
use lock_common::{DECIDER, LockRepo};
use specengine_store::{Decision, ProposalQueue as _, ProposalStatus};

const SLUG: &str = "code-a";

/// The item of `node` among `raised`'s created ones.
fn created_id(raised: &drift_common::Raised, node: &str) -> String {
    raised
        .created()
        .into_iter()
        .find(|(_, created)| created == node)
        .unwrap_or_else(|| panic!("no item for {node}: {}", raised.text()))
        .0
}

/// The hash `node`'s item stores.
fn hash_of(repo: &LockRepo, id: &str) -> String {
    proposal(repo, SLUG, id).drift.expect("drift").hash
}

/// RD-5: a second raise of the same commit stores nothing: each item a
/// hit, `queued PR | node | open | same evidence`, the queue (rows and
/// events) byte for byte as before. M: no dedup.
#[test]
fn second_raise_stores_nothing() {
    let repo = drifted("dd-second", cap_changed);
    let first = raised(&repo, &[]);
    let law = created_id(&first, "LAW-BOUND");
    let piece = created_id(&first, "PIECE-COUNTER");
    let before = dump(&repo, SLUG);
    let second = raised(&repo, &[]);
    assert_eq!(second.created(), []);
    assert_eq!(
        second.hits(),
        [
            (law.clone(), "LAW-BOUND".to_owned(), "open".to_owned(), true),
            (
                piece.clone(),
                "PIECE-COUNTER".to_owned(),
                "open".to_owned(),
                true
            ),
        ]
    );
    let lines: Vec<String> = second.text().lines().map(str::to_owned).collect();
    assert_eq!(
        lines[..2],
        [
            format!("queued {law} | LAW-BOUND | open | same evidence"),
            format!("queued {piece} | PIECE-COUNTER | open | same evidence"),
        ]
    );
    assert!(
        lines[2].starts_with("propose drift: 0 created, 2 hit(s); "),
        "{lines:?}"
    );
    assert_eq!(dump(&repo, SLUG), before, "nothing stored, nothing logged");
}

/// RD-5 / TE-4: an open item, other evidence (`CAP` changed again and
/// committed): a hit, `evidence changed since it was raised`, nothing
/// stored; an approved one alike, its status named.
#[test]
fn live_item_with_other_evidence_is_a_hit() {
    let repo = drifted("dd-live", cap_changed);
    let first = raised(&repo, &[]);
    let law = created_id(&first, "LAW-BOUND");
    repo.replace(COUNTER, "CAP: u32 = 12;", "CAP: u32 = 14;");
    repo.commit_all("another cap");
    let before = dump(&repo, SLUG);
    let other = raised(&repo, &[]);
    assert_eq!(other.created(), []);
    assert!(
        other.text().lines().any(|line| line
            == format!("queued {law} | LAW-BOUND | open | evidence changed since it was raised")),
        "{}",
        other.text()
    );
    assert_eq!(dump(&repo, SLUG), before);
    // `approved` (a run stopped after step 7): a hit too.
    {
        let mut queue = drift_common::queue_of(&repo, SLUG).expect("a queue");
        queue
            .approve(
                &law,
                &Decision {
                    decided_by: DECIDER.to_owned(),
                    note: None,
                    staged_at: None,
                },
                DECIDED,
            )
            .expect("approved");
    }
    let again = raised(&repo, &[]);
    assert!(
        again.text().lines().any(|line| line
            == format!(
                "queued {law} | LAW-BOUND | approved | evidence changed since it was raised"
            )),
        "{}",
        again.text()
    );
    assert_eq!(again.created(), []);
}

/// RD-5: a rejected item with the same evidence settles the raise:
/// `settled PR | node | rejected | same evidence`, nothing stored; with
/// other evidence a new item. M: settled ignored.
#[test]
fn rejected_drift_is_not_raised_again() {
    let repo = drifted("dd-rejected", cap_changed);
    let first = raised(&repo, &[]);
    let law = created_id(&first, "LAW-BOUND");
    reject_at(&repo, &repo.root, &law).done("reject");
    assert_eq!(proposal(&repo, SLUG, &law).status, ProposalStatus::Rejected);
    let before = dump(&repo, SLUG);
    let again = raised(&repo, &["LAW-BOUND"]);
    assert_eq!(again.created(), []);
    assert_eq!(
        again.text().lines().next(),
        Some(format!("settled {law} | LAW-BOUND | rejected | same evidence").as_str())
    );
    assert_eq!(dump(&repo, SLUG), before);
    repo.replace(COUNTER, "CAP: u32 = 12;", "CAP: u32 = 14;");
    repo.commit_all("another cap");
    let changed = raised(&repo, &["LAW-BOUND"]);
    let new_id = created_id(&changed, "LAW-BOUND");
    assert_ne!(new_id, law);
    assert_ne!(hash_of(&repo, &new_id), hash_of(&repo, &law));
}

/// RD-5 (Q13): marker lines are not hashed: a blank line above every item
/// of `counter.rs` committed moves `CAP`'s marker, the evidence (and its
/// hash) the same: a hit, `same evidence`; the item's stored rows keep
/// their lines. M: lines hashed.
#[test]
fn moved_marker_line_is_the_same_drift() {
    let repo = drifted("dd-moved", cap_changed);
    let first = raised(&repo, &["LAW-BOUND"]);
    let law = created_id(&first, "LAW-BOUND");
    let text = repo.read(COUNTER);
    repo.write(COUNTER, format!("\n{text}"));
    repo.commit_all("a line above");
    let again = raised(&repo, &["LAW-BOUND"]);
    assert_eq!(again.created(), []);
    assert_eq!(
        again.hits(),
        [(law.clone(), "LAW-BOUND".to_owned(), "open".to_owned(), true)]
    );
    // A fresh queue judging the moved commit stores the same hash.
    let twin = drifted("dd-moved-twin", |repo| {
        cap_changed(repo);
    });
    let text = twin.read(COUNTER);
    twin.write(COUNTER, format!("\n{text}"));
    twin.commit_all("a line above");
    let fresh = raised(&twin, &["LAW-BOUND"]);
    let fresh_id = created_id(&fresh, "LAW-BOUND");
    assert_eq!(
        proposal(&twin, SLUG, &fresh_id).drift.unwrap().hash,
        hash_of(&repo, &law)
    );
    let lines_first: Vec<usize> = proposal(&repo, SLUG, &law)
        .drift
        .unwrap()
        .evidence
        .rows
        .iter()
        .flat_map(|row| row.markers.iter().map(|marker| marker.line))
        .collect();
    let lines_moved: Vec<usize> = proposal(&twin, SLUG, &fresh_id)
        .drift
        .unwrap()
        .evidence
        .rows
        .iter()
        .flat_map(|row| row.markers.iter().map(|marker| marker.line))
        .collect();
    assert_ne!(lines_first, lines_moved, "the stored rows keep their lines");
}

/// RD-5 (Q13): an edit beside the item in its file (a comment appended
/// to `counter.rs`, committed) is the same drift: a hit, `same evidence`.
/// M: the item's file blob hashed.
#[test]
fn edit_beside_the_item_is_the_same_drift() {
    let repo = drifted("dd-beside", cap_changed);
    let first = raised(&repo, &["LAW-BOUND"]);
    let law = created_id(&first, "LAW-BOUND");
    repo.append(COUNTER, "\n// A note beside the items.\n");
    repo.commit_all("a note");
    let again = raised(&repo, &["LAW-BOUND"]);
    assert_eq!(again.created(), [], "{}", again.text());
    assert_eq!(
        again.hits(),
        [(law, "LAW-BOUND".to_owned(), "open".to_owned(), true)]
    );
}

/// RD-5 / TE-4: the same drift on another branch is another item (the key
/// holds the branch); back on `main`, a hit. M: branch dropped from the
/// key.
#[test]
fn drift_on_two_branches_is_two_items() {
    let repo = drifted("dd-branches", cap_changed);
    let main = raised(&repo, &["LAW-BOUND"]);
    let on_main = created_id(&main, "LAW-BOUND");
    repo.git(&["checkout", "-q", "-b", "t1"]);
    let t1 = raised(&repo, &["LAW-BOUND"]);
    let on_t1 = created_id(&t1, "LAW-BOUND");
    assert_ne!(on_t1, on_main);
    assert_eq!(proposal(&repo, SLUG, &on_t1).place.branch, "t1");
    assert_eq!(hash_of(&repo, &on_t1), hash_of(&repo, &on_main));
    repo.git(&["checkout", "-q", "main"]);
    let back = raised(&repo, &["LAW-BOUND"]);
    assert_eq!(back.created(), []);
    assert_eq!(back.hits()[0].0, on_main);
}

/// TE-4: two `spec propose drift` processes racing on one data
/// directory (released together, each under the run watchdog) store one
/// item per node; the other run reports hits. Eight rounds, a fresh data
/// directory each. M: the dedup read outside the insert's transaction.
#[test]
fn concurrent_raises_store_one_item() {
    let repo = drifted("dd-race", cap_changed);
    for round in 0..8 {
        let _ = fs::remove_dir_all(data_dir_of(&repo));
        let barrier = Arc::new(Barrier::new(2));
        let runs: Vec<common::Run> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let repo = &repo;
                    scope.spawn(move || {
                        barrier.wait();
                        repo.spec_piped(&repo.root, &["propose", "drift", "--json"])
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("a raise"))
                .collect()
        });
        let mut created = 0;
        let mut hits = 0;
        for run in &runs {
            run.code(0);
            let document = run.json();
            created += document["created"].as_array().map_or(0, Vec::len);
            hits += document["hits"].as_array().map_or(0, Vec::len);
        }
        assert_eq!((created, hits), (2, 2), "round {round}: {runs:?}");
        let stored = proposals(&repo, SLUG);
        let mut nodes: Vec<&str> = stored.iter().map(|item| item.target_id.as_str()).collect();
        nodes.sort_unstable();
        assert_eq!(nodes, ["LAW-BOUND", "PIECE-COUNTER"], "round {round}");
    }
}

/// "Dedup": an open item whose node reads all `ok` at a later commit is
/// noted: `PR-0001: PERK-REGEN reads ok at HEAD now; spec reject PR-0001
/// --reason … settles it`, the item left open (verify-ecs, the system's
/// body restored).
#[test]
fn open_item_reading_ok_is_noted() {
    let changed = "(stamina.current + rate * dt * 2.0).min(stamina.max)";
    let original = "(stamina.current + rate * dt).min(stamina.max)";
    let repo = ecs_drifted("dd-ok-now", |repo| {
        repo.replace(STAMINA, original, changed);
    });
    let first = raised(&repo, &[]);
    let id = created_id(&first, "PERK-REGEN");
    repo.replace(STAMINA, changed, original);
    repo.commit_all("restored");
    let again = raised(&repo, &[]);
    assert_eq!(again.created(), []);
    assert_eq!(again.hits(), []);
    let note = format!(
        "{id}: PERK-REGEN reads ok at HEAD now; spec reject {id} --reason \u{2026} settles it"
    );
    assert_eq!(again.notes(), [note.as_str()]);
    assert!(
        again.stderr().contains(&format!("note: {note}")),
        "{:?}",
        again.stderr()
    );
    assert_eq!(
        proposal(&repo, "verify-ecs", &id).status,
        ProposalStatus::Open
    );
}

/// RD-5 (Q14) through the command: an applied item whose lock commit is
/// gone from the branch (reset) never settles the same evidence; the new
/// item carries the reverted note. (The revert case: `drift_approve.rs`.)
#[test]
fn applied_item_never_settles() {
    let repo = drifted("dd-applied", cap_changed);
    let first = raised(&repo, &["LAW-BOUND"]);
    let law = created_id(&first, "LAW-BOUND");
    approve_yes(&repo, &law).done("approve");
    let applied = proposal(&repo, SLUG, &law)
        .applied_commit
        .expect("its commit");
    repo.git(&["reset", "-q", "--hard", "HEAD~1"]);
    let again = raised(&repo, &["LAW-BOUND"]);
    let new_id = created_id(&again, "LAW-BOUND");
    assert_ne!(new_id, law);
    assert!(
        again.notes().iter().any(|note| note
            == &format!(
                "{new_id}: LAW-BOUND drifts as when {law} was applied by {applied}; that lock is \
                 not in effect at the commit judged"
            )),
        "{:?}",
        again.notes()
    );
}
