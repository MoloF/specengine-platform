//! The items of one Rust file, each with its hash state (05 §5.1 layer A).
//!
//! Items are the kinds listed in
//! `docs/canon/code-identity.md` "Units and `qpath`": `function_item`,
//! `struct_item`, `enum_item`, `union_item`, `trait_item`, `impl_item` (plus
//! its methods), `const_item`, `static_item`, `type_item`, `mod_item`,
//! `macro_definition`.
//! Inline `mod` bodies are entered; items inside function bodies belong to the
//! function's hash. Items produced by macros are invisible (stated limitation).

use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::grammar::RustParser;
use crate::hash::{HashState, attached_attributes, hash_item, item_has_error};

/// Item kinds collected at file and module level.
pub const ITEM_KINDS: &[&str] = &[
    "function_item",
    "struct_item",
    "enum_item",
    "union_item",
    "trait_item",
    "impl_item",
    "const_item",
    "static_item",
    "type_item",
    "mod_item",
    "macro_definition",
];

/// Item kinds collected inside `impl` and `trait` bodies (methods).
pub const MEMBER_KINDS: &[&str] = &["function_item", "function_signature_item"];

/// One item and its hash state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRecord {
    /// Public `kind()` string of the node.
    pub kind: String,
    /// Text of the `name` field; `None` for `impl` blocks.
    pub name: Option<String>,
    /// Human label: the name, or for `impl` blocks `Type` / `<Type as Trait>` with whitespace removed.
    pub label: String,
    /// Inline `mod` chain enclosing the item inside this file.
    pub mod_path: Vec<String>,
    /// Label of the enclosing `impl` or `trait` for methods.
    pub owner: Option<String>,
    pub byte_range: Range<usize>,
    /// 1-based start line.
    pub line: usize,
    /// The item or an attached attribute has a parse error (never hashed then).
    pub has_error: bool,
    pub state: HashState,
}

/// A `mod name;` declaration whose body lives in another file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDeclaration {
    pub name: String,
    pub mod_path: Vec<String>,
    pub line: usize,
    /// Value of an attached `#[path = "..."]` attribute.
    pub path_attribute: Option<String>,
}

/// Everything the hash layer knows about one file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileAnalysis {
    pub items: Vec<ItemRecord>,
    /// The root node has a parse error somewhere.
    pub has_error: bool,
    /// `ERROR` nodes at file or module level that did not become an item.
    pub orphan_errors: usize,
    pub mod_declarations: Vec<ModDeclaration>,
}

impl FileAnalysis {
    /// Number of `#[path]` attributes on `mod` declarations.
    #[must_use]
    pub fn path_attributes(&self) -> usize {
        self.mod_declarations
            .iter()
            .filter(|m| m.path_attribute.is_some())
            .count()
    }
}

/// Parses and analyses one file. `None` when the parse was cancelled.
pub fn analyze_file(parser: &mut RustParser, source: &str) -> Option<FileAnalysis> {
    let tree = parser.parse(source)?;
    Some(analyze_tree(&tree, source))
}

/// Collects the items of an already parsed file.
#[must_use]
pub fn analyze_tree(tree: &Tree, source: &str) -> FileAnalysis {
    let root = tree.root_node();
    let mut analysis = FileAnalysis {
        has_error: root.has_error(),
        ..FileAnalysis::default()
    };
    collect(root, source.as_bytes(), &mut analysis);
    analysis
}

struct Scope {
    mod_path: Vec<String>,
    owner: Option<String>,
}

/// One open container of the walk: its remaining children and its scope.
struct Level<'tree> {
    children: std::vec::IntoIter<Node<'tree>>,
    scope: Scope,
}

impl<'tree> Level<'tree> {
    fn new(container: Node<'tree>, scope: Scope) -> Self {
        let mut cursor = container.walk();
        let children: Vec<Node<'tree>> = container.children(&mut cursor).collect();
        Self {
            children: children.into_iter(),
            scope,
        }
    }
}

/// Items of `root` in pre-order: an item's record, then the items of its
/// inline `mod`, `impl` or `trait` body, then its next sibling. An explicit
/// stack of [`Level`]s instead of recursion, so the nesting depth of the file
/// never bounds the thread stack.
fn collect(root: Node, source: &[u8], out: &mut FileAnalysis) {
    let mut stack = vec![Level::new(
        root,
        Scope {
            mod_path: Vec::new(),
            owner: None,
        },
    )];
    while let Some(level) = stack.last_mut() {
        let Some(child) = level.children.next() else {
            stack.pop();
            continue;
        };
        let scope = &level.scope;
        let kind = child.kind();
        if child.is_error() {
            out.orphan_errors += 1;
            continue;
        }
        let is_item = if scope.owner.is_some() {
            MEMBER_KINDS.contains(&kind)
        } else {
            ITEM_KINDS.contains(&kind)
        };
        if !is_item {
            continue;
        }
        let record = make_record(child, source, scope);
        let label = record.label.clone();
        let name = record.name.clone();
        out.items.push(record);
        let body = match kind {
            "mod_item" => match child.child_by_field_name("body") {
                Some(body) => {
                    let mut mod_path = scope.mod_path.clone();
                    mod_path.push(name.unwrap_or_default());
                    Some((
                        body,
                        Scope {
                            mod_path,
                            owner: None,
                        },
                    ))
                }
                None => {
                    out.mod_declarations.push(ModDeclaration {
                        name: name.unwrap_or_default(),
                        mod_path: scope.mod_path.clone(),
                        line: child.start_position().row + 1,
                        path_attribute: path_attribute(child, source),
                    });
                    None
                }
            },
            "impl_item" | "trait_item" => child.child_by_field_name("body").map(|body| {
                (
                    body,
                    Scope {
                        mod_path: scope.mod_path.clone(),
                        owner: Some(label),
                    },
                )
            }),
            _ => None,
        };
        if let Some((body, scope)) = body {
            stack.push(Level::new(body, scope));
        }
    }
}

fn make_record(node: Node, source: &[u8], scope: &Scope) -> ItemRecord {
    let kind = node.kind();
    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(str::to_owned);
    let label = if kind == "impl_item" {
        impl_label(node, source)
    } else {
        name.clone().unwrap_or_else(|| kind.to_owned())
    };
    ItemRecord {
        kind: kind.to_owned(),
        name,
        label,
        mod_path: scope.mod_path.clone(),
        owner: scope.owner.clone(),
        byte_range: node.byte_range(),
        line: node.start_position().row + 1,
        has_error: item_has_error(node),
        state: hash_item(node, source),
    }
}

/// `Type` for inherent impls, `<Type as Trait>` for trait impls, whitespace removed.
fn impl_label(node: Node, source: &[u8]) -> String {
    let ty = field_text_compact(node, "type", source).unwrap_or_default();
    match field_text_compact(node, "trait", source) {
        Some(trait_name) => format!("<{ty} as {trait_name}>"),
        None => ty,
    }
}

fn field_text_compact(node: Node, field: &str, source: &[u8]) -> Option<String> {
    let text = node.child_by_field_name(field)?.utf8_text(source).ok()?;
    Some(text.chars().filter(|c| !c.is_whitespace()).collect())
}

/// The string of an attached `#[path = "..."]` attribute, read structurally:
/// `attribute_item` → `attribute` → path `identifier` "path" + `value` string literal.
fn path_attribute(item: Node, source: &[u8]) -> Option<String> {
    attached_attributes(item)
        .into_iter()
        .find_map(|attribute_item| {
            let attribute = attribute_item
                .named_child(0)
                .filter(|n| n.kind() == "attribute")?;
            let path = attribute.named_child(0)?;
            if path.kind() != "identifier" || path.utf8_text(source).ok()? != "path" {
                return None;
            }
            let value = attribute.child_by_field_name("value")?;
            if value.kind() != "string_literal" {
                return None;
            }
            let mut text = String::new();
            let mut cursor = value.walk();
            for part in value.named_children(&mut cursor) {
                if matches!(part.kind(), "string_content" | "escape_sequence") {
                    text.push_str(part.utf8_text(source).ok()?);
                }
            }
            Some(text)
        })
}
