//! docs/features/hash-levels.md (5a-1), the rust-developer criteria by the
//! test names the spec gives them: RD-1 (levels and the body field), RD-2
//! (broken items), RD-3 (v2 untouched, v3 frozen), RD-4 (file module and
//! N5), RD-6 (fingerprints), RD-8 (genre of the level sources), RD-9 (entry
//! points). RD-1's grammar pin is `crates/specengine-eval/tests/hash_levels_grammar.rs`;
//! RD-7 is `hash_levels_depth.rs`. Inputs are inline sources; the fixture
//! `fixtures/hash-levels/` is read where a committed rustfmt variant is the
//! subject.

mod levels_common;

use levels_common::{
    ALL, Levels, STABLE, Snapshot, changed, crlf, deps_after_header, digest_after_header,
    digest_by_hand, index_named, item_node, module, new_parser, one, parse, put, read,
    read_variant, sig_stream_by_hand, snapshot, targets_of,
};
use specengine_code::grammar::{TREE_SITTER_RUST_VERSION, grammar_info};
use specengine_code::hash::{ErrorCategory, HashState, RECIPE, error_categories, recipe_header};
use specengine_code::{
    Binding, LEVEL_RECIPE, LevelState, TRIVIAL_BODY_BYTES, analyze_tree, bind_tree, body_nodes,
    hash_bound_items, item_levels, level_header, module_levels,
};

/// The levels of item `kind label` before and after replacing `from` by `to`
/// (exactly once) in `source`; the item is found by `label` before and
/// `label_after` after.
fn edited(
    source: &str,
    kind: &str,
    labels: (&str, &str),
    from: &str,
    to: &str,
) -> (Levels, Levels, Snapshot, Snapshot) {
    assert_eq!(
        source.matches(from).count(),
        1,
        "{from:?} once in {source:?}"
    );
    let before = snapshot(source);
    let after = snapshot(&source.replacen(from, to, 1));
    (
        one(&before, kind, labels.0),
        one(&after, kind, labels.1),
        before,
        after,
    )
}

fn v2_of(snap: &Snapshot, kind: &str, label: &str) -> HashState {
    snap.v2[index_named(snap, kind, label)].clone()
}

fn refused(snap: &Snapshot, kind: &str, label: &str) -> Vec<ErrorCategory> {
    match &snap.units[index_named(snap, kind, label)] {
        levels_common::Unit::Refused(categories) => categories.clone(),
        levels_common::Unit::Hashed(levels) => {
            panic!("{kind} {label} is broken, yet hashed: {levels:?}")
        }
    }
}

// =================================================================== RD-1

#[test]
fn rename_keeps_sig_body_and_deps() {
    let cases = [
        (
            "function_item",
            "/// Doc.\n#[inline]\npub fn old_name(x: u8) -> u8 { helper(x).max(1) }\n",
            "old_name",
            "new_name_longer",
        ),
        (
            "struct_item",
            "#[derive(Debug)]\npub struct OldName { a: u8, b: [u8; size()] }\n",
            "OldName",
            "Renamed",
        ),
        (
            "macro_definition",
            "macro_rules! old_name { ($x:expr) => { make($x) }; }\n",
            "old_name",
            "renamed",
        ),
        (
            "const_item",
            "pub const OLD: u32 = limit(3);\n",
            "OLD",
            "RENAMED",
        ),
    ];
    for (kind, source, old, new) in cases {
        let (a, b, before, after) = edited(source, kind, (old, new), old, new);
        assert!(
            changed(&a, &b, &["sig", "body", "deps"]).is_empty(),
            "{kind}: a rename changed {:?}",
            changed(&a, &b, &ALL)
        );
        assert_eq!(v2_of(&before, kind, old), v2_of(&after, kind, new));
    }
}

#[test]
fn body_edit_leaves_sig() {
    let cases = [
        (
            "function_item",
            "pub fn f(x: u8) -> u8 { x + 1 }\n",
            "f",
            "x + 1",
            "x + 2",
        ),
        (
            "struct_item",
            "#[derive(Clone)]\npub struct S { a: u8 }\n",
            "S",
            "a: u8",
            "a: u16",
        ),
        (
            "struct_item",
            "pub struct P(u8, u8);\n",
            "P",
            "(u8, u8)",
            "(u8, u16)",
        ),
        ("enum_item", "pub enum E { A, B }\n", "E", "A, B", "A, B, C"),
        (
            "union_item",
            "pub union U { a: u8 }\n",
            "U",
            "a: u8",
            "a: i8",
        ),
        (
            "trait_item",
            "pub trait T: Clone { fn f(&self) -> u8 { 1 } }\n",
            "T",
            "{ 1 }",
            "{ 2 }",
        ),
        (
            "impl_item",
            "impl T for S { fn f(&self) -> u8 { 1 } }\n",
            "<S as T>",
            "{ 1 }",
            "{ 2 }",
        ),
        (
            "mod_item",
            "pub mod m { fn f() -> u8 { 1 } }\n",
            "m",
            "{ 1 }",
            "{ 2 }",
        ),
        (
            "type_item",
            "pub type A = Vec<u8>;\n",
            "A",
            "Vec<u8>",
            "Vec<u16>",
        ),
        (
            "macro_definition",
            "#[macro_export]\nmacro_rules! m { ($x:expr) => { $x * 2 }; }\n",
            "m",
            "$x * 2",
            "$x * 3",
        ),
    ];
    for (kind, source, label, from, to) in cases {
        let (a, b, ..) = edited(source, kind, (label, label), from, to);
        assert_eq!(a.sig, b.sig, "{kind}: a body edit moved sig");
        assert_ne!(a.body, b.body, "{kind}: the body edit is seen by body");
    }
}

#[test]
fn signature_edit_leaves_body() {
    let cases = [
        (
            "function_item",
            "pub fn f(x: u8) -> u8 { x + 1 }\n",
            "f",
            "(x: u8)",
            "(x: u16)",
        ),
        (
            "function_item",
            "pub fn f(x: u8) -> u8 { x + 1 }\n",
            "f",
            "-> u8",
            "-> u16",
        ),
        (
            "function_item",
            "pub fn f(x: u8) -> u8 { x + 1 }\n",
            "f",
            "pub fn",
            "#[must_use]\npub fn",
        ),
        (
            "function_item",
            "pub fn f<T: Copy>(x: T) -> T { x }\n",
            "f",
            "T: Copy",
            "T: Clone",
        ),
        (
            "struct_item",
            "pub struct S<T> where T: Copy { a: T }\n",
            "S",
            "T: Copy",
            "T: Clone",
        ),
        (
            "trait_item",
            "pub trait T: Clone { fn f(&self); }\n",
            "T",
            ": Clone",
            ": Copy",
        ),
        (
            "impl_item",
            "impl T for S { fn f(&self) {} }\n",
            "<S as T>",
            "impl T",
            "unsafe impl T",
        ),
        (
            "mod_item",
            "pub mod m { fn f() {} }\n",
            "m",
            "pub mod",
            "pub(crate) mod",
        ),
        ("type_item", "pub type A = u8;\n", "A", "pub type", "type"),
        (
            "macro_definition",
            "macro_rules! m { () => { 1 }; }\n",
            "m",
            "macro_rules! m",
            "#[macro_export]\nmacro_rules! m",
        ),
    ];
    for (kind, source, label, from, to) in cases {
        let (a, b, ..) = edited(source, kind, (label, label), from, to);
        assert_ne!(
            a.sig, b.sig,
            "{kind}: {from:?} -> {to:?} is a signature edit"
        );
        assert_eq!(a.body, b.body, "{kind}: a signature edit moved body");
        assert_eq!(a.deps, b.deps, "{kind}: a signature edit moved deps");
    }
    // A signature item has no body: every edit is a sig edit.
    let (a, b, ..) = edited(
        "trait T { fn f(&self) -> u8; }\n",
        "function_signature_item",
        ("f", "f"),
        "-> u8",
        "-> u16",
    );
    assert_ne!(a.sig, b.sig);
    assert_eq!((a.body, a.deps), (b.body, b.deps));
}

#[test]
fn const_value_edit_leaves_sig() {
    for (kind, source, label, value_from, value_to, type_from, type_to) in [
        (
            "const_item",
            "pub const LIMIT: u32 = 100;\n",
            "LIMIT",
            "= 100",
            "= 200",
            ": u32",
            ": u64",
        ),
        (
            "static_item",
            "static NAME: &str = \"a\";\n",
            "NAME",
            "\"a\"",
            "\"b\"",
            "static NAME",
            "static mut NAME",
        ),
    ] {
        let (a, b, ..) = edited(source, kind, (label, label), value_from, value_to);
        assert_eq!(a.sig, b.sig, "{kind}: the value walked into sig");
        assert_ne!(a.body, b.body, "{kind}: the value is the body");
        let (a, b, ..) = edited(source, kind, (label, label), type_from, type_to);
        assert_ne!(a.sig, b.sig, "{kind}: the type and `mut` are sig");
        assert_eq!(a.body, b.body, "{kind}: the type walked into body");
    }
}

#[test]
fn bodiless_impl_and_static_hash_an_empty_body() {
    let source = "pub trait T {}\npub struct X;\nimpl T for X;\nimpl X where X: Sized;\n\
                  static S: u8;\nconst C: u16;\nmod m;\ntrait U { fn f(&self); }\n";
    let tree = parse(&mut new_parser(), source);
    assert!(
        !tree.root_node().has_error(),
        "grammar-only forms parse cleanly"
    );
    let snap = snapshot(source);
    let empty_body = digest_after_header("body", &[]);
    let empty_deps = digest_after_header("deps", &[]);
    let analysis = analyze_tree(&tree, source);
    for (kind, label) in [
        ("impl_item", "<X as T>"),
        ("impl_item", "X"),
        ("static_item", "S"),
        ("const_item", "C"),
        ("mod_item", "m"),
        ("struct_item", "X"),
        ("function_signature_item", "f"),
    ] {
        let levels = one(&snap, kind, label);
        assert_eq!(
            *levels.body.as_bytes(),
            empty_body,
            "{kind} {label}: no body, the empty body stream"
        );
        assert_eq!(*levels.deps.as_bytes(), empty_deps, "{kind} {label}");
        assert_eq!(levels.body_bytes, 0, "{kind} {label}");
        let record = &analysis.items[index_named(&snap, kind, label)];
        let node = item_node(tree.root_node(), &record.byte_range, kind);
        assert!(body_nodes(node).is_empty(), "{kind} {label}: no body node");
    }
    // Their signatures still tell them apart.
    let a = one(&snap, "impl_item", "<X as T>");
    let b = one(&snap, "impl_item", "X");
    assert_ne!(a.sig, b.sig);
    // A body given back is seen.
    let (a, b, ..) = edited(
        source,
        "static_item",
        ("S", "S"),
        "static S: u8;",
        "static S: u8 = 0;",
    );
    assert_ne!(a.body, b.body);
    assert_eq!(*a.body.as_bytes(), empty_body);
}

#[test]
fn call_order_swap_keeps_deps() {
    let (a, b, ..) = edited(
        "fn f() { load(); start(); x.run(); }\n",
        "function_item",
        ("f", "f"),
        "load(); start(); x.run();",
        "x.run(); start(); load();",
    );
    assert_eq!(
        a.deps, b.deps,
        "targets are byte-sorted, not in source order"
    );
    assert_ne!(a.body, b.body);
    assert_eq!(
        targets_of(
            "fn f() { load(); start(); x.run(); }\n",
            "function_item",
            "f"
        ),
        [".run", "load", "start"]
    );
    // Deduplicated: a repeated call is one target.
    let (a, b, ..) = edited(
        "fn f() { load(); }\n",
        "function_item",
        ("f", "f"),
        "load();",
        "load(); load();",
    );
    assert_eq!(a.deps, b.deps, "targets are deduplicated");
    assert_eq!(*a.deps.as_bytes(), deps_after_header(&["load"]));
}

#[test]
fn deps_ignore_calls_in_the_signature() {
    let source = "#[cfg(feature = \"x\")]\npub fn f(data: [u8; capacity()]) -> [u8; size()] { data.map(step) }\n";
    assert_eq!(targets_of(source, "function_item", "f"), [".map"]);
    let (a, b, ..) = edited(source, "function_item", ("f", "f"), "capacity()", "limit()");
    assert_eq!(a.deps, b.deps, "a call in a parameter type is no dep");
    assert_ne!(a.sig, b.sig, "it is part of sig");
    assert_eq!(*a.deps.as_bytes(), deps_after_header(&[".map"]));
}

#[test]
fn calls_inside_macro_arguments_are_not_deps() {
    let source =
        "fn f(x: u8) { println!(\"{}\", show(x)); let v = vec![make(x)]; assert!(check(v)); }\n";
    assert_eq!(
        targets_of(source, "function_item", "f"),
        ["assert!", "println!", "vec!"]
    );
    let (a, b, ..) = edited(source, "function_item", ("f", "f"), "show(x)", "render(x)");
    assert_eq!(
        a.deps, b.deps,
        "a call inside a macro's token tree is invisible"
    );
    assert_ne!(a.body, b.body);
}

#[test]
fn crlf_multiline_literal_keeps_every_level() {
    let source = "pub fn banner() -> &'static str { \"ARENA\nGO\" }\n\
                  pub const RAW: &str = r\"a\nb\";\n\
                  pub static BYTES: &[u8] = b\"x\ny\";\n\
                  /// Doc\n/// lines.\npub fn plain() -> u8 {\n    1\n}\n";
    let lf = snapshot(source);
    let cr = snapshot(&crlf(source));
    for (kind, label) in [
        ("function_item", "banner"),
        ("const_item", "RAW"),
        ("static_item", "BYTES"),
        ("function_item", "plain"),
    ] {
        let (a, b) = (one(&lf, kind, label), one(&cr, kind, label));
        assert!(
            changed(&a, &b, &ALL).is_empty(),
            "{kind} {label}: CRLF changed {:?}",
            changed(&a, &b, &ALL)
        );
    }
    assert_eq!(module(&lf), module(&cr), "the file module too");
    assert_ne!(
        v2_of(&lf, "function_item", "banner"),
        v2_of(&cr, "function_item", "banner"),
        "v2 reads the CR (the named exception 5b's drop of ast_hash relies on)"
    );
    // Only a CR right before an LF is dropped: a lone CR is text.
    let (a, b, ..) = edited(
        "fn f() -> &'static str { \"ab\" }\n",
        "function_item",
        ("f", "f"),
        "\"ab\"",
        "\"a\rb\"",
    );
    assert_ne!(a.body, b.body, "a lone CR is kept");
}

// =================================================================== RD-2

#[test]
fn item_with_a_body_error_has_no_level_hash() {
    let source = "fn good() -> u8 { 1 }\nfn bad(x: u8) -> u8 { let y = 1 2; x }\n\
                  #[derive(Debug,]\nstruct Torn;\n";
    let snap = snapshot(source);
    one(&snap, "function_item", "good");
    let categories = refused(&snap, "function_item", "bad");
    assert!(categories.contains(&ErrorCategory::Other), "{categories:?}");
    match v2_of(&snap, "function_item", "bad") {
        HashState::CannotVerify { categories: v2 } => assert_eq!(categories, v2, "v2's categories"),
        other => panic!("v2 hashed a broken item: {other:?}"),
    }
    assert!(categories.is_sorted());
    // An error in the attached attribute alone refuses the item.
    assert!(!refused(&snap, "struct_item", "Torn").is_empty());
    // The control: the same function without the error has every level.
    let fixed = snapshot(&source.replace("1 2;", "1;"));
    one(&fixed, "function_item", "bad");
}

#[test]
fn broken_items_never_share_a_level_hash() {
    let source = "pub fn broken_one() {\n    let x = 1 2;\n}\n\npub fn broken_two() {\n    let x = 1 2 3;\n}\n";
    let snap = snapshot(source);
    for label in ["broken_one", "broken_two"] {
        let categories = refused(&snap, "function_item", label);
        assert_eq!(categories, [ErrorCategory::Other], "{label}");
    }
    assert!(
        snap.module.levels().is_none(),
        "the file's module refuses too"
    );
    // A digest only for a hashed unit: every refusal carries none.
    for unit in &snap.units {
        assert!(unit.levels().is_none(), "{unit:?}");
    }
}

// =================================================================== RD-3

#[test]
fn level_header_is_v3_and_item_hash_is_v2() {
    let mut header = Vec::new();
    put(&mut header, b"specengine-hash/v3");
    put(&mut header, TREE_SITTER_RUST_VERSION.as_bytes());
    put(&mut header, grammar_info().abi.to_string().as_bytes());
    assert_eq!(level_header(), header.as_slice());
    assert_eq!(LEVEL_RECIPE, "specengine-hash/v3");
    assert_eq!(RECIPE, "specengine-hash/v2");
    let mut v2_header = Vec::new();
    put(&mut v2_header, RECIPE.as_bytes());
    assert!(
        recipe_header().starts_with(&v2_header),
        "v2's header untouched"
    );
    assert_ne!(level_header(), recipe_header(), "one header shared");
    // The item hash is still v2's: its stream under the v2 header; sig is
    // under the v3 header and its own tag.
    let source = "#[inline]\npub fn f(x: u8) -> u8 { x }\n";
    let tree = parse(&mut new_parser(), source);
    let analysis = analyze_tree(&tree, source);
    let node = item_node(
        tree.root_node(),
        &analysis.items[0].byte_range,
        "function_item",
    );
    let mut v2_stream = recipe_header().to_vec();
    for attribute in specengine_code::hash::attached_attributes(node) {
        assert!(specengine_code::hash::normalize(
            attribute,
            source.as_bytes(),
            None,
            &mut v2_stream
        ));
    }
    assert!(specengine_code::hash::normalize(
        node,
        source.as_bytes(),
        specengine_code::hash::name_node(node),
        &mut v2_stream
    ));
    let v2 = analysis.items[0].state.digest().expect("hashed by v2");
    assert_eq!(*v2.as_bytes(), *blake3::hash(&v2_stream).as_bytes());
    let snap = snapshot(source);
    let levels = one(&snap, "function_item", "f");
    assert_eq!(
        *levels.sig.as_bytes(),
        digest_by_hand("sig", &sig_stream_by_hand(node, source))
    );
    assert_ne!(*levels.sig.as_bytes(), *v2.as_bytes());
}

/// One input per kind (every `ITEM_KINDS` and `MEMBER_KINDS` kind) and the
/// file module, with N4 and N5 runs (each N5 run out of its sorted order,
/// and sorted unlike by name: `put` prefixes the length), a `#[macro_use]`
/// member and a CRLF literal; every level and fingerprint pinned. A change here is a
/// `LEVEL_RECIPE` bump and a `spec lock rebase` of every lock (ADR-0050).
///
/// Kept out until iteration 2 settles them (review 1, H1-H4): a call whose
/// function is not a path, a shebang, `cfg_attr(.., macro_use)` (none of
/// them in this source) and the module's `quote` (not pinned).
const PINNED_SOURCE: &str = concat!(
    "//! Pinned input of `level_streams_are_pinned`: one item per kind.\n",
    "#![allow(dead_code)]\n",
    "extern crate core;\n",
    "extern crate cell;\n",
    "extern crate alloc;\n",
    "mod zeta;\n",
    "mod beta;\n",
    "mod alpha;\n",
    "#[macro_use]\n",
    "mod gamma;\n",
    "\n",
    "use std::fmt;\n",
    "use std::collections::BTreeMap;\n",
    "\n",
    "/// A function.\n",
    "#[inline]\n",
    "pub fn tally(base: u32, bonus: u32) -> u32 { let total = base + bonus; clamp(total).max(Vec::<u32>::new().len() as u32) }\n",
    "\n",
    "#[derive(Debug, Clone)]\n",
    "pub struct Stamina { pub current: u32, pub max: [u8; slots()] }\n",
    "\n",
    "pub enum Phase { Idle, Running(u8), Done { at: u32 } }\n",
    "\n",
    "#[repr(C)]\n",
    "pub union Bits { int: u32, float: f32 }\n",
    "\n",
    "pub trait Shape: fmt::Debug {\n",
    "    fn area(&self) -> f32;\n",
    "    fn scaled(&self, k: f32) -> f32 { self.area() * k }\n",
    "}\n",
    "\n",
    "impl Shape for Stamina {\n",
    "    fn area(&self) -> f32 { <f32 as From<u8>>::from(1) }\n",
    "}\n",
    "\n",
    "pub const LIMIT: usize = compute(4);\n",
    "static GREETING: &str = \"hello\r\nworld\";\n",
    "pub type Grid = Vec<BTreeMap<u8, u8>>;\n",
    "\n",
    "pub mod scoring {\n",
    "    #![allow(unused)]\n",
    "    mod zeta;\n",
    "    mod alpha;\n",
    "    pub fn bonus() -> u32 { base() + 5 }\n",
    "}\n",
    "\n",
    "macro_rules! square { ($x:expr) => { $x * $x }; }\n",
);

/// Every unit of `snap`, one line per level and fingerprint.
fn pinned_lines(snap: &Snapshot) -> String {
    let mut out = String::new();
    let units = snap
        .keys
        .iter()
        .map(ToString::to_string)
        .zip(&snap.units)
        .chain(std::iter::once(("module".to_owned(), &snap.module)));
    for (name, unit) in units {
        let levels = unit
            .levels()
            .unwrap_or_else(|| panic!("{name} is refused: {unit:?}"));
        for field in ["sig", "body", "deps", "skeleton", "quote"] {
            if name == "module" && field == "quote" {
                // Not pinned until iteration 2 fixes its range (H3,
                // `module_quote_trims_whitespace_at_both_ends`).
                continue;
            }
            let digest = match field {
                "sig" => levels.sig,
                "body" => levels.body,
                "deps" => levels.deps,
                "skeleton" => levels.skeleton,
                _ => levels.quote,
            };
            out.push_str(&format!("{name} | {field} {digest}\n"));
        }
        out.push_str(&format!("{name} | body_bytes {}\n", levels.body_bytes));
    }
    out
}

/// Golden bytes of every level and fingerprint over `PINNED_SOURCE`
/// (`specengine-hash/v3`, tree-sitter-rust 0.24.2), one line each.
const PINNED: &str = "\
mod_item zeta | sig 4db09565adb7bf3fee1f0a1e47a2ddd614cbe634ecf626847d1a06730ff761c3
mod_item zeta | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
mod_item zeta | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
mod_item zeta | skeleton 2f1189e5eec59cdacf34e82151d408f2b6364be9f244c09ab40f6aea4e74d5f2
mod_item zeta | quote caa459188b4f529ab8a43e3798224f64e6d3caa5407f7f3e1821c5560cd4446e
mod_item zeta | body_bytes 0
mod_item beta | sig 4db09565adb7bf3fee1f0a1e47a2ddd614cbe634ecf626847d1a06730ff761c3
mod_item beta | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
mod_item beta | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
mod_item beta | skeleton 2f1189e5eec59cdacf34e82151d408f2b6364be9f244c09ab40f6aea4e74d5f2
mod_item beta | quote 00dd396ea52336902f1c4d349952709d4c33274f755eae076d62e61ed50e996e
mod_item beta | body_bytes 0
mod_item alpha | sig 4db09565adb7bf3fee1f0a1e47a2ddd614cbe634ecf626847d1a06730ff761c3
mod_item alpha | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
mod_item alpha | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
mod_item alpha | skeleton 2f1189e5eec59cdacf34e82151d408f2b6364be9f244c09ab40f6aea4e74d5f2
mod_item alpha | quote 3f5db0f399d3a8d2a4af4dc4421f5927b6e528b7cf930de99b93873636932a84
mod_item alpha | body_bytes 0
mod_item gamma | sig 33e5eaa9306213ddecb904aa1e8f3e73fc9d81d98b86c63e9aa97e5c0f84af41
mod_item gamma | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
mod_item gamma | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
mod_item gamma | skeleton 130af2d33d5993b9e81302abb0541e0f2c65ad5bf852834e56920674fa4c2698
mod_item gamma | quote ba85afc7772b35b4f14c43bde7f7429eb0e27ac01ab746d060998ba387cac098
mod_item gamma | body_bytes 0
function_item tally | sig 57db1739715aee8d507b8cf7e2c8a880329e886bce454190ad3b9cb20b70470a
function_item tally | body af891ba8fb8a6e4284f0ce06b77dbbcb56840190e0b9b75dee4e9db5b792f0a1
function_item tally | deps 4e69ee9afc54ca5d6119afd68f89423cb70ae65f277cfe55018d6d586d2bba73
function_item tally | skeleton 480b0a92245f46ca19801a2315cff249bd7a33283196974a30eb019529633dc2
function_item tally | quote 44aa256224ad1587b0f8b06889a6e242fbc6225f19586465b966b66eb2c692f8
function_item tally | body_bytes 68
struct_item Stamina | sig 8f027e657b7656402b190b6e7ff1504c27ccd0b42baac1c76e7419038072f837
struct_item Stamina | body 8f3bdf842bf66e35dfb50e8f0dbff62d6c8ad44b6d5c83e55679a0604addfcee
struct_item Stamina | deps 082bd8fcd5802bca009cb539a30893db6adf8325d4b55b2e92c52a3c1b01cfa3
struct_item Stamina | skeleton bbd8dbd6eb20e14f71236d638bd02b87df08a97eb027f0a9af4d0cbd47ff9150
struct_item Stamina | quote f73eae75c31b54f7f54da4b613bac4f1656ab214050bcfd77681443a38c0535b
struct_item Stamina | body_bytes 35
enum_item Phase | sig 340bd53b3bdf584c5ff1283509bae75bb203202a59ae7dab4aa25962e5492774
enum_item Phase | body ae4ca1a924a892e12030cbf8f1610e20a273898b13142dfe455336378777422d
enum_item Phase | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
enum_item Phase | skeleton 5fe507cd7606cfe332169e43ff3ecedb2f8013e894043608102125d0ce179246
enum_item Phase | quote f8f0f38f392fd2a5317bd768bc3f024ff7d65df8e9d2d3f8fe24d13ef32a5872
enum_item Phase | body_bytes 29
union_item Bits | sig e7d2b6b991b166a85e7f7d207953ba678fb5c689343c1ca1e3d8a53d14ecf910
union_item Bits | body 6af4f7746cfe87d769dfd2ac44e7d0fac527590e6667de10ea35716d6196da4d
union_item Bits | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
union_item Bits | skeleton 41b077824f8812d84528c33653ed0ff0c832f0f6a8d28d1cecedbb047ca22f13
union_item Bits | quote e45e5633bc4315cedfbc145c1207dee957601621d670ec5818b0f4e31cfc382f
union_item Bits | body_bytes 18
trait_item Shape | sig b96b577edf31abceca495cf5a105e47814936c52a0c29696287d50484841917b
trait_item Shape | body 3a6df69db6635405e15e716b79048c5e66ee6daba0ab2d327e1895eaec4ab2ce
trait_item Shape | deps 145052391afe5a7b5f486abed8c572455e2f09af11cd17b2f70773c8fc2ef6cc
trait_item Shape | skeleton ddea7679be278ee29c4fa179329b9de6c1158845962791e5c551b95c8c0036e4
trait_item Shape | quote dc3c034464c2f31ede8b15d3aa0c79b6cc5ec8f450d9e4baba1e6d4aab645150
trait_item Shape | body_bytes 61
function_signature_item area (in Shape) | sig cde343cb80e5f0dd357555a9ede31e6f7ee253c5e85b7c2e8a0e5fc07d9d1cb2
function_signature_item area (in Shape) | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
function_signature_item area (in Shape) | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
function_signature_item area (in Shape) | skeleton 80297b12443df76728488381fb1bde8b8e3930d1e9c0b694cf1982a2c5c0bec5
function_signature_item area (in Shape) | quote d201f5bc6c77a065da939b7911e5086ce9b1ae0995dc2c534673b2c8270dfc87
function_signature_item area (in Shape) | body_bytes 0
function_item scaled (in Shape) | sig d1f5ce0f5ebab1be184e611c21f682c6703fb51a30ece46cd98ed45583690130
function_item scaled (in Shape) | body d28fb6910794595df572534e12b6cc62f77304ad0215ad78c392957d46ac81db
function_item scaled (in Shape) | deps 145052391afe5a7b5f486abed8c572455e2f09af11cd17b2f70773c8fc2ef6cc
function_item scaled (in Shape) | skeleton 92c6dc0494d85d02b66000a8b62e928415e7d6c0d8722acfa948e45e64aae10b
function_item scaled (in Shape) | quote 45a272d438867a6b870da1b3980292d8355976e42a67d31dcc1d32cc67a5f6df
function_item scaled (in Shape) | body_bytes 15
impl_item <Stamina as Shape> | sig ec3a85dcdd188f55152b839a923879baaef7f9b90e5c46e4a6216e5e23f8fdc0
impl_item <Stamina as Shape> | body f55e9bfd3fd585a416791578b821556644274b993683ddaad8c7e21749c133ac
impl_item <Stamina as Shape> | deps 885fd03ba1e860382e505115c3b6b5b8eaa6b5f61043e7e9b887a21b60ae3802
impl_item <Stamina as Shape> | skeleton 0a98ca72837d3ac08386705ff2abb3a47acb36cfd9bb7b2118e70dc45009af95
impl_item <Stamina as Shape> | quote 9c0dbf4a7e20352234ad5590f5a8b3f5cf623bf98e9f8c8224bc94cef1e3cd81
impl_item <Stamina as Shape> | body_bytes 46
function_item area (in <Stamina as Shape>) | sig 713614ced8c9cdd7fae1170a671f32be148ecc47ed805241fdbb02fcd26b5453
function_item area (in <Stamina as Shape>) | body 1c1f2b43b45f8c8f42cd5f0720bddcd60992f13eb36a2a86ad5cefd0d8f782e7
function_item area (in <Stamina as Shape>) | deps 885fd03ba1e860382e505115c3b6b5b8eaa6b5f61043e7e9b887a21b60ae3802
function_item area (in <Stamina as Shape>) | skeleton 876e4a36c0fed9104bbaf85c3a66f085c19eb6df9cccab98015751ace982ac5d
function_item area (in <Stamina as Shape>) | quote a41541c4a00bfcad7b5364409dc580dc2aff0d3668d697db3007d382dd7822a3
function_item area (in <Stamina as Shape>) | body_bytes 26
const_item LIMIT | sig 6bb3f109be5bf1801b6b721db66c8a4809db4a54474c0b24eb2c8048701e43d2
const_item LIMIT | body 22769fd3e9bf41e6cfbdc74811dc483d5e72337769c7ec48a6fd1db1164ca57a
const_item LIMIT | deps ff3b6cda735ea4f968a9032f66fda2b342b010f6b1b62b78140d896b881f0494
const_item LIMIT | skeleton b630f66f3211ff76b97c64b387f2011e79d6c131e618fa989a1d1ed57e6e4a04
const_item LIMIT | quote 61b16dcca8380095905c758e9fab3819bd82f0727c5799b8116bab5de2b78558
const_item LIMIT | body_bytes 10
static_item GREETING | sig 62bd144a1637bc2a48c7ddca61c17ce226867d6c84e19cc887e0d67dfc7e1a17
static_item GREETING | body 08b739a3156c3c479827e23c9f9cfc8b4eff59b45c583eb33681baf37aeab0af
static_item GREETING | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
static_item GREETING | skeleton 039dbdaa078d02a4532a410b5e60a365509b337f5504149d3903b3e60754da8c
static_item GREETING | quote 95b86cb6982f822bc7573c4327d59205bedb86dc6133554771e1d7cf915a5b19
static_item GREETING | body_bytes 13
type_item Grid | sig 32eecdb330d098e56f65686c83a0b3a236ee4097ef7285579a61e0c1ec989c63
type_item Grid | body af8362dd88fcaec9a3b1217085fa46c6657713556ed82047b86ea8eb3caeeae0
type_item Grid | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
type_item Grid | skeleton f5d12b4bc56e609f4639c08ee36c6c727b318bf82af4709b9248cec8cdc665ac
type_item Grid | quote 05878f89d421189736202aeaccc3a93d2cadce23babe8fd2fffbab22bc252de6
type_item Grid | body_bytes 19
mod_item scoring | sig 9a401400eb97ecc6984e1db261f90db8fe6b03b98202ecce9c70bf34ccacee5a
mod_item scoring | body 1e670dda3a823404552dd088e1d64fa7ca09ce22e7c451a751a3bf4c86dd2a8f
mod_item scoring | deps 5f6d5a5c5c4e44c8b666fc1a48f219ca871a13d098297a46007707971aac314f
mod_item scoring | skeleton 97429a0c33eecdbec7eb154965ae9ebecebca50ce05f82007e288acbceab5a47
mod_item scoring | quote 3dd0b2ae8ff24209209fb9c8fa19a63b520f80638bb07b8a5e7f69501a497395
mod_item scoring | body_bytes 63
mod_item zeta (mod scoring) | sig 4db09565adb7bf3fee1f0a1e47a2ddd614cbe634ecf626847d1a06730ff761c3
mod_item zeta (mod scoring) | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
mod_item zeta (mod scoring) | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
mod_item zeta (mod scoring) | skeleton 2f1189e5eec59cdacf34e82151d408f2b6364be9f244c09ab40f6aea4e74d5f2
mod_item zeta (mod scoring) | quote 049120c3c609b9965582b67ddb025993940ef5409b6a0a16b9f3d5d50d6d99f1
mod_item zeta (mod scoring) | body_bytes 0
mod_item alpha (mod scoring) | sig 4db09565adb7bf3fee1f0a1e47a2ddd614cbe634ecf626847d1a06730ff761c3
mod_item alpha (mod scoring) | body 0f2739e03055e763887cfbc5f28ebe08f9e088a20f9fcf73e6cdab54b5580ba5
mod_item alpha (mod scoring) | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
mod_item alpha (mod scoring) | skeleton 2f1189e5eec59cdacf34e82151d408f2b6364be9f244c09ab40f6aea4e74d5f2
mod_item alpha (mod scoring) | quote 37a3ec23384f8c82e1624d164ea739b219d1049611e8b5c333316774973eec18
mod_item alpha (mod scoring) | body_bytes 0
function_item bonus (mod scoring) | sig ccdd1b4b9362e66eb3050f424877484c7b2beb7ea41ebabcd27d2ef401145627
function_item bonus (mod scoring) | body 1a50617b2959a2179b0a8ad49686284c9a101ab49e7ce944320803380db21376
function_item bonus (mod scoring) | deps 5f6d5a5c5c4e44c8b666fc1a48f219ca871a13d098297a46007707971aac314f
function_item bonus (mod scoring) | skeleton 26506a3c8bbe13250248fb69a978799995a3766426945aa7aa5da8916ceb0193
function_item bonus (mod scoring) | quote ddc7bec3e75f6c910afced413b9b8389ad362f192dad78c781f26b13f12d1c18
function_item bonus (mod scoring) | body_bytes 10
macro_definition square | sig 9ad2c7613d7415fe00a9baa3943e0b47941354eb54decd20c7fd9c2f0a4b2016
macro_definition square | body 4650c88d35bad4ff5e4859f964a708998d9af9332f986c187ad10351181bf6d1
macro_definition square | deps dc3905e4443a1de6f1d90f4782879e8b5b993ce64e2e13bd77fb6d298e0f9bcb
macro_definition square | skeleton b08b7b4ccab689662a163b2eb8b6bf9687003d30a59305c80eafb328bec806b7
macro_definition square | quote c5644e936cca52b958320c650ddd8ac089c619afd12361114d60ccdf6fa96bb7
macro_definition square | body_bytes 18
module | sig 8725e95d50443c006834f11bd912a35ce912bc9271b65361d6501695ac9124fa
module | body 2c34cdb69c0c512baa8df876a2893946e10dcffb70843e057aa3f4f0af45d54f
module | deps f38fbd77104ff6886e8ba60f965ac70a293be6849d791d3a30c8dc8a1d87df4d
module | skeleton 5d79c998b7dbce2541cac62cbbcd4e313320898cdf7cc31bdcc323033e4c25d4
module | body_bytes 763
";

#[test]
fn level_streams_are_pinned() {
    let snap = snapshot(PINNED_SOURCE);
    let mut kinds: Vec<&str> = snap.keys.iter().map(|k| k.kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    let mut every: Vec<&str> = specengine_code::items::ITEM_KINDS
        .iter()
        .chain(specengine_code::items::MEMBER_KINDS)
        .copied()
        .collect();
    every.sort_unstable();
    every.dedup();
    assert_eq!(kinds, every, "one input per kind");
    let actual = pinned_lines(&snap);
    let mismatched: Vec<String> = actual
        .lines()
        .zip(PINNED.lines().chain(std::iter::repeat("")))
        .filter(|(got, want)| got != want)
        .map(|(got, want)| format!("got  {got}\nwant {want}"))
        .collect();
    assert!(
        mismatched.is_empty() && actual.lines().count() == PINNED.lines().count(),
        "v3 bytes moved ({} lines differ); the whole rendering:\n{actual}\n{}",
        mismatched.len(),
        mismatched.join("\n")
    );
}

// =================================================================== RD-4

#[test]
fn reordered_mod_declarations_keep_the_module_hash() {
    let source = "//! A module.\nmod zeta;\n// a comment between\nmod alpha;\n#[cfg(test)]\nmod tests;\npub mod beta;\n\npub fn f() {}\n";
    let reordered = "//! A module.\npub mod beta;\n#[cfg(test)]\nmod tests;\nmod alpha;\n// a comment between\nmod zeta;\n\npub fn f() {}\n";
    let (a, b) = (module(&snapshot(source)), module(&snapshot(reordered)));
    assert!(
        changed(&a, &b, &STABLE).is_empty(),
        "N5: {:?}",
        changed(&a, &b, &STABLE)
    );
    // The same run inside an inline mod: its levels hold, v2's hash moves
    // (the N5 exception TE-6 names).
    let inline = "mod hub {\n    mod zeta;\n    mod alpha;\n}\n";
    let swapped = "mod hub {\n    mod alpha;\n    mod zeta;\n}\n";
    let (before, after) = (snapshot(inline), snapshot(swapped));
    let (a, b) = (
        one(&before, "mod_item", "hub"),
        one(&after, "mod_item", "hub"),
    );
    assert!(changed(&a, &b, &STABLE).is_empty());
    assert_ne!(
        v2_of(&before, "mod_item", "hub"),
        v2_of(&after, "mod_item", "hub")
    );
}

#[test]
fn macro_use_mod_order_changes_the_module_hash() {
    let source = "#[macro_use]\nmod macros;\nmod alpha;\nmod beta;\n";
    let moved = "mod alpha;\n#[macro_use]\nmod macros;\nmod beta;\n";
    let (a, b) = (module(&snapshot(source)), module(&snapshot(moved)));
    assert_ne!(a.body, b.body, "a #[macro_use] member stays in place");
    // The members around it still sort.
    let control = "#[macro_use]\nmod macros;\nmod beta;\nmod alpha;\n";
    assert_eq!(a.body, module(&snapshot(control)).body);
    // `#[macro_use(..)]` and `extern crate` alike.
    let crates = "#[macro_use(m)]\nextern crate x;\nextern crate a;\nextern crate b;\n";
    let crates_moved = "extern crate a;\n#[macro_use(m)]\nextern crate x;\nextern crate b;\n";
    assert_ne!(
        module(&snapshot(crates)).body,
        module(&snapshot(crates_moved)).body
    );
}

#[test]
fn inner_attribute_edit_is_a_module_sig_change() {
    let source = "#![allow(dead_code)]\n// between\n#![warn(missing_docs)]\n//! Doc.\n\npub fn f() -> u8 { 1 }\n";
    let base = module(&snapshot(source));
    for (from, to) in [
        ("allow(dead_code)", "allow(unused)"),
        ("warn(missing_docs)", "deny(missing_docs)"),
    ] {
        let other = module(&snapshot(&source.replace(from, to)));
        assert_ne!(base.sig, other.sig, "{from}: the file's #![..] is sig");
        assert_eq!(base.body, other.body, "{from}: not body");
        assert_eq!(base.deps, other.deps);
    }
    let other = module(&snapshot(&source.replace("{ 1 }", "{ 2 }")));
    assert_eq!(base.sig, other.sig, "an item edit is body");
    assert_ne!(base.body, other.body);
    // Without inner attributes, sig is the empty stream.
    let bare = module(&snapshot("pub fn f() -> u8 { 1 }\n"));
    assert_eq!(*bare.sig.as_bytes(), digest_after_header("sig", &[]));
    // The asymmetry: an inline mod's #![..] is its body.
    let (a, b, ..) = edited(
        "mod m {\n    #![allow(dead_code)]\n    fn f() {}\n}\n",
        "mod_item",
        ("m", "m"),
        "allow(dead_code)",
        "allow(unused)",
    );
    assert_eq!(a.sig, b.sig);
    assert_ne!(a.body, b.body);
}

#[test]
fn module_of_a_file_with_an_error_cannot_verify() {
    for source in [
        "pub fn ok() -> u8 { 1 }\npub fn bad() { let x = 1 2; }\n",
        "pub fn ok() -> u8 { 1 }\nfn missing() { let x = 1 + ; }\n",
    ] {
        let tree = parse(&mut new_parser(), source);
        assert!(tree.root_node().has_error());
        let categories = match module_levels(&tree, source) {
            LevelState::CannotVerify { categories } => categories,
            other => panic!("a file with an error elsewhere hashed: {other:?}"),
        };
        assert_eq!(
            categories,
            error_categories(tree.root_node(), source.as_bytes()),
            "{source}"
        );
        assert!(!categories.is_empty());
        one(&snapshot(source), "function_item", "ok");
    }
}

#[test]
fn reordered_extern_crates_keep_the_module_hash() {
    let source = "extern crate zeta;\n// note\nextern crate alpha as a;\npub extern crate beta;\n\nfn f() {}\n";
    let reordered = "pub extern crate beta;\nextern crate alpha as a;\n// note\nextern crate zeta;\n\nfn f() {}\n";
    let (a, b) = (module(&snapshot(source)), module(&snapshot(reordered)));
    assert!(
        changed(&a, &b, &STABLE).is_empty(),
        "{:?}",
        changed(&a, &b, &STABLE)
    );
    let inline = snapshot("mod m {\n    extern crate b;\n    extern crate a;\n}\n");
    let swapped = snapshot("mod m {\n    extern crate a;\n    extern crate b;\n}\n");
    assert!(
        changed(
            &one(&inline, "mod_item", "m"),
            &one(&swapped, "mod_item", "m"),
            &STABLE
        )
        .is_empty()
    );
}

#[test]
fn module_deps_are_every_call_in_the_file() {
    let source = "#![allow(dead_code)]\n\
                  pub const LIMIT: usize = limit();\n\
                  pub fn run(data: [u8; capacity()]) -> u8 { boot(); data[0] }\n\
                  pub struct S { cells: [u8; cells()] }\n\
                  impl S { fn m(&self) { self.tick(); } }\n\
                  static TABLE: [u8; 2] = table();\n\
                  mod inner { fn f() { inner_call(); println!(\"{}\", hidden()); } }\n";
    let want = [
        ".tick",
        "boot",
        "capacity",
        "cells",
        "inner_call",
        "limit",
        "println!",
        "table",
    ];
    let levels = module(&snapshot(source));
    assert_eq!(
        *levels.deps.as_bytes(),
        deps_after_header(&want),
        "every call in the file, the signature's `capacity` included"
    );
    // One call more anywhere, a signature included, moves the module's deps.
    let more = module(&snapshot(
        &source.replace("capacity()", "capacity() + extra()"),
    ));
    assert_ne!(levels.deps, more.deps);
}

// =================================================================== RD-6

#[test]
fn renamed_locals_keep_the_skeleton() {
    let a = snapshot("fn f(a: u8, s: Foo) -> u8 { let b = a + s.field; b }\nstruct S { a: Foo }\n");
    let b =
        snapshot("fn f(x: u16, t: Bar) -> u16 { let y = x + t.other; y }\nstruct S { b: Bar }\n");
    for kind in ["function_item", "struct_item"] {
        let label = if kind == "struct_item" { "S" } else { "f" };
        let (one_a, one_b) = (one(&a, kind, label), one(&b, kind, label));
        assert_eq!(one_a.skeleton, one_b.skeleton, "{kind}: identifiers masked");
        assert_ne!(one_a.body, one_b.body, "{kind}");
    }
}

#[test]
fn changed_literal_keeps_the_skeleton() {
    let (a, b, ..) = edited(
        "fn f() -> f64 { let n = 1 + \"ab\".len() + 'c' as usize; n as f64 * 0.5 }\n",
        "function_item",
        ("f", "f"),
        "1 + \"ab\".len() + 'c' as usize; n as f64 * 0.5",
        "12345 + \"a longer one\".len() + 'z' as usize; n as f64 * 12.75",
    );
    assert_eq!(a.skeleton, b.skeleton, "literals masked, their lengths too");
    assert_ne!(a.body, b.body);
    assert_ne!(a.body_bytes, b.body_bytes);
}

#[test]
fn copies_differ_by_quote_when_their_context_differs() {
    let source =
        "// first context\nfn a() -> u8 { 1 }\n// between\nfn b() -> u8 { 1 }\n// last context\n";
    let snap = snapshot(source);
    let (a, b) = (
        one(&snap, "function_item", "a"),
        one(&snap, "function_item", "b"),
    );
    assert_eq!(
        changed(&a, &b, &ALL),
        ["quote"],
        "copies differ by their lines around"
    );
    // The same copy in the same context elsewhere: the same quote.
    let elsewhere = snapshot("use x;\n\n// first context\nfn a() -> u8 { 1 }\n// between\n");
    assert_eq!(one(&elsewhere, "function_item", "a").quote, a.quote);
    // Spaces and tabs around a context line do not count; blank lines are skipped.
    let spaced = snapshot("\t// first context  \n\n   \nfn a() -> u8 { 1 }\n\n  // between\t\n");
    assert_eq!(one(&spaced, "function_item", "a").quote, a.quote);
}

#[test]
fn quote_is_equal_under_lf_and_crlf() {
    let source = "// before  \n#[inline]\npub fn f(x: u8) -> u8 {\n    let s = \"a\nb\";\n    x\n}\n\t// after\n";
    let (lf, cr) = (snapshot(source), snapshot(&crlf(source)));
    assert_eq!(
        one(&lf, "function_item", "f").quote,
        one(&cr, "function_item", "f").quote
    );
    assert_eq!(
        module(&lf).quote,
        module(&cr).quote,
        "the file module's quote"
    );
}

#[test]
fn crlf_literal_keeps_body_bytes() {
    let source = "fn f() -> &'static str { \"a\nb\" }\n";
    let lf = one(&snapshot(source), "function_item", "f");
    let cr = one(&snapshot(&crlf(source)), "function_item", "f");
    // `{` `"` `a\nb` `"` `}`
    assert_eq!(lf.body_bytes, 7);
    assert_eq!(cr.body_bytes, 7, "the CR before the LF is not counted");
}

#[test]
fn rustfmt_variants_keep_body_bytes() {
    let pairs = [
        (
            "fn f(v: &[u32]) -> u32 { v.iter().map(|x| { x + 1 }).sum() }\n",
            "fn f(v: &[u32]) -> u32 {\n    v.iter().map(|x| x + 1).sum()\n}\n",
        ),
        (
            "fn f(n: u8) -> u8 { match n { 0 => { return 0 } k => k, } }\n",
            "fn f(n: u8) -> u8 {\n    match n {\n        0 => return 0,\n        k => k,\n    }\n}\n",
        ),
        (
            "fn f(v: &[u8]) { for x in v { if *x == 0 { continue } else { g(*x, 1,) } } }\n",
            "fn f(v: &[u8]) {\n    for x in v {\n        if *x == 0 {\n            continue;\n        } else {\n            g(*x, 1)\n        }\n    }\n}\n",
        ),
    ];
    for (compact, formatted) in pairs {
        let a = one(&snapshot(compact), "function_item", "f");
        let b = one(&snapshot(formatted), "function_item", "f");
        assert_eq!(a.body_bytes, b.body_bytes, "{compact}");
        assert_eq!(a.body, b.body, "{compact}");
    }
    // Every item of the committed variants of one fixture file.
    let file = "src/functions.rs";
    let base = snapshot(&read(file));
    for variant in ["fmt-default", "fmt-60"] {
        let other = snapshot(&read_variant(variant, file));
        for (index, key) in base.keys.iter().enumerate() {
            let at = other.index_of(key).expect("matched");
            let (a, b) = (
                base.units[index].levels().expect("hashed"),
                other.units[at].levels().expect("hashed"),
            );
            assert_eq!(a.body_bytes, b.body_bytes, "{variant}: {key}");
        }
    }
}

#[test]
fn body_of_exactly_124_bytes_is_not_trivial() {
    assert_eq!(TRIVIAL_BODY_BYTES, 124);
    // `{` + 61 `a;` + `}` = 124; one `a;` fewer and an `ab;` = 123.
    let at = format!("fn f() {{ {} }}\n", "a; ".repeat(61));
    let below = format!("fn f() {{ ab; {} }}\n", "a; ".repeat(59));
    let at = one(&snapshot(&at), "function_item", "f");
    let below = one(&snapshot(&below), "function_item", "f");
    assert_eq!((at.body_bytes, below.body_bytes), (124, 123));
    let state = |source: &str| {
        let tree = parse(&mut new_parser(), source);
        match item_levels(&tree, source, &[0]).remove(0) {
            Some(LevelState::Hashed(levels)) => levels.is_trivial(),
            other => panic!("{other:?}"),
        }
    };
    assert!(!state(&format!("fn f() {{ {} }}\n", "a; ".repeat(61))));
    assert!(state(&format!("fn f() {{ ab; {} }}\n", "a; ".repeat(59))));
}

// =================================================================== RD-8

/// Words of one framework's vocabulary, matched whole and case-sensitively.
const FRAMEWORK_WORDS: [&str; 14] = [
    "build",
    "Plugin",
    "Plugins",
    "add_systems",
    "add_plugins",
    "bevy",
    "Bevy",
    "ecs",
    "ECS",
    "App",
    "Startup",
    "Update",
    "Commands",
    "Query",
];

fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// RD-8: the level sources (`levels.rs`, the twin in `bind.rs`) know no
/// framework: no `build`, `Plugin`, `add_systems` (comments included).
#[test]
fn level_sources_name_no_framework() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let levels = std::fs::read_to_string(src.join("levels.rs")).expect("levels.rs");
    let bind = std::fs::read_to_string(src.join("bind.rs")).expect("bind.rs");
    let start = bind.find("pub fn item_levels(").expect("the twin");
    let twin = &bind[start..start + bind[start..].find("\n}\n").expect("its end")];
    let mut offenders = Vec::new();
    for (name, text) in [
        ("levels.rs", levels.as_str()),
        ("bind.rs item_levels", twin),
    ] {
        for (number, line) in text.lines().enumerate() {
            for word in FRAMEWORK_WORDS {
                if has_word(line, word) {
                    offenders.push(format!("{name}:{}: {word}", number + 1));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{offenders:#?}");
    assert!(
        levels.contains("pub fn call_targets("),
        "the scan reads the real source"
    );
    assert!(has_word("app.add_systems(Update, f)", "add_systems"));
    assert!(!has_word("rebuild", "build"));
}

// =================================================================== RD-9

#[test]
fn level_twin_follows_binding_indexes() {
    let source = "fn a() -> u8 { 1 }\nstruct B;\nfn broken() { let x = 1 2; }\nmod m { fn c() -> u8 { 3 } }\n";
    let tree = parse(&mut new_parser(), source);
    let binding = bind_tree(&tree, source);
    let analysis = analyze_tree(&tree, source);
    let shape =
        |items: &[specengine_code::ItemRecord]| -> Vec<(String, String, std::ops::Range<usize>)> {
            items
                .iter()
                .map(|i| (i.kind.clone(), i.label.clone(), i.byte_range.clone()))
                .collect()
        };
    assert_eq!(
        shape(&binding.analysis.items),
        shape(&analysis.items),
        "both analyses give the same items in the same order"
    );
    let count = analysis.items.len();
    assert_eq!(count, 5);
    let all: Vec<usize> = (0..count).collect();
    let forward = item_levels(&tree, source, &all);
    // An error-free item: NotHashed in the binding, Hashed through the twin.
    assert_eq!(binding.analysis.items[0].state, HashState::NotHashed);
    assert!(matches!(forward[0], Some(LevelState::Hashed(_))));
    assert!(matches!(forward[2], Some(LevelState::CannotVerify { .. })));
    // The order asked, repeats and an index past the items.
    let asked = [4, 0, 4, 2, count, 1, 3];
    let got = item_levels(&tree, source, &asked);
    assert_eq!(got.len(), asked.len());
    for (position, &index) in asked.iter().enumerate() {
        let want = forward.get(index).cloned().flatten();
        assert_eq!(got[position], want, "index {index} asked at {position}");
    }
    // Each alone, and the v2 twin answers by the same indexes.
    for (index, state) in forward.iter().enumerate() {
        assert_eq!(
            item_levels(&tree, source, &[index]),
            std::slice::from_ref(state)
        );
    }
    let v2 = hash_bound_items(&tree, source, &all);
    for (index, item) in analysis.items.iter().enumerate() {
        assert_eq!(v2[index].as_ref(), Some(&item.state), "{}", item.label);
    }
}

#[test]
fn inline_mod_module_is_its_items_levels() {
    let source = "//! The file.\nfn top() {}\nmod inner {\n    //! The inline module.\n    fn f() -> u8 { 1 }\n}\n";
    let tree = parse(&mut new_parser(), source);
    let binding = bind_tree(&tree, source);
    let inner = binding
        .analysis
        .items
        .iter()
        .position(|item| item.kind == "mod_item" && item.label == "inner")
        .expect("the inline mod");
    let modules: Vec<Binding> = binding
        .comments
        .iter()
        .map(|comment| comment.binding)
        .filter(|binding| matches!(binding, Binding::Module(_)))
        .collect();
    assert_eq!(
        modules,
        [Binding::Module(None), Binding::Module(Some(inner))],
        "a //! at file level and one inside the inline mod"
    );
    let entry = item_levels(&tree, source, &[inner])
        .remove(0)
        .expect("an entry");
    let snap = snapshot(source);
    let LevelState::Hashed(levels) = &entry else {
        panic!("{entry:?}");
    };
    let mine = one(&snap, "mod_item", "inner");
    assert_eq!(levels.sig, mine.sig);
    assert_eq!(levels.body, mine.body);
    assert_ne!(
        LevelState::Hashed(*levels),
        module_levels(&tree, source),
        "the inline mod's module is item `inner`'s entry, not the file's"
    );
}

#[test]
fn qualified_call_targets_do_not_glue() {
    let source = "fn f(x: X) {\n    <T as Trait>::make();\n    < T  as  Trait >::make();\n    <Vec<u8> as Default>::default();\n    <dyn Tr>::g();\n    crate::a::b();\n    Self::new();\n    x.y::<T>();\n    Vec::<u8>::with_capacity(4);\n    S::<T>::f();\n}\n";
    assert_eq!(
        targets_of(source, "function_item", "f"),
        [
            ".y",
            "<T as Trait>::make",
            "<Vec<u8>as Default>::default",
            "<dyn Tr>::g",
            "S::f",
            "Self::new",
            "Vec::with_capacity",
            "crate::a::b",
        ]
    );
}

// ============================================ iteration 2 readings (H1-H4)
//
// Review 1 settled four byte-level readings that iteration 2 builds (owner
// pre-approved). These tests state the target behaviour and are red on
// iteration 1 by design; nothing they touch is in the golden pin.

/// H1 (red on iteration 1 by design): a call whose `function` is not an
/// `identifier`, `scoped_identifier`, `field_expression` or
/// `generic_function` contributes only its `kind()` as a target, so the
/// target text stays bounded; calls nested inside still count.
#[test]
fn non_path_call_targets_are_their_kind() {
    let source = "fn f(c: bool) { (make())(); get_fn()(1); [g][0](); (|x: u8| x)(1); (if c { a } else { b })(2); }\n";
    let want = [
        "call_expression",
        "get_fn",
        "index_expression",
        "make",
        "parenthesized_expression",
    ];
    assert_eq!(targets_of(source, "function_item", "f"), want);
    let levels = one(&snapshot(source), "function_item", "f");
    assert_eq!(*levels.deps.as_bytes(), deps_after_header(&want));
    // Nested n deep, the targets stay two.
    let chained = format!("fn f() {{ g(){} }}\n", "()".repeat(1_000));
    assert_eq!(
        targets_of(&chained, "function_item", "f"),
        ["call_expression", "g"]
    );
}

/// H2 (red on iteration 1 by design): a `shebang` does not end the file's
/// leading inner-attribute run; its leaf stays in `body`.
#[test]
fn shebang_keeps_the_module_inner_attribute_run() {
    let with = "#!/usr/bin/env run-arena\n#![allow(dead_code)]\nfn f() {}\n";
    let without = "#![allow(dead_code)]\nfn f() {}\n";
    let tree = parse(&mut new_parser(), with);
    assert!(!tree.root_node().has_error());
    assert_eq!(tree.root_node().child(0).map(|n| n.kind()), Some("shebang"));
    let (a, b) = (module(&snapshot(with)), module(&snapshot(without)));
    assert_ne!(
        *b.sig.as_bytes(),
        digest_after_header("sig", &[]),
        "the control holds its attribute in sig"
    );
    assert_eq!(
        a.sig, b.sig,
        "the #![..] after a shebang is the module's sig"
    );
    assert_ne!(a.body, b.body, "the shebang's leaf is in body");
    let edited = module(&snapshot(&with.replace("dead_code", "unused")));
    assert_ne!(a.sig, edited.sig);
    assert_eq!(a.body, edited.body);
}

/// H3 (red on iteration 1 by design): the module's `quote` runs from its
/// first child's start to its last child's end: whitespace at either end of
/// the file does not count.
#[test]
fn module_quote_trims_whitespace_at_both_ends() {
    let plain = module(&snapshot("fn f() {}"));
    for padded in ["fn f() {}\n", "\n\n  fn f() {}\n\n\n", "fn f() {}\r\n \t\n"] {
        assert_eq!(
            module(&snapshot(padded)).quote,
            plain.quote,
            "{padded:?}: whitespace around the file"
        );
    }
    let mut stream = Vec::new();
    for part in ["", "fn f() {}", "", ""] {
        put(&mut stream, part.as_bytes());
    }
    assert_eq!(
        *plain.quote.as_bytes(),
        digest_after_header("quote", &stream),
        "no name, no context lines, the file's text"
    );
    // A last child that ends in whitespace itself (a comment's trailing
    // spaces, a doc comment's newline) is trimmed too.
    for (padded, bare) in [
        ("fn f() {}\n// end  \n", "fn f() {}\n// end"),
        (
            "fn f() {}\n/// trailing doc\n\n",
            "fn f() {}\n/// trailing doc",
        ),
        ("//! The file.\n", "//! The file."),
    ] {
        assert_eq!(
            module(&snapshot(padded)).quote,
            module(&snapshot(bare)).quote,
            "{padded:?}"
        );
    }
}

/// H4 (red on iteration 1 by design): `#[cfg_attr(.., macro_use)]` breaks an
/// N5 run as `#[macro_use]` does.
#[test]
fn cfg_attr_macro_use_breaks_the_run() {
    let source = "#[cfg_attr(feature = \"m\", macro_use)]\nmod macros;\nmod alpha;\nmod beta;\n";
    let moved = "mod alpha;\n#[cfg_attr(feature = \"m\", macro_use)]\nmod macros;\nmod beta;\n";
    let control = "#[cfg_attr(feature = \"m\", macro_use)]\nmod macros;\nmod beta;\nmod alpha;\n";
    let base = module(&snapshot(source));
    assert_eq!(
        base.body,
        module(&snapshot(control)).body,
        "the others still sort"
    );
    assert_ne!(
        base.body,
        module(&snapshot(moved)).body,
        "a cfg_attr macro_use member stays in place"
    );
    let crates = "#[cfg_attr(test, macro_use)]\nextern crate x;\nextern crate a;\n";
    let crates_moved = "extern crate a;\n#[cfg_attr(test, macro_use)]\nextern crate x;\n";
    assert_ne!(
        module(&snapshot(crates)).body,
        module(&snapshot(crates_moved)).body
    );
    // A cfg_attr without macro_use joins the run as any attribute does.
    let plain = "#[cfg_attr(test, allow(unused))]\nmod t;\nmod a;\n";
    let plain_moved = "mod a;\n#[cfg_attr(test, allow(unused))]\nmod t;\n";
    assert_eq!(
        module(&snapshot(plain)).body,
        module(&snapshot(plain_moved)).body
    );
}

/// H3, as built in iteration 2: a file without a child (empty, or only
/// whitespace) quotes the empty text.
#[test]
fn module_quote_of_an_empty_file_is_empty() {
    let mut stream = Vec::new();
    for _ in 0..4 {
        put(&mut stream, b"");
    }
    let empty = digest_after_header("quote", &stream);
    for source in ["", "\n", "  \n\t\r\n\n"] {
        assert_eq!(
            *module(&snapshot(source)).quote.as_bytes(),
            empty,
            "{source:?}"
        );
    }
}

/// H4, as built in iteration 2: only a group after the predicate that is
/// exactly `macro_use` or `macro_use(..)` breaks the run; the predicate, a
/// longer name, another shape or `macro_use` inside another attribute's
/// arguments joins it.
#[test]
fn cfg_attr_breaks_the_run_only_for_a_macro_use_attribute() {
    let joins = |attribute: &str| {
        let first = module(&snapshot(&format!("{attribute}\nmod t;\nmod a;\n"))).body;
        let moved = module(&snapshot(&format!("mod a;\n{attribute}\nmod t;\n"))).body;
        first == moved
    };
    for attribute in [
        "#[cfg_attr(macro_use, allow(unused))]",
        "#[cfg_attr(test, macro_use_x)]",
        "#[cfg_attr(test, macro_use = \"x\")]",
        "#[cfg_attr(test, allow(macro_use))]",
    ] {
        assert!(joins(attribute), "{attribute} joins the run");
    }
    for attribute in [
        "#[cfg_attr(all(a, b), allow(x), macro_use(m1, m2))]",
        "#[cfg_attr(test, macro_use,)]",
    ] {
        assert!(!joins(attribute), "{attribute} breaks the run");
    }
}

// ======================================================= 5a-1c (N6) TE-3, TE-4, TE-8
//
// docs/features/hash-levels-use-lists.md: the members of a `use_list` are a
// run (N6). TE-1 (every permutation keeps every level) and TE-2 (the
// committed rustfmt variants) are `hash_levels_props.rs`'; TE-6 (depth) is
// `hash_levels_depth.rs`'; TE-5 (v2 untouched) is `hash_v2.rs`'.

/// An item by `kind` and `label`.
type ItemName = (&'static str, &'static str);

/// The places of a `use` tree: (what, source, the item holding it); the
/// file-level place holds no item.
fn use_places(tree: &str) -> [(&'static str, String, Option<ItemName>); 3] {
    [
        (
            "file level",
            format!("use {tree};\n\nfn tail() {{}}\n"),
            None,
        ),
        (
            "a function body",
            format!("fn host() {{\n    use {tree};\n    let _ = 0;\n}}\n"),
            Some(("function_item", "host")),
        ),
        (
            "an inline mod",
            format!("mod hub {{\n    use {tree};\n    fn g() {{}}\n}}\n"),
            Some(("mod_item", "hub")),
        ),
    ]
}

/// Every unit holding `use {tree};` in each place: the item (if any) and the
/// module, each with its name.
fn use_units(tree: &str) -> Vec<(String, Levels)> {
    let mut out = Vec::new();
    for (place, source, item) in use_places(tree) {
        let snap = snapshot(&source);
        assert!(!snap.root_has_error, "{source}: parses cleanly");
        if let Some((kind, label)) = item {
            out.push((format!("{place}: {kind} {label}"), one(&snap, kind, label)));
        }
        out.push((format!("{place}: the module"), module(&snap)));
    }
    out
}

/// The fields among `names` that differ between the units of `a` and `b`,
/// unit by unit.
fn use_changes(a: &str, b: &str, names: &[&str]) -> Vec<(String, Vec<String>)> {
    use_units(a)
        .into_iter()
        .zip(use_units(b))
        .map(|((what, x), (_, y))| (what, changed(&x, &y, names)))
        .collect()
}

/// TE-3: every edit that changes what a list imports changes `body` of the
/// item and of the module, in every place, and nothing but the body
/// streams (`sig` and `deps` hold: no call, no signature). M3 duplicates
/// removed after sorting.
#[test]
fn te3_use_list_edits_change_the_body() {
    for (edit, before, after) in [
        ("a member removed", "x::{a, b, c}", "x::{a, b}"),
        ("a member renamed", "x::{a, b, c}", "x::{a, b, d}"),
        ("an alias changed", "x::{a as p, b}", "x::{a as q, b}"),
        ("an alias dropped", "x::{a as p, b}", "x::{a, b}"),
        ("a glob swapped for a name", "x::{a, *}", "x::{a, b}"),
        ("self removed", "x::{self, a, b}", "x::{a, b}"),
        (
            "a member moved between nested lists",
            "x::{a::{p, q}, b::{r, s}}",
            "x::{a::{p, r}, b::{q, s}}",
        ),
        (
            "a member moved one list out",
            "x::{a::{p, q}, b}",
            "x::{a::{p}, b, q}",
        ),
        ("a duplicate added", "x::{a, b}", "x::{a, b, b}"),
        (
            "a duplicate added out of order",
            "x::{b, a}",
            "x::{b, a, b}",
        ),
        ("the path of the list changed", "x::{a, b}", "y::{a, b}"),
    ] {
        for (what, moved) in use_changes(before, after, &ALL) {
            assert!(
                moved.iter().any(|f| f == "body"),
                "{edit}, {what}: `{before}` -> `{after}` must change body, changed {moved:?}"
            );
            assert!(
                !moved.iter().any(|f| f == "sig" || f == "deps"),
                "{edit}, {what}: `{before}` -> `{after}` changed {moved:?}: only the body streams"
            );
        }
    }
}

/// TE-4: `skeleton` sorts a list by its members' masked walks: two lists
/// whose members mask alike are equal in `skeleton`, whatever their names'
/// order, while `body` tells them apart. M4 the masked stream sorted by the
/// normal walks.
#[test]
fn te4_use_list_skeleton_sorts_by_the_masked_walks() {
    let base = "x::{a::{p, q}, b::{r}}";
    for other in [
        "x::{z::{p, q}, b::{r}}",
        "x::{b::{r}, z::{p, q}}",
        "x::{a::{q, p}, b::{r}}",
        "x::{y::{r}, a::{p, q}}",
    ] {
        for (what, moved) in use_changes(base, other, &STABLE) {
            assert!(
                !moved.iter().any(|f| f == "skeleton"),
                "{what}: `{base}` and `{other}` mask alike, yet skeleton changed"
            );
            if other != "x::{a::{q, p}, b::{r}}" {
                assert!(
                    moved.iter().any(|f| f == "body"),
                    "{what}: `{base}` and `{other}` import different names: body must differ"
                );
            }
        }
    }
    // Control: a list of another shape is another skeleton.
    for (what, moved) in use_changes(base, "x::{a::{p, q}, b::{r, s}}", &STABLE) {
        assert!(
            moved.iter().any(|f| f == "skeleton"),
            "{what}: one more member is a structural change"
        );
    }
}

/// TE-8, the named limits of Q2: N6 sorts, it never rewrites. A brace list
/// of one member is not its unwrapped path (rustfmt unwraps it); an empty
/// list is not its absence (rustfmt removes it); a duplicate is kept (rustfmt
/// 1.9.0 may remove it; a duplicate import does not compile, E0252). M: a
/// single-member unwrap.
#[test]
fn te8_single_member_and_empty_lists_are_not_normalised() {
    for (braced, plain) in [
        ("a::{b}", "a::b"),
        ("a::{*}", "a::*"),
        ("a::{b as c}", "a::b as c"),
        ("a::{b::{c}}", "a::b::c"),
        ("a::{b::{c}}", "a::{b::c}"),
        ("a::{a, a}", "a::{a}"),
    ] {
        for (what, moved) in use_changes(braced, plain, &STABLE) {
            assert!(
                moved.iter().any(|f| f == "body"),
                "{what}: `use {braced};` and `use {plain};` must differ in body (a named limit)"
            );
        }
    }
    let empty = module(&snapshot("use a::{};\n\nfn tail() {}\n"));
    let absent = module(&snapshot("\nfn tail() {}\n"));
    assert_ne!(
        empty.body, absent.body,
        "`use a::{{}};` is not normalised away (a named limit)"
    );
}
