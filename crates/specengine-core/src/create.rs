//! The pure half of a proposal of kind `create` (task spec
//! `proposal-kinds`, "Rules and edge cases"): where a text defines IDs as
//! written ([`id_sites`]), what one written ID is ([`written_id`]), and the
//! section rule ([`add_sections`]): new `{#ID}` sections spliced into one
//! node's span, deeper than the node, every (ID, heading level) pair of the
//! file kept in order. The number form of a new ID is
//! [`crate::record::record_form`]. Nothing is read, written or hashed here,
//! and no project's prefix, kind or directory appears (ADR-0008): the
//! proposer names the path and the IDs.

use std::fmt;

use specengine_model::grammar::{self, Homoglyph};
use specengine_model::{IdScheme, IdScript, ParsedFile, RefForm, Span};

use crate::lines::LineIndex;
use crate::patch::{StructureEntry, is_section, splice, structure, update_text};
use crate::{front_matter, markdown};

/// Where a text defines an ID, as written: its document's `id:` or one
/// heading's `{#…}` attribute (an ID of the scheme or not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdSite {
    /// The `id:` value or the attribute's text, trimmed.
    pub written: String,
    /// The heading's level; `None` for the document's `id:`.
    pub level: Option<u8>,
    /// Byte offset in the text: the heading's first byte; the front-matter
    /// block's for `id:`.
    pub offset: usize,
    /// 1-based line.
    pub line: usize,
}

/// Every [`IdSite`] of `text` (read under `scheme`), in text order: `id:`
/// when its value is a string, then each heading that carries a `{#…}`
/// attribute.
pub fn id_sites(text: &str, scheme: &IdScheme) -> Vec<IdSite> {
    let lines = LineIndex::new(text.as_bytes());
    let layout = front_matter::split(text);
    let mut sites = Vec::new();
    if let Some(block) = &layout.block {
        let front = front_matter::read(text, block, lines.line(block.yaml.start), scheme);
        if let Some((written, line)) = front.written_id {
            sites.push(IdSite {
                written: written.trim().to_owned(),
                level: None,
                offset: block.span.start,
                line,
            });
        }
    }
    let body = markdown::scan(text, layout.body);
    for heading in &body.headings {
        if let Some((written, _)) = &heading.id {
            sites.push(IdSite {
                written: written.trim().to_owned(),
                level: Some(heading.level),
                offset: heading.span.start,
                line: lines.line(heading.span.start),
            });
        }
    }
    sites
}

/// What one written ID is under a scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrittenId {
    /// No bare ID of the scheme (a plain anchor, an unconfigured prefix, a
    /// qualified, sectioned or revised reference): it defines nothing.
    NoId,
    /// A legacy `aliases_from` prefix, matched verbatim: the canonical ID
    /// (`PREFIX-body`).
    Alias { canonical: String },
    /// Look-alike characters or mixed scripts in it: their Latin fixes
    /// (offsets into the written text).
    LookAlike { homoglyphs: Vec<Homoglyph> },
    /// A bare Latin ID of a configured prefix.
    Id(String),
}

/// `written` (an [`IdSite`]'s text) read as one reference of `scheme`:
/// see [`WrittenId`]. An alias prefix is matched verbatim before any
/// look-alike is normalised (the grammar's rule).
pub fn written_id(written: &str, scheme: &IdScheme) -> WrittenId {
    let Some(found) = grammar::parse_reference(written, 0, scheme) else {
        return WrittenId::NoId;
    };
    let reference = &found.reference;
    if reference.project.is_some()
        || reference.scope.is_some()
        || reference.section.is_some()
        || reference.rev.is_some()
        || reference.form != RefForm::Bare
        || found.bad_rev.is_some()
    {
        return WrittenId::NoId;
    }
    if let Some(prefix) = &reference.alias_of {
        let body = reference.id.split_once('-').map_or("", |(_, body)| body);
        return WrittenId::Alias {
            canonical: format!("{prefix}-{body}"),
        };
    }
    if !found.homoglyphs.is_empty() || reference.script != IdScript::Latin {
        return WrittenId::LookAlike {
            homoglyphs: found.homoglyphs,
        };
    }
    WrittenId::Id(reference.id.clone())
}

/// New sections spliced into one node's span, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Sections {
    /// The target's span in the unpatched bytes.
    pub base_span: Span,
    /// The text spliced in ([`update_text`]): what a proposal stores as
    /// `new_text`.
    pub text: String,
    /// The patched bytes.
    pub bytes: Vec<u8>,
    /// Their fresh parse.
    pub parsed: ParsedFile,
    /// The added sections: their positions in `parsed.nodes`, in order.
    pub added: Vec<usize>,
}

/// Why new sections spliced into a node's span were refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionsError {
    /// The document's `id:` is another after the edit (added, dropped or
    /// changed).
    DocumentId {
        before: Option<String>,
        after: Option<String>,
    },
    /// A node of the file is not there after the edit.
    Dropped(StructureEntry),
    /// A node of the file at another heading level.
    Level { id: String, before: u8, after: u8 },
    /// A node of the file out of its order.
    Moved(StructureEntry),
    /// The target's span after the edit is not exactly the inserted text:
    /// a heading at the target's level or above inside it, or text running
    /// into what follows.
    SpanDiffers { expected: Span, found: Span },
    /// The text adds no `{#ID}` section.
    NoNewId,
}

impl fmt::Display for SectionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shown = |id: &Option<String>| match id {
            Some(id) => format!("`id: {id}`"),
            None => "no `id:`".to_owned(),
        };
        match self {
            Self::DocumentId { before, after } => write!(
                f,
                "the document had {}, would have {}: a create adds `{{#ID}}` sections to an \
                 existing file and never changes its `id:` (a new file is proposed at a free path)",
                shown(before),
                shown(after)
            ),
            Self::Dropped(entry) => write!(
                f,
                "{entry} is not in the text: a create keeps every `{{#ID}}` heading of the span \
                 and its level"
            ),
            Self::Level { id, before, after } => write!(
                f,
                "`{id}` was level {before}, would be level {after}: a create keeps every \
                 `{{#ID}}` heading of the span and its level"
            ),
            Self::Moved(entry) => write!(
                f,
                "{entry} moved: a create keeps the `{{#ID}}` headings of the span in their order"
            ),
            Self::SpanDiffers { expected, found } => write!(
                f,
                "the edit does not stay inside the node: the new text is bytes {}-{} but the node \
                 would span {}-{} (a heading at the node's level or above inside the text, or \
                 text running into what follows); new sections go below the node's level",
                expected.start, expected.end, found.start, found.end
            ),
            Self::NoNewId => f.write_str(
                "no new ID: the text adds no `{#ID}` section; `spec propose update` replaces a span",
            ),
        }
    }
}

impl std::error::Error for SectionsError {}

/// Node `ord` of `parsed` (the parse of `bytes`, named `path`) given
/// `new_text` that adds `{#ID}` sections inside its span: [`update_text`],
/// [`splice`], a fresh parse under `scheme`; the file's ordered (ID,
/// heading level) list kept as a subsequence, the document's `id:` the
/// same, the target spanning exactly the text, and at least one section
/// added, each inside it (so deeper than the target).
pub fn add_sections(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    ord: usize,
    new_text: &str,
    scheme: &IdScheme,
) -> Result<Sections, SectionsError> {
    let Some(node) = parsed.nodes.get(ord) else {
        return Err(SectionsError::NoNewId);
    };
    let base_span = node.span;
    let text = update_text(new_text, is_section(node));
    let patched = splice(bytes, base_span, text);
    let reparsed = crate::parse(path, &patched, scheme);
    let inserted = Span::new(base_span.start, base_span.start + text.len());
    let old = structure(parsed);
    let new = structure(&reparsed);
    let document = |list: &[StructureEntry]| list.first().and_then(|entry| entry.id.clone());
    if document(&old) != document(&new) {
        return Err(SectionsError::DocumentId {
            before: document(&old),
            after: document(&new),
        });
    }
    // The old list as a subsequence of the new one: what is left over was
    // added.
    let mut kept = 0;
    let mut added = Vec::new();
    for (at, entry) in new.iter().enumerate() {
        if old.get(kept) == Some(entry) {
            kept += 1;
        } else {
            added.push(at);
        }
    }
    if let Some(missing) = old.get(kept) {
        let elsewhere = new
            .iter()
            .filter(|entry| entry.id.is_some() && entry.id == missing.id);
        let mut moved = false;
        for entry in elsewhere {
            match (missing.level, entry.level) {
                (Some(before), Some(after)) if before != after => {
                    return Err(SectionsError::Level {
                        id: missing.id.clone().unwrap_or_default(),
                        before,
                        after,
                    });
                }
                _ => moved = true,
            }
        }
        return Err(if moved {
            SectionsError::Moved(missing.clone())
        } else {
            SectionsError::Dropped(missing.clone())
        });
    }
    let found = reparsed
        .nodes
        .get(ord)
        .map_or(Span::default(), |node| node.span);
    if found != inserted {
        return Err(SectionsError::SpanDiffers {
            expected: inserted,
            found,
        });
    }
    if let Some(&outside) = added.iter().find(|&&at| {
        at <= ord
            || reparsed
                .nodes
                .get(at)
                .is_none_or(|node| node.span.start < inserted.start || node.span.end > inserted.end)
    }) {
        let found = reparsed
            .nodes
            .get(outside)
            .map_or(Span::default(), |node| node.span);
        return Err(SectionsError::SpanDiffers {
            expected: inserted,
            found,
        });
    }
    if added.is_empty() {
        return Err(SectionsError::NoNewId);
    }
    Ok(Sections {
        base_span,
        text: text.to_owned(),
        bytes: patched,
        parsed: reparsed,
        added,
    })
}
