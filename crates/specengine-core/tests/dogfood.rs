//! AC-19 of docs/features/spec-parser.md, as amended by AC-17 of
//! docs/features/spec-check-graph.md (the owner's Q-2 answer: the four
//! invalid YAML scalars quoted): this repository's own documentation parses.
//! Every file listed by `cargo xtask docs budget` parses under the scheme
//! {ADR, width 4} with no front-matter diagnostic and no exception
//! (`INVALID_YAML` is empty); every ADR's `id` equals its file stem, with no
//! exemption; every `canon:` is a path with an anchor. The four files the
//! Q-2 edit quoted parse strictly and keep their quoted scalar.
//!
//! docs/features/spec-check.md AC-03: every parsing ADR's `canon:` anchor
//! is among its target's anchors (ADR-0023 through an `html` anchor,
//! ADR-0025 through a heading slug).
//!
//! Read-only: the repository's files are read, never written.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use specengine_model::{
    AnchorOrigin, CanonTarget, DiagnosticCode, IdScheme, ParsedFile, PrefixSpec,
};

use common::repository_root;

/// The files whose front-matter is not valid YAML: none since the Q-2 edit
/// (docs/features/spec-check-graph.md AC-17).
const INVALID_YAML: [&str; 0] = [];

/// The four files the Q-2 edit quoted, with the key it quoted.
const QUOTED_BY_Q2: [(&str, &str); 4] = [
    ("docs/decisions/ADR-0015.md", "title"),
    ("docs/decisions/ADR-0018.md", "title"),
    ("docs/decisions/ADR-0020.md", "title"),
    ("docs/features/phase-0-spikes.md", "ref"),
];

fn adr_scheme() -> IdScheme {
    IdScheme::new(vec![PrefixSpec::number("ADR", "decision", 4)]).unwrap()
}

/// The document list of `cargo xtask docs budget`: the first column of the
/// table between its header and the blank line before the W summary.
fn budget_files() -> Vec<String> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .current_dir(repository_root())
        .args(["xtask", "docs", "budget"])
        .output()
        .expect("cargo xtask docs budget runs");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let mut lines = stdout.lines();
    let header = lines.next().unwrap_or_default();
    assert!(
        header.starts_with("document") && header.contains("class"),
        "unexpected budget output (exit {:?}):\n{stdout}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let files: Vec<String> = lines
        .take_while(|line| !line.trim().is_empty())
        .map(|line| line.split_whitespace().next().unwrap().to_owned())
        .collect();
    assert!(files.len() >= 40, "only {} files listed", files.len());
    files
}

struct Parsed {
    path: String,
    bytes: Vec<u8>,
    parsed: ParsedFile,
}

fn parse_all() -> Vec<Parsed> {
    let root = repository_root();
    let scheme = adr_scheme();
    budget_files()
        .into_iter()
        .map(|path| {
            let bytes = fs::read(root.join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            let parsed = specengine_core::parse(&path, &bytes, &scheme);
            Parsed {
                path,
                bytes,
                parsed,
            }
        })
        .collect()
}

/// Last file line of the front-matter block (0 without one).
fn front_matter_last_line(file: &Parsed) -> usize {
    file.parsed.front_matter.map_or(0, |span| {
        file.bytes[..span.end]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
    })
}

fn is_front_matter_diagnostic(file: &Parsed, code: DiagnosticCode, line: usize) -> bool {
    code.as_str().starts_with("frontmatter-")
        || code == DiagnosticCode::IdNotInScheme
        || line <= front_matter_last_line(file)
}

fn every_listed_file_parses_without_front_matter_diagnostics(files: &[Parsed]) {
    let mut unexpected = Vec::new();
    let mut invalid_seen = Vec::new();
    for file in files {
        let front: Vec<(DiagnosticCode, usize)> = file
            .parsed
            .diagnostics
            .iter()
            .filter(|d| is_front_matter_diagnostic(file, d.code, d.line))
            .map(|d| (d.code, d.line))
            .collect();
        if INVALID_YAML.contains(&file.path.as_str()) {
            invalid_seen.push(file.path.clone());
        } else if !front.is_empty() {
            unexpected.push(format!("{}: {:?}", file.path, file.parsed.diagnostics));
        }
    }
    assert_eq!(invalid_seen, INVALID_YAML, "no file is excepted any more");
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n"));
}

/// The Q-2 edit: the four files are listed by `docs budget`, their
/// front-matter is read strictly, and the quoted scalar is the key's value
/// (a `: ` inside it, which made the YAML invalid).
fn the_four_quoted_files_parse_strictly(files: &[Parsed]) {
    for (path, key) in QUOTED_BY_Q2 {
        let file = files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("{path}: listed by docs budget"));
        let codes: Vec<&str> = file
            .parsed
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .filter(|code| code.starts_with("frontmatter-"))
            .collect();
        assert!(codes.is_empty(), "{path}: {codes:?}");
        let fields = file
            .parsed
            .document()
            .and_then(|d| d.fields.as_ref())
            .unwrap_or_else(|| panic!("{path}: front-matter read"));
        let front = file
            .parsed
            .front_matter
            .map(|span| String::from_utf8_lossy(&file.bytes[span.range()]).into_owned())
            .unwrap_or_default();
        let raw = front
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}: ")))
            .unwrap_or_else(|| panic!("{path}: `{key}:` written"));
        let quoted = |quote: char| raw.len() > 1 && raw.starts_with(quote) && raw.ends_with(quote);
        assert!(
            quoted('"') || quoted('\''),
            "{path}: `{key}:` quoted, got {raw}"
        );
        assert!(raw.contains(": "), "{path}: the quoted `{key}` holds `: `");
        if key == "title" {
            let title = file.parsed.document().and_then(|d| d.title.as_deref());
            assert!(
                title.is_some_and(|title| title.contains(": ") && !title.starts_with('"')),
                "{path}: title read unquoted, got {title:?}"
            );
            assert_eq!(
                file.parsed.document().and_then(|d| d.id.as_deref()),
                Path::new(path).file_stem().and_then(|s| s.to_str()),
                "{path}: the ID is read"
            );
        } else {
            let value = fields.reference.as_deref();
            assert!(
                value.is_some_and(|value| value.contains(": ") && !value.starts_with(['"', '\''])),
                "{path}: `{key}` read unquoted, got {value:?}"
            );
        }
    }
}

fn every_adr_id_is_its_file_stem(files: &[Parsed]) {
    let mut adrs = 0;
    for file in files {
        let path = Path::new(&file.path);
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        if !(file.path.starts_with("docs/decisions/") && stem.starts_with("ADR-")) {
            continue;
        }
        adrs += 1;
        let id = file.parsed.document().and_then(|d| d.id.clone());
        // No exemption since the Q-2 edit (AC-17 of spec-check-graph).
        assert_eq!(id.as_deref(), Some(stem.as_str()), "{}", file.path);
        assert_eq!(
            file.parsed.document().unwrap().kind.as_deref(),
            Some("decision"),
            "{}: kind from the scheme",
            file.path
        );
    }
    assert!(adrs >= 25, "only {adrs} ADRs");
}

fn every_canon_is_a_path_with_an_anchor(files: &[Parsed]) {
    let mut canons = 0;
    for file in files {
        let front_text = file
            .parsed
            .front_matter
            .map(|span| String::from_utf8_lossy(&file.bytes[span.range()]).into_owned())
            .unwrap_or_default();
        let declares = front_text.lines().any(|line| line.starts_with("canon:"));
        let canon = file
            .parsed
            .document()
            .and_then(|d| d.fields.as_ref())
            .and_then(|f| f.canon.as_ref());
        assert_eq!(declares, canon.is_some(), "{}: canon: read", file.path);
        let Some(canon) = canon else { continue };
        canons += 1;
        match canon {
            CanonTarget::Path(target) => {
                assert!(
                    target.anchor.as_deref().is_some_and(|a| !a.is_empty()),
                    "{}: canon {} has no anchor",
                    file.path,
                    target.path
                );
                assert!(
                    repository_root().join(&target.path).is_file(),
                    "{}: canon path {} exists",
                    file.path,
                    target.path
                );
            }
            CanonTarget::Reference(reference) => {
                panic!("{}: canon is a reference {reference:?}", file.path)
            }
        }
    }
    assert!(canons >= 20, "only {canons} canon: values");
}

/// docs/features/spec-check.md AC-03: the anchor of every parsed ADR's
/// `canon:` is among its target's anchors; ADR-0023's (`CLAUDE.md#process`)
/// is an `html` anchor, ADR-0025's (`README.md#license`) a heading `slug`.
fn every_canon_anchor_is_among_its_targets(files: &[Parsed]) {
    let scheme = adr_scheme();
    let mut resolved = 0;
    let mut origins = std::collections::BTreeMap::new();
    for file in files {
        if !file.path.starts_with("docs/decisions/ADR-") {
            continue;
        }
        let Some(CanonTarget::Path(target)) = file
            .parsed
            .document()
            .and_then(|d| d.fields.as_ref())
            .and_then(|f| f.canon.as_ref())
        else {
            continue;
        };
        let anchor = target.anchor.as_deref().unwrap_or_default();
        let parsed_target = match files.iter().find(|other| other.path == target.path) {
            Some(other) => other.parsed.clone(),
            None => {
                let bytes = fs::read(repository_root().join(&target.path))
                    .unwrap_or_else(|e| panic!("{}: canon {}: {e}", file.path, target.path));
                specengine_core::parse(&target.path, &bytes, &scheme)
            }
        };
        let found = parsed_target
            .anchors
            .iter()
            .find(|known| known.name == anchor)
            .unwrap_or_else(|| {
                panic!(
                    "{}: canon {}#{anchor} is none of its anchors {:?}",
                    file.path,
                    target.path,
                    parsed_target
                        .anchors
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                )
            });
        origins.insert(file.path.clone(), found.origin);
        resolved += 1;
    }
    assert!(
        resolved >= 20,
        "only {resolved} ADR canon: anchors resolved"
    );
    assert_eq!(
        origins.get("docs/decisions/ADR-0023.md"),
        Some(&AnchorOrigin::Html),
        "ADR-0023 lands on CLAUDE.md's <a id=\"process\">"
    );
    assert_eq!(
        origins.get("docs/decisions/ADR-0025.md"),
        Some(&AnchorOrigin::Slug),
        "ADR-0025 lands on README.md's `## License` slug"
    );
}

type Check = fn(&[Parsed]);

/// One test, one `cargo xtask docs budget`: parallel test processes each
/// running `cargo run -p xtask` race when the binary is (re)built — one
/// cargo replaces `target/debug/xtask` while another executes it (ENOENT).
#[test]
fn repository_docs_parse_under_the_adr_scheme() {
    let files = parse_all();
    let checks: [(&str, Check); 5] = [
        (
            "no front-matter diagnostic, no exception",
            every_listed_file_parses_without_front_matter_diagnostics,
        ),
        (
            "the four Q-2 files parse strictly",
            the_four_quoted_files_parse_strictly,
        ),
        ("every ADR id is its stem", every_adr_id_is_its_file_stem),
        (
            "every canon: is path + anchor",
            every_canon_is_a_path_with_an_anchor,
        ),
        (
            "every ADR canon: anchor is among its target's (spec-check AC-03)",
            every_canon_anchor_is_among_its_targets,
        ),
    ];
    let mut failed = Vec::new();
    for (name, check) in checks {
        if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&files)))
        {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_default();
            failed.push(format!("{name}: {message}"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
