//! The string value of a `const` or `static` item (docs/features/constant-census.md,
//! D2 and D4: the reader of the `constants` census in `specengine-eval`).
//!
//! An item's value is the text between the quotes of its whole initializer
//! when that is one plain `"..."` or raw `r#*"..."#*` string literal, never
//! unescaped. Anything else is [`NotValued`], the first category that
//! applies, in the order of [`NotValued::ALL`]. The reader looks at the
//! item's node, its initializer node and that literal's bytes, never below:
//! constant work per item, no recursion, no I/O, no hash and nothing stored,
//! so the code index stamp ([`crate::CODE_INDEX_FORMAT`]) is untouched.
//!
//! Only file-level and inline-`mod` items exist ([`crate::items`]): an
//! associated `const` of an `impl` or a trait, a `const` in a function body
//! and an enum variant are no items, so never read. The literal forms are
//! the Rust language's, nothing of a project's convention.

use tree_sitter::{Node, Tree};

use crate::items::ItemRecord;

/// The item kinds the reader answers for; every other kind reads `None`.
const READ_KINDS: [&str; 2] = ["const_item", "static_item"];

/// The initializer of a `const_item` or a `static_item` (tree-sitter-rust's
/// field name).
const INITIALIZER_FIELD: &str = "value";

/// Initializer kinds that compute their value: a macro invocation (any
/// macro: `concat!`, `include_str!`, `env!`, a project's own) and a call
/// (a path, a function or a method call alike).
const COMPUTED_KINDS: [&str; 2] = ["macro_invocation", "call_expression"];

/// A char or byte char literal (`'x'`, `b'x'`).
const CHAR_KIND: &str = "char_literal";

/// A plain string literal, `b"..."` and `c"..."` included (the grammar
/// aliases their opening to `"`).
const PLAIN_KIND: &str = "string_literal";

/// A raw string literal, `br"..."` and `cr"..."` included.
const RAW_KIND: &str = "raw_string_literal";

/// What [`const_value`] reads from one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    /// The bytes between the literal's quotes, as written.
    Valued(String),
    /// No value, with the first category that applies.
    NotValued(NotValued),
}

/// Why an item holds no value; tried in the order of [`NotValued::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NotValued {
    /// The item or an attached attribute has a parse error.
    Broken,
    /// The initializer is a macro invocation or a call.
    Macro,
    /// The initializer is a byte string, a C string (raw or not), a char
    /// or a byte char.
    ByteString,
    /// A plain literal holding `\` (an escape or a line continuation).
    Escaped,
    /// A literal holding a line end as written (LF or CR).
    MultiLine,
    /// Anything else: no initializer, a number, a path, a reference, a
    /// parenthesised or block expression, a cast, a tuple, an array.
    Other,
}

impl NotValued {
    /// Every category, in the order they are tried.
    pub const ALL: [Self; 6] = [
        Self::Broken,
        Self::Macro,
        Self::ByteString,
        Self::Escaped,
        Self::MultiLine,
        Self::Other,
    ];

    /// The category's stored and reported name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Broken => "broken",
            Self::Macro => "macro",
            Self::ByteString => "byte_string",
            Self::Escaped => "escaped",
            Self::MultiLine => "multi_line",
            Self::Other => "other",
        }
    }
}

/// The value of `item`, an item of `tree` (parsed from `source`), or
/// `None` when `item` is no `const_item` or `static_item`, or when no node
/// of its kind spans exactly its byte range in `tree` (an item of another
/// tree).
#[must_use]
pub fn const_value(tree: &Tree, source: &str, item: &ItemRecord) -> Option<ConstValue> {
    if !READ_KINDS.contains(&item.kind.as_str()) {
        return None;
    }
    let range = &item.byte_range;
    // `descendant_for_byte_range` descends in a loop, never recursively;
    // the smallest node spanning an item's range is the item itself.
    let node = tree
        .root_node()
        .descendant_for_byte_range(range.start, range.end)?;
    if node.kind() != item.kind || node.byte_range() != *range {
        return None;
    }
    Some(match not_valued(node, item, source) {
        Ok(text) => ConstValue::Valued(text.to_owned()),
        Err(category) => ConstValue::NotValued(category),
    })
}

/// The item's value as written, or the first category that applies.
fn not_valued<'source>(
    node: Node,
    item: &ItemRecord,
    source: &'source str,
) -> Result<&'source str, NotValued> {
    if item.has_error || node.has_error() {
        return Err(NotValued::Broken);
    }
    let Some(initializer) = node.child_by_field_name(INITIALIZER_FIELD) else {
        return Err(NotValued::Other);
    };
    let kind = initializer.kind();
    if COMPUTED_KINDS.contains(&kind) {
        return Err(NotValued::Macro);
    }
    if kind == CHAR_KIND {
        return Err(NotValued::ByteString);
    }
    let raw = if kind == PLAIN_KIND {
        false
    } else if kind == RAW_KIND {
        true
    } else {
        return Err(NotValued::Other);
    };
    let literal = source
        .get(initializer.byte_range())
        .ok_or(NotValued::Other)?;
    let bytes = literal.as_bytes();
    // A byte or C string opens with its letter before the quote (plain) or
    // before the `r` (raw).
    if matches!(bytes.first(), Some(b'b' | b'c')) {
        return Err(NotValued::ByteString);
    }
    let text = if raw {
        between_raw_quotes(literal)
    } else {
        between_plain_quotes(literal)
    }
    .ok_or(NotValued::Other)?;
    if !raw && text.contains('\\') {
        return Err(NotValued::Escaped);
    }
    if text.contains(['\n', '\r']) {
        return Err(NotValued::MultiLine);
    }
    Ok(text)
}

/// The text of a plain literal `"..."` between its quotes.
fn between_plain_quotes(literal: &str) -> Option<&str> {
    literal.strip_prefix('"')?.strip_suffix('"')
}

/// The text of a raw literal `r#*"..."#*` between its quotes.
fn between_raw_quotes(literal: &str) -> Option<&str> {
    let after_r = literal.strip_prefix('r')?;
    let hashes = after_r.len() - after_r.trim_start_matches('#').len();
    let opened = after_r.get(hashes..)?.strip_prefix('"')?;
    let closed = opened.get(..opened.len().checked_sub(hashes)?)?;
    if !opened.get(closed.len()..)?.bytes().all(|byte| byte == b'#') {
        return None;
    }
    closed.strip_suffix('"')
}
