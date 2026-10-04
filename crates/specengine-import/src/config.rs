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
//!
//! The import keys (`docs/features/import-records.md` AC-02, `[documents]`
//! of `docs/features/import-gaps.md` AC-08) live in the same file and are
//! compiled into [`ImportConfig`]; the census validates them and reads none
//! of them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use regex::Regex;
use serde::Deserialize;
use toml::Spanned;

use crate::script::{IdScript, Normalized};

/// ID-like text when `ids.like` is absent: an upper-case letter run, a
/// hyphen, digits (`docs/features/import-records.md` AC-02, AC-06).
pub const DEFAULT_LIKE: &str = r"\p{Lu}[\p{Lu}\p{N}]*-[0-9]+";

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
    /// The import keys; never read by the census.
    pub import: ImportConfig,
}

/// The import keys of the config, compiled. Every key is optional and off
/// by default, `like` aside.
#[derive(Debug, Clone)]
pub struct ImportConfig {
    /// Over the first header cell of a two-column table opening the body.
    pub header_table: Option<Regex>,
    /// The header row of such a table is a field too.
    pub header_row_field: bool,
    /// Header key as written → target key.
    pub key_map: BTreeMap<String, String>,
    /// Per target key: value as written → target value.
    pub value_map: BTreeMap<String, BTreeMap<String, String>>,
    /// ID-like text, for the unclaimed counter.
    pub like: Regex,
    /// Latin prefixes whose IDs are unique per document (ADR-0026).
    pub feature_prefixes: BTreeSet<String>,
    /// Hyphenless ID patterns: counted only.
    pub hyphenless: Vec<Pattern>,
    /// Legacy prefix as written → Latin prefix.
    pub legacy: BTreeMap<String, String>,
    /// The mapped cell of a record row; `None`: the column after the ID column.
    pub text_column: Option<usize>,
    /// Over header cells: the first match is the mapped cell (beats `text_column`).
    pub text_header: Option<Regex>,
    /// Over an ID cell holding no ID: a local number.
    pub local_number: Option<Regex>,
    /// List items opening with a strong ID are records.
    pub list_lead_in: bool,
    /// One of these is stripped after a lead-in ID.
    pub separators: Vec<String>,
    /// Globs over corpus-relative paths: documents whose IDs are references.
    pub reference_paths: Vec<Regex>,
    /// Over header cells: tables whose IDs are references.
    pub reference_headers: Vec<Regex>,
    /// Corpus-relative directory a missing file link is retried from.
    pub link_base: Option<PathBuf>,
    pub code: CodeConfig,
    pub documents: DocumentsConfig,
}

/// A configured regex and its text as written (for `labels.json`).
#[derive(Debug, Clone)]
pub struct Pattern {
    pub source: String,
    pub regex: Regex,
}

/// `[documents]`: where a document names the ID it defines itself
/// (`docs/features/import-gaps.md` AC-02). No defaults: absent is off.
#[derive(Debug, Clone, Default)]
pub struct DocumentsConfig {
    /// A header target (after `key_map`) whose value is the document's ID.
    pub id_key: Option<String>,
    /// Over the corpus-relative `/`-separated path; group `id` is the ID.
    pub id_path: Option<Regex>,
}

/// `[code]`: files scanned for literal document paths, never built.
#[derive(Debug, Clone, Default)]
pub struct CodeConfig {
    /// Corpus-relative, without `..`; empty: no code scan.
    pub roots: Vec<PathBuf>,
    /// Lower-case, without the dot; empty: every file under the roots.
    pub extensions: Vec<String>,
    pub exclude: Vec<Regex>,
    /// Path prefixes a document path is also matched without.
    pub strip: Vec<String>,
}

impl CodeConfig {
    /// Whether a file under a code root is scanned.
    pub fn keeps(&self, file_name: &str, relative: &str) -> bool {
        let listed = self.extensions.is_empty()
            || file_name.rsplit_once('.').is_some_and(|(stem, extension)| {
                !stem.is_empty()
                    && self
                        .extensions
                        .iter()
                        .any(|wanted| wanted.eq_ignore_ascii_case(extension))
            });
        listed && !self.exclude.iter().any(|glob| glob.is_match(relative))
    }
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
    #[serde(default)]
    lists: RawLists,
    #[serde(default)]
    definitions: RawDefinitions,
    #[serde(default)]
    code: RawCode,
    #[serde(default)]
    documents: RawDocuments,
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
    #[serde(default)]
    header_table: Option<Spanned<String>>,
    #[serde(default)]
    header_row_field: bool,
    #[serde(default)]
    key_map: BTreeMap<String, Spanned<String>>,
    #[serde(default)]
    value_map: BTreeMap<String, BTreeMap<String, Spanned<String>>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIds {
    regex: Spanned<String>,
    #[serde(default)]
    like: Option<Spanned<String>>,
    #[serde(default)]
    feature_prefixes: Vec<Spanned<String>>,
    #[serde(default)]
    hyphenless: Vec<Spanned<String>>,
    #[serde(default)]
    legacy: BTreeMap<String, Spanned<String>>,
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
    #[serde(default)]
    text_column: Option<usize>,
    #[serde(default)]
    text_header: Option<Spanned<String>>,
    #[serde(default)]
    local_number: Option<Spanned<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLists {
    #[serde(default)]
    lead_in: bool,
    #[serde(default)]
    separators: Vec<Spanned<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawDefinitions {
    #[serde(default)]
    reference_paths: Vec<Spanned<String>>,
    #[serde(default)]
    reference_headers: Vec<Spanned<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawCode {
    #[serde(default)]
    roots: Vec<Spanned<String>>,
    #[serde(default)]
    extensions: Vec<Spanned<String>>,
    #[serde(default)]
    exclude: Vec<Spanned<String>>,
    #[serde(default)]
    strip: Vec<Spanned<String>>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawDocuments {
    #[serde(default)]
    id_key: Option<Spanned<String>>,
    #[serde(default)]
    id_path: Option<Spanned<String>>,
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
    #[serde(default)]
    base: Option<Spanned<String>>,
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

        // Before the census keys below are moved out of `raw`.
        let import = compile_import(&raw, &error_at, &inside)?;

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
            import,
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

/// A config error at a span of the config text.
type ErrorAt<'a> = dyn Fn(Option<Range<usize>>, String) -> ConfigError + 'a;

/// A corpus-relative path key, validated (no `..`, not absolute, not empty).
type Inside<'a> = dyn Fn(&str, &Spanned<String>) -> Result<PathBuf, ConfigError> + 'a;

/// Compiles and validates the import keys; an error names the key's line.
fn compile_import(
    raw: &RawConfig,
    error_at: &ErrorAt<'_>,
    inside: &Inside<'_>,
) -> Result<ImportConfig, ConfigError> {
    let regex = |key: &str, value: &Spanned<String>| {
        Regex::new(value.get_ref())
            .map_err(|error| error_at(Some(value.span()), format!("`{key}`: {error}")))
    };
    let non_empty_regex = |key: &str, value: &Spanned<String>| {
        let compiled = regex(key, value)?;
        if compiled.is_match("") {
            return Err(error_at(
                Some(value.span()),
                format!("`{key}` matches the empty string"),
            ));
        }
        Ok(compiled)
    };
    let glob = |key: &str, value: &Spanned<String>| {
        Regex::new(&glob_to_regex(value.get_ref())).map_err(|error| {
            error_at(
                Some(value.span()),
                format!("`{key}` glob `{}`: {error}", value.get_ref()),
            )
        })
    };
    let filled = |key: &str, value: &Spanned<String>| {
        if value.get_ref().trim().is_empty() {
            Err(error_at(
                Some(value.span()),
                format!("`{key}` holds an empty entry"),
            ))
        } else {
            Ok(value.get_ref().clone())
        }
    };
    let latin_prefix = |key: &str, value: &Spanned<String>| {
        if is_latin_prefix(value.get_ref()) {
            Ok(value.get_ref().clone())
        } else {
            Err(error_at(
                Some(value.span()),
                format!(
                    "`{key}` entry `{}` is not a Latin prefix (an ASCII capital, then ASCII capitals or digits)",
                    value.get_ref()
                ),
            ))
        }
    };

    let front = &raw.front_matter;
    let header_table = front
        .header_table
        .as_ref()
        .map(|value| non_empty_regex("front_matter.header_table", value))
        .transpose()?;
    let mut key_map = BTreeMap::new();
    for (written, target) in &front.key_map {
        if written.trim().is_empty() {
            return Err(error_at(
                Some(target.span()),
                "`front_matter.key_map` has an empty key".to_owned(),
            ));
        }
        key_map.insert(written.clone(), filled("front_matter.key_map", target)?);
    }
    let mut value_map = BTreeMap::new();
    for (target, values) in &front.value_map {
        let mut compiled = BTreeMap::new();
        for (written, value) in values {
            if written.trim().is_empty() {
                return Err(error_at(
                    Some(value.span()),
                    format!("`front_matter.value_map.{target}` has an empty key"),
                ));
            }
            compiled.insert(written.clone(), filled("front_matter.value_map", value)?);
        }
        value_map.insert(target.clone(), compiled);
    }

    let ids = &raw.ids;
    let like = match &ids.like {
        Some(value) => non_empty_regex("ids.like", value)?,
        None => Regex::new(DEFAULT_LIKE)
            .map_err(|error| error_at(None, format!("the default `ids.like`: {error}")))?,
    };
    let feature_prefixes = ids
        .feature_prefixes
        .iter()
        .map(|value| latin_prefix("ids.feature_prefixes", value))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut hyphenless: Vec<Pattern> = Vec::new();
    for value in &ids.hyphenless {
        if hyphenless
            .iter()
            .any(|pattern| &pattern.source == value.get_ref())
        {
            return Err(error_at(
                Some(value.span()),
                format!("`ids.hyphenless` lists `{}` twice", value.get_ref()),
            ));
        }
        hyphenless.push(Pattern {
            source: value.get_ref().clone(),
            regex: non_empty_regex("ids.hyphenless", value)?,
        });
    }
    let mut legacy = BTreeMap::new();
    for (written, target) in &ids.legacy {
        let mut chars = written.chars();
        let letters =
            chars.next().is_some_and(char::is_alphabetic) && chars.all(char::is_alphanumeric);
        if !letters {
            return Err(error_at(
                Some(target.span()),
                format!(
                    "`ids.legacy` key `{written}` is not a letter run (a letter, then letters or digits)"
                ),
            ));
        }
        legacy.insert(written.clone(), latin_prefix("ids.legacy", target)?);
    }

    let tables = &raw.tables;
    let text_header = tables
        .text_header
        .as_ref()
        .map(|value| non_empty_regex("tables.text_header", value))
        .transpose()?;
    let local_number = tables
        .local_number
        .as_ref()
        .map(|value| non_empty_regex("tables.local_number", value))
        .transpose()?;

    let separators = raw
        .lists
        .separators
        .iter()
        .map(|value| {
            if value.get_ref().is_empty() {
                Err(error_at(
                    Some(value.span()),
                    "`lists.separators` holds an empty entry".to_owned(),
                ))
            } else {
                Ok(value.get_ref().clone())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let reference_paths = raw
        .definitions
        .reference_paths
        .iter()
        .map(|value| glob("definitions.reference_paths", value))
        .collect::<Result<Vec<_>, _>>()?;
    let reference_headers = raw
        .definitions
        .reference_headers
        .iter()
        .map(|value| non_empty_regex("definitions.reference_headers", value))
        .collect::<Result<Vec<_>, _>>()?;

    let link_base = raw
        .links
        .base
        .as_ref()
        .map(|value| inside("links.base", value))
        .transpose()?;

    let code = &raw.code;
    let code = CodeConfig {
        roots: code
            .roots
            .iter()
            .map(|value| {
                let root = inside("code.roots", value)?;
                if root
                    .components()
                    .all(|component| component == Component::CurDir)
                {
                    return Err(error_at(
                        Some(value.span()),
                        format!(
                            "`code.roots` entry `{}` is the corpus root; name the code directories inside it",
                            value.get_ref()
                        ),
                    ));
                }
                Ok(root)
            })
            .collect::<Result<Vec<_>, _>>()?,
        extensions: code
            .extensions
            .iter()
            .map(|value| {
                let extension = value.get_ref().trim_start_matches('.').to_ascii_lowercase();
                if extension.is_empty() || extension.contains('/') {
                    Err(error_at(
                        Some(value.span()),
                        format!(
                            "`code.extensions` entry `{}` is not an extension",
                            value.get_ref()
                        ),
                    ))
                } else {
                    Ok(extension)
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        exclude: code
            .exclude
            .iter()
            .map(|value| glob("code.exclude", value))
            .collect::<Result<Vec<_>, _>>()?,
        strip: code
            .strip
            .iter()
            .map(|value| filled("code.strip", value))
            .collect::<Result<Vec<_>, _>>()?,
    };

    let documents = &raw.documents;
    let id_key = match &documents.id_key {
        Some(value) if value.get_ref().trim().is_empty() => {
            return Err(error_at(
                Some(value.span()),
                "`documents.id_key` is empty".to_owned(),
            ));
        }
        // Header keys are read trimmed: a key with blanks around it would
        // never match.
        Some(value) if value.get_ref().trim() != value.get_ref() => {
            return Err(error_at(
                Some(value.span()),
                "`documents.id_key` has blanks around it; no header key would match".to_owned(),
            ));
        }
        Some(value) => Some(value.get_ref().clone()),
        None => None,
    };
    let id_path = match &documents.id_path {
        Some(value) => {
            let compiled = non_empty_regex("documents.id_path", value)?;
            if !compiled.capture_names().any(|name| name == Some("id")) {
                return Err(error_at(
                    Some(value.span()),
                    "`documents.id_path` has no group `id`".to_owned(),
                ));
            }
            Some(compiled)
        }
        None => None,
    };

    Ok(ImportConfig {
        header_table,
        header_row_field: front.header_row_field,
        key_map,
        value_map,
        like,
        feature_prefixes,
        hyphenless,
        legacy,
        text_column: tables.text_column,
        text_header,
        local_number,
        list_lead_in: raw.lists.lead_in,
        separators,
        reference_paths,
        reference_headers,
        link_base,
        code,
        documents: DocumentsConfig { id_key, id_path },
    })
}

/// An ASCII capital, then ASCII capitals or digits (ADR-0009).
pub(crate) fn is_latin_prefix(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|first| first.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
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
        self.find_range(text).map(|(found, _)| found)
    }

    /// [`IdPattern::find`] and the byte range of the match in `text`.
    pub(crate) fn find_range(&self, text: &str) -> Option<(IdMatch, Range<usize>)> {
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
        let verbatim = text.get(original.clone()).unwrap_or(&id).to_owned();
        let script = IdScript::of(&verbatim);
        Some((
            IdMatch {
                verbatim,
                id,
                prefix,
                script,
            },
            original,
        ))
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
