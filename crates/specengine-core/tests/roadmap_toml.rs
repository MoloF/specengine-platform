//! RD-1 of docs/features/roadmap-items.md ("`[roadmap]` in
//! `specengine.toml`"): `RoadmapConfig::from_toml` reads the table of both
//! fixtures (`roadmap-a`, `roadmap-b`), is `None` without it, and refuses
//! every bad value with its message at its line (`specengine.toml:<line>:
//! message`), the first error by line when there are several;
//! `ProjectConfig::from_toml` only name-checks `roadmap` (a bad table
//! fails no other reader; a `[roadmapp]` table is an unknown key).
//!
//! Every config is real TOML read by the real readers; nothing is mocked.

mod common;

use common::check::{blocking, fixture_input};
use common::fixture;
use specengine_core::check::CheckConfig;
use specengine_core::roadmap_toml::RoadmapConfig;
use specengine_core::{IdSchemeToml, Paths, ProjectConfig, ProjectError};
use specengine_model::IdScheme;

/// `[ids]` of every cell: the item prefix, a name-shaped prefix, a
/// feature-scoped one.
const IDS: &str = "[ids]\n\
RM  = { kind = \"plan-item\",   width = 4 }\n\
BAY = { kind = \"harbor-zone\", shape = \"name\" }\n\
CK  = { kind = \"check\",       width = 2, scope = \"feature\" }\n";

/// The six keys of a good table, in the spec's order.
const GOOD: [&str; 6] = [
    "prefix     = \"RM\"",
    "dir        = \"plan\"",
    "releases   = [\"alpha\", \"beta\", \"launch\"]",
    "mvp_cut    = \"beta\"",
    "priorities = [\"must\", \"should\", \"could\"]",
    "unit       = \"days\"",
];

/// A whole `specengine.toml` whose `[roadmap]` holds `lines`.
fn config_with(lines: &[&str]) -> String {
    format!(
        "[project]\nslug = \"probe\"\n\n{IDS}\n[roadmap]\n{}\n",
        lines.join("\n")
    )
}

/// [`GOOD`] with the line starting `key` replaced by `with` (`None`: removed).
fn good_but(key: &str, with: Option<&str>) -> String {
    let lines: Vec<&str> = GOOD
        .iter()
        .filter_map(|line| {
            if line.split_whitespace().next() == Some(key) {
                with
            } else {
                Some(*line)
            }
        })
        .collect();
    config_with(&lines)
}

fn read(text: &str) -> Result<Option<RoadmapConfig>, ProjectError> {
    let scheme =
        IdScheme::from_toml(text).unwrap_or_else(|error| panic!("{}", error.at("specengine.toml")));
    RoadmapConfig::from_toml(text, &scheme)
}

fn read_ok(text: &str) -> RoadmapConfig {
    read(text)
        .unwrap_or_else(|error| panic!("{}\n{text}", error.at("specengine.toml")))
        .unwrap_or_else(|| panic!("a [roadmap] table reads as Some:\n{text}"))
}

/// The 1-based line of the first line of `text` containing `needle`.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("{needle:?} in:\n{text}"))
        + 1
}

/// `text` is refused at the line holding `at`, its message holding each
/// of `fragments`; the error prints as `specengine.toml:<line>: message`.
#[track_caller]
fn refused(text: &str, at: &str, fragments: &[&str]) -> ProjectError {
    let error = match read(text) {
        Err(error) => error,
        Ok(config) => panic!("accepted ({config:?}):\n{text}"),
    };
    let line = line_of(text, at);
    assert_eq!(
        error.line,
        Some(line),
        "the line of {at:?}: {}\n{text}",
        error.message
    );
    for fragment in fragments {
        assert!(
            error.message.contains(fragment),
            "{fragment:?} in the message {:?}\n{text}",
            error.message
        );
    }
    assert_eq!(
        error.at("specengine.toml"),
        format!("specengine.toml:{line}: {}", error.message)
    );
    assert!(
        !error.message.contains('\n'),
        "one line: {:?}",
        error.message
    );
    error
}

// ------------------------------------------------------------ the fixtures

#[test]
fn both_fixture_tables_read() {
    let a = std::fs::read_to_string(fixture("roadmap-a").join("specengine.toml")).unwrap();
    let a = read_ok(&a);
    assert_eq!(a.prefix, "RM");
    assert_eq!(a.dir, "plan");
    assert_eq!(a.releases, ["alpha", "beta", "launch"]);
    assert_eq!(a.mvp_cut.as_deref(), Some("beta"));
    assert_eq!(a.priorities, ["must", "should", "could"]);
    assert_eq!(a.unit.as_deref(), Some("days"));

    let b = std::fs::read_to_string(fixture("roadmap-b").join("specengine.toml")).unwrap();
    let b = read_ok(&b);
    assert_eq!(b.prefix, "WORK");
    assert_eq!(b.dir, "work");
    assert_eq!(
        b.releases,
        ["now", "next", "later"],
        "config order, not sorted"
    );
    assert_eq!(b.mvp_cut, None);
    assert_eq!(b.priorities, ["p0", "p1", "p2"]);
    assert_eq!(b.unit, None);
}

#[test]
fn no_table_is_none() {
    for name in ["spec-a", "spec-b", "spec-c"] {
        let text = std::fs::read_to_string(fixture(name).join("specengine.toml")).unwrap();
        let scheme = IdScheme::from_toml(&text).unwrap();
        assert_eq!(
            RoadmapConfig::from_toml(&text, &scheme),
            Ok(None),
            "{name} has no [roadmap]"
        );
    }
    assert_eq!(read(&format!("[project]\nslug = \"x\"\n\n{IDS}")), Ok(None));
    // A key merely named like the table elsewhere is no table.
    let text = format!("[project]\nslug = \"x\"\n\n{IDS}\n[check]\nmode = \"observe\"\n");
    assert_eq!(read(&text), Ok(None));
}

#[test]
fn optional_keys_absent_and_a_trailing_slash_dropped() {
    let config = read_ok(&config_with(&[
        "prefix = \"RM\"",
        "dir = \"docs/roadmap/\"",
        "releases = [\"mvp\"]",
        "priorities = [\"high\"]",
    ]));
    assert_eq!(config.mvp_cut, None);
    assert_eq!(config.unit, None);
    assert_eq!(
        config.dir, "docs/roadmap",
        "the [paths] rules: one trailing / dropped"
    );
    assert_eq!(config.releases, ["mvp"]);
}

// ------------------------------------------------------------ RD-1 cells

#[test]
fn unknown_key_is_the_readers_unknown_field_listing_the_six() {
    let mut lines = GOOD.to_vec();
    lines.push("flavour    = 1");
    let text = config_with(&lines);
    refused(
        &text,
        "flavour",
        &[
            "unknown field `flavour`",
            "`prefix`",
            "`dir`",
            "`releases`",
            "`mvp_cut`",
            "`priorities`",
            "`unit`",
        ],
    );
}

#[test]
fn a_required_key_missing_is_refused() {
    for key in ["prefix", "dir", "releases", "priorities"] {
        let text = good_but(key, None);
        let error = read(&text).expect_err(&text);
        assert!(error.message.contains(key), "{key}: {}", error.message);
        // At the table: its header or one of its lines, never nowhere.
        let header = line_of(&text, "[roadmap]");
        let last = text.lines().count();
        assert!(
            error
                .line
                .is_some_and(|line| (header..=last).contains(&line)),
            "{key}: line {:?} outside the table ({header}..={last})",
            error.line
        );
    }
}

#[test]
fn a_value_of_another_type_names_both_types() {
    for (key, bad) in [
        ("prefix", "prefix = 3"),
        ("dir", "dir = [\"plan\"]"),
        ("releases", "releases = \"alpha\""),
        ("mvp_cut", "mvp_cut = 2"),
        ("priorities", "priorities = { must = 1 }"),
        ("unit", "unit = 5"),
    ] {
        let text = good_but(key, Some(bad));
        refused(
            &text,
            bad,
            &[&format!("`roadmap.{key}`: expected "), "found"],
        );
    }
    // An entry of a list of another type.
    let text = good_but("releases", Some("releases = [\"alpha\", 2]"));
    refused(&text, "releases = [", &["`roadmap.releases`"]);
    // The table itself of another type.
    let text = format!("roadmap = 3\n\n[project]\nslug = \"x\"\n\n{IDS}");
    refused(&text, "roadmap = 3", &["roadmap"]);
}

#[test]
fn prefix_undeclared_name_shaped_or_feature_scoped_is_refused() {
    // Undeclared, of shape `name`, of scope `feature`.
    for prefix in ["XX", "BAY", "CK"] {
        let bad = format!("prefix = \"{prefix}\"");
        let text = good_but("prefix", Some(&bad));
        refused(&text, &bad, &["roadmap.prefix", prefix]);
    }
    // A lower-case spelling of a declared prefix is not that prefix.
    let text = good_but("prefix", Some("prefix = \"rm\""));
    refused(&text, "prefix = \"rm\"", &["roadmap.prefix"]);
}

#[test]
fn dir_breaking_the_path_rules_is_refused() {
    for bad in [
        "/plan",
        "../plan",
        "docs/../plan",
        "docs/./plan",
        "docs//plan",
        "",
    ] {
        let line = format!("dir = \"{bad}\"");
        let text = good_but("dir", Some(&line));
        refused(&text, &line, &["roadmap.dir"]);
    }
}

#[test]
fn empty_releases_or_priorities_are_refused() {
    let text = good_but("releases", Some("releases = []"));
    refused(&text, "releases = []", &["roadmap.releases"]);
    let text = good_but("priorities", Some("priorities = []"));
    refused(&text, "priorities = []", &["roadmap.priorities"]);
}

#[test]
fn an_entry_that_is_no_slug_is_refused() {
    let long = format!("a{}", "b".repeat(64));
    assert_eq!(long.len(), 65);
    for bad in [
        "Beta",
        "1x",
        "-x",
        "a_b",
        "a b",
        "",
        "be\u{3b2}a",
        long.as_str(),
    ] {
        for key in ["releases", "priorities"] {
            let line = format!("{key} = [\"ok\", \"{bad}\"]");
            let text = good_but(key, Some(&line));
            let text = if key == "releases" {
                text.replace("mvp_cut    = \"beta\"", "mvp_cut    = \"ok\"")
            } else {
                text
            };
            refused(&text, &line, &[&format!("roadmap.{key}")]);
        }
    }
    // 64 bytes is a slug.
    let at_cap = format!("a{}", "b".repeat(63));
    let line = format!("releases = [\"{at_cap}\"]");
    let text = good_but("releases", Some(&line)).replace("mvp_cut    = \"beta\"\n", "");
    assert_eq!(read_ok(&text).releases, [at_cap]);
}

#[test]
fn release_repeated_is_refused() {
    let line = "releases = [\"v1\", \"v2\", \"v1\"]";
    let text = good_but("releases", Some(line)).replace("\"beta\"\n", "\"v2\"\n");
    let error = refused(&text, line, &[]);
    assert_eq!(error.message, "`roadmap.releases`: \"v1\" is repeated");
}

#[test]
fn priority_repeated_is_refused() {
    let line = "priorities = [\"must\", \"could\", \"must\"]";
    let text = good_but("priorities", Some(line));
    let error = refused(&text, line, &[]);
    assert_eq!(error.message, "`roadmap.priorities`: \"must\" is repeated");
}

#[test]
fn mvp_cut_not_a_release_is_refused() {
    let text = good_but("mvp_cut", Some("mvp_cut = \"gamma\""));
    refused(&text, "mvp_cut = \"gamma\"", &["roadmap.mvp_cut", "gamma"]);
    // A priority is no release.
    let text = good_but("mvp_cut", Some("mvp_cut = \"must\""));
    refused(&text, "mvp_cut = \"must\"", &["roadmap.mvp_cut"]);
}

#[test]
fn unit_blank_long_or_with_a_control_character_is_refused() {
    let long = "d".repeat(33);
    for bad in [
        "\"\"".to_owned(),
        "\"   \"".to_owned(),
        format!("\"{long}\""),
        "\"da\\tys\"".to_owned(),
        "\"da\\nys\"".to_owned(),
        "\"days\\u0007\"".to_owned(),
    ] {
        let line = format!("unit = {bad}");
        let text = good_but("unit", Some(&line));
        refused(&text, &line, &["roadmap.unit"]);
    }
    // 32 bytes, and a unit of another script, read as written.
    let cap = "d".repeat(32);
    let line = format!("unit = \"{cap}\"");
    assert_eq!(read_ok(&good_but("unit", Some(&line))).unit, Some(cap));
    let line = "unit = \"\u{3b7}\u{3bc}\u{3ad}\u{3c1}\u{3b5}\u{3c2}\"";
    assert_eq!(
        read_ok(&good_but("unit", Some(line))).unit.as_deref(),
        Some("\u{3b7}\u{3bc}\u{3ad}\u{3c1}\u{3b5}\u{3c2}")
    );
}

#[test]
fn the_first_error_by_line_is_reported() {
    // A bad unit (last line) and an unknown key above it: the key's.
    let text = config_with(&[
        "prefix = \"RM\"",
        "zzz = 1",
        "dir = \"plan\"",
        "releases = [\"a\"]",
        "priorities = [\"p\"]",
        "unit = \"\"",
    ]);
    refused(&text, "zzz = 1", &["unknown field `zzz`"]);
    // A bad prefix above a repeated release: the prefix's.
    let text = config_with(&[
        "prefix = \"XX\"",
        "dir = \"plan\"",
        "releases = [\"a\", \"a\"]",
        "priorities = [\"p\"]",
    ]);
    refused(&text, "prefix = \"XX\"", &["roadmap.prefix"]);
    // Reversed: the repeated release above the bad prefix.
    let text = config_with(&[
        "releases = [\"a\", \"a\"]",
        "dir = \"plan\"",
        "prefix = \"XX\"",
        "priorities = [\"p\"]",
    ]);
    refused(&text, "releases = [", &["\"a\" is repeated"]);
}

#[test]
fn invalid_toml_is_an_error_at_its_line() {
    let good = format!("[project]\nslug = \"x\"\n\n{IDS}");
    let scheme = IdScheme::from_toml(&good).unwrap();
    let text = format!("{good}\n[roadmap]\nprefix = \"RM\ndir = \"plan\"\n");
    let error = RoadmapConfig::from_toml(&text, &scheme).expect_err("unterminated string");
    assert_eq!(
        error.line,
        Some(line_of(&text, "prefix = \"RM")),
        "{error:?}"
    );
}

/// RD-1: the table is only name-checked by the reader every command uses:
/// `roadmap-a` and `roadmap-b` load as projects and as check configs (the
/// tables `spec check` and `spec tree` read), and their corpora check with
/// no blocking finding; a bad `[roadmap]` still loads (it fails `spec
/// roadmap` alone); `[roadmapp]` is an unknown top-level key at its line.
#[test]
fn roadmap_table_is_name_checked() {
    for name in ["roadmap-a", "roadmap-b"] {
        let corpus = fixture(name);
        let text = std::fs::read_to_string(corpus.join("specengine.toml")).unwrap();
        let project =
            ProjectConfig::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at(name)));
        assert!(project.slug().is_ok(), "{name}");
        CheckConfig::from_toml(&text).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        Paths::from_toml(&text).unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let (config, input) = fixture_input(&corpus);
        let report = config.run(&input);
        assert!(
            report.cannot_check.is_empty(),
            "{name}: {:?}",
            report.cannot_check
        );
        assert_eq!(blocking(&report), Vec::new(), "{name}: blocking findings");
        // The cycle each fixture holds is the check's warning, not the
        // roadmap's business.
        assert!(
            report.findings.iter().any(|f| f.code == "depends-cycle"),
            "{name}: the fixture's cycle is a depends-cycle warning"
        );

        // A bad table loads everywhere but in `spec roadmap`.
        let bad = text.replace("releases   = [", "releases   = [\"x\", \"x\", ");
        assert!(read(&bad).is_err(), "{name}: the bad table is refused here");
        ProjectConfig::from_toml(&bad).unwrap_or_else(|error| panic!("{}", error.at(name)));
        CheckConfig::from_toml(&bad).unwrap_or_else(|error| panic!("{name}: {error:?}"));

        // A misspelt table is an unknown key of the file.
        let misspelt = text.replace("[roadmap]", "[roadmapp]");
        assert_ne!(misspelt, text);
        let error = ProjectConfig::from_toml(&misspelt).expect_err("[roadmapp]");
        assert_eq!(
            error.line,
            Some(line_of(&misspelt, "[roadmapp]")),
            "{name}: {}",
            error.message
        );
        assert!(
            error.message.contains("unknown field `roadmapp`"),
            "{name}: {}",
            error.message
        );
        assert!(
            error.message.contains("`roadmap`"),
            "{name}: the known keys list `roadmap`: {}",
            error.message
        );
    }
}
