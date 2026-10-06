//! The served projects, read once at start from the `--root` arguments
//! (docs/features/daemon-read.md "Description and interactions").

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specengine_cli::{CliError, Env, Globals, discover};
use tokio::sync::Mutex;

/// A served project: the slug it is served under, its canonical root, and
/// the turn its CLI calls take (one at a time: each read refreshes the
/// project's index). The turn is awaited by the request's own task, not
/// on the blocking pool: a request dropped while it waits never runs.
#[derive(Debug)]
pub(crate) struct Project {
    pub slug: String,
    /// Canonical.
    pub root: PathBuf,
    pub turn: Arc<Mutex<()>>,
}

impl Project {
    /// The CLI globals of every call on this project: its root, no walk,
    /// no `--config`.
    pub(crate) fn globals(&self) -> Globals {
        Globals {
            root: Some(self.root.clone()),
            config: None,
        }
    }
}

/// Each root canonicalised and its own `specengine.toml` read through the
/// CLI's discovery (a slug required), in the order given; the reason the
/// start fails names the root as given and the CLI's line: a missing root,
/// no config, a broken one, no slug, or two roots of one slug (naming
/// both). Nothing is opened in the data directory.
pub(crate) fn projects(env: &Env, roots: &[PathBuf]) -> Result<Vec<Project>, String> {
    let mut projects: Vec<(PathBuf, Project)> = Vec::new();
    for given in roots {
        let named = |error: CliError| format!("--root {}: {}", given.display(), error.message);
        let globals = Globals {
            root: Some(given.clone()),
            config: None,
        };
        let found = discover(env, &globals).map_err(named)?;
        let slug = found.slug().map_err(named)?.to_owned();
        if let Some((other, _)) = projects.iter().find(|(_, project)| project.slug == slug) {
            return Err(format!(
                "--root {} and --root {} are both the project `{slug}`: a slug is served once \
                 (its URL key and its database)",
                shown(other),
                shown(given)
            ));
        }
        projects.push((
            given.clone(),
            Project {
                slug,
                root: found.root,
                turn: Arc::new(Mutex::new(())),
            },
        ));
    }
    Ok(projects.into_iter().map(|(_, project)| project).collect())
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}
