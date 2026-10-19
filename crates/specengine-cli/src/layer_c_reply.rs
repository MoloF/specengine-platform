//! Protocol v1's reply as `spec resolve` reads it (task spec
//! `layer-c-resolve`, "Checks"; canon `layer-c-protocol`, "Reply",
//! "Order", "Endings"): each complete line judged as it arrives, against
//! the request's files; the first fault ends the judging. Pure: bytes in,
//! the reply's facts or a [`Fault`] out; the process is
//! `crate::layer_c_run`'s.
//!
//! - **Start**: `refused` alone, or `hello` first. The `protocol` of either
//!   is read before the line's other checks: another number is a
//!   [`Fault::Mismatch`], never a malformed line.
//! - **Steps**: workspaces numbered from 1 without a gap, each `discover`
//!   then `database` with one `dir` (any string: `.`, `a/b`, with `..`,
//!   repeated, unsorted); a lone `discover` only before an `error`.
//! - **End**: an `error` whose `workspace` is 0 to the last step's number
//!   plus 1, or, after every step, one `file` line per requested path in
//!   request order, then `done` with the tallies.
//! - **Every line**: none after the last (`done`, `error`, `refused`);
//!   JSON, an object with exactly its type's keys; at most
//!   [`REPLY_LINE_CAP`] bytes (the reader's to measure).
//!
//! The keys, line types, states and categories are protocol v1's words,
//! this module's own data; the kinds are the code crate's
//! (`specengine_code::layer_c::KIND_MAP`).

use serde_json::{Map, Value};
use specengine_code::layer_c::KIND_MAP;

/// The protocol `spec` speaks.
pub(crate) const PROTOCOL: u64 = 1;

/// The longest reply line read, in bytes before its line end.
pub(crate) const REPLY_LINE_CAP: usize = 8_388_608;

const KEY_TYPE: &str = "t";
const KEY_PROTOCOL: &str = "protocol";
const KEY_STEP: &str = "step";

const HELLO: &str = "hello";
const STEP: &str = "step";
const FILE: &str = "file";
const DONE: &str = "done";
const ERROR: &str = "error";
const REFUSED: &str = "refused";

const DISCOVER: &str = "discover";
const DATABASE: &str = "database";

const HELLO_KEYS: [&str; 4] = [KEY_TYPE, KEY_PROTOCOL, "ra_ap", "host"];
const DISCOVER_KEYS: [&str; 8] = [
    KEY_TYPE,
    "workspace",
    "dir",
    KEY_STEP,
    "ms",
    "toolchain",
    "sysroot_packages",
    "metadata_degraded",
];
const DATABASE_KEYS: [&str; 5] = [KEY_TYPE, "workspace", "dir", KEY_STEP, "ms"];
const FILE_KEYS: [&str; 4] = [KEY_TYPE, "path", "in_crate", "items"];
const ITEM_KEYS: [&str; 5] = ["start", "end", "kind", "state", "moniker"];
const DONE_KEYS: [&str; 6] = [KEY_TYPE, "files", "items", "by_state", "peak_rss", "ms"];
const ERROR_KEYS: [&str; 3] = [KEY_TYPE, "workspace", "category"];
const REFUSED_KEYS: [&str; 3] = [KEY_TYPE, KEY_PROTOCOL, "reason"];

/// An item's states, in `by_state`'s order.
pub(crate) const STATES: [&str; 5] = ["moniker", "local", "none", "unresolved", "not_loaded"];

/// The state whose item carries a moniker.
pub(crate) const MONIKER_STATE: &str = STATES[0];

/// The categories of an `error` line.
const CATEGORIES: [&str; 3] = ["metadata", "database", "path"];

/// The largest offset or count a row stores (SQLite's signed integers).
const LARGEST: u64 = i64::MAX.unsigned_abs();

/// Why the reply is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fault {
    /// The line `n` (from 1) fails a check.
    Malformed(usize),
    /// `hello` or `refused` gave this protocol.
    Mismatch(u64),
}

/// One workspace's two steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workspace {
    pub(crate) dir: String,
    pub(crate) toolchain: Option<String>,
    pub(crate) sysroot_packages: u64,
    pub(crate) metadata_degraded: bool,
    pub(crate) discover_ms: u64,
    /// `None` until its `database` step is read.
    pub(crate) database_ms: Option<u64>,
}

/// One item of a `file` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) kind: String,
    pub(crate) state: String,
    pub(crate) moniker: Option<String>,
}

/// One `file` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileReply {
    pub(crate) path: String,
    pub(crate) in_crate: bool,
    pub(crate) items: Vec<Item>,
}

/// The `done` line's figures beside the tallies it repeats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Done {
    pub(crate) peak_rss: Option<u64>,
    pub(crate) ms: u64,
}

/// The reply's last line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Last {
    Done(Done),
    Error { workspace: u64, category: String },
    Refused { reason: String },
}

/// Where the reply stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Nothing read.
    Start,
    /// After `hello`: steps; `open` is a `discover` awaiting its
    /// `database`.
    Steps { open: bool },
    /// After the first `file` line.
    Files,
    /// After the last line, or a fault.
    Ended,
}

/// The reply read so far.
#[derive(Debug, Clone)]
pub(crate) struct Reply {
    requested: Vec<String>,
    lines: usize,
    phase: Phase,
    /// `hello`'s `ra_ap` and `host`.
    pub(crate) hello: Option<(String, String)>,
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) files: Vec<FileReply>,
    pub(crate) last: Option<Last>,
}

impl Reply {
    /// Nothing read yet; `requested` the request's `files`, in order.
    pub(crate) fn new(requested: Vec<String>) -> Self {
        Self {
            requested,
            lines: 0,
            phase: Phase::Start,
            hello: None,
            workspaces: Vec::new(),
            files: Vec::new(),
            last: None,
        }
    }

    /// The last line (`done`, `error` or `refused`) is read.
    pub(crate) fn complete(&self) -> bool {
        self.last.is_some()
    }

    /// `done` is read.
    pub(crate) fn done(&self) -> bool {
        matches!(self.last, Some(Last::Done(_)))
    }

    /// The next line is longer than [`REPLY_LINE_CAP`]: its fault.
    pub(crate) fn oversized(&mut self) -> Fault {
        self.lines += 1;
        self.phase = Phase::Ended;
        Fault::Malformed(self.lines)
    }

    /// Judges the next complete line (`bytes` without its line end).
    pub(crate) fn line(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        self.lines += 1;
        let judged = self.judge(bytes);
        if judged.is_err() {
            self.phase = Phase::Ended;
        }
        judged
    }

    fn judge(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        let bad = Fault::Malformed(self.lines);
        if self.phase == Phase::Ended {
            return Err(bad);
        }
        let text = std::str::from_utf8(bytes).map_err(|_| bad.clone())?;
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
            return Err(bad);
        };
        let kind = map
            .get(KEY_TYPE)
            .and_then(Value::as_str)
            .ok_or(bad.clone())?;
        if kind == HELLO || kind == REFUSED {
            let protocol = map
                .get(KEY_PROTOCOL)
                .and_then(Value::as_u64)
                .ok_or(bad.clone())?;
            if protocol != PROTOCOL {
                return Err(Fault::Mismatch(protocol));
            }
        }
        let judged = match (self.phase, kind) {
            (Phase::Start, HELLO) => self.hello(&map),
            (Phase::Start, REFUSED) => self.refused(&map),
            (Phase::Steps { open }, STEP) => self.step(&map, open),
            (Phase::Steps { open: false } | Phase::Files, FILE) => self.file(&map),
            (Phase::Steps { open: false } | Phase::Files, DONE) => self.finish(&map),
            (Phase::Steps { .. } | Phase::Files, ERROR) => self.error(&map),
            _ => None,
        };
        judged.ok_or(bad)
    }

    fn hello(&mut self, map: &Map<String, Value>) -> Option<()> {
        exact(map, &HELLO_KEYS)?;
        let ra_ap = text(map, "ra_ap")?;
        let host = text(map, "host")?;
        self.hello = Some((ra_ap, host));
        self.phase = Phase::Steps { open: false };
        Some(())
    }

    fn refused(&mut self, map: &Map<String, Value>) -> Option<()> {
        exact(map, &REFUSED_KEYS)?;
        let reason = text(map, "reason")?;
        self.last = Some(Last::Refused { reason });
        self.phase = Phase::Ended;
        Some(())
    }

    fn step(&mut self, map: &Map<String, Value>, open: bool) -> Option<()> {
        let workspace = number(map, "workspace")?;
        let dir = text(map, "dir")?;
        let ms = number(map, "ms")?;
        match map.get(KEY_STEP).and_then(Value::as_str)? {
            DISCOVER if !open => {
                exact(map, &DISCOVER_KEYS)?;
                if workspace != count(self.workspaces.len()) + 1 {
                    return None;
                }
                let toolchain = match map.get("toolchain")? {
                    Value::Null => None,
                    Value::String(toolchain) => Some(toolchain.clone()),
                    _ => return None,
                };
                let sysroot_packages = number(map, "sysroot_packages")?;
                let metadata_degraded = map.get("metadata_degraded")?.as_bool()?;
                self.workspaces.push(Workspace {
                    dir,
                    toolchain,
                    sysroot_packages,
                    metadata_degraded,
                    discover_ms: ms,
                    database_ms: None,
                });
                self.phase = Phase::Steps { open: true };
                Some(())
            }
            DATABASE if open => {
                exact(map, &DATABASE_KEYS)?;
                let number = count(self.workspaces.len());
                let current = self.workspaces.last_mut()?;
                if workspace != number || dir != current.dir {
                    return None;
                }
                current.database_ms = Some(ms);
                self.phase = Phase::Steps { open: false };
                Some(())
            }
            _ => None,
        }
    }

    fn file(&mut self, map: &Map<String, Value>) -> Option<()> {
        exact(map, &FILE_KEYS)?;
        let path = text(map, "path")?;
        if self.requested.get(self.files.len()) != Some(&path) {
            return None;
        }
        let in_crate = map.get("in_crate")?.as_bool()?;
        let mut items: Vec<Item> = Vec::new();
        for value in map.get("items")?.as_array()? {
            let Value::Object(item) = value else {
                return None;
            };
            exact(item, &ITEM_KEYS)?;
            let start = number(item, "start")?;
            let end = number(item, "end")?;
            if items.last().is_some_and(|before| before.start >= start) {
                return None;
            }
            let kind = text(item, "kind")?;
            if !KIND_MAP.iter().any(|(known, _)| *known == kind) {
                return None;
            }
            let state = text(item, "state")?;
            if !STATES.contains(&state.as_str()) {
                return None;
            }
            let moniker = match item.get("moniker")? {
                Value::String(moniker) if state == MONIKER_STATE => Some(moniker.clone()),
                Value::Null if state != MONIKER_STATE => None,
                _ => return None,
            };
            items.push(Item {
                start,
                end,
                kind,
                state,
                moniker,
            });
        }
        self.files.push(FileReply {
            path,
            in_crate,
            items,
        });
        self.phase = Phase::Files;
        Some(())
    }

    fn finish(&mut self, map: &Map<String, Value>) -> Option<()> {
        exact(map, &DONE_KEYS)?;
        if self.files.len() != self.requested.len() {
            return None;
        }
        if number(map, "files")? != count(self.files.len()) {
            return None;
        }
        let tallies = self.tallies();
        if number(map, "items")? != tallies.iter().sum::<u64>() {
            return None;
        }
        let Value::Object(by_state) = map.get("by_state")? else {
            return None;
        };
        exact(by_state, &STATES)?;
        for (state, tally) in STATES.iter().zip(tallies) {
            if number(by_state, state)? != tally {
                return None;
            }
        }
        let peak_rss = match map.get("peak_rss")? {
            Value::Null => None,
            value => Some(value.as_u64()?),
        };
        let ms = number(map, "ms")?;
        self.last = Some(Last::Done(Done { peak_rss, ms }));
        self.phase = Phase::Ended;
        Some(())
    }

    fn error(&mut self, map: &Map<String, Value>) -> Option<()> {
        exact(map, &ERROR_KEYS)?;
        let workspace = number(map, "workspace")?;
        if workspace > count(self.workspaces.len()) + 1 {
            return None;
        }
        let category = text(map, "category")?;
        if !CATEGORIES.contains(&category.as_str()) {
            return None;
        }
        self.last = Some(Last::Error {
            workspace,
            category,
        });
        self.phase = Phase::Ended;
        Some(())
    }

    /// The items read, per state in [`STATES`] order.
    pub(crate) fn tallies(&self) -> [u64; 5] {
        let mut tallies = [0u64; 5];
        for item in self.files.iter().flat_map(|file| &file.items) {
            if let Some(at) = STATES.iter().position(|state| *state == item.state) {
                tallies[at] += 1;
            }
        }
        tallies
    }
}

/// `map` has exactly `keys`.
fn exact(map: &Map<String, Value>, keys: &[&str]) -> Option<()> {
    (map.len() == keys.len() && keys.iter().all(|key| map.contains_key(*key))).then_some(())
}

/// A string value.
fn text(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)?.as_str().map(str::to_owned)
}

/// A non-negative integer a row can store.
fn number(map: &Map<String, Value>, key: &str) -> Option<u64> {
    map.get(key)?.as_u64().filter(|value| *value <= LARGEST)
}

/// A length as the reply counts.
fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
