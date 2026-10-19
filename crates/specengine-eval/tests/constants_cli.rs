//! docs/features/constant-census.md (12a) RD-2, RD-3, RD-4, TE-1, TE-2 and
//! TE-3: `specengine-eval constants` end to end on the real binary.
//!
//! - TE-1: the two invented conventions `fixtures/code-consts/one` (the
//!   default corpus) and `fixtures/code-consts/two` (`--pilot`): each run's
//!   `result` without `constants_ms` and every row of `constants.json` and
//!   `skipped.json` equal the fixture's `expected.json`, derived by hand
//!   from the spec, never read back from a run.
//! - RD-2 (matching): small corpora written here, one rule each, under the
//!   invented convention of `MINI_CONFIG`.
//! - RD-3 (files), RD-4 (read-only, deterministic, counts only on stdout,
//!   every key), TE-2 (two absolute roots, another cwd, reverse creation,
//!   CRLF; a mode-000 file), TE-3 (the fixtures keep every byte, size and
//!   mtime; `git status -- fixtures/` unchanged).
//!
//! Each test names the mutation that turns it red. Nothing here names a
//! pilot; TE-4 is `constants_pilot.rs`. PHASE-B: the subcommand, its stdout
//! keys and its two detail files are the spec's; the readings the
//! expectations rest on are L-tagged in the slice's test report.

mod import_support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

use import_support::*;

/// The two invented conventions (D5), under `fixtures/`.
const CONSTANTS_FIXTURES: [&str; 2] = ["code-consts/one", "code-consts/two"];

/// Every leaf of `result`, as `/`-separated paths, sorted.
const RESULT_LEAVES: [&str; 19] = [
    "ambiguous/two_items",
    "ambiguous/two_records",
    "constants_ms",
    "files",
    "hyphenless",
    "items",
    "marked",
    "matched/alias",
    "matched/id",
    "not_valued/broken",
    "not_valued/byte_string",
    "not_valued/escaped",
    "not_valued/macro",
    "not_valued/multi_line",
    "not_valued/other",
    "roots_missing",
    "skipped",
    "unmatched",
    "valued",
];

/// The six `not_valued` categories, in the order they are tried.
const NOT_VALUED: [&str; 6] = [
    "broken",
    "macro",
    "byte_string",
    "escaped",
    "multi_line",
    "other",
];

/// Each `match` of a row and the `result` count it adds to.
const MATCHES: [(&str, &str); 6] = [
    ("id", "matched/id"),
    ("alias", "matched/alias"),
    ("hyphenless", "hyphenless"),
    ("unmatched", "unmatched"),
    ("two_records", "ambiguous/two_records"),
    ("two_items", "ambiguous/two_items"),
];

/// The keys of a `constants.json` row, sorted.
const ROW_KEYS: [&str; 11] = [
    "cfg",
    "item_kind",
    "line",
    "marked",
    "match",
    "mod_path",
    "name",
    "not_valued",
    "path",
    "records",
    "value",
];

// ------------------------------------------------------------ plumbing

/// One successful `constants` run.
struct Run {
    envelope: Value,
    stdout: String,
    /// `<out>/constants/<label>`.
    detail: PathBuf,
}

impl Run {
    fn result(&self) -> &Value {
        &self.envelope["result"]
    }

    fn rows(&self) -> Vec<Value> {
        array(&self.detail.join("constants.json"))
    }

    fn skipped(&self) -> Vec<Value> {
        array(&self.detail.join("skipped.json"))
    }

    /// Every file of the detail directory with its bytes.
    fn detail_files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        snapshot(&self.detail)
    }
}

fn array(path: &Path) -> Vec<Value> {
    read_json(path)
        .as_array()
        .unwrap_or_else(|| panic!("{} is a JSON array", path.display()))
        .clone()
}

/// `specengine-eval constants <args>` from `cwd`, no pilot variable inherited.
fn constants_in(cwd: &Path, args: &[&str]) -> Output {
    let mut command = eval();
    command.current_dir(cwd).arg("constants").args(args);
    command.output().expect("specengine-eval runs")
}

/// A successful run from `cwd`: `--out <out>`, `--pilot <corpus>` unless
/// `None` (the default fixture), `--label` when given, then `extra`.
fn measure_in(
    cwd: &Path,
    corpus: Option<&str>,
    out: &Path,
    label: Option<&str>,
    extra: &[&str],
) -> Run {
    let out_text = out.to_str().expect("a UTF-8 --out").to_owned();
    let mut args: Vec<&str> = vec!["--out", out_text.as_str()];
    if let Some(corpus) = corpus {
        args.extend(["--pilot", corpus]);
    }
    if let Some(label) = label {
        args.extend(["--label", label]);
    }
    args.extend_from_slice(extra);
    let output = constants_in(cwd, &args);
    let envelope = envelope(&output);
    assert_eq!(envelope["measurement"], "constants", "{envelope}");
    let label = envelope["label"].as_str().expect("a label").to_owned();
    Run {
        stdout: String::from_utf8(output.stdout).expect("UTF-8 stdout"),
        detail: out.join("constants").join(label),
        envelope,
    }
}

/// A run over `corpus` (absolute) from the repository root.
fn measure_at(corpus: &Path, out: &Path) -> Run {
    measure_in(
        &repository_root(),
        Some(corpus.to_str().expect("a UTF-8 corpus")),
        out,
        None,
        &[],
    )
}

fn expected(fixture: &str) -> Value {
    read_json(&fixture_dir(fixture).join("expected.json"))
}

fn expected_array(fixture: &str, key: &str) -> Vec<Value> {
    expected(fixture)[key]
        .as_array()
        .unwrap_or_else(|| panic!("{fixture}: expected.json {key} is an array"))
        .clone()
}

/// The count at a `/`-separated path of `result`.
fn count(result: &Value, path: &str) -> u64 {
    let mut value = result;
    for part in path.split('/') {
        value = &value[part];
    }
    value
        .as_u64()
        .unwrap_or_else(|| panic!("result/{path} is not a count: {result}"))
}

/// Every leaf of `value` with its `/`-separated path, sorted by path.
fn leaves(value: &Value) -> Vec<(String, Value)> {
    fn walk(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}/{key}")
                    };
                    walk(inner, &path, out);
                }
            }
            other => out.push((prefix.to_owned(), other.clone())),
        }
    }
    let mut out = Vec::new();
    walk(value, "", &mut out);
    let sorted: BTreeMap<String, Value> = out.into_iter().collect();
    sorted.into_iter().collect()
}

/// The spec's invariants over `result`, and what the detail must keep with
/// it: one row per item with the row keys, the per-row matches and
/// categories summing to the counts, `marked` = valued rows with a marker,
/// one `skipped.json` row per skipped file and per missing root (reading
/// L13), rows by path bytes then line, skipped rows by path bytes.
fn assert_invariants(result: &Value, rows: &[Value], skipped: &[Value], context: &str) {
    let n = |path: &str| count(result, path);
    let not_valued: u64 = NOT_VALUED
        .iter()
        .map(|category| n(&format!("not_valued/{category}")))
        .sum();
    assert_eq!(
        n("items"),
        n("valued") + not_valued,
        "{context}: items = valued + not_valued: {result}"
    );
    let classified: u64 = MATCHES.iter().map(|(_, path)| n(path)).sum();
    assert_eq!(
        n("valued"),
        classified,
        "{context}: valued = matched + hyphenless + unmatched + ambiguous: {result}"
    );
    assert!(n("marked") <= n("valued"), "{context}: marked <= valued");
    assert!(n("skipped") <= n("files"), "{context}: skipped <= files");
    assert_ne!(n("ambiguous/two_items"), 1, "{context}: two_items != 1");

    assert_eq!(rows.len() as u64, n("items"), "{context}: one row per item");
    for row in rows {
        let mut keys: Vec<&str> = row
            .as_object()
            .unwrap_or_else(|| panic!("{context}: a row is an object: {row}"))
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ROW_KEYS, "{context}: row keys of {row}");
        assert_eq!(
            row["value"].is_string(),
            row["match"].is_string(),
            "{context}: a row has a value exactly when it has a match: {row}"
        );
        assert_eq!(
            row["value"].is_null(),
            row["not_valued"].is_string(),
            "{context}: a row has no value exactly when it has a category: {row}"
        );
    }
    for category in NOT_VALUED {
        let rows_of = rows
            .iter()
            .filter(|row| row["not_valued"] == category)
            .count() as u64;
        assert_eq!(
            rows_of,
            n(&format!("not_valued/{category}")),
            "{context}: rows not_valued {category}"
        );
    }
    for (name, path) in MATCHES {
        let rows_of = rows.iter().filter(|row| row["match"] == name).count() as u64;
        assert_eq!(rows_of, n(path), "{context}: rows matched {name}");
    }
    let marked = rows
        .iter()
        .filter(|row| {
            row["value"].is_string() && row["marked"].as_array().is_some_and(|ids| !ids.is_empty())
        })
        .count() as u64;
    assert_eq!(marked, n("marked"), "{context}: valued rows with a marker");
    assert_eq!(
        skipped.len() as u64,
        n("skipped") + n("roots_missing"),
        "{context}: skipped.json holds the skipped files and the missing roots"
    );
    let missing = skipped
        .iter()
        .filter(|row| row["cause"] == "root_missing")
        .count() as u64;
    assert_eq!(missing, n("roots_missing"), "{context}: root_missing rows");
    let order: Vec<(Vec<u8>, u64)> = rows
        .iter()
        .map(|row| {
            (
                row["path"].as_str().expect("a path").as_bytes().to_vec(),
                row["line"].as_u64().expect("a line"),
            )
        })
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] <= pair[1]),
        "{context}: rows by path bytes, then line"
    );
    let skipped_order: Vec<&[u8]> = skipped
        .iter()
        .map(|row| row["path"].as_str().expect("a path").as_bytes())
        .collect();
    assert!(
        skipped_order.windows(2).all(|pair| pair[0] <= pair[1]),
        "{context}: skipped rows by path bytes"
    );
}

/// Row by row, naming the first difference.
fn assert_rows_equal(got: &[Value], want: &[Value], context: &str) {
    for (index, (got, want)) in got.iter().zip(want).enumerate() {
        assert_eq!(got, want, "{context}: row {index} ({})", want["name"]);
    }
    assert_eq!(got.len(), want.len(), "{context}: number of rows");
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("parent directory");
    fs::write(path, bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn to_crlf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 16);
    for &byte in bytes {
        if byte == b'\n' {
            out.push(b'\r');
        }
        out.push(byte);
    }
    out
}

/// Copies `from` to `to` byte for byte, hidden and build directories
/// included (the fixtures hold no symlink), each file created in path
/// order or, with `reverse`, in reverse path order; with `crlf` each LF of
/// a `.rs` or `.md` file becomes CRLF.
fn copy_tree(from: &Path, to: &Path, reverse: bool, crlf: bool) {
    let mut files: Vec<(PathBuf, Vec<u8>)> = snapshot(from).into_iter().collect();
    if reverse {
        files.reverse();
    }
    for (relative, bytes) in files {
        let lines = crlf
            && relative
                .extension()
                .is_some_and(|extension| extension == "rs" || extension == "md");
        let bytes = if lines { to_crlf(&bytes) } else { bytes };
        write(&to.join(&relative), bytes);
    }
}

/// A scratch copy of `fixture` at `scratch/<name>` whose `census.toml` has
/// its first `from` replaced by `to` (the original must hold `from`).
fn edited_copy(scratch: &Scratch, fixture: &str, name: &str, from: &str, to: &str) -> PathBuf {
    let copy = scratch.join(name);
    copy_tree(&fixture_dir(fixture), &copy, false, false);
    let config = copy.join("census.toml");
    let text = fs::read_to_string(&config).expect("the copied census config");
    assert!(
        text.contains(from),
        "{fixture}'s census.toml holds {from:?}"
    );
    fs::write(&config, text.replacen(from, to, 1)).expect("the edited config");
    copy
}

/// (size, mtime s, mtime ns) of every entry under `dir`, directories
/// included, symlinks not followed.
fn stat_listing(dir: &Path) -> BTreeMap<PathBuf, (u64, i64, i64)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current).expect("readable directory") {
            let path = entry.expect("directory entry").path();
            let metadata = fs::symlink_metadata(&path).expect("metadata");
            out.insert(
                path.strip_prefix(dir).expect("under dir").to_path_buf(),
                (metadata.len(), metadata.mtime(), metadata.mtime_nsec()),
            );
            if metadata.is_dir() {
                stack.push(path);
            }
        }
    }
    out
}

/// `git status --porcelain --untracked-files=all -- fixtures/`.
fn fixtures_status() -> String {
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
    assert!(output.status.success(), "{}", stderr(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

// ------------------------------------------------------- RD-2 corpora

/// The invented convention of the RD-2 corpora: `NU` rules in `Key`
/// tables, `OL` the legacy prefix of `NU`, `FE` unique per document, two
/// hyphenless patterns (the second an alternation whose leftmost match can
/// be the shorter one), `notes/index.md` a document that only refers.
const MINI_CONFIG: &str = r#"[corpus]
roots = ["notes"]
[ids]
regex = '^(?P<prefix>[A-Z]{2})-[0-9]{2}$'
feature_prefixes = ["FE"]
hyphenless = ['H[0-9]{3}', 'K[0-9]{2}|K[0-9]{3}']
[ids.legacy]
"OL" = "NU"
[tables]
id_header = '^Key$'
[definitions]
reference_paths = ["notes/index.md"]
[code]
roots = ["src"]
"#;

/// The records of the RD-2 corpora: NU-01, NU-02, NU-03 (written OL-03),
/// NU-04 (defined twice: an `Import.duplicates` entry) and FE-01 (once in
/// each of two feature documents: two records).
const MINI_DOCS: [(&str, &str); 3] = [
    (
        "notes/rules.md",
        "# Rules\n\n| Key | Text |\n|---|---|\n| NU-01 | One. |\n| NU-02 | Two. |\n\
         | OL-03 | Three, under the legacy prefix. |\n| NU-04 | Four. |\n\
         | NU-04 | Four, defined again. |\n",
    ),
    (
        "notes/feature-a.md",
        "# A\n\n| Key | Text |\n|---|---|\n| FE-01 | A criterion of A. |\n",
    ),
    (
        "notes/feature-b.md",
        "# B\n\n| Key | Text |\n|---|---|\n| FE-01 | A criterion of B. |\n",
    ),
];

/// A corpus at `scratch/<name>`: `MINI_CONFIG`, `MINI_DOCS`, then `extra`.
fn mini_corpus(scratch: &Scratch, name: &str, extra: &[(&str, &str)]) -> PathBuf {
    let root = scratch.join(name);
    write(&root.join("census.toml"), MINI_CONFIG);
    for (path, text) in MINI_DOCS.iter().chain(extra) {
        write(&root.join(path), text);
    }
    root
}

/// The rows of one run over `corpus` (`--out` beside it), the invariants
/// checked.
fn mini_rows(corpus: &Path) -> Vec<Value> {
    let run = measure_at(corpus, &corpus.with_extension("out"));
    let rows = run.rows();
    assert_invariants(
        run.result(),
        &rows,
        &run.skipped(),
        &corpus.display().to_string(),
    );
    rows
}

/// A row's outcome: its `match`, or `not_valued <category>`.
fn outcome(row: &Value) -> String {
    match row["match"].as_str() {
        Some(name) => name.to_owned(),
        None => format!(
            "not_valued {}",
            row["not_valued"].as_str().expect("a category")
        ),
    }
}

/// Each name's rows read `outcome`, and the rows name exactly `expected`'s
/// names.
fn assert_outcomes(rows: &[Value], expected: &[(&str, &str)], context: &str) {
    let mut got: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        got.entry(row["name"].as_str().expect("a name").to_owned())
            .or_default()
            .insert(outcome(row));
    }
    let want: BTreeMap<String, BTreeSet<String>> = expected
        .iter()
        .map(|(name, outcome)| ((*name).to_owned(), BTreeSet::from([(*outcome).to_owned()])))
        .collect();
    assert_eq!(got, want, "{context}");
}

/// The `records` of the first row named `name`.
fn records_of(rows: &[Value], name: &str) -> Vec<String> {
    rows.iter()
        .find(|row| row["name"] == name)
        .unwrap_or_else(|| panic!("a row named {name}"))["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|id| id.as_str().expect("an ID").to_owned())
        .collect()
}

// ------------------------------------------------------- TE-1 fixtures

/// TE-1. M: any rule of the spec's Data changed. Both conventions: the
/// default corpus (no `--pilot`, label `fixtures`) and `--pilot
/// fixtures/code-consts/two` relative to the working directory (label
/// `pilot`): `result` without `constants_ms` and every row of both detail
/// files equal `expected.json`; the detail directory holds those two files
/// only; the invariants hold.
#[test]
fn fixture_runs_match_expected_json() {
    let scratch = Scratch::new("constants-fixtures");
    let root = repository_root();
    let runs = [
        (
            "code-consts/one",
            "fixtures",
            measure_in(&root, None, &scratch.join("one"), None, &[]),
        ),
        (
            "code-consts/two",
            "pilot",
            measure_in(
                &root,
                Some("fixtures/code-consts/two"),
                &scratch.join("two"),
                None,
                &[],
            ),
        ),
    ];
    for (fixture, label, run) in &runs {
        assert_eq!(run.envelope["label"], *label, "{fixture}");
        let expected = expected(fixture);
        assert_eq!(
            without_ms(run.result()),
            expected["result"],
            "{fixture}: result without constants_ms"
        );
        assert!(
            run.result()["constants_ms"].is_u64(),
            "{fixture}: constants_ms is a count"
        );
        let rows = run.rows();
        assert_rows_equal(
            &rows,
            expected["constants"].as_array().expect("rows"),
            fixture,
        );
        assert_eq!(
            run.skipped(),
            *expected["skipped"].as_array().expect("skipped rows"),
            "{fixture}: skipped.json"
        );
        let mut files: Vec<String> = fs::read_dir(&run.detail)
            .expect("the detail directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        files.sort();
        assert_eq!(files, ["constants.json", "skipped.json"], "{fixture}");
        assert_invariants(run.result(), &rows, &run.skipped(), fixture);
    }
}

/// The source line a row names.
fn source_line(fixture: &str, row: &Value) -> String {
    let path = fixture_dir(fixture).join(row["path"].as_str().expect("a path"));
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let line = row["line"].as_u64().expect("a line") as usize;
    text.lines()
        .nth(line - 1)
        .unwrap_or_else(|| panic!("{}:{line} exists", path.display()))
        .to_owned()
}

/// Names written in each fixture that no row may carry: enum variants,
/// associated and fn-body consts, and items of excluded, build, hidden,
/// unlisted and non-UTF-8 files.
const NEVER_READ: [(&str, [&str; 8]); 2] = [
    (
        "code-consts/one",
        [
            "Frost",
            "FROST_CODE",
            "INNER",
            "STALE_PRESSURE",
            "CACHED_PRESSURE",
            "GENERATED_WINDOW",
            "NOTE_PRESSURE",
            "LATIN_PRESSURE",
        ],
    ),
    (
        "code-consts/two",
        [
            "Hoist",
            "CODE",
            "INNER",
            "OLD_HOOK",
            "BUILT_HOOK",
            "BACKUP_HOOK",
            "DRUM_HOOK",
            "TOWER_HOOK",
        ],
    ),
];

/// TE-1, the pair itself (no run). M: a fixture row dropped. Each
/// `expected.json` holds a static, an inline-mod const, cfg twins of one
/// value, a cross-file pair of one value (both `two_items`), an alias, a
/// hyphenless, an unmatched and a broken row, two `two_records` rows and a
/// marked valued row, each row naming its source line; across the pair a
/// raw literal, `concat!`, a call and every `not_valued` category; each
/// name of `NEVER_READ` is written in its fixture and has no row; both
/// skip a non-UTF-8 file and a missing root.
#[test]
fn the_fixtures_hold_every_te1_row() {
    let mut categories = BTreeSet::new();
    let (mut raw, mut concat, mut call) = (false, false, false);
    for (fixture, never_read) in NEVER_READ {
        let rows = expected_array(fixture, "constants");
        let with =
            |predicate: &dyn Fn(&Value) -> bool| rows.iter().filter(|row| predicate(row)).count();
        assert!(
            with(&|row| row["item_kind"] == "static_item" && row["value"].is_string()) > 0,
            "{fixture}: a valued static"
        );
        assert!(
            with(&|row| row["mod_path"]
                .as_array()
                .is_some_and(|mods| !mods.is_empty()))
                > 0,
            "{fixture}: an inline-mod const"
        );
        let pairs: Vec<(&Value, &Value)> = rows
            .iter()
            .enumerate()
            .flat_map(|(index, a)| rows[index + 1..].iter().map(move |b| (a, b)))
            .filter(|(a, b)| {
                a["value"].is_string()
                    && a["value"] == b["value"]
                    && a["match"] == "two_items"
                    && b["match"] == "two_items"
            })
            .collect();
        assert!(
            pairs.iter().any(|(a, b)| a["path"] == b["path"]
                && a["name"] == b["name"]
                && a["cfg"] != b["cfg"]),
            "{fixture}: cfg twins of one value"
        );
        assert!(
            pairs.iter().any(|(a, b)| a["path"] != b["path"]),
            "{fixture}: a cross-file pair of one value"
        );
        for wanted in ["id", "alias", "hyphenless", "unmatched"] {
            assert!(
                with(&|row| row["match"] == wanted) > 0,
                "{fixture}: {wanted}"
            );
        }
        assert!(
            with(&|row| row["match"] == "two_records") >= 2,
            "{fixture}: a duplicated ID's value and one naming two records"
        );
        assert!(
            with(&|row| row["not_valued"] == "broken") > 0,
            "{fixture}: a broken item"
        );
        assert!(
            with(&|row| row["value"].is_string()
                && row["marked"].as_array().is_some_and(|ids| !ids.is_empty()))
                > 0,
            "{fixture}: a marked constant"
        );
        for row in &rows {
            let line = source_line(fixture, row);
            assert!(
                line.contains(row["name"].as_str().expect("a name")),
                "{fixture}: row {row} names its line {line:?}"
            );
            if let Some(category) = row["not_valued"].as_str() {
                categories.insert(category.to_owned());
            }
            if row["not_valued"] == "macro" {
                concat |= line.contains("concat!(");
                call |= !line.contains('!') && line.contains("()");
            }
            if row["value"].is_string() && (line.contains("= r\"") || line.contains("= r#\"")) {
                raw = true;
            }
        }
        let sources = snapshot(&fixture_dir(fixture));
        for name in never_read {
            assert!(
                sources.values().any(|bytes| bytes
                    .windows(name.len())
                    .any(|window| window == name.as_bytes())),
                "{fixture}: {name} is written in the fixture"
            );
            assert!(
                rows.iter().all(|row| row["name"] != name),
                "{fixture}: {name} has no row"
            );
        }
        let causes: BTreeSet<String> = expected_array(fixture, "skipped")
            .iter()
            .map(|row| row["cause"].as_str().expect("a cause").to_owned())
            .collect();
        assert!(
            causes.contains("not_utf8") && causes.contains("root_missing"),
            "{fixture}: {causes:?}"
        );
    }
    let all: BTreeSet<String> = NOT_VALUED.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(categories, all, "every not_valued category across the pair");
    assert!(raw, "a raw literal valued");
    assert!(concat, "a concat! row");
    assert!(call, "a call row");
}

/// TE-1's two non-UTF-8 files, written as bytes (Phase B's recipe in the
/// slice's test report): each is not UTF-8 by one Latin-1 byte in a
/// comment, and holds a `const` whose value an item of the fixture holds,
/// so reading it lossily would turn that item `two_items`.
#[test]
fn the_non_utf8_fixture_files_are_not_utf8() {
    for (path, value) in [
        ("code-consts/one/firmware/latin1.rs", "\"VLV-001\""),
        ("code-consts/two/panel/latin1.rs", "\"CR-0001\""),
    ] {
        let file = repository_root().join("fixtures").join(path);
        let bytes = fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        assert!(
            String::from_utf8(bytes.clone()).is_err(),
            "{path} is not UTF-8"
        );
        assert_eq!(
            bytes.iter().filter(|byte| !byte.is_ascii()).count(),
            1,
            "{path}: one non-ASCII byte"
        );
        let lossy = String::from_utf8_lossy(&bytes);
        assert!(
            lossy
                .lines()
                .next()
                .is_some_and(|first| first.starts_with("//")),
            "{path}: the byte is in the opening comment"
        );
        assert!(
            lossy.contains("pub const ") && lossy.contains(value),
            "{path}: a const holding {value}"
        );
    }
}

// --------------------------------------------------------- RD-2 match

/// M: a substring, prefix, case-folded or trimmed comparison. Only the
/// whole value, byte for byte, names a record or an alias.
#[test]
fn partial_value_never_matches() {
    let scratch = Scratch::new("constants-partial");
    let corpus = mini_corpus(
        &scratch,
        "corpus",
        &[(
            "src/lib.rs",
            "pub const EXACT: &str = \"NU-01\";\n\
             pub const PREFIX: &str = \"NU-0\";\n\
             pub const LONGER: &str = \"NU-011\";\n\
             pub const INSIDE: &str = \"xNU-01x\";\n\
             pub const LOWER: &str = \"nu-01\";\n\
             pub const LEADING: &str = \" NU-01\";\n\
             pub const TRAILING: &str = \"NU-01 \";\n\
             pub const ALIAS_EXACT: &str = \"OL-03\";\n\
             pub const ALIAS_PREFIX: &str = \"OL-0\";\n\
             pub const ALIAS_LOWER: &str = \"ol-03\";\n\
             pub const HYPHENLESS_LOWER: &str = \"h123\";\n",
        )],
    );
    let rows = mini_rows(&corpus);
    assert_outcomes(
        &rows,
        &[
            ("EXACT", "id"),
            ("PREFIX", "unmatched"),
            ("LONGER", "unmatched"),
            ("INSIDE", "unmatched"),
            ("LOWER", "unmatched"),
            ("LEADING", "unmatched"),
            ("TRAILING", "unmatched"),
            ("ALIAS_EXACT", "alias"),
            ("ALIAS_PREFIX", "unmatched"),
            ("ALIAS_LOWER", "unmatched"),
            ("HYPHENLESS_LOWER", "unmatched"),
        ],
        "partial values",
    );
    assert_eq!(records_of(&rows, "EXACT"), ["NU-01"]);
    assert_eq!(
        records_of(&rows, "ALIAS_EXACT"),
        ["NU-03"],
        "an alias names its record's ID"
    );
    assert!(records_of(&rows, "PREFIX").is_empty());
}

const TWO_ITEMS_A: &str = "pub const A_TWO: &str = \"NU-02\";\n\
                           pub const A_ALIAS: &str = \"OL-03\";\n\
                           pub const A_HYPHENLESS: &str = \"H123\";\n\
                           pub const A_NOTHING: &str = \"nothing here\";\n\
                           pub const A_DUPLICATED: &str = \"NU-04\";\n";
const TWO_ITEMS_B: &str = "pub const B_TWO: &str = \"NU-02\";\n\
                           pub const B_ALIAS: &str = \"OL-03\";\n\
                           pub const B_HYPHENLESS: &str = \"H123\";\n\
                           pub const B_NOTHING: &str = \"nothing here\";\n\
                           pub const B_DUPLICATED: &str = \"NU-04\";\n";
const CFG_TWINS: &str = "#[cfg(unix)]\npub const TWIN: &str = \"NU-01\";\n\n\
                         #[cfg(not(unix))]\npub const TWIN: &str = \"NU-01\";\n";

/// M: first by path wins. A value two valued items hold reads
/// `two_items` on both, whether the twin is a cfg twin or in another file,
/// whether it would match by `id`, `alias` or `hyphenless`; a shared value
/// naming nothing stays `unmatched`, one naming two records `two_records`
/// (rule 1 first). Swapping the two files' contents changes no outcome.
#[test]
fn one_value_on_two_items_is_ambiguous() {
    let scratch = Scratch::new("constants-two-items");
    let expected = [
        ("A_TWO", "two_items"),
        ("B_TWO", "two_items"),
        ("A_ALIAS", "two_items"),
        ("B_ALIAS", "two_items"),
        ("A_HYPHENLESS", "two_items"),
        ("B_HYPHENLESS", "two_items"),
        ("A_NOTHING", "unmatched"),
        ("B_NOTHING", "unmatched"),
        ("A_DUPLICATED", "two_records"),
        ("B_DUPLICATED", "two_records"),
        ("TWIN", "two_items"),
    ];
    for (name, first, second) in [
        ("forward", TWO_ITEMS_A, TWO_ITEMS_B),
        ("swapped", TWO_ITEMS_B, TWO_ITEMS_A),
    ] {
        let corpus = mini_corpus(
            &scratch,
            name,
            &[
                ("src/a.rs", first),
                ("src/b.rs", second),
                ("src/c.rs", CFG_TWINS),
            ],
        );
        let rows = mini_rows(&corpus);
        assert_outcomes(&rows, &expected, name);
        assert_eq!(
            rows.iter().filter(|row| row["name"] == "TWIN").count(),
            2,
            "{name}: both cfg twins have a row"
        );
        assert_eq!(
            records_of(&rows, "A_TWO"),
            ["NU-02"],
            "{name} (reading L12)"
        );
        assert_eq!(
            records_of(&rows, "B_ALIAS"),
            ["NU-03"],
            "{name} (reading L12)"
        );
        assert!(records_of(&rows, "A_HYPHENLESS").is_empty(), "{name}");
    }
}

/// M: first record wins. A value naming two records (one ID defined once
/// in each of two feature documents) reads `two_records`, never `id`; a
/// value naming one record beside it reads `id`.
#[test]
fn one_value_naming_two_records_is_ambiguous() {
    let scratch = Scratch::new("constants-two-records");
    let corpus = mini_corpus(
        &scratch,
        "corpus",
        &[(
            "src/lib.rs",
            "pub const CRITERION: &str = \"FE-01\";\npub const SINGLE: &str = \"NU-02\";\n",
        )],
    );
    let rows = mini_rows(&corpus);
    assert_outcomes(
        &rows,
        &[("CRITERION", "two_records"), ("SINGLE", "id")],
        "two records",
    );
    assert_eq!(
        records_of(&rows, "CRITERION"),
        ["FE-01"],
        "the sorted distinct IDs"
    );
}

/// D7. M: `Import.duplicates` unread. An ID defined twice reads
/// `two_records`, never `id`. (If the implementation counts every
/// definition `Import.records` holds, which keeps the second one too, the
/// mutation is equivalent: reading L14, the report's A3 note.)
#[test]
fn duplicate_definition_reads_two_records() {
    let scratch = Scratch::new("constants-duplicate");
    let corpus = mini_corpus(
        &scratch,
        "corpus",
        &[(
            "src/lib.rs",
            "pub const FOUR: &str = \"NU-04\";\npub const ONE: &str = \"NU-01\";\n",
        )],
    );
    let rows = mini_rows(&corpus);
    assert_outcomes(
        &rows,
        &[("FOUR", "two_records"), ("ONE", "id")],
        "duplicates",
    );
    assert_eq!(records_of(&rows, "FOUR"), ["NU-04"]);
}

/// Rules 1 and 3 count definitions only (F1, the owner's 2026-10-19;
/// reading L14). M: every `Import.records` entry counted, references too.
/// An ID defined once and listed again in a document that only refers
/// (`[definitions] reference_paths`) reads `id`; an ID that document only
/// refers to, defined nowhere, names no record: `unmatched`, `records`
/// empty.
#[test]
fn a_reference_row_is_no_second_definition() {
    let scratch = Scratch::new("constants-reference");
    let corpus = mini_corpus(
        &scratch,
        "corpus",
        &[
            (
                "notes/index.md",
                "# Index\n\n| Key | Where |\n|---|---|\n| NU-01 | rules |\n| NU-02 | rules |\n\
                 | NU-09 | nowhere |\n",
            ),
            (
                "src/lib.rs",
                "pub const ONE: &str = \"NU-01\";\npub const TWO_A: &str = \"NU-02\";\n\
                 pub const ONLY_REFERENCED: &str = \"NU-09\";\n",
            ),
            ("src/more.rs", "pub const TWO_B: &str = \"NU-02\";\n"),
        ],
    );
    let rows = mini_rows(&corpus);
    assert_outcomes(
        &rows,
        &[
            ("ONE", "id"),
            ("TWO_A", "two_items"),
            ("TWO_B", "two_items"),
            ("ONLY_REFERENCED", "unmatched"),
        ],
        "references",
    );
    assert!(
        records_of(&rows, "ONLY_REFERENCED").is_empty(),
        "a reference names no record"
    );
}

/// M: `is_match` or an unanchored regex (or one anchored with `^…$`). A
/// hyphenless pattern counts only when its leftmost match (`Regex::find`,
/// import's `is_hyphenless` restated) spans the whole value: a match on a
/// prefix of the value, one not at its start, and an alternation whose
/// leftmost-first match is the shorter branch all read `unmatched`.
#[test]
fn hyphenless_matches_the_whole_value_only() {
    let scratch = Scratch::new("constants-hyphenless");
    let corpus = mini_corpus(
        &scratch,
        "corpus",
        &[(
            "src/lib.rs",
            "pub const WHOLE: &str = \"H123\";\n\
             pub const LONGER: &str = \"H1234\";\n\
             pub const OFFSET: &str = \"xH123\";\n\
             pub const SHORT_BRANCH: &str = \"K12\";\n\
             pub const LONG_BRANCH: &str = \"K123\";\n",
        )],
    );
    let rows = mini_rows(&corpus);
    assert_outcomes(
        &rows,
        &[
            ("WHOLE", "hyphenless"),
            ("LONGER", "unmatched"),
            ("OFFSET", "unmatched"),
            ("SHORT_BRANCH", "hyphenless"),
            ("LONG_BRANCH", "unmatched"),
        ],
        "hyphenless",
    );
    assert!(
        records_of(&rows, "WHOLE").is_empty(),
        "hyphenless names no record"
    );
}

// --------------------------------------------------------- RD-3 files

/// M: `[code] extensions` applied. They narrow the import's code scan,
/// never the census: one given `extensions = ["py"]` and two without its
/// `["txt"]` read exactly as their fixtures.
#[test]
fn extensions_never_narrow_constants() {
    let scratch = Scratch::new("constants-extensions");
    for (fixture, from, to) in [
        (
            "code-consts/one",
            "[code]\n",
            "[code]\nextensions = [\"py\"]\n",
        ),
        ("code-consts/two", "extensions = [\"txt\"]\n", ""),
    ] {
        let name = fixture.replace('/', "-");
        let copy = edited_copy(&scratch, fixture, &name, from, to);
        let run = measure_at(&copy, &scratch.join(&format!("{name}-out")));
        let expected = expected(fixture);
        assert_eq!(
            without_ms(run.result()),
            expected["result"],
            "{fixture}: {from:?} -> {to:?}"
        );
        assert_rows_equal(
            &run.rows(),
            expected["constants"].as_array().expect("rows"),
            fixture,
        );
    }
}

/// M: `[code] exclude` ignored. An excluded file is not listed: with the
/// glob, a run over a copy of each fixture counts the fixture's `files` and
/// no row names it; with the glob removed it is read (`files` + 1, its row
/// appears) and the value it shares turns its twin `two_items`, so the
/// glob, not chance, keeps it out.
#[test]
fn excluded_file_is_not_read() {
    let scratch = Scratch::new("constants-exclude");
    for (fixture, line, excluded, twin) in [
        (
            "code-consts/one",
            "exclude = [\"firmware/generated/**\"]\n",
            "firmware/generated/table.rs",
            "VALVE_WINDOW",
        ),
        (
            "code-consts/two",
            "exclude = [\"panel/legacy_*.rs\"]\n",
            "panel/legacy_codes.rs",
            "HOOK_SPEED",
        ),
    ] {
        let expected = expected(fixture);
        assert!(fixture_dir(fixture).join(excluded).is_file(), "{excluded}");
        assert!(
            expected["constants"]
                .as_array()
                .expect("rows")
                .iter()
                .all(|row| row["path"] != excluded),
            "{fixture}: no row names {excluded}"
        );
        let name = fixture.replace('/', "-");
        let kept = scratch.join(&format!("{name}-kept"));
        copy_tree(&fixture_dir(fixture), &kept, false, false);
        let with_glob = measure_at(&kept, &scratch.join(&format!("{name}-kept-out")));
        assert_eq!(
            count(with_glob.result(), "files"),
            count(&expected["result"], "files"),
            "{fixture}: with the glob"
        );
        assert!(
            with_glob.rows().iter().all(|row| row["path"] != excluded),
            "{fixture}: with the glob no row names {excluded}"
        );
        let copy = edited_copy(&scratch, fixture, &name, line, "");
        let run = measure_at(&copy, &scratch.join(&format!("{name}-out")));
        assert_eq!(
            count(run.result(), "files"),
            count(&expected["result"], "files") + 1,
            "{fixture}: without the glob"
        );
        let rows = run.rows();
        assert!(
            rows.iter().any(|row| row["path"] == excluded),
            "{fixture}: without the glob {excluded} is read"
        );
        let twin_rows: Vec<String> = rows
            .iter()
            .filter(|row| row["name"] == twin)
            .map(outcome)
            .collect();
        assert_eq!(twin_rows, ["two_items"], "{fixture}: {twin}");
    }
}

/// M: a `target*` or hidden directory entered, or a symlink followed. The
/// fixtures' `target/`, `target.old/`, `.cache/` and `.backup/` copies
/// exist and have no row; a scratch copy of one with a symlinked directory
/// and a symlinked file, and a `target.noindex/` directory, each holding a
/// value an item holds, reads exactly as the fixture.
#[test]
fn build_and_hidden_directories_are_skipped() {
    for (fixture, directories) in [
        ("code-consts/one", ["firmware/target/", "firmware/.cache/"]),
        ("code-consts/two", ["panel/target.old/", "panel/.backup/"]),
    ] {
        let rows = expected_array(fixture, "constants");
        for directory in directories {
            assert!(fixture_dir(fixture).join(directory).is_dir(), "{directory}");
            assert!(
                rows.iter()
                    .all(|row| !row["path"].as_str().expect("a path").starts_with(directory)),
                "{fixture}: no row under {directory}"
            );
        }
    }
    let scratch = Scratch::new("constants-skipped-directories");
    let copy = scratch.join("one");
    copy_tree(&fixture_dir("code-consts/one"), &copy, false, false);
    let elsewhere = scratch.join("elsewhere");
    write(
        &elsewhere.join("linked.rs"),
        "pub const LINKED_PRESSURE: &str = \"VLV-001\";\n",
    );
    std::os::unix::fs::symlink(&elsewhere, copy.join("firmware/linked")).expect("a dir symlink");
    std::os::unix::fs::symlink(
        copy.join("firmware/codes.rs"),
        copy.join("firmware/again.rs"),
    )
    .expect("a file symlink");
    write(
        &copy.join("firmware/target.noindex/built.rs"),
        "pub const BUILT_PRESSURE: &str = \"VLV-001\";\n",
    );
    let run = measure_at(&copy, &scratch.join("out"));
    let expected = expected("code-consts/one");
    assert_eq!(without_ms(run.result()), expected["result"]);
    assert_rows_equal(
        &run.rows(),
        expected["constants"].as_array().expect("rows"),
        "symlinks and target.noindex",
    );
}

/// F2 (the owner's 2026-10-19): only `target` and `target.*` are build
/// directories (`harness::rust_files`); a `targets/` directory is entered.
/// M: every name starting `target` skipped. A file added under
/// `firmware/targets/` of a copy of fixture one is listed and read: one
/// more file, item, valued and unmatched, and its row.
#[test]
fn a_targets_directory_is_entered() {
    let scratch = Scratch::new("constants-targets");
    let copy = scratch.join("one");
    copy_tree(&fixture_dir("code-consts/one"), &copy, false, false);
    write(
        &copy.join("firmware/targets/kept.rs"),
        "pub const KEPT_LABEL: &str = \"kept here\";\n",
    );
    let run = measure_at(&copy, &scratch.join("out"));
    let expected = expected("code-consts/one");
    for key in ["files", "items", "valued", "unmatched"] {
        assert_eq!(
            count(run.result(), key),
            count(&expected["result"], key) + 1,
            "{key}"
        );
    }
    let rows: Vec<Value> = run
        .rows()
        .into_iter()
        .filter(|row| row["path"] == "firmware/targets/kept.rs")
        .collect();
    assert_outcomes(&rows, &[("KEPT_LABEL", "unmatched")], "targets/");
}

/// M: files counted per root. A file under two roots (nested directory
/// roots in one; a directory and a file root in two) is listed, read and
/// counted once: each (path, line) has one row, its unique value reads
/// `id`; the roots in another order change nothing.
#[test]
fn file_under_two_roots_is_read_once() {
    for (fixture, shared, rows_in_shared, unique) in [
        (
            "code-consts/one",
            "firmware/zones/north.rs",
            2,
            "NORTH_VALVE",
        ),
        ("code-consts/two", "panel/lifts.rs", 22, "HOOK_SPEED"),
    ] {
        let rows = expected_array(fixture, "constants");
        let places: BTreeSet<(String, u64)> = rows
            .iter()
            .map(|row| {
                (
                    row["path"].as_str().expect("a path").to_owned(),
                    row["line"].as_u64().expect("a line"),
                )
            })
            .collect();
        assert_eq!(places.len(), rows.len(), "{fixture}: one row per place");
        assert_eq!(
            rows.iter().filter(|row| row["path"] == shared).count(),
            rows_in_shared,
            "{fixture}: {shared}"
        );
        assert_outcomes(
            &rows
                .iter()
                .filter(|row| row["name"] == unique)
                .cloned()
                .collect::<Vec<_>>(),
            &[(unique, "id")],
            fixture,
        );
    }
    let scratch = Scratch::new("constants-two-roots");
    for (fixture, from, to) in [
        (
            "code-consts/one",
            "roots = [\"firmware\", \"firmware/zones\", \"drivers\"]",
            "roots = [\"drivers\", \"firmware/zones\", \"firmware\"]",
        ),
        (
            "code-consts/two",
            "roots = [\"panel\", \"winch/limits.rs\", \"panel/lifts.rs\", \"spare\"]",
            "roots = [\"panel/lifts.rs\", \"spare\", \"winch/limits.rs\", \"panel\"]",
        ),
    ] {
        let name = fixture.replace('/', "-");
        let copy = edited_copy(&scratch, fixture, &name, from, to);
        let run = measure_at(&copy, &scratch.join(&format!("{name}-out")));
        let expected = expected(fixture);
        assert_eq!(
            without_ms(run.result()),
            expected["result"],
            "{fixture}: roots reordered"
        );
        assert_rows_equal(
            &run.rows(),
            expected["constants"].as_array().expect("rows"),
            fixture,
        );
        assert_eq!(
            run.skipped(),
            *expected["skipped"].as_array().expect("skipped rows"),
            "{fixture}: roots reordered"
        );
    }
}

/// M: a missing root fatal (or dropped silently). A root that cannot be
/// read is counted and listed `root_missing` while the rest is measured
/// (both fixtures); a corpus whose only code root is missing measures
/// nothing, exit 0, every count present and zero but `roots_missing`.
#[test]
fn missing_root_is_counted() {
    for fixture in CONSTANTS_FIXTURES {
        let expected = expected(fixture);
        assert_eq!(count(&expected["result"], "roots_missing"), 1, "{fixture}");
    }
    let scratch = Scratch::new("constants-missing-root");
    let corpus = mini_corpus(&scratch, "corpus", &[]);
    let run = measure_at(&corpus, &scratch.join("out"));
    for (path, value) in leaves(&without_ms(run.result())) {
        let want = u64::from(path == "roots_missing");
        assert_eq!(value, want, "result/{path}");
    }
    assert!(run.rows().is_empty(), "no rows");
    assert_eq!(
        run.skipped(),
        [json!({"path": "src", "cause": "root_missing"})]
    );
}

/// SW refinement. M: empty roots measured. A census config with no `[code]
/// roots` (no `[code]` table, an empty list, a table with an `exclude`
/// only; at the corpus root or by `--config`) is refused before anything is
/// written: exit 2, stdout empty, `--out` never created, stderr naming the
/// config and `[code] roots is empty`.
#[test]
fn config_without_code_roots_is_exit_2() {
    let scratch = Scratch::new("constants-no-roots");
    let roots_line = "[code]\nroots = [\"src\"]\n";
    assert!(MINI_CONFIG.contains(roots_line));
    for (name, code, by_flag) in [
        ("absent", "", false),
        ("empty", "[code]\nroots = []\n", false),
        ("exclude-only", "[code]\nexclude = [\"src/**\"]\n", false),
        ("flag", "[code]\nroots = []\n", true),
    ] {
        let corpus = scratch.join(name);
        for (path, text) in MINI_DOCS {
            write(&corpus.join(path), text);
        }
        write(
            &corpus.join("src/lib.rs"),
            "pub const ONE: &str = \"NU-01\";\n",
        );
        let config_text = MINI_CONFIG.replace(roots_line, code);
        let config = if by_flag {
            write(&corpus.join("census.toml"), MINI_CONFIG);
            scratch.join("no-roots.toml")
        } else {
            corpus.join("census.toml")
        };
        write(&config, &config_text);
        let out = scratch.join(&format!("{name}-out"));
        let corpus_arg = corpus.to_str().expect("UTF-8");
        let out_arg = out.to_str().expect("UTF-8");
        let config_arg = config.to_str().expect("UTF-8");
        let mut args = vec!["--pilot", corpus_arg, "--out", out_arg];
        if by_flag {
            args.extend(["--config", config_arg]);
        }
        let output = constants_in(&repository_root(), &args);
        let message = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{name}: {message}");
        assert!(output.stdout.is_empty(), "{name}: stdout empty");
        assert!(!out.exists(), "{name}: --out is not created");
        let file_name = config.file_name().expect("a name").to_string_lossy();
        assert!(
            message.contains("[code] roots is empty") && message.contains(&*file_name),
            "{name}: stderr names {file_name} and the empty roots:\n{message}"
        );
    }
}

// --------------------------------------------- RD-4 read-only, stable

/// RD-4, TE-3. M: a file written under the corpus. The default run and
/// `--pilot` runs over both fixtures leave every entry of each fixture
/// with its bytes, size and mtime (bytes compared in full: the eval tests
/// have no BLAKE3, and no manifest may gain one, CR-1), nothing added or
/// removed, and `git status --porcelain --untracked-files=all --
/// fixtures/` unchanged.
#[test]
fn constants_leaves_the_corpus_untouched() {
    let status = fixtures_status();
    let before: Vec<_> = CONSTANTS_FIXTURES
        .iter()
        .map(|fixture| {
            let dir = fixture_dir(fixture);
            (snapshot(&dir), stat_listing(&dir))
        })
        .collect();
    let scratch = Scratch::new("constants-untouched");
    measure_in(
        &repository_root(),
        None,
        &scratch.join("default"),
        None,
        &[],
    );
    for fixture in CONSTANTS_FIXTURES {
        measure_at(
            &fixture_dir(fixture),
            &scratch.join(&fixture.replace('/', "-")),
        );
    }
    for (fixture, (bytes, stats)) in CONSTANTS_FIXTURES.iter().zip(&before) {
        let dir = fixture_dir(fixture);
        assert!(
            snapshot(&dir) == *bytes,
            "{fixture}: a file's bytes changed"
        );
        assert_eq!(stat_listing(&dir), *stats, "{fixture}: size or mtime");
    }
    assert_eq!(fixtures_status(), status, "git status -- fixtures/");
}

/// RD-4, TE-2. M: walk order (or the absolute root, the cwd, line ends)
/// reaching the output. Per fixture four runs under one label: the fixture
/// from the repository root; a copy under another absolute root, run from
/// the scratch directory; a copy created in reverse path order, one level
/// deeper, given relative to its parent as the cwd; a copy whose `.rs` and
/// `.md` files end lines with CRLF. `result` but `constants_ms` equals the
/// fixture's and both detail files are byte-identical across the four.
#[test]
fn constants_is_byte_identical_across_roots_and_order() {
    let scratch = Scratch::new("constants-identical");
    for fixture in CONSTANTS_FIXTURES {
        let name = fixture.replace('/', "-");
        let original = fixture_dir(fixture);
        let plain = scratch.join(&format!("a/{name}"));
        copy_tree(&original, &plain, false, false);
        let reversed_parent = scratch.join("b/deeper");
        copy_tree(&original, &reversed_parent.join(&name), true, false);
        let crlf = scratch.join(&format!("c/{name}"));
        copy_tree(&original, &crlf, false, true);
        assert!(
            snapshot(&crlf) != snapshot(&original),
            "{fixture}: the CRLF copy differs in bytes"
        );
        let label = Some("same");
        let out = |index: usize| scratch.join(&format!("out-{name}-{index}"));
        let runs = [
            measure_in(
                &repository_root(),
                Some(original.to_str().expect("UTF-8")),
                &out(0),
                label,
                &[],
            ),
            measure_in(
                &scratch.0,
                Some(plain.to_str().expect("UTF-8")),
                &out(1),
                label,
                &[],
            ),
            measure_in(&reversed_parent, Some(name.as_str()), &out(2), label, &[]),
            measure_in(
                &scratch.join("c"),
                Some(crlf.to_str().expect("UTF-8")),
                &out(3),
                label,
                &[],
            ),
        ];
        let first = &runs[0];
        assert_eq!(
            without_ms(first.result()),
            expected(fixture)["result"],
            "{fixture}"
        );
        let first_files = first.detail_files();
        assert_eq!(
            first_files.keys().cloned().collect::<Vec<_>>(),
            [
                PathBuf::from("constants.json"),
                PathBuf::from("skipped.json")
            ],
            "{fixture}"
        );
        for (index, run) in runs.iter().enumerate().skip(1) {
            assert_eq!(
                without_ms(run.result()),
                without_ms(first.result()),
                "{fixture}: run {index}"
            );
            for (file, bytes) in run.detail_files() {
                assert!(
                    first_files.get(&file) == Some(&bytes),
                    "{fixture}: run {index}: {} differs",
                    file.display()
                );
            }
        }
    }
}

/// RD-4 (eval README "Output"). M: a path, name, ID or value on stdout.
/// The envelope holds exactly `measurement`, `label`, `versions`,
/// `wall_ms` and `result`; every leaf of `result` is a count; no path, item
/// name, value, record or marker ID, mod or cfg text of the detail, nor the
/// corpus root, appears anywhere on stdout.
#[test]
fn constants_stdout_holds_counts_only() {
    let scratch = Scratch::new("constants-stdout");
    for fixture in CONSTANTS_FIXTURES {
        let corpus = fixture_dir(fixture);
        let run = measure_at(&corpus, &scratch.join(&fixture.replace('/', "-")));
        let keys: BTreeSet<&str> = run
            .envelope
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["label", "measurement", "result", "versions", "wall_ms"]),
            "{fixture}"
        );
        for (path, value) in leaves(run.result()) {
            assert!(
                value.is_u64(),
                "{fixture}: result/{path} is a count: {value}"
            );
        }
        let mut strings = BTreeSet::new();
        for row in run.rows() {
            for key in ["path", "name", "value"] {
                if let Some(text) = row[key].as_str() {
                    strings.insert(text.to_owned());
                }
            }
            for key in ["records", "marked", "mod_path", "cfg"] {
                for item in row[key].as_array().expect("a list") {
                    strings.insert(item.as_str().expect("a string").to_owned());
                }
            }
        }
        for row in run.skipped() {
            strings.insert(row["path"].as_str().expect("a path").to_owned());
        }
        strings.insert(corpus.display().to_string());
        assert!(
            strings.len() > 40,
            "{fixture}: {} strings checked",
            strings.len()
        );
        let leaked: Vec<&String> = strings
            .iter()
            .filter(|text| text.len() >= 3 && run.stdout.contains(text.as_str()))
            .collect();
        assert!(
            leaked.is_empty(),
            "{fixture}: stdout holds {leaked:?}:\n{}",
            run.stdout
        );
    }
}

/// RD-4. M: a zero key dropped (or a key added). `result` has exactly the
/// spec's keys, nested as its stdout shows them, zeros too: both fixtures
/// and a corpus whose only code root is missing (zero everywhere but
/// `roots_missing`).
#[test]
fn constants_result_has_every_key() {
    let scratch = Scratch::new("constants-keys");
    let zero = mini_corpus(&scratch, "zero", &[]);
    let corpora = [
        fixture_dir("code-consts/one"),
        fixture_dir("code-consts/two"),
        zero,
    ];
    for (index, corpus) in corpora.iter().enumerate() {
        let run = measure_at(corpus, &scratch.join(&format!("out-{index}")));
        let paths: Vec<String> = leaves(run.result())
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        assert_eq!(paths, RESULT_LEAVES, "{}", corpus.display());
    }
}

// ------------------------------------------------------ TE-2 unreadable

/// TE-2. M: an unreadable file fatal, left out of `files`, or given
/// another cause. A mode-000 `.rs` file added to a scratch copy of each
/// fixture is listed (`files` + 1) and skipped `unreadable` (`skipped` +
/// 1), and changes nothing else: its value, which an item holds, is never
/// read. Skipped when mode 000 does not stop reads (root).
#[test]
fn unreadable_file_is_skipped_unreadable() {
    let scratch = Scratch::new("constants-unreadable");
    for (fixture, sealed, value) in [
        ("code-consts/one", "firmware/sealed.rs", "VLV-001"),
        ("code-consts/two", "panel/sealed.rs", "CR-0001"),
    ] {
        let name = fixture.replace('/', "-");
        let copy = scratch.join(&name);
        copy_tree(&fixture_dir(fixture), &copy, false, false);
        let locked = copy.join(sealed);
        write(&locked, format!("pub const SEALED: &str = \"{value}\";\n"));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod 000");
        if fs::read(&locked).is_ok() {
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).expect("chmod back");
            eprintln!("mode 000 is readable here (root?); nothing to measure");
            return;
        }
        let run = measure_at(&copy, &scratch.join(&format!("{name}-out")));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).expect("chmod back");
        let expected = expected(fixture);
        let mut want = expected["result"].clone();
        for key in ["files", "skipped"] {
            want[key] = json!(count(&expected["result"], key) + 1);
        }
        assert_eq!(without_ms(run.result()), want, "{fixture}");
        assert_rows_equal(
            &run.rows(),
            expected["constants"].as_array().expect("rows"),
            fixture,
        );
        let mut want_skipped = expected_array(fixture, "skipped");
        want_skipped.push(json!({"path": sealed, "cause": "unreadable"}));
        want_skipped.sort_by(|a, b| {
            a["path"]
                .as_str()
                .expect("a path")
                .as_bytes()
                .cmp(b["path"].as_str().expect("a path").as_bytes())
        });
        assert_eq!(run.skipped(), want_skipped, "{fixture}");
    }
}

// ------------------------------------------------- as census (contract)

/// "As census": `--out` under the corpus is refused, exit 2, nothing
/// written (the corpus is a scratch copy, so a broken guard writes nowhere
/// that matters). M: the guard dropped.
#[test]
fn out_under_the_corpus_is_refused() {
    let scratch = Scratch::new("constants-guard");
    let corpus = scratch.join("corpus");
    copy_tree(&fixture_dir("code-consts/one"), &corpus, false, false);
    let before = snapshot(&corpus);
    let out = corpus.join("out");
    let output = constants_in(
        &repository_root(),
        &[
            "--pilot",
            corpus.to_str().expect("UTF-8"),
            "--out",
            out.to_str().expect("UTF-8"),
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        output.stdout.is_empty(),
        "a refusal prints nothing on stdout"
    );
    assert!(stderr(&output).contains("--out"), "{}", stderr(&output));
    assert!(!out.exists(), "--out is not created");
    assert!(snapshot(&corpus) == before, "the corpus is unchanged");
}

/// "As census". M: the label's census config ignored. `--label pilot-b`
/// without `--pilot` reads the corpus from `SPECENGINE_PILOT_B` and the
/// config from `SPECENGINE_CENSUS_CONFIG_B` before the corpus root's
/// `census.toml`, which here names only the missing root: the run reads
/// as fixture two, its detail under `constants/pilot-b/`.
#[test]
fn pilot_label_takes_the_census_config_from_the_environment() {
    let scratch = Scratch::new("constants-pilot-label");
    let corpus = scratch.join("corpus");
    copy_tree(&fixture_dir("code-consts/two"), &corpus, false, false);
    let config = scratch.join("config-b.toml");
    let original = fs::read_to_string(corpus.join("census.toml")).expect("the config");
    fs::write(&config, &original).expect("the label's config");
    let roots = "roots = [\"panel\", \"winch/limits.rs\", \"panel/lifts.rs\", \"spare\"]";
    assert!(original.contains(roots));
    fs::write(
        corpus.join("census.toml"),
        original.replace(roots, "roots = [\"spare\"]"),
    )
    .expect("the decoy config");
    let out = scratch.join("out");
    let mut command = eval();
    command
        .env("SPECENGINE_PILOT_B", &corpus)
        .env("SPECENGINE_CENSUS_CONFIG_B", &config)
        .args([
            "constants",
            "--label",
            "pilot-b",
            "--out",
            out.to_str().expect("UTF-8"),
        ]);
    let output = command.output().expect("specengine-eval runs");
    let envelope = envelope(&output);
    assert_eq!(envelope["label"], "pilot-b");
    let expected = expected("code-consts/two");
    assert_eq!(without_ms(&envelope["result"]), expected["result"]);
    assert_rows_equal(
        &array(&out.join("constants/pilot-b/constants.json")),
        expected["constants"].as_array().expect("rows"),
        "pilot-b",
    );
}

/// "No `--scheme`". M: `constants` given another subcommand's arguments.
/// The flag is refused (exit 2) and nothing is written.
#[test]
fn scheme_flag_is_refused() {
    let scratch = Scratch::new("constants-scheme");
    let out = scratch.join("out");
    let scheme = scratch.join("specengine.toml");
    write(&scheme, "[ids]\n");
    let output = constants_in(
        &repository_root(),
        &[
            "--scheme",
            scheme.to_str().expect("UTF-8"),
            "--out",
            out.to_str().expect("UTF-8"),
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(!out.exists(), "--out is not created");
}
