//! docs/features/work-references.md (RM-0137, ADR-0088) "Acceptance
//! criteria", the CLI half: the `spec` binary on scratch git repositories of
//! `fixtures/spec-a` (a game's brief, aliases, look-alikes) and
//! `fixtures/trace-almanac` (a village almanac, no game word, a card flow
//! with an accept), so the reference rules are tried on two projects of
//! different nature (ADR-0008); each with its own `HOME` and so its own data
//! directory and blob store. The files come from `fixtures/blob-types/`
//! (`expected.json` holds each one's sniff and facts) or are built here at
//! the size a criterion names.
//!
//! - RF2: two links on a card store `RF-0001`, `RF-0002`; an alias exits 1
//!   naming the canonical ID; a look-alike exits 2 with its Latin form;
//!   `C-0099` and `PR-0099` exit 1; a node's `on_rev` is the index's
//!   revision; `[ids]` claiming `RF` exits 2 on every `spec ref` command; no
//!   refusal stores a row, a file or an event. M: the card existence check
//!   skipped.
//! - RF3, the terminal: each stored fixture stores with its media type,
//!   class, facts and warning, named by its BLAKE3, mode 0400; each refused
//!   one exits 1 with its line, leaving no file, no `.part`, no row, no
//!   event; `allow = ["image"]` refuses the WAV and stores the PNG;
//!   `allow = ["image/svg+xml"]` exits 2 naming its line, and only `spec ref
//!   add` refuses. M: the type taken from the name's extension.
//! - RF4, the terminal's settings: `SPECENGINE_BLOB_FILE_MAX` refuses a
//!   larger file before reading it, `SPECENGINE_BLOB_QUOTA` the file that
//!   would pass it; a setting that would widen exits 2.
//! - RF6: a file's name is the BLAKE3 of its bytes, mode 0400 in a 0700
//!   store; equal bytes keep one file whose mtime is the second write's;
//!   deleting one of two references keeps it, the other removes it in that
//!   call. M: a dedup writing a second file.
//! - RF9, the terminal: a WAV without "what matters" exits 1; an image and
//!   a link store without it; a `.blend` with its warning.
//! - RF12, the terminal: `spec ref delete` with stdin no terminal exits 2,
//!   a declined prompt exits 1, both with no event; the prompt's words.
//! - RF13: title, "what matters" and name holding markers, deleted, the WAL
//!   checkpointed: no marker in the database, the WAL or the dump; the row
//!   keeps `deleted_by`, `deleted_at`; its file and agent copy gone in the
//!   same call. With `.swept` over an hour old, `spec ref add` sweeps a file
//!   no row names 24 h + 1 s old and a `.part` 6 h + 1 s old, keeping a 23 h
//!   one; within the hour it sweeps nothing. M: the sweep's re-check.
//! - RF14: with 50 references on a card, its assign, move, accept, an
//!   approve of its target, its clear and another's cancel give the same
//!   documents and events as with none, and run with `refs` renamed away
//!   (no statement on it); a plain `card show` too. M: `change_card` reads
//!   `refs`.
//! - RF15: `card show --context` on 12 references lists the newest 10,
//!   oldest first, the note for the rest, `context.images:` the newest three
//!   agent-ready; a 2 048-byte "what matters" cut at 512 with ` [cut]`; the
//!   4 096 bound; the lines count in `U`; `bundle_hash` as with none;
//!   `task show` carries both cards' references, each naming its card,
//!   `schema_version` 1. M: the lines left out of `U`.
//! - RF16: each attach and delete one `reference.*` event, its payload
//!   `{id, on, as}` or `{id, as}`, no title, URL, name, hash or "what
//!   matters"; the repository unchanged; `spec feed` words both. M: the
//!   title in the payload.
//! - RF17: link, file, deleted and node references dump as format 9 (11
//!   header keys, `refs` counted), import into an empty data directory and
//!   dump again byte-identical; the dump holds no file's bytes and its size
//!   does not follow them; a format 8 dump written by 0.50.0
//!   (`tests/work_references_format_8.jsonl`) imports with no references;
//!   after an import with no blob store a file reference reads `file: lost`
//!   and `spec ref get --out` exits 1 with its line. M: `refs` left out of
//!   the dump.
//! - "Data": the 21 keys of a reference, the list's text and paging, `spec
//!   ref get --out` (the copy, an existing file, a damaged store), `spec ref`
//!   alone naming `spec refs`.
//!
//! The daemon's half (the binary door, serving, Range, rights on a hub, the
//! sweep's race) is `specengine-http`'s `tests/work_references.rs`, MCP's
//! its crate's. Every expected value is worked out from the spec, never read
//! back from a run.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
#[path = "work_refs_common/mod.rs"]
mod refs_common;

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::proposal::{LATER, Pair};
use common::{Run, data_dir, read_text, repository_root, with_ids_line, write};
use refs_common::{
    MATERIAL, MATTERS_REQUIRED, REFERENCE_KEYS, SeekWav, age, age_of, blob_expected, blob_fixture,
    facts_text, keys_in_order, mode, needs_matters, parts, png, refused_line, store_files,
};
use serde_json::{Value, json};
use specengine_cli::{
    BlobBegun, BlobCaps, BlobFailure, BlobFlight, BlobPutRequest, BlobRefusalKind, BlobSeams,
    CardAcceptRequest, CardAssignRequest, CardCancelRequest, CardClearOutcome, CardClearRequest,
    CardMoveRequest, CardNewRequest, Env, Exit, Globals, card_accept, card_assign, card_cancel,
    card_clear, card_move, card_new,
};

/// The game's project.
const KEEP: &str = "spec-a";
/// The almanac's project.
const ALMANAC: &str = "trace-almanac";

const MIB: u64 = 1024 * 1024;

const WARNING: &str = "a .blend is only downloaded here; Blender can run the scripts it holds: \
                       open it with Auto Run Python Scripts off";

// ------------------------------------------------------------- helpers

/// `spec args` in the main worktree with `input` on stdin and `envs` added.
fn run_env(pair: &Pair, args: &[&str], input: &[u8], envs: &[(&str, &str)]) -> Run {
    let mut command = Command::new(common::SPEC);
    command
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", &pair.home)
        .envs(envs.iter().copied())
        .current_dir(&pair.main)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn spec");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(input);
    }
    let mut stdout = child.stdout.take().expect("stdout");
    let mut stderr = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait on spec") {
            break status;
        }
        if started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec {args:?} ran longer than 120 s; killed");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: status.code().expect("spec exited, not killed by a signal"),
        stdout: String::from_utf8(out.join().expect("stdout")).expect("UTF-8 stdout"),
        stderr: String::from_utf8(err.join().expect("stderr")).expect("UTF-8 stderr"),
    }
}

/// `spec --json args`.
fn json_run(pair: &Pair, args: &[&str]) -> Run {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    pair.spec_piped(&pair.main, &all, b"")
}

/// `spec args` (text).
fn text_run(pair: &Pair, args: &[&str]) -> Run {
    pair.spec_piped(&pair.main, args, b"")
}

/// `spec --json args`, exit 0: the document.
fn doc(pair: &Pair, args: &[&str]) -> Value {
    let done = json_run(pair, args);
    done.code(0);
    done.json()
}

fn id_of(document: &Value) -> String {
    document["id"]
        .as_str()
        .unwrap_or_else(|| panic!("an ID: {document}"))
        .to_owned()
}

/// `spec --json card new --title <title> <flags>`: its ID.
fn card(pair: &Pair, title: &str, flags: &[&str]) -> String {
    let mut args = vec!["card", "new", "--title", title];
    args.extend_from_slice(flags);
    id_of(&doc(pair, &args))
}

/// `spec --json ref add --on <on> <file> <extra>`.
fn add_file(pair: &Pair, on: &str, file: &Path, extra: &[&str]) -> Run {
    let file = file.to_str().expect("a UTF-8 path");
    let mut args = vec!["ref", "add", "--on", on, file];
    args.extend_from_slice(extra);
    json_run(pair, &args)
}

/// `spec --json ref add --on <on> --url <url> <extra>`.
fn add_url(pair: &Pair, on: &str, url: &str, extra: &[&str]) -> Run {
    let mut args = vec!["ref", "add", "--on", on, "--url", url];
    args.extend_from_slice(extra);
    json_run(pair, &args)
}

/// [`add_file`], exit 0: the reference.
fn file_ref(pair: &Pair, on: &str, file: &Path, extra: &[&str]) -> Value {
    let done = add_file(pair, on, file, extra);
    done.code(0);
    done.json()
}

/// [`add_url`], exit 0: the reference.
fn link_ref(pair: &Pair, on: &str, url: &str, extra: &[&str]) -> Value {
    let done = add_url(pair, on, url, extra);
    done.code(0);
    done.json()
}

/// `spec --json ref list --on <on> <extra>`, exit 0.
fn list(pair: &Pair, on: &str, extra: &[&str]) -> Value {
    let mut args = vec!["ref", "list", "--on", on];
    args.extend_from_slice(extra);
    doc(pair, &args)
}

/// `spec ref delete <id> <extra>` on a pseudo-terminal answering `answer`.
fn delete_on_terminal(
    pair: &Pair,
    id: &str,
    extra: &[&str],
    answer: Option<&str>,
) -> (i32, String) {
    let mut args = vec!["ref", "delete", id];
    args.extend_from_slice(extra);
    let run = pair.spec_pty(&pair.main, &args, answer);
    (run.code, run.output)
}

/// [`delete_on_terminal`] answered `y`: it must delete.
fn deleted(pair: &Pair, id: &str, extra: &[&str]) {
    let (code, output) = delete_on_terminal(pair, id, extra, Some("y"));
    assert_eq!(code, 0, "delete {id}: {output}");
}

/// The project's blob store.
fn store(pair: &Pair) -> PathBuf {
    data_dir(&pair.home).join("blobs").join(&pair.slug)
}

/// Every reference row, the events' count and last number, the store's
/// files and `.part` files: what a refusal must leave.
fn kept(pair: &Pair) -> String {
    let rows = if pair.db().exists() {
        pair.sql(
            "SELECT * FROM refs ORDER BY id; SELECT count(*) FROM events; SELECT max(seq) FROM events;",
        )
    } else {
        String::new()
    };
    format!(
        "{rows}\nfiles {:?}\nparts {:?}",
        store_files(&store(pair)),
        parts(&store(pair))
    )
}

/// A refusal of a reference command: exit `code`, `line` on stderr, nothing
/// stored (rows, events, files, `.part`).
fn refused(pair: &Pair, before: &str, done: &Run, code: i32, line: &str) {
    assert_eq!(done.code, code, "{line}: {}", done.show());
    assert!(
        done.stderr.contains(line),
        "{line} on stderr: {}",
        done.show()
    );
    assert_eq!(kept(pair), before, "{line}: nothing stored");
}

/// `(type, payload)` of every event after the first `skip`.
fn events_after(pair: &Pair, skip: usize) -> Vec<(String, Value)> {
    pair.events()
        .into_iter()
        .skip(skip)
        .map(|event| (event.event_type, event.payload))
        .collect()
}

/// The BLAKE3 of `bytes`, 64 lower-case hex (the store's `b3_hash` without
/// its `b3:`).
fn b3(bytes: &[u8]) -> String {
    specengine_store::b3_hash(bytes)
        .strip_prefix("b3:")
        .expect("b3:")
        .to_owned()
}

/// A scratch file holding `bytes`, named `name`.
fn scratch_file(pair: &Pair, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = pair.scratch.dir("files");
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write a scratch file");
    path
}

fn env(pair: &Pair) -> Env {
    pair.env(&pair.main)
}

fn holds(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

/// Binary units, one decimal: the spec's sizes (`300.0 MiB`, `2.1 MiB`).
fn size_text(bytes: u64) -> String {
    let units = ["KiB", "MiB", "GiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", units[unit])
}

// ------------------------------------------------------------------ RF2

/// RF2: links number from `RF-0001`; a card, a proposal and node REFs
/// store their canonical IDs, a node's `on_rev` the index's revision; the
/// document's 21 keys, in order.
#[test]
fn rf2_targets_and_numbers() {
    let pair = Pair::new("rf-rf2-targets", KEEP);
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    let first = add_url(&pair, &c1, "https://example.org/a", &[]);
    first.code(0);
    assert_eq!(
        keys_in_order(&first.stdout),
        REFERENCE_KEYS,
        "{}",
        first.stdout
    );
    let first = first.json();
    assert_eq!(first["id"], json!("RF-0001"));
    assert_eq!(first["on"], json!({"kind": "card", "id": c1}));
    assert_eq!(first["on_rev"], Value::Null);
    assert_eq!(first["kind"], json!("link"));
    assert_eq!(first["url"], json!("https://example.org/a"));
    for key in [
        "name", "b3", "agent_b3", "size", "media", "meta", "file", "warning", "deleted",
    ] {
        assert_eq!(first[key], Value::Null, "a link's {key}");
    }
    let second = link_ref(&pair, &c1, "https://example.org/a", &[]);
    assert_eq!(second["id"], json!("RF-0002"), "a link may repeat");

    let shown = doc(&pair, &["show", "RULE-SPRINT-COST"]);
    let rev = shown["nodes"][0]["rev"].clone();
    assert_eq!(rev, json!(3), "premise: rev 3");
    let on_node = link_ref(&pair, "RULE-SPRINT-COST", "https://example.org/n", &[]);
    assert_eq!(
        on_node["on"],
        json!({"kind": "node", "id": "RULE-SPRINT-COST"})
    );
    assert_eq!(on_node["on_rev"], rev);
    let slugged = link_ref(&pair, "stamina-tuning/AC-07", "https://example.org/s", &[]);
    assert_eq!(
        slugged["on"],
        json!({"kind": "node", "id": "stamina-tuning/AC-07"})
    );
    assert_eq!(
        pair.sql("SELECT on_kind, on_id, quote(on_rev) FROM refs WHERE id = 'RF-0003';")
            .trim(),
        "node|RULE-SPRINT-COST|3"
    );
    // A proposal's.
    let asked = json_run(
        &pair,
        &[
            "propose",
            "question",
            "MEC-SPRINT",
            "--text",
            "Does a sprint end at zero?",
            "--working-answer",
            "Yes: as the brief says.",
            "--price-of-other",
            "One scene rewritten.",
            "--author-role",
            "developer",
        ],
    );
    asked.code(0);
    let question = id_of(&asked.json());
    let on_proposal = link_ref(&pair, &question, "https://example.org/q", &[]);
    assert_eq!(
        on_proposal["on"],
        json!({"kind": "proposal", "id": question})
    );
}

/// RF2: an alias exits 1 naming the canonical ID; a look-alike exits 2 with
/// its Latin form; `C-0099` and `PR-0099` exit 1; none stores a row, a file
/// or an event, and the next reference takes the next number. M: the card
/// existence check skipped.
#[test]
fn rf2_refused_targets_store_nothing() {
    let pair = Pair::new("rf-rf2-refused", KEEP);
    card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    link_ref(&pair, "C-0001", "https://example.org/first", &[]);
    let png = blob_fixture("harbour.png");
    let before = kept(&pair);
    let alias = add_url(&pair, "QST-031", "https://example.org/x", &[]);
    refused(&pair, &before, &alias, 1, "`Q-031`");
    let look = add_url(
        &pair,
        "RULE-S\u{0420}RINT-COST",
        "https://example.org/x",
        &[],
    );
    refused(&pair, &before, &look, 2, "RULE-SPRINT-COST");
    assert_eq!(look.stdout, "", "exit 2 prints no document");
    for (on, line) in [
        ("C-0099", "on: no card C-0099"),
        ("PR-0099", "on: no proposal PR-0099"),
    ] {
        refused(
            &pair,
            &before,
            &add_url(&pair, on, "https://example.org/x", &[]),
            1,
            line,
        );
        // A file on a refused target leaves no file in the store either.
        refused(&pair, &before, &add_file(&pair, on, &png, &[]), 1, line);
    }
    let card_look = add_url(&pair, "\u{0421}-0001", "https://example.org/x", &[]);
    refused(&pair, &before, &card_look, 2, "C-0001");
    assert_eq!(
        id_of(&link_ref(
            &pair,
            "C-0001",
            "https://example.org/second",
            &[]
        )),
        "RF-0002",
        "nothing was taken"
    );
}

/// RF2, "IDs and targets": `[ids]` claiming `RF` exits 2 on every `spec
/// ref` command with "Data"'s line, nothing stored.
#[test]
fn rf2_ids_claiming_rf_exit_2_on_every_ref_command() {
    let pair = Pair::new("rf-rf2-clash", ALMANAC);
    card(&pair, "Call the entries", &["--discipline", "editing"]);
    link_ref(&pair, "C-0001", "https://example.org/before", &[]);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        with_ids_line(&config, "RF = { kind = \"rule\", width = 4 }\n"),
    );
    let before = kept(&pair);
    let png = blob_fixture("lantern.gif");
    let line = "[ids] claims RF, which names references here: rename that prefix";
    for args in [
        vec![
            "ref",
            "add",
            "--on",
            "C-0001",
            "--url",
            "https://example.org/x",
        ],
        vec!["ref", "add", "--on", "C-0001", png.to_str().expect("UTF-8")],
        vec!["ref", "list", "--on", "C-0001"],
        vec!["ref", "get", "RF-0001"],
    ] {
        let done = json_run(&pair, &args);
        refused(&pair, &before, &done, 2, line);
        assert_eq!(done.stdout, "", "{args:?}");
    }
    let (code, output) = delete_on_terminal(&pair, "RF-0001", &[], None);
    assert_eq!(code, 2, "{output}");
    assert!(
        output.contains(line) && !output.contains("[y/N]"),
        "{output}"
    );
    assert_eq!(kept(&pair), before);
}

// ------------------------------------------------------------------ RF3

/// RF3: every stored fixture, through `spec ref add`: its media type,
/// class, serving, facts, size and warning; the file in the store named by
/// its BLAKE3, mode 0400, the store 0700, no `.part` left.
#[test]
fn rf3_every_type_stores_with_its_facts() {
    let pair = Pair::new("rf-rf3-types", KEEP);
    let c1 = card(&pair, "Gather the references", &["--target", "MEC-SPRINT"]);
    let expected = blob_expected();
    let mut names = BTreeSet::new();
    for (name, want) in expected["stored"].as_object().expect("stored") {
        let path = blob_fixture(name);
        let bytes = fs::read(&path).expect("the fixture");
        let stored = file_ref(&pair, &c1, &path, &["--matters", "the shape of it"]);
        assert_eq!(stored["kind"], json!("file"), "{name}");
        assert_eq!(
            stored["name"],
            json!(name),
            "{name}: --name defaults to the base name"
        );
        assert_eq!(stored["media"], want["media"], "{name}");
        assert_eq!(stored["class"], want["class"], "{name}");
        assert_eq!(stored["inline"], want["inline"], "{name}");
        assert_eq!(stored["meta"], want["meta"], "{name}");
        assert_eq!(stored["size"], json!(bytes.len()), "{name}");
        assert_eq!(stored["warning"], want["warning"], "{name}");
        assert_eq!(stored["file"], json!("stored"), "{name}");
        assert_eq!(stored["url"], Value::Null, "{name}");
        assert_eq!(stored["b3"], json!(b3(&bytes)), "{name}");
        let file = store(&pair).join(b3(&bytes));
        assert_eq!(fs::read(&file).expect("stored"), bytes, "{name}: the bytes");
        assert_eq!(mode(&file), 0o400, "{name}");
        names.insert(b3(&bytes));
    }
    assert_eq!(mode(&store(&pair)), 0o700);
    assert_eq!(
        store_files(&store(&pair))
            .into_iter()
            .collect::<BTreeSet<_>>(),
        names
    );
    assert_eq!(parts(&store(&pair)), Vec::<String>::new());
    assert_eq!(
        doc(&pair, &["ref", "list", "--on", &c1])["total"],
        json!(names.len())
    );
}

/// RF3: each refused fixture exits 1 with its line on stderr, nothing
/// stored. M: the type taken from the name's extension (the page named
/// `.png` stored).
#[test]
fn rf3_refused_types_leave_nothing() {
    let pair = Pair::new("rf-rf3-refused", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    link_ref(&pair, &c1, "https://example.org/before", &[]);
    let expected = blob_expected();
    let before = kept(&pair);
    for name in expected["refused"].as_object().expect("refused").keys() {
        let done = add_file(
            &pair,
            &c1,
            &blob_fixture(name),
            &["--matters", "the shape of it"],
        );
        refused(&pair, &before, &done, 1, &refused_line(&expected, name));
    }
    // The page named after an image, under another name too: the name
    // never decides.
    let page = fs::read(blob_fixture("page.html")).expect("page");
    let renamed = scratch_file(&pair, "harbour.png", &page);
    refused(
        &pair,
        &before,
        &add_file(&pair, &c1, &renamed, &[]),
        1,
        "refused: harbour.png: this file is HTML, which is never stored here",
    );
}

/// RF3: `allow = ["image"]` refuses the WAV with "Types"' line and stores
/// the PNG; `allow = ["image/svg+xml"]` exits 2 naming its line on `spec ref
/// add` alone: listing and links' reads go on.
#[test]
fn rf3_allow_narrows_and_a_bad_table_refuses_adds() {
    let pair = Pair::new("rf-rf3-allow", KEEP);
    let c1 = card(&pair, "Gather the references", &["--target", "MEC-SPRINT"]);
    let config = read_text(&pair.main, "specengine.toml");
    write(
        &pair.main,
        "specengine.toml",
        format!("{config}\n[references]\nallow = [\"image\"]\n"),
    );
    let before = kept(&pair);
    refused(
        &pair,
        &before,
        &add_file(
            &pair,
            &c1,
            &blob_fixture("sound.wav"),
            &["--matters", "the hum"],
        ),
        1,
        "refused: sound.wav: audio/wav is not allowed in this project ([references] allow)",
    );
    let png = file_ref(&pair, &c1, &blob_fixture("harbour.png"), &[]);
    assert_eq!(png["media"], json!("image/png"));

    let lines = config.lines().count();
    write(
        &pair.main,
        "specengine.toml",
        format!("{config}\n[references]\nallow = [\"image/svg+xml\"]\n"),
    );
    // The config's lines, a blank one, the table's head, then `allow`.
    let at = lines + 3;
    let line = format!(
        "specengine.toml:{at}: [references] allow: \"image/svg+xml\" is not a type SpecEngine stores"
    );
    let before = kept(&pair);
    refused(
        &pair,
        &before,
        &add_file(&pair, &c1, &blob_fixture("lantern.gif"), &[]),
        2,
        &line,
    );
    refused(
        &pair,
        &before,
        &add_url(&pair, &c1, "https://example.org/x", &[]),
        2,
        &line,
    );
    // Nothing else refuses: the list, a get, a card's show.
    assert_eq!(list(&pair, &c1, &[])["total"], json!(1));
    doc(&pair, &["ref", "get", &id_of(&png)]);
    doc(&pair, &["card", "show", &c1]);
}

// ------------------------------------------------------------------ RF4

/// RF4, the terminal's settings: a file over `SPECENGINE_BLOB_FILE_MAX` is
/// refused with its size before a byte is stored; one passing
/// `SPECENGINE_BLOB_QUOTA` with what is stored; a setting that would widen
/// exits 2 naming it. No refusal stores or emits anything.
#[test]
fn rf4_the_settings_bound_a_terminal() {
    let pair = Pair::new("rf-rf4-terminal", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    let one_mib = png(2000, 2000, 0x0F, Some(1_048_577));
    let big = scratch_file(&pair, "big.png", &one_mib);
    let before = kept(&pair);
    let over = run_env(
        &pair,
        &[
            "--json",
            "ref",
            "add",
            "--on",
            &c1,
            big.to_str().expect("UTF-8"),
        ],
        b"",
        &[("SPECENGINE_BLOB_FILE_MAX", "1048576")],
    );
    refused(
        &pair,
        &before,
        &over,
        1,
        "refused: big.png is 1.0 MiB; a file here is at most 1 MiB",
    );
    let at_cap = run_env(
        &pair,
        &[
            "--json",
            "ref",
            "add",
            "--on",
            &c1,
            big.to_str().expect("UTF-8"),
        ],
        b"",
        &[("SPECENGINE_BLOB_FILE_MAX", "1048577")],
    );
    at_cap.code(0);

    // 1 MiB + 1 stored; a quota of 2 MiB refuses another 1 MiB.
    let second = scratch_file(&pair, "second.png", &png(2000, 2000, 0xF0, Some(1_048_576)));
    let before = kept(&pair);
    let quota = run_env(
        &pair,
        &[
            "--json",
            "ref",
            "add",
            "--on",
            &c1,
            second.to_str().expect("UTF-8"),
        ],
        b"",
        &[("SPECENGINE_BLOB_QUOTA", "2097152")],
    );
    refused(
        &pair,
        &before,
        &quota,
        1,
        "refused: this project's files would pass 2 MiB (1.0 MiB stored); delete some first",
    );
    for (name, value) in [
        ("SPECENGINE_BLOB_QUOTA", "20000000000"),
        ("SPECENGINE_BLOB_FLOOR", "1"),
        ("SPECENGINE_BLOB_FILE_MAX", "268435457"),
    ] {
        let widened = run_env(
            &pair,
            &[
                "--json",
                "ref",
                "add",
                "--on",
                &c1,
                second.to_str().expect("UTF-8"),
            ],
            b"",
            &[(name, value)],
        );
        refused(&pair, &before, &widened, 2, &format!("{name}: {value}"));
    }
}

/// RF4, "Seams": the free-space reader injected: the floor + 64 MiB at the
/// upload's start, the floor − 1 after: a 64 MiB upload passes its start,
/// is refused when 16 MiB are in with "Data"'s line, and its `.part` is gone
/// when the upload drops; the reader was asked again. M: the floor read
/// only at the start.
#[test]
fn rf4_the_floor_is_read_while_reading() {
    let pair = Pair::new("rf-rf4-floor", ALMANAC);
    card(&pair, "Call the entries", &["--discipline", "editing"]);
    let floor = specengine_core::blob::BLOB_FLOOR;
    let calls = Arc::new(AtomicU64::new(0));
    let seen = Arc::clone(&calls);
    let seams = BlobSeams {
        free_space: Arc::new(move |_: &Path| {
            let n = seen.fetch_add(1, Ordering::SeqCst);
            Ok(if n == 0 { floor + 64 * MIB } else { floor - 1 })
        }),
        now: Arc::new(std::time::SystemTime::now),
    };
    let total = 64 * MIB;
    let begun = specengine_cli::blob_begin(
        &env(&pair),
        &Globals::default(),
        &BlobPutRequest {
            name: Some("take.wav".to_owned()),
            declared: total,
            caps: BlobCaps::default(),
            seams,
            flight: BlobFlight::default(),
        },
    )
    .expect("the start runs");
    let BlobBegun::Upload(mut upload) = begun else {
        panic!("refused at the start: {begun:?}");
    };
    assert_eq!(parts(&store(&pair)).len(), 1, "a .part in flight");
    let mut body = SeekWav::new(total, 4);
    let mut buffer = vec![0u8; 65_536];
    let mut sent = 0u64;
    let refused = loop {
        let n = body.read(&mut buffer).expect("generate");
        if n == 0 {
            break None;
        }
        match upload.write(&buffer[..n]) {
            Ok(()) => sent += n as u64,
            Err(BlobFailure::Refused(refusal)) => break Some((sent + n as u64, refusal)),
            Err(other) => panic!("{other:?}"),
        }
    };
    let (at, refusal) = refused.expect("M: the floor read only at the start");
    assert_eq!(at, 16 * MIB, "refused at 16 MiB");
    assert_eq!(refusal.kind, BlobRefusalKind::TooLarge);
    assert!(
        refusal
            .line
            .starts_with("refused: the store keeps 5 GiB free; "),
        "{}",
        refusal.line
    );
    drop(upload);
    assert_eq!(parts(&store(&pair)), Vec::<String>::new(), "its .part gone");
    assert!(
        calls.load(Ordering::SeqCst) >= 2,
        "asked again while reading"
    );
}

// ------------------------------------------------------------------ RF6

/// RF6: one file for equal bytes, its mtime the second write's; two
/// references share it; deleting one keeps it, deleting the other removes
/// it in that call. M: a dedup writing a second file.
#[test]
fn rf6_equal_bytes_share_one_file_until_the_last_reference_goes() {
    let pair = Pair::new("rf-rf6", KEEP);
    let c1 = card(&pair, "Gather the references", &["--target", "MEC-SPRINT"]);
    let c2 = card(&pair, "Draw the harbour", &["--target", "MEC-SPRINT"]);
    let path = blob_fixture("harbour.png");
    let bytes = fs::read(&path).expect("harbour");
    let hash = b3(&bytes);
    let first = file_ref(&pair, &c1, &path, &["--as", "mira"]);
    let file = store(&pair).join(&hash);
    assert_eq!(first["b3"], json!(hash));
    assert!(file.is_file());
    age(&file, 7200);
    assert!(age_of(&file) >= 7199, "premise: an old mtime");
    let copy = scratch_file(&pair, "same-bytes.png", &bytes);
    let second = file_ref(&pair, &c2, &copy, &["--as", "lev"]);
    assert_eq!(second["b3"], json!(hash));
    assert_eq!(second["name"], json!("same-bytes.png"));
    assert_eq!(
        store_files(&store(&pair)),
        std::slice::from_ref(&hash),
        "one file"
    );
    assert!(
        age_of(&file) < 120,
        "its mtime the second write's: {}",
        age_of(&file)
    );
    assert_eq!(mode(&file), 0o400);

    deleted(&pair, &id_of(&first), &["--as", "mira"]);
    assert!(file.is_file(), "another reference names it");
    let get = doc(&pair, &["ref", "get", &id_of(&second)]);
    assert_eq!(get["file"], json!("stored"));
    deleted(&pair, &id_of(&second), &["--as", "lev"]);
    assert!(
        !file.exists(),
        "the last reference gone, the file gone in that call"
    );
    assert_eq!(store_files(&store(&pair)), Vec::<String>::new());
}

// ------------------------------------------------------------------ RF9

/// RF9, the terminal: a WAV, an MP4, a GLB and a PDF without "what
/// matters", or with a blank one, exit 1 with "Data"'s line, nothing
/// stored; with it they store; an image and a link store without it; a
/// `.blend` stores with its warning. M: the rule dropped for `model`.
#[test]
fn rf9_what_matters_is_required_of_four_classes() {
    let pair = Pair::new("rf-rf9", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    let expected = blob_expected();
    for name in [
        "sound.wav",
        "walk.mp4",
        "crate.glb",
        "plan.pdf",
        "scene.blend",
    ] {
        let class = expected["stored"][name]["class"].as_str().expect("a class");
        assert!(needs_matters(class), "premise: {name}");
        let before = kept(&pair);
        refused(
            &pair,
            &before,
            &add_file(&pair, &c1, &blob_fixture(name), &[]),
            1,
            MATTERS_REQUIRED,
        );
        refused(
            &pair,
            &before,
            &add_file(&pair, &c1, &blob_fixture(name), &["--matters", "  \n\t"]),
            1,
            MATTERS_REQUIRED,
        );
        let stored = file_ref(
            &pair,
            &c1,
            &blob_fixture(name),
            &["--matters", "the pace of it"],
        );
        assert_eq!(stored["matters"], json!("the pace of it"), "{name}");
    }
    // `--matters-file -` reads stdin.
    let piped = pair.spec_piped(
        &pair.main,
        &[
            "--json",
            "ref",
            "add",
            "--on",
            &c1,
            blob_fixture("bell.flac").to_str().expect("UTF-8"),
            "--matters-file",
            "-",
        ],
        b"the ring, not the room",
    );
    piped.code(0);
    assert_eq!(piped.json()["matters"], json!("the ring, not the room"));
    let image = file_ref(&pair, &c1, &blob_fixture("lantern.gif"), &[]);
    assert_eq!(image["matters"], Value::Null);
    let link = link_ref(&pair, &c1, "https://example.org/board", &[]);
    assert_eq!(link["matters"], Value::Null);
    let blend = file_ref(
        &pair,
        &c1,
        &blob_fixture("scene.blend"),
        &["--matters", "the rig"],
    );
    assert_eq!(blend["warning"], json!(WARNING));
    assert_eq!(blend["inline"], json!(false));
    let text = text_run(&pair, &["ref", "get", &id_of(&blend)]);
    text.code(0);
    assert!(
        text.stdout.contains(&format!("\n  warning: {WARNING}\n")),
        "{}",
        text.stdout
    );
}

// ----------------------------------------------------------------- RF12

/// RF12, the terminal: `spec ref delete` with stdin no terminal exits 2, a
/// declined prompt exits 1, both with no event; the prompt names the
/// reference, its author, target, type and size; a second delete exits 1.
#[test]
fn rf12_a_terminal_delete_asks_and_needs_a_terminal() {
    let pair = Pair::new("rf-rf12", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    let id = id_of(&file_ref(
        &pair,
        &c1,
        &blob_fixture("harbour.png"),
        &["--as", "mira"],
    ));
    let before = kept(&pair);

    let piped = json_run(&pair, &["ref", "delete", &id]);
    refused(&pair, &before, &piped, 2, "");
    assert_eq!(piped.stdout, "", "{}", piped.show());
    let (code, output) = delete_on_terminal(&pair, &id, &[], Some("n"));
    assert_eq!(code, 1, "{output}");
    assert!(
        output.contains(&format!(
            "delete {id} (mira, on {c1}, image/png 1.5 KiB)? Its file goes when no other \
             reference names it. [y/N]"
        )),
        "{output}"
    );
    assert_eq!(kept(&pair), before, "a declined prompt changes nothing");

    let events = pair.events().len();
    deleted(&pair, &id, &["--as", "Olga R"]);
    assert_eq!(
        events_after(&pair, events),
        [(
            "reference.deleted".to_owned(),
            json!({"id": id, "as": "Olga R"})
        )]
    );
    let tombstone = doc(&pair, &["ref", "get", &id]);
    assert_eq!(tombstone["deleted"]["by"], json!("Olga R"));
    assert!(tombstone["deleted"]["at"].is_string());
    for key in [
        "title", "matters", "url", "name", "b3", "agent_b3", "meta", "file",
    ] {
        assert_eq!(tombstone[key], Value::Null, "a deleted reference's {key}");
    }
    let before = kept(&pair);
    let (code, output) = delete_on_terminal(&pair, &id, &[], Some("y"));
    assert_eq!(code, 1, "{output}");
    assert!(
        output.contains(&format!("{id} is deleted: nothing changed")),
        "{output}"
    );
    assert_eq!(kept(&pair), before);
    let (code, output) = delete_on_terminal(&pair, "RF-0099", &[], Some("y"));
    assert_eq!(code, 1, "{output}");
}

// ----------------------------------------------------------------- RF13

/// RF13: title, "what matters" and name each holding a marker no other
/// byte of the queue holds; deleted, the WAL checkpointed: no marker in the
/// database file, its WAL or the dump; the row keeps `deleted_by`,
/// `deleted_at`; the file and its agent copy, named by no other row, gone
/// in the same call.
#[test]
fn rf13_a_delete_erases_its_texts_and_collects_its_files() {
    let pair = Pair::new("rf-rf13-erase", KEEP);
    let c1 = card(&pair, "Gather the references", &["--target", "MEC-SPRINT"]);
    let title = "harbourmaster-9f2a-title";
    let matters = "saltwind-41cc-what-matters";
    let name = "tidepool-77ab-name.png";
    let original = png(4032, 3024, 0x33, None);
    let small = png(1568, 1176, 0x33, None);
    let file = scratch_file(&pair, "original.png", &original);
    let copy = scratch_file(&pair, "copy.png", &small);
    let made = file_ref(
        &pair,
        &c1,
        &file,
        &[
            "--title",
            title,
            "--matters",
            matters,
            "--name",
            name,
            "--agent-copy",
            copy.to_str().expect("UTF-8"),
            "--as",
            "mira",
        ],
    );
    let id = id_of(&made);
    assert_eq!(made["agent_b3"], json!(b3(&small)));
    assert_eq!(made["name"], json!(name));
    let stored = store(&pair);
    assert_eq!(
        store_files(&stored).into_iter().collect::<BTreeSet<_>>(),
        [b3(&original), b3(&small)]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    let db = pair.db();
    let wal = db.with_extension("db-wal");
    let any = |marker: &str| {
        holds(&fs::read(&db).unwrap_or_default(), marker)
            || holds(&fs::read(&wal).unwrap_or_default(), marker)
    };
    assert!(
        any(title) && any(matters) && any(name),
        "premise: the markers are stored"
    );

    deleted(&pair, &id, &["--as", "mira"]);
    assert_eq!(
        store_files(&stored),
        Vec::<String>::new(),
        "both files in that call"
    );
    pair.sql("PRAGMA wal_checkpoint(TRUNCATE);");
    for marker in [title, matters, name] {
        assert!(!any(marker), "{marker} still in the database or its WAL");
    }
    assert_eq!(
        pair.sql(&format!(
            "SELECT deleted_by, deleted_at IS NOT NULL, title IS NULL, matters IS NULL, \
             name IS NULL, b3 IS NULL, agent_b3 IS NULL, meta IS NULL, url IS NULL, \
             media, size, kind FROM refs WHERE id = '{id}';"
        ))
        .trim(),
        format!("mira|1|1|1|1|1|1|1|1|image/png|{}|file", original.len())
    );
    let out = pair.scratch.dir("dumps").join("erased.jsonl");
    let dumped = run_env(
        &pair,
        &["export", "state", "--out", out.to_str().expect("UTF-8")],
        b"",
        &[],
    );
    dumped.code(0);
    let dump = fs::read(&out).expect("the dump");
    for marker in [title, matters, name, &b3(&original), &b3(&small)] {
        assert!(!holds(&dump, marker), "{marker} in the dump");
    }
}

/// RF13, "Collection": with `.swept` over an hour old, `spec ref add`
/// sweeps a file no row names whose mtime is 24 h + 1 s old and a `.part`
/// 6 h + 1 s old, keeping an unnamed file 23 h old, a named one 48 h old
/// and a `.part` 5 h old; `.swept` is touched. Within the hour, nothing is
/// swept.
#[test]
fn rf13_the_terminals_sweep() {
    let pair = Pair::new("rf-rf13-sweep", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    let named = fs::read(blob_fixture("lantern.gif")).expect("gif");
    file_ref(&pair, &c1, &blob_fixture("lantern.gif"), &[]);
    let stored = store(&pair);
    let plant = |bytes: &[u8], seconds: u64| -> String {
        let hash = b3(bytes);
        let path = stored.join(&hash);
        fs::write(&path, bytes).expect("plant");
        age(&path, seconds);
        hash
    };
    let stale = plant(&png(64, 64, 0x01, None), 24 * 3600 + 1);
    let young = plant(&png(64, 64, 0x02, None), 23 * 3600);
    age(&stored.join(b3(&named)), 48 * 3600);
    fs::create_dir_all(stored.join(".incoming")).expect("incoming");
    let old_part = stored.join(".incoming").join("00112233445566ff.part");
    let new_part = stored.join(".incoming").join("ffeeddccbbaa9988.part");
    fs::write(&old_part, b"half an upload").expect("part");
    fs::write(&new_part, b"another half").expect("part");
    age(&old_part, 6 * 3600 + 1);
    age(&new_part, 5 * 3600);
    let swept = stored.join(".swept");
    fs::write(&swept, b"").expect(".swept");

    // Within the hour: nothing swept.
    age(&swept, 1800);
    link_ref(&pair, &c1, "https://example.org/a", &[]);
    file_ref(&pair, &c1, &blob_fixture("sea-lossy.webp"), &[]);
    assert!(stored.join(&stale).exists(), "swept within the hour");
    assert!(old_part.exists());

    age(&swept, 3601);
    file_ref(&pair, &c1, &blob_fixture("sea-lossless.webp"), &[]);
    assert!(
        !stored.join(&stale).exists(),
        "a file 24 h + 1 s old, named by no row"
    );
    assert!(!old_part.exists(), "a .part 6 h + 1 s old");
    assert!(stored.join(&young).exists(), "23 h: kept");
    assert!(new_part.exists(), "a .part 5 h old: kept");
    assert!(
        stored.join(b3(&named)).exists(),
        "a named file is never swept"
    );
    assert!(age_of(&swept) < 120, ".swept touched: {}", age_of(&swept));
}

// ----------------------------------------------------------------- RF14

/// The almanac's card `title` on `targets`, made through the library at
/// [`LATER`]: its ID.
fn cut(pair: &Pair, title: &str, targets: &[&str]) -> String {
    let outcome = card_new(
        &env(pair),
        &Globals::default(),
        &CardNewRequest {
            title: title.to_owned(),
            discipline: Some("editing".to_owned()),
            targets: targets.iter().map(|target| (*target).to_owned()).collect(),
            now: LATER.to_owned(),
            ..CardNewRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("card new {title}: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
    outcome.id.expect("a card's ID")
}

type Labelled = Vec<(String, Value)>;

/// The almanac with two cards, `refs` references on the first and three on
/// the second when `refs` is not zero, then `refs` renamed away: a
/// statement on it fails. Then, at fixed clocks, the first assigned, moved
/// to review, accepted, its target approved and cleared, the second
/// cancelled; after each, `card show`'s document and a plain show's text.
fn rf14_scene(label: &str, refs: usize) -> (Labelled, Labelled) {
    let pair = Pair::new(label, ALMANAC);
    let first = cut(&pair, "Call the entries", &["STEP-CALL"]);
    let second = cut(&pair, "Sort the entries", &["STEP-SORT"]);
    if refs > 0 {
        let gif = blob_fixture("lantern.gif");
        for n in 0..refs {
            if n % 5 == 0 {
                file_ref(&pair, &first, &gif, &["--title", &format!("Lamp {n}")]);
            } else {
                link_ref(
                    &pair,
                    &first,
                    &format!("https://example.org/{n}"),
                    &["--as", "mira"],
                );
            }
        }
        for n in 0..3 {
            link_ref(
                &pair,
                &second,
                &format!("https://example.org/sort/{n}"),
                &[],
            );
        }
        pair.sql("ALTER TABLE refs RENAME TO refs_gone;");
    }
    let skip = pair.events().len();
    let globals = Globals::default();
    let shown = |pair: &Pair, id: &str| {
        let plain = text_run(pair, &["card", "show", id]);
        plain.code(0);
        let mut document = doc(pair, &["card", "show", id]);
        document["plain_text"] = json!(plain.stdout);
        document
    };
    let mut documents = Vec::new();
    let assigned = card_assign(
        &env(&pair),
        &globals,
        &CardAssignRequest {
            id: first.clone(),
            assignee: Some("ana".to_owned()),
            reviewer: Some("ben".to_owned()),
            now: LATER.to_owned(),
            ..CardAssignRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("assign: {error}"));
    assert_eq!(assigned.exit(), Exit::Answered, "{assigned:?}");
    documents.push(("assign".to_owned(), shown(&pair, &first)));
    let moved = card_move(
        &env(&pair),
        &globals,
        &CardMoveRequest {
            id: first.clone(),
            to: Some("review".to_owned()),
            now: LATER.to_owned(),
            ..CardMoveRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("move: {error}"));
    assert_eq!(moved.exit(), Exit::Answered, "{moved:?}");
    documents.push(("move".to_owned(), shown(&pair, &first)));
    let mut consent = |_: &str| true;
    let accepted = card_accept(
        &env(&pair),
        &globals,
        &CardAcceptRequest {
            id: first.clone(),
            actor: "ben".to_owned(),
            now: LATER.to_owned(),
            briefed: true,
            ..CardAcceptRequest::default()
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("accept: {error}"));
    assert_eq!(accepted.exit(), Exit::Answered, "{accepted:?}");
    documents.push(("accept".to_owned(), shown(&pair, &first)));
    let change = pair.propose_edit(
        &pair.main,
        "STEP-CALL",
        "on the first of the month",
        "on the last of the month",
    );
    pair.approve_ok(&pair.main, &change);
    documents.push(("approve".to_owned(), shown(&pair, &first)));
    let mut consent = |_: &str| true;
    let cleared = card_clear(
        &env(&pair),
        &globals,
        &CardClearRequest {
            id: first.clone(),
            outcome: CardClearOutcome::NoEffect,
            reason: "The date moved, the call did not.".to_owned(),
            estimate: None,
            title: None,
            revision: None,
            actor: Some("ben".to_owned()),
            now: LATER.to_owned(),
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("clear: {error}"));
    assert_eq!(cleared.exit(), Exit::Answered, "{cleared:?}");
    documents.push(("clear".to_owned(), shown(&pair, &first)));
    let cancelled = card_cancel(
        &env(&pair),
        &globals,
        &CardCancelRequest {
            id: second.clone(),
            reason: "Sorted with the call.".to_owned(),
            now: LATER.to_owned(),
            ..CardCancelRequest::default()
        },
    )
    .unwrap_or_else(|error| panic!("cancel: {error}"));
    assert_eq!(cancelled.exit(), Exit::Answered, "{cancelled:?}");
    documents.push(("cancel".to_owned(), shown(&pair, &second)));
    documents.push(("list".to_owned(), doc(&pair, &["card", "list"])));

    let scratch = pair.scratch.path().to_string_lossy().into_owned();
    let masked = |value: Value| -> Value {
        serde_json::from_str(&value.to_string().replace(&scratch, "<scratch>")).expect("JSON")
    };
    let documents = documents
        .into_iter()
        .map(|(step, document)| (step, masked(document)))
        .collect();
    let events = events_after(&pair, skip)
        .into_iter()
        .map(|(kind, payload)| (kind, masked(payload)))
        .collect();
    (documents, events)
}

/// RF14: with 50 references on a card and none, its assign, move, accept,
/// an approve of its target, its clear and another card's cancel give the
/// same documents, plain shows and events; with `refs` renamed away every
/// one of them still runs: none executes a statement on it. M:
/// `change_card` reads `refs`.
#[test]
fn rf14_nothing_reads_a_reference() {
    let (with_none, events_none) = rf14_scene("rf-rf14-none", 0);
    let (with_refs, events_refs) = rf14_scene("rf-rf14-refs", 50);
    assert_eq!(with_refs, with_none, "the documents");
    assert_eq!(events_refs, events_none, "the events");
    assert!(
        events_none
            .iter()
            .all(|(kind, _)| !kind.starts_with("reference.")),
        "{events_none:?}"
    );
}

// ----------------------------------------------------------------- RF15

/// The lines of `card show --context`'s text from `context.references:` up
/// to (not including) `context.bundle:`.
fn reference_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with("context.references: "))
        .unwrap_or_else(|| panic!("a context.references line: {text}"));
    // The references' lines end where the spec's begin
    // (docs/features/reference-promotion.md: `context.spec-references:`
    // after `context.images:`), else at the bundle's.
    let end = lines
        .iter()
        .position(|line| {
            line.starts_with("context.spec-references: ") || line.starts_with("context.bundle: ")
        })
        .expect("a context.spec-references or context.bundle line");
    assert!(start < end, "before the bundle: {text}");
    let comments = lines
        .iter()
        .position(|line| line.starts_with("context.comments: "))
        .expect("a context.comments line");
    assert!(comments < start, "after the comments' lines: {text}");
    lines[start..end]
        .iter()
        .map(|line| (*line).to_owned())
        .collect()
}

/// "A card's context": the `context.reference:` line of a reference
/// document (a person's, its title and "what matters" given).
fn context_line(reference: &Value) -> String {
    let who = reference["actor"].as_str().expect("an actor");
    let matters = reference["matters"].as_str().map(|matters| {
        let one = matters.replace("\r\n", " ").replace(['\n', '\r'], " ");
        if one.chars().count() > 512 {
            format!("{} [cut]", one.chars().take(512).collect::<String>())
        } else {
            one
        }
    });
    let title = reference["title"].as_str().expect("a title");
    let mut parts = vec![
        id_of(reference),
        who.to_owned(),
        reference["created_at"].as_str().expect("a time").to_owned(),
    ];
    if reference["kind"] == json!("link") {
        parts.push("link".to_owned());
        parts.push(title.to_owned());
        parts.push(reference["url"].as_str().expect("a URL").to_owned());
    } else {
        let mut kind = reference["media"].as_str().expect("a media").to_owned();
        let facts = facts_text(&reference["meta"]);
        if !facts.is_empty() {
            kind.push(' ');
            kind.push_str(&facts);
        }
        if reference["class"] == json!("image") {
            if reference["agent_b3"].is_string() {
                kind.push_str(", agent copy");
            } else if agent_ready(reference) {
                kind.push_str(", agent image");
            }
        }
        parts.push(kind);
        parts.push(title.to_owned());
        parts.push(reference["name"].as_str().expect("a name").to_owned());
    }
    if let Some(matters) = matters {
        parts.push(format!("matters: {matters}"));
    }
    format!("context.reference: {}", parts.join(" | "))
}

/// The IDs of the `context.images:` line among `lines` (`-`: none), its
/// tail "Data"'s; `None` when there is no such line.
fn images_line(lines: &[String]) -> Option<Vec<String>> {
    let line = lines
        .iter()
        .find(|line| line.starts_with("context.images: "))?;
    let rest = &line["context.images: ".len()..];
    if rest == "-" {
        return Some(Vec::new());
    }
    let (ids, tail) = rest.split_once("; ").unwrap_or_else(|| panic!("{line}"));
    assert_eq!(
        tail, "get_card attaches these images, in this order",
        "{line}"
    );
    Some(
        ids.split([',', ' '])
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// "The agent copy": an image whose sizes are known, its long edge at most
/// 1 568 and its size at most 1 MiB.
fn agent_ready(reference: &Value) -> bool {
    let meta = &reference["meta"];
    match (
        meta["width"].as_u64(),
        meta["height"].as_u64(),
        reference["size"].as_u64(),
    ) {
        (Some(width), Some(height), Some(size)) => width.max(height) <= 1568 && size <= 1_048_576,
        _ => false,
    }
}

/// `card show <id> --context` (text and JSON).
fn context(pair: &Pair, id: &str) -> (Run, Value) {
    let text = text_run(pair, &["card", "show", id, "--context"]);
    text.code(0);
    let json = json_run(pair, &["card", "show", id, "--context"]);
    json.code(0);
    (text, json.json())
}

/// The context's notes on the text's stderr (`note: context: …`) or among
/// its lines (`context: …`), as `context: …`.
fn context_notes(done: &Run) -> Vec<String> {
    done.stderr
        .lines()
        .filter_map(|line| line.strip_prefix("note: "))
        .chain(done.stdout.lines())
        .filter(|line| line.starts_with("context: ") && line.contains("reference"))
        .map(str::to_owned)
        .collect()
}

/// RF15: 12 references (5 images, 2 audio, 5 links), one "what matters" of
/// 2 048 bytes: the newest 10, oldest first, with the note for the 2 left
/// out; `context.images:` the newest three agent-ready images of those
/// listed, newest first; the long one cut at 512 ` [cut]`; `bundle_hash`
/// as with none; the JSON's listed keys. A card with none prints
/// `context.references: -` and no images line.
#[test]
fn rf15_the_context_lists_the_newest_ten() {
    let pair = Pair::new("rf-rf15-ten", KEEP);
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &[
            "--target",
            "MEC-SPRINT",
            "--goal",
            "Readers learn the sprint.",
        ],
    );
    let (bare_text, bare) = context(&pair, &c1);
    assert_eq!(
        reference_lines(&bare_text.stdout),
        ["context.references: -"]
    );
    let hash = bare["context"]["bundle"]["bundle_hash"].clone();
    assert!(hash.is_string(), "{bare}");

    let long = format!("Long: {}", "s".repeat(2042));
    assert_eq!(long.len(), 2048);
    let mut made: Vec<Value> = Vec::new();
    let plan: [(&str, &str); 12] = [
        ("link", "https://example.org/board"),
        ("file", "lantern.gif"),
        ("link", "https://example.org/palette"),
        ("file", "sound.wav"),
        ("file", "sea-lossy.webp"),
        ("link", "https://example.org/map"),
        ("file", "harbour.png"),
        ("file", "bell.flac"),
        ("link", "https://example.org/tide"),
        ("file", "sea-extended.webp"),
        ("file", "dusk.jpg"),
        ("link", "https://example.org/last"),
    ];
    for (n, (kind, what)) in plan.iter().enumerate() {
        let title = format!("Reference {n}");
        let matters = if n == 6 {
            long.clone()
        } else {
            format!("the part numbered {n}")
        };
        let extra = [
            "--title",
            title.as_str(),
            "--matters",
            matters.as_str(),
            "--as",
            "mira",
        ];
        made.push(if *kind == "link" {
            link_ref(&pair, &c1, what, &extra)
        } else {
            file_ref(&pair, &c1, &blob_fixture(what), &extra)
        });
    }
    let (text, json) = context(&pair, &c1);
    let newest = &made[2..];
    let mut want = vec![format!(
        "context.references: 10 of 12, the newest; {MATERIAL}"
    )];
    want.extend(newest.iter().map(context_line));
    let got = reference_lines(&text.stdout);
    let got_lines: Vec<&String> = got
        .iter()
        .filter(|line| !line.starts_with("context: ") && !line.starts_with("context.images: "))
        .collect();
    assert_eq!(
        got_lines,
        want.iter().collect::<Vec<_>>(),
        "{}",
        text.stdout
    );
    // The agent-ready images among the listed, newest first: the extended
    // WebP, the lossy WebP; the PNG and the JPEG are over 1 568 px; the GIF
    // is not listed.
    assert_eq!(
        images_line(&got),
        Some(vec![id_of(&made[9]), id_of(&made[4])]),
        "{}",
        text.stdout
    );
    assert_eq!(
        got.iter()
            .position(|line| line.starts_with("context.images: ")),
        Some(11),
        "after the reference lines"
    );
    assert!(
        want.iter().any(|line| line.contains(" [cut]")),
        "the long one is cut"
    );
    assert_eq!(
        context_notes(&text),
        [format!(
            "context: 2 older references not shown: list_references on {c1} has them"
        )]
    );
    let references = &json["context"]["references"];
    assert_eq!(references["total"], json!(12), "{references}");
    let listed = references["listed"].as_array().expect("listed");
    assert_eq!(
        listed.iter().map(id_of).collect::<Vec<_>>(),
        newest.iter().map(id_of).collect::<Vec<_>>()
    );
    assert_eq!(
        references["images"],
        json!([id_of(&made[9]), id_of(&made[4])])
    );
    let raw = json_run(&pair, &["card", "show", &c1, "--context"]);
    let block = &raw.stdout[raw.stdout.find("\"references\":{").expect("references")..];
    let first_listed = &block[block.find("\"listed\":[").expect("listed") + 10..];
    assert_eq!(
        keys_in_order(first_listed),
        [
            "id",
            "actor",
            "author",
            "created_at",
            "kind",
            "title",
            "matters",
            "url",
            "name",
            "media",
            "class",
            "size",
            "meta",
            "agent_image",
            "cut"
        ]
    );
    for (entry, reference) in listed.iter().zip(newest) {
        let cut = reference["matters"]
            .as_str()
            .is_some_and(|matters| matters.chars().count() > 512);
        assert_eq!(entry["cut"], json!(cut), "{entry}");
    }
    assert_eq!(
        json["context"]["bundle"]["bundle_hash"], hash,
        "references stay outside the bundle"
    );
}

/// RF15: the 4 096-character bound: "what matters" of 500 characters each,
/// the newest that fit with each line's end counted, the first that does
/// not fit and every older one left out, the note counting them.
#[test]
fn rf15_the_lines_stay_within_4096_characters() {
    let pair = Pair::new("rf-rf15-bound", ALMANAC);
    let c1 = card(
        &pair,
        "Call the entries",
        &["--discipline", "editing", "--target", "STEP-CALL"],
    );
    let made: Vec<Value> = (1..=10)
        .map(|n| {
            let matters = format!("{n:02} {}", "w".repeat(497));
            link_ref(
                &pair,
                &c1,
                &format!("https://example.org/{n}"),
                &[
                    "--title",
                    &format!("Board {n}"),
                    "--matters",
                    &matters,
                    "--as",
                    "mira",
                ],
            )
        })
        .collect();
    let lines: Vec<String> = made.iter().map(context_line).collect();
    let mut fit = 0;
    let mut used = 0;
    for line in lines.iter().rev() {
        let cost = line.chars().count() + 1;
        if used + cost > 4096 {
            break;
        }
        used += cost;
        fit += 1;
    }
    assert!(fit < 10 && fit > 0, "premise: the bound binds ({fit})");
    let (text, json) = context(&pair, &c1);
    let mut want = vec![format!(
        "context.references: {fit} of 10, the newest; {MATERIAL}"
    )];
    want.extend(lines[10 - fit..].iter().cloned());
    let all = reference_lines(&text.stdout);
    assert_eq!(images_line(&all), Some(Vec::new()), "no agent-ready image");
    let got: Vec<String> = all
        .into_iter()
        .filter(|line| !line.starts_with("context: ") && !line.starts_with("context.images: "))
        .collect();
    assert_eq!(got, want);
    assert_eq!(
        context_notes(&text),
        [format!(
            "context: {} older references not shown: list_references on {c1} has them",
            10 - fit
        )]
    );
    assert_eq!(json["context"]["references"]["total"], json!(10));
    assert_eq!(
        json["context"]["references"]["listed"]
            .as_array()
            .map(Vec::len),
        Some(fit)
    );
}

/// About `chars` characters of plain prose, marked with `mark`.
fn prose(mark: &str, chars: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    while out.len() < chars {
        n += 1;
        out.push_str(&format!(
            "The {mark} keeper trims wick number {n} before the tide turns, and the lamp burns \
             steady through watch {n}.\n"
        ));
        if n % 6 == 0 {
            out.push('\n');
        }
    }
    out
}

/// RF15: the reference lines count in `U`: a card whose own lines are
/// about 25 000 characters, three long targets (one of 11 500 characters,
/// which fits the room only if the reference lines are not counted), 10
/// links whose lines total about 4 000 characters: the text before the
/// body, reference lines included, plus the bundle's characters stay within
/// 40 000. M: the lines left out of `U`.
#[test]
fn rf15_the_reference_lines_count_in_the_room() {
    let pair = Pair::new("rf-rf15-room", KEEP);
    let cwd = pair.main.clone();
    for (name, size) in [("LONG-A", 11_500), ("LONG-B", 16_000), ("LONG-C", 16_000)] {
        write(
            &cwd,
            &format!("docs/spec/long/{}.md", name.to_lowercase()),
            format!(
                "---\nid: MEC-{name}\nkind: mechanic\nclass: canon\ntier: 2\ntitle: Long \
                 {name}\nparent: DOM-MOVEMENT\nstatus: accepted\nowner: owner\nreviewed: \
                 2026-09-20\n---\n\n# Long {name}\n\n{}",
                prose(name, size)
            ),
        );
    }
    pair.commit_all(&cwd, "Three long documents.");
    let mut args = vec![
        "card",
        "new",
        "--title",
        "The long watch",
        "--target",
        "MEC-LONG-A",
        "--target",
        "MEC-LONG-B",
        "--target",
        "MEC-LONG-C",
    ];
    let criteria: Vec<String> = (0..25)
        .map(|i| {
            let mut line = format!("Criterion {i}: the lamp stays lit");
            while line.len() < 1000 {
                line.push_str(" and steady");
            }
            line.truncate(1000);
            line
        })
        .collect();
    for criterion in &criteria {
        args.extend(["--criterion", criterion.as_str()]);
    }
    let c1 = id_of(&doc(&pair, &args));
    let (before_text, before) = context(&pair, &c1);
    let before_bundle = before["context"]["bundle"].clone();
    let lines_before: usize = reference_lines(&before_text.stdout)
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    for n in 0..10 {
        link_ref(
            &pair,
            &c1,
            &format!("https://example.org/watch/{n}"),
            &[
                "--title",
                &format!("Watch {n}"),
                "--matters",
                &format!("{n:02} {}", "watch the tide ".repeat(21)),
                "--as",
                "mira",
            ],
        );
    }
    let (text, json) = context(&pair, &c1);
    let lines = reference_lines(&text.stdout);
    let chars: usize = lines.iter().map(|line| line.chars().count() + 1).sum();
    assert!(chars > 3000, "premise: {chars}");
    let room = |stdout: &str| {
        let at = stdout.find("\ncontext.bundle: ").expect("a bundle line");
        let end = at + 1 + stdout[at + 1..].find('\n').expect("its end") + 1;
        stdout[..end].chars().count()
    };
    let bundle = &json["context"]["bundle"];
    assert!(bundle.is_object(), "a bundle is made: {}", json["context"]);
    let bundle_chars = usize::try_from(bundle["chars"].as_u64().expect("chars")).expect("usize");
    let before_chars =
        usize::try_from(before_bundle["chars"].as_u64().expect("chars")).expect("usize");
    assert!(
        room(&text.stdout) + before_chars > 40_000,
        "premise: the bundle made without the lines would not fit with them: U {} + {before_chars}",
        room(&text.stdout)
    );
    assert!(
        room(&text.stdout) + bundle_chars <= 40_000,
        "M: the lines left out of U: U {} + chars {bundle_chars} > 40 000",
        room(&text.stdout)
    );
    // The bundle's own line may change with the bundle: 84 characters of
    // slack.
    assert!(
        room(&text.stdout) + 84 >= room(&before_text.stdout) + chars - lines_before,
        "U grows by the lines ({} -> {} characters): {} -> {}",
        lines_before,
        chars,
        room(&before_text.stdout),
        room(&text.stdout)
    );
    assert!(before_bundle.is_object(), "premise: a bundle with none");
    assert!(text.stdout.chars().count() <= 40_000);
}

/// RF15: a task two cards list carries both cards' references, the newest
/// 10 across them oldest first, each naming its card; a third card's stays
/// out; `schema_version` stays 1; the brief's header and its tail line.
#[test]
fn rf15_the_task_package_carries_its_cards_references() {
    let pair = Pair::new("rf-rf15-package", KEEP);
    let task = id_of(&doc(
        &pair,
        &[
            "task",
            "new",
            "--nodes",
            "MEC-SPRINT",
            "--title",
            "Tune the sprint",
        ],
    ));
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    let c2 = card(&pair, "Draw the sprint meter", &["--target", "MEC-SPRINT"]);
    let c3 = card(&pair, "Unrelated", &["--target", "MEC-STAMINA"]);
    for card_id in [&c1, &c2] {
        doc(&pair, &["card", "edit", card_id, "--task", &task]);
    }
    let bare = doc(&pair, &["task", "show", &task]);
    assert_eq!(bare["schema_version"], json!(1));

    let mut made = Vec::new();
    for n in 1..=12 {
        let on = if n % 2 == 0 { &c2 } else { &c1 };
        made.push((
            on.clone(),
            link_ref(
                &pair,
                on,
                &format!("https://example.org/{n}"),
                &["--title", &format!("Board {n}"), "--matters", "the layout"],
            ),
        ));
        if n == 6 {
            link_ref(&pair, &c3, "https://example.org/other", &[]);
        }
    }
    let package = doc(&pair, &["task", "show", &task]);
    assert_eq!(package["schema_version"], json!(1), "a new key keeps it");
    let references = &package["references"];
    assert!(references.is_object(), "M: the key dropped: {package}");
    assert_eq!(references["total"], json!(12), "{references}");
    let listed = references["listed"].as_array().expect("listed");
    assert_eq!(
        listed
            .iter()
            .map(|entry| (entry["on"].clone(), entry["id"].clone()))
            .collect::<Vec<_>>(),
        made[2..]
            .iter()
            .map(|(on, reference)| (json!(on), reference["id"].clone()))
            .collect::<Vec<_>>(),
        "the newest 10 across both cards, oldest first, each naming its card"
    );
    let brief = text_run(&pair, &["task", "show", &task]);
    brief.code(0);
    assert!(
        brief
            .stdout
            .contains(&format!("References: 10 of 12, the newest; {MATERIAL}")),
        "{}",
        brief.stdout
    );
    assert!(
        brief
            .stdout
            .contains("  get_reference shows a file's details and its image"),
        "{}",
        brief.stdout
    );
    let last = &made[11].1;
    assert!(
        brief.stdout.contains(&format!(
            "  {} | local | {} | on {c2} | link | Board 12 | matters: the layout",
            id_of(last),
            last["created_at"].as_str().expect("a time")
        )) || brief.stdout.contains(&format!("  {} | ", id_of(last))),
        "{}",
        brief.stdout
    );
}

// ----------------------------------------------------------------- RF16

/// The `spec feed` text lines of `type`.
fn feed_lines(pair: &Pair, kind: &str) -> Vec<String> {
    let done = text_run(pair, &["feed"]);
    done.code(0);
    done.stdout
        .lines()
        .filter(|line| line.split(" | ").nth(2) == Some(kind))
        .map(str::to_owned)
        .collect()
}

/// RF16: each attach and delete adds exactly one `reference.*` event, its
/// payload "Data"'s and no title, URL, name, hash or "what matters"; a
/// refused write none; the repository untouched; `spec feed` words both
/// types. M: the title in the payload.
#[test]
fn rf16_one_event_a_write_and_no_text() {
    let pair = Pair::new("rf-rf16", KEEP);
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    let repository = pair.state();
    let events = pair.events().len();
    let bytes = fs::read(blob_fixture("sound.wav")).expect("wav");
    let file = file_ref(
        &pair,
        &c1,
        &blob_fixture("sound.wav"),
        &[
            "--title",
            "Harbour hum",
            "--matters",
            "the low hum, not the gulls",
            "--as",
            "mira",
        ],
    );
    let link = link_ref(
        &pair,
        "RULE-SPRINT-COST",
        "https://example.org/moodboard",
        &["--title", "Moodboard"],
    );
    assert_eq!(
        events_after(&pair, events),
        [
            (
                "reference.created".to_owned(),
                json!({"id": id_of(&file), "on": c1, "as": "mira"})
            ),
            (
                "reference.created".to_owned(),
                json!({"id": id_of(&link), "on": "RULE-SPRINT-COST", "as": "local"})
            ),
        ],
        "M: the title in the payload"
    );
    let events = pair.events().len();
    deleted(&pair, &id_of(&file), &["--as", "mira"]);
    assert_eq!(
        events_after(&pair, events),
        [(
            "reference.deleted".to_owned(),
            json!({"id": id_of(&file), "as": "mira"})
        )]
    );
    let raw = pair.sql("SELECT payload FROM events WHERE type LIKE 'reference.%';");
    for word in [
        "Harbour",
        "hum",
        "gulls",
        "Moodboard",
        "example.org",
        "sound.wav",
        &b3(&bytes)[..16],
        "matters",
        "title",
        "url",
    ] {
        assert!(!raw.contains(word), "{word} in a payload: {raw}");
    }
    let before = pair.events().len();
    add_url(&pair, "C-0099", "https://example.org/x", &[]).code(1);
    assert_eq!(pair.events().len(), before, "a refused write: no event");
    assert_eq!(pair.state(), repository, "the repository untouched");
    // "Events and feed": `RF-0012 attached to C-0007 by mira`, the
    // reference the line's subject (as a comment's: spec-feed.md "Words").
    let worded = |kind: &str, words: &str| -> usize {
        feed_lines(&pair, kind)
            .iter()
            .filter(|line| {
                let parts: Vec<&str> = line.split(" | ").collect();
                parts.get(3) == Some(&id_of(&file).as_str()) && parts.last() == Some(&words)
            })
            .count()
    };
    assert_eq!(
        worded("reference.created", &format!("attached to {c1} by mira")),
        1,
        "{:?}",
        feed_lines(&pair, "reference.created")
    );
    assert_eq!(
        worded("reference.deleted", "deleted by mira"),
        1,
        "{:?}",
        feed_lines(&pair, "reference.deleted")
    );
}

// ----------------------------------------------------------------- RF17

fn dump_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8(bytes.to_vec())
        .expect("a UTF-8 dump")
        .strip_suffix('\n')
        .expect("a dump ends with LF")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// `spec args` in the main worktree with `HOME=home`.
fn spec_home(pair: &Pair, home: &Path, args: &[&str]) -> Run {
    let output = Command::new(common::SPEC)
        .env_clear()
        .envs(pair.git.vars())
        .env("HOME", home)
        .current_dir(&pair.main)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("spec ran");
    Run {
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        code: output.status.code().expect("an exit code"),
        stdout: String::from_utf8(output.stdout).expect("UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("UTF-8"),
    }
}

/// `spec export state --out <file>` with `HOME=home`.
fn export(pair: &Pair, home: &Path, file: &Path) -> Vec<u8> {
    let done = spec_home(
        pair,
        home,
        &["export", "state", "--out", file.to_str().expect("UTF-8")],
    );
    done.code(0);
    fs::read(file).expect("the dump")
}

/// Library `import-state <file>` into `home`'s empty data directory.
fn import(pair: &Pair, home: &Path, file: &Path) {
    let mut consent = |_: &str| true;
    let outcome = specengine_cli::import_state(
        &Env {
            cwd: pair.main.clone(),
            home: Some(home.as_os_str().to_owned()),
            xdg_data_home: None,
        },
        &Globals::default(),
        &specengine_cli::ImportStateRequest {
            file: file.to_path_buf(),
        },
        &mut consent,
    )
    .unwrap_or_else(|error| panic!("import-state: {error}"));
    assert_eq!(outcome.exit(), Exit::Answered, "{outcome:?}");
}

/// RF17: link, file, deleted and node references dump as format 9: the
/// header's 11 keys with `refs` after `comment_versions`, the rows after
/// the version rows by ID number, every column in table order; the dump
/// imports into an empty data directory and dumps again byte-identical; it
/// holds no file's bytes. M: `refs` left out of the dump.
#[test]
fn rf17_references_round_trip_through_format_9() {
    let pair = Pair::new("rf-rf17", KEEP);
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    link_ref(
        &pair,
        &c1,
        "https://example.org/board",
        &["--title", "Moodboard", "--as", "lev"],
    );
    let gif = file_ref(
        &pair,
        &c1,
        &blob_fixture("lantern.gif"),
        &["--title", "Lamp", "--as", "mira"],
    );
    let gone = file_ref(
        &pair,
        &c1,
        &blob_fixture("sound.wav"),
        &["--matters", "the hum"],
    );
    link_ref(
        &pair,
        "RULE-SPRINT-COST",
        "https://example.org/n",
        &["--matters", "twelve"],
    );
    deleted(&pair, &id_of(&gone), &[]);

    let out = pair.scratch.dir("dumps").join("nine.jsonl");
    let bytes = export(&pair, &pair.home, &out);
    let lines = dump_lines(&bytes);
    assert!(
        lines[0].starts_with("{\"format\":10,\"queue_schema\":17,\"project\":\"lantern-keep\","),
        "{}",
        lines[0]
    );
    assert_eq!(
        keys_in_order(&lines[0]),
        [
            "format",
            "queue_schema",
            "project",
            "proposals",
            "tasks",
            "runs",
            "cards",
            "comments",
            "comment_versions",
            "refs",
            "events"
        ]
    );
    let header: Value = serde_json::from_str(&lines[0]).expect("a header");
    assert_eq!(header["refs"], json!(4));
    let rows: Vec<&String> = lines
        .iter()
        .filter(|line| line.starts_with("{\"refs\":"))
        .collect();
    assert_eq!(rows.len(), 4);
    let first_event = lines
        .iter()
        .position(|line| line.starts_with("{\"events\":"))
        .expect("events");
    let first_ref = lines
        .iter()
        .position(|line| line.starts_with("{\"refs\":"))
        .expect("refs");
    assert!(first_ref < first_event, "refs before events");
    for (n, row) in rows.iter().enumerate() {
        let inner = &row["{\"refs\":".len()..];
        assert_eq!(
            keys_in_order(inner),
            [
                "id",
                "project",
                "on_kind",
                "on_id",
                "on_rev",
                "actor",
                "author",
                "kind",
                "title",
                "matters",
                "url",
                "name",
                "b3",
                "agent_b3",
                "size",
                "media",
                "meta",
                "created_at",
                "deleted_by",
                "deleted_at"
            ],
            "{row}"
        );
        let value: Value = serde_json::from_str(row).expect("a row");
        assert_eq!(
            value["refs"]["id"],
            json!(format!("RF-{:04}", n + 1)),
            "by ID number"
        );
    }
    let gif_row: Value = serde_json::from_str(rows[1]).expect("a row");
    assert!(gif_row["refs"]["size"].is_u64(), "size a number: {gif_row}");
    let node_row: Value = serde_json::from_str(rows[3]).expect("a row");
    assert_eq!(node_row["refs"]["on_rev"], json!(3), "on_rev a number");
    let gif_bytes = fs::read(blob_fixture("lantern.gif")).expect("gif");
    assert!(
        !holds(&bytes, "GIF89a") && !holds(&bytes, "GIF87a"),
        "no file bytes"
    );
    assert_eq!(gif_row["refs"]["b3"], json!(b3(&gif_bytes)));

    let fresh = pair.scratch.home("restored");
    import(&pair, &fresh, &out);
    let again = pair.scratch.dir("dumps").join("again.jsonl");
    assert_eq!(export(&pair, &fresh, &again), bytes, "byte-identical");
    // The restored queue has no blob store: the file is lost, never fetched.
    let lost = spec_home(&pair, &fresh, &["--json", "ref", "get", &id_of(&gif)]);
    lost.code(0);
    assert_eq!(lost.json()["file"], json!("lost"));
    let text = spec_home(&pair, &fresh, &["ref", "get", &id_of(&gif)]);
    text.code(0);
    assert!(text.stdout.contains("file lost"), "{}", text.stdout);
    let target = pair.scratch.dir("out").join("lamp.gif");
    let fetched = spec_home(
        &pair,
        &fresh,
        &[
            "ref",
            "get",
            &id_of(&gif),
            "--out",
            target.to_str().expect("UTF-8"),
        ],
    );
    assert_eq!(fetched.code, 1, "{}", fetched.show());
    assert!(
        fetched.stderr.contains(&format!(
            "{}'s file is not on this host: it was lost with the host it was uploaded to",
            id_of(&gif)
        )),
        "{}",
        fetched.show()
    );
    assert!(!target.exists());
}

/// RF17: the dump holds rows, never bytes: two queues equal but for the
/// size of one file (1.5 KiB and 1 MiB) dump to lengths within a few
/// bytes (the size's digits).
#[test]
fn rf17_the_dumps_size_does_not_follow_the_files() {
    let sizes = [None, Some(1_000_000usize)];
    let mut lengths = Vec::new();
    for (n, size) in sizes.iter().enumerate() {
        let pair = Pair::new(&format!("rf-rf17-size-{n}"), ALMANAC);
        let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
        let bytes = png(400, 300, 0x0F, *size);
        let file = scratch_file(&pair, "lamp.png", &bytes);
        file_ref(&pair, &c1, &file, &["--title", "Lamp"]);
        let out = pair.scratch.dir("dumps").join("d.jsonl");
        lengths.push(export(&pair, &pair.home, &out).len());
    }
    assert!(
        lengths[0].abs_diff(lengths[1]) <= 8,
        "the dump follows the file's size: {lengths:?}"
    );
}

/// RF17: a format 8 dump written by 0.50.0 imports with no references; the
/// queue dumps again as format 9 with `"refs":0`.
#[test]
fn rf17_a_format_8_dump_imports_with_no_references() {
    let pair = Pair::new("rf-rf17-eight", KEEP);
    let eight =
        repository_root().join("crates/specengine-cli/tests/work_references_format_8.jsonl");
    let bytes = fs::read(&eight).expect("the format 8 dump");
    let lines = dump_lines(&bytes);
    assert!(
        lines[0].starts_with("{\"format\":8,\"queue_schema\":15,\"project\":\"lantern-keep\","),
        "{}",
        lines[0]
    );
    let fresh = pair.scratch.home("from-8");
    import(&pair, &fresh, &eight);
    let db = data_dir(&fresh).join("lantern-keep.db");
    let sql = |query: &str| {
        let output = Command::new(common::proposal::sqlite3())
            .arg(&db)
            .arg(query)
            .output()
            .expect("sqlite3");
        String::from_utf8(output.stdout).expect("UTF-8")
    };
    assert_eq!(sql("PRAGMA user_version;").trim(), "17");
    assert_eq!(sql("SELECT count(*) FROM refs;").trim(), "0");
    assert_eq!(sql("SELECT count(*) FROM comments;").trim(), "3");
    let listed = spec_home(&pair, &fresh, &["--json", "ref", "list", "--on", "C-0001"]);
    listed.code(0);
    assert_eq!(listed.json()["references"], json!([]));
    assert_eq!(listed.json()["total"], json!(0));

    let out = pair.scratch.dir("dumps").join("from-8.jsonl");
    let nine = dump_lines(&export(&pair, &fresh, &out));
    let mut want = lines.clone();
    want[0] = want[0]
        .replacen(
            "\"format\":8,\"queue_schema\":15,",
            "\"format\":10,\"queue_schema\":17,",
            1,
        )
        .replacen(",\"events\":", ",\"refs\":0,\"events\":", 1);
    // `ref_id` last on each proposal since docs/features/reference-promotion.md
    // (format 10).
    for line in want
        .iter_mut()
        .filter(|line| line.starts_with("{\"proposals\":"))
    {
        let body = line.strip_suffix("}}").expect("a row's end").to_owned();
        *line = format!("{body},\"ref_id\":null}}}}");
    }
    assert_eq!(nine, want);
}

/// RF17, the twin of comments' CM13: a schema-15 database exports as
/// format 9, schema 16, unmigrated, `"refs":0`; a format 10 dump is refused
/// at its first line, nothing written.
#[test]
fn rf17_a_schema_15_queue_exports_as_format_9_unmigrated() {
    let pair = Pair::new("rf-rf17-fifteen", KEEP);
    card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    pair.sql("DROP TABLE refs; DROP INDEX IF EXISTS proposals_ref; ALTER TABLE proposals DROP COLUMN ref_id; PRAGMA user_version = 15;");
    let out = pair.scratch.dir("dumps").join("fifteen.jsonl");
    let lines = dump_lines(&export(&pair, &pair.home, &out));
    assert!(
        lines[0].starts_with("{\"format\":10,\"queue_schema\":17,\"project\":\"lantern-keep\","),
        "{}",
        lines[0]
    );
    assert!(lines[0].contains(",\"refs\":0,\"events\":"), "{}", lines[0]);
    assert_eq!(pair.sql("PRAGMA user_version;").trim(), "15", "unmigrated");

    let mut ten = lines.clone();
    ten[0] = ten[0].replacen("\"format\":10,", "\"format\":11,", 1);
    let file = pair.scratch.dir("dumps").join("ten.jsonl");
    let mut text = ten.join("\n");
    text.push('\n');
    fs::write(&file, text).expect("write");
    let fresh = pair.scratch.home("ten");
    let mut consent = |_: &str| true;
    let refused = specengine_cli::import_state(
        &Env {
            cwd: pair.main.clone(),
            home: Some(fresh.as_os_str().to_owned()),
            xdg_data_home: None,
        },
        &Globals::default(),
        &specengine_cli::ImportStateRequest { file },
        &mut consent,
    );
    let message = match refused {
        Err(error) => error.to_string(),
        Ok(outcome) => panic!("format 10 imported: {outcome:?}"),
    };
    assert!(message.contains("upgrade SpecEngine"), "{message}");
}

// --------------------------------------------------------------- "Data"

/// "Text": the list's header, a link's line with its URL, a file's line
/// with its type, size, facts and agent copy, "what matters" and name, a
/// deleted one's; the JSON list `{on, total, references, next_before,
/// notes}`; an empty list `: none`.
#[test]
fn the_list_text_is_the_datas() {
    let pair = Pair::new("rf-text", KEEP);
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    let empty = text_run(&pair, &["ref", "list", "--on", &c1]);
    empty.code(0);
    assert!(
        empty
            .stdout
            .starts_with(&format!("# References on {c1}: none")),
        "{}",
        empty.stdout
    );
    let link = link_ref(
        &pair,
        &c1,
        "https://example.org/board",
        &["--title", "Moodboard", "--as", "lev"],
    );
    let copy = scratch_file(&pair, "copy.png", &png(1568, 1176, 0x55, None));
    let image = file_ref(
        &pair,
        &c1,
        &blob_fixture("harbour.png"),
        &[
            "--title",
            "Harbour at dusk",
            "--matters",
            "the light on the water, not the boats",
            "--agent-copy",
            copy.to_str().expect("UTF-8"),
            "--as",
            "mira",
        ],
    );
    let gone = link_ref(&pair, &c1, "https://example.org/gone", &["--as", "mira"]);
    deleted(&pair, &id_of(&gone), &["--as", "mira"]);
    let gone = doc(&pair, &["ref", "get", &id_of(&gone)]);
    let text = text_run(&pair, &["ref", "list", "--on", &c1]);
    text.code(0);
    let want = format!(
        "# References on {c1}: 3; {MATERIAL}\n\
         {} | lev | {} | link | Moodboard\n  url: https://example.org/board\n\
         {} | mira | {} | image/png 1.5 KiB 4032x3024, agent copy | Harbour at dusk\n\
         \x20 matters: the light on the water, not the boats\n  file: harbour.png\n\
         {} | deleted by mira {}\n",
        id_of(&link),
        link["created_at"].as_str().expect("a time"),
        id_of(&image),
        image["created_at"].as_str().expect("a time"),
        id_of(&gone),
        gone["deleted"]["at"].as_str().expect("a time"),
    );
    assert_eq!(text.stdout, want);
    let raw = json_run(&pair, &["ref", "list", "--on", &c1]);
    raw.code(0);
    assert_eq!(
        keys_in_order(&raw.stdout),
        // `in_spec` since docs/features/reference-promotion.md.
        [
            "on",
            "total",
            "references",
            "next_before",
            "notes",
            "in_spec"
        ]
    );
    let listed = raw.json();
    assert_eq!(listed["total"], json!(3));
    assert_eq!(listed["next_before"], Value::Null);
    assert_eq!(listed["on"], json!({"kind": "card", "id": c1}));
    // The image's agent copy is named by it, not by a reference of its own.
    assert_eq!(listed["references"][1]["agent_b3"], image["agent_b3"]);
    let get = text_run(&pair, &["ref", "get", &id_of(&image)]);
    get.code(0);
    assert!(
        get.stdout
            .contains("image/png 1.5 KiB 4032x3024, agent copy"),
        "{}",
        get.stdout
    );
}

/// "Data", the spec's example size: a file of 2 202 009 bytes reads
/// `2.1 MiB` in a list and the delete prompt ("Documents", "Text").
#[test]
fn the_examples_size_reads_2_1_mib() {
    let pair = Pair::new("rf-text-size", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    let bytes = png(4032, 3024, 0x0F, Some(2_202_009));
    let file = scratch_file(&pair, "harbour.png", &bytes);
    let made = file_ref(&pair, &c1, &file, &["--as", "mira"]);
    assert_eq!(size_text(2_202_009), "2.1 MiB", "premise");
    let text = text_run(&pair, &["ref", "list", "--on", &c1]);
    text.code(0);
    assert!(
        text.stdout.contains("image/png 2.1 MiB 4032x3024"),
        "{}",
        text.stdout
    );
    let (code, output) = delete_on_terminal(&pair, &id_of(&made), &[], Some("n"));
    assert_eq!(code, 1);
    assert!(
        output.contains(&format!(
            "delete {} (mira, on {c1}, image/png 2.1 MiB)?",
            id_of(&made)
        )),
        "{output}"
    );
}

/// "Data", D19: lists page at 100, oldest first within a page, `--before`
/// and `--limit` (1-100), `next_before` the page's smallest ID when older
/// exist; the text's `next: --before RF-…`.
#[test]
fn a_list_pages_at_100() {
    let pair = Pair::new("rf-pages", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    for n in 1..=105 {
        link_ref(&pair, &c1, &format!("https://example.org/{n}"), &[]);
    }
    let page = list(&pair, &c1, &[]);
    let ids: Vec<String> = page["references"]
        .as_array()
        .expect("references")
        .iter()
        .map(id_of)
        .collect();
    assert_eq!(ids.len(), 100);
    assert_eq!(ids.first().map(String::as_str), Some("RF-0006"));
    assert_eq!(ids.last().map(String::as_str), Some("RF-0105"));
    assert_eq!(page["total"], json!(105));
    assert_eq!(page["next_before"], json!("RF-0006"));
    let older = list(&pair, &c1, &["--before", "RF-0006"]);
    let older_ids: Vec<String> = older["references"]
        .as_array()
        .expect("references")
        .iter()
        .map(id_of)
        .collect();
    assert_eq!(
        older_ids,
        ["RF-0001", "RF-0002", "RF-0003", "RF-0004", "RF-0005"]
    );
    assert_eq!(older["next_before"], Value::Null);
    let three = list(&pair, &c1, &["--limit", "3"]);
    assert_eq!(three["references"].as_array().map(Vec::len), Some(3));
    assert_eq!(three["next_before"], json!("RF-0103"));
    let text = text_run(&pair, &["ref", "list", "--on", &c1, "--limit", "3"]);
    assert!(
        text.stdout.ends_with("next: --before RF-0103\n"),
        "{}",
        text.stdout
    );
    for bad in ["0", "101"] {
        assert_eq!(
            json_run(&pair, &["ref", "list", "--on", &c1, "--limit", bad]).code,
            2
        );
    }
}

/// "CLI": `spec ref get --out F` writes the file or, with `--agent-copy`,
/// its copy to a new F; an existing F exits 2; a damaged store exits 1,
/// removing F. `--agent-copy` and `--name` take a file alone.
#[test]
fn get_out_writes_a_new_file_checked_as_it_copies() {
    let pair = Pair::new("rf-get-out", KEEP);
    let c1 = card(
        &pair,
        "Write the sprint foreword",
        &["--target", "MEC-SPRINT"],
    );
    let original = png(4032, 3024, 0x21, None);
    let small = png(1568, 1176, 0x21, None);
    let file = scratch_file(&pair, "harbour.png", &original);
    let copy = scratch_file(&pair, "small.png", &small);
    let made = file_ref(
        &pair,
        &c1,
        &file,
        &["--agent-copy", copy.to_str().expect("UTF-8")],
    );
    let id = id_of(&made);
    let out = pair.scratch.dir("out");
    let target = out.join("whole.png");
    let done = text_run(
        &pair,
        &["ref", "get", &id, "--out", target.to_str().expect("UTF-8")],
    );
    done.code(0);
    assert_eq!(fs::read(&target).expect("written"), original);
    let small_target = out.join("small.png");
    text_run(
        &pair,
        &[
            "ref",
            "get",
            &id,
            "--out",
            small_target.to_str().expect("UTF-8"),
            "--agent-copy",
        ],
    )
    .code(0);
    assert_eq!(fs::read(&small_target).expect("written"), small);
    let again = text_run(
        &pair,
        &["ref", "get", &id, "--out", target.to_str().expect("UTF-8")],
    );
    assert_eq!(again.code, 2, "{}", again.show());
    assert_eq!(
        fs::read(&target).expect("kept"),
        original,
        "an existing F is kept"
    );

    // A damaged store: the stored file's bytes changed.
    let stored = store(&pair).join(b3(&original));
    let mut damaged = original.clone();
    let last = damaged.len() - 20;
    damaged[last] ^= 0xFF;
    fs::set_permissions(
        &stored,
        <fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o600),
    )
    .expect("chmod");
    fs::write(&stored, &damaged).expect("damage");
    let broken = out.join("broken.png");
    let done = text_run(
        &pair,
        &["ref", "get", &id, "--out", broken.to_str().expect("UTF-8")],
    );
    assert_eq!(done.code, 1, "{}", done.show());
    assert!(
        done.stderr.contains(&format!(
            "{id}'s file does not match its BLAKE3: the store is damaged"
        )),
        "{}",
        done.show()
    );
    assert!(!broken.exists(), "F removed");

    let link = id_of(&link_ref(&pair, &c1, "https://example.org/x", &[]));
    let to = out.join("link.out");
    let no_file = text_run(
        &pair,
        &["ref", "get", &link, "--out", to.to_str().expect("UTF-8")],
    );
    assert_ne!(no_file.code, 0, "a link has no file: {}", no_file.show());
    assert!(!to.exists());
    for flags in [
        vec!["--agent-copy", copy.to_str().expect("UTF-8")],
        vec!["--name", "x.png"],
    ] {
        let mut args = vec!["ref", "add", "--on", &c1, "--url", "https://example.org/y"];
        args.extend(flags.iter().copied());
        assert_eq!(json_run(&pair, &args).code, 2, "{flags:?} with --url");
    }
}

/// "CLI": `spec ref` alone prints its usage, which names `spec refs REF`,
/// the code layer's markers, a different command.
#[test]
fn spec_ref_alone_names_spec_refs() {
    let pair = Pair::new("rf-usage", ALMANAC);
    let done = text_run(&pair, &["ref"]);
    let all = format!("{}{}", done.stdout, done.stderr);
    assert!(all.contains("spec refs"), "{}", done.show());
    for command in ["add", "list", "get", "delete"] {
        assert!(all.contains(command), "{command}: {}", done.show());
    }
}

/// D2, D20: `--as N` stores a person as a comment does; the agent flags an
/// agent, `actor` `agent`; both together exit 2. The author matches a
/// comment's written with the same flags.
#[test]
fn authors_are_a_comments() {
    let pair = Pair::new("rf-authors", ALMANAC);
    let c1 = card(&pair, "Call the entries", &["--discipline", "editing"]);
    for flags in [
        vec!["--as", "mira"],
        vec![],
        vec!["--author-role", "writer"],
    ] {
        let reference = link_ref(&pair, &c1, "https://example.org/a", &flags);
        let mut args = vec!["--json", "comment", "add", "--on", &c1, "--body-file", "-"];
        args.extend(flags.iter().copied());
        let comment = pair.spec_piped(&pair.main, &args, b"A word.");
        comment.code(0);
        let comment = comment.json();
        assert_eq!(reference["author"], comment["author"], "{flags:?}");
        assert_eq!(reference["actor"], comment["actor"], "{flags:?}");
    }
    let both = add_url(
        &pair,
        &c1,
        "https://example.org/b",
        &["--as", "mira", "--author-role", "writer"],
    );
    assert_eq!(both.code, 2, "{}", both.show());
}
