//! Core's typed front-matter keys, as far as the layout writes values under
//! them (`docs/features/import-layout.md` AC-05): a carried value of a
//! typed key is written in core's type when its text parses as it — an
//! integer plain, a list-typed key a flow list (a reference-list key's
//! value split at its commas) — else double-quoted (core then reports
//! `frontmatter-type`, a source cause). The key names live in serde-named
//! variants, not string literals, so that the genre test's literal scan
//! keeps reading only corpus conventions. The table copies
//! `specengine_core::TYPED_KEYS` (this crate does not depend on core); an
//! eval test pins [`typed_keys`] to it.

use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};

use super::yaml;
use crate::config::word;

/// The type core reads a typed key's value as (core's `KeyType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreType {
    /// A string (dates included: core keeps them as text).
    Text,
    /// A string holding exactly one reference.
    Reference,
    /// An integer.
    Integer,
    /// A list of strings; a bare string is mistyped.
    List,
    /// A list of strings, each exactly one reference; a bare string is
    /// mistyped.
    ReferenceList,
    /// A mapping; no scalar is one.
    Mapping,
}

/// Core's typed keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum TypedKey {
    Id,
    Kind,
    Class,
    Title,
    Status,
    Owner,
    Reviewed,
    Date,
    Shipped,
    Ref,
    To,
    Severity,
    Generator,
    Source,
    Acceptance,
    Tier,
    Rev,
    Scope,
    Aliases,
    Parent,
    WorkingAnswer,
    Canon,
    Supersedes,
    Adrs,
    Refs,
    Links,
    RaisedBy,
}

/// Every [`TypedKey`], in core's table order.
const TYPED: [TypedKey; 27] = [
    TypedKey::Id,
    TypedKey::Kind,
    TypedKey::Class,
    TypedKey::Title,
    TypedKey::Status,
    TypedKey::Owner,
    TypedKey::Reviewed,
    TypedKey::Date,
    TypedKey::Shipped,
    TypedKey::Ref,
    TypedKey::To,
    TypedKey::Severity,
    TypedKey::Generator,
    TypedKey::Source,
    TypedKey::Acceptance,
    TypedKey::Tier,
    TypedKey::Rev,
    TypedKey::Scope,
    TypedKey::Aliases,
    TypedKey::Parent,
    TypedKey::WorkingAnswer,
    TypedKey::Canon,
    TypedKey::Supersedes,
    TypedKey::Adrs,
    TypedKey::Refs,
    TypedKey::Links,
    TypedKey::RaisedBy,
];

impl TypedKey {
    fn core_type(self) -> CoreType {
        match self {
            Self::Id
            | Self::Kind
            | Self::Class
            | Self::Title
            | Self::Status
            | Self::Owner
            | Self::Reviewed
            | Self::Date
            | Self::Shipped
            | Self::Ref
            | Self::To
            | Self::Severity
            | Self::Generator
            | Self::Source
            | Self::Acceptance => CoreType::Text,
            Self::Tier | Self::Rev => CoreType::Integer,
            Self::Scope | Self::Aliases => CoreType::List,
            Self::Parent | Self::WorkingAnswer | Self::Canon => CoreType::Reference,
            Self::Supersedes | Self::Adrs | Self::Refs => CoreType::ReferenceList,
            Self::Links | Self::RaisedBy => CoreType::Mapping,
        }
    }

    /// The typed key `text` names, if any.
    fn of(text: &str) -> Option<Self> {
        let deserializer: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            text.into_deserializer();
        Self::deserialize(deserializer).ok()
    }
}

/// The type core gives `key`; `None` for a key core keeps untyped.
pub fn core_type(key: &str) -> Option<CoreType> {
    TypedKey::of(key).map(TypedKey::core_type)
}

/// The layout's copy of core's typed-key table: each key's name and type,
/// in core's order (`docs/features/import-layout.md` AC-05; pinned equal to
/// `specengine_core::TYPED_KEYS` by an eval test).
pub fn typed_keys() -> Vec<(String, CoreType)> {
    TYPED
        .iter()
        .map(|key| (word(key), key.core_type()))
        .collect()
}

/// An integer as YAML reads it back with the same text: `0` or an optional
/// `-` and digits without a leading zero, within 64 bits. Other integer
/// spellings (`+1`, `007`, `1_000`, `0x1F`) are quoted: their read-back
/// text would differ.
fn canonical_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty()
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
        && text != "-0"
        && text.parse::<i64>().is_ok()
}

/// Whether `text`, carried under `key`, reads back in core's type when
/// [`value_text`] writes it: any text under a string, reference or list key
/// or an untyped one, a canonical integer under an integer key, never a
/// scalar under a mapping key.
pub fn parses_as(key: &str, text: &str) -> bool {
    match core_type(key) {
        Some(CoreType::Integer) => canonical_integer(text),
        Some(CoreType::Mapping) => false,
        _ => true,
    }
}

/// The items a reference-list key carries of one cell or scalar
/// (`docs/features/import-layout.md` AC-05): the text split at every
/// comma, each item trimmed, empty items dropped (`XQ-42,`, `a,, b`).
fn reference_items(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The YAML text of a value the layout writes under `key`: a canonical
/// integer under an integer key plain, under a list key a one-item flow
/// list, under a reference-list key a flow list of its comma-separated
/// items, anything else double-quoted.
pub(super) fn value_text(key: &str, text: &str) -> String {
    match core_type(key) {
        Some(CoreType::Integer) if canonical_integer(text) => text.to_owned(),
        Some(CoreType::List) => yaml::sequence(&[text.to_owned()]),
        Some(CoreType::ReferenceList) => yaml::sequence(&reference_items(text)),
        _ => yaml::quoted(text),
    }
}

/// A header entry's single-line scalar (`written`, quotes included;
/// `value`, the lenient reader's text) carried as written under the typed
/// key `key`, retyped where core would not read it in its type: a plain
/// scalar under a string or reference key quoted when YAML may read it
/// otherwise, a quoted canonical integer under an integer key unquoted, a
/// scalar under a list key wrapped as a one-item flow list, under a
/// reference-list key split at its commas into a flow list (a quoted
/// scalar holding a YAML escape — `\` inside double quotes, `''` inside
/// single quotes — stays one item, as written: the lenient reader keeps
/// escapes raw, so its split would change the text;
/// `docs/features/import-layout.md` AC-05). `None` when it stays.
pub(super) fn retyped_scalar(key: &str, written: &str, value: &str) -> Option<String> {
    let quoted_written = written.starts_with(['"', '\'']);
    let flow = written.starts_with(['[', '{']);
    let one_item = || {
        if quoted_written {
            format!("[{written}]")
        } else {
            yaml::sequence(&[value.to_owned()])
        }
    };
    match core_type(key)? {
        CoreType::Text | CoreType::Reference
            if !quoted_written && !yaml::reads_as_string(written) =>
        {
            Some(yaml::quoted(value))
        }
        CoreType::Integer if canonical_integer(value) && written != value => Some(value.to_owned()),
        CoreType::List if !flow => Some(one_item()),
        CoreType::ReferenceList if !flow => {
            let raw_escapes = match written.chars().next() {
                Some('"') => value.contains('\\'),
                Some('\'') => value.contains("''"),
                _ => false,
            };
            Some(if raw_escapes {
                one_item()
            } else {
                yaml::sequence(&reference_items(value))
            })
        }
        _ => None,
    }
}
