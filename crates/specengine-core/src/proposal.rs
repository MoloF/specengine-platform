//! The engine's own words of the proposal queue (task spec
//! `proposal-apply`, "Data", "Safety"): the `PR-NNNN` identifier and its
//! look-alikes, the `[ids]` clash with it, the author's provenance, the
//! commit message of an applied proposal, the bytes its `patch_hash`
//! covers, and the queue's UTC time stamps. Pure: nothing is read, written
//! or hashed here (the content hash is the store's `b3_hash`), and no
//! project's prefix or word appears (ADR-0008): `PR` and `spec: apply` are
//! the engine's.

use serde::{Deserialize, Serialize};
use specengine_model::IdScheme;
use specengine_model::script::normalize_char;

use crate::check::date_from_unix_days;

/// The prefix of every proposal ID; a project's `[ids]` may not take it
/// (owner's Q7).
pub const PROPOSAL_PREFIX: &str = "PR";

/// The fewest digits of a proposal ID: `PR-0001`; more past `PR-9999`.
pub const PROPOSAL_DIGITS: usize = 4;

/// The most bytes of a proposed text (`propose update`, a proposed patch):
/// 1 MiB.
pub const TEXT_MAX_BYTES: usize = 1 << 20;

/// The most bytes of an author's `role`, `model` or `run`.
pub const AUTHOR_FIELD_MAX: usize = 128;

/// The subject line's start of an apply commit: `spec: apply PR-0001`.
pub const COMMIT_SUBJECT: &str = "spec: apply";

/// The trailer naming the applied proposal, the one an interrupted apply is
/// found by.
pub const PROPOSAL_TRAILER: &str = "Proposal";

/// The proposal ID of `number`: `PR-` and the number, zero-padded to
/// [`PROPOSAL_DIGITS`].
pub fn proposal_id(number: u64) -> String {
    format!(
        "{PROPOSAL_PREFIX}-{number:0width$}",
        width = PROPOSAL_DIGITS
    )
}

/// The number of a proposal ID written exactly as [`proposal_id`] writes
/// it (`PR-0001`, `PR-10000`; never `PR-00001`, `PR-1`, `pr-0001`).
pub fn proposal_number(id: &str) -> Option<u64> {
    let digits = id.strip_prefix(PROPOSAL_PREFIX)?.strip_prefix('-')?;
    if digits.len() < PROPOSAL_DIGITS || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u64 = digits.parse().ok()?;
    (proposal_id(number) == id).then_some(number)
}

/// Why a written proposal ID was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalIdError {
    /// Look-alike letters or digits (fullwidth forms, Cyrillic or Greek
    /// capitals) whose Latin form is a proposal ID: `fix` (exit 2, naming
    /// it).
    LookAlike { fix: String },
    /// Not a proposal ID in any script.
    NotAnId,
}

/// The proposal ID `written` names (surrounding whitespace ignored): itself
/// when it is one ([`proposal_number`]); a look-alike of one is
/// [`ProposalIdError::LookAlike`] with the Latin form (ADR-0009: never
/// taken silently).
pub fn parse_proposal_id(written: &str) -> Result<String, ProposalIdError> {
    let written = written.trim();
    if proposal_number(written).is_some() {
        return Ok(written.to_owned());
    }
    let fix: String = written.chars().map(normalize_char).collect();
    if fix != written && proposal_number(&fix).is_some() {
        return Err(ProposalIdError::LookAlike { fix });
    }
    Err(ProposalIdError::NotAnId)
}

/// One line naming the Latin form of a look-alike proposal ID.
pub fn look_alike_message(written: &str, fix: &str) -> String {
    format!(
        "`{written}` uses look-alike letters or digits; proposal IDs are Latin only: write `{fix}`"
    )
}

/// Why the project's `[ids]` clashes with the engine's [`PROPOSAL_PREFIX`]:
/// a configured prefix `PR`, or an `aliases_from` entry that is `PR` or
/// reads `PR` once its look-alikes are normalised. `None`: no clash.
pub fn prefix_clash(scheme: &IdScheme) -> Option<String> {
    for spec in scheme.prefixes() {
        if spec.prefix == PROPOSAL_PREFIX {
            return Some(format!(
                "the project's `[ids]` configures the prefix `{PROPOSAL_PREFIX}`, which is \
                 SpecEngine's proposal prefix (`{PROPOSAL_PREFIX}-0001`): rename it before \
                 using the proposal queue"
            ));
        }
        for alias in &spec.aliases_from {
            let normalized: String = alias.chars().map(normalize_char).collect();
            if normalized == PROPOSAL_PREFIX {
                return Some(format!(
                    "the project's `[ids]` lists `{alias}` in `aliases_from` of `{}`, which reads \
                     as SpecEngine's proposal prefix `{PROPOSAL_PREFIX}`: rename it before using \
                     the proposal queue",
                    spec.prefix
                ));
            }
        }
    }
    None
}

/// Who raised a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthorType {
    Human,
    Agent,
}

impl AuthorType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
        }
    }
}

/// The author's self-reported provenance (A5), stored as the JSON
/// `{type, role, model, run}`; an absent field is `null` and reads
/// `unknown` in the commit.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Author {
    #[serde(rename = "type")]
    pub author_type: AuthorType,
    pub role: Option<String>,
    pub model: Option<String>,
    pub run: Option<String>,
}

impl Author {
    /// A human author: no role, model or run.
    pub const fn human() -> Self {
        Self {
            author_type: AuthorType::Human,
            role: None,
            model: None,
            run: None,
        }
    }

    /// `--author-role`, `--author-model`, `--run`: any given → an agent.
    /// Each given value must be [`author_field_problem`]-free; the first
    /// problem is the error (one line, naming the option).
    pub fn new(
        role: Option<String>,
        model: Option<String>,
        run: Option<String>,
    ) -> Result<Self, String> {
        for (name, value) in [
            ("--author-role", &role),
            ("--author-model", &model),
            ("--run", &run),
        ] {
            if let Some(problem) = value.as_deref().and_then(author_field_problem) {
                return Err(format!("{name}: {problem}"));
            }
        }
        let author_type = if role.is_some() || model.is_some() || run.is_some() {
            AuthorType::Agent
        } else {
            AuthorType::Human
        };
        Ok(Self {
            author_type,
            role,
            model,
            run,
        })
    }

    /// `<type> role=<role> model=<model> run=<run>`, each absent field
    /// `unknown`: the `Proposed-by:` trailer's value.
    pub fn provenance(&self) -> String {
        let field = |value: &Option<String>| value.clone().unwrap_or_else(|| "unknown".to_owned());
        format!(
            "{} role={} model={} run={}",
            self.author_type.as_str(),
            field(&self.role),
            field(&self.model),
            field(&self.run)
        )
    }
}

/// Why `value` cannot be an author field: printable ASCII without spaces,
/// 1 to [`AUTHOR_FIELD_MAX`] bytes. `None`: it can.
pub fn author_field_problem(value: &str) -> Option<String> {
    if value.is_empty() {
        return Some("empty; give a value or leave the option out".to_owned());
    }
    if value.len() > AUTHOR_FIELD_MAX {
        return Some(format!("{} bytes; at most {AUTHOR_FIELD_MAX}", value.len()));
    }
    if !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Some("printable ASCII without spaces only".to_owned());
    }
    None
}

/// What the commit of an applied proposal records (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitFacts<'a> {
    pub id: &'a str,
    /// Verbatim.
    pub rationale: &'a str,
    /// `Name <email>`: the committer identity without its date.
    pub decided_by: &'a str,
    pub author: &'a Author,
    /// The 40 (or 64) hex digits of `HEAD` at creation.
    pub base_commit: &'a str,
}

/// The message of the apply commit, given to `git commit --cleanup=verbatim`:
///
/// ```text
/// spec: apply PR-0001
///
/// <rationale, verbatim>
///
/// Proposal: PR-0001
/// Decided-by: <name> <email>
/// Proposed-by: <type> role=<r> model=<m> run=<run>
/// Base-commit: <hex>
/// ```
pub fn commit_message(facts: &CommitFacts<'_>) -> String {
    format!(
        "{COMMIT_SUBJECT} {id}\n\n{rationale}\n\n{PROPOSAL_TRAILER}: {id}\nDecided-by: {decided_by}\n\
         Proposed-by: {provenance}\nBase-commit: {base}\n",
        id = facts.id,
        rationale = facts.rationale,
        decided_by = facts.decided_by,
        provenance = facts.author.provenance(),
        base = facts.base_commit,
    )
}

/// The bytes `patch_hash` is the `b3:` hash of (07 §1.2): `target_id`, LF,
/// `base_hash`, LF, `new_text`.
pub fn patch_hash_input(target_id: &str, base_hash: &str, new_text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(target_id.len() + base_hash.len() + new_text.len() + 2);
    bytes.extend_from_slice(target_id.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(base_hash.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(new_text.as_bytes());
    bytes
}

/// A UTC time stamp `YYYY-MM-DDTHH:MM:SSZ` of a count of seconds since
/// 1970-01-01T00:00:00Z: the queue's `created_at`, `updated_at`,
/// `decided_at` and event `at` (stored, never shown relative).
pub fn utc_timestamp(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds = unix_seconds.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        date_from_unix_days(days),
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

/// `text` has the shape of [`utc_timestamp`]'s output.
pub fn is_utc_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.is_ascii()
        && bytes.len() == 20
        && crate::check::is_calendar_date(&text[..10])
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && [11, 12, 14, 15, 17, 18]
            .iter()
            .all(|&at| bytes[at].is_ascii_digit())
        && text[11..13] < *"24"
        && text[14..16] < *"60"
        && text[17..19] < *"60"
}
