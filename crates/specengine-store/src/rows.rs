//! The projection of one `ParsedFile` onto rows, and back. Nothing is
//! resolved: `dst`, `parent`, aliases and anchors are stored as written, so
//! a row depends on its own file's bytes and the scheme only.
//!
//! Each row carries its query columns plus the model value as JSON (`node`,
//! `link`, `anchor`, `diagnostic`, the file's `shell`), and `ord` is the
//! position in `ParsedFile.{nodes, links, anchors, diagnostics}`: reading a
//! file's rows back in `ord` order gives the same `ParsedFile`.

use std::borrow::Cow;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use specengine_model::{Anchor, Diagnostic, Link, LinkTarget, Node, ParsedFile, Span};

use crate::error::StoreError;

/// A file's row and the rows it owns, ready to insert.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FileRows {
    pub path: String,
    /// BLAKE3 of the bytes, lower-case hex; `None` when they were not read.
    pub blake3: Option<String>,
    pub size: i64,
    pub read_error: Option<String>,
    /// `{bom, front_matter?, body}` of the parse; `None` without one.
    pub shell: Option<String>,
    pub nodes: Vec<NodeRow>,
    pub links: Vec<LinkRow>,
    pub anchors: Vec<AnchorRow>,
    pub diagnostics: Vec<DiagnosticRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeRow {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub parent_id: Option<String>,
    pub own_text: String,
    pub node: String,
    /// Front-matter `aliases:` of the node, in source order.
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LinkRow {
    pub src: Option<String>,
    pub link_type: String,
    pub dst_id: Option<String>,
    pub dst_path: Option<String>,
    pub link: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AnchorRow {
    pub name: String,
    pub anchor: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DiagnosticRow {
    pub code: String,
    pub diagnostic: String,
}

/// The parts of a `ParsedFile` that are neither its path nor a row of its own.
#[derive(Debug, Serialize, Deserialize)]
struct Shell {
    bom: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    front_matter: Option<Span>,
    body: Span,
}

/// BLAKE3 of `bytes`, lower-case hex.
pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

impl FileRows {
    /// The rows of a parsed file.
    pub(crate) fn parsed(bytes: &[u8], parsed: &ParsedFile) -> Result<Self, StoreError> {
        let shell = Shell {
            bom: parsed.bom,
            front_matter: parsed.front_matter,
            body: parsed.body,
        };
        let nodes = parsed
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                Ok(NodeRow {
                    id: node.id.clone(),
                    kind: node.kind.clone(),
                    title: node.title.clone(),
                    parent_id: node.parent.as_ref().map(|parent| parent.id.clone()),
                    own_text: own_text(bytes, parsed, index),
                    node: to_json(node, "node")?,
                    aliases: node
                        .fields
                        .as_ref()
                        .and_then(|fields| fields.aliases.clone())
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<_, StoreError>>()?;
        let links = parsed
            .links
            .iter()
            .map(|link| {
                let (dst_id, dst_path) = match &link.dst {
                    LinkTarget::Reference(reference) => (Some(reference.id.clone()), None),
                    LinkTarget::Path(target) => (None, Some(target.path.clone())),
                };
                Ok(LinkRow {
                    src: link.src.clone(),
                    link_type: link.link_type.clone(),
                    dst_id,
                    dst_path,
                    link: to_json(link, "link")?,
                })
            })
            .collect::<Result<_, StoreError>>()?;
        let anchors = parsed
            .anchors
            .iter()
            .map(|anchor| {
                Ok(AnchorRow {
                    name: anchor.name.clone(),
                    anchor: to_json(anchor, "anchor")?,
                })
            })
            .collect::<Result<_, StoreError>>()?;
        let diagnostics = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| {
                Ok(DiagnosticRow {
                    code: diagnostic.code.as_str().to_owned(),
                    diagnostic: to_json(diagnostic, "diagnostic")?,
                })
            })
            .collect::<Result<_, StoreError>>()?;
        Ok(Self {
            path: parsed.path.clone(),
            blake3: Some(hash_bytes(bytes)),
            size: size_of(bytes),
            read_error: None,
            shell: Some(to_json(&shell, "shell")?),
            nodes,
            links,
            anchors,
            diagnostics,
        })
    }

    /// The row of a file whose bytes could not be read (or whose parse
    /// failed): no hash, so the next update tries it again.
    pub(crate) fn unreadable(path: &str, size: i64, error: String) -> Self {
        Self {
            path: path.to_owned(),
            blake3: None,
            size,
            read_error: Some(error),
            shell: None,
            nodes: Vec::new(),
            links: Vec::new(),
            anchors: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

pub(crate) fn size_of(bytes: &[u8]) -> i64 {
    i64::try_from(bytes.len()).unwrap_or(i64::MAX)
}

/// The node's own text: its body (a section's `body`; the document's the
/// file's `body`) minus every ID section inside it, the remaining non-empty
/// pieces joined by `\n`. Nested sections are indexed on their own, so a
/// word belongs to exactly one node.
fn own_text(bytes: &[u8], parsed: &ParsedFile, index: usize) -> String {
    let Some(node) = parsed.nodes.get(index) else {
        return String::new();
    };
    let range = if index == 0 {
        parsed.body
    } else {
        node.body.unwrap_or(Span::new(node.span.end, node.span.end))
    };
    let mut pieces: Vec<Cow<'_, str>> = Vec::new();
    let mut push = |start: usize, end: usize| {
        if start < end
            && let Some(slice) = bytes.get(start..end)
        {
            pieces.push(String::from_utf8_lossy(slice));
        }
    };
    let mut cursor = range.start;
    for (position, section) in parsed.nodes.iter().enumerate().skip(1) {
        if position == index || !range.contains(section.span) || section.span.start < cursor {
            continue;
        }
        push(cursor, section.span.start);
        cursor = section.span.end;
    }
    push(cursor, range.end);
    pieces.retain(|piece| !piece.is_empty());
    pieces.join("\n")
}

pub(crate) fn to_json<T: Serialize>(value: &T, what: &str) -> Result<String, StoreError> {
    serde_json::to_string(value)
        .map_err(|error| StoreError::Sqlite(format!("cannot encode a {what} as JSON: {error}")))
}

pub(crate) fn from_json<T: DeserializeOwned>(text: &str, what: &str) -> Result<T, StoreError> {
    serde_json::from_str(text)
        .map_err(|error| StoreError::Sqlite(format!("a stored {what} does not decode: {error}")))
}

/// A `ParsedFile` from its stored parts, rows in `ord` order.
pub(crate) fn rebuild_parsed(
    path: &str,
    shell: &str,
    nodes: &[String],
    links: &[String],
    anchors: &[String],
    diagnostics: &[String],
) -> Result<ParsedFile, StoreError> {
    let shell: Shell = from_json(shell, "shell")?;
    Ok(ParsedFile {
        path: path.to_owned(),
        bom: shell.bom,
        front_matter: shell.front_matter,
        body: shell.body,
        nodes: nodes
            .iter()
            .map(|node| from_json::<Node>(node, "node"))
            .collect::<Result<_, _>>()?,
        links: links
            .iter()
            .map(|link| from_json::<Link>(link, "link"))
            .collect::<Result<_, _>>()?,
        anchors: anchors
            .iter()
            .map(|anchor| from_json::<Anchor>(anchor, "anchor"))
            .collect::<Result<_, _>>()?,
        diagnostics: diagnostics
            .iter()
            .map(|diagnostic| from_json::<Diagnostic>(diagnostic, "diagnostic"))
            .collect::<Result<_, _>>()?,
    })
}
