//! Helpers of the docs/features/tested-evidence.md (8b-1) tests over 8a's
//! repositories (`verify_tests_common::Kiln`: a scratch copy of
//! `fixtures/verify-tests` with the fake runner committed): the fixture's
//! bindings and tests as "Data" "Fixture" names them, the golden's blocks
//! computed from the spec, a passing run and what its JSON `lock` says.
//!
//! Only the 8b-1 test binaries over the kiln include this module (`mod
//! tested_kiln;`, after `mod common;`, `mod lock_common;`, `mod
//! verify_common;`, `mod verify_tests_common;` and `mod tested_common;`).
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

#![allow(dead_code)]

use std::path::Path;

use serde_json::Value;

use crate::tested_common::{Block, Bound, DAY, bound_hash, item, module};
use crate::verify_tests_common::{
    GLAZE, HOLD, KILN_MD, Kiln, RAMP, RAMP_STOPS, SCHEDULE, SCHEDULE_MODULE, TWIN_NOT_UNIX,
    TWIN_UNIX, TestsRun,
};

/// `next_temperature`'s qpath (`@implements KILN-RAMP`).
pub const NEXT: &str = "crates/kiln::ramp::next_temperature";

/// The fixture's own bindings of `node` (expected.json `nodes[].bindings`)
/// as `root`'s files are now: every relation, an alias marker under the
/// canonical ID, the module marker too.
pub fn kiln_bindings(root: &Path, node: &str) -> Vec<Bound> {
    match node {
        "CONE-01" => vec![
            item(
                root,
                node,
                "verifies",
                RAMP,
                TWIN_UNIX,
                "peak_is_capped",
                Some("test,unix"),
            ),
            item(
                root,
                node,
                "verifies",
                RAMP,
                TWIN_NOT_UNIX,
                "peak_is_capped",
                Some("test,not(unix)"),
            ),
        ],
        "KILN-HOLD" => vec![
            module(root, node, "verifies", SCHEDULE, SCHEDULE_MODULE),
            item(
                root,
                node,
                "verifies",
                SCHEDULE,
                HOLD,
                "hold_follows_the_ramp",
                None,
            ),
        ],
        "KILN-RAMP" => vec![
            item(
                root,
                node,
                "implements",
                RAMP,
                NEXT,
                "next_temperature",
                None,
            ),
            item(
                root,
                node,
                "verifies",
                RAMP,
                RAMP_STOPS,
                "ramp_stops_at_peak",
                None,
            ),
            item(
                root,
                node,
                "verifies",
                SCHEDULE,
                HOLD,
                "hold_follows_the_ramp",
                None,
            ),
        ],
        "glaze-check/GLZ-01" => vec![item(
            root,
            node,
            "verifies",
            SCHEDULE,
            GLAZE,
            "glaze_survives_the_hold",
            None,
        )],
        other => panic!("no fixture bindings for {other}"),
    }
}

/// The tests a passing full run lists in `node`'s block, by (file, symbol)
/// bytes (Data "Fixture").
pub fn kiln_tests(node: &str) -> Vec<(&'static str, &'static str)> {
    match node {
        "CONE-01" => vec![(RAMP, TWIN_NOT_UNIX), (RAMP, TWIN_UNIX)],
        "KILN-HOLD" => vec![(SCHEDULE, HOLD)],
        "KILN-RAMP" => vec![(RAMP, RAMP_STOPS), (SCHEDULE, HOLD)],
        "glaze-check/GLZ-01" => vec![(SCHEDULE, GLAZE)],
        other => panic!("no fixture tests for {other}"),
    }
}

/// The node's revision in the fixture (expected.json).
pub fn kiln_rev(node: &str) -> u32 {
    match node {
        "KILN-COOL" => 2,
        _ => 1,
    }
}

/// The node's document (expected.json).
pub fn kiln_doc(node: &str) -> &'static str {
    match node {
        "CONE-01" => "docs/records/CONE-01.md",
        "glaze-check/GLZ-01" => "docs/features/glaze-check.md",
        _ => KILN_MD,
    }
}

/// The blocks a passing run at `kiln`'s `HEAD` writes for `nodes`, built
/// from the spec: `rev` the fixture's, `spec_hash` `spec show`'s norm hash,
/// `bound_hash` over [`kiln_bindings`] of the files now, `tests`
/// [`kiln_tests`], `commit` `HEAD`, `verified_at` [`DAY`].
pub fn expected_blocks(kiln: &Kiln, nodes: &[&str]) -> Vec<Block> {
    let head = kiln.repo.head();
    nodes
        .iter()
        .map(|node| Block {
            node: (*node).to_owned(),
            rev: kiln_rev(node),
            spec_hash: kiln.repo.norm_hash(node),
            bound_hash: bound_hash(&kiln_bindings(kiln.root(), node)),
            tests: kiln_tests(node)
                .into_iter()
                .map(|(file, symbol)| (file.to_owned(), symbol.to_owned()))
                .collect(),
            commit: head.clone(),
            verified_at: DAY.to_owned(),
        })
        .collect()
}

// ----------------------------------------------------------------- runs

/// A passing run (every test passes, consent `y`) that must commit its
/// evidence: the run, the evidence commit.
pub fn pass(kiln: &Kiln) -> (TestsRun, String) {
    let before = kiln.repo.head();
    let run = kiln.run();
    let commit = kiln.repo.head();
    assert_ne!(commit, before, "the evidence committed:\n{}", run.show());
    assert_eq!(
        run.json()["lock"]["commit"],
        Value::String(commit.clone()),
        "{}",
        run.show()
    );
    (run, commit)
}

/// The run's JSON `lock` rows as `(node, change)`.
pub fn lock_rows(run: &TestsRun) -> Vec<(String, String)> {
    run.json()["lock"]["rows"]
        .as_array()
        .unwrap_or_else(|| panic!("`lock.rows` is an array:\n{}", run.show()))
        .iter()
        .map(|row| {
            (
                row["node"].as_str().expect("a node").to_owned(),
                row["change"].as_str().expect("a change").to_owned(),
            )
        })
        .collect()
}

/// stdout's `lock | <change> | <node>` lines.
pub fn lock_lines(run: &TestsRun) -> Vec<String> {
    run.lines()
        .into_iter()
        .filter(|line| line.starts_with("lock | "))
        .collect()
}

/// `(node, change)` pairs from `&str`s.
pub fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(node, change)| ((*node).to_owned(), (*change).to_owned()))
        .collect()
}

// ------------------------------------------------------------------ RON

/// The RON file a test adds to the kiln: a firing's tuning, its `peak`
/// bound to KILN-RAMP (`@configures`), `hold_minutes` its sibling.
pub const FIRING_RON: &str = "crates/kiln/data/firing.ron";

/// [`FIRING_RON`]'s bytes: the anchored comment on line 3.
pub const FIRING_TEXT: &str = "// Firing tuning.\nFiring(\n    peak: 1200, // @configures KILN-RAMP\n    hold_minutes: 60,\n)\n";

/// [`FIRING_RON`] written and committed.
pub fn add_firing(kiln: &Kiln) {
    kiln.repo.write(FIRING_RON, FIRING_TEXT);
    kiln.repo.commit_all("the firing's tuning");
}

/// `next_temperature` as rustfmt would never leave it: the same tokens on
/// one line.
pub const RAMP_SQUEEZED: (&str, &str) = (
    "pub fn next_temperature(now: u32, rate: u32, peak: u32) -> u32 {\n    (now + rate).min(peak)\n}",
    "pub fn next_temperature(now:u32,rate:u32,peak:u32)->u32{(now+rate).min(peak)}",
);

/// A comment far from every binding of `ramp.rs`.
pub const RAMP_COMMENT: &str = "\n// A note on ramps, bound to nothing.\n";

// ------------------------------------------------- 8b-2: the reads' corpus

/// The glaze criterion's sentence, and the same naming KILN-HOLD (a
/// mention: a layer-4 candidate of KILN-HOLD's bundle).
pub const GLAZE_TEXT: (&str, &str) = (
    "Hold at the peak for an hour: the temperature stays at the peak.",
    "Hold at the peak for an hour: the temperature stays at the peak. It keeps KILN-HOLD.",
);

/// The glaze criterion's document.
pub const GLAZE_MD: &str = "docs/features/glaze-check.md";

/// A block whose node no corpus holds (`node_gone` in plain verify; a
/// read's set never holds it).
pub const GONE: &str = "KILN-GONE";

/// The reads' corpus (docs/features/tested-reads.md RD-1): 8a's kiln with
/// KILN-RAMP's three markers and GLZ-01's pinned at `@1` (a raise makes
/// them lag: `unverifiable spec_ahead`), GLZ-01's text naming KILN-HOLD,
/// committed; then a lock of the four blocks a passing run writes there
/// ([`expected_blocks`]: every one `verified`) and a made-up block of
/// [`GONE`], committed as `the evidence`. No `[[binding]]` entry, as a
/// first run writes it.
pub fn evidence_kiln(label: &str) -> Kiln {
    with_evidence(Kiln::new(label))
}

/// [`evidence_kiln`] over a kiln made elsewhere (another root, order or
/// object format).
pub fn with_evidence(kiln: Kiln) -> Kiln {
    pin_and_mention(&kiln);
    kiln.repo
        .commit_all("pinned at 1, the criterion names KILN-HOLD");
    let mut blocks = expected_blocks(&kiln, &crate::tested_common::BLOCK_NODES);
    blocks.push(crate::tested_common::made_up_block(
        GONE,
        1,
        '9',
        &kiln.repo.head(),
    ));
    kiln.repo
        .write("spec.lock", crate::tested_common::lock_with(&[], &blocks));
    kiln.repo.commit_all("the evidence");
    kiln
}

/// [`evidence_kiln`]'s edits before its evidence (uncommitted).
pub fn pin_and_mention(kiln: &Kiln) {
    for (file, from, to) in [
        (
            RAMP,
            "// @implements KILN-RAMP\n",
            "// @implements KILN-RAMP@1\n",
        ),
        (
            RAMP,
            "    // @verifies KILN-RAMP\n",
            "    // @verifies KILN-RAMP@1\n",
        ),
        (
            SCHEDULE,
            "// @verifies KILN-RAMP\n",
            "// @verifies KILN-RAMP@1\n",
        ),
        (
            SCHEDULE,
            "// @verifies glaze-check/GLZ-01\n",
            "// @verifies glaze-check/GLZ-01@1\n",
        ),
        (GLAZE_MD, GLAZE_TEXT.0, GLAZE_TEXT.1),
    ] {
        kiln.repo.replace(file, from, to);
    }
}

/// One state of the reads' corpus, written into its worktree over the
/// evidence (uncommitted): each a block state of
/// docs/canon/spec-lock-tested.md "The state".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceState {
    /// Every block `verified` (GONE's `node_gone`).
    Verified,
    /// KILN-RAMP's and GLZ-01's texts edited: `stale spec_changed`.
    SpecChanged,
    /// `next_temperature` and the glaze test edited: KILN-RAMP and GLZ-01
    /// `stale code_changed`.
    CodeChanged,
    /// KILN-RAMP and GLZ-01 raised to 2, their markers at `@1`:
    /// `unverifiable spec_ahead`.
    Unverifiable,
    /// One of each: KILN-HOLD's shared test edited (KILN-HOLD and
    /// KILN-RAMP `stale code_changed`), CONE-01's text edited (`stale
    /// spec_changed`), GLZ-01 raised (`unverifiable spec_ahead`).
    Mixed,
}

/// Every [`EvidenceState`], `Verified` first.
pub const EVIDENCE_STATES: [EvidenceState; 5] = [
    EvidenceState::Verified,
    EvidenceState::SpecChanged,
    EvidenceState::CodeChanged,
    EvidenceState::Unverifiable,
    EvidenceState::Mixed,
];

impl EvidenceState {
    /// `(file, from, to)` edits of the evidence kiln's committed files.
    pub fn edits(self) -> Vec<(&'static str, &'static str, &'static str)> {
        let ramp_text = (
            "The temperature rises by the ramp rate each hour and never passes the peak.",
            "The temperature rises by the ramp rate each hour and stops at the peak.",
        );
        let ramp_raise = ("## Ramp {#KILN-RAMP}", "## Ramp {#KILN-RAMP rev=2}");
        let glaze_raise = (
            "### The glaze survives the hold {#GLZ-01}",
            "### The glaze survives the hold {#GLZ-01 rev=2}",
        );
        match self {
            Self::Verified => Vec::new(),
            Self::SpecChanged => vec![
                (KILN_MD, ramp_text.0, ramp_text.1),
                (
                    GLAZE_MD,
                    "the temperature stays at the peak.",
                    "the temperature holds at the peak.",
                ),
            ],
            Self::CodeChanged => vec![
                (
                    RAMP,
                    "(now + rate).min(peak)",
                    "now.saturating_add(rate).min(peak)",
                ),
                (
                    SCHEDULE,
                    "assert_eq!(next_temperature(1200, 0, 1200), 1200);",
                    "assert_eq!(next_temperature(1200, 1, 1200), 1200);",
                ),
            ],
            Self::Unverifiable => vec![
                (KILN_MD, ramp_raise.0, ramp_raise.1),
                (GLAZE_MD, glaze_raise.0, glaze_raise.1),
            ],
            Self::Mixed => vec![
                (
                    SCHEDULE,
                    "assert_eq!(next_temperature(1150, 100, 1200), 1200);",
                    "assert_eq!(next_temperature(1150, 50, 1200), 1200);",
                ),
                (
                    "docs/records/CONE-01.md",
                    "the kiln stops at the cone's rated temperature.",
                    "the kiln halts at the cone's rated temperature.",
                ),
                (GLAZE_MD, glaze_raise.0, glaze_raise.1),
            ],
        }
    }

    /// `(node, state, category)` plain verify gives the blocks this state
    /// aims at (a premise; the reads' oracle is plain verify itself), by
    /// node bytes: [`GONE`]'s always `node_gone`.
    pub fn aimed(self) -> Vec<(&'static str, &'static str, Option<&'static str>)> {
        let verified = |node| (node, "verified", None);
        let stale = |node, category| (node, "stale", Some(category));
        let ahead = |node| (node, "unverifiable", Some("spec_ahead"));
        let gone = (GONE, "node_gone", None);
        match self {
            Self::Verified => vec![
                verified("CONE-01"),
                verified("KILN-HOLD"),
                verified("KILN-RAMP"),
                gone,
                verified("glaze-check/GLZ-01"),
            ],
            Self::SpecChanged => vec![
                verified("CONE-01"),
                verified("KILN-HOLD"),
                stale("KILN-RAMP", "spec_changed"),
                gone,
                stale("glaze-check/GLZ-01", "spec_changed"),
            ],
            Self::CodeChanged => vec![
                verified("CONE-01"),
                stale("KILN-RAMP", "code_changed"),
                gone,
                stale("glaze-check/GLZ-01", "code_changed"),
            ],
            Self::Unverifiable => vec![
                verified("CONE-01"),
                verified("KILN-HOLD"),
                ahead("KILN-RAMP"),
                gone,
                ahead("glaze-check/GLZ-01"),
            ],
            Self::Mixed => vec![
                stale("CONE-01", "spec_changed"),
                stale("KILN-HOLD", "code_changed"),
                stale("KILN-RAMP", "code_changed"),
                gone,
                ahead("glaze-check/GLZ-01"),
            ],
        }
    }

    /// The edits written into `kiln`'s worktree.
    pub fn write(self, kiln: &Kiln) {
        for (file, from, to) in self.edits() {
            kiln.repo.replace(file, from, to);
        }
    }

    /// The edits taken back: the worktree clean again.
    pub fn undo(self, kiln: &Kiln) {
        for (file, from, to) in self.edits().into_iter().rev() {
            kiln.repo.replace(file, to, from);
        }
        assert!(
            kiln.repo.porcelain().is_empty(),
            "{self:?}: the tree is clean again"
        );
    }
}

/// An evidence kiln's twin: its lock without the blocks (format 2, no
/// entry), committed as `no evidence`.
pub fn without_evidence(kiln: &Kiln) {
    kiln.repo.write(
        "spec.lock",
        crate::tested_common::without_blocks(&kiln.repo.lock_text()),
    );
    kiln.repo.commit_all("no evidence");
}
