//! IDs at import (`docs/features/import-records.md` AC-05, AC-06): a legacy
//! prefix as written is substituted by its Latin prefix before `ids.regex`;
//! otherwise look-alikes are normalised only where the prefix keeps an ASCII
//! letter (a typo, ADR-0009); a prefix with no ASCII letter that is in no
//! legacy map is never guessed. Such a prefix is found by its shape (letters
//! of any script, `-`, digits), whatever `ids.like`.

use std::collections::BTreeMap;
use std::ops::Range;

use regex::Regex;

use crate::config::{IdPattern, Pattern};
use crate::markdown::emphasis_blanks;
use crate::script::IdScript;

/// What a candidate text holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resolution {
    Id(Resolved),
    /// An ID-shaped token whose prefix has no ASCII letter and is not in
    /// `[ids.legacy]`: counted, never turned into an ID.
    Unmapped(String),
    None,
}

/// An ID found in a candidate text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
    /// Latin: legacy prefix substituted, look-alikes normalised.
    pub id: String,
    pub prefix: String,
    /// The ID as written in the corpus.
    pub written: String,
    pub script: IdScript,
    /// The prefix was substituted through `[ids.legacy]`.
    pub legacy: bool,
    /// Look-alikes were normalised (mixed script).
    pub homoglyph: bool,
}

/// The ID rules of one config.
pub(crate) struct Resolver<'c> {
    ids: &'c IdPattern,
    legacy: &'c BTreeMap<String, String>,
    like: &'c Regex,
    hyphenless: &'c [Pattern],
}

impl<'c> Resolver<'c> {
    pub fn new(
        ids: &'c IdPattern,
        legacy: &'c BTreeMap<String, String>,
        like: &'c Regex,
        hyphenless: &'c [Pattern],
    ) -> Self {
        Self {
            ids,
            legacy,
            like,
            hyphenless,
        }
    }

    /// The first ID of `text` (a cell, an anchor, a lead-in), legacy map
    /// first, then `ids.regex`; a token of any-script letters, `-` and
    /// digits whose prefix holds no ASCII letter and neither maps is
    /// [`Resolution::Unmapped`].
    pub fn resolve(&self, text: &str) -> Resolution {
        if !self.legacy.is_empty() {
            for run in candidate_runs(text) {
                let Some(latin) = self.legacy.get(&text[run.clone()]) else {
                    continue;
                };
                let substituted = format!("{}{latin}{}", &text[..run.start], &text[run.end..]);
                let Some((found, range)) = self.ids.find_range(&substituted) else {
                    continue;
                };
                let prefix_end = run.start + latin.len();
                if range.start != run.start || range.end < prefix_end {
                    continue;
                }
                let end = range.end - prefix_end + run.end;
                let Some(written) = text.get(run.start..end).map(str::to_owned) else {
                    continue;
                };
                return Resolution::Id(Resolved {
                    id: found.id,
                    prefix: found.prefix,
                    script: IdScript::of(&written),
                    written,
                    legacy: true,
                    homoglyph: false,
                });
            }
        }
        if let Some((found, _)) = self.ids.find_range(text) {
            return match found.script {
                IdScript::Latin => Resolution::Id(Resolved {
                    id: found.id,
                    prefix: found.prefix,
                    written: found.verbatim,
                    script: found.script,
                    legacy: false,
                    homoglyph: false,
                }),
                IdScript::MixedScript if !non_latin_prefix(&found.verbatim) => {
                    Resolution::Id(Resolved {
                        id: found.id,
                        prefix: found.prefix,
                        written: found.verbatim,
                        script: found.script,
                        legacy: false,
                        homoglyph: true,
                    })
                }
                _ => Resolution::Unmapped(found.verbatim),
            };
        }
        match script_tokens(text).find(|range| non_latin_prefix(&text[range.clone()])) {
            Some(range) => Resolution::Unmapped(text[range].to_owned()),
            None => Resolution::None,
        }
    }

    /// [`Resolver::resolve`] when the ID is the whole of `text`.
    pub fn resolve_whole(&self, text: &str) -> Resolution {
        match self.resolve(text) {
            Resolution::Id(resolved) if resolved.written == text => Resolution::Id(resolved),
            Resolution::Unmapped(written) if written == text => Resolution::Unmapped(written),
            _ => Resolution::None,
        }
    }

    /// `ids.like` matches bounded as the reference grammar's recognition
    /// steps 1 and 4 (model README "Reference grammar"): no letter, digit,
    /// `_` or `-` before; after, the end or a char that is no letter, digit
    /// or `_`, nor a `-` before a letter or digit. Emphasis delimiter runs
    /// read as blanks, so `__ID__` bounds its ID.
    pub fn like_tokens(&self, text: &str) -> Vec<Range<usize>> {
        let text = emphasis_blanks(text);
        self.like
            .find_iter(&text)
            .map(|found| found.range())
            .filter(|range| !range.is_empty() && bounded(&text, range))
            .collect()
    }

    /// Hyphenless matches in `text`, emphasis delimiter runs read as
    /// blanks: (pattern index, range), pattern by pattern in config order; a
    /// range overlapping one an earlier pattern matched is that token again
    /// and is dropped.
    pub fn hyphenless(&self, text: &str) -> Vec<(usize, Range<usize>)> {
        let text = emphasis_blanks(text);
        let mut found: Vec<(usize, Range<usize>)> = Vec::new();
        for (index, pattern) in self.hyphenless.iter().enumerate() {
            let earlier = found.len();
            for range in pattern
                .regex
                .find_iter(&text)
                .map(|matched| matched.range())
            {
                let seen = found[..earlier]
                    .iter()
                    .any(|(_, kept)| kept.start < range.end && range.start < kept.end);
                if !range.is_empty() && !seen {
                    found.push((index, range));
                }
            }
        }
        found
    }

    /// Whether a hyphenless pattern matches the whole of `token`: such a
    /// token is never a record.
    pub fn is_hyphenless(&self, token: &str) -> bool {
        self.hyphenless.iter().any(|pattern| {
            pattern
                .regex
                .find(token)
                .is_some_and(|found| found.start() == 0 && found.end() == token.len())
        })
    }
}

/// Maximal letter-digit runs followed by `-`, not preceded by `_` or `-`
/// (recognition step 1): where a prefix as written can stand.
fn candidate_runs(text: &str) -> Vec<Range<usize>> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    let mut before: Option<char> = None;
    let mut run_before: Option<char> = None;
    for (offset, c) in text.char_indices() {
        if c.is_alphanumeric() {
            if start.is_none() {
                start = Some(offset);
                run_before = before;
            }
        } else if let Some(begin) = start.take()
            && c == '-'
            && !matches!(run_before, Some('_' | '-'))
        {
            runs.push(begin..offset);
        }
        before = Some(c);
    }
    runs
}

/// Tokens of any-script letters and digits (a letter first), `-`, digits,
/// bounded as the reference grammar's recognition steps 1 and 4.
fn script_tokens(text: &str) -> impl Iterator<Item = Range<usize>> + '_ {
    candidate_runs(text).into_iter().filter_map(move |run| {
        if !text[run.clone()].starts_with(char::is_alphabetic) {
            return None;
        }
        let digits_from = run.end + 1;
        let digits = text[digits_from..]
            .char_indices()
            .find(|&(_, c)| !c.is_numeric())
            .map_or(text.len() - digits_from, |(offset, _)| offset);
        let token = run.start..digits_from + digits;
        (digits > 0 && bounded(text, &token)).then_some(token)
    })
}

/// The leading letters of a written ID hold a foreign letter and no ASCII one.
pub(crate) fn non_latin_prefix(written: &str) -> bool {
    let mut ascii = false;
    let mut foreign = false;
    for c in written.chars().take_while(|c| c.is_alphabetic()) {
        if c.is_ascii_alphabetic() {
            ascii = true;
        } else {
            foreign = true;
        }
    }
    foreign && !ascii
}

fn bounded(text: &str, range: &Range<usize>) -> bool {
    let before = text[..range.start].chars().next_back();
    if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return false;
    }
    let mut after = text[range.end..].chars();
    match after.next() {
        None => true,
        Some(c) if c.is_alphanumeric() || c == '_' => false,
        Some('-') => !after.next().is_some_and(char::is_alphanumeric),
        Some(_) => true,
    }
}
