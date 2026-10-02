//! `[[check.rules]]` from TOML (task spec `spec-check-process`, ADR-0031):
//! the project's process rules under `[check]`, read by
//! [`super::config::check_config_from_toml`] and checked here. Names only
//! the rule table's own keys: every kind, key, label and path a rule holds
//! is the project's (`#universal`).

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use serde::Deserialize;
use specengine_model::Severity;
use toml::Spanned;

use super::config::{ConfigError, DocClass};

/// One `[[check.rules]]` entry, checked (task spec `spec-check-process`,
/// ADR-0031): selectors, ANDed (an empty list selects every document), each
/// list ORed; the `when` condition; the requirements; the severity of its
/// findings. It names only the project's vocabulary: the core knows none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRule {
    /// The document's kind (declared `kind:`, else its prefix's), each one
    /// an `[ids]` prefix declares.
    pub kinds: Vec<String>,
    /// The document's `class:`.
    pub classes: Vec<DocClass>,
    /// Globs over the root-relative path (the `[paths] exclude` grammar).
    pub paths: Vec<String>,
    /// Key → the strings one of which it must be written as, by key.
    pub when: Vec<(String, Vec<String>)>,
    /// Each written and not empty.
    pub keys: Vec<String>,
    /// Key → the strings each of its written strings must be one of, by
    /// key.
    pub values: Vec<(String, Vec<String>)>,
    /// Labelled parts, as written: each present and filled.
    pub parts: Vec<String>,
    /// The document's own text is filled.
    pub text: bool,
    /// Of every finding of the rule; default error.
    pub severity: Severity,
    /// The 1-based line of the entry.
    pub line: usize,
}

/// The `[[check.rules]]` entries, checked, in config order (task spec
/// `spec-check-process`, "Rules and edge cases"): no empty list or table;
/// each kind one an `[ids]` prefix declares (checked when `[ids]` reads:
/// its own errors are `ProjectConfig`'s), each class one of the four, each
/// glob a root-relative path (the `exclude` rules), each label with a
/// non-empty slug, no blank key; a `when` or `values` value a string or a
/// non-empty list of strings; `severity` "error" or "warning"; then at least
/// one selector and one requirement (`text = false` is none). An error is
/// at its value, else at the entry's header.
pub(super) fn rules_from(
    text: &str,
    raw: Vec<Spanned<RawRule>>,
) -> Result<Vec<CheckRule>, ConfigError> {
    let error_at = |span: Range<usize>, message: String| ConfigError {
        line: Some(super::text::line_of_str(text, span.start)),
        message,
    };
    let declared: Option<BTreeSet<String>> = crate::scheme_from_toml(text).ok().map(|scheme| {
        scheme
            .prefixes()
            .iter()
            .map(|spec| spec.kind.clone())
            .collect()
    });
    // A list's items; `what` names it in a message.
    let items = |what: &str, raw: Option<RawList>| match raw {
        None => Ok(Vec::new()),
        Some(raw) if raw.get_ref().is_empty() => {
            Err(error_at(raw.span(), format!("check rule {what} is empty")))
        }
        Some(raw) => Ok(raw.into_inner()),
    };
    // A `when` or `values` table: key → its strings, by key.
    let table = |what: &str, raw: Option<RawTable>| {
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        if raw.get_ref().is_empty() {
            return Err(error_at(raw.span(), format!("check rule {what} is empty")));
        }
        // Validated in source order, so of two bad entries the first
        // written is reported; the result is by key.
        let mut entries: Vec<(String, Spanned<toml::Value>)> =
            raw.into_inner().into_iter().collect();
        entries.sort_by_key(|(_, value)| value.span().start);
        let mut out: Vec<(String, Vec<String>)> = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let span = value.span();
            if key.trim().is_empty() {
                return Err(error_at(
                    span,
                    format!("check rule {what}: an empty key name"),
                ));
            }
            let strings = match value.into_inner() {
                toml::Value::String(one) => vec![one],
                toml::Value::Array(list) if list.is_empty() => {
                    return Err(error_at(
                        span,
                        format!("check rule {what} `{key}`: an empty list"),
                    ));
                }
                toml::Value::Array(list) => {
                    let mut strings = Vec::with_capacity(list.len());
                    for item in list {
                        match item {
                            toml::Value::String(one) => strings.push(one),
                            other => {
                                return Err(error_at(
                                    span,
                                    format!(
                                        "check rule {what} `{key}`: a list of strings, not one holding {}",
                                        toml_type(&other)
                                    ),
                                ));
                            }
                        }
                    }
                    strings
                }
                other => {
                    return Err(error_at(
                        span,
                        format!(
                            "check rule {what} `{key}`: a string or a list of strings, not {}",
                            toml_type(&other)
                        ),
                    ));
                }
            };
            out.push((key, strings));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    };
    let mut rules = Vec::with_capacity(raw.len());
    for entry in raw {
        let span = entry.span();
        let line = super::text::line_of_str(text, span.start);
        let entry = entry.into_inner();
        let mut kinds = Vec::new();
        for kind in items("`kinds`", entry.kinds)? {
            if let Some(declared) = &declared
                && !declared.contains(kind.get_ref())
            {
                return Err(error_at(
                    kind.span(),
                    format!(
                        "check rule `kinds`: `{}` is the kind of no `[ids]` prefix",
                        kind.get_ref()
                    ),
                ));
            }
            kinds.push(kind.into_inner());
        }
        let mut classes = Vec::new();
        for class in items("`classes`", entry.classes)? {
            match DocClass::parse(class.get_ref()) {
                Some(known) => classes.push(known),
                None => {
                    return Err(error_at(
                        class.span(),
                        format!(
                            "check rule `classes`: `{}` is none of canon | decision | spec | generated",
                            class.get_ref()
                        ),
                    ));
                }
            }
        }
        let mut paths = Vec::new();
        for glob in items("`paths`", entry.paths)? {
            let checked =
                crate::paths_toml::checked_path(glob.get_ref(), false).map_err(|problem| {
                    error_at(glob.span(), format!("check rule `paths`: {problem}"))
                })?;
            paths.push(checked);
        }
        let when = table("`when`", entry.when)?;
        let mut keys = Vec::new();
        for key in items("`keys`", entry.keys)? {
            if key.get_ref().trim().is_empty() {
                return Err(error_at(
                    key.span(),
                    "check rule `keys`: an empty key name".to_owned(),
                ));
            }
            keys.push(key.into_inner());
        }
        let values = table("`values`", entry.values)?;
        let mut parts = Vec::new();
        for label in items("`parts`", entry.parts)? {
            if crate::markdown::slug(label.get_ref()).is_empty() {
                return Err(error_at(
                    label.span(),
                    format!(
                        "check rule `parts`: label {:?} has an empty slug (no letter, digit, `-` or `_`)",
                        label.get_ref()
                    ),
                ));
            }
            parts.push(label.into_inner());
        }
        let own_text = entry.text.is_some_and(|value| *value.get_ref());
        let severity = match entry.severity {
            None => Severity::Error,
            Some(written) => match written.get_ref().as_str() {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                other => {
                    return Err(error_at(
                        written.span(),
                        format!("check rule `severity` `{other}` is not \"error\" or \"warning\""),
                    ));
                }
            },
        };
        if kinds.is_empty() && classes.is_empty() && paths.is_empty() {
            return Err(error_at(
                span,
                "check rule without a selector: give `kinds`, `classes` or `paths`".to_owned(),
            ));
        }
        if keys.is_empty() && values.is_empty() && parts.is_empty() && !own_text {
            return Err(error_at(
                span,
                "check rule without a requirement: give `keys`, `values`, `parts` or `text = true`"
                    .to_owned(),
            ));
        }
        rules.push(CheckRule {
            kinds,
            classes,
            paths,
            when,
            keys,
            values,
            parts,
            text: own_text,
            severity,
            line,
        });
    }
    Ok(rules)
}

/// The TOML type of `value` with its article, for messages.
fn toml_type(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "a string",
        toml::Value::Integer(_) => "an integer",
        toml::Value::Float(_) => "a float",
        toml::Value::Boolean(_) => "a boolean",
        toml::Value::Datetime(_) => "a datetime",
        toml::Value::Array(_) => "an array",
        toml::Value::Table(_) => "a table",
    }
}

/// A string list as written, each item with its span.
type RawList = Spanned<Vec<Spanned<String>>>;
/// A `when` or `values` table as written: key → its value with its span.
type RawTable = Spanned<BTreeMap<String, Spanned<toml::Value>>>;

/// One `[[check.rules]]` entry as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRule {
    #[serde(default)]
    kinds: Option<RawList>,
    #[serde(default)]
    classes: Option<RawList>,
    #[serde(default)]
    paths: Option<RawList>,
    #[serde(default)]
    when: Option<RawTable>,
    #[serde(default)]
    keys: Option<RawList>,
    #[serde(default)]
    values: Option<RawTable>,
    #[serde(default)]
    parts: Option<RawList>,
    #[serde(default)]
    text: Option<Spanned<bool>>,
    #[serde(default)]
    severity: Option<Spanned<String>>,
}
