//! docs/features/plugin-gate.md, the pure half (core `gate.rs`; canon
//! `docs/canon/gate.md` "Config", "Rule"):
//!
//! - AC-01: no `[gate]` is off (`None`); each bad `[gate]` or `[zones]`
//!   an error at its line, while `ProjectConfig::from_toml` (the reads)
//!   still loads the same text. M: absent read as `selective`; a bad
//!   `[gate]` fails the reads.
//! - `mode = "off"` reads only `[gate]`'s keys and its mode: undeclared
//!   zones, no `[project]` pass (the recovery path).
//! - A file that is not TOML: an error when a line names the table
//!   ([`mentions_gate`]), else off; `mentions_gate` on the widened forms
//!   and the near misses.
//! - `controlled`: a listed zone's glob, case-sensitively, the first
//!   listed zone winning; any path whose last component is the config's
//!   name in any ASCII case, at any depth, zone `None`; nothing while
//!   off. M: the raw `rel == "specengine.toml"`.
//! - `opens`: only an `in_progress` task claimed on that branch in a
//!   worktree the caller takes as the file's; detached opens nothing; the
//!   lowest number by value. M: `ready` opens; the branch ignored.
//! - `fold`: lexical `.`/`..` folding.
//!
//! Two projects of different nature (`fixtures/spec-a`, a game design;
//! `fixtures/spec-c`, a web service in Python) take the same tables.

mod common;

use std::path::{Path, PathBuf};

use common::fixture;
use specengine_core::ProjectConfig;
use specengine_core::gate::{GateConfig, GateMode, GateTask, fold, mentions_gate, opens};
use specengine_model::{TaskClaim, TaskStatus};

const CONFIG: &str = "specengine.toml";

/// `[project] slug`, `[zones]` with `zones` (one entry per line), `[gate]`
/// with `gate`: `[zones]`'s first entry on line 5, `[gate]` on line 6 +
/// the zone lines, its first key the line after ([`ZONES`]: `[gate]` on
/// line 8, its keys from line 9).
fn config(zones: &str, gate: &str) -> String {
    format!("[project]\nslug = \"lantern-keep\"\n\n[zones]\n{zones}\n\n[gate]\n{gate}\n")
}

const ZONES: &str = "code = [\"src/**\", \"Cargo.toml\"]\ntests = [\"tests/**\"]";

fn gate(text: &str) -> GateConfig {
    GateConfig::from_toml(text)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.at(CONFIG)))
        .unwrap_or_else(|| panic!("a [gate] table:\n{text}"))
}

fn error(text: &str) -> (Option<usize>, String) {
    let error = GateConfig::from_toml(text).expect_err(text);
    (error.line, error.message)
}

// ------------------------------------------------------------------ AC-01

#[test]
fn ac01_no_gate_table_is_off_on_both_fixtures() {
    for name in ["spec-a", "spec-c"] {
        let text = std::fs::read_to_string(fixture(name).join(CONFIG)).unwrap();
        assert_eq!(GateConfig::from_toml(&text), Ok(None), "{name}");
        // `[zones]` alone is unread: even a bad one.
        let zoned = format!("{text}\n[zones]\nCode = []\n");
        assert_eq!(GateConfig::from_toml(&zoned), Ok(None), "{name} + [zones]");
        assert!(ProjectConfig::from_toml(&zoned).is_ok(), "{name}");
        // The same project, gated, reads its own slug.
        let gated = format!(
            "{text}\n[zones]\napp = [\"src/**\"]\n\n[gate]\nmode = \"selective\"\nzones = [\"app\"]\n"
        );
        let config = gate(&gated);
        assert_eq!(config.mode, GateMode::Selective, "{name}");
        assert_eq!(config.zones.len(), 1);
        assert_eq!(config.zones[0].name, "app");
        assert_eq!(config.zones[0].globs, ["src/**"]);
        let slug = ProjectConfig::from_toml(&text)
            .unwrap()
            .slug()
            .unwrap()
            .to_owned();
        assert_eq!(config.slug.as_deref(), Some(slug.as_str()), "{name}");
    }
    // M: absent read as `selective` -- the empty file, too.
    assert_eq!(GateConfig::from_toml(""), Ok(None));
}

#[test]
fn ac01_a_valid_table_reads_mode_zones_and_slug() {
    for (mode, want) in [
        ("selective", GateMode::Selective),
        ("observe", GateMode::Observe),
    ] {
        let text = config(
            ZONES,
            &format!("mode = \"{mode}\"\nzones = [\"tests\", \"code\"]"),
        );
        let config = gate(&text);
        assert_eq!(config.mode, want);
        let names: Vec<&str> = config.zones.iter().map(|zone| zone.name.as_str()).collect();
        assert_eq!(names, ["tests", "code"], "[gate] zones order");
        assert_eq!(config.slug.as_deref(), Some("lantern-keep"));
        assert!(ProjectConfig::from_toml(&text).is_ok());
    }
    assert_eq!(GateMode::parse("selective"), Some(GateMode::Selective));
    assert_eq!(GateMode::parse("strict"), None);
    assert_eq!(GateMode::Observe.to_string(), "observe");
}

/// Each bad table: an error at its line naming the problem, while the
/// reads' loader takes the same text.
#[test]
fn ac01_each_bad_table_is_an_error_at_its_line_and_the_reads_still_load() {
    let selective = "mode = \"selective\"\nzones = [\"code\"]";
    let cases: Vec<(&str, String, Option<usize>, &str)> = vec![
        (
            "an unknown key",
            config(ZONES, &format!("{selective}\ncolour = \"red\"")),
            Some(11),
            "has no key `colour`",
        ),
        (
            "mode strict",
            config(ZONES, "mode = \"strict\"\nzones = [\"code\"]"),
            Some(9),
            "`strict` is not built",
        ),
        (
            "mode on",
            config(ZONES, "mode = \"on\"\nzones = [\"code\"]"),
            Some(9),
            "\"on\" is no mode",
        ),
        (
            "min_files",
            config(ZONES, &format!("{selective}\nmin_files = 3")),
            Some(11),
            "`[gate] min_files` is not built",
        ),
        (
            "contours",
            config(ZONES, &format!("{selective}\ncontours = []")),
            Some(11),
            "`[gate] contours` is not built",
        ),
        (
            "an undeclared zone",
            config(ZONES, "mode = \"selective\"\nzones = [\"code\", \"docs\"]"),
            Some(10),
            "lists `docs`, which `[zones]` does not declare",
        ),
        (
            "a glob src/[",
            config("code = [\"src/[\"]", selective),
            Some(5),
            "holds `[`",
        ),
        (
            "a glob with braces",
            config("code = [\"src/{a,b}/**\"]", selective),
            Some(5),
            "holds `{`",
        ),
        (
            "a zone Code",
            config(
                "Code = [\"src/**\"]",
                "mode = \"selective\"\nzones = [\"Code\"]",
            ),
            Some(5),
            "is no zone name",
        ),
        (
            "a 65-byte zone name",
            config(
                &format!("{} = [\"src/**\"]", "a".repeat(65)),
                &format!("mode = \"selective\"\nzones = [\"{}\"]", "a".repeat(65)),
            ),
            Some(5),
            "1 to 64 bytes",
        ),
        (
            "no mode",
            config(ZONES, "zones = [\"code\"]"),
            None,
            "has no `mode`",
        ),
        (
            "mode not a string",
            config(ZONES, "mode = 1\nzones = [\"code\"]"),
            Some(9),
            "not a string",
        ),
        (
            "no zones",
            config(ZONES, "mode = \"selective\""),
            None,
            "`[gate] zones` is missing or empty",
        ),
        (
            "empty zones",
            config(ZONES, "mode = \"observe\"\nzones = []"),
            Some(10),
            "`[gate] zones` is missing or empty",
        ),
        (
            "zones not a list",
            config(ZONES, "mode = \"selective\"\nzones = \"code\""),
            Some(10),
            "not a list of zone names",
        ),
        (
            "a zone listed twice",
            config(ZONES, "mode = \"selective\"\nzones = [\"code\", \"code\"]"),
            Some(10),
            "lists `code` twice",
        ),
        (
            "a zone with no glob",
            config("code = []", selective),
            Some(5),
            "has no glob",
        ),
        (
            "a glob ../x",
            config("code = [\"../x\"]", selective),
            Some(5),
            "`[zones] code`",
        ),
        (
            "an empty glob",
            config("code = [\"\"]", selective),
            Some(5),
            "`[zones] code`",
        ),
        (
            "a zone not a list",
            config("code = \"src/**\"", selective),
            Some(5),
            "",
        ),
    ];
    for (what, text, line, needle) in cases {
        let (got_line, message) = error(&text);
        match line {
            Some(line) => assert_eq!(got_line, Some(line), "{what}: {message}\n{text}"),
            // The table's own span: its header or first key.
            None => assert!(
                matches!(got_line, Some(8..=9)),
                "{what}: line {got_line:?}: {message}\n{text}"
            ),
        }
        assert!(
            message.contains(needle),
            "{what}: {message:?} lacks {needle:?}"
        );
        let at = specengine_core::ProjectError {
            line: got_line,
            message: message.clone(),
        }
        .at("/r/specengine.toml");
        assert!(
            at.starts_with(&format!("/r/specengine.toml:{}: ", got_line.unwrap())),
            "{at}"
        );
        // M: a bad `[gate]` fails the reads.
        ProjectConfig::from_toml(&text)
            .unwrap_or_else(|e| panic!("{what}: the reads fail: {}", e.at(CONFIG)));
    }
}

#[test]
fn ac01_gate_needs_the_project_slug_unless_off() {
    let no_project =
        format!("[zones]\n{ZONES}\n\n[gate]\nmode = \"selective\"\nzones = [\"code\"]\n");
    let (line, message) = error(&no_project);
    assert!(line.is_some(), "{message}");
    assert!(message.contains("`[project] slug`"), "{message}");
    let no_slug = no_project.replacen("[zones]", "[project]\nname = \"x\"\n\n[zones]", 1);
    let (_, message) = error(&no_slug);
    assert!(message.contains("no `slug`"), "{message}");
    let bad_slug = no_project.replacen("[zones]", "[project]\nslug = \"Bad Slug\"\n\n[zones]", 1);
    let (line, message) = error(&bad_slug);
    assert_eq!(line, Some(2), "{message}");
}

#[test]
fn off_reads_only_the_gate_keys_and_its_mode() {
    // No `[project]`, an undeclared zone, a bad `[zones]`: off still.
    let text = "[zones]\nCode = []\n\n[gate]\nmode = \"off\"\nzones = [\"nope\", \"nope\"]\n";
    let config = gate(text);
    assert_eq!(config.mode, GateMode::Off);
    assert!(config.zones.is_empty());
    assert_eq!(config.slug, None);
    assert_eq!(gate("[gate]\nmode = \"off\"\n").mode, GateMode::Off);
    // The table's keys are still checked.
    let (line, message) = error("[gate]\nmode = \"off\"\ncolour = 1\n");
    assert_eq!(line, Some(3), "{message}");
    let (line, _) = error("[gate]\nmode = \"strict\"\n");
    assert_eq!(line, Some(2));
}

#[test]
fn a_file_that_is_not_toml_is_an_error_only_when_a_line_names_gate() {
    for text in [
        "[gate]\nmode = \n",
        "[project\nslug = \"x\"\n[ gate ]\n",
        "x = \ngate.mode = \"selective\"\n",
        "[\"gate\"]\nmode = = 1\n",
    ] {
        let (line, message) = error(text);
        assert!(line.is_some(), "{text:?}: {message}");
        assert!(message.contains("not TOML"), "{message}");
        assert!(message.contains("`[gate]`"), "{message}");
    }
    for text in [
        "[project\nslug = \"x\"\n",
        "x = \n[gateway]\n",
        "# [gate]\nx = \n",
    ] {
        assert_eq!(GateConfig::from_toml(text), Ok(None), "{text:?}");
    }
}

#[test]
fn mentions_gate_takes_the_widened_forms_and_refuses_near_misses() {
    for line in [
        "[gate]",
        "  [gate]",
        "\t[gate]  # the gate",
        "[ gate ]",
        "[\"gate\"]",
        "['gate']",
        "[gate.x]",
        "[ gate.zones ]",
        "gate.mode = \"selective\"",
        "gate = { mode = \"off\" }",
        "gate=1",
        "\"gate\".mode = 1",
        "'gate' = {}",
    ] {
        assert!(mentions_gate(line), "{line:?}");
        assert!(
            mentions_gate(&format!("x = 1\n{line}\ny = 2")),
            "{line:?} inside"
        );
    }
    for line in [
        "[gates]",
        "[gateway]",
        "gateway = 1",
        "gates.mode = 1",
        "# [gate]",
        "x = \"[gate]\"",
        "agate = 1",
        "[ g ate ]",
        "[zones.gate_x]",
        "",
    ] {
        assert!(!mentions_gate(line), "{line:?}");
    }
}

// --------------------------------------------------------------- controlled

#[test]
fn controlled_matches_listed_zones_case_sensitively_and_the_config_at_any_depth() {
    for mode in ["selective", "observe"] {
        let config = gate(&config(
            ZONES,
            &format!("mode = \"{mode}\"\nzones = [\"code\"]"),
        ));
        let zone = |rel: &str| {
            config
                .controlled(rel, CONFIG)
                .map(|control| control.zone.unwrap_or_else(|| "<config>".to_owned()))
        };
        for rel in ["src/a.rs", "src/new/dir/b.rs", "Cargo.toml", "src/x/y/z.py"] {
            assert_eq!(zone(rel).as_deref(), Some("code"), "{mode} {rel}");
        }
        // Globs compare case-sensitively; an unlisted zone is no control.
        for rel in [
            "SRC/a.rs",
            "cargo.toml",
            "docs/a.md",
            "tests/a.rs",
            "srcx/a.rs",
            "a.rs",
        ] {
            assert_eq!(zone(rel), None, "{mode} {rel}");
        }
        // M: the raw `rel == "specengine.toml"`: any depth, any ASCII
        // case, inside a zone too, is the config (zone `None`).
        for rel in [
            "specengine.toml",
            "SpecEngine.toml",
            "SPECENGINE.TOML",
            "sub/specengine.toml",
            "src/x/SpecEngine.toml",
            "docs/spec/specengine.toml",
            "crates/foo/specengine.toml",
        ] {
            assert_eq!(zone(rel).as_deref(), Some("<config>"), "{mode} {rel}");
        }
        for rel in [
            "specengine.toml.bak",
            "notspecengine.toml",
            "specengine.toml/x",
            "docs/specengine.tom",
        ] {
            assert_eq!(zone(rel), None, "{mode} {rel}");
        }
    }
}

#[test]
fn a_zone_whose_parent_is_outside_still_controls_a_nested_config() {
    let text = config(
        "code = [\"crates/*/src/**\"]",
        "mode = \"selective\"\nzones = [\"code\"]",
    );
    let config = gate(&text);
    let control = |rel: &str| config.controlled(rel, CONFIG);
    assert_eq!(
        control("crates/foo/src/lib.rs").unwrap().zone.as_deref(),
        Some("code")
    );
    assert_eq!(control("crates/foo/specengine.toml").unwrap().zone, None);
    assert_eq!(control("crates/foo/README.md"), None);
}

#[test]
fn the_first_listed_zone_names_the_control() {
    let zones = "code = [\"src/**\"]\ntests = [\"src/tests/**\"]";
    let first = gate(&config(
        zones,
        "mode = \"selective\"\nzones = [\"tests\", \"code\"]",
    ));
    assert_eq!(
        first
            .controlled("src/tests/a.rs", CONFIG)
            .unwrap()
            .zone
            .as_deref(),
        Some("tests")
    );
    let second = gate(&config(
        zones,
        "mode = \"selective\"\nzones = [\"code\", \"tests\"]",
    ));
    assert_eq!(
        second
            .controlled("src/tests/a.rs", CONFIG)
            .unwrap()
            .zone
            .as_deref(),
        Some("code")
    );
}

#[test]
fn nothing_is_controlled_while_off() {
    let config = gate(&config(ZONES, "mode = \"off\"\nzones = [\"code\"]"));
    for rel in [
        "src/a.rs",
        "specengine.toml",
        "SpecEngine.toml",
        "sub/specengine.toml",
    ] {
        assert_eq!(config.controlled(rel, CONFIG), None, "{rel}");
    }
}

// -------------------------------------------------------------------- opens

fn task(id: &str, status: TaskStatus, claim: Option<(&str, &str)>) -> GateTask {
    GateTask {
        id: id.to_owned(),
        status,
        claim: claim.map(|(worktree, branch)| TaskClaim {
            at: "2026-10-08T00:00:00Z".to_owned(),
            role: "implementer".to_owned(),
            worktree: worktree.to_owned(),
            branch: branch.to_owned(),
        }),
    }
}

#[test]
fn opens_only_an_in_progress_task_claimed_on_the_branch_here() {
    let here = |worktree: &str| worktree == "/w1";
    // M: `ready` opens -- every other status, even with a claim, does not.
    for status in [
        TaskStatus::Draft,
        TaskStatus::Analysis,
        TaskStatus::Review,
        TaskStatus::ChangesRequested,
        TaskStatus::Ready,
        TaskStatus::InReview,
        TaskStatus::Done,
        TaskStatus::Accepted,
        TaskStatus::Cancelled,
    ] {
        let tasks = [task("T-0001", status, Some(("/w1", "main")))];
        assert_eq!(opens(&tasks, Some("main"), here), None, "{status:?}");
    }
    let claimed = [task(
        "T-0001",
        TaskStatus::InProgress,
        Some(("/w1", "main")),
    )];
    assert_eq!(opens(&claimed, Some("main"), here).unwrap().id, "T-0001");
    // M: the branch ignored.
    assert_eq!(opens(&claimed, Some("other"), here), None);
    // Detached: nothing opens.
    assert_eq!(opens(&claimed, None, here), None);
    // Another worktree, as the caller compares it.
    assert_eq!(opens(&claimed, Some("main"), |w: &str| w == "/w2"), None);
    // In progress without a claim opens nothing.
    let unclaimed = [task("T-0001", TaskStatus::InProgress, None)];
    assert_eq!(opens(&unclaimed, Some("main"), |_: &str| true), None);
}

#[test]
fn opens_names_the_lowest_number_by_value() {
    let tasks = [
        task("T-10000", TaskStatus::InProgress, Some(("/w1", "main"))),
        task("T-9999", TaskStatus::InProgress, Some(("/w1", "main"))),
        task("T-0007", TaskStatus::InProgress, Some(("/w2", "main"))),
        task("T-0001", TaskStatus::Ready, None),
    ];
    let here = |worktree: &str| worktree == "/w1";
    assert_eq!(opens(&tasks, Some("main"), here).unwrap().id, "T-9999");
}

// --------------------------------------------------------------------- fold

#[test]
fn fold_drops_dots_and_folds_parents_lexically() {
    for (path, want) in [
        ("src/../src/a.rs", "src/a.rs"),
        ("./src/./a.rs", "src/a.rs"),
        ("docs/../src/x/../y.rs", "src/y.rs"),
        ("/r/src/../src/a.rs", "/r/src/a.rs"),
        ("/..", "/"),
        ("/a/../..", "/"),
        ("../x", "../x"),
        ("a/../../x", "../x"),
    ] {
        assert_eq!(fold(Path::new(path)), PathBuf::from(want), "{path}");
    }
}
