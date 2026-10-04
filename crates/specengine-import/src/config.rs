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
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};
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
    /// `[layout]`, defaults when absent; read by neither the census nor the
    /// import, only by [`crate::layout`].
    pub layout: LayoutConfig,
}

/// Default of `[layout] records`: core's `[paths] records` default
/// (`docs/canon/architecture.md` "Spec layout in a project").
pub const DEFAULT_LAYOUT_RECORDS: &str = "docs/records";
/// Default of `[layout] features`: core's `[paths] features` default
/// (`docs/canon/architecture.md` "Spec layout in a project").
pub const DEFAULT_LAYOUT_FEATURES: &str = "docs/features";
/// Default of `[layout] debt_expires`.
pub const DEFAULT_DEBT_EXPIRES: &str = "9999-12-31";

/// `[layout]`, validated (`crates/specengine-import/README.md` "Config"): where
/// the after-tree puts record files and feature documents, the classes it
/// gives, the task-box key, the baseline's expiry and the target per
/// prefix. Absent: every default.
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// `<records>/<PREFIX>/<ID>.md`; root-relative, `/`-separated.
    pub records: String,
    /// `<features>/<slug>.md`; root-relative, `/`-separated.
    pub features: String,
    /// Over the corpus-relative source path; group `slug`. `None`: the stem.
    pub slug: Option<Regex>,
    /// The `class` of every record file.
    pub record_class: DocClass,
    /// A document's class when its header gives none: the first match over
    /// its after path.
    pub classes: Vec<ClassRule>,
    /// The key carrying a list item's task box; `None`: boxes are dropped.
    pub task_box_key: Option<String>,
    /// `YYYY-MM-DD`: the `expires` of every baseline entry.
    pub debt_expires: String,
    /// The line of `debt_expires`, when written.
    pub debt_expires_line: Option<usize>,
    /// Per Latin prefix; absent: project scope `file`, feature `section`.
    pub targets: BTreeMap<String, TargetRule>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            records: DEFAULT_LAYOUT_RECORDS.to_owned(),
            features: DEFAULT_LAYOUT_FEATURES.to_owned(),
            slug: None,
            record_class: DocClass::default(),
            classes: Vec::new(),
            task_box_key: None,
            debt_expires: DEFAULT_DEBT_EXPIRES.to_owned(),
            debt_expires_line: None,
            targets: BTreeMap::new(),
        }
    }
}

impl LayoutConfig {
    /// Where a definition of `prefix` goes: its `targets` entry, else
    /// `section` for a feature-scoped prefix and `file` for the others.
    pub fn target(&self, prefix: &str, feature: bool) -> Target {
        match self.targets.get(prefix) {
            Some(rule) => rule.target,
            None if feature => Target::Section,
            None => Target::File,
        }
    }

    /// The first `classes` entry matching a document's after path.
    pub fn class_of(&self, after: &str) -> Option<DocClass> {
        self.classes
            .iter()
            .find(|rule| rule.glob.is_match(after))
            .map(|rule| rule.class)
    }

    /// The refusals that need the before scheme and the run's date
    /// (`crates/specengine-import/README.md` "Config", at start): a `targets`
    /// key the scheme does not configure, a feature-scoped one set to
    /// `file`, `debt_expires` before `today` (`YYYY-MM-DD`). `origin` names
    /// the config in the error. The index output path is checked by the
    /// caller against [`crate::layout::Layout::holds`].
    pub fn check_start(
        &self,
        origin: &Path,
        scheme: &crate::layout::Scheme,
        today: &str,
    ) -> Result<(), ConfigError> {
        let error = |line: Option<usize>, message: String| ConfigError {
            path: origin.to_path_buf(),
            line,
            message,
        };
        for (prefix, rule) in &self.targets {
            if !scheme.has_prefix(prefix) {
                return Err(error(
                    Some(rule.line),
                    format!(
                        "`layout.targets` names `{prefix}`, which the scheme's `[ids]` does not configure"
                    ),
                ));
            }
            if rule.target == Target::File && scheme.is_feature(prefix) {
                return Err(error(
                    Some(rule.line),
                    format!(
                        "`layout.targets.{prefix}` is `{}`, but the scheme scopes `{prefix}` to a feature: its IDs are sections of a feature document",
                        word(&Target::File)
                    ),
                ));
            }
        }
        if self.debt_expires.as_str() < today {
            return Err(error(
                self.debt_expires_line,
                format!(
                    "`layout.debt_expires` `{}` is before today, {today}",
                    self.debt_expires
                ),
            ));
        }
        Ok(())
    }
}

/// One `[layout] classes` entry, compiled.
#[derive(Debug, Clone)]
pub struct ClassRule {
    /// Over the after path, `/`-separated.
    pub glob: Regex,
    pub class: DocClass,
}

/// One `[layout] targets` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetRule {
    pub target: Target,
    /// The entry's line in the config.
    pub line: usize,
}

/// Where a prefix's definitions go (ADR-0026): one record file each, or a
/// `{#ID}` section of their document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    File,
    Section,
}

/// The four document classes of the target layout (`docs/canon/spec-check.md`).
/// The names are the format's vocabulary, spelled by serde from the variants:
/// no string literal of the engine names a class.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DocClass {
    #[default]
    Canon,
    Decision,
    Spec,
    Generated,
}

impl DocClass {
    /// The class as the target front-matter writes it.
    pub fn name(self) -> String {
        word(&self)
    }
}

/// The front-matter keys the after-tree writes itself (core's reader): a
/// task-box key may be none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoreKey {
    Id,
    Class,
    Title,
    Aliases,
}

impl CoreKey {
    pub fn name(self) -> String {
        word(&self)
    }

    /// The key `text` names, if it is one.
    pub fn of(text: &str) -> Option<Self> {
        let deserializer: serde::de::value::StrDeserializer<'_, serde::de::value::Error> =
            text.into_deserializer();
        Self::deserialize(deserializer).ok()
    }
}

/// The name serde gives a unit variant of the format's vocabulary
/// ([`Target`], [`DocClass`], [`CoreKey`], the scheme's scope): the target
/// format's words live in the variants, not in string literals, so that the
/// genre test's literal scan keeps reading only corpus conventions.
pub(crate) fn word<T: Serialize>(value: &T) -> String {
    match toml::Value::try_from(value) {
        Ok(toml::Value::String(name)) => name,
        _ => String::new(),
    }
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
    #[serde(default)]
    layout: RawLayout,
}

/// `[layout]` as written (`crates/specengine-import/README.md` "Config"); the
/// census and the import validate it and read none of it.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawLayout {
    #[serde(default)]
    records: Option<Spanned<String>>,
    #[serde(default)]
    features: Option<Spanned<String>>,
    #[serde(default)]
    slug: Option<Spanned<String>>,
    #[serde(default)]
    record_class: Option<Spanned<DocClass>>,
    #[serde(default)]
    classes: Vec<RawClassRule>,
    #[serde(default)]
    task_box_key: Option<Spanned<String>>,
    #[serde(default)]
    debt_expires: Option<Spanned<String>>,
    #[serde(default)]
    targets: BTreeMap<String, Spanned<Target>>,
}

/// One `[layout] classes` entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClassRule {
    glob: Spanned<String>,
    class: Spanned<DocClass>,
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
        let layout = compile_layout(&raw.layout, &error_at, &import)?;

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
            layout,
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

/// Compiles and validates `[layout]`
/// (`crates/specengine-import/README.md` "Config"); an error names the key's
/// line. The refusals that need the before scheme or the run's date are
/// [`LayoutConfig::check_start`].
fn compile_layout(
    raw: &RawLayout,
    error_at: &ErrorAt<'_>,
    import: &ImportConfig,
) -> Result<LayoutConfig, ConfigError> {
    let directory = |key: &str, value: &Option<Spanned<String>>, default: &str| match value {
        None => Ok((default.to_owned(), None)),
        Some(value) => root_relative(value.get_ref())
            .map(|path| (path, Some(value.span())))
            .map_err(|problem| error_at(Some(value.span()), format!("`layout.{key}`: {problem}"))),
    };
    let (records, records_span) = directory("records", &raw.records, DEFAULT_LAYOUT_RECORDS)?;
    let (features, features_span) = directory("features", &raw.features, DEFAULT_LAYOUT_FEATURES)?;
    let nested = |outer: &str, inner: &str| {
        inner
            .strip_prefix(outer)
            .is_some_and(|rest| rest.starts_with('/'))
    };
    if records == features || nested(&records, &features) || nested(&features, &records) {
        return Err(error_at(
            features_span.or(records_span),
            format!(
                "`layout.records` `{records}` and `layout.features` `{features}` are equal or nested"
            ),
        ));
    }

    let slug = match &raw.slug {
        None => None,
        Some(value) => {
            let compiled = Regex::new(value.get_ref())
                .map_err(|error| error_at(Some(value.span()), format!("`layout.slug`: {error}")))?;
            if compiled.is_match("") {
                return Err(error_at(
                    Some(value.span()),
                    "`layout.slug` matches the empty string".to_owned(),
                ));
            }
            if !compiled.capture_names().any(|name| name == Some("slug")) {
                return Err(error_at(
                    Some(value.span()),
                    "`layout.slug` has no group `slug`".to_owned(),
                ));
            }
            Some(compiled)
        }
    };

    let mut classes = Vec::with_capacity(raw.classes.len());
    for rule in &raw.classes {
        let source = rule.glob.get_ref();
        if source.is_empty() {
            return Err(error_at(
                Some(rule.glob.span()),
                "`layout.classes` holds an empty glob".to_owned(),
            ));
        }
        let glob = Regex::new(&glob_to_regex(source)).map_err(|error| {
            error_at(
                Some(rule.glob.span()),
                format!("`layout.classes` glob `{source}`: {error}"),
            )
        })?;
        classes.push(ClassRule {
            glob,
            class: *rule.class.get_ref(),
        });
    }

    let task_box_key = match &raw.task_box_key {
        None => None,
        Some(value) => {
            let key = value.get_ref();
            let problem = if key.is_empty() {
                Some("is empty".to_owned())
            } else if key.trim() != key {
                Some("has blanks around it".to_owned())
            } else {
                CoreKey::of(key).map(|_| format!("`{key}` is a key the layout writes itself"))
            };
            if let Some(problem) = problem {
                return Err(error_at(
                    Some(value.span()),
                    format!("`layout.task_box_key` {problem}"),
                ));
            }
            Some(key.clone())
        }
    };

    let (debt_expires, debt_expires_line) = match &raw.debt_expires {
        None => (DEFAULT_DEBT_EXPIRES.to_owned(), None),
        Some(value) => {
            if !is_date(value.get_ref()) {
                return Err(error_at(
                    Some(value.span()),
                    format!(
                        "`layout.debt_expires` `{}` is not a date `YYYY-MM-DD`",
                        value.get_ref()
                    ),
                ));
            }
            let line = error_at(Some(value.span()), String::new()).line;
            (value.get_ref().clone(), line)
        }
    };

    let mut targets = BTreeMap::new();
    for (prefix, target) in &raw.targets {
        if !is_latin_prefix(prefix) {
            return Err(error_at(
                Some(target.span()),
                format!(
                    "`layout.targets` key `{prefix}` is not a Latin prefix (an ASCII capital, then ASCII capitals or digits)"
                ),
            ));
        }
        let target_value = *target.get_ref();
        if target_value == Target::File && import.feature_prefixes.contains(prefix) {
            return Err(error_at(
                Some(target.span()),
                format!(
                    "`layout.targets.{prefix}` is `{}`, but `{prefix}` is one of `ids.feature_prefixes`: its IDs are sections of a feature document",
                    word(&Target::File)
                ),
            ));
        }
        let line = error_at(Some(target.span()), String::new())
            .line
            .unwrap_or(1);
        targets.insert(
            prefix.clone(),
            TargetRule {
                target: target_value,
                line,
            },
        );
    }

    Ok(LayoutConfig {
        records,
        features,
        slug,
        record_class: raw
            .record_class
            .as_ref()
            .map(|class| *class.get_ref())
            .unwrap_or_default(),
        classes,
        task_box_key,
        debt_expires,
        debt_expires_line,
        targets,
    })
}

/// A root-relative directory under core's `[paths]` rules: no leading `/`,
/// no `..`, no `.` or empty component; one trailing `/` dropped.
fn root_relative(value: &str) -> Result<String, String> {
    let trimmed = value.strip_suffix('/').unwrap_or(value);
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

/// `YYYY-MM-DD`, a date of the proleptic Gregorian calendar.
pub fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let number = |range: Range<usize>| {
        text.get(range)
            .and_then(|part| part.parse::<u32>().ok())
            .unwrap_or(0)
    };
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
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
