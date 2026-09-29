//! AC-18 of docs/features/spec-check.md: genre independence (ADR-0008).
//! The same check, each fixture read through its own `specengine.toml`
//! alone, blocks on exactly the pinned findings: spec-a (a game) on the
//! `frontmatter-type` of `docs/features/stamina-tuning.md`; spec-b (a
//! command-line tool, Russian prose, Cyrillic legacy prefixes) on the
//! homoglyphs of `docs/spec/cli.md` and `docs/records/QN/QN-08.md`, the
//! latter's `frontmatter-yaml`, and the two aliases without a target (AC-10);
//! so no `class-missing` (every fixture document declares its class per the
//! Data rule "Class per record kind").
//! The check source names no prefix, path or file of this repository or a
//! fixture.
//!
//! AC-13 of docs/features/spec-check-graph.md: the blocking sets above are
//! unchanged (no registry in either fixture: §11.5–6 off); the new warnings
//! are exactly spec-a's `depends-cycle` and spec-b's `mention-dangling`; the
//! new sources (renderer, registry, resolution, graph rules) are scanned
//! with the old, and name no `cargo xtask` either.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::check::{blocking, fixture_input, show};
use common::{corpus_scheme, fixture, repository_root};
use specengine_core::check::Verdict;

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(p, c)| ((*p).to_owned(), (*c).to_owned()))
        .collect()
}

/// One function for both corpora.
fn blocking_of(corpus: &str) -> (Vec<(String, String)>, Verdict) {
    let (config, input) = fixture_input(&fixture(corpus));
    let report = config.run(&input);
    eprintln!("{corpus}:\n{}", show(&report));
    assert!(
        report.findings.iter().all(|f| f.code != "class-missing"),
        "{corpus}: every document declares its class:\n{}",
        show(&report)
    );
    (blocking(&report), report.verdict)
}

#[test]
fn spec_a_blocks_exactly_on_its_type_error() {
    let (found, verdict) = blocking_of("spec-a");
    assert_eq!(
        found,
        pairs(&[("docs/features/stamina-tuning.md", "frontmatter-type")])
    );
    assert_eq!(verdict, Verdict::Blocked);
}

#[test]
fn spec_b_blocks_exactly_on_its_homoglyphs_yaml_and_dangling_aliases() {
    let (found, verdict) = blocking_of("spec-b");
    assert_eq!(
        found,
        pairs(&[
            ("docs/features/dry-run.md", "ref-dangling"),
            ("docs/records/QN/QN-07.md", "ref-dangling"),
            ("docs/records/QN/QN-08.md", "frontmatter-yaml"),
            ("docs/records/QN/QN-08.md", "homoglyph"),
            ("docs/spec/cli.md", "homoglyph"),
        ])
    );
    assert_eq!(verdict, Verdict::Blocked);
}

/// `(code, path, line, subject)` of the findings of increment 2's codes.
fn increment_2(corpus: &str) -> Vec<(String, String, usize, String)> {
    let (config, input) = fixture_input(&fixture(corpus));
    let report = config.run(&input);
    report
        .findings
        .iter()
        .filter(|f| {
            [
                "index-missing",
                "index-drift",
                "generator-unknown",
                "generator-path",
                "mention-dangling",
                "depends-cycle",
                "ref-superseded",
            ]
            .contains(&f.code.as_str())
        })
        .inspect(|f| assert_eq!(f.severity, specengine_model::Severity::Warning))
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

#[test]
fn the_new_warnings_are_one_per_fixture() {
    assert_eq!(
        increment_2("spec-a"),
        [(
            "depends-cycle".to_owned(),
            "docs/spec/movement/sprint.md".to_owned(),
            9,
            "MEC-SPRINT, MEC-STAMINA".to_owned()
        )]
    );
    assert_eq!(
        increment_2("spec-b"),
        [(
            "mention-dangling".to_owned(),
            "docs/spec/cli.md".to_owned(),
            25,
            "R\u{0415}Q-003".to_owned()
        )]
    );
}

/// Every string literal of a Rust line outside comments (naive: the text
/// between pairs of `"`; enough for the check sources).
fn literals(line: &str) -> Vec<&str> {
    let code = line.trim_start();
    if code.starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

fn check_sources() -> Vec<std::path::PathBuf> {
    let mut sources = Vec::new();
    let dir = repository_root().join("crates/specengine-core/src/check");
    for entry in fs::read_dir(&dir).expect("the check module is a directory") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            sources.push(path);
        }
    }
    sources.push(repository_root().join("crates/specengine-store/src/check.rs"));
    sources.sort();
    sources
}

/// Paths and file names of this repository and of the fixtures, and this
/// repository's generator command.
const PROJECT_NAMES: &[&str] = &[
    "cargo xtask",
    "xtask",
    "CLAUDE.md",
    "README.md",
    "index.md",
    "architecture.md",
    "docs/",
    "fixtures",
    "spec-a",
    "spec-b",
    "decisions",
    "records/",
    "features/",
];

#[test]
fn the_check_source_names_no_prefix_path_or_file_of_a_project() {
    let mut prefixes: BTreeSet<String> = BTreeSet::new();
    for corpus in ["spec-a", "spec-b"] {
        for spec in corpus_scheme(&fixture(corpus)).prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
        }
    }
    // This repository's own scheme: `[ids] ADR` (spec, Data).
    prefixes.insert("ADR".to_owned());
    let mut offenders = Vec::new();
    let sources = check_sources();
    assert!(sources.len() >= 11, "{sources:?}");
    for new in ["render.rs", "generated.rs", "graph.rs", "resolve.rs"] {
        assert!(
            sources.iter().any(|path| path.ends_with(new)),
            "{new} is scanned: {sources:?}"
        );
    }
    for path in &sources {
        let text = fs::read_to_string(path).unwrap();
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                let names_prefix = prefixes.iter().any(|prefix| {
                    literal == prefix
                        || literal.starts_with(&format!("{prefix}-"))
                        || literal.contains(&format!(" {prefix}-"))
                        || literal.contains(&format!("`{prefix}-"))
                });
                let names_path = PROJECT_NAMES.iter().any(|name| literal.contains(name));
                if names_prefix || names_path {
                    offenders.push(format!(
                        "{}:{}: {:?} in {}",
                        relative(path),
                        number + 1,
                        literal,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "project names in the check source:\n{}",
        offenders.join("\n")
    );
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .unwrap_or(path)
        .display()
        .to_string()
}
