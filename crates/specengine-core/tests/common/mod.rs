//! Shared helpers of the `specengine-core` integration tests
//! (docs/features/spec-parser.md): fixture corpora read from `fixtures/`,
//! schemes read through their own `specengine.toml`, derived CRLF / BOM
//! variants, and a compact rendering of links for `expected.json`.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024);
//! the fixtures carry short self-written Russian samples (spec Q5).

#![allow(dead_code)]

pub mod check;

use std::fs;
use std::path::{Path, PathBuf};

use specengine_core::IdSchemeToml;
use specengine_model::{IdScheme, Link, LinkTarget, ParsedFile, PrefixSpec, Reference};

pub const BOM: &[u8] = b"\xEF\xBB\xBF";

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

pub fn fixture(name: &str) -> PathBuf {
    repository_root().join("fixtures").join(name)
}

/// The corpus's scheme, read through its `specengine.toml` alone (ADR-0008).
pub fn corpus_scheme(corpus: &Path) -> IdScheme {
    let path = corpus.join("specengine.toml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    IdScheme::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")))
}

/// Every `.md` file under `dir`, corpus-relative with `/`, sorted, with bytes.
pub fn md_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(dir).expect("readable directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|ext| ext == "md") {
                let relative = path
                    .strip_prefix(root)
                    .expect("under root")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((relative, fs::read(&path).expect("readable file")));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "no .md files under {}", dir.display());
    out
}

/// A scheme of `number` prefixes, width 2, kind = lowercase prefix.
pub fn numbers(prefixes: &[&str]) -> IdScheme {
    IdScheme::new(
        prefixes
            .iter()
            .map(|p| PrefixSpec::number(*p, p.to_lowercase(), 2))
            .collect(),
    )
    .expect("valid scheme")
}

pub fn scheme(entries: Vec<PrefixSpec>) -> IdScheme {
    IdScheme::new(entries).expect("valid scheme")
}

pub fn parse_str(text: &str, scheme: &IdScheme) -> ParsedFile {
    specengine_core::parse("test.md", text.as_bytes(), scheme)
}

/// LF → CRLF, byte for byte otherwise.
pub fn to_crlf(bytes: &[u8]) -> Vec<u8> {
    assert!(
        !bytes.windows(2).any(|w| w == b"\r\n"),
        "the LF original already holds CRLF"
    );
    let mut out = Vec::with_capacity(bytes.len() + bytes.len() / 16);
    for &byte in bytes {
        if byte == b'\n' {
            out.push(b'\r');
        }
        out.push(byte);
    }
    out
}

pub fn with_bom(bytes: &[u8]) -> Vec<u8> {
    let mut out = BOM.to_vec();
    out.extend_from_slice(bytes);
    out
}

/// The LF original and its derived variants: CRLF, BOM, CRLF+BOM.
pub fn variants(lf: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("lf", lf.to_vec()),
        ("crlf", to_crlf(lf)),
        ("bom", with_bom(lf)),
        ("crlf+bom", with_bom(&to_crlf(lf))),
    ]
}

/// A reference rendered as the grammar writes it; wiki form in `[[…]]`.
pub fn render_reference(reference: &Reference) -> String {
    let mut out = String::new();
    if let Some(project) = &reference.project {
        out.push_str(project);
        out.push(':');
    }
    if let Some(scope) = &reference.scope {
        out.push_str(scope);
        out.push('/');
    }
    out.push_str(&reference.id);
    if let Some(section) = &reference.section {
        out.push('#');
        out.push_str(section);
    }
    if let Some(rev) = reference.rev {
        out.push('@');
        out.push_str(&rev.to_string());
    }
    if reference.form == specengine_model::RefForm::Wiki {
        out = match &reference.label {
            Some(label) => format!("[[{out}|{label}]]"),
            None => format!("[[{out}]]"),
        };
    }
    out
}

/// `"<src|-> <type> <origin> <dst>"`, dst with ` (alias of P)` and
/// ` {script}` when not Latin.
pub fn render_link(link: &Link) -> String {
    let src = link.src.as_deref().unwrap_or("-");
    let origin = match link.origin {
        specengine_model::LinkOrigin::Frontmatter => "frontmatter",
        specengine_model::LinkOrigin::Inline => "inline",
    };
    let dst = match &link.dst {
        LinkTarget::Reference(reference) => {
            let mut out = render_reference(reference);
            if let Some(prefix) = &reference.alias_of {
                out.push_str(&format!(" (alias of {prefix})"));
            }
            match reference.script {
                specengine_model::IdScript::Latin => {}
                specengine_model::IdScript::Mixed => out.push_str(" {mixed}"),
                specengine_model::IdScript::NonLatin => out.push_str(" {non-latin}"),
            }
            out
        }
        LinkTarget::Path(path) => match &path.anchor {
            Some(anchor) => format!("{}#{anchor}", path.path),
            None => path.path.clone(),
        },
    };
    format!("{src} {} {origin} {dst}", link.link_type)
}

/// `"code:line"` of every diagnostic, in order.
pub fn render_diagnostics(parsed: &ParsedFile) -> Vec<String> {
    parsed
        .diagnostics
        .iter()
        .map(|d| format!("{}:{}", d.code, d.line))
        .collect()
}

/// Text of a span of `bytes` (panics when the span is out of range or cuts a
/// UTF-8 character).
pub fn text_of(bytes: &[u8], span: specengine_model::Span) -> &str {
    let slice = bytes
        .get(span.range())
        .unwrap_or_else(|| panic!("span {span:?} outside a {}-byte file", bytes.len()));
    std::str::from_utf8(slice).unwrap_or_else(|_| panic!("span {span:?} cuts a UTF-8 character"))
}

/// Canonical JSON of a parse.
pub fn json(parsed: &ParsedFile) -> String {
    serde_json::to_string(parsed).expect("ParsedFile serialises")
}
