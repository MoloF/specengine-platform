//! `spec workspace [--json]` (task spec `workspaces`, "Data"): the project's
//! `[[workspace]]` and `[[module]]` tables as [`WorkspaceConfig`] reads
//! them, for the person who writes the config and for `GET .../workspaces`,
//! whose answer is this document byte for byte. It reads the config once,
//! opens no index and writes nothing, like `spec team`.
//!
//! The tables never fail it: a malformed one serves its default with a
//! note, and the exit is 0 whatever they say. Exit 2 is only no project or
//! a config `ProjectConfig` refuses, as for every command. Each note is one
//! `note:` line on stderr and the same text in JSON `notes`, the config named
//! as given, its control characters escaped. No cut: the reader's limits
//! bound the document.

use serde::Serialize;
use specengine_core::{Workspace, WorkspaceConfig, WorkspaceModule, WorkspaceSource};

use crate::project::discover_with_text;
use crate::{CliError, Env, Globals, escape_controls, one_line};

/// What `spec workspace` printed: every key present, in this order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceOutcome {
    pub source: WorkspaceSource,
    /// `<config>:<line>: <message>`, each on one line as on stderr.
    pub notes: Vec<String>,
    /// At least one: the implicit workspace when none is configured.
    pub workspaces: Vec<Workspace>,
    pub modules: Vec<WorkspaceModule>,
}

impl WorkspaceOutcome {
    /// `config`'s document, its notes naming `label`.
    pub fn of(config: WorkspaceConfig, label: &str) -> Self {
        Self {
            source: config.source,
            notes: config
                .notes_at(label)
                .iter()
                .map(|note| escape_controls(&one_line(note)))
                .collect(),
            workspaces: config.workspaces,
            modules: config.modules,
        }
    }

    /// One `note:` line per note, in order.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        self.notes
            .iter()
            .map(|note| format!("note: {note}"))
            .collect()
    }
}

/// `spec workspace` (see the module documentation).
pub fn workspace(env: &Env, globals: &Globals) -> Result<WorkspaceOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    Ok(WorkspaceOutcome::of(
        WorkspaceConfig::from_toml(&text),
        &project.config_label,
    ))
}

/// What a text line shows for no value: parentheses are no name character
/// and no section ID, so it is never a name.
const NONE: &str = "(none)";

/// `workspaces <source> | workspaces <n>, modules <m>`, then a line per
/// workspace and per module, in served order. Each line is one line and its
/// control characters escaped: a `[card]` that does not read serves `flows`
/// as written.
pub(crate) fn render_text(outcome: &WorkspaceOutcome) -> String {
    let mut lines = vec![format!(
        "workspaces {} | workspaces {}, modules {}",
        outcome.source.as_str(),
        outcome.workspaces.len(),
        outcome.modules.len()
    )];
    for workspace in &outcome.workspaces {
        let sections = if workspace.sections.is_empty() {
            NONE.to_owned()
        } else {
            workspace.sections.join(", ")
        };
        let flows = workspace
            .flows
            .as_ref()
            .map_or_else(|| "(all)".to_owned(), |flows| flows.join(", "));
        lines.push(format!(
            "workspace {} | {} | landing {} | sections {sections} | flows {flows} | module {}",
            workspace.name,
            workspace.title,
            workspace.landing,
            workspace.module.as_deref().unwrap_or(NONE)
        ));
    }
    for module in &outcome.modules {
        lines.push(format!(
            "module {} | {} | root {}",
            module.name, module.title, module.root
        ));
    }
    lines
        .iter()
        .map(|line| format!("{}\n", escape_controls(&one_line(line))))
        .collect()
}
