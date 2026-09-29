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
//!   `[check]` (mode), `[[generators]]` (the generator registry);
//! - [`baseline`] — `.spec-debt.toml`: expiring debt entries;
//! - [`input`] — what the check runs over;
//! - [`report`] — findings, verdict, lines and JSON;
//! - [`resolve`] — the one reference resolution ([`Resolver`]), scope-aware:
//!   `slug/ID` and bare feature-scoped IDs resolve in feature documents
//!   only (ADR-0026);
//! - [`render_index`] — the generated index, rendered in memory;
//! - the rules: parser diagnostics through [`PARSER_SEVERITY`], class
//!   contracts, budgets, ID definitions (`id-scope`, `id-width`,
//!   `id-taken`, `file-name`), `canon:` and front-matter references; the index drift
//!   and the generator registry (§11.5–6, errors); the graph warnings
//!   (`mention-dangling`, `depends-cycle`, `ref-superseded`).

pub mod baseline;
pub mod config;
mod engine;
mod generated;
mod graph;
pub mod input;
mod render;
pub mod report;
pub mod resolve;
mod text;

pub use baseline::{Baseline, BaselineError, DebtEntry, baseline_from_toml};
pub use config::{
    Budgets, CheckConfig, ClassContract, Classes, ConfigError, DEFAULT_GATE, DocClass, Generator,
    Mode, check_config_from_toml,
};
pub use engine::{PARSER_SEVERITY, parser_severity, run};
pub use input::{CheckFile, CheckInput, Problem, ProblemKind};
pub use render::render_index;
pub use report::{Cause, Counts, Debt, Finding, Fix, Report, Verdict};
pub use resolve::{Resolution, Resolver};
pub use text::{date_from_unix_days, is_calendar_date, is_date_shaped};

/// The codes of the findings the check itself emits (parser codes come
/// through [`PARSER_SEVERITY`]): increment 1 (errors, then the warning
/// `name-skipped`), then increment 2 part 1 (the §11.5–6 errors, then the
/// graph warnings), then part 2 (the error `id-scope`, ADR-0026). A stale
/// baseline entry is no finding: it goes to [`Report::stale`], labelled
/// `debt-stale` only in the detail lines.
pub const CHECK_CODES: [&str; 27] = [
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
    "index-missing",
    "index-drift",
    "generator-unknown",
    "generator-path",
    "mention-dangling",
    "depends-cycle",
    "ref-superseded",
    "id-scope",
];
