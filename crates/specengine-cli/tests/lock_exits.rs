//! docs/features/spec-lock.md TE-3 (every exit 1 and 2 cause of `spec
//! lock accept`; regen's are in `lock_regen.rs`), RD-4, RD-6, RD-12, TE-9
//! and TE-10: after each refusal `HEAD`, the refs, `git status
//! --porcelain=v1 -z --untracked-files=all`, the index and every file
//! outside `.git` are as they were.
//!
//! Non-Latin characters are Unicode escapes (ADR-0024).

mod common;
mod lock_common;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use common::repository_root;
use lock_common::{HEADER, LockRepo, PREAMBLE, fake_b3, fake_oid};
use specengine_cli::LockPrompt;

/// `[code]`'s refusal on a project without the table (canon
/// `code-bindings` "The `[code]` table").
const NO_CODE: &str =
    "spec: specengine.toml has no [code] table: the code layer is off (add [code] with roots)";

/// Step 9's refusal ("spec lock accept", step 9).
const CHANGED: &str = "spec.lock changed while being accepted; nothing written";

/// An accept of `reference` in `repo` that must be refused (exit 1) with
/// no question, nothing changed: the refusal.
fn refused_quietly(repo: &LockRepo, reference: &str, context: &str) -> String {
    let before = repo.state();
    let attempt = repo.accept(reference, true);
    let refusal = attempt.refused(context);
    assert!(
        attempt.prompts.is_empty(),
        "{context}: asked {:?}",
        attempt.prompts
    );
    assert_eq!(repo.state(), before, "{context}: something changed");
    refusal
}

/// An accept that cannot run (exit 2), nothing changed: the message.
fn cannot_quietly(repo: &LockRepo, reference: &str, context: &str) -> String {
    let before = repo.state();
    let attempt = repo.accept(reference, true);
    let message = attempt.cannot(context);
    assert!(
        attempt.prompts.is_empty(),
        "{context}: asked {:?}",
        attempt.prompts
    );
    assert_eq!(repo.state(), before, "{context}: something changed");
    message
}

// ------------------------------------------------------------ exit 1

/// TE-3: REF naming nothing (and no gone node's entries): exit 1.
#[test]
fn a_ref_naming_nothing_is_exit_1() {
    let repo = LockRepo::code_a("lock-x-nothing");
    let refusal = refused_quietly(&repo, "GOAL-77", "GOAL-77");
    assert!(refusal.contains("GOAL-77"), "{refusal}");
    // A lock holding other nodes' entries changes nothing.
    repo.accept_ok("PREM-001");
    refused_quietly(&repo, "GOAL-77", "GOAL-77 with a lock");
}

/// TE-3: REF naming several nodes (two holders of LAW-SOURCE): exit 1.
#[test]
fn a_ref_naming_several_nodes_is_exit_1() {
    let repo = LockRepo::code_a("lock-x-several");
    repo.write(
        "docs/spec/copy.md",
        "---\nclass: canon\nid: PIECE-COPY\nowner: owner\nreviewed: 2026-10-10\n---\n\n# Copy\n\n## Source again {#LAW-SOURCE}\n\nA second holder.\n",
    );
    repo.append(
        "crates/tally/src/main.rs",
        "\n// @implements PIECE-LIMITS#LAW-SOURCE\nfn source() -> u32 {\n    10\n}\n",
    );
    repo.commit_all("two holders");
    let refusal = refused_quietly(&repo, "LAW-SOURCE", "two holders");
    assert!(refusal.contains("names 2 nodes"), "{refusal}");
}

/// TE-3: no marker names the node: exit 1.
#[test]
fn a_node_no_marker_names_is_exit_1() {
    let repo = LockRepo::code_a("lock-x-unmarked");
    let refusal = refused_quietly(&repo, "LAW-SOURCE", "no marker");
    assert!(
        refusal.contains("no marker names `LAW-SOURCE`"),
        "{refusal}"
    );
}

/// TE-3: markers name the node but none is lockable (`predated`): exit
/// 1 saying why, the rows printed.
#[test]
fn nothing_lockable_is_exit_1() {
    let repo = LockRepo::code_a("lock-x-unlockable");
    repo.append(
        "crates/tally/src/main.rs",
        "\n// @implements LAW-DEFAULT@2\nfn default_cap() -> u32 {\n    10\n}\n",
    );
    repo.commit_all("a marker ahead of its node");
    let before = repo.state();
    let attempt = repo.accept("LAW-DEFAULT", true);
    let refusal = attempt.refused("nothing lockable");
    assert!(attempt.prompts.is_empty());
    assert!(refusal.contains("can be locked"), "{refusal}");
    assert_eq!(
        attempt.rows(),
        "not locked | implements | crates/tally/src/main.rs | item crates/tally::bin:tally-kit::default_cap function_item | predated\n"
    );
    assert_eq!(repo.state(), before);
}

/// RD-6 (M: only the node's file checked): a modified marker file that is
/// not the node's own file refuses the accept, named with its state.
#[test]
fn dirty_bound_file_refuses_accept() {
    let repo = LockRepo::code_a("lock-x-dirty-bound");
    repo.replace("data/limits.ron", "cap: 10,", "cap: 11,");
    let refusal = refused_quietly(&repo, "PIECE-COUNTER", "limits.ron modified");
    assert!(refusal.contains("`data/limits.ron` modified"), "{refusal}");
    assert!(refusal.contains("commit them first"), "{refusal}");
}

/// TE-3, RD-6: each dirty input named with its state, exit 1: the node's
/// file, a marker file modified or staged, `specengine.toml`, an
/// untracked `spec.lock`, an untracked or ignored (never committed) marker
/// file, a holder deleted or renamed in the worktree whose HEAD blob marks
/// the node.
#[test]
fn every_dirty_input_is_named() {
    type Setup = fn(&LockRepo);
    let cases: [(&str, Setup, &[&str]); 10] = [
        (
            "the node's file modified",
            |repo| {
                repo.replace(
                    "docs/spec/counter.md",
                    "never passes its cap",
                    "never passes the cap",
                )
            },
            &["`docs/spec/counter.md` modified"],
        ),
        (
            "a marker file staged",
            |repo| {
                repo.replace(
                    "crates/tally/tests/bounds.rs",
                    "tally.step();\n    assert_eq!(tally.value(), 1);",
                    "tally.step();\n    assert_eq!(tally.value(), 1); // one",
                );
                repo.git(&["add", "crates/tally/tests/bounds.rs"]);
            },
            &["`crates/tally/tests/bounds.rs` staged"],
        ),
        (
            "specengine.toml modified",
            |repo| repo.append("specengine.toml", "# a comment\n"),
            &["`specengine.toml` modified"],
        ),
        (
            "spec.lock untracked",
            |repo| repo.write("spec.lock", format!("{HEADER}\n{PREAMBLE}")),
            &["`spec.lock` untracked"],
        ),
        (
            "a marker file untracked",
            |repo| {
                repo.write(
                    "crates/tally/src/fresh.rs",
                    "// @implements PIECE-COUNTER\npub fn fresh() {}\n",
                )
            },
            &["`crates/tally/src/fresh.rs` untracked"],
        ),
        (
            "a marker file ignored, never committed",
            |repo| {
                repo.write(".gitignore", "gen.rs\n");
                repo.commit_all("ignore gen.rs");
                repo.write(
                    "crates/tally/src/gen.rs",
                    "// @implements PIECE-COUNTER\npub fn generated() {}\n",
                );
            },
            &["`crates/tally/src/gen.rs` not tracked"],
        ),
        (
            "a holder deleted",
            |repo| fs::remove_file(repo.root.join("crates/tally/tests/bounds.rs")).expect("remove"),
            &["`crates/tally/tests/bounds.rs` deleted"],
        ),
        (
            "a holder renamed in the worktree",
            |repo| {
                fs::rename(
                    repo.root.join("data/limits.ron"),
                    repo.root.join("data/bounds.ron"),
                )
                .expect("rename");
            },
            &["`data/limits.ron` deleted", "`data/bounds.ron` untracked"],
        ),
        (
            "a holder renamed by git mv",
            |repo| {
                repo.git(&["mv", "data/limits.ron", "data/bounds.ron"]);
            },
            &["`data/limits.ron` ", "`data/bounds.ron` staged"],
        ),
        (
            "two at once",
            |repo| {
                repo.replace("data/limits.ron", "cap: 10,", "cap: 12,");
                repo.append("specengine.toml", "# a comment\n");
            },
            &["`data/limits.ron` modified", "`specengine.toml` modified"],
        ),
    ];
    let mut failures = Vec::new();
    for (context, setup, named) in cases {
        let repo = LockRepo::code_a("lock-x-dirty");
        setup(&repo);
        let refusal = refused_quietly(&repo, "PIECE-COUNTER", context);
        for part in named
            .iter()
            .copied()
            .chain(["commit them first", "nothing written"])
        {
            if !refusal.contains(part) {
                failures.push(format!("{context}: {part:?} not in {refusal}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Iteration 2, 5(a) (M: drop untracked; drop manifests): the lock is
/// made of one commit's code, so every code file the walk reads and every
/// `Cargo.toml` that can name one (under a root, or at or above one) that
/// differs from HEAD is a dirty input, named, exit 1: another `.rs`
/// modified, a package manifest modified, a new `.rs` untracked, the root
/// `Cargo.toml` above the roots modified, another `.rs` deleted.
#[test]
fn every_changed_code_file_and_manifest_is_dirty() {
    type Setup = fn(&LockRepo);
    let cases: [(&str, Setup, &str); 6] = [
        (
            "another .rs modified",
            |repo| repo.append("crates/tally/src/extra/moved.rs", "\npub fn more() {}\n"),
            "`crates/tally/src/extra/moved.rs` modified",
        ),
        (
            "a package manifest modified",
            |repo| repo.append("crates/tally/Cargo.toml", "\n[features]\nextra = []\n"),
            "`crates/tally/Cargo.toml` modified",
        ),
        (
            "a new .rs untracked",
            |repo| repo.write("crates/tally/src/fresh_module.rs", "pub fn fresh() {}\n"),
            "`crates/tally/src/fresh_module.rs` untracked",
        ),
        (
            "the root manifest above the roots modified",
            |repo| {
                repo.write("Cargo.toml", "[workspace]\nmembers = []\n");
                repo.commit_all("a root manifest");
                repo.append("Cargo.toml", "exclude = [\"crates/tally\"]\n");
            },
            "`Cargo.toml` modified",
        ),
        (
            "the root manifest above the roots untracked",
            |repo| repo.write("Cargo.toml", "[workspace]\nmembers = []\n"),
            "`Cargo.toml` untracked",
        ),
        (
            "another .rs deleted",
            |repo| fs::remove_file(repo.root.join("crates/tally/src/broken.rs")).expect("remove"),
            "`crates/tally/src/broken.rs` deleted",
        ),
    ];
    let mut failures = Vec::new();
    for (context, setup, named) in cases {
        let repo = LockRepo::code_a("lock-x-code-dirty");
        setup(&repo);
        let before = repo.state();
        let attempt = repo.accept("PIECE-COUNTER", true);
        match &attempt.outcome {
            Ok(outcome)
                if outcome
                    .refusal
                    .as_deref()
                    .is_some_and(|reason| reason.contains(named)) =>
            {
                if !attempt.prompts.is_empty() || repo.state() != before {
                    failures.push(format!("{context}: asked or changed something"));
                }
            }
            other => failures.push(format!(
                "{context}: expected exit 1 naming {named}: {other:?}"
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Iteration 2, 5(a): what the walk never reads stays out: an excluded
/// `.rs`, a `.`-named `.rs`, a file outside the roots, a manifest outside
/// them; the accept commits.
#[test]
fn files_the_walk_never_reads_are_no_inputs() {
    let repo = LockRepo::code_a("lock-x-code-clean");
    repo.append(
        "crates/tally/tests/fixtures/sample.rs",
        "\npub fn more() {}\n",
    );
    repo.append("crates/tally/src/.draft.rs", "\npub fn more() {}\n");
    repo.write("NOTES.txt", "untracked\n");
    repo.write("docs/Cargo.toml", "[package]\nname = \"elsewhere\"\n");
    let status = repo.porcelain();
    repo.accept_ok("PIECE-COUNTER");
    assert_eq!(repo.porcelain(), status, "the other files as they were");
}

/// Iteration 2, 5(b) (M: judge gone from the working tree): LAW-STEP's
/// section deleted in the working tree only is not gone (HEAD still holds
/// it): exit 1 naming the node file, no question, nothing dropped. Once
/// the deletion is committed, the drop proceeds.
#[test]
fn a_node_deleted_only_in_the_worktree_is_not_gone() {
    let repo = LockRepo::code_a("lock-x-gone-worktree");
    repo.accept_ok("LAW-STEP");
    let text = repo.read("docs/spec/counter.md");
    let start = text.find("## Step {#LAW-STEP}").expect("the section");
    let end = text.find("## Odd {#LAW-ODD").expect("the next one");
    repo.write(
        "docs/spec/counter.md",
        format!("{}{}", &text[..start], &text[end..]),
    );
    let refusal = refused_quietly(&repo, "LAW-STEP", "deleted in the worktree only");
    assert!(
        refusal.contains("`docs/spec/counter.md` modified"),
        "{refusal}"
    );
    assert!(repo.lock_text().contains("node = \"LAW-STEP\""));
    repo.commit_all("LAW-STEP deleted");
    let attempt = repo.accept("LAW-STEP", true);
    attempt.committed("gone once committed");
    assert!(!repo.lock_text().contains("node = \"LAW-STEP\""));
}

/// TE-3: a written revision that is no number from 1 (LAW-ODD `rev=0`):
/// exit 1.
#[test]
fn an_unreadable_revision_is_exit_1() {
    let repo = LockRepo::code_a("lock-x-rev");
    let refusal = refused_quietly(&repo, "LAW-ODD", "rev=0");
    assert!(refusal.contains("revision"), "{refusal}");
}

/// TE-3: a node whose `norm_hash` is null (its file's front-matter has a
/// `frontmatter-type` finding): exit 1.
#[test]
fn a_null_norm_hash_is_exit_1() {
    let repo = LockRepo::code_a("lock-x-norm");
    repo.replace(
        "docs/records/PREM-001.md",
        "owner: owner\n",
        "owner: owner\nlinks: 5\n",
    );
    repo.commit_all("a front-matter type error");
    let shown = repo.spec_piped(&repo.root, &["show", "PREM-001"]);
    assert!(
        !shown
            .stdout
            .lines()
            .next()
            .unwrap_or_default()
            .contains(" | norm b3:"),
        "the setup gives a null norm_hash: {}",
        shown.show()
    );
    let refusal = refused_quietly(&repo, "PREM-001", "null norm_hash");
    assert!(refusal.contains("norm_hash"), "{refusal}");
}

/// TE-3: `spec.lock` ignored by git, no lock yet: exit 1 before asking.
#[test]
fn an_ignored_lock_is_exit_1_before_asking() {
    let repo = LockRepo::code_a("lock-x-ignored");
    repo.write(".gitignore", "spec.lock\n");
    repo.commit_all("ignore the lock");
    let refusal = refused_quietly(&repo, "PIECE-COUNTER", "ignored");
    assert!(refusal.contains("ignored"), "{refusal}");
}

/// TE-3: only `y` or `yes` on the terminal consents; anything else is
/// exit 1, nothing written.
#[test]
fn only_y_or_yes_consents() {
    let repo = LockRepo::code_a("lock-x-answer");
    for answer in ["n", "", "Y", "YES", "no", "yes please"] {
        let before = repo.state();
        let run = repo.spec_pty(
            &repo.root,
            &["lock", "accept", "PIECE-COUNTER"],
            Some(answer),
        );
        run.code(1);
        assert!(run.asked, "{answer:?}: {}", run.show());
        assert!(
            run.stderr.contains("not confirmed"),
            "{answer:?}: {}",
            run.show()
        );
        assert_eq!(repo.state(), before, "{answer:?}");
    }
    let run = repo.spec_pty(
        &repo.root,
        &["lock", "accept", "PIECE-COUNTER"],
        Some("yes"),
    );
    run.code(0);
    assert!(repo.lock().is_some());
    // The library: a declined consent is exit 1.
    let repo = LockRepo::code_a("lock-x-answer-lib");
    let before = repo.state();
    let attempt = repo.accept("PIECE-COUNTER", false);
    assert!(attempt.refused("declined").contains("not confirmed"));
    assert_eq!(attempt.prompts.len(), 1);
    assert_eq!(repo.state(), before);
}

/// RD-12 (M: no re-read): the lock's bytes, HEAD or the branch changed
/// while the question waits: exit 1 `spec.lock changed while being
/// accepted; nothing written`; spec.lock is not written over what the
/// change left, no lock commit.
#[test]
fn lock_changed_while_accepting_is_refused() {
    type Meddle = fn(&LockRepo);
    let cases: [(&str, bool, Meddle); 4] = [
        ("the lock's bytes", true, |repo| {
            repo.append("spec.lock", "\n")
        }),
        ("HEAD", true, |repo| {
            repo.write("NOTES.txt", "meanwhile\n");
            repo.commit_all("meanwhile");
        }),
        ("the branch", true, |repo| {
            repo.git(&["switch", "-q", "-c", "elsewhere"]);
        }),
        ("a first lock appearing", false, |repo| {
            repo.write("spec.lock", format!("{HEADER}\n{PREAMBLE}"));
        }),
    ];
    for (context, with_lock, meddle) in cases {
        let repo = LockRepo::code_a("lock-x-changed");
        if with_lock {
            repo.accept_ok("PREM-001");
        }
        let head = repo.head();
        let mut asked = 0;
        let mut consent = |_: &LockPrompt<'_>| {
            asked += 1;
            meddle(&repo);
            true
        };
        let outcome = repo
            .accept_with(
                &repo.root,
                "PIECE-COUNTER",
                false,
                lock_common::CLOCK,
                repo.git_env(&repo.root),
                &mut consent,
            )
            .unwrap_or_else(|error| panic!("{context}: {error}"));
        assert_eq!(asked, 1, "{context}");
        assert_eq!(
            outcome.refusal.as_deref(),
            Some(CHANGED),
            "{context}: {outcome:?}"
        );
        assert!(outcome.commit.is_none(), "{context}");
        let log = repo.git_text(&["log", "--format=%s", "-3"]);
        assert!(
            !log.lines()
                .next()
                .unwrap_or_default()
                .starts_with("spec: lock PIECE-COUNTER"),
            "{context}: a lock commit was made: {log}"
        );
        if context == "HEAD" {
            assert_ne!(repo.head(), head);
        } else {
            assert_eq!(repo.head(), head, "{context}");
        }
        let lock = repo.lock_text();
        assert!(
            !lock.contains("node = \"PIECE-COUNTER\""),
            "{context}: written:\n{lock}"
        );
    }
}

// ------------------------------------------------------------ exit 2

/// RD-4 (M: REF resolved before it): with stdin no terminal, `spec lock
/// accept` refuses with the terminal's message whatever REF and project:
/// a REF naming nothing, a look-alike REF, no project at all; nothing is
/// read (no data directory made). `spec lock regen` too.
#[test]
fn not_a_terminal_wins_over_a_bad_ref() {
    let repo = LockRepo::code_a("lock-x-terminal");
    let outside = repo.scratch.dir("outside");
    let look_alike = "L\u{0410}W-STEP";
    let cases: [(&std::path::Path, Vec<&str>, &str); 5] = [
        (&repo.root, vec!["lock", "accept", "GOAL-77"], "lock accept"),
        (
            &repo.root,
            vec!["lock", "accept", look_alike],
            "lock accept",
        ),
        (
            &repo.root,
            vec!["lock", "accept", "PIECE-COUNTER"],
            "lock accept",
        ),
        (
            &outside,
            vec!["lock", "accept", "PIECE-COUNTER"],
            "lock accept",
        ),
        (&outside, vec!["lock", "regen"], "lock regen"),
    ];
    for (cwd, args, command) in cases {
        let before = repo.state();
        let run = repo.spec_piped(cwd, &args);
        run.code(2);
        assert!(run.stdout.is_empty(), "{}", run.show());
        assert!(
            run.stderr.contains(&format!("`spec {command}`")) && run.stderr.contains("terminal"),
            "{args:?}: {}",
            run.show()
        );
        assert!(!run.stderr.contains("GOAL-77"), "{}", run.show());
        assert_eq!(repo.state(), before);
        assert!(
            common::snapshot(&repo.home).is_empty(),
            "{args:?}: the data directory was made"
        );
    }
}

/// TE-3: usage errors are exit 2: a bare `spec lock`, no REF, two REFs,
/// `--yes`, 05's `--regen`, `regen` with an argument.
#[test]
fn usage_errors_are_exit_2() {
    let repo = LockRepo::code_a("lock-x-usage");
    let before = repo.state();
    for args in [
        vec!["lock"],
        vec!["lock", "accept"],
        vec!["lock", "accept", "GOAL-01", "LAW-STEP"],
        vec!["lock", "accept", "GOAL-01", "--yes"],
        vec!["lock", "--regen"],
        vec!["lock", "regen", "spec.lock"],
        vec!["lock", "show"],
    ] {
        let run = repo.spec_pty(&repo.root, &args, None);
        run.code(2);
        assert!(!run.asked, "{}", run.show());
        assert!(
            run.stdout.is_empty() || run.stdout.contains("Usage"),
            "{}",
            run.show()
        );
        assert!(
            !run.stderr.contains("stdin is not one"),
            "{args:?}: {}",
            run.show()
        );
    }
    assert_eq!(repo.state(), before);
}

/// TE-3: discovery (no `specengine.toml` above the cwd) and a broken
/// config are exit 2.
#[test]
fn discovery_and_config_errors_are_exit_2() {
    let repo = LockRepo::code_a("lock-x-config");
    let outside = repo.scratch.dir("outside");
    let run = repo.spec_pty(&outside, &["lock", "accept", "PIECE-COUNTER"], None);
    run.code(2);
    assert!(!run.asked);
    repo.append("specengine.toml", "[ids\n");
    repo.commit_all("a broken config");
    let message = cannot_quietly(&repo, "PIECE-COUNTER", "broken config");
    assert!(message.contains("specengine.toml"), "{message}");
}

/// TE-3, TE-10: `fixtures/spec-c` has no `[code]`: exit 2 with exactly
/// `spec refs`' text, accept and regen alike.
#[test]
fn no_code_table_is_exit_2_naming_it() {
    let repo = LockRepo::of("lock-x-spec-c", "spec-c", "repo", &[]);
    let message = cannot_quietly(&repo, "CHK-01", "spec-c");
    assert_eq!(message, NO_CODE);
    let regen = repo.regen(true);
    assert_eq!(regen.cannot("spec-c regen"), NO_CODE);
    let run = repo.spec_pty(&repo.root, &["lock", "accept", "CHK-01"], None);
    run.code(2);
    assert_eq!(run.stderr.trim_end(), NO_CODE, "{}", run.show());
}

/// TE-3: a walk gap (a code file that cannot be read): exit 2.
#[test]
fn a_walk_gap_is_exit_2() {
    let repo = LockRepo::code_a("lock-x-gap");
    repo.write("crates/tally/src/sealed.rs", "pub fn sealed() {}\n");
    repo.commit_all("a sealed file");
    let sealed = repo.root.join("crates/tally/src/sealed.rs");
    let before = repo.state();
    fs::set_permissions(&sealed, fs::Permissions::from_mode(0o000)).expect("chmod");
    let attempt = repo.accept("PIECE-COUNTER", true);
    fs::set_permissions(&sealed, fs::Permissions::from_mode(0o644)).expect("chmod back");
    let message = attempt.cannot("unreadable file");
    assert!(message.contains("the code walk is incomplete"), "{message}");
    assert!(attempt.prompts.is_empty());
    assert_eq!(repo.state(), before);
}

/// TE-3: no git worktree, an unborn HEAD, a detached HEAD, an operation
/// in progress: exit 2.
#[test]
fn git_places_that_cannot_lock_are_exit_2() {
    // No git at all.
    let repo = LockRepo::uncommitted("lock-x-nogit");
    let before = common::snapshot(&repo.root);
    let attempt = repo.accept("PIECE-COUNTER", true);
    attempt.cannot("no worktree");
    assert_eq!(common::snapshot(&repo.root), before);

    // Unborn.
    let repo = LockRepo::uncommitted("lock-x-unborn");
    repo.git.init(&repo.root);
    cannot_quietly(&repo, "PIECE-COUNTER", "unborn HEAD");

    // Detached.
    let repo = LockRepo::code_a("lock-x-detached");
    repo.git(&["checkout", "-q", "--detach"]);
    cannot_quietly(&repo, "PIECE-COUNTER", "detached HEAD");

    // A merge in progress (a conflict in a file no binding reads).
    let repo = LockRepo::code_a("lock-x-merging");
    repo.git(&["switch", "-q", "-c", "side"]);
    repo.write("NOTES.txt", "side\n");
    repo.commit_all("side");
    repo.git(&["switch", "-q", "main"]);
    repo.write("NOTES.txt", "main\n");
    repo.commit_all("main");
    let merge = repo
        .git
        .git_output(&repo.root, &["merge", "-q", "side"], &[]);
    assert!(!merge.status.success(), "the merge conflicts");
    let message = cannot_quietly(&repo, "PIECE-COUNTER", "merge in progress");
    assert!(message.contains("merge"), "{message}");
}

/// TE-3: a lock that cannot be read (not TOML, an unknown key) or was
/// written with other recipes: exit 2, `spec.lock:<line>: …`, the
/// recipes both named.
#[test]
fn an_unreadable_lock_or_other_recipes_is_exit_2() {
    // At rev 2, above PREM-001's: the accept refreshes it (no text
    // changed at R).
    let entry = lock_common::block(
        "PREM-001",
        2,
        &fake_b3('a'),
        "implements",
        "crates/tally/src/main.rs",
        "item",
        "crates/tally::bin:tally-kit::main",
        Some("function_item"),
        &lock_common::Code::fake('b', true),
        &fake_oid('c'),
        &fake_oid('d'),
        "2026-10-01",
    );
    let valid = lock_common::lock_of(std::slice::from_ref(&entry));
    let cases: [(&str, String, &[&str]); 3] = [
        (
            "not TOML",
            format!("{HEADER}\n{PREAMBLE}\n[[binding]]\nnode = \n"),
            &["spec.lock:"],
        ),
        (
            "an unknown key",
            valid.replace(
                "verified_by = \"owner\"\n",
                "verified_by = \"owner\"\nline = 12\n",
            ),
            &["spec.lock:25:", "line"],
        ),
        (
            "other recipes",
            valid.replace(
                "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15",
                "specengine-hash/v1 tree-sitter-rust/0.23.0 abi/14",
            ),
            &[
                "spec.lock:4: ",
                "specengine-hash/v1 tree-sitter-rust/0.23.0 abi/14",
                "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15",
                "run spec lock rebase on a terminal",
            ],
        ),
    ];
    for (context, text, named) in cases {
        let repo = LockRepo::code_a("lock-x-badlock");
        repo.write("spec.lock", &text);
        repo.commit_all(context);
        let message = cannot_quietly(&repo, "PIECE-COUNTER", context);
        for part in named {
            assert!(
                message.contains(part),
                "{context}: {part:?} not in {message}"
            );
        }
    }
    // The valid one reads: PREM-001's accept refreshes its fake entry.
    let repo = LockRepo::code_a("lock-x-goodlock");
    repo.write("spec.lock", &valid);
    repo.commit_all("a hand-made lock");
    let attempt = repo.accept("PREM-001", true);
    attempt.committed("a valid lock");
}

/// TE-3: no committer identity: exit 2 before the question.
#[test]
fn no_committer_identity_is_exit_2() {
    let repo = LockRepo::code_a("lock-x-ident");
    repo.git(&["config", "user.useConfigOnly", "true"]);
    let git = repo
        .git_env(&repo.root)
        .without_var("GIT_COMMITTER_NAME")
        .without_var("GIT_COMMITTER_EMAIL")
        .without_var("GIT_AUTHOR_NAME")
        .without_var("GIT_AUTHOR_EMAIL")
        .with_var("EMAIL", "");
    let before = repo.state();
    let mut asked = 0;
    let mut consent = |_: &LockPrompt<'_>| {
        asked += 1;
        true
    };
    let outcome = repo.accept_with(
        &repo.root,
        "PIECE-COUNTER",
        false,
        lock_common::CLOCK,
        git,
        &mut consent,
    );
    let error = outcome.expect_err("no identity: exit 2");
    assert_eq!(error.exit.code(), 2, "{error}");
    assert!(error.message.contains("committer identity"), "{error}");
    assert_eq!(asked, 0, "asked before failing");
    assert_eq!(repo.state(), before);
}

// ------------------------------------------------------------------ TE-9

/// TE-9 (M: plain check reads the lock): plain `spec check` prints and
/// exits the same with no lock, a committed one and an unreadable one; the
/// repository's `.githooks/pre-commit` is unchanged from HEAD.
#[test]
fn plain_check_never_reads_the_lock() {
    let repo = LockRepo::code_a("lock-check");
    let check = |repo: &LockRepo| {
        let run = repo.spec_piped(&repo.root, &["check"]);
        (run.code, run.stdout, run.stderr)
    };
    let without = check(&repo);
    repo.accept_ok("PIECE-COUNTER");
    assert_eq!(check(&repo), without, "a committed lock");
    repo.write("spec.lock", "this is = not [ a lock\n");
    assert_eq!(check(&repo), without, "an unreadable lock");
    let json = |repo: &LockRepo| repo.spec_piped(&repo.root, &["check", "--json"]).stdout;
    let broken = json(&repo);
    repo.git(&["checkout", "--", "spec.lock"]);
    assert_eq!(json(&repo), broken, "--json alike");

    let root = repository_root();
    let sandbox = common::git::Sandbox::new(repo.scratch.path());
    let committed = sandbox.git_output(&root, &["show", "HEAD:.githooks/pre-commit"], &[]);
    assert!(
        committed.status.success(),
        "{}",
        String::from_utf8_lossy(&committed.stderr)
    );
    assert_eq!(
        fs::read(root.join(".githooks/pre-commit")).expect("the hook"),
        committed.stdout,
        ".githooks/pre-commit differs from HEAD"
    );
}
