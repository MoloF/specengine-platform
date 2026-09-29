//! `Paths::from_toml`: the `[paths]` table of `specengine.toml`, and only
//! that table (docs/features/spec-index.md, "Data"). Pure: the text in, the
//! checked values out; which of them exist on disk is the walker's business.
//!
//! Role keys `spec`, `records`, `features`, `generated`, `archive` (07 §5,
//! defaults 05 §2); `roots`, the walked directories or `.md` files (default:
//! the role directories but `generated`, SpecEngine's own output);
//! `exclude`, census globs (`*`, `**`, `?`) over root-relative file paths.
//! Every path is root-relative with `/`: no leading `/`, no `..`, no `.` or
//! empty component (one trailing `/` of a directory is dropped). An unknown
//! key, a wrong type or a bad path is an error `file:line: message`
//! through [`PathsError::at`]; other tables are ignored.

use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

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
    Ok(paths)
}

/// A root-relative `/` path: no leading `/`, no `..`, no `.` or empty
/// component. `directory`: one trailing `/` is dropped.
fn checked_path(value: &str, directory: bool) -> Result<String, String> {
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
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
