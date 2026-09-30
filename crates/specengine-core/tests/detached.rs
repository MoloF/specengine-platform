//! AC-19 of docs/features/spec-cli.md (core): `check::is_tier3_file`, the
//! one Tier 3 predicate of the index's `tier3` column, the generated
//! index's archive and `spec search` (false when the front-matter failed or
//! the file is not UTF-8); and `Resolver::resolve_detached`, `resolve` with
//! no citing file, where a bare feature-scoped ID resolves wherever it is
//! defined.

mod common;

use common::check::{Config, fixture_input};
use common::fixture;
use specengine_core::check::{Resolution, Resolver, is_live, is_tier3_file};
use specengine_model::{LinkTarget, Reference, grammar};

const TOML: &str = "\
[ids]
R   = { kind = \"requirement\", width = 2 }
DEC = { kind = \"decision\",    width = 4 }
AC  = { kind = \"criterion\",   width = 2, scope = \"feature\" }
MEC = { kind = \"mechanic\",    shape = \"name\" }
Q   = { kind = \"question\",    width = 3, aliases_from = [\"QST\"] }
";

fn tier3_of(config: &Config, text: &[u8]) -> bool {
    let parsed = specengine_core::parse("docs/x.md", text, &config.scheme);
    is_tier3_file(&parsed)
}

#[test]
fn is_tier3_file_by_class_and_status() {
    let config = Config::from_toml(TOML);
    for (front, tier3) in [
        ("class: spec\nstatus: shipped", true),
        ("class: spec\nstatus: abandoned", true),
        ("class: spec\nstatus: draft", false),
        ("class: spec", false),
        ("class: decision\nstatus: superseded-by DEC-0002", true),
        ("class: decision\nstatus: rejected", true),
        ("class: decision\nstatus: accepted", false),
        ("class: decision", false),
        ("class: canon\nstatus: superseded-by R-02", false),
        ("class: generated", false),
        ("status: shipped", false),
    ] {
        let text = format!("---\n{front}\n---\n\n# X\n\nBody.\n");
        assert_eq!(tier3_of(&config, text.as_bytes()), tier3, "{front:?}");
    }
    // A failed front-matter, a file without one, a non-UTF-8 file: false.
    for text in [
        b"---\nclass: decision\nstatus: rejected\n\n# never closed\n".as_slice(),
        b"---\nclass: [decision\nstatus: rejected\n---\n\n# X\n".as_slice(),
        b"# Plain\n".as_slice(),
        b"---\nclass: decision\nstatus: rejected\n---\n\n# Bad \xff\n".as_slice(),
    ] {
        assert!(
            !tier3_of(&config, text),
            "{:?}",
            String::from_utf8_lossy(text)
        );
    }
}

/// The fixtures: exactly their superseded records are Tier 3, and no
/// Tier 3 file is live.
#[test]
fn the_fixtures_tier3_files_are_their_superseded_records() {
    for (name, tier3) in [
        ("spec-a", vec!["docs/records/DEC/DEC-0007.md"]),
        ("spec-b", vec!["docs/records/ADR/ADR-0002.md"]),
    ] {
        let (_, input) = fixture_input(&fixture(name));
        let found: Vec<&str> = input
            .files
            .iter()
            .filter(|file| file.parsed.as_ref().is_some_and(is_tier3_file))
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(found, tier3, "{name}");
        for file in &input.files {
            let parsed = file.parsed.as_ref().unwrap();
            if is_tier3_file(parsed) {
                assert!(!is_live(parsed), "{name}: {}", file.path);
            }
        }
    }
}

fn reference(config: &Config, written: &str) -> Reference {
    grammar::parse_reference(written, 0, &config.scheme)
        .unwrap_or_else(|| panic!("{written} is a reference"))
        .reference
}

fn feature(title: &str, body: &str) -> String {
    format!("---\nclass: spec\nstatus: draft\n---\n\n# {title}\n\n{body}")
}

fn corpus(config: &Config) -> specengine_core::check::CheckInput {
    let other = feature("Other", "## One {#AC-01}\n\nOther's.\n");
    let feat = feature("Feat", "## One {#AC-01}\n\n## Two {#AC-02}\n");
    let spec = "---\nid: MEC-RUN\nclass: canon\n---\n\n# Run\n\n## Fast {#MEC-RUN-FAST}\n";
    let q = "---\nid: Q-001\nclass: canon\n---\n\n# Q\n";
    let r = "---\nid: R-01\nclass: canon\naliases: [R-02]\n---\n\n# R\n";
    config.input(&[
        ("docs/features/other.md", &other),
        ("docs/spec/run.md", spec),
        ("docs/features/feat.md", &feat),
        ("docs/records/Q-001.md", q),
        ("docs/records/R-01.md", r),
    ])
}

#[test]
fn resolve_detached_finds_a_bare_feature_scoped_id_wherever_defined() {
    let config = Config::from_toml(TOML);
    let input = corpus(&config);
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    let index_of = |path: &str| resolver.paths().iter().position(|p| *p == path).unwrap();
    let feat = index_of("docs/features/feat.md");
    let other = index_of("docs/features/other.md");

    // Bare AC-01: both feature documents, in path order; the check's
    // `resolve` with no citing feature document leaves it dangling.
    let bare = reference(&config, "AC-01");
    assert_eq!(
        resolver.resolve_detached(&bare, "AC-01"),
        Resolution::Resolved(vec![feat, other])
    );
    assert!(matches!(
        resolver.resolve("", &bare, "AC-01"),
        Resolution::Dangling(_)
    ));
    assert!(matches!(
        resolver.resolve("docs/spec/run.md", &bare, "AC-01"),
        Resolution::Dangling(_)
    ));
    // Defined once: that document.
    let two = reference(&config, "AC-02");
    assert_eq!(
        resolver.resolve_detached(&two, "AC-02"),
        Resolution::Resolved(vec![feat])
    );
    // `slug/ID`: its feature document only, as `resolve` does.
    let scoped = reference(&config, "other/AC-01");
    assert_eq!(
        resolver.resolve_detached(&scoped, "other/AC-01"),
        Resolution::Resolved(vec![other])
    );
    assert_eq!(
        resolver.resolve_detached(&scoped, "other/AC-01"),
        resolver.resolve("", &scoped, "other/AC-01")
    );
    let missing = reference(&config, "other/AC-02");
    assert!(matches!(
        resolver.resolve_detached(&missing, "other/AC-02"),
        Resolution::Dangling(_)
    ));
    let nowhere = reference(&config, "nope/AC-01");
    assert!(matches!(
        resolver.resolve_detached(&nowhere, "nope/AC-01"),
        Resolution::Dangling(_)
    ));
    // `project:` is skipped.
    let project = reference(&config, "p:feat/AC-01");
    assert_eq!(
        resolver.resolve_detached(&project, "p:feat/AC-01"),
        Resolution::Skipped
    );
}

#[test]
fn resolve_detached_equals_resolve_for_project_scoped_ids_and_aliases() {
    let config = Config::from_toml(TOML);
    let input = corpus(&config);
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    for written in [
        "MEC-RUN",
        "MEC-RUN#MEC-RUN-FAST",
        "MEC-RUN-FAST",
        "Q-001",
        "QST-001",
        "R-01",
        "R-02",
        "R-01@4",
        "R-99",
        "MEC-RUN#MEC-NOPE",
    ] {
        let reference = reference(&config, written);
        assert_eq!(
            resolver.resolve_detached(&reference, written),
            resolver.resolve("", &reference, written),
            "{written}"
        );
    }
    // Not vacuous: all but the last two resolve.
    for written in [
        "MEC-RUN",
        "MEC-RUN#MEC-RUN-FAST",
        "MEC-RUN-FAST",
        "Q-001",
        "QST-001",
        "R-01",
        "R-02",
        "R-01@4",
    ] {
        let reference = reference(&config, written);
        assert!(
            matches!(
                resolver.resolve_detached(&reference, written),
                Resolution::Resolved(_)
            ),
            "{written}"
        );
    }
    // No name fallback: a longer name is not cut back to its parent.
    let longer = reference(&config, "MEC-RUN-SLOW");
    assert!(matches!(
        resolver.resolve_detached(&longer, "MEC-RUN-SLOW"),
        Resolution::Dangling(_)
    ));
    assert!(matches!(
        resolver.resolve_mention("docs/spec/run.md", &longer, "MEC-RUN-SLOW"),
        Resolution::Resolved(_)
    ));
}

/// Over the spec-a fixture: the feature-scoped `AC-07` of
/// `stamina-tuning` resolves detached, from nowhere.
#[test]
fn resolve_detached_over_spec_a() {
    let (config, input) = fixture_input(&fixture("spec-a"));
    let resolver = Resolver::new(&input, &config.scheme, &config.paths);
    let tuning = resolver
        .paths()
        .iter()
        .position(|path| *path == "docs/features/stamina-tuning.md")
        .unwrap();
    for written in ["AC-07", "stamina-tuning/AC-07"] {
        let reference = reference(&config, written);
        assert_eq!(
            resolver.resolve_detached(&reference, written),
            Resolution::Resolved(vec![tuning])
        );
    }
    // The link form in the fixture is the same reference type.
    let parsed = input.files[tuning].parsed.as_ref().unwrap();
    assert!(
        parsed
            .links
            .iter()
            .any(|link| matches!(&link.dst, LinkTarget::Reference(r) if r.id == "AC-07"))
    );
}
