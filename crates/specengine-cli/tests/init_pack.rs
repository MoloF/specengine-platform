//! docs/features/pack.md "Acceptance criteria" (RM-0061): `spec init --pack
//! PATH` writes `[project]`, the slug, a blank line and the pack's bytes
//! verbatim, checked before any write, by path only, never a merge.
//!
//! - PK1: the gamedev pack installs byte for byte; `spec workspace` and
//!   `spec card list` read it; JSON `{path, slug, pack}`, the text names the
//!   path as given. M: the pack re-serialised (its comments lost).
//! - PK2: every refusal of "Checks, in order" exits 2 naming the pack's own
//!   line, and no `specengine.toml` exists after. M: write, then check; the
//!   file's line reported (`:21` for (d)).
//! - PK3: the software preset, `fixtures/plant-diary` holding it verbatim,
//!   its check, its workspaces, an install of it and a split on it. M: a flow
//!   title changed in the fixture's copy alone. (f), the guards, is
//!   `workspace_names.rs`, `no_domain.rs`, `anonymity.rs` and
//!   `ui/src/policy.test.ts`.
//! - PK4: an existing `specengine.toml` is refused with or without a pack,
//!   its bytes unchanged. M: the file overwritten.
//! - PK5: `--pack gamedev` is a path; no file by that name is a read error.
//!   M: a preset embedded by name.
//! - PK6: no pack, no change: the base commit's bytes, text and exit; JSON
//!   `pack: null`. M: the scaffold dropped without a pack.
//! - PK7: the gamedev preset opens with "The gamedev header" (studio-lamp's
//!   copy is core's `workspace_toml.rs` WS-7).
//!
//! Every expected value is worked out from the spec; a reader's message is
//! the core reader's own on the composed text, its line minus the three
//! header lines.

#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Run, Scratch, fixture, read, read_text, repository_root, snapshot, spec, write};
use serde_json::{Value, json};
use specengine_core::{
    ProjectConfig, WorkspaceConfig, card_from_toml, paths_from_toml, scheme_from_toml,
};

/// `PACK_MAX_BYTES` of "Checks, in order", 2.
const PACK_MAX_BYTES: usize = 1_048_576;

/// What `spec init` writes without a pack after its two lines: the base
/// commit's scaffold (`tests/init.rs`), unchanged by this slice.
const SCAFFOLD: &str = "\n\
    # Commit this file: a proposal needs a commit, and the project's rules live here.\n\
    # [ids]\n\
    # NODE = { kind = \"node\", shape = \"name\" }\n\
    # [[flow.start]]\n\
    # paths = [\"docs/spec/first.md\"]\n\
    # [[flow.end]]\n\
    # paths = [\"docs/spec/last.md\"]\n";

/// "The gamedev header", the preset's first lines since this slice.
const GAMEDEV_HEADER: &str = "\
# The gamedev preset (docs/features/workspaces.md \"The gamedev preset\"): six
# card flows and five workspaces for a game studio's trades. Install it in a
# new project with `spec init --pack <this file>`, or copy it into a project's
# specengine.toml; add the project's own [[module]] tables and a workspace's
# `module`, and retitle anything in the project's language.
";

/// The gamedev preset's flows, `(name, title)`, in its order.
const GAMEDEV_FLOWS: [(&str, &str); 6] = [
    ("narrative", "Narrative"),
    ("development", "Programming"),
    ("art", "Art"),
    ("level-design", "Level design"),
    ("production", "Production"),
    ("sound", "Sound"),
];

/// The software preset's flows, `(name, title)`, in its order ("Data").
const SOFTWARE_FLOWS: [(&str, &str); 5] = [
    ("product", "Product"),
    ("experience", "UX design"),
    ("engineering", "Engineering"),
    ("quality", "Quality assurance"),
    ("documentation", "Documentation"),
];

/// plant-diary's marker lines, as studio-lamp's.
const SOFTWARE_OPENS: &str = "# --- scripts/presets/software.toml ---\n";
const PRESET_ENDS: &str = "# --- end of the preset ---\n";

// ------------------------------------------------------------- helpers

fn preset_path(name: &str) -> PathBuf {
    repository_root().join(format!("scripts/presets/{name}.toml"))
}

fn preset(name: &str) -> String {
    fs::read_to_string(preset_path(name)).expect("the preset")
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// "The file written": the header, then the pack's bytes.
fn header(slug: &str) -> String {
    format!("[project]\nslug = \"{slug}\"\n\n")
}

fn installed(slug: &str, pack: &[u8]) -> Vec<u8> {
    let mut bytes = header(slug).into_bytes();
    bytes.extend_from_slice(pack);
    bytes
}

/// `text` with `insert` (whole lines) placed so its first line is line
/// `line` (1-based).
fn insert_at(text: &str, line: usize, insert: &str) -> String {
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    lines.insert(line - 1, insert);
    lines.concat()
}

/// `text` with line `line` (1-based, its line end kept) replaced by `with`.
fn replace_line(text: &str, line: usize, with: &str) -> String {
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_owned).collect();
    assert!(lines[line - 1].ends_with('\n'), "line {line} ends");
    lines[line - 1] = format!("{with}\n");
    lines.concat()
}

fn line_text(text: &str, line: usize) -> &str {
    text.lines().nth(line - 1).expect("the line")
}

/// Asserts a refusal: exit 2, stdout empty, stderr `spec: <message>\n`, and
/// the directory holding exactly `kept` (no `specengine.toml`).
fn assert_refused(run: &Run, message: &str, dir: &Path, kept: &[&str], label: &str) {
    run.code(2);
    assert_eq!(run.stdout, "", "{label}: {}", run.show());
    assert_eq!(
        run.stderr,
        format!("spec: {message}\n"),
        "{label}: {}",
        run.show()
    );
    assert_nothing_written(dir, kept, label);
}

/// No `specengine.toml` under `dir`: its entries are exactly `kept`.
fn assert_nothing_written(dir: &Path, kept: &[&str], label: &str) {
    let entries: Vec<String> = snapshot(dir).into_keys().collect();
    assert_eq!(entries, kept, "{label}: nothing written");
    assert!(
        !dir.join("specengine.toml").exists(),
        "{label}: no specengine.toml"
    );
}

/// The first refusal of "Checks, in order" 4 on `composed`, in the pack's
/// lines: `<PATH>:<L>: <message>`, or `<PATH>: <message>` with no line.
fn reader_refusal(composed: &str, path: &str) -> Option<String> {
    let at = |line: Option<usize>, message: String| match line {
        Some(line) => format!("{path}:{}: {message}", line - 3),
        None => format!("{path}: {message}"),
    };
    if let Err(error) = ProjectConfig::from_toml(composed) {
        return Some(at(error.line, error.message));
    }
    if let Err(error) = scheme_from_toml(composed) {
        return Some(at(error.line, error.message));
    }
    if let Err(error) = paths_from_toml(composed) {
        return Some(at(error.line, error.message));
    }
    if let Err(error) = card_from_toml(composed) {
        return Some(at(error.line, error.message));
    }
    None
}

/// `spec workspace` in `root`: exit 0, its stdout; asserts no note.
fn workspace_text(home: &Path, root: &Path) -> String {
    let run = spec(home, root, &["workspace"]);
    run.code(0);
    assert!(!run.stderr.contains("note:"), "no note: {}", run.show());
    assert_eq!(run.stderr, "", "{}", run.show());
    run.stdout
}

/// `spec card list --json` in `root`: the flows' `(name, title, return)`.
fn listed_flows(home: &Path, root: &Path) -> Vec<(String, String, Value)> {
    let run = spec(home, root, &["card", "list", "--json"]);
    run.code(0);
    run.json()["flows"]
        .as_array()
        .unwrap_or_else(|| panic!("flows: {}", run.show()))
        .iter()
        .map(|flow| {
            (
                flow["name"].as_str().expect("a name").to_owned(),
                flow["title"].as_str().expect("a title").to_owned(),
                flow["return"].clone(),
            )
        })
        .collect()
}

/// The five workspace lines of studio-lamp's `expected.json` `workspaces`.
fn lamp_workspace_lines() -> String {
    let expected: Value =
        serde_json::from_str(&read_text(&fixture("studio-lamp"), "expected.json")).expect("JSON");
    let text = expected["workspaces"].as_str().expect("workspaces");
    let lines: String = text
        .split_inclusive('\n')
        .filter(|line| line.starts_with("workspace "))
        .collect();
    assert_eq!(lines.lines().count(), 5, "{text}");
    lines
}

/// The `text` between `opens` and `PRESET_ENDS`, asserting each marker once.
fn between_markers<'a>(text: &'a str, opens: &str) -> &'a str {
    assert_eq!(text.matches(opens).count(), 1, "one opening marker");
    assert_eq!(text.matches(PRESET_ENDS).count(), 1, "one closing marker");
    let start = text.find(opens).expect("opens") + opens.len();
    let end = text.find(PRESET_ENDS).expect("ends");
    assert!(start <= end, "the markers in order");
    &text[start..end]
}

// ----------------------------------------------------------------- PK1

/// PK1: the gamedev pack, by its absolute path, in `lamp-test`: exit 0, JSON
/// `{"path","slug","pack"}` in that order with the path as given; the file is
/// the header plus the preset's bytes; `spec workspace` prints `modules 0`
/// and studio-lamp's five workspace lines, no note; `card list` serves the
/// six flows in order with their titles and `return` `sent-back`. The text
/// form names the path; a second install gives the same bytes. M: the pack
/// re-serialised (its comments lost).
#[test]
fn pk1_the_gamedev_pack_installs_verbatim_and_reads() {
    let scratch = Scratch::new("pk1");
    let home = scratch.home("h");
    let dir = scratch.dir("lamp-test");
    let pack = preset_path("gamedev");
    let given = path_text(&pack);
    let run = spec(&home, &dir, &["init", "--pack", given, "--json"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        format!(
            "{{\"path\":\"specengine.toml\",\"slug\":\"lamp-test\",\"pack\":{}}}\n",
            serde_json::to_string(given).expect("a JSON string")
        ),
        "{}",
        run.show()
    );
    assert_eq!(run.stderr, "", "{}", run.show());
    let bytes = read(&dir, "specengine.toml");
    let preset_bytes = fs::read(&pack).expect("the preset");
    assert!(
        bytes == installed("lamp-test", &preset_bytes),
        "the header plus the preset's bytes, comments included (M: re-serialised):\n{}",
        String::from_utf8_lossy(&bytes)
    );
    let entries: Vec<String> = snapshot(&dir).into_keys().collect();
    assert_eq!(entries, ["specengine.toml"], "init writes only its file");
    assert!(snapshot(&home).is_empty(), "init creates no data directory");

    assert_eq!(
        workspace_text(&home, &dir),
        format!(
            "workspaces config | workspaces 5, modules 0\n{}",
            lamp_workspace_lines()
        )
    );
    let flows = listed_flows(&home, &dir);
    let want: Vec<(String, String, Value)> = GAMEDEV_FLOWS
        .iter()
        .map(|(name, title)| ((*name).to_owned(), (*title).to_owned(), json!("sent-back")))
        .collect();
    assert_eq!(flows, want);

    // The text form, and one pack and slug giving one file (determinism).
    let again = Scratch::new("pk1-text");
    let home = again.home("h");
    let dir = again.dir("lamp-test");
    let run = spec(&home, &dir, &["init", "--pack", given]);
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("created specengine.toml with slug lamp-test from {given}\n"),
        "{}",
        run.show()
    );
    assert_eq!(run.stderr, "", "{}", run.show());
    assert_eq!(read(&dir, "specengine.toml"), bytes, "one pack, one file");
}

/// PK1 and "Rules and edge cases": the bytes pass untouched, with no newline
/// added or removed: a pack with no final newline, an empty pack and a
/// comment-only pack each give the header plus exactly their bytes.
#[test]
fn pk1_no_newline_is_added_or_removed() {
    let scratch = Scratch::new("pk1-bytes");
    let home = scratch.home("h");
    for (label, pack) in [
        (
            "no final newline",
            "# a kind\n[ids]\nNOTE = { kind = \"note\", shape = \"name\" }",
        ),
        ("empty", ""),
        (
            "comments only",
            "# nothing but a comment\n\n# and another\n",
        ),
        (
            "CRLF",
            "# a comment\r\n[ids]\r\nNOTE = { kind = \"note\", shape = \"name\" }\r\n",
        ),
    ] {
        let dir = scratch.dir(&label.replace(' ', "-").to_lowercase());
        write(&dir, "p.toml", pack);
        let run = spec(
            &home,
            &dir,
            &["init", "--slug", "bytes", "--pack", "p.toml"],
        );
        run.code(0);
        assert_eq!(
            run.stdout,
            "created specengine.toml with slug bytes from p.toml\n",
            "{label}: {}",
            run.show()
        );
        assert_eq!(
            read(&dir, "specengine.toml"),
            installed("bytes", pack.as_bytes()),
            "{label}"
        );
    }
}

// ----------------------------------------------------------------- PK2

/// One PK2 case: the pack written as `pack.toml` in a fresh `dir`, run with
/// `--pack pack.toml`; refused with `message`, nothing written.
fn refused_with(scratch: &Scratch, home: &Path, label: &str, pack: &[u8], message: &str) {
    let dir = scratch.dir(label);
    write(&dir, "pack.toml", pack);
    let run = spec(home, &dir, &["init", "--pack", "pack.toml"]);
    assert_refused(&run, message, &dir, &["pack.toml"], label);
    let run = spec(home, &dir, &["--json", "init", "--pack", "pack.toml"]);
    assert_refused(
        &run,
        message,
        &dir,
        &["pack.toml"],
        &format!("{label} --json"),
    );
}

/// PK2 (a)-(c): the pack alone. (a) `[project]` at line 10 of a copy of the
/// software preset; (b) a key `color = "red"` at line 10, above the first
/// table; (c) a `[team]` table at line 10. M: write, then check.
#[test]
fn pk2_abc_the_pack_alone_is_refused_at_its_own_line() {
    let scratch = Scratch::new("pk2-alone");
    let home = scratch.home("h");
    let software = preset("software");
    assert_eq!(line_text(&software, 10), "", "line 10 is the blank line");
    assert_eq!(line_text(&software, 11), "[[card.flow]]");

    let project = insert_at(&software, 10, "[project]\nslug = \"other\"\n");
    assert_eq!(line_text(&project, 10), "[project]");
    refused_with(
        &scratch,
        &home,
        "a-project",
        project.as_bytes(),
        "pack.toml:10: a pack holds no [project]: spec init writes it",
    );

    let key = replace_line(&software, 10, "color = \"red\"");
    refused_with(
        &scratch,
        &home,
        "b-key",
        key.as_bytes(),
        "pack.toml:10: a pack starts with a table header; a key above it would join [project]",
    );

    let team = insert_at(
        &software,
        10,
        "[team]\nana = { role = \"member\", email = \"ana@example.test\" }\n",
    );
    assert_eq!(line_text(&team, 10), "[team]");
    refused_with(
        &scratch,
        &home,
        "c-team",
        team.as_bytes(),
        "pack.toml:10: a pack holds no [team]: a project's people are its own",
    );
}

/// PK2 (d): the first flow's `return = "ideas"` (line 18) is the card
/// reader's refusal at `pack.toml:18`, never the file's `:21`. M: the file's
/// line reported.
#[test]
fn pk2_d_a_bad_return_is_named_at_the_packs_line() {
    let scratch = Scratch::new("pk2-return");
    let home = scratch.home("h");
    let software = preset("software");
    assert_eq!(line_text(&software, 18), "return = \"reopened\"");
    let pack = replace_line(&software, 18, "return = \"ideas\"");
    let message = "pack.toml:18: `return`: `ideas` is the first column, where a new card starts";
    assert_eq!(
        reader_refusal(&format!("{}{pack}", header("d-return")), "pack.toml").as_deref(),
        Some(message),
        "the core reader's own message, at the pack's line"
    );
    refused_with(&scratch, &home, "d-return", pack.as_bytes(), message);
}

/// PK2 (e): a workspace section `"wiki"` on line 59 is the workspaces' note,
/// refusing at `pack.toml:59`.
#[test]
fn pk2_e_a_workspace_note_refuses_at_the_packs_line() {
    let scratch = Scratch::new("pk2-note");
    let home = scratch.home("h");
    let software = preset("software");
    let sections =
        "sections = [\"roadmap\", \"board\", \"tree\", \"flow\", \"questions\", \"inbox\"]";
    assert_eq!(line_text(&software, 59), sections);
    let pack = replace_line(
        &software,
        59,
        &sections.replace("\"inbox\"]", "\"inbox\", \"wiki\"]"),
    );
    let composed = format!("{}{pack}", header("e-note"));
    assert_eq!(
        reader_refusal(&composed, "pack.toml"),
        None,
        "the readers pass"
    );
    let notes = WorkspaceConfig::from_toml(&composed).notes;
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].line, 62, "the file's line 62 is the pack's 59");
    assert!(
        notes[0].message.contains("`wiki` is no section"),
        "{notes:?}"
    );
    refused_with(
        &scratch,
        &home,
        "e-note",
        pack.as_bytes(),
        &format!("pack.toml:59: {}", notes[0].message),
    );
}

/// PK2 (f): a bad `[ids]` entry, the scheme reader's refusal at the pack's
/// line: an unknown shape, and a prefix with a Cyrillic look-alike (ADR-0009).
#[test]
fn pk2_f_a_bad_ids_entry_is_refused_at_the_packs_line() {
    let scratch = Scratch::new("pk2-ids");
    let home = scratch.home("h");
    let software = preset("software");
    for (label, entry) in [
        ("f-shape", "EPC = { kind = \"epic\", shape = \"nope\" }\n"),
        (
            "f-mixed",
            "\"E\u{0420}C\" = { kind = \"epic\", shape = \"name\" }\n",
        ),
    ] {
        let pack = insert_at(&software, 10, &format!("[ids]\n{entry}\n"));
        let composed = format!("{}{pack}", header(label));
        let message = reader_refusal(&composed, "pack.toml")
            .unwrap_or_else(|| panic!("{label}: a reader refuses it"));
        assert!(
            message.starts_with("pack.toml:11: "),
            "{label}: the entry is the pack's line 11: {message}"
        );
        refused_with(&scratch, &home, label, pack.as_bytes(), &message);
    }
}

/// PK2 (g): not TOML: `pack.toml:<L>: <the reader's message>` at the pack's
/// broken line 12.
#[test]
fn pk2_g_not_toml_names_the_packs_line() {
    let scratch = Scratch::new("pk2-toml");
    let home = scratch.home("h");
    let software = preset("software");
    assert_eq!(line_text(&software, 12), "name = \"product\"");
    let pack = replace_line(&software, 12, "this is not toml");
    let dir = scratch.dir("g-toml");
    write(&dir, "pack.toml", &pack);
    let run = spec(&home, &dir, &["init", "--pack", "pack.toml"]);
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(
        run.stderr.starts_with("spec: pack.toml:12: ") && run.stderr.ends_with('\n'),
        "{}",
        run.show()
    );
    assert!(
        run.stderr.len() > "spec: pack.toml:12: \n".len(),
        "a message follows: {}",
        run.show()
    );
    assert_nothing_written(&dir, &["pack.toml"], "g-toml");
}

/// PK2 (h)-(j): the read. Not UTF-8; 1 048 577 bytes (a valid pack padded
/// with a comment), while exactly 1 048 576 installs; a directory.
#[test]
fn pk2_hij_the_read_is_refused_before_any_write() {
    let scratch = Scratch::new("pk2-read");
    let home = scratch.home("h");

    let mut bad = b"[[card.flow]]\nname = \"".to_vec();
    bad.extend_from_slice(&[0xff, 0xfe]);
    bad.extend_from_slice(b"\"\n");
    refused_with(
        &scratch,
        &home,
        "h-utf8",
        &bad,
        "--pack pack.toml: not UTF-8",
    );

    let padded = |size: usize| {
        let mut text = preset("software");
        text.push('#');
        while text.len() < size - 1 {
            text.push('x');
        }
        text.push('\n');
        assert_eq!(text.len(), size);
        text
    };
    refused_with(
        &scratch,
        &home,
        "i-size",
        padded(PACK_MAX_BYTES + 1).as_bytes(),
        "--pack pack.toml: 1048577 bytes; a pack is at most 1048576",
    );
    let dir = scratch.dir("i-at-most");
    let at_most = padded(PACK_MAX_BYTES);
    write(&dir, "pack.toml", &at_most);
    let run = spec(
        &home,
        &dir,
        &["init", "--slug", "big", "--pack", "pack.toml"],
    );
    run.code(0);
    assert!(
        read(&dir, "specengine.toml") == installed("big", at_most.as_bytes()),
        "1048576 bytes install"
    );

    let dir = scratch.dir("j-dir");
    fs::create_dir(dir.join("packs")).expect("a directory");
    let run = spec(&home, &dir, &["init", "--pack", "packs"]);
    assert_refused(&run, &not_regular("packs"), &dir, &["packs"], "j-dir");
}

/// The refusal of a path that is no regular file.
fn not_regular(given: &str) -> String {
    format!("--pack {given}: cannot read: not a regular file; a pack is a file's path")
}

/// The longest a refused non-regular pack may take: a hang is a failure.
const AT_ONCE: Duration = Duration::from_secs(10);

/// `spec init --pack given` in `dir` with `HOME=home`, killed and failed
/// after [`AT_ONCE`].
fn init_at_once(home: &Path, dir: &Path, given: &str) -> Run {
    let mut child = Command::new(common::SPEC)
        .env_clear()
        .env("HOME", home)
        .current_dir(dir)
        .args(["init", "--pack", given])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn spec");
    let started = Instant::now();
    loop {
        if child.try_wait().expect("wait on spec").is_some() {
            break;
        }
        if started.elapsed() > AT_ONCE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("spec init --pack {given} still ran after {AT_ONCE:?}: it waits on the pack");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().expect("the output");
    Run {
        args: vec!["init".into(), "--pack".into(), given.into()],
        code: output.status.code().expect("an exit code"),
        stdout: String::from_utf8(output.stdout).expect("UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("UTF-8"),
    }
}

/// PK2 (j) and P3, iteration 2 of the developer: a named pipe with no
/// writer, a symlink to it, and a directory each return at once, exit 2,
/// `cannot read: not a regular file`, nothing written. M: the pack opened
/// blocking, with no check of the handle.
#[test]
fn pk2_j_a_pipe_a_link_to_it_and_a_directory_return_at_once() {
    let scratch = Scratch::new("pk2-pipe");
    let home = scratch.home("h");
    let dir = scratch.dir("pipes");
    let fifo = dir.join("fifo");
    let made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("run mkfifo");
    assert!(made.success(), "mkfifo");
    std::os::unix::fs::symlink("fifo", dir.join("link")).expect("a symlink");
    fs::create_dir(dir.join("packs")).expect("a directory");
    for given in ["fifo", "link", "packs"] {
        let run = init_at_once(&home, &dir, given);
        run.code(2);
        assert_eq!(run.stdout, "", "{given}: {}", run.show());
        assert_eq!(
            run.stderr,
            format!("spec: {}\n", not_regular(given)),
            "{given}: {}",
            run.show()
        );
        // Names only: `snapshot` would read the pipe and wait on it.
        let mut names: Vec<String> = fs::read_dir(&dir)
            .expect("the directory")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(names, ["fifo", "link", "packs"], "{given}: nothing written");
    }
}

/// PK2 and "Checks, in order" 1: today's checks come first: a bad `--slug`
/// with a missing pack names `--slug`, and `--config` is refused before the
/// pack is read.
#[test]
fn pk2_todays_checks_come_first() {
    let scratch = Scratch::new("pk2-order");
    let home = scratch.home("h");
    let dir = scratch.dir("order");
    let run = spec(
        &home,
        &dir,
        &["init", "--slug", "Not A Slug", "--pack", "missing.toml"],
    );
    run.code(2);
    assert!(run.stderr.starts_with("spec: --slug: "), "{}", run.show());
    let run = spec(
        &home,
        &dir,
        &["--config", "x.toml", "init", "--pack", "missing.toml"],
    );
    run.code(2);
    assert!(
        run.stderr
            .starts_with("spec: `spec init` takes no --config"),
        "{}",
        run.show()
    );
    assert_nothing_written(&dir, &[], "order");
}

/// PK2, edge data (iteration 2 of the developer): a pack led by a byte-order
/// mark is refused at its line 1 with its own message, nothing written: the
/// mark would land inside the file, after the header. M: the mark's check
/// dropped (the TOML reader's `key with no value` instead).
#[test]
fn pk2_a_byte_order_mark_is_refused_at_line_1() {
    let scratch = Scratch::new("pk2-bom");
    let home = scratch.home("h");
    let dir = scratch.dir("bom");
    let mut pack = "\u{feff}".as_bytes().to_vec();
    pack.extend_from_slice(preset("software").as_bytes());
    write(&dir, "pack.toml", &pack);
    let run = spec(&home, &dir, &["init", "--pack", "pack.toml"]);
    assert_refused(
        &run,
        "pack.toml:1: a pack starts with a byte-order mark; save it as UTF-8 without one",
        &dir,
        &["pack.toml"],
        "bom",
    );
}

/// "Checks, in order" 6: under a configured project a pack still installs,
/// with today's one `warning:`, the ancestor's file untouched.
#[test]
fn pk2_a_config_above_warns_and_the_pack_installs() {
    let scratch = Scratch::new("pk2-above");
    let home = scratch.home("h");
    let outer = scratch.dir("outer");
    let own = b"[project]\nslug = \"outer\"\n".to_vec();
    write(&outer, "specengine.toml", &own);
    let inner = scratch.dir("outer/inner");
    let software = fs::read(preset_path("software")).expect("preset");
    write(&inner, "p.toml", &software);
    let run = spec(&home, &inner, &["init", "--pack", "p.toml"]);
    run.code(0);
    assert_eq!(
        run.stderr_lines(),
        ["warning: ../specengine.toml already configures a project above this directory"],
        "{}",
        run.show()
    );
    assert_eq!(
        read(&inner, "specengine.toml"),
        installed("inner", &software)
    );
    assert_eq!(read(&outer, "specengine.toml"), own, "the ancestor's kept");
}

// ----------------------------------------------------------------- PK3

/// PK3 (a): plant-diary holds the software preset verbatim between its
/// markers, once each. M: a flow title changed in the fixture's copy alone.
#[test]
fn pk3_a_plant_diary_holds_the_software_preset_verbatim() {
    let text = read_text(&fixture("plant-diary"), "specengine.toml");
    assert!(
        between_markers(&text, SOFTWARE_OPENS) == preset("software"),
        "fixtures/plant-diary holds scripts/presets/software.toml verbatim between its markers"
    );
}

/// PK3 (b), (c): on a copy, `spec check` exits 0 with no finding, and `spec
/// workspace` prints `expected.json` `workspaces` with no note; the tree is
/// the five documents of "Data" plus one ID'd section of a story, which the
/// store's own-text oracle needs of every corpus (a story's, so the split's
/// `epic` targets stay `EPC-WATERING` alone).
#[test]
fn pk3_bc_plant_diary_checks_clean_and_serves_its_workspaces() {
    let scratch = Scratch::new("pk3-fixture");
    let home = scratch.home("h");
    let root = scratch.copy("plant-diary", "plant");
    let expected: Value =
        serde_json::from_str(&read_text(&fixture("plant-diary"), "expected.json")).expect("JSON");

    let check = spec(&home, &root, &["check", "--json"]);
    check.code(0);
    let document = check.json();
    for count in ["errors", "warnings", "debt", "expired", "stale"] {
        assert_eq!(
            document["counts"][count],
            json!(0),
            "{count}: {}",
            check.show()
        );
    }
    assert_eq!(document["counts"]["documents"], json!(5));
    assert_eq!(document["findings"], json!([]), "{}", check.show());

    assert_eq!(json!(workspace_text(&home, &root)), expected["workspaces"]);

    let tree = spec(&home, &root, &["tree", "--json"]);
    tree.code(0);
    let mut nodes: Vec<(String, String, Value)> = tree.json()["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            (
                node["id"].as_str().expect("an ID").to_owned(),
                node["kind"].as_str().expect("a kind").to_owned(),
                node["parent"].clone(),
            )
        })
        .collect();
    nodes.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        nodes,
        [
            ("EPC-HOUSEHOLD".to_owned(), "epic".to_owned(), json!(null)),
            ("EPC-WATERING".to_owned(), "epic".to_owned(), json!(null)),
            (
                "STR-INVITE".to_owned(),
                "story".to_owned(),
                json!("EPC-HOUSEHOLD")
            ),
            (
                "STR-SCHEDULE".to_owned(),
                "story".to_owned(),
                json!("EPC-WATERING")
            ),
            (
                "STR-SCHEDULE-QUIET".to_owned(),
                "story".to_owned(),
                json!("STR-SCHEDULE")
            ),
            (
                "STR-SNOOZE".to_owned(),
                "story".to_owned(),
                json!("EPC-WATERING")
            ),
        ]
    );
    let mut prefixes: Vec<&str> = nodes.iter().map(|(id, _, _)| &id[..3]).collect();
    prefixes.dedup();
    assert_eq!(json!(prefixes), expected["prefixes"]);
    let mut kinds: Vec<&str> = nodes.iter().map(|(_, kind, _)| kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(json!(kinds), expected["kinds"]);
}

/// PK3 (d): `spec init --pack scripts/presets/software.toml`, the path
/// resolved against the current directory (the repository), the project at
/// `--root`: 5 workspaces, `modules 0`, no note; `card list` serves the 5
/// flows, each `return` `reopened`.
#[test]
fn pk3_d_the_software_pack_installs_and_reads() {
    let scratch = Scratch::new("pk3-install");
    let home = scratch.home("h");
    let dir = scratch.dir("team-app");
    let given = "scripts/presets/software.toml";
    let run = spec(
        &home,
        &repository_root(),
        &["--root", path_text(&dir), "init", "--pack", given],
    );
    run.code(0);
    assert_eq!(
        run.stdout,
        format!("created specengine.toml with slug team-app from {given}\n"),
        "{}",
        run.show()
    );
    assert_eq!(
        read(&dir, "specengine.toml"),
        installed(
            "team-app",
            &fs::read(preset_path("software")).expect("preset")
        )
    );
    let text = workspace_text(&home, &dir);
    assert!(
        text.starts_with("workspaces config | workspaces 5, modules 0\n"),
        "{text}"
    );
    let flows = listed_flows(&home, &dir);
    let want: Vec<(String, String, Value)> = SOFTWARE_FLOWS
        .iter()
        .map(|(name, title)| ((*name).to_owned(), (*title).to_owned(), json!("reopened")))
        .collect();
    assert_eq!(flows, want);
}

/// PK3 (e): a dry-run split on `EPC-WATERING` plans 5 children in flow
/// order: the row's four disciplines suggested, each targeting
/// `["EPC-WATERING"]` from `kinds`; `documentation` not suggested, targeting
/// the parent's. The spec's command names no `--discipline`; the split
/// refuses that (exit 1, "no flow takes a card without a discipline"), so
/// the parent's is `product`, the first flow's.
#[test]
fn pk3_e_a_split_on_plant_diary_suggests_the_rows_disciplines() {
    let scratch = Scratch::new("pk3-split");
    let home = scratch.home("h");
    let root = scratch.copy("plant-diary", "plant");
    let args = [
        "card",
        "split",
        "--target",
        "EPC-WATERING",
        "--title",
        "Watering reminders",
        "--discipline",
        "product",
        "--dry-run",
        "--json",
    ];
    let run = spec(&home, &root, &args);
    run.code(0);
    let document = run.json();
    assert_eq!(document["dry_run"], json!(true));
    assert_eq!(document["parent"]["targets"], json!(["EPC-WATERING"]));
    assert_eq!(document["notes"], json!([]), "{}", run.show());
    let children: Vec<(String, Value, Value, Value)> = document["children"]
        .as_array()
        .expect("children")
        .iter()
        .map(|child| {
            (
                child["discipline"]
                    .as_str()
                    .expect("a discipline")
                    .to_owned(),
                child["targets"].clone(),
                child["from"].clone(),
                child["suggested"].clone(),
            )
        })
        .collect();
    let target = json!(["EPC-WATERING"]);
    let want: Vec<(String, Value, Value, Value)> = SOFTWARE_FLOWS
        .iter()
        .map(|(name, _)| {
            let row = *name != "documentation";
            (
                (*name).to_owned(),
                target.clone(),
                json!(if row { "kinds" } else { "parent" }),
                json!(row),
            )
        })
        .collect();
    assert_eq!(children, want, "{}", run.show());
    assert_eq!(
        spec(&home, &root, &args).stdout,
        run.stdout,
        "one state, one plan"
    );
}

// ----------------------------------------------------------------- PK4

/// PK4: with a `specengine.toml` present, `spec init` and `spec init --pack
/// <a valid pack>` each exit 2 and leave its bytes. M: the file overwritten.
#[test]
fn pk4_an_existing_config_is_never_merged() {
    let scratch = Scratch::new("pk4");
    let home = scratch.home("h");
    let dir = scratch.dir("present");
    let own = b"# the owner's own file\n[project]\nslug = \"present\"\n".to_vec();
    write(&dir, "specengine.toml", &own);
    let pack = preset_path("software");
    for args in [
        &["init"][..],
        &["init", "--pack", path_text(&pack)],
        &["--json", "init", "--pack", path_text(&pack)],
    ] {
        let run = spec(&home, &dir, args);
        run.code(2);
        assert_eq!(run.stdout, "", "{args:?}");
        assert_eq!(
            run.stderr,
            "spec: specengine.toml already exists; nothing was written\n",
            "{args:?}: {}",
            run.show()
        );
        assert_eq!(read(&dir, "specengine.toml"), own, "{args:?}: bytes kept");
        let entries: Vec<String> = snapshot(&dir).into_keys().collect();
        assert_eq!(entries, ["specengine.toml"], "{args:?}");
    }
}

// ----------------------------------------------------------------- PK5

/// PK5: `--pack gamedev` is a path. With no file by that name it exits 2,
/// `--pack gamedev: cannot read: ...; a pack is a file's path`, writing
/// nothing; with a file `gamedev` holding the software preset's bytes it
/// installs those. M: a preset embedded by name.
#[test]
fn pk5_a_pack_is_a_path_never_a_name() {
    let scratch = Scratch::new("pk5");
    let home = scratch.home("h");
    let dir = scratch.dir("by-path");
    let run = spec(&home, &dir, &["init", "--pack", "gamedev"]);
    run.code(2);
    assert_eq!(run.stdout, "", "{}", run.show());
    assert!(
        run.stderr
            .starts_with("spec: --pack gamedev: cannot read: ")
            && run.stderr.ends_with("; a pack is a file's path\n"),
        "{}",
        run.show()
    );
    assert_nothing_written(&dir, &[], "no file gamedev");

    let software = fs::read(preset_path("software")).expect("preset");
    write(&dir, "gamedev", &software);
    let run = spec(&home, &dir, &["init", "--pack", "gamedev"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "created specengine.toml with slug by-path from gamedev\n"
    );
    assert_eq!(
        read(&dir, "specengine.toml"),
        installed("by-path", &software)
    );
}

// ----------------------------------------------------------------- PK6

/// PK6: no pack, no change: `spec init` and `spec init --slug x` write the
/// base commit's bytes and text, exit 0; JSON `{path, slug, pack: null}`.
/// M: the scaffold dropped without a pack.
#[test]
fn pk6_no_pack_no_change() {
    let scratch = Scratch::new("pk6");
    let home = scratch.home("h");
    for (dir_name, args, slug) in [
        ("plain-dir", &["init"][..], "plain-dir"),
        ("slugged", &["init", "--slug", "x"], "x"),
    ] {
        let dir = scratch.dir(dir_name);
        let run = spec(&home, &dir, args);
        run.code(0);
        assert_eq!(
            run.stdout,
            format!("created specengine.toml with slug {slug}\n")
        );
        assert_eq!(run.stderr, "");
        assert_eq!(
            read(&dir, "specengine.toml"),
            format!("[project]\nslug = \"{slug}\"\n{SCAFFOLD}").into_bytes(),
            "{args:?}: the scaffold as before"
        );
    }
    let dir = scratch.dir("json-dir");
    let run = spec(&home, &dir, &["--json", "init"]);
    run.code(0);
    assert_eq!(
        run.stdout,
        "{\"path\":\"specengine.toml\",\"slug\":\"json-dir\",\"pack\":null}\n"
    );
}

// ----------------------------------------------------------------- PK7

/// PK7: `scripts/presets/gamedev.toml` opens with "The gamedev header"
/// (studio-lamp's copy is core's `workspace_toml.rs` WS-7).
#[test]
fn pk7_the_gamedev_preset_opens_with_its_header() {
    let text = preset("gamedev");
    assert!(
        text.starts_with(GAMEDEV_HEADER),
        "the preset opens with the header:\n{}",
        &text[..text.len().min(600)]
    );
    let lamp = read_text(&fixture("studio-lamp"), "specengine.toml");
    assert!(
        between_markers(&lamp, "# --- scripts/presets/gamedev.toml ---\n") == text,
        "studio-lamp holds the preset verbatim"
    );
}

// ------------------------------------------------- release-hardening RH-R4

/// docs/features/release-hardening.md "Acceptance criteria" RH-R4: a split
/// row's error is no longer the card reader's `Err` but its split problem,
/// yet `spec init --pack` of the software preset with the "Data" row
/// (`disciplnes`) appended still refuses at the pack's line, nothing
/// written. M: `init` not judging the split.
#[test]
fn rh_r4_a_pack_holding_a_bad_split_row_is_refused() {
    let scratch = Scratch::new("rh-r4-pack");
    let home = scratch.home("h");
    let pack = format!(
        "{}\n[[card.split]]\nkind = \"mechanic\"\ndisciplnes = [\"copy\"]\n",
        preset("software")
    );
    let line = pack
        .lines()
        .position(|text| text.starts_with("disciplnes"))
        .expect("the row")
        + 1;
    refused_with(
        &scratch,
        &home,
        "split-row",
        pack.as_bytes(),
        &format!("pack.toml:{line}: unknown field `disciplnes`, expected `kind` or `disciplines`"),
    );
}
