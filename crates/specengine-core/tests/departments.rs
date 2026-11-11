//! docs/features/departments.md (RM-0096, v5 step 4, ADR-0076) "Acceptance
//! criteria", the pure half: core's `card_from_toml` and
//! `WorkspaceConfig::from_toml` over a flow's `title`, a `[[module]]` with
//! `kinds` (a direction) and each workspace's `disciplines`, and the gamedev
//! preset as committed. Every expected value is worked out from the spec,
//! never read back from a run.
//!
//! - DP-1: `title` served after `name`, the name when absent, `plain` for
//!   `PLAIN_FLOW`; the 64-byte bound counted in bytes; each error of `[card]`
//!   at its value's line, the two hints between `title` and `titles`; a
//!   flow's unknown key lists eight keys, `title` second. M: `title`
//!   serialized after `disciplines`; the `titles` hint dropped; the bound
//!   counted in characters.
//! - DP-2: a direction served `root: None, kinds: Some(..)` in written
//!   order, a rooted module `kinds: None`; each `[[module]]` problem one note
//!   at its line ending `; no module is served`, every module dropped; 32
//!   kinds read, 33 refused; a workspace naming a direction served. M: both
//!   keys served; a repeated kind kept; "required to hold `root`" kept.
//! - DP-3: the five rows of the `disciplines` table on flows `a`, `b`, `c`;
//!   a `[card]` that does not read gives `[]` with today's notes; the
//!   implicit workspace serves every flow's. M: a discipline kept when any of
//!   the department's flows lists it; config order instead of the
//!   department's.
//! - DP-5: the preset's header sentence naming `title`, `return` and the
//!   upgrade rule. M: the sentence dropped.
//! - DP-7: the preset read with the TOML reader: five flows titled as
//!   "Data", in order; no `module` key; the commented direction's four
//!   `# ` lines verbatim. M: a live `[[module]]`; `art`'s title dropped.
//!
//! The words are invented here or the fixtures' own (a print shop's `copy`
//! and `plates`, a lighthouse game's `scene`); the core names none
//! (ADR-0008).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

use std::fs;

use common::repository_root;
use specengine_core::card_toml::MAX_TITLE_BYTES;
use specengine_core::{
    CardConfig, PLAIN_FLOW, ProjectConfig, WorkspaceConfig, WorkspaceModule, WorkspaceSource,
    card_from_toml,
};

/// How every message names the config.
const CONFIG: &str = "specengine.toml";

/// The suffix of a malformed `[[module]]` note.
const NO_MODULE_SERVED: &str = "; no module is served";

/// A flow's keys, in the order a message lists them and `list --json`
/// serves them (set 88 less `return`'s place: "Data").
const FLOW_KEYS_LISTED: &str =
    "`name`, `title`, `disciplines`, `columns`, `titles`, `accept`, `return`, `moves`";

/// Set 88 of `fixtures/daemon-keys.json`, `CardFlow`, in order.
const CARD_FLOW_KEYS: [&str; 7] = [
    "name",
    "title",
    "disciplines",
    "columns",
    "moves",
    "accept",
    "return",
];

/// A print shop's two flows of "Data": `copy` titled, `plates` not; the
/// title on line 6.
const PRESS: &str = "[project]\nslug = \"press\"\n\n[[card.flow]]\nname = \"copy\"\ntitle = \
                     \"Copy desk\"\ndisciplines = [\"copy\"]\ncolumns = [\"draft\", \"edit\", \
                     \"proof\", \"filed\"]\naccept = { from = \"proof\", to = \"filed\" \
                     }\n\n[[card.flow]]\nname = \"plates\"\ndisciplines = [\"plates\"]\ncolumns \
                     = [\"queued\", \"blocked\", \"checked\", \"struck\"]\naccept = { from = \
                     \"checked\", to = \"struck\" }\n";

/// The line of [`PRESS`]'s `title`.
const TITLE_LINE: &str = "title = \"Copy desk\"";

/// The 1-based line of the first line of `text` that is exactly `line`.
fn line_of(text: &str, line: &str) -> usize {
    1 + text
        .lines()
        .position(|written| written == line)
        .unwrap_or_else(|| panic!("no line {line:?} in\n{text}"))
}

/// The 1-based line of the first line of `text` holding `needle`.
fn line_holding(text: &str, needle: &str, nth: usize) -> usize {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .nth(nth - 1)
        .map(|(index, _)| index + 1)
        .unwrap_or_else(|| panic!("no line {nth} holding {needle:?} in\n{text}"))
}

/// `<config>:<line>: <message>`.
fn note(line: usize, message: &str) -> String {
    format!("{CONFIG}:{line}: {message}")
}

/// [`PRESS`] with its title line replaced by `line`: the text and the line
/// it stands on.
fn press_with(line: &str) -> (String, usize) {
    let at = line_of(PRESS, TITLE_LINE);
    (PRESS.replacen(TITLE_LINE, line, 1), at)
}

/// The press flows read, or the error in full.
fn press_flows(text: &str) -> CardConfig {
    card_from_toml(text).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)))
}

// ---------------------------------------------------------------- DP-1

/// DP-1: `copy`'s `title` is served, untitled `plates` serves its name and
/// `PLAIN_FLOW` serves `plain`; a flow serializes `title` right after `name`,
/// set 88's order. M: `title` serialized after `disciplines`.
#[test]
fn dp1_a_flow_serves_its_title_or_its_name() {
    let cards = press_flows(PRESS);
    let titles: Vec<(&str, &str)> = cards
        .flows
        .iter()
        .map(|flow| (flow.name.as_str(), flow.title.as_str()))
        .collect();
    assert_eq!(titles, [("copy", "Copy desk"), ("plates", "plates")]);

    let plain = press_flows(PLAIN_FLOW);
    assert_eq!(plain.flows.len(), 1);
    assert_eq!(
        (plain.flows[0].name.as_str(), plain.flows[0].title.as_str()),
        ("plain", "plain"),
        "PLAIN_FLOW is unchanged and titled by its name"
    );
    assert!(
        !PLAIN_FLOW.contains("\ntitle ="),
        "PLAIN_FLOW is unchanged: no `title` line"
    );
    let none = press_flows("[project]\nslug = \"empty\"\n");
    assert_eq!(none.flows, plain.flows, "no `[card]` is PLAIN_FLOW");

    // The serialized order: a serde map's keys come back sorted, so the
    // order is read off the text.
    let text = serde_json::to_string(&cards.flows[0]).expect("a flow serializes");
    let mut order: Vec<(usize, &str)> = CARD_FLOW_KEYS
        .iter()
        .map(|key| {
            let found = text
                .find(&format!("\"{key}\":"))
                .unwrap_or_else(|| panic!("`{key}` in {text}"));
            (found, *key)
        })
        .collect();
    order.sort();
    assert_eq!(
        order.iter().map(|(_, key)| *key).collect::<Vec<_>>(),
        CARD_FLOW_KEYS,
        "M: `title` serialized after `disciplines`: {text}"
    );
    assert!(
        text.starts_with("{\"name\":\"copy\",\"title\":\"Copy desk\",\"disciplines\":"),
        "{text}"
    );
}

/// DP-1: the 64-byte bound counts bytes, in any script: `\u{e9}` 32 times
/// (64 bytes, 32 characters) reads; one more ASCII letter, 65 bytes in 33
/// characters, is refused at its line. M: the bound counted in characters.
#[test]
fn dp1_the_title_bound_counts_bytes() {
    let full = "\u{e9}".repeat(32);
    assert_eq!((full.len(), full.chars().count()), (MAX_TITLE_BYTES, 32));
    let (text, _) = press_with(&format!("title = \"{full}\""));
    assert_eq!(press_flows(&text).flows[0].title, full);

    let over = format!("{full}a");
    assert_eq!((over.len(), over.chars().count()), (65, 33));
    let (text, line) = press_with(&format!("title = \"{over}\""));
    let error = card_from_toml(&text).expect_err("65 bytes");
    assert_eq!(
        error.at(CONFIG),
        note(line, "`title` is 65 bytes long; a title has 1 to 64"),
        "M: the bound counted in characters"
    );

    let (text, line) = press_with("title = \"\"");
    assert_eq!(
        card_from_toml(&text).expect_err("empty").at(CONFIG),
        note(line, "`title` is 0 bytes long; a title has 1 to 64")
    );
    // Any script, an inner space, a single byte: data.
    for title in ["C", "Copy desk", "\u{0421}\u{0434}\u{0430}\u{0447}\u{0430}"] {
        let (text, _) = press_with(&format!("title = \"{title}\""));
        assert_eq!(press_flows(&text).flows[0].title, title);
    }
}

/// DP-1: each `[card]` error of "Data" stands at its value's line, word for
/// word, `spec card`'s `<config>:<line>: <message>`; the two hints name the
/// other key. M: the `titles` hint dropped.
#[test]
fn dp1_each_title_error_stands_at_its_line() {
    let long = format!("title = \"{}\"", "t".repeat(65));
    for (line, message) in [
        (
            "title = { draft = \"Draft\" }",
            "`title` is a string, not a table: column titles go in `titles`",
        ),
        ("title = 3", "`title` is a string, not an integer"),
        ("title = 1.5", "`title` is a string, not a float"),
        ("title = true", "`title` is a string, not a boolean"),
        ("title = 2026-11-11", "`title` is a string, not a datetime"),
        (
            "title = [\"Copy desk\"]",
            "`title` is a string, not an array",
        ),
        (
            long.as_str(),
            "`title` is 65 bytes long; a title has 1 to 64",
        ),
        (
            "title = \"Copy\\tdesk\"",
            "`title` holds a control character",
        ),
        (
            "title = \"Copy\\ndesk\"",
            "`title` holds a control character",
        ),
        (
            "titles = \"Copy desk\"",
            "`titles` is a table, not a string: a flow's own title is `title`",
        ),
        ("titles = 3", "`titles` is a table, not an integer"),
        ("titles = [\"Draft\"]", "`titles` is a table, not an array"),
    ] {
        let (text, at) = press_with(line);
        let error = card_from_toml(&text).expect_err(line);
        assert_eq!(error.line, Some(at), "{line}: {error:?}");
        assert_eq!(error.at(CONFIG), note(at, message), "{line}");
        // `spec workspace` reads apart and is not refused (`ProjectConfig`
        // only name-checks `card`).
        ProjectConfig::from_toml(&text)
            .unwrap_or_else(|error| panic!("{line}: `[card]` is only name-checked: {error:?}"));
    }
    // A table under `title` written as its own header: the same hint, at
    // the header's line.
    let text = format!("{PRESS}\n[card.flow.title]\ndraft = \"Draft\"\n");
    let text = text.replacen(&format!("{TITLE_LINE}\n"), "", 1);
    let error = card_from_toml(&text).expect_err("a table under `title`");
    assert!(
        error.message.ends_with(": column titles go in `titles`"),
        "{error:?}"
    );
}

/// DP-1: a flow's unknown key is the TOML reader's message at the key's line,
/// listing eight keys, `title` second.
#[test]
fn dp1_an_unknown_flow_key_lists_eight_keys() {
    let (text, at) = press_with("colour = \"red\"");
    let error = card_from_toml(&text).expect_err("colour");
    assert_eq!(
        error.at(CONFIG),
        note(
            at,
            &format!("unknown field `colour`, expected one of {FLOW_KEYS_LISTED}")
        )
    );
    assert_eq!(FLOW_KEYS_LISTED.matches('`').count(), 16, "eight keys");
}

// ---------------------------------------------------------------- DP-2

/// A project, a desk workspace naming no module, then `tables`.
fn with_modules(tables: &str) -> String {
    format!(
        "[project]\nslug = \"lamp\"\n\n[[workspace]]\nname = \"desk\"\nsections = \
         [\"tree\"]\n\n{tables}"
    )
}

/// One module as `name | title | root | kinds`, `-` for none.
fn module_row(module: &WorkspaceModule) -> String {
    format!(
        "{} | {} | {} | {}",
        module.name,
        module.title,
        module.root.as_deref().unwrap_or("-"),
        module
            .kinds
            .as_ref()
            .map_or_else(|| "-".to_owned(), |kinds| kinds.join(", "))
    )
}

fn module_rows(config: &WorkspaceConfig) -> Vec<String> {
    config.modules.iter().map(module_row).collect()
}

/// A rooted module, so a bad direction is seen to drop it too.
const ROOTED: &str = "[[module]]\nname = \"shore\"\ntitle = \"The shore\"\nroot = \
                      \"CHP-SHORE\"\n\n";

/// DP-2: `kinds` serves `root: None` and the kinds in written order, a rooted
/// module `kinds: None`; a workspace's `module` may name the direction.
#[test]
fn dp2_a_direction_is_served_beside_a_rooted_module() {
    let text = with_modules(&format!(
        "{ROOTED}[[module]]\nname = \"scenes\"\ntitle = \"Scenes\"\nkinds = [\"scene\", \
         \"chapter\"]\n"
    ))
    .replacen(
        "sections = [\"tree\"]\n",
        "sections = [\"tree\"]\nmodule = \"scenes\"\n",
        1,
    );
    let config = WorkspaceConfig::from_toml(&text);
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));
    assert_eq!(
        config.modules,
        [
            WorkspaceModule {
                name: "shore".to_owned(),
                title: "The shore".to_owned(),
                root: Some("CHP-SHORE".to_owned()),
                kinds: None,
            },
            WorkspaceModule {
                name: "scenes".to_owned(),
                title: "Scenes".to_owned(),
                root: None,
                kinds: Some(vec!["scene".to_owned(), "chapter".to_owned()]),
            },
        ]
    );
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(
        config.workspaces[0].module.as_deref(),
        Some("scenes"),
        "a workspace's module may name a direction (A7)"
    );
    ProjectConfig::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)));

    // Untitled: the name, as a rooted module's.
    let text = with_modules("[[module]]\nname = \"scenes\"\nkinds = [\"scene\"]\n");
    assert_eq!(
        module_rows(&WorkspaceConfig::from_toml(&text)),
        ["scenes | scenes | - | scene"]
    );
    // Kinds are data, matched exactly, never checked against `[ids]`: any
    // script, inner spaces, upper case.
    let odd = "Big \u{0441}\u{0446}\u{0435}\u{043d}\u{0430}";
    let text = with_modules(&format!(
        "[[module]]\nname = \"odd\"\nkinds = [\"{odd}\", \"Scene\", \"scene\"]\n"
    ));
    let config = WorkspaceConfig::from_toml(&text);
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));
    assert_eq!(
        config.modules[0].kinds,
        Some(vec![odd.to_owned(), "Scene".to_owned(), "scene".to_owned()])
    );
}

/// A bad `[[module]]` case: its label, the table, the line's needle and which
/// of its lines, the whole message when "Data" fixes it.
type ModuleCase = (&'static str, String, (&'static str, usize), Option<String>);

/// DP-2: each `[[module]]` problem of "Data": one note, the first by line, at
/// its line, `; no module is served`, every module dropped, the workspaces
/// read apart, `ProjectConfig` accepting the file. M: both keys served; a
/// repeated kind kept; "required to hold `root`" kept.
#[test]
fn dp2_each_module_problem_drops_every_module_with_one_note() {
    let many = |count: usize| -> String {
        (0..count)
            .map(|index| format!("  \"k{index}\",\n"))
            .collect::<String>()
    };
    let long = "k".repeat(65);
    // (label, the bad table, the line's needle and which of its lines, the
    // whole message when "Data" fixes it).
    let cases: Vec<ModuleCase> = vec![
        (
            "both keys, root first",
            "[[module]]\nname = \"scenes\"\nroot = \"CHP-SHORE\"\nkinds = [\"scene\"]\n".to_owned(),
            ("kinds = ", 1),
            Some(
                "`[[module]]` `scenes` holds both `root` and `kinds`: a module holds one"
                    .to_owned(),
            ),
        ),
        (
            "both keys, kinds first",
            "[[module]]\nname = \"scenes\"\nkinds = [\"scene\"]\nroot = \"CHP-SHORE\"\n".to_owned(),
            ("kinds = ", 1),
            Some(
                "`[[module]]` `scenes` holds both `root` and `kinds`: a module holds one"
                    .to_owned(),
            ),
        ),
        (
            "neither key",
            "[[module]]\nname = \"scenes\"\ntitle = \"Scenes\"\n".to_owned(),
            ("[[module]]", 2),
            Some("`[[module]]` is required to hold `root` or `kinds`".to_owned()),
        ),
        (
            "no kind",
            "[[module]]\nname = \"scenes\"\nkinds = []\n".to_owned(),
            ("kinds = []", 1),
            Some("`kinds` lists no kind; a direction lists 1 to 32".to_owned()),
        ),
        (
            "33 kinds",
            format!("[[module]]\nname = \"scenes\"\nkinds = [\n{}]\n", many(33)),
            ("\"k32\"", 1),
            Some("a list holds at most 32 entries".to_owned()),
        ),
        (
            "a kind of 65 bytes",
            format!("[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \"{long}\",\n]\n"),
            ("kkkkkkkkkk", 1),
            Some(format!(
                "`kinds`: `{long}` is 65 bytes long; a kind has 1 to 64"
            )),
        ),
        (
            "an empty kind",
            "[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \"\",\n]\n".to_owned(),
            ("\"\",", 1),
            Some("`kinds`: `` is 0 bytes long; a kind has 1 to 64".to_owned()),
        ),
        (
            "a control character",
            "[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \"sc\\tene\",\n]\n"
                .to_owned(),
            ("sc\\tene", 1),
            Some("`kinds`: a kind holds a control character".to_owned()),
        ),
        (
            "a space at the start",
            "[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \" chapter\",\n]\n"
                .to_owned(),
            ("\" chapter\"", 1),
            Some("`kinds`: ` chapter` has a space at an end".to_owned()),
        ),
        (
            "a space at the end",
            "[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \"chapter \",\n]\n"
                .to_owned(),
            ("\"chapter \"", 1),
            Some("`kinds`: `chapter ` has a space at an end".to_owned()),
        ),
        (
            "a kind twice",
            "[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \"chapter\",\n  \
             \"scene\",\n]\n"
                .to_owned(),
            ("\"scene\",", 2),
            Some("`kinds`: `scene` is listed twice".to_owned()),
        ),
        (
            "a string, not a list",
            "[[module]]\nname = \"scenes\"\nkinds = \"scene\"\n".to_owned(),
            ("kinds = ", 1),
            None,
        ),
        (
            "a number in the list",
            "[[module]]\nname = \"scenes\"\nkinds = [\"scene\", 3]\n".to_owned(),
            ("kinds = ", 1),
            None,
        ),
    ];
    for (label, table, (needle, nth), whole) in cases {
        let text = with_modules(&format!("{ROOTED}{table}"));
        let config = WorkspaceConfig::from_toml(&text);
        assert_eq!(config.source, WorkspaceSource::Config, "{label}");
        assert_eq!(config.workspaces.len(), 1, "{label}: the workspaces apart");
        assert_eq!(config.workspaces[0].name, "desk", "{label}");
        assert!(
            config.modules.is_empty(),
            "{label}: every module dropped, the good one too: {:?}",
            module_rows(&config)
        );
        let all = config.notes_at(CONFIG);
        assert_eq!(all.len(), 1, "{label}: one note: {all:?}");
        let line = line_holding(&text, needle, nth);
        match whole {
            Some(message) => assert_eq!(
                all[0],
                note(line, &format!("{message}{NO_MODULE_SERVED}")),
                "{label}"
            ),
            None => assert!(
                all[0].starts_with(&format!("{CONFIG}:{line}: "))
                    && all[0].ends_with(NO_MODULE_SERVED),
                "{label}: the TOML reader's message at line {line}: {all:?}"
            ),
        }
        ProjectConfig::from_toml(&text)
            .unwrap_or_else(|error| panic!("{label}: nothing is refused: {error:?}"));
    }
}

/// DP-2, "Rules and edge cases": a workspace naming a module of a malformed
/// `[[module]]` gets today's "no `[[module]]` is named" note, before the
/// module's by line; of two problems the first by line is the one noted.
#[test]
fn dp2_a_dropped_direction_leaves_its_workspace_without_a_module() {
    let text = with_modules(
        "[[module]]\nname = \"scenes\"\nkinds = [\n  \"scene\",\n  \"scene\",\n]\nroot = \
         \"CHP-SHORE\"\n",
    )
    .replacen(
        "sections = [\"tree\"]\n",
        "sections = [\"tree\"]\nmodule = \"scenes\"\n",
        1,
    );
    let config = WorkspaceConfig::from_toml(&text);
    assert!(config.modules.is_empty());
    assert_eq!(config.workspaces[0].module, None);
    let kinds = line_holding(&text, "kinds = ", 1);
    assert_eq!(
        config.notes_at(CONFIG),
        [
            note(
                line_holding(&text, "module = \"scenes\"", 1),
                "workspace `desk`: no `[[module]]` is named `scenes`: no default module"
            ),
            note(
                kinds,
                &format!(
                    "`[[module]]` `scenes` holds both `root` and `kinds`: a module holds \
                     one{NO_MODULE_SERVED}"
                )
            ),
        ],
        "both keys at `kinds`' line come before the repeat two lines below it"
    );
}

/// DP-2: 32 kinds are served, 33 are refused.
#[test]
fn dp2_thirty_two_kinds_read_and_thirty_three_do_not() {
    let list = |count: usize| -> String {
        (0..count)
            .map(|index| format!("\"k{index}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let text = with_modules(&format!(
        "[[module]]\nname = \"scenes\"\nkinds = [{}]\n",
        list(32)
    ));
    let config = WorkspaceConfig::from_toml(&text);
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));
    let kinds = config.modules[0].kinds.clone().expect("a direction");
    assert_eq!(kinds.len(), 32);
    assert_eq!(kinds[31], "k31", "written order");

    let text = with_modules(&format!(
        "[[module]]\nname = \"scenes\"\nkinds = [{}]\n",
        list(33)
    ));
    let config = WorkspaceConfig::from_toml(&text);
    assert!(config.modules.is_empty());
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            line_holding(&text, "kinds = ", 1),
            &format!("a list holds at most 32 entries{NO_MODULE_SERVED}")
        )]
    );
    // A kind of exactly 64 bytes is one.
    let text = with_modules(&format!(
        "[[module]]\nname = \"scenes\"\nkinds = [\"{}\"]\n",
        "\u{e9}".repeat(32)
    ));
    assert!(
        WorkspaceConfig::from_toml(&text).notes.is_empty(),
        "64 bytes"
    );
}

// ---------------------------------------------------------------- DP-3

/// "Data"'s `disciplines` rule: flows `a` `["x", "y"]`, `b` `["y", "z"]`, `c`
/// none, then `workspaces`.
fn lens(workspaces: &str) -> String {
    format!(
        "[project]\nslug = \"lens\"\n\n[[card.flow]]\nname = \"a\"\ndisciplines = [\"x\", \
         \"y\"]\ncolumns = [\"todo\", \"check\", \"done\"]\naccept = {{ from = \"check\", to = \
         \"done\" }}\n\n[[card.flow]]\nname = \"b\"\ndisciplines = [\"y\", \"z\"]\ncolumns = \
         [\"todo\", \"check\", \"done\"]\naccept = {{ from = \"check\", to = \"done\" \
         }}\n\n[[card.flow]]\nname = \"c\"\ncolumns = [\"todo\", \"check\", \"done\"]\naccept = \
         {{ from = \"check\", to = \"done\" }}\n{workspaces}"
    )
}

/// A `[[workspace]]` named `name` with `flows` written as given (`None`: no
/// key).
fn department(name: &str, flows: Option<&str>) -> String {
    let flows = flows.map_or_else(String::new, |flows| format!("flows = {flows}\n"));
    format!("\n[[workspace]]\nname = \"{name}\"\n{flows}")
}

/// The five rows of "Data"'s table: `(workspace, flows, disciplines)`.
const ROWS: [(&str, Option<&str>, &[&str]); 5] = [
    ("only-b", Some("[\"b\"]"), &["z"]),
    ("a-then-b", Some("[\"a\", \"b\"]"), &["x", "y", "z"]),
    ("b-then-a", Some("[\"b\", \"a\"]"), &["y", "z", "x"]),
    ("every", None, &["x", "y", "z"]),
    ("only-c", Some("[\"c\"]"), &[]),
];

fn disciplines(config: &WorkspaceConfig) -> Vec<(String, Vec<String>)> {
    config
        .workspaces
        .iter()
        .map(|workspace| (workspace.name.clone(), workspace.disciplines.clone()))
        .collect()
}

/// DP-3: the five rows, exactly, in one config and each alone. M: a
/// discipline kept when any of the department's flows lists it (`["b"]`
/// gives `["y", "z"]`); config order instead of the department's (`["b",
/// "a"]` gives `["x", "y", "z"]`).
#[test]
fn dp3_the_disciplines_rule_takes_the_five_rows() {
    let all: String = ROWS
        .iter()
        .map(|(name, flows, _)| department(name, *flows))
        .collect();
    let config = WorkspaceConfig::from_toml(&lens(&all));
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));
    let want: Vec<(String, Vec<String>)> = ROWS
        .iter()
        .map(|(name, _, disciplines)| {
            (
                (*name).to_owned(),
                disciplines.iter().map(|d| (*d).to_owned()).collect(),
            )
        })
        .collect();
    assert_eq!(disciplines(&config), want);
    assert_eq!(config.workspaces[3].flows, None, "`every` serves null");
    for (name, flows, want) in ROWS {
        let config = WorkspaceConfig::from_toml(&lens(&department(name, flows)));
        assert_eq!(config.workspaces[0].disciplines, want, "{name} alone");
    }
    // The rule is `CardConfig::flow_for`'s: each kept discipline's first
    // flow, in config order, is one of the department's.
    let cards = card_from_toml(&lens("")).expect("the lens flows read");
    for (name, flows, want) in ROWS {
        for discipline in want {
            let first = cards
                .flow_for(Some(discipline))
                .expect("a flow takes it")
                .name
                .clone();
            let listed = flows.is_none_or(|flows| flows.contains(&format!("\"{first}\"")));
            assert!(listed, "{name}: `{discipline}` goes to `{first}`");
        }
    }
}

/// DP-3, "Rules and edge cases": a flow listed by a department but left out
/// (no such flow) leaves the department's served `flows`; none left is
/// `null`, every flow, so every flow's disciplines.
#[test]
fn dp3_disciplines_follow_the_served_flows() {
    let text = lens(&format!(
        "{}{}",
        department("half", Some("[\"nope\", \"b\"]")),
        department("none-left", Some("[\"nope\"]"))
    ));
    let config = WorkspaceConfig::from_toml(&text);
    assert_eq!(config.workspaces[0].flows, Some(vec!["b".to_owned()]));
    assert_eq!(config.workspaces[0].disciplines, ["z"]);
    assert_eq!(config.workspaces[1].flows, None);
    assert_eq!(config.workspaces[1].disciplines, ["x", "y", "z"]);
    assert_eq!(config.notes.len(), 2, "{:?}", config.notes_at(CONFIG));
}

/// DP-3: a `[card]` that does not read gives `[]` everywhere, and the notes
/// are today's: one, where `flows` are judged, none where they are not; the
/// implicit workspace serves every flow's disciplines, `[]` when `[card]`
/// does not read, with no note.
#[test]
fn dp3_an_unreadable_card_gives_no_discipline_and_no_new_note() {
    let broken =
        |text: String| text.replacen("accept = { from = \"check\", to = \"done\" }\n", "", 1);
    let all: String = ROWS
        .iter()
        .map(|(name, flows, _)| department(name, *flows))
        .collect();
    let text = broken(lens(&all));
    let error = card_from_toml(&text).expect_err("`a` has no accept");
    let config = WorkspaceConfig::from_toml(&text);
    assert_eq!(
        config.notes_at(CONFIG),
        [format!(
            "{}; the workspaces' flows are served unjudged",
            error.at(CONFIG)
        )],
        "today's one note"
    );
    for workspace in &config.workspaces {
        assert!(
            workspace.disciplines.is_empty(),
            "{}: {:?}",
            workspace.name,
            workspace.disciplines
        );
    }
    assert_eq!(
        config.workspaces[1].flows,
        Some(vec!["a".to_owned(), "b".to_owned()]),
        "flows served as written"
    );

    // No workspace judges `flows`: no note at all, as today.
    let text = broken(lens(&department("every", None)));
    let config = WorkspaceConfig::from_toml(&text);
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));
    assert!(config.workspaces[0].disciplines.is_empty());

    // The implicit workspace.
    let config = WorkspaceConfig::from_toml(&lens(""));
    assert_eq!(config.source, WorkspaceSource::Implicit);
    assert_eq!(config.workspaces[0].flows, None);
    assert_eq!(config.workspaces[0].disciplines, ["x", "y", "z"]);
    assert!(config.notes.is_empty());
    let config = WorkspaceConfig::from_toml(&broken(lens("")));
    assert_eq!(config.source, WorkspaceSource::Implicit);
    assert!(config.workspaces[0].disciplines.is_empty());
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));
    // No `[card]`: `PLAIN_FLOW`, which lists none.
    let config = WorkspaceConfig::from_toml("[project]\nslug = \"bare\"\n");
    assert!(config.workspaces[0].disciplines.is_empty());
    // A malformed `[[workspace]]` serves the implicit one, which still
    // serves every flow's disciplines.
    let config = WorkspaceConfig::from_toml(&lens(&department("Bad name", None)));
    assert_eq!(config.source, WorkspaceSource::Implicit);
    assert_eq!(config.workspaces[0].disciplines, ["x", "y", "z"]);
}

// ------------------------------------------------------------ the preset

/// `scripts/presets/gamedev.toml` as committed.
fn preset() -> String {
    let path = repository_root().join("scripts/presets/gamedev.toml");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// "The preset": each flow's name and title, in order (W8: `development` is
/// "Programming"); docs/features/department-split.md "Acceptance criteria",
/// S9: `sound` sixth, "Sound".
const PRESET_TITLES: [(&str, &str); 6] = [
    ("narrative", "Narrative"),
    ("development", "Programming"),
    ("art", "Art"),
    ("level-design", "Level design"),
    ("production", "Production"),
    ("sound", "Sound"),
];

/// The commented direction of "The preset", its eight lines in order: four
/// of prose, then the four `# ` lines of the table.
const COMMENTED_DIRECTION: [&str; 8] = [
    "# A direction: a module chosen by node kind, with no root. The Spec tree lists",
    "# its kinds' nodes and a document's Inside groups them under its title; Flow,",
    "# which draws from a root, leaves it out. Use your project's own kinds, and add",
    "# it only once every terminal, plugin and hub runs a build that reads `kinds`.",
    "# [[module]]",
    "# name = \"scenes\"",
    "# title = \"Scenes\"",
    "# kinds = [\"scene\"]",
];

/// DP-7: the preset read with the TOML reader: five flows titled as "Data",
/// in order, each `title` the line after `name`; no `module` key; the
/// commented direction verbatim (its four `# ` table lines and the prose
/// above them) and in a row, after the workspaces. The engine reads
/// the same titles. M: a live `[[module]]`; `art`'s title dropped.
#[test]
fn dp7_the_preset_titles_its_flows_and_ships_no_direction() {
    let text = preset();
    let table: toml::Table = text.parse().expect("the preset is TOML");
    let flows = table["card"]["flow"].as_array().expect("[[card.flow]]");
    let read: Vec<(&str, Option<&str>)> = flows
        .iter()
        .map(|flow| {
            (
                flow["name"].as_str().expect("a name"),
                flow.get("title").and_then(toml::Value::as_str),
            )
        })
        .collect();
    let want: Vec<(&str, Option<&str>)> = PRESET_TITLES
        .iter()
        .map(|(name, title)| (*name, Some(*title)))
        .collect();
    assert_eq!(read, want, "M: `art`'s title dropped");
    assert!(
        table.get("module").is_none(),
        "M: a live `[[module]]` in the preset"
    );
    let lines: Vec<&str> = text.lines().collect();
    for (name, title) in PRESET_TITLES {
        let at = lines
            .iter()
            .position(|line| *line == format!("name = \"{name}\""))
            .unwrap_or_else(|| panic!("{name}'s line"));
        assert_eq!(
            lines.get(at + 1).copied(),
            Some(format!("title = \"{title}\"").as_str()),
            "`title` after `name`"
        );
    }
    let start = lines
        .iter()
        .position(|line| *line == COMMENTED_DIRECTION[0])
        .expect("the commented direction");
    assert_eq!(
        lines.get(start..start + COMMENTED_DIRECTION.len()),
        Some(&COMMENTED_DIRECTION[..]),
        "the commented direction verbatim, in a row"
    );
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.trim_start_matches("# ").starts_with("[[module]]"))
            .count(),
        1,
        "one table line, the commented one (the header names `[[module]]` in prose)"
    );
    let workspaces = lines
        .iter()
        .rposition(|line| *line == "[[workspace]]")
        .expect("a workspace");
    assert!(start > workspaces, "the example after the workspaces");

    let preset = format!("[project]\nslug = \"preset\"\n{text}");
    ProjectConfig::from_toml(&preset).unwrap_or_else(|error| panic!("{}", error.at("preset")));
    let cards = press_flows(&preset);
    let titles: Vec<(&str, &str)> = cards
        .flows
        .iter()
        .map(|flow| (flow.name.as_str(), flow.title.as_str()))
        .collect();
    assert_eq!(titles, PRESET_TITLES);
    let config = WorkspaceConfig::from_toml(&preset);
    assert!(config.modules.is_empty());
    assert!(config.notes.is_empty(), "{:?}", config.notes_at("preset"));
    // Each preset workspace's disciplines (DP-3's rule on the preset).
    let served: Vec<(&str, Vec<&str>)> = config
        .workspaces
        .iter()
        .map(|workspace| {
            (
                workspace.name.as_str(),
                workspace.disciplines.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        served,
        [
            ("writing", vec!["narrative"]),
            ("development", vec!["development"]),
            ("art", vec!["art", "level-design"]),
            // department-split S9: the `sound` workspace before
            // `production`, which serves every flow's, `sound`'s last.
            ("sound", vec!["sound"]),
            (
                "production",
                vec![
                    "narrative",
                    "development",
                    "art",
                    "level-design",
                    "production",
                    "sound"
                ]
            ),
        ]
    );
}

/// The sentences of the preset's header: its leading `# ` lines joined.
fn header_sentences(text: &str) -> Vec<String> {
    let joined: Vec<&str> = text
        .lines()
        .take_while(|line| line.starts_with('#'))
        .map(|line| line.trim_start_matches('#').trim())
        .collect();
    joined
        .join(" ")
        .split_inclusive(". ")
        .map(|sentence| sentence.trim().to_owned())
        .collect()
}

/// DP-5 (D11, D14): the preset's header holds one sentence naming `title`,
/// `return` and the rollout rule: every terminal, plugin and hub serving the
/// project runs a build that reads them, an older one refusing them. M: the
/// sentence dropped. Since docs/features/department-split.md (S9) the
/// sentence names a `[[card.split]]` row too and writes the keys bare, so
/// the words are matched without their backticks.
#[test]
fn dp5_the_preset_header_names_the_rollout_of_title_and_return() {
    let text = preset();
    let sentences = header_sentences(&text);
    let rule: Vec<&String> = sentences
        .iter()
        .filter(|sentence| {
            let bare = sentence.replace('`', "");
            bare.contains("flow's title and return")
                && sentence.contains("every terminal, plugin and hub")
                && sentence.contains("older")
        })
        .collect();
    assert_eq!(
        rule.len(),
        1,
        "one such sentence in the header: {sentences:#?}"
    );
}
