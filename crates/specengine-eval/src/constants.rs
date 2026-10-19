//! Measurement `constants` (docs/features/constant-census.md, slice 12a):
//! which free string constants of a project's code carry a code of its
//! records. Read-only: it binds, locks and writes nothing under the corpus.
//!
//! - Files (D1): each census `[code] roots` entry, a directory listed by
//!   [`harness::rust_files`]' rules or a `.rs` file root kept as is, minus a
//!   path a `[code] exclude` glob matches; `extensions` and `strip` unused.
//!   A file under two roots is read once; a root that cannot be read is
//!   counted, never fatal.
//! - Items and markers (D6): one parse per file, `bind_tree`'s items of
//!   kind `const_item` or `static_item`, each read by
//!   [`specengine_code::const_value`]; the markers of the comments bound to
//!   the item, any relation, IDs verbatim.
//! - Records (D7): the import's definitions (`Role::Definition`), by `id`
//!   and by alias, plus `Import.duplicates`; its hyphenless patterns.
//! - Match: the whole value byte for byte; two definitions read
//!   `two_records`, a would-be match two valued items hold `two_items`,
//!   then `id`, `alias`, `hyphenless` (a pattern's leftmost match spanning
//!   the whole value), `unmatched`.
//!
//! The config comes from `--config` as for `census`; a config with no
//! `[code] roots` is refused. stdout carries counts only; every row goes to
//! `--out/constants/<label>/`. Nothing here knows a kind, a prefix or a
//! template: the convention is the config's (ADR-0008).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};
use specengine_code::{
    Binding, ConstValue, ItemRecord, NotValued, RustParser, bind_tree, const_value, markers_in,
};
use specengine_import::CensusConfig;
use specengine_import::config::{CodeConfig, Pattern};
use specengine_import::import::{self, Import, Role};

use crate::census::{self, write_json};
use crate::harness::{self, Corpus};

/// The measurement's name: the envelope's and the detail directory's.
pub const MEASUREMENT: &str = "constants";

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "code-consts/one";

/// One row per item, by path bytes, line and start byte.
const ROWS_FILE: &str = "constants.json";

/// One row per skipped file and per root that cannot be read.
const SKIPPED_FILE: &str = "skipped.json";

/// The file name ending of a Rust source, as [`harness::rust_files`] lists.
const RUST_SUFFIX: &str = ".rs";

/// The `result` object: counts only, every key present, zeros too.
#[derive(Serialize)]
pub struct ConstantsResult {
    /// Files listed, the skipped ones included.
    pub files: usize,
    pub skipped: usize,
    pub roots_missing: usize,
    /// `const` and `static` items read.
    pub items: usize,
    pub valued: usize,
    pub not_valued: NotValuedCounts,
    pub matched: Matched,
    pub hyphenless: usize,
    pub unmatched: usize,
    pub ambiguous: Ambiguous,
    /// Valued items with a bound marker.
    pub marked: usize,
    /// First file listed to last row matched, the import excluded.
    pub constants_ms: u128,
}

/// The not-valued items per category, every category of
/// [`NotValued::ALL`] present in that order.
pub struct NotValuedCounts(BTreeMap<NotValued, usize>);

impl Serialize for NotValuedCounts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(NotValued::ALL.len()))?;
        for category in NotValued::ALL {
            map.serialize_entry(
                category.as_str(),
                &self.0.get(&category).copied().unwrap_or(0),
            )?;
        }
        map.end()
    }
}

#[derive(Serialize)]
pub struct Matched {
    pub id: usize,
    pub alias: usize,
}

#[derive(Serialize)]
pub struct Ambiguous {
    pub two_records: usize,
    pub two_items: usize,
}

/// How a valued item's value matches (spec "Match", first that applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
enum Match {
    Id,
    Alias,
    Hyphenless,
    Unmatched,
    TwoRecords,
    TwoItems,
}

/// Why a listed file or a configured root was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Cause {
    NotUtf8,
    Unreadable,
    Unparsed,
    RootMissing,
}

/// One row of `constants.json`.
#[derive(Serialize)]
struct Row {
    /// Corpus-relative, `/`-separated.
    path: String,
    line: usize,
    name: Option<String>,
    mod_path: Vec<String>,
    cfg: Vec<String>,
    item_kind: String,
    value: Option<String>,
    not_valued: Option<&'static str>,
    r#match: Option<Match>,
    /// The sorted distinct IDs the value names (an alias: its record's ID).
    records: Vec<String>,
    /// The bound markers' IDs, in source order.
    marked: Vec<String>,
    /// The item's first byte: the last sort key.
    #[serde(skip)]
    start: usize,
    /// The category `not_valued` names.
    #[serde(skip)]
    category: Option<NotValued>,
}

/// One row of `skipped.json`.
#[derive(Serialize)]
struct Skipped {
    path: String,
    cause: Cause,
}

/// Reads the census config before anything is written (as `census`); a
/// config with no `[code] roots` is refused too, naming the file.
pub fn prepare(
    root: &Path,
    config: Option<&Path>,
    label: Option<&str>,
) -> Result<CensusConfig, String> {
    let path = harness::resolve_file(
        "--config",
        config,
        harness::PILOT_CENSUS_CONFIG,
        label,
        root,
        census::DEFAULT_CONFIG,
    )?;
    let loaded = CensusConfig::load(&path).map_err(|error| error.to_string())?;
    if loaded.import.code.roots.is_empty() {
        return Err(format!("{}: [code] roots is empty", path.display()));
    }
    Ok(loaded)
}

pub fn run(corpus: &Corpus, config: CensusConfig) -> Result<ConstantsResult, String> {
    let out_dir = corpus.out.join(MEASUREMENT).join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;

    let imported = import::run(&corpus.root, &config)?;
    let records = Records::new(&imported, &config.import.hyphenless);

    let started = Instant::now();
    let listing = list_files(&corpus.root, &config.import.code);
    let mut parser =
        RustParser::new().map_err(|error| format!("the Rust grammar cannot be loaded: {error}"))?;
    let mut skipped: Vec<Skipped> = listing
        .missing
        .iter()
        .map(|path| Skipped {
            path: path.clone(),
            cause: Cause::RootMissing,
        })
        .collect();
    let mut skipped_files = 0;
    let mut rows = Vec::new();
    for (path, absolute) in &listing.files {
        match read_file(&mut parser, path, absolute) {
            Ok(read) => rows.extend(read),
            Err(cause) => {
                skipped_files += 1;
                skipped.push(Skipped {
                    path: path.clone(),
                    cause,
                });
            }
        }
    }
    rows.sort_by(|a, b| {
        (a.path.as_bytes(), a.line, a.start).cmp(&(b.path.as_bytes(), b.line, b.start))
    });
    skipped.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    match_rows(&mut rows, &records);

    let mut not_valued = BTreeMap::new();
    let mut matches: HashMap<Match, usize> = HashMap::new();
    let mut marked = 0;
    for row in &rows {
        match (row.category, row.r#match) {
            (Some(category), _) => *not_valued.entry(category).or_insert(0) += 1,
            (None, Some(found)) => {
                *matches.entry(found).or_insert(0) += 1;
                if !row.marked.is_empty() {
                    marked += 1;
                }
            }
            (None, None) => {}
        }
    }
    let constants_ms = started.elapsed().as_millis();
    let count = |found: Match| matches.get(&found).copied().unwrap_or(0);
    let result = ConstantsResult {
        files: listing.files.len(),
        skipped: skipped_files,
        roots_missing: listing.missing.len(),
        items: rows.len(),
        valued: rows.iter().filter(|row| row.value.is_some()).count(),
        not_valued: NotValuedCounts(not_valued),
        matched: Matched {
            id: count(Match::Id),
            alias: count(Match::Alias),
        },
        hyphenless: count(Match::Hyphenless),
        unmatched: count(Match::Unmatched),
        ambiguous: Ambiguous {
            two_records: count(Match::TwoRecords),
            two_items: count(Match::TwoItems),
        },
        marked,
        constants_ms,
    };

    write_json(&out_dir.join(ROWS_FILE), &rows)?;
    write_json(&out_dir.join(SKIPPED_FILE), &skipped)?;
    summarize(&result, &corpus.label, &out_dir);
    Ok(result)
}

/// The files the census reads, and the roots it cannot.
struct Listing {
    /// Corpus-relative `/`-separated path -> the path to read.
    files: BTreeMap<String, PathBuf>,
    /// The configured roots that cannot be read, as configured.
    missing: BTreeSet<String>,
}

/// D1: every `[code] roots` entry, a directory by [`harness::rust_files`]'
/// rules or a `.rs` file root as is, minus a path an `exclude` glob
/// matches; each path once, whatever the roots' order.
fn list_files(root: &Path, code: &CodeConfig) -> Listing {
    let mut listing = Listing {
        files: BTreeMap::new(),
        missing: BTreeSet::new(),
    };
    let kept = |path: &str| !code.exclude.iter().any(|glob| glob.is_match(path));
    for configured in &code.roots {
        let relative = slash_path(configured);
        let absolute = root.join(configured);
        let metadata = match fs::metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("{MEASUREMENT}: code root {relative} cannot be read: {error}");
                listing.missing.insert(relative);
                continue;
            }
        };
        if !metadata.is_dir() {
            let is_rust = relative
                .rsplit('/')
                .next()
                .is_some_and(|name| name.ends_with(RUST_SUFFIX));
            if is_rust && kept(&relative) {
                listing.files.entry(relative).or_insert(absolute);
            }
            continue;
        }
        let found = match harness::rust_files(&absolute) {
            Ok(found) => found,
            Err(error) => {
                eprintln!("{MEASUREMENT}: code root {relative} cannot be read: {error}");
                listing.missing.insert(relative);
                continue;
            }
        };
        for file in found {
            let below = slash_path(&file);
            let path = if relative.is_empty() {
                below
            } else {
                format!("{relative}/{below}")
            };
            if kept(&path) {
                listing
                    .files
                    .entry(path)
                    .or_insert_with(|| absolute.join(&file));
            }
        }
    }
    listing
}

/// `/`-separated relative path; `.` components dropped.
fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The `const` and `static` items of one file, read and unmatched; or why
/// the file was skipped.
fn read_file(parser: &mut RustParser, path: &str, absolute: &Path) -> Result<Vec<Row>, Cause> {
    let bytes = fs::read(absolute).map_err(|error| {
        eprintln!("{MEASUREMENT}: {path}: unreadable: {error}");
        Cause::Unreadable
    })?;
    let text = String::from_utf8(bytes).map_err(|_| Cause::NotUtf8)?;
    let tree = parser.parse(&text).ok_or(Cause::Unparsed)?;
    let binding = bind_tree(&tree, &text);
    let mut marked: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for comment in &binding.comments {
        let Binding::Item(index) = comment.binding else {
            continue;
        };
        let Some(body) = text.get(comment.range.clone()) else {
            continue;
        };
        marked
            .entry(index)
            .or_default()
            .extend(markers_in(body).into_iter().map(|marker| marker.id));
    }
    let mut rows = Vec::new();
    for (index, item) in binding.analysis.items.iter().enumerate() {
        let Some(read) = const_value(&tree, &text, item) else {
            continue;
        };
        rows.push(row(
            path,
            item,
            read,
            marked.remove(&index).unwrap_or_default(),
        ));
    }
    Ok(rows)
}

fn row(path: &str, item: &ItemRecord, read: ConstValue, marked: Vec<String>) -> Row {
    let (value, category) = match read {
        ConstValue::Valued(text) => (Some(text), None),
        ConstValue::NotValued(category) => (None, Some(category)),
    };
    Row {
        path: path.to_owned(),
        line: item.line,
        name: item.name.clone(),
        mod_path: item.mod_path.clone(),
        cfg: item.cfg.clone(),
        item_kind: item.kind.clone(),
        value,
        not_valued: category.map(NotValued::as_str),
        r#match: None,
        records: Vec::new(),
        marked,
        start: item.byte_range.start,
        category,
    }
}

/// A definition of a record: where it stands and the ID it defines.
type Definition<'a> = (&'a str, usize, &'a str);

/// The import's definitions, keyed by what a value is compared with.
struct Records<'a> {
    by_id: HashMap<&'a str, Vec<Definition<'a>>>,
    by_alias: HashMap<&'a str, Vec<Definition<'a>>>,
    /// `Import.duplicates`: every further definition of an ID.
    duplicates: HashMap<&'a str, Vec<Definition<'a>>>,
    hyphenless: &'a [Pattern],
}

impl<'a> Records<'a> {
    /// Definitions only: a reference never adds a record (reading F1).
    fn new(imported: &'a Import, hyphenless: &'a [Pattern]) -> Self {
        let mut records = Self {
            by_id: HashMap::new(),
            by_alias: HashMap::new(),
            duplicates: HashMap::new(),
            hyphenless,
        };
        for record in &imported.records {
            if record.role != Role::Definition {
                continue;
            }
            let definition = (record.path.as_str(), record.line, record.id.as_str());
            records
                .by_id
                .entry(record.id.as_str())
                .or_default()
                .push(definition);
            for alias in &record.aliases {
                records
                    .by_alias
                    .entry(alias.as_str())
                    .or_default()
                    .push(definition);
            }
        }
        for duplicate in &imported.duplicates {
            records
                .duplicates
                .entry(duplicate.id.as_str())
                .or_default()
                .push((
                    duplicate.path.as_str(),
                    duplicate.line,
                    duplicate.id.as_str(),
                ));
        }
        records
    }

    /// Rules 1 and 3-6 for one value, and the sorted distinct IDs it names.
    fn classify(&self, value: &str) -> (Match, Vec<String>) {
        let by_id = self.by_id.get(value).map_or(&[][..], Vec::as_slice);
        let by_alias = self.by_alias.get(value).map_or(&[][..], Vec::as_slice);
        let mut definitions: BTreeSet<Definition> = by_id.iter().chain(by_alias).copied().collect();
        let named: BTreeSet<&str> = definitions.iter().map(|definition| definition.2).collect();
        for id in &named {
            if let Some(further) = self.duplicates.get(id) {
                definitions.extend(further.iter().copied());
            }
        }
        let records = named.iter().map(|id| (*id).to_owned()).collect();
        let found = if definitions.len() >= 2 {
            Match::TwoRecords
        } else if !by_id.is_empty() {
            Match::Id
        } else if !by_alias.is_empty() {
            Match::Alias
        } else if self.is_hyphenless(value) {
            Match::Hyphenless
        } else {
            Match::Unmatched
        };
        (found, records)
    }

    /// A pattern's leftmost match (`Regex::find`) spans the whole value:
    /// the importer's whole-token rule, restated (its module is private).
    fn is_hyphenless(&self, value: &str) -> bool {
        self.hyphenless.iter().any(|pattern| {
            pattern
                .regex
                .find(value)
                .is_some_and(|found| found.start() == 0 && found.end() == value.len())
        })
    }
}

/// Matches every valued row: rule 2 (`two_items`) over the values all
/// valued rows hold, so no order decides.
fn match_rows(rows: &mut [Row], records: &Records) {
    let mut holders: HashMap<String, usize> = HashMap::new();
    for row in rows.iter() {
        if let Some(value) = &row.value {
            *holders.entry(value.clone()).or_insert(0) += 1;
        }
    }
    for row in rows.iter_mut() {
        let Some(value) = &row.value else {
            continue;
        };
        let (mut found, named) = records.classify(value);
        let shared = holders.get(value).copied().unwrap_or(0) > 1;
        if shared && matches!(found, Match::Id | Match::Alias | Match::Hyphenless) {
            found = Match::TwoItems;
        }
        row.r#match = Some(found);
        row.records = named;
    }
}

fn summarize(result: &ConstantsResult, label: &str, out_dir: &Path) {
    eprintln!(
        "{MEASUREMENT} [{label}]: {} files ({} skipped), {} roots missing, {} items, {} valued",
        result.files, result.skipped, result.roots_missing, result.items, result.valued
    );
    let not_valued: Vec<String> = NotValued::ALL
        .iter()
        .map(|category| {
            format!(
                "{} {}",
                category.as_str(),
                result.not_valued.0.get(category).copied().unwrap_or(0)
            )
        })
        .collect();
    eprintln!("  not valued: {}", not_valued.join(", "));
    eprintln!(
        "  matched by ID {}, by alias {}; hyphenless {}; unmatched {}; ambiguous: two records {}, two items {}",
        result.matched.id,
        result.matched.alias,
        result.hyphenless,
        result.unmatched,
        result.ambiguous.two_records,
        result.ambiguous.two_items
    );
    eprintln!("  marked {} ({} ms)", result.marked, result.constants_ms);
    eprintln!("  detail: {}", out_dir.display());
}
