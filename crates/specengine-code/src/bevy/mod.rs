//! The syntactic Bevy registration detector (05 §5.1 layer A, "Bevy detector").
//!
//! One pass over a parsed file finds:
//!
//! - **systems** of `.add_systems(Schedule, …)` (and of the one-argument
//!   `Schedule::add_systems(…)`, schedule unknown): tuples nest to any depth,
//!   the combinators of [`COMBINATORS`] are peeled off, the adapters of
//!   [`ADAPTERS`] are recorded; a leaf is a path (`tick`, `a::b`, `f::<T>`),
//!   a closure (named by the `fn` it is written in) or a call returning a
//!   system (a factory, named by its callee);
//! - **observers** of `.add_observer(…)`, read like a system leaf;
//! - **plugins**: every `impl Plugin for X` and every function whose only
//!   parameter is `&mut App` (`fn(&mut App)` is a plugin in Bevy);
//! - **plugin uses** of `.add_plugins(…)`;
//! - the same calls inside `macro_rules!` transcribers and macro invocation
//!   arguments, read from tokens: [`Origin::MacroRules`] and
//!   [`Origin::MacroCall`] carry names only and are never resolved.
//!
//! For the framework roles of the code index (`crate::frameworks`,
//! `docs/canon/code-roles.md` "Role facts") it also reads, apart from the
//! lists above (which stay as they were):
//!
//! - **derives**: each `#[derive(..)]` element of a struct, enum or union
//!   whose last path segment is one of [`DeriveRole`] (several attributes,
//!   qualified paths), also inside `#[cfg_attr(<p>, derive(..))]`
//!   ([`Derive::cfg_attr`]); one with a parse error by name only
//!   ([`Derive::parse_error`]); a derive in a macro body is not read;
//! - **trait impls**: `impl Component for T` and `impl Plugin for T`
//!   ([`TraitImpl`]), in code and in macro tokens as `impl Plugin for` is;
//! - **messages**: `.add_message::<T>()` ([`MessageUse`]), read as
//!   `add_observer` is: code exactly, macro tokens by name only;
//! - **subjects** ([`Subject`]): what each registration, `pipe` argument
//!   ([`Piped`]), plugin use, impl and message names, as written.
//!
//! What the detector cannot read is an [`Uncertain`] with a category, never a
//! guess. Everything is text as written: resolution to definitions is
//! layer B (the schedule dump) or layer C (rust-analyzer).
//!
//! **Cost.** The file is walked once with a tree cursor; each registration's
//! argument subtree once more with an explicit work stack (no recursion over
//! user input); a macro token tree is flattened once and scanned linearly.
//! Tuple nesting past [`MAX_NESTING`] is reported as
//! [`UncertainCategory::NestingTooDeep`] and skipped. Texts are capped at
//! [`MAX_TEXT_BYTES`]. Output is in source order: one input, one result.

mod expr;
mod tokens;

use std::ops::Range;

use tree_sitter::{Node, Tree};

use crate::grammar::RustParser;
use crate::hash::{attached_attributes, is_comment};

/// Configuration combinators of `IntoScheduleConfigs` (Bevy 0.19): peeled off
/// a system expression, their arguments are sets or conditions, not systems.
pub const COMBINATORS: &[&str] = &[
    "in_set",
    "before",
    "after",
    "before_ignore_deferred",
    "after_ignore_deferred",
    "run_if",
    "distributive_run_if",
    "ambiguous_with",
    "ambiguous_with_all",
    "chain",
    "chain_ignore_deferred",
    "into_configs",
];

/// System adapters of `IntoSystem` (Bevy 0.19): recorded on the registration;
/// the argument of `pipe` is the piped system.
pub const ADAPTERS: &[&str] = &["pipe", "map", "with_input", "with_input_from"];

/// Tuple and parenthesis nesting read per registration; deeper is
/// [`UncertainCategory::NestingTooDeep`].
pub const MAX_NESTING: usize = 256;

/// Bytes kept of a schedule label, path or other text (whitespace removed);
/// longer texts end with [`TRUNCATION_MARK`].
pub const MAX_TEXT_BYTES: usize = 128;

/// Appended to a text cut at [`MAX_TEXT_BYTES`].
pub const TRUNCATION_MARK: char = '…';

/// What a registration adds to the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    /// An argument of `add_systems`.
    System,
    /// The argument of `add_observer`.
    Observer,
}

impl Target {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Observer => "observer",
        }
    }
}

/// Where the detector read a registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// The syntax tree of ordinary code.
    Code,
    /// Tokens of a `macro_rules!` transcriber: names only, never resolved.
    MacroRules,
    /// Tokens of a macro invocation's arguments: names only, never resolved.
    MacroCall,
}

impl Origin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::MacroRules => "macro_rules",
            Self::MacroCall => "macro_call",
        }
    }
}

/// The shape of a system or observer leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    /// A path: `tick`, `systems::tick`, `Self::apply`, `tick::<T>`.
    Path,
    /// A closure; it has no name, [`Registration::enclosing_fn`] names the
    /// function it is written in (Bevy names it `…::that_fn::{{closure}}`).
    Closure,
    /// A call that returns a system (`make_system(3)`); the name is the callee.
    Factory,
}

impl Form {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Closure => "closure",
            Self::Factory => "factory",
        }
    }
}

/// One system or observer registration, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub target: Target,
    pub origin: Origin,
    pub form: Form,
    /// Schedule label text without whitespace (capped); `None` for observers
    /// and for the one-argument `add_systems` of a `Schedule`.
    pub schedule: Option<String>,
    /// The leaf's text without whitespace (capped); `closure` for closures.
    pub text: String,
    /// Last path segment without generic arguments; `None` for closures.
    pub name: Option<String>,
    /// The innermost `fn` around the registration (code only).
    pub enclosing_fn: Option<String>,
    /// Adapters in source order (`pipe`, `map`, `with_input`, `with_input_from`).
    pub adapters: Vec<&'static str>,
    /// Names of the systems piped into by `pipe`, in source order.
    pub piped: Vec<String>,
    /// 1-based line of the leaf.
    pub line: usize,
    /// What a path or a factory names ([`Subject`]); `None` for a closure
    /// and for a path the detector cannot name. Macro tokens: the name only.
    pub subject: Option<Subject>,
    /// The leaf's first byte (a closure's place among the file's items).
    pub start: usize,
    /// Each `pipe` argument read as a system, source order.
    pub pipes: Vec<Piped>,
}

/// What a path or a type names, as written (`crate::frameworks`' role
/// subject): the path's segments joined by `::`, generic arguments dropped
/// (`tick::<T>` → `tick`), each leading `crate`, `self` or `super` segment
/// followed by another dropped (no qualified path holds them), capped at
/// [`MAX_TEXT_BYTES`] plus [`TRUNCATION_MARK`]; a qualifier that is no
/// plain name named as `crate::items`' impl label names an impl: a
/// qualified type each side compacted, joined by ` as ` (`<Foo<A, B> as
/// Tr>::f` → `<Foo<A,B> as Tr>::f`, so it meets the impl's item's
/// suffix), any other kept as written, each whitespace run one space
/// ([`qualifier_text`]). From macro tokens: the last segment alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    pub text: String,
    /// The last segment (never cut).
    pub name: String,
}

/// One argument of `.pipe(…)`, read as a system leaf is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piped {
    /// Its form and subject (a closure has none), or why it cannot be read.
    pub read: Result<(Form, Option<Subject>), UncertainCategory>,
    /// Its first byte.
    pub start: usize,
    pub line: usize,
}

/// A role a `#[derive(..)]` element gives its item: the Bevy 0.19 derive
/// macro its last path segment names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeriveRole {
    Component,
    Resource,
    Message,
    Event,
    EntityEvent,
    Reflect,
}

impl DeriveRole {
    /// Every derive read.
    pub const ALL: [Self; 6] = [
        Self::Component,
        Self::Resource,
        Self::Message,
        Self::Event,
        Self::EntityEvent,
        Self::Reflect,
    ];

    /// The derive macro's name, as an element's last segment holds it.
    #[must_use]
    pub const fn macro_name(self) -> &'static str {
        match self {
            Self::Component => "Component",
            Self::Resource => "Resource",
            Self::Message => "Message",
            Self::Event => "Event",
            Self::EntityEvent => "EntityEvent",
            Self::Reflect => "Reflect",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.macro_name() == name)
    }
}

/// One role derive of a struct, enum or union in code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derive {
    pub role: DeriveRole,
    /// Read inside `#[cfg_attr(<p>, derive(..))]`: it holds only where `<p>`
    /// does.
    pub cfg_attr: bool,
    /// Read from an attribute with a parse error, by name only: the
    /// attribute may not say what it seems to.
    pub parse_error: bool,
    /// The byte range of the item's node (attributes excluded), as
    /// [`crate::ItemRecord::byte_range`] holds it.
    pub item: Range<usize>,
    /// The line of the element's last segment.
    pub line: usize,
    /// The first byte of the element's last segment (its source order).
    pub start: usize,
}

/// The trait of a [`TraitImpl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImplTrait {
    /// `impl Component for T`.
    Component,
    /// `impl Plugin for T` (also in [`BevyAnalysis::plugins`]).
    Plugin,
}

/// `impl Component for T` or `impl Plugin for T`: the trait's role on `T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImpl {
    pub implemented: ImplTrait,
    pub origin: Origin,
    /// `T` without generic arguments, or why it cannot be named.
    pub subject: Result<Subject, UncertainCategory>,
    pub line: usize,
    /// The first byte of the `impl` (code) or of the trait's name (tokens).
    pub start: usize,
}

/// One `.add_message::<T>()` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageUse {
    pub origin: Origin,
    /// `T` without generic arguments, or why the call cannot be read.
    pub subject: Result<Subject, UncertainCategory>,
    /// The method name's line.
    pub line: usize,
    /// The method name's first byte.
    pub start: usize,
}

/// How a plugin is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginKind {
    /// `impl Plugin for X`.
    ImplPlugin,
    /// A function whose only parameter is `&mut App`.
    FnApp,
}

impl PluginKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImplPlugin => "impl_plugin",
            Self::FnApp => "fn_app",
        }
    }
}

/// One plugin definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDef {
    pub kind: PluginKind,
    pub origin: Origin,
    /// The type's or function's name without generic arguments; `None` when
    /// a macro transcriber names it by a metavariable.
    pub name: Option<String>,
    pub line: usize,
    /// The byte range of the `impl` or `fn` node in code (none from macro
    /// tokens): the item a [`PluginKind::FnApp`] role sits on.
    pub item: Option<Range<usize>>,
    /// The first byte it was read at (its source order).
    pub start: usize,
}

/// One argument leaf of `add_plugins`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginUse {
    pub origin: Origin,
    /// The plugin's type or function name when readable (`DefaultPlugins`
    /// for `DefaultPlugins.set(…)`, `P` for `P::default()` or `P { … }`).
    pub name: Option<String>,
    /// The leaf's text without whitespace (capped).
    pub text: String,
    pub line: usize,
    /// What the leaf names, the same path as [`Self::name`] whole
    /// ([`Subject`]); `None` when it has no name.
    pub subject: Option<Subject>,
    /// [`Form::Factory`] when the subject is a callee that is no type
    /// (`make_plugin()`), else [`Form::Path`]; `None` without a subject.
    pub form: Option<Form>,
    /// The leaf's first byte.
    pub start: usize,
}

/// Why a construct could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UncertainCategory {
    /// A macro invocation in a system position: `add_systems(Update, my_systems!())`.
    MacroInArguments,
    /// A `$name` metavariable in a system position of a macro transcriber.
    Metavariable,
    /// A method call on a system that is neither a combinator nor an adapter.
    UnknownMethod,
    /// Any other expression in a system position (a field, a block, an `if`…).
    Expression,
    /// A registration call whose arguments are not the expected shape.
    Arguments,
    /// Tuple or parenthesis nesting past [`MAX_NESTING`].
    NestingTooDeep,
    /// The registration call contains a parse error; it is not read.
    ParseError,
}

impl UncertainCategory {
    /// Every category.
    pub const ALL: [Self; 7] = [
        Self::MacroInArguments,
        Self::Metavariable,
        Self::UnknownMethod,
        Self::Expression,
        Self::Arguments,
        Self::NestingTooDeep,
        Self::ParseError,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MacroInArguments => "macro_in_arguments",
            Self::Metavariable => "metavariable",
            Self::UnknownMethod => "unknown_method",
            Self::Expression => "expression",
            Self::Arguments => "arguments",
            Self::NestingTooDeep => "nesting_too_deep",
            Self::ParseError => "parse_error",
        }
    }
}

/// A construct the detector saw and could not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uncertain {
    pub category: UncertainCategory,
    pub origin: Origin,
    /// `add_systems`, `add_observer` or `add_plugins`.
    pub call: &'static str,
    /// The construct's text without whitespace (capped).
    pub text: String,
    pub line: usize,
    /// The construct's first byte.
    pub start: usize,
}

/// Registration call sites per method.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallSites {
    pub add_systems: usize,
    pub add_observer: usize,
    pub add_plugins: usize,
    /// Of the above, those read from macro tokens.
    pub in_macros: usize,
}

/// Everything the detector found in one file, in source order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BevyAnalysis {
    pub registrations: Vec<Registration>,
    pub plugins: Vec<PluginDef>,
    pub plugin_uses: Vec<PluginUse>,
    pub uncertain: Vec<Uncertain>,
    pub call_sites: CallSites,
    /// The file has a parse error somewhere.
    pub has_error: bool,
    /// Role derives of the file's structs, enums and unions, source order.
    pub derives: Vec<Derive>,
    /// `impl Component for` and `impl Plugin for`, by line.
    pub impls: Vec<TraitImpl>,
    /// `.add_message::<T>()` calls, by line (counted in no [`CallSites`]).
    pub messages: Vec<MessageUse>,
}

/// The three registration methods the detector reads.
pub(crate) const ADD_SYSTEMS: &str = "add_systems";
pub(crate) const ADD_OBSERVER: &str = "add_observer";
pub(crate) const ADD_PLUGINS: &str = "add_plugins";
/// The message registration, read for the roles only.
pub(crate) const ADD_MESSAGE: &str = "add_message";

/// The traits of [`ImplTrait`], by the last segment of the impl's trait.
pub(crate) const COMPONENT_TRAIT: &str = "Component";
pub(crate) const PLUGIN_TRAIT: &str = "Plugin";

/// The attributes a derive is read from.
const DERIVE_ATTRIBUTE: &str = "derive";
const CFG_ATTR_ATTRIBUTE: &str = "cfg_attr";

/// Parses and analyses one file. `None` when the parse was cancelled.
pub fn detect_file(parser: &mut RustParser, source: &str) -> Option<BevyAnalysis> {
    let tree = parser.parse(source)?;
    Some(detect(&tree, source))
}

/// Registrations, plugins and uncertain constructs of an already parsed file.
#[must_use]
pub fn detect(tree: &Tree, source: &str) -> BevyAnalysis {
    let bytes = source.as_bytes();
    let root = tree.root_node();
    let mut out = BevyAnalysis {
        has_error: root.has_error(),
        ..BevyAnalysis::default()
    };
    let mut cursor = root.walk();
    let mut depth = 0usize;
    // Open `fn` items: (depth, name). A closure is named after the innermost.
    let mut functions: Vec<(usize, String)> = Vec::new();
    loop {
        let node = cursor.node();
        let enclosing = functions.last().map(|(_, name)| name.as_str());
        visit(node, bytes, enclosing, &mut out);
        if node.kind() == "function_item" {
            let name = node
                .child_by_field_name("name")
                .map(|n| compact_node(n, bytes))
                .unwrap_or_default();
            functions.push((depth, name));
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                // Pre-order visits a chained call before the calls of its
                // receiver; a stable sort restores source order.
                out.registrations.sort_by_key(|r| r.line);
                out.plugins.sort_by_key(|p| p.line);
                out.plugin_uses.sort_by_key(|u| u.line);
                out.uncertain.sort_by_key(|u| u.line);
                out.derives.sort_by_key(|d| d.line);
                out.impls.sort_by_key(|i| i.line);
                out.messages.sort_by_key(|m| m.line);
                return out;
            }
            depth -= 1;
        }
        while functions.last().is_some_and(|(d, _)| *d >= depth) {
            functions.pop();
        }
    }
}

fn visit(node: Node, source: &[u8], enclosing: Option<&str>, out: &mut BevyAnalysis) {
    match node.kind() {
        "call_expression" => {
            if let Some((method, field)) = registration_method(node, source) {
                expr::registration(node, field, method, source, enclosing, out);
            } else if let Some(field) = message_method(node, source) {
                expr::message(node, field, source, out);
            }
        }
        "impl_item" => {
            if let Some(plugin) = impl_plugin(node, source) {
                out.plugins.push(plugin);
            }
            if let Some(implemented) = trait_impl(node, source) {
                out.impls.push(implemented);
            }
        }
        "function_item" => {
            if is_app_function(node, source) {
                out.plugins.push(PluginDef {
                    kind: PluginKind::FnApp,
                    origin: Origin::Code,
                    name: node
                        .child_by_field_name("name")
                        .map(|n| compact_node(n, source)),
                    line: line(node),
                    item: Some(node.byte_range()),
                    start: node.start_byte(),
                });
            }
        }
        "struct_item" | "enum_item" | "union_item" => derives(node, source, out),
        "macro_definition" => {
            let mut cursor = node.walk();
            for rule in node.named_children(&mut cursor) {
                if rule.kind() == "macro_rule"
                    && let Some(right) = rule.child_by_field_name("right")
                {
                    tokens::scan(right, source, Origin::MacroRules, None, out);
                }
            }
        }
        "macro_invocation" => {
            let mut cursor = node.walk();
            let tree = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "token_tree");
            if let Some(tree) = tree {
                tokens::scan(tree, source, Origin::MacroCall, enclosing, out);
            }
        }
        _ => {}
    }
}

/// `add_systems`, `add_observer` or `add_plugins` when `call` is a method
/// call of one of them (`x.add_systems(…)`, also with a turbofish), with
/// the method name node.
fn registration_method<'tree>(
    call: Node<'tree>,
    source: &[u8],
) -> Option<(&'static str, Node<'tree>)> {
    let mut function = call.child_by_field_name("function")?;
    if function.kind() == "generic_function" {
        function = function.child_by_field_name("function")?;
    }
    if function.kind() != "field_expression" {
        return None;
    }
    let field = function.child_by_field_name("field")?;
    let method = match text(field, source) {
        ADD_SYSTEMS => ADD_SYSTEMS,
        ADD_OBSERVER => ADD_OBSERVER,
        ADD_PLUGINS => ADD_PLUGINS,
        _ => return None,
    };
    Some((method, field))
}

/// `impl Plugin for X` (also `impl bevy::app::Plugin for X<T>`).
fn impl_plugin(node: Node, source: &[u8]) -> Option<PluginDef> {
    let trait_node = node.child_by_field_name("trait")?;
    if type_name(trait_node, source)? != "Plugin" {
        return None;
    }
    let ty = node.child_by_field_name("type")?;
    Some(PluginDef {
        kind: PluginKind::ImplPlugin,
        origin: Origin::Code,
        name: type_name(ty, source),
        line: line(node),
        item: Some(node.byte_range()),
        start: node.start_byte(),
    })
}

/// `impl Component for T` or `impl Plugin for T` (any path ending in the
/// trait's name, generic arguments dropped), `T`'s subject.
fn trait_impl(node: Node, source: &[u8]) -> Option<TraitImpl> {
    let trait_node = node.child_by_field_name("trait")?;
    let implemented = match type_name(trait_node, source)?.as_str() {
        COMPONENT_TRAIT => ImplTrait::Component,
        PLUGIN_TRAIT => ImplTrait::Plugin,
        _ => return None,
    };
    let subject = node
        .child_by_field_name("type")
        .and_then(|ty| path_subject(ty, source))
        .ok_or(UncertainCategory::Expression);
    Some(TraitImpl {
        implemented,
        origin: Origin::Code,
        subject,
        line: line(node),
        start: node.start_byte(),
    })
}

/// The method name node of `x.add_message(…)` (also with a turbofish).
fn message_method<'tree>(call: Node<'tree>, source: &[u8]) -> Option<Node<'tree>> {
    let mut function = call.child_by_field_name("function")?;
    if function.kind() == "generic_function" {
        function = function.child_by_field_name("function")?;
    }
    if function.kind() != "field_expression" {
        return None;
    }
    let field = function.child_by_field_name("field")?;
    (text(field, source) == ADD_MESSAGE).then_some(field)
}

/// The role derives on `item` (a struct, enum or union): every element of
/// each attached `#[derive(..)]`, and of each `derive(..)` inside an
/// attached `#[cfg_attr(..)]` (nested `cfg_attr` too, on a work stack),
/// whose last path segment is a [`DeriveRole`]. An attribute with a parse
/// error is read by name only ([`broken_derives`]).
fn derives(item: Node, source: &[u8], out: &mut BevyAnalysis) {
    for attribute_item in attached_attributes(item) {
        if attribute_item.has_error() {
            broken_derives(item, attribute_item, source, out);
            continue;
        }
        let mut cursor = attribute_item.walk();
        let Some(attribute) = attribute_item
            .named_children(&mut cursor)
            .find(|child| child.kind() == "attribute")
        else {
            continue;
        };
        let mut cursor = attribute.walk();
        let path = attribute
            .named_children(&mut cursor)
            .find(|child| !is_comment(*child));
        let (Some(path), Some(arguments)) = (path, attribute.child_by_field_name("arguments"))
        else {
            continue;
        };
        let first = match type_name(path, source).as_deref() {
            Some(DERIVE_ATTRIBUTE) => (arguments, AttributeTree::Derive, false),
            Some(CFG_ATTR_ATTRIBUTE) => (arguments, AttributeTree::CfgAttr, true),
            _ => continue,
        };
        // (token tree, how it reads, inside a `cfg_attr`): popped in source
        // order, as the nested trees are pushed reversed.
        let mut stack = vec![first];
        while let Some((tree, reads, cfg_attr)) = stack.pop() {
            let elements = token_elements(tree);
            match reads {
                AttributeTree::Derive => {
                    for element in &elements {
                        let Some(last) = element_path_end(element) else {
                            continue;
                        };
                        if let Some(role) = DeriveRole::named(text(last, source)) {
                            out.derives.push(Derive {
                                role,
                                cfg_attr,
                                parse_error: false,
                                item: item.byte_range(),
                                line: line(last),
                                start: last.start_byte(),
                            });
                        }
                    }
                }
                AttributeTree::CfgAttr => {
                    // The first element is the predicate; each other one an
                    // attribute.
                    let mut nested = Vec::new();
                    for element in elements.iter().skip(1) {
                        let [name, inner] = element.as_slice() else {
                            continue;
                        };
                        if name.kind() != "identifier" || inner.kind() != "token_tree" {
                            continue;
                        }
                        match text(*name, source) {
                            DERIVE_ATTRIBUTE => nested.push((*inner, AttributeTree::Derive, true)),
                            CFG_ATTR_ATTRIBUTE => {
                                nested.push((*inner, AttributeTree::CfgAttr, true));
                            }
                            _ => {}
                        }
                    }
                    stack.extend(nested.into_iter().rev());
                }
            }
        }
    }
}

/// The role derives of an attached attribute with a parse error, whose
/// element structure cannot be trusted: its identifiers in source order
/// (inside `ERROR` nodes too). When the first is `derive`, or `cfg_attr`
/// with a `derive` later, each identifier after that `derive` naming a
/// [`DeriveRole`] is a [`Derive::parse_error`] derive. Any other attribute,
/// or one naming no role, gives none.
fn broken_derives(item: Node, attribute_item: Node, source: &[u8], out: &mut BevyAnalysis) {
    let names = identifiers(attribute_item);
    let Some((first, rest)) = names.split_first() else {
        return;
    };
    let (cfg_attr, roles) = match text(*first, source) {
        DERIVE_ATTRIBUTE => (false, rest),
        CFG_ATTR_ATTRIBUTE => {
            let Some(at) = rest
                .iter()
                .position(|name| text(*name, source) == DERIVE_ATTRIBUTE)
            else {
                return;
            };
            (true, &rest[at + 1..])
        }
        _ => return,
    };
    for name in roles {
        if let Some(role) = DeriveRole::named(text(*name, source)) {
            out.derives.push(Derive {
                role,
                cfg_attr,
                parse_error: true,
                item: item.byte_range(),
                line: line(*name),
                start: name.start_byte(),
            });
        }
    }
}

/// Every `identifier` node under `scope`, in source order (a cursor walk,
/// no recursion); `MISSING` nodes skipped.
fn identifiers(scope: Node) -> Vec<Node> {
    let mut found = Vec::new();
    let root = scope.id();
    let mut cursor = scope.walk();
    loop {
        let current = cursor.node();
        if current.kind() == "identifier" && !current.is_missing() {
            found.push(current);
        }
        if cursor.goto_first_child() {
            continue;
        }
        if cursor.node().id() == root {
            return found;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root {
                return found;
            }
        }
    }
}

/// How the token tree of an attribute reads.
#[derive(Clone, Copy)]
enum AttributeTree {
    /// `derive(..)`: a list of paths.
    Derive,
    /// `cfg_attr(<p>, ..)`: a predicate, then attributes.
    CfgAttr,
}

/// The elements of a token tree: its tokens between the delimiters, split
/// at the commas of this level (a nested tree is one token), comments
/// dropped, empty elements dropped.
fn token_elements(tree: Node) -> Vec<Vec<Node>> {
    let mut cursor = tree.walk();
    let children: Vec<Node> = tree.children(&mut cursor).collect();
    let inner = match children.as_slice() {
        [open, inner @ .., close]
            if matches!(open.kind(), "(" | "[" | "{")
                && matches!(close.kind(), ")" | "]" | "}") =>
        {
            inner
        }
        _ => children.as_slice(),
    };
    let mut elements = Vec::new();
    let mut current = Vec::new();
    for child in inner {
        if is_comment(*child) {
            continue;
        }
        if !child.is_named() && child.kind() == "," {
            if !current.is_empty() {
                elements.push(std::mem::take(&mut current));
            }
        } else {
            current.push(*child);
        }
    }
    if !current.is_empty() {
        elements.push(current);
    }
    elements
}

/// The last identifier of an element that is a path (`C`, `b::C`,
/// `::b::C`, `crate::C`); `None` for any other element.
fn element_path_end<'tree>(element: &[Node<'tree>]) -> Option<Node<'tree>> {
    let segments = match element {
        [first, rest @ ..] if first.kind() == "::" => rest,
        _ => element,
    };
    // Segments at even places, separators at odd ones, a segment last.
    if segments.len() % 2 == 0 {
        return None;
    }
    for (at, token) in segments.iter().enumerate() {
        let fits = if at % 2 == 0 {
            matches!(token.kind(), "identifier" | "crate" | "self" | "super")
        } else {
            token.kind() == "::"
        };
        if !fits {
            return None;
        }
    }
    segments
        .last()
        .filter(|last| last.kind() == "identifier")
        .copied()
}

/// A function whose parameter list is exactly one non-`self` parameter of
/// type `&mut App` (any path ending in `App`) and which returns nothing.
fn is_app_function(node: Node, source: &[u8]) -> bool {
    if let Some(ret) = node.child_by_field_name("return_type")
        && ret.kind() != "unit_type"
    {
        return false;
    }
    let Some(parameters) = node.child_by_field_name("parameters") else {
        return false;
    };
    let mut cursor = parameters.walk();
    let mut params = parameters
        .named_children(&mut cursor)
        .filter(|p| !crate::hash::is_comment(*p) && p.kind() != "attribute_item");
    let (Some(param), None) = (params.next(), params.next()) else {
        return false;
    };
    if param.kind() != "parameter" {
        return false;
    }
    let Some(ty) = param.child_by_field_name("type") else {
        return false;
    };
    if ty.kind() != "reference_type" {
        return false;
    }
    let mut cursor = ty.walk();
    let mutable = ty
        .children(&mut cursor)
        .any(|c| c.kind() == "mutable_specifier");
    mutable
        && ty
            .child_by_field_name("type")
            .and_then(|inner| type_name(inner, source))
            .is_some_and(|name| name == "App")
}

/// Last segment of a type path without generic arguments: `App` for
/// `bevy::app::App`, `P` for `P<T>`.
fn type_name(node: Node, source: &[u8]) -> Option<String> {
    let mut node = node;
    // Each step moves to a strictly smaller child; the loop is bounded by the
    // depth of one type path.
    loop {
        match node.kind() {
            "type_identifier" | "identifier" => return Some(compact_node(node, source)),
            "generic_type" => node = node.child_by_field_name("type")?,
            "scoped_type_identifier" | "scoped_identifier" => {
                node = node.child_by_field_name("name")?;
            }
            _ => return None,
        }
    }
}

/// The [`Subject`] of a path or type node: an `identifier`,
/// `type_identifier`, `self`, `super` or `crate`, a scoped path, a
/// `generic_function` or a generic type (its arguments dropped); `None` for
/// any other node. Each step moves to a strictly smaller child: bounded by
/// the path's depth, no recursion over user input.
pub(crate) fn path_subject(node: Node, source: &[u8]) -> Option<Subject> {
    // From the last segment to the first.
    let mut segments: Vec<String> = Vec::new();
    let mut node = node;
    loop {
        match node.kind() {
            "identifier" | "type_identifier" | "self" | "super" | "crate" => {
                segments.push(compact_node(node, source));
                break;
            }
            "scoped_identifier" | "scoped_type_identifier" => {
                segments.push(compact_node(node.child_by_field_name("name")?, source));
                match node.child_by_field_name("path") {
                    Some(path) => node = path,
                    // `::a`: a global path.
                    None => break,
                }
            }
            "generic_function" => node = node.child_by_field_name("function")?,
            "generic_type" | "generic_type_with_turbofish" => {
                node = node.child_by_field_name("type")?;
            }
            // A qualifier that is no plain path (`<T as Tr>::f`).
            _ if !segments.is_empty() => {
                segments.push(qualifier_text(node, source));
                break;
            }
            _ => return None,
        }
    }
    segments.reverse();
    subject_of(&segments)
}

/// A qualifier that is no plain path as `crate::items`' impl label names
/// the impl it may sit in: a `bracketed_type` holding a `qualified_type`
/// (`<T as Tr>`) as `<` type ` as ` alias `>`, each side compacted (its
/// whitespace dropped); any other node as written, each whitespace run one
/// space. Children found by kind, comments skipped by kind.
fn qualifier_text(node: Node, source: &[u8]) -> String {
    let qualified = (node.kind() == "bracketed_type")
        .then(|| {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| child.kind() == "qualified_type")
        })
        .flatten();
    let sides = qualified.and_then(|qualified| {
        Some((
            qualified.child_by_field_name("type")?,
            qualified.child_by_field_name("alias")?,
        ))
    });
    match sides {
        Some((ty, alias)) => format!(
            "<{} as {}>",
            compact_node(ty, source),
            compact_node(alias, source)
        ),
        None => kept_span(source, node.start_byte(), node.end_byte(), Spacing::Single),
    }
}

/// The leading segments a [`Subject`] drops while another follows.
const DROPPED_LEADS: [&str; 3] = ["crate", "self", "super"];

/// The [`Subject`] of `segments`, first to last.
fn subject_of(segments: &[String]) -> Option<Subject> {
    let (name, before) = segments.split_last()?;
    let lead = before
        .iter()
        .take_while(|segment| DROPPED_LEADS.contains(&segment.as_str()))
        .count();
    let mut text = String::new();
    // A plain segment holds no whitespace; a kept one its single spaces.
    for (at, segment) in segments[lead..].iter().enumerate() {
        if at > 0 && !push_kept(&mut text, "::", Spacing::Single) {
            break;
        }
        if !push_kept(&mut text, segment, Spacing::Single) {
            break;
        }
    }
    Some(Subject {
        text,
        name: name.clone(),
    })
}

/// The [`Subject`] of a name read from macro tokens: the name alone.
pub(crate) fn name_subject(name: &str) -> Subject {
    let mut text = String::new();
    push_compact(&mut text, name);
    Subject {
        text,
        name: name.to_owned(),
    }
}

/// The whole text of a node, validated in full: only for names and other
/// single tokens; a text that is kept goes through [`compact_node`].
pub(crate) fn text<'s>(node: Node, source: &'s [u8]) -> &'s str {
    node.utf8_text(source).unwrap_or_default()
}

pub(crate) fn line(node: Node) -> usize {
    node.start_position().row + 1
}

/// A node's source text compacted, see [`compact_span`].
pub(crate) fn compact_node(node: Node, source: &[u8]) -> String {
    compact_span(source, node.start_byte(), node.end_byte())
}

/// `source[start..end]` without whitespace, capped at [`MAX_TEXT_BYTES`]
/// plus [`TRUNCATION_MARK`] ([`kept_span`]).
pub(crate) fn compact_span(source: &[u8], start: usize, end: usize) -> String {
    kept_span(source, start, end, Spacing::Dropped)
}

/// How a kept text treats whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spacing {
    /// Dropped: a compacted text.
    Dropped,
    /// Each run one space, none leading.
    Single,
}

/// `source[start..end]` with its whitespace as `spacing` says, capped at
/// [`MAX_TEXT_BYTES`] plus [`TRUNCATION_MARK`]. The range is never
/// validated whole: it is read [`MAX_TEXT_BYTES`] bytes at a time, each
/// window cut back to a character boundary and validated alone, until the
/// cap is reached — a large node costs what its kept text costs. Invalid
/// UTF-8 ends the text.
fn kept_span(source: &[u8], start: usize, end: usize, spacing: Spacing) -> String {
    let end = end.min(source.len());
    let mut out = String::new();
    let mut at = start;
    while at < end {
        let mut cut = (at + MAX_TEXT_BYTES).min(end);
        // A UTF-8 continuation byte is `0b10xx_xxxx`.
        while cut > at && cut < end && source[cut] & 0xC0 == 0x80 {
            cut -= 1;
        }
        let Some(window) = (cut > at)
            .then(|| std::str::from_utf8(&source[at..cut]).ok())
            .flatten()
        else {
            return out;
        };
        if !push_kept(&mut out, window, spacing) {
            return out;
        }
        at = cut;
    }
    out
}

/// Appends `text` without whitespace to `out` up to [`MAX_TEXT_BYTES`];
/// `false` once the cap is hit and [`TRUNCATION_MARK`] appended.
fn push_compact(out: &mut String, text: &str) -> bool {
    push_kept(out, text, Spacing::Dropped)
}

/// Appends `text` to `out` up to [`MAX_TEXT_BYTES`], its whitespace as
/// `spacing` says (a run spanning two calls is still one space); `false`
/// once the cap is hit and [`TRUNCATION_MARK`] appended.
fn push_kept(out: &mut String, text: &str, spacing: Spacing) -> bool {
    for c in text.chars() {
        let c = if !c.is_whitespace() {
            c
        } else if spacing == Spacing::Single && !out.is_empty() && !out.ends_with(' ') {
            ' '
        } else {
            continue;
        };
        if out.len() + c.len_utf8() > MAX_TEXT_BYTES {
            out.push(TRUNCATION_MARK);
            return false;
        }
        out.push(c);
    }
    true
}
