//! docs/features/health-code.md (9a-1): `spec health`'s rows 4-5 over the
//! code index the call refreshes, as `spec refs` reads it (ADR-0051); the
//! CLI half of RD-1..RD-11, AC-1..AC-7 and TE-1..TE-5 (RD-10 and AC-9 are
//! the HTTP crate's; TE-7, the pilots, `health_code_pilot.rs`).
//!
//! Corpora, every one a scratch copy (never `fixtures/` itself):
//! `fixtures/code-a` (a counter library: tally), `code-index` (sheets and
//! their marks: inkwell), `code-bevy` (an ECS farm game, `[code]
//! frameworks = ["bevy"]`), and `spec-a`, `spec-c` (no `[code]`). TE-1's
//! tree ([`Tally`]) is code-a with `@assumes` markers appended to
//! `crates/tally/src/lib.rs` (lines 22-35), one inside a function body
//! (`crates/tally/src/assumed.rs:4`), three decision documents (`PREM-002`
//! superseded by `PREM-003`, `PREM-004` rejected), the golden `spec.lock`,
//! one commit; and its queue seeded through the store ([`seed_queue`]):
//! `PR-0001` a question open, `PR-0002` a question approved on its working
//! answer, `PR-0003` a question applied on another answer, `PR-0004` and
//! `PR-0005` discrepancies whose options are labelled `"1"`, `"0"` with
//! recommendation 0 (index 1 applied, index 0 approved: a label compare
//! flips both), `PR-0006` a question rejected, `PR-0007` an update applied,
//! `PR-0008` an update approved, `PR-0009` a question open on `LAW-SOURCE`;
//! `PR-7` (`lib.rs:13`) names none of them.
//!
//! Oracles: the spec's own examples (AC-3's tree), `spec refs <node>
//! --json` per selected node (AC-4's parity: a node is bound for a rule iff
//! its block lists a marker of the rule's relations, its `id_state`
//! `resolved`, `alias` or `ambiguous`, its `binding` neither `unanchored`
//! nor `cannot_verify`), `spec unmapped <file> --json` (TE-2), the failing
//! step's own CLI line (`spec refs` exit 2, "Failures and exits"), `spec
//! check --json` `counts.documents` (row 3).
//!
//! PHASE A: written from the spec before the code exists. The API is the
//! spec's ("Data", "The shared predicate"); a name it leaves to the
//! developer is marked PHASE-B ([`PHASE_B_SOURCES`]).
//!
//! docs/features/health-drift.md (9b-1) extends these pins to row 6: the
//! document's keys, the `off` and shared `cannot` rows, the text's order
//! (RD-3), RD-2's scan (`verify_as` never called), RD-4's probe over the
//! example tree, RD-8's byte identity with row 6 measured (the copies
//! carry `.git`). Row 6 itself: `health_drift`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use specengine_core::drift::{DriftEvidence, DriftLock, DriftMarker, DriftRow};
use specengine_core::intake::{Evidence, GapType, IntakeOption, IntakeSeverity};
use specengine_core::proposal::Author;
use specengine_store::{
    ApplyFailure, Choice, Decision, DriftResult, Intake, NewDrift, NewIntake, NewProposal, Place,
    ProposalKind, ProposalQueue as _, RecordApproval, RecordSeries, SqliteQueue, patch_hash,
};

use super::health_drift::DRIFT_KEYS;
use super::{J, NO_CODE, check_documents, health_json, health_run};
use crate::common::bundle::blake3_hex;
use crate::common::git::Sandbox;
use crate::common::proposal::sqlite3;
use crate::common::{
    Run, Scratch, data_dir, index, read, read_text, replace, repository_root, snapshot, spec,
    spec_with, with_ids_line, write,
};

// ------------------------------------------------------------ the words

/// Row 5 without a rule ("`[[health.bound]]`").
const NO_RULE: &str =
    "no [[health.bound]] rule: which nodes must be bound is the project's to declare";

/// "The document": Health's keys since 9a-1, in order;
/// docs/features/health-drift.md "Data": `drift` the 7th.
const DOCUMENT_KEYS: [&str; 7] = [
    "documents",
    "amendments",
    "notes",
    "code",
    "assumes",
    "unbound",
    "drift",
];
/// `HealthAssumes`, `HealthUnbound`.
const ROW_KEYS: [&str; 4] = ["state", "causes", "counts", "rows"];
/// `HealthCode`: `spec refs`' count of the stored files.
const CODE_KEYS: [&str; 4] = ["files", "rs", "ron", "skipped"];
/// `AssumesCounts`.
const ASSUMES_COUNT_KEYS: [&str; 10] = [
    "markers",
    "rows",
    "pending",
    "as_assumed",
    "otherwise",
    "applied",
    "rejected",
    "unknown",
    "archived",
    "in_question",
];
/// `AssumesRow`.
const ASSUMES_ROW_KEYS: [&str; 8] = [
    "id", "state", "path", "line", "column", "relation", "proposal", "node",
];
/// `AssumedProposal`.
const PROPOSAL_KEYS: [&str; 6] = [
    "kind",
    "status",
    "choice",
    "record_id",
    "record_path",
    "reason",
];
/// `AssumedNode`.
const NODE_KEYS: [&str; 6] = ["id", "path", "line", "status", "successor", "proposals"];
/// `UnboundCounts`.
const UNBOUND_COUNT_KEYS: [&str; 4] = ["rules", "selected", "bound", "unbound"];
/// `UnboundRow`.
const UNBOUND_ROW_KEYS: [&str; 8] = [
    "id", "kind", "title", "path", "line", "rules", "mentions", "markers",
];
/// `UnboundMarker`.
const MARKER_KEYS: [&str; 7] = [
    "path", "line", "column", "relation", "id_state", "binding", "reason",
];

/// PHASE-B: the core and CLI sources of rows 4-5 (the `[health]` reader,
/// section selection, the row-4 judge, a CLI sibling of `health.rs`) whose
/// path does not hold `health`, repository-relative. AC-2 and AC-R7 scan
/// every source whose path holds `health` already; the developer lists the
/// others here (and in `code_refs_walk.rs` `NEW_MODULES`, TE-5).
pub(super) const PHASE_B_SOURCES: [&str; 0] = [];

/// code-a's laws, by default relations.
const LAW_RULE: &str = "\n[[health.bound]]\nkinds = [\"law\"]\n";

/// code-a's `crates/tally/src/lib.rs`.
const LIB: &str = "crates/tally/src/lib.rs";

/// code-a's nodes with an ID (every kind of its `[ids]`), as `spec refs`
/// takes them.
const CODE_A_REFERENCES: [&str; 10] = [
    "PIECE-COUNTER",
    "PIECE-LIMITS",
    "LAW-BOUND",
    "LAW-STEP",
    "LAW-ODD",
    "LAW-SOURCE",
    "LAW-DEFAULT",
    "GOAL-01",
    "PREM-001",
    "cap-check/PASS-01",
];
const CODE_A_KINDS: [&str; 5] = ["piece", "law", "objective", "premise", "pass-mark"];

/// inkwell's nodes and kinds.
const INKWELL_REFERENCES: [&str; 8] = [
    "SLAB-INK",
    "TENET-FLOW",
    "SLAB-SHEET",
    "TENET-ORDER",
    "TENET-WIDTH",
    "SLAB-PROBE",
    "QUILL-01",
    "QUILL-02",
];
const INKWELL_KINDS: [&str; 3] = ["slab", "tenet", "quill"];

/// The farm game's nodes and kinds.
const FARM_REFERENCES: [&str; 7] = [
    "ALM-FARM",
    "ALM-GROWTH",
    "ALM-HERD",
    "ALM-GRAZING",
    "CHORE-01",
    "CHORE-02",
    "CHORE-03",
];
const FARM_KINDS: [&str; 2] = ["almanac", "chore"];

// ------------------------------------------------------------ trees and runs

/// A byte copy of `fixtures/<fixture>` at `<scratch>/<dir>` (no
/// repository), `tail` appended to its config, `files` written.
fn tree(
    scratch: &Scratch,
    fixture: &str,
    dir: &str,
    tail: &str,
    files: &[(&str, &str)],
) -> PathBuf {
    let root = scratch.copy(fixture, dir);
    append_config(&root, tail);
    for (path, text) in files {
        write(&root, path, text);
    }
    root
}

/// `tail` appended to `root`'s config.
fn append_config(root: &Path, tail: &str) {
    let config = read_text(root, "specengine.toml");
    write(root, "specengine.toml", format!("{config}{tail}"));
}

/// `root`'s config without its `[code]` table (code-a's last table).
fn drop_code_table(root: &Path) {
    let config = read_text(root, "specengine.toml");
    let (head, _) = config.split_once("\n[code]\n").expect("a [code] table");
    write(root, "specengine.toml", format!("{head}\n"));
}

/// `text` appended to `relative` under `root`.
fn append(root: &Path, relative: &str, text: &str) {
    let old = read_text(root, relative);
    write(root, relative, format!("{old}{text}"));
}

/// The 1-based line of `text` holding `needle`.
fn line_with(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("no line holds {needle:?}:\n{text}"))
}

/// `spec health --json` in `root` under `home`, exit 0: the run, the
/// document ordered and as a value.
fn document(home: &Path, root: &Path) -> (Run, J, Value) {
    let (run, ordered) = health_json(home, root);
    let value = serde_json::from_str(run.stdout.trim_end()).expect("JSON");
    (run, ordered, value)
}

/// `spec health` in `root` under `home`, exit 0.
fn text(home: &Path, root: &Path) -> Run {
    let run = health_run(home, root, false);
    run.code(0);
    run
}

/// `spec health [--json]` in `root` with exactly `env`.
fn health_with(env: &[(&str, &OsStr)], root: &Path, json: bool) -> Run {
    let args: &[&str] = if json {
        &["health", "--json"]
    } else {
        &["health"]
    };
    spec_with(root, args, env)
}

/// stdout and stderr of `spec health --json`, then of `spec health`, with
/// `HOME` `home` in `cwd` (`--root root` when given).
fn both(home: &Path, cwd: &Path, root: Option<&Path>) -> Vec<String> {
    let root = root.map(|root| root.to_str().expect("a UTF-8 root").to_owned());
    let mut out = Vec::new();
    for json in [true, false] {
        let mut args: Vec<&str> = Vec::new();
        if let Some(root) = &root {
            args.extend(["--root", root.as_str()]);
        }
        args.push("health");
        if json {
            args.push("--json");
        }
        let run = spec(home, cwd, &args);
        run.code(0);
        out.push(run.stdout.clone());
        out.push(run.stderr.clone());
    }
    out
}

/// The IDs of a row's `rows`, in order.
fn row_ids(row: &Value) -> Vec<String> {
    row["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("no rows: {row}"))
        .iter()
        .map(|entry| entry["id"].as_str().expect("an ID").to_owned())
        .collect()
}

/// Row 5's one entry for `id`.
fn unbound_row<'a>(document: &'a Value, id: &str) -> &'a Value {
    let rows: Vec<&Value> = document["unbound"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("row 5 not measured: {}", document["unbound"]))
        .iter()
        .filter(|row| row["id"] == id)
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "{id}: one row 5 entry: {}",
        document["unbound"]
    );
    rows[0]
}

/// Row 4's entries for `id`, in order.
fn assumes_rows<'a>(document: &'a Value, id: &str) -> Vec<&'a Value> {
    document["assumes"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("row 4 not measured: {}", document["assumes"]))
        .iter()
        .filter(|row| row["id"] == id)
        .collect()
}

/// A row 5 marker entry.
fn marker(
    path: &str,
    line: u64,
    column: u64,
    relation: &str,
    id_state: &str,
    binding: &str,
    reason: &str,
) -> Value {
    json!({"path": path, "line": line, "column": column, "relation": relation,
        "id_state": id_state, "binding": binding, "reason": reason})
}

/// `spec refs <reference> --json` in `root`, exit 0, uncut: its one node.
fn refs_node(home: &Path, root: &Path, reference: &str) -> Value {
    let run = spec(home, root, &["refs", reference, "--json"]);
    run.code(0);
    let document: Value = serde_json::from_str(run.stdout.trim_end()).expect("refs JSON");
    let omitted = &document["omitted"];
    assert!(
        omitted.is_null() || (omitted["markers"] == 0 && omitted["mentions"] == 0),
        "{reference}: refs cut its block: {document}"
    );
    let nodes = document["nodes"].as_array().expect("nodes");
    assert_eq!(nodes.len(), 1, "{reference}: one holder: {document}");
    nodes[0].clone()
}

/// `spec refs <reference> --json`'s `code` in `root`.
fn refs_code(home: &Path, root: &Path, reference: &str) -> Value {
    let run = spec(home, root, &["refs", reference, "--json"]);
    run.code(0);
    serde_json::from_str::<Value>(run.stdout.trim_end()).expect("refs JSON")["code"].clone()
}

/// The cause a failing step gives ("Failures and exits": the step's CLI
/// line less `spec: `): `spec refs LAW-STEP`'s exit-2 line in `root` with
/// exactly `env`.
fn failing_line(env: &[(&str, &OsStr)], root: &Path) -> String {
    let run = spec_with(root, &["refs", "LAW-STEP"], env);
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    let line = run
        .stderr
        .lines()
        .last()
        .unwrap_or_else(|| panic!("no line: {}", run.show()));
    line.strip_prefix("spec: ").unwrap_or(line).to_owned()
}

/// `spec unmapped <path> --json` in `root`, exit 0.
fn unmapped_file(home: &Path, root: &Path, path: &str) -> Value {
    let run = spec(home, root, &["unmapped", path, "--json"]);
    run.code(0);
    serde_json::from_str(run.stdout.trim_end()).expect("unmapped JSON")
}

/// The qpaths of an unmapped document's items.
fn unmapped_qpaths(document: &Value) -> Vec<String> {
    document["unmapped"]
        .as_array()
        .expect("unmapped")
        .iter()
        .map(|item| item["qpath"].as_str().expect("qpath").to_owned())
        .collect()
}

/// The slug's database under `home`.
fn slug_db(home: &Path, slug: &str) -> PathBuf {
    data_dir(home).join(format!("{slug}.db"))
}

/// `sqlite3 <db> <statement>` (the system's client), which must succeed.
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
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The queue's tables `db` holds, one per line.
fn queue_tables(db: &Path) -> String {
    sql(
        db,
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('proposals', \
         'events', 'tasks', 'runs') ORDER BY name;",
    )
}

/// The first directory of the data directory's path under `home`
/// (`Library` on macOS).
fn data_top(home: &Path) -> PathBuf {
    let data = data_dir(home);
    let first = data
        .strip_prefix(home)
        .expect("under HOME")
        .components()
        .next()
        .expect("a component");
    home.join(first)
}

/// Every `.rs` file under `dir`, repository-relative, sorted.
pub(super) fn rust_files(dir: &Path) -> Vec<String> {
    let root = repository_root();
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(
                    path.strip_prefix(&root)
                        .expect("under the repository")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    out.sort();
    out
}

// ------------------------------------------------------------ TE-1's queue

/// The commit every applied item names.
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn at(minute: u32) -> String {
    format!("2026-10-05T10:{minute:02}:00Z")
}

/// Another repository's place: Health reads every repository's items
/// (`git_common_dir: None`).
fn place() -> Place {
    Place {
        git_common_dir: "/elsewhere/.git".to_owned(),
        worktree: "/elsewhere".to_owned(),
        root_rel: String::new(),
        branch: "t1".to_owned(),
        base_commit: "1".repeat(40),
    }
}

fn author() -> Author {
    Author::new(Some("developer".to_owned()), None, None).expect("an agent")
}

fn decision(note: &str) -> Decision {
    Decision {
        decided_by: "Owner <owner@example.invalid>".to_owned(),
        note: Some(note.to_owned()),
        staged_at: None,
    }
}

fn question(target: &str, summary: &str) -> NewIntake {
    NewIntake {
        kind: ProposalKind::Question,
        target_path: "docs/spec/limits.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: vec![target.to_owned()],
            severity: IntakeSeverity::Normal,
            gap_type: None,
            summary: summary.to_owned(),
            working_answer: Some("Yes.".to_owned()),
            price_of_other: Some("A rework of the counter.".to_owned()),
            evidence: Vec::new(),
            options: Vec::new(),
            recommendation: None,
            distinct_from: Vec::new(),
        },
    }
}

/// A discrepancy whose two options are labelled `"1"` and `"0"`, its
/// recommendation index 0: a compare by label reads index 1 as the
/// recommendation and index 0 as another.
fn discrepancy(target: &str, summary: &str) -> NewIntake {
    let option = |label: &str| IntakeOption {
        label: label.to_owned(),
        effect: format!("option {label} changes the counter"),
        price: "1 item".to_owned(),
    };
    NewIntake {
        kind: ProposalKind::Discrepancy,
        target_path: "docs/spec/counter.md".to_owned(),
        place: place(),
        author: author(),
        intake: Intake {
            target_ids: vec![target.to_owned()],
            severity: IntakeSeverity::High,
            gap_type: Some(GapType::Contradicts),
            summary: summary.to_owned(),
            working_answer: None,
            price_of_other: None,
            evidence: vec![Evidence {
                file: "crates/tally/src/counter.rs".to_owned(),
                qpath: None,
                lines: Some("23-27".to_owned()),
                observed: "steps by one".to_owned(),
                documented: "steps by two".to_owned(),
            }],
            options: vec![option("1"), option("0")],
            recommendation: Some(0),
            distinct_from: Vec::new(),
        },
    }
}

fn update(target: &str) -> NewProposal {
    let base_hash = format!("b3:{}", blake3_hex(target.as_bytes()));
    let new_text = format!("{target}, edited.\n");
    NewProposal {
        kind: ProposalKind::Update,
        target_id: target.to_owned(),
        target_path: "docs/spec/counter.md".to_owned(),
        place: place(),
        patch_hash: patch_hash(target, &base_hash, &new_text),
        base_hash: Some(base_hash),
        base_text: Some(format!("{target}.\n")),
        new_text,
        rationale: "A test edit.".to_owned(),
        author: author(),
        diagnostics: Vec::new(),
        new_ids: Vec::new(),
        interpretation: None,
    }
}

/// `intake` stored `open`: its ID.
fn raise(queue: &mut SqliteQueue, intake: &NewIntake, minute: u32) -> String {
    queue
        .create_intake(intake, &[], None, &at(minute))
        .expect("an intake")
        .created
        .expect("stored")
        .id
}

/// `id` approved with `choice` and its record (apply step 7): the
/// record's ID.
fn decide(queue: &mut SqliteQueue, id: &str, choice: Choice, minute: u32) -> String {
    let series = RecordSeries {
        prefix: "DEC".to_owned(),
        width: 4,
        corpus_max: 0,
    };
    let preview = queue.next_record(&series).expect("the next record");
    let seen = queue.get(id).expect("get").expect("stored").seen();
    let approval = RecordApproval {
        series,
        preview: preview.clone(),
        path: format!("docs/decisions/{preview}.md"),
        title: "Record".to_owned(),
        text: format!("---\nid: {preview}\n---\n"),
        choice,
    };
    queue
        .approve_record_from(id, &seen, &approval, &decision("Because."), &at(minute))
        .expect("approved with its record");
    preview
}

/// TE-1's queue in `home`'s database of `slug`, through the store; the
/// IDs and records as the module documentation lists them.
fn seed_queue(home: &Path, slug: &str) {
    let db = slug_db(home, slug);
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    let mut queue = SqliteQueue::open(&db, slug).expect("the queue opens");
    let q = &mut queue;
    let goal = question("GOAL-01", "Does the count wrap at the cap?");
    assert_eq!(raise(q, &goal, 1), "PR-0001");
    let default = question("LAW-DEFAULT", "Is the default cap ten?");
    assert_eq!(raise(q, &default, 2), "PR-0002");
    assert_eq!(decide(q, "PR-0002", Choice::WorkingAnswer, 3), "DEC-0001");
    let bound = question("LAW-BOUND", "Does the cap bound every step?");
    assert_eq!(raise(q, &bound, 4), "PR-0003");
    let no = Choice::Answer("No".to_owned());
    assert_eq!(decide(q, "PR-0003", no, 5), "DEC-0002");
    q.applied("PR-0003", COMMIT, &at(6)).expect("applied");
    let wraps = discrepancy("GOAL-01", "The count wraps past the cap.");
    assert_eq!(raise(q, &wraps, 7), "PR-0004");
    assert_eq!(decide(q, "PR-0004", Choice::Option(1), 8), "DEC-0003");
    q.applied("PR-0004", COMMIT, &at(9)).expect("applied");
    let skips = discrepancy("PIECE-COUNTER", "The counter skips a step.");
    assert_eq!(raise(q, &skips, 10), "PR-0005");
    assert_eq!(decide(q, "PR-0005", Choice::Option(0), 11), "DEC-0004");
    let twice = question("GOAL-01", "Is a step ever two?");
    assert_eq!(raise(q, &twice, 12), "PR-0006");
    q.reject("PR-0006", &decision("asked before"), &at(13))
        .expect("rejected");
    let step = q.create(&update("LAW-STEP"), &at(14)).expect("an update");
    assert_eq!(step.id, "PR-0007");
    q.approve("PR-0007", &decision("Because."), &at(15))
        .expect("approved");
    q.applied("PR-0007", COMMIT, &at(16)).expect("applied");
    let odd = q.create(&update("LAW-ODD"), &at(17)).expect("an update");
    assert_eq!(odd.id, "PR-0008");
    q.approve("PR-0008", &decision("Because."), &at(18))
        .expect("approved");
    let source = question("LAW-SOURCE", "Where does the cap come from?");
    assert_eq!(raise(q, &source, 19), "PR-0009");
}

/// The `@assumes` markers appended to `lib.rs` (its lines 22 to 35).
const LIB_TAIL: &str = "// @assumes PR-0001\n// @assumes PR-0002\n// @assumes PR-0003\n\
                        // @assumes PR-0004\n// @assumes PR-0005\n// @assumes PR-0006\n\
                        // @assumes PR-0007\n// @assumes PR-0008\n// @assumes PREM-002\n\
                        // @assumes PREM-004\n// @assumes LAW-SOURCE\n// @assumes LAW-DEFAULT\n\
                        // @assumes LAW-BOUND\n// @assumes GOAL-01\n";

/// TE-1's other files: an `@assumes` in a function body, three decisions.
const TE1_FILES: [(&str, &str); 4] = [
    (
        "crates/tally/src/assumed.rs",
        "//! Work done on a working answer.\n\npub fn assumed() -> u32 {\n    \
         // @assumes PR-0001\n    1\n}\n",
    ),
    (
        "docs/records/PREM-002.md",
        "---\nclass: decision\nid: PREM-002\nstatus: superseded-by PREM-003\n---\n\n\
         # Callers may share the count\n",
    ),
    (
        "docs/records/PREM-003.md",
        "---\nclass: decision\nid: PREM-003\nstatus: accepted\n---\n\n\
         # Callers share the count through a lock\n",
    ),
    (
        "docs/records/PREM-004.md",
        "---\nclass: decision\nid: PREM-004\nstatus: rejected\n---\n\n\
         # The count resets every day\n",
    ),
];

/// TE-1's tree, committed, and its seeded queue.
struct Tally {
    scratch: Scratch,
    git: Sandbox,
    root: PathBuf,
    home: PathBuf,
}

impl Tally {
    /// TE-1's tree at `<scratch>/tally`, `tail` appended to its config.
    fn new(label: &str, tail: &str) -> Self {
        let scratch = Scratch::new(label);
        let root = tree(&scratch, "code-a", "tally", tail, &TE1_FILES);
        append(&root, LIB, LIB_TAIL);
        write(
            &root,
            "spec.lock",
            read(&repository_root().join("fixtures/spec-lock"), "code-a.lock"),
        );
        let git = Sandbox::new(scratch.path());
        git.init(&root);
        git.add_all(&root);
        git.commit(&root, "TE-1's tree");
        let home = scratch.home("h");
        seed_queue(&home, "code-a");
        Self {
            scratch,
            git,
            root,
            home,
        }
    }

    fn db(&self) -> PathBuf {
        slug_db(&self.home, "code-a")
    }

    /// `spec health --json` here.
    fn document(&self) -> Value {
        document(&self.home, &self.root).2
    }
}

/// An `AssumedProposal`.
fn proposal(
    kind: &str,
    status: &str,
    choice: Value,
    record: Option<&str>,
    reason: Option<&str>,
) -> Value {
    json!({"kind": kind, "status": status, "choice": choice, "record_id": record,
        "record_path": record.map(|id| format!("docs/decisions/{id}.md")), "reason": reason})
}

/// An `AssumedNode`.
fn node(
    id: &str,
    path: &str,
    line: u64,
    status: Option<&str>,
    successor: Option<&str>,
    proposals: &[&str],
) -> Value {
    json!({"id": id, "path": path, "line": line, "status": status, "successor": successor,
        "proposals": proposals})
}

/// An (a) row of `lib.rs`'s tail.
fn queue_row(id: &str, state: &str, line: u64, proposal: Value) -> Value {
    json!({"id": id, "state": state, "path": LIB, "line": line, "column": 4,
        "relation": "assumes", "proposal": proposal, "node": null})
}

/// A (b) row of `lib.rs`'s tail.
fn node_row(id: &str, state: &str, line: u64, node: Value) -> Value {
    json!({"id": id, "state": state, "path": LIB, "line": line, "column": 4,
        "relation": "assumes", "proposal": null, "node": node})
}

/// TE-1's row 4, in "Row 4"'s order (ID as written, path, line, column).
fn te1_rows() -> Vec<Value> {
    let open = || proposal("question", "open", Value::Null, None, None);
    vec![
        node_row(
            "GOAL-01",
            "in_question",
            35,
            node(
                "GOAL-01",
                "docs/records/GOAL-01.md",
                1,
                None,
                None,
                &["PR-0001"],
            ),
        ),
        node_row(
            "LAW-DEFAULT",
            "in_question",
            33,
            node(
                "LAW-DEFAULT",
                "docs/spec/limits.md",
                16,
                None,
                None,
                &["PR-0002"],
            ),
        ),
        node_row(
            "LAW-SOURCE",
            "in_question",
            32,
            node(
                "LAW-SOURCE",
                "docs/spec/limits.md",
                12,
                None,
                None,
                &["PR-0009"],
            ),
        ),
        json!({"id": "PR-0001", "state": "pending", "path": "crates/tally/src/assumed.rs",
            "line": 4, "column": 8, "relation": "assumes", "proposal": open(), "node": null}),
        queue_row("PR-0001", "pending", 22, open()),
        queue_row(
            "PR-0002",
            "as_assumed",
            23,
            proposal(
                "question",
                "approved",
                json!({"working_answer": true}),
                Some("DEC-0001"),
                None,
            ),
        ),
        queue_row(
            "PR-0003",
            "otherwise",
            24,
            proposal(
                "question",
                "applied",
                json!({"answer": "No"}),
                Some("DEC-0002"),
                None,
            ),
        ),
        queue_row(
            "PR-0004",
            "otherwise",
            25,
            proposal(
                "discrepancy",
                "applied",
                json!({"option": 1}),
                Some("DEC-0003"),
                None,
            ),
        ),
        queue_row(
            "PR-0005",
            "as_assumed",
            26,
            proposal(
                "discrepancy",
                "approved",
                json!({"option": 0}),
                Some("DEC-0004"),
                None,
            ),
        ),
        queue_row(
            "PR-0006",
            "rejected",
            27,
            proposal(
                "question",
                "rejected",
                Value::Null,
                None,
                Some("asked before"),
            ),
        ),
        queue_row(
            "PR-0007",
            "applied",
            28,
            proposal("update", "applied", Value::Null, None, None),
        ),
        queue_row(
            "PR-0008",
            "pending",
            29,
            proposal("update", "approved", Value::Null, None, None),
        ),
        json!({"id": "PR-7", "state": "unknown", "path": LIB, "line": 13, "column": 4,
            "relation": "implements", "proposal": null, "node": null}),
        node_row(
            "PREM-002",
            "archived",
            30,
            node(
                "PREM-002",
                "docs/records/PREM-002.md",
                1,
                Some("superseded-by PREM-003"),
                Some("PREM-003"),
                &[],
            ),
        ),
        node_row(
            "PREM-004",
            "archived",
            31,
            node(
                "PREM-004",
                "docs/records/PREM-004.md",
                1,
                Some("rejected"),
                None,
                &[],
            ),
        ),
    ]
}

/// TE-1's row 4 entries of `id`.
fn te1_rows_of(id: &str) -> Vec<Value> {
    te1_rows()
        .into_iter()
        .filter(|row| row["id"] == id)
        .collect()
}

/// TE-1's row 4 counts: (a) ten markers (`PR-0001` twice, `PR-0002` to
/// `PR-0008`, `PR-7`), (b) seven (`GOAL-01`, `LAW-DEFAULT`, `LAW-SOURCE`,
/// `LAW-BOUND`, `PREM-002`, `PREM-004`, code-a's `PREM-001`).
fn te1_counts() -> Value {
    json!({"markers": 17, "rows": 15, "pending": 3, "as_assumed": 2, "otherwise": 2,
        "applied": 1, "rejected": 1, "unknown": 1, "archived": 2, "in_question": 3})
}

/// TE-1's row 4 lines ("Text", "Row 4's tail").
fn te1_lines() -> Vec<String> {
    let lib = |line: u32| format!("{LIB}:{line}:4");
    vec![
        format!(
            "assumes | in_question | GOAL-01 | {} | assumes | GOAL-01 | \
             docs/records/GOAL-01.md:1 | under PR-0001",
            lib(35)
        ),
        format!(
            "assumes | in_question | LAW-DEFAULT | {} | assumes | LAW-DEFAULT | \
             docs/spec/limits.md:16 | under PR-0002",
            lib(33)
        ),
        format!(
            "assumes | in_question | LAW-SOURCE | {} | assumes | LAW-SOURCE | \
             docs/spec/limits.md:12 | under PR-0009",
            lib(32)
        ),
        "assumes | pending | PR-0001 | crates/tally/src/assumed.rs:4:8 | assumes | question open"
            .to_owned(),
        format!(
            "assumes | pending | PR-0001 | {} | assumes | question open",
            lib(22)
        ),
        format!(
            "assumes | as_assumed | PR-0002 | {} | assumes | question approved \
             {{\"working_answer\":true}} | record DEC-0001 docs/decisions/DEC-0001.md",
            lib(23)
        ),
        format!(
            "assumes | otherwise | PR-0003 | {} | assumes | question applied \
             {{\"answer\":\"No\"}} | record DEC-0002 docs/decisions/DEC-0002.md",
            lib(24)
        ),
        format!(
            "assumes | otherwise | PR-0004 | {} | assumes | discrepancy applied \
             {{\"option\":1}} | record DEC-0003 docs/decisions/DEC-0003.md",
            lib(25)
        ),
        format!(
            "assumes | as_assumed | PR-0005 | {} | assumes | discrepancy approved \
             {{\"option\":0}} | record DEC-0004 docs/decisions/DEC-0004.md",
            lib(26)
        ),
        format!(
            "assumes | rejected | PR-0006 | {} | assumes | question rejected: asked before",
            lib(27)
        ),
        format!(
            "assumes | applied | PR-0007 | {} | assumes | update applied",
            lib(28)
        ),
        format!(
            "assumes | pending | PR-0008 | {} | assumes | update approved",
            lib(29)
        ),
        format!("assumes | unknown | PR-7 | {LIB}:13:4 | implements"),
        format!(
            "assumes | archived | PREM-002 | {} | assumes | PREM-002 | \
             docs/records/PREM-002.md:1 | status superseded-by PREM-003 | successor PREM-003",
            lib(30)
        ),
        format!(
            "assumes | archived | PREM-004 | {} | assumes | PREM-004 | \
             docs/records/PREM-004.md:1 | status rejected",
            lib(31)
        ),
    ]
}

/// TE-1's row 4 summary line.
const TE1_SUMMARY: &str = "assumes: 15 rows of 17 markers: 3 pending, 2 as_assumed, \
                           2 otherwise, 1 applied, 1 rejected, 1 unknown, 2 archived, \
                           3 in_question";

/// The lines of a text run that start with `prefix`.
fn lines_with(run: &Run, prefix: &str) -> Vec<String> {
    run.stdout
        .lines()
        .filter(|line| line.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

// ------------------------------------------------------------ TE-1: row 4

/// TE-1 (a) and (b), whole: row 4 `measured`, its fifteen rows of
/// seventeen markers in "Row 4"'s order with every key in "Data"'s order,
/// the text's rows and summary; `code` is `spec refs`' count of the stored
/// files; row 5 `not_configured` (no rule). Read through the store with
/// `git_common_dir: None`: the items' place is another repository's.
#[test]
fn assumes_rows_over_the_seeded_queue() {
    let t = Tally::new("health-te1", "");
    let checks = t.scratch.home("checks");
    let documents = check_documents(&checks, &t.root);
    let (_, ordered, doc) = document(&t.home, &t.root);
    assert_eq!(ordered.keys(), DOCUMENT_KEYS);
    assert_eq!(doc["documents"], documents);
    assert_eq!(doc["assumes"]["state"], "measured", "{}", doc["assumes"]);
    assert_eq!(doc["assumes"]["causes"], json!([]));
    assert_eq!(doc["assumes"]["counts"], te1_counts());
    assert_eq!(doc["assumes"]["rows"], Value::Array(te1_rows()));
    let assumes = ordered.get("assumes");
    assert_eq!(assumes.keys(), ROW_KEYS);
    assert_eq!(assumes.get("counts").keys(), ASSUMES_COUNT_KEYS);
    for row in assumes.get("rows").items() {
        assert_eq!(row.keys(), ASSUMES_ROW_KEYS);
        if row.get("proposal") != &J::Null {
            assert_eq!(row.get("proposal").keys(), PROPOSAL_KEYS);
        }
        if row.get("node") != &J::Null {
            assert_eq!(row.get("node").keys(), NODE_KEYS);
        }
    }
    assert_eq!(ordered.get("code").keys(), CODE_KEYS);
    assert_eq!(doc["code"], refs_code(&t.home, &t.root, "LAW-STEP"));
    assert_eq!(
        doc["code"],
        json!({"files": 9, "rs": 8, "ron": 1, "skipped": 0})
    );
    assert_eq!(
        doc["unbound"],
        json!({"state": "not_configured", "causes": [NO_RULE], "counts": null, "rows": null})
    );

    let run = text(&t.home, &t.root);
    assert_eq!(
        lines_with(&run, "assumes | "),
        te1_lines(),
        "{}",
        run.stdout
    );
    // docs/features/health-drift.md "Text and stderr": row 6's summary
    // between rows 4-5's and `code:` (TE-1's tree holds the golden lock).
    let mut tail: Vec<&str> = run.stdout.lines().rev().take(5).collect();
    let drift = tail.remove(2);
    assert!(drift.starts_with("drift: "), "{}", run.stdout);
    assert_eq!(
        tail,
        [
            format!("spec health: {documents} documents, 0 unapplied amendments").as_str(),
            "code: 9 files (.rs 8, .ron 1), skipped 0",
            format!("unbound: not configured: {NO_RULE}").as_str(),
            TE1_SUMMARY,
        ],
        "{}",
        run.stdout
    );
    assert!(
        !run.stderr.contains("note: assumes:") && !run.stderr.contains("note: unbound:"),
        "only `cannot` goes to stderr: {}",
        run.show()
    );
}

/// RD-3: `PR-7` (`lib.rs:13`, an `implements`) is `unknown`, never the
/// queue's `PR-0007` (the queue writes four digits); its text line has no
/// tail. M: `PR-7` matched loosely.
#[test]
fn short_queue_id_is_unknown() {
    let t = Tally::new("health-pr7", "");
    let doc = t.document();
    let rows: Vec<Value> = assumes_rows(&doc, "PR-7").into_iter().cloned().collect();
    assert_eq!(rows, te1_rows_of("PR-7"), "{}", doc["assumes"]);
    assert_eq!(rows[0]["proposal"], Value::Null);
    let applied: Vec<Value> = assumes_rows(&doc, "PR-0007").into_iter().cloned().collect();
    assert_eq!(applied, te1_rows_of("PR-0007"));
    let run = text(&t.home, &t.root);
    assert!(
        run.stdout
            .lines()
            .any(|line| line == format!("assumes | unknown | PR-7 | {LIB}:13:4 | implements")),
        "{}",
        run.stdout
    );
}

/// RD-3: a discrepancy is decided as assumed when its chosen option's
/// index is its recommendation's, never by label: `PR-0004` (index 1,
/// labelled "0", recommendation 0) `otherwise`, `PR-0005` (index 0,
/// labelled "1") `as_assumed`. M: a discrepancy by label.
#[test]
fn discrepancy_option_against_recommendation_index() {
    let t = Tally::new("health-option", "");
    let doc = t.document();
    for id in ["PR-0004", "PR-0005"] {
        let rows: Vec<Value> = assumes_rows(&doc, id).into_iter().cloned().collect();
        assert_eq!(rows, te1_rows_of(id), "{id}: {}", doc["assumes"]);
    }
    assert_eq!(assumes_rows(&doc, "PR-0004")[0]["state"], "otherwise");
    assert_eq!(assumes_rows(&doc, "PR-0005")[0]["state"], "as_assumed");
}

/// RD-3: a question approved on `{"working_answer":true}` is `as_assumed`
/// (`PR-0002`); one applied on `{"answer":"No"}` is `otherwise`
/// (`PR-0003`), each with its record. M: the working-answer test inverted.
#[test]
fn working_answer_choice_is_as_assumed() {
    let t = Tally::new("health-working", "");
    let doc = t.document();
    for id in ["PR-0002", "PR-0003"] {
        let rows: Vec<Value> = assumes_rows(&doc, id).into_iter().cloned().collect();
        assert_eq!(rows, te1_rows_of(id), "{id}: {}", doc["assumes"]);
    }
    assert_eq!(assumes_rows(&doc, "PR-0002")[0]["state"], "as_assumed");
    assert_eq!(assumes_rows(&doc, "PR-0003")[0]["state"], "otherwise");
}

/// RD-4: an `@assumes` on a node an `open` or `approved` item targets is
/// `in_question`, `proposals` those items alone (`GOAL-01`: `PR-0001`
/// open, never the applied `PR-0004` nor the rejected `PR-0006`;
/// `LAW-DEFAULT`: the approved `PR-0002`; `LAW-SOURCE`: `PR-0009`); a node
/// only an applied item targets (`LAW-BOUND`) and one no item targets
/// (`PREM-001`, `counter.rs:37`) are unlisted. M: `target_ids` skipped.
#[test]
fn assumed_node_under_an_open_proposal_is_in_question() {
    let t = Tally::new("health-in-question", "");
    let doc = t.document();
    for id in ["GOAL-01", "LAW-DEFAULT", "LAW-SOURCE"] {
        let rows: Vec<Value> = assumes_rows(&doc, id).into_iter().cloned().collect();
        assert_eq!(rows, te1_rows_of(id), "{id}: {}", doc["assumes"]);
    }
    for id in ["LAW-BOUND", "PREM-001"] {
        assert!(
            assumes_rows(&doc, id).is_empty(),
            "{id} unlisted: {}",
            doc["assumes"]
        );
    }
    assert_eq!(doc["assumes"]["counts"]["in_question"], 3);
}

/// RD-4: an `@assumes` on a node of a Tier 3 document is `archived`, its
/// document's raw `status:` and, for `superseded-by X`, `successor` X
/// (`PREM-002` -> `PREM-003`); a rejected decision has none (`PREM-004`).
/// The text's tails. M: the successor dropped.
#[test]
fn superseded_decision_shows_its_successor() {
    let t = Tally::new("health-successor", "");
    let doc = t.document();
    for id in ["PREM-002", "PREM-004"] {
        let rows: Vec<Value> = assumes_rows(&doc, id).into_iter().cloned().collect();
        assert_eq!(rows, te1_rows_of(id), "{id}: {}", doc["assumes"]);
    }
    assert_eq!(
        assumes_rows(&doc, "PREM-002")[0]["node"]["successor"],
        "PREM-003"
    );
    let run = text(&t.home, &t.root);
    let lines = lines_with(&run, "assumes | archived | ");
    assert_eq!(lines, te1_lines()[13..].to_vec(), "{}", run.stdout);
}

/// RD-4 (Q12): row 4's unit is a stored marker, whatever it binds: the
/// `@assumes PR-0001` inside `assumed()`'s body (`unanchored in_body`) is
/// judged beside the one on `lib.rs:22` (no item), path first. M: row 4
/// filtered by binding.
#[test]
fn assumes_in_a_function_body_is_judged() {
    let t = Tally::new("health-in-body", "");
    let doc = t.document();
    let rows: Vec<Value> = assumes_rows(&doc, "PR-0001").into_iter().cloned().collect();
    assert_eq!(rows, te1_rows_of("PR-0001"), "{}", doc["assumes"]);
    assert_eq!(rows[0]["path"], "crates/tally/src/assumed.rs");
    assert_eq!(rows[0]["column"], 8, "inside the body");
}

/// AC-7 (S13): a fresh data directory, `[code]` present, nothing ever
/// queued: row 4 `measured` (inkwell: no queue marker, no `@assumes`: 0
/// rows; code-a: `PR-7` `unknown`), and the slug's DB the refresh made
/// holds no queue table afterwards, `user_version` 0 (no schema step:
/// `open_existing`, never `open`). M: `SqliteQueue::open`.
#[test]
fn health_runs_no_queue_schema_step() {
    let scratch = Scratch::new("health-ac7");
    let ink = tree(&scratch, "code-index", "ink", "", &[]);
    let home = scratch.home("ink");
    let (_, _, doc) = document(&home, &ink);
    assert_eq!(
        doc["assumes"],
        json!({"state": "measured", "causes": [],
            "counts": {"markers": 0, "rows": 0, "pending": 0, "as_assumed": 0, "otherwise": 0,
                "applied": 0, "rejected": 0, "unknown": 0, "archived": 0, "in_question": 0},
            "rows": []})
    );
    let db = slug_db(&home, "inkwell");
    assert!(db.is_file(), "the refresh made the slug's DB");
    assert_eq!(queue_tables(&db), "", "no queue table after a health call");
    assert_eq!(sql(&db, "PRAGMA user_version;"), "0\n");

    let tally = tree(&scratch, "code-a", "tally", "", &[]);
    let home = scratch.home("tally");
    let (_, _, doc) = document(&home, &tally);
    assert_eq!(doc["assumes"]["state"], "measured");
    assert_eq!(row_ids(&doc["assumes"]), ["PR-7"]);
    assert_eq!(doc["assumes"]["rows"][0]["state"], "unknown");
    text(&home, &tally);
    let db = slug_db(&home, "code-a");
    assert_eq!(queue_tables(&db), "", "nor after a second call");
    assert_eq!(sql(&db, "PRAGMA user_version;"), "0\n");
}

// ------------------------------------------------------------ RD-1: the keys

/// RD-1: every key present whatever the state: without `[code]` (`spec-c`,
/// `spec-a`, code-a with its table dropped) `code` null and both rows
/// `off`, keys in order; with it (code-a) the same keys, `code` an object
/// of four. M: keys dropped without `[code]`.
#[test]
fn health_code_keys_present_without_code_table() {
    let scratch = Scratch::new("health-keys");
    let home = scratch.home("h");
    let c = tree(&scratch, "spec-c", "c", "", &[]);
    let a = tree(&scratch, "spec-a", "a", "", &[]);
    let tally_off = tree(&scratch, "code-a", "tally-off", LAW_RULE, &[]);
    drop_code_table(&tally_off);
    let off = json!({"state": "off", "causes": [NO_CODE], "counts": null, "rows": null});
    let off_drift = json!({"state": "off", "causes": [NO_CODE], "counts": null, "rows": null,
        "tested": null, "notes": []});
    for root in [&c, &a, &tally_off] {
        let (_, ordered, doc) = document(&home, root);
        let context = root.display().to_string();
        assert_eq!(ordered.keys(), DOCUMENT_KEYS, "{context}");
        assert_eq!(doc["code"], Value::Null, "{context}");
        for key in ["assumes", "unbound"] {
            assert_eq!(ordered.get(key).keys(), ROW_KEYS, "{context}: {key}");
            assert_eq!(doc[key], off, "{context}: {key}");
        }
        assert_eq!(ordered.get("drift").keys(), DRIFT_KEYS, "{context}");
        assert_eq!(doc["drift"], off_drift, "{context}");
    }
    let tally = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    let (_, ordered, doc) = document(&home, &tally);
    assert_eq!(ordered.keys(), DOCUMENT_KEYS);
    assert_eq!(ordered.get("code").keys(), CODE_KEYS);
    for key in ["assumes", "unbound"] {
        assert_eq!(ordered.get(key).keys(), ROW_KEYS, "{key}");
        assert_eq!(doc[key]["state"], "measured", "{key}");
    }
    assert_eq!(ordered.get("drift").keys(), DRIFT_KEYS);
}

/// RD-1: row 3 is the same with rows 4-5 measured and `off`: code-a with
/// an amendment and a broken front-matter, its config with and without
/// `[code]`: `documents`, `amendments`, `notes` byte-equal, row 3's text
/// lines equal, `documents` = `spec check`'s. M: row 3 altered.
#[test]
fn amendments_unchanged_by_code_rows() {
    let scratch = Scratch::new("health-row3");
    let files = [
        (
            "docs/features/cap-raise.md",
            "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [LAW-DEFAULT]\n---\n\n\
             # Raise the default cap\n",
        ),
        (
            "docs/features/broken.md",
            "---\nclass: spec\nstatus: draft\ntitle: Broken: a colon\n---\n\n# Broken\n",
        ),
    ];
    let measured = tree(&scratch, "code-a", "measured", LAW_RULE, &files);
    let off = tree(&scratch, "code-a", "off", LAW_RULE, &files);
    drop_code_table(&off);
    let checks = scratch.home("checks");
    let (on_run, on, on_doc) = document(&scratch.home("on"), &measured);
    let (off_run, off_ordered, off_doc) = document(&scratch.home("off"), &off);
    assert_eq!(on_doc["unbound"]["state"], "measured");
    assert_eq!(off_doc["unbound"]["state"], "off");
    for key in ["documents", "amendments", "notes"] {
        assert_eq!(on.get(key), off_ordered.get(key), "{key}");
    }
    assert_eq!(on_doc["amendments"].as_array().map(Vec::len), Some(1));
    assert_eq!(on_doc["notes"].as_array().map(Vec::len), Some(1));
    assert_eq!(on_doc["documents"], check_documents(&checks, &measured));
    for run in [&on_run, &off_run] {
        assert!(
            run.stderr
                .lines()
                .any(|line| line.starts_with("note: docs/features/broken.md:")),
            "{}",
            run.show()
        );
    }
    let row_3 = |run: &Run| -> Vec<String> {
        run.stdout
            .lines()
            .filter(|line| {
                ![
                    "assumes | ",
                    "assumes: ",
                    "unbound | ",
                    "unbound: ",
                    "drift | ",
                    "drift: ",
                    "code: ",
                    "  ",
                ]
                .iter()
                .any(|prefix| line.starts_with(prefix))
                    || line.starts_with("  out ")
            })
            .map(str::to_owned)
            .collect()
    };
    let on_text = text(&scratch.home("on"), &measured);
    let off_text = text(&scratch.home("off"), &off);
    assert_eq!(row_3(&on_text), row_3(&off_text));
    assert!(
        row_3(&on_text)
            .iter()
            .any(|line| line.starts_with("docs/features/cap-raise.md")),
        "{}",
        on_text.stdout
    );
}

// ------------------------------------------------------------ RD-2, AC-6, TE-3

/// Rows 4-5 `cannot` with `causes` `[cause]`, `counts` and `rows` null,
/// `code` null, row 3 measured (`documents`), exit 0 in both modes; the
/// text's two `<row>: cannot: <cause>` lines, no `code:` line, the last
/// line; stderr's two `note: <row>: <cause>` lines.
fn assert_code_rows_cannot(
    env: &[(&str, &OsStr)],
    root: &Path,
    cause: &str,
    documents: usize,
    context: &str,
) {
    assert_rows_cannot(env, root, cause, documents, context, true);
}

/// [`assert_code_rows_cannot`], row 6 too:
/// docs/features/health-drift.md "Degrade matrix": a `shared` step's cause
/// is row 6's too (`cannot`, never a count; these trees hold no worktree,
/// a row-6 cause of its own, so the shared one is among its causes), else
/// row 6 never names it.
fn assert_rows_cannot(
    env: &[(&str, &OsStr)],
    root: &Path,
    cause: &str,
    documents: usize,
    context: &str,
    shared: bool,
) {
    let cannot = json!({"state": "cannot", "causes": [cause], "counts": null, "rows": null});
    let run = health_with(env, root, true);
    run.code(0);
    let doc: Value = serde_json::from_str(run.stdout.trim_end())
        .unwrap_or_else(|error| panic!("{context}: {error}: {}", run.show()));
    assert_eq!(doc["documents"], documents, "{context}");
    assert_eq!(doc["code"], Value::Null, "{context}");
    assert_eq!(doc["assumes"], cannot, "{context}: {}", run.show());
    assert_eq!(doc["unbound"], cannot, "{context}: {}", run.show());
    let drift = &doc["drift"];
    assert_eq!(drift["state"], "cannot", "{context}: {drift}");
    for key in ["counts", "rows", "tested"] {
        assert_eq!(drift[key], Value::Null, "{context}: {drift}");
    }
    let names = drift["causes"]
        .as_array()
        .expect("causes")
        .iter()
        .any(|named| named == cause);
    assert_eq!(names, shared, "{context}: row 6's causes: {drift}");
    let last = format!("spec health: {documents} documents, 0 unapplied amendments");
    let lines = health_with(env, root, false);
    lines.code(0);
    let stdout: Vec<&str> = lines.stdout.lines().collect();
    assert_eq!(stdout.last().copied(), Some(last.as_str()), "{context}");
    assert!(
        !stdout.iter().any(|line| line.starts_with("code: ")),
        "{context}: `code:` only when read: {}",
        lines.stdout
    );
    let rows: &[&str] = if shared {
        &["assumes", "unbound", "drift"]
    } else {
        &["assumes", "unbound"]
    };
    for row in rows {
        let shown = format!("{row}: cannot: {cause}");
        assert!(
            stdout.contains(&shown.as_str()),
            "{context}: {shown}\n{}",
            lines.show()
        );
        let note = format!("note: {row}: {cause}");
        for done in [&run, &lines] {
            assert!(
                done.stderr.lines().any(|line| line == note),
                "{context}: {note}\n{}",
                done.show()
            );
        }
    }
}

/// RD-2 (AC-6): an unreadable `.rs` file is a gap of the code walk: rows
/// 4-5 `cannot`, the cause `spec refs`' line less `spec: ` (``the code walk
/// is incomplete: `<path>` …``), row 3 measured, exit 0; readable again,
/// both measured. M: a code walk gap exits 2.
#[test]
fn code_walk_gap_is_rows_cannot_row_3_measured() {
    let scratch = Scratch::new("health-code-gap");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    let home = scratch.home("h");
    let env = [("HOME", home.as_os_str())];
    let documents = check_documents(&scratch.home("checks"), &root);
    let counter = root.join("crates/tally/src/counter.rs");
    fs::set_permissions(&counter, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let cause = failing_line(&env, &root);
    assert!(
        cause.starts_with("the code walk is incomplete: ")
            && cause.contains("`crates/tally/src/counter.rs`"),
        "{cause}"
    );
    assert_code_rows_cannot(&env, &root, &cause, documents, "an unreadable .rs file");
    fs::set_permissions(&counter, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    let (_, _, doc) = document(&home, &root);
    assert_eq!(doc["assumes"]["state"], "measured");
    assert_eq!(doc["unbound"]["state"], "measured");
}

/// RD-2, AC-6, TE-3 "Failures and exits": each step that fails leaves rows
/// 4-5 `cannot`, its line less `spec: ` the one cause, no count, `code`
/// null, row 3 measured, exit 0: a bad `[code]`, an unknown `[code]
/// frameworks` name, `HOME` unset, a data directory that cannot be made,
/// one inside the root (nothing created), a config without a slug
/// (`discover_with_text`'s refusal at the index), an unknown stored
/// binding word (`place`'s). M: `cannot` with a count.
#[test]
fn a_row_that_cannot_be_measured_has_no_count() {
    let scratch = Scratch::new("health-cannot");
    let checks = scratch.home("checks");

    // A bad `[code]`: `roots` a string.
    let root = tree(&scratch, "code-a", "bad-code", LAW_RULE, &[]);
    replace(
        &root,
        "specengine.toml",
        "roots   = [\"crates\", \"data\", \"tools/loose.rs\"]",
        "roots   = \"crates\"",
    );
    let home = scratch.home("bad-code");
    let env = [("HOME", home.as_os_str())];
    let cause = failing_line(&env, &root);
    assert!(cause.starts_with("specengine.toml:"), "{cause}");
    let documents = check_documents(&checks, &root);
    assert_code_rows_cannot(&env, &root, &cause, documents, "a bad [code]");

    // An unknown `[code] frameworks` name (10a-1's refusal in the refresh).
    let root = tree(
        &scratch,
        "code-a",
        "frameworks",
        &format!("frameworks = [\"nosuch\"]\n{LAW_RULE}"),
        &[],
    );
    let home = scratch.home("frameworks");
    let env = [("HOME", home.as_os_str())];
    let cause = failing_line(&env, &root);
    assert!(cause.contains("`nosuch` is not known"), "{cause}");
    assert_code_rows_cannot(&env, &root, &cause, documents, "an unknown framework");

    // `HOME` unset.
    let root = tree(&scratch, "code-a", "no-home", LAW_RULE, &[]);
    let cause = failing_line(&[], &root);
    assert_eq!(
        cause,
        "HOME is not set: the index database lives under the home directory"
    );
    assert_code_rows_cannot(&[], &root, &cause, documents, "HOME unset");

    // A data directory that cannot be made.
    let root = tree(&scratch, "code-a", "read-only", LAW_RULE, &[]);
    let home = scratch.home("read-only");
    let top = data_top(&home);
    fs::create_dir_all(&top).expect("the data directory's top");
    fs::set_permissions(&top, fs::Permissions::from_mode(0o555)).expect("chmod 555");
    let env = [("HOME", home.as_os_str())];
    let cause = failing_line(&env, &root);
    assert!(
        cause.starts_with("cannot create the data directory "),
        "{cause}"
    );
    assert_code_rows_cannot(
        &env,
        &root,
        &cause,
        documents,
        "an unwritable data directory",
    );
    fs::set_permissions(&top, fs::Permissions::from_mode(0o755)).expect("chmod 755");

    // A data directory inside the root: nothing created.
    let root = tree(&scratch, "code-a", "inside", LAW_RULE, &[]);
    let env = [("HOME", root.as_os_str())];
    let cause = failing_line(&env, &root);
    assert!(cause.contains("lies inside the project root"), "{cause}");
    assert_code_rows_cannot(&env, &root, &cause, documents, "a data directory inside");
    assert!(!data_top(&root).exists(), "nothing created in the root");

    // A config without a slug: row 3 reads, the index needs one.
    let root = tree(&scratch, "code-a", "no-slug", LAW_RULE, &[]);
    replace(&root, "specengine.toml", "slug = \"code-a\"\n", "");
    let home = scratch.home("no-slug");
    let env = [("HOME", home.as_os_str())];
    let cause = failing_line(&env, &root);
    assert_code_rows_cannot(&env, &root, &cause, documents, "no slug");

    // An unknown stored binding word: `place`'s error.
    let root = tree(&scratch, "code-a", "word", LAW_RULE, &[]);
    let home = scratch.home("word");
    index(&home, &root);
    sql(
        &slug_db(&home, "code-a"),
        "UPDATE code_markers SET binding = 'bogus';",
    );
    let (_, _, doc) = document(&home, &root);
    let causes = doc["unbound"]["causes"].as_array().expect("causes").clone();
    assert_eq!(causes.len(), 1, "{}", doc["unbound"]);
    let cause = causes[0].as_str().expect("a cause").to_owned();
    assert!(
        cause.contains("unknown binding `bogus`") && cause.contains("spec index --full"),
        "{cause}"
    );
    let env = [("HOME", home.as_os_str())];
    assert_rows_cannot(
        &env,
        &root,
        &cause,
        documents,
        "an unknown stored word",
        false,
    );
}

/// RD-2: exit 0 in every state, both modes, never 1: rows measured with
/// rows listed (TE-1 and a rule), a code gap (`cannot`), no `[code]`
/// (`off`), no rule (`not_configured`), a bad rule (row 5 `cannot`). M: an
/// exit 1.
#[test]
fn health_never_exits_1() {
    let t = Tally::new("health-exit", LAW_RULE);
    let gap = tree(&t.scratch, "code-a", "gap", LAW_RULE, &[]);
    fs::set_permissions(
        gap.join("crates/tally/src/lib.rs"),
        fs::Permissions::from_mode(0o000),
    )
    .expect("chmod 000");
    let off = tree(&t.scratch, "spec-c", "off", LAW_RULE, &[]);
    let no_rule = tree(&t.scratch, "code-a", "no-rule", "", &[]);
    let bad_rule = tree(
        &t.scratch,
        "code-a",
        "bad-rule",
        "\n[[health.bound]]\nkinds = 3\n",
        &[],
    );
    let (_, _, doc) = document(&t.home, &t.root);
    assert!(
        !doc["assumes"]["rows"].as_array().expect("rows").is_empty()
            && !doc["unbound"]["rows"].as_array().expect("rows").is_empty(),
        "rows listed: {doc}"
    );
    for (root, home) in [
        (&t.root, t.home.clone()),
        (&gap, t.scratch.home("gap")),
        (&off, t.scratch.home("off")),
        (&no_rule, t.scratch.home("no-rule")),
        (&bad_rule, t.scratch.home("bad-rule")),
    ] {
        for json in [true, false] {
            let run = health_run(&home, root, json);
            assert_eq!(run.code, 0, "{}: {}", root.display(), run.show());
        }
    }
    fs::set_permissions(
        gap.join("crates/tally/src/lib.rs"),
        fs::Permissions::from_mode(0o644),
    )
    .expect("chmod 644");
}

/// RD-2 (Q17): the refreshes' notes go to stderr as `spec refs` prints
/// them, never into JSON `notes` (row 3's alone): a code file whose bytes
/// are not UTF-8 is a refresh note and `code.skipped` 1 (`spec refs`'
/// count), a broken front-matter row 3's note. M: a refresh note in JSON
/// `notes`.
#[test]
fn refresh_notes_go_to_stderr_only() {
    let scratch = Scratch::new("health-notes");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        LAW_RULE,
        &[(
            "docs/features/broken.md",
            "---\nclass: spec\nstatus: draft\ntitle: Broken: a colon\n---\n\n# Broken\n",
        )],
    );
    write(
        &root,
        "crates/tally/src/latin1.rs",
        b"// caf\xe9\npub fn cafe() {}\n",
    );
    let home = scratch.home("h");
    let refs = spec(&home, &root, &["refs", "LAW-STEP"]);
    refs.code(0);
    let refresh_note = "note: crates/tally/src/latin1.rs is not UTF-8: skipped";
    assert!(
        refs.stderr.lines().any(|line| line == refresh_note),
        "spec refs prints it: {}",
        refs.show()
    );
    let (run, _, doc) = document(&home, &root);
    let notes = doc["notes"].as_array().expect("notes");
    assert_eq!(notes.len(), 1, "row 3's note alone: {notes:?}");
    assert!(
        notes[0]
            .as_str()
            .is_some_and(|note| note.starts_with("docs/features/broken.md:")
                && note.ends_with("; not judged")),
        "{notes:?}"
    );
    assert!(!run.stdout.contains("latin1"), "{}", run.stdout);
    assert!(
        run.stderr.lines().any(|line| line == refresh_note),
        "{}",
        run.show()
    );
    assert_eq!(doc["code"], refs_code(&home, &root, "LAW-STEP"));
    assert_eq!(doc["code"]["skipped"], 1);
    let lines = text(&home, &root);
    assert!(
        lines.stderr.lines().any(|line| line == refresh_note),
        "{}",
        lines.show()
    );
}

/// AC-6: no `[code]` (spec-c, spec-a, code-a with its table dropped): rows
/// 4-5 `off`, `code` null, their two `off` lines (never on stderr), and a
/// scratch `HOME` holds nothing after: no data directory resolved or
/// created. M: the index opened without `[code]`.
#[test]
fn no_code_table_resolves_no_data_directory() {
    let scratch = Scratch::new("health-no-code");
    let home = scratch.home("fresh");
    let c = tree(&scratch, "spec-c", "c", "", &[]);
    let a = tree(&scratch, "spec-a", "a", "", &[]);
    let tally = tree(&scratch, "code-a", "tally", "", &[]);
    drop_code_table(&tally);
    let off = json!({"state": "off", "causes": [NO_CODE], "counts": null, "rows": null});
    let off_drift = json!({"state": "off", "causes": [NO_CODE], "counts": null, "rows": null,
        "tested": null, "notes": []});
    for root in [&c, &a, &tally] {
        let context = root.display().to_string();
        let (run, _, doc) = document(&home, root);
        assert_eq!(doc["code"], Value::Null, "{context}");
        assert_eq!(doc["assumes"], off, "{context}");
        assert_eq!(doc["unbound"], off, "{context}");
        assert_eq!(doc["drift"], off_drift, "{context}");
        let lines = text(&home, root);
        assert!(
            lines.stdout.contains(&format!(
                "assumes: off: {NO_CODE}\nunbound: off: {NO_CODE}\ndrift: off: {NO_CODE}\n"
            )),
            "{context}: {}",
            lines.stdout
        );
        for done in [&run, &lines] {
            assert!(
                !done.stderr.contains("note: assumes:")
                    && !done.stderr.contains("note: unbound:")
                    && !done.stderr.contains("note: drift:"),
                "{context}: {}",
                done.show()
            );
        }
    }
    assert!(
        fs::read_dir(&home).expect("home").next().is_none(),
        "no data directory: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
}

/// TE-3: an md walk gap is still row 3's exit 2 (its causes stay first):
/// stdout empty, one `spec: the walk is incomplete: …: unapplied
/// amendments not measured` line, nothing opened (the `HOME` empty).
#[test]
fn md_walk_gap_is_still_exit_2() {
    let scratch = Scratch::new("health-md-gap");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    let home = scratch.home("fresh");
    let limits = root.join("docs/spec/limits.md");
    fs::set_permissions(&limits, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    for json in [true, false] {
        let run = health_run(&home, &root, json);
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        let line = run.stderr.trim_end();
        assert!(
            !line.contains('\n')
                && line.starts_with("spec: the walk is incomplete: ")
                && line.contains("`docs/spec/limits.md`")
                && line.ends_with(": unapplied amendments not measured"),
            "{}",
            run.show()
        );
    }
    fs::set_permissions(&limits, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    assert!(
        fs::read_dir(&home).expect("home").next().is_none(),
        "nothing opened: {:?}",
        snapshot(&home).keys().collect::<Vec<_>>()
    );
}

/// code-a with a law rule, `// @assumes PR-0001` appended to `lib.rs` and
/// `PR-0001` (a question on `GOAL-01`) stored in `home`'s queue.
fn queued_tally(scratch: &Scratch, dir: &str) -> (PathBuf, PathBuf) {
    let root = tree(scratch, "code-a", dir, LAW_RULE, &[]);
    append(&root, LIB, "// @assumes PR-0001\n");
    let home = scratch.home(dir);
    let db = slug_db(&home, "code-a");
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    let mut queue = SqliteQueue::open(&db, "code-a").expect("the queue opens");
    let goal = question("GOAL-01", "Does the count wrap at the cap?");
    assert_eq!(raise(&mut queue, &goal, 1), "PR-0001");
    (root, home)
}

/// TE-3: the queue's causes fail row 4 alone, whole: a queue of a newer
/// schema, a row that does not decode (the one a marker names), `PR` in
/// `[ids]`: row 4 `cannot` (one cause, no count), row 5 and row 3
/// measured, exit 0, `note: assumes: …` on stderr.
#[test]
fn queue_causes_fail_row_4_alone() {
    let scratch = Scratch::new("health-queue-causes");
    let checks = scratch.home("checks");
    let (newer, newer_home) = queued_tally(&scratch, "newer");
    sql(&slug_db(&newer_home, "code-a"), "PRAGMA user_version = 99;");
    let (corrupt, corrupt_home) = queued_tally(&scratch, "corrupt");
    sql(
        &slug_db(&corrupt_home, "code-a"),
        "UPDATE proposals SET target_ids = 'not a list' WHERE id = 'PR-0001';",
    );
    let (clash, clash_home) = queued_tally(&scratch, "clash");
    let config = read_text(&clash, "specengine.toml");
    write(
        &clash,
        "specengine.toml",
        with_ids_line(&config, "PR    = { kind = \"pull\", width = 4 }\n"),
    );
    for (root, home, fragment) in [
        (&newer, &newer_home, "schema version 99"),
        (&corrupt, &corrupt_home, "PR-0001"),
        (&clash, &clash_home, "`PR`"),
    ] {
        let context = format!("{}: {fragment}", root.display());
        let (run, _, doc) = document(home, root);
        let assumes = &doc["assumes"];
        assert_eq!(assumes["state"], "cannot", "{context}: {assumes}");
        assert_eq!(assumes["counts"], Value::Null, "{context}");
        assert_eq!(assumes["rows"], Value::Null, "{context}");
        let causes = assumes["causes"].as_array().expect("causes");
        assert_eq!(causes.len(), 1, "{context}: {causes:?}");
        let cause = causes[0].as_str().expect("a cause");
        assert!(cause.contains(fragment), "{context}: {cause}");
        assert_eq!(doc["unbound"]["state"], "measured", "{context}");
        assert_eq!(
            row_ids(&doc["unbound"]),
            ["LAW-SOURCE", "LAW-DEFAULT"],
            "{context}"
        );
        assert!(doc["code"].is_object(), "{context}");
        assert_eq!(
            doc["documents"],
            check_documents(&checks, root),
            "{context}"
        );
        let note = format!("note: assumes: {cause}");
        assert!(
            run.stderr.lines().any(|line| line == note),
            "{context}: {}",
            run.show()
        );
        assert!(!run.stderr.contains("note: unbound:"), "{context}");
    }
}

/// The queue of `home`'s DB for `slug`, its directory made.
fn open_queue(home: &Path, slug: &str) -> SqliteQueue {
    let db = slug_db(home, slug);
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    SqliteQueue::open(&db, slug).expect("the queue opens")
}

/// R8 as built (the developer's deviation 5, accepted): a queue row that
/// does not decode fails row 4 whole, never skipped, even when no marker
/// names it (code-a's `@assumes PREM-001` makes row 4 read the open and
/// approved items): its one cause `proposal <ID>: the stored `<column>`
/// cannot be read: …`, row 5 measured, `note: assumes: …`. A deciding item
/// approved without its record is such a row (the store's
/// `decode_record`), so the judge's "no choice: `otherwise`" never meets
/// one through the store. A tree whose markers judge no item (inkwell)
/// opens no queue: a corrupt one leaves row 4 measured, 0 rows.
#[test]
fn an_unreadable_queue_row_fails_row_4_whole() {
    let scratch = Scratch::new("health-r8");
    // A corrupt item no marker names.
    let root = tree(&scratch, "code-a", "unnamed", LAW_RULE, &[]);
    let home = scratch.home("unnamed");
    let mut queue = open_queue(&home, "code-a");
    let goal = question("GOAL-01", "Does the count wrap at the cap?");
    assert_eq!(raise(&mut queue, &goal, 1), "PR-0001");
    let step = question("LAW-STEP", "Does a step add one?");
    assert_eq!(raise(&mut queue, &step, 2), "PR-0002");
    drop(queue);
    sql(
        &slug_db(&home, "code-a"),
        "UPDATE proposals SET target_ids = 'not a list' WHERE id = 'PR-0002';",
    );
    // An approved question without its record, named by a marker.
    let named = tree(&scratch, "code-a", "named", LAW_RULE, &[]);
    append(&named, LIB, "// @assumes PR-0001\n");
    let named_home = scratch.home("named");
    let mut queue = open_queue(&named_home, "code-a");
    assert_eq!(raise(&mut queue, &goal, 1), "PR-0001");
    drop(queue);
    sql(
        &slug_db(&named_home, "code-a"),
        "UPDATE proposals SET status = 'approved', decided_by = 'Owner \
         <owner@example.invalid>', decided_at = '2026-10-05T10:02:00Z', \
         updated_at = '2026-10-05T10:02:00Z' WHERE id = 'PR-0001';",
    );
    for (root, home, start, reason) in [
        (
            &root,
            &home,
            "proposal PR-0002: the stored `target_ids` cannot be read: ",
            "",
        ),
        (
            &named,
            &named_home,
            "proposal PR-0001: the stored `record_id` cannot be read: ",
            "NULL on an approved question",
        ),
    ] {
        let context = root.display().to_string();
        let (run, _, doc) = document(home, root);
        let assumes = &doc["assumes"];
        assert_eq!(assumes["state"], "cannot", "{context}: {assumes}");
        assert_eq!(assumes["counts"], Value::Null, "{context}");
        let causes = assumes["causes"].as_array().expect("causes");
        assert_eq!(causes.len(), 1, "{context}: {assumes}");
        let cause = causes[0].as_str().expect("a cause");
        assert!(
            cause.starts_with(start) && cause.contains(reason),
            "{context}: {cause}"
        );
        assert_eq!(doc["unbound"]["state"], "measured", "{context}");
        let note = format!("note: assumes: {cause}");
        assert!(
            run.stderr.lines().any(|line| line == note),
            "{context}: {}",
            run.show()
        );
    }

    // No marker judged, no queue opened.
    let ink = tree(&scratch, "code-index", "ink", "", &[]);
    let ink_home = scratch.home("ink");
    let mut queue = open_queue(&ink_home, "inkwell");
    let quill = question("QUILL-02", "Is a broken item listed?");
    assert_eq!(raise(&mut queue, &quill, 1), "PR-0001");
    drop(queue);
    sql(
        &slug_db(&ink_home, "inkwell"),
        "UPDATE proposals SET target_ids = 'not a list' WHERE id = 'PR-0001';",
    );
    let (_, _, doc) = document(&ink_home, &ink);
    assert_eq!(doc["assumes"]["state"], "measured", "{}", doc["assumes"]);
    assert_eq!(doc["assumes"]["rows"], json!([]));
    assert_eq!(doc["assumes"]["counts"]["markers"], 0);
}

/// One drift item of `node` (`drift-queue`'s shape: `CAP`'s `code_ahead`),
/// its hash of `digit`.
fn drift_item(node: &str, digit: char) -> NewDrift {
    let row = DriftRow {
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
    };
    NewDrift {
        target_id: node.to_owned(),
        target_path: "spec.lock".to_owned(),
        place: place(),
        author: author(),
        task_id: None,
        severity: IntakeSeverity::Normal,
        summary: "code_ahead code_changed".to_owned(),
        evidence: serde_json::to_string(&DriftEvidence {
            rows: vec![row],
            more: 0,
        })
        .expect("JSON"),
        hash: format!("b3:{}", digit.to_string().repeat(64)),
    }
}

/// The developer's choice (accepted): a `drift` item locks, it does not
/// decide: approved, `pending` (`drift approved`); applied, `applied`
/// (`drift applied`); no choice, no record, no reason.
#[test]
fn drift_items_are_judged_as_non_deciding() {
    let scratch = Scratch::new("health-drift-items");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    append(&root, LIB, "// @assumes PR-0001\n// @assumes PR-0002\n");
    let home = scratch.home("h");
    let mut queue = open_queue(&home, "code-a");
    let created = queue
        .create_drift(
            &[drift_item("LAW-BOUND", '1'), drift_item("LAW-STEP", '2')],
            &at(1),
        )
        .expect("two drift items");
    let ids: Vec<String> = created
        .into_iter()
        .map(|result| match result {
            DriftResult::Created { proposal, .. } => proposal.id,
            other => panic!("not stored: {other:?}"),
        })
        .collect();
    assert_eq!(ids, ["PR-0001", "PR-0002"]);
    queue
        .approve("PR-0001", &decision("Because."), &at(2))
        .expect("approved");
    queue
        .approve("PR-0002", &decision("Because."), &at(3))
        .expect("approved");
    queue.applied("PR-0002", COMMIT, &at(4)).expect("applied");
    drop(queue);
    let (_, _, doc) = document(&home, &root);
    let rows: Vec<Value> = ["PR-0001", "PR-0002"]
        .iter()
        .flat_map(|id| assumes_rows(&doc, id))
        .cloned()
        .collect();
    assert_eq!(
        rows,
        [
            queue_row(
                "PR-0001",
                "pending",
                22,
                proposal("drift", "approved", Value::Null, None, None)
            ),
            queue_row(
                "PR-0002",
                "applied",
                23,
                proposal("drift", "applied", Value::Null, None, None)
            ),
        ],
        "{}",
        doc["assumes"]
    );
    let run = text(&home, &root);
    let lines = lines_with(&run, "assumes | ");
    for line in [
        format!("assumes | pending | PR-0001 | {LIB}:22:4 | assumes | drift approved"),
        format!("assumes | applied | PR-0002 | {LIB}:23:4 | assumes | drift applied"),
    ] {
        assert!(lines.contains(&line), "{line}\n{}", run.stdout);
    }
}

/// The developer's deviation 1 (accepted): rows 4-5 read the config row 3
/// located and read, once. With `--config <file>` outside the root, that
/// file's `[code]` and `[[health.bound]]` measure the rows (the root's own
/// config has no rule); a `--config` without `[code]` names that file, as
/// given, in `off`'s cause.
#[test]
fn rows_4_5_read_the_config_row_3_reads() {
    let scratch = Scratch::new("health-config");
    let root = scratch.copy("code-a", "tally");
    let config = read_text(&root, "specengine.toml");
    let (head, _) = config.split_once("\n[code]\n").expect("a [code] table");
    write(
        scratch.path(),
        "outside.toml",
        format!("{config}{LAW_RULE}"),
    );
    write(scratch.path(), "bare.toml", format!("{head}\n"));
    let home = scratch.home("h");
    let root_text = root.to_str().expect("a UTF-8 root");
    let health = |config: &str| -> Value {
        let given = scratch.join(config);
        let given = given.to_str().expect("a UTF-8 path");
        let run = spec(
            &home,
            scratch.path(),
            &["--root", root_text, "--config", given, "health", "--json"],
        );
        run.code(0);
        serde_json::from_str(run.stdout.trim_end()).expect("JSON")
    };
    let doc = health("outside.toml");
    assert_eq!(doc["unbound"]["state"], "measured", "{}", doc["unbound"]);
    assert_eq!(row_ids(&doc["unbound"]), ["LAW-SOURCE", "LAW-DEFAULT"]);
    assert_eq!(doc["assumes"]["state"], "measured");
    let doc = health("bare.toml");
    let bare = scratch.join("bare.toml");
    let cause = format!(
        "{} has no [code] table: the code layer is off (add [code] with roots)",
        bare.display()
    );
    for row in ["assumes", "unbound"] {
        assert_eq!(
            doc[row],
            json!({"state": "off", "causes": [cause], "counts": null, "rows": null}),
            "{row}"
        );
    }
    // docs/features/health-drift.md "Data": row 6 reads the same config.
    assert_eq!(
        doc["drift"],
        json!({"state": "off", "causes": [cause], "counts": null, "rows": null,
            "tested": null, "notes": []}),
    );
}

// ------------------------------------------------------------ iteration 2's targets

/// Queue schema 1's DDL (the store's `STEP_1`, as `queue_intake.rs` keeps
/// it): a slug DB no queue command has upgraded since.
const QUEUE_SCHEMA_1: &str = "
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

/// Iteration 2's target G2 (review 1's second minor, owner pre-approved):
/// a slug DB whose queue stands at an older schema (1), upgraded by no
/// queue command yet, gives row 4 `cannot` with the one cause `the queue
/// stands at schema 1; a queue command upgrades it` (on stderr as `note:
/// assumes: …`), row 5 measured, exit 0; once a queue command has run the
/// steps (the store's `open`), row 4 is measured. Red on iteration 1 by
/// design (an opaque `no such column` cause).
#[test]
fn an_older_queue_schema_is_named() {
    let scratch = Scratch::new("health-old-queue");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    let home = scratch.home("h");
    let db = slug_db(&home, "code-a");
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    sql(&db, QUEUE_SCHEMA_1);
    let (run, _, doc) = document(&home, &root);
    let cause = "the queue stands at schema 1; a queue command upgrades it";
    assert_eq!(
        doc["assumes"],
        json!({"state": "cannot", "causes": [cause], "counts": null, "rows": null}),
        "{}",
        run.show()
    );
    assert_eq!(doc["unbound"]["state"], "measured");
    assert!(
        run.stderr
            .lines()
            .any(|line| line == format!("note: assumes: {cause}")),
        "{}",
        run.show()
    );
    drop(SqliteQueue::open(&db, "code-a").expect("a queue command's steps"));
    let (_, _, doc) = document(&home, &root);
    assert_eq!(doc["assumes"]["state"], "measured", "{}", doc["assumes"]);
}

/// Iteration 2's target G3 (review 1's third minor, owner pre-approved):
/// the text tail prints the choice and ` | record …` only for `as_assumed`
/// and `otherwise`. A deciding item reopened after a failed apply keeps its
/// record and choice in the queue (`reopen` clears only the decision), yet
/// reads `question open`; rejected after that, `question rejected:
/// <reason>`. JSON stays as stored. Red on iteration 1 by design (both
/// tails print `{…} | record …`).
#[test]
fn a_reopened_or_rejected_item_shows_no_record_in_its_tail() {
    let scratch = Scratch::new("health-reopened");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    append(&root, LIB, "// @assumes PR-0001\n// @assumes PR-0002\n");
    let home = scratch.home("h");
    let mut queue = open_queue(&home, "code-a");
    let goal = question("GOAL-01", "Does the count wrap at the cap?");
    assert_eq!(raise(&mut queue, &goal, 1), "PR-0001");
    assert_eq!(
        decide(&mut queue, "PR-0001", Choice::WorkingAnswer, 2),
        "DEC-0001"
    );
    let failure = ApplyFailure {
        step: 3,
        reason: "the record cannot be written".to_owned(),
    };
    queue.reopen("PR-0001", &failure, &at(3)).expect("reopened");
    let default = question("LAW-DEFAULT", "Is the default cap ten?");
    assert_eq!(raise(&mut queue, &default, 4), "PR-0002");
    let no = Choice::Answer("No".to_owned());
    assert_eq!(decide(&mut queue, "PR-0002", no, 5), "DEC-0002");
    queue.reopen("PR-0002", &failure, &at(6)).expect("reopened");
    queue
        .reject("PR-0002", &decision("asked before"), &at(7))
        .expect("rejected");
    for (id, status) in [("PR-0001", "open"), ("PR-0002", "rejected")] {
        let stored = queue.get(id).expect("get").expect("stored");
        assert_eq!(stored.status.as_str(), status, "{id}");
        assert!(stored.record.is_some(), "{id} keeps its record");
    }
    drop(queue);
    let (_, _, doc) = document(&home, &root);
    assert_eq!(assumes_rows(&doc, "PR-0001")[0]["state"], "pending");
    assert_eq!(assumes_rows(&doc, "PR-0002")[0]["state"], "rejected");
    let run = text(&home, &root);
    let lines = lines_with(&run, "assumes | ");
    for line in [
        format!("assumes | pending | PR-0001 | {LIB}:22:4 | assumes | question open"),
        format!(
            "assumes | rejected | PR-0002 | {LIB}:23:4 | assumes | question rejected: asked \
             before"
        ),
    ] {
        assert!(lines.contains(&line), "{line}\n{}", run.stdout);
    }
}

/// Iteration 2's target G4 (review 1's nit, owner pre-approved): row 4 and
/// row 5 lines go through `escape_controls`: a rejection note holding ESC
/// prints `\u{1b}`, a section title holding BEL `\u{7}`; row 3's bytes stay
/// as they are (an amendment titled with BEL printed raw, as health-left
/// prints it); JSON keeps the note as stored. Red on iteration 1 by design
/// (the note and the title printed raw).
#[test]
fn row_4_and_5_lines_escape_control_characters() {
    let scratch = Scratch::new("health-escape");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        LAW_RULE,
        &[
            (
                "docs/features/cap-raise.md",
                "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [LAW-DEFAULT]\n---\n\n\
                 # Raise \u{7} the cap\n",
            ),
            (
                "docs/spec/bell.md",
                "---\nclass: canon\nid: PIECE-BELL\n---\n\n# Bell\n\n\
                 ## Ring\u{7} twice {#LAW-BELL}\n\nRings.\n",
            ),
        ],
    );
    append(&root, LIB, "// @assumes PR-0001\n");
    let home = scratch.home("h");
    let mut queue = open_queue(&home, "code-a");
    let goal = question("GOAL-01", "Does the count wrap at the cap?");
    assert_eq!(raise(&mut queue, &goal, 1), "PR-0001");
    let note = "asked \u{1b}[2J before";
    queue
        .reject("PR-0001", &decision(note), &at(2))
        .expect("rejected");
    drop(queue);
    let (_, _, doc) = document(&home, &root);
    assert_eq!(
        assumes_rows(&doc, "PR-0001")[0]["proposal"]["reason"],
        note,
        "JSON as stored"
    );
    let run = text(&home, &root);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let rejected = format!(
        "assumes | rejected | PR-0001 | {LIB}:22:4 | assumes | question rejected: asked \
         \\u{{1b}}[2J before"
    );
    assert!(
        lines.contains(&rejected.as_str()),
        "{rejected}\n{}",
        run.stdout
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("unbound | LAW-BELL | law | Ring\\u{7} twice | ")),
        "{}",
        run.stdout
    );
    assert!(
        lines.contains(
            &"docs/features/cap-raise.md | class spec | status draft | Raise \u{7} the cap"
        ),
        "row 3's line, raw: {}",
        run.stdout
    );
    assert!(!run.stdout.contains('\u{1b}'), "{}", run.stdout);
    assert_eq!(
        run.stdout.matches('\u{7}').count(),
        1,
        "row 3's BEL alone: {}",
        run.stdout
    );
}

/// G4 as built (iteration 2, the developer's deviation 2): a row's
/// `<row>: <state>: <cause>` line on stdout goes through `escape_controls`
/// too, its stderr `note: <row>: <cause>` stays raw (every command's
/// `Message` convention), JSON `causes` raw. A rule naming a kind that
/// holds BEL (the TOML escape `\u0007`): `unbound: cannot: …ru\u{7}ling…`
/// on stdout, the BEL itself on stderr and in JSON.
#[test]
fn a_row_5_cause_line_is_escaped_on_stdout_raw_on_stderr() {
    let scratch = Scratch::new("health-cause-escape");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        "\n[[health.bound]]\nkinds = [\"ru\\u0007ling\"]   # HERE\n",
        &[],
    );
    let at = line_with(&read_text(&root, "specengine.toml"), "# HERE");
    let cause = format!(
        "specengine.toml:{at}: `[[health.bound]]` `kinds`: `ru\u{7}ling` is the kind of no \
         `[ids]` prefix"
    );
    let home = scratch.home("h");
    let (json, _, doc) = document(&home, &root);
    assert_eq!(
        doc["unbound"],
        json!({"state": "cannot", "causes": [cause], "counts": null, "rows": null}),
        "JSON raw"
    );
    let run = text(&home, &root);
    let escaped = format!("unbound: cannot: {}", cause.replace('\u{7}', "\\u{7}"));
    assert!(
        run.stdout.lines().any(|line| line == escaped),
        "{escaped}\n{}",
        run.stdout
    );
    assert!(!run.stdout.contains('\u{7}'), "{}", run.stdout);
    for done in [&json, &run] {
        assert!(
            done.stderr
                .lines()
                .any(|line| line == format!("note: unbound: {cause}")),
            "stderr raw: {}",
            done.show()
        );
    }
}

/// "Failures and exits", row 5's precedence: `off` (no `[code]`, a bad rule
/// beside it); else `not_configured` (no rule, a code gap beside it); else
/// `cannot`, every cause, `[health]`'s then the code's; a bad rule alone:
/// row 5 `cannot`, row 4 measured.
#[test]
fn row_5_precedence_off_not_configured_cannot() {
    let scratch = Scratch::new("health-precedence");
    let bad_rule = "\n[[health.bound]]\nkinds = 3\n";
    // `off` first.
    let c = tree(&scratch, "spec-c", "c", bad_rule, &[]);
    let (_, _, doc) = document(&scratch.home("c"), &c);
    assert_eq!(
        doc["unbound"],
        json!({"state": "off", "causes": [NO_CODE], "counts": null, "rows": null})
    );
    // `not_configured` before a code gap.
    let no_rule = tree(&scratch, "code-a", "no-rule", "\n[health]\n", &[]);
    let home = scratch.home("no-rule");
    let env = [("HOME", home.as_os_str())];
    let lib = no_rule.join(LIB);
    fs::set_permissions(&lib, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let gap = failing_line(&env, &no_rule);
    let (_, _, doc) = document(&home, &no_rule);
    assert_eq!(
        doc["unbound"],
        json!({"state": "not_configured", "causes": [NO_RULE], "counts": null, "rows": null})
    );
    assert_eq!(
        doc["assumes"],
        json!({"state": "cannot", "causes": [gap.clone()], "counts": null, "rows": null})
    );
    fs::set_permissions(&lib, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    // `cannot`: `[health]`'s cause, then the code's.
    let both_bad = tree(&scratch, "code-a", "both-bad", bad_rule, &[]);
    let config = read_text(&both_bad, "specengine.toml");
    let at = line_with(&config, "kinds = 3");
    let home = scratch.home("both-bad");
    let env = [("HOME", home.as_os_str())];
    let lib = both_bad.join(LIB);
    fs::set_permissions(&lib, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let gap = failing_line(&env, &both_bad);
    let (_, _, doc) = document(&home, &both_bad);
    let unbound = &doc["unbound"];
    assert_eq!(unbound["state"], "cannot", "{unbound}");
    let causes = unbound["causes"].as_array().expect("causes");
    assert_eq!(causes.len(), 2, "{unbound}");
    assert!(
        causes[0]
            .as_str()
            .is_some_and(|cause| cause.starts_with(&format!("specengine.toml:{at}: "))),
        "{unbound}"
    );
    assert_eq!(causes[1], gap.as_str(), "{unbound}");
    assert_eq!(doc["assumes"]["causes"], json!([gap]));
    fs::set_permissions(&lib, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    // A bad rule alone.
    let (_, _, doc) = document(&home, &both_bad);
    assert_eq!(doc["unbound"]["state"], "cannot");
    assert_eq!(
        doc["unbound"]["causes"].as_array().map(Vec::len),
        Some(1),
        "{}",
        doc["unbound"]
    );
    assert_eq!(doc["assumes"]["state"], "measured");
}

// ------------------------------------------------------------ RD-5: the rules

/// RD-5 (Q3): no `[[health.bound]]` rule, `[health]` absent, empty or with
/// `bound = []`: row 5 `not_configured` with its sentence, never every
/// node unbound; the text's line; nothing on stderr for it. M: all
/// selected by default.
#[test]
fn no_rule_is_not_configured_never_all_unbound() {
    let scratch = Scratch::new("health-no-rule");
    let home = scratch.home("h");
    let want = json!({"state": "not_configured", "causes": [NO_RULE], "counts": null,
        "rows": null});
    for (dir, tail) in [
        ("absent", ""),
        ("empty", "\n[health]\n"),
        ("no-bound", "\n[health]\nbound = []\n"),
    ] {
        let root = tree(&scratch, "code-a", dir, tail, &[]);
        let (run, _, doc) = document(&home, &root);
        assert_eq!(doc["unbound"], want, "{dir}");
        assert_eq!(doc["assumes"]["state"], "measured", "{dir}");
        let lines = text(&home, &root);
        assert!(
            lines
                .stdout
                .lines()
                .any(|line| line == format!("unbound: not configured: {NO_RULE}")),
            "{dir}: {}",
            lines.stdout
        );
        assert!(
            !lines
                .stdout
                .lines()
                .any(|line| line.starts_with("unbound | ")),
            "{dir}"
        );
        assert!(!run.stderr.contains("note: unbound:"), "{dir}");
    }
}

/// RD-5: `ProjectConfig` only name-checks `[health]`: a valid rule and a
/// malformed one change no other command (`spec index`, `refs`, `show`,
/// `unmapped` answer as without them), a misspelled top-level table is
/// still refused by name. M: `health` not name-checked.
#[test]
fn binding_rule_table_is_name_checked() {
    let scratch = Scratch::new("health-name-check");
    let root = scratch.copy("code-a", "tally");
    let home = scratch.home("h");
    let config = read_text(&root, "specengine.toml");
    let reads = |home: &Path| -> Vec<(i32, String)> {
        let indexed = spec(home, &root, &["index"]);
        assert_eq!(indexed.code, 0, "{}", indexed.show());
        [
            &["refs", "LAW-STEP", "--json"][..],
            &["show", "LAW-STEP", "--json"],
            &["unmapped", "--json"],
            &["unmapped", "crates/tally/src/broken.rs", "--json"],
        ]
        .iter()
        .map(|args| {
            let run = spec(home, &root, args);
            (run.code, run.stdout)
        })
        .collect()
    };
    let base = reads(&home);
    assert!(base.iter().all(|(code, _)| *code == 0), "{base:?}");
    for tail in [
        LAW_RULE,
        "\n[[health.bound]]\nkinds = 3\n",
        "\n[health]\nbond = 1\n",
    ] {
        write(&root, "specengine.toml", format!("{config}{tail}"));
        assert_eq!(reads(&home), base, "{tail:?}");
    }
    write(
        &root,
        "specengine.toml",
        format!("{config}\n[helth]\nx = 1\n"),
    );
    let refused = spec(&home, &root, &["refs", "LAW-STEP"]);
    refused.code(2);
    assert!(
        refused.stderr.contains("unknown field `helth`"),
        "{}",
        refused.show()
    );
}

/// RD-5 ("`[[health.bound]]`"): a malformed rule, `[[check.rules]]`'
/// grammar and messages for the selectors, the TOML reader's for another
/// key: row 5 `cannot`, the one cause `specengine.toml:<line>: message` at
/// the offending value (the rule's header for a missing selector), row 4
/// measured, exit 0, `note: unbound: …` on stderr. As built (the
/// developer's deviation 4, accepted): check's wording with the prefix
/// `` `[[health.bound]]` `` for `check rule`, pinned whole; the TOML
/// reader's own for an unknown key, pinned by its start; `[health] bound`
/// not an array of tables. M: an unknown key taken.
#[test]
fn binding_rule_unknown_key_is_refused() {
    let scratch = Scratch::new("health-bad-rules");
    let home = scratch.home("h");
    // (the config's tail, the message after `specengine.toml:<line>: `,
    // whole or its start).
    let cases: [(&str, &str, bool); 12] = [
        (
            "\n[[health.bound]]\nkinds = [\"law\"]\nflavour = 1   # HERE\n",
            "unknown field `flavour`",
            false,
        ),
        (
            "\n[health]\nbond = 1   # HERE\n\n[[health.bound]]\nkinds = [\"law\"]\n",
            "unknown field `bond`",
            false,
        ),
        (
            "\n[[health.bound]]\nkinds = [\"law\", \"ruling\"]   # HERE\n",
            "`[[health.bound]]` `kinds`: `ruling` is the kind of no `[ids]` prefix",
            true,
        ),
        (
            "\n[[health.bound]]\nclasses = [\"canon\", \"record\"]   # HERE\n",
            "`[[health.bound]]` `classes`: `record` is none of canon | decision | spec | \
             generated",
            true,
        ),
        (
            "\n[[health.bound]]\nkinds = []   # HERE\n",
            "`[[health.bound]]` `kinds` is empty",
            true,
        ),
        (
            "\n[[health.bound]]\nkinds = 3   # HERE\n",
            "`[[health.bound]]` `kinds`: a list of strings, not an integer",
            true,
        ),
        (
            "\n[[health.bound]]\npaths = [\"docs/../x.md\"]   # HERE\n",
            "`[[health.bound]]` `paths`: ",
            false,
        ),
        (
            "\n[[health.bound]]\nkinds = [\"law\"]\nwhen = { status = 3 }   # HERE\n",
            "`[[health.bound]]` `when` `status`: a string or a list of strings, not an integer",
            true,
        ),
        (
            "\n[[health.bound]]\nkinds = [\"law\"]\nrelations = []   # HERE\n",
            "`[[health.bound]]` `relations` is empty",
            true,
        ),
        (
            "\n[[health.bound]]\nkinds = [\"law\"]\nrelations = [\"binds\"]   # HERE\n",
            "`[[health.bound]]` `relations`: `binds` is none of implements | verifies | \
             configures | assumes",
            true,
        ),
        (
            "\n[[health.bound]]   # HERE\nrelations = [\"implements\"]\n",
            "`[[health.bound]]` without a selector: give `kinds`, `classes`, `paths` or `when`",
            true,
        ),
        (
            "\n[health]\nbound = 3   # HERE\n",
            "`[health] bound` is an array of tables (`[[health.bound]]`), not an integer",
            true,
        ),
    ];
    for (index, (tail, message, whole)) in cases.iter().enumerate() {
        let root = tree(&scratch, "code-a", &format!("case-{index}"), tail, &[]);
        let config = read_text(&root, "specengine.toml");
        let at = line_with(&config, "# HERE");
        let (run, _, doc) = document(&home, &root);
        let context = format!("{tail:?}");
        let unbound = &doc["unbound"];
        assert_eq!(unbound["state"], "cannot", "{context}: {unbound}");
        assert_eq!(unbound["counts"], Value::Null, "{context}");
        let causes = unbound["causes"].as_array().expect("causes");
        assert_eq!(causes.len(), 1, "{context}: {unbound}");
        let cause = causes[0].as_str().expect("a cause");
        let want = format!("specengine.toml:{at}: {message}");
        if *whole {
            assert_eq!(cause, want, "{context}");
        } else {
            assert!(cause.starts_with(&want), "{context}: {cause}");
        }
        assert_eq!(doc["assumes"]["state"], "measured", "{context}");
        assert!(
            run.stderr
                .lines()
                .any(|line| line == format!("note: unbound: {cause}")),
            "{context}: {}",
            run.show()
        );
    }
}

/// RD-5 ("`[[health.bound]]`": "The first error by line"): a rule whose
/// bad kind comes before its unknown key is refused at the kind's line.
#[test]
fn binding_rule_first_error_by_line() {
    let scratch = Scratch::new("health-first-error");
    let tail = "\n[[health.bound]]\nkinds = [\"ruling\"]   # HERE\nflavour = 1\n";
    let root = tree(&scratch, "code-a", "tally", tail, &[]);
    let at = line_with(&read_text(&root, "specengine.toml"), "# HERE");
    let (_, _, doc) = document(&scratch.home("h"), &root);
    let causes = doc["unbound"]["causes"].as_array().expect("causes").clone();
    assert_eq!(causes.len(), 1, "{}", doc["unbound"]);
    let cause = causes[0].as_str().expect("a cause");
    assert!(
        cause.starts_with(&format!("specengine.toml:{at}: ")) && cause.contains("`ruling`"),
        "{cause}"
    );
}

/// RD-5 ("Candidates"): a document by its own kind, a `{#ID}` section by
/// its ID's prefix kind and its document's class, path and front-matter;
/// selectors AND, the items of a list OR. code-a: `kinds = ["piece"]`
/// selects the two documents, never their law sections; `["law"]` the
/// five sections; `classes = ["spec"]` and `when = {status = "draft"}`
/// `PASS-01` (its document's, which has no ID); `paths` the spec files'
/// seven; `kinds` with `paths` two. M: a section by its document's kind.
#[test]
fn section_selected_by_its_own_prefix_kind() {
    let scratch = Scratch::new("health-selection");
    let home = scratch.home("h");
    let root = scratch.copy("code-a", "tally");
    let config = read_text(&root, "specengine.toml");
    for (rule, selected, unbound) in [
        ("kinds = [\"piece\"]", 2, vec!["PIECE-LIMITS"]),
        ("kinds = [\"law\"]", 5, vec!["LAW-SOURCE", "LAW-DEFAULT"]),
        ("kinds = [\"pass-mark\"]", 1, vec![]),
        ("classes = [\"spec\"]", 1, vec![]),
        ("when = { status = \"draft\" }", 1, vec![]),
        (
            "paths = [\"docs/spec/**\"]",
            7,
            vec!["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"],
        ),
        (
            "kinds = [\"law\"]\npaths = [\"docs/spec/limits.md\"]",
            2,
            vec!["LAW-SOURCE", "LAW-DEFAULT"],
        ),
        (
            "kinds = [\"law\", \"piece\"]",
            7,
            vec!["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"],
        ),
        (
            "classes = [\"canon\"]",
            9,
            vec!["PIECE-LIMITS", "LAW-SOURCE", "LAW-DEFAULT"],
        ),
    ] {
        write(
            &root,
            "specengine.toml",
            format!("{config}\n[[health.bound]]\n{rule}\n"),
        );
        let (_, _, doc) = document(&home, &root);
        let counts = &doc["unbound"]["counts"];
        assert_eq!(counts["selected"], selected, "{rule}: {}", doc["unbound"]);
        assert_eq!(row_ids(&doc["unbound"]), unbound, "{rule}");
    }
}

/// "Candidates": a node of a file whose front-matter does not read is no
/// candidate (row 3 notes the file); nor is one of a Tier 3 or generated
/// file ([`archived_nodes_are_never_unbound`]).
#[test]
fn a_file_whose_front_matter_fails_offers_no_candidate() {
    let scratch = Scratch::new("health-broken-candidate");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        LAW_RULE,
        &[(
            "docs/spec/broken-laws.md",
            "---\nclass: canon\ntitle: Broken: a colon\n---\n\n# Broken\n\n\
             ## Lost {#LAW-LOST}\n\nNever selected.\n",
        )],
    );
    let (_, _, doc) = document(&scratch.home("h"), &root);
    assert_eq!(
        doc["unbound"]["counts"]["selected"], 5,
        "{}",
        doc["unbound"]
    );
    assert!(!row_ids(&doc["unbound"]).contains(&"LAW-LOST".to_owned()));
    assert_eq!(doc["notes"].as_array().map(Vec::len), Some(1));
}

// ------------------------------------------------------------ RD-6: what binds

/// RD-6: a mention never binds: `LAW-SOURCE` named only in a comment on an
/// item stays unbound, `mentions` 1 (`spec refs`' count), no marker. M:
/// mentions bind.
#[test]
fn mentions_never_bind() {
    let scratch = Scratch::new("health-mentions");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        LAW_RULE,
        &[(
            "crates/tally/src/source.rs",
            "// Reads LAW-SOURCE from the data file.\npub fn source() -> u32 {\n    10\n}\n",
        )],
    );
    let home = scratch.home("h");
    let refs = refs_node(&home, &root, "LAW-SOURCE");
    assert_eq!(refs["mentions"].as_array().map(Vec::len), Some(1), "{refs}");
    assert_eq!(refs["markers"], json!([]));
    let (_, _, doc) = document(&home, &root);
    let row = unbound_row(&doc, "LAW-SOURCE");
    assert_eq!(row["mentions"], 1, "{row}");
    assert_eq!(row["markers"], json!([]), "{row}");
}

/// RD-6: an `unanchored` marker (in a function body) binds nothing: listed
/// with `binding` and `reason` `unanchored`; `spec unmapped` maps nothing
/// in its file. M: `unanchored` binds.
#[test]
fn unanchored_marker_binds_nothing() {
    let scratch = Scratch::new("health-unanchored");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        LAW_RULE,
        &[(
            "crates/tally/src/lone.rs",
            "pub fn lone() -> u32 {\n    // @implements LAW-SOURCE\n    10\n}\n",
        )],
    );
    let home = scratch.home("h");
    let (_, _, doc) = document(&home, &root);
    let row = unbound_row(&doc, "LAW-SOURCE");
    assert_eq!(
        row["markers"],
        json!([marker(
            "crates/tally/src/lone.rs",
            2,
            8,
            "implements",
            "resolved",
            "unanchored",
            "unanchored"
        )]),
        "{row}"
    );
    let unmapped = unmapped_file(&home, &root, "crates/tally/src/lone.rs");
    assert_eq!(unmapped["rows"][0]["mapped"], 0, "{unmapped}");
}

/// AC-3 (X3, Q14): a broken item's marker (`cannot_verify`) binds nothing.
/// code-a with `broken.rs:4` rewritten to `// @implements LAW-DEFAULT`
/// (no other marker names it): row 5 lists `LAW-DEFAULT` with that
/// marker, `binding` and `reason` `cannot_verify`; `spec unmapped
/// crates/tally/src/broken.rs` reads `units 2`, `mapped 1`, `broken`
/// unmapped. inkwell, a second project, as shipped: `QUILL-02`'s one
/// marker is its broken item's. M: `cannot_verify` binds.
#[test]
fn cannot_verify_marker_binds_nothing() {
    let scratch = Scratch::new("health-x3");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    replace(
        &root,
        "crates/tally/src/broken.rs",
        "// @implements GOAL-01@1",
        "// @implements LAW-DEFAULT",
    );
    let home = scratch.home("h");
    let (_, _, doc) = document(&home, &root);
    assert_eq!(
        unbound_row(&doc, "LAW-DEFAULT"),
        &json!({"id": "LAW-DEFAULT", "kind": "law", "title": "Default",
            "path": "docs/spec/limits.md", "line": 16, "rules": [1], "mentions": 0,
            "markers": [marker("crates/tally/src/broken.rs", 4, 4, "implements", "resolved",
                "cannot_verify", "cannot_verify")]})
    );
    let unmapped = unmapped_file(&home, &root, "crates/tally/src/broken.rs");
    assert_eq!(unmapped["rows"][0]["units"], 2, "{unmapped}");
    assert_eq!(unmapped["rows"][0]["mapped"], 1, "{unmapped}");
    assert_eq!(unmapped_qpaths(&unmapped), ["crates/tally::broken::broken"]);
    let lines = spec(&home, &root, &["unmapped", "crates/tally/src/broken.rs"]);
    lines.code(0);
    assert!(
        lines
            .stdout
            .starts_with("crates/tally/src/broken.rs | units 2 | mapped 1 | 50.0 % | "),
        "{}",
        lines.stdout
    );

    let ink = tree(
        &scratch,
        "code-index",
        "ink",
        "\n[[health.bound]]\nkinds = [\"quill\"]\n",
        &[],
    );
    let (_, _, doc) = document(&home, &ink);
    assert_eq!(
        doc["unbound"]["counts"],
        json!({"rules": 1, "selected": 2, "bound": 1, "unbound": 1})
    );
    assert_eq!(
        unbound_row(&doc, "QUILL-02")["markers"],
        json!([marker(
            "crates/inkwell/src/broken.rs",
            3,
            4,
            "implements",
            "resolved",
            "cannot_verify",
            "cannot_verify"
        )])
    );
    assert_eq!(unbound_row(&doc, "QUILL-02")["mentions"], 1);
}

/// AC-3's tree whole, against the spec's own example ("Row 4", "Row 5",
/// "Text"): a fresh `HOME` (no queue), code-a's laws by default relations,
/// `broken.rs:4` naming `LAW-DEFAULT`: the document byte for byte and the
/// text line for line (`documents` = `spec check`'s).
/// docs/features/health-drift.md "Degrade matrix": the tree holds no
/// worktree, so row 6 is `cannot` with plain's cause (4), its line after
/// rows 4-5's summaries.
#[test]
fn ac_3_tree_document_and_text_are_the_specs_example() {
    let scratch = Scratch::new("health-ac3-golden");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    replace(
        &root,
        "crates/tally/src/broken.rs",
        "// @implements GOAL-01@1",
        "// @implements LAW-DEFAULT",
    );
    let documents = check_documents(&scratch.home("checks"), &root);
    let home = scratch.home("fresh");
    let (run, _, _) = document(&home, &root);
    let want = r#"{"documents":DOCS,"amendments":[],"notes":[],"code":{"files":8,"rs":7,"ron":1,"skipped":0},"assumes":{"state":"measured","causes":[],"counts":{"markers":2,"rows":1,"pending":0,"as_assumed":0,"otherwise":0,"applied":0,"rejected":0,"unknown":1,"archived":0,"in_question":0},"rows":[{"id":"PR-7","state":"unknown","path":"crates/tally/src/lib.rs","line":13,"column":4,"relation":"implements","proposal":null,"node":null}]},"unbound":{"state":"measured","causes":[],"counts":{"rules":1,"selected":5,"bound":3,"unbound":2},"rows":[{"id":"LAW-SOURCE","kind":"law","title":"Source","path":"docs/spec/limits.md","line":12,"rules":[1],"mentions":0,"markers":[]},{"id":"LAW-DEFAULT","kind":"law","title":"Default","path":"docs/spec/limits.md","line":16,"rules":[1],"mentions":0,"markers":[{"path":"crates/tally/src/broken.rs","line":4,"column":4,"relation":"implements","id_state":"resolved","binding":"cannot_verify","reason":"cannot_verify"}]}]},"drift":{"state":"cannot","causes":["no git repository was found for the root, or git could not read it"],"counts":null,"rows":null,"tested":null,"notes":[]}}"#;
    assert_eq!(
        run.stdout,
        format!("{}\n", want.replace("DOCS", &documents.to_string()))
    );
    let lines = text(&home, &root);
    assert_eq!(
        lines.stdout,
        format!(
            "assumes | unknown | PR-7 | crates/tally/src/lib.rs:13:4 | implements\n\
             unbound | LAW-SOURCE | law | Source | docs/spec/limits.md:12 | rules 1 | mentions 0\n\
             unbound | LAW-DEFAULT | law | Default | docs/spec/limits.md:16 | rules 1 | mentions 0\n\
             \x20 implements | crates/tally/src/broken.rs:4:4 | resolved | cannot_verify | \
             cannot_verify\n\
             assumes: 1 rows of 2 markers: 0 pending, 0 as_assumed, 0 otherwise, 0 applied, \
             0 rejected, 1 unknown, 0 archived, 0 in_question\n\
             unbound: 2 of 5 selected nodes, 1 rules\n\
             drift: cannot: no git repository was found for the root, or git could not read it\n\
             code: 8 files (.rs 7, .ron 1), skipped 0\n\
             spec health: {documents} documents, 0 unapplied amendments\n"
        )
    );
}

/// RD-6 (Q14): a `ron_ambiguous` marker binds: by `configures` alone,
/// `LAW-STEP`'s one such marker is `data/limits.ron:6`, a duplicate field
/// (`spec refs`: `ron`, ambiguity `duplicate`), and `LAW-STEP` is bound.
/// M: `ron_ambiguous` refused.
#[test]
fn ron_ambiguous_marker_binds() {
    let scratch = Scratch::new("health-ron-ambiguous");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        "\n[[health.bound]]\nkinds = [\"law\"]\nrelations = [\"configures\"]\n",
        &[],
    );
    let home = scratch.home("h");
    let step = refs_node(&home, &root, "LAW-STEP");
    let configures: Vec<&Value> = step["markers"]
        .as_array()
        .expect("markers")
        .iter()
        .filter(|marker| marker["relation"] == "configures")
        .collect();
    assert_eq!(configures.len(), 1, "{step}");
    assert_eq!(configures[0]["path"], "data/limits.ron");
    assert_eq!(configures[0]["binding"]["kind"], "ron");
    assert_eq!(configures[0]["binding"]["ambiguity"], "duplicate");
    let (_, _, doc) = document(&home, &root);
    assert_eq!(
        row_ids(&doc["unbound"]),
        ["LAW-ODD", "LAW-SOURCE", "LAW-DEFAULT"],
        "{}",
        doc["unbound"]
    );
    assert_eq!(
        doc["unbound"]["counts"],
        json!({"rules": 1, "selected": 5, "bound": 2, "unbound": 3})
    );
}

/// RD-6, AC-5: a look-alike ID binds nothing, nor its fix's node: an item
/// marker `@implements L\u{0410}W-SOURCE` leaves `LAW-SOURCE` unbound,
/// listed under it (`id_state` `homoglyph`, `binding` `item`, `reason`
/// `homoglyph`), and `spec unmapped` maps nothing in its file. M: `maps`
/// ignores the ID state (with `symbol_unmapped.rs`
/// `dangling_marker_never_maps`).
#[test]
fn look_alike_marker_binds_nothing() {
    let scratch = Scratch::new("health-look-alike");
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        LAW_RULE,
        &[(
            "crates/tally/src/look.rs",
            "// @implements L\u{0410}W-SOURCE\npub fn look() {}\n",
        )],
    );
    let home = scratch.home("h");
    let refs = refs_node(&home, &root, "LAW-SOURCE");
    assert_eq!(refs["markers"][0]["id_state"], "homoglyph", "{refs}");
    let (_, _, doc) = document(&home, &root);
    assert_eq!(
        unbound_row(&doc, "LAW-SOURCE")["markers"],
        json!([marker(
            "crates/tally/src/look.rs",
            1,
            4,
            "implements",
            "homoglyph",
            "item",
            "homoglyph"
        )])
    );
    let unmapped = unmapped_file(&home, &root, "crates/tally/src/look.rs");
    assert_eq!(unmapped["rows"][0]["mapped"], 0, "{unmapped}");
}

/// RD-6: containment never applies between nodes: a document's marker
/// binds none of its sections (`PIECE-LIMITS` bound, `LAW-SOURCE`,
/// `LAW-DEFAULT` not), a section's marker not its document (`LAW-SOURCE`
/// bound, `PIECE-LIMITS` not). M: a document's marker covers its sections.
#[test]
fn a_document_marker_binds_no_section() {
    let scratch = Scratch::new("health-containment");
    let home = scratch.home("h");
    let rule = "\n[[health.bound]]\nkinds = [\"piece\", \"law\"]\n";
    for (dir, id, unbound) in [
        ("document", "PIECE-LIMITS", ["LAW-SOURCE", "LAW-DEFAULT"]),
        ("section", "LAW-SOURCE", ["PIECE-LIMITS", "LAW-DEFAULT"]),
    ] {
        let file = format!("// @implements {id}\npub fn cap() -> u32 {{\n    10\n}}\n");
        let root = tree(
            &scratch,
            "code-a",
            dir,
            rule,
            &[("crates/tally/src/cap.rs", file.as_str())],
        );
        let (_, _, doc) = document(&home, &root);
        assert_eq!(
            row_ids(&doc["unbound"]),
            unbound,
            "{id}: {}",
            doc["unbound"]
        );
        for other in unbound {
            assert_eq!(
                unbound_row(&doc, other)["markers"],
                json!([]),
                "{id}: {other}"
            );
        }
    }
}

/// RD-6: an `@assumes` binds nothing unless the rule lists `assumes`: on
/// an item, by default relations, `LAW-SOURCE` stays unbound (`reason`
/// `relation`); with `relations = ["assumes"]` it is bound. M: `@assumes`
/// binds.
#[test]
fn assumes_marker_binds_nothing_by_default() {
    let scratch = Scratch::new("health-assumes-binds");
    let home = scratch.home("h");
    let file = [(
        "crates/tally/src/assumed.rs",
        "// @assumes LAW-SOURCE\npub fn assumed() -> u32 {\n    10\n}\n",
    )];
    let by_default = tree(&scratch, "code-a", "default", LAW_RULE, &file);
    let (_, _, doc) = document(&home, &by_default);
    assert_eq!(
        unbound_row(&doc, "LAW-SOURCE")["markers"],
        json!([marker(
            "crates/tally/src/assumed.rs",
            1,
            4,
            "assumes",
            "resolved",
            "item",
            "relation"
        )])
    );
    let listed = tree(
        &scratch,
        "code-a",
        "listed",
        "\n[[health.bound]]\nkinds = [\"law\"]\nrelations = [\"assumes\"]\n",
        &file,
    );
    let (_, _, doc) = document(&home, &listed);
    assert!(
        !row_ids(&doc["unbound"]).contains(&"LAW-SOURCE".to_owned()),
        "{}",
        doc["unbound"]
    );
}

/// RD-6 ("Candidates": live nodes): never a node of a Tier 3 document
/// (`LAW-OLD` in a shipped spec, `PREM-002` superseded, `PREM-004`
/// rejected) nor of a generated one (`LAW-GEN`): seven selected, the
/// accepted `PREM-003` listed. M: Tier 3 selected.
#[test]
fn archived_nodes_are_never_unbound() {
    let scratch = Scratch::new("health-archived");
    let mut files: Vec<(&str, &str)> = TE1_FILES[1..].to_vec();
    files.push((
        "docs/spec/old-limits.md",
        "---\nclass: spec\nstatus: shipped\nid: PIECE-OLD\n---\n\n# Old limits\n\n\
         ## Old cap {#LAW-OLD}\n\nThe cap was five.\n",
    ));
    files.push((
        "docs/spec/digest.md",
        "---\nclass: generated\n---\n\n# Digest\n\n## Digest cap {#LAW-GEN}\n\nTen.\n",
    ));
    let root = tree(
        &scratch,
        "code-a",
        "tally",
        "\n[[health.bound]]\nkinds = [\"law\", \"premise\"]\n",
        &files,
    );
    let (_, _, doc) = document(&scratch.home("h"), &root);
    assert_eq!(
        doc["unbound"]["counts"]["selected"], 7,
        "{}",
        doc["unbound"]
    );
    assert_eq!(
        row_ids(&doc["unbound"]),
        ["PREM-003", "LAW-SOURCE", "LAW-DEFAULT"],
        "{}",
        doc["unbound"]
    );
}

/// "What binds" and "Row 4" for an ambiguous ID (two documents declare
/// `GOAL-03`): a marker binds each holder (by default relations both are
/// bound); by `configures` alone both holders are listed, each with the
/// marker (`id_state` `ambiguous`), by path; an `@assumes GOAL-03` under
/// an open question is one row per holder, by holder path, one marker
/// judged.
#[test]
fn ambiguous_id_counts_each_holder() {
    let scratch = Scratch::new("health-ambiguous");
    let files = [
        (
            "docs/records/GOAL-03.md",
            "---\nclass: canon\nid: GOAL-03\nrev: 2\nowner: owner\nreviewed: 2026-10-10\n---\n\n\
             # First holder\n",
        ),
        (
            "docs/records/GOAL-03-twin.md",
            "---\nclass: canon\nid: GOAL-03\nowner: owner\nreviewed: 2026-10-10\n---\n\n\
             # Second holder\n",
        ),
        (
            "crates/tally/src/twin.rs",
            "// @implements GOAL-03@2\npub fn twin() {}\n\n// @assumes GOAL-03\npub fn leaned() {}\n",
        ),
    ];
    let by_default = tree(
        &scratch,
        "code-a",
        "default",
        "\n[[health.bound]]\nkinds = [\"objective\"]\n",
        &files,
    );
    let home = scratch.home("default");
    let (_, _, doc) = document(&home, &by_default);
    assert_eq!(
        doc["unbound"]["counts"],
        json!({"rules": 1, "selected": 3, "bound": 3, "unbound": 0}),
        "{}",
        doc["unbound"]
    );

    let configures = tree(
        &scratch,
        "code-a",
        "configures",
        "\n[[health.bound]]\nkinds = [\"objective\"]\nrelations = [\"configures\"]\n",
        &files,
    );
    let home = scratch.home("configures");
    let db = slug_db(&home, "code-a");
    fs::create_dir_all(db.parent().expect("the data directory")).expect("the data directory");
    let mut queue = SqliteQueue::open(&db, "code-a").expect("the queue opens");
    let twins = question("GOAL-03", "Which holder is meant?");
    assert_eq!(raise(&mut queue, &twins, 1), "PR-0001");
    drop(queue);
    let (_, _, doc) = document(&home, &configures);
    let rows = doc["unbound"]["rows"].as_array().expect("rows");
    let places: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("an ID"),
                row["path"].as_str().expect("path"),
            )
        })
        .collect();
    assert_eq!(
        places,
        [
            ("GOAL-01", "docs/records/GOAL-01.md"),
            ("GOAL-03", "docs/records/GOAL-03-twin.md"),
            ("GOAL-03", "docs/records/GOAL-03.md"),
        ],
        "{}",
        doc["unbound"]
    );
    let twin = "crates/tally/src/twin.rs";
    for row in &rows[1..] {
        assert_eq!(
            row["markers"],
            json!([
                marker(twin, 1, 4, "implements", "ambiguous", "item", "relation"),
                marker(twin, 4, 4, "assumes", "ambiguous", "item", "relation"),
            ]),
            "{row}"
        );
    }
    let holder = |path: &str| {
        json!({"id": "GOAL-03", "state": "in_question", "path": twin, "line": 4, "column": 4,
            "relation": "assumes", "proposal": null,
            "node": {"id": "GOAL-03", "path": path, "line": 1, "status": null,
                "successor": null, "proposals": ["PR-0001"]}})
    };
    let assumed: Vec<Value> = assumes_rows(&doc, "GOAL-03").into_iter().cloned().collect();
    assert_eq!(
        assumed,
        [
            holder("docs/records/GOAL-03-twin.md"),
            holder("docs/records/GOAL-03.md")
        ],
        "{}",
        doc["assumes"]
    );
    assert_eq!(doc["assumes"]["counts"]["markers"], 3, "{}", doc["assumes"]);
    assert_eq!(doc["assumes"]["counts"]["in_question"], 2);
}

/// RD-6 (Q14, S7): a non-binding marker's `reason` is the first that
/// applies of `homoglyph`, `unanchored`, `cannot_verify`, `relation`. By
/// `configures` alone, `LAW-DEFAULT`'s five: `broken.rs:4` (implements,
/// a broken item) `cannot_verify`; a look-alike on an item `homoglyph`; an
/// implements in a body `unanchored`; an `@assumes` on an item
/// `relation`; a look-alike `configures` in a body `homoglyph`; by path,
/// line, column, and the text's marker lines. M: `relation` before
/// `cannot_verify`.
#[test]
fn unbound_marker_reason_order() {
    let scratch = Scratch::new("health-reasons");
    let root = code_a_variant(
        &scratch,
        "tally",
        "\n[[health.bound]]\nkinds = [\"law\"]\nrelations = [\"configures\"]\n",
    );
    let home = scratch.home("h");
    let (_, _, doc) = document(&home, &root);
    let reasons = "crates/tally/src/reasons.rs";
    assert_eq!(
        unbound_row(&doc, "LAW-DEFAULT")["markers"],
        json!([
            marker(
                "crates/tally/src/broken.rs",
                4,
                4,
                "implements",
                "resolved",
                "cannot_verify",
                "cannot_verify"
            ),
            marker(
                reasons,
                1,
                4,
                "implements",
                "homoglyph",
                "item",
                "homoglyph"
            ),
            marker(
                reasons,
                5,
                8,
                "implements",
                "resolved",
                "unanchored",
                "unanchored"
            ),
            marker(reasons, 9, 4, "assumes", "resolved", "item", "relation"),
            marker(
                reasons,
                13,
                8,
                "configures",
                "homoglyph",
                "unanchored",
                "homoglyph"
            ),
        ]),
        "{}",
        doc["unbound"]
    );
    let run = text(&home, &root);
    let at = run
        .stdout
        .lines()
        .position(|line| line.starts_with("unbound | LAW-DEFAULT | "))
        .unwrap_or_else(|| panic!("{}", run.stdout));
    let block: Vec<&str> = run.stdout.lines().skip(at + 1).take(5).collect();
    assert_eq!(
        block,
        [
            "  implements | crates/tally/src/broken.rs:4:4 | resolved | cannot_verify | \
             cannot_verify",
            "  implements | crates/tally/src/reasons.rs:1:4 | homoglyph | item | homoglyph",
            "  implements | crates/tally/src/reasons.rs:5:8 | resolved | unanchored | unanchored",
            "  assumes | crates/tally/src/reasons.rs:9:4 | resolved | item | relation",
            "  configures | crates/tally/src/reasons.rs:13:8 | homoglyph | unanchored | \
             homoglyph",
        ],
        "{}",
        run.stdout
    );
}

/// The reasons file of [`code_a_variant`].
const REASONS_RS: &str = "// @implements L\u{0410}W-DEFAULT\npub fn look() {}\n\n\
                          pub fn body() {\n    // @implements LAW-DEFAULT\n    1;\n}\n\n\
                          // @assumes LAW-DEFAULT\npub fn assumed() {}\n\n\
                          pub fn look_body() {\n    // @configures L\u{0410}W-DEFAULT\n    2;\n}\n";

/// code-a with every kind of non-binding marker: `broken.rs:4` naming
/// `LAW-DEFAULT` (a broken item), `reasons.rs` ([`REASONS_RS`]: a
/// look-alike on an item, an implements in a body, an `@assumes` on an
/// item, a look-alike in a body), `source.rs` (a mention of
/// `LAW-SOURCE`); `tail` appended to its config.
fn code_a_variant(scratch: &Scratch, dir: &str, tail: &str) -> PathBuf {
    let root = tree(
        scratch,
        "code-a",
        dir,
        tail,
        &[
            ("crates/tally/src/reasons.rs", REASONS_RS),
            (
                "crates/tally/src/source.rs",
                "// Reads LAW-SOURCE from the data file.\npub fn source() -> u32 {\n    \
                 10\n}\n",
            ),
        ],
    );
    replace(
        &root,
        "crates/tally/src/broken.rs",
        "// @implements GOAL-01@1",
        "// @implements LAW-DEFAULT",
    );
    root
}

// ------------------------------------------------------------ AC-4, TE-2: parity

/// AC-4's relation sets: the default (no `relations` key), each alone,
/// all four.
const RELATION_SETS: [Option<&[&str]>; 6] = [
    None,
    Some(&["implements"]),
    Some(&["verifies"]),
    Some(&["configures"]),
    Some(&["assumes"]),
    Some(&["implements", "verifies", "configures", "assumes"]),
];

/// The relations a rule takes without `relations` ("`[[health.bound]]`").
const DEFAULT_RELATIONS: [&str; 3] = ["implements", "verifies", "configures"];

/// One `[[health.bound]]` of `kinds`, `relations` when given.
fn rule(kinds: &[&str], relations: Option<&[&str]>) -> String {
    let list = |words: &[&str]| {
        words
            .iter()
            .map(|word| format!("\"{word}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = format!("\n[[health.bound]]\nkinds = [{}]\n", list(kinds));
    if let Some(relations) = relations {
        out.push_str(&format!("relations = [{}]\n", list(relations)));
    }
    out
}

/// Why a marker of a node's `spec refs` block does not bind for a rule of
/// `relations` ("What binds"' parity, the order of S7); `None`: it binds.
fn reason(marker: &Value, relations: &[&str]) -> Option<&'static str> {
    let id_state = marker["id_state"].as_str().expect("id_state");
    let binding = marker["binding"]["kind"].as_str().expect("binding.kind");
    let relation = marker["relation"].as_str().expect("relation");
    if id_state == "homoglyph" {
        return Some("homoglyph");
    }
    assert!(
        ["resolved", "alias", "ambiguous"].contains(&id_state),
        "a node's block lists its own IDs and look-alikes: {marker}"
    );
    match binding {
        "unanchored" => Some("unanchored"),
        "cannot_verify" => Some("cannot_verify"),
        _ if !relations.contains(&relation) => Some("relation"),
        _ => None,
    }
}

/// The row 5 entry `spec refs` predicts for `node` under one rule of
/// `relations` (index 1); `None`: bound.
fn predicted(node: &Value, relations: &[&str]) -> Option<Value> {
    let mut markers = Vec::new();
    for entry in node["markers"].as_array().expect("markers") {
        let why = reason(entry, relations)?;
        markers.push(json!({"path": entry["path"], "line": entry["line"],
            "column": entry["column"], "relation": entry["relation"],
            "id_state": entry["id_state"], "binding": entry["binding"]["kind"],
            "reason": why}));
    }
    markers.sort_by_key(|entry| {
        (
            entry["path"].as_str().expect("path").to_owned(),
            entry["line"].as_u64().expect("line"),
            entry["column"].as_u64().expect("column"),
        )
    });
    let row = json!({"id": node["id"], "kind": node["kind"], "title": node["title"],
        "path": node["path"], "line": node["line"], "rules": [1],
        "mentions": node["mentions"].as_array().expect("mentions").len(),
        "markers": markers});
    Some(row)
}

/// Row entries by path bytes, then line.
fn sort_rows(rows: &mut [Value]) {
    rows.sort_by_key(|row| {
        (
            row["path"].as_str().expect("path").to_owned(),
            row["line"].as_u64().expect("line"),
        )
    });
}

/// AC-4 on `root`: a rule of `kinds` (every node of `references`) per
/// relation set, then all six at once: row 5 is `spec refs`' parity, row
/// for row (`rules` the indices failed), keys in "Data"'s order.
fn assert_parity(scratch: &Scratch, label: &str, root: &Path, kinds: &[&str], references: &[&str]) {
    let home = scratch.home(label);
    let nodes: Vec<Value> = references
        .iter()
        .map(|reference| refs_node(&home, root, reference))
        .collect();
    let config = read_text(root, "specengine.toml");
    let mut failing: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (index, relations) in RELATION_SETS.iter().enumerate() {
        let set: &[&str] = (*relations).unwrap_or(&DEFAULT_RELATIONS);
        write(
            root,
            "specengine.toml",
            format!("{config}{}", rule(kinds, *relations)),
        );
        let context = format!("{label}, relations {set:?}");
        let (_, ordered, doc) = document(&home, root);
        let mut want = Vec::new();
        for (at, node) in nodes.iter().enumerate() {
            if let Some(row) = predicted(node, set) {
                want.push(row);
                failing[at].push(index + 1);
            }
        }
        sort_rows(&mut want);
        let unbound = &doc["unbound"];
        assert_eq!(unbound["state"], "measured", "{context}: {unbound}");
        assert_eq!(unbound["rows"], Value::Array(want.clone()), "{context}");
        assert_eq!(
            unbound["counts"],
            json!({"rules": 1, "selected": nodes.len(), "bound": nodes.len() - want.len(),
                "unbound": want.len()}),
            "{context}"
        );
        let ordered = ordered.get("unbound");
        assert_eq!(ordered.keys(), ROW_KEYS, "{context}");
        assert_eq!(
            ordered.get("counts").keys(),
            UNBOUND_COUNT_KEYS,
            "{context}"
        );
        for row in ordered.get("rows").items() {
            assert_eq!(row.keys(), UNBOUND_ROW_KEYS, "{context}");
            for entry in row.get("markers").items() {
                assert_eq!(entry.keys(), MARKER_KEYS, "{context}");
            }
        }
    }
    // The six at once: a node is unbound when some rule finds it so.
    let all: String = RELATION_SETS
        .iter()
        .map(|relations| rule(kinds, *relations))
        .collect();
    write(root, "specengine.toml", format!("{config}{all}"));
    let (_, _, doc) = document(&home, root);
    let mut want: Vec<(String, u64, String, Vec<u64>)> = nodes
        .iter()
        .zip(&failing)
        .filter(|(_, rules)| !rules.is_empty())
        .map(|(node, rules)| {
            (
                node["path"].as_str().expect("path").to_owned(),
                node["line"].as_u64().expect("line"),
                node["id"].as_str().expect("an ID").to_owned(),
                rules.iter().map(|rule| *rule as u64).collect(),
            )
        })
        .collect();
    want.sort();
    let got: Vec<(String, u64, String, Vec<u64>)> = doc["unbound"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("{label}: {}", doc["unbound"]))
        .iter()
        .map(|row| {
            (
                row["path"].as_str().expect("path").to_owned(),
                row["line"].as_u64().expect("line"),
                row["id"].as_str().expect("an ID").to_owned(),
                row["rules"]
                    .as_array()
                    .expect("rules")
                    .iter()
                    .map(|rule| rule.as_u64().expect("an index"))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(got, want, "{label}: six rules");
    assert_eq!(
        doc["unbound"]["counts"],
        json!({"rules": 6, "selected": nodes.len(), "bound": nodes.len() - want.len(),
            "unbound": want.len()}),
        "{label}"
    );
    write(root, "specengine.toml", &config);
}

/// AC-4 (TE-2): on `fixtures/code-a` as shipped and on its variant holding
/// every non-binding marker ([`code_a_variant`]), a rule per relation set:
/// every selected node meets "What binds"' parity with `spec refs <node>
/// --json`. M: Health's own copy of the predicate admitting
/// `cannot_verify`.
#[test]
fn unbound_is_refs_parity_on_code_a() {
    let scratch = Scratch::new("health-parity-a");
    let plain = scratch.copy("code-a", "plain");
    assert_parity(&scratch, "plain", &plain, &CODE_A_KINDS, &CODE_A_REFERENCES);
    let variant = code_a_variant(&scratch, "variant", "");
    assert_parity(
        &scratch,
        "variant",
        &variant,
        &CODE_A_KINDS,
        &CODE_A_REFERENCES,
    );
}

/// AC-4 on two other projects (WA-11, genre): inkwell (sheets and marks)
/// and the farm game (ECS, `[code] frameworks = ["bevy"]`); the game's
/// rows the same without `frameworks` (Health reads no role row).
#[test]
fn unbound_is_refs_parity_on_a_second_project() {
    let scratch = Scratch::new("health-parity-genre");
    let ink = scratch.copy("code-index", "ink");
    assert_parity(&scratch, "ink", &ink, &INKWELL_KINDS, &INKWELL_REFERENCES);
    let farm = scratch.copy("code-bevy", "farm");
    assert_parity(&scratch, "farm", &farm, &FARM_KINDS, &FARM_REFERENCES);

    let tail = rule(&FARM_KINDS, None);
    let with_roles = tree(&scratch, "code-bevy", "with-roles", &tail, &[]);
    let without = tree(&scratch, "code-bevy", "without-roles", &tail, &[]);
    replace(&without, "specengine.toml", "frameworks = [\"bevy\"]\n", "");
    let (with_run, _, _) = document(&scratch.home("with-roles"), &with_roles);
    let (without_run, _, _) = document(&scratch.home("without-roles"), &without);
    assert_eq!(with_run.stdout, without_run.stdout, "roles change no row");
}

/// TE-2, `spec unmapped`'s half: on code-a's variant, every marker row 5
/// would count as binding (any relation) sits on a mapped unit (its item
/// not unmapped, its RON file mapped); the `cannot_verify` one leaves the
/// broken item unmapped; in `reasons.rs` the look-alikes' and the
/// body's items stay unmapped, the `@assumes`' item is mapped.
#[test]
fn bound_markers_sit_on_mapped_units() {
    let scratch = Scratch::new("health-te2");
    let root = code_a_variant(&scratch, "variant", "");
    let home = scratch.home("h");
    let all = ["implements", "verifies", "configures", "assumes"];
    let mut files: BTreeMap<String, Value> = BTreeMap::new();
    let mut checked = 0;
    for reference in CODE_A_REFERENCES {
        let node = refs_node(&home, &root, reference);
        for entry in node["markers"].as_array().expect("markers") {
            if reason(entry, &all).is_some() {
                continue;
            }
            let path = entry["path"].as_str().expect("path").to_owned();
            let unmapped = files
                .entry(path.clone())
                .or_insert_with(|| unmapped_file(&home, &root, &path));
            match entry["binding"]["kind"].as_str().expect("binding") {
                "item" => {
                    let qpath = entry["binding"]["qpath"].as_str().expect("qpath");
                    assert!(
                        !unmapped_qpaths(unmapped).contains(&qpath.to_owned()),
                        "{reference}: {entry} sits on an unmapped item: {unmapped}"
                    );
                    checked += 1;
                }
                "ron" => {
                    assert_eq!(
                        unmapped["rows"][0]["mapped"], unmapped["rows"][0]["units"],
                        "{reference}: {entry}: {unmapped}"
                    );
                    checked += 1;
                }
                _ => {}
            }
        }
    }
    assert!(checked >= 10, "{checked} markers checked");
    let broken = unmapped_file(&home, &root, "crates/tally/src/broken.rs");
    assert!(
        unmapped_qpaths(&broken).contains(&"crates/tally::broken::broken".to_owned()),
        "{broken}"
    );
    let reasons = unmapped_file(&home, &root, "crates/tally/src/reasons.rs");
    let lines: Vec<u64> = reasons["unmapped"]
        .as_array()
        .expect("unmapped")
        .iter()
        .map(|item| item["line"].as_u64().expect("line"))
        .collect();
    assert_eq!(lines, [2, 4, 12], "look, body, look_body: {reasons}");
}

// ------------------------------------------------------------ RD-8, RD-9, AC-1, AC-2, RD-11

/// What RD-8 compares: git's status and `HEAD`, `spec.lock`, the queue's
/// dump and last event, every file outside the data directory.
#[derive(Debug, PartialEq)]
struct Probe {
    status: Vec<u8>,
    head: Option<String>,
    lock: Vec<u8>,
    dump: String,
    last_seq: Option<i64>,
    outside: BTreeMap<String, Option<Vec<u8>>>,
}

fn probe(t: &Tally) -> Probe {
    let status = t.git.git(
        &t.root,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ],
    );
    let head = t.git.head(&t.root);
    let queue = SqliteQueue::open_existing(t.db(), "code-a")
        .expect("open_existing")
        .expect("the seeded queue");
    let dump = queue.dump().expect("dump");
    let last_seq = queue
        .events()
        .expect("events")
        .last()
        .map(|event| event.seq);
    drop(queue);
    let data = data_dir(&t.home);
    let inside = data
        .strip_prefix(t.scratch.path())
        .expect("in the scratch")
        .to_string_lossy()
        .into_owned();
    let outside = snapshot(t.scratch.path())
        .into_iter()
        .filter(|(key, _)| !(key == &inside || key.starts_with(&format!("{inside}/"))))
        .collect();
    Probe {
        status,
        head,
        lock: read(&t.root, "spec.lock"),
        dump,
        last_seq,
        outside,
    }
}

/// RD-8 (TE-4): over TE-1's tree, two JSON and two text calls change
/// nothing but the data directory's two indexes: `git status
/// --porcelain=v1 -z --untracked-files=all` (empty), `HEAD`, `spec.lock`,
/// the queue's dump, the last event's `seq`, every file outside the data
/// directory; the data directory holds the slug's DB files alone. M: an
/// event written.
///
/// docs/features/health-drift.md RD-4: then the example tree's edit
/// (`CAP: u32 = 10;` -> `12;` in `counter.rs`, uncommitted: row 6
/// measured with a `code_ahead` row over the golden lock): the same probe,
/// and `.git/index`'s mtime and the count of `.git/objects` files too.
/// M: Health writes a git object, the index or the lock.
#[test]
fn health_writes_no_lock_queue_row_or_event() {
    let t = Tally::new("health-rd8", LAW_RULE);
    let before = probe(&t);
    assert!(before.status.is_empty(), "a clean tree to begin with");
    assert!(before.last_seq.is_some(), "the seeded events");
    for _ in 0..2 {
        document(&t.home, &t.root);
        text(&t.home, &t.root);
    }
    let after = probe(&t);
    assert_eq!(after.status, before.status, "git status");
    assert_eq!(after.head, before.head, "HEAD");
    assert_eq!(after.lock, before.lock, "spec.lock");
    assert_eq!(after.dump, before.dump, "the queue's rows");
    assert_eq!(after.last_seq, before.last_seq, "no event");
    assert!(
        after.outside == before.outside,
        "nothing written outside the data directory"
    );
    let names: Vec<String> = fs::read_dir(data_dir(&t.home))
        .expect("the data directory")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        names
            .iter()
            .all(|name| ["code-a.db", "code-a.db-wal", "code-a.db-shm"].contains(&name.as_str())),
        "{names:?}"
    );

    replace(
        &t.root,
        "crates/tally/src/counter.rs",
        "CAP: u32 = 10;",
        "CAP: u32 = 12;",
    );
    let git_index = t.root.join(".git/index");
    let mtime = || {
        fs::metadata(&git_index)
            .and_then(|meta| meta.modified())
            .expect("the index's mtime")
    };
    let objects = || -> usize {
        snapshot(&t.root.join(".git/objects"))
            .values()
            .filter(|bytes| bytes.is_some())
            .count()
    };
    let before = probe(&t);
    let (index_before, objects_before) = (mtime(), objects());
    assert!(objects_before > 0, "premise: objects to count");
    let doc = t.document();
    assert_eq!(doc["drift"]["state"], "measured", "{}", doc["drift"]);
    assert!(
        doc["drift"]["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .any(|row| row["state"] == "code_ahead" && row["lock"].is_object()),
        "premise: a locked binding drifts: {}",
        doc["drift"]
    );
    for _ in 0..2 {
        document(&t.home, &t.root);
        text(&t.home, &t.root);
    }
    let after = probe(&t);
    assert_eq!(after.status, before.status, "git status");
    assert_eq!(after.head, before.head, "HEAD");
    assert_eq!(after.lock, before.lock, "spec.lock");
    assert_eq!(after.dump, before.dump, "the queue's rows");
    assert_eq!(after.last_seq, before.last_seq, "no event");
    assert!(
        after.outside == before.outside,
        "nothing written outside the data directory"
    );
    assert_eq!(mtime(), index_before, ".git/index untouched");
    assert_eq!(objects(), objects_before, "no git object written");
}

/// `root`'s files but `.git`, written under `to` in reverse path order.
fn copy_reversed(from: &Path, to: &Path) {
    let files: Vec<(String, Vec<u8>)> = snapshot(from)
        .into_iter()
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .filter_map(|(path, bytes)| bytes.map(|bytes| (path, bytes)))
        .collect();
    for (path, bytes) in files.iter().rev() {
        write(to, path, bytes);
    }
}

/// `from`'s `.git` copied under `to` byte for byte (docs/features/health-drift.md
/// RD-8: a copy is the same repository at another path, so row 6 is judged
/// there as here).
fn copy_git(from: &Path, to: &Path) {
    crate::common::copy_dir(&from.join(".git"), &to.join(".git"));
}

/// Every `.rs`, `.ron` and `.md` file under `root` with CRLF line ends.
fn to_crlf(root: &Path) {
    for (path, bytes) in snapshot(root) {
        let Some(bytes) = bytes else {
            continue;
        };
        if [".rs", ".ron", ".md"].iter().any(|ext| path.ends_with(ext)) {
            let text = String::from_utf8(bytes).expect("UTF-8");
            write(
                root,
                &path,
                text.replace("\r\n", "\n").replace('\n', "\r\n"),
            );
        }
    }
}

/// A file added after a first refresh: its stored rows come last, its path
/// sorts first.
const AAA_RS: &str =
    "pub fn aaa() -> u32 {\n    // @assumes PR-0001\n    // @implements LAW-SOURCE\n    1\n}\n";

/// RD-9 (TE-4): over TE-1's tree with a law rule, stdout and stderr of
/// both modes are byte-identical from another cwd (`--root`), from a
/// second root sharing the data directory written in reverse order, from
/// a CRLF copy; a file added after the first refresh (its rows stored
/// last, its path first) gives the bytes a fresh data directory gives, and
/// the rows by path and line. M: rows in stored order, not path and line.
///
/// docs/features/health-drift.md RD-8: the same with row 6 measured (the
/// golden lock; the copies carry `.git`, the CRLF one's files differing
/// from its index); with no `git` on `PATH` row 6 is `cannot`, its one
/// cause `` `git` could not be run: no `git` program was found on PATH ``,
/// rows 3-5's bytes (the document's first six keys) kept. M: row 6 read
/// from the cwd, or rows 3-5 reading row 6.
#[test]
fn health_code_rows_are_byte_identical_across_roots_and_order() {
    let t = Tally::new("health-rd9", LAW_RULE);
    let first = both(&t.home, &t.root, None);
    let measured: Value = serde_json::from_str(first[0].trim_end()).expect("JSON");
    assert_eq!(
        measured["drift"]["state"], "measured",
        "{}",
        measured["drift"]
    );
    let empty = t.scratch.join("no-git-bin");
    fs::create_dir_all(&empty).expect("an empty PATH directory");
    let no_git = health_with(
        &[("HOME", t.home.as_os_str()), ("PATH", empty.as_os_str())],
        &t.root,
        true,
    );
    no_git.code(0);
    let cannot: Value = serde_json::from_str(no_git.stdout.trim_end()).expect("JSON");
    assert_eq!(
        cannot["drift"],
        json!({"state": "cannot",
            "causes": ["`git` could not be run: no `git` program was found on PATH"],
            "counts": null, "rows": null, "tested": null, "notes": []}),
    );
    let (_, with_git) = health_json(&t.home, &t.root);
    let without_git = J::parse(no_git.stdout.trim_end());
    for key in &DOCUMENT_KEYS[..6] {
        assert_eq!(
            with_git.get(key),
            without_git.get(key),
            "rows 3-5's `{key}`"
        );
    }
    assert_eq!(both(&t.home, &t.root, None), first, "a repeat");
    assert_eq!(
        both(&t.home, t.scratch.path(), Some(&t.root)),
        first,
        "another cwd"
    );
    let reversed = t.scratch.join("elsewhere/reversed");
    copy_reversed(&t.root, &reversed);
    copy_git(&t.root, &reversed);
    assert_eq!(
        both(&t.home, &reversed, None),
        first,
        "a second root, one data directory"
    );
    let crlf = t.scratch.join("other/place/crlf");
    copy_reversed(&t.root, &crlf);
    to_crlf(&crlf);
    copy_git(&t.root, &crlf);
    assert_eq!(both(&t.home, &crlf, None), first, "a CRLF copy");

    let late = t.scratch.join("late");
    copy_reversed(&t.root, &late);
    let incremental = t.scratch.home("incremental");
    seed_queue(&incremental, "code-a");
    both(&incremental, &late, None);
    write(&late, "crates/tally/src/aaa.rs", AAA_RS);
    let grown = both(&incremental, &late, None);
    let fresh = t.scratch.home("fresh");
    seed_queue(&fresh, "code-a");
    assert_eq!(both(&fresh, &late, None), grown, "stored order is no order");
    let doc: Value = serde_json::from_str(grown[0].trim_end()).expect("JSON");
    // Row 4 sorts by the ID first: the markers' stored order (path, then
    // position: `aaa.rs` first) is not its order (the review's RD-9 note:
    // row 5's stored order is path order already).
    let keys: Vec<(String, String, u64, u64)> = doc["assumes"]["rows"]
        .as_array()
        .expect("row 4's rows")
        .iter()
        .map(|row| {
            (
                row["id"].as_str().expect("an ID").to_owned(),
                row["path"].as_str().expect("path").to_owned(),
                row["line"].as_u64().expect("line"),
                row["column"].as_u64().expect("column"),
            )
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "row 4 by ID, path, line, column");
    assert_eq!(keys[0].0, "GOAL-01", "{keys:?}");
    let paths: Vec<&str> = assumes_rows(&doc, "PR-0001")
        .iter()
        .map(|row| row["path"].as_str().expect("path"))
        .collect();
    assert_eq!(
        paths,
        [
            "crates/tally/src/aaa.rs",
            "crates/tally/src/assumed.rs",
            "crates/tally/src/lib.rs"
        ]
    );
    let source: Vec<&str> = unbound_row(&doc, "LAW-SOURCE")["markers"]
        .as_array()
        .expect("markers")
        .iter()
        .map(|entry| entry["path"].as_str().expect("path"))
        .collect();
    assert_eq!(source, ["crates/tally/src/aaa.rs", LIB]);
}

/// AC-1 (ADR-0051): the call refreshes the code index as `spec refs` does.
/// After `spec index`, an edit binding `LAW-SOURCE` in `counter.rs`: the
/// next Health shows it bound, and `spec index --json` then re-derives
/// nothing (`code.parsed` 0); a repeated call re-derives nothing either
/// (TE-4). M: Health reads `code_refs()` without `refresh_code`.
#[test]
fn health_refreshes_the_code_index_as_refs_does() {
    let scratch = Scratch::new("health-ac1");
    let root = tree(&scratch, "code-a", "tally", LAW_RULE, &[]);
    let home = scratch.home("h");
    index(&home, &root);
    let (_, _, before) = document(&home, &root);
    assert!(row_ids(&before["unbound"]).contains(&"LAW-SOURCE".to_owned()));
    append(
        &root,
        "crates/tally/src/counter.rs",
        "\n// @configures LAW-SOURCE\npub const SOURCE: &str = \"data/limits.ron\";\n",
    );
    let (_, _, after) = document(&home, &root);
    assert_eq!(
        row_ids(&after["unbound"]),
        ["LAW-DEFAULT"],
        "{}",
        after["unbound"]
    );
    let parsed = |home: &Path| -> Value {
        let run = spec(home, &root, &["--json", "index"]);
        run.code(0);
        serde_json::from_str::<Value>(run.stdout.trim_end()).expect("JSON")["code"]["parsed"]
            .clone()
    };
    assert_eq!(parsed(&home), 0, "Health refreshed the edited file");
    let (_, _, again) = document(&home, &root);
    assert_eq!(again, after, "a repeat");
    assert_eq!(parsed(&home), 0, "a repeat re-derives nothing");
}

/// The code of `text`: up to a `#[cfg(test)]` line, each line cut at `//`.
fn code_of(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        out.push_str(line.split("//").next().unwrap_or_default());
        out.push('\n');
    }
    out
}

/// `code` calls `name` (`name(` not inside a longer identifier).
fn calls(code: &str, name: &str) -> bool {
    let needle = format!("{name}(");
    code.match_indices(&needle).any(|(at, _)| {
        code[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !(before.is_alphanumeric() || before == '_'))
    })
}

/// AC-2 (a source scan): rows 4-5's CLI sources (every source whose path
/// holds `health`, [`PHASE_B_SOURCES`]) call `refresh_code`,
/// `require_complete`, `code_refs`, `maps`, never `read_tree`,
/// `collect_rust`, `collect_ron`, `code_symbols` (nor a role or symbol
/// snapshot: "no symbol read", "never the role rows"); Q13: `maps` is
/// defined in `symbols.rs` and called by `unmapped.rs`. M: the walk
/// revived. docs/features/health-drift.md RD-2 (M3): row 6's sources
/// (`health_drift.rs` holds `health`) never call verify's own walk,
/// `verify_as`, nor `read_tree`.
#[test]
fn health_reads_the_code_index_not_the_tree() {
    let mut sources: Vec<String> = rust_files(&repository_root().join("crates/specengine-cli/src"))
        .into_iter()
        .filter(|path| path.contains("health"))
        .collect();
    sources.extend(
        PHASE_B_SOURCES
            .iter()
            .filter(|path| path.starts_with("crates/specengine-cli/"))
            .map(|path| (*path).to_owned()),
    );
    assert!(
        sources.contains(&"crates/specengine-cli/src/health.rs".to_owned()),
        "{sources:?}"
    );
    let code: String = sources
        .iter()
        .map(|path| code_of(&read_text(&repository_root(), path)))
        .collect();
    for name in ["refresh_code", "require_complete", "code_refs", "maps"] {
        assert!(calls(&code, name), "{sources:?} call `{name}`");
    }
    for name in [
        "read_tree",
        "verify_as",
        "collect_rust",
        "collect_ron",
        "code_symbols",
        "code_roles",
        "code_snapshot",
        "code_snapshot_with_roles",
    ] {
        assert!(!calls(&code, name), "{sources:?} never call `{name}`");
    }
    let root = repository_root();
    let symbols = code_of(&read_text(&root, "crates/specengine-cli/src/symbols.rs"));
    // `fn maps<'s>(…)`: the spec's signature needs its lifetime named
    // (the developer's deviation 3, E0106).
    assert!(
        symbols.contains("fn maps(") || symbols.contains("fn maps<"),
        "`symbols::maps` (Q13)"
    );
    let unmapped = code_of(&read_text(&root, "crates/specengine-cli/src/unmapped.rs"));
    assert!(calls(&unmapped, "maps"), "`unmapped.rs` calls `maps` (Q13)");
    // The scan sees a call and skips a comment and a longer name.
    assert!(calls("    let x = maps(a, b)?;\n", "maps"));
    assert!(!calls(&code_of("    // read_tree(root)\n"), "read_tree"));
    assert!(!calls("    bitmaps(a)\n", "maps"));
}

/// `spec check` and `spec check --json` in `root`: (exit, stdout, stderr)
/// each.
fn check_runs(home: &Path, root: &Path) -> Vec<(i32, String, String)> {
    [&["check"][..], &["check", "--json"]]
        .iter()
        .map(|args| {
            let run = spec(home, root, args);
            (run.code, run.stdout, run.stderr)
        })
        .collect()
}

/// RD-11: `spec check` judges no `[health]`: its output, both modes, is
/// byte-identical with a valid rule, a malformed one, an unknown key and
/// an empty table, on code-a and on spec-a. M: `spec check` reads
/// `[health]`.
#[test]
fn check_output_equal_with_binding_rules() {
    let scratch = Scratch::new("health-rd11");
    let home = scratch.home("h");
    for fixture in ["code-a", "spec-a"] {
        let root = scratch.copy(fixture, fixture);
        let config = read_text(&root, "specengine.toml");
        let base = check_runs(&home, &root);
        for tail in [
            LAW_RULE,
            "\n[[health.bound]]\nkinds = 3\n",
            "\n[[health.bound]]\nkinds = [\"law\"]\nflavour = 1\n",
            "\n[health]\n",
        ] {
            write(&root, "specengine.toml", format!("{config}{tail}"));
            assert_eq!(check_runs(&home, &root), base, "{fixture}: {tail:?}");
        }
    }
}
