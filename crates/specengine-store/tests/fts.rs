//! AC-10 – AC-14 of docs/features/spec-index.md: full-text search.
//!
//! - AC-10: a word removed by an edit, or only in a deleted file, has no hit;
//!   FTS5 `integrity-check` (rank 1) passes after every step of the AC-07
//!   script.
//! - AC-11: a word of `fixtures/spec-b/docs/spec/cli.md` line 25 found
//!   nowhere else has one hit, `FLAG-DRY-RUN` (a node's own text excludes the
//!   ID sections inside it).
//! - AC-12: a capitalised Cyrillic word is found by its lower-case form; the
//!   four-letter stem `\u{043a}\u{043e}\u{043f}\u{0438}` hits `MOD-CLI` (line 15) and `CMD-SYNC`
//!   (line 20) — trigram substrings, no stemmer. Queries are `\u{…}` escapes
//!   (ADR-0024).
//! - AC-13: IDs, paths and FTS5 syntax characters are searched as text,
//!   never as syntax; `RULE-STAM` finds `RULE-STAM-REGEN`; a query without a
//!   term of three characters sets `short_query`.
//! - AC-14: two nodes of identical text in different files tie on rank and
//!   are ordered by path, identically for a fresh index, the AC-07 path and
//!   a reversed walk.

#![cfg(unix)]

mod common;

use common::{Corpus, Reversed, Scratch, ac07_script};
use specengine_store::{
    IndexWriter, SEARCH_LIMIT_MAX, SearchHit, SearchQuery, SpecIndex, SqliteIndex,
};

fn search(index: &SqliteIndex, text: &str) -> Vec<SearchHit> {
    let results = index
        .search(&SearchQuery::new(text))
        .unwrap_or_else(|error| panic!("search {text:?}: {error}"));
    assert!(!results.short_query, "search {text:?}: not a short query");
    results.hits
}

fn keys(hits: &[SearchHit]) -> Vec<(String, usize, Option<String>)> {
    hits.iter()
        .map(|hit| (hit.path.clone(), hit.ord, hit.id.clone()))
        .collect()
}

// ------------------------------------------------------------------ AC-10

#[test]
fn a_removed_or_deleted_word_has_no_hit() {
    let scratch = Scratch::new("fts-removed");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);

    // Removed by an edit: `Drained` lives only in stamina.md's own text.
    let before = search(&index, "Drained");
    assert!(!before.is_empty(), "the word is indexed first");
    assert!(
        before
            .iter()
            .all(|hit| hit.path == "docs/spec/movement/stamina.md"),
        "{:?}",
        keys(&before)
    );
    corpus.replace(
        "docs/spec/movement/stamina.md",
        "Drained while sprinting",
        "Spent while sprinting",
    );
    corpus.update(&mut index);
    assert_eq!(keys(&search(&index, "Drained")), [], "a removed word");
    assert!(
        !search(&index, "Spent while").is_empty(),
        "the new words are found"
    );

    // Only in a deleted file: `minute` lives only in A-102 (its title).
    let before = search(&index, "minute");
    assert!(!before.is_empty(), "the word is indexed first");
    assert!(
        before
            .iter()
            .all(|hit| hit.path == "docs/records/A/A-102.md"),
        "{:?}",
        keys(&before)
    );
    corpus.remove("docs/records/A/A-102.md");
    corpus.update(&mut index);
    assert_eq!(keys(&search(&index, "minute")), [], "a deleted file's word");
    index
        .check_fts()
        .expect("FTS5 integrity-check after the edits");
}

#[test]
fn fts_integrity_check_passes_after_every_step_of_the_edit_script() {
    let scratch = Scratch::new("fts-integrity");
    let mut corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    index
        .check_fts()
        .expect("integrity-check after the first index");
    for (step, edit) in ac07_script() {
        edit(&mut corpus);
        corpus.update(&mut index);
        index
            .check_fts()
            .unwrap_or_else(|error| panic!("integrity-check after {step}: {error}"));
    }
    index
        .rebuild(&corpus.tree(), &corpus.scheme)
        .expect("rebuild");
    index.check_fts().expect("integrity-check after a rebuild");
}

// ------------------------------------------------------------------ AC-11

/// Two words of cli.md line 25 ("nothing", "writes") found nowhere else in spec-b.
const NICHEGO: &str = "\u{043d}\u{0438}\u{0447}\u{0435}\u{0433}\u{043e}";
const PISHET: &str = "\u{043f}\u{0438}\u{0448}\u{0435}\u{0442}";

#[test]
fn a_word_of_a_nested_section_has_one_hit_its_own_section() {
    let scratch = Scratch::new("fts-own-text");
    let corpus = Corpus::copy_of("spec-b", &scratch, "wt");
    let line = corpus
        .read_text("docs/spec/cli.md")
        .lines()
        .nth(24)
        .expect("cli.md line 25")
        .to_owned();
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    for word in [NICHEGO, PISHET] {
        assert!(line.contains(word), "cli.md line 25 holds the query word");
        let elsewhere: Vec<String> = corpus
            .listing()
            .paths
            .into_iter()
            .filter(|path| {
                corpus.read_text(path).to_lowercase().matches(word).count()
                    != usize::from(path == "docs/spec/cli.md")
            })
            .collect();
        assert!(
            elsewhere.is_empty(),
            "the word occurs once in spec-b: {elsewhere:?}"
        );
        let hits = search(&index, word);
        assert_eq!(
            keys(&hits),
            [(
                "docs/spec/cli.md".to_owned(),
                2,
                Some("FLAG-DRY-RUN".to_owned())
            )],
            "one hit, the innermost section (not CMD-SYNC, not MOD-CLI)"
        );
        assert_eq!(hits[0].kind.as_deref(), Some("flag"));
        assert!(
            hits[0].snippet.contains(&format!("**{word}**")),
            "the snippet marks the match: {}",
            hits[0].snippet
        );
    }
}

// ------------------------------------------------------------------ AC-12

/// "Tool", capitalised on cli.md line 15, and its lower-case form.
const INSTRUMENT_CAPITAL: &str =
    "\u{0418}\u{043d}\u{0441}\u{0442}\u{0440}\u{0443}\u{043c}\u{0435}\u{043d}\u{0442}";
const INSTRUMENT_LOWER: &str =
    "\u{0438}\u{043d}\u{0441}\u{0442}\u{0440}\u{0443}\u{043c}\u{0435}\u{043d}\u{0442}";
/// The stem "cop-" (of "copies" on line 15, "copy" on line 20).
const KOPI: &str = "\u{043a}\u{043e}\u{043f}\u{0438}";

#[test]
fn cyrillic_is_case_folded_and_a_stem_matches_as_a_substring() {
    let scratch = Scratch::new("fts-cyrillic");
    let corpus = Corpus::copy_of("spec-b", &scratch, "wt");
    let cli = corpus.read_text("docs/spec/cli.md");
    let lines: Vec<&str> = cli.lines().collect();
    assert!(lines[14].contains(INSTRUMENT_CAPITAL), "cli.md line 15");
    assert!(
        lines[14].contains(KOPI) && lines[19].contains(KOPI),
        "cli.md lines 15, 20"
    );
    for path in corpus.listing().paths {
        assert!(
            !corpus.read_text(&path).contains(INSTRUMENT_LOWER),
            "{path}: the lower-case form must not occur literally"
        );
    }
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);

    let hits = search(&index, INSTRUMENT_LOWER);
    assert_eq!(
        keys(&hits),
        [("docs/spec/cli.md".to_owned(), 0, Some("MOD-CLI".to_owned()))],
        "the lower-case query finds the capitalised word"
    );
    assert_eq!(keys(&search(&index, INSTRUMENT_CAPITAL)), keys(&hits));

    let ids: Vec<Option<String>> = search(&index, KOPI).into_iter().map(|hit| hit.id).collect();
    for wanted in ["MOD-CLI", "CMD-SYNC"] {
        assert!(
            ids.contains(&Some(wanted.to_owned())),
            "the stem hits {wanted}: {ids:?}"
        );
    }
}

// ------------------------------------------------------------------ AC-13

#[test]
fn ids_paths_and_syntax_characters_are_searched_as_text() {
    let scratch = Scratch::new("fts-syntax");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);

    let queries = [
        "R-12",
        "RULE-STAM",
        "stamina::regen",
        "MEC-STAMINA#RULE-STAM-REGEN@3",
        "docs/spec/game.md",
        "\"",
        "\"stamina",
        "stam\"ina",
        "\"\"\"\"",
        "(",
        "(stamina",
        "stamina)",
        "*",
        "stam*",
        "***",
        "NOT",
        "NOT stamina",
        "stamina NOT sprint",
        "AND",
        "stamina OR sprint",
        "NEAR(stamina sprint)",
        "id:stamina",
        "{id title}: stamina",
        "^stamina",
        "-stamina",
        "+stamina",
        "'stamina'",
        "stamina:",
        "\\",
        "%_%",
        "\u{1F3EE} lantern",
        "\t\n  ",
        "",
    ];
    for query in queries {
        index
            .search(&SearchQuery::new(query))
            .unwrap_or_else(|error| panic!("search {query:?} must be Ok: {error}"));
    }

    // Substrings of IDs.
    let hits = search(&index, "RULE-STAM");
    assert!(
        hits.iter()
            .any(|hit| hit.id.as_deref() == Some("RULE-STAM-REGEN")),
        "RULE-STAM finds RULE-STAM-REGEN: {:?}",
        keys(&hits)
    );
    assert!(!search(&index, "R-12").is_empty(), "R-12 is found");

    // `NOT` is a term, not an operator: every hit also matches `not`.
    let with_not = search(&index, "stamina NOT sprint");
    let not_alone = search(&index, "not");
    for hit in &with_not {
        assert!(
            not_alone
                .iter()
                .any(|other| other.path == hit.path && other.ord == hit.ord),
            "{}#{} does not contain `not`",
            hit.path,
            hit.ord
        );
    }

    // Short queries: no term of three characters left.
    for short in ["ab", "R", "a b", "", "  ", "(*", "\u{1F3EE}"] {
        let results = index.search(&SearchQuery::new(short)).expect("Ok");
        assert!(results.short_query, "{short:?} is a short query");
        assert!(results.hits.is_empty(), "{short:?} has no hits");
    }
    // A short term beside a long one is dropped, not fatal.
    let mixed = index.search(&SearchQuery::new("ab stamina")).expect("Ok");
    assert!(!mixed.short_query);
    assert_eq!(keys(&mixed.hits), keys(&search(&index, "stamina")));

    // `kinds` filters; `limit` is clamped to 1..=200.
    let mut query = SearchQuery::new("stamina");
    query.kinds = vec!["rule".to_owned(), "edge-case".to_owned()];
    let filtered = index.search(&query).expect("Ok").hits;
    assert!(!filtered.is_empty());
    assert!(
        filtered
            .iter()
            .all(|hit| matches!(hit.kind.as_deref(), Some("rule" | "edge-case"))),
        "{:?}",
        filtered.iter().map(|hit| &hit.kind).collect::<Vec<_>>()
    );
    let mut query = SearchQuery::new("the");
    query.limit = 0;
    assert_eq!(
        index.search(&query).expect("Ok").hits.len(),
        1,
        "limit 0 → 1"
    );
    query.limit = 100_000;
    assert!(index.search(&query).expect("Ok").hits.len() <= SEARCH_LIMIT_MAX);
    assert_eq!(SearchQuery::new("x").limit, 20, "the default limit");
}

// ------------------------------------------------------------------ AC-14

const TWIN_TEXT: &str = "# Glimmerwick lamps\n\nThe glimmerwick lamp burns without oil.\n";

fn searches(index: &SqliteIndex) -> Vec<Vec<SearchHit>> {
    ["glimmerwick", "stamina", "lantern", "sprint", "rule", "the"]
        .into_iter()
        .map(|query| search(index, query))
        .collect()
}

#[test]
fn ties_order_by_path_whatever_the_insertion_order() {
    let scratch = Scratch::new("fts-ties");
    let mut corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write("docs/spec/twin-a.md", TWIN_TEXT);
    corpus.write("docs/spec/twin-b.md", TWIN_TEXT);

    // The AC-07 path.
    let mut incremental = corpus.open(&scratch.db("incremental"));
    corpus.update(&mut incremental);
    for (_, edit) in ac07_script() {
        edit(&mut corpus);
        corpus.update(&mut incremental);
    }
    // A fresh index, and one written in reverse walk order.
    let fresh = corpus.fresh(&scratch);
    let mut reversed = corpus.open(&scratch.db("reversed"));
    let tree = corpus.tree();
    reversed
        .update(&Reversed(&tree), &corpus.scheme)
        .expect("reversed update");
    assert_eq!(reversed.dump().unwrap(), fresh.dump().unwrap(), "same rows");

    let twins = search(&fresh, "glimmerwick");
    assert_eq!(
        twins
            .iter()
            .map(|hit| (hit.path.as_str(), hit.ord))
            .collect::<Vec<_>>(),
        [("docs/spec/twin-a.md", 0), ("docs/spec/twin-b.md", 0)],
        "a tie orders by path"
    );
    assert_eq!(twins[0].snippet, twins[1].snippet);
    let expected = searches(&fresh);
    assert_eq!(searches(&incremental), expected, "the AC-07 path");
    assert_eq!(searches(&reversed), expected, "a reversed walk");
    // The same again after a rebuild in reverse order.
    reversed
        .rebuild(&Reversed(&tree), &corpus.scheme)
        .expect("reversed rebuild");
    assert_eq!(searches(&reversed), expected, "a reversed rebuild");
}
