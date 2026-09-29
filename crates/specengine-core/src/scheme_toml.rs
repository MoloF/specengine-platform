//! `IdScheme::from_toml`: the `[ids]` table of `specengine.toml`, and only
//! that table. The scheme loads whole or not at all; every error names the
//! line (`file:line: message` through [`SchemeError::at`]).

use std::collections::BTreeMap;
use std::ops::Range;

use serde::Deserialize;
use specengine_model::{IdScheme, IdScope, PrefixSpec, SchemeError, SchemeField, Shape};
use toml::Spanned;

/// Reading an [`IdScheme`] from TOML text.
pub trait IdSchemeToml: Sized {
    /// Reads the `[ids]` table; other tables are ignored. No `[ids]` is an
    /// empty scheme.
    fn from_toml(text: &str) -> Result<Self, SchemeError>;
}

impl IdSchemeToml for IdScheme {
    fn from_toml(text: &str) -> Result<Self, SchemeError> {
        scheme_from_toml(text)
    }
}

/// [`IdSchemeToml::from_toml`] as a function.
pub fn scheme_from_toml(text: &str) -> Result<IdScheme, SchemeError> {
    let error_at = |span: Option<Range<usize>>, message: String| SchemeError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let raw: RawFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(ids) = raw.ids else {
        return IdScheme::new(Vec::new()).map_err(|problem| error_at(None, problem.message));
    };

    // Validated in source order, so of two bad entries the first written is
    // reported; `IdScheme::new` sorts by prefix, so the scheme and its
    // fingerprint do not depend on this order.
    let mut ids: Vec<(String, Spanned<RawPrefix>)> = ids.into_iter().collect();
    ids.sort_by_key(|(_, raw)| raw.span().start);
    let mut entries = Vec::with_capacity(ids.len());
    let mut spans = Vec::with_capacity(ids.len());
    for (prefix, raw) in ids {
        let entry_span = raw.span();
        let raw = raw.into_inner();
        let shape = match &raw.shape {
            None => Shape::Number,
            Some(shape) => match shape.get_ref().as_str() {
                "number" => Shape::Number,
                "name" => Shape::Name,
                other => {
                    return Err(error_at(
                        Some(shape.span()),
                        format!("`{prefix}`: shape `{other}` is neither \"number\" nor \"name\""),
                    ));
                }
            },
        };
        let width = match &raw.width {
            None => None,
            Some(width) => match u32::try_from(*width.get_ref()) {
                Ok(value) => Some(value),
                Err(_) => {
                    return Err(error_at(
                        Some(width.span()),
                        format!("`{prefix}`: `width` must be at least 1"),
                    ));
                }
            },
        };
        let scope = match &raw.scope {
            None => IdScope::Project,
            Some(scope) => match scope.get_ref().as_str() {
                "project" => IdScope::Project,
                "feature" => IdScope::Feature,
                other => {
                    return Err(error_at(
                        Some(scope.span()),
                        format!(
                            "`{prefix}`: scope `{other}` is neither \"project\" nor \"feature\""
                        ),
                    ));
                }
            },
        };
        let aliases = raw.aliases_from.unwrap_or_default();
        spans.push(EntrySpans {
            entry: entry_span,
            width: raw.width.as_ref().map(Spanned::span),
            aliases: aliases.iter().map(Spanned::span).collect(),
        });
        entries.push(PrefixSpec {
            prefix,
            kind: raw.kind.into_inner(),
            shape,
            width,
            aliases_from: aliases.into_iter().map(Spanned::into_inner).collect(),
            immutable_text: raw.immutable_text.unwrap_or(false),
            scope,
        });
    }
    IdScheme::new(entries).map_err(|problem| {
        let spans = &spans[problem.entry];
        let span = match problem.field {
            SchemeField::Prefix => Some(spans.entry.clone()),
            SchemeField::Width => spans.width.clone().or_else(|| Some(spans.entry.clone())),
            SchemeField::Alias(index) => spans
                .aliases
                .get(index)
                .cloned()
                .or_else(|| Some(spans.entry.clone())),
        };
        error_at(span, problem.message)
    })
}

struct EntrySpans {
    entry: Range<usize>,
    width: Option<Range<usize>>,
    aliases: Vec<Range<usize>>,
}

/// The file: only `[ids]` is read; every other table is ignored.
#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    ids: Option<BTreeMap<String, Spanned<RawPrefix>>>,
}

/// One `[ids]` entry; `script` and any other key are errors (ADR-0009:
/// every prefix is Latin).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPrefix {
    kind: Spanned<String>,
    #[serde(default)]
    shape: Option<Spanned<String>>,
    #[serde(default)]
    width: Option<Spanned<i64>>,
    #[serde(default)]
    aliases_from: Option<Vec<Spanned<String>>>,
    #[serde(default)]
    immutable_text: Option<bool>,
    #[serde(default)]
    scope: Option<Spanned<String>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
