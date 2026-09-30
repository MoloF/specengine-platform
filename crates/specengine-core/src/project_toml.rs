//! `ProjectConfig::from_toml`: the one loader of `specengine.toml` for the
//! `spec` commands (docs/features/spec-cli.md, "Data"). Pure: the text in,
//! the checked values out.
//!
//! - `[project]` is closed: `slug` (a [`grammar::is_slug`] of at most
//!   [`MAX_SLUG_BYTES`] bytes), `name`, `language`, all optional here; an
//!   unknown key or a wrong type is an error at its line. A missing slug is
//!   an error only where a command needs one ([`ProjectConfig::slug`]: the
//!   index database is named by it), at the `[project]` line, else line 1.
//! - `[ids]` and `[paths]` go through their own readers
//!   ([`scheme_from_toml`], [`paths_from_toml`]), lines kept; no `[ids]` is
//!   the empty scheme.
//! - Top-level keys: `project`, `paths`, `ids`, then `budgets`, `classes`,
//!   `check`, `generators`, `zones`, `gate`, `code`, which are only
//!   name-checked here (their readers are the check's and later
//!   increments'); any other key is an error at its line, so a `[path]`
//!   typo cannot silently fall back to the default walk.
//!
//! Every error is `file:line: message` through [`ProjectError::at`].

use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use serde::de::IgnoredAny;
use specengine_model::{IdScheme, SchemeError, grammar};
use toml::Spanned;

use crate::paths_toml::{Paths, PathsError, paths_from_toml};
use crate::scheme_toml::scheme_from_toml;

/// The longest `[project] slug`, in bytes (a slug is ASCII).
pub const MAX_SLUG_BYTES: usize = 64;

/// The `[project]` table as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Project {
    /// The project's name in the index: the database file is `<slug>.db`.
    pub slug: Option<String>,
    /// A human name, carried only.
    pub name: Option<String>,
    /// The prose language, carried only.
    pub language: Option<String>,
}

/// A whole `specengine.toml` as the `spec` commands read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfig {
    pub project: Project,
    /// `[ids]`; empty without the table.
    pub scheme: IdScheme,
    /// `[paths]`, defaults applied.
    pub paths: Paths,
    /// A `[paths]` table exists: a missing root is then worth a warning.
    pub paths_written: bool,
    /// The 1-based line of the `[project]` header; `None` without the table.
    pub project_line: Option<usize>,
}

impl ProjectConfig {
    /// Reads and checks the whole file (see the module documentation).
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        project_from_toml(text)
    }

    /// The `[project] slug`, required by every command that opens the
    /// index; missing → an error at the `[project]` line (line 1 without
    /// the table).
    pub fn slug(&self) -> Result<&str, ProjectError> {
        self.project
            .slug
            .as_deref()
            .ok_or_else(|| match self.project_line {
                Some(line) => ProjectError {
                    line: Some(line),
                    message: "`[project]` has no `slug`: the index database is named by it \
                          (`slug = \"my-project\"`)"
                        .to_owned(),
                },
                None => ProjectError {
                    line: Some(1),
                    message:
                        "no `[project]` table with a `slug`: the index database is named by it \
                          (`[project]` then `slug = \"my-project\"`)"
                            .to_owned(),
                },
            })
    }
}

/// A `specengine.toml` error: the 1-based line when known, and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectError {
    pub line: Option<usize>,
    pub message: String,
}

impl ProjectError {
    /// `file:line: message` (`file: message` without a line).
    pub fn at(&self, file: &str) -> String {
        match self.line {
            Some(line) => format!("{file}:{line}: {}", self.message),
            None => format!("{file}: {}", self.message),
        }
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<SchemeError> for ProjectError {
    fn from(error: SchemeError) -> Self {
        Self {
            line: error.line,
            message: error.message,
        }
    }
}

impl From<PathsError> for ProjectError {
    fn from(error: PathsError) -> Self {
        Self {
            line: error.line,
            message: error.message,
        }
    }
}

/// Why `slug` is no `[project] slug`: not a [`grammar::is_slug`]
/// (`[a-z][a-z0-9-]*`), or longer than [`MAX_SLUG_BYTES`]. `None`: valid.
pub fn slug_problem(slug: &str) -> Option<String> {
    if !grammar::is_slug(slug) {
        return Some(format!(
            "{slug:?} is not a slug: lower-case ASCII letters, digits and `-`, \
             starting with a letter"
        ));
    }
    if slug.len() > MAX_SLUG_BYTES {
        return Some(format!(
            "{slug:?} is {} bytes long; a slug has at most {MAX_SLUG_BYTES}",
            slug.len()
        ));
    }
    None
}

/// [`ProjectConfig::from_toml`] as a function.
pub fn project_from_toml(text: &str) -> Result<ProjectConfig, ProjectError> {
    let error_at = |span: Option<Range<usize>>, message: String| ProjectError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let raw: RawFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;

    let mut project = Project::default();
    let mut project_line = None;
    if let Some(table) = raw.project {
        project_line = Some(line_of(text, table.span().start));
        let table = table.into_inner();
        if let Some(slug) = table.slug {
            let span = slug.span();
            let slug = slug.into_inner();
            if let Some(problem) = slug_problem(&slug) {
                return Err(error_at(Some(span), format!("`slug`: {problem}")));
            }
            project.slug = Some(slug);
        }
        project.name = table.name;
        project.language = table.language;
    }
    let scheme = scheme_from_toml(text)?;
    let paths = paths_from_toml(text)?;
    Ok(ProjectConfig {
        project,
        scheme,
        paths,
        paths_written: raw.paths.is_some(),
        project_line,
    })
}

/// The top level: every key the file may hold. `[ids]` and `[paths]` are
/// read by their own readers; the rest are only name-checked here.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct RawFile {
    #[serde(default)]
    project: Option<Spanned<ProjectTable>>,
    #[serde(default)]
    paths: Option<IgnoredAny>,
    #[serde(default)]
    ids: Option<IgnoredAny>,
    #[serde(default)]
    budgets: Option<IgnoredAny>,
    #[serde(default)]
    classes: Option<IgnoredAny>,
    #[serde(default)]
    check: Option<IgnoredAny>,
    #[serde(default)]
    generators: Option<IgnoredAny>,
    #[serde(default)]
    zones: Option<IgnoredAny>,
    #[serde(default)]
    gate: Option<IgnoredAny>,
    #[serde(default)]
    code: Option<IgnoredAny>,
}

/// `[project]`; any other key is an error.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectTable {
    #[serde(default)]
    slug: Option<Spanned<String>>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
