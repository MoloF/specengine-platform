//! The corpus model of SpecEngine (docs/features/spec-parser.md).
//!
//! Types and pure functions only — `serde`, no I/O, no parser library, no
//! other SpecEngine crate. Below `specengine-core` and `specengine-code`;
//! nothing here knows a project: prefixes, kinds and aliases come from the
//! project's `[ids]` scheme (ADR-0008).
//!
//! - [`span`] — byte spans into the original file;
//! - [`script`] — the look-alike table and the Latin / mixed / non-Latin class
//!   (ADR-0009);
//! - [`scheme`] — the `[ids]` scheme: prefixes, kinds, shapes, aliases;
//! - [`grammar`] — the one reference grammar and its lexer, shared by text,
//!   front-matter and `{#ID}` definitions;
//! - [`reference`], [`link`], [`node`], [`value`], [`diagnostic`],
//!   [`parsed`] — what a parsed file is made of.

pub mod diagnostic;
pub mod grammar;
pub mod link;
pub mod node;
pub mod parsed;
pub mod reference;
pub mod scheme;
pub mod script;
pub mod span;
pub mod value;

pub use diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use grammar::{Canon, Definition, Found, Homoglyph};
pub use link::{
    Direction, IMPACT_LINK_TYPES, LINK_TYPES, Link, LinkOrigin, LinkTarget, MENTIONS,
    graph_direction, impact_direction, is_weak_link,
};
pub use node::{Anchor, AnchorOrigin, ExtraEntry, Fields, Node, ParentRef};
pub use parsed::ParsedFile;
pub use reference::{CanonTarget, PathTarget, RefForm, Reference};
pub use scheme::{IdScheme, IdScope, PrefixSpec, SchemeError, SchemeField, SchemeProblem, Shape};
pub use script::IdScript;
pub use span::Span;
pub use value::{FmValue, OrderedMap};
