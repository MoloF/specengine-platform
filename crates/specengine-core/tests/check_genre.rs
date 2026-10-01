//! AC-18 of docs/features/spec-check.md: genre independence (ADR-0008).
//! The same check, each fixture read through its own `specengine.toml`
//! alone, blocks on exactly the pinned findings: spec-a (a game) on the
//! `frontmatter-type` of `docs/features/stamina-tuning.md`; spec-b (a
//! command-line tool, Russian prose, Cyrillic legacy prefixes) on the
//! homoglyphs of `docs/spec/cli.md` and `docs/records/QN/QN-08.md`, the
//! latter's `frontmatter-yaml`, and the two aliases without a target (AC-10);
//! so no `class-missing` (every fixture document declares its class per the
//! Data rule "Class per record kind").
//! The check source names no prefix, path or file of this repository or a
//! fixture.
//!
//! AC-13 of docs/features/spec-check-graph.md: the blocking sets above are
//! unchanged (no registry in either fixture: §11.5–6 off); the new warnings
//! are exactly spec-a's `depends-cycle` and spec-b's `mention-dangling`; the
//! new sources (renderer, registry, resolution, graph rules) are scanned
//! with the old, and name no generator command of this repository either.
//!
//! AC-09 and AC-10 of docs/features/spec-check-scopes.md: with the feature
//! criteria moved into `{#ID}` sections, the pins above hold, no `id-scope`
//! fires and nothing new dangles; the moved citations really resolve in
//! their feature documents. The scope pass's sources name no prefix, slug,
//! `features/`, `records/` or `docs/`; their one path-shaped literal is the
//! document extension `".md"`.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::check::{blocking, fixture_input, show, with_code};
use common::{corpus_scheme, fixture, repository_root};
use specengine_core::check::{Resolution, Resolver, Verdict};
use specengine_model::LinkTarget;

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(p, c)| ((*p).to_owned(), (*c).to_owned()))
        .collect()
}

/// One function for both corpora.
fn blocking_of(corpus: &str) -> (Vec<(String, String)>, Verdict) {
    let (config, input) = fixture_input(&fixture(corpus));
    let report = config.run(&input);
    eprintln!("{corpus}:\n{}", show(&report));
    assert!(
        report.findings.iter().all(|f| f.code != "class-missing"),
        "{corpus}: every document declares its class:\n{}",
        show(&report)
    );
    (blocking(&report), report.verdict)
}

#[test]
fn spec_a_blocks_exactly_on_its_type_error() {
    let (found, verdict) = blocking_of("spec-a");
    assert_eq!(
        found,
        pairs(&[("docs/features/stamina-tuning.md", "frontmatter-type")])
    );
    assert_eq!(verdict, Verdict::Blocked);
}

#[test]
fn spec_b_blocks_exactly_on_its_homoglyphs_yaml_and_dangling_aliases() {
    let (found, verdict) = blocking_of("spec-b");
    assert_eq!(
        found,
        pairs(&[
            ("docs/features/dry-run.md", "ref-dangling"),
            ("docs/records/QN/QN-07.md", "ref-dangling"),
            ("docs/records/QN/QN-08.md", "frontmatter-yaml"),
            ("docs/records/QN/QN-08.md", "homoglyph"),
            ("docs/spec/cli.md", "homoglyph"),
        ])
    );
    assert_eq!(verdict, Verdict::Blocked);
}

/// `(code, path, line, subject)` of the findings of increment 2's codes.
fn increment_2(corpus: &str) -> Vec<(String, String, usize, String)> {
    let (config, input) = fixture_input(&fixture(corpus));
    let report = config.run(&input);
    report
        .findings
        .iter()
        .filter(|f| {
            [
                "index-missing",
                "index-drift",
                "generator-unknown",
                "generator-path",
                "mention-dangling",
                "depends-cycle",
                "ref-superseded",
            ]
            .contains(&f.code.as_str())
        })
        .inspect(|f| assert_eq!(f.severity, specengine_model::Severity::Warning))
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

#[test]
fn the_new_warnings_are_one_per_fixture() {
    assert_eq!(
        increment_2("spec-a"),
        [(
            "depends-cycle".to_owned(),
            "docs/spec/movement/sprint.md".to_owned(),
            9,
            "MEC-SPRINT, MEC-STAMINA".to_owned()
        )]
    );
    assert_eq!(
        increment_2("spec-b"),
        [(
            "mention-dangling".to_owned(),
            "docs/spec/cli.md".to_owned(),
            25,
            "R\u{0415}Q-003".to_owned()
        )]
    );
}

/// `(code, path, line, subject)` of every `id-scope`, `ref-dangling` and
/// `mention-dangling`.
fn scope_findings(corpus: &str) -> Vec<(String, String, usize, String)> {
    let (config, input) = fixture_input(&fixture(corpus));
    let report = config.run(&input);
    report
        .findings
        .iter()
        .filter(|f| ["id-scope", "ref-dangling", "mention-dangling"].contains(&f.code.as_str()))
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

fn four(code: &str, path: &str, line: usize, subject: &str) -> (String, String, usize, String) {
    (code.to_owned(), path.to_owned(), line, subject.to_owned())
}

#[test]
fn the_scope_pass_adds_no_finding_to_the_fixtures() {
    assert_eq!(scope_findings("spec-a"), [], "spec-a");
    // spec-b: the two aliases without a target (AC-10 of spec-check.md) and
    // the mixed-script mention; `dry-run/CRIT-01` of `docs/spec/cli.md:25`
    // resolves.
    let (config, input) = fixture_input(&fixture("spec-b"));
    let report = config.run(&input);
    assert!(
        with_code(&report, "id-scope").is_empty(),
        "{}",
        show(&report)
    );
    assert_eq!(
        scope_findings("spec-b"),
        [
            four(
                "ref-dangling",
                "docs/features/dry-run.md",
                6,
                "\u{0422}\u{0420}\u{0411}-003"
            ),
            four(
                "ref-dangling",
                "docs/records/QN/QN-07.md",
                9,
                "\u{0412}\u{041e}\u{041f}-6"
            ),
            four("mention-dangling", "docs/spec/cli.md", 25, "R\u{0415}Q-003"),
        ],
        "{}",
        show(&report)
    );
}

/// Every inline reference of a feature-scoped prefix in the fixture, as
/// `(citing path, written, resolved paths)`.
fn scoped_citations(corpus: &str) -> Vec<(String, String, Vec<String>)> {
    let (config, input) = fixture_input(&fixture(corpus));
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    let mut out = Vec::new();
    for file in &input.files {
        let Some(parsed) = &file.parsed else { continue };
        for link in &parsed.links {
            let LinkTarget::Reference(reference) = &link.dst else {
                continue;
            };
            let prefix = reference.id.split('-').next().unwrap_or_default();
            let scoped = config
                .scheme
                .prefix(prefix)
                .is_some_and(|spec| spec.scope == specengine_model::IdScope::Feature);
            let Some(span) = reference.span.filter(|_| scoped) else {
                continue;
            };
            let written = String::from_utf8(file.bytes[span.range()].to_vec()).unwrap();
            let resolved = match resolver.resolve_mention(&file.path, reference, &written) {
                Resolution::Resolved(holders) => holders
                    .iter()
                    .map(|&index| resolver.paths()[index].to_owned())
                    .collect(),
                other => panic!("{corpus}/{}: {written}: {other:?}", file.path),
            };
            out.push((file.path.clone(), written, resolved));
        }
    }
    out.sort();
    out
}

#[test]
fn the_moved_criteria_are_cited_into_their_feature_documents() {
    let feature = |path: &str| vec![path.to_owned()];
    let triple =
        |from: &str, written: &str, to: &str| (from.to_owned(), written.to_owned(), feature(to));
    assert_eq!(
        scoped_citations("spec-a"),
        [
            triple(
                "docs/features/stamina-tuning.md",
                "AC-07",
                "docs/features/stamina-tuning.md"
            ),
            triple(
                "docs/spec/movement/sprint.md",
                "stamina-tuning/AC-07",
                "docs/features/stamina-tuning.md"
            ),
        ]
    );
    assert_eq!(
        scoped_citations("spec-b"),
        [
            triple(
                "docs/features/dry-run.md",
                "dry-run/CRIT-01",
                "docs/features/dry-run.md"
            ),
            triple(
                "docs/spec/cli.md",
                "dry-run/CRIT-01",
                "docs/features/dry-run.md"
            ),
        ]
    );
    for corpus in ["spec-a", "spec-b"] {
        let (_, input) = fixture_input(&fixture(corpus));
        assert!(
            input
                .files
                .iter()
                .all(|file| !file.path.contains("/AC/") && !file.path.contains("/CRIT/")),
            "{corpus}: no criterion record file"
        );
    }
}

/// Every string literal of a Rust line outside comments (naive: the text
/// between pairs of `"`; enough for the check sources).
fn literals(line: &str) -> Vec<&str> {
    let code = line.trim_start();
    if code.starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

fn check_sources() -> Vec<std::path::PathBuf> {
    let mut sources = Vec::new();
    let dir = repository_root().join("crates/specengine-core/src/check");
    for entry in fs::read_dir(&dir).expect("the check module is a directory") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            sources.push(path);
        }
    }
    sources.push(repository_root().join("crates/specengine-store/src/check.rs"));
    sources.sort();
    sources
}

/// Paths and file names of this repository and of the fixtures, and this
/// repository's generator commands (the retired one and the registered
/// prefix, docs/features/spec-cli-switch.md AC-15).
const PROJECT_NAMES: &[&str] = &[
    "cargo xtask",
    "xtask",
    "cargo run -q -p specengine-cli",
    "CLAUDE.md",
    "README.md",
    "index.md",
    "architecture.md",
    "docs/",
    "fixtures",
    "spec-a",
    "spec-b",
    "decisions",
    "records/",
    "features/",
    // docs/features/index-shards.md AC-11: the shard names of this
    // repository (`docs/index-archive.md`) and of the fixtures' scratch
    // configs in the shard tests.
    "index-archive",
    "index-records",
];

#[test]
fn the_check_source_names_no_prefix_path_or_file_of_a_project() {
    let mut prefixes: BTreeSet<String> = BTreeSet::new();
    for corpus in ["spec-a", "spec-b"] {
        for spec in corpus_scheme(&fixture(corpus)).prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
        }
    }
    // This repository's own scheme: `[ids] ADR` (spec, Data).
    prefixes.insert("ADR".to_owned());
    let mut offenders = Vec::new();
    let sources = check_sources();
    assert!(sources.len() >= 11, "{sources:?}");
    for new in ["render.rs", "generated.rs", "graph.rs", "resolve.rs"] {
        assert!(
            sources.iter().any(|path| path.ends_with(new)),
            "{new} is scanned: {sources:?}"
        );
    }
    for path in &sources {
        let text = fs::read_to_string(path).unwrap();
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                let names_prefix = prefixes.iter().any(|prefix| {
                    literal == prefix
                        || literal.starts_with(&format!("{prefix}-"))
                        || literal.contains(&format!(" {prefix}-"))
                        || literal.contains(&format!("`{prefix}-"))
                });
                let names_path = PROJECT_NAMES.iter().any(|name| literal.contains(name));
                if names_prefix || names_path {
                    offenders.push(format!(
                        "{}:{}: {:?} in {}",
                        relative(path),
                        number + 1,
                        literal,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "project names in the check source:\n{}",
        offenders.join("\n")
    );
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The sources the scope pass changed: the resolver, the rules, the graph
/// warnings, the code table, and the model grammar's `is_slug`.
const SCOPE_SOURCES: [&str; 6] = [
    "crates/specengine-core/src/check/resolve.rs",
    "crates/specengine-core/src/check/engine.rs",
    "crates/specengine-core/src/check/graph.rs",
    "crates/specengine-core/src/check/mod.rs",
    "crates/specengine-model/src/grammar.rs",
    // docs/features/spec-check-links.md: the home of `DOCUMENT_EXTENSION`.
    "crates/specengine-core/src/walk_scope.rs",
];

#[test]
fn the_scope_sources_name_no_prefix_slug_or_role_directory() {
    let mut prefixes: BTreeSet<String> = BTreeSet::new();
    let mut slugs: BTreeSet<String> = BTreeSet::new();
    for corpus in ["spec-a", "spec-b"] {
        for spec in corpus_scheme(&fixture(corpus)).prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
        }
        let features = fixture(corpus).join("docs").join("features");
        for entry in fs::read_dir(&features).expect("the fixture has feature documents") {
            let path = entry.unwrap().path();
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                slugs.insert(stem.to_owned());
            }
        }
    }
    prefixes.insert("ADR".to_owned());
    assert!(
        slugs.contains("stamina-tuning") && slugs.contains("dry-run"),
        "{slugs:?}"
    );
    let mut offenders = Vec::new();
    let mut extensions = Vec::new();
    for source in SCOPE_SOURCES {
        let path = repository_root().join(source);
        let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("{source}"));
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                let names_prefix = prefixes.iter().any(|prefix| {
                    literal == prefix
                        || literal.starts_with(&format!("{prefix}-"))
                        || literal.contains(&format!(" {prefix}-"))
                        || literal.contains(&format!("`{prefix}-"))
                });
                let names_slug = slugs.iter().any(|slug| {
                    literal == slug
                        || literal.contains(&format!("{slug}/"))
                        || literal.contains(&format!("/{slug}"))
                        || literal.contains(&format!("{slug}.md"))
                });
                let names_role = ["features/", "records/", "docs/", "features", "records"]
                    .iter()
                    .any(|role| literal.contains(role));
                if names_prefix || names_slug || names_role {
                    offenders.push(format!("{source}:{}: {literal:?}", number + 1));
                }
                let bytes = literal.as_bytes();
                if bytes.len() > 1
                    && bytes[0] == b'.'
                    && bytes[1..].iter().all(u8::is_ascii_alphanumeric)
                {
                    extensions.push(format!("{source}:{}: {literal}", number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "project names in the scope sources:\n{}",
        offenders.join("\n")
    );
    // The one extension literal: core's `DOCUMENT_EXTENSION`, moved from the
    // resolver to the walk scope (docs/features/spec-check-links.md).
    assert_eq!(extensions.len(), 1, "{extensions:?}");
    assert!(
        extensions[0].starts_with("crates/specengine-core/src/walk_scope.rs:")
            && extensions[0].ends_with(": .md"),
        "{extensions:?}"
    );
}

/// The `.md` literals of a source outside comments (and outside its
/// `#[cfg(test)]` module): literals naming the extension as a path part,
/// not prose quoting it as `` `.md` `` in a message.
fn md_literals(source: &str) -> Vec<String> {
    let path = repository_root().join(source);
    let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("{source}"));
    let mut found = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        for literal in literals(line) {
            if literal.replace("`.md`", "").contains(".md") {
                found.push(format!("{source}:{}: {literal}", number + 1));
            }
        }
    }
    found
}

/// docs/features/spec-check-links.md (iteration 3): one `.md` rule. The
/// only `.md` literal of the scope sources, the link rule and the store's
/// walker is `DOCUMENT_EXTENSION` in `walk_scope.rs`, re-exported by core.
#[test]
fn the_one_md_literal_is_the_core_document_extension() {
    assert_eq!(specengine_core::DOCUMENT_EXTENSION, ".md");
    let mut found = Vec::new();
    for source in SCOPE_SOURCES.iter().copied().chain([
        "crates/specengine-core/src/check/links.rs",
        "crates/specengine-store/src/source.rs",
    ]) {
        found.extend(md_literals(source));
    }
    assert_eq!(found.len(), 1, "the one `.md` literal: {found:#?}");
    assert!(
        found[0].starts_with("crates/specengine-core/src/walk_scope.rs:")
            && found[0].ends_with(": .md"),
        "{found:#?}"
    );
}

// ------------------------------------------------ docs/features/spec-check-links.md

/// `(code, path, line, subject)` of every file link finding.
fn link_findings(report: &specengine_core::check::Report) -> Vec<(String, String, usize, String)> {
    report
        .findings
        .iter()
        .filter(|f| f.code == "link-dangling" || f.code == "link-anchor")
        .map(|f| (f.code.clone(), f.path.clone(), f.line, f.subject.clone()))
        .collect()
}

/// AC-09: both fixtures record file links (spec-a two, spec-b one) and
/// give no link finding; spec-b's resolves only through its `link_base`.
#[test]
fn the_fixtures_links_resolve_and_spec_b_needs_its_base() {
    for (corpus, links) in [("spec-a", 2), ("spec-b", 1)] {
        let (config, input) = fixture_input(&fixture(corpus));
        let recorded = input
            .files
            .iter()
            .filter_map(|file| file.parsed.as_ref())
            .flat_map(|parsed| &parsed.links)
            .filter(|link| {
                link.origin == specengine_model::LinkOrigin::Inline
                    && matches!(link.dst, LinkTarget::Path(_))
            })
            .count();
        assert_eq!(recorded, links, "{corpus}: file links recorded");
        let report = config.run(&input);
        assert_eq!(link_findings(&report), [], "{corpus}:\n{}", show(&report));
    }
    let (mut config, input) = fixture_input(&fixture("spec-b"));
    assert_eq!(config.paths.link_base.as_deref(), Some("docs"));
    config.paths.link_base = None;
    let report = config.run(&input);
    assert_eq!(
        link_findings(&report),
        [(
            "link-dangling".to_owned(),
            "docs/records/REQ/REQ-001.md".to_owned(),
            14,
            "spec/cli.md#CMD-SYNC".to_owned()
        )],
        "{}",
        show(&report)
    );
}

/// AC-12 (ADR-0008): the link rule and the shared exclude matcher name no
/// project path, prefix or default base; the link rule has no extension
/// literal of its own (the resolver's `DOCUMENT_EXTENSION`); both are
/// scanned.
#[test]
fn the_link_sources_name_no_project_and_no_default_base() {
    let sources = check_sources();
    assert!(
        sources.iter().any(|path| path.ends_with("check/links.rs")),
        "links.rs is scanned with the check sources: {sources:?}"
    );
    let mut prefixes: BTreeSet<String> = BTreeSet::new();
    for corpus in ["spec-a", "spec-b"] {
        for spec in corpus_scheme(&fixture(corpus)).prefixes() {
            prefixes.insert(spec.prefix.clone());
            prefixes.extend(spec.aliases_from.iter().cloned());
        }
    }
    prefixes.insert("ADR".to_owned());
    let mut offenders = Vec::new();
    for source in [
        "crates/specengine-core/src/check/links.rs",
        "crates/specengine-core/src/glob.rs",
    ] {
        let path = repository_root().join(source);
        let text = fs::read_to_string(&path).unwrap_or_else(|_| panic!("{source}"));
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                let names_prefix = prefixes.iter().any(|prefix| {
                    literal == prefix
                        || literal.starts_with(&format!("{prefix}-"))
                        || literal.contains(&format!(" {prefix}-"))
                        || literal.contains(&format!("`{prefix}-"))
                });
                let names_path = PROJECT_NAMES.iter().any(|name| literal.contains(name))
                    || literal.contains("docs")
                    || literal.contains("design");
                let extension = literal.contains(".md");
                if names_prefix || names_path || extension {
                    offenders.push(format!("{source}:{}: {literal:?}", number + 1));
                }
            }
            if line.contains("link_base") && line.contains("unwrap_or") {
                offenders.push(format!(
                    "{source}:{}: a defaulted base: {}",
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "project names or defaults in the link sources:\n{}",
        offenders.join("\n")
    );
    assert_eq!(specengine_core::Paths::default().link_base, None);
}
