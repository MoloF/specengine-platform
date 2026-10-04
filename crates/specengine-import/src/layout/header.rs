//! A residue document's header, normalised (`docs/canon/import-layout.md` "Headers",
//! documentation-system §8). Keys are read by the import's
//! lenient reader: a top-level key becomes its `key_map` target, a
//! single-line scalar of a `value_map` target its mapping (double-quoted,
//! the comment kept); a missing `id` (document record), `class` and
//! `aliases` go right after `---`; a field table becomes the YAML block.
//! Nothing else of the header changes: bad YAML is carried verbatim, a
//! block core reads as no mapping without a key added
//! (`docs/features/import-layout.md` AC-05).

use std::collections::BTreeSet;

use crate::config::{CensusConfig, CoreKey};
use crate::frontmatter::{self, Entry, FrontMatter};
use crate::import::{HeaderForm, field_table_entries, verbatim_cells};
use crate::markdown::Scan;

use super::{HeaderOutcome, LayoutDiagnostic, typed, yaml};

/// What the header normaliser needs of one document.
pub(super) struct HeaderPlan<'p, 's> {
    pub source: &'p str,
    pub after: &'p str,
    pub text: &'s str,
    pub front: &'p FrontMatter,
    pub scan: &'p Scan<'s>,
    /// Index of the field table in `scan.tables`.
    pub field_table: Option<usize>,
    /// An in-place document definition: (id, written).
    pub document: Option<(&'p str, &'p str)>,
    /// The line of the header key the import read the document's ID from
    /// (1 when it read the path).
    pub document_line: Option<usize>,
    /// The before check could not parse the source's YAML block: it is
    /// carried verbatim (`docs/features/import-layout.md` AC-05).
    pub header_unparsed: bool,
    /// The before check read the source's YAML block as no mapping (a
    /// list, a scalar): carried verbatim, no core key added (AC-05).
    pub header_not_mapping: bool,
}

/// The header lines (delimiters included; none: no header) and the outcome.
pub(super) struct Normalised {
    pub lines: Vec<String>,
    pub outcome: HeaderOutcome,
}

pub(super) fn normalise(
    plan: &HeaderPlan<'_, '_>,
    config: &CensusConfig,
    diagnostics: &mut Vec<LayoutDiagnostic>,
) -> Normalised {
    let import = &config.import;
    let id_key = CoreKey::Id.name();
    let class_key = CoreKey::Class.name();
    let aliases_key = CoreKey::Aliases.name();
    let target_of = |key: &str| {
        import
            .key_map
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_owned())
    };
    let mut outcome = HeaderOutcome {
        path: plan.source.to_owned(),
        after: plan.after.to_owned(),
        source: HeaderForm::None,
        written: false,
        added: Vec::new(),
        renamed: 0,
        mapped: 0,
        conflicts: 0,
        last_row_comment: false,
    };
    let mut conflict = |outcome: &mut HeaderOutcome, line: Option<usize>, message: String| {
        outcome.conflicts += 1;
        diagnostics.push(LayoutDiagnostic {
            path: Some(plan.source.to_owned()),
            line,
            reason: None,
            message,
        });
    };
    // Keys of the after header, in order.
    let mut keys: Vec<String> = Vec::new();
    // `docs/features/import-layout.md` AC-05: a key reaching `id` is renamed
    // only when the import read the document's ID from it and its value is
    // that ID alone; any other is carried under its written name (the `id`
    // a document record needs is added): a value with more than the ID, no
    // ID, a second key, an ID read from the path.
    let id_as_written = |key: &str, target: &str, line: usize, value: Option<&str>| {
        let alone = match (plan.document, plan.document_line) {
            (Some((_, written)), Some(defining)) => defining == line && value == Some(written),
            _ => false,
        };
        target != key && target == id_key && !alone
    };

    // `docs/features/import-layout.md` AC-05: a block core reads as no
    // mapping is no header to add a key to; its lines stay as written (core
    // opened it with exactly `---`), core's findings on it the source's.
    let verbatim = plan.header_unparsed || plan.header_not_mapping;
    // The YAML block, entry lines rewritten in place.
    let mut block: Vec<String> = Vec::new();
    let mut closing: Option<String> = None;
    if let FrontMatter::Present { body_line, .. } = plan.front {
        outcome.source = HeaderForm::Yaml;
        let entries = frontmatter::entries(plan.text);
        let written: BTreeSet<&str> = entries.iter().map(|entry| entry.key).collect();
        let mut entries = entries.iter().peekable();
        for (line, content) in frontmatter::block(plan.text) {
            let Some(entry) = entries.next_if(|entry| entry.line == line) else {
                block.push(content.to_owned());
                continue;
            };
            // A block the before check could not parse: no rename, no value
            // map, no retyping (AC-05).
            if verbatim {
                block.push(content.to_owned());
                keys.push(entry.key.to_owned());
                continue;
            }
            let target = target_of(entry.key);
            let as_written = id_as_written(entry.key, &target, line, entry.value);
            let rename = target != entry.key && !as_written;
            let collides = rename && (keys.contains(&target) || written.contains(target.as_str()));
            let key = if collides {
                conflict(
                    &mut outcome,
                    Some(line),
                    format!(
                        "header key `{}` maps to `{target}`, which the header already holds: kept as written",
                        entry.key
                    ),
                );
                None
            } else if rename {
                outcome.renamed += 1;
                Some(yaml::key(&target))
            } else {
                None
            };
            // A key kept as written takes no value mapping (AC-05).
            let scalar = if collides || as_written {
                rewritten_scalar(entry, None, config, plan.document, &id_key)
            } else {
                rewritten_scalar(
                    entry,
                    Some((&target, rename)),
                    config,
                    plan.document,
                    &id_key,
                )
            };
            if scalar.as_ref().is_some_and(|(_, mapped)| *mapped) {
                outcome.mapped += 1;
            }
            block.push(rewrite(entry, key, scalar.map(|(text, _)| text)));
            keys.push(if collides || as_written {
                entry.key.to_owned()
            } else {
                target
            });
        }
        closing = plan
            .text
            .split_inclusive('\n')
            .nth(body_line.saturating_sub(2))
            .map(|line| line.trim_end_matches(['\r', '\n']).to_owned());
    } else if *plan.front == FrontMatter::Unclosed {
        outcome.source = HeaderForm::Unclosed;
    }

    // A field table becomes YAML entries, after the block's own.
    let mut fields: Vec<String> = Vec::new();
    if let Some(table) = plan
        .field_table
        .and_then(|index| plan.scan.tables.get(index))
    {
        if outcome.source == HeaderForm::None {
            outcome.source = HeaderForm::FieldTable;
        }
        let end = table
            .rows
            .last()
            .map_or(table.header_line + 1, |row| row.line);
        outcome.last_row_comment = plan
            .scan
            .lines
            .iter()
            .find(|line| line.number == end + 1)
            .is_some_and(|line| line.opens_in_comment)
            && !table.rows.is_empty();
        for (line, key, value) in field_table_entries(config, table) {
            let target = target_of(key);
            let as_written = id_as_written(key, &target, line, value);
            let carried = if as_written {
                key.to_owned()
            } else {
                target.clone()
            };
            // A repeated key is carried as `<key>-<n>` (n from 2, the first
            // name the header does not hold), its cell as written: nothing
            // is dropped (AC-05).
            let repeat = keys.contains(&carried).then(|| {
                let mut n = 2;
                while keys.contains(&format!("{carried}-{n}")) {
                    n += 1;
                }
                format!("{carried}-{n}")
            });
            if let Some(name) = &repeat {
                conflict(
                    &mut outcome,
                    Some(line),
                    format!(
                        "field-table key `{key}` repeats the header key `{carried}`: carried as `{name}`"
                    ),
                );
            }
            // The value cell as written, else the scanner's.
            let written = table
                .rows
                .iter()
                .find(|row| row.line == line)
                .and_then(|row| verbatim_cells(row.verbatim, &row.cells).get(1).copied())
                .map(str::to_owned)
                .unwrap_or_else(|| value.unwrap_or("").to_owned());
            // Under another name than its target: no mapping, no rule S.
            if repeat.is_some() || as_written {
                let name = repeat.unwrap_or(carried);
                fields.push(yaml::entry(&name, &typed::value_text(&name, &written)));
                keys.push(name);
                continue;
            }
            if target != key {
                outcome.renamed += 1;
            }
            let mapped = value.and_then(|value| {
                import
                    .value_map
                    .get(&target)
                    .and_then(|values| values.get(value))
                    .filter(|mapped| mapped.as_str() != value)
            });
            let text = match mapped {
                Some(mapped) => {
                    outcome.mapped += 1;
                    mapped.clone()
                }
                None => rule_s(&written, &target, plan.document, &id_key),
            };
            fields.push(yaml::entry(&target, &typed::value_text(&target, &text)));
            keys.push(target);
        }
    }

    // Missing core keys, right after `---`; none in a block core reads as
    // no mapping (AC-05).
    let mut added: Vec<String> = Vec::new();
    let has = |keys: &[String], name: &str| keys.iter().any(|key| key == name);
    let add_keys = !plan.header_not_mapping;
    if add_keys
        && let Some((id, _)) = plan.document
        && !has(&keys, &id_key)
    {
        added.push(yaml::entry(&id_key, &yaml::plain_or_quoted(id)));
        outcome.added.push(id_key.clone());
    }
    if add_keys && !has(&keys, &class_key) {
        let source_class = match plan.front {
            FrontMatter::Present { class, .. } => class.as_deref(),
            _ => None,
        };
        let class = source_class
            .and_then(|class| {
                import
                    .value_map
                    .get(&class_key)
                    .and_then(|values| values.get(class))
            })
            .cloned()
            .or_else(|| config.layout.class_of(plan.after).map(|class| class.name()));
        if let Some(class) = class {
            added.push(yaml::entry(&class_key, &yaml::plain_or_quoted(&class)));
            outcome.added.push(class_key.clone());
        }
    }
    if add_keys
        && let Some((id, written)) = plan.document
        && written != id
    {
        if has(&keys, &aliases_key) {
            conflict(
                &mut outcome,
                None,
                format!("the header's own `{aliases_key}` stays: `{written}` is not added"),
            );
        } else {
            added.push(yaml::entry(
                &aliases_key,
                &yaml::sequence(&[written.to_owned()]),
            ));
            outcome.added.push(aliases_key.clone());
        }
    }

    let mut lines = Vec::new();
    match closing {
        Some(closing) => {
            lines.push(DELIMITER.to_owned());
            lines.extend(added);
            lines.extend(block);
            lines.extend(fields);
            lines.push(closing);
        }
        None if !added.is_empty() || !fields.is_empty() => {
            lines.push(DELIMITER.to_owned());
            lines.extend(added);
            lines.extend(fields);
            lines.push(DELIMITER.to_owned());
        }
        None => {}
    }
    outcome.written = !lines.is_empty();
    Normalised { lines, outcome }
}

const DELIMITER: &str = "---";

/// An entry's scalar rewritten: under its carried key `(key, renamed)`, the
/// `value_map` mapping (`true`; in core's type where `key` is typed), else
/// rule S on an `id` value, else, for a key renamed to a typed core key,
/// the scalar retyped where core would not read it in its type (`false`);
/// `None` when it stays as written. A key kept as written (`carried`
/// `None`: its target taken, or a key reaching `id` the document's ID was
/// not read from alone) takes no mapping and no retyping, rule S only
/// (AC-05).
fn rewritten_scalar(
    entry: &Entry<'_>,
    carried: Option<(&str, bool)>,
    config: &CensusConfig,
    document: Option<(&str, &str)>,
    id_key: &str,
) -> Option<(String, bool)> {
    let range = entry.scalar.clone()?;
    let value = entry.value?;
    let (key, renamed) = carried.unwrap_or((entry.key, false));
    if let Some(mapped) = config
        .import
        .value_map
        .get(key)
        .and_then(|values| values.get(value))
        .filter(|mapped| carried.is_some() && mapped.as_str() != value)
    {
        return Some((typed::value_text(key, mapped), true));
    }
    let scalar = entry.content.get(range)?;
    let changed = rule_s(scalar, key, document, id_key);
    if changed != scalar {
        return Some((changed, false));
    }
    renamed
        .then(|| typed::retyped_scalar(key, scalar, value))
        .flatten()
        .map(|text| (text, false))
}

/// Rule S on a value: under the `id` key, the written ID of a legacy-mapped
/// or look-alike-fixed document record becomes its Latin ID; nothing else
/// changes.
fn rule_s(value: &str, target: &str, document: Option<(&str, &str)>, id_key: &str) -> String {
    match document {
        Some((id, written)) if target == id_key && written != id => value.replacen(written, id, 1),
        _ => value.to_owned(),
    }
}

/// An entry line with its key and scalar replaced where given.
fn rewrite(entry: &Entry<'_>, key: Option<String>, scalar: Option<String>) -> String {
    let content = entry.content;
    let key_range = entry.key_token.clone();
    let mut out = String::with_capacity(content.len() + 8);
    out.push_str(&content[..key_range.start]);
    out.push_str(key.as_deref().unwrap_or(&content[key_range.clone()]));
    match (scalar, entry.scalar.clone()) {
        (Some(scalar), Some(range)) if range.start >= key_range.end => {
            out.push_str(&content[key_range.end..range.start]);
            out.push_str(&scalar);
            out.push_str(&content[range.end..]);
        }
        _ => out.push_str(&content[key_range.end..]),
    }
    out
}
