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

use tree_sitter::{Node, Tree};

use crate::grammar::RustParser;

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
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
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
    pub fn as_str(self) -> &'static str {
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
    #[must_use]
    pub fn as_str(self) -> &'static str {
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
}

/// The three registration methods the detector reads.
pub(crate) const ADD_SYSTEMS: &str = "add_systems";
pub(crate) const ADD_OBSERVER: &str = "add_observer";
pub(crate) const ADD_PLUGINS: &str = "add_plugins";

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
            }
        }
        "impl_item" => {
            if let Some(plugin) = impl_plugin(node, source) {
                out.plugins.push(plugin);
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
                });
            }
        }
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
    })
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
/// plus [`TRUNCATION_MARK`]. The range is never validated whole: it is read
/// [`MAX_TEXT_BYTES`] bytes at a time, each window cut back to a character
/// boundary and validated alone, until the cap is reached — a large node
/// costs what its kept text costs. Invalid UTF-8 ends the text.
pub(crate) fn compact_span(source: &[u8], start: usize, end: usize) -> String {
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
        if !push_compact(&mut out, window) {
            return out;
        }
        at = cut;
    }
    out
}

/// Appends `text` without whitespace to `out` up to [`MAX_TEXT_BYTES`];
/// `false` once the cap is hit and [`TRUNCATION_MARK`] appended.
fn push_compact(out: &mut String, text: &str) -> bool {
    for c in text.chars().filter(|c| !c.is_whitespace()) {
        if out.len() + c.len_utf8() > MAX_TEXT_BYTES {
            out.push(TRUNCATION_MARK);
            return false;
        }
        out.push(c);
    }
    true
}
