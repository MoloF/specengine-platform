//! Registration arguments read from the syntax tree (ordinary code).

use tree_sitter::Node;

use super::{
    ADAPTERS, ADD_OBSERVER, ADD_PLUGINS, ADD_SYSTEMS, BevyAnalysis, COMBINATORS, Form, MAX_NESTING,
    MessageUse, Origin, Piped, PluginUse, Registration, Subject, Target, Uncertain,
    UncertainCategory, compact_node, compact_span, line, path_subject, text,
};
use crate::hash::is_comment;

/// Reads one `.add_message::<T>()` call whose method name node is `field`:
/// `T`'s subject, or why it cannot be read (a parse error in the call's own
/// parts, no turbofish or not one type, an argument, a type that is no
/// path).
pub(super) fn message(call: Node, field: Node, source: &[u8], out: &mut BevyAnalysis) {
    let subject = if own_error(call) {
        Err(UncertainCategory::ParseError)
    } else {
        message_subject(call, source)
    };
    out.messages.push(MessageUse {
        origin: Origin::Code,
        subject,
        line: line(field),
        start: field.start_byte(),
    });
}

fn message_subject(call: Node, source: &[u8]) -> Result<Subject, UncertainCategory> {
    let arguments = call
        .child_by_field_name("arguments")
        .ok_or(UncertainCategory::Arguments)?;
    if !expression_children(arguments).is_empty() {
        return Err(UncertainCategory::Arguments);
    }
    let function = call
        .child_by_field_name("function")
        .filter(|function| function.kind() == "generic_function")
        .ok_or(UncertainCategory::Arguments)?;
    let types = function
        .child_by_field_name("type_arguments")
        .map(expression_children)
        .unwrap_or_default();
    let [ty] = types.as_slice() else {
        return Err(UncertainCategory::Arguments);
    };
    match ty.kind() {
        "type_identifier" | "scoped_type_identifier" | "generic_type" => {
            path_subject(*ty, source).ok_or(UncertainCategory::Expression)
        }
        _ => Err(UncertainCategory::Expression),
    }
}

/// Reads one `add_systems` / `add_observer` / `add_plugins` call whose
/// method name node is `field`.
pub(super) fn registration(
    call: Node,
    field: Node,
    method: &'static str,
    source: &[u8],
    enclosing: Option<&str>,
    out: &mut BevyAnalysis,
) {
    match method {
        ADD_SYSTEMS => out.call_sites.add_systems += 1,
        ADD_OBSERVER => out.call_sites.add_observer += 1,
        _ => out.call_sites.add_plugins += 1,
    }
    // The call's own text and line, from the method name to its closing
    // parenthesis: never the receiver chain written before it.
    let uncertain = |category| Uncertain {
        category,
        origin: Origin::Code,
        call: method,
        text: compact_span(source, field.start_byte(), call.end_byte()),
        line: line(field),
        start: field.start_byte(),
    };
    if own_error(call) {
        out.uncertain.push(uncertain(UncertainCategory::ParseError));
        return;
    }
    let Some(arguments) = call.child_by_field_name("arguments") else {
        out.uncertain.push(uncertain(UncertainCategory::Arguments));
        return;
    };
    let args = expression_children(arguments);
    let context = Context {
        method,
        source,
        enclosing,
    };
    match (method, args.as_slice()) {
        (ADD_SYSTEMS, [schedule, systems]) => {
            let schedule = Some(compact_node(*schedule, source));
            walk(*systems, Target::System, schedule.as_ref(), &context, out);
        }
        (ADD_SYSTEMS, [systems]) => walk(*systems, Target::System, None, &context, out),
        (ADD_OBSERVER, [observer]) => walk(*observer, Target::Observer, None, &context, out),
        (ADD_PLUGINS, [plugins]) => plugin_uses(*plugins, &context, out),
        _ => out.uncertain.push(uncertain(UncertainCategory::Arguments)),
    }
}

/// A parse error in the call's own parts: the method name, a turbofish, the
/// arguments and the tokens between them. The receiver is excluded: in a
/// builder chain it is the previous call, judged on its own, so one broken
/// call does not take the rest of the chain with it.
fn own_error(call: Node) -> bool {
    let mut node = call;
    // call_expression → [generic_function →] field_expression: each step
    // moves to a child, at most three levels.
    loop {
        let (inner, receiver) = match node.kind() {
            "call_expression" | "generic_function" => (node.child_by_field_name("function"), false),
            _ => (node.child_by_field_name("value"), true),
        };
        let mut cursor = node.walk();
        if node
            .children(&mut cursor)
            .any(|child| Some(child) != inner && child.has_error())
        {
            return true;
        }
        match inner {
            Some(inner) if !receiver => node = inner,
            _ => return false,
        }
    }
}

struct Context<'a> {
    method: &'static str,
    source: &'a [u8],
    enclosing: Option<&'a str>,
}

impl Context<'_> {
    fn uncertain(&self, category: UncertainCategory, node: Node) -> Uncertain {
        Uncertain {
            category,
            origin: Origin::Code,
            call: self.method,
            text: compact_node(node, self.source),
            line: line(node),
            start: node.start_byte(),
        }
    }
}

/// Named children that are expressions: comments (by `kind()`) and
/// attributes are not.
fn expression_children(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| !is_comment(*child) && child.kind() != "attribute_item")
        .collect()
}

/// The system expression of one registration: tuples and parentheses on an
/// explicit work stack, each leaf through [`peel`] and [`leaf`].
fn walk(
    root: Node,
    target: Target,
    schedule: Option<&String>,
    context: &Context,
    out: &mut BevyAnalysis,
) {
    // (node, nesting); popped in source order because children are pushed reversed.
    let mut stack = vec![(root, 0usize)];
    while let Some((node, nesting)) = stack.pop() {
        if nesting > MAX_NESTING {
            out.uncertain
                .push(context.uncertain(UncertainCategory::NestingTooDeep, node));
            continue;
        }
        let peeled = match peel(node, context.source) {
            Ok(peeled) => peeled,
            Err(method) => {
                out.uncertain
                    .push(context.uncertain(UncertainCategory::UnknownMethod, method));
                continue;
            }
        };
        match peeled.base.kind() {
            "tuple_expression" | "parenthesized_expression" => {
                for child in expression_children(peeled.base).into_iter().rev() {
                    stack.push((child, nesting + 1));
                }
            }
            _ => match leaf(peeled.base, context) {
                Ok(read) => out.registrations.push(Registration {
                    target,
                    origin: Origin::Code,
                    form: read.form,
                    schedule: schedule.cloned(),
                    text: read.text,
                    name: read.name,
                    enclosing_fn: context.enclosing.map(str::to_owned),
                    adapters: peeled.adapters,
                    piped: peeled
                        .piped
                        .iter()
                        .map(|piped| {
                            path_name(*piped, context.source)
                                .unwrap_or_else(|| compact_node(*piped, context.source))
                        })
                        .collect(),
                    line: line(peeled.base),
                    subject: read.subject,
                    start: peeled.base.start_byte(),
                    pipes: peeled
                        .piped
                        .iter()
                        .map(|piped| Piped {
                            read: leaf(*piped, context).map(|read| (read.form, read.subject)),
                            start: piped.start_byte(),
                            line: line(*piped),
                        })
                        .collect(),
                }),
                Err(category) => out.uncertain.push(context.uncertain(category, peeled.base)),
            },
        }
    }
}

/// A system expression without its combinators and adapters.
struct Peeled<'tree> {
    base: Node<'tree>,
    /// Source order (innermost first).
    adapters: Vec<&'static str>,
    /// Arguments of `pipe`, source order.
    piped: Vec<Node<'tree>>,
}

/// Strips `.combinator(…)` and `.adapter(…)` calls from the outside in; a
/// method chain is as long as it is written, each step moves to a child.
/// `Err` carries the method name node of an unknown method.
fn peel<'tree>(node: Node<'tree>, source: &[u8]) -> Result<Peeled<'tree>, Node<'tree>> {
    let mut base = node;
    let mut adapters = Vec::new();
    let mut piped = Vec::new();
    loop {
        if base.kind() == "parenthesized_expression" {
            let inner = expression_children(base);
            if let [single] = inner.as_slice()
                && single.kind() == "call_expression"
            {
                base = *single;
                continue;
            }
        }
        let Some((receiver, method, arguments)) = method_call(base) else {
            break;
        };
        let name = text(method, source);
        if COMBINATORS.contains(&name) {
            base = receiver;
        } else if let Some(adapter) = ADAPTERS.iter().find(|a| **a == name) {
            adapters.push(*adapter);
            if *adapter == "pipe"
                && let Some(first) =
                    arguments.and_then(|a| expression_children(a).into_iter().next())
            {
                piped.push(first);
            }
            base = receiver;
        } else {
            return Err(method);
        }
    }
    adapters.reverse();
    piped.reverse();
    Ok(Peeled {
        base,
        adapters,
        piped,
    })
}

/// `(receiver, method name node, arguments)` of `receiver.method(…)` or
/// `receiver.method::<T>(…)`.
fn method_call(node: Node) -> Option<(Node, Node, Option<Node>)> {
    if node.kind() != "call_expression" {
        return None;
    }
    let mut function = node.child_by_field_name("function")?;
    if function.kind() == "generic_function" {
        function = function.child_by_field_name("function")?;
    }
    if function.kind() != "field_expression" {
        return None;
    }
    Some((
        function.child_by_field_name("value")?,
        function.child_by_field_name("field")?,
        node.child_by_field_name("arguments"),
    ))
}

/// A leaf read: its form, name, text and subject.
struct LeafRead {
    form: Form,
    name: Option<String>,
    text: String,
    /// `None` for a closure, and for a path [`path_subject`] cannot name.
    subject: Option<Subject>,
}

/// Form, name, text and subject of a leaf that is not a tuple.
fn leaf(node: Node, context: &Context) -> Result<LeafRead, UncertainCategory> {
    let source = context.source;
    match node.kind() {
        "identifier" | "scoped_identifier" | "generic_function" | "self" => Ok(LeafRead {
            form: Form::Path,
            name: path_name(node, source),
            text: compact_node(node, source),
            subject: path_subject(node, source),
        }),
        "closure_expression" => Ok(LeafRead {
            form: Form::Closure,
            name: None,
            text: "closure".to_owned(),
            subject: None,
        }),
        "call_expression" => {
            let function = node
                .child_by_field_name("function")
                .ok_or(UncertainCategory::Expression)?;
            let name = path_name(function, source).ok_or(UncertainCategory::Expression)?;
            Ok(LeafRead {
                form: Form::Factory,
                name: Some(name),
                text: compact_node(function, source),
                subject: path_subject(function, source),
            })
        }
        "macro_invocation" => Err(UncertainCategory::MacroInArguments),
        _ => Err(UncertainCategory::Expression),
    }
}

/// Last segment of a path expression without generic arguments: `tick` for
/// `systems::tick::<T>`; `None` when `node` is not a path.
fn path_name(node: Node, source: &[u8]) -> Option<String> {
    let mut node = node;
    // Each step moves to a child; bounded by the depth of one path.
    loop {
        match node.kind() {
            "identifier" | "self" | "super" | "crate" => return Some(compact_node(node, source)),
            "scoped_identifier" => node = node.child_by_field_name("name")?,
            "generic_function" => node = node.child_by_field_name("function")?,
            _ => return None,
        }
    }
}

/// The argument of `add_plugins`: tuples on a work stack, each leaf with any
/// builder method chain (`.set(…)`, `.build().disable::<T>()`) peeled off.
fn plugin_uses(root: Node, context: &Context, out: &mut BevyAnalysis) {
    let source = context.source;
    let mut stack = vec![(root, 0usize)];
    while let Some((node, nesting)) = stack.pop() {
        if nesting > MAX_NESTING {
            out.uncertain
                .push(context.uncertain(UncertainCategory::NestingTooDeep, node));
            continue;
        }
        let mut base = node;
        while let Some((receiver, _, _)) = method_call(base) {
            base = receiver;
        }
        match base.kind() {
            "tuple_expression" | "parenthesized_expression" => {
                for child in expression_children(base).into_iter().rev() {
                    stack.push((child, nesting + 1));
                }
            }
            _ => {
                let (subject, form) = match plugin_subject(base, source) {
                    Some((subject, form)) => (Some(subject), Some(form)),
                    None => (None, None),
                };
                out.plugin_uses.push(PluginUse {
                    origin: Origin::Code,
                    name: plugin_name(base, source),
                    text: compact_node(base, source),
                    line: line(base),
                    subject,
                    form,
                    start: base.start_byte(),
                });
            }
        }
    }
}

/// A plugin use's subject ([`path_subject`]) and how it is written: `P`,
/// `a::P`, `P { … }`, `P(…)` are [`Form::Path`] (the plugin's type or
/// value); so is a lowercase callee whose qualifier's last segment starts
/// uppercase (`P::default()`, `a::P::new(…)`), its subject the qualifier,
/// the type. Any other lowercase callee (`make_plugin()`,
/// `a::make_plugin()`) is a [`Form::Factory`], its subject the whole
/// callee.
fn plugin_subject(node: Node, source: &[u8]) -> Option<(Subject, Form)> {
    match node.kind() {
        "identifier" | "scoped_identifier" | "generic_function" => {
            Some((path_subject(node, source)?, Form::Path))
        }
        "struct_expression" => {
            let name = node.child_by_field_name("name")?;
            Some((path_subject(name, source)?, Form::Path))
        }
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            let last = path_name(function, source)?;
            if !last.starts_with(|c: char| c.is_ascii_lowercase()) {
                return Some((path_subject(function, source)?, Form::Path));
            }
            match type_qualifier(function, source) {
                Some(qualifier) => Some((qualifier, Form::Path)),
                None => Some((path_subject(function, source)?, Form::Factory)),
            }
        }
        _ => None,
    }
}

/// The qualifier of a callee (`P` of `P::new`, `a::P` of
/// `a::P::default::<T>`) when it is a path whose last segment starts
/// uppercase: the type the callee is called on. `None` for a callee with
/// no qualifier, a lowercase one (a module, `a` of `a::make_plugin`) or
/// one that is no path (`<T as Tr>`).
fn type_qualifier(function: Node, source: &[u8]) -> Option<Subject> {
    let callee = if function.kind() == "generic_function" {
        function.child_by_field_name("function")?
    } else {
        function
    };
    if callee.kind() != "scoped_identifier" {
        return None;
    }
    let qualifier = path_subject(callee.child_by_field_name("path")?, source)?;
    qualifier
        .name
        .starts_with(|c: char| c.is_ascii_uppercase())
        .then_some(qualifier)
}

/// `P` for `P`, `a::P`, `P { … }`, `P(…)`, `P::default()` / `a::P::new(…)`.
fn plugin_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" | "scoped_identifier" | "generic_function" => path_name(node, source),
        "struct_expression" => {
            let name = node.child_by_field_name("name")?;
            super::type_name(name, source).or_else(|| path_name(name, source))
        }
        "call_expression" => {
            let function = node.child_by_field_name("function")?;
            let last = path_name(function, source)?;
            if last.starts_with(|c: char| c.is_ascii_lowercase())
                && function.kind() == "scoped_identifier"
            {
                // `P::new(…)`: the type is the qualifier.
                path_name(function.child_by_field_name("path")?, source)
            } else {
                Some(last)
            }
        }
        _ => None,
    }
}
