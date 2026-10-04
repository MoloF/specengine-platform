//! The before scheme (`--scheme`, a `specengine.toml`) as plain data, and
//! the `specengine.toml` the after-tree carries
//! (`docs/canon/import-layout.md` "Emitted configs"). The scheme is read
//! as a generic TOML table: `specengine-core` validated it already, and this
//! crate does not depend on it.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use serde::Serialize;
use toml::{Table, Value};

use crate::config::{CensusConfig, word};

use super::{LayoutDiagnostic, toml_out};

/// A prefix's `scope` in the scheme: the words are serde's, from the
/// variants (`crate::config::word`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum SchemeScope {
    Feature,
}

/// The before scheme: the whole `specengine.toml` table.
#[derive(Debug, Clone)]
pub struct Scheme {
    table: Table,
}

impl Scheme {
    /// Reads `specengine.toml` text as TOML (its rules are core's; the
    /// caller loads it through core first).
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str::<Table>(text)
            .map(|table| Self { table })
            .map_err(|error| error.message().trim().to_owned())
    }

    /// The `[ids]` table, if any.
    fn ids(&self) -> Option<&Table> {
        self.table.get(IDS).and_then(Value::as_table)
    }

    /// Whether `[ids]` configures `prefix`.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        self.ids().is_some_and(|ids| ids.contains_key(prefix))
    }

    /// Whether `[ids]` scopes `prefix` to a feature.
    pub fn is_feature(&self, prefix: &str) -> bool {
        let feature = word(&SchemeScope::Feature);
        self.ids()
            .and_then(|ids| ids.get(prefix))
            .and_then(|entry| entry.get(SCOPE))
            .and_then(Value::as_str)
            .is_some_and(|scope| scope == feature)
    }

    /// The configured prefixes, sorted.
    pub fn prefixes(&self) -> Vec<String> {
        self.ids()
            .map(|ids| ids.keys().cloned().collect())
            .unwrap_or_default()
    }
}

const IDS: &str = "ids";
const SCOPE: &str = "scope";
const ALIASES_FROM: &str = "aliases_from";
const PATHS: &str = "paths";

/// The after-tree's `specengine.toml`: the before scheme with `[paths]`
/// `records`, `features` from `[layout]`, `roots` the sorted first
/// components of `tree` (the after paths), `exclude = []`, `link_base` its
/// own else `[links] base`; each `[ids.legacy]` key joins its target's
/// `aliases_from` (a key that is a prefix, another prefix's alias or names
/// an unconfigured target: a diagnostic, skipped); `scope = "feature"` for
/// each configured `feature_prefixes` entry. Keys sorted.
pub fn emit_scheme<'p>(
    scheme: &Scheme,
    config: &CensusConfig,
    tree: impl IntoIterator<Item = &'p str>,
) -> (String, Vec<LayoutDiagnostic>) {
    let mut diagnostics = Vec::new();
    let mut table = scheme.table.clone();
    let layout = &config.layout;

    let mut paths = match table.remove(PATHS) {
        Some(Value::Table(paths)) => paths,
        _ => Table::new(),
    };
    let roots: BTreeSet<String> = tree
        .into_iter()
        .filter_map(|path| path.split('/').next())
        .filter(|first| !first.is_empty())
        .map(str::to_owned)
        .collect();
    paths.insert("records".to_owned(), Value::String(layout.records.clone()));
    paths.insert(
        "features".to_owned(),
        Value::String(layout.features.clone()),
    );
    paths.insert(
        "roots".to_owned(),
        Value::Array(roots.into_iter().map(Value::String).collect()),
    );
    paths.insert("exclude".to_owned(), Value::Array(Vec::new()));
    if !paths.contains_key("link_base")
        && let Some(base) = config.import.link_base.as_deref().map(slash_path)
        && !base.is_empty()
    {
        paths.insert("link_base".to_owned(), Value::String(base));
    }
    table.insert(PATHS.to_owned(), Value::Table(paths));

    let mut ids = match table.remove(IDS) {
        Some(Value::Table(ids)) => ids,
        _ => Table::new(),
    };
    for (written, target) in &config.import.legacy {
        let problem = if ids.contains_key(written) {
            Some(format!(
                "legacy prefix `{written}` is itself a prefix of the scheme: no alias"
            ))
        } else if let Some(other) = ids.iter().find(|(prefix, entry)| {
            *prefix != target && aliases(entry).iter().any(|alias| alias == written)
        }) {
            Some(format!(
                "legacy prefix `{written}` is already an alias of `{}`: not added to `{target}`",
                other.0
            ))
        } else if !ids.contains_key(target) {
            Some(format!(
                "legacy prefix `{written}` maps to `{target}`, which the scheme does not configure: no alias"
            ))
        } else {
            None
        };
        if let Some(message) = problem {
            diagnostics.push(LayoutDiagnostic::config(message));
            continue;
        }
        if let Some(Value::Table(entry)) = ids.get_mut(target) {
            let mut list = aliases(&Value::Table(entry.clone()));
            if !list.iter().any(|alias| alias == written) {
                list.push(written.clone());
                entry.insert(
                    ALIASES_FROM.to_owned(),
                    Value::Array(list.into_iter().map(Value::String).collect()),
                );
            }
        }
    }
    let feature = word(&SchemeScope::Feature);
    for prefix in &config.import.feature_prefixes {
        if let Some(Value::Table(entry)) = ids.get_mut(prefix) {
            entry.insert(SCOPE.to_owned(), Value::String(feature.clone()));
        }
    }
    if !ids.is_empty() {
        table.insert(IDS.to_owned(), Value::Table(ids));
    }
    (toml_out::write(&table), diagnostics)
}

/// An entry's `aliases_from` strings.
fn aliases(entry: &Value) -> Vec<String> {
    entry
        .get(ALIASES_FROM)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A corpus-relative path, `/`-separated.
fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
