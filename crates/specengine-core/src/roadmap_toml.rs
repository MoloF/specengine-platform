//! `[roadmap]` of `specengine.toml` (canon `spec-roadmap`, "Config"): where
//! plan items are found and how the plan is ordered, read by `spec roadmap`
//! and, only when the index entry registers a `roadmap` view, by the check
//! tables ([`crate::check::CheckConfig::from_toml`]): a bad table then fails
//! every command that loads them, as a bad `[[generators]]` entry does.
//! [`crate::ProjectConfig::from_toml`] only name-checks `roadmap`, so
//! without a view a bad table fails `spec roadmap` alone.
//!
//! - `prefix` (required): an `[ids]` prefix of shape `number`, not of
//!   `scope = "feature"`; a legacy alias is an error naming the canonical
//!   prefix. An item is a document whose `id:` has it.
//! - `dir` (required): where new items are created, a root-relative
//!   directory under the `[paths]` path rules.
//! - `releases`, `priorities` (required): non-empty lists of unique slugs
//!   ([`specengine_model::grammar::is_slug`], at most
//!   [`crate::MAX_SLUG_BYTES`] bytes); the path's
//!   order, the priority scale highest first.
//! - `mvp_cut` (optional): one of `releases`.
//! - `unit` (optional): a label printed after every sum, never converted:
//!   not blank, at most [`MAX_UNIT_BYTES`] bytes, no control character.
//! - **Another key**: the TOML reader's message (`unknown field`, the six).
//!
//! Every value is judged and the first error by line is reported, as a
//! [`ProjectError`] at that line (`file:line` through [`ProjectError::at`]).
//! Pure: the text in, the table out; it names no prefix, release, priority
//! or unit of any project. The checked table also carries the `[ids]` it
//! was judged against and the file's `[paths]`: the plan resolves a
//! `depends_on` entry as `spec check` does.

use std::collections::BTreeSet;
use std::ops::Range;

use serde::de::Error as _;
use specengine_model::{IdScheme, IdScope, Shape};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::paths_toml::{Paths, checked_path, paths_from_toml};
use crate::project_toml::{ProjectError, slug_problem};

/// The table's name.
const TABLE: &str = "roadmap";
/// `[roadmap]`'s keys, in the order a message lists them.
const KEYS: [&str; 6] = ["prefix", "dir", "releases", "mvp_cut", "priorities", "unit"];
/// The longest `unit`, in UTF-8 bytes.
pub const MAX_UNIT_BYTES: usize = 32;

/// `[roadmap]`, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapConfig {
    /// The `[ids]` prefix of an item's `id:`.
    pub prefix: String,
    /// Where new items are created, root-relative, no trailing `/`.
    pub dir: String,
    /// The path's releases, in order.
    pub releases: Vec<String>,
    /// The last release "to the cut" sums; one of `releases`.
    pub mvp_cut: Option<String>,
    /// The priority scale, highest first.
    pub priorities: Vec<String>,
    /// The label after every sum.
    pub unit: Option<String>,
    /// The `[ids]` the table was judged against: the check's resolution.
    pub scheme: IdScheme,
    /// The file's `[paths]`, defaults applied: the check's resolution
    /// (`[paths] features`).
    pub paths: Paths,
}

impl RoadmapConfig {
    /// Reads `[roadmap]` from `text` (the whole `specengine.toml`), its
    /// `prefix` judged against `scheme`; `None` without the table.
    pub fn from_toml(text: &str, scheme: &IdScheme) -> Result<Option<Self>, ProjectError> {
        roadmap_from_toml(text, scheme)
    }
}

/// [`RoadmapConfig::from_toml`] as a function.
pub fn roadmap_from_toml(
    text: &str,
    scheme: &IdScheme,
) -> Result<Option<RoadmapConfig>, ProjectError> {
    let root = DeTable::parse(text).map_err(|error| ProjectError {
        line: error.span().map(|span| line_of(text, span.start)),
        message: error.message().trim().to_owned(),
    })?;
    let Some((_, table)) = root
        .get_ref()
        .iter()
        .find(|(key, _)| key.get_ref().as_ref() == TABLE)
    else {
        return Ok(None);
    };
    let mut errors = Errors {
        text,
        found: Vec::new(),
    };
    let DeValue::Table(entries) = table.get_ref() else {
        errors.push(
            table.span(),
            format!("`[{TABLE}]` is a table, not {}", type_name(table.get_ref())),
        );
        return Err(errors.first());
    };
    let mut values: [Option<&Spanned<DeValue<'_>>>; 6] = [None; 6];
    for (key, value) in entries.iter() {
        let name = key.get_ref().as_ref();
        match KEYS.iter().position(|known| *known == name) {
            Some(at) => values[at] = Some(value),
            None => errors.push(key.span(), unknown_field(name, &KEYS)),
        }
    }
    let [prefix, dir, releases, mvp_cut, priorities, unit] = values;
    let header = table.span();

    let prefix = required(KEYS[0], prefix, &header, &mut errors)
        .and_then(|value| string(KEYS[0], value, &mut errors))
        .and_then(|(prefix, at)| match prefix_problem(prefix, scheme) {
            None => Some(prefix.to_owned()),
            Some(problem) => {
                errors.push(at, format!("`{TABLE}.{}`: {problem}", KEYS[0]));
                None
            }
        });
    let dir = required(KEYS[1], dir, &header, &mut errors)
        .and_then(|value| string(KEYS[1], value, &mut errors))
        .and_then(|(dir, at)| match checked_path(dir, true) {
            Ok(dir) => Some(dir),
            Err(problem) => {
                errors.push(at, format!("`{TABLE}.{}`: {problem}", KEYS[1]));
                None
            }
        });
    let releases = required(KEYS[2], releases, &header, &mut errors)
        .and_then(|value| slugs(KEYS[2], value, &mut errors));
    let mvp_cut = mvp_cut
        .and_then(|value| string(KEYS[3], value, &mut errors))
        .and_then(|(cut, at)| {
            // Judged against the releases only when they read.
            if let Some(releases) = &releases
                && !releases.iter().any(|release| release == cut)
            {
                errors.push(
                    at,
                    format!(
                        "`{TABLE}.{}`: {cut:?} is not one of `{TABLE}.{}` ({})",
                        KEYS[3],
                        KEYS[2],
                        releases.join(", ")
                    ),
                );
                return None;
            }
            Some(cut.to_owned())
        });
    let priorities = required(KEYS[4], priorities, &header, &mut errors)
        .and_then(|value| slugs(KEYS[4], value, &mut errors));
    let unit = unit
        .and_then(|value| string(KEYS[5], value, &mut errors))
        .and_then(|(unit, at)| match unit_problem(unit) {
            None => Some(unit.to_owned()),
            Some(problem) => {
                errors.push(at, format!("`{TABLE}.{}`{problem}", KEYS[5]));
                None
            }
        });

    if !errors.found.is_empty() {
        return Err(errors.first());
    }
    let (Some(prefix), Some(dir), Some(releases), Some(priorities)) =
        (prefix, dir, releases, priorities)
    else {
        // A required value that did not read pushed its error.
        return Err(errors.first());
    };
    let paths = paths_from_toml(text)?;
    Ok(Some(RoadmapConfig {
        prefix,
        dir,
        releases,
        mvp_cut,
        priorities,
        unit,
        scheme: scheme.clone(),
        paths,
    }))
}

/// Why `prefix` names no item prefix; `None` when it does.
fn prefix_problem(prefix: &str, scheme: &IdScheme) -> Option<String> {
    match scheme.prefix(prefix) {
        None => Some(match scheme.alias(prefix) {
            Some(spec) => format!(
                "`{prefix}` is a legacy alias of `{}`: name the canonical prefix",
                spec.prefix
            ),
            None => format!("`{prefix}` is no `[ids]` prefix"),
        }),
        Some(spec) if spec.shape == Shape::Name => Some(format!(
            "`[ids] {prefix}` is of shape `name`: an item's ID is numbered (shape `number`)"
        )),
        Some(spec) if spec.scope == IdScope::Feature => Some(format!(
            "`[ids] {prefix}` is feature-scoped: an item is a document with its own ID \
             (scope `project`)"
        )),
        Some(_) => None,
    }
}

/// Why `unit` is no unit (the text after the key's name); `None`: valid.
fn unit_problem(unit: &str) -> Option<String> {
    if unit.trim().is_empty() {
        return Some(" is blank".to_owned());
    }
    if unit.len() > MAX_UNIT_BYTES {
        return Some(format!(
            ": {} bytes; a unit has at most {MAX_UNIT_BYTES}",
            unit.len()
        ));
    }
    if unit.chars().any(char::is_control) {
        return Some(" holds a control character".to_owned());
    }
    None
}

/// The errors found, each at its line; the first by line is reported.
struct Errors<'t> {
    text: &'t str,
    found: Vec<(usize, String)>,
}

impl Errors<'_> {
    fn push(&mut self, span: Range<usize>, message: String) {
        self.found.push((line_of(self.text, span.start), message));
    }

    /// The first error by line (the first pushed among one line's).
    fn first(self) -> ProjectError {
        self.found
            .into_iter()
            .enumerate()
            .min_by_key(|(at, (line, _))| (*line, *at))
            .map_or_else(
                || ProjectError {
                    line: None,
                    message: format!("`[{TABLE}]` does not read"),
                },
                |(_, (line, message))| ProjectError {
                    line: Some(line),
                    message,
                },
            )
    }
}

/// A required key's value; missing: an error at the table's header.
fn required<'v>(
    key: &str,
    value: Option<&'v Spanned<DeValue<'v>>>,
    header: &Range<usize>,
    errors: &mut Errors<'_>,
) -> Option<&'v Spanned<DeValue<'v>>> {
    if value.is_none() {
        errors.push(
            header.clone(),
            format!("`[{TABLE}]` has no `{key}`: it is required"),
        );
    }
    value
}

/// A string value and its span; another type: an error.
fn string<'v>(
    key: &str,
    value: &'v Spanned<DeValue<'v>>,
    errors: &mut Errors<'_>,
) -> Option<(&'v str, Range<usize>)> {
    match value.get_ref() {
        DeValue::String(text) => Some((text.as_ref(), value.span())),
        other => {
            errors.push(value.span(), mistyped(key, "a string", other));
            None
        }
    }
}

/// A non-empty list of unique slugs, in the order written; any error
/// pushed (every entry judged) gives `None`.
fn slugs(key: &str, value: &Spanned<DeValue<'_>>, errors: &mut Errors<'_>) -> Option<Vec<String>> {
    let DeValue::Array(items) = value.get_ref() else {
        errors.push(
            value.span(),
            mistyped(key, "an array of strings", value.get_ref()),
        );
        return None;
    };
    if items.is_empty() {
        errors.push(value.span(), format!("`{TABLE}.{key}` is empty"));
        return None;
    }
    let before = errors.found.len();
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        let DeValue::String(text) = item.get_ref() else {
            errors.push(item.span(), mistyped(key, "a string", item.get_ref()));
            continue;
        };
        let text = text.as_ref();
        if let Some(problem) = slug_problem(text) {
            errors.push(item.span(), format!("`{TABLE}.{key}`: {problem}"));
            continue;
        }
        if !seen.insert(text) {
            errors.push(
                item.span(),
                format!("`{TABLE}.{key}`: {text:?} is repeated"),
            );
            continue;
        }
        out.push(text.to_owned());
    }
    (errors.found.len() == before).then_some(out)
}

/// `` `roadmap.<key>`: expected <type>, found <type> ``.
fn mistyped(key: &str, expected: &str, found: &DeValue<'_>) -> String {
    format!(
        "`{TABLE}.{key}`: expected {expected}, found {}",
        type_name(found)
    )
}

/// The TOML reader's message for a key a table does not take.
fn unknown_field(name: &str, expected: &'static [&'static str]) -> String {
    serde::de::value::Error::unknown_field(name, expected).to_string()
}

/// The TOML type of `value` with its article, for messages.
fn type_name(value: &DeValue<'_>) -> &'static str {
    match value {
        DeValue::String(_) => "a string",
        DeValue::Integer(_) => "an integer",
        DeValue::Float(_) => "a float",
        DeValue::Boolean(_) => "a boolean",
        DeValue::Datetime(_) => "a datetime",
        DeValue::Array(_) => "an array",
        DeValue::Table(_) => "a table",
    }
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
