//! docs/features/constant-census.md (12a) TE-4: `specengine-eval constants`
//! over pilot B, the owner's manual run (`#[ignore]`, release, in the
//! foreground under a self-terminating timeout). The corpus and the census
//! config's code roots are only read (`tests/pilot/mod.rs`
//! `run_census_read_only`: `git status` and the (path, kind, size, mtime)
//! listing of the corpus and code roots equal before and after; the child
//! gets an empty scratch `HOME` and `--timeout 600`). Checked: the spec's
//! invariants over `result` and over the detail under the scratch `--out`,
//! and every figure of a tally the owner counts by a read-only grep, kept
//! outside the repository: a JSON object of `result` keys
//! (`SPECENGINE_CONSTANTS_TALLY_B`, a PHASE-B name; for pilot B the spec
//! expects its hyphenless codes to read `hyphenless` and 0 `matched`). The
//! result, counts only, is printed for the owner's record, never committed.
//!
//! Run (nothing here names a pilot):
//!
//! ```text
//! SPECENGINE_PILOT_B=… SPECENGINE_CENSUS_CONFIG_B=… SPECENGINE_CONSTANTS_TALLY_B=… \
//!   perl -e 'alarm 1800; exec @ARGV' \
//!   cargo nextest run --release -p specengine-eval --test constants_pilot \
//!   --run-ignored only --no-capture
//! ```
//!
//! It fails, never skips, naming an unset variable. The slice ships without
//! this run.

mod pilot;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The tally of TE-4: a JSON object whose leaves are `result` counts.
const TALLY: &str = "SPECENGINE_CONSTANTS_TALLY_B";

/// The six `not_valued` categories.
const NOT_VALUED: [&str; 6] = [
    "broken",
    "macro",
    "byte_string",
    "escaped",
    "multi_line",
    "other",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// A scratch directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-constants-pilot-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch");
        Self(fs::canonicalize(&path).expect("canonical scratch"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The count at a `/`-separated path of `result`.
fn count(result: &Value, path: &str) -> u64 {
    let mut value = result;
    for part in path.split('/') {
        value = &value[part];
    }
    value
        .as_u64()
        .unwrap_or_else(|| panic!("result/{path} is not a count"))
}

/// Every leaf of `value` with its `/`-separated path.
fn leaves(value: &Value, prefix: &str, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}/{key}")
                };
                leaves(inner, &path, out);
            }
        }
        other => out.push((prefix.to_owned(), other.clone())),
    }
}

fn array(path: &Path) -> Vec<Value> {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let value: Value = serde_json::from_str(&text).expect("a JSON detail file");
    value.as_array().expect("a JSON array").clone()
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B, SPECENGINE_CENSUS_CONFIG_B and SPECENGINE_CONSTANTS_TALLY_B; read-only run over a real corpus"]
fn pilot_b_constants_hold_the_invariants_and_the_tally() {
    let variables = pilot::variables("pilot-b");
    let corpus = pilot::required(variables.corpus, "pilot B's corpus");
    let config = pilot::required(variables.config, "pilot B's census config");
    let tally_path = pilot::required(
        TALLY,
        "a JSON object of `result` counts from a read-only grep of pilot B",
    );
    let root = repository_root();
    for (variable, path) in [
        (variables.corpus, &corpus),
        (variables.config, &config),
        (TALLY, &tally_path),
    ] {
        let canonical = fs::canonicalize(path)
            .unwrap_or_else(|error| panic!("{variable}: unreadable: {error}"));
        assert!(
            !canonical.starts_with(&root),
            "{variable} must point outside the repository"
        );
    }
    let tally: Value = serde_json::from_str(
        &fs::read_to_string(&tally_path).unwrap_or_else(|error| panic!("{TALLY}: {error}")),
    )
    .unwrap_or_else(|error| panic!("{TALLY}: not JSON: {error}"));
    let mut expected = Vec::new();
    leaves(&tally, "", &mut expected);
    assert!(
        !expected.is_empty() && expected.iter().all(|(_, value)| value.is_u64()),
        "{TALLY}: an object of counts"
    );

    let scratch = Scratch::new("b");
    let output = pilot::run_census_read_only("constants", "pilot-b", &scratch.0, &corpus, &config);
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    let envelope: Value = serde_json::from_str(stdout.trim()).expect("one JSON envelope");
    assert_eq!(envelope["measurement"], "constants");
    assert_eq!(envelope["label"], "pilot-b");
    let result = &envelope["result"];
    assert!(
        result.is_object(),
        "the run finished within --timeout (result is not \"timeout\")"
    );
    let mut got = Vec::new();
    leaves(result, "", &mut got);
    assert!(
        got.iter().all(|(_, value)| value.is_u64()),
        "result holds counts only"
    );

    let n = |path: &str| count(result, path);
    let not_valued: u64 = NOT_VALUED
        .iter()
        .map(|category| n(&format!("not_valued/{category}")))
        .sum();
    assert_eq!(n("items"), n("valued") + not_valued, "items");
    let classified = n("matched/id")
        + n("matched/alias")
        + n("hyphenless")
        + n("unmatched")
        + n("ambiguous/two_records")
        + n("ambiguous/two_items");
    assert_eq!(n("valued"), classified, "valued");
    assert!(n("marked") <= n("valued"), "marked <= valued");
    assert!(n("skipped") <= n("files"), "skipped <= files");
    assert_ne!(n("ambiguous/two_items"), 1, "two_items != 1");

    let detail = scratch.0.join("out").join("constants").join("pilot-b");
    let rows = array(&detail.join("constants.json"));
    let skipped = array(&detail.join("skipped.json"));
    assert_eq!(rows.len() as u64, n("items"), "one row per item");
    assert_eq!(
        skipped.len() as u64,
        n("skipped") + n("roots_missing"),
        "one skipped row per skipped file and missing root"
    );

    let mismatched: Vec<String> = expected
        .iter()
        .filter(|(path, want)| result_leaf(result, path) != Some(want))
        .map(|(path, want)| format!("{path}: tally {want}, run {:?}", result_leaf(result, path)))
        .collect();
    eprintln!(
        "constants pilot-b result (counts only): {}",
        serde_json::to_string(result).expect("JSON")
    );
    assert!(
        mismatched.is_empty(),
        "the run differs from {TALLY}:\n{}",
        mismatched.join("\n")
    );
}

/// The leaf of `result` at a `/`-separated path, if any.
fn result_leaf<'a>(result: &'a Value, path: &str) -> Option<&'a Value> {
    let mut value = result;
    for part in path.split('/') {
        value = value.get(part)?;
    }
    Some(value)
}
