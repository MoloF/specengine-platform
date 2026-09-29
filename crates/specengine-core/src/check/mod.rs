//! `spec check`, increment 1 (task spec `spec-check`): one check of the
//! documentation convention, driven only by the project's `specengine.toml`
//! (architecture rules `#universal`, `#checks-migration`), over a set of
//! parses. The check source names no prefix, path or file of any project.
//!
//! [`run`] takes a [`CheckInput`] (per walked file: path, size, parse or
//! read error, bytes; the walk's problems), the `[ids]` scheme, `[paths]`,
//! the check tables ([`CheckConfig`]), the debt [`Baseline`] and today's
//! date, and gives a [`Report`]: findings sorted by (path, line, code,
//! subject, message), stale debt, cannot-check causes, counts and the
//! [`Verdict`]. It reads no file and writes none (the homoglyph fix is data
//! for `apply_proposal`); nothing it finds sets a status or a flag
//! (`#control`): `observe` never blocks, `enforce` blocks on errors not in
//! live debt, and exit 2 means the check could not vouch for the corpus.
//!
//! - [`config`] — `[budgets]` (caps in bytes), `[classes]` (contracts),
//!   `[check]` (mode);
//! - [`baseline`] — `.spec-debt.toml`: expiring debt entries;
//! - [`input`] — what the check runs over;
//! - [`report`] — findings, verdict, lines and JSON;
//! - the rules: parser diagnostics through [`PARSER_SEVERITY`], class
//!   contracts, budgets, ID definitions (`id-width`, `id-taken`,
//!   `file-name`), `canon:` and front-matter references.

pub mod baseline;
pub mod config;
mod engine;
pub mod input;
pub mod report;
mod text;

pub use baseline::{Baseline, BaselineError, DebtEntry, baseline_from_toml};
pub use config::{
    Budgets, CheckConfig, ClassContract, Classes, ConfigError, DocClass, Mode,
    check_config_from_toml,
};
pub use engine::{PARSER_SEVERITY, parser_severity, run};
pub use input::{CheckFile, CheckInput, Problem, ProblemKind};
pub use report::{Cause, Counts, Debt, Finding, Fix, Report, Verdict};
pub use text::{date_from_unix_days, is_calendar_date, is_date_shaped};

/// The codes of the findings the check itself emits (parser codes come
/// through [`PARSER_SEVERITY`]): errors, then the warning. A stale baseline
/// entry is no finding: it goes to [`Report::stale`], labelled `debt-stale`
/// only in the detail lines.
pub const CHECK_CODES: [&str; 19] = [
    "class-missing",
    "class-unknown",
    "key-missing",
    "key-extra",
    "scope-empty",
    "date-invalid",
    "status-invalid",
    "shipped-missing",
    "canon-missing",
    "tier-invalid",
    "id-width",
    "id-taken",
    "file-name",
    "budget",
    "canon-form",
    "canon-file",
    "canon-anchor",
    "ref-dangling",
    "name-skipped",
];
