//! AC-03 and AC-04 of docs/features/spec-check-graph.md: the pure index
//! renderer (`check::render_index`) and the §11.5 drift rule, over scratch
//! inputs built in memory.
//!
//! AC-03: the header carries the registered `command` twice and the gate
//! (default `spec check`); sections `Canon`, `Decisions`, `Specs`,
//! `No class — fix`, `Archive — Tier 3, by id only` in that order, empty
//! ones omitted, paths in byte order; links relative to the index's
//! directory; the index and other generated documents unlisted; `tier ?`,
//! `?` title and status, empty scope; reversed input → identical bytes; no
//! `cargo xtask`, `docs/`, `ADR` or `index.md` literal in the renderer.
//!
//! AC-04: committed = render → nothing; any byte difference (a hand edit, an
//! unrendered document or status, a trailing space, CRLF, a BOM, a missing
//! final newline) → `index-drift` on the line of the first differing byte;
//! the index not walked → `index-missing`; no `index = true` entry →
//! neither; bytes not supplied → cannot check.

mod common;

use std::fs;

use common::check::{Config, show, with_code};
use common::repository_root;
use specengine_core::check::{
    CheckFile, CheckInput, Generator, Problem, ProblemKind, Report, Verdict, render_index,
};
use specengine_model::Severity;

const INDEX: &str = "docs/index.md";

const TOML: &str = "\
[paths]
index = \"docs/index.md\"

[ids]
ADR = { kind = \"decision\", width = 4 }

[[generators]]
command = \"make index\"
writes  = [\"docs/index.md\"]
index   = true
";

fn config() -> Config {
    Config::from_toml(TOML)
}

fn generator(config: &Config) -> &Generator {
    config.check.index_generator().expect("the index entry")
}

const HEADER: &str = "\
---
class: generated
generator: make index
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `make index`. Manual edits are overwritten on rebuild, and `spec check` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.
";

/// One document of every kind the renderer tells apart.
const CORPUS: &[(&str, &str)] = &[
    (
        "CLAUDE.md",
        "---\nclass: canon\ntier: 0\nscope: [all]\nowner: o\nreviewed: 2026-09-01\n---\n\n# Root rules\n",
    ),
    (
        "docs/canon/b.md",
        "---\nclass: canon\ntier: 2\nscope: [x, y]\nowner: o\nreviewed: 2026-09-01\n---\n\n# Canon *B*\n",
    ),
    (
        "docs/canon/a.md",
        "---\nclass: canon\nscope: [x]\nowner: o\nreviewed: 2026-09-01\n---\n\nNo heading here.\n",
    ),
    (
        "docs/decisions/ADR-0001.md",
        "---\nid: ADR-0001\nclass: decision\ntitle: First decision\nstatus: accepted\nscope: [core]\n---\n\n# Ignored heading\n",
    ),
    (
        "docs/decisions/ADR-0002.md",
        "---\nid: ADR-0002\nclass: decision\ntitle: Rejected one\nstatus: rejected\nscope: [core]\n---\n",
    ),
    (
        "docs/decisions/ADR-0003.md",
        "---\nid: ADR-0003\nclass: decision\ntitle: Old one\nstatus: superseded-by ADR-0001\nscope: [core]\n---\n",
    ),
    (
        "docs/decisions/ADR-0004.md",
        "---\nid: ADR-0004\nclass: decision\ntitle: Undecided\nscope: [core]\n---\n",
    ),
    (
        "docs/features/f-draft.md",
        "---\nclass: spec\nstatus: draft\nscope: [core, store]\n---\n\n# Draft feature\n",
    ),
    (
        "docs/features/f-shipped.md",
        "---\nclass: spec\nstatus: shipped\nshipped: 2026-09-01\nscope: [core]\n---\n\n# Shipped feature\n",
    ),
    (
        "docs/features/f-abandoned.md",
        "---\nclass: spec\nstatus: abandoned\nscope: [core]\n---\n\n# Abandoned feature\n",
    ),
    (
        "docs/features/f-nostatus.md",
        "---\nclass: spec\nscope: []\n---\n\n# No status\n",
    ),
    ("docs/plain.md", "# Plain prose\n\nNo front-matter.\n"),
    (
        "docs/memo.md",
        "---\nclass: memo\nstatus: open\nscope: [m]\n---\n\n# A memo\n",
    ),
    (
        "docs/broken.md",
        "---\nclass: canon\ntitle: a: b\nscope: [z]\n---\n\n# Broken front-matter\n",
    ),
    (
        "docs/other-generated.md",
        "---\nclass: generated\ngenerator: make index\nsource: x\n---\n\n# Other generated\n",
    ),
    ("docs/index.md", "stale bytes\n"),
];

/// The render of [`CORPUS`] from `docs/index.md`, written by hand.
fn expected() -> String {
    format!(
        "{HEADER}
## Canon

- [CLAUDE.md](../CLAUDE.md) Root rules · all · tier 0
- [docs/canon/a.md](canon/a.md) ? · x · tier ?
- [docs/canon/b.md](canon/b.md) Canon B · x, y · tier 2

## Decisions

- [ADR-0001](decisions/ADR-0001.md) First decision · core · accepted
- [ADR-0004](decisions/ADR-0004.md) Undecided · core · ?

## Specs

- [docs/features/f-draft.md](features/f-draft.md) Draft feature · core, store · draft
- [docs/features/f-nostatus.md](features/f-nostatus.md) No status ·  · ?

## No class — fix

- [docs/broken.md](broken.md) Broken front-matter ·  · ?
- [docs/memo.md](memo.md) A memo · m · open
- [docs/plain.md](plain.md) Plain prose ·  · ?

## Archive — Tier 3, by id only

- [ADR-0002](decisions/ADR-0002.md) Rejected one · core · rejected
- [ADR-0003](decisions/ADR-0003.md) Old one · core · superseded-by ADR-0001
- [docs/features/f-abandoned.md](features/f-abandoned.md) Abandoned feature · core · abandoned
- [docs/features/f-shipped.md](features/f-shipped.md) Shipped feature · core · shipped
"
    )
}

fn show_difference(got: &str, want: &str) -> String {
    let line = got
        .lines()
        .zip(want.lines())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| got.lines().count().min(want.lines().count()));
    format!(
        "first difference on line {}:\n  got:  {:?}\n  want: {:?}\n--- got ---\n{got}",
        line + 1,
        got.lines().nth(line),
        want.lines().nth(line)
    )
}

// ------------------------------------------------------------------ AC-03

#[test]
fn the_render_is_the_convention_s_index() {
    let config = config();
    let input = config.input(CORPUS);
    let render = render_index(&input, INDEX, generator(&config));
    let want = expected();
    assert!(render == want, "{}", show_difference(&render, &want));
}

#[test]
fn the_header_names_the_command_twice_and_the_gate() {
    let config = Config::from_toml(
        "[paths]\nindex = \"toc.md\"\n\n[[generators]]\ncommand = \"gen toc\"\nwrites = [\"toc.md\"]\nindex = true\ngate = \"lint docs\"\n",
    );
    let render = render_index(&CheckInput::default(), "toc.md", generator(&config));
    assert_eq!(
        render,
        "---\nclass: generated\ngenerator: gen toc\nsource: front-matter of the repository's documents\n---\n\n# Documentation index\n\n<!-- Built by `gen toc`. Manual edits are overwritten on rebuild, and `lint docs` rejects them. -->\n\nReading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.\n",
        "an empty corpus: the header alone, no section"
    );
    assert_eq!(render.matches("gen toc").count(), 2);
    // Without `gate`: the check itself.
    let default_gate = self::config();
    let render = render_index(&CheckInput::default(), INDEX, generator(&default_gate));
    assert_eq!(render, HEADER);
}

#[test]
fn links_are_relative_to_the_index_directory() {
    let files: &[(&str, &str)] = &[
        ("CLAUDE.md", "# Root\n"),
        ("a/c/x.md", "# X\n"),
        ("a/b/y.md", "# Y\n"),
        ("a/b/c/z.md", "# Z\n"),
        ("a/bb/w.md", "# W\n"),
        ("b/v.md", "# V\n"),
    ];
    let config = config();
    let input = config.input(files);
    let links = |index: &str| -> Vec<(String, String)> {
        render_index(&input, index, generator(&config))
            .lines()
            .filter_map(|line| line.strip_prefix("- ["))
            .map(|line| {
                let (label, rest) = line.split_once("](").unwrap();
                let link = rest.split_once(')').unwrap().0;
                (label.to_owned(), link.to_owned())
            })
            .collect()
    };
    let pairs = |items: &[(&str, &str)]| -> Vec<(String, String)> {
        items
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    };
    assert_eq!(
        links("a/b/index.md"),
        pairs(&[
            ("CLAUDE.md", "../../CLAUDE.md"),
            ("a/b/c/z.md", "c/z.md"),
            ("a/b/y.md", "y.md"),
            ("a/bb/w.md", "../bb/w.md"),
            ("a/c/x.md", "../c/x.md"),
            ("b/v.md", "../../b/v.md"),
        ])
    );
    assert_eq!(
        links("index.md"),
        pairs(&[
            ("CLAUDE.md", "CLAUDE.md"),
            ("a/b/c/z.md", "a/b/c/z.md"),
            ("a/b/y.md", "a/b/y.md"),
            ("a/bb/w.md", "a/bb/w.md"),
            ("a/c/x.md", "a/c/x.md"),
            ("b/v.md", "b/v.md"),
        ])
    );
    assert_eq!(
        links("a/index.md"),
        pairs(&[
            ("CLAUDE.md", "../CLAUDE.md"),
            ("a/b/c/z.md", "b/c/z.md"),
            ("a/b/y.md", "b/y.md"),
            ("a/bb/w.md", "bb/w.md"),
            ("a/c/x.md", "c/x.md"),
            ("b/v.md", "../b/v.md"),
        ])
    );
}

#[test]
fn paths_are_in_byte_order() {
    let files: &[(&str, &str)] = &[
        ("d/a/b.md", "# 1\n"),
        ("d/a.md", "# 2\n"),
        ("d/a-b.md", "# 3\n"),
        ("d/B.md", "# 4\n"),
        ("d/_x.md", "# 5\n"),
    ];
    let config = config();
    let render = render_index(&config.input(files), "index.md", generator(&config));
    let labels: Vec<&str> = render
        .lines()
        .filter_map(|line| line.strip_prefix("- ["))
        .map(|line| line.split_once(']').unwrap().0)
        .collect();
    assert_eq!(
        labels,
        ["d/B.md", "d/_x.md", "d/a-b.md", "d/a.md", "d/a/b.md"]
    );
}

#[test]
fn the_index_and_generated_documents_are_not_listed() {
    let config = config();
    let render = render_index(&config.input(CORPUS), INDEX, generator(&config));
    assert!(!render.contains("[docs/index.md]"), "{render}");
    assert!(!render.contains("other-generated"), "{render}");
    // The index path decides, not the class: another index path lists the
    // file at `docs/index.md` (it has no class: `No class — fix`).
    let render = render_index(&config.input(CORPUS), "docs/toc.md", generator(&config));
    assert!(
        render.contains("- [docs/index.md](index.md) ? ·  · ?\n"),
        "{render}"
    );
}

#[test]
fn reversed_input_renders_identical_bytes() {
    let config = config();
    let mut input = config.input(CORPUS);
    let forward = render_index(&input, INDEX, generator(&config));
    input.files.reverse();
    let backward = render_index(&input, INDEX, generator(&config));
    assert_eq!(forward.as_bytes(), backward.as_bytes());
    // Any rotation, too.
    input.files.rotate_left(5);
    assert_eq!(render_index(&input, INDEX, generator(&config)), forward);
}

/// Every string literal of a Rust line outside comments (naive: the text
/// between pairs of `"`).
fn literals(line: &str) -> Vec<&str> {
    if line.trim_start().starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

#[test]
fn the_renderer_names_no_command_path_or_prefix_of_a_project() {
    let dir = repository_root().join("crates/specengine-core/src/check");
    let mut offenders = Vec::new();
    for name in ["render.rs", "generated.rs"] {
        let text = fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                for banned in ["cargo xtask", "docs/", "ADR", "index.md", "xtask"] {
                    if literal.contains(banned) {
                        offenders.push(format!("{name}:{}: {banned:?} in {literal:?}", number + 1));
                    }
                }
            }
        }
        // The whole renderer, comments included, names no project path.
        if name == "render.rs" {
            for banned in ["cargo xtask", "docs/", "index.md", "ADR-"] {
                if text.contains(banned) {
                    offenders.push(format!("{name}: {banned:?} anywhere"));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

// ------------------------------------------------------------------ AC-04

/// The corpus without its index, then the index with `bytes(render)`.
fn with_index(
    config: &Config,
    files: &[(&str, &str)],
    edit: impl Fn(&str) -> Vec<u8>,
) -> CheckInput {
    let docs: Vec<(&str, &str)> = files.iter().copied().filter(|(p, _)| *p != INDEX).collect();
    let mut input = config.input(&docs);
    let render = render_index(&input, INDEX, generator(config));
    input
        .files
        .push(CheckFile::parse(INDEX, edit(&render), &config.scheme));
    input
}

/// A clean corpus (every document meets its default contract).
const CLEAN: &[(&str, &str)] = &[
    (
        "docs/canon/a.md",
        "---\nclass: canon\ntier: 2\nscope: [x]\nowner: o\nreviewed: 2026-09-01\n---\n\n# A\n",
    ),
    (
        "docs/decisions/ADR-0001.md",
        "---\nid: ADR-0001\nclass: decision\ntitle: One\nstatus: accepted\nscope: [x]\ncanon: docs/canon/a.md#a\n---\n\n# One\n",
    ),
    (
        "docs/features/f.md",
        "---\nclass: spec\nstatus: draft\nscope: [x]\n---\n\n# F\n",
    ),
];

fn index_findings(report: &Report) -> Vec<(String, String, usize, String)> {
    report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("index-"))
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

fn drift_at(line: usize) -> Vec<(String, String, usize, String)> {
    vec![(
        "index-drift".to_owned(),
        INDEX.to_owned(),
        line,
        String::new(),
    )]
}

/// The line of the first byte where `walked` and `rendered` differ.
fn first_differing_line(walked: &[u8], rendered: &[u8]) -> usize {
    let offset = walked
        .iter()
        .zip(rendered)
        .position(|(a, b)| a != b)
        .unwrap_or(walked.len().min(rendered.len()));
    1 + walked[..offset].iter().filter(|&&b| b == b'\n').count()
}

#[test]
fn a_committed_render_is_clean() {
    let config = config();
    let input = with_index(&config, CLEAN, |render| render.as_bytes().to_vec());
    let report = config.run(&input);
    assert!(report.findings.is_empty(), "{}", show(&report));
    assert_eq!(report.verdict, Verdict::Clean);
    // The big corpus too: no index finding.
    let input = with_index(&config, CORPUS, |render| render.as_bytes().to_vec());
    let report = config.run(&input);
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn a_hand_edit_is_drift_on_its_line() {
    let config = config();
    let render = render_index(&config.input(CLEAN), INDEX, generator(&config));
    let lines: Vec<&str> = render.lines().collect();
    for (number, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let edited: String = lines
            .iter()
            .enumerate()
            .map(|(at, text)| {
                if at == number {
                    format!("{text}!\n")
                } else {
                    format!("{text}\n")
                }
            })
            .collect();
        let input = with_index(&config, CLEAN, |_| edited.as_bytes().to_vec());
        let report = config.run(&input);
        assert_eq!(
            index_findings(&report),
            drift_at(number + 1),
            "edit on line {}:\n{}",
            number + 1,
            show(&report)
        );
        let drift = with_code(&report, "index-drift")[0];
        assert_eq!(drift.severity, Severity::Error);
        assert!(drift.message.contains("make index"), "{}", drift.message);
        assert_eq!(report.verdict, Verdict::Blocked);
    }
}

#[test]
fn an_unrendered_document_or_status_is_drift() {
    let config = config();
    let old = render_index(&config.input(CLEAN), INDEX, generator(&config));

    // A new document, the index not rebuilt.
    let mut grown: Vec<(&str, &str)> = CLEAN.to_vec();
    grown.push((
        "docs/canon/b.md",
        "---\nclass: canon\ntier: 2\nscope: [x]\nowner: o\nreviewed: 2026-09-01\n---\n\n# B\n",
    ));
    let new = render_index(&config.input(&grown), INDEX, generator(&config));
    let input = with_index(&config, &grown, |_| old.as_bytes().to_vec());
    let report = config.run(&input);
    assert_eq!(
        index_findings(&report),
        drift_at(first_differing_line(old.as_bytes(), new.as_bytes())),
        "{}",
        show(&report)
    );

    // A status change, the index not rebuilt: the ADR moves to the archive.
    let mut changed: Vec<(&str, &str)> = CLEAN.to_vec();
    changed[1].1 = "---\nid: ADR-0001\nclass: decision\ntitle: One\nstatus: rejected\nscope: [x]\n---\n\n# One\n";
    let input = with_index(&config, &changed, |_| old.as_bytes().to_vec());
    let report = config.run(&input);
    assert_eq!(index_findings(&report).len(), 1, "{}", show(&report));
    assert_eq!(index_findings(&report)[0].0, "index-drift");
}

#[test]
fn whitespace_line_endings_and_a_bom_are_drift() {
    let config = config();
    let render = render_index(&config.input(CLEAN), INDEX, generator(&config));
    let last_line = render.lines().count();
    let cases: Vec<(&str, Vec<u8>, usize)> = vec![
        (
            "a trailing space on line 7",
            render
                .replacen("# Documentation index\n", "# Documentation index \n", 1)
                .into_bytes(),
            7,
        ),
        ("CRLF", render.replace('\n', "\r\n").into_bytes(), 1),
        (
            "a BOM",
            [b"\xEF\xBB\xBF".as_slice(), render.as_bytes()].concat(),
            1,
        ),
        (
            "no final newline",
            render.strip_suffix('\n').unwrap().as_bytes().to_vec(),
            last_line,
        ),
        (
            "an extra final newline",
            format!("{render}\n").into_bytes(),
            last_line + 1,
        ),
        (
            "a trailing space at the end",
            format!("{render} ").into_bytes(),
            last_line + 1,
        ),
        (
            "a blank line removed",
            render
                .replacen("---\n\n# Documentation", "---\n# Documentation", 1)
                .into_bytes(),
            6,
        ),
        ("an empty file", Vec::new(), 1),
    ];
    for (case, bytes, line) in cases {
        assert_eq!(
            first_differing_line(&bytes, render.as_bytes()),
            line,
            "{case}: the test's own arithmetic"
        );
        let input = with_index(&config, CLEAN, |_| bytes.clone());
        let report = config.run(&input);
        assert_eq!(
            index_findings(&report),
            drift_at(line),
            "{case}:\n{}",
            show(&report)
        );
    }
}

#[test]
fn an_index_not_walked_is_index_missing() {
    let config = config();
    let report = config.check(CLEAN);
    assert_eq!(
        index_findings(&report),
        vec![(
            "index-missing".to_owned(),
            INDEX.to_owned(),
            1,
            String::new()
        )],
        "{}",
        show(&report)
    );
    let missing = with_code(&report, "index-missing")[0];
    assert_eq!(missing.severity, Severity::Error);
    assert!(
        missing.message.contains("make index"),
        "{}",
        missing.message
    );
    assert_eq!(report.verdict, Verdict::Blocked);
    // An index at another path does not count.
    let mut files = CLEAN.to_vec();
    files.push(("docs/INDEX.md", "x\n"));
    let report = config.check(&files);
    assert_eq!(index_findings(&report)[0].0, "index-missing");
}

#[test]
fn without_an_index_entry_neither_rule_runs() {
    for toml in [
        // A registry without `index = true`.
        "[paths]\nindex = \"docs/index.md\"\n\n[[generators]]\ncommand = \"make index\"\nwrites = [\"docs/index.md\"]\n",
        "[paths]\nindex = \"docs/index.md\"\n\n[[generators]]\ncommand = \"make index\"\nwrites = [\"docs/index.md\"]\nindex = false\n",
        "generators = []\n\n[paths]\nindex = \"docs/index.md\"\n",
        // No registry at all.
        "[paths]\nindex = \"docs/index.md\"\n",
        "",
    ] {
        let config = Config::from_toml(toml);
        assert!(config.check.index_generator().is_none(), "{toml}");
        // Absent index: no `index-missing`.
        let report = config.check(CLEAN);
        assert!(
            index_findings(&report).is_empty(),
            "{toml}\n{}",
            show(&report)
        );
        // A drifted index: no `index-drift`.
        let mut files = CLEAN.to_vec();
        files.push((
            INDEX,
            "---\nclass: generated\ngenerator: make index\nsource: x\n---\n\n# Hand-written\n",
        ));
        let report = config.check(&files);
        assert!(
            index_findings(&report).is_empty(),
            "{toml}\n{}",
            show(&report)
        );
    }
}

#[test]
fn index_bytes_not_supplied_cannot_be_checked() {
    let config = config();
    let mut input = with_index(&config, CLEAN, |render| render.as_bytes().to_vec());
    let index = input.files.iter_mut().find(|f| f.path == INDEX).unwrap();
    assert!(index.size > 0);
    index.bytes.clear();
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::CannotCheck, "{}", show(&report));
    assert!(
        report.cannot_check.iter().any(|cause| cause.path == INDEX),
        "{:?}",
        report.cannot_check
    );
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
    // Other files without bytes do not matter to the rule.
    let mut input = with_index(&config, CLEAN, |render| render.as_bytes().to_vec());
    for file in input.files.iter_mut().filter(|f| f.path != INDEX) {
        file.bytes.clear();
    }
    let report = config.run(&input);
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
    assert!(report.cannot_check.is_empty(), "{:?}", report.cannot_check);
}

#[test]
fn an_unreadable_index_is_cannot_check_not_drift() {
    let config = config();
    let mut input = config.input(CLEAN);
    input
        .files
        .push(CheckFile::unreadable(INDEX, "permission denied"));
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::CannotCheck, "{}", show(&report));
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
}

#[test]
fn drift_findings_are_independent_of_input_order() {
    let config = config();
    let mut input = with_index(&config, CORPUS, |render| {
        render.replacen("tier 2", "tier 1", 1).into_bytes()
    });
    let forward = config.run(&input);
    input.files.reverse();
    let backward = config.run(&input);
    assert_eq!(forward.lines(true), backward.lines(true));
    assert_eq!(forward.to_json(), backward.to_json());
    assert_eq!(index_findings(&forward).len(), 1, "{}", show(&forward));
}

// ------------------------------------------------------------------ iteration 2
// A walk that cannot vouch for the corpus skips the comparison: the render
// would not list every document, so drift (or a missing index) would be a
// finding the generator cannot fix. The three triggers are the causes of
// "cannot check": an unreadable file, an unreadable directory, a written
// root missing. A skipped non-UTF-8 name is only the warning `name-skipped`
// and must not skip it (a hand-edited index would pass as clean); APFS
// refuses such names, so that problem is built here, as the store's walk
// would report it.

fn roots_written() -> Config {
    Config::from_toml(&TOML.replacen("[paths]\n", "[paths]\nroots = [\"docs\"]\n", 1))
}

/// `(index state, bytes of the index or None when absent)`.
fn index_states(config: &Config) -> Vec<(&'static str, Option<Vec<u8>>)> {
    let render = render_index(&config.input(CLEAN), INDEX, generator(config));
    vec![
        ("a drifted index", Some(format!("{render}x\n").into_bytes())),
        ("no index", None),
    ]
}

fn input_with(config: &Config, index: &Option<Vec<u8>>) -> CheckInput {
    let mut input = config.input(CLEAN);
    if let Some(bytes) = index {
        input
            .files
            .push(CheckFile::parse(INDEX, bytes.clone(), &config.scheme));
    }
    input
}

#[test]
fn a_walk_that_cannot_be_vouched_for_skips_the_index_comparison() {
    let config = roots_written();
    assert!(config.paths.roots_written);
    type Break = fn(&mut CheckInput);
    let cases: [(&str, Break); 3] = [
        ("an unreadable file", |input| {
            input
                .files
                .push(CheckFile::unreadable("docs/locked.md", "permission denied"))
        }),
        ("an unreadable directory", |input| {
            input.problems.push(Problem {
                kind: ProblemKind::UnreadableDir,
                path: "docs/locked".to_owned(),
            })
        }),
        ("a written root missing", |input| {
            input.problems.push(Problem {
                kind: ProblemKind::MissingRoot,
                path: "docs".to_owned(),
            })
        }),
    ];
    for (state, index) in index_states(&config) {
        // The corpus alone: the rule runs.
        let report = config.run(&input_with(&config, &index));
        assert_eq!(
            index_findings(&report).len(),
            1,
            "{state}, complete walk:\n{}",
            show(&report)
        );
        for (case, break_walk) in cases {
            let mut input = input_with(&config, &index);
            break_walk(&mut input);
            let report = config.run(&input);
            assert!(
                index_findings(&report).is_empty(),
                "{state}, {case}: no index-drift / index-missing:\n{}",
                show(&report)
            );
            assert_eq!(
                report.verdict,
                Verdict::CannotCheck,
                "{state}, {case}:\n{}",
                show(&report)
            );
            // Order-independent, too.
            input.files.reverse();
            input.problems.reverse();
            assert_eq!(config.run(&input).to_json(), report.to_json());
        }
    }
}

/// The corrected behaviour (coordinator, iteration 2 review): `name-skipped`
/// is only a warning, so the comparison still runs. Red until iteration 3
/// removes `SkippedName` from the incomplete-walk guard.
#[test]
fn a_skipped_name_does_not_skip_the_index_comparison() {
    let config = roots_written();
    let render = render_index(&config.input(CLEAN), INDEX, generator(&config));
    for (state, index, code) in [
        (
            "a hand-edited index",
            Some(
                render
                    .replacen("# Documentation index", "# Documentation Index", 1)
                    .into_bytes(),
            ),
            "index-drift",
        ),
        ("no index", None, "index-missing"),
    ] {
        let mut input = input_with(&config, &index);
        input.problems.push(Problem {
            kind: ProblemKind::SkippedName,
            path: "docs".to_owned(),
        });
        let report = config.run(&input);
        assert_eq!(with_code(&report, "name-skipped").len(), 1, "{state}");
        assert_ne!(report.verdict, Verdict::CannotCheck, "{state}");
        let found: Vec<String> = index_findings(&report)
            .into_iter()
            .map(|(code, ..)| code)
            .collect();
        assert_eq!(found, [code], "{state}:\n{}", show(&report));
        assert_eq!(
            report.verdict,
            Verdict::Blocked,
            "{state}: the index error blocks"
        );
    }
}

#[test]
fn a_default_role_root_missing_does_not_skip_the_comparison() {
    // `roots` not written: a missing default role root (spec-a's
    // `docs/archive`) is no cause and the walk counts as complete.
    let config = config();
    assert!(!config.paths.roots_written);
    for (state, index) in index_states(&config) {
        let mut input = input_with(&config, &index);
        input.problems.push(Problem {
            kind: ProblemKind::MissingRoot,
            path: "docs/archive".to_owned(),
        });
        let report = config.run(&input);
        assert_ne!(report.verdict, Verdict::CannotCheck, "{state}");
        assert_eq!(
            index_findings(&report).len(),
            1,
            "{state}:\n{}",
            show(&report)
        );
    }
}

#[test]
fn an_index_entry_without_an_index_path_is_a_cause_in_code() {
    // The TOML reader rejects this; a config built in code can still hold it.
    let mut config = config();
    config.paths.index = None;
    config.check.generators.as_mut().unwrap()[0].line = 7;
    let input = with_index(&self::config(), CLEAN, |render| render.as_bytes().to_vec());
    let report = config.run(&input);
    assert_eq!(report.verdict, Verdict::CannotCheck, "{}", show(&report));
    let causes: Vec<(&str, &str)> = report
        .cannot_check
        .iter()
        .map(|cause| (cause.path.as_str(), cause.message.as_str()))
        .collect();
    assert_eq!(causes.len(), 1, "{causes:?}");
    assert_eq!(causes[0].0, "", "no path: the config is at fault");
    assert!(
        causes[0].1.contains("make index") && causes[0].1.contains("line 7"),
        "the message names the entry: {}",
        causes[0].1
    );
    assert!(index_findings(&report).is_empty(), "{}", show(&report));
    // Without the index entry, nothing to say.
    config.check.generators.as_mut().unwrap()[0].index = false;
    let report = config.run(&input);
    assert!(report.cannot_check.is_empty(), "{:?}", report.cannot_check);
}
