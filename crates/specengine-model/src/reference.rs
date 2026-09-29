//! A reference to a node as it was recognised in text or front-matter.

use serde::{Deserialize, Serialize};

use crate::script::IdScript;
use crate::span::Span;

/// How a reference was written.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum RefForm {
    /// A bare ID with its optional qualifiers, section and revision.
    #[default]
    Bare,
    /// `[[reference]]` or `[[reference|label]]`.
    Wiki,
}

impl RefForm {
    pub fn is_bare(&self) -> bool {
        matches!(self, Self::Bare)
    }
}

/// `[project ":"] [scope "/"] id ["#" id] ["@" rev]`, or its wiki form.
///
/// Nothing here is resolved: existence, aliases, `slug/` and `project:` are
/// `spec check`'s job.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Reference {
    /// The Latin ID after look-alike normalisation; for an alias, the prefix
    /// as written and the body normalised (`alias_of` names the prefix it
    /// stands for).
    pub id: String,
    /// The configured prefix whose `aliases_from` matched the written prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
    /// Script class of the ID as written.
    pub script: IdScript,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// The `#ID` part: a section inside the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// The `@N` part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<u32>,
    #[serde(default, skip_serializing_if = "RefForm::is_bare")]
    pub form: RefForm,
    /// The `|label` part of the wiki form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The whole occurrence as written: qualifiers, section and revision
    /// included, `[[`…`]]` included for the wiki form. Absent when a
    /// front-matter scalar does not lie verbatim in the source (escapes,
    /// folding, aliases).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// The value of `canon:`: a reference, else `path[#anchor]`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CanonTarget {
    Reference(Reference),
    Path(PathTarget),
}

/// `path[#anchor]`: a file of the corpus and an optional heading anchor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PathTarget {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}
