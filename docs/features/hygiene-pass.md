---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-code, crates/specengine-store, crates/specengine-mcp]
ref: RM-0029; the requirement-analyst's analysis 2026-10-21 against df99db5; Q1-Q5 under the owner's pre-approval, review open; developer 1 iteration, review 1 accepted with nits
shipped: 2026-10-21
adrs: [ADR-0022, ADR-0008, ADR-0050]
---

# hygiene-pass (RM-0029): the review nits of 5b-1b, 5c and 8a-2

## Why

The "Later" sections of `lock-levels-verify.md`, `anchor-states.md`, `verify-task.md` (shipped) held review nits. Two were duplication that could drift into wrong output: the canonical-ID function twice and the node-by-canonical-ID loop three times, so `verify --task`'s "names no node" (verify-task Q2) could part from the judge's `node_gone`; the RON pairing copied in accept, verify and rebase. The rest: a clone per row on every judged side, stale doc lines and test data. One helper per duplicate, the text fixed, no output byte changed, proved against a binary built from the clean base commit df99db5.

## Description and interactions

A behaviour-free refactor of the CLI's verify and lock internals: no CLI, MCP, daemon, UI, plugin, gate, canon or README change; behaviour is every byte printed or written (stdout, stderr, exit, `spec.lock`, queue rows, daemon keys), not speed or allocation. Each item of the three "Later" sections is marked there: done, already gone (the UI mock's `ROW_KEYS`; `select_bindings`' group test), or left with its reason (`level_code`'s fingerprints, a range-keyed RON map, untested Q1 and Q3, the `merges` collection). Decided (Q1-Q5): the judge's moves taken, HY-1 guarding them; RM-0029's scope adds code, store, MCP; one pairing helper, no map; an `#[ignore]` harness, base binary against new, no goldens; leftovers stay in their "Later" sections, no new plan item.

## Acceptance criteria

Mutations each red once, then reverted.

- [x] HY-1 byte identity: six `identity_base.rs` tests, each `#[ignore]`, panicking with the base `spec` named when `SPECENGINE_BASE_SPEC` is unset; run with a `spec` built from the clean base commit. Both binaries take turns in a scratch git copy at one path (paths match unmasked), fixed `GIT_{AUTHOR,COMMITTER}_{NAME,EMAIL,DATE}`, a `HOME` each; compared: exit, stdout, stderr, `spec.lock`, `HEAD`; masked only `spec review --json`'s top-level `created_at`, `updated_at` (the queue clock), `drift` raw. Steps: (a) `fixtures/anchor-states` (`anchor_common::base`), `spec verify`, `--json`, `--staged`, `--node <node>`: the baseline and 18 `SCENARIOS` (89), the 18 one at a time (109); (b) `fixtures/code-a` with `data/limits.ron`: `spec lock accept` per bound node, then after a RON edit, a body edit and a node removal: `spec verify --json`, `spec lock diff <ID>`, `spec propose drift`, `spec review <PR> --json` (37); (c) code-a, a task missing rows and a dangling marker: `spec verify --task T`, plain and `--json` (82); (d) `spec lock rebase` over a RON entry and an alias marker (4x8); (e) `fixtures/verify-tests` (`kiln`), `tested_common::lock_with` blocks: `spec verify`, `spec show <ID> --bindings --json` (71). 420 steps byte-identical against iteration 1, run twice; base against a byte copy of base 6/6 (determinism). M3 applied: red, 94 differences in `code_a_accept_and_drift`, and `lock_rebase_over_ron_and_alias_entries`.
- [x] HY-2 gates (phase B): `cargo fmt --check`, `cargo check`, clippy `--workspace --all-targets -D warnings` clean; `cargo nextest run --workspace` 3 871 passed, 70 skipped; R3's source-reading suites green; eval `doc_pointers`, `anonymity`, `build_graph` 58/58; `spec export index`, `spec check` clean.
- [x] HY-3 tests kept: 3 974 `#[test]` names at df99db5, 3 980 now: none missing or renamed, the 6 new in `identity_base.rs`; the default `nextest list` identical (3 871).
- [x] HY-4 no leaks: `rg -n 'docs/|ADR' crates/specengine-cli/src` and `git diff --stat df99db5 -- fixtures ui crates/specengine-http docs/canon docs/decisions` empty (run 2026-10-21).
- [x] HY-5 one path each (run 2026-10-21): `rg -n 'fn node_id\b|_ => Unit::FileModule' crates/specengine-cli/src` empty; `rg -c '0\.\.parsed\.nodes\.len\(\)' crates/specengine-cli/src` only `node_set.rs:1`; `rg -n '\(range, _\)' crates/specengine-cli/src` only in `paired_value`; `rg -n package_label crates/specengine-{cli,code}`, `rg -nF 'levels\":[' crates/specengine-{core,store,mcp}/tests`, `rg -nF '["levels"], json!([' crates/specengine-{core,mcp}/tests` empty; review 1 read that `judge_gathered` owns `building`.
- [x] HY-6 mutations: M1 `canonical_nodes` returns an empty map: red in `verify_anchors` (26), `tested_reads` (2). M2 `canonical_of` returns the ID as written: red in `verify_task` (2) and HY-1 (4 of 6); narrowed from the draft's "and a `lock_rebase.rs` or `lock_diff.rs` test": rebase calls `canonical_of` only for an alias marker, and every fixture's alias is already canonical (the gap: "Out of scope"). M3 `paired_value` ignores the range: red in `lock_accept` (2), `ron_sibling_edit_is_ok` (`verify_cascade.rs`), `ron_value_sibling_and_deps` (`verify_levels.rs`), `lock_rebase` (20), HY-1. None for docs, test data, N3, N9.
- [x] HY-7 docs (2026-10-21): no ADR, canon or README line; the three "Later" sections marked in place; RM-0029 shipped, its scope widened; `CLAUDE.md`'s State; 08 s2 Phases 3 and 6.

## Out of scope

`level_code`'s two fingerprints (accept's digests by construction); a range-keyed RON map (speed; changes first-match); untested Q1, Q3 (new tests and a fixture); a rebase test of a scoped node bound under another spelling, M2's gap (a new test, not hygiene); `verify-node.md` "Later"; other specs' "Implementation" nits; a pilot run; any output, hash, JSON key, daemon key set or recipe.

## Implementation

| Module | What it does |
|---|---|
| CLI `lock.rs` | `paired_value(values, comment)`: a RON comment's value digest, the first match; called by accept, verify's `digests_of`, rebase |
| CLI `verify_rows.rs` | `judge_gathered` owns `building`: bound rows split into a `bound` map and a parallel `NodeSide` vector, lent to `select_bindings` and `cascade`; a `Merge` pre-pass computes merges while it is borrowed, then the map is consumed in key order, only a merged row's E entry cloned; an explicit `Target::Module { item: None, .. } \| Target::Ron { .. } \| Target::Keyless { .. }` arm; `canonical_nodes`, `node_id` gone |
| CLI `node_set.rs` | `canonical_nodes(corpus, scheme)` on `canonical_of`, for the judge, `Named::of`, `tested.rs` |
| CLI `tested.rs`, `lock_rebase.rs` | the shared map, `canonical_of`, `paired_value` |
| core `verify.rs`; code `lib.rs` | `LevelDigests`' field docs; the `dir_text` and `file_place` doc |
| tests | `identity_base.rs` (new, 6 `#[ignore]`); shared helpers `Scratch::at` (one path for both binaries), `program_with` (another binary), `spec_piped_with`, `spec_pty_with`, `LockRepo::of_in`, `anchor_common::based`, `Kiln::with_runner` made `pub`; N6's docs; N7's `levels` objects in `drift_stored.rs`, `queue_drift.rs`, `mcp_drift.rs` |

`src`: 7 files, +140/-123. As built: `select_bindings` and `cascade` keep only bound rows, so lending `bound` instead of all rows gives the same results (review 1, against the base). Review 1 accepted with nits, applied after the test round: `Named::any`'s doc, `tested.rs`' "first holder" comment, a `debug_assert_eq!` of `bound` and `bound_nodes`' lengths. Gates: phase B in HY-2; the final run after the nits: fmt, clippy (probes too) clean; nextest 3 871/3 871, 70 skipped; UI lint 0, `pnpm test` 2 546/2 546, build clean; export, check, parity clean; eval quartet 86/86; `identity_base` against the base 6/6.
