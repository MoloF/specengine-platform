//! AC-04 of docs/features/spec-parser.md: spans are byte offsets into the
//! original file, BOM included, CRLF never normalised.
//!
//! For every fixture file (spec-a, spec-b, corpus-mini, token-calibration)
//! and crafted inputs, plus derived CRLF, BOM and CRLF+BOM variants:
//! `&bytes[span]` is the verbatim text of every node, heading, body and
//! reference; BOM + front-matter + body rebuild the file; and the variant's
//! parse equals the LF parse with every span moved by exactly the bytes the
//! variant inserted before it (3 for the BOM, one `\r` per earlier `\n`).

mod common;

use specengine_model::script::normalize_char;
use specengine_model::{
    AnchorOrigin, CanonTarget, IdScheme, LinkTarget, Node, ParsedFile, PathTarget, PrefixSpec,
    Reference, Span,
};

use common::{BOM, corpus_scheme, fixture, md_files, render_reference, text_of, variants};

/// Crafted inputs: layouts the fixtures do not cover.
const CRAFTED: [(&str, &str); 12] = [
    ("empty", ""),
    ("only-fence", "---\n"),
    ("empty-front-matter", "---\n---\n"),
    ("front-matter-no-body", "---\nid: X-1\n---"),
    ("unclosed", "---\nid: X-1\n# Title {#X-2}\n"),
    (
        "no-final-newline",
        "---\nid: X-1\n---\n# T\n\n## A {#X-2}\ntext X-3",
    ),
    ("heading-at-eof", "# T\n\n## A {#X-2}"),
    (
        "setext",
        "Title {#X-1}\n=====\n\nBody X-2.\n\nSub {#X-3}\n---\n\nMore.\n",
    ),
    (
        "trailing-space-and-closing-hashes",
        "## A {#X-1}   \n\ntext  \n\n## B ## {#X-2}\n\n\n\n",
    ),
    (
        "nested-and-code",
        "# D\n\n## A {#X-1 rev=2 .c k=v}\n\n    X-9 indented\n\n### B {#X-2}\n\n> quote X-4\n\n| X-5 | `X-6` |\n|---|---|\n| a | b |\n\n## C\n",
    ),
    (
        "wiki-and-qualifiers",
        "---\nid: X-1\nparent: X-2\nlinks:\n  depends_on: [X-3, 'X-4']\nsupersedes:\n  - X-5\nstatus: superseded-by X-6\ncanon: docs/a.md#b\n---\n\nsee [[s/X-7#X-8@3|label]] and p:X-9@2, [[X-10]].\n",
    ),
    (
        "homoglyph",
        "---\nid: X\u{2011}1\n---\n\nX-1 and \u{0425}-2 and \u{FF38}-3.\n\n## A {#\u{0425}-4}\n",
    ),
];

fn x_scheme() -> IdScheme {
    IdScheme::new(vec![PrefixSpec::number("X", "x", 1)]).expect("valid scheme")
}

/// Every input: (name, LF bytes, scheme).
fn inputs() -> Vec<(String, Vec<u8>, IdScheme)> {
    let mut out = Vec::new();
    for corpus in ["spec-a", "spec-b", "corpus-mini"] {
        let dir = fixture(corpus);
        let scheme = corpus_scheme(&dir);
        for (path, bytes) in md_files(&dir) {
            out.push((format!("{corpus}/{path}"), bytes, scheme.clone()));
        }
    }
    let spec_a = corpus_scheme(&fixture("spec-a"));
    for (path, bytes) in md_files(&fixture("token-calibration")) {
        out.push((format!("token-calibration/{path}"), bytes, spec_a.clone()));
    }
    for (name, text) in CRAFTED {
        out.push((
            format!("crafted/{name}"),
            text.as_bytes().to_vec(),
            x_scheme(),
        ));
    }
    out
}

// ------------------------------------------------------------ offset mapping

/// Maps an offset of the LF original into a variant.
struct Shift {
    bom: usize,
    /// `newlines[i]` = number of `\n` in `lf[..i]` (when CRLF).
    newlines: Option<Vec<usize>>,
}

impl Shift {
    fn new(lf: &[u8], crlf: bool, bom: bool) -> Self {
        let newlines = crlf.then(|| {
            let mut counts = Vec::with_capacity(lf.len() + 1);
            let mut n = 0;
            counts.push(0);
            for &byte in lf {
                if byte == b'\n' {
                    n += 1;
                }
                counts.push(n);
            }
            counts
        });
        Self {
            bom: if bom { BOM.len() } else { 0 },
            newlines,
        }
    }

    fn at(&self, offset: usize) -> usize {
        // Offsets of the LF parse are already past its own BOM (none).
        offset + self.bom + self.newlines.as_ref().map_or(0, |n| n[offset])
    }

    fn span(&self, span: Span) -> Span {
        Span::new(self.at(span.start), self.at(span.end))
    }

    fn opt(&self, span: Option<Span>) -> Option<Span> {
        span.map(|s| self.span(s))
    }

    fn reference(&self, reference: &mut Reference) {
        reference.span = self.opt(reference.span);
    }

    fn path(&self, path: &mut PathTarget) {
        path.span = self.opt(path.span);
    }

    fn node(&self, node: &mut Node) {
        node.summary = self.opt(node.summary);
        node.heading = self.opt(node.heading);
        node.body = self.opt(node.body);
        node.span = self.span(node.span);
        node.tokens_est = 0;
        if let Some(parent) = &mut node.parent {
            parent.span = self.opt(parent.span);
        }
        if let Some(fields) = &mut node.fields {
            if let Some(r) = &mut fields.working_answer {
                self.reference(r);
            }
            match &mut fields.canon {
                Some(CanonTarget::Reference(r)) => self.reference(r),
                Some(CanonTarget::Path(p)) => self.path(p),
                None => {}
            }
            for list in [&mut fields.supersedes, &mut fields.adrs, &mut fields.refs]
                .into_iter()
                .flatten()
            {
                for r in list {
                    self.reference(r);
                }
            }
            if let Some(links) = &mut fields.links {
                for (_, list) in &mut links.0 {
                    for r in list {
                        self.reference(r);
                    }
                }
            }
        }
    }

    /// The LF parse as the variant's parse must be.
    fn parsed(&self, lf: &ParsedFile) -> ParsedFile {
        let mut out = lf.clone();
        out.bom = self.bom > 0;
        out.front_matter = self.opt(out.front_matter);
        // The body of a file without front-matter starts after the BOM.
        out.body = Span::new(
            if lf.body.start == 0 {
                self.bom
            } else {
                self.at(lf.body.start)
            },
            self.at(lf.body.end),
        );
        for node in &mut out.nodes {
            self.node(node);
        }
        if let Some(document) = out.nodes.first_mut() {
            // The document is the whole file, BOM included.
            document.span = Span::new(0, self.at(lf.nodes[0].span.end));
        }
        for link in &mut out.links {
            link.src_span = self.opt(link.src_span);
            match &mut link.dst {
                LinkTarget::Reference(r) => self.reference(r),
                LinkTarget::Path(p) => self.path(p),
            }
        }
        for anchor in &mut out.anchors {
            anchor.span = self.span(anchor.span);
        }
        for diagnostic in &mut out.diagnostics {
            diagnostic.span = self.opt(diagnostic.span);
        }
        out
    }
}

fn without_tokens(parsed: &ParsedFile) -> ParsedFile {
    let mut out = parsed.clone();
    for node in &mut out.nodes {
        node.tokens_est = 0;
    }
    out
}

// ------------------------------------------------------------ verbatim checks

fn normalized(text: &str) -> String {
    text.chars().map(normalize_char).collect()
}

/// Every span of `parsed` lies in `bytes` on char boundaries and holds the
/// text it claims to.
fn assert_verbatim(name: &str, bytes: &[u8], parsed: &ParsedFile) {
    let len = bytes.len();
    // BOM + front-matter + body = the file.
    let lead = if parsed.bom { BOM.len() } else { 0 };
    assert_eq!(parsed.bom, bytes.starts_with(BOM), "{name}: bom flag");
    let mut rebuilt = bytes[..lead].to_vec();
    let mut next = lead;
    if let Some(front) = parsed.front_matter {
        assert_eq!(
            front.start, lead,
            "{name}: front-matter starts after the BOM"
        );
        assert!(
            text_of(bytes, front).starts_with("---"),
            "{name}: front-matter opens with ---"
        );
        rebuilt.extend_from_slice(&bytes[front.range()]);
        next = front.end;
    }
    assert_eq!(
        parsed.body.start, next,
        "{name}: body follows the front-matter"
    );
    assert_eq!(parsed.body.end, len, "{name}: body runs to the end");
    rebuilt.extend_from_slice(&bytes[parsed.body.range()]);
    assert_eq!(rebuilt, bytes, "{name}: BOM + front-matter + body = file");

    let Some(document) = parsed.document() else {
        return;
    };
    assert_eq!(
        document.span,
        Span::new(0, len),
        "{name}: document span is the file"
    );
    if let Some(summary) = document.summary {
        let text = text_of(bytes, summary);
        assert!(
            !text.is_empty() && text.trim() == text,
            "{name}: summary {text:?} is a trimmed paragraph"
        );
        assert!(parsed.body.contains(summary), "{name}: summary in the body");
    }
    if let (Some(parent), Some(span)) = (
        &document.parent,
        document.parent.as_ref().and_then(|p| p.span),
    ) {
        assert_eq!(
            normalized(text_of(bytes, span)),
            parent.id,
            "{name}: parent span holds the parent ID"
        );
    }
    for section in parsed.sections() {
        let id = section.id.as_deref().unwrap_or("?");
        let heading = section.heading.expect("a section has a heading span");
        let body = section.body.expect("a section has a body span");
        let heading_text = text_of(bytes, heading);
        let level = usize::from(section.level.expect("level"));
        assert!(
            !heading_text.contains(['\r', '\n']) || heading_text.lines().count() == 2,
            "{name}/{id}: heading {heading_text:?} excludes the line ending"
        );
        assert!(
            !heading_text.ends_with([' ', '\t', '\r', '\n']),
            "{name}/{id}: heading {heading_text:?} excludes trailing whitespace"
        );
        let atx = "#".repeat(level) + " ";
        let setext = heading_text.lines().count() == 2;
        assert!(
            heading_text.starts_with(&atx) || setext,
            "{name}/{id}: heading {heading_text:?} is the heading line"
        );
        assert!(
            heading_text.contains("{#"),
            "{name}/{id}: heading {heading_text:?} holds its attribute block"
        );
        assert_eq!(
            section.span.start, heading.start,
            "{name}/{id}: span starts at the heading"
        );
        assert_eq!(
            section.span.end, body.end,
            "{name}/{id}: span ends with the body"
        );
        assert!(
            heading.end <= body.start,
            "{name}/{id}: heading and body are disjoint"
        );
        let between = text_of(bytes, Span::new(heading.end, body.start));
        assert!(
            ["", "\n", "\r\n"].contains(&between) || between.trim().is_empty(),
            "{name}/{id}: body starts on the line after the heading, got {between:?} in between"
        );
        let whole = text_of(bytes, section.span);
        assert_eq!(
            whole.trim_end(),
            whole,
            "{name}/{id}: section span excludes trailing whitespace"
        );
        assert!(
            parsed.body.contains(section.span),
            "{name}/{id}: in the body"
        );
    }
    for anchor in &parsed.anchors {
        let text = text_of(bytes, anchor.span);
        match anchor.origin {
            AnchorOrigin::Attr => assert!(
                text.contains(&format!("{{#{}", anchor.name)),
                "{name}: attr anchor heading {text:?} holds {{#{}",
                anchor.name
            ),
            AnchorOrigin::Slug => {
                // An ATX heading line, or a setext heading through its underline.
                let atx = text.trim_start().starts_with('#');
                let setext = text.lines().count() >= 2
                    && text.lines().last().is_some_and(|underline| {
                        let underline = underline.trim();
                        !underline.is_empty()
                            && (underline.chars().all(|c| c == '=')
                                || underline.chars().all(|c| c == '-'))
                    });
                assert!(
                    (atx || setext) && anchor.level.is_some_and(|level| (1..=6).contains(&level)),
                    "{name}: slug anchor {:?} spans {text:?}, which is no heading",
                    anchor.name
                );
            }
            AnchorOrigin::Html => {
                assert!(
                    text.starts_with('<') && text.ends_with('>') && text.contains(&anchor.name),
                    "{name}: html anchor {:?} spans {text:?}, not its start tag",
                    anchor.name
                );
                assert!(anchor.level.is_none(), "{name}: html anchors have no level");
            }
        }
    }
    let check_reference = |what: &str, reference: &Reference| {
        if let Some(span) = reference.span {
            let text = text_of(bytes, span);
            let want = render_reference(reference);
            if text != want {
                // Only a look-alike ID may differ from its Latin rendering.
                assert!(
                    reference.alias_of.is_none()
                        && reference.script != specengine_model::IdScript::Latin
                        && normalized(text) == normalized(&want),
                    "{name}: {what} span holds {text:?}, the reference renders as {want:?}"
                );
            }
        }
    };
    for link in &parsed.links {
        match &link.dst {
            LinkTarget::Reference(reference) => {
                if link.origin == specengine_model::LinkOrigin::Inline {
                    assert!(
                        reference.span.is_some(),
                        "{name}: inline reference has a span"
                    );
                }
                check_reference("link", reference);
            }
            LinkTarget::Path(path) => {
                if let Some(span) = path.span {
                    let want = match &path.anchor {
                        Some(anchor) => format!("{}#{anchor}", path.path),
                        None => path.path.clone(),
                    };
                    assert_eq!(text_of(bytes, span), want, "{name}: canon path span");
                }
            }
        }
        if let Some(span) = link.src_span {
            assert_eq!(
                Some(normalized(text_of(bytes, span))),
                link.src,
                "{name}: src_span holds the source ID"
            );
        }
    }
    if let Some(fields) = &document.fields {
        for reference in fields
            .working_answer
            .iter()
            .chain(fields.supersedes.iter().flatten())
            .chain(fields.adrs.iter().flatten())
            .chain(fields.refs.iter().flatten())
            .chain(
                fields
                    .links
                    .iter()
                    .flat_map(|m| m.0.iter().flat_map(|(_, l)| l.iter())),
            )
        {
            check_reference("front-matter field", reference);
        }
    }
    for diagnostic in &parsed.diagnostics {
        assert!(diagnostic.line >= 1, "{name}: 1-based line");
        if let Some(span) = diagnostic.span {
            let text = text_of(bytes, span);
            if let Some(fix) = &diagnostic.fix {
                assert_eq!(
                    &normalized(text),
                    fix,
                    "{name}: homoglyph span holds the ID"
                );
                assert_ne!(text, fix, "{name}: the span holds look-alikes");
            }
        }
    }
}

#[test]
fn spans_are_verbatim_in_every_fixture_and_variant() {
    let mut checked = 0;
    for (name, lf, scheme) in inputs() {
        for (variant, bytes) in variants(&lf) {
            let parsed = specengine_core::parse(&name, &bytes, &scheme);
            assert_verbatim(&format!("{name} [{variant}]"), &bytes, &parsed);
            checked += 1;
        }
    }
    assert!(checked >= 4 * 40, "only {checked} inputs checked");
}

#[test]
fn variants_shift_every_span_by_exactly_the_inserted_bytes() {
    let mut failures = Vec::new();
    for (name, lf, scheme) in inputs() {
        let original = specengine_core::parse(&name, &lf, &scheme);
        for (variant, bytes) in variants(&lf).into_iter().skip(1) {
            let crlf = variant.contains("crlf");
            let bom = variant.contains("bom");
            let want = Shift::new(&lf, crlf, bom).parsed(&original);
            let got = without_tokens(&specengine_core::parse(&name, &bytes, &scheme));
            if got != want {
                failures.push(format!(
                    "{name} [{variant}]:\n   got: {}\n  want: {}",
                    common::json(&got),
                    common::json(&want)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The section texts of a CRLF file are the LF texts with `\r` before every
/// `\n`: nothing was normalised before the offsets were taken.
#[test]
fn crlf_section_texts_keep_their_carriage_returns() {
    let dir = fixture("spec-a");
    let scheme = corpus_scheme(&dir);
    let (path, lf) = md_files(&dir)
        .into_iter()
        .find(|(p, _)| p.ends_with("sprint.md"))
        .expect("sprint.md");
    let crlf = common::to_crlf(&lf);
    let a = specengine_core::parse(&path, &lf, &scheme);
    let b = specengine_core::parse(&path, &crlf, &scheme);
    assert_eq!(a.sections().len(), 2);
    for (x, y) in a.sections().iter().zip(b.sections()) {
        let lf_text = text_of(&lf, x.span);
        let crlf_text = text_of(&crlf, y.span);
        assert!(crlf_text.contains("\r\n"), "CRLF kept in {crlf_text:?}");
        assert_eq!(crlf_text.replace("\r\n", "\n"), lf_text);
        let body = text_of(&crlf, y.body.unwrap());
        assert!(
            !body.starts_with('\n'),
            "body starts after the heading's \\r\\n"
        );
    }
}

#[test]
fn not_utf8_keeps_the_layout_invariant_and_names_the_line() {
    for (variant, mut bytes) in variants(b"---\nid: X-1\n---\n# T\n\nline three of body \n") {
        let at = bytes.len() - 2;
        bytes[at] = 0xFF;
        let parsed = specengine_core::parse("bad.md", &bytes, &x_scheme());
        assert!(parsed.nodes.is_empty(), "[{variant}] no nodes");
        assert_eq!(parsed.diagnostics.len(), 1, "[{variant}]");
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.code.as_str(), "not-utf8");
        assert_eq!(diagnostic.line, 6, "[{variant}] the bad byte's line");
        assert_eq!(diagnostic.span, Some(Span::new(at, at + 1)), "[{variant}]");
        let lead = if variant.contains("bom") { 3 } else { 0 };
        assert_eq!(parsed.body, Span::new(lead, bytes.len()), "[{variant}]");
        assert_eq!(parsed.bom, lead == 3);
    }
}
