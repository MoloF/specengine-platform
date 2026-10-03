//! The import engine (`docs/features/import-records.md` AC-03): the census
//! scanner plus record recognizers — `{#ID}` sections, record-table rows
//! (with and without a header row), list items opening with a strong ID —
//! the definition / reference rule, legacy prefixes, header maps, local
//! numbers, the unclaimed counter, hyphenless codes, link base and the code
//! scan, into a record model with verbatim hashes. Read-only; every
//! convention comes from the [`CensusConfig`] (ADR-0008).

mod code;
mod header;
mod ids;
mod lead_in;
mod model;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::census::{Diagnostic, LinkCheck, WikiIndex, check_link, clean_cell, heading_anchor};
use crate::config::CensusConfig;
use crate::frontmatter::{self, FrontMatter};
use crate::markdown::{self, LinkKind, Scan, Table, cell_ranges};
use crate::walk::{self, Walk};

pub use code::{Citation, CodeScan};
pub use header::{KeyEntry, MapOutcome, ValueEntry};
pub use model::{
    DocumentDetail, Duplicate, Field, Form, HeaderForm, HyphenlessMatch, HyphenlessRole, IdChange,
    Import, ImportLink, ImportRecord, Legacy, LinkProblem, Role, RowCause, RowWithoutId, Scope,
    Token, TokenCause, UnclaimedToken,
};

use header::HeaderKeys;
use ids::{Resolution, Resolved, Resolver};
use lead_in::LeadIn;

/// Runs the import over the corpus at `root`. Fails only when `root` itself
/// cannot be read; every per-file problem becomes a [`Diagnostic`].
pub fn run(root: &Path, config: &CensusConfig) -> Result<Import, String> {
    let walk = walk::documents(root, config)?;
    Ok(run_walked(root, config, walk))
}

/// An ID-like token waiting for the corpus's definitions.
struct Pending {
    path: String,
    line: usize,
    token: String,
    /// The Latin ID it names, when it names one.
    id: Option<String>,
    /// That ID's prefix is one of `feature_prefixes`.
    feature: bool,
}

/// The import over the documents of a walk already taken.
pub fn run_walked(root: &Path, config: &CensusConfig, walk: Walk) -> Import {
    let mut import = Import {
        documents: walk.documents.len(),
        roots_missing: walk.roots_missing,
        diagnostics: walk.diagnostics,
        ..Import::default()
    };
    let wiki = config
        .wiki_root
        .as_deref()
        .map(|wiki_root| WikiIndex::build(root, wiki_root, &mut import.diagnostics));
    let resolver = Resolver::new(
        &config.ids,
        &config.import.legacy,
        &config.import.like,
        &config.import.hyphenless,
    );
    let mut pending: Vec<Pending> = Vec::new();
    for relative in &walk.documents {
        let text = match crate::census::read_document(root, relative) {
            Ok(text) => text,
            Err(message) => {
                import.files_skipped += 1;
                import.diagnostics.push(Diagnostic {
                    path: relative.clone(),
                    line: None,
                    message,
                });
                import.documents_detail.push(DocumentDetail {
                    path: relative.clone(),
                    bytes: 0,
                    skipped: true,
                    header: HeaderForm::None,
                    class: None,
                    non_latin_keys: false,
                    keys: Vec::new(),
                    values: Vec::new(),
                    records: 0,
                    unclaimed: 0,
                });
                continue;
            }
        };
        let mut document = DocumentRun {
            root,
            relative,
            text: &text,
            config,
            resolver: &resolver,
            import: &mut import,
            pending: &mut pending,
        };
        document.run(wiki.as_ref());
    }
    import
        .records
        .sort_by(|a, b| (&a.path, a.line, a.form).cmp(&(&b.path, b.line, b.form)));
    settle(&mut import, pending);
    import.code = code::scan(
        root,
        &config.import.code,
        &walk.documents,
        &mut import.diagnostics,
    );
    import
}

/// Duplicates, unresolved references and claimed tokens, once every
/// definition is known.
fn settle(import: &mut Import, pending: Vec<Pending>) {
    let mut first: HashMap<(Option<&str>, &str), (&str, usize)> = HashMap::new();
    let mut duplicates = Vec::new();
    let mut defined: HashSet<&str> = HashSet::new();
    let mut defined_in: HashSet<(&str, &str)> = HashSet::new();
    // Records are in (path, line) order: the first definition comes first.
    for record in import
        .records
        .iter()
        .filter(|record| record.role == Role::Definition)
    {
        defined.insert(record.id.as_str());
        defined_in.insert((record.path.as_str(), record.id.as_str()));
        let document = (record.scope == Scope::Feature).then_some(record.path.as_str());
        match first.get(&(document, record.id.as_str())) {
            Some(&(first_path, first_line)) => duplicates.push(Duplicate {
                path: record.path.clone(),
                line: record.line,
                id: record.id.clone(),
                scope: record.scope,
                first_path: first_path.to_owned(),
                first_line,
            }),
            None => {
                first.insert(
                    (document, record.id.as_str()),
                    (record.path.as_str(), record.line),
                );
            }
        }
    }
    let unresolved: Vec<Token> = import
        .records
        .iter()
        .filter(|record| record.role == Role::Reference && !defined.contains(record.id.as_str()))
        .map(|record| Token {
            path: record.path.clone(),
            line: record.line,
            token: record.id.clone(),
        })
        .collect();
    let mut claimed = 0;
    let mut unclaimed = Vec::new();
    for token in pending {
        let cause = match token.id.as_deref() {
            Some(id) if token.feature && defined_in.contains(&(token.path.as_str(), id)) => None,
            Some(id) if token.feature && defined.contains(id) => Some(TokenCause::FeatureOutside),
            Some(id) if !token.feature && defined.contains(id) => None,
            _ => Some(TokenCause::Unclaimed),
        };
        match cause {
            None => claimed += 1,
            Some(cause) => unclaimed.push(UnclaimedToken {
                path: token.path,
                line: token.line,
                token: token.token,
                cause,
            }),
        }
    }
    let mut per_document: HashMap<&str, usize> = HashMap::new();
    for token in unclaimed
        .iter()
        .filter(|token| token.cause == TokenCause::Unclaimed)
    {
        *per_document.entry(token.path.as_str()).or_default() += 1;
    }
    let mut records: HashMap<&str, usize> = HashMap::new();
    for record in &import.records {
        *records.entry(record.path.as_str()).or_default() += 1;
    }
    for document in &mut import.documents_detail {
        document.unclaimed = per_document
            .get(document.path.as_str())
            .copied()
            .unwrap_or(0);
        document.records = records.get(document.path.as_str()).copied().unwrap_or(0);
    }
    import.duplicates = duplicates;
    import.unresolved = unresolved;
    import.claimed = claimed;
    import.unclaimed = unclaimed;
}

/// One document being imported.
struct DocumentRun<'r, 'c> {
    root: &'r Path,
    relative: &'r str,
    text: &'r str,
    config: &'c CensusConfig,
    resolver: &'r Resolver<'c>,
    import: &'r mut Import,
    pending: &'r mut Vec<Pending>,
}

impl DocumentRun<'_, '_> {
    fn run(&mut self, wiki: Option<&WikiIndex>) {
        let config = self.config;
        let text = self.text;
        let (header, class, body_start, body_line) =
            match frontmatter::read(text, config.class_key.as_deref()) {
                FrontMatter::Absent => (HeaderForm::None, None, 0, 1),
                FrontMatter::Unclosed => {
                    self.import.diagnostics.push(Diagnostic {
                        path: self.relative.to_owned(),
                        line: Some(1),
                        message: "front-matter opened with `---` and never closed; read as body"
                            .to_owned(),
                    });
                    (HeaderForm::Unclosed, None, 0, 1)
                }
                FrontMatter::Present {
                    class,
                    body_start,
                    body_line,
                } => (HeaderForm::Yaml, class, body_start, body_line),
            };
        let scan = markdown::scan(text, body_start, body_line, wiki.is_some());

        let mut keys = HeaderKeys::default();
        if header == HeaderForm::Yaml {
            for entry in frontmatter::entries(text) {
                keys.add(&config.import, entry.line, entry.key, entry.value);
            }
        }
        let field_table = scan
            .tables
            .iter()
            .position(|table| header::is_field_table(config, table));
        if let Some(index) = field_table {
            for (line, key, value) in header::field_table_entries(config, &scan.tables[index]) {
                keys.add(&config.import, line, key, value);
            }
        }
        let header = match (header, field_table) {
            (HeaderForm::None, Some(_)) => HeaderForm::FieldTable,
            (header, _) => header,
        };

        let reference_document = config
            .import
            .reference_paths
            .iter()
            .any(|glob| glob.is_match(self.relative));
        let mut record_ids: HashMap<usize, Vec<String>> = HashMap::new();
        for (index, table) in scan.tables.iter().enumerate() {
            if Some(index) != field_table {
                self.table(table, reference_document, &mut record_ids);
            }
        }
        if config.section_ids {
            self.sections(&scan, reference_document, &mut record_ids);
        }
        let table_lines = table_lines(&scan.tables);
        if config.import.list_lead_in {
            self.list_items(&scan, &table_lines, reference_document, &mut record_ids);
        }
        if header == HeaderForm::Yaml {
            for (line, value) in frontmatter::values(text) {
                self.tokens(line, value, None, Vec::new());
            }
        }
        self.text_tokens(&scan, &table_lines, record_ids);
        self.links(&scan, wiki);

        self.import.documents_detail.push(DocumentDetail {
            path: self.relative.to_owned(),
            bytes: text.len(),
            skipped: false,
            header,
            class,
            non_latin_keys: keys.non_latin_key,
            keys: keys.keys,
            values: keys.values,
            records: 0,
            unclaimed: 0,
        });
    }

    /// Records of one table, rows without an ID, unmapped legacy IDs.
    fn table(
        &mut self,
        table: &Table<'_>,
        reference_document: bool,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let config = self.config;
        let import = &config.import;
        let column = config.id_column;
        let resolutions: Vec<Resolution> = table
            .rows
            .iter()
            .map(|row| {
                let cell = row.cells.get(column).map_or("", |cell| clean_cell(cell));
                if cell.is_empty() {
                    Resolution::None
                } else {
                    match self.resolver.resolve(cell) {
                        Resolution::Id(resolved)
                            if self.resolver.is_hyphenless(&resolved.written) =>
                        {
                            Resolution::None
                        }
                        other => other,
                    }
                }
            })
            .collect();
        let any_id = resolutions
            .iter()
            .any(|resolution| !matches!(resolution, Resolution::None));
        let reference_table = table.header.as_ref().is_some_and(|header| {
            header.iter().any(|cell| {
                import
                    .reference_headers
                    .iter()
                    .any(|pattern| pattern.is_match(clean_cell(cell)))
            })
        });
        let is_record_table = match (&table.header, &config.id_header) {
            (None, _) => config.headerless_tables && any_id,
            (Some(_), _) if reference_table => any_id,
            (Some(header), Some(pattern)) => header
                .get(column)
                .is_some_and(|cell| pattern.is_match(clean_cell(cell))),
            (Some(_), None) => any_id,
        };
        if !is_record_table {
            return;
        }
        let text_column = table
            .header
            .as_ref()
            .and_then(|header| {
                let pattern = import.text_header.as_ref()?;
                header
                    .iter()
                    .enumerate()
                    .position(|(index, cell)| index != column && pattern.is_match(clean_cell(cell)))
            })
            .or(import.text_column)
            .unwrap_or(column + 1);
        let role = if reference_document || reference_table {
            Role::Reference
        } else {
            Role::Definition
        };
        let form = if table.header.is_some() {
            Form::TableRow
        } else {
            Form::HeaderlessRow
        };
        for (row, resolution) in table.rows.iter().zip(resolutions) {
            match resolution {
                Resolution::Id(resolved) => {
                    let cells = verbatim_cells(row.verbatim, &row.cells);
                    let text = cells.get(text_column).copied().unwrap_or("");
                    let fields = fields(table.header.as_deref(), &cells, column, text_column);
                    self.record(
                        row.line,
                        form,
                        role,
                        resolved,
                        text.to_owned(),
                        fields,
                        record_ids,
                    );
                }
                Resolution::Unmapped(written) => self.unmapped(row.line, written),
                Resolution::None if role == Role::Definition => {
                    let cell = row.cells.get(column).map_or("", |cell| clean_cell(cell));
                    let cause = match &import.local_number {
                        Some(pattern) if !cell.is_empty() && pattern.is_match(cell) => {
                            RowCause::LocalNumber
                        }
                        _ => RowCause::None,
                    };
                    self.import.rows_without_id.push(RowWithoutId {
                        path: self.relative.to_owned(),
                        line: row.line,
                        cause,
                        cell: cell.to_owned(),
                    });
                }
                Resolution::None => {}
            }
        }
    }

    /// `{#ID}` sections: from the heading to the next heading of the same or
    /// a higher level, trailing whitespace trimmed (the census span).
    fn sections(
        &mut self,
        scan: &Scan<'_>,
        reference_document: bool,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let role = if reference_document {
            Role::Reference
        } else {
            Role::Definition
        };
        for (index, heading) in scan.headings.iter().enumerate() {
            let Some(anchor) = heading_anchor(&heading.text) else {
                continue;
            };
            match self.resolver.resolve(anchor) {
                Resolution::Id(resolved) if !self.resolver.is_hyphenless(&resolved.written) => {
                    let end = scan.headings[index + 1..]
                        .iter()
                        .find(|next| next.level <= heading.level)
                        .map_or(self.text.len(), |next| next.start);
                    let section = self
                        .text
                        .get(heading.start..end)
                        .unwrap_or("")
                        .trim_end_matches([' ', '\t', '\r', '\n']);
                    self.record(
                        heading.line,
                        Form::Section,
                        role,
                        resolved,
                        lf_lines(section),
                        Vec::new(),
                        record_ids,
                    );
                }
                Resolution::Unmapped(written) => self.unmapped(heading.line, written),
                _ => {}
            }
        }
    }

    /// List items whose strong lead-in, minus one separator, is an ID. The
    /// text is taken from the line as written (HTML comments kept), from
    /// where the span and separator end in the comment-free line.
    fn list_items(
        &mut self,
        scan: &Scan<'_>,
        table_lines: &HashSet<usize>,
        reference_document: bool,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let config = self.config;
        let resolver = self.resolver;
        let separators = &config.import.separators;
        let role = if reference_document {
            Role::Reference
        } else {
            Role::Definition
        };
        let nested_record = |visible: &str| {
            matches!(
                list_lead_in(resolver, separators, visible),
                Some((_, _, Resolution::Id(_)))
            )
        };
        for (index, line) in scan.lines.iter().enumerate() {
            let Some(visible) = &line.visible else {
                continue;
            };
            if table_lines.contains(&line.number) {
                continue;
            }
            let Some((lead, stripped_inside, resolution)) =
                list_lead_in(resolver, separators, visible)
            else {
                continue;
            };
            match resolution {
                Resolution::Id(resolved) => {
                    let after = lead_in::after_span(visible, &lead, separators, stripped_inside);
                    let first = line.raw.get(line.raw_offset(after)..).unwrap_or("");
                    let mut text = first.trim_start_matches([' ', '\t']).to_owned();
                    for continued in lead_in::item_continuation(
                        &scan.lines,
                        index,
                        lead.marker_indent,
                        table_lines,
                        &nested_record,
                    ) {
                        text.push('\n');
                        text.push_str(continued);
                    }
                    let text = text.trim_end().to_owned();
                    self.record(
                        line.number,
                        Form::ListItem,
                        role,
                        resolved,
                        text,
                        Vec::new(),
                        record_ids,
                    );
                }
                Resolution::Unmapped(written) => self.unmapped(line.number, written),
                Resolution::None => {}
            }
        }
    }

    /// Hyphenless matches and ID-like tokens over the body text the scanner
    /// sees, record IDs excluded.
    fn text_tokens(
        &mut self,
        scan: &Scan<'_>,
        table_lines: &HashSet<usize>,
        mut record_ids: HashMap<usize, Vec<String>>,
    ) {
        let mut previous_blank = true;
        for line in &scan.lines {
            let Some(visible) = &line.visible else {
                previous_blank = false;
                continue;
            };
            if visible.trim().is_empty() {
                previous_blank = true;
                continue;
            }
            let heading = markdown::heading_level(visible).is_some();
            let lead = if heading || table_lines.contains(&line.number) {
                None
            } else {
                lead_in::list_item(visible).or_else(|| {
                    previous_blank
                        .then(|| lead_in::paragraph(visible))
                        .flatten()
                })
            };
            previous_blank = heading;
            let lead_start = lead.as_ref().map(|lead| {
                let content = &visible[lead.content.clone()];
                lead.content.start + (content.len() - content.trim_start().len())
            });
            let excluded = record_ids.remove(&line.number).unwrap_or_default();
            self.tokens(line.number, visible, lead_start, excluded);
        }
    }

    /// Hyphenless matches and ID-like tokens of one line of text:
    /// `lead_start` is where a strong lead-in's content starts (a hyphenless
    /// match there is a definition); each record ID in `excluded` (as
    /// written on the line) excludes one equal token.
    fn tokens(
        &mut self,
        line: usize,
        text: &str,
        lead_start: Option<usize>,
        mut excluded: Vec<String>,
    ) {
        let hyphenless = self.resolver.hyphenless(text);
        for (pattern, range) in &hyphenless {
            let role = if lead_start == Some(range.start) {
                HyphenlessRole::Definition
            } else {
                HyphenlessRole::Mention
            };
            self.import.legacy.hyphenless.push(HyphenlessMatch {
                path: self.relative.to_owned(),
                line,
                pattern: *pattern,
                token: text[range.clone()].to_owned(),
                role,
            });
        }
        for range in self.resolver.like_tokens(text) {
            if hyphenless
                .iter()
                .any(|(_, found)| found.start < range.end && range.start < found.end)
            {
                continue;
            }
            let token = &text[range];
            if let Some(position) = excluded.iter().position(|written| written == token) {
                excluded.swap_remove(position);
                continue;
            }
            let (id, feature) = match self.resolver.resolve_whole(token) {
                Resolution::Id(resolved) => {
                    let feature = self
                        .config
                        .import
                        .feature_prefixes
                        .contains(&resolved.prefix);
                    (Some(resolved.id), feature)
                }
                _ => (None, false),
            };
            self.pending.push(Pending {
                path: self.relative.to_owned(),
                line,
                token: token.to_owned(),
                id,
                feature,
            });
        }
    }

    fn links(&mut self, scan: &Scan<'_>, wiki: Option<&WikiIndex>) {
        let base = self.config.import.link_base.as_deref();
        for link in &scan.links {
            let problem = match check_link(self.root, self.relative, link, self.config, wiki, base)
            {
                LinkCheck::Skipped | LinkCheck::Resolved => continue,
                LinkCheck::ResolvedByBase => LinkProblem::ResolvedByBase,
                LinkCheck::Broken if link.kind == LinkKind::Wiki => LinkProblem::Wiki,
                LinkCheck::Broken => LinkProblem::File,
            };
            self.import.links.push(ImportLink {
                path: self.relative.to_owned(),
                line: link.line,
                target: link.destination.clone(),
                problem,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        line: usize,
        form: Form,
        role: Role,
        resolved: Resolved,
        text: String,
        fields: Vec<Field>,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let path = self.relative.to_owned();
        if resolved.legacy {
            self.import.legacy.mapped.push(IdChange {
                path: path.clone(),
                line,
                written: resolved.written.clone(),
                id: resolved.id.clone(),
            });
        }
        if resolved.homoglyph {
            self.import.legacy.homoglyph_fixes.push(IdChange {
                path: path.clone(),
                line,
                written: resolved.written.clone(),
                id: resolved.id.clone(),
            });
        }
        record_ids
            .entry(line)
            .or_default()
            .push(resolved.written.clone());
        let scope = if self
            .config
            .import
            .feature_prefixes
            .contains(&resolved.prefix)
        {
            Scope::Feature
        } else {
            Scope::Project
        };
        let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
        self.import.records.push(ImportRecord {
            path,
            line,
            form,
            aliases: if resolved.legacy {
                vec![resolved.written]
            } else {
                Vec::new()
            },
            id: resolved.id,
            prefix: resolved.prefix,
            script: resolved.script,
            role,
            scope,
            text,
            hash,
            fields,
        });
    }

    fn unmapped(&mut self, line: usize, written: String) {
        self.import.legacy.unmapped.push(Token {
            path: self.relative.to_owned(),
            line,
            token: written,
        });
    }
}

/// A list item whose strong lead-in, minus one separator, is a candidate:
/// the lead-in, whether the span held the separator, and what the
/// candidate resolves to; `None` for any other line.
fn list_lead_in(
    resolver: &Resolver<'_>,
    separators: &[String],
    visible: &str,
) -> Option<(LeadIn, bool, Resolution)> {
    let lead = lead_in::list_item(visible)?;
    let (candidate, stripped_inside) =
        lead_in::strip_separator(&visible[lead.content.clone()], separators);
    if candidate.is_empty() || resolver.is_hyphenless(candidate) {
        return None;
    }
    Some((lead, stripped_inside, resolver.resolve_whole(candidate)))
}

/// Lines of every table: header, delimiter and rows.
fn table_lines(tables: &[Table<'_>]) -> HashSet<usize> {
    let mut lines = HashSet::new();
    for table in tables {
        if table.header.is_some() {
            lines.insert(table.header_line);
            lines.insert(table.header_line + 1);
        }
        lines.extend(table.rows.iter().map(|row| row.line));
    }
    lines
}

/// The cells of a row as written, each without surrounding blanks; the
/// scanner's cells (HTML comments removed) when a comment shifts the split.
fn verbatim_cells<'t>(verbatim: &'t str, cells: &'t [String]) -> Vec<&'t str> {
    let ranges = cell_ranges(verbatim);
    if ranges.len() == cells.len() {
        ranges
            .into_iter()
            .map(|range| verbatim[range].trim())
            .collect()
    } else {
        cells.iter().map(String::as_str).collect()
    }
}

/// The cells other than the ID and the text, by header as written.
fn fields(
    header: Option<&[String]>,
    cells: &[&str],
    id_column: usize,
    text_column: usize,
) -> Vec<Field> {
    let mut seen = HashSet::new();
    let mut fields = Vec::new();
    for (column, value) in cells.iter().enumerate() {
        if column == id_column || column == text_column {
            continue;
        }
        let name = header
            .and_then(|header| header.get(column))
            .filter(|name| !name.is_empty() && !seen.contains(name.as_str()))
            .cloned()
            .unwrap_or_else(|| format!("col-{column}"));
        seen.insert(name.clone());
        fields.push(Field {
            header: name,
            value: (*value).to_owned(),
        });
    }
    fields
}

/// Lines joined by LF: a CR before an LF dropped, nothing else changed.
fn lf_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
}
