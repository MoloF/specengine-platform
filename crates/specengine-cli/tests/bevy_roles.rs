//! docs/features/bevy-roles.md (slice 10a-1): TE-1 (`fixtures/code-bevy`
//! and its `expected.json`: every stored `code_roles` row, the symbols the
//! rows sit on and link to, the counts and the `spec index` line), RD-1
//! (derives), RD-2 (`add_message`, `impl Component for`, subjects: Z1,
//! SR3, SR4), RD-3's opt-in and stamp halves (F2), RD-7's index half (the
//! `roles` line counts symbols, lists every word, is absent off) and TE-6's
//! "every output 6b-1's but `code.roles: null`" for `spec index`.
//!
//! Real data: the `spec` binary over scratch copies of the fixtures and
//! over probe projects written at test time, each index in a scratch
//! `HOME`; the stored rows read by column name through the system's
//! `sqlite3`. A probe holds one package, `crates/probe`, its sources the
//! construct under test; what the row says is read from the table, what a
//! link gives from the `roles` line (links are computed per read, never
//! stored: "Links").
//!
//! Readings the spec leaves open, as built (iteration 1; the report's
//! L-list): the derive row's `line` (its element's), `form` NULL on
//! derive, impl, fn and uncertain rows, `symbol_ord` NULL on rows with a
//! subject, `certainty`
//! `certain` on every row with a subject, rows in source order with a
//! `pipe` argument after its leaf, an unreadable `add_message` in tokens
//! `uncertain` with the shipped category `metavariable`.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod code_index_common;
mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use serde_json::{Value, json};

use bevy_roles_common::{
    CODE_EMPTY, CODE_OFF, CODE_ON, PROBE_SLUG, ROLE_COLUMNS, Row, SLUG, WORDS, app_fn, called,
    copy, derived, derived_cfg_attr, expected, implemented, index, index_json, line_of,
    parse_roles_line, probe, role_rows, roles_line, roles_line_of_run, rows_of, set_code, stored,
    unreadable,
};
use code_index_common::{
    FILE_JOIN, assert_equals_fresh, code_line, count, counts_of, db, query, with_code_table,
};
use common::{Scratch, spec, write};

/// The rows of `files` in a probe project: `spec index` (exit 0), the
/// stored rows of `crates/probe/src/lib.rs` and the `roles` line, which
/// JSON `code.roles` equals.
fn probe_rows(name: &str, files: &[(&str, &str)]) -> (Vec<BTreeMap<String, Value>>, String) {
    let scratch = Scratch::new(name);
    let home = scratch.home("h");
    let root = probe(&scratch, "probe", files);
    let run = index(&home, &root);
    let line = roles_line_of_run(&run);
    let json = index_json(&home, &root);
    assert_eq!(
        json["code"]["roles"],
        parse_roles_line(&line),
        "JSON `code.roles` is the line: {json}"
    );
    (
        rows_of(&db(&home, PROBE_SLUG), "crates/probe/src/lib.rs"),
        line,
    )
}

fn lib(text: &str) -> [(&str, &str); 1] {
    [("crates/probe/src/lib.rs", text)]
}

// ------------------------------------------------------------------ TE-1

/// TE-1. M: a role rule, a column or an order of the fixture changed. Every
/// `code_roles` row of `fixtures/code-bevy` as `expected.json` gives it, by
/// (path, ord); its `code_files` (two packages) and its 41 symbols; each
/// `symbol_ord` the symbol `sits_on` names; each link candidate a symbol of
/// the row's `subject_name`; the `spec index` lines, `roles` right after
/// `code` and before `db`.
#[test]
fn code_bevy_role_rows_match_expected_json() {
    let want = expected();
    let scratch = Scratch::new("br-te1");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy");
    let run = index(&home, &root);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines.len(), 4, "indexed, code, roles, db:\n{}", run.show());
    assert!(
        lines[0].starts_with("indexed code-bevy: "),
        "{}",
        run.show()
    );
    assert_eq!(lines[1], want["index_code_line"], "{}", run.show());
    assert_eq!(lines[2], want["index_roles_line"], "{}", run.show());
    assert!(lines[3].starts_with("db "), "{}", run.show());

    let db = db(&home, SLUG);
    let files: Vec<Value> = query(
        &db,
        "SELECT path, package, module FROM code_files ORDER BY path",
    )
    .into_iter()
    .map(|row| json!(row))
    .collect();
    assert_eq!(Value::Array(files), want["code_files"], "code_files");

    let symbols: Vec<Value> = query(
        &db,
        &format!(
            "SELECT f.path AS path, x.ord AS ord, x.item_kind AS item_kind, x.name AS name, \
             x.qpath AS qpath FROM symbols x {FILE_JOIN} ORDER BY f.path, x.ord"
        ),
    )
    .into_iter()
    .map(|row| json!(row))
    .collect();
    assert_eq!(symbols.len(), 41, "{symbols:#?}");
    assert_eq!(Value::Array(symbols.clone()), want["symbols"], "symbols");

    let have: Vec<Value> = role_rows(&db).into_iter().map(|row| json!(row)).collect();
    let rows = want["roles"].as_array().expect("roles");
    let projected: Vec<Value> = rows
        .iter()
        .map(|row| {
            let mut out = serde_json::Map::new();
            out.insert("path".to_owned(), row["path"].clone());
            for column in ROLE_COLUMNS {
                out.insert(column.to_owned(), row[column].clone());
            }
            Value::Object(out)
        })
        .collect();
    assert_eq!(have.len(), 27, "{have:#?}");
    for (at, (have, want)) in have.iter().zip(&projected).enumerate() {
        assert_eq!(have, want, "row {at}");
    }

    let by_key: BTreeMap<(String, u64), &Value> = symbols
        .iter()
        .map(|row| {
            let path = row["path"].as_str().expect("path").to_owned();
            ((path, row["ord"].as_u64().expect("ord")), row)
        })
        .collect();
    let by_qpath: BTreeMap<&str, &Value> = symbols
        .iter()
        .map(|row| (row["qpath"].as_str().expect("qpath"), row))
        .collect();
    for row in rows {
        if let Some(ord) = row["symbol_ord"].as_u64() {
            let path = row["path"].as_str().expect("path").to_owned();
            assert_eq!(by_key[&(path, ord)]["qpath"], row["sits_on"], "{row}");
        } else {
            assert!(row["sits_on"].is_null(), "{row}");
        }
        for candidate in row["candidates"].as_array().expect("candidates") {
            let symbol = by_qpath
                .get(candidate.as_str().expect("a qpath"))
                .unwrap_or_else(|| panic!("{candidate} is no symbol: {row}"));
            assert_eq!(symbol["name"], row["subject_name"], "{row}");
        }
    }
    for qpath in want["not_candidates"]
        .as_object()
        .expect("not_candidates")
        .keys()
    {
        if qpath != "_comment" {
            assert!(by_qpath.contains_key(qpath.as_str()), "{qpath} is a symbol");
        }
    }
}

/// TE-1, RD-7, RD-5. M: a link state or a count rule changed. The counts
/// follow from `expected.json`'s per-row states (each word: the symbols
/// holding a row of it `certain`, `linked` or `ambiguous`, ambiguous on
/// every candidate; `subjects` the rows with a subject by state;
/// `uncertain` the rows `uncertain`), equal its `holders` and
/// `code_roles`, and `spec index` gives them, text and JSON (JSON `code`
/// keeps the code line's counts beside `roles`).
#[test]
fn code_bevy_counts_follow_from_the_row_states() {
    let want = expected();
    let mut holders: BTreeMap<String, BTreeSet<String>> = WORDS
        .iter()
        .map(|word| ((*word).to_owned(), BTreeSet::new()))
        .collect();
    let (mut linked, mut ambiguous, mut unlinked, mut uncertain) = (0, 0, 0, 0);
    for row in want["roles"].as_array().expect("roles") {
        let word = row["word"].as_str().expect("word").to_owned();
        let candidates: Vec<String> = row["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .map(|c| c.as_str().expect("a qpath").to_owned())
            .collect();
        let held = holders.get_mut(&word).expect("a word of the spec");
        match row["state"].as_str().expect("state") {
            "linked" => {
                assert!(candidates.len() == 1 && row["subject"].is_string(), "{row}");
                linked += 1;
                held.extend(candidates);
            }
            "ambiguous" => {
                assert!(candidates.len() > 1 && row["subject"].is_string(), "{row}");
                ambiguous += 1;
                held.extend(candidates);
            }
            "unlinked" => {
                assert!(candidates.is_empty() && row["subject"].is_string(), "{row}");
                unlinked += 1;
            }
            "certain" => {
                assert!(row["subject"].is_null(), "{row}");
                assert_eq!(row["certainty"], "certain", "{row}");
                held.insert(row["sits_on"].as_str().expect("sits_on").to_owned());
            }
            "uncertain" => {
                assert_eq!(row["certainty"], "uncertain", "{row}");
                assert!(row["category"].is_string(), "{row}");
                uncertain += 1;
            }
            other => panic!("no state {other}: {row}"),
        }
    }
    let words: serde_json::Map<String, Value> = holders
        .iter()
        .map(|(word, set)| (word.clone(), json!(set.len())))
        .collect();
    let derived = json!({
        "words": words,
        "subjects": {
            "total": linked + ambiguous + unlinked,
            "linked": linked,
            "ambiguous": ambiguous,
            "unlinked": unlinked,
        },
        "uncertain": uncertain,
    });
    assert_eq!(derived, want["code_roles"], "expected.json is consistent");
    let listed: BTreeMap<String, BTreeSet<String>> =
        serde_json::from_value(want["holders"].clone()).expect("holders");
    assert_eq!(holders, listed, "holders");
    assert_eq!(
        parse_roles_line(want["index_roles_line"].as_str().expect("the line")),
        want["code_roles"]
    );

    // Text and JSON each from a fresh `HOME`: both first runs, so the code
    // line's `parsed` and `unchanged` compare like with like.
    let scratch = Scratch::new("br-counts");
    let root = copy(&scratch, "code-bevy");
    let run = index(&scratch.home("text"), &root);
    assert_eq!(roles_line_of_run(&run), want["index_roles_line"]);
    let json = index_json(&scratch.home("json"), &root);
    assert_eq!(json["code"]["roles"], want["code_roles"], "{json}");
    let code = json["code"].as_object().expect("a code object");
    for (word, value) in counts_of(&run) {
        if word != "rederived_all" {
            assert_eq!(code[word.as_str()], value, "{word}: {json}");
        }
    }
}

// ------------------------------------------------------------------ RD-1

/// RD-1. M: first element only. A derive list yields a row per element
/// whose last segment is a role word, in element order, on a struct, an
/// enum and a union, `certain`, `symbol_ord` the item.
#[test]
fn derive_list_yields_every_role() {
    let source = r#"use bevy::prelude::*;

#[derive(Component, Resource, Message, Event, EntityEvent, Reflect)]
pub struct Every {
    pub entity: Entity,
}

#[derive(Component, Debug)]
pub enum Mood {
    Calm,
}

#[derive(Component)]
pub union Bits {
    pub raw: u32,
}
"#;
    let (rows, line) = probe_rows("br-derive-list", &lib(source));
    assert_eq!(
        rows,
        stored(&[
            derived("component", 0, 3),
            derived("resource", 0, 3),
            derived("message", 0, 3),
            derived("event", 0, 3),
            derived("entity_event", 0, 3),
            derived("reflect", 0, 3),
            derived("component", 1, 8),
            derived("component", 2, 13),
        ])
    );
    let counts = [
        ("component", 3),
        ("entity_event", 1),
        ("event", 1),
        ("message", 1),
        ("reflect", 1),
        ("resource", 1),
    ];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [0, 0, 0], 0));
}

/// RD-1. M: a qualified path dropped. `bevy::prelude::Component`,
/// `::bevy::ecs::resource::Resource` and `bevy::reflect::Reflect` read by
/// their last segment.
#[test]
fn qualified_derive_path_is_read() {
    let source = r#"#[derive(bevy::prelude::Component, ::bevy::ecs::resource::Resource, Debug)]
pub struct Field;

#[derive(bevy::reflect::Reflect)]
pub struct Plot;
"#;
    let (rows, line) = probe_rows("br-derive-qualified", &lib(source));
    assert_eq!(
        rows,
        stored(&[
            derived("component", 0, 1),
            derived("resource", 0, 1),
            derived("reflect", 1, 4),
        ])
    );
    let counts = [("component", 1), ("reflect", 1), ("resource", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [0, 0, 0], 0));
}

/// RD-1. M: `cfg_attr` certain. A derive inside `#[cfg_attr(<p>,
/// derive(..))]` is `uncertain`, category `cfg_attr`, on its item; it
/// counts as `uncertain`, never for its word; a plain derive beside it
/// stays `certain`.
#[test]
fn cfg_attr_derive_is_uncertain() {
    let source = r#"use bevy::prelude::*;

#[cfg_attr(feature = "net", derive(Component, Debug))]
pub struct Ghost;

#[derive(Reflect)]
#[cfg_attr(test, derive(Resource))]
pub struct Shade;
"#;
    let (rows, line) = probe_rows("br-derive-cfg-attr", &lib(source));
    assert_eq!(
        rows,
        stored(&[
            derived_cfg_attr("component", 0, 3),
            derived("reflect", 1, 6),
            derived_cfg_attr("resource", 1, 7),
        ])
    );
    assert_eq!(line, line_of(PROBE_SLUG, &[("reflect", 1)], [0, 0, 0], 2));
}

/// RD-1. M: a second `#[derive]` ignored. Every derive attribute of an
/// item is read, each at its own line.
#[test]
fn every_derive_attribute_is_read() {
    let source = r#"use bevy::prelude::*;

#[derive(Debug)]
#[derive(Component)]
#[derive(Clone, Reflect)]
pub struct Barn;
"#;
    let (rows, line) = probe_rows("br-derive-attributes", &lib(source));
    assert_eq!(
        rows,
        stored(&[derived("component", 0, 4), derived("reflect", 0, 5)])
    );
    let counts = [("component", 1), ("reflect", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [0, 0, 0], 0));
}

/// RD-1 ("As written, never resolved"). M: `Debug` given a role. Other
/// derives, look-alike names (a plural, a prefix, another case) and a
/// derive inside a `macro_rules!` body (its item is no symbol) give no row;
/// every word 0.
#[test]
fn other_derives_name_no_role() {
    let source = r#"use bevy::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Plain;

#[derive(Components, MyComponent, component, Resources, Reflected)]
pub struct Lookalike;

macro_rules! made {
    () => {
        #[derive(Component)]
        pub struct Made;
    };
}

made!();
"#;
    let (rows, line) = probe_rows("br-derive-other", &lib(source));
    assert_eq!(rows, stored(&[]));
    assert_eq!(line, line_of(PROBE_SLUG, &[], [0, 0, 0], 0));
}

/// "Role facts": RON, unreadable and non-UTF-8 files hold no role. M: the
/// detector run over a lossy decode; an unreadable file's rows kept. A
/// probe whose `lib.rs` is not UTF-8 (a derive and a registration in it),
/// a file of mode 000 with a derive, a RON file under the root: `spec
/// index` exits 0 (unreadable 1, skipped 1), `code_roles` holds no row,
/// every word 0.
#[test]
fn ron_unreadable_and_non_utf8_files_hold_no_role() {
    let scratch = Scratch::new("br-no-role-files");
    let home = scratch.home("h");
    let root = probe(&scratch, "probe", &[]);
    write(
        &root,
        "crates/probe/src/lib.rs",
        b"// caf\xe9\nuse bevy::prelude::*;\n\n#[derive(Component)]\npub struct Latin;\n\npub fn wire(app: &mut App) {\n    app.add_systems(Update, wire);\n}\n"
            .as_slice(),
    );
    write(
        &root,
        "crates/probe/src/locked.rs",
        "#[derive(Component)]\npub struct Locked;\n",
    );
    write(
        &root,
        "crates/probe/tuning.ron",
        "// component: Resource\n(component: 1)\n",
    );
    let locked = root.join("crates/probe/src/locked.rs");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (counts["unreadable"], counts["skipped"]),
        (1, 1),
        "{}",
        run.show()
    );
    assert_eq!(count(&db(&home, PROBE_SLUG), "code_roles"), 0);
    assert_eq!(
        roles_line_of_run(&run),
        line_of(PROBE_SLUG, &[], [0, 0, 0], 0)
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).expect("chmod back");
}

// ------------------------------------------------------------------ RD-2

/// RD-2 (Z1). M: the method not read. `.add_message::<T>()` is a
/// `message` row of source `call`, its subject the type as written, linked
/// to the struct; in a chain each call at its own line.
#[test]
fn add_message_registers_its_type() {
    let source = r#"pub mod news;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_message::<news::Harvest>()
        .add_message::<Quake>();
}

#[derive(Debug)]
pub struct Quake;
"#;
    let files = [
        ("crates/probe/src/lib.rs", source),
        ("crates/probe/src/news.rs", "pub struct Harvest;\n"),
    ];
    let (rows, line) = probe_rows("br-message", &files);
    assert_eq!(
        rows,
        stored(&[
            app_fn(1, 5),
            called(
                "message",
                "path",
                "code",
                None,
                "news::Harvest",
                "Harvest",
                6
            ),
            called("message", "path", "code", None, "Quake", "Quake", 7),
        ])
    );
    let counts = [("message", 2), ("plugin", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [2, 0, 0], 0));
}

/// RD-2 ("As written, never resolved"). M: macro tokens resolved. In a
/// `macro_rules!` transcriber `add_message` names its type by its name only
/// (`origin` `macro_rules`), a metavariable is `uncertain` (the shipped
/// category `metavariable`, no subject), never filled from an invocation;
/// in a macro invocation's tokens it reads alike (`macro_call`).
#[test]
fn add_message_in_macro_tokens_is_name_only() {
    let source = r#"pub mod news;

use bevy::prelude::*;

macro_rules! register {
    ($app:expr, $t:ty) => {
        $app.add_message::<news::Quake>();
        $app.add_message::<$t>();
    };
}

pub fn wire(app: &mut App) {
    register!(app, news::Rain);
    wrap! {
        app.add_message::<news::Rain>();
    }
}
"#;
    let files = [
        ("crates/probe/src/lib.rs", source),
        (
            "crates/probe/src/news.rs",
            "pub struct Quake;\n\npub struct Rain;\n",
        ),
    ];
    let (rows, line) = probe_rows("br-message-macro", &files);
    assert_eq!(
        rows,
        stored(&[
            called("message", "path", "macro_rules", None, "Quake", "Quake", 7),
            unreadable("message", "macro_rules", "metavariable", 8),
            app_fn(2, 12),
            called("message", "path", "macro_call", None, "Rain", "Rain", 15),
        ])
    );
    let counts = [("message", 2), ("plugin", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [2, 0, 0], 1));
}

/// RD-2 ("Subject"). M: generics kept in `subject`. `news::Envelope<u8>`
/// is `news::Envelope` (linked at its suffix), `Envelope<Vec<u8>>` is
/// `Envelope` (two items of that name at the suffix tier: ambiguous on
/// each).
#[test]
fn add_message_subject_drops_generic_arguments() {
    let source = r#"pub mod news;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_message::<news::Envelope<u8>>();
    app.add_message::<Envelope<Vec<u8>>>();
}

pub struct Envelope<T>(pub T);
"#;
    let files = [
        ("crates/probe/src/lib.rs", source),
        (
            "crates/probe/src/news.rs",
            "pub struct Envelope<T>(pub T);\n",
        ),
    ];
    let (rows, line) = probe_rows("br-message-generic", &files);
    assert_eq!(
        rows,
        stored(&[
            app_fn(1, 5),
            called(
                "message",
                "path",
                "code",
                None,
                "news::Envelope",
                "Envelope",
                6
            ),
            called("message", "path", "code", None, "Envelope", "Envelope", 7),
        ])
    );
    let counts = [("message", 2), ("plugin", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [1, 1, 0], 0));
}

/// RD-2 (Z1). M: `impl Component for` not read. `impl Component for T`
/// (a qualified trait path, a generic type too) is a `component` row of
/// source `impl`, subject `T` without its generics, linked to the struct;
/// an inherent `impl` gives none.
#[test]
fn impl_component_links_its_type() {
    let source = r#"use bevy::prelude::*;

pub struct Tractor;

impl Component for Tractor {
    type Mutability = Mutable;
}

pub struct Cart<T>(T);

impl<T: Send + Sync + 'static> bevy::ecs::component::Component for Cart<T> {
    type Mutability = Mutable;
}

pub struct Plow;

impl Plow {}
"#;
    let (rows, line) = probe_rows("br-impl-component", &lib(source));
    assert_eq!(
        rows,
        stored(&[
            implemented("component", "Tractor", 5),
            implemented("component", "Cart", 11),
        ])
    );
    assert_eq!(line, line_of(PROBE_SLUG, &[("component", 2)], [2, 0, 0], 0));
}

/// RD-2 (SR3). M: `crate::` kept. `crate::clock::run` and
/// `super::clock::run` are stored as `clock::run` and meet `clock::run`'s
/// suffix (linked, beside `fields::run`); `self::sow` is `sow`.
#[test]
fn crate_prefixed_subject_meets_its_suffix() {
    let source = r#"pub mod clock;
pub mod fields;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_systems(Update, crate::clock::run);
}
"#;
    let fields = r#"use bevy::prelude::*;

pub fn run() {}

pub fn sow() {}

pub fn wire_fields(app: &mut App) {
    app.add_systems(Update, (self::sow, super::clock::run));
}
"#;
    let scratch = Scratch::new("br-crate-prefix");
    let home = scratch.home("h");
    let root = probe(
        &scratch,
        "probe",
        &[
            ("crates/probe/src/lib.rs", source),
            ("crates/probe/src/clock.rs", "pub fn run() {}\n"),
            ("crates/probe/src/fields.rs", fields),
        ],
    );
    let run = index(&home, &root);
    let db = db(&home, PROBE_SLUG);
    let update = Some("Update");
    assert_eq!(
        rows_of(&db, "crates/probe/src/lib.rs"),
        stored(&[
            app_fn(2, 6),
            called("system", "path", "code", update, "clock::run", "run", 7),
        ])
    );
    assert_eq!(
        rows_of(&db, "crates/probe/src/fields.rs"),
        stored(&[
            app_fn(2, 7),
            called("system", "path", "code", update, "sow", "sow", 8),
            called("system", "path", "code", update, "clock::run", "run", 8),
        ])
    );
    let counts = [("plugin", 2), ("system", 2)];
    assert_eq!(
        roles_line_of_run(&run),
        line_of(PROBE_SLUG, &counts, [3, 0, 0], 0)
    );
}

/// RD-2 (SR4). M: a `pipe` argument dropped. Each `pipe` argument is a
/// `system` row of its leaf's group, after its leaf, linked as a path.
#[test]
fn piped_system_is_a_row() {
    let source = r#"pub mod sky;

use bevy::prelude::*;

pub fn wire(app: &mut App) {
    app.add_systems(Update, sky::sense.pipe(sky::react).pipe(sky::store));
}
"#;
    let sky = "pub fn sense() -> bool {\n    true\n}\n\npub fn react(_: In<bool>) -> u8 {\n    0\n}\n\npub fn store(_: In<u8>) {}\n";
    let files = [
        ("crates/probe/src/lib.rs", source),
        ("crates/probe/src/sky.rs", sky),
    ];
    let (rows, line) = probe_rows("br-pipe", &files);
    let update = Some("Update");
    assert_eq!(
        rows,
        stored(&[
            app_fn(1, 5),
            called("system", "path", "code", update, "sky::sense", "sense", 6),
            called("system", "path", "code", update, "sky::react", "react", 6),
            called("system", "path", "code", update, "sky::store", "store", 6),
        ])
    );
    let counts = [("plugin", 1), ("system", 3)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [3, 0, 0], 0));
}

// ------------------------------------- review 1 targets (iteration 2)

/// Review 1 target, built in iteration 2 (B2; "Subject": a factory,
/// its callee). M: a lowercase qualified callee read as `P::new(..)`.
/// `add_plugins(a::make_plugin())` is form `factory`, subject the whole
/// callee `a::make_plugin`, linked to the fn; `HudPlugin::new()` (an
/// uppercase qualifier: the type) stays form `path`, subject `HudPlugin`.
#[test]
fn add_plugins_factory_subject_is_its_whole_callee() {
    let source = r#"pub mod a;

use bevy::prelude::*;

pub struct HudPlugin;

pub fn wire(app: &mut App, level: u8) {
    app.add_plugins((a::make_plugin(), HudPlugin::new()));
    let _ = level;
}
"#;
    let files = [
        ("crates/probe/src/lib.rs", source),
        (
            "crates/probe/src/a.rs",
            "pub fn make_plugin() -> super::HudPlugin {\n    super::HudPlugin\n}\n",
        ),
    ];
    let (rows, line) = probe_rows("br-plugin-factory", &files);
    assert_eq!(
        rows,
        stored(&[
            called(
                "plugin",
                "factory",
                "code",
                None,
                "a::make_plugin",
                "make_plugin",
                8
            ),
            called("plugin", "path", "code", None, "HudPlugin", "HudPlugin", 8),
        ])
    );
    let counts = [("plugin", 2)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [2, 0, 0], 0));
}

/// Review 1 target, built in iteration 2 (B3; "what the detector
/// cannot read is uncertain"). M: a derive attribute with a parse error
/// dropped. `#[derive(Component, Reflect]` (an unbalanced delimiter) is
/// read by name only: one `uncertain` `parse_error` row per role name, on
/// the item, at the name's line; the line counts both.
#[test]
fn derive_with_a_parse_error_is_uncertain() {
    let source = "use bevy::prelude::*;\n\n#[derive(Component, Reflect]\npub struct Broken;\n";
    let (rows, line) = probe_rows("br-derive-broken", &lib(source));
    let broken = |word| -> Row<'static> {
        (
            word,
            "derive",
            None,
            "code",
            None,
            Some(0),
            None,
            None,
            "uncertain",
            Some("parse_error"),
            3,
        )
    };
    assert_eq!(rows, stored(&[broken("component"), broken("reflect")]));
    assert_eq!(line, line_of(PROBE_SLUG, &[], [0, 0, 0], 2));
}

/// Iteration 2 (B3, as built): a derive attribute with a parse error that
/// names no role gives no row; nothing counts. M: a role guessed for any
/// name of a broken attribute.
#[test]
fn broken_derive_naming_no_role_gives_no_row() {
    let source = "use bevy::prelude::*;\n\n#[derive(Debug, Clone]\npub struct Plain;\n";
    let (rows, line) = probe_rows("br-derive-broken-plain", &lib(source));
    assert_eq!(rows, stored(&[]));
    assert_eq!(line, line_of(PROBE_SLUG, &[], [0, 0, 0], 0));
}

/// Iteration 2 (B2, as built): a lowercase callee whose qualifier is no
/// path (`<HudPlugin as Default>::default()`) is form `factory`, subject
/// the whole callee as written (single spaces kept); a turbofish on the
/// method (`HudPlugin::new::<u8>()`) keeps the uppercase qualifier as the
/// type, form `path`. M: the bracketed qualifier read as the type; the
/// turbofish breaking the qualifier rule.
#[test]
fn add_plugins_reads_qualified_type_and_turbofish_callees() {
    let source = r#"use bevy::prelude::*;

#[derive(Default)]
pub struct HudPlugin;

pub fn wire(app: &mut App, level: u8) {
    app.add_plugins((<HudPlugin as Default>::default(), HudPlugin::new::<u8>()));
    let _ = level;
}
"#;
    let (rows, line) = probe_rows("br-plugin-qualified", &lib(source));
    assert_eq!(
        rows,
        stored(&[
            called(
                "plugin",
                "factory",
                "code",
                None,
                "<HudPlugin as Default>::default",
                "default",
                7
            ),
            called("plugin", "path", "code", None, "HudPlugin", "HudPlugin", 7),
        ])
    );
    let counts = [("plugin", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [1, 0, 1], 0));
}

/// Review 1 target, built in iteration 2 (B3). M: an unclosed
/// turbofish in macro tokens dropped. `$app.add_message::<Quake;` in a
/// transcriber gives an `uncertain` `message` row, category `arguments`,
/// at the method's line.
#[test]
fn unclosed_add_message_turbofish_is_uncertain() {
    let source = r#"macro_rules! register {
    ($app:expr) => {
        $app.add_message::<Quake;
    };
}

pub struct Quake;
"#;
    let (rows, line) = probe_rows("br-message-unclosed", &lib(source));
    assert_eq!(
        rows,
        stored(&[unreadable("message", "macro_rules", "arguments", 3)])
    );
    assert_eq!(line, line_of(PROBE_SLUG, &[], [0, 0, 0], 1));
}

/// Review 1 target, built in iteration 2 (the qualifier nit): a
/// qualifier that is no plain path keeps its single spaces.
/// `<Farm as Grow>::tick` is stored as written, never `<FarmasGrow>::tick`.
#[test]
fn qualified_type_subject_keeps_its_spaces() {
    let source = r#"use bevy::prelude::*;

pub trait Grow {
    fn tick();
}

pub struct Farm;

impl Grow for Farm {
    fn tick() {}
}

pub fn wire(app: &mut App) {
    app.add_systems(Update, <Farm as Grow>::tick);
}
"#;
    let (rows, _) = probe_rows("br-qualified-type", &lib(source));
    assert_eq!(
        rows,
        stored(&[
            app_fn(5, 13),
            called(
                "system",
                "path",
                "code",
                Some("Update"),
                "<Farm as Grow>::tick",
                "tick",
                14
            ),
        ])
    );
}

// ------------------------------------------------------------ RD-3 (F2)

/// RD-3 (F2). M: `frameworks` outside the stamp. Opt-in off re-derives
/// every code file (`rederived all`) and leaves no role row and no line;
/// `frameworks = []` is the same stamp (nothing re-derived); on again
/// re-derives every file and the rows come back. Each step equals a fresh
/// index.
#[test]
fn frameworks_change_rederives_roles() {
    let want = expected();
    let scratch = Scratch::new("br-opt-in");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy");
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["parsed"], 9, "{}", run.show());
    let db = db(&home, SLUG);
    assert_eq!(count(&db, "code_roles"), 27);

    set_code(&root, CODE_OFF);
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (counts["parsed"], counts.get("rederived_all")),
        (9, Some(&1)),
        "{}",
        run.show()
    );
    assert!(roles_line(&run).is_none(), "{}", run.show());
    assert_eq!(count(&db, "code_roles"), 0);
    assert!(index_json(&home, &root)["code"]["roles"].is_null());
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the opt-in off");

    set_code(&root, CODE_EMPTY);
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (
            counts["parsed"],
            counts["unchanged"],
            counts.get("rederived_all")
        ),
        (0, 9, None),
        "`[]` is the stamp of no key: {}",
        run.show()
    );
    assert!(roles_line(&run).is_none(), "{}", run.show());
    assert_equals_fresh(&scratch, &home, &root, SLUG, "listed empty");

    set_code(&root, CODE_ON);
    let run = index(&home, &root);
    let counts = counts_of(&run);
    assert_eq!(
        (counts["parsed"], counts.get("rederived_all")),
        (9, Some(&1)),
        "{}",
        run.show()
    );
    assert_eq!(count(&db, "code_roles"), 27);
    assert_eq!(roles_line_of_run(&run), want["index_roles_line"]);
    assert_equals_fresh(&scratch, &home, &root, SLUG, "the opt-in on again");
}

/// `parts`, each prefixed with its length (u64, little-endian), BLAKE3'd
/// as `b3:` hex: the store's framing of `code_fp` (6a-2: `framed_hash(
/// [analyzer stamp, [ids] fingerprint])`, store `code.rs` `update`).
fn framed(parts: &[&[u8]]) -> String {
    let mut stream = Vec::new();
    for part in parts {
        let length = u64::try_from(part.len()).expect("a length");
        stream.extend_from_slice(&length.to_le_bytes());
        stream.extend_from_slice(part);
    }
    specengine_store::b3_hash(&stream)
}

/// RD-3 (F2). M: the key left out when empty. The stored `code_fp` is the
/// framing of the analyzer stamp `{format, levels, roots, exclude,
/// frameworks}` (compact JSON, `frameworks` always present, sorted,
/// repeats dropped, `[]` when absent or empty) and the worktree's `[ids]`
/// fingerprint (`worktrees.scheme_fp`).
#[test]
fn stamp_always_holds_frameworks() {
    let levels: String = specengine_code::level_header()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let twice = "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = [\"bevy\", \"bevy\"]\n";
    let mut stamps = BTreeMap::new();
    for (label, table, frameworks) in [
        ("absent", CODE_OFF, "[]"),
        ("empty", CODE_EMPTY, "[]"),
        ("on", CODE_ON, "[\"bevy\"]"),
        ("twice", twice, "[\"bevy\"]"),
    ] {
        let scratch = Scratch::new("br-stamp");
        let home = scratch.home("h");
        let root = copy(&scratch, "code-bevy");
        with_code_table(&root, table);
        index(&home, &root);
        let db = db(&home, SLUG);
        let stored_fp = query(&db, "SELECT code_fp FROM code_worktrees");
        assert_eq!(stored_fp.len(), 1, "{label}: one code worktree");
        let scheme = query(&db, "SELECT scheme_fp FROM worktrees");
        assert_eq!(scheme.len(), 1, "{label}: one worktree");
        let stamp = format!(
            "{{\"format\":{},\"levels\":\"{levels}\",\"roots\":[\"crates\",\"assets\"],\"exclude\":[],\"frameworks\":{frameworks}}}",
            specengine_code::CODE_INDEX_FORMAT
        );
        let scheme_fp = scheme[0]["scheme_fp"].as_str().expect("a scheme_fp");
        let want = framed(&[stamp.as_bytes(), scheme_fp.as_bytes()]);
        assert_eq!(stored_fp[0]["code_fp"], want, "{label}: {stamp}");
        stamps.insert(label, want);
    }
    assert_eq!(stamps["absent"], stamps["empty"]);
    assert_eq!(stamps["on"], stamps["twice"]);
    assert_ne!(stamps["absent"], stamps["on"]);
}

/// RD-3 ("Rules and edge cases"). M: the detector run without the opt-in.
/// code-bevy with `frameworks` absent or `[]`: the code rows are the
/// fixture's (its code line), `code_roles` holds no row, no `roles` line,
/// JSON `code.roles` `null`.
#[test]
fn no_roles_without_frameworks() {
    let want = expected();
    for table in [CODE_OFF, CODE_EMPTY] {
        let scratch = Scratch::new("br-off");
        let home = scratch.home("h");
        let root = copy(&scratch, "code-bevy");
        set_code(&root, table);
        let run = index(&home, &root);
        assert_eq!(
            code_line(&run).expect("a code line"),
            want["index_code_line"],
            "{table}"
        );
        assert!(roles_line(&run).is_none(), "{table}: {}", run.show());
        assert_eq!(run.stdout.lines().count(), 3, "{table}: {}", run.show());
        assert_eq!(count(&db(&home, SLUG), "code_roles"), 0, "{table}");
        let json = index_json(&home, &root);
        let code = json["code"].as_object().expect("a code object");
        assert!(
            code.contains_key("roles") && code["roles"].is_null(),
            "{table}: {json}"
        );
    }
}

// ------------------------------------------------------------------ RD-7

/// RD-7 (G1). M: rows counted, not symbols. A system registered four
/// times (three calls, a combinator's argument aside) counts once; on
/// code-bevy 9 `system` rows give 7 symbols (three rows on `crops::tick`,
/// `clock::run` held by a linked and an ambiguous row).
#[test]
fn role_counts_count_symbols() {
    let source = r#"use bevy::prelude::*;

pub fn tick() {}

pub fn wire(app: &mut App) {
    app.add_systems(Update, tick);
    app.add_systems(PostUpdate, tick);
    app.add_systems(Last, (tick, tick.after(tick)));
}
"#;
    let (rows, line) = probe_rows("br-count-symbols", &lib(source));
    let systems = rows.iter().filter(|row| row["word"] == "system").count();
    assert_eq!(systems, 4, "{rows:#?}");
    let counts = [("plugin", 1), ("system", 1)];
    assert_eq!(line, line_of(PROBE_SLUG, &counts, [4, 0, 0], 0));

    let scratch = Scratch::new("br-count-bevy");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy");
    let run = index(&home, &root);
    let rows = role_rows(&db(&home, SLUG));
    let systems = rows.iter().filter(|row| row["word"] == "system").count();
    assert_eq!(systems, 9, "{rows:#?}");
    let roles = parse_roles_line(&roles_line_of_run(&run));
    assert_eq!(roles["words"]["system"], 7, "{roles}");
}

/// RD-7 (G1). M: a zero word left out. Opted in over code without a
/// framework role (code-index: a library; verify-ecs: an ECS-style crate
/// of its own `App`, ADR-0008's second project), the line lists every
/// word at 0, byte order, and JSON `code.roles.words` holds every word.
#[test]
fn role_counts_list_every_word() {
    for (fixture, slug, roots) in [
        ("code-index", "inkwell", "[\"crates\", \"data\"]"),
        ("verify-ecs", "verify-ecs", "[\"crates\", \"assets\"]"),
    ] {
        let scratch = Scratch::new("br-every-word");
        let home = scratch.home("h");
        let root = code_index_common::copy(&scratch, fixture, fixture);
        with_code_table(
            &root,
            &format!("[code]\nroots = {roots}\nframeworks = [\"bevy\"]\n"),
        );
        let run = spec(&home, &root, &["index"]);
        run.code(0);
        assert_eq!(
            roles_line_of_run(&run),
            line_of(slug, &[], [0, 0, 0], 0),
            "{fixture}"
        );
        let json = index_json(&home, &root);
        let words = json["code"]["roles"]["words"]
            .as_object()
            .unwrap_or_else(|| panic!("{fixture}: {json}"));
        assert_eq!(
            words.keys().map(String::as_str).collect::<Vec<_>>(),
            WORDS.to_vec(),
            "{fixture}"
        );
        assert_eq!(count(&db(&home, slug), "code_roles"), 0, "{fixture}");
    }
}

/// RD-7, TE-6 (J1). M: `code.roles` not `null` without the opt-in.
/// code-index, code-a and verify-ecs (no `frameworks`): no `roles` line,
/// the text 6b-1's three lines, JSON `code.roles` present and `null`;
/// spec-c (no `[code]`): JSON `code` `null` as before.
#[test]
fn index_roles_are_null_without_frameworks() {
    for (fixture, slug) in [
        ("code-index", "inkwell"),
        ("code-a", "code-a"),
        ("verify-ecs", "verify-ecs"),
    ] {
        let scratch = Scratch::new("br-null");
        let home = scratch.home("h");
        let root = code_index_common::copy(&scratch, fixture, fixture);
        let run = spec(&home, &root, &["index"]);
        run.code(0);
        assert!(roles_line(&run).is_none(), "{fixture}: {}", run.show());
        assert_eq!(run.stdout.lines().count(), 3, "{fixture}: {}", run.show());
        let json = index_json(&home, &root);
        let code = json["code"]
            .as_object()
            .unwrap_or_else(|| panic!("{fixture}: {json}"));
        assert!(
            code.contains_key("roles") && code["roles"].is_null(),
            "{fixture}: {json}"
        );
        assert_eq!(count(&db(&home, slug), "code_roles"), 0, "{fixture}");
    }
    let scratch = Scratch::new("br-null-spec-c");
    let home = scratch.home("h");
    let root = scratch.copy("spec-c", "spec-c");
    let run = spec(&home, &root, &["index"]);
    run.code(0);
    assert!(roles_line(&run).is_none(), "{}", run.show());
    assert_eq!(run.stdout.lines().count(), 2, "{}", run.show());
    let json = index_json(&home, &root);
    assert!(json["code"].is_null(), "{json}");
}
