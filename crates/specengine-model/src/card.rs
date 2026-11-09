//! A card (`docs/features/card-store.md` "Store", ADR-0056): the lighter
//! record of the work of any trade, a row of the project's queue beside its
//! tasks, never a file or a commit.
//!
//! Types only: the store keeps a card's rows, core's `card` and `card_toml`
//! judge them against the project's flows, the CLI prints them. No column,
//! flow or discipline is named here: those words are the project's and
//! travel as data.

use serde::{Deserialize, Serialize};

/// The most `T-` tasks a card links
/// (`docs/canon/card-tasks.md` "The rule").
pub const CARD_TASKS_MAX: usize = 32;

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

/// What an accept says of the result against its brief.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardVerdict {
    /// As briefed; a row stored without a verdict reads this.
    #[default]
    Briefed,
    /// It departs from the brief: a discrepancy says how.
    Departs,
}

impl CardVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Briefed => "briefed",
            Self::Departs => "departs",
        }
    }
}

/// `acceptance`: the reviewer's act.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardAcceptance {
    pub by: String,
    pub at: String,
    pub note: Option<String>,
    #[serde(default)]
    pub verdict: CardVerdict,
    /// The direct targets a departing accept names.
    #[serde(default)]
    pub departs: Vec<String>,
    /// The discrepancy a departing accept made.
    #[serde(default)]
    pub discrepancy: Option<String>,
}

/// `cancellation`: who took the card out of its column, and why.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardCancellation {
    pub by: String,
    pub at: String,
    pub reason: String,
}

/// One direct target as a card last saw it: the entry a meaning change is
/// compared with. The state of a card against it is computed per read and
/// never stored here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardFrozen {
    /// The target as `targets` holds it.
    pub id: String,
    pub rev: Option<u32>,
    /// The target's own text and links.
    pub norm_hash: Option<String>,
    /// The target and the ID sections nested in it, the hash a mark compares;
    /// `None` when none could be frozen, and on an entry stored before there
    /// was one, whose `norm_hash` stands in.
    #[serde(default)]
    pub within_hash: Option<String>,
    /// Hashes vouched for since (an editorial apply's), of the kind this
    /// entry compares, the last 16.
    #[serde(default)]
    pub also: Vec<String>,
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
    /// Each direct target as frozen, in `targets` order, then any target
    /// since dropped, inert until it returns; `None` on a card that never
    /// froze one.
    pub snapshot: Option<Vec<CardFrozen>>,
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
    /// The `T-` tasks it links, in the order written; none on a row stored
    /// before the link. The link decides nothing: no rule reads it
    /// (`docs/canon/card-tasks.md` "The rule").
    pub tasks: Vec<String>,
}

impl Card {
    /// Accepted or cancelled: every write of its own is refused; a person's
    /// clear, and the accept of its correction, still reach an accepted card.
    pub const fn is_closed(&self) -> bool {
        self.acceptance.is_some() || self.cancellation.is_some()
    }
}
