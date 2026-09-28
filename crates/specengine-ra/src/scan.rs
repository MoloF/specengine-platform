//! The items of one file and their monikers.
//!
//! The item set is the one of layer A (`specengine-code` `ITEM_KINDS` and
//! `MEMBER_KINDS`): `fn`, `struct`, `enum`, `union`, `trait`, `impl`, `const`,
//! `static`, `type`, `mod`, `macro_rules!` (plus unstable `macro`) at module
//! level and inside inline `mod` bodies, and the functions of `impl` and
//! `trait` bodies. Function bodies and macro invocations are not entered, so
//! the count stays comparable with the tree-sitter walk and excludes items
//! that exist only in macro expansions.

use ra_ap_ide::{
    FileId, MonikerDescriptorKind, MonikerIdentifier, MonikerResult, RootDatabase, Semantics,
};
use ra_ap_ide_db::defs::Definition;
use ra_ap_syntax::ast::{self, AstChildren, HasModuleItem, HasName};
use ra_ap_syntax::{AstNode, Edition, SourceFile};

/// Kind of a collected item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Function,
    /// A function inside an `impl` or `trait` body.
    Method,
    Struct,
    Enum,
    Union,
    Trait,
    Impl,
    Const,
    Static,
    TypeAlias,
    Module,
    MacroRules,
    /// Unstable `macro` (declarative macros 2.0).
    MacroDef,
}

impl ItemKind {
    pub const ALL: [ItemKind; 13] = [
        ItemKind::Function,
        ItemKind::Method,
        ItemKind::Struct,
        ItemKind::Enum,
        ItemKind::Union,
        ItemKind::Trait,
        ItemKind::Impl,
        ItemKind::Const,
        ItemKind::Static,
        ItemKind::TypeAlias,
        ItemKind::Module,
        ItemKind::MacroRules,
        ItemKind::MacroDef,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Function => "function",
            ItemKind::Method => "method",
            ItemKind::Struct => "struct",
            ItemKind::Enum => "enum",
            ItemKind::Union => "union",
            ItemKind::Trait => "trait",
            ItemKind::Impl => "impl",
            ItemKind::Const => "const",
            ItemKind::Static => "static",
            ItemKind::TypeAlias => "type_alias",
            ItemKind::Module => "mod",
            ItemKind::MacroRules => "macro_rules",
            ItemKind::MacroDef => "macro_def",
        }
    }
}

/// What rust-analyzer says about one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MonikerStatus {
    /// Resolved, with a non-local moniker: crate name, then SCIP-style
    /// descriptors (`a/` namespace, `T#` type, `f().` method, `x.` term,
    /// `[T]` type parameter, `m!` macro), e.g. `demo a/Point#norm().`.
    Moniker(String),
    /// Resolved to a definition that only has a local moniker.
    Local,
    /// Resolved, but `MonikerResult::from_def` gave nothing.
    NoMoniker,
    /// Not resolved to a definition: `cfg`-disabled, or the file is in no
    /// crate's module tree.
    Unresolved,
    /// The file is not part of the loaded workspace (outside every package root).
    NotLoaded,
}

impl MonikerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MonikerStatus::Moniker(_) => "moniker",
            MonikerStatus::Local => "local",
            MonikerStatus::NoMoniker => "none",
            MonikerStatus::Unresolved => "unresolved",
            MonikerStatus::NotLoaded => "not_loaded",
        }
    }
}

/// One item of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemRecord {
    pub kind: ItemKind,
    /// `None` for `impl` blocks and nameless items.
    pub name: Option<String>,
    /// 1-based line of the item's first token (attributes and doc comments included).
    pub line: usize,
    pub status: MonikerStatus,
}

/// The items of one file, in source order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileScan {
    /// The file belongs to at least one crate's module tree.
    pub in_crate: bool,
    pub items: Vec<ItemRecord>,
}

/// Items of a file rust-analyzer did not load, parsed syntactically only:
/// every item is [`MonikerStatus::NotLoaded`].
pub fn scan_unloaded(text: &str) -> FileScan {
    let tree = SourceFile::parse(text, Edition::CURRENT).tree();
    let lines = Lines::new(text);
    let items = collect(&tree)
        .into_iter()
        .map(|found| ItemRecord {
            kind: found.kind,
            name: found.name,
            line: lines.line(found.offset),
            status: MonikerStatus::NotLoaded,
        })
        .collect();
    FileScan {
        in_crate: false,
        items,
    }
}

pub(crate) fn scan_loaded(db: &RootDatabase, file_id: FileId) -> FileScan {
    let sema = Semantics::new(db);
    let tree = sema.parse_guess_edition(file_id);
    let in_crate = sema.file_to_module_defs(file_id).next().is_some();
    let lines = Lines::new(&tree.syntax().text().to_string());
    let items = collect(&tree)
        .into_iter()
        .map(|found| ItemRecord {
            kind: found.kind,
            line: lines.line(found.offset),
            status: resolve(&sema, db, &found.node),
            name: found.name,
        })
        .collect();
    FileScan { in_crate, items }
}

/// An item node found by [`collect`], before resolution.
struct Found {
    kind: ItemKind,
    node: Node,
    name: Option<String>,
    offset: u32,
}

enum Node {
    Fn(ast::Fn),
    Adt(ast::Adt),
    Trait(ast::Trait),
    Impl(ast::Impl),
    Const(ast::Const),
    Static(ast::Static),
    TypeAlias(ast::TypeAlias),
    Module(ast::Module),
    Macro(ast::Macro),
}

/// The layer A item set of `tree`, in source order, on an explicit stack of
/// item lists (the file, then each inline `mod` body) — no recursion.
fn collect(tree: &SourceFile) -> Vec<Found> {
    let mut found = Vec::new();
    let mut stack: Vec<AstChildren<ast::Item>> = vec![tree.items()];
    while let Some(top) = stack.last_mut() {
        let Some(item) = top.next() else {
            stack.pop();
            continue;
        };
        let offset = u32::from(item.syntax().text_range().start());
        let (kind, name, node) = match item {
            ast::Item::Fn(it) => (ItemKind::Function, name_of(&it), Node::Fn(it)),
            ast::Item::Struct(it) => (
                ItemKind::Struct,
                name_of(&it),
                Node::Adt(ast::Adt::Struct(it)),
            ),
            ast::Item::Enum(it) => (ItemKind::Enum, name_of(&it), Node::Adt(ast::Adt::Enum(it))),
            ast::Item::Union(it) => (
                ItemKind::Union,
                name_of(&it),
                Node::Adt(ast::Adt::Union(it)),
            ),
            ast::Item::Trait(it) => {
                let members = it.assoc_item_list();
                found.push(Found {
                    kind: ItemKind::Trait,
                    name: name_of(&it),
                    node: Node::Trait(it),
                    offset,
                });
                push_methods(members, &mut found);
                continue;
            }
            ast::Item::Impl(it) => {
                let members = it.assoc_item_list();
                found.push(Found {
                    kind: ItemKind::Impl,
                    name: None,
                    node: Node::Impl(it),
                    offset,
                });
                push_methods(members, &mut found);
                continue;
            }
            ast::Item::Const(it) => (ItemKind::Const, name_of(&it), Node::Const(it)),
            ast::Item::Static(it) => (ItemKind::Static, name_of(&it), Node::Static(it)),
            ast::Item::TypeAlias(it) => (ItemKind::TypeAlias, name_of(&it), Node::TypeAlias(it)),
            ast::Item::Module(it) => {
                let body = it.item_list();
                found.push(Found {
                    kind: ItemKind::Module,
                    name: name_of(&it),
                    node: Node::Module(it),
                    offset,
                });
                if let Some(body) = body {
                    stack.push(body.items());
                }
                continue;
            }
            ast::Item::MacroRules(it) => (
                ItemKind::MacroRules,
                name_of(&it),
                Node::Macro(ast::Macro::MacroRules(it)),
            ),
            ast::Item::MacroDef(it) => (
                ItemKind::MacroDef,
                name_of(&it),
                Node::Macro(ast::Macro::MacroDef(it)),
            ),
            _ => continue,
        };
        found.push(Found {
            kind,
            node,
            name,
            offset,
        });
    }
    found
}

fn push_methods(members: Option<ast::AssocItemList>, found: &mut Vec<Found>) {
    let Some(members) = members else {
        return;
    };
    for member in members.assoc_items() {
        if let ast::AssocItem::Fn(it) = member {
            found.push(Found {
                kind: ItemKind::Method,
                offset: u32::from(it.syntax().text_range().start()),
                name: name_of(&it),
                node: Node::Fn(it),
            });
        }
    }
}

fn name_of(node: &impl HasName) -> Option<String> {
    node.name().map(|name| name.text().to_string())
}

fn resolve(sema: &Semantics<'_, RootDatabase>, db: &RootDatabase, node: &Node) -> MonikerStatus {
    let definition = match node {
        Node::Fn(it) => sema.to_def(it).map(Definition::Function),
        Node::Adt(it) => sema.to_def(it).map(Definition::Adt),
        Node::Trait(it) => sema.to_def(it).map(Definition::Trait),
        Node::Impl(it) => sema.to_def(it).map(Definition::SelfType),
        Node::Const(it) => sema.to_def(it).map(Definition::Const),
        Node::Static(it) => sema.to_def(it).map(Definition::Static),
        Node::TypeAlias(it) => sema.to_def(it).map(Definition::TypeAlias),
        Node::Module(it) => sema.to_def(it).map(Definition::Module),
        Node::Macro(it) => sema.to_def(it).map(Definition::Macro),
    };
    let Some(definition) = definition else {
        return MonikerStatus::Unresolved;
    };
    let Some(krate) = definition.krate(db) else {
        return MonikerStatus::NoMoniker;
    };
    match MonikerResult::from_def(db, definition, krate) {
        Some(MonikerResult::Moniker(moniker)) => {
            MonikerStatus::Moniker(render(&moniker.identifier))
        }
        Some(MonikerResult::Local { .. }) => MonikerStatus::Local,
        None => MonikerStatus::NoMoniker,
    }
}

/// `crate descriptors`, SCIP-style suffixes per descriptor kind (the
/// `Display` of [`MonikerIdentifier`] drops the kinds, so a module and a
/// function of one name would print alike).
fn render(identifier: &MonikerIdentifier) -> String {
    let mut out = identifier.crate_name.clone();
    out.push(' ');
    for descriptor in &identifier.description {
        let name = descriptor.name.as_str();
        match descriptor.desc {
            MonikerDescriptorKind::Namespace => {
                out.push_str(name);
                out.push('/');
            }
            MonikerDescriptorKind::Type => {
                out.push_str(name);
                out.push('#');
            }
            MonikerDescriptorKind::Term => {
                out.push_str(name);
                out.push('.');
            }
            MonikerDescriptorKind::Method => {
                out.push_str(name);
                out.push_str("().");
            }
            MonikerDescriptorKind::TypeParameter => {
                out.push('[');
                out.push_str(name);
                out.push(']');
            }
            MonikerDescriptorKind::Parameter => {
                out.push('(');
                out.push_str(name);
                out.push(')');
            }
            MonikerDescriptorKind::Macro => {
                out.push_str(name);
                out.push('!');
            }
            MonikerDescriptorKind::Meta => {
                out.push_str(name);
                out.push(':');
            }
        }
    }
    out
}

/// Byte offset → 1-based line, from one newline index per file.
struct Lines {
    newlines: Vec<u32>,
}

impl Lines {
    fn new(text: &str) -> Lines {
        let newlines = text
            .bytes()
            .enumerate()
            .filter(|&(_, byte)| byte == b'\n')
            .map(|(offset, _)| u32::try_from(offset).unwrap_or(u32::MAX))
            .collect();
        Lines { newlines }
    }

    fn line(&self, offset: u32) -> usize {
        self.newlines.partition_point(|&newline| newline < offset) + 1
    }
}
