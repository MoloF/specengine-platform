//! docs/features/symbol-discriminator.md (6a-1), the display signature:
//! RD-2's tests by the names the spec gives them, TE-2's signature cases
//! (one per `BODY_FIELDS` row, read from the exported table; a multi-line
//! `where`; a 600-byte signature with a multi-byte char at the cut) and
//! TE-3's signatures under LF and CRLF. Inputs are inline sources; an item
//! is found in the comment binding's analysis (`bind_file`, the indexes
//! `item_signatures` takes) by its kind and label.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

use specengine_code::{
    BODY_FIELDS, ItemRecord, RustParser, SIGNATURE_CAP_BYTES, bind_file, item_signatures,
};

/// "Text": the cap, in bytes, `…` included.
const CAP: usize = 512;

/// What a cut signature ends with.
const ELLIPSIS: char = '\u{2026}';

fn parser() -> RustParser {
    RustParser::new().expect("the pinned grammar loads")
}

/// The items of `source` and the signatures of the items at `asked`.
fn signatures_at(source: &str, asked: &[usize]) -> (Vec<ItemRecord>, Vec<Option<String>>) {
    let mut parser = parser();
    let tree = parser.parse(source).expect("the parser gives a tree");
    let items = bind_file(&mut parser, source)
        .expect("the parser gives a tree")
        .analysis
        .items;
    (items, item_signatures(&tree, source, asked))
}

/// Every item of `source` with its signature, in item order.
fn all_signatures(source: &str) -> Vec<(ItemRecord, Option<String>)> {
    let count = signatures_at(source, &[]).0.len();
    let asked: Vec<usize> = (0..count).collect();
    let (items, signatures) = signatures_at(source, &asked);
    assert_eq!(signatures.len(), items.len(), "one answer per index asked");
    items.into_iter().zip(signatures).collect()
}

/// The signature of the one item of `kind` labelled `label` in `source`.
fn signature(source: &str, kind: &str, label: &str) -> Option<String> {
    let found: Vec<Option<String>> = all_signatures(source)
        .into_iter()
        .filter(|(item, _)| item.kind == kind && item.label == label)
        .map(|(_, signature)| signature)
        .collect();
    assert_eq!(found.len(), 1, "one {kind} {label} in {source:?}");
    found.into_iter().next().expect("one")
}

fn some(text: &str) -> Option<String> {
    Some(text.to_owned())
}

// ---------------------------------------------------- TE-2: BODY_FIELDS

/// `(kind, label, source, signature)`: one case per `BODY_FIELDS` row, each
/// with attributes or docs where the kind takes them, so the signature's
/// start is tested too.
const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "function_item",
        "step",
        "/// Steps.\n#[inline]\npub fn step(count: u32, by: u32) -> u32 {\n    count + by\n}\n",
        "pub fn step(count: u32, by: u32) -> u32",
    ),
    (
        "struct_item",
        "Tally",
        "/// A count.\n#[derive(Debug, Default)]\npub struct Tally<T: Copy> {\n    count: T,\n}\n",
        "pub struct Tally<T: Copy>",
    ),
    (
        "enum_item",
        "Size",
        "pub enum Size {\n    One,\n    Many(u32),\n}\n",
        "pub enum Size",
    ),
    (
        "union_item",
        "Bits",
        "#[repr(C)]\npub union Bits {\n    word: u32,\n    bytes: [u8; 4],\n}\n",
        "pub union Bits",
    ),
    (
        "trait_item",
        "Limit",
        "pub trait Limit: Clone {\n    fn max(&self) -> u32;\n}\n",
        "pub trait Limit: Clone",
    ),
    (
        "impl_item",
        "<Tally as fmt::Display>",
        "impl fmt::Display for Tally {\n    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n        Ok(())\n    }\n}\n",
        "impl fmt::Display for Tally",
    ),
    (
        "const_item",
        "CAP",
        "/// The cap.\npub const CAP: u32 = 10;\n",
        "pub const CAP: u32",
    ),
    (
        "static_item",
        "COUNT",
        "pub static mut COUNT: u32 = 0;\n",
        "pub static mut COUNT: u32",
    ),
    (
        "type_item",
        "Id",
        "pub type Id<T> = Vec<T>;\n",
        "pub type Id<T>",
    ),
    (
        "mod_item",
        "tests",
        "#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n",
        "mod tests",
    ),
    (
        "macro_definition",
        "twice",
        "#[macro_export]\nmacro_rules! twice {\n    ($e:expr) => {\n        $e + $e\n    };\n}\n",
        "macro_rules! twice",
    ),
    (
        "function_signature_item",
        "max",
        "pub trait Limit {\n    /// The max.\n    fn max(&self) -> u32;\n}\n",
        "fn max(&self) -> u32",
    ),
];

/// TE-2: a signature for every `BODY_FIELDS` row, the case looked up by the
/// row's kind, so a row without a case (a grammar bump adding one) fails
/// here, and no case outlives its row. M: a kind left out.
#[test]
fn signature_covers_every_body_field_row() {
    let mut missing = Vec::new();
    let mut failures = Vec::new();
    for row in BODY_FIELDS {
        let Some((kind, label, source, want)) = CASES.iter().find(|case| case.0 == row.kind) else {
            missing.push(row.kind);
            continue;
        };
        let got = signature(source, kind, label);
        if got.as_deref() != Some(*want) {
            failures.push(format!("{kind}: got {got:?}, want {want:?}"));
        }
    }
    assert!(
        missing.is_empty(),
        "BODY_FIELDS rows without a signature case: {missing:?}"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    for (kind, ..) in CASES {
        assert!(
            BODY_FIELDS.iter().any(|row| row.kind == *kind),
            "a case for {kind}, which BODY_FIELDS has no row for"
        );
    }
}

// ------------------------------------------------------------------ RD-2

/// RD-2 ("Span"): the signature runs up to the body, exclusive, for every
/// kind that has one, members included; nothing of a body shows. M: the
/// body kept.
#[test]
fn signature_stops_at_the_body() {
    let source = "\
pub fn run(x: u32) -> u32 {
    let secret = x + 1;
    secret
}

pub struct Gauge {
    secret: u32,
}

pub enum Mode {
    Secret,
}

pub trait Meter {
    fn secret(&self) -> u32;
}

impl Gauge {
    pub fn read(&self) -> u32 {
        self.secret
    }
}

pub mod inner {
    pub fn secret() {}
}
";
    let all = all_signatures(source);
    let got: Vec<(&str, &str, Option<&str>)> = all
        .iter()
        .map(|(item, signature)| {
            (
                item.kind.as_str(),
                item.label.as_str(),
                signature.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("function_item", "run", Some("pub fn run(x: u32) -> u32")),
            ("struct_item", "Gauge", Some("pub struct Gauge")),
            ("enum_item", "Mode", Some("pub enum Mode")),
            ("trait_item", "Meter", Some("pub trait Meter")),
            (
                "function_signature_item",
                "secret",
                Some("fn secret(&self) -> u32")
            ),
            ("impl_item", "Gauge", Some("impl Gauge")),
            ("function_item", "read", Some("pub fn read(&self) -> u32")),
            ("mod_item", "inner", Some("pub mod inner")),
            ("function_item", "secret", Some("pub fn secret()")),
        ]
    );
}

/// "Span": an item without a body is the whole item minus a final `;`; a
/// tuple struct's fields are its body.
#[test]
fn bodyless_items_drop_the_final_semicolon() {
    let source = "\
pub struct Unit;

pub struct Pair(pub u32, u32);

mod outer;

pub trait Meter {
    fn read(&self) -> u32;
}
";
    assert_eq!(
        signature(source, "struct_item", "Unit"),
        some("pub struct Unit")
    );
    assert_eq!(
        signature(source, "struct_item", "Pair"),
        some("pub struct Pair")
    );
    assert_eq!(signature(source, "mod_item", "outer"), some("mod outer"));
    assert_eq!(
        signature(source, "function_signature_item", "read"),
        some("fn read(&self) -> u32")
    );
}

/// "Not the `sig` level" (Q9): the signature keeps the name and leaves the
/// item's attributes and doc comments out.
#[test]
fn signature_keeps_the_name_and_leaves_attributes_out() {
    let source = "/// A gauge.\n#[derive(Debug)]\n#[cfg(test)]\n// A note.\npub struct Gauge;\n";
    assert_eq!(
        signature(source, "struct_item", "Gauge"),
        some("pub struct Gauge")
    );
}

/// RD-2 ("Text"), the owner's S2: comments inside the span are dropped,
/// line and block, each counting as whitespace, so the runs fold as any
/// other and no two tokens fuse (`pub/*c*/fn` is `pub fn`). M: comments
/// kept; a comment dropped without a space.
#[test]
fn signature_drops_comments() {
    let source = "\
pub fn run( // the input
    x: u32, /* why */ y: u32
) -> u32 /* returns */ {
    x + y
}

pub/*c*/fn glued(/*a*/x: u32/*b*/) {}
";
    assert_eq!(
        signature(source, "function_item", "run"),
        some("pub fn run(x: u32, y: u32) -> u32")
    );
    assert_eq!(
        signature(source, "function_item", "glued"),
        some("pub fn glued(x: u32)")
    );
}

/// RD-2 ("Text"), TE-2 (a multi-line `where`): every whitespace run is one
/// space, none after `(` or `[` and none before `)`, `]` or `,`; no rule
/// drops a comma, so rustfmt's trailing ones stay (reading L4).
#[test]
fn multi_line_where_folds_to_one_line() {
    let source = "\
pub fn run<T, U>(
    left: T,
    right: U,
) -> (T, U)
where
    T: Clone + Send,
    U: Default,
{
    (left, right)
}

impl<T> Holder<T>
where
    T: Copy,
{
    fn get(&self) {}
}

pub fn pick(
    values: [
        u8; 4
    ],
) -> u8 {
    values[0]
}
";
    assert_eq!(
        signature(source, "function_item", "run"),
        some("pub fn run<T, U>(left: T, right: U,) -> (T, U) where T: Clone + Send, U: Default,")
    );
    assert_eq!(
        signature(source, "impl_item", "Holder<T>"),
        some("impl<T> Holder<T> where T: Copy,")
    );
    assert_eq!(
        signature(source, "function_item", "pick"),
        some("pub fn pick(values: [u8; 4],) -> u8")
    );
}

/// RD-2, TE-3: CR is whitespace, so a CRLF checkout gives the LF
/// signature, byte for byte. M: CR kept.
#[test]
fn signature_is_equal_under_lf_and_crlf() {
    let source = "\
pub fn run<T>(
    left: T, // the left
    right: T
) -> T
where
    T: Clone
{
    left
}
";
    let lf = signature(source, "function_item", "run");
    assert_eq!(
        lf,
        some("pub fn run<T>(left: T, right: T) -> T where T: Clone")
    );
    let crlf = signature(&source.replace('\n', "\r\n"), "function_item", "run");
    assert_eq!(crlf, lf);
}

/// RD-2 ("Span"): a `const`'s and a `static`'s body is the `value` field,
/// a `type`'s the `type` field, so the trailing `=` goes. M: `=` kept.
#[test]
fn const_signature_drops_the_value() {
    let source = "\
pub const CAP: u32 = 10;

const N: usize = {
    1 + 2
};

pub static NAME: &str = \"tally\";

type Id = u64;
";
    assert_eq!(
        signature(source, "const_item", "CAP"),
        some("pub const CAP: u32")
    );
    assert_eq!(signature(source, "const_item", "N"), some("const N: usize"));
    assert_eq!(
        signature(source, "static_item", "NAME"),
        some("pub static NAME: &str")
    );
    assert_eq!(signature(source, "type_item", "Id"), some("type Id"));
}

/// RD-2 ("Span"): a `macro_rules!` definition is `macro_rules! <name>`,
/// its rules and its attribute left out. M: the macro body kept.
#[test]
fn macro_signature_ends_at_its_name() {
    let source = "\
#[macro_export]
macro_rules! twice {
    ($e:expr) => {
        $e + $e
    };
    ($e:expr, $f:expr) => {
        $e + $f
    };
}
";
    assert_eq!(
        signature(source, "macro_definition", "twice"),
        some("macro_rules! twice")
    );
}

/// RD-2 ("Signature", `code-bindings.md` rule 7): `None` for an item with a
/// parse error in itself or in an attached attribute, and only for those;
/// fixed, they have one. M: broken text rendered.
#[test]
fn broken_item_has_no_signature() {
    let source = "fn good() -> u8 { 1 }\nfn bad(x: u8) -> u8 { let y = 1 2; x }\n\
                  #[derive(Debug,]\nstruct Torn;\n";
    let all = all_signatures(source);
    for (item, signature) in &all {
        assert_eq!(
            signature.is_none(),
            item.has_error,
            "{} {}: {signature:?}",
            item.kind,
            item.label
        );
    }
    assert_eq!(
        signature(source, "function_item", "good"),
        some("fn good() -> u8")
    );
    assert_eq!(signature(source, "function_item", "bad"), None);
    assert_eq!(signature(source, "struct_item", "Torn"), None);
    let fixed = source
        .replace("1 2;", "1;")
        .replace("#[derive(Debug,]", "#[derive(Debug)]");
    assert_eq!(
        signature(&fixed, "function_item", "bad"),
        some("fn bad(x: u8) -> u8")
    );
    assert_eq!(
        signature(&fixed, "struct_item", "Torn"),
        some("struct Torn")
    );
}

/// RD-2 ("Text"), TE-2 (a 600-byte signature with a multi-byte char at the
/// cut), reading L5: over 512 bytes the text is cut at the last char
/// boundary that leaves room for `…` (3 bytes) and `…` appended, the whole
/// within 512; a signature of exactly 512 bytes is kept whole, 513 is cut.
/// M: a byte cut.
#[test]
fn long_signature_is_cut_at_a_char_boundary() {
    assert_eq!(SIGNATURE_CAP_BYTES, 512);
    let room = CAP - ELLIPSIS.len_utf8();
    // `fn f(xy: [u8; "€…€ab".len()]) -> u32`: 15 + 189 * 3 + 2 + 16 bytes.
    let text = format!(
        "fn f(xy: [u8; \"{}ab\".len()]) -> u32",
        "\u{20AC}".repeat(189)
    );
    assert_eq!(text.len(), 600);
    assert!(!text.is_char_boundary(room), "a char spans the cut");
    assert!(!text.is_char_boundary(CAP), "a char spans the cap");
    let mut cut = room;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    assert_eq!(cut, 507);
    let want = format!("{}{ELLIPSIS}", &text[..cut]);
    let got =
        signature(&format!("{text} {{\n    0\n}}\n"), "function_item", "f").expect("a signature");
    assert_eq!(got, want);
    assert!(got.len() <= CAP, "{} bytes", got.len());

    // At the cap: kept whole. One byte over: cut at `room`, an ASCII
    // boundary.
    let at_cap = format!("fn {}()", "a".repeat(CAP - 5));
    assert_eq!(at_cap.len(), CAP);
    let name = "a".repeat(CAP - 5);
    assert_eq!(
        signature(&format!("{at_cap} {{}}\n"), "function_item", &name),
        Some(at_cap.clone())
    );
    let over = format!("fn {}()", "a".repeat(CAP - 4));
    let name = "a".repeat(CAP - 4);
    let got = signature(&format!("{over} {{}}\n"), "function_item", &name).expect("a signature");
    assert_eq!(got, format!("{}{ELLIPSIS}", &over[..room]));
    assert_eq!(got.len(), CAP);
}

/// RD-2 ("Entry"): the answers follow the indexes asked, repeats included,
/// `None` for an index past the items; nothing asked, nothing answered. M:
/// the order asked ignored, or an index past its items answered.
#[test]
fn item_signatures_follow_the_indexes_asked() {
    let source = "pub fn a() {}\npub struct B;\npub const C: u8 = 1;\n";
    let (items, got) = signatures_at(source, &[2, 0, 3, 1, 0, 99]);
    assert_eq!(items.len(), 3);
    assert_eq!(
        got,
        [
            some("pub const C: u8"),
            some("pub fn a()"),
            None,
            some("pub struct B"),
            some("pub fn a()"),
            None,
        ]
    );
    let (_, none) = signatures_at(source, &[]);
    assert!(none.is_empty());
}
