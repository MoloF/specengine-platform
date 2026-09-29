//! AC-15 and AC-16 of docs/features/spec-parser.md: the token estimator.
//!
//! AC-15 (calibration against `fixtures/token-calibration/reference.json`)
//! is ignored until the owner supplies the reference counts (Q4). AC-16
//! holds now: empty is 0, a prefix never costs more than the whole, Russian
//! prose costs more than English of equal character count, repeat calls
//! agree, and every fixture node carries the estimate of its own span.

mod common;

use std::fs;

use serde_json::Value;
use specengine_core::tokens_est;

use common::{corpus_scheme, fixture, md_files, text_of};

fn sample(name: &str) -> String {
    fs::read_to_string(fixture("token-calibration").join(format!("{name}.md")))
        .unwrap_or_else(|error| panic!("{name}.md: {error}"))
}

/// Every fixture text of the spec-parser corpora.
fn fixture_texts() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for corpus in ["spec-a", "spec-b", "corpus-mini", "token-calibration"] {
        for (path, bytes) in md_files(&fixture(corpus)) {
            out.push((
                format!("{corpus}/{path}"),
                String::from_utf8(bytes).expect("UTF-8 fixture"),
            ));
        }
    }
    out
}

// ------------------------------------------------------------------- AC-16

#[test]
fn empty_text_costs_nothing_and_any_visible_char_costs_something() {
    assert_eq!(tokens_est(""), 0);
    for text in ["a", " ", "\n", "#", "\u{0436}", "\u{4E2D}", "\u{1F600}"] {
        assert!(tokens_est(text) >= 1, "{text:?}");
    }
}

#[test]
fn no_prefix_of_a_fixture_text_estimates_above_the_whole() {
    for (name, text) in fixture_texts() {
        let whole = tokens_est(&text);
        let mut previous = 0;
        for (index, _) in text.char_indices().chain([(text.len(), ' ')]) {
            let prefix = tokens_est(&text[..index]);
            assert!(prefix <= whole, "{name}: prefix of {index} bytes > whole");
            assert!(
                prefix >= previous,
                "{name}: estimate shrank at byte {index}"
            );
            previous = prefix;
        }
        assert!(whole > 0, "{name}");
    }
}

#[test]
fn russian_prose_costs_more_than_english_of_equal_char_count() {
    let english = sample("english");
    let russian = sample("russian");
    let count = english.chars().count().min(russian.chars().count());
    assert!(count >= 150, "samples are long enough ({count})");
    let english: String = english.chars().take(count).collect();
    let russian: String = russian.chars().take(count).collect();
    assert!(
        tokens_est(&russian) > tokens_est(&english),
        "russian {} vs english {} over {count} chars",
        tokens_est(&russian),
        tokens_est(&english)
    );
    // Letter against letter, whitespace and punctuation aside.
    let cyrillic_letters: String = "\u{0430}\u{0431}\u{0432}".repeat(100);
    let latin_letters: String = "abc".repeat(100);
    assert!(tokens_est(&cyrillic_letters) > tokens_est(&latin_letters));
}

#[test]
fn repeat_calls_agree() {
    for (name, text) in fixture_texts() {
        let first = tokens_est(&text);
        for _ in 0..3 {
            assert_eq!(tokens_est(&text), first, "{name}");
        }
    }
}

#[test]
fn every_fixture_node_carries_the_estimate_of_its_span() {
    let mut nodes = 0;
    for corpus in ["spec-a", "spec-b", "corpus-mini"] {
        let dir = fixture(corpus);
        let scheme = corpus_scheme(&dir);
        for (path, bytes) in md_files(&dir) {
            let parsed = specengine_core::parse(&path, &bytes, &scheme);
            let json: Value = serde_json::from_str(&common::json(&parsed)).unwrap();
            for (index, node) in parsed.nodes.iter().enumerate() {
                assert!(
                    json["nodes"][index]["tokens_est"].is_u64(),
                    "{corpus}/{path}: node {index} serialises tokens_est"
                );
                let want = tokens_est(text_of(&bytes, node.span));
                assert_eq!(node.tokens_est, want, "{corpus}/{path}: node {index}");
                assert!(node.tokens_est > 0, "{corpus}/{path}: node {index}");
                nodes += 1;
            }
            // A document costs the whole file.
            assert_eq!(
                parsed.document().unwrap().tokens_est,
                tokens_est(std::str::from_utf8(&bytes).unwrap())
            );
        }
    }
    assert!(nodes >= 40, "{nodes} nodes checked");
}

#[test]
fn the_estimate_saturates_instead_of_overflowing() {
    // 5 000 000 CJK chars at a conservative weight stay far below u32::MAX;
    // the check is that a large input is finite and monotone, not wrapped.
    let big = "\u{4E2D}".repeat(5_000_000);
    let estimate = tokens_est(&big);
    assert!(estimate >= 1_000_000, "{estimate}");
    assert!(estimate < u32::MAX);
}

// ------------------------------------------------------------------- AC-15

#[test]
#[ignore = "reference counts pending owner (Q4)"]
fn calibration_samples_are_within_fifteen_percent_of_the_reference() {
    let reference: Value = serde_json::from_str(
        &fs::read_to_string(fixture("token-calibration").join("reference.json")).unwrap(),
    )
    .unwrap();
    assert!(
        reference["model"].is_string(),
        "reference.json names no tokenizer model yet (Q4)"
    );
    let counts = reference["counts"].as_object().expect("counts");
    let mut estimated_sum = 0u64;
    let mut reference_sum = 0u64;
    for (name, count) in counts {
        let want = count
            .as_u64()
            .unwrap_or_else(|| panic!("{name}: reference count pending (Q4)"));
        let got = u64::from(tokens_est(&sample(name)));
        let low = want * 85 / 100;
        let high = want * 115 / 100;
        assert!(
            (low..=high).contains(&got),
            "{name}: estimate {got} outside ±15 % of {want}"
        );
        estimated_sum += got;
        reference_sum += want;
    }
    assert!(
        estimated_sum * 100 >= reference_sum * 95,
        "sum {estimated_sum} more than 5 % below {reference_sum}"
    );
}

#[test]
fn calibration_fixture_has_the_five_samples_and_an_empty_reference() {
    let reference: Value = serde_json::from_str(
        &fs::read_to_string(fixture("token-calibration").join("reference.json")).unwrap(),
    )
    .unwrap();
    let counts = reference["counts"].as_object().expect("counts");
    let names: Vec<&str> = counts.keys().map(String::as_str).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        ["code-block", "english", "mixed", "russian", "table"]
    );
    for name in names {
        assert!(!sample(name).is_empty(), "{name}.md");
    }
}
