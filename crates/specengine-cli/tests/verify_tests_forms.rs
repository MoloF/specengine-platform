//! docs/features/verify-tests.md (8a-1) RD-1's CLI half, RD-2's terminal
//! half, TE-2's "stdin a pipe" and TE-8's `[code]` half: the forms of
//! `spec verify --tests` ("Order" step 1: usage, exit 2, stdout empty; the
//! terminal before any config, task or code read), the `[verify]` table's
//! exits ("The verify table": no `tests`, a bad template), a project
//! without `[code]` (`fixtures/spec-c`) and one without a `@verifies`
//! marker (`fixtures/verify-ecs`: `empty`). The binary runs where the
//! terminal or the usage error is the subject (stdin a pipe holding `y`:
//! an agent's shell), the library with a consent closure elsewhere. Each
//! test names the mutation that turns it red (`M:`). `--task` without
//! `--tests` is plain verify's scope since docs/features/verify-task.md
//! (8a-2): its forms, `--task --staged` the usage error, are
//! `verify_task.rs`'s `verify_task_with_staged_is_usage`.

mod common;
mod lock_common;
mod verify_tests_common;

use lock_common::LockRepo;
use verify_tests_common::{Kiln, TEMPLATE};

/// The terminal refusal's words (`main.rs` `require_terminal`).
const NOT_A_TERMINAL: &str =
    "`spec verify --tests` asks the owner for consent on a terminal, and stdin is not one";

/// The text of a project without `[code]` (`docs/canon/code-bindings.md`
/// "The `[code]` table").
const NO_CODE: &str =
    "specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// RD-1 (P1): `--tests` with `--staged` or `--changed` is a usage error:
/// exit 2, stdout empty, the message naming both flags, before the
/// terminal check; nothing started. M: `--tests` with `--changed` or
/// `--staged` accepted (the run reaches the terminal check, or runs).
#[test]
fn verify_tests_with_staged_is_usage() {
    let kiln = Kiln::new("vt-usage-staged");
    for other in ["--staged", "--changed"] {
        for args in [
            &["verify", "--tests", other][..],
            &["verify", other, "--tests"][..],
            &["verify", "--tests", other, "--json"][..],
        ] {
            let run = kiln.spec_piped_stdin(args, b"y\n");
            run.code(2);
            assert_eq!(run.stdout, "", "{args:?}: stdout empty");
            assert!(
                run.stderr.contains("--tests") && run.stderr.contains(other),
                "{args:?}: the usage error names both flags:\n{}",
                run.stderr
            );
            assert!(
                !run.stderr.contains(NOT_A_TERMINAL),
                "{args:?}: usage is judged before the terminal:\n{}",
                run.stderr
            );
        }
    }
    kiln.assert_nothing_started("usage");
}

/// RD-2 (ADR-0044): `require_terminal("verify --tests")` runs before any
/// config, task or code read: a broken `[verify]` table, an unknown task
/// and a config that does not parse all give the terminal refusal, exit 2,
/// stdout empty, never the config's line. TE-2: stdin a pipe holding `y`
/// (an agent's shell) never starts the runner: its log stays absent. M:
/// the config read first.
#[test]
fn verify_tests_not_a_terminal_wins_over_a_bad_config() {
    let kiln = Kiln::new("vt-terminal");
    // A sound config first: the pipe alone refuses, nothing started.
    for args in [
        &["verify", "--tests"][..],
        &["verify", "--tests", "--json"][..],
        &["verify", "--tests", "--task", "T-0099"][..],
    ] {
        let run = kiln.spec_piped_stdin(args, b"y\nyes\n");
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}: stdout empty");
        assert!(
            run.stderr.contains(NOT_A_TERMINAL),
            "{args:?}: the terminal refusal:\n{}",
            run.stderr
        );
        assert!(
            !run.stderr.contains("T-0099"),
            "{args:?}: no task was read:\n{}",
            run.stderr
        );
    }
    kiln.assert_nothing_started("stdin a pipe");

    // Then broken configs, uncommitted: still the terminal, never a line.
    let config = kiln.repo.read("specengine.toml");
    for broken in [
        format!("{config}\n[verify]\ntests = \"scripts/run-test {{test}}\"\n"),
        config.replace("[verify]\ntests = ", "[verify]\ntests = \"x\"\nmode = "),
        config.replace("[code]\nroots = [\"crates\"]\n", "[code]\nroots = 7\n"),
        format!("{config}\n[verify\n"),
    ] {
        kiln.repo.write("specengine.toml", &broken);
        let run = kiln.spec_piped_stdin(&["verify", "--tests"], b"y\n");
        run.code(2);
        assert_eq!(run.stdout, "", "stdout empty");
        assert!(
            run.stderr.contains(NOT_A_TERMINAL),
            "the terminal is checked first:\n{broken}\n{}",
            run.stderr
        );
        assert!(
            !run.stderr.contains("specengine.toml:"),
            "no config line read:\n{}",
            run.stderr
        );
    }
    kiln.assert_nothing_started("a broken config");
}

/// "The verify table": a committed config without `tests` exits 2
/// `[verify] tests is not set in specengine.toml` (no question, nothing
/// started); a string template exits 2 naming its line with the argv-list
/// text. M: a missing `tests` run with a default runner.
#[test]
fn verify_tests_without_tests_key_is_exit_2() {
    let kiln = Kiln::new("vt-no-tests");
    kiln.set_verify_table("[verify]\nmode = \"observe\"\n", "no tests key");
    let run = kiln.run();
    let text = run.cannot_text("no tests key");
    assert!(
        text.contains("[verify] tests is not set in specengine.toml"),
        "{text}"
    );
    assert!(run.prompts.is_empty(), "no question: {:?}", run.prompts);
    kiln.assert_nothing_started("no tests key");
    assert!(kiln.events().is_empty(), "no event");

    kiln.set_verify_table(
        "[verify]\nmode = \"observe\"\ntests = \"scripts/run-test {test}\"\n",
        "a string template",
    );
    let line = kiln.config_line("tests = ");
    let run = kiln.run();
    let text = run.cannot_text("a string template");
    assert!(
        text.contains(&format!(
            "specengine.toml:{line}: [verify] tests must be an argv list, e.g. [\"scripts/run-test\", \"{{test}}\"]"
        )),
        "{text}"
    );
    assert!(run.prompts.is_empty(), "no question");
    kiln.assert_nothing_started("a string template");
}

/// "The verify table": `timeout` defaults to 600 and a written one is
/// what the question and the document carry; the template is shown as
/// written. M: another default; the written timeout ignored.
#[test]
fn verify_timeout_defaults_to_600_and_is_shown() {
    let kiln = Kiln::new("vt-timeout-shown");
    let run = kiln.run_with(&kiln.repo.root, None, false, &[]);
    run.code(1);
    let question = run.question();
    assert!(
        question.contains("(timeout 600 s each)"),
        "the default: {question}"
    );
    for element in TEMPLATE {
        assert!(question.contains(element), "{element} shown: {question}");
    }
    assert_eq!(run.json()["timeout"], 600);

    kiln.set_verify(&TEMPLATE, Some(7));
    let run = kiln.run_with(&kiln.repo.root, None, false, &[]);
    assert!(
        run.question().contains("(timeout 7 s each)"),
        "{}",
        run.question()
    );
    assert_eq!(run.json()["timeout"], 7);
    kiln.assert_nothing_started("declined");
}

/// "Order" steps 2-3: no project found (discovery: stdout empty), an
/// unborn `HEAD`, a detached `HEAD` and a merge in progress each exit 2
/// before the clean check and the question; nothing started, no event. M:
/// a detached `HEAD` or an operation in progress run.
#[test]
fn cannot_run_without_a_branch_or_a_project_is_exit_2() {
    let kiln = Kiln::new("vt-place");
    let elsewhere = kiln.repo.scratch.dir("no-project");
    let run = kiln.run_with(&elsewhere, None, true, &[]);
    run.failed("no project");

    let head = kiln.repo.head();
    kiln.repo.git(&["checkout", "-q", "--detach"]);
    let run = kiln.run();
    run.cannot_text("a detached HEAD");
    assert!(run.prompts.is_empty(), "no question");
    kiln.repo.git(&["checkout", "-q", "main"]);

    std::fs::write(
        kiln.root().join(".git").join("MERGE_HEAD"),
        format!("{head}\n"),
    )
    .expect("MERGE_HEAD");
    let run = kiln.run();
    run.cannot_text("a merge in progress");
    assert!(run.prompts.is_empty(), "no question");
    std::fs::remove_file(kiln.root().join(".git").join("MERGE_HEAD")).expect("MERGE_HEAD");
    kiln.assert_nothing_started("no place");
    assert!(kiln.events().is_empty(), "no event");

    let unborn = Kiln::bare(LockRepo::uncommitted("vt-unborn"), "kiln-works");
    std::fs::remove_dir_all(unborn.root()).expect("clear");
    common::copy_dir(&common::fixture("verify-tests"), unborn.root());
    let _ = std::fs::remove_file(unborn.root().join("expected.json"));
    unborn.write_runner("scripts/run-test", 0o755);
    unborn.repo.git.init(unborn.root());
    let run = unborn.run();
    run.cannot_text("an unborn HEAD");
    assert!(run.prompts.is_empty(), "no question");
    unborn.assert_nothing_started("an unborn HEAD");
}

/// TE-8 (ADR-0008): a project without `[code]` (`fixtures/spec-c`, a
/// non-Rust corpus) given a `[verify] tests` exits 2 naming `[code]`, no
/// question, nothing run; without `[verify] tests` it exits 2 naming
/// that. M: a missing `[code]` read as an empty walk.
#[test]
fn spec_c_without_code_table_is_exit_2() {
    let kiln = Kiln::bare(
        LockRepo::of("vt-spec-c", "spec-c", "repo", &[]),
        "dock-hire",
    );
    let run = kiln.run();
    let text = run.cannot_text("spec-c as it is");
    assert!(
        text.contains("[verify] tests is not set in specengine.toml"),
        "{text}"
    );

    kiln.add_template_and_runner();
    let run = kiln.run();
    let text = run.cannot_text("spec-c with a template");
    assert!(text.contains(NO_CODE), "{text}");
    assert!(run.prompts.is_empty(), "no question");
    kiln.assert_nothing_started("spec-c");
    assert!(kiln.events().is_empty(), "no event");
}

/// "Order" step 6 (S10): `fixtures/verify-ecs`, an ECS game crate with no
/// `@verifies` marker, given a template and a runner: `empty`, exit 1, no
/// question, no event, nothing run; the summary gives the reason. A
/// project of another nature than the kiln (ADR-0008). M: an empty set
/// asking the question, or `verified` over nothing.
#[test]
fn project_without_verifies_marker_is_empty() {
    let kiln = Kiln::bare(
        LockRepo::of("vt-ecs-empty", "verify-ecs", "repo", &[]),
        "verify-ecs",
    );
    kiln.add_template_and_runner();
    let run = kiln.run();
    run.code(1);
    assert_eq!(run.verdict(), "empty", "{}", run.show());
    assert!(run.prompts.is_empty(), "no question: {:?}", run.prompts);
    assert!(
        run.summary().starts_with("verify --tests: empty: "),
        "{}",
        run.summary()
    );
    assert!(
        run.rows().is_empty() && run.nodes().is_empty(),
        "{}",
        run.show()
    );
    kiln.assert_nothing_started("empty");
    assert!(kiln.events().is_empty(), "no event on empty");
}
