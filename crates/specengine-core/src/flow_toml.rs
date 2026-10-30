//! `[flow]` of `specengine.toml` (task spec `flow-read`): what a project's
//! flow starts from, ends at and calls a step or a definition, read only by
//! `spec flow`. [`crate::ProjectConfig::from_toml`] only name-checks `flow`,
//! so a bad rule fails no other command, and nothing here is a finding or a
//! gate.
//!
//! - `[flow]` holds four arrays of tables, each a list of rules: `step`,
//!   `start`, `end`, `definition`. None (no table, an empty one, `end = []`):
//!   no rule, the row's "not configured".
//! - **Selectors**, at least one, ANDed, the items of a list ORed, rules
//!   ORed: `kinds`, `classes`, `paths`, `when`, the grammar and messages of
//!   `[[check.rules]]` through [`crate::health_toml`]'s reader. They select
//!   documents only.
//! - **Another key** in a rule: the TOML reader's message. In `[flow]`
//!   itself, or `[flow]` not being a table: no table can be trusted, so
//!   every table carries the error.
//!
//! A table's first error by line is kept on the table, never returned: the
//! rows needing it read it as a cause, the others are measured. Pure: the
//! text in, the rules out; it names no kind, class, path or key of any
//! project.

use std::ops::Range;

use specengine_model::IdScheme;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::check::config::DocClass;
use crate::health_toml::{
    Errors, SELECTOR_KEYS, line_of, no_selector, selectors, type_name, unknown_field,
};
use crate::project_toml::ProjectError;

/// The table's name.
const TABLE: &str = "flow";
/// `[flow]`'s keys, in the order a message lists them.
const TABLES: [&str; 4] = ["step", "start", "end", "definition"];

/// One `[[flow.*]]` entry, checked: its selectors (an empty list selects
/// every document).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowRule {
    /// A document's kind, each one an `[ids]` prefix declares.
    pub kinds: Vec<String>,
    /// Its `class:`.
    pub classes: Vec<DocClass>,
    /// Globs over its root-relative path.
    pub paths: Vec<String>,
    /// Its front-matter key → the strings one of which it must be written
    /// as, by key.
    pub when: Vec<(String, Vec<String>)>,
    /// The 1-based line of the entry.
    pub line: usize,
}

/// One table of `[flow]`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowTable {
    /// The rules, in the order written; none under an `error`.
    pub rules: Vec<FlowRule>,
    /// The first error by line: the table cannot be read.
    pub error: Option<ProjectError>,
}

impl FlowTable {
    /// `rules` as read, unless `errors` holds one.
    fn of(rules: Vec<FlowRule>, errors: Errors<'_>) -> Self {
        match errors.first() {
            Some(error) => Self {
                rules: Vec::new(),
                error: Some(error),
            },
            None => Self { rules, error: None },
        }
    }
}

/// `[flow]`, each table read on its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowConfig {
    pub step: FlowTable,
    pub start: FlowTable,
    pub end: FlowTable,
    pub definition: FlowTable,
}

impl FlowConfig {
    /// Reads `[flow]` from `text`: `scheme` declares the kinds a rule may
    /// name. Never an error: see the module documentation.
    pub fn from_toml(text: &str, scheme: &IdScheme) -> Self {
        flow_from_toml(text, scheme)
    }

    /// Every table carrying `error`.
    fn unreadable(error: ProjectError) -> Self {
        let table = FlowTable {
            rules: Vec::new(),
            error: Some(error),
        };
        Self {
            step: table.clone(),
            start: table.clone(),
            end: table.clone(),
            definition: table,
        }
    }
}

/// [`FlowConfig::from_toml`] as a function.
pub fn flow_from_toml(text: &str, scheme: &IdScheme) -> FlowConfig {
    let root = match DeTable::parse(text) {
        Ok(root) => root,
        Err(error) => {
            return FlowConfig::unreadable(ProjectError {
                line: error.span().map(|span| line_of(text, span.start)),
                message: error.message().trim().to_owned(),
            });
        }
    };
    let Some((_, table)) = root
        .get_ref()
        .iter()
        .find(|(key, _)| key.get_ref().as_ref() == TABLE)
    else {
        return FlowConfig::default();
    };
    let DeValue::Table(entries) = table.get_ref() else {
        return FlowConfig::unreadable(ProjectError {
            line: Some(line_of(text, table.span().start)),
            message: format!("`[{TABLE}]` is a table, not {}", type_name(table.get_ref())),
        });
    };
    let declared: Vec<&str> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.kind.as_str())
        .collect();
    let mut errors = TABLES.map(|_| Errors::new(text));
    let mut rules: [Vec<FlowRule>; 4] = Default::default();
    for (key, value) in entries.iter() {
        let name = key.get_ref().as_ref();
        let Some(at) = TABLES.iter().position(|known| *known == name) else {
            for table in &mut errors {
                table.push(key.span(), unknown_field(name, &TABLES));
            }
            continue;
        };
        let errors = &mut errors[at];
        let DeValue::Array(items) = value.get_ref() else {
            errors.push(
                value.span(),
                format!(
                    "`[{TABLE}] {name}` is an array of tables (`[[{TABLE}.{name}]]`), not {}",
                    type_name(value.get_ref())
                ),
            );
            continue;
        };
        let label = format!("`[[{TABLE}.{name}]]`");
        for item in items.iter() {
            let DeValue::Table(entry) = item.get_ref() else {
                errors.push(
                    item.span(),
                    format!("{label} is a table, not {}", type_name(item.get_ref())),
                );
                continue;
            };
            if let Some(rule) = flow_rule(&label, item.span(), entry, &declared, errors) {
                rules[at].push(rule);
            }
        }
    }
    let [step_errors, start_errors, end_errors, definition_errors] = errors;
    let [step, start, end, definition] = rules;
    FlowConfig {
        step: FlowTable::of(step, step_errors),
        start: FlowTable::of(start, start_errors),
        end: FlowTable::of(end, end_errors),
        definition: FlowTable::of(definition, definition_errors),
    }
}

/// One entry checked; `None` when it holds an error (pushed).
fn flow_rule(
    label: &str,
    span: Range<usize>,
    entry: &DeTable<'_>,
    declared: &[&str],
    errors: &mut Errors<'_>,
) -> Option<FlowRule> {
    let before = errors.len();
    let mut written: [Option<&Spanned<DeValue<'_>>>; 4] = [None; 4];
    for (key, value) in entry.iter() {
        let name = key.get_ref().as_ref();
        match SELECTOR_KEYS.iter().position(|known| *known == name) {
            Some(at) => written[at] = Some(value),
            None => errors.push(key.span(), unknown_field(name, &SELECTOR_KEYS)),
        }
    }
    let chosen = selectors(label, written, declared, errors);
    no_selector(label, span.clone(), written, errors);
    (errors.len() == before).then(|| FlowRule {
        kinds: chosen.kinds,
        classes: chosen.classes,
        paths: chosen.paths,
        when: chosen.when,
        line: errors.line(span),
    })
}
