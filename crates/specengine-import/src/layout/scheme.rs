//! The before scheme (`--scheme`, a `specengine.toml`) as plain data, and
//! the `specengine.toml` the after-tree carries
//! (`docs/canon/import-layout.md` "Emitted configs"). The scheme is read
//! as a generic TOML table: `specengine-core` validated it already, and this
//! crate does not depend on it.

use std::path::{Component, Path};

use serde::Serialize;
use toml::{Table, Value};

use crate::config::{CensusConfig, word};

use super::LayoutDiagnostic;
use super::toml_out::{self, Comment};

/// A prefix's `scope` in the scheme: the words are serde's, from the
/// variants (`crate::config::word`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum SchemeScope {
    Feature,
}

/// The key of a prefix's entry in `[ids]` the layout reads and writes, a
/// word of the format named by serde like [`SchemeScope`]: a corpus may use
/// the same word as a header key (`docs/features/import-homes.md`), so no
/// string literal of the engine holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum PrefixKey {
    Scope,
}

/// The before scheme: the whole `specengine.toml` table and its walk as
/// core reads it.
#[derive(Debug, Clone)]
pub struct Scheme {
    table: Table,
    /// Core's `[paths] roots`: as written, duplicates dropped; unwritten,
    /// the role defaults.
    roots: Vec<String>,
    /// Core's `[paths] exclude`, in order, each with whether it matches an
    /// index output the emitted scheme writes.
    exclude: Vec<(String, bool)>,
}

impl Scheme {
    /// Reads `specengine.toml` text as TOML (its rules are core's; the
    /// caller loads it through core first) with the walk core read from it:
    /// its `roots` and `exclude` (`docs/features/import-lead-ins.md`).
    /// `hides_index_output` tells, by core's glob rule, whether an exclude
    /// glob matches an index output the emitted scheme writes (the `[paths]
    /// index` file or a shard of its `index = true` generator).
    pub fn parse(
        text: &str,
        roots: &[String],
        exclude: &[String],
        hides_index_output: impl Fn(&str) -> bool,
    ) -> Result<Self, String> {
        toml::from_str::<Table>(text)
            .map(|table| Self {
                table,
                roots: roots.to_vec(),
                exclude: exclude
                    .iter()
                    .map(|glob| (glob.clone(), hides_index_output(glob)))
                    .collect(),
            })
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
            .and_then(|entry| entry.get(word(&PrefixKey::Scope)))
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
const ALIASES_FROM: &str = "aliases_from";
const PATHS: &str = "paths";
const EXCLUDE: &str = "exclude";

/// The after-tree's `specengine.toml`: the before scheme with `[paths]`
/// `records`, `features` from `[layout]`; `roots` the before roots each
/// holding a path of `tree` (the after paths: its documents, and the index
/// outputs when rendered) at or under it, then `records`, then `features`,
/// each appended when no kept root equals or holds it and a path of `tree`
/// lies under it; `exclude` the before globs, in order, but each matching
/// an index output this scheme writes, which a comment line right above
/// `exclude` names instead (`docs/features/import-lead-ins.md`);
/// `link_base` its own else `[links]
/// base`; each `[ids.legacy]` key joins its target's `aliases_from` (a key
/// that is a prefix, another prefix's alias or names an unconfigured
/// target: a diagnostic, skipped); `scope = "feature"` for each configured
/// `feature_prefixes` entry. Keys sorted.
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
    let tree: Vec<&str> = tree.into_iter().collect();
    let mut roots: Vec<String> = scheme
        .roots
        .iter()
        .filter(|root| tree.iter().any(|path| at_or_under(path, root)))
        .cloned()
        .collect();
    let kept = roots.len();
    for role in [&layout.records, &layout.features] {
        let held = roots[..kept].iter().any(|root| at_or_under(role, root));
        if !held && !roots.contains(role) && tree.iter().any(|path| under(path, role)) {
            roots.push(role.clone());
        }
    }
    paths.insert("records".to_owned(), Value::String(layout.records.clone()));
    paths.insert(
        "features".to_owned(),
        Value::String(layout.features.clone()),
    );
    paths.insert(
        "roots".to_owned(),
        Value::Array(roots.into_iter().map(Value::String).collect()),
    );
    let kept_exclude = scheme
        .exclude
        .iter()
        .filter(|(_, hides)| !hides)
        .map(|(glob, _)| Value::String(glob.clone()))
        .collect();
    paths.insert(EXCLUDE.to_owned(), Value::Array(kept_exclude));
    let comments: Vec<Comment<'_>> = scheme
        .exclude
        .iter()
        .filter(|(_, hides)| *hides)
        .map(|(glob, _)| Comment {
            section: PATHS,
            key: EXCLUDE,
            line: format!(
                "exclude dropped: {} matches an index output this scheme writes",
                toml_out::basic_string(glob)
            ),
        })
        .collect();
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
            entry.insert(word(&PrefixKey::Scope), Value::String(feature.clone()));
        }
    }
    if !ids.is_empty() {
        table.insert(IDS.to_owned(), Value::Table(ids));
    }
    (toml_out::write_commented(&table, &comments), diagnostics)
}

/// `path` is `root` or lies under it.
fn at_or_under(path: &str, root: &str) -> bool {
    path == root || under(path, root)
}

/// `path` lies under the directory `dir`.
fn under(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
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
