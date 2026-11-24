//! docs/features/write-cost.md (RM-0125) WC1, E's oracle: over every
//! fixture corpus and a seeded sample of 60 of this repository's documents,
//! every document is P under every family of the catalogue
//! (`tests/common/write_cost.rs`), and every new file the catalogue makes;
//! `PatchCheck::introduced_over`, its readers and holders from
//! `resolve_files` (recording on) and `given_keys`, gives exactly
//! `PatchCheck::introduced` over the whole input: the same findings, order
//! included. Twice: with a fresh `AsIs` per edit, and with one `AsIs` kept
//! across a corpus's edits (memo C, filled path by path).
//!
//! This repository's documents are read from `git ls-files` within its
//! `[paths]` (tracked files: the walk's own ignore rules aside), never
//! written.
//!
//! WC2, same file: generated corpora (`common::write_cost::generated`), six
//! fixed seeds of 20 to 200 documents, every reference form but `link_base`
//! and an index with shards; five drawn documents each under every family,
//! the corpus's new files, and the review's chain closers (a cycle whose
//! first member by name lies outside the set); at least 300 cases.
//!
//! The analyst's cycle cases (`common::write_cost::cycle_cases`: a split,
//! a duplicate-ID swap, a create closing a cycle through a `slug/ID` that
//! dangled, a new duplicate holder, P's liveness flip both ways, a
//! `parent-cycle` whose (path, position) and name orders disagree, longer
//! cycles whose first member lies outside the set), in a corpus of their own
//! and inside every generated one: the oracle, and each case's expected
//! finding (code, path, line, subject; or none), on both sides.
//!
//! WC1's mutations, in `crates/specengine-core/src/check/affected.rs`: the
//! set cut to "P plus files naming P's IDs", no hop 2, no holders, cycles over
//! the set only, no render on a title change; each red on at least one case.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use common::check::{Config, TODAY, fixture_input};
use common::repository_root;
use common::write_cost::{
    Doc, Draw, Edit, FAMILIES, GENERATED_CONFIG, catalogue, chain_closers, creates, cycle_cases,
    cycle_corpus, edits_of, generated, sample, unmet,
};
use specengine_core::check::{
    AsIs, Baseline, CheckFile, CheckInput, Dependents, EdgeKey, given_keys, resolve_files,
};
use specengine_core::patch::PatchCheck;

/// Who reads and who gives each key over one input: `resolve_files` with
/// recording on, and `given_keys`.
struct Keys {
    readers: BTreeMap<EdgeKey, BTreeSet<String>>,
    holders: BTreeMap<EdgeKey, BTreeSet<String>>,
}

impl Keys {
    fn of(config: &Config, input: &CheckInput) -> Self {
        let resolved = resolve_files(input, &config.scheme, &config.paths, None);
        let mut readers: BTreeMap<EdgeKey, BTreeSet<String>> = BTreeMap::new();
        for file in &resolved.files {
            let path = resolved.paths[file.file].to_owned();
            for key in &file.reads {
                readers.entry(key.clone()).or_default().insert(path.clone());
            }
        }
        let mut holders: BTreeMap<EdgeKey, BTreeSet<String>> = BTreeMap::new();
        for file in &input.files {
            for key in given_keys(file) {
                holders.entry(key).or_default().insert(file.path.clone());
            }
        }
        Self { readers, holders }
    }
}

impl Dependents for Keys {
    fn readers(&self, key: &EdgeKey) -> Vec<String> {
        self.readers
            .get(key)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn holders(&self, key: &EdgeKey) -> Vec<String> {
        self.holders
            .get(key)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }
}

/// The documents of `input`, as text (a file that is not UTF-8 is none).
fn docs_of(input: &CheckInput) -> Vec<Doc> {
    input
        .files
        .iter()
        .filter_map(|file| {
            Some(Doc {
                path: file.path.clone(),
                text: String::from_utf8(file.bytes.clone()).ok()?,
            })
        })
        .collect()
}

/// What one corpus's edits gave: the cases, the families met, the ones that
/// differ.
#[derive(Default)]
struct Tally {
    cases: usize,
    families: BTreeSet<&'static str>,
    differ: Vec<String>,
}

/// `edits` over `input`: the oracle against the set, fresh and shared C.
fn judge(label: &str, config: &Config, input: &CheckInput, edits: &[Edit], tally: &mut Tally) {
    let baseline = Baseline::empty();
    let check = PatchCheck {
        scheme: &config.scheme,
        paths: &config.paths,
        config: &config.check,
        baseline: &baseline,
        today: TODAY,
    };
    let keys = Keys::of(config, input);
    let mut shared = AsIs::default();
    for edit in edits {
        let bytes = edit.text.clone().into_bytes();
        let whole = check.introduced(input, &edit.path, bytes.clone());
        let fresh = check.introduced_over(
            input,
            &edit.path,
            bytes.clone(),
            &keys,
            &mut AsIs::default(),
        );
        let kept = check.introduced_over(input, &edit.path, bytes, &keys, &mut shared);
        tally.cases += 1;
        tally.families.insert(edit.family);
        for (side, got) in [("fresh C", &fresh), ("kept C", &kept)] {
            if format!("{got:?}") != format!("{whole:?}") {
                tally.differ.push(format!(
                    "{label} {} {} ({side}):\n  over the set: {got:?}\n  whole:        {whole:?}",
                    edit.family, edit.path
                ));
            }
        }
    }
}

/// Every fixture corpus's directory, in path order.
fn corpora() -> Vec<std::path::PathBuf> {
    let fixtures = repository_root().join("fixtures");
    let mut found = Vec::new();
    let mut dirs: Vec<_> = fs::read_dir(&fixtures)
        .expect("fixtures")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        if dir.join("specengine.toml").is_file() {
            found.push(dir.clone());
        }
        let mut inner: Vec<_> = fs::read_dir(&dir)
            .expect("a fixture")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir() && path.join("specengine.toml").is_file())
            .collect();
        inner.sort();
        found.extend(inner);
    }
    found
}

fn report(tally: &Tally, what: &str) {
    let missing: Vec<&&str> = FAMILIES
        .iter()
        .filter(|family| !tally.families.contains(**family))
        .collect();
    assert!(
        missing.is_empty(),
        "{what}: catalogue families never met: {missing:?}"
    );
    assert!(
        tally.differ.is_empty(),
        "{what}: {} of {} cases differ:\n{}",
        tally.differ.len(),
        tally.cases,
        tally
            .differ
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

/// WC1 over every fixture corpus: every document under every family, and
/// the corpus's new files.
#[test]
fn every_fixture_document_under_every_family() {
    let started = Instant::now();
    let mut tally = Tally::default();
    let corpora = corpora();
    // 32 since docs/features/reference-promotion.md (`promote-mini`).
    assert_eq!(corpora.len(), 32, "the fixture corpora");
    for corpus in &corpora {
        // A corpus whose walk reads no document (`corpus-mini`'s roots hold
        // none) gives no case.
        let text = fs::read_to_string(corpus.join("specengine.toml")).expect("a config");
        let walked = common::md_files(corpus)
            .iter()
            .any(|(path, _)| Config::from_toml(&text).paths.in_walk_scope(path));
        if !walked {
            continue;
        }
        let (config, input) = fixture_input(corpus);
        let docs = docs_of(&input);
        let label = corpus
            .strip_prefix(repository_root().join("fixtures"))
            .expect("a fixture")
            .display()
            .to_string();
        judge(&label, &config, &input, &catalogue(&docs), &mut tally);
    }
    eprintln!(
        "WC1 fixtures: {} cases, {} families, {:?}",
        tally.cases,
        tally.families.len(),
        started.elapsed()
    );
    report(&tally, "the fixtures");
}

/// This repository's walked, tracked documents: its config and `git
/// ls-files` within `[paths]`.
fn this_repository() -> (Config, CheckInput) {
    let root = repository_root();
    let text = fs::read_to_string(root.join("specengine.toml")).expect("specengine.toml");
    let config = Config::from_toml(&text);
    let listed = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["ls-files", "-z", "--", "*.md"])
        .output()
        .expect("git ls-files");
    assert!(listed.status.success(), "git ls-files");
    let mut paths: Vec<String> = listed
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .filter(|path| config.paths.in_walk_scope(path))
        .collect();
    paths.sort();
    let files = paths
        .into_iter()
        .filter_map(|path| {
            let bytes = fs::read(Path::new(&root).join(&path)).ok()?;
            Some(CheckFile::parse(path, bytes, &config.scheme))
        })
        .collect();
    (
        config,
        CheckInput {
            files,
            problems: Vec::new(),
        },
    )
}

/// WC1 over a seeded sample of 60 of this repository's documents, each P
/// under every family, inside the whole repository.
#[test]
#[ignore = "WC1's repository sample takes about 8 min in a debug build (960 cases over 581 \
            documents, 2026-11-15): run it with --run-ignored ignored-only"]
fn sixty_of_this_repositorys_documents_under_every_family() {
    let started = Instant::now();
    let (config, input) = this_repository();
    let docs = docs_of(&input);
    assert!(docs.len() > 300, "{} documents", docs.len());
    let mut edits = Vec::new();
    for at in sample(docs.len(), 60, 0x5EED_0060) {
        edits.extend(edits_of(&docs, at));
    }
    edits.extend(common::write_cost::creates(&docs).into_iter().take(12));
    let mut tally = Tally::default();
    judge("this repository", &config, &input, &edits, &mut tally);
    eprintln!(
        "WC1 this repository: {} cases over {} documents, {:?}",
        tally.cases,
        docs.len(),
        started.elapsed()
    );
    // A sample need not meet every family (`file-at-linked-path` needs a
    // dangling link): the families are the fixtures' test's to cover.
    assert!(
        tally.differ.is_empty(),
        "{} of {} cases differ:\n{}",
        tally.differ.len(),
        tally.cases,
        tally
            .differ
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

/// WC2: generated corpora under the catalogue. M: references dangling
/// today left out of the readers (a reader of a key no file gives).
#[test]
fn generated_corpora_under_the_catalogue() {
    let started = Instant::now();
    let config = Config::from_toml(GENERATED_CONFIG);
    let mut tally = Tally::default();
    let mut sizes = Vec::new();
    for seed in 1..=6_u64 {
        let count = 20 + Draw::new(seed * 7_919).below(181);
        let docs = generated(seed, count);
        sizes.push(docs.len());
        let input = CheckInput {
            files: docs
                .iter()
                .map(|doc| {
                    CheckFile::parse(
                        doc.path.clone(),
                        doc.text.clone().into_bytes(),
                        &config.scheme,
                    )
                })
                .collect(),
            problems: Vec::new(),
        };
        let mut edits: Vec<Edit> = sample(docs.len(), 5, seed)
            .into_iter()
            .flat_map(|at| edits_of(&docs, at))
            .collect();
        edits.extend(creates(&docs).into_iter().take(8));
        edits.extend(chain_closers(&docs));
        judge(
            &format!("generated seed {seed}"),
            &config,
            &input,
            &edits,
            &mut tally,
        );
    }
    eprintln!(
        "WC2: {} cases over corpora of {sizes:?} documents, {:?}",
        tally.cases,
        started.elapsed()
    );
    assert!(tally.cases >= 300, "{} cases", tally.cases);
    assert!(
        sizes.iter().all(|size| (20..=220).contains(size)),
        "{sizes:?}"
    );
    assert!(
        tally.differ.is_empty(),
        "{} of {} cases differ:\n{}",
        tally.differ.len(),
        tally.cases,
        tally
            .differ
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

/// `(code, path, line, subject)` of each finding.
fn tuples(found: &[specengine_core::check::Finding]) -> Vec<(String, String, usize, String)> {
    found
        .iter()
        .map(|finding| {
            (
                finding.code.clone(),
                finding.path.clone(),
                finding.line,
                finding.subject.clone(),
            )
        })
        .collect()
}

/// The analyst's cycle cases in a corpus of their own: the oracle (fresh and
/// kept C), and each case's expected finding on the whole check and on the
/// set. M: `parent-cycle` located by name; "introduced" by node set; P's
/// liveness flip left out; the region (rows) without both sides.
#[test]
fn cycle_cases_through_the_oracle_and_their_findings() {
    let config = Config::from_toml(GENERATED_CONFIG);
    let docs = cycle_corpus();
    let input = CheckInput {
        files: docs
            .iter()
            .map(|doc| {
                CheckFile::parse(
                    doc.path.clone(),
                    doc.text.clone().into_bytes(),
                    &config.scheme,
                )
            })
            .collect(),
        problems: Vec::new(),
    };
    let (_, edits, expects) = cycle_cases();
    let mut tally = Tally::default();
    judge("the cycle cases", &config, &input, &edits, &mut tally);
    report_differ(&tally);
    let baseline = Baseline::empty();
    let check = PatchCheck {
        scheme: &config.scheme,
        paths: &config.paths,
        config: &config.check,
        baseline: &baseline,
        today: TODAY,
    };
    let keys = Keys::of(&config, &input);
    let mut problems = Vec::new();
    for expect in &expects {
        let edit = edits
            .iter()
            .find(|edit| edit.family == expect.family)
            .expect("the case's edit");
        let bytes = edit.text.clone().into_bytes();
        let whole = tuples(&check.introduced(&input, &edit.path, bytes.clone()));
        let over =
            tuples(&check.introduced_over(&input, &edit.path, bytes, &keys, &mut AsIs::default()));
        for (side, found) in [("whole", &whole), ("over the set", &over)] {
            if let Some(problem) = unmet(expect, found) {
                problems.push(format!("{side}: {problem}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

fn report_differ(tally: &Tally) {
    assert!(
        tally.differ.is_empty(),
        "{} of {} cases differ:\n{}",
        tally.differ.len(),
        tally.cases,
        tally
            .differ
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}
