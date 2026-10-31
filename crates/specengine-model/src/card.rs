//! A card (`docs/features/card-store.md` "Store", ADR-0056): the lighter
//! record of the work of any trade, a row of the project's queue beside its
//! tasks, never a file or a commit.
//!
//! Types only: the store keeps a card's rows, core's `card` and `card_toml`
//! judge them against the project's flows, the CLI prints them. No column,
//! flow or discipline is named here: those words are the project's and
//! travel as data.

use serde::{Deserialize, Serialize};

/// What an evidence entry is: a reference, never a binary, and nothing is
/// fetched.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceKind {
    /// `http://` or `https://`.
    Url,
    /// `<path>@<hex blob>`.
    Path,
    /// A build's name.
    Build,
    /// Free text on one line.
    Note,
}

impl EvidenceKind {
    pub const ALL: [Self; 4] = [Self::Url, Self::Path, Self::Build, Self::Note];

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Url => "url",
            Self::Path => "path",
            Self::Build => "build",
            Self::Note => "note",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

/// One entry of `evidence`, oldest first: `{kind, value, by, at}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardEvidence {
    pub kind: EvidenceKind,
    pub value: String,
    /// The name it was given under.
    pub by: String,
    pub at: String,
}

/// `acceptance`: the reviewer's act.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardAcceptance {
    pub by: String,
    pub at: String,
    pub note: Option<String>,
}

/// `cancellation`: who took the card out of its column, and why.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardCancellation {
    pub by: String,
    pub at: String,
    pub reason: String,
}

/// A stored card.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Card {
    /// `C-` and 4 or more digits.
    pub id: String,
    pub project: String,
    pub flow: String,
    /// The key `column` of every document; `col` in the table.
    pub column: String,
    /// From 1, dense per (`flow`, `column`); 0 once cancelled.
    pub position: u64,
    pub title: String,
    pub goal: Option<String>,
    pub discipline: Option<String>,
    /// Canonical IDs or an id-less document's path, each once; none on a
    /// manual card.
    pub targets: Vec<String>,
    /// Held for a later slice; `None` on every card this build writes.
    pub snapshot: Option<String>,
    pub criteria: Vec<String>,
    pub assignee: Option<String>,
    pub reviewer: Option<String>,
    pub estimate: Option<String>,
    /// As given; nothing is derived from it.
    pub due: Option<String>,
    pub parent: Option<String>,
    pub corrects: Option<String>,
    /// Oldest first.
    pub evidence: Vec<CardEvidence>,
    pub acceptance: Option<CardAcceptance>,
    pub cancellation: Option<CardCancellation>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    /// 1 when made, one more on every change of this card; a neighbour
    /// shifted by another card's move takes none.
    pub revision: u64,
}

impl Card {
    /// Accepted or cancelled: every write on it is refused.
    pub const fn is_closed(&self) -> bool {
        self.acceptance.is_some() || self.cancellation.is_some()
    }
}
