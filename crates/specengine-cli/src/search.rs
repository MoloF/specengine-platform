//! `spec search QUERY… [--kind K]… [--limit N] [--archive]`: the store's
//! full-text search over a fresh index, in the store's order (bm25, path,
//! position). Terms shorter than three characters are dropped with a note;
//! none left → exit 2 suggesting `spec show`. `--kind` is a free string,
//! repeatable; `--limit` 1..=200, default 20. Tier 3 files are left out in
//! the query itself, before the limit, unless `--archive`.
//!
//! The answer is bounded like `show`'s: the hits whose text lines, with the
//! summary line, fit in [`OUTPUT_CAP_CHARS`] are printed, cut at a hit
//! boundary, in order; the rest are counted in a note (JSON `notes`,
//! `truncated: true`) and, in text, a tail line `[truncated: <k> of <n>
//! hits not shown; …]`. The first hit is always printed: when its lines
//! alone pass the cap, its path, line and separators stay, its snippet
//! keeps what fits first, and its name, kind and title share the rest,
//! shortest first, so only the longest is cut, at a character boundary. A
//! field cut to nothing prints as absent (`-`, JSON `null`). The same hits,
//! cut alike, are printed with and without `--json`.

use serde::Serialize;
use specengine_store::{
    MIN_TERM_CHARS, SEARCH_LIMIT_DEFAULT, SEARCH_LIMIT_MAX, SEARCH_LIMIT_MIN, SearchHit,
    SearchQuery, SpecIndex,
};

use crate::cap::OUTPUT_CAP_CHARS;
use crate::location::open_index;
use crate::project::discover;
use crate::refresh::refresh;
use crate::{CliError, Env, Globals, Message, one_line, store_error};

/// `spec search` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchRequest {
    /// The query words as given; whitespace splits them further.
    pub terms: Vec<String>,
    /// `--kind`: node kinds to keep; empty: any.
    pub kinds: Vec<String>,
    /// `--limit`; `None`: the default, 20.
    pub limit: Option<i64>,
    /// `--archive`: keep Tier 3 files.
    pub archive: bool,
}

/// What `spec search` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutcome {
    /// The query as given, words joined by a space.
    pub query: String,
    pub kinds: Vec<String>,
    pub limit: usize,
    pub archive: bool,
    /// Best first: every hit the store gave.
    pub hits: Vec<SearchHit>,
    /// How many of `hits` are printed: those within [`OUTPUT_CAP_CHARS`],
    /// never fewer than one when there is a hit.
    pub shown: usize,
    /// The first hit's title and snippet as printed, when its lines alone
    /// pass the cap.
    pub cut: Option<HitCut>,
    /// Tier 3 matches left out (0 with `archive`).
    pub tier3_left_out: u32,
    pub messages: Vec<Message>,
}

/// How much of the first hit is printed when its lines alone pass the cap:
/// the byte lengths kept (character boundaries) of its name (the ID, else
/// the path), kind, title and snippet. A kind or title cut to nothing is
/// printed as absent (`-`, JSON `null`), an ID cut to nothing as `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitCut {
    pub name: usize,
    pub kind: usize,
    pub title: usize,
    pub snippet: usize,
}

impl SearchOutcome {
    /// The cap cut the answer: hits left out, or the first hit cut.
    pub fn truncated(&self) -> bool {
        self.shown < self.hits.len() || self.cut.is_some()
    }
}

/// `spec search`: updates the index, then searches it.
pub fn search(
    env: &Env,
    globals: &Globals,
    request: &SearchRequest,
) -> Result<SearchOutcome, CliError> {
    let limit = match request.limit {
        None => SEARCH_LIMIT_DEFAULT,
        Some(limit) => usize::try_from(limit)
            .ok()
            .filter(|limit| (SEARCH_LIMIT_MIN..=SEARCH_LIMIT_MAX).contains(limit))
            .ok_or_else(|| {
                CliError::spec(format!(
                    "--limit {limit}: the limit is {SEARCH_LIMIT_MIN} to {SEARCH_LIMIT_MAX}"
                ))
            })?,
    };
    let query = request.terms.join(" ");
    let (kept, dropped): (Vec<&str>, Vec<&str>) = query
        .split_whitespace()
        .partition(|term| term.chars().count() >= MIN_TERM_CHARS);
    let mut messages = Vec::new();
    if !dropped.is_empty() {
        let dropped: Vec<String> = dropped.iter().map(|term| format!("`{term}`")).collect();
        messages.push(Message::Note(format!(
            "search terms shorter than {MIN_TERM_CHARS} characters dropped: {}",
            dropped.join(", ")
        )));
    }
    if kept.is_empty() {
        return Err(CliError::spec(format!(
            "no search term of {MIN_TERM_CHARS} or more characters; \
             to read a node by its ID, use `spec show <ID>`"
        )));
    }

    let project = discover(env, globals)?;
    let mut open = open_index(env, &project)?;
    let (_, warnings) = refresh(&mut open.index, &project, false)?;
    messages.extend(warnings);
    let results = open
        .index
        .search(&SearchQuery {
            text: query.clone(),
            kinds: request.kinds.clone(),
            limit,
            archive: request.archive,
        })
        .map_err(store_error)?;
    let mut outcome = SearchOutcome {
        query,
        kinds: request.kinds.clone(),
        limit,
        archive: request.archive,
        shown: results.hits.len(),
        cut: None,
        hits: results.hits,
        tier3_left_out: results.tier3_left_out,
        messages,
    };
    (outcome.shown, outcome.cut) = fitting(&outcome);
    if outcome.truncated() {
        outcome.messages.push(Message::Note(format!(
            "{}: the answer is capped at {OUTPUT_CAP_CHARS} characters; \
             lower --limit or narrow the query",
            left_out(&outcome)
        )));
    }
    Ok(outcome)
}

/// How many hits, from the best, fit with the summary line in
/// [`OUTPUT_CAP_CHARS`] characters of text; at least the first, cut to fit
/// when its lines alone pass the cap.
fn fitting(outcome: &SearchOutcome) -> (usize, Option<HitCut>) {
    let summary = summary(outcome).chars().count();
    let mut used = summary;
    for (index, hit) in outcome.hits.iter().enumerate() {
        used += hit_block(hit, None).chars().count();
        if used <= OUTPUT_CAP_CHARS {
            continue;
        }
        if index > 0 {
            return (index, None);
        }
        // The first hit alone passes the cap: its fixed parts (path, line,
        // separators) stay; the snippet (the match) keeps what fits, then
        // the name, the kind and the title share the rest. Counted on the
        // raw texts: printed on one line they are never longer.
        let bare = HitCut {
            name: 0,
            kind: 0,
            title: 0,
            snippet: 0,
        };
        let fixed = summary + hit_block(hit, Some(bare)).chars().count();
        let mut room = OUTPUT_CAP_CHARS.saturating_sub(fixed);
        let snippet = take(&hit.snippet, 0, &mut room);
        // Name, kind, title: the shortest first, so only the longest (the
        // one that passes the cap) is cut; ties in that order. Cut to
        // nothing, a kind or title prints `-`, already counted.
        let mut fields = [
            (0, hit.id.as_deref().unwrap_or(&hit.path), 0),
            (1, hit.kind.as_deref().unwrap_or_default(), 1),
            (2, hit.title.as_deref().unwrap_or_default(), 1),
        ];
        fields.sort_by_key(|&(slot, text, _)| (text.chars().count(), slot));
        let mut kept = [0; 3];
        for (slot, text, placeholder) in fields {
            kept[slot] = take(text, placeholder, &mut room);
        }
        let [name, kind, title] = kept;
        return (
            1,
            Some(HitCut {
                name,
                kind,
                title,
                snippet,
            }),
        );
    }
    (outcome.hits.len(), None)
}

/// Bytes of the longest prefix of `text` that fits in `room` characters,
/// `placeholder` of them already counted for what prints when nothing of
/// it does; `room` loses what the prefix costs beyond that.
fn take(text: &str, placeholder: usize, room: &mut usize) -> usize {
    let chars = text.chars().count().min(*room + placeholder);
    *room -= chars.saturating_sub(placeholder);
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(offset, _)| offset)
}

/// What is printed of a hit: all of it, or the first hit cut to the cap.
struct HitTexts<'h> {
    id: Option<&'h str>,
    /// The ID, else the path.
    name: &'h str,
    kind: Option<&'h str>,
    title: Option<&'h str>,
    snippet: &'h str,
}

/// The texts printed for `hit`, cut when `cut` is given.
fn hit_texts(hit: &SearchHit, cut: Option<HitCut>) -> HitTexts<'_> {
    let Some(cut) = cut else {
        return HitTexts {
            id: hit.id.as_deref(),
            name: hit.id.as_deref().unwrap_or(&hit.path),
            kind: hit.kind.as_deref(),
            title: hit.title.as_deref(),
            snippet: &hit.snippet,
        };
    };
    // `bytes` always falls on a character boundary of its own text.
    fn prefix(text: &str, bytes: usize) -> &str {
        text.get(..bytes.min(text.len())).unwrap_or(text)
    }
    /// Cut to nothing is absent.
    fn kept(text: Option<&str>, bytes: usize) -> Option<&str> {
        text.map(|text| prefix(text, bytes))
            .filter(|kept| !kept.is_empty())
    }
    HitTexts {
        id: kept(hit.id.as_deref(), cut.name),
        name: prefix(hit.id.as_deref().unwrap_or(&hit.path), cut.name),
        kind: kept(hit.kind.as_deref(), cut.kind),
        title: kept(hit.title.as_deref(), cut.title),
        snippet: prefix(&hit.snippet, cut.snippet),
    }
}

/// The cut of hit `index` of `outcome`: only the first hit is ever cut.
fn cut_of(outcome: &SearchOutcome, index: usize) -> Option<HitCut> {
    if index == 0 { outcome.cut } else { None }
}

/// What the cap left out, for the note and the tail line.
fn left_out(outcome: &SearchOutcome) -> String {
    let shown = outcome.shown.min(outcome.hits.len());
    let not_shown = format!(
        "{} of {} hits not shown",
        outcome.hits.len() - shown,
        outcome.hits.len()
    );
    if outcome.cut.is_some() {
        format!("the first hit's title and snippet cut at the cap; {not_shown}")
    } else {
        not_shown
    }
}

/// A hit's two lines: `<id or path> | <kind or -> | <title or -> |
/// <path>:<line>` (+ ` | archived`), then the snippet indented four spaces.
fn hit_block(hit: &SearchHit, cut: Option<HitCut>) -> String {
    let texts = hit_texts(hit, cut);
    let kind = texts.kind.map_or_else(|| "-".to_owned(), one_line);
    let title = texts.title.map_or_else(|| "-".to_owned(), one_line);
    let archived = if hit.tier3 { " | archived" } else { "" };
    format!(
        "{} | {kind} | {title} | {}:{}{archived}\n    {}\n",
        one_line(texts.name),
        one_line(&hit.path),
        hit.line,
        one_line(texts.snippet)
    )
}

/// `hits <n> (limit <l>); …`, `n` every hit the store gave.
fn summary(outcome: &SearchOutcome) -> String {
    let hits = outcome.hits.len();
    let limit = outcome.limit;
    if outcome.archive {
        format!("hits {hits} (limit {limit}); archive included\n")
    } else {
        format!(
            "hits {hits} (limit {limit}); archived matches left out: {} (--archive)\n",
            outcome.tier3_left_out
        )
    }
}

pub(crate) fn render_text(outcome: &SearchOutcome) -> String {
    let shown = outcome.shown.min(outcome.hits.len());
    let mut out = String::new();
    for (index, hit) in outcome.hits[..shown].iter().enumerate() {
        out.push_str(&hit_block(hit, cut_of(outcome, index)));
    }
    out.push_str(&summary(outcome));
    if outcome.truncated() {
        out.push_str(&format!(
            "[truncated: {}; lower --limit or narrow the query]\n",
            left_out(outcome)
        ));
    }
    out
}

#[derive(Serialize)]
struct SearchJson<'a> {
    query: &'a str,
    kinds: &'a [String],
    limit: usize,
    archive: bool,
    hits: Vec<HitJson<'a>>,
    /// The cap cut the answer (hits left out, or the first hit cut).
    truncated: bool,
    tier3_left_out: u32,
    notes: Vec<String>,
}

#[derive(Serialize)]
struct HitJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    ord: usize,
    archived: bool,
    snippet: &'a str,
}

fn view(outcome: &SearchOutcome) -> SearchJson<'_> {
    SearchJson {
        query: &outcome.query,
        kinds: &outcome.kinds,
        limit: outcome.limit,
        archive: outcome.archive,
        hits: outcome.hits[..outcome.shown.min(outcome.hits.len())]
            .iter()
            .enumerate()
            .map(|(index, hit)| {
                let texts = hit_texts(hit, cut_of(outcome, index));
                HitJson {
                    id: texts.id,
                    kind: texts.kind,
                    title: texts.title,
                    path: &hit.path,
                    line: hit.line,
                    ord: hit.ord,
                    archived: hit.tier3,
                    snippet: texts.snippet,
                }
            })
            .collect(),
        truncated: outcome.truncated(),
        tier3_left_out: outcome.tier3_left_out,
        notes: notes(&outcome.messages),
    }
}

/// The texts of the notes, in order, each on one line as on stderr.
pub(crate) fn notes(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Note(text) => Some(one_line(text)),
            Message::Warning(_) => None,
        })
        .collect()
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for SearchOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
