//! docs/features/reference-promotion.md (RM-0138, ADR-0089) "Acceptance
//! criteria", the CLI's half: the `spec` binary over scratch git
//! repositories of `fixtures/promote-mini` (homes `media`, given to LFS by
//! its `.gitattributes`, and `art`, plain), each with its own `HOME`, so its
//! own data directory and blob store; `spec approve` on a pseudo-terminal;
//! git-lfs on `PATH` only where a test says so ([`Lfs::On`]: the real
//! git-lfs linked alone into a scratch directory; D18: a missing git-lfs
//! fails the test naming it, none skips).
//!
//! - PR1: `spec propose reference RF-0001 --to LOC-HARBOUR` stores PR-0001,
//!   kind `reference`, `ref_id` `RF-0001`, `new_text` `media/harbour.png`,
//!   the target, base and patch hash of "Queue schema 17"; `git status`
//!   empty, `HEAD` unmoved, one `proposal.created`. M: the route writing the
//!   file at propose (its CLI twin: the propose writing it).
//! - PR2: approving a promotion to CHR-MIRA (plain, no git-lfs needed) is one
//!   commit, one parent, `M` the document and `A` the file, trailers through
//!   `Reference: RF-0002` after `Base-commit:`; the file's BLAKE3 the
//!   reference's, its blob `git hash-object`'s; the document gains the key
//!   and every other byte stays. Another file at the path after propose:
//!   exit 1 `holds other bytes`, nothing written, one `proposal.apply_failed`;
//!   equal bytes committed by hand: `M` alone, `is in place`; the reference
//!   deleted, or its file gone from the store: exit 1, no file, no `.tmp`. M:
//!   the occupancy check skipped.
//! - PR3: with git-lfs, applying PR-0001 commits a v1 pointer (`oid` the
//!   bytes' SHA-256, `size` theirs, under 200 B), never the bytes; `git lfs
//!   ls-files` lists it; `git lfs fsck --objects` is clean; the worktree holds
//!   the pointer, `git status` clean. git-lfs off `PATH`: a note at propose,
//!   exit 2 before the question at apply, nothing written. As built (D7's
//!   deviation: the apply writes the object and the pointer itself), git-lfs
//!   is asked only `version`, and one older than 3.0 exits 2. A 51 MiB WAV for a
//!   plain home, a `filter=crypt` home, a `text` home: propose exit 1 with
//!   their lines. M: the raw blob committed.
//! - PR4: `--to MEC-TIDE` exits 2 naming the kind and the fix; an ID-less
//!   document exits 2 `has no kind`; no row, no event. A home with an
//!   undeclared kind, or a kind in two homes: the reference writes exit 2 at
//!   its line. M: a fallback to the document's own directory.
//! - PR5: on LOC-HARBOUR locked, the apply leaves `norm_hash` and `rev`, its
//!   question shows no `raises`, `spec verify` reads `ok`; `spec check
//!   --staged` over a hand-added entry gives no `rev-not-raised`; bad
//!   entries are one `reference-entry` each, 32 kept, `norm_hash` non-null; a
//!   scalar key is `frontmatter-type`. M: `references` in the norm recipe.
//! - PR6: after PR-0001 a card on LOC-HARBOUR and one on its section print
//!   the spec reference, a card on CHR-MIRA `-`; `U` grows by the lines (the
//!   bundle may change: the developer's deviation 4); a task on the section carries `spec_references` and
//!   "In the spec"; `spec ref list --on` carries the `in_spec` row; 25
//!   entries list 20 and the note. M: a section target not resolved to its
//!   document.
//! - PR11, the CLI's half: a `reference` row dumps as format 10, `ref_id`
//!   last, imports into an empty data directory and dumps again
//!   byte-identical.
//! - PR12: a format-21 index is read again at the first write; `promoted`
//!   holds each document's kept entries, root-relative; an edited key
//!   replaces its rows, a deleted file drops them. M: `update` skipping
//!   `promoted` for a changed file.
//!
//! The forms of PR14 and PR15 and PR7's `[auto]` are
//! `tests/reference_promotion_forms.rs`; the daemon's half (the route, the
//! cover, memory, the sync) `specengine-http`'s `tests/reference_promotion.rs`;
//! MCP's its crate's. Every expected value is worked out from the spec and
//! docs/canon/work-references-promotion.md, never read back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
#[path = "promote_common/mod.rs"]
mod promote_common;
#[path = "work_refs_common/mod.rs"]
mod refs_common;

use std::fs;
use std::path::Path;

use common::data_dir;
use promote_common::lfs::{self, Pointer, Stub, size_text};
use promote_common::{
    ALMANAC_MD, HARBOUR_HOME, HARBOUR_MD, IN_THE_SPEC, Lfs, MIRA_HOME, MIRA_MD, SLUG, Scene, b3,
    before_fence, id_of,
};
use refs_common::{SeekWav, blob_fixture, keys_in_order, png};
use serde_json::{Value, json};

const MIB: u64 = 1024 * 1024;
const HARBOUR_PNG: &str = "docs/spec/world/media/harbour.png";
const MIRA_PNG: &str = "docs/spec/cast/art/mira.png";

// ------------------------------------------------------------- the scene

/// C-0001 on LOC-HARBOUR with `fixtures/blob-types/harbour.png` attached
/// (RF-0001), C-0002 on CHR-MIRA with a fresh `mira.png` (RF-0002): the
/// scene and the two files' bytes.
fn two_refs(label: &str) -> (Scene, Vec<u8>, Vec<u8>) {
    let scene = Scene::new(label);
    assert_eq!(scene.card("Light the harbour", &["LOC-HARBOUR"]), "C-0001");
    assert_eq!(scene.card("Draw Mira", &["CHR-MIRA"]), "C-0002");
    let harbour = fs::read(blob_fixture("harbour.png")).expect("the fixture");
    let rf1 = scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]);
    assert_eq!(id_of(&rf1), "RF-0001");
    let mira = png(64, 48, 0x5a, None);
    let file = scene.scratch_file("mira.png", &mira);
    let rf2 = scene.file_ref("C-0002", &file, &[]);
    assert_eq!(id_of(&rf2), "RF-0002");
    (scene, harbour, mira)
}

// ------------------------------------------------------------------ PR1

/// PR1: a promotion is a proposal and nothing else: the row of "Queue schema
/// 17", the repository untouched, one `proposal.created`. M: the propose
/// writing the file (the route's twin).
#[test]
fn pr1_a_promotion_is_only_a_proposal() {
    let (scene, _, _) = two_refs("pr-pr1");
    let head = scene.head();
    let status = scene.status();
    assert_eq!(status, "", "premise: a clean worktree");
    let (span_hash, span_text) = scene.pair.span(&scene.pair.main, "LOC-HARBOUR");
    let events = scene.event_types().len();

    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    assert_eq!(id, "PR-0001");
    assert_eq!(scene.status(), "", "M: nothing written under the root");
    assert_eq!(scene.head(), head, "HEAD unmoved");
    assert!(scene.file(HARBOUR_PNG).is_none(), "no file at the home");
    assert!(
        !scene.pair.main.join(HARBOUR_HOME).exists(),
        "no home directory made"
    );
    assert_eq!(
        scene.event_types()[events..].to_vec(),
        vec!["proposal.created".to_owned()],
        "one event"
    );

    assert_eq!(scene.column(&id, "kind"), "reference");
    assert_eq!(scene.column(&id, "status"), "open");
    assert_eq!(scene.column(&id, "ref_id"), "RF-0001");
    assert_eq!(scene.column(&id, "new_text"), "media/harbour.png");
    assert_eq!(scene.column(&id, "target_id"), "LOC-HARBOUR");
    assert_eq!(scene.column(&id, "target_path"), HARBOUR_MD);
    assert_eq!(scene.column(&id, "target_ids"), "[\"LOC-HARBOUR\"]");
    assert_eq!(scene.column(&id, "base_hash"), span_hash);
    assert_eq!(
        scene.column(&id, "base_text"),
        span_text.trim_end_matches('\n')
    );
    let patch = specengine_store::b3_hash(
        format!("LOC-HARBOUR\n{span_hash}\nmedia/harbour.png").as_bytes(),
    );
    assert_eq!(
        scene.column(&id, "patch_hash"),
        patch,
        "b3(target LF base LF text)"
    );
    for column in ["comment_id", "record_id", "interpretation", "card_id"] {
        assert_eq!(
            scene.column(&id, column),
            "",
            "{column} NULL on a reference"
        );
    }

    // The review document names the reference after `comment_id`, and its
    // note what the apply adds.
    let reviewed = scene.json_run(Lfs::On, &["review", &id]);
    reviewed.code(0);
    let review = reviewed.json();
    assert_eq!(review["kind"], json!("reference"));
    assert_eq!(review["ref_id"], json!("RF-0001"));
    // The key order as written (`serde_json::Value` sorts its keys).
    let keys = keys_in_order(&reviewed.stdout);
    let comment = keys.iter().position(|key| *key == "comment_id");
    let reference = keys.iter().position(|key| *key == "ref_id");
    assert!(
        matches!((comment, reference), (Some(c), Some(r)) if r == c + 1),
        "`ref_id` right after `comment_id`: {keys:?}"
    );
    let size = fs::metadata(blob_fixture("harbour.png"))
        .expect("the fixture")
        .len();
    let note = format!(
        "adds {HARBOUR_PNG}: RF-0001, image/png {}, LFS",
        size_text(size)
    );
    assert!(
        review["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|n| n.as_str().is_some_and(|n| n.starts_with(&note))),
        "{note} among {}",
        review["notes"]
    );
    // The Inbox's line.
    let inbox = scene.spec(Lfs::Off, &["inbox"]);
    inbox.code(0);
    assert!(
        inbox
            .stdout
            .lines()
            .any(|line| line.starts_with(&format!("{id} | reference | open | LOC-HARBOUR | "))),
        "{}",
        inbox.stdout
    );
}

/// The body of `fn <name>(` in `text` (its signature to its closing
/// brace, braces counted outside strings and comments); `None`: no such fn.
fn fn_body<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = [format!("fn {name}("), format!("fn {name}<")]
        .iter()
        .filter_map(|head| text.find(head.as_str()))
        .min()?;
    let open = start + text[start..].find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let bytes = text.as_bytes();
    let mut at = open;
    while at < bytes.len() {
        let c = bytes[at];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            in_string = true;
        } else if c == b'\'' && bytes.get(at + 2) == Some(&b'\'') {
            // A character literal (`'{'`); a lifetime has no closing quote.
            at += 3;
            continue;
        } else if c == b'/' && bytes.get(at + 1) == Some(&b'/') {
            at += text[at..].find('\n').unwrap_or(text.len() - at);
            continue;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(&text[start..=at]);
            }
        }
        at += 1;
    }
    None
}

/// Every `.rs` source of `crates/*/src`, root-relative, with its text.
fn sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).expect("a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    let root = common::repository_root();
    let mut files = Vec::new();
    for krate in fs::read_dir(root.join("crates")).expect("crates").flatten() {
        let src = krate.path().join("src");
        if src.is_dir() {
            walk(&src, &mut files);
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path).expect("a source");
            (
                path.strip_prefix(&root)
                    .expect("under the root")
                    .to_string_lossy()
                    .into_owned(),
                text,
            )
        })
        .collect()
}

/// PR1, the source scan: a reference's bytes reach a root only through the
/// apply. The one function that copies a stored file into a worktree
/// (`promote.rs`'s `copy_in`) is called from `approve_reference` alone;
/// `approve_reference` from `apply.rs`'s `run_approve_as` alone, which
/// `approve_with` reaches through `run_approve`; no propose path, and no
/// source of the daemon or the MCP server, names either. M: the route (or
/// the propose) writing the file at propose.
#[test]
fn pr1_only_the_apply_writes_a_reference_under_a_root() {
    let sources = sources();
    let calls = |name: &str| -> Vec<(String, usize)> {
        let call = format!("{name}(");
        let head = format!("fn {name}(");
        sources
            .iter()
            .flat_map(|(path, text)| {
                text.lines()
                    .enumerate()
                    .filter(|(_, line)| {
                        let code = line.trim_start();
                        !code.starts_with("//") && line.contains(&call) && !line.contains(&head)
                    })
                    .map(|(at, _)| (path.clone(), at + 1))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let promote = &sources
        .iter()
        .find(|(path, _)| path == "crates/specengine-cli/src/promote.rs")
        .expect("the CLI's promote.rs")
        .1;
    let apply = &sources
        .iter()
        .find(|(path, _)| path == "crates/specengine-cli/src/apply.rs")
        .expect("the CLI's apply.rs")
        .1;
    // The copy: called in `approve_reference` alone.
    let copies = calls("copy_in");
    assert_eq!(copies.len(), 1, "one call of copy_in: {copies:?}");
    assert_eq!(copies[0].0, "crates/specengine-cli/src/promote.rs");
    let approve_reference = fn_body(promote, "approve_reference").expect("approve_reference");
    assert!(
        approve_reference.contains("copy_in("),
        "copy_in in approve_reference"
    );
    // The propose paths never copy.
    for name in [
        "propose_reference",
        "promote_reference",
        "run_propose",
        "propose_reference_brief",
    ] {
        let body = fn_body(promote, name).unwrap_or_else(|| panic!("{name} in promote.rs"));
        assert!(!body.contains("copy_in("), "M: {name} copies the file");
        assert!(!body.contains("approve_reference("), "{name} applies");
    }
    // approve_reference: from run_approve_as alone, which approve_with
    // reaches through run_approve.
    let callers = calls("approve_reference");
    assert_eq!(
        callers
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        ["crates/specengine-cli/src/apply.rs"],
        "{callers:?}"
    );
    let run_as = fn_body(apply, "run_approve_as").expect("run_approve_as");
    assert!(run_as.contains("approve_reference("), "in run_approve_as");
    let run = fn_body(apply, "run_approve").expect("run_approve");
    assert!(run.contains("run_approve_as("), "run_approve reaches it");
    let with = fn_body(apply, "approve_with").expect("approve_with");
    assert!(with.contains("run_approve("), "approve_with reaches it");
    // The daemon and the MCP server name neither.
    for (path, text) in &sources {
        if path.starts_with("crates/specengine-http/") || path.starts_with("crates/specengine-mcp/")
        {
            for name in ["copy_in(", "approve_reference("] {
                assert!(!text.contains(name), "{path} names {name}");
            }
        }
    }
    assert!(fn_body("fn a() { let s = \"}\"; }\nfn b() {}", "a").is_some_and(|b| b.ends_with("}")));
}

/// PR1: the review document of a promotion repeats byte for byte; a
/// promotion needs no `--dry-run` (clap refuses it, exit 2) and no
/// `--base`.
#[test]
fn pr1_no_dry_run_and_the_review_repeats() {
    let (scene, _, _) = two_refs("pr-pr1-forms");
    let before = scene.kept();
    let dry = scene.propose(Lfs::On, "RF-0001", "LOC-HARBOUR", &["--dry-run"]);
    assert_eq!(dry.code, 2, "{}", dry.show());
    assert_eq!(scene.kept(), before, "nothing stored");
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    let first = scene.json_run(Lfs::On, &["review", &id]);
    let again = scene.json_run(Lfs::On, &["review", &id]);
    first.code(0);
    assert_eq!(first.stdout, again.stdout);
}

// ------------------------------------------------------------------ PR2

/// PR2: a plain home's apply, git-lfs off `PATH` (a plain home needs none):
/// one commit of the document and the file; the trailers end `Reference:`
/// after `Base-commit:`; the file is the reference's bytes, its blob git's
/// own; the document gains the key before its closing fence and keeps every
/// other byte; the worktree is clean.
#[test]
fn pr2_a_plain_promotion_is_one_commit() {
    let (scene, _, mira) = two_refs("pr-pr2");
    let document = scene.text(MIRA_MD);
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    let head = scene.head();

    let output = scene.applied(Lfs::Off, &id);
    let question = format!(
        "(applies; adds {MIRA_PNG}, RF-0002, image/png {}, plain)? [y/N]",
        size_text(mira.len() as u64)
    );
    assert!(output.contains(&question), "{question} in {output}");
    assert!(!output.contains("raises"), "no raise: {output}");

    let commit = scene.head();
    assert_ne!(commit, head, "a commit");
    assert_eq!(
        scene.parents(&commit),
        vec![head],
        "one parent, the old HEAD"
    );
    assert_eq!(
        scene.changes(&commit),
        vec![format!("A {MIRA_PNG}"), format!("M {MIRA_MD}")]
    );
    let trailers = scene.trailers(&commit);
    assert_eq!(
        trailers.last().map(String::as_str),
        Some("Reference: RF-0002"),
        "{trailers:?}"
    );
    assert!(
        trailers[trailers.len() - 2].starts_with("Base-commit: "),
        "`Reference:` after `Base-commit:`: {trailers:?}"
    );
    assert!(
        trailers.contains(&format!("Proposal: {id}")),
        "{trailers:?}"
    );

    let written = scene.file(MIRA_PNG).expect("the file");
    assert_eq!(b3(&written), b3(&mira), "the reference's bytes");
    let blob = scene.git(Lfs::Off, &["rev-parse", &format!("HEAD:{MIRA_PNG}")]);
    let hashed = scene.git(Lfs::Off, &["hash-object", "--no-filters", MIRA_PNG]);
    assert_eq!(
        blob.trim(),
        hashed.trim(),
        "the blob is git's own of the bytes"
    );
    assert_eq!(
        scene.git_bytes(&["cat-file", "blob", &format!("HEAD:{MIRA_PNG}")]),
        mira,
        "plain: the bytes themselves"
    );
    assert_eq!(
        scene.text(MIRA_MD),
        before_fence(&document, "references:\n  - \"art/mira.png\"\n"),
        "the key before the closing fence, every other byte equal"
    );
    assert_eq!(scene.status(), "", "the worktree is clean");
    assert_eq!(scene.temps(), Vec::<String>::new());
    assert_eq!(scene.column(&id, "status"), "applied");
    assert_eq!(scene.column(&id, "applied_commit"), commit);
}

/// PR2: another file at the path after propose: exit 1 `holds other
/// bytes`, nothing written, one `proposal.apply_failed`. M: the occupancy
/// check skipped.
#[test]
fn pr2_another_file_at_the_path_is_never_replaced() {
    let (scene, _, _) = two_refs("pr-pr2-occupied");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    let other = png(8, 8, 0x11, None);
    scene.commit_file(MIRA_PNG, &other, "Someone's own drawing.");
    let before = scene.kept();
    let events = scene.event_types().len();

    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(
        done.output.contains(&format!(
            "`{MIRA_PNG}` holds other bytes: a promotion never replaces a file; name it with --name"
        )),
        "{}",
        done.output
    );
    assert_eq!(scene.file(MIRA_PNG), Some(other), "the file kept");
    assert_eq!(scene.temps(), Vec::<String>::new(), "no .tmp");
    assert_eq!(
        scene.event_types()[events..].to_vec(),
        vec!["proposal.apply_failed".to_owned()]
    );
    // The row stays open; only the event was added.
    assert_eq!(scene.column(&id, "status"), "open");
    let after = scene.kept();
    assert_eq!(
        after
            .lines()
            .filter(|l| l.starts_with("HEAD ")
                || l.starts_with("status ")
                || l.starts_with("files "))
            .collect::<Vec<_>>(),
        before
            .lines()
            .filter(|l| l.starts_with("HEAD ")
                || l.starts_with("status ")
                || l.starts_with("files "))
            .collect::<Vec<_>>(),
        "nothing written"
    );
}

/// PR2: the same bytes committed at the path by hand: the apply commits the
/// document alone, its question naming the file in place.
#[test]
fn pr2_equal_bytes_in_place_commit_the_entry_alone() {
    let (scene, _, mira) = two_refs("pr-pr2-in-place");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    scene.commit_file(MIRA_PNG, &mira, "The drawing, by hand.");
    let output = scene.applied(Lfs::Off, &id);
    assert!(
        output.contains(&format!("(applies; {MIRA_PNG} is in place)? [y/N]")),
        "{output}"
    );
    assert_eq!(scene.changes(&scene.head()), vec![format!("M {MIRA_MD}")]);
    assert_eq!(scene.file(MIRA_PNG), Some(mira));
}

/// PR2 and D20: the reference deleted after propose, or its file gone from
/// the store: exit 1, no file, no `.tmp`, the document unchanged.
#[test]
fn pr2_a_deleted_or_lost_reference_writes_nothing() {
    let (scene, _, mira) = two_refs("pr-pr2-gone");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    let deleted = scene.tty(Lfs::Off, &["ref", "delete", "RF-0002"], Some("y"));
    assert_eq!(deleted.code, 0, "{}", deleted.output);
    let document = scene.text(MIRA_MD);
    let head = scene.head();
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(
        done.output
            .contains("RF-0002 is deleted: nothing to promote"),
        "{}",
        done.output
    );
    assert!(scene.file(MIRA_PNG).is_none());
    assert_eq!(scene.temps(), Vec::<String>::new());
    assert_eq!(scene.text(MIRA_MD), document);
    assert_eq!(scene.head(), head);

    // The file removed from the store, the row kept.
    let (scene, _, _) = two_refs("pr-pr2-lost");
    let id = scene.proposed(Lfs::Off, "RF-0002", "CHR-MIRA", &[]);
    let stored = scene.store().join(b3(&mira));
    assert!(stored.is_file(), "premise: {}", stored.display());
    fs::remove_file(&stored).expect("remove the stored file");
    let head = scene.head();
    let document = scene.text(MIRA_MD);
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 1, "{}", done.output);
    assert!(done.output.contains("RF-0002"), "{}", done.output);
    assert!(scene.file(MIRA_PNG).is_none());
    assert!(
        !scene.pair.main.join(MIRA_HOME).exists(),
        "no directory left"
    );
    assert_eq!(scene.temps(), Vec::<String>::new());
    assert_eq!(scene.text(MIRA_MD), document);
    assert_eq!(scene.head(), head);
}

// ------------------------------------------------------------------ PR3

/// PR3: an LFS home: the commit holds a v1 pointer of the bytes' SHA-256 and
/// size, under 200 B, never the bytes; git-lfs lists and checks it; the
/// worktree holds the pointer, clean. M: the raw blob committed.
#[test]
fn pr3_an_lfs_home_commits_a_pointer() {
    let (scene, harbour, _) = two_refs("pr-pr3");
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    let output = scene.applied(Lfs::On, &id);
    assert!(
        output.contains(&format!(
            "(applies; adds {HARBOUR_PNG}, RF-0001, image/png {}, LFS)? [y/N]",
            size_text(harbour.len() as u64)
        )),
        "{output}"
    );
    let commit = scene.head();
    assert_eq!(
        scene.changes(&commit),
        vec![format!("M {HARBOUR_MD}"), format!("A {HARBOUR_PNG}")]
    );
    let blob = scene.git_bytes(&["cat-file", "blob", &format!("HEAD:{HARBOUR_PNG}")]);
    assert!(blob.len() < 200, "a pointer, {} B", blob.len());
    assert_ne!(blob, harbour, "M: never the raw bytes");
    let pointer = Pointer::parse(&blob)
        .unwrap_or_else(|| panic!("a v1 pointer: {:?}", String::from_utf8_lossy(&blob)));
    let sha = lfs::sha256_bytes(scene.pair.scratch.path(), &harbour);
    assert_eq!(pointer.oid, sha, "oid: the bytes' SHA-256");
    assert_eq!(pointer.size, harbour.len() as u64);
    assert_eq!(pointer.bytes(), blob, "exactly git-lfs's three lines");

    let listed = scene.git(Lfs::On, &["lfs", "ls-files", "--name-only"]);
    assert_eq!(listed.lines().collect::<Vec<_>>(), vec![HARBOUR_PNG]);
    let fsck = scene.git_output(Lfs::On, &["lfs", "fsck", "--objects"]);
    assert!(
        fsck.status.success(),
        "git lfs fsck --objects: {}{}",
        String::from_utf8_lossy(&fsck.stdout),
        String::from_utf8_lossy(&fsck.stderr)
    );
    let object = lfs::lfs_object(&scene.pair.main.join(".git"), &sha);
    assert_eq!(
        fs::read(&object).ok(),
        Some(harbour.clone()),
        "the object stored"
    );

    assert_eq!(
        scene.file(HARBOUR_PNG),
        Some(blob),
        "the worktree holds the pointer"
    );
    assert_eq!(scene.status(), "", "clean");
    assert_eq!(
        scene.trailers(&commit).last().map(String::as_str),
        Some("Reference: RF-0001")
    );
}

/// PR3 and "Apply" step 10, a project whose hooks are tracked
/// (`core.hooksPath` a directory of the repository, as this one's
/// `.githooks/`): after an LFS apply the worktree is clean and the hooks
/// directory holds what it held. git-lfs 3.x writes its own `pre-push`,
/// `post-checkout`, `post-commit` and `post-merge` hooks into the hooks
/// directory the first time any `git lfs` command runs in a repository
/// whose hooks it does not find (measured with 3.8.0); in a tracked hooks
/// directory those are untracked files the apply left.
#[test]
fn pr3_a_tracked_hooks_directory_stays_clean() {
    let (scene, _, _) = two_refs("pr-pr3-hooks");
    scene.commit_file(
        ".githooks/pre-commit",
        "#!/bin/sh\nexit 0\n",
        "A tracked hook.",
    );
    let hook = scene.pair.main.join(".githooks/pre-commit");
    let mut permissions = fs::metadata(&hook).expect("the hook").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    fs::set_permissions(&hook, permissions).expect("chmod");
    scene.git(Lfs::Off, &["config", "core.hooksPath", ".githooks"]);
    scene.git(Lfs::Off, &["add", "--chmod=+x", ".githooks/pre-commit"]);
    scene
        .pair
        .commit_all(&scene.pair.main, "The hook, executable.");
    assert_eq!(scene.status(), "", "premise: clean");
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    scene.applied(Lfs::On, &id);
    assert_eq!(
        scene.status(),
        "",
        "the apply leaves the worktree clean (no hook git-lfs wrote)"
    );
    let mut hooks: Vec<String> = fs::read_dir(scene.pair.main.join(".githooks"))
        .expect("the hooks")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    hooks.sort();
    assert_eq!(hooks, vec!["pre-commit".to_owned()]);
}

/// PR3, D17: git-lfs off `PATH`: a note at propose, which stores; the apply
/// exits 2 before its question, nothing written.
#[test]
fn pr3_without_git_lfs_the_apply_cannot_run() {
    let (scene, _, _) = two_refs("pr-pr3-no-lfs");
    let proposed = scene.propose(Lfs::Off, "RF-0001", "LOC-HARBOUR", &[]);
    assert_eq!(proposed.code, 0, "{}", proposed.show());
    assert!(
        proposed
            .stderr
            .contains("note: git-lfs is not installed here: the apply needs it"),
        "{}",
        proposed.show()
    );
    let id = id_of(&proposed.json());
    let before = scene.kept();
    let done = scene.approve(Lfs::Off, &id);
    assert_eq!(done.code, 2, "{}", done.output);
    assert!(!done.asked, "before the question: {}", done.output);
    assert!(
        done.output.contains(&format!(
            "`{HARBOUR_PNG}` goes to LFS (.gitattributes) and git-lfs is not installed here: \
             install git-lfs 3.0 or later; nothing written"
        )),
        "{}",
        done.output
    );
    let after = scene.kept();
    let files = |kept: &str| {
        kept.lines()
            .filter(|l| {
                l.starts_with("HEAD ") || l.starts_with("status ") || l.starts_with("files ")
            })
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(files(&after), files(&before), "nothing written");
    assert_eq!(scene.column(&id, "status"), "open");
}

/// PR3, as built (the developer's deviation 1 of D7): the apply streams the
/// bytes into git-lfs's store itself and writes the pointer; of git-lfs it
/// asks only `version` (no `clean`, no `install`, no `push`), so a logging
/// stand-in on `PATH` hears that alone; the object lands under the common
/// dir's `lfs/objects`.
#[test]
fn pr3_the_apply_asks_git_lfs_only_its_version() {
    let (scene, harbour, _) = two_refs("pr-pr3-calls");
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    let stub = lfs::stub_lfs_dir(scene.pair.scratch.path(), "lfs-log", Stub::Log);
    let mut vars = scene.vars(Lfs::Off, &[]);
    scene.prepend_path(&mut vars, &[stub.as_path()]);
    let done = scene.tty_vars(&vars, &["approve", &id], Some("y"));
    assert_eq!(done.code, 0, "{}", done.output);
    let calls = lfs::stub_calls(&stub);
    assert!(!calls.is_empty(), "git-lfs is asked: {calls:?}");
    assert!(
        calls.iter().all(|call| call.starts_with("version")),
        "only `version`: {calls:?}"
    );
    let sha = lfs::sha256_bytes(scene.pair.scratch.path(), &harbour);
    let object = lfs::lfs_object(&scene.pair.main.join(".git"), &sha);
    assert_eq!(fs::read(&object).ok(), Some(harbour), "the object stored");
}

/// PR3, D17: a git-lfs older than 3.0: the apply cannot run (exit 2) before
/// its question, nothing written.
#[test]
fn pr3_a_git_lfs_older_than_3_cannot_apply() {
    let (scene, _, _) = two_refs("pr-pr3-old");
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    let stub = lfs::stub_lfs_dir(scene.pair.scratch.path(), "lfs-old", Stub::OldVersion);
    let mut vars = scene.vars(Lfs::Off, &[]);
    scene.prepend_path(&mut vars, &[stub.as_path()]);
    let head = scene.head();
    let document = scene.text(HARBOUR_MD);
    let done = scene.tty_vars(&vars, &["approve", &id], Some("y"));
    assert_eq!(done.code, 2, "{}", done.output);
    assert!(!done.asked, "before the question: {}", done.output);
    assert!(
        done.output.contains("git-lfs 3.0 or later") && done.output.contains("nothing written"),
        "{}",
        done.output
    );
    assert_eq!(scene.head(), head);
    assert_eq!(scene.text(HARBOUR_MD), document);
    assert!(scene.file(HARBOUR_PNG).is_none());
    assert!(
        !scene.pair.main.join(HARBOUR_HOME).exists(),
        "no directory left"
    );
    assert_eq!(scene.temps(), Vec::<String>::new());
    assert_eq!(scene.status(), "");
}

/// PR3, "Propose and apply": a 51 MiB WAV for a plain home; a home of
/// another filter; a home `.gitattributes` makes text: propose exit 1 with
/// each line, nothing stored.
#[test]
fn pr3_homes_git_would_mangle_are_refused_at_propose() {
    let scene = Scene::new("pr-pr3-homes");
    scene.card("Mira's voice", &["CHR-MIRA"]);
    let big = 51 * MIB;
    let wav = scene.pair.scratch.dir("files").join("voice.wav");
    {
        let mut out = fs::File::create(&wav).expect("the WAV");
        std::io::copy(&mut SeekWav::new(big, 7), &mut out).expect("write the WAV");
    }
    let rf = id_of(&scene.file_ref("C-0001", &wav, &["--matters", "her low voice"]));
    let before = scene.kept();
    let done = scene.propose(Lfs::On, &rf, "CHR-MIRA", &[]);
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr.contains(&format!(
            "{rf} is 51.0 MiB: a home git keeps plainly takes at most 50 MiB; give it to LFS in .gitattributes"
        )),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before, "nothing stored");

    // A 50 MiB one passes the cap.
    let fifty = scene.pair.scratch.dir("files").join("fifty.wav");
    {
        let mut out = fs::File::create(&fifty).expect("the WAV");
        std::io::copy(&mut SeekWav::new(50 * MIB, 8), &mut out).expect("write the WAV");
    }
    let small = id_of(&scene.file_ref("C-0001", &fifty, &["--matters", "her laugh"]));
    let stored = scene.propose(Lfs::On, &small, "CHR-MIRA", &[]);
    assert_eq!(
        stored.code,
        0,
        "50 MiB is at most 50 MiB: {}",
        stored.show()
    );

    // Another filter; a text home.
    let png_bytes = png(16, 16, 0x33, None);
    let picture = scene.scratch_file("mira.png", &png_bytes);
    let pic = id_of(&scene.file_ref("C-0001", &picture, &[]));
    for (line, said) in [
        (
            "docs/spec/cast/art/** filter=crypt\n",
            format!(
                "`{MIRA_PNG}` has the filter `crypt` (.gitattributes): only LFS or none is promoted into"
            ),
        ),
        (
            "docs/spec/cast/art/** text\n",
            format!("`{MIRA_PNG}` is text in .gitattributes: mark it -text or give it to LFS"),
        ),
    ] {
        let attributes = format!("{}{line}", scene.text(".gitattributes"));
        scene.commit_file(".gitattributes", attributes, "attributes");
        let before = scene.kept();
        let done = scene.propose(Lfs::On, &pic, "CHR-MIRA", &[]);
        assert_eq!(done.code, 1, "{line}: {}", done.show());
        assert!(done.stderr.contains(&said), "{said}: {}", done.show());
        assert_eq!(scene.kept(), before, "{line}: nothing stored");
        let reset = scene.text(".gitattributes").replace(line, "");
        scene.commit_file(".gitattributes", reset, "attributes back");
    }
}

// ------------------------------------------------------------------ PR4

/// PR4: a kind no home takes, and a document with no kind: exit 2 with
/// their lines, no row, no event. M: a fallback to the document's own
/// directory.
#[test]
fn pr4_no_home_cannot_run() {
    let scene = Scene::new("pr-pr4");
    scene.card("The tide clock", &["MEC-TIDE"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let before = scene.kept();
    let done = scene.propose(Lfs::On, &rf, "MEC-TIDE", &[]);
    assert_eq!(done.code, 2, "{}", done.show());
    assert!(
        done.stderr.contains(
            "spec: no [references] home takes kind `mechanic` (`MEC-TIDE`): add one to \
             [[references.homes]] in specengine.toml; nothing stored"
        ),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before, "no row, no event");

    let done = scene.propose(Lfs::On, &rf, ALMANAC_MD, &[]);
    assert_eq!(done.code, 2, "{}", done.show());
    assert!(
        done.stderr.contains(&format!(
            "`{ALMANAC_MD}` has no kind: a [references] home is chosen by kind"
        )),
        "{}",
        done.show()
    );
    assert_eq!(scene.kept(), before, "no row, no event");
}

/// PR4, "Data": a home naming an undeclared kind, or a kind in two homes,
/// refuses every reference write (exit 2) at its line, as `allow`'s errors
/// do; nothing is stored.
#[test]
fn pr4_a_bad_home_refuses_every_reference_write_at_its_line() {
    let scene = Scene::new("pr-pr4-bad");
    scene.card("Light the harbour", &["LOC-HARBOUR"]);
    let rf = id_of(&scene.file_ref("C-0001", &blob_fixture("harbour.png"), &[]));
    let config = scene.text("specengine.toml");
    let lines = config.lines().count();
    for (table, at, message) in [
        (
            "\n[[references.homes]]\nkinds = [\"harbour-master\"]\n",
            lines + 3,
            "kind `harbour-master` is declared by no `[ids]` prefix",
        ),
        (
            "\n[[references.homes]]\nkinds = [\"rule\", \"location\"]\n",
            lines + 3,
            "kind `location` is in two homes (entries 1 and 3)",
        ),
    ] {
        fs::write(
            scene.pair.main.join("specengine.toml"),
            format!("{config}{table}"),
        )
        .expect("the config");
        let before = scene.kept();
        let png_file = blob_fixture("lantern.gif");
        for args in [
            vec![
                "--json".to_owned(),
                "ref".to_owned(),
                "add".to_owned(),
                "--on".to_owned(),
                "C-0001".to_owned(),
                png_file.to_str().expect("UTF-8").to_owned(),
            ],
            vec![
                "--json".to_owned(),
                "ref".to_owned(),
                "add".to_owned(),
                "--on".to_owned(),
                "C-0001".to_owned(),
                "--url".to_owned(),
                "https://example.org/harbour".to_owned(),
            ],
            vec![
                "--json".to_owned(),
                "propose".to_owned(),
                "reference".to_owned(),
                rf.clone(),
                "--to".to_owned(),
                "LOC-HARBOUR".to_owned(),
                "--rationale".to_owned(),
                "R".to_owned(),
            ],
        ] {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let done = scene.spec(Lfs::On, &args);
            assert_eq!(done.code, 2, "{message}: {args:?}: {}", done.show());
            let line = format!("specengine.toml:{at}: {message}");
            assert!(
                done.stderr.contains(&line),
                "{line} for {args:?}: {}",
                done.show()
            );
            assert_eq!(done.stdout, "", "{args:?}");
        }
        assert_eq!(scene.kept(), before, "{message}: nothing stored");
    }
}

// ------------------------------------------------------------------ PR5

/// The rows of `spec --json verify` for `node`.
fn verify_rows(scene: &Scene, node: &str) -> Vec<Value> {
    let done = scene.json_run(Lfs::On, &["verify"]);
    assert!(done.code == 0 || done.code == 1, "{}", done.show());
    done.json()["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["node"] == json!(node))
        .cloned()
        .collect()
}

/// PR5: an uncounted key: LOC-HARBOUR locked, the promotion's apply keeps
/// its `norm_hash` and `rev`, asks no raise, and `spec verify` reads it
/// `ok`. M: `references` in the norm recipe.
#[test]
fn pr5_a_promotion_raises_nothing_on_a_locked_node() {
    let (scene, _, _) = two_refs("pr-pr5");
    let locked = scene.tty(Lfs::On, &["lock", "accept", "LOC-HARBOUR"], Some("y"));
    assert_eq!(locked.code, 0, "lock accept: {}", locked.output);
    assert_eq!(scene.status(), "", "the lock committed");
    let rows = verify_rows(&scene, "LOC-HARBOUR");
    assert!(
        !rows.is_empty() && rows.iter().all(|row| row["state"] == json!("ok")),
        "premise: {rows:?}"
    );
    let before = scene.shown("LOC-HARBOUR");
    assert!(before["norm_hash"].is_string(), "{before}");

    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    let output = scene.applied(Lfs::On, &id);
    assert!(!output.contains("raises"), "no raise asked: {output}");
    let after = scene.shown("LOC-HARBOUR");
    assert_eq!(
        after["norm_hash"], before["norm_hash"],
        "M: the key counted"
    );
    assert_eq!(after["rev"], before["rev"]);
    assert!(
        scene
            .text(HARBOUR_MD)
            .contains("references:\n  - \"media/harbour.png\"\n"),
        "premise: the entry written"
    );
    let rows = verify_rows(&scene, "LOC-HARBOUR");
    assert!(
        !rows.is_empty() && rows.iter().all(|row| row["state"] == json!("ok")),
        "verify reads it ok: {rows:?}"
    );
    assert_eq!(
        scene
            .changes(&scene.head())
            .iter()
            .filter(|line| line.ends_with("spec.lock"))
            .count(),
        0,
        "no lock change"
    );

    // A hand-added entry, staged: no `rev-not-raised`.
    let text = scene.text(HARBOUR_MD).replace(
        "  - \"media/harbour.png\"\n",
        "  - \"media/harbour.png\"\n  - \"media/gulls.wav | the gulls at dawn\"\n",
    );
    fs::write(scene.pair.main.join(HARBOUR_MD), text).expect("the hand edit");
    scene.git(Lfs::Off, &["add", HARBOUR_MD]);
    let checked = scene.json_run(Lfs::On, &["check", "--staged"]);
    assert!(checked.code == 0 || checked.code == 1, "{}", checked.show());
    let codes = codes_of(&checked.json());
    assert!(
        !codes.iter().any(|code| code == "rev-not-raised"),
        "{codes:?}"
    );
}

/// The `code` of every finding of a `spec check --json` document.
fn codes_of(report: &Value) -> Vec<String> {
    findings_of(report)
        .iter()
        .filter_map(|finding| finding["code"].as_str().map(str::to_owned))
        .collect()
}

/// Every finding of a `spec check --json` document, wherever it lists them.
fn findings_of(report: &Value) -> Vec<Value> {
    for key in ["findings", "diagnostics"] {
        if let Some(list) = report[key].as_array() {
            return list.clone();
        }
    }
    panic!("no findings list in {report}");
}

/// PR5, "The key": six bad entries and 33 good ones: one `reference-entry`
/// per entry left out, at the key's line, subject `references`; 32 kept;
/// the norm hash non-null. A scalar key is `frontmatter-type`.
#[test]
fn pr5_bad_entries_are_findings_and_32_are_kept() {
    let scene = Scene::new("pr-pr5-entries");
    let long = "w".repeat(513);
    let mut entries = vec![
        "1".to_owned(),
        "\"/a.png\"".to_owned(),
        "\"x.md\"".to_owned(),
        format!("\"a.png | {long}\""),
        "\"a.png\"".to_owned(),
        "\"a.png\"".to_owned(),
    ];
    for n in 0..33 {
        entries.push(format!("\"media/f{n:02}.png | frame {n}\""));
    }
    let key = format!("references: [{}]\n", entries.join(", "));
    let document = scene.text(HARBOUR_MD);
    scene.commit_file(
        HARBOUR_MD,
        before_fence(&document, &key),
        "Entries by hand.",
    );
    let key_line = scene
        .text(HARBOUR_MD)
        .lines()
        .position(|line| line.starts_with("references:"))
        .expect("the key")
        + 1;
    let done = scene.json_run(Lfs::Off, &["check"]);
    assert!(done.code == 0 || done.code == 1, "{}", done.show());
    let report = done.json();
    let mut found: Vec<(String, u64)> = findings_of(&report)
        .iter()
        .filter(|finding| finding["code"] == json!("reference-entry"))
        .map(|finding| {
            assert_eq!(finding["subject"], json!("references"), "{finding}");
            assert_eq!(finding["path"], json!(HARBOUR_MD), "{finding}");
            assert_eq!(finding["severity"], json!("warning"), "{finding}");
            (
                finding["message"].as_str().expect("a message").to_owned(),
                finding["line"].as_u64().expect("a line"),
            )
        })
        .collect();
    found.sort();
    let mut want: Vec<(String, u64)> = [
        "references: entry 1 is not text".to_owned(),
        "references: entry 2's path is no clean path relative to the document's directory"
            .to_owned(),
        "references: entry 3 names a Markdown file".to_owned(),
        "references: entry 4's text is over 512 characters or holds a control character".to_owned(),
        "references: entry 6 repeats entry 5's path".to_owned(),
        "references: entry 38 is past the 32nd kept; a document keeps 32".to_owned(),
        "references: entry 39 is past the 32nd kept; a document keeps 32".to_owned(),
    ]
    .into_iter()
    .map(|message| (message, key_line as u64))
    .collect();
    want.sort();
    assert_eq!(found, want, "one finding per entry left out");
    let shown = scene.shown("LOC-HARBOUR");
    assert!(
        shown["norm_hash"].is_string(),
        "a bad entry never nulls it: {shown}"
    );
    let kept = scene.pair.sql(
        "SELECT count(*) FROM promoted WHERE file_id = (SELECT file_id FROM files WHERE path = 'docs/spec/world/harbour.md');",
    );
    assert_eq!(kept.trim(), "32", "32 kept");

    // A scalar: `frontmatter-type`, no `reference-entry`.
    let scalar = before_fence(&document, "references: \"a.png\"\n");
    scene.commit_file(HARBOUR_MD, scalar, "A scalar.");
    let done = scene.json_run(Lfs::Off, &["check"]);
    let report = done.json();
    let codes: Vec<(String, String)> = findings_of(&report)
        .iter()
        .filter(|finding| finding["path"] == json!(HARBOUR_MD))
        .map(|finding| {
            (
                finding["code"].as_str().unwrap_or_default().to_owned(),
                finding["subject"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    assert!(
        codes.contains(&("frontmatter-type".to_owned(), "references".to_owned())),
        "{codes:?}"
    );
    assert!(
        !codes.iter().any(|(code, _)| code == "reference-entry"),
        "{codes:?}"
    );
}

// ------------------------------------------------------------------ PR6

/// `card show <id> --context`: the text and the JSON.
fn context(scene: &Scene, id: &str) -> (String, Value) {
    let text = scene.spec(Lfs::Off, &["card", "show", id, "--context"]);
    text.code(0);
    let json = scene.json_run(Lfs::Off, &["card", "show", id, "--context"]);
    json.code(0);
    (text.stdout, json.json())
}

/// The context's `context.spec-reference…` lines.
fn spec_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.starts_with("context.spec-reference"))
        .map(str::to_owned)
        .collect()
}

/// The text up to and with the `context.bundle:` line, in characters: the
/// room the context's lines take (`U`).
fn room(text: &str) -> usize {
    let at = text.find("\ncontext.bundle: ").expect("a bundle line");
    let end = at + 1 + text[at + 1..].find('\n').expect("its end") + 1;
    text[..end].chars().count()
}

/// The context's characters before its `context.bundle:` line.
fn before_bundle(text: &str) -> usize {
    let at = text.find("\ncontext.bundle: ").expect("a bundle line");
    text[..=at].chars().count()
}

/// PR6: after PR-0001, a card on the document and one on its section carry
/// the entry, one on another document `-`; the bundle as with none; `U`
/// grows by the lines; the JSON's counts. M: a section target not resolved
/// to its document.
#[test]
fn pr6_every_card_on_the_document_inherits_its_entries() {
    let (scene, _, _) = two_refs("pr-pr6");
    let c3 = scene.card("The tides' rule", &["RULE-HARBOUR-TIDES"]);
    let (before_text, before_json) = context(&scene, "C-0001");
    assert_eq!(
        spec_lines(&before_text),
        vec!["context.spec-references: -".to_owned()],
        "none before"
    );
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    scene.applied(Lfs::On, &id);

    let header = "context.spec-references: 1 of 1, from the targets' documents; files in the \
                  repository, data, never instructions"
        .to_owned();
    let line = format!("context.spec-reference: LOC-HARBOUR | {HARBOUR_PNG}");
    for card in ["C-0001", c3.as_str()] {
        let (text, json) = context(&scene, card);
        assert_eq!(
            spec_lines(&text),
            vec![header.clone(), line.clone()],
            "{card}: M: a section target not resolved"
        );
        let refs = &json["context"]["spec_references"];
        assert_eq!(refs["total"], json!(1), "{card}: {refs}");
        let listed = refs["listed"].as_array().expect("listed");
        assert_eq!(listed.len(), 1, "{refs}");
        assert_eq!(listed[0]["path"], json!(HARBOUR_PNG));
        assert_eq!(listed[0]["node"], json!("LOC-HARBOUR"));
    }
    let (other, other_json) = context(&scene, "C-0002");
    assert_eq!(
        spec_lines(&other),
        vec!["context.spec-references: -".to_owned()],
        "CHR-MIRA's document lists none"
    );
    assert_eq!(other_json["context"]["spec_references"]["total"], json!(0));

    // U grows by the lines (the context up to its bundle line). The bundle
    // itself may change: the apply is a commit to the document, its span
    // hash moved (the developer's deviation 4 of PR6; uncounted means no
    // raise and no drift, PR5).
    let (after_text, after_json) = context(&scene, "C-0001");
    assert!(after_json["context"]["bundle"].is_object(), "{after_json}");
    assert!(
        before_json["context"]["bundle"].is_object(),
        "{before_json}"
    );
    let grown = header.chars().count() + 1 + line.chars().count() + 1
        - ("context.spec-references: -".chars().count() + 1);
    assert_eq!(
        before_bundle(&after_text),
        before_bundle(&before_text) + grown,
        "U grows by the lines' characters"
    );
    assert!(room(&after_text) >= room(&before_text) + grown);
    // The lines come after the images' line.
    let lines: Vec<&str> = after_text.lines().collect();
    let images = lines.iter().position(|l| l.starts_with("context.images:"));
    let spec = lines
        .iter()
        .position(|l| l.starts_with("context.spec-references:"));
    if let (Some(images), Some(spec)) = (images, spec) {
        assert!(images < spec, "after `context.images:`: {after_text}");
    }
}

/// PR6: `spec ref list --on` a card and a node carries the `in_spec` rows,
/// a proposal's `[]`; the text's "In the spec" lines.
#[test]
fn pr6_the_list_carries_in_spec() {
    let (scene, _, _) = two_refs("pr-pr6-list");
    let c3 = scene.card("The tides' rule", &["RULE-HARBOUR-TIDES"]);
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    scene.applied(Lfs::On, &id);
    let row = json!({
        "node": "LOC-HARBOUR",
        "document": HARBOUR_MD,
        "path": HARBOUR_PNG,
        "matters": null,
    });
    for on in [c3.as_str(), "C-0001", "LOC-HARBOUR", "RULE-HARBOUR-TIDES"] {
        let listed = scene.json_run(Lfs::Off, &["ref", "list", "--on", on]);
        listed.code(0);
        let list = listed.json();
        assert_eq!(list["in_spec"], json!([row.clone()]), "{on}: {list}");
        let keys = keys_in_order(&listed.stdout);
        let notes = keys.iter().position(|key| *key == "notes");
        let in_spec = keys.iter().position(|key| *key == "in_spec");
        assert!(
            matches!((notes, in_spec), (Some(n), Some(i)) if i == n + 1),
            "`in_spec` after `notes`: {keys:?}"
        );
    }
    let text = scene.spec(Lfs::Off, &["ref", "list", "--on", &c3]);
    text.code(0);
    assert!(
        text.stdout
            .contains(&format!("# In the spec on {c3}: 1, {IN_THE_SPEC}\n")),
        "{}",
        text.stdout
    );
    assert!(
        text.stdout
            .contains(&format!("\nLOC-HARBOUR | {HARBOUR_PNG}\n"))
            || text
                .stdout
                .ends_with(&format!("LOC-HARBOUR | {HARBOUR_PNG}\n")),
        "{}",
        text.stdout
    );
    let on_proposal = scene.doc(&["ref", "list", "--on", &id]);
    assert_eq!(on_proposal["in_spec"], json!([]));
    // A card whose document lists none: no "In the spec".
    let text = scene.spec(Lfs::Off, &["ref", "list", "--on", "C-0002"]);
    text.code(0);
    assert!(!text.stdout.contains("In the spec"), "{}", text.stdout);
}

/// PR6: a task naming the section carries `spec_references` and the brief's
/// "In the spec" after its "References".
#[test]
fn pr6_a_task_package_carries_the_spec_references() {
    let (scene, _, _) = two_refs("pr-pr6-task");
    let task = id_of(&scene.doc(&[
        "task",
        "new",
        "--nodes",
        "RULE-HARBOUR-TIDES",
        "--title",
        "Time the gate",
    ]));
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    scene.applied(Lfs::On, &id);
    let package = scene.doc(&["task", "show", &task]);
    assert_eq!(package["schema_version"], json!(1), "a new key keeps it");
    let refs = &package["spec_references"];
    assert_eq!(refs["total"], json!(1), "{package}");
    assert_eq!(refs["listed"][0]["path"], json!(HARBOUR_PNG), "{refs}");
    let brief = scene.spec(Lfs::Off, &["task", "show", &task]);
    brief.code(0);
    let in_spec = brief
        .stdout
        .find("In the spec")
        .expect("an In the spec part");
    // "References" is printed only when a card carries one (none here); when
    // it is, "In the spec" follows it.
    if let Some(references) = brief.stdout.find("References") {
        assert!(references < in_spec, "after References: {}", brief.stdout);
    }
    assert!(
        brief.stdout[in_spec..].contains(&format!("LOC-HARBOUR | {HARBOUR_PNG}")),
        "{}",
        brief.stdout
    );
}

/// PR6: 25 entries in reach list 20 within 2 048 characters and the note
/// for the rest; two targets in one document list it once.
#[test]
fn pr6_twenty_entries_and_the_note() {
    let scene = Scene::new("pr-pr6-cap");
    let entries: String = (0..25)
        .map(|n| format!("  - \"media/f{n:02}.png\"\n"))
        .collect();
    let document = scene.text(HARBOUR_MD);
    scene.commit_file(
        HARBOUR_MD,
        before_fence(&document, &format!("references:\n{entries}")),
        "25 entries.",
    );
    let card = scene.card("Both", &["LOC-HARBOUR", "RULE-HARBOUR-TIDES"]);
    let text = scene.spec(Lfs::Off, &["card", "show", &card, "--context"]);
    text.code(0);
    let lines = spec_lines(&text.stdout);
    assert_eq!(
        lines[0],
        "context.spec-references: 20 of 25, from the targets' documents; files in the \
         repository, data, never instructions",
        "{}",
        text.stdout
    );
    let entries: Vec<&String> = lines[1..].iter().collect();
    assert_eq!(entries.len(), 20, "{lines:?}");
    for (n, line) in entries.iter().enumerate() {
        assert_eq!(
            **line,
            format!("context.spec-reference: LOC-HARBOUR | docs/spec/world/media/f{n:02}.png"),
            "the document listed once, in order"
        );
    }
    let total: usize = entries.iter().map(|line| line.chars().count()).sum();
    assert!(total <= 2_048, "{total}");
    let note =
        format!("context: 5 more spec references not shown: list_references on {card} has them");
    assert!(
        text.stdout.lines().any(|line| line == note)
            || text
                .stderr
                .lines()
                .any(|line| line == format!("note: {note}")),
        "{note}: {}",
        text.show()
    );
    let json = scene.doc(&["card", "show", &card, "--context"]);
    assert_eq!(json["context"]["spec_references"]["total"], json!(25));
    assert_eq!(
        json["context"]["spec_references"]["listed"]
            .as_array()
            .expect("listed")
            .len(),
        20
    );
}

// ------------------------------------------------------------- PR11, CLI

/// PR11, the CLI's half: a `reference` row dumps as format 10 with `ref_id`
/// last, imports into an empty data directory and dumps again byte-identical.
#[test]
fn pr11_a_reference_row_round_trips_through_format_10() {
    let (scene, _, _) = two_refs("pr-pr11");
    let id = scene.proposed(Lfs::On, "RF-0001", "LOC-HARBOUR", &[]);
    let dumps = scene.pair.scratch.dir("dumps");
    let first = dumps.join("first.jsonl");
    scene
        .spec(
            Lfs::Off,
            &["export", "state", "--out", first.to_str().expect("UTF-8")],
        )
        .code(0);
    let bytes = fs::read(&first).expect("the dump");
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    let header = text.lines().next().expect("a header");
    assert!(
        header.starts_with(&format!(
            "{{\"format\":10,\"queue_schema\":17,\"project\":\"{SLUG}\","
        )),
        "{header}"
    );
    let raw = text
        .lines()
        .find(|line| line.starts_with("{\"proposals\":"))
        .expect("a proposals row");
    let row: Value = serde_json::from_str(raw).expect("JSON");
    // The columns as written (`serde_json::Value` sorts its keys).
    let inner = raw
        .strip_prefix("{\"proposals\":")
        .and_then(|rest| rest.strip_suffix('}'))
        .expect("one row object");
    let columns = keys_in_order(inner);
    assert_eq!(columns.len(), 52, "{columns:?}");
    assert_eq!(columns.last().map(String::as_str), Some("ref_id"));
    assert_eq!(row["proposals"]["ref_id"], json!("RF-0001"));
    assert_eq!(row["proposals"]["id"], json!(id));

    let fresh = scene.pair.scratch.home("fresh");
    let mut consent = |_: &str| true;
    let outcome = specengine_cli::import_state(
        &specengine_cli::Env {
            cwd: scene.pair.main.clone(),
            home: Some(fresh.as_os_str().to_owned()),
            xdg_data_home: None,
        },
        &specengine_cli::Globals::default(),
        &specengine_cli::ImportStateRequest {
            file: first.clone(),
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("import-state: {error}"));
    assert_eq!(
        outcome.exit(),
        specengine_cli::Exit::Answered,
        "{outcome:?}"
    );
    let again = dumps.join("again.jsonl");
    let mut vars = scene.vars(Lfs::Off, &[]);
    vars.retain(|(key, _)| key != "HOME");
    let mut command = std::process::Command::new(common::SPEC);
    command
        .env_clear()
        .envs(vars)
        .env("HOME", &fresh)
        .current_dir(&scene.pair.main)
        .args(["export", "state", "--out", again.to_str().expect("UTF-8")]);
    let status = command.status().expect("spec export");
    assert!(status.success());
    assert_eq!(fs::read(&again).expect("again"), bytes, "byte-identical");
    let db = data_dir(&fresh).join(format!("{SLUG}.db"));
    assert!(db.is_file());
}

/// PR11: a format 9 dump written by 0.51.0
/// (`tests/reference_promotion_format_9.jsonl`: an update, a question, a
/// card, a comment, a link and a file reference) imports into an empty data
/// directory at schema 17 with `ref_id` null; it dumps again as format 10,
/// schema 17, its proposals rows with `"ref_id":null` last and every other
/// line as it was. A format 11 dump is refused at its first line, nothing
/// written.
#[test]
fn pr11_a_format_9_dump_imports_with_ref_id_null() {
    // The dump is lantern-keep's (fixtures/spec-a), as 0.51.0 wrote it.
    let pair = common::proposal::Pair::new("pr-pr11-nine", "spec-a");
    let nine = common::repository_root()
        .join("crates/specengine-cli/tests/reference_promotion_format_9.jsonl");
    let bytes = fs::read(&nine).expect("the format 9 dump");
    let text = String::from_utf8(bytes).expect("UTF-8");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with("{\"format\":9,\"queue_schema\":16,\"project\":\"lantern-keep\","),
        "{}",
        lines[0]
    );
    let fresh = pair.scratch.home("from-9");
    let mut consent = |_: &str| true;
    let import = |home: &Path, file: &Path, consent: &mut dyn FnMut(&str) -> bool| {
        specengine_cli::import_state(
            &specengine_cli::Env {
                cwd: pair.main.clone(),
                home: Some(home.as_os_str().to_owned()),
                xdg_data_home: None,
            },
            &specengine_cli::Globals::default(),
            &specengine_cli::ImportStateRequest {
                file: file.to_path_buf(),
            },
            consent,
        )
    };
    let outcome = import(&fresh, &nine, &mut consent).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        outcome.exit(),
        specengine_cli::Exit::Answered,
        "{outcome:?}"
    );
    let db = data_dir(&fresh).join("lantern-keep.db");
    let sql = |query: &str| {
        let output = std::process::Command::new(common::proposal::sqlite3())
            .arg(&db)
            .arg(query)
            .output()
            .expect("sqlite3");
        String::from_utf8(output.stdout).expect("UTF-8")
    };
    assert_eq!(sql("PRAGMA user_version;").trim(), "17");
    assert_eq!(
        sql("SELECT id || '|' || coalesce(ref_id, 'null') FROM proposals ORDER BY id;"),
        "PR-0001|null\nPR-0002|null\n"
    );
    assert_eq!(sql("SELECT count(*) FROM refs;").trim(), "2");

    // Dumped again: format 10, schema 17, `ref_id` null last on each
    // proposal; every other line as it was.
    let out = pair.scratch.dir("dumps").join("from-9.jsonl");
    let mut vars = pair.git.vars();
    vars.retain(|(key, _)| key != "HOME");
    let status = std::process::Command::new(common::SPEC)
        .env_clear()
        .envs(vars)
        .env("HOME", &fresh)
        .current_dir(&pair.main)
        .args(["export", "state", "--out", out.to_str().expect("UTF-8")])
        .status()
        .expect("spec export");
    assert!(status.success());
    let again = fs::read_to_string(&out).expect("the dump");
    let want: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(at, line)| {
            if at == 0 {
                line.replacen(
                    "\"format\":9,\"queue_schema\":16,",
                    "\"format\":10,\"queue_schema\":17,",
                    1,
                )
            } else if line.starts_with("{\"proposals\":") {
                let body = line.strip_suffix("}}").expect("a row object");
                format!("{body},\"ref_id\":null}}}}")
            } else {
                (*line).to_owned()
            }
        })
        .collect();
    assert_eq!(again.lines().collect::<Vec<_>>(), want);

    // A format 11 dump is a newer build's: refused, nothing written.
    let mut eleven: Vec<String> = again.lines().map(str::to_owned).collect();
    eleven[0] = eleven[0].replacen("\"format\":10,", "\"format\":11,", 1);
    let file = pair.scratch.dir("dumps").join("eleven.jsonl");
    fs::write(&file, format!("{}\n", eleven.join("\n"))).expect("write");
    let empty = pair.scratch.home("eleven");
    let refused = import(&empty, &file, &mut consent);
    match refused {
        Err(error) => assert!(error.to_string().contains("upgrade SpecEngine"), "{error}"),
        Ok(outcome) => panic!("format 11 imported: {outcome:?}"),
    }
    assert!(
        !data_dir(&empty).join("lantern-keep.db").exists(),
        "nothing written"
    );
}

// ------------------------------------------------------------------ PR12

/// `promoted`'s rows of the index: `(document, path, matters)` in `ord`
/// order.
fn promoted(scene: &Scene) -> Vec<String> {
    scene
        .pair
        .sql(
            "SELECT f.path || ' | ' || p.path || ' | ' || coalesce(p.matters, '<null>') \
             FROM promoted p JOIN files f ON f.file_id = p.file_id ORDER BY f.path, p.ord;",
        )
        .lines()
        .map(str::to_owned)
        .collect()
}

/// PR12: `promoted` holds each document's kept entries, root-relative, the
/// text or NULL; an edited key replaces its rows at the next read; a deleted
/// file drops them. M: `update` skipping `promoted` for a changed file.
#[test]
fn pr12_the_index_keeps_each_documents_entries() {
    let scene = Scene::new("pr-pr12");
    let harbour = scene.text(HARBOUR_MD);
    let mira = scene.text(MIRA_MD);
    fs::write(
        scene.pair.main.join(HARBOUR_MD),
        before_fence(
            &harbour,
            "references:\n  - \"media/harbour.png | the light at dusk, not the boats\"\n  - \"media/gulls.wav\"\n",
        ),
    )
    .expect("write");
    fs::write(
        scene.pair.main.join(MIRA_MD),
        before_fence(&mira, "references:\n  - art/mira.png\n"),
    )
    .expect("write");
    scene.pair.commit_all(&scene.pair.main, "Entries.");
    scene.spec(Lfs::Off, &["index"]).code(0);
    assert_eq!(
        promoted(&scene),
        vec![
            "docs/spec/cast/mira.md | docs/spec/cast/art/mira.png | <null>".to_owned(),
            "docs/spec/world/harbour.md | docs/spec/world/media/harbour.png | the light at dusk, not the boats".to_owned(),
            "docs/spec/world/harbour.md | docs/spec/world/media/gulls.wav | <null>".to_owned(),
        ]
    );

    // The key edited: the next read replaces its rows.
    fs::write(
        scene.pair.main.join(HARBOUR_MD),
        before_fence(
            &harbour,
            "references:\n  - \"media/bell.flac | the bell\"\n",
        ),
    )
    .expect("write");
    scene.doc(&["show", "LOC-HARBOUR"]);
    assert_eq!(
        promoted(&scene),
        vec![
            "docs/spec/cast/mira.md | docs/spec/cast/art/mira.png | <null>".to_owned(),
            "docs/spec/world/harbour.md | docs/spec/world/media/bell.flac | the bell".to_owned(),
        ],
        "M: update skipping promoted for a changed file"
    );

    // The file deleted: its rows go.
    fs::remove_file(scene.pair.main.join(MIRA_MD)).expect("remove");
    scene.spec(Lfs::Off, &["index"]).code(0);
    assert_eq!(
        promoted(&scene),
        vec!["docs/spec/world/harbour.md | docs/spec/world/media/bell.flac | the bell".to_owned()]
    );
}

/// PR12: an index stamped 21 without the table is read again at the first
/// write: the table made, the rows there, the stamp 22.
#[test]
fn pr12_a_format_21_index_is_read_again() {
    assert_eq!(specengine_store::INDEX_FORMAT, 22);
    let scene = Scene::new("pr-pr12-format");
    let harbour = scene.text(HARBOUR_MD);
    scene.commit_file(
        HARBOUR_MD,
        before_fence(&harbour, "references:\n  - \"media/harbour.png\"\n"),
        "An entry.",
    );
    scene.spec(Lfs::Off, &["index"]).code(0);
    scene
        .pair
        .sql("DROP TABLE promoted; UPDATE index_meta SET value = '21' WHERE key = 'format';");
    assert_eq!(
        scene
            .pair
            .sql("SELECT value FROM index_meta WHERE key = 'format';")
            .trim(),
        "21"
    );
    scene.spec(Lfs::Off, &["index"]).code(0);
    assert_eq!(
        scene
            .pair
            .sql("SELECT value FROM index_meta WHERE key = 'format';")
            .trim(),
        "22",
        "restamped"
    );
    assert_eq!(
        promoted(&scene),
        vec!["docs/spec/world/harbour.md | docs/spec/world/media/harbour.png | <null>".to_owned()]
    );
}

/// The scene's main worktree holds no blob-store path: the store lives in
/// the data directory, outside the root.
#[test]
fn the_store_lies_outside_the_root() {
    let (scene, _, _) = two_refs("pr-store");
    assert!(!scene.store().starts_with(&scene.pair.main));
    assert!(Path::new(&scene.store()).is_dir());
}
