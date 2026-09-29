//! The rules of increment 1 (docs/canon/spec-check.md, "Rules"): parser
//! diagnostics through one table, class contracts, budgets,
//! ID definitions, `canon:` and front-matter references. Everything the
//! rules know about a project comes from its `[ids]`, `[paths]` and check
//! tables (`#universal`): no prefix, path or file name is written here.

use std::collections::{BTreeMap, BTreeSet};

use specengine_model::grammar;
use specengine_model::{
    CanonTarget, Diagnostic, DiagnosticCode, IdScheme, IdScope, Node, ParsedFile, Reference,
    Severity, Shape,
};

use super::baseline::{Baseline, DebtEntry};
use super::config::{CheckConfig, DocClass};
use super::input::{CheckFile, CheckInput, ProblemKind};
use super::report::{Cause, Debt, Finding, Fix, Report};
use super::text::{FileText, front_matter_failed, is_calendar_date, is_date_shaped};
use crate::Paths;

/// The one table: how a parser diagnostic reaches the report. Every code
/// keeps the parser's severity but `homoglyph` and `duplicate-id`, which are
/// errors here (`#ids`: mixed scripts and doubled IDs block).
pub const PARSER_SEVERITY: [(DiagnosticCode, Severity); 13] = [
    (DiagnosticCode::NotUtf8, Severity::Error),
    (DiagnosticCode::FrontmatterUnclosed, Severity::Error),
    (DiagnosticCode::FrontmatterYaml, Severity::Error),
    (DiagnosticCode::FrontmatterNotMapping, Severity::Error),
    (DiagnosticCode::FrontmatterType, Severity::Error),
    (DiagnosticCode::IdNotInScheme, Severity::Error),
    (DiagnosticCode::UnknownKey, Severity::Warning),
    (DiagnosticCode::UnknownLinkType, Severity::Warning),
    (DiagnosticCode::UnparsedReference, Severity::Warning),
    (DiagnosticCode::Homoglyph, Severity::Error),
    (DiagnosticCode::KindMismatch, Severity::Warning),
    (DiagnosticCode::DuplicateId, Severity::Error),
    (DiagnosticCode::BadRev, Severity::Warning),
];

/// The check's severity of a parser code, by [`PARSER_SEVERITY`].
pub fn parser_severity(code: DiagnosticCode) -> Severity {
    PARSER_SEVERITY
        .iter()
        .find(|(known, _)| *known == code)
        .map_or(code.severity(), |&(_, severity)| severity)
}

/// Statuses of a spec, by the convention.
const SPEC_STATUS: [&str; 4] = ["draft", "in-progress", "shipped", "abandoned"];

/// Runs every rule over `input`. Reads nothing and writes nothing; the
/// result depends only on the arguments, whatever the order of
/// `input.files`.
pub fn run(
    input: &CheckInput,
    scheme: &IdScheme,
    paths: &Paths,
    config: &CheckConfig,
    baseline: &Baseline,
    today: &str,
) -> Report {
    let mut causes = Vec::new();
    let mut findings = Vec::new();
    let today_valid = is_calendar_date(today);
    if !today_valid {
        causes.push(Cause {
            path: String::new(),
            message: format!("today `{today}` is not a YYYY-MM-DD date"),
        });
    }
    for problem in &input.problems {
        match problem.kind {
            ProblemKind::MissingRoot if paths.roots_written => causes.push(Cause {
                path: problem.path.clone(),
                message: "root written in `[paths] roots` names no directory and no `.md` file"
                    .to_owned(),
            }),
            ProblemKind::MissingRoot => {}
            ProblemKind::UnreadableDir => causes.push(Cause {
                path: problem.path.clone(),
                message: "directory cannot be listed; its files are unchecked".to_owned(),
            }),
            ProblemKind::SkippedName => findings.push(Finding {
                code: "name-skipped".to_owned(),
                severity: Severity::Warning,
                path: problem.path.clone(),
                line: 1,
                subject: String::new(),
                message: "a directory or `.md` name that is not UTF-8 was skipped".to_owned(),
                fix: None,
                debt: None,
            }),
        }
    }

    let mut files: Vec<&CheckFile> = input.files.iter().collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    for file in &files {
        if let Some(error) = &file.read_error {
            causes.push(Cause {
                path: file.path.clone(),
                message: format!("cannot read: {error}"),
            });
        }
    }

    let corpus = Corpus::new(&files, scheme);
    for (index, file) in files.iter().enumerate() {
        let Some(parsed) = &file.parsed else {
            continue;
        };
        let text = &corpus.texts[index];
        FileCheck {
            file,
            parsed,
            text,
            scheme,
            paths,
            config,
            corpus: &corpus,
            findings: &mut findings,
        }
        .run();
    }
    corpus.id_taken(&mut findings);

    let stale = apply_baseline(&mut findings, baseline, today, today_valid);
    Report::assemble(config.mode, input.files.len(), findings, stale, causes)
}

/// Marks findings matched by a baseline entry; returns the entries that
/// matched none. The severity is never changed: past `expires` an error
/// blocks again (its debt is marked expired) and a warning stays a warning.
fn apply_baseline(
    findings: &mut [Finding],
    baseline: &Baseline,
    today: &str,
    today_valid: bool,
) -> Vec<DebtEntry> {
    let mut by_key: BTreeMap<(&str, &str, &str), (usize, &DebtEntry)> = BTreeMap::new();
    for (index, entry) in baseline.entries.iter().enumerate() {
        by_key
            .entry((&entry.code, &entry.path, &entry.subject))
            .or_insert((index, entry));
    }
    let mut used = vec![false; baseline.entries.len()];
    for finding in findings.iter_mut() {
        let key = (
            finding.code.as_str(),
            finding.path.as_str(),
            finding.subject.as_str(),
        );
        let Some(&(index, entry)) = by_key.get(&key) else {
            continue;
        };
        used[index] = true;
        let expired = today_valid && today > entry.expires.as_str();
        finding.debt = Some(Debt {
            reason: entry.reason.clone(),
            expires: entry.expires.clone(),
            expired,
        });
    }
    baseline
        .entries
        .iter()
        .zip(used)
        .filter(|(_, used)| !used)
        .map(|(entry, _)| entry.clone())
        .collect()
}

/// What the rules need to know across files; every per-file vector is in
/// path order.
struct Corpus<'a> {
    paths: Vec<&'a str>,
    parses: Vec<Option<&'a ParsedFile>>,
    texts: Vec<FileText<'a>>,
    /// Path → file index.
    by_path: BTreeMap<&'a str, usize>,
    /// Latin ID → the files defining it (document or section), path order.
    defined: BTreeMap<String, Vec<usize>>,
    /// Legacy ID as written in `aliases:` → the files declaring it.
    aliases: BTreeMap<String, Vec<usize>>,
    /// Per file: its section IDs.
    sections: Vec<BTreeSet<String>>,
    /// Per file: its definitions (ID, line), document first.
    definitions: Vec<Vec<(String, usize)>>,
    /// Per file: the front-matter was read.
    readable: Vec<bool>,
    /// Prefixes unique only within a feature: exempt from `id-taken` until
    /// `slug/` scopes are checked.
    feature_prefixes: BTreeSet<&'a str>,
}

impl<'a> Corpus<'a> {
    fn new(files: &[&'a CheckFile], scheme: &'a IdScheme) -> Self {
        let mut corpus = Corpus {
            paths: files.iter().map(|file| file.path.as_str()).collect(),
            parses: files.iter().map(|file| file.parsed.as_ref()).collect(),
            feature_prefixes: scheme
                .prefixes()
                .iter()
                .filter(|spec| spec.scope == IdScope::Feature)
                .map(|spec| spec.prefix.as_str())
                .collect(),
            texts: Vec::with_capacity(files.len()),
            by_path: BTreeMap::new(),
            defined: BTreeMap::new(),
            aliases: BTreeMap::new(),
            sections: Vec::with_capacity(files.len()),
            definitions: Vec::with_capacity(files.len()),
            readable: Vec::with_capacity(files.len()),
        };
        for (index, file) in files.iter().enumerate() {
            let text = FileText::new(&file.bytes);
            corpus.by_path.entry(file.path.as_str()).or_insert(index);
            let mut sections = BTreeSet::new();
            let mut definitions = Vec::new();
            let mut readable = false;
            if let Some(parsed) = &file.parsed {
                readable = !front_matter_failed(parsed);
                if let Some(document) = parsed.document() {
                    if let Some(id) = &document.id {
                        let line = text.key_line("id").unwrap_or(1);
                        definitions.push((id.clone(), line));
                    }
                    if let Some(aliases) = document.fields.as_ref().and_then(|f| f.aliases.as_ref())
                    {
                        for alias in aliases {
                            push_unique(corpus.aliases.entry(alias.clone()).or_default(), index);
                        }
                    }
                }
                for section in parsed.sections() {
                    if let Some(id) = &section.id {
                        sections.insert(id.clone());
                        let line = section.heading.map_or(1, |span| text.line(span.start));
                        definitions.push((id.clone(), line));
                    }
                }
            }
            for (id, _) in &definitions {
                push_unique(corpus.defined.entry(id.clone()).or_default(), index);
            }
            corpus.texts.push(text);
            corpus.sections.push(sections);
            corpus.definitions.push(definitions);
            corpus.readable.push(readable);
        }
        corpus
    }

    /// `id-taken`: an ID of a project-scoped prefix defined in two files,
    /// reported on each later file (by path), naming the first.
    fn id_taken(&self, findings: &mut Vec<Finding>) {
        for (id, holders) in &self.defined {
            let Some((&first, later)) = holders.split_first() else {
                continue;
            };
            if later.is_empty() || self.feature_scoped(id) {
                continue;
            }
            for &index in later {
                if !self.readable[index] {
                    continue;
                }
                let line = self.definitions[index]
                    .iter()
                    .find(|(defined, _)| defined == id)
                    .map_or(1, |&(_, line)| line);
                findings.push(Finding {
                    code: "id-taken".to_owned(),
                    severity: Severity::Error,
                    path: self.paths[index].to_owned(),
                    line,
                    subject: id.clone(),
                    message: format!("`{id}` is already defined in {}", self.paths[first]),
                    fix: None,
                    debt: None,
                });
            }
        }
    }

    fn feature_scoped(&self, id: &str) -> bool {
        id.split_once('-')
            .is_some_and(|(prefix, _)| self.feature_prefixes.contains(prefix))
    }
}

fn push_unique(holders: &mut Vec<usize>, index: usize) {
    if holders.last() != Some(&index) {
        holders.push(index);
    }
}

/// How a front-matter reference fared.
enum Resolution {
    Resolved,
    /// `project:` or `slug/`: resolved in a later increment.
    Skipped,
    Dangling(String),
}

/// The class a document declares.
enum Declared<'a> {
    None,
    /// `class:` is not a string (the parser reported `frontmatter-type`).
    Mistyped,
    Unknown(&'a str),
    Known(DocClass),
}

/// The rules over one read file.
struct FileCheck<'r, 'a> {
    file: &'r CheckFile,
    parsed: &'r ParsedFile,
    text: &'r FileText<'a>,
    scheme: &'r IdScheme,
    paths: &'r Paths,
    config: &'r CheckConfig,
    corpus: &'r Corpus<'a>,
    findings: &'r mut Vec<Finding>,
}

impl FileCheck<'_, '_> {
    /// Every rule. A file whose front-matter fails gives only its parser
    /// findings; a document without `class:` (or without front-matter) gets
    /// `class-missing`, `id:` or not, and no contract or class cap, while its
    /// IDs, references and `canon:` are still checked.
    fn run(&mut self) {
        for diagnostic in &self.parsed.diagnostics {
            let finding = self.parser_finding(diagnostic);
            self.findings.push(finding);
        }
        if front_matter_failed(self.parsed) {
            return;
        }
        let Some(document) = self.parsed.document() else {
            return;
        };
        let declared = self.declared(document);
        match declared {
            Declared::None => self.push(
                "class-missing",
                1,
                "",
                "no `class:`: declare one of canon | decision | spec | generated".to_owned(),
            ),
            Declared::Mistyped => {}
            Declared::Unknown(class) => {
                let line = self.key_line("class");
                self.push(
                    "class-unknown",
                    line,
                    class,
                    format!("class `{class}` is none of canon | decision | spec | generated"),
                );
            }
            Declared::Known(class) => self.class_rules(document, class),
        }
        let class = match declared {
            Declared::Known(class) => Some(class),
            _ => None,
        };
        self.budget(document, class);
        self.ids(document);
        self.canon_path(document);
        self.references(document);
    }

    fn push(&mut self, code: &str, line: usize, subject: &str, message: String) {
        self.findings.push(Finding {
            code: code.to_owned(),
            severity: Severity::Error,
            path: self.file.path.clone(),
            line,
            subject: subject.to_owned(),
            message,
            fix: None,
            debt: None,
        });
    }

    fn parser_finding(&self, diagnostic: &Diagnostic) -> Finding {
        let subject = diagnostic
            .span
            .map(|span| self.text.text(span))
            .unwrap_or_default();
        Finding {
            code: diagnostic.code.as_str().to_owned(),
            severity: parser_severity(diagnostic.code),
            path: self.file.path.clone(),
            line: diagnostic.line,
            subject,
            message: diagnostic.message.clone(),
            fix: diagnostic
                .span
                .zip(diagnostic.fix.as_ref())
                .map(|(span, text)| Fix {
                    span,
                    text: text.clone(),
                }),
            debt: None,
        }
    }

    fn declared<'n>(&self, document: &'n Node) -> Declared<'n> {
        let fields = document.fields.as_ref();
        match fields.and_then(|fields| fields.class.as_deref()) {
            Some(name) => DocClass::parse(name).map_or(Declared::Unknown(name), Declared::Known),
            None if self.in_extra(document, "class") => Declared::Mistyped,
            None => Declared::None,
        }
    }

    fn in_extra(&self, document: &Node, key: &str) -> bool {
        document
            .extra
            .as_ref()
            .is_some_and(|extra| extra.iter().any(|entry| entry.key == key))
    }

    /// The line of a top-level key as written; 1 when unknown.
    fn key_line(&self, key: &str) -> usize {
        self.text.key_line(key).unwrap_or(1)
    }

    /// The top-level keys as written, with lines; from the parse (line 1)
    /// when no bytes were given.
    fn written_keys(&self, document: &Node) -> Vec<(String, usize)> {
        if !self.text.is_empty() {
            return self
                .text
                .keys()
                .map(|(key, line)| (key.to_owned(), line))
                .collect();
        }
        let mut keys: Vec<String> = Vec::new();
        if document.id.is_some() {
            keys.push("id".to_owned());
        }
        if document.rev.is_some() {
            keys.push("rev".to_owned());
        }
        if document.parent.is_some() {
            keys.push("parent".to_owned());
        }
        if let Some(fields) = &document.fields
            && let Ok(serde_json::Value::Object(map)) = serde_json::to_value(fields)
        {
            keys.extend(map.keys().cloned());
        }
        if let Some(extra) = &document.extra {
            keys.extend(extra.iter().map(|entry| entry.key.clone()));
        }
        keys.into_iter().map(|key| (key, 1)).collect()
    }

    fn has_key(&self, keys: &[(String, usize)], key: &str) -> bool {
        keys.iter().any(|(name, _)| name == key)
    }

    /// Contract, `scope`, dates, status, tier.
    fn class_rules(&mut self, document: &Node, class: DocClass) {
        let contract = self.config.classes.get(class);
        let keys = self.written_keys(document);
        for key in &contract.required {
            if !self.has_key(&keys, key) {
                self.push(
                    "key-missing",
                    1,
                    key,
                    format!("key `{key}` is required for class {class}"),
                );
            }
        }
        if contract.closed {
            for (key, line) in &keys {
                if !contract.allows(key) {
                    self.push(
                        "key-extra",
                        *line,
                        key,
                        format!("key `{key}` is not part of the {class} contract"),
                    );
                }
            }
        }
        let fields = document.fields.clone().unwrap_or_default();
        let scope_empty = match &fields.scope {
            Some(scope) => scope.is_empty(),
            None => self.has_key(&keys, "scope") && !self.in_extra(document, "scope"),
        };
        if scope_empty {
            let line = self.key_line("scope");
            self.push(
                "scope-empty",
                line,
                "scope",
                "`scope` is empty; routing depends on it".to_owned(),
            );
        }
        for (key, value) in [
            ("reviewed", &fields.reviewed),
            ("date", &fields.date),
            ("shipped", &fields.shipped),
        ] {
            if let Some(value) = value
                && !is_date_shaped(value)
            {
                let line = self.key_line(key);
                self.push(
                    "date-invalid",
                    line,
                    key,
                    format!("`{key}: {value}` is not a YYYY-MM-DD date"),
                );
            }
        }
        let status_line = self.key_line("status");
        match class {
            DocClass::Spec => {
                if let Some(status) = &fields.status {
                    if !SPEC_STATUS.contains(&status.as_str()) {
                        self.push(
                            "status-invalid",
                            status_line,
                            "status",
                            format!(
                                "status `{status}` is none of draft | in-progress | shipped | abandoned"
                            ),
                        );
                    }
                    if status == "shipped" && !self.has_key(&keys, "shipped") {
                        self.push(
                            "shipped-missing",
                            status_line,
                            "shipped",
                            "a shipped spec has no `shipped:` date".to_owned(),
                        );
                    }
                }
            }
            DocClass::Decision => {
                if let Some(status) = &fields.status {
                    let valid = matches!(status.as_str(), "accepted" | "rejected")
                        || grammar::split_superseded_by(status).is_some_and(|(_, target)| {
                            grammar::parse_reference(target, 0, self.scheme).is_some()
                        });
                    if !valid {
                        self.push(
                            "status-invalid",
                            status_line,
                            "status",
                            format!(
                                "status `{status}` is none of accepted | rejected | superseded-by <ID>"
                            ),
                        );
                    }
                    if status == "accepted" && fields.canon.is_none() {
                        self.push(
                            "canon-missing",
                            status_line,
                            "canon",
                            "an accepted decision has no `canon:` (the promotion rule)".to_owned(),
                        );
                    }
                }
            }
            DocClass::Canon => self.tier_rules(fields.tier),
            DocClass::Generated => {}
        }
        if class != DocClass::Canon
            && let Some(tier0) = &self.paths.tier0
            && *tier0 == self.file.path
        {
            let line = self.key_line("class");
            self.push(
                "tier-invalid",
                line,
                "tier",
                format!("{tier0} is the tier 0 file: class canon, tier: 0"),
            );
        }
    }

    fn tier_rules(&mut self, tier: Option<i64>) {
        let line = self.key_line("tier");
        let path = self.file.path.clone();
        let is_tier0_file = self.paths.tier0.as_deref() == Some(path.as_str());
        match tier {
            Some(0..=2) | None => {}
            Some(other) => self.push(
                "tier-invalid",
                line,
                "tier",
                format!("tier `{other}`: canon is 0, 1 or 2 (tier 3 is a status)"),
            ),
        }
        if tier == Some(0)
            && let Some(tier0) = &self.paths.tier0
            && !is_tier0_file
        {
            let message = format!("tier 0 is only {tier0}");
            self.push("tier-invalid", line, "tier", message);
        }
        if tier == Some(1)
            && let Some(name) = &self.paths.tier1_name
            && file_name(&path) != name
        {
            let message = format!("tier 1 is only a file named {name}");
            self.push("tier-invalid", line, "tier", message);
        }
        if is_tier0_file && tier != Some(0) {
            let message = format!("{path} is the tier 0 file: tier: 0");
            self.push("tier-invalid", line, "tier", message);
        }
    }

    /// Whole-file bytes against the cap of the document's slot.
    fn budget(&mut self, document: &Node, class: Option<DocClass>) {
        let budgets = &self.config.budgets;
        let tier = document.fields.as_ref().and_then(|fields| fields.tier);
        let slot = if self.paths.index.as_deref() == Some(self.file.path.as_str()) {
            Some(("index", budgets.index_bytes))
        } else {
            match class {
                Some(DocClass::Canon) => match tier {
                    Some(0) => Some(("tier0", budgets.tier0_bytes)),
                    Some(1) => Some(("tier1", budgets.tier1_bytes)),
                    _ => budgets.canon_bytes.map(|cap| ("canon", cap)),
                },
                Some(DocClass::Decision) => Some(("decision", budgets.decision_bytes)),
                _ => None,
            }
        };
        if let Some((slot, cap)) = slot
            && self.file.size > cap
        {
            let message = format!(
                "{} bytes, over the {slot} cap of {cap}: move detail down a tier; caps are never raised",
                self.file.size
            );
            self.push("budget", 1, slot, message);
        }
    }

    /// `id-width` on number-shape definitions; `file-name` under `records`.
    fn ids(&mut self, document: &Node) {
        let mut definitions: Vec<(String, usize)> = Vec::new();
        if let Some(id) = &document.id {
            definitions.push((id.clone(), self.key_line("id")));
        }
        for section in self.parsed.sections() {
            if let Some(id) = &section.id {
                let line = section.heading.map_or(1, |span| self.text.line(span.start));
                definitions.push((id.clone(), line));
            }
        }
        for (id, line) in &definitions {
            let Some((prefix, body)) = id.split_once('-') else {
                continue;
            };
            let Some(spec) = self.scheme.prefix(prefix) else {
                continue;
            };
            if spec.shape != Shape::Number {
                continue;
            }
            if let Some(width) = spec.width
                && body.chars().count() != width as usize
            {
                self.push(
                    "id-width",
                    *line,
                    id,
                    format!(
                        "`{id}` has {} digits; `[ids] {prefix}` issues {width}",
                        body.chars().count()
                    ),
                );
            }
        }
        if let Some(id) = &document.id
            && under(&self.file.path, &self.paths.records)
        {
            let name = file_name(&self.file.path);
            let named = name
                .strip_prefix(id.as_str())
                .is_some_and(|rest| rest.starts_with(['.', '-']));
            if !named {
                let line = self.key_line("id");
                self.push(
                    "file-name",
                    line,
                    id,
                    format!("a record holding `{id}` is named `{id}` followed by `.` or `-`"),
                );
            }
        }
    }

    /// A path-form `canon:`: `path#anchor` naming a walked canon document
    /// and one of its anchors or section IDs.
    fn canon_path(&mut self, document: &Node) {
        let Some(CanonTarget::Path(target)) = document
            .fields
            .as_ref()
            .and_then(|fields| fields.canon.as_ref())
        else {
            return;
        };
        let written = match (target.span, &target.anchor) {
            (Some(span), _) if !self.text.is_empty() => self.text.text(span),
            (_, Some(anchor)) => format!("{}#{anchor}", target.path),
            (_, None) => target.path.clone(),
        };
        let line = target
            .span
            .filter(|_| !self.text.is_empty())
            .map_or_else(|| self.key_line("canon"), |span| self.text.line(span.start));
        let Some(anchor) = &target.anchor else {
            self.push(
                "canon-form",
                line,
                &written,
                format!("`canon: {written}` is no path#anchor of a canon section"),
            );
            return;
        };
        let Some(&index) = self.corpus.by_path.get(target.path.as_str()) else {
            self.push(
                "canon-file",
                line,
                &written,
                format!(
                    "`canon:` names {}, which is no walked document",
                    target.path
                ),
            );
            return;
        };
        let Some(parsed) = self.corpus_parse(index) else {
            self.push(
                "canon-file",
                line,
                &written,
                format!("`canon:` names {}, which could not be read", target.path),
            );
            return;
        };
        let is_canon = parsed
            .document()
            .and_then(|document| document.fields.as_ref())
            .and_then(|fields| fields.class.as_deref())
            == Some(DocClass::Canon.as_str());
        if !is_canon {
            self.push(
                "canon-file",
                line,
                &written,
                format!("`canon:` names {}, which is not canon", target.path),
            );
            return;
        }
        let found = parsed.anchors.iter().any(|known| known.name == *anchor)
            || self.corpus.sections[index].contains(anchor);
        if !found {
            self.push(
                "canon-anchor",
                line,
                &written,
                format!("{} has no anchor or section `#{anchor}`", target.path),
            );
        }
    }

    fn corpus_parse(&self, index: usize) -> Option<&ParsedFile> {
        self.corpus.parses.get(index).copied().flatten()
    }

    /// Front-matter references: each resolves to a defined ID, a document's
    /// `aliases:` entry, or through `aliases_from`; `#Y` names a section of
    /// the ID's file.
    fn references(&mut self, document: &Node) {
        let mut references: Vec<(&str, Reference)> = Vec::new();
        let fields = document.fields.clone().unwrap_or_default();
        for (key, list) in [
            ("supersedes", &fields.supersedes),
            ("adrs", &fields.adrs),
            ("refs", &fields.refs),
        ] {
            for reference in list.iter().flatten() {
                references.push((key, reference.clone()));
            }
        }
        if let Some(status) = &fields.status
            && let Some((_, target)) = grammar::split_superseded_by(status)
            && let Some(found) = grammar::parse_reference(target, 0, self.scheme)
        {
            let mut reference = found.reference;
            reference.span = None;
            references.push(("status", reference));
        }
        if let Some(reference) = &fields.working_answer {
            references.push(("working_answer", reference.clone()));
        }
        if let Some(parent) = &document.parent {
            // `ParentRef` keeps no `alias_of`: the ID is read again.
            let reference = grammar::parse_reference(&parent.id, 0, self.scheme).map_or_else(
                || Reference {
                    id: parent.id.clone(),
                    alias_of: None,
                    script: specengine_model::IdScript::of(&parent.id),
                    project: None,
                    scope: None,
                    section: None,
                    rev: None,
                    form: Default::default(),
                    label: None,
                    span: None,
                },
                |found| found.reference,
            );
            references.push((
                "parent",
                Reference {
                    span: parent.span,
                    ..reference
                },
            ));
        }
        if let Some(links) = &fields.links {
            for (_, list) in links.iter() {
                for reference in list {
                    references.push(("links", reference.clone()));
                }
            }
        }
        if let Some(CanonTarget::Reference(reference)) = &fields.canon {
            references.push(("canon", reference.clone()));
        }

        for (key, reference) in references {
            let written = self.written(&reference);
            let reason = match self.resolve(&reference, &written) {
                Resolution::Resolved | Resolution::Skipped => continue,
                Resolution::Dangling(reason) => reason,
            };
            let line = reference
                .span
                .filter(|_| !self.text.is_empty())
                .map_or_else(|| self.key_line(key), |span| self.text.line(span.start));
            self.push(
                "ref-dangling",
                line,
                &written,
                format!("`{key}`: `{written}` {reason}"),
            );
        }
    }

    /// The reference as written: the text under its span, else rebuilt.
    fn written(&self, reference: &Reference) -> String {
        if let Some(span) = reference.span
            && !self.text.is_empty()
        {
            let text = self.text.text(span);
            if !text.is_empty() {
                return text;
            }
        }
        match &reference.section {
            Some(section) => format!("{}#{section}", reference.id),
            None => reference.id.clone(),
        }
    }

    fn resolve(&self, reference: &Reference, written: &str) -> Resolution {
        if reference.project.is_some() || reference.scope.is_some() {
            return Resolution::Skipped;
        }
        let corpus = self.corpus;
        let mut holders: Option<&Vec<usize>> = None;
        if reference.alias_of.is_none() {
            holders = corpus.defined.get(&reference.id);
        }
        if holders.is_none() {
            let bare = written.split(['#', '@']).next().unwrap_or(written);
            holders = corpus
                .aliases
                .get(&reference.id)
                .or_else(|| corpus.aliases.get(bare));
        }
        if holders.is_none()
            && let Some(prefix) = &reference.alias_of
            && let Some((_, body)) = reference.id.split_once('-')
        {
            holders = corpus.defined.get(&format!("{prefix}-{body}"));
        }
        let Some(holders) = holders else {
            return Resolution::Dangling("resolves to no ID and no alias".to_owned());
        };
        if let Some(section) = &reference.section
            && !holders
                .iter()
                .any(|&index| corpus.sections[index].contains(section))
        {
            return Resolution::Dangling(format!("has no section `#{section}` in its file"));
        }
        Resolution::Resolved
    }
}

/// `path` lies strictly under the directory `dir`.
fn under(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
