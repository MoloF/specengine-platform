//! The one reference grammar: text, front-matter scalars and `{#ID}`
//! definitions are read by the same lexer (`crates/specengine-model/README.md`,
//! "Reference grammar").
//!
//! ```text
//! reference = [ project ":" ] [ scope "/" ] id [ "#" id ] [ "@" rev ]
//! wiki      = "[[" reference [ "|" label ] "]]"
//! id        = prefix "-" body         ; configured prefix or aliases_from entry
//! body      = digit+                  ; shape "number"
//!           | alnum+ ( "-" alnum+ )*  ; shape "name", greedy; alnum = [A-Za-z0-9]
//! project   = slug ; scope = slug ; slug = [a-z][a-z0-9-]* ; rev = digit{1,9}
//! ```
//!
//! Recognition: (1) a candidate is a maximal letter-digit run followed by
//! `-` and not preceded by `_` or `-`; (2) the run matches an alias verbatim
//! first (no [`Homoglyph`], the body still normalised), else run and body
//! match after look-alike normalisation, and any normalised char is a
//! [`Homoglyph`]; (3) `#` and `@` join only before an ID
//! or 1–9 digits; (4) the right boundary is the end, or a char that is no
//! letter, digit, `_`, nor a `-` before a letter or digit; (5) qualifiers are
//! read by look-back, `slug/` then `slug:`, and kept only when the char before
//! them is none of letter, digit, `_ - . / :`; (6) the script class is that
//! of the ID as written. [`is_slug`] is the `slug` rule, also the stem of a
//! feature document (ADR-0026).
//!
//! Every scan is one pass: each byte is visited a bounded number of times,
//! whatever the input (no rescan from a line start per `[[`, `#`, `@`, `-`).

use crate::reference::{PathTarget, RefForm, Reference};
use crate::scheme::{IdScheme, PrefixSpec, Shape};
use crate::script::{IdScript, normalize_char};
use crate::span::Span;

/// A look-alike inside an ID: the span of the ID as written and its Latin text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Homoglyph {
    pub span: Span,
    pub fix: String,
}

/// One recognised reference with what the lexer noticed about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// `span` is always set here.
    pub reference: Reference,
    /// Look-alikes in the ID and in its `#` section.
    pub homoglyphs: Vec<Homoglyph>,
    /// `@` followed by digits that are no revision (more than nine, or glued
    /// to a letter): the reference stands without a revision.
    pub bad_rev: Option<Span>,
}

/// A definition (`id:`, `{#ID}`): a bare ID of a configured prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// The Latin ID.
    pub id: String,
    /// The kind of its prefix in the scheme.
    pub kind: String,
    /// Script class of the ID as written.
    pub script: IdScript,
    pub span: Span,
    pub homoglyph: Option<Homoglyph>,
}

/// The value of `canon:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Canon {
    Reference(Found),
    Path(PathTarget),
}

/// Every reference in `text`, in order. Spans are `base` + offset in `text`.
pub fn scan(text: &str, base: usize, scheme: &IdScheme) -> Vec<Found> {
    let mut found = Vec::new();
    if scheme.is_empty() {
        return found;
    }
    let bytes = text.as_bytes();
    let mut pos = 0;
    while let Some(c) = char_at(text, pos) {
        if !c.is_alphanumeric() {
            pos += c.len_utf8();
            continue;
        }
        // `pos` starts a maximal run: the char before it is no letter or digit.
        let run_end = run_end(text, pos);
        if bytes.get(run_end) == Some(&b'-')
            && !matches!(char_before(text, pos), Some('_' | '-'))
            && let Some((one, end)) = match_at(text, pos, run_end, scheme, base)
        {
            found.push(one);
            pos = end;
            continue;
        }
        pos = run_end;
    }
    found
}

/// `text` (trimmed) is exactly one reference: the form a reference-carrying
/// front-matter scalar must have.
pub fn parse_reference(text: &str, base: usize, scheme: &IdScheme) -> Option<Found> {
    let lead = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    let start = base + lead;
    let mut all = scan(trimmed, start, scheme);
    if all.len() != 1 {
        return None;
    }
    let one = all.pop()?;
    let span = one.reference.span?;
    (span == Span::new(start, start + trimmed.len())).then_some(one)
}

/// `text` is exactly a bare ID of a configured prefix (never an alias, no
/// qualifier, section or revision); look-alikes are normalised and reported.
pub fn parse_definition(text: &str, base: usize, scheme: &IdScheme) -> Option<Definition> {
    if !char_at(text, 0).is_some_and(char::is_alphanumeric) {
        return None;
    }
    let run_end = run_end(text, 0);
    let head = match_id(text, 0, run_end, scheme, false, base)?;
    if head.end != text.len() {
        return None;
    }
    Some(Definition {
        id: head.id,
        kind: head.spec.kind.clone(),
        script: head.script,
        span: Span::new(base, base + text.len()),
        homoglyph: head.homoglyph,
    })
}

/// `canon:` tries a reference, else `path[#anchor]` (a non-empty path, no
/// whitespace, at most one `#` followed by a non-empty anchor).
pub fn parse_canon(text: &str, base: usize, scheme: &IdScheme) -> Option<Canon> {
    if let Some(found) = parse_reference(text, base, scheme) {
        return Some(Canon::Reference(found));
    }
    let lead = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    let (path, anchor) = match trimmed.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (trimmed, None),
    };
    if path.is_empty() || anchor.is_some_and(|anchor| anchor.is_empty() || anchor.contains('#')) {
        return None;
    }
    Some(Canon::Path(PathTarget {
        path: path.to_owned(),
        anchor: anchor.map(str::to_owned),
        span: Some(Span::new(base + lead, base + lead + trimmed.len())),
    }))
}

/// `superseded-by X`: the offset of `X` in `status` and `X` itself; `None`
/// when `status` is not of that form.
pub fn split_superseded_by(status: &str) -> Option<(usize, &str)> {
    let rest = status.strip_prefix("superseded-by")?;
    let target = rest.trim_start();
    if target.len() == rest.len() || target.is_empty() {
        return None;
    }
    Some((status.len() - target.len(), target.trim_end()))
}

/// An ID recognised at a candidate run.
struct IdMatch<'s> {
    end: usize,
    id: String,
    alias_of: Option<String>,
    script: IdScript,
    spec: &'s PrefixSpec,
    homoglyph: Option<Homoglyph>,
}

/// The reference whose ID starts at the run `run_start..run_end`, and the
/// end of its whole occurrence.
fn match_at(
    text: &str,
    run_start: usize,
    run_end_at: usize,
    scheme: &IdScheme,
    base: usize,
) -> Option<(Found, usize)> {
    let bytes = text.as_bytes();
    let head = match_id(text, run_start, run_end_at, scheme, true, base)?;
    let mut end = head.end;
    let mut homoglyphs: Vec<Homoglyph> = head.homoglyph.into_iter().collect();

    // (3) `#` joins only before an ID.
    let mut section = None;
    if bytes.get(end) == Some(&b'#') && char_at(text, end + 1).is_some_and(char::is_alphanumeric) {
        let section_start = end + 1;
        let section_run = run_end(text, section_start);
        if let Some(part) = match_id(text, section_start, section_run, scheme, false, base)
            && (is_boundary(text, part.end) || bytes.get(part.end) == Some(&b'@'))
        {
            end = part.end;
            homoglyphs.extend(part.homoglyph);
            section = Some(part.id);
        }
    }

    // (3) `@` joins only before 1–9 digits.
    let mut rev = None;
    let mut bad_rev = None;
    if bytes.get(end) == Some(&b'@') {
        let digits_start = end + 1;
        let digits = bytes[digits_start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        let digits_end = digits_start + digits;
        if (1..=9).contains(&digits) && is_boundary(text, digits_end) {
            rev = text[digits_start..digits_end].parse().ok();
            end = digits_end;
        } else if digits > 0 {
            bad_rev = Some(Span::new(base + end, base + digits_end));
        }
    }

    // (4) the right boundary.
    if !is_boundary(text, end) {
        return None;
    }

    // (5) qualifiers by look-back.
    let mut start = run_start;
    let mut scope = None;
    let mut project = None;
    if run_start > 0 && bytes[run_start - 1] == b'/' {
        if let Some(slug) = slug_before(bytes, run_start - 1) {
            scope = Some(&text[slug..run_start - 1]);
            start = slug;
            if slug > 0
                && bytes[slug - 1] == b':'
                && let Some(project_start) = slug_before(bytes, slug - 1)
            {
                project = Some(&text[project_start..slug - 1]);
                start = project_start;
            }
        }
    } else if run_start > 0
        && bytes[run_start - 1] == b':'
        && let Some(project_start) = slug_before(bytes, run_start - 1)
    {
        project = Some(&text[project_start..run_start - 1]);
        start = project_start;
    }
    if start != run_start
        && char_before(text, start)
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':'))
    {
        start = run_start;
        scope = None;
        project = None;
    }

    // The wiki form around the whole reference.
    let mut form = RefForm::Bare;
    let mut label = None;
    let mut occurrence = (start, end);
    if start >= 2 && &bytes[start - 2..start] == b"[[" {
        if bytes[end..].starts_with(b"]]") {
            form = RefForm::Wiki;
            occurrence = (start - 2, end + 2);
        } else if bytes.get(end) == Some(&b'|') {
            // The label runs to `]]` on the same line; a `[` or a lone `]`
            // ends the attempt, so label scans never overlap.
            let label_start = end + 1;
            let mut at = label_start;
            while let Some(&byte) = bytes.get(at) {
                match byte {
                    b']' => {
                        if bytes.get(at + 1) == Some(&b']') {
                            form = RefForm::Wiki;
                            let text = &text[label_start..at];
                            label = (!text.is_empty()).then(|| text.to_owned());
                            occurrence = (start - 2, at + 2);
                        }
                        break;
                    }
                    b'[' | b'\n' | b'\r' => break,
                    _ => at += 1,
                }
            }
        }
    }

    let reference = Reference {
        id: head.id,
        alias_of: head.alias_of,
        script: head.script,
        project: project.map(str::to_owned),
        scope: scope.map(str::to_owned),
        section,
        rev,
        form,
        label,
        span: Some(Span::new(base + occurrence.0, base + occurrence.1)),
    };
    Some((
        Found {
            reference,
            homoglyphs,
            bad_rev,
        },
        occurrence.1,
    ))
}

/// (1)–(2): the ID whose prefix is the run `run_start..run_end_at`.
fn match_id<'s>(
    text: &str,
    run_start: usize,
    run_end_at: usize,
    scheme: &'s IdScheme,
    allow_alias: bool,
    base: usize,
) -> Option<IdMatch<'s>> {
    if text.as_bytes().get(run_end_at) != Some(&b'-') {
        return None;
    }
    let run = &text[run_start..run_end_at];
    let limit = scheme.max_run_chars();
    if run.len() > limit.saturating_mul(4) || run.chars().count() > limit {
        return None;
    }
    let mut changed = false;
    // An alias match is never a homoglyph (model README, Recognition (2)):
    // its body is normalised into `id` without being reported.
    let (spec, alias_of, mut id) = match allow_alias.then(|| scheme.alias(run)).flatten() {
        Some(spec) => (spec, Some(spec.prefix.clone()), run.to_owned()),
        None => {
            let mut normalized = String::with_capacity(run.len());
            for c in run.chars() {
                let latin = normalize_char(c);
                changed |= latin != c;
                normalized.push(latin);
            }
            let spec = scheme.prefix(&normalized)?;
            (spec, None, normalized)
        }
    };
    id.push('-');
    let body_start = run_end_at + 1;
    let body_end = match spec.shape {
        Shape::Number => {
            let mut at = body_start;
            for c in text[body_start..].chars() {
                let latin = normalize_char(c);
                if !latin.is_ascii_digit() {
                    break;
                }
                changed |= latin != c;
                id.push(latin);
                at += c.len_utf8();
            }
            at
        }
        Shape::Name => name_body(text, body_start, &mut id, &mut changed),
    };
    if body_end == body_start {
        return None;
    }
    let homoglyph = (changed && alias_of.is_none()).then(|| Homoglyph {
        span: Span::new(base + run_start, base + body_end),
        fix: id.clone(),
    });
    Some(IdMatch {
        end: body_end,
        script: IdScript::of(&text[run_start..body_end]),
        id,
        alias_of,
        spec,
        homoglyph,
    })
}

/// `alnum+ ("-" alnum+)*`, greedy, after normalisation; appends the Latin
/// body to `id` and returns its end.
fn name_body(text: &str, start: usize, id: &mut String, changed: &mut bool) -> usize {
    let bytes = text.as_bytes();
    let mut at = start;
    let mut end = start;
    loop {
        let segment = at;
        let before = id.len();
        if segment != start {
            id.push('-');
        }
        for c in text[at..].chars() {
            let latin = normalize_char(c);
            if !latin.is_ascii_alphanumeric() {
                break;
            }
            *changed |= latin != c;
            id.push(latin);
            at += c.len_utf8();
        }
        if at == segment {
            id.truncate(before);
            break;
        }
        end = at;
        let continues = bytes.get(at) == Some(&b'-')
            && char_at(text, at + 1).is_some_and(|c| normalize_char(c).is_ascii_alphanumeric());
        if !continues {
            break;
        }
        at += 1;
    }
    end
}

/// (4): the end, or a char that is no letter, digit, `_`, nor a `-` before
/// a letter or digit.
fn is_boundary(text: &str, at: usize) -> bool {
    match char_at(text, at) {
        None => true,
        Some('_') => false,
        Some('-') => !char_at(text, at + 1).is_some_and(char::is_alphanumeric),
        Some(c) => !c.is_alphanumeric(),
    }
}

/// `text` is a `slug`, `[a-z][a-z0-9-]*`: the one rule for the `project:`
/// and `slug/` qualifiers and for the stem of a feature document
/// (ADR-0026).
pub fn is_slug(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.first().is_some_and(|&first| is_slug_start(first))
        && bytes.iter().all(|&byte| is_slug_byte(byte))
}

/// The first byte of a slug.
fn is_slug_start(byte: u8) -> bool {
    byte.is_ascii_lowercase()
}

/// Any byte of a slug.
fn is_slug_byte(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-')
}

/// Start of the slug that ends right before `separator`: the maximal run of
/// slug bytes, which must start as a slug does ([`is_slug`]).
fn slug_before(bytes: &[u8], separator: usize) -> Option<usize> {
    let mut start = separator;
    while start > 0 && is_slug_byte(bytes[start - 1]) {
        start -= 1;
    }
    (start < separator && is_slug_start(bytes[start])).then_some(start)
}

/// End of the maximal letter-digit run starting at `start`.
fn run_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for c in text[start..].chars() {
        if !c.is_alphanumeric() {
            break;
        }
        end += c.len_utf8();
    }
    end
}

fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}

fn char_before(text: &str, at: usize) -> Option<char> {
    text.get(..at)?.chars().next_back()
}
