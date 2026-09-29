//! Measurement `check`: `spec check`, increment 1, over a corpus
//! (docs/canon/spec-check.md; its flags in this crate's README): the
//! fresh-parse loader of `specengine-store` and the check of
//! `specengine-core`, read-only.
//!
//! The configuration (`[ids]`, `[paths]`, `[budgets]`, `[classes]`,
//! `[check]`) comes from `--scheme` (default: `SPECENGINE_SCHEME_A` / `_B`
//! for `--label pilot-a` / `pilot-b` when set, else `specengine.toml` at the
//! corpus root); the baseline from `--baseline`, else `.spec-debt.toml` at
//! the corpus root when present; today from `--today`, else the UTC date.
//! A missing or invalid config, baseline or date refuses the run (exit 2,
//! nothing written).
//!
//! stdout carries counts only: files, the verdict under each mode, per code
//! the errors, warnings and debt (`class-missing` counts the documents
//! without a class), expired and stale debt. Paths, IDs and messages go
//! only to
//! `--out/check/<label>/findings.json`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use specengine_core::check::{self, Baseline, CheckConfig, Mode, Report, Verdict};
use specengine_core::{IdSchemeToml, Paths};
use specengine_model::{IdScheme, Severity};
use specengine_store::{BASELINE_FILE, WorkingTree, check_input, today_utc};

use crate::harness::Corpus;

/// Fixture directory (relative to `fixtures/`) used without `--pilot`.
pub const FIXTURE: &str = "spec-a";

/// Config file looked up at the corpus root when `--scheme` is absent.
pub const DEFAULT_SCHEME: &str = "specengine.toml";

const ENV_SCHEME_A: &str = "SPECENGINE_SCHEME_A";
const ENV_SCHEME_B: &str = "SPECENGINE_SCHEME_B";

/// What the run needs, read before anything is written.
pub struct Setup {
    scheme: IdScheme,
    paths: Paths,
    config: CheckConfig,
    baseline: Baseline,
    today: String,
}

/// Reads the config, the baseline and the date; any error refuses the run.
pub fn prepare(
    root: &Path,
    scheme: Option<&Path>,
    baseline: Option<&Path>,
    today: Option<&str>,
    label: Option<&str>,
) -> Result<Setup, String> {
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
        .map_err(|error| format!("{shown}: cannot read the config: {error}"))?;
    let scheme = IdScheme::from_toml(&text).map_err(|error| error.at(&shown))?;
    let paths = Paths::from_toml(&text).map_err(|error| error.at(&shown))?;
    let config = CheckConfig::from_toml(&text).map_err(|error| error.at(&shown))?;

    let baseline_path = match baseline {
        Some(path) => Some(path.to_path_buf()),
        None => Some(root.join(BASELINE_FILE)).filter(|path| path.is_file()),
    };
    let baseline = match baseline_path {
        None => Baseline::empty(),
        Some(path) => {
            let shown = path.display().to_string();
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("{shown}: cannot read the baseline: {error}"))?;
            Baseline::from_toml(&text).map_err(|error| error.at(&shown))?
        }
    };
    let today = match today {
        Some(today) if check::is_calendar_date(today) => today.to_owned(),
        Some(today) => return Err(format!("--today {today:?} is not a YYYY-MM-DD date")),
        None => today_utc(),
    };
    Ok(Setup {
        scheme,
        paths,
        config,
        baseline,
        today,
    })
}

/// The `result` object.
#[derive(Serialize)]
pub struct CheckResult {
    pub files: usize,
    pub verdicts: Verdicts,
    /// Code → findings by kind; a finding in live debt counts as `debt`,
    /// one whose debt expired by its severity.
    pub codes: BTreeMap<String, CodeCounts>,
    pub expired: usize,
    pub stale: usize,
}

#[derive(Serialize)]
pub struct Verdicts {
    pub observe: Verdict,
    pub enforce: Verdict,
}

#[derive(Serialize, Default)]
pub struct CodeCounts {
    pub error: usize,
    pub warning: usize,
    pub debt: usize,
}

pub fn run(corpus: &Corpus, setup: Setup) -> Result<CheckResult, String> {
    let Setup {
        scheme,
        paths,
        config,
        baseline,
        today,
    } = setup;
    let out_dir = corpus.out.join("check").join(&corpus.label);
    fs::create_dir_all(&out_dir)
        .map_err(|error| format!("cannot create {}: {error}", out_dir.display()))?;

    let tree = WorkingTree::new(&corpus.root, &paths).map_err(|error| error.to_string())?;
    let input = check_input(&tree, &scheme);
    let report = check::run(&input, &scheme, &paths, &config, &baseline, &today);

    let detail = out_dir.join("findings.json");
    fs::write(&detail, report.to_json())
        .map_err(|error| format!("cannot write {}: {error}", detail.display()))?;
    let result = summarise(&report);
    eprintln!(
        "check: {} files, observe {}, enforce {}; detail in {}",
        result.files,
        result.verdicts.observe.as_str(),
        result.verdicts.enforce.as_str(),
        out_dir.display()
    );
    Ok(result)
}

fn summarise(report: &Report) -> CheckResult {
    let mut codes: BTreeMap<String, CodeCounts> = BTreeMap::new();
    for finding in &report.findings {
        let counts = codes.entry(finding.code.clone()).or_default();
        if finding.is_live_debt() {
            counts.debt += 1;
        } else {
            match finding.severity {
                Severity::Error => counts.error += 1,
                Severity::Warning => counts.warning += 1,
            }
        }
    }
    CheckResult {
        files: report.counts.documents,
        verdicts: Verdicts {
            observe: report.verdict_in(Mode::Observe),
            enforce: report.verdict_in(Mode::Enforce),
        },
        codes,
        expired: report.counts.expired,
        stale: report.counts.stale,
    }
}
