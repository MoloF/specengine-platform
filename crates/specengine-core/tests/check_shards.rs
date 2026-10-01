//! docs/features/index-shards.md (ADR-0030), core part, over crafted inputs
//! built in memory (no project path: the root is `a/index.md`, the archive
//! shard `a/b/arch.md`, a live shard `n/claimed.md`).
//!
//! AC-02: with an archive shard the root has no Tier 3 line and ends with
//! one pointer linked from its directory (`b/arch.md`); the shard holds
//! exactly the Tier 3 documents (by status, never by folder) as compact
//! lines in path byte order, linked from `a/b/`, under the header naming the
//! registered command and gate, its back link `../index.md`. The root is
//! the unsharded render with its Archive section replaced by `## Shards`.
//!
//! AC-03: every walked document but `class: generated` is on exactly one
//! line across the outputs; no output is listed, though the root is
//! pre-seeded with `class: canon` front-matter and the shard with an
//! unclosed one.
//!
//! AC-05: §11.5 per output: a byte edited on line k of a shard is one
//! `index-drift` on the shard at k, the root clean; a shard not walked is
//! `index-missing` on its path; a root edit drifts the root only; both
//! edited → both reported; a `walk_gap` → no comparison at all.
//!
//! AC-11 (determinism): reversed input → identical bytes for every output.

mod common;

use std::collections::BTreeMap;

use common::check::{Config, show};
use specengine_core::check::{
    CheckFile, CheckInput, IndexOutput, Report, Verdict, is_tier3_file, render_index,
    render_index_set,
};

const ROOT: &str = "a/index.md";
const ARCH: &str = "a/b/arch.md";
const CLAIMED: &str = "n/claimed.md";
const ARCHIVE_HEADING: &str = "## Archive \u{2014} Tier 3, by id only";

/// The archive-shard config of AC-02.
const TOML: &str = "\
[paths]
index = \"a/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[[generators]]
command = \"make index\"
writes  = [\"a/index.md\", \"a/b/arch.md\"]
index   = true
gate    = \"make gate\"
shards  = [{ path = \"a/b/arch.md\", tier3 = true }]
";

/// The same entry without a shard.
const TOML_PLAIN: &str = "\
[paths]
index = \"a/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[[generators]]
command = \"make index\"
writes  = [\"a/index.md\"]
index   = true
gate    = \"make gate\"
";

/// An archive shard and a live shard claiming `n/**`.
const TOML_TWO: &str = "\
[paths]
index = \"a/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[[generators]]
command = \"make index\"
writes  = [\"a/index.md\", \"a/b/arch.md\", \"n/claimed.md\"]
index   = true
gate    = \"make gate\"
shards  = [
  { path = \"a/b/arch.md\", tier3 = true },
  { path = \"n/claimed.md\", claims = [\"n/**\"] },
]
";

/// Every kind the placement tells apart: live canon, decisions, specs; a
/// failed front-matter; a generated file; Tier 3 by status in three
/// folders, none named `archive`; a live spec in a folder named `archive`.
fn corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "top.md",
            "---\nclass: canon\ntier: 0\nscope: [all]\n---\n\n# Top rules\n",
        ),
        (
            "a/c/canon.md",
            "---\nclass: canon\ntier: 2\nscope: [x]\n---\n\n# A canon\n",
        ),
        (
            "a/d/ADR-0001.md",
            "---\nid: ADR-0001\nclass: decision\ntitle: One\nstatus: accepted\nscope: [x]\n---\n\n# One\n",
        ),
        (
            "a/d/ADR-0002.md",
            "---\nid: ADR-0002\nclass: decision\ntitle: Two\nstatus: superseded-by ADR-0001\nscope: [x]\n---\n\n# Two\n",
        ),
        (
            "a/s/live.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# Live spec\n",
        ),
        (
            "a/s/old.md",
            "---\nclass: spec\nstatus: shipped\nscope: [x]\n---\n\n# Old spec\n",
        ),
        (
            "z/abandoned.md",
            "---\nclass: spec\nstatus: abandoned\nscope: [x]\n---\n\n# Gone\n",
        ),
        (
            "a/archive/kept.md",
            "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# Kept in an archive folder\n",
        ),
        (
            "a/n/broken.md",
            "---\nclass: spec\nstatus: shipped\n\n# Never closed\n",
        ),
        (
            "a/g/gen.md",
            "---\nclass: generated\ngenerator: make other\nsource: x\n---\n\n# Generated\n",
        ),
        (
            "n/live.md",
            "---\nclass: canon\ntier: 2\nscope: [n]\n---\n\n# Claimed canon\n",
        ),
        (
            "n/old.md",
            "---\nclass: spec\nstatus: shipped\nscope: [n]\n---\n\n# Claimed but shipped\n",
        ),
    ]
}

/// The outputs pre-seeded with front-matter that would list them if the
/// renderer excluded by class: the root `class: canon`, the shard unclosed.
fn seeded_outputs() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            ROOT,
            "---\nclass: canon\ntier: 2\nscope: [x]\n---\n\n# Not the index yet\n",
        ),
        (ARCH, "---\nclass: generated\n\n# Unclosed\n"),
        (
            CLAIMED,
            "---\nclass: spec\nstatus: draft\nscope: [n]\n---\n\n# Not the shard yet\n",
        ),
    ]
}

fn input(config: &Config, files: &[(&str, &str)]) -> CheckInput {
    config.input(files)
}

fn render_set(config: &Config, input: &CheckInput) -> Vec<IndexOutput> {
    let generator = config.check.index_generator().expect("the index entry");
    render_index_set(input, ROOT, generator)
}

fn output<'o>(outputs: &'o [IndexOutput], path: &str) -> &'o str {
    &outputs
        .iter()
        .find(|o| o.path == path)
        .unwrap_or_else(|| panic!("no output {path}"))
        .bytes
}

/// `link` from the directory of `from`, root-relative.
fn resolve(from: &str, link: &str) -> String {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in link.split('/') {
        match part {
            ".." => {
                assert!(parts.pop().is_some(), "{link} from {from} leaves the root");
            }
            "." | "" => {}
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// `(output, heading, target)` of every entry line of every output, the
/// `## Shards` pointers aside.
fn entries(outputs: &[IndexOutput]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for o in outputs {
        let mut heading = String::new();
        for line in o.bytes.lines() {
            if line.starts_with("## ") {
                heading = line.to_owned();
            } else if let Some(rest) = line.strip_prefix("- [") {
                if heading == "## Shards" {
                    continue;
                }
                let (_, rest) = rest.split_once("](").expect("`](` in an entry");
                let link = rest.split_once(')').expect("`)` in an entry").0;
                out.push((o.path.clone(), heading.clone(), resolve(&o.path, link)));
            }
        }
    }
    out
}

fn generated(file: &CheckFile) -> bool {
    file.parsed
        .as_ref()
        .and_then(|parsed| parsed.document())
        .and_then(|document| document.fields.as_ref())
        .and_then(|fields| fields.class.as_deref())
        == Some("generated")
}

/// AC-03: every walked document but a generated one or an output is on
/// exactly one line across `outputs`. `Err`: what is wrong.
fn listed_once(
    outputs: &[IndexOutput],
    input: &CheckInput,
) -> Result<BTreeMap<String, String>, String> {
    let output_paths: Vec<&str> = outputs.iter().map(|o| o.path.as_str()).collect();
    let expected: Vec<String> = input
        .files
        .iter()
        .filter(|f| !generated(f) && !output_paths.contains(&f.path.as_str()))
        .map(|f| f.path.clone())
        .collect();
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (output, _, target) in entries(outputs) {
        seen.entry(target).or_default().push(output);
    }
    let mut problems = Vec::new();
    for (target, at) in &seen {
        if at.len() != 1 {
            problems.push(format!("{target} listed {} times: {at:?}", at.len()));
        }
        if !expected.contains(target) {
            problems.push(format!("{target} listed in {at:?}, not expected"));
        }
    }
    for path in &expected {
        if !seen.contains_key(path) {
            problems.push(format!("{path} not listed"));
        }
    }
    if problems.is_empty() {
        Ok(seen
            .into_iter()
            .map(|(k, mut v)| (k, v.remove(0)))
            .collect())
    } else {
        Err(problems.join("\n"))
    }
}

fn index_findings(report: &Report) -> Vec<(String, String, usize)> {
    report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("index-"))
        .map(|f| (f.code.clone(), f.path.clone(), f.line))
        .collect()
}

// ------------------------------------------------------------------ AC-02

const SHARD_EXPECTED: &str = "\
---
class: generated
generator: make index
source: front-matter of the repository's documents
---

# Documentation index: Archive \u{2014} Tier 3, by id only

<!-- Built by `make index`. Manual edits are overwritten on rebuild, and `make gate` rejects them. -->

A shard of [a/index.md](../index.md), the index's one entry point.

## Archive \u{2014} Tier 3, by id only

- [ADR-0002](../d/ADR-0002.md) superseded-by ADR-0001
- [a/s/old.md](../s/old.md) shipped
- [n/old.md](../../n/old.md) shipped
- [z/abandoned.md](../../z/abandoned.md) abandoned
";

#[test]
fn the_archive_shard_holds_exactly_the_tier3_lines_and_the_root_points_to_it() {
    let config = Config::from_toml(TOML);
    let mut files = corpus();
    files.extend(seeded_outputs().into_iter().filter(|(p, _)| *p != CLAIMED));
    let input = input(&config, &files);
    let outputs = render_set(&config, &input);
    assert_eq!(
        outputs.iter().map(|o| o.path.as_str()).collect::<Vec<_>>(),
        [ROOT, ARCH],
        "the root first, then the shard"
    );
    let root = output(&outputs, ROOT);
    let shard = output(&outputs, ARCH);

    // The shard, byte for byte: compact lines of exactly the Tier 3 files.
    assert!(
        shard == SHARD_EXPECTED,
        "the shard:\n{shard}\n--- expected:\n{SHARD_EXPECTED}"
    );
    let tier3: Vec<&str> = input
        .files
        .iter()
        .filter(|f| f.parsed.as_ref().is_some_and(is_tier3_file))
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(
        tier3,
        [
            "a/d/ADR-0002.md",
            "a/s/old.md",
            "z/abandoned.md",
            "n/old.md"
        ],
        "the corpus's Tier 3 (by status; the unclosed shipped spec is live)"
    );

    // The root: no Tier 3 line, no Archive section, one pointer at the end.
    assert!(!root.contains(ARCHIVE_HEADING), "{root}");
    for path in &tier3 {
        assert!(
            !root.contains(&format!("({})", path.trim_start_matches("a/"))),
            "{path} in the root:\n{root}"
        );
    }
    assert!(
        root.ends_with(
            "\n## Shards\n\n- [a/b/arch.md](b/arch.md) Archive \u{2014} Tier 3, by id only\n"
        ),
        "{root}"
    );
    assert_eq!(root.matches("\n## Shards\n").count(), 1);
    assert!(
        root.contains("- [a/archive/kept.md](archive/kept.md) Kept in an archive folder"),
        "a live spec in a folder named archive stays in the root:\n{root}"
    );

    // The root is the unsharded render with its Archive section swapped for
    // the pointer section: every live line as before.
    let plain = Config::from_toml(TOML_PLAIN);
    let without_outputs: Vec<(&str, &str)> = corpus();
    let single = render_index(
        &plain.input(&without_outputs),
        ROOT,
        plain.check.index_generator().unwrap(),
    );
    let cut = single
        .find(&format!("\n{ARCHIVE_HEADING}\n"))
        .expect("the unsharded render has an Archive section");
    let expected_root = format!(
        "{}\n## Shards\n\n- [a/b/arch.md](b/arch.md) Archive \u{2014} Tier 3, by id only\n",
        &single[..cut]
    );
    assert!(
        root == expected_root,
        "the root:\n{root}\n--- expected:\n{expected_root}"
    );

    // `render_index` keeps its signature and returns the root.
    assert_eq!(
        render_index(&input, ROOT, config.check.index_generator().unwrap()),
        root
    );
}

// ------------------------------------------------------------------ AC-03

#[test]
fn every_document_is_listed_once_across_the_outputs_and_no_output_is_listed() {
    for (what, toml) in [("archive shard", TOML), ("archive + claims", TOML_TWO)] {
        let config = Config::from_toml(toml);
        let mut files = corpus();
        files.extend(
            seeded_outputs()
                .into_iter()
                .filter(|(p, _)| toml.contains(&format!("path = \"{p}\"")) || *p == ROOT),
        );
        let input = input(&config, &files);
        let outputs = render_set(&config, &input);
        let placed = listed_once(&outputs, &input).unwrap_or_else(|e| panic!("{what}:\n{e}"));
        for o in &outputs {
            assert!(
                !placed.contains_key(&o.path),
                "{what}: the output {} is listed",
                o.path
            );
        }
        // Placement: Tier 3 → the archive shard (even when a claim matches),
        // claimed live → the claiming shard, the rest → the root.
        let claims = toml == TOML_TWO;
        for (path, at) in &placed {
            let file = input.files.iter().find(|f| &f.path == path).unwrap();
            let want = if file.parsed.as_ref().is_some_and(is_tier3_file) {
                ARCH
            } else if claims && path.starts_with("n/") {
                CLAIMED
            } else {
                ROOT
            };
            assert_eq!(at, want, "{what}: {path}");
        }
        // The failed front-matter is live, under `No class — fix` of the root.
        assert!(
            output(&outputs, ROOT).contains(
                "\n## No class \u{2014} fix\n\n- [a/n/broken.md](n/broken.md) Never closed \u{b7}  \u{b7} ?\n"
            ),
            "{what}:\n{}",
            output(&outputs, ROOT)
        );
    }
}

/// AC-04 in core: with no archive shard, a Tier 3 document a claim matches
/// stays in the claiming shard's Archive section; unclaimed Tier 3 in the
/// root's.
#[test]
fn without_an_archive_shard_tier3_follows_the_claims() {
    let toml = "\
[paths]
index = \"a/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[[generators]]
command = \"make index\"
writes  = [\"a/index.md\", \"n/claimed.md\"]
index   = true
shards  = [{ path = \"n/claimed.md\", claims = [\"n/**\"] }]
";
    let config = Config::from_toml(toml);
    let input = input(&config, &corpus());
    let outputs = render_set(&config, &input);
    let shard = output(&outputs, CLAIMED);
    assert!(
        shard.ends_with(&format!(
            "\n## Canon\n\n- [n/live.md](live.md) Claimed canon \u{b7} n \u{b7} tier 2\n\n{ARCHIVE_HEADING}\n\n- [n/old.md](old.md) shipped\n"
        )),
        "{shard}"
    );
    assert!(
        shard.contains("\n# Documentation index: `n/**`\n"),
        "the label is the claims in backticks:\n{shard}"
    );
    let root = output(&outputs, ROOT);
    assert!(
        root.ends_with(&format!(
            "\n{ARCHIVE_HEADING}\n\n- [ADR-0002](d/ADR-0002.md) superseded-by ADR-0001\n- [a/s/old.md](s/old.md) shipped\n- [z/abandoned.md](../z/abandoned.md) abandoned\n\n## Shards\n\n- [n/claimed.md](../n/claimed.md) `n/**`\n"
        )),
        "{root}"
    );
    listed_once(&outputs, &input).unwrap();
}

// ------------------------------------------------------------------ AC-05

/// The corpus with every output holding its render.
fn rendered(config: &Config) -> (Vec<(String, String)>, Vec<IndexOutput>) {
    let base = corpus();
    let outputs = render_set(config, &input(config, &base));
    let mut files: Vec<(String, String)> = base
        .iter()
        .map(|(p, t)| ((*p).to_owned(), (*t).to_owned()))
        .collect();
    for o in &outputs {
        files.push((o.path.clone(), o.bytes.clone()));
    }
    (files, outputs)
}

fn check(config: &Config, files: &[(String, String)]) -> Report {
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    config.check(&pairs)
}

fn edit(files: &mut [(String, String)], path: &str, from: &str, to: &str) {
    let file = files.iter_mut().find(|(p, _)| p == path).unwrap();
    assert_eq!(file.1.matches(from).count(), 1, "{from:?} once in {path}");
    file.1 = file.1.replacen(from, to, 1);
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} not in:\n{text}"))
        + 1
}

#[test]
fn each_output_is_compared_on_its_own() {
    let config = Config::from_toml(TOML_TWO);
    let (files, outputs) = rendered(&config);
    // Rendered: clean.
    let report = check(&config, &files);
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
    // The rendered set is stable once its files are walked.
    let walked = render_set(
        &config,
        &input(&config, &{
            files
                .iter()
                .map(|(p, t)| (p.as_str(), t.as_str()))
                .collect::<Vec<_>>()
        }),
    );
    assert_eq!(walked, outputs, "the outputs are not listed in themselves");

    // One byte on line k of the archive shard: one drift there, at k.
    let k = line_of(output(&outputs, ARCH), "- [a/s/old.md]");
    let mut edited = files.clone();
    edit(
        &mut edited,
        ARCH,
        "(../s/old.md) shipped",
        "(../s/old.md) Shipped",
    );
    assert_eq!(
        index_findings(&check(&config, &edited)),
        [("index-drift".to_owned(), ARCH.to_owned(), k)]
    );

    // The live shard likewise.
    let k = line_of(output(&outputs, CLAIMED), "- [n/live.md]");
    let mut edited = files.clone();
    edit(&mut edited, CLAIMED, "Claimed canon", "Claimed Canon");
    assert_eq!(
        index_findings(&check(&config, &edited)),
        [("index-drift".to_owned(), CLAIMED.to_owned(), k)]
    );

    // A shard not walked: `index-missing` on its path, the others clean.
    let without: Vec<(String, String)> = files.iter().filter(|(p, _)| p != ARCH).cloned().collect();
    let report = check(&config, &without);
    let missing = index_findings(&report);
    assert_eq!(
        missing,
        [("index-missing".to_owned(), ARCH.to_owned(), 1)],
        "{}",
        show(&report)
    );
    let finding = report
        .findings
        .iter()
        .find(|f| f.code == "index-missing")
        .unwrap();
    assert!(
        finding.message.contains("`make index`"),
        "{}",
        finding.message
    );

    // A root edit: drift on the root only.
    let k = line_of(output(&outputs, ROOT), "- [top.md]");
    let mut edited = files.clone();
    edit(&mut edited, ROOT, "Top rules", "Top Rules");
    assert_eq!(
        index_findings(&check(&config, &edited)),
        [("index-drift".to_owned(), ROOT.to_owned(), k)]
    );

    // Root and both shards edited: each reported, none stops the rest.
    edit(
        &mut edited,
        ARCH,
        "(../s/old.md) shipped",
        "(../s/old.md) Shipped",
    );
    edit(&mut edited, CLAIMED, "Claimed canon", "Claimed Canon");
    let mut got: Vec<String> = index_findings(&check(&config, &edited))
        .into_iter()
        .map(|(code, path, _)| format!("{code} {path}"))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            format!("index-drift {ARCH}"),
            format!("index-drift {ROOT}"),
            format!("index-drift {CLAIMED}"),
        ]
    );
}

#[test]
fn a_walk_gap_compares_no_output() {
    let config = Config::from_toml(TOML_TWO);
    let (files, _) = rendered(&config);
    let mut edited: Vec<(String, String)> = files
        .iter()
        .filter(|(p, _)| p != CLAIMED)
        .cloned()
        .collect();
    edit(
        &mut edited,
        ARCH,
        "(../s/old.md) shipped",
        "(../s/old.md) Shipped",
    );
    edit(&mut edited, ROOT, "Top rules", "Top Rules");
    let complete = check(&config, &edited);
    assert_eq!(index_findings(&complete).len(), 3, "{}", show(&complete));
    let pairs: Vec<(&str, &str)> = edited
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    let mut gap = config.input(&pairs);
    gap.files.push(CheckFile {
        path: "q/unread.md".to_owned(),
        size: 10,
        parsed: None,
        read_error: Some("Permission denied (os error 13)".to_owned()),
        bytes: Vec::new(),
    });
    let report = config.run(&gap);
    assert_eq!(report.verdict, Verdict::CannotCheck);
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn shard_bytes_not_supplied_cannot_be_compared() {
    let config = Config::from_toml(TOML_TWO);
    let (files, _) = rendered(&config);
    let pairs: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    let mut input = config.input(&pairs);
    let shard = input.files.iter_mut().find(|f| f.path == CLAIMED).unwrap();
    shard.bytes = Vec::new();
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::CannotCheck, "{}", show(&report));
    assert!(
        report
            .cannot_check
            .iter()
            .any(|cause| cause.path == CLAIMED),
        "{:?}",
        report.cannot_check
    );
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
}

// ------------------------------------------------------------------ AC-11

#[test]
fn reversed_input_renders_identical_bytes_for_every_output() {
    for toml in [TOML, TOML_TWO] {
        let config = Config::from_toml(toml);
        let mut files = corpus();
        files.extend(seeded_outputs());
        let mut input = input(&config, &files);
        let forward = render_set(&config, &input);
        input.files.reverse();
        assert_eq!(render_set(&config, &input), forward);
        input.files.rotate_left(5);
        assert_eq!(render_set(&config, &input), forward);
    }
}
