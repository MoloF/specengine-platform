//! Rust comments bound to items (`docs/canon/code-bindings.md` "Rust binding"):
//! every comment of a file, doc comments included, gets one [`Binding`],
//! the place a marker in it names, and the place a mention in it belongs
//! to ("Mentions").
//!
//! An *item* is what [`crate::items::analyze_tree`] collects: `ITEM_KINDS`
//! at file level and directly in an inline `mod`, `MEMBER_KINDS` directly
//! in an `impl` or `trait`; nothing in function bodies. A comment binds by
//! the first rule that applies; a multi-line block comment trails by its
//! `/*` line and leads by its `*/` line:
//!
//! 1. a block comment, and an item starts later on its `*/` line (its
//!    first token, or the first of its outer attributes, next after the
//!    comment): that item — `/* m */ fn f() {}`;
//! 2. a comment on its own line, and the next token, past blank lines,
//!    comments and outer attributes `#[..]`, starts an item: that item —
//!    `// m`, `#[derive(Debug)]`, `struct S;`;
//! 3. a comment after an item's last token on that line (of several items
//!    ending there, the one whose last token is right before it): that
//!    item — `fn f() {} // m`, `} // m` closing an `impl`;
//! 4. a comment after an item's opening `{` on that line: that item —
//!    `impl S { // m`;
//! 5. an inner doc comment (`//!`, `/*!`) at file level or directly in an
//!    inline `mod`: the file's module or that `mod` — judged before rules
//!    1 to 4, as rustdoc reads it: `//! m` above `mod a;` documents the
//!    file's module, never `a`;
//! 6. anything else: [`Binding::Unanchored`] with the first category that
//!    applies — in a macro call's token tree or a `macro_rules!` body
//!    (`in_macro`); in a function body, block or a `const`/`static`
//!    initializer (`in_body`); followed, past what rule 2 crosses, by `}`
//!    or the file's end (`no_item`); else (`not_an_item`: a `use`, an
//!    `extern crate`, an item-position macro call, a field, a variant, an
//!    associated `const` or `type`, `#![..]`).
//! 7. an item of rules 1 to 5 with a parse error (never the file's module),
//!    or an `ERROR` node standing where an item would: [`Binding::CannotVerify`]
//!    with the hash's [`ErrorCategory`], never a guessed item.
//!
//! Strict first: fields and variants bind nothing yet. A mention belongs
//! to its comment's binding when that is an item, a module or
//! `cannot_verify`; an unanchored comment's mention to the innermost item
//! enclosing it; else to the file ([`Binding::File`]).
//!
//! Only `kind()` strings are compared, comments are recognised by kind
//! (never `is_extra()`, true for `ERROR` nodes too). One explicit-stack walk
//! over the tree, then one lookup per comment: linear in the file.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::grammar::RustParser;
use crate::hash::{ErrorCategory, HashState, hash_item, is_comment};
use crate::items::{FileAnalysis, Hashing, MOD_KIND, analyze_with_nodes};

/// Kinds a comment inside of which is `in_macro`: a macro call (its token
/// tree included) and a `macro_rules!` definition (its rules included). A
/// token tree elsewhere, an attribute's arguments, is no macro.
const MACRO_KINDS: [&str; 2] = ["macro_invocation", "macro_definition"];

/// A block: function bodies, block expressions (`in_body`).
const BLOCK_KIND: &str = "block";

/// Items whose value after `=` is an initializer (`in_body`).
const INITIALIZED_KINDS: [&str; 2] = ["const_item", "static_item"];

/// The `=` before an initializer.
const INITIALIZER_SIGN: &str = "=";

/// An outer attribute `#[..]`, crossed by rule 2.
const ATTRIBUTE_KIND: &str = "attribute_item";

/// The item kinds whose `body` is a container of items ([`MOD_KIND`] too).
const MEMBER_OWNER_KINDS: [&str; 2] = ["impl_item", "trait_item"];

/// The field naming a body.
const BODY_FIELD: &str = "body";

/// A `macro_rules!` definition: its opening brace is a direct child.
const MACRO_DEFINITION_KIND: &str = "macro_definition";

/// Braces.
const OPEN_BRACE: &str = "{";
const CLOSE_BRACE: &str = "}";

/// The block comment kind (the other is `line_comment`).
const BLOCK_COMMENT_KIND: &str = "block_comment";

/// The field of a doc comment's `!` marker (`//!`, `/*!`).
const INNER_DOC_FIELD: &str = "inner";

/// Why a comment binds to nothing (rule 6); the first that applies, in
/// this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unanchored {
    /// In a macro call's token tree or a `macro_rules!` body.
    InMacro,
    /// In a function body, a block or a `const`/`static` initializer.
    InBody,
    /// Followed, past blank lines, comments and outer attributes, by `}`
    /// or the file's end.
    NoItem,
    /// At anything that is no item: a `use`, an `extern crate`, an
    /// item-position macro call, a field, a variant, an associated `const`
    /// or `type`, `#![..]`, a token of a signature.
    NotAnItem,
}

impl Unanchored {
    /// The fixed category name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InMacro => "in_macro",
            Self::InBody => "in_body",
            Self::NoItem => "no_item",
            Self::NotAnItem => "not_an_item",
        }
    }
}

/// What a comment binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Binding {
    /// An item, by its index in [`FileAnalysis::items`].
    Item(usize),
    /// The file's module (`None`), or the module of an inline `mod` item
    /// (its index in [`FileAnalysis::items`]).
    Module(Option<usize>),
    /// Nothing (rule 6), with the category.
    Unanchored(Unanchored),
    /// The item it would bind has a parse error (rule 7).
    CannotVerify(ErrorCategory),
    /// The file itself: where a mention belongs when nothing else holds it;
    /// never a marker's binding.
    File,
}

/// One comment of the file and what it binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundComment {
    /// The comment's bytes in the source (a doc line comment ends with its
    /// line end).
    pub range: Range<usize>,
    /// Where a marker in it binds; never [`Binding::File`].
    pub binding: Binding,
    /// Where a mention in it belongs: the binding when it is an item, a
    /// module or `cannot_verify`; for an unanchored comment the innermost
    /// item enclosing it (`cannot_verify` when that item has a parse
    /// error), else [`Binding::File`].
    pub mention: Binding,
}

/// One file's items and its comments, bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustBinding {
    /// The items ([`crate::items::analyze_tree`]'s, not hashed:
    /// [`FileAnalysis::hashed`] is `false`, only an item with a parse
    /// error has its state, an error-free one [`HashState::NotHashed`]:
    /// [`hash_bound_items`] hashes one); bindings index them.
    pub analysis: FileAnalysis,
    /// Every comment, in source order.
    pub comments: Vec<BoundComment>,
}

/// Parses and binds one file. `None` when the parser gave no tree.
pub fn bind_file(parser: &mut RustParser, source: &str) -> Option<RustBinding> {
    let tree = parser.parse(source)?;
    Some(bind_tree(&tree, source))
}

/// Binds the comments of an already parsed file.
#[must_use]
pub fn bind_tree(tree: &Tree, source: &str) -> RustBinding {
    let (analysis, nodes) = analyze_with_nodes(tree, source, Hashing::ErrorsOnly);
    let walked = Walk::new(&analysis, &nodes, source).run(tree.root_node());
    let comments = walked
        .comments
        .iter()
        .map(|comment| {
            let binding = walked.bind(comment, &analysis);
            let mention = match binding {
                Binding::Unanchored(_) => match comment.ctx.enclosing {
                    Some(item) => checked(Binding::Item(item), &analysis),
                    None => Binding::File,
                },
                other => other,
            };
            BoundComment {
                range: comment.range.clone(),
                binding,
                mention,
            }
        })
        .collect();
    RustBinding { analysis, comments }
}

/// An item binding, or `cannot_verify` when the item has a parse error
/// (rule 7; the file's module never has one).
fn checked(binding: Binding, analysis: &FileAnalysis) -> Binding {
    let (Binding::Item(index) | Binding::Module(Some(index))) = binding else {
        return binding;
    };
    match analysis.items.get(index) {
        Some(item) if item.has_error => Binding::CannotVerify(first_category(&item.state)),
        _ => binding,
    }
}

/// The first of a refused hash's categories; `other` when none is named.
fn first_category(state: &HashState) -> ErrorCategory {
    match state {
        HashState::CannotVerify { categories } => {
            categories.first().copied().unwrap_or(ErrorCategory::Other)
        }
        HashState::Hashed(_) | HashState::NotHashed => ErrorCategory::Other,
    }
}

/// The whole-item hashes ([`hash_item`], the recipe of 05 §5.2) of the
/// items at `items`, indexes into the [`RustBinding::analysis`] that
/// [`bind_tree`] gives for the same `tree` and `source`, in the order
/// asked; `None` for an index past its items. Only these items are
/// hashed: the binding itself hashes none (an error-free item's state
/// there is [`HashState::NotHashed`]), so a caller that needs a bound
/// item's hash (`spec.lock`) asks for it here and never reads that state.
#[must_use]
pub fn hash_bound_items(tree: &Tree, source: &str, items: &[usize]) -> Vec<Option<HashState>> {
    let (_, nodes) = analyze_with_nodes(tree, source, Hashing::ErrorsOnly);
    items
        .iter()
        .map(|&index| {
            nodes
                .get(index)
                .map(|node| hash_item(*node, source.as_bytes()))
        })
        .collect()
}

/// The node a comment or token hangs under, as the rules need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parent {
    /// The file (`source_file`).
    File,
    /// The body of a collected inline `mod` item.
    Mod(usize),
    /// The body of a collected `impl` or `trait` item.
    Members,
    /// Anything else.
    Other,
}

impl Parent {
    /// Items are collected among its children.
    fn is_container(self) -> bool {
        !matches!(self, Self::Other)
    }
}

/// What the walk knows about a node from its ancestors.
#[derive(Debug, Clone, Copy)]
struct Ctx {
    in_macro: bool,
    in_body: bool,
    /// The innermost collected item holding it.
    enclosing: Option<usize>,
    /// Its parent.
    parent: Parent,
}

/// A leaf token (no comment, no zero-width node such as `MISSING`).
#[derive(Debug, Clone, Copy)]
struct Token {
    start_row: usize,
    /// The row of its last byte.
    end_row: usize,
    close_brace: bool,
}

/// A comment and where it sits among the tokens.
#[derive(Debug, Clone)]
struct Comment {
    range: Range<usize>,
    start_row: usize,
    /// The row of its last byte.
    end_row: usize,
    block: bool,
    inner_doc: bool,
    /// The index of the first token after it (`tokens.len()`: none).
    next: usize,
    ctx: Ctx,
}

/// What the walk ends with.
enum Frame<'tree> {
    Enter(Node<'tree>, Ctx),
    /// A collected item ends: its first token's index.
    ExitItem(usize, usize),
    /// An outer attribute ends: its first token's index.
    ExitAttribute(usize),
    /// An `ERROR` node at container level ends: its first token's index.
    ExitError(ErrorCategory, usize),
}

/// The walk's state and its tables.
struct Walk<'a> {
    source: &'a [u8],
    /// Collected item node id → its index.
    item_of_node: HashMap<usize, usize>,
    /// Body node id of a collected `mod` → the item.
    mod_bodies: HashMap<usize, usize>,
    /// Body node ids of collected `impl`s and `trait`s.
    member_bodies: HashSet<usize>,
    /// Opening brace node id → its collected item.
    brace_of_node: HashMap<usize, usize>,
    tokens: Vec<Token>,
    comments: Vec<Comment>,
    /// First token → the collected item starting there.
    item_first: HashMap<usize, usize>,
    /// Last token → the collected item ending there.
    item_last: HashMap<usize, usize>,
    /// Token → the collected item it opens with `{`.
    open_brace: HashMap<usize, usize>,
    /// First token of an outer attribute → the token after it.
    attribute_after: HashMap<usize, usize>,
    /// First and last tokens of an `ERROR` node at container level.
    error_first: HashMap<usize, ErrorCategory>,
    error_last: HashMap<usize, ErrorCategory>,
    /// Token index → the first token at or after it that no outer
    /// attribute starts (`tokens.len()`: none); filled after the walk.
    past_attributes: Vec<usize>,
}

impl<'a> Walk<'a> {
    fn new(analysis: &FileAnalysis, nodes: &[Node<'_>], source: &'a str) -> Self {
        let mut walk = Walk {
            source: source.as_bytes(),
            item_of_node: HashMap::new(),
            mod_bodies: HashMap::new(),
            member_bodies: HashSet::new(),
            brace_of_node: HashMap::new(),
            tokens: Vec::new(),
            comments: Vec::new(),
            item_first: HashMap::new(),
            item_last: HashMap::new(),
            open_brace: HashMap::new(),
            attribute_after: HashMap::new(),
            error_first: HashMap::new(),
            error_last: HashMap::new(),
            past_attributes: Vec::new(),
        };
        for (index, (node, item)) in nodes.iter().zip(&analysis.items).enumerate() {
            walk.item_of_node.insert(node.id(), index);
            let body = node.child_by_field_name(BODY_FIELD);
            if let Some(body) = body {
                if item.kind == MOD_KIND {
                    walk.mod_bodies.insert(body.id(), index);
                } else if MEMBER_OWNER_KINDS.contains(&item.kind.as_str()) {
                    walk.member_bodies.insert(body.id());
                }
            }
            if let Some(brace) = opening_brace(*node, body) {
                walk.brace_of_node.insert(brace.id(), index);
            }
        }
        walk
    }

    /// Walks the tree in source order on an explicit stack.
    fn run(mut self, root: Node<'_>) -> Self {
        let mut stack = vec![Frame::Enter(
            root,
            Ctx {
                in_macro: false,
                in_body: false,
                enclosing: None,
                parent: Parent::Other,
            },
        )];
        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Enter(node, ctx) => self.enter(node, ctx, root, &mut stack),
                Frame::ExitItem(item, first) => {
                    if self.tokens.len() > first {
                        self.item_first.entry(first).or_insert(item);
                        self.item_last.insert(self.tokens.len() - 1, item);
                    }
                }
                Frame::ExitAttribute(first) => {
                    if self.tokens.len() > first {
                        self.attribute_after.insert(first, self.tokens.len());
                    }
                }
                Frame::ExitError(category, first) => {
                    if self.tokens.len() > first {
                        self.error_first.entry(first).or_insert(category);
                        self.error_last.insert(self.tokens.len() - 1, category);
                    }
                }
            }
        }
        let count = self.tokens.len();
        let mut past = vec![count; count + 1];
        for index in (0..count).rev() {
            past[index] = match self.attribute_after.get(&index) {
                Some(&after) if after > index => past[after],
                _ => index,
            };
        }
        self.past_attributes = past;
        self
    }

    fn enter<'t>(&mut self, node: Node<'t>, ctx: Ctx, root: Node<'t>, stack: &mut Vec<Frame<'t>>) {
        if is_comment(node) {
            let range = node.byte_range();
            self.comments.push(Comment {
                start_row: node.start_position().row,
                end_row: end_row(node),
                block: node.kind() == BLOCK_COMMENT_KIND,
                inner_doc: node.child_by_field_name(INNER_DOC_FIELD).is_some(),
                next: self.tokens.len(),
                ctx,
                range,
            });
            return;
        }
        let first = self.tokens.len();
        if node.is_error() && ctx.parent.is_container() {
            let category = first_category(&hash_item(node, self.source));
            stack.push(Frame::ExitError(category, first));
        }
        if node.child_count() == 0 {
            // A `MISSING` node, or an empty file's root, holds no text.
            if !node.is_missing() && node.start_byte() < node.end_byte() {
                if let Some(&item) = self.brace_of_node.get(&node.id()) {
                    self.open_brace.insert(self.tokens.len(), item);
                }
                self.tokens.push(Token {
                    start_row: node.start_position().row,
                    end_row: end_row(node),
                    close_brace: node.kind() == CLOSE_BRACE,
                });
            }
            return;
        }
        let kind = node.kind();
        let item = self.item_of_node.get(&node.id()).copied();
        if let Some(item) = item {
            stack.push(Frame::ExitItem(item, first));
        } else if kind == ATTRIBUTE_KIND {
            stack.push(Frame::ExitAttribute(first));
        }
        let parent = if node.id() == root.id() {
            Parent::File
        } else if let Some(&item) = self.mod_bodies.get(&node.id()) {
            Parent::Mod(item)
        } else if self.member_bodies.contains(&node.id()) {
            Parent::Members
        } else {
            Parent::Other
        };
        let base = Ctx {
            in_macro: ctx.in_macro || MACRO_KINDS.contains(&kind),
            in_body: ctx.in_body || kind == BLOCK_KIND,
            enclosing: item.or(ctx.enclosing),
            parent,
        };
        let initialized = INITIALIZED_KINDS.contains(&kind);
        let mut cursor = node.walk();
        let children: Vec<Node<'t>> = node.children(&mut cursor).collect();
        let mut framed = Vec::with_capacity(children.len());
        let mut after_sign = false;
        for child in children {
            let mut child_ctx = base;
            child_ctx.in_body |= after_sign;
            if initialized && child.kind() == INITIALIZER_SIGN {
                after_sign = true;
            }
            framed.push(Frame::Enter(child, child_ctx));
        }
        stack.extend(framed.into_iter().rev());
    }

    /// The comment's binding by rules 1 to 7.
    fn bind(&self, comment: &Comment, analysis: &FileAnalysis) -> Binding {
        if comment.inner_doc {
            match comment.ctx.parent {
                Parent::File => return Binding::Module(None),
                Parent::Mod(item) => return checked(Binding::Module(Some(item)), analysis),
                Parent::Members | Parent::Other => {}
            }
        }
        let count = self.tokens.len();
        let next = comment.next.min(count);
        let past = self.past_attributes.get(next).copied().unwrap_or(count);
        let previous = next.checked_sub(1).map(|index| (index, self.tokens[index]));
        // 1. A block comment leading an item on its `*/` line.
        if comment.block
            && self
                .tokens
                .get(next)
                .is_some_and(|token| token.start_row == comment.end_row)
            && let Some(binding) = self.leading(past, analysis)
        {
            return binding;
        }
        // 2. On its own line, before an item.
        let own_line = previous.is_none_or(|(_, token)| token.end_row < comment.start_row);
        if own_line && let Some(binding) = self.leading(past, analysis) {
            return binding;
        }
        if let Some((index, token)) = previous
            && token.end_row == comment.start_row
        {
            // 3. Trailing an item's last token.
            if let Some(&item) = self.item_last.get(&index) {
                return checked(Binding::Item(item), analysis);
            }
            if let Some(&category) = self.error_last.get(&index) {
                return Binding::CannotVerify(category);
            }
            // 4. Trailing an item's opening brace.
            if let Some(&item) = self.open_brace.get(&index) {
                return checked(Binding::Item(item), analysis);
            }
        }
        // 6. Nothing.
        let category = if comment.ctx.in_macro {
            Unanchored::InMacro
        } else if comment.ctx.in_body {
            Unanchored::InBody
        } else if self.tokens.get(past).is_none_or(|token| token.close_brace) {
            Unanchored::NoItem
        } else {
            Unanchored::NotAnItem
        };
        Binding::Unanchored(category)
    }

    /// The item (or `ERROR` node) starting at token `past`, if any.
    fn leading(&self, past: usize, analysis: &FileAnalysis) -> Option<Binding> {
        if let Some(&item) = self.item_first.get(&past) {
            return Some(checked(Binding::Item(item), analysis));
        }
        self.error_first
            .get(&past)
            .map(|&category| Binding::CannotVerify(category))
    }
}

/// The opening `{` of an item: its body's first token, or a
/// `macro_rules!` definition's own; `None` when it has none (a unit
/// struct, `mod x;`, a type alias) or it is missing.
fn opening_brace<'t>(item: Node<'t>, body: Option<Node<'t>>) -> Option<Node<'t>> {
    let brace = if item.kind() == MACRO_DEFINITION_KIND {
        let mut cursor = item.walk();
        item.children(&mut cursor)
            .find(|child| child.kind() == OPEN_BRACE)
    } else {
        body?.child(0).filter(|child| child.kind() == OPEN_BRACE)
    }?;
    (!brace.is_missing()).then_some(brace)
}

/// The row of a node's last byte (a doc line comment ends with its line
/// end, which belongs to its own row).
fn end_row(node: Node<'_>) -> usize {
    let end = node.end_position();
    if end.column == 0 && end.row > 0 && node.end_byte() > node.start_byte() {
        end.row - 1
    } else {
        end.row
    }
}
