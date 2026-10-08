//! The pure half of a question round (`docs/canon/round.md`, ADR-0038):
//! the sheet's bounds, the order of its items, the reading time, and the
//! grammar of the answers the owner returns. Nothing is read, written or
//! stored here; no project's word, node kind or ID prefix appears
//! (ADR-0008): the tokens are ASCII frame words, and `PR` is the queue's.
//!
//! - **Order** ([`order_key`]): severity `high`, `normal`, `low`, then
//!   none; then the waiting tasks, most first; then `created_at`, oldest
//!   first; then the ID's number.
//! - **Answers** ([`parse_answers`]): lines split at LF, a trailing CR
//!   dropped, a leading U+FEFF dropped. Blank lines and lines starting with
//!   whitespace or `>` (indented and quoted) are ignored, and so is every
//!   other line but two: a column-0 `project: <slug>` line, and a column-0
//!   answer line `PR-<digits>: <answer>`. A column-0 line that reads as an
//!   answer line only once its case or its look-alike letters are made
//!   Latin (U+0420 for `P`), or that misses the colon, is refused with its
//!   Latin form, never ignored.
//! - **Tokens** ([`Token`]): blank or `?` leave the item open; `ok` (ASCII,
//!   any case) takes the working answer; `N` or `[N]` (ASCII digits) an
//!   option; `reject: <reason>` (any case) rejects; any other text is the
//!   owner's own answer. `reject` with no reason, an answer over
//!   [`ANSWER_MAX`] bytes, a reason over [`ROUND_REASON_MAX_BYTES`], a
//!   character the queue escapes ([`crate::record::is_escaped`]) are
//!   refused, never cut; so is the word `reject` not followed directly by
//!   its colon. Two or more answers that are not blank for one ID refuse
//!   them all (a line that is no answer line counts for the ID its Latin
//!   form names); a blank slot is no answer.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use specengine_model::script::normalize_char;

use crate::intake::{ANSWER_MAX, IntakeSeverity};
use crate::proposal::{PROPOSAL_PREFIX, proposal_number};
use crate::record::{is_escaped, refused_char};

/// Items on a sheet when `--limit` is not given.
pub const ROUND_LIMIT_DEFAULT: usize = 20;

/// The most items `--limit` takes; the fewest is 1.
pub const ROUND_LIMIT_MAX: usize = 200;

/// The most bytes of a returned sheet or reply: 8 MiB, `--input`'s cap.
pub const ROUND_FILE_MAX_BYTES: usize = 8 << 20;

/// The most bytes of a `reject:` reason, a stage's `reason` cap.
pub const ROUND_REASON_MAX_BYTES: usize = 4096;

/// Characters of item text read in one minute.
pub const ROUND_CHARS_PER_MINUTE: usize = 1000;

/// The frame word of the project line.
pub const PROJECT_LINE: &str = "project:";

/// The word of a rejecting answer, before its colon.
const REJECT_WORD: &str = "reject";

/// The word that takes the working answer.
const OK_WORD: &str = "ok";

/// The answer that leaves an item open, besides a blank one.
const OPEN_WORD: &str = "?";

/// An item's place on a sheet: compare two keys, the lesser first.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderKey {
    severity: u8,
    waiting: Reverse<usize>,
    created_at: String,
    number: u64,
    id: String,
}

/// The order key of an item of `severity` (none: last), `waiting` tasks
/// (most first), made at `created_at` (oldest first), and of ID `id` (its
/// number; an ID the queue never wrote last, by its text).
pub fn order_key(
    severity: Option<IntakeSeverity>,
    waiting: usize,
    created_at: &str,
    id: &str,
) -> OrderKey {
    let severity = match severity {
        Some(IntakeSeverity::High) => 0,
        Some(IntakeSeverity::Normal) => 1,
        Some(IntakeSeverity::Low) => 2,
        None => 3,
    };
    OrderKey {
        severity,
        waiting: Reverse(waiting),
        created_at: created_at.to_owned(),
        number: proposal_number(id).unwrap_or(u64::MAX),
        id: id.to_owned(),
    }
}

/// Minutes of reading for `chars` characters of item text: rounded up,
/// 0 for none.
pub fn reading_minutes(chars: usize) -> usize {
    chars.div_ceil(ROUND_CHARS_PER_MINUTE)
}

/// The text of a returned file's `bytes`, or the 1-based line of the
/// first byte that is not UTF-8.
pub fn answers_text(bytes: &[u8]) -> Result<&str, usize> {
    std::str::from_utf8(bytes).map_err(|error| {
        let valid = &bytes[..error.valid_up_to()];
        valid.iter().filter(|&&byte| byte == b'\n').count() + 1
    })
}

/// What one answer asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// Blank or `?`: the item stays open.
    Open,
    /// `ok`: approve with the working answer.
    Ok,
    /// `N` or `[N]`: approve with option `N` (`u64::MAX` past the range of
    /// a number).
    Option(u64),
    /// `reject: <reason>`: reject, the reason trimmed.
    Reject(String),
    /// Any other text: approve with it as the owner's own answer.
    Text(String),
}

/// Why an answer line is refused before the queue is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A column-0 line that is an answer line only up to case, look-alike
    /// letters, invisible characters before it or its colon: `written`, the
    /// line's start as written; `latin`, the ID it reads as; `look_alike`,
    /// whether a visible character of `written` is not ASCII.
    NotAnAnswerLine {
        written: String,
        latin: String,
        look_alike: bool,
    },
    /// `PR-<digits>:` naming no ID the queue writes (`PR-1`).
    NoProposal,
    /// `reject` with no reason.
    RejectWithoutReason,
    /// The word `reject` (any case) not followed directly by its colon
    /// (`reject : dup`, `reject dup`): neither a reject nor the owner's
    /// own answer.
    RejectWithoutColon,
    /// The answer (`reason`: a reject's reason) over its cap.
    OverCap {
        reason: bool,
        bytes: usize,
        max: usize,
    },
    /// A character the queue escapes.
    Escaped(char),
    /// Two or more answers that are not blank for one ID: every line of
    /// them, shared by the lines it refuses (one list per ID).
    Duplicate(Arc<[usize]>),
}

/// One answer line, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerLine {
    /// 1-based.
    pub line: usize,
    /// The ID as written, or the Latin form of one that is no answer line.
    pub id: String,
    /// The answer after the colon, trimmed.
    pub answer: String,
    pub token: Result<Token, Refusal>,
}

/// One project line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLine {
    /// 1-based.
    pub line: usize,
    /// The slug, trimmed.
    pub slug: String,
}

/// The lines of a returned sheet or reply that count.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answers {
    pub projects: Vec<ProjectLine>,
    /// File order; a blank slot dropped when its ID has an answer, and
    /// every blank slot of an ID but the first.
    pub answers: Vec<AnswerLine>,
}

/// The project lines and answer lines of `text` (see the module
/// documentation).
pub fn parse_answers(text: &str) -> Answers {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut parsed = Answers::default();
    for (index, line) in text.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let number = index + 1;
        let Some(first) = line.chars().next() else {
            continue;
        };
        if first.is_whitespace() || first == '>' {
            continue;
        }
        if let Some(slug) = line.strip_prefix(PROJECT_LINE) {
            parsed.projects.push(ProjectLine {
                line: number,
                slug: slug.trim().to_owned(),
            });
            continue;
        }
        if let Some(answer) = answer_line(line, number) {
            parsed.answers.push(answer);
        }
    }
    settle_duplicates(&mut parsed.answers);
    parsed
}

/// The answer line `line` is, or `None` when it reads as no ID at all.
fn answer_line(line: &str, number: usize) -> Option<AnswerLine> {
    if let Some((id, answer)) = exact_answer(line) {
        let answer = answer.trim().to_owned();
        let token = if proposal_number(id).is_some() {
            token(&answer)
        } else {
            Err(Refusal::NoProposal)
        };
        return Some(AnswerLine {
            line: number,
            id: id.to_owned(),
            answer,
            token,
        });
    }
    let start = latin_start(line)?;
    Some(AnswerLine {
        line: number,
        id: start.latin.clone(),
        answer: start.rest.trim().to_owned(),
        token: Err(Refusal::NotAnAnswerLine {
            written: start.written,
            latin: start.latin,
            look_alike: start.look_alike,
        }),
    })
}

/// `PR-<digits>:` at the start of `line`: the ID and the rest after the
/// colon.
fn exact_answer(line: &str) -> Option<(&str, &str)> {
    let digits = line.strip_prefix(PROPOSAL_PREFIX)?.strip_prefix('-')?;
    let count = digits.bytes().take_while(u8::is_ascii_digit).count();
    if count == 0 {
        return None;
    }
    let rest = digits[count..].strip_prefix(':')?;
    let id_len = PROPOSAL_PREFIX.len() + 1 + count;
    Some((&line[..id_len], rest))
}

/// The start of a line that reads as an answer line only once made Latin.
struct LatinStart<'a> {
    /// As written, through the digits and the character after them.
    written: String,
    latin: String,
    /// A visible character of `written` is not ASCII.
    look_alike: bool,
    /// After a colon, else after the digits.
    rest: &'a str,
}

/// A line that starts `PR-<digit>` once leading invisible characters are
/// skipped, its look-alikes made Latin and its letters upper case.
fn latin_start(line: &str) -> Option<LatinStart<'_>> {
    let mut chars = line.char_indices().peekable();
    let mut visible = 0;
    while let Some(&(at, c)) = chars.peek() {
        if invisible(c) {
            chars.next();
        } else {
            visible = at;
            break;
        }
    }
    let mut latin = String::new();
    for want in format!("{PROPOSAL_PREFIX}-").chars() {
        let (_, c) = chars.next()?;
        if normalize_char(c).to_ascii_uppercase() != want {
            return None;
        }
        latin.push(want);
    }
    let mut digits = 0;
    let mut end = line.len();
    while let Some(&(at, c)) = chars.peek() {
        let latin_c = normalize_char(c);
        if latin_c.is_ascii_digit() {
            latin.push(latin_c);
            digits += 1;
            chars.next();
        } else {
            end = at;
            break;
        }
    }
    if digits == 0 {
        return None;
    }
    let (written_end, rest) = match line[end..].chars().next() {
        Some(next) => {
            let after = end + next.len_utf8();
            let rest = if normalize_char(next) == ':' {
                &line[after..]
            } else {
                &line[end..]
            };
            (after, rest)
        }
        None => (end, ""),
    };
    Some(LatinStart {
        written: line[..written_end].to_owned(),
        latin,
        look_alike: !line[visible..written_end].is_ascii(),
        rest,
    })
}

/// A character that shows nothing at the start of a line: a control,
/// a bidirectional mark, U+FEFF, a zero-width space or joiner, U+2060.
fn invisible(c: char) -> bool {
    is_escaped(c) || matches!(c, '\u{feff}' | '\u{200b}'..='\u{200d}' | '\u{2060}')
}

/// The token `answer` (trimmed) asks, or why it is refused.
fn token(answer: &str) -> Result<Token, Refusal> {
    if answer.is_empty() || answer == OPEN_WORD {
        return Ok(Token::Open);
    }
    if answer.eq_ignore_ascii_case(OK_WORD) {
        return Ok(Token::Ok);
    }
    if let Some(option) = option_number(answer) {
        return Ok(Token::Option(option));
    }
    if answer.eq_ignore_ascii_case(REJECT_WORD) {
        return Err(Refusal::RejectWithoutReason);
    }
    // The first word `reject`: a reject, its reason after the colon right
    // behind it; anything else after the word is refused (`rejected` is
    // another word).
    let word: usize = answer
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .map(char::len_utf8)
        .sum();
    if answer[..word].eq_ignore_ascii_case(REJECT_WORD) {
        let Some(reason) = answer[word..].strip_prefix(':') else {
            return Err(Refusal::RejectWithoutColon);
        };
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(Refusal::RejectWithoutReason);
        }
        return checked(reason, true).map(|reason| Token::Reject(reason.to_owned()));
    }
    checked(answer, false).map(|text| Token::Text(text.to_owned()))
}

/// `N` or `[N]`, ASCII digits: `N` (`u64::MAX` past a `u64`).
fn option_number(answer: &str) -> Option<u64> {
    let digits = answer
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(answer);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(digits.parse().unwrap_or(u64::MAX))
}

/// `text` within its cap (`reason`: a reject's) and free of a character
/// the queue escapes.
fn checked(text: &str, reason: bool) -> Result<&str, Refusal> {
    let max = if reason {
        ROUND_REASON_MAX_BYTES
    } else {
        ANSWER_MAX
    };
    if text.len() > max {
        return Err(Refusal::OverCap {
            reason,
            bytes: text.len(),
            max,
        });
    }
    match refused_char(text) {
        Some(c) => Err(Refusal::Escaped(c)),
        None => Ok(text),
    }
}

/// Two or more answers that are not blank for one ID refuse every exact
/// answer line of it, each holding the one shared list of the ID's lines; a
/// line that is no answer line counts as an answer of the ID its Latin form
/// names exactly (it keeps its own refusal). A blank slot of an answered
/// ID, and every blank slot of an ID but the first, is dropped.
fn settle_duplicates(answers: &mut Vec<AnswerLine>) {
    let exact = |answer: &AnswerLine| !matches!(answer.token, Err(Refusal::NotAnAnswerLine { .. }));
    let counts = |answer: &AnswerLine| exact(answer) || proposal_number(&answer.id).is_some();
    let mut lines: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for answer in answers.iter().filter(|answer| counts(answer)) {
        if !answer.answer.is_empty() {
            lines
                .entry(answer.id.clone())
                .or_default()
                .push(answer.line);
        }
    }
    let mut blank_seen: BTreeSet<String> = BTreeSet::new();
    answers.retain(|answer| {
        if !exact(answer) || !answer.answer.is_empty() {
            return true;
        }
        !lines.contains_key(&answer.id) && blank_seen.insert(answer.id.clone())
    });
    let shared: BTreeMap<String, Arc<[usize]>> = lines
        .into_iter()
        .filter(|(_, found)| found.len() > 1)
        .map(|(id, found)| (id, Arc::from(found)))
        .collect();
    for answer in answers.iter_mut().filter(|answer| exact(answer)) {
        if let Some(found) = shared.get(&answer.id) {
            answer.token = Err(Refusal::Duplicate(Arc::clone(found)));
        }
    }
}
