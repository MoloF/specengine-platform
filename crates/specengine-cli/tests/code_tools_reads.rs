//! docs/features/code-tools.md (6b-1), the developer's carry-overs on the
//! CLI and store side of the code reads (iteration 1, X4 and the
//! `unmapped` and `file_place` fixes; iteration 2, K1):
//!
//! - store `code_snapshot`: one read gives what each separate reader
//!   gives (`code_refs`; `lookup_symbols`, `file_symbols`, `code_symbols`;
//!   `Bound`: each marker-bound symbol once, by (path, ord)), and nothing
//!   without indexed code. M: `Bound` reading every symbol; a lookup's
//!   hits dropped.
//! - `spec unmapped`: a code file the walk does not read (not UTF-8) is
//!   refused as PATH (exit 2, stdout empty, the scope line), and so is a
//!   directory holding only such a file; a root that is such a file is a
//!   file row (`<file> | units 0 | mapped 0 | - | mentioned 0`, JSON `dir`
//!   false, `percent` null), never `<file>/`. M: the unread file read as a
//!   file of no unit; the root row a directory's.
//! - `file_place` (and `dir_text`): on Unix a package directory whose name
//!   holds `\` keeps it in the qpath and in the row, `/` never put in its
//!   place. M: `dir_text` rewriting `\` whatever the separator.
//!
//! Over scratch copies of `fixtures/code-index` (6a-2's committed
//! corpus), a scratch `HOME` per index, the `spec` binary.

#![cfg(unix)]

mod code_index_common;
mod common;

use std::collections::BTreeSet;

use serde_json::json;
use specengine_store::{SnapshotSymbols, SqliteIndex, StoredSymbol};

use code_index_common::{FIXTURE, SLUG, copy, db, index};
use common::{Scratch, replace, spec, write};

/// A file of the code walk whose bytes are not UTF-8: stored, not read.
const RAW: &[u8] = b"pub fn raw() -> u32 {\n    1\n}\n// \xff\xfe not text\n";

/// X4: `code_snapshot` reads, in one transaction, what each reader reads
/// alone: its `refs` are `code_refs`; `Lookup` gives `lookup_symbols`'
/// hits and no symbol; `File` gives `file_symbols` (a RON file and an
/// unknown path too); `All` gives `code_symbols`; `Bound` gives each
/// symbol a stored marker binds, once, by (path, ord). Without indexed
/// code it answers nothing.
#[test]
fn code_snapshot_reads_each_form_as_its_reader_does() {
    let scratch = Scratch::new("ct-snapshot");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    index(&home, &root);
    let store = SqliteIndex::open(db(&home, SLUG), SLUG, &root).expect("open the index");
    let refs = store
        .code_refs()
        .expect("code_refs")
        .expect("code is indexed");
    let all = store
        .code_symbols()
        .expect("code_symbols")
        .expect("code is indexed");
    assert_eq!(all.len(), 31, "6a-2's golden: 31 symbols");

    for (query, name) in [
        ("separator", "separator"),
        ("impl Sheet #2", "impl Sheet"),
        ("crates/inkwell::twins::impl Sheet #2", "impl Sheet"),
        ("joined", "joined"),
        ("nothing_is_named_so", "nothing_is_named_so"),
    ] {
        let snapshot = store
            .code_snapshot(SnapshotSymbols::Lookup { query, name })
            .expect("a snapshot")
            .expect("code is indexed");
        assert_eq!(snapshot.refs, refs, "{query}");
        assert_eq!(
            snapshot.hits,
            store.lookup_symbols(query, name).expect("lookup"),
            "{query}"
        );
        assert!(snapshot.symbols.is_empty(), "{query}");
    }
    let separator = store
        .code_snapshot(SnapshotSymbols::Lookup {
            query: "separator",
            name: "separator",
        })
        .expect("a snapshot")
        .expect("code is indexed");
    assert_eq!(separator.hits.len(), 2, "the two cfg twins");

    for path in [
        "crates/inkwell/src/twins.rs",
        "crates/inkwell/src/lib.rs",
        "data/inks.ron",
        "crates/inkwell/src/nope.rs",
    ] {
        let snapshot = store
            .code_snapshot(SnapshotSymbols::File(path))
            .expect("a snapshot")
            .expect("code is indexed");
        assert_eq!(snapshot.refs, refs, "{path}");
        assert_eq!(
            snapshot.symbols,
            store.file_symbols(path).expect("file_symbols"),
            "{path}"
        );
        assert!(snapshot.hits.is_empty(), "{path}");
    }

    let every = store
        .code_snapshot(SnapshotSymbols::All)
        .expect("a snapshot")
        .expect("code is indexed");
    assert_eq!(every.refs, refs);
    assert_eq!(every.symbols, all);

    // The oracle: every symbol a marker names (its file, its ord), once.
    let bound_keys: BTreeSet<(String, u32)> = refs
        .markers
        .iter()
        .filter_map(|marker| {
            marker
                .binding
                .symbol
                .as_ref()
                .map(|symbol| (marker.path.clone(), symbol.ord))
        })
        .collect();
    let want: Vec<StoredSymbol> = all
        .iter()
        .filter(|symbol| bound_keys.contains(&(symbol.path.clone(), symbol.ord)))
        .cloned()
        .collect();
    let bound = store
        .code_snapshot(SnapshotSymbols::Bound)
        .expect("a snapshot")
        .expect("code is indexed");
    assert_eq!(bound.refs, refs);
    assert_eq!(bound.symbols, want);
    assert_eq!(bound.symbols.len(), bound_keys.len(), "each once");
    assert!(
        !bound.symbols.is_empty() && bound.symbols.len() < all.len(),
        "{} bound of {}",
        bound.symbols.len(),
        all.len()
    );
    let order: Vec<(String, u32)> = bound
        .symbols
        .iter()
        .map(|symbol| (symbol.path.clone(), symbol.ord))
        .collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "by (path, ord)");

    // No code indexed: nothing.
    let plain = scratch.copy("spec-a", "spec-a");
    let plain_home = scratch.home("plain");
    spec(&plain_home, &plain, &["index"]).code(0);
    let slug = "lantern-keep";
    let store = SqliteIndex::open(db(&plain_home, slug), slug, &plain).expect("open spec-a");
    assert_eq!(
        store
            .code_snapshot(SnapshotSymbols::All)
            .expect("a snapshot"),
        None
    );
}

/// `spec unmapped PATH` refuses a code file the walk does not read (exit
/// 2, stdout empty, the scope line naming it) in text and JSON, and a
/// directory holding only such files; the read file beside it answers.
#[test]
fn unmapped_refuses_an_unread_file_as_path() {
    let scratch = Scratch::new("ct-unread-path");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(&root, "crates/inkwell/src/raw.rs", RAW);
    write(&root, "crates/inkwell/src/rawdir/only.rs", RAW);
    for path in ["crates/inkwell/src/raw.rs", "crates/inkwell/src/rawdir"] {
        let line = format!(
            "spec: `{path}` is no `[code]` root, nor a directory or code file the walk reads \
             under one"
        );
        for args in [vec!["unmapped", path], vec!["--json", "unmapped", path]] {
            let run = spec(&home, &root, &args);
            run.code(2);
            assert_eq!(run.stdout, "", "{args:?}");
            assert!(
                run.stderr.lines().any(|stderr| stderr == line),
                "{args:?}: want {line:?} in\n{}",
                run.stderr
            );
        }
    }
    let read = spec(&home, &root, &["unmapped", "crates/inkwell/src/twins.rs"]);
    read.code(0);
    assert!(
        read.stdout
            .starts_with("crates/inkwell/src/twins.rs | units "),
        "{}",
        read.stdout
    );
}

/// A `[code]` root that is a code file the walk does not read is a file
/// row of no unit (`-`), never a directory's `<file>/`, in text and JSON.
#[test]
fn unmapped_lists_an_unread_root_file_as_a_file_row() {
    let scratch = Scratch::new("ct-unread-root");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    replace(
        &root,
        "specengine.toml",
        "roots = [\"crates\", \"data\"]",
        "roots = [\"crates\", \"data\", \"tools/raw.rs\"]",
    );
    write(&root, "tools/raw.rs", RAW);
    let run = spec(&home, &root, &["unmapped"]);
    run.code(0);
    assert!(
        run.stdout
            .lines()
            .any(|line| line == "tools/raw.rs | units 0 | mapped 0 | - | mentioned 0"),
        "{}",
        run.stdout
    );
    assert!(!run.stdout.contains("tools/raw.rs/"), "{}", run.stdout);
    let document = spec(&home, &root, &["--json", "unmapped"]).json();
    let row = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["path"] == json!("tools/raw.rs"))
        .unwrap_or_else(|| panic!("no tools/raw.rs row: {document}"));
    assert_eq!(
        row,
        &json!({"path": "tools/raw.rs", "dir": false, "units": 0, "mapped": 0,
            "percent": null, "mentioned": 0})
    );
}

/// On Unix `\` is a character of a directory name: a package directory
/// `crates/back\slash` keeps it in its items' qpaths (`file_place`) and in
/// `unmapped`'s row (`dir_text`); `/` never takes its place.
#[test]
fn a_backslash_package_keeps_its_name_on_unix() {
    let scratch = Scratch::new("ct-backslash");
    let home = scratch.home("h");
    let root = copy(&scratch, FIXTURE, "code-index");
    write(
        &root,
        "crates/back\\slash/Cargo.toml",
        "[package]\nname = \"backslash\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         publish = false\n\n[workspace]\n",
    );
    write(
        &root,
        "crates/back\\slash/src/lib.rs",
        "pub fn lone_back() -> u32 {\n    1\n}\n",
    );
    let document = spec(&home, &root, &["--json", "symbols", "lone_back"]).json();
    assert_eq!(
        document["symbols"][0]["qpath"],
        json!("crates/back\\slash::lone_back"),
        "{document}"
    );
    let run = spec(&home, &root, &["unmapped", "crates"]);
    run.code(0);
    assert!(
        run.stdout
            .lines()
            .any(|line| line.starts_with("crates/back\\slash/ | units 1 | ")),
        "{}",
        run.stdout
    );
    assert!(!run.stdout.contains("crates/back/slash"), "{}", run.stdout);
}
