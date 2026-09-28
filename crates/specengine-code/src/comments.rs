//! Comment stripping by the parser's own comment nodes.
//!
//! Used as a perturbation in the hash measurement: every `line_comment` and
//! `block_comment` (doc comments `///`, `//!`, `/** */` included, recognised
//! by `kind()`) is replaced by one space, so tokens never glue together.

use tree_sitter::Tree;

use crate::hash::is_comment;

/// Byte ranges of all comment nodes in `tree`, in source order, outermost only.
#[must_use]
pub fn comment_ranges(tree: &Tree) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    loop {
        let current = cursor.node();
        let mut descend = true;
        if is_comment(current) {
            ranges.push(current.byte_range());
            descend = false;
        }
        if descend && cursor.goto_first_child() {
            continue;
        }
        if cursor.node().id() == root.id() {
            break;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root.id() {
                return ranges;
            }
        }
    }
    ranges
}

/// `source` with every comment replaced by a single space.
#[must_use]
pub fn strip_comments(source: &str, tree: &Tree) -> String {
    let mut out = String::with_capacity(source.len());
    let mut position = 0;
    for range in comment_ranges(tree) {
        if range.start < position || range.end > source.len() {
            continue;
        }
        out.push_str(&source[position..range.start]);
        out.push(' ');
        position = range.end;
    }
    out.push_str(&source[position..]);
    out
}
