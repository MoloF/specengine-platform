//! The `[ids]` scheme of `specengine.toml`: which prefixes are IDs, their
//! kind and shape, and the legacy prefixes that alias them.
//!
//! The scheme is data of the project (ADR-0008): no prefix is known here.
//! Reading it from TOML is `specengine-core`'s job; this module holds the
//! types, the validation that needs no source text and the lookups.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// How the body of an ID after `PREFIX-` is formed.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// `digit+`; `width` is the zero-padded width `spec new` issues.
    #[default]
    Number,
    /// `alnum+ ("-" alnum+)*`, greedy; `alnum = [A-Za-z0-9]`.
    Name,
}

/// Where an ID is unique; carried, not acted on in this increment.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum IdScope {
    #[default]
    Project,
    /// Unique within a feature; cited from outside as `slug/ID`.
    Feature,
}

/// One entry of `[ids]`: `PREFIX = { kind = "...", ... }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixSpec {
    /// `[A-Z][A-Z0-9]*`, case-sensitive.
    pub prefix: String,
    /// Node kind of IDs with this prefix; a string, not validated.
    pub kind: String,
    #[serde(default)]
    pub shape: Shape,
    /// Digits `spec new` issues; required for `number`, forbidden for `name`;
    /// never checked on recognition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Legacy prefixes in any script (`\p{L}[\p{L}\p{N}]*`) that cite this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases_from: Vec<String>,
    /// Carried, not acted on in this increment.
    #[serde(default)]
    pub immutable_text: bool,
    /// Carried, not acted on in this increment.
    #[serde(default)]
    pub scope: IdScope,
}

impl PrefixSpec {
    /// A `number` prefix of the given width.
    pub fn number(prefix: impl Into<String>, kind: impl Into<String>, width: u32) -> Self {
        Self {
            prefix: prefix.into(),
            kind: kind.into(),
            shape: Shape::Number,
            width: Some(width),
            aliases_from: Vec::new(),
            immutable_text: false,
            scope: IdScope::Project,
        }
    }

    /// A `name` prefix.
    pub fn name(prefix: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            kind: kind.into(),
            shape: Shape::Name,
            width: None,
            aliases_from: Vec::new(),
            immutable_text: false,
            scope: IdScope::Project,
        }
    }

    /// The same entry with `aliases_from` replaced.
    pub fn with_aliases<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases_from = aliases.into_iter().map(Into::into).collect();
        self
    }
}

/// The part of an entry a [`SchemeProblem`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemeField {
    Prefix,
    Width,
    /// The `n`-th entry of `aliases_from`.
    Alias(usize),
}

/// Why a list of entries is not a scheme; `entry` indexes the input list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemeProblem {
    pub entry: usize,
    pub field: SchemeField,
    pub message: String,
}

impl fmt::Display for SchemeProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SchemeProblem {}

/// A scheme that could not be loaded; it loads whole or not at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemeError {
    /// 1-based line in the TOML text, when known.
    pub line: Option<usize>,
    pub message: String,
}

impl SchemeError {
    /// `file:line: message`, the form every scheme error is reported in.
    pub fn at(&self, file: &str) -> String {
        match self.line {
            Some(line) => format!("{file}:{line}: {}", self.message),
            None => format!("{file}: {}", self.message),
        }
    }
}

impl fmt::Display for SchemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for SchemeError {}

/// The validated `[ids]` scheme: entries sorted by prefix, plus lookups.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdScheme {
    prefixes: Vec<PrefixSpec>,
    by_prefix: BTreeMap<String, usize>,
    by_alias: BTreeMap<String, usize>,
    /// Chars of the longest prefix or alias: a longer letter-digit run is no ID.
    max_run_chars: usize,
}

impl IdScheme {
    /// Validates the entries: prefix grammar, `width` against `shape`, alias
    /// grammar, alias uniqueness, no alias equal to a prefix, no repeated
    /// prefix. The first problem found is returned.
    pub fn new(entries: Vec<PrefixSpec>) -> Result<Self, SchemeProblem> {
        let problem = |entry: usize, field: SchemeField, message: String| SchemeProblem {
            entry,
            field,
            message,
        };
        let mut seen_prefixes: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, spec) in entries.iter().enumerate() {
            if !is_prefix(&spec.prefix) {
                return Err(problem(
                    index,
                    SchemeField::Prefix,
                    format!(
                        "ID prefix `{}` is not `[A-Z][A-Z0-9]*` (IDs are Latin, ADR-0009)",
                        spec.prefix
                    ),
                ));
            }
            if seen_prefixes.insert(&spec.prefix, index).is_some() {
                return Err(problem(
                    index,
                    SchemeField::Prefix,
                    format!("ID prefix `{}` is configured twice", spec.prefix),
                ));
            }
            match (spec.shape, spec.width) {
                (Shape::Number, None) => {
                    return Err(problem(
                        index,
                        SchemeField::Prefix,
                        format!(
                            "ID prefix `{}` has shape \"number\" and no `width`",
                            spec.prefix
                        ),
                    ));
                }
                (Shape::Number, Some(0)) => {
                    return Err(problem(
                        index,
                        SchemeField::Width,
                        format!("ID prefix `{}`: `width` must be at least 1", spec.prefix),
                    ));
                }
                (Shape::Name, Some(_)) => {
                    return Err(problem(
                        index,
                        SchemeField::Width,
                        format!(
                            "ID prefix `{}` has shape \"name\", which takes no `width`",
                            spec.prefix
                        ),
                    ));
                }
                _ => {}
            }
        }
        let mut by_alias: BTreeMap<String, usize> = BTreeMap::new();
        for (index, spec) in entries.iter().enumerate() {
            for (position, alias) in spec.aliases_from.iter().enumerate() {
                let at = SchemeField::Alias(position);
                if !is_alias(alias) {
                    return Err(problem(
                        index,
                        at,
                        format!(
                            "alias `{alias}` of `{}` is not a letter followed by letters or digits",
                            spec.prefix
                        ),
                    ));
                }
                if seen_prefixes.contains_key(alias.as_str()) {
                    return Err(problem(
                        index,
                        at,
                        format!(
                            "alias `{alias}` of `{}` is itself a configured prefix",
                            spec.prefix
                        ),
                    ));
                }
                if let Some(&other) = by_alias.get(alias) {
                    return Err(problem(
                        index,
                        at,
                        format!(
                            "alias `{alias}` of `{}` is already an alias of `{}`",
                            spec.prefix, entries[other].prefix
                        ),
                    ));
                }
                by_alias.insert(alias.clone(), index);
            }
        }

        // Sorted by prefix: the scheme is the same whatever the input order.
        let mut order: Vec<usize> = (0..entries.len()).collect();
        order.sort_by(|&a, &b| entries[a].prefix.cmp(&entries[b].prefix));
        let mut new_index = vec![0; entries.len()];
        for (sorted, &original) in order.iter().enumerate() {
            new_index[original] = sorted;
        }
        let mut slots: Vec<Option<PrefixSpec>> = entries.into_iter().map(Some).collect();
        let prefixes: Vec<PrefixSpec> = order
            .iter()
            .filter_map(|&original| slots[original].take())
            .collect();
        let by_prefix = prefixes
            .iter()
            .enumerate()
            .map(|(index, spec)| (spec.prefix.clone(), index))
            .collect();
        let by_alias = by_alias
            .into_iter()
            .map(|(alias, original)| (alias, new_index[original]))
            .collect();
        let max_run_chars = prefixes
            .iter()
            .flat_map(|spec| std::iter::once(&spec.prefix).chain(&spec.aliases_from))
            .map(|text| text.chars().count())
            .max()
            .unwrap_or(0);
        Ok(Self {
            prefixes,
            by_prefix,
            by_alias,
            max_run_chars,
        })
    }

    /// Entries sorted by prefix.
    pub fn prefixes(&self) -> &[PrefixSpec] {
        &self.prefixes
    }

    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }

    /// The entry of a configured (Latin) prefix.
    pub fn prefix(&self, prefix: &str) -> Option<&PrefixSpec> {
        self.by_prefix
            .get(prefix)
            .map(|&index| &self.prefixes[index])
    }

    /// The entry a legacy prefix of `aliases_from` stands for.
    pub fn alias(&self, alias: &str) -> Option<&PrefixSpec> {
        self.by_alias.get(alias).map(|&index| &self.prefixes[index])
    }

    /// The kind of a Latin ID by its prefix (the text before the first `-`).
    pub fn kind_of_id(&self, id: &str) -> Option<&str> {
        let (prefix, _) = id.split_once('-')?;
        self.prefix(prefix).map(|spec| spec.kind.as_str())
    }

    /// Chars of the longest configured prefix or alias.
    pub fn max_run_chars(&self) -> usize {
        self.max_run_chars
    }
}

/// `[A-Z][A-Z0-9]*`.
pub fn is_prefix(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// `\p{L}[\p{L}\p{N}]*`.
pub fn is_alias(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(char::is_alphabetic) && chars.all(char::is_alphanumeric)
}
