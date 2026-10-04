//! Document headers at import: YAML top-level keys and the field/value table
//! opening a body, mapped through `front_matter.key_map` and `value_map`
//! (`docs/features/import-records.md` AC-03). A key or value the maps rename
//! is `mapped`; one equal to a target (an identity entry included) is
//! `kept`; any other, whatever its script, `unmapped`
//! (`docs/features/import-gaps.md` gap 4).

use serde::Serialize;

use crate::census::clean_cell;
use crate::config::{CensusConfig, ImportConfig};
use crate::markdown::Table;

/// How a key or a value relates to the maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MapOutcome {
    /// Written as a map key with another target: renamed.
    Mapped,
    /// Equal to a target (an identity map entry included).
    Kept,
    Unmapped,
}

/// One header key of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyEntry {
    pub line: usize,
    pub written: String,
    pub target: Option<String>,
    pub outcome: MapOutcome,
}

/// One single-line value of a key whose target has a value map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValueEntry {
    pub line: usize,
    /// The key's target.
    pub key: String,
    pub written: String,
    pub target: Option<String>,
    pub outcome: MapOutcome,
}

/// Keys and values of one document's header.
#[derive(Debug, Default)]
pub(crate) struct HeaderKeys {
    pub keys: Vec<KeyEntry>,
    pub values: Vec<ValueEntry>,
    /// A key holds a letter outside ASCII.
    pub non_latin_key: bool,
}

impl HeaderKeys {
    /// Adds one `key: value` of the header.
    pub fn add(&mut self, import: &ImportConfig, line: usize, key: &str, value: Option<&str>) {
        if key.chars().any(|c| !c.is_ascii() && c.is_alphabetic()) {
            self.non_latin_key = true;
        }
        let (target, outcome) = match import.key_map.get(key) {
            Some(target) => (Some(target.clone()), renamed(key, target)),
            None if is_target_key(import, key) => (Some(key.to_owned()), MapOutcome::Kept),
            None => (None, MapOutcome::Unmapped),
        };
        if let (Some(target), Some(value)) = (&target, value)
            && let Some(values) = import.value_map.get(target)
        {
            let (mapped, value_outcome) = match values.get(value) {
                Some(mapped) => (Some(mapped.clone()), renamed(value, mapped)),
                None if values.values().any(|known| known == value) => {
                    (Some(value.to_owned()), MapOutcome::Kept)
                }
                None => (None, MapOutcome::Unmapped),
            };
            self.values.push(ValueEntry {
                line,
                key: target.clone(),
                written: value.to_owned(),
                target: mapped,
                outcome: value_outcome,
            });
        }
        self.keys.push(KeyEntry {
            line,
            written: key.to_owned(),
            target,
            outcome,
        });
    }
}

/// A map entry renames unless it is an identity.
fn renamed(written: &str, target: &str) -> MapOutcome {
    if written == target {
        MapOutcome::Kept
    } else {
        MapOutcome::Mapped
    }
}

/// A key is a target when a key map entry, a value map or
/// `documents.id_key` names it.
fn is_target_key(import: &ImportConfig, key: &str) -> bool {
    import.key_map.values().any(|target| target == key)
        || import.value_map.contains_key(key)
        || import.documents.id_key.as_deref() == Some(key)
}

/// Whether a table is the document's field table: it opens the body, has a
/// header row of two cells and its first header cell (decoration removed, as
/// for `tables.id_header`) matches `front_matter.header_table`.
pub(crate) fn is_field_table(config: &CensusConfig, table: &Table<'_>) -> bool {
    let Some(pattern) = &config.import.header_table else {
        return false;
    };
    table.opens_body
        && table
            .header
            .as_ref()
            .is_some_and(|header| header.len() == 2 && pattern.is_match(clean_cell(&header[0])))
}

/// The fields of a field table: the header row first when
/// `header_row_field`, then each row's first cell as the key and its second
/// as the value (an empty value is no value).
pub(crate) fn field_table_entries<'t>(
    config: &CensusConfig,
    table: &'t Table<'_>,
) -> Vec<(usize, &'t str, Option<&'t str>)> {
    let mut fields = Vec::new();
    if config.import.header_row_field
        && let Some(header) = &table.header
    {
        fields.push((
            table.header_line,
            header[0].as_str(),
            header
                .get(1)
                .map(String::as_str)
                .filter(|value| !value.is_empty()),
        ));
    }
    for row in &table.rows {
        let Some(key) = row.cells.first().filter(|key| !key.is_empty()) else {
            continue;
        };
        let value = row
            .cells
            .get(1)
            .map(String::as_str)
            .filter(|value| !value.is_empty());
        fields.push((row.line, key.as_str(), value));
    }
    fields
}
