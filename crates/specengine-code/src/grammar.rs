//! The Rust grammar: pinned versions, the ABI it was generated with, a parser.

use tree_sitter::{Language, LanguageError, Parser, Tree};

/// Version of the `tree-sitter` runtime pinned in the workspace manifest (04 §6).
/// Kept in sync by hand: Cargo exposes no dependency versions at build time.
pub const TREE_SITTER_VERSION: &str = "0.27.0";

/// Version of the `tree-sitter-rust` grammar pinned in the workspace manifest (04 §6).
/// It is part of the recipe header (05 §5.2): a grammar upgrade rebases every
/// hash deliberately instead of showing up as drift.
pub const TREE_SITTER_RUST_VERSION: &str = "0.24.2";

/// The Rust grammar as a tree-sitter language.
#[must_use]
pub fn language() -> Language {
    Language::new(tree_sitter_rust::LANGUAGE)
}

/// Versions that identify the parsing stack in measurements and in the recipe header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrammarInfo {
    pub tree_sitter: &'static str,
    pub tree_sitter_rust: &'static str,
    /// ABI version the grammar was generated with (`Language::abi_version`).
    pub abi: usize,
}

/// Versions of the runtime and the grammar, plus the grammar's ABI.
#[must_use]
pub fn grammar_info() -> GrammarInfo {
    GrammarInfo {
        tree_sitter: TREE_SITTER_VERSION,
        tree_sitter_rust: TREE_SITTER_RUST_VERSION,
        abi: language().abi_version(),
    }
}

/// A parser bound to the Rust grammar.
pub struct RustParser {
    parser: Parser,
}

impl RustParser {
    /// Fails only if the grammar's ABI is outside the runtime's supported range.
    pub fn new() -> Result<Self, LanguageError> {
        let mut parser = Parser::new();
        parser.set_language(&language())?;
        Ok(Self { parser })
    }

    /// Parses one file from scratch.
    ///
    /// `None` only when tree-sitter cancels the parse, which needs a progress
    /// callback that this parser never installs; callers treat it as "not parsed".
    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }
}
