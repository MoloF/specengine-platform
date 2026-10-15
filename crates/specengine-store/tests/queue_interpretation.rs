//! The store half of docs/features/kind-interpretation.md (AC-R1, AC-R5,
//! AC-T5; "Data"):
//!
//! - AC-R1: `ProposalKind::Interpretation` is the fifth of `ALL`, named by
//!   core's `INTERPRETATION_KIND`, its `family()` `Applies` (drift-queue);
//!   `create` stores it with its `{decision, how, why}` (the column the
//!   compact JSON, keys in "Data"'s order), `create_intake` refuses it
//!   (`Invalid`); the fields are required on it and refused on any other
//!   kind; a blank value or a `how`/`why` over 2 048 bytes is `Invalid`,
//!   2 048 is stored. M: its `family()` `Decides`.
//! - AC-R5 / AC-T5: a schema-5 queue (as decision-staging's build left it)
//!   is read unstepped at 46 columns, the last three `None`, and opens as 8
//!   (drift-queue's schema), the column `NULL`, its rows kept; `user_version`
//!   9 is `SchemaTooNew` for
//!   `open` and `open_existing`, left alone. M: step 6 omitted;
//!   `user_version` left 5.
//! - AC-R5, corrupt rows: the column on an update, `NULL` on an
//!   interpretation, another shape (a key missing, added, reordered, a
//!   value not a string, not compact), a blank value, a `how` over 2 048
//!   bytes: `get` and `list` fail naming the row and the column,
//!   `list_readable` skips it naming it.
//! - The backup's rows: `dump`, `restore` into a fresh queue, `dump`
//!   byte-identical with an interpretation.

mod common;

use std::path::Path;
use std::process::{Command, Stdio};

use common::{Scratch, blake3_hex};
use specengine_core::intake::{
    INTERPRETATION_FIELDS, INTERPRETATION_KIND, INTERPRETATION_TEXT_MAX, IntakeSeverity,
};
use specengine_core::proposal::Author;
use specengine_store::{
    Intake, Interpretation, KindFamily, NewIntake, NewProposal, PROPOSAL_COLUMNS, Place,
    ProposalFilter, ProposalKind, ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION,
    QueueError, SqliteQueue, patch_hash, proposal_columns,
};

const T0: &str = "2026-10-08T09:00:00Z";
const T1: &str = "2026-10-08T09:14:02Z";
const PROJECT: &str = "lantern-keep";
const STAMINA: &str = "docs/spec/movement/stamina.md";

fn place() -> Place {
    Place {
        git_common_dir: "/r/.git".to_owned(),
        worktree: "/r/t1".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn agent() -> Author {
    Author::new(Some("spec-author".to_owned()), None, None).expect("an agent")
}

fn interpretation(how: &str, why: &str) -> Interpretation {
    Interpretation {
        decision: "DEC-0023".to_owned(),
        how: how.to_owned(),
        why: why.to_owned(),
    }
}

/// "Data"'s example: rest is a speed under 40 % of walking.
fn example() -> Interpretation {
    interpretation(
        "Rest is any speed under 40 % of walking.",
        "DEC-0023 says rest; the rule names no speed.",
    )
}

fn of_kind(kind: ProposalKind, target: &str, carries: Option<Interpretation>) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(b"base"));
    NewProposal {
        kind,
        target_id: target.to_owned(),
        target_path: STAMINA.to_owned(),
        place: place(),
        patch_hash: patch_hash(target, &base_hash, "new"),
        base_hash: Some(base_hash),
        base_text: Some("base".to_owned()),
        new_text: "new".to_owned(),
        rationale: "Carry DEC-0023 into the rule.".to_owned(),
        author: agent(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: carries,
    }
}

fn interpreting(target: &str) -> NewProposal {
    of_kind(ProposalKind::Interpretation, target, Some(example()))
}

fn update(target: &str) -> NewProposal {
    of_kind(ProposalKind::Update, target, None)
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

fn column_at(name: &str) -> usize {
    PROPOSAL_COLUMNS
        .iter()
        .position(|column| *column == name)
        .unwrap_or_else(|| panic!("no column {name}"))
}

fn invalid(result: Result<impl std::fmt::Debug, QueueError>, context: &str) -> String {
    match result {
        Err(QueueError::Invalid(reason)) => reason,
        other => panic!("{context}: expected Invalid, got {other:?}"),
    }
}

// ------------------------------------------------------------------ AC-R1

/// AC-R1: the kind, its two tests, `create` stores it with its column,
/// `create_intake` refuses it. M: `decides()` true.
#[test]
fn ac_r1_an_interpretation_applies_decides_nothing_and_is_stored_by_create() {
    assert_eq!(INTERPRETATION_KIND, "interpretation");
    assert_eq!(INTERPRETATION_FIELDS, ["decision", "how", "why"]);
    assert_eq!(INTERPRETATION_TEXT_MAX, 2048);
    // Six since docs/features/kind-decision.md appends `decision`; seven
    // since docs/features/kind-amendment.md appends `amendment`; eight
    // since docs/features/drift-queue.md appends `drift`.
    assert_eq!(ProposalKind::ALL.len(), 8);
    assert_eq!(ProposalKind::ALL[4], ProposalKind::Interpretation);
    let kind = ProposalKind::Interpretation;
    assert_eq!(kind.as_str(), INTERPRETATION_KIND);
    assert_eq!(ProposalKind::parse(INTERPRETATION_KIND), Some(kind));
    assert_eq!(
        kind.family(),
        KindFamily::Applies,
        "an interpretation applies, writes no record"
    );
    // The other kinds as before.
    assert_eq!(ProposalKind::Update.family(), KindFamily::Applies);
    assert_eq!(ProposalKind::Question.family(), KindFamily::Decides);

    let scratch = Scratch::new("qi-r1");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let stored = queue
        .create(&interpreting("RULE-STAM-REGEN"), T0)
        .expect("create stores an interpretation");
    assert_eq!(stored.id, "PR-0001");
    assert_eq!(stored.kind, ProposalKind::Interpretation);
    assert_eq!(stored.status, ProposalStatus::Open);
    assert_eq!(stored.interpretation, Some(example()));
    assert_eq!(stored.record, None);
    assert_eq!(queue.get("PR-0001").unwrap().unwrap(), stored);
    // The column: "Data"'s compact JSON, keys in order.
    assert_eq!(
        sqlite3(
            &db,
            "SELECT kind || '|' || interpretation FROM proposals WHERE id = 'PR-0001';"
        ),
        "interpretation|{\"decision\":\"DEC-0023\",\"how\":\"Rest is any speed under 40 % of \
         walking.\",\"why\":\"DEC-0023 says rest; the rule names no speed.\"}"
    );
    // An update beside it: the column NULL, its `interpretation` None.
    let plain = queue.create(&update("EDGE-STAM-ZERO"), T0).expect("update");
    assert_eq!(plain.interpretation, None);
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE id = 'PR-0002' AND interpretation IS NULL;"
        ),
        "1"
    );
    let listed = queue.list(&ProposalFilter::default()).expect("list");
    assert_eq!(listed.len(), 2);

    // `create_intake` stores no kind that applies.
    let intake = NewIntake {
        kind: ProposalKind::Interpretation,
        target_path: STAMINA.to_owned(),
        place: place(),
        author: agent(),
        intake: Intake {
            target_ids: vec!["RULE-STAM-REGEN".to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: "Is rest a speed?".to_owned(),
            working_answer: Some("Under 40 %.".to_owned()),
            price_of_other: Some("A rebalance.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
        },
    };
    let reason = invalid(
        queue.create_intake(&intake, &[], None, T0),
        "create_intake of an interpretation",
    );
    assert!(reason.contains("an interpretation"), "{reason}");
    assert_eq!(queue.list(&ProposalFilter::default()).unwrap().len(), 2);
}

/// AC-R1, `NewProposal.interpretation` "set iff that kind, else
/// `Invalid`" ("Data"); a blank value or a text over the cap `Invalid`,
/// 2 048 bytes stored; an interpretation without its base or with new IDs
/// `Invalid`. Nothing written by a refusal.
#[test]
fn ac_r1_the_fields_go_with_an_interpretation_only_within_their_caps() {
    let scratch = Scratch::new("qi-r1-fields");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let at_cap = "h".repeat(INTERPRETATION_TEXT_MAX);
    let over = "w".repeat(INTERPRETATION_TEXT_MAX + 1);
    let mut no_base = interpreting("RULE-STAM-REGEN");
    no_base.base_hash = None;
    no_base.base_text = None;
    let mut new_ids = interpreting("RULE-STAM-REGEN");
    new_ids.new_ids = vec!["RULE-STAM-REST".to_owned()];
    let cases: Vec<(&str, NewProposal)> = vec![
        (
            "an interpretation without its fields",
            of_kind(ProposalKind::Interpretation, "RULE-STAM-REGEN", None),
        ),
        (
            "an update carrying them",
            of_kind(ProposalKind::Update, "RULE-STAM-REGEN", Some(example())),
        ),
        (
            "a create carrying them",
            NewProposal {
                new_ids: vec!["RULE-STAM-REST".to_owned()],
                ..of_kind(ProposalKind::Create, "RULE-STAM-REGEN", Some(example()))
            },
        ),
        (
            "a blank decision",
            of_kind(
                ProposalKind::Interpretation,
                "RULE-STAM-REGEN",
                Some(Interpretation {
                    decision: " ".to_owned(),
                    ..example()
                }),
            ),
        ),
        (
            "a blank how",
            of_kind(
                ProposalKind::Interpretation,
                "RULE-STAM-REGEN",
                Some(interpretation("\n\t ", "why")),
            ),
        ),
        (
            "a why over the cap",
            of_kind(
                ProposalKind::Interpretation,
                "RULE-STAM-REGEN",
                Some(interpretation("how", &over)),
            ),
        ),
        ("an interpretation without its base", no_base),
        ("an interpretation adding an ID", new_ids),
    ];
    for (label, proposal) in cases {
        let reason = invalid(queue.create(&proposal, T0), label);
        assert!(!reason.is_empty(), "{label}");
    }
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM proposals;"),
        "0",
        "nothing written"
    );
    let stored = queue
        .create(
            &of_kind(
                ProposalKind::Interpretation,
                "RULE-STAM-REGEN",
                Some(interpretation(&at_cap, &at_cap)),
            ),
            T0,
        )
        .expect("2 048 bytes are stored");
    let read = queue.get(&stored.id).unwrap().unwrap();
    let carried = read.interpretation.expect("its interpretation");
    assert_eq!(carried.how.len(), INTERPRETATION_TEXT_MAX);
    assert_eq!(carried.why.len(), INTERPRETATION_TEXT_MAX);
}

/// A U+202E and a control in `how` are stored and read raw (escaping is
/// the text output's, not the store's).
#[test]
fn ac_r4_controls_are_stored_raw() {
    let scratch = Scratch::new("qi-raw");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    let raw = interpretation("rest \u{202e}tluaf\u{202c} at 40 %\nsecond line", "why\tso");
    let stored = queue
        .create(
            &of_kind(
                ProposalKind::Interpretation,
                "RULE-STAM-REGEN",
                Some(raw.clone()),
            ),
            T0,
        )
        .expect("stored");
    assert_eq!(
        queue.get(&stored.id).unwrap().unwrap().interpretation,
        Some(raw)
    );
}

// --------------------------------------------------------- AC-R5, AC-T5

/// AC-R5 / AC-T5: schema 6, 44 columns, `interpretation` last; a schema-5
/// database (its column dropped) is read unstepped with the column `None`,
/// opens as 6 with the column `NULL` and its rows kept, stepped once;
/// `user_version` 7 refused by `open` and `open_existing`, left alone.
/// M: step 6 omitted; `user_version` left 5.
#[test]
fn ac_t5_a_schema_5_queue_opens_as_6_and_7_is_too_new() {
    // Schema 8, 46 columns since docs/features/drift-queue.md "Queue
    // schema 8": `drift`, `drift_hash` after `interpretation`.
    assert_eq!(QUEUE_SCHEMA_VERSION, 8);
    assert_eq!(PROPOSAL_COLUMNS.len(), 46);
    assert_eq!(PROPOSAL_COLUMNS[43], INTERPRETATION_KIND);
    assert_eq!(proposal_columns(5), Some(&PROPOSAL_COLUMNS[..43]));
    assert_eq!(proposal_columns(6), Some(&PROPOSAL_COLUMNS[..44]));
    // Schema 7 adds no column (docs/features/rev-discipline.md "Review
    // document, stage, queue").
    assert_eq!(proposal_columns(7), Some(&PROPOSAL_COLUMNS[..44]));
    assert_eq!(proposal_columns(8), Some(&PROPOSAL_COLUMNS[..]));
    assert_eq!(proposal_columns(9), None);

    let scratch = Scratch::new("qi-t5");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue.create(&update("EDGE-STAM-ZERO"), T0).expect("update");
    queue
        .create(&update("RULE-STAM-REGEN"), T0)
        .expect("update");
    let made = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    drop(queue);
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8");
    assert_eq!(
        sqlite3(
            &db,
            "SELECT name || ' ' || type FROM pragma_table_info('proposals') WHERE cid = 43;"
        ),
        "interpretation TEXT"
    );

    // The database as decision-staging's build left it: schema 5 (step
    // 8's two columns dropped too).
    sqlite3(
        &db,
        "ALTER TABLE proposals DROP COLUMN drift_hash; \
         ALTER TABLE proposals DROP COLUMN drift; \
         ALTER TABLE proposals DROP COLUMN interpretation; PRAGMA user_version = 5;",
    );
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "43"
    );
    let read = SqliteQueue::open_existing(&db, PROJECT)
        .expect("open_existing")
        .expect("the file exists");
    let unstepped = read.stored_rows().expect("rows of a schema-5 DB");
    drop(read);
    assert_eq!(
        sqlite3(&db, "PRAGMA user_version;"),
        "5",
        "a read steps nothing"
    );
    assert_eq!(unstepped, made, "read at 46 columns, the last three None");
    assert!(
        unstepped
            .proposals
            .iter()
            .all(|row| row.columns[43..].iter().all(Option::is_none))
    );

    let queue = SqliteQueue::open(&db, PROJECT).expect("opens, stepping 5 -> 8");
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8");
    assert_eq!(
        sqlite3(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "46"
    );
    assert_eq!(
        sqlite3(
            &db,
            "SELECT count(*) FROM proposals WHERE interpretation IS NULL;"
        ),
        "2"
    );
    assert_eq!(queue.dump().expect("dump"), dump, "rows kept");
    for id in ["PR-0001", "PR-0002"] {
        assert_eq!(queue.get(id).unwrap().unwrap().interpretation, None);
    }
    drop(queue);
    drop(SqliteQueue::open(&db, PROJECT).expect("reopen"));
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "8", "stepped once");
    // An interpretation goes into the stepped queue.
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    assert_eq!(
        queue
            .create(&interpreting("RULE-STAM-REGEN"), T1)
            .unwrap()
            .id,
        "PR-0003"
    );
    drop(queue);

    // Schema 9: refused by an open and a read, left alone.
    sqlite3(&db, "PRAGMA user_version = 9;");
    let rows = sqlite3(&db, "SELECT count(*) FROM proposals;");
    for result in [
        SqliteQueue::open(&db, PROJECT).map(|_| ()),
        SqliteQueue::open_existing(&db, PROJECT).map(|_| ()),
    ] {
        match result {
            Err(error @ QueueError::SchemaTooNew { found: 9 }) => {
                let text = error.to_string();
                assert!(text.contains('9') && text.contains('8'), "{text}");
            }
            other => panic!("expected SchemaTooNew 9, got {other:?}"),
        }
    }
    assert_eq!(sqlite3(&db, "PRAGMA user_version;"), "9", "left alone");
    assert_eq!(sqlite3(&db, "SELECT count(*) FROM proposals;"), rows);
}

/// "Data", the backup's rows: a queue holding an interpretation and an
/// update dumps, restores into a fresh queue raw, and dumps
/// byte-identical; the restored interpretation reads equal.
#[test]
fn ac_r13_rows_with_an_interpretation_restore_byte_identical() {
    let scratch = Scratch::new("qi-restore");
    let mut queue = SqliteQueue::open(scratch.db("q"), PROJECT).expect("open");
    queue.create(&interpreting("RULE-STAM-REGEN"), T0).unwrap();
    queue.create(&update("EDGE-STAM-ZERO"), T0).unwrap();
    let state = queue.stored_rows().expect("rows");
    let dump = queue.dump().expect("dump");
    let first = dump
        .lines()
        .find(|line| line.starts_with("proposals\t"))
        .expect("a proposals line");
    // The column before step 8's two (`drift`, `drift_hash`, null on an
    // interpretation), its JSON text a string of the dump's array.
    let column = serde_json::to_string(&example()).expect("JSON");
    let tail = format!(",{},null,null]", serde_json::Value::String(column));
    assert!(first.ends_with(&tail), "{first} ends with {tail}");
    let mut fresh = SqliteQueue::open(scratch.db("fresh"), PROJECT).expect("open");
    fresh.restore(&state).expect("restore");
    assert_eq!(fresh.dump().expect("dump"), dump, "byte-identical");
    assert_eq!(
        fresh.get("PR-0001").unwrap().unwrap(),
        queue.get("PR-0001").unwrap().unwrap()
    );
}

// --------------------------------------------------------- corrupt rows

/// A corrupt-row case: its label, the ID, the `interpretation` column (or
/// another edit), the column the refusal names.
type CorruptCase<'a> = (&'a str, &'a str, Vec<(&'a str, Option<String>)>, &'a str);

/// AC-R5: the column on another kind or `NULL` on this one; another shape
/// (a key missing, extra, reordered, not a string, not compact); a blank
/// value; `how`, `why` over 2 048 bytes: `get` and `list` fail naming the
/// row and `interpretation`; `list_readable` skips it naming it. The good
/// rows read.
#[test]
fn ac_r5_a_corrupt_interpretation_column_is_named() {
    let scratch = Scratch::new("qi-corrupt");
    let db = scratch.db("q");
    let mut queue = SqliteQueue::open(&db, PROJECT).expect("open");
    queue.create(&interpreting("RULE-STAM-REGEN"), T0).unwrap();
    queue.create(&update("EDGE-STAM-ZERO"), T0).unwrap();
    let state = queue.stored_rows().expect("rows");
    drop(queue);
    let row_of = |id: &str| {
        let mut one = state.clone();
        one.proposals.retain(|row| row.id() == Some(id));
        one.events.clear();
        one
    };
    let set = |text: &str| Some(text.to_owned());
    let long = "x".repeat(INTERPRETATION_TEXT_MAX + 1);
    let over_how = format!("{{\"decision\":\"DEC-0023\",\"how\":\"{long}\",\"why\":\"w\"}}");
    let over_why = format!("{{\"decision\":\"DEC-0023\",\"how\":\"h\",\"why\":\"{long}\"}}");
    let column = INTERPRETATION_KIND;
    let cases: Vec<CorruptCase<'_>> = vec![
        (
            "the column on an update",
            "PR-0002",
            vec![(
                column,
                set("{\"decision\":\"DEC-0023\",\"how\":\"h\",\"why\":\"w\"}"),
            )],
            column,
        ),
        (
            "NULL on an interpretation",
            "PR-0001",
            vec![(column, None)],
            column,
        ),
        (
            "a key missing",
            "PR-0001",
            vec![(column, set("{\"decision\":\"DEC-0023\",\"how\":\"h\"}"))],
            column,
        ),
        (
            "a key added",
            "PR-0001",
            vec![(
                column,
                set("{\"decision\":\"DEC-0023\",\"how\":\"h\",\"why\":\"w\",\"link\":\"x\"}"),
            )],
            column,
        ),
        (
            "keys reordered",
            "PR-0001",
            vec![(
                column,
                set("{\"how\":\"h\",\"decision\":\"DEC-0023\",\"why\":\"w\"}"),
            )],
            column,
        ),
        (
            "a value not a string",
            "PR-0001",
            vec![(column, set("{\"decision\":23,\"how\":\"h\",\"why\":\"w\"}"))],
            column,
        ),
        (
            "not compact",
            "PR-0001",
            vec![(
                column,
                set("{\"decision\": \"DEC-0023\", \"how\": \"h\", \"why\": \"w\"}"),
            )],
            column,
        ),
        (
            "not JSON",
            "PR-0001",
            vec![(column, set("DEC-0023"))],
            column,
        ),
        (
            "a blank how",
            "PR-0001",
            vec![(
                column,
                set("{\"decision\":\"DEC-0023\",\"how\":\" \",\"why\":\"w\"}"),
            )],
            column,
        ),
        (
            "a blank decision",
            "PR-0001",
            vec![(
                column,
                set("{\"decision\":\"\",\"how\":\"h\",\"why\":\"w\"}"),
            )],
            column,
        ),
        (
            "how over 2048 bytes",
            "PR-0001",
            vec![(column, Some(over_how))],
            column,
        ),
        (
            "why over 2048 bytes",
            "PR-0001",
            vec![(column, Some(over_why))],
            column,
        ),
    ];
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
            Ok(found) => panic!("{label}: read as {found:?}"),
        };
        assert!(
            message.contains(id) && message.contains(&format!("`{named}`")),
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
        assert_eq!(listed.unreadable[0].column, named, "{label}");
    }
    // The rows themselves read.
    let mut good = SqliteQueue::open(scratch.db("good"), PROJECT).expect("open");
    good.restore(&state).expect("restore");
    assert_eq!(
        good.get("PR-0001").unwrap().unwrap().interpretation,
        Some(example())
    );
    assert_eq!(good.get("PR-0002").unwrap().unwrap().interpretation, None);
}
