//! A node's history, the pure half (task spec `node-history`, NH1, NH2):
//! the trailer parse of a commit's message, the identity and provenance
//! names an entry shows, and the judge of one commit's two versions of a
//! file, which lists the commit only when the node's span bytes differ.
//!
//! Nothing is read, written or stored here: the store reads git's log and
//! blobs, the CLI joins the queue and orders the entries. The trailers are
//! parsed from the raw message (`%B`), never with git's trailer options: an
//! apply commit's subject is `spec: apply <ID>` and its last paragraph holds
//! `Proposal: <the same ID>`; any other commit is a hand commit. A
//! `Proposal:` line inside the rationale never counts (NH2).

use specengine_model::ParsedFile;

use crate::patch::span_bytes;
use crate::proposal::{COMMIT_SUBJECT, PROPOSAL_TRAILER, REVISION_TRAILER, editorial_revision};
use crate::revision::revision;

/// Entries one node's history lists at most (NH9).
pub const HISTORY_MAX: usize = 64;

/// Commits of the holder's file the scan judges at most; one more is read
/// to know whether the scan stopped short (NH9).
pub const HISTORY_SCAN_MAX: usize = 128;

/// Characters of a rationale or a title an entry keeps, then `…` (NH10).
pub const HISTORY_TEXT_MAX: usize = 300;

/// The trailer naming who decided an apply.
const DECIDED_BY_TRAILER: &str = "Decided-by";

/// The trailer naming who proposed an apply.
const PROPOSED_BY_TRAILER: &str = "Proposed-by";

/// The `unknown` an absent provenance field reads in a commit.
const UNKNOWN: &str = "unknown";

/// What an apply's `Revision:` trailer says of one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Revised {
    /// `Revision: R-12 1 -> 2`.
    Raised { from: u32, to: u32 },
    /// `Revision: R-12 2 (editorial)`: the revision kept.
    Editorial { rev: u32 },
}

/// The trailers of an apply commit's last paragraph, as written.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppliedCommit {
    /// `Proposal:`, the subject's ID.
    pub proposal: String,
    /// `Decided-by:`, the first.
    pub decided_by: Option<String>,
    /// `Proposed-by:`, the first.
    pub proposed_by: Option<String>,
    /// Each readable `Revision:`, in order: the node's ID and what it says.
    pub revisions: Vec<(String, Revised)>,
    /// The text between the subject's blank line and the trailer block,
    /// trimmed; `None` when empty.
    pub rationale: Option<String>,
}

/// What one commit did to a node, as an entry's `change` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommitChange {
    /// An apply whose `Revision:` raises the node.
    Raised,
    /// An apply whose `Revision:` keeps the node's revision.
    Editorial,
    /// An apply with no `Revision:` for the node.
    Applied,
    /// A commit with no apply trailers.
    Hand,
}

impl CommitChange {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raised => "raised",
            Self::Editorial => "editorial",
            Self::Applied => "applied",
            Self::Hand => "hand",
        }
    }

    /// The change of a commit whose trailers are `applied` (`None`: a hand
    /// commit) to the node `id` (`None`: an ID-less node, never named by a
    /// `Revision:`).
    pub fn of(applied: Option<&AppliedCommit>, id: Option<&str>) -> Self {
        let Some(applied) = applied else {
            return Self::Hand;
        };
        let said = id.and_then(|id| {
            applied
                .revisions
                .iter()
                .find(|(named, _)| named == id)
                .map(|(_, revised)| *revised)
        });
        match said {
            Some(Revised::Raised { .. }) => Self::Raised,
            Some(Revised::Editorial { .. }) => Self::Editorial,
            None => Self::Applied,
        }
    }
}

/// The apply trailers of the raw commit message `message`; `None`: a hand
/// commit. The subject must be `spec: apply <ID>` and the message's last
/// paragraph must hold `Proposal: <ID>`, the same ID; only that paragraph
/// is read as trailers, so a `Proposal:` line in the rationale never
/// counts.
pub fn applied_trailers(message: &str) -> Option<AppliedCommit> {
    let lines: Vec<&str> = message.lines().collect();
    let subject = lines.first()?;
    let id = subject.strip_prefix(COMMIT_SUBJECT)?.strip_prefix(' ')?;
    if id.is_empty() || id.chars().any(char::is_whitespace) {
        return None;
    }
    // The last paragraph after the subject: its lines up to the last
    // non-blank one, back to the blank line before them.
    let body = &lines[1..];
    let end = body.iter().rposition(|line| !line.trim().is_empty())? + 1;
    let start = body[..end]
        .iter()
        .rposition(|line| line.trim().is_empty())
        .map_or(0, |blank| blank + 1);
    if start == 0 {
        // No blank line between the subject and the last paragraph: it is
        // the subject's own paragraph.
        return None;
    }
    let trailers = &body[start..end];
    let mut proposal = false;
    let mut decided_by = None;
    let mut proposed_by = None;
    let mut revisions = Vec::new();
    for line in trailers {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key {
            PROPOSAL_TRAILER => proposal |= value == id,
            DECIDED_BY_TRAILER => {
                decided_by.get_or_insert_with(|| value.to_owned());
            }
            PROPOSED_BY_TRAILER => {
                proposed_by.get_or_insert_with(|| value.to_owned());
            }
            REVISION_TRAILER => revisions.extend(revision_value(value)),
            _ => {}
        }
    }
    if !proposal {
        return None;
    }
    let rationale = body[..start].join("\n");
    let rationale = rationale.trim();
    Some(AppliedCommit {
        proposal: id.to_owned(),
        decided_by,
        proposed_by,
        revisions,
        rationale: (!rationale.is_empty()).then(|| rationale.to_owned()),
    })
}

/// A `Revision:` trailer's value: `R-12 1 -> 2` or `R-12 2 (editorial)`
/// (`editorial_revision`'s grammar); `None` for any other text.
fn revision_value(value: &str) -> Option<(String, Revised)> {
    if let Some(id) = editorial_revision(value) {
        let rev = value
            .strip_prefix(id)?
            .trim_start()
            .split(' ')
            .next()?
            .parse()
            .ok()?;
        return Some((id.to_owned(), Revised::Editorial { rev }));
    }
    let (rest, to) = value.rsplit_once(" -> ")?;
    let (id, from) = rest.rsplit_once(' ')?;
    if id.is_empty() || id.contains(' ') {
        return None;
    }
    Some((
        id.to_owned(),
        Revised::Raised {
            from: from.parse().ok()?,
            to: to.parse().ok()?,
        },
    ))
}

/// The name part of an identity as written: `Ann Owner <ann@example.org>`
/// gives `Ann Owner`; a value without a ` <…>` suffix is the whole.
pub fn identity_name(value: &str) -> &str {
    value
        .strip_suffix('>')
        .and_then(|rest| rest.rsplit_once(" <"))
        .map_or(value, |(name, _)| name)
}

/// Who proposed, from a `Proposed-by:` value: its type and its role,
/// `agent role=spec-writer model=m run=r` giving `agent spec-writer`; an
/// `unknown` or absent role gives the type alone. `None` when the value
/// does not start with a type word.
pub fn proposer(proposed_by: &str) -> Option<String> {
    let mut words = proposed_by.split_whitespace();
    let kind = words.next()?;
    if kind.contains('=') {
        return None;
    }
    match words.find_map(|word| word.strip_prefix("role=")) {
        Some(role) if !role.is_empty() && role != UNKNOWN => Some(format!("{kind} {role}")),
        _ => Some(kind.to_owned()),
    }
}

/// Which node of a file a history follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryKey<'a> {
    /// The first node holding this ID.
    Id(&'a str),
    /// The whole file: an ID-less REF.
    File,
}

/// A node as one version of its file holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeVersion {
    /// Its span's bytes: a section's span, a document's or the whole file.
    pub span: Vec<u8>,
    /// Its revision: none written is 1; unknown `None`.
    pub rev: Option<u32>,
}

/// The node `key` names in `parsed`, the parse of `bytes`; `None` when this
/// version holds no such node.
pub fn node_version(parsed: &ParsedFile, bytes: &[u8], key: HistoryKey<'_>) -> Option<NodeVersion> {
    match key {
        HistoryKey::File => Some(NodeVersion {
            span: bytes.to_vec(),
            rev: revision(parsed, 0).number(),
        }),
        HistoryKey::Id(id) => {
            let ord = parsed
                .nodes
                .iter()
                .position(|node| node.id.as_deref() == Some(id))?;
            Some(NodeVersion {
                span: span_bytes(bytes, &parsed.nodes[ord]).to_vec(),
                rev: revision(parsed, ord).number(),
            })
        }
    }
}

/// What one commit did to a node, its version after (`after`) and in its
/// first parent (`before`, `None` when absent there or a root commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Judged {
    /// The span's bytes are the parent's: the commit is not listed.
    Same,
    /// Listed: `created` when the parent holds no such node (the scan
    /// stops); the revisions after and before (`None` when created).
    Changed {
        created: bool,
        rev: Option<u32>,
        rev_before: Option<u32>,
    },
    /// The commit's version holds no such node: the node's present life
    /// began later (or is not committed); the scan stops, nothing listed.
    Absent,
}

/// The judge of one commit (NH1): listed only when the node's span bytes
/// differ from its first parent's, or the parent holds none.
pub fn judge(after: Option<&NodeVersion>, before: Option<&NodeVersion>) -> Judged {
    let Some(after) = after else {
        return Judged::Absent;
    };
    match before {
        None => Judged::Changed {
            created: true,
            rev: after.rev,
            rev_before: None,
        },
        Some(before) if before.span == after.span => Judged::Same,
        Some(before) => Judged::Changed {
            created: false,
            rev: after.rev,
            rev_before: before.rev,
        },
    }
}

/// `text` cut at [`HISTORY_TEXT_MAX`] characters on a character boundary,
/// `…` added when cut.
pub fn history_text(text: &str) -> String {
    match text.char_indices().nth(HISTORY_TEXT_MAX) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}
