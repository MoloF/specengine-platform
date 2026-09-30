//! AC-08 of docs/features/spec-check-links.md: the census differential.
//! On `fixtures/corpus-mini`, checked with `[paths] roots = ["design"]` (a
//! config written by the test outside the fixture, `[ids]` the fixture's
//! own), the census's `broken_links` and the check's `link-dangling`
//! findings are the same set `(path, line, target as written)`: exactly
//! `design/sections.md:10 missing.md`; `design/notes.md`'s
//! `sections.md#a-rule-written-as-a-section` resolves, no `link-anchor`.
//! A scratch copy with more `.md` links keeps the two sets equal. The
//! fixture is only read.

#![cfg(unix)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use specengine_core::check::Report;
use specengine_import::CensusConfig;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

fn corpus_mini() -> PathBuf {
    repository_root().join("fixtures").join("corpus-mini")
}

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "specengine-eval-links-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(fs::canonicalize(&path).expect("canonical scratch"))
    }

    fn join(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git_status_fixtures() -> String {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--", "fixtures/"])
        .current_dir(repository_root())
        .output()
        .expect("git runs");
    String::from_utf8(output.stdout).expect("UTF-8")
}

/// The fixture's `[ids]` and `[paths] roots = ["design"]`, written to
/// `<scratch>/specengine.toml`.
fn check_config(scratch: &Scratch, root: &Path) -> PathBuf {
    let ids = fs::read_to_string(root.join("specengine.toml")).expect("specengine.toml");
    let path = scratch.join("specengine.toml");
    fs::write(&path, format!("{ids}\n[paths]\nroots = [\"design\"]\n")).expect("config");
    path
}

type Triple = (String, usize, String);

fn census_broken(root: &Path) -> BTreeSet<Triple> {
    let config = CensusConfig::load(&corpus_mini().join("census.toml"))
        .unwrap_or_else(|error| panic!("census.toml: {error:?}"));
    let census = specengine_import::run(root, &config).expect("census runs");
    census
        .broken_links
        .iter()
        .map(|link| (link.path.clone(), link.line, link.target.clone()))
        .collect()
}

fn check(root: &Path, config: &Path) -> Report {
    specengine_store::check_worktree(root, config, None, "2026-09-29")
}

fn dangling(report: &Report) -> BTreeSet<Triple> {
    report
        .findings
        .iter()
        .filter(|f| f.code == "link-dangling")
        .map(|f| (f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

#[test]
fn corpus_mini_census_and_check_agree_on_the_one_broken_link() {
    let before = git_status_fixtures();
    let scratch = Scratch::new("mini");
    let root = corpus_mini();
    let config = check_config(&scratch, &root);
    let census = census_broken(&root);
    let report = check(&root, &config);
    assert_eq!(report.counts.documents, 3, "{:#?}", report.lines(true));
    let want: BTreeSet<Triple> = [("design/sections.md".to_owned(), 10, "missing.md".to_owned())]
        .into_iter()
        .collect();
    assert_eq!(census, want, "the census");
    assert_eq!(dangling(&report), want, "{:#?}", report.lines(true));
    assert!(
        report.findings.iter().all(|f| f.code != "link-anchor"),
        "`sections.md#a-rule-written-as-a-section` resolves: {:#?}",
        report.lines(true)
    );
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "link-dangling")
        .expect("one");
    assert_eq!(
        finding.message,
        "`missing.md` names no walked document (tried `design/missing.md`)"
    );
    assert_eq!(git_status_fixtures(), before, "the fixture is only read");
}

#[test]
fn more_md_links_keep_the_census_and_the_check_equal() {
    let scratch = Scratch::new("more");
    let root = scratch.join("corpus");
    copy_dir(&corpus_mini(), &root);
    fs::create_dir_all(root.join("design/sub")).unwrap();
    fs::write(
        root.join("design/sub/more.md"),
        "---\nkind: note\n---\n\n# More\n\n\
         [a](../rules.md) and [b](../gone.md).\n\
         [c](none.md) and [d](../sub/../notes.md).\n\
         [e](gone%20x.md) and [f](/design/rules.md).\n\
         [g](/design/nothing.md) and [h](../sections.md#nope).\n\
         [i](../../design/rules.md) and [j](deep/x.md?plain=1).\n",
    )
    .unwrap();
    let config = check_config(&scratch, &root);
    let census = census_broken(&root);
    let report = check(&root, &config);
    let checked = dangling(&report);
    assert_eq!(checked, census, "{:#?}", report.lines(true));
    let subjects: Vec<&str> = checked
        .iter()
        .filter(|(path, ..)| path == "design/sub/more.md")
        .map(|(_, _, subject)| subject.as_str())
        .collect();
    assert_eq!(
        subjects,
        [
            "../gone.md",
            "none.md",
            "gone%20x.md",
            "/design/nothing.md",
            "deep/x.md?plain=1"
        ],
        "by line"
    );
    // A resolved target with a wrong anchor is no broken link.
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "link-anchor" && f.subject == "../sections.md#nope"),
        "{:#?}",
        report.lines(true)
    );
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}
