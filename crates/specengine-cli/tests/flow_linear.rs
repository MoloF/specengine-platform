//! docs/features/flow-linear.md (RM-0076), the CLI half: the golden.
//!
//! `flow_linear_before.json` holds what `spec flow`, `spec flow --trace`,
//! `spec trace` and `spec bundle` printed with the `spec` binary of this
//! slice's parent commit (`e863186`), before any `src/` change of the slice
//! (L6). Each row is one command, run once as given and once with
//! `--json`, both in one fresh scratch `HOME` over one fresh scratch copy of
//! its fixture, from an empty working directory, `--root <copy>` last. The
//! replay below runs every row the same way with the binary under test.
//!
//! A `flow` or `trace` row keeps its bytes (`code`, `text`, `json`), so a
//! failure shows its first differing line. A `bundle` row keeps BLAKE3
//! digests (`b3`, as `specengine_store::b3_hash` writes them) of its text
//! (the plain run's stdout), its JSON (the `--json` run's stdout), its
//! stderr (the plain run's, a NUL, the `--json` run's) and its exit codes
//! (`"<plain> <json>"`, e.g. `"0 0"`): the history keeps 89 rows of bytes,
//! not 476 bundles. A failure names the row, its arguments and each digest
//! that moved.
//!
//! - AC-10 "The golden": every `flow` and `trace` row byte-equal, its exit
//!   code included: the five flow fixtures, the whole read and at least two
//!   ROOTs each at `--depth 0`, `1` and unbounded, `2` where the tree has
//!   three levels, a ROOT naming nothing (exit 1); `flow --trace` and `spec
//!   trace` on `trace-almanac`, on `flow-gaps` too.
//! - AC-11: every `bundle` row equal by digest, its text, JSON (and so
//!   `bundle_hash`), stderr and exit codes: `spec-a` and `spec-b`, each
//!   target set `bundle_fit.rs` runs over an unchanged fixture, at the
//!   minimum − 1 (exit 2), the minimum, the minimum + 1, the default, the
//!   unbounded bundle's `tokens` and `u32::MAX`, and at least 20 budgets
//!   between the minimum and those `tokens`, each boundary where an item
//!   just fits taken with the budget one below it. M: round per part (a
//!   part that just fits is refused at its boundary).
//!
//! The golden is never recorded again but as below: a second recording with
//! the binary after the slice would prove nothing on its own. The cases it
//! must hold are the constants below; `the_golden_holds_every_case_of_the_spec`
//! checks them on the file alone, and the bundle replays check what needs
//! the bytes (the `tokens`, the boundaries) once the digests have proved
//! them the bytes before.
//!
//! docs/features/flow-scope.md (RM-0083) FS-8: the golden is re-recorded at
//! that slice's head (`fs8_record_the_golden`, `#[ignore]`, by hand), and
//! the re-record is judged against the rows `e863186` printed, git's blob
//! [`BEFORE_FLOW_SCOPE`] (the file at `baac762`):
//! `fs8_the_re_record_moves_only_depends_on`. A `flow` row that read no
//! `depends_on` keeps its bytes; one that did keeps its gap rows byte for
//! byte, each kept node's `distance`, `start`, `end` and `stage` (and every
//! other key but `inner`), every `follows`, `uses` and `supplies` edge of an
//! uncut row, and loses only `depends_on` edges and `inner` entries, or a
//! node only such edges admitted; every `trace` and `bundle` row is
//! byte-equal. M13: a re-record that also drops a `follows` edge.
//!
//! docs/features/flow-scenario.md (RM-0094) FS-5 and its test churn: a
//! `flow` row's JSON is compared after removing exactly `in_flow`,
//! `not_in_flow` and `types` ([`common::keys::without_keys`], every other
//! byte kept; no row here is a `show`, the fourth key's command); its text
//! with nothing removed. The golden and its pinned blob do not
//! change. M: a ` | in flow` field on a text node line.
//!
//! docs/features/doc-modal.md (RM-0095) DM-3 and its test churn: a gap
//! row's `titles` is taken out too
//! ([`common::keys::without_doc_modal_keys`], only from a gap row; a node's
//! `title` stays); the text with nothing removed.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::graph::spec30;
use common::keys::{FLOW_KEYS, without_doc_modal_keys, without_keys};
use common::{Run, Scratch, one_json_document, repository_root};
use serde::Serialize as _;
use serde_json::Value;
use specengine_store::b3_hash;

/// The golden's file name, under `tests/`.
const GOLDEN: &str = "flow_linear_before.json";

/// The budget that leaves nothing out.
const UNBOUNDED: &str = "4294967295";

/// A fixture, the ROOTs read under it and the depths of every read.
type FlowCase = (
    &'static str,
    &'static [&'static str],
    &'static [Option<&'static str>],
);

/// The flow fixtures, the ROOTs read under each (a document with children,
/// one without or an ID section, whose read is its document's) and the
/// depths every read is taken at: unbounded, then `--depth N`; `2` where
/// the `parent:` tree has three levels.
const FLOW_CASES: [FlowCase; 5] = [
    ("flow-relay", &["LEG-START", "LEG-START-LINE"], &SHALLOW),
    ("flow-hire", &["JRN-SCAN", "JRN-SCAN-BAD"], &SHALLOW),
    (
        "flow-gaps",
        &["WING-LIBRARY", "WING-LEND", "WING-BROKEN"],
        &DEEP,
    ),
    (
        "trace-almanac",
        &["BOOK-ALMANAC", "PART-GATHER", "STEP-SET-PROOF"],
        &DEEP,
    ),
    ("spec-c", &["AREA-HIRE", "FEE-RIDE"], &SHALLOW),
];

/// The depths of a fixture whose `parent:` tree has at most two levels.
const SHALLOW: [Option<&str>; 3] = [None, Some("0"), Some("1")];

/// The depths of a fixture whose `parent:` tree has three.
const DEEP: [Option<&str>; 4] = [None, Some("0"), Some("1"), Some("2")];

/// A ROOT naming nothing: exit 1.
const NOTHING: &str = "NOPE-404";

/// `spec flow --trace`: the fixtures, their ROOTs and depths.
const FLOW_TRACE_CASES: [FlowCase; 2] = [
    ("trace-almanac", &["BOOK-ALMANAC", "PART-GATHER"], &SHALLOW),
    ("flow-gaps", &[], &[None, Some("1")]),
];

/// `spec trace`: the fixtures and the argument lists after `trace`.
const TRACE_CASES: [(&str, &[&[&str]]); 2] = [
    (
        "trace-almanac",
        &[
            &[],
            &["BOOK-ALMANAC"],
            &["PART-GATHER"],
            &["PART-PRINT"],
            &["STEP-CALL"],
            &["STEP-SET-PROOF"],
            &["--summary"],
            &["--stage", "gather"],
            &["--stage", "print"],
            &["--stage", "-"],
            &[NOTHING],
        ],
    ),
    ("flow-gaps", &[&[], &["WING-LIBRARY"], &["--summary"]]),
];

/// Every target set `bundle_fit.rs` bundles over an unchanged `spec-a` or
/// `spec-b` (`ac06_`, `ac11_`, `the_minimum_is_the_frame`).
const BUNDLE_SETS: [(&str, &[&str]); 19] = [
    ("spec-a", &["MEC-STAMINA"]),
    ("spec-a", &["MEC-SPRINT"]),
    ("spec-a", &["R-12"]),
    ("spec-a", &["DOM-GAME"]),
    ("spec-a", &["DOM-MOVEMENT"]),
    ("spec-a", &["DEC-0023"]),
    ("spec-a", &["EDGE-SPRINT-EMPTY"]),
    ("spec-a", &["RULE-SPRINT-COST"]),
    ("spec-a", &["docs/features/stamina-tuning.md"]),
    ("spec-a", &["A-102"]),
    ("spec-a", &["MEC-SPRINT", "R-12"]),
    ("spec-a", &["MEC-SPRINT", "MEC-STAMINA"]),
    ("spec-b", &["REQ-001"]),
    ("spec-b", &["MOD-CLI"]),
    ("spec-b", &["CMD-SYNC"]),
    ("spec-b", &["REQ-002"]),
    ("spec-b", &["dry-run/CRIT-01"]),
    ("spec-b", &["ADR-0001"]),
    ("spec-b", &["REQ-001", "REQ-002"]),
];

/// Budgets strictly between the minimum and the unbounded `tokens`, at
/// least (fewer only when fewer exist).
const INSIDE: usize = 20;

// ------------------------------------------------------------------ helpers

/// One row of the golden: the bytes of a `flow` or `trace` row, or the
/// digests of a `bundle` row.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
struct Row {
    fixture: String,
    args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    b3: Option<Digests>,
}

/// A `bundle` row's digests (see the module documentation).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq)]
struct Digests {
    code: String,
    text: String,
    json: String,
    stderr: String,
}

impl Digests {
    /// The digests of a row's two runs.
    fn of(text: &Run, json: &Run) -> Self {
        let mut stderr = text.stderr.clone().into_bytes();
        stderr.push(0);
        stderr.extend_from_slice(json.stderr.as_bytes());
        Self {
            code: b3_hash(codes(text.code, json.code).as_bytes()),
            text: b3_hash(text.stdout.as_bytes()),
            json: b3_hash(json.stdout.as_bytes()),
            stderr: b3_hash(&stderr),
        }
    }

    /// The exit codes a `code` digest stands for, among those a bundle
    /// gives (0, 1, 2 on both runs).
    fn codes(&self) -> Option<i32> {
        (0..=2).find(|&code| b3_hash(codes(code, code).as_bytes()) == self.code)
    }
}

/// The two exit codes as the `code` digest hashes them.
fn codes(plain: i32, json: i32) -> String {
    format!("{plain} {json}")
}

fn golden_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(GOLDEN)
}

fn golden() -> Vec<Row> {
    let path = golden_path();
    serde_json::from_slice(&fs::read(&path).unwrap_or_else(|error| panic!("{GOLDEN}: {error}")))
        .unwrap_or_else(|error| panic!("{GOLDEN} is no list of rows: {error}"))
}

/// A row's two runs, `spec <args> --root <copy>` then `spec <args> --json
/// --root <copy>`, in one fresh scratch copy of `fixture` with one fresh
/// `HOME`, from an empty working directory.
fn replay(fixture: &str, args: &[String]) -> (Run, Run) {
    let scratch = Scratch::new("flow-linear");
    let root = scratch.copy(fixture, "copy");
    let home = scratch.home("h");
    let cwd = scratch.dir("cwd");
    let root_text = root.to_str().expect("UTF-8 root").to_owned();
    let plain: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut as_json = plain.clone();
    as_json.push("--json");
    let mut first = plain.clone();
    first.extend(["--root", root_text.as_str()]);
    as_json.extend(["--root", root_text.as_str()]);
    let text = spec30(&home, &cwd, &first);
    let json = spec30(&home, &cwd, &as_json);
    (text, json)
}

/// The first line where `got` and `want` part, for a readable failure.
fn first_difference(got: &str, want: &str) -> String {
    for (number, (left, right)) in got.lines().zip(want.lines()).enumerate() {
        if left != right {
            return format!(
                "line {}:\n  now    {left:?}\n  before {right:?}",
                number + 1
            );
        }
    }
    format!(
        "{} lines now, {} before (one is the other's prefix)",
        got.lines().count(),
        want.lines().count()
    )
}

/// What one row's replay differs in from the golden; empty when equal.
fn differences(row: &Row, text: &Run, json: &Run, first: &mut Option<String>) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(want) = &row.b3 {
        let got = Digests::of(text, json);
        if got.code != want.code {
            let before = want
                .codes()
                .map_or_else(|| "another".to_owned(), |code| codes(code, code));
            parts.push(format!(
                "exit {} now, {before} before",
                codes(text.code, json.code)
            ));
        }
        for (name, now, before) in [
            ("the text", &got.text, &want.text),
            ("the JSON", &got.json, &want.json),
            ("stderr", &got.stderr, &want.stderr),
        ] {
            if now != before {
                parts.push(format!("{name} ({now} now, {before} before)"));
            }
        }
        return parts;
    }
    let code = row.code.expect("a row of bytes has its code");
    // flow-scenario's keys out of a flow row's JSON, and doc-modal's gap
    // `titles` (DM-3), every other byte kept.
    let json_before = if command(row) == "flow" && json.stdout.starts_with('{') {
        without_doc_modal_keys(&without_keys(&json.stdout, &FLOW_KEYS))
    } else {
        json.stdout.clone()
    };
    if text.code != code || json.code != code {
        parts.push(format!(
            "exit {} and {} (--json), {code} before",
            text.code, json.code
        ));
    }
    for (name, now, before) in [
        (
            "the text",
            &text.stdout,
            row.text.as_deref().unwrap_or_default(),
        ),
        (
            "the JSON",
            &json_before,
            row.json.as_deref().unwrap_or_default(),
        ),
    ] {
        if now != before {
            parts.push(name.to_owned());
            first.get_or_insert_with(|| {
                format!(
                    "{} {:?}, {name}: {}",
                    row.fixture,
                    row.args,
                    first_difference(now, before)
                )
            });
        }
    }
    parts
}

/// Replays every row `keep` selects and fails naming each row that is not
/// the golden's, with the first difference of the first row of bytes;
/// returns each row with its two runs.
fn replay_rows(keep: impl Fn(&Row) -> bool, least: usize) -> Vec<(Row, Run, Run)> {
    let rows: Vec<Row> = golden().into_iter().filter(|row| keep(row)).collect();
    assert!(
        rows.len() >= least,
        "only {} rows selected, at least {least} expected",
        rows.len()
    );
    let mut failed: Vec<String> = Vec::new();
    let mut first: Option<String> = None;
    let mut replayed = Vec::new();
    for row in rows {
        let (text, json) = replay(&row.fixture, &row.args);
        let parts = differences(&row, &text, &json, &mut first);
        if !parts.is_empty() {
            failed.push(format!(
                "{} {:?}: {}",
                row.fixture,
                row.args,
                parts.join(", ")
            ));
        }
        replayed.push((row, text, json));
    }
    assert!(
        failed.is_empty(),
        "{} of {} rows differ from the binary before:\n{}\nfirst: {}",
        failed.len(),
        replayed.len(),
        failed.join("\n"),
        first.unwrap_or_default()
    );
    replayed
}

fn command(row: &Row) -> &str {
    row.args.first().map(String::as_str).unwrap_or_default()
}

fn has(row: &Row, flag: &str) -> bool {
    row.args.iter().any(|arg| arg == flag)
}

/// The value after `flag` in a row's args.
fn value_of<'r>(row: &'r Row, flag: &str) -> Option<&'r str> {
    row.args
        .iter()
        .position(|arg| arg == flag)
        .and_then(|at| row.args.get(at + 1))
        .map(String::as_str)
}

/// The ROOT of a `flow` row: its one positional argument after `flow`.
fn flow_root(row: &Row) -> Option<&str> {
    let mut args = row.args.iter().skip(1);
    let mut root = None;
    while let Some(arg) = args.next() {
        if arg == "--depth" {
            args.next();
        } else if !arg.starts_with("--") {
            root = Some(arg.as_str());
        }
    }
    root
}

/// The targets of a `bundle` row: its positional arguments.
fn bundle_targets(row: &Row) -> Vec<&str> {
    let mut args = row.args.iter().skip(1);
    let mut targets = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--budget" {
            args.next();
        } else if !arg.starts_with("--") {
            targets.push(arg.as_str());
        }
    }
    targets
}

fn is_plain_flow(row: &Row) -> bool {
    command(row) == "flow" && !has(row, "--trace")
}

fn is_trace(row: &Row) -> bool {
    command(row) == "trace" || (command(row) == "flow" && has(row, "--trace"))
}

// ------------------------------------------------------------- the golden

/// The golden holds every case the spec's "Data" names, and each row is a
/// whole answer: one JSON document on exit 0 and 1, nothing on exit 2. M:
/// a case dropped from the recording.
#[test]
fn the_golden_holds_every_case_of_the_spec() {
    let rows = golden();
    let mut seen = BTreeSet::new();
    for row in &rows {
        assert!(
            seen.insert((row.fixture.clone(), row.args.clone())),
            "a row twice: {} {:?}",
            row.fixture,
            row.args
        );
        assert!(
            !row.args
                .iter()
                .any(|arg| arg == "--json" || arg == "--root"),
            "{:?}: --json and --root are the replay's",
            row.args
        );
        if command(row) == "bundle" {
            let digests = row.b3.as_ref().unwrap_or_else(|| {
                panic!("{} {:?}: a bundle row keeps digests", row.fixture, row.args)
            });
            assert!(
                row.code.is_none() && row.text.is_none() && row.json.is_none(),
                "{} {:?}: a bundle row keeps no bytes",
                row.fixture,
                row.args
            );
            for digest in [&digests.code, &digests.text, &digests.json, &digests.stderr] {
                assert!(
                    digest.len() == 67
                        && digest.starts_with("b3:")
                        && digest[3..].bytes().all(|b| b.is_ascii_hexdigit()),
                    "{} {:?}: {digest}",
                    row.fixture,
                    row.args
                );
            }
            continue;
        }
        assert!(row.b3.is_none(), "{} {:?}", row.fixture, row.args);
        let (Some(code), Some(text), Some(json)) = (row.code, &row.text, &row.json) else {
            panic!("{} {:?}: a row of bytes", row.fixture, row.args);
        };
        match code {
            0 | 1 => {
                let document = one_json_document(json)
                    .unwrap_or_else(|problem| panic!("{} {:?}: {problem}", row.fixture, row.args));
                assert!(document.is_object(), "{:?}", row.args);
                if code == 0 {
                    assert!(!text.is_empty(), "{} {:?}", row.fixture, row.args);
                }
            }
            code => panic!("{} {:?}: exit {code}", row.fixture, row.args),
        }
    }
    assert_eq!(
        rows.iter().filter(|row| row.b3.is_none()).count(),
        89,
        "the rows of bytes"
    );

    // `spec flow` on every flow fixture: the whole flow and each ROOT, at
    // every depth, exit 0; a ROOT naming nothing, exit 1.
    for (fixture, roots, depths) in FLOW_CASES {
        assert!(roots.len() >= 2, "{fixture}");
        for depth in [None, Some("0"), Some("1")] {
            assert!(depths.contains(&depth), "{fixture}: {depth:?}");
        }
        for root in std::iter::once(None).chain(roots.iter().copied().map(Some)) {
            for &depth in depths {
                let found: Vec<&Row> = rows
                    .iter()
                    .filter(|row| {
                        row.fixture == fixture
                            && is_plain_flow(row)
                            && flow_root(row) == root
                            && value_of(row, "--depth") == depth
                    })
                    .collect();
                assert_eq!(found.len(), 1, "{fixture} flow {root:?} --depth {depth:?}");
                assert_eq!(found[0].code, Some(0), "{fixture} {:?}", found[0].args);
            }
        }
        let nothing: Vec<&Row> = rows
            .iter()
            .filter(|row| {
                row.fixture == fixture && is_plain_flow(row) && flow_root(row) == Some(NOTHING)
            })
            .collect();
        assert_eq!(nothing.len(), 1, "{fixture}: a ROOT naming nothing");
        assert_eq!(nothing[0].code, Some(1), "{fixture}");
    }

    // `spec flow --trace`, every ROOT at every depth, and `spec trace`.
    for (fixture, roots, depths) in FLOW_TRACE_CASES {
        for root in std::iter::once(None).chain(roots.iter().copied().map(Some)) {
            for &depth in depths {
                let found = rows
                    .iter()
                    .filter(|row| {
                        row.fixture == fixture
                            && command(row) == "flow"
                            && has(row, "--trace")
                            && flow_root(row) == root
                            && value_of(row, "--depth") == depth
                    })
                    .count();
                assert_eq!(found, 1, "{fixture} flow --trace {root:?} {depth:?}");
            }
        }
    }
    for (fixture, calls) in TRACE_CASES {
        for call in calls {
            let mut args = vec!["trace".to_owned()];
            args.extend(call.iter().map(|arg| (*arg).to_owned()));
            let found: Vec<&Row> = rows
                .iter()
                .filter(|row| row.fixture == fixture && row.args == args)
                .collect();
            assert_eq!(found.len(), 1, "{fixture} {args:?}");
            let want = if call.contains(&NOTHING) { 1 } else { 0 };
            assert_eq!(found[0].code, Some(want), "{fixture} {args:?}");
        }
    }

    // `spec bundle`: every target set and nothing else.
    let mut sets: BTreeMap<(String, Vec<String>), Vec<&Row>> = BTreeMap::new();
    for row in rows.iter().filter(|row| command(row) == "bundle") {
        let targets = bundle_targets(row).into_iter().map(str::to_owned).collect();
        sets.entry((row.fixture.clone(), targets))
            .or_default()
            .push(row);
    }
    let want: BTreeSet<(String, Vec<String>)> = BUNDLE_SETS
        .iter()
        .map(|(fixture, targets)| {
            (
                (*fixture).to_owned(),
                targets.iter().map(|target| (*target).to_owned()).collect(),
            )
        })
        .collect();
    assert_eq!(
        sets.keys().cloned().collect::<BTreeSet<_>>(),
        want,
        "the bundle target sets"
    );
    for ((fixture, targets), set) in &sets {
        bundle_set_has_its_budgets(fixture, targets, set);
    }
}

/// A row's `--budget`, `None` for the default.
fn budget_of(row: &Row) -> Option<u64> {
    value_of(row, "--budget").map(|text| {
        text.parse()
            .unwrap_or_else(|_| panic!("{} {:?}", row.fixture, row.args))
    })
}

/// One target set's rows, from the file alone: the default, `u32::MAX`,
/// the minimum − 1 refused (exit 2), the minimum and the minimum + 1, and
/// enough budgets above them.
fn bundle_set_has_its_budgets(fixture: &str, targets: &[String], set: &[&Row]) {
    let context = format!("{fixture} {targets:?}");
    let default = set.iter().filter(|row| budget_of(row).is_none()).count();
    assert_eq!(default, 1, "{context}: the default budget");
    let mut by_budget: BTreeMap<u64, i32> = BTreeMap::new();
    for row in set {
        let Some(budget) = budget_of(row) else {
            continue;
        };
        let code = row
            .b3
            .as_ref()
            .and_then(Digests::codes)
            .unwrap_or_else(|| panic!("{context} --budget {budget}: no exit code of a bundle"));
        assert!(
            by_budget.insert(budget, code).is_none(),
            "{context}: {budget}"
        );
    }
    assert_eq!(
        by_budget.get(&u64::from(u32::MAX)),
        Some(&0),
        "{context}: {UNBOUNDED}"
    );
    let minimum = by_budget
        .iter()
        .find(|(_, code)| **code == 0)
        .map(|(&budget, _)| budget)
        .unwrap_or_else(|| panic!("{context}: no budget answered"));
    for (&budget, &code) in &by_budget {
        let want = if budget < minimum { 2 } else { 0 };
        assert_eq!(code, want, "{context} --budget {budget}");
    }
    assert!(
        by_budget.contains_key(&(minimum - 1)),
        "{context}: the minimum - 1, refused"
    );
    assert!(
        by_budget.contains_key(&(minimum + 1)),
        "{context}: the minimum + 1"
    );
    let above = by_budget.keys().filter(|&&budget| budget > minimum).count();
    assert!(above > INSIDE, "{context}: {above} budgets above {minimum}");
}

/// One target set's replayed rows, the bytes before once their digests
/// agree: the unbounded `tokens` is a budget of the set, at least
/// [`INSIDE`] budgets lie between the minimum and it, and at one of them
/// an item just fits (`bundle_hash` moves from the budget below).
fn bundle_set_is_whole(context: &str, set: &[(u64, &Run, &Run)]) {
    let json_of = |run: &Run| -> Value { one_json_document(&run.stdout).expect("JSON") };
    let by_budget: BTreeMap<u64, (&Run, &Run)> = set
        .iter()
        .map(|&(budget, text, json)| (budget, (text, json)))
        .collect();
    let (_, unbounded) = by_budget[&u64::from(u32::MAX)];
    let whole = json_of(unbounded);
    let tokens = whole["tokens"].as_u64().expect("tokens");
    assert_eq!(whole["more"], Value::from(0), "{context}: all placed");
    assert_eq!(
        whole["tail"],
        Value::Array(Vec::new()),
        "{context}: all placed"
    );
    let minimum = by_budget
        .iter()
        .find(|(_, (text, _))| text.code == 0)
        .map(|(&budget, _)| budget)
        .expect("a budget answered");
    assert!(
        by_budget.contains_key(&tokens),
        "{context}: the tokens {tokens}"
    );
    let inside = by_budget
        .keys()
        .filter(|&&budget| minimum < budget && budget < tokens)
        .count();
    let exist = usize::try_from(tokens.saturating_sub(minimum + 1)).expect("small");
    assert!(
        inside >= INSIDE.min(exist),
        "{context}: {inside} budgets between {minimum} and {tokens}"
    );
    let hash = |run: &Run| -> String {
        json_of(run)["bundle_hash"]
            .as_str()
            .expect("bundle_hash")
            .to_owned()
    };
    let boundaries = by_budget
        .iter()
        .filter(|&(&budget, (_, json))| {
            budget > minimum
                && by_budget
                    .get(&(budget - 1))
                    .is_some_and(|(_, below)| hash(below) != hash(json))
        })
        .count();
    if tokens > minimum {
        assert!(
            boundaries >= 1,
            "{context}: no boundary where an item just fits"
        );
    }
}

/// Replays every `bundle` row of `fixture`, then checks each target set.
fn replay_bundles(fixture: &str, least: usize) {
    let replayed = replay_rows(
        |row| command(row) == "bundle" && row.fixture == fixture,
        least,
    );
    let mut sets: BTreeMap<Vec<String>, Vec<(u64, &Run, &Run)>> = BTreeMap::new();
    for (row, text, json) in &replayed {
        if let Some(budget) = budget_of(row) {
            let targets = bundle_targets(row).into_iter().map(str::to_owned).collect();
            sets.entry(targets).or_default().push((budget, text, json));
        }
    }
    for (targets, set) in &sets {
        bundle_set_is_whole(&format!("{fixture} {targets:?}"), set);
    }
}

/// AC-10 "The golden": every `spec flow` row, without `--trace`, is the
/// bytes before, its exit code included.
#[test]
fn ac10_every_flow_row_is_the_bytes_before() {
    replay_rows(is_plain_flow, 60);
}

/// AC-10 "The golden": every `spec flow --trace` and `spec trace` row is the
/// bytes before (the lanes walk the flow).
#[test]
fn ac10_every_trace_row_is_the_bytes_before() {
    replay_rows(is_trace, 20);
}

/// AC-11: every `spec bundle` row of `spec-a` is the bytes before by
/// digest: the text, the JSON (`bundle_hash` in it), stderr, the exit
/// codes. M: round per part.
#[test]
fn ac11_every_bundle_row_of_spec_a_is_the_bytes_before() {
    replay_bundles("spec-a", 12 * 25);
}

/// AC-11: every `spec bundle` row of `spec-b` is the bytes before by
/// digest. M: round per part.
#[test]
fn ac11_every_bundle_row_of_spec_b_is_the_bytes_before() {
    replay_bundles("spec-b", 7 * 25);
}

// ------------------------------------------------------------------ FS-8

/// Git's blob of `flow_linear_before.json` at `baac762`, the commit before
/// docs/features/flow-scope.md's re-record: the rows `e863186` printed.
const BEFORE_FLOW_SCOPE: &str = "5dfc22d94c5f9ad78a6946cb9ba1cbaef0a14733";

/// The link type no flow reads since flow-scope (F1).
const DEPENDS_ON: &str = "depends_on";

/// The three a flow reads.
const FLOW_TYPES: [&str; 3] = ["follows", "uses", "supplies"];

/// The rows before flow-scope's re-record, from git's object store.
fn golden_before_flow_scope() -> Vec<Row> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root())
        .args(["cat-file", "blob", BEFORE_FLOW_SCOPE])
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "the golden before flow-scope, blob {BEFORE_FLOW_SCOPE} (baac762:crates/specengine-cli/\
         tests/{GOLDEN}), is not in this clone: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{BEFORE_FLOW_SCOPE} is no list of rows: {error}"))
}

/// The field of a text line up to its first ` | `.
fn head(line: &str) -> &str {
    line.split(" | ").next().unwrap_or(line)
}

/// The ID of a node line (`<distance or -> <ID> | ...`), if it is one.
fn node_line_id(line: &str) -> Option<&str> {
    let (distance, id) = head(line).split_once(' ')?;
    (!id.contains(' ') && (distance == "-" || distance.parse::<usize>().is_ok())).then_some(id)
}

/// Whether `part` is a subsequence of `whole`.
fn in_order<T: PartialEq>(part: &[T], whole: &[T]) -> bool {
    let mut rest = whole.iter();
    part.iter().all(|item| rest.any(|other| other == item))
}

fn array<'a>(document: &'a Value, key: &str) -> &'a [Value] {
    document[key].as_array().map_or(&[], Vec::as_slice)
}

/// The type of an edge or an `inner` entry.
fn type_of(item: &Value) -> &str {
    item["type"].as_str().unwrap_or_default()
}

/// A node of the old read only `depends_on` admitted: every edge it is an
/// end of and every `inner` entry it holds is `depends_on`, and one is.
fn admitted_by_depends_on_alone(old: &Value, node: &Value) -> bool {
    let id = &node["id"];
    let touching: Vec<&Value> = array(old, "edges")
        .iter()
        .filter(|edge| edge["from"] == *id || edge["to"] == *id)
        .collect();
    let inner = array(node, "inner");
    let all_depends = touching.iter().all(|edge| type_of(edge) == DEPENDS_ON)
        && inner.iter().all(|entry| type_of(entry) == DEPENDS_ON);
    all_depends && (!touching.is_empty() || !inner.is_empty())
}

/// What a `flow` row's re-record moved that FS-8 does not allow; empty when
/// every move is a `depends_on` one.
fn flow_moves(old: &Row, new: &Row) -> Vec<String> {
    let mut problems = Vec::new();
    if old.code != new.code {
        problems.push(format!("exit {:?} now, {:?} before", new.code, old.code));
        return problems;
    }
    let (old_text, new_text) = (
        old.text.as_deref().unwrap_or_default(),
        new.text.as_deref().unwrap_or_default(),
    );
    let (old_json, new_json) = (
        old.json.as_deref().unwrap_or_default(),
        new.json.as_deref().unwrap_or_default(),
    );
    // A row that read no `depends_on` keeps its bytes.
    if !old_json.contains("\"depends_on\"") {
        if old_text != new_text {
            problems.push(format!(
                "the text moved, and the row read no depends_on: {}",
                first_difference(new_text, old_text)
            ));
        }
        if old_json != new_json {
            problems.push("the JSON moved, and the row read no depends_on".to_owned());
        }
        return problems;
    }
    let mut parse = |text: &str, when: &str| -> Option<Value> {
        let parsed = serde_json::from_str(text).ok();
        if parsed.is_none() {
            problems.push(format!("the JSON {when} is no document"));
        }
        parsed
    };
    let (Some(before), Some(now)) = (parse(old_json, "before"), parse(new_json, "now")) else {
        return problems;
    };

    // The gap rows, and what stands beside them, byte for byte.
    let gaps = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|line| line.starts_with("gap "))
            .map(str::to_owned)
            .collect()
    };
    if gaps(old_text) != gaps(new_text) {
        problems.push("the text's gap lines moved".to_owned());
    }
    for key in ["gaps", "root", "reason", "depth", "notes", "left_out"] {
        if before[key] != now[key] {
            problems.push(format!("`{key}` moved"));
        }
    }

    // Nothing reads `depends_on` now.
    for edge in array(&now, "edges") {
        if type_of(edge) == DEPENDS_ON {
            problems.push(format!("a depends_on edge is read now: {edge}"));
        }
    }
    for node in array(&now, "nodes") {
        for entry in array(node, "inner") {
            if type_of(entry) == DEPENDS_ON {
                problems.push(format!("{}: a depends_on inner entry now", node["id"]));
            }
        }
    }

    // Nodes: the kept ones in their order, each the same but for `inner`,
    // whose removed entries are `depends_on` ones; a removed one admitted by
    // `depends_on` alone.
    let (old_nodes, new_nodes) = (array(&before, "nodes"), array(&now, "nodes"));
    let old_ids: Vec<&Value> = old_nodes.iter().map(|node| &node["id"]).collect();
    let new_ids: Vec<&Value> = new_nodes.iter().map(|node| &node["id"]).collect();
    if !in_order(&new_ids, &old_ids) {
        problems.push("the nodes now are no subsequence of the nodes before".to_owned());
    }
    let mut removed: BTreeSet<String> = BTreeSet::new();
    for node in old_nodes {
        let Some(kept) = new_nodes.iter().find(|other| other["id"] == node["id"]) else {
            if !admitted_by_depends_on_alone(&before, node) {
                problems.push(format!(
                    "node {} removed, and not depends_on's alone",
                    node["id"]
                ));
            }
            removed.insert(node["id"].as_str().unwrap_or_default().to_owned());
            continue;
        };
        for key in ["distance", "start", "end", "stage"] {
            if node[key] != kept[key] {
                problems.push(format!("{}: `{key}` moved", node["id"]));
            }
        }
        let keys: BTreeSet<&String> = node
            .as_object()
            .into_iter()
            .chain(kept.as_object())
            .flat_map(|object| object.keys())
            .collect();
        for key in keys {
            if key != "inner" && node[key] != kept[key] {
                problems.push(format!("{}: `{key}` moved", node["id"]));
            }
        }
        let (old_inner, new_inner) = (array(node, "inner"), array(kept, "inner"));
        if !in_order(new_inner, old_inner) {
            problems.push(format!(
                "{}: `inner` gained or reordered an entry",
                node["id"]
            ));
        }
        for entry in old_inner {
            if !new_inner.contains(entry) && type_of(entry) != DEPENDS_ON {
                problems.push(format!("{}: inner entry {entry} removed", node["id"]));
            }
        }
    }

    // Edges: the kept ones in their order; of an uncut row, every removed
    // one `depends_on`, so every `follows`, `uses` and `supplies` stays.
    let (old_edges, new_edges) = (array(&before, "edges"), array(&now, "edges"));
    if !in_order(new_edges, old_edges) {
        problems.push("the edges now are no subsequence of the edges before".to_owned());
    }
    let uncut = before["truncated"] == Value::Bool(false);
    if uncut {
        for edge in old_edges {
            if !new_edges.contains(edge) {
                if FLOW_TYPES.contains(&type_of(edge)) {
                    problems.push(format!("a {} edge removed: {edge}", type_of(edge)));
                } else if type_of(edge) != DEPENDS_ON {
                    problems.push(format!("an edge of another type removed: {edge}"));
                }
            }
        }
        for key in ["truncated", "omitted_nodes", "omitted_edges"] {
            if before[key] != now[key] {
                problems.push(format!("`{key}` moved on an uncut row"));
            }
        }
    }

    // The text: its header counts what the JSON holds; every other line one
    // of the lines before, in order; a line gone a `depends_on` edge's or a
    // removed node's.
    let mut old_lines = old_text.lines();
    let mut new_lines = new_text.lines();
    let (old_header, new_header) = (old_lines.next(), new_lines.next());
    if let (Some(old_header), Some(new_header)) = (old_header, new_header) {
        let count = |key: &str, omitted: &str| {
            array(&now, key).len() as u64 + now[omitted].as_u64().unwrap_or(0)
        };
        let rebuilt = old_header
            .split_once(" | nodes ")
            .and_then(|(scope, rest)| {
                let (_, tail) = rest.split_once(", depth ")?;
                Some(format!(
                    "{scope} | nodes {}, edges {}, depth {tail}",
                    count("nodes", "omitted_nodes"),
                    count("edges", "omitted_edges")
                ))
            });
        if rebuilt.as_deref() != Some(new_header) {
            problems.push(format!(
                "the header {new_header:?}, not {old_header:?} with the counts kept"
            ));
        }
    } else if old_header != new_header {
        problems.push("one text has no header".to_owned());
    }
    // The lines before, less each `depends_on` edge's and each removed
    // node's, a kept node's `inner <n>` lowered by its `depends_on` entries'
    // counts (the sum a folded line prints): the lines now, exactly, of an
    // uncut row; of a cut one, in order.
    let old_body: Vec<&str> = old_lines.filter(|line| !line.starts_with("gap ")).collect();
    let new_body: Vec<String> = new_lines
        .filter(|line| !line.starts_with("gap "))
        .map(str::to_owned)
        .collect();
    let mut expected: Vec<String> = Vec::new();
    for line in &old_body {
        if head(line).contains(" -> ") && line.contains(" | depends_on | ") {
            continue;
        }
        let Some(id) = node_line_id(line) else {
            expected.push((*line).to_owned());
            continue;
        };
        if removed.contains(id) {
            continue;
        }
        let dropped: u64 = old_nodes
            .iter()
            .find(|node| node["id"] == id)
            .map(|node| {
                array(node, "inner")
                    .iter()
                    .filter(|entry| type_of(entry) == DEPENDS_ON)
                    .filter_map(|entry| entry["count"].as_u64())
                    .sum()
            })
            .unwrap_or(0);
        expected.push(lowered_inner(line, dropped));
    }
    let exact = uncut && !old_body.iter().any(|line| line.starts_with("[truncated: "));
    if exact && new_body != expected {
        problems.push(format!(
            "the text's lines now are not the lines before less depends_on's: {}",
            first_difference(&new_body.join("\n"), &expected.join("\n"))
        ));
    } else if !exact && !in_order(&new_body, &expected) {
        problems.push("the text's lines now are no subsequence of the lines before".to_owned());
    }
    problems
}

/// A node line with its `inner <n>` lowered by `by` (unchanged at 0, or with
/// no `inner` field).
fn lowered_inner(line: &str, by: u64) -> String {
    const FIELD: &str = ", inner ";
    let Some(at) = line.find(FIELD).filter(|_| by > 0) else {
        return line.to_owned();
    };
    let start = at + FIELD.len();
    let digits = line[start..].bytes().take_while(u8::is_ascii_digit).count();
    let Ok(count) = line[start..start + digits].parse::<u64>() else {
        return line.to_owned();
    };
    format!(
        "{}{}{}",
        &line[..start],
        count.saturating_sub(by),
        &line[start + digits..]
    )
}

/// FS-8: the re-recorded golden holds the same rows, in the same order, as
/// the one before flow-scope; every `trace` and `bundle` row byte-equal,
/// every `flow` row moved by `depends_on` alone ([`flow_moves`]); and some
/// `flow` row did move, or the golden was not re-recorded. M13: a re-record
/// that also drops a `follows` edge.
#[test]
fn fs8_the_re_record_moves_only_depends_on() {
    let before = golden_before_flow_scope();
    let now = golden();
    let keys = |rows: &[Row]| -> Vec<(String, Vec<String>)> {
        rows.iter()
            .map(|row| (row.fixture.clone(), row.args.clone()))
            .collect()
    };
    assert_eq!(
        keys(&now),
        keys(&before),
        "the same rows, in the same order"
    );
    let mut problems: Vec<String> = Vec::new();
    let mut moved = Vec::new();
    for (old, new) in before.iter().zip(&now) {
        let context = format!("{} {:?}", old.fixture, old.args);
        if command(old) == "flow" {
            if old.text != new.text || old.json != new.json {
                moved.push(context.clone());
            }
            problems.extend(
                flow_moves(old, new)
                    .into_iter()
                    .map(|problem| format!("{context}: {problem}")),
            );
        } else if (&old.code, &old.text, &old.json, &old.b3)
            != (&new.code, &new.text, &new.json, &new.b3)
        {
            problems.push(format!("{context}: a {} row moved", command(old)));
        }
    }
    assert!(
        problems.is_empty(),
        "{} moves FS-8 does not allow:\n{}",
        problems.len(),
        problems.join("\n")
    );
    assert!(
        !moved.is_empty(),
        "no flow row moved: {GOLDEN} is still the golden before flow-scope"
    );
    eprintln!(
        "FS-8: {} flow rows moved by depends_on:\n{}",
        moved.len(),
        moved.join("\n")
    );
}

/// `value` with every object's keys in byte order, as the golden is written.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted: BTreeMap<&String, Value> = object
                .iter()
                .map(|(key, item)| (key, canonical(item)))
                .collect();
            Value::Object(
                sorted
                    .into_iter()
                    .map(|(key, item)| (key.clone(), item))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// FS-8's re-record, by hand: every row of the golden replayed as the
/// replays run it, its bytes (`flow`, `trace`) or digests (`bundle`) kept,
/// written in the golden's format (keys in order, one-space indent, a final
/// line end) to the absolute path `SPECENGINE_FLOW_LINEAR_RECORD` names,
/// outside the repository. `fs8_the_re_record_moves_only_depends_on` judges
/// the file once it replaces the golden.
#[test]
#[ignore = "manual: FS-8's re-record of the golden (docs/features/flow-scope.md)"]
fn fs8_record_the_golden() {
    let out = std::env::var_os("SPECENGINE_FLOW_LINEAR_RECORD")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .expect("SPECENGINE_FLOW_LINEAR_RECORD names the file to write");
    assert!(out.is_absolute(), "{}: not absolute", out.display());
    assert!(
        !out.starts_with(repository_root()),
        "{}: write outside the repository, then copy",
        out.display()
    );
    let rows: Vec<Row> = golden()
        .into_iter()
        .map(|row| {
            let (text, json) = replay(&row.fixture, &row.args);
            if command(&row) == "bundle" {
                Row {
                    code: None,
                    text: None,
                    json: None,
                    b3: Some(Digests::of(&text, &json)),
                    ..row
                }
            } else {
                assert_eq!(
                    text.code, json.code,
                    "{} {:?}: one exit code",
                    row.fixture, row.args
                );
                Row {
                    code: Some(text.code),
                    text: Some(text.stdout),
                    json: Some(json.stdout),
                    b3: None,
                    ..row
                }
            }
        })
        .collect();
    let value = canonical(&serde_json::to_value(&rows).expect("rows to JSON"));
    let mut bytes = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(
        &mut bytes,
        serde_json::ser::PrettyFormatter::with_indent(b" "),
    );
    value.serialize(&mut serializer).expect("rows written");
    bytes.push(b'\n');
    fs::write(&out, bytes).unwrap_or_else(|error| panic!("{}: {error}", out.display()));
}
