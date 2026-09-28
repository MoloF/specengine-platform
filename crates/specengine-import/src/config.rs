//! The census config: the whole corpus convention, read at run time from TOML
//! (ADR-0008). Nothing about a particular corpus is known outside this file.
//!
//! ```toml
//! [corpus]
//! roots = ["design"]            # relative to the corpus root; default ["."]
//! extensions = ["md"]           # document extensions; default ["md"]
//! exclude = ["design/old/**"]   # globs over corpus-relative paths: `*`, `**`, `?`
//! [front_matter]
//! class_key = "kind"            # value -> per-class bucket; absent -> unclassified
//! [ids]
//! regex = '^[A-Z]{2}-[0-9]{3}$' # checked after look-alike normalization;
//!                               # an optional named group `prefix` names the prefix
//! [tables]
//! id_column = 0                 # 0-based column holding the ID
//! id_header = '^ID$'            # optional: only tables whose ID-column header matches
//! headerless = false            # also read blocks of `|` rows without a header
//! [sections]
//! id_attr = true                # {#ID} on headings
//! [links]
//! wiki = false                  # also check [[target]] links
//! wiki_root = "design"          # corpus-relative directory wiki targets resolve in; default "."
//! ```
//!
//! A wiki target resolves when a file under `wiki_root` has the target — as
//! written or with a document extension appended — as its relative path or
//! as a trailing part of it after a `/` (case ignored), or when it
//! exists relative to the linking document.

use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde::Deserialize;
use toml::Spanned;

use crate::script::{IdScript, Normalized};

/// A config error names the file and, when known, the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.path.display(), self.message),
            None => write!(f, "{}: {}", self.path.display(), self.message),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The validated, compiled census config.
#[derive(Debug, Clone)]
pub struct CensusConfig {
    /// Corpus-relative roots, without `..` and never absolute.
    pub roots: Vec<PathBuf>,
    /// Lower-case extensions without the dot.
    pub extensions: Vec<String>,
    /// Compiled exclude globs over `/`-separated corpus-relative paths.
    pub exclude: Vec<Regex>,
    pub class_key: Option<String>,
    pub ids: IdPattern,
    pub id_column: usize,
    pub id_header: Option<Regex>,
    /// Blocks of `|` rows without a header row are record tables too (by the
    /// any-row-has-an-ID rule, since they have no header to match).
    pub headerless_tables: bool,
    pub section_ids: bool,
    /// Corpus-relative directory wiki links resolve in; `None`: wiki links off.
    pub wiki_root: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    corpus: RawCorpus,
    #[serde(default)]
    front_matter: RawFrontMatter,
    ids: RawIds,
    #[serde(default)]
    tables: RawTables,
    #[serde(default)]
    sections: RawSections,
    #[serde(default)]
    links: RawLinks,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCorpus {
    /// The list itself is spanned so that an empty list is reported at its line.
    #[serde(default)]
    roots: Option<Spanned<Vec<Spanned<String>>>>,
    #[serde(default)]
    extensions: Option<Spanned<Vec<Spanned<String>>>>,
    #[serde(default)]
    exclude: Vec<Spanned<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawFrontMatter {
    #[serde(default)]
    class_key: Option<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIds {
    regex: Spanned<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawTables {
    #[serde(default)]
    id_column: usize,
    #[serde(default)]
    id_header: Option<Spanned<String>>,
    #[serde(default)]
    headerless: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSections {
    #[serde(default = "enabled")]
    id_attr: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLinks {
    #[serde(default)]
    wiki: bool,
    #[serde(default)]
    wiki_root: Option<Spanned<String>>,
}

impl Default for RawSections {
    fn default() -> Self {
        Self { id_attr: true }
    }
}

fn enabled() -> bool {
    true
}

impl CensusConfig {
    /// Reads and validates a config file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|error| ConfigError {
            path: path.to_path_buf(),
            line: None,
            message: format!("cannot read the census config: {error}"),
        })?;
        Self::parse(&text, path)
    }

    /// Parses and validates config text; `origin` only names it in errors.
    pub fn parse(text: &str, origin: &Path) -> Result<Self, ConfigError> {
        let error_at = |span: Option<Range<usize>>, message: String| ConfigError {
            path: origin.to_path_buf(),
            line: span.map(|span| line_of(text, span.start)),
            message,
        };
        let raw: RawConfig = toml::from_str(text)
            .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;

        let inside = |key: &str, value: &Spanned<String>| {
            let path = PathBuf::from(value.get_ref());
            let escapes = path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            });
            if escapes || value.get_ref().is_empty() {
                Err(error_at(
                    Some(value.span()),
                    format!(
                        "`{key}` entry `{}` must be a non-empty path relative to the corpus, without `..`",
                        value.get_ref()
                    ),
                ))
            } else {
                Ok(path)
            }
        };
        let roots = match &raw.corpus.roots {
            None => vec![PathBuf::from(".")],
            Some(roots) if roots.get_ref().is_empty() => {
                return Err(error_at(
                    Some(roots.span()),
                    "`corpus.roots` is empty".to_owned(),
                ));
            }
            Some(roots) => roots
                .get_ref()
                .iter()
                .map(|root| inside("corpus.roots", root))
                .collect::<Result<Vec<_>, _>>()?,
        };
        let wiki_root = match (raw.links.wiki, &raw.links.wiki_root) {
            (false, Some(root)) => {
                return Err(error_at(
                    Some(root.span()),
                    "`links.wiki_root` is set but `links.wiki` is false".to_owned(),
                ));
            }
            (false, None) => None,
            (true, None) => Some(PathBuf::from(".")),
            (true, Some(root)) => Some(inside("links.wiki_root", root)?),
        };

        let extensions = match raw.corpus.extensions {
            None => vec!["md".to_owned()],
            Some(list) if list.get_ref().is_empty() => {
                return Err(error_at(
                    Some(list.span()),
                    "`corpus.extensions` is empty".to_owned(),
                ));
            }
            Some(list) => list
                .into_inner()
                .into_iter()
                .map(|extension| {
                    let value = extension
                        .get_ref()
                        .trim_start_matches('.')
                        .to_ascii_lowercase();
                    if value.is_empty() || value.contains('/') {
                        Err(error_at(
                            Some(extension.span()),
                            format!(
                                "`corpus.extensions` entry `{}` is not an extension",
                                extension.get_ref()
                            ),
                        ))
                    } else {
                        Ok(value)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        let exclude = raw
            .corpus
            .exclude
            .into_iter()
            .map(|glob| {
                Regex::new(&glob_to_regex(glob.get_ref())).map_err(|error| {
                    error_at(
                        Some(glob.span()),
                        format!("`corpus.exclude` glob `{}`: {error}", glob.get_ref()),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let class_key = match raw.front_matter.class_key {
            Some(key) if key.get_ref().trim().is_empty() => {
                return Err(error_at(
                    Some(key.span()),
                    "`front_matter.class_key` is empty".to_owned(),
                ));
            }
            Some(key) => Some(key.into_inner().trim().to_owned()),
            None => None,
        };

        let regex = Regex::new(raw.ids.regex.get_ref()).map_err(|error| {
            error_at(Some(raw.ids.regex.span()), format!("`ids.regex`: {error}"))
        })?;
        if regex.is_match("") {
            return Err(error_at(
                Some(raw.ids.regex.span()),
                "`ids.regex` matches the empty string, so every cell would be an ID".to_owned(),
            ));
        }

        let id_header = raw
            .tables
            .id_header
            .map(|pattern| {
                Regex::new(pattern.get_ref()).map_err(|error| {
                    error_at(Some(pattern.span()), format!("`tables.id_header`: {error}"))
                })
            })
            .transpose()?;

        Ok(Self {
            roots,
            extensions,
            exclude,
            class_key,
            ids: IdPattern { regex },
            id_column: raw.tables.id_column,
            id_header,
            headerless_tables: raw.tables.headerless,
            section_ids: raw.sections.id_attr,
            wiki_root,
        })
    }

    /// Whether a `/`-separated corpus-relative path is excluded.
    pub fn is_excluded(&self, relative: &str) -> bool {
        self.exclude.iter().any(|glob| glob.is_match(relative))
    }

    /// Whether a file name carries one of the document extensions.
    pub fn is_document(&self, file_name: &str) -> bool {
        file_name.rsplit_once('.').is_some_and(|(stem, extension)| {
            !stem.is_empty()
                && self
                    .extensions
                    .iter()
                    .any(|wanted| wanted.eq_ignore_ascii_case(extension))
        })
    }
}

/// The ID regex of the config and how a match splits into a prefix.
#[derive(Debug, Clone)]
pub struct IdPattern {
    regex: Regex,
}

/// One ID found in a cell or an anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdMatch {
    /// The ID as written in the corpus.
    pub verbatim: String,
    /// The ID after look-alike normalization.
    pub id: String,
    /// Named group `prefix`, else the leading ASCII letters of `id`.
    pub prefix: String,
    pub script: IdScript,
}

impl IdPattern {
    /// The first ID in `text`, matched after look-alike normalization.
    pub fn find(&self, text: &str) -> Option<IdMatch> {
        let normalized = Normalized::new(text);
        let captures = self.regex.captures(&normalized.text)?;
        let whole = captures.get(0)?;
        if whole.as_str().is_empty() {
            return None;
        }
        let id = whole.as_str().to_owned();
        let prefix = match captures.name("prefix") {
            Some(group) => group.as_str().to_owned(),
            None => id.chars().take_while(char::is_ascii_alphabetic).collect(),
        };
        let original = normalized.original_range(whole.range());
        let verbatim = text.get(original).unwrap_or(&id).to_owned();
        let script = IdScript::of(&verbatim);
        Some(IdMatch {
            verbatim,
            id,
            prefix,
            script,
        })
    }
}

/// 1-based line of a byte offset.
fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// `*` = any run without `/`, `**` = any run, `**/` = any directories (also
/// none), `?` = one char other than `/`; anchored at both ends.
fn glob_to_regex(glob: &str) -> String {
    let mut pattern = String::from("^");
    let chars: Vec<char> = glob.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '*' if chars.get(index + 1) == Some(&'*') => {
                if chars.get(index + 2) == Some(&'/') {
                    pattern.push_str("(?:.*/)?");
                    index += 3;
                } else {
                    pattern.push_str(".*");
                    index += 2;
                }
            }
            '*' => {
                pattern.push_str("[^/]*");
                index += 1;
            }
            '?' => {
                pattern.push_str("[^/]");
                index += 1;
            }
            other => {
                pattern.push_str(&regex::escape(other.encode_utf8(&mut [0; 4])));
                index += 1;
            }
        }
    }
    pattern.push('$');
    pattern
}
