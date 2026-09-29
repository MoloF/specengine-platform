//! The front-matter block: where it is (BOM, `---` fences, CRLF kept) and
//! what its typed keys say.

use std::ops::Range;

use specengine_model::grammar::{self, Canon, Found};
use specengine_model::link::is_link_type;
use specengine_model::{
    CanonTarget, Diagnostic, DiagnosticCode, ExtraEntry, Fields, FmValue, IdScheme, IdScript,
    LinkTarget, OrderedMap, ParentRef, Reference, Span,
};

use crate::yaml::{self, YNode, YValue};

/// Highest revision: `rev = digit{1,9}`.
const MAX_REV: i64 = 999_999_999;

/// The layout of a file: BOM, front-matter block, body.
pub(crate) struct Layout {
    pub bom: bool,
    pub block: Option<Block>,
    /// `---` opened a block that nothing closes: the whole file is body.
    pub unclosed: bool,
    pub body: Span,
}

/// A closed front-matter block.
pub(crate) struct Block {
    /// From the opening `---` through the closing line's ending.
    pub span: Span,
    /// The YAML between the fences.
    pub yaml: Range<usize>,
}

/// Front-matter iff the first line after the optional BOM is exactly `---`,
/// closed by the next line that is exactly `---` (`\n` or `\r\n` endings).
pub(crate) fn split(text: &str) -> Layout {
    let bom = text.starts_with('\u{FEFF}');
    let start = if bom { '\u{FEFF}'.len_utf8() } else { 0 };
    let whole_body = |unclosed| Layout {
        bom,
        block: None,
        unclosed,
        body: Span::new(start, text.len()),
    };
    let Some((first, mut next)) = line_at(text, start) else {
        return whole_body(false);
    };
    if first != "---" {
        return whole_body(false);
    }
    let yaml_start = next;
    while let Some((line, after)) = line_at(text, next) {
        if line == "---" {
            return Layout {
                bom,
                block: Some(Block {
                    span: Span::new(start, after),
                    yaml: yaml_start..next,
                }),
                unclosed: false,
                body: Span::new(after, text.len()),
            };
        }
        next = after;
    }
    whole_body(true)
}

/// The line starting at `at` without its ending, and where the next starts.
fn line_at(text: &str, at: usize) -> Option<(&str, usize)> {
    if at >= text.len() {
        return None;
    }
    let rest = &text[at..];
    let (line, after) = match rest.find('\n') {
        Some(newline) => (&rest[..newline], at + newline + 1),
        None => (rest, text.len()),
    };
    Some((line.strip_suffix('\r').unwrap_or(line), after))
}

/// A declared link before the document's ID is known.
pub(crate) enum PendingLink {
    /// From the document.
    Declared { link_type: String, dst: LinkTarget },
    /// `status: superseded-by X`: `X` supersedes the document.
    SupersededBy { src: String, src_span: Option<Span> },
}

/// The document's ID as defined by `id:`.
pub(crate) struct DocumentId {
    pub id: String,
    pub span: Option<Span>,
    pub script: IdScript,
}

/// What the typed keys say.
#[derive(Default)]
pub(crate) struct FrontMatter {
    pub id: Option<DocumentId>,
    /// Declared `kind` and its line.
    pub kind: Option<(String, usize)>,
    pub title: Option<String>,
    pub rev: Option<u32>,
    pub parent: Option<ParentRef>,
    pub fields: Fields,
    pub extra: Vec<ExtraEntry>,
    pub links: Vec<PendingLink>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Reads the block's YAML. A YAML error is one diagnostic at its file line
/// and nothing else: no guessed value, no lax re-parse.
pub(crate) fn read(text: &str, block: &Block, first_line: usize, scheme: &IdScheme) -> FrontMatter {
    let yaml_text = &text[block.yaml.clone()];
    let mut reader = Reader {
        yaml_text,
        yaml_start: block.yaml.start,
        first_line,
        scheme,
        out: FrontMatter::default(),
    };
    match yaml::parse(yaml_text) {
        Err(error) => {
            let line = reader.file_line(error.line);
            reader.out.diagnostics.push(Diagnostic::new(
                DiagnosticCode::FrontmatterYaml,
                line,
                format!("front-matter is not valid YAML: {}", error.message),
            ));
        }
        Ok(root) => match root.value {
            YValue::Null => {}
            YValue::Map(entries) => {
                for (key, value) in entries {
                    reader.entry(key, value);
                }
                reader.check_kind();
            }
            other => {
                let line = reader.file_line(root.line);
                reader.out.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::FrontmatterNotMapping,
                    line,
                    format!("front-matter is {}, not a mapping", other.type_name()),
                ));
            }
        },
    }
    reader.out
}

struct Reader<'a> {
    yaml_text: &'a str,
    yaml_start: usize,
    /// File line of the YAML text's first line.
    first_line: usize,
    scheme: &'a IdScheme,
    out: FrontMatter,
}

impl Reader<'_> {
    /// File line of a YAML line; an unknown line (0) is the opening `---`.
    fn file_line(&self, yaml_line: usize) -> usize {
        if yaml_line == 0 {
            self.first_line.saturating_sub(1).max(1)
        } else {
            self.first_line + yaml_line - 1
        }
    }

    /// File offset at which `value` lies verbatim inside the node's source,
    /// when it does (plain or quoted without escapes).
    fn verbatim_at(&self, node: &YNode, value: &str) -> Option<usize> {
        let span = node.span.clone()?;
        let source = self.yaml_text.get(span.clone())?;
        source
            .find(value)
            .map(|offset| self.yaml_start + span.start + offset)
    }

    fn entry(&mut self, key: YNode, value: YNode) {
        let line = self.file_line(if value.line == 0 {
            key.line
        } else {
            value.line
        });
        let Some(name) = key.value.scalar_text().map(|name| name.into_owned()) else {
            self.out.diagnostics.push(Diagnostic::new(
                DiagnosticCode::FrontmatterType,
                self.file_line(key.line),
                "a front-matter key is not a scalar; the entry is skipped",
            ));
            return;
        };
        if matches!(value.value, YValue::Null) && is_typed(&name) {
            return;
        }
        match name.as_str() {
            "id" => self.id(&name, value, line),
            "kind" => {
                if let Some(kind) = self.string(&name, &value, line) {
                    self.out.kind = Some((kind, line));
                }
            }
            "title" => self.out.title = self.string(&name, &value, line),
            "status" => {
                let status = self.string(&name, &value, line);
                if let Some(status) = &status {
                    self.superseded_by(status, &value, line);
                }
                self.out.fields.status = status;
            }
            "class" => self.out.fields.class = self.string(&name, &value, line),
            "owner" => self.out.fields.owner = self.string(&name, &value, line),
            "reviewed" => self.out.fields.reviewed = self.string(&name, &value, line),
            "date" => self.out.fields.date = self.string(&name, &value, line),
            "shipped" => self.out.fields.shipped = self.string(&name, &value, line),
            "ref" => self.out.fields.reference = self.string(&name, &value, line),
            "to" => self.out.fields.to = self.string(&name, &value, line),
            "severity" => self.out.fields.severity = self.string(&name, &value, line),
            "generator" => self.out.fields.generator = self.string(&name, &value, line),
            "source" => self.out.fields.source = self.string(&name, &value, line),
            "acceptance" => self.out.fields.acceptance = self.string(&name, &value, line),
            "tier" => match value.value {
                YValue::Int(tier) => self.out.fields.tier = Some(tier),
                _ => self.mistyped(&name, &value, line, "an integer"),
            },
            "rev" => self.rev(&name, &value, line),
            "scope" => self.out.fields.scope = self.string_list(&name, &value, line),
            "aliases" => self.out.fields.aliases = self.string_list(&name, &value, line),
            "parent" => {
                if let Some(found) = self.reference(&name, &value, line) {
                    let reference = found.reference;
                    self.out.parent = Some(ParentRef {
                        id: reference.id,
                        span: reference.span,
                    });
                }
            }
            "working_answer" => {
                if let Some(found) = self.reference(&name, &value, line) {
                    self.declare(&name, found.reference.clone());
                    self.out.fields.working_answer = Some(found.reference);
                }
            }
            "canon" => self.canon(&name, &value, line),
            "supersedes" => {
                if let Some(list) = self.reference_list(&name, &value, line) {
                    for reference in &list {
                        self.declare(&name, reference.clone());
                    }
                    self.out.fields.supersedes = Some(list);
                }
            }
            "adrs" | "refs" => {
                if let Some(list) = self.reference_list(&name, &value, line) {
                    for reference in &list {
                        self.declare(specengine_model::MENTIONS, reference.clone());
                    }
                    if name == "adrs" {
                        self.out.fields.adrs = Some(list);
                    } else {
                        self.out.fields.refs = Some(list);
                    }
                }
            }
            "links" => self.links(&name, &value, line),
            "raised_by" => match &value.value {
                YValue::Map(entries) => {
                    self.out.fields.raised_by = Some(ordered_map(entries));
                }
                _ => self.mistyped(&name, &value, line, "a mapping"),
            },
            _ => {
                self.out.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::UnknownKey,
                    self.file_line(key.line),
                    format!("front-matter key `{name}` is not a typed key; kept in `extra`"),
                ));
                self.keep(name, &value);
            }
        }
    }

    fn keep(&mut self, key: String, value: &YNode) {
        self.out.extra.push(ExtraEntry {
            key,
            value: fm_value(value),
        });
    }

    fn mistyped(&mut self, name: &str, value: &YNode, line: usize, expected: &str) {
        self.out.diagnostics.push(Diagnostic::new(
            DiagnosticCode::FrontmatterType,
            line,
            format!(
                "front-matter `{name}` is {}, expected {expected}; kept in `extra`",
                value.value.type_name()
            ),
        ));
        self.keep(name.to_owned(), value);
    }

    fn string(&mut self, name: &str, value: &YNode, line: usize) -> Option<String> {
        match &value.value {
            YValue::Str(text) => Some(text.clone()),
            _ => {
                self.mistyped(name, value, line, "a string");
                None
            }
        }
    }

    /// A list of strings; anything else (a bare string included) is
    /// `frontmatter-type`.
    fn string_list(&mut self, name: &str, value: &YNode, line: usize) -> Option<Vec<String>> {
        match &value.value {
            YValue::Seq(items) => {
                let strings: Option<Vec<String>> = items
                    .iter()
                    .map(|item| match &item.value {
                        YValue::Str(text) => Some(text.clone()),
                        _ => None,
                    })
                    .collect();
                if strings.is_none() {
                    self.mistyped(name, value, line, "a list of strings");
                }
                strings
            }
            _ => {
                self.mistyped(name, value, line, "a list of strings");
                None
            }
        }
    }

    fn id(&mut self, name: &str, value: YNode, line: usize) {
        let YValue::Str(text) = &value.value else {
            self.mistyped(name, &value, line, "a string");
            return;
        };
        let at = self.verbatim_at(&value, text);
        match grammar::parse_definition(text, at.unwrap_or(0), self.scheme) {
            Some(definition) => {
                if let Some(homoglyph) = definition.homoglyph {
                    self.out.diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::Homoglyph,
                            line,
                            format!(
                                "ID `{text}` mixes in look-alike characters; the Latin ID is `{}`",
                                homoglyph.fix
                            ),
                        )
                        .with_span(at.map(|_| homoglyph.span))
                        .with_fix(homoglyph.fix),
                    );
                }
                self.out.id = Some(DocumentId {
                    id: definition.id,
                    span: at.map(|_| definition.span),
                    script: definition.script,
                });
            }
            None => self.out.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::IdNotInScheme,
                    line,
                    format!("`id: {text}` is not an ID of a prefix configured in `[ids]`; the document has no ID"),
                )
                .with_span(at.map(|at| Span::new(at, at + text.len()))),
            ),
        }
    }

    fn rev(&mut self, name: &str, value: &YNode, line: usize) {
        match value.value {
            YValue::Int(rev) if (0..=MAX_REV).contains(&rev) => {
                self.out.rev = u32::try_from(rev).ok();
            }
            YValue::Int(_) | YValue::UInt(_) => {
                self.out.diagnostics.push(Diagnostic::new(
                    DiagnosticCode::BadRev,
                    line,
                    "front-matter `rev` is not 1-9 digits; kept in `extra`",
                ));
                self.keep(name.to_owned(), value);
            }
            _ => self.mistyped(name, value, line, "an integer"),
        }
    }

    /// Exactly one reference; anything else is `unparsed-reference`.
    fn reference(&mut self, name: &str, value: &YNode, line: usize) -> Option<Found> {
        let YValue::Str(text) = &value.value else {
            self.mistyped(name, value, line, "a string");
            return None;
        };
        self.one_reference(name, text, value, line)
    }

    fn one_reference(
        &mut self,
        name: &str,
        text: &str,
        node: &YNode,
        line: usize,
    ) -> Option<Found> {
        let at = self.verbatim_at(node, text);
        match grammar::parse_reference(text, at.unwrap_or(0), self.scheme) {
            Some(found) => Some(self.accept(found, at.is_some(), line)),
            None => {
                self.unparsed(name, text, at, line);
                None
            }
        }
    }

    fn unparsed(&mut self, name: &str, text: &str, at: Option<usize>, line: usize) {
        self.out.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::UnparsedReference,
                line,
                format!("front-matter `{name}`: `{text}` is not exactly one reference"),
            )
            .with_span(at.map(|at| Span::new(at, at + text.len()))),
        );
    }

    /// Reports the lexer's findings and drops spans that are not verbatim.
    fn accept(&mut self, mut found: Found, verbatim: bool, line: usize) -> Found {
        for homoglyph in &found.homoglyphs {
            self.out.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::Homoglyph,
                    line,
                    format!(
                        "reference mixes in look-alike characters; the Latin ID is `{}`",
                        homoglyph.fix
                    ),
                )
                .with_span(verbatim.then_some(homoglyph.span))
                .with_fix(homoglyph.fix.clone()),
            );
        }
        if let Some(bad) = found.bad_rev {
            self.out.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::BadRev,
                    line,
                    "`@` is followed by digits that are no 1-9 digit revision",
                )
                .with_span(verbatim.then_some(bad)),
            );
        }
        if !verbatim {
            found.reference.span = None;
        }
        found
    }

    /// A list of strings, each exactly one reference; anything else (a bare
    /// string included) is `frontmatter-type`. Items that are not exactly
    /// one reference are `unparsed-reference` and dropped.
    fn reference_list(&mut self, name: &str, value: &YNode, line: usize) -> Option<Vec<Reference>> {
        let YValue::Seq(items) = &value.value else {
            self.mistyped(name, value, line, "a list of references");
            return None;
        };
        if items
            .iter()
            .any(|item| !matches!(item.value, YValue::Str(_)))
        {
            self.mistyped(name, value, line, "a list of references");
            return None;
        }
        let mut references = Vec::with_capacity(items.len());
        for item in items {
            if let YValue::Str(text) = &item.value {
                let item_line = if item.line == 0 {
                    line
                } else {
                    self.file_line(item.line)
                };
                if let Some(found) = self.one_reference(name, text, item, item_line) {
                    references.push(found.reference);
                }
            }
        }
        Some(references)
    }

    fn canon(&mut self, name: &str, value: &YNode, line: usize) {
        let YValue::Str(text) = &value.value else {
            self.mistyped(name, value, line, "a string");
            return;
        };
        let at = self.verbatim_at(value, text);
        match grammar::parse_canon(text, at.unwrap_or(0), self.scheme) {
            Some(Canon::Reference(found)) => {
                let found = self.accept(found, at.is_some(), line);
                self.declare(name, found.reference.clone());
                self.out.fields.canon = Some(CanonTarget::Reference(found.reference));
            }
            Some(Canon::Path(mut path)) => {
                if at.is_none() {
                    path.span = None;
                }
                self.out.links.push(PendingLink::Declared {
                    link_type: name.to_owned(),
                    dst: LinkTarget::Path(path.clone()),
                });
                self.out.fields.canon = Some(CanonTarget::Path(path));
            }
            None => self.unparsed(name, text, at, line),
        }
    }

    fn superseded_by(&mut self, status: &str, value: &YNode, line: usize) {
        let Some((offset, target)) = grammar::split_superseded_by(status) else {
            return;
        };
        let at = self.verbatim_at(value, status);
        let base = at.map_or(0, |at| at + offset);
        match grammar::parse_reference(target, base, self.scheme) {
            Some(found) => {
                let found = self.accept(found, at.is_some(), line);
                self.out.links.push(PendingLink::SupersededBy {
                    src: found.reference.id,
                    src_span: found.reference.span,
                });
            }
            None => self.unparsed("status", target, at.map(|at| at + offset), line),
        }
    }

    fn links(&mut self, name: &str, value: &YNode, line: usize) {
        let YValue::Map(entries) = &value.value else {
            self.mistyped(name, value, line, "a mapping of link type to references");
            return;
        };
        let well_typed = entries.iter().all(|(key, list)| {
            matches!(key.value, YValue::Str(_))
                && match &list.value {
                    YValue::Null => true,
                    YValue::Seq(items) => items
                        .iter()
                        .all(|item| matches!(item.value, YValue::Str(_))),
                    _ => false,
                }
        });
        if !well_typed {
            self.mistyped(name, value, line, "a mapping of link type to references");
            return;
        }
        let mut map = OrderedMap::new();
        for (key, list) in entries {
            let YValue::Str(link_type) = &key.value else {
                continue;
            };
            let key_line = self.file_line(key.line);
            if !is_link_type(link_type) {
                self.out.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::UnknownLinkType,
                        key_line,
                        format!(
                            "link type `{link_type}` is not one of the shared link types; kept"
                        ),
                    )
                    .with_span(
                        self.verbatim_at(key, link_type)
                            .map(|at| Span::new(at, at + link_type.len())),
                    ),
                );
            }
            let references = if matches!(list.value, YValue::Null) {
                Vec::new()
            } else {
                self.reference_list(link_type, list, key_line)
                    .unwrap_or_default()
            };
            for reference in &references {
                self.declare(link_type, reference.clone());
            }
            map.push(link_type.clone(), references);
        }
        self.out.fields.links = Some(map);
    }

    fn declare(&mut self, link_type: &str, dst: Reference) {
        self.out.links.push(PendingLink::Declared {
            link_type: link_type.to_owned(),
            dst: LinkTarget::Reference(dst),
        });
    }

    /// `kind-mismatch`: the declared kind differs from the ID prefix's kind.
    fn check_kind(&mut self) {
        let (Some((declared, line)), Some(id)) = (&self.out.kind, &self.out.id) else {
            return;
        };
        if let Some(prefix_kind) = self.scheme.kind_of_id(&id.id)
            && prefix_kind != declared
        {
            let message = format!(
                "declared kind `{declared}` differs from `{prefix_kind}` of the ID's prefix; the declared kind is kept"
            );
            self.out.diagnostics.push(Diagnostic::new(
                DiagnosticCode::KindMismatch,
                *line,
                message,
            ));
        }
    }
}

/// Keys with a type: a null value leaves them absent.
fn is_typed(name: &str) -> bool {
    matches!(
        name,
        "id" | "kind"
            | "class"
            | "title"
            | "status"
            | "owner"
            | "reviewed"
            | "date"
            | "shipped"
            | "ref"
            | "to"
            | "severity"
            | "generator"
            | "source"
            | "acceptance"
            | "tier"
            | "rev"
            | "scope"
            | "aliases"
            | "parent"
            | "working_answer"
            | "canon"
            | "supersedes"
            | "adrs"
            | "refs"
            | "links"
            | "raised_by"
    )
}

fn fm_value(node: &YNode) -> FmValue {
    match &node.value {
        YValue::Null => FmValue::Null,
        YValue::Bool(value) => FmValue::Bool(*value),
        YValue::Int(value) => FmValue::Int(*value),
        YValue::UInt(value) => FmValue::UInt(*value),
        YValue::Float(value) => FmValue::Float(*value),
        YValue::Str(value) => FmValue::Str(value.clone()),
        YValue::Seq(items) => FmValue::Seq(items.iter().map(fm_value).collect()),
        YValue::Map(entries) => FmValue::Map(ordered_map(entries)),
    }
}

/// Keys in source order; a collection as a key is kept as `?`.
fn ordered_map(entries: &[(YNode, YNode)]) -> OrderedMap<FmValue> {
    OrderedMap(
        entries
            .iter()
            .map(|(key, value)| {
                let key = key
                    .value
                    .scalar_text()
                    .map_or_else(|| "?".to_owned(), |text| text.into_owned());
                (key, fm_value(value))
            })
            .collect(),
    )
}
