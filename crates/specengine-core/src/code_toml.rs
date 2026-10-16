//! The `[code]` table of `specengine.toml`
//! (`docs/canon/code-bindings.md` "The `[code]` table"): which code tree
//! the commands that read code (`spec refs`, `spec lock accept`) walk. Read apart from the reads ([`CodeConfig::from_toml`];
//! [`crate::ProjectConfig::from_toml`] only name-checks `code`, so a bad
//! `[code]` fails no other command). Pure: the text in, the checked values
//! out; which roots exist is the walker's business.
//!
//! - **Off**: no table, no default root (ADR-0008).
//! - **Closed**: `roots` (required, a non-empty list), `exclude` and
//!   `frameworks` (optional), lists of strings. Another key is an error at
//!   its line (07 s5's `languages`, `crates`, `scip` named as not built), a
//!   wrong type too.
//! - **Frameworks**: names of the shape `[a-z][a-z0-9_-]{0,31}`, repeats
//!   dropped, byte-sorted, each name's line kept
//!   ([`CodeConfig::framework_line`]): the shape only; which names are
//!   known is the code crate's data, checked by the code index's refresh
//!   (`docs/canon/code-roles.md` "`[code] frameworks`"). Absent or `[]`: off.
//! - **Roots**: root-relative `/` paths under `[paths]`' rules (no leading
//!   `/`, no `.`, `..` or empty component, one trailing `/` dropped),
//!   repeats dropped; each a directory or a `.rs` or `.ron` file.
//! - **Exclude**: globs over root-relative file paths in the dialect of
//!   `[paths] exclude` and the zones (`docs/canon/gate.md` "Config"): `*`,
//!   `**`, `?`, every other character literal; `[`, `]`, `{`, `}` and `\`
//!   refused, as another dialect's syntax would be taken literally.
//!
//! Every error is a [`ProjectError`] at the line (`file:line` through
//! [`ProjectError::at`]).

use std::collections::BTreeMap;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::gate::dialect_problem;
use crate::glob::Glob;
use crate::paths_toml::checked_path;
use crate::project_toml::ProjectError;

/// The keys `[code]` takes.
const KEYS: [&str; 3] = ["roots", "exclude", "frameworks"];

/// 07 s5's `[code]` keys, built by later slices: named as such when written.
const NOT_BUILT_KEYS: [&str; 3] = ["languages", "crates", "scip"];

/// The suffix of a glob that matches everything under a directory.
const ANY_BELOW: &str = "/**";

/// The longest name `frameworks` takes, in bytes.
const MAX_NAME_BYTES: usize = 32;

/// The `[code]` table, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeConfig {
    /// The walked roots, root-relative, in the order written, repeats
    /// dropped; never empty.
    pub roots: Vec<String>,
    /// The exclude globs as written.
    pub exclude: Vec<String>,
    /// The listed framework names, repeats dropped, byte-sorted; empty:
    /// the roles are off.
    pub frameworks: Vec<String>,
    /// Each listed name's line (its first, when repeated).
    framework_lines: BTreeMap<String, usize>,
    /// `exclude`, compiled.
    globs: Vec<Glob>,
    /// For each glob ending in `/**`, the glob before that suffix: a
    /// directory it matches holds only excluded paths.
    below: Vec<Glob>,
}

impl CodeConfig {
    /// Reads the `[code]` table; `None` without one (the code layer is
    /// off).
    pub fn from_toml(text: &str) -> Result<Option<Self>, ProjectError> {
        code_from_toml(text)
    }

    /// `path` (root-relative, `/`) matches an `exclude` glob.
    pub fn is_excluded(&self, path: &str) -> bool {
        self.globs.iter().any(|glob| glob.matches(path))
    }

    /// Every path below the directory `dir` (root-relative, `/`) matches an
    /// `exclude` glob: a glob `G/**` whose `G` matches `dir` itself. The
    /// walk need not list such a directory.
    pub fn excludes_all_below(&self, dir: &str) -> bool {
        self.below.iter().any(|glob| glob.matches(dir))
    }

    /// The line `name` is listed at in `frameworks` (its first, when
    /// repeated); `None` when it is not listed.
    pub fn framework_line(&self, name: &str) -> Option<usize> {
        self.framework_lines.get(name).copied()
    }
}

/// [`CodeConfig::from_toml`] as a function.
pub fn code_from_toml(text: &str) -> Result<Option<CodeConfig>, ProjectError> {
    let error_at = |span: Option<Range<usize>>, message: String| ProjectError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let raw: RawCodeFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(table) = raw.code else {
        return Ok(None);
    };
    let table_span = table.span();
    let table = table.into_inner();

    let mut roots_value = None;
    let mut exclude_value = None;
    let mut frameworks_value = None;
    for (key, value) in &table {
        match key.as_str() {
            "roots" => roots_value = Some(value),
            "exclude" => exclude_value = Some(value),
            "frameworks" => frameworks_value = Some(value),
            other if NOT_BUILT_KEYS.contains(&other) => {
                return Err(error_at(
                    Some(value.span()),
                    format!(
                        "`[code] {other}` is not built: `[code]` takes `{}` only",
                        KEYS.join("` and `")
                    ),
                ));
            }
            other => {
                return Err(error_at(
                    Some(value.span()),
                    format!(
                        "`[code]` has no key `{other}`: it takes `{}` only",
                        KEYS.join("` and `")
                    ),
                ));
            }
        }
    }

    let Some(roots_value) = roots_value else {
        return Err(error_at(
            Some(table_span),
            "`[code]` has no `roots`: list the directories and `.rs` or `.ron` files to read \
             (`roots = [\"src\"]`)"
                .to_owned(),
        ));
    };
    let written = strings("roots", roots_value, &error_at)?;
    if written.is_empty() {
        return Err(error_at(
            Some(roots_value.span()),
            "`[code] roots` is empty: list the directories and `.rs` or `.ron` files to read"
                .to_owned(),
        ));
    }
    let mut roots: Vec<String> = Vec::with_capacity(written.len());
    for root in written {
        let checked = checked_path(&root, true).map_err(|problem| {
            error_at(
                Some(roots_value.span()),
                format!("`[code] roots`: {problem}"),
            )
        })?;
        if !roots.contains(&checked) {
            roots.push(checked);
        }
    }

    let exclude = match exclude_value {
        None => Vec::new(),
        Some(value) => {
            let globs = strings("exclude", value, &error_at)?;
            for glob in &globs {
                if let Some(problem) = dialect_problem(glob, "a `[code] exclude` glob") {
                    return Err(error_at(
                        Some(value.span()),
                        format!("`[code] exclude`: {problem}"),
                    ));
                }
            }
            globs
        }
    };
    let framework_lines = match frameworks_value {
        None => BTreeMap::new(),
        Some(value) => framework_names(text, value, &error_at)?,
    };
    let frameworks = framework_lines.keys().cloned().collect();

    let globs = exclude.iter().map(|glob| Glob::new(glob)).collect();
    let below = exclude
        .iter()
        .filter_map(|glob| glob.strip_suffix(ANY_BELOW))
        .filter(|prefix| !prefix.is_empty())
        .map(Glob::new)
        .collect();
    Ok(Some(CodeConfig {
        roots,
        exclude,
        frameworks,
        framework_lines,
        globs,
        below,
    }))
}

/// The names of `frameworks` (its value `value` in `text`), each with its
/// line (the first, when repeated), byte-sorted; an error at the value
/// when it is no list of strings, at the name when a name has another
/// shape.
fn framework_names(
    text: &str,
    value: &Spanned<toml::Value>,
    error_at: &impl Fn(Option<Range<usize>>, String) -> ProjectError,
) -> Result<BTreeMap<String, usize>, ProjectError> {
    match value.get_ref() {
        toml::Value::Array(items) => {
            if let Some(other) = items.iter().find(|item| !item.is_str()) {
                return Err(error_at(
                    Some(value.span()),
                    format!(
                        "`[code] frameworks` holds a {}: it lists names",
                        other.type_str()
                    ),
                ));
            }
        }
        other => {
            return Err(error_at(
                Some(value.span()),
                format!(
                    "`[code] frameworks` is a {}, not a list of names",
                    other.type_str()
                ),
            ));
        }
    }
    // Every item is a string: read again with each one's span.
    let spanned: RawFrameworksFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let names = spanned
        .code
        .and_then(|code| code.frameworks)
        .unwrap_or_default();
    let mut lines = BTreeMap::new();
    for name in names {
        let span = name.span();
        let name = name.into_inner();
        if !is_framework_name(&name) {
            return Err(error_at(
                Some(span),
                format!(
                    "`[code] frameworks`: `{name}` is not a name: a lowercase letter, then at most \
                     31 of a-z, 0-9, `_`, `-`"
                ),
            ));
        }
        let line = line_of(text, span.start);
        lines.entry(name).or_insert(line);
    }
    Ok(lines)
}

/// `[a-z][a-z0-9_-]{0,31}`.
fn is_framework_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.len() <= MAX_NAME_BYTES
        && bytes.iter().all(|&byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

/// The strings of a `[code]` list, or an error at the value naming `key`.
fn strings(
    key: &str,
    value: &Spanned<toml::Value>,
    error_at: &impl Fn(Option<Range<usize>>, String) -> ProjectError,
) -> Result<Vec<String>, ProjectError> {
    match value.get_ref() {
        toml::Value::Array(items) => items
            .iter()
            .map(|item| match item {
                toml::Value::String(text) => Ok(text.clone()),
                other => Err(error_at(
                    Some(value.span()),
                    format!(
                        "`[code] {key}` holds a {}: it lists root-relative paths",
                        other.type_str()
                    ),
                )),
            })
            .collect(),
        other => Err(error_at(
            Some(value.span()),
            format!(
                "`[code] {key}` is a {}, not a list of root-relative paths",
                other.type_str()
            ),
        )),
    }
}

/// Only `[code]`, its keys read here.
#[derive(Deserialize)]
struct RawCodeFile {
    #[serde(default)]
    code: Option<Spanned<BTreeMap<String, Spanned<toml::Value>>>>,
}

/// Only `[code] frameworks`, once it is known to be a list of strings: each
/// with its span.
#[derive(Deserialize)]
struct RawFrameworksFile {
    #[serde(default)]
    code: Option<RawFrameworks>,
}

#[derive(Deserialize)]
struct RawFrameworks {
    #[serde(default)]
    frameworks: Option<Vec<Spanned<String>>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}
