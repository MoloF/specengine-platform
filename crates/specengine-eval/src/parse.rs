//! Measurement `parse`: the spec parser of `specengine-core` over a corpus
//! (docs/features/spec-parser.md, "`specengine-eval parse`").
//!
//! Files are the census's documents: the same `--config` (default
//! `census.toml` at the corpus root), the same walk. The ID scheme comes from
//! `--scheme` (default: `SPECENGINE_SCHEME_A` / `_B` for `--label pilot-a` /
//! `pilot-b` when set, else `specengine.toml` at the corpus root); a missing
//! or invalid scheme refuses the run (exit 2, `file:line: message`).
//!
//! stdout carries counts only; paths, IDs and messages go to `--out` only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use specengine_core::IdSchemeToml;
use specengine_import::{CensusConfig, RecordKind};
use specengine_model::{DiagnosticCode, IdScheme, LinkOrigin, LinkTarget, ParsedFile};

use crate::census;
use crate::harness::Corpus;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "corpus-mini";

/// Scheme file looked up at the corpus root when `--scheme` is absent.
pub const DEFAULT_SCHEME: &str = "specengine.toml";

const ENV_SCHEME_A: &str = "SPECENGINE_SCHEME_A";
const ENV_SCHEME_B: &str = "SPECENGINE_SCHEME_B";

/// Diagnostics echoed on stderr; the rest are in `diagnostics.json`.
const DIAGNOSTICS_ON_STDERR: usize = 10;

/// What the run needs, read before anything is written.
pub struct Setup {
    config: CensusConfig,
    scheme: IdScheme,
}

/// Reads the census config and the ID scheme; any error refuses the run.
pub fn prepare(
    root: &Path,
    config: Option<&Path>,
    scheme: Option<&Path>,
    label: Option<&str>,
) -> Result<Setup, String> {
    let config = census::load_config(config, root)?;
    let from_environment = || {
        let variable = match label {
            Some("pilot-a") => ENV_SCHEME_A,
            Some("pilot-b") => ENV_SCHEME_B,
            _ => return None,
        };
        std::env::var_os(variable)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let path = match scheme.map(Path::to_path_buf).or_else(from_environment) {
        Some(path) => path,
        None => {
            let path = root.join(DEFAULT_SCHEME);
            if !path.is_file() {
                return Err(format!(
                    "no --scheme given and no {DEFAULT_SCHEME} at the corpus root"
                ));
            }
            path
        }
    };
    let shown = path.display().to_string();
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("{shown}: cannot read the ID scheme: {error}"))?;
    let scheme = IdScheme::from_toml(&text).map_err(|error| error.at(&shown))?;
    Ok(Setup { config, scheme })
}

/// The `result` object.
#[derive(Serialize)]
pub struct ParseResult {
    pub files: usize,
    /// Files whose parse panicked (caught per file).
    pub panics: usize,
    pub not_utf8: usize,
    pub front_matter: FrontMatterCounts,
    pub sections: SectionCounts,
    /// `{#…}` heading anchors that are not IDs of the scheme.
    pub heading_attrs_not_section: usize,
    pub references: ReferenceCounts,
    pub tokens_est: TokenCounts,
}

#[derive(Serialize)]
pub struct FrontMatterCounts {
    /// Files with a closed front-matter block.
    pub present: usize,
    /// Every diagnostic code of the parser (all files, front-matter and
    /// body), including the zeros.
    pub diagnostics: BTreeMap<&'static str, usize>,
}

#[derive(Serialize)]
pub struct SectionCounts {
    pub parsed: usize,
    pub census_id_sections: usize,
    /// Per file, the heading lines that one of parser and census has as a
    /// section and the other does not, summed.
    pub differ: usize,
}

#[derive(Serialize)]
pub struct ReferenceCounts {
    /// Mentions in text.
    pub inline: usize,
    /// Links declared in front-matter.
    pub declared: usize,
    /// `homoglyph` findings.
    pub homoglyph: usize,
    /// References through an `aliases_from` prefix, inline and declared.
    pub alias: usize,
}

#[derive(Serialize)]
pub struct TokenCounts {
    /// Sum over documents (a document costs the whole file).
    pub total: u64,
    /// Largest estimate of any node.
    pub max_node: u32,
}

/// One row of `files.json`.
#[derive(Serialize)]
struct FileRow<'a> {
    path: &'a str,
    bytes: usize,
    front_matter: bool,
    sections: usize,
    anchors: usize,
    links: usize,
    diagnostics: usize,
    tokens_est: u32,
}

/// One row of `diagnostics.json`.
#[derive(Serialize)]
struct DiagnosticRow<'a> {
    path: &'a str,
    line: usize,
    code: DiagnosticCode,
    message: &'a str,
}

/// One row of `sections_differ.json`.
#[derive(Serialize)]
struct DifferRow<'a> {
    path: &'a str,
    parser_only: Vec<usize>,
    census_only: Vec<usize>,
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<ParseResult, String> {
    let out_dir = corpus.out.join("parse").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
    let Setup { config, scheme } = setup;

    let started = Instant::now();
    let census = specengine_import::run(&corpus.root, &config)?;
    let mut census_sections: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for record in census
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Section)
    {
        census_sections
            .entry(record.path.as_str())
            .or_default()
            .insert(record.line);
    }

    let mut problems: Vec<String> = Vec::new();
    let paths = documents(&corpus.root, &config, &mut problems);
    let mut result = ParseResult {
        files: paths.len(),
        panics: 0,
        not_utf8: 0,
        front_matter: FrontMatterCounts {
            present: 0,
            diagnostics: DiagnosticCode::ALL
                .iter()
                .map(|code| (code.as_str(), 0))
                .collect(),
        },
        sections: SectionCounts {
            parsed: 0,
            census_id_sections: census.id_sections(),
            differ: 0,
        },
        heading_attrs_not_section: 0,
        references: ReferenceCounts {
            inline: 0,
            declared: 0,
            homoglyph: 0,
            alias: 0,
        },
        tokens_est: TokenCounts {
            total: 0,
            max_node: 0,
        },
    };
    let mut parsed_files: Vec<ParsedFile> = Vec::with_capacity(paths.len());
    let mut panicked: Vec<&str> = Vec::new();
    let mut differ_rows = Vec::new();
    let mut sizes: Vec<usize> = Vec::with_capacity(paths.len());

    for relative in &paths {
        let bytes = match fs::read(corpus.root.join(relative)) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(format!("{relative}: skipped: {error}"));
                continue;
            }
        };
        let parsed = match panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(relative, &bytes, &scheme)
        })) {
            Ok(parsed) => parsed,
            Err(_) => {
                result.panics += 1;
                panicked.push(relative);
                continue;
            }
        };

        if parsed.front_matter.is_some() {
            result.front_matter.present += 1;
        }
        for diagnostic in &parsed.diagnostics {
            *result
                .front_matter
                .diagnostics
                .entry(diagnostic.code.as_str())
                .or_default() += 1;
            match diagnostic.code {
                DiagnosticCode::NotUtf8 => result.not_utf8 += 1,
                DiagnosticCode::Homoglyph => result.references.homoglyph += 1,
                _ => {}
            }
        }
        result.sections.parsed += parsed.sections().len();
        result.heading_attrs_not_section += parsed.anchors.len();
        for link in &parsed.links {
            match link.origin {
                LinkOrigin::Inline => result.references.inline += 1,
                LinkOrigin::Frontmatter => result.references.declared += 1,
            }
            if let LinkTarget::Reference(reference) = &link.dst
                && reference.alias_of.is_some()
            {
                result.references.alias += 1;
            }
        }
        if let Some(document) = parsed.document() {
            result.tokens_est.total += u64::from(document.tokens_est);
        }
        for node in &parsed.nodes {
            result.tokens_est.max_node = result.tokens_est.max_node.max(node.tokens_est);
        }

        let parser_lines: BTreeSet<usize> = parsed
            .sections()
            .iter()
            .filter_map(|node| node.heading)
            .map(|heading| line_of(&bytes, heading.start))
            .collect();
        let empty = BTreeSet::new();
        let census_lines = census_sections.get(relative.as_str()).unwrap_or(&empty);
        let parser_only: Vec<usize> = parser_lines.difference(census_lines).copied().collect();
        let census_only: Vec<usize> = census_lines.difference(&parser_lines).copied().collect();
        if !parser_only.is_empty() || !census_only.is_empty() {
            result.sections.differ += parser_only.len() + census_only.len();
            differ_rows.push((relative.as_str(), parser_only, census_only));
        }
        sizes.push(bytes.len());
        parsed_files.push(parsed);
    }
    let parse_ms = started.elapsed().as_millis();

    let files: Vec<FileRow<'_>> = parsed_files
        .iter()
        .zip(&sizes)
        .map(|(parsed, &bytes)| FileRow {
            path: &parsed.path,
            bytes,
            front_matter: parsed.front_matter.is_some(),
            sections: parsed.sections().len(),
            anchors: parsed.anchors.len(),
            links: parsed.links.len(),
            diagnostics: parsed.diagnostics.len(),
            tokens_est: parsed.document().map_or(0, |document| document.tokens_est),
        })
        .collect();
    let diagnostics: Vec<DiagnosticRow<'_>> = parsed_files
        .iter()
        .flat_map(|parsed| {
            parsed.diagnostics.iter().map(|diagnostic| DiagnosticRow {
                path: &parsed.path,
                line: diagnostic.line,
                code: diagnostic.code,
                message: &diagnostic.message,
            })
        })
        .collect();
    let differ: Vec<DifferRow<'_>> = differ_rows
        .into_iter()
        .map(|(path, parser_only, census_only)| DifferRow {
            path,
            parser_only,
            census_only,
        })
        .collect();
    write_json(&out_dir.join("files.json"), &files)?;
    write_json(&out_dir.join("diagnostics.json"), &diagnostics)?;
    write_json(&out_dir.join("sections_differ.json"), &differ)?;
    write_json(&out_dir.join("panics.json"), &panicked)?;
    write_json(&out_dir.join("problems.json"), &problems)?;

    summarize(&result, &diagnostics, &corpus.label, &out_dir, parse_ms);
    Ok(result)
}

/// The census's documents: configured roots, dot-directories and symlinks
/// skipped, document extensions, excludes; sorted, corpus-relative, `/`.
fn documents(root: &Path, config: &CensusConfig, problems: &mut Vec<String>) -> Vec<String> {
    let mut documents = BTreeSet::new();
    for configured in &config.roots {
        let relative = relative_string(configured);
        let absolute = root.join(configured);
        match fs::metadata(&absolute) {
            Ok(meta) if meta.is_dir() => {
                walk(&absolute, &relative, config, &mut documents, problems)
            }
            Ok(_) => {
                let name = relative.rsplit('/').next().unwrap_or("");
                if config.is_document(name) && !config.is_excluded(&relative) {
                    documents.insert(relative);
                }
            }
            Err(error) => {
                problems.push(format!("{relative}: configured root not readable: {error}"))
            }
        }
    }
    documents.into_iter().collect()
}

fn walk(
    absolute: &Path,
    relative: &str,
    config: &CensusConfig,
    documents: &mut BTreeSet<String>,
    problems: &mut Vec<String>,
) {
    let entries = match fs::read_dir(absolute) {
        Ok(entries) => entries,
        Err(error) => {
            problems.push(format!("{relative}: directory skipped: {error}"));
            return;
        }
    };
    let mut children = Vec::new();
    for entry in entries {
        match entry.and_then(|entry| Ok((entry.file_name(), entry.file_type()?))) {
            Ok(child) => children.push(child),
            Err(error) => problems.push(format!("{relative}: directory entry skipped: {error}")),
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
                    problems,
                );
            }
        } else if config.is_document(&name) && !config.is_excluded(&child) {
            documents.insert(child);
        }
    }
}

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

/// 1-based line of a byte offset.
fn line_of(bytes: &[u8], offset: usize) -> usize {
    bytes[..offset.min(bytes.len())]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

fn summarize(
    result: &ParseResult,
    diagnostics: &[DiagnosticRow<'_>],
    label: &str,
    out_dir: &Path,
    parse_ms: u128,
) {
    eprintln!(
        "parse [{label}]: {} files, {} with front-matter, {} not UTF-8, {} panics ({parse_ms} ms)",
        result.files, result.front_matter.present, result.not_utf8, result.panics
    );
    eprintln!(
        "  sections {} (census {}, differ {}), heading anchors that are no ID {}",
        result.sections.parsed,
        result.sections.census_id_sections,
        result.sections.differ,
        result.heading_attrs_not_section
    );
    eprintln!(
        "  references: inline {}, declared {}, homoglyph {}, alias {}",
        result.references.inline,
        result.references.declared,
        result.references.homoglyph,
        result.references.alias
    );
    eprintln!(
        "  tokens_est: total {}, largest node {}",
        result.tokens_est.total, result.tokens_est.max_node
    );
    for diagnostic in diagnostics.iter().take(DIAGNOSTICS_ON_STDERR) {
        eprintln!(
            "  {}:{}: {}: {}",
            diagnostic.path, diagnostic.line, diagnostic.code, diagnostic.message
        );
    }
    if diagnostics.len() > DIAGNOSTICS_ON_STDERR {
        eprintln!(
            "  … {} more in diagnostics.json",
            diagnostics.len() - DIAGNOSTICS_ON_STDERR
        );
    }
    eprintln!("  detail: {}", out_dir.display());
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, json).map_err(|error| format!("cannot write {}: {error}", path.display()))
}
