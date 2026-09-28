//! The canonical AST hash of spec 05 §5.2.
//!
//! ```text
//! hash(item) = BLAKE3( recipe_header ‖ walk(attrs(item)) ‖ walk(item, skip = name_node(item)) )
//! recipe_header = "specengine-hash/v2" ‖ tree-sitter-rust version ‖ abi_version()
//! walk(n) = if n.kind() ∈ {line_comment, block_comment} and !n.is_error() → ""
//!           else if n is anonymous ","                                   → ""
//!           else if n is a transparent body wrapper (below)              → walk(its single child)
//!           else if n is leaf → len‖kind(n) ‖ len‖text(n)
//!           else              → "(" ‖ len‖kind(n) ‖ concat(walk(c) for c in children) ‖ ")"
//! ```
//!
//! Every string is prefixed by its length as a little-endian `u32`, so no two
//! walks can collide by token gluing. Only `kind()` strings enter the stream —
//! never `kind_id()`, which is an index into generated tables — and never
//! `to_sexp()`, which drops anonymous nodes and leaf text.
//!
//! **Transparent wrappers and sorted imports (v2).** Measured on the pilots,
//! v1 flickered on ≈ 15 % of items under a contrasting `max_width`, all of it
//! rustfmt moving braces and semicolons that carry no meaning, plus its import
//! reordering. v2 closes exactly those, each rule semantics-preserving:
//!
//! - **`expression_statement`** — anywhere — whose only named child is an
//!   expression and whose `;` is absent (tree-sitter's shape for a tail that
//!   ends with a block: `if`, `match`, `unsafe { }`, a nested block) or follows
//!   a diverging `return`/`break`/`continue` (style edition 2024 adds that `;`:
//!   `else { continue }` → `else { continue; }`): the walk sees only the
//!   expression. A `;` after any other expression changes the value and stays.
//! - **`block`** in a body position — the `body` of a `closure_expression`,
//!   the `value` of a `match_arm`, or the child of a transparent wrapper —
//!   without a label, whose only child besides `{`, `}` and comments is one
//!   expression (a subtype of the grammar's `_expression`) or one transparent
//!   `expression_statement`: rustfmt writes `|x| { x + 1 }` ↔ `|x| x + 1`,
//!   `P => { f() }` ↔ `P => f(),` (`match_arm_blocks`), `P => return x,` ↔
//!   `P => { return x; }`. `{ { x } }` unwraps recursively. A labelled block
//!   is a `break 'a` target, a `let` changes the value, `unsafe`/`async`/
//!   `const` blocks are other node kinds, and a block anywhere else (an `if`
//!   branch, a function body) is never removed by rustfmt — all keep their
//!   wrapper.
//! - **`{ … }` `token_tree`** inside the arguments of a `(`- or `[`-delimited
//!   `macro_invocation` whose previous token is `|`, `||` or `=>`: the same
//!   closure body / match arm, seen by rustfmt after parsing the arguments of
//!   `assert!`, `vec!`, `format!` … as expressions. The walk keeps the tokens
//!   and drops the braces. Brace-delimited invocations (`quote! { }`,
//!   `thread_local! { }`, DSLs) are never formatted by rustfmt and are left
//!   alone; a macro definition contains no `macro_invocation` frame.
//! - **Runs of `use_declaration` siblings** (comments between allowed) below
//!   the root are emitted in the byte order of their walks (`reorder_imports`);
//!   import order never affects name resolution. A member of the run is the
//!   declaration together with its attached outer attributes — the
//!   `attribute_item` siblings before it, comments between allowed, exactly as
//!   `attached_attributes` reads them for any item — and its walk is
//!   `walk(attrs) ‖ walk(use)`: `#[cfg(feature = "x")] use c;` moves as one
//!   unit, so gating a different import is a different hash, and rustfmt's
//!   `#[cfg] use c; use b;` → `use b; #[cfg] use c;` is stable. Any other
//!   item, a statement or an inner attribute breaks the run; a run of one
//!   member is walked as it stands.
//!
//! A transparent wrapper contributes no `(`‖kind, no delimiters and no `)` —
//! only the walk of its children.
//!
//! Traps (05 §5.2): comments are recognised by `kind()`, not by `is_extra()`
//! (true for `ERROR` nodes too); an item whose subtree `has_error()` is
//! reported as `cannot_verify` and is not hashed at all.
//!
//! **Depth.** The walk is iterative except for use runs: each member is walked
//! into its own buffer by a nested call, and a member's attribute value may
//! hold a block with another run (`#[a = { use x; use y; }] use c; use d;`).
//! Past [`MAX_USE_RUN_NESTING`] such nested runs the item is `cannot_verify`
//! (`nesting_too_deep`), never hashed: the stack and the copying of nested
//! walks stay bounded whatever the file. Real code nests none.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::OnceLock;

use tree_sitter::Node;

use crate::grammar::{TREE_SITTER_RUST_VERSION, language};

/// Name of the recipe; changes only with the recipe itself.
///
/// History: `v1` — the plain walk; `v2` — transparent body wrappers (closure
/// bodies and match-arm values) and sorted `use` runs, a deliberate rebase of
/// every hash. Attributes attached to a `use` joined its run member within
/// `v2`, before any digest was recorded: the stream of every input without
/// attributes on a `use` is unchanged.
pub const RECIPE: &str = "specengine-hash/v2";

/// Use runs nested one inside another (through the attribute values of run
/// members) that the walk follows; each is one level of recursion. A run met
/// deeper makes the walk incomplete and the item `cannot_verify`.
pub const MAX_USE_RUN_NESTING: usize = 64;

/// Consumer of the normalized byte stream: the hasher in production, a buffer in tests.
pub trait Sink {
    fn write(&mut self, bytes: &[u8]);
}

impl Sink for Vec<u8> {
    fn write(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl Sink for blake3::Hasher {
    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}

/// Length-prefixed string: `len as u32 LE ‖ bytes`.
fn put(sink: &mut impl Sink, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    sink.write(&len.to_le_bytes());
    sink.write(bytes);
}

/// `RECIPE ‖ tree-sitter-rust version ‖ abi_version()`, each length-prefixed.
pub fn recipe_header() -> &'static [u8] {
    static HEADER: OnceLock<Vec<u8>> = OnceLock::new();
    HEADER.get_or_init(|| {
        let mut header = Vec::new();
        put(&mut header, RECIPE.as_bytes());
        put(&mut header, TREE_SITTER_RUST_VERSION.as_bytes());
        put(&mut header, language().abi_version().to_string().as_bytes());
        header
    })
}

/// A comment node, recognised by its public kind — never by `is_extra()`.
#[must_use]
pub fn is_comment(node: Node) -> bool {
    matches!(node.kind(), "line_comment" | "block_comment") && !node.is_error()
}

/// An anonymous `,`: rustfmt adds and removes trailing commas, so they never count.
#[must_use]
pub fn is_anonymous_comma(node: Node) -> bool {
    !node.is_named() && node.kind() == ","
}

/// Public kinds of every subtype of the grammar's `_expression` supertype,
/// nested supertypes (`_literal`) expanded. Read once from the pinned grammar
/// instead of a hand-written list, so the set cannot drift from it; only the
/// names are kept, never the ids.
fn expression_kinds() -> &'static BTreeSet<String> {
    static KINDS: OnceLock<BTreeSet<String>> = OnceLock::new();
    KINDS.get_or_init(|| {
        let language = language();
        let supertypes = language.supertypes();
        let mut kinds = BTreeSet::new();
        let mut pending: Vec<u16> = supertypes
            .iter()
            .copied()
            .filter(|&id| {
                matches!(
                    language.node_kind_for_id(id),
                    Some("_expression" | "expression")
                )
            })
            .collect();
        let mut seen = BTreeSet::new();
        while let Some(supertype) = pending.pop() {
            if !seen.insert(supertype) {
                continue;
            }
            for &subtype in language.subtypes_for_supertype(supertype) {
                if supertypes.contains(&subtype) {
                    pending.push(subtype);
                } else if let Some(kind) = language.node_kind_for_id(subtype) {
                    kinds.insert(kind.to_owned());
                }
            }
        }
        kinds
    })
}

/// `{ expr }`: no label, and exactly one child besides the braces and comments,
/// which is an expression or a transparent `expression_statement`.
#[must_use]
pub fn is_tail_only_block(block: Node) -> bool {
    if block.child_by_field_name("label").is_some() {
        return false;
    }
    let mut cursor = block.walk();
    let mut tail: Option<Node> = None;
    for child in block.children(&mut cursor) {
        if is_comment(child) {
            continue;
        }
        if !child.is_named() {
            if matches!(child.kind(), "{" | "}") {
                continue;
            }
            return false;
        }
        if tail.replace(child).is_some() {
            return false;
        }
    }
    tail.is_some_and(|node| {
        !node.is_error()
            && (expression_kinds().contains(node.kind()) || is_transparent_statement(node))
    })
}

/// An `expression_statement` whose `;` (if any) carries no meaning: none at
/// all (the tail ends with a block), or one after `return`/`break`/`continue`.
#[must_use]
pub fn is_transparent_statement(statement: Node) -> bool {
    let mut cursor = statement.walk();
    let mut expression: Option<Node> = None;
    let mut semicolon = false;
    for child in statement.children(&mut cursor) {
        if is_comment(child) {
            continue;
        }
        if !child.is_named() {
            if child.kind() == ";" {
                semicolon = true;
                continue;
            }
            return false;
        }
        if expression.replace(child).is_some() {
            return false;
        }
    }
    expression.is_some_and(|node| {
        !node.is_error()
            && expression_kinds().contains(node.kind())
            && (!semicolon
                || matches!(
                    node.kind(),
                    "return_expression" | "break_expression" | "continue_expression"
                ))
    })
}

/// The previous sibling of `node` that is not a comment.
fn previous_code_sibling(node: Node) -> Option<Node> {
    let mut previous = node.prev_sibling();
    while let Some(candidate) = previous {
        if !is_comment(candidate) {
            return Some(candidate);
        }
        previous = candidate.prev_sibling();
    }
    None
}

/// A `{ … }` token tree whose previous token opens a closure body or a match
/// arm — meaningful only inside the arguments of an expression macro.
#[must_use]
pub fn is_body_token_tree(node: Node) -> bool {
    node.kind() == "token_tree"
        && node.child(0).is_some_and(|open| open.kind() == "{")
        && previous_code_sibling(node).is_some_and(|previous| {
            !previous.is_named() && matches!(previous.kind(), "|" | "||" | "=>")
        })
}

/// The item an `attribute_item` is attached to: the next sibling past
/// attributes and comments.
fn attributed_item(attribute: Node) -> Option<Node> {
    let mut next = attribute.next_sibling();
    while let Some(candidate) = next {
        if candidate.kind() != "attribute_item" && !is_comment(candidate) {
            return Some(candidate);
        }
        next = candidate.next_sibling();
    }
    None
}

/// One member of a run of `use` declarations: the declaration with its
/// attached outer attributes, which move with it (`#[cfg(feature = "x")] use b;`).
struct UseMember<'tree> {
    attributes: Vec<Node<'tree>>,
    declaration: Node<'tree>,
}

/// The run of `use` declarations — each with its attached attributes, comments
/// between allowed — whose first node is `node`: a `use_declaration` without
/// attributes, or the first `attribute_item` attached to one. `None` unless
/// `node` opens the run's first member and the run has at least two members.
fn use_run(node: Node) -> Option<Vec<UseMember>> {
    // A preceding attribute means `node` is not the member's first node (its
    // run, if any, opened there); a preceding `use` means an earlier member.
    if !matches!(node.kind(), "use_declaration" | "attribute_item")
        || previous_code_sibling(node)
            .is_some_and(|previous| matches!(previous.kind(), "use_declaration" | "attribute_item"))
    {
        return None;
    }
    let first = match node.kind() {
        "use_declaration" => UseMember {
            attributes: Vec::new(),
            declaration: node,
        },
        _ => {
            let declaration =
                attributed_item(node).filter(|item| item.kind() == "use_declaration")?;
            UseMember {
                attributes: attached_attributes(declaration),
                declaration,
            }
        }
    };
    let mut next = first.declaration.next_sibling();
    let mut run = vec![first];
    let mut pending: Vec<Node> = Vec::new();
    while let Some(candidate) = next {
        match candidate.kind() {
            "use_declaration" => run.push(UseMember {
                attributes: std::mem::take(&mut pending),
                declaration: candidate,
            }),
            "attribute_item" => pending.push(candidate),
            _ if is_comment(candidate) => {}
            // Any other item or statement ends the run; pending attributes
            // belong to it and are walked in the ordinary way.
            _ => break,
        }
        next = candidate.next_sibling();
    }
    (run.len() > 1).then_some(run)
}

/// One ancestor of the node under the cursor, kept while the cursor is below it.
#[derive(Clone, Copy)]
struct Frame<'tree> {
    kind: &'tree str,
    /// The ancestor is a transparent wrapper: no `(`‖kind was emitted for it,
    /// its delimiters are dropped and no `)` closes it.
    transparent: bool,
    /// The ancestor is a transparent wrapper standing in a body position, so
    /// its children stand there too (`{ { x } }` as a match-arm value).
    body: bool,
    /// Inside the arguments of a `(`- or `[`-delimited `macro_invocation`.
    expression_macro: bool,
}

impl<'tree> Frame<'tree> {
    /// The frame of `node`, a child of this frame reached through `field`.
    fn child(self, node: Node<'tree>, field: Option<&str>, transparent: bool) -> Self {
        let expression_macro = if self.kind == "macro_invocation" && node.kind() == "token_tree" {
            node.child(0)
                .is_some_and(|open| matches!(open.kind(), "(" | "["))
        } else {
            self.expression_macro
        };
        Self {
            kind: node.kind(),
            transparent,
            body: transparent && self.body_position(field),
            expression_macro,
        }
    }

    /// Whether a child reached through `field` stands in a body position.
    fn body_position(self, field: Option<&str>) -> bool {
        self.body
            || matches!(
                (self.kind, field),
                ("closure_expression", Some("body")) | ("match_arm", Some("value"))
            )
    }

    /// Whether `node`, a child reached through `field`, is a transparent wrapper.
    fn transparent_child(self, node: Node<'tree>, field: Option<&str>) -> bool {
        match node.kind() {
            "expression_statement" => is_transparent_statement(node),
            "block" => self.body_position(field) && is_tail_only_block(node),
            "token_tree" => self.expression_macro && is_body_token_tree(node),
            _ => false,
        }
    }

    /// Whether an anonymous child of this (transparent) frame is a dropped delimiter.
    fn drops(self, punctuation: &str) -> bool {
        self.transparent
            && match self.kind {
                "block" | "token_tree" => matches!(punctuation, "{" | "}"),
                "expression_statement" => punctuation == ";",
                _ => false,
            }
    }
}

/// Writes the normalized walk of `node` into `sink`. `skip` (the item's own
/// name node) and every comment or anonymous `,` contribute nothing; a
/// transparent wrapper contributes only its children; a run of `use`
/// declarations is emitted sorted (module docs).
///
/// Iterative pre-order over a `TreeCursor`; the cursor never leaves `node`.
/// Returns `false` when a use run nested past [`MAX_USE_RUN_NESTING`] stopped
/// the walk: the stream in `sink` is then incomplete and must not be hashed.
#[must_use = "`false` means the stream in `sink` is incomplete and must not be hashed"]
pub fn normalize(node: Node, source: &[u8], skip: Option<Node>, sink: &mut impl Sink) -> bool {
    normalize_nested(node, source, skip, sink, 0)
}

/// [`normalize`] at `nesting` use runs deep.
fn normalize_nested(
    node: Node,
    source: &[u8],
    skip: Option<Node>,
    sink: &mut impl Sink,
    nesting: usize,
) -> bool {
    let root = node.id();
    let skip = skip.map(|n| n.id());
    let mut cursor = node.walk();
    let mut frames: Vec<Frame<'_>> = Vec::new();
    loop {
        let current = cursor.node();
        let parent = frames.last().copied();
        let mut descended = false;
        if skip == Some(current.id()) || is_comment(current) || is_anonymous_comma(current) {
            // Contributes nothing.
        } else if !current.is_named() && parent.is_some_and(|frame| frame.drops(current.kind())) {
            // `{` `}` of a transparent block or token tree, `;` of a transparent statement.
        } else if let Some(run) = parent.and_then(|_| use_run(current)) {
            if nesting >= MAX_USE_RUN_NESTING {
                return false;
            }
            let mut walks: Vec<Vec<u8>> = Vec::with_capacity(run.len());
            for member in &run {
                let mut walk = Vec::new();
                for node in member.attributes.iter().chain([&member.declaration]) {
                    if !normalize_nested(*node, source, None, &mut walk, nesting + 1) {
                        return false;
                    }
                }
                walks.push(walk);
            }
            walks.sort_unstable();
            for walk in &walks {
                sink.write(walk);
            }
            // The run is done, attributes of every member included; continue
            // after its last declaration.
            let last = run
                .last()
                .map_or(current.id(), |member| member.declaration.id());
            while cursor.node().id() != last && cursor.goto_next_sibling() {}
        } else if let Some(frame) =
            parent.filter(|f| f.transparent_child(current, cursor.field_name()))
        {
            let field = cursor.field_name();
            if cursor.goto_first_child() {
                frames.push(frame.child(current, field, true));
                descended = true;
            }
        } else if current.child_count() == 0 {
            put(sink, current.kind().as_bytes());
            put(sink, source.get(current.byte_range()).unwrap_or(&[]));
        } else {
            sink.write(b"(");
            put(sink, current.kind().as_bytes());
            let field = cursor.field_name();
            if cursor.goto_first_child() {
                frames.push(match parent {
                    Some(frame) => frame.child(current, field, false),
                    None => Frame {
                        kind: current.kind(),
                        transparent: false,
                        body: false,
                        expression_macro: false,
                    },
                });
                descended = true;
            } else {
                sink.write(b")");
            }
        }
        if descended {
            continue;
        }
        if cursor.node().id() == root {
            return true;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return true;
            }
            if !frames.pop().is_some_and(|frame| frame.transparent) {
                sink.write(b")");
            }
            if cursor.node().id() == root {
                return true;
            }
        }
    }
}

/// BLAKE3 output, 32 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest([u8; 32]);

impl Digest {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({})", self.to_hex())
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Why an item could not be hashed: coarse categories of its parse errors,
/// or `nesting_too_deep` for an error-free item whose use runs nest past
/// [`MAX_USE_RUN_NESTING`]. Heuristic labels for the measurement; the set is
/// closed and sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ErrorCategory {
    /// The error text contains `$` or `~` (trap 2: macro punctuation).
    MacroPunct,
    /// The error sits inside a macro definition or invocation body.
    MacroBody,
    /// The error sits in or right after a `where` clause (trap 2: multi-line `where`).
    WhereMultiline,
    /// A `MISSING` node: tree-sitter inserted a token to recover.
    Missing,
    /// No parse error, but use runs nest past [`MAX_USE_RUN_NESTING`]: the
    /// walk stopped before the item's end (module docs, "Depth").
    NestingTooDeep,
    Other,
}

impl ErrorCategory {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MacroPunct => "macro_punct",
            Self::MacroBody => "macro_body",
            Self::WhereMultiline => "where_multiline",
            Self::Missing => "missing",
            Self::NestingTooDeep => "nesting_too_deep",
            Self::Other => "other",
        }
    }
}

/// Result of hashing one item: a digest, or the explicit refusal to produce one.
///
/// `CannotVerify` is never collapsed into a hash of any kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashState {
    Hashed(Digest),
    CannotVerify { categories: Vec<ErrorCategory> },
}

impl HashState {
    #[must_use]
    pub fn digest(&self) -> Option<Digest> {
        match self {
            Self::Hashed(digest) => Some(*digest),
            Self::CannotVerify { .. } => None,
        }
    }
}

/// The item's own name node (`name` field); `None` for `impl` blocks.
#[must_use]
pub fn name_node(item: Node) -> Option<Node> {
    item.child_by_field_name("name")
}

/// Outer attributes attached to `item`: the run of `attribute_item` siblings
/// right before it (comments in between do not break the run), in source order.
/// tree-sitter-rust keeps attributes as siblings, not children, and 05 §5.2
/// wants them hashed: they change behaviour.
#[must_use]
pub fn attached_attributes(item: Node) -> Vec<Node> {
    let mut attributes = Vec::new();
    let mut previous = item.prev_sibling();
    while let Some(node) = previous {
        if node.kind() == "attribute_item" {
            attributes.push(node);
        } else if !is_comment(node) {
            break;
        }
        previous = node.prev_sibling();
    }
    attributes.reverse();
    attributes
}

/// Whether the item or one of its attached attributes has a parse error.
#[must_use]
pub fn item_has_error(item: Node) -> bool {
    item.has_error() || attached_attributes(item).iter().any(|a| a.has_error())
}

/// Hashes one item with its attached attributes, or reports `cannot_verify`.
#[must_use]
pub fn hash_item(item: Node, source: &[u8]) -> HashState {
    let attributes = attached_attributes(item);
    if item.has_error() || attributes.iter().any(|a| a.has_error()) {
        let mut categories = BTreeSet::new();
        for node in attributes.iter().chain(std::iter::once(&item)) {
            if node.has_error() {
                categories.extend(error_categories(*node, source));
            }
        }
        return HashState::CannotVerify {
            categories: categories.into_iter().collect(),
        };
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(recipe_header());
    let complete = attributes
        .iter()
        .all(|attribute| normalize(*attribute, source, None, &mut hasher))
        && normalize(item, source, name_node(item), &mut hasher);
    finish(complete, &hasher)
}

/// The digest of a complete walk; `cannot_verify` for an incomplete one.
fn finish(complete: bool, hasher: &blake3::Hasher) -> HashState {
    if complete {
        HashState::Hashed(Digest(*hasher.finalize().as_bytes()))
    } else {
        HashState::CannotVerify {
            categories: vec![ErrorCategory::NestingTooDeep],
        }
    }
}

/// Hashes an arbitrary node without attributes or a name exclusion.
/// For fingerprints other than the item hash; still refuses on parse errors.
#[must_use]
pub fn hash_node(node: Node, source: &[u8]) -> HashState {
    if node.has_error() {
        return HashState::CannotVerify {
            categories: error_categories(node, source),
        };
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(recipe_header());
    let complete = normalize(node, source, None, &mut hasher);
    finish(complete, &hasher)
}

const MACRO_KINDS: &[&str] = &[
    "macro_definition",
    "macro_rule",
    "macro_invocation",
    "token_tree",
    "token_tree_pattern",
    "token_repetition",
    "token_repetition_pattern",
    "token_binding_pattern",
];

/// Categories of every `ERROR` and `MISSING` node inside `scope`, sorted and deduplicated.
#[must_use]
pub fn error_categories(scope: Node, source: &[u8]) -> Vec<ErrorCategory> {
    let mut categories = BTreeSet::new();
    let root = scope.id();
    let mut cursor = scope.walk();
    loop {
        let current = cursor.node();
        let mut descend = true;
        if current.is_missing() {
            categories.insert(ErrorCategory::Missing);
            descend = false;
        } else if current.is_error() {
            categories.insert(classify_error(current, scope, source));
            descend = false;
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.node().id() == root {
            break;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root {
                return categories.into_iter().collect();
            }
        }
    }
    categories.into_iter().collect()
}

fn classify_error(error: Node, scope: Node, source: &[u8]) -> ErrorCategory {
    let text = source.get(error.byte_range()).unwrap_or(&[]);
    if text.contains(&b'$') || text.contains(&b'~') {
        return ErrorCategory::MacroPunct;
    }
    let mut in_macro = false;
    let mut in_where = false;
    let mut ancestor = Some(error);
    while let Some(node) = ancestor {
        if MACRO_KINDS.contains(&node.kind()) {
            in_macro = true;
        }
        if node.kind() == "where_clause" {
            in_where = true;
        }
        if node.id() == scope.id() {
            break;
        }
        ancestor = node.parent();
    }
    if in_macro {
        return ErrorCategory::MacroBody;
    }
    if in_where
        || error
            .prev_sibling()
            .is_some_and(|previous| previous.kind() == "where_clause")
        || text.starts_with(b"where")
    {
        return ErrorCategory::WhereMultiline;
    }
    ErrorCategory::Other
}
