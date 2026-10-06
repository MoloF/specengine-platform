//! Helpers of the `spec check` tests (docs/features/spec-check.md): a
//! `specengine.toml` read through the real TOML readers, a set of files
//! parsed by the real parser into a `CheckInput`, and compact views of the
//! report. Everything runs in memory: the check reads no file.

use std::path::Path;

use specengine_core::check::{self, Baseline, CheckConfig, CheckFile, CheckInput, Finding, Report};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::IdScheme;

use super::walked_md_files;

/// The date every test runs on unless it says otherwise.
pub const TODAY: &str = "2026-09-29";

/// `[ids]`, `[paths]` and the check tables of one `specengine.toml`.
pub struct Config {
    pub scheme: IdScheme,
    pub paths: Paths,
    pub check: CheckConfig,
}

impl Config {
    pub fn from_toml(text: &str) -> Self {
        Self {
            scheme: IdScheme::from_toml(text)
                .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml"))),
            paths: Paths::from_toml(text)
                .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml"))),
            check: CheckConfig::from_toml(text)
                .unwrap_or_else(|error| panic!("{}", error.at("specengine.toml"))),
        }
    }

    /// Parses every `(path, text)` with this scheme.
    pub fn input(&self, files: &[(&str, &str)]) -> CheckInput {
        CheckInput {
            files: files
                .iter()
                .map(|(path, text)| CheckFile::parse(*path, text.as_bytes().to_vec(), &self.scheme))
                .collect(),
            problems: Vec::new(),
        }
    }

    pub fn run(&self, input: &CheckInput) -> Report {
        self.run_with(input, &Baseline::empty(), TODAY)
    }

    pub fn run_with(&self, input: &CheckInput, baseline: &Baseline, today: &str) -> Report {
        check::run(
            input,
            &self.scheme,
            &self.paths,
            &self.check,
            baseline,
            today,
        )
    }

    /// Parses and checks `files` with an empty baseline, today [`TODAY`].
    pub fn check(&self, files: &[(&str, &str)]) -> Report {
        self.run(&self.input(files))
    }
}

/// A fixture corpus: its own `specengine.toml` and every `.md` file of
/// its walk ([`walked_md_files`]: a record template outside the roots is
/// none), parsed (the fixture is only read).
pub fn fixture_input(corpus: &Path) -> (Config, CheckInput) {
    let text = std::fs::read_to_string(corpus.join("specengine.toml")).expect("specengine.toml");
    let config = Config::from_toml(&text);
    let input = CheckInput {
        files: walked_md_files(corpus)
            .into_iter()
            .map(|(path, bytes)| CheckFile::parse(path, bytes, &config.scheme))
            .collect(),
        problems: Vec::new(),
    };
    (config, input)
}

/// `(path, code, subject)` of every finding.
pub fn triples(report: &Report) -> Vec<(String, String, String)> {
    report
        .findings
        .iter()
        .map(|f| (f.path.clone(), f.code.clone(), f.subject.clone()))
        .collect()
}

/// `(path, code)` of every finding that blocks under `enforce`.
pub fn blocking(report: &Report) -> Vec<(String, String)> {
    report
        .findings
        .iter()
        .filter(|f| f.blocks_when_enforced())
        .map(|f| (f.path.clone(), f.code.clone()))
        .collect()
}

/// The findings of one code.
pub fn with_code<'r>(report: &'r Report, code: &str) -> Vec<&'r Finding> {
    report.findings.iter().filter(|f| f.code == code).collect()
}

/// The codes of every finding, in report order.
pub fn codes(report: &Report) -> Vec<&str> {
    report.findings.iter().map(|f| f.code.as_str()).collect()
}

/// A one-line rendering of every finding, for assertion messages.
pub fn show(report: &Report) -> String {
    report
        .findings
        .iter()
        .map(|f| {
            format!(
                "{:?} {}:{} {} [{}] {}",
                f.severity, f.path, f.line, f.code, f.subject, f.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
