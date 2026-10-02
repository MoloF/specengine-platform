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
    /// Tier 3 by core's one predicate (`check::is_tier3_file`); `false`
    /// without a parse.
    pub tier3: bool,
    pub nodes: Vec<NodeRow>,
    pub links: Vec<LinkRow>,
    pub anchors: Vec<AnchorRow>,
    pub diagnostics: Vec<DiagnosticRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeRow {
    /// The 1-based line of the node's span start.
    pub line: i64,
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
        let newlines: Vec<usize> = bytes
            .iter()
            .enumerate()
            .filter_map(|(offset, &byte)| (byte == b'\n').then_some(offset))
            .collect();
        let line_of = |offset: usize| {
            i64::try_from(newlines.partition_point(|&newline| newline < offset) + 1)
                .unwrap_or(i64::MAX)
        };
        let nodes = parsed
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                Ok(NodeRow {
                    line: line_of(node.span.start),
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
            tier3: specengine_core::check::is_tier3_file(parsed),
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
            tier3: false,
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

/// The node's own text: the pieces of the core's one split
/// ([`specengine_core::own_spans`]: its body minus every ID section inside
/// it), joined by `\n`. Nested sections are indexed on their own, so a word
/// belongs to exactly one node.
fn own_text(bytes: &[u8], parsed: &ParsedFile, index: usize) -> String {
    let mut pieces: Vec<Cow<'_, str>> = specengine_core::own_spans(parsed, index)
        .into_iter()
        .filter_map(|span| bytes.get(span.range()))
        .map(String::from_utf8_lossy)
        .collect();
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

/// AC-13 of docs/features/phase1-cleanup.md (S3): `own_text` gives what the
/// full scan over every section gave, on every node of the two fixture
/// corpora and a crafted nested file, and stays linear on one file of
/// 50 000 top-level ID sections.
#[cfg(test)]
mod own_text_oracle {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use specengine_core::IdSchemeToml;
    use specengine_model::{IdScheme, PrefixSpec};

    use super::*;

    /// The algorithm before S3, kept as the oracle: every section of the
    /// file is looked at for every node.
    fn full_scan(bytes: &[u8], parsed: &ParsedFile, index: usize) -> String {
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

    fn md_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
            .map(|entry| entry.expect("entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                md_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "md") {
                out.push(path);
            }
        }
    }

    /// Asserts `own_text` equals the oracle on every node (and one past the
    /// last); returns the number of nodes that own a nested section.
    fn assert_same(context: &str, bytes: &[u8], parsed: &ParsedFile) -> usize {
        let mut with_nested = 0;
        for index in 0..=parsed.nodes.len() {
            assert_eq!(
                own_text(bytes, parsed, index),
                full_scan(bytes, parsed, index),
                "{context}: node {index}"
            );
            let Some(node) = parsed.nodes.get(index) else {
                continue;
            };
            let range = if index == 0 {
                parsed.body
            } else {
                node.body.unwrap_or(Span::new(node.span.end, node.span.end))
            };
            if parsed.nodes[index + 1..]
                .iter()
                .any(|section| range.contains(section.span))
            {
                with_nested += 1;
            }
        }
        with_nested
    }

    /// The fixture corpora with a scheme and a `docs/` tree (spec-a and
    /// spec-b), found on disk: `src` names no fixture (`tests/genre.rs`).
    fn scheme_corpora() -> Vec<PathBuf> {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let mut corpora: Vec<PathBuf> = std::fs::read_dir(&fixtures)
            .expect("fixtures")
            .map(|entry| entry.expect("entry").path())
            .filter(|dir| dir.join("specengine.toml").is_file() && dir.join("docs").is_dir())
            .collect();
        corpora.sort();
        corpora
    }

    #[test]
    fn own_text_equals_the_full_scan_on_both_fixture_corpora() {
        let corpora = scheme_corpora();
        assert_eq!(corpora.len(), 2, "{corpora:?}");
        for root in corpora {
            let corpus = root.display().to_string();
            let toml = std::fs::read_to_string(root.join("specengine.toml")).expect("scheme");
            let scheme = IdScheme::from_toml(&toml).expect("fixture scheme");
            let mut files = Vec::new();
            md_files(&root.join("docs"), &mut files);
            let count = files.len();
            assert!(count > 5, "{corpus}: {count} files");
            let mut nodes = 0;
            let mut with_nested = 0;
            for file in files {
                let bytes = std::fs::read(&file).expect("fixture file");
                let parsed = specengine_core::parse("f.md", &bytes, &scheme);
                nodes += parsed.nodes.len();
                with_nested += assert_same(&file.display().to_string(), &bytes, &parsed);
            }
            assert!(nodes > count, "{corpus}: {nodes} nodes, sections too");
            assert!(with_nested > 0, "{corpus}: no node holds a section");
        }
    }

    fn x_scheme() -> IdScheme {
        IdScheme::new(vec![PrefixSpec::number("X", "x", 5)]).expect("scheme")
    }

    /// Nesting three deep, siblings, a non-ID heading between sections, an
    /// empty section right before its sibling, text before the first
    /// section, and a section running to the end of the file.
    const NESTED: &str = "---\nid: X-00001\n---\nIntro text.\n\n# A {#X-00002}\n\nalpha\n\n## A1 {#X-00003}\n\nbravo\n\n### A1a {#X-00004}\n\ncharlie\n\n## Plain heading\n\ndelta\n\n## A2 {#X-00005}\n## A3 {#X-00006}\n\necho\n\n# Plain top\n\nfoxtrot\n\n# B {#X-00007}\n\ngolf\n\n#### B deep {#X-00008}\n\nhotel";

    #[test]
    fn own_text_equals_the_full_scan_on_a_crafted_nested_file() {
        let scheme = x_scheme();
        let crlf = NESTED.replace('\n', "\r\n");
        for (case, text) in [("lf", NESTED.to_owned()), ("crlf", crlf)] {
            let parsed = specengine_core::parse("nested.md", text.as_bytes(), &scheme);
            assert_eq!(parsed.sections().len(), 7, "{case}: {:?}", parsed.nodes);
            let with_nested = assert_same(case, text.as_bytes(), &parsed);
            assert!(
                with_nested >= 4,
                "{case}: {with_nested} nodes hold sections"
            );
            // Each word belongs to exactly one node.
            let texts: Vec<String> = (0..parsed.nodes.len())
                .map(|index| own_text(text.as_bytes(), &parsed, index))
                .collect();
            for word in [
                "Intro", "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
            ] {
                let owners = texts.iter().filter(|own| own.contains(word)).count();
                assert_eq!(owners, 1, "{case}: {word:?} in {texts:#?}");
            }
        }
        // Degenerate inputs: nothing, and no section at all.
        for text in ["", "just text\n", "---\nid: X-00001\n---\n"] {
            let parsed = specengine_core::parse("d.md", text.as_bytes(), &scheme);
            assert_same(text, text.as_bytes(), &parsed);
        }
    }

    /// 50 000 top-level ID sections: `own_text` of every node, the loop
    /// alone (the parse is not timed), within 2 s in a debug build. The
    /// loop gives up at the bound, so a quadratic scan fails fast.
    #[test]
    fn own_text_of_fifty_thousand_sections_is_linear() {
        const SECTIONS: usize = 50_000;
        const BOUND: Duration = Duration::from_secs(2);
        let mut text = String::from("# Top\n\nIntro.\n\n");
        for n in 1..=SECTIONS {
            text.push_str(&format!("# Section {n} {{#X-{n:05}}}\n\nword{n}\n\n"));
        }
        let parsed = specengine_core::parse("big.md", text.as_bytes(), &x_scheme());
        assert_eq!(parsed.sections().len(), SECTIONS);
        let bytes = text.as_bytes();
        let started = Instant::now();
        let mut total = 0;
        for index in 0..parsed.nodes.len() {
            total += own_text(bytes, &parsed, index).len();
            if index % 1_000 == 0 {
                assert!(
                    started.elapsed() <= BOUND,
                    "own_text of {index} of {} nodes took {:?} (> {BOUND:?})",
                    parsed.nodes.len(),
                    started.elapsed()
                );
            }
        }
        let elapsed = started.elapsed();
        eprintln!("own_text of {} nodes: {elapsed:?}", parsed.nodes.len());
        assert!(elapsed <= BOUND, "own_text loop: {elapsed:?} (> {BOUND:?})");
        assert!(total > SECTIONS * 5, "texts were built: {total} bytes");
        // Spot checks against the oracle: the first, a middle and the last node.
        for index in [0, 1, SECTIONS / 2, SECTIONS] {
            assert_eq!(
                own_text(bytes, &parsed, index),
                full_scan(bytes, &parsed, index),
                "node {index}"
            );
        }
    }
}
