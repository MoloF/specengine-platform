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
//!
//! Phase 3 slice `code-refs` (`docs/canon/code-bindings.md` "Rust binding",
//! "Mentions"):
//!
//! - [`bind`] — every comment of a Rust file bound to an item, a module,
//!   nothing (with a category) or `cannot_verify`, and where a mention in
//!   it belongs; [`ron::comment_anchors`] the same for every RON comment;
//!   [`hash_bound_items`] the hashes of chosen bound items (the lock's
//!   `ast_hash`: `docs/canon/spec-lock.md` "Entry"), the binding itself
//!   hashing none;
//! - [`naming`] — a tree's items and modules named at once: `qpath` and
//!   ambiguity (`path_attribute`, `unrooted`, `duplicate`: an item of
//!   another file renders the same `qpath`).
//!
//! Phase 3 slice 5a-1 `hash-levels` (`crates/specengine-code/README.md`
//! "Hash recipe"):
//!
//! - [`levels`] — recipe `specengine-hash/v3` beside v2: an item's levels
//!   `sig`, `body`, `deps` and fingerprints `skeleton`, `quote`,
//!   `body_bytes` ([`LevelHashes`], [`TRIVIAL_BODY_BYTES`]), or
//!   `cannot_verify` ([`LevelState`]); the file's module
//!   ([`module_levels`]); the body of every item kind as data
//!   ([`BODY_FIELDS`], [`body_nodes`]) and the calls `deps` hashes
//!   ([`call_targets`]); [`item_levels`] the twin of [`hash_bound_items`]
//!   for chosen bound items. Nothing records these digests yet.
//!
//! Phase 3 slice 5a-1b `hash-levels-ron` (the same section):
//!
//! - [`ron::value_hashes`] — the value hash of every RON comment anchored to
//!   a path, keyed by the comment's range ([`RonValues`]), or the file's
//!   refusal when [`ron::analyze`] rejects it.
//!
//! Phase 3 slice 6a-1 `symbol-discriminator`
//! (`docs/canon/symbol-names.md` "Discriminator", "Signature"):
//!
//! - [`naming`] — in-file twins named apart, `<qpath> #<discriminator>`
//!   (their `cfg` chain, [`ItemRecord::cfg`], else their ordinal), in two
//!   halves: per file ([`name_file`], [`indexed_name_file`] over a
//!   package's prepared [`TargetIndex`]) and tree-wide ([`tree_passes`]
//!   over [`path_targets`]);
//! - [`item_signatures`] — the display signature of chosen bound items,
//!   at most [`SIGNATURE_CAP_BYTES`], never hashed.
//!
//! Phase 3 slice 6a-2 `symbol-index`
//! (`docs/canon/symbol-index.md` "Analyzer stamp"): [`CODE_INDEX_FORMAT`],
//! the stamp of every rule above whose output the code index stores;
//! [`placed_name_file`] a file's names with its place and each item's
//! structured path, each read once.
//!
//! Phase 3 slice 6a-3 `symbol-reads` (`docs/canon/symbol-reads.md`
//! "Lookup"): [`query_name`], a lookup query's `name` key, the one reader
//! of `::` in a query; the closed word lists `ALL` of [`Relation`],
//! [`LevelError`], [`Unanchored`], [`ErrorCategory`] and [`Ambiguity`], so
//! a reader of stored words knows every variant.
//!
//! Phase 3 slice 10a-1 (`docs/canon/code-roles.md` "Role facts"):
//!
//! - [`frameworks`] — the framework roles as data ([`frameworks::KNOWN`],
//!   each framework's words, sources, categories and per-word item kinds)
//!   and [`frameworks::role_facts`], the role facts of one parsed file;
//! - [`bevy`] — also reads role derives (`cfg_attr` too), `impl Component
//!   for`, `add_message`, and each registration's subject; its shipped
//!   lists and counts are unchanged.

pub mod bevy;
pub mod bind;
pub mod comments;
pub mod frameworks;
pub mod grammar;
pub mod hash;
pub mod items;
pub mod levels;
pub mod markers;
pub mod naming;
pub mod qpath;
pub mod ron;

pub use bevy::{BevyAnalysis, detect, detect_file};
pub use bind::{
    Binding, BoundComment, RustBinding, SIGNATURE_CAP_BYTES, Unanchored, bind_file, bind_tree,
    hash_bound_items, item_levels, item_signatures,
};
pub use grammar::{GrammarInfo, RustParser, grammar_info};
pub use hash::{Digest, ErrorCategory, HashState, RECIPE, hash_item, normalize, recipe_header};
pub use items::{FileAnalysis, ItemRecord, MOD_KIND, ModDeclaration, analyze_file, analyze_tree};
pub use levels::{
    BODY_FIELDS, BodyField, BodySlot, LEVEL_RECIPE, LevelHashes, LevelState, TRIVIAL_BODY_BYTES,
    body_nodes, call_targets, level_header, module_levels,
};
pub use markers::{DEFAULT_LEVELS, Level, LevelError, Levels, Marker, Relation, markers_in};
pub use naming::{
    DISCRIMINATOR_SEPARATOR, FileNames, Name, NamedFile, Names, PlacedNames, ROOT_PACKAGE,
    file_place, indexed_name_file, name_file, name_files, path_targets, placed_name_file,
    query_name, tree_passes,
};
pub use qpath::{
    Ambiguity, FileRole, PackageTargets, QPath, Target, TargetIndex, TargetSource, Unit, UnitKind,
    layout_targets,
};
pub use ron::{
    Anchor, CommentAnchor, Rejected, RonAnalysis, RonMarker, RonValues, comment_anchors,
    value_hashes,
};

/// The stamp of what the code index stores per file
/// (`docs/canon/symbol-index.md` "Analyzer stamp"): the binding, the
/// naming and its discriminator, the display signature, the markers and
/// mentions as this crate reads them. Any change of a rule whose output a
/// stored row holds needs a new number (the levels' own recipe is
/// [`level_header`]'s, a part of the same cache key), and a new line in
/// the CLI's `tests/code_format_history.txt`.
///
/// 1: the first stored rules: 6a-1's discriminator (`cfg` chains read past
/// comments in attributes) and signature (a space ending a cut dropped).
///
/// 2: a stored directory's `\` becomes `/` only where it separates
/// components (`std::path::MAIN_SEPARATOR`); on Unix it is a character of
/// a name and kept. Two rules: 6a-3's `dir_text` in the CLI (the stored
/// package and target directories) and 6b-1's [`file_place`] package label
/// (the package of a qpath and of a name). Rows stored under 1 re-derive
/// once.
///
/// 3: 10a-1's framework role rows: with `[code] frameworks` listed, each
/// Rust file's [`frameworks::role_facts`] (`code_roles`); the stamp gains
/// `frameworks`. From 3 on the history line hashes `fixtures/code-index`'s
/// dump, then `fixtures/code-bevy`'s. Rows stored under 2 re-derive once.
///
/// 4: 10a-2's B5: a role subject's qualifier that is no path is rendered
/// as `items.rs`' `impl_label` names an impl, each side compacted and
/// joined by ` as ` (`<Foo<A, B> as Tr>::f` stored `<Foo<A,B> as
/// Tr>::f`), so a spaced one meets its suffix. Rows stored under 3
/// re-derive once.
pub const CODE_INDEX_FORMAT: u32 = 4;
