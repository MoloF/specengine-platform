//! docs/features/spec-lock.md RD-2, RD-3 and TE-10's core half, and
//! docs/features/lock-levels.md RD-1 (with its M1, M2) and "Readers": the
//! lock model of `specengine_core::lock` (pure: text in, text out). The
//! golden is derived by hand from lock-levels.md "Format 2": the header
//! line, `format = 2`, both recipes, then the blocks by key (`node` bytes,
//! `relation` as listed, `file`, `kind`, `symbol`, `item_kind` bytes), the
//! keys in WA-4's order (`item_kind` an item's; `levels` and the three
//! level digests every kind's; the fingerprints and `body_bytes` an item's
//! or a module's), an empty line before each block, LF and a final LF;
//! digests and OIDs are made-up values of the right form, each field's
//! distinct so a swapped field shows. Format 1 (`spec-lock.md` "Entry"
//! before 5b-1a) is read only without the build's recipes (Q10).

use specengine_core::lock::{
    AcceptInput, Change, LOCK_HEADER, Lock, LockFormat, LockLevel, Recipes, lock_code,
    merge_accept, parse_lock, refresh_editorial, render_lock,
};

const HEADER: &str = "# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit";

const NORM: &str = "specengine-norm/v1";
const CODE: &str = "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15";
const CODE_FORMAT_1: &str = "specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15";

fn build() -> Recipes {
    Recipes {
        norm: NORM.to_owned(),
        code: CODE.to_owned(),
    }
}

fn hex(digit: char, len: usize) -> String {
    digit.to_string().repeat(len)
}

/// A digest of field `field` (one hex letter) for block `digit`.
fn digest(field: char, digit: char) -> String {
    format!("b3:{field}{}", hex(digit, 63))
}

/// One format-2 block, its lines in WA-4's order; `levels` as written
/// (the TOML array's inside).
#[allow(clippy::too_many_arguments)]
fn block_with(
    node: &str,
    rev: u32,
    relation: &str,
    file: &str,
    kind: &str,
    symbol: &str,
    item_kind: Option<&str>,
    levels: &str,
    digit: char,
) -> String {
    let mut out = format!(
        "[[binding]]\nnode = \"{node}\"\nrev = {rev}\nspec_hash = \"b3:{}\"\nrelation = \"{relation}\"\nfile = \"{file}\"\nkind = \"{kind}\"\nsymbol = \"{symbol}\"\n",
        hex(digit, 64)
    );
    if let Some(item_kind) = item_kind {
        out.push_str(&format!("item_kind = \"{item_kind}\"\n"));
    }
    out.push_str(&format!(
        "levels = [{levels}]\nsig_hash = \"{}\"\nbody_hash = \"{}\"\ndeps_hash = \"{}\"\n",
        digest('a', digit),
        digest('b', digit),
        digest('c', digit)
    ));
    if kind != "ron" {
        out.push_str(&format!(
            "skeleton_hash = \"{}\"\nquote_hash = \"{}\"\nbody_bytes = {}\n",
            digest('d', digit),
            digest('e', digit),
            200 + digit.to_digit(16).expect("a hex digit")
        ));
    }
    out.push_str(&format!(
        "file_blob = \"{}\"\ncommit = \"{}\"\nverified_by = \"owner\"\nverified_at = \"2026-10-10\"\n",
        hex(digit, 40),
        hex(digit, 40)
    ));
    out
}

/// [`block_with`] at the default levels.
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
    block_with(
        node,
        rev,
        relation,
        file,
        kind,
        symbol,
        item_kind,
        "\"sig\", \"body\"",
        digit,
    )
}

/// One format-1 block, its lines in format 1's "Entry" order.
#[allow(clippy::too_many_arguments)]
fn block_format_1(
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

/// The golden's eight places: `(node, rev, relation, file, kind, symbol,
/// item_kind, digit)`, in key order. `R-12` < `R-2` by bytes;
/// `implements` < `verifies` < `configures` < `assumes` as listed, not by
/// bytes.
type Place = (
    &'static str,
    u32,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    char,
);

const PLACES: [Place; 8] = [
    (
        "R-12",
        3,
        "implements",
        "crates/game/src/stamina.rs",
        "item",
        "game::stamina::regen_system",
        Some("function_item"),
        '1',
    ),
    (
        "R-12",
        3,
        "implements",
        "crates/game/src/stamina.rs",
        "module",
        "game::stamina",
        None,
        '2',
    ),
    (
        "R-12",
        3,
        "verifies",
        "crates/game/tests/stamina.rs",
        "item",
        "game::test:stamina::regen",
        Some("function_item"),
        '3',
    ),
    (
        "R-12",
        3,
        "configures",
        "assets/data/stamina.ron",
        "ron",
        "root.stamina.regen_per_second",
        None,
        '4',
    ),
    (
        "R-12",
        3,
        "assumes",
        "crates/game/src/lib.rs",
        "module",
        "game",
        None,
        '5',
    ),
    (
        "R-2",
        1,
        "implements",
        "crates/game/src/a.rs",
        "item",
        "game::a::f",
        Some("const_item"),
        '6',
    ),
    (
        "R-2",
        1,
        "implements",
        "crates/game/src/a.rs",
        "item",
        "game::a::f",
        Some("function_item"),
        '7',
    ),
    (
        "stamina/AC-07",
        999_999_999,
        "verifies",
        "crates/game/tests/stamina.rs",
        "item",
        "game::test:stamina::drains",
        Some("function_item"),
        '8',
    ),
];

/// The blocks of the format-2 golden, in key order.
fn golden_blocks() -> Vec<String> {
    PLACES
        .iter()
        .map(
            |&(node, rev, relation, file, kind, symbol, item_kind, digit)| {
                block(node, rev, relation, file, kind, symbol, item_kind, digit)
            },
        )
        .collect()
}

/// The blocks of the format-1 golden, in key order.
fn golden_blocks_format_1() -> Vec<String> {
    PLACES
        .iter()
        .map(
            |&(node, rev, relation, file, kind, symbol, item_kind, digit)| {
                block_format_1(node, rev, relation, file, kind, symbol, item_kind, digit)
            },
        )
        .collect()
}

fn preamble() -> String {
    format!("{HEADER}\nformat = 2\nnorm_recipe = \"{NORM}\"\ncode_recipe = \"{CODE}\"\n")
}

fn preamble_format_1() -> String {
    format!("{HEADER}\nformat = 1\nnorm_recipe = \"{NORM}\"\ncode_recipe = \"{CODE_FORMAT_1}\"\n")
}

fn with_blocks(blocks: &[String]) -> String {
    let mut out = preamble();
    for block in blocks {
        out.push('\n');
        out.push_str(block);
    }
    out
}

fn with_blocks_format_1(blocks: &[String]) -> String {
    let mut out = preamble_format_1();
    for block in blocks {
        out.push('\n');
        out.push_str(block);
    }
    out
}

fn golden() -> String {
    with_blocks(&golden_blocks())
}

fn golden_format_1() -> String {
    with_blocks_format_1(&golden_blocks_format_1())
}

fn parse(text: &str) -> Lock {
    parse_lock(text, Some(&build())).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// The refusal of `text` under `build`: `spec.lock:<line>: <message>`.
fn refusal_under(text: &str, build: Option<&Recipes>) -> String {
    match parse_lock(text, build) {
        Ok(lock) => panic!("read, not refused: {lock:?}\n{text}"),
        Err(error) => error.to_string(),
    }
}

/// The refusal of `text` with the build's recipes.
fn refusal(text: &str) -> String {
    refusal_under(text, Some(&build()))
}

/// The 1-based line of the first line of `text` equal to `line`.
fn line_of(text: &str, line: &str) -> usize {
    text.lines()
        .position(|candidate| candidate == line)
        .unwrap_or_else(|| panic!("no line {line:?}"))
        + 1
}

/// The header line is the spec's.
#[test]
fn the_header_is_the_specs() {
    assert_eq!(LOCK_HEADER, HEADER);
}

/// RD-1: the format-2 golden reads and renders back byte for byte: the
/// header, `format = 2`, both recipes, every key in WA-4's order,
/// `levels = ["sig", "body"]` (basic strings, `", "` between, no trailing
/// comma), the three digests of every kind (a RON value's too), the
/// fingerprints and a decimal `body_bytes` of an item and a module only.
#[test]
fn the_golden_renders_back() {
    let text = golden();
    let lock = parse(&text);
    assert_eq!(lock.format, LockFormat::Two);
    assert_eq!(render_lock(&lock), text);
    // The keys of each kind, in order, as the golden spells them.
    let keys = |kind: &str| -> Vec<String> {
        let block = text
            .split("\n[[binding]]\n")
            .skip(1)
            .find(|block| block.contains(&format!("\nkind = \"{kind}\"\n")))
            .expect("a block of the kind");
        block
            .lines()
            .filter_map(|line| line.split_once(" = ").map(|(key, _)| key.to_owned()))
            .collect()
    };
    let all = [
        "node",
        "rev",
        "spec_hash",
        "relation",
        "file",
        "kind",
        "symbol",
        "item_kind",
        "levels",
        "sig_hash",
        "body_hash",
        "deps_hash",
        "skeleton_hash",
        "quote_hash",
        "body_bytes",
        "file_blob",
        "commit",
        "verified_by",
        "verified_at",
    ];
    assert_eq!(keys("item"), all);
    let module: Vec<&str> = all
        .iter()
        .copied()
        .filter(|key| *key != "item_kind")
        .collect();
    assert_eq!(keys("module"), module);
    let ron: Vec<&str> = module
        .iter()
        .copied()
        .filter(|key| !["skeleton_hash", "quote_hash", "body_bytes"].contains(key))
        .collect();
    assert_eq!(keys("ron"), ron);
    assert!(text.contains("\nlevels = [\"sig\", \"body\"]\n"));
    assert!(text.contains("\nbody_bytes = 201\n"));
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
/// and renders back as the LF golden. An empty lock is format 2: the
/// header and the three keys, nothing after.
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
    let empty = Lock::empty(build());
    assert_eq!(empty.format, LockFormat::Two);
    assert_eq!(render_lock(&empty), preamble());
}

/// RD-2: an equivalent in another TOML layout (keys reordered, spaces,
/// comments, literal strings, a multi-line `levels` array with a trailing
/// comma, an inline array of tables) renders as the golden.
#[test]
fn a_respaced_equivalent_renders_back() {
    let mut respaced = format!(
        "# hand-edited\ncode_recipe='{CODE}'\n  format=2\nnorm_recipe   =   \"{NORM}\"   # the spec side\n\n\n"
    );
    for block in golden_blocks() {
        let mut lines: Vec<&str> = block.lines().collect();
        let header = lines.remove(0);
        lines.reverse();
        respaced.push_str(header);
        respaced.push('\n');
        for line in lines {
            let (key, value) = line.split_once(" = ").expect("key = value");
            let value = if key == "levels" {
                "[\n  'sig',\n  \"body\",\n]".to_owned()
            } else {
                value.to_owned()
            };
            respaced.push_str(&format!("\t{key}= {value}  \n"));
        }
        respaced.push_str("\n\n");
    }
    assert_eq!(render_lock(&parse(&respaced)), golden());

    let mut inline =
        format!("format = 2\nnorm_recipe = \"{NORM}\"\ncode_recipe = \"{CODE}\"\nbinding = [\n");
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
    let line = line_of(&text, "line = 12");
    let message = refusal(&text);
    assert!(
        message.starts_with(&format!("spec.lock:{line}: ")),
        "{message}"
    );
    assert!(message.contains("line"), "{message}");
    let top = golden().replacen("format = 2\n", "format = 2\ncreated = \"2026-10-10\"\n", 1);
    let message = refusal(&top);
    assert!(message.starts_with("spec.lock:3: "), "{message}");
    assert!(message.contains("created"), "{message}");
}

/// RD-1 **M1** (`ast_hash` accepted in format 2): a format-2 item block
/// holding `ast_hash` is refused at that line, with the build's recipes
/// and without; a module's too. Format 1's key is unknown in format 2.
#[test]
fn format_2_refuses_ast_hash() {
    let ast = format!("ast_hash = \"b3:{}\"", hex('9', 64));
    for (kind, anchor) in [
        ("item", "item_kind = \"function_item\"\n"),
        ("module", "symbol = \"game::stamina\"\n"),
    ] {
        let text = golden().replacen(anchor, &format!("{anchor}{ast}\n"), 1);
        assert_ne!(text, golden(), "{kind}: the key inserted");
        let line = line_of(&text, &ast);
        for build in [Some(build()), None] {
            let message = refusal_under(&text, build.as_ref());
            assert!(
                message.starts_with(&format!("spec.lock:{line}: ")) && message.contains("ast_hash"),
                "{kind} {:?}: {message}",
                build.is_some()
            );
        }
    }
}

/// RD-3, RD-1: every refused form of "Reader" in a format-2 lock, at its
/// line, `spec.lock:<line>: <message>`.
#[test]
fn the_reader_refuses_every_wrong_form() {
    let base = golden();
    // The first block's lines: 7 node, 8 rev, 9 spec_hash, 10 relation,
    // 11 file, 12 kind, 13 symbol, 14 item_kind, 15 levels, 16 sig_hash,
    // 17 body_hash, 18 deps_hash, 19 skeleton_hash, 20 quote_hash, 21
    // body_bytes, 22 file_blob, 23 commit, 24 verified_by, 25 verified_at.
    assert_eq!(line_of(&base, "levels = [\"sig\", \"body\"]"), 15);
    assert_eq!(line_of(&base, "verified_at = \"2026-10-10\""), 25);
    let first = |from: &str, to: &str| base.replacen(from, to, 1);
    let one = hex('1', 64);
    let oid = hex('1', 40);
    let sig = digest('a', '1');
    let deps = digest('c', '1');
    let skeleton = digest('d', '1');
    let cases: Vec<(&str, String, Option<usize>, &str)> = vec![
        (
            "a missing recipe",
            first(&format!("norm_recipe = \"{NORM}\"\n"), ""),
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
            "an unknown kind",
            first("kind = \"item\"", "kind = \"file\""),
            Some(12),
            "kind",
        ),
        (
            "an item without item_kind",
            first("item_kind = \"function_item\"\n", ""),
            None,
            "item_kind",
        ),
        (
            "an item without levels",
            first("levels = [\"sig\", \"body\"]\n", ""),
            None,
            "levels",
        ),
        (
            "levels a string",
            first("levels = [\"sig\", \"body\"]", "levels = \"sig\""),
            Some(15),
            "levels",
        ),
        (
            "an item without sig_hash",
            first(&format!("sig_hash = \"{sig}\"\n"), ""),
            None,
            "sig_hash",
        ),
        (
            "an item without deps_hash",
            first(&format!("deps_hash = \"{deps}\"\n"), ""),
            None,
            "deps_hash",
        ),
        (
            "a short sig_hash",
            first(
                &format!("sig_hash = \"{sig}\""),
                &format!("sig_hash = \"b3:{}\"", hex('1', 62)),
            ),
            Some(16),
            "sig_hash",
        ),
        (
            "an uppercase deps_hash",
            first(
                &format!("deps_hash = \"{deps}\""),
                &format!("deps_hash = \"b3:{}\"", hex('C', 64)),
            ),
            Some(18),
            "deps_hash",
        ),
        (
            "an item without skeleton_hash",
            first(&format!("skeleton_hash = \"{skeleton}\"\n"), ""),
            None,
            "skeleton_hash",
        ),
        (
            "an item without body_bytes",
            first("body_bytes = 201\n", ""),
            None,
            "body_bytes",
        ),
        (
            "a negative body_bytes",
            first("body_bytes = 201", "body_bytes = -1"),
            Some(21),
            "body_bytes",
        ),
        (
            "body_bytes past u32",
            first("body_bytes = 201", "body_bytes = 4294967296"),
            Some(21),
            "body_bytes",
        ),
        (
            "body_bytes a string",
            first("body_bytes = 201", "body_bytes = \"201\""),
            Some(21),
            "body_bytes",
        ),
        (
            "body_bytes a float",
            first("body_bytes = 201", "body_bytes = 201.0"),
            Some(21),
            "body_bytes",
        ),
        (
            "an abbreviated file_blob",
            first(
                &format!("file_blob = \"{oid}\""),
                &format!("file_blob = \"{}\"", hex('1', 7)),
            ),
            Some(22),
            "file_blob",
        ),
        (
            "a 41-hex commit",
            first(
                &format!("commit = \"{oid}\""),
                &format!("commit = \"{}\"", hex('1', 41)),
            ),
            Some(23),
            "commit",
        ),
        (
            "verified by a test run",
            first("verified_by = \"owner\"", "verified_by = \"test\""),
            Some(24),
            "verified_by",
        ),
        (
            "no such day",
            first(
                "verified_at = \"2026-10-10\"",
                "verified_at = \"2026-02-30\"",
            ),
            Some(25),
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

/// RD-1 (`levels_out_of_canonical_order_refused`): `levels` is non-empty,
/// distinct and in the canonical order `path, sig, body, deps`; else
/// `spec.lock:<line>: levels: <empty|unknown <name>|duplicate <name>|
/// order>`, with the build's recipes and without. Every canonical
/// non-empty subset reads and renders back.
#[test]
fn levels_out_of_canonical_order_refused() {
    let base = golden();
    let line = line_of(&base, "levels = [\"sig\", \"body\"]");
    for (written, message) in [
        ("\"body\", \"sig\"", "levels: order"),
        ("\"deps\", \"path\"", "levels: order"),
        ("", "levels: empty"),
        ("\"Sig\"", "levels: unknown Sig"),
        ("\"sig\", \"tests\"", "levels: unknown tests"),
        ("\"sig\", \"sig\"", "levels: duplicate sig"),
        ("\"path\", \"path\"", "levels: duplicate path"),
    ] {
        let text = base.replacen(
            "levels = [\"sig\", \"body\"]",
            &format!("levels = [{written}]"),
            1,
        );
        for build in [Some(build()), None] {
            assert_eq!(
                refusal_under(&text, build.as_ref()),
                format!("spec.lock:{line}: {message}"),
                "[{written}]"
            );
        }
    }
    let names = ["path", "sig", "body", "deps"];
    for mask in 1u8..16 {
        let subset: Vec<String> = names
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, name)| format!("\"{name}\""))
            .collect();
        let text = base.replacen(
            "levels = [\"sig\", \"body\"]",
            &format!("levels = [{}]", subset.join(", ")),
            1,
        );
        assert_eq!(render_lock(&parse(&text)), text, "{subset:?}");
    }
}

/// RD-1: the lock's own copy of the level names is the marker grammar's
/// four, in canonical order (`code-identity.md` "Marker grammar"; the CLI
/// pins it against `specengine-code`'s `Level::ALL`).
#[test]
fn lock_levels_are_the_four_in_canonical_order() {
    let names: Vec<&str> = LockLevel::ALL.iter().map(|level| level.as_str()).collect();
    assert_eq!(names, ["path", "sig", "body", "deps"]);
}

/// RD-1: the three digests for every kind (a RON value's too); the
/// fingerprints and `body_bytes` iff `item` or `module`.
#[test]
fn digests_every_kind_fingerprints_items_and_modules() {
    let ron = block("R-1", 1, "configures", "a.ron", "ron", "root.a", None, '1');
    let module = block("R-1", 1, "implements", "a.rs", "module", "a", None, '2');
    // A RON value without a digest; with a fingerprint.
    for key in ["sig_hash", "body_hash", "deps_hash"] {
        let field = match key {
            "sig_hash" => 'a',
            "body_hash" => 'b',
            _ => 'c',
        };
        let without = ron.replacen(&format!("{key} = \"{}\"\n", digest(field, '1')), "", 1);
        assert_ne!(without, ron);
        let message = refusal(&with_blocks(&[without]));
        assert!(message.contains(key), "ron without {key}: {message}");
    }
    for (key, value) in [
        ("skeleton_hash", format!("\"{}\"", digest('d', '1'))),
        ("quote_hash", format!("\"{}\"", digest('e', '1'))),
        ("body_bytes", "12".to_owned()),
    ] {
        let with = ron.replacen("file_blob", &format!("{key} = {value}\nfile_blob"), 1);
        let message = refusal(&with_blocks(&[with]));
        assert!(message.contains(key), "ron with {key}: {message}");
    }
    // A module without a fingerprint; with `item_kind`.
    for (key, line) in [
        (
            "skeleton_hash",
            format!("skeleton_hash = \"{}\"\n", digest('d', '2')),
        ),
        (
            "quote_hash",
            format!("quote_hash = \"{}\"\n", digest('e', '2')),
        ),
        ("body_bytes", "body_bytes = 202\n".to_owned()),
    ] {
        let without = module.replacen(&line, "", 1);
        assert_ne!(without, module);
        let message = refusal(&with_blocks(&[without]));
        assert!(message.contains(key), "module without {key}: {message}");
    }
    let with_kind = module.replacen("levels", "item_kind = \"mod_item\"\nlevels", 1);
    let message = refusal(&with_blocks(&[with_kind]));
    assert!(message.contains("item_kind"), "{message}");
    let ron_kind = ron.replacen("levels", "item_kind = \"function_item\"\nlevels", 1);
    let message = refusal(&with_blocks(&[ron_kind]));
    assert!(message.contains("item_kind"), "{message}");
    // Both read as written.
    let text = with_blocks(&[module, ron]);
    assert_eq!(render_lock(&parse(&text)), text);
}

/// RD-1: `body_bytes` is a TOML integer 0 to 4 294 967 295, rendered in
/// decimal (no sign, `_` or leading zero) whatever layout it was read in.
#[test]
fn body_bytes_is_a_decimal_u32() {
    let base = golden();
    for (written, rendered) in [
        ("0", "0"),
        ("4294967295", "4294967295"),
        ("+212", "212"),
        ("1_000", "1000"),
        ("0x10", "16"),
    ] {
        let text = base.replacen("body_bytes = 201", &format!("body_bytes = {written}"), 1);
        let want = base.replacen("body_bytes = 201", &format!("body_bytes = {rendered}"), 1);
        assert_eq!(render_lock(&parse(&text)), want, "{written}");
    }
}

/// RD-1, Q27 (1): a `format` past 2 is newer, one below 1 not this
/// build's; both at the `format` line, with the build's recipes and
/// without.
#[test]
fn other_formats_are_refused_by_their_text() {
    for (number, message) in [
        (3, "format 3 is newer than this build's 2"),
        (9, "format 9 is newer than this build's 2"),
        (0, "format 0 is not this build's: it reads formats 1 and 2"),
        (
            -1,
            "format -1 is not this build's: it reads formats 1 and 2",
        ),
    ] {
        let text = golden().replacen("format = 2\n", &format!("format = {number}\n"), 1);
        for build in [Some(build()), None] {
            assert_eq!(
                refusal_under(&text, build.as_ref()),
                format!("spec.lock:2: {message}"),
                "format {number}"
            );
        }
    }
}

/// RD-1, RD-2 (Q10): without the build's recipes format 1 reads in full
/// by its own rules: the format-1 golden reads, its format known, every
/// entry kept; `ast_hash` iff `item` (missing on an item, present on a
/// module or a RON value: refused); format 2's keys unknown in it. The
/// recipes are never compared.
#[test]
fn format_1_reads_without_the_build() {
    let text = golden_format_1();
    let lock = parse_lock(&text, None).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(lock.format, LockFormat::One);
    assert_eq!(lock.entries.len(), 8);
    assert_eq!(lock.recipes.code, CODE_FORMAT_1);
    // Another grammar's format-1 lock reads too.
    let other = text.replace(
        CODE_FORMAT_1,
        "specengine-hash/v1 tree-sitter-rust/0.20.0 abi/14",
    );
    assert!(parse_lock(&other, None).is_ok());

    let one = hex('1', 64);
    let item_without = text.replacen(&format!("ast_hash = \"b3:{one}\"\n"), "", 1);
    let message = refusal_under(&item_without, None);
    assert!(message.contains("ast_hash"), "{message}");
    let module = block_format_1("R-1", 1, "implements", "a.rs", "module", "a", None, '2').replace(
        "file_blob",
        &format!("ast_hash = \"b3:{}\"\nfile_blob", hex('2', 64)),
    );
    let message = refusal_under(&with_blocks_format_1(&[module]), None);
    assert!(message.contains("ast_hash"), "{message}");
    for (key, value) in [
        ("levels", "[\"sig\", \"body\"]".to_owned()),
        ("sig_hash", format!("\"b3:{one}\"")),
        ("body_bytes", "12".to_owned()),
    ] {
        let with = text.replacen(
            "verified_by = \"owner\"\n",
            &format!("{key} = {value}\nverified_by = \"owner\"\n"),
            1,
        );
        let line = line_of(&with, &format!("{key} = {value}"));
        let message = refusal_under(&with, None);
        assert!(
            message.starts_with(&format!("spec.lock:{line}: ")) && message.contains(key),
            "{key} in format 1: {message}"
        );
    }
}

/// Q27 with the build's recipes, the first failing check the error:
/// (1) unreadable or a `format` neither 1 nor 2; (2) another
/// `norm_recipe`, today's text, no step; (3) format 1, naming its
/// `code_recipe` and the step; (4) another `code_recipe`, the step; (5)
/// the entries.
#[test]
fn the_build_refuses_in_q27_order() {
    let norm_text = |its: &str, line: usize| {
        format!(
            "spec.lock:{line}: `norm_recipe` is \"{its}\", this build's is \"{NORM}\": the lock was written by another build (a grammar or recipe change re-hashes every entry)"
        )
    };
    let format_1_text = format!(
        "spec.lock:2: format 1 (\"{CODE_FORMAT_1}\"), this build's is 2 (\"{CODE}\"): run spec lock rebase on a terminal"
    );
    let other_code = "specengine-hash/v3 tree-sitter-rust/0.24.1 abi/15";
    let code_text = format!(
        "spec.lock:4: `code_recipe` is \"{other_code}\", this build's is \"{CODE}\": run spec lock rebase on a terminal"
    );
    let format_1 = golden_format_1();
    let cases: Vec<(&str, String, String)> = vec![
        ("format 1", format_1.clone(), format_1_text.clone()),
        (
            "format 1 with a broken entry",
            format_1.replacen("rev = 3", "rev = 0", 1),
            format_1_text.clone(),
        ),
        (
            "format 1 of another norm_recipe",
            format_1.replacen(NORM, "specengine-norm/v0", 1),
            norm_text("specengine-norm/v0", 3),
        ),
        (
            "format 1 of another grammar",
            format_1.replacen(
                CODE_FORMAT_1,
                "specengine-hash/v1 tree-sitter-rust/0.20.0 abi/14",
                1,
            ),
            format!(
                "spec.lock:2: format 1 (\"specengine-hash/v1 tree-sitter-rust/0.20.0 abi/14\"), this build's is 2 (\"{CODE}\"): run spec lock rebase on a terminal"
            ),
        ),
        (
            "another code_recipe",
            golden().replacen(CODE, other_code, 1),
            code_text.clone(),
        ),
        (
            "another code_recipe with a broken entry",
            golden()
                .replacen(CODE, other_code, 1)
                .replacen("rev = 3", "rev = 0", 1),
            code_text,
        ),
        (
            "another norm_recipe",
            golden().replacen(NORM, "specengine-norm/v2", 1),
            norm_text("specengine-norm/v2", 3),
        ),
        (
            "another norm_recipe and code_recipe",
            golden()
                .replacen(NORM, "specengine-norm/v2", 1)
                .replacen(CODE, other_code, 1),
            norm_text("specengine-norm/v2", 3),
        ),
        (
            "format 3 of another norm_recipe",
            golden()
                .replacen("format = 2\n", "format = 3\n", 1)
                .replacen(NORM, "specengine-norm/v2", 1),
            "spec.lock:2: format 3 is newer than this build's 2".to_owned(),
        ),
    ];
    let mut failures = Vec::new();
    for (context, text, want) in cases {
        let got = refusal(&text);
        if got != want {
            failures.push(format!("{context}:\n  want {want}\n  got  {got}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Q10: without the build's recipes nothing is compared: a format-2 lock
/// of another `code_recipe` or `norm_recipe` reads, its recipes as
/// written, its format 2 (no reader upgrades or downgrades in memory).
#[test]
fn without_the_build_recipes_are_never_compared() {
    let other = golden()
        .replacen(CODE, "specengine-hash/v9 tree-sitter-rust/9.9.9 abi/99", 1)
        .replacen(NORM, "specengine-norm/v9", 1);
    let lock = parse_lock(&other, None).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(lock.format, LockFormat::Two);
    assert_eq!(lock.recipes.norm, "specengine-norm/v9");
    assert_eq!(render_lock(&lock), other);
}

/// RD-3: a duplicate binding key is refused; the same key but another
/// `item_kind` is two entries.
#[test]
fn duplicate_keys_are_refused() {
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
    let message = refusal(&with_blocks(&[one.clone(), two]));
    assert!(message.starts_with("spec.lock:"), "{message}");
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

// ------------------------------------------------------- lock_code, merges

/// The golden's first entry (`R-12`'s item), parsed from `text`.
fn first_entry(
    text: &str,
) -> (
    specengine_core::lock::EntryKey,
    specengine_core::lock::Verified,
) {
    parse(text).entries.pop_first().expect("an entry")
}

/// RD-1 (`lock_code_has_no_fingerprint`): `lock_code` is the entry's
/// `levels`, `sig_hash`, `body_hash`, `deps_hash` lines as `render_lock`
/// writes them, LF-joined, no final LF, for every kind; a fingerprint or
/// `body_bytes` changes nothing of it, a level or a digest does.
#[test]
fn lock_code_has_no_fingerprint() {
    let text = golden();
    let lock = parse(&text);
    for (key, verified) in &lock.entries {
        let block =
            text.split("\n[[binding]]\n")
                .skip(1)
                .find(|block| {
                    block.contains(&format!("\nkind = \"{}\"\n", key.kind.as_str()))
                        && block.contains(&format!("\nsymbol = \"{}\"\n", key.symbol))
                        && block.contains(&format!("node = \"{}\"\n", key.node))
                        && key.item_kind.as_ref().is_none_or(|kind| {
                            block.contains(&format!("\nitem_kind = \"{kind}\"\n"))
                        })
                })
                .expect("the entry's block");
        let want: Vec<&str> = block
            .lines()
            .filter(|line| {
                ["levels = ", "sig_hash = ", "body_hash = ", "deps_hash = "]
                    .iter()
                    .any(|prefix| line.starts_with(prefix))
            })
            .collect();
        assert_eq!(want.len(), 4, "{block}");
        assert_eq!(lock_code(verified), want.join("\n"), "{key:?}");
    }
    let (_, base) = first_entry(&text);
    for (from, to) in [
        (digest('d', '1'), digest('f', '1')),
        (digest('e', '1'), digest('f', '1')),
        ("body_bytes = 201".to_owned(), "body_bytes = 7".to_owned()),
    ] {
        let (_, other) = first_entry(&text.replacen(&from, &to, 1));
        assert_ne!(other, base, "{from} changed");
        assert_eq!(
            lock_code(&other),
            lock_code(&base),
            "{from}: not in lock_code"
        );
    }
    for (from, to) in [
        (digest('a', '1'), digest('f', '1')),
        (digest('b', '1'), digest('f', '1')),
        (digest('c', '1'), digest('f', '1')),
        (
            "levels = [\"sig\", \"body\"]".to_owned(),
            "levels = [\"sig\"]".to_owned(),
        ),
    ] {
        let (_, other) = first_entry(&text.replacen(&from, &to, 1));
        assert_ne!(lock_code(&other), lock_code(&base), "{from}: in lock_code");
    }
}

/// The lock the merge tests start from: `N-1`'s item (digit 1), module
/// (2) and RON value (4) at rev 1 under one spec hash, and `N-2`'s item
/// (6).
fn merge_golden() -> String {
    let shared = |block: String, digit: char| {
        block.replacen(
            &format!("spec_hash = \"b3:{}\"", hex(digit, 64)),
            &format!("spec_hash = \"b3:{}\"", hex('e', 64)),
            1,
        )
    };
    with_blocks(&[
        shared(
            block(
                "N-1",
                1,
                "implements",
                "a.rs",
                "item",
                "a::f",
                Some("function_item"),
                '1',
            ),
            '1',
        ),
        shared(
            block("N-1", 1, "implements", "a.rs", "module", "a", None, '2'),
            '2',
        ),
        shared(
            block("N-1", 1, "configures", "a.ron", "ron", "root.a", None, '4'),
            '4',
        ),
        block(
            "N-2",
            1,
            "implements",
            "b.rs",
            "item",
            "b::g",
            Some("function_item"),
            '6',
        ),
    ])
}

/// An accept of `N-1` over [`merge_golden`], its fresh entries read from
/// `fresh` (that lock with one value of `N-1`'s item changed): the item's
/// row and the merged lock.
fn accept_first(fresh: &str) -> (Change, String) {
    let old = parse(&merge_golden());
    let entries: Vec<_> = parse(fresh)
        .entries
        .into_iter()
        .filter(|(key, _)| key.node == "N-1")
        .collect();
    assert_eq!(entries.len(), 3);
    let input = AcceptInput {
        node: "N-1".to_owned(),
        rev: entries[0].1.rev,
        spec_hash: entries[0].1.spec_hash.clone(),
        fresh: entries,
        unlockable: Vec::new(),
        keyless: Vec::new(),
    };
    let merge = merge_accept(&old, &input).expect("merged");
    let row = merge
        .rows
        .iter()
        .find(|row| row.place.kind == "item")
        .expect("the item's row");
    assert_eq!(
        merge
            .rows
            .iter()
            .filter(|row| row.place.kind != "item" && row.change == Change::Unchanged)
            .count(),
        2,
        "the module and the RON value unchanged: {:?}",
        merge.rows
    );
    (row.change, render_lock(&merge.lock))
}

/// RD-1 **M2** (a digest ignored by `same_pair`): an accept whose fresh
/// entry differs from the old one in `deps_hash` alone refreshes it (so
/// in `sig_hash`, `body_hash`, `levels`, `file_blob`); one differing in a
/// fingerprint or `body_bytes` alone is `unchanged`, the old entry kept
/// byte for byte (Q12); equal is `unchanged`.
#[test]
fn changed_deps_digest_refreshes() {
    let old = merge_golden();
    let (change, lock) = accept_first(&old);
    assert_eq!(change, Change::Unchanged, "equal");
    assert_eq!(lock, old);
    let mut failures = Vec::new();
    for (context, from, to) in [
        ("deps_hash", digest('c', '1'), digest('f', '1')),
        ("sig_hash", digest('a', '1'), digest('f', '1')),
        ("body_hash", digest('b', '1'), digest('f', '1')),
        (
            "levels",
            "levels = [\"sig\", \"body\"]".to_owned(),
            "levels = [\"body\"]".to_owned(),
        ),
        (
            "file_blob",
            format!("file_blob = \"{}\"", hex('1', 40)),
            format!("file_blob = \"{}\"", hex('f', 40)),
        ),
    ] {
        let fresh = old.replacen(&from, &to, 1);
        let (change, lock) = accept_first(&fresh);
        if change != Change::Refreshed || lock != fresh {
            failures.push(format!("{context}: {change:?}\n{lock}"));
        }
    }
    for (context, from, to) in [
        ("skeleton_hash", digest('d', '1'), digest('f', '1')),
        ("quote_hash", digest('e', '1'), digest('f', '1')),
        (
            "body_bytes",
            "body_bytes = 201".to_owned(),
            "body_bytes = 7".to_owned(),
        ),
    ] {
        let fresh = old.replacen(&from, &to, 1);
        let (change, lock) = accept_first(&fresh);
        if change != Change::Unchanged || lock != old {
            failures.push(format!("{context} alone: {change:?}\n{lock}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// "Accept": `--editorial` and the editorial apply keep every code field
/// byte for byte: a refresh of `R-12` at rev 3 sets `spec_hash` alone.
#[test]
fn editorial_refresh_keeps_every_code_field() {
    let old = golden();
    let new_hash = format!("b3:{}", hex('f', 64));
    let editorial = refresh_editorial(&parse(&old), "R-12", 3, &new_hash, None);
    assert_eq!(editorial.refreshed(), 5);
    // `R-12`'s five entries (digits 1-5) at rev 3 get the new hash; the
    // other nodes' and every code field stay as they were.
    let mut want = old.clone();
    for digit in ['1', '2', '3', '4', '5'] {
        want = want.replace(
            &format!("spec_hash = \"b3:{}\"", hex(digit, 64)),
            &format!("spec_hash = \"{new_hash}\""),
        );
    }
    assert_eq!(render_lock(&editorial.lock), want);
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
