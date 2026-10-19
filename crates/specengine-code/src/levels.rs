//! Per-level hashes and fingerprints of a bound item, recipe
//! `specengine-hash/v3` (`crates/specengine-code/README.md` "Hash recipe"),
//! beside v2's whole-item hash ([`crate::hash`]), which stays byte for byte.
//!
//! ```text
//! level_header = put("specengine-hash/v3") ‖ put(tree-sitter-rust version) ‖ put(abi_version())
//! sig          = BLAKE3(level_header ‖ put("sig")  ‖ walk3(attr)... ‖ walk3(item, skip = {name, body}))
//! body         = BLAKE3(level_header ‖ put("body") ‖ walk3(body node)...)
//! deps         = BLAKE3(level_header ‖ put("deps") ‖ put(call target)...)       byte-sorted, deduplicated
//! skeleton     = BLAKE3(level_header ‖ put("skeleton") ‖ masked walk3(attr)... ‖ masked walk3(item, skip = {name}))
//! quote        = BLAKE3(level_header ‖ put("quote") ‖ put(before) ‖ put(head) ‖ put(tail) ‖ put(after))
//! body_bytes   = the text lengths of the leaves walk3(body) emits, summed
//! ```
//!
//! - `walk3` is v2's walk ([`crate::hash::normalize`]: comments by kind, the
//!   anonymous `,`, the transparent wrappers N1-N3 read through v2's own
//!   frames, sorted `use` runs N4) plus **N5**: a run of `mod x;`
//!   declarations, or of `extern crate`s, below the root is emitted in the
//!   byte order of its members' walks, a member being its attached
//!   attributes ‖ itself; a member carrying `#[macro_use]`, or a
//!   `#[cfg_attr(pred, ..)]` with `macro_use` among the attributes after
//!   `pred`, breaks the run and stays in place (macro scope follows textual
//!   order); and **N6**: the named non-comment children of a `use_list`,
//!   when two or more, form a run emitted between the list's `{` and `}` in
//!   the byte order of their own walks, each member's nested lists sorted
//!   first; comments inside the braces stay invisible, and no member is
//!   dropped, merged or rewritten (duplicates kept). Runs of every class
//!   nest at most [`MAX_USE_RUN_NESTING`] deep, else the unit is
//!   `nesting_too_deep`.
//! - **CR rule**: in every stream here and in `body_bytes`, a CR right before
//!   an LF inside a leaf's text (in `quote`, inside the quoted text) is
//!   dropped, so an LF and a CRLF checkout read alike.
//! - **Masked** (`skeleton`): a named leaf is its length-prefixed kind only,
//!   its text dropped; anonymous leaves and structure are kept; runs are
//!   sorted by their masked walks.
//! - The body nodes are [`BODY_FIELDS`]'s; an item without one hashes the
//!   empty `body` and `deps` streams and counts 0 bytes.
//! - A call target ([`call_targets`]): a `call_expression`'s `function` (a
//!   `generic_function`'s own `function`); an `identifier` or a
//!   `scoped_identifier` as its leaf texts, comments and every
//!   `type_arguments` right after a `::` dropped (with that `::`), joined
//!   with one space where two word characters would touch; a
//!   `field_expression` as `.` ‖ its field; any other function (a closure,
//!   a call's result, an index) as its `kind()` only, so a target's text is
//!   bounded and no closure body enters it. A `macro_invocation` is its
//!   `macro` field's texts ‖ `!`. Calls inside a macro's token tree are
//!   invisible.
//! - **Quote**: `head`, `tail` = the text from the first attached attribute
//!   (else the item) to the item's end, split around the name (no name:
//!   `tail` empty); `before`, `after` = the nearest non-blank line before
//!   the line `head` starts on and after the item's last line, spaces and
//!   tabs trimmed (none: empty).
//! - **Broken**: an item with a parse error on itself or an attached
//!   attribute gets no level and no fingerprint: [`LevelState::CannotVerify`]
//!   with v2's categories; the file's module, any parse error in the file.
//! - **File module** ([`module_levels`]): `sig` = the leading run of the
//!   file's inner attributes (a comment or a `shebang` does not end it, the
//!   shebang's leaf staying in `body`); `body` = the file's walk without
//!   that run; `deps` = every call in the file; `skeleton` the masked walk
//!   of the file; `quote` the file's text from its first child's start to
//!   its last child's end, ASCII whitespace trimmed at both ends, no name
//!   and no context lines.
//!
//! One pass over the attributes and the item feeds every stream at once;
//! the walk keeps an explicit stack, runs included, so nothing recurses over
//! the input. Only `kind()` strings enter a stream, never `kind_id()`.

use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::BTreeSet;
use std::sync::OnceLock;

use tree_sitter::{Node, Tree, TreeCursor};

use crate::grammar::{TREE_SITTER_RUST_VERSION, language};
use crate::hash::{
    Digest, ErrorCategory, Frame, HashState, MAX_USE_RUN_NESTING, Sink, attached_attributes,
    attributed_item, error_categories, hash_item, is_anonymous_comma, is_comment, item_has_error,
    name_node, previous_code_sibling, put,
};

/// Name of the level recipe; changes only with the recipe itself, and once
/// a lock records its digests, only together with a rebase of every lock.
///
/// History: sorted `use_list` members (N6) joined `v3` in place, before any
/// lock recorded a level digest: every stream of an input without a
/// `use_list` of two or more members is unchanged.
pub const LEVEL_RECIPE: &str = "specengine-hash/v3";

/// An item whose `body_bytes` is below this is trivial ([`LevelHashes::is_trivial`]).
/// One constant, not configuration.
pub const TRIVIAL_BODY_BYTES: u32 = 124;

/// `LEVEL_RECIPE ‖ tree-sitter-rust version ‖ abi_version()`, each
/// length-prefixed: v2's header ([`crate::hash::recipe_header`]) with v3's
/// recipe name, heading every level and fingerprint stream.
#[must_use]
pub fn level_header() -> &'static [u8] {
    static HEADER: OnceLock<Vec<u8>> = OnceLock::new();
    HEADER.get_or_init(|| {
        let mut header = Vec::new();
        put(&mut header, LEVEL_RECIPE.as_bytes());
        put(&mut header, TREE_SITTER_RUST_VERSION.as_bytes());
        put(&mut header, language().abi_version().to_string().as_bytes());
        header
    })
}

/// The levels and fingerprints of one item or module, or the explicit
/// refusal to give any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelState {
    Hashed(LevelHashes),
    /// A parse error, or runs nested past [`MAX_USE_RUN_NESTING`]
    /// (`nesting_too_deep`): no level and no fingerprint. Sorted, deduplicated;
    /// the first is the detail.
    CannotVerify {
        categories: Vec<ErrorCategory>,
    },
}

/// One unit's levels (`sig`, `body`, `deps`) and fingerprints (`skeleton`,
/// `quote`, `body_bytes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LevelHashes {
    pub sig: Digest,
    pub body: Digest,
    pub deps: Digest,
    pub skeleton: Digest,
    pub quote: Digest,
    /// The text lengths of the leaves the body walk emits, after the CR rule:
    /// independent of formatting, comments and the checkout.
    pub body_bytes: u32,
}

impl LevelHashes {
    /// `body_bytes < TRIVIAL_BODY_BYTES`: the one triviality comparison.
    #[must_use]
    pub fn is_trivial(&self) -> bool {
        self.body_bytes < TRIVIAL_BODY_BYTES
    }
}

/// Where a kind keeps its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodySlot {
    /// None: the item is all signature.
    Absent,
    /// The child in this grammar field, if present.
    Field(&'static str),
    /// Every named child whose kind is in the row's `types`, in source order
    /// (no field holds them).
    Children,
}

/// One body field row (`crates/specengine-code/README.md` "Hash recipe"):
/// where an item kind keeps its body, the node types the grammar allows
/// there (`node-types.json`, supertypes as named there) and whether the
/// grammar requires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyField {
    pub kind: &'static str,
    pub field: BodySlot,
    pub types: &'static [&'static str],
    pub required: bool,
}

impl BodyField {
    /// Whether `node`, a direct child of an item of this row's kind reached
    /// through `field`, is a body node.
    fn holds(&self, node: Node, field: Option<&str>) -> bool {
        match self.field {
            BodySlot::Absent => false,
            BodySlot::Field(name) => field == Some(name),
            BodySlot::Children => node.is_named() && self.types.contains(&node.kind()),
        }
    }
}

/// The body of every kind of `items::ITEM_KINDS` and `items::MEMBER_KINDS`,
/// one row per kind, as tree-sitter-rust 0.24.2 declares it; re-pinned
/// against [`crate::grammar::NODE_TYPES`] with every grammar bump.
pub const BODY_FIELDS: &[BodyField] = &[
    BodyField {
        kind: "function_item",
        field: BodySlot::Field("body"),
        types: &["block"],
        required: true,
    },
    BodyField {
        kind: "struct_item",
        field: BodySlot::Field("body"),
        types: &["field_declaration_list", "ordered_field_declaration_list"],
        required: false,
    },
    BodyField {
        kind: "enum_item",
        field: BodySlot::Field("body"),
        types: &["enum_variant_list"],
        required: true,
    },
    BodyField {
        kind: "union_item",
        field: BodySlot::Field("body"),
        types: &["field_declaration_list"],
        required: true,
    },
    BodyField {
        kind: "trait_item",
        field: BodySlot::Field("body"),
        types: &["declaration_list"],
        required: true,
    },
    BodyField {
        kind: "impl_item",
        field: BodySlot::Field("body"),
        types: &["declaration_list"],
        required: false,
    },
    BodyField {
        kind: "const_item",
        field: BodySlot::Field("value"),
        types: &["_expression"],
        required: false,
    },
    BodyField {
        kind: "static_item",
        field: BodySlot::Field("value"),
        types: &["_expression"],
        required: false,
    },
    BodyField {
        kind: "type_item",
        field: BodySlot::Field("type"),
        types: &["_type"],
        required: true,
    },
    BodyField {
        kind: "mod_item",
        field: BodySlot::Field("body"),
        types: &["declaration_list"],
        required: false,
    },
    BodyField {
        kind: "macro_definition",
        field: BodySlot::Children,
        types: &["macro_rule"],
        required: false,
    },
    BodyField {
        kind: "function_signature_item",
        field: BodySlot::Absent,
        types: &[],
        required: false,
    },
];

/// The [`BODY_FIELDS`] row of `kind`.
fn body_field(kind: &str) -> Option<&'static BodyField> {
    BODY_FIELDS.iter().find(|row| row.kind == kind)
}

/// The body node(s) of `item` by [`BODY_FIELDS`], in source order; empty
/// when it has none (a signature item, a unit struct, `mod x;`, `impl … ;`,
/// `const X: T;`) or its kind has no row.
#[must_use]
pub fn body_nodes(item: Node) -> Vec<Node> {
    let Some(row) = body_field(item.kind()) else {
        return Vec::new();
    };
    match row.field {
        BodySlot::Absent => Vec::new(),
        BodySlot::Field(name) => item.child_by_field_name(name).into_iter().collect(),
        BodySlot::Children => {
            let mut cursor = item.walk();
            item.named_children(&mut cursor)
                .filter(|child| row.holds(*child, None))
                .collect()
        }
    }
}

/// The call targets `deps` hashes, byte-sorted and deduplicated: those
/// inside `item`'s body nodes, or, for the file's root (`source_file`),
/// every call in the file (its module's `deps`).
#[must_use]
pub fn call_targets(item: Node, source: &str) -> Vec<String> {
    let scopes = if item.kind() == "source_file" {
        vec![item]
    } else {
        body_nodes(item)
    };
    let mut targets = BTreeSet::new();
    for scope in scopes {
        collect_calls(scope, source.as_bytes(), &mut targets);
    }
    targets.into_iter().collect()
}

/// The levels of the file's module (a `//!` marker's binding): `CannotVerify`
/// with the file's error categories when any `ERROR` or `MISSING` node is in
/// it, `nesting_too_deep` when its runs nest past the cap.
#[must_use]
pub fn module_levels(tree: &Tree, source: &str) -> LevelState {
    let root = tree.root_node();
    let bytes = source.as_bytes();
    if root.has_error() {
        return LevelState::CannotVerify {
            categories: error_categories(root, bytes),
        };
    }
    let mut out = Out::new(true);
    if !walk3(root, Region::Body, Place::Module, bytes, &mut out) {
        return too_deep();
    }
    let text = module_text(root, bytes);
    LevelState::Hashed(out.finish(quote_digest(b"", text, b"", b"")))
}

/// The file's text for its module's `quote`: from the first child's start
/// to the last child's end, ASCII whitespace trimmed at both ends (a doc
/// comment or a shebang ends with its newline); empty when the file has no
/// child (empty or whitespace only).
fn module_text<'a>(root: Node, source: &'a [u8]) -> &'a [u8] {
    let last = root.child_count().checked_sub(1);
    match (root.child(0), last.and_then(|last| root.child(last))) {
        (Some(first), Some(last)) => source
            .get(first.start_byte()..last.end_byte())
            .unwrap_or(&[])
            .trim_ascii(),
        _ => &[],
    }
}

/// The levels of items of one source text, its lines indexed once, on the
/// first item that needs them ([`crate::bind::item_levels`]).
pub(crate) struct FileLevels<'s> {
    source: &'s str,
    lines: OnceCell<Lines<'s>>,
}

impl<'s> FileLevels<'s> {
    pub(crate) fn new(source: &'s str) -> Self {
        Self {
            source,
            lines: OnceCell::new(),
        }
    }

    /// The levels of `item`, a node of `source`'s tree.
    pub(crate) fn item(&self, item: Node) -> LevelState {
        let bytes = self.source.as_bytes();
        if item_has_error(item) {
            // v2 reads the categories without hashing a broken item.
            let categories = match hash_item(item, bytes) {
                HashState::CannotVerify { categories } => categories,
                HashState::Hashed(_) | HashState::NotHashed => vec![ErrorCategory::Other],
            };
            return LevelState::CannotVerify { categories };
        }
        let attributes = attached_attributes(item);
        let place = Place::Item {
            name: name_node(item).map(|name| name.id()),
            body: body_field(item.kind()),
        };
        let mut out = Out::new(false);
        let complete = attributes
            .iter()
            .all(|attribute| walk3(*attribute, Region::Sig, Place::Plain, bytes, &mut out))
            && walk3(item, Region::Sig, place, bytes, &mut out);
        if !complete {
            return too_deep();
        }
        let lines = self.lines.get_or_init(|| Lines::new(self.source));
        let start = attributes
            .first()
            .map_or(item.start_byte(), Node::start_byte);
        let end = item.end_byte();
        let (head, tail) = match name_node(item) {
            Some(name) => (start..name.start_byte(), name.end_byte()..end),
            None => (start..end, end..end),
        };
        let quote = quote_digest(
            lines.before(start),
            bytes.get(head).unwrap_or(&[]),
            bytes.get(tail).unwrap_or(&[]),
            lines.after(end),
        );
        LevelState::Hashed(out.finish(quote))
    }
}

fn too_deep() -> LevelState {
    LevelState::CannotVerify {
        categories: vec![ErrorCategory::NestingTooDeep],
    }
}

// ------------------------------------------------------------------ streams

/// The CR rule: every CR right before an LF dropped.
pub(crate) fn cr_rule(text: &[u8]) -> Cow<'_, [u8]> {
    if !text.windows(2).any(|pair| pair == b"\r\n") {
        return Cow::Borrowed(text);
    }
    let mut kept = Vec::with_capacity(text.len());
    let mut bytes = text.iter().copied().peekable();
    while let Some(byte) = bytes.next() {
        if byte == b'\r' && bytes.peek() == Some(&b'\n') {
            continue;
        }
        kept.push(byte);
    }
    Cow::Owned(kept)
}

/// Bytes held before they reach the hasher, so many small writes cost one update.
const FLUSH_AT: usize = 16 * 1024;

/// One v3 stream: `level_header() ‖ put(tag)`, then what is written.
pub(crate) struct Stream {
    hasher: blake3::Hasher,
    pending: Vec<u8>,
}

impl Stream {
    pub(crate) fn tagged(tag: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(level_header());
        let mut stream = Self {
            hasher,
            pending: Vec::new(),
        };
        put(&mut stream, tag.as_bytes());
        stream
    }

    pub(crate) fn finish(mut self) -> Digest {
        self.hasher.update(&self.pending);
        Digest(*self.hasher.finalize().as_bytes())
    }
}

impl Sink for Stream {
    fn write(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() >= FLUSH_AT {
            self.hasher.update(&self.pending);
            self.pending.clear();
        }
    }
}

/// `deps` over `targets`, given byte-sorted and deduplicated: no target is
/// the empty set's digest.
pub(crate) fn deps_digest<'a>(targets: impl IntoIterator<Item = &'a str>) -> Digest {
    let mut stream = Stream::tagged("deps");
    for target in targets {
        put(&mut stream, target.as_bytes());
    }
    stream.finish()
}

/// `quote` over its four parts; the CR rule is applied to `head` and `tail`
/// here, `before` and `after` come trimmed ([`Lines`]).
fn quote_digest(before: &[u8], head: &[u8], tail: &[u8], after: &[u8]) -> Digest {
    let mut stream = Stream::tagged("quote");
    put(&mut stream, before);
    put(&mut stream, &cr_rule(head));
    put(&mut stream, &cr_rule(tail));
    put(&mut stream, after);
    stream.finish()
}

/// Which level a node's bytes belong to: `sig` (and the module's leading
/// inner attributes) or `body`. `skeleton` takes both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Sig,
    Body,
}

/// The walk buffers of one run member: its normal and its masked bytes.
#[derive(Default)]
struct Member {
    normal: Vec<u8>,
    skeleton: Vec<u8>,
}

/// Every sink of one unit's pass.
struct Out {
    sig: Stream,
    body: Stream,
    skeleton: Stream,
    body_bytes: u32,
    calls: BTreeSet<String>,
    /// The file's module: every call counts, not only the body's.
    calls_everywhere: bool,
    /// The members of the runs being walked, innermost last: while one is
    /// open, the walk writes there instead of to the streams.
    members: Vec<Member>,
}

impl Out {
    fn new(calls_everywhere: bool) -> Self {
        Self {
            sig: Stream::tagged("sig"),
            body: Stream::tagged("body"),
            skeleton: Stream::tagged("skeleton"),
            body_bytes: 0,
            calls: BTreeSet::new(),
            calls_everywhere,
            members: Vec::new(),
        }
    }

    /// The normal and the masked sink of `region`.
    fn sinks(&mut self, region: Region) -> (&mut dyn Sink, &mut dyn Sink) {
        if let Some(member) = self.members.last_mut() {
            return (&mut member.normal, &mut member.skeleton);
        }
        let normal: &mut dyn Sink = match region {
            Region::Sig => &mut self.sig,
            Region::Body => &mut self.body,
        };
        (normal, &mut self.skeleton)
    }

    /// `(` ‖ kind of an inner node; a call's target when calls count here.
    fn open(&mut self, region: Region, node: Node, source: &[u8]) {
        if (region == Region::Body || self.calls_everywhere)
            && let Some(target) = call_target(node, source)
        {
            self.calls.insert(target);
        }
        let kind = node.kind().as_bytes();
        let (normal, skeleton) = self.sinks(region);
        normal.write(b"(");
        put(normal, kind);
        skeleton.write(b"(");
        put(skeleton, kind);
    }

    fn close(&mut self, region: Region) {
        let (normal, skeleton) = self.sinks(region);
        normal.write(b")");
        skeleton.write(b")");
    }

    /// A leaf: kind ‖ text, masked to its kind when named; its bytes counted in a body.
    fn leaf(&mut self, region: Region, node: Node, source: &[u8]) {
        let text = cr_rule(source.get(node.byte_range()).unwrap_or(&[]));
        if region == Region::Body {
            let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
            self.body_bytes = self.body_bytes.saturating_add(length);
        }
        let kind = node.kind().as_bytes();
        let named = node.is_named();
        let (normal, skeleton) = self.sinks(region);
        put(normal, kind);
        put(normal, &text);
        put(skeleton, kind);
        if !named {
            put(skeleton, &text);
        }
    }

    fn finish(self, quote: Digest) -> LevelHashes {
        LevelHashes {
            sig: self.sig.finish(),
            body: self.body.finish(),
            deps: deps_digest(self.calls.iter().map(String::as_str)),
            skeleton: self.skeleton.finish(),
            quote,
            body_bytes: self.body_bytes,
        }
    }
}

// --------------------------------------------------------------------- walk

/// How the direct children of a walk's root are placed.
#[derive(Clone, Copy)]
enum Place {
    /// An attribute or a run member: everything in the root's region.
    Plain,
    /// An item: its name skipped, its body nodes in `body`.
    Item {
        name: Option<usize>,
        body: Option<&'static BodyField>,
    },
    /// The file: the leading inner attributes in `sig`, the rest in `body`.
    Module,
}

/// An open ancestor: v2's frame and the region of its bytes.
#[derive(Clone, Copy)]
struct Open<'t> {
    frame: Frame<'t>,
    region: Region,
    /// The ancestor itself when it is a `use_list`: its members (named
    /// non-comment children) form a run when two or more (N6).
    list: Option<Node<'t>>,
}

/// One root's walk: v2's iterative pre-order over a cursor that never
/// leaves the root, handing each run it meets back to [`walk3`].
struct Walk<'t> {
    cursor: TreeCursor<'t>,
    root: usize,
    region: Region,
    place: Place,
    /// Still inside the file's leading inner attributes ([`Place::Module`]).
    leading: bool,
    open: Vec<Open<'t>>,
    /// The cursor stands on a run's last declaration or member, the run
    /// walked: move on.
    past_run: bool,
}

impl<'t> Walk<'t> {
    fn new(root: Node<'t>, region: Region, place: Place) -> Self {
        Self {
            cursor: root.walk(),
            root: root.id(),
            region,
            place,
            leading: true,
            open: Vec::new(),
            past_run: false,
        }
    }

    /// The region of `current`, reached through `field` below `parent`;
    /// `None` for the item's name, which no stream reads.
    fn region_of(&mut self, current: Node, field: Option<&str>, parent: Open) -> Option<Region> {
        if self.open.len() > 1 {
            return Some(parent.region);
        }
        match self.place {
            Place::Plain => Some(parent.region),
            Place::Item { name, body } => {
                if name == Some(current.id()) {
                    None
                } else if body.is_some_and(|row| row.holds(current, field)) {
                    Some(Region::Body)
                } else {
                    Some(parent.region)
                }
            }
            Place::Module => {
                if self.leading {
                    if current.kind() == "inner_attribute_item" {
                        return Some(Region::Sig);
                    }
                    // A comment or the shebang (its leaf in `body`) keeps the run open.
                    if !is_comment(current) && current.kind() != "shebang" {
                        self.leading = false;
                    }
                }
                Some(parent.region)
            }
        }
    }

    /// Moves the cursor along its siblings to the node `id` (a run's last
    /// declaration or member), or to the last sibling.
    fn move_to_sibling(&mut self, id: usize) {
        while self.cursor.node().id() != id && self.cursor.goto_next_sibling() {}
    }

    /// Walks on until the root is done (`None`) or a run is met (`Some`):
    /// the caller walks the run's members, then resumes here past the run.
    fn resume(&mut self, source: &[u8], out: &mut Out) -> Option<RunWalk<'t>> {
        loop {
            if !std::mem::take(&mut self.past_run) {
                let current = self.cursor.node();
                let field = self.cursor.field_name();
                let parent = self.open.last().copied();
                let region = match parent {
                    None => Some(self.region),
                    Some(parent) => self.region_of(current, field, parent),
                };
                if let Some(region) = region {
                    if is_comment(current) || is_anonymous_comma(current) {
                        // Contributes nothing.
                    } else if !current.is_named()
                        && parent.is_some_and(|open| open.frame.drops(current.kind()))
                    {
                        // `{` `}` of a transparent block or token tree, `;` of a transparent statement.
                    } else if let Some(run) = parent.and_then(|open| match open.list {
                        Some(list) => list_run(list, current),
                        None => run_at(current, source),
                    }) {
                        self.move_to_sibling(run.last);
                        self.past_run = true;
                        return Some(RunWalk::new(run, region));
                    } else if let Some(open) =
                        parent.filter(|open| open.frame.transparent_child(current, field))
                    {
                        if self.cursor.goto_first_child() {
                            // A transparent wrapper is never a `use_list`.
                            self.open.push(Open {
                                frame: open.frame.child(current, field, true),
                                region,
                                list: None,
                            });
                            continue;
                        }
                    } else if current.child_count() == 0 {
                        out.leaf(region, current, source);
                    } else {
                        out.open(region, current, source);
                        if self.cursor.goto_first_child() {
                            let frame = match parent {
                                Some(open) => open.frame.child(current, field, false),
                                None => Frame::root(current.kind()),
                            };
                            self.open.push(Open {
                                frame,
                                region,
                                list: (current.kind() == "use_list").then_some(current),
                            });
                            continue;
                        }
                        out.close(region);
                    }
                }
            }
            if self.cursor.node().id() == self.root {
                return None;
            }
            loop {
                if self.cursor.goto_next_sibling() {
                    break;
                }
                if !self.cursor.goto_parent() {
                    return None;
                }
                match self.open.pop() {
                    Some(open) if open.frame.transparent => {}
                    Some(open) => out.close(open.region),
                    None => out.close(self.region),
                }
                if self.cursor.node().id() == self.root {
                    return None;
                }
            }
        }
    }
}

/// The kinds whose sibling runs are sorted: `use` (N4), `mod x;` and
/// `extern crate` (N5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunClass {
    Use,
    Mod,
    ExternCrate,
}

fn run_class(node: Node) -> Option<RunClass> {
    match node.kind() {
        "use_declaration" => Some(RunClass::Use),
        "mod_item" if node.child_by_field_name("body").is_none() => Some(RunClass::Mod),
        "extern_crate_declaration" => Some(RunClass::ExternCrate),
        _ => None,
    }
}

/// Whether a member with these attached attributes may join a run: always
/// for `use` (v2's N4), unless it carries `macro_use` ([`is_macro_use`]) for N5.
fn joins_runs(class: RunClass, attributes: &[Node], source: &[u8]) -> bool {
    class == RunClass::Use
        || !attributes
            .iter()
            .any(|attribute| is_macro_use(*attribute, source))
}

/// `#[macro_use]`, `#[macro_use(..)]`, or `#[cfg_attr(pred, ..)]` with
/// `macro_use` or `macro_use(..)` among the attributes after `pred`, read
/// structurally: the attribute's path, then the top-level `,`-separated
/// groups of its arguments' token tree. A `cfg_attr` nested in a
/// `cfg_attr` is not read.
fn is_macro_use(attribute_item: Node, source: &[u8]) -> bool {
    let Some(attribute) =
        first_code_child(attribute_item).filter(|attribute| attribute.kind() == "attribute")
    else {
        return false;
    };
    let Some(path) = first_code_child(attribute).filter(|path| path.kind() == "identifier") else {
        return false;
    };
    match source.get(path.byte_range()) {
        Some(b"macro_use") => true,
        Some(b"cfg_attr") => attribute
            .child_by_field_name("arguments")
            .is_some_and(|arguments| cfg_attr_carries_macro_use(arguments, source)),
        _ => false,
    }
}

/// Whether a `cfg_attr`'s arguments (`(pred, attr, ..)`, a token tree) hold
/// `macro_use` or `macro_use(..)` as an attribute after the first group.
fn cfg_attr_carries_macro_use(arguments: Node, source: &[u8]) -> bool {
    let text = |node: Node| source.get(node.byte_range()).unwrap_or(&[]);
    // `macro_use` alone, or followed by one token tree.
    let names_macro_use = |group: &[Node]| match group {
        [path, rest @ ..] if path.kind() == "identifier" && text(*path) == b"macro_use" => {
            match rest {
                [] => true,
                [list] => list.kind() == "token_tree",
                _ => false,
            }
        }
        _ => false,
    };
    // The current group's code tokens, at most three: enough to tell.
    let mut group: Vec<Node> = Vec::new();
    let mut after_predicate = false;
    let mut cursor = arguments.walk();
    for child in arguments.children(&mut cursor) {
        if is_comment(child) {
            continue;
        }
        // Nested delimiters open a `token_tree` child: an anonymous one is this tree's own.
        match (!child.is_named()).then(|| text(child)) {
            Some(b",") => {
                if after_predicate && names_macro_use(&group) {
                    return true;
                }
                after_predicate = true;
                group.clear();
            }
            Some(b"(" | b"[" | b"{") => group.clear(),
            Some(b")" | b"]" | b"}") => return after_predicate && names_macro_use(&group),
            _ if group.len() < 3 => group.push(child),
            _ => {}
        }
    }
    false
}

fn first_code_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| !is_comment(*child))
}

/// A run to sort: each member's nodes (attributes, then the declaration;
/// a `use_list`'s member, its clause alone) and the last member's last
/// node, after which the walk goes on.
struct Run<'t> {
    members: Vec<Vec<Node<'t>>>,
    last: usize,
}

/// The run whose first member's first node is `node` (a declaration of a
/// run class without attributes, or the first attribute attached to one),
/// when it has at least two members. As v2's `use` runs: comments between
/// allowed, any other node ends the run; a member that may not join
/// (`#[macro_use]`) ends it too and is walked in place.
fn run_at<'t>(node: Node<'t>, source: &[u8]) -> Option<Run<'t>> {
    if node.kind() != "attribute_item" && run_class(node).is_none() {
        return None;
    }
    // A preceding attribute means `node` is not its member's first node.
    let previous = previous_code_sibling(node);
    if previous.is_some_and(|previous| previous.kind() == "attribute_item") {
        return None;
    }
    let declaration = if node.kind() == "attribute_item" {
        attributed_item(node)?
    } else {
        node
    };
    let class = run_class(declaration)?;
    let attributes = attached_attributes(declaration);
    if !joins_runs(class, &attributes, source) {
        return None;
    }
    // A preceding member that may join means the run opened there.
    if previous.is_some_and(|previous| {
        run_class(previous) == Some(class)
            && joins_runs(class, &attached_attributes(previous), source)
    }) {
        return None;
    }
    let mut members = vec![member(attributes, declaration)];
    let mut last = declaration;
    let mut pending: Vec<Node> = Vec::new();
    let mut next = declaration.next_sibling();
    while let Some(candidate) = next {
        if candidate.kind() == "attribute_item" {
            pending.push(candidate);
        } else if is_comment(candidate) {
            // Comments between members are allowed.
        } else if run_class(candidate) == Some(class) {
            let attributes = std::mem::take(&mut pending);
            if !joins_runs(class, &attributes, source) {
                break;
            }
            members.push(member(attributes, candidate));
            last = candidate;
        } else {
            break;
        }
        next = candidate.next_sibling();
    }
    (members.len() > 1).then(|| Run {
        members,
        last: last.id(),
    })
}

fn member<'t>(mut attributes: Vec<Node<'t>>, declaration: Node<'t>) -> Vec<Node<'t>> {
    attributes.push(declaration);
    attributes
}

/// N6: the run of `list`, a `use_list`, met at its child `first`, its first
/// member (a later one is only ever reached inside the run): the list's
/// named children from `first` on, comments skipped, when two or more; its
/// `{`, `,` and `}` are anonymous. A list with fewer than two named children
/// (a comment is named) is told apart in O(1); any other is read once,
/// through a fresh cursor on the list: never a copy of the walk's cursor,
/// whose stack is as deep as the input, and no node asks for its parent or
/// its siblings.
fn list_run<'t>(list: Node<'t>, first: Node<'t>) -> Option<Run<'t>> {
    if !first.is_named() || is_comment(first) || list.named_child_count() < 2 {
        return None;
    }
    let mut members: Vec<Vec<Node<'t>>> = Vec::new();
    let mut last = first;
    let mut cursor = list.walk();
    let mut more = cursor.goto_first_child();
    while more {
        let candidate = cursor.node();
        if candidate.id() == first.id() {
            members.push(vec![first]);
        } else if !members.is_empty() && candidate.is_named() && !is_comment(candidate) {
            members.push(vec![candidate]);
            last = candidate;
        }
        more = cursor.goto_next_sibling();
    }
    (members.len() > 1).then(|| Run {
        members,
        last: last.id(),
    })
}

/// A run being walked: each member's nodes walked as roots into its own
/// buffers, then the buffers sorted, normal and masked apart.
struct RunWalk<'t> {
    members: std::vec::IntoIter<Vec<Node<'t>>>,
    current: Option<std::vec::IntoIter<Node<'t>>>,
    region: Region,
    normal: Vec<Vec<u8>>,
    skeleton: Vec<Vec<u8>>,
}

impl<'t> RunWalk<'t> {
    fn new(run: Run<'t>, region: Region) -> Self {
        Self {
            members: run.members.into_iter(),
            current: None,
            region,
            normal: Vec::new(),
            skeleton: Vec::new(),
        }
    }

    /// The next node to walk; opens and closes the members' buffers in `out`.
    fn next_node(&mut self, out: &mut Out) -> Option<Node<'t>> {
        loop {
            if let Some(nodes) = &mut self.current {
                if let Some(node) = nodes.next() {
                    return Some(node);
                }
                let done = out.members.pop().unwrap_or_default();
                self.normal.push(done.normal);
                self.skeleton.push(done.skeleton);
                self.current = None;
            }
            let member = self.members.next()?;
            out.members.push(Member::default());
            self.current = Some(member.into_iter());
        }
    }

    /// The members' walks in byte order, into the enclosing sinks.
    fn flush(mut self, out: &mut Out) {
        self.normal.sort_unstable();
        self.skeleton.sort_unstable();
        let (normal, skeleton) = out.sinks(self.region);
        for walk in &self.normal {
            normal.write(walk);
        }
        for walk in &self.skeleton {
            skeleton.write(walk);
        }
    }
}

enum Task<'t> {
    Walk(Walk<'t>),
    Run(RunWalk<'t>),
}

/// Walks `root` (its region `region`, its children placed by `place`) into
/// `out`. `false` when runs nest past [`MAX_USE_RUN_NESTING`]: the streams
/// are then incomplete and must not be hashed. An explicit stack of walks
/// and runs, so the input's depth never bounds the thread stack.
fn walk3<'t>(root: Node<'t>, region: Region, place: Place, source: &[u8], out: &mut Out) -> bool {
    let mut tasks = vec![Task::Walk(Walk::new(root, region, place))];
    let mut runs = 0;
    while let Some(task) = tasks.last_mut() {
        match task {
            Task::Walk(walk) => match walk.resume(source, out) {
                Some(run) => {
                    if runs >= MAX_USE_RUN_NESTING {
                        return false;
                    }
                    runs += 1;
                    tasks.push(Task::Run(run));
                }
                None => {
                    tasks.pop();
                }
            },
            Task::Run(run) => match run.next_node(out) {
                Some(node) => {
                    let region = run.region;
                    tasks.push(Task::Walk(Walk::new(node, region, Place::Plain)));
                }
                None => {
                    if let Some(Task::Run(run)) = tasks.pop() {
                        run.flush(out);
                    }
                    runs -= 1;
                }
            },
        }
    }
    true
}

// -------------------------------------------------------------------- calls

/// Every call target inside `scope`, comments skipped; an explicit-stack walk.
fn collect_calls(scope: Node, source: &[u8], out: &mut BTreeSet<String>) {
    let root = scope.id();
    let mut cursor = scope.walk();
    loop {
        let node = cursor.node();
        let enter = !is_comment(node);
        if enter && let Some(target) = call_target(node, source) {
            out.insert(target);
        }
        if enter && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.node().id() == root {
                return;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// The target of a call or a macro invocation; `None` for any other node.
/// A call's function that is no path and no field (a closure, a call's
/// result, an index) is its `kind()` only, so the text stays bounded and
/// no closure body enters `deps`; the calls inside it count on their own.
fn call_target(node: Node, source: &[u8]) -> Option<String> {
    let target = match node.kind() {
        "call_expression" => {
            let mut function = node.child_by_field_name("function")?;
            if function.kind() == "generic_function" {
                function = function.child_by_field_name("function")?;
            }
            match function.kind() {
                "identifier" | "scoped_identifier" => joined_leaves(function, source),
                "field_expression" => format!(
                    ".{}",
                    joined_leaves(function.child_by_field_name("field")?, source)
                ),
                other => other.to_owned(),
            }
        }
        "macro_invocation" => {
            let mut text = joined_leaves(node.child_by_field_name("macro")?, source);
            text.push('!');
            text
        }
        _ => return None,
    };
    (!target.is_empty()).then_some(target)
}

/// A word character: a letter, a digit or `_`.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// The leaf texts of `scope` in order, after the CR rule: comments dropped,
/// every `type_arguments` right after a `::` dropped with that `::`
/// (`Vec::<u8>::new` -> `Vec::new`), one space between two leaves where
/// word characters would touch (`<T as Trait>::f`).
fn joined_leaves(scope: Node, source: &[u8]) -> String {
    let root = scope.id();
    let mut text = String::new();
    // Where the text stood before the last leaf, when that leaf was `::`.
    let mut before_colons: Option<usize> = None;
    let mut cursor = scope.walk();
    loop {
        let node = cursor.node();
        let mut enter = false;
        if is_comment(node) {
            // Dropped.
        } else if node.kind() == "type_arguments"
            && previous_code_sibling(node)
                .is_some_and(|previous| !previous.is_named() && previous.kind() == "::")
        {
            if let Some(at) = before_colons.take() {
                text.truncate(at);
            }
        } else if node.child_count() == 0 {
            let bytes = cr_rule(source.get(node.byte_range()).unwrap_or(&[]));
            let piece = String::from_utf8_lossy(&bytes);
            if !piece.is_empty() {
                let at = text.len();
                let touching = text.chars().next_back().is_some_and(is_word)
                    && piece.chars().next().is_some_and(is_word);
                if touching {
                    text.push(' ');
                }
                text.push_str(&piece);
                before_colons = (piece == "::").then_some(at);
            }
        } else {
            enter = true;
        }
        if enter && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.node().id() == root {
                return text;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return text;
            }
        }
    }
}

// -------------------------------------------------------------------- lines

const NO_LINE: usize = usize::MAX;

/// The lines of a source for `quote`'s context: where each starts, and per
/// line the nearest non-blank line at or before it and at or after it, so
/// each item's context costs a binary search however many items share a
/// line or a blank stretch.
struct Lines<'s> {
    source: &'s [u8],
    starts: Vec<usize>,
    text_at_or_before: Vec<usize>,
    text_at_or_after: Vec<usize>,
}

impl<'s> Lines<'s> {
    fn new(source: &'s str) -> Self {
        let source = source.as_bytes();
        let mut starts = vec![0];
        starts.extend(
            source
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'\n')
                .map(|(at, _)| at + 1),
        );
        let mut lines = Self {
            source,
            starts,
            text_at_or_before: Vec::new(),
            text_at_or_after: Vec::new(),
        };
        let count = lines.starts.len();
        let blank: Vec<bool> = (0..count)
            .map(|line| lines.trimmed(line).is_empty())
            .collect();
        let mut nearest = NO_LINE;
        lines.text_at_or_before = blank
            .iter()
            .enumerate()
            .map(|(line, blank)| {
                if !blank {
                    nearest = line;
                }
                nearest
            })
            .collect();
        let mut after = vec![NO_LINE; count];
        nearest = NO_LINE;
        for (line, blank) in blank.iter().enumerate().rev() {
            if !blank {
                nearest = line;
            }
            after[line] = nearest;
        }
        lines.text_at_or_after = after;
        lines
    }

    /// Line `line` without its LF and one trailing CR, spaces and tabs
    /// trimmed at both ends; empty means blank.
    fn trimmed(&self, line: usize) -> &'s [u8] {
        let Some(&start) = self.starts.get(line) else {
            return b"";
        };
        let end = self
            .starts
            .get(line + 1)
            .map_or(self.source.len(), |next| next - 1);
        let mut text = self.source.get(start..end).unwrap_or(&[]);
        if let Some(rest) = text.strip_suffix(b"\r") {
            text = rest;
        }
        while let [b' ' | b'\t', rest @ ..] = text {
            text = rest;
        }
        while let [rest @ .., b' ' | b'\t'] = text {
            text = rest;
        }
        text
    }

    /// The line holding byte `offset`.
    fn row(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    }

    fn text_of(&self, line: Option<&usize>) -> &'s [u8] {
        match line {
            Some(&line) if line != NO_LINE => self.trimmed(line),
            _ => b"",
        }
    }

    /// The nearest non-blank line before the line holding `offset`, trimmed.
    fn before(&self, offset: usize) -> &'s [u8] {
        let row = self.row(offset);
        self.text_of(
            row.checked_sub(1)
                .and_then(|line| self.text_at_or_before.get(line)),
        )
    }

    /// The nearest non-blank line after the line holding `offset`, trimmed.
    fn after(&self, offset: usize) -> &'s [u8] {
        self.text_of(self.text_at_or_after.get(self.row(offset) + 1))
    }
}
