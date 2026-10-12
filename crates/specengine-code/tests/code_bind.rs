//! docs/features/code-refs.md TE-2 and RD-3 ("Binding rules"): every rule
//! and every unanchored category of a Rust comment's binding is a row of
//! [`ROWS`], each run on the source as written (LF) and with every line end
//! turned to CRLF at test time, each naming the mutation that turns it red.
//! The five RD-3 mutations have a test of their own name.
//!
//! A binding is rendered without the file's package (that is `naming`'s,
//! checked end to end by `crates/specengine-cli/tests/code_refs.rs` over
//! `fixtures/code-a`): `item <mod::owner::name> <kind>`, `module`,
//! `module <mod>`, `unanchored <category>`, `cannot_verify <category>`,
//! `file`.

use specengine_code::{
    Binding, HashState, ItemRecord, Relation, RustBinding, RustParser, analyze_tree, bind_file,
    bind_tree, markers_in,
};

fn bind(source: &str) -> RustBinding {
    let mut parser = RustParser::new().expect("the pinned grammar loads");
    bind_file(&mut parser, source).expect("the parser gives a tree")
}

fn item_path(item: &ItemRecord) -> String {
    let mut parts: Vec<String> = item.mod_path.clone();
    parts.extend(item.owner.clone());
    parts.push(if item.kind == "impl_item" {
        format!("impl {}", item.label)
    } else {
        item.label.clone()
    });
    parts.join("::")
}

fn render(binding: Binding, bound: &RustBinding) -> String {
    let items = &bound.analysis.items;
    match binding {
        Binding::Item(index) => {
            let item = &items[index];
            format!("item {} {}", item_path(item), item.kind)
        }
        Binding::Module(None) => "module".to_owned(),
        Binding::Module(Some(index)) => format!("module {}", item_path(&items[index])),
        Binding::Unanchored(category) => format!("unanchored {}", category.as_str()),
        Binding::CannotVerify(category) => format!("cannot_verify {}", category.as_str()),
        Binding::File => "file".to_owned(),
    }
}

/// The 1-based line of a byte offset.
fn line_of(source: &str, offset: usize) -> usize {
    source[..offset].bytes().filter(|&b| b == b'\n').count() + 1
}

/// `(line, marker binding, mention binding)` of every comment holding a
/// marker, in source order.
fn marker_bindings(source: &str) -> Vec<(usize, String, String)> {
    let bound = bind(source);
    bound
        .comments
        .iter()
        .filter(|comment| !markers_in(&source[comment.range.clone()]).is_empty())
        .map(|comment| {
            (
                line_of(source, comment.range.start),
                render(comment.binding, &bound),
                render(comment.mention, &bound),
            )
        })
        .collect()
}

/// `(line, mention binding)` of the comment holding `needle` (no marker).
fn mention_binding(source: &str, needle: &str) -> (usize, String) {
    let bound = bind(source);
    let comment = bound
        .comments
        .iter()
        .find(|comment| source[comment.range.clone()].contains(needle))
        .unwrap_or_else(|| panic!("no comment holds {needle:?} in:\n{source}"));
    (
        line_of(source, comment.range.start),
        render(comment.mention, &bound),
    )
}

fn crlf(source: &str) -> String {
    source.replace('\n', "\r\n")
}

/// The one marker's binding of `source`, LF and CRLF alike.
fn the_binding(source: &str) -> String {
    let lf = marker_bindings(source);
    let cr = marker_bindings(&crlf(source));
    assert_eq!(lf, cr, "LF and CRLF bind alike:\n{source}");
    assert_eq!(lf.len(), 1, "one marker comment in:\n{source}\n{lf:?}");
    lf[0].1.clone()
}

/// One rule or category: the source (one marker comment), the marker's
/// binding, and the mutation that turns the row red.
struct Row {
    covers: &'static str,
    source: &'static str,
    binding: &'static str,
    mutation: &'static str,
}

const ROWS: &[Row] = &[
    Row {
        covers: "rule 1: a block comment leading an item on its `*/` line",
        source: "/* @implements A-1 */ fn f() {}\n",
        binding: "item f function_item",
        mutation: "rule 1 dropped: the block comment trails nothing and leads nothing",
    },
    Row {
        covers: "rule 1: a multi-line block comment leads by its `*/` line",
        source: "/* @implements A-1\n   more */ pub fn f() {}\n",
        binding: "item f function_item",
        mutation: "a block comment judged by its `/*` line",
    },
    Row {
        covers: "rule 2: own line, the next token starts an item",
        source: "// @implements A-1\nfn f() {}\n",
        binding: "item f function_item",
        mutation: "rule 2 dropped",
    },
    Row {
        covers: "rule 2: crosses blank lines and comments, doc comments too",
        source: "// @implements A-1\n\n// other\n/// doc\nfn f() {}\n",
        binding: "item f function_item",
        mutation: "rule 2 stops at a blank line or a comment",
    },
    Row {
        covers: "rule 2: crosses outer attributes",
        source: "// @implements A-1\n#[derive(Debug)]\n#[allow(dead_code)]\nstruct S;\n",
        binding: "item S struct_item",
        mutation: "stop at an attribute",
    },
    Row {
        covers: "rule 2: a `mod x;` declaration is an item",
        source: "// @implements A-1\nmod x;\n",
        binding: "item x mod_item",
        mutation: "a declared mod is no item",
    },
    Row {
        covers: "rule 2: a method directly in an `impl`",
        source: "impl S {\n    // @implements A-1\n    fn m(&self) {}\n}\n",
        binding: "item S::m function_item",
        mutation: "members are no items",
    },
    Row {
        covers: "rule 2: a signature directly in a `trait`",
        source: "trait T {\n    // @implements A-1\n    fn m(&self);\n}\n",
        binding: "item T::m function_signature_item",
        mutation: "trait signatures are no items",
    },
    Row {
        covers: "rule 3: trailing an item's last token",
        source: "fn f() {} // @implements A-1\nfn g() {}\n",
        binding: "item f function_item",
        mutation: "trailing to the next item",
    },
    Row {
        covers: "rule 3: `} // m` closing an `impl`",
        source: "impl S {\n    fn m(&self) {}\n} // @implements A-1\nfn g() {}\n",
        binding: "item impl S impl_item",
        mutation: "trailing to the next item",
    },
    Row {
        covers: "rule 3: several items end on the line, the one right before",
        source: "mod m { fn f() {} } // @implements A-1\n",
        binding: "item m mod_item",
        mutation: "the first item ending on the line",
    },
    Row {
        covers: "rule 4: after an item's opening `{`",
        source: "impl S { // @implements A-1\n    fn m(&self) {}\n}\n",
        binding: "item impl S impl_item",
        mutation: "rule 4 dropped: binds `S::m` by rule 2 or nothing",
    },
    Row {
        covers: "rule 5: `//!` at file level, the file's module",
        source: "//! @implements A-1\nuse a::b;\n",
        binding: "module",
        mutation: "inner doc comments judged as plain comments",
    },
    Row {
        covers: "rule 5 before rule 2 (deviation 1): `//!` above an item is the module's",
        source: "//! @implements A-1\nfn f() {}\n",
        binding: "module",
        mutation: "rule 5 judged after rule 2: binds `f`",
    },
    Row {
        covers: "rule 5: `//!` directly in an inline `mod`, that mod",
        source: "mod m {\n    //! @implements A-1\n    use a::b;\n}\n",
        binding: "module m",
        mutation: "an inline mod's inner doc to the file",
    },
    Row {
        covers: "rule 5: `/*!` at file level",
        source: "/*! @implements A-1 */\nuse a::b;\n",
        binding: "module",
        mutation: "`/*!` judged as a plain block comment",
    },
    Row {
        covers: "rule 6 in_macro: a `macro_rules!` body",
        source: "macro_rules! m {\n    // @implements A-1\n    () => {};\n}\n",
        binding: "unanchored in_macro",
        mutation: "macro bodies judged as bodies or items",
    },
    Row {
        covers: "rule 6 in_macro: a macro call's token tree, before in_body",
        source: "fn f() {\n    foo! {\n        // @implements A-1\n        x\n    }\n}\n",
        binding: "unanchored in_macro",
        mutation: "in_body judged before in_macro",
    },
    Row {
        covers: "rule 6 in_body: trailing a statement",
        source: "fn f() {\n    let x = 1; // @implements A-1\n}\n",
        binding: "unanchored in_body",
        mutation: "body to its fn",
    },
    Row {
        covers: "rule 6 in_body: an `fn` in a body is no item",
        source: "fn f() {\n    // @implements A-1\n    fn g() {}\n}\n",
        binding: "unanchored in_body",
        mutation: "items collected in function bodies",
    },
    Row {
        covers: "rule 6 in_body: a `const` initializer",
        source: "const C: u32 =\n    // @implements A-1\n    1;\n",
        binding: "unanchored in_body",
        mutation: "initializers judged at item level",
    },
    Row {
        covers: "rule 6 no_item: the file's end",
        source: "fn f() {}\n// @implements A-1\n",
        binding: "unanchored no_item",
        mutation: "no_item folded into not_an_item",
    },
    Row {
        covers: "rule 6 no_item: before `}`",
        source: "impl S {\n    fn m(&self) {}\n    // @implements A-1\n}\n",
        binding: "unanchored no_item",
        mutation: "a closer crossed to the next item",
    },
    Row {
        covers: "rule 6 not_an_item: a `use`",
        source: "// @implements A-1\nuse a::b;\nfn f() {}\n",
        binding: "unanchored not_an_item",
        mutation: "cross a use",
    },
    Row {
        covers: "rule 6 not_an_item: `extern crate`",
        source: "// @implements A-1\nextern crate a;\nfn f() {}\n",
        binding: "unanchored not_an_item",
        mutation: "cross an `extern crate`",
    },
    Row {
        covers: "rule 6 not_an_item: an item-position macro call",
        source: "// @implements A-1\nthread_local! { static X: u32 = 1; }\nfn f() {}\n",
        binding: "unanchored not_an_item",
        mutation: "a macro call taken for an item",
    },
    Row {
        covers: "rule 6 not_an_item: a field",
        source: "struct S {\n    // @implements A-1\n    a: u32,\n}\n",
        binding: "unanchored not_an_item",
        mutation: "fields bound",
    },
    Row {
        covers: "rule 6 not_an_item: a variant",
        source: "enum E {\n    // @implements A-1\n    A,\n}\n",
        binding: "unanchored not_an_item",
        mutation: "variants bound",
    },
    Row {
        covers: "rule 6 not_an_item: an associated `const`",
        source: "impl S {\n    // @implements A-1\n    const C: u32 = 0;\n}\n",
        binding: "unanchored not_an_item",
        mutation: "associated consts bound",
    },
    Row {
        covers: "rule 6 not_an_item: an associated `type`",
        source: "impl T for S {\n    // @implements A-1\n    type X = u32;\n}\n",
        binding: "unanchored not_an_item",
        mutation: "associated types bound",
    },
    Row {
        covers: "rule 6 not_an_item: `#![..]`",
        source: "// @implements A-1\n#![forbid(unsafe_code)]\nfn f() {}\n",
        binding: "unanchored not_an_item",
        mutation: "an inner attribute crossed as an outer one",
    },
    Row {
        covers: "rule 7: an item with a parse error",
        source: "// @implements A-1\nfn f() -> u32 {\n    let x = 1 2;\n    x\n}\n",
        binding: "cannot_verify other",
        mutation: "drop has_error",
    },
    Row {
        covers: "rule 7: a trailing marker on a broken item",
        source: "fn f() -> u32 {\n    let x = 1 2;\n    x\n} // @implements A-1\n",
        binding: "cannot_verify other",
        mutation: "drop has_error (rule 3)",
    },
    Row {
        covers: "rule 7: the hash's first category, not a default (`~` is macro punctuation)",
        source: "// @implements A-1\nfn f() -> u32 {\n    let x = 1 ~ 2;\n    x\n}\n",
        binding: "cannot_verify macro_punct",
        mutation: "the binder's analysis skips `hash_item` for broken items too (categories lost)",
    },
    Row {
        covers: "rule 6 not_an_item: inside an attribute's arguments, no macro",
        source: "#[cfg(any( // @implements A-1\n    unix,\n))]\nfn f() {}\n",
        binding: "unanchored not_an_item",
        mutation: "`token_tree` back in MACRO_KINDS (`in_macro`)",
    },
];

#[test]
fn every_binding_rule_and_category_is_a_row_lf_and_crlf() {
    let mut failures = Vec::new();
    for row in ROWS {
        let lf = marker_bindings(row.source);
        let cr = marker_bindings(&crlf(row.source));
        let got: Vec<&str> = lf.iter().map(|(_, binding, _)| binding.as_str()).collect();
        if got != [row.binding] || lf != cr {
            failures.push(format!(
                "{} (red under: {})\n  source: {:?}\n  want:   {}\n  LF:     {lf:?}\n  CRLF:   {cr:?}",
                row.covers, row.mutation, row.source, row.binding
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The table covers every rule and every category the spec names.
#[test]
fn the_table_names_every_rule_and_category() {
    for needle in [
        "rule 1",
        "rule 2",
        "rule 3",
        "rule 4",
        "rule 5",
        "rule 7",
        "in_macro",
        "in_body",
        "no_item",
        "not_an_item",
    ] {
        assert!(
            ROWS.iter().any(|row| row.covers.contains(needle)),
            "no row covers {needle}"
        );
    }
    for row in ROWS {
        assert!(!row.mutation.is_empty(), "{} names no mutation", row.covers);
    }
}

// ------------------------------------------------------------ RD-3, named

/// M: cross a `use` (rule 2 passing it as it passes an attribute) binds
/// `f`.
#[test]
fn marker_above_use_is_unanchored() {
    assert_eq!(
        the_binding("// @implements A-1\nuse a::b;\n\nfn f() {}\n"),
        "unanchored not_an_item"
    );
}

/// M: stop at an attribute (rule 2 not crossing `#[..]`) leaves it
/// `not_an_item`.
#[test]
fn marker_above_derive_binds_the_struct() {
    assert_eq!(
        the_binding(
            "// @implements A-1\n#[derive(Debug, Clone)]\npub struct S {\n    a: u32,\n}\n"
        ),
        "item S struct_item"
    );
}

/// M: trailing to the next item binds `g`.
#[test]
fn trailing_marker_binds_its_line_item() {
    assert_eq!(
        the_binding("pub const C: u32 = 10; // @configures A-1\n\npub fn g() {}\n"),
        "item C const_item"
    );
    assert_eq!(
        the_binding("impl S {\n    fn m(&self) {}\n} // @implements A-1\nimpl S {}\n"),
        "item impl S impl_item"
    );
}

/// M: body to its fn binds `f` instead of naming the body.
#[test]
fn marker_in_body_is_unanchored() {
    let source = "pub fn f() -> u32 {\n    let x = 1; // @implements A-1\n    // @implements A-2\n    x\n}\n";
    let lf = marker_bindings(source);
    assert_eq!(lf, marker_bindings(&crlf(source)));
    let got: Vec<&str> = lf.iter().map(|(_, b, _)| b.as_str()).collect();
    assert_eq!(got, ["unanchored in_body", "unanchored in_body"]);
    // A mention there belongs to the enclosing item, the marker not.
    let mentions: Vec<&str> = lf.iter().map(|(_, _, m)| m.as_str()).collect();
    assert_eq!(mentions, ["item f function_item", "item f function_item"]);
}

/// M: drop `has_error` binds the broken `fn`; its neighbour stays bound.
#[test]
fn marker_on_broken_item_cannot_verify() {
    let source = "// @implements A-1\npub fn broken() -> u32 {\n    let x = 1 2;\n    x\n}\n\n// @implements A-2\npub fn intact() -> u32 {\n    7\n}\n";
    let lf = marker_bindings(source);
    assert_eq!(lf, marker_bindings(&crlf(source)));
    let got: Vec<&str> = lf.iter().map(|(_, b, _)| b.as_str()).collect();
    assert_eq!(got, ["cannot_verify other", "item intact function_item"]);
}

// ---------------------------------------------------- mentions' places

/// "Mentions": a doc comment's mention belongs to its item; an unanchored
/// comment's to the innermost enclosing item, else the file.
#[test]
fn mentions_belong_to_the_binding_the_enclosing_item_or_the_file() {
    let source = "/// The A-1 count.\npub struct S;\n\nimpl S {\n    pub fn f(&self) {\n        // keeps A-2\n    }\n}\n\nuse a::b; // A-3 at a use\n// A-4 at the end\n";
    for text in [source.to_owned(), crlf(source)] {
        assert_eq!(
            mention_binding(&text, "A-1"),
            (1, "item S struct_item".to_owned())
        );
        assert_eq!(
            mention_binding(&text, "A-2"),
            (6, "item S::f function_item".to_owned())
        );
        assert_eq!(mention_binding(&text, "A-3"), (10, "file".to_owned()));
        assert_eq!(mention_binding(&text, "A-4"), (11, "file".to_owned()));
    }
}

/// A mention in an unanchored comment inside a broken item: that item's
/// `cannot_verify`.
#[test]
fn a_mention_inside_a_broken_item_cannot_verify() {
    let source = "pub fn f() -> u32 {\n    // keeps A-1\n    let x = 1 2;\n    x\n}\n";
    assert_eq!(
        mention_binding(source, "A-1"),
        (2, "cannot_verify other".to_owned())
    );
}

/// Every comment of a file is bound once, string literals never: a
/// marker-looking string is no comment.
#[test]
fn string_literals_are_no_comments() {
    let source =
        "pub const L: &str = \"A-1 // @implements A-2\";\n// @implements A-3\npub fn f() {}\n";
    let bound = bind(source);
    let texts: Vec<&str> = bound
        .comments
        .iter()
        .map(|comment| &source[comment.range.clone()])
        .collect();
    assert_eq!(texts, ["// @implements A-3"]);
}

/// The binding of the whole of `fixtures/code-a/crates/tally/src/counter.rs`
/// is the same with CRLF line ends.
#[test]
fn the_fixture_binds_alike_lf_and_crlf() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/code-a/crates/tally/src/counter.rs");
    let source = std::fs::read_to_string(&path).expect("the fixture file");
    let lf = marker_bindings(&source);
    assert!(lf.len() > 15, "{lf:?}");
    assert_eq!(lf, marker_bindings(&crlf(&source)));
}

// ------------------------------------------- the binder's analysis (iteration 2)

/// Every `.rs` file of `fixtures/code-a`, `.`-named and excluded ones too.
fn fixture_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("fixture directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).expect("UTF-8 fixture");
                out.push((path.display().to_string(), text));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-a");
    let mut out = Vec::new();
    walk(&root, &mut out);
    out.sort();
    out
}

/// The binder analyses without hashing (`Hashing::ErrorsOnly`): over every
/// item of `fixtures/code-a` and of [`ROWS`], LF and CRLF, its items equal
/// full hashing's (`analyze_tree`) — kind, label, place, `has_error` — and
/// a broken item's state (every error category) is the same; an
/// error-free item's state is `HashState::NotHashed`, never a hash, and
/// the analysis says `hashed: false`. M: skip `hash_item` for broken
/// items too.
#[test]
fn the_binders_analysis_matches_full_hashing_on_errors() {
    let mut sources = fixture_sources();
    for row in ROWS {
        sources.push((row.covers.to_owned(), row.source.to_owned()));
    }
    let crlf_sources: Vec<(String, String)> = sources
        .iter()
        .map(|(name, text)| (format!("{name} (CRLF)"), crlf(text)))
        .collect();
    sources.extend(crlf_sources);
    let mut parser = RustParser::new().expect("grammar");
    let (mut items, mut broken) = (0, 0);
    let mut failures = Vec::new();
    for (name, source) in &sources {
        let tree = parser.parse(source).expect("a tree");
        let full = analyze_tree(&tree, source);
        let bound = bind_tree(&tree, source).analysis;
        assert!(full.hashed && !bound.hashed, "{name}");
        assert_eq!(
            (full.has_error, full.orphan_errors, &full.mod_declarations),
            (
                bound.has_error,
                bound.orphan_errors,
                &bound.mod_declarations
            ),
            "{name}"
        );
        assert_eq!(full.items.len(), bound.items.len(), "{name}");
        for (a, b) in full.items.iter().zip(&bound.items) {
            items += 1;
            let same_item = (
                &a.kind,
                &a.label,
                &a.mod_path,
                &a.owner,
                &a.byte_range,
                a.line,
                a.has_error,
            ) == (
                &b.kind,
                &b.label,
                &b.mod_path,
                &b.owner,
                &b.byte_range,
                b.line,
                b.has_error,
            );
            let same_state = if a.has_error {
                broken += 1;
                matches!(&a.state, HashState::CannotVerify { categories } if !categories.is_empty())
                    && a.state == b.state
            } else {
                matches!(a.state, HashState::Hashed(_)) && b.state == HashState::NotHashed
            };
            if !same_item || !same_state {
                failures.push(format!("{name}: {}\n  full: {a:?}\n  bind: {b:?}", a.label));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(items > 100 && broken >= 8, "{items} items, {broken} broken");
}

/// Review iteration 2, case (1): `// @implements @verifies A-1` above an
/// `fn` is two markers in one comment, and that comment binds the `fn`
/// (LF and CRLF).
#[test]
fn two_markers_in_one_comment_bind_the_next_fn() {
    let source = "// @implements @verifies A-1\npub fn f() {}\n";
    for text in [source.to_owned(), crlf(source)] {
        let bound = bind(&text);
        assert_eq!(bound.comments.len(), 1);
        let comment = &bound.comments[0];
        let markers = markers_in(&text[comment.range.clone()]);
        let got: Vec<(Relation, &str)> = markers
            .iter()
            .map(|m| (m.relation, m.id.as_str()))
            .collect();
        assert_eq!(
            got,
            [(Relation::Implements, ""), (Relation::Verifies, "A-1")]
        );
        assert_eq!(render(comment.binding, &bound), "item f function_item");
    }
}
