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
//!   `superseded-by X`; `supersedes` items, the `status:` value,
//!   references from X's own files and from an accepted decision dated
//!   before X (never edited: it cited Y while Y was live) are exempt. Both
//!   dates are a readable `date:` of `YYYY-MM-DD` shape, X's from its first
//!   holder in path order, compared as text; one missing, the reference
//!   warns.
//! - `parent-cycle`: one finding per `parent:` cycle `spec tree` breaks
//!   ([`SpecGraph::cycles`]) with a live member.

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use specengine_model::grammar;
use specengine_model::{
    IdScheme, LinkOrigin, LinkTarget, MENTIONS, ParsedFile, Reference, Severity,
};

use super::config::DocClass;
use super::engine::Corpus;
use super::input::CheckInput;
use super::render::{is_live, readable_fields};
use super::report::Finding;
use super::resolve::{Resolution, Resolver, declared_references, reference_line, written};
use super::spec_graph::{SpecGraph, Standing};
use super::text::{FileText, front_matter_failed, is_date_shaped};
use crate::Paths;

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

/// Live sources in path order: (file index, parse). Also the link rules'.
pub(super) fn live_sources<'c, 'a>(
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

/// The successor X of a superseded file.
struct Successor<'a> {
    /// `X` as written in its `status: superseded-by X`.
    written: String,
    /// The files holding `X` as cited from the superseded file.
    holders: Vec<usize>,
    /// X's date: the [`shaped_date`] of its first holder (path order).
    date: Option<&'a str>,
}

/// Per file: its successor when its `status:` is `superseded-by X`.
fn supersessions<'a>(corpus: &Corpus<'a>, scheme: &IdScheme) -> Vec<Option<Successor<'a>>> {
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
            // Files are in path order: the least index is the first holder.
            let date = holders
                .iter()
                .min()
                .and_then(|&holder| corpus.parses[holder])
                .and_then(shaped_date);
            Some(Successor {
                written: target.to_owned(),
                holders,
                date,
            })
        })
        .collect()
}

/// The readable `date:` when it is `YYYY-MM-DD` by shape, else none.
fn shaped_date(parsed: &ParsedFile) -> Option<&str> {
    readable_fields(parsed)?
        .date
        .as_deref()
        .filter(|date| is_date_shaped(date))
}

/// D of an accepted decision: the [`shaped_date`] of a readable
/// front-matter with `class: decision` and `status: accepted`, else none.
fn accepted_decision_date(parsed: &ParsedFile) -> Option<&str> {
    let fields = readable_fields(parsed)?;
    let decision = fields.class.as_deref().and_then(DocClass::parse) == Some(DocClass::Decision);
    let accepted = fields.status.as_deref() == Some("accepted");
    if decision && accepted {
        shaped_date(parsed)
    } else {
        None
    }
}

fn ref_superseded(corpus: &Corpus<'_>, scheme: &IdScheme, findings: &mut Vec<Finding>) {
    let superseded = supersessions(corpus, scheme);
    if superseded.iter().all(Option::is_none) {
        return;
    }
    for (index, parsed) in live_sources(corpus) {
        let text = &corpus.texts[index];
        let from = corpus.paths[index];
        // An accepted decision is never edited: dated before X, it cited
        // Y while Y was live.
        let cited_on = accepted_decision_date(parsed);
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
            let Some(successor) = holders
                .iter()
                .find_map(|&holder| superseded[holder].as_ref())
            else {
                continue;
            };
            if successor.holders.contains(&index) {
                continue;
            }
            // Text order is the calendar's for `YYYY-MM-DD`; strictly later.
            if let (Some(cited_on), Some(succeeded_on)) = (cited_on, successor.date)
                && succeeded_on > cited_on
            {
                continue;
            }
            let by = &successor.written;
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

/// `parent-cycle`: one warning per `parent:` cycle of the read commands'
/// tree ([`SpecGraph::cycles`], the cycles `spec tree` breaks and warns
/// about) holding a live member ([`Standing::Live`]), rules or none. At the
/// `parent:` line of its first member in (path, position) order, the root
/// the tree lists; subject the members' names (ID, else path) sorted by
/// name, then path, as `depends-cycle`'s. The tree alone is built
/// ([`SpecGraph::tree_only`], no links); without a document whose
/// front-matter gives a `parent:`, none.
pub(super) fn parent_cycles(
    input: &CheckInput,
    scheme: &IdScheme,
    paths: &Paths,
    findings: &mut Vec<Finding>,
) {
    let any_parent = input.files.iter().any(|file| {
        file.parsed.as_ref().is_some_and(|parsed| {
            !front_matter_failed(parsed)
                && parsed
                    .document()
                    .is_some_and(|document| document.parent.is_some())
        })
    });
    if !any_parent {
        return;
    }
    let graph = SpecGraph::tree_only(input, scheme, paths);
    for members in graph.cycles() {
        let Some(&first) = members.first() else {
            continue;
        };
        if !members
            .iter()
            .any(|member| graph.standing(member.file) == Standing::Live)
        {
            continue;
        }
        let mut named: Vec<(String, &str)> = members
            .iter()
            .map(|&member| (graph.name(member), graph.paths()[member.file]))
            .collect();
        named.sort();
        let subject = named
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let line = graph.file(first.file).map_or(1, |file| {
            FileText::new(&file.bytes).key_line("parent").unwrap_or(1)
        });
        findings.push(warning(
            "parent-cycle",
            graph.paths()[first.file],
            line,
            &subject,
            format!("`parent:` forms a cycle through {subject}"),
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

/// A warning finding; also the link rules'.
pub(super) fn warning(
    code: &str,
    path: &str,
    line: usize,
    subject: &str,
    message: String,
) -> Finding {
    Finding {
        code: code.to_owned(),
        severity: Severity::Warning,
        path: path.to_owned(),
        line,
        subject: subject.to_owned(),
        message,
        fix: None,
        debt: None,
        introduced: None,
    }
}
