//! docs/features/constant-census.md (12a) RD-1 (D2, D4): the value reader
//! `specengine_code::const_value` over the items `bind_tree` collects.
//!
//! An item's value is the text between the quotes of its whole initializer
//! when that is one plain `"…"` or raw `r#*"…"#*` string literal, never
//! unescaped. Else `not_valued`, the first category that applies: `broken`
//! (`has_error`), `macro` (a macro invocation or any call), `byte_string`
//! (`b"…"`, `br"…"`, `c"…"`, `cr"…"`, a char), `escaped` (a plain literal
//! holding `\`), `multi_line` (a literal holding LF or CR), `other`. `None`
//! for any item but a `const_item` or a `static_item`; the node read is the
//! one spanning `item.byte_range`, its initializer and that literal's bytes,
//! never below.
//!
//! Every source is read as written (LF) and with every LF turned to CRLF at
//! test time; both give the same outcomes. Each test names the mutation
//! that turns it red. Values are invented (`QA-…`); nothing here names a
//! project.
//!
//! PHASE-B: `const_value`, `ConstValue` (`Valued(String)`,
//! `NotValued(NotValued)`) and `NotValued::as_str` are the spec's names,
//! re-exported at the crate root (reading L1, L2).

use std::collections::BTreeSet;
use std::thread;

use specengine_code::{ConstValue, ItemRecord, RustParser, bind_tree, const_value};

const SIXTY_FOUR_MB: usize = 64 * 1024 * 1024;
const QUARTER_MB: usize = 256 * 1024;

fn crlf(source: &str) -> String {
    source.replace('\n', "\r\n")
}

/// The reader's answer as text: `valued "<text>"` (the text in its Debug
/// form, so an empty value and a backslash show), `not_valued <category>`
/// or `none`.
fn outcome(value: Option<ConstValue>) -> String {
    match value {
        Some(ConstValue::Valued(text)) => format!("valued {text:?}"),
        Some(ConstValue::NotValued(category)) => format!("not_valued {}", category.as_str()),
        None => "none".to_owned(),
    }
}

/// `(name, kind, outcome)` of every item of `source`, in analysis order.
fn read_items(source: &str) -> Vec<(String, String, String)> {
    let tree = RustParser::new()
        .expect("the pinned grammar loads")
        .parse(source)
        .expect("the parser gives a tree");
    let bound = bind_tree(&tree, source);
    bound
        .analysis
        .items
        .iter()
        .map(|item| {
            (
                item.name.clone().unwrap_or_else(|| item.label.clone()),
                item.kind.clone(),
                outcome(const_value(&tree, source, item)),
            )
        })
        .collect()
}

/// `(name, outcome)` of every `const` and `static` item of `source`; the
/// CRLF form of `source` reads alike, every item included.
fn read(source: &str) -> Vec<(String, String)> {
    let lf = read_items(source);
    assert_eq!(
        lf,
        read_items(&crlf(source)),
        "LF and CRLF read alike:\n{source}"
    );
    lf.into_iter()
        .filter(|(_, kind, _)| kind == "const_item" || kind == "static_item")
        .map(|(name, _, outcome)| (name, outcome))
        .collect()
}

fn owned(rows: &[(&str, &str)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(name, outcome)| ((*name).to_owned(), (*outcome).to_owned()))
        .collect()
}

/// The item named `name` of `source`'s binding analysis.
fn item_named(source: &str, name: &str) -> ItemRecord {
    let tree = RustParser::new()
        .expect("the pinned grammar loads")
        .parse(source)
        .expect("the parser gives a tree");
    bind_tree(&tree, source)
        .analysis
        .items
        .into_iter()
        .find(|item| item.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("{name} is an item of:\n{source}"))
}

// ------------------------------------------------------------- valued

/// A plain literal's value is the text between its quotes, for `const`
/// and `static` alike, an empty literal included (reading L7).
#[test]
fn plain_literal_value_is_its_text() {
    let source = "pub const PLAIN: &str = \"QA-101\";\n\
                  pub static HELD: &str = \"QA-102\";\n\
                  const SPACED: &'static str = \"two words\";\n\
                  pub const EMPTY: &str = \"\";\n";
    assert_eq!(
        read(source),
        owned(&[
            ("PLAIN", "valued \"QA-101\""),
            ("HELD", "valued \"QA-102\""),
            ("SPACED", "valued \"two words\""),
            ("EMPTY", "valued \"\""),
        ])
    );
}

/// M: `r#` kept. A raw literal's value is the text between its quotes,
/// whatever its hashes; a backslash in it is a character, never an escape
/// (`escaped` is a plain literal's, reading L6).
#[test]
fn raw_literal_value_is_its_text() {
    let source = r####"pub const BARE: &str = r"QA-201";
pub const HASHED: &str = r#"QA-202"#;
pub const QUOTED: &str = r##"say "QA-203" #now"##;
pub const SLASHED: &str = r"QA\204";
"####;
    assert_eq!(
        read(source),
        owned(&[
            ("BARE", r#"valued "QA-201""#),
            ("HASHED", r#"valued "QA-202""#),
            ("QUOTED", r#"valued "say \"QA-203\" #now""#),
            ("SLASHED", r#"valued "QA\\204""#),
        ])
    );
}

/// The node read is the one spanning the item's byte range: cfg twins of
/// one name, each with its own value, read their own (M: the node found by
/// name, or the first `const` of the file).
#[test]
fn twins_are_read_by_their_own_byte_range() {
    let source = "#[cfg(unix)]\npub const TWIN: &str = \"QA-301\";\n\n\
                  #[cfg(not(unix))]\npub const TWIN: &str = \"QA-302\";\n";
    assert_eq!(
        read(source),
        owned(&[("TWIN", "valued \"QA-301\""), ("TWIN", "valued \"QA-302\"")])
    );
}

// --------------------------------------------------------- not valued

/// M: `"\x45…"` unescaped. A plain literal holding `\` is never a value,
/// whatever the escape: a hex, a Unicode, a tab, a quote, a backslash, a
/// line continuation (which holds an LF too: `escaped` comes first).
#[test]
fn escaped_literal_is_not_a_value() {
    let source = r#"pub const HEX: &str = "\x45-401";
pub const UNICODE: &str = "Q\u{41}-402";
pub const TAB: &str = "QA\t403";
pub const QUOTE: &str = "QA\"404";
pub const BACKSLASH: &str = "\\";
pub const CONTINUED: &str = "QA-\
    405";
pub const PLAIN: &str = "QA-406";
"#;
    assert_eq!(
        read(source),
        owned(&[
            ("HEX", "not_valued escaped"),
            ("UNICODE", "not_valued escaped"),
            ("TAB", "not_valued escaped"),
            ("QUOTE", "not_valued escaped"),
            ("BACKSLASH", "not_valued escaped"),
            ("CONTINUED", "not_valued escaped"),
            ("PLAIN", "valued \"QA-406\""),
        ])
    );
}

/// M: a CR kept. A literal holding a line end as written, LF or a lone CR,
/// plain or raw, is never a value; the next item still is.
#[test]
fn multi_line_literal_is_not_a_value() {
    let source = "pub const LF: &str = \"QA-501\nQA-502\";\n\
                  pub const RAW_LF: &str = r\"QA-503\nQA-504\";\n\
                  pub const CR: &str = \"QA-505\rQA-506\";\n\
                  pub const RAW_CR: &str = r#\"QA-507\rQA-508\"#;\n\
                  pub const AFTER: &str = \"QA-509\";\n";
    assert_eq!(
        read(source),
        owned(&[
            ("LF", "not_valued multi_line"),
            ("RAW_LF", "not_valued multi_line"),
            ("CR", "not_valued multi_line"),
            ("RAW_CR", "not_valued multi_line"),
            ("AFTER", "valued \"QA-509\""),
        ])
    );
}

/// M: `b"…"` accepted. Byte strings, raw or not, C strings, raw or not, a
/// char and a byte char are never a value; a byte string holding an escape
/// is `byte_string` (it comes before `escaped`).
#[test]
fn byte_string_is_not_a_value() {
    let source = r##"pub const BYTES: &[u8] = b"QA-601";
pub const RAW_BYTES: &[u8] = br"QA-602";
pub const HASHED_BYTES: &[u8] = br#"QA-603"#;
pub const C_STRING: &CStr = c"QA-604";
pub const RAW_C_STRING: &CStr = cr"QA-605";
pub const LETTER: char = 'Q';
pub const BYTE: u8 = b'Q';
pub const ESCAPED_BYTES: &[u8] = b"QA\x2D606";
"##;
    assert_eq!(
        read(source),
        owned(&[
            ("BYTES", "not_valued byte_string"),
            ("RAW_BYTES", "not_valued byte_string"),
            ("HASHED_BYTES", "not_valued byte_string"),
            ("C_STRING", "not_valued byte_string"),
            ("RAW_C_STRING", "not_valued byte_string"),
            ("LETTER", "not_valued byte_string"),
            ("BYTE", "not_valued byte_string"),
            ("ESCAPED_BYTES", "not_valued byte_string"),
        ])
    );
}

/// M: `concat!` accepted. A macro invocation, whichever, and any call (a
/// path call, a function call, a method call on a literal: reading L11)
/// are never a value, even when their arguments are plain literals.
#[test]
fn macro_value_is_not_a_value() {
    let source = r#"pub const JOINED: &str = concat!("QA", "-701");
pub const INCLUDED: &str = include_str!("codes.txt");
pub const NAMED: &str = stringify!(QA);
pub const FROM_ENV: &str = env!("QA_CODE");
pub const WRAPPED: &str = wrap!("QA-702");
pub const BUILT: String = String::new();
pub const CALLED: &str = code("QA-703");
pub const LENGTH: usize = "QA-704".len();
"#;
    assert_eq!(
        read(source),
        owned(&[
            ("JOINED", "not_valued macro"),
            ("INCLUDED", "not_valued macro"),
            ("NAMED", "not_valued macro"),
            ("FROM_ENV", "not_valued macro"),
            ("WRAPPED", "not_valued macro"),
            ("BUILT", "not_valued macro"),
            ("CALLED", "not_valued macro"),
            ("LENGTH", "not_valued macro"),
        ])
    );
}

/// M: `has_error` ignored (the block would read `other`). An item with a
/// parse error is `broken`; an intact neighbour still reads its value.
/// The broken item is last, so recovery cannot take the neighbour; the
/// precondition (tree-sitter keeps it a `const_item` with `has_error`,
/// reading L4) is asserted first.
#[test]
fn broken_item_is_not_a_value() {
    let source = "pub const INTACT: &str = \"QA-801\";\n\n\
                  pub const BROKEN: &str = { let code = 1 2; \"QA-802\" };\n";
    for text in [source.to_owned(), crlf(source)] {
        let broken = item_named(&text, "BROKEN");
        assert_eq!(broken.kind, "const_item");
        assert!(broken.has_error, "the fixture item has a parse error");
        assert!(!item_named(&text, "INTACT").has_error);
    }
    assert_eq!(
        read(source),
        owned(&[
            ("INTACT", "valued \"QA-801\""),
            ("BROKEN", "not_valued broken"),
        ])
    );
}

/// The rest is `other`: no initializer (`const` and `static`), numbers, a
/// bool, paths, `&[..]`, a borrowed literal, parentheses, a tuple, a cast,
/// a block, an `if`, an index. M: a literal looked for below the
/// initializer (the parenthesised, borrowed, block and `if` ones would
/// read valued).
#[test]
fn other_initializers_are_other() {
    let source = r#"pub const NO_VALUE: &str;
pub static NO_VALUE_EITHER: &str;
pub const NUMBER: u32 = 7;
pub const NEGATIVE: i32 = -7;
pub const FLOAT: f32 = 1.5;
pub const FLAG: bool = true;
pub const PATH: &str = TEXT;
pub const SCOPED: &str = crate::codes::TEXT;
pub const LIST: &[&str] = &["QA-901", "QA-902"];
pub const BORROWED: &str = &"QA-903";
pub const PARENTHESISED: &str = ("QA-904");
pub const PAIR: (&str, &str) = ("QA-905", "QA-906");
pub const CAST: u8 = 7 as u8;
pub const BLOCK: &str = { "QA-907" };
pub const CHOICE: &str = if true { "QA-908" } else { "QA-909" };
pub const INDEXED: &str = TABLE[0];
"#;
    let rows = read(source);
    let names: Vec<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names.len(), 16, "every item read: {names:?}");
    for (name, outcome) in &rows {
        assert_eq!(outcome, "not_valued other", "{name}");
    }
}

/// The first category that applies wins: a byte string holding an escape
/// or a line end is `byte_string`; an escaped literal holding a line end
/// is `escaped`; a macro over a byte string is `macro`; a broken item is
/// `broken` whatever its initializer. M: the categories tried in another
/// order.
#[test]
fn the_first_category_that_applies_wins() {
    let source = r#"pub const BYTES_ESCAPED: &[u8] = b"QA\x2D1";
pub const BYTES_LINES: &[u8] = b"QA-1
QA-2";
pub const ESCAPED_LINES: &str = "QA-1\
QA-2";
pub const ESCAPED_AND_LF: &str = "QA\t1
QA-2";
pub const MACRO_OF_BYTES: &[u8] = concat_bytes!(b"QA");
pub const BROKEN_CALL: &str = { let code = 1 2; code() };
"#;
    assert_eq!(
        read(source),
        owned(&[
            ("BYTES_ESCAPED", "not_valued byte_string"),
            ("BYTES_LINES", "not_valued byte_string"),
            ("ESCAPED_LINES", "not_valued escaped"),
            ("ESCAPED_AND_LF", "not_valued escaped"),
            ("MACRO_OF_BYTES", "not_valued macro"),
            ("BROKEN_CALL", "not_valued broken"),
        ])
    );
}

/// `NotValued::as_str` gives exactly the six names of the spec's Data.
#[test]
fn not_valued_names_are_the_six_categories() {
    let source = r#"pub const A: &str = concat!("QA");
pub const B: &[u8] = b"QA";
pub const C: &str = "Q\x41";
pub const D: &str = "QA
QB";
pub const E: u32 = 1;
pub const F: &str = { let code = 1 2; "QA" };
"#;
    let names: BTreeSet<String> = read(source)
        .into_iter()
        .map(|(_, outcome)| outcome)
        .collect();
    let expected: BTreeSet<String> = [
        "not_valued broken",
        "not_valued byte_string",
        "not_valued escaped",
        "not_valued macro",
        "not_valued multi_line",
        "not_valued other",
    ]
    .iter()
    .map(|name| (*name).to_owned())
    .collect();
    assert_eq!(names, expected);
}

// ------------------------------------------------------ what is read

/// `None` unless the item is a `const_item` or a `static_item` (M: any
/// item with a `value` field read, or a function's tail literal).
#[test]
fn only_const_and_static_items_are_read() {
    let source = r#"pub fn run() -> &'static str { "QA-1" }
pub struct Gauge;
pub enum Level { Low }
pub type Code = &'static str;
pub trait Named { fn name(&self) -> &str; }
impl Gauge { pub fn code(&self) -> &str { "QA-2" } }
macro_rules! code { () => { "QA-3" }; }
pub mod inner { pub fn f() {} }
pub const READ: &str = "QA-4";
pub static ALSO_READ: &str = "QA-5";
"#;
    let tree = RustParser::new()
        .expect("the pinned grammar loads")
        .parse(source)
        .expect("the parser gives a tree");
    let bound = bind_tree(&tree, source);
    let mut kinds = BTreeSet::new();
    for item in &bound.analysis.items {
        let value = const_value(&tree, source, item);
        let readable = item.kind == "const_item" || item.kind == "static_item";
        assert_eq!(
            value.is_some(),
            readable,
            "{} {}: {}",
            item.kind,
            item.label,
            outcome(value)
        );
        kinds.insert(item.kind.clone());
    }
    for kind in [
        "function_item",
        "struct_item",
        "enum_item",
        "type_item",
        "trait_item",
        "impl_item",
        "macro_definition",
        "mod_item",
        "const_item",
        "static_item",
    ] {
        assert!(kinds.contains(kind), "{kind} is among the items: {kinds:?}");
    }
}

/// D2: an associated `const` (an `impl`'s, a trait's), a `const` or
/// `static` in a function body and an enum variant are no items, so never
/// read; an inline `mod`'s `const` is, with its `mod_path`.
#[test]
fn associated_and_body_consts_are_never_read() {
    let source = r#"pub struct Gauge;
impl Gauge {
    pub const ASSOCIATED: &'static str = "QA-1";
}
pub trait Named {
    const TRAIT_CONST: &'static str = "QA-2";
}
pub fn body() -> &'static str {
    const BODY_CONST: &str = "QA-3";
    static BODY_STATIC: &str = "QA-4";
    BODY_CONST
}
pub enum Level {
    Low,
}
pub mod inner {
    pub const MODULE_CONST: &str = "QA-5";
}
"#;
    assert_eq!(read(source), owned(&[("MODULE_CONST", "valued \"QA-5\"")]));
    assert_eq!(item_named(source, "MODULE_CONST").mod_path, ["inner"]);
}

/// M: a walk below the initializer (recursive: it overflows the quarter
/// megabyte stack; iterative: it finds the literal and reads it valued).
/// 10 000 parentheses around a plain literal are one parenthesised
/// expression: `other`, read in constant depth. Parsing and binding run on
/// a large stack; only the reader runs on the small one. The neighbour
/// after it still reads its value.
#[test]
fn deeply_nested_initializer_is_other() {
    let depth = 10_000;
    let source = format!(
        "pub const DEEP: &str = {}\"QA-1001\"{};\npub const NEXT: &str = \"QA-1002\";\n",
        "(".repeat(depth),
        ")".repeat(depth)
    );
    let (tree, deep, next, source) = thread::Builder::new()
        .name("const-value-parse".to_owned())
        .stack_size(SIXTY_FOUR_MB)
        .spawn(move || {
            let tree = RustParser::new()
                .expect("the pinned grammar loads")
                .parse(&source)
                .expect("the parser gives a tree");
            let items = bind_tree(&tree, &source).analysis.items;
            let find = |name: &str| {
                items
                    .iter()
                    .find(|item| item.name.as_deref() == Some(name))
                    .cloned()
                    .unwrap_or_else(|| panic!("{name} is an item"))
            };
            let (deep, next) = (find("DEEP"), find("NEXT"));
            (tree, deep, next, source)
        })
        .expect("the parse thread spawns")
        .join()
        .expect("parsing and binding the deep source must not fail");
    assert_eq!(deep.kind, "const_item");
    assert!(!deep.has_error, "the deep source parses cleanly");
    let answers = thread::Builder::new()
        .name("const-value-read".to_owned())
        .stack_size(QUARTER_MB)
        .spawn(move || {
            (
                outcome(const_value(&tree, &source, &deep)),
                outcome(const_value(&tree, &source, &next)),
            )
        })
        .expect("the reader thread spawns")
        .join()
        .expect("the reader must not panic or overflow a small stack");
    assert_eq!(
        answers,
        (
            "not_valued other".to_owned(),
            "valued \"QA-1002\"".to_owned()
        )
    );
}
