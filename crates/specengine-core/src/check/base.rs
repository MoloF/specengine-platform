//! A run judged against its base (docs/features/spec-cli-introduced.md):
//! what the commit adds. The base is HEAD's tree checked under the checked
//! tree's scheme, `[paths]` and check tables, its causes dropped; the
//! loader (the store's staged check) builds it, [`judge`] compares. Pure:
//! the result depends only on the arguments.
//!
//! - **Introduced**: a finding whose (code, path, subject) the base's
//!   findings lack, by set membership: a key's second occurrence is
//!   pre-existing, a moved file's findings are introduced.
//! - **New debt**: an entry of the checked baseline that the base's
//!   baseline lacks by its triple, or holds with an earlier `expires`;
//!   never an earlier `expires`, a changed `reason` or a removal. A base
//!   baseline that is not known lifts the rule (`new_debt` absent).
//! - **Mode**: the stricter of the checked and the base's (`Ord` is the
//!   ladder); a base mode that is not known leaves the checked one.

use std::collections::{BTreeMap, BTreeSet};

use super::baseline::{Baseline, DebtEntry};
use super::config::Mode;
use super::report::{Finding, NewDebt, Report, entry_order};

/// What a run is compared with. No `Default`: an empty value would read as
/// "baseline lifted, mode unknown", not as an unborn `HEAD` (whose
/// baseline is known and empty); the loader states each field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// The base's findings, judged under the checked tree's rules with an
    /// empty baseline; its cannot-check causes are not part of it.
    pub findings: Vec<Finding>,
    /// The base's baseline at the checked baseline's path, empty when it
    /// has none there; `None` when it is not known: the new-debt rule is
    /// lifted.
    pub baseline: Option<Baseline>,
    /// The base's `[check] mode`; `None` when it has no config there or
    /// the mode is not known.
    pub mode: Option<Mode>,
}

/// `report`, the run of `baseline`'s check, judged against `base`: every
/// finding gets `introduced`, the counts `introduced` (and `new_debt` with
/// a known base baseline), the report its new debt and the stricter of the
/// two modes, and the verdict is taken again.
pub fn judge(mut report: Report, baseline: &Baseline, base: &Base) -> Report {
    let known: BTreeSet<(&str, &str, &str)> = base.findings.iter().map(finding_key).collect();
    for finding in &mut report.findings {
        let introduced = !known.contains(&finding_key(finding));
        finding.introduced = Some(introduced);
    }
    report.new_debt = base.baseline.as_ref().map(|head| new_debt(baseline, head));
    if let Some(mode) = base.mode {
        report.mode = report.mode.max(mode);
    }
    report.settle(true);
    report
}

/// The checked entries `head` lacks, or holds with an earlier `expires`,
/// sorted by (path, code, subject, line).
fn new_debt(checked: &Baseline, head: &Baseline) -> Vec<NewDebt> {
    let by_key: BTreeMap<(&str, &str, &str), &DebtEntry> = head
        .entries
        .iter()
        .map(|entry| (entry_key(entry), entry))
        .collect();
    let mut entries: Vec<NewDebt> = checked
        .entries
        .iter()
        .filter_map(|entry| match by_key.get(&entry_key(entry)) {
            None => Some(NewDebt {
                entry: entry.clone(),
                head_expires: None,
            }),
            // Both are validated `YYYY-MM-DD` dates: the text order is the
            // calendar's.
            Some(head) if entry.expires > head.expires => Some(NewDebt {
                entry: entry.clone(),
                head_expires: Some(head.expires.clone()),
            }),
            Some(_) => None,
        })
        .collect();
    entries.sort_by(|a, b| entry_order(&a.entry, &b.entry));
    entries
}

fn finding_key(finding: &Finding) -> (&str, &str, &str) {
    (&finding.code, &finding.path, &finding.subject)
}

fn entry_key(entry: &DebtEntry) -> (&str, &str, &str) {
    (&entry.code, &entry.path, &entry.subject)
}
