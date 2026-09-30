//! AC-12 of docs/features/spec-cli.md: `spec search` keeps the store's
//! order, prints a line and a one-line snippet per hit, filters by
//! `--kind`, drops terms under 3 characters with a note, answers zero hits
//! with exit 0, finds a capitalised Cyrillic word by its lower case, and
//! repeats byte for byte.

#![cfg(unix)]

mod common;

use std::path::Path;

use common::{FIXTURES, Scratch, data_dir, index, read, spec};
use specengine_store::{SearchQuery, SpecIndex as _, SqliteIndex};

/// The store's own answer for `query` over the CLI's database.
fn store_hits(home: &Path, root: &Path, slug: &str, query: &SearchQuery) -> Vec<(String, usize)> {
    let index = SqliteIndex::open(data_dir(home).join(format!("{slug}.db")), slug, root)
        .expect("open the CLI's database");
    index
        .search(query)
        .expect("search")
        .hits
        .into_iter()
        .map(|hit| (hit.path, hit.ord))
        .collect()
}

fn json_hits(json: &serde_json::Value) -> Vec<(String, usize)> {
    json["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| {
            (
                hit["path"].as_str().unwrap().to_owned(),
                usize::try_from(hit["ord"].as_u64().unwrap()).unwrap(),
            )
        })
        .collect()
}

/// `(fixture, a query with many hits)`.
const QUERIES: [(&str, &str); 2] = [("spec-a", "stamina sprint"), ("spec-b", "sync")];

#[test]
fn search_keeps_the_store_order_with_a_line_and_snippet_per_hit() {
    for ((fixture, slug), (_, query)) in FIXTURES.iter().zip(QUERIES) {
        let scratch = Scratch::new("search");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        index(&home, &root);
        let terms: Vec<&str> = query.split(' ').collect();
        let mut args = vec!["--json", "search"];
        args.extend(&terms);
        let run = spec(&home, &root, &args);
        run.code(0);
        let json = run.json();
        let hits = json_hits(&json);
        assert!(hits.len() > 3, "{fixture}: {query}: {hits:?}");
        let mut store_query = SearchQuery::new(query);
        store_query.limit = 20;
        assert_eq!(
            hits,
            store_hits(&home, &root, slug, &store_query),
            "{fixture}: the store's order"
        );
        assert_eq!(json["query"], query);
        assert_eq!(json["limit"], 20);
        assert_eq!(json["kinds"], serde_json::json!([]));
        assert_eq!(json["notes"], serde_json::json!([]));

        // The text: two lines per hit, then the summary.
        let mut text_args = vec!["search"];
        text_args.extend(&terms);
        let text = spec(&home, &root, &text_args);
        text.code(0);
        let lines: Vec<&str> = text.stdout.lines().collect();
        let hit_list = json["hits"].as_array().unwrap();
        assert_eq!(
            lines.len(),
            hit_list.len() * 2 + 1,
            "{fixture}\n{}",
            text.show()
        );
        for (index, hit) in hit_list.iter().enumerate() {
            let path = hit["path"].as_str().unwrap();
            let name = hit["id"].as_str().unwrap_or(path);
            let kind = hit["kind"].as_str().unwrap_or("-");
            let title = hit["title"].as_str().unwrap_or("-").replace('\n', " ");
            let archived = if hit["archived"] == true {
                " | archived"
            } else {
                ""
            };
            assert_eq!(
                lines[index * 2],
                format!(
                    "{name} | {kind} | {title} | {path}:{}{archived}",
                    hit["line"]
                ),
                "{fixture}: hit {index}"
            );
            let snippet = hit["snippet"].as_str().unwrap().replace(['\n', '\r'], " ");
            assert_eq!(
                lines[index * 2 + 1],
                format!("    {snippet}"),
                "{fixture}: hit {index}"
            );
            // The line is the line of the node's span start in the file.
            let bytes = read(&root, path);
            let line = usize::try_from(hit["line"].as_u64().unwrap()).unwrap();
            assert!(line >= 1 && line <= bytes.iter().filter(|&&b| b == b'\n').count() + 1);
        }
        assert_eq!(
            lines.last().unwrap(),
            &format!(
                "hits {} (limit 20); archived matches left out: {} (--archive)",
                hit_list.len(),
                json["tier3_left_out"]
            )
        );
        // Two runs, byte for byte.
        let again = spec(&home, &root, &text_args);
        assert_eq!(again.stdout, text.stdout, "{fixture}: repeat");
        let again = spec(&home, &root, &args);
        assert_eq!(again.stdout, run.stdout, "{fixture}: repeat --json");
    }
}

#[test]
fn a_hit_line_is_its_node_line() {
    let scratch = Scratch::new("search-line");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let json = spec(
        &home,
        &root,
        &["--json", "search", "Walk", "--kind", "rule"],
    )
    .json();
    let hit = json["hits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|hit| hit["id"] == "RULE-MOVE-SPEEDS")
        .unwrap_or_else(|| panic!("RULE-MOVE-SPEEDS: {json}"));
    assert_eq!(hit["line"], 16);
    assert_eq!(hit["path"], "docs/spec/movement/README.md");
}

#[test]
fn kind_filters_are_repeatable_and_free() {
    let scratch = Scratch::new("search-kind");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    let kinds_of = |args: &[&str]| -> Vec<String> {
        let json = spec(&home, &root, args).json();
        json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|hit| hit["kind"].as_str().unwrap_or("-").to_owned())
            .collect()
    };
    let rules = kinds_of(&["--json", "search", "stamina", "--kind", "rule"]);
    assert!(
        !rules.is_empty() && rules.iter().all(|kind| kind == "rule"),
        "{rules:?}"
    );
    let two = kinds_of(&[
        "--json", "search", "stamina", "--kind", "rule", "--kind", "mechanic",
    ]);
    assert!(two.iter().any(|kind| kind == "mechanic") && two.iter().any(|kind| kind == "rule"));
    assert!(
        two.iter().all(|kind| kind == "rule" || kind == "mechanic"),
        "{two:?}"
    );
    // A kind no prefix declares is no error: zero hits.
    let run = spec(
        &home,
        &root,
        &["search", "stamina", "--kind", "no-such-kind"],
    );
    run.code(0);
    assert_eq!(
        run.stdout,
        "hits 0 (limit 20); archived matches left out: 0 (--archive)\n"
    );
    let json = spec(
        &home,
        &root,
        &[
            "--json", "search", "stamina", "--kind", "rule", "--kind", "x",
        ],
    )
    .json();
    assert_eq!(json["kinds"], serde_json::json!(["rule", "x"]));
}

#[test]
fn short_terms_are_dropped_with_a_note_and_none_left_is_exit_2() {
    let scratch = Scratch::new("search-short");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    index(&home, &root);
    for args in [
        &["search", "a", "of"][..],
        &["search", "R-"],
        &["--json", "search", "ab"],
    ] {
        let run = spec(&home, &root, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        let lines = run.stderr_lines();
        assert_eq!(lines.len(), 1, "{args:?}: {}", run.show());
        assert!(
            lines[0].starts_with("spec: ") && lines[0].contains("spec show"),
            "{args:?}: {}",
            run.show()
        );
    }
    let run = spec(&home, &root, &["search", "a", "stamina", "of"]);
    run.code(0);
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{}", run.show());
    assert!(
        lines[0].starts_with("note: ") && lines[0].contains("`a`") && lines[0].contains("`of`")
    );
    let plain = spec(&home, &root, &["search", "stamina"]);
    assert_eq!(run.stdout, plain.stdout, "dropped terms change nothing");
    let json = spec(&home, &root, &["--json", "search", "a", "stamina"]).json();
    assert_eq!(json["notes"].as_array().unwrap().len(), 1, "{json}");
}

#[test]
fn no_hits_is_exit_0() {
    for (fixture, _) in FIXTURES {
        let scratch = Scratch::new("search-none");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let run = spec(&home, &root, &["search", "zzqqxxwwvv"]);
        run.code(0);
        assert_eq!(
            run.stdout,
            "hits 0 (limit 20); archived matches left out: 0 (--archive)\n"
        );
        assert_eq!(run.stderr, "");
        let json = spec(&home, &root, &["--json", "search", "zzqqxxwwvv"]).json();
        assert_eq!(json["hits"], serde_json::json!([]));
    }
}

/// spec-b: "Sinkhronizatsiya" is written capitalised in REQ-001's title;
/// the lower case finds it.
#[test]
fn a_capitalised_cyrillic_word_is_found_by_its_lower_case() {
    let scratch = Scratch::new("search-cyrillic");
    let home = scratch.home("h");
    let root = scratch.copy("spec-b", "copy");
    index(&home, &root);
    let lower = "\u{0441}\u{0438}\u{043d}\u{0445}\u{0440}\u{043e}\u{043d}\u{0438}\u{0437}\u{0430}\u{0446}\u{0438}\u{044f}";
    let capital = "\u{0421}\u{0438}\u{043d}\u{0445}\u{0440}\u{043e}\u{043d}\u{0438}\u{0437}\u{0430}\u{0446}\u{0438}\u{044f}";
    assert!(
        String::from_utf8(read(&root, "docs/records/REQ/REQ-001.md"))
            .unwrap()
            .contains(capital)
    );
    let json = spec(&home, &root, &["--json", "search", lower]).json();
    let ids: Vec<&str> = json["hits"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|hit| hit["id"].as_str())
        .collect();
    assert!(ids.contains(&"REQ-001"), "{ids:?}");
}

/// `--limit` in 1..=200; the default is 20.
#[test]
fn limit_is_taken_as_given_within_bounds() {
    let scratch = Scratch::new("search-limit");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    for (limit, count) in [("1", 1), ("2", 2), ("200", 14)] {
        let json = spec(
            &home,
            &root,
            &["--json", "search", "stamina", "--limit", limit],
        )
        .json();
        assert_eq!(json["limit"], limit.parse::<u64>().unwrap());
        assert_eq!(
            json["hits"].as_array().unwrap().len(),
            count,
            "--limit {limit}"
        );
    }
}

/// `--json`: exactly the keys of "Data" plus `truncated` (iteration 3),
/// every key present, absent = `null` (a document hit has no ID and no
/// kind).
#[test]
fn search_json_has_exactly_the_data_keys() {
    let scratch = Scratch::new("search-keys");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let json = spec(&home, &root, &["--json", "search", "tuning"]).json();
    let keys = |value: &serde_json::Value| -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(
        keys(&json),
        [
            "archive",
            "hits",
            "kinds",
            "limit",
            "notes",
            "query",
            "tier3_left_out",
            "truncated"
        ]
    );
    // Nothing cut: `truncated` is false, no note.
    assert_eq!(json["truncated"], false, "{json}");
    assert_eq!(json["notes"], serde_json::json!([]));
    let hits = json["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "{json}");
    for hit in hits {
        assert_eq!(
            keys(hit),
            [
                "archived", "id", "kind", "line", "ord", "path", "snippet", "title"
            ],
            "{hit}"
        );
    }
    let document = hits
        .iter()
        .find(|hit| hit["path"] == "docs/features/stamina-tuning.md" && hit["ord"] == 0)
        .unwrap_or_else(|| panic!("the feature document: {json}"));
    assert!(
        document["id"].is_null() && document["kind"].is_null(),
        "{document}"
    );
    assert_eq!(document["title"], "Stamina tuning");
}

/// Iteration 2, item (5): a front-matter `kind:` (and title) holding a line
/// break prints on one line in the hit's header; JSON keeps the value.
#[test]
fn a_multi_line_kind_prints_on_one_line() {
    let scratch = Scratch::new("search-kind-line");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    common::write(
        &root,
        "docs/spec/weird.md",
        "---\nclass: canon\nkind: \"odd\\nkind\"\ntitle: \"Two\\r\\nlines\"\n---\n\n# Weird zephyrine\n\nBody zephyrine.\n",
    );
    let run = spec(&home, &root, &["search", "zephyrine"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines.len(),
        3,
        "one hit, two lines, the summary\n{}",
        run.show()
    );
    assert_eq!(
        lines[0],
        "docs/spec/weird.md | odd kind | Two lines | docs/spec/weird.md:1"
    );
    assert!(lines[1].starts_with("    "));
    let json = spec(&home, &root, &["--json", "search", "zephyrine"]).json();
    assert_eq!(json["hits"][0]["kind"], "odd\nkind");
    assert_eq!(json["hits"][0]["title"], "Two\r\nlines");
    // `show` flattens the same fields in its header.
    let shown = spec(&home, &root, &["show", "docs/spec/weird.md"]);
    shown.code(0);
    assert!(
        shown
            .stdout
            .starts_with("docs/spec/weird.md | odd kind | Two lines | docs/spec/weird.md:1 | "),
        "{}",
        shown.show()
    );
}
