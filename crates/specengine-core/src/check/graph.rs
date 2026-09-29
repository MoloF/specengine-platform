//! The graph warnings, over the parse the check already has; a live source
//! is neither `class: generated` nor Tier 3 (a document whose front-matter
//! failed is live). Warnings never block (`#control`).
//!
//! - `mention-dangling`: an inline mention that resolves to nothing, with
//!   the name-shape fallback ([`Resolver::resolve_mention`]); front-matter
//!   references keep `ref-dangling`.
//! - `depends-cycle`: one finding per strongly connected component of the
//!   `links.depends_on` graph between documents (≥ 2 documents, or one with
//!   an edge to itself).
//! - `ref-superseded`: a reference to a document whose `status:` is
//!   `superseded-by X`; `supersedes` items, the `status:` value and
//!   references from X's own files are exempt.

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use specengine_model::grammar;
use specengine_model::{
    IdScheme, LinkOrigin, LinkTarget, MENTIONS, ParsedFile, Reference, Severity,
};

use super::engine::Corpus;
use super::render::{is_live, readable_fields};
use super::report::Finding;
use super::resolve::{Resolution, Resolver, declared_references, reference_line, written};
use super::text::{FileText, front_matter_failed};

/// The link type whose items are exempt from `ref-superseded`, as is the
/// `supersedes:` key.
const SUPERSEDES: &str = "supersedes";
/// The link type of the cycle rule.
const DEPENDS_ON: &str = "depends_on";

/// The three warnings.
pub(crate) fn run(corpus: &Corpus<'_>, scheme: &IdScheme, findings: &mut Vec<Finding>) {
    mention_dangling(corpus, findings);
    ref_superseded(corpus, scheme, findings);
    depends_cycle(corpus, findings);
}

/// The inline mentions of a parse, in text order.
fn inline_mentions(parsed: &ParsedFile) -> impl Iterator<Item = &Reference> {
    parsed.links.iter().filter_map(|link| match &link.dst {
        LinkTarget::Reference(reference)
            if link.origin == LinkOrigin::Inline && link.link_type == MENTIONS =>
        {
            Some(reference)
        }
        _ => None,
    })
}

/// The line of an inline mention: its span's, 1 without bytes.
fn mention_line(text: &FileText<'_>, reference: &Reference) -> usize {
    reference
        .span
        .filter(|_| !text.is_empty())
        .map_or(1, |span| text.line(span.start))
}

/// Live sources in path order: (file index, parse).
fn live_sources<'c, 'a>(
    corpus: &'c Corpus<'a>,
) -> impl Iterator<Item = (usize, &'a ParsedFile)> + 'c {
    corpus
        .parses
        .iter()
        .enumerate()
        .filter_map(|(index, parsed)| parsed.map(|parsed| (index, parsed)))
        .filter(|(_, parsed)| is_live(parsed))
}

fn mention_dangling(corpus: &Corpus<'_>, findings: &mut Vec<Finding>) {
    for (index, parsed) in live_sources(corpus) {
        let text = &corpus.texts[index];
        for reference in inline_mentions(parsed) {
            let written = written(text, reference);
            if let Resolution::Dangling(reason) =
                corpus
                    .resolver
                    .resolve_mention(corpus.paths[index], reference, &written)
            {
                findings.push(warning(
                    "mention-dangling",
                    corpus.paths[index],
                    mention_line(text, reference),
                    &written,
                    format!("`{MENTIONS}`: `{written}` {reason}"),
                ));
            }
        }
    }
}

/// Per file: `X` as written in its `status: superseded-by X`, and the files
/// holding `X` as cited from that file.
fn supersessions(corpus: &Corpus<'_>, scheme: &IdScheme) -> Vec<Option<(String, Vec<usize>)>> {
    corpus
        .parses
        .iter()
        .zip(&corpus.paths)
        .map(|(parsed, &path)| {
            let status = parsed.and_then(readable_fields)?.status.as_deref()?;
            let (_, target) = grammar::split_superseded_by(status)?;
            let holders = grammar::parse_reference(target, 0, scheme)
                .and_then(|found| corpus.resolver.holders_of(path, &found.reference, target))
                .unwrap_or_default();
            Some((target.to_owned(), holders))
        })
        .collect()
}

fn ref_superseded(corpus: &Corpus<'_>, scheme: &IdScheme, findings: &mut Vec<Finding>) {
    let superseded = supersessions(corpus, scheme);
    if superseded.iter().all(Option::is_none) {
        return;
    }
    for (index, parsed) in live_sources(corpus) {
        let text = &corpus.texts[index];
        let from = corpus.paths[index];
        // (line, as written, resolution) per reference.
        let mut cited: Vec<(usize, String, Resolution)> = Vec::new();
        let declared = match parsed.document() {
            Some(document) if !front_matter_failed(parsed) => {
                declared_references(document, scheme, text)
            }
            _ => Vec::new(),
        };
        for item in &declared {
            let exempt = matches!(item.key, "supersedes" | "status")
                || item.link_type.as_deref() == Some(SUPERSEDES);
            if exempt {
                continue;
            }
            let written = written(text, &item.reference);
            let resolution = corpus.resolver.resolve(from, &item.reference, &written);
            let line = reference_line(text, &item.reference, item.key);
            cited.push((line, written, resolution));
        }
        for reference in inline_mentions(parsed) {
            let written = written(text, reference);
            let resolution = corpus.resolver.resolve_mention(from, reference, &written);
            cited.push((mention_line(text, reference), written, resolution));
        }
        for (line, written, resolution) in cited {
            let Resolution::Resolved(holders) = resolution else {
                continue;
            };
            let Some((by, by_files)) = holders
                .iter()
                .find_map(|&holder| superseded[holder].as_ref())
            else {
                continue;
            };
            if by_files.contains(&index) {
                continue;
            }
            findings.push(warning(
                "ref-superseded",
                corpus.paths[index],
                line,
                &written,
                format!("`{written}` is superseded by {by}"),
            ));
        }
    }
}

fn depends_cycle(corpus: &Corpus<'_>, findings: &mut Vec<Finding>) {
    let resolver: &Resolver<'_> = &corpus.resolver;
    let count = corpus.paths.len();
    let mut graph: DiGraph<usize, ()> = DiGraph::with_capacity(count, 0);
    let nodes: Vec<NodeIndex> = (0..count).map(|index| graph.add_node(index)).collect();
    // Per file: its `depends_on` items that resolve, in source order, with
    // their lines and the files they reach.
    let mut items: Vec<Vec<(usize, Vec<usize>)>> = vec![Vec::new(); count];
    for (index, parsed) in live_sources(corpus) {
        let Some(list) = readable_fields(parsed)
            .and_then(|fields| fields.links.as_ref())
            .and_then(|links| links.get(DEPENDS_ON))
        else {
            continue;
        };
        let text = &corpus.texts[index];
        let from = corpus.paths[index];
        for reference in list {
            let written = written(text, reference);
            let Some(holders) = resolver.holders_of(from, reference, &written) else {
                continue;
            };
            for &holder in &holders {
                graph.add_edge(nodes[index], nodes[holder], ());
            }
            let line = reference_line(text, reference, "links");
            items[index].push((line, holders));
        }
    }
    for component in tarjan_scc(&graph) {
        let cyclic = match component.as_slice() {
            [] => false,
            [only] => graph.contains_edge(*only, *only),
            _ => true,
        };
        if !cyclic {
            continue;
        }
        let members: Vec<usize> = component.iter().map(|&node| graph[node]).collect();
        let mut named: Vec<(String, usize)> = members
            .iter()
            .map(|&member| (name(corpus, member), member))
            .collect();
        named.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| corpus.paths[a.1].cmp(corpus.paths[b.1]))
        });
        let subject = named
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let first = named[0].1;
        let line = items[first]
            .iter()
            .find(|(_, reached)| reached.iter().any(|file| members.contains(file)))
            .map_or(1, |&(line, _)| line);
        findings.push(warning(
            "depends-cycle",
            corpus.paths[first],
            line,
            &subject,
            format!("`{DEPENDS_ON}` forms a cycle through {subject}"),
        ));
    }
}

/// A document's name in a cycle: its ID, else its path.
fn name(corpus: &Corpus<'_>, index: usize) -> String {
    corpus.parses[index]
        .and_then(|parsed| parsed.document())
        .and_then(|document| document.id.clone())
        .unwrap_or_else(|| corpus.paths[index].to_owned())
}

fn warning(code: &str, path: &str, line: usize, subject: &str, message: String) -> Finding {
    Finding {
        code: code.to_owned(),
        severity: Severity::Warning,
        path: path.to_owned(),
        line,
        subject: subject.to_owned(),
        message,
        fix: None,
        debt: None,
    }
}
