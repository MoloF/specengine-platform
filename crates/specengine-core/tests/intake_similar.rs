//! docs/features/intake-similar.md (RM-0059) "Acceptance criteria", core's
//! part: `similar.rs`'s sources, constants, keyword check, terms and reason
//! lists, as the CLI and the MCP server read them.
//!
//! - IS1 to IS4: the five sources in the list's order, `title`, `alias` and
//!   `queue` gating, `search` and `neighbours` advising (D1). M: search
//!   entries gating.
//! - IS6: `keyword_problem` names `keywords[i]` for a short, a wide, a
//!   spaced, a controlled and a repeated keyword, in input order; the caps
//!   are "Command"'s.
//! - "The entry": an entry is named by its `id`, else its `path`; a reason's
//!   list names at most eight, then `, and <k> more`.

use specengine_core::intake::DISTINCT_MAX;
use specengine_core::similar::{
    KEYWORD_MAX_BYTES, KEYWORD_MIN_CHARS, KEYWORDS_MAX, SIMILAR_GATING_MAX, SIMILAR_NEIGHBOURS_MAX,
    SIMILAR_SEARCH_MAX, SIMILAR_SHARED_MIN, SIMILAR_TARGET_DEGREE_MAX, SIMILAR_TERM_NODES_MAX,
    SIMILAR_TERMS_MAX, SimilarEntry, SimilarSource, keyword_problem, named_list, similar_terms,
    term_words,
};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

/// D1 and "The probes": five sources in order; the exact three gate.
/// M: search entries gating.
#[test]
fn the_exact_three_gate_and_the_rest_advise() {
    let names: Vec<&str> = SimilarSource::ALL.iter().map(|s| s.as_str()).collect();
    assert_eq!(names, ["title", "alias", "queue", "search", "neighbours"]);
    let gating: Vec<bool> = SimilarSource::ALL.iter().map(|s| s.gates()).collect();
    assert_eq!(gating, [true, true, true, false, false]);
    assert!(
        SimilarSource::Title < SimilarSource::Neighbours,
        "the list's order"
    );
    let json = serde_json::to_string(&SimilarSource::ALL).expect("JSON");
    assert_eq!(json, r#"["title","alias","queue","search","neighbours"]"#);
}

/// "Command" and W5: the caps as the spec writes them.
#[test]
fn the_caps_are_the_specs() {
    assert_eq!(SIMILAR_GATING_MAX, DISTINCT_MAX);
    assert_eq!(SIMILAR_GATING_MAX, 64);
    assert_eq!(KEYWORDS_MAX, 16);
    assert_eq!(KEYWORD_MIN_CHARS, 3);
    assert_eq!(KEYWORD_MAX_BYTES, 64);
    assert_eq!(SIMILAR_TERMS_MAX, 32);
    assert_eq!(SIMILAR_TERM_NODES_MAX, 1000);
    assert_eq!(SIMILAR_SEARCH_MAX, 5);
    assert_eq!(SIMILAR_NEIGHBOURS_MAX, 5);
    assert_eq!(SIMILAR_TARGET_DEGREE_MAX, 200);
    assert_eq!(SIMILAR_SHARED_MIN, 2);
}

/// IS6: each broken keyword is `keywords[i]`, the first in input order;
/// the caps themselves pass.
#[test]
fn a_broken_keyword_names_its_index() {
    let field = |list: &[&str]| keyword_problem(&words(list)).map(|problem| problem.field);
    assert_eq!(field(&["drained", "ab"]), Some("keywords[1]".to_owned()));
    assert_eq!(field(&["two words"]), Some("keywords[0]".to_owned()));
    assert_eq!(field(&["tab\there"]), Some("keywords[0]".to_owned()));
    assert_eq!(field(&["bell\u{7}"]), Some("keywords[0]".to_owned()));
    assert_eq!(
        field(&["drained", "rested", "drained"]),
        Some("keywords[2]".to_owned())
    );
    let wide = "k".repeat(65);
    assert_eq!(field(&[&wide]), Some("keywords[0]".to_owned()));
    let at_cap = "k".repeat(64);
    assert_eq!(field(&[&at_cap, "abc"]), None);
    // Three characters, not three bytes: a three-letter Cyrillic word
    // passes (six bytes), a two-letter one does not.
    assert_eq!(field(&["\u{043b}\u{0435}\u{0441}"]), None);
    assert_eq!(field(&["\u{043b}\u{0435}"]), Some("keywords[0]".to_owned()));
    let sixteen: Vec<String> = (0..16).map(|n| format!("word{n:02}")).collect();
    assert_eq!(keyword_problem(&sixteen), None);
    let mut seventeen = sixteen.clone();
    seventeen.push("word16".to_owned());
    let problem = keyword_problem(&seventeen).expect("17 keywords");
    assert!(problem.field.starts_with("keywords"), "{problem:?}");
    assert_eq!(keyword_problem(&[]), None);
}

/// "The probes", probe 4: the titles' terms of three characters or more,
/// then the keywords, lower-cased, each once, at most 32.
#[test]
fn the_terms_are_the_titles_then_the_keywords_once() {
    assert_eq!(
        similar_terms(&["stamina regen of it"], &words(&["Drained", "stamina"])),
        ["stamina", "regen", "drained"]
    );
    let long: Vec<String> = (0..40).map(|n| format!("term{n:02}")).collect();
    let title = long.join(" ");
    let terms = similar_terms(&[title.as_str()], &[]);
    assert_eq!(terms.len(), 32);
    assert_eq!(terms[31], "term31");
    assert!(similar_terms(&["ox"], &[]).is_empty());
}

/// "The entry": named by its id, else its path; at most eight names in a
/// reason's list, then `, and <k> more`.
#[test]
fn an_entry_is_named_by_its_id_else_its_path() {
    let entry = |id: Option<&str>| SimilarEntry {
        id: id.map(str::to_owned),
        path: "docs/spec/keep/lamp.md".to_owned(),
        title: Some("Lamp".to_owned()),
        kind: None,
        source: SimilarSource::Title,
        reason: "title equal to new `MEC-OIL`'s".to_owned(),
        gates: true,
    };
    assert_eq!(entry(Some("MEC-LAMP")).name(), "MEC-LAMP");
    assert_eq!(entry(None).name(), "docs/spec/keep/lamp.md");
    let json = serde_json::to_string(&entry(Some("MEC-LAMP"))).expect("JSON");
    assert_eq!(
        json,
        "{\"id\":\"MEC-LAMP\",\"path\":\"docs/spec/keep/lamp.md\",\"title\":\"Lamp\",\
         \"kind\":null,\"source\":\"title\",\"reason\":\"title equal to new `MEC-OIL`'s\",\
         \"gates\":true}"
    );
    assert_eq!(named_list(&["A", "B"]), "`A`, `B`");
    let ten: Vec<String> = (1..=10).map(|n| format!("T{n}")).collect();
    assert_eq!(
        named_list(&ten),
        "`T1`, `T2`, `T3`, `T4`, `T5`, `T6`, `T7`, `T8`, and 2 more"
    );
    let eight: Vec<String> = (1..=8).map(|n| format!("T{n}")).collect();
    assert!(!named_list(&eight).contains("more"));
}

/// IS1, the review's iteration 2: a U+0000 splits a title's words as
/// whitespace does (it would end an FTS5 string early), the empty parts
/// dropped; the terms are the parts. M: the U+0000 split removed.
#[test]
fn a_nul_splits_the_words_of_a_title() {
    assert_eq!(
        term_words("lan\0tern oil").collect::<Vec<_>>(),
        ["lan", "tern", "oil"]
    );
    assert_eq!(
        term_words("\0lan\0\0tern\0 oil\0").collect::<Vec<_>>(),
        ["lan", "tern", "oil"]
    );
    assert_eq!(
        similar_terms(&["lan\0tern oil"], &[]),
        ["lan", "tern", "oil"]
    );
    assert!(term_words("\0 \0").next().is_none());
}
