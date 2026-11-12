//! The pure half of a create's similar step (task spec `intake-similar`,
//! IS1 to IS6): the five sources a create's `similar` list names, the entry
//! its answer carries, the search terms its titles and keywords give, the
//! `keywords` check and the caps. Nothing is read, written or resolved
//! here, and no project's word appears (ADR-0008): a kind is an opaque
//! string, compared as written.
//!
//! Only the exact sources gate ([`SimilarSource::gates`]): an equal title,
//! an alias equal to the title, a live create's or amendment's title. A
//! search rank or shared link targets advise; a gate hung on bm25 would
//! differ between a dry run, two worktrees, a hub and a terminal.

use serde::Serialize;

use crate::intake::{DISTINCT_MAX, FieldProblem, distinct_problem};

/// The search entries a list holds at most.
pub const SIMILAR_SEARCH_MAX: usize = 5;
/// The neighbour entries a list holds at most.
pub const SIMILAR_NEIGHBOURS_MAX: usize = 5;
/// The search terms a probe ORs at most: the titles' first, then the
/// keywords'.
pub const SIMILAR_TERMS_MAX: usize = 32;
/// A search term matching more nodes of the corpus than this is dropped:
/// it tells nothing apart.
pub const SIMILAR_TERM_NODES_MAX: usize = 1000;
/// A link target held by more front-matter edges of the corpus than this
/// is skipped by the neighbour probe.
pub const SIMILAR_TARGET_DEGREE_MAX: usize = 200;
/// The link targets a neighbour shares with the new document at least.
pub const SIMILAR_SHARED_MIN: usize = 2;
/// The gating entries a list names at most: what `distinct_from` can name.
pub const SIMILAR_GATING_MAX: usize = DISTINCT_MAX;
/// The names a reason's list spells out before `, and <k> more`.
pub const SIMILAR_REASON_NAMES_MAX: usize = 8;
/// The keywords one create takes at most.
pub const KEYWORDS_MAX: usize = 16;
/// The characters a keyword holds at least: the store's shortest search
/// term (a compile-time assert there keeps them equal).
pub const KEYWORD_MIN_CHARS: usize = 3;
/// The bytes a keyword holds at most.
pub const KEYWORD_MAX_BYTES: usize = 64;

/// Where a similar entry was found, in list order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SimilarSource {
    /// A corpus node's title equals a new node's.
    Title,
    /// A corpus document's `aliases:` entry equals a new node's title.
    Alias,
    /// A live create or amendment adds a node titled as a new one.
    Queue,
    /// A search rank on the new titles' terms and the keywords.
    Search,
    /// A node of the new document's kind sharing its link targets.
    Neighbours,
}

impl SimilarSource {
    /// Every source, in list order.
    pub const ALL: [Self; 5] = [
        Self::Title,
        Self::Alias,
        Self::Queue,
        Self::Search,
        Self::Neighbours,
    ];

    /// `title`, `alias`, `queue`, `search` or `neighbours`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Alias => "alias",
            Self::Queue => "queue",
            Self::Search => "search",
            Self::Neighbours => "neighbours",
        }
    }

    /// An entry of this source keeps a create unstored until
    /// `distinct_from` names it: the exact three.
    pub const fn gates(self) -> bool {
        match self {
            Self::Title | Self::Alias | Self::Queue => true,
            Self::Search | Self::Neighbours => false,
        }
    }
}

/// One entry of a create's `similar` list (task spec `intake-similar`,
/// IS1): a corpus node, or a live proposal (`id` its `PR-` ID, `path` its
/// target path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SimilarEntry {
    /// The node's ID (`null` for an ID-less document), or the proposal's.
    pub id: Option<String>,
    /// The node's file, or the proposal's target path; root-relative.
    pub path: String,
    /// As indexed, or as parsed from the proposal's text.
    pub title: Option<String>,
    /// As indexed, or as parsed from the proposal's text.
    pub kind: Option<String>,
    pub source: SimilarSource,
    /// Why it is listed, one line.
    pub reason: String,
    /// [`SimilarSource::gates`] of `source`.
    pub gates: bool,
}

impl SimilarEntry {
    /// What `distinct_from` names it by, byte for byte: its `id`, else its
    /// `path`.
    pub fn name(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.path)
    }
}

/// The search terms of a create: each normalized title's terms of at least
/// [`KEYWORD_MIN_CHARS`] characters, in text order, then the keywords,
/// lower-cased, each once, the first [`SIMILAR_TERMS_MAX`].
pub fn similar_terms(titles: &[&str], keywords: &[String]) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let words = titles
        .iter()
        .flat_map(|title| term_words(title))
        .chain(keywords.iter().flat_map(|keyword| term_words(keyword)));
    for word in words {
        if terms.len() == SIMILAR_TERMS_MAX {
            break;
        }
        let term = word.to_lowercase();
        if term.chars().count() >= KEYWORD_MIN_CHARS && !terms.contains(&term) {
            terms.push(term);
        }
    }
    terms
}

/// The words of `text` a search may quote: split at Unicode whitespace and
/// at U+0000, which would end an FTS5 string early (task spec
/// `intake-similar`, IS1), the empty ones dropped.
pub fn term_words(text: &str) -> impl Iterator<Item = &str> {
    text.split_whitespace()
        .flat_map(|word| word.split('\0'))
        .filter(|word| !word.is_empty())
}

/// `` `a`, `b` `` for `names`, at most [`SIMILAR_REASON_NAMES_MAX`], then
/// `, and <k> more`.
pub fn named_list<S: AsRef<str>>(names: &[S]) -> String {
    let shown: Vec<String> = names
        .iter()
        .take(SIMILAR_REASON_NAMES_MAX)
        .map(|name| format!("`{}`", name.as_ref()))
        .collect();
    let mut list = shown.join(", ");
    if names.len() > SIMILAR_REASON_NAMES_MAX {
        list.push_str(&format!(
            ", and {} more",
            names.len() - SIMILAR_REASON_NAMES_MAX
        ));
    }
    list
}

/// The first problem of a create's `distinct_from`: past [`DISTINCT_MAX`]
/// entries the first one past it named (`distinct_from[64]`), else the
/// intake's check of each entry ([`distinct_problem`]).
pub fn distinct_from_problem(distinct_from: &[String]) -> Option<FieldProblem> {
    past_cap("distinct_from", distinct_from.len(), DISTINCT_MAX)
        .or_else(|| distinct_problem(distinct_from))
}

/// The first problem of a create's `keywords`, in input order: at most
/// [`KEYWORDS_MAX`], the first one past it named (`keywords[16]`); each
/// without a control character or whitespace (one word each),
/// [`KEYWORD_MIN_CHARS`] characters to [`KEYWORD_MAX_BYTES`] bytes, given
/// once.
pub fn keyword_problem(keywords: &[String]) -> Option<FieldProblem> {
    if let Some(problem) = past_cap("keywords", keywords.len(), KEYWORDS_MAX) {
        return Some(problem);
    }
    keywords.iter().enumerate().find_map(|(index, keyword)| {
        let problem = if keyword.chars().any(char::is_control) {
            "holds a control character".to_owned()
        } else if keyword.chars().any(char::is_whitespace) {
            "holds whitespace: one word each".to_owned()
        } else if keyword.chars().count() < KEYWORD_MIN_CHARS {
            format!(
                "{} character(s); at least {KEYWORD_MIN_CHARS}",
                keyword.chars().count()
            )
        } else if keyword.len() > KEYWORD_MAX_BYTES {
            format!("{} bytes; at most {KEYWORD_MAX_BYTES}", keyword.len())
        } else {
            let first = keywords[..index].iter().position(|seen| seen == keyword)?;
            format!("repeats keywords[{first}]: give each once")
        };
        Some(FieldProblem {
            field: format!("keywords[{index}]"),
            problem,
        })
    })
}

/// `<field>[<max>]: <count> entries; at most <max>` when `count` is past
/// `max`.
fn past_cap(field: &str, count: usize, max: usize) -> Option<FieldProblem> {
    (count > max).then(|| FieldProblem {
        field: format!("{field}[{max}]"),
        problem: format!("{count} entries; at most {max}"),
    })
}
