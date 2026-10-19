//! `[health]` of `specengine.toml` (task spec `health-code`,
//! "`[[health.bound]]`"): which nodes the code must bind, read only by
//! `spec health` (row 5). [`crate::ProjectConfig::from_toml`] only
//! name-checks `health`, so a bad rule fails no other command; `spec check`
//! judges none.
//!
//! - `[health]` holds `bound` alone: an array of tables, each a rule, in
//!   the order written. None (no table, an empty one, `bound = []`): no
//!   rule, the caller's "not configured".
//! - **Selectors**, at least one, ANDed, the items of a list ORed: `kinds`
//!   (each one an `[ids]` prefix declares), `classes` (the four), `paths`
//!   (root-relative globs, the `[paths] exclude` grammar), `when` (a
//!   front-matter key → a string or a non-empty list of strings): the
//!   grammar and messages of `[[check.rules]]`.
//! - **`relations`**: optional, a non-empty list of the caller's words (the
//!   code crate's marker relations); absent: the caller's defaults.
//! - **Another key**: the TOML reader's message (`unknown field`, the keys
//!   it takes).
//!
//! Every value is judged and the first error by line is reported (a bad
//! kind above an unknown key: the kind's), as a [`ProjectError`] at that
//! line (`file:line` through [`ProjectError::at`]). Pure: the text in, the
//! rules out; it names no kind, class, path or relation of any project.

use std::ops::Range;

use serde::de::Error as _;
use specengine_model::IdScheme;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::check::config::DocClass;
use crate::paths_toml::checked_path;
use crate::project_toml::ProjectError;

/// The table's name.
const TABLE: &str = "health";
/// `[health]`'s keys.
const HEALTH_KEYS: [&str; 1] = ["bound"];
/// A rule's keys, in the order a message lists them.
const RULE_KEYS: [&str; 5] = ["kinds", "classes", "paths", "when", "relations"];
/// How a message names a rule.
const RULE: &str = "`[[health.bound]]`";

/// `[health]`, checked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HealthConfig {
    /// The rules, in the order written (row 5's 1-based `rules`).
    pub bound: Vec<BoundRule>,
}

/// One `[[health.bound]]` entry, checked: its selectors (an empty list
/// selects every node) and the relations whose markers bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundRule {
    /// A node's kind, each one an `[ids]` prefix declares.
    pub kinds: Vec<String>,
    /// Its document's `class:`.
    pub classes: Vec<DocClass>,
    /// Globs over its file's root-relative path.
    pub paths: Vec<String>,
    /// Its document's front-matter key → the strings one of which it must
    /// be written as, by key.
    pub when: Vec<(String, Vec<String>)>,
    /// The marker relations that bind, as given.
    pub relations: Vec<String>,
    /// The 1-based line of the entry.
    pub line: usize,
}

impl HealthConfig {
    /// Reads `[health]` from `text`: `scheme` declares the kinds a rule may
    /// name, `relations` the words `relations` takes, `default_relations`
    /// the rule's relations when it lists none.
    pub fn from_toml(
        text: &str,
        scheme: &IdScheme,
        relations: &[&str],
        default_relations: &[&str],
    ) -> Result<Self, ProjectError> {
        health_from_toml(text, scheme, relations, default_relations)
    }
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

    fn first(self) -> Option<ProjectError> {
        self.found
            .into_iter()
            .enumerate()
            .min_by_key(|(at, (line, _))| (*line, *at))
            .map(|(_, (line, message))| ProjectError {
                line: Some(line),
                message,
            })
    }
}

/// [`HealthConfig::from_toml`] as a function.
pub fn health_from_toml(
    text: &str,
    scheme: &IdScheme,
    relations: &[&str],
    default_relations: &[&str],
) -> Result<HealthConfig, ProjectError> {
    let root = DeTable::parse(text).map_err(|error| ProjectError {
        line: error.span().map(|span| line_of(text, span.start)),
        message: error.message().trim().to_owned(),
    })?;
    let Some((_, table)) = root
        .get_ref()
        .iter()
        .find(|(key, _)| key.get_ref().as_ref() == TABLE)
    else {
        return Ok(HealthConfig::default());
    };
    let mut errors = Errors {
        text,
        found: Vec::new(),
    };
    let mut config = HealthConfig::default();
    let DeValue::Table(entries) = table.get_ref() else {
        errors.push(
            table.span(),
            format!("`[{TABLE}]` is a table, not {}", type_name(table.get_ref())),
        );
        return Err(errors.first().unwrap_or_else(unreadable));
    };
    let kinds: Vec<&str> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.kind.as_str())
        .collect();
    let words = Words {
        kinds: &kinds,
        relations,
        default_relations,
    };
    for (key, value) in entries.iter() {
        let name = key.get_ref().as_ref();
        if name != HEALTH_KEYS[0] {
            errors.push(key.span(), unknown_field(name, &HEALTH_KEYS));
            continue;
        }
        let DeValue::Array(items) = value.get_ref() else {
            errors.push(
                value.span(),
                format!(
                    "`[{TABLE}] {name}` is an array of tables ({RULE}), not {}",
                    type_name(value.get_ref())
                ),
            );
            continue;
        };
        for item in items.iter() {
            let DeValue::Table(rule) = item.get_ref() else {
                errors.push(
                    item.span(),
                    format!("{RULE} is a table, not {}", type_name(item.get_ref())),
                );
                continue;
            };
            if let Some(rule) = bound_rule(item.span(), rule, &words, &mut errors) {
                config.bound.push(rule);
            }
        }
    }
    match errors.first() {
        Some(error) => Err(error),
        None => Ok(config),
    }
}

/// What a rule's words are checked against.
struct Words<'w> {
    kinds: &'w [&'w str],
    relations: &'w [&'w str],
    default_relations: &'w [&'w str],
}

/// One entry checked; `None` when it holds an error (pushed).
fn bound_rule(
    span: Range<usize>,
    entry: &DeTable<'_>,
    words: &Words<'_>,
    errors: &mut Errors<'_>,
) -> Option<BoundRule> {
    let before = errors.found.len();
    let mut values: [Option<&Spanned<DeValue<'_>>>; 5] = [None; 5];
    for (key, value) in entry.iter() {
        let name = key.get_ref().as_ref();
        match RULE_KEYS.iter().position(|known| *known == name) {
            Some(at) => values[at] = Some(value),
            None => errors.push(key.span(), unknown_field(name, &RULE_KEYS)),
        }
    }
    let [kinds, classes, paths, when, relations] = values;

    let mut rule = BoundRule {
        kinds: Vec::new(),
        classes: Vec::new(),
        paths: Vec::new(),
        when: Vec::new(),
        relations: Vec::new(),
        line: line_of(errors.text, span.start),
    };
    if let Some(items) = kinds.and_then(|value| strings(RULE_KEYS[0], value, errors)) {
        for (kind, at) in items {
            if words.kinds.contains(&kind) {
                rule.kinds.push(kind.to_owned());
            } else {
                errors.push(
                    at,
                    format!(
                        "{RULE} `{}`: `{kind}` is the kind of no `[ids]` prefix",
                        RULE_KEYS[0]
                    ),
                );
            }
        }
    }
    if let Some(items) = classes.and_then(|value| strings(RULE_KEYS[1], value, errors)) {
        for (class, at) in items {
            match DocClass::parse(class) {
                Some(known) => rule.classes.push(known),
                None => {
                    let all: Vec<&str> = DocClass::ALL.iter().map(|class| class.as_str()).collect();
                    errors.push(
                        at,
                        format!(
                            "{RULE} `{}`: `{class}` is none of {}",
                            RULE_KEYS[1],
                            all.join(" | ")
                        ),
                    );
                }
            }
        }
    }
    if let Some(items) = paths.and_then(|value| strings(RULE_KEYS[2], value, errors)) {
        for (glob, at) in items {
            match checked_path(glob, false) {
                Ok(checked) => rule.paths.push(checked),
                Err(problem) => {
                    errors.push(at, format!("{RULE} `{}`: {problem}", RULE_KEYS[2]));
                }
            }
        }
    }
    if let Some(value) = when {
        rule.when = condition(value, errors);
    }
    match relations {
        None => {
            rule.relations = words
                .default_relations
                .iter()
                .map(|word| (*word).to_owned())
                .collect();
        }
        Some(value) => {
            for (relation, at) in strings(RULE_KEYS[4], value, errors).unwrap_or_default() {
                if words.relations.contains(&relation) {
                    rule.relations.push(relation.to_owned());
                } else {
                    errors.push(
                        at,
                        format!(
                            "{RULE} `{}`: `{relation}` is none of {}",
                            RULE_KEYS[4],
                            words.relations.join(" | ")
                        ),
                    );
                }
            }
        }
    }
    if kinds.is_none() && classes.is_none() && paths.is_none() && when.is_none() {
        errors.push(
            span,
            format!(
                "{RULE} without a selector: give `{}`, `{}`, `{}` or `{}`",
                RULE_KEYS[0], RULE_KEYS[1], RULE_KEYS[2], RULE_KEYS[3]
            ),
        );
    }
    (errors.found.len() == before).then_some(rule)
}

/// A selector's list: each string with its span; `None` when it is no
/// non-empty list of strings (the error pushed).
fn strings<'v>(
    what: &str,
    value: &'v Spanned<DeValue<'_>>,
    errors: &mut Errors<'_>,
) -> Option<Vec<(&'v str, Range<usize>)>> {
    let DeValue::Array(items) = value.get_ref() else {
        errors.push(
            value.span(),
            format!(
                "{RULE} `{what}`: a list of strings, not {}",
                type_name(value.get_ref())
            ),
        );
        return None;
    };
    if items.is_empty() {
        errors.push(value.span(), format!("{RULE} `{what}` is empty"));
        return None;
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        match item.get_ref() {
            DeValue::String(text) => out.push((text.as_ref(), item.span())),
            other => {
                errors.push(
                    item.span(),
                    format!(
                        "{RULE} `{what}`: a list of strings, not one holding {}",
                        type_name(other)
                    ),
                );
                return None;
            }
        }
    }
    Some(out)
}

/// `when`: key → its strings, by key; the entries judged in source order.
fn condition(value: &Spanned<DeValue<'_>>, errors: &mut Errors<'_>) -> Vec<(String, Vec<String>)> {
    let what = RULE_KEYS[3];
    let DeValue::Table(table) = value.get_ref() else {
        errors.push(
            value.span(),
            format!(
                "{RULE} `{what}`: a table, not {}",
                type_name(value.get_ref())
            ),
        );
        return Vec::new();
    };
    if table.is_empty() {
        errors.push(value.span(), format!("{RULE} `{what}` is empty"));
        return Vec::new();
    }
    let mut entries: Vec<_> = table.iter().collect();
    entries.sort_by_key(|(key, _)| key.span().start);
    let mut out: Vec<(String, Vec<String>)> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let name = key.get_ref().as_ref();
        let span = value.span();
        if name.trim().is_empty() {
            errors.push(span, format!("{RULE} `{what}`: an empty key name"));
            continue;
        }
        let strings = match value.get_ref() {
            DeValue::String(one) => vec![one.to_string()],
            DeValue::Array(list) if list.is_empty() => {
                errors.push(span, format!("{RULE} `{what}` `{name}`: an empty list"));
                continue;
            }
            DeValue::Array(list) => {
                let mut strings = Vec::with_capacity(list.len());
                for item in list.iter() {
                    match item.get_ref() {
                        DeValue::String(one) => strings.push(one.to_string()),
                        other => {
                            errors.push(
                                span.clone(),
                                format!(
                                    "{RULE} `{what}` `{name}`: a list of strings, not one holding {}",
                                    type_name(other)
                                ),
                            );
                            strings.clear();
                            break;
                        }
                    }
                }
                if strings.is_empty() {
                    continue;
                }
                strings
            }
            other => {
                errors.push(
                    span,
                    format!(
                        "{RULE} `{what}` `{name}`: a string or a list of strings, not {}",
                        type_name(other)
                    ),
                );
                continue;
            }
        };
        out.push((name.to_owned(), strings));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
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

/// An error pushed and then not found: never (a guard, not a panic).
fn unreadable() -> ProjectError {
    ProjectError {
        line: None,
        message: format!("`[{TABLE}]` does not read"),
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
