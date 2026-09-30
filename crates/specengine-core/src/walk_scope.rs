//! The walk's rules as pure predicates over root-relative `/` paths
//! (`crates/specengine-store/README.md`, "Walk"): which `.md` files the walk
//! lists, judged without the disk. One matcher for the store's walker and
//! for `spec check`'s link scope (docs/features/spec-check-links.md): the
//! `exclude` globs are compiled once per [`WalkScope`].

use crate::Paths;
use crate::glob::Glob;

/// The extension of a walked document: the one `.md` rule of the walk, the
/// walk scope, feature documents and checked file-link targets.
pub const DOCUMENT_EXTENSION: &str = ".md";

/// The roots and the compiled `exclude` globs of one `[paths]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkScope {
    roots: Vec<String>,
    exclude: Vec<Glob>,
}

impl WalkScope {
    /// Compiles `paths.exclude`; keeps `paths.roots` in their order.
    pub fn new(paths: &Paths) -> Self {
        Self {
            roots: paths.roots.clone(),
            exclude: paths.exclude.iter().map(|glob| Glob::new(glob)).collect(),
        }
    }

    /// The walked roots, as in `[paths] roots`.
    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    /// `path` matches an `exclude` glob.
    pub fn is_excluded(&self, path: &str) -> bool {
        self.exclude.iter().any(|glob| glob.matches(path))
    }

    /// `path` is a file the walk would list, judged without the disk: a
    /// clean relative path ending exactly in `.md` that equals a root (a
    /// `.md` file root) or lies under one (a directory root) with no
    /// `.`-named component below that root (the root's own components may
    /// be), and matches no `exclude` glob. Symlinks, non-UTF-8 names and
    /// missing roots are the walker's business and invisible here.
    pub fn in_walk_scope(&self, path: &str) -> bool {
        is_clean_relative(path)
            && path.ends_with(DOCUMENT_EXTENSION)
            && self.roots.iter().any(|root| {
                (path == root.as_str() || is_under(path, root))
                    && path
                        .split('/')
                        .skip(root.split('/').count())
                        .all(|component| !component.starts_with('.'))
            })
            && !self.is_excluded(path)
    }
}

/// `path` lies strictly under the directory `dir` (both root-relative).
pub fn is_under(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Non-empty, relative, `/`-separated, no empty, `.` or `..` component.
pub fn is_clean_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}
