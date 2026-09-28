//! AC-13 of docs/features/phase-0-spikes.md: `docs/`, `crates/` and
//! `fixtures/` carry no absolute paths, no pilot names and no non-English text.
//! The ID prefixes of a runtime census config (08 §4.3;
//! `crates/specengine-import/README.md`) are not grepped here: such a
//! config may name single letters or prefixes this repository uses itself,
//! so a repository-wide grep cannot tell a leak from a coincidence.
//! `census_cli.rs` checks instead that the census stdout carries no corpus
//! string at all (fixture and `#[ignore]` pilot runs) and that the label
//! mapping lands only under `--out`.

use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// Every UTF-8 text file under `docs/`, `crates/` and `fixtures/`, skipping
/// build directories and version-control internals.
fn text_files() -> Vec<(PathBuf, String)> {
    let root = repository_root();
    let mut files = Vec::new();
    for area in ["docs", "crates", "fixtures"] {
        let dir = root.join(area);
        if dir.is_dir() {
            walk(&dir, &mut files);
        }
    }
    assert!(
        !files.is_empty(),
        "no text files found under the scanned areas"
    );
    files
}

fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
    for entry in fs::read_dir(dir).expect("readable directory") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type().expect("file type");
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if name.starts_with('.') || name.starts_with("target") || name == "node_modules" {
                continue;
            }
            walk(&path, out);
        } else if let Ok(text) = fs::read_to_string(&path) {
            out.push((path, text));
        }
    }
}

fn offending_lines(pattern: &str, needle: impl Fn(&str) -> bool) -> Vec<String> {
    let root = repository_root();
    let mut hits = Vec::new();
    for (path, text) in text_files() {
        for (index, line) in text.lines().enumerate() {
            if needle(line) {
                hits.push(format!(
                    "{}:{}: {pattern}",
                    path.strip_prefix(&root).unwrap_or(&path).display(),
                    index + 1
                ));
            }
        }
    }
    hits
}

#[test]
fn no_absolute_user_paths_in_docs_crates_or_fixtures() {
    // Assembled at run time so this file does not contain the patterns itself.
    for root in ["Users", "home", "Volumes", "private/tmp", "var/folders"] {
        let prefix = format!("/{root}/");
        let hits = offending_lines(&prefix, |line| line.contains(&prefix));
        assert!(
            hits.is_empty(),
            "absolute paths leaked:\n{}",
            hits.join("\n")
        );
    }
}

#[test]
fn no_pilot_names_in_docs_crates_or_fixtures() {
    let mut checked = 0;
    for variable in ["SPECENGINE_PILOT_A", "SPECENGINE_PILOT_B"] {
        let Some(value) = std::env::var_os(variable).filter(|v| !v.is_empty()) else {
            continue;
        };
        let path = fs::canonicalize(&value).unwrap_or_else(|_| PathBuf::from(&value));
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        checked += 1;
        let hits = offending_lines(&format!("<{variable} name>"), |line| line.contains(&name));
        assert!(
            hits.is_empty(),
            "the corpus name of {variable} leaked into the repository:\n{}",
            hits.join("\n")
        );
        let full = path.to_string_lossy().into_owned();
        let hits = offending_lines(&format!("<{variable} path>"), |line| line.contains(&full));
        assert!(
            hits.is_empty(),
            "the corpus path of {variable} leaked into the repository:\n{}",
            hits.join("\n")
        );
    }
    if checked == 0 {
        eprintln!("no SPECENGINE_PILOT_A/_B set: pilot-name check skipped, path check ran");
    }
}

#[test]
fn all_text_is_english_no_cyrillic_letters() {
    let hits = offending_lines("cyrillic", |line| {
        line.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
        // The fixture's own mixed-script test data is explicit escapes, never raw letters.
    });
    assert!(
        hits.is_empty(),
        "non-English text (ADR-0024):\n{}",
        hits.join("\n")
    );
}
