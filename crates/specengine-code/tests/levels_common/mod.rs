//! Helpers of the docs/features/hash-levels.md (5a-1) tests in this crate
//! (`hash_levels_props.rs`: TE-2, TE-6 over TE-2's edits, the recipe
//! oracles; `hash_levels_library.rs`: TE-4, TE-5): the fixture
//! `fixtures/hash-levels/` (read only), a snapshot of every unit of a source
//! text, the targeted edits, and an independent re-statement of the v3
//! recipe built on v2's public walk (`hash::normalize`).
//!
//! PHASE-B: the 5a-1 entry points are used by the spec's names ("Entry
//! points"); `unit_of` is the only function reading the shape of
//! `LevelState` and `LevelHashes`.
//!
//! Only the hash-levels test binaries include this module.

#![allow(dead_code)]

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::thread;

use specengine_code::grammar::{self, TREE_SITTER_RUST_VERSION};
use specengine_code::hash::{
    Digest, ErrorCategory, HashState, attached_attributes, is_comment, name_node, normalize,
};
use specengine_code::{LevelState, RustParser, analyze_tree, item_levels, module_levels};
use tree_sitter::{Node, Tree};

/// Walks of `broken/nesting.rs` recurse once per nested use run; depth is
/// RD-7's subject, so every snapshot runs on a generous stack.
pub const STACK: usize = 16 * 1024 * 1024;

/// The fields TE-2 holds stable (every level and fingerprint but `quote`,
/// which reads the lines beside the item by design: Q16).
pub const STABLE: [&str; 5] = ["sig", "body", "deps", "skeleton", "body_bytes"];
pub const ALL: [&str; 6] = ["sig", "body", "deps", "skeleton", "quote", "body_bytes"];

// ------------------------------------------------------------------ fixture

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/hash-levels")
}

pub fn read(relative: &str) -> String {
    let path = fixture_root().join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        !text.contains('\r'),
        "{relative}: fixture sources are LF only"
    );
    text
}

/// Every `.rs` file of the fixture outside `variants/`, relative, sorted.
pub fn base_files() -> Vec<String> {
    let root = fixture_root();
    let mut out = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("a directory entry").path();
            let relative = path
                .strip_prefix(&root)
                .expect("below the fixture root")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if relative != "variants" {
                    pending.push(path);
                }
            } else if relative.ends_with(".rs") {
                out.push(relative);
            }
        }
    }
    out.sort();
    assert!(out.len() >= 10, "the fixture holds its files: {out:?}");
    out
}

/// The files with committed rustfmt variants: every `.rs` file under `src/`.
pub fn variant_files() -> Vec<String> {
    let files: Vec<String> = base_files()
        .into_iter()
        .filter(|f| f.starts_with("src/"))
        .collect();
    assert!(!files.is_empty());
    files
}

pub const VARIANTS: [&str; 2] = ["fmt-default", "fmt-60"];

pub fn read_variant(variant: &str, relative: &str) -> String {
    read(&format!("variants/{variant}/{relative}"))
}

pub fn crlf(source: &str) -> String {
    source.replace('\n', "\r\n")
}

pub fn parse(parser: &mut RustParser, source: &str) -> Tree {
    parser.parse(source).expect("parse completes")
}

pub fn new_parser() -> RustParser {
    RustParser::new().expect("grammar loads")
}

/// Runs `f` on a thread with [`STACK`] bytes of stack.
pub fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("thread spawns")
        .join()
        .expect("must not panic or overflow")
}

// ----------------------------------------------------------------- snapshot

/// One unit's levels and fingerprints, copied out of `LevelHashes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Levels {
    pub sig: Digest,
    pub body: Digest,
    pub deps: Digest,
    pub skeleton: Digest,
    pub quote: Digest,
    pub body_bytes: u32,
    pub trivial: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    Hashed(Levels),
    Refused(Vec<ErrorCategory>),
}

/// PHASE-B: the one place that reads the shape of `LevelState` and `LevelHashes`.
pub fn unit_of(state: &LevelState) -> Unit {
    match state {
        LevelState::Hashed(levels) => Unit::Hashed(Levels {
            sig: levels.sig,
            body: levels.body,
            deps: levels.deps,
            skeleton: levels.skeleton,
            quote: levels.quote,
            body_bytes: levels.body_bytes,
            trivial: levels.is_trivial(),
        }),
        LevelState::CannotVerify { categories } => Unit::Refused(categories.clone()),
    }
}

impl Unit {
    pub fn levels(&self) -> Option<&Levels> {
        match self {
            Self::Hashed(levels) => Some(levels),
            Self::Refused(_) => None,
        }
    }
}

/// A field of `Levels` as bytes, so digests and counts compare alike.
pub fn field(levels: &Levels, name: &str) -> Vec<u8> {
    match name {
        "sig" => levels.sig.as_bytes().to_vec(),
        "body" => levels.body.as_bytes().to_vec(),
        "deps" => levels.deps.as_bytes().to_vec(),
        "skeleton" => levels.skeleton.as_bytes().to_vec(),
        "quote" => levels.quote.as_bytes().to_vec(),
        "body_bytes" => levels.body_bytes.to_le_bytes().to_vec(),
        other => panic!("unknown field {other}"),
    }
}

/// The fields among `names` where `a` and `b` differ.
pub fn changed(a: &Levels, b: &Levels, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| field(a, name) != field(b, name))
        .map(|name| (*name).to_owned())
        .collect()
}

/// An item's identity across two texts: kind, label, owner, inline-`mod`
/// path and the occurrence among items sharing all four.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub kind: String,
    pub label: String,
    pub owner: Option<String>,
    pub mod_path: Vec<String>,
    pub nth: usize,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.kind, self.label)?;
        if let Some(owner) = &self.owner {
            write!(f, " (in {owner})")?;
        }
        if !self.mod_path.is_empty() {
            write!(f, " (mod {})", self.mod_path.join("::"))?;
        }
        if self.nth > 0 {
            write!(f, " #{}", self.nth)?;
        }
        Ok(())
    }
}

/// Every unit of one source text, items in analysis order.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub keys: Vec<Key>,
    /// `analyze_tree`'s v2 states (`hash_item`).
    pub v2: Vec<HashState>,
    pub units: Vec<Unit>,
    pub module: Unit,
    /// From the first attached attribute (else the item) to the item's end.
    pub spans: Vec<Range<usize>>,
    /// The item's subtree holds an N5 run (two or more `mod x;` or
    /// `extern crate` siblings in a row): where v2's walk and v3's differ.
    pub n5: Vec<bool>,
    pub root_has_error: bool,
}

impl Snapshot {
    pub fn index_of(&self, key: &Key) -> Option<usize> {
        self.keys.iter().position(|k| k == key)
    }

    pub fn v2_digest(&self, index: usize) -> Option<Digest> {
        self.v2[index].digest()
    }
}

/// [`snapshot_with`] on a fresh parser.
pub fn snapshot(source: &str) -> Snapshot {
    snapshot_with(&mut new_parser(), source)
}

/// [`snapshot`] on a thread with a big stack.
pub fn snapshot_owned(source: String) -> Snapshot {
    on_big_stack(move || snapshot(&source))
}

pub fn snapshot_with(parser: &mut RustParser, source: &str) -> Snapshot {
    let tree = parse(parser, source);
    let analysis = analyze_tree(&tree, source);
    let indexes: Vec<usize> = (0..analysis.items.len()).collect();
    let states = item_levels(&tree, source, &indexes);
    assert_eq!(states.len(), indexes.len(), "one entry per index asked");
    let root = tree.root_node();
    let mut keys: Vec<Key> = Vec::new();
    let mut spans = Vec::new();
    let mut n5 = Vec::new();
    for item in &analysis.items {
        let mut key = Key {
            kind: item.kind.clone(),
            label: item.label.clone(),
            owner: item.owner.clone(),
            mod_path: item.mod_path.clone(),
            nth: 0,
        };
        key.nth = keys
            .iter()
            .filter(|k| {
                k.kind == key.kind
                    && k.label == key.label
                    && k.owner == key.owner
                    && k.mod_path == key.mod_path
            })
            .count();
        keys.push(key);
        let node = item_node(root, &item.byte_range, &item.kind);
        spans.push(span_of(node));
        n5.push(has_n5_run(node));
    }
    Snapshot {
        keys,
        v2: analysis
            .items
            .iter()
            .map(|item| item.state.clone())
            .collect(),
        units: states
            .iter()
            .enumerate()
            .map(|(index, state)| {
                unit_of(
                    state
                        .as_ref()
                        .unwrap_or_else(|| panic!("item {index} has no entry")),
                )
            })
            .collect(),
        module: unit_of(&module_levels(&tree, source)),
        spans,
        n5,
        root_has_error: root.has_error(),
    }
}

/// The index of the one item `kind label` (any owner, any `mod` path).
pub fn index_named(snap: &Snapshot, kind: &str, label: &str) -> usize {
    let hits: Vec<usize> = snap
        .keys
        .iter()
        .enumerate()
        .filter(|(_, k)| k.kind == kind && k.label == label)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(hits.len(), 1, "one {kind} {label} among {:?}", snap.keys);
    hits[0]
}

/// The levels of the one item `kind label`, which must be hashed.
pub fn one(snap: &Snapshot, kind: &str, label: &str) -> Levels {
    let index = index_named(snap, kind, label);
    *snap.units[index]
        .levels()
        .unwrap_or_else(|| panic!("{kind} {label} is refused: {:?}", snap.units[index]))
}

/// The file module's levels, which must be hashed.
pub fn module(snap: &Snapshot) -> Levels {
    *snap
        .module
        .levels()
        .unwrap_or_else(|| panic!("the module is refused: {:?}", snap.module))
}

/// `call_targets` of the one item `kind label` of `source`.
pub fn targets_of(source: &str, kind: &str, label: &str) -> Vec<String> {
    let tree = parse(&mut new_parser(), source);
    let analysis = analyze_tree(&tree, source);
    let hits: Vec<&specengine_code::ItemRecord> = analysis
        .items
        .iter()
        .filter(|item| item.kind == kind && item.label == label)
        .collect();
    assert_eq!(hits.len(), 1, "one {kind} {label}");
    let node = item_node(tree.root_node(), &hits[0].byte_range, kind);
    specengine_code::call_targets(node, source)
}

/// BLAKE3(`level_header()` || `put(tag)` || `stream`), through the crate's
/// own header (the header's bytes are L1's subject, not this helper's).
pub fn digest_after_header(tag: &str, stream: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(specengine_code::level_header());
    let mut tagged = Vec::new();
    put(&mut tagged, tag.as_bytes());
    hasher.update(&tagged);
    hasher.update(stream);
    *hasher.finalize().as_bytes()
}

/// `deps` over `targets` (byte-sorted, deduplicated by the caller).
pub fn deps_after_header(targets: &[&str]) -> [u8; 32] {
    let mut stream = Vec::new();
    for target in targets {
        put(&mut stream, target.as_bytes());
    }
    digest_after_header("deps", &stream)
}

/// The item node of an `ItemRecord`, by its byte range.
pub fn item_node<'t>(root: Node<'t>, range: &Range<usize>, kind: &str) -> Node<'t> {
    let node = root
        .descendant_for_byte_range(range.start, range.end)
        .expect("a node over the item's range");
    assert_eq!(node.kind(), kind, "the node at {range:?}");
    assert_eq!(node.byte_range(), *range);
    node
}

pub fn span_of(item: Node) -> Range<usize> {
    let start = attached_attributes(item)
        .first()
        .map_or(item.start_byte(), Node::start_byte);
    start..item.end_byte()
}

fn n5_class(node: Node) -> Option<&'static str> {
    match node.kind() {
        "mod_item" if node.child_by_field_name("body").is_none() => Some("mod"),
        "extern_crate_declaration" => Some("extern"),
        _ => None,
    }
}

/// Two or more `mod x;` (or `extern crate`) siblings in a row anywhere in
/// `node`'s subtree, attributes and comments between allowed.
pub fn has_n5_run(node: Node) -> bool {
    let mut stack = vec![node];
    while let Some(parent) = stack.pop() {
        let mut cursor = parent.walk();
        let mut previous: Option<&'static str> = None;
        for child in parent.children(&mut cursor) {
            match n5_class(child) {
                Some(class) if previous == Some(class) => return true,
                Some(class) => previous = Some(class),
                None if child.kind() == "attribute_item" || is_comment(child) => {}
                None => previous = None,
            }
            stack.push(child);
        }
    }
    false
}

// ----------------------------------------------------------- targeted edits

/// Inserts `text` at `at`, keeping the rest.
pub fn insert(source: &str, at: usize, text: &str) -> String {
    format!("{}{text}{}", &source[..at], &source[at..])
}

/// Inserts `#[cfg(all())]` and an LF before every item at `indexes` (its
/// first attached attribute, else the item): 5a-2's neighbour edit.
pub fn with_cfg_before(source: &str, spans: &[Range<usize>], indexes: &[usize]) -> String {
    let mut starts: Vec<usize> = indexes.iter().map(|&i| spans[i].start).collect();
    starts.sort_unstable();
    starts.dedup();
    let mut out = source.to_owned();
    for at in starts.into_iter().rev() {
        out = insert(&out, at, "#[cfg(all())]\n");
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Targeted {
    /// Inside the body, no call: `body` (and `skeleton`, `body_bytes`) only.
    Body,
    /// Inside the body, a call: `deps` (and `body`); for a macro, tokens
    /// that look like a call, which `deps` never sees.
    Deps,
}

/// The body node(s) of `item` by "The body field" table, read here from the
/// grammar's fields, independently of `body_nodes`.
pub fn body_by_hand(item: Node) -> Vec<Node> {
    match item.kind() {
        "function_signature_item" => Vec::new(),
        "const_item" | "static_item" => item.child_by_field_name("value").into_iter().collect(),
        "type_item" => item.child_by_field_name("type").into_iter().collect(),
        "macro_definition" => {
            let mut cursor = item.walk();
            item.named_children(&mut cursor)
                .filter(|child| child.kind() == "macro_rule")
                .collect()
        }
        _ => item.child_by_field_name("body").into_iter().collect(),
    }
}

/// The source with one targeted edit inside `item`'s body; `None` when the
/// item has no body to edit.
pub fn targeted(source: &str, item: Node, which: Targeted) -> Option<String> {
    let bodies = body_by_hand(item);
    let body = *bodies.first()?;
    let deps = which == Targeted::Deps;
    let after_open = body.start_byte() + 1;
    let edit = match (item.kind(), body.kind()) {
        ("function_item", _) => insert(
            source,
            after_open,
            if deps {
                " hl_probe();"
            } else {
                " let _hl_probe = 1;"
            },
        ),
        (_, "field_declaration_list") => insert(
            source,
            after_open,
            if deps {
                " hl_extra: [u8; hl_len()],"
            } else {
                " hl_extra: u8,"
            },
        ),
        (_, "ordered_field_declaration_list") => insert(
            source,
            after_open,
            if deps { " [u8; hl_len()]," } else { " u8," },
        ),
        (_, "enum_variant_list") => insert(
            source,
            after_open,
            if deps {
                " HlExtra = hl_disc(),"
            } else {
                " HlExtra,"
            },
        ),
        (_, "declaration_list") => insert(
            source,
            body.end_byte() - 1,
            if deps {
                " const HL_PROBE: u8 = hl_value(); "
            } else {
                " const HL_PROBE: u8 = 1; "
            },
        ),
        ("const_item" | "static_item", _) => {
            let value = &source[body.byte_range()];
            let wrapped = if deps {
                format!("{{ hl_probe(); {value} }}")
            } else {
                format!("{{ let _hl_probe = 1; {value} }}")
            };
            format!(
                "{}{wrapped}{}",
                &source[..body.start_byte()],
                &source[body.end_byte()..]
            )
        }
        ("type_item", _) => {
            let ty = &source[body.byte_range()];
            let wrapped = if deps {
                format!("[{ty}; hl_len()]")
            } else {
                format!("[{ty}; 1]")
            };
            format!(
                "{}{wrapped}{}",
                &source[..body.start_byte()],
                &source[body.end_byte()..]
            )
        }
        ("macro_definition", _) => {
            let right = body
                .child_by_field_name("right")
                .expect("a rule's right side");
            insert(
                source,
                right.start_byte() + 1,
                if deps { " hl_probe()" } else { " hl_tok" },
            )
        }
        (kind, body_kind) => panic!("no targeted edit for {kind} with a {body_kind} body"),
    };
    Some(edit)
}

// ------------------------------------------------- the recipe, independently

/// `len(b)` as u32 LE, then `b`.
pub fn put(out: &mut Vec<u8>, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).expect("a length below 4 GiB");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
}

/// L1: `level_header()` = `put(LEVEL_RECIPE) || put(TREE_SITTER_RUST_VERSION)
/// || put(abi as decimal text)`, built as v2's `recipe_header()` is.
pub fn header_by_hand() -> Vec<u8> {
    let mut header = Vec::new();
    put(&mut header, b"specengine-hash/v3");
    put(&mut header, TREE_SITTER_RUST_VERSION.as_bytes());
    put(
        &mut header,
        grammar::grammar_info().abi.to_string().as_bytes(),
    );
    header
}

/// BLAKE3(`level_header()` || `put(tag)` || `stream`).
pub fn digest_by_hand(tag: &str, stream: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&header_by_hand());
    let mut tagged = Vec::new();
    put(&mut tagged, tag.as_bytes());
    hasher.update(&tagged);
    hasher.update(stream);
    *hasher.finalize().as_bytes()
}

/// v2's walk of `node` (`hash::normalize`), complete.
pub fn v2_walk(node: Node, source: &str, skip: Option<Node>) -> Vec<u8> {
    let mut out = Vec::new();
    assert!(
        normalize(node, source.as_bytes(), skip, &mut out),
        "the v2 walk of a fixture node completes"
    );
    out
}

/// L2: `sig`'s stream: the attributes, then the item without its name and
/// body nodes, each child walked as v2 walks it: `"(" || put(kind) ||
/// walk(child)... || ")"`. Valid where walk3 is v2's walk: no N5 run, no CR.
pub fn sig_stream_by_hand(item: Node, source: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for attribute in attached_attributes(item) {
        out.extend(v2_walk(attribute, source, None));
    }
    let name = name_node(item).map(|n| n.id());
    let bodies: Vec<usize> = body_by_hand(item).iter().map(Node::id).collect();
    out.push(b'(');
    put(&mut out, item.kind().as_bytes());
    let mut cursor = item.walk();
    for child in item.children(&mut cursor) {
        if Some(child.id()) == name || bodies.contains(&child.id()) {
            continue;
        }
        out.extend(v2_walk(child, source, None));
    }
    out.push(b')');
    out
}

/// L2: `body`'s stream: v2's walk of every body node, in source order.
pub fn body_stream_by_hand(item: Node, source: &str) -> Vec<u8> {
    body_by_hand(item)
        .into_iter()
        .flat_map(|body| v2_walk(body, source, None))
        .collect()
}

/// The sum of the leaf text lengths in a v2 walk stream: `(` opens a node
/// (then `put(kind)`), `)` closes it, anything else is a leaf `put(kind) ||
/// put(text)`. No kind of tree-sitter-rust 0.24.2 is 40 or 41 bytes long,
/// so a length never reads as `(` or `)`.
pub fn leaf_text_bytes(stream: &[u8]) -> u32 {
    fn length(stream: &[u8], at: &mut usize) -> usize {
        let bytes: [u8; 4] = stream[*at..*at + 4].try_into().expect("four length bytes");
        *at += 4;
        usize::try_from(u32::from_le_bytes(bytes)).expect("a length")
    }
    let mut at = 0;
    let mut depth = 0usize;
    let mut total = 0usize;
    while at < stream.len() {
        match stream[at] {
            b'(' => {
                at += 1;
                let kind = length(stream, &mut at);
                at += kind;
                depth += 1;
            }
            b')' => {
                at += 1;
                depth = depth.checked_sub(1).expect("balanced parentheses");
            }
            _ => {
                let kind = length(stream, &mut at);
                assert!(kind < 40, "a kind length below 40");
                at += kind;
                let text = length(stream, &mut at);
                total += text;
                at += text;
            }
        }
    }
    assert_eq!(at, stream.len(), "the stream decodes exactly");
    assert_eq!(depth, 0, "balanced parentheses");
    u32::try_from(total).expect("a count below 4 GiB")
}

/// L3: `quote` = put(before) || put(head) || put(tail) || put(after):
/// `head`, `tail` the text from the first attached attribute (else the item)
/// to the item's end, split around the name node (no name: `tail` empty);
/// `before`, `after` the nearest line before the line `head` starts on and
/// after the item's last line that is not blank once spaces and tabs are
/// trimmed at both ends (none: empty); the CR rule first.
pub fn quote_stream_by_hand(item: Node, source: &str) -> Vec<u8> {
    let span = span_of(item);
    let (head, tail) = match name_node(item) {
        Some(name) => (
            &source[span.start..name.start_byte()],
            &source[name.end_byte()..span.end],
        ),
        None => (&source[span.clone()], ""),
    };
    let mut out = Vec::new();
    for part in [
        line_before(source, span.start),
        cr_rule(head),
        cr_rule(tail),
        line_after(source, span.end),
    ] {
        put(&mut out, part.as_bytes());
    }
    out
}

/// The CR rule: a CR right before an LF is dropped.
pub fn cr_rule(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn trimmed(line: &str) -> &str {
    let line = line.strip_suffix('\r').unwrap_or(line);
    line.trim_matches(|c| c == ' ' || c == '\t')
}

fn line_before(source: &str, at: usize) -> String {
    let line_start = source[..at].rfind('\n').map_or(0, |i| i + 1);
    if line_start == 0 {
        return String::new();
    }
    source[..line_start - 1]
        .rsplit('\n')
        .map(trimmed)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn line_after(source: &str, end: usize) -> String {
    let Some(newline) = source[end..].find('\n') else {
        return String::new();
    };
    source[end + newline + 1..]
        .split('\n')
        .map(trimmed)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// The leading run of `source_file`'s inner attributes, comments between; a
/// `shebang` does not end it (H2, iteration 2's reading).
pub fn leading_inner_attributes(root: Node) -> Vec<Node> {
    let mut cursor = root.walk();
    let mut run = Vec::new();
    for child in root.children(&mut cursor) {
        if child.kind() == "inner_attribute_item" {
            run.push(child);
        } else if !is_comment(child) && child.kind() != "shebang" {
            break;
        }
    }
    run
}
