//! docs/features/spec-verify.md TE-5 and RD-4: the cascade at verify
//! level (AC-2) over `fixtures/verify-ecs`, its nodes accepted: an equal
//! blob is never parsed or hashed; a changed blob of a `module` or a RON
//! value is a change by itself; a changed blob of an item is re-hashed
//! whole, so rustfmt (default and `max_width = 60`), a comment or a
//! neighbour's edit leave it `ok` and a body edit makes it `code_ahead`;
//! the counters pinned. Under `core.autocrlf` the same verdicts, the
//! literal limit (`docs/canon/spec-verify.md` "Known limits": a
//! multi-line literal holding CRLF reads `code_ahead`) pinned.

mod common;
mod lock_common;
mod verify_common;

use std::fs;
use std::io::Write as _;
use std::process::{Command, Stdio};

use lock_common::LockRepo;
use serde_json::{Value, json};
use verify_common::{ECS_LIB, MOVEMENT, STAMINA, Verify, ecs, verify};

/// Each row's `(state, category, compared)` by its node and symbol.
fn states(run: &Verify) -> Vec<(String, String, Value, Value, Value)> {
    run.rows()
        .into_iter()
        .map(|row| {
            (
                row["node"].as_str().unwrap_or_default().to_owned(),
                row["symbol"].as_str().unwrap_or_default().to_owned(),
                row["state"].clone(),
                row["category"].clone(),
                row["compared"].clone(),
            )
        })
        .collect()
}

/// The six rows of verify-ecs, each `(state, category, compared)`.
fn ecs_rows(
    build: (&str, Option<&str>, &str),
    module: (&str, Option<&str>, &str),
    drain: (&str, Option<&str>, &str),
    regen: (&str, Option<&str>, &str),
    ron: (&str, Option<&str>, &str),
    base_regen: (&str, Option<&str>, &str),
) -> Vec<(String, String, Value, Value, Value)> {
    [
        ("PERK-DRAIN", "crates/game::stamina::drain_system", drain),
        ("PERK-REGEN", "crates/game::stamina::regen_system", regen),
        ("PERK-STAMINA", "crates/game::StaminaPlugin::build", build),
        ("PERK-STAMINA", "crates/game", module),
        ("TUNE-01", "root.regen_per_second", ron),
        ("TUNE-01", "crates/game::stamina::BASE_REGEN", base_regen),
    ]
    .into_iter()
    .map(|(node, symbol, (state, category, compared))| {
        (
            node.to_owned(),
            symbol.to_owned(),
            json!(state),
            json!(category),
            json!(compared),
        )
    })
    .collect()
}

const OK_BLOB: (&str, Option<&str>, &str) = ("ok", None, "blob");
const OK_AST: (&str, Option<&str>, &str) = ("ok", None, "ast");
const CHANGED_AST: (&str, Option<&str>, &str) = ("code_ahead", Some("code_changed"), "ast");
const CHANGED_BLOB: (&str, Option<&str>, &str) = ("code_ahead", Some("code_changed"), "blob");

/// `rustfmt --edition 2024 [--config max_width=60]` over `text`, through
/// stdin, outside any project's `rustfmt.toml`.
fn rustfmt(repo: &LockRepo, text: &str, narrow: bool) -> String {
    let mut command = Command::new("rustfmt");
    command
        .args(["--edition", "2024", "--emit", "stdout", "--quiet"])
        .current_dir(repo.scratch.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if narrow {
        command.args(["--config", "max_width=60"]);
    }
    let mut child = command
        .spawn()
        .expect("rustfmt is installed (rustup component)");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(text.as_bytes())
        .expect("feed rustfmt");
    let output = child.wait_with_output().expect("rustfmt ran");
    assert!(
        output.status.success(),
        "rustfmt: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

/// RD-4: over the accepted fixture every entry is compared by its blob
/// alone, nothing hashed: `cascade.by_blob` 6, `by_ast` 0, `clean`. M:
/// every item hashed.
#[test]
fn blob_equal_entries_are_not_hashed() {
    let repo = ecs("verify-by-blob");
    let run = verify(&repo, &[]);
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    assert_eq!(run.json["cascade"], json!({"by_blob": 6, "by_ast": 0}));
    assert_eq!(
        states(&run),
        ecs_rows(OK_BLOB, OK_BLOB, OK_BLOB, OK_BLOB, OK_BLOB, OK_BLOB)
    );
    assert_eq!(
        run.text.stdout,
        "verify: clean [observe]: 0 fix, 0 check, 0 cannot, 6 ok; 6 by blob, 0 by ast; 0 skipped\n"
    );
    // A change in one file hashes only that file's items.
    repo.replace(
        STAMINA,
        "//! Stamina: regen at rest",
        "//! Stamina: it regens at rest",
    );
    let one = verify(&repo, &[]);
    assert_eq!(one.json["cascade"], json!({"by_blob": 3, "by_ast": 3}));
    assert_eq!(
        states(&one),
        ecs_rows(OK_BLOB, OK_BLOB, OK_AST, OK_AST, OK_BLOB, OK_AST)
    );
}

/// RD-4: a RON value and a module are compared by their file's blob alone:
/// an edit anywhere in the file (the RON's unbound field, the module's
/// unbound struct) is `code_ahead`, by `blob`. M: a RON blob change `ok`.
#[test]
fn blob_only_binding_change_is_code_ahead() {
    let repo = ecs("verify-blob-only");
    repo.replace(MOVEMENT, "sprint_cost: 4.0,", "sprint_cost: 5.0,");
    repo.replace(
        ECS_LIB,
        "pub systems: Vec<&'static str>,",
        "pub systems: Vec<&'static str>,\n    pub paused: bool,",
    );
    let run = verify(&repo, &[]);
    assert_eq!(
        states(&run),
        ecs_rows(
            OK_AST,
            CHANGED_BLOB,
            OK_BLOB,
            OK_BLOB,
            CHANGED_BLOB,
            OK_BLOB
        )
    );
    assert_eq!(run.json["cascade"], json!({"by_blob": 5, "by_ast": 1}));
    assert_eq!((run.verdict(), run.code()), ("observed", 0));
    run.has_line("check | code_ahead code_changed | TUNE-01 | configures | assets/data/movement.ron | ron root.regen_per_second");
    run.has_line("check | code_ahead code_changed | PERK-STAMINA | implements | crates/game/src/lib.rs | module crates/game");
}

/// RD-4, TE-5: comment edits (inside the body, the doc comment above, a
/// new comment line) leave a locked item `ok` by its hash. M: comments
/// hashed.
#[test]
fn comment_edit_is_ok() {
    let repo = ecs("verify-comment");
    repo.replace(
        STAMINA,
        "// Rest restores a fixed amount per second.",
        "// Rest restores a fixed amount every second.",
    );
    repo.replace(
        STAMINA,
        "    stamina.current = (stamina.current - cost",
        "    // Never below zero.\n    stamina.current = (stamina.current - cost",
    );
    repo.replace(
        STAMINA,
        "/// A runner's stamina.",
        "/// A runner's stamina, in points.",
    );
    let run = verify(&repo, &[]);
    assert_eq!(
        states(&run),
        ecs_rows(OK_BLOB, OK_BLOB, OK_AST, OK_AST, OK_BLOB, OK_AST)
    );
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    assert_eq!(run.json["cascade"], json!({"by_blob": 3, "by_ast": 3}));
}

/// TE-5 (AC-2): rustfmt under its defaults leaves the fixture's bytes, so
/// nothing is hashed; under `max_width = 60` it reflows the bound items:
/// each `ok` by its hash; a neighbour's edit `ok`; a body edit
/// `code_ahead` while the reflowed neighbours stay `ok`. Counters pinned.
#[test]
fn rustfmt_and_neighbour_edits_are_ok_a_body_edit_is_not() {
    let repo = ecs("verify-rustfmt");
    let original = repo.read(STAMINA);
    assert_eq!(
        rustfmt(&repo, &original, false),
        original,
        "the fixture is rustfmt-clean"
    );
    let narrow = rustfmt(&repo, &original, true);
    assert_ne!(narrow, original, "max_width = 60 reflows the items");
    repo.write(STAMINA, &narrow);
    let run = verify(&repo, &[]);
    assert_eq!(
        states(&run),
        ecs_rows(OK_BLOB, OK_BLOB, OK_AST, OK_AST, OK_BLOB, OK_AST)
    );
    assert_eq!(
        (run.verdict(), run.code()),
        ("clean", 0),
        "{}",
        run.text.show()
    );
    assert_eq!(run.json["cascade"], json!({"by_blob": 3, "by_ast": 3}));

    // A neighbour (the unbound `REST_DELAY`) edited.
    repo.replace(
        STAMINA,
        "pub const REST_DELAY: f32 = 0.5;",
        "pub const REST_DELAY: f32 = 0.75;",
    );
    let neighbour = verify(&repo, &[]);
    assert_eq!(states(&neighbour), states(&run));
    assert_eq!(neighbour.verdict(), "clean");

    // A body edit of one bound item.
    let body = repo
        .read(STAMINA)
        .replacen("rate * dt", "rate * dt * 0.5", 1);
    repo.write(STAMINA, body);
    let edited = verify(&repo, &[]);
    assert_eq!(
        states(&edited),
        ecs_rows(OK_BLOB, OK_BLOB, OK_AST, CHANGED_AST, OK_BLOB, OK_AST)
    );
    assert_eq!((edited.verdict(), edited.code()), ("observed", 0));
    assert_eq!(edited.json["cascade"], json!({"by_blob": 3, "by_ast": 3}));
    assert_eq!(
        edited.summary(),
        "verify: observed [observe]: 0 fix, 1 check, 0 cannot, 5 ok; 3 by blob, 3 by ast; 0 skipped"
    );
}

/// `core.autocrlf=true` set and the given paths checked out again: CRLF in
/// the worktree, LF committed, `git status` clean.
fn autocrlf_checkout(repo: &LockRepo, paths: &[&str]) {
    repo.git(&["config", "core.autocrlf", "true"]);
    for path in paths {
        fs::remove_file(repo.root.join(path)).expect("remove");
        repo.git(&["checkout", "--", path]);
        assert!(
            fs::read(repo.root.join(path))
                .expect("checked out")
                .windows(2)
                .any(|pair| pair == b"\r\n"),
            "CRLF in the worktree: {path}"
        );
    }
    assert!(repo.porcelain().is_empty(), "clean under autocrlf");
}

/// TE-5: under `core.autocrlf` every locked item hashes as under LF (`ok`
/// by its hash); a comment edit `ok`, a body edit `code_ahead`. The item
/// limit `docs/canon/spec-verify.md` "Known limits" named before format 2,
/// a multi-line string literal holding the CRLF, is gone: format 2 judges
/// `sig` and `body`, which apply the CR rule (docs/features/lock-levels.md
/// "Description and interactions": "a CRLF checkout's CR … now reads
/// `ok`"), so the literal reads `ok` by its hash too.
#[test]
fn autocrlf_items_verify_as_lf_but_the_literal() {
    let repo = ecs("verify-autocrlf-items");
    repo.append(
        STAMINA,
        "\n// @implements PERK-REGEN@1\npub const REGEN_HELP: &str = \"Rest\nto regen\";\n",
    );
    repo.commit_all("a multi-line literal");
    repo.accept_ok("PERK-REGEN");
    autocrlf_checkout(&repo, &[STAMINA]);
    let run = verify(&repo, &[]);
    let regen = |run: &Verify, symbol: &str| {
        let row = run.row("PERK-REGEN", "implements", STAMINA, Some(symbol));
        (
            row["state"].clone(),
            row["category"].clone(),
            row["compared"].clone(),
        )
    };
    for (node, symbol) in [
        ("PERK-REGEN", "crates/game::stamina::regen_system"),
        ("PERK-DRAIN", "crates/game::stamina::drain_system"),
        ("TUNE-01", "crates/game::stamina::BASE_REGEN"),
    ] {
        let row = run.rows_where(|row| row["node"] == node && row["symbol"] == symbol);
        assert_eq!(row.len(), 1, "{node} {symbol}");
        assert_eq!(
            (&row[0]["state"], &row[0]["compared"]),
            (&json!("ok"), &json!("ast")),
            "{node} {symbol}:\n{}",
            run.text.show()
        );
    }
    assert_eq!(
        regen(&run, "crates/game::stamina::REGEN_HELP"),
        (json!("ok"), json!(null), json!("ast")),
        "the literal limit lifted by the CR rule"
    );
    repo.replace(
        STAMINA,
        "// Rest restores a fixed amount per second.",
        "// Rest restores a fixed amount every second.",
    );
    let comment = verify(&repo, &[]);
    assert_eq!(
        regen(&comment, "crates/game::stamina::regen_system"),
        (json!("ok"), Value::Null, json!("ast"))
    );
    let body = repo
        .read(STAMINA)
        .replacen("rate * dt", "rate * dt * 0.5", 1);
    repo.write(STAMINA, body);
    let edited = verify(&repo, &[]);
    assert_eq!(
        regen(&edited, "crates/game::stamina::regen_system"),
        (json!("code_ahead"), json!("code_changed"), json!("ast"))
    );
}

/// TE-5, Q9 (decided 2026-10-13, the owner's pre-approval, review open):
/// under `core.autocrlf`, every file checked out with CRLF and `git
/// status` clean, the items give the LF verdicts (`ok`, by their hash);
/// the module and the RON value, compared by their blob alone (`hash-object
/// --no-filters` hashes the CRLF bytes), read `code_ahead code_changed` by
/// `blob`: a known limit until slice 5b. Nothing else differs. M: a CRLF
/// item judged changed; a module or RON row judged without its blob.
#[test]
fn autocrlf_items_same_module_and_ron_code_ahead() {
    let repo = ecs("verify-autocrlf");
    let lf = verify(&repo, &[]);
    assert_eq!(
        states(&lf),
        ecs_rows(OK_BLOB, OK_BLOB, OK_BLOB, OK_BLOB, OK_BLOB, OK_BLOB)
    );
    autocrlf_checkout(&repo, &[STAMINA, ECS_LIB, MOVEMENT]);
    let crlf = verify(&repo, &[]);
    assert_eq!(
        states(&crlf),
        ecs_rows(OK_AST, CHANGED_BLOB, OK_AST, OK_AST, CHANGED_BLOB, OK_AST),
        "{}",
        crlf.text.show()
    );
    assert_eq!(crlf.json["cascade"], json!({"by_blob": 2, "by_ast": 4}));
    assert_eq!((crlf.verdict(), crlf.code()), ("observed", 0));
    assert_eq!(
        crlf.summary(),
        "verify: observed [observe]: 0 fix, 2 check, 0 cannot, 4 ok; 2 by blob, 4 by ast; 0 skipped"
    );
}
