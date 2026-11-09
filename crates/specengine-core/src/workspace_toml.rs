//! `[[workspace]]` and `[[module]]` of `specengine.toml` (task spec
//! `workspaces`, "Data"; `docs/canon/workspaces.md`): a trade's lens on a
//! project and the roots it may scope Tree and Flow to, read only by
//! `spec workspace` and its daemon twin. Of [`crate::card_toml`]'s family,
//! read apart: [`crate::ProjectConfig::from_toml`] only name-checks both
//! keys, so neither table fails a read, a command, a check or a gate.
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
//!   (the TOML reader's message), a missing `name` (or a module's `root`),
//!   a bad name ([`name_problem`]), title or root, a name twice, more than
//!   [`MAX_MODULES`] modules, [`MAX_WORKSPACES`] workspaces or
//!   [`MAX_LIST`] entries in a list.
//! - **Judged, per workspace in config order**: `sections`, `landing`,
//!   `flows` (against [`card_from_toml`]; a `[card]` that does not read
//!   serves the lists as written, one note for the document), `module`.
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

use crate::card_toml::{MAX_TITLE_BYTES, card_from_toml, name_problem};
use crate::health_toml::{Errors, line_of, unknown_field};
use crate::project_toml::ProjectError;

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

/// The top-level key of the workspaces.
const WORKSPACE: &str = "workspace";
/// The top-level key of the modules.
const MODULE: &str = "module";
/// A workspace's keys, in the order a message lists them.
const WORKSPACE_KEYS: [&str; 6] = ["name", "title", "sections", "landing", "flows", "module"];
/// A module's keys, in the order a message lists them.
const MODULE_KEYS: [&str; 3] = ["name", "title", "root"];
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
    /// A served module's name.
    pub module: Option<String>,
}

impl Workspace {
    /// The implicit workspace: every section but `tasks`, on the Overview,
    /// every flow, no module.
    pub fn implicit() -> Self {
        Self {
            name: IMPLICIT_NAME.to_owned(),
            title: IMPLICIT_TITLE.to_owned(),
            landing: OVERVIEW.to_owned(),
            sections: IMPLICIT_SECTIONS.map(str::to_owned).to_vec(),
            flows: None,
            module: None,
        }
    }
}

/// One module: a root document and its subtree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct WorkspaceModule {
    pub name: String,
    pub title: String,
    /// A REF in any `spec show` form, resolved by the read it roots.
    pub root: String,
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
            return implicit(Vec::new(), notes);
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
    let Some(written) = written else {
        return implicit(modules, notes);
    };
    let mut judge = Judge {
        text,
        modules: &modules,
        card: None,
        notes: &mut notes,
    };
    let workspaces = written
        .into_iter()
        .map(|workspace| judge.workspace(workspace))
        .collect();
    notes.sort_by_key(|note| note.line);
    WorkspaceConfig {
        source: WorkspaceSource::Config,
        workspaces,
        modules,
        notes,
    }
}

/// The implicit document over `modules`, `notes` sorted.
fn implicit(modules: Vec<WorkspaceModule>, mut notes: Vec<WorkspaceNote>) -> WorkspaceConfig {
    notes.sort_by_key(|note| note.line);
    WorkspaceConfig {
        source: WorkspaceSource::Implicit,
        workspaces: vec![Workspace::implicit()],
        modules,
        notes,
    }
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
        // another type (pushed), so it is not also missing.
        let mut name = None;
        let mut title = None;
        let mut root = None;
        for (key, value) in entry.iter() {
            match key.get_ref().as_ref() {
                "name" => name = Some(string(value, &mut errors)),
                "title" => title = string(value, &mut errors),
                "root" => root = Some(string(value, &mut errors)),
                other => errors.push(key.span(), unknown_field(other, &MODULE_KEYS)),
            }
        }
        let name = named(&span, name, MODULE_TABLE, &mut names, &mut errors);
        if let Some((title, at)) = &title
            && let Some(problem) = title_problem(title)
        {
            errors.push(at.clone(), problem);
        }
        let root = match root {
            Some(Some((root, at))) => {
                if let Some(problem) = root_problem(&root) {
                    errors.push(at, problem);
                }
                Some(root)
            }
            Some(None) => None,
            None => {
                errors.push(span, required(MODULE_TABLE, "root"));
                None
            }
        };
        if let (Some(name), Some(root)) = (name, root) {
            modules.push(WorkspaceModule {
                title: title.map_or_else(|| name.clone(), |(title, _)| title),
                name,
                root,
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
        for (key, value) in entry.iter() {
            match key.get_ref().as_ref() {
                "name" => name = Some(string(value, &mut errors)),
                "title" => title = string(value, &mut errors),
                "sections" => sections = strings(value, &mut errors),
                "landing" => landing = string(value, &mut errors),
                "flows" => flows = strings(value, &mut errors),
                "module" => module = string(value, &mut errors),
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

/// Why `title` is none: 1 to [`MAX_TITLE_BYTES`] UTF-8 bytes, any script,
/// no control character.
fn title_problem(title: &str) -> Option<String> {
    if title.is_empty() || title.len() > MAX_TITLE_BYTES {
        return Some(format!(
            "`title` is {} bytes long; a title has 1 to {MAX_TITLE_BYTES}",
            title.len()
        ));
    }
    title
        .chars()
        .any(char::is_control)
        .then(|| "`title` holds a control character".to_owned())
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
    /// `[card]`'s flow names once read, or its problem; read at the first
    /// `flows` met.
    card: Option<Result<BTreeSet<String>, ()>>,
    notes: &'a mut Vec<WorkspaceNote>,
}

impl Judge<'_, '_> {
    fn note(&mut self, span: &Range<usize>, message: String) {
        self.notes.push(WorkspaceNote {
            line: line_of(self.text, span.start),
            message,
        });
    }

    /// One workspace: sections, landing, flows, module, each value naming
    /// nothing left out with a note.
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
        Workspace {
            title: written.title.unwrap_or_else(|| name.clone()),
            name,
            landing,
            sections,
            flows,
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
        let known = match self.card_flows() {
            Ok(known) => known,
            Err(()) => return Some(listed.into_iter().map(|(flow, _)| flow).collect()),
        };
        let mut kept: Vec<String> = Vec::with_capacity(listed.len());
        for (flow, at) in listed {
            if !known.contains(&flow) {
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

    /// `[card]`'s flow names, read once; its problem noted once.
    fn card_flows(&mut self) -> Result<BTreeSet<String>, ()> {
        if let Some(card) = &self.card {
            return card.clone();
        }
        let read = match card_from_toml(self.text) {
            Ok(card) => Ok(card.flows.into_iter().map(|flow| flow.name).collect()),
            Err(error) => {
                self.notes.push(fell_back(
                    &error,
                    "the workspaces' flows are served unjudged",
                ));
                Err(())
            }
        };
        self.card = Some(read.clone());
        read
    }
}
