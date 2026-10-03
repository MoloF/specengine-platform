//! Trap tests for the canonical AST hash (05 §5.2) on real tree-sitter parses:
//! anonymous commas, comment filtering by `kind()` (never `is_extra()`),
//! `cannot_verify` on parse errors, name exclusion, attributes, determinism,
//! comment stripping, item collection edge cases and `#[path]` / file roles.

use std::path::{Path, PathBuf};

use specengine_code::comments::strip_comments;
use specengine_code::hash::{
    self, Digest, ErrorCategory, HashState, attached_attributes, hash_item, normalize,
};
use specengine_code::items::{FileAnalysis, analyze_tree};
use specengine_code::qpath::{self, FileRole, Unit, UnitKind};
use specengine_code::{RustParser, grammar};
use tree_sitter::{Node, Tree};

fn parse(source: &str) -> Tree {
    RustParser::new()
        .expect("grammar loads")
        .parse(source)
        .expect("parse completes")
}

/// Top-level nodes that the item walker would consider (attributes and
/// comments are attached, not items).
fn top_level_items(root: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = root.walk();
    root.children(&mut cursor)
        .filter(|node| {
            !matches!(
                node.kind(),
                "attribute_item" | "inner_attribute_item" | "line_comment" | "block_comment"
            ) && !node.is_error()
        })
        .collect()
}

fn states(source: &str) -> Vec<HashState> {
    let tree = parse(source);
    top_level_items(tree.root_node())
        .into_iter()
        .map(|item| hash_item(item, source.as_bytes()))
        .collect()
}

/// Digest of the single top-level item in `source`.
fn digest(source: &str) -> Digest {
    let states = states(source);
    assert_eq!(states.len(), 1, "exactly one item expected in {source:?}");
    states[0]
        .digest()
        .unwrap_or_else(|| panic!("item in {source:?} must hash, got {:?}", states[0]))
}

/// Normalised byte stream of the single top-level item, name excluded.
fn stream(source: &str) -> Vec<u8> {
    let tree = parse(source);
    let items = top_level_items(tree.root_node());
    assert_eq!(items.len(), 1, "exactly one item expected in {source:?}");
    let mut sink: Vec<u8> = Vec::new();
    // No use run nests here, so the walk is complete; `false` would mean an
    // incomplete stream that must not be compared.
    assert!(
        normalize(
            items[0],
            source.as_bytes(),
            hash::name_node(items[0]),
            &mut sink,
        ),
        "incomplete stream for {source:?}"
    );
    sink
}

fn analysis(source: &str) -> FileAnalysis {
    analyze_tree(&parse(source), source)
}

fn labels(analysis: &FileAnalysis) -> Vec<String> {
    analysis.items.iter().map(|i| i.label.clone()).collect()
}

fn digests(analysis: &FileAnalysis) -> Vec<Option<String>> {
    analysis
        .items
        .iter()
        .map(|i| i.state.digest().map(|d| d.to_hex()))
        .collect()
}

// ---------------------------------------------------------------- commas

#[test]
fn trailing_commas_do_not_change_the_hash() {
    let pairs = [
        ("fn f(a: u8, b: u8) {}", "fn f(a: u8, b: u8,) {}"),
        (
            "fn f() -> P { P { x: 1, y: 2 } }",
            "fn f() -> P { P { x: 1, y: 2, } }",
        ),
        (
            "fn f() -> [u8; 3] { [1, 2, 3] }",
            "fn f() -> [u8; 3] { [1, 2, 3,] }",
        ),
        ("enum E { A, B }", "enum E { A, B, }"),
        ("fn f() { g(1, 2) }", "fn f() { g(1, 2,) }"),
        (
            "fn f(e: E) -> u8 { match e { E::A => 1, E::B => 2 } }",
            "fn f(e: E) -> u8 { match e { E::A => 1, E::B => 2, } }",
        ),
        (
            "struct S<A, B> { a: A, b: B }",
            "struct S<A, B,> { a: A, b: B, }",
        ),
    ];
    for (without, with) in pairs {
        assert_eq!(
            digest(without),
            digest(with),
            "trailing comma changed the hash: {without:?} vs {with:?}"
        );
    }
    // Control: the comma skip must not blind the hash to real changes.
    assert_ne!(
        digest("fn f() -> [u8; 2] { [1, 2] }"),
        digest("fn f() -> [u8; 2] { [1, 3] }")
    );
    assert_ne!(digest("fn f() { g(1, 2) }"), digest("fn f() { g(1) }"));
}

// ---------------------------------------------------------------- trap 1

#[test]
fn error_nodes_are_extra_but_never_treated_as_comments() {
    // Premise of trap 1 in this grammar: an ERROR node answers `is_extra()`.
    let source = "fn a() { let x = 1 2; }";
    let tree = parse(source);
    let item = top_level_items(tree.root_node())[0];
    let error = find_error(item).expect("the item contains an ERROR node");
    assert!(error.is_error());
    assert!(
        error.is_extra(),
        "trap 1 premise: ERROR nodes are `is_extra()` in tree-sitter-rust {}",
        grammar::TREE_SITTER_RUST_VERSION
    );
    assert!(
        !hash::is_comment(error),
        "the comment filter must not swallow ERROR nodes"
    );
    let comment_tree = parse("// line\n/* block */\nfn a() {}");
    let root = comment_tree.root_node();
    assert!(hash::is_comment(root.child(0).unwrap()));
    assert!(hash::is_comment(root.child(1).unwrap()));
    assert!(!hash::is_comment(root.child(2).unwrap()));
}

fn find_error(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() {
        return Some(node);
    }
    let mut cursor = node.walk();
    let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
    children.into_iter().find_map(find_error)
}

#[test]
fn error_regions_stay_in_the_normalized_stream() {
    // The two bodies differ only inside their ERROR node. A filter on
    // `is_extra()` drops both regions and the streams collide.
    let one = stream("fn broken_one() { let x = 1 2; }");
    let two = stream("fn broken_two() { let x = 1 2 3; }");
    assert_ne!(
        one, two,
        "ERROR regions were dropped from the stream (trap 1: is_extra())"
    );
    assert!(
        one.windows(5).any(|w| w == b"ERROR"),
        "the ERROR node kind must appear in the stream"
    );
}

#[test]
fn items_with_parse_errors_are_cannot_verify_and_never_hashed() {
    let cases = [
        ("fn broken_one() { let x = 1 2; }", ErrorCategory::Other),
        ("fn broken_two() { let x = 1 2 3; }", ErrorCategory::Other),
        ("fn missing() { let x = 1 + ; }", ErrorCategory::Missing),
        (
            "macro_rules! tilde { ($v:expr) => { $v ~ 1 }; }",
            ErrorCategory::MacroPunct,
        ),
    ];
    for (source, category) in cases {
        let states = states(source);
        assert_eq!(states.len(), 1, "one item in {source:?}");
        match &states[0] {
            HashState::CannotVerify { categories } => {
                assert!(
                    categories.contains(&category),
                    "{source:?}: expected {category:?} in {categories:?}"
                );
                assert!(categories.is_sorted(), "categories are sorted");
            }
            HashState::Hashed(digest) => {
                panic!("{source:?} has a parse error but was hashed as {digest}")
            }
        }
        assert!(
            states[0].digest().is_none(),
            "{source:?} must not yield a digest"
        );
    }
    // A broken attribute poisons the item it is attached to.
    let states = states("#[derive(Debug,]\nstruct S;");
    assert_eq!(states.len(), 1);
    assert!(
        states[0].digest().is_none(),
        "an item with a broken attached attribute must be cannot_verify: {:?}",
        states[0]
    );
    // Category labels form a closed, English set.
    for category in [
        ErrorCategory::MacroPunct,
        ErrorCategory::MacroBody,
        ErrorCategory::WhereMultiline,
        ErrorCategory::Missing,
        ErrorCategory::Other,
    ] {
        let label = category.as_str();
        assert!(
            label.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{label:?}"
        );
    }
}

// ---------------------------------------------------------------- comments

#[test]
fn comments_of_every_kind_do_not_change_the_hash() {
    let plain = digest("fn f() -> u8 { self_check(1) }");
    let commented = [
        "/// Doc comment.\nfn f() -> u8 { self_check(1) }",
        "/** Block doc. */\nfn f() -> u8 { self_check(1) }",
        "fn f() -> u8 { // why\n self_check(1) }",
        "fn f() -> u8 { self_check(/* inline */ 1) /* trailing */ }",
        "//! Inner doc.\nfn f() -> u8 { self_check(1) }",
        "/* before */ fn f() -> u8 {\n    // one\n    // two\n    self_check(1)\n}",
    ];
    for source in commented {
        assert_eq!(
            digest(source),
            plain,
            "comment changed the hash: {source:?}"
        );
    }
}

#[test]
fn attributes_are_hashed_and_stay_attached_across_comments() {
    assert_ne!(digest("#[inline]\nfn f() {}"), digest("fn f() {}"));
    assert_ne!(
        digest("#[cfg(test)]\nfn f() {}"),
        digest("#[cfg(feature = \"x\")]\nfn f() {}")
    );
    assert_eq!(
        digest("#[inline]\n/// doc\nfn f() {}"),
        digest("#[inline]\nfn f() {}")
    );
    let source = "#[derive(Debug)]\n// between\n/// doc\n#[repr(C)]\nstruct S;";
    let tree = parse(source);
    let item = top_level_items(tree.root_node())[0];
    assert_eq!(item.kind(), "struct_item");
    let attributes = attached_attributes(item);
    assert_eq!(
        attributes.len(),
        2,
        "comments must not break the attribute run"
    );
    assert_eq!(
        digest(source),
        digest("#[derive(Debug)]\n#[repr(C)]\nstruct S;")
    );
    assert_ne!(digest(source), digest("#[repr(C)]\nstruct S;"));
}

// ---------------------------------------------------------------- names, operators

#[test]
fn own_name_is_excluded_but_body_identifiers_and_operators_count() {
    assert_eq!(
        digest("fn alpha() { compute(1) }"),
        digest("fn beta() { compute(1) }")
    );
    assert_eq!(digest("struct A { x: u8 }"), digest("struct B { x: u8 }"));
    assert_eq!(digest("enum A { X, Y }"), digest("enum B { X, Y }"));
    assert_ne!(
        digest("fn alpha() { compute(1) }"),
        digest("fn alpha() { compute(2) }")
    );
    assert_ne!(
        digest("fn alpha() { compute(1) }"),
        digest("fn alpha() { compote(1) }")
    );
    // `to_sexp()` trap: anonymous operator tokens carry meaning.
    assert_ne!(
        digest("fn f(a: u8, b: u8) -> u8 { a + b }"),
        digest("fn f(a: u8, b: u8) -> u8 { a - b }")
    );
    assert_ne!(
        digest("fn f(a: u8, b: u8) -> bool { a < b }"),
        digest("fn f(a: u8, b: u8) -> bool { a > b }")
    );
    // Whitespace is not a node.
    assert_eq!(
        digest("fn f(a:u8,b:u8)->u8{a+b}"),
        digest("fn f(a: u8, b: u8) -> u8 {\n    a + b\n}")
    );
}

#[test]
fn mixed_script_identifiers_hash_by_bytes_without_panic() {
    let latin = digest("fn f() -> u8 { let ok = 1; ok }");
    let cyrillic_o = digest("fn f() -> u8 { let \u{043e}k = 1; \u{043e}k }");
    assert_ne!(
        latin, cyrillic_o,
        "a look-alike identifier is a different body"
    );
    assert_eq!(
        digest("fn caf\u{e9}() {}"),
        digest("fn cafe() {}"),
        "names are excluded"
    );
}

// ---------------------------------------------------------------- determinism

#[test]
fn recipe_header_pins_the_grammar_versions() {
    let header = String::from_utf8_lossy(hash::recipe_header()).into_owned();
    assert!(header.contains(hash::RECIPE), "{header:?}");
    assert!(
        header.contains(grammar::TREE_SITTER_RUST_VERSION),
        "{header:?}"
    );
    let info = grammar::grammar_info();
    assert_eq!(info.tree_sitter, grammar::TREE_SITTER_VERSION);
    assert_eq!(info.tree_sitter_rust, grammar::TREE_SITTER_RUST_VERSION);
    assert!(
        header.contains(&info.abi.to_string()),
        "{header:?} lacks abi {}",
        info.abi
    );
}

#[test]
fn hash_is_deterministic_across_parsers_and_runs() {
    let source = "#[derive(Clone)]\npub struct Pair { a: u32, b: u32 }";
    let first = digest(source);
    let second = digest(source);
    assert_eq!(first, second);
    assert_eq!(first.as_bytes(), second.as_bytes());
    let hex = first.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(hex, format!("{first}"));
    // Same bytes twice through two independent parser instances.
    let mut a = RustParser::new().unwrap();
    let mut b = RustParser::new().unwrap();
    let ta = a.parse(source).unwrap();
    let tb = b.parse(source).unwrap();
    let ia = top_level_items(ta.root_node())[0];
    let ib = top_level_items(tb.root_node())[0];
    assert_eq!(
        hash_item(ia, source.as_bytes()),
        hash_item(ib, source.as_bytes())
    );
}

// ---------------------------------------------------------------- stripping

#[test]
fn strip_comments_keeps_strings_and_item_hashes() {
    let source = "//! Module doc.\n\
                  /// Doc on f.\n\
                  pub fn f() -> &'static str { \"// not a comment /* nor this */\" } // trailing\n\
                  /* block */ pub struct S { /* inner */ pub x: u8, // field\n }\n\
                  impl S { /** doc */ pub fn m(&self) -> u8 { self.x /* here */ } }\n";
    let tree = parse(source);
    let stripped = strip_comments(source, &tree);
    assert!(stripped.contains("\"// not a comment /* nor this */\""));
    assert!(!stripped.contains("// trailing"));
    assert!(!stripped.contains("/* block */"));
    assert!(!stripped.contains("Module doc"));
    assert!(!stripped.contains("/** doc */"));
    let before = analysis(source);
    let after = analysis(&stripped);
    assert!(
        !after.has_error,
        "stripping must not introduce parse errors"
    );
    assert_eq!(labels(&before), labels(&after));
    assert_eq!(digests(&before), digests(&after));
    assert!(digests(&before).iter().all(Option::is_some));
    // Idempotent.
    let again = strip_comments(&stripped, &parse(&stripped));
    assert_eq!(again, stripped);
}

// ---------------------------------------------------------------- collection

#[test]
fn empty_and_comment_only_files_yield_no_items_and_no_error() {
    for source in [
        "",
        "   \n\t\n",
        "// only a comment\n/* and a block */\n",
        "//! doc only\n",
    ] {
        let analysis = analysis(source);
        assert!(analysis.items.is_empty(), "{source:?}");
        assert!(!analysis.has_error, "{source:?}");
        assert_eq!(analysis.orphan_errors, 0, "{source:?}");
        assert!(analysis.mod_declarations.is_empty(), "{source:?}");
        assert_eq!(analysis.path_attributes(), 0, "{source:?}");
    }
}

#[test]
fn orphan_junk_is_a_region_not_an_item_and_recovery_is_local() {
    let analysis = analysis("fn ok() -> u8 { 1 }\n) ) )\nfn also_ok() -> u8 { 2 }\n");
    assert!(analysis.has_error);
    assert!(
        analysis.orphan_errors >= 1,
        "stray tokens are orphan regions"
    );
    assert_eq!(labels(&analysis), ["ok", "also_ok"]);
    for item in &analysis.items {
        assert!(
            !item.has_error,
            "{}: recovery is local, neighbours hash",
            item.label
        );
        assert!(item.state.digest().is_some(), "{}", item.label);
    }
}

#[test]
fn items_are_collected_with_scope_owner_and_line() {
    let source = "pub mod outer {\n\
                  pub mod inner { pub fn deep() {} }\n\
                  pub struct T;\n\
                  impl T { pub fn m(&self) {} fn n() {} }\n\
                  impl Clone for T { fn clone(&self) -> Self { T } }\n\
                  pub trait Tr { fn r(&self); fn p(&self) {} }\n\
                  }\n\
                  const C: u8 = 1; static S: u8 = 2; type A = u8; union U { a: u8 }\n";
    let analysis = analysis(source);
    assert!(!analysis.has_error);
    let find = |label: &str| {
        analysis
            .items
            .iter()
            .find(|i| i.label == label)
            .unwrap_or_else(|| panic!("{label} missing from {:?}", labels(&analysis)))
    };
    assert_eq!(find("deep").mod_path, ["outer", "inner"]);
    assert_eq!(find("deep").line, 2);
    // Owners carry the impl label (`Type` / `<Type as Trait>`); the `impl `
    // prefix is added only to the block's own qpath
    // (`docs/canon/code-identity.md` "Units and `qpath`").
    assert_eq!(find("m").owner.as_deref(), Some("T"));
    assert_eq!(find("m").mod_path, ["outer"]);
    assert_eq!(find("n").owner.as_deref(), Some("T"));
    assert_eq!(find("clone").owner.as_deref(), Some("<T as Clone>"));
    assert_eq!(find("<T as Clone>").kind, "impl_item");
    assert_eq!(find("r").owner.as_deref(), Some("Tr"));
    assert_eq!(find("r").kind, "function_signature_item");
    assert_eq!(find("p").owner.as_deref(), Some("Tr"));
    for label in ["C", "S", "A", "U", "T", "Tr", "outer", "inner"] {
        assert!(find(label).state.digest().is_some(), "{label} hashes");
    }
    assert_eq!(find("C").line, 8);
    assert!(
        analysis.items.iter().all(|i| i.line >= 1),
        "lines are 1-based"
    );
}

#[test]
fn path_attribute_is_counted_and_resolved() {
    let source =
        "#[path = \"vendored/relocated.rs\"]\npub mod relocated;\nmod plain;\nmod inline_mod {}\n";
    let analysis = analysis(source);
    assert_eq!(analysis.path_attributes(), 1);
    assert_eq!(
        analysis.mod_declarations.len(),
        2,
        "inline modules are not declarations"
    );
    let relocated = &analysis.mod_declarations[0];
    assert_eq!(relocated.name, "relocated");
    assert_eq!(
        relocated.path_attribute.as_deref(),
        Some("vendored/relocated.rs")
    );
    assert_eq!(relocated.line, 2);
    assert_eq!(analysis.mod_declarations[1].name, "plain");
    assert_eq!(analysis.mod_declarations[1].path_attribute, None);
    assert_eq!(
        qpath::resolve_path_attribute(Path::new("src/lib.rs"), "vendored/relocated.rs"),
        PathBuf::from("src/vendored/relocated.rs")
    );
    assert_eq!(
        qpath::resolve_path_attribute(Path::new("src/a/b.rs"), "../c.rs"),
        PathBuf::from("src/c.rs")
    );
    assert_eq!(
        qpath::resolve_path_attribute(Path::new("src/lib.rs"), "./x.rs"),
        PathBuf::from("src/x.rs")
    );
    // A `#[path]` on an inline module is not a declaration either.
    let inline = analyze_tree(
        &parse("#[path = \"x.rs\"]\nmod m { fn f() {} }\n"),
        "#[path = \"x.rs\"]\nmod m { fn f() {} }\n",
    );
    assert_eq!(inline.path_attributes(), 0);
}

#[test]
fn file_roles_follow_the_cargo_layout() {
    // The old path-only table, re-read against the package's own targets
    // (Cargo's auto-discovery over the same files): every crate root now
    // names its unit, the primary target (the lib) stays unnamed.
    let files = [
        "src/lib.rs",
        "src/main.rs",
        "build.rs",
        "src/bin/tool.rs",
        "src/bin/multi/main.rs",
        "src/bin/multi/cli.rs",
        "tests/smoke.rs",
        "examples/demo.rs",
        "benches/bench.rs",
        "tests/suite/main.rs",
        "tests/suite/helpers.rs",
        "src/a/b.rs",
        "src/a/mod.rs",
        "src/broken.rs",
        "src/vendored/relocated.rs",
        "scripts/gen.rs",
        "lib.rs",
        "src/notes.txt",
    ];
    let table = qpath::layout_targets("pkg", files.iter().map(Path::new));
    let unit = |kind: &str, name: &str| {
        Some(Unit {
            kind: UnitKind::Target(kind.to_owned()),
            name: name.to_owned(),
        })
    };
    let module = |unit: Option<Unit>, parts: &[&str]| {
        FileRole::Module(unit, parts.iter().map(|p| (*p).to_owned()).collect())
    };
    let cases = [
        ("src/lib.rs", FileRole::CrateRoot(None)),
        ("src/main.rs", FileRole::CrateRoot(unit("bin", "pkg"))),
        (
            "build.rs",
            FileRole::CrateRoot(unit("custom-build", "build-script-build")),
        ),
        ("src/bin/tool.rs", FileRole::CrateRoot(unit("bin", "tool"))),
        (
            "src/bin/multi/main.rs",
            FileRole::CrateRoot(unit("bin", "multi")),
        ),
        (
            "src/bin/multi/cli.rs",
            module(unit("bin", "multi"), &["cli"]),
        ),
        ("tests/smoke.rs", FileRole::CrateRoot(unit("test", "smoke"))),
        (
            "examples/demo.rs",
            FileRole::CrateRoot(unit("example", "demo")),
        ),
        (
            "benches/bench.rs",
            FileRole::CrateRoot(unit("bench", "bench")),
        ),
        (
            "tests/suite/main.rs",
            FileRole::CrateRoot(unit("test", "suite")),
        ),
        (
            "tests/suite/helpers.rs",
            module(unit("test", "suite"), &["helpers"]),
        ),
        ("src/a/b.rs", module(None, &["a", "b"])),
        ("src/a/mod.rs", module(None, &["a"])),
        ("src/broken.rs", module(None, &["broken"])),
        (
            "src/vendored/relocated.rs",
            module(None, &["vendored", "relocated"]),
        ),
        ("scripts/gen.rs", FileRole::Unrooted),
        ("lib.rs", FileRole::Unrooted),
        ("src/notes.txt", FileRole::Unrooted),
    ];
    for (path, expected) in cases {
        assert_eq!(
            qpath::file_role(Path::new(path), &table),
            expected,
            "{path}"
        );
    }
    let packages = ["", "crates/x"];
    let is_package = |dir: &Path| packages.iter().any(|p| Path::new(p) == dir);
    assert_eq!(
        qpath::package_dir(Path::new("crates/x/src/lib.rs"), is_package),
        Some(PathBuf::from("crates/x"))
    );
    assert_eq!(
        qpath::package_dir(Path::new("src/lib.rs"), is_package),
        Some(PathBuf::from(""))
    );
    assert_eq!(
        qpath::package_dir(Path::new("src/lib.rs"), |_: &Path| false),
        None
    );
}

#[test]
fn qpath_names_owner_and_never_an_absolute_path() {
    let analysis = analysis(
        "pub struct Counter;\nimpl Counter { pub fn tick(&self) {} }\nimpl Default for Counter { fn default() -> Self { Counter } }\n",
    );
    let role = FileRole::Module(None, vec!["a".to_owned()]);
    let rendered: Vec<String> = analysis
        .items
        .iter()
        .map(|item| qpath::qpath(".", &role, item).to_string())
        .collect();
    assert_eq!(
        rendered,
        [
            ".::a::Counter",
            ".::a::impl Counter",
            ".::a::Counter::tick",
            ".::a::impl <Counter as Default>",
            ".::a::<Counter as Default>::default",
        ]
    );
    let crate_root: Vec<String> = analysis
        .items
        .iter()
        .map(|item| qpath::qpath("crates/x", &FileRole::CrateRoot(None), item).to_string())
        .collect();
    assert_eq!(crate_root[0], "crates/x::Counter");
    // A non-primary unit sits between the package and the module path.
    let tool = FileRole::CrateRoot(Some(Unit {
        kind: UnitKind::Target("bin".to_owned()),
        name: "tool".to_owned(),
    }));
    let in_tool = qpath::qpath("crates/x", &tool, &analysis.items[2]);
    assert_eq!(in_tool.to_string(), "crates/x::bin:tool::Counter::tick");
    let unrooted = qpath::qpath("crates/x", &FileRole::Unrooted, &analysis.items[0]);
    assert_eq!(unrooted.to_string(), "crates/x::Counter");
    assert_eq!(unrooted.unit, None);
    assert!(
        rendered
            .iter()
            .chain(&crate_root)
            .all(|q| !q.starts_with('/'))
    );
}
