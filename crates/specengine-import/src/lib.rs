//! Importers of existing spec corpora (08 §4).
//!
//! The census, a read-only dry-run counter over an existing corpus, and the
//! import engine grown from it: record recognizers, definitions and
//! references, legacy prefixes, verbatim hashes and the "before" report
//! (config schema and counts: `crates/specengine-import/README.md`; findings
//! on the pilots: 08 §4.3). Nothing corpus-specific lives here: every
//! convention — roots, document extensions, the front-matter class key, the
//! ID pattern, the table ID column, `{#ID}` sections, header maps, list
//! lead-ins, legacy prefixes, reference tables, code roots — comes from a
//! [`CensusConfig`] read at run time (ADR-0008).

pub mod census;
pub mod config;
mod frontmatter;
pub mod import;
mod markdown;
pub mod script;
pub mod walk;

pub use census::{
    BrokenLink, Census, Diagnostic, DocumentSummary, FrontMatterState, Location, Record,
    RecordKind, run,
};
pub use config::{
    CensusConfig, CodeConfig, ConfigError, DocumentsConfig, IdMatch, IdPattern, ImportConfig,
    Pattern,
};
pub use script::IdScript;
pub use walk::Walk;
