//! docs/features/norm-hash.md TE-12: the meaning hash over the pilots,
//! manual (`#[ignore]`, foreground): the walk the index takes (the
//! scheme's `[paths]`), every file parsed in process, every node's
//! `norm_input` and `revision`; the node, hashed, null and
//! unknown-revision counts, the null documents by finding, the `bad-rev`
//! findings; a second pass gives the same bytes. Nothing is written: no
//! index, no `HOME` use, and the read-only proof (`git status` and the
//! listing of the scheme's roots) is equal before and after.
//!
//! Run: `SPECENGINE_PILOT_A=… [SPECENGINE_SCHEME_A=…] cargo nextest run -p
//! specengine-eval --test norm_pilot --run-ignored only --no-capture`; it
//! fails, never skips, naming an unset variable. Nothing here names a pilot.

mod pilot;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use specengine_core::{IdSchemeToml, Paths, Revision, norm_input, revision};
use specengine_model::{DiagnosticCode, IdScheme};
use specengine_store::{Source, WorkingTree};

/// The label's corpus and scheme: the scheme variable when set, else the
/// corpus's own `specengine.toml`.
fn setup(label: &str) -> (PathBuf, PathBuf, &'static str) {
    let variables = pilot::variables(label);
    let corpus = pilot::required(variables.corpus, "the pilot's worktree");
    let scheme =
        pilot::optional(variables.scheme).unwrap_or_else(|| corpus.join("specengine.toml"));
    assert!(
        scheme.is_file(),
        "set {} (or give the corpus a specengine.toml): no scheme at the corpus root",
        variables.scheme
    );
    (corpus, scheme, variables.scheme)
}

/// `(path, ord)` -> the node's `norm_input` (`None` null).
type Inputs = BTreeMap<(String, usize), Option<Vec<u8>>>;

#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    files: usize,
    nodes: usize,
    hashed: usize,
    null: usize,
    null_by_code: BTreeMap<String, usize>,
    not_utf8_files: usize,
    rev_written: usize,
    rev_unknown: usize,
    bad_rev: usize,
}

/// One pass over the walk: the counts and every node's input bytes.
fn pass(corpus: &Path, scheme: &IdScheme, paths: &Paths) -> (Counts, Inputs) {
    let tree = WorkingTree::new(corpus, paths).expect("the worktree");
    let listing = tree.list().expect("the walk");
    let mut counts = Counts::default();
    let mut inputs = BTreeMap::new();
    for path in &listing.paths {
        let bytes = tree.read(path).expect("a listed file");
        let parsed = specengine_core::parse(path, &bytes, scheme);
        counts.files += 1;
        counts.not_utf8_files += usize::from(parsed.nodes.is_empty());
        counts.bad_rev += parsed
            .diagnostics
            .iter()
            .filter(|found| found.code == DiagnosticCode::BadRev)
            .count();
        for ord in 0..parsed.nodes.len() {
            counts.nodes += 1;
            let input = norm_input(&parsed, &bytes, ord);
            if input.is_some() {
                counts.hashed += 1;
            } else {
                counts.null += 1;
                let codes: Vec<String> = parsed
                    .diagnostics
                    .iter()
                    .map(|found| found.code.as_str().to_owned())
                    .filter(|code| {
                        [
                            "frontmatter-unclosed",
                            "frontmatter-yaml",
                            "frontmatter-not-mapping",
                            "frontmatter-type",
                            "unparsed-reference",
                        ]
                        .contains(&code.as_str())
                    })
                    .collect();
                assert_eq!(ord, 0, "{path}: only a document is ever null ({codes:?})");
                for code in codes {
                    *counts.null_by_code.entry(code).or_default() += 1;
                }
            }
            match revision(&parsed, ord) {
                Revision::Written(_) => counts.rev_written += 1,
                Revision::Unreadable => counts.rev_unknown += 1,
                Revision::Unwritten => {}
            }
            inputs.insert((path.clone(), ord), input);
        }
    }
    (counts, inputs)
}

fn run(label: &str) {
    let (corpus, scheme_path, scheme_variable) = setup(label);
    let text = std::fs::read_to_string(&scheme_path).expect("the scheme");
    let scheme = IdScheme::from_toml(&text)
        .unwrap_or_else(|error| panic!("{scheme_variable}: {}", error.at("the scheme")));
    let paths = Paths::from_toml(&text)
        .unwrap_or_else(|error| panic!("{scheme_variable}: {}", error.at("the scheme")));
    let roots = pilot::scheme_roots(&scheme_path, scheme_variable);
    let before = pilot::proof(&corpus, &roots);
    let started = std::time::Instant::now();
    let (counts, inputs) = pass(&corpus, &scheme, &paths);
    let elapsed = started.elapsed();
    let (again, inputs_again) = pass(&corpus, &scheme, &paths);
    let after = pilot::proof(&corpus, &roots);
    eprintln!("TE-12 {label}: {counts:?} in {elapsed:?}");
    assert_eq!(before, after, "{label}: the pilot is left as it was");
    assert_eq!(again, counts, "{label}: a second pass counts the same");
    assert!(
        inputs == inputs_again,
        "{label}: a second pass gives the same bytes"
    );
    assert!(counts.files > 0 && counts.nodes > 0, "{label}: {counts:?}");
    assert_eq!(counts.hashed + counts.null, counts.nodes);
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_A; read-only, in-process run over a real corpus"]
fn te12_pilot_a() {
    run("pilot-a");
}

#[test]
#[ignore = "needs SPECENGINE_PILOT_B; read-only, in-process run over a real corpus"]
fn te12_pilot_b() {
    run("pilot-b");
}
