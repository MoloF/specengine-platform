//! docs/features/code-refs.md RD-1, RD-2, RD-6, RD-7, TE-4 to TE-7: the
//! `[code]` table, the code walk, determinism, read-only, the genre of the
//! new modules, and `spec check` blind to code — over scratch copies of
//! `fixtures/code-a` (and `fixtures/spec-c`, which has no `[code]`).
//!
//! docs/features/symbol-index.md RD-11 (`[code] crates` refused, Q2),
//! RD-13 (one listing, one parse, the binder's state) and the genre rule
//! of its "Rules and edge cases" (the store, core and the new modules hold
//! no fixture word, `::` split or Cargo literal).
//!
//! docs/features/symbol-reads.md TE-3 and the genre rule of its "Rules and
//! edge cases": the slice's store and CLI modules join the scans; no
//! `tests` or `src` literal (no path is classified) in the store, core or
//! those modules.
//!
//! docs/features/bevy-roles.md RD-4 and RD-8: `[code] frameworks` is a key
//! (`bevy` no longer named as not built, `EXEMPT` 5 -> 4); the slice's
//! store and CLI role modules join the scans, and no core, store or CLI
//! module holds a role word, a source or a framework name, all read from
//! the code crate's data; the detector runs on the analyzer's one parse.
//!
//! docs/features/layer-c-resolve.md R11: `[code] scip` is retired, an
//! unknown key (case 13 of `code_table_unknown_key_is_refused`, `EXEMPT`
//! 4 -> 3), and every exemption names a literal its file still holds
//! (`every_exempt_literal_occurs_in_its_file`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod code_index_common;
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

/// RD-1: a closed table. Another key (07 s5's `languages` too, and
/// `crates`, refused as not built: docs/features/symbol-index.md RD-11,
/// Q2), a glob of another dialect, `roots = []`, no `roots`, a path out of
/// the root, a wrong type: exit 2, `specengine.toml:<line>:` at the key,
/// for `spec refs` only — `spec check` and `spec show` name-check `[code]`
/// and answer. M: accept an unknown key; accept `crates`.
/// docs/features/bevy-roles.md RD-4: `bevy` is an unknown key now (the
/// text names `frameworks` among the keys); `frameworks` of a wrong shape
/// or naming no known framework is refused alike, `check` and `show`
/// still answer; `frameworks = ["bevy"]` reads. M: `bevy` kept in
/// `NOT_BUILT_KEYS`; `spec check` failing on the key.
/// docs/features/layer-c-resolve.md R11 (its "Config", Q11): `scip` is
/// retired, an unknown key with the unknown-key text (case 13). M: `scip`
/// kept in `NOT_BUILT_KEYS`.
#[test]
fn code_table_unknown_key_is_refused() {
    let scratch = Scratch::new("code-table");
    let root = scratch.copy("code-a", "code-a");
    let home = scratch.home("h");
    let cases: [(&str, &str, Option<&str>); 13] = [
        ("[code]\nroots = [\"crates\"]\ncolour = 1\n", "colour", None),
        (
            "[code]\nroots = [\"crates\"]\nlanguages = [\"rust\"]\n",
            "languages",
            None,
        ),
        (
            "[code]\nroots = [\"crates\"]\ncrates = \"cargo-metadata\"\n",
            "crates",
            None,
        ),
        (
            "[code]\nroots = [\"crates\"]\nexclude = [\"a/[b]\"]\n",
            "exclude",
            None,
        ),
        ("[code]\nroots = []\n", "roots", None),
        ("[code]\nroots = [\"../x\"]\n", "roots", None),
        ("[code]\nroots = \"crates\"\n", "roots", None),
        ("[code]\nexclude = []\n", "[code]", None),
        (
            "[code]\nroots = [\"crates\"]\nbevy = true\n",
            "bevy",
            Some(
                "`[code]` has no key `bevy`: it takes `roots` and `exclude` and `frameworks` only",
            ),
        ),
        (
            "[code]\nroots = [\"crates\"]\nframeworks = \"bevy\"\n",
            "frameworks",
            Some("`[code] frameworks` is a string, not a list of names"),
        ),
        (
            "[code]\nroots = [\"crates\"]\nframeworks = [\"Bevy\"]\n",
            "frameworks",
            Some(
                "`[code] frameworks`: `Bevy` is not a name: a lowercase letter, then at most 31 of a-z, 0-9, `_`, `-`",
            ),
        ),
        (
            "[code]\nroots = [\"crates\"]\nframeworks = [\"loom\"]\n",
            "frameworks",
            None,
        ),
        (
            "[code]\nroots = [\"crates\"]\nscip = false\n",
            "scip",
            Some(
                "`[code]` has no key `scip`: it takes `roots` and `exclude` and `frameworks` only",
            ),
        ),
    ];
    for (table, key, message) in cases {
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
            if let Some(message) = message {
                assert_eq!(run.stderr, format!("{prefix}{message}\n"), "{table:?}");
            }
        }
        spec(&home, &root, &["check"]).code(0);
        spec(&home, &root, &["show", "GOAL-01"]).code(0);
    }
    let config = with_code("[code]\nroots = [\"crates\"]\nframeworks = [\"bevy\"]\n");
    write(&root, "specengine.toml", &config);
    spec(&home, &root, &["refs", "GOAL-01"]).code(0);
    spec(&home, &root, &["check"]).code(0);
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

/// The store's module of the code tables, the pass and the
/// refresh (docs/canon/symbol-index.md "Crates and seams": the store owns compare,
/// apply and the cross-file pass behind `CodeAnalyzer`).
const STORE_CODE_MODULE: &str = "crates/specengine-store/src/code.rs";

/// The CLI library's `CodeAnalyzer` over `code_walk.rs` and
/// `specengine-code` ("Crates").
const ANALYZER_MODULE: &str = "crates/specengine-cli/src/code_index.rs";

/// docs/features/symbol-reads.md's store module, the tier lookups and the
/// containment as queries over 6a-2's tables (its "Description and
/// interactions", Crates).
const STORE_READS_MODULE: &str = "crates/specengine-store/src/symbols.rs";

/// The CLI module of `spec symbols`.
const SYMBOLS_MODULE: &str = "crates/specengine-cli/src/symbols.rs";

/// The CLI module of `spec unmapped`.
const UNMAPPED_MODULE: &str = "crates/specengine-cli/src/unmapped.rs";

/// docs/features/bevy-roles.md's store module: the role read (L3), role
/// rows with their file's package and their link candidates, strings only
/// (as built).
const STORE_ROLES_MODULE: &str = "crates/specengine-store/src/roles.rs";

/// docs/features/bevy-roles.md's CLI role module: links (L1) and counts
/// (G1), the spec's working name `roles.rs`.
const ROLES_MODULE: &str = "crates/specengine-cli/src/roles.rs";

/// docs/features/health-code.md's modules (TE-5): the CLI's Health and its
/// rows 4-5, the core's `[health]` reader and section selection.
const HEALTH_MODULE: &str = "crates/specengine-cli/src/health.rs";
const HEALTH_CODE_MODULE: &str = "crates/specengine-cli/src/health_code.rs";
const HEALTH_TOML_MODULE: &str = "crates/specengine-core/src/health_toml.rs";
const HEALTH_CHECK_MODULE: &str = "crates/specengine-core/src/check/health.rs";

/// The new modules of the slice, `items.rs` since
/// docs/features/symbol-discriminator.md reads `cfg` there (its Q19, RD-3),
/// the store's and the CLI's code-index modules since
/// docs/features/symbol-index.md ("Rules and edge cases", Genre), the
/// lookup, `symbols` and `unmapped` modules since
/// docs/features/symbol-reads.md (its "Rules and edge cases", TE-3), and
/// the store's and the CLI's role modules since docs/features/bevy-roles.md
/// (RD-8: `NEW_MODULES` 11 + 2; the code crate's `frameworks` and `bevy`
/// name the framework and stay out); Health's four since
/// docs/features/health-code.md (TE-5: 13 + 4).
const NEW_MODULES: [&str; 17] = [
    "crates/specengine-core/src/code_toml.rs",
    "crates/specengine-code/src/bind.rs",
    "crates/specengine-code/src/naming.rs",
    "crates/specengine-code/src/items.rs",
    "crates/specengine-cli/src/code_walk.rs",
    "crates/specengine-cli/src/refs.rs",
    STORE_CODE_MODULE,
    ANALYZER_MODULE,
    STORE_READS_MODULE,
    SYMBOLS_MODULE,
    UNMAPPED_MODULE,
    STORE_ROLES_MODULE,
    ROLES_MODULE,
    HEALTH_MODULE,
    HEALTH_CODE_MODULE,
    HEALTH_TOML_MODULE,
    HEALTH_CHECK_MODULE,
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
/// keys, refused by name as not built (the spec's "The code table"; `bevy`
/// dropped by docs/features/bevy-roles.md RD-4 and `scip` by
/// docs/features/layer-c-resolve.md R11: unknown keys now), and the
/// binding word `module` of "spec refs" (equal to `fixtures/spec-b`'s kind
/// `module`, not taken from it). Each must still occur in its file:
/// [`every_exempt_literal_occurs_in_its_file`].
const EXEMPT: [(&str, &str); 3] = [
    ("crates/specengine-core/src/code_toml.rs", "languages"),
    ("crates/specengine-core/src/code_toml.rs", "crates"),
    ("crates/specengine-cli/src/refs.rs", "module"),
];

/// Narrow word exemptions of the code-index modules: the spec's column
/// names `module` (`code_files`) and `note` (`code_markers`), equal to
/// `fixtures/spec-b`'s kind `module` and a fixture's kind `note`, not
/// taken from them (docs/features/symbol-index.md "Tables"); and of the
/// symbol-reads modules: the target word `module` and the JSON key `note`
/// of `spec symbols`, the binding word `module` the containment reads
/// (docs/features/symbol-reads.md "spec symbols", "spec unmapped"); and
/// Health's row 5 cause, the spec's sentence ``no [[health.bound]] rule:
/// which nodes must be bound is the project's to declare``, whose `rule`
/// is `fixtures/spec-a`'s kind `rule`, not taken from it
/// (docs/features/health-code.md "`[[health.bound]]`").
const EXEMPT_WORDS: [(&str, &str); 10] = [
    (STORE_CODE_MODULE, "module"),
    (STORE_CODE_MODULE, "note"),
    (ANALYZER_MODULE, "module"),
    (ANALYZER_MODULE, "note"),
    (STORE_READS_MODULE, "module"),
    (STORE_READS_MODULE, "note"),
    (SYMBOLS_MODULE, "module"),
    (SYMBOLS_MODULE, "note"),
    (UNMAPPED_MODULE, "module"),
    (HEALTH_CODE_MODULE, "rule"),
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

/// docs/features/bevy-roles.md RD-8 (WA-11, G1): every role word and
/// source the code crate's `frameworks` data lists, and every framework
/// name it knows, read from the data, never written here: the words are
/// the keys of `spec index`'s JSON `code.roles.words` over a scratch copy
/// of `fixtures/code-bevy` (every word of the listed frameworks, zeros
/// too), the sources the `code_roles.source` values stored there (the
/// fixture holds every source of "Role facts"), the names
/// `specengine_code::frameworks::KNOWN` (the spec's name, a list
/// of names).
fn framework_vocabulary() -> BTreeSet<String> {
    let scratch = Scratch::new("genre-roles");
    let home = scratch.home("h");
    let root = code_index_common::copy(&scratch, "code-bevy", "code-bevy");
    let run = spec(&home, &root, &["--json", "index"]);
    run.code(0);
    let json = run.json();
    let mut out: BTreeSet<String> = json["code"]["roles"]["words"]
        .as_object()
        .unwrap_or_else(|| panic!("code.roles.words: {json}"))
        .keys()
        .cloned()
        .collect();
    let sources: BTreeSet<String> = code_index_common::query(
        &code_index_common::db(&home, "code-bevy"),
        "SELECT DISTINCT source FROM code_roles",
    )
    .into_iter()
    .map(|row| row["source"].as_str().expect("a source").to_owned())
    .collect();
    for source in ["call", "derive", "fn", "impl"] {
        assert!(
            sources.contains(source),
            "code-bevy holds a row of every source: {sources:?}"
        );
    }
    out.extend(sources);
    out.extend(
        specengine_code::frameworks::KNOWN
            .iter()
            .map(|name| (*name).to_owned()),
    );
    out
}

/// A source without its own `#[cfg(test)] mod` (over the fixture, by
/// design).
fn scanned(file: &str) -> String {
    let whole = fs::read_to_string(repository_root().join(file))
        .unwrap_or_else(|error| panic!("{file}: {error}"));
    whole
        .find("\n#[cfg(test)]\nmod ")
        .map_or(whole.clone(), |at| whole[..at].to_owned())
}

/// TE-6: no string literal of the slice's new modules holds a fixture
/// prefix or alias (as a word or `PREFIX-…`), a fixture kind or project
/// slug, a fixture path (`fixtures/`, `tally`, `inkwell`), or a stack word
/// — but the narrow [`EXEMPT`] and [`EXEMPT_WORDS`] lists. A module's own
/// `#[cfg(test)] mod` (over the fixture, by design) ends what is scanned.
/// docs/features/bevy-roles.md RD-8: nor, in a core, store or CLI module,
/// a role word, a source or a framework name of the code crate's data
/// ([`framework_vocabulary`]). M: a `"component"` or `"call"` literal, or
/// a framework name, in a core, store or CLI module; `bevy` kept in
/// `code_toml.rs`.
#[test]
fn new_modules_hold_no_fixture_prefix_kind_path_or_stack_word() {
    let (prefixes, kinds, slugs) = fixture_vocabulary();
    assert!(prefixes.contains("PIECE") && prefixes.contains("AIM") && kinds.contains("pass-mark"));
    assert!(slugs.contains("code-a"));
    assert!(
        prefixes.contains("TENET") && kinds.contains("slab") && slugs.contains("inkwell"),
        "fixtures/code-index is scanned for"
    );
    assert!(
        prefixes.contains("CHORE") && kinds.contains("almanac") && slugs.contains("code-bevy"),
        "fixtures/code-bevy is scanned for"
    );
    let roles = framework_vocabulary();
    for word in [
        "bevy",
        "component",
        "entity_event",
        "system",
        "derive",
        "call",
    ] {
        assert!(roles.contains(word), "{word} read from the data: {roles:?}");
    }
    for file in NEW_MODULES {
        assert!(
            !file.starts_with("crates/specengine-code/src/frameworks")
                && !file.starts_with("crates/specengine-code/src/bevy"),
            "{file}: the code crate's framework modules stay out of the scan"
        );
    }
    let mut offenders = Vec::new();
    let mut scanned_literals = 0;
    for file in NEW_MODULES {
        let source = scanned(file);
        if file.ends_with("refs.rs") {
            assert!(
                source.contains("fn render_text("),
                "{file}: the code is scanned"
            );
        }
        let engine = !file.starts_with("crates/specengine-code/");
        for literal in literals(&source) {
            scanned_literals += 1;
            if EXEMPT.contains(&(file, literal.as_str())) {
                continue;
            }
            let lower = literal.to_lowercase();
            if lower.contains("fixtures/") || lower.contains("tally") || lower.contains("inkwell") {
                offenders.push(format!("{file}: {literal:?} (a fixture path)"));
            }
            for word in words(&literal) {
                let low = word.to_lowercase();
                if EXEMPT_WORDS.contains(&(file, low.as_str())) {
                    continue;
                }
                let prefix = prefixes.contains(&word)
                    || prefixes.iter().any(|p| word.starts_with(&format!("{p}-")));
                if prefix
                    || kinds.contains(&low)
                    || slugs.contains(&low)
                    || STACK_WORDS.contains(&low.as_str())
                {
                    offenders.push(format!("{file}: {literal:?} ({word})"));
                } else if engine && roles.contains(&low) {
                    offenders.push(format!(
                        "{file}: {literal:?} ({word}: the code crate's data)"
                    ));
                }
            }
        }
    }
    assert!(scanned_literals > 50, "{scanned_literals} literals scanned");
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// docs/features/layer-c-resolve.md R11: every [`EXEMPT`] entry is a whole
/// literal its file still holds, in the part the scan reads ([`scanned`]):
/// an exemption outliving its literal widens the scan's hole for nothing,
/// and the stack-word scan cannot see it (none of `languages`, `crates`,
/// `scip`, `module` is a word it flags). M: `EXEMPT` kept at 4 once `scip`
/// leaves `code_toml.rs`.
#[test]
fn every_exempt_literal_occurs_in_its_file() {
    let mut stale = Vec::new();
    for (file, literal) in EXEMPT {
        let held = literals(&scanned(file));
        if !held.iter().any(|text| text == literal) {
            stale.push(format!(
                "{file}: no literal {literal:?} (of {} scanned)",
                held.len()
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "EXEMPT names literals its files no longer hold:\n{}",
        stale.join("\n")
    );
}

/// Every `.rs` file under `dir` (relative to the repository), sorted.
fn sources_under(dir: &str) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).expect("a source directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(
                    path.strip_prefix(root)
                        .expect("under the repository")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let root = repository_root();
    let mut out = Vec::new();
    walk(&root, &root.join(dir), &mut out);
    out.sort();
    out
}

/// docs/features/symbol-index.md "Rules and edge cases", Genre: Rust
/// syntax stays in `specengine-code`. The store, core and the slice's CLI
/// module hold no `::` literal (a qpath is compared, never split: the
/// suffixes come from the structured segments, WA-8) and no Cargo literal
/// (`Cargo.toml`, a word `cargo`). M: the store splits a qpath at `::`.
/// docs/features/bevy-roles.md RD-5 (L3): the CLI role module too. M: the
/// package cut from a qpath.
#[test]
fn store_core_and_new_modules_hold_no_path_split_or_cargo_literal() {
    let mut files = sources_under("crates/specengine-store/src");
    files.extend(sources_under("crates/specengine-core/src"));
    files.push(ANALYZER_MODULE.to_owned());
    files.push(SYMBOLS_MODULE.to_owned());
    files.push(UNMAPPED_MODULE.to_owned());
    files.push(ROLES_MODULE.to_owned());
    for module in [STORE_CODE_MODULE, STORE_READS_MODULE, STORE_ROLES_MODULE] {
        assert!(
            files.iter().any(|file| file == module),
            "the store's module is {module}: {files:?}"
        );
    }
    let mut offenders = Vec::new();
    let mut scanned_literals = 0;
    for file in &files {
        let source = scanned(file);
        for literal in literals(&source) {
            scanned_literals += 1;
            if literal == "::" || literal.starts_with("::") || literal.ends_with("::") {
                offenders.push(format!("{file}: {literal:?} (a path separator)"));
            }
            if literal.contains("Cargo.toml")
                || words(&literal)
                    .iter()
                    .any(|word| word.eq_ignore_ascii_case("cargo"))
            {
                offenders.push(format!("{file}: {literal:?} (a Cargo literal)"));
            }
        }
    }
    assert!(
        scanned_literals > 200,
        "{scanned_literals} literals scanned"
    );
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

/// Words of earlier modules that are no path classification: `[verify]`'s
/// refused key `tests` (core `verify.rs`), `spec.lock`'s header naming
/// `spec verify --tests` (core `lock.rs`), the `[code]` example
/// `roots = ["src"]` of a message (core `code_toml.rs`), the `links`
/// table's column `src`, a link's source node (store `schema.rs`,
/// `write.rs`).
const PATH_WORD_EXEMPT: [(&str, &str); 5] = [
    ("crates/specengine-core/src/verify.rs", "tests"),
    ("crates/specengine-core/src/lock.rs", "tests"),
    ("crates/specengine-core/src/code_toml.rs", "src"),
    ("crates/specengine-store/src/schema.rs", "src"),
    ("crates/specengine-store/src/write.rs", "src"),
];

/// Whole literals of earlier modules that are no path classification:
/// the event type `verify.tests` (store `queue.rs`,
/// `docs/canon/spec-verify-tests.md` "Verdicts and event"). Only that
/// literal is exempt; the word `tests` stays scanned in the rest of the
/// file.
const PATH_LITERAL_EXEMPT: [(&str, &str); 1] =
    [("crates/specengine-store/src/queue.rs", "verify.tests")];

/// docs/features/symbol-reads.md "spec unmapped" ("No path classified")
/// and its "Rules and edge cases", Genre: no string literal of the store,
/// core, or the `symbols` and `unmapped` modules holds the word `tests` or
/// `src` (a directory read as tests or sources), but the narrow
/// [`PATH_WORD_EXEMPT`] and [`PATH_LITERAL_EXEMPT`]. M: `unmapped` sorts a
/// `tests` directory apart.
#[test]
fn store_core_and_new_modules_hold_no_tests_or_src_literal() {
    let mut files = sources_under("crates/specengine-store/src");
    files.extend(sources_under("crates/specengine-core/src"));
    files.push(SYMBOLS_MODULE.to_owned());
    files.push(UNMAPPED_MODULE.to_owned());
    assert!(
        files.iter().any(|file| file == STORE_READS_MODULE),
        "the store's lookup module is {STORE_READS_MODULE}: {files:?}"
    );
    let mut offenders = Vec::new();
    let mut scanned_literals = 0;
    for file in &files {
        let source = scanned(file);
        for literal in literals(&source) {
            scanned_literals += 1;
            if PATH_LITERAL_EXEMPT.contains(&(file.as_str(), literal.as_str())) {
                continue;
            }
            for word in words(&literal) {
                let low = word.to_lowercase();
                if (low == "tests" || low == "src")
                    && !PATH_WORD_EXEMPT.contains(&(file.as_str(), low.as_str()))
                {
                    offenders.push(format!("{file}: {literal:?} ({word})"));
                }
            }
        }
    }
    assert!(
        scanned_literals > 200,
        "{scanned_literals} literals scanned"
    );
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
/// level list's. docs/features/lock-levels.md RD-4 ("Rules": the pin,
/// Q16, S6): `spec lock accept` (`docs/canon/spec-lock-accept.md` "Steps"
/// 5) hashes the items and modules it locks through `item_levels` and
/// `module_levels` alone: `lock.rs` calls both, never `hash_bound_items`,
/// `hash_item` or `hash_node`, names no `HashState`, and like
/// `lock_regen.rs` reads no item's `.state` (a judgement's, a level list's
/// or a printed row's only). docs/features/symbol-index.md RD-13: the code
/// index's analyzer joins, hashing through `item_levels` alone (the pin
/// names per file the entries it must call). `verify_rows.rs` joins in
/// 5b-1b (Q26), `lock_rebase.rs` in 5b-2.
#[test]
fn nothing_outside_the_binder_reads_an_items_state() {
    const HASHING: [&str; 5] = [
        "hash_item(",
        "hash_node(",
        "hash_bound_items(",
        "item_levels(",
        "module_levels(",
    ];
    let pinned: [(&str, &[&str]); 6] = [
        ("crates/specengine-cli/src/code_walk.rs", &[]),
        ("crates/specengine-cli/src/refs.rs", &[]),
        ("crates/specengine-code/src/naming.rs", &[]),
        (
            "crates/specengine-cli/src/lock.rs",
            &["item_levels(", "module_levels("],
        ),
        ("crates/specengine-cli/src/lock_regen.rs", &[]),
        (ANALYZER_MODULE, &["item_levels("]),
    ];
    for (file, entries) in pinned {
        let source = scanned(file);
        assert!(!source.contains("HashState"), "{file} names HashState");
        for entry in entries {
            assert!(
                source.contains(entry),
                "{file} hashes its units through `{entry}`"
            );
        }
        if !entries.is_empty() {
            for other in HASHING {
                if entries.contains(&other) {
                    continue;
                }
                // The analyzer hashes no file module: `module_levels(` is
                // `lock.rs`' alone, absent from it as an entry, not banned.
                if other == "module_levels(" {
                    continue;
                }
                assert!(
                    !source.contains(other),
                    "{file} hashes through `{other}`, not {entries:?}"
                );
            }
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

// ----------------------------------------------- RD-13 (symbol-index)

/// docs/features/symbol-index.md RD-13. M: a second parse. The analyzer
/// parses each re-derived `.rs` file once and runs every per-file step on
/// that one tree: one `.parse(` call, `bind_tree`, `name_file`,
/// `item_levels`, `item_signatures` given the tree, no entry that parses
/// on its own (`bind_file`, `analyze_file`); `comment_anchors` once per
/// `.ron` file. `docs/canon/code-roles.md` "Role facts": the framework
/// detector runs on that one tree too, never through `detect_file`, which
/// parses again.
#[test]
fn one_parse_per_rederived_file() {
    let source = scanned(ANALYZER_MODULE);
    assert_eq!(
        source.matches(".parse(").count(),
        1,
        "{ANALYZER_MODULE}: one parse"
    );
    for entry in [
        "bind_tree(",
        "name_file(",
        "item_levels(",
        "item_signatures(",
        "comment_anchors(",
    ] {
        assert_eq!(
            source.matches(entry).count(),
            1,
            "{ANALYZER_MODULE}: `{entry}` called once"
        );
    }
    for parses in ["bind_file(", "analyze_file(", "name_files(", "detect_file("] {
        assert!(
            !source.contains(parses),
            "{ANALYZER_MODULE}: `{parses}` parses or names the tree again"
        );
    }
}

/// The paths of `code_files` after `spec index` in `root` (a fresh `HOME`).
fn indexed_paths(scratch: &Scratch, root: &Path, label: &str) -> Vec<String> {
    let home = scratch.home(label);
    let run = spec(&home, root, &["index"]);
    run.code(0);
    code_index_common::code_paths(&code_index_common::db(&home, "code-a"))
}

/// docs/features/symbol-index.md RD-13. M: a second listing. Over this
/// file's walk cases — code-a as it is (a `.`-name, an excluded directory,
/// a root that is a file), linked files, directories and roots, missing
/// roots, a root of another file type and content that is not UTF-8 —
/// `spec index` stores exactly the files the walk lists (`expected.json`
/// `walked`, plus the non-UTF-8 file it lists and skips), so `refs` counts
/// what it stored.
#[test]
fn index_and_walk_list_alike() {
    let scratch = Scratch::new("listing");
    let expected: Value = serde_json::from_str(&read_text(&fixture("code-a"), "expected.json"))
        .expect("expected.json");
    let walked: Vec<String> = expected["walked"]
        .as_array()
        .expect("walked")
        .iter()
        .map(|path| path.as_str().expect("a path").to_owned())
        .collect();

    let plain = scratch.copy("code-a", "plain");
    assert_eq!(indexed_paths(&scratch, &plain, "plain"), walked, "code-a");

    let linked = scratch.copy("code-a", "linked");
    write(
        &linked,
        "outside/extra.rs",
        "// @implements GOAL-01@1\npub fn outside() {}\n",
    );
    std::os::unix::fs::symlink(
        "../../../outside/extra.rs",
        linked.join("crates/tally/src/linked.rs"),
    )
    .expect("file link");
    std::os::unix::fs::symlink("../outside", linked.join("data/linked")).expect("dir link");
    std::os::unix::fs::symlink("outside", linked.join("linked-root")).expect("root link");
    write(
        &linked,
        "specengine.toml",
        with_code(
            "[code]\nroots = [\"crates\", \"data\", \"tools/loose.rs\", \"linked-root\"]\nexclude = [\"crates/*/tests/fixtures/**\"]\n",
        ),
    );
    assert_eq!(
        indexed_paths(&scratch, &linked, "linked"),
        walked,
        "no link is followed"
    );

    let notes = scratch.copy("code-a", "notes");
    write(
        &notes,
        "specengine.toml",
        with_code(
            "[code]\nroots = [\"absent\", \"crates\", \"crates/tally/Cargo.toml\", \"data\", \"tools/loose.rs\"]\nexclude = [\"crates/*/tests/fixtures/**\"]\n",
        ),
    );
    write(&notes, "crates/tally/src/latin.rs", b"// caf\xe9 GOAL-01\n");
    let mut with_latin = walked.clone();
    with_latin.push("crates/tally/src/latin.rs".to_owned());
    with_latin.sort();
    let home = scratch.home("notes");
    spec(&home, &notes, &["index"]).code(0);
    let db = code_index_common::db(&home, "code-a");
    assert_eq!(code_index_common::code_paths(&db), with_latin, "notes");
    let latin = code_index_common::query(
        &db,
        "SELECT utf8 FROM code_files WHERE path = 'crates/tally/src/latin.rs'",
    );
    assert_eq!(latin[0]["utf8"], 0, "listed, not UTF-8");
    let run = spec(&home, &notes, &["refs", "PREM-001"]);
    run.code(0);
    assert!(
        run.stdout
            .ends_with("refs: markers 3, mentions 0; code files 8 (.rs 7, .ron 1), skipped 1\n"),
        "{}",
        run.show()
    );
}
