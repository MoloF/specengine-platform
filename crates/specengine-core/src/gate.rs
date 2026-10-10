//! The pure half of the task gate (`docs/canon/gate.md` "Rule", "Config",
//! "Verdict"; ADR-0037): the `[gate]` and `[zones]` tables of
//! `specengine.toml`, read apart from the reads ([`GateConfig::from_toml`];
//! [`crate::ProjectConfig::from_toml`] only name-checks both, so a bad
//! `[gate]` fails no read), which root-relative paths they control
//! ([`GateConfig::controlled`]), the lexical fold of a path ([`fold`]) and
//! the opening rule over the tasks the store read ([`opens`]). Nothing
//! here touches a file, git or the queue.
//!
//! - **Tables**: no `[gate]` is off and `[zones]` stays unread; under
//!   `mode = "off"` only `[gate]`'s keys and its mode are read. Otherwise
//!   `mode` is one of the three, `zones` lists declared zones, each once;
//!   every `[zones]` entry is a name of `[a-z0-9_-]`, 1 to
//!   [`ZONE_NAME_MAX`] bytes, with at least one glob in the dialect of
//!   `[paths] exclude` (root-relative, no `.` or `..` component, no
//!   `[`, `]`, `{`, `}` or `\`: another dialect's syntax would be taken
//!   literally here); `[project] slug` names the queue.
//! - **Errors**: a [`ProjectError`] at the line (`file:line` through
//!   [`ProjectError::at`]). A file that is not TOML: an error when one of
//!   its lines names the table ([`mentions_gate`]), else off.
//! - **Controlled** while the mode is not off: a path whose last component
//!   is the config file's name in any ASCII case, at any depth (a new
//!   project config below the root would re-root the files under it),
//!   else the first listed zone with a glob matching it.
//! - The core knows no zone by name: every name comes from the config.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde::de::IgnoredAny;
use specengine_model::{TaskClaim, TaskStatus};
use toml::Spanned;

use crate::glob::Glob;
use crate::paths_toml::checked_path;
use crate::project_toml::{ProjectError, slug_problem};
use crate::task::task_number;

/// The longest zone name, in bytes (a name is ASCII).
pub const ZONE_NAME_MAX: usize = 64;

/// The characters a zone glob may not hold: other dialects' classes,
/// alternations and escapes, which this one would match literally.
const GLOB_RESERVED: [char; 5] = ['[', ']', '{', '}', '\\'];

/// `[gate] keys` built later (07 s5): named as such when written.
const NOT_BUILT_KEYS: [&str; 2] = ["min_files", "contours"];

/// `[gate] mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateMode {
    /// Passes every write, reading nothing more.
    Off,
    /// Passes every write and records an uncovered one.
    Observe,
    /// Refuses an uncovered write.
    Selective,
}

impl GateMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Observe, Self::Selective];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Observe => "observe",
            Self::Selective => "selective",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == text)
    }
}

impl fmt::Display for GateMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One controlled zone: its name and globs as written, compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub name: String,
    pub globs: Vec<String>,
    compiled: Vec<Glob>,
}

/// `[gate]` with the zones it controls, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateConfig {
    pub mode: GateMode,
    /// The controlled zones in `[gate] zones` order; empty when off.
    pub zones: Vec<Zone>,
    /// `[project] slug`, the queue's name; `None` only when off.
    pub slug: Option<String>,
}

/// Why a path is controlled: the first listed zone it matches, `None` for
/// the config file itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub zone: Option<String>,
}

impl GateConfig {
    /// Reads `[gate]` and, when the gate is on, `[zones]` and `[project]
    /// slug` (see the module documentation). `None`: no `[gate]` table, or
    /// a file that is not TOML without a `[gate]` line.
    pub fn from_toml(text: &str) -> Result<Option<Self>, ProjectError> {
        gate_from_toml(text)
    }

    /// Whether the root-relative `rel` (`/`-joined, folded) is controlled
    /// while the mode is not off: a path whose last component is
    /// `config_name` (the config file's name) ignoring ASCII case, at any
    /// depth, the root's own config included (zone `None`); else the
    /// first listed zone with a glob matching it, globs compared
    /// case-sensitively.
    pub fn controlled(&self, rel: &str, config_name: &str) -> Option<Control> {
        if self.mode == GateMode::Off {
            return None;
        }
        let last = rel.rsplit('/').next().unwrap_or(rel);
        if last.eq_ignore_ascii_case(config_name) {
            return Some(Control { zone: None });
        }
        self.zones
            .iter()
            .find(|zone| zone.compiled.iter().any(|glob| glob.matches(rel)))
            .map(|zone| Control {
                zone: Some(zone.name.clone()),
            })
    }
}

/// A task as the gate reads it: its ID, state and claim, nothing more
/// (no title, goal, proposal or `stale`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateTask {
    pub id: String,
    pub status: TaskStatus,
    pub claim: Option<TaskClaim>,
}

/// The lowest-numbered task that opens a gated write on `branch` of the
/// file's worktree (`docs/canon/gate.md` "Rule"): `in_progress`, claimed
/// on that branch in a worktree `here` takes as the file's (the caller
/// compares directories, never strings). `tasks` are the repository's
/// already; a detached `HEAD` (`branch` `None`) opens nothing. A reported
/// run keeps the task open: runs are not read.
pub fn opens<'t>(
    tasks: &'t [GateTask],
    branch: Option<&str>,
    here: impl Fn(&str) -> bool,
) -> Option<&'t GateTask> {
    let branch = branch?;
    tasks
        .iter()
        .filter(|task| task.status == TaskStatus::InProgress)
        .filter(|task| {
            task.claim
                .as_ref()
                .is_some_and(|claim| claim.branch == branch && here(&claim.worktree))
        })
        .min_by_key(|task| task_number(&task.id).unwrap_or(u64::MAX))
}

/// `path` with `.` components dropped and each `..` folding the component
/// before it (at the root it stays the root), lexically: no file is read.
pub fn fold(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match parts.last() {
                Some(Component::Normal(_)) => {
                    parts.pop();
                }
                // `/..` is `/`.
                Some(Component::RootDir | Component::Prefix(_)) => {}
                // A relative path keeps its leading `..`.
                _ => parts.push(component),
            },
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

/// [`GateConfig::from_toml`] as a function.
pub fn gate_from_toml(text: &str) -> Result<Option<GateConfig>, ProjectError> {
    let error_at = |span: Option<Range<usize>>, message: String| ProjectError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    if let Err(error) = toml::from_str::<IgnoredAny>(text) {
        if !mentions_gate(text) {
            return Ok(None);
        }
        return Err(error_at(
            error.span(),
            format!(
                "the file is not TOML ({}), and a line of it names `[gate]`",
                error.message().trim()
            ),
        ));
    }
    let raw: RawGateFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(table) = raw.gate else {
        return Ok(None);
    };
    let table_span = table.span();
    let table = table.into_inner();

    let mut mode = None;
    let mut listed = None;
    for (key, value) in &table {
        match key.as_str() {
            "mode" => mode = Some(value),
            "zones" => listed = Some(value),
            other if NOT_BUILT_KEYS.contains(&other) => {
                return Err(error_at(
                    Some(value.span()),
                    format!(
                        "`[gate] {other}` is not built: `[gate]` takes `mode` and `zones` only"
                    ),
                ));
            }
            other => {
                return Err(error_at(
                    Some(value.span()),
                    format!("`[gate]` has no key `{other}`: it takes `mode` and `zones` only"),
                ));
            }
        }
    }
    let Some(mode_value) = mode else {
        return Err(error_at(
            Some(table_span),
            "`[gate]` has no `mode`: one of `off`, `observe`, `selective`".to_owned(),
        ));
    };
    let mode = match mode_value.get_ref() {
        toml::Value::String(text) => GateMode::parse(text).ok_or_else(|| {
            let built = if text == "strict" {
                " (`strict` is not built)"
            } else {
                ""
            };
            error_at(
                Some(mode_value.span()),
                format!(
                    "`[gate] mode`: {text:?} is no mode: `off`, `observe` or `selective`{built}"
                ),
            )
        })?,
        other => {
            return Err(error_at(
                Some(mode_value.span()),
                format!(
                    "`[gate] mode` is a {}, not a string: `off`, `observe` or `selective`",
                    other.type_str()
                ),
            ));
        }
    };
    let names = match listed {
        None => Vec::new(),
        Some(value) => match value.get_ref() {
            toml::Value::Array(items) => {
                let mut names = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        toml::Value::String(name) => names.push(name.clone()),
                        other => {
                            return Err(error_at(
                                Some(value.span()),
                                format!(
                                    "`[gate] zones` holds a {}: it lists zone names",
                                    other.type_str()
                                ),
                            ));
                        }
                    }
                }
                names
            }
            other => {
                return Err(error_at(
                    Some(value.span()),
                    format!(
                        "`[gate] zones` is a {}, not a list of zone names",
                        other.type_str()
                    ),
                ));
            }
        },
    };
    if mode == GateMode::Off {
        return Ok(Some(GateConfig {
            mode,
            zones: Vec::new(),
            slug: None,
        }));
    }
    let listed_span = listed.map_or(table_span.clone(), Spanned::span);
    if names.is_empty() {
        return Err(error_at(
            Some(listed_span),
            format!(
                "`[gate] zones` is missing or empty: mode `{mode}` controls the zones it lists"
            ),
        ));
    }

    let raw: RawZonesFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let declared = match raw.zones {
        Some(zones) => zones.into_inner(),
        None => BTreeMap::new(),
    };
    let mut checked: BTreeMap<String, Zone> = BTreeMap::new();
    for (name, globs) in declared {
        let span = globs.span();
        if let Some(problem) = zone_name_problem(&name) {
            return Err(error_at(Some(span), format!("`[zones] {name}`: {problem}")));
        }
        let globs = globs.into_inner();
        if globs.is_empty() {
            return Err(error_at(
                Some(span),
                format!("`[zones] {name}` has no glob: a zone names at least one"),
            ));
        }
        let mut written = Vec::with_capacity(globs.len());
        let mut compiled = Vec::with_capacity(globs.len());
        for glob in globs {
            let glob_span = glob.span();
            let glob = glob.into_inner();
            if let Some(problem) = glob_problem(&glob) {
                return Err(error_at(
                    Some(glob_span),
                    format!("`[zones] {name}`: {problem}"),
                ));
            }
            compiled.push(Glob::new(&glob));
            written.push(glob);
        }
        checked.insert(
            name.clone(),
            Zone {
                name,
                globs: written,
                compiled,
            },
        );
    }
    let mut seen = BTreeSet::new();
    let mut zones = Vec::with_capacity(names.len());
    for name in names {
        if !seen.insert(name.clone()) {
            return Err(error_at(
                Some(listed_span),
                format!("`[gate] zones` lists `{name}` twice"),
            ));
        }
        let zone = checked.get(&name).ok_or_else(|| {
            error_at(
                Some(listed_span.clone()),
                format!("`[gate] zones` lists `{name}`, which `[zones]` does not declare"),
            )
        })?;
        zones.push(zone.clone());
    }

    let raw: RawProjectFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let slug = match raw.project {
        None => {
            return Err(error_at(
                Some(table_span),
                "`[gate]` needs `[project] slug`: the task queue is named by it".to_owned(),
            ));
        }
        Some(project) => {
            let project_span = project.span();
            match project.into_inner().slug {
                None => {
                    return Err(error_at(
                        Some(project_span),
                        "`[project]` has no `slug`: the gate's task queue is named by it"
                            .to_owned(),
                    ));
                }
                Some(slug) => {
                    let span = slug.span();
                    let slug = slug.into_inner();
                    if let Some(problem) = slug_problem(&slug) {
                        return Err(error_at(Some(span), format!("`slug`: {problem}")));
                    }
                    slug
                }
            }
        }
    };
    Ok(Some(GateConfig {
        mode,
        zones,
        slug: Some(slug),
    }))
}

/// One of `text`'s lines names the `gate` table, as a header or a key, the
/// name bare or quoted: after leading blanks, an optional `[` and blanks,
/// `gate` (or `"gate"`, `'gate'`), blanks, then `]`, `.` or `=` (`[gate]`,
/// `[ gate ]`, `["gate"]`, `[gate.x]`, `gate.mode = …`, `gate = {…}`).
/// The test of a file that is not TOML, and the wrapper's (`grep -E`) when
/// `spec` gives no verdict: wider than the table, never narrower.
pub fn mentions_gate(text: &str) -> bool {
    text.lines().any(|line| {
        let rest = line.trim_start();
        let rest = rest.strip_prefix('[').map_or(rest, str::trim_start);
        let rest = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => match rest[1..].strip_prefix("gate") {
                Some(after) => after.strip_prefix(quote).unwrap_or(after),
                None => return false,
            },
            _ => match rest.strip_prefix("gate") {
                Some(after) => after,
                None => return false,
            },
        };
        matches!(rest.trim_start().chars().next(), Some(']' | '.' | '='))
    })
}

/// Why `name` is no zone name; `None`: it is one.
fn zone_name_problem(name: &str) -> Option<String> {
    if name.is_empty() || name.len() > ZONE_NAME_MAX {
        return Some(format!(
            "a zone name has 1 to {ZONE_NAME_MAX} bytes, not {}",
            name.len()
        ));
    }
    let valid = name.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    });
    (!valid)
        .then(|| format!("{name:?} is no zone name: lower-case ASCII letters, digits, `_` and `-`"))
}

/// Why `glob` is no zone glob; `None`: it is one.
fn glob_problem(glob: &str) -> Option<String> {
    dialect_problem(glob, "a zone glob")
}

/// Why `glob` is outside the `[paths] exclude` dialect (root-relative, `*`,
/// `**`, `?`, every other character literal); `None`: it is in it.
/// `noun` names the glob in the message (`a zone glob`; `[code] exclude`'s
/// too, task spec `code-refs`).
pub(crate) fn dialect_problem(glob: &str, noun: &str) -> Option<String> {
    if let Err(problem) = checked_path(glob, false) {
        return Some(problem);
    }
    if let Some(reserved) = glob.chars().find(|c| GLOB_RESERVED.contains(c)) {
        return Some(format!(
            "{glob:?} holds `{reserved}`: {noun} takes `*`, `**` and `?`, every other \
             character literally, so `[`, `]`, `{{`, `}}` and `\\` are refused"
        ));
    }
    if glob.chars().any(char::is_control) {
        return Some(format!("{glob:?} holds a control character"));
    }
    None
}

/// Only `[gate]`, its keys read here.
#[derive(Deserialize)]
struct RawGateFile {
    #[serde(default)]
    gate: Option<Spanned<BTreeMap<String, Spanned<toml::Value>>>>,
}

/// `[zones]` as written: name → its globs, each with its span.
type RawZones = BTreeMap<String, Spanned<Vec<Spanned<String>>>>;

/// Only `[zones]`.
#[derive(Deserialize)]
struct RawZonesFile {
    #[serde(default)]
    zones: Option<Spanned<RawZones>>,
}

/// Only `[project] slug`; the reads check the rest of `[project]`.
#[derive(Deserialize)]
struct RawProjectFile {
    #[serde(default)]
    project: Option<Spanned<RawProject>>,
}

#[derive(Deserialize)]
struct RawProject {
    #[serde(default)]
    slug: Option<Spanned<String>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
