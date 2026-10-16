//! `spec index [--full]`, and the freshness step every read command runs
//! first: the store's incremental `update` over the working tree
//! (`--full`: `rebuild`). A missing root is worth a warning only when
//! `[paths]` is written; unreadable directories and skipped names always
//! are. Nothing the walk meets is fatal.
//!
//! `spec index` then refreshes the code index, a call of its own
//! ([`crate::code_index::refresh_code`]; canon `symbol-index`,
//! "spec refs and spec index"): its `code <slug>: …` line before `db`,
//! JSON `code` (`null` without `[code]`, whose code rows go). A `[code]`
//! that does not read leaves the code rows as they are: the spec index is
//! written and printed, then exit 2 naming the cause at its line. A code
//! file that cannot be read, or a directory that cannot be listed, is
//! stored and counted (exit 0); the reads that need the whole tree refuse.
//!
//! With `[code] frameworks` listed, a `roles <slug>: …` line follows the
//! code line (task spec `bevy-roles` "spec index"; [`crate::roles`]), JSON
//! `code.roles`; `null` and no line without it. A framework the code crate
//! does not know is refused as a `[code]` that does not read: the spec
//! index written and printed, `code` `null`, exit 2 at the name's line.

use serde::Serialize;
use specengine_core::CodeConfig;
use specengine_store::{CodeReport, IndexWriter, SqliteIndex, UpdateReport, WorkingTree};

use crate::code_index::{known_frameworks, refresh_code};
use crate::location::open_index;
use crate::project::{ProjectRoot, config_error, discover_with_text};
use crate::roles::{RoleCounts, render_line, role_counts};
use crate::{CliError, Env, Globals, Message, store_error};

/// `spec index` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexRequest {
    /// `--full`: re-parse everything (`rebuild`).
    pub full: bool,
}

/// What `spec index` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexOutcome {
    /// The `[project] slug`.
    pub project: String,
    /// The database file.
    pub db: String,
    pub report: UpdateReport,
    /// The code index's refresh; `None` without `[code]`, or when it does
    /// not read.
    pub code: Option<CodeReport>,
    /// The role counts after the refresh; `None` without `frameworks` (or
    /// without `code`).
    pub roles: Option<RoleCounts>,
    /// A `[code]` table that does not read: the whole stderr line
    /// (`<config>:<line>: …`), exit 2 after the spec index was written.
    pub code_error: Option<String>,
    pub messages: Vec<Message>,
}

/// `spec index`: brings the project's index up to date with its files,
/// then its code index with the `[code]` tree.
pub fn index(
    env: &Env,
    globals: &Globals,
    request: &IndexRequest,
) -> Result<IndexOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    let mut open = open_index(env, &project)?;
    let (report, mut messages) = refresh(&mut open.index, &project, request.full)?;
    let mut code = None;
    let mut roles = None;
    let mut code_error = None;
    match CodeConfig::from_toml(&text) {
        Err(error) => code_error = Some(config_error(&project.config_label, &error).message),
        Ok(None) => {
            open.index.clear_code().map_err(store_error)?;
        }
        Ok(Some(config)) => match known_frameworks(&config, &project.config_label) {
            // Refused as a `[code]` that does not read (`refresh_code`
            // refuses it too, before any code row).
            Err(error) => code_error = Some(error.message),
            Ok(()) => {
                let refreshed = refresh_code(
                    &mut open.index,
                    &project.root,
                    &config,
                    &project.config_label,
                    &project.config.scheme,
                    request.full,
                )?;
                messages.extend(refreshed.messages.iter().cloned());
                if let Some(gap) = refreshed.gap() {
                    messages.push(Message::Warning(format!(
                        "the code walk is incomplete: {gap}; reads of the code refuse until it is not"
                    )));
                }
                if !config.frameworks.is_empty() {
                    roles = Some(role_counts(&open.index, &config.frameworks)?);
                }
                code = Some(refreshed.report);
            }
        },
    }
    Ok(IndexOutcome {
        project: open.slug,
        db: open.db.display().to_string(),
        report,
        code,
        roles,
        code_error,
        messages,
    })
}

/// Walks the working tree into the index: `update`, or `rebuild` when
/// `full`; the walk's troubles as warnings.
pub(crate) fn refresh(
    index: &mut SqliteIndex,
    project: &ProjectRoot,
    full: bool,
) -> Result<(UpdateReport, Vec<Message>), CliError> {
    let tree = WorkingTree::new(&project.root, &project.config.paths).map_err(store_error)?;
    let scheme = &project.config.scheme;
    let report = if full {
        index.rebuild(&tree, scheme)
    } else {
        index.update(&tree, scheme)
    }
    .map_err(store_error)?;
    let mut messages = Vec::new();
    if project.config.paths_written {
        for root in &report.missing_roots {
            messages.push(Message::Warning(format!(
                "`[paths]` root `{root}` names no directory and no `.md` file"
            )));
        }
    }
    for dir in &report.unreadable_dirs {
        let dir = if dir.is_empty() { "." } else { dir.as_str() };
        messages.push(Message::Warning(format!(
            "cannot list `{dir}`: its files are left out of the index"
        )));
    }
    if report.skipped_names > 0 {
        messages.push(Message::Warning(format!(
            "{} directory or `.md` names skipped: not UTF-8",
            report.skipped_names
        )));
    }
    Ok((report, messages))
}

pub(crate) fn render_text(outcome: &IndexOutcome) -> String {
    let report = &outcome.report;
    let mut out = format!(
        "indexed {}: walked {}, parsed {}, unchanged {}, removed {}, unreadable {}",
        outcome.project,
        report.walked,
        report.parsed,
        report.unchanged,
        report.removed,
        report.unreadable
    );
    if report.reparsed_all {
        out.push_str(", reparsed all");
    }
    out.push('\n');
    if let Some(code) = &outcome.code {
        out.push_str(&format!(
            "code {}: walked {}, parsed {}, unchanged {}, removed {}, unreadable {}, skipped {}, symbols {}, markers {}, mentions {}",
            outcome.project,
            code.walked,
            code.parsed,
            code.unchanged,
            code.removed,
            code.unreadable,
            code.skipped,
            code.symbols,
            code.markers,
            code.mentions
        ));
        if code.rederived_all {
            out.push_str(", rederived all");
        }
        out.push('\n');
        if let Some(roles) = &outcome.roles {
            out.push_str(&render_line(&outcome.project, roles));
            out.push('\n');
        }
    }
    out.push_str(&format!("db {}\n", outcome.db));
    out
}

#[derive(Serialize)]
struct IndexJson<'a> {
    project: &'a str,
    db: &'a str,
    #[serde(flatten)]
    report: &'a UpdateReport,
    code: Option<CodeJson<'a>>,
}

/// JSON `code`: the store's report and the CLI's role counts (`null`
/// without `frameworks`).
#[derive(Serialize)]
struct CodeJson<'a> {
    #[serde(flatten)]
    report: &'a CodeReport,
    roles: Option<&'a RoleCounts>,
}

fn view(outcome: &IndexOutcome) -> IndexJson<'_> {
    IndexJson {
        project: &outcome.project,
        db: &outcome.db,
        report: &outcome.report,
        code: outcome.code.as_ref().map(|report| CodeJson {
            report,
            roles: outcome.roles.as_ref(),
        }),
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for IndexOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
