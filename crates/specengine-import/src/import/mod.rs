//! The import engine (`docs/features/import-records.md` AC-03): the census
//! scanner plus record recognizers — `{#ID}` sections, record-table rows
//! (with and without a header row), list items opening with a strong ID
//! (optionally titled), documents defining their own ID
//! (`docs/features/import-gaps.md` AC-01–AC-03) — the definition /
//! reference rule and its document precedence, legacy prefixes, header maps, local
//! numbers, the unclaimed counter, hyphenless codes, link base and the code
//! scan, into a record model with verbatim hashes. Read-only; every
//! convention comes from the [`CensusConfig`] (ADR-0008).

mod code;
mod header;
mod ids;
mod lead_in;
mod model;

use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
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
pub(crate) use header::field_table_entries;
use ids::{Resolution, Resolved, Resolver};
use lead_in::{LeadIn, Split};

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

/// Precedence, duplicates, unresolved references and claimed tokens, once
/// every definition is known.
fn settle(import: &mut Import, pending: Vec<Pending>) {
    // An ID a document defines turns every record-position definition of it
    // into a reference (`docs/features/import-gaps.md` AC-03).
    let by_document: HashSet<String> = import
        .records
        .iter()
        .filter(|record| record.form == Form::Document && record.role == Role::Definition)
        .map(|record| record.id.clone())
        .collect();
    for record in &mut import.records {
        if record.form != Form::Document
            && record.role == Role::Definition
            && by_document.contains(&record.id)
        {
            record.role = Role::Reference;
            import.by_document += 1;
        }
    }
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
        let scan = markdown::scan_import(text, body_start, body_line, wiki.is_some());

        // Header keys whose target is `documents.id_key`: (line, value).
        let id_key = config.import.documents.id_key.as_deref();
        let mut id_values: Vec<(usize, Option<&str>)> = Vec::new();
        let mut keys = HeaderKeys::default();
        let mut add_key = |keys: &mut HeaderKeys, line, key, value| {
            keys.add(&config.import, line, key, value);
            if id_key.is_some() && keys.keys.last().and_then(|key| key.target.as_deref()) == id_key
            {
                id_values.push((line, value));
            }
        };
        if header == HeaderForm::Yaml {
            for entry in frontmatter::entries(text) {
                add_key(&mut keys, entry.line, entry.key, entry.value);
            }
        }
        let field_table_found = field_table(config, &scan);
        let field_table = field_table_found.as_ref().map(|(index, _)| *index);
        if let Some(index) = field_table {
            for (line, key, value) in header::field_table_entries(config, &scan.tables[index]) {
                add_key(&mut keys, line, key, value);
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
        let field_table_lines = field_table_found.map(|(_, lines)| lines);
        self.document(
            &id_values,
            || document_body(text, &scan, body_start, field_table_lines),
            reference_document,
            &mut record_ids,
        );
        for (index, table) in scan.tables.iter().enumerate() {
            if Some(index) != field_table {
                self.table(table, reference_document, &mut record_ids);
            }
        }
        let table_lines = table_lines(&scan.tables);
        let items = if config.import.list_lead_in {
            self.list_items(&scan, &table_lines)
        } else {
            Vec::new()
        };
        if config.section_ids {
            self.sections(&scan, reference_document, &mut record_ids);
        }
        self.list_records(items, reference_document, &mut record_ids);
        if header == HeaderForm::Yaml {
            for (line, value) in frontmatter::values(text) {
                // The value defining the document's ID is no token.
                let excluded = record_ids.remove(&line).unwrap_or_default();
                self.tokens(line, value, None, excluded);
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
                cell_resolution(
                    self.resolver,
                    row.cells.get(column).map_or("", String::as_str),
                )
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
                    let id_cell = (text_column != column)
                        .then(|| id_cell(table.header.as_deref(), &cells, column, &resolved))
                        .flatten();
                    self.record(
                        Found {
                            line: row.line,
                            form,
                            title: None,
                            text: text.to_owned(),
                            fields,
                            extent: [row.line, row.line],
                            task_box: None,
                            id_cell,
                        },
                        role,
                        resolved,
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
        // The import's scan reads no heading on a line opening inside a
        // comment: no section starts or ends there.
        let headings = &scan.headings;
        for (index, heading) in headings.iter().enumerate() {
            let Some(anchor) = heading_anchor(&heading.text) else {
                continue;
            };
            match self.resolver.resolve(anchor) {
                Resolution::Id(resolved) if !self.resolver.is_hyphenless(&resolved.written) => {
                    let end = headings[index + 1..]
                        .iter()
                        .find(|next| next.level <= heading.level)
                        .map_or(self.text.len(), |next| next.start);
                    let section = self
                        .text
                        .get(heading.start..end)
                        .unwrap_or("")
                        .trim_end_matches([' ', '\t', '\r', '\n']);
                    // The heading line, then one line per LF the trimmed span holds.
                    let last = heading.line + section.matches('\n').count();
                    self.record(
                        Found {
                            line: heading.line,
                            form: Form::Section,
                            title: None,
                            text: lf_lines(section),
                            fields: Vec::new(),
                            extent: [heading.line, last],
                            task_box: None,
                            id_cell: None,
                        },
                        role,
                        resolved,
                        record_ids,
                    );
                }
                Resolution::Unmapped(written) => self.unmapped(heading.line, written),
                _ => {}
            }
        }
    }

    /// List items whose strong lead-in holds an ID, optionally followed by a
    /// separator and a title. The text is taken from the line as written (HTML
    /// comments kept), from where the span and separator end in the
    /// comment-free line; the title, outside the text, from the span. No
    /// item is read on a line opening inside a comment.
    fn list_items(&self, scan: &Scan<'_>, table_lines: &HashSet<usize>) -> Vec<ListItem> {
        let resolver = self.resolver;
        let separators = &self.config.import.separators;
        let nested_record = |visible: &str| {
            list_lead_in(resolver, separators, visible)
                .is_some_and(|found| matches!(found.resolution, Resolution::Id(_)))
        };
        let mut items = Vec::new();
        for (index, line) in scan.lines.iter().enumerate() {
            if line.opens_in_comment || table_lines.contains(&line.number) {
                continue;
            }
            let Some(visible) = &line.visible else {
                continue;
            };
            let Some(ListLeadIn {
                lead,
                split,
                resolution,
            }) = list_lead_in(resolver, separators, visible)
            else {
                continue;
            };
            match resolution {
                Resolution::Id(resolved) => {
                    // The title as written: a comment right after the
                    // separator or before the closing delimiter is kept.
                    let title = split.title_from.and_then(|from| {
                        let end = line.raw_offset_past(lead.content.end);
                        line.raw
                            .get(line.raw_offset(from)..end)
                            .map(str::trim)
                            .filter(|title| !title.is_empty())
                            .map(str::to_owned)
                    });
                    let after =
                        lead_in::after_span(visible, &lead, separators, split.stripped_inside);
                    let first = line.raw.get(line.raw_offset(after)..).unwrap_or("");
                    let mut text = first.trim_start_matches([' ', '\t']).to_owned();
                    let mut last = line.number;
                    for continued in lead_in::item_continuation(
                        &scan.lines,
                        index,
                        lead.marker_indent,
                        table_lines,
                        &nested_record,
                    ) {
                        text.push('\n');
                        text.push_str(continued.raw);
                        last = continued.number;
                    }
                    items.push(ListItem::Record {
                        line: line.number,
                        resolved,
                        title,
                        text: text.trim_end().to_owned(),
                        last,
                        task_box: lead.task_box,
                    });
                }
                Resolution::Unmapped(written) => items.push(ListItem::Unmapped {
                    line: line.number,
                    written,
                }),
                Resolution::None => {}
            }
        }
        items
    }

    /// The records and unmapped legacy IDs of [`Self::list_items`].
    fn list_records(
        &mut self,
        items: Vec<ListItem>,
        reference_document: bool,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let role = if reference_document {
            Role::Reference
        } else {
            Role::Definition
        };
        for item in items {
            match item {
                ListItem::Record {
                    line,
                    resolved,
                    title,
                    text,
                    last,
                    task_box,
                } => self.record(
                    Found {
                        line,
                        form: Form::ListItem,
                        title,
                        text,
                        fields: Vec::new(),
                        extent: [line, last],
                        task_box,
                        id_cell: None,
                    },
                    role,
                    resolved,
                    record_ids,
                ),
                ListItem::Unmapped { line, written } => self.unmapped(line, written),
            }
        }
    }

    /// Hyphenless matches and ID-like tokens over the body text the scanner
    /// sees, record IDs excluded. No lead-in or heading is read on a line
    /// opening inside a comment.
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
            let heading = !line.opens_in_comment && markdown::heading_level(visible).is_some();
            let lead = if heading || line.opens_in_comment || table_lines.contains(&line.number) {
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

    fn record(
        &mut self,
        found: Found,
        role: Role,
        resolved: Resolved,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let Found {
            line,
            form,
            title,
            text,
            fields,
            extent,
            task_box,
            id_cell,
        } = found;
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
                vec![resolved.written.clone()]
            } else {
                Vec::new()
            },
            written: resolved.written,
            extent,
            task_box,
            id_cell,
            id: resolved.id,
            prefix: resolved.prefix,
            script: resolved.script,
            role,
            scope,
            title,
            text,
            hash,
            fields,
        });
    }

    /// The document's own ID (`[documents]`, `docs/features/import-gaps.md`
    /// AC-02): the first header key reaching `id_key`, else the `id_path`
    /// group `id`, each read as a record-table ID cell; the header's wins.
    /// One `document` record whose text is `body()`. The header value read
    /// is no citation: it is no token (`record_ids`) whether it defines the
    /// document, resolves to no ID or to a feature-scoped one.
    fn document(
        &mut self,
        id_values: &[(usize, Option<&str>)],
        body: impl FnOnce() -> String,
        reference_document: bool,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let resolver = self.resolver;
        if let Some(&(line, _)) = id_values.get(1) {
            self.diagnostic(
                Some(line),
                "another header key reaches `documents.id_key`; the first one's value is read"
                    .to_owned(),
            );
        }
        let header_id = match id_values.first() {
            Some(&(line, value)) => {
                match value.map_or(Resolution::None, |value| cell_resolution(resolver, value)) {
                    Resolution::Id(resolved) => Some((line, resolved)),
                    _ => {
                        self.diagnostic(
                            Some(line),
                            "the `documents.id_key` value is no single-line ID".to_owned(),
                        );
                        self.exclude_value(line, value, record_ids);
                        None
                    }
                }
            }
            None => None,
        };
        // `Some(None)`: the path matches, group `id` takes no part in it.
        let path_id = self
            .config
            .import
            .documents
            .id_path
            .as_ref()
            .and_then(|pattern| pattern.captures(self.relative))
            .map(|captures| {
                captures
                    .name("id")
                    .map(|found| cell_resolution(resolver, found.as_str()))
            });
        let (line, resolved, from_header) = match (header_id, path_id) {
            (Some((line, header)), path) => {
                if let Some(Some(Resolution::Id(path))) = path
                    && path.id != header.id
                {
                    self.diagnostic(
                        Some(line),
                        format!(
                            "the header names `{}`, the path `{}`; the header's ID is read",
                            header.written, path.written
                        ),
                    );
                }
                (line, header, true)
            }
            (None, Some(Some(Resolution::Id(path)))) => (1, path, false),
            (None, Some(Some(_))) => {
                self.diagnostic(
                    None,
                    "the `documents.id_path` group `id` is no ID".to_owned(),
                );
                return;
            }
            (None, Some(None)) => {
                self.diagnostic(
                    None,
                    "the path matches `documents.id_path` without its group `id`".to_owned(),
                );
                return;
            }
            (None, None) => return,
        };
        if self
            .config
            .import
            .feature_prefixes
            .contains(&resolved.prefix)
        {
            self.diagnostic(
                from_header.then_some(line),
                format!(
                    "`{}` is feature-scoped: no document record",
                    resolved.written
                ),
            );
            if let Some(&(line, value)) = id_values.first().filter(|_| from_header) {
                self.exclude_value(line, value, record_ids);
            }
            return;
        }
        let role = if reference_document {
            Role::Reference
        } else {
            Role::Definition
        };
        // An ID read from the path stands on no line: it excludes no token.
        let mut path_ids = HashMap::new();
        let record_ids = if from_header {
            record_ids
        } else {
            &mut path_ids
        };
        self.record(
            Found {
                line,
                form: Form::Document,
                title: None,
                text: body(),
                fields: Vec::new(),
                extent: [1, self.text.split_inclusive('\n').count().max(1)],
                task_box: None,
                id_cell: None,
            },
            role,
            resolved,
            record_ids,
        );
    }

    /// Excludes every ID-like token of a header value at `line` that defines
    /// no record: the value is no citation.
    fn exclude_value(
        &self,
        line: usize,
        value: Option<&str>,
        record_ids: &mut HashMap<usize, Vec<String>>,
    ) {
        let Some(value) = value else { return };
        record_ids.entry(line).or_default().extend(
            self.resolver
                .like_tokens(value)
                .into_iter()
                .map(|range| value[range].to_owned()),
        );
    }

    fn diagnostic(&mut self, line: Option<usize>, message: String) {
        self.import.diagnostics.push(Diagnostic {
            path: self.relative.to_owned(),
            line,
            message,
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

/// What a recognizer found at one position, before the record is made.
struct Found {
    line: usize,
    form: Form,
    title: Option<String>,
    text: String,
    fields: Vec<Field>,
    /// `[first, last]` source lines.
    extent: [usize; 2],
    task_box: Option<bool>,
    /// A table row's ID cell holding more than its ID
    /// ([`ImportRecord::id_cell`]).
    id_cell: Option<Field>,
}

/// A list item read before any record is made.
enum ListItem {
    Record {
        line: usize,
        resolved: Resolved,
        title: Option<String>,
        text: String,
        /// The line of the text's last line (the marker's when it has one).
        last: usize,
        task_box: Option<bool>,
    },
    /// The lead-in holds an unmapped legacy ID.
    Unmapped { line: usize, written: String },
}

/// A list item's strong lead-in read as ID [separator [title]].
struct ListLeadIn {
    lead: LeadIn,
    split: Split,
    /// An ID or an unmapped legacy prefix, never [`Resolution::None`].
    resolution: Resolution,
}

/// A list item whose strong lead-in holds a resolving ID candidate: the
/// longest (a hyphenless one is none); `None` for any other line.
fn list_lead_in(
    resolver: &Resolver<'_>,
    separators: &[String],
    visible: &str,
) -> Option<ListLeadIn> {
    let lead = lead_in::list_item(visible)?;
    let (split, resolution) = lead_in::splits(visible, &lead, separators)
        .into_iter()
        .find_map(|split| {
            let candidate = &visible[split.id.clone()];
            if resolver.is_hyphenless(candidate) {
                return None;
            }
            match resolver.resolve_whole(candidate) {
                Resolution::None => None,
                resolution => Some((split, resolution)),
            }
        })?;
    Some(ListLeadIn {
        lead,
        split,
        resolution,
    })
}

/// What a record-table ID cell holds: decoration removed, legacy map,
/// look-alikes, `ids.regex`; a hyphenless ID is none.
fn cell_resolution(resolver: &Resolver<'_>, cell: &str) -> Resolution {
    let cell = clean_cell(cell);
    if cell.is_empty() {
        return Resolution::None;
    }
    match resolver.resolve(cell) {
        Resolution::Id(resolved) if resolver.is_hyphenless(&resolved.written) => Resolution::None,
        other => other,
    }
}

/// A document's text without its header: the body after a closed YAML
/// block, less the lines of a field table opening it (`field_table`) and
/// the blank lines right after them; headings before the field table are
/// text, kept in order. Normalised by [`document_text`].
fn document_body(
    text: &str,
    scan: &Scan<'_>,
    body_start: usize,
    field_table: Option<RangeInclusive<usize>>,
) -> String {
    let body = match field_table {
        Some(lines) => {
            let line_start = |number: usize| {
                scan.lines
                    .iter()
                    .find(|line| line.number == number)
                    .map_or(text.len(), |line| line.start)
            };
            let after = scan
                .lines
                .iter()
                .find(|line| line.number > *lines.end() && !is_blank(line.raw))
                .map_or(text.len(), |line| line.start);
            let before = text
                .get(body_start..line_start(*lines.start()))
                .unwrap_or("");
            format!("{before}{}", text.get(after..).unwrap_or(""))
        }
        None => text.get(body_start..).unwrap_or("").to_owned(),
    };
    document_text(&body)
}

/// The document's field table (`front_matter.header_table`): its index in
/// `scan.tables` and its lines, header row, delimiter row and rows.
pub(crate) fn field_table(
    config: &CensusConfig,
    scan: &Scan<'_>,
) -> Option<(usize, RangeInclusive<usize>)> {
    let index = scan
        .tables
        .iter()
        .position(|table| header::is_field_table(config, table))?;
    let table = &scan.tables[index];
    let end = table
        .rows
        .last()
        .map_or(table.header_line + 1, |row| row.line);
    Some((index, table.header_line..=end))
}

/// Blanks as Markdown reads them: spaces, tabs and line terminators, not
/// every Unicode white space.
const BLANKS: [char; 4] = [' ', '\t', '\r', '\n'];

/// A `document` record's `text` from a document's body after its header
/// (`docs/canon/import.md` "Text and hash"): a leading BOM
/// excluded, a CR before an LF dropped, leading blank lines (only spaces and
/// tabs) and trailing spaces, tabs, CRs and LFs trimmed, nothing else
/// changed (no other Unicode white space is a blank). `import-layout`
/// normalises an emitted file's body with it, so that the two hash alike.
pub fn document_text(body: &str) -> String {
    let body = lf_lines(body.strip_prefix('\u{FEFF}').unwrap_or(body));
    let mut rest = body.as_str();
    while let Some(newline) = rest.find('\n')
        && is_blank(&rest[..newline])
    {
        rest = &rest[newline + 1..];
    }
    rest.trim_end_matches(BLANKS).to_owned()
}

/// A line (its CR before an LF dropped) of only spaces and tabs.
pub(crate) fn is_blank(line: &str) -> bool {
    line.bytes().all(|byte| byte == b' ' || byte == b'\t')
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
pub(crate) fn verbatim_cells<'t>(verbatim: &'t str, cells: &'t [String]) -> Vec<&'t str> {
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

/// A row's ID cell as a field (`docs/features/import-layout.md` AC-03,
/// AC-04): the cell as written under its column's header as written
/// (`col-N` without one, or for an empty header cell), when a letter or
/// digit is left once the written ID is cut from it (decoration alone
/// leaves none); `None` otherwise.
fn id_cell(
    header: Option<&[String]>,
    cells: &[&str],
    id_column: usize,
    resolved: &Resolved,
) -> Option<Field> {
    let cell = *cells.get(id_column)?;
    let beyond = cell
        .replacen(resolved.written.as_str(), " ", 1)
        .chars()
        .any(|c| c.is_alphabetic() || c.is_ascii_digit());
    beyond.then(|| Field {
        header: header
            .and_then(|header| header.get(id_column))
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("col-{id_column}")),
        value: cell.to_owned(),
        column: id_column,
    })
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
            column,
        });
    }
    fields
}

/// Lines joined by LF: a CR before an LF dropped, nothing else changed.
fn lf_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
}
