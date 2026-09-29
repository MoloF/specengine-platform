//! The generated documentation index (§9 of the documentation convention),
//! rendered from a set of parses: the header with the registered command
//! and gate, then one line per document in sections `Canon`, `Decisions`,
//! `Specs`, `No class — fix` and `Archive — Tier 3, by id only`, paths in
//! byte order, empty sections omitted. Pure: the render stays in memory;
//! the §11.5 rule compares it with the walked bytes of `[paths] index`.
//!
//! Which documents are Tier 3 (excluded from default retrieval, listed in
//! the archive) is decided here once, by status, never by folder: a spec
//! `shipped` or `abandoned`, a decision with a `status:` other than
//! `accepted`. The graph rules read their live sources through the same
//! predicate.

use std::fmt::Write as _;

use specengine_model::{Fields, ParsedFile};

use super::config::{DocClass, Generator};
use super::input::{CheckFile, CheckInput};
use super::text::front_matter_failed;

/// The section a document is listed in; `None`: not listed (generated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Canon,
    Decisions,
    Specs,
    NoClass,
    Archive,
}

/// The section headings, in order.
const SECTIONS: [(Section, &str); 5] = [
    (Section::Canon, "Canon"),
    (Section::Decisions, "Decisions"),
    (Section::Specs, "Specs"),
    (Section::NoClass, "No class — fix"),
    (Section::Archive, "Archive — Tier 3, by id only"),
];

/// The index of `input` as the file `index_path` (root-relative), built by
/// `generator`: byte for byte what the convention's generator writes. The
/// index itself and every other generated document are not listed; the
/// result is independent of the order of `input.files`.
pub fn render_index(input: &CheckInput, index_path: &str, generator: &Generator) -> String {
    let mut files: Vec<&CheckFile> = input
        .files
        .iter()
        .filter(|file| file.path != index_path)
        .collect();
    files.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));

    let mut out = header(generator);
    for (section, title) in SECTIONS {
        let lines: Vec<String> = files
            .iter()
            .filter(|file| section_of(file) == Some(section))
            .map(|file| line(file, index_path))
            .collect();
        if lines.is_empty() {
            continue;
        }
        let _ = write!(out, "\n## {title}\n\n");
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// Front-matter, H1, the build comment and the reading protocol line.
fn header(generator: &Generator) -> String {
    let command = &generator.command;
    let gate = generator.gate();
    format!(
        "---\nclass: generated\ngenerator: {command}\nsource: front-matter of the repository's documents\n---\n\n\
         # Documentation index\n\n\
         <!-- Built by `{command}`. Manual edits are overwritten on rebuild, and `{gate}` rejects them. -->\n\n\
         Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.\n"
    )
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

/// `- [label](link) title · scope · status`.
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
    let title = document
        .and_then(|document| document.title.as_deref())
        .unwrap_or("?");
    let scope = fields
        .and_then(|fields| fields.scope.as_ref())
        .map(|scope| scope.join(", "))
        .unwrap_or_default();
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
pub(crate) fn is_tier3(fields: &Fields) -> bool {
    let status = fields.status.as_deref();
    match fields.class.as_deref().and_then(DocClass::parse) {
        Some(DocClass::Spec) => matches!(status, Some("shipped" | "abandoned")),
        Some(DocClass::Decision) => status.is_some_and(|status| status != "accepted"),
        _ => false,
    }
}

/// A live source of the graph rules: neither `class: generated` nor Tier 3.
/// A document whose front-matter failed is live.
pub(crate) fn is_live(parsed: &ParsedFile) -> bool {
    readable_fields(parsed).is_none_or(|fields| {
        !is_tier3(fields) && fields.class.as_deref() != Some(DocClass::Generated.as_str())
    })
}
