//! B12 of docs/features/hub-confirm.md, the CLI's half of the create path
//! (`docs/canon/proposal-kinds.md` "Propose", its Target item): `spec propose create` refuses a
//! new file's path holding a control character, LF included, and stores
//! nothing: no proposal, no event, no file in the worktree. The same text at a
//! clean path is stored, so the character is the refusal. The MCP half is
//! `specengine-mcp`'s `tests/mcp_create.rs`.
//!
//! Every repository is a scratch copy of `fixtures/spec-a` (`common::proposal`).
//! Control characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use common::proposal::Pair;

/// A spec-a record `id`, closed front-matter, one heading.
fn record(id: &str) -> String {
    format!(
        "---\nid: {id}\nclass: canon\nstatus: accepted\nowner: owner\nreviewed: 2026-09-20\n---\n\n\
         # Record {id}\n\nA body.\n"
    )
}

/// `spec propose create path --text-file -` of `R-13`'s record in the linked
/// worktree, as an agent raises it.
fn proposed(pair: &Pair, path: &str) -> common::Run {
    pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "create",
            path,
            "--text-file",
            "-",
            "--rationale",
            "A new record.",
        ],
        record("R-13").as_bytes(),
    )
}

/// The control characters of a path: LF and U+0007, the criterion's two, and
/// CR, TAB, ESC, DEL and the C1 U+0085, which `char::is_control` names too.
const CONTROLS: [(&str, &str); 7] = [
    ("LF", "docs/records/R/R-1\n3.md"),
    ("BEL", "docs/records/R/R-13\u{7}.md"),
    ("CR", "docs/records/R/R-13\r.md"),
    ("TAB", "docs/records/R\t/R-13.md"),
    ("ESC", "docs/records/R/\u{1b}R-13.md"),
    ("DEL", "docs/records/R/R-13\u{7f}.md"),
    ("NEL", "docs/records/R/R-13\u{85}.md"),
];

/// B12: each path refused, a non-zero exit with no ID on stdout, the queue and
/// both worktrees as they were; the clean path then stores `PR-0001`. M: the
/// path check dropped from `spec propose create`.
#[test]
fn a_new_file_path_with_a_control_character_is_refused_and_nothing_is_stored() {
    let pair = Pair::new("create-path-controls", "spec-a");
    let status = |dir: &std::path::Path| {
        pair.git
            .git_text(dir, &["status", "--porcelain=v1", "--untracked-files=all"])
    };
    for (label, path) in CONTROLS {
        let run = proposed(&pair, path);
        assert_ne!(run.code, 0, "{label}: refused: {run:?}");
        assert!(!run.stdout.contains("PR-"), "{label}: no ID: {run:?}");
        assert!(pair.proposals().is_empty(), "{label}: nothing stored");
        assert!(pair.events().is_empty(), "{label}: no event");
        assert_eq!(status(&pair.linked), "", "{label}: the worktree clean");
        assert_eq!(status(&pair.main), "", "{label}: the main worktree clean");
    }
    let run = proposed(&pair, "docs/records/R/R-13.md");
    assert_eq!(run.code, 0, "{run:?}");
    assert_eq!(run.stdout.lines().next(), Some("PR-0001"), "{run:?}");
    assert_eq!(pair.proposals().len(), 1);
}
