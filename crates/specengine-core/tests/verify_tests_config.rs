//! docs/features/verify-tests.md (8a-1) RD-1, core half: the `[verify]`
//! table's two new keys ("The verify table"): `tests`, a TOML array of
//! strings run as argv, its placeholders closed (`{file}`, `{qpath}`,
//! `{package}`, `{target_kind}`, `{target}`, `{test}`; `{{`, `}}` literal
//! braces; any other brace an error), `{test}` or `{qpath}` required; and
//! `timeout`, an integer 1..86 400. Every error names its line
//! (`specengine.toml:<line>: <message>`). Pure: `verify_from_toml` over
//! texts built here. Each test names the mutation that turns it red (`M:`).

use specengine_core::check::Mode;
use specengine_core::verify::verify_from_toml;

/// The argv-list text of "The verify table".
const NOT_A_LIST: &str =
    "[verify] tests must be an argv list, e.g. [\"scripts/run-test\", \"{test}\"]";

/// `[verify]` with `tests = <value>` on line 2 (and `extra` lines after).
fn table(value: &str, extra: &str) -> String {
    format!("[verify]\ntests = {value}\n{extra}")
}

/// `text` is refused at `line`, its message holding `words`, its
/// `specengine.toml:<line>: ` form.
fn refused(text: &str, line: usize, words: &str) -> String {
    let error = verify_from_toml(text).expect_err(text);
    assert_eq!(error.line, Some(line), "{text:?}: {error:?}");
    assert!(
        error.message.contains(words),
        "{text:?}: {words:?} not in {}",
        error.message
    );
    assert!(
        error
            .at("specengine.toml")
            .starts_with(&format!("specengine.toml:{line}: ")),
        "{}",
        error.at("specengine.toml")
    );
    error.message
}

/// `text` reads.
fn accepted(text: &str) {
    verify_from_toml(text).unwrap_or_else(|error| panic!("{text:?}: {error:?}"));
}

/// RD-1 (P1): `tests` must be a TOML array of strings: a string (a shell
/// line) is refused with the spec's text, at its line; a number, a table,
/// an array holding a non-string likewise refused at their line. M: a
/// string accepted.
#[test]
fn verify_tests_must_be_an_argv_list() {
    let message = refused(&table("\"scripts/run-test {test}\"", ""), 2, NOT_A_LIST);
    assert_eq!(message, NOT_A_LIST);
    let message = refused(
        "[project]\nslug = \"x\"\n\n[verify]\nmode = \"observe\"\ntests = \"run {test}\"\n",
        6,
        NOT_A_LIST,
    );
    assert_eq!(message, NOT_A_LIST);
    for value in [
        "7",
        "{ program = \"run\" }",
        "[\"run\", 1, \"{test}\"]",
        "true",
    ] {
        refused(&table(value, ""), 2, "tests");
    }
    accepted(&table("[\"scripts/run-test\", \"{test}\"]", ""));
}

/// RD-1: the placeholders are closed: any brace that is neither one of the
/// six nor `{{` / `}}` is refused at its line, naming what it saw; the six
/// and the literal braces read. M: an unknown placeholder accepted (passed
/// through, or expanded empty).
#[test]
fn verify_tests_unknown_placeholder_is_refused() {
    for (element, words) in [
        ("{name}", "{name}"),
        ("{TEST}", "{TEST}"),
        ("{ test }", "{ test }"),
        ("--filter={test", "{"),
        ("x}", "}"),
        ("{}", "{}"),
        ("{test}}", "}"),
    ] {
        refused(
            &table(
                &format!("[\"scripts/run-test\", \"{element}\", \"{{test}}\"]"),
                "",
            ),
            2,
            words,
        );
    }
    accepted(&table(
        "[\"scripts/run-test\", \"{file}\", \"{qpath}\", \"{package}\", \"{target_kind}\", \"{target}\", \"{test}\"]",
        "",
    ));
    accepted(&table(
        "[\"scripts/run-test\", \"{{\", \"}}\", \"{{test}}\", \"--exact-name={test}\"]",
        "",
    ));
    // A placeholder anywhere in an element, element 0 too.
    accepted(&table("[\"run-{package}\", \"--name={qpath}\"]", ""));
}

/// RD-1: `{test}` or `{qpath}` is required somewhere in the template (a
/// literal `{{test}}` is no placeholder); an empty list is refused too.
/// M: a template without `{test}` or `{qpath}` accepted.
#[test]
fn verify_tests_needs_a_test_placeholder() {
    for value in [
        "[\"scripts/run-test\", \"{file}\", \"{package}\", \"{target}\"]",
        "[\"scripts/run-test\"]",
        "[\"scripts/run-test\", \"{{test}}\", \"{{qpath}}\"]",
        "[]",
    ] {
        refused(&table(value, ""), 2, "tests");
    }
    for value in [
        "[\"scripts/run-test\", \"{test}\"]",
        "[\"scripts/run-test\", \"{qpath}\"]",
        "[\"sh\", \"-c\", \"run \\\"$1\\\"\", \"sh\", \"{test}\"]",
    ] {
        accepted(&table(value, ""));
    }
}

/// RD-1: `timeout` is an integer of seconds, 1..86 400: 0, 86 401, a
/// negative number, a float and a string are refused at their line naming
/// the key; 1 and 86 400 read, with `mode` and `tests` beside. M: timeout
/// 0 or 86 401 accepted.
#[test]
fn verify_timeout_out_of_range_is_refused() {
    let tests = "[\"scripts/run-test\", \"{test}\"]";
    for value in ["0", "86401", "-1", "1.5", "\"600\"", "[600]"] {
        refused(&table(tests, &format!("timeout = {value}\n")), 3, "timeout");
    }
    for value in ["1", "600", "86400"] {
        accepted(&table(tests, &format!("timeout = {value}\n")));
    }
    let full = format!("[verify]\nmode = \"enforce\"\ntests = {tests}\ntimeout = 30\n");
    let config = verify_from_toml(&full).unwrap_or_else(|error| panic!("{full:?}: {error:?}"));
    assert_eq!(config.mode, Mode::Enforce, "the mode still reads");
}
