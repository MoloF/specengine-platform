//! docs/features/spec-lock.md RD-2, RD-3 and TE-10's core half: the lock
//! model of `specengine_core::lock` (pure: text in, text out). The golden
//! is derived by hand from the spec's "The file" and "Entry": the header
//! line, `format`, both recipes, then the blocks by key (`node` bytes,
//! `relation` as listed, `file`, `kind`, `symbol`, `item_kind` bytes), the
//! keys in the order shown, an empty line before each block, LF and a
//! final LF; hashes and OIDs are made-up values of the right form.

use specengine_core::lock::{LOCK_HEADER, Lock, Recipes, parse_lock, render_lock};

const HEADER: &str = "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit";

fn build() -> Recipes {
    Recipes {
        norm: "specengine-norm/v1".to_owned(),
        code: "specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15".to_owned(),
    }
}

fn hex(digit: char, len: usize) -> String {
    digit.to_string().repeat(len)
}

/// One block, its lines in the order of "Entry".
#[allow(clippy::too_many_arguments)]
fn block(
    node: &str,
    rev: u32,
    relation: &str,
    file: &str,
    kind: &str,
    symbol: &str,
    item_kind: Option<&str>,
    digit: char,
) -> String {
    let mut out = format!(
        "[[binding]]\nnode = \"{node}\"\nrev = {rev}\nspec_hash = \"b3:{}\"\nrelation = \"{relation}\"\nfile = \"{file}\"\nkind = \"{kind}\"\nsymbol = \"{symbol}\"\n",
        hex(digit, 64)
    );
    if let Some(item_kind) = item_kind {
        out.push_str(&format!(
            "item_kind = \"{item_kind}\"\nast_hash = \"b3:{}\"\n",
            hex(digit, 64)
        ));
    }
    out.push_str(&format!(
        "file_blob = \"{}\"\ncommit = \"{}\"\nverified_by = \"owner\"\nverified_at = \"2026-10-10\"\n",
        hex(digit, 40),
        hex(digit, 40)
    ));
    out
}

/// The blocks of the golden, in key order.
fn golden_blocks() -> Vec<String> {
    vec![
        // `R-12` < `R-2` by bytes; `implements` < `verifies` < `configures`
        // < `assumes` as listed, not by bytes.
        block(
            "R-12",
            3,
            "implements",
            "crates/game/src/stamina.rs",
            "item",
            "game::stamina::regen_system",
            Some("function_item"),
            '1',
        ),
        block(
            "R-12",
            3,
            "implements",
            "crates/game/src/stamina.rs",
            "module",
            "game::stamina",
            None,
            '2',
        ),
        block(
            "R-12",
            3,
            "verifies",
            "crates/game/tests/stamina.rs",
            "item",
            "game::test:stamina::regen",
            Some("function_item"),
            '3',
        ),
        block(
            "R-12",
            3,
            "configures",
            "assets/data/stamina.ron",
            "ron",
            "root.stamina.regen_per_second",
            None,
            '4',
        ),
        block(
            "R-12",
            3,
            "assumes",
            "crates/game/src/lib.rs",
            "module",
            "game",
            None,
            '5',
        ),
        block(
            "R-2",
            1,
            "implements",
            "crates/game/src/a.rs",
            "item",
            "game::a::f",
            Some("const_item"),
            '6',
        ),
        block(
            "R-2",
            1,
            "implements",
            "crates/game/src/a.rs",
            "item",
            "game::a::f",
            Some("function_item"),
            '7',
        ),
        block(
            "stamina/AC-07",
            999_999_999,
            "verifies",
            "crates/game/tests/stamina.rs",
            "item",
            "game::test:stamina::drains",
            Some("function_item"),
            '8',
        ),
    ]
}

fn golden() -> String {
    let mut out = format!(
        "{HEADER}\nformat = 1\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15\"\n"
    );
    for block in golden_blocks() {
        out.push('\n');
        out.push_str(&block);
    }
    out
}

fn with_blocks(blocks: &[String]) -> String {
    let mut out = format!(
        "{HEADER}\nformat = 1\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15\"\n"
    );
    for block in blocks {
        out.push('\n');
        out.push_str(block);
    }
    out
}

fn parse(text: &str) -> Lock {
    parse_lock(text, Some(&build())).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// The refusal of `text`: `spec.lock:<line>: <message>`.
fn refusal(text: &str) -> String {
    match parse_lock(text, Some(&build())) {
        Ok(lock) => panic!("read, not refused: {lock:?}\n{text}"),
        Err(error) => error.to_string(),
    }
}

/// The header line is the spec's.
#[test]
fn the_header_is_the_specs() {
    assert_eq!(LOCK_HEADER, HEADER);
}

/// RD-2: the golden reads and renders back byte for byte.
#[test]
fn the_golden_renders_back() {
    let text = golden();
    assert_eq!(render_lock(&parse(&text)), text);
}

/// RD-2 (M: insertion order): blocks in reverse and in a shuffled order
/// render in key order.
#[test]
fn lock_entries_are_sorted_by_key() {
    let mut blocks = golden_blocks();
    blocks.reverse();
    assert_eq!(render_lock(&parse(&with_blocks(&blocks))), golden());
    let mut shuffled = golden_blocks();
    shuffled.rotate_left(3);
    shuffled.swap(0, 4);
    assert_eq!(render_lock(&parse(&with_blocks(&shuffled))), golden());
}

/// RD-2 (M: CRLF): the bytes are LF with one final LF; a CRLF copy reads
/// and renders back as the LF golden.
#[test]
fn lock_bytes_are_lf_with_a_final_lf() {
    let text = golden();
    let rendered = render_lock(&parse(&text));
    assert!(!rendered.contains('\r'));
    assert!(rendered.ends_with('\n'));
    assert!(!rendered.ends_with("\n\n"));
    assert!(!rendered.starts_with('\u{feff}'));
    let crlf = text.replace('\n', "\r\n");
    assert_eq!(render_lock(&parse(&crlf)), text);
    // An empty lock: the header and the three keys, nothing after.
    let empty = Lock::empty(build());
    assert_eq!(
        render_lock(&empty),
        format!(
            "{HEADER}\nformat = 1\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15\"\n"
        )
    );
}

/// RD-2: an equivalent in another TOML layout (keys reordered, spaces,
/// comments, literal strings, an inline array of tables) renders as the
/// golden.
#[test]
fn a_respaced_equivalent_renders_back() {
    let mut respaced = String::from(
        "# hand-edited\ncode_recipe='specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15'\n  format=1\nnorm_recipe   =   \"specengine-norm/v1\"   # the spec side\n\n\n",
    );
    for block in golden_blocks() {
        let mut lines: Vec<&str> = block.lines().collect();
        let header = lines.remove(0);
        lines.reverse();
        respaced.push_str(header);
        respaced.push('\n');
        for line in lines {
            let (key, value) = line.split_once(" = ").expect("key = value");
            respaced.push_str(&format!("\t{key}= {value}  \n"));
        }
        respaced.push_str("\n\n");
    }
    assert_eq!(render_lock(&parse(&respaced)), golden());

    let mut inline = String::from(
        "format = 1\nnorm_recipe = \"specengine-norm/v1\"\ncode_recipe = \"specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15\"\nbinding = [\n",
    );
    for block in golden_blocks() {
        let fields: Vec<String> = block.lines().skip(1).map(str::to_owned).collect();
        inline.push_str(&format!("  {{ {} }},\n", fields.join(", ")));
    }
    inline.push_str("]\n");
    assert_eq!(render_lock(&parse(&inline)), golden());
}

/// RD-2: basic strings escape only `"`, `\` and U+0000-U+001F, U+007F
/// (`\b \t \n \f \r`, else `\u00XX`); any other character is written as
/// it is.
#[test]
fn strings_escape_only_what_toml_needs() {
    let text = with_blocks(&[block(
        "R-1",
        1,
        "implements",
        "a.rs",
        "module",
        "a",
        None,
        'a',
    )]);
    let lock = parse(&text);
    for (raw, escaped) in [
        ("q\"x\\y", "q\\\"x\\\\y"),
        ("\u{e9}\u{4e16}", "\u{e9}\u{4e16}"),
        ("s\u{8}s", "s\\bs"),
        ("s\ts", "s\\ts"),
        ("s\ns", "s\\ns"),
        ("s\u{c}s", "s\\fs"),
        ("s\rs", "s\\rs"),
        ("s\u{0}s", "s\\u0000s"),
        ("s\u{1f}s", "s\\u001Fs"),
        ("s\u{7f}s", "s\\u007Fs"),
    ] {
        let mut lock = lock.clone();
        let (mut key, verified) = lock.entries.pop_first().expect("an entry");
        key.symbol = raw.to_owned();
        lock.entries.insert(key, verified);
        let rendered = render_lock(&lock);
        assert!(
            rendered.contains(&format!("symbol = \"{escaped}\"\n")),
            "{raw:?}: {rendered}"
        );
    }
}

/// RD-2, RD-3: what the writer escapes the reader reads back: `symbol`
/// has no form of its own in "Reader" (a `qpath` or a RON field path,
/// opaque), so a symbol holding a tab or U+007F renders and reads back.
#[test]
fn escaped_symbols_read_back() {
    let text = with_blocks(&[block(
        "R-1",
        1,
        "implements",
        "a.rs",
        "module",
        "a",
        None,
        'a',
    )]);
    let lock = parse(&text);
    let mut failures = Vec::new();
    for raw in ["q\"x\\y", "\u{e9}\u{4e16}", "s\ts", "s\u{7f}s", "s\u{1}s"] {
        let mut lock = lock.clone();
        let (mut key, verified) = lock.entries.pop_first().expect("an entry");
        key.symbol = raw.to_owned();
        lock.entries.insert(key, verified);
        let rendered = render_lock(&lock);
        match parse_lock(&rendered, Some(&build())) {
            Ok(read) if read == lock => {}
            Ok(read) => failures.push(format!("{raw:?} read back as {read:?}")),
            Err(error) => failures.push(format!("{raw:?}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// RD-3 (M: an unknown key accepted): an unknown key in a block or at the
/// top is refused at its line.
#[test]
fn lock_unknown_key_is_refused() {
    let text = golden().replacen(
        "verified_by = \"owner\"\n",
        "verified_by = \"owner\"\nline = 12\n",
        1,
    );
    let line = text
        .lines()
        .position(|line| line == "line = 12")
        .expect("the key")
        + 1;
    let message = refusal(&text);
    assert!(
        message.starts_with(&format!("spec.lock:{line}: ")),
        "{message}"
    );
    assert!(message.contains("line"), "{message}");
    let top = golden().replacen("format = 1\n", "format = 1\ncreated = \"2026-10-10\"\n", 1);
    let message = refusal(&top);
    assert!(message.starts_with("spec.lock:3: "), "{message}");
    assert!(message.contains("created"), "{message}");
}

/// RD-3: every refused form of "The file" "Reader", at its line, `spec.lock:
/// <line>: <message>`; the recipes of another build, both named.
#[test]
fn the_reader_refuses_every_wrong_form() {
    let base = golden();
    // The first block's lines: 7 node, 8 rev, 9 spec_hash, 10 relation,
    // 11 file, 12 kind, 13 symbol, 14 item_kind, 15 ast_hash, 16
    // file_blob, 17 commit, 18 verified_by, 19 verified_at.
    let first = |from: &str, to: &str| base.replacen(from, to, 1);
    let one = hex('1', 64);
    let oid = hex('1', 40);
    let cases: Vec<(&str, String, Option<usize>, &str)> = vec![
        (
            "another format",
            first("format = 1", "format = 2"),
            Some(2),
            "format",
        ),
        (
            "a missing recipe",
            first("norm_recipe = \"specengine-norm/v1\"\n", ""),
            None,
            "norm_recipe",
        ),
        (
            "a missing key",
            first("verified_at = \"2026-10-10\"\n", ""),
            None,
            "verified_at",
        ),
        (
            "a repeated key",
            first("rev = 3\n", "rev = 3\nrev = 3\n"),
            Some(9),
            "duplicate",
        ),
        ("rev 0", first("rev = 3", "rev = 0"), Some(8), "rev"),
        (
            "rev over the cap",
            first("rev = 3", "rev = 1000000000"),
            Some(8),
            "rev",
        ),
        (
            "rev a string",
            first("rev = 3", "rev = \"3\""),
            Some(8),
            "rev",
        ),
        (
            "spec_hash uppercase",
            first(
                &format!("spec_hash = \"b3:{one}\""),
                &format!("spec_hash = \"b3:{}\"", hex('A', 64)),
            ),
            Some(9),
            "spec_hash",
        ),
        (
            "spec_hash no prefix",
            first(
                &format!("spec_hash = \"b3:{one}\""),
                &format!("spec_hash = \"{one}\""),
            ),
            Some(9),
            "spec_hash",
        ),
        (
            "spec_hash short",
            first(
                &format!("spec_hash = \"b3:{one}\""),
                &format!("spec_hash = \"b3:{}\"", hex('1', 63)),
            ),
            Some(9),
            "spec_hash",
        ),
        (
            "an unknown relation",
            first("relation = \"implements\"", "relation = \"mentions\""),
            Some(10),
            "relation",
        ),
        (
            "an absolute file",
            first(
                "file = \"crates/game/src/stamina.rs\"",
                "file = \"/crates/game/src/stamina.rs\"",
            ),
            Some(11),
            "file",
        ),
        (
            "a dot-dot file",
            first(
                "file = \"crates/game/src/stamina.rs\"",
                "file = \"crates/../game/src/stamina.rs\"",
            ),
            Some(11),
            "file",
        ),
        (
            "a backslash file",
            first(
                "file = \"crates/game/src/stamina.rs\"",
                "file = \"crates\\\\game\\\\stamina.rs\"",
            ),
            Some(11),
            "file",
        ),
        (
            "an unknown kind",
            first("kind = \"item\"", "kind = \"file\""),
            Some(12),
            "kind",
        ),
        (
            "an item without ast_hash",
            first(&format!("ast_hash = \"b3:{one}\"\n"), ""),
            None,
            "ast_hash",
        ),
        (
            "an item without item_kind",
            first("item_kind = \"function_item\"\n", ""),
            None,
            "item_kind",
        ),
        (
            "a short ast_hash",
            first(
                &format!("ast_hash = \"b3:{one}\""),
                &format!("ast_hash = \"b3:{}\"", hex('1', 62)),
            ),
            Some(15),
            "ast_hash",
        ),
        (
            "an abbreviated file_blob",
            first(
                &format!("file_blob = \"{oid}\""),
                &format!("file_blob = \"{}\"", hex('1', 7)),
            ),
            Some(16),
            "file_blob",
        ),
        (
            "a 41-hex commit",
            first(
                &format!("commit = \"{oid}\""),
                &format!("commit = \"{}\"", hex('1', 41)),
            ),
            Some(17),
            "commit",
        ),
        (
            "an uppercase commit",
            first(
                &format!("commit = \"{oid}\""),
                &format!("commit = \"{}\"", hex('F', 40)),
            ),
            Some(17),
            "commit",
        ),
        (
            "verified by a test run",
            first("verified_by = \"owner\"", "verified_by = \"test\""),
            Some(18),
            "verified_by",
        ),
        (
            "a time, not a date",
            first(
                "verified_at = \"2026-10-10\"",
                "verified_at = \"2026-10-10T09:30:00Z\"",
            ),
            Some(19),
            "verified_at",
        ),
        (
            "no such day",
            first(
                "verified_at = \"2026-10-10\"",
                "verified_at = \"2026-02-30\"",
            ),
            Some(19),
            "verified_at",
        ),
        (
            "no such month",
            first(
                "verified_at = \"2026-10-10\"",
                "verified_at = \"2026-13-01\"",
            ),
            Some(19),
            "verified_at",
        ),
    ];
    let mut failures = Vec::new();
    for (context, text, line, named) in cases {
        let message = match parse_lock(&text, Some(&build())) {
            Ok(_) => {
                failures.push(format!("{context}: read, not refused"));
                continue;
            }
            Err(error) => error.to_string(),
        };
        let prefix_ok = match line {
            Some(line) => message.starts_with(&format!("spec.lock:{line}: ")),
            None => message
                .strip_prefix("spec.lock:")
                .and_then(|rest| rest.split_once(": "))
                .is_some_and(|(line, _)| line.parse::<usize>().is_ok()),
        };
        if !prefix_ok || !message.contains(named) {
            failures.push(format!("{context}: {message}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// RD-3: a module or a RON value with `item_kind` or `ast_hash`, and a
/// duplicate binding key, are refused.
#[test]
fn item_only_keys_and_duplicates_are_refused() {
    let module = block("R-1", 1, "implements", "a.rs", "module", "a", None, '1').replace(
        "file_blob",
        &format!("ast_hash = \"b3:{}\"\nfile_blob", hex('1', 64)),
    );
    let message = refusal(&with_blocks(&[module]));
    assert!(message.contains("ast_hash"), "{message}");
    let ron = block("R-1", 1, "configures", "a.ron", "ron", "root.a", None, '1')
        .replace("file_blob", "item_kind = \"function_item\"\nfile_blob");
    let message = refusal(&with_blocks(&[ron]));
    assert!(message.contains("item_kind"), "{message}");
    // Same key, other values: a duplicate.
    let one = block(
        "R-1",
        1,
        "implements",
        "a.rs",
        "item",
        "a::f",
        Some("function_item"),
        '1',
    );
    let two = block(
        "R-1",
        2,
        "implements",
        "a.rs",
        "item",
        "a::f",
        Some("function_item"),
        '2',
    );
    let text = with_blocks(&[one.clone(), two]);
    let message = refusal(&text);
    assert!(message.starts_with("spec.lock:"), "{message}");
    // Same key but another item_kind: two entries.
    let three = block(
        "R-1",
        1,
        "implements",
        "a.rs",
        "item",
        "a::f",
        Some("const_item"),
        '3',
    );
    assert_eq!(parse(&with_blocks(&[one, three])).entries.len(), 2);
}

/// RD-3: recipes of another build are refused naming both; without a
/// build to compare (regen), they read.
#[test]
fn another_builds_recipes_are_refused_naming_both() {
    let other = golden().replace(
        "specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15",
        "specengine-hash/v2 tree-sitter-rust/0.25.0 abi/15",
    );
    let message = refusal(&other);
    assert!(message.starts_with("spec.lock:4: "), "{message}");
    assert!(message.contains("tree-sitter-rust/0.25.0"), "{message}");
    assert!(message.contains("tree-sitter-rust/0.24.2"), "{message}");
    let norm = golden().replace("specengine-norm/v1", "specengine-norm/v2");
    let message = refusal(&norm);
    assert!(
        message.contains("specengine-norm/v2") && message.contains("specengine-norm/v1"),
        "{message}"
    );
    assert!(parse_lock(&other, None).is_ok());
}

/// TE-10: the core module holds no fixture prefix or kind, no Rust item
/// kind, no `.rs`, no `crate::` (ADR-0008): the code side is opaque text.
#[test]
fn the_core_lock_module_knows_no_domain() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lock.rs");
    let source = std::fs::read_to_string(&path).expect("crates/specengine-core/src/lock.rs");
    let mut found = Vec::new();
    for needle in [
        "PIECE",
        "LAW-",
        "GOAL",
        "PREM",
        "PASS-",
        "AIM-",
        "piece",
        "objective",
        "premise",
        "pass-mark",
        "tally",
        "function_item",
        "struct_item",
        "const_item",
        "mod_item",
        ".rs",
        "crate::",
    ] {
        if source.contains(needle) {
            found.push(needle);
        }
    }
    assert!(
        found.is_empty(),
        "crates/specengine-core/src/lock.rs names {found:?}"
    );
}
