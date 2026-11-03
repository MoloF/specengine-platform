//! `spec sparse-set` (task spec `hub-git`, "The sparse set"), read-only: the
//! paths a hub's sparse checkout must hold, from the repository's top, out
//! of the root's `specengine.toml` at `HEAD` (store [`sparse_set`]), one per
//! line in config order; a path `HEAD` holds that this checkout lacks is
//! also a `warning:`. `--json`: `{"paths": [...], "missing": [...]}`. Exit 0
//! whatever is missing (a gap blocks nothing); exit 2 with no repository, no commit, or
//! a config `HEAD` holds that cannot be used, named `HEAD:<path>` as `spec
//! export index` names a config's causes. A sync's look checks the same set.

use serde::Serialize;
use specengine_store::{SparseError, WorktreeGit, sparse_set};

use crate::export::config_errors;
use crate::project::locate;
use crate::{CliError, Env, Globals, Message, process_git};

/// What `spec sparse-set` found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SparseOutcome {
    /// From the top, in config order: a directory ends in `/`.
    pub paths: Vec<String>,
    /// Those `HEAD` holds that this checkout lacks.
    pub missing: Vec<String>,
    #[serde(skip)]
    pub messages: Vec<Message>,
}

/// `spec sparse-set` (see the module documentation).
pub fn sparse(env: &Env, globals: &Globals) -> Result<SparseOutcome, CliError> {
    let located = locate(env, globals)?;
    let root = &located.root;
    let git = WorktreeGit::new(root, &process_git(env)).map_err(CliError::spec)?;
    let set = sparse_set(root, &git).map_err(|error| match error {
        SparseError::NoRepository => CliError::spec(format!(
            "{} lies in no git repository: the sparse set is read from HEAD",
            root.display()
        )),
        SparseError::NoCommit => CliError::spec(
            "HEAD has no commit yet: the sparse set is read from HEAD's specengine.toml",
        ),
        SparseError::Config { label, report } => config_errors(&label, &report),
        SparseError::Git(error) => CliError::spec(error),
    })?;
    let messages = set
        .missing
        .iter()
        .map(|path| {
            Message::Warning(format!(
                "{path} is in HEAD but not in this checkout: widen the sparse checkout"
            ))
        })
        .collect();
    Ok(SparseOutcome {
        paths: set.paths,
        missing: set.missing,
        messages,
    })
}

/// One path per line.
pub(crate) fn render_text(outcome: &SparseOutcome) -> String {
    outcome
        .paths
        .iter()
        .map(|path| format!("{path}\n"))
        .collect()
}
