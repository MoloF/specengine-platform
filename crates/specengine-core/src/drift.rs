//! The pure half of a `drift` queue item (task spec `drift-queue`, "The
//! item", "Triggers"; ADR-0045): its stored evidence ([`DriftEvidence`]:
//! the first [`DRIFT_ROWS_MAX`] non-`ok` rows of its node as `spec verify
//! --json` writes them, and how many more), the bytes its `drift_hash`
//! covers ([`drift_hash_input`]: one array per non-`ok` row, by key; the
//! caller hashes them), which rows raise an item ([`raises`]) and its
//! summary ([`drift_summary`]). Nothing is judged, hashed or read here, and
//! no project's word appears (ADR-0008).

use std::fmt;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Number, Value};

use crate::lock::HASH_PREFIX;
use crate::verify::{KEY_PROBLEM, MarkerRev, State};

/// The most rows an item stores; `more` counts the rest.
pub const DRIFT_ROWS_MAX: usize = 64;

/// The first line of a `drift_hash`'s input: its recipe.
pub const DRIFT_HASH_DOMAIN: &str = "specengine-drift/v1";

/// One stored row: a `rows` item of `spec verify --json`
/// (`docs/canon/spec-verify-output.md` "JSON"), key for key and in its
/// order. Every key is there, a `null` one too: a row missing one is off
/// its shape (serde alone would read a missing `Option` as `null`). A key
/// a later build writes after these (5b's `levels`, 5c's `anchor`) is no
/// defect: this view reads past it, and [`StoredJson`] keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftRow {
    pub node: String,
    pub relation: String,
    pub file: String,
    #[serde(deserialize_with = "present")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "present")]
    pub symbol: Option<String>,
    #[serde(deserialize_with = "present")]
    pub item_kind: Option<String>,
    pub state: String,
    #[serde(deserialize_with = "present")]
    pub category: Option<String>,
    #[serde(deserialize_with = "present")]
    pub detail: Option<String>,
    #[serde(deserialize_with = "present")]
    pub class: Option<String>,
    pub pre_existing: bool,
    pub markers: Vec<DriftMarker>,
    #[serde(deserialize_with = "present")]
    pub node_rev: Option<u32>,
    #[serde(deserialize_with = "present")]
    pub lock: Option<DriftLock>,
    #[serde(deserialize_with = "present")]
    pub compared: Option<String>,
}

/// A row's marker: its line and `@N` (`null`: unversioned or malformed),
/// both keys there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftMarker {
    pub line: usize,
    #[serde(deserialize_with = "present")]
    pub rev: Option<u32>,
}

/// A nullable key that must be written: `null` reads as `None`; a field
/// read through it is required (serde's derive names a missing one, where
/// a plain `Option` field would read as `None`).
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// A row's lock entry, as `spec verify` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftLock {
    pub rev: u32,
    pub commit: String,
    pub verified_at: String,
}

/// An item's `drift` column: compact JSON `{"rows":[…],"more":<n>}`, the
/// first [`DRIFT_ROWS_MAX`] non-`ok` rows of its node by key, `more` the
/// rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftEvidence {
    pub rows: Vec<DriftRow>,
    pub more: u64,
}

impl DriftEvidence {
    /// The stored column read back; `Err`: why it is off its shape (not
    /// JSON of it, a row missing a key, more than [`DRIFT_ROWS_MAX`] rows,
    /// none at all, a row whose state is no state of a finding).
    pub fn parse(text: &str) -> Result<Self, String> {
        let evidence: Self = serde_json::from_str(text)
            .map_err(|error| format!("not the compact `{{\"rows\":[…],\"more\":…}}`: {error}"))?;
        if evidence.rows.is_empty() {
            return Err("it holds no row".to_owned());
        }
        if evidence.rows.len() > DRIFT_ROWS_MAX {
            return Err(format!(
                "it holds {} rows, more than {DRIFT_ROWS_MAX}",
                evidence.rows.len()
            ));
        }
        if let Some(row) = evidence
            .rows
            .iter()
            .find(|row| state_named(&row.state).is_none_or(|state| state == State::Ok))
        {
            return Err(format!("a row's state `{}` is no finding's", row.state));
        }
        Ok(evidence)
    }

    /// [`Self::parse`], and the column's rows as stored: each row's keys in
    /// their stored order, a later build's kept (what a review shows).
    pub fn parse_stored(text: &str) -> Result<(Self, Vec<StoredJson>), String> {
        /// The column's `rows` alone; [`Self::parse`] read the rest.
        #[derive(Deserialize)]
        struct Rows {
            rows: Vec<StoredJson>,
        }
        let evidence = Self::parse(text)?;
        let stored: Rows = serde_json::from_str(text)
            .map_err(|error| format!("its rows cannot be read as stored: {error}"))?;
        Ok((evidence, stored.rows))
    }
}

/// A JSON value as stored: an object keeps its keys in their stored order,
/// a later build's included (`serde_json::Value` would sort them), so a
/// stored row is shown as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredJson {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<StoredJson>),
    Object(Vec<(String, StoredJson)>),
}

impl Serialize for StoredJson {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(number) => number.serialize(serializer),
            Self::String(text) => serializer.serialize_str(text),
            Self::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for StoredJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StoredVisitor)
    }
}

struct StoredVisitor;

impl<'de> Visitor<'de> for StoredVisitor {
    type Value = StoredJson;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<StoredJson, E> {
        Ok(StoredJson::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<StoredJson, E> {
        Ok(StoredJson::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<StoredJson, D::Error> {
        StoredJson::deserialize(deserializer)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<StoredJson, E> {
        Ok(StoredJson::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, number: i64) -> Result<StoredJson, E> {
        Ok(StoredJson::Number(number.into()))
    }

    fn visit_u64<E: de::Error>(self, number: u64) -> Result<StoredJson, E> {
        Ok(StoredJson::Number(number.into()))
    }

    fn visit_f64<E: de::Error>(self, number: f64) -> Result<StoredJson, E> {
        Number::from_f64(number)
            .map(StoredJson::Number)
            .ok_or_else(|| E::custom("a number that is not finite"))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<StoredJson, E> {
        Ok(StoredJson::String(text.to_owned()))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<StoredJson, E> {
        Ok(StoredJson::String(text))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<StoredJson, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<StoredJson>()? {
            items.push(item);
        }
        Ok(StoredJson::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<StoredJson, A::Error> {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            let value = map.next_value::<StoredJson>()?;
            entries.push((key, value));
        }
        Ok(StoredJson::Object(entries))
    }
}

/// The state named `name`.
fn state_named(name: &str) -> Option<State> {
    [
        State::CannotVerify,
        State::Predated,
        State::BadMarker,
        State::BrokenBinding,
        State::Unlocked,
        State::SpecAhead,
        State::CodeAhead,
        State::Conflict,
        State::Ok,
    ]
    .into_iter()
    .find(|state| state.as_str() == name)
}

/// `b3:` and 64 lowercase hex digits: a `drift_hash`'s form.
pub fn is_drift_hash(text: &str) -> bool {
    text.strip_prefix(HASH_PREFIX).is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

/// A row raises an item for its node ("Triggers"): `code_ahead`,
/// `spec_ahead`, `conflict`, `broken_binding` always; `unlocked` only for a
/// node named by the raise (never in a sweep), and never for a key the
/// lock cannot hold (`unlocked key_problem`: a note); `cannot_verify`,
/// `bad_marker`, `predated` never on their own.
pub fn raises(state: State, detail: Option<&str>, named: bool) -> bool {
    match state {
        State::CodeAhead | State::SpecAhead | State::Conflict | State::BrokenBinding => true,
        State::Unlocked => named && detail != Some(KEY_PROBLEM),
        State::CannotVerify | State::Predated | State::BadMarker | State::Ok => false,
    }
}

/// An item's summary: its rows' distinct `state[ category]`, in key order
/// (the order given), `, `-joined.
pub fn drift_summary<'a>(rows: impl IntoIterator<Item = (&'a str, Option<&'a str>)>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for (state, category) in rows {
        let label = match category {
            Some(category) => format!("{state} {category}"),
            None => state.to_owned(),
        };
        if !seen.contains(&label) {
            seen.push(label);
        }
    }
    seen.join(", ")
}

/// A row's lock entry as its hash covers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashedEntry<'a> {
    pub rev: u32,
    pub spec_hash: &'a str,
    /// The entry's [`crate::lock::lock_code`]: its `levels`, `sig_hash`,
    /// `body_hash` and `deps_hash` lines as the render writes them,
    /// LF-joined, never a fingerprint or `body_bytes`, every kind (task
    /// spec `lock-levels` Q7, Q12). A format-1 lock is never judged, so
    /// never hashed.
    pub code: Option<&'a str>,
    pub file_blob: &'a str,
}

/// The facts of one non-`ok` row that its item's `drift_hash` covers
/// ("The item", Q13). Not covered: marker lines, times, the entry's
/// `commit` and `verified_at`, the commit judged, the mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashedRow<'a> {
    pub node: &'a str,
    pub relation: &'a str,
    pub file: &'a str,
    /// `None` for a keyless row, with `symbol` and `item_kind`.
    pub kind: Option<&'a str>,
    pub symbol: Option<&'a str>,
    pub item_kind: Option<&'a str>,
    pub state: State,
    pub category: Option<&'a str>,
    pub detail: Option<&'a str>,
    /// Its markers' `@N`, in any order.
    pub marker_revs: Vec<MarkerRev>,
    pub entry: Option<HashedEntry<'a>>,
    /// The node's revision R and `norm_hash` H at the commit judged.
    pub rev: Option<u32>,
    pub norm_hash: Option<&'a str>,
    /// The cascade's current fact: an item compared changed by its levels,
    /// `<level>:<digest>` per changed level of `sig`, `body`, `,`-joined in
    /// canonical order (`body:b3:…`); a `module` or `ron` row changed by
    /// its blob, the blob's OID; else `None`. No item's file blob: an edit
    /// beside the item is the same drift.
    pub code: Option<&'a str>,
}

/// The bytes of a `drift_hash`: [`DRIFT_HASH_DOMAIN`], LF, then compact
/// JSON, one array per row in the order given (key order):
/// `[node, relation, file, kind, symbol, item_kind, state, category,
/// detail, marker_revs, lock_rev, lock_spec_hash, lock_code,
/// lock_file_blob, rev, norm_hash, code, moved_from]`. `marker_revs`:
/// `null` (unversioned) first, the `@N` ascending, then `"bad"`;
/// `moved_from` `null` (reserved for 5c's `anchor.from`, so the recipe
/// stays `v1`).
pub fn drift_hash_input(rows: &[HashedRow<'_>]) -> String {
    let arrays: Vec<Value> = rows.iter().map(row_array).collect();
    format!("{DRIFT_HASH_DOMAIN}\n{}", Value::Array(arrays))
}

fn row_array(row: &HashedRow<'_>) -> Value {
    let text = |value: Option<&str>| value.map_or(Value::Null, Value::from);
    let mut revs = row.marker_revs.clone();
    revs.sort_by_key(|rev| match rev {
        MarkerRev::Unversioned => (0, 0),
        MarkerRev::Written(n) => (1, *n),
        MarkerRev::Bad => (2, 0),
    });
    let revs: Vec<Value> = revs
        .into_iter()
        .map(|rev| match rev {
            MarkerRev::Unversioned => Value::Null,
            MarkerRev::Written(n) => Value::from(n),
            MarkerRev::Bad => Value::from("bad"),
        })
        .collect();
    let entry = row.entry;
    Value::Array(vec![
        Value::from(row.node),
        Value::from(row.relation),
        Value::from(row.file),
        text(row.kind),
        text(row.symbol),
        text(row.item_kind),
        Value::from(row.state.as_str()),
        text(row.category),
        text(row.detail),
        Value::Array(revs),
        entry.map_or(Value::Null, |entry| Value::from(entry.rev)),
        text(entry.map(|entry| entry.spec_hash)),
        text(entry.and_then(|entry| entry.code)),
        text(entry.map(|entry| entry.file_blob)),
        row.rev.map_or(Value::Null, Value::from),
        text(row.norm_hash),
        text(row.code),
        Value::Null,
    ])
}
