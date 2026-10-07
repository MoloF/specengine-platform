//! docs/features/agent-intake.md AC-10 through the CLI library, at the
//! queue schema docs/features/task-package.md makes current (4: "Data",
//! "Backup"): the queue's backup (`format` 2, header `queue_schema` 4, 41
//! proposal columns, no task here) — every kind (an update, an open
//! question, a rejected question, a discrepancy and its linked update)
//! exported and imported fresh, `dump()` equal, the fresh queue's export
//! the same bytes; a format-1 `queue_schema` 1 dump (24 columns) imports,
//! the seventeen later columns `NULL`, and re-exports as format 2, schema
//! 4; a database still at queue schema 1 exports as 4 without stepping,
//! and the first queue command that opens it steps it to 4, its rows kept
//! (a newer schema: `queue_state.rs`'s AC-06; the decided rows and a
//! schema-2 dump: `decision_apply.rs`'s AC-12; tasks and runs:
//! `tasks.rs`'s AC-11).
//!
//! Scratch git repositories of `fixtures/spec-a` (`common::proposal`), a
//! scratch `HOME` per queue, the injected clock, consent through the
//! callback; the version-1 database is made from a real one by the
//! system's `sqlite3` (the seventeen later columns and the `tasks` and
//! `runs` tables dropped, `user_version` 1).
//! Every dump is written under the test's scratch directory.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use common::data_dir;
use common::proposal::{NOW, Pair, cannot, sqlite3};
use serde_json::{Value, json};
use specengine_cli::{
    CliError, DiscrepancyInput, DiscrepancyRequest, Env, Evidence, Exit, ExportStateOutcome,
    ExportStateRequest, GapType, Globals, ImportStateOutcome, ImportStateRequest, IntakeOption,
    IntakeSeverity, ProposedPatch, QuestionRequest, export_state, import_state,
    propose_discrepancy, propose_question,
};
use specengine_store::{PROPOSAL_COLUMNS, ProposalQueue as _, ProposalStatus, SqliteQueue};

/// The eleven columns of queue schema 2, the five of schema 3 and the
/// `task_id` of schema 4, as the dump writes an unbound update's.
const NULL_TAIL: &str = ",\"target_ids\":null,\"severity\":null,\"gap_type\":null,\
\"summary\":null,\"working_answer\":null,\"price_of_other\":null,\"evidence\":null,\
\"options\":null,\"recommendation\":null,\"distinct_from\":null,\"linked\":null,\
\"record_id\":null,\"record_path\":null,\"record_title\":null,\"record_text\":null,\
\"choice\":null,\"task_id\":null}}";

fn env_at(home: &Path, cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        home: Some(home.as_os_str().to_owned()),
        xdg_data_home: None,
    }
}

fn db_of(home: &Path) -> PathBuf {
    data_dir(home).join("lantern-keep.db")
}

fn export_to(pair: &Pair, home: &Path, out: &Path) -> Result<ExportStateOutcome, CliError> {
    export_state(
        &env_at(home, &pair.main),
        &Globals::default(),
        &ExportStateRequest {
            out: Some(out.to_path_buf()),
            now: NOW.to_owned(),
            git: pair.git_env(&pair.main),
        },
    )
}

fn export_ok(pair: &Pair, home: &Path, out: &Path) -> Vec<u8> {
    export_to(pair, home, out).unwrap_or_else(|error| panic!("export state: {error}"));
    fs::read(out).expect("the dump")
}

fn import_into(home: &Path, cwd: &Path, file: &Path) -> Result<ImportStateOutcome, CliError> {
    let mut consent = |_: &str| true;
    import_state(
        &env_at(home, cwd),
        &Globals::default(),
        &ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    )
}

fn import_ok(home: &Path, cwd: &Path, file: &Path) -> ImportStateOutcome {
    let outcome = import_into(home, cwd, file).unwrap_or_else(|error| panic!("import: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome
}

fn dump_of(home: &Path) -> String {
    SqliteQueue::open(db_of(home), "lantern-keep")
        .expect("open")
        .dump()
        .expect("dump")
}

fn lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8(bytes.to_vec())
        .expect("UTF-8 dump")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `sqlite3 <db> <sql>`, which must succeed: its stdout.
fn sql(db: &Path, statement: &str) -> String {
    let output = Command::new(sqlite3())
        .arg(db)
        .arg(statement)
        .stdin(Stdio::null())
        .output()
        .expect("sqlite3 runs");
    assert!(
        output.status.success(),
        "sqlite3 {statement}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn ask(pair: &Pair, ids: &[&str], text: &str) -> String {
    let cwd = &pair.linked;
    let outcome = propose_question(
        &pair.env(cwd),
        &Globals::default(),
        &QuestionRequest {
            node_ids: ids.iter().map(|id| (*id).to_owned()).collect(),
            text: text.to_owned(),
            working_answer: "yes".to_owned(),
            price_of_other: "a rebalance".to_owned(),
            severity: Some(IntakeSeverity::Low),
            distinct_from: Vec::new(),
            author_role: Some("developer".to_owned()),
            author_model: Some("m-1".to_owned()),
            run: Some("r-1".to_owned()),
            now: NOW.to_owned(),
            git: pair.git_env(cwd),
        },
    )
    .expect("ask");
    outcome.document.id.expect("stored")
}

/// A queue of every kind and state the intake adds: `PR-0001` an open
/// update, `PR-0002` an open question, `PR-0003` a question rejected with
/// its answer, `PR-0004` a discrepancy linked to the update `PR-0005`.
fn every_kind(pair: &Pair) {
    pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    assert_eq!(ask(pair, &["EDGE-STAM-ZERO"], "Does it stop?"), "PR-0002");
    assert_eq!(
        ask(pair, &["EDGE-SPRINT-EMPTY", "A-101"], "Is it loud?"),
        "PR-0003"
    );
    let (outcome, _) = pair.reject_answer(&pair.main, "PR-0003", "No.\nQuiet.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    let (hash, text) = pair.span(&pair.linked, "RULE-STAM-REGEN");
    let outcome = propose_discrepancy(
        &pair.env(&pair.linked),
        &Globals::default(),
        &DiscrepancyRequest {
            input: DiscrepancyInput {
                node_ids: vec!["RULE-STAM-REGEN".to_owned()],
                summary: "Regenerates while walking \"fast\".".to_owned(),
                gap_type: GapType::Unrequested,
                severity: IntakeSeverity::High,
                evidence: vec![Evidence {
                    file: "src/a.rs".to_owned(),
                    qpath: None,
                    lines: Some("7".to_owned()),
                    observed: "walks".to_owned(),
                    documented: "rests".to_owned(),
                }],
                options: (0..3)
                    .map(|index| IntakeOption {
                        label: format!("o{index}"),
                        effect: "e".to_owned(),
                        price: "p".to_owned(),
                    })
                    .collect(),
                recommendation: 2,
                working_answer: Some("rest".to_owned()),
                proposed_patch: Some(ProposedPatch {
                    target: "RULE-STAM-REGEN".to_owned(),
                    base: hash,
                    text: text.replacen("(R-12).", "(R-12, R-99).", 1),
                    rationale: "Cite it.".to_owned(),
                }),
                distinct_from: Some(vec!["DEC-0023".to_owned()]),
            },
            author_role: Some("developer".to_owned()),
            author_model: None,
            run: None,
            now: NOW.to_owned(),
            git: pair.git_env(&pair.linked),
        },
    )
    .expect("report");
    assert_eq!(outcome.document.id.as_deref(), Some("PR-0004"));
    assert_eq!(outcome.document.linked.as_deref(), Some("PR-0005"));
}

/// The text of every key `"<column>":` appears in `line` in `columns`
/// order, and no other column.
fn assert_columns(line: &str, columns: &[&str]) {
    let value: Value = serde_json::from_str(line).expect("a JSON row");
    let row = value["proposals"].as_object().expect("a proposals row");
    assert_eq!(row.len(), columns.len(), "{line}");
    let mut last = 0;
    for column in columns {
        let key = format!("\"{column}\":");
        let at = line[last..]
            .find(&key)
            .unwrap_or_else(|| panic!("{key} after byte {last} in {line}"));
        last += at + key.len();
    }
}

// ------------------------------------------------------------------ AC-10

/// AC-10: every kind exported (header format 2, `queue_schema` 4, no
/// task, each row the 41 columns in table order, a question's and a
/// discrepancy's fields as stored, an undecided one's record columns and
/// an unbound one's `task_id` `null`), imported into a fresh queue:
/// `dump()` equal, every row read back alike, and the fresh queue's export
/// byte-identical. M: a column left out.
#[test]
fn ac10_every_kind_round_trips_at_schema_4() {
    let pair = Pair::new("ai-ac10", "spec-a");
    every_kind(&pair);
    let before = dump_of(&pair.home);
    let dumps = pair.scratch.dir("dumps");
    let bytes = export_ok(&pair, &pair.home, &dumps.join("all.jsonl"));
    let rows = lines(&bytes);
    assert_eq!(
        rows[0],
        "{\"format\":2,\"queue_schema\":4,\"project\":\"lantern-keep\",\"proposals\":5,\
         \"tasks\":0,\"runs\":0,\"events\":6}"
    );
    assert_eq!(PROPOSAL_COLUMNS.len(), 41);
    for row in &rows[1..6] {
        assert_columns(row, &PROPOSAL_COLUMNS);
        let value: Value = serde_json::from_str(row).unwrap();
        for column in &PROPOSAL_COLUMNS[35..] {
            assert!(value["proposals"][column].is_null(), "{column}: {row}");
        }
    }
    assert!(rows[1].ends_with(NULL_TAIL), "an update's: {}", rows[1]);
    let question: Value = serde_json::from_str(&rows[3]).unwrap();
    assert_eq!(question["proposals"]["kind"], json!("question"));
    assert_eq!(
        question["proposals"]["target_ids"],
        json!("[\"EDGE-SPRINT-EMPTY\",\"A-101\"]")
    );
    assert_eq!(question["proposals"]["severity"], json!("low"));
    assert_eq!(question["proposals"]["decision_note"], json!("No.\nQuiet."));
    assert!(question["proposals"]["base_text"].is_null());
    assert!(question["proposals"]["evidence"].is_null());
    let reported: Value = serde_json::from_str(&rows[4]).unwrap();
    assert_eq!(reported["proposals"]["recommendation"], json!("2"));
    assert_eq!(reported["proposals"]["linked"], json!("PR-0005"));
    assert_eq!(reported["proposals"]["gap_type"], json!("unrequested"));
    assert_eq!(
        reported["proposals"]["distinct_from"],
        json!("[\"DEC-0023\"]")
    );
    let linked: Value = serde_json::from_str(&rows[5]).unwrap();
    assert_eq!(linked["proposals"]["kind"], json!("update"));
    assert_eq!(linked["proposals"]["linked"], json!("PR-0004"));

    let fresh = pair.scratch.home("fresh");
    let outcome = import_ok(&fresh, &pair.main, &dumps.join("all.jsonl"));
    assert_eq!((outcome.proposals, outcome.events), (5, 6));
    assert_eq!(dump_of(&fresh), before, "dump() equal");
    let original = pair.queue();
    let restored = SqliteQueue::open(db_of(&fresh), "lantern-keep").expect("open");
    for number in 1..=5 {
        let id = format!("PR-{number:04}");
        assert_eq!(
            restored.get(&id).expect("get"),
            original.get(&id).expect("get"),
            "{id}"
        );
    }
    assert_eq!(
        restored.get("PR-0003").unwrap().unwrap().status,
        ProposalStatus::Rejected
    );
    let again = export_ok(&pair, &fresh, &dumps.join("again.jsonl"));
    assert_eq!(again, bytes, "the fresh queue's export is the same file");
}

/// AC-10: a format-1 `queue_schema` 1 dump (its 24 columns) imports into
/// a fresh queue, the seventeen later columns `NULL`; its export is a
/// format-2 `queue_schema` 4 dump, equal to the current export of the same
/// queue; a schema-1 row carrying a later column, or a schema-4 row
/// without them, is refused naming its line. M: schema 1 refused; a
/// column left out.
#[test]
fn ac10_a_schema_1_dump_imports_and_re_exports_as_4() {
    let pair = Pair::new("ai-ac10-v1", "spec-a");
    pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let rejected = pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint stops;",
    );
    let (outcome, _) = pair.reject_answer(&pair.main, &rejected, "No.", true);
    assert_eq!(outcome.expect("reject").exit(), Exit::Answered);
    let dumps = pair.scratch.dir("dumps");
    let v3 = export_ok(&pair, &pair.home, &dumps.join("v3.jsonl"));
    let v3_lines = lines(&v3);
    assert!(
        v3_lines[0].starts_with("{\"format\":2,\"queue_schema\":4,")
            && v3_lines[0].contains(",\"tasks\":0,\"runs\":0,"),
        "{}",
        v3_lines[0]
    );
    let mut v1_lines = v3_lines.clone();
    v1_lines[0] = v1_lines[0]
        .replacen(
            "\"format\":2,\"queue_schema\":4,",
            "\"format\":1,\"queue_schema\":1,",
            1,
        )
        .replacen("\"tasks\":0,\"runs\":0,", "", 1);
    for line in &mut v1_lines[1..3] {
        assert!(line.ends_with(NULL_TAIL), "{line}");
        *line = format!("{}}}}}", &line[..line.len() - NULL_TAIL.len()]);
        assert_columns(line, &PROPOSAL_COLUMNS[..24]);
    }
    let v1 = format!("{}\n", v1_lines.join("\n"));
    let v1_file = dumps.join("v1.jsonl");
    fs::write(&v1_file, &v1).unwrap();

    let fresh = pair.scratch.home("fresh");
    let outcome = import_ok(&fresh, &pair.main, &v1_file);
    assert_eq!((outcome.proposals, outcome.events), (2, 3));
    assert_eq!(dump_of(&fresh), dump_of(&pair.home), "the seventeen NULL");
    let again = export_ok(&pair, &fresh, &dumps.join("again.jsonl"));
    assert_eq!(again, v3, "re-exported as format 2, queue_schema 4");

    // A schema-1 row with the later columns; a schema-4 row without them.
    let mut mixed = v1_lines.clone();
    mixed[2] = v3_lines[2].clone();
    let mut short = v3_lines.clone();
    short[2] = v1_lines[2].clone();
    for (name, bad) in [("v1 with 41", mixed), ("v4 with 24", short)] {
        let file = dumps.join(format!("{}.jsonl", name.replace(' ', "-")));
        fs::write(&file, format!("{}\n", bad.join("\n"))).unwrap();
        let home = pair
            .scratch
            .home(&format!("refused-{}", name.replace(' ', "-")));
        let message = cannot(&import_into(&home, &pair.main, &file), name);
        assert!(
            message.contains(&format!("{}:3: ", file.display())),
            "{name}: {message}"
        );
        assert!(!data_dir(&home).exists(), "{name}: no queue made");
    }
}

/// AC-10: a database still at queue schema 1 (a real queue's seventeen
/// later columns and its `tasks` and `runs` tables dropped, its
/// `user_version` 1) exports as format 2, `queue_schema` 4 without
/// stepping (the seventeen `null`, the bytes of the current export); the
/// first queue command that opens it (`spec inbox`) steps it to 4 in
/// place: its rows kept, the seventeen `NULL`, both tables made.
#[test]
fn ac10_a_version_1_database_is_stepped_when_opened() {
    let pair = Pair::new("ai-ac10-db", "spec-a");
    pair.propose_edit(
        &pair.linked,
        "EDGE-SPRINT-EMPTY",
        "the sprint ends;",
        "the sprint ends at once;",
    );
    let dumps = pair.scratch.dir("dumps");
    let v3 = export_ok(&pair, &pair.home, &dumps.join("v3.jsonl"));
    let before = dump_of(&pair.home);
    let db = pair.db();
    let drops: Vec<String> = PROPOSAL_COLUMNS[24..]
        .iter()
        .map(|column| format!("ALTER TABLE proposals DROP COLUMN {column};"))
        .collect();
    sql(
        &db,
        &format!(
            "{} DROP TABLE tasks; DROP TABLE runs; PRAGMA user_version = 1;",
            drops.join(" ")
        ),
    );
    assert_eq!(sql(&db, "PRAGMA user_version;"), "1");
    assert_eq!(
        sql(&db, "SELECT count(*) FROM pragma_table_info('proposals');"),
        "24"
    );

    let exported = export_ok(&pair, &pair.home, &dumps.join("from-v1.jsonl"));
    assert_eq!(
        exported, v3,
        "a version-1 DB exports as format 2, queue_schema 4"
    );
    assert_eq!(
        sql(&db, "PRAGMA user_version;"),
        "1",
        "export steps nothing"
    );

    let inbox = pair.inbox(&pair.main, false).expect("inbox");
    assert_eq!(inbox.proposals.len(), 1, "{inbox:?}");
    assert_eq!(sql(&db, "PRAGMA user_version;"), "4");
    let columns = sql(&db, "SELECT name FROM pragma_table_info('proposals');");
    assert_eq!(
        columns.lines().collect::<Vec<_>>(),
        PROPOSAL_COLUMNS,
        "steps 2, 3 and 4 append the seventeen in order"
    );
    assert_eq!(
        sql(
            &db,
            "SELECT name FROM sqlite_master WHERE name IN ('tasks', 'runs') ORDER BY name;"
        ),
        "runs\ntasks",
        "step 4 makes both tables"
    );
    assert_eq!(dump_of(&pair.home), before, "rows kept, the seventeen NULL");
}
