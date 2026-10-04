//! Record files and reshaped `{#ID}` headings
//! (`docs/canon/import-layout.md` "Record files", "Reshaped sections"):
//! what a definition carries besides its text — ID, class, title, aliases,
//! task box, fields, a section's other heading attributes.

use std::collections::BTreeSet;

use crate::config::{CensusConfig, CoreKey};
use crate::import::{Field, ImportRecord};

use super::{typed, yaml};

/// The keys every record file holds or may hold before its fields: a field
/// or attribute named like one of them takes `col-N` (or is dropped).
fn reserved(config: &CensusConfig) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = [
        CoreKey::Id,
        CoreKey::Class,
        CoreKey::Title,
        CoreKey::Aliases,
    ]
    .into_iter()
    .map(CoreKey::name)
    .collect();
    if let Some(task) = &config.layout.task_box_key {
        keys.insert(task.clone());
    }
    keys
}

/// The fields `record` carries, in column order: a row's ID cell holding
/// text beyond its ID ([`ImportRecord::id_cell`];
/// `docs/features/import-layout.md` AC-03, AC-04) and its other cells.
pub fn carried_fields(record: &ImportRecord) -> Vec<&Field> {
    let mut fields: Vec<&Field> = record.id_cell.iter().chain(&record.fields).collect();
    fields.sort_by_key(|field| field.column);
    fields
}

/// The key each of [`carried_fields`] is carried under, in column order:
/// the header's `key_map` target, else the header as written (the ID
/// cell's: its header as written, AC-03); a key already taken (a core key,
/// the task-box key, an earlier field) → `col-N` (N the 0-based column);
/// taken again → `None`, dropped (a header conflict). The verifier reads
/// the fields back under the same keys.
pub fn field_keys(record: &ImportRecord, config: &CensusConfig) -> Vec<Option<String>> {
    let mut taken = reserved(config);
    let id_column = record.id_cell.as_ref().map(|cell| cell.column);
    carried_fields(record)
        .into_iter()
        .map(|field| {
            let mapped = config.import.key_map.get(&field.header);
            let key = match mapped {
                Some(target) if Some(field.column) != id_column => target.clone(),
                _ => field.header.clone(),
            };
            let key = if taken.contains(&key) {
                format!("col-{}", field.column)
            } else {
                key
            };
            taken.insert(key.clone()).then_some(key)
        })
        .collect()
}

/// A section heading as written, split (`docs/features/import-layout.md`
/// AC-03): the title is the heading minus its ATX marker (the opening run
/// and an optional closing sequence) and minus exactly the `{#…}` block the
/// import recognises (the first `{#` to the next `}`), the text on either
/// side trimmed and the two joined by one space; the attributes are the
/// block's tokens after the anchor, a bare one with no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHeading {
    pub title: String,
    pub attributes: Vec<(String, Option<String>)>,
}

/// Reads an ATX heading line holding a `{#…}` block; `None` for any other.
pub fn section_heading(raw: &str) -> Option<SectionHeading> {
    let content = raw.trim_start_matches(' ');
    if raw.len() - content.len() > 3 {
        return None;
    }
    let level = content.bytes().take_while(|&byte| byte == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &content[level..];
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let rest = without_closing_sequence(rest);
    let open = rest.find("{#")?;
    let block = &rest[open + 1..];
    let close = block.find('}')?;
    let mut tokens = block[..close].split_whitespace();
    tokens.next()?;
    let attributes = tokens
        .map(|token| match token.split_once('=') {
            Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
            None => (token.to_owned(), None),
        })
        .collect();
    let sides = [rest[..open].trim(), block[close + 1..].trim()];
    Some(SectionHeading {
        title: sides
            .into_iter()
            .filter(|side| !side.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        attributes,
    })
}

/// An ATX heading's content without its optional closing sequence: a run
/// of `#` at the end that is all of it or follows a blank, with the blanks
/// around it (CommonMark).
fn without_closing_sequence(content: &str) -> &str {
    let trimmed = content.trim_end_matches([' ', '\t']);
    let open = trimmed.trim_end_matches('#');
    if open.len() == trimmed.len() || !(open.is_empty() || open.ends_with([' ', '\t'])) {
        trimmed
    } else {
        open.trim_end_matches([' ', '\t'])
    }
}

/// What a record file carries besides the ID.
pub(super) struct RecordFile<'r> {
    pub record: &'r ImportRecord,
    pub title: Option<String>,
    /// A section's heading attributes other than its anchor.
    pub attributes: Vec<(String, Option<String>)>,
    /// The body, already [`crate::import::document_text`]-normalised.
    pub body: String,
}

/// A record file's text and its header conflicts: `---`; `id`, `class`
/// plain; `title`; `aliases` when the ID was written otherwise; the task
/// key; fields in column order; a section's attributes (bare → `true`);
/// `---`, a blank line, the body. Strings double-quoted, a typed core
/// key's value in core's type where it parses (AC-05).
pub(super) fn render(file: &RecordFile<'_>, config: &CensusConfig) -> (String, usize) {
    let record = file.record;
    let mut conflicts = 0;
    let mut lines = vec![DELIMITER.to_owned()];
    lines.push(yaml::entry(
        &CoreKey::Id.name(),
        &yaml::plain_or_quoted(&record.id),
    ));
    lines.push(yaml::entry(
        &CoreKey::Class.name(),
        &yaml::plain_or_quoted(&config.layout.record_class.name()),
    ));
    if let Some(title) = file.title.as_deref().filter(|title| !title.is_empty()) {
        lines.push(yaml::entry(&CoreKey::Title.name(), &yaml::quoted(title)));
    }
    if record.written != record.id {
        lines.push(yaml::entry(
            &CoreKey::Aliases.name(),
            &yaml::sequence(std::slice::from_ref(&record.written)),
        ));
    }
    if let (Some(key), Some(checked)) = (&config.layout.task_box_key, record.task_box) {
        lines.push(yaml::entry(key, &checked.to_string()));
    }
    let mut taken = reserved(config);
    for (field, key) in carried_fields(record)
        .into_iter()
        .zip(field_keys(record, config))
    {
        match key {
            Some(key) => {
                lines.push(yaml::entry(&key, &typed::value_text(&key, &field.value)));
                taken.insert(key);
            }
            None => conflicts += 1,
        }
    }
    for (key, value) in &file.attributes {
        if !taken.insert(key.clone()) {
            conflicts += 1;
            continue;
        }
        let value = value
            .as_deref()
            .map_or_else(|| true.to_string(), |value| typed::value_text(key, value));
        lines.push(yaml::entry(key, &value));
    }
    lines.push(DELIMITER.to_owned());
    if !file.body.is_empty() {
        lines.push(String::new());
        lines.push(file.body.clone());
    }
    (format!("{}\n", lines.join("\n")), conflicts)
}

const DELIMITER: &str = "---";

/// A reshaped row's or item's heading after its ATX marker: title else ID,
/// ` {#ID`, the task key `=true|false`, each field `key=value`, `}`. A
/// field (or task key) holding a blank, a brace, or a key that is no
/// attribute key cannot be carried: the second value counts those
/// (`section_fields`, a field mismatch).
pub(super) fn reshaped_heading(record: &ImportRecord, config: &CensusConfig) -> (String, usize) {
    let mut lost = 0;
    let mut heading = format!(
        "{} {{#{}",
        record
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .unwrap_or(&record.id),
        record.id
    );
    // Whether `key=value` went into the block.
    let attribute = |key: &str, value: &str, heading: &mut String| {
        let carried = attribute_safe(key, true) && attribute_safe(value, false);
        if carried {
            heading.push_str(&format!(" {key}={value}"));
        }
        carried
    };
    if let (Some(key), Some(checked)) = (&config.layout.task_box_key, record.task_box)
        && !attribute(key, &checked.to_string(), &mut heading)
    {
        lost += 1;
    }
    for (field, key) in carried_fields(record)
        .into_iter()
        .zip(field_keys(record, config))
    {
        let carried = key.is_some_and(|key| attribute(&key, &field.value, &mut heading));
        if !carried {
            lost += 1;
        }
    }
    heading.push('}');
    (heading, lost)
}

/// A heading attribute's key or value as `{… key=value}` carries it: no
/// blank, no brace; a key not empty, without `=`, not opening like an
/// anchor (`#`) or a class (`.`).
fn attribute_safe(text: &str, key: bool) -> bool {
    let plain = !text
        .chars()
        .any(|c| c.is_whitespace() || c == '{' || c == '}');
    if key {
        plain && !text.is_empty() && !text.contains('=') && !text.starts_with(['#', '.'])
    } else {
        plain
    }
}
