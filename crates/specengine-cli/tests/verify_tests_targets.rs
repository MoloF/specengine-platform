//! docs/features/verify-tests.md (8a-1), the rulings of review 1
//! (`vt-review-iter1.md`) that iteration 2 builds, written as targets
//! before the code: tests marked "red by design on iteration 1" fail
//! against iteration 1's code and must pass after iteration 2.
//!
//! - V1 (deviation 4 rejected): `nodes` is `[]` whenever step 9 did not
//!   complete (`declined`, `refused`; `interrupted` in
//!   `verify_tests_targets_signal.rs`), as under `changed`: no node reads
//!   `failed` when nothing ran, in stdout, the document or the event.
//! - V2: the template's provenance (`docs/canon/spec-verify-tests.md`
//!   "Order", `docs/canon/spec-verify-output.md` "`--tests`"):
//!   `specengine.toml` (step 4) and every distinct planned program that is
//!   relative and holds a `/`, probes included (step 6), must be files of
//!   `HEAD`, else the verdict is `cannot` with the canon's `cannot:` line.
//! - Keyless `@verifies` bindings of one file with the same state stay
//!   separate rows.
//! - `primary_target` with a bin declared `./src/main.rs` beside another
//!   bin (green on iteration 1: see the test).
//! - Round 2 pins of V2's program path: a `./` program accepted, a path
//!   leaving the worktree `cannot` (V3's `..` rule:
//!   `verify_tests_targets_v3.rs`).
//!
//! Scratch repositories of `fixtures/verify-tests` (P10) with the fake,
//! self-stopping runner (TE-1); the library with a consent closure. Each
//! test names the mutation that turns it red (`M:`).

mod common;
mod lock_common;
mod verify_tests_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use serde_json::{Value, json};
use verify_tests_common::{Kiln, RUNNER_SCRIPT, TestsRun, args_of, owned};

/// V2, step 4's `cannot:` line (the canon's text).
const CONFIG_NOT_AT_HEAD: &str =
    "specengine.toml is not a file of HEAD: [verify] tests runs only a committed template";

/// V2, step 6's `cannot:` line for `scripts/local-runner` (the canon's
/// text).
const PROGRAM_NOT_AT_HEAD: &str =
    "the [verify] tests program `scripts/local-runner` is not a file of HEAD: commit it first";

/// V2: exit 2, verdict `cannot`, `line` among the document's `cannot:`
/// lines and printed as one; no question.
fn assert_cannot_line(run: &TestsRun, line: &str, context: &str) {
    run.code(2);
    let doc = run.json();
    assert_eq!(doc["verdict"], "cannot", "{context}: {}", run.show());
    assert!(
        doc["cannot"]
            .as_array()
            .is_some_and(|lines| lines.contains(&json!(line))),
        "{context}: no cannot line {line:?}: {}",
        doc["cannot"]
    );
    run.has_line(&format!("cannot: {line}"));
    assert!(run.prompts.is_empty(), "{context}: asked {:?}", run.prompts);
}

/// V1: the document's `nodes` is `[]`, stdout holds no `node` line, the
/// node counts hold no `failed`.
fn assert_no_node_verdict(run: &TestsRun, context: &str) {
    let doc = run.json();
    assert_eq!(doc["nodes"], json!([]), "{context}: {}", run.show());
    assert_eq!(
        doc["counts"]["nodes"]["failed"], 0,
        "{context}: {}",
        doc["counts"]
    );
    assert!(
        !run.lines().iter().any(|line| line.starts_with("node ")),
        "{context}: a node line though nothing ran:\n{}",
        run.text()
    );
}

/// V1, red by design on iteration 1: the owner's `n` (`declined`) and a
/// probe that passes (`refused`) run no test, so no node gets a verdict:
/// `nodes: []` in the document and in the refused run's event, no `node`
/// line on stdout. M: `failed` nodes when nothing ran.
#[test]
fn nodes_are_empty_when_no_test_ran() {
    let kiln = Kiln::new("vt-v1-stopped");
    let declined = kiln.run_with(kiln.root(), None, false, &[]);
    declined.code(1);
    assert_eq!(declined.verdict(), "declined");
    assert_no_node_verdict(&declined, "declined");
    assert!(kiln.events().is_empty(), "declined records nothing");

    kiln.plan(&[("*probe", "pass")]);
    let refused = kiln.run();
    refused.code(2);
    assert_eq!(refused.verdict(), "refused");
    assert_no_node_verdict(&refused, "refused");
    let event = kiln.one_event();
    assert_eq!(event["nodes"], json!([]), "the event: {event}");
}

/// V2 (step 6), red by design on iteration 1: element 0 names a relative
/// program holding a `/` that `HEAD` does not hold (written on disk,
/// ignored by a committed `.gitignore`, so the whole-tree status is
/// clean): exit 2, verdict `cannot`, ``the [verify] tests program
/// `scripts/local-runner` is not a file of HEAD: commit it first``; no
/// question, nothing started, no event. M: the program read from disk
/// unchecked.
#[test]
fn untracked_runner_program_is_cannot() {
    let kiln = Kiln::new("vt-v2-program");
    kiln.repo.write(".gitignore", "scripts/local-runner\n");
    kiln.set_verify(&["scripts/local-runner", "{test}"], None);
    kiln.write_runner("scripts/local-runner", 0o755);
    assert!(
        kiln.repo.porcelain().is_empty(),
        "the scenario: a clean status"
    );
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    assert_cannot_line(&run, PROGRAM_NOT_AT_HEAD, "a program HEAD lacks");
    kiln.assert_nothing_started("a program HEAD lacks");
    assert!(kiln.events().is_empty(), "no event");
}

/// V2 (step 4), red by design on iteration 1: `specengine.toml` (the
/// template's source) not a file of `HEAD` (removed from the index,
/// ignored by a committed `.gitignore`, still on disk; the whole-tree
/// status is clean): exit 2, verdict `cannot`, `specengine.toml is not a
/// file of HEAD: [verify] tests runs only a committed template`; no
/// question, nothing started, no event. M: the template read from an
/// untracked config.
#[test]
fn untracked_config_is_cannot() {
    let kiln = Kiln::new("vt-v2-config");
    kiln.repo.git(&["rm", "-q", "--cached", "specengine.toml"]);
    kiln.repo.write(".gitignore", "specengine.toml\n");
    kiln.repo.commit_all("the config untracked");
    assert!(
        kiln.repo.porcelain().is_empty(),
        "the scenario: a clean status"
    );
    assert!(kiln.root().join("specengine.toml").is_file());
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    assert_cannot_line(&run, CONFIG_NOT_AT_HEAD, "a config HEAD lacks");
    kiln.assert_nothing_started("a config HEAD lacks");
    assert!(kiln.events().is_empty(), "no event");
}

/// Review 1 (nit, deviation 6), red by design on iteration 1: two keyless
/// `@verifies` bindings in one file with the same state (`unanchored
/// in_body`, markers in a function body naming two nodes) are two rows,
/// each with its own node, kind the state word, symbol the category,
/// `not run | … | not a function`. M: keyless rows keyed by (file,
/// category), their markers merged.
#[test]
fn keyless_bindings_of_one_file_stay_separate_rows() {
    let kiln = Kiln::new("vt-keyless-rows");
    kiln.repo.write(
        "crates/kiln/src/vents.rs",
        "//! Vents.\n\npub fn vent_open() -> u32 {\n    // @verifies KILN-RAMP\n    let low = 1;\n    // @verifies KILN-HOLD\n    let high = 2;\n    low + high\n}\n",
    );
    kiln.repo.commit_all("two markers in one body");
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    run.code(1);
    let rows: Vec<Value> = run
        .rows()
        .into_iter()
        .filter(|row| row["file"] == "crates/kiln/src/vents.rs")
        .collect();
    assert_eq!(rows.len(), 2, "two rows: {rows:#?}");
    for row in &rows {
        assert_eq!(
            (
                &row["kind"],
                &row["symbol"],
                &row["outcome"],
                &row["reason"]
            ),
            (
                &json!("unanchored"),
                &json!("in_body"),
                &json!("not_run"),
                &json!("not a function")
            ),
            "{row}"
        );
    }
    let mut nodes: Vec<String> = rows.iter().map(|row| row["nodes"].to_string()).collect();
    nodes.sort();
    assert_eq!(nodes, ["[\"KILN-HOLD\"]", "[\"KILN-RAMP\"]"], "{rows:#?}");
}

/// Review 1 (minor, `primary_target`): a package whose manifest declares
/// `[[bin]] path = "./src/main.rs"` beside a second bin: the test in
/// `src/main.rs` is the primary target's (`{target_kind}` `bin`,
/// `{target}` the package's name, the unit left out of its qpath) and
/// runs. Green on iteration 1 too: the CLI's layout tables root the file
/// as `src/main.rs` (`code_walk` strips the package directory; the
/// manifest's `path` is not read), so the `./` comparison the review names
/// is not reachable through the walk. A `#[cfg(test)]` test of
/// `primary_target` over a crafted table belongs in `verify_tests.rs`
/// once iteration 2 has landed. M: the default bin not found beside
/// another (`no value {target}`).
#[test]
fn primary_bin_resolves_beside_another_bin() {
    let kiln = Kiln::new("vt-primary-bin");
    kiln.repo.write(
        "docs/spec/tool.md",
        "---\nclass: canon\nid: KILN-TOOL\nowner: owner\nreviewed: 2026-10-16\n---\n\n# Tool\n\nThe tool reads the kiln's schedule aloud.\n",
    );
    kiln.repo.write(
        "crates/kiln-tool/Cargo.toml",
        "[package]\nname = \"kiln-tool\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[[bin]]\nname = \"kiln-tool\"\npath = \"./src/main.rs\"\n\n[[bin]]\nname = \"kiln-extra\"\npath = \"src/bin/extra.rs\"\n\n[workspace]\n",
    );
    kiln.repo.write(
        "crates/kiln-tool/src/main.rs",
        "fn main() {}\n\n#[cfg(test)]\nmod tests {\n    // @verifies KILN-TOOL\n    #[test]\n    fn tool_check() {}\n}\n",
    );
    kiln.repo
        .write("crates/kiln-tool/src/bin/extra.rs", "fn main() {}\n");
    kiln.repo.commit_all("a package of two bins");
    let run = kiln.run();
    let row = run.row("crates/kiln-tool::tests::tool_check");
    assert_eq!(
        args_of(&row),
        owned(&["kiln-tool", "bin", "kiln-tool", "tests::tool_check"]),
        "{row}"
    );
    assert_eq!(row["outcome"], "passed", "{row}");
    assert_eq!(run.node("KILN-TOOL")["verdict"], "verified");
}

/// V2 (step 6, the program's path): element 0 relative with a `/` is
/// joined to the worktree top with `.` dropped, so `./scripts/run-test`
/// names the committed runner: no `cannot` line, the question is asked
/// (`n`: `declined`, exit 1, nothing started); with `y` it runs the runner
/// from the root. A `..` after a name (`scripts/../scripts/run-test`) is
/// V3's `cannot` and a leading `..` from a project below the top is
/// accepted: `verify_tests_targets_v3.rs`. M: a written `.` refused (the
/// program looked up in `HEAD` as written).
#[test]
fn current_dir_program_is_accepted() {
    let kiln = Kiln::new("vt-v2-dotted");
    let program = "./scripts/run-test";
    kiln.set_verify(&[program, "{test}"], None);
    let run = kiln.run_with(kiln.root(), None, false, &[]);
    run.code(1);
    assert_eq!(run.verdict(), "declined", "{}", run.show());
    assert_eq!(run.json()["cannot"], json!([]), "{}", run.show());
    assert_eq!(run.prompts.len(), 1, "asked once");
    kiln.assert_nothing_started(program);
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.verdict(), "unverified", "{}", run.show());
    assert!(
        !kiln.test_invocations().is_empty(),
        "the runner ran: {:?}",
        kiln.log()
    );
    let root = kiln.root().to_str().expect("a UTF-8 root");
    assert!(
        kiln.log().iter().all(|call| call.cwd == root),
        "from the root: {:?}",
        kiln.log()
    );
}

/// V2 (step 6): a program whose path leaves the worktree is not a file of
/// `HEAD`, though it exists on disk and is executable: `../run-test` and
/// `scripts/../../run-test` (the fake runner written beside the worktree;
/// a runner `run-test` committed at the worktree top too, so a `..` held
/// at the top would find a file of `HEAD`): exit 2, `cannot` with the
/// canon's line naming the program as written; no question, nothing
/// started, no event. M: a `..` past the top dropped (the path clamped to
/// the worktree), or the program read from disk unchecked.
#[test]
fn program_outside_the_worktree_is_cannot() {
    let kiln = Kiln::new("vt-v2-outside");
    kiln.write_runner("run-test", 0o755);
    kiln.repo.commit_all("a runner at the top");
    let outside = kiln.repo.scratch.join("run-test");
    fs::write(&outside, RUNNER_SCRIPT).expect("the runner outside");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o755)).expect("its mode");
    for program in ["../run-test", "scripts/../../run-test"] {
        kiln.set_verify(&[program, "{test}"], None);
        assert!(
            kiln.repo.porcelain().is_empty(),
            "the scenario: a clean status"
        );
        let run = kiln.run_with(kiln.root(), None, true, &[]);
        let line = format!(
            "the [verify] tests program `{program}` is not a file of HEAD: commit it first"
        );
        assert_cannot_line(&run, &line, program);
        kiln.assert_nothing_started(program);
        assert!(kiln.events().is_empty(), "{program}: no event");
    }
}
