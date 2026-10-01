//! The generated documentation index (§9 of the documentation convention),
//! rendered from a set of parses: the header with the registered command
//! and gate, then one line per document in sections `Canon`, `Decisions`,
//! `Specs`, `No class — fix` and `Archive — Tier 3, by id only`, paths in
//! byte order, empty sections omitted. Pure: the render stays in memory;
//! the §11.5 rule compares it with the walked bytes of `[paths] index`.
//!
//! A live line is `- [label](link) title · scope · status`; a Tier 3 line
//! only `- [label](link) status`, the status whole (`superseded-by <id>`),
//! title and scope not read. Which documents are Tier 3 (excluded from
//! default retrieval, listed in the archive) is decided here once, by
//! status, never by folder: a spec `shipped` or `abandoned`, a decision
//! with a `status:` other than `accepted`. The archive section, the compact
//! line and the graph rules' live sources all read the same predicate.
//!
//! With shards the index is a set of outputs: the root, then the
//! shards of the index entry in config order. Each listed document goes to
//! one output — a Tier 3 document to the archive shard if configured, else
//! to the first shard with a matching claim, else to the root — and every
//! output keeps the sections and line formats above, its links relative to
//! its own directory. The root ends with one pointer per shard; no output is
//! listed in any output. No shard: the single file, byte for byte.

use std::fmt::Write as _;

use specengine_model::{Fields, ParsedFile};

use super::config::{DocClass, Generator, Shard, ShardKind};
use super::input::{CheckFile, CheckInput};
use super::text::front_matter_failed;
use crate::glob::Glob;

/// The section a document is listed in; `None`: not listed (generated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Canon,
    Decisions,
    Specs,
    NoClass,
    Archive,
}

/// The Archive section's title, also the archive shard's label (the
/// section it holds).
const ARCHIVE_TITLE: &str = "Archive — Tier 3, by id only";

/// The section headings, in order.
const SECTIONS: [(Section, &str); 5] = [
    (Section::Canon, "Canon"),
    (Section::Decisions, "Decisions"),
    (Section::Specs, "Specs"),
    (Section::NoClass, "No class — fix"),
    (Section::Archive, ARCHIVE_TITLE),
];

/// The title of the root's pointer section, present only with shards.
const SHARDS_TITLE: &str = "Shards";

/// One output of the index: the root or a shard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexOutput {
    /// Root-relative: `[paths] index` or a shard's `path`.
    pub path: String,
    /// The render: byte for byte what the generator writes there.
    pub bytes: String,
}

/// The root of the index as the file `index_path` (root-relative), built by
/// `generator`: the first output of [`render_index_set`]. Without shards,
/// the whole index.
pub fn render_index(input: &CheckInput, index_path: &str, generator: &Generator) -> String {
    render_index_set(input, index_path, generator)
        .into_iter()
        .next()
        .map(|output| output.bytes)
        .unwrap_or_default()
}

/// Every output of the index of `input`, built by `generator`: the root at
/// `index_path` (root-relative) first, then each of `generator.shards` in
/// config order. Every walked document but a generated one is listed in
/// exactly one output; no output is listed, whatever its front-matter. The
/// result is independent of the order of `input.files`.
pub fn render_index_set(
    input: &CheckInput,
    index_path: &str,
    generator: &Generator,
) -> Vec<IndexOutput> {
    let shards = &generator.shards;
    let is_output =
        |path: &str| path == index_path || shards.iter().any(|shard| shard.path == path);
    let mut files: Vec<&CheckFile> = input
        .files
        .iter()
        .filter(|file| !is_output(&file.path))
        .collect();
    files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    // Placement: a Tier 3 document to the archive shard if one is
    // configured, else to the first shard with a matching claim, else to the
    // root. Per output (0: the root, 1 + i: shard i), its documents.
    let claims: Vec<Vec<Glob>> = shards
        .iter()
        .map(|shard| match &shard.kind {
            ShardKind::Tier3 => Vec::new(),
            ShardKind::Claims(globs) => globs.iter().map(|glob| Glob::new(glob)).collect(),
        })
        .collect();
    let archive = shards.iter().position(Shard::is_archive);
    let mut placed: Vec<Vec<(Section, &CheckFile)>> = vec![Vec::new(); 1 + shards.len()];
    for file in files {
        let Some(section) = section_of(file) else {
            continue;
        };
        let shard = match archive {
            Some(at) if section == Section::Archive => Some(at),
            _ => claims
                .iter()
                .position(|globs| globs.iter().any(|glob| glob.matches(&file.path))),
        };
        placed[shard.map_or(0, |at| at + 1)].push((section, file));
    }

    let mut outputs = Vec::with_capacity(1 + shards.len());
    let mut root = header(generator, "Documentation index");
    root.push_str(
        "Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.\n",
    );
    sections(&mut root, index_path, &placed[0]);
    if !shards.is_empty() {
        let _ = write!(root, "\n## {SHARDS_TITLE}\n\n");
        for shard in shards {
            let link = relative_link(index_path, &shard.path);
            let _ = writeln!(root, "- [{}]({link}) {}", shard.path, label(shard));
        }
    }
    outputs.push(IndexOutput {
        path: index_path.to_owned(),
        bytes: root,
    });
    for (shard, documents) in shards.iter().zip(&placed[1..]) {
        let mut out = header(generator, &format!("Documentation index: {}", label(shard)));
        let back = relative_link(&shard.path, index_path);
        let _ = writeln!(
            out,
            "A shard of [{index_path}]({back}), the index's one entry point."
        );
        sections(&mut out, &shard.path, documents);
        outputs.push(IndexOutput {
            path: shard.path.clone(),
            bytes: out,
        });
    }
    outputs
}

/// Front-matter, the H1 `title` and the build comment, then a blank line.
fn header(generator: &Generator, title: &str) -> String {
    let command = &generator.command;
    let gate = generator.gate();
    format!(
        "---\nclass: generated\ngenerator: {command}\nsource: front-matter of the repository's documents\n---\n\n\
         # {title}\n\n\
         <!-- Built by `{command}`. Manual edits are overwritten on rebuild, and `{gate}` rejects them. -->\n\n"
    )
}

/// Each non-empty section of `documents` (in path order) as `\n## <name>\n\n`
/// plus one line per document, linked from the directory of `output`.
fn sections(out: &mut String, output: &str, documents: &[(Section, &CheckFile)]) {
    for (section, title) in SECTIONS {
        let mut lines = documents
            .iter()
            .filter(|(placed, _)| *placed == section)
            .peekable();
        if lines.peek().is_none() {
            continue;
        }
        let _ = write!(out, "\n## {title}\n\n");
        for (_, file) in lines {
            out.push_str(&line(file, output));
            out.push('\n');
        }
    }
}

/// A shard's label: the archive's section title, else its claims in
/// backticks joined by `, `.
fn label(shard: &Shard) -> String {
    match &shard.kind {
        ShardKind::Tier3 => ARCHIVE_TITLE.to_owned(),
        ShardKind::Claims(globs) => globs
            .iter()
            .map(|glob| format!("`{glob}`"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn section_of(file: &CheckFile) -> Option<Section> {
    let Some(fields) = file.parsed.as_ref().and_then(readable_fields) else {
        return Some(Section::NoClass);
    };
    if is_tier3(fields) {
        return Some(Section::Archive);
    }
    match fields.class.as_deref().and_then(DocClass::parse) {
        Some(DocClass::Canon) => Some(Section::Canon),
        Some(DocClass::Decision) => Some(Section::Decisions),
        Some(DocClass::Spec) => Some(Section::Specs),
        Some(DocClass::Generated) => None,
        None => Some(Section::NoClass),
    }
}

/// `- [label](link) title · scope · status`; Tier 3: `- [label](link) status`.
fn line(file: &CheckFile, index_path: &str) -> String {
    let parsed = file.parsed.as_ref();
    let document = parsed.and_then(|parsed| parsed.document());
    let fields = parsed.and_then(readable_fields);
    let class = fields
        .and_then(|fields| fields.class.as_deref())
        .and_then(DocClass::parse);
    let label = match (class, document.and_then(|document| document.id.as_deref())) {
        (Some(DocClass::Decision), Some(id)) => id,
        _ => file.path.as_str(),
    };
    let link = relative_link(index_path, &file.path);
    let status = match class {
        Some(DocClass::Canon) => match fields.and_then(|fields| fields.tier) {
            Some(tier) => format!("tier {tier}"),
            None => "tier ?".to_owned(),
        },
        _ => fields
            .and_then(|fields| fields.status.as_deref())
            .unwrap_or("?")
            .to_owned(),
    };
    if fields.is_some_and(is_tier3) {
        return format!("- [{label}]({link}) {status}");
    }
    let title = document
        .and_then(|document| document.title.as_deref())
        .unwrap_or("?");
    let scope = fields
        .and_then(|fields| fields.scope.as_ref())
        .map(|scope| scope.join(", "))
        .unwrap_or_default();
    format!("- [{label}]({link}) {title} · {scope} · {status}")
}

/// `target` relative to the directory of the file `from`: the common
/// leading directories dropped, one `..` per remaining directory of
/// `from` (`a/b/x.md` → `c.md`: `../../c.md`; → `a/c/y.md`: `../c/y.md`).
fn relative_link(from: &str, target: &str) -> String {
    let from_dirs: Vec<&str> = from.split('/').collect();
    let from_dirs = &from_dirs[..from_dirs.len() - 1];
    let target_parts: Vec<&str> = target.split('/').collect();
    let target_dirs = &target_parts[..target_parts.len() - 1];
    let common = from_dirs
        .iter()
        .zip(target_dirs)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from_dirs.len() - common];
    parts.extend_from_slice(&target_parts[common..]);
    parts.join("/")
}

/// The typed front-matter keys when the front-matter was read.
pub(crate) fn readable_fields(parsed: &ParsedFile) -> Option<&Fields> {
    if front_matter_failed(parsed) {
        return None;
    }
    parsed.document()?.fields.as_ref()
}

/// Tier 3, by status: a spec `shipped` or `abandoned`, a decision with a
/// `status:` other than `accepted`.
pub fn is_tier3(fields: &Fields) -> bool {
    let status = fields.status.as_deref();
    match fields.class.as_deref().and_then(DocClass::parse) {
        Some(DocClass::Spec) => matches!(status, Some("shipped" | "abandoned")),
        Some(DocClass::Decision) => status.is_some_and(|status| status != "accepted"),
        _ => false,
    }
}

/// The file is Tier 3 ([`is_tier3`] of its typed front-matter keys): the
/// one predicate of the generated index's archive, the index's `tier3`
/// column and the archive filter of `spec search`. `false` when the
/// front-matter failed or the file has no document (not UTF-8).
pub fn is_tier3_file(parsed: &ParsedFile) -> bool {
    readable_fields(parsed).is_some_and(is_tier3)
}

/// A live source of the graph rules: neither `class: generated` nor Tier 3.
/// A document whose front-matter failed is live.
pub fn is_live(parsed: &ParsedFile) -> bool {
    readable_fields(parsed).is_none_or(|fields| {
        !is_tier3(fields) && fields.class.as_deref() != Some(DocClass::Generated.as_str())
    })
}
