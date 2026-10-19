//! The served projects, read once at start from the `--root` arguments
//! (docs/features/daemon-read.md "Description and interactions"), and their
//! data directory (docs/features/spec-serve.md "Description and interactions"):
//! guarded against every root, then created.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specengine_cli::{
    CliError, Env, Globals, ProjectRoot, checked_data_dir, discover, prepared_data_dir,
};
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

/// What a start serves: its projects, in `--root` order, and the data
/// directory, outside every root, created.
#[derive(Debug)]
pub(crate) struct Served {
    pub projects: Vec<Project>,
    pub data_dir: PathBuf,
}

/// Each root canonicalised and its own `specengine.toml` read through the
/// CLI's discovery (a slug required), in the order given; then the data
/// directory guarded against every root (the CLI's `checked_data_dir`,
/// nothing created) and created (its `prepared_data_dir`). The reason the
/// start fails names the root as given and the CLI's line: a missing root,
/// no config, a broken one, no slug, two roots of one slug (naming both),
/// or the data directory inside it (nothing created then). Nothing is
/// opened in the data directory.
pub(crate) fn served(env: &Env, roots: &[PathBuf]) -> Result<Served, String> {
    let mut roots_found: Vec<(PathBuf, ProjectRoot)> = Vec::new();
    let mut projects: Vec<(PathBuf, Project)> = Vec::new();
    for given in roots {
        let named = |error: CliError| named_root(given, &error);
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
                root: found.root.clone(),
                turn: Arc::new(Mutex::new(())),
            },
        ));
        roots_found.push((given.clone(), found));
    }
    for (given, project) in &roots_found {
        checked_data_dir(env, project).map_err(|error| named_root(given, &error))?;
    }
    let Some((given, first)) = roots_found.first() else {
        return Err("no --root given".to_owned());
    };
    let data_dir = prepared_data_dir(env, first).map_err(|error| named_root(given, &error))?;
    Ok(Served {
        projects: projects.into_iter().map(|(_, project)| project).collect(),
        data_dir,
    })
}

/// `--root <given>: <the CLI's line>`.
fn named_root(given: &Path, error: &CliError) -> String {
    format!("--root {}: {}", given.display(), error.message)
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}
