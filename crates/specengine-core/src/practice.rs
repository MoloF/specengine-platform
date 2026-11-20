//! Practices, the pure half (task spec `department-practices`; ADR-0084): a
//! live, non-generated document whose typed `disciplines:` lists card
//! disciplines is a practice, and its direct child `{#ID}` sections are its
//! rules (none: the document itself is its one rule). Nothing here gates,
//! refuses or judges a card, a task or an artefact (ADR-0012): the only
//! finding is the check's `practice-discipline` warning.
//!
//! - **Entries** ([`judge_disciplines`]): judged in written order, the first
//!   rule an entry breaks naming it: it is no discipline name (the flows'
//!   grammar, [`card_toml::name_problem`]); it repeats an earlier entry; it
//!   is past the [`DISCIPLINES_MAX`]th kept one. A kept entry that no
//!   declared `[[card.flow]]` lists is still kept, and warned of only when
//!   `[card]` reads and declares a flow ([`declared_disciplines`]).
//! - **Rules** ([`practice_rules`]): the direct child ID sections by
//!   position; a practice with none gives its document.
//! - **Citations** ([`cited_in`]): core's body-reference scan over a text
//!   (text and inline code, never code blocks, HTML or link destinations),
//!   each distinct written reference once, in first-seen order; the caller
//!   resolves them and keeps at most [`CITED_MAX`].
//!
//! The core names no discipline, kind or trade word (ADR-0008): a discipline
//! is data, compared byte for byte.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::{IdScheme, IdScript, Node, Reference, Span, grammar};

use crate::card_toml::{self, declared_disciplines};
use crate::markdown;

/// Kept `disciplines:` entries per document, and disciplines per bundle
/// request.
pub const DISCIPLINES_MAX: usize = 16;

/// Practice documents listed per card or task, by path; past them, one
/// note.
pub const PRACTICES_MAX: usize = 32;

/// Rule candidates per practice in a bundle's practices layer; the rest
/// count in the not-included `more`.
pub const PRACTICE_RULES_MAX: usize = 128;

/// Resolved citations per card or proposal, in first-seen order; also the
/// most references a read looks up.
pub const CITED_MAX: usize = 64;

/// The characters a read's practices may take as JSON (each entry's
/// compact JSON and a comma): past them the rest are left out with the
/// note past [`PRACTICES_MAX`], so a card's whole answer stays within an
/// MCP result's cap however long its practices' names, titles, paths and
/// disciplines are.
pub const PRACTICES_CHARS_MAX: usize = 16_384;

/// The characters a read's citations may take as JSON, as
/// [`PRACTICES_CHARS_MAX`]; past them the rest are left out with a note.
pub const CITED_CHARS_MAX: usize = 16_384;

/// The front-matter key, the check's subject.
pub const DISCIPLINES_KEY: &str = "disciplines";

/// The check code of a `disciplines:` entry's warning.
pub const PRACTICE_DISCIPLINE: &str = "practice-discipline";

/// One `disciplines:` warning: the 1-based entry it is about (0: none in
/// particular) and its words, as `spec check` gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisciplineProblem {
    pub entry: usize,
    pub message: String,
}

/// `disciplines:` judged: the entries kept, in written order, and each
/// warning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Disciplines {
    pub kept: Vec<String>,
    pub problems: Vec<DisciplineProblem>,
}

/// Judges `written`, the string entries of `disciplines:` in written order
/// (see the module documentation). `flows`: the disciplines the declared
/// flows list, [`declared_disciplines`]; `None` judges no entry against
/// them.
pub fn judge_disciplines(written: &[String], flows: Option<&BTreeSet<String>>) -> Disciplines {
    let mut judged = Disciplines::default();
    judge_each(written, |entry, discipline, verdict| match verdict {
        Verdict::Kept => {
            if let Some(flows) = flows
                && !flows.contains(discipline)
            {
                judged.problems.push(DisciplineProblem {
                    entry,
                    message: format!(
                        "disciplines: `{discipline}` is in no `[[card.flow]] disciplines`; no \
                         card receives this practice"
                    ),
                });
            }
            judged.kept.push(discipline.to_owned());
        }
        failed => judged.problems.push(DisciplineProblem {
            entry,
            message: failed.message(entry),
        }),
    });
    judged
}

/// [`judge_disciplines`]' kept entries alone, no problem worded: what the
/// index stores and a read serves.
pub fn kept_disciplines(written: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    judge_each(written, |_, discipline, verdict| {
        if verdict == Verdict::Kept {
            kept.push(discipline.to_owned());
        }
    });
    kept
}

/// A document's kept disciplines: none when it has no `disciplines:` or a
/// mistyped one.
pub fn document_disciplines(document: &Node) -> Vec<String> {
    document
        .fields
        .as_ref()
        .and_then(|fields| fields.disciplines.as_deref())
        .map(kept_disciplines)
        .unwrap_or_default()
}

/// The `practice-discipline` warnings of `written` under the config text
/// `config` ([`declared_disciplines`] read from it).
pub fn discipline_problems(written: &[String], config: &str) -> Vec<DisciplineProblem> {
    let flows = declared_disciplines(config);
    judge_disciplines(written, flows.as_ref()).problems
}

/// Why `name` is no discipline name, for `--discipline`: `None` when it is
/// one (the flows' grammar, `[a-z0-9_-]`, 1 to 64 bytes).
pub fn discipline_name_problem(name: &str) -> Option<String> {
    card_toml::name_problem(name).map(|_| format!("`--discipline`: `{name}` is no discipline name"))
}

/// One entry's verdict, by the first rule it breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Kept,
    /// No discipline name.
    Name,
    /// The 1-based entry it repeats, the first byte-equal one.
    Repeats(usize),
    /// Past [`DISCIPLINES_MAX`] kept entries.
    Past,
}

impl Verdict {
    /// `spec check`'s words for a failed entry.
    fn message(self, entry: usize) -> String {
        match self {
            Self::Kept => String::new(),
            Self::Name => format!(
                "disciplines: entry {entry} is no discipline name ([a-z0-9_-], 1 to 64 bytes)"
            ),
            Self::Repeats(earlier) => {
                format!("disciplines: entry {entry} repeats entry {earlier}")
            }
            Self::Past => format!(
                "disciplines: entry {entry} is past the {DISCIPLINES_MAX}th kept; a practice \
                 keeps {DISCIPLINES_MAX}"
            ),
        }
    }
}

/// Calls `each` with every entry's 1-based position, text and verdict, in
/// written order.
fn judge_each<'w>(written: &'w [String], mut each: impl FnMut(usize, &'w str, Verdict)) {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut kept = 0;
    for (index, discipline) in written.iter().enumerate() {
        let entry = index + 1;
        let first = *seen.entry(discipline.as_str()).or_insert(entry);
        let verdict = if card_toml::name_problem(discipline).is_some() {
            Verdict::Name
        } else if first != entry {
            Verdict::Repeats(first)
        } else if kept >= DISCIPLINES_MAX {
            Verdict::Past
        } else {
            kept += 1;
            Verdict::Kept
        };
        each(entry, discipline, verdict);
    }
}

/// A practice's rules, positions in `nodes` (one parse): its direct child
/// ID sections by position, none nested in another section; `[0]`, the
/// document, when it has none. Empty without a document.
pub fn practice_rules(nodes: &[Node]) -> Vec<usize> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut rules = Vec::new();
    // The end of the last direct child: a section starting before it is
    // nested in it (sections come in source order).
    let mut open_until = 0;
    for (ord, node) in nodes.iter().enumerate().skip(1) {
        if node.span.start < open_until || node.id.is_none() {
            continue;
        }
        rules.push(ord);
        open_until = node.span.end;
    }
    if rules.is_empty() {
        rules.push(0);
    }
    rules
}

/// A practice's rule count as served: its direct child ID sections, or 1
/// (the document) when it has none.
pub fn rule_count(nodes: &[Node]) -> usize {
    practice_rules(nodes).len()
}

/// One reference a text cites, before it is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// Its occurrence's bytes, verbatim: what a reader links.
    pub written: String,
    pub reference: Reference,
}

/// Every distinct reference `texts` cite, in first-seen order, text by
/// text (see the module documentation); a reference with a look-alike, of
/// mixed script or naming another project is left out, as a REF would be
/// refused. At most `limit`.
pub fn cited_in<'t>(
    texts: impl IntoIterator<Item = &'t str>,
    scheme: &IdScheme,
    limit: usize,
) -> Vec<Citation> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cited = Vec::new();
    if limit == 0 {
        return cited;
    }
    for text in texts {
        if text.is_empty() {
            continue;
        }
        let body = markdown::scan(text, Span::new(0, text.len()));
        for region in &body.regions {
            let Some(piece) = text.get(region.clone()) else {
                continue;
            };
            for found in grammar::scan(piece, region.start, scheme) {
                let reference = found.reference;
                if !found.homoglyphs.is_empty()
                    || reference.script == IdScript::Mixed
                    || reference.project.is_some()
                {
                    continue;
                }
                let Some(written) = reference
                    .span
                    .and_then(|span| text.get(span.range()))
                    .map(str::to_owned)
                else {
                    continue;
                };
                if seen.insert(written.clone()) {
                    cited.push(Citation { written, reference });
                    if cited.len() >= limit {
                        return cited;
                    }
                }
            }
        }
    }
    cited
}
