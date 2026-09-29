//! AC-05 and AC-06 of docs/features/spec-index.md: the index is a projection
//! of `specengine_core::parse` — `file(path)` gives back exactly the parsed
//! file, its BLAKE3 and size, for every file of spec-a and spec-b and for an
//! ID-less document with a top-level section and a mention outside ID
//! sections — and nothing is resolved or made unique: an ID defined in two
//! files and twice in a third is stored four times and `lookup_id` returns
//! all four by `(path, ord)`.

#![cfg(unix)]

mod common;

use common::{Corpus, Scratch, blake3_hex, md_files_under};
use specengine_store::{IndexWriter, IndexedFile, SpecIndex};

/// `file(path)` of every walked file equals the parser's output on the same
/// bytes, with the BLAKE3 and size of the bytes; `lookup_id` of every node's
/// ID contains that node at its `(path, ord)`.
fn assert_projection(corpus: &Corpus, index: &impl SpecIndex, context: &str) -> usize {
    let listing = corpus.listing();
    assert_eq!(
        index.files().expect("files"),
        listing.paths,
        "{context}: files() is the walk"
    );
    let mut nodes = 0;
    for path in &listing.paths {
        let bytes = corpus.bytes(path);
        let parsed = specengine_core::parse(path, &bytes, &corpus.scheme);
        let expected = IndexedFile {
            parsed: Some(parsed.clone()),
            blake3: Some(blake3_hex(&bytes)),
            size: bytes.len() as u64,
            read_error: None,
        };
        let stored = index
            .file(path)
            .expect("file")
            .unwrap_or_else(|| panic!("{context}: {path} is not stored"));
        assert!(
            stored == expected,
            "{context}: {path}: the stored file differs from parse()\nstored:   {stored:?}\nexpected: {expected:?}"
        );
        for (ord, node) in parsed.nodes.iter().enumerate() {
            nodes += 1;
            let Some(id) = &node.id else { continue };
            let hits = index.lookup_id(id).expect("lookup_id");
            assert!(
                hits.iter()
                    .any(|hit| hit.path == *path && hit.ord == ord && hit.node == *node),
                "{context}: lookup_id({id}) lacks ({path}, {ord}): {:?}",
                hits.iter()
                    .map(|hit| (&hit.path, hit.ord))
                    .collect::<Vec<_>>()
            );
            assert!(
                hits.iter()
                    .all(|hit| hit.node.id.as_deref() == Some(id.as_str())),
                "{context}: lookup_id({id}) returned another ID"
            );
        }
    }
    assert!(index.file("docs/no-such-file.md").expect("file").is_none());
    nodes
}

#[test]
fn every_file_of_spec_a_and_spec_b_reads_back_as_parsed() {
    let scratch = Scratch::new("projection");
    for (name, walked_dirs) in [
        (
            "spec-a",
            &["docs/spec", "docs/records", "docs/features"][..],
        ),
        ("spec-b", &["docs"][..]),
    ] {
        let corpus = Corpus::copy_of(name, &scratch, name);
        let mut index = corpus.open(&scratch.db(name));
        let report = corpus.update(&mut index);
        let expected: Vec<String> = walked_dirs
            .iter()
            .flat_map(|dir| md_files_under(&corpus.root, dir))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(corpus.listing().paths, expected, "{name}: walked files");
        assert_eq!(report.walked, expected.len(), "{name}: walked");
        assert_eq!(report.parsed, expected.len(), "{name}: parsed");
        let nodes = assert_projection(&corpus, &index, name);
        assert!(nodes > expected.len(), "{name}: sections are stored too");
    }
}

#[test]
fn an_id_less_document_with_a_top_level_section_and_a_loose_mention_is_stored_as_parsed() {
    let scratch = Scratch::new("projection-idless");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let path = "docs/spec/field-notes.md";
    corpus.write(
        path,
        "# Field notes\n\n\
         The lantern flickers near RULE-CORE-LOOP, as MEC-STAMINA warns.\n\n\
         ## Night watch {#RULE-NIGHT-WATCH .watch rev=2}\n\n\
         Guards rotate every hour.\n",
    );
    // The parser's view first: the case really has an ID-less document, a
    // section whose parent is the (ID-less) document, and links without `src`.
    let parsed = specengine_core::parse(path, &corpus.bytes(path), &corpus.scheme);
    assert!(parsed.nodes[0].id.is_none(), "the document has no ID");
    let section = parsed
        .nodes
        .iter()
        .find(|node| node.id.as_deref() == Some("RULE-NIGHT-WATCH"))
        .expect("the top-level section is a node");
    assert!(
        section.parent.is_none(),
        "a top-level section of an ID-less document has no parent ID"
    );
    // Heading attributes too (no fixture heading carries a class).
    assert_eq!(section.classes, ["watch"]);
    assert_eq!(section.rev, Some(2));
    assert!(
        parsed.links.iter().any(|link| link.src.is_none()),
        "a mention outside ID sections has no src: {:?}",
        parsed.links
    );

    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    assert_projection(&corpus, &index, "spec-a + ID-less document");
    let hits = index.lookup_id("RULE-NIGHT-WATCH").expect("lookup_id");
    assert_eq!(
        hits.iter()
            .map(|hit| (hit.path.as_str(), hit.ord))
            .collect::<Vec<_>>(),
        [(path, 1)]
    );
}

#[test]
fn an_id_defined_in_two_files_and_twice_in_a_third_is_stored_four_times() {
    let scratch = Scratch::new("projection-duplicates");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write(
        "docs/spec/twin-one.md",
        "# One\n\n## Twin rule {#RULE-TWIN}\n\nFirst.\n\n## Near miss {#RULE-TWINSET}\n\nNot it.\n",
    );
    corpus.write(
        "docs/spec/twin-two.md",
        "# Two\n\n## Twin rule {#RULE-TWIN}\n\nSecond.\n",
    );
    corpus.write(
        "docs/spec/twin-three.md",
        "# Three\n\n## Twin rule {#RULE-TWIN}\n\nThird.\n\n## Twin again {#RULE-TWIN}\n\nFourth.\n",
    );
    // A feature-scoped ID (spec-a's `AC`) defined in two feature specs.
    corpus.write(
        "docs/features/lantern-oil.md",
        "# Lantern oil\n\n## Criterion {#AC-01}\n\nOil lasts.\n",
    );
    corpus.write(
        "docs/features/night-watch.md",
        "# Night watch\n\n## Criterion {#AC-01}\n\nGuards rotate.\n",
    );

    let mut index = corpus.open(&scratch.db("index"));
    let report = index
        .update(&corpus.tree(), &corpus.scheme)
        .expect("duplicate IDs never fail an update");
    assert_eq!(report.walked, corpus.listing().paths.len());

    let hits = index.lookup_id("RULE-TWIN").expect("lookup_id");
    let keys: Vec<(&str, usize)> = hits
        .iter()
        .map(|hit| (hit.path.as_str(), hit.ord))
        .collect();
    assert_eq!(
        keys,
        [
            ("docs/spec/twin-one.md", 1),
            ("docs/spec/twin-three.md", 1),
            ("docs/spec/twin-three.md", 2),
            ("docs/spec/twin-two.md", 1),
        ],
        "all four definitions, by (path, ord); RULE-TWINSET is not an exact match"
    );
    for hit in &hits {
        let parsed = specengine_core::parse(&hit.path, &corpus.bytes(&hit.path), &corpus.scheme);
        assert_eq!(hit.node, parsed.nodes[hit.ord], "{}#{}", hit.path, hit.ord);
    }

    let scoped = index.lookup_id("AC-01").expect("lookup_id");
    assert_eq!(
        scoped
            .iter()
            .map(|hit| (hit.path.as_str(), hit.ord))
            .collect::<Vec<_>>(),
        [
            ("docs/features/lantern-oil.md", 1),
            ("docs/features/night-watch.md", 1)
        ],
        "a feature-scoped ID in two features"
    );
    assert!(
        index
            .lookup_id("RULE-NO-SUCH")
            .expect("lookup_id")
            .is_empty()
    );
    assert_projection(&corpus, &index, "spec-a + duplicates");
}

/// AC-04 of docs/features/phase1-cleanup.md (P3), with the other parser
/// changes of that spec (M1, P4): a file with non-finite floats in any
/// spelling, a float that needs `float_roundtrip`, repeated and collection
/// map keys, and an alias with look-alike digits reads back from the index
/// exactly as `parse()` gives it.
#[test]
fn non_finite_floats_repeated_keys_and_aliases_read_back_as_parsed() {
    let scratch = Scratch::new("projection-cleanup");
    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    let path = "docs/spec/odd-values.md";
    corpus.write(
        path,
        "---\nclass: spec\nx_nan: .nan\nx_inf: .Inf\nx_neg: -.inf\nx_big: 1e999\n\
         x_fine: 0.30000000000000004\nx_seq: [.nan, 2.5e-308, -.inf]\n\
         x_map: {1: a, \"1\": b, k: {? [q] : r, s: .nan}}\nraised_by: {1: a, \"1\": b}\n\
         1: top\n\"1\": again\n---\n\
         # Odd values\n\nSee QST-\u{FF10}\u{FF13}\u{FF11} and Q-\u{FF10}31.\n",
    );
    let parsed = specengine_core::parse(path, &corpus.bytes(path), &corpus.scheme);
    let extra = parsed.nodes[0].extra.as_ref().expect("extra");
    assert!(
        extra.iter().any(|entry| entry.key == "x_nan"
            && entry.value == specengine_model::FmValue::Str(".nan".to_owned())),
        "{extra:?}"
    );
    let mut index = corpus.open(&scratch.db("index"));
    corpus.update(&mut index);
    assert_projection(&corpus, &index, "spec-a + odd values");
}
