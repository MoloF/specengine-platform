//! docs/features/norm-hash.md TE-6 ("Index", WA-10): every node row of a
//! fresh index of spec-a, spec-b and spec-c carries `norm_hash` =
//! `b3_hash(norm_input(..))` of a fresh parse of the file's bytes (NULL
//! where null: a document whose front-matter may have lost a link), the
//! value read back through the canonical dump (the store has no read API
//! for it yet: slice 3 reads it); the five golden hashes of TE-1 are what
//! the index stores; an edit updates the column incrementally and the index
//! still equals a fresh one (`assert_equals_fresh`). The format pin and the
//! 7-stamped database live in `format.rs` (the one test naming `rusqlite`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeMap;

use common::{Corpus, Scratch, assert_equals_fresh, blake3_hex};
use serde_json::Value;
use specengine_store::{SqliteIndex, b3_hash};

/// `(path, ord)` -> `(id, norm_hash)`.
type Rows = BTreeMap<(String, usize), (Option<String>, Option<String>)>;

/// `(path, ord)` -> `(id, norm_hash)` of every `nodes` row of the dump:
/// `nodes\t[project, root, path, ord, line, id, kind, title, parent_id,
/// own_text, norm_hash, node]`.
fn stored(index: &SqliteIndex) -> Rows {
    let dump = index.dump().expect("dump");
    let mut rows = BTreeMap::new();
    for line in dump.lines() {
        let Some(json) = line.strip_prefix("nodes\t") else {
            continue;
        };
        let row: Vec<Value> = serde_json::from_str(json).expect("a JSON row");
        assert_eq!(row.len(), 12, "the nodes columns: {line}");
        assert!(row[9].is_string(), "own_text before norm_hash: {line}");
        let norm = match &row[10] {
            Value::Null => None,
            Value::String(hash) => Some(hash.clone()),
            other => panic!("norm_hash is TEXT or NULL, not {other}"),
        };
        rows.insert(
            (
                row[2].as_str().expect("path").to_owned(),
                usize::try_from(row[3].as_u64().expect("ord")).unwrap(),
            ),
            (row[5].as_str().map(str::to_owned), norm),
        );
    }
    assert!(!rows.is_empty(), "the dump holds node rows");
    rows
}

/// `(path, ord)` -> `(id, norm_hash)` from core over the corpus's files as
/// they are now: a fresh parse of each walked file's bytes; the hash by the
/// tests' own `blake3` crate call.
fn from_core(corpus: &Corpus) -> Rows {
    let mut want = BTreeMap::new();
    for path in corpus.listing().paths {
        let bytes = corpus.bytes(&path);
        let parsed = specengine_core::parse(&path, &bytes, &corpus.scheme);
        for (ord, node) in parsed.nodes.iter().enumerate() {
            let norm = specengine_core::norm_input(&parsed, &bytes, ord)
                .map(|input| format!("b3:{}", blake3_hex(&input)));
            want.insert((path.clone(), ord), (node.id.clone(), norm));
        }
    }
    want
}

fn nulls(rows: &Rows) -> Vec<String> {
    rows.iter()
        .filter(|(_, (_, norm))| norm.is_none())
        .map(|((path, ord), _)| format!("{path}#{ord}"))
        .collect()
}

/// TE-6: every node row of spec-a, spec-b and spec-c equals core's hash of
/// a fresh parse, NULL exactly where core gives null.
#[test]
fn every_node_row_is_cores_hash_of_a_fresh_parse() {
    for (name, want_nulls) in [
        ("spec-a", vec!["docs/features/stamina-tuning.md#0"]),
        ("spec-b", vec!["docs/records/QN/QN-08.md#0"]),
        ("spec-c", vec![]),
    ] {
        let scratch = Scratch::new("norm-rows");
        let corpus = Corpus::copy_of(name, &scratch, "wt");
        let index = corpus.fresh(&scratch);
        let rows = stored(&index);
        let want = from_core(&corpus);
        assert_eq!(rows, want, "{name}: the stored column is core's hash");
        assert_eq!(nulls(&rows), want_nulls, "{name}: the NULL rows");
        for (key, (_, norm)) in &rows {
            if let Some(norm) = norm {
                assert!(
                    norm.len() == 67
                        && norm.starts_with("b3:")
                        && norm[3..]
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                    "{name} {key:?}: {norm}"
                );
            }
        }
        // `b3_hash` is the same function: `b3:` and the lower-case hex.
        assert_eq!(b3_hash(b"x"), format!("b3:{}", blake3_hex(b"x")));
    }
}

/// TE-1's golden hashes are what the index stores for the five nodes.
#[test]
fn the_golden_hashes_are_stored() {
    let golden = [
        (
            "spec-a",
            "MEC-STAMINA",
            "b3:d28068144ffb79d558aa83cb5da6756bf2e5284b685d7df96671f2091218ce06",
        ),
        (
            "spec-a",
            "RULE-STAM-REGEN",
            "b3:613edf30c8b48316017d34ed06c3f83fe2a7ba27c294f09afe48467771e7c014",
        ),
        (
            "spec-a",
            "RULE-SPRINT-COST",
            "b3:bb97b1d5b795143baa4c6e9b3cfc3d7f7c798239339680cf8ed0f3af1623ca4d",
        ),
        (
            "spec-a",
            "MEC-SPRINT",
            "b3:1a1fd1318af16cc454a8630747054d51d8a72ba11b33873c0a02f2896f5cdcae",
        ),
        (
            "spec-b",
            "FLAG-DRY-RUN",
            "b3:f7564300dc534d7507947e7935b594a9a24ce15120da98dede9239d0e9f503eb",
        ),
    ];
    let scratch = Scratch::new("norm-golden");
    for name in ["spec-a", "spec-b"] {
        let corpus = Corpus::copy_of(name, &scratch, name);
        let rows = stored(&corpus.fresh(&scratch));
        for (_, id, hash) in golden.iter().filter(|(corpus, _, _)| *corpus == name) {
            let found: Vec<_> = rows
                .values()
                .filter(|(row_id, _)| row_id.as_deref() == Some(*id))
                .map(|(_, norm)| norm.clone())
                .collect();
            assert_eq!(found, [Some((*hash).to_owned())], "{name} {id}");
        }
    }
}

/// TE-6: an edit of meaning updates the row incrementally, a reflow of
/// whitespace leaves it, and the index equals a fresh one after each step;
/// a file made null and then sound again follows.
#[test]
fn edits_update_the_column_and_equal_a_fresh_index() {
    let scratch = Scratch::new("norm-edit");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let db = scratch.db("index");
    let mut index = corpus.open(&db);
    corpus.update(&mut index);
    let sprint = "docs/spec/movement/sprint.md";
    let hash_of = |index: &SqliteIndex, id: &str| -> Option<String> {
        stored(index)
            .into_values()
            .find(|(row_id, _)| row_id.as_deref() == Some(id))
            .unwrap_or_else(|| panic!("{id} stored"))
            .1
    };
    let cost = hash_of(&index, "RULE-SPRINT-COST");
    let sprint_doc = hash_of(&index, "MEC-SPRINT");

    // Whitespace only: trailing spaces and blank lines.
    corpus.replace(sprint, "bounded by A-101.\n", "bounded by A-101.   \n\n\n");
    corpus.update(&mut index);
    assert_eq!(hash_of(&index, "RULE-SPRINT-COST"), cost, "whitespace");
    assert_equals_fresh(&index, &corpus, &scratch, "a whitespace edit");

    // A word: the section changes, its document does not.
    corpus.replace(sprint, "costs 12 units/s", "costs 14 units/s");
    corpus.update(&mut index);
    assert_ne!(hash_of(&index, "RULE-SPRINT-COST"), cost, "a word");
    assert_eq!(hash_of(&index, "MEC-SPRINT"), sprint_doc, "the parent");
    assert_equals_fresh(&index, &corpus, &scratch, "a word");

    // A broken front-matter nulls the document, its sections keep theirs.
    let after_word = hash_of(&index, "RULE-SPRINT-COST");
    corpus.replace(sprint, "tier: 2\n", "tier: high\n");
    corpus.update(&mut index);
    let rows = stored(&index);
    assert_eq!(
        rows.get(&(sprint.to_owned(), 0))
            .map(|(_, norm)| norm.clone()),
        Some(None),
        "frontmatter-type nulls the document"
    );
    assert_eq!(hash_of(&index, "RULE-SPRINT-COST"), after_word);
    assert_equals_fresh(&index, &corpus, &scratch, "a broken front-matter");
    corpus.replace(sprint, "tier: high\n", "tier: 2\n");
    corpus.update(&mut index);
    assert_eq!(hash_of(&index, "MEC-SPRINT"), sprint_doc, "sound again");
    assert_equals_fresh(&index, &corpus, &scratch, "the front-matter mended");

    // A file that is not UTF-8 has no node rows at all.
    corpus.write("docs/spec/broken.md", b"---\nid: MEC-BROKEN\n---\n\xff\n");
    corpus.update(&mut index);
    assert!(
        !stored(&index)
            .keys()
            .any(|(path, _)| path == "docs/spec/broken.md"),
        "no rows for a file that is not UTF-8"
    );
    assert_equals_fresh(&index, &corpus, &scratch, "a file that is not UTF-8");
}
