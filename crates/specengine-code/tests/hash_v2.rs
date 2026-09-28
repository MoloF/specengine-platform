//! Recipe v2 of the canonical AST hash (`hash.rs` module docs, 05 §5.2): the
//! shapes rustfmt moves between — closure bodies and match-arm values in and
//! out of `{ }`, `;` after a diverging tail, `{ }` token trees after `|`, `||`
//! or `=>` in the arguments of an expression macro, and the order of `use`
//! runs — must hash equal, while every pair that changes meaning must not.
//! Each pair goes through the real parser and `hash_item`.

use specengine_code::RustParser;
use specengine_code::hash::{self, Digest, hash_item};
use tree_sitter::{Node, Tree};

fn parse(source: &str) -> Tree {
    RustParser::new()
        .expect("grammar loads")
        .parse(source)
        .expect("parse completes")
}

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

/// Digest of the single, error-free top-level item in `source`.
fn digest(source: &str) -> Digest {
    let tree = parse(source);
    let items = top_level_items(tree.root_node());
    assert_eq!(items.len(), 1, "exactly one item expected in {source:?}");
    assert!(
        !tree.root_node().has_error(),
        "the pair source must parse cleanly: {source:?}"
    );
    hash_item(items[0], source.as_bytes())
        .digest()
        .unwrap_or_else(|| panic!("item in {source:?} must hash"))
}

fn assert_equal_pairs(pairs: &[(&str, &str)]) {
    for (left, right) in pairs {
        assert_eq!(
            digest(left),
            digest(right),
            "rustfmt-equivalent shapes must hash equal:\n  {left}\n  {right}"
        );
    }
}

fn assert_different_pairs(pairs: &[(&str, &str)]) {
    for (left, right) in pairs {
        assert_ne!(
            digest(left),
            digest(right),
            "different meaning must hash differently:\n  {left}\n  {right}"
        );
    }
}

/// `body` as the value of the arm `A` in a `match` inside a function.
fn arm(body: &str) -> String {
    format!("fn f(e: E, x: u8, c: bool) -> u8 {{ match e {{ A => {body} _ => 0, }} }}")
}

/// Same, with the match inside a `loop` so `break` is legal.
fn arm_in_loop(body: &str) -> String {
    format!("fn f(e: E) {{ loop {{ match e {{ A => {body} _ => {{}} }} }} }}")
}

// ---------------------------------------------------------------- the recipe name

#[test]
fn recipe_is_v2_and_heads_the_stream() {
    assert_eq!(hash::RECIPE, "specengine-hash/v2");
    let header = String::from_utf8_lossy(hash::recipe_header()).into_owned();
    assert!(header.contains("specengine-hash/v2"), "{header:?}");
    // A v1 and a v2 header can never coincide: the name is inside the digest.
    assert!(!header.contains("specengine-hash/v1"), "{header:?}");
}

// ---------------------------------------------------------------- rule (b): closures

#[test]
fn closure_body_block_around_one_expression_is_transparent() {
    assert_equal_pairs(&[
        (
            "fn f() { let g = |x| { x + 1 }; }",
            "fn f() { let g = |x| x + 1; }",
        ),
        (
            "fn f() { let g = |x| { call(x, 1) }; }",
            "fn f() { let g = |x| call(x, 1); }",
        ),
        (
            "fn f() { let g = || { { x } }; }",
            "fn f() { let g = || x; }",
        ),
        (
            "fn f() -> bool { v.iter().all(|c| { positive(c) }) }",
            "fn f() -> bool { v.iter().all(|c| positive(c)) }",
        ),
        (
            "fn f() { let g = |x| { /* c */ x + 1 }; }",
            "fn f() { let g = |x| x + 1; }",
        ),
    ]);
    // Controls: unwrapping never hides a real change.
    assert_different_pairs(&[
        (
            "fn f() { let g = |x| { x + 1 }; }",
            "fn f() { let g = |x| x + 2; }",
        ),
        (
            "fn f() { let g = |x| { x + 1 }; }",
            "fn f() { let g = |y| y + 1; }",
        ),
    ]);
}

#[test]
fn closure_bodies_that_differ_in_meaning_keep_their_hashes_apart() {
    assert_different_pairs(&[
        // `{ x; }` returns `()`, `{ x }` returns `x`.
        (
            "fn f() { let g = |x| -> u8 { x }; }",
            "fn f() { let g = |x| -> u8 { x; }; }",
        ),
        // A block with two statements is not a wrapper, and the inner block
        // is not in a body position.
        (
            "fn f() { let g = |x| { foo(); x }; }",
            "fn f() { let g = |x| { foo(); { x } }; }",
        ),
    ]);
}

// ---------------------------------------------------------------- rule (b): match arms

#[test]
fn match_arm_block_around_one_expression_is_transparent() {
    let pairs = [
        ("{ foo() }", "foo(),"),
        ("{ if c { 1 } else { 2 } }", "if c { 1 } else { 2 },"),
        ("{ unsafe { x } }", "unsafe { x },"),
        ("{ return x; }", "return x,"),
        ("{ { x } }", "x,"),
        ("{ /* c */ foo() }", "foo(),"),
        ("{ foo!() }", "foo!(),"),
        ("{ let y = 1; y }", "{ { let y = 1; y } }"),
        (
            "{ match x { 1 => 2, _ => 3 } }",
            "match x { 1 => 2, _ => 3 },",
        ),
        ("{ x.max(1) }", "x.max(1),"),
    ];
    for (block, flat) in pairs {
        assert_eq!(
            digest(&arm(block)),
            digest(&arm(flat)),
            "match_arm_blocks shapes must hash equal: `A => {block}` vs `A => {flat}`"
        );
    }
    assert_eq!(
        digest(&arm_in_loop("{ break; }")),
        digest(&arm_in_loop("break,"))
    );
    // Controls.
    assert_ne!(digest(&arm("{ foo() }")), digest(&arm("bar(),")));
    assert_ne!(digest(&arm("{ return x; }")), digest(&arm("return 1,")));
    assert_ne!(
        digest(&arm("{ let y = 1; y }")),
        digest(&arm("{ let y = 2; y }"))
    );
}

#[test]
fn match_arm_blocks_that_differ_in_meaning_keep_their_hashes_apart() {
    let pairs = [
        // `foo();` returns `()`.
        ("{ foo(); }", "foo(),"),
        // A labelled block is a `break 'a` target.
        ("{ 'a: { x } }", "x,"),
        // An attribute on the tail is a second child of the block.
        ("{ #[cfg(x)] foo() }", "foo(),"),
        // `let` changes the value.
        ("{ let y = 1; y }", "y,"),
        // `async` / `const` blocks are other node kinds, not wrappers.
        ("{ async { x } }", "x,"),
        ("{ const { x } }", "x,"),
    ];
    for (block, flat) in pairs {
        assert_ne!(
            digest(&arm(block)),
            digest(&arm(flat)),
            "`A => {block}` and `A => {flat}` differ in meaning"
        );
    }
}

#[test]
fn blocks_outside_a_body_position_are_never_unwrapped() {
    assert_different_pairs(&[
        ("fn f() { if a { x } }", "fn f() { if a { { x } } }"),
        (
            "fn f() -> u8 { if a { 1 } else { 2 } }",
            "fn f() -> u8 { if a { 1 } else { { 2 } } }",
        ),
        ("fn f() -> u8 { x }", "fn f() -> u8 { { x } }"),
        ("fn f() { loop { x } }", "fn f() { loop { { x } } }"),
        ("fn f() { while a { x } }", "fn f() { while a { { x } } }"),
        ("fn f() { let y = { x }; }", "fn f() { let y = x; }"),
    ]);
}

// ---------------------------------------------------------------- rule (a): `;` after diverging tails

#[test]
fn semicolon_after_a_diverging_tail_is_transparent() {
    assert_equal_pairs(&[
        (
            "fn f(a: bool) { loop { if a { break } else { continue } } }",
            "fn f(a: bool) { loop { if a { break; } else { continue; } } }",
        ),
        (
            "fn f(a: bool) -> u8 { if a { return 1 } 2 }",
            "fn f(a: bool) -> u8 { if a { return 1; } 2 }",
        ),
        (
            "fn f() { for x in v { if x { continue } g(x) } }",
            "fn f() { for x in v { if x { continue; } g(x) } }",
        ),
        (
            "fn f(v: &[u8]) -> u8 { for x in v { if *x > 1 { return *x } } 0 }",
            "fn f(v: &[u8]) -> u8 { for x in v { if *x > 1 { return *x; } } 0 }",
        ),
    ]);
}

#[test]
fn semicolon_after_a_value_changes_the_hash() {
    assert_different_pairs(&[
        (
            "fn f(a: bool) -> u8 { if a { 1 } else { 2 } }",
            "fn f(a: bool) -> u8 { if a { 1 } else { 2; } }",
        ),
        ("fn f() -> u8 { g() }", "fn f() -> u8 { g(); }"),
        (
            "fn f(a: bool) -> u8 { if a { x } else { y } }",
            "fn f(a: bool) -> u8 { if a { x; } else { y } }",
        ),
    ]);
}

// ---------------------------------------------------------------- rule (c): expression-macro arguments

#[test]
fn body_token_trees_inside_expression_macros_are_transparent() {
    assert_equal_pairs(&[
        (
            "fn f() { assert!(g(|x| { x > 0 })); }",
            "fn f() { assert!(g(|x| x > 0)); }",
        ),
        (
            "fn f() { assert!(g(|| { 1 })); }",
            "fn f() { assert!(g(|| 1)); }",
        ),
        ("fn f() { m!(A => { 1 }); }", "fn f() { m!(A => 1); }"),
        (
            "fn f() { let v = vec![h(|a| { a + 1 })]; }",
            "fn f() { let v = vec![h(|a| a + 1)]; }",
        ),
        (
            "fn f() { assert!(v.iter().all(|c| { positive(c, scale) })); }",
            "fn f() { assert!(v.iter().all(|c| positive(c, scale))); }",
        ),
    ]);
    // Controls: the tokens inside the braces still count.
    assert_different_pairs(&[
        (
            "fn f() { assert!(g(|x| { x > 0 })); }",
            "fn f() { assert!(g(|x| x > 1)); }",
        ),
        ("fn f() { m!(A => { 1 }); }", "fn f() { m!(A => 2); }"),
    ]);
}

#[test]
fn brace_macros_and_macro_definitions_keep_their_token_trees() {
    assert_different_pairs(&[
        // A brace-delimited invocation is never formatted by rustfmt.
        ("fn f() { n! { A => { 1 } } }", "fn f() { n! { A => 1 } }"),
        // A macro definition has no `macro_invocation` frame.
        (
            "macro_rules! d { ($a:expr) => { $a | { 1 } }; }",
            "macro_rules! d { ($a:expr) => { $a | 1 }; }",
        ),
        // A `{ }` not preceded by `|`, `||` or `=>` inside an expression macro
        // is a struct literal or a block of its own.
        ("fn f() { m!(a, { 1 }); }", "fn f() { m!(a, 1); }"),
        ("fn f() { m!(S { a: 1 }); }", "fn f() { m!(S a: 1); }"),
    ]);
}

// ---------------------------------------------------------------- rule (d): sorted `use` runs

#[test]
fn use_runs_below_the_root_hash_in_sorted_order() {
    assert_equal_pairs(&[
        (
            "fn f() { use b::B; use a::A; }",
            "fn f() { use a::A; use b::B; }",
        ),
        (
            "fn f() {\n    use b::B;\n\n    // between\n    use a::A;\n    g()\n}",
            "fn f() {\n    use a::A;\n    use b::B;\n    g()\n}",
        ),
        (
            "mod m { use b::B; use a::A; }",
            "mod m { use a::A; use b::B; }",
        ),
        (
            "mod m {\n    use c::C;\n    /* x */\n    use b::B;\n\n    use a::A;\n    fn g() {}\n}",
            "mod m {\n    use a::A;\n    use b::B;\n    use c::C;\n    fn g() {}\n}",
        ),
        (
            "mod m { use b::{X, Y}; pub use a::A; }",
            "mod m { pub use a::A; use b::{X, Y}; }",
        ),
    ]);
    // Controls: the set of imports still counts.
    assert_different_pairs(&[
        (
            "fn f() { use b::B; use a::A; }",
            "fn f() { use b::B; use a::C; }",
        ),
        ("mod m { use a::A; use b::B; }", "mod m { use a::A; }"),
    ]);
}

#[test]
fn use_declarations_separated_by_a_statement_are_separate_runs() {
    assert_different_pairs(&[
        (
            "fn f() { use a::A; let x = 1; use b::B; }",
            "fn f() { use b::B; let x = 1; use a::A; }",
        ),
        (
            "mod m { use a::A; fn g() {} use b::B; }",
            "mod m { use b::B; fn g() {} use a::A; }",
        ),
    ]);
}

#[test]
fn sorted_use_run_is_deterministic_across_parser_instances() {
    let source = "mod m {\n    use z::Z;\n    use b::B;\n    use a::A;\n}";
    let digests: Vec<Digest> = (0..3).map(|_| digest(source)).collect();
    assert!(digests.windows(2).all(|w| w[0] == w[1]), "{digests:?}");
}

// ---------------------------------------------------------------- rule (d): attributes travel with their `use`

/// Every pair as the contents of a module and of a function body: rule (d)
/// applies below the root, wherever a run of `use` declarations can stand.
fn in_mod_and_fn(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    let wraps: [fn(&str) -> String; 2] = [
        |body| format!("mod m {{ {body} }}"),
        |body| format!("fn f() {{ {body} }}"),
    ];
    pairs
        .iter()
        .flat_map(|(left, right)| wraps.iter().map(move |wrap| (wrap(left), wrap(right))))
        .collect()
}

fn as_refs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(left, right)| (left.as_str(), right.as_str()))
        .collect()
}

/// Named mutation: bind attributes to the ordinary walk again (members = bare
/// `use`, attributes emitted in source order before the sorted run) → both
/// sides become `attr ‖ sorted(b, c)` and this test goes red.
#[test]
fn cfg_gated_use_does_not_collide_with_the_gate_on_its_neighbour() {
    assert_different_pairs(&as_refs(&in_mod_and_fn(&[(
        "#[cfg(feature = \"x\")] use b; use c;",
        "#[cfg(feature = \"x\")] use c; use b;",
    )])));
}

#[test]
fn attributes_on_a_use_change_the_hash_with_their_declaration() {
    assert_different_pairs(&as_refs(&in_mod_and_fn(&[
        // A different gate.
        ("#[cfg(x)] use b; use c;", "#[cfg(y)] use b; use c;"),
        // No gate at all.
        ("#[cfg(x)] use b; use c;", "use b; use c;"),
        // The gate on the other member.
        ("#[cfg(x)] use b; use c;", "use b; #[cfg(x)] use c;"),
        // An attributed item between two `use` ends the run: the runs stay in place.
        (
            "use b; #[derive(Debug)] struct S; use a;",
            "use a; #[derive(Debug)] struct S; use b;",
        ),
    ])));
    // A statement ends the run too, attributes or not.
    assert_different_pairs(&[(
        "fn f() { #[cfg(x)] use b; let x = 1; use a; }",
        "fn f() { use a; let x = 1; #[cfg(x)] use b; }",
    )]);
}

#[test]
fn rustfmt_moving_an_attributed_use_keeps_the_hash() {
    // `reorder_imports` moves `#[cfg] use c;` past `use b;` as one unit.
    assert_equal_pairs(&as_refs(&in_mod_and_fn(&[(
        "#[cfg(x)] use c; use b;",
        "use b; #[cfg(x)] use c;",
    )])));
    assert_equal_pairs(&[
        // A comment between the attribute and its declaration is invisible.
        (
            "mod m { #[cfg(x)] /* c */ use c; use b; }",
            "mod m { use b; #[cfg(x)] use c; }",
        ),
        // Several attributes move together, in their order.
        (
            "mod m { #[cfg(x)] #[allow(unused)] use c; use b; }",
            "mod m { use b; #[cfg(x)] #[allow(unused)] use c; }",
        ),
        // Three members, two of them gated.
        (
            "mod m { #[cfg(x)] use c; use b; #[cfg(y)] use a; }",
            "mod m { #[cfg(y)] use a; use b; #[cfg(x)] use c; }",
        ),
        // Attributes on the item that ends the run belong to that item, not to the run.
        (
            "mod m { use b; use a; #[derive(Debug)] struct S; }",
            "mod m { use a; use b; #[derive(Debug)] struct S; }",
        ),
    ]);
}

/// `bytes` as a Rust byte-string literal, so a mismatch prints the new stream
/// in a form that can be read and, deliberately, pasted.
fn literal(bytes: &[u8]) -> String {
    let body: String = bytes
        .iter()
        .flat_map(|byte| std::ascii::escape_default(*byte))
        .map(char::from)
        .collect();
    format!("b\"{body}\"")
}

/// The normalized stream of a `use` run without attributes is pinned byte for
/// byte: attributes joined the run member inside v2 without a rebase, on the
/// promise that unattributed input streams exactly as before. Any drift here
/// means every recorded v2 digest of such an item silently changed.
#[test]
fn unattributed_use_run_stream_is_pinned() {
    let source =
        "mod m {\n    use c::C;\n    /* x */\n    use b::B;\n\n    use a::A;\n    fn g() {}\n}";
    let tree = parse(source);
    let items = top_level_items(tree.root_node());
    assert_eq!(items.len(), 1);
    let mut stream: Vec<u8> = Vec::new();
    hash::normalize(
        items[0],
        source.as_bytes(),
        hash::name_node(items[0]),
        &mut stream,
    );
    // One line per node frame; `\` at the end of a line joins the next one.
    const PINNED: &[u8] = b"(\x08\x00\x00\x00mod_item\x03\x00\x00\x00mod\x03\x00\x00\x00mod\
        (\x10\x00\x00\x00declaration_list\x01\x00\x00\x00{\x01\x00\x00\x00{\
        (\x0f\x00\x00\x00use_declaration\x03\x00\x00\x00use\x03\x00\x00\x00use\
        (\x11\x00\x00\x00scoped_identifier\n\x00\x00\x00identifier\x01\x00\x00\x00a\x02\x00\x00\x00::\x02\x00\x00\x00::\n\x00\x00\x00identifier\x01\x00\x00\x00A)\
        \x01\x00\x00\x00;\x01\x00\x00\x00;)\
        (\x0f\x00\x00\x00use_declaration\x03\x00\x00\x00use\x03\x00\x00\x00use\
        (\x11\x00\x00\x00scoped_identifier\n\x00\x00\x00identifier\x01\x00\x00\x00b\x02\x00\x00\x00::\x02\x00\x00\x00::\n\x00\x00\x00identifier\x01\x00\x00\x00B)\
        \x01\x00\x00\x00;\x01\x00\x00\x00;)\
        (\x0f\x00\x00\x00use_declaration\x03\x00\x00\x00use\x03\x00\x00\x00use\
        (\x11\x00\x00\x00scoped_identifier\n\x00\x00\x00identifier\x01\x00\x00\x00c\x02\x00\x00\x00::\x02\x00\x00\x00::\n\x00\x00\x00identifier\x01\x00\x00\x00C)\
        \x01\x00\x00\x00;\x01\x00\x00\x00;)\
        (\r\x00\x00\x00function_item\x02\x00\x00\x00fn\x02\x00\x00\x00fn\n\x00\x00\x00identifier\x01\x00\x00\x00g\
        (\n\x00\x00\x00parameters\x01\x00\x00\x00(\x01\x00\x00\x00(\x01\x00\x00\x00)\x01\x00\x00\x00))\
        (\x05\x00\x00\x00block\x01\x00\x00\x00{\x01\x00\x00\x00{\x01\x00\x00\x00}\x01\x00\x00\x00}))\
        \x01\x00\x00\x00}\x01\x00\x00\x00}))";
    assert!(
        stream == PINNED,
        "the v2 stream of an unattributed `use` run drifted:\n  {}",
        literal(&stream)
    );
}
