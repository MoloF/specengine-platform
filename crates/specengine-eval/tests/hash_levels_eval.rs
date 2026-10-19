//! docs/features/hash-levels-eval.md (Phase 3 slice 5a-2): the figures
//! `specengine-eval ast-hash` prints as `result.levels` (5a-1's levels and
//! fingerprints) and `ron` prints as `result.value_hash` (5a-1b's RON value
//! hash), on the real binary.
//!
//! - RD-1, every field present on the fixtures:
//!   `rd1_levels_and_value_hash_carry_every_field_on_the_fixtures`. Its
//!   mutation "`0` for an absent rustfmt":
//!   `levels_fmt_rows_are_null_without_rustfmt` (a rustfmt stub failing
//!   `--version`, as in `ast_hash_units.rs`). Refused RON files and a corpus
//!   without a marker, against 5a-1b's hand inventory of
//!   `fixtures/hash-levels/ron/`:
//!   `value_hash_counts_refused_files_and_is_null_without_markers`.
//! - TE-2: without `--pilot`, `ast-hash` and `ron` give the `levels` pins of
//!   `fixtures/ast-hash/expected.json` and the `value_hash` pins of
//!   `fixtures/ron/expected.json` exactly, `edited_changed` = `edited`, and
//!   leave `git status -- fixtures/` as they found it (`te2_*`); a repeat run
//!   differs only in its times. The pins keep the Data's relations and tell
//!   the named mutations apart without a run
//!   (`te2_expected_json_pins_keep_the_data_s_relations`).
//! - TE-1, pilots A and B, the owner's manual run: `te1_pilot_a`,
//!   `te1_pilot_b` (`#[ignore]`), the TE-1 section below.
//!
//! PHASE-B: written from the spec before the code. Every JSON path here is
//! the spec's Data (the constants below), none read from an implementation.
//!
//! Readings where the spec leaves a choice (recorded at shipping):
//!
//! - L1: an item's index (the neighbour parity, the rename's matching) runs
//!   over every item of the file in analysis order, broken ones too, and the
//!   neighbour edit and the rename touch broken items like any other (they
//!   change the lines around their neighbours); every count covers items
//!   with levels, before and, where compared, after.
//! - L2: a percentage has one decimal, over the items compared: items hashed
//!   in the original (`fmt_*`, `comments`: v2's rule), `checked`
//!   (`neighbour`), hashed files (`module_stable_pct`), `hashed`
//!   (`trivial.pct`, `crlf_stable_pct`), `bound` (`bound_stable_pct`).
//! - L3: without a rustfmt a `fmt_*` row is itself `null`
//!   (`stable_pct.fmt_default: null`), in `stable_pct` and
//!   `module_stable_pct` alike, not an object of nulls; no other row moves.
//! - L4: `trivial_at` is an object keyed by the threshold's decimal text;
//!   `cost` sits in `levels`.
//! - L5: `quote_separates` counts over `body_groups` and
//!   `skeleton_groups_in_file` together; the fixture holds no body group, so
//!   either reading gives its pin.
//! - L6: `renamed` counts the named items holding no named item that have
//!   levels; containers holding named items keep their names; `kept.quote`
//!   is measured like any field: an item whose `before` or `after` line
//!   holds a renamed name is not kept.
//! - L7: `bound` counts the markers whose comment has an entry in the
//!   original and in the stripped copy, whatever their ID or level list;
//!   `bound_stable_pct` is `null` exactly when `bound` is 0.
//! - L8: "leave `git status -- fixtures/` empty" is checked as "the same
//!   before and after the run": a tree under development may hold
//!   uncommitted fixture edits; on a committed tree both are empty.
//! - L9: TE-1 judges at 100.0 `stable_pct.<p>.<f>` for every `f` but
//!   `quote`, `rename.kept.<f>` = `renamed` for the same `f`,
//!   `crlf_stable_pct` and a non-null `bound_stable_pct`, and requires zero
//!   rows in the run's `levels-unstable.json` (only its count printed): a
//!   rounded 100.0 can hide up to 0.05 % changed items, a row cannot (its
//!   `changed` lists judged fields only, modules included); every other
//!   figure is printed, not judged; a shortfall fails the run after every
//!   figure is printed.
//! - L10: the TE-1 child gets `PATH`, the scratch `HOME` (also its working
//!   directory), the label's corpus variable (neither command reads a scheme
//!   or a census config) and `SPECENGINE_RUSTFMT`, `SPECENGINE_CARGO` as
//!   absolute paths: the parent's when set, else `rustup which` run at `/`,
//!   where the harness runs both. What lands under the scratch `HOME` (the
//!   toolchain's own files) is reported, not judged.
//! - L11: TE-1's read-only proof lists every entry the harness walk can
//!   reach (hidden and `target*` directories skipped, hidden files it cannot
//!   read skipped, symlinks listed, never followed), beside `git status`.
//! - L12: `hashed` + `cannot_verify` = v2's `items`, `modules.hashed` +
//!   `modules.cannot_verify` = v2's `files`, `trivial.items` =
//!   `trivial_at.<TRIVIAL_BODY_BYTES>.items`.
//! - L13: comment stripping turns a comment into one space (v2's
//!   `strip_comments`), so a comment-only line turns blank and `before` /
//!   `after` skip it; the neighbour edit inserts at the attribute's (else
//!   the item's) byte, so the moved line keeps its trimmed text and only an
//!   `after` line can change.
//! - L14: `skeleton_groups_in_file` counts every item with levels, trivial
//!   ones included; only `body_groups` and `followable` leave trivial items
//!   out (the Data names "non-trivial" for those alone).
//! - L15: on a corpus without a `.rs` newline each `cost` field is `null`
//!   (a rate per zero kLOC is undefined); elsewhere a number.

mod pilot;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};
use specengine_code::TRIVIAL_BODY_BYTES;

const BIN: &str = env!("CARGO_BIN_EXE_specengine-eval");

// PHASE-B: the spec's names (its Data), the whole contract of this file.

/// `levels.stable_pct.<p>`: the perturbations.
const PERTURBATIONS: [&str; 4] = ["fmt_default", "fmt_contrast", "comments", "neighbour"];

/// The rows that need a rustfmt: `null` without one (L3).
const FMT_ROWS: [&str; 2] = ["fmt_default", "fmt_contrast"];

/// `levels.module_stable_pct.<p>`: the first three perturbations.
const MODULE_PERTURBATIONS: [&str; 3] = ["fmt_default", "fmt_contrast", "comments"];

/// `levels.stable_pct.<p>.<f>` and `levels.rename.kept.<f>`.
const FIELDS: [&str; 6] = ["sig", "body", "deps", "skeleton", "body_bytes", "quote"];

/// The fields judged at 100.0: `quote` is reported, never judged.
const JUDGED: [&str; 5] = ["sig", "body", "deps", "skeleton", "body_bytes"];

/// `levels.module_stable_pct.<p>.<f>`.
const MODULE_FIELDS: [&str; 3] = ["sig", "body", "deps"];

/// `levels.trivial_at.<t>`: the sweep (Q2).
const THRESHOLDS: [u32; 5] = [64, 96, 124, 160, 256];

/// The keys of `result.levels`.
const LEVELS: [&str; 13] = [
    "hashed",
    "cannot_verify",
    "modules",
    "stable_pct",
    "module_stable_pct",
    "neighbour",
    "trivial",
    "body_groups",
    "skeleton_groups_in_file",
    "quote_separates",
    "trivial_at",
    "rename",
    "cost",
];

/// The keys of `result.value_hash` (Q1).
const VALUE_HASH: [&str; 6] = [
    "anchored",
    "hashed",
    "cannot_verify",
    "crlf_stable_pct",
    "bound",
    "bound_stable_pct",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

fn read_json(path: &Path) -> Value {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{}: not JSON: {error}", path.display()))
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-levels-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(fs::canonicalize(&path).expect("canonical scratch"))
    }

    fn join(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// `specengine-eval` at the repository root, no pilot variable inherited.
fn eval() -> Command {
    let mut command = Command::new(BIN);
    command
        .current_dir(repository_root())
        .env_remove("SPECENGINE_PILOT_A")
        .env_remove("SPECENGINE_PILOT_B");
    command
}

/// Exit 0 and exactly one JSON line on stdout: the envelope.
fn envelope(output: &Output, context: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "{context}: stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    let mut lines = stdout.lines();
    let line = lines
        .next()
        .unwrap_or_else(|| panic!("{context}: no JSON line on stdout"));
    assert!(
        lines.next().is_none(),
        "{context}: stdout must hold exactly one line:\n{stdout}"
    );
    serde_json::from_str(line)
        .unwrap_or_else(|error| panic!("{context}: stdout is not JSON: {error}\n{line}"))
}

/// `<measurement> --out <out>` over its default fixture: the envelope and
/// stdout.
fn run_fixture(measurement: &str, out: &Path) -> (Value, String) {
    let output = eval()
        .arg(measurement)
        .arg("--out")
        .arg(out)
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output, measurement);
    assert_eq!(envelope["measurement"], measurement);
    assert_eq!(envelope["label"], "fixtures");
    (
        envelope,
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// Byte-for-byte snapshot of a directory tree.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .expect("under the root")
                        .to_path_buf(),
                    fs::read(&path).expect("readable file"),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// `git status` of `fixtures/`, untracked files included (L8).
fn git_status_of_fixtures() -> String {
    let output = Command::new("git")
        .current_dir(repository_root())
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "fixtures/",
        ])
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn count(value: &Value, what: &str) -> u64 {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("{what} must be a count, got {value}"))
}

fn number(value: &Value, what: &str) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{what} must be a number, got {value}"))
}

/// The value at a dotted path (`null` when absent).
fn at<'a>(value: &'a Value, dotted: &str) -> &'a Value {
    dotted.split('.').fold(value, |value, key| &value[key])
}

/// Problems found, one line each, checked once: a red run lists them all.
#[derive(Default)]
struct Problems(Vec<String>);

impl Problems {
    fn push(&mut self, line: String) {
        self.0.push(line);
    }

    fn count(&mut self, value: &Value, path: &str) {
        if !value.is_u64() {
            self.push(format!("{path}: a count expected, got {value}"));
        }
    }

    fn pct(&mut self, value: &Value, path: &str) {
        if !value
            .as_f64()
            .is_some_and(|pct| (0.0..=100.0).contains(&pct))
        {
            self.push(format!(
                "{path}: a percentage in 0..=100 expected, got {value}"
            ));
        }
    }

    /// A time >= 0, or `null` where nothing was timed (L15).
    fn time(&mut self, value: &Value, path: &str) {
        if !value.is_null()
            && !value
                .as_f64()
                .is_some_and(|time| time.is_finite() && time >= 0.0)
        {
            self.push(format!("{path}: a time >= 0 or null expected, got {value}"));
        }
    }

    fn null(&mut self, value: &Value, path: &str) {
        if !value.is_null() {
            self.push(format!("{path}: null expected, got {value}"));
        }
    }

    /// An object whose keys are exactly `keys`; false when not an object.
    fn exact(&mut self, value: &Value, path: &str, keys: &[&str]) -> bool {
        let Some(map) = value.as_object() else {
            self.push(format!("{path}: an object expected, got {value}"));
            return false;
        };
        let got: BTreeSet<&str> = map.keys().map(String::as_str).collect();
        let want: BTreeSet<&str> = keys.iter().copied().collect();
        if got != want {
            self.push(format!("{path}: keys {got:?}, expected {want:?}"));
        }
        true
    }

    /// An object holding at least `keys`; false when not an object.
    fn holds(&mut self, value: &Value, path: &str, keys: &[&str]) -> bool {
        let Some(map) = value.as_object() else {
            self.push(format!("{path}: an object expected, got {value}"));
            return false;
        };
        for key in keys {
            if !map.contains_key(*key) {
                self.push(format!("{path}.{key}: missing"));
            }
        }
        true
    }

    fn check(self, context: &str) {
        assert!(self.0.is_empty(), "{context}:\n{}", self.0.join("\n"));
    }
}

/// RD-1: every field of `result.levels` the Data names, typed; the `fmt_*`
/// rows numbers with a rustfmt, `null` without one (L3).
fn levels_shape(levels: &Value, rustfmt: bool) -> Problems {
    let mut problems = Problems::default();
    if !problems.holds(levels, "levels", &LEVELS) {
        return problems;
    }
    for key in ["hashed", "cannot_verify", "quote_separates"] {
        problems.count(&levels[key], &format!("levels.{key}"));
    }
    for key in ["hashed", "cannot_verify"] {
        problems.count(&levels["modules"][key], &format!("levels.modules.{key}"));
    }
    for (table, rows, fields) in [
        ("stable_pct", &PERTURBATIONS[..], &FIELDS[..]),
        (
            "module_stable_pct",
            &MODULE_PERTURBATIONS[..],
            &MODULE_FIELDS[..],
        ),
    ] {
        let path = format!("levels.{table}");
        if !problems.exact(&levels[table], &path, rows) {
            continue;
        }
        for row in rows {
            let value = &levels[table][*row];
            let path = format!("{path}.{row}");
            if FMT_ROWS.contains(row) && !rustfmt {
                problems.null(value, &path);
            } else if problems.exact(value, &path, fields) {
                for field in fields {
                    problems.pct(&value[*field], &format!("{path}.{field}"));
                }
            }
        }
    }
    for (group, keys) in [
        ("neighbour", &["edited", "edited_changed", "checked"][..]),
        ("body_groups", &["groups", "items"][..]),
        ("skeleton_groups_in_file", &["groups", "items"][..]),
        ("trivial", &["items"][..]),
    ] {
        let path = format!("levels.{group}");
        if problems.holds(&levels[group], &path, keys) {
            for key in keys {
                problems.count(&levels[group][*key], &format!("{path}.{key}"));
            }
        }
    }
    problems.pct(&levels["trivial"]["pct"], "levels.trivial.pct");
    let names: Vec<String> = THRESHOLDS.iter().map(u32::to_string).collect();
    let thresholds: Vec<&str> = names.iter().map(String::as_str).collect();
    if problems.exact(&levels["trivial_at"], "levels.trivial_at", &thresholds) {
        for threshold in &thresholds {
            for key in ["items", "body_twins"] {
                problems.count(
                    &levels["trivial_at"][*threshold][key],
                    &format!("levels.trivial_at.{threshold}.{key}"),
                );
            }
        }
    }
    let rename = &levels["rename"];
    for key in ["renamed", "followable"] {
        problems.count(&rename[key], &format!("levels.rename.{key}"));
    }
    if problems.exact(&rename["kept"], "levels.rename.kept", &FIELDS) {
        for field in FIELDS {
            problems.count(
                &rename["kept"][field],
                &format!("levels.rename.kept.{field}"),
            );
        }
    }
    let cost = ["levels_ms_per_kloc", "v2_ms_per_kloc"];
    if problems.holds(&levels["cost"], "levels.cost", &cost) {
        for key in cost {
            problems.time(&levels["cost"][key], &format!("levels.cost.{key}"));
        }
    }
    problems
}

/// RD-1: the six fields of `result.value_hash`, typed.
fn value_hash_shape(value_hash: &Value) -> Problems {
    let mut problems = Problems::default();
    if !problems.exact(value_hash, "value_hash", &VALUE_HASH) {
        return problems;
    }
    for key in ["anchored", "hashed", "cannot_verify", "bound"] {
        problems.count(&value_hash[key], &format!("value_hash.{key}"));
    }
    let crlf = &value_hash["crlf_stable_pct"];
    if value_hash["hashed"].as_u64() != Some(0) || !crlf.is_null() {
        problems.pct(crlf, "value_hash.crlf_stable_pct");
    }
    let stable = &value_hash["bound_stable_pct"];
    match value_hash["bound"].as_u64() {
        Some(0) => problems.null(stable, "value_hash.bound_stable_pct (no bound marker)"),
        Some(_) => problems.pct(stable, "value_hash.bound_stable_pct"),
        None => {}
    }
    problems
}

/// The relations the Data states or implies between one run's figures
/// (L12); `result` is the whole `ast-hash` result.
fn levels_invariants(result: &Value) -> Problems {
    let levels = &result["levels"];
    let mut problems = Problems::default();
    let get = |dotted: &str| at(levels, dotted).as_u64();
    let Some(hashed) = get("hashed") else {
        problems.push("levels.hashed: a count expected".to_owned());
        return problems;
    };
    // Non-vacuity: every edited item's `sig` changed.
    match (get("neighbour.edited"), get("neighbour.edited_changed")) {
        (Some(edited), Some(changed)) => {
            if edited == 0 && hashed > 0 {
                problems.push("levels.neighbour.edited is 0: nothing was edited".to_owned());
            }
            if changed != edited {
                problems.push(format!(
                    "levels.neighbour.edited_changed {changed} != edited {edited}: every \
                     edited item's sig must change"
                ));
            }
        }
        other => problems.push(format!(
            "levels.neighbour.edited, edited_changed: {other:?}"
        )),
    }
    // Every analysed item and file has levels or not.
    for (parts, whole, what) in [
        (
            (get("hashed"), get("cannot_verify")),
            result["items"].as_u64(),
            "levels.hashed + levels.cannot_verify = items",
        ),
        (
            (get("modules.hashed"), get("modules.cannot_verify")),
            result["files"].as_u64(),
            "levels.modules.hashed + levels.modules.cannot_verify = files",
        ),
    ] {
        match (parts, whole) {
            ((Some(a), Some(b)), Some(total)) if a + b == total => {}
            other => problems.push(format!("{what}: {other:?}")),
        }
    }
    // The one triviality constant is a row of the sweep.
    let row = format!("trivial_at.{TRIVIAL_BODY_BYTES}.items");
    if get("trivial.items") != get(&row) {
        problems.push(format!(
            "levels.trivial.items {:?} != levels.{row} {:?}",
            get("trivial.items"),
            get(&row)
        ));
    }
    // Items below a threshold never fall, twins at or above it never rise.
    let mut previous: Option<(u32, u64, u64)> = None;
    for threshold in THRESHOLDS {
        let items = get(&format!("trivial_at.{threshold}.items"));
        let twins = get(&format!("trivial_at.{threshold}.body_twins"));
        let (Some(items), Some(twins)) = (items, twins) else {
            problems.push(format!(
                "levels.trivial_at.{threshold}: {items:?}, {twins:?}"
            ));
            continue;
        };
        if items > hashed {
            problems.push(format!(
                "levels.trivial_at.{threshold}.items {items} > hashed {hashed}"
            ));
        }
        if let Some((before, items_before, twins_before)) = previous
            && (items < items_before || twins > twins_before)
        {
            problems.push(format!(
                "levels.trivial_at: {before} gives items {items_before}, body_twins \
                 {twins_before}; {threshold} gives {items}, {twins}"
            ));
        }
        previous = Some((threshold, items, twins));
    }
    let mut bounded = vec![
        ("neighbour.checked", "hashed"),
        ("neighbour.edited", "hashed"),
        ("trivial.items", "hashed"),
        ("rename.renamed", "hashed"),
        ("rename.followable", "rename.renamed"),
    ];
    let kept: Vec<String> = FIELDS
        .iter()
        .map(|field| format!("rename.kept.{field}"))
        .collect();
    bounded.extend(kept.iter().map(|part| (part.as_str(), "rename.renamed")));
    for (part, whole) in bounded {
        if let (Some(part_value), Some(whole_value)) = (get(part), get(whole))
            && part_value > whole_value
        {
            problems.push(format!(
                "levels.{part} {part_value} > levels.{whole} {whole_value}"
            ));
        }
    }
    for group in ["body_groups", "skeleton_groups_in_file"] {
        if let (Some(groups), Some(items)) = (
            get(&format!("{group}.groups")),
            get(&format!("{group}.items")),
        ) && items < 2 * groups
        {
            problems.push(format!(
                "levels.{group}: {groups} groups of 2+ items hold only {items} items"
            ));
        }
    }
    if let (Some(separates), Some(body), Some(skeleton)) = (
        get("quote_separates"),
        get("body_groups.groups"),
        get("skeleton_groups_in_file.groups"),
    ) && separates > body + skeleton
    {
        problems.push(format!(
            "levels.quote_separates {separates} > the {} groups",
            body + skeleton
        ));
    }
    problems
}

/// The relations between the `value_hash` figures.
fn value_hash_invariants(value_hash: &Value) -> Problems {
    let mut problems = Problems::default();
    let get = |key: &str| value_hash[key].as_u64();
    match (get("anchored"), get("hashed"), get("cannot_verify")) {
        (Some(anchored), Some(hashed), Some(refused)) if anchored == hashed + refused => {}
        other => problems.push(format!(
            "value_hash.anchored = hashed + cannot_verify: {other:?}"
        )),
    }
    if let (Some(bound), Some(hashed)) = (get("bound"), get("hashed"))
        && bound > hashed
    {
        problems.push(format!("value_hash.bound {bound} > hashed {hashed}"));
    }
    problems
}

/// Every value `expected` pins, at any depth (`_` keys are notes), against
/// `got`: a count or a `null` exactly, a percentage by value.
fn pinned(expected: &Value, got: &Value, path: &str, problems: &mut Problems) {
    match expected {
        Value::Object(map) => {
            for (key, want) in map {
                if key.starts_with('_') {
                    continue;
                }
                let child = format!("{path}.{key}");
                match got.get(key) {
                    Some(value) => pinned(want, value, &child, problems),
                    None => problems.push(format!("{child}: missing, expected {want}")),
                }
            }
        }
        Value::Number(want) if want.is_f64() => {
            if got.as_f64() != want.as_f64() {
                problems.push(format!("{path}: {got}, expected {want}"));
            }
        }
        _ => {
            if got != expected {
                problems.push(format!("{path}: {got}, expected {expected}"));
            }
        }
    }
}

/// The `fmt_*` quote rows, the one part of `levels` no pin holds (rustfmt's
/// version decides them): numbers below 100.0, since any rustfmt rewrites
/// `unformatted.rs`'s `pub struct   Config{`, whose `quote` holds the text.
fn assert_fmt_quote_rows_measured(levels: &Value) {
    for row in FMT_ROWS {
        let quote = number(
            &levels["stable_pct"][row]["quote"],
            &format!("levels.stable_pct.{row}.quote"),
        );
        assert!(
            (0.0..100.0).contains(&quote),
            "levels.stable_pct.{row}.quote = {quote}: a rewritten item's quote must change"
        );
    }
}

/// The rows of the run's `<out>/ast-hash/<label>/levels-unstable.json`: one
/// per item or module whose judged field changed, or that lost its levels,
/// under a perturbation (L9). Only the count leaves this function.
fn unstable_rows(out: &Path, label: &str) -> usize {
    let path = out
        .join("ast-hash")
        .join(label)
        .join("levels-unstable.json");
    read_json(&path)
        .as_array()
        .map(Vec::len)
        .unwrap_or_else(|| panic!("levels-unstable.json under --out must be a JSON array"))
}

// ------------------------------------------------------------------ RD-1

/// RD-1: both commands carry every field of the Data on the fixtures, typed,
/// in their relations. A field dropped or renamed turns this red.
#[test]
fn rd1_levels_and_value_hash_carry_every_field_on_the_fixtures() {
    let scratch = Scratch::new("rd1");
    let (ast_hash, _) = run_fixture("ast-hash", &scratch.join("ast-hash"));
    let result = &ast_hash["result"];
    assert_eq!(
        result["detail"]["rustfmt"]["available"], true,
        "the fmt rows need rustfmt (or SPECENGINE_RUSTFMT)"
    );
    levels_shape(&result["levels"], true).check("ast-hash: result.levels");
    levels_invariants(result).check("ast-hash: result.levels relations");
    let (ron, _) = run_fixture("ron", &scratch.join("ron"));
    let value_hash = &ron["result"]["value_hash"];
    value_hash_shape(value_hash).check("ron: result.value_hash");
    value_hash_invariants(value_hash).check("ron: result.value_hash relations");
}

/// RD-1's mutation "`0` for an absent rustfmt": with a `SPECENGINE_RUSTFMT`
/// stub that fails `--version` (as `ast_hash_units.rs` stubs it), the
/// `fmt_*` rows of `stable_pct` and `module_stable_pct` are `null`, as v2's
/// are, and every other pin of `expected.json` holds (L3).
#[test]
fn levels_fmt_rows_are_null_without_rustfmt() {
    let scratch = Scratch::new("no-rustfmt");
    let log = scratch.join("rustfmt.log");
    let stub = scratch.join("rustfmt-stub.sh");
    fs::write(
        &stub,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\nexit 1\n",
            log.display()
        ),
    )
    .expect("stub written");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("stub executable");
    let output = eval()
        .env("SPECENGINE_RUSTFMT", &stub)
        .arg("ast-hash")
        .arg("--out")
        .arg(scratch.join("out"))
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output, "ast-hash without rustfmt");
    let result = &envelope["result"];
    // Premises: the harness asked the stub and found no rustfmt.
    let calls = fs::read_to_string(&log).unwrap_or_default();
    assert!(
        calls.lines().any(|call| call == "--version"),
        "the stub was never asked: {calls:?}"
    );
    assert_eq!(result["detail"]["rustfmt"]["available"], false);
    for row in FMT_ROWS {
        assert_eq!(
            result["stable_pct"][row],
            Value::Null,
            "v2's stable_pct.{row}"
        );
    }
    let levels = &result["levels"];
    levels_shape(levels, false).check("levels without rustfmt");
    for table in ["stable_pct", "module_stable_pct"] {
        for row in FMT_ROWS {
            assert_eq!(
                levels[table][row],
                Value::Null,
                "levels.{table}.{row} without rustfmt"
            );
        }
    }
    let expected = read_json(&fixture("ast-hash").join("expected.json"));
    let mut want = expected["levels"].clone();
    for table in ["stable_pct", "module_stable_pct"] {
        let rows = want[table]
            .as_object_mut()
            .unwrap_or_else(|| panic!("expected.json pins levels.{table}"));
        for row in FMT_ROWS {
            rows.remove(row);
        }
    }
    let mut problems = Problems::default();
    pinned(&want, levels, "levels", &mut problems);
    problems.check("the rows that need no rustfmt still measure");
}

/// RD-1 over refused files: `ron` over `fixtures/hash-levels/ron/` gives
/// `hashed` = the values 5a-1b's inventory lists for its clean files,
/// `cannot_verify` = the path comments of its refused files, `anchored`
/// their sum, every value stable on the CRLF copy, and, the directory
/// holding no marker, `bound` 0 with `bound_stable_pct` `null`. Mutations:
/// a refused file's comments dropped from `anchored`, or `0` for the `null`.
#[test]
fn value_hash_counts_refused_files_and_is_null_without_markers() {
    let corpus = fixture("hash-levels").join("ron");
    let before = snapshot(&corpus);
    let inventory = read_json(&fixture("hash-levels").join("expected.json"));
    let files = inventory["ron"]["files"]
        .as_object()
        .expect("5a-1b's ron.files in fixtures/hash-levels/expected.json");
    let (mut hashed, mut refused, mut refused_files) = (0, 0, 0);
    for (name, file) in files {
        let listed = |key: &str| {
            file[key]
                .as_array()
                .unwrap_or_else(|| panic!("{name}: {key} is a list"))
                .len() as u64
        };
        match file["state"].as_str() {
            Some("hashed") => hashed += listed("values"),
            Some("cannot_verify") => {
                refused += listed("path_comments");
                refused_files += 1;
            }
            other => panic!("{name}: state {other:?}"),
        }
    }
    let on_disk: BTreeSet<String> = fs::read_dir(&corpus)
        .expect("readable")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".ron"))
        .map(|name| format!("ron/{name}"))
        .collect();
    let listed: BTreeSet<String> = files.keys().cloned().collect();
    assert_eq!(
        listed, on_disk,
        "premise: the inventory lists every .ron file"
    );
    assert!(
        hashed > 0 && refused > 0 && refused_files >= 2,
        "premise: clean and refused files ({hashed} values, {refused} refused comments \
         in {refused_files} files)"
    );
    let scratch = Scratch::new("ron-refused");
    let output = eval()
        .arg("ron")
        .arg("--pilot")
        .arg(&corpus)
        .arg("--out")
        .arg(scratch.join("out"))
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output, "ron over fixtures/hash-levels/ron");
    let result = &envelope["result"];
    assert_eq!(result["markers"]["total"], 0, "premise: no marker");
    let value_hash = &result["value_hash"];
    value_hash_shape(value_hash).check("value_hash");
    assert_eq!(count(&value_hash["hashed"], "hashed"), hashed);
    assert_eq!(
        count(&value_hash["cannot_verify"], "cannot_verify"),
        refused
    );
    assert_eq!(count(&value_hash["anchored"], "anchored"), hashed + refused);
    assert_eq!(
        number(&value_hash["crlf_stable_pct"], "crlf_stable_pct"),
        100.0
    );
    assert_eq!(value_hash["bound"], 0);
    assert_eq!(value_hash["bound_stable_pct"], Value::Null);
    assert_eq!(snapshot(&corpus), before, "the fixture is read only");
}

/// The sweep on its edges, which `fixtures/ast-hash` lacks: six functions
/// whose `body_bytes` sit exactly on 64, 96, 124, 160 (twice, one body) and
/// 256 (`{ "x…x" }` is 4 bytes more than its string). An item at `t` is not
/// below it, so `trivial_at.<t>.items` counts the lower points only, and the
/// two 160-byte twins count up to 160. Mutations: `<=` in the sweep (every
/// count one up), twins counted across thresholds or kinds.
#[test]
fn trivial_at_puts_an_item_at_a_threshold_above_it() {
    let scratch = Scratch::new("thresholds");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(corpus.join("src")).expect("corpus");
    let function = |name: &str, body_bytes: usize, fill: char| {
        let text: String = std::iter::repeat_n(fill, body_bytes - 4).collect();
        format!("pub fn {name}() -> &'static str {{ \"{text}\" }}\n\n")
    };
    let source = [
        function("at_64", 64, 'a'),
        function("at_96", 96, 'b'),
        function("at_124", 124, 'c'),
        function("at_160_one", 160, 'd'),
        function("at_160_two", 160, 'd'),
        function("at_256", 256, 'e'),
    ]
    .concat();
    fs::write(corpus.join("src").join("lib.rs"), source).expect("lib.rs");
    let output = eval()
        .arg("ast-hash")
        .arg("--pilot")
        .arg(&corpus)
        .arg("--out")
        .arg(scratch.join("out"))
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output, "ast-hash over the threshold corpus");
    let levels = &envelope["result"]["levels"];
    assert_eq!(levels["hashed"], 6, "premise: six items with levels");
    let mut problems = Problems::default();
    pinned(
        &json!({
            "trivial": { "items": 2 },
            "trivial_at": {
                "64": { "items": 0, "body_twins": 2 },
                "96": { "items": 1, "body_twins": 2 },
                "124": { "items": 2, "body_twins": 2 },
                "160": { "items": 3, "body_twins": 2 },
                "256": { "items": 5, "body_twins": 0 },
            },
            "body_groups": { "groups": 1, "items": 2 },
            "skeleton_groups_in_file": { "groups": 1, "items": 6 },
            "quote_separates": 2,
        }),
        levels,
        "levels",
        &mut problems,
    );
    problems.check("the sweep on its edges");
    levels_invariants(&envelope["result"]).check("levels relations on the edges");
}

/// Edge data: a corpus without a `.rs` file still measures, with zero counts
/// and every `cost` field `null` (L15), never a panic or a rate per zero kLOC.
#[test]
fn an_empty_corpus_gives_zero_counts_and_null_cost() {
    let scratch = Scratch::new("empty");
    let corpus = scratch.join("corpus");
    fs::create_dir_all(&corpus).expect("empty corpus");
    let out = scratch.join("out");
    let output = eval()
        .arg("ast-hash")
        .arg("--pilot")
        .arg(&corpus)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(&output, "ast-hash over an empty corpus");
    let result = &envelope["result"];
    assert_eq!(result["items"], 0, "premise: nothing to measure");
    let levels = &result["levels"];
    levels_shape(levels, result["detail"]["rustfmt"]["available"] == true)
        .check("levels of an empty corpus");
    levels_invariants(result).check("levels relations of an empty corpus");
    for key in [
        "hashed",
        "cannot_verify",
        "modules.hashed",
        "modules.cannot_verify",
        "neighbour.edited",
        "neighbour.checked",
        "rename.renamed",
        "trivial.items",
    ] {
        assert_eq!(*at(levels, key), 0, "levels.{key}");
    }
    for key in ["levels_ms_per_kloc", "v2_ms_per_kloc"] {
        assert_eq!(levels["cost"][key], Value::Null, "levels.cost.{key}");
    }
    assert_eq!(unstable_rows(&out, "pilot"), 0, "levels-unstable.json");
    assert_eq!(
        fs::read_dir(&corpus).expect("readable").count(),
        0,
        "nothing written into the corpus"
    );
}

// ------------------------------------------------------------------ TE-2

/// TE-2 on `ast-hash`:`result.levels` gives every pin of
/// `fixtures/ast-hash/expected.json` exactly, `edited_changed` = `edited`,
/// and `fixtures/` is left as found (L8). Named mutations: a neighbour pass
/// skipped (`levels.neighbour`), a constant hash (`edited_changed` below
/// `edited`), the sweep off by one threshold (`levels.trivial_at`), a count
/// off by one (its pin).
#[test]
fn te2_ast_hash_levels_match_expected_json() {
    let fixture = fixture("ast-hash");
    let before = snapshot(&fixture);
    let status_before = git_status_of_fixtures();
    let scratch = Scratch::new("te2-ast-hash");
    let out = scratch.join("out");
    let (envelope, stdout) = run_fixture("ast-hash", &out);
    let result = &envelope["result"];
    assert_eq!(
        result["detail"]["rustfmt"]["available"], true,
        "TE-2 pins the fmt rows: install rustfmt (or set SPECENGINE_RUSTFMT)"
    );
    // The fixture holds `.rs` newlines: `cost` is timed (L15), never pinned.
    for key in ["levels_ms_per_kloc", "v2_ms_per_kloc"] {
        number(
            &result["levels"]["cost"][key],
            &format!("levels.cost.{key}"),
        );
    }
    // Every judged figure is exactly 100.0 here, so no row names an item.
    assert_eq!(unstable_rows(&out, "fixtures"), 0, "levels-unstable.json");
    let levels = &result["levels"];
    levels_shape(levels, true).check("result.levels");
    let expected = read_json(&fixture.join("expected.json"));
    let want = &expected["levels"];
    assert!(
        want.is_object(),
        "fixtures/ast-hash/expected.json pins `levels`"
    );
    let mut problems = Problems::default();
    pinned(want, levels, "levels", &mut problems);
    problems.check("result.levels against fixtures/ast-hash/expected.json");
    assert_eq!(
        levels["neighbour"]["edited_changed"], levels["neighbour"]["edited"],
        "non-vacuity: every edited item's sig changed"
    );
    assert_fmt_quote_rows_measured(levels);
    levels_invariants(result).check("result.levels relations");
    // Counts, percentages and times only: no file or item name.
    for name in [".rs", "summarize", "heavy_labels", "Counter", "tilde"] {
        assert!(!stdout.contains(name), "{name:?} reached stdout: {stdout}");
    }
    assert!(
        !stdout.contains(fixture.to_str().expect("a UTF-8 path")),
        "the fixture path reached stdout"
    );
    assert_eq!(snapshot(&fixture), before, "fixtures/ast-hash changed");
    assert_eq!(
        git_status_of_fixtures(),
        status_before,
        "git status -- fixtures/ changed"
    );
}

/// TE-2 on `ron`: `result.value_hash` gives every pin of
/// `fixtures/ron/expected.json` exactly and `fixtures/` is left as found.
/// Named mutations: markers only (`value_hash.anchored`), a marker's comment
/// stripped with the plain ones (`value_hash.bound`), a count off by one.
#[test]
fn te2_ron_value_hash_matches_expected_json() {
    let fixture = fixture("ron");
    let before = snapshot(&fixture);
    let status_before = git_status_of_fixtures();
    let scratch = Scratch::new("te2-ron");
    let (envelope, stdout) = run_fixture("ron", &scratch.join("out"));
    let value_hash = &envelope["result"]["value_hash"];
    value_hash_shape(value_hash).check("result.value_hash");
    let expected = read_json(&fixture.join("expected.json"));
    let want = &expected["value_hash"];
    assert!(
        want.is_object(),
        "fixtures/ron/expected.json pins `value_hash`"
    );
    let mut problems = Problems::default();
    pinned(want, value_hash, "value_hash", &mut problems);
    problems.check("result.value_hash against fixtures/ron/expected.json");
    value_hash_invariants(value_hash).check("result.value_hash relations");
    // Counts and percentages only: no file name, ID or path.
    for name in [".ron", "DEEP-MAP", "WATER", "root.player"] {
        assert!(!stdout.contains(name), "{name:?} reached stdout: {stdout}");
    }
    assert!(
        !stdout.contains(fixture.to_str().expect("a UTF-8 path")),
        "the fixture path reached stdout"
    );
    assert_eq!(snapshot(&fixture), before, "fixtures/ron changed");
    assert_eq!(
        git_status_of_fixtures(),
        status_before,
        "git status -- fixtures/ changed"
    );
}

/// The times of an envelope, the only part a repeat may change: `wall_ms`;
/// `ast-hash`: `detail.hash_ms`, `levels.cost` and, with the `syn` feature,
/// `syn.time_ratio`; `ron`: `detail.lexer_ms`.
fn strip_times(envelope: &mut Value, measurement: &str) {
    let envelope = envelope.as_object_mut().expect("an envelope");
    envelope.remove("wall_ms").expect("wall_ms");
    let result = envelope
        .get_mut("result")
        .and_then(Value::as_object_mut)
        .expect("result");
    let detail = result
        .get_mut("detail")
        .and_then(Value::as_object_mut)
        .expect("result.detail");
    if measurement == "ast-hash" {
        detail.remove("hash_ms").expect("detail.hash_ms");
        result
            .get_mut("levels")
            .and_then(Value::as_object_mut)
            .expect("result.levels")
            .remove("cost")
            .expect("levels.cost");
        if let Some(syn) = result.get_mut("syn").and_then(Value::as_object_mut) {
            syn.remove("time_ratio");
        }
    } else {
        detail.remove("lexer_ms").expect("detail.lexer_ms");
    }
}

/// TE-2 and the spec's rule on repeats: a second fixture run gives the same
/// JSON but its times (`wall_ms`, `detail.hash_ms`, `cost`).
#[test]
fn te2_a_repeat_run_differs_only_in_its_times() {
    let scratch = Scratch::new("repeat");
    for measurement in ["ast-hash", "ron"] {
        let (mut one, _) = run_fixture(measurement, &scratch.join(&format!("{measurement}-1")));
        let (mut two, _) = run_fixture(measurement, &scratch.join(&format!("{measurement}-2")));
        strip_times(&mut one, measurement);
        strip_times(&mut two, measurement);
        assert_eq!(
            one, two,
            "{measurement}: a repeat changed more than its times"
        );
    }
}

/// The pins keep the Data's relations and tell the named mutations apart,
/// without a run: a hand slip in `expected.json` shows here first.
#[test]
fn te2_expected_json_pins_keep_the_data_s_relations() {
    let ast_hash = read_json(&fixture("ast-hash").join("expected.json"));
    let levels = &ast_hash["levels"];
    // `cost` is never pinned and no relation reads it.
    let pinned_run = json!({
        "items": ast_hash["items"],
        "files": ast_hash["files"],
        "levels": levels,
    });
    levels_invariants(&pinned_run).check("fixtures/ast-hash/expected.json levels");
    assert_eq!(
        levels["neighbour"]["edited_changed"], levels["neighbour"]["edited"],
        "non-vacuity pinned"
    );
    for field in JUDGED {
        assert_eq!(
            levels["rename"]["kept"][field], levels["rename"]["renamed"],
            "a rename keeps {field}"
        );
    }
    // A shifted sweep is red only when every threshold counts differently.
    let items: Vec<u64> = THRESHOLDS
        .iter()
        .map(|threshold| {
            count(
                &levels["trivial_at"][threshold.to_string().as_str()]["items"],
                "trivial_at items",
            )
        })
        .collect();
    assert!(
        items.windows(2).all(|pair| pair[0] < pair[1]),
        "every threshold must count differently on the fixture: {items:?}"
    );
    // A skipped pass halves `checked`; a constant hash zeroes `edited_changed`.
    assert!(count(&levels["neighbour"]["checked"], "checked") > 0);
    assert!(count(&levels["neighbour"]["edited"], "edited") > 0);

    let ron = read_json(&fixture("ron").join("expected.json"));
    let value_hash = &ron["value_hash"];
    value_hash_invariants(value_hash).check("fixtures/ron/expected.json value_hash");
    let markers = count(&ron["markers"]["anchored"]["lexer"], "markers.anchored");
    assert!(
        count(&value_hash["anchored"], "anchored") > markers,
        "markers only must give another `anchored`"
    );
    assert!(
        count(&value_hash["bound"], "bound") > 0,
        "stripping the markers' comments must show in `bound`"
    );
}

// ------------------------------------------------------------------ TE-1
//
// Pilots A and B, manual: the owner's run, never CI; how to run it is the
// doc comment of `te1` below.

/// The harness's budget per measurement in a TE-1 run, seconds.
const TE1_TIMEOUT_S: &str = "1800";

/// An absolute toolchain binary for the TE-1 child (L10): `variable` when
/// set (an absolute path to a file), else `rustup which <tool>` run at `/`.
fn toolchain_binary(variable: &str, tool: &str) -> PathBuf {
    if let Some(value) = std::env::var_os(variable).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(value);
        assert!(
            path.is_absolute() && path.is_file(),
            "{variable} must name the toolchain's {tool} by absolute path, got {}",
            path.display()
        );
        return path;
    }
    let output = Command::new("rustup")
        .current_dir("/")
        .args(["which", tool])
        .output()
        .unwrap_or_else(|error| {
            panic!("set {variable} to the toolchain's {tool} (absolute path), or put rustup on PATH: {error}")
        });
    assert!(
        output.status.success(),
        "rustup which {tool}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    assert!(
        path.is_absolute() && path.is_file(),
        "rustup which {tool} gave {}",
        path.display()
    );
    path
}

/// One entry the harness walk can reach: corpus-relative path, kind, size,
/// mtime in nanoseconds.
type Entry = (PathBuf, &'static str, u64, i128);

/// TE-1's read-only proof of a pilot (L11).
#[derive(Debug, PartialEq, Eq)]
struct Proof {
    status: String,
    entries: BTreeSet<Entry>,
}

fn proof(corpus: &Path) -> Proof {
    let output = Command::new("git")
        .current_dir(corpus)
        .args([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=all",
        ])
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "the pilot must be a git worktree for the read-only proof: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut entries = BTreeSet::new();
    list(corpus, Path::new(""), &mut entries);
    Proof {
        status: String::from_utf8_lossy(&output.stdout).into_owned(),
        entries,
    }
}

/// The entries under `relative` the harness walk can reach: hidden and
/// `target*` directories skipped, hidden files it cannot read skipped,
/// symlinks listed and never followed.
fn list(corpus: &Path, relative: &Path, out: &mut BTreeSet<Entry>) {
    let Ok(children) = fs::read_dir(corpus.join(relative)) else {
        out.insert((relative.to_path_buf(), "unlistable", 0, 0));
        return;
    };
    for child in children {
        let child = child.expect("a directory entry");
        let name = child.file_name().to_string_lossy().into_owned();
        let path = relative.join(&name);
        let Ok(metadata) = fs::symlink_metadata(child.path()) else {
            out.insert((path, "missing", 0, 0));
            continue;
        };
        let kind = if metadata.file_type().is_symlink() {
            "symlink"
        } else if metadata.is_dir() {
            "dir"
        } else if metadata.is_file() {
            "file"
        } else {
            "other"
        };
        let hidden = name.starts_with('.');
        let readable = name.ends_with(".rs") || name.ends_with(".ron");
        let build = name == "target" || name.starts_with("target.");
        if (kind == "dir" && (hidden || build)) || (kind != "dir" && hidden && !readable) {
            continue;
        }
        let mtime =
            i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
        out.insert((path.clone(), kind, metadata.len(), mtime));
        if kind == "dir" {
            list(corpus, &path, out);
        }
    }
}

/// One TE-1 child: `specengine-eval <measurement> --label <label> --timeout
/// <TE1_TIMEOUT_S> --out <out>` with only `PATH`, `HOME` (also its working
/// directory), the label's corpus variable and the toolchain paths (L10).
struct Child<'a> {
    label: &'a str,
    variable: &'a str,
    corpus: &'a Path,
    home: &'a Path,
    out: &'a Path,
    rustfmt: &'a Path,
    cargo: &'a Path,
}

impl Child<'_> {
    fn run(&self, measurement: &str) -> (Value, String) {
        let mut command = Command::new(BIN);
        command.env_clear();
        if let Some(path) = std::env::var_os("PATH") {
            command.env("PATH", path);
        }
        let output = command
            .current_dir(self.home)
            .env("HOME", self.home)
            .env(self.variable, self.corpus)
            .env("SPECENGINE_RUSTFMT", self.rustfmt)
            .env("SPECENGINE_CARGO", self.cargo)
            .args([
                measurement,
                "--label",
                self.label,
                "--timeout",
                TE1_TIMEOUT_S,
            ])
            .arg("--out")
            .arg(self.out)
            .output()
            .expect("specengine-eval runs");
        let context = format!("{} {measurement}", self.label);
        let envelope = envelope(&output, &context);
        assert_eq!(envelope["label"], self.label, "{context}");
        (
            envelope,
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }
}

/// The judged figures below 100.0 (L9), one line each; from
/// docs/features/hash-levels-use-lists.md (5a-1c, Q3) the module figures
/// `levels.module_stable_pct.<p>.<f>` are judged too.
fn shortfalls(levels: &Value, value_hash: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for row in PERTURBATIONS {
        for field in JUDGED {
            let value = &levels["stable_pct"][row][field];
            if value.as_f64() != Some(100.0) {
                out.push(format!("levels.stable_pct.{row}.{field} = {value}"));
            }
        }
    }
    for row in MODULE_PERTURBATIONS {
        for field in MODULE_FIELDS {
            let value = &levels["module_stable_pct"][row][field];
            if value.as_f64() != Some(100.0) {
                out.push(format!("levels.module_stable_pct.{row}.{field} = {value}"));
            }
        }
    }
    let renamed = &levels["rename"]["renamed"];
    for field in JUDGED {
        let kept = &levels["rename"]["kept"][field];
        if kept != renamed {
            out.push(format!(
                "levels.rename.kept.{field} = {kept} of {renamed} renamed"
            ));
        }
    }
    for key in ["crlf_stable_pct", "bound_stable_pct"] {
        let value = &value_hash[key];
        if !value.is_null() && value.as_f64() != Some(100.0) {
            out.push(format!("value_hash.{key} = {value}"));
        }
    }
    out
}

/// Counts and times only: what the owner records in the spec.
fn print_figures(label: &str, ast_hash: &Value, ron: &Value) {
    let result = &ast_hash["result"];
    let levels = &result["levels"];
    eprintln!(
        "TE-1 {label} ast-hash: wall_ms {}, files {}, items {}, {}",
        ast_hash["wall_ms"],
        result["files"],
        result["items"],
        result["detail"]["rustfmt"]["version"]
    );
    eprintln!(
        "TE-1 {label} levels:\n{}",
        serde_json::to_string_pretty(levels).expect("JSON")
    );
    let cost = number(
        &levels["cost"]["levels_ms_per_kloc"],
        "levels.cost.levels_ms_per_kloc",
    );
    let verdict = if cost > 6.67 {
        "above: 6c is planned at once"
    } else {
        "within"
    };
    eprintln!(
        "TE-1 {label} cost.levels_ms_per_kloc {cost} against 08 AC-10's 6.67 ms per kLOC: {verdict}"
    );
    eprintln!(
        "TE-1 {label} ron: wall_ms {}, value_hash {}",
        ron["wall_ms"], ron["result"]["value_hash"]
    );
}

/// TE-1 over the pilot of `label`: pilots A and B, manual, the owner's run,
/// never CI. Release, in the foreground under a self-terminating timeout,
/// one pilot at a time (`cost` is a timing); a scratch `HOME` the test
/// makes; the child given only `PATH`, that `HOME`, the label's corpus
/// variable and `SPECENGINE_RUSTFMT`, `SPECENGINE_CARGO` as absolute
/// toolchain paths (L10): rustup's proxies find no toolchain under a scratch
/// `HOME`. Two steps from the owner's own shell (nothing here names a
/// pilot): the release build first, outside the alarm, then the run under
/// it, which covers 4 children of `--timeout 1800` each plus a margin:
///
/// ```text
/// cargo nextest run --release -p specengine-eval --test hash_levels_eval --no-run
/// SPECENGINE_PILOT_A=… SPECENGINE_PILOT_B=… perl -e 'alarm 7800; exec @ARGV' \
///   cargo nextest run --release -p specengine-eval --test hash_levels_eval \
///   te1_pilot --run-ignored only --no-capture --test-threads 1
/// ```
///
/// It fails, never skips: an unset variable (named), a debug build, a run
/// without rustfmt, a changed read-only proof (L11), a broken relation, and,
/// after every figure is printed, a judged figure below 100.0 or a row in
/// `levels-unstable.json` (L9). The figures go into the spec's
/// Implementation by hand, counts only.
fn te1(label: &str) {
    if cfg!(debug_assertions) {
        panic!(
            "TE-1 counts only in release (`cost` is a release figure): cargo nextest run --release"
        );
    }
    let variable = pilot::variables(label).corpus;
    let corpus = pilot::required(variable, "the pilot's worktree");
    let corpus = fs::canonicalize(corpus).unwrap_or_else(|error| panic!("{variable}: {error}"));
    let rustfmt = toolchain_binary("SPECENGINE_RUSTFMT", "rustfmt");
    let cargo = toolchain_binary("SPECENGINE_CARGO", "cargo");
    let scratch = Scratch::new(label);
    let home = scratch.join("home");
    fs::create_dir_all(&home).expect("the scratch HOME");
    let out = scratch.join("out");
    assert!(
        !scratch.0.starts_with(&corpus) && !corpus.starts_with(&scratch.0),
        "{label}: the scratch directory and the pilot must not nest"
    );
    let before = proof(&corpus);
    let child = Child {
        label,
        variable,
        corpus: &corpus,
        home: &home,
        out: &out,
        rustfmt: &rustfmt,
        cargo: &cargo,
    };
    let (ast_hash, ast_hash_stdout) = child.run("ast-hash");
    let (ron, ron_stdout) = child.run("ron");
    let after = proof(&corpus);
    let changed: Vec<&Entry> = before
        .entries
        .symmetric_difference(&after.entries)
        .take(20)
        .collect();
    assert!(
        before == after,
        "{label}: the read-only proof changed: git status equal {}, entries differing \
         (first 20): {changed:?}",
        before.status == after.status
    );
    let left_in_home: Vec<String> = fs::read_dir(&home)
        .expect("the scratch HOME")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    eprintln!(
        "TE-1 {label} read-only proof: {} git status lines and {} entries, equal before and \
         after; left under the scratch HOME: {left_in_home:?}",
        before.status.lines().count(),
        before.entries.len()
    );

    let name = corpus
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path = corpus.to_str().expect("a UTF-8 pilot path");
    for (stdout, extension) in [(&ast_hash_stdout, ".rs"), (&ron_stdout, ".ron")] {
        assert!(
            !stdout.contains(path),
            "{label}: the pilot path reached stdout"
        );
        assert!(
            name.is_empty() || !stdout.contains(&name),
            "{label}: the pilot name reached stdout"
        );
        assert!(
            !stdout.contains(extension),
            "{label}: a file name reached stdout"
        );
    }

    let result = &ast_hash["result"];
    assert!(
        result.is_object(),
        "{label}: ast-hash must finish under --timeout {TE1_TIMEOUT_S}, got {result}"
    );
    assert_eq!(
        result["detail"]["rustfmt"]["available"],
        true,
        "{label}: a run counts only with rustfmt available ({})",
        rustfmt.display()
    );
    let levels = &result["levels"];
    levels_shape(levels, true).check(&format!("{label}: result.levels"));
    levels_invariants(result).check(&format!("{label}: result.levels relations"));
    assert!(
        count(&levels["hashed"], "levels.hashed") >= 1000,
        "{label}: sanity floor, 1000 items with levels"
    );
    let ron_result = &ron["result"];
    assert!(
        ron_result.is_object(),
        "{label}: ron must finish under --timeout {TE1_TIMEOUT_S}, got {ron_result}"
    );
    let value_hash = &ron_result["value_hash"];
    value_hash_shape(value_hash).check(&format!("{label}: result.value_hash"));
    value_hash_invariants(value_hash).check(&format!("{label}: result.value_hash relations"));

    print_figures(label, &ast_hash, &ron);
    // A rounded 100.0 can hide up to 0.05 % changed items; a row cannot (L9).
    // Only the count is printed: the rows name files and items.
    let unstable = unstable_rows(&out, label);
    eprintln!("TE-1 {label} levels-unstable.json: {unstable} rows");
    let mut shortfalls = shortfalls(levels, value_hash);
    if unstable > 0 {
        shortfalls.push(format!(
            "levels-unstable.json: {unstable} rows (an item or module changed a judged field)"
        ));
    }
    assert!(
        shortfalls.is_empty(),
        "{label}: judged figures below 100.0 (a 5a-1 or 5a-1b recipe change before 5b, \
         ADR-0050, or an exception the owner accepts in the spec):\n{}",
        shortfalls.join("\n")
    );
}

#[test]
#[ignore = "TE-1: needs SPECENGINE_PILOT_A; manual, release, read-only run over a real corpus"]
fn te1_pilot_a() {
    te1("pilot-a");
}

#[test]
#[ignore = "TE-1: needs SPECENGINE_PILOT_B; manual, release, read-only run over a real corpus"]
fn te1_pilot_b() {
    te1("pilot-b");
}

// ------------------------------------------------- 5a-1c TE-7, TE-10 (N6)
//
// docs/features/hash-levels-use-lists.md: the module figures are judged
// (Q3), and the fixture `fixtures/hash-levels/src/` (its `use_lists.rs`
// lists out of rustfmt's order) leaves no row in `levels-unstable.json`.

/// `result.levels` with every judged figure at 100.0 and `renamed` kept in
/// every field: what a clean TE-1 run prints.
fn clean_levels() -> Value {
    let mut stable = serde_json::Map::new();
    for row in PERTURBATIONS {
        let fields: serde_json::Map<String, Value> = FIELDS
            .iter()
            .map(|field| ((*field).to_owned(), json!(100.0)))
            .collect();
        stable.insert(row.to_owned(), Value::Object(fields));
    }
    let mut modules = serde_json::Map::new();
    for row in MODULE_PERTURBATIONS {
        let fields: serde_json::Map<String, Value> = MODULE_FIELDS
            .iter()
            .map(|field| ((*field).to_owned(), json!(100.0)))
            .collect();
        modules.insert(row.to_owned(), Value::Object(fields));
    }
    let kept: serde_json::Map<String, Value> = FIELDS
        .iter()
        .map(|field| ((*field).to_owned(), json!(7)))
        .collect();
    json!({
        "stable_pct": stable,
        "module_stable_pct": modules,
        "rename": { "renamed": 7, "kept": kept },
    })
}

/// TE-7 (Q3): `shortfalls` names every module figure below 100.0, and only
/// those; a clean run has none. M7 the module loop removed.
#[test]
fn shortfalls_name_the_module_figures_below_100() {
    let value_hash = json!({ "crlf_stable_pct": 100.0, "bound_stable_pct": null });
    assert_eq!(
        shortfalls(&clean_levels(), &value_hash),
        Vec::<String>::new(),
        "a clean run"
    );
    for row in MODULE_PERTURBATIONS {
        for field in MODULE_FIELDS {
            for (value, shown) in [(json!(79.6), "79.6"), (Value::Null, "null")] {
                let mut levels = clean_levels();
                levels["module_stable_pct"][row][field] = value;
                assert_eq!(
                    shortfalls(&levels, &value_hash),
                    vec![format!("levels.module_stable_pct.{row}.{field} = {shown}")],
                    "module_stable_pct.{row}.{field} at {shown}"
                );
            }
        }
    }
    // pilot A's first TE-1 run (79.6 % module bodies under both rustfmt
    // passes): two shortfalls, the item figures clean.
    let mut levels = clean_levels();
    for row in FMT_ROWS {
        levels["module_stable_pct"][row]["body"] = json!(79.6);
    }
    assert_eq!(
        shortfalls(&levels, &value_hash),
        vec![
            "levels.module_stable_pct.fmt_default.body = 79.6".to_owned(),
            "levels.module_stable_pct.fmt_contrast.body = 79.6".to_owned(),
        ]
    );
}

/// TE-10: `ast-hash --pilot` over a scratch copy of
/// `fixtures/hash-levels/src/` (unsorted `use` lists at file level, in
/// function bodies, inline mods and a method) writes zero rows in
/// `levels-unstable.json`, module rows included: every item and module
/// field judged holds under both rustfmt passes, comment stripping and the
/// neighbour edit. Needs rustfmt (`SPECENGINE_RUSTFMT` or `rustup which`).
/// M1 N6 removed.
#[test]
#[ignore = "TE-10 (5a-1c): needs rustfmt; cargo nextest run -p specengine-eval --test hash_levels_eval te10 --run-ignored only"]
fn te10_hash_levels_sources_leave_no_unstable_row() {
    let rustfmt = toolchain_binary("SPECENGINE_RUSTFMT", "rustfmt");
    let scratch = Scratch::new("te10");
    let corpus = scratch.join("corpus");
    let source = fixture("hash-levels").join("src");
    fs::create_dir_all(&corpus).expect("the scratch corpus");
    let mut files = 0;
    for entry in fs::read_dir(&source).expect("fixtures/hash-levels/src") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            fs::copy(&path, corpus.join(path.file_name().expect("a name"))).expect("copy");
            files += 1;
        }
    }
    assert!(
        corpus.join("use_lists.rs").is_file(),
        "the N6 fixture file is in the corpus"
    );
    let out = scratch.join("out");
    let output = eval()
        .env("SPECENGINE_RUSTFMT", &rustfmt)
        .args(["ast-hash", "--pilot"])
        .arg(&corpus)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("specengine-eval runs");
    let envelope = envelope(
        &output,
        "ast-hash --pilot <copy of fixtures/hash-levels/src>",
    );
    let result = &envelope["result"];
    assert_eq!(
        result["detail"]["rustfmt"]["available"],
        true,
        "TE-10 counts only with rustfmt ({})",
        rustfmt.display()
    );
    let levels = &result["levels"];
    assert_eq!(
        count(&levels["modules"]["hashed"], "levels.modules.hashed"),
        files,
        "every file module hashed"
    );
    assert!(
        count(&levels["hashed"], "levels.hashed") >= 50,
        "the fixture's items hashed: {}",
        levels["hashed"]
    );
    let mut judged = Vec::new();
    for row in MODULE_PERTURBATIONS {
        for field in MODULE_FIELDS {
            let value = &levels["module_stable_pct"][row][field];
            if value.as_f64() != Some(100.0) {
                judged.push(format!("levels.module_stable_pct.{row}.{field} = {value}"));
            }
        }
    }
    for row in PERTURBATIONS {
        for field in JUDGED {
            let value = &levels["stable_pct"][row][field];
            if value.as_f64() != Some(100.0) {
                judged.push(format!("levels.stable_pct.{row}.{field} = {value}"));
            }
        }
    }
    let rows = unstable_rows(&out, "pilot");
    assert!(
        rows == 0 && judged.is_empty(),
        "levels-unstable.json: {rows} rows; judged figures below 100.0: {judged:?}\n{}",
        fs::read_to_string(
            out.join("ast-hash")
                .join("pilot")
                .join("levels-unstable.json")
        )
        .unwrap_or_default()
    );
}
