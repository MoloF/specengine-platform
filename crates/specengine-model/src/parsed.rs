//! The parse of one file.

use serde::{Deserialize, Serialize};

use crate::diagnostic::Diagnostic;
use crate::link::Link;
use crate::node::{Anchor, Node};
use crate::span::Span;

/// Everything read from one file; a function of (path, bytes, scheme) only.
///
/// Invariant: BOM (3 bytes when `bom`) + `front_matter` + `body` = the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedFile {
    /// As given by the caller (corpus-relative, `/`-separated).
    pub path: String,
    /// The file starts with a UTF-8 byte-order mark.
    pub bom: bool,
    /// From the opening `---` through the line ending of the closing `---`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_matter: Option<Span>,
    /// Everything after the front-matter (after the BOM without one).
    pub body: Span,
    /// The document first, then the sections in source order; empty for a
    /// file that is not UTF-8.
    pub nodes: Vec<Node>,
    /// Declared links in front-matter order, then inline mentions in text order.
    pub links: Vec<Link>,
    /// Heading slugs, non-ID `{#…}` attributes and HTML `<a id|name>`
    /// tags, in source order (a heading's slug before its attribute).
    pub anchors: Vec<Anchor>,
    /// In line order.
    pub diagnostics: Vec<Diagnostic>,
}

impl ParsedFile {
    /// The document node, when the file was read.
    pub fn document(&self) -> Option<&crate::node::Node> {
        self.nodes.first()
    }

    /// The section nodes.
    pub fn sections(&self) -> &[Node] {
        self.nodes.get(1..).unwrap_or(&[])
    }
}
