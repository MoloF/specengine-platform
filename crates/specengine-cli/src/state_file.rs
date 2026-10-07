//! The queue's dump, format 1 (canon `queue-backup`, "Format"): UTF-8
//! compact JSON, one object per LF-ended line. The header
//! `{"format":1,"queue_schema":3,"project":"<slug>","proposals":<p>,"events":<e>}`
//! (keys in this order; the counts of the rows below it), then every
//! `proposals` row by ID number and every `events` row by `seq`, each
//! `{"<table>":{…}}` with every column in table order: `TEXT` a string,
//! `NULL` `null`, `seq` a number; `author`, `diagnostics`, `payload` stay
//! the strings stored. No export time and no host inside: equal queues give
//! equal bytes. A dump of queue schema 1 or 2 (canon `queue-backup`,
//! "Format"; canon `decision-record`, "Queue and documents") still
//! restores: its rows hold that schema's 24 or 35 columns, the later ones
//! restored `NULL`.
//!
//! [`render`] writes it; [`parse`] reads a whole file back, refusing its
//! first defect as `<FILE>:<line>: <defect>` without ever quoting the line's
//! text (a dump holds proposal texts, paths and identities).

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write as _};

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use specengine_core::proposal::proposal_number;
use specengine_store::{
    EVENT_COLUMNS, PROPOSAL_COLUMNS, QUEUE_SCHEMA_VERSION, StoredEvent, StoredProposal,
    StoredQueue, proposal_columns,
};

use crate::CliError;

/// The dump's format: the header's `format`.
pub const STATE_FORMAT: u64 = 1;

/// The header's keys, in the order written.
const HEADER_KEYS: [&str; 5] = ["format", "queue_schema", "project", "proposals", "events"];

/// The dump of `state`, the queue of `slug` (see the module documentation):
/// every string escaped by `serde_json` straight into the buffer, no text
/// copied first.
pub(crate) fn render(slug: &str, state: &StoredQueue) -> io::Result<Vec<u8>> {
    let counts = state.counts();
    let mut out = Vec::new();
    write!(
        out,
        "{{\"format\":{STATE_FORMAT},\"queue_schema\":{QUEUE_SCHEMA_VERSION},\"project\":"
    )?;
    serde_json::to_writer(&mut out, slug)?;
    writeln!(
        out,
        ",\"proposals\":{},\"events\":{}}}",
        counts.proposals, counts.events
    )?;
    for row in &state.proposals {
        out.extend_from_slice(b"{\"proposals\":{");
        for (index, (column, value)) in PROPOSAL_COLUMNS.iter().zip(&row.columns).enumerate() {
            if index > 0 {
                out.push(b',');
            }
            push_column(&mut out, column, value.as_deref())?;
        }
        out.extend_from_slice(b"}}\n");
    }
    for row in &state.events {
        write!(out, "{{\"events\":{{\"seq\":{}", row.seq)?;
        for (column, value) in EVENT_COLUMNS[1..].iter().zip(&row.columns) {
            out.push(b',');
            push_column(&mut out, column, value.as_deref())?;
        }
        out.extend_from_slice(b"}}\n");
    }
    Ok(out)
}

/// `"<column>":<value>`: a string, or `null`.
fn push_column(out: &mut Vec<u8>, column: &str, value: Option<&str>) -> io::Result<()> {
    serde_json::to_writer(&mut *out, column)?;
    out.push(b':');
    serde_json::to_writer(&mut *out, &value)?;
    Ok(())
}

/// The rows of the dump `bytes` (the file `label` as given), checked whole
/// against this build and the root's `slug` (canon `queue-backup`,
/// "Import", step 2): `Err` is exit 2 naming the file and, but for the
/// counts, the line.
pub(crate) fn parse(label: &str, bytes: &[u8], slug: &str) -> Result<StoredQueue, CliError> {
    let at = |line: usize, defect: &str| CliError::cannot(format!("{label}:{line}: {defect}"));
    if bytes.is_empty() {
        return Err(at(
            1,
            "the file is empty: a dump holds at least its header line",
        ));
    }
    let Some(body) = bytes.strip_suffix(b"\n") else {
        let last = bytes.iter().filter(|&&byte| byte == b'\n').count() + 1;
        return Err(at(
            last,
            "the last line has no line end (LF): the file is cut short",
        ));
    };
    let mut header: Option<Header> = None;
    let mut state = StoredQueue::default();
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut seqs: HashMap<i64, usize> = HashMap::new();
    for (index, line) in body.split(|&byte| byte == b'\n').enumerate() {
        let number = index + 1;
        let value = line_value(line).map_err(|defect| at(number, defect))?;
        let Some(header) = &header else {
            header = Some(read_header(value, slug).map_err(|defect| at(number, &defect))?);
            continue;
        };
        match read_row(value, &header.project, header.columns)
            .map_err(|defect| at(number, &defect))?
        {
            Row::Proposal(row) => {
                // `read_row` took only an ID as the queue writes it.
                let id = row.id().unwrap_or_default().to_owned();
                if let Some(first) = ids.insert(id, number) {
                    return Err(at(
                        number,
                        &format!("`id` repeats the `proposals` row of line {first}"),
                    ));
                }
                state.proposals.push(*row);
            }
            Row::Event(row) => {
                if let Some(first) = seqs.insert(row.seq, number) {
                    return Err(at(
                        number,
                        &format!("`seq` repeats the `events` row of line {first}"),
                    ));
                }
                state.events.push(row);
            }
        }
    }
    // `body` has at least one line: the header was read or refused.
    let Some(header) = header else {
        return Err(at(1, "no header line"));
    };
    let found = state.counts();
    if found.proposals != header.proposals || found.events != header.events {
        return Err(CliError::cannot(format!(
            "{label}: header counts {}, {}; found {}, {}",
            header.proposals, header.events, found.proposals, found.events
        )));
    }
    Ok(state)
}

/// The header's values that the rows are checked against.
struct Header {
    project: String,
    /// The `proposals` columns of its queue schema.
    columns: &'static [&'static str],
    proposals: u64,
    events: u64,
}

/// A parsed row.
enum Row {
    Proposal(Box<StoredProposal>),
    Event(StoredEvent),
}

/// One line as a JSON object (its keys in order, repeats kept), or its
/// defect.
fn line_value(line: &[u8]) -> Result<Vec<(String, Json)>, &'static str> {
    if line.is_empty() {
        return Err("an empty line");
    }
    let text = std::str::from_utf8(line).map_err(|_| "not UTF-8")?;
    match serde_json::from_str::<Json>(text) {
        Ok(Json::Object(entries)) => Ok(entries),
        Ok(_) => Err("not a JSON object"),
        Err(_) => Err("not JSON"),
    }
}

/// The header line: `format` first (a newer build's header may differ in
/// all else), then exactly the five keys, `queue_schema`, `project` the
/// root's slug, the counts.
fn read_header(entries: Vec<(String, Json)>, slug: &str) -> Result<Header, String> {
    let named = |key: &str| {
        entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    };
    let Some(format) = named("format") else {
        return Err(
            "no header: a dump's first line is `{\"format\":…,\"queue_schema\":…,\"project\":…,\
             \"proposals\":…,\"events\":…}`"
                .to_owned(),
        );
    };
    let Json::Count(format) = *format else {
        return Err("the header's `format` is not an integer".to_owned());
    };
    if format > STATE_FORMAT {
        return Err(format!(
            "the dump is of format {format}, a newer SpecEngine's (this build reads format \
             {STATE_FORMAT}): upgrade SpecEngine"
        ));
    }
    if format != STATE_FORMAT {
        return Err(format!(
            "the dump is of format {format}, which this build does not read (it reads format \
             {STATE_FORMAT})"
        ));
    }
    let mut keys: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    keys.sort_unstable();
    let mut expected = HEADER_KEYS;
    expected.sort_unstable();
    if keys != expected {
        return Err(format!(
            "the header's keys are not exactly {}",
            HEADER_KEYS
                .iter()
                .map(|key| format!("`{key}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let schema = match named("queue_schema") {
        Some(Json::Count(schema)) => i64::try_from(*schema).unwrap_or(i64::MAX),
        _ => return Err("the header's `queue_schema` is not an integer".to_owned()),
    };
    if schema > QUEUE_SCHEMA_VERSION {
        return Err(format!(
            "the dump's queue schema is {schema}, a newer SpecEngine's (this build's is \
             {QUEUE_SCHEMA_VERSION}): upgrade SpecEngine"
        ));
    }
    let Some(columns) = proposal_columns(schema) else {
        return Err(format!(
            "the dump's queue schema is {schema}, which this build does not restore (it \
             restores queue schemas 1 to {QUEUE_SCHEMA_VERSION})"
        ));
    };
    let Some(Json::String(project)) = named("project") else {
        return Err("the header's `project` is not a string".to_owned());
    };
    if project != slug {
        return Err(format!(
            "the dump is of the project `{project}`, and this root's slug is `{slug}`: \
             import-state restores a project's own queue only"
        ));
    }
    let count = |key: &str| match named(key) {
        Some(Json::Count(count)) => Ok(*count),
        _ => Err(format!(
            "the header's `{key}` is not a count (an integer ≥ 0)"
        )),
    };
    Ok(Header {
        project: project.clone(),
        columns,
        proposals: count("proposals")?,
        events: count("events")?,
    })
}

/// A row line: `{"proposals":{…}}` or `{"events":{…}}`, every column of
/// the table (`proposals`: of the header's queue schema) once and no other,
/// `TEXT` a string or `null`, `seq` an integer ≥ 1, `id` as the queue writes
/// it, `project` the header's.
fn read_row(
    mut entries: Vec<(String, Json)>,
    project: &str,
    proposal_columns: &[&str],
) -> Result<Row, String> {
    const SHAPE: &str = "not a row `{\"proposals\":{…}}` or `{\"events\":{…}}`";
    if entries.len() != 1 {
        return Err(SHAPE.to_owned());
    }
    let (table, value) = entries.remove(0);
    let columns: &[&str] = match table.as_str() {
        "proposals" => proposal_columns,
        "events" => &EVENT_COLUMNS,
        _ => {
            return Err(
                "a row of an unknown table (a dump holds `proposals` and `events` rows)".to_owned(),
            );
        }
    };
    let Json::Object(fields) = value else {
        return Err(SHAPE.to_owned());
    };
    let mut values: Vec<Option<Json>> = columns.iter().map(|_| None).collect();
    for (name, value) in fields {
        let Some(index) = columns.iter().position(|column| *column == name) else {
            return Err(format!(
                "the `{table}` row has a column the table does not have"
            ));
        };
        if values[index].replace(value).is_some() {
            return Err(format!("the `{table}` row repeats `{}`", columns[index]));
        }
    }
    let mut taken = Vec::with_capacity(columns.len());
    for (column, value) in columns.iter().zip(values) {
        let Some(value) = value else {
            return Err(format!("the `{table}` row has no `{column}`"));
        };
        taken.push((*column, value));
    }
    if table == "events" {
        let mut taken = taken.into_iter();
        let seq = match taken.next() {
            Some((_, Json::Count(seq))) if seq >= 1 => i64::try_from(seq).ok(),
            _ => None,
        }
        .ok_or_else(|| "`seq` is not an integer ≥ 1".to_owned())?;
        let mut columns: [Option<String>; EVENT_COLUMNS.len() - 1] = Default::default();
        for (slot, (column, value)) in columns.iter_mut().zip(taken) {
            *slot = text(column, value)?;
        }
        let row = StoredEvent { seq, columns };
        if row.project() != Some(project) {
            return Err(format!("`project` is not the header's (`{project}`)"));
        }
        return Ok(Row::Event(row));
    }
    let mut columns: [Option<String>; PROPOSAL_COLUMNS.len()] = std::array::from_fn(|_| None);
    for (slot, (column, value)) in columns.iter_mut().zip(taken) {
        *slot = text(column, value)?;
    }
    let row = StoredProposal { columns };
    // The queue numbers from 1 and gives the next proposal the highest
    // number plus one, up to `u64::MAX` itself: `PR-0000` is never written,
    // and every number the queue can write restores (after `u64::MAX` the
    // restored queue refuses `propose` exactly as the original one did).
    let number = row.id().and_then(proposal_number);
    if !number.is_some_and(|number| number >= 1) {
        return Err(format!(
            "`id` is not a proposal ID as the queue writes it (`PR-` and 4 or more digits, \
             numbered from 1 to {})",
            u64::MAX
        ));
    }
    if row.project() != Some(project) {
        return Err(format!("`project` is not the header's (`{project}`)"));
    }
    Ok(Row::Proposal(Box::new(row)))
}

/// A `TEXT` column's value: a string or `null`.
fn text(column: &str, value: Json) -> Result<Option<String>, String> {
    match value {
        Json::String(text) => Ok(Some(text)),
        Json::Null => Ok(None),
        _ => Err(format!("`{column}` is not a string or null")),
    }
}

/// A JSON value as written: an object keeps its keys in order, a repeated
/// key included (`serde_json::Value` would keep only the last).
enum Json {
    Null,
    /// An integer ≥ 0 that fits 64 bits.
    Count(u64),
    String(String),
    Object(Vec<(String, Json)>),
    /// A boolean, a negative or fractional number, an array.
    Other,
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Json, E> {
        Ok(Json::Other)
    }

    fn visit_u64<E: de::Error>(self, number: u64) -> Result<Json, E> {
        Ok(Json::Count(number))
    }

    fn visit_i64<E: de::Error>(self, number: i64) -> Result<Json, E> {
        Ok(u64::try_from(number).map_or(Json::Other, Json::Count))
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Json, E> {
        Ok(Json::Other)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Json, E> {
        Ok(Json::String(text.to_owned()))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<Json, E> {
        Ok(Json::String(text))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        while seq.next_element::<Json>()?.is_some() {}
        Ok(Json::Other)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value::<Json>()?;
            entries.push((key, value));
        }
        Ok(Json::Object(entries))
    }
}
