//! docs/features/kind-amendment.md, the store's half (AC-T1 over AC-R1,
//! AC-R2's reservation, AC-R8's stage, AC-R13's dump): the kind
//! `ProposalKind::Amendment` as "Names" (`ALL` 7, `applies()`,
//! `decides()`, `adds_nodes()`, `with_article()`), its row a file-form
//! create's with `kind` `amendment` (no base, `target_ids` the file and its
//! own new IDs, never an amended one: the store never sees those), its new
//! IDs reserved as a create's, `create` refusing a base (`Invalid`) and
//! `create_intake` refusing the kind, the rows "Data" names corrupt (a
//! create's rules plus a base, `base_hash`; a stored `span_hash`), the
//! stage a new file takes (approve without `span_hash`, no decision flag),
//! and the queue's schema (6, 44 columns) unchanged by it.
//!
//! Through the public API only; the one raw read (`user_version`) uses the
//! system's `sqlite3` client.
//!
//! Mutations (AC-R1, AC-R8, AC-R13): `adds_nodes()` false for the kind;
//! `new_file()` left `== ProposalKind::Create`; export skipping the kind.

#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::Scratch;
use specengine_core::intake::{AMENDMENT_KIND, CREATE_KIND, IntakeSeverity, with_article};
use specengine_core::proposal::Author;
use specengine_store::{
    Decision, Intake, NewIntake, NewProposal, PROPOSAL_COLUMNS, Place, ProposalFilter,
    ProposalKind, ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION, QueueError,
    Reservation, SqliteQueue, Stage, StoredQueue, patch_hash,
};

const T0: &str = "2026-10-08T09:00:00Z";
const T1: &str = "2026-10-08T09:30:00Z";
const T2: &str = "2026-10-08T10:00:00Z";
const PROJECT: &str = "lantern-keep";

/// "Data"'s path and text.
const PATH: &str = "docs/features/regen-delay.md";
const TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
                    [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
                    {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint, not after any \
                    stamina use.\n";
/// Its one new ID, canonical.
const NEW_ID: &str = "regen-delay/AC-01";

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
        staged_at: None,
    }
}

/// "Data"'s amendment as the CLI stores it: named by its path (no `id:`),
/// its one new ID, no base.
fn amendment() -> NewProposal {
    NewProposal {
        kind: ProposalKind::Amendment,
        patch_hash: patch_hash(PATH, "", TEXT),
        target_id: PATH.to_owned(),
        target_path: PATH.to_owned(),
        place: place("/r/.git"),
        base_hash: None,
        base_text: None,
        new_text: TEXT.to_owned(),
        rationale: "Regeneration waits after a sprint.".to_owned(),
        author: Author::new(Some("spec-author".to_owned()), None, None).expect("an agent"),
        diagnostics: Vec::new(),
        new_ids: vec![NEW_ID.to_owned()],
        interpretation: None,
    }
}

/// A file-form create of `path` adding `new_ids`, named by its path.
fn create_file(path: &str, new_ids: &[&str]) -> NewProposal {
    let text = "---\nclass: spec\nstatus: draft\n---\n\n# New\n\n## One {#AC-01}\n\nA.\n";
    NewProposal {
        kind: ProposalKind::Create,
        patch_hash: patch_hash(path, "", text),
        target_id: path.to_owned(),
        target_path: path.to_owned(),
        place: place("/r/.git"),
        base_hash: None,
        base_text: None,
        new_text: text.to_owned(),
        rationale: "A new spec.".to_owned(),
        author: Author::human(),
        diagnostics: Vec::new(),
        new_ids: new_ids.iter().map(|id| (*id).to_owned()).collect(),
        interpretation: None,
    }
}

fn held(queue: &SqliteQueue) -> Vec<(String, String, ProposalStatus)> {
    queue
        .reserved()
        .expect("reserved")
        .into_iter()
        .map(|Reservation { id, by, status, .. }| (id, by, status))
        .collect()
}

fn at(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

fn column(state: &StoredQueue, row: usize, name: &str) -> Option<String> {
    state.proposals[row].columns[at(name)].clone()
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

// ------------------------------------------------------------- AC-R1

/// AC-R1, "Names" (W26, W34): `amendment` is core's constant, the seventh
/// of `ALL` (appended), applies and never decides, adds nodes; its article
/// `an amendment`; `adds_nodes()` true for `create` and `amendment` only,
/// false for the other five. M: `adds_nodes()` false for it.
#[test]
fn ac_r1_the_kind_is_named_appended_applies_and_adds_nodes() {
    assert_eq!(AMENDMENT_KIND, "amendment");
    let kind = ProposalKind::Amendment;
    assert_eq!(kind.as_str(), AMENDMENT_KIND);
    assert_eq!(ProposalKind::parse(AMENDMENT_KIND), Some(kind));
    assert_eq!(ProposalKind::parse("Amendment"), None);
    assert_eq!(ProposalKind::ALL.len(), 7);
    assert_eq!(ProposalKind::ALL[6], kind, "appended last");
    assert!(kind.applies(), "an amendment applies (a file committed)");
    assert!(!kind.decides(), "an amendment never decides");
    assert!(kind.adds_nodes(), "W34: an amendment adds nodes");
    assert_eq!(kind.with_article(), "an amendment");
    assert_eq!(with_article(AMENDMENT_KIND), "an amendment");
    let adding: Vec<ProposalKind> = ProposalKind::ALL
        .into_iter()
        .filter(|kind| kind.adds_nodes())
        .collect();
    assert_eq!(adding, [ProposalKind::Create, ProposalKind::Amendment]);
    for other in ProposalKind::ALL {
        if !matches!(other, ProposalKind::Create | ProposalKind::Amendment) {
            assert!(!other.adds_nodes(), "{other:?} adds no node");
        }
    }
    // The serde name is the constant too.
    assert_eq!(
        serde_json::to_string(&kind).expect("serialize"),
        format!("\"{AMENDMENT_KIND}\"")
    );
    // The developer's second method (deviation 1): only the amendment
    // amends.
    let amending: Vec<ProposalKind> = ProposalKind::ALL
        .into_iter()
        .filter(|kind| kind.amends())
        .collect();
    assert_eq!(amending, [ProposalKind::Amendment]);
}

/// AC-R1, AC-R2 (store half), W28: `create` stores "Data"'s amendment as
/// `PR-0001`, `open`, no base (`base_hash`, `base_text` `NULL`, read as
/// `""`), `new_file()`, `target_ids` `[path, regen-delay/AC-01]` (the
/// amended `RULE-STAM-REGEN` nowhere in the row but its text), every intake
/// and record column `NULL`; `reserved()` exactly `regen-delay/AC-01` by
/// `PR-0001`; a create of that ID is `Reserved`, nothing written. M:
/// `adds_nodes()` false for it; `new_file()` left `== Create`.
#[test]
fn ac_r1_r2_the_row_is_a_new_files_and_holds_its_own_new_ids() {
    let scratch = Scratch::new("qa-row");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let stored = queue.create(&amendment(), T0).expect("an amendment");
    assert_eq!(stored.id, "PR-0001");
    assert_eq!(stored.kind, ProposalKind::Amendment);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert!(stored.new_file(), "an amendment is a new file");
    assert_eq!(
        (stored.base_hash.as_str(), stored.base_text.as_str()),
        ("", "")
    );
    assert_eq!(stored.target_id, PATH);
    assert_eq!(stored.target_path, PATH);
    assert_eq!(stored.new_ids, [NEW_ID]);
    assert_eq!(stored.target_ids(), [PATH, NEW_ID]);
    assert!(stored.intake.is_none() && stored.record.is_none());
    assert!(stored.interpretation.is_none());
    assert_eq!(stored.new_text, TEXT);
    assert_eq!(
        queue.get("PR-0001").unwrap().unwrap(),
        stored,
        "get reads it"
    );

    let state = queue.stored_rows().expect("rows");
    assert_eq!(column(&state, 0, "kind").as_deref(), Some(AMENDMENT_KIND));
    assert_eq!(column(&state, 0, "base_hash"), None);
    assert_eq!(column(&state, 0, "base_text"), None);
    assert_eq!(
        column(&state, 0, "target_ids").as_deref(),
        Some("[\"docs/features/regen-delay.md\",\"regen-delay/AC-01\"]")
    );
    assert_eq!(
        column(&state, 0, "patch_hash"),
        Some(patch_hash(PATH, "", TEXT))
    );
    for name in &PROPOSAL_COLUMNS[24..] {
        if *name != "target_ids" {
            assert_eq!(column(&state, 0, name), None, "{name}");
        }
    }
    let row = state.proposals[0]
        .columns
        .iter()
        .flatten()
        .filter(|value| value.contains("RULE-STAM-REGEN"))
        .count();
    assert_eq!(row, 1, "the amended ID only in new_text, its one copy");

    assert_eq!(
        held(&queue),
        [(
            NEW_ID.to_owned(),
            "PR-0001".to_owned(),
            ProposalStatus::Open
        )]
    );
    // A create of the reserved ID (another file): refused, nothing written.
    let dump = queue.dump().expect("dump");
    // The holder named by its kind (iteration 2): an amendment.
    assert_eq!(
        queue.reserved().expect("reserved")[0].kind,
        ProposalKind::Amendment
    );
    match queue.create(&create_file("docs/features/other.md", &[NEW_ID]), T0) {
        Err(error @ QueueError::Reserved { .. }) => {
            assert_eq!(
                error.to_string(),
                format!("`{NEW_ID}` is reserved by `PR-0001`, a live amendment")
            );
            let QueueError::Reserved { id, by, kind } = error else {
                unreachable!()
            };
            assert_eq!(
                (id.as_str(), by.as_str(), kind),
                (NEW_ID, "PR-0001", ProposalKind::Amendment)
            );
        }
        other => panic!("expected Reserved, got {other:?}"),
    }
    // A second amendment of the same ID alike: an amendment's new IDs are
    // checked as a create's.
    let mut again = amendment();
    again.target_id = "docs/features/regen-again.md".to_owned();
    again.target_path = again.target_id.clone();
    again.patch_hash = patch_hash(&again.target_id, "", TEXT);
    match queue.create(&again, T0) {
        Err(QueueError::Reserved { id, by, .. }) => {
            assert_eq!((id.as_str(), by.as_str()), (NEW_ID, "PR-0001"));
        }
        other => panic!("expected Reserved, got {other:?}"),
    }
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    // A live create's ID is refused to an amendment the same way.
    let mut created = create_file("docs/features/held.md", &["held/AC-01"]);
    created.patch_hash = patch_hash("docs/features/held.md", "", &created.new_text);
    queue.create(&created, T0).expect("a create");
    let mut taking = amendment();
    taking.new_ids = vec!["held/AC-01".to_owned()];
    match queue.create(&taking, T0) {
        Err(QueueError::Reserved { id, by, kind }) => {
            assert_eq!(
                (id.as_str(), by.as_str(), kind),
                ("held/AC-01", "PR-0002", ProposalKind::Create)
            );
        }
        other => panic!("expected Reserved, got {other:?}"),
    }
}

/// AC-R1: `create` refuses an amendment with a base (both, either part, an
/// empty hash) as `Invalid`, nothing written; a repeated or empty new ID
/// as a create's; `create_intake` refuses the kind (`Invalid`). M: the base
/// rule dropped.
#[test]
fn ac_r1_an_amendment_with_a_base_and_the_intake_door_are_invalid() {
    let scratch = Scratch::new("qa-invalid");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    let based = |hash: Option<&str>, text: Option<&str>| {
        let mut proposal = amendment();
        proposal.base_hash = hash.map(str::to_owned);
        proposal.base_text = text.map(str::to_owned);
        proposal
    };
    let mut repeated = amendment();
    repeated.new_ids = vec![NEW_ID.to_owned(), NEW_ID.to_owned()];
    let mut empty = amendment();
    empty.new_ids = vec![String::new()];
    let mut interpreted = amendment();
    interpreted.interpretation = Some(specengine_store::Interpretation {
        decision: "DEC-0023".to_owned(),
        how: "h".to_owned(),
        why: "w".to_owned(),
    });
    let cases: Vec<(&str, NewProposal, &str)> = vec![
        (
            "a base",
            based(Some("b3:00"), Some("old")),
            "an amendment is a new file",
        ),
        ("the hash alone", based(Some("b3:00"), None), "base"),
        ("the text alone", based(None, Some("old")), "base"),
        ("an empty hash", based(Some(""), Some("")), "amendment"),
        ("a repeated new ID", repeated, "repeated"),
        ("an empty new ID", empty, "empty"),
        ("an interpretation's fields", interpreted, ""),
    ];
    let dump = queue.dump().expect("dump");
    for (label, proposal, names) in cases {
        match queue.create(&proposal, T0) {
            Err(QueueError::Invalid(message)) => {
                assert!(message.contains(names), "{label}: {message}");
            }
            other => panic!("{label}: expected Invalid, got {other:?}"),
        }
        assert_eq!(
            queue.dump().expect("dump"),
            dump,
            "{label}: nothing written"
        );
    }
    // The intake door takes no amendment.
    let intake = NewIntake {
        kind: ProposalKind::Amendment,
        target_path: PATH.to_owned(),
        place: place("/r/.git"),
        author: Author::human(),
        intake: Intake {
            target_ids: vec!["RULE-STAM-REGEN".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: "Waits after a sprint?".to_owned(),
            working_answer: Some("Yes.".to_owned()),
            price_of_other: Some("A rebalance.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
        },
    };
    match queue.create_intake(&intake, &[], None, T0) {
        Err(QueueError::Invalid(message)) => assert!(!message.is_empty()),
        other => panic!("create_intake of an amendment: expected Invalid, got {other:?}"),
    }
    assert_eq!(queue.dump().expect("dump"), dump, "nothing written");
    assert!(queue.events().expect("events").is_empty(), "no event");
}

/// AC-R1, "Row": an amendment row holding what a file-form create's may
/// not (a create's rules), or a base (`base_hash`; the text alone:
/// `base_text`), is corrupt, named: `get` fails naming the row and the
/// column, `list_readable` skips it naming the column. The row itself
/// reads. M: a base read as an update's.
#[test]
fn ac_r1_a_corrupt_amendment_row_names_its_column() {
    let scratch = Scratch::new("qa-corrupt");
    let mut queue = SqliteQueue::open(scratch.db("good"), PROJECT).expect("open");
    queue.create(&amendment(), T0).expect("an amendment");
    let good = queue.stored_rows().expect("rows");
    drop(queue);
    let hash = format!("b3:{}", common::blake3_hex(b"span"));
    let span_stage = "{\"decision\":\"approve\",\"option\":null,\"answer\":null,\"canon\":null,\
                      \"note\":null,\"span_hash\":\"b3:00\"}";
    let option_stage = "{\"decision\":\"approve\",\"option\":0,\"answer\":null,\"canon\":null,\
                        \"note\":null,\"span_hash\":null}";
    type Case<'a> = (&'a str, Vec<(&'a str, Option<&'a str>)>, &'a str);
    let cases: Vec<Case<'_>> = vec![
        (
            "a base",
            vec![("base_hash", Some(&hash)), ("base_text", Some("old"))],
            "base_hash",
        ),
        (
            "the base hash alone",
            vec![("base_hash", Some(&hash))],
            "base_hash",
        ),
        (
            "the base text alone",
            vec![("base_text", Some("old"))],
            "base_text",
        ),
        (
            "an empty base hash",
            vec![("base_hash", Some(""))],
            "base_hash",
        ),
        ("targets NULL", vec![("target_ids", None)], "target_ids"),
        (
            "not headed by target_id",
            vec![("target_ids", Some("[\"regen-delay/AC-01\"]"))],
            "target_ids",
        ),
        (
            "repeated",
            vec![(
                "target_ids",
                Some(
                    "[\"docs/features/regen-delay.md\",\"regen-delay/AC-01\",\"regen-delay/AC-01\"]",
                ),
            )],
            "target_ids",
        ),
        ("a severity", vec![("severity", Some("high"))], "severity"),
        ("linked", vec![("linked", Some("PR-0002"))], "linked"),
        (
            "a record",
            vec![("record_id", Some("DEC-0024"))],
            "record_id",
        ),
        (
            "a choice",
            vec![("choice", Some("{\"option\":0}"))],
            "choice",
        ),
        (
            "an interpretation",
            vec![(
                "interpretation",
                Some("{\"decision\":\"DEC-0023\",\"how\":\"h\",\"why\":\"w\"}"),
            )],
            "interpretation",
        ),
        (
            "a staged span hash",
            vec![("staged", Some(span_stage)), ("staged_at", Some(T1))],
            "staged",
        ),
        (
            "a staged option",
            vec![("staged", Some(option_stage)), ("staged_at", Some(T1))],
            "staged",
        ),
        ("new_text NULL", vec![("new_text", None)], "new_text"),
    ];
    for (index, (label, edits, named)) in cases.into_iter().enumerate() {
        let mut state = good.clone();
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
    }
    let mut fine = SqliteQueue::open(scratch.db("fine"), PROJECT).expect("open");
    fine.restore(&good).expect("restore");
    assert_eq!(
        fine.get("PR-0001").unwrap().unwrap().kind,
        ProposalKind::Amendment
    );
}

// --------------------------------------------------------- reservations

/// AC-R2, AC-R7 (store half): the amendment's ID is held while `open` and
/// `approved`, freed once applied or rejected; a create's and an
/// amendment's reservations are read together, by proposal number. M:
/// `live_reservations` keyed on `create` alone.
#[test]
fn ac_r2_r7_an_amendment_holds_its_ids_until_applied_or_rejected() {
    let scratch = Scratch::new("qa-held");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    queue
        .create(&create_file("docs/features/first.md", &["first/AC-01"]), T0)
        .expect("a create");
    queue.create(&amendment(), T0).expect("an amendment");
    let mut second = amendment();
    second.target_id = "docs/features/regen-other.md".to_owned();
    second.target_path = second.target_id.clone();
    second.patch_hash = patch_hash(&second.target_id, "", TEXT);
    second.new_ids = vec!["regen-other/AC-01".to_owned()];
    queue.create(&second, T0).expect("a second amendment");
    let open = ProposalStatus::Open;
    assert_eq!(
        held(&queue),
        [
            ("first/AC-01".to_owned(), "PR-0001".to_owned(), open),
            (NEW_ID.to_owned(), "PR-0002".to_owned(), open),
            ("regen-other/AC-01".to_owned(), "PR-0003".to_owned(), open),
        ]
    );
    let approved = queue.approve("PR-0002", &owner(), T1).expect("approved");
    assert_eq!(approved.status, ProposalStatus::Approved);
    assert_eq!(
        held(&queue)[1],
        (
            NEW_ID.to_owned(),
            "PR-0002".to_owned(),
            ProposalStatus::Approved
        )
    );
    queue
        .applied("PR-0002", &"a".repeat(40), T2)
        .expect("applied");
    queue.reject("PR-0003", &owner(), T2).expect("rejected");
    assert_eq!(
        held(&queue),
        [("first/AC-01".to_owned(), "PR-0001".to_owned(), open)],
        "applied and rejected hold nothing"
    );
    // The freed ID is free to a create again.
    queue
        .create(&create_file("docs/features/later.md", &[NEW_ID]), T2)
        .expect("free again");
}

// ---------------------------------------------------------------- stage

/// AC-R8 (store half): a staged approve of an amendment holds no
/// `span_hash` (it is a new file); `option`, `answer`, `canon` and a span
/// hash are refused `Invalid` naming the flag, nothing written; a staged
/// reject is held. `Stage::problem` judges it as a create's new file. M:
/// `new_file()` left `== ProposalKind::Create`.
#[test]
fn ac_r8_an_amendment_stages_an_approve_without_a_span_hash() {
    let scratch = Scratch::new("qa-stage");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    queue.create(&amendment(), T0).expect("an amendment");
    let flagged = |option: Option<u64>, answer: Option<&str>, canon: Option<&str>, span: bool| {
        Stage::Approve {
            option,
            answer: answer.map(str::to_owned),
            canon: canon.map(str::to_owned),
            editorial: false,
            note: None,
            span_hash: span.then(|| "b3:00".to_owned()),
        }
    };
    for (stage, named) in [
        (flagged(Some(0), None, None, false), "option"),
        (flagged(None, Some("yes"), None, false), "answer"),
        (flagged(None, None, Some("MEC-STAMINA"), false), "canon"),
        (flagged(None, None, None, true), "span_hash"),
    ] {
        let problem = stage.problem(ProposalKind::Amendment, true);
        assert!(
            problem
                .as_deref()
                .is_some_and(|text| text.contains(named) && text.contains("an amendment")),
            "{named}: {problem:?}"
        );
        let dump = queue.dump().expect("dump");
        let seen = queue.get("PR-0001").unwrap().unwrap().seen();
        match queue.stage_from("PR-0001", &seen, &stage, T1) {
            Err(QueueError::Invalid(message)) => {
                assert!(message.contains(named), "{named}: {message}");
            }
            other => panic!("{named}: expected Invalid, got {other:?}"),
        }
        assert_eq!(
            queue.dump().expect("dump"),
            dump,
            "{named}: nothing written"
        );
    }
    let approve = flagged(None, None, None, false);
    assert_eq!(approve.problem(ProposalKind::Amendment, true), None);
    let seen = queue.get("PR-0001").unwrap().unwrap().seen();
    let staged = queue
        .stage_from("PR-0001", &seen, &approve, T1)
        .expect("an approve stages");
    let choice = staged.staged.clone().expect("staged");
    assert_eq!(choice.stage, approve);
    assert_eq!(choice.at, T1);
    let reject = Stage::Reject {
        reason: "Absorb it in the rule instead.".to_owned(),
    };
    let staged = queue
        .stage_from("PR-0001", &staged.seen(), &reject, T2)
        .expect("a reject stages");
    assert_eq!(staged.staged.expect("staged").stage, reject);
}

// --------------------------------------------------------------- schema

/// AC-R13 (store half), W28: no schema step: `QUEUE_SCHEMA_VERSION` 6, 44
/// columns, `user_version` 6 after an amendment; an open and an applied
/// amendment dump, restore into a fresh queue and dump byte for byte, the
/// reservation of the open one restored. M: export skipping the kind.
#[test]
fn ac_r13_amendments_keep_schema_6_and_round_trip_byte_identical() {
    assert_eq!(QUEUE_SCHEMA_VERSION, 7);
    assert_eq!(PROPOSAL_COLUMNS.len(), 44);
    let scratch = Scratch::new("qa-schema");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue.create(&amendment(), T0).expect("an amendment");
    let mut open = amendment();
    open.target_id = "docs/features/regen-other.md".to_owned();
    open.target_path = open.target_id.clone();
    open.patch_hash = patch_hash(&open.target_id, "", TEXT);
    open.new_ids = vec!["regen-other/AC-01".to_owned()];
    queue.create(&open, T0).expect("a second amendment");
    queue.approve("PR-0001", &owner(), T1).expect("approved");
    queue
        .applied("PR-0001", &"b".repeat(40), T2)
        .expect("applied");
    let state = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    drop(queue);
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "7");
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "44"
    );
    assert_eq!(
        sqlite3(&db, "SELECT kind, status FROM proposals ORDER BY id;"),
        "amendment|applied\namendment|open"
    );
    assert_eq!(state.proposals.len(), 2);
    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("open");
    fresh.restore(&state).expect("restore");
    assert_eq!(fresh.dump().expect("dump"), dump, "byte-identical");
    assert_eq!(fresh.stored_rows().expect("rows"), state);
    let kinds: Vec<(ProposalKind, ProposalStatus)> = fresh
        .list(&ProposalFilter::default())
        .expect("list")
        .into_iter()
        .map(|proposal| (proposal.kind, proposal.status))
        .collect();
    assert_eq!(
        kinds,
        [
            (ProposalKind::Amendment, ProposalStatus::Applied),
            (ProposalKind::Amendment, ProposalStatus::Open),
        ]
    );
    assert_eq!(
        held(&fresh),
        [(
            "regen-other/AC-01".to_owned(),
            "PR-0002".to_owned(),
            ProposalStatus::Open
        )]
    );
    // A create is still a create beside it (kinds are strings).
    assert_ne!(CREATE_KIND, AMENDMENT_KIND);
}
