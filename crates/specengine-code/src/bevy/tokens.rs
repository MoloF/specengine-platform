//! Registrations inside macro token trees: `macro_rules!` transcribers and
//! macro invocation arguments. tree-sitter keeps them as flat tokens, so the
//! same shapes as in `expr` are read from tokens — names only, never
//! resolved ([`Origin::MacroRules`], [`Origin::MacroCall`]).
//!
//! Linear: the tree is flattened once, delimiter pairs are matched once, and
//! every walk jumps over a group it does not enter through that table.

use tree_sitter::Node;

use super::{
    ADAPTERS, ADD_OBSERVER, ADD_PLUGINS, ADD_SYSTEMS, BevyAnalysis, COMBINATORS, Form, MAX_NESTING,
    Origin, PluginDef, PluginKind, PluginUse, Registration, Target, Uncertain, UncertainCategory,
    compact_span,
};
use crate::hash::is_comment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Open,
    Close,
    Ident,
    /// `$name` of a transcriber, or `$` in a nested definition.
    Meta,
    /// Punctuation, keywords and literals: compared by text.
    Other,
}

#[derive(Debug, Clone, Copy)]
struct Tok<'s> {
    kind: Kind,
    text: &'s str,
    start: usize,
    end: usize,
    line: usize,
}

/// Flattened tokens with matched delimiters.
struct Tokens<'s> {
    toks: Vec<Tok<'s>>,
    /// For an `Open` token, the index of its `Close`; `toks.len()` when unbalanced.
    close: Vec<usize>,
    /// For a `<` token, the index of the `>` / `>>` that closes it; `usize::MAX`
    /// when unclosed or not a `<` ([`angle_closes`]).
    angle_close: Vec<usize>,
    source: &'s [u8],
}

impl<'s> Tokens<'s> {
    fn new(tree: Node, source: &'s [u8]) -> Self {
        let toks = flatten(tree, source);
        let mut close = vec![toks.len(); toks.len()];
        let mut open = Vec::new();
        for (i, tok) in toks.iter().enumerate() {
            match tok.kind {
                Kind::Open => open.push(i),
                Kind::Close => {
                    if let Some(o) = open.pop() {
                        close[o] = i;
                    }
                }
                _ => {}
            }
        }
        let angle_close = angle_closes(&toks);
        Self {
            toks,
            close,
            angle_close,
            source,
        }
    }

    fn text_is(&self, i: usize, text: &str) -> bool {
        self.toks.get(i).is_some_and(|t| t.text == text)
    }

    fn kind_is(&self, i: usize, kind: Kind) -> bool {
        self.toks.get(i).is_some_and(|t| t.kind == kind)
    }

    fn open_paren(&self, i: usize) -> bool {
        self.kind_is(i, Kind::Open) && self.text_is(i, "(")
    }

    /// Source text of tokens `a..b` without whitespace (capped); only the
    /// kept bytes are read ([`compact_span`]).
    fn span_text(&self, a: usize, b: usize) -> String {
        if a >= b || b > self.toks.len() {
            return String::new();
        }
        compact_span(self.source, self.toks[a].start, self.toks[b - 1].end)
    }

    /// Index after the group opened at `i` (or after the token when it opens none).
    fn after_group(&self, i: usize) -> usize {
        if self.kind_is(i, Kind::Open) {
            (self.close[i] + 1).min(self.toks.len())
        } else {
            i + 1
        }
    }

    /// Index after the generic arguments whose `<` (or `<<`) is at `i`, by
    /// the table of [`angle_closes`]. Unclosed before `end`: `end`.
    fn skip_angle(&self, i: usize, end: usize) -> usize {
        match self.angle_close.get(i) {
            Some(&close) if close < end => close + 1,
            _ => end,
        }
    }

    /// A `<` or `<<` at `i`. A token tree lexes `::<<T as Tr>::M>` as `::`
    /// `<<`: the first `<` opens the generic arguments, the second a
    /// qualified path.
    fn angle_open(&self, i: usize) -> bool {
        self.toks
            .get(i)
            .is_some_and(|t| t.kind == Kind::Other && matches!(t.text, "<" | "<<"))
    }

    /// A turbofish at `i`: `::` followed by `<` or `<<`.
    fn turbofish(&self, i: usize) -> bool {
        self.text_is(i, "::") && self.angle_open(i + 1)
    }

    /// Comma-separated elements of `a..b` at this level, empty ones dropped.
    /// The commas of generic arguments separate nothing: a turbofish
    /// (`sys::<A, B>`) and a qualified path opening an element
    /// (`<T as Tr<A, B>>::f`) are skipped whole; an unclosed `<` there is a
    /// plain token.
    fn elements(&self, a: usize, b: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut start = a;
        let mut j = a;
        while j < b {
            if self.toks[j].text == "," && self.toks[j].kind == Kind::Other {
                if start < j {
                    out.push((start, j));
                }
                start = j + 1;
                j += 1;
            } else if self.angle_open(j) && (j == start || self.text_is(j - 1, "::")) {
                j = match self.angle_close[j] {
                    close if close < b => close + 1,
                    _ => j + 1,
                };
            } else {
                j = self.after_group(j).min(b);
            }
        }
        if start < b {
            out.push((start, b));
        }
        out
    }

    /// A path starting at `i`: `a::b::c`, `::a`, `<T as X>::f`, `f::<T>`.
    /// Returns (index after the path, last identifier) or `None` when no
    /// identifier segment starts here.
    fn path(&self, i: usize, end: usize) -> Option<(usize, &'s str)> {
        let mut j = i;
        let mut last = None;
        while j < end {
            let tok = self.toks[j];
            match (tok.kind, tok.text) {
                (Kind::Other, "::") => {
                    if self.angle_open(j + 1) {
                        j = self.skip_angle(j + 1, end);
                    } else {
                        j += 1;
                    }
                }
                (Kind::Other, "<" | "<<") if last.is_none() => j = self.skip_angle(j, end),
                (Kind::Meta, "$crate") if last.is_none() && j == i => j += 1,
                (Kind::Ident, text) if last.is_none() || self.text_is(j - 1, "::") => {
                    last = Some(text);
                    j += 1;
                }
                _ => break,
            }
        }
        last.map(|name| (j, name))
    }
}

/// Pairs every `<` with the `>` / `>>` that brings its depth back to zero,
/// in one pass over the tokens: a stack of pending `<` units per delimiter
/// group, groups inside kept apart (`<<` and `>>` count two). A `<<` is
/// paired through its first, outer unit (`::<<T as Tr>::M>` closes at the
/// last `>`). Generic arguments never leave their group nor cross a `;`, so
/// a group's closer or a `;` drops the units still pending: an unclosed `<`
/// (a comparison) is settled once instead of being rescanned from every
/// later position.
fn angle_closes(toks: &[Tok]) -> Vec<usize> {
    /// The inner unit of `<<`: counted, never looked up.
    const NO_START: usize = usize::MAX;
    let mut closes = vec![usize::MAX; toks.len()];
    // One stack per open group; `frames[0]` is the tree itself. In step
    // with the `open` stack of [`Tokens::new`]: a stray closer pops nothing.
    let mut frames: Vec<Vec<usize>> = vec![Vec::new()];
    for (i, tok) in toks.iter().enumerate() {
        match (tok.kind, tok.text) {
            (Kind::Open, _) => frames.push(Vec::new()),
            (Kind::Close, _) => {
                if frames.len() > 1 {
                    frames.pop();
                } else {
                    frames[0].clear();
                }
            }
            (Kind::Other, text) => {
                let Some(pending) = frames.last_mut() else {
                    continue;
                };
                let pops = match text {
                    ";" => {
                        pending.clear();
                        0
                    }
                    "<" => {
                        pending.push(i);
                        0
                    }
                    "<<" => {
                        pending.extend([i, NO_START]);
                        0
                    }
                    ">" => 1,
                    ">>" => 2,
                    _ => 0,
                };
                for _ in 0..pops {
                    if let Some(start) = pending.pop()
                        && start != NO_START
                    {
                        closes[start] = i;
                    }
                }
            }
            _ => {}
        }
    }
    closes
}

/// Tokens of `tree`: descends only into `token_tree` and `token_repetition`;
/// any other node is one token; comments are dropped by `kind()`.
fn flatten<'s>(tree: Node, source: &'s [u8]) -> Vec<Tok<'s>> {
    let mut toks = Vec::new();
    let mut cursor = tree.walk();
    let mut depth = 0usize;
    loop {
        let node = cursor.node();
        let container = matches!(node.kind(), "token_tree" | "token_repetition");
        if !container && !is_comment(node) {
            let text = node.utf8_text(source).unwrap_or_default();
            let kind = match node.kind() {
                "(" | "[" | "{" => Kind::Open,
                ")" | "]" | "}" => Kind::Close,
                "identifier" | "self" | "super" | "crate" | "primitive_type" => Kind::Ident,
                "metavariable" | "$" => Kind::Meta,
                _ => Kind::Other,
            };
            toks.push(Tok {
                kind,
                text,
                start: node.start_byte(),
                end: node.end_byte(),
                line: node.start_position().row + 1,
            });
        }
        if container && cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if depth == 0 {
                return toks;
            }
            if cursor.goto_next_sibling() {
                break;
            }
            cursor.goto_parent();
            depth -= 1;
        }
    }
}

/// Registrations and plugin definitions inside one macro token tree.
pub(super) fn scan(
    tree: Node,
    source: &[u8],
    origin: Origin,
    enclosing: Option<&str>,
    out: &mut BevyAnalysis,
) {
    let tokens = Tokens::new(tree, source);
    let n = tokens.toks.len();
    for i in 0..n {
        let tok = tokens.toks[i];
        match (tok.kind, tok.text) {
            (Kind::Ident, ADD_SYSTEMS | ADD_OBSERVER | ADD_PLUGINS)
                if tokens.text_is(i.wrapping_sub(1), ".") =>
            {
                call(&tokens, i, origin, enclosing, out);
            }
            (Kind::Ident, "Plugin") if tokens.text_is(i + 1, "for") => {
                let name = match tokens.toks.get(i + 2) {
                    Some(t) if t.kind == Kind::Meta => None,
                    _ => tokens.path(i + 2, n).map(|(_, name)| name.to_owned()),
                };
                out.plugins.push(PluginDef {
                    kind: PluginKind::ImplPlugin,
                    origin,
                    name,
                    line: tok.line,
                });
            }
            (Kind::Other, "fn") => {
                if let Some(name) = app_function(&tokens, i) {
                    out.plugins.push(PluginDef {
                        kind: PluginKind::FnApp,
                        origin,
                        name,
                        line: tok.line,
                    });
                }
            }
            _ => {}
        }
    }
}

/// `fn name(pat: &mut App)` at `i` (optionally generic, `mut pat`, a
/// lifetime, a path ending in `App`), returning nothing. `Some(None)` when the
/// name is a metavariable.
fn app_function(tokens: &Tokens, i: usize) -> Option<Option<String>> {
    let n = tokens.toks.len();
    let name_tok = tokens.toks.get(i + 1)?;
    let name = match name_tok.kind {
        Kind::Ident => Some(name_tok.text.to_owned()),
        Kind::Meta => None,
        _ => return None,
    };
    let mut j = i + 2;
    if tokens.text_is(j, "<") {
        j = tokens.skip_angle(j, n);
    }
    if !tokens.open_paren(j) {
        return None;
    }
    let close = tokens.close[j];
    if close >= n {
        return None;
    }
    let mut k = j + 1;
    if tokens.text_is(k, "mut") {
        k += 1;
    }
    if !(tokens.kind_is(k, Kind::Ident) || tokens.kind_is(k, Kind::Meta) || tokens.text_is(k, "_"))
    {
        return None;
    }
    k += 1;
    if !tokens.text_is(k, ":") || !tokens.text_is(k + 1, "&") {
        return None;
    }
    k += 2;
    // A lifetime is `'` + identifier in a token tree.
    if tokens.text_is(k, "'") {
        k += 2;
    } else if tokens.toks.get(k).is_some_and(|t| t.text.starts_with('\'')) {
        k += 1;
    }
    if !tokens.text_is(k, "mut") {
        return None;
    }
    let (after, last) = tokens.path(k + 1, close)?;
    if last != "App" || after != close || tokens.text_is(close + 1, "->") {
        return None;
    }
    Some(name)
}

/// One `.add_systems(…)` / `.add_observer(…)` / `.add_plugins(…)` whose
/// method name is at `i`.
fn call(
    tokens: &Tokens,
    i: usize,
    origin: Origin,
    enclosing: Option<&str>,
    out: &mut BevyAnalysis,
) {
    let n = tokens.toks.len();
    let method = match tokens.toks[i].text {
        ADD_SYSTEMS => ADD_SYSTEMS,
        ADD_OBSERVER => ADD_OBSERVER,
        _ => ADD_PLUGINS,
    };
    let mut j = i + 1;
    if tokens.turbofish(j) {
        j = tokens.skip_angle(j + 1, n);
    }
    if !tokens.open_paren(j) {
        return;
    }
    match method {
        ADD_SYSTEMS => out.call_sites.add_systems += 1,
        ADD_OBSERVER => out.call_sites.add_observer += 1,
        _ => out.call_sites.add_plugins += 1,
    }
    out.call_sites.in_macros += 1;
    let end = tokens.close[j];
    let args = tokens.elements(j + 1, end);
    let reader = Reader {
        tokens,
        origin,
        method,
        enclosing,
    };
    match (method, args.as_slice()) {
        (ADD_SYSTEMS, [schedule, systems]) => {
            let schedule = tokens.span_text(schedule.0, schedule.1);
            reader.systems(*systems, Target::System, Some(&schedule), out);
        }
        (ADD_SYSTEMS, [systems]) => reader.systems(*systems, Target::System, None, out),
        (ADD_OBSERVER, [observer]) => reader.systems(*observer, Target::Observer, None, out),
        (ADD_PLUGINS, [plugins]) => reader.plugin_uses(*plugins, out),
        _ => out
            .uncertain
            .push(reader.uncertain(UncertainCategory::Arguments, (i, end.min(n)))),
    }
}

struct Reader<'a, 's> {
    tokens: &'a Tokens<'s>,
    origin: Origin,
    method: &'static str,
    enclosing: Option<&'a str>,
}

/// A system element's primary.
enum Primary<'s> {
    /// A parenthesised group: its inner token range.
    Group(usize, usize),
    /// A leaf: its form, name and the end of its text (a path, a callee, a closure).
    Leaf(Form, Option<&'s str>, usize),
}

/// A leaf ready to become a [`Registration`].
struct Leaf {
    form: Form,
    name: Option<String>,
    text: String,
    at: usize,
    adapters: Vec<&'static str>,
    piped: Vec<String>,
}

impl<'s> Reader<'_, 's> {
    fn uncertain(&self, category: UncertainCategory, (a, b): (usize, usize)) -> Uncertain {
        Uncertain {
            category,
            origin: self.origin,
            call: self.method,
            text: self.tokens.span_text(a, b),
            line: self.tokens.toks.get(a).map_or(0, |t| t.line),
        }
    }

    /// The system expression `range`: tuples on a work stack, each element
    /// read as primary + postfix chain.
    fn systems(
        &self,
        range: (usize, usize),
        target: Target,
        schedule: Option<&String>,
        out: &mut BevyAnalysis,
    ) {
        let tokens = self.tokens;
        let mut stack = vec![(range, 0usize)];
        while let Some(((a, b), nesting)) = stack.pop() {
            if nesting > MAX_NESTING {
                out.uncertain
                    .push(self.uncertain(UncertainCategory::NestingTooDeep, (a, b)));
                continue;
            }
            let (primary, mut p) = match self.primary(a, b) {
                Ok(found) => found,
                Err(category) => {
                    out.uncertain.push(self.uncertain(category, (a, b)));
                    continue;
                }
            };
            if let Primary::Leaf(Form::Closure, ..) = primary {
                let leaf = Leaf {
                    form: Form::Closure,
                    name: None,
                    text: "closure".to_owned(),
                    at: a,
                    adapters: Vec::new(),
                    piped: Vec::new(),
                };
                out.registrations
                    .push(self.registration(target, schedule, leaf));
                continue;
            }
            let mut adapters = Vec::new();
            let mut piped = Vec::new();
            // What stops the chain: a field access (not a call) or an unknown method.
            let mut unreadable = None;
            while p < b && tokens.text_is(p, ".") && tokens.kind_is(p + 1, Kind::Ident) {
                let method = tokens.toks[p + 1].text;
                let mut q = p + 2;
                if tokens.turbofish(q) {
                    q = tokens.skip_angle(q + 1, b);
                }
                if !tokens.open_paren(q) {
                    unreadable = Some((UncertainCategory::Expression, (a, b)));
                    break;
                }
                if let Some(adapter) = ADAPTERS.iter().find(|x| **x == method) {
                    adapters.push(*adapter);
                    if *adapter == "pipe"
                        && let Some((_, name)) = tokens.path(q + 1, tokens.close[q])
                    {
                        piped.push(name.to_owned());
                    }
                } else if !COMBINATORS.contains(&method) {
                    unreadable = Some((UncertainCategory::UnknownMethod, (p + 1, p + 2)));
                    break;
                }
                p = tokens.after_group(q);
            }
            // Tokens left after the chain (`a + b`, `c as D`, `e?`, `f[0]`,
            // `x.0`): the element is an expression, as the code reader
            // reports it, never a path guessed from its first tokens.
            if unreadable.is_none() && p < b {
                unreadable = Some((UncertainCategory::Expression, (a, b)));
            }
            if let Some((category, range)) = unreadable {
                out.uncertain.push(self.uncertain(category, range));
                continue;
            }
            match primary {
                Primary::Group(ga, gb) => {
                    for element in tokens.elements(ga, gb).into_iter().rev() {
                        stack.push((element, nesting + 1));
                    }
                }
                Primary::Leaf(form, name, text_end) => {
                    let leaf = Leaf {
                        form,
                        name: name.map(str::to_owned),
                        text: tokens.span_text(a, text_end),
                        at: a,
                        adapters,
                        piped,
                    };
                    out.registrations
                        .push(self.registration(target, schedule, leaf));
                }
            }
        }
    }

    /// The primary of an element and the index where its postfix starts.
    fn primary(&self, a: usize, b: usize) -> Result<(Primary<'s>, usize), UncertainCategory> {
        let tokens = self.tokens;
        let first = tokens.toks[a];
        match (first.kind, first.text) {
            (Kind::Open, "(") => {
                let close = tokens.close[a].min(b);
                Ok((Primary::Group(a + 1, close), tokens.after_group(a)))
            }
            (Kind::Other, "|" | "||") | (Kind::Ident, "move") => {
                Ok((Primary::Leaf(Form::Closure, None, b), b))
            }
            (Kind::Meta, text) if text != "$crate" => Err(UncertainCategory::Metavariable),
            _ => {
                let (after, name) = tokens.path(a, b).ok_or(UncertainCategory::Expression)?;
                if tokens.text_is(after, "!") {
                    return Err(UncertainCategory::MacroInArguments);
                }
                if tokens.open_paren(after) {
                    let leaf = Primary::Leaf(Form::Factory, Some(name), after);
                    Ok((leaf, tokens.after_group(after)))
                } else {
                    Ok((Primary::Leaf(Form::Path, Some(name), after), after))
                }
            }
        }
    }

    fn registration(&self, target: Target, schedule: Option<&String>, leaf: Leaf) -> Registration {
        Registration {
            target,
            origin: self.origin,
            form: leaf.form,
            schedule: schedule.cloned(),
            text: leaf.text,
            name: leaf.name,
            enclosing_fn: self.enclosing.map(str::to_owned),
            adapters: leaf.adapters,
            piped: leaf.piped,
            line: self.tokens.toks[leaf.at].line,
        }
    }

    /// The argument of `add_plugins`: tuples on a work stack; a leaf's name
    /// is its path (or the qualifier of `P::new(…)`), method chains ignored.
    fn plugin_uses(&self, range: (usize, usize), out: &mut BevyAnalysis) {
        let tokens = self.tokens;
        let mut stack = vec![(range, 0usize)];
        while let Some(((a, b), nesting)) = stack.pop() {
            if nesting > MAX_NESTING {
                out.uncertain
                    .push(self.uncertain(UncertainCategory::NestingTooDeep, (a, b)));
                continue;
            }
            if tokens.kind_is(a, Kind::Open) && tokens.text_is(a, "(") {
                let close = tokens.close[a].min(b);
                for element in tokens.elements(a + 1, close).into_iter().rev() {
                    stack.push((element, nesting + 1));
                }
                continue;
            }
            let name = tokens.path(a, b).map(|(after, last)| {
                let lower = last.starts_with(|c: char| c.is_ascii_lowercase());
                if lower
                    && tokens.open_paren(after)
                    && after >= a + 3
                    && tokens.text_is(after - 2, "::")
                {
                    tokens.toks[after - 3].text.to_owned()
                } else {
                    last.to_owned()
                }
            });
            out.plugin_uses.push(PluginUse {
                origin: self.origin,
                name,
                text: tokens.span_text(a, b),
                line: tokens.toks[a].line,
            });
        }
    }
}
