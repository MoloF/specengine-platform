//! AC-15 and AC-16 of docs/features/spec-parser.md: the token estimator,
//! with AC-01 to AC-05 of docs/features/token-calibration.md.
//!
//! AC-15 runs: every calibration sample estimates within ±15 % of its count
//! in `fixtures/token-calibration/reference.json` (Anthropic `count_tokens`,
//! `claude-opus-5-5`, 2026-10-04), with the reference moved by one either
//! way, and the estimated sum is not below the reference sum; the reference
//! file is self-consistent (counts = input_tokens − overhead, overhead =
//! baseline − 1). AC-16 holds: empty is 0, a prefix never costs more than
//! the whole, Russian prose costs more than English of equal character
//! count, repeat calls agree, and every fixture node carries the estimate of
//! its own span.

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

/// The five calibration samples, `fixtures/token-calibration/<name>.md`.
const SAMPLES: [&str; 5] = ["code-block", "english", "mixed", "russian", "table"];

fn reference() -> Value {
    let path = fixture("token-calibration").join("reference.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("reference.json"))
        .unwrap_or_else(|error| panic!("reference.json: {error}"))
}

fn count(map: &Value, name: &str) -> u64 {
    map[name]
        .as_u64()
        .unwrap_or_else(|| panic!("reference.json: {name} is not a count: {map}"))
}

fn names(map: &Value, key: &str) -> Vec<String> {
    map[key]
        .as_object()
        .unwrap_or_else(|| panic!("reference.json: no `{key}` map"))
        .keys()
        .cloned()
        .collect()
}

/// token-calibration AC-02, AC-03: each estimate within ±15 % of its
/// reference count, for the count and the count moved by one either way;
/// the estimated sum is not below the reference sum.
/// M: the weights before calibration; `chars / 4`; CYRILLIC 500 (mixed 66);
/// ASCII_ALNUM 330 (sum 464).
#[test]
fn calibration_samples_are_within_fifteen_percent_of_the_reference() {
    let reference = reference();
    let counts = &reference["counts"];
    let mut estimated_sum = 0u64;
    let mut reference_sum = 0u64;
    for name in SAMPLES {
        let want = count(counts, name);
        let got = u64::from(tokens_est(&sample(name)));
        for r in [want - 1, want, want + 1] {
            let (low, high) = (r * 85 / 100, r * 115 / 100);
            assert!(
                (low..=high).contains(&got),
                "{name}: estimate {got} outside {low}..={high} (±15 % of {r}, reference {want})"
            );
        }
        estimated_sum += got;
        reference_sum += want;
    }
    assert!(
        estimated_sum >= reference_sum,
        "estimated sum {estimated_sum} below the reference sum {reference_sum}"
    );
}

/// token-calibration AC-01: the reference names the model, the date and the
/// method; `counts + overhead = input_tokens` for each sample, the overhead
/// is the one-token baseline less that token, and both maps hold exactly
/// the five samples, each a non-empty file. M: table 111.
#[test]
fn calibration_fixture_has_the_five_samples_and_consistent_reference_counts() {
    let reference = reference();
    for key in ["model", "date", "method"] {
        assert!(
            reference[key]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "reference.json: `{key}` is not a non-empty string: {reference}"
        );
    }
    assert_eq!(reference["baseline"]["text"], "x", "{reference}");
    let baseline = count(&reference["baseline"], "input_tokens");
    let overhead = reference["overhead"].as_u64().expect("overhead");
    assert_eq!(overhead, baseline - 1, "overhead = baseline − 1");
    for key in ["counts", "input_tokens"] {
        let mut keys = names(&reference, key);
        keys.sort_unstable();
        assert_eq!(keys, SAMPLES, "reference.json `{key}`");
    }
    for name in SAMPLES {
        assert_eq!(
            count(&reference["counts"], name) + overhead,
            count(&reference["input_tokens"], name),
            "{name}: counts + overhead = input_tokens"
        );
        assert!(!sample(name).is_empty(), "{name}.md");
    }
}

/// token-calibration AC-05, the documentation half: each of the six weight
/// constants carries a reason, and the module doc names the reference's
/// model, date and file (a new reference rewrites it). The values are not
/// pinned here: the calibration test bounds them, and the store's format
/// history pins the estimates of the fixture nodes.
#[test]
fn the_weights_carry_reasons_and_the_module_doc_names_the_reference() {
    let source =
        fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tokens.rs"))
            .expect("src/tokens.rs");
    let lines: Vec<&str> = source.lines().collect();
    let constants: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim_start().starts_with("pub const "))
        .collect();
    assert_eq!(constants.len(), 6, "six weight constants");
    for i in constants {
        assert!(
            i > 0 && lines[i - 1].trim_start().starts_with("///"),
            "src/tokens.rs:{}: a weight without a reason: {}",
            i + 1,
            lines[i]
        );
    }
    let module_doc: String = lines
        .iter()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect::<Vec<_>>()
        .join(" ");
    let reference = reference();
    for named in [
        reference["model"].as_str().unwrap(),
        reference["date"].as_str().unwrap(),
        "fixtures/token-calibration/reference.json",
    ] {
        assert!(
            module_doc.contains(named),
            "module doc names {named}: {module_doc}"
        );
    }
}
