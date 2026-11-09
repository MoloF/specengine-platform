//! docs/features/workspaces.md (RM-0081, v4 step 1a) "Acceptance criteria",
//! the pure half: core's `WorkspaceConfig::from_toml` over the grammar of
//! "Data", the texts of "Notes, exactly" and the edges of "Rules and edge
//! cases". Every expected value is worked out from the spec, never read back
//! from a run.
//!
//! - The implicit workspace (WS-1): no table, `workspace = []`, spec-a; its
//!   eight sections are `SECTIONS` without `tasks`. M1: `tasks` kept in
//!   `IMPLICIT_SECTIONS`.
//! - card-press's two workspaces and two modules as written, every default of
//!   an absent key, `tasks` kept when a workspace lists it (D7).
//! - WS-4 (a) to (g) on the reader: each note's text and line, the rest
//!   served. M8: (e) dropping the modules. M9: (a) failing its workspace whole.
//! - A malformed table, each way: one note, its default served, the other
//!   table untouched; the limits at their cap and one past it.
//! - Notes sorted by line, ties in judging order; `notes_at` names the file as
//!   given.
//! - WS-7: the preset appended to a `[project]` reads whole (`ProjectConfig`,
//!   `WorkspaceConfig` with no note, `CardConfig`'s five flows), and
//!   `fixtures/studio-lamp` holds its bytes verbatim between its markers. M16:
//!   a title changed in the fixture's copy alone.
//! - WS-8, the core half: `ProjectConfig` name-checks both keys, a malformed
//!   table included (M7), and refuses `[[workspaces]]` at its line. M18: a key
//!   missing from `RawFile`.
//!
//! Two projects of different nature: `fixtures/card-press` (a print shop) and
//! `fixtures/studio-lamp` (a lighthouse game), and the game's brief
//! `fixtures/spec-a` with no table (ADR-0008).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

mod common;

#[path = "../../specengine-cli/tests/workspaces_common/mod.rs"]
mod workspaces_common;

use std::fs;

use common::{corpus_scheme, fixture, repository_root};
use specengine_core::card_toml::{MAX_NAME_BYTES, MAX_TITLE_BYTES, name_problem};
use specengine_core::workspace_toml::{
    IMPLICIT_NAME, IMPLICIT_SECTIONS, IMPLICIT_TITLE, MAX_LIST, MAX_MODULES, MAX_ROOT_BYTES,
    MAX_WORKSPACES, OVERVIEW, SECTIONS,
};
use specengine_core::{
    CardConfig, ProjectConfig, Workspace, WorkspaceConfig, WorkspaceModule, WorkspaceSource,
    card_from_toml, workspace_from_toml,
};
use workspaces_common::{
    CONFIG, changed, line_holding, misspelt, nth_line_holding, press_config, variants,
    without_tables,
};

/// The nine section IDs of "Data", in `sections.ts`'s order.
const NINE: [&str; 9] = [
    "inbox",
    "tasks",
    "board",
    "roadmap",
    "flow",
    "tree",
    "graph",
    "health",
    "questions",
];

/// The implicit workspace as a row of [`row`].
const IMPLICIT_ROW: &str = "all | All sections | landing overview | sections inbox, board, \
                            roadmap, flow, tree, graph, health, questions | flows (all) | module \
                            (none)";

/// The suffix of a malformed `[[workspace]]` note.
const IMPLICIT_SERVED: &str = "; the implicit workspace is served";
/// The suffix of a malformed `[[module]]` note.
const NO_MODULE_SERVED: &str = "; no module is served";

/// The spec's list of section IDs inside the unknown-section note.
const SECTION_LIST: &str = "(inbox, tasks, board, roadmap, flow, tree, graph, health, questions)";

/// A project with no table, so the tests below add their own.
const PROJECT: &str = "[project]\nslug = \"press\"\n";

fn read(text: &str) -> WorkspaceConfig {
    WorkspaceConfig::from_toml(text)
}

fn notes(text: &str) -> Vec<String> {
    read(text).notes_at(CONFIG)
}

/// One workspace as one line: `name | title | landing L | sections a, b |
/// flows f (or `(all)` for null) | module m (or `(none)`)`.
fn row(workspace: &Workspace) -> String {
    let flows = workspace
        .flows
        .as_ref()
        .map_or_else(|| "(all)".to_owned(), |flows| flows.join(", "));
    format!(
        "{} | {} | landing {} | sections {} | flows {flows} | module {}",
        workspace.name,
        workspace.title,
        workspace.landing,
        workspace.sections.join(", "),
        workspace.module.as_deref().unwrap_or("(none)")
    )
}

fn rows(config: &WorkspaceConfig) -> Vec<String> {
    config.workspaces.iter().map(row).collect()
}

/// One module as `name | title | root`.
fn module_rows(config: &WorkspaceConfig) -> Vec<String> {
    config
        .modules
        .iter()
        .map(|module: &WorkspaceModule| {
            format!("{} | {} | {}", module.name, module.title, module.root)
        })
        .collect()
}

/// card-press's two modules.
fn press_modules() -> Vec<String> {
    vec![
        "weekly | The weekly run | PRN-WEEKLY".to_owned(),
        "rulings | Style rulings | STY-0001".to_owned(),
    ]
}

/// card-press's two workspaces.
fn press_rows() -> Vec<String> {
    vec![
        "newsroom | Newsroom | landing board | sections board, tree, inbox | flows copy | module \
         weekly"
            .to_owned(),
        "pressroom | Pressroom | landing overview | sections board, flow, tree, health | flows \
         plates | module (none)"
            .to_owned(),
    ]
}

/// `<config>:<line>: <message>`.
fn note(line: usize, message: &str) -> String {
    format!("{CONFIG}:{line}: {message}")
}

/// Asserts `config` serves the implicit workspace alone.
fn assert_implicit(config: &WorkspaceConfig, context: &str) {
    assert_eq!(config.source, WorkspaceSource::Implicit, "{context}");
    assert_eq!(rows(config), [IMPLICIT_ROW], "{context}");
}

// ------------------------------------------------------------ the constants

/// "The reader": the constants, and the implicit eight as the nine without
/// `tasks`. M1: `tasks` kept in `IMPLICIT_SECTIONS`.
#[test]
fn ws1_the_constants_are_the_spec_s() {
    assert_eq!(SECTIONS, NINE);
    let without_tasks: Vec<&str> = SECTIONS
        .iter()
        .copied()
        .filter(|section| *section != "tasks")
        .collect();
    assert_eq!(IMPLICIT_SECTIONS.to_vec(), without_tasks);
    assert!(!IMPLICIT_SECTIONS.contains(&"tasks"));
    assert_eq!(OVERVIEW, "overview");
    assert!(!SECTIONS.contains(&OVERVIEW), "the Overview is no section");
    assert_eq!(IMPLICIT_NAME, "all");
    assert_eq!(IMPLICIT_TITLE, "All sections");
    assert_eq!(
        (MAX_WORKSPACES, MAX_MODULES, MAX_LIST, MAX_ROOT_BYTES),
        (32, 64, 32, 256)
    );
    assert_eq!((MAX_NAME_BYTES, MAX_TITLE_BYTES), (64, 64));
}

// ------------------------------------------------------- the implicit one

/// WS-1, D3: no `[[workspace]]` (no key, an empty array, spec-a, card-press
/// with both tables removed) serves the implicit workspace with no note, and
/// no `[[module]]` (or `module = []`) serves none. M1.
#[test]
fn ws1_no_table_is_the_implicit_workspace_with_no_note() {
    let spec_a = workspaces_common::fixture_config("spec-a");
    let press_bare = without_tables(&press_config());
    for (label, text) in [
        ("empty", String::new()),
        ("a project alone", PROJECT.to_owned()),
        (
            "empty arrays",
            format!("workspace = []\nmodule = []\n{PROJECT}"),
        ),
        ("spec-a", spec_a),
        ("card-press, both tables removed", press_bare),
    ] {
        let config = read(&text);
        assert_implicit(&config, label);
        assert!(config.modules.is_empty(), "{label}");
        assert!(config.notes.is_empty(), "{label}: {:?}", config.notes);
        let only = &config.workspaces[0];
        assert_eq!(
            only.sections,
            IMPLICIT_SECTIONS.map(str::to_owned),
            "{label}"
        );
        assert_eq!(only.flows, None, "{label}: every flow");
        assert_eq!(only.module, None, "{label}");
    }
}

/// D2, the reader never fails: a text the TOML reader refuses serves the
/// implicit workspace and no module, with one note (the developer's reading
/// of "Notes, exactly", its line that of the parse error).
#[test]
fn a_text_that_is_no_toml_serves_the_implicit_workspace_with_one_note() {
    let config = read("[[[ broken\n");
    assert_implicit(&config, "no TOML");
    assert!(config.modules.is_empty());
    let all = config.notes_at(CONFIG);
    assert_eq!(all.len(), 1, "{all:?}");
    assert!(all[0].starts_with(&format!("{CONFIG}:1: ")), "{all:?}");
}

/// D2: `workspace_from_toml` is `WorkspaceConfig::from_toml`, and a repeat
/// reads the same: one state, one result.
#[test]
fn the_reader_is_pure_and_repeats() {
    let text = press_config();
    let once = read(&text);
    assert_eq!(once, read(&text));
    assert_eq!(once, workspace_from_toml(&text));
    let noted = changed(&text, "name = \"newsroom\"", "name = \"News room\"");
    assert_eq!(read(&noted), read(&noted));
    assert_eq!(notes(&noted), notes(&noted));
}

// ------------------------------------------------------------ card-press

/// "Data": card-press's tables read as written, in config order, no note.
#[test]
fn the_press_tables_read_as_written() {
    let config = read(&press_config());
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(rows(&config), press_rows());
    assert_eq!(module_rows(&config), press_modules());
    assert!(config.notes.is_empty(), "{:?}", config.notes);
}

/// "Data", the "Absent" column: a title is the name, the sections the
/// implicit eight, the landing the Overview, the flows null, no module; a
/// module's title its name.
#[test]
fn every_absent_key_takes_its_default() {
    let text = format!(
        "{PROJECT}\n[[workspace]]\nname = \"desk\"\n\n[[module]]\nname = \"front\"\nroot = \
         \"SHT-FRONT\"\n"
    );
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(
        rows(&config),
        [
            "desk | desk | landing overview | sections inbox, board, roadmap, flow, tree, graph, \
          health, questions | flows (all) | module (none)"
        ]
    );
    assert_eq!(module_rows(&config), ["front | front | SHT-FRONT"]);
    assert!(config.notes.is_empty(), "{:?}", config.notes);
}

/// D7: the grammar still takes `tasks`, a landing on it included; an explicit
/// `overview` landing and a `module` naming a served module stand.
#[test]
fn a_workspace_may_keep_tasks_and_land_on_it() {
    let text = format!(
        "{PROJECT}\n[[workspace]]\nname = \"desk\"\nsections = [\"tasks\", \"inbox\"]\nlanding = \
         \"tasks\"\nmodule = \"front\"\n\n[[workspace]]\nname = \"floor\"\nsections = \
         [\"board\"]\nlanding = \"overview\"\n\n[[module]]\nname = \"front\"\nroot = \
         \"SHT-FRONT\"\n"
    );
    let config = read(&text);
    assert_eq!(
        rows(&config),
        [
            "desk | desk | landing tasks | sections tasks, inbox | flows (all) | module front",
            "floor | floor | landing overview | sections board | flows (all) | module (none)",
        ]
    );
    assert!(config.notes.is_empty(), "{:?}", config.notes);
}

// ------------------------------------------------- WS-4, value by value

/// The variant of [`variants`] labelled `(x)`.
fn variant(label: &str) -> String {
    variants(&press_config())
        .into_iter()
        .find(|(name, _)| name.starts_with(label))
        .unwrap_or_else(|| panic!("no variant {label}"))
        .1
}

/// WS-4 (a), W2: an unknown section is left out with its note; newsroom keeps
/// `board, tree, inbox` and everything else stands. M9: (a) failing its
/// workspace whole.
#[test]
fn ws4a_an_unknown_section_is_left_out_with_its_note() {
    let text = variant("(a)");
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(rows(&config), press_rows(), "newsroom keeps the rest");
    assert_eq!(module_rows(&config), press_modules());
    let line = line_holding(&text, "\"stage\"");
    assert_eq!(line, 39, "the sections line of card-press");
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            line,
            &format!("workspace `newsroom`: `stage` is no section {SECTION_LIST}: left out")
        )]
    );
    assert_eq!(config.notes[0].line.to_string(), "39");
    assert_eq!(
        config.notes[0].message,
        format!("workspace `newsroom`: `stage` is no section {SECTION_LIST}: left out")
    );
}

/// "Notes, exactly": a section listed twice keeps its first place; a
/// workspace whose every section is unknown keeps none (the table is well
/// formed: no implicit list), landing on the Overview.
#[test]
fn a_repeated_section_keeps_the_first_and_none_left_is_none() {
    let text = changed(
        &press_config(),
        "sections = [\"board\", \"tree\", \"inbox\"]",
        "sections = [\"tree\", \"board\", \"tree\", \"inbox\"]",
    );
    let config = read(&text);
    assert_eq!(
        row(&config.workspaces[0]),
        "newsroom | Newsroom | landing board | sections tree, board, inbox | flows copy | module \
         weekly"
    );
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            39,
            "workspace `newsroom`: section `tree` is listed twice: the second left out"
        )]
    );

    let text =
        format!("{PROJECT}\n[[workspace]]\nname = \"desk\"\nsections = [\"stage\", \"wings\"]\n");
    let config = read(&text);
    let line = line_holding(&text, "sections");
    assert_eq!(config.source, WorkspaceSource::Config, "no fallback");
    assert_eq!(
        rows(&config),
        ["desk | desk | landing overview | sections  | flows (all) | module (none)"]
    );
    assert_eq!(
        config.notes_at(CONFIG),
        [
            note(
                line,
                &format!("workspace `desk`: `stage` is no section {SECTION_LIST}: left out")
            ),
            note(
                line,
                &format!("workspace `desk`: `wings` is no section {SECTION_LIST}: left out")
            ),
        ]
    );
}

/// WS-4 (b): a flow no `[[card.flow]]` names is left out with its note.
#[test]
fn ws4b_an_unknown_flow_is_left_out_with_its_note() {
    let text = variant("(b)");
    let config = read(&text);
    assert_eq!(rows(&config), press_rows());
    assert_eq!(module_rows(&config), press_modules());
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            line_holding(&text, "\"inks\""),
            "workspace `newsroom`: no `[[card.flow]]` is named `inks`: left out"
        )]
    );
}

/// "Notes, exactly": a flow listed twice keeps its first place; every flow
/// unknown, or `flows = []`, is null (every flow), with a note only for the
/// unknown ones.
#[test]
fn a_repeated_flow_keeps_the_first_and_none_left_is_null() {
    let base = press_config();
    let text = changed(
        &base,
        "flows = [\"copy\"]\n",
        "flows = [\"copy\", \"copy\"]\n",
    );
    let config = read(&text);
    assert_eq!(rows(&config), press_rows());
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            41,
            "workspace `newsroom`: flow `copy` is listed twice: the second left out"
        )]
    );

    let text = changed(&base, "flows = [\"copy\"]\n", "flows = [\"inks\"]\n");
    let config = read(&text);
    assert_eq!(config.workspaces[0].flows, None, "every flow unknown: null");
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            41,
            "workspace `newsroom`: no `[[card.flow]]` is named `inks`: left out"
        )]
    );

    let text = changed(&base, "flows = [\"copy\"]\n", "flows = []\n");
    let config = read(&text);
    assert_eq!(config.workspaces[0].flows, None, "none given: null");
    assert!(config.notes.is_empty(), "{:?}", config.notes);
}

/// "The reader", `flows`: judged against `card_from_toml(text)`, so with no
/// `[card]` the default flow `plain` is a flow and `copy` none.
#[test]
fn flows_are_judged_against_the_card_flows_the_text_serves() {
    let text = format!(
        "{PROJECT}\n[[workspace]]\nname = \"desk\"\nflows = [\"plain\"]\n\n[[workspace]]\nname \
         = \"floor\"\nflows = [\"copy\"]\n"
    );
    let config = read(&text);
    assert_eq!(config.workspaces[0].flows, Some(vec!["plain".to_owned()]));
    assert_eq!(config.workspaces[1].flows, None);
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            line_holding(&text, "flows = [\"copy\"]"),
            "workspace `floor`: no `[[card.flow]]` is named `copy`: left out"
        )]
    );
}

/// WS-4 (g): with `[card]` unreadable, both lists are served as written, an
/// unknown name included, with one note for the document: the card's error
/// at its line, then `; the workspaces' flows are served unjudged`.
#[test]
fn ws4g_an_unreadable_card_serves_the_flows_as_written() {
    let text = variant("(g)");
    let card = card_from_toml(&text).expect_err("copy has no accept");
    let card_note = format!(
        "{}; the workspaces' flows are served unjudged",
        card.at(CONFIG)
    );
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(rows(&config), press_rows());
    assert_eq!(module_rows(&config), press_modules());
    assert_eq!(config.notes_at(CONFIG), std::slice::from_ref(&card_note));
    assert!(card.line.is_some(), "the card's error has a line: {card:?}");

    let unknown = changed(
        &text,
        "flows = [\"copy\"]\n",
        "flows = [\"copy\", \"inks\"]\n",
    );
    let config = read(&unknown);
    assert_eq!(
        config.workspaces[0].flows,
        Some(vec!["copy".to_owned(), "inks".to_owned()]),
        "served unjudged"
    );
    assert_eq!(config.notes_at(CONFIG), [card_note]);
}

/// WS-4 (c): a landing outside the kept sections lands on the Overview, with
/// its note; a landing on a section left out has both notes, in line order.
#[test]
fn ws4c_a_landing_outside_its_sections_lands_on_the_overview() {
    let text = variant("(c)");
    let config = read(&text);
    assert_eq!(
        rows(&config),
        press_rows(),
        "pressroom lands on the Overview"
    );
    assert_eq!(config.workspaces[1].landing, "overview");
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            line_holding(&text, "landing = \"inbox\""),
            "workspace `pressroom`: landing `inbox` is not one of its sections: it lands on the \
             Overview"
        )]
    );

    let text = format!(
        "{PROJECT}\n[[workspace]]\nname = \"desk\"\nsections = [\"board\", \"stage\"]\nlanding = \
         \"stage\"\n"
    );
    let config = read(&text);
    assert_eq!(
        rows(&config),
        ["desk | desk | landing overview | sections board | flows (all) | module (none)"]
    );
    assert_eq!(
        config.notes_at(CONFIG),
        [
            note(
                line_holding(&text, "sections"),
                &format!("workspace `desk`: `stage` is no section {SECTION_LIST}: left out")
            ),
            note(
                line_holding(&text, "landing"),
                "workspace `desk`: landing `stage` is not one of its sections: it lands on the \
                 Overview"
            ),
        ]
    );
}

/// WS-4 (d): a module no `[[module]]` serves is no default module.
#[test]
fn ws4d_an_unknown_module_is_no_default_module() {
    let text = variant("(d)");
    let config = read(&text);
    assert_eq!(rows(&config), press_rows());
    assert_eq!(
        config.notes_at(CONFIG),
        [note(
            line_holding(&text, "plates-room"),
            "workspace `pressroom`: no `[[module]]` is named `plates-room`: no default module"
        )]
    );
}

/// WS-4 (e), W1: a name off the grammar makes the table malformed: the
/// implicit workspace, one note of `name_problem`'s words, and both modules
/// still served (the tables are read apart). M8: (e) dropping the modules.
#[test]
fn ws4e_a_bad_name_serves_the_implicit_workspace_and_keeps_the_modules() {
    let text = variant("(e)");
    let config = read(&text);
    assert_implicit(&config, "(e)");
    assert_eq!(module_rows(&config), press_modules(), "M8");
    let problem = name_problem("News room").expect("a space is no name character");
    assert_eq!(
        config.notes_at(CONFIG),
        [note(37, &format!("{problem}{IMPLICIT_SERVED}"))]
    );
    assert_eq!(
        problem,
        "`News room` is not a name: lower-case ASCII letters, digits, `_` and `-`"
    );
}

/// WS-4 (f): a module without `root` makes `[[module]]` malformed: no
/// module, its note at the table's header, and newsroom's module note before
/// it (sorted by line, though modules are judged first); `source` stays
/// `config`.
#[test]
fn ws4f_a_module_without_root_serves_no_module() {
    let text = variant("(f)");
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert!(config.modules.is_empty());
    assert_eq!(
        row(&config.workspaces[0]),
        "newsroom | Newsroom | landing board | sections board, tree, inbox | flows copy | module \
         (none)"
    );
    let header = nth_line_holding(&text, "[[module]]", 2);
    assert_eq!(header, 55, "rulings' header");
    assert_eq!(
        config.notes_at(CONFIG),
        [
            note(
                42,
                "workspace `newsroom`: no `[[module]]` is named `weekly`: no default module"
            ),
            note(
                header,
                &format!("`[[module]]` is required to hold `root`{NO_MODULE_SERVED}")
            ),
        ]
    );
}

// ------------------------------------------------------- malformed tables

/// `n` workspaces `w0`..: a header and a name each.
fn workspace_tables(n: usize) -> String {
    (0..n)
        .map(|index| format!("\n[[workspace]]\nname = \"w{index}\"\n"))
        .collect()
}

/// `n` modules `m0`..: a header, a name and a root each.
fn module_tables(n: usize) -> String {
    (0..n)
        .map(|index| format!("\n[[module]]\nname = \"m{index}\"\nroot = \"SHT-FRONT\"\n"))
        .collect()
}

/// A malformed-table case: its label, the tables after the project, the
/// line's needle and which of its lines, the whole message when the spec fixes
/// it.
type Case = (&'static str, String, (&'static str, usize), Option<String>);

/// A good module, so a bad `[[workspace]]` is seen to leave it.
const ONE_MODULE: &str = "\n[[module]]\nname = \"front\"\nroot = \"SHT-FRONT\"\n";

/// W1: each malformed `[[workspace]]` serves the implicit workspace with one
/// note at the offending value's line (a missing key: the table's header),
/// ending `; the implicit workspace is served`, and leaves `[[module]]` as it
/// was. Where "Notes, exactly" fixes the words, the whole note is compared.
#[test]
fn a_malformed_workspace_table_serves_the_implicit_workspace_with_one_note() {
    let long_name = "n".repeat(MAX_NAME_BYTES + 1);
    let long_title = format!("t{}", "\u{e9}".repeat(MAX_TITLE_BYTES / 2));
    let many: Vec<String> = (0..=MAX_LIST).map(|_| "\"board\"".to_owned()).collect();
    let many = many.join(", ");
    let cases: Vec<Case> = vec![
        (
            "a string, not an array of tables",
            "workspace = \"desk\"\n".to_owned(),
            ("workspace = ", 1),
            None,
        ),
        (
            "a table, not an array of tables",
            "[workspace]\nname = \"desk\"\n".to_owned(),
            ("[workspace]", 1),
            None,
        ),
        (
            "an array of integers",
            "workspace = [1, 2]\n".to_owned(),
            ("workspace = ", 1),
            None,
        ),
        (
            "an unknown key",
            "[[workspace]]\nname = \"desk\"\ncolour = \"red\"\n".to_owned(),
            ("colour", 1),
            None,
        ),
        (
            "a later key (RM-0089's per-login default)",
            "[[workspace]]\nname = \"desk\"\ndefault_for = [\"mira\"]\n".to_owned(),
            ("default_for", 1),
            None,
        ),
        (
            "a name of the wrong type",
            "[[workspace]]\nname = 5\n".to_owned(),
            ("name = 5", 1),
            None,
        ),
        (
            "sections of the wrong type",
            "[[workspace]]\nname = \"desk\"\nsections = \"board\"\n".to_owned(),
            ("sections", 1),
            None,
        ),
        (
            "a section of the wrong type",
            "[[workspace]]\nname = \"desk\"\nsections = [\"board\", 5]\n".to_owned(),
            ("sections", 1),
            None,
        ),
        (
            "a landing of the wrong type",
            "[[workspace]]\nname = \"desk\"\nlanding = [\"board\"]\n".to_owned(),
            ("landing", 1),
            None,
        ),
        (
            "a module of the wrong type",
            "[[workspace]]\nname = \"desk\"\nmodule = 1\n".to_owned(),
            ("module = 1", 1),
            None,
        ),
        (
            "no name",
            "[[workspace]]\ntitle = \"Desk\"\n".to_owned(),
            ("[[workspace]]", 1),
            Some(format!(
                "`[[workspace]]` is required to hold `name`{IMPLICIT_SERVED}"
            )),
        ),
        (
            "a name in capitals",
            "[[workspace]]\nname = \"Desk\"\n".to_owned(),
            ("name = ", 1),
            Some(format!(
                "{}{IMPLICIT_SERVED}",
                name_problem("Desk").expect("no name")
            )),
        ),
        (
            "an empty name",
            "[[workspace]]\nname = \"\"\n".to_owned(),
            ("name = ", 1),
            Some(format!(
                "{}{IMPLICIT_SERVED}",
                name_problem("").expect("no name")
            )),
        ),
        (
            "a name past 64 bytes",
            format!("[[workspace]]\nname = \"{long_name}\"\n"),
            ("name = ", 1),
            Some(format!(
                "{}{IMPLICIT_SERVED}",
                name_problem(&long_name).expect("no name")
            )),
        ),
        (
            "a name twice",
            "[[workspace]]\nname = \"desk\"\n\n[[workspace]]\nname = \"desk\"\n".to_owned(),
            ("name = \"desk\"", 2),
            None,
        ),
        (
            "an empty title",
            "[[workspace]]\nname = \"desk\"\ntitle = \"\"\n".to_owned(),
            ("title", 1),
            None,
        ),
        (
            "a title past 64 UTF-8 bytes",
            format!("[[workspace]]\nname = \"desk\"\ntitle = \"{long_title}\"\n"),
            ("title", 1),
            None,
        ),
        (
            "a control character in a title",
            "[[workspace]]\nname = \"desk\"\ntitle = \"Desk\\u0007\"\n".to_owned(),
            ("title", 1),
            None,
        ),
        (
            "33 sections in a list",
            format!("[[workspace]]\nname = \"desk\"\nsections = [{many}]\n"),
            ("sections", 1),
            None,
        ),
        (
            "33 flows in a list",
            format!(
                "[[workspace]]\nname = \"desk\"\nflows = [{}]\n",
                many.replace("board", "plain")
            ),
            ("flows", 1),
            None,
        ),
    ];
    for (label, tables, (needle, nth), whole) in cases {
        // A top-level key goes before the first table header.
        let text = if tables.starts_with('[') {
            format!("{PROJECT}\n{tables}{ONE_MODULE}")
        } else {
            format!("{tables}{PROJECT}{ONE_MODULE}")
        };
        let config = read(&text);
        assert_implicit(&config, label);
        assert_eq!(
            module_rows(&config),
            ["front | front | SHT-FRONT"],
            "{label}: the modules are read apart"
        );
        let all = config.notes_at(CONFIG);
        assert_eq!(all.len(), 1, "{label}: one note: {all:?}");
        let line = nth_line_holding(&text, needle, nth);
        let opening = format!("{CONFIG}:{line}: ");
        assert!(
            all[0].starts_with(&opening) && all[0].ends_with(IMPLICIT_SERVED),
            "{label}: `{opening}...{IMPLICIT_SERVED}`: {all:?}"
        );
        if let Some(message) = whole {
            assert_eq!(all[0], note(line, &message), "{label}");
        }
        if label == "an unknown key" || label.starts_with("a later key") {
            let key = if label == "an unknown key" {
                "colour"
            } else {
                "default_for"
            };
            assert!(
                all[0].starts_with(&format!("{opening}unknown field `{key}`")),
                "{label}: the TOML reader's message: {all:?}"
            );
        }
        if label == "a name twice" {
            assert!(all[0].contains("`desk`"), "{label}: names it: {all:?}");
        }
        assert!(
            ProjectConfig::from_toml(&text).is_ok(),
            "{label}: nothing is refused (D2, M7)"
        );
    }
}

/// W1: each malformed `[[module]]` serves no module with one note ending
/// `; no module is served`, and leaves `[[workspace]]` as it was. A module
/// selector (D4's left-out kind key) is an unknown key.
#[test]
fn a_malformed_module_table_serves_no_module_with_one_note() {
    let long_root = "R".repeat(MAX_ROOT_BYTES + 1);
    let cases: Vec<Case> = vec![
        (
            "a string, not an array of tables",
            "module = \"front\"\n".to_owned(),
            ("module = ", 1),
            None,
        ),
        (
            "a table, not an array of tables",
            "[module]\nname = \"front\"\nroot = \"SHT-FRONT\"\n".to_owned(),
            ("[module]", 1),
            None,
        ),
        (
            "an unknown key",
            "[[module]]\nname = \"front\"\nroot = \"SHT-FRONT\"\ncolour = \"red\"\n".to_owned(),
            ("colour", 1),
            None,
        ),
        (
            "a kind selector (D4)",
            "[[module]]\nname = \"front\"\nroot = \"SHT-FRONT\"\nkinds = [\"broadsheet\"]\n"
                .to_owned(),
            ("kinds", 1),
            None,
        ),
        (
            "a root of the wrong type",
            "[[module]]\nname = \"front\"\nroot = [\"SHT-FRONT\"]\n".to_owned(),
            ("root", 1),
            None,
        ),
        (
            "no name",
            "[[module]]\nroot = \"SHT-FRONT\"\n".to_owned(),
            ("[[module]]", 1),
            Some(format!(
                "`[[module]]` is required to hold `name`{NO_MODULE_SERVED}"
            )),
        ),
        (
            "no root",
            "[[module]]\nname = \"front\"\n".to_owned(),
            ("[[module]]", 1),
            Some(format!(
                "`[[module]]` is required to hold `root`{NO_MODULE_SERVED}"
            )),
        ),
        (
            "a bad name",
            "[[module]]\nname = \"Front page\"\nroot = \"SHT-FRONT\"\n".to_owned(),
            ("name = \"Front page\"", 1),
            Some(format!(
                "{}{NO_MODULE_SERVED}",
                name_problem("Front page").expect("no name")
            )),
        ),
        (
            "a name twice",
            "[[module]]\nname = \"front\"\nroot = \"SHT-FRONT\"\n\n[[module]]\nname = \
             \"front\"\nroot = \"SHT-BACK\"\n"
                .to_owned(),
            ("name = \"front\"", 2),
            None,
        ),
        (
            "an empty title",
            "[[module]]\nname = \"front\"\ntitle = \"\"\nroot = \"SHT-FRONT\"\n".to_owned(),
            ("title", 1),
            None,
        ),
        (
            "an empty root",
            "[[module]]\nname = \"front\"\nroot = \"\"\n".to_owned(),
            ("root", 1),
            None,
        ),
        (
            "a root past 256 bytes",
            format!("[[module]]\nname = \"front\"\nroot = \"{long_root}\"\n"),
            ("root", 1),
            None,
        ),
        (
            "a space at a root's start",
            "[[module]]\nname = \"front\"\nroot = \" SHT-FRONT\"\n".to_owned(),
            ("root", 1),
            None,
        ),
        (
            "a space at a root's end",
            "[[module]]\nname = \"front\"\nroot = \"SHT-FRONT \"\n".to_owned(),
            ("root", 1),
            None,
        ),
        (
            "a control character in a root",
            "[[module]]\nname = \"front\"\nroot = \"SHT-\\tFRONT\"\n".to_owned(),
            ("root", 1),
            None,
        ),
    ];
    let workspace = "\n[[workspace]]\nname = \"desk\"\nsections = [\"tree\"]\n";
    for (label, tables, (needle, nth), whole) in cases {
        // A top-level key goes before the first table header.
        let text = if tables.starts_with('[') {
            format!("{PROJECT}{workspace}\n{tables}")
        } else {
            format!("{tables}{PROJECT}{workspace}")
        };
        let config = read(&text);
        assert_eq!(config.source, WorkspaceSource::Config, "{label}");
        assert_eq!(
            rows(&config),
            ["desk | desk | landing overview | sections tree | flows (all) | module (none)"],
            "{label}: the workspaces are read apart"
        );
        assert!(config.modules.is_empty(), "{label}: no module");
        let all = config.notes_at(CONFIG);
        assert_eq!(all.len(), 1, "{label}: one note: {all:?}");
        let line = nth_line_holding(&text, needle, nth);
        let opening = format!("{CONFIG}:{line}: ");
        assert!(
            all[0].starts_with(&opening) && all[0].ends_with(NO_MODULE_SERVED),
            "{label}: `{opening}...{NO_MODULE_SERVED}`: {all:?}"
        );
        if let Some(message) = whole {
            assert_eq!(all[0], note(line, &message), "{label}");
        }
        if label == "a kind selector (D4)" {
            assert!(
                all[0].starts_with(&format!("{opening}unknown field `kinds`")),
                "{label}: the TOML reader's message: {all:?}"
            );
        }
        assert!(
            ProjectConfig::from_toml(&text).is_ok(),
            "{label}: nothing is refused (D2, M7)"
        );
    }
}

/// "Data": every limit at its cap is served, one past it is a malformed
/// table: names and titles of 64 bytes (a title in two-byte letters), a root
/// of 256, 32 workspaces, 64 modules, lists of 32.
#[test]
fn the_limits_hold_at_their_cap_and_fail_one_past_it() {
    let name = "n".repeat(MAX_NAME_BYTES);
    let title = "\u{e9}".repeat(MAX_TITLE_BYTES / 2);
    assert_eq!(title.len(), MAX_TITLE_BYTES);
    let root = "R".repeat(MAX_ROOT_BYTES);
    let list: Vec<&str> = (0..MAX_LIST).map(|_| "\"board\"").collect();
    let text = format!(
        "{PROJECT}\n[[workspace]]\nname = \"{name}\"\ntitle = \"{title}\"\nsections = \
         [{}]\nflows = [{}]\n\n[[module]]\nname = \"{name}\"\ntitle = \"{title}\"\nroot = \
         \"{root}\"\n",
        list.join(", "),
        list.join(", ").replace("board", "plain")
    );
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    let only = &config.workspaces[0];
    assert_eq!(
        (only.name.as_str(), only.title.as_str()),
        (name.as_str(), title.as_str())
    );
    assert_eq!(only.sections, ["board"]);
    assert_eq!(only.flows, Some(vec!["plain".to_owned()]));
    assert_eq!(module_rows(&config), [format!("{name} | {title} | {root}")]);
    // Thirty-one repeats of each list: each a note, none a fallback.
    let all = config.notes_at(CONFIG);
    assert_eq!(all.len(), 2 * (MAX_LIST - 1), "{all:?}");
    assert!(
        all.iter()
            .all(|text| text.ends_with("is listed twice: the second left out"))
    );

    let config = read(&format!("{PROJECT}{}", workspace_tables(MAX_WORKSPACES)));
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(config.workspaces.len(), MAX_WORKSPACES);
    assert!(config.notes.is_empty(), "{:?}", config.notes);
    let config = read(&format!(
        "{PROJECT}{}",
        workspace_tables(MAX_WORKSPACES + 1)
    ));
    assert_implicit(&config, "33 workspaces");
    let all = config.notes_at(CONFIG);
    assert_eq!(all.len(), 1, "{all:?}");
    assert!(all[0].ends_with(IMPLICIT_SERVED), "{all:?}");

    let config = read(&format!("{PROJECT}{}", module_tables(MAX_MODULES)));
    assert_eq!(config.modules.len(), MAX_MODULES);
    assert!(config.notes.is_empty(), "{:?}", config.notes);
    let config = read(&format!("{PROJECT}{}", module_tables(MAX_MODULES + 1)));
    assert!(config.modules.is_empty(), "65 modules: none");
    let all = config.notes_at(CONFIG);
    assert_eq!(all.len(), 1, "{all:?}");
    assert!(all[0].ends_with(NO_MODULE_SERVED), "{all:?}");
}

// ---------------------------------------------------------------- notes

/// "The reader": notes sorted by line, ties in judging order. One line holds
/// a whole workspace (sections, then landing, flows, module), the next a
/// malformed module table: the module's note, judged first, comes last.
#[test]
fn notes_are_sorted_by_line_and_ties_keep_the_judging_order() {
    let text = format!(
        "workspace = [{{ name = \"desk\", sections = [\"board\", \"board\", \"stage\"], landing \
         = \"stage\", flows = [\"inks\"], module = \"front\" }}]\nmodule = [{{ name = \"Front\", \
         root = \"SHT-FRONT\" }}]\n{PROJECT}"
    );
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert!(config.modules.is_empty());
    assert_eq!(
        config.notes_at(CONFIG),
        [
            note(
                1,
                "workspace `desk`: section `board` is listed twice: the second left out"
            ),
            note(
                1,
                &format!("workspace `desk`: `stage` is no section {SECTION_LIST}: left out")
            ),
            note(
                1,
                "workspace `desk`: landing `stage` is not one of its sections: it lands on the \
                 Overview"
            ),
            note(
                1,
                "workspace `desk`: no `[[card.flow]]` is named `inks`: left out"
            ),
            note(
                1,
                "workspace `desk`: no `[[module]]` is named `front`: no default module"
            ),
            note(
                2,
                &format!(
                    "{}{NO_MODULE_SERVED}",
                    name_problem("Front").expect("no name")
                )
            ),
        ]
    );
    let lines: Vec<String> = config
        .notes
        .iter()
        .map(|note| note.line.to_string())
        .collect();
    assert_eq!(lines, ["1", "1", "1", "1", "1", "2"]);
}

/// "Rules and edge cases", `--config FILE`: `notes_at` names the file as
/// given; each note's `line` and `message` are its two halves.
#[test]
fn notes_at_names_the_file_as_given() {
    let text = variant("(b)");
    let config = read(&text);
    assert_eq!(
        config.notes_at("conf/press.toml"),
        ["conf/press.toml:41: workspace `newsroom`: no `[[card.flow]]` is named `inks`: left out"]
    );
    assert_eq!(config.notes[0].line.to_string(), "41");
    assert_eq!(
        config.notes[0].message,
        "workspace `newsroom`: no `[[card.flow]]` is named `inks`: left out"
    );
}

// ------------------------------------------------------------------ data

/// "Rules and edge cases": mixed scripts in a title are data, and a root is
/// not resolved here (a look-alike is the read's exit 2, not the reader's).
#[test]
fn titles_and_roots_are_data() {
    // "Ma" + Cyrillic "ket" + "a": a mixed-script title.
    let title = "Ma\u{043a}\u{0435}\u{0442}a";
    // SHT-FRONT with a Cyrillic O.
    let root = "SHT-FR\u{041e}NT";
    let text = format!(
        "{PROJECT}\n[[workspace]]\nname = \"desk\"\ntitle = \"{title}\"\n\n[[module]]\nname = \
         \"front\"\ntitle = \"{title}\"\nroot = \"{root}\"\n"
    );
    let config = read(&text);
    assert_eq!(config.workspaces[0].title, title);
    assert_eq!(module_rows(&config), [format!("front | {title} | {root}")]);
    assert!(config.notes.is_empty(), "{:?}", config.notes);
}

/// "Description and interactions": the reader never reads `[team]`: a bad one
/// leaves card-press's document as it was.
#[test]
fn a_bad_team_changes_nothing_here() {
    let text = press_config();
    let with_team = format!("{text}\n[team.mira]\nrole = \"editor\"\nhat = 5\n");
    assert_eq!(read(&with_team), read(&text));
}

// ------------------------------------------------------------- the preset

/// `scripts/presets/gamedev.toml` as committed.
fn preset() -> String {
    let path = repository_root().join("scripts/presets/gamedev.toml");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The four workspaces of "The gamedev preset", as [`row`]s.
const PRESET_ROWS: [&str; 4] = [
    "writing | Writing | landing flow | sections flow, tree, board, questions, inbox | flows \
     narrative | module (none)",
    "development | Development | landing board | sections board, inbox, tree, flow, graph, \
     health, roadmap, questions | flows development | module (none)",
    "art | Art and levels | landing board | sections board, flow, tree, questions, inbox | flows \
     art, level-design | module (none)",
    "production | Production | landing overview | sections inbox, roadmap, board, flow, tree, \
     graph, health, questions | flows (all) | module (none)",
];

/// The five flows of "The gamedev preset": name, disciplines, `(id, title)`
/// columns, accept. Since board-handoff (docs/canon/workspaces.md "Preset"):
/// each flow's return column `sent-back`, titled "Returned", second in
/// `columns`.
type PresetFlow = (
    &'static str,
    &'static [(&'static str, &'static str)],
    (&'static str, &'static str),
);

const PRESET_FLOWS: [PresetFlow; 5] = [
    (
        "narrative",
        &[
            ("backlog", "Backlog"),
            ("sent-back", "Returned"),
            ("outline", "Outline"),
            ("draft", "Draft"),
            ("review", "Review"),
            ("approved", "Approved"),
            ("ready-for-dev", "Ready for dev"),
            ("in-game", "In game"),
        ],
        ("ready-for-dev", "in-game"),
    ),
    (
        "development",
        &[
            ("backlog", "Backlog"),
            ("sent-back", "Returned"),
            ("ready", "Ready"),
            ("in-progress", "In progress"),
            ("in-review", "In review"),
            ("verify", "Verify"),
            ("done", "Done"),
        ],
        ("verify", "done"),
    ),
    (
        "art",
        &[
            ("waiting", "Waiting"),
            ("sent-back", "Returned"),
            ("ready", "Ready"),
            ("in-progress", "In progress"),
            ("pending-review", "Pending review"),
            ("approved", "Approved"),
            ("in-engine", "In engine"),
        ],
        ("approved", "in-engine"),
    ),
    (
        "level-design",
        &[
            ("layout", "Layout"),
            ("sent-back", "Returned"),
            ("blockout", "Blockout"),
            ("playtest", "Playtest"),
            ("scripting", "Scripting"),
            ("lighting", "Lighting"),
            ("art-pass", "Art pass"),
            ("final", "Final"),
        ],
        ("art-pass", "final"),
    ),
    (
        "production",
        &[
            ("to-do", "To do"),
            ("sent-back", "Returned"),
            ("in-progress", "In progress"),
            ("review", "Review"),
            ("done", "Done"),
        ],
        ("review", "done"),
    ),
];

/// docs/canon/workspaces.md "Preset": the preset's return column, the same in
/// every flow.
const PRESET_RETURN: &str = "sent-back";

/// WS-7: the preset appended to `[project]\nslug = "preset"\n` is a config
/// `ProjectConfig` accepts; `WorkspaceConfig` reads `config`, the four
/// workspaces in order with their fields, no module and no note; `CardConfig`
/// its five flows, each its own discipline, with columns, titles and accept,
/// and no `moves` (every ordered pair of the open columns, ADR-0012).
#[test]
fn ws7_the_preset_reads_whole_with_no_note() {
    let text = format!("[project]\nslug = \"preset\"\n{}", preset());
    ProjectConfig::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("preset")));
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(rows(&config), PRESET_ROWS);
    assert!(config.modules.is_empty(), "roots are a project's own IDs");
    assert!(config.notes.is_empty(), "{:?}", config.notes_at("preset"));
    for workspace in &config.workspaces {
        assert!(
            !["lead", "owner", "member"].contains(&workspace.name.as_str()),
            "R10: no name echoes `[team]`'s roles: {}",
            workspace.name
        );
    }

    let cards =
        CardConfig::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at("preset")));
    let names: Vec<&str> = cards.flows.iter().map(|flow| flow.name.as_str()).collect();
    assert_eq!(names, PRESET_FLOWS.map(|(name, _, _)| name));
    for (flow, (name, columns, (from, to))) in cards.flows.iter().zip(PRESET_FLOWS) {
        assert_eq!(flow.disciplines, [name], "{name}");
        let read: Vec<(&str, &str)> = flow
            .columns
            .iter()
            .map(|column| (column.id.as_str(), column.title.as_str()))
            .collect();
        assert_eq!(read, columns, "{name}");
        assert_eq!(
            (flow.accept.from.as_str(), flow.accept.to.as_str()),
            (from, to),
            "{name}"
        );
        // docs/canon/card-flows.md "Config": `return = "sent-back"` on the
        // line after `accept`, in every flow.
        assert_eq!(flow.return_column.as_deref(), Some(PRESET_RETURN), "{name}");
        let open = columns.len() - 1;
        assert_eq!(
            flow.moves.len(),
            open * (open - 1),
            "{name}: every ordered pair"
        );
        assert!(
            flow.moves.iter().all(|step| step.by.is_empty()),
            "{name}: anyone"
        );
    }
}

/// The marker lines of `fixtures/studio-lamp/specengine.toml`.
const PRESET_OPENS: &str = "# --- scripts/presets/gamedev.toml ---\n";
const PRESET_ENDS: &str = "# --- end of the preset ---\n";

/// WS-7: `fixtures/studio-lamp/specengine.toml` holds the preset's bytes
/// verbatim between its markers, and reads as the preset plus its own two
/// modules, with no note; its prefixes and kinds are its `expected.json`'s.
/// M16: a title changed in the fixture's copy alone.
#[test]
fn ws7_studio_lamp_holds_the_preset_verbatim() {
    let text = workspaces_common::fixture_config("studio-lamp");
    assert_eq!(text.matches(PRESET_OPENS).count(), 1, "one opening marker");
    assert_eq!(text.matches(PRESET_ENDS).count(), 1, "one closing marker");
    let start = text.find(PRESET_OPENS).expect("opens") + PRESET_OPENS.len();
    let end = text.find(PRESET_ENDS).expect("ends");
    assert!(start <= end, "the markers in order");
    assert!(
        text[start..end] == preset(),
        "fixtures/studio-lamp holds scripts/presets/gamedev.toml verbatim between its markers"
    );

    ProjectConfig::from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)));
    let config = read(&text);
    assert_eq!(config.source, WorkspaceSource::Config);
    assert_eq!(rows(&config), PRESET_ROWS);
    assert_eq!(
        module_rows(&config),
        [
            "shore | The shore | CHP-SHORE",
            "island | The lamp island | ISL-LAMP"
        ]
    );
    assert!(config.notes.is_empty(), "{:?}", config.notes_at(CONFIG));

    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture("studio-lamp").join("expected.json")).expect("expected.json"),
    )
    .expect("JSON");
    let scheme = corpus_scheme(&fixture("studio-lamp"));
    let mut prefixes: Vec<String> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.prefix.clone())
        .collect();
    let mut kinds: Vec<String> = scheme
        .prefixes()
        .iter()
        .map(|spec| spec.kind.clone())
        .collect();
    prefixes.sort();
    kinds.sort();
    assert_eq!(serde_json::json!(prefixes), expected["prefixes"]);
    assert_eq!(serde_json::json!(kinds), expected["kinds"]);
}

// ---------------------------------------------- board-handoff, `return`

/// board-handoff BH-13 (docs/canon/workspaces.md "Preset"): the preset's text
/// gives each of its five flows `"sent-back"` second in `columns`, `sent-back =
/// "Returned"` second in `titles`, and `return = "sent-back"` on the line
/// after `accept`; studio-lamp's copy holds the same (its verbatim test
/// above).
#[test]
fn board_handoff_bh13_the_preset_gives_each_flow_its_return_line() {
    let text = preset();
    let lines: Vec<&str> = text.lines().collect();
    let accepts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.starts_with("accept = "))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(accepts.len(), 5, "one `accept` per flow");
    for at in accepts {
        assert_eq!(
            lines.get(at + 1).copied(),
            Some("return = \"sent-back\""),
            "the line after {:?}",
            lines[at]
        );
    }
    assert_eq!(text.matches("return = ").count(), 5, "five return lines");
    let columns: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with("columns = ["))
        .collect();
    assert_eq!(columns.len(), 5);
    for line in columns {
        let second = line
            .trim_start_matches("columns = [")
            .split(", ")
            .nth(1)
            .unwrap_or_else(|| panic!("a second column: {line}"));
        assert_eq!(second, "\"sent-back\"", "{line}");
    }
    let titles: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| line.starts_with("titles = {"))
        .collect();
    assert_eq!(titles.len(), 5);
    for line in titles {
        assert_eq!(
            line.trim_start_matches("titles = { ").split(", ").nth(1),
            Some("sent-back = \"Returned\""),
            "{line}"
        );
    }
}

/// The preset under `[project]`, `development`'s `return` line replaced by
/// `line`: the text, and the 1-based line it stands on.
fn with_development_return(line: &str) -> (String, usize) {
    let base = format!("[project]\nslug = \"preset\"\n{}", preset());
    let at = nth_line_holding(&base, "return = \"sent-back\"", 2);
    let mut lines: Vec<&str> = base.lines().collect();
    assert_eq!(
        lines[at - 3],
        "titles = { backlog = \"Backlog\", sent-back = \"Returned\", ready = \"Ready\", \
         in-progress = \"In progress\", in-review = \"In review\", verify = \"Verify\", done = \
         \"Done\" }",
        "the second flow is development"
    );
    lines[at - 1] = line;
    let mut text = lines.join("\n");
    text.push('\n');
    (text, at)
}

/// board-handoff BH-13 (docs/canon/card-flows.md "Config"): each of the five
/// errors of `return` stands at its line, `<config>:<line>: <message>`, word for word;
/// `WorkspaceConfig` serves the flows unjudged with that error as its one note
/// (`workspaces.md` "Config"), so only `spec card` stops on it. M15: the first
/// column admitted as `return`.
#[test]
fn board_handoff_bh13_each_return_error_stands_at_its_line() {
    let (sound, _) = with_development_return("return = \"sent-back\"");
    let cards = card_from_toml(&sound).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)));
    assert_eq!(
        cards.flows[1].return_column.as_deref(),
        Some("sent-back"),
        "the unchanged copy reads"
    );
    for (line, message) in [
        ("return = 3", "`return` is a string, not an integer"),
        ("return = true", "`return` is a string, not a boolean"),
        (
            "return = [\"sent-back\"]",
            "`return` is a string, not an array",
        ),
        (
            "return = { column = \"sent-back\" }",
            "`return` is a string, not a table",
        ),
        (
            "return = \"shelved\"",
            "`return`: `shelved` is no column of this flow",
        ),
        (
            "return = \"Sent-back\"",
            "`return`: `Sent-back` is no column of this flow",
        ),
        (
            "return = \"outline\"",
            "`return`: `outline` is no column of this flow",
        ),
        (
            "return = \"backlog\"",
            "`return`: `backlog` is the first column, where a new card starts",
        ),
        (
            "return = \"verify\"",
            "`return`: `verify` is `accept.from`, where a card waits for its accept",
        ),
        (
            "return = \"done\"",
            "`return`: `done` is `accept.to`, reached only by an accept",
        ),
    ] {
        let (text, at) = with_development_return(line);
        let error = card_from_toml(&text).expect_err(line);
        assert_eq!(error.line, Some(at), "{line}: {error:?}");
        assert_eq!(error.at(CONFIG), note(at, message), "{line}");
        // `spec workspace` serves its flows unjudged, the card's error its
        // one note, and the workspaces of the preset as written.
        let config = read(&text);
        assert_eq!(config.source, WorkspaceSource::Config, "{line}");
        assert_eq!(rows(&config), PRESET_ROWS, "{line}");
        assert_eq!(
            config.notes_at(CONFIG),
            [format!(
                "{}; the workspaces' flows are served unjudged",
                error.at(CONFIG)
            )],
            "{line}"
        );
        ProjectConfig::from_toml(&text)
            .unwrap_or_else(|error| panic!("{line}: `[card]` is only name-checked: {error:?}"));
    }
    // Any other open column of the flow is a return column; leaving the key
    // out is none.
    for column in ["ready", "in-progress", "in-review"] {
        let (text, _) = with_development_return(&format!("return = \"{column}\""));
        let cards = card_from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)));
        assert_eq!(cards.flows[1].return_column.as_deref(), Some(column));
    }
    let (text, _) = with_development_return("");
    let cards = card_from_toml(&text).unwrap_or_else(|error| panic!("{}", error.at(CONFIG)));
    assert_eq!(
        cards.flows[1].return_column, None,
        "absent: no return column"
    );
    assert_eq!(
        cards.flows[0].return_column.as_deref(),
        Some("sent-back"),
        "the other flows keep theirs"
    );
}

// --------------------------------------------------------------- RawFile

/// WS-8, the core half: `ProjectConfig` takes both keys whatever they hold
/// (they are only name-checked: M7), on both projects, and refuses
/// `[[workspaces]]` and `[[modules]]` at their header with the TOML reader's
/// unknown-field message naming both keys among the expected. M18.
#[test]
fn ws8_project_config_name_checks_both_keys_and_refuses_a_misspelt_one() {
    let press = press_config();
    for (label, text) in [
        ("card-press", press.clone()),
        (
            "studio-lamp",
            workspaces_common::fixture_config("studio-lamp"),
        ),
        (
            "a string",
            format!("workspace = \"x\"\nmodule = 5\n{PROJECT}"),
        ),
        (
            "unknown keys inside",
            format!("{PROJECT}\n[[workspace]]\nbogus = 1\n\n[[module]]\nbogus = 1\n"),
        ),
    ] {
        ProjectConfig::from_toml(&text)
            .unwrap_or_else(|error| panic!("{label}: {}", error.at(CONFIG)));
    }
    let workspaces = misspelt(&press);
    let modules = press.replacen("\n[[module]]\n", "\n[[modules]]\n", 1);
    for (key, text, header) in [
        ("workspaces", &workspaces, "[[workspaces]]"),
        ("modules", &modules, "[[modules]]"),
    ] {
        let error = ProjectConfig::from_toml(text).expect_err(key);
        assert_eq!(
            error.line,
            Some(line_holding(text, header)),
            "{key}: {error:?}"
        );
        assert!(
            error
                .message
                .starts_with(&format!("unknown field `{key}`, expected one of ")),
            "{key}: {error:?}"
        );
        for known in ["`workspace`", "`module`", "`team`", "`card`"] {
            assert!(
                error.message.contains(known),
                "{key}: lists {known}: {error:?}"
            );
        }
    }
}
