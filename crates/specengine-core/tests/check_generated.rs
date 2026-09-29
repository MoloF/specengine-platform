//! AC-05 of docs/features/spec-check-graph.md (the rule half), §11.6 of
//! the documentation convention: with a `[[generators]]` table (even an
//! empty one) every `class: generated` document names a registered
//! `command` in `generator:`, else `generator-unknown` (subject the value,
//! `""` when absent; line the `generator:` line, else 1), and its path is in
//! that entry's `writes`, else `generator-path` (the message names the
//! entry's `writes`). Without the table the rules are off. A failed
//! front-matter gives only its parser findings. Both codes are errors.
//!
//! The configuration half is in `check_config.rs`, the re-parse half in the
//! store's `tests/check_config.rs`.

mod common;

use common::check::{Config, show, with_code};
use specengine_core::check::{Report, Verdict};
use specengine_model::Severity;

const IDS: &str = "[ids]\nR = { kind = \"requirement\", width = 2 }\n";

const REGISTRY: &str = "
[[generators]]
command = \"make toc\"
writes  = [\"out/toc.md\", \"out/list.md\"]

[[generators]]
command = \"tool gen --all\"
writes  = [\"gen/all.md\"]
";

fn with_registry() -> Config {
    Config::from_toml(&format!("{IDS}{REGISTRY}"))
}

fn without_registry() -> Config {
    Config::from_toml(IDS)
}

fn generated(generator: Option<&str>) -> String {
    match generator {
        Some(value) => format!("---\nclass: generated\nsource: x\ngenerator: {value}\n---\n# G\n"),
        None => "---\nclass: generated\nsource: x\n---\n# G\n".to_owned(),
    }
}

/// `(code, path, line, subject)` of the generator findings.
fn generator_findings(report: &Report) -> Vec<(String, String, usize, String)> {
    report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("generator-"))
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

fn one(code: &str, path: &str, line: usize, subject: &str) -> Vec<(String, String, usize, String)> {
    vec![(code.to_owned(), path.to_owned(), line, subject.to_owned())]
}

#[test]
fn without_the_table_the_rules_are_off() {
    let config = without_registry();
    let foo = generated(Some("foo"));
    let bare = generated(None);
    let report = config.check(&[("out/a.md", &foo), ("out/b.md", &bare)]);
    assert!(generator_findings(&report).is_empty(), "{}", show(&report));
    assert_eq!(report.verdict, Verdict::Clean, "{}", show(&report));
}

#[test]
fn an_unknown_generator_is_generator_unknown_on_its_line() {
    let config = with_registry();
    let foo = generated(Some("foo"));
    let report = config.check(&[("out/toc.md", &foo)]);
    assert_eq!(
        generator_findings(&report),
        one("generator-unknown", "out/toc.md", 4, "foo"),
        "{}",
        show(&report)
    );
    let found = with_code(&report, "generator-unknown");
    assert_eq!(found[0].severity, Severity::Error);
    assert!(found[0].message.contains("foo"), "{}", found[0].message);
    assert!(report.blocks(found[0]));
    assert_eq!(report.verdict, Verdict::Blocked);
}

#[test]
fn a_missing_generator_is_generator_unknown_with_an_empty_subject_at_line_1() {
    let config = with_registry();
    let bare = generated(None);
    let report = config.check(&[("out/toc.md", &bare)]);
    assert_eq!(
        generator_findings(&report),
        one("generator-unknown", "out/toc.md", 1, ""),
        "{}",
        show(&report)
    );
}

#[test]
fn a_registered_command_is_compared_byte_for_byte() {
    let config = with_registry();
    for value in [
        "make  toc",
        "Make toc",
        "make toc --write",
        "tool gen",
        "\"make toc \"",
    ] {
        let text = generated(Some(value));
        let report = config.check(&[("out/toc.md", &text)]);
        let found = generator_findings(&report);
        assert_eq!(found.len(), 1, "{value:?}:\n{}", show(&report));
        assert_eq!(found[0].0, "generator-unknown", "{value:?}");
    }
}

#[test]
fn a_registered_command_writing_the_path_is_clean() {
    let config = with_registry();
    let toc = generated(Some("make toc"));
    let all = generated(Some("tool gen --all"));
    let report = config.check(&[
        ("out/toc.md", &toc),
        ("out/list.md", &toc),
        ("gen/all.md", &all),
    ]);
    assert!(report.findings.is_empty(), "{}", show(&report));
    assert_eq!(report.verdict, Verdict::Clean);
}

#[test]
fn a_registered_command_not_writing_the_path_is_generator_path() {
    let config = with_registry();
    let toc = generated(Some("make toc"));
    let all = generated(Some("tool gen --all"));
    let report = config.check(&[("out/other.md", &toc), ("out/toc.md", &all)]);
    assert_eq!(
        generator_findings(&report),
        vec![
            (
                "generator-path".to_owned(),
                "out/other.md".to_owned(),
                4,
                "make toc".to_owned()
            ),
            (
                "generator-path".to_owned(),
                "out/toc.md".to_owned(),
                4,
                "tool gen --all".to_owned()
            ),
        ],
        "{}",
        show(&report)
    );
    let found = with_code(&report, "generator-path");
    assert_eq!(
        found[0].message, "`make toc` writes out/toc.md, out/list.md, not this file",
        "the message names the entry's `writes`"
    );
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(report.verdict, Verdict::Blocked);
}

#[test]
fn an_empty_table_turns_the_rules_on() {
    let config = Config::from_toml(&format!("generators = []\n{IDS}"));
    assert_eq!(config.check.generators, Some(Vec::new()));
    let toc = generated(Some("make toc"));
    let report = config.check(&[("out/toc.md", &toc)]);
    assert_eq!(
        generator_findings(&report),
        one("generator-unknown", "out/toc.md", 4, "make toc"),
        "{}",
        show(&report)
    );
}

#[test]
fn only_generated_documents_are_judged() {
    let config = with_registry();
    let canon = "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\ngenerator: foo\n---\n# C\n";
    let spec = "---\nclass: spec\nstatus: draft\nscope: [x]\ngenerator: foo\n---\n# S\n";
    let classless = "---\ngenerator: foo\n---\n# N\n";
    let report = config.check(&[
        ("docs/c.md", canon),
        ("docs/s.md", spec),
        ("docs/n.md", classless),
    ]);
    assert!(generator_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn a_failed_front_matter_gives_only_its_parser_findings() {
    let config = with_registry();
    for (code, text) in [
        (
            "frontmatter-yaml",
            "---\nclass: generated\ngenerator: foo\ntitle: a: b\n---\n# G\n",
        ),
        (
            "frontmatter-unclosed",
            "---\nclass: generated\ngenerator: foo\n# G\n",
        ),
        ("frontmatter-not-mapping", "---\n- generated\n---\n# G\n"),
    ] {
        let report = config.check(&[("out/elsewhere.md", text)]);
        let errors: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.code.as_str())
            .collect();
        assert_eq!(errors, [code], "{}", show(&report));
        assert!(generator_findings(&report).is_empty(), "{}", show(&report));
    }
}

#[test]
fn the_findings_are_independent_of_input_order() {
    let config = with_registry();
    let foo = generated(Some("foo"));
    let toc = generated(Some("make toc"));
    let bare = generated(None);
    let files = [
        ("out/z.md", foo.as_str()),
        ("out/a.md", toc.as_str()),
        ("gen/b.md", bare.as_str()),
        ("out/toc.md", toc.as_str()),
    ];
    let forward = config.check(&files);
    let mut reversed = files;
    reversed.reverse();
    let backward = config.check(&reversed);
    assert_eq!(forward.lines(true), backward.lines(true));
    assert_eq!(forward.to_json(), backward.to_json());
    assert_eq!(generator_findings(&forward).len(), 3, "{}", show(&forward));
}

/// Iterations 3–4, end to end: a registry the TOML reader accepts must pass
/// its own check once the index is the render — the rendered `generator:`
/// line must read back as the registered command. (Values serde-saphyr
/// reads as a boolean, a null or a number are rejected by the reader:
/// `check_config.rs`.)
#[test]
fn an_accepted_index_command_passes_its_own_check() {
    use specengine_core::check::render_index;
    let mut broken = Vec::new();
    for value in [
        "make docs",
        "v1.0",
        "1e",
        "0x",
        "nan",
        "inf",
        "yes",
        "1:30",
        "1__0",
        "NaN",
    ] {
        let toml = format!(
            "{IDS}\n[paths]\nindex = \"out/index.md\"\n\n[[generators]]\ncommand = \"{value}\"\nwrites = [\"out/index.md\"]\nindex = true\n"
        );
        let config = Config::from_toml(&toml);
        let generator = config.check.index_generator().expect("the index entry");
        let render = render_index(&config.input(&[]), "out/index.md", generator);
        let report = config.check(&[("out/index.md", &render)]);
        if !report.findings.is_empty() {
            broken.push(format!(
                "{value:?}:\n  {}",
                show(&report).replace('\n', "\n  ")
            ));
        }
    }
    assert!(broken.is_empty(), "{}", broken.join("\n"));
}
