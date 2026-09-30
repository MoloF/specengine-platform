//! `spec index [--full]`, and the freshness step every read command runs
//! first: the store's incremental `update` over the working tree
//! (`--full`: `rebuild`). A missing root is worth a warning only when
//! `[paths]` is written; unreadable directories and skipped names always
//! are. Nothing the walk meets is fatal.

use serde::Serialize;
use specengine_store::{IndexWriter, SqliteIndex, UpdateReport, WorkingTree};

use crate::location::open_index;
use crate::project::{ProjectRoot, discover};
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
    pub messages: Vec<Message>,
}

/// `spec index`: brings the project's index up to date with its files.
pub fn index(
    env: &Env,
    globals: &Globals,
    request: &IndexRequest,
) -> Result<IndexOutcome, CliError> {
    let project = discover(env, globals)?;
    let mut open = open_index(env, &project)?;
    let (report, messages) = refresh(&mut open.index, &project, request.full)?;
    Ok(IndexOutcome {
        project: open.slug,
        db: open.db.display().to_string(),
        report,
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
    out.push_str(&format!("db {}\n", outcome.db));
    out
}

#[derive(Serialize)]
struct IndexJson<'a> {
    project: &'a str,
    db: &'a str,
    #[serde(flatten)]
    report: &'a UpdateReport,
}

fn view(outcome: &IndexOutcome) -> IndexJson<'_> {
    IndexJson {
        project: &outcome.project,
        db: &outcome.db,
        report: &outcome.report,
    }
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for IndexOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        view(self).serialize(serializer)
    }
}
