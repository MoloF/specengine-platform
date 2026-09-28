//! Measurement `bevy-detector` (spike group 5 `bevy-schedule`): the syntactic
//! Bevy registration detector of `specengine-code` over a corpus, and —
//! with `--dump <app_data.ron>` — its comparison against the schedule dump of
//! an instrumented build (05 §5.1 layer B).
//!
//! stdout: counts only. Names, paths and texts go to
//! `--out/bevy/<label>/`: `registrations.json`, `plugins.json`,
//! `plugin_uses.json`, `uncertain.json`, `crates.json`, and with a dump
//! `dump_match.json` and `dump_schema.json`.
//!
//! The corpus's own crates (the comparison base) are the `[package]`,
//! `[lib]` and `[[bin]]` names of its `Cargo.toml` files, `-` read as `_`.

mod compare;
mod dump;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use specengine_code::RustParser;
use specengine_code::bevy::{self, BevyAnalysis, Origin, PluginKind, Target};

use crate::harness::{self, Corpus};

pub use compare::DumpSummary;
pub use dump::Dump;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "bevy-mini";

/// Keys of `systems_by` / `observers_by`: code forms, then macro origins.
const LEAF_KEYS: &[&str] = &["path", "closure", "factory", "macro_rules", "macro_call"];

/// Reads and parses `--dump` before anything is written; an error names the
/// file and line and is a refusal (exit 2).
pub fn load_dump(path: Option<&Path>) -> Result<Option<Dump>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text =
        fs::read_to_string(path).map_err(|error| format!("--dump {}: {error}", path.display()))?;
    dump::parse(&text)
        .map(Some)
        .map_err(|error| format!("{}:{}: {}", path.display(), error.line, error.message))
}

/// The `result` object: the rows of the "Results" table plus `detail`.
#[derive(Serialize)]
pub struct BevyResult {
    pub files: usize,
    pub files_with_errors: usize,
    pub detected: Detected,
    /// `null` without `--dump`.
    pub dump: Option<DumpSummary>,
    pub detail: Detail,
}

/// Every origin counted: code and macro tokens.
#[derive(Serialize)]
pub struct Detected {
    pub systems: usize,
    pub observers: usize,
    pub plugins: usize,
}

/// Side information. No names, no paths.
#[derive(Serialize)]
pub struct Detail {
    pub files_skipped: usize,
    pub bytes: usize,
    /// Crate names found in the corpus's manifests.
    pub crates: usize,
    pub manifests_unreadable: usize,
    pub call_sites: CallSites,
    /// `path` / `closure` / `factory` from code, `macro_rules` / `macro_call` from tokens.
    pub systems_by: BTreeMap<&'static str, usize>,
    pub observers_by: BTreeMap<&'static str, usize>,
    /// `impl_plugin`, `fn_app`, and `in_macros` of both.
    pub plugins_by: BTreeMap<&'static str, usize>,
    pub plugin_uses: usize,
    /// Adapters on system and observer registrations.
    pub adapters: BTreeMap<&'static str, usize>,
    /// Distinct schedule labels of system registrations (as `Debug` prints them).
    pub schedules_distinct: usize,
    /// System registrations whose label is not a literal type or variant.
    pub schedule_not_literal: usize,
    /// Constructs the detector could not read, per category (zeros included).
    pub uncertain: BTreeMap<&'static str, usize>,
    pub detector_ms: u128,
}

#[derive(Serialize)]
pub struct CallSites {
    pub add_systems: usize,
    pub add_observer: usize,
    pub add_plugins: usize,
    pub in_macros: usize,
}

const UNCERTAIN_KEYS: &[&str] = &[
    "macro_in_arguments",
    "metavariable",
    "unknown_method",
    "expression",
    "arguments",
    "nesting_too_deep",
    "parse_error",
];

#[derive(Serialize)]
struct RegistrationRow<'a> {
    path: &'a str,
    line: usize,
    target: &'static str,
    origin: &'static str,
    form: &'static str,
    schedule: Option<&'a str>,
    text: &'a str,
    name: Option<&'a str>,
    enclosing_fn: Option<&'a str>,
    adapters: &'a [&'static str],
    piped: &'a [String],
}

#[derive(Serialize)]
struct PluginRow<'a> {
    path: &'a str,
    line: usize,
    kind: &'static str,
    origin: &'static str,
    name: Option<&'a str>,
}

#[derive(Serialize)]
struct PluginUseRow<'a> {
    path: &'a str,
    line: usize,
    origin: &'static str,
    name: Option<&'a str>,
    text: &'a str,
}

#[derive(Serialize)]
struct UncertainRow<'a> {
    path: &'a str,
    line: usize,
    category: &'static str,
    origin: &'static str,
    call: &'static str,
    text: &'a str,
}

#[derive(Serialize)]
struct DumpSchemaDetail<'a> {
    unknown_fields: &'a BTreeSet<String>,
    missing_fields: &'a BTreeSet<String>,
}

struct Analyzed {
    path: String,
    text: String,
    analysis: BevyAnalysis,
}

pub fn run(corpus: &Corpus, dump: Option<Dump>) -> Result<BevyResult, String> {
    let out_dir = corpus.out.join("bevy").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;

    // 1. The corpus's own crates.
    let (crates, manifests_unreadable) = crate_names(&corpus.root)?;

    // 2. Read and detect.
    let paths = harness::rust_files(&corpus.root)
        .map_err(|error| format!("cannot list {}: {error}", corpus.root.display()))?;
    let mut parser = RustParser::new().map_err(|error| format!("grammar: {error}"))?;
    let mut files = Vec::with_capacity(paths.len());
    let mut files_skipped = 0;
    let started = Instant::now();
    for relative in paths {
        let text = match fs::read(corpus.root.join(&relative)).map(String::from_utf8) {
            Ok(Ok(text)) => text,
            Ok(Err(_)) => {
                eprintln!("bevy-detector: skipped {}: not UTF-8", relative.display());
                files_skipped += 1;
                continue;
            }
            Err(error) => {
                eprintln!("bevy-detector: skipped {}: {error}", relative.display());
                files_skipped += 1;
                continue;
            }
        };
        let Some(analysis) = bevy::detect_file(&mut parser, &text) else {
            eprintln!(
                "bevy-detector: skipped {}: parse cancelled",
                relative.display()
            );
            files_skipped += 1;
            continue;
        };
        files.push(Analyzed {
            path: relative_string(&relative),
            text,
            analysis,
        });
    }
    let detector_ms = started.elapsed().as_millis();

    // 3. Aggregates.
    let (detected, detail) = aggregate(
        &files,
        files_skipped,
        crates.len(),
        manifests_unreadable,
        detector_ms,
    );

    // 4. The comparison.
    let dump_summary = match &dump {
        Some(dump) => {
            let located: Vec<compare::Located> = files
                .iter()
                .flat_map(|file| {
                    file.analysis
                        .registrations
                        .iter()
                        .map(|registration| compare::Located {
                            file: &file.path,
                            registration,
                        })
                })
                .collect();
            let words = words(files.iter().map(|f| f.text.as_str()));
            let (summary, rows) = compare::compare(dump, &located, &crates, &words);
            write_json(&out_dir.join("dump_match.json"), &rows)?;
            write_json(
                &out_dir.join("dump_schema.json"),
                &DumpSchemaDetail {
                    unknown_fields: &dump.unknown_fields,
                    missing_fields: &dump.missing_fields,
                },
            )?;
            Some(summary)
        }
        None => None,
    };

    // 5. Per-file detail to --out.
    write_detail(&out_dir, &files, &crates)?;

    let result = BevyResult {
        files: files.len(),
        files_with_errors: files.iter().filter(|f| f.analysis.has_error).count(),
        detected,
        dump: dump_summary,
        detail,
    };
    summarize(&result, &corpus.label, &out_dir);
    Ok(result)
}

fn aggregate(
    files: &[Analyzed],
    files_skipped: usize,
    crates: usize,
    manifests_unreadable: usize,
    detector_ms: u128,
) -> (Detected, Detail) {
    let zeros = |keys: &[&'static str]| -> BTreeMap<&'static str, usize> {
        keys.iter().map(|k| (*k, 0)).collect()
    };
    let mut systems_by = zeros(LEAF_KEYS);
    let mut observers_by = zeros(LEAF_KEYS);
    let mut plugins_by = zeros(&["impl_plugin", "fn_app", "in_macros"]);
    let mut adapters = zeros(bevy::ADAPTERS);
    let mut uncertain = zeros(UNCERTAIN_KEYS);
    let mut call_sites = CallSites {
        add_systems: 0,
        add_observer: 0,
        add_plugins: 0,
        in_macros: 0,
    };
    let mut schedules = BTreeSet::new();
    let mut schedule_not_literal = 0;
    let mut plugin_uses = 0;
    let (mut systems, mut observers, mut plugins) = (0, 0, 0);
    for file in files {
        let analysis = &file.analysis;
        call_sites.add_systems += analysis.call_sites.add_systems;
        call_sites.add_observer += analysis.call_sites.add_observer;
        call_sites.add_plugins += analysis.call_sites.add_plugins;
        call_sites.in_macros += analysis.call_sites.in_macros;
        for registration in &analysis.registrations {
            let key = match registration.origin {
                Origin::Code => registration.form.as_str(),
                other => other.as_str(),
            };
            let by = match registration.target {
                Target::System => {
                    systems += 1;
                    if registration.origin == Origin::Code {
                        match registration
                            .schedule
                            .as_deref()
                            .map(compare::normalize_label)
                        {
                            Some(label) if label.starts_with(|c: char| c.is_ascii_uppercase()) => {
                                schedules.insert(label);
                            }
                            _ => schedule_not_literal += 1,
                        }
                    }
                    &mut systems_by
                }
                Target::Observer => {
                    observers += 1;
                    &mut observers_by
                }
            };
            *by.entry(key).or_default() += 1;
            for adapter in &registration.adapters {
                *adapters.entry(adapter).or_default() += 1;
            }
        }
        for plugin in &analysis.plugins {
            plugins += 1;
            let key = match plugin.kind {
                PluginKind::ImplPlugin => "impl_plugin",
                PluginKind::FnApp => "fn_app",
            };
            *plugins_by.entry(key).or_default() += 1;
            if plugin.origin != Origin::Code {
                *plugins_by.entry("in_macros").or_default() += 1;
            }
        }
        plugin_uses += analysis.plugin_uses.len();
        for item in &analysis.uncertain {
            *uncertain.entry(item.category.as_str()).or_default() += 1;
        }
    }
    let detected = Detected {
        systems,
        observers,
        plugins,
    };
    let detail = Detail {
        files_skipped,
        bytes: files.iter().map(|f| f.text.len()).sum(),
        crates,
        manifests_unreadable,
        call_sites,
        systems_by,
        observers_by,
        plugins_by,
        plugin_uses,
        adapters,
        schedules_distinct: schedules.len(),
        schedule_not_literal,
        uncertain,
        detector_ms,
    };
    (detected, detail)
}

fn write_detail(
    out_dir: &Path,
    files: &[Analyzed],
    crates: &BTreeSet<String>,
) -> Result<(), String> {
    let mut registrations = Vec::new();
    let mut plugins = Vec::new();
    let mut plugin_uses = Vec::new();
    let mut uncertain = Vec::new();
    for file in files {
        let path = file.path.as_str();
        let analysis = &file.analysis;
        registrations.extend(analysis.registrations.iter().map(|r| RegistrationRow {
            path,
            line: r.line,
            target: r.target.as_str(),
            origin: r.origin.as_str(),
            form: r.form.as_str(),
            schedule: r.schedule.as_deref(),
            text: &r.text,
            name: r.name.as_deref(),
            enclosing_fn: r.enclosing_fn.as_deref(),
            adapters: &r.adapters,
            piped: &r.piped,
        }));
        plugins.extend(analysis.plugins.iter().map(|p| PluginRow {
            path,
            line: p.line,
            kind: p.kind.as_str(),
            origin: p.origin.as_str(),
            name: p.name.as_deref(),
        }));
        plugin_uses.extend(analysis.plugin_uses.iter().map(|u| PluginUseRow {
            path,
            line: u.line,
            origin: u.origin.as_str(),
            name: u.name.as_deref(),
            text: &u.text,
        }));
        uncertain.extend(analysis.uncertain.iter().map(|u| UncertainRow {
            path,
            line: u.line,
            category: u.category.as_str(),
            origin: u.origin.as_str(),
            call: u.call,
            text: &u.text,
        }));
    }
    write_json(&out_dir.join("registrations.json"), &registrations)?;
    write_json(&out_dir.join("plugins.json"), &plugins)?;
    write_json(&out_dir.join("plugin_uses.json"), &plugin_uses)?;
    write_json(&out_dir.join("uncertain.json"), &uncertain)?;
    write_json(&out_dir.join("crates.json"), crates)
}

/// The parts of a manifest that name crates; everything else is ignored.
#[derive(Deserialize)]
struct Manifest {
    package: Option<Named>,
    lib: Option<Named>,
    #[serde(default)]
    bin: Vec<Named>,
}

#[derive(Deserialize)]
struct Named {
    name: Option<String>,
}

/// Crate names of every `Cargo.toml` under `root`, underscored, and the number
/// of manifests that could not be read or parsed (reported on stderr, never fatal).
fn crate_names(root: &Path) -> Result<(BTreeSet<String>, usize), String> {
    let manifests = harness::cargo_manifests(root)
        .map_err(|error| format!("cannot list {}: {error}", root.display()))?;
    let mut names = BTreeSet::new();
    let mut unreadable = 0;
    for relative in manifests {
        let path = root.join(&relative);
        let manifest: Manifest = match fs::read_to_string(&path)
            .map_err(|error| error.to_string())
            .and_then(|text| toml::from_str(&text).map_err(|error| error.to_string()))
        {
            Ok(manifest) => manifest,
            Err(error) => {
                eprintln!(
                    "bevy-detector: skipped {}: {}",
                    relative.display(),
                    error.trim()
                );
                unreadable += 1;
                continue;
            }
        };
        let named = manifest
            .package
            .iter()
            .chain(manifest.lib.iter())
            .chain(manifest.bin.iter());
        for Named { name } in named {
            if let Some(name) = name {
                names.insert(name.replace('-', "_"));
            }
        }
    }
    Ok((names, unreadable))
}

/// Every identifier-like word (ASCII letters, digits, `_`) of `texts`.
fn words<'a>(texts: impl Iterator<Item = &'a str>) -> HashSet<&'a str> {
    let mut out = HashSet::new();
    for text in texts {
        out.extend(
            text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty()),
        );
    }
    out
}

fn summarize(result: &BevyResult, label: &str, out_dir: &Path) {
    eprintln!(
        "bevy-detector [{label}]: {} files ({} with parse errors, {} skipped), {} crates",
        result.files, result.files_with_errors, result.detail.files_skipped, result.detail.crates
    );
    eprintln!(
        "  detected: {} systems, {} observers, {} plugins; {} plugin uses",
        result.detected.systems,
        result.detected.observers,
        result.detected.plugins,
        result.detail.plugin_uses
    );
    let uncertain: usize = result.detail.uncertain.values().sum();
    eprintln!("  uncertain constructs: {uncertain}");
    if let Some(dump) = &result.dump {
        eprintln!(
            "  dump: {} schedules, {} systems ({} of the corpus's crates); matched {} ({:.1} %: {} exact, {} schedule unverified)",
            dump.schedules,
            dump.systems_total,
            dump.systems,
            dump.matched,
            dump.match_pct,
            dump.matched_schedule_exact,
            dump.matched_schedule_unverified
        );
        let misses: Vec<String> = dump
            .miss_categories
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(k, n)| format!("{k} {n}"))
            .collect();
        eprintln!(
            "  misses: {} ({}); name found {:.1} %, with macro names {:.1} %",
            dump.misses,
            if misses.is_empty() {
                "none".to_owned()
            } else {
                misses.join(", ")
            },
            dump.name_found_pct,
            dump.name_found_incl_macros_pct
        );
    }
    eprintln!("  detail: {}", out_dir.display());
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    fs::write(path, json).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn relative_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
