//! docs/features/mixed-load-memory.md (RM-0134) MX2 and MX5, inside the
//! store (the oracles read rows no public call returns undecoded). The
//! oracles are `head`, the store's own code at 540db8ff copied verbatim.
//!
//! - `mx2_list_readable_is_heads_read_for_every_filter`: over the queue of
//!   `queue_rows` (every kind in every state, a staged item, a
//!   recorded item, another repository's rows, 12 rows of a gone one,
//!   corrupt `author`, `branch`, `base_commit`, a NULL and an unknown
//!   state, a corrupt row in a state the filter leaves out), for every
//!   subset of the four states and every repository filter (none, each of
//!   the three, one with no row): `list_readable` of the queue and of a
//!   snapshot equal `head::list`, HEAD's
//!   `readable(raw_rows_in(…))`, proposals and
//!   unreadable rows alike. An index-only DB reads empty in both.
//!   M: a bare `status IN (…)` prefilter (the unknown and NULL states
//!   dropped); only the inbox's columns selected (the corrupt `author`
//!   unnoticed): red.
//! - `mx5_the_dedup_is_heads_over_2000_rows`: 2 000 rows inserted as stored
//!   (every kind, every state, NULL and unknown states, three repositories,
//!   another project, targets shared, unshared, prefix-alike (`T-1`,
//!   `T-10`), escaped (`"T\u002d1"`), malformed, non-array, mixed, empty,
//!   NULL; summaries equal after normalising, unequal, NULL; rejected rows
//!   with and without a note; records on decided rows): for 54 intakes of
//!   the three intake kinds, `create_intake`'s hits and related (a corpus
//!   hit never named, so nothing is stored) equal `head::matches`', HEAD's
//!   `queue_matches`.
//!   M: a state filter, a text-only target prefilter, or the Rust target
//!   check dropped: red.

mod head;

/// The store's public items, as `queue_rows` names them.
mod store {
    pub use crate::*;
}

/// The seeded queue, shared with the CLI's goldens (`inbox_rows.rs`).
mod queue_rows;

use std::collections::BTreeSet;
use std::path::PathBuf;

use rusqlite::{Transaction, TransactionBehavior};
use specengine_core::intake::{GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;

use super::{
    Intake, KindFamily, NewIntake, Place, ProposalFilter, ProposalKind, ProposalQueue as _,
    ProposalStatus, QueueMatch, SqliteQueue,
};

const PROJECT: &str = "lantern-keep";

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-store-mixed-load-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// HEAD's `list_readable` of `queue` (its `raw_rows` read the rows in one
/// deferred transaction; the queue is at rest, so the plain connection
/// reads the same).
fn oracle_list(queue: &SqliteQueue, filter: &ProposalFilter) -> super::ProposalList {
    head::list(&queue.conn, &queue.project, filter).expect("HEAD's rows")
}

#[test]
fn mx2_list_readable_is_heads_read_for_every_filter() {
    let scratch = Scratch::new("mx2");
    let db = scratch.0.join("queue.db");
    let places = queue_rows::Places::invented();
    let seeded = queue_rows::seed(&scratch.0.join("template.db"), &db, PROJECT, &places);
    let queue = SqliteQueue::open(&db, PROJECT).expect("the seeded queue");
    let repositories = [
        None,
        Some(places.ours.git_common_dir.clone()),
        Some(places.other.git_common_dir.clone()),
        Some(places.gone.git_common_dir.clone()),
        Some("/nowhere/.git".to_owned()),
    ];
    let mut compared = 0;
    for mask in 0..16_u32 {
        let statuses: Vec<ProposalStatus> = ProposalStatus::ALL
            .into_iter()
            .enumerate()
            .filter(|(at, _)| mask & (1 << at) != 0)
            .map(|(_, status)| status)
            .collect();
        for repository in &repositories {
            let filter = ProposalFilter {
                git_common_dir: repository.clone(),
                statuses: statuses.clone(),
            };
            let oracle = oracle_list(&queue, &filter);
            let listed = queue.list_readable(&filter).expect("list_readable");
            assert_eq!(listed, oracle, "the queue's list_readable of {filter:?}");
            let snapshot = queue.snapshot().expect("a snapshot");
            let read = snapshot
                .list_readable(&filter)
                .expect("snapshot list_readable");
            assert_eq!(read, oracle, "a snapshot's list_readable of {filter:?}");
            drop(snapshot);
            compared += 1;
        }
    }
    assert_eq!(compared, 80);
    // The scene is what the comparison needs: every row, every kind in
    // every state decoded, the six corrupt rows named, in ID order.
    let every = oracle_list(
        &queue,
        &ProposalFilter {
            git_common_dir: None,
            statuses: Vec::new(),
        },
    );
    assert_eq!(
        every.proposals.len() + every.unreadable.len(),
        queue_rows::ROWS
    );
    let unreadable: Vec<(String, String)> = every
        .unreadable
        .iter()
        .map(|row| (row.id.clone(), row.column.clone()))
        .collect();
    let named = |role: &str, column: &str| (seeded.id(role).to_owned(), column.to_owned());
    assert_eq!(
        unreadable,
        [
            named("ours corrupt author", "author"),
            named("ours corrupt branch", "branch"),
            named("ours corrupt base_commit", "base_commit"),
            named("ours NULL status", "status"),
            named("ours unknown status", "status"),
            named("ours corrupt author rejected", "author"),
        ]
    );
    let held: BTreeSet<(ProposalKind, ProposalStatus)> = every
        .proposals
        .iter()
        .map(|proposal| (proposal.kind, proposal.status))
        .collect();
    for kind in ProposalKind::ALL {
        for status in ProposalStatus::ALL {
            assert!(held.contains(&(kind, status)), "no {kind:?} {status:?}");
        }
    }
    assert!(
        every
            .proposals
            .iter()
            .any(|proposal| proposal.staged.is_some()),
        "a staged item"
    );
    assert!(
        every
            .proposals
            .iter()
            .any(|proposal| proposal.record.is_some()),
        "a recorded item"
    );
    // The open-and-approved list (the inbox's) keeps the unknown and NULL
    // states as unreadable rows and leaves the rejected corrupt row out.
    let inbox = ProposalFilter {
        git_common_dir: None,
        statuses: vec![ProposalStatus::Open, ProposalStatus::Approved],
    };
    let columns: Vec<String> = queue
        .list_readable(&inbox)
        .expect("the inbox's list")
        .unreadable
        .into_iter()
        .map(|row| row.column)
        .collect();
    assert_eq!(
        columns,
        ["author", "branch", "base_commit", "status", "status"]
    );
}

#[test]
fn mx2_an_index_only_db_reads_empty_as_head_did() {
    let scratch = Scratch::new("mx2-index-only");
    let db = scratch.0.join("index-only.db");
    rusqlite::Connection::open(&db)
        .and_then(|conn| conn.execute_batch("CREATE TABLE nodes (id TEXT);"))
        .expect("an index-only DB");
    let queue = SqliteQueue::open_existing(&db, PROJECT)
        .expect("opened")
        .expect("a file");
    let filter = ProposalFilter {
        git_common_dir: None,
        statuses: vec![ProposalStatus::Open, ProposalStatus::Approved],
    };
    let oracle = oracle_list(&queue, &filter);
    assert_eq!(oracle, super::ProposalList::default());
    assert_eq!(queue.list_readable(&filter).expect("listed"), oracle);
    let snapshot = queue.snapshot().expect("a snapshot");
    assert_eq!(snapshot.list_readable(&filter).expect("listed"), oracle);
}

// ------------------------------------------------------------------ MX5

const MX5_ROWS: usize = 2_000;

/// The rows' `target_ids` and what each must do in a match.
fn mx5_targets(i: usize) -> (Option<String>, &'static str) {
    let a = 1 + (i * 5) % 19;
    let b = 1 + (i * 11) % 19;
    match (i * 7 + 3) % 23 {
        0..=9 => (Some(format!("[\"T-{a}\"]")), "plain"),
        10..=14 => (Some(format!("[\"T-{a}\",\"T-{b}\"]")), "plain"),
        15 => (Some(format!("[\"T\\u002d{a}\"]")), "escaped"),
        16 => (Some(format!("[\"X-9\",\"\\u0054-{a}\"]")), "escaped"),
        17 => (Some(format!("[\"T-{a}\"")), "malformed"),
        18 => (Some(format!("{{\"T-{a}\":1}}")), "malformed"),
        19 => (Some(format!("\"T-{a}\"")), "malformed"),
        20 => (None, "malformed"),
        21 => (Some("[]".to_owned()), "malformed"),
        _ => (Some(format!("[1,\"T-{a}\"]")), "malformed"),
    }
}

const MX5_SUMMARIES: [Option<&str>; 8] = [
    Some("Does it end?"),
    Some("does  it END?"),
    Some("Does it end at once?"),
    Some(" Does it\tend? "),
    Some("Another thing"),
    None,
    Some("Does it end?\n"),
    Some("DOES IT END?"),
];

const STATES: [Option<&str>; 6] = [
    Some("open"),
    Some("approved"),
    Some("applied"),
    Some("rejected"),
    None,
    Some("parked"),
];

const REPOSITORIES: [&str; 3] = ["/r/.git", "/o/.git", "/g/.git"];

/// What a row must never be, and what some match must be.
#[derive(Default)]
struct Mx5Rows {
    never: BTreeSet<String>,
    escaped: BTreeSet<String>,
    elsewhere: BTreeSet<String>,
}

/// [`MX5_ROWS`] rows inserted as stored, by ID number.
fn mx5_insert(queue: &SqliteQueue) -> Mx5Rows {
    let mut rows = Mx5Rows::default();
    let tx =
        Transaction::new_unchecked(&queue.conn, TransactionBehavior::Immediate).expect("a write");
    {
        let mut insert = tx
            .prepare(
                "INSERT INTO main.proposals (id, project, kind, status, target_id, target_path, \
                 git_common_dir, worktree, root_rel, branch, base_commit, author, diagnostics, \
                 created_at, updated_at, target_ids, severity, summary, decision_note, \
                 record_id, record_path, record_title) VALUES (?1, ?2, ?3, ?4, 'T-1', \
                 'docs/t.md', ?5, '/w', '', 'main', ?6, '{\"type\":\"agent\"}', '[]', \
                 '2026-11-20T09:00:00Z', '2026-11-20T09:00:00Z', ?7, 'normal', ?8, ?9, ?10, \
                 ?11, ?12)",
            )
            .expect("the insert");
        for i in 1..=MX5_ROWS {
            let id = format!("PR-{i:04}");
            let project = if i % 97 == 0 {
                "other-project"
            } else {
                PROJECT
            };
            let kind = ProposalKind::ALL[(i / 6) % ProposalKind::ALL.len()];
            let status = STATES[i % STATES.len()];
            let repository = REPOSITORIES[(i / 48) % REPOSITORIES.len()];
            let (targets, shape) = mx5_targets(i);
            let summary = MX5_SUMMARIES[(i * 3) % MX5_SUMMARIES.len()];
            let note =
                (status == Some("rejected") && i % 5 != 0).then(|| format!("Asked before ({i})."));
            let decided = matches!(status, Some("approved" | "applied"))
                && kind.family() == KindFamily::Decides;
            let record = decided.then(|| format!("DEC-{i:04}"));
            let path = decided.then(|| format!("docs/decisions/DEC-{i:04}.md"));
            let title = decided.then(|| format!("Record {i}"));
            let intake_kind = kind.family() == KindFamily::Decides;
            if project != PROJECT
                || shape == "malformed"
                || !intake_kind
                || !matches!(status, Some("open" | "approved" | "applied" | "rejected"))
            {
                rows.never.insert(id.clone());
            } else {
                if shape == "escaped" {
                    rows.escaped.insert(id.clone());
                }
                if repository != REPOSITORIES[0] {
                    rows.elsewhere.insert(id.clone());
                }
            }
            insert
                .execute(rusqlite::params![
                    id,
                    project,
                    kind.as_str(),
                    status,
                    repository,
                    "1".repeat(40),
                    targets,
                    summary,
                    note,
                    record,
                    path,
                    title
                ])
                .expect("a row");
        }
    }
    tx.commit().expect("committed");
    rows
}

fn mx5_intake(kind: ProposalKind, targets: &[&str], summary: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    let discrepancy = kind == ProposalKind::Discrepancy;
    NewIntake {
        kind,
        target_path: "docs/t.md".to_owned(),
        place: Place {
            git_common_dir: "/r/.git".to_owned(),
            worktree: "/r".to_owned(),
            root_rel: String::new(),
            branch: "main".to_owned(),
            base_commit: "1".repeat(40),
        },
        author: Author::new(Some("developer".to_owned()), None, None).expect("an agent"),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            severity: IntakeSeverity::Normal,
            gap_type: discrepancy.then_some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: (!discrepancy).then(|| "Yes.".to_owned()),
            price_of_other: (!discrepancy).then(|| "A rebalance.".to_owned()),
            evidence: Vec::new(),
            options: if discrepancy {
                vec![option("code"), option("spec")]
            } else {
                Vec::new()
            },
            recommendation: discrepancy.then_some(1),
            distinct_from: Vec::new(),
            why: None,
            gist: None,
            matters: None,
        },
        actor: None,
        card_id: None,
    }
}

#[test]
fn mx5_the_dedup_is_heads_over_2000_rows() {
    let scratch = Scratch::new("mx5");
    let db = scratch.0.join("queue.db");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("a queue");
    let rows = mx5_insert(&queue);
    let target_sets: [&[&str]; 6] = [
        &["T-1"],
        &["T-2", "T-10"],
        &["T-11"],
        &["T-19"],
        &["X-9"],
        &["Z-0"],
    ];
    let summaries = ["Does it end?", "Another thing", "Nothing like it"];
    let never_named = ["MX5-NEVER-NAMED".to_owned()];
    let (mut hits, mut related): (Vec<QueueMatch>, Vec<QueueMatch>) = (Vec::new(), Vec::new());
    let mut intakes = 0;
    for kind in [
        ProposalKind::Question,
        ProposalKind::Decision,
        ProposalKind::Discrepancy,
    ] {
        for targets in target_sets {
            for summary in summaries {
                let intake = mx5_intake(kind, targets, summary);
                let oracle = {
                    let tx = Transaction::new_unchecked(&queue.conn, TransactionBehavior::Deferred)
                        .expect("a read");
                    let found = head::matches(&tx, PROJECT, &intake).expect("HEAD's dedup");
                    tx.commit().expect("ended");
                    found
                };
                let result = queue
                    .create_intake(&intake, &never_named, None, "2026-11-20T10:00:00Z")
                    .expect("the dedup");
                assert!(result.created.is_none(), "nothing stored");
                assert_eq!(
                    (&result.hits, &result.related),
                    (&oracle.0, &oracle.1),
                    "{kind:?} on {targets:?}, {summary:?}"
                );
                hits.extend(oracle.0);
                related.extend(oracle.1);
                intakes += 1;
            }
        }
    }
    assert_eq!(intakes, 54);
    let stored: i64 = queue
        .conn
        .query_row("SELECT count(*) FROM main.proposals", [], |row| row.get(0))
        .expect("counted");
    assert_eq!(stored, i64::try_from(MX5_ROWS).expect("a count"));
    // The scene exercises every rule the comparison guards.
    let found: BTreeSet<&str> = hits
        .iter()
        .chain(&related)
        .map(|found| found.id.as_str())
        .collect();
    assert!(!hits.is_empty() && !related.is_empty(), "hits and related");
    assert!(
        hits.iter()
            .any(|hit| hit.status == ProposalStatus::Rejected && hit.reason.is_some()),
        "a rejected hit with its note"
    );
    assert!(
        hits.iter()
            .any(|hit| hit.status == ProposalStatus::Rejected && hit.reason.is_none()),
        "a rejected hit with no note"
    );
    assert!(
        hits.iter().any(|hit| hit.record_id.is_some()),
        "a hit with its record"
    );
    for status in ProposalStatus::ALL {
        assert!(
            hits.iter().chain(&related).any(|m| m.status == status),
            "a match {status:?}"
        );
    }
    assert!(
        rows.escaped.iter().any(|id| found.contains(id.as_str())),
        "an escaped target matched"
    );
    assert!(
        rows.elsewhere.iter().any(|id| found.contains(id.as_str())),
        "another repository's row matched"
    );
    let wrong: Vec<&&str> = found
        .iter()
        .filter(|id| rows.never.contains(**id))
        .collect();
    assert!(wrong.is_empty(), "rows that share nothing: {wrong:?}");
}
