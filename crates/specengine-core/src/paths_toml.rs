//! `Paths::from_toml`: the `[paths]` table of `specengine.toml`, and only
//! that table (`crates/specengine-core/README.md`, "`[paths]`"). Pure: the
//! text in, the checked values out; which of them exist on disk is the
//! walker's business.
//!
//! Role keys `spec`, `records`, `features`, `generated`, `archive` (07 §5,
//! defaults `docs/canon/architecture.md` "Spec layout in a project");
//! `roots`, the walked directories or `.md` files (default: the role
//! directories but `generated`, SpecEngine's own output);
//! `exclude`, census globs (`*`, `**`, `?`) over root-relative file paths;
//! for `spec check` (docs/features/spec-check.md): `tier0` (the one file
//! canon tier 0 may be), `tier1_name` (the file name canon tier 1 is
//! restricted to), `index` (the generated index, capped by `index_bytes`),
//! `link_base` (the fallback directory Markdown file links resolve from;
//! docs/features/spec-check-links.md).
//! Every path is root-relative with `/`: no leading `/`, no `..`, no `.` or
//! empty component (one trailing `/` of a directory is dropped). An unknown
//! key, a wrong type or a bad path is an error `file:line: message`
//! through [`PathsError::at`]; other tables are ignored.
//!
//! The walk's rules as pure predicates over a root-relative path live in
//! [`WalkScope`] ([`Paths::walk_scope`]; one-off: [`Paths::is_excluded`],
//! [`Paths::in_walk_scope`]): the store's walker and the check's link rules
//! share them.

use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::walk_scope::WalkScope;

/// Default of the `spec` role key: the business-logic tree.
pub const DEFAULT_SPEC: &str = "docs/spec";
/// Default of the `records` role key: one file per record.
pub const DEFAULT_RECORDS: &str = "docs/records";
/// Default of the `features` role key: change specs.
pub const DEFAULT_FEATURES: &str = "docs/features";
/// Default of the `generated` role key: SpecEngine's own output, never walked
/// by default.
pub const DEFAULT_GENERATED: &str = "docs/generated";
/// Default of the `archive` role key.
pub const DEFAULT_ARCHIVE: &str = "docs/archive";

/// The `[paths]` table, defaults applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub spec: String,
    pub records: String,
    pub features: String,
    pub generated: String,
    pub archive: String,
    /// Walked roots: directories or single `.md` files, root-relative, in
    /// the order written (duplicates dropped). Default: `spec`, `records`,
    /// `features`, `archive`.
    pub roots: Vec<String>,
    /// Census globs over root-relative file paths: `*` any run without `/`,
    /// `**` any run, `**/` any directories (also none), `?` one character
    /// but `/`; anchored at both ends.
    pub exclude: Vec<String>,
    /// `roots` was written: a missing root is then a cause of "cannot
    /// check"; a missing default role root is not.
    pub roots_written: bool,
    /// The one file that may be canon tier 0 (`spec check`); absent: no
    /// such rule.
    pub tier0: Option<String>,
    /// The file name canon tier 1 is restricted to; absent: no such rule.
    pub tier1_name: Option<String>,
    /// The generated index, capped by `index_bytes`; absent: no index cap.
    pub index: Option<String>,
    /// The directory a relative Markdown file link is retried from when it
    /// names no walked document from the linking file's directory; absent:
    /// no retry. Read by the check only (outside the index fingerprint);
    /// its existence is never checked.
    pub link_base: Option<String>,
}

impl Default for Paths {
    fn default() -> Self {
        Self::with_roles(
            DEFAULT_SPEC.to_owned(),
            DEFAULT_RECORDS.to_owned(),
            DEFAULT_FEATURES.to_owned(),
            DEFAULT_GENERATED.to_owned(),
            DEFAULT_ARCHIVE.to_owned(),
        )
    }
}

impl Paths {
    /// Reads the `[paths]` table; no table gives [`Paths::default`].
    pub fn from_toml(text: &str) -> Result<Self, PathsError> {
        paths_from_toml(text)
    }

    /// The walk's rules with the exclude globs compiled: build it once for
    /// many paths (the walker, the check).
    pub fn walk_scope(&self) -> WalkScope {
        WalkScope::new(self)
    }

    /// [`WalkScope::is_excluded`] for one path (compiles the globs).
    pub fn is_excluded(&self, path: &str) -> bool {
        self.walk_scope().is_excluded(path)
    }

    /// [`WalkScope::in_walk_scope`] for one path (compiles the globs).
    pub fn in_walk_scope(&self, path: &str) -> bool {
        self.walk_scope().in_walk_scope(path)
    }

    fn with_roles(
        spec: String,
        records: String,
        features: String,
        generated: String,
        archive: String,
    ) -> Self {
        let mut roots = Vec::new();
        for role in [&spec, &records, &features, &archive] {
            if !roots.contains(role) {
                roots.push(role.clone());
            }
        }
        Self {
            spec,
            records,
            features,
            generated,
            archive,
            roots,
            exclude: Vec::new(),
            roots_written: false,
            tier0: None,
            tier1_name: None,
            index: None,
            link_base: None,
        }
    }
}

/// A `[paths]` error: the 1-based line when known, and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathsError {
    pub line: Option<usize>,
    pub message: String,
}

impl PathsError {
    /// `file:line: message` (`file: message` without a line).
    pub fn at(&self, file: &str) -> String {
        match self.line {
            Some(line) => format!("{file}:{line}: {}", self.message),
            None => format!("{file}: {}", self.message),
        }
    }
}

impl fmt::Display for PathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for PathsError {}

/// [`Paths::from_toml`] as a function.
pub fn paths_from_toml(text: &str) -> Result<Paths, PathsError> {
    let error_at = |span: Option<Range<usize>>, message: String| PathsError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let raw: RawFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(raw) = raw.paths else {
        return Ok(Paths::default());
    };

    let role = |key: &str, value: Option<Spanned<String>>, default: &str| match value {
        None => Ok(default.to_owned()),
        Some(value) => {
            let span = value.span();
            checked_path(value.get_ref(), true)
                .map_err(|problem| error_at(Some(span), format!("`{key}`: {problem}")))
        }
    };
    let spec = role("spec", raw.spec, DEFAULT_SPEC)?;
    let records = role("records", raw.records, DEFAULT_RECORDS)?;
    let features = role("features", raw.features, DEFAULT_FEATURES)?;
    let generated = role("generated", raw.generated, DEFAULT_GENERATED)?;
    let archive = role("archive", raw.archive, DEFAULT_ARCHIVE)?;
    let mut paths = Paths::with_roles(spec, records, features, generated, archive);

    if let Some(roots) = raw.roots {
        paths.roots.clear();
        paths.roots_written = true;
        for root in roots {
            let span = root.span();
            let checked = checked_path(root.get_ref(), true)
                .map_err(|problem| error_at(Some(span), format!("`roots`: {problem}")))?;
            if !paths.roots.contains(&checked) {
                paths.roots.push(checked);
            }
        }
    }
    if let Some(exclude) = raw.exclude {
        for glob in exclude {
            let span = glob.span();
            let checked = checked_path(glob.get_ref(), false)
                .map_err(|problem| error_at(Some(span), format!("`exclude`: {problem}")))?;
            paths.exclude.push(checked);
        }
    }
    let file = |key: &str, value: Option<Spanned<String>>| match value {
        None => Ok(None),
        Some(value) => {
            let span = value.span();
            checked_path(value.get_ref(), false)
                .map(Some)
                .map_err(|problem| error_at(Some(span), format!("`{key}`: {problem}")))
        }
    };
    paths.tier0 = file("tier0", raw.tier0)?;
    paths.index = file("index", raw.index)?;
    // A directory, under the roots' rules; `""` is an error, not "no base".
    if let Some(base) = raw.link_base {
        let span = base.span();
        let checked = checked_path(base.get_ref(), true)
            .map_err(|problem| error_at(Some(span), format!("`link_base`: {problem}")))?;
        paths.link_base = Some(checked);
    }
    if let Some(name) = raw.tier1_name {
        let span = name.span();
        let value = name.into_inner();
        if value.is_empty() || value.contains('/') || value == "." || value == ".." {
            return Err(error_at(
                Some(span),
                format!("`tier1_name`: {value:?} is not a single file name"),
            ));
        }
        paths.tier1_name = Some(value);
    }
    Ok(paths)
}

/// A root-relative `/` path: no leading `/`, no `..`, no `.` or empty
/// component. `directory`: one trailing `/` is dropped.
pub(crate) fn checked_path(value: &str, directory: bool) -> Result<String, String> {
    let trimmed = if directory {
        value.strip_suffix('/').unwrap_or(value)
    } else {
        value
    };
    if trimmed.is_empty() {
        return Err(format!("{value:?} is empty"));
    }
    if trimmed.starts_with('/') {
        return Err(format!("{value:?} is absolute; paths are root-relative"));
    }
    for component in trimmed.split('/') {
        match component {
            ".." => return Err(format!("{value:?} leaves the root through `..`")),
            "." => return Err(format!("{value:?} has a `.` component")),
            "" => return Err(format!("{value:?} has an empty component")),
            _ => {}
        }
    }
    Ok(trimmed.to_owned())
}

/// The file: only `[paths]` is read; every other table is ignored.
#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    paths: Option<RawPaths>,
}

/// The `[paths]` table; any other key is an error.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPaths {
    #[serde(default)]
    spec: Option<Spanned<String>>,
    #[serde(default)]
    records: Option<Spanned<String>>,
    #[serde(default)]
    features: Option<Spanned<String>>,
    #[serde(default)]
    generated: Option<Spanned<String>>,
    #[serde(default)]
    archive: Option<Spanned<String>>,
    #[serde(default)]
    roots: Option<Vec<Spanned<String>>>,
    #[serde(default)]
    exclude: Option<Vec<Spanned<String>>>,
    #[serde(default)]
    tier0: Option<Spanned<String>>,
    #[serde(default)]
    tier1_name: Option<Spanned<String>>,
    #[serde(default)]
    index: Option<Spanned<String>>,
    #[serde(default)]
    link_base: Option<Spanned<String>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
