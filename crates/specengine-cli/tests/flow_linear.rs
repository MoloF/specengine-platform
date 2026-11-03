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
//! The golden is never recorded again: a second recording with the binary
//! after the slice would prove nothing. The cases it must hold are the
//! constants below; `the_golden_holds_every_case_of_the_spec` checks them
//! on the file alone, and the bundle replays check what needs the bytes
//! (the `tokens`, the boundaries) once the digests have proved them the
//! bytes before.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use common::graph::spec30;
use common::{Run, Scratch, one_json_document};
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
            &json.stdout,
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
