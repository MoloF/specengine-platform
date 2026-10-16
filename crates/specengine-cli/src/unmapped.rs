//! `spec unmapped [PATH]` (task spec `symbol-reads`, "spec unmapped"):
//! how much of the code the spec maps, per directory and code file. Read
//! from the code index the refresh just brought up to date; read-only
//! but for the data directory.
//!
//! - **Units**: every stored item of a Rust file, each RON file.
//! - **Mapped**: an item a marker reaches — bound to it, to an item or
//!   inline mod enclosing it, or to its file's module ([`reached`], the
//!   one containment) — whose ID is resolved, an alias or ambiguous (each
//!   holder): a dangling, look-alike, queue or other-project ID maps
//!   nothing at any depth. A RON file is mapped when such a marker anchors
//!   to one of its values. Never by qpath, never by a path's words.
//! - **Mentioned**: a unit a mention reaches alike, whatever its ID (a
//!   file mention: every unit of its file); a mention never maps.
//! - **Rows**: no PATH, a row per `[code]` root; a directory, a row per
//!   direct child directory (summed, `/`) and code file; then the total.
//!   A file PATH: its row, then its unmapped items in source order. By
//!   the printed path's bytes. A percent is the mapped per mille, rounded
//!   half up, one decimal; `-` with no unit. Rows and items are printed
//!   while the text stays within [`OUTPUT_CAP_CHARS`], then a
//!   `[truncated: …]` line; the JSON holds exactly the printed ones.
//! - **PATH**: a file or a directory as the stored code files say, a code
//!   file the walk could not read refused (exit 2); a root that is such a
//!   file is a file's row without a unit. The markers and the symbols
//!   come from one read transaction (one snapshot).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_core::is_clean_relative;
use specengine_store::{CodeFileState, CodeRefs, CodeSnapshot, SnapshotSymbols, StoredSymbol};

use crate::cap::OUTPUT_CAP_CHARS;
use crate::code_index::{LANG_RUST, refresh_code};
use crate::corpus::indexed_in;
use crate::location::open_index;
use crate::project::discover_with_text;
use crate::refs::{CodeCount, Corpus, code_config, stored_count};
use crate::search::notes;
use crate::show::unclean_path;
use crate::symbols::{Place, is_read, place};
use crate::{CliError, Env, Globals, Message, one_line, store_error};

/// `spec unmapped` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnmappedRequest {
    /// PATH as given: a `[code]` root, or a directory or a code file under
    /// one; `None`: a row per root.
    pub path: Option<String>,
}

/// What `spec unmapped` counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmappedOutcome {
    /// PATH, one trailing `/` dropped.
    pub path: Option<String>,
    pub messages: Vec<Message>,
    /// By printed path; a file PATH: its one row. The cap is applied when
    /// rendering.
    pub rows: Vec<UnmappedRow>,
    /// The sums over PATH (every root without one); a file PATH: its row.
    pub total: UnmappedRow,
    /// PATH is a code file: no total line.
    pub file: bool,
    /// A file PATH's unmapped items, in source order.
    pub unmapped: Vec<UnmappedItem>,
    pub code: CodeCount,
}

/// One row: a directory's sums or a code file's counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmappedRow {
    /// Root-relative, no trailing `/`; `None` for the total of every root.
    pub path: Option<String>,
    /// A directory (or the total of one), not a code file.
    pub dir: bool,
    pub units: usize,
    pub mapped: usize,
    pub mentioned: usize,
}

impl UnmappedRow {
    /// The mapped units per mille, rounded half up; `None` with no unit.
    #[must_use]
    pub fn per_mille(&self) -> Option<usize> {
        (self.units > 0).then(|| (self.mapped * 2000 + self.units) / (self.units * 2))
    }

    fn add(&mut self, counts: &Counts) {
        self.units += counts.units;
        self.mapped += counts.mapped;
        self.mentioned += counts.mentioned;
    }
}

/// One unmapped item of a file PATH.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnmappedItem {
    pub qpath: String,
    pub item_kind: String,
    pub path: String,
    /// Its first line (attributes excluded).
    pub line: usize,
}

/// `spec unmapped`: reads the config and PATH, refreshes both indexes and
/// counts from the code index.
pub fn unmapped(
    env: &Env,
    globals: &Globals,
    request: &UnmappedRequest,
) -> Result<UnmappedOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let code = code_config(&project, &text)?;
    let scheme = &project.config.scheme;
    let asked = request
        .path
        .as_deref()
        .map(|path| path.strip_suffix('/').unwrap_or(path));
    if let Some(path) = asked
        && !is_clean_relative(path)
    {
        return Err(unclean_path(&one_line(path)));
    }

    let mut messages = Vec::new();
    // One database handle for both refreshes.
    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, false)?;
    let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
    let refreshed = refresh_code(&mut open.index, &project.root, &code, scheme, false)?;
    refreshed.require_complete()?;
    messages.extend(refreshed.messages.iter().cloned());
    // One snapshot: the markers and every symbol from one read
    // transaction, whatever another connection writes meanwhile.
    let CodeSnapshot {
        refs: stored,
        symbols,
        ..
    } = open
        .index
        .code_snapshot(SnapshotSymbols::All)
        .map_err(store_error)?
        .unwrap_or_default();
    let walked: Vec<&CodeFileState> = stored.files.iter().filter(|file| is_read(file)).collect();
    let scope = Scope::of(asked, &code.roots, &stored.files, &walked)?;
    let coverage = Coverage::new(&stored, &symbols, &walked, &mut corpus)?;

    let mut outcome = UnmappedOutcome {
        path: asked.map(str::to_owned),
        messages,
        rows: Vec::new(),
        total: UnmappedRow {
            path: asked.map(str::to_owned),
            dir: true,
            units: 0,
            mapped: 0,
            mentioned: 0,
        },
        file: false,
        unmapped: Vec::new(),
        code: stored_count(&stored, refreshed.report.skipped),
    };
    match scope {
        Scope::File(path) => {
            let counts = coverage.of(path);
            let row = UnmappedRow {
                path: Some(path.to_owned()),
                dir: false,
                units: counts.units,
                mapped: counts.mapped,
                mentioned: counts.mentioned,
            };
            outcome.rows = vec![row.clone()];
            outcome.total = row;
            outcome.file = true;
            outcome.unmapped = counts
                .unmapped
                .iter()
                .map(|symbol| UnmappedItem {
                    qpath: symbol.qpath.clone(),
                    item_kind: symbol.item_kind.clone(),
                    path: symbol.path.clone(),
                    line: symbol.start_line as usize,
                })
                .collect();
        }
        Scope::Dir(dir) => {
            let prefix = format!("{dir}/");
            let mut rows: BTreeMap<String, UnmappedRow> = BTreeMap::new();
            for file in walked.iter().filter(|file| file.path.starts_with(&prefix)) {
                let counts = coverage.of(&file.path);
                outcome.total.add(counts);
                let rest = &file.path[prefix.len()..];
                let (path, dir) = match rest.split_once('/') {
                    Some((child, _)) => (format!("{prefix}{child}"), true),
                    None => (file.path.clone(), false),
                };
                row_of(&mut rows, path, dir).add(counts);
            }
            outcome.rows = rows.into_values().collect();
        }
        Scope::Roots => {
            let mut rows: BTreeMap<String, UnmappedRow> = BTreeMap::new();
            for root in &code.roots {
                let prefix = format!("{root}/");
                // A root that is a code file, read or not, is a file's row
                // (an unread one: no unit).
                let file = stored_file(&stored.files, root).is_some();
                let row = row_of(&mut rows, root.clone(), !file);
                for file in walked
                    .iter()
                    .filter(|file| file.path == *root || file.path.starts_with(&prefix))
                {
                    row.add(coverage.of(&file.path));
                }
            }
            // The total counts a file under two roots once.
            for file in &walked {
                outcome.total.add(coverage.of(&file.path));
            }
            outcome.rows = rows.into_values().collect();
        }
    }
    Ok(outcome)
}

/// The row printed as `path` (`/` after a directory's) in `rows`, new
/// rows empty.
fn row_of(rows: &mut BTreeMap<String, UnmappedRow>, path: String, dir: bool) -> &mut UnmappedRow {
    let label = if dir {
        format!("{path}/")
    } else {
        path.clone()
    };
    rows.entry(label).or_insert(UnmappedRow {
        path: Some(path),
        dir,
        units: 0,
        mapped: 0,
        mentioned: 0,
    })
}

/// What PATH names.
enum Scope<'a> {
    /// A code file the walk reads.
    File(&'a str),
    /// A root, or a directory under one holding a code file the walk
    /// reads.
    Dir(&'a str),
    /// No PATH: every root.
    Roots,
}

impl<'a> Scope<'a> {
    /// What `asked` names: a file or a directory as the stored files say
    /// (`files`, every code file of the walk, by path), never by the read
    /// ones alone; a code file the walk could not read is refused.
    fn of(
        asked: Option<&'a str>,
        roots: &[String],
        files: &[CodeFileState],
        walked: &[&CodeFileState],
    ) -> Result<Self, CliError> {
        let Some(path) = asked else {
            return Ok(Self::Roots);
        };
        let refused = || {
            CliError::spec(format!(
                "`{}` is no `[code]` root, nor a directory or code file the walk reads under one",
                one_line(path)
            ))
        };
        let under_root = roots
            .iter()
            .any(|root| path == root || is_below(path, root));
        if !under_root {
            return Err(refused());
        }
        if let Some(file) = stored_file(files, path) {
            return if is_read(file) {
                Ok(Self::File(path))
            } else {
                Err(refused())
            };
        }
        let root = roots.iter().any(|root| root == path);
        if root || walked.iter().any(|file| is_below(&file.path, path)) {
            return Ok(Self::Dir(path));
        }
        Err(refused())
    }
}

/// The stored code file at `path` (`files` by path), read or not.
fn stored_file<'s>(files: &'s [CodeFileState], path: &str) -> Option<&'s CodeFileState> {
    files
        .binary_search_by(|file| file.path.as_str().cmp(path))
        .ok()
        .and_then(|at| files.get(at))
}

/// `path` lies below the directory `dir`.
fn is_below(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// A file's units, mapped and mentioned, and its unmapped items.
#[derive(Debug, Clone, Default)]
struct Counts<'s> {
    units: usize,
    mapped: usize,
    mentioned: usize,
    unmapped: Vec<&'s StoredSymbol>,
}

/// The counts of every file the walk reads.
struct Coverage<'s> {
    files: BTreeMap<&'s str, Counts<'s>>,
    /// A file not read: nothing.
    none: Counts<'s>,
}

impl<'s> Coverage<'s> {
    fn new(
        stored: &'s CodeRefs,
        symbols: &'s [StoredSymbol],
        walked: &[&'s CodeFileState],
        corpus: &mut Corpus<'_>,
    ) -> Result<Self, CliError> {
        // Per file, what its mapping markers and its mentions reach.
        let mut mapping: BTreeMap<&str, Seeds> = BTreeMap::new();
        for marker in &stored.markers {
            let seed = place(&marker.binding)?;
            if corpus.judge(&marker.id).state.names_nodes() {
                mapping.entry(marker.path.as_str()).or_default().add(seed);
            }
        }
        let mut mentioned: BTreeMap<&str, Seeds> = BTreeMap::new();
        for mention in &stored.mentions {
            let seed = place(&mention.binding)?;
            mentioned
                .entry(mention.path.as_str())
                .or_default()
                .add(seed);
        }
        let mut items: BTreeMap<&str, Vec<&StoredSymbol>> = BTreeMap::new();
        for symbol in symbols {
            items.entry(symbol.path.as_str()).or_default().push(symbol);
        }

        let none = Seeds::default();
        let mut files = BTreeMap::new();
        for file in walked {
            let path = file.path.as_str();
            let mapping = mapping.get(path).unwrap_or(&none);
            let mentioned = mentioned.get(path).unwrap_or(&none);
            let counts = if file.lang == LANG_RUST {
                let items = items.get(path).map_or(&[][..], Vec::as_slice);
                let mapped = reached(items, mapping);
                let reached_by_mention = reached(items, mentioned);
                Counts {
                    units: items.len(),
                    mapped: mapped.iter().filter(|&&mapped| mapped).count(),
                    mentioned: reached_by_mention.iter().filter(|&&hit| hit).count(),
                    unmapped: items
                        .iter()
                        .zip(&mapped)
                        .filter(|(_, mapped)| !**mapped)
                        .map(|(symbol, _)| *symbol)
                        .collect(),
                }
            } else {
                Counts {
                    units: 1,
                    mapped: usize::from(mapping.ron),
                    mentioned: usize::from(mentioned.ron || mentioned.whole),
                    unmapped: Vec::new(),
                }
            };
            files.insert(path, counts);
        }
        Ok(Self {
            files,
            none: Counts::default(),
        })
    }

    /// The counts of the file at `path` (nothing: not read).
    fn of(&self, path: &str) -> &Counts<'s> {
        self.files.get(path).unwrap_or(&self.none)
    }
}

/// What the markers or the mentions of one file are bound to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Seeds {
    /// The file's module or the file itself: every item.
    pub(crate) whole: bool,
    /// Items, by `ord`.
    pub(crate) items: BTreeSet<u32>,
    /// A RON value of the file.
    pub(crate) ron: bool,
}

impl Seeds {
    pub(crate) fn add(&mut self, place: Place<'_>) {
        match place {
            Place::Item(ord) => {
                self.items.insert(ord);
            }
            Place::FileModule | Place::File => self.whole = true,
            Place::Ron(_) => self.ron = true,
            Place::Nothing => {}
        }
    }
}

/// The one containment (7b's `impact` reads it too): per item of one file
/// (`items`, by `ord`: pre-order, a parent before what it holds), whether
/// `seeds` reach it — bound to it, to an item enclosing it (its
/// `parent_ord` chain: an impl's methods, an inline mod's items at any
/// depth), or to the whole file. Structural only: never by qpath.
pub(crate) fn reached(items: &[&StoredSymbol], seeds: &Seeds) -> Vec<bool> {
    let at: BTreeMap<u32, usize> = items
        .iter()
        .enumerate()
        .map(|(index, symbol)| (symbol.ord, index))
        .collect();
    let mut reached: Vec<bool> = Vec::with_capacity(items.len());
    for (index, symbol) in items.iter().enumerate() {
        let by_parent = symbol
            .parent_ord
            .and_then(|parent| at.get(&parent).copied())
            .filter(|&parent| parent < index)
            .is_some_and(|parent| reached[parent]);
        reached.push(seeds.whole || seeds.items.contains(&symbol.ord) || by_parent);
    }
    reached
}

// ------------------------------------------------------------- rendering

/// A row's line: `<label> | units U | mapped M | <P %|-> | mentioned N`.
fn row_line(row: &UnmappedRow, label: &str) -> String {
    let percent = row.per_mille().map_or_else(
        || "-".to_owned(),
        |per_mille| format!("{}.{} %", per_mille / 10, per_mille % 10),
    );
    format!(
        "{} | units {} | mapped {} | {percent} | mentioned {}",
        one_line(label),
        row.units,
        row.mapped,
        row.mentioned
    )
}

/// A child row's label: its path, `/` after a directory's.
fn child_label(row: &UnmappedRow) -> String {
    let path = row.path.as_deref().unwrap_or_default();
    if row.dir {
        format!("{path}/")
    } else {
        path.to_owned()
    }
}

/// The total's label: `(total)`, after PATH when one is given.
fn total_label(outcome: &UnmappedOutcome) -> String {
    match &outcome.path {
        Some(path) => format!("{path} (total)"),
        None => "(total)".to_owned(),
    }
}

/// An unmapped item's line: `  <qpath> | <item_kind> | <line>`.
fn item_line(item: &UnmappedItem) -> String {
    format!(
        "  {} | {} | {}",
        one_line(&item.qpath),
        one_line(&item.item_kind),
        item.line
    )
}

/// How many rows and unmapped items the text shows: while the text
/// before the tail stays within [`OUTPUT_CAP_CHARS`] characters, the total
/// line kept (a file PATH: its row, then items).
fn shown(outcome: &UnmappedOutcome) -> (usize, usize) {
    let size = |line: String| line.chars().count() + 1;
    let mut used = if outcome.file {
        0
    } else {
        size(row_line(&outcome.total, &total_label(outcome)))
    };
    let mut rows = 0;
    for (index, row) in outcome.rows.iter().enumerate() {
        let line = size(row_line(row, &child_label(row)));
        if index > 0 && used + line > OUTPUT_CAP_CHARS {
            break;
        }
        used += line;
        rows += 1;
    }
    let mut items = 0;
    if rows == outcome.rows.len() {
        for item in &outcome.unmapped {
            let line = size(item_line(item));
            if used + line > OUTPUT_CAP_CHARS {
                break;
            }
            used += line;
            items += 1;
        }
    }
    (rows, items)
}

/// The text: the rows, the total (but for a file PATH), the unmapped items
/// within the cap, the tail when cut, the summary.
pub(crate) fn render_text(outcome: &UnmappedOutcome) -> String {
    let (rows, items) = shown(outcome);
    let mut out = String::new();
    for row in &outcome.rows[..rows] {
        out.push_str(&row_line(row, &child_label(row)));
        out.push('\n');
    }
    if !outcome.file {
        out.push_str(&row_line(&outcome.total, &total_label(outcome)));
        out.push('\n');
    }
    for item in &outcome.unmapped[..items] {
        out.push_str(&item_line(item));
        out.push('\n');
    }
    let cut: Vec<String> = [
        (outcome.rows.len() - rows, "rows"),
        (outcome.unmapped.len() - items, "unmapped items"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, what)| format!("{count} {what}"))
    .collect();
    if !cut.is_empty() {
        out.push_str(&format!("[truncated: {} not shown]\n", cut.join(", ")));
    }
    let code = outcome.code;
    out.push_str(&format!(
        "unmapped: code files {} (.rs {}, .ron {}), skipped {}\n",
        code.files, code.rs, code.ron, code.skipped
    ));
    out
}

/// A row as JSON: `percent` a number (the text's), `null` for `-`.
#[derive(Serialize)]
struct RowJson<'a> {
    path: Option<&'a str>,
    dir: bool,
    units: usize,
    mapped: usize,
    percent: Option<f64>,
    mentioned: usize,
}

impl<'a> From<&'a UnmappedRow> for RowJson<'a> {
    fn from(row: &'a UnmappedRow) -> Self {
        Self {
            path: row.path.as_deref(),
            dir: row.dir,
            units: row.units,
            mapped: row.mapped,
            percent: row.per_mille().map(|per_mille| per_mille as f64 / 10.0),
            mentioned: row.mentioned,
        }
    }
}

/// The rows and items the cap left out.
#[derive(Serialize)]
struct Omitted {
    rows: usize,
    unmapped: usize,
}

/// The JSON document: the text's rows and items, every key present.
#[derive(Serialize)]
struct UnmappedJson<'a> {
    path: Option<&'a str>,
    rows: Vec<RowJson<'a>>,
    total: RowJson<'a>,
    unmapped: &'a [UnmappedItem],
    notes: Vec<String>,
    omitted: Omitted,
    code: CodeCount,
}

impl Serialize for UnmappedOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (rows, items) = shown(self);
        UnmappedJson {
            path: self.path.as_deref(),
            rows: self.rows[..rows].iter().map(RowJson::from).collect(),
            total: RowJson::from(&self.total),
            unmapped: &self.unmapped[..items],
            notes: notes(&self.messages),
            omitted: Omitted {
                rows: self.rows.len() - rows,
                unmapped: self.unmapped.len() - items,
            },
            code: self.code,
        }
        .serialize(serializer)
    }
}
