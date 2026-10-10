//! The input of a node's meaning hash, recipe `specengine-norm/v1` (task
//! spec `norm-hash` "The recipe"; `spec.lock`'s `spec_hash`, 05 §3.5).
//!
//! It sees only spans and links: a node's own text (its body minus nested
//! ID sections; a document's also minus its first H1's lines, never a
//! heading line of its own), normalised, and for a document the links its
//! front-matter declares, rendered from their parsed values. No key, kind
//! or prefix is named here, no path, root or time enters the bytes, and
//! nothing is hashed or read: the caller hashes the bytes
//! (`specengine_store::b3_hash`).
//!
//! ```text
//! frame(x) = u32le(len x) || x
//! input    = frame(NORM_RECIPE) || u32le(link count) || frame(link)... || frame(text)
//! ```

use std::borrow::Cow;

use specengine_model::{DiagnosticCode, Link, LinkOrigin, LinkTarget, ParsedFile, Span};

use crate::markdown;
use crate::own_text::own_spans;

/// The recipe's name, the first frame of every input.
pub const NORM_RECIPE: &str = "specengine-norm/v1";

/// The bytes whose `b3_hash` is node `ord`'s `norm_hash`; `parsed` is the
/// parse of `bytes`. `None` is a null hash: no node at `ord` (a file that
/// is not UTF-8 has none); a document whose file carries a finding that
/// may have lost a declared link (`frontmatter-unclosed`,
/// `frontmatter-yaml`, `frontmatter-not-mapping`, `frontmatter-type`,
/// `unparsed-reference`), its sections keeping theirs; a span that is no
/// range of `bytes` or not UTF-8 (a parse of other bytes); a piece over
/// 4 GiB.
///
/// Linear in the node's own spans and links: only the bytes of its own
/// pieces are read as UTF-8, never the whole file per node, so every node
/// of a file costs linear time in the file. A document also re-reads its
/// body up to its first H1.
pub fn norm_input(parsed: &ParsedFile, bytes: &[u8], ord: usize) -> Option<Vec<u8>> {
    let node = parsed.nodes.get(ord)?;
    let mut spans = own_spans(parsed, ord);
    let mut links = Vec::new();
    if ord == 0 {
        if parsed.diagnostics.iter().any(|found| nulls(found.code)) {
            return None;
        }
        if let Some(lines) = title_lines(bytes, parsed)? {
            spans = without(spans, lines);
        }
        links = parsed
            .links
            .iter()
            .filter(|link| declared_by(link, node.id.as_deref()))
            .map(rendered)
            .collect();
        // Byte order (`str`'s `Ord`), each rendering once.
        links.sort_unstable();
        links.dedup();
    }
    let mut pieces = Vec::with_capacity(spans.len());
    for span in spans {
        pieces.push(std::str::from_utf8(bytes.get(span.range())?).ok()?);
    }
    let normalized = normalized(&pieces);

    let mut input = Vec::new();
    frame(&mut input, NORM_RECIPE.as_bytes())?;
    input.extend_from_slice(&u32le(links.len())?);
    for link in &links {
        frame(&mut input, link.as_bytes())?;
    }
    frame(&mut input, normalized.as_bytes())?;
    Some(input)
}

/// A finding after which a document's declared links may be incomplete.
fn nulls(code: DiagnosticCode) -> bool {
    matches!(
        code,
        DiagnosticCode::FrontmatterUnclosed
            | DiagnosticCode::FrontmatterYaml
            | DiagnosticCode::FrontmatterNotMapping
            | DiagnosticCode::FrontmatterType
            | DiagnosticCode::UnparsedReference
    )
}

/// The document's first H1 as the parser finds it (re-read with the same
/// reader, never through slug anchors), when it is no ID section's
/// heading: from the start of its first line through the line ending of
/// its last. `Some(None)` without one; `None` when the body is no UTF-8
/// range of `bytes`. The body is read as UTF-8 once, here.
fn title_lines(bytes: &[u8], parsed: &ParsedFile) -> Option<Option<Span>> {
    let body = std::str::from_utf8(bytes.get(parsed.body.range())?).ok()?;
    let Some(heading) = markdown::first_h1(body, parsed.body.start) else {
        return Some(None);
    };
    if parsed
        .sections()
        .iter()
        .any(|section| section.heading == Some(heading))
    {
        return Some(None);
    }
    let floor = parsed.body.start.min(heading.start);
    let start = bytes
        .get(floor..heading.start)?
        .iter()
        .rposition(|&byte| byte == b'\n' || byte == b'\r')
        .map_or(floor, |at| floor + at + 1);
    let ceiling = parsed.body.end.max(heading.end);
    let rest = bytes.get(heading.end..ceiling)?;
    let end = match rest.iter().position(|&byte| byte == b'\n' || byte == b'\r') {
        Some(at) if rest[at..].starts_with(b"\r\n") => heading.end + at + 2,
        Some(at) => heading.end + at + 1,
        None => ceiling,
    };
    Some(Some(Span::new(start, end)))
}

/// `spans` minus `cut`: each span's parts before and after it, none empty.
fn without(spans: Vec<Span>, cut: Span) -> Vec<Span> {
    let mut kept = Vec::with_capacity(spans.len() + 1);
    for span in spans {
        if cut.end <= span.start || span.end <= cut.start {
            kept.push(span);
            continue;
        }
        if span.start < cut.start {
            kept.push(Span::new(span.start, cut.start));
        }
        if cut.end < span.end {
            kept.push(Span::new(cut.end, span.end));
        }
    }
    kept
}

/// A link node 0's front-matter declares: origin front-matter, `src` its
/// ID (both absent included), and `src` not written elsewhere (the
/// `supersedes` link a `status: superseded-by X` gives has `src` X).
fn declared_by(link: &Link, id: Option<&str>) -> bool {
    link.origin == LinkOrigin::Frontmatter && link.src_span.is_none() && link.src.as_deref() == id
}

/// `<type> ref:[<project>:][<scope>/]<id>[#<section>][@<rev>]` or
/// `<type> path:<path>[#<anchor>]`, from the parsed value: the ID Latin (an
/// alias prefix as written), the revision decimal, a wiki label and form
/// dropped.
fn rendered(link: &Link) -> String {
    let mut out = String::with_capacity(link.link_type.len() + 32);
    out.push_str(&link.link_type);
    match &link.dst {
        LinkTarget::Reference(reference) => {
            out.push_str(" ref:");
            if let Some(project) = &reference.project {
                out.push_str(project);
                out.push(':');
            }
            if let Some(scope) = &reference.scope {
                out.push_str(scope);
                out.push('/');
            }
            out.push_str(&reference.id);
            if let Some(section) = &reference.section {
                out.push('#');
                out.push_str(section);
            }
            if let Some(rev) = reference.rev {
                out.push('@');
                out.push_str(&rev.to_string());
            }
        }
        LinkTarget::Path(target) => {
            out.push_str(" path:");
            out.push_str(&target.path);
            if let Some(anchor) = &target.anchor {
                out.push('#');
                out.push_str(anchor);
            }
        }
    }
    out
}

/// The normalisation, in order: each piece's CRLF, then lone CR, to LF;
/// the pieces joined with LF; trailing Unicode `White_Space` trimmed per
/// line; runs of empty lines to one; leading and trailing empty lines
/// dropped; no final LF. Indentation, tabs, inner spaces, Markdown, HTML
/// comments, code blocks and the Unicode form are kept.
///
/// The line endings are made LF inside each piece before the join, so a
/// piece ending in a lone CR stays two lines from the next, as it does in
/// the file's LF form.
fn normalized(pieces: &[&str]) -> String {
    let mut out = String::with_capacity(pieces.iter().map(|piece| piece.len()).sum());
    // An empty line seen since the last line kept.
    let mut blank = false;
    for &piece in pieces {
        // A copy only for a piece holding a CR.
        let piece: Cow<'_, str> = if piece.contains('\r') {
            Cow::Owned(piece.replace("\r\n", "\n").replace('\r', "\n"))
        } else {
            Cow::Borrowed(piece)
        };
        for line in piece.split('\n') {
            let line = line.trim_end();
            if line.is_empty() {
                blank = true;
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
                if blank {
                    out.push('\n');
                }
            }
            blank = false;
            out.push_str(line);
        }
        // The join's LF: the next piece starts a line of its own.
    }
    out
}

/// Appends `u32le(len bytes) || bytes`; `None` past `u32::MAX` bytes.
fn frame(input: &mut Vec<u8>, bytes: &[u8]) -> Option<()> {
    input.extend_from_slice(&u32le(bytes.len())?);
    input.extend_from_slice(bytes);
    Some(())
}

fn u32le(len: usize) -> Option<[u8; 4]> {
    u32::try_from(len).ok().map(u32::to_le_bytes)
}
