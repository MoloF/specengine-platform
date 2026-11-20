//! docs/features/department-practices.md (RM-0128) "Acceptance criteria",
//! what names no discipline reads as the base build does:
//!
//! - DP6, the literal form (`#[ignore]`, needs the base build): every node
//!   of `fixtures/spec-a` and `fixtures/spec-b` (`spec map --json`'s names,
//!   the targets `bundle.rs` and `bundle_fit.rs` draw from) bundled at 300,
//!   2 000 and 10 000 tokens by the base `spec` and by this build, each with
//!   a `HOME` of its own: the exit, the text (body, `bundle_hash`, totals)
//!   and stderr byte-equal, and the JSON equal once its one
//!   `,"practices":[]` closing `layers` is taken out. The always-run half
//!   is `practices_bundle.rs` (no layer 10 without `--discipline`).
//! - DP22: this repository (read only, its index under scratch `HOME`s):
//!   the always-run half checks clean (exit 0, no `practice-discipline`)
//!   and its bundles of the canon pages, the slice's spec and ADR-0084 print
//!   no `## Practices` and serve `layers.practices: []`; the literal form
//!   (`#[ignore]`) compares those bundles, text and JSON, and `card list`,
//!   `task list` and `card counts` with the base build's as DP6 does. A
//!   scratch `HOME` holds no card or task of the owner's (his queue is
//!   never read), so those three compare empty queues.
//!
//! Run the literal forms: `SPECENGINE_BASE_SPEC=<a spec built at 690fe65e>
//! cargo nextest run -p specengine-cli --test practices_base --run-ignored
//! only`.

#![cfg(unix)]

mod common;

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use common::{Run, SPEC, Scratch, fixture, program_with, repository_root};
use serde_json::{Value, json};

/// The variable naming the base `spec` binary.
const BASE_VAR: &str = "SPECENGINE_BASE_SPEC";
/// What this build adds to a bundle that names no discipline.
const PRACTICES_KEY: &str = ",\"practices\":[]}";

fn base() -> OsString {
    std::env::var_os(BASE_VAR).unwrap_or_else(|| panic!("{BASE_VAR} names the base `spec`"))
}

fn run(program: &OsStr, cwd: &Path, home: &Path, args: &[&str]) -> Run {
    program_with(program, cwd, args, &[("HOME", home.as_os_str())])
}

/// `spec map --json`'s names in `root`.
fn map_names(root: &Path, home: &Path) -> Vec<String> {
    let done = run(OsStr::new(SPEC), root, home, &["--json", "map"]);
    done.code(0);
    done.json()["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// `args` by the base and by this build, each under its own `HOME`: the
/// exit, stderr and text stdout byte-equal; a JSON stdout equal once its
/// one `"practices":[]` closing `layers` is taken out (when `bundle`).
fn assert_as_base(base: &OsStr, cwd: &Path, homes: (&Path, &Path), args: &[&str], bundle: bool) {
    let old = run(base, cwd, homes.0, args);
    let new = run(OsStr::new(SPEC), cwd, homes.1, args);
    assert_eq!(
        new.code,
        old.code,
        "{args:?}: exit\n{}\n{}",
        old.show(),
        new.show()
    );
    assert_eq!(new.stderr, old.stderr, "{args:?}: stderr");
    let json = args.first() == Some(&"--json");
    if json && bundle && new.code == 0 {
        assert_eq!(
            new.stdout.matches(PRACTICES_KEY).count(),
            1,
            "{args:?}: one empty layer 10 closing `layers`: {}",
            new.stdout
        );
        let without = new.stdout.replacen(PRACTICES_KEY, "}", 1);
        assert_eq!(without, old.stdout, "{args:?}: the JSON but layer 10");
        let document: Value = serde_json::from_str(&new.stdout).expect("JSON");
        assert_eq!(document["layers"]["practices"], json!([]));
    } else {
        assert_eq!(new.stdout, old.stdout, "{args:?}: stdout");
    }
}

// ------------------------------------------------------------------ DP6

/// DP6: every node of spec-a and spec-b at 300, 2 000 and 10 000 tokens,
/// text and JSON, as the base build. M: layer 10 filled without
/// `--discipline` (the JSON's layer 10 is not `[]`, the body moves).
#[test]
#[ignore = "needs SPECENGINE_BASE_SPEC, a `spec` built at the base commit"]
fn dp6_every_fixture_bundle_is_the_base_builds() {
    let base = base();
    let scratch = Scratch::new("dp6-base");
    for name in ["spec-a", "spec-b"] {
        let root = scratch.copy(name, name);
        let old_home = scratch.home(&format!("{name}-old"));
        let new_home = scratch.home(&format!("{name}-new"));
        let nodes = map_names(&root, &new_home);
        assert!(nodes.len() > 10, "{name}: {nodes:?}");
        for node in &nodes {
            for budget in ["300", "2000", "10000"] {
                for json in [false, true] {
                    let mut args = Vec::new();
                    if json {
                        args.push("--json");
                    }
                    args.extend(["bundle", node.as_str(), "--budget", budget]);
                    assert_as_base(&base, &root, (&old_home, &new_home), &args, true);
                }
            }
        }
        // And two targets at once, as bundle.rs names them.
        if name == "spec-a" {
            for json in [false, true] {
                let mut args = Vec::new();
                if json {
                    args.push("--json");
                }
                args.extend(["bundle", "MEC-STAMINA", "MEC-SPRINT", "--budget", "2000"]);
                assert_as_base(&base, &root, (&old_home, &new_home), &args, true);
            }
        }
    }
    assert!(fixture("spec-a").is_dir());
}

// ----------------------------------------------------------------- DP22

/// The REFs of this repository DP22 bundles: every canon page, the
/// slice's spec and ADR-0084, by path.
fn repository_refs(root: &Path) -> Vec<String> {
    let mut refs: Vec<String> = fs::read_dir(root.join("docs/canon"))
        .expect("docs/canon")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            path.strip_prefix(root)
                .expect("under the root")
                .to_str()
                .expect("UTF-8")
                .to_owned()
        })
        .collect();
    refs.sort();
    refs.push("docs/features/department-practices.md".to_owned());
    refs.push("docs/decisions/ADR-0084.md".to_owned());
    refs
}

/// DP22, always run: this repository checks clean and its bundles name no
/// practice.
#[test]
fn dp22_this_repository_checks_clean_with_no_practice() {
    let root = repository_root();
    let scratch = Scratch::new("dp22-repo");
    let home = scratch.home("h");
    let check = run(OsStr::new(SPEC), &root, &home, &["--json", "check"]);
    assert_eq!(check.code, 0, "the check stays clean: {}", check.show());
    let report = check.json();
    let warned: Vec<&Value> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|finding| finding["code"] == "practice-discipline")
        .collect();
    assert_eq!(warned, Vec::<&Value>::new());
    let refs = repository_refs(&root);
    assert!(refs.len() > 20, "{refs:?}");
    for reference in &refs {
        let done = run(
            OsStr::new(SPEC),
            &root,
            &home,
            &["--json", "bundle", reference],
        );
        done.code(0);
        let document = done.json();
        assert_eq!(document["layers"]["practices"], json!([]), "{reference}");
        let body = document["body"].as_str().expect("a body");
        assert!(
            !body.lines().any(|line| line == "## Practices"),
            "{reference}"
        );
    }
}

/// DP22, the literal form: the bundles of [`repository_refs`] at the
/// default budget and at 10 000, text and JSON, and `card list`, `task
/// list`, `card counts` (empty scratch queues) as the base build.
#[test]
#[ignore = "needs SPECENGINE_BASE_SPEC, a `spec` built at the base commit"]
fn dp22_this_repository_reads_as_the_base_builds() {
    let base = base();
    let root: PathBuf = repository_root();
    let scratch = Scratch::new("dp22-base");
    let old_home = scratch.home("old");
    let new_home = scratch.home("new");
    for reference in repository_refs(&root) {
        for budget in [None, Some("10000")] {
            for json in [false, true] {
                let mut args = Vec::new();
                if json {
                    args.push("--json");
                }
                args.extend(["bundle", reference.as_str()]);
                if let Some(budget) = budget {
                    args.extend(["--budget", budget]);
                }
                assert_as_base(&base, &root, (&old_home, &new_home), &args, true);
            }
        }
    }
    for args in [
        &["card", "list"][..],
        &["--json", "card", "list"][..],
        &["--json", "task", "list"][..],
        &["--json", "card", "counts"][..],
    ] {
        assert_as_base(&base, &root, (&old_home, &new_home), args, false);
    }
}
