//! Measurement `ast-hash`: stability of the 05 §5.2 recipe under (a) default
//! rustfmt, (b) contrasting rustfmt, (c) comment stripping, with parse-error
//! and `qpath` by-products. Per-file detail goes to `--out`, aggregates to stdout.

mod fmt;
#[cfg(feature = "syn")]
mod syn_cmp;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;
use specengine_code::comments::strip_comments;
use specengine_code::qpath::{self, Ambiguity, FileRole};
use specengine_code::{Digest, FileAnalysis, ItemRecord, RustParser};

use crate::harness::{self, Corpus, percent, stability_percent};

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "ast-hash";

/// The `result` object: the rows of the "Results" table plus `detail`.
#[derive(Serialize)]
pub struct AstHashResult {
    pub files: usize,
    pub items: usize,
    pub items_error: usize,
    pub items_error_pct: f64,
    pub files_with_errors: usize,
    pub stable_pct: StablePct,
    pub files_changed: FilesChanged,
    pub cannot_verify: usize,
    pub cannot_verify_distinct: bool,
    pub error_categories: Vec<&'static str>,
    pub qpath_ambiguous_pct: f64,
    pub path_attrs: usize,
    /// `null` unless built with feature `syn`.
    pub syn: Option<SynSummary>,
    pub detail: Detail,
}

/// Share of items hashed in the original whose hash survived the perturbation
/// unchanged. `null` for the rustfmt rows when no rustfmt was found.
#[derive(Serialize)]
pub struct StablePct {
    pub fmt_default: Option<f64>,
    pub fmt_contrast: Option<f64>,
    pub comments: f64,
}

#[derive(Serialize)]
pub struct FilesChanged {
    pub fmt_default: Option<usize>,
    pub fmt_contrast: Option<usize>,
}

#[derive(Serialize)]
pub struct SynSummary {
    pub files_failed: usize,
    pub naive_stable_pct: f64,
    pub normalized_stable_pct: f64,
    pub time_ratio: f64,
    pub naive_by_perturbation: BTreeMap<&'static str, f64>,
    pub normalized_by_perturbation: BTreeMap<&'static str, f64>,
}

/// Side information that explains the headline numbers. No names, no paths.
#[derive(Serialize)]
pub struct Detail {
    pub files_skipped: usize,
    pub orphan_error_regions: usize,
    /// Parse + hash time of the originals, tree-sitter.
    pub hash_ms: u128,
    pub rustfmt: RustfmtDetail,
    pub stability: BTreeMap<&'static str, Stability>,
    pub qpath_ambiguity: BTreeMap<&'static str, usize>,
}

#[derive(Serialize)]
pub struct RustfmtDetail {
    pub available: bool,
    pub version: Option<String>,
    pub contrast_config: &'static str,
    pub rejected_options: Vec<String>,
    pub failed_files: BTreeMap<&'static str, usize>,
}

#[derive(Serialize, Default, Clone, Copy)]
pub struct Stability {
    /// Items hashed in the original (the denominator).
    pub compared: usize,
    pub equal: usize,
    /// Both hashed, different digest.
    pub changed: usize,
    /// Hashed before, `cannot_verify` after.
    pub became_error: usize,
    /// No item with the same key after the perturbation.
    pub unmatched: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Perturbation {
    FmtDefault,
    FmtContrast,
    Comments,
}

impl Perturbation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FmtDefault => "fmt_default",
            Self::FmtContrast => "fmt_contrast",
            Self::Comments => "comments",
        }
    }
}

pub struct SourceFile {
    pub relative: PathBuf,
    pub text: String,
}

/// Identity of an item across perturbations: rustfmt may reorder `mod` and
/// `use` declarations, so the ordinal counts only among items with the same
/// scope, kind and label.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ItemKey {
    mod_path: Vec<String>,
    owner: Option<String>,
    kind: String,
    label: String,
    ordinal: usize,
}

struct Analyzed {
    analysis: FileAnalysis,
    keys: Vec<ItemKey>,
}

fn analyze(parser: &mut RustParser, text: &str) -> Result<Analyzed, String> {
    let analysis = specengine_code::analyze_file(parser, text)
        .ok_or_else(|| "tree-sitter cancelled a parse".to_owned())?;
    let keys = keys_of(&analysis.items);
    Ok(Analyzed { analysis, keys })
}

fn keys_of(items: &[ItemRecord]) -> Vec<ItemKey> {
    let mut seen: HashMap<(Vec<String>, Option<String>, String, String), usize> = HashMap::new();
    items
        .iter()
        .map(|item| {
            let slot = seen
                .entry((
                    item.mod_path.clone(),
                    item.owner.clone(),
                    item.kind.clone(),
                    item.label.clone(),
                ))
                .or_insert(0);
            let key = ItemKey {
                mod_path: item.mod_path.clone(),
                owner: item.owner.clone(),
                kind: item.kind.clone(),
                label: item.label.clone(),
                ordinal: *slot,
            };
            *slot += 1;
            key
        })
        .collect()
}

/// One row of `unstable.json`.
#[derive(Serialize)]
struct Unstable {
    path: String,
    line: usize,
    kind: String,
    label: String,
    perturbation: &'static str,
    before: String,
    after: String,
}

#[derive(Serialize)]
struct Manifest {
    label: String,
    files: Vec<ManifestFile>,
}

#[derive(Serialize)]
struct ManifestFile {
    path: String,
    package: String,
    role: String,
    has_error: bool,
    orphan_error_regions: usize,
    items: Vec<ManifestItem>,
}

#[derive(Serialize)]
struct ManifestItem {
    line: usize,
    kind: String,
    label: String,
    owner: Option<String>,
    qpath: String,
    ambiguity: Option<&'static str>,
    has_error: bool,
    hash: Option<String>,
    error_categories: Vec<&'static str>,
    perturbed: BTreeMap<&'static str, String>,
}

pub fn run(corpus: &Corpus) -> Result<AstHashResult, String> {
    let out_dir = corpus.out.join("ast-hash").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;
    let mut parser = RustParser::new().map_err(|error| format!("grammar: {error}"))?;

    // 1. Read the corpus.
    let paths = harness::rust_files(&corpus.root)
        .map_err(|error| format!("cannot list {}: {error}", corpus.root.display()))?;
    let mut files = Vec::with_capacity(paths.len());
    let mut files_skipped = 0;
    for relative in paths {
        let absolute = corpus.root.join(&relative);
        match fs::read(&absolute).map(String::from_utf8) {
            Ok(Ok(text)) => files.push(SourceFile { relative, text }),
            Ok(Err(_)) => {
                eprintln!("ast-hash: skipped {}: not UTF-8", relative.display());
                files_skipped += 1;
            }
            Err(error) => {
                eprintln!("ast-hash: skipped {}: {error}", relative.display());
                files_skipped += 1;
            }
        }
    }

    // 2. Originals, timed (the reference for the syn time ratio).
    let started = Instant::now();
    let originals = files
        .iter()
        .map(|file| analyze(&mut parser, &file.text))
        .collect::<Result<Vec<_>, _>>()?;
    let hash_us = started.elapsed().as_micros();

    // 3. Perturbed texts, on copies in memory; `None` = the file stays as it is.
    let rustfmt = fmt::Rustfmt::detect();
    if rustfmt.is_none() {
        eprintln!("ast-hash: no rustfmt found; the rustfmt rows are null");
    }
    let default_config = out_dir.join("rustfmt-default.toml");
    let contrast_config = out_dir.join("rustfmt-contrast.toml");
    fs::write(&default_config, fmt::DEFAULT_CONFIG)
        .and_then(|()| fs::write(&contrast_config, fmt::CONTRAST_CONFIG))
        .map_err(|error| format!("cannot write rustfmt configs: {error}"))?;
    let mut rejected_options = BTreeSet::new();
    let mut failed_files = BTreeMap::new();
    let mut perturbed: BTreeMap<Perturbation, Vec<Option<String>>> = BTreeMap::new();
    for (perturbation, config) in [
        (Perturbation::FmtDefault, &default_config),
        (Perturbation::FmtContrast, &contrast_config),
    ] {
        let Some(rustfmt) = &rustfmt else {
            continue;
        };
        let mut failed = 0;
        let texts = files
            .iter()
            .map(|file| match rustfmt.format(&file.text, config) {
                Ok(formatted) => {
                    rejected_options.extend(formatted.rejected_options);
                    Some(formatted.text)
                }
                Err(reason) => {
                    eprintln!(
                        "ast-hash: rustfmt ({}) failed on {}: {reason}",
                        perturbation.as_str(),
                        file.relative.display()
                    );
                    failed += 1;
                    None
                }
            })
            .collect();
        failed_files.insert(perturbation.as_str(), failed);
        perturbed.insert(perturbation, texts);
    }
    let stripped = files
        .iter()
        .map(|file| {
            parser
                .parse(&file.text)
                .map(|tree| strip_comments(&file.text, &tree))
        })
        .collect();
    perturbed.insert(Perturbation::Comments, stripped);

    // 4. Compare hashes across perturbations.
    let mut stability: BTreeMap<&'static str, Stability> = BTreeMap::new();
    let mut unstable: Vec<Unstable> = Vec::new();
    let mut perturbed_hashes: Vec<BTreeMap<&'static str, Vec<String>>> =
        files.iter().map(|_| BTreeMap::new()).collect();
    let mut files_changed: BTreeMap<Perturbation, usize> = BTreeMap::new();
    for (perturbation, texts) in &perturbed {
        let name = perturbation.as_str();
        let mut stat = Stability::default();
        let mut changed_files = 0;
        for (index, (file, original)) in files.iter().zip(&originals).enumerate() {
            let text = texts[index].as_deref();
            if text.is_some_and(|t| t != file.text) {
                changed_files += 1;
            }
            let after = match text {
                Some(text) => Some(analyze(&mut parser, text)?),
                None => None,
            };
            let after = after.as_ref().unwrap_or(original);
            let after_by_key: HashMap<&ItemKey, &ItemRecord> =
                after.keys.iter().zip(&after.analysis.items).collect();
            let mut per_item = Vec::with_capacity(original.analysis.items.len());
            let mut dump_sources = false;
            for (key, item) in original.keys.iter().zip(&original.analysis.items) {
                let Some(before) = item.state.digest() else {
                    per_item.push("cannot_verify".to_owned());
                    continue;
                };
                stat.compared += 1;
                let (after_text, verdict) = match after_by_key.get(key) {
                    None => ("unmatched".to_owned(), Some("unmatched")),
                    Some(after_item) => match after_item.state.digest() {
                        None => ("cannot_verify".to_owned(), Some("cannot_verify")),
                        Some(after) if after == before => (after.to_hex(), None),
                        Some(after) => (after.to_hex(), Some("changed")),
                    },
                };
                match verdict {
                    None => stat.equal += 1,
                    Some("unmatched") => stat.unmatched += 1,
                    Some("cannot_verify") => stat.became_error += 1,
                    Some(_) => stat.changed += 1,
                }
                if verdict.is_some() {
                    dump_sources = true;
                    unstable.push(Unstable {
                        path: relative_string(&file.relative),
                        line: item.line,
                        kind: item.kind.clone(),
                        label: item.label.clone(),
                        perturbation: name,
                        before: before.to_hex(),
                        after: after_text.clone(),
                    });
                }
                per_item.push(after_text);
            }
            perturbed_hashes[index].insert(name, per_item);
            if dump_sources && let Some(text) = text {
                dump_unstable_sources(&out_dir, &file.relative, &file.text, name, text);
            }
        }
        files_changed.insert(*perturbation, changed_files);
        stability.insert(name, stat);
    }

    // 5. qpath: package roots, file roles, `#[path]` targets, duplicates.
    let is_package_dir = |dir: &Path| corpus.root.join(dir).join("Cargo.toml").is_file();
    let mut path_targets: BTreeSet<PathBuf> = BTreeSet::new();
    let mut path_attrs = 0;
    for (file, original) in files.iter().zip(&originals) {
        for declaration in &original.analysis.mod_declarations {
            if let Some(value) = &declaration.path_attribute {
                path_attrs += 1;
                path_targets.insert(qpath::resolve_path_attribute(&file.relative, value));
            }
        }
    }
    let mut file_meta = Vec::with_capacity(files.len());
    let mut qpaths: Vec<Vec<String>> = Vec::with_capacity(files.len());
    let mut ambiguities: Vec<Vec<Option<Ambiguity>>> = Vec::with_capacity(files.len());
    let mut occurrences: HashMap<String, usize> = HashMap::new();
    for (file, original) in files.iter().zip(&originals) {
        let package = qpath::package_dir(&file.relative, is_package_dir);
        let package_label = package
            .as_deref()
            .map(relative_string)
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| ".".to_owned());
        let role = match &package {
            Some(package) => file
                .relative
                .strip_prefix(package)
                .map(qpath::file_role)
                .unwrap_or(FileRole::Unrooted),
            None => FileRole::Unrooted,
        };
        let is_path_target = path_targets.contains(&file.relative);
        let mut file_qpaths = Vec::with_capacity(original.analysis.items.len());
        let mut file_ambiguities = Vec::with_capacity(original.analysis.items.len());
        for item in &original.analysis.items {
            let path = qpath::qpath(&package_label, &role, item).to_string();
            *occurrences.entry(path.clone()).or_insert(0) += 1;
            file_qpaths.push(path);
            file_ambiguities.push(if is_path_target {
                Some(Ambiguity::PathAttribute)
            } else if role == FileRole::Unrooted {
                Some(Ambiguity::Unrooted)
            } else {
                None
            });
        }
        file_meta.push((package_label, role_string(&role)));
        qpaths.push(file_qpaths);
        ambiguities.push(file_ambiguities);
    }
    let mut ambiguity_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (file_qpaths, file_ambiguities) in qpaths.iter().zip(ambiguities.iter_mut()) {
        for (path, ambiguity) in file_qpaths.iter().zip(file_ambiguities.iter_mut()) {
            if ambiguity.is_none() && occurrences.get(path).copied().unwrap_or(0) > 1 {
                *ambiguity = Some(Ambiguity::Duplicate);
            }
            if let Some(reason) = ambiguity {
                *ambiguity_counts.entry(reason.as_str()).or_insert(0) += 1;
            }
        }
    }

    // 6. Aggregates.
    let items: usize = originals.iter().map(|o| o.analysis.items.len()).sum();
    let all_items = originals.iter().flat_map(|o| o.analysis.items.iter());
    let items_error = all_items.clone().filter(|item| item.has_error).count();
    let cannot_verify = all_items
        .clone()
        .filter(|item| item.state.digest().is_none())
        .count();
    let errored_digests: Vec<Digest> = all_items
        .clone()
        .filter(|item| item.has_error)
        .filter_map(|item| item.state.digest())
        .collect();
    let cannot_verify_distinct =
        errored_digests.iter().collect::<BTreeSet<_>>().len() == errored_digests.len();
    let error_categories: Vec<&'static str> = all_items
        .clone()
        .filter_map(|item| match &item.state {
            specengine_code::HashState::CannotVerify { categories } => Some(categories),
            specengine_code::HashState::Hashed(_) => None,
        })
        .flatten()
        .map(|category| category.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let files_with_errors = originals.iter().filter(|o| o.analysis.has_error).count();
    let orphan_error_regions = originals.iter().map(|o| o.analysis.orphan_errors).sum();
    let ambiguous: usize = ambiguity_counts.values().sum();
    let stable = |perturbation: Perturbation| {
        stability
            .get(perturbation.as_str())
            .map(|s| stability_percent(s.equal, s.compared))
    };
    let changed = |perturbation: Perturbation| files_changed.get(&perturbation).copied();

    // 7. syn comparison (feature `syn`).
    #[cfg(feature = "syn")]
    let syn = Some(syn_cmp::run(&files, &perturbed, hash_us));
    #[cfg(not(feature = "syn"))]
    let syn = None;

    // 8. Per-file detail into --out.
    let manifest = Manifest {
        label: corpus.label.clone(),
        files: files
            .iter()
            .enumerate()
            .map(|(index, file)| {
                let original = &originals[index];
                let (package, role) = &file_meta[index];
                ManifestFile {
                    path: relative_string(&file.relative),
                    package: package.clone(),
                    role: role.clone(),
                    has_error: original.analysis.has_error,
                    orphan_error_regions: original.analysis.orphan_errors,
                    items: original
                        .analysis
                        .items
                        .iter()
                        .enumerate()
                        .map(|(item_index, item)| ManifestItem {
                            line: item.line,
                            kind: item.kind.clone(),
                            label: item.label.clone(),
                            owner: item.owner.clone(),
                            qpath: qpaths[index][item_index].clone(),
                            ambiguity: ambiguities[index][item_index].map(Ambiguity::as_str),
                            has_error: item.has_error,
                            hash: item.state.digest().map(|d| d.to_hex()),
                            error_categories: match &item.state {
                                specengine_code::HashState::CannotVerify { categories } => {
                                    categories.iter().map(|c| c.as_str()).collect()
                                }
                                specengine_code::HashState::Hashed(_) => Vec::new(),
                            },
                            perturbed: perturbed_hashes[index]
                                .iter()
                                .filter_map(|(name, hashes)| {
                                    hashes.get(item_index).map(|h| (*name, h.clone()))
                                })
                                .collect(),
                        })
                        .collect(),
                }
            })
            .collect(),
    };
    write_json(&out_dir.join("manifest.json"), &manifest)?;
    write_json(&out_dir.join("unstable.json"), &unstable)?;

    let result = AstHashResult {
        files: files.len(),
        items,
        items_error,
        items_error_pct: percent(items_error, items),
        files_with_errors,
        stable_pct: StablePct {
            fmt_default: stable(Perturbation::FmtDefault),
            fmt_contrast: stable(Perturbation::FmtContrast),
            comments: stable(Perturbation::Comments).unwrap_or(100.0),
        },
        files_changed: FilesChanged {
            fmt_default: changed(Perturbation::FmtDefault),
            fmt_contrast: changed(Perturbation::FmtContrast),
        },
        cannot_verify,
        cannot_verify_distinct,
        error_categories,
        qpath_ambiguous_pct: percent(ambiguous, items),
        path_attrs,
        syn,
        detail: Detail {
            files_skipped,
            orphan_error_regions,
            hash_ms: hash_us / 1000,
            rustfmt: RustfmtDetail {
                available: rustfmt.is_some(),
                version: rustfmt.as_ref().map(|r| r.version.clone()),
                contrast_config: fmt::CONTRAST_CONFIG,
                rejected_options: rejected_options.into_iter().collect(),
                failed_files,
            },
            stability,
            qpath_ambiguity: ambiguity_counts,
        },
    };

    // 9. Human summary.
    summarize(&result, &corpus.label, &out_dir);
    Ok(result)
}

fn summarize(result: &AstHashResult, label: &str, out_dir: &Path) {
    let opt_pct = |value: Option<f64>| value.map_or("n/a".to_owned(), |v| format!("{v:.1} %"));
    let opt_n = |value: Option<usize>| value.map_or("n/a".to_owned(), |v| v.to_string());
    eprintln!(
        "ast-hash [{label}]: {} files, {} items, {} with parse errors ({:.1} %), {} files with errors",
        result.files,
        result.items,
        result.items_error,
        result.items_error_pct,
        result.files_with_errors
    );
    eprintln!(
        "  stable: fmt_default {} ({} files changed), fmt_contrast {} ({} files changed), comments {:.1} %",
        opt_pct(result.stable_pct.fmt_default),
        opt_n(result.files_changed.fmt_default),
        opt_pct(result.stable_pct.fmt_contrast),
        opt_n(result.files_changed.fmt_contrast),
        result.stable_pct.comments
    );
    eprintln!(
        "  cannot_verify {} (distinct: {}), categories: {}",
        result.cannot_verify,
        if result.cannot_verify_distinct {
            "yes"
        } else {
            "no"
        },
        if result.error_categories.is_empty() {
            "none".to_owned()
        } else {
            result.error_categories.join(", ")
        }
    );
    eprintln!(
        "  qpath ambiguous {:.1} %, #[path] attributes {}",
        result.qpath_ambiguous_pct, result.path_attrs
    );
    match &result.syn {
        Some(syn) => eprintln!(
            "  syn: {} files failed, naive {:.1} %, normalized {:.1} %, time ratio {:.2}",
            syn.files_failed, syn.naive_stable_pct, syn.normalized_stable_pct, syn.time_ratio
        ),
        None => eprintln!("  syn: not built (feature `syn`)"),
    }
    if !result.detail.rustfmt.rejected_options.is_empty() {
        eprintln!(
            "  rustfmt rejected options: {}",
            result.detail.rustfmt.rejected_options.join(", ")
        );
    }
    eprintln!("  detail: {}", out_dir.display());
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, json).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Original and perturbed text of a file with an unstable item, for reading the diff.
fn dump_unstable_sources(out_dir: &Path, relative: &Path, original: &str, name: &str, after: &str) {
    let dir = out_dir.join("unstable");
    if let Err(error) = fs::create_dir_all(&dir) {
        eprintln!("ast-hash: cannot create {}: {error}", dir.display());
        return;
    }
    let stem = relative_string(relative).replace('/', "__");
    for (suffix, text) in [("original", original), (name, after)] {
        let path = dir.join(format!("{stem}.{suffix}.rs"));
        if let Err(error) = fs::write(&path, text) {
            eprintln!("ast-hash: cannot write {}: {error}", path.display());
        }
    }
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn role_string(role: &FileRole) -> String {
    match role {
        FileRole::CrateRoot => "crate_root".to_owned(),
        FileRole::Module(path) => format!("module:{}", path.join("::")),
        FileRole::Unrooted => "unrooted".to_owned(),
    }
}
