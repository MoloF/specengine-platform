//! docs/features/tested-evidence.md (8b-1) TE-4: every other lock writer
//! over one repository holding real evidence (a scratch copy of
//! `fixtures/verify-tests`, 8a's fake runner, one passing run): accept,
//! `--editorial`, 4b-1's editorial approve, a drift approve, a gone node,
//! and a rebase of a simulated grammar bump (lock-rebase.md TE-3's
//! pattern) giving `rebased`, `entry_dropped` and `unproven`; after each
//! writer plain `spec verify` reads the blocks as "The state" says. Regen of
//! format 2 against 3 is `lock_regen.rs`' `regen_merges_format_2_and_3_stages`.
//! Each step names the writer's rule it checks (`M:` that rule broken).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod drift_common;
mod lock_common;
mod rev_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use common::proposal::edit;
use lock_common::{LockRepo, Variant, node_blocks};
use rev_common::{At, approve_at, proposed};
use specengine_cli::{Globals, ShowRequest, show};
use tested_common::{block_nodes, block_of, plain, state_of};
use tested_kiln::pass;
use verify_tests_common::{KILN_MD, Kiln, RAMP};

/// The library's view from the root.
fn at(repo: &LockRepo) -> At<'_> {
    At::new(&repo.root, &repo.home, repo.git_env(&repo.root))
}

/// `(span_hash, text)` of `reference` as `spec show` gives them.
fn shown(repo: &LockRepo, reference: &str) -> (String, String) {
    let outcome = show(
        &repo.env(&repo.root),
        &Globals::default(),
        &ShowRequest {
            reference: reference.to_owned(),
            links: false,
            archive: false,
            bindings: false,
            git: repo.git_env(&repo.root),
        },
    )
    .unwrap_or_else(|error| panic!("show {reference}: {error}"));
    let node = outcome.nodes.into_iter().next().expect("one node");
    (node.span_hash, node.text)
}

fn pair(state: &str, category: &str) -> (String, String) {
    (state.to_owned(), category.to_owned())
}

#[test]
fn writers_over_one_repository() {
    let kiln = Kiln::new("tw-writers");
    let repo = &kiln.repo;
    pass(&kiln);
    let evidence = repo.lock_text();

    // Accept (M: accept rewrites a block): three nodes' entries added,
    // every block byte for byte, each accept's row `kept | tested`.
    for node in ["KILN-HOLD", "glaze-check/GLZ-01", "KILN-RAMP"] {
        let attempt = repo.accept(node, true);
        let outcome = attempt.committed(node).clone();
        assert_eq!(outcome.tested, Some("kept"), "{node}");
        assert_eq!(
            attempt.rows().lines().last(),
            Some("kept | tested"),
            "{node}"
        );
    }
    let accepted = repo.lock_text();
    assert_eq!(
        tested_common::tested_blocks(&accepted),
        tested_common::tested_blocks(&evidence),
        "accept keeps every block"
    );
    for node in tested_common::BLOCK_NODES {
        assert_eq!(
            state_of(&plain(repo, &[]), node),
            pair("verified", ""),
            "{node}"
        );
    }

    // `--editorial` (M: the block skipped): KILN-HOLD's text changed at
    // its revision; its entries and block take the new `norm_hash`, and
    // its evidence still reads `verified`.
    repo.replace(
        KILN_MD,
        "At the peak the kiln keeps its temperature until the hold time ends.",
        "At the peak, the kiln keeps its temperature until the hold time ends.",
    );
    repo.commit_all("an editorial change of the hold");
    assert_eq!(
        state_of(&plain(repo, &[]), "KILN-HOLD"),
        pair("stale", "spec_changed")
    );
    let attempt = repo.editorial("KILN-HOLD", true);
    assert_eq!(attempt.committed("--editorial").tested, Some("refreshed"));
    let norm = repo.norm_hash("KILN-HOLD");
    let block = block_of(&repo.lock_text(), "KILN-HOLD").expect("its block");
    assert!(
        block.contains(&format!("\nspec_hash = \"{norm}\"\n")),
        "{block}"
    );
    assert_eq!(
        state_of(&plain(repo, &[]), "KILN-HOLD"),
        pair("verified", "")
    );

    // 4b-1's editorial approve (M: the apply skips the block): GLZ-01's
    // text, editorially, through the queue: its block refreshed in the
    // apply commit, `Lock: glaze-check/GLZ-01`; still `verified`.
    let (hash, text) = shown(repo, "glaze-check/GLZ-01");
    let id = proposed(
        &at(repo),
        "glaze-check/GLZ-01",
        &hash,
        &edit(
            &text,
            "the temperature stays at the peak.",
            "the temperature stays at its peak.",
        ),
    );
    approve_at(&at(repo), &id, true, true).applied("the editorial approve");
    let head = repo.head();
    assert!(
        repo.message(&head).contains("\nLock: glaze-check/GLZ-01\n"),
        "{}",
        repo.message(&head)
    );
    assert_eq!(
        state_of(&plain(repo, &[]), "glaze-check/GLZ-01"),
        pair("verified", "")
    );

    // A drift approve (4c-1; M: an accept refreshing evidence): KILN-RAMP's
    // implementation edited, its drift raised and approved: its entries
    // refreshed, its block kept byte for byte, so it reads `stale
    // code_changed` (only a test run refreshes evidence).
    let before = block_of(&repo.lock_text(), "KILN-RAMP");
    repo.replace(
        RAMP,
        "(now + rate).min(peak)",
        "now.saturating_add(rate).min(peak)",
    );
    repo.commit_all("the ramp saturates");
    let raised = drift_common::raised(repo, &["KILN-RAMP"]);
    let (item, _) = raised
        .created()
        .into_iter()
        .find(|(_, node)| node == "KILN-RAMP")
        .unwrap_or_else(|| panic!("a drift item: {}", raised.text()));
    drift_common::approve_yes(repo, &item).done("the drift approve");
    assert_eq!(block_of(&repo.lock_text(), "KILN-RAMP"), before, "kept");
    assert_eq!(
        state_of(&plain(repo, &[]), "KILN-RAMP"),
        pair("stale", "code_changed")
    );

    // A gone node (M: its block kept): CONE-01's record deleted: its block
    // dropped by its accept, `dropped | tested`.
    std::fs::remove_file(repo.root.join("docs/records/CONE-01.md")).expect("CONE-01");
    repo.commit_all("CONE-01 gone");
    assert_eq!(
        state_of(&plain(repo, &[]), "CONE-01"),
        pair("node_gone", "")
    );
    let attempt = repo.accept("CONE-01", true);
    attempt.committed("CONE-01 gone");
    assert_eq!(attempt.rows(), "dropped | tested\n");
    assert!(block_of(&repo.lock_text(), "CONE-01").is_none());

    // A rebase of a simulated grammar bump (M: a block kept at the old
    // hash, or re-derived without proof): the lock under another
    // grammar's `code_recipe`, GLZ-01's entry at a blob no object has:
    // KILN-HOLD's block proven over its entries, re-derived (`rebased`);
    // GLZ-01's an entry dropped (`entry_dropped`); KILN-RAMP's at older
    // code than its refreshed entries (`unproven`).
    let lock = repo.lock_text();
    assert_eq!(node_blocks(&lock, "glaze-check/GLZ-01").len(), 1, "{lock}");
    let missing = lock_common::fake_oid('d');
    let bumped = lock_common::rebase::edit_block(
        &lock,
        (
            "glaze-check/GLZ-01",
            "verifies",
            "crates/kiln::test:schedule::glaze_survives_the_hold",
        ),
        |block| lock_common::rebase::set_string(block, "file_blob", &missing),
    )
    .replacen(lock_common::CODE_RECIPE, Variant::OTHER_CODE, 1);
    assert_ne!(bumped, lock);
    repo.write("spec.lock", &bumped);
    repo.commit_all("a grammar bump");
    let attempt = repo.rebase(true);
    attempt.committed("the bump");
    let rows = attempt.rows();
    let tested: Vec<&str> = rows
        .lines()
        .filter(|line| line.contains(" | tested"))
        .collect();
    assert_eq!(
        tested,
        [
            "rebased | KILN-HOLD | tested",
            "dropped | KILN-RAMP | tested | unproven",
            "dropped | glaze-check/GLZ-01 | tested | entry_dropped",
        ],
        "{rows}"
    );
    let after = repo.lock_text();
    assert_eq!(block_nodes(&after), ["KILN-HOLD"]);
    assert_eq!(
        state_of(&plain(repo, &[]), "KILN-HOLD"),
        pair("verified", "")
    );
}
