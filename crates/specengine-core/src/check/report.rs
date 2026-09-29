//! What a check found: findings, stale debt, the causes that stop it from
//! vouching for the corpus, counts and the verdict; rendered as a few lines
//! or as JSON, both deterministic (everything sorted).

use serde::Serialize;
use specengine_model::{Severity, Span};

use super::baseline::DebtEntry;
use super::config::Mode;

/// One finding about one file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Finding {
    /// A check code (`key-missing`, `ref-dangling`, …) or a parser code.
    pub code: String,
    pub severity: Severity,
    /// Root-relative path of the file; `""` for no file.
    pub path: String,
    /// 1-based; 1 for the file as a whole.
    pub line: usize,
    /// The object as written: an ID, a key, a `canon:` value, a budget slot
    /// (`tier0`, `tier1`, `index`, `decision`, `canon`), a parser
    /// diagnostic's span text; `""` for the file. Baseline entries match on
    /// it, never on the line.
    pub subject: String,
    pub message: String,
    /// The replacement for `span` (the Latin ID of a homoglyph): data for
    /// `apply_proposal`; the check never writes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
    /// The baseline entry the finding matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debt: Option<Debt>,
}

impl Finding {
    /// Blocks under `enforce`: an error not in live debt (expired debt
    /// blocks).
    pub fn blocks_when_enforced(&self) -> bool {
        self.severity == Severity::Error && self.debt.as_ref().is_none_or(|debt| debt.expired)
    }

    /// In debt that has not expired.
    pub fn is_live_debt(&self) -> bool {
        self.debt.as_ref().is_some_and(|debt| !debt.expired)
    }
}

/// A replacement of the bytes under `span` by `text`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Fix {
    pub span: Span,
    pub text: String,
}

/// The matched baseline entry's reason and expiry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Debt {
    pub reason: String,
    pub expires: String,
    /// `today > expires`: an error blocks again; a warning stays a warning.
    pub expired: bool,
}

/// Why the check cannot vouch for the corpus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Cause {
    /// The file or directory concerned; `""` for none.
    pub path: String,
    pub message: String,
}

/// The outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Nothing blocks (warnings, debt and stale entries never do).
    Clean,
    /// `observe`: errors that `enforce` would block on.
    Observed,
    /// `enforce`: an error not in live debt.
    Blocked,
    /// A missing root, an unreadable file or directory, an invalid config or
    /// baseline, whatever the mode.
    CannotCheck,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Observed => "observed",
            Self::Blocked => "blocked",
            Self::CannotCheck => "cannot-check",
        }
    }

    /// 0 clean or observed, 1 blocked, 2 cannot check.
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Clean | Self::Observed => 0,
            Self::Blocked => 1,
            Self::CannotCheck => 2,
        }
    }
}

/// The summary counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// Files walked, read or not.
    pub documents: usize,
    /// Errors not in live debt (expired debt included).
    pub errors: usize,
    /// Warnings not in live debt (expired debt included).
    pub warnings: usize,
    /// Findings in live debt.
    pub debt: usize,
    /// Errors whose debt expired (also counted in `errors`).
    pub expired: usize,
    /// Baseline entries that matched nothing; never counted as warnings.
    pub stale: usize,
}

/// One run of the check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub mode: Mode,
    pub verdict: Verdict,
    pub counts: Counts,
    /// Sorted by (path, line, code, subject, message).
    pub findings: Vec<Finding>,
    /// Baseline entries that matched no finding, sorted: no finding, shown
    /// as `debt-stale` only in the detail lines.
    pub stale: Vec<DebtEntry>,
    /// Sorted; non-empty exactly when the verdict is `cannot-check`.
    pub cannot_check: Vec<Cause>,
}

impl Report {
    /// A report of a check that could not start (no root, an invalid config
    /// or baseline): no documents, only causes.
    pub fn cannot(mode: Mode, causes: Vec<Cause>) -> Self {
        Self::assemble(mode, 0, Vec::new(), Vec::new(), causes)
    }

    /// Sorts, counts and judges.
    pub(crate) fn assemble(
        mode: Mode,
        documents: usize,
        mut findings: Vec<Finding>,
        mut stale: Vec<DebtEntry>,
        mut cannot_check: Vec<Cause>,
    ) -> Self {
        findings.sort_by(|a, b| {
            (&a.path, a.line, &a.code, &a.subject, &a.message)
                .cmp(&(&b.path, b.line, &b.code, &b.subject, &b.message))
                .then_with(|| a.cmp(b))
        });
        findings.dedup();
        stale.sort_by(|a, b| {
            (&a.path, &a.code, &a.subject, a.line).cmp(&(&b.path, &b.code, &b.subject, b.line))
        });
        cannot_check.sort();
        cannot_check.dedup();
        let mut counts = Counts {
            documents,
            stale: stale.len(),
            ..Counts::default()
        };
        for finding in &findings {
            if finding.is_live_debt() {
                counts.debt += 1;
                continue;
            }
            match finding.severity {
                Severity::Error => {
                    counts.errors += 1;
                    if finding.debt.is_some() {
                        counts.expired += 1;
                    }
                }
                Severity::Warning => counts.warnings += 1,
            }
        }
        let mut report = Self {
            mode,
            verdict: Verdict::Clean,
            counts,
            findings,
            stale,
            cannot_check,
        };
        report.verdict = report.verdict_in(mode);
        report
    }

    /// The verdict this run would have in `mode`.
    pub fn verdict_in(&self, mode: Mode) -> Verdict {
        if !self.cannot_check.is_empty() {
            return Verdict::CannotCheck;
        }
        if self.findings.iter().any(Finding::blocks_when_enforced) {
            return match mode {
                Mode::Observe => Verdict::Observed,
                Mode::Enforce => Verdict::Blocked,
            };
        }
        Verdict::Clean
    }

    /// The process exit code of the verdict.
    pub fn exit_code(&self) -> u8 {
        self.verdict.exit_code()
    }

    /// Whether `finding` blocks in this run's mode.
    pub fn blocks(&self, finding: &Finding) -> bool {
        self.mode == Mode::Enforce && finding.blocks_when_enforced()
    }

    /// One line per finding that blocks in the mode, one per cannot-check
    /// cause, then the summary. `detail` adds every other finding (errors
    /// under `observe`, warnings, debt) and the stale entries.
    pub fn lines(&self, detail: bool) -> Vec<String> {
        let mut lines = Vec::new();
        for finding in &self.findings {
            let blocking = self.blocks(finding);
            if !blocking && !detail {
                continue;
            }
            let label = if blocking {
                "error"
            } else if finding.is_live_debt() {
                "debt"
            } else {
                match finding.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                }
            };
            let note = match &finding.debt {
                Some(debt) if debt.expired => {
                    format!(" (debt expired {}: {})", debt.expires, debt.reason)
                }
                Some(debt) => format!(" (debt until {}: {})", debt.expires, debt.reason),
                None => String::new(),
            };
            lines.push(format!(
                "{label}  {}:{}: {}: {}{note}",
                shown(&finding.path),
                finding.line,
                finding.code,
                finding.message
            ));
        }
        if detail {
            for entry in &self.stale {
                lines.push(format!(
                    "stale  {}: debt-stale: the baseline entry at line {} ({}, subject {:?}) matches no finding ({}; expires {})",
                    shown(&entry.path),
                    entry.line,
                    entry.code,
                    entry.subject,
                    entry.reason,
                    entry.expires
                ));
            }
        }
        for cause in &self.cannot_check {
            lines.push(format!("cannot  {}: {}", shown(&cause.path), cause.message));
        }
        let counts = &self.counts;
        lines.push(format!(
            "spec check [{}]: {} documents, {} errors, {} warnings, {} debt, {} expired, {} stale — {}",
            self.mode,
            counts.documents,
            counts.errors,
            counts.warnings,
            counts.debt,
            counts.expired,
            counts.stale,
            self.verdict.as_str()
        ));
        lines
    }

    /// `{mode, verdict, counts, findings, stale, cannot_check}`, one line.
    pub fn to_json(&self) -> String {
        // Every field is a plain struct, enum, string or number: encoding
        // cannot fail.
        serde_json::to_string(self).unwrap_or_default()
    }
}

fn shown(path: &str) -> &str {
    if path.is_empty() { "." } else { path }
}
