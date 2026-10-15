//! docs/features/code-refs.md RD-1, RD-2, RD-6, RD-7, TE-4 to TE-7: the
//! `[code]` table, the code walk, determinism, read-only, the genre of the
//! new modules, and `spec check` blind to code — over scratch copies of
//! `fixtures/code-a` (and `fixtures/spec-c`, which has no `[code]`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde_json::Value;

use common::bundle::blake3_hex;
use common::git::Sandbox;
use common::{Run, Scratch, fixture, read_text, repository_root, spec, write};

/// The fixture's config without its `[code]` table.
fn config_without_code() -> String {
    let text = fs::read_to_string(fixture("code-a").join("specengine.toml")).expect("config");
    let at = text.find("\n[code]\n").expect("code-a has a [code] table");
    text[..=at].to_owned()
}

fn with_code(code: &str) -> String {
    format!("{}{code}", config_without_code())
}

/// The 1-based line of the first line of `text` starting with `needle`.
fn line_starting(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.starts_with(needle))
        .unwrap_or_else(|| panic!("no line starts with {needle:?}"))
        + 1
}

fn exit_2_quiet(run: &Run) {
    run.code(2);
    assert_eq!(
        run.stdout,
        "",
        "stdout stays empty on exit 2:\n{}",
        run.show()
    );
}

// ------------------------------------------------------------------ RD-1

/// RD-1: a closed table. Another key (07 s5's `languages` too), a glob of
/// another dialect, `roots = []`, no `roots`, a path out of the root, a
/// wrong type: exit 2, `specengine.toml:<line>:` at the key, for
/// `spec refs` only — `spec check` and `spec show` name-check `[code]`
/// and answer. M: accept an unknown key.
#[test]
fn code_table_unknown_key_is_refused() {
    let scratch = Scratch::new("code-table");
    let root = scratch.copy("code-a", "code-a");
    let home = scratch.home("h");
    let cases: [(&str, &str); 7] = [
        ("[code]\nroots = [\"crates\"]\ncolour = 1\n", "colour"),
        (
            "[code]\nroots = [\"crates\"]\nlanguages = [\"rust\"]\n",
            "languages",
        ),
        (
            "[code]\nroots = [\"crates\"]\nexclude = [\"a/[b]\"]\n",
            "exclude",
        ),
        ("[code]\nroots = []\n", "roots"),
        ("[code]\nroots = [\"../x\"]\n", "roots"),
        ("[code]\nroots = \"crates\"\n", "roots"),
        ("[code]\nexclude = []\n", "[code]"),
    ];
    for (table, key) in cases {
        let config = with_code(table);
        write(&root, "specengine.toml", &config);
        let line = line_starting(&config, key);
        for args in [&["refs", "GOAL-01"][..], &["--json", "refs", "GOAL-01"][..]] {
            let run = spec(&home, &root, args);
            exit_2_quiet(&run);
            let prefix = format!("specengine.toml:{line}: ");
            assert!(
                run.stderr.starts_with(&prefix) && run.stderr_lines().len() == 1,
                "{table:?}: want one line starting {prefix:?}\n{}",
                run.show()
            );
        }
        spec(&home, &root, &["check"]).code(0);
        spec(&home, &root, &["show", "GOAL-01"]).code(0);
    }
}

// ------------------------------------------------------------------ RD-6

const NO_TABLE: &str =
    "spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)\n";

/// RD-6. M: exit 0 without `[code]`. Exactly the one line, stdout empty,
/// text and JSON; before the REF is read (a REF naming nothing too).
#[test]
fn refs_without_code_table_is_exit_2() {
    let scratch = Scratch::new("no-table");
    let root = scratch.copy("code-a", "code-a");
    let home = scratch.home("h");
    write(&root, "specengine.toml", config_without_code());
    for args in [
        &["refs", "GOAL-01"][..],
        &["--json", "refs", "GOAL-01"][..],
        &["refs", "GOAL-99"][..],
    ] {
        let run = spec(&home, &root, args);
        exit_2_quiet(&run);
        assert_eq!(run.stderr, NO_TABLE, "{}", run.show());
    }
}

/// TE-6: `fixtures/spec-c` (a project of another genre, no `[code]`)
/// exits 2 naming `[code]`.
#[test]
fn refs_on_spec_c_exits_2_naming_the_code_table() {
    let scratch = Scratch::new("spec-c");
    let root = scratch.copy("spec-c", "spec-c");
    let home = scratch.home("h");
    let run = spec(&home, &root, &["refs", "WISH-001"]);
    exit_2_quiet(&run);
    assert_eq!(run.stderr, NO_TABLE, "{}", run.show());
}

// ------------------------------------------------------------------ RD-2

/// RD-2. M: follow a symlink. A file and a directory linked under a root,
/// a root that is a link: none is read; the link root gets its note.
#[test]
fn code_walk_skips_symlinks() {
    let scratch = Scratch::new("symlinks");
    let plain = scratch.copy("code-a", "plain");
    let root = scratch.copy("code-a", "linked");
    let home = scratch.home("h");
    let outside = "// @implements GOAL-01@1\npub fn outside() {}\n";
    write(&root, "outside/extra.rs", outside);
    write(
        &root,
        "outside/extra.ron",
        "// @configures GOAL-01\n(a: 1)\n",
    );
    std::os::unix::fs::symlink(
        "../../../outside/extra.rs",
        root.join("crates/tally/src/linked.rs"),
    )
    .expect("file link");
    std::os::unix::fs::symlink("../outside", root.join("data/linked")).expect("dir link");
    std::os::unix::fs::symlink("outside", root.join("linked-root")).expect("root link");
    write(
        &root,
        "specengine.toml",
        with_code(
            "[code]\nroots = [\"crates\", \"data\", \"tools/loose.rs\", \"linked-root\"]\nexclude = [\"crates/*/tests/fixtures/**\"]\n",
        ),
    );
    let want = spec(&home, &plain, &["refs", "GOAL-01"]);
    want.code(0);
    let run = spec(&home, &root, &["refs", "GOAL-01"]);
    run.code(0);
    assert_eq!(run.stdout, want.stdout, "no row from a link");
    assert_eq!(run.stderr, "note: [code] root linked-root does not exist\n");
}

/// RD-2. M: print before the gap. An unlistable directory, an unreadable
/// file: exit 2, stdout empty (text and JSON), the first gap by path.
#[test]
fn code_walk_gap_is_exit_2() {
    let scratch = Scratch::new("gaps");
    let home = scratch.home("h");
    let tests_dir = "crates/tally/tests";
    let ron = "data/limits.ron";
    let lock =
        |path: &Path| fs::set_permissions(path, fs::Permissions::from_mode(0o000)).expect("chmod");

    let dir_root = scratch.copy("code-a", "dir");
    lock(&dir_root.join(tests_dir));
    let file_root = scratch.copy("code-a", "file");
    lock(&file_root.join(ron));
    let both_root = scratch.copy("code-a", "both");
    lock(&both_root.join(tests_dir));
    lock(&both_root.join(ron));

    for (root, gap) in [
        (
            &dir_root,
            format!("the directory `{tests_dir}` cannot be listed"),
        ),
        (&file_root, format!("`{ron}` cannot be read")),
        (
            &both_root,
            format!("the directory `{tests_dir}` cannot be listed"),
        ),
    ] {
        for args in [&["refs", "GOAL-01"][..], &["--json", "refs", "GOAL-01"][..]] {
            let run = spec(&home, root, args);
            exit_2_quiet(&run);
            let want = format!("spec: the code walk is incomplete: {gap}");
            assert!(
                run.stderr.starts_with(&want) && run.stderr_lines().len() == 1,
                "want {want:?}\n{}",
                run.show()
            );
        }
    }
}

/// "The code table", "The walk": a missing root and a root of another
/// file type are notes (config order) and contribute nothing; content
/// that is not UTF-8 is a note, skipped and counted.
#[test]
fn code_walk_notes_missing_roots_and_skips_content_that_is_not_utf8() {
    let scratch = Scratch::new("notes");
    let root = scratch.copy("code-a", "code-a");
    let home = scratch.home("h");
    write(
        &root,
        "specengine.toml",
        with_code(
            "[code]\nroots = [\"absent\", \"crates\", \"crates/tally/Cargo.toml\", \"data\", \"tools/loose.rs\"]\nexclude = [\"crates/*/tests/fixtures/**\"]\n",
        ),
    );
    write(&root, "crates/tally/src/latin.rs", b"// caf\xe9 GOAL-01\n");
    let run = spec(&home, &root, &["refs", "PREM-001"]);
    run.code(0);
    assert_eq!(
        run.stderr,
        "note: [code] root absent does not exist\n\
         note: [code] root crates/tally/Cargo.toml is not a .rs or .ron file\n\
         note: crates/tally/src/latin.rs is not UTF-8: skipped\n"
    );
    assert!(
        run.stdout
            .ends_with("refs: markers 3, mentions 0; code files 8 (.rs 7, .ron 1), skipped 1\n"),
        "{}",
        run.show()
    );
    let json = spec(&home, &root, &["--json", "refs", "PREM-001"]);
    let value = json.json();
    assert_eq!(value["notes"].as_array().expect("notes").len(), 3);
    assert_eq!(
        value["code"],
        serde_json::json!({"files": 8, "rs": 7, "ron": 1, "skipped": 1})
    );
}

/// "Output and the cap": rows while within `OUTPUT_CAP_CHARS`, then the
/// tail; the summary counts all; JSON holds the printed rows and
/// `omitted` the rest.
#[test]
fn refs_rows_are_capped_with_a_tail() {
    let scratch = Scratch::new("cap");
    let root = scratch.copy("code-a", "code-a");
    let home = scratch.home("h");
    let mut many = String::new();
    for index in 0..600 {
        many.push_str(&format!(
            "// @implements GOAL-01@1 [body] note {index}\npub fn f{index}() {{}}\n"
        ));
    }
    write(&root, "crates/tally/src/many.rs", many);
    let run = spec(&home, &root, &["refs", "GOAL-01"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let tail = lines[lines.len() - 2];
    assert!(
        tail.starts_with("[truncated: ") && tail.ends_with(" mentions not shown]"),
        "{tail}"
    );
    assert!(
        lines[lines.len() - 1].starts_with("refs: markers 612, mentions 4; "),
        "{}",
        lines[lines.len() - 1]
    );
    let before_tail: usize = lines[..lines.len() - 2]
        .iter()
        .map(|line| line.chars().count() + 1)
        .sum();
    assert!(
        before_tail <= 40_000,
        "{before_tail} characters before the tail"
    );
    let json = spec(&home, &root, &["--json", "refs", "GOAL-01"]).json();
    let shown = json["nodes"][0]["markers"]
        .as_array()
        .expect("markers")
        .len()
        + json["nodes"][0]["mentions"]
            .as_array()
            .expect("mentions")
            .len();
    let omitted = json["omitted"]["markers"].as_u64().expect("omitted")
        + json["omitted"]["mentions"].as_u64().expect("omitted");
    assert_eq!(shown + usize::try_from(omitted).expect("usize"), 616);
    assert!(omitted > 0);
    assert_eq!(lines.len() - 3, shown, "text and JSON show the same rows");
}

// ------------------------------------------------------------------ TE-4

/// Copies `from` to `to` creating every entry in reverse name order.
fn copy_reversed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("target");
    let mut entries: Vec<_> = fs::read_dir(from)
        .expect("directory")
        .map(|entry| entry.expect("entry"))
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in entries {
        let target = to.join(entry.file_name());
        if entry.file_type().expect("type").is_dir() {
            copy_reversed(&entry.path(), &target);
        } else {
            fs::write(&target, fs::read(entry.path()).expect("read")).expect("write");
        }
    }
}

fn write_generated(root: &Path, reversed: bool) {
    let expected: Value = serde_json::from_str(&read_text(&fixture("code-a"), "expected.json"))
        .expect("expected.json");
    let mut files: Vec<&Value> = expected["generated"]["files"]
        .as_array()
        .expect("files")
        .iter()
        .collect();
    if reversed {
        files.reverse();
    }
    for file in files {
        let mut text = String::new();
        for part in file["parts"].as_array().expect("parts") {
            let times = usize::try_from(part["times"].as_u64().expect("times")).expect("usize");
            text.push_str(&part["text"].as_str().expect("text").repeat(times));
        }
        write(root, file["path"].as_str().expect("path"), text);
    }
}

const REFS: [&str; 9] = [
    "PIECE-COUNTER",
    "LAW-BOUND",
    "LAW-STEP",
    "LAW-ODD",
    "GOAL-01",
    "PREM-001",
    "cap-check/PASS-01",
    "GOAL-03",
    "GOAL-99",
];

/// RD-6, TE-4. M: discovery order. Two roots at other absolute paths under
/// other names, one created in reverse order, read from the root and from
/// another cwd with `--root`, each with its own `HOME`, twice: stdout,
/// stderr and exit byte for byte alike, text and JSON.
#[test]
fn refs_is_byte_identical_across_roots_and_order() {
    let scratch = Scratch::new("determinism");
    let first = scratch.copy("code-a", "one/code-a");
    write_generated(&first, false);
    let second = scratch.dir("two/elsewhere-else");
    copy_reversed(&fixture("code-a"), &second);
    write_generated(&second, true);
    let cwd = scratch.dir("somewhere");
    let (home_a, home_b) = (scratch.home("a"), scratch.home("b"));
    let second_text = second.to_str().expect("UTF-8 scratch");
    for reference in REFS {
        for json in [false, true] {
            let mut args: Vec<&str> = if json { vec!["--json"] } else { Vec::new() };
            args.extend(["refs", reference]);
            let a = spec(&home_a, &first, &args);
            let again = spec(&home_a, &first, &args);
            let mut rooted = vec!["--root", second_text];
            rooted.extend(&args);
            let b = spec(&home_b, &cwd, &rooted);
            for other in [&again, &b] {
                assert_eq!(
                    (a.code, &a.stdout, &a.stderr),
                    (other.code, &other.stdout, &other.stderr),
                    "{args:?}"
                );
            }
        }
    }
}

/// TE-4 over iteration 1's deviation 6: a root `Cargo.toml` without
/// `[package] name` (here a virtual manifest) named the root package after
/// the root directory itself, so its `src/main.rs` bin unit (beside a lib,
/// not the primary target) carried the checkout's directory name.
/// "Rules and edge cases": byte-identical whatever the absolute root.
/// Since iteration 2 a virtual manifest has no targets (its files are
/// unrooted): `manifests_name_packages_alike_from_any_root`.
#[test]
fn an_unnamed_root_package_is_named_alike_from_any_root() {
    let scratch = Scratch::new("unnamed-root");
    let home = scratch.home("h");
    let mut outputs = Vec::new();
    for dir in ["one/alpha", "two/beta"] {
        let root = scratch.dir(dir);
        write(
            &root,
            "specengine.toml",
            "[project]\nslug = \"unnamed\"\n\n[ids]\nGOAL = { kind = \"objective\", width = 2 }\n\n[code]\nroots = [\"src\"]\n",
        );
        write(&root, "Cargo.toml", "[workspace]\n");
        write(&root, "src/lib.rs", "pub fn shared() {}\n");
        write(
            &root,
            "src/main.rs",
            "// @implements GOAL-01@1\nfn main() {}\n",
        );
        write(
            &root,
            "docs/records/GOAL-01.md",
            read_text(&fixture("code-a"), "docs/records/GOAL-01.md"),
        );
        let run = spec(&home, &root, &["refs", "GOAL-01"]);
        run.code(0);
        outputs.push(run.stdout);
    }
    assert_eq!(
        outputs[0], outputs[1],
        "the same tree under another directory name prints another qpath"
    );
    // A `[workspace]`-only root is virtual: no targets, its files unrooted.
    assert!(
        outputs[0].contains(
            "\nimplements | src/main.rs:1:4 | GOAL-01@1 | resolved | current | default | item .::main function_item ambiguous unrooted\n"
        ),
        "{}",
        outputs[0]
    );
}

// ------------------------------------------------------------- RD-7, TE-5

/// `(path, size, mtime ns, BLAKE3)` of every code file and manifest under
/// `root`, `.git` left out.
fn code_files(root: &Path) -> Vec<(String, u64, i128, String)> {
    let mut out = Vec::new();
    for (path, bytes) in common::snapshot(root) {
        let Some(bytes) = bytes else { continue };
        if path.starts_with(".git") {
            continue;
        }
        if path.ends_with(".rs") || path.ends_with(".ron") || path.ends_with("Cargo.toml") {
            let metadata = fs::symlink_metadata(root.join(&path)).expect("metadata");
            let mtime =
                i128::from(metadata.mtime()) * 1_000_000_000 + i128::from(metadata.mtime_nsec());
            out.push((path, metadata.len(), mtime, blake3_hex(&bytes)));
        }
    }
    out
}

fn worktree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    common::snapshot(root)
        .into_iter()
        .filter(|(path, _)| path != ".git" && !path.starts_with(".git/"))
        .collect()
}

/// RD-7, TE-5. M: a cache file under the root. In a git repository every
/// `refs` (answers, exit 1, exit 2) leaves `git status --porcelain` empty
/// (ignored files too), every code file's (path, size, mtime, BLAKE3) and
/// the whole worktree unchanged.
#[test]
fn refs_leaves_the_worktree_untouched() {
    let scratch = Scratch::new("read-only");
    let sandbox = Sandbox::new(scratch.path());
    let repo = scratch.copy("code-a", "repo");
    write_generated(&repo, false);
    sandbox.init(&repo);
    sandbox.add_all(&repo);
    sandbox.commit(&repo, "the fixture");
    let status = || {
        sandbox.git_text(
            &repo,
            &[
                "--no-optional-locks",
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--ignored",
            ],
        )
    };
    assert_eq!(status(), "", "a clean repository to begin with");
    let code_before = code_files(&repo);
    assert!(code_before.len() >= 12, "{code_before:?}");
    let tree_before = worktree(&repo);
    let home = scratch.home("h");
    let mut references: Vec<&str> = REFS.to_vec();
    references.push("\u{0420}IECE-COUNTER");
    for reference in references {
        spec(&home, &repo, &["refs", reference]);
        spec(&home, &repo, &["--json", "refs", reference]);
    }
    assert_eq!(status(), "", "refs wrote under the root");
    assert_eq!(code_files(&repo), code_before);
    assert_eq!(worktree(&repo), tree_before);
}

// ------------------------------------------------------------------ TE-7

/// TE-7. M: check reading code. `spec check` (text and JSON) on code-a
/// with dangling markers added equals `spec check` with every code root
/// removed: no finding, note or count comes from code.
#[test]
fn check_is_the_same_with_and_without_the_code() {
    let scratch = Scratch::new("check-blind");
    let with = scratch.copy("code-a", "with/code-a");
    write(
        &with,
        "crates/tally/src/stray.rs",
        "// @implements GOAL-42@7\n// @verifies PIECE-MISSING\npub fn stray() {}\n",
    );
    let without = scratch.copy("code-a", "without/code-a");
    for dir in ["crates", "data", "tools"] {
        fs::remove_dir_all(without.join(dir)).expect("remove a code root");
    }
    let home = scratch.home("h");
    for args in [&["check"][..], &["--json", "check"][..]] {
        let a = spec(&home, &with, args);
        let b = spec(&home, &without, args);
        assert_eq!(
            (a.code, &a.stdout, &a.stderr),
            (b.code, &b.stdout, &b.stderr),
            "{args:?}"
        );
        assert!(
            a.stdout.contains("5 documents") || args.len() == 2,
            "{}",
            a.show()
        );
    }
}

// ------------------------------------------------------------------ TE-6

/// The new modules of the slice, and `items.rs` since
/// docs/features/symbol-discriminator.md reads `cfg` there (its Q19, RD-3).
const NEW_MODULES: [&str; 6] = [
    "crates/specengine-core/src/code_toml.rs",
    "crates/specengine-code/src/bind.rs",
    "crates/specengine-code/src/naming.rs",
    "crates/specengine-code/src/items.rs",
    "crates/specengine-cli/src/code_walk.rs",
    "crates/specengine-cli/src/refs.rs",
];

/// Stack and genre words no engine source names.
const STACK_WORDS: [&str; 14] = [
    "bevy",
    "ecs",
    "game",
    "player",
    "system",
    "plugin",
    "component",
    "resource",
    "python",
    "pytest",
    "unity",
    "godot",
    "django",
    "react",
];

/// Narrow exemptions, each a whole literal of one file: 07 s5's `[code]`
/// keys, refused by name as not built (the spec's "The code table"), and
/// the binding word `module` of "spec refs" (equal to `fixtures/spec-b`'s
/// kind `module`, not taken from it).
const EXEMPT: [(&str, &str); 5] = [
    ("crates/specengine-core/src/code_toml.rs", "languages"),
    ("crates/specengine-core/src/code_toml.rs", "crates"),
    ("crates/specengine-core/src/code_toml.rs", "bevy"),
    ("crates/specengine-core/src/code_toml.rs", "scip"),
    ("crates/specengine-cli/src/refs.rs", "module"),
];

/// The string literals of Rust `source`, comments skipped (raw strings,
/// char literals and lifetimes told apart).
fn literals(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            let mut depth = 1;
            at += 2;
            while at < chars.len() && depth > 0 {
                if chars[at] == '/' && chars.get(at + 1) == Some(&'*') {
                    depth += 1;
                    at += 2;
                } else if chars[at] == '*' && chars.get(at + 1) == Some(&'/') {
                    depth -= 1;
                    at += 2;
                } else {
                    at += 1;
                }
            }
            continue;
        }
        let word_before = at > 0 && (chars[at - 1].is_alphanumeric() || chars[at - 1] == '_');
        if c == 'r' && !word_before && matches!(next, Some('"' | '#')) {
            let mut hashes = 0;
            let mut j = at + 1;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let start = j + 1;
                let mut end = start;
                loop {
                    if chars[end] == '"'
                        && (0..hashes).all(|k| chars.get(end + 1 + k) == Some(&'#'))
                    {
                        break;
                    }
                    end += 1;
                }
                out.push(chars[start..end].iter().collect());
                at = end + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            let mut text = String::new();
            at += 1;
            while chars[at] != '"' {
                if chars[at] == '\\' {
                    text.push(chars[at]);
                    text.push(chars[at + 1]);
                    at += 2;
                } else {
                    text.push(chars[at]);
                    at += 1;
                }
            }
            out.push(text);
            at += 1;
            continue;
        }
        if c == '\'' {
            if next == Some('\\') {
                let close = (at + 2..chars.len())
                    .find(|&i| chars[i] == '\'')
                    .expect("a char");
                at = close + 1;
                continue;
            }
            if chars.get(at + 2) == Some(&'\'') {
                at += 3;
                continue;
            }
        }
        at += 1;
    }
    out
}

/// The words of a literal, `{…}` format arguments left out: runs of
/// letters, digits, `_` and `-`.
fn words(literal: &str) -> Vec<String> {
    let mut text = String::new();
    let mut depth = 0;
    for c in literal.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => text.push(c),
            _ => {}
        }
    }
    text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .filter(|word| !word.is_empty())
        .map(|word| word.trim_matches('-').to_owned())
        .filter(|word| !word.is_empty())
        .collect()
}

/// Every `specengine.toml` under `fixtures/`: (prefixes and aliases,
/// kinds, project slugs).
fn fixture_vocabulary() -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    fn configs(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("fixtures").flatten() {
            let path = entry.path();
            if path.is_dir() {
                configs(&path, out);
            } else if entry.file_name() == "specengine.toml" {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    configs(&repository_root().join("fixtures"), &mut files);
    let (mut prefixes, mut kinds, mut slugs) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for file in files {
        let Ok(table) = fs::read_to_string(&file)
            .expect("config")
            .parse::<toml::Table>()
        else {
            continue;
        };
        if let Some(slug) = table
            .get("project")
            .and_then(|project| project.get("slug"))
            .and_then(toml::Value::as_str)
        {
            slugs.insert(slug.to_lowercase());
        }
        let Some(ids) = table.get("ids").and_then(toml::Value::as_table) else {
            continue;
        };
        for (prefix, spec) in ids {
            prefixes.insert(prefix.clone());
            if let Some(kind) = spec.get("kind").and_then(toml::Value::as_str) {
                kinds.insert(kind.to_lowercase());
            }
            for alias in spec
                .get("aliases_from")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
            {
                prefixes.insert(alias.as_str().expect("an alias").to_owned());
            }
        }
    }
    (prefixes, kinds, slugs)
}

/// TE-6: no string literal of the slice's new modules holds a fixture
/// prefix or alias (as a word or `PREFIX-…`), a fixture kind or project
/// slug, a fixture path (`fixtures/`, `tally`), or a stack word — but the
/// narrow [`EXEMPT`] list. A module's own `#[cfg(test)] mod` (over the
/// fixture, by design) ends what is scanned.
#[test]
fn new_modules_hold_no_fixture_prefix_kind_path_or_stack_word() {
    let (prefixes, kinds, slugs) = fixture_vocabulary();
    assert!(prefixes.contains("PIECE") && prefixes.contains("AIM") && kinds.contains("pass-mark"));
    assert!(slugs.contains("code-a"));
    let mut offenders = Vec::new();
    let mut scanned = 0;
    for file in NEW_MODULES {
        let whole = fs::read_to_string(repository_root().join(file)).expect("a new module");
        let source = whole
            .find("\n#[cfg(test)]\nmod ")
            .map_or(whole.as_str(), |at| &whole[..at]);
        if file.ends_with("refs.rs") {
            assert!(
                source.contains("fn render_text("),
                "{file}: the code is scanned"
            );
        }
        for literal in literals(source) {
            scanned += 1;
            if EXEMPT.contains(&(file, literal.as_str())) {
                continue;
            }
            let lower = literal.to_lowercase();
            if lower.contains("fixtures/") || lower.contains("tally") {
                offenders.push(format!("{file}: {literal:?} (a fixture path)"));
            }
            for word in words(&literal) {
                let low = word.to_lowercase();
                let prefix = prefixes.contains(&word)
                    || prefixes.iter().any(|p| word.starts_with(&format!("{p}-")));
                if prefix
                    || kinds.contains(&low)
                    || slugs.contains(&low)
                    || STACK_WORDS.contains(&low.as_str())
                {
                    offenders.push(format!("{file}: {literal:?} ({word})"));
                }
            }
        }
    }
    assert!(scanned > 50, "{scanned} literals scanned");
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// The scan's own lexer: comments, raw strings, chars and lifetimes.
#[test]
fn the_literal_scan_reads_strings_only() {
    let source = "// \"no\"\n/* \"no\" /* \"nested\" */ */ fn f<'a>(x: &'a str) -> char { let _ = r#\"raw \"q\"\"#; let _ = \"a\\\"b\"; '\"' }";
    assert_eq!(literals(source), ["raw \"q\"", "a\\\"b"]);
    assert_eq!(words("`[code] {key}` holds a {}"), ["code", "holds", "a"]);
}

// ------------------------------------------------ manifests (iteration 2)

/// The tree `files` (plus `docs/records/GOAL-01.md`) under two roots of
/// other names, `[code] roots = [<roots>]`: `spec refs GOAL-01` prints the
/// same from both; its marker rows.
fn rows_from_two_roots(name: &str, roots: &str, files: &[(&str, &str)]) -> Vec<String> {
    let scratch = Scratch::new(name);
    let home = scratch.home("h");
    let mut outputs = Vec::new();
    for dir in ["one/alpha", "two/beta"] {
        let root = scratch.dir(dir);
        write(
            &root,
            "specengine.toml",
            format!(
                "[project]\nslug = \"manifests\"\n\n[ids]\nGOAL = {{ kind = \"objective\", width = 2 }}\n\n[code]\nroots = [{roots}]\n"
            ),
        );
        write(
            &root,
            "docs/records/GOAL-01.md",
            read_text(&fixture("code-a"), "docs/records/GOAL-01.md"),
        );
        for (path, text) in files {
            write(&root, path, text);
        }
        let run = spec(&home, &root, &["refs", "GOAL-01"]);
        run.code(0);
        assert_eq!(run.stderr, "", "{}", run.show());
        outputs.push(run.stdout);
    }
    assert_eq!(
        outputs[0], outputs[1],
        "{name}: the same tree under another directory name prints another answer"
    );
    outputs[0]
        .lines()
        .filter(|line| line.contains(" | GOAL-01@1 | "))
        .map(str::to_owned)
        .collect()
}

const LIB: &str = "pub fn shared() {}\n";
const MAIN: &str = "// @implements GOAL-01@1\nfn main() {}\n";

/// Review iteration 2, `Manifest` (task spec `code-refs`, "The walk"): a
/// manifest that reads as TOML without `[package]` is virtual — no
/// targets, its files unrooted — at the root and nested; one that cannot
/// be parsed, or names no package, takes the directory's name, `.` for
/// the root; a named one its name. Each answer alike from two roots. M:
/// fall back to the absolute root's name (`bin:alpha`, `bin:beta`).
#[test]
fn manifests_name_packages_alike_from_any_root() {
    let row = |path: &str, binding: &str| {
        vec![format!(
            "implements | {path}:1:4 | GOAL-01@1 | resolved | current | default | {binding}"
        )]
    };
    assert_eq!(
        rows_from_two_roots(
            "virtual-root",
            "\"src\"",
            &[
                ("Cargo.toml", "[workspace]\n"),
                ("src/lib.rs", LIB),
                ("src/main.rs", MAIN)
            ],
        ),
        row(
            "src/main.rs",
            "item .::main function_item ambiguous unrooted"
        )
    );
    assert_eq!(
        rows_from_two_roots(
            "virtual-nested",
            "\"crates\"",
            &[
                ("crates/vws/Cargo.toml", "[workspace]\nmembers = []\n"),
                ("crates/vws/src/lib.rs", LIB),
                ("crates/vws/src/main.rs", MAIN),
            ],
        ),
        row(
            "crates/vws/src/main.rs",
            "item crates/vws::main function_item ambiguous unrooted"
        )
    );
    assert_eq!(
        rows_from_two_roots(
            "unparsable-root",
            "\"src\"",
            &[
                ("Cargo.toml", "[package\nname = \"broken\"\n"),
                ("src/lib.rs", LIB),
                ("src/main.rs", MAIN)
            ],
        ),
        row("src/main.rs", "item .::bin:.::main function_item")
    );
    assert_eq!(
        rows_from_two_roots(
            "nameless-nested",
            "\"crates\"",
            &[
                (
                    "crates/noname/Cargo.toml",
                    "[package]\nversion = \"0.1.0\"\n"
                ),
                ("crates/noname/src/lib.rs", LIB),
                ("crates/noname/src/main.rs", MAIN),
            ],
        ),
        row(
            "crates/noname/src/main.rs",
            "item crates/noname::bin:noname::main function_item"
        )
    );
    assert_eq!(
        rows_from_two_roots(
            "named-nested",
            "\"crates\"",
            &[
                ("crates/named/Cargo.toml", "[package]\nname = \"given\"\n"),
                ("crates/named/src/lib.rs", LIB),
                ("crates/named/src/main.rs", MAIN),
            ],
        ),
        row(
            "crates/named/src/main.rs",
            "item crates/named::bin:given::main function_item"
        )
    );
}

// ------------------------------------- the binder's state (iteration 2)

/// Review iteration 2's API note: in `RustBinding::analysis` an error-free
/// item's state is `HashState::NotHashed`, a value hashing never gives.
/// Nothing outside `bind.rs` reads it: the walk, the naming and `refs`
/// name no `HashState`, and every `.state` they read is a judgement's or a
/// level list's. `spec lock accept`
/// (`docs/canon/spec-lock-accept.md` "Steps") hashes the items it locks
/// through `hash_bound_items` alone: `lock.rs` names
/// `HashState` only for that function's results, and like `lock_regen.rs`
/// reads no item's `.state` (a judgement's, a level list's or a printed
/// row's only).
#[test]
fn nothing_outside_the_binder_reads_an_items_state() {
    for (file, hashes) in [
        ("crates/specengine-cli/src/code_walk.rs", false),
        ("crates/specengine-cli/src/refs.rs", false),
        ("crates/specengine-code/src/naming.rs", false),
        ("crates/specengine-cli/src/lock.rs", true),
        ("crates/specengine-cli/src/lock_regen.rs", false),
    ] {
        let whole = fs::read_to_string(repository_root().join(file)).expect("a source");
        let source = whole
            .find("\n#[cfg(test)]\nmod ")
            .map_or(whole.as_str(), |at| &whole[..at]);
        if hashes {
            assert!(
                source.contains("hash_bound_items("),
                "{file} hashes its items through `hash_bound_items`"
            );
            for other in ["hash_item(", "hash_node("] {
                assert!(
                    !source.contains(other),
                    "{file} hashes through `{other}`, not `hash_bound_items`"
                );
            }
        } else {
            assert!(!source.contains("HashState"), "{file} names HashState");
        }
        let lock_file = file.contains("/lock");
        let mut from = 0;
        while let Some(found) = source[from..].find(".state") {
            let at = from + found;
            from = at + ".state".len();
            let after = source[from..].chars().next();
            if after.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let before: String = source[..at]
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            assert!(
                before == "judged" || before == "levels" || (lock_file && before == "row"),
                "{file}: `{before}.state` reads a state"
            );
        }
    }
}
