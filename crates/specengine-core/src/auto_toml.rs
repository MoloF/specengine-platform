//! `[auto]` of `specengine.toml` (task spec `auto-rule`;
//! `docs/canon/auto-mode.md` "Rule", "Rule hash"): the rule under which
//! SpecEngine's own code approves, applies and lands. Read apart, from the bytes of the file at
//! `HEAD` of the repository's main worktree (the caller reads them):
//! [`crate::ProjectConfig::from_toml`] only name-checks `auto`, so a bad
//! table fails no read, command, check or gate.
//!
//! - **Keys**: `mode` (`off`, `local`, `hub`, `both`) and `zones` required;
//!   `exclude_zones`, `exclude_kinds` (default none), `branch` (default
//!   [`DEFAULT_BRANCH`]), `max_tasks` (1 to [`MAX_TASKS`], default
//!   [`DEFAULT_MAX_TASKS`]), `max_applies` (1 to [`MAX_APPLIES`], default
//!   [`DEFAULT_MAX_APPLIES`]). Every key is judged whatever `mode` says.
//! - **Names**: a zone is a `[zones]` key ([`zones_from_toml`] on the same
//!   bytes), a kind one some `[ids]` prefix declares ([`scheme_from_toml`]);
//!   each listed once, at most [`AUTO_LIST_MAX`] per list, `zones` at least
//!   one.
//! - **Never failing**: no `[auto]` is [`AutoRead::absent`], no note; any
//!   problem turns the rule off ([`AutoRead::rule`] `None`) with one note,
//!   the first problem by line (ties in reading order).
//! - **Canonical form and hash**: [`CanonicalRule`] serialized as compact
//!   JSON, keys in its field order, every list and named map in byte order;
//!   [`rule_hash`] is `b3:` and the lowercase hex BLAKE3 of those bytes. Key
//!   order, comments, spacing and list order change nothing; a zone's glob
//!   or a kind's prefix does.
//!
//! The core knows no zone or kind by name: every name is the config's
//! (ADR-0008). Pure: the text in, the rule out.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::gate::{Zone, zones_from_toml};
use crate::health_toml::{Errors, line_of};
use crate::project_toml::ProjectError;
use crate::scheme_toml::scheme_from_toml;
use crate::workspace_toml::{misread, required, string, strings};

/// The top-level key of the table.
pub const AUTO_TABLE: &str = "auto";
/// The most entries of `zones`, `exclude_zones` or `exclude_kinds`.
pub const AUTO_LIST_MAX: usize = 32;
/// `branch` when unwritten.
pub const DEFAULT_BRANCH: &str = "auto";
/// The longest `branch`, in bytes.
pub const BRANCH_MAX: usize = 64;
/// `max_tasks` when unwritten.
pub const DEFAULT_MAX_TASKS: u32 = 10;
/// The highest `max_tasks`.
pub const MAX_TASKS: u32 = 100;
/// `max_applies` when unwritten.
pub const DEFAULT_MAX_APPLIES: u32 = 20;
/// The highest `max_applies`.
pub const MAX_APPLIES: u32 = 1000;
/// The canonical form's `format`.
pub const CANONICAL_FORMAT: u32 = 1;
/// What every note ends with.
const OFF: &str = "auto is off";
/// How a message names the table.
const TABLE: &str = "`[auto]`";
/// The table's keys.
const KEYS: [&str; 7] = [
    "mode",
    "zones",
    "exclude_zones",
    "exclude_kinds",
    "branch",
    "max_tasks",
    "max_applies",
];

/// `[auto] mode`: which surfaces act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AutoMode {
    Off,
    Local,
    Hub,
    Both,
}

impl AutoMode {
    pub const ALL: [Self; 4] = [Self::Off, Self::Local, Self::Hub, Self::Both];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Local => "local",
            Self::Hub => "hub",
            Self::Both => "both",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == text)
    }
}

impl fmt::Display for AutoMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One named `[zones]` entry the rule lists: its name and globs (byte
/// order), compiled as the gate's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedGlobs {
    pub name: String,
    /// In byte order.
    pub globs: Vec<String>,
    zone: Zone,
}

impl NamedGlobs {
    fn of(name: &str, globs: &[String]) -> Result<Self, String> {
        let mut sorted = globs.to_vec();
        sorted.sort();
        sorted.dedup();
        let zone = Zone::from_globs(name, &sorted)?;
        Ok(Self {
            name: name.to_owned(),
            globs: sorted,
            zone,
        })
    }

    /// One of its globs matches the root-relative `path` (`Zone::covers`,
    /// the gate's test).
    pub fn covers(&self, path: &str) -> bool {
        self.zone.covers(path)
    }
}

/// One excluded kind and the `[ids]` prefixes declaring it (byte order).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KindPrefixes {
    pub kind: String,
    pub prefixes: Vec<String>,
}

impl KindPrefixes {
    /// The canonical ID `id` (`slug/ID` too) has one of its prefixes.
    pub fn holds(&self, id: &str) -> bool {
        id_prefix(id).is_some_and(|prefix| self.prefixes.iter().any(|known| known == prefix))
    }
}

/// The prefix of a canonical ID: the text before the first `-` of its last
/// `/`-separated part.
pub fn id_prefix(id: &str) -> Option<&str> {
    let local = id.rsplit_once('/').map_or(id, |(_, local)| local);
    local.split_once('-').map(|(prefix, _)| prefix)
}

/// The `[auto]` table, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoRule {
    pub mode: AutoMode,
    /// The covered zones, by name.
    pub zones: Vec<NamedGlobs>,
    /// The excluded zones, by name.
    pub exclude_zones: Vec<NamedGlobs>,
    /// The excluded kinds, by kind.
    pub exclude_kinds: Vec<KindPrefixes>,
    pub branch: String,
    pub max_tasks: u32,
    pub max_applies: u32,
    /// The canonical form's bytes ([`CanonicalRule`]).
    pub canonical: String,
    /// `b3:` and 64 lowercase hex digits ([`rule_hash`]).
    pub hash: String,
}

/// The canonical form of a rule (`docs/canon/auto-mode.md` "Rule hash"):
/// its fields serialize in this order, each map by name in byte order, each
/// list in byte order. A pack's `auto.started` freezes it as `frozen`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalRule {
    pub format: u32,
    pub mode: String,
    pub zones: BTreeMap<String, Vec<String>>,
    pub exclude_zones: BTreeMap<String, Vec<String>>,
    pub exclude_kinds: BTreeMap<String, Vec<String>>,
    pub branch: String,
    pub max_tasks: u32,
    pub max_applies: u32,
}

impl CanonicalRule {
    /// The compact JSON of the form: the hashed bytes.
    pub fn to_text(&self) -> String {
        // A struct of strings, numbers and string maps always serializes.
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// `b3:` and the lowercase hex BLAKE3 of `canonical`.
pub fn rule_hash(canonical: &str) -> String {
    format!("b3:{}", blake3::hash(canonical.as_bytes()).to_hex())
}

/// Why the rule is off: the first problem by line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AutoNote {
    /// The file as the reading root names it, `@HEAD` added by [`Self::text`].
    pub path: String,
    pub line: usize,
    pub message: String,
}

impl AutoNote {
    /// `<path>@HEAD:<line>: <problem>; auto is off`.
    pub fn text(&self) -> String {
        format!("{}; {OFF}", self.located())
    }

    /// `<path>@HEAD:<line>: <problem>`: the switch's why.
    pub fn located(&self) -> String {
        format!("{}@HEAD:{}: {}", self.path, self.line, self.message)
    }
}

impl fmt::Display for AutoNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text())
    }
}

/// What reading `[auto]` gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoRead {
    /// The rule; `None` when absent or malformed.
    pub rule: Option<AutoRule>,
    /// No `[auto]` table at all.
    pub absent: bool,
    /// At most one: why a written table is off.
    pub notes: Vec<AutoNote>,
}

impl AutoRule {
    /// Reads `[auto]` from `bytes`, the file `path` names at `HEAD` (see the
    /// module documentation). Never fails.
    pub fn from_toml(bytes: &[u8], path: &str) -> AutoRead {
        auto_from_toml(bytes, path)
    }

    /// The rule a frozen canonical form describes, rebuilt and checked;
    /// else why it does not read.
    pub fn from_canonical(form: &CanonicalRule) -> Result<Self, String> {
        if form.format != CANONICAL_FORMAT {
            return Err(format!(
                "a frozen rule of format {}; this build reads {CANONICAL_FORMAT}",
                form.format
            ));
        }
        let mode = AutoMode::parse(&form.mode)
            .ok_or_else(|| format!("a frozen rule's mode `{}` is none", form.mode))?;
        let named = |map: &BTreeMap<String, Vec<String>>| -> Result<Vec<NamedGlobs>, String> {
            map.iter()
                .map(|(name, globs)| NamedGlobs::of(name, globs))
                .collect()
        };
        let zones = named(&form.zones)?;
        let exclude_zones = named(&form.exclude_zones)?;
        let exclude_kinds = form
            .exclude_kinds
            .iter()
            .map(|(kind, prefixes)| {
                let mut prefixes = prefixes.clone();
                prefixes.sort();
                prefixes.dedup();
                KindPrefixes {
                    kind: kind.clone(),
                    prefixes,
                }
            })
            .collect();
        if let Some(problem) = branch_problem(&form.branch) {
            return Err(problem);
        }
        Ok(Self::assembled(
            mode,
            zones,
            exclude_zones,
            exclude_kinds,
            form.branch.clone(),
            form.max_tasks,
            form.max_applies,
        ))
    }

    /// Its canonical form.
    pub fn canonical_form(&self) -> CanonicalRule {
        let named = |list: &[NamedGlobs]| {
            list.iter()
                .map(|zone| (zone.name.clone(), zone.globs.clone()))
                .collect::<BTreeMap<_, _>>()
        };
        CanonicalRule {
            format: CANONICAL_FORMAT,
            mode: self.mode.as_str().to_owned(),
            zones: named(&self.zones),
            exclude_zones: named(&self.exclude_zones),
            exclude_kinds: self
                .exclude_kinds
                .iter()
                .map(|kind| (kind.kind.clone(), kind.prefixes.clone()))
                .collect(),
            branch: self.branch.clone(),
            max_tasks: self.max_tasks,
            max_applies: self.max_applies,
        }
    }

    /// The covered zones' names, in the rule's order.
    pub fn zone_names(&self) -> Vec<&str> {
        self.zones.iter().map(|zone| zone.name.as_str()).collect()
    }

    /// The excluded zones' names, in the rule's order.
    pub fn excluded_zone_names(&self) -> Vec<&str> {
        self.exclude_zones
            .iter()
            .map(|zone| zone.name.as_str())
            .collect()
    }

    /// The excluded kinds, in the rule's order.
    pub fn excluded_kind_names(&self) -> Vec<&str> {
        self.exclude_kinds
            .iter()
            .map(|kind| kind.kind.as_str())
            .collect()
    }

    /// The rule of these parts, its canonical form and hash computed; the
    /// lists sorted by name.
    fn assembled(
        mode: AutoMode,
        mut zones: Vec<NamedGlobs>,
        mut exclude_zones: Vec<NamedGlobs>,
        mut exclude_kinds: Vec<KindPrefixes>,
        branch: String,
        max_tasks: u32,
        max_applies: u32,
    ) -> Self {
        zones.sort_by(|a, b| a.name.cmp(&b.name));
        exclude_zones.sort_by(|a, b| a.name.cmp(&b.name));
        exclude_kinds.sort_by(|a, b| a.kind.cmp(&b.kind));
        let mut rule = Self {
            mode,
            zones,
            exclude_zones,
            exclude_kinds,
            branch,
            max_tasks,
            max_applies,
            canonical: String::new(),
            hash: String::new(),
        };
        rule.canonical = rule.canonical_form().to_text();
        rule.hash = rule_hash(&rule.canonical);
        rule
    }
}

/// Why `branch` is no branch name the rule takes (its judging,
/// `docs/canon/auto-mode.md` "Rule"): ASCII letters, digits, `.`, `_`, `-`, `/`, 1 to
/// [`BRANCH_MAX`] bytes; no leading `-`, `.` or `/`, no `..` or `//`, no
/// ending `/`, `.` or `.lock`.
pub fn branch_problem(branch: &str) -> Option<String> {
    let allowed = branch
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'));
    let refused = branch.is_empty()
        || branch.len() > BRANCH_MAX
        || !allowed
        || branch.starts_with(['-', '.', '/'])
        || branch.contains("..")
        || branch.contains("//")
        || branch.ends_with(['/', '.'])
        || branch.ends_with(".lock");
    refused.then(|| {
        format!(
            "`branch` `{branch}` is no branch name (letters, digits, `.`, `_`, `-`, `/`; 1 to \
             {BRANCH_MAX} bytes)"
        )
    })
}

/// [`AutoRule::from_toml`] as a function.
pub fn auto_from_toml(bytes: &[u8], path: &str) -> AutoRead {
    let note = |line: usize, message: String| AutoRead {
        rule: None,
        absent: false,
        notes: vec![AutoNote {
            path: path.to_owned(),
            line,
            message,
        }],
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        // No table can be told apart in bytes that are no text: a table
        // `ProjectConfig` would refuse first, judged off.
        return note(1, "the file is not UTF-8".to_owned());
    };
    let root = match DeTable::parse(text) {
        Ok(root) => root,
        Err(error) => {
            // Absent unless a line names the table: a file that is not TOML
            // is `ProjectConfig`'s error; here only whether auto is on.
            if !mentions_auto(text) {
                return AutoRead {
                    rule: None,
                    absent: true,
                    notes: Vec::new(),
                };
            }
            return note(
                error.span().map_or(1, |span| line_of(text, span.start)),
                error.message().trim().to_owned(),
            );
        }
    };
    let Some((key, value)) = root
        .get_ref()
        .iter()
        .find(|(name, _)| name.get_ref().as_ref() == AUTO_TABLE)
    else {
        return AutoRead {
            rule: None,
            absent: true,
            notes: Vec::new(),
        };
    };
    match read_rule(text, key.span(), value) {
        Ok(rule) => AutoRead {
            rule: Some(rule),
            absent: false,
            notes: Vec::new(),
        },
        Err(error) => note(error.line.unwrap_or(1), error.message),
    }
}

/// One of `text`'s lines names the `auto` table as a header or a dotted
/// key (`[auto]`, `[ auto ]`, `["auto"]`, `auto.mode = …`, `auto = {…}`).
fn mentions_auto(text: &str) -> bool {
    text.lines().any(|line| {
        let rest = line.trim_start();
        let rest = rest.strip_prefix('[').map_or(rest, str::trim_start);
        let rest = rest
            .strip_prefix('"')
            .or_else(|| rest.strip_prefix('\''))
            .unwrap_or(rest);
        rest.strip_prefix(AUTO_TABLE).is_some_and(|after| {
            let after = after.trim_start_matches(['"', '\'']).trim_start();
            after.starts_with([']', '.', '='])
        })
    })
}

/// The table at `value` (its key at `key_span`), or the first problem by
/// line.
fn read_rule(
    text: &str,
    key_span: Range<usize>,
    value: &Spanned<DeValue<'_>>,
) -> Result<AutoRule, ProjectError> {
    let mut errors = Errors::new(text);
    let DeValue::Table(table) = value.get_ref() else {
        errors.push(value.span(), format!("{TABLE} is not a table"));
        return Err(first(errors));
    };
    let written = Written::read(table, &mut errors);
    // `[zones]` and `[ids]` from the same bytes: their own problem is the
    // table's, at its line.
    let declared = match zones_from_toml(text) {
        Ok(zones) => Some(zones),
        Err(error) => {
            errors.push(
                line_span(text, error.line.unwrap_or(1)),
                format!("`[zones]` does not read: {}", error.message),
            );
            None
        }
    };
    let kinds = match scheme_from_toml(text) {
        Ok(scheme) => {
            let mut kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for spec in scheme.prefixes() {
                kinds
                    .entry(spec.kind.clone())
                    .or_default()
                    .push(spec.prefix.clone());
            }
            Some(kinds)
        }
        Err(error) => {
            errors.push(
                line_span(text, error.line.unwrap_or(1)),
                format!("`[ids]` does not read: {}", error.message),
            );
            None
        }
    };
    let mode = match &written.mode {
        Some(Some((mode, at))) => match AutoMode::parse(mode) {
            Some(mode) => Some(mode),
            None => {
                errors.push(
                    at.clone(),
                    format!("`mode` is `{mode}`: one of off, local, hub, both"),
                );
                None
            }
        },
        Some(None) => None,
        None => {
            errors.push(key_span.clone(), required(TABLE, "mode"));
            None
        }
    };
    let zones = match &written.zones {
        Some(Some((list, at))) => {
            if list.is_empty() {
                errors.push(at.clone(), "`zones` lists no zone".to_owned());
            }
            zone_list(list, declared.as_ref(), &mut errors)
        }
        Some(None) => Vec::new(),
        None => {
            errors.push(key_span, required(TABLE, "zones"));
            Vec::new()
        }
    };
    let exclude_zones = match &written.exclude_zones {
        Some(Some((list, _))) => zone_list(list, declared.as_ref(), &mut errors),
        _ => Vec::new(),
    };
    let exclude_kinds = match &written.exclude_kinds {
        Some(Some((list, _))) => kind_list(list, kinds.as_ref(), &mut errors),
        _ => Vec::new(),
    };
    let branch = match &written.branch {
        Some(Some((branch, at))) => {
            if let Some(problem) = branch_problem(branch) {
                errors.push(at.clone(), problem);
            }
            branch.clone()
        }
        _ => DEFAULT_BRANCH.to_owned(),
    };
    let max_tasks = bounded(
        &written.max_tasks,
        "max_tasks",
        MAX_TASKS,
        DEFAULT_MAX_TASKS,
        &mut errors,
    );
    let max_applies = bounded(
        &written.max_applies,
        "max_applies",
        MAX_APPLIES,
        DEFAULT_MAX_APPLIES,
        &mut errors,
    );
    if let Some(error) = errors.first() {
        return Err(error);
    }
    let Some(mode) = mode else {
        return Err(unreadable());
    };
    Ok(AutoRule::assembled(
        mode,
        zones,
        exclude_zones,
        exclude_kinds,
        branch,
        max_tasks,
        max_applies,
    ))
}

/// A list of strings with each entry's span, and the list's span.
type Listed = (Vec<(String, Range<usize>)>, Range<usize>);

/// The table's keys as written: `None` unwritten, `Some(None)` of another
/// type (pushed already).
#[derive(Default)]
struct Written {
    mode: Option<Option<(String, Range<usize>)>>,
    zones: Option<Option<Listed>>,
    exclude_zones: Option<Option<Listed>>,
    exclude_kinds: Option<Option<Listed>>,
    branch: Option<Option<(String, Range<usize>)>>,
    max_tasks: Option<Option<(i128, String, Range<usize>)>>,
    max_applies: Option<Option<(i128, String, Range<usize>)>>,
}

impl Written {
    fn read(table: &DeTable<'_>, errors: &mut Errors<'_>) -> Self {
        let mut written = Self::default();
        for (key, value) in table.iter() {
            let list =
                |errors: &mut Errors<'_>| strings(value, errors).map(|list| (list, value.span()));
            match key.get_ref().as_ref() {
                "mode" => written.mode = Some(string(value, errors)),
                "zones" => written.zones = Some(list(errors)),
                "exclude_zones" => written.exclude_zones = Some(list(errors)),
                "exclude_kinds" => written.exclude_kinds = Some(list(errors)),
                "branch" => written.branch = Some(string(value, errors)),
                "max_tasks" => written.max_tasks = Some(integer(value, errors)),
                "max_applies" => written.max_applies = Some(integer(value, errors)),
                other => {
                    debug_assert!(!KEYS.contains(&other));
                    errors.push(key.span(), format!("unknown key `{other}` in {TABLE}"));
                }
            }
        }
        written
    }
}

/// The integer `value`, as written and parsed, with its span; else the
/// TOML reader's error (pushed).
fn integer(
    value: &Spanned<DeValue<'_>>,
    errors: &mut Errors<'_>,
) -> Option<(i128, String, Range<usize>)> {
    if let DeValue::Integer(number) = value.get_ref() {
        let shown = number.to_string();
        // Past `i128`: shown as written, judged out of range.
        let parsed = i128::from_str_radix(number.as_str(), number.radix()).unwrap_or(i128::MAX);
        return Some((parsed, shown, value.span()));
    }
    misread::<i64>(value, errors);
    None
}

/// `max_tasks` or `max_applies`: 1 to `max`, else its problem (pushed);
/// `default` unwritten or misread.
fn bounded(
    written: &Option<Option<(i128, String, Range<usize>)>>,
    key: &str,
    max: u32,
    default: u32,
    errors: &mut Errors<'_>,
) -> u32 {
    match written {
        Some(Some((number, shown, at))) => match u32::try_from(*number) {
            Ok(value) if (1..=max).contains(&value) => value,
            _ => {
                errors.push(at.clone(), format!("`{key}` is {shown}: 1 to {max}"));
                default
            }
        },
        _ => default,
    }
}

/// The zones `list` names, each declared in `declared` (unread: none
/// judged) and listed once; the problems pushed.
fn zone_list(
    list: &[(String, Range<usize>)],
    declared: Option<&BTreeMap<String, Zone>>,
    errors: &mut Errors<'_>,
) -> Vec<NamedGlobs> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(list.len());
    for (name, at) in list {
        if !seen.insert(name.clone()) {
            errors.push(at.clone(), format!("zone `{name}` is listed twice"));
            continue;
        }
        let Some(declared) = declared else {
            continue;
        };
        match declared.get(name) {
            Some(zone) => match NamedGlobs::of(name, &zone.globs) {
                Ok(named) => out.push(named),
                Err(problem) => errors.push(at.clone(), problem),
            },
            None => errors.push(
                at.clone(),
                format!("zone `{name}` is not declared in `[zones]`"),
            ),
        }
    }
    out
}

/// The kinds `list` names, each declared by an `[ids]` prefix (unread:
/// none judged) and listed once; the problems pushed.
fn kind_list(
    list: &[(String, Range<usize>)],
    kinds: Option<&BTreeMap<String, Vec<String>>>,
    errors: &mut Errors<'_>,
) -> Vec<KindPrefixes> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(list.len());
    for (kind, at) in list {
        if !seen.insert(kind.clone()) {
            errors.push(at.clone(), format!("kind `{kind}` is listed twice"));
            continue;
        }
        let Some(kinds) = kinds else {
            continue;
        };
        match kinds.get(kind) {
            Some(prefixes) => {
                let mut prefixes = prefixes.clone();
                prefixes.sort();
                out.push(KindPrefixes {
                    kind: kind.clone(),
                    prefixes,
                });
            }
            None => errors.push(
                at.clone(),
                format!("kind `{kind}` is declared by no `[ids]` prefix"),
            ),
        }
    }
    out
}

/// An empty span at the start of the 1-based `line` of `text` (past the
/// end: the end).
fn line_span(text: &str, line: usize) -> Range<usize> {
    let start = text
        .match_indices('\n')
        .nth(line.saturating_sub(2))
        .map_or(if line <= 1 { 0 } else { text.len() }, |(at, _)| at + 1);
    let start = if line <= 1 { 0 } else { start };
    start..start
}

fn first(errors: Errors<'_>) -> ProjectError {
    errors.first().unwrap_or_else(unreadable)
}

/// An error pushed and then not found: never (a guard, not a panic).
fn unreadable() -> ProjectError {
    ProjectError {
        line: Some(1),
        message: format!("{TABLE} does not read"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "[project]\nslug = \"auto-mini\"\n\n[ids]\nXR = { kind = \"requirement\", width = 2 }\nXS = { kind = \"scenario\", width = 3 }\nXD = { kind = \"decision\", width = 4 }\n\n[zones]\ncode = [\"src/**\"]\ntests = [\"tests/**\"]\ndocs = [\"docs/**\"]\nnarrative = [\"docs/narrative/**\"]\n";

    fn read(table: &str) -> AutoRead {
        let text = format!("{BASE}\n{table}");
        AutoRule::from_toml(text.as_bytes(), "specengine.toml")
    }

    fn note_of(table: &str) -> String {
        let read = read(table);
        assert!(read.rule.is_none(), "{table}: a rule");
        assert_eq!(read.notes.len(), 1, "{table}");
        read.notes[0].text()
    }

    #[test]
    fn absent_is_no_note() {
        let read = AutoRule::from_toml(BASE.as_bytes(), "specengine.toml");
        assert!(read.absent);
        assert!(read.rule.is_none());
        assert!(read.notes.is_empty());
    }

    #[test]
    fn canonical_and_hash_ignore_order() {
        let one = read(
            "[auto]\nmode = \"both\"\nzones = [\"code\", \"tests\", \"docs\"]\nexclude_zones = [\"narrative\"]\nexclude_kinds = [\"scenario\"]\n",
        );
        let two = read(
            "[auto]\n# reordered\nexclude_kinds = [\"scenario\"]\nzones = [\"tests\", \"docs\", \"code\"]\nexclude_zones = [ \"narrative\" ]\nmode = \"both\"\n",
        );
        let (one, two) = (one.rule.unwrap(), two.rule.unwrap());
        assert_eq!(one.canonical, two.canonical);
        assert_eq!(one.hash, two.hash);
        assert_eq!(
            one.canonical,
            "{\"format\":1,\"mode\":\"both\",\"zones\":{\"code\":[\"src/**\"],\"docs\":[\"docs/**\"],\"tests\":[\"tests/**\"]},\"exclude_zones\":{\"narrative\":[\"docs/narrative/**\"]},\"exclude_kinds\":{\"scenario\":[\"XS\"]},\"branch\":\"auto\",\"max_tasks\":10,\"max_applies\":20}"
        );
        assert!(one.hash.starts_with("b3:") && one.hash.len() == 67);
        let back = AutoRule::from_canonical(&one.canonical_form()).unwrap();
        assert_eq!(back.hash, one.hash);
    }

    #[test]
    fn a_glob_changes_the_hash() {
        let table = "[auto]\nmode = \"both\"\nzones = [\"docs\"]\n";
        let one = read(table).rule.unwrap();
        let text = format!(
            "{}\n{table}",
            BASE.replace("docs = [\"docs/**\"]", "docs = [\"doc/**\"]")
        );
        let two = AutoRule::from_toml(text.as_bytes(), "specengine.toml")
            .rule
            .unwrap();
        assert_ne!(one.hash, two.hash);
    }

    #[test]
    fn problems_are_one_note_at_their_line() {
        assert_eq!(
            note_of("[auto]\nzones = [\"code\"]\n"),
            "specengine.toml@HEAD:15: `[auto]` is required to hold `mode`; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"on\"\nzones = [\"code\"]\n"),
            "specengine.toml@HEAD:16: `mode` is `on`: one of off, local, hub, both; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"code\"]\nextra = 1\n"),
            "specengine.toml@HEAD:18: unknown key `extra` in `[auto]`; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = []\n"),
            "specengine.toml@HEAD:17: `zones` lists no zone; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"nope\"]\n"),
            "specengine.toml@HEAD:17: zone `nope` is not declared in `[zones]`; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"code\", \"code\"]\n"),
            "specengine.toml@HEAD:17: zone `code` is listed twice; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"code\"]\nexclude_kinds = [\"lore\"]\n"),
            "specengine.toml@HEAD:18: kind `lore` is declared by no `[ids]` prefix; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"code\"]\nmax_tasks = 0\n"),
            "specengine.toml@HEAD:18: `max_tasks` is 0: 1 to 100; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"code\"]\nmax_applies = 1001\n"),
            "specengine.toml@HEAD:18: `max_applies` is 1001: 1 to 1000; auto is off"
        );
        assert_eq!(
            note_of("[auto]\nmode = \"both\"\nzones = [\"code\"]\nbranch = \"a..b\"\n"),
            "specengine.toml@HEAD:18: `branch` `a..b` is no branch name (letters, digits, `.`, `_`, `-`, `/`; 1 to 64 bytes); auto is off"
        );
        let top = format!("auto = 3\n{BASE}");
        let read = AutoRule::from_toml(top.as_bytes(), "specengine.toml");
        assert_eq!(
            read.notes[0].text(),
            "specengine.toml@HEAD:1: `[auto]` is not a table; auto is off"
        );
        assert!(
            note_of("[auto]\nmode = 3\nzones = [\"code\"]\n")
                .starts_with("specengine.toml@HEAD:16: invalid type")
        );
    }

    #[test]
    fn branch_names() {
        for good in ["auto", "auto/night", "a-b_c.d", "x1"] {
            assert!(branch_problem(good).is_none(), "{good}");
        }
        for bad in [
            "", "-a", ".a", "/a", "a..b", "a//b", "a/", "a.", "a.lock", "a b", "ä",
        ] {
            assert!(branch_problem(bad).is_some(), "{bad}");
        }
        assert!(branch_problem(&"a".repeat(65)).is_some());
    }

    #[test]
    fn prefixes() {
        assert_eq!(id_prefix("XR-01"), Some("XR"));
        assert_eq!(id_prefix("slug/XS-001"), Some("XS"));
        assert_eq!(id_prefix("docs/x.md"), None);
    }
}
