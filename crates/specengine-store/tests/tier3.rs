//! AC-19 of docs/features/spec-cli.md (store): `files.tier3` (core's
//! `is_tier3_file`) and `nodes.line` (the 1-based line of the span start)
//! are pure functions of the bytes; `SearchQuery::archive` false drops Tier
//! 3 nodes in the query, before `LIMIT`, and `tier3_left_out` counts every
//! dropped match; `indexed_input` equals the walk-fed `check_input` in
//! paths, sizes, parses and read errors; an incremental update equals a
//! rebuild with the new columns.

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;

use common::{Corpus, Scratch, assert_equals_fresh, chmod};
use specengine_core::check::is_tier3_file;
use specengine_store::{IndexWriter, SearchQuery, SpecIndex, SqliteIndex, check_input};

/// `path → tier3` and `(path, ord) → line`, read from the worktree's
/// canonical dump (the public view of the rows: `files` ends with `tier3`,
/// `nodes` has `line` after `ord`; crates/specengine-store/README.md).
fn columns(index: &SqliteIndex) -> (BTreeMap<String, i64>, BTreeMap<(String, i64), i64>) {
    let dump = index.dump_worktree().expect("dump");
    let mut tier3 = BTreeMap::new();
    let mut lines = BTreeMap::new();
    for line in dump.lines() {
        let Some((table, row)) = line.split_once('\t') else {
            continue;
        };
        let row: serde_json::Value = serde_json::from_str(row).expect("a JSON row");
        let row = row.as_array().expect("an array row");
        match table {
            "files" => {
                assert_eq!(
                    row.len(),
                    8,
                    "files: project, root, path, blake3, size, read_error, shell, tier3"
                );
                tier3.insert(
                    row[2].as_str().unwrap().to_owned(),
                    row[7].as_i64().unwrap(),
                );
            }
            "nodes" => {
                assert_eq!(row.len(), 11, "nodes: project, root, path, ord, line, …");
                lines.insert(
                    (
                        row[2].as_str().unwrap().to_owned(),
                        row[3].as_i64().unwrap(),
                    ),
                    row[4].as_i64().unwrap(),
                );
            }
            _ => {}
        }
    }
    assert!(
        !tier3.is_empty() && !lines.is_empty(),
        "the dump lists files and nodes"
    );
    (tier3, lines)
}

fn line_of(bytes: &[u8], offset: usize) -> i64 {
    i64::try_from(bytes[..offset].iter().filter(|&&b| b == b'\n').count() + 1).unwrap()
}

#[test]
fn tier3_and_line_are_functions_of_the_bytes() {
    for (name, archived) in [
        ("spec-a", "docs/records/DEC/DEC-0007.md"),
        ("spec-b", "docs/records/ADR/ADR-0002.md"),
    ] {
        let scratch = Scratch::new(&format!("tier3-{name}"));
        let corpus = Corpus::copy_of(name, &scratch, "wt");
        // A broken front-matter and a non-UTF-8 file: tier3 0.
        corpus.write(
            "docs/records/broken-decision.md",
            "---\nclass: decision\nstatus: rejected\n\n# never closed\n",
        );
        corpus.write("docs/records/raw.md", b"# Raw \xff\n".as_slice());
        let mut index = corpus.open(&scratch.db("index"));
        corpus.update(&mut index);
        let (tier3, lines) = columns(&index);
        let mut tier3_paths = Vec::new();
        for path in index.files().unwrap() {
            let bytes = corpus.bytes(&path);
            let parsed = specengine_core::parse(&path, &bytes, &corpus.scheme);
            assert_eq!(
                tier3[&path],
                i64::from(is_tier3_file(&parsed)),
                "{name}: {path}"
            );
            if tier3[&path] == 1 {
                tier3_paths.push(path.clone());
            }
            for (ord, node) in parsed.nodes.iter().enumerate() {
                let key = (path.clone(), i64::try_from(ord).unwrap());
                assert_eq!(
                    lines[&key],
                    line_of(&bytes, node.span.start),
                    "{name}: {path}#{ord}"
                );
            }
        }
        assert_eq!(tier3_paths, [archived], "{name}");
        assert_eq!(tier3["docs/records/broken-decision.md"], 0);
        assert_eq!(tier3["docs/records/raw.md"], 0);
        if name == "spec-a" {
            let ord = specengine_core::parse(
                "docs/spec/movement/stamina.md",
                &corpus.bytes("docs/spec/movement/stamina.md"),
                &corpus.scheme,
            )
            .nodes
            .iter()
            .position(|node| node.id.as_deref() == Some("RULE-STAM-REGEN"))
            .unwrap();
            assert_eq!(
                lines[&(
                    "docs/spec/movement/stamina.md".to_owned(),
                    i64::try_from(ord).unwrap()
                )],
                21
            );
        }
        // The search hit carries both.
        let mut checked = 0;
        let mut archived_hits = 0;
        for word in [
            "regener",
            "sync",
            "stamina",
            "replaced",
            "\u{043a}\u{043b}\u{043e}\u{043d}",
        ] {
            let mut query = SearchQuery::new(word);
            query.archive = true;
            query.limit = 200;
            for hit in index.search(&query).unwrap().hits {
                assert_eq!(hit.tier3, tier3[&hit.path] == 1, "{name}: {}", hit.path);
                assert_eq!(
                    i64::try_from(hit.line).unwrap(),
                    lines[&(hit.path.clone(), i64::try_from(hit.ord).unwrap())],
                    "{name}: {}",
                    hit.path
                );
                checked += 1;
                archived_hits += usize::from(hit.tier3);
            }
        }
        assert!(
            checked > 3 && archived_hits >= 1,
            "{name}: {checked} hits, {archived_hits} archived"
        );
    }
}

/// Three Tier 3 nodes outrank three live ones: the default search with
/// limit 3 returns the live three and counts the three dropped.
#[test]
fn the_archive_filter_runs_before_the_limit_and_counts_every_drop() {
    let scratch = Scratch::new("tier3-limit");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let term = "obsidianwick";
    let filler = "Unrelated filler words to dilute the match in a long body. ".repeat(30);
    for n in 1..=3 {
        corpus.write(
            &format!("docs/records/DEC/DEC-01{n}0.md"),
            format!("---\nid: DEC-01{n}0\nclass: decision\nstatus: rejected\n---\n\n# {term} {term} {n}\n\n{term} {term}.\n"),
        );
        corpus.write(
            &format!("docs/records/R/R-7{n}.md"),
            format!("---\nid: R-7{n}\nclass: canon\n---\n\n# Live {n}\n\n{filler}\n{term}.\n\n{filler}\n"),
        );
    }
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);

    let mut query = SearchQuery::new(term);
    query.limit = 3;
    query.archive = true;
    let all = index.search(&query).unwrap();
    assert!(
        all.hits.iter().all(|hit| hit.tier3),
        "archived rank first: {all:?}"
    );
    assert_eq!(all.tier3_left_out, 0);

    query.archive = false;
    let live = index.search(&query).unwrap();
    let mut paths: Vec<&str> = live.hits.iter().map(|hit| hit.path.as_str()).collect();
    paths.sort_unstable();
    assert_eq!(
        paths,
        [
            "docs/records/R/R-71.md",
            "docs/records/R/R-72.md",
            "docs/records/R/R-73.md"
        ]
    );
    assert!(live.hits.iter().all(|hit| !hit.tier3));
    assert_eq!(
        live.tier3_left_out, 3,
        "every dropped match, not only those within the limit"
    );

    // limit 1: still one live hit, still 3 counted.
    query.limit = 1;
    let one = index.search(&query).unwrap();
    assert_eq!(one.hits.len(), 1);
    assert!(!one.hits[0].tier3);
    assert_eq!(one.tier3_left_out, 3);
    // Kinds filter the count too.
    query.kinds = vec!["requirement".into()];
    assert_eq!(index.search(&query).unwrap().tier3_left_out, 0);
    query.kinds = vec!["decision".into()];
    let decisions = index.search(&query).unwrap();
    assert!(decisions.hits.is_empty());
    assert_eq!(decisions.tier3_left_out, 3);
    // A short query searches nothing and drops nothing.
    let short = index.search(&SearchQuery::new("ab")).unwrap();
    assert!(short.short_query && short.tier3_left_out == 0);
    // The default query leaves Tier 3 out.
    assert!(!SearchQuery::new("x").archive);
}

/// `indexed_input` = the walk-fed `check_input` in paths (order included),
/// sizes, parses and read errors; `bytes` empty and no `problems`.
#[test]
fn indexed_input_equals_the_walk_fed_check_input() {
    for name in ["spec-a", "spec-b"] {
        let scratch = Scratch::new(&format!("tier3-input-{name}"));
        let corpus = Corpus::copy_of(name, &scratch, "wt");
        corpus.write(
            "docs/records/zz-unclosed.md",
            "---\nclass: canon\n\n# never closed\n",
        );
        corpus.write("docs/records/raw.md", b"# Raw \xff bytes\n".as_slice());
        corpus.write("docs/records/locked.md", "# Locked\n");
        chmod(&corpus.root, "docs/records/locked.md", 0o000);
        let mut index = corpus.open(&scratch.db("index"));
        corpus.update(&mut index);
        let stored = index.indexed_input().unwrap();
        let walked = check_input(&corpus.tree(), &corpus.scheme);
        assert!(
            stored.problems.is_empty(),
            "{name}: the index keeps no walk problems"
        );
        let paths = |input: &specengine_core::check::CheckInput| -> Vec<String> {
            input.files.iter().map(|file| file.path.clone()).collect()
        };
        assert_eq!(paths(&stored), paths(&walked), "{name}: paths in order");
        for (s, w) in stored.files.iter().zip(&walked.files) {
            assert!(s.bytes.is_empty(), "{name}: {}: no bytes", s.path);
            assert_eq!(s.size, w.size, "{name}: {}: size", s.path);
            assert_eq!(s.parsed, w.parsed, "{name}: {}: parse", s.path);
            assert_eq!(
                s.read_error.is_some(),
                w.read_error.is_some(),
                "{name}: {}: read error",
                s.path
            );
        }
        let locked = stored
            .files
            .iter()
            .find(|file| file.path == "docs/records/locked.md")
            .unwrap();
        assert!(locked.read_error.is_some() && locked.parsed.is_none());
        assert!(
            stored
                .files
                .iter()
                .any(|file| file.path == "docs/records/raw.md" && file.parsed.is_some())
        );
        chmod(&corpus.root, "docs/records/locked.md", 0o644);
    }
}

/// An edit that flips a file into Tier 3 and shifts lines: the incremental
/// update equals a fresh index and a rebuild.
#[test]
fn incremental_equals_rebuild_with_the_new_columns() {
    let scratch = Scratch::new("tier3-incremental");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    // DEC-0023 accepted → rejected: Tier 3; lines shift in stamina.md.
    corpus.replace(
        "docs/records/DEC/DEC-0023.md",
        "status: accepted",
        "status: rejected",
    );
    corpus.replace(
        "docs/spec/movement/stamina.md",
        "# Stamina\n",
        "# Stamina\n\nOne.\n\nTwo.\n",
    );
    let report = corpus.update(&mut index);
    assert_eq!(report.parsed, 2);
    assert_equals_fresh(&index, &corpus, &scratch, "flip");
    let mut query = SearchQuery::new("Regeneration waits");
    query.limit = 200;
    let results = index.search(&query).unwrap();
    assert!(
        results
            .hits
            .iter()
            .all(|hit| hit.path != "docs/records/DEC/DEC-0023.md"),
        "{results:?}"
    );
    assert!(results.tier3_left_out >= 1);
    let dump = index.dump().unwrap();
    index.rebuild(&corpus.tree(), &corpus.scheme).unwrap();
    assert_eq!(index.dump().unwrap(), dump, "rebuild");
    // And back.
    corpus.replace(
        "docs/records/DEC/DEC-0023.md",
        "status: rejected",
        "status: accepted",
    );
    corpus.update(&mut index);
    assert_equals_fresh(&index, &corpus, &scratch, "back");
    let results = index.search(&query).unwrap();
    assert!(
        results
            .hits
            .iter()
            .any(|hit| hit.path == "docs/records/DEC/DEC-0023.md")
    );
}
