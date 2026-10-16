//! docs/features/symbol-reads.md RD-3 (WA-12, WA-13, Q7, Q18): what
//! `spec unmapped` counts as mapped. One containment for every unit (the
//! item, an enclosing item or inline mod, its file's module); only a marker
//! whose ID is resolved, an alias or ambiguous maps (health row 5's rule):
//! a dangling, look-alike, queue or other-project ID maps nothing at any
//! depth; a mention never maps; a directory's percent comes from its summed
//! units, never from its children's percents.
//!
//! Over a scratch ledger project written at test time (another genre than
//! the fixtures: books, shelves, terms; `symbol_reads_common::ledger`), its
//! line numbers fixed by the sources there. Text byte for byte, the JSON
//! derived from it.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod symbol_reads_common;

use std::path::Path;

use serde_json::json;

use common::{Scratch, spec};
use symbol_reads_common::{
    ITEM_KEYS, ROW_KEYS, UNMAPPED_KEYS, assert_key_order, both, ledger, percents_as_f64,
    unmapped_json,
};

/// The ledger's walk, every summary's tail.
const CODE: &str = "unmapped: code files 15 (.rs 11, .ron 4), skipped 0";

/// `spec unmapped <path>` prints `lines` (then [`CODE`]) and the JSON
/// derived from them, keys in order.
fn assert_unmapped(home: &Path, root: &Path, path: &str, lines: &[&str]) {
    let (text, json_run) = both(home, root, &["unmapped", path]);
    let mut want: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    want.push(CODE.to_owned());
    let want_text: String = want.iter().map(|line| format!("{line}\n")).collect();
    text.code(0);
    assert_eq!(text.stdout, want_text, "spec unmapped {path}");
    json_run.code(0);
    let wanted = unmapped_json(&want, &json!(path));
    assert_eq!(
        percents_as_f64(&json_run.json()),
        percents_as_f64(&wanted),
        "spec --json unmapped {path}"
    );
    let rows = wanted["rows"].as_array().expect("rows").len();
    let items = wanted["unmapped"].as_array().expect("unmapped").len();
    assert_key_order(&json_run.stdout, "$", &UNMAPPED_KEYS, 1, path);
    assert_key_order(&json_run.stdout, "$.rows[]", &ROW_KEYS, rows, path);
    assert_key_order(&json_run.stdout, "$.total", &ROW_KEYS, 1, path);
    assert_key_order(&json_run.stdout, "$.unmapped[]", &ITEM_KEYS, items, path);
}

/// RD-3. M: no containment. A file module's marker maps every item of its
/// file (a struct, an impl and its method, an inline mod and its function);
/// an impl's marker maps its method; an inline mod's inner-doc marker maps
/// the mod and everything below it, two levels deep; a sibling mod and its
/// function stay unmapped.
#[test]
fn module_marker_maps_its_items() {
    let scratch = Scratch::new("su-containment");
    let root = ledger(&scratch, "ledger");
    let home = scratch.home("h");
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/book.rs",
        &["crates/ledger/src/book.rs | units 5 | mapped 5 | 100.0 % | mentioned 0"],
    );
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/shelf.rs",
        &[
            "crates/ledger/src/shelf.rs | units 8 | mapped 5 | 62.5 % | mentioned 0",
            "  crates/ledger::shelf::Shelf | struct_item | 10",
            "  crates/ledger::shelf::loose | mod_item | 22",
            "  crates/ledger::shelf::loose::alone | function_item | 23",
        ],
    );
}

/// RD-3 (Q7). M: mentions map. Mentions in every place a mention can sit
/// (the file's inner doc, an unanchored comment before a `use`: the file's,
/// an item's doc, a function body, a dangling marker's note, a RON comment)
/// make units `mentioned` and never mapped; a mention binding one item
/// reaches that item only.
#[test]
fn mentions_never_map() {
    let scratch = Scratch::new("su-mentions");
    let root = ledger(&scratch, "ledger");
    let home = scratch.home("h");
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/notes.rs",
        &[
            "crates/ledger/src/notes.rs | units 2 | mapped 0 | 0.0 % | mentioned 2",
            "  crates/ledger::notes::kept | function_item | 7",
            "  crates/ledger::notes::plain | function_item | 12",
        ],
    );
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/quiet.rs",
        &[
            "crates/ledger/src/quiet.rs | units 2 | mapped 0 | 0.0 % | mentioned 2",
            "  crates/ledger::quiet::one | function_item | 4",
            "  crates/ledger::quiet::two | function_item | 8",
        ],
    );
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/single.rs",
        &[
            "crates/ledger/src/single.rs | units 2 | mapped 0 | 0.0 % | mentioned 1",
            "  crates/ledger::single::noted | function_item | 2",
            "  crates/ledger::single::silent | function_item | 6",
        ],
    );
    assert_unmapped(&home, &root, "data", &DATA_ROWS);
}

/// The ledger's RON files: an alias marker anchored (mapped), a mention
/// only, a dangling marker anchored, a resolved marker unanchored (own line
/// before a closer).
const DATA_ROWS: [&str; 5] = [
    "data/even.ron | units 1 | mapped 1 | 100.0 % | mentioned 0",
    "data/notes.ron | units 1 | mapped 0 | 0.0 % | mentioned 1",
    "data/odd.ron | units 1 | mapped 0 | 0.0 % | mentioned 0",
    "data/tail.ron | units 1 | mapped 0 | 0.0 % | mentioned 0",
    "data (total) | units 4 | mapped 1 | 25.0 % | mentioned 1",
];

/// RD-3. M: percents averaged. A directory's units, mapped and mentioned
/// are its children's sums and its percent their ratio: `count/` is 7 of
/// 33 (21.2 %), not the mean of 100.0, 31.3 and 6.3; one level up `src`
/// is 22 of 64 (34.4 %). The per-mille rounds half up (1 of 16 is 6.3 %,
/// 5 of 16 is 31.3 %); an item-less file shows `-`.
#[test]
fn unmapped_counts_items() {
    let scratch = Scratch::new("su-counts");
    let root = ledger(&scratch, "ledger");
    let home = scratch.home("h");
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/count",
        &[
            "crates/ledger/src/count/few.rs | units 1 | mapped 1 | 100.0 % | mentioned 0",
            "crates/ledger/src/count/five.rs | units 16 | mapped 5 | 31.3 % | mentioned 0",
            "crates/ledger/src/count/many.rs | units 16 | mapped 1 | 6.3 % | mentioned 0",
            "crates/ledger/src/count (total) | units 33 | mapped 7 | 21.2 % | mentioned 0",
        ],
    );
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src",
        &[
            "crates/ledger/src/book.rs | units 5 | mapped 5 | 100.0 % | mentioned 0",
            "crates/ledger/src/count/ | units 33 | mapped 7 | 21.2 % | mentioned 0",
            "crates/ledger/src/even.rs | units 6 | mapped 5 | 83.3 % | mentioned 0",
            "crates/ledger/src/lib.rs | units 0 | mapped 0 | - | mentioned 0",
            "crates/ledger/src/notes.rs | units 2 | mapped 0 | 0.0 % | mentioned 2",
            "crates/ledger/src/odd.rs | units 6 | mapped 0 | 0.0 % | mentioned 0",
            "crates/ledger/src/quiet.rs | units 2 | mapped 0 | 0.0 % | mentioned 2",
            "crates/ledger/src/shelf.rs | units 8 | mapped 5 | 62.5 % | mentioned 0",
            "crates/ledger/src/single.rs | units 2 | mapped 0 | 0.0 % | mentioned 1",
            "crates/ledger/src (total) | units 64 | mapped 22 | 34.4 % | mentioned 5",
        ],
    );
}

/// RD-3 (Q18). M: any `id_state` maps. A dangling ID, a queue ID, another
/// project's ID and a look-alike whose Latin fix names a real node, as the
/// file's module markers, on an impl, on an inline mod, on a nested inline
/// mod and on an item: nothing in the file maps. The control file maps by
/// an ambiguous ID (two holders), an alias and a section ID at the same
/// depths; in RON an alias maps, a dangling ID or an unanchored marker
/// does not.
#[test]
fn dangling_marker_never_maps() {
    let scratch = Scratch::new("su-states");
    let root = ledger(&scratch, "ledger");
    let home = scratch.home("h");
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/odd.rs",
        &[
            "crates/ledger/src/odd.rs | units 6 | mapped 0 | 0.0 % | mentioned 0",
            "  crates/ledger::odd::impl Odd | impl_item | 7",
            "  crates/ledger::odd::Odd::even | function_item | 8",
            "  crates/ledger::odd::Odd | struct_item | 13",
            "  crates/ledger::odd::deep | mod_item | 15",
            "  crates/ledger::odd::deep::deeper | mod_item | 18",
            "  crates/ledger::odd::deep::deeper::bottom | function_item | 22",
        ],
    );
    assert_unmapped(
        &home,
        &root,
        "crates/ledger/src/even.rs",
        &[
            "crates/ledger/src/even.rs | units 6 | mapped 5 | 83.3 % | mentioned 0",
            "  crates/ledger::even::Even | struct_item | 10",
        ],
    );
    assert_unmapped(&home, &root, "data", &DATA_ROWS);
    // The look-alike is a real candidate: `refs` lists it under its fix's
    // node, so "names a node" is not "maps".
    let refs = spec(&home, &root, &["refs", "BOOK-SHELF"]);
    refs.code(0);
    assert!(
        refs.stdout
            .contains("crates/ledger/src/odd.rs:4:5 | \u{0412}OOK-SHELF@1 | homoglyph BOOK-SHELF"),
        "{}",
        refs.show()
    );
}
