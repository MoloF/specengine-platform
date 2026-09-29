//! Nodes of one file: the document, then its `{#ID}` sections.

use serde::{Deserialize, Serialize};

use crate::reference::{CanonTarget, Reference};
use crate::script::IdScript;
use crate::span::Span;
use crate::value::{FmValue, OrderedMap};

/// The document (`nodes[0]`) or a section. Covers 05 §3.3 `nodes` except
/// `project`, `worktree`, `norm_hash`; the file is the `path` of the parsed
/// file, a section's anchor is its `id`. Absent optional fields are omitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// The Latin ID; absent for a document without a valid `id:`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Script class of the ID as written, when it is not `latin` (the ID was
    /// normalised from look-alikes; a `homoglyph` diagnostic carries the fix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<IdScript>,
    /// Document: declared `kind`, else the kind of the ID's prefix. Section:
    /// the kind of the ID's prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Document: `title:`, else the text of the first H1. Section: the
    /// heading's text without its attribute block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Document: the first paragraph after the first H1 (no H1: before the
    /// first heading).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Span>,
    /// Section: the heading level, 1–6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Section: the heading line(s), line ending excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<Span>,
    /// Section: from the line after the heading to the section's end;
    /// disjoint from `heading`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Span>,
    /// Section: `key=value` (value) and bare `key` (null) attributes of the
    /// heading's attribute block, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attrs: Vec<(String, Option<String>)>,
    /// Section: `.class` entries of the attribute block.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// Document: front-matter `rev`. Section: the `rev=N` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<u32>,
    /// Document: `parent:` (containment, not a link). Section: the nearest
    /// enclosing ID section, else the document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<ParentRef>,
    /// Document: the whole file. Section: its heading to the next heading of
    /// the same or a higher level, trailing whitespace excluded.
    pub span: Span,
    /// The token estimate of `span` (`specengine-core`'s `tokens_est`).
    pub tokens_est: u32,
    /// Document: typed front-matter keys other than `id`, `kind`, `title`,
    /// `rev`, `parent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Fields>,
    /// Document: untyped keys and the raw values of mistyped ones, in
    /// source order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<ExtraEntry>>,
}

/// The node's parent: an ID, with the span of `parent:`'s value for a document.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ParentRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// One untyped front-matter key with its value as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtraEntry {
    pub key: String,
    pub value: FmValue,
}

/// Typed front-matter keys (docs/features/spec-parser.md, "Front-matter keys
/// and links"). Absent keys are omitted.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Fields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shipped: Option<String>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
    /// Legacy IDs of this node; not lexed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_answer: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canon: Option<CanonTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<Vec<Reference>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adrs: Option<Vec<Reference>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refs: Option<Vec<Reference>>,
    /// Link type → references, in source order; unknown types kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub links: Option<OrderedMap<Vec<Reference>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raised_by: Option<OrderedMap<FmValue>>,
}

/// A `{#…}` heading anchor that is not an ID of the scheme.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Anchor {
    /// The text after `#`.
    pub name: String,
    pub level: u8,
    /// The heading line(s), line ending excluded.
    pub heading: Span,
}
