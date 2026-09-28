//! The census: a read-only dry-run counter over an existing corpus. It walks
//! the configured roots, reads each document's front-matter, ID'd table rows,
//! `{#ID}` sections and links, and hashes every record's verbatim text
//! (BLAKE3). The whole convention comes from [`CensusConfig`]; the census
//! itself knows no corpus (ADR-0008) and never writes.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path};

use serde::Serialize;

use crate::config::{CensusConfig, IdMatch};
use crate::frontmatter::{self, FrontMatter};
use crate::markdown::{self, Heading, LinkKind, Table};
use crate::script::IdScript;

/// Where a record comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecordKind {
    /// A row of a record table; the verbatim text is the row line.
    Row,
    /// A heading with `{#ID}`; the verbatim text runs to the next heading of
    /// the same or a higher level, trailing blank lines excluded.
    Section,
}

/// One hashed record: an entry of the hash manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    /// Corpus-relative, `/`-separated.
    pub path: String,
    /// 1-based line of the row or heading.
    pub line: usize,
    pub kind: RecordKind,
    /// The ID as written.
    pub verbatim_id: String,
    /// The ID after look-alike normalization.
    pub id: String,
    pub prefix: String,
    pub script: IdScript,
    /// BLAKE3 of the verbatim text, hex.
    pub blake3: String,
}

/// A position in the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub path: String,
    pub line: usize,
}

/// A link whose file does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrokenLink {
    pub path: String,
    pub line: usize,
    pub target: String,
}

/// A problem with one input, reported and skipped; never fatal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub line: Option<usize>,
    pub message: String,
}

/// How a document opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrontMatterState {
    Absent,
    Unclosed,
    Present,
}

/// Per-document detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentSummary {
    pub path: String,
    pub bytes: usize,
    pub front_matter: FrontMatterState,
    /// Value of the class key when the front-matter carries it.
    pub class: Option<String>,
    /// Pipe tables with a header row.
    pub tables: usize,
    /// Blocks of `|` rows without a header row.
    pub headerless_blocks: usize,
    pub record_tables: usize,
    pub id_rows: usize,
    pub rows_without_id: usize,
    pub id_sections: usize,
    pub links_checked: usize,
    pub broken_links: usize,
}

/// Everything the census found. Aggregates are methods; the vectors are the
/// per-record detail (paths and IDs) that only a scratch directory receives.
#[derive(Debug, Clone, Default)]
pub struct Census {
    /// Documents found, including the skipped ones.
    pub documents: usize,
    /// Documents that could not be read as UTF-8 text.
    pub files_skipped: usize,
    /// Configured roots that do not exist in the corpus.
    pub roots_missing: usize,
    pub bytes: usize,
    pub documents_detail: Vec<DocumentSummary>,
    /// Hash manifest: rows and sections in path, then line order.
    pub records: Vec<Record>,
    pub rows_without_id: Vec<Location>,
    pub broken_links: Vec<BrokenLink>,
    /// `{#…}` heading anchors that are not IDs.
    pub other_anchors: usize,
    /// Rows of headerless blocks whose ID cell holds an ID, whether or not
    /// the config reads such blocks as record tables.
    pub headerless_id_rows: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl Census {
    /// Documents whose front-matter block is closed.
    pub fn with_front_matter(&self) -> usize {
        self.documents_detail
            .iter()
            .filter(|document| document.front_matter == FrontMatterState::Present)
            .count()
    }

    /// Documents with front-matter per value of the class key (`None`: the
    /// block has no class key).
    pub fn per_class(&self) -> BTreeMap<Option<String>, usize> {
        let mut counts = BTreeMap::new();
        for document in &self.documents_detail {
            if document.front_matter == FrontMatterState::Present {
                *counts.entry(document.class.clone()).or_default() += 1;
            }
        }
        counts
    }

    pub fn front_matter_unclosed(&self) -> usize {
        self.documents_detail
            .iter()
            .filter(|document| document.front_matter == FrontMatterState::Unclosed)
            .count()
    }

    fn rows(&self) -> impl Iterator<Item = &Record> {
        self.records
            .iter()
            .filter(|record| record.kind == RecordKind::Row)
    }

    /// Rows whose ID cell holds an ID of any script.
    pub fn id_rows(&self) -> usize {
        self.rows().count()
    }

    /// ID'd rows per normalized prefix.
    pub fn per_prefix(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for record in self.rows() {
            *counts.entry(record.prefix.clone()).or_default() += 1;
        }
        counts
    }

    pub fn id_sections(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.kind == RecordKind::Section)
            .count()
    }

    /// Rows and sections whose ID mixes ASCII letters with look-alikes.
    pub fn mixed_script_ids(&self) -> usize {
        self.script_count(IdScript::MixedScript)
    }

    /// Rows and sections whose ID has no ASCII letter but foreign ones.
    pub fn non_latin_ids(&self) -> usize {
        self.script_count(IdScript::NonLatin)
    }

    fn script_count(&self, script: IdScript) -> usize {
        self.records
            .iter()
            .filter(|record| record.script == script)
            .count()
    }

    /// Records whose normalized ID already occurred earlier in the manifest.
    pub fn duplicate_ids(&self) -> usize {
        let mut seen = HashSet::new();
        self.records
            .iter()
            .filter(|record| !seen.insert(record.id.as_str()))
            .count()
    }

    pub fn tables(&self) -> usize {
        self.documents_detail.iter().map(|d| d.tables).sum()
    }

    pub fn headerless_blocks(&self) -> usize {
        self.documents_detail
            .iter()
            .map(|d| d.headerless_blocks)
            .sum()
    }

    pub fn record_tables(&self) -> usize {
        self.documents_detail.iter().map(|d| d.record_tables).sum()
    }

    pub fn links_checked(&self) -> usize {
        self.documents_detail.iter().map(|d| d.links_checked).sum()
    }
}

/// Runs the census over the corpus at `root`. Fails only when `root` itself
/// cannot be read; every per-file problem becomes a [`Diagnostic`].
pub fn run(root: &Path, config: &CensusConfig) -> Result<Census, String> {
    fs::read_dir(root).map_err(|error| format!("cannot read the corpus root: {error}"))?;
    let mut census = Census::default();
    let mut documents = BTreeSet::new();
    for configured in &config.roots {
        let relative = relative_string(configured);
        let absolute = root.join(configured);
        match fs::metadata(&absolute) {
            Ok(meta) if meta.is_dir() => walk(
                &absolute,
                &relative,
                config,
                &mut documents,
                &mut census.diagnostics,
            ),
            Ok(_) => {
                if file_name_of(&relative).is_some_and(|name| config.is_document(name))
                    && !config.is_excluded(&relative)
                {
                    documents.insert(relative);
                }
            }
            Err(error) => {
                census.roots_missing += 1;
                census.diagnostics.push(Diagnostic {
                    path: relative,
                    line: None,
                    message: format!("configured root not readable: {error}"),
                });
            }
        }
    }
    census.documents = documents.len();
    let wiki = config
        .wiki_root
        .as_deref()
        .map(|wiki_root| WikiIndex::build(root, wiki_root, &mut census.diagnostics));
    for relative in documents {
        let absolute = root.join(&relative);
        let text = match fs::read(&absolute) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    census.files_skipped += 1;
                    census.diagnostics.push(Diagnostic {
                        path: relative,
                        line: None,
                        message: "skipped: not UTF-8".to_owned(),
                    });
                    continue;
                }
            },
            Err(error) => {
                census.files_skipped += 1;
                census.diagnostics.push(Diagnostic {
                    path: relative,
                    line: None,
                    message: format!("skipped: {error}"),
                });
                continue;
            }
        };
        census_document(root, &relative, &text, config, wiki.as_ref(), &mut census);
    }
    census
        .records
        .sort_by(|a, b| (&a.path, a.line, a.kind).cmp(&(&b.path, b.line, b.kind)));
    Ok(census)
}

/// Collects documents under a directory: dot-directories and symlinks are
/// skipped; an unreadable directory is a diagnostic.
fn walk(
    absolute: &Path,
    relative: &str,
    config: &CensusConfig,
    documents: &mut BTreeSet<String>,
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
            if !name.starts_with('.') {
                walk(
                    &absolute.join(name.as_ref()),
                    &child,
                    config,
                    documents,
                    diagnostics,
                );
            }
        } else if config.is_document(&name) && !config.is_excluded(&child) {
            documents.insert(child);
        }
    }
}

fn census_document(
    root: &Path,
    relative: &str,
    text: &str,
    config: &CensusConfig,
    wiki: Option<&WikiIndex>,
    census: &mut Census,
) {
    census.bytes += text.len();
    let (state, class, body_start, body_line) =
        match frontmatter::read(text, config.class_key.as_deref()) {
            FrontMatter::Absent => (FrontMatterState::Absent, None, 0, 1),
            FrontMatter::Unclosed => {
                census.diagnostics.push(Diagnostic {
                    path: relative.to_owned(),
                    line: Some(1),
                    message: "front-matter opened with `---` and never closed; read as body"
                        .to_owned(),
                });
                (FrontMatterState::Unclosed, None, 0, 1)
            }
            FrontMatter::Present {
                class,
                body_start,
                body_line,
            } => (FrontMatterState::Present, class, body_start, body_line),
        };
    let scan = markdown::scan(text, body_start, body_line, wiki.is_some());
    let mut summary = DocumentSummary {
        path: relative.to_owned(),
        bytes: text.len(),
        front_matter: state,
        class,
        tables: scan
            .tables
            .iter()
            .filter(|table| table.header.is_some())
            .count(),
        headerless_blocks: 0,
        record_tables: 0,
        id_rows: 0,
        rows_without_id: 0,
        id_sections: 0,
        links_checked: 0,
        broken_links: 0,
    };

    for table in &scan.tables {
        census_table(relative, table, config, census, &mut summary);
    }
    if config.section_ids {
        census_sections(relative, text, &scan.headings, config, census, &mut summary);
    }

    let directory = Path::new(relative).parent().unwrap_or(Path::new(""));
    for link in &scan.links {
        let exists = match (link.kind, wiki) {
            (LinkKind::Markdown, _) => {
                let Some(target) = local_target(&link.destination) else {
                    continue;
                };
                let resolved = match target.strip_prefix('/') {
                    Some(from_root) => root.join(from_root),
                    None => root.join(directory).join(&target),
                };
                fs::metadata(&resolved).is_ok()
            }
            (LinkKind::Wiki, Some(wiki)) => {
                let target = link.destination.split('#').next().unwrap_or("").trim();
                if target.is_empty() {
                    continue;
                }
                wiki.resolves(target, &config.extensions, &root.join(directory))
            }
            (LinkKind::Wiki, None) => continue,
        };
        summary.links_checked += 1;
        if !exists {
            summary.broken_links += 1;
            census.broken_links.push(BrokenLink {
                path: relative.to_owned(),
                line: link.line,
                target: link.destination.clone(),
            });
        }
    }
    census.documents_detail.push(summary);
}

/// Every file under the wiki root, lower-cased, by its relative path and by
/// each trailing part of it after a `/`: how wiki links name files.
struct WikiIndex {
    suffixes: HashSet<String>,
}

impl WikiIndex {
    fn build(root: &Path, wiki_root: &Path, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let mut files = BTreeSet::new();
        collect_files(
            &root.join(wiki_root),
            &relative_string(wiki_root),
            &mut files,
            diagnostics,
        );
        let base = relative_string(wiki_root);
        let mut suffixes = HashSet::new();
        for file in files {
            let inside = if base.is_empty() {
                file.as_str()
            } else {
                file.strip_prefix(&base)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .unwrap_or(&file)
            };
            let lower = inside.to_lowercase();
            let mut rest = lower.as_str();
            loop {
                suffixes.insert(rest.to_owned());
                match rest.split_once('/') {
                    Some((_, tail)) => rest = tail,
                    None => break,
                }
            }
        }
        Self { suffixes }
    }

    /// Whether `target` names a file: as written or with a document
    /// extension, through the index or relative to the linking document.
    fn resolves(&self, target: &str, extensions: &[String], directory: &Path) -> bool {
        let target = target.trim_start_matches('/');
        let mut candidates = vec![target.to_owned()];
        candidates.extend(
            extensions
                .iter()
                .map(|extension| format!("{target}.{extension}")),
        );
        candidates.iter().any(|candidate| {
            self.suffixes.contains(&candidate.to_lowercase())
                || fs::metadata(directory.join(candidate)).is_ok()
        })
    }
}

/// Every file under a directory (dot-directories and symlinks skipped).
fn collect_files(
    absolute: &Path,
    relative: &str,
    files: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entries = match fs::read_dir(absolute) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(Diagnostic {
                path: relative.to_owned(),
                line: None,
                message: format!("wiki directory skipped: {error}"),
            });
            return;
        }
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if file_type.is_dir() && !name.starts_with('.') {
            collect_files(&entry.path(), &child, files, diagnostics);
        } else if file_type.is_file() {
            files.insert(child);
        }
    }
}

fn census_table(
    relative: &str,
    table: &Table<'_>,
    config: &CensusConfig,
    census: &mut Census,
    summary: &mut DocumentSummary,
) {
    let column = config.id_column;
    let ids: Vec<Option<IdMatch>> = table
        .rows
        .iter()
        .map(|row| {
            let cell = row.cells.get(column).map_or("", |cell| clean_cell(cell));
            if cell.is_empty() {
                None
            } else {
                config.ids.find(cell)
            }
        })
        .collect();
    let any_id = ids.iter().any(Option::is_some);
    let is_record_table = match (&table.header, &config.id_header) {
        (None, _) => {
            summary.headerless_blocks += 1;
            census.headerless_id_rows += ids.iter().filter(|id| id.is_some()).count();
            config.headerless_tables && any_id
        }
        (Some(header), Some(pattern)) => header
            .get(column)
            .is_some_and(|cell| pattern.is_match(clean_cell(cell))),
        (Some(_), None) => any_id,
    };
    if !is_record_table {
        return;
    }
    summary.record_tables += 1;
    for (row, id) in table.rows.iter().zip(ids) {
        match id {
            Some(id) => {
                summary.id_rows += 1;
                census.records.push(record(
                    relative,
                    row.line,
                    RecordKind::Row,
                    id,
                    row.verbatim,
                ));
            }
            None => {
                summary.rows_without_id += 1;
                census.rows_without_id.push(Location {
                    path: relative.to_owned(),
                    line: row.line,
                });
            }
        }
    }
}

fn census_sections(
    relative: &str,
    text: &str,
    headings: &[Heading<'_>],
    config: &CensusConfig,
    census: &mut Census,
    summary: &mut DocumentSummary,
) {
    for (index, heading) in headings.iter().enumerate() {
        let Some(anchor) = heading_anchor(&heading.text) else {
            continue;
        };
        let Some(id) = config.ids.find(anchor) else {
            census.other_anchors += 1;
            continue;
        };
        let end = headings[index + 1..]
            .iter()
            .find(|next| next.level <= heading.level)
            .map_or(text.len(), |next| next.start);
        let section = text
            .get(heading.start..end)
            .unwrap_or("")
            .trim_end_matches([' ', '\t', '\r', '\n']);
        summary.id_sections += 1;
        census.records.push(record(
            relative,
            heading.line,
            RecordKind::Section,
            id,
            section,
        ));
    }
}

fn record(relative: &str, line: usize, kind: RecordKind, id: IdMatch, verbatim: &str) -> Record {
    Record {
        path: relative.to_owned(),
        line,
        kind,
        verbatim_id: id.verbatim,
        id: id.id,
        prefix: id.prefix,
        script: id.script,
        blake3: blake3::hash(verbatim.as_bytes()).to_hex().to_string(),
    }
}

/// The `X` of the first `{#X …}` attribute block on a heading line.
fn heading_anchor(heading: &str) -> Option<&str> {
    let start = heading.find("{#")? + 2;
    let rest = &heading[start..];
    let close = rest.find('}')?;
    let anchor = rest[..close].split_whitespace().next()?;
    Some(anchor)
}

/// The text of an ID cell without Markdown decoration: emphasis, strike,
/// code and link brackets around it, and a leading unmatched marker.
fn clean_cell(cell: &str) -> &str {
    let mut text = cell.trim();
    loop {
        let before = text;
        for wrapper in ["**", "__", "~~", "`", "*", "_"] {
            if text.len() > 2 * wrapper.len()
                && let Some(inner) = text
                    .strip_prefix(wrapper)
                    .and_then(|rest| rest.strip_suffix(wrapper))
            {
                text = inner.trim();
                break;
            }
        }
        if let Some(rest) = text.strip_prefix('[')
            && let Some(close) = rest.find(']')
            && matches!(rest.as_bytes().get(close + 1), Some(b'(' | b'['))
        {
            text = rest[..close].trim();
        }
        if text == before {
            break;
        }
    }
    text.trim_start_matches(['*', '_', '~', '`']).trim()
}

/// The file part of a link destination that points into the file system;
/// `None` for anchors, URLs (any scheme) and empty targets. Backslash escapes
/// are resolved first, then percent-escapes in the file part.
fn local_target(destination: &str) -> Option<String> {
    let destination = unescape_backslashes(destination.trim());
    let destination = destination.as_str();
    if destination.is_empty() || destination.starts_with('#') || destination.starts_with("//") {
        return None;
    }
    let scheme_end = destination.find(':');
    if let Some(end) = scheme_end {
        let scheme = &destination[..end];
        if scheme
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            return None;
        }
    }
    let file = destination.split(['#', '?']).next().unwrap_or(destination);
    let file = percent_decode(file);
    (!file.is_empty()).then_some(file)
}

/// CommonMark backslash escapes: `\` before ASCII punctuation stands for that
/// character; before anything else it is a literal backslash.
fn unescape_backslashes(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(&next) = chars.peek()
            && next.is_ascii_punctuation()
        {
            out.push(next);
            chars.next();
            continue;
        }
        out.push(c);
    }
    out
}

fn percent_decode(text: &str) -> String {
    if !text.contains('%') {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let Some(hex) = text.get(index + 1..index + 3)
            && hex.bytes().all(|b| b.is_ascii_hexdigit())
            && let Ok(value) = u8::from_str_radix(hex, 16)
        {
            decoded.push(value);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| text.to_owned())
}

/// `/`-separated relative path; `.` components dropped, `""` for the root.
fn relative_string(path: &Path) -> String {
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
