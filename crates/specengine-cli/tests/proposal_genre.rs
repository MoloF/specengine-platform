//! docs/features/proposal-apply.md AC-20, the scan half (the run half:
//! AC-01, AC-07, AC-11 loop over both fixtures in `proposal_create.rs` and
//! `proposal_apply.rs`): the new modules — core `patch`, `proposal`; store
//! `queue`, `update`, `worktree`; CLI `apply`, `inbox`, `preflight`,
//! `proposals`, `propose`, `review` — name no prefix or alias of either
//! fixture, no fixture slug, path or file, as core's `tests/check_genre.rs`
//! scans the check sources (string literals outside comments); and the
//! slice adds no dependency ("Roles": no manifest or lock change, no
//! `similar`, no `rusqlite_migration`). M: a hard-coded prefix; `similar`
//! added.

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fs;

use common::{FIXTURES, fixture, read_text, repository_root};
use specengine_core::ProjectConfig;

/// The new sources of the slice.
const NEW_SOURCES: [&str; 11] = [
    "crates/specengine-core/src/patch.rs",
    "crates/specengine-core/src/proposal.rs",
    "crates/specengine-store/src/queue.rs",
    "crates/specengine-store/src/update.rs",
    "crates/specengine-store/src/worktree.rs",
    "crates/specengine-cli/src/apply.rs",
    "crates/specengine-cli/src/inbox.rs",
    "crates/specengine-cli/src/preflight.rs",
    "crates/specengine-cli/src/proposals.rs",
    "crates/specengine-cli/src/propose.rs",
    "crates/specengine-cli/src/review.rs",
];

/// Paths, files and names of the fixtures and of this repository.
const PROJECT_NAMES: &[&str] = &[
    "fixtures",
    "spec-a",
    "spec-b",
    "docs/",
    "records/",
    "features/",
    "decisions",
    "CLAUDE.md",
    "README.md",
    "index.md",
    "stamina",
    "sprint",
    "glossary",
    "dry-run",
];

/// Every string literal of a Rust line outside comments (naive: the text
/// between pairs of `"`), as core's `check_genre.rs` takes them.
fn literals(line: &str) -> Vec<&str> {
    let code = line.trim_start();
    if code.starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

/// The offending literals of `text` (named `source`).
fn offenders(source: &str, text: &str, prefixes: &BTreeSet<String>, names: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        for literal in literals(line) {
            let names_prefix = prefixes.iter().any(|prefix| {
                literal == prefix
                    || literal.starts_with(&format!("{prefix}-"))
                    || literal.contains(&format!(" {prefix}-"))
                    || literal.contains(&format!("`{prefix}-"))
            });
            let names_project = names.iter().any(|name| literal.contains(name));
            if names_prefix || names_project {
                found.push(format!("{source}:{}: {literal:?}", number + 1));
            }
        }
    }
    found
}

fn fixture_prefixes() -> BTreeSet<String> {
    let mut prefixes = BTreeSet::new();
    for (name, _) in FIXTURES {
        let config = ProjectConfig::from_toml(&read_text(&fixture(name), "specengine.toml"))
            .expect("the fixture's config");
        for spec in config.scheme.prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
        }
    }
    prefixes
}

#[test]
fn ac20_the_new_modules_name_no_fixture_prefix_slug_or_path() {
    let prefixes = fixture_prefixes();
    assert!(
        prefixes.contains("RULE") && prefixes.contains("CMD") && prefixes.contains("QST"),
        "{prefixes:?}"
    );
    let mut names: Vec<&str> = PROJECT_NAMES.to_vec();
    names.extend(FIXTURES.iter().map(|(_, slug)| *slug));
    let mut found = Vec::new();
    for source in NEW_SOURCES {
        let text = fs::read_to_string(repository_root().join(source))
            .unwrap_or_else(|error| panic!("{source}: {error}"));
        found.extend(offenders(source, &text, &prefixes, &names));
    }
    assert!(
        found.is_empty(),
        "fixture names in the new modules:\n{}",
        found.join("\n")
    );
}

/// The scan sees what it must refuse.
#[test]
fn the_scan_sees_a_hard_coded_prefix() {
    let prefixes = fixture_prefixes();
    let names: Vec<&str> = PROJECT_NAMES.to_vec();
    let sample = "let a = \"RULE-STAM-REGEN\";\n// \"CMD-SYNC\" in a comment\nlet b = format!(\"see `QST-031`\");\nlet c = \"ok PR-0001\";\nlet d = \"fixtures/spec-a\";\n";
    let found = offenders("sample.rs", sample, &prefixes, &names);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].starts_with("sample.rs:1:"), "{found:?}");
    assert!(found[1].starts_with("sample.rs:3:"), "{found:?}");
    assert!(found[2].starts_with("sample.rs:5:"), "{found:?}");
}

/// The direct dependency names of `manifest`'s `[dependencies]` and
/// `[dev-dependencies]` tables.
fn dependencies(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[dependencies]" || line == "[dev-dependencies]";
            continue;
        }
        if inside
            && !line.starts_with('#')
            && let Some((name, _)) = line.split_once('=')
        {
            let name = name.trim();
            out.insert(name.strip_suffix(".workspace").unwrap_or(name).to_owned());
        }
    }
    out
}

/// "Roles": the slice changes no manifest: the four crates keep their
/// direct dependencies, and neither `similar` nor `rusqlite_migration`
/// enters a manifest or the lock.
#[test]
fn ac20_no_dependency_is_added() {
    let root = repository_root();
    let set = |names: &[&str]| {
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>()
    };
    for (manifest, want) in [
        (
            "crates/specengine-cli/Cargo.toml",
            set(&[
                "specengine-model",
                "specengine-core",
                "specengine-store",
                "clap",
                "serde",
                "serde_json",
            ]),
        ),
        (
            "crates/specengine-store/Cargo.toml",
            set(&[
                "specengine-model",
                "specengine-core",
                "rusqlite",
                "blake3",
                "serde",
                "serde_json",
            ]),
        ),
        (
            "crates/specengine-core/Cargo.toml",
            set(&[
                "specengine-model",
                "pulldown-cmark",
                "serde-saphyr",
                "serde",
                "toml",
                "serde_json",
                "petgraph",
            ]),
        ),
        (
            "crates/specengine-mcp/Cargo.toml",
            set(&[
                "specengine-cli",
                "rmcp",
                "tokio",
                "getrandom",
                "serde",
                "serde_json",
                "clap",
            ]),
        ),
    ] {
        let text = read_text(&root, manifest);
        assert_eq!(dependencies(&text), want, "{manifest}");
    }
    let mut manifests = vec!["Cargo.toml".to_owned(), "Cargo.lock".to_owned()];
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let path = entry.unwrap().path().join("Cargo.toml");
        if path.exists() {
            manifests.push(path.strip_prefix(&root).unwrap().display().to_string());
        }
    }
    for manifest in &manifests {
        let text = read_text(&root, manifest);
        for crate_name in ["similar", "rusqlite_migration", "rusqlite-migration"] {
            assert!(
                !text.lines().any(|line| {
                    let line = line.trim();
                    line == format!("name = \"{crate_name}\"")
                        || line.starts_with(&format!("{crate_name} ="))
                        || line.starts_with(&format!("{crate_name}."))
                }),
                "{manifest} names `{crate_name}`"
            );
        }
    }
}
