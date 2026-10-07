//! docs/features/proposal-kinds.md, the store's half ("Data", Queue row
//! and Store; AC-05's store half, AC-14's schema): `ProposalKind::Create`
//! rows of both forms (a new file: no base, `target_ids` its `id:` and new
//! IDs; new sections: an update's columns plus the new IDs), the new IDs a
//! live create (`open`, `approved`, any repository of the project) holds
//! read by `reserved()` and checked by `create` inside its `Immediate`
//! transaction (`QueueError::Reserved`, nothing written; eight handles at
//! once store one), `create`'s refusals of a malformed proposal, the rows
//! "Data" names corrupt, each naming its column, and the queue's schema
//! (3, 40 columns) unchanged.
//!
//! Through the public API only; the one raw read (`user_version`) uses the
//! system's `sqlite3` client.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};

use common::Scratch;
use specengine_core::intake::CREATE_KIND;
use specengine_core::proposal::Author;
use specengine_store::{
    Decision, NewProposal, PROPOSAL_COLUMNS, Place, ProposalFilter, ProposalKind,
    ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION, QueueError, Reservation, SqliteQueue,
    StoredQueue, patch_hash,
};

const T0: &str = "2026-10-06T12:00:00Z";
const T1: &str = "2026-10-06T12:30:00Z";
const PROJECT: &str = "lantern-keep";

fn place(common_dir: &str) -> Place {
    Place {
        git_common_dir: common_dir.to_owned(),
        worktree: "/r/t1".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn owner() -> Decision {
    Decision {
        decided_by: "Ann Owner <ann@example.org>".to_owned(),
        note: None,
    }
}

/// A new file at `path` whose `id:` is `id` (its target), adding `others`.
fn new_file(path: &str, id: Option<&str>, others: &[&str], common_dir: &str) -> NewProposal {
    let text = format!("---\nid: {}\n---\n\n# New\n", id.unwrap_or("none"));
    let target = id.unwrap_or(path).to_owned();
    let mut new_ids: Vec<String> = id.iter().map(|id| (*id).to_owned()).collect();
    new_ids.extend(others.iter().map(|other| (*other).to_owned()));
    NewProposal {
        kind: ProposalKind::Create,
        patch_hash: patch_hash(&target, "", &text),
        target_id: target,
        target_path: path.to_owned(),
        place: place(common_dir),
        base_hash: None,
        base_text: None,
        new_text: text,
        rationale: "A new record.".to_owned(),
        author: Author::new(Some("writer".to_owned()), None, None).expect("an agent"),
        diagnostics: Vec::new(),
        new_ids,
    }
}

/// New sections `new_ids` in `target`'s span.
fn sections(target: &str, new_ids: &[&str]) -> NewProposal {
    let base_hash = format!("b3:{}", common::blake3_hex(b"span"));
    let text = "## Old {#X}\n\n### New {#Y}\n".to_owned();
    NewProposal {
        kind: ProposalKind::Create,
        patch_hash: patch_hash(target, &base_hash, &text),
        target_id: target.to_owned(),
        target_path: "docs/spec/movement/stamina.md".to_owned(),
        place: place("/r/.git"),
        base_hash: Some(base_hash),
        base_text: Some("## Old {#X}".to_owned()),
        new_text: text,
        rationale: "New sections.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: new_ids.iter().map(|id| (*id).to_owned()).collect(),
    }
}

/// An update of `target`.
fn update(target: &str) -> NewProposal {
    let mut proposal = sections(target, &[]);
    proposal.kind = ProposalKind::Update;
    proposal
}

/// `(id, by, status)` of every reservation.
fn held(queue: &SqliteQueue) -> Vec<(String, String, ProposalStatus)> {
    queue
        .reserved()
        .expect("reserved")
        .into_iter()
        .map(|Reservation { id, by, status }| (id, by, status))
        .collect()
}

fn reservation(id: &str, by: &str, status: ProposalStatus) -> (String, String, ProposalStatus) {
    (id.to_owned(), by.to_owned(), status)
}

/// The value of `column` in the first stored proposal row.
fn column(state: &StoredQueue, row: usize, name: &str) -> Option<String> {
    let at = PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"));
    state.proposals[row].columns[at].clone()
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

// ------------------------------------------------------------ the rows

/// "Data", Queue row: a new file stores `kind` `create`, `base_hash` and
/// `base_text` `NULL` (read as `""`), `target_ids` `[target_id, other new
/// IDs]`, its new IDs its `id:` first; a new file without `id:` is named by
/// its path, its new IDs the others; the section form stores the base and
/// `target_ids` `[target, new IDs]`; every intake and record column `NULL`.
/// The kind's name is core's constant; a create applies, never decides.
#[test]
fn create_rows_store_both_forms() {
    assert_eq!(ProposalKind::Create.as_str(), CREATE_KIND);
    assert_eq!(ProposalKind::parse(CREATE_KIND), Some(ProposalKind::Create));
    assert!(ProposalKind::Create.applies() && !ProposalKind::Create.decides());
    let scratch = Scratch::new("qk-rows");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let file = queue
        .create(
            &new_file("docs/records/R/R-13.md", Some("R-13"), &[], "/r/.git"),
            T0,
        )
        .expect("a new file");
    assert_eq!(file.id, "PR-0001");
    assert_eq!(file.kind, ProposalKind::Create);
    assert!(file.new_file());
    assert_eq!((file.base_hash.as_str(), file.base_text.as_str()), ("", ""));
    assert_eq!(file.new_ids, ["R-13"]);
    assert_eq!(file.target_ids(), ["R-13"]);
    assert!(file.intake.is_none() && file.record.is_none());
    let pathed = queue
        .create(
            &new_file(
                "docs/features/stamina-tuning.md",
                None,
                &["stamina-tuning/AC-09"],
                "/r/.git",
            ),
            T0,
        )
        .expect("a new file without id:");
    assert_eq!(pathed.target_id, "docs/features/stamina-tuning.md");
    assert_eq!(pathed.new_ids, ["stamina-tuning/AC-09"]);
    assert_eq!(
        pathed.target_ids(),
        ["docs/features/stamina-tuning.md", "stamina-tuning/AC-09"]
    );
    let spans = queue
        .create(&sections("RULE-STAM-REGEN", &["EDGE-STAM-REST"]), T0)
        .expect("new sections");
    assert!(!spans.new_file());
    assert_eq!(spans.base_text, "## Old {#X}");
    assert_eq!(spans.new_ids, ["EDGE-STAM-REST"]);
    assert_eq!(spans.target_ids(), ["RULE-STAM-REGEN", "EDGE-STAM-REST"]);

    let state = queue.stored_rows().expect("rows");
    assert_eq!(column(&state, 0, "kind").as_deref(), Some(CREATE_KIND));
    assert_eq!(column(&state, 0, "base_hash"), None);
    assert_eq!(column(&state, 0, "base_text"), None);
    assert_eq!(
        column(&state, 0, "target_ids").as_deref(),
        Some("[\"R-13\"]")
    );
    assert_eq!(
        column(&state, 0, "patch_hash"),
        Some(patch_hash("R-13", "", &file.new_text))
    );
    assert_eq!(
        column(&state, 1, "target_ids").as_deref(),
        Some("[\"docs/features/stamina-tuning.md\",\"stamina-tuning/AC-09\"]")
    );
    assert_eq!(
        column(&state, 2, "target_ids").as_deref(),
        Some("[\"RULE-STAM-REGEN\",\"EDGE-STAM-REST\"]")
    );
    assert!(column(&state, 2, "base_hash").is_some());
    for row in 0..3 {
        for name in &PROPOSAL_COLUMNS[24..] {
            if *name != "target_ids" {
                assert_eq!(column(&state, row, name), None, "row {row}: {name}");
            }
        }
    }
    // An update stores no `target_ids` and reads one target.
    let plain = queue.create(&update("MEC-SPRINT"), T0).expect("an update");
    assert_eq!(plain.target_ids(), ["MEC-SPRINT"]);
    assert!(plain.new_ids.is_empty());
    assert_eq!(column(&queue.stored_rows().unwrap(), 3, "target_ids"), None);
}

/// `create` refuses, as `Invalid` with nothing written: an update without
/// its base or with new IDs; a base given in part; a create's empty base
/// hash; a new ID empty or repeated; a new file whose target is neither
/// its path nor its first new ID; an intake kind.
#[test]
fn a_malformed_create_is_invalid_and_writes_nothing() {
    let scratch = Scratch::new("qk-invalid");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let mut cases: Vec<(&str, NewProposal)> = Vec::new();
    let mut no_base = update("MEC-SPRINT");
    no_base.base_hash = None;
    no_base.base_text = None;
    cases.push(("an update without its base", no_base));
    let mut adding = update("MEC-SPRINT");
    adding.new_ids = vec!["EDGE-X".to_owned()];
    cases.push(("an update adding an ID", adding));
    let mut part = sections("RULE-STAM-REGEN", &["EDGE-STAM-REST"]);
    part.base_text = None;
    cases.push(("a base in part", part));
    let mut empty = sections("RULE-STAM-REGEN", &["EDGE-STAM-REST"]);
    empty.base_hash = Some(String::new());
    cases.push(("an empty base hash", empty));
    cases.push((
        "a repeated new ID",
        sections("RULE-STAM-REGEN", &["EDGE-STAM-REST", "EDGE-STAM-REST"]),
    ));
    cases.push(("an empty new ID", sections("RULE-STAM-REGEN", &[""])));
    let mut target = new_file("docs/records/R/R-13.md", Some("R-13"), &[], "/r/.git");
    target.target_id = "R-14".to_owned();
    cases.push(("a new file named by another ID", target));
    let mut asked = new_file("docs/records/R/R-13.md", Some("R-13"), &[], "/r/.git");
    asked.kind = ProposalKind::Question;
    cases.push(("an intake kind", asked));
    let dump = queue.dump().expect("dump");
    for (label, proposal) in cases {
        match queue.create(&proposal, T0) {
            Err(QueueError::Invalid(message)) => assert!(!message.is_empty(), "{label}"),
            other => panic!("{label}: expected Invalid, got {other:?}"),
        }
        assert_eq!(
            queue.dump().expect("dump"),
            dump,
            "{label}: nothing written"
        );
    }
}

// ------------------------------------------------------- reservations

/// AC-05, the store half: `PR-0001` holds `R-13` while `open` and
/// `approved` (`reserved()`, read only); a second handle's `create` of
/// `R-13` (another file, another repository of the project) is `Reserved
/// {R-13, PR-0001}`, nothing written; another project's create of `R-13`
/// stores; applied or rejected, the ID is free again. M: the
/// in-transaction check dropped.
#[test]
fn ac05_a_live_create_holds_its_new_ids_against_every_handle() {
    let scratch = Scratch::new("qk-reserve");
    let db = scratch.db("q");
    let mut first = SqliteQueue::open(&db, PROJECT).expect("open");
    let mut second = SqliteQueue::open(&db, PROJECT).expect("a second handle");
    assert!(held(&second).is_empty());
    first
        .create(
            &new_file("docs/records/R/R-13.md", Some("R-13"), &[], "/r/.git"),
            T0,
        )
        .expect("PR-0001");
    assert_eq!(
        held(&second),
        [reservation("R-13", "PR-0001", ProposalStatus::Open)]
    );
    let dump = second.dump().expect("dump");
    assert_eq!(second.reserved().unwrap().len(), 1);
    assert_eq!(second.dump().expect("dump"), dump, "reserved() reads only");
    for status in [ProposalStatus::Open, ProposalStatus::Approved] {
        if status == ProposalStatus::Approved {
            first.approve("PR-0001", &owner(), T1).expect("approved");
            assert_eq!(
                held(&second),
                [reservation("R-13", "PR-0001", ProposalStatus::Approved)]
            );
        }
        let dump = second.dump().expect("dump");
        for proposal in [
            new_file("docs/records/R/R-13-b.md", Some("R-13"), &[], "/r/.git"),
            new_file(
                "docs/records/R/R-13-c.md",
                Some("R-13"),
                &[],
                "/other/clone/.git",
            ),
            sections("RULE-STAM-REGEN", &["EDGE-STAM-REST", "R-13"]),
        ] {
            match second.create(&proposal, T1) {
                Err(QueueError::Reserved { id, by }) => {
                    assert_eq!((id.as_str(), by.as_str()), ("R-13", "PR-0001"), "{status}");
                }
                other => panic!("{status}: expected Reserved, got {other:?}"),
            }
            assert_eq!(second.dump().expect("dump"), dump, "nothing written");
        }
    }
    // Another project in the same database holds nothing of this one's.
    let mut elsewhere = SqliteQueue::open(&db, "zerkalo").expect("another project");
    assert!(held(&elsewhere).is_empty());
    elsewhere
        .create(
            &new_file("docs/records/R/R-13.md", Some("R-13"), &[], "/z/.git"),
            T1,
        )
        .expect("another project's R-13");
    // Applied: free again.
    first
        .applied("PR-0001", &"a".repeat(40), T1)
        .expect("applied");
    assert!(held(&second).is_empty());
    let next = second
        .create(
            &new_file("docs/records/R/R-13-b.md", Some("R-13"), &[], "/r/.git"),
            T1,
        )
        .expect("free after the apply");
    assert_eq!(
        held(&first),
        [reservation("R-13", &next.id, ProposalStatus::Open)]
    );
    // Rejected: free again.
    second.reject(&next.id, &owner(), T1).expect("rejected");
    assert!(held(&first).is_empty());
    // New sections hold every new ID, never their target.
    let spans = first
        .create(&sections("RULE-STAM-REGEN", &["EDGE-A", "EDGE-B"]), T1)
        .expect("sections");
    assert_eq!(
        held(&second),
        [
            reservation("EDGE-A", &spans.id, ProposalStatus::Open),
            reservation("EDGE-B", &spans.id, ProposalStatus::Open),
        ]
    );
    // An update never reserves, never meets a reservation.
    second
        .create(&update("RULE-STAM-REGEN"), T1)
        .expect("an update");
    assert_eq!(held(&second).len(), 2);
}

/// AC-05, the race: eight handles create the same new ID at once (a
/// barrier releases them together), five rounds: one stored per round,
/// every other `Reserved` naming it. M: the reservations read before
/// `BEGIN IMMEDIATE`.
#[test]
fn ac05_parallel_creates_of_one_new_id_store_one() {
    let scratch = Scratch::new("qk-race");
    let db = scratch.db("q");
    drop(SqliteQueue::open(&db, PROJECT).expect("open"));
    for round in 0..5_u64 {
        let new_id = format!("R-{}", 20 + round);
        let barrier = Arc::new(Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|worker| {
                let db = db.clone();
                let barrier = Arc::clone(&barrier);
                let proposal = new_file(
                    &format!("docs/records/R/{new_id}-{worker}.md"),
                    Some(&new_id),
                    &[],
                    "/r/.git",
                );
                std::thread::spawn(move || {
                    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
                    barrier.wait();
                    queue.create(&proposal, T0)
                })
            })
            .collect();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("a worker"))
            .collect();
        let stored: Vec<String> = results
            .iter()
            .filter_map(|result| result.as_ref().ok().map(|row| row.id.clone()))
            .collect();
        assert_eq!(stored.len(), 1, "round {round}: {results:#?}");
        let winner = &stored[0];
        assert_eq!(*winner, format!("PR-{:04}", round + 1));
        for result in &results {
            match result {
                Ok(_) => {}
                Err(QueueError::Reserved { id, by }) => {
                    assert_eq!((id, by), (&new_id, winner), "round {round}");
                }
                Err(other) => panic!("round {round}: {other}"),
            }
        }
        let queue = SqliteQueue::open(&db, PROJECT).expect("open");
        let counts = queue.counts().expect("counts");
        assert_eq!(
            (counts.proposals, counts.events),
            (round + 1, round + 1),
            "round {round}: one row, one event"
        );
    }
}

// ------------------------------------------------------- corrupt rows

/// "Data", New IDs of a row: corrupt, named: `target_ids` `NULL`, not a
/// JSON list, not headed by `target_id`, an item not canonical or
/// repeated; the base set partly or an empty base hash; `new_text`,
/// `rationale` `NULL`; an intake column, `linked` or a record column set.
/// Each broken row restored alone: `get` fails naming `PR-0001` and the
/// column, `list_readable` skips it naming it, `reserved()` reads it as
/// holding nothing when its targets do not read. M: a column missing.
#[test]
fn a_corrupt_create_row_names_its_column() {
    let scratch = Scratch::new("qk-corrupt");
    let mut files = SqliteQueue::open(scratch.db("file"), PROJECT).expect("open");
    files
        .create(
            &new_file("docs/records/R/R-13.md", Some("R-13"), &["R-14"], "/r/.git"),
            T0,
        )
        .expect("a new file");
    let file = files.stored_rows().expect("rows");
    drop(files);
    let mut spans = SqliteQueue::open(scratch.db("spans"), PROJECT).expect("open");
    spans
        .create(&sections("RULE-STAM-REGEN", &["EDGE-STAM-REST"]), T0)
        .expect("sections");
    let section = spans.stored_rows().expect("rows");
    drop(spans);
    let at = |name: &str| {
        PROPOSAL_COLUMNS
            .iter()
            .position(|column| *column == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    type Case<'a> = (
        &'a str,
        &'a StoredQueue,
        Vec<(&'a str, Option<&'a str>)>,
        &'a str,
    );
    let cases: Vec<Case<'_>> = vec![
        (
            "targets NULL",
            &file,
            vec![("target_ids", None)],
            "target_ids",
        ),
        (
            "not JSON",
            &file,
            vec![("target_ids", Some("R-13"))],
            "target_ids",
        ),
        (
            "not headed by target_id",
            &file,
            vec![("target_ids", Some("[\"R-14\",\"R-13\"]"))],
            "target_ids",
        ),
        (
            "not canonical",
            &file,
            vec![("target_ids", Some("[\"R-13\",\"r 14\"]"))],
            "target_ids",
        ),
        (
            "repeated",
            &file,
            vec![("target_ids", Some("[\"R-13\",\"R-13\"]"))],
            "target_ids",
        ),
        (
            "an object",
            &file,
            vec![("target_ids", Some("{\"R-13\":1}"))],
            "target_ids",
        ),
        (
            "the base text alone",
            &file,
            vec![("base_text", Some("old"))],
            "base_hash",
        ),
        (
            "the base hash alone",
            &section,
            vec![("base_text", None)],
            "base_text",
        ),
        (
            "an empty base hash",
            &section,
            vec![("base_hash", Some(""))],
            "base_hash",
        ),
        ("new_text NULL", &file, vec![("new_text", None)], "new_text"),
        (
            "rationale NULL",
            &file,
            vec![("rationale", None)],
            "rationale",
        ),
        (
            "a severity",
            &file,
            vec![("severity", Some("high"))],
            "severity",
        ),
        (
            "a summary",
            &section,
            vec![("summary", Some("x"))],
            "summary",
        ),
        ("linked", &file, vec![("linked", Some("PR-0002"))], "linked"),
        (
            "a record",
            &file,
            vec![("record_id", Some("DEC-0024"))],
            "record_id",
        ),
        (
            "a choice",
            &section,
            vec![("choice", Some("{\"option\":0}"))],
            "choice",
        ),
    ];
    for (index, (label, base, edits, named)) in cases.into_iter().enumerate() {
        let mut state = base.clone();
        for (name, value) in edits {
            state.proposals[0].columns[at(name)] = value.map(str::to_owned);
        }
        let mut broken =
            SqliteQueue::open(scratch.db(&format!("broken-{index}")), PROJECT).expect("open");
        broken.restore(&state).expect("restore takes rows raw");
        let message = match broken.get("PR-0001") {
            Err(error) => error.to_string(),
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        assert!(
            message.contains("PR-0001") && message.contains(&format!("`{named}`")),
            "{label}: {message}"
        );
        let listed = broken
            .list_readable(&ProposalFilter::default())
            .expect("list_readable");
        assert!(listed.proposals.is_empty(), "{label}");
        assert_eq!(listed.unreadable.len(), 1, "{label}");
        assert_eq!(listed.unreadable[0].column, named, "{label}");
        // `reserved()` still reads; targets that do not read hold nothing.
        let reserved = broken.reserved().expect("reserved reads a corrupt row");
        if ["targets NULL", "not JSON", "an object"].contains(&label) {
            assert!(reserved.is_empty(), "{label}: {reserved:?}");
        }
    }
    // The rows themselves read.
    for (label, state) in [("file", &file), ("sections", &section)] {
        let mut good =
            SqliteQueue::open(scratch.db(&format!("good-{label}")), PROJECT).expect("open");
        good.restore(state).expect("restore");
        assert_eq!(
            good.get("PR-0001").unwrap().unwrap().kind,
            ProposalKind::Create,
            "{label}"
        );
    }
}

// ------------------------------------------------------------- schema

/// AC-14, the store half: creates change no schema: `QUEUE_SCHEMA_VERSION`
/// as the queue stands (4 since docs/features/task-package.md, which adds
/// `task_id`), `user_version` 4 after a create, `proposals` 41 columns,
/// the dump's header and column list as before; a create's row restores
/// into a fresh queue and dumps the same (the name, cited by
/// docs/features/proposal-kinds.md, kept). M: a column or a schema bump.
#[test]
fn ac14_creates_keep_schema_3_and_40_columns() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 4);
    assert_eq!(PROPOSAL_COLUMNS.len(), 41);
    assert_eq!(PROPOSAL_COLUMNS[..4], ["id", "project", "kind", "status"]);
    let scratch = Scratch::new("qk-schema");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue
        .create(
            &new_file("docs/records/R/R-13.md", Some("R-13"), &[], "/r/.git"),
            T0,
        )
        .expect("a create");
    queue
        .create(&sections("RULE-STAM-REGEN", &["EDGE-STAM-REST"]), T0)
        .expect("sections");
    let state = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    drop(queue);
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "4");
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "41"
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT group_concat(name, ',') FROM pragma_table_info('proposals');"
        ),
        PROPOSAL_COLUMNS.join(",")
    );
    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("open");
    fresh.restore(&state).expect("restore");
    assert_eq!(fresh.dump().expect("dump"), dump);
    assert_eq!(fresh.stored_rows().expect("rows"), state);
    assert_eq!(
        held(&fresh),
        [
            reservation("R-13", "PR-0001", ProposalStatus::Open),
            reservation("EDGE-STAM-REST", "PR-0002", ProposalStatus::Open),
        ]
    );
}
