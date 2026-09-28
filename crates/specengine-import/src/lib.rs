//! Importers of existing spec corpora (08 §4).
//!
//! Phase 0 increment: the census, a read-only dry-run counter over an existing
//! corpus (`docs/features/phase-0-spikes.md`, spike group 3). Nothing
//! corpus-specific lives here: every convention — roots, document extensions,
//! the front-matter class key, the ID pattern, the table ID column, `{#ID}`
//! sections — comes from a [`CensusConfig`] read at run time (ADR-0008).

pub mod census;
pub mod config;
mod frontmatter;
mod markdown;
pub mod script;

pub use census::{
    BrokenLink, Census, Diagnostic, DocumentSummary, FrontMatterState, Location, Record,
    RecordKind, run,
};
pub use config::{CensusConfig, ConfigError, IdMatch, IdPattern};
pub use script::IdScript;
