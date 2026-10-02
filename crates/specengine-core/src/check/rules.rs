//! The project's process rules (task spec `spec-check-process`, ADR-0031):
//! each `[[check.rules]]` entry over the documents it selects. The core
//! knows no kind, key or label (`#universal`): each comes from the entry;
//! only the four class names are the convention's (ADR-0022).
//!
//! - Judged: documents only, never a `{#ID}` section alone; not a file whose
//!   front-matter failed, nor a `class: generated` one; a Tier 3 document
//!   when selected.
//! - Selected by every selector the entry gives (`kinds`: the document's
//!   kind, declared else its prefix's; `classes`; `paths`, globs over the
//!   root-relative path) and by its `when`: each key written as a string
//!   equal to one of its strings (absent or not a string: not selected).
//! - Requirements: `keys` (`key-missing` at line 1, `key-empty` at the key),
//!   `values` (`value-invalid` at the key, one per string outside the list,
//!   one for a value that is no string; empty or absent: none), `parts`
//!   (`part-missing` at line 1, `part-empty` at the heading or lead-in) and
//!   `text = true` (`text-empty` at line 1). Without bytes, `parts` and
//!   `text` judge nothing.
//! - Keys and values are the front-matter as written ([`written_keys`],
//!   [`written_values`]); parts and own text come through the parse's
//!   Markdown reader ([`markdown::outline`]) and the one own-text split
//!   ([`crate::own_spans`]).
//! - Findings equal but for severity are one, an error over a warning; no
//!   message names a rule, so rule order never shows.

use std::collections::BTreeMap;
use std::ops::Range;

use specengine_model::{LinkOrigin, LinkTarget, Node, ParsedFile, Severity};

use super::config::{CheckRule, DocClass};
use super::engine::{Corpus, written_keys, written_values};
use super::input::CheckFile;
use super::report::Finding;
use super::text::{FileText, Written, front_matter_failed};
use crate::glob::Glob;
use crate::markdown::{self, Outline};

/// A rule with its globs compiled and its labels slugged.
struct Compiled<'r> {
    rule: &'r CheckRule,
    globs: Vec<Glob>,
    /// (label as written, its slug).
    parts: Vec<(&'r str, String)>,
}

/// A finding of one file before its severity: (line, code, subject,
/// message).
type Found = (usize, &'static str, String, String);

/// Every rule over every judged document of `corpus`.
pub(super) fn run(corpus: &Corpus<'_>, rules: &[CheckRule], findings: &mut Vec<Finding>) {
    if rules.is_empty() {
        return;
    }
    let compiled: Vec<Compiled<'_>> = rules
        .iter()
        .map(|rule| Compiled {
            rule,
            globs: rule.paths.iter().map(|glob| Glob::new(glob)).collect(),
            parts: rule
                .parts
                .iter()
                .map(|label| (label.as_str(), markdown::slug(label)))
                .collect(),
        })
        .collect();
    for (index, file) in corpus.files.iter().enumerate() {
        let Some(parsed) = corpus.parses[index] else {
            continue;
        };
        if front_matter_failed(parsed) {
            continue;
        }
        let Some(document) = parsed.document() else {
            continue;
        };
        let class = document
            .fields
            .as_ref()
            .and_then(|fields| fields.class.as_deref())
            .and_then(DocClass::parse);
        if class == Some(DocClass::Generated) {
            continue;
        }
        let mut judged = Judged {
            file,
            parsed,
            document,
            text: &corpus.texts[index],
            class,
            keys: None,
            values: None,
            body: None,
        };
        let mut found: BTreeMap<Found, Severity> = BTreeMap::new();
        for rule in &compiled {
            if judged.selects(rule) {
                judged.judge(rule, &mut found);
            }
        }
        for ((line, code, subject, message), severity) in found {
            findings.push(Finding {
                code: code.to_owned(),
                severity,
                path: file.path.clone(),
                line,
                subject,
                message,
                fix: None,
                debt: None,
                introduced: None,
            });
        }
    }
}

/// One judged document; what the rules read of it is read once, when first
/// needed.
struct Judged<'r, 'a> {
    file: &'r CheckFile,
    parsed: &'r ParsedFile,
    document: &'r Node,
    text: &'r FileText<'a>,
    class: Option<DocClass>,
    keys: Option<Vec<(String, usize)>>,
    values: Option<Vec<(String, Written)>>,
    /// The body's text and outline; `Some(None)`: no bytes (or not UTF-8),
    /// so `parts` and `text` judge nothing.
    body: Option<Option<(&'r str, Outline)>>,
}

impl Judged<'_, '_> {
    /// The written keys, with lines.
    fn keys(&mut self) -> &[(String, usize)] {
        self.keys
            .get_or_insert_with(|| written_keys(self.text, self.document))
    }

    /// The value of `key` as written; `None` when not written.
    fn value(&mut self, key: &str) -> Option<&Written> {
        self.values
            .get_or_insert_with(|| written_values(self.text, self.document))
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// The line of written key `key`; 1 when unknown.
    fn key_line(&mut self, key: &str) -> usize {
        self.keys()
            .iter()
            .find(|(name, _)| name == key)
            .map_or(1, |&(_, line)| line)
    }

    fn read_body(&mut self) {
        if self.body.is_none() {
            let bytes: &[u8] = &self.file.bytes;
            let text = (!bytes.is_empty())
                .then(|| std::str::from_utf8(bytes).ok())
                .flatten();
            self.body = Some(text.map(|text| (text, markdown::outline(text, self.parsed.body))));
        }
    }

    fn selects(&mut self, compiled: &Compiled<'_>) -> bool {
        let rule = compiled.rule;
        if !rule.kinds.is_empty()
            && !self
                .document
                .kind
                .as_ref()
                .is_some_and(|kind| rule.kinds.contains(kind))
        {
            return false;
        }
        if !rule.classes.is_empty()
            && !self
                .class
                .is_some_and(|class| rule.classes.contains(&class))
        {
            return false;
        }
        if !compiled.globs.is_empty()
            && !compiled
                .globs
                .iter()
                .any(|glob| glob.matches(&self.file.path))
        {
            return false;
        }
        rule.when.iter().all(|(key, allowed)| {
            matches!(self.value(key), Some(Written::Str(value)) if allowed.contains(value))
        })
    }

    fn judge(&mut self, compiled: &Compiled<'_>, found: &mut BTreeMap<Found, Severity>) {
        let rule = compiled.rule;
        let mut push = |finding: Found| {
            found
                .entry(finding)
                .and_modify(|severity| *severity = (*severity).min(rule.severity))
                .or_insert(rule.severity);
        };
        for key in &rule.keys {
            if !self.keys().iter().any(|(name, _)| name == key) {
                push((
                    1,
                    "key-missing",
                    key.clone(),
                    format!("key `{key}` is required by a check rule"),
                ));
            } else if self.value(key).is_some_and(Written::is_empty) {
                let line = self.key_line(key);
                push((
                    line,
                    "key-empty",
                    key.clone(),
                    format!("key `{key}` is empty; a check rule requires it filled"),
                ));
            }
        }
        for (key, allowed) in &rule.values {
            let Some(value) = self.value(key).cloned() else {
                continue;
            };
            if value.is_empty() {
                continue;
            }
            let line = self.key_line(key);
            let invalid = |shown: Option<&str>| {
                let message = match shown {
                    Some(shown) => {
                        format!("key `{key}` is `{shown}`; allowed: {}", allowed.join(", "))
                    }
                    None => format!("key `{key}` is not a string"),
                };
                (line, "value-invalid", key.clone(), message)
            };
            match &value {
                Written::Str(written) => {
                    if !allowed.contains(written) {
                        push(invalid(Some(written)));
                    }
                }
                Written::Seq(items) => {
                    for item in items {
                        match item {
                            Written::Str(written) if allowed.contains(written) => {}
                            Written::Str(written) => push(invalid(Some(written))),
                            _ => push(invalid(None)),
                        }
                    }
                }
                Written::Null | Written::Scalar | Written::Map { .. } => push(invalid(None)),
            }
        }
        if compiled.parts.is_empty() && !rule.text {
            return;
        }
        self.read_body();
        let Some(Some((text, outline))) = &self.body else {
            return;
        };
        for (label, slug) in &compiled.parts {
            match outline.labels.iter().find(|place| place.slug == *slug) {
                None => push((
                    1,
                    "part-missing",
                    (*label).to_owned(),
                    format!("part `{label}` is missing"),
                )),
                Some(place) if !outline.filled(&place.content) => push((
                    self.text.line(place.start),
                    "part-empty",
                    (*label).to_owned(),
                    format!("part `{label}` is empty"),
                )),
                Some(_) => {}
            }
        }
        if rule.text && !own_text_filled(self.parsed, text, outline) {
            push((
                1,
                "text-empty",
                self.document.id.clone().unwrap_or_default(),
                "no own text: only headings, links or references".to_owned(),
            ));
        }
    }
}

/// The document's own text ([`crate::own_spans`]: its body minus nested ID
/// sections) holds a letter or digit in a text run once heading text, HTML,
/// link and image text, wiki links (`[[…]]`) and the parse's inline
/// mentions are dropped.
fn own_text_filled(parsed: &ParsedFile, text: &str, outline: &Outline) -> bool {
    let own = crate::own_spans(parsed, 0);
    let meets_own = |drop: &Range<usize>| {
        own.iter()
            .any(|span| drop.start < span.end && span.start < drop.end)
    };
    let mut dropped: Vec<Range<usize>> = parsed
        .links
        .iter()
        .filter(|link| link.origin == LinkOrigin::Inline)
        .filter_map(|link| match &link.dst {
            LinkTarget::Reference(reference) => reference.span.map(|span| span.range()),
            LinkTarget::Path(_) => None,
        })
        .filter(|drop| meets_own(drop))
        .collect();
    for span in &own {
        dropped.extend(wiki_links(text, span.range()));
    }
    let dropped = disjoint(dropped);
    outline.runs.iter().filter(|run| !run.in_link).any(|run| {
        own.iter().any(|span| {
            let start = run.range.start.max(span.start);
            let end = run.range.end.min(span.end);
            if start >= end {
                return false;
            }
            // The dropped ranges meeting `start..end`: ends ascend as
            // starts do, so they are one run of the list.
            let first = dropped.partition_point(|drop| drop.end <= start);
            let count = dropped[first..].partition_point(|drop| drop.start < end);
            let cuts = &dropped[first..first + count];
            let whole = start == run.range.start && end == run.range.end;
            if whole && cuts.is_empty() {
                return run.alnum;
            }
            outside(text, start..end, cuts)
                .iter()
                .any(|piece| piece.chars().any(char::is_alphanumeric))
        })
    })
}

/// `ranges` sorted by start, overlapping or adjacent ones merged: the same
/// bytes covered, in ascending, disjoint ranges (an empty range inside a
/// gap kept, so it still splits the text around it).
fn disjoint(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// The pieces of `text[range]` outside every cut; `cuts` ascending and
/// disjoint ([`disjoint`]), each meeting `range`.
fn outside<'t>(text: &'t str, range: Range<usize>, cuts: &[Range<usize>]) -> Vec<&'t str> {
    let mut pieces = Vec::new();
    let mut cursor = range.start;
    for cut in cuts {
        if cut.start > cursor
            && let Some(piece) = text.get(cursor..cut.start)
        {
            pieces.push(piece);
        }
        cursor = cursor.max(cut.end);
    }
    if cursor < range.end
        && let Some(piece) = text.get(cursor..range.end)
    {
        pieces.push(piece);
    }
    pieces
}

/// The wiki links (`[[` … `]]` on one line) inside `text[range]`, as file
/// ranges.
fn wiki_links(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let Some(source) = text.get(range.clone()) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    let mut at = 0;
    // The end of the line holding the last `[[`; searched again only when
    // a `[[` lies past it (a `[[` never spans a newline).
    let mut line_end = 0;
    while let Some(open) = source[at..].find("[[") {
        let inner = at + open + 2;
        if inner > line_end {
            line_end = source[inner..]
                .find('\n')
                .map_or(source.len(), |newline| inner + newline);
        }
        match source[inner..line_end].find("]]") {
            Some(close) => {
                let end = inner + close + 2;
                links.push(range.start + at + open..range.start + end);
                at = end;
            }
            // No `]]` before the line's end: no later `[[` of the line
            // closes either.
            None => at = line_end,
        }
    }
    links
}
