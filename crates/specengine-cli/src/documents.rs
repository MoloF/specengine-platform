//! The live document list of MCP's `resources/list` (task spec `mcp-read`,
//! "Data"): the index, updated first, as `spec show` reads it; every indexed
//! live file (neither `class: generated` nor Tier 3), by path. No command
//! prints it.

use specengine_core::check::is_live;

use crate::corpus::indexed;
use crate::project::discover;
use crate::{CliError, Env, Globals};

/// One indexed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentEntry {
    /// Root-relative, `/`-separated: a `REF` `spec show` takes.
    pub path: String,
    /// The document's ID; `None` without a valid `id:` (or not UTF-8, or
    /// not parsed).
    pub id: Option<String>,
    /// Its `title:`, else its first H1; `None` without either.
    pub title: Option<String>,
}

/// Updates the index, then lists each indexed live `.md` file ([`is_live`]:
/// neither `class: generated` nor Tier 3; a file without a parse is live,
/// as `spec tree` takes it, and listed nameless), by path
/// in byte order. Exit 2 as `spec show`: no project, no slug, no usable
/// `HOME`, a database error. The update's warnings are dropped.
pub fn documents(env: &Env, globals: &Globals) -> Result<Vec<DocumentEntry>, CliError> {
    let project = discover(env, globals)?;
    project.slug()?;
    let mut messages = Vec::new();
    let input = indexed(env, &project, &mut messages, false)?;
    let mut entries: Vec<DocumentEntry> = input
        .files
        .iter()
        .filter(|file| file.parsed.as_ref().is_none_or(is_live))
        .map(|file| {
            let document = file.parsed.as_ref().and_then(|parsed| parsed.document());
            DocumentEntry {
                path: file.path.clone(),
                id: document.and_then(|node| node.id.clone()),
                title: document.and_then(|node| node.title.clone()),
            }
        })
        .collect();
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}
