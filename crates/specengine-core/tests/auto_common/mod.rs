//! Helpers of the auto-mode core tests (docs/features/auto-rule.md
//! "Acceptance criteria": `tests/auto_rule.rs`, `tests/auto_predicate.rs`):
//! `fixtures/auto-mini`'s config, its rule variants under `rules/`, and the
//! config with one of them appended (as a test appends and commits it), all
//! read from the fixture, never written.
//!
//! Only the auto-mode test binaries include this module (`mod
//! auto_common;`), so the other core test binaries are not rebuilt.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The fixture's directory.
pub fn auto_mini() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/auto-mini")
        .canonicalize()
        .expect("fixtures/auto-mini exists")
}

/// The fixture's `specengine.toml`, with no `[auto]` table.
pub fn base_config() -> String {
    fs::read_to_string(auto_mini().join("specengine.toml")).expect("the fixture config")
}

/// `rules/<name>`, as written.
pub fn rule(name: &str) -> String {
    fs::read_to_string(auto_mini().join("rules").join(name))
        .unwrap_or_else(|e| panic!("rules/{name}: {e}"))
}

/// `config` with `table` appended after one blank line.
pub fn with_table(config: &str, table: &str) -> String {
    format!("{}\n\n{table}", config.trim_end())
}

/// The fixture's config with `rules/<name>` appended.
pub fn config_with(name: &str) -> String {
    with_table(&base_config(), &rule(name))
}

/// The line of `config_with(_)` that holds line `k` (1-based) of the
/// appended table.
pub fn appended_line(k: usize) -> usize {
    base_config().trim_end().lines().count() + 1 + k
}

/// The fixture's `expected.json`.
pub fn expected() -> Value {
    serde_json::from_str(
        &fs::read_to_string(auto_mini().join("expected.json")).expect("expected.json"),
    )
    .expect("expected.json is JSON")
}

/// The names of `rules/bad-*.toml`, sorted.
pub fn bad_rules() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(auto_mini().join("rules"))
        .expect("rules/")
        .map(|entry| entry.expect("an entry").file_name().into_string().unwrap())
        .filter(|name| name.starts_with("bad-") && name.ends_with(".toml"))
        .collect();
    names.sort();
    names
}

/// `text` with the one occurrence of `from` replaced by `to`.
pub fn edit(text: &str, from: &str, to: &str) -> String {
    assert_eq!(text.matches(from).count(), 1, "{from:?} once in {text}");
    text.replacen(from, to, 1)
}
