//! docs/features/verify-tests.md (8a-1), re-review 2's V2 gap
//! (`vt-review-iter2.md`) and V3, the owner's pre-approved fix that
//! iteration 3 builds, written as targets before the code: tests marked
//! "red by design on iteration 2" fail against iteration 2's code and must
//! pass after iteration 3.
//!
//! - The gap: step 6 reads element 0 lexically (`..` taking the segment
//!   before), the kernel physically. A `..` after an ignored symlinked
//!   directory (`status_all` never lists an ignored path) passes the check
//!   on a committed path while the program that runs lies outside the
//!   worktree.
//! - V3: in element 0, a `..` climbs only before the first written name
//!   (out of a project root below the worktree top, whose segments are
//!   physical: the root is canonical); a `..` after a name makes the
//!   program not a file of `HEAD`: `cannot`, ``the [verify] tests program
//!   `<program>` is not a file of HEAD: commit it first``
//!   (`docs/canon/spec-verify-output.md` "`--tests`"), nothing asked or run,
//!   no event.
//!
//! Scratch repositories of `fixtures/verify-tests` (P10) with the fake,
//! self-stopping runner (TE-1); the library with a consent closure. Each
//! test names the mutation that turns it red (`M:`).

mod common;
mod lock_common;
mod verify_tests_common;

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};

use serde_json::json;
use verify_tests_common::{FIXTURE, Kiln, RUNNER_SCRIPT, SLUG, TestsRun};

use crate::common::Scratch;
use crate::common::git::Sandbox;
use crate::lock_common::LockRepo;

/// V2's `cannot` line for `program`.
fn not_at_head(program: &str) -> String {
    format!("the [verify] tests program `{program}` is not a file of HEAD: commit it first")
}

/// Exit 2, verdict `cannot`, `line` among the document's `cannot:` lines
/// and printed as one; no question.
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

/// The fake runner at the absolute `path`, executable.
fn write_runner_at(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("its directory");
    fs::write(path, RUNNER_SCRIPT).expect("the runner");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("its mode");
}

/// Red by design on iteration 2 (the gap): `.gitignore` holds `target/`,
/// `target/x` is a symlink to `<scratch>/e/a/b` outside the repository,
/// `scripts/run-test` is committed, and the template's program is
/// `target/x/../../scripts/run-test`: lexically the committed runner,
/// physically `<scratch>/e/scripts/run-test` (a fake runner written there,
/// so iteration 2 runs it). The status is clean (the link is ignored).
/// Expected: exit 2, `cannot` with the program as written, no question,
/// nothing run (consent `y`), no event. M: a `..` after a name taking the
/// segment before (iteration 2's lexical path).
#[test]
fn dotdot_through_an_ignored_symlink_is_cannot() {
    let kiln = Kiln::new("vt-v3-symlink");
    kiln.repo.write(".gitignore", "target/\n");
    kiln.repo.commit_all("target ignored");
    let outside = kiln.repo.scratch.dir("e/a/b");
    write_runner_at(&kiln.repo.scratch.join("e/scripts/run-test"));
    fs::create_dir_all(kiln.root().join("target")).expect("target/");
    symlink(&outside, kiln.root().join("target/x")).expect("target/x");
    let program = "target/x/../../scripts/run-test";
    kiln.set_verify(&[program, "{test}"], None);
    assert!(
        kiln.repo.porcelain().is_empty(),
        "the scenario: a clean status"
    );
    let run = kiln.run_with(kiln.root(), None, true, &[]);
    assert_cannot_line(&run, &not_at_head(program), "through an ignored link");
    kiln.assert_nothing_started("through an ignored link");
    assert!(kiln.events().is_empty(), "no event");
}

/// Red by design on iteration 2: a `..` after a name, even one that lands
/// on the committed runner (`scripts/../scripts/run-test`), is not a file
/// of `HEAD`: exit 2, `cannot` with the program as written, no question,
/// nothing run (consent `y`), no event. M: a `..` after a name taking the
/// segment before.
#[test]
fn dotdot_after_a_name_is_cannot() {
    let kiln = Kiln::new("vt-v3-after-name");
    let program = "scripts/../scripts/run-test";
    kiln.set_verify(&[program, "{test}"], None);
    let run = kiln.run_with(kiln.root(), None, true, &[]);
    assert_cannot_line(&run, &not_at_head(program), "a .. after a name");
    kiln.assert_nothing_started("a .. after a name");
    assert!(kiln.events().is_empty(), "no event");
}

/// Control, green on iteration 2 and after V3: the project root is `sub/`
/// below the worktree top, `tools/run-test` is committed at the top, and
/// the template's program is `../tools/run-test` (a `..` before the first
/// name climbs out of the root): no `cannot` line, the question is asked
/// (`n`: `declined`, exit 1, nothing started); with `y` the runner runs
/// from the project root. M: every `..` refused (V3 over-applied).
#[test]
fn leading_dotdot_from_a_subfolder_project_is_accepted() {
    let scratch = Scratch::new("vt-v3-subfolder");
    let git = Sandbox::new(scratch.path());
    let top = scratch.dir("top");
    let root = scratch.copy(FIXTURE, "top/sub");
    let _ = fs::remove_file(root.join("expected.json"));
    write_runner_at(&top.join("tools/run-test"));
    git.init(&top);
    git.add_all(&top);
    git.commit(&top, "the fixture below the top, the runner at it");
    let home = scratch.home("h");
    let kiln = Kiln::bare(
        LockRepo {
            scratch,
            git,
            root,
            home,
        },
        SLUG,
    );
    let program = "../tools/run-test";
    kiln.set_verify(&[program, "{test}"], None);
    assert!(
        kiln.repo.porcelain().is_empty(),
        "the scenario: a clean status"
    );
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
        "from the project root: {:?}",
        kiln.log()
    );
}
