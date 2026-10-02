//! Core of SpecEngine, Phase 1 increment 1: the spec parser
//! (docs/features/spec-parser.md).
//!
//! [`parse`] turns one file's bytes into a [`ParsedFile`]: the document
//! node, its `{#ID}` section nodes, declared and inline links (ID mentions
//! and local Markdown file links, docs/features/spec-check-links.md),
//! heading anchors, byte-exact spans into the original bytes (BOM and CRLF
//! untouched), a token estimate per node, and diagnostics. It reads no file
//! and writes none; the output depends only on (path, bytes, scheme), and a
//! broken file is reported, never fatal (ADR-0012).
//!
//! - [`check`] — `spec check` increment 1: the config-driven check over a
//!   set of parses (docs/features/spec-check.md);
//! - [`scheme_toml`] — `IdScheme::from_toml`: the `[ids]` table;
//! - [`paths_toml`] — `Paths::from_toml`: the `[paths]` table (role
//!   directories, walked roots, exclude globs; docs/features/spec-index.md);
//! - [`project_toml`] — `ProjectConfig::from_toml`: the whole
//!   `specengine.toml` of the `spec` commands (a closed `[project]` with its
//!   `slug`, `[ids]`, `[paths]`, the known top-level keys;
//!   docs/features/spec-cli.md);
//! - [`tokens`] — the per-script token estimator;
//! - `front_matter` — the block and its typed keys (`serde-saphyr`, with
//!   depth and alias budgets);
//! - `markdown` — headings, attribute blocks, text regions and local link
//!   destinations (`pulldown-cmark` with offsets);
//! - [`walk_scope`] — [`WalkScope`]: the walk's rules as pure predicates,
//!   shared by the store's walker and the check's link scope;
//! - `glob` — the `[paths] exclude` matcher behind it;
//! - [`own_spans`] — a node's own text as spans (its body minus nested ID
//!   sections): the index's `own_text` and the check's `text` rule.
//!
//! The corpus model and the reference grammar live in `specengine-model`.

pub mod check;
mod front_matter;
mod glob;
mod lines;
mod markdown;
mod own_text;
pub mod paths_toml;
pub mod project_toml;
pub mod scheme_toml;
pub mod tokens;
pub mod walk_scope;
mod yaml;

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::grammar::{self, Definition};
use specengine_model::{
    Anchor, AnchorOrigin, Diagnostic, DiagnosticCode, IdScheme, IdScript, Link, LinkOrigin,
    LinkTarget, Node, ParentRef, ParsedFile, PathTarget, Reference, Span,
};

pub use own_text::own_spans;
pub use paths_toml::{Paths, PathsError, paths_from_toml};
pub use project_toml::{
    MAX_SLUG_BYTES, Project, ProjectConfig, ProjectError, project_from_toml, slug_problem,
};
pub use scheme_toml::{IdSchemeToml, scheme_from_toml};
pub use tokens::tokens_est;
pub use walk_scope::{DOCUMENT_EXTENSION, WalkScope, is_clean_relative, is_under};
pub use yaml::{MAX_ALIAS_EXPANSION, MAX_DEPTH};

use crate::front_matter::{FrontMatter, PendingLink};
use crate::lines::LineIndex;
use crate::markdown::Heading;

/// UTF-8 byte-order mark.
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Parses one file. `path` only names the file in the result.
pub fn parse(path: &str, bytes: &[u8], scheme: &IdScheme) -> ParsedFile {
    let bom = bytes.starts_with(BOM);
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => return not_utf8(path, bytes, bom, &error),
    };
    let lines = LineIndex::new(bytes);
    let layout = front_matter::split(text);
    let mut diagnostics = Vec::new();
    if layout.unclosed {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::FrontmatterUnclosed,
            1,
            "`---` opens front-matter and no `---` line closes it; the whole file is read as body",
        ));
    }
    let front = match &layout.block {
        Some(block) => front_matter::read(text, block, lines.line(block.yaml.start), scheme),
        None => FrontMatter::default(),
    };
    let FrontMatter {
        id,
        kind,
        title,
        rev,
        parent,
        fields,
        extra,
        links: pending,
        diagnostics: front_diagnostics,
    } = front;
    diagnostics.extend(front_diagnostics);

    let document_id = id.as_ref().map(|id| id.id.clone());
    let body = markdown::scan(text, layout.body);

    // The document node.
    let document_kind = kind.map(|(kind, _)| kind).or_else(|| {
        document_id
            .as_deref()
            .and_then(|id| scheme.kind_of_id(id))
            .map(str::to_owned)
    });
    let document_title = title.or_else(|| {
        body.first_h1
            .map(|index| body.headings[index].text.clone())
            .filter(|text| !text.is_empty())
    });
    let mut nodes = vec![Node {
        id: document_id.clone(),
        script: id
            .as_ref()
            .map(|id| id.script)
            .filter(|script| *script != IdScript::Latin),
        kind: document_kind,
        title: document_title,
        summary: body.summary,
        level: None,
        heading: None,
        body: None,
        attrs: Vec::new(),
        classes: Vec::new(),
        rev,
        parent,
        span: Span::new(0, text.len()),
        tokens_est: tokens_est(text),
        fields: Some(fields),
        extra: Some(extra),
    }];

    // Declared links, in front-matter order.
    let mut links = Vec::new();
    for link in pending {
        match link {
            PendingLink::Declared { link_type, dst } => links.push(Link {
                src: document_id.clone(),
                src_span: None,
                link_type,
                origin: LinkOrigin::Frontmatter,
                dst,
            }),
            PendingLink::SupersededBy { src, src_span } => {
                if let Some(this) = &id {
                    links.push(Link {
                        src: Some(src),
                        src_span,
                        link_type: "supersedes".to_owned(),
                        origin: LinkOrigin::Frontmatter,
                        dst: LinkTarget::Reference(Reference {
                            id: this.id.clone(),
                            alias_of: None,
                            script: this.script,
                            project: None,
                            scope: None,
                            section: None,
                            rev: None,
                            form: Default::default(),
                            label: None,
                            span: this.span,
                        }),
                    });
                }
            }
        }
    }

    // Sections and anchors.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    if let Some(id) = &id {
        seen.insert(id.id.clone());
    }
    let definitions: Vec<Option<Definition>> = body
        .headings
        .iter()
        .map(|heading| {
            let (raw_id, offset) = heading.id.as_ref()?;
            grammar::parse_definition(raw_id, offset.unwrap_or(0), scheme)
        })
        .collect();
    let extents = extents(text, &body.headings, &definitions, layout.body.end);
    let anchors = anchors(&body, &definitions);
    // Heading index → section ID, for the parents of nested sections.
    let mut section_ids: BTreeMap<usize, String> = BTreeMap::new();
    for (index, (extent, definition)) in extents.iter().zip(definitions).enumerate() {
        let heading = &body.headings[index];
        let Some((raw_id, offset)) = &heading.id else {
            continue;
        };
        let line = lines.line(heading.span.start);
        let Some(definition) = definition else {
            continue;
        };
        let id_span = offset.map(|_| definition.span);
        if let Some(homoglyph) = &definition.homoglyph {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::Homoglyph,
                    line,
                    format!(
                        "section ID `{raw_id}` mixes in look-alike characters; the Latin ID is `{}`",
                        homoglyph.fix
                    ),
                )
                .with_span(offset.map(|_| homoglyph.span))
                .with_fix(homoglyph.fix.clone()),
            );
        }
        if !seen.insert(definition.id.clone()) {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::DuplicateId,
                    line,
                    format!(
                        "ID `{}` is defined twice in this file; both are kept",
                        definition.id
                    ),
                )
                .with_span(id_span),
            );
        }
        let rev = section_rev(heading, line, &mut diagnostics);
        let parent = extent
            .parent
            .and_then(|parent| section_ids.get(&parent).cloned())
            .or_else(|| document_id.clone())
            .map(|id| ParentRef { id, span: None });
        section_ids.insert(index, definition.id.clone());
        // The body starts on the line after the heading.
        let body_start = heading.raw_end.clamp(heading.span.end, extent.end);
        nodes.push(Node {
            script: Some(definition.script).filter(|script| *script != IdScript::Latin),
            id: Some(definition.id),
            kind: Some(definition.kind),
            title: Some(heading.text.clone()).filter(|text| !text.is_empty()),
            summary: None,
            level: Some(heading.level),
            heading: Some(heading.span),
            body: Some(Span::new(body_start, extent.end)),
            attrs: heading.attrs.clone(),
            classes: heading.classes.clone(),
            rev,
            parent,
            span: Span::new(heading.span.start, extent.end),
            tokens_est: tokens_est(&text[heading.span.start..extent.end]),
            fields: None,
            extra: None,
        });
    }

    // Inline mentions and file links, by span start, from the innermost ID
    // section. The sort is stable and two never share a start: a
    // destination is never text.
    let mut inline: Vec<(usize, InlineLink)> = Vec::new();
    for region in &body.regions {
        for found in grammar::scan(&text[region.clone()], region.start, scheme) {
            if let Some(span) = found.reference.span {
                inline.push((span.start, InlineLink::Mention(found)));
            }
        }
    }
    for link in body.file_links {
        inline.push((link.span.start, InlineLink::File(link)));
    }
    inline.sort_by_key(|&(start, _)| start);
    let section_spans: Vec<(Span, String)> = nodes[1..]
        .iter()
        .filter_map(|node| node.id.clone().map(|id| (node.span, id)))
        .collect();
    let mut open: Vec<usize> = Vec::new();
    let mut next = 0;
    for (start, link) in inline {
        while next < section_spans.len() && section_spans[next].0.start <= start {
            while open
                .last()
                .is_some_and(|&top| section_spans[top].0.end <= section_spans[next].0.start)
            {
                open.pop();
            }
            open.push(next);
            next += 1;
        }
        while open
            .last()
            .is_some_and(|&top| section_spans[top].0.end <= start)
        {
            open.pop();
        }
        let src = open
            .last()
            .map(|&index| section_spans[index].1.clone())
            .or_else(|| document_id.clone());
        let dst = match link {
            InlineLink::Mention(found) => {
                let line = lines.line(start);
                for homoglyph in &found.homoglyphs {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::Homoglyph,
                            line,
                            format!(
                                "reference mixes in look-alike characters; the Latin ID is `{}`",
                                homoglyph.fix
                            ),
                        )
                        .with_span(Some(homoglyph.span))
                        .with_fix(homoglyph.fix.clone()),
                    );
                }
                if let Some(bad) = found.bad_rev {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::BadRev,
                            line,
                            "`@` is followed by digits that are no 1-9 digit revision",
                        )
                        .with_span(Some(bad)),
                    );
                }
                LinkTarget::Reference(found.reference)
            }
            InlineLink::File(link) => LinkTarget::Path(PathTarget {
                path: link.path,
                anchor: link.anchor,
                span: Some(link.span),
            }),
        };
        links.push(Link {
            src,
            src_span: None,
            link_type: specengine_model::MENTIONS.to_owned(),
            origin: LinkOrigin::Inline,
            dst,
        });
    }

    diagnostics.sort_by_key(|diagnostic| diagnostic.line);
    ParsedFile {
        path: path.to_owned(),
        bom: layout.bom,
        front_matter: layout.block.as_ref().map(|block| block.span),
        body: layout.body,
        nodes,
        links,
        anchors,
        diagnostics,
    }
}

/// A body link before its `src` is known.
enum InlineLink {
    /// An ID reference in the text.
    Mention(grammar::Found),
    /// A local Markdown link destination.
    File(markdown::FileLink),
}

/// A file that is not UTF-8: no nodes, one diagnostic at the first bad byte.
fn not_utf8(path: &str, bytes: &[u8], bom: bool, error: &std::str::Utf8Error) -> ParsedFile {
    let at = error.valid_up_to();
    let bad_len = error.error_len().unwrap_or(bytes.len() - at);
    let line = LineIndex::new(bytes).line(at);
    ParsedFile {
        path: path.to_owned(),
        bom,
        front_matter: None,
        body: Span::new(if bom { BOM.len() } else { 0 }, bytes.len()),
        nodes: Vec::new(),
        links: Vec::new(),
        anchors: Vec::new(),
        diagnostics: vec![
            Diagnostic::new(
                DiagnosticCode::NotUtf8,
                line,
                format!("the file is not UTF-8 (byte {at}); nothing is read"),
            )
            .with_span(Some(Span::new(at, at + bad_len))),
        ],
    }
}

/// Every anchor of the body in source order: per heading its slug, then its
/// `{#…}` attribute when that is no definable ID; HTML anchors by position
/// (after the heading they sit in).
fn anchors(body: &markdown::Body, definitions: &[Option<Definition>]) -> Vec<Anchor> {
    let mut slugger = markdown::Slugger::default();
    let mut from_headings = Vec::new();
    for (heading, definition) in body.headings.iter().zip(definitions) {
        if let Some(name) = slugger.next(&heading.slug_text) {
            from_headings.push(Anchor {
                name,
                origin: AnchorOrigin::Slug,
                level: Some(heading.level),
                span: heading.span,
            });
        }
        if let (Some((raw_id, _)), None) = (&heading.id, definition) {
            from_headings.push(Anchor {
                name: raw_id.clone(),
                origin: AnchorOrigin::Attr,
                level: Some(heading.level),
                span: heading.span,
            });
        }
    }
    let mut merged = Vec::with_capacity(from_headings.len() + body.html_anchors.len());
    let mut html = body.html_anchors.iter().peekable();
    for anchor in from_headings {
        while let Some(next) = html.next_if(|next| next.span.start < anchor.span.start) {
            merged.push(html_anchor(next));
        }
        merged.push(anchor);
    }
    merged.extend(html.map(html_anchor));
    merged
}

fn html_anchor(anchor: &markdown::HtmlAnchor) -> Anchor {
    Anchor {
        name: anchor.name.clone(),
        origin: AnchorOrigin::Html,
        level: None,
        span: anchor.span,
    }
}

/// Where one heading's section ends and which section encloses it.
struct Extent {
    /// End of the section: the next heading of the same or a higher level
    /// (or the body's end), trailing whitespace excluded — the census rule.
    end: usize,
    /// The nearest enclosing heading that defines an ID, if any.
    parent: Option<usize>,
}

/// Extents of every heading, in one pass with a stack: linear in the number
/// of headings whatever their levels.
fn extents(
    text: &str,
    headings: &[Heading],
    definitions: &[Option<Definition>],
    body_end: usize,
) -> Vec<Extent> {
    let mut raw_end = vec![body_end; headings.len()];
    let mut parents = vec![None; headings.len()];
    let mut stack: Vec<usize> = Vec::new();
    // Headings on the stack that define an ID, innermost last.
    let mut id_stack: Vec<usize> = Vec::new();
    for (index, heading) in headings.iter().enumerate() {
        while let Some(&top) = stack.last() {
            if headings[top].level < heading.level {
                break;
            }
            raw_end[top] = heading.span.start;
            stack.pop();
            if id_stack.last() == Some(&top) {
                id_stack.pop();
            }
        }
        parents[index] = id_stack.last().copied();
        stack.push(index);
        if definitions[index].is_some() {
            id_stack.push(index);
        }
    }
    headings
        .iter()
        .enumerate()
        .map(|(index, heading)| {
            let raw = heading.span.start..raw_end[index].max(heading.span.end);
            Extent {
                end: markdown::trimmed(text, raw).end.max(heading.span.end),
                parent: parents[index],
            }
        })
        .collect()
}

/// `rev=N` on the heading: 1–9 digits, else `bad-rev`.
fn section_rev(heading: &Heading, line: usize, diagnostics: &mut Vec<Diagnostic>) -> Option<u32> {
    let (_, value) = heading.attrs.iter().find(|(key, _)| key == "rev")?;
    match value.as_deref() {
        Some(digits)
            if (1..=9).contains(&digits.len())
                && digits.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            digits.parse().ok()
        }
        _ => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::BadRev,
                line,
                "heading attribute `rev` is not 1-9 digits",
            ));
            None
        }
    }
}
