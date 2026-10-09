//! docs/features/kind-amendment.md, iteration 2: `check::front_matter_failure`
//! (public; the crate's `front_matter_failed` is `front_matter_failure(..)
//! .is_some()`, so the two read the same codes by construction): the
//! parser's first finding that failed to read the front-matter, for
//! exactly the four codes the check treats as "the front-matter does not
//! read" (`not-utf8`, `frontmatter-unclosed`, `frontmatter-yaml`,
//! `frontmatter-not-mapping`), none for the other nine; on real texts its
//! code and line are the parser's. An amendment's refusal quotes it.
//!
//! M: one code dropped from `front_matter_failure`.

mod common;

use specengine_core::check::{front_matter_failure, is_live};
use specengine_model::{Diagnostic, DiagnosticCode, IdScheme, ParsedFile};

use common::{corpus_scheme, fixture};

fn spec_a() -> IdScheme {
    corpus_scheme(&fixture("spec-a"))
}

/// The front-matter failure codes, as the check names them.
const FAILURES: [&str; 4] = [
    "not-utf8",
    "frontmatter-unclosed",
    "frontmatter-yaml",
    "frontmatter-not-mapping",
];

/// A clean parse of a small document with `diagnostics` in place of its
/// own.
fn with_diagnostics(diagnostics: Vec<Diagnostic>) -> ParsedFile {
    let mut parsed = specengine_core::parse(
        "docs/features/x.md",
        b"---\nclass: spec\nstatus: draft\n---\n\n# X\n\nBody.\n",
        &spec_a(),
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.diagnostics = diagnostics;
    parsed
}

fn diagnostic(code: DiagnosticCode, line: usize) -> Diagnostic {
    Diagnostic {
        code,
        severity: code.severity(),
        line,
        span: None,
        fix: None,
        message: format!("a {code} finding"),
    }
}

/// Every code of the model: `front_matter_failure` returns the finding for
/// exactly the four failure codes, `None` for the other nine and for none.
#[test]
fn exactly_the_four_failure_codes_are_a_failure() {
    assert!(front_matter_failure(&with_diagnostics(Vec::new())).is_none());
    let mut failing = Vec::new();
    for code in DiagnosticCode::ALL {
        let parsed = with_diagnostics(vec![diagnostic(code, 7)]);
        match front_matter_failure(&parsed) {
            Some(found) => {
                assert_eq!((found.code, found.line), (code, 7), "{code}");
                assert_eq!(found.message, format!("a {code} finding"));
                failing.push(code.as_str());
            }
            None => assert!(!FAILURES.contains(&code.as_str()), "{code} is a failure"),
        }
    }
    assert_eq!(failing, FAILURES);
}

/// The first failure among several findings: a warning before it is
/// skipped, a second failure after it is not returned.
#[test]
fn the_first_failure_is_returned() {
    let parsed = with_diagnostics(vec![
        diagnostic(DiagnosticCode::UnknownKey, 3),
        diagnostic(DiagnosticCode::FrontmatterType, 4),
        diagnostic(DiagnosticCode::FrontmatterYaml, 6),
        diagnostic(DiagnosticCode::NotUtf8, 1),
    ]);
    let found = front_matter_failure(&parsed).expect("a failure");
    assert_eq!(
        (found.code, found.line),
        (DiagnosticCode::FrontmatterYaml, 6)
    );
}

/// Real texts: each failure the parser reports is the one returned, its
/// code and line the parser's (`frontmatter-yaml` at 6 for "Data"'s text
/// with an unclosed `amends` list); a text that reads (an unknown key, a
/// mistyped value) has none. A failed front-matter counts as live.
#[test]
fn real_texts_return_the_parsers_finding() {
    let scheme = spec_a();
    let cases: Vec<(&str, Vec<u8>, &str, usize)> = vec![
        (
            "not UTF-8",
            b"---\nclass: spec\n---\n\n# X\n\n\xff\xfe\n".to_vec(),
            "not-utf8",
            0,
        ),
        (
            "unclosed",
            b"---\nclass: spec\nstatus: draft\n\n# X\n".to_vec(),
            "frontmatter-unclosed",
            0,
        ),
        (
            "invalid YAML",
            b"---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
              [RULE-STAM-REGEN\n---\n\n# X\n"
                .to_vec(),
            "frontmatter-yaml",
            6,
        ),
        (
            "not a mapping",
            b"---\n- class\n- spec\n---\n\n# X\n".to_vec(),
            "frontmatter-not-mapping",
            0,
        ),
    ];
    for (label, bytes, code, line) in cases {
        let parsed = specengine_core::parse("docs/features/x.md", &bytes, &scheme);
        let reported: Vec<(&str, usize)> = parsed
            .diagnostics
            .iter()
            .filter(|found| FAILURES.contains(&found.code.as_str()))
            .map(|found| (found.code.as_str(), found.line))
            .collect();
        assert!(!reported.is_empty(), "{label}: {:?}", parsed.diagnostics);
        let found = front_matter_failure(&parsed)
            .unwrap_or_else(|| panic!("{label}: no failure in {:?}", parsed.diagnostics));
        assert_eq!((found.code.as_str(), found.line), reported[0], "{label}");
        assert_eq!(found.code.as_str(), code, "{label}");
        if line > 0 {
            assert_eq!(found.line, line, "{label}");
        }
        assert!(is_live(&parsed), "{label}: a failed front-matter is live");
    }
    for (label, text) in [
        (
            "an unknown key",
            "---\nclass: spec\nstatus: draft\nx_weird: 1\n---\n\n# X\n",
        ),
        (
            "a mistyped value",
            "---\nclass: spec\nstatus: [draft]\n---\n\n# X\n",
        ),
    ] {
        let parsed = specengine_core::parse("docs/features/x.md", text.as_bytes(), &scheme);
        assert!(!parsed.diagnostics.is_empty(), "{label}: a finding");
        assert!(
            front_matter_failure(&parsed).is_none(),
            "{label}: {:?}",
            parsed.diagnostics
        );
    }
}
