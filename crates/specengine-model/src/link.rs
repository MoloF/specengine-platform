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

/// The weak link of an inline citation and of `refs:` / `adrs:`.
pub const MENTIONS: &str = "mentions";

/// `true` for a member of [`LINK_TYPES`].
pub fn is_link_type(name: &str) -> bool {
    LINK_TYPES.contains(&name)
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
    Reference(Reference),
    /// `canon: path[#anchor]`.
    Path(PathTarget),
}

/// `src --type--> dst`. Unresolved: `dst` is what the text says.
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
