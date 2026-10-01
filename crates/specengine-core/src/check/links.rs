//! Markdown file links (docs/features/spec-check-links.md), two warnings
//! over the `mentions` path links of live sources (neither
//! `class: generated` nor Tier 3; a failed front-matter is live). Warnings
//! never block (`#control`).
//!
//! Checked: a destination whose percent-decoded path ends exactly in the
//! document extension, or an empty path with an anchor (the linking file
//! itself); every other one is recorded by the parser and never checked.
//! Candidates, normalised (empty and `.` components dropped, `..` pops,
//! popping above the root leaves it): a `/`-led path gives one, from the
//! root; else the linking file's directory + path, and only when that names
//! no walked document, `[paths] link_base` + path (when configured). The
//! first candidate naming a walked document resolves.
//!
//! - `link-dangling`: no candidate resolves and at least one lies in the
//!   walk scope ([`WalkScope::in_walk_scope`]); a target outside it is
//!   nothing.
//! - `link-anchor`: the link resolves, its anchor (percent-decoded) is
//!   neither an anchor (slug, attr, html) nor a section ID of the target —
//!   the `canon-anchor` predicate ([`has_anchor`]). A target without a
//!   parse is not checked.
//!
//! The base comes only from the config: there is no default (ADR-0008).

use std::collections::BTreeMap;

use specengine_model::{DiagnosticCode, LinkOrigin, LinkTarget, MENTIONS, ParsedFile, PathTarget};

use super::engine::{Corpus, has_anchor};
use super::graph::{live_sources, warning};
use super::report::Finding;
use super::text::FileText;
use crate::{DOCUMENT_EXTENSION, Paths, WalkScope};

/// The two warnings.
pub(crate) fn run(corpus: &Corpus<'_>, paths: &Paths, findings: &mut Vec<Finding>) {
    let links = Links {
        corpus,
        scope: paths.walk_scope(),
        link_base: paths.link_base.as_deref(),
    };
    for (index, parsed) in live_sources(corpus) {
        for target in file_links(parsed) {
            links.check(index, target, findings);
        }
    }
}

/// What the link rules need: the corpus, the walk scope compiled once, the
/// configured base.
struct Links<'c, 'a> {
    corpus: &'c Corpus<'a>,
    scope: WalkScope,
    link_base: Option<&'c str>,
}

/// The Markdown file links of a parse, in text order.
fn file_links(parsed: &ParsedFile) -> impl Iterator<Item = &PathTarget> {
    parsed.links.iter().filter_map(|link| match &link.dst {
        LinkTarget::Path(target)
            if link.origin == LinkOrigin::Inline && link.link_type == MENTIONS =>
        {
            Some(target)
        }
        _ => None,
    })
}

impl Links<'_, '_> {
    /// One link of the file at `index`.
    fn check(&self, index: usize, target: &PathTarget, findings: &mut Vec<Finding>) {
        let corpus = self.corpus;
        let text = &corpus.texts[index];
        let from = corpus.paths[index];
        let path = percent_decode(&target.path);
        let resolved = if path.is_empty() {
            if target.anchor.is_none() {
                return;
            }
            index
        } else if path.ends_with(DOCUMENT_EXTENSION) {
            match self.resolve(from, &path) {
                Ok(resolved) => resolved,
                Err(tried) => {
                    let tried: Vec<&str> = tried.iter().flatten().map(String::as_str).collect();
                    if tried
                        .iter()
                        .any(|candidate| self.scope.in_walk_scope(candidate))
                    {
                        let written = written(text, target);
                        let tried = tried
                            .iter()
                            .map(|candidate| format!("`{candidate}`"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        findings.push(warning(
                            "link-dangling",
                            from,
                            line(text, target),
                            &written,
                            format!("`{written}` names no walked document (tried {tried})"),
                        ));
                    }
                    return;
                }
            }
        } else {
            return;
        };
        let Some(anchor) = &target.anchor else {
            return;
        };
        let Some(parsed) = corpus.parses[resolved].filter(|parsed| was_read(parsed)) else {
            return;
        };
        let anchor = percent_decode(anchor);
        if !has_anchor(parsed, &corpus.resolver.sections[resolved], &anchor) {
            let written = written(text, target);
            findings.push(warning(
                "link-anchor",
                from,
                line(text, target),
                &written,
                format!(
                    "`{written}`: `{}` has no anchor or section `#{anchor}`",
                    corpus.paths[resolved]
                ),
            ));
        }
    }

    /// The walked file a checked, non-empty `path` (percent-decoded) names,
    /// else the candidates tried, in order (`None`: one leaving the root).
    fn resolve(&self, from: &str, path: &str) -> Result<usize, Vec<Option<String>>> {
        resolve_link_path(&self.corpus.by_path, self.link_base, from, path)
    }
}

/// The file a checked, non-empty `path` (percent-decoded) of a Markdown
/// file link written in `from` names among the walked files `by_path`,
/// else the candidates tried, in order (`None`: one leaving the root).
/// Also the spec graph's.
pub(crate) fn resolve_link_path(
    by_path: &BTreeMap<&str, usize>,
    link_base: Option<&str>,
    from: &str,
    path: &str,
) -> Result<usize, Vec<Option<String>>> {
    let walked = |candidate: &Option<String>| {
        candidate
            .as_deref()
            .and_then(|candidate| by_path.get(candidate).copied())
    };
    let mut tried = Vec::with_capacity(2);
    if let Some(from_root) = path.strip_prefix('/') {
        let only = normalise("", from_root);
        if let Some(found) = walked(&only) {
            return Ok(found);
        }
        tried.push(only);
        return Err(tried);
    }
    let directory = from.rsplit_once('/').map_or("", |(directory, _)| directory);
    let relative = normalise(directory, path);
    if let Some(found) = walked(&relative) {
        return Ok(found);
    }
    tried.push(relative);
    if let Some(base) = link_base {
        let from_base = normalise(base, path);
        if let Some(found) = walked(&from_base) {
            return Ok(found);
        }
        // The same path twice is tried and listed once.
        if !tried.contains(&from_base) {
            tried.push(from_base);
        }
    }
    Err(tried)
}

/// `directory` + `path`, `/`-joined: empty and `.` components dropped,
/// `..` pops; `None` when it pops above the root.
fn normalise(directory: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for component in directory.split('/').chain(path.split('/')) {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

/// `%XX` → that byte; a malformed `%` stays; a result that is not UTF-8
/// gives the text as written (the census's rule).
pub(crate) fn percent_decode(text: &str) -> String {
    if !text.contains('%') {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(at + 1).and_then(|&byte| hex_value(byte)),
                bytes.get(at + 2).and_then(|&byte| hex_value(byte)),
            )
        {
            decoded.push((high << 4) | low);
            at += 3;
            continue;
        }
        decoded.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| text.to_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// The target has a parse whose body was read (not the empty parse of a
/// file that is not UTF-8): its anchors can be judged.
pub(crate) fn was_read(parsed: &ParsedFile) -> bool {
    parsed
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != DiagnosticCode::NotUtf8)
}

/// The destination as written: the text under its span, else rebuilt from
/// the path and the anchor.
pub(crate) fn written(text: &FileText<'_>, target: &PathTarget) -> String {
    if let Some(span) = target.span
        && !text.is_empty()
    {
        let written = text.text(span);
        if !written.is_empty() {
            return written;
        }
    }
    match &target.anchor {
        Some(anchor) => format!("{}#{anchor}", target.path),
        None => target.path.clone(),
    }
}

/// The line of the destination: its span's, 1 without bytes.
pub(crate) fn line(text: &FileText<'_>, target: &PathTarget) -> usize {
    target
        .span
        .filter(|_| !text.is_empty())
        .map_or(1, |span| text.line(span.start))
}
