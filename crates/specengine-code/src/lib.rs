//! Code side of SpecEngine: parsing Rust with tree-sitter, the normalized AST
//! hash of spec 05 §5.2, item collection and heuristic module paths (`qpath`).
//!
//! Module map, rules and known limits: `crates/specengine-code/README.md`.
//!
//! Phase 0 increment `ast-hash` (recipe and measured stability: 05 §5.2):
//!
//! - [`grammar`] — the pinned grammar, its ABI and a ready parser;
//! - [`hash`] — the recipe: name node excluded, comments filtered by `kind()`,
//!   anonymous `,` skipped, length-prefixed strings, BLAKE3, recipe header;
//!   items with a parse error are `cannot_verify` and never hashed;
//! - [`comments`] — comment stripping by comment node ranges (a perturbation);
//! - [`items`] — the items of one file with their hash state and attached
//!   `#[path]` attributes;
//! - [`qpath`] — module path from the file's place among its package's Cargo
//!   targets (a target table handed in, or Cargo's auto-discovery): one unit
//!   per file; every ambiguity is reported, never guessed.
//!
//! Phase 0 increment `ron` (marker grammar and RON binding:
//! `docs/canon/code-identity.md` "Marker grammar", "RON binding"):
//!
//! - [`markers`] — the marker syntax shared by `.rs` and `.ron` comments,
//!   level lists (`[sig]`) included;
//! - [`ron`] — markers in `.ron` files resolved to field paths by an own
//!   lexer and a tolerant structure walk (the Phase 0 verdict `lexer`,
//!   05 §9 "AST" row); colliding sibling segments make a path ambiguous.
//!
//! Phase 0 increment `bevy-schedule` (the Bevy detector of 05 §5.1):
//!
//! - [`bevy`] — the syntactic Bevy registration detector: systems of
//!   `add_systems` through nested tuples, combinators and adapters, observers,
//!   `impl Plugin for` and `fn(&mut App)` plugins, `add_plugins`, and the same
//!   calls in macro token trees (names only); what it cannot read is reported
//!   with a category.

pub mod bevy;
pub mod comments;
pub mod grammar;
pub mod hash;
pub mod items;
pub mod markers;
pub mod qpath;
pub mod ron;

pub use bevy::{BevyAnalysis, detect, detect_file};
pub use grammar::{GrammarInfo, RustParser, grammar_info};
pub use hash::{Digest, ErrorCategory, HashState, RECIPE, hash_item, normalize, recipe_header};
pub use items::{FileAnalysis, ItemRecord, ModDeclaration, analyze_file, analyze_tree};
pub use markers::{Level, LevelError, Levels, Marker, Relation, markers_in};
pub use qpath::{
    Ambiguity, FileRole, PackageTargets, QPath, Target, TargetIndex, TargetSource, Unit, UnitKind,
    layout_targets,
};
pub use ron::{Anchor, Rejected, RonAnalysis, RonMarker};
