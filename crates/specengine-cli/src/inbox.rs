//! `spec inbox [--all]` (task spec `proposal-apply`, "Data", "Safety"): the
//! current repository's proposals by ID number, `open` and `approved`
//! (`--all`: every state), one line each: `<id> | <kind> | <status> |
//! <target_id> | <branch> | <created_at> | <rationale's first line, at most
//! 80 characters>`; a question's or a discrepancy's last column
//! `<severity>: <summary's first line>` (canon `agent-intake`,
//! "Review document"), an applied one's ending ` [<record_id>]` (task spec
//! `decision-apply`).
//! Proposals of another repository of the same slug (the database is the
//! slug's) are never listed: those of an existing one are counted in a
//! note; those of a repository that no longer exists (orphans) are named
//! in another (at most 10, the rest counted), with the `spec reject` that
//! takes them out unless a commit of theirs is in history. A row of the
//! queue that does not decode is skipped with a note naming it and its
//! column (exit 0; `spec review` of it exits 2 naming it). Times as
//! stored; nothing is written but the data directory. Control characters
//! in the text output are escaped ([`crate::escape_controls`]).

use serde::Serialize;
use specengine_store::{
    GitEnv, Proposal, ProposalFilter, ProposalQueue as _, ProposalStatus, same_repository,
};

use crate::proposals::{escaped_error, open_context, queue_cannot, repository_gone};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line};

/// The most characters of the rationale's first line `inbox` shows.
pub const INBOX_RATIONALE_CHARS: usize = 80;

/// The most IDs the note on a gone repository's proposals names; the rest
/// are counted (`… and N more`).
const GONE_IDS_MAX: usize = 10;

/// `spec inbox` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRequest {
    /// `--all`: every state, not only `open` and `approved`.
    pub all: bool,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// One listed proposal: the text line's fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InboxEntry {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub target_id: String,
    pub branch: String,
    pub created_at: String,
    /// An update's rationale's first line, at most
    /// [`INBOX_RATIONALE_CHARS`] characters (a longer one cut, ending in
    /// `…`); `null` for the intake kinds.
    pub rationale: Option<String>,
    /// A question's or a discrepancy's severity; `null` for an update.
    pub severity: Option<String>,
    /// A question's text's or a discrepancy's summary's first line, cut as
    /// the rationale; `null` for an update.
    pub summary: Option<String>,
    /// A question's or a discrepancy's decision record, from its step 7.
    pub record_id: Option<String>,
}

/// What `spec inbox` listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxOutcome {
    /// By ID number.
    pub proposals: Vec<InboxEntry>,
    /// Also printed as `note:` lines.
    pub notes: Vec<String>,
    pub messages: Vec<Message>,
}

#[derive(Serialize)]
struct InboxJson<'a> {
    proposals: &'a [InboxEntry],
    notes: &'a [String],
}

impl Serialize for InboxOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        InboxJson {
            proposals: &self.proposals,
            notes: &self.notes,
        }
        .serialize(serializer)
    }
}

/// `spec inbox`: the current repository's proposals.
pub fn inbox(
    env: &Env,
    globals: &Globals,
    request: &InboxRequest,
) -> Result<InboxOutcome, CliError> {
    run_inbox(env, globals, request).map_err(escaped_error)
}

fn run_inbox(
    env: &Env,
    globals: &Globals,
    request: &InboxRequest,
) -> Result<InboxOutcome, CliError> {
    let context = open_context(env, globals, &request.git)?;
    let statuses = if request.all {
        Vec::new()
    } else {
        vec![ProposalStatus::Open, ProposalStatus::Approved]
    };
    let listed = context
        .queue
        .list_readable(&ProposalFilter {
            git_common_dir: None,
            statuses,
        })
        .map_err(queue_cannot)?;
    let (ours, others): (Vec<_>, Vec<_>) = listed
        .proposals
        .into_iter()
        .partition(|proposal| same_repository(&proposal.place.git_common_dir, &context.common_dir));
    let proposals = ours.iter().map(entry).collect();
    let mut notes: Vec<String> = listed
        .unreadable
        .iter()
        .map(|row| one_line(&format!("{row}; not listed")))
        .collect();
    let (gone, existing): (Vec<_>, Vec<_>) = others.iter().partition(|p| repository_gone(p));
    if !existing.is_empty() {
        notes.push(format!(
            "{} proposal(s) of another repository of the project `{}` not listed: \
             `spec inbox` lists the current repository's",
            existing.len(),
            context.slug
        ));
    }
    if !gone.is_empty() {
        let mut ids = gone
            .iter()
            .take(GONE_IDS_MAX)
            .map(|proposal| proposal.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if gone.len() > GONE_IDS_MAX {
            ids.push_str(&format!(" … and {} more", gone.len() - GONE_IDS_MAX));
        }
        notes.push(format!(
            "{} proposal(s) of a repository that no longer exists not listed: {ids}; `spec \
             reject <ID> --reason …` takes an open or approved one out of the inbox unless its \
             commit is in history",
            gone.len()
        ));
    }
    let messages = notes.iter().cloned().map(Message::Note).collect();
    Ok(InboxOutcome {
        proposals,
        notes,
        messages,
    })
}

/// One proposal's line.
fn entry(proposal: &Proposal) -> InboxEntry {
    let intake = proposal.intake.as_ref();
    InboxEntry {
        id: proposal.id.clone(),
        kind: proposal.kind.as_str().to_owned(),
        status: proposal.status.as_str().to_owned(),
        target_id: proposal.target_id.clone(),
        branch: proposal.place.branch.clone(),
        created_at: proposal.created_at.clone(),
        rationale: intake.is_none().then(|| first_line(&proposal.rationale)),
        severity: intake.map(|intake| intake.severity.as_str().to_owned()),
        summary: intake.map(|intake| first_line(&intake.summary)),
        record_id: proposal.record.as_ref().map(|record| record.id.clone()),
    }
}

/// The first line of `text` (a `\r` before its end dropped), at most
/// [`INBOX_RATIONALE_CHARS`] characters: a longer one keeps one less and
/// ends in `…`.
fn first_line(text: &str) -> String {
    let line = text.split('\n').next().unwrap_or_default();
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.chars().count() <= INBOX_RATIONALE_CHARS {
        return line.to_owned();
    }
    let mut cut: String = line.chars().take(INBOX_RATIONALE_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// One line per proposal, control characters escaped.
pub(crate) fn render_text(outcome: &InboxOutcome) -> String {
    escape_controls(&raw_text(outcome))
}

fn raw_text(outcome: &InboxOutcome) -> String {
    let mut out = String::new();
    for entry in &outcome.proposals {
        let mut last = match (&entry.severity, &entry.summary) {
            (Some(severity), Some(summary)) => format!("{severity}: {summary}"),
            _ => entry.rationale.clone().unwrap_or_default(),
        };
        if entry.status == ProposalStatus::Applied.as_str()
            && let Some(record) = &entry.record_id
        {
            last.push_str(&format!(" [{record}]"));
        }
        out.push_str(&one_line(&format!(
            "{} | {} | {} | {} | {} | {} | {last}",
            entry.id, entry.kind, entry.status, entry.target_id, entry.branch, entry.created_at,
        )));
        out.push('\n');
    }
    out
}
