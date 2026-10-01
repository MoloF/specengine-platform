//! What a check found: findings, stale debt, the causes that stop it from
//! vouching for the corpus, counts and the verdict; rendered as a few lines
//! or as JSON, both deterministic (everything sorted). A run judged against
//! a base ([`super::judge`]) adds whether each finding is introduced and the
//! new debt; without a base those fields are absent and the output is the
//! plain run's.

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
    /// With a base: whether the base's findings lack this finding's
    /// (code, path, subject); `false` = pre-existing. `None` without a
    /// base, where every finding counts as introduced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduced: Option<bool>,
}

impl Finding {
    /// Blocks under `enforce`: an error not in live debt (expired debt
    /// blocks).
    pub fn blocks_when_enforced(&self) -> bool {
        self.severity == Severity::Error && self.debt.as_ref().is_none_or(|debt| debt.expired)
    }

    /// Blocks in `mode`: never under `observe`; under `enforce-introduced`
    /// an error whose debt expired (pre-existing or not), or without debt
    /// and introduced (no base: introduced); under `enforce` an error not
    /// in live debt.
    pub fn blocks_in(&self, mode: Mode) -> bool {
        match mode {
            Mode::Observe => false,
            Mode::EnforceIntroduced => {
                self.severity == Severity::Error
                    && match &self.debt {
                        Some(debt) => debt.expired,
                        None => self.introduced != Some(false),
                    }
            }
            Mode::Enforce => self.blocks_when_enforced(),
        }
    }

    /// Pre-existing: a base holds its key.
    pub fn is_pre_existing(&self) -> bool {
        self.introduced == Some(false)
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

/// A baseline entry its base's baseline lacks (by its triple), or holds
/// with an earlier `expires`: it blocks under `enforce-introduced` and
/// `enforce`, shown as `debt-new` (a label, not a code).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct NewDebt {
    #[serde(flatten)]
    pub entry: DebtEntry,
    /// The base's `expires` this entry extends; `None` when the base's
    /// baseline lacks the triple.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_expires: Option<String>,
}

/// The outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No error outside live debt and no new debt (warnings, debt and
    /// stale entries never count).
    Clean,
    /// Nothing blocks, but an error outside live debt or new debt remains
    /// (`observe`; pre-existing errors under `enforce-introduced`).
    Observed,
    /// Something blocks in the mode.
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
    /// With a base: introduced errors not in live debt (also counted in
    /// `errors`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introduced: Option<usize>,
    /// With a base whose baseline is known: the new-debt entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_debt: Option<usize>,
    /// The worst-case working set W in bytes ([`worst_w`](super::worst_w));
    /// 0 when the check could not vouch for the corpus.
    pub worst_w_bytes: u64,
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
    /// With a base whose baseline is known: the entries it lacks or holds
    /// with an earlier `expires`, sorted like `stale`; `None` without a
    /// base or when the new-debt rule is lifted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_debt: Option<Vec<NewDebt>>,
    /// Sorted; non-empty exactly when the verdict is `cannot-check`.
    pub cannot_check: Vec<Cause>,
}

impl Report {
    /// A report of a check that could not start (no root, an invalid config
    /// or baseline): no documents, only causes.
    pub fn cannot(mode: Mode, causes: Vec<Cause>) -> Self {
        Self::assemble(mode, 0, 0, Vec::new(), Vec::new(), causes)
    }

    /// Sorts, counts and judges; `worst_w_bytes` is kept only when nothing
    /// stops the check from vouching for the corpus.
    pub(crate) fn assemble(
        mode: Mode,
        documents: usize,
        worst_w_bytes: u64,
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
        stale.sort_by(entry_order);
        cannot_check.sort();
        cannot_check.dedup();
        let counts = Counts {
            documents,
            worst_w_bytes: if cannot_check.is_empty() {
                worst_w_bytes
            } else {
                0
            },
            ..Counts::default()
        };
        let mut report = Self {
            mode,
            verdict: Verdict::Clean,
            counts,
            findings,
            stale,
            new_debt: None,
            cannot_check,
        };
        report.settle(false);
        report
    }

    /// Recounts everything but `documents` and `worst_w_bytes` and judges
    /// in `self.mode`; `with_base` gives `counts.introduced`, a known base
    /// baseline (`new_debt`) `counts.new_debt`.
    pub(crate) fn settle(&mut self, with_base: bool) {
        let mut counts = Counts {
            documents: self.counts.documents,
            stale: self.stale.len(),
            introduced: with_base.then_some(0),
            new_debt: self.new_debt.as_ref().map(Vec::len),
            worst_w_bytes: self.counts.worst_w_bytes,
            ..Counts::default()
        };
        for finding in &self.findings {
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
                    if finding.introduced == Some(true)
                        && let Some(introduced) = counts.introduced.as_mut()
                    {
                        *introduced += 1;
                    }
                }
                Severity::Warning => counts.warnings += 1,
            }
        }
        self.counts = counts;
        self.verdict = self.verdict_in(self.mode);
    }

    /// The same run without a base (a plain run): `enforce-introduced` is
    /// judged and shown as `enforce`, which it equals there; any other
    /// mode is kept.
    #[must_use]
    pub fn without_base(mut self) -> Self {
        if self.mode == Mode::EnforceIntroduced {
            self.mode = Mode::Enforce;
            self.verdict = self.verdict_in(self.mode);
        }
        self
    }

    /// The verdict this run would have in `mode`: `blocked` when a finding
    /// blocks in it ([`Finding::blocks_in`]) or, under `enforce-introduced`
    /// and `enforce`, new debt remains; else `observed` when an error
    /// outside live debt or new debt remains; else `clean`.
    pub fn verdict_in(&self, mode: Mode) -> Verdict {
        if !self.cannot_check.is_empty() {
            return Verdict::CannotCheck;
        }
        let new_debt = self
            .new_debt
            .as_ref()
            .is_some_and(|entries| !entries.is_empty());
        if (new_debt && mode != Mode::Observe)
            || self.findings.iter().any(|finding| finding.blocks_in(mode))
        {
            return Verdict::Blocked;
        }
        if new_debt
            || self
                .findings
                .iter()
                .any(|finding| finding.severity == Severity::Error && !finding.is_live_debt())
        {
            return Verdict::Observed;
        }
        Verdict::Clean
    }

    /// The process exit code of the verdict.
    pub fn exit_code(&self) -> u8 {
        self.verdict.exit_code()
    }

    /// Whether `finding` blocks in this run's mode.
    pub fn blocks(&self, finding: &Finding) -> bool {
        finding.blocks_in(self.mode)
    }

    /// Whether the new-debt entries block in this run's mode (every one
    /// does under `enforce-introduced` and `enforce`).
    pub fn new_debt_blocks(&self) -> bool {
        self.mode != Mode::Observe
    }

    /// One line per finding that blocks in the mode, one per new-debt
    /// entry when they block, one per cannot-check cause, then the
    /// summary. `detail` adds every other finding (errors under `observe`,
    /// pre-existing errors, warnings, debt), the stale entries and the new
    /// debt in any mode.
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
                None if finding.severity == Severity::Error && finding.is_pre_existing() => {
                    " (pre-existing)".to_owned()
                }
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
        if detail || self.new_debt_blocks() {
            for new in self.new_debt.iter().flatten() {
                let entry = &new.entry;
                let what = match &new.head_expires {
                    Some(head) => format!("extends HEAD's expiry {head}"),
                    None => "is not in HEAD's baseline".to_owned(),
                };
                lines.push(format!(
                    "new  {}: debt-new: the baseline entry at line {} ({}, subject {:?}) {what} ({}; expires {})",
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
        let mut against_base = String::new();
        if let Some(introduced) = counts.introduced {
            against_base.push_str(&format!(", {introduced} introduced"));
        }
        if let Some(new_debt) = counts.new_debt {
            against_base.push_str(&format!(", {new_debt} new debt"));
        }
        lines.push(format!(
            "spec check [{}]: {} documents, {} errors, {} warnings, {} debt, {} expired, {} stale{against_base}, worst W {} B — {}",
            self.mode,
            counts.documents,
            counts.errors,
            counts.warnings,
            counts.debt,
            counts.expired,
            counts.stale,
            counts.worst_w_bytes,
            self.verdict.as_str()
        ));
        lines
    }

    /// `{mode, verdict, counts, findings, stale, new_debt?, cannot_check}`,
    /// one line; the base's fields are absent without a base.
    pub fn to_json(&self) -> String {
        // Every field is a plain struct, enum, string or number: encoding
        // cannot fail.
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// The order of stale and new-debt entries: (path, code, subject, line).
pub(crate) fn entry_order(a: &DebtEntry, b: &DebtEntry) -> std::cmp::Ordering {
    (&a.path, &a.code, &a.subject, a.line).cmp(&(&b.path, &b.code, &b.subject, b.line))
}

fn shown(path: &str) -> &str {
    if path.is_empty() { "." } else { path }
}
