//! `[[workspace]]` and `[[module]]` of `specengine.toml` (task spec
//! `workspaces`, "Data"; `docs/canon/workspaces.md` "The lens"): a trade's
//! lens on a project, and the modules it may scope its reads to, read only
//! by `spec workspace` and its daemon twin. Of [`crate::card_toml`]'s
//! family, read apart: [`crate::ProjectConfig::from_toml`] only name-checks
//! both keys, so neither table fails a read, a command, a check or a gate.
//!
//! A module is a root document and its subtree, or a direction
//! (`docs/features/departments.md` "Acceptance criteria", DP-2): the nodes
//! of the kinds its `kinds` lists, with no root. It holds exactly one of
//! `root` and `kinds`; a kind is the project's word, never checked against
//! `[ids]` here.
//!
//! The reader never fails. A malformed table is replaced by its default,
//! with one note, the first problem by line: `[[module]]` by no module,
//! `[[workspace]]` by the implicit workspace ([`IMPLICIT_NAME`], the
//! [`IMPLICIT_SECTIONS`], landing on the [`OVERVIEW`], every flow, no
//! module). In a well-formed `[[workspace]]` a value naming what is not
//! there (a section, a landing, a `[[card.flow]]`, a module) is left out
//! with a note, and the rest is served.
//!
//! - **Malformed**: not an array of tables, an unknown key or a wrong type
//!   (the TOML reader's message), a missing `name`, a module with both or
//!   neither of `root` and `kinds`, a bad name ([`name_problem`]), title,
//!   root or kind, a kind twice, a name twice, more than [`MAX_MODULES`]
//!   modules, [`MAX_WORKSPACES`] workspaces or [`MAX_LIST`] entries in a
//!   list.
//! - **Judged, per workspace in config order**: `sections`, `landing`,
//!   `flows` (against [`card_from_toml`]; a `[card]` that does not read
//!   serves the lists as written, one note for the document), `module`.
//! - **Served from `[card]`**, read once: each workspace's `disciplines`
//!   ([`disciplines_of`], DP-3 of the same criteria), none when `[card]`
//!   does not read, with no note of their own.
//! - **`logins`, judged after the rest** (task spec `hub-department` HD5):
//!   no array of strings, or more than [`MAX_LOGINS`] entries, is malformed;
//!   then each entry, in config order, is left out with a note when
//!   [`hub_login_problem`] refuses it, its workspace lists it twice or an
//!   earlier workspace lists it. The kept ones make
//!   [`WorkspaceConfig::logins`], never a [`Workspace`]'s: a note names an
//!   entry's position, never its login (HD6), since every login reads the
//!   notes.
//!
//! The section IDs are the engine's closed vocabulary, the UI's
//! `ui/src/app/sections.ts` in its order: nothing here names a trade, and
//! every workspace, module and flow name is data, compared only with
//! another name of the same file (ADR-0008, ADR-0073). Pure: the text in,
//! the document out; a note's line is its offending value's, a missing
//! key's table header's, else 1.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use toml::Spanned;
use toml::de::{DeTable, DeValue, ValueDeserializer};

use crate::card::hub_login_problem;
use crate::card_toml::{CardConfig, card_from_toml, name_problem, title_problem};
use crate::health_toml::{Errors, line_of, type_name, unknown_field};
use crate::project_toml::ProjectError;
use crate::team_toml::TEAM_ENTRIES_MAX;

/// The section IDs, as `ui/src/app/sections.ts` lists them, in its order.
pub const SECTIONS: [&str; 9] = [
    "inbox",
    "tasks",
    "board",
    "roadmap",
    "flow",
    "tree",
    "graph",
    "health",
    "questions",
];

/// [`SECTIONS`] without `tasks`: the implicit workspace's nav.
pub const IMPLICIT_SECTIONS: [&str; 8] = [
    "inbox",
    "board",
    "roadmap",
    "flow",
    "tree",
    "graph",
    "health",
    "questions",
];

/// The landing every workspace may take, whatever its sections.
pub const OVERVIEW: &str = "overview";
/// The implicit workspace's name.
pub const IMPLICIT_NAME: &str = "all";
/// The implicit workspace's title.
pub const IMPLICIT_TITLE: &str = "All sections";

/// The most `[[workspace]]` tables.
pub const MAX_WORKSPACES: usize = 32;
/// The most `[[module]]` tables.
pub const MAX_MODULES: usize = 64;
/// The most entries of a workspace's `sections` or `flows`.
pub const MAX_LIST: usize = 32;
/// The longest module `root`, in bytes.
pub const MAX_ROOT_BYTES: usize = 256;
/// The longest entry of a direction's `kinds`, in bytes; the list holds at
/// most [`MAX_LIST`].
pub const MAX_KIND_BYTES: usize = 64;
/// The most entries of a workspace's `logins`: as many as `[team]` may hold
/// ([`TEAM_ENTRIES_MAX`]).
pub const MAX_LOGINS: usize = TEAM_ENTRIES_MAX;

/// The top-level key of the workspaces.
const WORKSPACE: &str = "workspace";
/// The top-level key of the modules.
const MODULE: &str = "module";
/// A workspace's keys, in the order a message lists them.
const WORKSPACE_KEYS: [&str; 7] = [
    "name", "title", "sections", "landing", "flows", "module", "logins",
];
/// A module's keys, in the order a message lists them.
const MODULE_KEYS: [&str; 4] = ["name", "title", "root", "kinds"];
/// How a message names a workspace table.
const WORKSPACE_TABLE: &str = "`[[workspace]]`";
/// How a message names a module table.
const MODULE_TABLE: &str = "`[[module]]`";

/// Where the served workspaces come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceSource {
    /// The file's `[[workspace]]` tables.
    Config,
    /// No `[[workspace]]`, or a malformed one: the implicit workspace.
    Implicit,
}

impl WorkspaceSource {
    /// As the document sends it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Implicit => "implicit",
        }
    }
}

/// One workspace, judged: the nav's sections in its order, its landing, the
/// card flows its Board shows (`None`: every flow) and its default module.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Workspace {
    pub name: String,
    pub title: String,
    /// [`OVERVIEW`] or one of `sections`.
    pub landing: String,
    /// Each a [`SECTIONS`] ID, once.
    pub sections: Vec<String>,
    /// Each a `[[card.flow]]` name, once; `None`: every flow.
    pub flows: Option<Vec<String>>,
    /// The disciplines a new card can take into one of `flows`
    /// ([`disciplines_of`]); empty when `[card]` does not read.
    pub disciplines: Vec<String>,
    /// A served module's name.
    pub module: Option<String>,
}

impl Workspace {
    /// The implicit workspace: every section but `tasks`, on the Overview,
    /// every flow, no module; its `disciplines` empty until the reader fills
    /// them from `[card]` ([`disciplines_of`] over every flow).
    pub fn implicit() -> Self {
        Self {
            name: IMPLICIT_NAME.to_owned(),
            title: IMPLICIT_TITLE.to_owned(),
            landing: OVERVIEW.to_owned(),
            sections: IMPLICIT_SECTIONS.map(str::to_owned).to_vec(),
            flows: None,
            disciplines: Vec::new(),
            module: None,
        }
    }
}

/// One module: a root document and its subtree, or a direction, the nodes
/// of the kinds it lists, with no root. Exactly one of `root` and `kinds`
/// is `Some`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct WorkspaceModule {
    pub name: String,
    pub title: String,
    /// A REF in any `spec show` form, resolved by the read it roots;
    /// `None` for a direction.
    pub root: Option<String>,
    /// A direction's kinds, as written: matched exactly against a node's
    /// `kind` by the reads that take one, never against `[ids]`; `None` for
    /// a rooted module.
    pub kinds: Option<Vec<String>>,
}

/// What the reader left out or replaced: its 1-based line and why.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceNote {
    pub line: usize,
    pub message: String,
}

impl WorkspaceNote {
    /// `<file>:<line>: <message>`.
    pub fn at(&self, file: &str) -> String {
        format!("{file}:{}: {}", self.line, self.message)
    }
}

/// `[[workspace]]` and `[[module]]`, read: at least one workspace, the
/// notes sorted by line, ties in judging order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub source: WorkspaceSource,
    pub workspaces: Vec<Workspace>,
    pub modules: Vec<WorkspaceModule>,
    pub notes: Vec<WorkspaceNote>,
    /// Each kept `logins` entry and the workspace it lands in, the first
    /// listing it in config order (task spec `hub-department` HD5); empty
    /// for the implicit workspace. Never served: a read gives one login's
    /// [`Self::default_for`] alone (HD6).
    pub logins: BTreeMap<String, String>,
}

impl WorkspaceConfig {
    /// Reads both tables from `text` (see the module documentation).
    pub fn from_toml(text: &str) -> Self {
        workspace_from_toml(text)
    }

    /// Each note as `<file>:<line>: <message>`, in order.
    pub fn notes_at(&self, file: &str) -> Vec<String> {
        self.notes.iter().map(|note| note.at(file)).collect()
    }

    /// The workspace whose `logins` holds `login` byte for byte, else
    /// `None` (task spec `hub-department` HD1): never trimmed or folded.
    pub fn default_for(&self, login: &str) -> Option<&str> {
        self.logins.get(login).map(String::as_str)
    }
}

/// [`WorkspaceConfig::from_toml`] as a function.
pub fn workspace_from_toml(text: &str) -> WorkspaceConfig {
    let mut notes = Vec::new();
    let root = match DeTable::parse(text) {
        Ok(root) => root,
        // `ProjectConfig` refuses such a file first; the reader still answers.
        Err(error) => {
            notes.push(WorkspaceNote {
                line: error.span().map_or(1, |span| line_of(text, span.start)),
                message: format!(
                    "{}; the implicit workspace is served",
                    error.message().trim()
                ),
            });
            // `[card]` does not read either: no discipline is served.
            return implicit(Vec::new(), None, notes);
        }
    };
    let top = |key: &str| {
        root.get_ref()
            .iter()
            .find(|(name, _)| name.get_ref().as_ref() == key)
            .map(|(_, value)| value)
    };
    let modules = match read_modules(text, top(MODULE)) {
        Ok(modules) => modules,
        Err(error) => {
            notes.push(fell_back(&error, "no module is served"));
            Vec::new()
        }
    };
    let written = match read_workspaces(text, top(WORKSPACE)) {
        Ok(written) => written,
        Err(error) => {
            notes.push(fell_back(&error, "the implicit workspace is served"));
            None
        }
    };
    // `[card]`, read once: the flows `flows` are judged against and the
    // disciplines each workspace serves. Its problem is noted only where a
    // workspace's `flows` are judged; elsewhere it serves no discipline.
    let card = card_from_toml(text);
    let Some(written) = written else {
        return implicit(modules, card.as_ref().ok(), notes);
    };
    let mut judge = Judge {
        text,
        modules: &modules,
        card: card.as_ref(),
        card_noted: false,
        notes: &mut notes,
        logins: BTreeMap::new(),
    };
    let workspaces = written
        .into_iter()
        .map(|workspace| judge.workspace(workspace))
        .collect();
    let logins = judge.logins;
    notes.sort_by_key(|note| note.line);
    WorkspaceConfig {
        source: WorkspaceSource::Config,
        workspaces,
        modules,
        notes,
        logins,
    }
}

/// The implicit document over `modules`, `notes` sorted; its workspace's
/// `disciplines` those of every flow of `card` (none when it does not read).
fn implicit(
    modules: Vec<WorkspaceModule>,
    card: Option<&CardConfig>,
    mut notes: Vec<WorkspaceNote>,
) -> WorkspaceConfig {
    notes.sort_by_key(|note| note.line);
    let mut workspace = Workspace::implicit();
    if let Some(card) = card {
        workspace.disciplines = disciplines_of(card, None);
    }
    WorkspaceConfig {
        source: WorkspaceSource::Implicit,
        workspaces: vec![workspace],
        modules,
        notes,
        logins: BTreeMap::new(),
    }
}

/// The disciplines a department shows (`docs/features/departments.md`
/// "Acceptance criteria", DP-3): its flows in its order (`None`: every flow
/// of `card`, in config order), each flow's disciplines as written, each
/// once, a discipline kept only when the flow a new card of it takes
/// ([`CardConfig::flow_for`], the first flow listing it) is one of those
/// flows. A name `card` does not hold contributes nothing.
pub fn disciplines_of(card: &CardConfig, flows: Option<&[String]>) -> Vec<String> {
    let shown: Vec<&str> = match flows {
        None => card.flows.iter().map(|flow| flow.name.as_str()).collect(),
        Some(names) => names.iter().map(String::as_str).collect(),
    };
    let mut kept: Vec<String> = Vec::new();
    for flow in shown.iter().filter_map(|name| card.flow(name)) {
        for discipline in &flow.disciplines {
            if kept.contains(discipline) {
                continue;
            }
            let placed = card
                .flow_for(Some(discipline))
                .is_ok_and(|first| shown.contains(&first.name.as_str()));
            if placed {
                kept.push(discipline.clone());
            }
        }
    }
    kept
}

/// A malformed table's note: its first problem, then what is served.
fn fell_back(error: &ProjectError, served: &str) -> WorkspaceNote {
    WorkspaceNote {
        line: error.line.unwrap_or(1),
        message: format!("{}; {served}", error.message),
    }
}

// ------------------------------------------------------------- the form

/// A string as written, and its span.
type Written = (String, Range<usize>);

/// One well-formed `[[workspace]]`, its values as written.
struct WrittenWorkspace {
    name: String,
    title: Option<String>,
    sections: Option<Vec<Written>>,
    landing: Option<Written>,
    flows: Option<Vec<Written>>,
    module: Option<Written>,
    logins: Option<Vec<Written>>,
}

/// `[[module]]`: the modules, or the first problem by line.
fn read_modules(
    text: &str,
    value: Option<&Spanned<DeValue<'_>>>,
) -> Result<Vec<WorkspaceModule>, ProjectError> {
    let mut errors = Errors::new(text);
    let mut modules = Vec::new();
    let mut names = BTreeSet::new();
    for (span, entry) in tables(value, MODULE_TABLE, MAX_MODULES, &mut errors) {
        // A required key: `Some` once written, its value `None` when of
        // another type (pushed), so it is not also missing. `root` and
        // `kinds` alike: exactly one of the two is required.
        let mut name = None;
        let mut title = None;
        let mut root = None;
        let mut kinds = None;
        for (key, value) in entry.iter() {
            match key.get_ref().as_ref() {
                "name" => name = Some(string(value, &mut errors)),
                "title" => title = string(value, &mut errors),
                "root" => root = Some(string(value, &mut errors)),
                "kinds" => kinds = Some((value.span(), strings(value, &mut errors))),
                other => errors.push(key.span(), unknown_field(other, &MODULE_KEYS)),
            }
        }
        let written_name = match &name {
            Some(Some((name, _))) => Some(name.clone()),
            _ => None,
        };
        let name = named(&span, name, MODULE_TABLE, &mut names, &mut errors);
        if let Some((title, at)) = &title
            && let Some(problem) = title_problem(title)
        {
            errors.push(at.clone(), problem);
        }
        let both = root.is_some() && kinds.is_some();
        if root.is_none() && kinds.is_none() {
            errors.push(
                span,
                format!("{MODULE_TABLE} is required to hold `root` or `kinds`"),
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
        let kinds = match kinds {
            Some((at, listed)) => {
                if both {
                    let named_as =
                        written_name.map_or_else(String::new, |name| format!(" `{name}`"));
                    errors.push(
                        at.clone(),
                        format!(
                            "{MODULE_TABLE}{named_as} holds both `root` and `kinds`: a module \
                             holds one"
                        ),
                    );
                }
                listed.and_then(|listed| kinds_of(at, listed, &mut errors))
            }
            None => None,
        };
        if let Some(name) = name
            && root.is_some() != kinds.is_some()
        {
            modules.push(WorkspaceModule {
                title: title.map_or_else(|| name.clone(), |(title, _)| title),
                name,
                root,
                kinds,
            });
        }
    }
    match errors.first() {
        Some(error) => Err(error),
        None => Ok(modules),
    }
}

/// `[[workspace]]`: `None` absent or empty, else the tables as written, or
/// the first problem by line.
fn read_workspaces(
    text: &str,
    value: Option<&Spanned<DeValue<'_>>>,
) -> Result<Option<Vec<WrittenWorkspace>>, ProjectError> {
    let mut errors = Errors::new(text);
    let mut written = Vec::new();
    let mut names = BTreeSet::new();
    for (span, entry) in tables(value, WORKSPACE_TABLE, MAX_WORKSPACES, &mut errors) {
        // `name` as a module's: `Some` once written.
        let mut name = None;
        let mut title = None;
        let mut sections = None;
        let mut landing = None;
        let mut flows = None;
        let mut module = None;
        let mut logins = None;
        for (key, value) in entry.iter() {
            match key.get_ref().as_ref() {
                "name" => name = Some(string(value, &mut errors)),
                "title" => title = string(value, &mut errors),
                "sections" => sections = strings(value, &mut errors),
                "landing" => landing = string(value, &mut errors),
                "flows" => flows = strings(value, &mut errors),
                "module" => module = string(value, &mut errors),
                "logins" => logins = logins_of(value, &mut errors),
                other => errors.push(key.span(), unknown_field(other, &WORKSPACE_KEYS)),
            }
        }
        let name = named(&span, name, WORKSPACE_TABLE, &mut names, &mut errors);
        if let Some((title, at)) = &title
            && let Some(problem) = title_problem(title)
        {
            errors.push(at.clone(), problem);
        }
        if let Some(name) = name {
            written.push(WrittenWorkspace {
                name,
                title: title.map(|(title, _)| title),
                sections,
                landing,
                flows,
                module,
                logins,
            });
        }
    }
    match errors.first() {
        Some(error) => Err(error),
        None => Ok((!written.is_empty()).then_some(written)),
    }
}

/// The tables of the array `value` (absent: none), each with its span, at
/// most `max`; whatever else, an error each.
fn tables<'v, 'i>(
    value: Option<&'v Spanned<DeValue<'i>>>,
    table: &str,
    max: usize,
    errors: &mut Errors<'_>,
) -> Vec<(Range<usize>, &'v DeTable<'i>)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let DeValue::Array(items) = value.get_ref() else {
        misread::<Vec<IgnoredAny>>(value, errors);
        return Vec::new();
    };
    let mut out = Vec::with_capacity(items.len().min(max));
    for (at, item) in items.iter().enumerate() {
        if at == max {
            errors.push(item.span(), format!("at most {max} {table} tables"));
            break;
        }
        match item.get_ref() {
            DeValue::Table(entry) => out.push((item.span(), entry)),
            _ => misread::<BTreeMap<String, IgnoredAny>>(item, errors),
        }
    }
    out
}

/// A table's `name`: required, a [`name_problem`] name, once among
/// `names`; `None` when it is none of these (pushed). `name` is `None`
/// unwritten, `Some(None)` of another type (pushed already).
fn named(
    table_span: &Range<usize>,
    name: Option<Option<Written>>,
    table: &str,
    names: &mut BTreeSet<String>,
    errors: &mut Errors<'_>,
) -> Option<String> {
    let (name, at) = match name {
        Some(Some(written)) => written,
        Some(None) => return None,
        None => {
            errors.push(table_span.clone(), required(table, "name"));
            return None;
        }
    };
    if let Some(problem) = name_problem(&name) {
        errors.push(at, problem);
        return None;
    }
    if !names.insert(name.clone()) {
        errors.push(at, format!("{table} `{name}` is named twice"));
        return None;
    }
    Some(name)
}

/// A missing key's message.
fn required(table: &str, key: &str) -> String {
    format!("{table} is required to hold `{key}`")
}

/// Why `root` is none: 1 to [`MAX_ROOT_BYTES`] bytes, no control
/// character, no space at either end. Never resolved here.
fn root_problem(root: &str) -> Option<String> {
    if root.is_empty() || root.len() > MAX_ROOT_BYTES {
        return Some(format!(
            "`root` is {} bytes long; a root has 1 to {MAX_ROOT_BYTES}",
            root.len()
        ));
    }
    if root.chars().any(char::is_control) {
        return Some("`root` holds a control character".to_owned());
    }
    (root.trim() != root).then(|| format!("`root` `{root}` has a space at an end"))
}

/// A direction's `kinds`, within [`strings`]' cap: at least one, each none
/// of [`kind_problem`]'s, each once; the problems pushed. Never checked
/// against `[ids]`: a kind is the project's word, matched by the reads
/// (`docs/features/departments.md` "Acceptance criteria", DP-2).
fn kinds_of(
    at: Range<usize>,
    listed: Vec<Written>,
    errors: &mut Errors<'_>,
) -> Option<Vec<String>> {
    if listed.is_empty() {
        errors.push(
            at,
            format!("`kinds` lists no kind; a direction lists 1 to {MAX_LIST}"),
        );
        return None;
    }
    let mut kept: Vec<String> = Vec::with_capacity(listed.len());
    for (kind, at) in listed {
        if let Some(problem) = kind_problem("kinds", &kind) {
            errors.push(at, problem);
        } else if kept.contains(&kind) {
            errors.push(at, format!("`kinds`: `{kind}` is listed twice"));
        } else {
            kept.push(kind);
        }
    }
    Some(kept)
}

/// Why `kind`, written under `key`, is no node kind: 1 to [`MAX_KIND_BYTES`]
/// bytes, no control character, no space at either end. A direction's
/// `kinds` and a `[[card.split]]` row's `kind` take these checks
/// (`docs/features/department-split.md` "Acceptance criteria", S1), each
/// message opening with its key.
pub(crate) fn kind_problem(key: &str, kind: &str) -> Option<String> {
    if kind.is_empty() || kind.len() > MAX_KIND_BYTES {
        return Some(format!(
            "`{key}`: `{kind}` is {} bytes long; a kind has 1 to {MAX_KIND_BYTES}",
            kind.len()
        ));
    }
    if kind.chars().any(char::is_control) {
        return Some(format!("`{key}`: a kind holds a control character"));
    }
    (kind.trim() != kind).then(|| format!("`{key}`: `{kind}` has a space at an end"))
}

/// The string `value` and its span, else the TOML reader's error (pushed).
fn string(value: &Spanned<DeValue<'_>>, errors: &mut Errors<'_>) -> Option<Written> {
    if let DeValue::String(text) = value.get_ref() {
        return Some((text.as_ref().to_owned(), value.span()));
    }
    misread::<String>(value, errors);
    None
}

/// The strings `value` lists, at most [`MAX_LIST`], each with its span;
/// else the TOML reader's error or the cap's (pushed).
fn strings(value: &Spanned<DeValue<'_>>, errors: &mut Errors<'_>) -> Option<Vec<Written>> {
    let DeValue::Array(items) = value.get_ref() else {
        misread::<Vec<String>>(value, errors);
        return None;
    };
    let before = errors.len();
    let mut out = Vec::with_capacity(items.len().min(MAX_LIST));
    for (at, item) in items.iter().enumerate() {
        if at == MAX_LIST {
            errors.push(
                item.span(),
                format!("a list holds at most {MAX_LIST} entries"),
            );
            break;
        }
        if let Some(written) = string(item, errors) {
            out.push(written);
        }
    }
    (errors.len() == before).then_some(out)
}

/// The strings `logins` lists, at most [`MAX_LOGINS`], each with its span;
/// else a problem (pushed). Unlike [`strings`], a message names a type and
/// a position, never the value the TOML reader would echo: a login stays
/// out of the notes every login reads (task spec `hub-department` HD6).
fn logins_of(value: &Spanned<DeValue<'_>>, errors: &mut Errors<'_>) -> Option<Vec<Written>> {
    let DeValue::Array(items) = value.get_ref() else {
        errors.push(
            value.span(),
            format!(
                "`logins` is {}, not an array of strings",
                type_name(value.get_ref())
            ),
        );
        return None;
    };
    let before = errors.len();
    let mut out = Vec::with_capacity(items.len().min(MAX_LOGINS));
    for (at, item) in items.iter().enumerate() {
        if at == MAX_LOGINS {
            errors.push(
                item.span(),
                format!("`logins` holds at most {MAX_LOGINS} entries"),
            );
            break;
        }
        match item.get_ref() {
            DeValue::String(login) => out.push((login.as_ref().to_owned(), item.span())),
            other => errors.push(
                item.span(),
                format!("`logins[{at}]` is {}, not a string", type_name(other)),
            ),
        }
    }
    (errors.len() == before).then_some(out)
}

/// The TOML reader's message for `value` read as a `T`, pushed at the span
/// it names (else `value`'s). Called only once `value` is known to be no
/// `T`; a reader that took it anyway pushes a plain type line.
fn misread<'i, T: Deserialize<'i>>(value: &Spanned<DeValue<'i>>, errors: &mut Errors<'_>) {
    match T::deserialize(ValueDeserializer::from(value.clone())) {
        Err(error) => errors.push(
            error.span().unwrap_or_else(|| value.span()),
            error.message().trim().to_owned(),
        ),
        Ok(_) => errors.push(value.span(), "invalid type".to_owned()),
    }
}

// ------------------------------------------------------------- judged

/// What judging the well-formed workspaces holds on to.
struct Judge<'a, 't> {
    text: &'t str,
    modules: &'a [WorkspaceModule],
    /// `[card]`, read once, or its problem.
    card: Result<&'a CardConfig, &'a ProjectError>,
    /// `card`'s problem is noted: once, at the first `flows` judged.
    card_noted: bool,
    notes: &'a mut Vec<WorkspaceNote>,
    /// Each kept login and the workspace it lands in, so far.
    logins: BTreeMap<String, String>,
}

impl Judge<'_, '_> {
    fn note(&mut self, span: &Range<usize>, message: String) {
        self.notes.push(WorkspaceNote {
            line: line_of(self.text, span.start),
            message,
        });
    }

    /// One workspace: sections, landing, flows, module, each value naming
    /// nothing left out with a note; then its `logins` ([`Self::logins`]).
    fn workspace(&mut self, written: WrittenWorkspace) -> Workspace {
        let name = written.name;
        let sections = match written.sections {
            None => IMPLICIT_SECTIONS.map(str::to_owned).to_vec(),
            Some(listed) => self.sections(&name, listed),
        };
        let landing = match written.landing {
            None => OVERVIEW.to_owned(),
            Some((landing, _)) if landing == OVERVIEW || sections.contains(&landing) => landing,
            Some((landing, at)) => {
                self.note(
                    &at,
                    format!(
                        "workspace `{name}`: landing `{landing}` is not one of its sections: it \
                         lands on the Overview"
                    ),
                );
                OVERVIEW.to_owned()
            }
        };
        let flows = written.flows.and_then(|listed| self.flows(&name, listed));
        let disciplines = self.card.map_or_else(
            |_| Vec::new(),
            |card| disciplines_of(card, flows.as_deref()),
        );
        let module = written.module.and_then(|(module, at)| {
            if self.modules.iter().any(|known| known.name == module) {
                return Some(module);
            }
            self.note(
                &at,
                format!(
                    "workspace `{name}`: no `[[module]]` is named `{module}`: no default module"
                ),
            );
            None
        });
        if let Some(listed) = written.logins {
            self.logins(&name, listed);
        }
        Workspace {
            title: written.title.unwrap_or_else(|| name.clone()),
            name,
            landing,
            sections,
            flows,
            disciplines,
            module,
        }
    }

    /// `sections`: each a [`SECTIONS`] ID, once.
    fn sections(&mut self, name: &str, listed: Vec<Written>) -> Vec<String> {
        let mut kept: Vec<String> = Vec::with_capacity(listed.len());
        for (section, at) in listed {
            if !SECTIONS.contains(&section.as_str()) {
                self.note(
                    &at,
                    format!(
                        "workspace `{name}`: `{section}` is no section ({}): left out",
                        SECTIONS.join(", ")
                    ),
                );
            } else if kept.contains(&section) {
                self.note(
                    &at,
                    format!(
                        "workspace `{name}`: section `{section}` is listed twice: the second left \
                         out"
                    ),
                );
            } else {
                kept.push(section);
            }
        }
        kept
    }

    /// `flows`: each a `[[card.flow]]` name, once; none left (or none
    /// given) is `None`. A `[card]` that does not read serves the list as
    /// written, with one note for the document.
    fn flows(&mut self, name: &str, listed: Vec<Written>) -> Option<Vec<String>> {
        if listed.is_empty() {
            return None;
        }
        let card = match self.card {
            Ok(card) => card,
            Err(error) => {
                if !self.card_noted {
                    self.card_noted = true;
                    self.notes.push(fell_back(
                        error,
                        "the workspaces' flows are served unjudged",
                    ));
                }
                return Some(listed.into_iter().map(|(flow, _)| flow).collect());
            }
        };
        let mut kept: Vec<String> = Vec::with_capacity(listed.len());
        for (flow, at) in listed {
            if card.flow(&flow).is_none() {
                self.note(
                    &at,
                    format!("workspace `{name}`: no `[[card.flow]]` is named `{flow}`: left out"),
                );
            } else if kept.contains(&flow) {
                self.note(
                    &at,
                    format!(
                        "workspace `{name}`: flow `{flow}` is listed twice: the second left out"
                    ),
                );
            } else {
                kept.push(flow);
            }
        }
        (!kept.is_empty()).then_some(kept)
    }

    /// `logins`, in config order: each kept in [`Self::logins`] unless
    /// [`hub_login_problem`] refuses it, this workspace listed it before or
    /// an earlier one holds it; else left out with a note naming its 0-based
    /// position, never the login (task spec `hub-department` HD5, HD6).
    fn logins(&mut self, name: &str, listed: Vec<Written>) {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (at, (login, span)) in listed.into_iter().enumerate() {
            let message = if let Some(problem) = hub_login_problem(&login) {
                format!("workspace `{name}`: logins[{at}]: {problem}: left out")
            } else if !seen.insert(login.clone()) {
                format!("workspace `{name}`: logins[{at}] is listed twice: the second left out")
            } else if let Some(first) = self.logins.get(&login) {
                format!("workspace `{name}`: logins[{at}] already lands in `{first}`: left out")
            } else {
                self.logins.insert(login, name.to_owned());
                continue;
            };
            self.note(&span, message);
        }
    }
}
