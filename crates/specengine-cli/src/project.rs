//! Where the project is: its root and its `specengine.toml`.
//!
//! With neither `--root` nor `--config`, the walk goes up from the canonical
//! current directory to the first directory holding a `specengine.toml`
//! file; none → exit 2 naming `spec init`. `--root DIR` takes that
//! directory, no walk. `--config FILE` replaces `<root>/specengine.toml`;
//! without `--root` the root is the current directory (read-only pilots).

use std::fs;
use std::path::{Path, PathBuf};

use specengine_core::{ProjectConfig, ProjectError};

use crate::{CliError, Env, Globals};

/// The project's config file, at its root (the store's constant: the
/// staged check reads the index entry of that name).
pub const CONFIG_FILE: &str = specengine_store::CONFIG_FILE;

/// A found project: its canonical root and its checked config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    /// Canonical.
    pub root: PathBuf,
    /// The config as named in messages: `--config` as given, else
    /// `specengine.toml` (root-relative).
    pub config_label: String,
    pub config: ProjectConfig,
}

impl ProjectRoot {
    /// The `[project] slug`, required by every command that opens the index.
    pub fn slug(&self) -> Result<&str, CliError> {
        self.config
            .slug()
            .map_err(|error| config_error(&self.config_label, &error))
    }
}

/// A located project, its config not read yet: `spec check` and
/// `spec export index` read it through the store's check loader, whose
/// every failure is a cause, not a discovery failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    /// Canonical.
    pub root: PathBuf,
    /// The file to read.
    pub config_file: PathBuf,
    /// As in [`ProjectRoot::config_label`].
    pub config_label: String,
}

/// Finds the project (see the module documentation) and reads its config.
pub fn discover(env: &Env, globals: &Globals) -> Result<ProjectRoot, CliError> {
    let Located {
        root,
        config_file,
        config_label,
    } = locate(env, globals)?;
    let bytes = fs::read(&config_file)
        .map_err(|error| CliError::spec(format!("cannot read {config_label}: {error}")))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| CliError::spec(format!("{config_label}: the file is not UTF-8")))?;
    let config =
        ProjectConfig::from_toml(&text).map_err(|error| config_error(&config_label, &error))?;
    Ok(ProjectRoot {
        root,
        config_label,
        config,
    })
}

/// The root and the config file (see the module documentation), nothing
/// read: a discovery failure is an unusable current directory or `--root`,
/// or no `specengine.toml` by the walk or in `--root`.
pub(crate) fn locate(env: &Env, globals: &Globals) -> Result<Located, CliError> {
    let cwd = canonical_dir(&env.cwd, "the current directory")?;
    let (root, config_file, config_label) = match (&globals.root, &globals.config) {
        (None, None) => {
            let root = find_config_dir(&cwd).ok_or_else(|| {
                CliError::spec(format!(
                    "no {CONFIG_FILE} in the current directory or any directory above it; \
                     run `spec init` at the project root, or pass --root"
                ))
            })?;
            let file = root.join(CONFIG_FILE);
            (root, file, CONFIG_FILE.to_owned())
        }
        (Some(root), None) => {
            let root = canonical_dir(&cwd.join(root), &format!("--root {}", root.display()))?;
            let file = root.join(CONFIG_FILE);
            if !is_regular_file(&file) {
                return Err(CliError::spec(format!(
                    "no {CONFIG_FILE} in the --root directory; run `spec init` there, \
                     or pass --config"
                )));
            }
            (root, file, CONFIG_FILE.to_owned())
        }
        (root, Some(config)) => {
            let root = match root {
                Some(root) => {
                    canonical_dir(&cwd.join(root), &format!("--root {}", root.display()))?
                }
                None => cwd.clone(),
            };
            (root, cwd.join(config), config.display().to_string())
        }
    };
    Ok(Located {
        root,
        config_file,
        config_label,
    })
}

/// `<config>:<line>: message`, else `spec: <config>: message`.
pub(crate) fn config_error(label: &str, error: &ProjectError) -> CliError {
    match error.line {
        Some(_) => CliError::cannot(error.at(label)),
        None => CliError::spec(error.at(label)),
    }
}

/// The first of `start` and its ancestors holding a `specengine.toml` file.
pub(crate) fn find_config_dir(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| is_regular_file(&dir.join(CONFIG_FILE)))
        .map(Path::to_path_buf)
}

/// `path` names a file (a symlink is followed), not a directory.
pub(crate) fn is_regular_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// `path` canonicalised; it must be a directory. `what` names it in the
/// error.
pub(crate) fn canonical_dir(path: &Path, what: &str) -> Result<PathBuf, CliError> {
    let canonical =
        fs::canonicalize(path).map_err(|error| CliError::spec(format!("{what}: {error}")))?;
    if !canonical.is_dir() {
        return Err(CliError::spec(format!("{what} is not a directory")));
    }
    Ok(canonical)
}
