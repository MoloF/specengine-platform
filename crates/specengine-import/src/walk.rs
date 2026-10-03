//! The one walk over a corpus: which files are documents (`census`,
//! `import`, eval `parse`) and which files a code root holds (`import`'s code
//! scan). Dot-directories and symlinks are skipped; an unreadable directory or
//! a missing root is a diagnostic, never fatal.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::census::Diagnostic;
use crate::config::CensusConfig;

/// The documents of a corpus under a config.
#[derive(Debug, Clone, Default)]
pub struct Walk {
    /// Corpus-relative, `/`-separated, sorted.
    pub documents: Vec<String>,
    /// Configured roots that could not be read.
    pub roots_missing: usize,
    /// Missing roots, unreadable directories and entries, in walk order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Walks the configured roots for documents: a listed extension, not
/// excluded. Fails only when `root` itself cannot be read.
pub fn documents(root: &Path, config: &CensusConfig) -> Result<Walk, String> {
    fs::read_dir(root).map_err(|error| format!("cannot read the corpus root: {error}"))?;
    let mut walk = Walk::default();
    let (documents, roots_missing) = files(
        root,
        &config.roots,
        &|name, relative| config.is_document(name) && !config.is_excluded(relative),
        &|_| true,
        &mut walk.diagnostics,
    );
    walk.documents = documents.into_iter().collect();
    walk.roots_missing = roots_missing;
    Ok(walk)
}

/// The files under `roots` (corpus-relative) that `keep(file name,
/// relative path)` accepts, and how many roots could not be read. A root that
/// is a file is kept on the same terms. Below a root, a directory is entered
/// when its name has no leading `.` and `enter(name)` accepts it.
pub(crate) fn files(
    root: &Path,
    roots: &[PathBuf],
    keep: &dyn Fn(&str, &str) -> bool,
    enter: &dyn Fn(&str) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> (BTreeSet<String>, usize) {
    let mut found = BTreeSet::new();
    let mut missing = 0;
    for configured in roots {
        let relative = relative_string(configured);
        let absolute = root.join(configured);
        match fs::metadata(&absolute) {
            Ok(meta) if meta.is_dir() => {
                descend(&absolute, &relative, keep, enter, &mut found, diagnostics);
            }
            Ok(_) => {
                if file_name_of(&relative).is_some_and(|name| keep(name, &relative)) {
                    found.insert(relative);
                }
            }
            Err(error) => {
                missing += 1;
                diagnostics.push(Diagnostic {
                    path: relative,
                    line: None,
                    message: format!("configured root not readable: {error}"),
                });
            }
        }
    }
    (found, missing)
}

fn descend(
    absolute: &Path,
    relative: &str,
    keep: &dyn Fn(&str, &str) -> bool,
    enter: &dyn Fn(&str) -> bool,
    found: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entries = match fs::read_dir(absolute) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(Diagnostic {
                path: relative.to_owned(),
                line: None,
                message: format!("directory skipped: {error}"),
            });
            return;
        }
    };
    let mut children = Vec::new();
    for entry in entries {
        match entry.and_then(|entry| Ok((entry.file_name(), entry.file_type()?))) {
            Ok(child) => children.push(child),
            Err(error) => diagnostics.push(Diagnostic {
                path: relative.to_owned(),
                line: None,
                message: format!("directory entry skipped: {error}"),
            }),
        }
    }
    children.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, file_type) in children {
        let name = name.to_string_lossy();
        let child = if relative.is_empty() {
            name.to_string()
        } else {
            format!("{relative}/{name}")
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if !name.starts_with('.') && enter(&name) {
                descend(
                    &absolute.join(name.as_ref()),
                    &child,
                    keep,
                    enter,
                    found,
                    diagnostics,
                );
            }
        } else if keep(&name, &child) {
            found.insert(child);
        }
    }
}

/// `/`-separated relative path; `.` components dropped, `""` for the root.
pub(crate) fn relative_string(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    parts.join("/")
}

fn file_name_of(relative: &str) -> Option<&str> {
    relative.rsplit('/').next().filter(|name| !name.is_empty())
}
