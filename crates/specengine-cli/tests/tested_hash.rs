//! docs/features/tested-evidence.md (8b-1) RD-3 ("Bound hash"): what a
//! block's `bound_hash` hashes, seen through plain `spec verify`'s
//! `tested` after a passing `spec verify --tests` run wrote the block:
//! never `file_blob` nor any git OID, never the levels, every relation
//! (`implements`, `verifies`, `configures`), an entry's digests only at its
//! own blob. Scratch repositories of `fixtures/verify-tests` with 8a's fake
//! runner; edits made in the worktree after the run (plain verify reads
//! it). Each test names the mutation that turns it red (`M:`).
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![cfg(unix)]

mod common;
mod lock_common;
mod tested_common;
mod tested_kiln;
mod verify_common;
mod verify_tests_common;

use tested_common::{block_of, block_value, plain, state_of};
use tested_kiln::{FIRING_RON, RAMP_COMMENT, RAMP_SQUEEZED, add_firing, expected_blocks, pass};
use verify_tests_common::{Kiln, RAMP, SCHEDULE};

/// `(state, category)` of `node` in plain verify over `kiln` now.
fn now(kiln: &Kiln, node: &str) -> (String, String) {
    state_of(&plain(&kiln.repo, &[]), node)
}

fn verified() -> (String, String) {
    ("verified".to_owned(), String::new())
}

fn code_changed() -> (String, String) {
    ("stale".to_owned(), "code_changed".to_owned())
}

/// RD-3 **M3** (`file_blob` hashed): after a pass, a comment far from
/// every binding of `ramp.rs` and of `schedule.rs` (the file of KILN-HOLD's
/// module binding) changes both files' blobs and no binding's digests:
/// every block still `verified`; a rustfmt-like respacing of
/// `next_temperature` too.
#[test]
fn comment_elsewhere_keeps_verified() {
    let kiln = Kiln::new("th-comment");
    pass(&kiln);
    for node in tested_common::BLOCK_NODES {
        assert_eq!(now(&kiln, node), verified(), "{node} after the pass");
    }
    kiln.repo.append(RAMP, RAMP_COMMENT);
    kiln.repo.append(SCHEDULE, "\n// A note on schedules.\n");
    for node in tested_common::BLOCK_NODES {
        assert_eq!(now(&kiln, node), verified(), "{node}: a comment elsewhere");
    }
    kiln.repo.replace(RAMP, RAMP_SQUEEZED.0, RAMP_SQUEEZED.1);
    assert_eq!(now(&kiln, "KILN-RAMP"), verified(), "respaced");
}

/// RD-3 (M: the marker's `levels` honoured): KILN-RAMP's `@implements`
/// marker narrowed to `[sig]` before the pass; an edit of
/// `next_temperature`'s body afterwards makes it `stale code_changed`
/// (every digest of the binding is hashed, whatever the marker's levels);
/// a marker narrowed after the pass, the code as it was, stays `verified`.
#[test]
fn body_edit_under_a_sig_marker_stales_evidence() {
    let kiln = Kiln::new("th-sig");
    kiln.repo.replace(
        RAMP,
        "// @implements KILN-RAMP\n",
        "// @implements KILN-RAMP [sig]\n",
    );
    kiln.repo.commit_all("a sig marker");
    pass(&kiln);
    assert_eq!(now(&kiln, "KILN-RAMP"), verified());
    kiln.repo.replace(
        RAMP,
        "(now + rate).min(peak)",
        "now.saturating_add(rate).min(peak)",
    );
    assert_eq!(now(&kiln, "KILN-RAMP"), code_changed(), "a body edit");

    let kiln = Kiln::new("th-narrowed");
    pass(&kiln);
    kiln.repo.replace(
        RAMP,
        "// @implements KILN-RAMP\n",
        "// @implements KILN-RAMP [sig]\n",
    );
    assert_eq!(now(&kiln, "KILN-RAMP"), verified(), "a narrowed marker");
}

/// RD-3 (M: a `verifies` or `configures` binding left out): with a RON
/// value bound to KILN-RAMP (`@configures`) before the pass, editing its
/// test's body, its bound RON value, or adding a `@verifies` marker to
/// another test makes it `stale code_changed`, each alone; a sibling RON
/// value or a RON comment keeps it `verified`.
#[test]
fn test_or_config_edit_stales_evidence() {
    let kiln = Kiln::new("th-test-config");
    add_firing(&kiln);
    pass(&kiln);
    assert_eq!(now(&kiln, "KILN-RAMP"), verified());
    let ron = kiln.repo.read(FIRING_RON);
    let ramp = kiln.repo.read(RAMP);
    let schedule = kiln.repo.read(SCHEDULE);

    kiln.repo.replace(
        RAMP,
        "assert_eq!(next_temperature(1190, 50, 1200), 1200);",
        "assert_eq!(next_temperature(1180, 50, 1200), 1200);",
    );
    assert_eq!(now(&kiln, "KILN-RAMP"), code_changed(), "the test edited");
    kiln.repo.write(RAMP, &ramp);

    kiln.repo.replace(FIRING_RON, "peak: 1200,", "peak: 1250,");
    assert_eq!(now(&kiln, "KILN-RAMP"), code_changed(), "the bound value");
    kiln.repo.write(FIRING_RON, &ron);

    kiln.repo.replace(
        SCHEDULE,
        "// @verifies glaze-check/GLZ-01\n",
        "// @verifies glaze-check/GLZ-01\n// @verifies KILN-RAMP\n",
    );
    assert_eq!(now(&kiln, "KILN-RAMP"), code_changed(), "a binding added");
    kiln.repo.write(SCHEDULE, &schedule);

    kiln.repo
        .replace(FIRING_RON, "hold_minutes: 60,", "hold_minutes: 90,");
    kiln.repo.replace(
        FIRING_RON,
        "// Firing tuning.",
        "// Firing tuning, revised.",
    );
    assert_eq!(
        now(&kiln, "KILN-RAMP"),
        verified(),
        "a sibling and a comment"
    );
}

/// RD-3 (M: a git OID hashed): a SHA-256 repository of the same files
/// writes the same `bound_hash` and `spec_hash` in every block as a SHA-1
/// one (only `commit` differs, 64 hex), each the hash "Bound hash" spells
/// over the fixture's bindings; plain verify reads `verified` there.
#[test]
fn sha256_repository_same_bound_hash() {
    let one = Kiln::new("th-sha1");
    let want = expected_blocks(&one, &tested_common::BLOCK_NODES);
    pass(&one);
    let sha256 = Kiln::bare(
        lock_common::LockRepo::of(
            "th-sha256",
            verify_tests_common::FIXTURE,
            "repo",
            &["--object-format=sha256"],
        ),
        verify_tests_common::SLUG,
    );
    sha256.write_runner(verify_tests_common::RUNNER, 0o755);
    sha256.repo.commit_all("the runner");
    let head = sha256.repo.head();
    assert_eq!(head.len(), 64);
    pass(&sha256);
    let one_lock = one.repo.lock_text();
    let lock = sha256.repo.lock_text();
    for block in &want {
        let node = block.node.as_str();
        let got = block_of(&lock, node).unwrap_or_else(|| panic!("{node}'s block"));
        let theirs = block_of(&one_lock, node).expect("the SHA-1 block");
        for key in ["bound_hash", "spec_hash"] {
            assert_eq!(
                block_value(&got, key),
                block_value(&theirs, key),
                "{node} {key}"
            );
        }
        assert_eq!(
            block_value(&got, "bound_hash"),
            block.bound_hash,
            "{node}: the spec's"
        );
        assert_eq!(block_value(&got, "commit"), head, "{node}: the run's HEAD");
        assert_eq!(now(&sha256, node), verified(), "{node} over SHA-256");
    }
}

/// RD-3 (On a read; M: an entry's digests reused at another blob): over a
/// lock holding KILN-RAMP's accepted entries and its block, the entries at
/// their blobs give the bound hash (`verified`); `next_temperature`'s body
/// edited in the worktree (its entry's `file_blob` no longer the disk's)
/// is parsed again, so the block reads `stale code_changed`, never the
/// entry's old digests.
#[test]
fn entry_digests_reused_only_at_its_blob() {
    let kiln = Kiln::new("th-entry-blob");
    kiln.repo.accept_ok("KILN-RAMP");
    pass(&kiln);
    let lock = kiln.repo.lock_text();
    assert!(lock.contains("\n[[binding]]\n") && lock.contains("\n[[tested]]\n"));
    assert_eq!(now(&kiln, "KILN-RAMP"), verified(), "at the entries' blobs");
    kiln.repo.replace(
        RAMP,
        "(now + rate).min(peak)",
        "now.saturating_add(rate).min(peak)",
    );
    assert_eq!(
        now(&kiln, "KILN-RAMP"),
        code_changed(),
        "another blob: parsed again"
    );
}

/// RD-3 ("a binding added or removed"): a `@verifies KILN-HOLD` marker
/// removed from `hold_follows_the_ramp` after the pass makes KILN-HOLD
/// `stale code_changed` (one binding fewer); KILN-RAMP, still bound there,
/// stays `verified`.
#[test]
fn binding_removed_stales_evidence() {
    let kiln = Kiln::new("th-removed");
    pass(&kiln);
    kiln.repo
        .replace(SCHEDULE, "// @verifies KILN-HOLD\n#[test]", "#[test]");
    assert_eq!(now(&kiln, "KILN-HOLD"), code_changed());
    assert_eq!(now(&kiln, "KILN-RAMP"), verified());
}

/// The median wall time of `runs` plain `spec verify --json` over `kiln`.
fn median_verify(kiln: &Kiln, runs: usize) -> std::time::Duration {
    let mut times: Vec<std::time::Duration> = (0..runs)
        .map(|_| {
            let started = std::time::Instant::now();
            let run = kiln.repo.spec_piped(kiln.root(), &["verify", "--json"]);
            let elapsed = started.elapsed();
            assert_ne!(run.code, 2, "{}", run.show());
            elapsed
        })
        .collect();
    times.sort();
    times[runs / 2]
}

/// docs/features/tested-evidence.md "Bound hash" (on a read, each tested
/// node's bindings hashed: the read's cost), a record, not a gate: over the kiln with
/// its four blocks (seven marker bindings over two files shared by the
/// nodes), then the same commit's lock without the blocks, the median of
/// seven plain `spec verify --json` runs each; counts and timings only.
/// Each file parsed once is not observable from outside the binary.
#[test]
#[ignore = "a record (8b-1 cost): run with --run-ignored ignored-only --no-capture"]
fn tested_read_cost_record() {
    let kiln = Kiln::new("th-cost");
    pass(&kiln);
    let lock = kiln.repo.lock_text();
    let blocks = tested_common::block_nodes(&lock).len();
    let with = median_verify(&kiln, 7);
    kiln.repo
        .write("spec.lock", tested_common::without_blocks(&lock));
    kiln.repo.commit_all("no block");
    let without = median_verify(&kiln, 7);
    eprintln!(
        "8b-1 read cost: {blocks} blocks: plain verify median {with:?}; without blocks {without:?}; ratio {:.2}",
        with.as_secs_f64() / without.as_secs_f64().max(f64::EPSILON)
    );
}
