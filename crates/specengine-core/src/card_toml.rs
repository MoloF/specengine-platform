//! `[card]` of `specengine.toml` (task spec `card-store`, "Data"): each
//! discipline's columns, moves and who makes them, read only by `spec card`
//! and its MCP twins. [`crate::ProjectConfig::from_toml`] only name-checks
//! `card`, so a bad table fails no other command.
//!
//! - `[card]` holds `flow` alone: an array of tables, at most
//!   [`MAX_FLOWS`]. None (no table, an empty one, `flow = []`): core's
//!   [`PLAIN_FLOW`], TOML through this reader.
//! - **Names** (`name`, `columns`, `disciplines`): `[a-z0-9_-]`, 1 to
//!   [`MAX_NAME_BYTES`] bytes; each once in its list. [`MIN_COLUMNS`] to
//!   [`MAX_COLUMNS`] columns. `titles = { id = "text" }`, any script, at most
//!   [`MAX_TITLE_BYTES`] bytes, default the ID.
//! - **`accept`** required, `{from, to}`, two columns: a person reaches `to`
//!   by accepting, on a terminal or the local page, and no agent tool does,
//!   so a move ending there, or a new card starting there, is an error.
//! - **`moves`**: `{from, to, by?}` of two columns, each pair once. No `by`:
//!   anyone; `by` names as `--as` writes them. No `moves`: every ordered pair
//!   of the columns but `accept.to`, expanded here, so a reader looks moves
//!   up and never judges them.
//! - **`return`** (canon `card-flows`, "The rule"), optional: one column a
//!   move enters only with a note saying why. Never the first column,
//!   `accept.from` or `accept.to`. Absent: no such column. Which column it is
//!   stays data: no code reads its name.
//! - **Another key**: the TOML reader's message.
//!
//! Every value is judged and the first error by line is returned
//! ([`ProjectError`]), so the caller prints `<config>:<line>: <message>`.
//! Pure: the text in, the flows out. No column means anything here: the
//! words of [`PLAIN_FLOW`] are data, and no code reads a column's name.

use std::collections::BTreeSet;
use std::ops::Range;

use serde::Serialize;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::card::{card_person, card_person_problem};
use crate::health_toml::{Errors, line_of, type_name, unknown_field};
use crate::project_toml::ProjectError;

/// The table's name.
const TABLE: &str = "card";
/// `[card]`'s keys.
const CARD_KEYS: [&str; 1] = ["flow"];
/// A flow's keys, in the order a message lists them.
const FLOW_KEYS: [&str; 7] = [
    "name",
    "disciplines",
    "columns",
    "titles",
    "accept",
    "return",
    "moves",
];
/// `accept`'s keys.
const ACCEPT_KEYS: [&str; 2] = ["from", "to"];
/// A move's keys.
const MOVE_KEYS: [&str; 3] = ["from", "to", "by"];
/// How a message names a flow.
const FLOW: &str = "`[[card.flow]]`";

/// The most `[[card.flow]]` tables.
pub const MAX_FLOWS: usize = 32;
/// The fewest columns of a flow.
pub const MIN_COLUMNS: usize = 2;
/// The most columns of a flow.
pub const MAX_COLUMNS: usize = 32;
/// The longest name, in bytes.
pub const MAX_NAME_BYTES: usize = 64;
/// The longest column title, in UTF-8 bytes.
pub const MAX_TITLE_BYTES: usize = 64;

/// The flow of a project with no `[[card.flow]]`: the default board.
pub const PLAIN_FLOW: &str = r#"[[card.flow]]
name = "plain"
columns = ["backlog", "ready", "in-progress", "review", "done"]
titles = { backlog = "Backlog", ready = "Ready", in-progress = "In progress", review = "Review", done = "Done" }
accept = { from = "review", to = "done" }
"#;

/// One column: its ID and how a board titles it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CardColumn {
    pub id: String,
    pub title: String,
}

/// `accept`: the column a card is accepted from, and the one that takes it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CardAccept {
    pub from: String,
    pub to: String,
}

/// One move a flow allows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CardMove {
    pub from: String,
    pub to: String,
    /// Who makes it; empty: anyone.
    pub by: Vec<String>,
}

/// One `[[card.flow]]`, checked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct CardFlow {
    pub name: String,
    pub disciplines: Vec<String>,
    /// In the order written: a new card takes the first.
    pub columns: Vec<CardColumn>,
    /// Expanded, in the order written (else by column order).
    pub moves: Vec<CardMove>,
    pub accept: CardAccept,
    /// `return`: the column a move enters only with a note; `None`: none.
    /// Served as `return`, after `accept`.
    #[serde(rename = "return")]
    pub return_column: Option<String>,
}

impl CardFlow {
    /// `column` is a column of this flow.
    pub fn holds(&self, column: &str) -> bool {
        self.columns.iter().any(|known| known.id == column)
    }

    /// The column a new card takes: never `accept.to`.
    pub fn first_column(&self) -> &str {
        self.columns
            .first()
            .map_or(self.accept.from.as_str(), |column| column.id.as_str())
    }

    /// `column` is this flow's `return` column.
    pub fn is_return(&self, column: &str) -> bool {
        self.return_column.as_deref() == Some(column)
    }
}

/// `[card]`, checked: at least one flow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CardConfig {
    pub flows: Vec<CardFlow>,
}

impl CardConfig {
    /// Reads `[card]` from `text` (see the module documentation).
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        card_from_toml(text)
    }

    /// The flow named `name`.
    pub fn flow(&self, name: &str) -> Option<&CardFlow> {
        self.flows.iter().find(|flow| flow.name == name)
    }

    /// The flow a new card of `discipline` takes: the first listing it,
    /// else the one flow listing none; else why there is none.
    pub fn flow_for(&self, discipline: Option<&str>) -> Result<&CardFlow, String> {
        let listing = discipline.and_then(|discipline| {
            self.flows
                .iter()
                .find(|flow| flow.disciplines.iter().any(|known| known == discipline))
        });
        if let Some(flow) = listing {
            return Ok(flow);
        }
        let open: Vec<&CardFlow> = self
            .flows
            .iter()
            .filter(|flow| flow.disciplines.is_empty())
            .collect();
        let asked = discipline.map_or_else(
            || "a card without a discipline".to_owned(),
            |discipline| format!("the discipline `{discipline}`"),
        );
        match open.as_slice() {
            [flow] => Ok(flow),
            [] => Err(format!(
                "no flow takes {asked}: none lists it, and every `[[card.flow]]` lists its \
                 disciplines"
            )),
            many => Err(format!(
                "no flow takes {asked}: none lists it, and {} flows list no discipline",
                many.len()
            )),
        }
    }
}

/// Why `name` is no name: `[a-z0-9_-]`, 1 to [`MAX_NAME_BYTES`] bytes.
pub fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return Some(format!(
            "`{name}` is {} bytes long; a name has 1 to {MAX_NAME_BYTES}",
            name.len()
        ));
    }
    if !name.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    }) {
        return Some(format!(
            "`{name}` is not a name: lower-case ASCII letters, digits, `_` and `-`"
        ));
    }
    None
}

/// [`CardConfig::from_toml`] as a function.
pub fn card_from_toml(text: &str) -> Result<CardConfig, ProjectError> {
    let root = DeTable::parse(text).map_err(|error| ProjectError {
        line: error.span().map(|span| line_of(text, span.start)),
        message: error.message().trim().to_owned(),
    })?;
    let mut errors = Errors::new(text);
    let mut flows: Vec<CardFlow> = Vec::new();
    let table = root
        .get_ref()
        .iter()
        .find(|(key, _)| key.get_ref().as_ref() == TABLE);
    if let Some((_, table)) = table {
        match table.get_ref() {
            DeValue::Table(entries) => read_card(entries, &mut flows, &mut errors),
            other => errors.push(
                table.span(),
                format!("`[{TABLE}]` is a table, not {}", type_name(other)),
            ),
        }
    }
    if let Some(error) = errors.first() {
        return Err(error);
    }
    if flows.is_empty() {
        return card_from_toml(PLAIN_FLOW);
    }
    Ok(CardConfig { flows })
}

/// `[card]`'s keys, each flow pushed or its error.
fn read_card(entries: &DeTable<'_>, flows: &mut Vec<CardFlow>, errors: &mut Errors<'_>) {
    for (key, value) in entries.iter() {
        let name = key.get_ref().as_ref();
        if name != CARD_KEYS[0] {
            errors.push(key.span(), unknown_field(name, &CARD_KEYS));
            continue;
        }
        let DeValue::Array(items) = value.get_ref() else {
            errors.push(
                value.span(),
                format!(
                    "`[{TABLE}] {name}` is an array of tables ({FLOW}), not {}",
                    type_name(value.get_ref())
                ),
            );
            continue;
        };
        let mut names = BTreeSet::new();
        for (at, item) in items.iter().enumerate() {
            if at == MAX_FLOWS {
                errors.push(item.span(), format!("at most {MAX_FLOWS} {FLOW} tables"));
                break;
            }
            let DeValue::Table(entry) = item.get_ref() else {
                errors.push(
                    item.span(),
                    format!("{FLOW} is a table, not {}", type_name(item.get_ref())),
                );
                continue;
            };
            let Some(flow) = read_flow(item.span(), entry, errors) else {
                continue;
            };
            if names.insert(flow.name.clone()) {
                flows.push(flow);
            } else {
                errors.push(
                    item.span(),
                    format!("{FLOW} `{}` is named twice", flow.name),
                );
            }
        }
    }
}

/// One flow's keys, as written.
#[derive(Default)]
struct Written<'v, 'i> {
    name: Option<String>,
    disciplines: Vec<String>,
    columns: Option<&'v Spanned<DeValue<'i>>>,
    titles: Option<&'v Spanned<DeValue<'i>>>,
    accept: Option<&'v Spanned<DeValue<'i>>>,
    return_column: Option<&'v Spanned<DeValue<'i>>>,
    moves: Option<&'v Spanned<DeValue<'i>>>,
}

/// One flow checked; `None` when it holds an error (pushed).
fn read_flow(span: Range<usize>, entry: &DeTable<'_>, errors: &mut Errors<'_>) -> Option<CardFlow> {
    let before = errors.len();
    let mut written = Written::default();
    for (key, value) in entry.iter() {
        match key.get_ref().as_ref() {
            "name" => {
                if let Some(name) = text("name", value, errors) {
                    if let Some(problem) = name_problem(name) {
                        errors.push(value.span(), format!("`name`: {problem}"));
                    }
                    written.name = Some(name.to_owned());
                }
            }
            "disciplines" => {
                written.disciplines = names("disciplines", value, errors)
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect();
            }
            "columns" => written.columns = Some(value),
            "titles" => written.titles = Some(value),
            "accept" => written.accept = Some(value),
            "return" => written.return_column = Some(value),
            "moves" => written.moves = Some(value),
            other => errors.push(key.span(), unknown_field(other, &FLOW_KEYS)),
        }
    }
    let Some(name) = written.name else {
        errors.push(span, format!("{FLOW} has no `name`"));
        return None;
    };
    let columns = read_columns(&span, written.columns, errors);
    let accept = read_accept(&span, written.accept, &columns, errors);
    let titles = read_titles(written.titles, &columns, errors);
    let return_column = read_return(written.return_column, &columns, accept.as_ref(), errors);
    let moves = read_moves(written.moves, &columns, accept.as_ref(), errors);
    if errors.len() != before {
        return None;
    }
    let accept = accept?;
    let moves = moves.unwrap_or_else(|| every_move(&columns, &accept));
    let columns = columns
        .into_iter()
        .map(|id| CardColumn {
            title: titles
                .iter()
                .find(|(known, _)| *known == id)
                .map_or_else(|| id.clone(), |(_, title)| title.clone()),
            id,
        })
        .collect();
    Some(CardFlow {
        name,
        disciplines: written.disciplines,
        columns,
        moves,
        accept,
        return_column,
    })
}

/// The string `value`, else its error (`None`).
fn text<'v>(
    what: &str,
    value: &'v Spanned<DeValue<'_>>,
    errors: &mut Errors<'_>,
) -> Option<&'v str> {
    match value.get_ref() {
        DeValue::String(text) => Some(text.as_ref()),
        other => {
            errors.push(
                value.span(),
                format!("`{what}` is a string, not {}", type_name(other)),
            );
            None
        }
    }
}

/// The list of strings `value`, each with its span, else its error (none).
fn strings(
    what: &str,
    value: &Spanned<DeValue<'_>>,
    errors: &mut Errors<'_>,
) -> Vec<(String, Range<usize>)> {
    let DeValue::Array(items) = value.get_ref() else {
        errors.push(
            value.span(),
            format!(
                "`{what}` is a list of strings, not {}",
                type_name(value.get_ref())
            ),
        );
        return Vec::new();
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        if let Some(text) = text(what, item, errors) {
            out.push((text.to_owned(), item.span()));
        }
    }
    out
}

/// The names `value` lists, each once (the rest: an error each).
fn names(
    what: &str,
    value: &Spanned<DeValue<'_>>,
    errors: &mut Errors<'_>,
) -> Vec<(String, Range<usize>)> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (name, span) in strings(what, value, errors) {
        if let Some(problem) = name_problem(&name) {
            errors.push(span, format!("`{what}`: {problem}"));
        } else if seen.insert(name.clone()) {
            out.push((name, span));
        } else {
            errors.push(span, format!("`{what}`: `{name}` is listed twice"));
        }
    }
    out
}

fn read_columns(
    span: &Range<usize>,
    value: Option<&Spanned<DeValue<'_>>>,
    errors: &mut Errors<'_>,
) -> Vec<String> {
    let Some(value) = value else {
        errors.push(span.clone(), format!("{FLOW} has no `columns`"));
        return Vec::new();
    };
    let columns: Vec<String> = names("columns", value, errors)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    if !(MIN_COLUMNS..=MAX_COLUMNS).contains(&columns.len()) {
        errors.push(
            value.span(),
            format!(
                "`columns`: {} listed; a flow has {MIN_COLUMNS} to {MAX_COLUMNS}",
                columns.len()
            ),
        );
    }
    columns
}

/// The string under `key` of the table `entry`, a column of `columns`.
fn column_of<'v>(
    what: &str,
    key: &str,
    span: &Range<usize>,
    entry: &'v DeTable<'_>,
    columns: &[String],
    errors: &mut Errors<'_>,
) -> Option<&'v str> {
    let Some((_, value)) = entry
        .iter()
        .find(|(name, _)| name.get_ref().as_ref() == key)
    else {
        errors.push(span.clone(), format!("`{what}` has no `{key}`"));
        return None;
    };
    let column = text(&format!("{what}.{key}"), value, errors)?;
    if !columns.iter().any(|known| known == column) {
        errors.push(
            value.span(),
            format!("`{what}.{key}`: `{column}` is no column of this flow"),
        );
        return None;
    }
    Some(column)
}

fn read_accept(
    span: &Range<usize>,
    value: Option<&Spanned<DeValue<'_>>>,
    columns: &[String],
    errors: &mut Errors<'_>,
) -> Option<CardAccept> {
    let Some(value) = value else {
        errors.push(span.clone(), format!("{FLOW} has no `accept`"));
        return None;
    };
    let DeValue::Table(entry) = value.get_ref() else {
        errors.push(
            value.span(),
            format!("`accept` is a table, not {}", type_name(value.get_ref())),
        );
        return None;
    };
    for (key, _) in entry.iter() {
        let name = key.get_ref().as_ref();
        if !ACCEPT_KEYS.contains(&name) {
            errors.push(key.span(), unknown_field(name, &ACCEPT_KEYS));
        }
    }
    let at = value.span();
    let from = column_of("accept", "from", &at, entry, columns, errors);
    let to = column_of("accept", "to", &at, entry, columns, errors);
    let (from, to) = (from?, to?);
    if from == to {
        errors.push(
            at,
            format!("`accept`: `from` and `to` are both `{from}`; it takes two columns"),
        );
        return None;
    }
    if columns.first().is_some_and(|first| first == to) {
        errors.push(
            at,
            format!(
                "`accept.to`: `{to}` is the first column, where a new card starts: accept alone \
                 reaches `to`"
            ),
        );
        return None;
    }
    Some(CardAccept {
        from: from.to_owned(),
        to: to.to_owned(),
    })
}

/// `return`, when written: one column of this flow, never the first, where
/// a new card starts, nor `accept.from` or `accept.to` (`None` when `accept`
/// itself holds an error, already pushed).
fn read_return(
    value: Option<&Spanned<DeValue<'_>>>,
    columns: &[String],
    accept: Option<&CardAccept>,
    errors: &mut Errors<'_>,
) -> Option<String> {
    let value = value?;
    let column = text("return", value, errors)?;
    let problem = if !columns.iter().any(|known| known == column) {
        Some("is no column of this flow")
    } else if columns.first().is_some_and(|first| first == column) {
        Some("is the first column, where a new card starts")
    } else if accept.is_some_and(|accept| accept.from == column) {
        Some("is `accept.from`, where a card waits for its accept")
    } else if accept.is_some_and(|accept| accept.to == column) {
        Some("is `accept.to`, reached only by an accept")
    } else {
        None
    };
    if let Some(problem) = problem {
        errors.push(value.span(), format!("`return`: `{column}` {problem}"));
        return None;
    }
    Some(column.to_owned())
}

fn read_titles(
    value: Option<&Spanned<DeValue<'_>>>,
    columns: &[String],
    errors: &mut Errors<'_>,
) -> Vec<(String, String)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let DeValue::Table(entry) = value.get_ref() else {
        errors.push(
            value.span(),
            format!("`titles` is a table, not {}", type_name(value.get_ref())),
        );
        return Vec::new();
    };
    let mut titles = Vec::with_capacity(entry.len());
    for (key, title) in entry.iter() {
        let id = key.get_ref().as_ref();
        if !columns.iter().any(|known| known == id) {
            errors.push(
                key.span(),
                format!("`titles`: `{id}` is no column of this flow"),
            );
            continue;
        }
        let Some(written) = text(&format!("titles.{id}"), title, errors) else {
            continue;
        };
        if written.len() > MAX_TITLE_BYTES {
            errors.push(
                title.span(),
                format!(
                    "`titles.{id}`: {} bytes; at most {MAX_TITLE_BYTES}",
                    written.len()
                ),
            );
            continue;
        }
        titles.push((id.to_owned(), written.to_owned()));
    }
    titles
}

/// `moves` checked; `None` when absent or empty: every move stands.
fn read_moves(
    value: Option<&Spanned<DeValue<'_>>>,
    columns: &[String],
    accept: Option<&CardAccept>,
    errors: &mut Errors<'_>,
) -> Option<Vec<CardMove>> {
    let value = value?;
    let DeValue::Array(items) = value.get_ref() else {
        errors.push(
            value.span(),
            format!("`moves` is a list, not {}", type_name(value.get_ref())),
        );
        return None;
    };
    let mut moves: Vec<CardMove> = Vec::with_capacity(items.len());
    for item in items.iter() {
        let DeValue::Table(entry) = item.get_ref() else {
            errors.push(
                item.span(),
                format!("a move is a table, not {}", type_name(item.get_ref())),
            );
            continue;
        };
        let Some(listed) = read_move(item, entry, columns, accept, errors) else {
            continue;
        };
        if moves
            .iter()
            .any(|known| known.from == listed.from && known.to == listed.to)
        {
            errors.push(
                item.span(),
                format!(
                    "`moves`: `{}` to `{}` is listed twice",
                    listed.from, listed.to
                ),
            );
            continue;
        }
        moves.push(listed);
    }
    (!moves.is_empty()).then_some(moves)
}

fn read_move(
    item: &Spanned<DeValue<'_>>,
    entry: &DeTable<'_>,
    columns: &[String],
    accept: Option<&CardAccept>,
    errors: &mut Errors<'_>,
) -> Option<CardMove> {
    let at = item.span();
    let mut by = Vec::new();
    for (key, value) in entry.iter() {
        let name = key.get_ref().as_ref();
        if name == MOVE_KEYS[2] {
            by = read_by(value, errors);
        } else if !MOVE_KEYS.contains(&name) {
            errors.push(key.span(), unknown_field(name, &MOVE_KEYS));
        }
    }
    let from = column_of("moves", "from", &at, entry, columns, errors);
    let to = column_of("moves", "to", &at, entry, columns, errors);
    let (from, to) = (from?, to?);
    if from == to {
        errors.push(
            at,
            format!("`moves`: `from` and `to` are both `{from}`; a move joins two columns"),
        );
        return None;
    }
    if let Some(accept) = accept
        && (from == accept.to || to == accept.to)
    {
        errors.push(
            at,
            format!(
                "`moves`: `{}` is `accept.to`, which accept alone reaches: no move starts or \
                 ends there",
                accept.to
            ),
        );
        return None;
    }
    Some(CardMove {
        from: from.to_owned(),
        to: to.to_owned(),
        by,
    })
}

/// `by`: at least one name, each as `--as` writes it.
fn read_by(value: &Spanned<DeValue<'_>>, errors: &mut Errors<'_>) -> Vec<String> {
    let listed = strings("by", value, errors);
    if listed.is_empty() {
        errors.push(
            value.span(),
            "`by`: name at least one, or leave it out: no `by` is anyone".to_owned(),
        );
    }
    let mut by = Vec::with_capacity(listed.len());
    for (name, span) in listed {
        match card_person_problem(&name) {
            Some(problem) => errors.push(span, format!("`by`: {problem}")),
            None => by.push(card_person(&name).to_owned()),
        }
    }
    by
}

/// No `moves`: every ordered pair of the columns but `accept.to`.
fn every_move(columns: &[String], accept: &CardAccept) -> Vec<CardMove> {
    let open: Vec<&String> = columns
        .iter()
        .filter(|column| **column != accept.to)
        .collect();
    let mut moves = Vec::with_capacity(open.len() * open.len().saturating_sub(1));
    for from in &open {
        for to in &open {
            if from != to {
                moves.push(CardMove {
                    from: (*from).clone(),
                    to: (*to).clone(),
                    by: Vec::new(),
                });
            }
        }
    }
    moves
}
