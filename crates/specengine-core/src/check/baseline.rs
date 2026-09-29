//! The debt baseline (`.spec-debt.toml`, read only): acknowledged findings,
//! each with a reason and an expiry. An entry matches every finding with
//! its `(code, path, subject)`, never by line, so debt survives edits above
//! it; a match is debt until `today > expires`, then an error blocks again,
//! counted `expired`, and a warning stays a warning; an entry that matches
//! nothing is stale: no finding, counted `stale`, labelled `debt-stale` in
//! the detail lines.
//!
//! ```toml
//! [[debt]]
//! code    = "ref-dangling"
//! path    = "notes/overview.md"
//! subject = "X-15"              # default ""
//! reason  = "owner question 6"
//! expires = "2026-12-31"
//! ```
//!
//! A missing `reason` or `expires`, a date that is not `YYYY-MM-DD`, an
//! unknown key, a repeated `(code, path, subject)` or bad TOML is a
//! [`BaselineError`]: the check cannot run (exit 2).

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use toml::Spanned;

use super::text::{is_calendar_date, line_of_str};

/// One acknowledged finding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct DebtEntry {
    pub code: String,
    pub path: String,
    /// The finding's subject; `""` for the file.
    pub subject: String,
    pub reason: String,
    /// `YYYY-MM-DD`: the last day the entry holds.
    pub expires: String,
    /// The entry's line in the baseline file.
    pub line: usize,
}

/// The entries, in file order; empty when there is no baseline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    pub entries: Vec<DebtEntry>,
}

impl Baseline {
    /// No debt.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Reads `[[debt]]` entries; any other top-level key is an error.
    pub fn from_toml(text: &str) -> Result<Self, BaselineError> {
        baseline_from_toml(text)
    }
}

/// A baseline error: the 1-based line when known, and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineError {
    pub line: Option<usize>,
    pub message: String,
}

impl BaselineError {
    /// `file:line: message` (`file: message` without a line).
    pub fn at(&self, file: &str) -> String {
        match self.line {
            Some(line) => format!("{file}:{line}: {}", self.message),
            None => format!("{file}: {}", self.message),
        }
    }
}

impl fmt::Display for BaselineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for BaselineError {}

/// [`Baseline::from_toml`] as a function.
pub fn baseline_from_toml(text: &str) -> Result<Baseline, BaselineError> {
    let error_at = |span: Option<Range<usize>>, message: String| BaselineError {
        line: span.map(|span| line_of_str(text, span.start)),
        message,
    };
    let raw: RawFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let mut entries = Vec::with_capacity(raw.debt.len());
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    for raw in raw.debt {
        let span = raw.span();
        let raw = raw.into_inner();
        let missing =
            |key: &str| error_at(Some(span.clone()), format!("debt entry without `{key}`"));
        let code = raw.code.ok_or_else(|| missing("code"))?;
        let path = raw.path.ok_or_else(|| missing("path"))?;
        let reason = raw.reason.ok_or_else(|| missing("reason"))?;
        let expires = raw.expires.ok_or_else(|| missing("expires"))?;
        if code.trim().is_empty() || path.trim().is_empty() {
            return Err(error_at(
                Some(span),
                "debt entry with an empty `code` or `path`".to_owned(),
            ));
        }
        if reason.get_ref().trim().is_empty() {
            return Err(error_at(
                Some(reason.span()),
                "debt entry with an empty `reason`".to_owned(),
            ));
        }
        if !is_calendar_date(expires.get_ref()) {
            return Err(error_at(
                Some(expires.span()),
                format!("`expires` `{}` is not a YYYY-MM-DD date", expires.get_ref()),
            ));
        }
        let subject = raw.subject.unwrap_or_default();
        if !seen.insert((code.clone(), path.clone(), subject.clone())) {
            return Err(error_at(
                Some(span),
                format!("debt entry ({code}, {path}, {subject:?}) is repeated"),
            ));
        }
        entries.push(DebtEntry {
            code,
            path,
            subject,
            reason: reason.into_inner(),
            expires: expires.into_inner(),
            line: line_of_str(text, span.start),
        });
    }
    Ok(Baseline { entries })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    debt: Vec<Spanned<RawEntry>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    reason: Option<Spanned<String>>,
    #[serde(default)]
    expires: Option<Spanned<String>>,
}
