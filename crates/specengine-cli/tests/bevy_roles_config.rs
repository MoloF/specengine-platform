//! docs/features/bevy-roles.md RD-4 (`docs/canon/code-roles.md` "`[code] frameworks`"; WA-4, Q3,
//! SR5): the core checks the key's shape (a list of names, each
//! `[a-z][a-z0-9_-]{0,31}`), `refresh_code` checks the values against the
//! code crate's `frameworks::KNOWN` before any row is touched; both are
//! exit 2 at the line, `<config>:<line>: …`, for every code-index reader
//! (`index` after its md index, `refs`, `symbols`, `unmapped`), while
//! `check` and `show` (name-check only) and `verify` (never reads the key)
//! answer as before. The core's refusal list and the stack-word scan are
//! `code_refs_walk.rs`' (`code_table_unknown_key_is_refused`,
//! `new_modules_hold_no_fixture_prefix_kind_path_or_stack_word`).
//!
//! Over scratch copies of `fixtures/code-bevy`; `verify` in a scratch git
//! repository (`lock_common`). The known names are
//! `specengine_code::frameworks::KNOWN` (the spec's name; `&[&str]`).
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod bevy_roles_common;
mod code_index_common;
mod common;
mod lock_common;

use std::path::Path;

use bevy_roles_common::{CODE_ON, SLUG, cause, copy, index, roles_line, set_code};
use code_index_common::{
    code_line, code_lines, counts_of, dump, fresh_dump, fresh_dump_with_exit, md_lines,
};
use common::{Run, Scratch, read_text, replace, spec, write};
use lock_common::LockRepo;

/// The 1-based line of the first line of `text` that, trimmed, starts
/// with `needle`.
fn line_of_text(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.trim_start().starts_with(needle))
        .unwrap_or_else(|| panic!("no line starts with {needle:?}"))
        + 1
}

/// The refusal of an unknown framework ("Values"): the names the code
/// crate knows, each in backticks, `, `-joined.
fn unknown(name: &str) -> String {
    let known: Vec<String> = specengine_code::frameworks::KNOWN
        .iter()
        .map(|known| format!("`{known}`"))
        .collect();
    format!(
        "`[code] frameworks`: `{name}` is not known: known {}",
        known.join(", ")
    )
}

/// The refusal of a bad name ("`[code] frameworks`", Core).
fn not_a_name(value: &str) -> String {
    format!(
        "`[code] frameworks`: `{value}` is not a name: a lowercase letter, then at most 31 of a-z, 0-9, `_`, `-`"
    )
}

/// One stderr line, `want` at `line` of `specengine.toml`.
fn assert_refused(run: &Run, line: usize, want: &str, context: &str) {
    run.code(2);
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "{context}: one stderr line\n{}", run.show());
    assert_eq!(
        cause(lines[0]),
        format!("specengine.toml:{line}: {want}"),
        "{context}"
    );
}

/// The reads that need the code index: each exits 2 with nothing on
/// stdout and the one line; `spec index` prints its md line first, no
/// code or roles line, and JSON `code` `null`.
fn assert_every_reader_refuses(home: &Path, root: &Path, line: usize, want: &str) {
    for args in [
        &["refs", "ALM-GROWTH"][..],
        &["--json", "refs", "ALM-GROWTH"],
        &["symbols", "tick"],
        &["--json", "symbols", "tick"],
        &["unmapped"],
        &["--json", "unmapped"],
    ] {
        let run = spec(home, root, args);
        assert_refused(&run, line, want, &format!("{args:?}"));
        assert_eq!(run.stdout, "", "{args:?}: stdout stays empty");
    }
    let run = spec(home, root, &["index"]);
    assert_refused(&run, line, want, "index");
    assert!(
        run.stdout.starts_with("indexed code-bevy: "),
        "the md index is printed: {}",
        run.show()
    );
    assert!(code_line(&run).is_none(), "{}", run.show());
    assert!(roles_line(&run).is_none(), "{}", run.show());
    let run = spec(home, root, &["--json", "index"]);
    assert_refused(&run, line, want, "--json index");
    assert!(run.json()["code"].is_null(), "{}", run.show());
}

/// `check` and `show` answer as with the fixture's own `[code]`.
fn assert_name_check_only(root: &Path, context: &str) {
    let config = read_text(root, "specengine.toml");
    let scratch = Scratch::new("br-name-check");
    let home = scratch.home("h");
    let mut bad = Vec::new();
    for args in [&["check"][..], &["--json", "check"], &["show", "ALM-FARM"]] {
        bad.push(spec(&home, root, args));
    }
    set_code(root, CODE_ON);
    for (args, bad) in [&["check"][..], &["--json", "check"], &["show", "ALM-FARM"]]
        .into_iter()
        .zip(bad)
    {
        let good = spec(&home, root, args);
        assert_eq!(
            (bad.code, &bad.stdout, &bad.stderr),
            (good.code, &good.stdout, &good.stderr),
            "{context}: {args:?}"
        );
    }
    write(root, "specengine.toml", &config);
}

/// RD-4 ("Values", SR5). M: an unknown framework accepted. A name the code
/// crate does not know, in a list over several lines: every code-index
/// reader exits 2 at that name's own line; `check` and `show` answer as
/// with `["bevy"]`.
#[test]
fn unknown_framework_is_exit_2_naming_the_line() {
    let scratch = Scratch::new("br-unknown");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy");
    set_code(
        &root,
        "[code]\nroots = [\"crates\", \"assets\"]\nframeworks = [\n    \"bevy\",\n    \"loom\",\n]\n",
    );
    let config = read_text(&root, "specengine.toml");
    let line = line_of_text(&config, "\"loom\"");
    assert!(line > line_of_text(&config, "frameworks"), "{config}");
    assert_every_reader_refuses(&home, &root, line, &unknown("loom"));
    assert_name_check_only(&root, "unknown");
}

/// RD-4 (Core, the forms of `exclude`'s). M: a bad shape accepted. A
/// string, a boolean, a list holding a number or a list, and names that
/// are none (a capital, a leading digit, a space, empty, 33 bytes, a
/// Cyrillic look-alike letter): exit 2 at the key's line for every reader;
/// 32 bytes of name is a shape the core takes (then unknown).
#[test]
fn frameworks_shape_is_exit_2_naming_the_line() {
    let long = format!("a{}", "b".repeat(32));
    let longest = format!("a{}", "b".repeat(31));
    let cases: [(String, String); 12] = [
        (
            "\"bevy\"".to_owned(),
            "`[code] frameworks` is a string, not a list of names".to_owned(),
        ),
        (
            "true".to_owned(),
            "`[code] frameworks` is a boolean, not a list of names".to_owned(),
        ),
        (
            "[1]".to_owned(),
            "`[code] frameworks` holds a integer: it lists names".to_owned(),
        ),
        (
            "[[\"bevy\"]]".to_owned(),
            "`[code] frameworks` holds a array: it lists names".to_owned(),
        ),
        ("[\"Bevy\"]".to_owned(), not_a_name("Bevy")),
        ("[\"1bevy\"]".to_owned(), not_a_name("1bevy")),
        ("[\"bevy app\"]".to_owned(), not_a_name("bevy app")),
        ("[\"\"]".to_owned(), not_a_name("")),
        (format!("[\"{long}\"]"), not_a_name(&long)),
        ("[\"b\u{0435}vy\"]".to_owned(), not_a_name("b\u{0435}vy")),
        (format!("[\"{longest}\"]"), unknown(&longest)),
        ("[\"x-y_z9\"]".to_owned(), unknown("x-y_z9")),
    ];
    for (value, want) in cases {
        let scratch = Scratch::new("br-shape");
        let home = scratch.home("h");
        let root = copy(&scratch, "code-bevy");
        set_code(
            &root,
            &format!("[code]\nroots = [\"crates\", \"assets\"]\nframeworks = {value}\n"),
        );
        let config = read_text(&root, "specengine.toml");
        let line = line_of_text(&config, "frameworks");
        assert_every_reader_refuses(&home, &root, line, &want);
        assert_name_check_only(&root, &value);
    }
}

/// RD-4 ("Values": checked before any row is touched). M: the value
/// checked after rows are written. Indexed with `["bevy"]`; then a file
/// edited and `"loom"` listed: `spec index` exits 2, no code row changes
/// (`code_roles` included), the md rows are a fresh index's; the name
/// dropped, the edit is indexed and the index equals a fresh one.
#[test]
fn unknown_framework_leaves_code_rows_untouched() {
    let scratch = Scratch::new("br-untouched");
    let home = scratch.home("h");
    let root = copy(&scratch, "code-bevy");
    index(&home, &root);
    let before = code_lines(&dump(&home, &root, SLUG));
    assert!(before.contains("code_roles\t"), "the rows are dumped");
    replace(
        &root,
        "crates/farm/src/herd.rs",
        "#[derive(Component)]\npub struct Barn;",
        "#[derive(Resource)]\npub struct Barn;",
    );
    set_code(
        &root,
        "[code]\nroots      = [\"crates\", \"assets\"]\nframeworks = [\"bevy\", \"loom\"]\n",
    );
    for args in [&["index"][..], &["refs", "ALM-GROWTH"], &["unmapped"]] {
        spec(&home, &root, args).code(2);
        let have = dump(&home, &root, SLUG);
        assert_eq!(code_lines(&have), before, "{args:?}: no code row changes");
    }
    let have = dump(&home, &root, SLUG);
    let fresh = fresh_dump_with_exit(&scratch, &root, SLUG, 2);
    assert_eq!(md_lines(&have), md_lines(&fresh), "the md rows are fresh");

    set_code(&root, CODE_ON);
    let run = index(&home, &root);
    assert_eq!(counts_of(&run)["parsed"], 1, "herd.rs: {}", run.show());
    assert_eq!(
        dump(&home, &root, SLUG),
        fresh_dump(&scratch, &root, SLUG),
        "the edit indexed"
    );
    assert_ne!(code_lines(&dump(&home, &root, SLUG)), before);
}

/// RD-4 ("Values": `verify` and `lock` never read the key). M: `verify`
/// refusing an unknown framework. In a scratch git repository of
/// code-bevy, `spec verify` (text and JSON) with `"loom"` listed answers
/// as with `["bevy"]`, both read through `--config` outside the root.
#[test]
fn verify_never_reads_the_frameworks_values() {
    let repo = LockRepo::of("br-verify-key", "code-bevy", "repo", &[]);
    let config = read_text(&repo.root, "specengine.toml");
    let at = config.find("\n[code]\n").expect("a [code] table") + 1;
    let configs = repo.scratch.dir("configs");
    write(&configs, "good.toml", &config);
    write(
        &configs,
        "loom.toml",
        format!(
            "{}[code]\nroots = [\"crates\", \"assets\"]\nframeworks = [\"loom\"]\n",
            &config[..at]
        ),
    );
    let root = repo.root.to_str().expect("a UTF-8 root").to_owned();
    let mut outputs = Vec::new();
    for name in ["good.toml", "loom.toml"] {
        let file = configs.join(name);
        let file = file.to_str().expect("a UTF-8 path").to_owned();
        let mut runs = Vec::new();
        for json in [false, true] {
            let mut args = vec!["--root", root.as_str(), "--config", file.as_str()];
            if json {
                args.push("--json");
            }
            args.push("verify");
            let run = repo.spec_piped(&repo.root, &args);
            runs.push((run.code, run.stdout, run.stderr));
        }
        outputs.push(runs);
    }
    assert_eq!(outputs[0], outputs[1], "verify reads no framework");
}
