//! The vocabulary of the process-rule tests (docs/features/spec-check-process.md,
//! ADR-0031): every key, label and value a test config of this task names,
//! in the core and the CLI tests alike. `check_genre.rs` (AC-14) pins that
//! the rules sources hold none of them; the core tests refuse a config
//! naming a word missing here, so the list cannot fall behind.

use std::collections::BTreeSet;

use specengine_core::check::CheckConfig;

use super::fixture;

/// Keys (`keys`, `when`, `values`) and labels (`parts`) of the test
/// configs, which [`unlisted`] enforces, then the main values they allow
/// (scanned too, not enforced). The Cyrillic label lives in
/// `fixtures/spec-b/check-rules.toml` only (ADR-0024).
pub const RULE_WORDS: &[&str] = &[
    // keys
    "to",
    "working_answer",
    "status",
    "owner",
    "reviewed",
    "acceptance",
    "kind",
    "nope",
    // labels
    "Working answer",
    "Implementation",
    "Cost",
    // values
    "open",
    "answered",
    "deferred",
    "dropped",
    "customer",
    "team",
    "shipped",
    "accepted",
];

/// spec-b's rules fragment, appended to a copy's `specengine.toml`.
pub fn spec_b_rules() -> String {
    std::fs::read_to_string(fixture("spec-b").join("check-rules.toml")).expect("the rules fragment")
}

/// Every key (`keys`, `when`, `values`) and label `check` names.
pub fn words_of(check: &CheckConfig) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    for rule in &check.rules {
        words.extend(rule.keys.iter().cloned());
        words.extend(rule.parts.iter().cloned());
        for (key, _) in rule.when.iter().chain(&rule.values) {
            words.insert(key.clone());
        }
    }
    words
}

/// The keys and labels of `check` missing from [`RULE_WORDS`] and spec-b's
/// fragment.
pub fn unlisted(check: &CheckConfig) -> Vec<String> {
    let fragment = format!(
        "[ids]\nQN = {{ kind = \"question\", width = 2 }}\n{}",
        spec_b_rules()
    );
    let listed = words_of(&CheckConfig::from_toml(&fragment).expect("spec-b's fragment"));
    words_of(check)
        .into_iter()
        .filter(|word| !RULE_WORDS.contains(&word.as_str()) && !listed.contains(word))
        .collect()
}
