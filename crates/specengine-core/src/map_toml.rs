//! `[[map.ring]]` of `specengine.toml` (task spec `scope-rings` SR1): the
//! scope rings, each a set of nodes chosen by a root, by kinds, or both,
//! read only by `spec rings` and its daemon twin. Of
//! [`crate::workspace_toml`]'s family, read apart:
//! [`crate::ProjectConfig::from_toml`] only name-checks `map`, so a bad
//! table fails no read, command, check or gate.
//!
//! A ring holds a `title` and a `root`, a `kinds` or both. `title` is a
//! `[[module]]`'s title, `root` a rooted module's (a REF in any `spec show`
//! form, never resolved here) and `kinds` a direction's, through the same
//! readers and with the same messages: a kind is the project's word, never
//! checked against `[ids]`. `[map]` holds `ring` alone, at most
//! [`MAP_RINGS_MAX`] tables.
//!
//! The reader never fails. Any problem serves no ring and gives one note,
//! the first problem by line (ties in reading order): `<problem>; no ring
//! is served`, its line the offending value's, a missing key's table
//! header's, else 1. Nothing here names a ring, a kind or a state: every
//! title and kind is data (ADR-0008). Pure: the text in, the rings out.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::de::IgnoredAny;
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::card_toml::title_problem;
use crate::health_toml::{Errors, line_of, unknown_field};
use crate::project_toml::ProjectError;
use crate::workspace_toml::{kinds_of, misread, required, root_problem, string, strings, tables};

/// The most `[[map.ring]]` tables.
pub const MAP_RINGS_MAX: usize = 12;

/// The top-level key of the rings' table.
const MAP: &str = "map";
/// `[map]`'s one key.
const RING: &str = "ring";
/// A ring's keys, in the order a message lists them.
const RING_KEYS: [&str; 3] = ["title", "root", "kinds"];
/// How a message names a ring table.
const RING_TABLE: &str = "`[[map.ring]]`";
/// What every note ends with: what is served.
const SERVED: &str = "no ring is served";

/// One ring as written: its title, and a root, kinds or both.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RingConfig {
    pub title: String,
    /// A REF in any `spec show` form, resolved by the read.
    pub root: Option<String>,
    /// As written, each once: matched exactly against a node's `kind`.
    pub kinds: Option<Vec<String>>,
    /// The 1-based line of the table's header.
    pub line: usize,
}

/// Why no ring is served: its 1-based line and the message.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapNote {
    pub line: usize,
    pub message: String,
}

impl MapNote {
    /// `<file>:<line>: <message>`.
    pub fn at(&self, file: &str) -> String {
        format!("{file}:{}: {}", self.line, self.message)
    }
}

/// `[map]`, read: the rings innermost first, or none with at most one note.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapConfig {
    pub rings: Vec<RingConfig>,
    pub notes: Vec<MapNote>,
}

impl MapConfig {
    /// Reads `[map]` from `text` (see the module documentation).
    pub fn from_toml(text: &str) -> Self {
        map_from_toml(text)
    }

    /// Each note as `<file>:<line>: <message>`, in order.
    pub fn notes_at(&self, file: &str) -> Vec<String> {
        self.notes.iter().map(|note| note.at(file)).collect()
    }
}

/// [`MapConfig::from_toml`] as a function.
pub fn map_from_toml(text: &str) -> MapConfig {
    let root = match DeTable::parse(text) {
        Ok(root) => root,
        // `ProjectConfig` refuses such a file first; the reader still answers.
        Err(error) => {
            return refused(MapNote {
                line: error.span().map_or(1, |span| line_of(text, span.start)),
                message: format!("{}; {SERVED}", error.message().trim()),
            });
        }
    };
    let map = root
        .get_ref()
        .iter()
        .find(|(name, _)| name.get_ref().as_ref() == MAP)
        .map(|(_, value)| value);
    match read_rings(text, map) {
        Ok(rings) => MapConfig {
            rings,
            notes: Vec::new(),
        },
        Err(error) => refused(MapNote {
            line: error.line.unwrap_or(1),
            message: format!("{}; {SERVED}", error.message),
        }),
    }
}

/// No ring, and `note`.
fn refused(note: MapNote) -> MapConfig {
    MapConfig {
        rings: Vec::new(),
        notes: vec![note],
    }
}

/// `[map]` (absent: no ring): the rings, or the first problem by line.
fn read_rings(
    text: &str,
    map: Option<&Spanned<DeValue<'_>>>,
) -> Result<Vec<RingConfig>, ProjectError> {
    let Some(map) = map else {
        return Ok(Vec::new());
    };
    let mut errors = Errors::new(text);
    let DeValue::Table(table) = map.get_ref() else {
        misread::<BTreeMap<String, IgnoredAny>>(map, &mut errors);
        return Err(first(errors));
    };
    let mut rings = Vec::new();
    for (key, value) in table.iter() {
        if key.get_ref().as_ref() != RING {
            errors.push(key.span(), format!("`[{MAP}]` holds only `{RING}`"));
            continue;
        }
        for (span, entry) in tables(Some(value), RING_TABLE, MAP_RINGS_MAX, &mut errors) {
            if let Some(ring) = read_ring(span, entry, &mut errors) {
                rings.push(ring);
            }
        }
    }
    match errors.first() {
        Some(error) => Err(error),
        None => Ok(rings),
    }
}

/// One `[[map.ring]]` at `span`: the ring, or `None` with its problems
/// pushed.
fn read_ring(
    span: Range<usize>,
    entry: &DeTable<'_>,
    errors: &mut Errors<'_>,
) -> Option<RingConfig> {
    let before = errors.len();
    // A key written of another type is `Some(None)` (pushed), so it is not
    // also missing.
    let mut title = None;
    let mut root = None;
    let mut kinds = None;
    for (key, value) in entry.iter() {
        match key.get_ref().as_ref() {
            "title" => title = Some(string(value, errors)),
            "root" => root = Some(string(value, errors)),
            "kinds" => kinds = Some((value.span(), strings(value, errors))),
            other => errors.push(key.span(), unknown_field(other, &RING_KEYS)),
        }
    }
    let title = match title {
        Some(Some((title, at))) => {
            if let Some(problem) = title_problem(&title) {
                errors.push(at, problem);
            }
            Some(title)
        }
        Some(None) => None,
        None => {
            errors.push(span.clone(), required(RING_TABLE, "title"));
            None
        }
    };
    if root.is_none() && kinds.is_none() {
        let named_as = title
            .as_ref()
            .map_or_else(String::new, |title| format!(" `{title}`"));
        errors.push(
            span.clone(),
            format!(
                "{RING_TABLE}{named_as} holds neither `root` nor `kinds`: a ring holds one or \
                 both"
            ),
        );
    }
    let root = match root {
        Some(Some((root, at))) => {
            if let Some(problem) = root_problem(&root) {
                errors.push(at, problem);
            }
            Some(root)
        }
        _ => None,
    };
    let kinds =
        kinds.and_then(|(at, listed)| listed.and_then(|listed| kinds_of(at, listed, errors)));
    let title = title?;
    (errors.len() == before).then(|| RingConfig {
        title,
        root,
        kinds,
        line: errors.line(span),
    })
}

/// The first problem pushed, by line; a guard when none was (never).
fn first(errors: Errors<'_>) -> ProjectError {
    errors.first().unwrap_or_else(|| ProjectError {
        line: None,
        message: format!("`[{MAP}]` does not read"),
    })
}
