//! AC-02 to AC-05 and the `Resolver` API of docs/features/spec-check-scopes.md
//! (ADR-0026): feature scopes. A feature document is a walked `.md` file
//! directly under `[paths] features` whose stem is a slug; a feature-scoped
//! ID (`[ids] scope = "feature"`) is defined only as a `{#ID}` section of one,
//! cited `<slug>/ID` from other files and bare in its own; `slug/` resolves
//! only in `<features>/<slug>.md`, for any prefix; `project:` stays skipped. A
//! misplaced definition is the error `id-scope`, one per definition, with no
//! `id-taken` or `file-name` beside it.
//!
//! Everything runs in memory through the real TOML readers and parser.

mod common;

use common::check::{Config, show, with_code};
use specengine_core::check::{Report, Resolution, Resolver, Verdict};
use specengine_model::{LinkTarget, Reference, Severity};

/// `AC` is feature-scoped, `R` project-scoped, `MEC` a project-scoped name
/// shape, `UC` a feature-scoped name shape; `[paths]` all default.
const TOML: &str = "\
[ids]
R   = { kind = \"requirement\", width = 2 }
AC  = { kind = \"criterion\",   width = 2, scope = \"feature\" }
MEC = { kind = \"mechanic\",    shape = \"name\" }
UC  = { kind = \"use-case\",    shape = \"name\", scope = \"feature\" }
";

fn config() -> Config {
    Config::from_toml(TOML)
}

/// The same scheme with `[paths] features = "specs/feat"` written with
/// `trailing` appended (`""` or `"/"`).
fn custom(trailing: &str) -> Config {
    Config::from_toml(&format!(
        "[paths]\nfeatures = \"specs/feat{trailing}\"\n\n{TOML}"
    ))
}

/// A live canon document: front-matter on lines 1-5, a blank line 6, `body`
/// from line 7.
fn canon(body: &str) -> String {
    format!("---\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n{body}")
}

/// A live canon document citing `refs` in its front-matter (line 5), a
/// blank line 7, `body` from line 8.
fn citing(refs: &str, body: &str) -> String {
    format!("---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nrefs: [{refs}]\n---\n\n{body}")
}

type Quad = (String, String, usize, String);

/// `(code, path, line, subject)` of every finding, in report order.
fn quads(report: &Report) -> Vec<Quad> {
    report
        .findings
        .iter()
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

fn quad(code: &str, path: &str, line: usize, subject: &str) -> Quad {
    (code.to_owned(), path.to_owned(), line, subject.to_owned())
}

/// The message of the one finding with `code` and `subject`.
fn message(report: &Report, code: &str, subject: &str) -> String {
    let found: Vec<_> = with_code(report, code)
        .into_iter()
        .filter(|f| f.subject == subject)
        .collect();
    assert_eq!(found.len(), 1, "one {code} on {subject}:\n{}", show(report));
    found[0].message.clone()
}

fn run_files(config: &Config, files: &[(&str, String)]) -> Report {
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    config.check(&borrowed)
}

/// The first ID reference of `text`, parsed as a lone inline mention.
fn reference(config: &Config, text: &str) -> Reference {
    let probe = config.input(&[("docs/probe.md", text)]);
    probe.files[0]
        .parsed
        .as_ref()
        .expect("parsed")
        .links
        .iter()
        .find_map(|link| match &link.dst {
            LinkTarget::Reference(reference) => Some(reference.clone()),
            LinkTarget::Path(_) => None,
        })
        .unwrap_or_else(|| panic!("{text:?} is a reference"))
}

// ------------------------------------------------------------------ AC-02

/// One feature document under `specs/feat`, and look-alikes that are not:
/// nested, a stem that is no slug (twice), another directory, a sibling
/// directory sharing the name's start.
fn ac02_files() -> Vec<(&'static str, String)> {
    vec![
        ("specs/feat/a.md", canon("# A\n\n## One {#AC-01}\n")),
        ("specs/feat/sub/b.md", canon("# B\n\n## Two {#AC-02}\n")),
        ("specs/feat/Upper.md", canon("# U\n\n## Three {#AC-03}\n")),
        ("docs/features/c.md", canon("# C\n\n## Four {#AC-04}\n")),
        ("specs/feat/README.md", canon("# R\n\n## Five {#AC-05}\n")),
        ("specs/feats/d.md", canon("# D\n\n## Six {#AC-06}\n")),
        ("docs/s.md", canon("Cites a/AC-01 and c/AC-04.\n")),
    ]
}

fn ac02_want() -> Vec<Quad> {
    vec![
        quad("id-scope", "docs/features/c.md", 9, "AC-04"),
        quad("mention-dangling", "docs/s.md", 7, "c/AC-04"),
        quad("id-scope", "specs/feat/README.md", 9, "AC-05"),
        quad("id-scope", "specs/feat/Upper.md", 9, "AC-03"),
        quad("id-scope", "specs/feat/sub/b.md", 9, "AC-02"),
        quad("id-scope", "specs/feats/d.md", 9, "AC-06"),
    ]
}

#[test]
fn only_direct_slug_children_of_the_configured_directory_are_feature_documents() {
    let config = custom("");
    let report = run_files(&config, &ac02_files());
    assert_eq!(quads(&report), ac02_want(), "{}", show(&report));
    // The reasons name the configured directory, not the default one.
    assert_eq!(
        message(&report, "mention-dangling", "c/AC-04"),
        "`mentions`: `c/AC-04` resolves to no feature document `specs/feat/c.md`"
    );
    assert_eq!(
        message(&report, "id-scope", "AC-04"),
        "`AC-04` is feature-scoped: define it as a `{#AC-04}` section of a document \
         directly under `specs/feat`"
    );
    // A trailing `/` in the configuration changes nothing.
    let slashed = run_files(&custom("/"), &ac02_files());
    assert_eq!(slashed.findings, report.findings, "{}", show(&slashed));
}

#[test]
fn the_default_features_directory_is_only_a_default() {
    // Under the default `docs/features`, `docs/features/c.md` is the feature
    // document and every `specs/feat/*` definition is misplaced.
    let report = run_files(&config(), &ac02_files());
    assert_eq!(
        quads(&report),
        [
            quad("mention-dangling", "docs/s.md", 7, "a/AC-01"),
            quad("id-scope", "specs/feat/README.md", 9, "AC-05"),
            quad("id-scope", "specs/feat/Upper.md", 9, "AC-03"),
            quad("id-scope", "specs/feat/a.md", 9, "AC-01"),
            quad("id-scope", "specs/feat/sub/b.md", 9, "AC-02"),
            quad("id-scope", "specs/feats/d.md", 9, "AC-06"),
        ],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report, "mention-dangling", "a/AC-01"),
        "`mentions`: `a/AC-01` resolves to no feature document `docs/features/a.md`"
    );
}

#[test]
fn feature_slug_is_the_stem_of_a_walked_direct_child_only() {
    let config = custom("");
    let files = ac02_files();
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let input = config.input(&borrowed);
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    assert_eq!(resolver.feature_slug("specs/feat/a.md"), Some("a"));
    for not_one in [
        "specs/feat/sub/b.md",
        "specs/feat/Upper.md",
        "specs/feat/README.md",
        "specs/feats/d.md",
        "docs/features/c.md",
        "docs/s.md",
        // Unwalked, or no file at all.
        "specs/feat/z.md",
        "",
    ] {
        assert_eq!(resolver.feature_slug(not_one), None, "{not_one:?}");
    }
}

#[test]
fn a_feature_document_with_a_failed_front_matter_still_defines_its_ids() {
    // S1: class, tier, status and a failed front-matter do not matter.
    let report = config().check(&[
        (
            "docs/features/feat.md",
            "---\nclass: [unclosed\n---\n# Feat\n\n## Crit {#AC-01}\n\nOwn: AC-01.\n",
        ),
        ("docs/s.md", &canon("Cites feat/AC-01.\n")),
    ]);
    assert!(
        with_code(&report, "id-scope").is_empty(),
        "{}",
        show(&report)
    );
    assert!(
        with_code(&report, "mention-dangling").is_empty(),
        "{}",
        show(&report)
    );
}

#[test]
fn tier3_documents_still_define_and_still_misplace() {
    // S1: a Tier 3 feature document still defines its IDs; S6: a Tier 3
    // misplaced definition is still `id-scope`.
    let rejected = |body: &str| {
        format!("---\nclass: decision\ntitle: T\nstatus: rejected\nscope: [x]\n---\n\n{body}")
    };
    let report = config().check(&[
        (
            "docs/features/old.md",
            &rejected("# Old\n\n## One {#AC-01}\n"),
        ),
        ("docs/decisions/x.md", &rejected("# X\n\n## Two {#AC-02}\n")),
        ("docs/s.md", &canon("Cites old/AC-01.\n")),
    ]);
    // (A decision without `id:` is also `key-missing`: not this rule's.)
    let scoped: Vec<Quad> = quads(&report)
        .into_iter()
        .filter(|(code, ..)| code != "key-missing")
        .collect();
    assert_eq!(
        scoped,
        [quad("id-scope", "docs/decisions/x.md", 10, "AC-02")],
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------------ AC-03

#[test]
fn a_slug_reference_resolves_in_its_feature_document() {
    let feature = canon("# Feat\n\n## One {#AC-01}\n\n## Two {#AC-02}\n\n## Mec {#MEC-X}\n");
    let source = citing(
        "feat/AC-01, feat/AC-01#AC-02",
        "Inline feat/AC-01, feat/AC-01#AC-02 and a name fallback feat/MEC-X-extra.\n",
    );
    let report = config().check(&[
        ("docs/features/feat.md", &feature),
        ("docs/spec/x.md", &source),
    ]);
    assert!(report.findings.is_empty(), "{}", show(&report));
    assert_eq!(report.verdict, Verdict::Clean);
}

#[test]
fn a_slug_reference_without_its_feature_document_dangles_naming_the_file() {
    let source = citing("feat/AC-01", "Inline feat/AC-01.\n");
    let report = config().check(&[
        // AC-01 is defined, but in another feature: never borrowed.
        ("docs/features/other.md", &canon("# O\n\n## One {#AC-01}\n")),
        ("docs/spec/x.md", &source),
    ]);
    assert_eq!(
        quads(&report),
        [
            quad("ref-dangling", "docs/spec/x.md", 5, "feat/AC-01"),
            quad("mention-dangling", "docs/spec/x.md", 8, "feat/AC-01"),
        ],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report, "ref-dangling", "feat/AC-01"),
        "`refs`: `feat/AC-01` resolves to no feature document `docs/features/feat.md`"
    );
    assert_eq!(
        message(&report, "mention-dangling", "feat/AC-01"),
        "`mentions`: `feat/AC-01` resolves to no feature document `docs/features/feat.md`"
    );
    let severities: Vec<Severity> = report.findings.iter().map(|f| f.severity).collect();
    assert_eq!(severities, [Severity::Error, Severity::Warning]);
    assert_eq!(report.verdict, Verdict::Blocked);
}

#[test]
fn a_slug_reference_to_an_id_its_feature_lacks_dangles_naming_the_file() {
    let source = citing("feat/AC-01", "Inline feat/AC-01.\n");
    let report = config().check(&[
        (
            "docs/features/feat.md",
            &canon("# Feat\n\n## Two {#AC-02}\n"),
        ),
        // Another file's AC-01 is never the one of `feat`.
        ("docs/features/other.md", &canon("# O\n\n## One {#AC-01}\n")),
        ("docs/spec/x.md", &source),
    ]);
    assert_eq!(
        quads(&report),
        [
            quad("ref-dangling", "docs/spec/x.md", 5, "feat/AC-01"),
            quad("mention-dangling", "docs/spec/x.md", 8, "feat/AC-01"),
        ],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report, "ref-dangling", "feat/AC-01"),
        "`refs`: `feat/AC-01` is not defined in `docs/features/feat.md`"
    );
    assert_eq!(
        message(&report, "mention-dangling", "feat/AC-01"),
        "`mentions`: `feat/AC-01` is not defined in `docs/features/feat.md`"
    );
}

#[test]
fn a_slug_section_must_be_a_section_of_the_feature_document() {
    let source = citing("feat/AC-01#AC-02", "Inline feat/AC-01#AC-02.\n");
    let report = config().check(&[
        (
            "docs/features/feat.md",
            &canon("# Feat\n\n## One {#AC-01}\n"),
        ),
        // `AC-02` exists, in another feature: not a section of `feat.md`.
        ("docs/features/other.md", &canon("# O\n\n## Two {#AC-02}\n")),
        ("docs/spec/x.md", &source),
    ]);
    assert_eq!(
        quads(&report),
        [
            quad("ref-dangling", "docs/spec/x.md", 5, "feat/AC-01#AC-02"),
            quad("mention-dangling", "docs/spec/x.md", 8, "feat/AC-01#AC-02"),
        ],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report, "mention-dangling", "feat/AC-01#AC-02"),
        "`mentions`: `feat/AC-01#AC-02` has no section `#AC-02` in `docs/features/feat.md`"
    );
}

#[test]
fn a_slug_reference_of_a_project_prefix_resolves_only_in_its_feature() {
    // `feat/R-97`: any prefix under `slug/` resolves in the feature only.
    let source = citing("feat/R-97", "Inline feat/R-97 and feat/MEC-Y-extra.\n");
    let files = [
        (
            "docs/features/feat.md",
            canon("# Feat\n\n## One {#AC-01}\n"),
        ),
        ("docs/r/R-97.md", canon("# R\n\n## Ninety-seven {#R-97}\n")),
        ("docs/m/y.md", canon("# M\n\n## Mec {#MEC-Y}\n")),
        ("docs/spec/x.md", source.clone()),
    ];
    let report = run_files(&config(), &files);
    assert_eq!(
        quads(&report),
        [
            quad("ref-dangling", "docs/spec/x.md", 5, "feat/R-97"),
            quad("mention-dangling", "docs/spec/x.md", 8, "feat/MEC-Y-extra"),
            quad("mention-dangling", "docs/spec/x.md", 8, "feat/R-97"),
        ],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report, "mention-dangling", "feat/R-97"),
        "`mentions`: `feat/R-97` is not defined in `docs/features/feat.md`"
    );
    // Defined in the feature, it resolves, whatever else defines it.
    let files = [
        (
            "docs/features/feat.md",
            canon("# Feat\n\n## R {#R-97}\n\n## Mec {#MEC-Y}\n"),
        ),
        ("docs/spec/x.md", source),
    ];
    let report = run_files(&config(), &files);
    assert!(report.findings.is_empty(), "{}", show(&report));
}

#[test]
fn project_qualified_references_are_skipped_with_or_without_a_slug() {
    let source = citing(
        "other:feat/AC-98, other:R-99",
        "Inline other:feat/AC-98 and other:R-99.\n",
    );
    let report = config().check(&[("docs/spec/x.md", &source)]);
    assert!(report.findings.is_empty(), "{}", show(&report));
}

// ------------------------------------------------------------------ AC-04

/// `feat.md` and `other.md` both define `AC-01`; `other` is given first so
/// that path order, not input order, decides the reason.
fn two_features() -> Vec<(&'static str, String)> {
    vec![
        (
            "docs/features/other.md",
            canon("# Other\n\n## One {#AC-01}\n\nOwn: AC-01.\n"),
        ),
        (
            "docs/features/feat.md",
            canon(
                "# Feat\n\nOwn: AC-01 and UC-LOGIN-extra.\n\n## One {#AC-01}\n\n## Login {#UC-LOGIN}\n",
            ),
        ),
    ]
}

#[test]
fn a_bare_feature_scoped_id_resolves_in_its_own_feature_document() {
    let report = run_files(&config(), &two_features());
    assert!(report.findings.is_empty(), "{}", show(&report));
}

#[test]
fn a_bare_feature_scoped_id_elsewhere_dangles_naming_each_feature_in_path_order() {
    let mut files = two_features();
    files.push((
        "docs/spec/x.md",
        citing("AC-01", "Inline AC-01 and UC-LOGIN.\n"),
    ));
    // A feature document lacking the ID is "elsewhere" too.
    files.push(("docs/features/third.md", canon("# Third\n\nBare AC-01.\n")));
    let report = run_files(&config(), &files);
    assert_eq!(
        quads(&report),
        [
            quad("mention-dangling", "docs/features/third.md", 9, "AC-01"),
            quad("ref-dangling", "docs/spec/x.md", 5, "AC-01"),
            quad("mention-dangling", "docs/spec/x.md", 8, "AC-01"),
            quad("mention-dangling", "docs/spec/x.md", 8, "UC-LOGIN"),
        ],
        "{}",
        show(&report)
    );
    let both = "is feature-scoped: cite it as `feat/AC-01` or `other/AC-01`";
    for finding in with_code(&report, "mention-dangling")
        .into_iter()
        .filter(|f| f.subject == "AC-01")
    {
        assert_eq!(finding.message, format!("`mentions`: `AC-01` {both}"));
    }
    assert_eq!(
        message(&report, "ref-dangling", "AC-01"),
        format!("`refs`: `AC-01` {both}")
    );
    assert_eq!(
        message(&report, "mention-dangling", "UC-LOGIN"),
        "`mentions`: `UC-LOGIN` is feature-scoped: cite it as `feat/UC-LOGIN`"
    );
}

#[test]
fn a_bare_feature_scoped_id_no_feature_defines_says_so() {
    let report = config().check(&[
        // AC-06 is defined only outside a feature document (`id-scope`).
        ("docs/spec/y.md", &canon("# Y\n\n## Six {#AC-06}\n")),
        (
            "docs/spec/x.md",
            &citing("AC-05", "Inline AC-05 and AC-06, and UC-LOGIN-extra.\n"),
        ),
        (
            "docs/features/feat.md",
            &canon("# Feat\n\n## Login {#UC-LOGIN}\n"),
        ),
    ]);
    assert_eq!(
        quads(&report),
        [
            quad("ref-dangling", "docs/spec/x.md", 5, "AC-05"),
            quad("mention-dangling", "docs/spec/x.md", 8, "AC-05"),
            quad("mention-dangling", "docs/spec/x.md", 8, "AC-06"),
            quad("mention-dangling", "docs/spec/x.md", 8, "UC-LOGIN-extra"),
            quad("id-scope", "docs/spec/y.md", 9, "AC-06"),
        ],
        "{}",
        show(&report)
    );
    let none = "is feature-scoped and no feature document defines it";
    assert_eq!(
        message(&report, "ref-dangling", "AC-05"),
        format!("`refs`: `AC-05` {none}")
    );
    assert_eq!(
        message(&report, "mention-dangling", "AC-06"),
        format!("`mentions`: `AC-06` {none}")
    );
    // S4: the reason is for the ID as written, not for the fallback
    // candidate `UC-LOGIN` that `feat.md` defines.
    assert_eq!(
        message(&report, "mention-dangling", "UC-LOGIN-extra"),
        format!("`mentions`: `UC-LOGIN-extra` {none}")
    );
}

#[test]
fn a_bare_project_scoped_id_resolves_anywhere() {
    let report = config().check(&[
        ("docs/features/feat.md", &canon("# Feat\n\n## R {#R-12}\n")),
        ("docs/spec/x.md", &citing("R-12", "Inline R-12.\n")),
    ]);
    assert!(report.findings.is_empty(), "{}", show(&report));
}

// ------------------------------------------------------------------ AC-05

#[test]
fn a_misplaced_feature_scoped_definition_is_one_id_scope_error_each() {
    let config = config();
    let report = config.check(&[
        (
            "docs/records/AC/AC-07.md",
            "---\nid: AC-07\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Seven\n",
        ),
        // Named after another ID: still no `file-name` beside `id-scope`.
        (
            "docs/records/AC/AC-08.md",
            "---\nid: AC-09\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Nine\n",
        ),
        ("docs/spec/x.md", &canon("# X\n\n## One {#AC-01}\n")),
        (
            "docs/features/sub/y.md",
            &canon("# Y\n\n## One again {#AC-01}\n"),
        ),
        (
            "docs/features/feat.md",
            "---\nid: AC-02\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Feat\n\n## Own {#AC-03}\n",
        ),
        // Any class: a generated file too.
        (
            "docs/generated/g.md",
            "---\nclass: generated\ngenerator: gen\nsource: s\n---\n# G\n\n## Crit {#AC-04}\n",
        ),
    ]);
    assert_eq!(
        quads(&report),
        [
            quad("id-scope", "docs/features/feat.md", 2, "AC-02"),
            quad("id-scope", "docs/features/sub/y.md", 9, "AC-01"),
            quad("id-scope", "docs/generated/g.md", 8, "AC-04"),
            quad("id-scope", "docs/records/AC/AC-07.md", 2, "AC-07"),
            quad("id-scope", "docs/records/AC/AC-08.md", 2, "AC-09"),
            quad("id-scope", "docs/spec/x.md", 9, "AC-01"),
        ],
        "{}",
        show(&report)
    );
    for finding in &report.findings {
        assert_eq!(finding.severity, Severity::Error, "{}", finding.subject);
        assert!(finding.blocks_when_enforced());
    }
    assert_eq!(
        message(&report, "id-scope", "AC-07"),
        "`AC-07` is feature-scoped: define it as a `{#AC-07}` section of a document \
         directly under `docs/features`"
    );
    assert_eq!(report.verdict, Verdict::Blocked);
    let lines = report.lines(false).join("\n");
    assert!(
        lines.contains("docs/records/AC/AC-07.md:2: id-scope: `AC-07` is feature-scoped"),
        "{lines}"
    );
}

#[test]
fn a_misplaced_definition_keeps_its_id_width() {
    let report = config().check(&[(
        "docs/records/AC/AC-7.md",
        "---\nid: AC-7\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Seven\n",
    )]);
    assert_eq!(
        quads(&report),
        [
            quad("id-scope", "docs/records/AC/AC-7.md", 2, "AC-7"),
            quad("id-width", "docs/records/AC/AC-7.md", 2, "AC-7"),
        ],
        "{}",
        show(&report)
    );
}

#[test]
fn a_legacy_prefix_of_a_feature_scoped_prefix_is_scoped_too() {
    // S2: `CR` is an `aliases_from` entry of the feature-scoped `AC`, so a
    // bare `CR-01` resolves only in its own feature document.
    let config = Config::from_toml(
        "[ids]\nAC = { kind = \"criterion\", width = 2, scope = \"feature\", aliases_from = [\"CR\"] }\n",
    );
    let report = config.check(&[
        (
            "docs/features/feat.md",
            &canon("# Feat\n\nLegacy CR-01.\n\n## One {#AC-01}\n"),
        ),
        (
            "docs/spec/x.md",
            &canon("Legacy CR-01, scoped feat/CR-01.\n"),
        ),
    ]);
    assert_eq!(
        quads(&report),
        [quad("mention-dangling", "docs/spec/x.md", 7, "CR-01")],
        "{}",
        show(&report)
    );
    assert_eq!(
        message(&report, "mention-dangling", "CR-01"),
        "`mentions`: `CR-01` is feature-scoped: cite it as `feat/CR-01`"
    );
}

// ------------------------------------------------------------ parent:
// Iteration 2 of docs/features/spec-check-scopes.md: `parent:` keeps its
// `project:` and `slug/` qualifiers and its `#section` (the verbatim bytes
// under `ParentRef.span` re-read, used only when they give back the same
// ID), so it resolves by scope and its section is checked. A value not
// written verbatim (an escape) has no span: it falls back to the bare ID.

/// The scheme of this file plus `Q` with its legacy prefix `QST`.
fn parent_config() -> Config {
    Config::from_toml(&format!(
        "{TOML}Q   = {{ kind = \"question\", width = 3, aliases_from = [\"QST\"] }}\n"
    ))
}

/// The corpus every `parent:` case runs against: `feat.md` defines
/// `AC-01` and `AC-02`; `R-97`, `R-01` (with the section `R-05`) and
/// `Q-031` are defined elsewhere.
fn parent_corpus() -> Vec<(&'static str, String)> {
    vec![
        (
            "docs/features/feat.md",
            canon("# Feat\n\n## One {#AC-01}\n\n## Two {#AC-02}\n"),
        ),
        ("docs/r/R-97.md", canon("# R\n\n## Ninety-seven {#R-97}\n")),
        (
            "docs/r/R-01.md",
            "---\nid: R-01\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# One\n\n## Five {#R-05}\n".to_owned(),
        ),
        (
            "docs/q/Q-031.md",
            "---\nid: Q-031\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# Q\n".to_owned(),
        ),
    ]
}

/// A live canon document whose `parent:` (line 5) is `value` as written.
fn with_parent(value: &str) -> String {
    format!("---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nparent: {value}\n---\n\n# X\n")
}

/// `(line, subject, message)` of every finding of `docs/spec/x.md`, the
/// document citing `parent: value`.
fn parent_findings(value: &str) -> Vec<(usize, String, String)> {
    let mut files = parent_corpus();
    files.push(("docs/spec/x.md", with_parent(value)));
    let report = run_files(&parent_config(), &files);
    assert!(
        report.findings.iter().all(|f| f.path == "docs/spec/x.md"),
        "{value}: the corpus is quiet:\n{}",
        show(&report)
    );
    report
        .findings
        .iter()
        .map(|f| {
            assert_eq!(f.code, "ref-dangling", "{value}: {}", show(&report));
            (f.line, f.subject.clone(), f.message.clone())
        })
        .collect()
}

fn dangles(subject: &str, reason: &str) -> Vec<(usize, String, String)> {
    vec![(
        5,
        subject.to_owned(),
        format!("`parent`: `{subject}` {reason}"),
    )]
}

#[test]
fn a_parent_resolves_with_its_qualifiers_and_section() {
    let none: Vec<(usize, String, String)> = Vec::new();
    for (value, want) in [
        ("feat/AC-01", none.clone()),
        ("\"feat/AC-01\"", none.clone()),
        ("'feat/AC-01'", none.clone()),
        ("feat/AC-01#AC-02", none.clone()),
        ("other:R-99", none.clone()),
        ("other:feat/AC-98", none.clone()),
        ("QST-031", none.clone()),
        ("R-01", none.clone()),
        ("R-01#R-05", none.clone()),
        (
            "feat/R-97",
            dangles("feat/R-97", "is not defined in `docs/features/feat.md`"),
        ),
        (
            "feat/AC-01#AC-09",
            dangles(
                "feat/AC-01#AC-09",
                "has no section `#AC-09` in `docs/features/feat.md`",
            ),
        ),
        (
            "nope/AC-01",
            dangles(
                "nope/AC-01",
                "resolves to no feature document `docs/features/nope.md`",
            ),
        ),
        (
            "AC-01",
            dangles("AC-01", "is feature-scoped: cite it as `feat/AC-01`"),
        ),
        (
            "R-01#R-09",
            dangles("R-01#R-09", "has no section `#R-09` in its file"),
        ),
        (
            "QST-099",
            dangles("QST-099", "resolves to no ID and no alias"),
        ),
    ] {
        assert_eq!(parent_findings(value), want, "parent: {value}");
    }
}

/// Known limit: a value written with an escape is not verbatim, keeps no
/// span, and is judged as its bare ID (`slug/` and `#Y` lost).
#[test]
fn an_escaped_parent_falls_back_to_its_bare_id() {
    assert_eq!(
        parent_findings(r#""feat\/AC-01""#),
        dangles("AC-01", "is feature-scoped: cite it as `feat/AC-01`")
    );
    // `\u0023` is `#`: the section is lost with the span, nothing to check.
    assert_eq!(parent_findings(r#""R-01\u0023R-09""#), []);
}

/// The verbatim bytes are used only when they give back the parser's ID:
/// a `ParentRef` whose ID differs from its span's text (crafted: the
/// parser never produces one) is judged by its ID.
#[test]
fn a_parent_whose_span_names_another_id_is_judged_by_its_id() {
    let config = parent_config();
    let mut files = parent_corpus();
    files.push(("docs/spec/x.md", with_parent("feat/AC-01")));
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let mut input = config.input(&borrowed);
    let x = input
        .files
        .iter_mut()
        .find(|f| f.path == "docs/spec/x.md")
        .unwrap();
    let parent = x.parsed.as_mut().unwrap().nodes[0]
        .parent
        .as_mut()
        .expect("a parent");
    assert_eq!(parent.id, "AC-01");
    assert!(parent.span.is_some(), "a verbatim value keeps its span");
    parent.id = "AC-02".to_owned();
    let report = config.run(&input);
    let found: Vec<(&str, &str)> = with_code(&report, "ref-dangling")
        .into_iter()
        .map(|f| (f.path.as_str(), f.message.as_str()))
        .collect();
    assert_eq!(
        found,
        [(
            "docs/spec/x.md",
            "`parent`: `feat/AC-01` is feature-scoped: cite it as `feat/AC-02`"
        )],
        "{}",
        show(&report)
    );
}

#[test]
fn a_superseded_feature_as_a_scoped_parent_warns() {
    let report = config().check(&[
        (
            "docs/features/old.md",
            "---\nclass: canon\nowner: o\nreviewed: 2026-09-01\nstatus: superseded-by R-02\n---\n\n# Old\n\n## One {#AC-01}\n",
        ),
        (
            "docs/r/R-02.md",
            "---\nid: R-02\nclass: canon\nowner: o\nreviewed: 2026-09-01\n---\n\n# New\n",
        ),
        ("docs/spec/x.md", &with_parent("old/AC-01")),
    ]);
    assert_eq!(
        quads(&report),
        [quad("ref-superseded", "docs/spec/x.md", 5, "old/AC-01")],
        "{}",
        show(&report)
    );
}

// ------------------------------------------------------------ the Resolver API

#[test]
fn the_resolver_takes_the_citing_file_first() {
    let config = config();
    let files = two_features();
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
    let input = config.input(&borrowed);
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    // Path order, whatever the input order.
    assert_eq!(
        resolver.paths(),
        ["docs/features/feat.md", "docs/features/other.md"]
    );
    let bare = reference(&config, "AC-01\n");
    let scoped = reference(&config, "other/AC-01\n");
    let project = reference(&config, "p:feat/AC-01\n");
    // Bare: its own file only; elsewhere ("" or unwalked) it dangles.
    assert_eq!(
        resolver.resolve("docs/features/feat.md", &bare, "AC-01"),
        Resolution::Resolved(vec![0])
    );
    assert_eq!(
        resolver.resolve_mention("docs/features/other.md", &bare, "AC-01"),
        Resolution::Resolved(vec![1])
    );
    for elsewhere in ["", "docs/spec/unwalked.md"] {
        assert!(
            matches!(
                resolver.resolve(elsewhere, &bare, "AC-01"),
                Resolution::Dangling(_)
            ),
            "{elsewhere:?}"
        );
        assert_eq!(resolver.holders_of(elsewhere, &bare, "AC-01"), None);
    }
    assert_eq!(
        resolver.holders_of("docs/features/other.md", &bare, "AC-01"),
        Some(vec![1])
    );
    // `slug/`: the feature document, from anywhere.
    for from in ["", "docs/features/feat.md", "docs/spec/x.md"] {
        assert_eq!(
            resolver.resolve(from, &scoped, "other/AC-01"),
            Resolution::Resolved(vec![1]),
            "{from:?}"
        );
        assert_eq!(
            resolver.holders_of(from, &scoped, "other/AC-01"),
            Some(vec![1])
        );
    }
    // `project:` alone is skipped.
    assert_eq!(
        resolver.resolve("", &project, "p:feat/AC-01"),
        Resolution::Skipped
    );
    assert_eq!(resolver.holders_of("", &project, "p:feat/AC-01"), None);
}
