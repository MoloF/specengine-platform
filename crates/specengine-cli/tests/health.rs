//! docs/features/health-left.md AC-T1, the CLI half: AC-R1 to AC-R5 and
//! AC-R7 over git copies of `fixtures/spec-a` (the game), `spec-b` (a
//! command-line tool, Russian prose, a legacy Cyrillic alias, a broken
//! front-matter in `QN-08`) and the non-Rust `spec-c`, the amendments
//! written into the copies (never into `fixtures/`).
//!
//! The amended copy of A ([`amended_files`]): listed, a `spec` `draft`
//! amending `RULE-STAM-REGEN`, an `accepted` decision whose `amends` take
//! every state (resolved, the alias `QST-031` written raw, the dangling
//! `RULE-NOPE`, `DEC-0007` in a Tier 3 file), a decision without
//! `status:`, a document without class, ID or title; not listed, a spec
//! `shipped` and one `abandoned`, a decision `superseded-by` and one
//! `proposed`, a `class: generated` one, one with only `depends_on` and a
//! body `[[RULE-STAM-REGEN]]`, and an amendment only proposed (in the
//! queue, not in the tree).
//!
//! Mutations (the spec's "Acceptance criteria"): AC-R1 `is_live` ->
//! `!is_tier3_file`, the live filter removed, any link type taken; AC-R2
//! unresolved links dropped, `written` canonicalised; AC-R3 `notes`
//! omitted when empty, rows sorted by title; AC-R4 the gap ignored, the
//! broken file skipped without a note; AC-R5 an index refresh, a `HashMap`
//! order; AC-R7 an `"amends"` literal in the new core or CLI code.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use common::check::{registered, set_paths_key};
use common::git::Sandbox;
use common::proposal::Pair;
use common::{Run, Scratch, read_text, repository_root, snapshot, spec, write};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use specengine_cli::{Env, Globals, Outcome, health, render_json, render_text};
use specengine_model::{AMENDS, LINK_TYPES};

// ------------------------------------------------------------ ordered JSON

/// A JSON value whose objects keep their keys in the order written.
#[derive(Debug, Clone, PartialEq)]
enum J {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Any;
        impl<'de> Visitor<'de> for Any {
            type Value = J;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any JSON value")
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_none<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_bool<E>(self, value: bool) -> Result<J, E> {
                Ok(J::Bool(value))
            }
            fn visit_u64<E>(self, value: u64) -> Result<J, E> {
                Ok(J::Num(value.to_string()))
            }
            fn visit_i64<E>(self, value: i64) -> Result<J, E> {
                Ok(J::Num(value.to_string()))
            }
            fn visit_f64<E>(self, value: f64) -> Result<J, E> {
                Ok(J::Num(value.to_string()))
            }
            fn visit_str<E>(self, value: &str) -> Result<J, E> {
                Ok(J::Str(value.to_owned()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<J, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element::<J>()? {
                    items.push(item);
                }
                Ok(J::Arr(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<J, A::Error> {
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, J>()? {
                    entries.push((key, value));
                }
                Ok(J::Obj(entries))
            }
        }
        deserializer.deserialize_any(Any)
    }
}

impl J {
    fn parse(text: &str) -> J {
        serde_json::from_str(text).unwrap_or_else(|error| panic!("not JSON ({error}): {text}"))
    }

    fn keys(&self) -> Vec<&str> {
        match self {
            J::Obj(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
            other => panic!("not an object: {other:?}"),
        }
    }

    fn get(&self, key: &str) -> &J {
        match self {
            J::Obj(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("no key {key}: {self:?}")),
            other => panic!("not an object: {other:?}"),
        }
    }

    fn items(&self) -> &[J] {
        match self {
            J::Arr(items) => items,
            other => panic!("not an array: {other:?}"),
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            J::Str(text) => Some(text),
            J::Null => None,
            other => panic!("neither a string nor null: {other:?}"),
        }
    }

    fn number(&self) -> usize {
        match self {
            J::Num(text) => text.parse().expect("a count"),
            other => panic!("not a number: {other:?}"),
        }
    }
}

// ------------------------------------------------------------ the corpora

/// The draft amendment of AC-R1 (listed).
const DRAFT: &str = "docs/features/regen-delay.md";
/// The accepted decision, every link state (listed).
const ACCEPTED: &str = "docs/records/DEC/DEC-0031.md";
/// The decision without `status:` (listed).
const NO_STATUS: &str = "docs/records/DEC/DEC-0032.md";
/// No class, ID or title (listed, the four `null`).
const BARE: &str = "docs/features/bare-amendment.md";
/// The amendment only proposed: never in the tree.
const QUEUED: &str = "docs/features/queued-delay.md";

/// The listed paths of the amended copy of A, by path (byte order).
const LISTED: [&str; 4] = [BARE, DRAFT, ACCEPTED, NO_STATUS];

/// A's amended copy: the documents written into it, in this order.
fn amended_files() -> Vec<(&'static str, String)> {
    vec![
        (
            DRAFT,
            "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  amends: \
             [RULE-STAM-REGEN]\n---\n\n# Regeneration waits after a sprint\n\n## Delay \
             {#AC-01}\n\nRegeneration starts 1.5 s after the last sprint.\n"
                .to_owned(),
        ),
        (
            ACCEPTED,
            "---\nid: DEC-0031\nclass: decision\nstatus: accepted\ndate: 2026-10-01\n\
             scope: [stamina]\nlinks:\n  amends: [EDGE-STAM-ZERO, QST-031, RULE-NOPE, \
             DEC-0007]\n---\n\n# Amber light at zero stamina\n\nThe lantern turns amber \
             at zero.\n"
                .to_owned(),
        ),
        (
            NO_STATUS,
            "---\nid: DEC-0032\nclass: decision\ndate: 2026-10-02\nlinks:\n  amends: \
             [RULE-SPRINT-COST]\n---\n\n# Cellar walls slow the sprint\n\nThe cellar \
             costs more.\n"
                .to_owned(),
        ),
        (
            BARE,
            "---\nlinks:\n  amends: [MEC-SPRINT]\n---\n\nNo heading, no class.\n".to_owned(),
        ),
        (
            "docs/features/shipped-tuning.md",
            "---\nclass: spec\nstatus: shipped\nlinks:\n  amends: [RULE-STAM-REGEN]\n---\n\n\
             # Shipped tuning\n"
                .to_owned(),
        ),
        (
            "docs/features/abandoned-tuning.md",
            "---\nclass: spec\nstatus: abandoned\nlinks:\n  amends: [RULE-STAM-REGEN]\n\
             ---\n\n# Abandoned tuning\n"
                .to_owned(),
        ),
        (
            "docs/records/DEC/DEC-0033.md",
            "---\nid: DEC-0033\nclass: decision\nstatus: superseded-by DEC-0031\n\
             date: 2026-09-30\nlinks:\n  amends: [EDGE-STAM-ZERO]\n---\n\n# Red light at \
             zero stamina\n"
                .to_owned(),
        ),
        (
            "docs/records/DEC/DEC-0034.md",
            "---\nid: DEC-0034\nclass: decision\nstatus: proposed\ndate: 2026-10-03\n\
             links:\n  amends: [EDGE-STAM-ZERO]\n---\n\n# Blue light at zero stamina\n"
                .to_owned(),
        ),
        (
            "docs/spec/amendment-digest.md",
            "---\nclass: generated\nlinks:\n  amends: [RULE-STAM-REGEN]\n---\n\n# Digest\n"
                .to_owned(),
        ),
        (
            "docs/features/depends-only.md",
            "---\nclass: spec\nstatus: draft\nlinks:\n  depends_on: [RULE-STAM-REGEN]\n---\n\n\
             # Depends only\n\nSee [[RULE-STAM-REGEN]] for the delay.\n"
                .to_owned(),
        ),
    ]
}

/// The amendment raised through the queue only.
const QUEUED_TEXT: &str = "---\nclass: spec\nstatus: draft\nscope: [movement]\nlinks:\n  \
                           amends: [EDGE-STAM-ZERO]\n---\n\n# Queued delay\n\n## Delay \
                           {#AC-01}\n\nOnly proposed.\n";

/// The spec-b amendment: a legacy Cyrillic alias (`\u{0422}\u{0420}\u{0411}-001`
/// of `REQ-001`) written raw, a resolved ID, a dangling one.
const B_PATH: &str = "docs/features/sync-retry.md";
fn b_files() -> Vec<(&'static str, String)> {
    vec![(
        B_PATH,
        "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [CMD-SYNC, \
         \u{0422}\u{0420}\u{0411}-001, CMD-NOPE]\n---\n\n# Sync retries\n"
            .to_owned(),
    )]
}

/// The spec-c amendment (a non-Rust project's own prefixes).
const C_PATH: &str = "docs/features/cap-notice.md";
fn c_files() -> Vec<(&'static str, String)> {
    vec![(
        C_PATH,
        "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [FEE-DAY-CAP, OPEN-01]\n---\n\n\
         # Cap notice\n"
            .to_owned(),
    )]
}

/// A git copy of `fixture` at `<scratch>/<dir>` with `files` written in
/// order, everything committed once; its sandbox.
fn git_copy(
    scratch: &Scratch,
    fixture: &str,
    dir: &str,
    files: &[(&str, String)],
) -> (PathBuf, Sandbox) {
    let root = scratch.copy(fixture, dir);
    for (path, text) in files {
        write(&root, path, text);
    }
    let git = Sandbox::new(scratch.path());
    git.init(&root);
    git.add_all(&root);
    git.commit(&root, "the fixture and its amendments");
    (root, git)
}

/// A pair on spec-a holding the amended files committed and [`QUEUED`]
/// proposed from its linked worktree (stored, never applied).
fn amended_pair(label: &str) -> Pair {
    let pair = Pair::new(label, "spec-a");
    let run = pair.spec_piped(
        &pair.linked,
        &[
            "propose",
            "amendment",
            QUEUED,
            "--text-file",
            "-",
            "--rationale",
            "Only proposed.",
            "--author-role",
            "spec-author",
        ],
        QUEUED_TEXT.as_bytes(),
    );
    run.code(0);
    assert!(run.stdout.starts_with("PR-0001\n"), "{}", run.show());
    for (path, text) in amended_files() {
        write(&pair.main, path, text);
    }
    pair.commit_all(&pair.main, "amendments written");
    pair
}

// ------------------------------------------------------------ runs

/// `spec health [--json]` in `root` under `home` (nothing else set).
fn health_run(home: &Path, root: &Path, json: bool) -> Run {
    let args: &[&str] = if json {
        &["health", "--json"]
    } else {
        &["health"]
    };
    spec(home, root, args)
}

/// `spec health --json`, exit 0: its document, ordered.
fn health_json(home: &Path, root: &Path) -> (Run, J) {
    let run = health_run(home, root, true);
    run.code(0);
    let body = run.stdout.strip_suffix('\n').expect("one final LF");
    assert!(!body.contains('\n'), "one line: {}", run.show());
    let document = J::parse(body);
    (run, document)
}

/// `spec check --json`'s `counts.documents` in `root`.
fn check_documents(home: &Path, root: &Path) -> usize {
    let run = spec(home, root, &["check", "--json"]);
    assert!([0, 1].contains(&run.code), "spec check: {}", run.show());
    J::parse(run.stdout.trim_end())
        .get("counts")
        .get("documents")
        .number()
}

/// `spec show <path> --links --json`'s `nodes[0].links.outgoing` entries
/// of type `amends`, in order (`home` takes the index it refreshes).
fn shown_amends(home: &Path, root: &Path, path: &str) -> Vec<J> {
    let run = spec(home, root, &["show", path, "--links", "--json"]);
    run.code(0);
    let document = J::parse(run.stdout.trim_end());
    document.get("nodes").items()[0]
        .get("links")
        .get("outgoing")
        .items()
        .iter()
        .filter(|link| link.get("type").text() == Some(AMENDS))
        .cloned()
        .collect()
}

/// `spec show <path> --links`'s `out amends` lines.
fn shown_amends_lines(home: &Path, root: &Path, path: &str) -> Vec<String> {
    let run = spec(home, root, &["show", path, "--links"]);
    run.code(0);
    let prefix = format!("  out {AMENDS} ");
    run.stdout
        .lines()
        .filter(|line| line.starts_with(&prefix))
        .map(str::to_owned)
        .collect()
}

/// The listed paths of a health document.
fn listed(document: &J) -> Vec<String> {
    document
        .get("amendments")
        .items()
        .iter()
        .map(|row| row.get("path").text().expect("a path").to_owned())
        .collect()
}

fn row<'a>(document: &'a J, path: &str) -> &'a J {
    document
        .get("amendments")
        .items()
        .iter()
        .find(|row| row.get("path").text() == Some(path))
        .unwrap_or_else(|| panic!("{path} not listed: {document:?}"))
}

// ------------------------------------------------------------ AC-R1

/// AC-R1: the live documents that name a node they amend are listed,
/// whatever their class, status or links' states; Tier 3 by status,
/// `class: generated`, a document with only other link types and an
/// amendment only proposed are not. M: `is_live` -> `!is_tier3_file`
/// (the generated one listed); the live filter removed (the shipped one
/// listed); any link type taken (`depends-only.md` listed).
#[test]
fn ac_r1_live_amendments_are_listed_and_nothing_else() {
    let pair = amended_pair("health-r1");
    // The proposal is stored, its file nowhere in the tree.
    assert_eq!(pair.proposals().len(), 1);
    assert!(!pair.main.join(QUEUED).exists() && !pair.linked.join(QUEUED).exists());
    let fresh = pair.scratch.home("fresh");
    let (_, document) = health_json(&fresh, &pair.main);
    assert_eq!(listed(&document), LISTED, "{document:?}");
    // The linked worktree, the fixture as committed before: none.
    let (_, linked) = health_json(&fresh, &pair.linked);
    assert_eq!(listed(&linked), Vec::<String>::new(), "{linked:?}");
    // Each listed row's fields as written.
    let draft = row(&document, DRAFT);
    assert_eq!(draft.get("id").text(), None);
    assert_eq!(
        draft.get("title").text(),
        Some("Regeneration waits after a sprint")
    );
    assert_eq!(draft.get("class").text(), Some("spec"));
    assert_eq!(draft.get("status").text(), Some("draft"));
    let accepted = row(&document, ACCEPTED);
    assert_eq!(accepted.get("id").text(), Some("DEC-0031"));
    assert_eq!(accepted.get("status").text(), Some("accepted"));
    let no_status = row(&document, NO_STATUS);
    assert_eq!(no_status.get("id").text(), Some("DEC-0032"));
    assert_eq!(no_status.get("class").text(), Some("decision"));
    assert_eq!(no_status.get("status"), &J::Null, "absent: null");
    let bare = row(&document, BARE);
    for key in ["id", "title", "class", "status"] {
        assert_eq!(bare.get(key), &J::Null, "{BARE}: {key} absent");
    }
    // Every listed row holds at least one link.
    for path in LISTED {
        assert!(
            !row(&document, path).get("links").items().is_empty(),
            "{path}: never empty"
        );
    }
}

/// AC-R1 on B and C: their own amendment listed, nothing else (B's broken
/// `QN-08` a note, not a row).
#[test]
fn ac_r1_spec_b_and_spec_c_list_their_own_amendment() {
    let scratch = Scratch::new("health-r1-bc");
    let home = scratch.home("fresh");
    for (fixture, files, path) in [("spec-b", b_files(), B_PATH), ("spec-c", c_files(), C_PATH)] {
        let (plain, _) = git_copy(&scratch, fixture, &format!("{fixture}-plain"), &[]);
        let (_, none) = health_json(&home, &plain);
        assert_eq!(listed(&none), Vec::<String>::new(), "{fixture}: {none:?}");
        let (root, _) = git_copy(&scratch, fixture, fixture, &files);
        let (_, document) = health_json(&home, &root);
        assert_eq!(listed(&document), [path], "{fixture}: {document:?}");
    }
    assert!(
        fs::read_dir(&home).expect("home").next().is_none(),
        "the fresh HOME stays empty"
    );
}

// ------------------------------------------------------------ AC-R2

/// AC-R2: `links` equals, as JSON and in order, the `amends` entries of
/// `spec show <path> --links --json` `nodes[0].links.outgoing`: a resolved
/// ID, a hand-written alias (`written` raw), a dangling ID, a node in a
/// Tier 3 file; on B a legacy Cyrillic alias written raw; on C its own
/// prefixes. The text's out lines are `spec show --links`'s. M: unresolved
/// links dropped; `written` canonicalised.
#[test]
fn ac_r2_links_are_show_links_amends_entries_every_state() {
    let pair = amended_pair("health-r2");
    let fresh = pair.scratch.home("fresh");
    let shows = pair.scratch.home("shows");
    let (_, document) = health_json(&fresh, &pair.main);
    let text = health_run(&fresh, &pair.main, false);
    text.code(0);
    for path in LISTED {
        let links = row(&document, path).get("links").items().to_vec();
        assert_eq!(
            links,
            shown_amends(&shows, &pair.main, path),
            "{path}: `links` = show's amends entries, in order"
        );
        let lines = shown_amends_lines(&shows, &pair.main, path);
        assert!(!lines.is_empty(), "{path}");
        let block: String = lines.iter().map(|line| format!("{line}\n")).collect();
        assert!(
            text.stdout.contains(&block),
            "{path}: the text prints show's out lines:\n{block}\n{}",
            text.stdout
        );
    }
    // The accepted decision's four: each state, the alias raw.
    let links = row(&document, ACCEPTED).get("links").items().to_vec();
    let seen: Vec<(Option<&str>, Option<&str>, Option<&str>)> = links
        .iter()
        .map(|link| {
            (
                link.get("name").text(),
                link.get("written").text(),
                link.get("state").text(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                Some("EDGE-STAM-ZERO"),
                Some("EDGE-STAM-ZERO"),
                Some("resolved")
            ),
            (Some("Q-031"), Some("QST-031"), Some("resolved")),
            (None, Some("RULE-NOPE"), Some("dangling")),
            (Some("DEC-0007"), Some("DEC-0007"), Some("resolved")),
        ],
        "{links:?}"
    );
    assert!(
        links[2].get("reason").text().is_some(),
        "a dangling link says why"
    );
    assert!(
        text.stdout
            .contains(&format!("  out {AMENDS} Q-031 | {ACCEPTED}:")),
        "{}",
        text.stdout
    );

    // B and C.
    for (fixture, files, path, written) in [
        (
            "spec-b",
            b_files(),
            B_PATH,
            vec!["CMD-SYNC", "\u{0422}\u{0420}\u{0411}-001", "CMD-NOPE"],
        ),
        ("spec-c", c_files(), C_PATH, vec!["FEE-DAY-CAP", "OPEN-01"]),
    ] {
        let (root, _) = git_copy(&pair.scratch, fixture, fixture, &files);
        let (_, document) = health_json(&fresh, &root);
        let links = row(&document, path).get("links").items().to_vec();
        assert_eq!(links, shown_amends(&shows, &root, path), "{fixture}");
        let raw: Vec<Option<&str>> = links
            .iter()
            .map(|link| link.get("written").text())
            .collect();
        let want: Vec<Option<&str>> = written.iter().map(|text| Some(*text)).collect();
        assert_eq!(raw, want, "{fixture}: written raw, in order");
        if fixture == "spec-b" {
            // The legacy alias resolves to its canonical ID, written raw.
            assert_eq!(links[1].get("name").text(), Some("REQ-001"), "{links:?}");
            assert_eq!(links[1].get("state").text(), Some("resolved"));
            assert_eq!(links[2].get("state").text(), Some("dangling"));
        }
    }
}

// ------------------------------------------------------------ AC-R3

/// AC-R3: exit 0 with none and several; the keys exactly and in order as
/// "Data", absent `null`, empty `[]`; `documents` = `spec check --json`
/// `counts.documents` on A, the amended A, B and C; the library's
/// document and text are the binary's; the text of "Data", its last line
/// with zero too. M: `notes` omitted when empty; rows sorted by title.
#[test]
fn ac_r3_the_document_keys_counts_order_and_text() {
    let pair = amended_pair("health-r3");
    let fresh = pair.scratch.home("fresh");
    let checks = pair.scratch.home("checks");
    let (plain, _) = git_copy(&pair.scratch, "spec-a", "plain", &[]);
    let (b, _) = git_copy(&pair.scratch, "spec-b", "b", &[]);
    let (c, _) = git_copy(&pair.scratch, "spec-c", "c", &[]);
    for (root, rows, documents) in [
        (&plain, 0, 13),
        (&pair.main, 4, 23),
        (&b, 0, 11),
        (&c, 0, 5),
    ] {
        let (run, document) = health_json(&fresh, root);
        let context = root.display().to_string();
        assert_eq!(
            document.keys(),
            ["documents", "amendments", "notes"],
            "{context}"
        );
        assert_eq!(document.get("documents").number(), documents, "{context}");
        assert_eq!(
            document.get("documents").number(),
            check_documents(&checks, root),
            "{context}: documents = spec check's counts.documents"
        );
        assert_eq!(document.get("amendments").items().len(), rows, "{context}");
        for row in document.get("amendments").items() {
            assert_eq!(
                row.keys(),
                ["path", "id", "title", "class", "status", "links"],
                "{context}"
            );
            for link in row.get("links").items() {
                assert_eq!(
                    link.keys(),
                    [
                        "type", "origin", "at", "name", "written", "path", "line", "state",
                        "reason"
                    ],
                    "{context}"
                );
            }
        }
        // Empty `[]`, never absent: `notes` on A, `amendments` on A plain.
        if root == &plain {
            assert_eq!(document.get("amendments"), &J::Arr(Vec::new()));
            assert_eq!(document.get("notes"), &J::Arr(Vec::new()));
            assert_eq!(
                run.stdout, "{\"documents\":13,\"amendments\":[],\"notes\":[]}\n",
                "the whole document with zero"
            );
        }
        // The library's document is the binary's.
        let outcome = health(
            &Env {
                cwd: root.clone(),
                home: Some(fresh.clone().into_os_string()),
                xdg_data_home: None,
            },
            &Globals::default(),
        )
        .expect("the library answers");
        let outcome = Outcome::Health(outcome);
        assert_eq!(render_json(&outcome), run.stdout, "{context}");
        let text = health_run(&fresh, root, false);
        text.code(0);
        assert_eq!(render_text(&outcome), text.stdout, "{context}");
        let last = text.stdout.lines().last().expect("a last line");
        assert_eq!(
            last,
            format!("spec health: {documents} documents, {rows} unapplied amendments"),
            "{context}: the count line, always"
        );
        if rows == 0 {
            assert_eq!(text.stdout, format!("{last}\n"), "{context}: zero rows");
        }
    }
    // The amended A's text: the header lines as "Data" (absent parts
    // omitted), by path, not by title.
    let text = health_run(&fresh, &pair.main, false);
    let headers: Vec<&str> = text
        .stdout
        .lines()
        .filter(|line| !line.starts_with("  ") && !line.starts_with("spec health: "))
        .collect();
    assert_eq!(
        headers,
        [
            BARE.to_owned(),
            format!("{DRAFT} | class spec | status draft | Regeneration waits after a sprint"),
            format!(
                "{ACCEPTED} | id DEC-0031 | class decision | status accepted | Amber light at \
                 zero stamina"
            ),
            format!("{NO_STATUS} | id DEC-0032 | class decision | Cellar walls slow the sprint"),
        ],
        "{}",
        text.stdout
    );
    assert!(text.stderr.is_empty(), "{}", text.show());
    // Sorted by path, not by title (the titles sort otherwise).
    let (_, document) = health_json(&fresh, &pair.main);
    let titles: Vec<Option<&str>> = document
        .get("amendments")
        .items()
        .iter()
        .map(|row| row.get("title").text())
        .collect();
    let mut by_title = titles.clone();
    by_title.sort();
    assert_ne!(titles, by_title, "the fixture tells the two orders apart");
}

/// AC-R3 "uncut": 250 amendments, every row in the JSON and the text, no
/// tail, exit 0; by path.
#[test]
fn ac_r3_many_amendments_are_listed_uncut_by_path() {
    let scratch = Scratch::new("health-r3-uncut");
    let home = scratch.home("fresh");
    let files: Vec<(String, String)> = (0..250)
        .rev()
        .map(|i| {
            (
                format!("docs/features/amend-{i:03}.md"),
                format!(
                    "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [RULE-STAM-REGEN, \
                     EDGE-STAM-ZERO]\n---\n\n# Amendment {}\n",
                    250 - i
                ),
            )
        })
        .collect();
    let borrowed: Vec<(&str, String)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.clone()))
        .collect();
    let (root, _) = git_copy(&scratch, "spec-a", "a", &borrowed);
    let (run, document) = health_json(&home, &root);
    let paths = listed(&document);
    let want: Vec<String> = (0..250)
        .map(|i| format!("docs/features/amend-{i:03}.md"))
        .collect();
    assert_eq!(paths, want, "every row, by path");
    assert!(run.stdout.len() > 100_000, "{} bytes", run.stdout.len());
    assert!(run.stderr.is_empty(), "{}", run.show());
    let text = health_run(&home, &root, false);
    text.code(0);
    assert_eq!(
        text.stdout
            .lines()
            .filter(|line| line.starts_with("docs/"))
            .count(),
        250
    );
    assert_eq!(
        text.stdout
            .lines()
            .filter(|line| line.starts_with("  out "))
            .count(),
        500
    );
    assert_eq!(
        text.stdout.lines().last(),
        Some("spec health: 263 documents, 250 unapplied amendments")
    );
}

// ------------------------------------------------------------ AC-R4

/// The `<why>` of `spec export index`'s walk-gap refusal on `root`, its
/// config registering an index generator for the occasion (a copy of the
/// config, restored after).
fn export_gap(home: &Path, root: &Path) -> String {
    let config = read_text(root, "specengine.toml");
    write(
        root,
        "specengine.toml",
        registered(&config, "docs/spec/index.md", "gen-index", None),
    );
    let run = spec(home, root, &["export", "index", "--stdout"]);
    write(root, "specengine.toml", &config);
    run.code(2);
    let line = run.stderr.trim_end();
    line.strip_prefix("spec: the walk is incomplete: ")
        .and_then(|rest| {
            rest.strip_suffix(": the index would leave documents out; nothing was written")
        })
        .unwrap_or_else(|| panic!("export's refusal: {}", run.show()))
        .to_owned()
}

/// AC-R4: an unreadable `.md` (mode 000) or a missing written root: exit
/// 2, stderr naming the first gap as `spec export index` words it, stdout
/// empty, both modes. M: the gap ignored and a list printed.
#[test]
fn ac_r4_a_walk_gap_is_exit_2_naming_it_stdout_empty() {
    let scratch = Scratch::new("health-r4-gap");
    let home = scratch.home("fresh");
    let export_home = scratch.home("export");
    // Mode 000, the amended A (a list it would otherwise print).
    let (root, _) = git_copy(&scratch, "spec-a", "a", &amended_files());
    let unreadable = root.join("docs/spec/game.md");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let why = export_gap(&export_home, &root);
    assert!(why.contains("`docs/spec/game.md`"), "{why}");
    for json in [true, false] {
        let run = health_run(&home, &root, json);
        run.code(2);
        assert_eq!(run.stdout, "", "exit 2 prints nothing: {}", run.show());
        assert_eq!(
            run.stderr,
            format!("spec: the walk is incomplete: {why}: unapplied amendments not measured\n"),
            "{}",
            run.show()
        );
    }
    // The first gap by path: a second unreadable file after it is not named.
    let later = root.join(ACCEPTED);
    fs::set_permissions(&later, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    let run = health_run(&home, &root, true);
    run.code(2);
    assert!(
        run.stderr.contains("`docs/records/DEC/DEC-0031.md`"),
        "{}",
        run.show()
    );
    fs::set_permissions(&later, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o644)).expect("chmod 644");
    health_json(&home, &root);

    // A missing written root.
    let (missing, _) = git_copy(&scratch, "spec-a", "missing", &amended_files());
    let config = read_text(&missing, "specengine.toml");
    write(
        &missing,
        "specengine.toml",
        set_paths_key(&config, "roots", "[\"docs\", \"notes\"]"),
    );
    let why = export_gap(&export_home, &missing);
    assert!(why.contains("`notes`"), "{why}");
    for json in [true, false] {
        let run = health_run(&home, &missing, json);
        run.code(2);
        assert_eq!(run.stdout, "", "{}", run.show());
        assert_eq!(
            run.stderr,
            format!("spec: the walk is incomplete: {why}: unapplied amendments not measured\n"),
        );
    }
    assert!(
        fs::read_dir(&home).expect("home").next().is_none(),
        "the fresh HOME stays empty"
    );
}

/// AC-R4: a broken front-matter is not judged: no row, one note
/// `path:line: code: message; not judged` (the check's finding), on stderr
/// as `note: …` and in `notes`, exit 0, never refused; B's `QN-08` alike;
/// two broken files, two notes by path. M: the broken file skipped without
/// a note.
#[test]
fn ac_r4_a_broken_front_matter_is_a_note_not_a_row() {
    let scratch = Scratch::new("health-r4-broken");
    let home = scratch.home("fresh");
    let checks = scratch.home("checks");
    let broken = "docs/features/broken-amendment.md";
    let mut files = amended_files();
    files.push((
        broken,
        "---\nclass: spec\nstatus: draft\ntitle: Broken: a colon\nlinks:\n  amends: \
         [RULE-STAM-REGEN]\n---\n\n# Broken\n"
            .to_owned(),
    ));
    let (root, _) = git_copy(&scratch, "spec-a", "a", &files);
    let (b, _) = git_copy(&scratch, "spec-b", "b", &[]);
    for (root, path, rows) in [(&root, broken, 4), (&b, "docs/records/QN/QN-08.md", 0)] {
        let check = spec(&checks, root, &["check", "--json"]);
        let report = J::parse(check.stdout.trim_end());
        let finding = report
            .get("findings")
            .items()
            .iter()
            .find(|finding| {
                finding.get("path").text() == Some(path)
                    && finding.get("code").text() == Some("frontmatter-yaml")
            })
            .unwrap_or_else(|| panic!("{path}: no frontmatter-yaml finding: {report:?}"))
            .clone();
        let note = format!(
            "{path}:{}: frontmatter-yaml: {}; not judged",
            finding.get("line").number(),
            finding.get("message").text().expect("a message")
        );
        let (run, document) = health_json(&home, root);
        assert!(
            !listed(&document).contains(&path.to_owned()),
            "{document:?}"
        );
        assert_eq!(document.get("amendments").items().len(), rows);
        assert_eq!(
            document.get("notes"),
            &J::Arr(vec![J::Str(note.clone())]),
            "one note"
        );
        assert_eq!(run.stderr, format!("note: {note}\n"), "{}", run.show());
        let text = health_run(&home, root, false);
        text.code(0);
        assert_eq!(text.stderr, format!("note: {note}\n"));
        assert!(!text.stdout.contains(path), "{}", text.stdout);
    }
    // Two broken files: one note each, by path whatever the write order.
    let earlier = "docs/features/another-broken.md";
    write(
        &root,
        earlier,
        "---\nclass: spec\ntitle: Also: broken\nlinks:\n  amends: [EDGE-STAM-ZERO]\n---\n",
    );
    let (run, document) = health_json(&home, &root);
    let notes: Vec<&str> = document
        .get("notes")
        .items()
        .iter()
        .map(|note| note.text().expect("a note"))
        .collect();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(
        notes[0].starts_with(&format!("{earlier}:")) && notes[1].starts_with(&format!("{broken}:")),
        "by path: {notes:?}"
    );
    assert!(
        notes.iter().all(|note| note.ends_with("; not judged")),
        "{notes:?}"
    );
    assert_eq!(listed(&document).len(), 4, "{document:?}");
    assert_eq!(
        run.stderr,
        format!("note: {}\nnote: {}\n", notes[0], notes[1])
    );
}

// ------------------------------------------------------------ AC-R5

/// AC-R5: read-only and deterministic. A fresh `HOME` holds nothing after
/// (no data directory, no `<slug>.db`); the pair's `HOME` (the queue) is
/// byte-identical after, its events unchanged; `git status --porcelain`
/// empty; two runs, and two copies written in opposite orders elsewhere,
/// give byte-identical stdout (JSON and text). M: an index refresh; a
/// `HashMap` order.
#[test]
fn ac_r5_read_only_and_byte_identical() {
    let pair = amended_pair("health-r5");
    // Forty more amendments: an unordered map would show.
    let more: Vec<(String, String)> = (0..40)
        .map(|i| {
            (
                format!("docs/features/more-{i:02}.md"),
                format!(
                    "---\nclass: spec\nstatus: draft\nlinks:\n  amends: [EDGE-STAM-ZERO]\n---\n\n\
                     # More {i}\n"
                ),
            )
        })
        .collect();
    for (path, text) in &more {
        write(&pair.main, path, text);
    }
    pair.commit_all(&pair.main, "forty more");
    let fresh = pair.scratch.home("fresh");
    let events = pair.events();
    assert_eq!(events.len(), 1, "the proposal's creation: {events:?}");
    let state = pair.state();
    // Taken last: nothing opens the queue until it is compared.
    let queue_home = snapshot(&pair.home);

    let first = (
        health_run(&fresh, &pair.main, true),
        health_run(&fresh, &pair.main, false),
    );
    first.0.code(0);
    first.1.code(0);
    let queue_run = pair.spec_piped(&pair.main, &["health", "--json"], b"");
    queue_run.code(0);
    assert_eq!(queue_run.stdout, first.0.stdout, "whatever the HOME");
    for _ in 0..2 {
        assert_eq!(health_run(&fresh, &pair.main, true).stdout, first.0.stdout);
        assert_eq!(health_run(&fresh, &pair.main, false).stdout, first.1.stdout);
    }
    let (_, document) = health_json(&fresh, &pair.main);
    let paths = listed(&document);
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "by path, byte order");
    assert_eq!(paths.len(), 44);

    // Read-only.
    assert!(
        fs::read_dir(&fresh).expect("home").next().is_none(),
        "the fresh HOME holds nothing: {:?}",
        snapshot(&fresh).keys().collect::<Vec<_>>()
    );
    assert_eq!(
        snapshot(&pair.home),
        queue_home,
        "the queue's HOME byte-identical"
    );
    assert_eq!(pair.events(), events, "no event");
    assert_eq!(
        pair.state(),
        state,
        "files, refs, status, proposals unchanged"
    );
    assert_eq!(
        pair.git.git_text(&pair.main, &["status", "--porcelain"]),
        "",
        "git status --porcelain empty"
    );

    // Two copies elsewhere, the files written in opposite orders.
    let mut all: Vec<(String, String)> = amended_files()
        .into_iter()
        .map(|(path, text)| (path.to_owned(), text))
        .collect();
    all.extend(more);
    let forward = pair.scratch.copy("spec-a", "elsewhere/forward");
    for (path, text) in &all {
        write(&forward, path, text);
    }
    let backward = pair.scratch.copy("spec-a", "other/place/backward");
    for (path, text) in all.iter().rev() {
        write(&backward, path, text);
    }
    for (root, name) in [(&forward, "forward"), (&backward, "backward")] {
        let home = pair.scratch.home(name);
        assert_eq!(
            health_run(&home, root, true).stdout,
            first.0.stdout,
            "{}",
            root.display()
        );
        assert_eq!(
            health_run(&home, root, false).stdout,
            first.1.stdout,
            "{}",
            root.display()
        );
        assert!(fs::read_dir(&home).expect("home").next().is_none());
    }
}

// ------------------------------------------------------------ AC-R7

/// Every string literal of a Rust line outside comments (naive: between
/// pairs of `"`), as `proposal_genre.rs` takes them.
fn literals(line: &str) -> Vec<&str> {
    if line.trim_start().starts_with("//") {
        return Vec::new();
    }
    line.split('"').skip(1).step_by(2).collect()
}

/// Every `kind = "…"` of the fixtures' configs.
fn fixture_kinds() -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    for fixture in ["spec-a", "spec-b", "spec-c"] {
        let config = read_text(&common::fixture(fixture), "specengine.toml");
        for part in config.split("kind").skip(1) {
            if let Some(value) = part
                .trim_start()
                .strip_prefix('=')
                .and_then(|rest| rest.trim_start().strip_prefix('"'))
                .and_then(|rest| rest.split('"').next())
            {
                kinds.insert(value.to_owned());
            }
        }
    }
    kinds
}

/// AC-R7, the scan half (`proposal_genre.rs` holds the `"amends"` scan of
/// every CLI source): the slice's new sources, core `check/unapplied.rs`
/// and CLI `health.rs`, write no class, status, kind or link-type literal;
/// core names `AMENDS` only through `amended_refs` (no `AMENDS` in its
/// code). M: an `"amends"` literal in the new core or CLI code.
#[test]
fn ac_r7_the_new_sources_write_no_class_status_kind_or_link_type() {
    let mut words: BTreeSet<String> = fixture_kinds();
    assert!(
        words.contains("mechanic") && words.contains("tariff"),
        "{words:?}"
    );
    words.extend(LINK_TYPES.iter().map(|name| (*name).to_owned()));
    for word in [
        "canon",
        "decision",
        "spec",
        "generated",
        "draft",
        "accepted",
        "shipped",
        "abandoned",
        "proposed",
        "superseded-by",
        "open",
        "amendment",
    ] {
        words.insert(word.to_owned());
    }
    let mut found = Vec::new();
    for source in [
        "crates/specengine-core/src/check/unapplied.rs",
        "crates/specengine-cli/src/health.rs",
    ] {
        let text = read_text(&repository_root(), source);
        for (number, line) in text.lines().enumerate() {
            for literal in literals(line) {
                if words.contains(literal) {
                    found.push(format!("{source}:{}: {literal:?}", number + 1));
                }
            }
            let code = line.split("//").next().unwrap_or_default();
            if source.contains("unapplied") && code.contains("AMENDS") {
                found.push(format!(
                    "{source}:{}: AMENDS outside amended_refs",
                    number + 1
                ));
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
    // The scan sees a literal and skips a comment.
    assert_eq!(literals("    if class == \"spec\" {"), ["spec"]);
    assert!(literals("    // \"amends\"").is_empty());
}
