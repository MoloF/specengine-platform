//! The queue's dump, format 7
//! (canon `queue-backup`, "Format", "Import";
//! cards: canon `cards`, "Backup"; tasks: canon `tasks`, "Backup"; a
//! proposal's card and why: canon `card-questions`, "The rule"; a card's
//! mandate: canon `card-mandate`, "The rule"; a question's gist and
//! matters: task spec `plain-questions` PQ8): UTF-8
//! compact JSON, one object per LF-ended line. The header
//! `{"format":7,"queue_schema":14,"project":"<slug>","proposals":<p>,"tasks":<t>,"runs":<r>,"cards":<c>,"events":<e>}`
//! (keys in this order; the counts of the rows below it), then every
//! `proposals` row by ID number, every `tasks` row by ID number, every
//! `runs` row by task and number, every `cards` row by ID number, every
//! `events` row by `seq`, each `{"<table>":{…}}` with every column in table
//! order: `TEXT` a string, `NULL` `null`, `run`, `position` and `seq`
//! numbers; JSON columns stay the strings stored. No export time and no host
//! inside: equal queues give equal bytes. Format 7 holds queue schema 13 or
//! 14, its proposals' 50 columns, `gist` and `matters` last; step 14 adds
//! only a local cache no dump holds (task spec `collision-pass` CP6), so a
//! schema 14 dump has a schema 13 one's tables and either restores into an
//! empty cache. A format-6 dump
//! holds queue schema 12 and still restores, its proposals' 48 columns,
//! `gist` and `matters` restored `NULL`; its cards' 26 columns, `mandate`
//! last. A format-5 dump holds queue
//! schema 11 and still restores, its cards' 25 columns, `mandate` restored
//! `NULL`; its proposals' 48 columns, `card_id` and `why` last. A format-4 dump
//! holds queue schema 10 and still restores, its proposals' 46 columns,
//! `card_id` and `why` restored `NULL`; its cards' 25 columns, `tasks`
//! last. A format-3 dump (eight header keys)
//! holds queue schema 9 and still restores, its cards' 24 columns, `tasks`
//! restored `NULL`. A format-1 dump (five header keys, queue schema 1, 2 or 3,
//! `proposals` and `events` rows only) still restores: its rows hold that
//! schema's 24, 35 or 40 columns, the later ones restored `NULL`; format 2
//! (seven header keys, no cards) holds queue schema 4 (41 columns, its stages restored `NULL`), 5 (43, a
//! proposal's `staged`, `staged_at` with them: canon `decision-staging`,
//! "Documents"; its `interpretation` restored `NULL`), 6 (44, an
//! interpretation's `interpretation` with them: task spec
//! `kind-interpretation`; its open approve stages restored in schema 7's
//! shape), 7 (44, an approve stage's `editorial`: task spec
//! `rev-discipline`; its `drift`, `drift_hash` restored `NULL`) or 8 (46, a
//! drift item's `drift` and `drift_hash`: task spec `drift-queue`).
//!
//! [`render`] writes it; [`parse`] reads a whole file back, refusing its
//! first defect as `<FILE>:<line>: <defect>` without ever quoting the line's
//! text (a dump holds proposal texts, paths and identities).

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write as _};

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use specengine_core::card::card_number;
use specengine_core::proposal::proposal_number;
use specengine_core::task::task_number;
use specengine_store::{
    CARD_COLUMNS, EVENT_COLUMNS, PROPOSAL_COLUMNS, QUEUE_SCHEMA_VERSION, RUN_COLUMNS, StoredCard,
    StoredEvent, StoredProposal, StoredQueue, StoredRun, StoredTask, TASK_COLUMNS, card_columns,
    proposal_columns,
};

use crate::CliError;

/// The dump's format: the header's `format`.
pub const STATE_FORMAT: u64 = 7;

/// The oldest format this build restores: 1, proposals and events only.
const PROPOSALS_ONLY_FORMAT: u64 = 1;

/// The format of proposals, tasks, runs and events, no cards.
const NO_CARDS_FORMAT: u64 = 2;

/// The format of cards without their linked tasks: queue schema 9 alone.
const NO_TASK_LINKS_FORMAT: u64 = 3;

/// The format of proposals naming no card: queue schema 10 alone.
const NO_CARD_ITEMS_FORMAT: u64 = 4;

/// The format of cards without a mandate: queue schema 11 alone.
const NO_MANDATE_FORMAT: u64 = 5;

/// The format of proposals without a gist or matters: queue schema 12
/// alone.
const NO_PLAIN_QUESTIONS_FORMAT: u64 = 6;

/// The first queue schema with tasks and runs: format 1 holds the schemas
/// before it, format 2 it and those after, to [`CARDS_SCHEMA`].
const TASKS_SCHEMA: i64 = 4;

/// The first queue schema with cards: format 3 holds it alone.
const CARDS_SCHEMA: i64 = 9;

/// The first queue schema whose cards link tasks: format 4 holds it alone.
const TASK_LINKS_SCHEMA: i64 = 10;

/// The first queue schema whose proposals hold `card_id` and `why`: format
/// 5 holds it alone.
const CARD_ITEMS_SCHEMA: i64 = 11;

/// The first queue schema whose cards hold `mandate`: format 6 holds it
/// alone.
const MANDATE_SCHEMA: i64 = 12;

/// The first queue schema whose proposals hold `gist` and `matters`:
/// format 7 holds it, to this build's (14: step 14 changes no dumped
/// table).
const PLAIN_QUESTIONS_SCHEMA: i64 = 13;

/// The header's keys, in the order written.
const HEADER_KEYS: [&str; 8] = [
    "format",
    "queue_schema",
    "project",
    "proposals",
    "tasks",
    "runs",
    "cards",
    "events",
];

/// A format-2 header's keys.
const HEADER_KEYS_2: [&str; 7] = [
    "format",
    "queue_schema",
    "project",
    "proposals",
    "tasks",
    "runs",
    "events",
];

/// A format-1 header's keys.
const HEADER_KEYS_1: [&str; 5] = ["format", "queue_schema", "project", "proposals", "events"];

/// The one `cards` column that is no `TEXT`.
const POSITION: &str = "position";

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
        ",\"proposals\":{},\"tasks\":{},\"runs\":{},\"cards\":{},\"events\":{}}}",
        counts.proposals, counts.tasks, counts.runs, counts.cards, counts.events
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
    for row in &state.tasks {
        out.extend_from_slice(b"{\"tasks\":{");
        for (index, (column, value)) in TASK_COLUMNS.iter().zip(&row.columns).enumerate() {
            if index > 0 {
                out.push(b',');
            }
            push_column(&mut out, column, value.as_deref())?;
        }
        out.extend_from_slice(b"}}\n");
    }
    for row in &state.runs {
        let [task_id, rest @ ..] = &row.columns;
        out.extend_from_slice(b"{\"runs\":{");
        push_column(&mut out, RUN_COLUMNS[0], task_id.as_deref())?;
        write!(out, ",\"run\":{}", row.run)?;
        for (column, value) in RUN_COLUMNS[2..].iter().zip(rest) {
            out.push(b',');
            push_column(&mut out, column, value.as_deref())?;
        }
        out.extend_from_slice(b"}}\n");
    }
    for row in &state.cards {
        out.extend_from_slice(b"{\"cards\":{");
        let mut texts = row.columns.iter();
        for (index, column) in CARD_COLUMNS.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            if *column == POSITION {
                write!(out, "\"{POSITION}\":")?;
                serde_json::to_writer(&mut out, &row.position)?;
            } else {
                let value = texts.next().and_then(Option::as_deref);
                push_column(&mut out, column, value)?;
            }
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
    let mut task_ids: HashMap<String, usize> = HashMap::new();
    let mut card_ids: HashMap<String, usize> = HashMap::new();
    let mut run_keys: HashMap<(String, i64), usize> = HashMap::new();
    let mut run_lines: Vec<usize> = Vec::new();
    let mut seqs: HashMap<i64, usize> = HashMap::new();
    for (index, line) in body.split(|&byte| byte == b'\n').enumerate() {
        let number = index + 1;
        let value = line_value(line).map_err(|defect| at(number, defect))?;
        let Some(header) = &header else {
            header = Some(read_header(value, slug).map_err(|defect| at(number, &defect))?);
            continue;
        };
        match read_row(value, header).map_err(|defect| at(number, &defect))? {
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
            Row::Task(row) => {
                let id = row.id().unwrap_or_default().to_owned();
                if let Some(first) = task_ids.insert(id, number) {
                    return Err(at(
                        number,
                        &format!("`id` repeats the `tasks` row of line {first}"),
                    ));
                }
                state.tasks.push(*row);
            }
            Row::Run(row) => {
                let key = (row.task_id().unwrap_or_default().to_owned(), row.run);
                if let Some(first) = run_keys.insert(key, number) {
                    return Err(at(
                        number,
                        &format!("`task_id` and `run` repeat the `runs` row of line {first}"),
                    ));
                }
                run_lines.push(number);
                state.runs.push(*row);
            }
            Row::Card(row) => {
                let id = row.id().unwrap_or_default().to_owned();
                if let Some(first) = card_ids.insert(id, number) {
                    return Err(at(
                        number,
                        &format!("`id` repeats the `cards` row of line {first}"),
                    ));
                }
                state.cards.push(*row);
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
    for (row, number) in state.runs.iter().zip(&run_lines) {
        let task = row.task_id().unwrap_or_default();
        if !task_ids.contains_key(task) {
            return Err(at(
                *number,
                &format!("a run of `{task}`, a task the dump does not hold"),
            ));
        }
    }
    let found = state.counts();
    let counts_differ = found.proposals != header.proposals
        || found.tasks != header.tasks
        || found.runs != header.runs
        || found.cards != header.cards
        || found.events != header.events;
    if counts_differ {
        return Err(CliError::cannot(match header.format {
            PROPOSALS_ONLY_FORMAT => format!(
                "{label}: header counts {}, {}; found {}, {}",
                header.proposals, header.events, found.proposals, found.events
            ),
            NO_CARDS_FORMAT => format!(
                "{label}: header counts {}, {}, {}, {}; found {}, {}, {}, {}",
                header.proposals,
                header.tasks,
                header.runs,
                header.events,
                found.proposals,
                found.tasks,
                found.runs,
                found.events
            ),
            _ => format!(
                "{label}: header counts {}, {}, {}, {}, {}; found {}, {}, {}, {}, {}",
                header.proposals,
                header.tasks,
                header.runs,
                header.cards,
                header.events,
                found.proposals,
                found.tasks,
                found.runs,
                found.cards,
                found.events
            ),
        }));
    }
    Ok(state)
}

/// The header's values that the rows are checked against.
struct Header {
    format: u64,
    project: String,
    /// The `proposals` columns of its queue schema.
    columns: &'static [&'static str],
    /// The `cards` columns of its queue schema: none before format 3.
    card_columns: &'static [&'static str],
    proposals: u64,
    tasks: u64,
    runs: u64,
    cards: u64,
    events: u64,
}

/// A parsed row.
enum Row {
    Proposal(Box<StoredProposal>),
    Task(Box<StoredTask>),
    Run(Box<StoredRun>),
    Card(Box<StoredCard>),
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
/// all else), then exactly the format's keys (format 1: five; 2: seven; 3
/// to 7: eight), `queue_schema` (format 1: 1 to 3; 2: 4 to 8; 3: 9; 4: 10;
/// 5: 11; 6: 12; 7: 13 to this build's), `project` the root's slug, the
/// counts.
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
             \"proposals\":…,\"tasks\":…,\"runs\":…,\"cards\":…,\"events\":…}`"
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
    if format < PROPOSALS_ONLY_FORMAT {
        return Err(format!(
            "the dump is of format {format}, which this build does not read (it reads formats \
             {PROPOSALS_ONLY_FORMAT} to {STATE_FORMAT})"
        ));
    }
    let header_keys: &[&str] = match format {
        PROPOSALS_ONLY_FORMAT => &HEADER_KEYS_1,
        NO_CARDS_FORMAT => &HEADER_KEYS_2,
        _ => &HEADER_KEYS,
    };
    let mut keys: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    keys.sort_unstable();
    let mut expected = header_keys.to_vec();
    expected.sort_unstable();
    if keys != expected {
        return Err(format!(
            "the header's keys are not exactly {} (format {format})",
            header_keys
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
    // Format 1 holds the schemas before the tasks, format 2 those to the
    // cards, format 3 the one before a card's tasks, format 4 the one before
    // a proposal's card, format 5 the one before a card's mandate, format 6
    // the one before a question's gist, format 7 the rest.
    let held = match format {
        PROPOSALS_ONLY_FORMAT => 1..TASKS_SCHEMA,
        NO_CARDS_FORMAT => TASKS_SCHEMA..CARDS_SCHEMA,
        NO_TASK_LINKS_FORMAT => CARDS_SCHEMA..TASK_LINKS_SCHEMA,
        NO_CARD_ITEMS_FORMAT => TASK_LINKS_SCHEMA..CARD_ITEMS_SCHEMA,
        NO_MANDATE_FORMAT => CARD_ITEMS_SCHEMA..MANDATE_SCHEMA,
        NO_PLAIN_QUESTIONS_FORMAT => MANDATE_SCHEMA..PLAIN_QUESTIONS_SCHEMA,
        _ => PLAIN_QUESTIONS_SCHEMA..QUEUE_SCHEMA_VERSION + 1,
    };
    let columns = proposal_columns(schema).filter(|_| held.contains(&schema));
    let Some(columns) = columns else {
        let holds = if held.end - held.start == 1 {
            format!("queue schema {}", held.start)
        } else {
            format!("queue schemas {} to {}", held.start, held.end - 1)
        };
        return Err(format!(
            "the dump's queue schema is {schema}, which a format-{format} dump does not hold (it \
             holds {holds})"
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
    let (tasks, runs) = if format == PROPOSALS_ONLY_FORMAT {
        (0, 0)
    } else {
        (count("tasks")?, count("runs")?)
    };
    let cards = if format > NO_CARDS_FORMAT {
        count("cards")?
    } else {
        0
    };
    Ok(Header {
        format,
        project: project.clone(),
        columns,
        card_columns: card_columns(schema).unwrap_or_default(),
        proposals: count("proposals")?,
        tasks,
        runs,
        cards,
        events: count("events")?,
    })
}

/// A row line: `{"proposals":{…}}`, `{"tasks":{…}}`, `{"runs":{…}}` (from
/// format 2), `{"cards":{…}}` (from format 3) or `{"events":{…}}`, every
/// column of the table (`proposals`, `cards`: of the header's queue schema)
/// once and no other, `TEXT` a string or `null`, `run` and `seq` integers ≥
/// 1, `position` an integer or `null`, an `id` and a run's `task_id` as the
/// queue writes them, `project` the header's.
fn read_row(mut entries: Vec<(String, Json)>, header: &Header) -> Result<Row, String> {
    const SHAPE: &str = "not a row `{\"<table>\":{…}}` of one table";
    let project = header.project.as_str();
    let with_tasks = header.format != PROPOSALS_ONLY_FORMAT;
    let with_cards = header.format > NO_CARDS_FORMAT;
    if entries.len() != 1 {
        return Err(SHAPE.to_owned());
    }
    let (table, value) = entries.remove(0);
    let columns: &[&str] = match table.as_str() {
        "proposals" => header.columns,
        "tasks" if with_tasks => &TASK_COLUMNS,
        "runs" if with_tasks => &RUN_COLUMNS,
        "cards" if with_cards => header.card_columns,
        "events" => &EVENT_COLUMNS,
        _ if with_cards => {
            return Err(
                "a row of an unknown table (a dump holds `proposals`, `tasks`, `runs`, `cards` \
                 and `events` rows)"
                    .to_owned(),
            );
        }
        _ if with_tasks => {
            return Err(
                "a row of an unknown table (a format-2 dump holds `proposals`, `tasks`, `runs` \
                 and `events` rows)"
                    .to_owned(),
            );
        }
        _ => {
            return Err(
                "a row of an unknown table (a format-1 dump holds `proposals` and `events` rows)"
                    .to_owned(),
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
    if table == "tasks" {
        let mut columns: [Option<String>; TASK_COLUMNS.len()] = std::array::from_fn(|_| None);
        for (slot, (column, value)) in columns.iter_mut().zip(taken) {
            *slot = text(column, value)?;
        }
        let row = StoredTask { columns };
        if !row
            .id()
            .and_then(task_number)
            .is_some_and(|number| number >= 1)
        {
            return Err(format!(
                "`id` is not a task ID as the queue writes it (`T-` and 4 or more digits, \
                 numbered from 1 to {})",
                u64::MAX
            ));
        }
        if row.project() != Some(project) {
            return Err(format!("`project` is not the header's (`{project}`)"));
        }
        return Ok(Row::Task(Box::new(row)));
    }
    if table == "runs" {
        let mut columns: [Option<String>; RUN_COLUMNS.len() - 1] = Default::default();
        let mut run = None;
        let mut slot = 0;
        for (column, value) in taken {
            if column == "run" {
                run = match value {
                    Json::Count(number) if number >= 1 => i64::try_from(number).ok(),
                    _ => None,
                };
                if run.is_none() {
                    return Err("`run` is not an integer ≥ 1".to_owned());
                }
                continue;
            }
            columns[slot] = text(column, value)?;
            slot += 1;
        }
        let Some(run) = run else {
            return Err("`run` is not an integer ≥ 1".to_owned());
        };
        let row = StoredRun { run, columns };
        if !row
            .task_id()
            .and_then(task_number)
            .is_some_and(|number| number >= 1)
        {
            return Err("`task_id` is not a task ID as the queue writes it".to_owned());
        }
        return Ok(Row::Run(Box::new(row)));
    }
    if table == "cards" {
        let mut columns: [Option<String>; CARD_COLUMNS.len() - 1] = Default::default();
        let mut position = None;
        let mut slot = 0;
        for (column, value) in taken {
            if column == POSITION {
                position = match value {
                    Json::Null => None,
                    Json::Count(number) => Some(i64::try_from(number).map_err(|_| bad_position())?),
                    Json::Negative(number) => Some(number),
                    _ => return Err(bad_position()),
                };
                continue;
            }
            columns[slot] = text(column, value)?;
            slot += 1;
        }
        let row = StoredCard { position, columns };
        if !row
            .id()
            .and_then(card_number)
            .is_some_and(|number| number >= 1)
        {
            return Err(format!(
                "`id` is not a card ID as the queue writes it (`C-` and 4 or more digits, \
                 numbered from 1 to {})",
                u64::MAX
            ));
        }
        if row.project() != Some(project) {
            return Err(format!("`project` is not the header's (`{project}`)"));
        }
        return Ok(Row::Card(Box::new(row)));
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

/// A `position` that is neither `null` nor a 64-bit integer.
fn bad_position() -> String {
    "`position` is not an integer or null".to_owned()
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
    /// An integer below 0: a corrupt `position`, kept as stored.
    Negative(i64),
    String(String),
    Object(Vec<(String, Json)>),
    /// A boolean, a fractional number, an array.
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
        Ok(u64::try_from(number).map_or(Json::Negative(number), Json::Count))
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
