//! docs/features/agent-intake.md, the store half: `create_intake` (Rules 7:
//! the queue's hits read under the write lock the insert takes, so parallel
//! identical intakes store one row — AC-05), the kinds that never apply
//! (`approve`, `approve_from`, `applied`, `applied_with` → `Invalid`
//! without a decision record, nothing written; `reject` settles — AC-09),
//! schema steps 1 → 3 on a version-1 database (rows kept, the sixteen new
//! columns `NULL`; `stored_rows` reads a version-1 DB without stepping it —
//! AC-10, at the schema docs/features/decision-apply.md makes current), and
//! the rows "Stored" names corrupt, each naming its column.
//!
//! Through the public API only (`api.rs`: no test but `format.rs` names the
//! SQL crate); the version-1 database is made, and `user_version` read, by
//! the system's `sqlite3` client.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};

use common::{Scratch, blake3_hex};
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_store::{
    Decision, Intake, NewIntake, NewProposal, PROPOSAL_COLUMNS, Place, ProposalKind,
    ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION, QueueError, SqliteQueue, StoredQueue,
    patch_hash, proposal_columns,
};

const T0: &str = "2026-10-05T21:14:03Z";
const T1: &str = "2026-10-06T08:00:00Z";
const PROJECT: &str = "demo";

/// The five columns step 3 appends, in table order
/// (docs/features/decision-apply.md "Data", "Queue").
const RECORD_COLUMNS: [&str; 5] = [
    "record_id",
    "record_path",
    "record_title",
    "record_text",
    "choice",
];

/// The eleven columns step 2 appends, in table order ("Stored").
const INTAKE_COLUMNS: [&str; 11] = [
    "target_ids",
    "severity",
    "gap_type",
    "summary",
    "working_answer",
    "price_of_other",
    "evidence",
    "options",
    "recommendation",
    "distinct_from",
    "linked",
];

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn author() -> Author {
    Author::new(Some("writer".to_owned()), None, None).expect("an agent")
}

fn question(targets: &[&str], text: &str, distinct_from: &[&str]) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/x.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: text.to_owned(),
            working_answer: Some("Yes.".to_owned()),
            price_of_other: Some("A rebalance.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: distinct_from.iter().map(|id| (*id).to_owned()).collect(),
        },
    }
}

fn discrepancy(targets: &[&str], summary: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("{label} changes"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/x.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: targets.iter().map(|id| (*id).to_owned()).collect(),
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
        },
    }
}

fn update(target: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/x.md".to_owned(),
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

/// `sqlite3 <db> <sql>`, which must succeed: its stdout.
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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn user_version(db: &Path) -> String {
    sqlite3(db, "PRAGMA user_version;").trim().to_owned()
}

// ------------------------------------------------------------------ AC-05

/// AC-05, the store half: eight handles on one database run the same
/// `create_intake` at once (a barrier releases them together), five
/// rounds: one row per round, and every answer names it — the one that
/// stored it as `created`, every other as its only hit, nothing created.
/// M: the queue read before `BEGIN IMMEDIATE`.
#[test]
fn ac05_parallel_identical_intakes_store_one_row() {
    let scratch = Scratch::new("qi-race");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    for round in 0..5_u64 {
        let text = format!("Does round {round} wait for rest?");
        let barrier = Arc::new(Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let db = db.clone();
                let barrier = Arc::clone(&barrier);
                let new = question(&["A-1"], &text, &[]);
                std::thread::spawn(move || {
                    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
                    barrier.wait();
                    queue.create_intake(&new, &[], None, T0).expect("intake")
                })
            })
            .collect();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("a worker"))
            .collect();
        let created: Vec<String> = results
            .iter()
            .filter_map(|result| result.created.as_ref().map(|row| row.id.clone()))
            .collect();
        assert_eq!(created.len(), 1, "round {round}: {results:#?}");
        let id = &created[0];
        assert_eq!(*id, format!("PR-{:04}", round + 1));
        for result in &results {
            if result.created.is_some() {
                assert!(result.hits.is_empty(), "round {round}: {result:?}");
            } else {
                let hits: Vec<&str> = result.hits.iter().map(|hit| hit.id.as_str()).collect();
                assert_eq!(hits, [id.as_str()], "round {round}: {result:?}");
            }
            assert!(result.linked.is_none());
        }
        let queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let counts = queue.counts().expect("counts");
        assert_eq!(
            (counts.proposals, counts.events),
            (round + 1, round + 1),
            "round {round}"
        );
        drop(queue);
    }
}

// ------------------------------------------------------------------ AC-09

/// AC-09, the store half, as docs/features/decision-apply.md ("Data",
/// "Store") changes it: a question and a discrepancy without a decision
/// record refuse every apply state change as `Invalid` naming its kind
/// and the record (`approve`, `approve_from`: approved only with its
/// decision record, or rejected; `applied`, `applied_with`: applied only
/// after the record's step 7), nothing written (`dump()` unchanged);
/// `reject` takes them `open → rejected`, the reason in `decision_note`,
/// one `proposal.rejected`. M: approve accepted.
#[test]
fn ac09_the_apply_steps_refuse_the_kinds_that_never_apply() {
    let scratch = Scratch::new("qi-settle");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let asked = queue
        .create_intake(&question(&["A-1"], "Why?", &[]), &[], None, T0)
        .expect("question")
        .created
        .expect("stored");
    let reported = queue
        .create_intake(&discrepancy(&["A-1"], "It departs."), &[], None, T0)
        .expect("discrepancy")
        .created
        .expect("stored");
    let decision = Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: Some("The answer.".to_owned()),
    };
    let dump = queue.dump().expect("dump");
    for item in [&asked, &reported] {
        let id = item.id.as_str();
        let refusals = [
            ("approve", queue.approve(id, &decision, T1).map(|_| ())),
            (
                "approve_from",
                queue
                    .approve_from(id, &item.seen(), &decision, T1)
                    .map(|_| ()),
            ),
            (
                "applied",
                queue.applied(id, &"2".repeat(40), T1).map(|_| ()),
            ),
            (
                "applied_with",
                queue
                    .applied_with(id, &"2".repeat(40), &decision, T1)
                    .map(|_| ()),
            ),
        ];
        let kind = item.kind.as_str();
        for (name, outcome) in refusals {
            let want = if name.starts_with("approve") {
                format!(
                    "`{id}` is a {kind}: it is approved only with its decision record (or \
                     rejected with the answer)"
                )
            } else {
                format!(
                    "`{id}` is a {kind} without its decision record: it is applied only after \
                     the record's step 7"
                )
            };
            match outcome {
                Err(QueueError::Invalid(message)) => {
                    assert_eq!(message, want, "{name} {id}");
                }
                other => panic!("{name} {id}: expected Invalid, got {other:?}"),
            }
        }
        assert_eq!(queue.dump().expect("dump"), dump, "{id}: nothing written");
    }
    for (item, reason) in [(&asked, "Yes, at rest."), (&reported, "Fix the code.")] {
        let decision = Decision {
            decided_by: "Owner <owner@example.invalid>".to_owned(),
            note: Some(reason.to_owned()),
        };
        let rejected = if item.kind == ProposalKind::Question {
            queue.reject(&item.id, &decision, T1)
        } else {
            queue.reject_from(&item.id, &item.seen(), &decision, T1)
        }
        .expect("reject");
        assert_eq!(rejected.status, ProposalStatus::Rejected);
        assert_eq!(rejected.decision_note.as_deref(), Some(reason));
        assert_eq!(rejected.decided_at.as_deref(), Some(T1));
        assert!(rejected.applied_commit.is_none());
        assert_eq!(rejected.intake, item.intake, "its fields kept");
    }
    let events: Vec<(String, String)> = queue
        .events()
        .expect("events")
        .into_iter()
        .map(|event| {
            (
                event.event_type,
                event.payload["id"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        events,
        [
            ("proposal.created".to_owned(), asked.id.clone()),
            ("proposal.created".to_owned(), reported.id.clone()),
            ("proposal.rejected".to_owned(), asked.id.clone()),
            ("proposal.rejected".to_owned(), reported.id.clone()),
        ]
    );
}

/// `create` stores only a kind that applies: a question or a discrepancy
/// handed to it is `Invalid` naming its kind and `create_intake`, nothing
/// written (`dump()` unchanged, no event, the next ID still `PR-0001`). M:
/// the guard removed.
#[test]
fn create_refuses_the_kinds_that_never_apply() {
    let scratch = Scratch::new("qi-create");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let dump = queue.dump().expect("dump");
    for (kind, name) in [
        (ProposalKind::Question, "question"),
        (ProposalKind::Discrepancy, "discrepancy"),
    ] {
        let mut proposal = update("A-1");
        proposal.kind = kind;
        match queue.create(&proposal, T0) {
            Err(QueueError::Invalid(message)) => assert!(
                message.contains(&format!("not a {name}")) && message.contains("create_intake"),
                "{name}: {message}"
            ),
            other => panic!("{name}: expected Invalid, got {other:?}"),
        }
        assert_eq!(queue.dump().expect("dump"), dump, "{name}: nothing written");
    }
    assert!(queue.events().expect("events").is_empty(), "no event");
    let stored = queue.create(&update("A-1"), T0).expect("an update");
    assert_eq!(stored.id, "PR-0001");
    assert_eq!(stored.kind, ProposalKind::Update);
}

/// "Stored": a discrepancy and its patch go in one transaction, the update
/// under the next ID, `linked` both ways, a `proposal.created` each; a
/// question names no linked item; the stored fields read back as given (a
/// question's `evidence` and `options` `[]`, its gap type and
/// recommendation `None`; an update's intake `None`).
#[test]
fn a_discrepancy_and_its_patch_are_linked_both_ways() {
    let scratch = Scratch::new("qi-linked");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let given = discrepancy(&["A-1", "A-2"], "It departs.");
    let result = queue
        .create_intake(&given, &[], Some(&update("A-1")), T0)
        .expect("intake");
    let stored = result.created.expect("stored");
    let linked = result.linked.expect("linked");
    assert_eq!(
        (stored.id.as_str(), linked.id.as_str()),
        ("PR-0001", "PR-0002")
    );
    assert_eq!(stored.linked.as_deref(), Some("PR-0002"));
    assert_eq!(linked.linked.as_deref(), Some("PR-0001"));
    assert_eq!(stored.kind, ProposalKind::Discrepancy);
    assert_eq!(linked.kind, ProposalKind::Update);
    assert_eq!(stored.target_id, "A-1");
    assert_eq!(stored.intake.as_ref(), Some(&given.intake));
    assert!(linked.intake.is_none());
    assert_eq!(queue.get("PR-0001").unwrap().unwrap(), stored);
    assert_eq!(queue.get("PR-0002").unwrap().unwrap(), linked);
    assert_eq!(queue.counts().unwrap().events, 2);

    let asked = question(&["A-2"], "Why?", &[]);
    let stored = queue
        .create_intake(&asked, &[], None, T0)
        .expect("question")
        .created
        .expect("stored");
    assert_eq!(stored.id, "PR-0003");
    assert!(stored.linked.is_none());
    assert_eq!(stored.intake.as_ref(), Some(&asked.intake));

    // A corpus hit not named: nothing stored, no ID taken; named: stored.
    let named = question(&["A-3"], "Other?", &[]);
    let result = queue
        .create_intake(&named, &["DEC-1".to_owned()], None, T0)
        .expect("intake");
    assert!(result.created.is_none());
    assert_eq!(queue.counts().unwrap().proposals, 3);
    let named = question(&["A-3"], "Other?", &["DEC-1"]);
    let result = queue
        .create_intake(&named, &["DEC-1".to_owned()], None, T0)
        .expect("intake");
    assert_eq!(result.created.expect("stored").id, "PR-0004");
}

// ------------------------------------------------------------------ AC-10

/// Queue schema 1's DDL (the queue-export slice's `STEP_1`).
const SCHEMA_1: &str = "
CREATE TABLE proposals (
  id TEXT PRIMARY KEY,
  project TEXT, kind TEXT, status TEXT,
  target_id TEXT, target_path TEXT,
  git_common_dir TEXT, worktree TEXT, root_rel TEXT,
  branch TEXT, base_commit TEXT,
  base_hash TEXT, base_text TEXT, new_text TEXT,
  patch_hash TEXT,
  rationale TEXT, author TEXT,
  diagnostics TEXT,
  decided_by TEXT, decided_at TEXT, decision_note TEXT, applied_commit TEXT,
  created_at TEXT, updated_at TEXT
) STRICT;
CREATE TABLE events (
  seq INTEGER PRIMARY KEY, project TEXT, type TEXT, payload TEXT, at TEXT
) STRICT;
PRAGMA user_version = 1;
";

/// A version-1 row of `PR-0007` (an open update) and its event.
const SCHEMA_1_ROWS: &str = "
INSERT INTO proposals VALUES ('PR-0007', 'demo', 'update', 'open', 'A-1', 'docs/x.md',
  '/r/.git', '/r', '', 't1', '1111111111111111111111111111111111111111',
  'b3:00', 'base', 'new', 'b3:11', 'Why.', '{\"type\":\"human\",\"role\":null,\"model\":null,\"run\":null}',
  '[]', NULL, NULL, NULL, NULL, '2026-10-05T21:14:03Z', '2026-10-05T21:14:03Z');
INSERT INTO events (project, type, payload, at)
  VALUES ('demo', 'proposal.created', '{\"id\":\"PR-0007\"}', '2026-10-05T21:14:03Z');
";

/// AC-10, the store half: a version-1 database read with `open_existing`
/// gives its rows as 40 columns, the sixteen later ones `None`, and stays
/// at version 1 (no step runs on a read); opened, it steps to 3 in place:
/// the row kept and readable, the sixteen `NULL` in `dump()`, the next ID
/// after it; `proposal_columns` knows 1 (the first 24), 2 (the first 35)
/// and 3 (all 40), nothing else. M: a column left out of step 2; schema 1
/// unknown.
#[test]
fn ac10_a_version_1_database_steps_to_3_keeping_its_rows() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 3);
    assert_eq!(PROPOSAL_COLUMNS.len(), 40);
    assert_eq!(PROPOSAL_COLUMNS[24..35], INTAKE_COLUMNS);
    assert_eq!(PROPOSAL_COLUMNS[35..], RECORD_COLUMNS);
    assert_eq!(proposal_columns(1), Some(&PROPOSAL_COLUMNS[..24]));
    assert_eq!(proposal_columns(2), Some(&PROPOSAL_COLUMNS[..35]));
    assert_eq!(proposal_columns(3), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(0), None);
    assert_eq!(proposal_columns(4), None);

    let scratch = Scratch::new("qi-v1");
    let db = scratch.db("q");
    sqlite3(&db, SCHEMA_1);
    sqlite3(&db, SCHEMA_1_ROWS);
    assert_eq!(user_version(&db), "1");

    let read = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file exists");
    let state: StoredQueue = read.stored_rows().expect("stored rows of a version-1 DB");
    drop(read);
    assert_eq!(user_version(&db), "1", "a read steps nothing");
    assert_eq!(state.proposals.len(), 1);
    assert_eq!(state.events.len(), 1);
    let row = &state.proposals[0];
    assert_eq!(row.id(), Some("PR-0007"));
    assert_eq!(row.columns[2].as_deref(), Some("update"));
    assert_eq!(row.columns[23].as_deref(), Some(T0));
    assert_eq!(row.columns.len(), 40, "{row:?}");
    assert!(
        row.columns[24..].iter().all(Option::is_none),
        "the sixteen later columns None: {row:?}"
    );

    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open steps the schema");
    assert_eq!(user_version(&db), "3");
    let table: Vec<String> = sqlite3(&db, "SELECT name FROM pragma_table_info('proposals');")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(table, PROPOSAL_COLUMNS, "the table's columns in order");
    assert_eq!(
        queue.stored_rows().expect("stored rows"),
        state,
        "rows kept"
    );
    let kept = queue.get("PR-0007").expect("get").expect("kept");
    assert_eq!(kept.kind, ProposalKind::Update);
    assert_eq!(kept.status, ProposalStatus::Open);
    assert_eq!(kept.new_text, "new");
    assert!(kept.intake.is_none() && kept.linked.is_none());
    let dump = queue.dump().expect("dump");
    let proposals = dump.lines().next().expect("the proposals line");
    assert!(
        proposals.ends_with(&format!("\"{T0}\",{}]", ["null"; 16].join(","))),
        "the sixteen NULL: {proposals}"
    );
    let next = queue
        .create_intake(&question(&["A-1"], "Why?", &[]), &[], None, T1)
        .expect("intake on a stepped DB")
        .created
        .expect("stored");
    assert_eq!(next.id, "PR-0008");
    drop(queue);
    let reopened = SqliteQueue::open(&db, PROJECT).expect("reopen");
    assert_eq!(user_version(&db), "3", "stepped once");
    assert_eq!(reopened.get("PR-0008").unwrap().unwrap(), next);
}

// --------------------------------------------------------------- "Stored"

/// "Stored": a row that breaks its kind's shape is corrupt, naming the
/// column: a required column `NULL`, JSON not of its shape, an enum or
/// `recommendation` out of range, a `linked` that is no `PR-` ID. Each
/// broken row is restored alone into a fresh queue; `get` fails naming
/// `PR-0001` and the column, `list_readable` skips it naming it.
#[test]
fn a_corrupt_intake_row_names_its_column() {
    let scratch = Scratch::new("qi-corrupt");
    let source = scratch.db("source");
    let mut queue = SqliteQueue::open(&source, PROJECT).expect("open");
    queue
        .create_intake(&discrepancy(&["A-1"], "It departs."), &[], None, T0)
        .expect("discrepancy");
    let good = queue.stored_rows().expect("rows");
    drop(queue);
    let at = |name: &str| {
        PROPOSAL_COLUMNS
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    let cases: [(&str, Option<&str>); 9] = [
        ("target_ids", None),
        ("target_ids", Some("A-1")),
        ("severity", Some("urgent")),
        ("gap_type", Some("absent")),
        ("summary", None),
        ("evidence", Some("{}")),
        ("options", Some("[{\"label\":\"x\"}]")),
        ("recommendation", Some("2")),
        ("linked", Some("P-1")),
    ];
    for (index, (column, value)) in cases.into_iter().enumerate() {
        let mut state = good.clone();
        state.proposals[0].columns[at(column)] = value.map(str::to_owned);
        let db = scratch.db(&format!("broken-{index}"));
        let mut broken = SqliteQueue::open(&db, PROJECT).expect("open");
        broken.restore(&state).expect("restore takes rows raw");
        let message = match broken.get("PR-0001") {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{column}={value:?}: read as {found:?}"),
        };
        assert!(
            message.contains("PR-0001") && message.contains(&format!("`{column}`")),
            "{column}={value:?}: {message}"
        );
        let listed = broken
            .list_readable(&specengine_store::ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{column}");
        assert_eq!(listed.unreadable.len(), 1, "{column}");
        assert_eq!(listed.unreadable[0].column, column);
    }
}
