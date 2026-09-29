//! Findings about one file. None is fatal: an error means part of the file
//! was not read, a warning that something was read with a caveat
//! (ADR-0012: nothing is blocked by them).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::span::Span;

/// Error or warning; fixed by the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// What a diagnostic is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticCode {
    /// The file is not UTF-8: no nodes.
    NotUtf8,
    /// `---` opens front-matter and nothing closes it: the whole file is body.
    FrontmatterUnclosed,
    /// YAML syntax, nesting over 32 or alias expansion over 10 000 nodes:
    /// no front-matter value is kept, the body is still read.
    FrontmatterYaml,
    /// The front-matter is YAML but not a mapping.
    FrontmatterNotMapping,
    /// A typed key holds a value of another type; the raw value is in `extra`.
    FrontmatterType,
    /// `id:` is no ID of a configured prefix: the document has no ID.
    IdNotInScheme,
    UnknownKey,
    UnknownLinkType,
    /// A reference-carrying value is not exactly one reference.
    UnparsedReference,
    /// A look-alike inside an ID; `fix` is the Latin ID.
    Homoglyph,
    /// Declared `kind` differs from the kind of the ID's prefix; declared kept.
    KindMismatch,
    /// An ID defined twice in the file; both are kept.
    DuplicateId,
    /// A revision that is not 1–9 digits.
    BadRev,
}

impl DiagnosticCode {
    /// Every code, in declaration order.
    pub const ALL: [DiagnosticCode; 13] = [
        Self::NotUtf8,
        Self::FrontmatterUnclosed,
        Self::FrontmatterYaml,
        Self::FrontmatterNotMapping,
        Self::FrontmatterType,
        Self::IdNotInScheme,
        Self::UnknownKey,
        Self::UnknownLinkType,
        Self::UnparsedReference,
        Self::Homoglyph,
        Self::KindMismatch,
        Self::DuplicateId,
        Self::BadRev,
    ];

    pub const fn severity(self) -> Severity {
        match self {
            Self::NotUtf8
            | Self::FrontmatterUnclosed
            | Self::FrontmatterYaml
            | Self::FrontmatterNotMapping
            | Self::FrontmatterType
            | Self::IdNotInScheme => Severity::Error,
            Self::UnknownKey
            | Self::UnknownLinkType
            | Self::UnparsedReference
            | Self::Homoglyph
            | Self::KindMismatch
            | Self::DuplicateId
            | Self::BadRev => Severity::Warning,
        }
    }

    /// The kebab-case name used in JSON.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotUtf8 => "not-utf8",
            Self::FrontmatterUnclosed => "frontmatter-unclosed",
            Self::FrontmatterYaml => "frontmatter-yaml",
            Self::FrontmatterNotMapping => "frontmatter-not-mapping",
            Self::FrontmatterType => "frontmatter-type",
            Self::IdNotInScheme => "id-not-in-scheme",
            Self::UnknownKey => "unknown-key",
            Self::UnknownLinkType => "unknown-link-type",
            Self::UnparsedReference => "unparsed-reference",
            Self::Homoglyph => "homoglyph",
            Self::KindMismatch => "kind-mismatch",
            Self::DuplicateId => "duplicate-id",
            Self::BadRev => "bad-rev",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `{code, severity, line, span?, fix?, message}`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    /// 1-based line in the file.
    pub line: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    /// The replacement text for `span` (the Latin ID of a homoglyph).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// Human-readable detail; the file is named by the caller.
    pub message: String,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, line: usize, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: code.severity(),
            line,
            span: None,
            fix: None,
            message: message.into(),
        }
    }

    pub fn with_span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }

    pub fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
}
