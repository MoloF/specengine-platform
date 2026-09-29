//! AC-15 of docs/features/spec-index.md (ADR-0012: nothing is fatal): a
//! non-UTF-8 file, an unclosed front-matter, a YAML error and a mode-000
//! file beside spec-a — the update is `Ok`, every other node is present,
//! and each broken file keeps its row: BLAKE3 and diagnostics as parsed, or
//! a NULL BLAKE3 with a `read_error` when unreadable (re-read by every
//! update, indexed once readable again).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;

use common::{Corpus, Scratch, assert_equals_fresh, blake3_hex, chmod, stored_nodes};
use specengine_store::{IndexWriter, IndexedFile, SpecIndex};

const NOT_UTF8: &str = "docs/spec/broken-bytes.md";
const UNCLOSED: &str = "docs/records/R/R-90.md";
const YAML_ERROR: &str = "docs/records/R/R-91.md";
const LOCKED: &str = "docs/spec/locked.md";

#[test]
fn broken_files_keep_their_rows_and_never_fail_an_update() {
    let scratch = Scratch::new("broken");
    let pristine = Corpus::copy_of("spec-a", &scratch, "pristine");
    let mut reference = pristine.open(&scratch.db("pristine"));
    pristine.update(&mut reference);
    let pristine_nodes: BTreeSet<_> = stored_nodes(&reference).into_iter().collect();

    let corpus = Corpus::copy_of("spec-a", &scratch, "wt");
    corpus.write(
        NOT_UTF8,
        b"# Broken bytes {#RULE-BYTES}\n\nLatin-1 \xe9t\xe9 and a stray \xff byte.\n".as_slice(),
    );
    corpus.write(
        UNCLOSED,
        "---\nid: R-90\nkind: requirement\n\n# The block never closes\n\nText.\n",
    );
    corpus.write(
        YAML_ERROR,
        "---\nid: R-91\nlinks: [R-12, unclosed\n---\n\n# Bad YAML\n\nText.\n",
    );
    corpus.write(LOCKED, "# Locked\n\nNobody may read this.\n");
    chmod(&corpus.root, LOCKED, 0o000);
    let locked_readable = std::fs::read(corpus.root.join(LOCKED)).is_ok();

    let mut index = corpus.open(&scratch.db("index"));
    let report = index
        .update(&corpus.tree(), &corpus.scheme)
        .expect("broken files never fail an update");
    assert_eq!(report.walked, corpus.listing().paths.len(), "{report:?}");

    // Every node of the untouched files is present.
    let stored: BTreeSet<_> = stored_nodes(&index).into_iter().collect();
    let missing: Vec<_> = pristine_nodes.difference(&stored).collect();
    assert!(
        missing.is_empty(),
        "nodes lost beside the broken files: {missing:?}"
    );

    // The parseable broken files: row, BLAKE3, diagnostics as parsed.
    for (path, code) in [
        (NOT_UTF8, "not-utf8"),
        (UNCLOSED, "frontmatter-unclosed"),
        (YAML_ERROR, "frontmatter-yaml"),
    ] {
        let bytes = corpus.bytes(path);
        let parsed = specengine_core::parse(path, &bytes, &corpus.scheme);
        assert!(
            parsed.diagnostics.iter().any(|d| d.code.as_str() == code),
            "{path}: the parser reports {code}: {:?}",
            parsed.diagnostics
        );
        let file = index
            .file(path)
            .expect("file")
            .expect("the broken file has a row");
        assert_eq!(
            file,
            IndexedFile {
                parsed: Some(parsed),
                blake3: Some(blake3_hex(&bytes)),
                size: bytes.len() as u64,
                read_error: None,
            },
            "{path}"
        );
    }

    // The unreadable file: a row with no hash and the read error.
    if locked_readable {
        eprintln!(
            "running with read access to mode-000 files (root?): the unreadable case is skipped"
        );
    } else {
        assert_eq!(report.unreadable, 1, "{report:?}");
        let file = index
            .file(LOCKED)
            .expect("file")
            .expect("the unreadable file has a row");
        assert_eq!(file.blake3, None, "{LOCKED}: no hash");
        assert!(
            file.read_error.is_some(),
            "{LOCKED}: the read error is stored"
        );
        assert!(index.files().expect("files").contains(&LOCKED.to_owned()));
        // Re-read by every update, and never taken for unchanged.
        let again = corpus.update(&mut index);
        assert_eq!(again.unreadable, 1, "re-read: {again:?}");
        assert_eq!(again.parsed, 0, "{again:?}");
        assert_equals_fresh(&index, &corpus, &scratch, "an unreadable file");
        // Readable again: indexed.
        chmod(&corpus.root, LOCKED, 0o644);
        let readable = corpus.update(&mut index);
        assert_eq!(readable.parsed, 1, "{readable:?}");
        assert_eq!(readable.unreadable, 0, "{readable:?}");
        let file = index.file(LOCKED).expect("file").expect("row");
        assert_eq!(file.blake3, Some(blake3_hex(&corpus.bytes(LOCKED))));
        assert_eq!(file.read_error, None);
    }
    assert_equals_fresh(&index, &corpus, &scratch, "broken files");
    index.check_fts().expect("FTS5 integrity-check");
}
