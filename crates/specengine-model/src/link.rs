//! Links between nodes: declared in front-matter or mentioned in text.

use serde::{Deserialize, Serialize};

use crate::reference::{PathTarget, Reference};
use crate::span::Span;

/// The closed, shared set of link types a `links:` map may declare.
pub const LINK_TYPES: [&str; 12] = [
    "derived_from",
    "depends_on",
    "constrains",
    "supersedes",
    "revises",
    "amends",
    "answers",
    "working_answer",
    "uses_term",
    "canon",
    "verifies",
    "adopts",
];

/// The weak link of an inline citation, of a local Markdown file link and
/// of `refs:` / `adrs:`.
pub const MENTIONS: &str = "mentions";

/// `true` for a member of [`LINK_TYPES`].
pub fn is_link_type(name: &str) -> bool {
    LINK_TYPES.contains(&name)
}

/// Which way a walk follows a link from the node it stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// From the link's source to its target.
    Out,
    /// From the link's target back to its source.
    In,
}

impl Direction {
    /// `out` or `in`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Out => "out",
            Self::In => "in",
        }
    }
}

/// The link types `spec graph --impact` follows, and which way: what an
/// edit of a node reaches. One table for every project; any other type
/// (`mentions`, `supersedes`, `revises`, `amends`, `answers`,
/// `working_answer`, `canon`, `adopts`, an unknown declared type) is not
/// followed.
pub const IMPACT_LINK_TYPES: [(&str, Direction); 5] = [
    ("depends_on", Direction::In),
    ("derived_from", Direction::In),
    ("verifies", Direction::In),
    ("uses_term", Direction::In),
    ("constrains", Direction::Out),
];

/// `mentions` is the one weak link type; every declared type, an unknown
/// one included, is strong.
pub fn is_weak_link(link_type: &str) -> bool {
    link_type == MENTIONS
}

/// `spec graph`'s default: every strong type outgoing, `mentions` not
/// followed.
pub fn graph_direction(link_type: &str) -> Option<Direction> {
    (!is_weak_link(link_type)).then_some(Direction::Out)
}

/// `spec graph --impact`: the direction [`IMPACT_LINK_TYPES`] gives
/// `link_type`, if any.
pub fn impact_direction(link_type: &str) -> Option<Direction> {
    IMPACT_LINK_TYPES
        .iter()
        .find(|(name, _)| *name == link_type)
        .map(|&(_, direction)| direction)
}

/// Where a link was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkOrigin {
    Frontmatter,
    Inline,
}

/// What a link points at.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LinkTarget {
    /// An ID reference: declared, or an inline mention.
    Reference(Reference),
    /// `canon: path[#anchor]` (origin `frontmatter`), or the local
    /// destination of a Markdown inline link or reference definition
    /// (`mentions`, origin `inline`): the path relative to the linking file
    /// (`/`-led: to the root; `""` for `#anchor` alone), never resolved.
    Path(PathTarget),
}

/// `src --type--> dst`. Unresolved: `dst` is what the text says: an ID
/// reference, a `canon:` path, or a Markdown link destination.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Link {
    /// The source node's ID: the document's, or for an inline mention the
    /// innermost ID section around it; for `status: superseded-by X`, `X`.
    /// Absent when the file has no ID to speak for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// Where `src` is written, when it is not the node itself
    /// (`status: superseded-by X`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_span: Option<Span>,
    /// A member of [`LINK_TYPES`], [`MENTIONS`], or an unknown declared type
    /// (kept, with an `unknown-link-type` warning).
    #[serde(rename = "type")]
    pub link_type: String,
    pub origin: LinkOrigin,
    pub dst: LinkTarget,
}
