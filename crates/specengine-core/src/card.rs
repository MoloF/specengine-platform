//! The engine's own words of a card (task spec `card-store`, "Rules and edge
//! cases"; ADR-0056): the `C-NNNN` identifier and its look-alikes, the
//! `[ids]` clash with it, the pure move decision [`card_move`], what a closed
//! card and an acceptance refuse, and the shape of each field. Pure: nothing
//! is read or written here, and no project's flow, column or discipline
//! appears: `C` is the engine's, the rest comes in as data. Both the CLI and
//! the store, again under its lock, ask these functions.

use serde::Serialize;
use specengine_model::script::normalize_char;
use specengine_model::{Card, EvidenceKind, IdScheme};

use crate::card_toml::CardFlow;
use crate::check::is_calendar_date;
use crate::record::is_escaped;
use crate::roadmap::parse_estimate;
use crate::walk_scope::is_clean_relative;

/// The prefix of every card ID; a project's `[ids]` may not take it.
pub const CARD_PREFIX: &str = "C";

/// The fewest digits of a card ID: `C-0001`; more past `C-9999`.
pub const CARD_DIGITS: usize = 4;

/// Who acts when `--as` is absent.
pub const LOCAL_ACTOR: &str = "local";

/// Who a tool acts as: a flow admits tools by listing it in `by`.
pub const AGENT_ACTOR: &str = "agent";

/// What `edit` and `assign` read as "clear this field".
pub const CLEAR: &str = "-";

/// The most evidence entries of a card.
pub const EVIDENCE_ENTRIES_MAX: usize = 32;

/// The most bytes of one evidence value.
pub const EVIDENCE_VALUE_MAX: usize = 1024;

/// The most `card.*` events `--history` gives, the last ones.
pub const HISTORY_MAX: usize = 64;

/// The most bytes of a card person's name, trimmed.
pub const CARD_PERSON_MAX: usize = 128;

/// The card ID of `number`: `C-` and the number, zero-padded to
/// [`CARD_DIGITS`].
pub fn card_id(number: u64) -> String {
    format!("{CARD_PREFIX}-{number:0width$}", width = CARD_DIGITS)
}

/// The number of a card ID written exactly as [`card_id`] writes it
/// (`C-0001`, `C-10000`; never `C-00001`, `C-1`, `c-0001`).
pub fn card_number(id: &str) -> Option<u64> {
    let digits = id.strip_prefix(CARD_PREFIX)?.strip_prefix('-')?;
    if digits.len() < CARD_DIGITS || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u64 = digits.parse().ok()?;
    (card_id(number) == id).then_some(number)
}

/// Why a written card ID was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardIdError {
    /// Look-alike letters or digits whose Latin form is a card ID: `fix`
    /// (exit 2, naming it).
    LookAlike { fix: String },
    /// Not a card ID in any script.
    NotAnId,
}

/// The card ID `written` names (surrounding whitespace ignored); a
/// look-alike of one is [`CardIdError::LookAlike`] with the Latin form
/// (ADR-0009: never taken silently).
pub fn parse_card_id(written: &str) -> Result<String, CardIdError> {
    let written = written.trim();
    if card_number(written).is_some() {
        return Ok(written.to_owned());
    }
    let fix: String = written.chars().map(normalize_char).collect();
    if fix != written && card_number(&fix).is_some() {
        return Err(CardIdError::LookAlike { fix });
    }
    Err(CardIdError::NotAnId)
}

/// One line naming the Latin form of a look-alike card ID.
pub fn card_look_alike_message(written: &str, fix: &str) -> String {
    format!("`{written}` uses look-alike letters or digits; card IDs are Latin only: write `{fix}`")
}

/// Why the project's `[ids]` clashes with the engine's [`CARD_PREFIX`]: a
/// configured prefix `C`, or an `aliases_from` entry that reads `C` once its
/// look-alikes are normalised. `None`: no clash.
pub fn card_prefix_clash(scheme: &IdScheme) -> Option<String> {
    for spec in scheme.prefixes() {
        if spec.prefix == CARD_PREFIX {
            return Some(format!(
                "the project's `[ids]` configures the prefix `{CARD_PREFIX}`, which is \
                 SpecEngine's card prefix (`{CARD_PREFIX}-0001`): rename it before using cards"
            ));
        }
        for alias in &spec.aliases_from {
            let normalized: String = alias.chars().map(normalize_char).collect();
            if normalized == CARD_PREFIX {
                return Some(format!(
                    "the project's `[ids]` lists `{alias}` in `aliases_from` of `{}`, which reads \
                     as SpecEngine's card prefix `{CARD_PREFIX}`: rename it before using cards",
                    spec.prefix
                ));
            }
        }
    }
    None
}

/// One move open to a card: a column and who makes it (empty: anyone).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct AllowedMove {
    pub to: String,
    pub by: Vec<String>,
}

/// The moves `flow` lists from `from`, in the flow's order; a column the
/// flow no longer holds has one way back, the flow's first column, open to
/// anyone.
fn moves_from(flow: &CardFlow, from: &str) -> Vec<AllowedMove> {
    if !flow.holds(from) {
        return vec![AllowedMove {
            to: flow.first_column().to_owned(),
            by: Vec::new(),
        }];
    }
    flow.moves
        .iter()
        .filter(|listed| listed.from == from)
        .map(|listed| AllowedMove {
            to: listed.to.clone(),
            by: listed.by.clone(),
        })
        .collect()
}

/// What `card` may do next: its flow's moves from its column; nothing when
/// it is closed or its flow (`None`) is gone.
pub fn allowed_moves(flow: Option<&CardFlow>, card: &Card) -> Vec<AllowedMove> {
    match flow {
        Some(flow) if !card.is_closed() => moves_from(flow, &card.column),
        _ => Vec::new(),
    }
}

/// Why [`card_move`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveRefusal {
    /// No such move: the columns open from here, and whether `spec card
    /// accept` takes the card on from here.
    NoMove { allowed: Vec<String>, accepts: bool },
    /// The move is `by`'s, and the actor is not among them.
    NotBy { by: Vec<String> },
}

impl MoveRefusal {
    /// ``C-0001 is in <from>: no move to `<to>`; allowed: `<column>`, `spec
    /// card accept`; nothing changed``, or the same naming `by`.
    pub fn message(&self, id: &str, from: &str, to: &str, actor: &str) -> String {
        match self {
            Self::NoMove { allowed, accepts } => {
                let mut open: Vec<String> =
                    allowed.iter().map(|column| format!("`{column}`")).collect();
                if *accepts {
                    open.push("`spec card accept`".to_owned());
                }
                let open = if open.is_empty() {
                    "nothing".to_owned()
                } else {
                    open.join(", ")
                };
                format!("{id} is in {from}: no move to `{to}`; allowed: {open}; nothing changed")
            }
            Self::NotBy { by } => format!(
                "{id} is in {from}: the move to `{to}` is {}'s, not `{actor}`'s; nothing changed",
                names_text(by)
            ),
        }
    }
}

/// `` `a` ``, `` `a` or `b` ``, `` `a`, `b` or `c` ``.
fn names_text(names: &[String]) -> String {
    match names {
        [] => "nobody".to_owned(),
        [one] => format!("`{one}`"),
        [head @ .., last] => format!(
            "{} or `{last}`",
            head.iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The pure decision of a move from `from` to `to` under `flow`, made by
/// `actor`. A move within one column is a reorder, no rule. A column the
/// flow lists no move to, `accept.to` among them, refuses with what is
/// allowed; a listed move refuses an `actor` outside its `by`.
pub fn card_move(flow: &CardFlow, from: &str, to: &str, actor: &str) -> Result<(), MoveRefusal> {
    if from == to {
        return Ok(());
    }
    let open = moves_from(flow, from);
    match open.iter().find(|allowed| allowed.to == to) {
        Some(allowed) if allowed.by.is_empty() || allowed.by.iter().any(|name| name == actor) => {
            Ok(())
        }
        Some(allowed) => Err(MoveRefusal::NotBy {
            by: allowed.by.clone(),
        }),
        None => Err(MoveRefusal::NoMove {
            allowed: open.into_iter().map(|allowed| allowed.to).collect(),
            accepts: flow.accept.from == from,
        }),
    }
}

/// Why `card` takes no write: it is accepted or cancelled, and a correction
/// is a new card.
pub fn closed_refusal(card: &Card) -> Option<String> {
    let state = if card.acceptance.is_some() {
        "accepted"
    } else if card.cancellation.is_some() {
        "cancelled"
    } else {
        return None;
    };
    Some(format!(
        "{id} is {state}: it takes no write; a correction is a new card, `spec card new \
         --corrects {id}`; nothing changed",
        id = card.id
    ))
}

/// The line of a lost compare-and-set on `card`: tasks', `in <column>` for
/// its state.
pub fn stale_refusal(card: &Card) -> String {
    format!(
        "`{}` changed since this run read it: it is in {} since {}; nothing changed",
        card.id, card.column, card.updated_at
    )
}

/// Why `actor` may not accept `card` under `flow` (`None`: its flow is
/// gone): the card must stand in `accept.from` and `actor` be its
/// `reviewer`, never its `assignee`, never a name no person holds.
pub fn accept_refusal(card: &Card, flow: Option<&CardFlow>, actor: &str) -> Option<String> {
    if let Some(closed) = closed_refusal(card) {
        return Some(closed);
    }
    let id = &card.id;
    let Some(flow) = flow else {
        return Some(format!(
            "{id} is in the flow `{}`, which `[card]` no longer holds: nothing accepts it; \
             nothing changed",
            card.flow
        ));
    };
    if card.column != flow.accept.from {
        return Some(format!(
            "{id} is in {}: `accept` needs {}; nothing changed",
            card.column, flow.accept.from
        ));
    }
    if let Some(problem) = reviewer_problem(actor) {
        return Some(format!("{id}: {problem}; nothing changed"));
    }
    if card.assignee.as_deref() == Some(actor) {
        return Some(format!(
            "{id}: `{actor}` is its assignee, who never accepts a result; nothing changed"
        ));
    }
    match card.reviewer.as_deref() {
        Some(reviewer) if reviewer == actor => None,
        Some(reviewer) => Some(format!(
            "{id}: its reviewer is `{reviewer}`, and `--as` says `{actor}`; nothing changed"
        )),
        None => Some(format!(
            "{id} has no reviewer: `spec card assign {id} --reviewer N` names one; nothing changed"
        )),
    }
}

/// Why `name` can be no reviewer: [`LOCAL_ACTOR`] and [`AGENT_ACTOR`] are no
/// one's name.
pub fn reviewer_problem(name: &str) -> Option<String> {
    (name == LOCAL_ACTOR || name == AGENT_ACTOR)
        .then(|| format!("`{name}` can be no reviewer: a person accepts a result"))
}

/// A card's person as stored and compared: `written`, trimmed. Every name
/// passes here (`--as`, `assignee`, `reviewer`, a move's `by`), so `by` and
/// the reviewer compare exact on the same text.
pub fn card_person(written: &str) -> &str {
    written.trim()
}

/// Why `written` can be no card's person, trimmed: empty, over
/// [`CARD_PERSON_MAX`] bytes, or holding a control character, U+2028, U+2029
/// or a bidirectional mark. Any script and inner spaces stand: a person's
/// name is typed as it is spoken and is no ID, so the Latin-only rule of IDs
/// does not reach it. [`LOCAL_ACTOR`] and [`AGENT_ACTOR`] pass here; only
/// [`reviewer_problem`] refuses them.
pub fn card_person_problem(written: &str) -> Option<String> {
    let name = card_person(written);
    if name.is_empty() {
        return Some("empty; give a value or leave the option out".to_owned());
    }
    if name.len() > CARD_PERSON_MAX {
        return Some(format!("{} bytes; at most {CARD_PERSON_MAX}", name.len()));
    }
    name.chars()
        .find(|&c| c.is_control() || is_escaped(c) || matches!(c, '\u{2028}' | '\u{2029}'))
        .map(|c| {
            format!(
                "U+{:04X} in a name: one line, no control character or bidirectional mark",
                u32::from(c)
            )
        })
}

/// Why `login`, as a proxy sent it, can be no person on a hub. Pure: it
/// reads and writes nothing. A login is recorded verbatim and compared
/// exactly, so one person must have one spelling: refused are text the store
/// would trim, what no card's person may hold, a comma (a proxy folding a
/// client's copy gives `eve, mira`), a format character (`mira` and a
/// zero-width character read as `mira` and are two people), [`LOCAL_ACTOR`]
/// and [`AGENT_ACTOR`] in any case, and [`CLEAR`].
///
/// The login door's duty, not judged here: a Latin look-alike (a Cyrillic
/// `o`), NFC against NFD, and an invisible mark outside category Cf (U+034F,
/// a variation selector).
pub fn hub_login_problem(login: &str) -> Option<String> {
    if card_person(login) != login {
        return Some("white space around the login".to_owned());
    }
    if let Some(problem) = card_person_problem(login) {
        return Some(problem);
    }
    if login.contains(',') {
        return Some("a comma: one login, not a list".to_owned());
    }
    if let Some(c) = login.chars().find(|&c| is_format_character(c)) {
        return Some(format!(
            "U+{:04X} in a name: no format character, which can read as another person",
            u32::from(c)
        ));
    }
    if login == CLEAR {
        return Some(format!(
            "`{CLEAR}` is the record's word for clearing a name"
        ));
    }
    reviewer_problem(&login.to_ascii_lowercase())
        .map(|_| format!("`{login}` is the record's word for no person"))
}

/// Unicode's general category Cf, which `char` does not expose (the same
/// ranges in Unicode 15.1 and 17.0): what a login refuses, and what a hub's
/// confirm escapes in the text it asks to be confirmed (CLI `confirm.rs`; the
/// web UI's `ui/text.ts` keeps a copy of these ranges, which must agree).
pub fn is_format_character(c: char) -> bool {
    matches!(
        c,
        '\u{ad}'
            | '\u{600}'..='\u{605}'
            | '\u{61c}'
            | '\u{6dd}'
            | '\u{70f}'
            | '\u{890}'..='\u{891}'
            | '\u{8e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            | '\u{e0001}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

/// Why `written` is no estimate: [`parse_estimate`]'s, no unit.
pub fn estimate_problem(written: &str) -> Option<String> {
    parse_estimate(written)
        .is_none()
        .then(|| format!("estimate: `{written}` is not `N` or `N-M`"))
}

/// Why `written` is no `--due`: a calendar date `YYYY-MM-DD`.
pub fn due_problem(written: &str) -> Option<String> {
    (!is_calendar_date(written)).then(|| format!("due: `{written}` is no date YYYY-MM-DD"))
}

/// The entry `--evidence KIND=VALUE` writes, else why not: `url`
/// (`http://`, `https://`), `path` (`<path>@<hex blob>`, a clean relative
/// path at lower-case hex digits), `build`, `note`; one line, at most
/// [`EVIDENCE_VALUE_MAX`] bytes. Nothing is fetched and no value is read.
pub fn parse_evidence(written: &str) -> Result<(EvidenceKind, String), String> {
    let Some((kind_text, value)) = written.split_once('=') else {
        return Err(format!("evidence: `{written}` is no KIND=VALUE"));
    };
    let Some(kind) = EvidenceKind::parse(kind_text) else {
        let kinds: Vec<String> = EvidenceKind::ALL
            .iter()
            .map(|kind| kind.as_str().to_owned())
            .collect();
        return Err(format!(
            "evidence: `{kind_text}` is no kind: {}",
            names_text(&kinds)
        ));
    };
    if value.trim().is_empty() {
        return Err(format!("evidence: the `{kind_text}` value is empty"));
    }
    if value.len() > EVIDENCE_VALUE_MAX {
        return Err(format!(
            "evidence: {} bytes; at most {EVIDENCE_VALUE_MAX}",
            value.len()
        ));
    }
    if value.chars().any(char::is_control) {
        return Err("evidence: one line, no control character".to_owned());
    }
    let problem = match kind {
        EvidenceKind::Url => (!is_web_url(value)).then(|| {
            format!("evidence: `{value}` is no `http://` or `https://` URL; nothing is fetched")
        }),
        EvidenceKind::Path => (!is_pinned_path(value))
            .then(|| format!("evidence: `{value}` is no `<path>@<hex blob>`")),
        EvidenceKind::Build | EvidenceKind::Note => None,
    };
    match problem {
        Some(problem) => Err(problem),
        None => Ok((kind, value.to_owned())),
    }
}

fn is_web_url(value: &str) -> bool {
    ["http://", "https://"].iter().any(|scheme| {
        value
            .strip_prefix(scheme)
            .is_some_and(|rest| !rest.is_empty())
    })
}

fn is_pinned_path(value: &str) -> bool {
    let Some((path, blob)) = value.rsplit_once('@') else {
        return false;
    };
    is_clean_relative(path)
        && !blob.is_empty()
        && blob
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
