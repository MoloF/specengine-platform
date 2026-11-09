//! The scratch changes of docs/features/workspaces.md "Acceptance criteria",
//! WS-4 and WS-8, over `fixtures/card-press`'s config: one source for core's
//! `tests/workspace_toml.rs`, this crate's `tests/workspace_cli.rs` and
//! `specengine-http`'s `tests/workspaces.rs` (each takes it by `#[path]`), so
//! the three judge the same bytes. Pure text: no process, no file written.
//!
//! The fixture's tables are the example of the spec's "Data", appended after
//! its `[card]`, so its first `[[workspace]]` header opens them all.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

/// How every note and config error names the config when none is given.
pub const CONFIG: &str = "specengine.toml";

/// The first line of the appended tables, its line end before it.
pub const TABLES_START: &str = "\n[[workspace]]\n";

/// The implicit document of the spec's "Data", on `fixtures/spec-a`.
pub const IMPLICIT_JSON: &str = "{\"source\":\"implicit\",\"notes\":[],\"workspaces\":[{\"name\":\
     \"all\",\"title\":\"All sections\",\"landing\":\"overview\",\"sections\":[\"inbox\",\"board\",\
     \"roadmap\",\"flow\",\"tree\",\"graph\",\"health\",\"questions\"],\"flows\":null,\"module\":\
     null}],\"modules\":[]}";

/// card-press's document of the spec's "Data", compact.
pub const PRESS_JSON: &str = "{\"source\":\"config\",\"notes\":[],\"workspaces\":[{\"name\":\
     \"newsroom\",\"title\":\"Newsroom\",\"landing\":\"board\",\"sections\":[\"board\",\"tree\",\
     \"inbox\"],\"flows\":[\"copy\"],\"module\":\"weekly\"},{\"name\":\"pressroom\",\"title\":\
     \"Pressroom\",\"landing\":\"overview\",\"sections\":[\"board\",\"flow\",\"tree\",\"health\"],\
     \"flows\":[\"plates\"],\"module\":null}],\"modules\":[{\"name\":\"weekly\",\"title\":\"The \
     weekly run\",\"root\":\"PRN-WEEKLY\"},{\"name\":\"rulings\",\"title\":\"Style rulings\",\
     \"root\":\"STY-0001\"}]}";

/// card-press's `spec workspace` text of the spec's "Data".
pub const PRESS_TEXT: &str = "workspaces config | workspaces 2, modules 2\n\
     workspace newsroom | Newsroom | landing board | sections board, tree, inbox | flows copy | \
     module weekly\n\
     workspace pressroom | Pressroom | landing overview | sections board, flow, tree, health | \
     flows plates | module (none)\n\
     module weekly | The weekly run | root PRN-WEEKLY\n\
     module rulings | Style rulings | root STY-0001\n";

/// spec-a's `spec workspace` text (WS-1).
pub const IMPLICIT_TEXT: &str = "workspaces implicit | workspaces 1, modules 0\n\
     workspace all | All sections | landing overview | sections inbox, board, roadmap, flow, \
     tree, graph, health, questions | flows (all) | module (none)\n";

pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repository root exists")
}

/// `fixtures/<name>/specengine.toml` as committed.
pub fn fixture_config(name: &str) -> String {
    let path = repository_root().join("fixtures").join(name).join(CONFIG);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// card-press's config, the tables appended.
pub fn press_config() -> String {
    let text = fixture_config("card-press");
    assert_eq!(
        text.matches(TABLES_START).count(),
        2,
        "card-press holds the spec's two `[[workspace]]` tables"
    );
    text
}

/// `text` with every table from its first `[[workspace]]` on cut: the copy
/// "with both tables removed" (the tables are the config's last).
pub fn without_tables(text: &str) -> String {
    let at = text
        .find(TABLES_START)
        .unwrap_or_else(|| panic!("no `[[workspace]]` table in\n{text}"));
    text[..=at].to_owned()
}

/// `text` with its one `from` replaced by `to`.
pub fn changed(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "exactly one {from:?} in\n{text}"
    );
    text.replacen(from, to, 1)
}

/// The 1-based line of the `nth` (from 1) line of `text` holding `needle`.
pub fn nth_line_holding(text: &str, needle: &str, nth: usize) -> usize {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .nth(nth - 1)
        .map(|(index, _)| index + 1)
        .unwrap_or_else(|| panic!("no line {nth} holding {needle:?} in\n{text}"))
}

/// The 1-based line of the first line of `text` holding `needle`.
pub fn line_holding(text: &str, needle: &str) -> usize {
    nth_line_holding(text, needle, 1)
}

/// WS-4's seven scratch changes of `base` (card-press's config), labelled
/// (a) to (g) as the spec names them, each the whole changed text.
pub fn variants(base: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "(a) newsroom's sections gain `stage`",
            changed(
                base,
                "sections = [\"board\", \"tree\", \"inbox\"]",
                "sections = [\"board\", \"tree\", \"inbox\", \"stage\"]",
            ),
        ),
        (
            "(b) newsroom's flows gain `inks`",
            changed(
                base,
                "flows = [\"copy\"]\n",
                "flows = [\"copy\", \"inks\"]\n",
            ),
        ),
        (
            "(c) pressroom gains `landing = \"inbox\"`",
            changed(
                base,
                "flows = [\"plates\"]\n",
                "landing = \"inbox\"\nflows = [\"plates\"]\n",
            ),
        ),
        (
            "(d) pressroom gains `module = \"plates-room\"`",
            changed(
                base,
                "flows = [\"plates\"]\n",
                "flows = [\"plates\"]\nmodule = \"plates-room\"\n",
            ),
        ),
        (
            "(e) newsroom's name is `News room`",
            changed(base, "name = \"newsroom\"", "name = \"News room\""),
        ),
        (
            "(f) rulings loses its root",
            changed(base, "root = \"STY-0001\"\n", ""),
        ),
        (
            "(g) copy's accept is removed",
            changed(base, "accept = { from = \"proof\", to = \"filed\" }\n", ""),
        ),
    ]
}

/// WS-8's copy: the first `[[workspace]]` header misspelt `[[workspaces]]`.
pub fn misspelt(base: &str) -> String {
    base.replacen(TABLES_START, "\n[[workspaces]]\n", 1)
}
