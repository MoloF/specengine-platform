//! AC-19 of docs/features/spec-parser.md: this repository's own
//! documentation parses. Every file listed by `cargo xtask docs budget`
//! parses under the scheme {ADR, width 4} with no front-matter diagnostic,
//! except exactly the four files whose front-matter is not valid YAML (Q6:
//! accepted ADRs stay unedited) — one `frontmatter-yaml` each; every ADR's
//! `id` equals its file stem; every `canon:` is a path with an anchor.
//!
//! Discrepancy kept visible (not smoothed over): three of the four
//! allowlisted files are ADRs (0015, 0018, 0020). Their YAML fails, so the
//! parser keeps no ID for them (AC-06: no guessed ID) and "every ADR's `id`
//! = its stem" cannot hold for them; they are exempted from that clause here
//! by name, and the criterion's text needs the same exemption.
//!
//! Read-only: the repository's files are read, never written.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use specengine_model::{CanonTarget, DiagnosticCode, IdScheme, ParsedFile, PrefixSpec};

use common::repository_root;

/// The files whose front-matter is not valid YAML (Q6).
const INVALID_YAML: [&str; 4] = [
    "docs/decisions/ADR-0015.md",
    "docs/decisions/ADR-0018.md",
    "docs/decisions/ADR-0020.md",
    "docs/features/phase-0-spikes.md",
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

fn every_listed_file_parses_without_front_matter_diagnostics_but_the_four(files: &[Parsed]) {
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
            let codes: Vec<DiagnosticCode> = front.iter().map(|(c, _)| *c).collect();
            if codes != [DiagnosticCode::FrontmatterYaml] {
                unexpected.push(format!(
                    "{}: want exactly one frontmatter-yaml, got {front:?}",
                    file.path
                ));
            }
            assert!(
                file.parsed.document().is_some(),
                "{}: body still read",
                file.path
            );
        } else if !front.is_empty() {
            unexpected.push(format!("{}: {:?}", file.path, file.parsed.diagnostics));
        }
    }
    assert_eq!(
        invalid_seen, INVALID_YAML,
        "the four files are listed by docs budget"
    );
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n"));
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
        if INVALID_YAML.contains(&file.path.as_str()) {
            // Exempt: invalid YAML keeps no ID (see the module comment).
            assert_eq!(id, None, "{}: no guessed ID", file.path);
            continue;
        }
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
        if INVALID_YAML.contains(&file.path.as_str()) {
            assert!(
                canon.is_none(),
                "{}: nothing read from invalid YAML",
                file.path
            );
            continue;
        }
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

type Check = fn(&[Parsed]);

/// One test, one `cargo xtask docs budget`: parallel test processes each
/// running `cargo run -p xtask` race when the binary is (re)built — one
/// cargo replaces `target/debug/xtask` while another executes it (ENOENT).
#[test]
fn repository_docs_parse_under_the_adr_scheme() {
    let files = parse_all();
    let checks: [(&str, Check); 3] = [
        (
            "no front-matter diagnostic but the four",
            every_listed_file_parses_without_front_matter_diagnostics_but_the_four,
        ),
        ("every ADR id is its stem", every_adr_id_is_its_file_stem),
        (
            "every canon: is path + anchor",
            every_canon_is_a_path_with_an_anchor,
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
