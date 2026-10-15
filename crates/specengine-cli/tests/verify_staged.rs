//! docs/features/rev-rule.md RD-5 and TE-5: `spec verify --staged`
//! (`docs/features/rev-rule.md` "spec verify --staged"), 4a's judge over
//! what the commit records, through 4c-1's one-commit seam: the current
//! side the index git names (`GIT_INDEX_FILE` kept), its config the
//! index's, no disk walk, no `hash-object`; `HEAD` the base. Over scratch
//! repositories of `fixtures/verify-ecs` (an ECS-style game crate whose
//! every binding is sound) with its four nodes accepted, which verifies
//! `clean`; every run through `verify_common::verify` is checked
//! read-only (the worktree, `.git` with its index, the queue).
//!
//! The lock commits of `spec lock accept` and of a drift item's approve
//! under a `spec verify --staged` hook: `drift_approve.rs`
//! `lock_commit_under_spec_verify_hooks`.

#![cfg(unix)]

mod common;
mod lock_common;
mod rev_rule_common;
mod verify_common;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::SPEC;
use common::staged::GitLog;
use lock_common::LockRepo;
use rev_rule_common::{LOCK, commit_hooked, shown, spec_at};
use serde_json::{Value, json};
use verify_common::{ECS_LIB, MOVEMENT, STAMINA, STAMINA_MD, ecs, ecs_unlocked, verify};

/// `PERK-REGEN`'s system body changed: its row `code_ahead code_changed`.
const REGEN_BODY: (&str, &str) = (
    "(stamina.current + rate * dt).min(stamina.max)",
    "(stamina.current + rate * dt * 2.0).min(stamina.max)",
);

/// `PERK-DRAIN`'s text changed at its locked revision: `spec_ahead
/// same_revision`.
const DRAIN_TEXT: (&str, &str) = ("at zero the runner walks.", "at zero the runner must walk.");

const REGEN_SYMBOL: &str = "crates/game::stamina::regen_system";
const DRAIN_SYMBOL: &str = "crates/game::stamina::drain_system";

/// The git index's empty blob (an intent-to-add entry's).
const EMPTY_BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";

/// `(state, category)` of the one row of `node`'s `symbol` in a verify
/// JSON document.
fn state_of(document: &Value, node: &str, symbol: &str) -> (Value, Value) {
    let rows: Vec<&Value> = document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["node"] == node && row["symbol"] == symbol)
        .collect();
    assert_eq!(rows.len(), 1, "{node} {symbol}: {document}");
    (rows[0]["state"].clone(), rows[0]["category"].clone())
}

fn code_ahead() -> (Value, Value) {
    (json!("code_ahead"), json!("code_changed"))
}

/// The files the rows name.
fn files(document: &Value) -> BTreeSet<String> {
    document["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter_map(|row| row["file"].as_str().map(str::to_owned))
        .collect()
}

/// `core.autocrlf=true` set and `paths` checked out again: CRLF in the
/// worktree, LF committed, `git status` clean (as `verify_cascade.rs`).
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

// ------------------------------------------------------------------ RD-5

/// RD-5 / TE-5: a code and a text change staged, the disk set back to
/// `HEAD`'s: `--staged` reads `PERK-REGEN` `code_ahead code_changed` and
/// `PERK-DRAIN` `spec_ahead same_revision`, the plain run `clean`; the
/// index reset and the same changes on the disk: `--staged` `clean`, the
/// plain run the changes. M: the disk read for the current side.
#[test]
fn verify_staged_judges_the_index_not_the_disk() {
    let repo = ecs("rd5-index");
    let code = repo.read(STAMINA);
    let text = repo.read(STAMINA_MD);
    let edit = |repo: &LockRepo| {
        repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
        repo.replace(STAMINA_MD, DRAIN_TEXT.0, DRAIN_TEXT.1);
    };
    edit(&repo);
    repo.git(&["add", "--", STAMINA, STAMINA_MD]);
    repo.write(STAMINA, code);
    repo.write(STAMINA_MD, text);

    let from_index = verify(&repo, &["--staged"]);
    assert_eq!(from_index.code(), 0, "{}", from_index.text.show());
    assert_eq!(
        state_of(&from_index.json, "PERK-REGEN", REGEN_SYMBOL),
        code_ahead()
    );
    assert_eq!(
        state_of(&from_index.json, "PERK-DRAIN", DRAIN_SYMBOL),
        (json!("spec_ahead"), json!("same_revision"))
    );
    let from_disk = verify(&repo, &[]);
    assert_eq!(from_disk.verdict(), "clean", "{}", from_disk.text.show());

    repo.git(&["reset", "-q"]);
    edit(&repo);
    let from_index = verify(&repo, &["--staged"]);
    assert_eq!(from_index.verdict(), "clean", "{}", from_index.text.show());
    let from_disk = verify(&repo, &[]);
    assert_eq!(
        state_of(&from_disk.json, "PERK-REGEN", REGEN_SYMBOL),
        code_ahead()
    );
}

/// RD-5: through a logging `git` wrapper first on `PATH`, `--staged` runs
/// no `hash-object` (the index's OIDs are the blobs) and only read-only
/// subcommands; the plain run, over the same disk, does hash. M:
/// `hash-object` under `--staged`.
#[test]
fn verify_staged_runs_no_hash_object() {
    let repo = ecs("rd5-no-hash-object");
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.git(&["add", "--", STAMINA]);
    let log = GitLog::new(&repo.scratch, &repo.git);
    let run = spec_at(
        &repo,
        &repo.root,
        &["verify", "--staged", "--json"],
        &log.env(),
    );
    assert_eq!(run.code, 0, "{}", run.show());
    assert_eq!(
        state_of(&run.json(), "PERK-REGEN", REGEN_SYMBOL),
        code_ahead()
    );
    let names: BTreeSet<String> = log
        .calls()
        .iter()
        .map(|call| call.name().to_owned())
        .collect();
    assert!(!names.contains("hash-object"), "{names:?}");
    let read_only: BTreeSet<String> =
        ["rev-parse", "ls-files", "ls-tree", "cat-file", "diff-files"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    assert!(names.is_subset(&read_only), "{names:?}");

    log.clear();
    let plain = spec_at(&repo, &repo.root, &["verify", "--json"], &log.env());
    assert_eq!(plain.code, 0, "{}", plain.show());
    assert!(
        log.calls().iter().any(|call| call.name() == "hash-object"),
        "the control: the plain run hashes the disk"
    );
}

/// RD-5, the code-reviewer's item made a test: under `--staged` every
/// object is asked of `cat-file` once, so a (path, OID) the index shares
/// with `HEAD` is never read again for the base (the index and `HEAD`
/// differ in one file here, all the others shared). M: `read_head`
/// wanting a blob the index's side already read.
#[test]
fn verify_staged_reads_each_object_once() {
    let repo = ecs("rd5-once");
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.git(&["add", "--", STAMINA]);
    let log = GitLog::new(&repo.scratch, &repo.git);
    let run = spec_at(&repo, &repo.root, &["verify", "--staged"], &log.env());
    assert_eq!(run.code, 0, "{}", run.show());
    let requests = log.requests();
    assert!(!requests.is_empty(), "the blobs come from cat-file");
    let mut seen = BTreeSet::new();
    let twice: Vec<&String> = requests
        .iter()
        .filter(|request| !seen.insert(request.as_str()))
        .collect();
    assert!(twice.is_empty(), "asked twice: {twice:?} of {requests:?}");
}

/// RD-5: an untracked file under a code root that cannot be read (a walk
/// gap) stops the plain run (exit 2, a cause); `--staged` never walks the
/// disk: `clean`, exit 0. M: a disk walk under `--staged`.
#[test]
fn verify_staged_ignores_a_disk_walk_gap() {
    let repo = ecs("rd5-walk-gap");
    let secret = repo.root.join("crates/game/src/secret.rs");
    fs::write(&secret, "pub fn secret() {}\n").expect("write");
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000)).expect("chmod");
    let plain = spec_at(&repo, &repo.root, &["verify", "--json"], &[]);
    assert_eq!(plain.code, 2, "the control: {}", plain.show());
    assert!(
        !plain.json()["cannot"]
            .as_array()
            .expect("cannot")
            .is_empty()
    );
    let from_index = spec_at(&repo, &repo.root, &["verify", "--staged", "--json"], &[]);
    assert_eq!(from_index.code, 0, "{}", from_index.show());
    let document = from_index.json();
    assert_eq!(document["verdict"], "clean", "{document}");
    assert_eq!(document["cannot"], json!([]));
    fs::set_permissions(secret, fs::Permissions::from_mode(0o644)).expect("chmod back");
}

/// RD-5: the current side is the index `GIT_INDEX_FILE` names: a code
/// change staged in another index file only (the default index and the
/// disk `HEAD`'s): through it `code_ahead`, without it `clean`.
/// M: `.git/index` read in place of `GIT_INDEX_FILE`.
#[test]
fn verify_staged_reads_git_index_file() {
    let repo = ecs("rd5-index-file");
    let code = repo.read(STAMINA);
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    let alternate = repo.scratch.join("alternate-index");
    fs::copy(repo.root.join(".git/index"), &alternate).expect("copy the index");
    repo.git.git_env(
        &repo.root,
        &["add", "--", STAMINA],
        &[("GIT_INDEX_FILE", alternate.as_os_str())],
    );
    repo.write(STAMINA, code);
    let through = spec_at(
        &repo,
        &repo.root,
        &["verify", "--staged", "--json"],
        &[("GIT_INDEX_FILE", alternate.as_os_str())],
    );
    assert_eq!(through.code, 0, "{}", through.show());
    assert_eq!(
        state_of(&through.json(), "PERK-REGEN", REGEN_SYMBOL),
        code_ahead()
    );
    let default = verify(&repo, &["--staged"]);
    assert_eq!(default.verdict(), "clean", "{}", default.text.show());
}

/// RD-5 / TE-5: the config is the index's, whole: a staged `[verify] mode
/// = "enforce"` and a staged `[code]` root `tools` (a package of its own,
/// its `Cargo.toml` staged with it) holding a stray marker naming no node
/// (the disk's config `HEAD`'s): `--staged` judges in `enforce`, the
/// stray's `bad_marker dangling` row there, a Fix that blocks (exit 1); the
/// plain run `observe`, no row of `tools/`; `--config specengine.toml`
/// (the disk) as the plain run. M: the disk config under `--staged`.
///
/// A root outside any package would bind the stray `cannot_verify
/// ambiguous_symbol unrooted` (exit 2, 4a's judge, plain alike), hence the
/// package.
#[test]
fn verify_staged_reads_the_staged_config() {
    let stray = "tools/src/lib.rs";
    let repo = ecs("rd5-config");
    let config = repo.read("specengine.toml");
    verify_common::set_mode(&repo.root, "enforce");
    repo.replace(
        "specengine.toml",
        "roots = [\"crates\", \"assets\"]",
        "roots = [\"crates\", \"assets\", \"tools\"]",
    );
    repo.write(
        "tools/Cargo.toml",
        "[package]\nname = \"tools\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[workspace]\n",
    );
    repo.write(stray, "// @implements PERK-NOPE\npub fn stray() {}\n");
    repo.git(&["add", "--", "specengine.toml", "tools"]);
    repo.write("specengine.toml", config);

    let from_index = verify(&repo, &["--staged"]);
    assert_eq!(
        from_index.json["mode"],
        "enforce",
        "{}",
        from_index.text.show()
    );
    let rows = from_index.rows_where(|row| row["file"] == stray);
    assert_eq!(rows.len(), 1, "{}", from_index.text.show());
    assert_eq!(
        (&rows[0]["state"], &rows[0]["category"], &rows[0]["class"]),
        (&json!("bad_marker"), &json!("dangling"), &json!("fix")),
        "{}",
        from_index.text.show()
    );
    assert_eq!(from_index.code(), 1, "{}", from_index.text.show());

    for extra in [&[][..], &["--staged", "--config", "specengine.toml"][..]] {
        let run = verify(&repo, extra);
        assert_eq!(run.json["mode"], "observe", "{extra:?}");
        assert!(
            !files(&run.json)
                .iter()
                .any(|file| file.starts_with("tools/")),
            "{extra:?}: {}",
            run.text.show()
        );
        assert_eq!(run.code(), 0, "{extra:?}: {}", run.text.show());
    }
}

/// RD-5 (Q18): a `spec.lock` given only an intent-to-add entry (`git add
/// -N`, its blob the empty one) is no lock under `--staged`: not exit 2,
/// no cause naming it, the same document as with no index entry at all.
/// M: an intent-to-add `spec.lock` read as a lock.
#[test]
fn verify_staged_drops_an_intent_to_add_lock() {
    let repo = ecs_unlocked("rd5-ita");
    repo.write(LOCK, "# a lock being written\n");
    repo.git(&["add", "-N", "--", LOCK]);
    let entry = repo.git_text(&["ls-files", "-s", "--", LOCK]);
    assert!(entry.contains(EMPTY_BLOB), "{entry}");
    let ita = verify(&repo, &["--staged"]);
    assert_ne!(ita.code(), 2, "{}", ita.text.show());
    assert!(
        !ita.json["cannot"].to_string().contains("spec.lock"),
        "{}",
        ita.text.show()
    );
    repo.git(&["rm", "-q", "--cached", "--", LOCK]);
    let absent = verify(&repo, &["--staged"]);
    assert_eq!(ita.json, absent.json);
    assert_eq!(ita.text.stdout, absent.text.stdout);
}

/// RD-5: `--staged` and `--changed` exclude each other, either order,
/// text or JSON: a usage error, exit 2, stdout empty.
#[test]
fn staged_and_changed_exclude_each_other() {
    let repo = ecs_unlocked("rd5-exclusive");
    for args in [
        &["verify", "--staged", "--changed"][..],
        &["verify", "--changed", "--staged"][..],
        &["--json", "verify", "--staged", "--changed"][..],
    ] {
        let run = spec_at(&repo, &repo.root, args, &[]);
        assert_eq!(run.code, 2, "{}", run.show());
        assert_eq!(run.stdout, "", "{}", run.show());
    }
}

/// Review 1's nit made a test (iteration 2, `IndexShare::bytes`): a file
/// `HEAD` and the index share whose name only ends in `Cargo.toml`, here
/// the `--config` judged (`crates/gameCargo.toml`, beside the manifest
/// `crates/game/Cargo.toml` the index's side read), is never given that
/// manifest's bytes: `HEAD`'s config is its own blob, whose `[verify]
/// mode = "enforce"`, stricter than the disk's `observe`, is the mode
/// judged, a note naming it. `--changed` (no index side) is the control.
/// M: the manifest's directory taken as `unwrap_or(dir)` (`…/gameCargo.toml`
/// read as the manifest of `…/game/`).
#[test]
fn verify_staged_gives_no_manifest_bytes_to_a_lookalike_path() {
    let config = "crates/gameCargo.toml";
    let repo = ecs("rd5-lookalike");
    let base = repo.read("specengine.toml");
    repo.write(config, format!("{base}\n[verify]\nmode = \"enforce\"\n"));
    repo.commit_all("a config named like a manifest");
    repo.write(config, format!("{base}\n[verify]\nmode = \"observe\"\n"));
    for flag in ["--changed", "--staged"] {
        let run = verify(&repo, &[flag, "--config", config]);
        assert_eq!(run.json["mode"], "enforce", "{flag}: {}", run.text.show());
        assert!(
            run.text.show().contains("stricter than `observe`"),
            "{flag}: {}",
            run.text.show()
        );
        assert_eq!(run.verdict(), "clean", "{flag}: {}", run.text.show());
        assert_eq!(run.code(), 0, "{flag}: {}", run.text.show());
    }
}

/// The same nit, the root's case: a root `Cargo.toml` (its directory the
/// empty one, above the code roots) that `HEAD` and the index share is
/// asked of `cat-file` once, by the index's side; `HEAD`'s side is given
/// its bytes, no object asked twice. M: the root's manifest never shared
/// (its empty directory refused as a lookalike's is).
#[test]
fn verify_staged_reads_a_shared_root_manifest_once() {
    let repo = ecs("rd5-root-manifest");
    repo.write(
        "Cargo.toml",
        "# The scratch repository's root manifest (rev-rule RD-5).\n[workspace]\n",
    );
    repo.commit_all("a manifest at the root");
    let root_blob = repo.blob_at("HEAD", "Cargo.toml").trim().to_owned();
    repo.replace(STAMINA, REGEN_BODY.0, REGEN_BODY.1);
    repo.git(&["add", "--", STAMINA]);
    let log = GitLog::new(&repo.scratch, &repo.git);
    let run = spec_at(
        &repo,
        &repo.root,
        &["verify", "--staged", "--json"],
        &log.env(),
    );
    assert_eq!(run.json()["cannot"], json!([]), "{}", run.show());
    let requests = log.requests();
    assert_eq!(
        requests.iter().filter(|oid| **oid == root_blob).count(),
        1,
        "the root manifest {root_blob} asked once: {requests:?}"
    );
    let mut seen = BTreeSet::new();
    let twice: Vec<&String> = requests
        .iter()
        .filter(|request| !seen.insert(request.as_str()))
        .collect();
    assert!(twice.is_empty(), "asked twice: {twice:?} of {requests:?}");
}

// ------------------------------------------------------------------ TE-5

/// TE-5: under `core.autocrlf` (CRLF in the worktree, LF committed) the
/// plain run reads the module and the RON value `code_ahead` (4a's known
/// limit, `docs/canon/spec-verify.md` "Known limits"); `--staged` compares
/// the index's clean blobs: `clean`, every row `ok` by its blob.
#[test]
fn verify_staged_reads_clean_blobs_under_autocrlf() {
    let repo = ecs("te5-autocrlf");
    autocrlf_checkout(&repo, &[STAMINA, ECS_LIB, MOVEMENT]);
    let plain = verify(&repo, &[]);
    assert_eq!(
        plain.rows_where(|row| row["state"] == "code_ahead").len(),
        2,
        "the limit: {}",
        plain.text.show()
    );
    let from_index = verify(&repo, &["--staged"]);
    assert_eq!(
        (from_index.verdict(), from_index.code()),
        ("clean", 0),
        "{}",
        from_index.text.show()
    );
    for row in from_index.rows() {
        assert_eq!(
            (&row["state"], &row["compared"]),
            (&json!("ok"), &json!("blob")),
            "{row}"
        );
    }
    assert_eq!(
        from_index.json["cascade"],
        json!({"by_blob": 6, "by_ast": 0})
    );
}

/// TE-5: `spec.lock` unmerged (two branches' accepts disagreeing on one
/// entry's `verified_at`): `--staged` cannot verify, the cause 4a's (5).
#[test]
fn verify_staged_refuses_an_unmerged_lock() {
    let repo = ecs("te5-unmerged");
    let entry = "verified_at = \"2026-10-08\"\n\n[[binding]]\nnode = \"PERK-REGEN\"";
    repo.git(&["checkout", "-q", "-b", "side"]);
    repo.replace(LOCK, entry, &entry.replace("2026-10-08", "2026-10-09"));
    repo.commit_all("side lock");
    repo.git(&["checkout", "-q", "main"]);
    repo.replace(LOCK, entry, &entry.replace("2026-10-08", "2026-10-07"));
    repo.commit_all("main lock");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "--no-edit", "side"], &[]);
    assert!(!merge.status.success(), "{}", shown(&merge));
    let run = verify(&repo, &["--staged"]);
    assert_eq!(run.code(), 2, "{}", run.text.show());
    let cannot = run.json["cannot"].as_array().expect("cannot");
    assert!(
        cannot.contains(&json!("spec.lock is unmerged: run spec lock regen")),
        "{cannot:?}"
    );
    assert!(run.rows().is_empty());
}

/// TE-5 (Q18): the state a first lock's refused accept leaves (a hook
/// edited the lock and failed: the lock and its intent-to-add entry kept,
/// as `drift_approve.rs` `lock_restore_commands_quote_the_path`); the
/// project's `pre-commit` hook then running `spec verify --staged`: the
/// next commit (another document) passes, the run alone exit 0.
#[test]
fn intent_to_add_lock_left_by_a_refused_accept_never_stops_the_next_commit() {
    let repo = ecs_unlocked("te5-ita-left");
    repo.hook(
        "pre-commit",
        "printf '# edited by a hook\\n' >> spec.lock\nexit 1",
    );
    let refusal = repo
        .accept("PERK-REGEN", true)
        .refused("an edited first lock");
    assert!(refusal.contains("intent-to-add"), "{refusal}");
    let entry = repo.git_text(&["ls-files", "-s", "--", LOCK]);
    assert!(entry.contains(EMPTY_BLOB), "{entry}");

    let hook_home = repo.scratch.home("hook");
    repo.hook(
        "pre-commit",
        &format!("HOME='{}' '{SPEC}' verify --staged", hook_home.display()),
    );
    repo.replace(
        "docs/spec/movement.md",
        "Walking costs nothing.",
        "Walking costs no stamina.",
    );
    repo.git(&["add", "--", "docs/spec/movement.md"]);
    let alone = spec_at(&repo, &repo.root, &["verify", "--staged"], &[]);
    assert_eq!(alone.code, 0, "{}", alone.show());
    let head = repo.head();
    let output = commit_hooked(&repo, "the next commit");
    assert!(output.status.success(), "{}", shown(&output));
    assert_ne!(repo.head(), head);
    assert_eq!(repo.changed(&repo.head()), ["docs/spec/movement.md"]);
}
