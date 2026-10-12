---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-code, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 3 (WA-1..WA-16; Q1-Q4 and the review's points decided under the owner's pre-approval, review open), Q4 from `spec-verify.md` Q3; 05 s3.5, s5.3; 06 s4, s6; 07 s2; 08 s2; Rust 3 iterations, review accepted
shipped: 2026-10-12
adrs: [ADR-0041]
---

# spec-lock: what was verified, written on the owner's terminal

## Why

Nothing tied a node's `norm_hash` at revision R to its bound items' AST hashes at a commit: slice 4 could not tell `ok` from `code_ahead` (05 s5.3), slice 8 had nowhere to write a passing run (06 s6), and "rewrote the meaning, coverage stayed green" (05 s3.5) could not be ruled out. Delivered: a canonical, committed `<root>/spec.lock`; `spec lock accept REF [--editorial]` (committed bytes, one verified commit with provenance each) and `spec lock regen` (a merge resolver, no hashing). ADR-0041 names the writers (`architecture.md#apply`, `architecture-code.md#spec-lock`). Built on slices 1 (`code-refs.md`) and 2 (`norm-hash.md`).

## Description and interactions

Canon: `docs/canon/spec-lock.md`, `spec-lock-accept.md`, `architecture-code.md#spec-lock`; the core, store, CLI, code READMEs; `plugin.md`, `plugin-files.md` (0.7.1). An agent never writes the lock: no MCP tool, route or UI stage (WA-12; a `drift` item's approve runs this accept: ADR-0045, `drift-queue.md`); the plugin's roles and `feature` skill say "or the lock" (WA-16). Not a spec file (WA-1). Unchanged: plain `spec check`, hook triggers, CI, `INDEX_FORMAT`, MCP, HTTP, UI; no new crate or dependency; this repository has no `[code]`, so no lock (WA-15). Spelling `spec lock regen`, not 05's `--regen`: owner-confirmed writes are verb subcommands with their own exits and `require_terminal` name; a bare `spec lock` is a usage error. Live-spec lines fixed: 05 s3.5 (`owner | ci`, "Generated", `--regen`, a format pointer, the lock half of `bump --editorial`), 06:121, 06:140, 07:106, 08:41, 08:44; left: 05:20's diagram, 07:105 and the rest of 05:103 (`bump`, slice 4b).

## Data

### The file

`docs/canon/spec-lock.md` "The file" (WA-4).

### Entry

`docs/canon/spec-lock.md` "Entry" (WA-5): keys, order, the reader, the key rules.

### Lockability

`docs/canon/spec-lock.md` "Lockability" (WA-6, Q2).

### spec lock accept

`docs/canon/spec-lock-accept.md` "Steps", "The lock commit" (WA-2, WA-3, WA-7, WA-9, WA-10, Q1).

### --editorial

`docs/canon/spec-lock-accept.md` "`--editorial`" (WA-8).

### A gone node

`docs/canon/spec-lock-accept.md` "A gone node" (Q4).

### spec lock regen

`docs/canon/spec-lock.md` "`spec lock regen`" (WA-11).

### Exits

`docs/canon/spec-lock-accept.md` "Exits"; regen's in its section (WA-14).

## Rules and edge cases

`docs/canon/spec-lock.md` "The file", "Travel and writes": one commit, one lock from any root, cwd, order, line ending or alias; no position, identity or time of day; writes only the data directory, `spec.lock` and one commit; nothing blocked (ADR-0012).

## Acceptance criteria

Core `tests/lock_model.rs` (12); CLI `lock_accept.rs` (22), `lock_commit.rs` (22), `lock_exits.rs` (23), `lock_regen.rs` (9), `lock_pilot.rs` (1, `#[ignore]`), helpers `lock_common/`; golden `fixtures/spec-lock/code-a.lock` (11 entries). Pins rewritten: code `hash_traps.rs`, `code_bind.rs` (`NotHashed`); CLI `code_refs_walk.rs` (the lock reads no item `.state`, hashes only through `hash_bound_items`); MCP `plugin_{files,roles,skills}.rs` (0.7.1). 52 mutation runs, 51 red (M08: "Implementation").

rust-developer:

- [x] RD-1 -- `model_and_core_sources_do_no_file_io`.
- [x] RD-2 -- `the_golden_renders_back`, `a_respaced_equivalent_renders_back`, `lock_entries_are_sorted_by_key`, `lock_bytes_are_lf_with_a_final_lf`.
- [x] RD-3 -- `lock_unknown_key_is_refused`, `the_reader_refuses_every_wrong_form` (25 cases at their lines), `another_builds_recipes_are_refused_naming_both`, `escaped_symbols_read_back`.
- [x] RD-4 -- `not_a_terminal_wins_over_a_bad_ref`.
- [x] RD-5 -- `lock_hashes_the_committed_blob`.
- [x] RD-6 -- `dirty_bound_file_refuses_accept`, `every_dirty_input_is_named` (10 states), `every_changed_code_file_and_manifest_is_dirty` (untracked, manifests dropped: red).
- [x] RD-7 -- `cannot_verify_binding_is_never_locked`, `marker_behind_the_revision_is_not_locked`, `ambiguous_binding_is_not_locked`, `ron_binding_is_locked_by_blob_only`.
- [x] RD-8 -- `meaning_change_without_a_revision_is_refused`, `other_nodes_entries_are_untouched`.
- [x] RD-9 -- `second_accept_changes_nothing`.
- [x] RD-10 -- `editorial_keeps_code_hashes`, `editorial_after_a_revision_raise_is_refused`.
- [x] RD-11 -- `lock_commit_runs_hooks`, `lock_commit_takes_only_the_lock`; the outcome table: ten `lock_commit.rs` tests, one per check, row and the restore guard (`a_hook_that_rewrites_the_staged_lock_fails_the_accept`, `a_commit_off_its_branch_is_unverified`, …).
- [x] RD-12 -- `lock_changed_while_accepting_is_refused`.
- [x] RD-13 -- `regen_is_symmetric`, `regen_reads_no_code_and_no_spec`, `regen_without_a_conflict_is_exit_1`.
- [x] RD-14 -- `build_graph.rs` `cli_direct_dependencies_are_all_workspace_entries` (a `sha1` dependency: red).
- [x] RD-15 -- `lock_accept_writes_no_queue_row`.
- [x] RD-16 -- "or the lock" in the five roles and `feature`; `plugin.json` 0.7.1, `PINS` `fc9423b7…1902`; `plugin_files` ac10.
- [x] RD-17 -- `gone_node_entries_are_dropped`, `alias_of_a_gone_id_drops_its_entries`, `a_node_deleted_only_in_the_worktree_is_not_gone`, `a_gone_id_with_a_revision_drops_its_entries`.

test-engineer:

- [x] TE-1 -- `golden_lock_of_code_a` (every hash recomputed independently), `golden_accepts_ask_and_commit_once_each`, `accept_on_a_terminal_commits_the_lock`.
- [x] TE-2 -- `every_row_of_code_a_is_judged_by_lockability`, `ambiguous_and_look_alike_ids_are_not_locked`, RD-7's four.
- [x] TE-3 -- `lock_exits.rs`: every exit 1 and 2 cause; after each, HEAD, refs, index, `status --porcelain=v1 -z --untracked-files=all` and files unchanged.
- [x] TE-4 -- `one_commit_gives_one_lock` (M: a path in a hash input), `a_sha256_repository_gives_the_same_hashes`.
- [x] TE-5 -- `lock_has_no_positions`.
- [x] TE-6 -- the five `editorial_*` tests.
- [x] TE-7 -- `regen_merges_every_case` (11 cases after a real `git merge`), `regen_resolves_whole_file_conflicts`, `non_adjacent_blocks_merge_cleanly`, `regen_refuses_other_headers_and_unreadable_stages`; four more in `lock_regen.rs`.
- [x] TE-8 -- `lock_commit_trailers_parse` (a caller's `Proposal:` too), `a_failing_hook_restores_the_bytes`, `lock_commit_runs_hooks`.
- [x] TE-9 -- `plain_check_never_reads_the_lock`; `.githooks/pre-commit` untouched.
- [x] TE-10 -- `no_code_table_is_exit_2_naming_it`, `the_core_lock_module_knows_no_domain`.
- [ ] TE-11 -- `lock_pilot.rs` `pilot_rows_by_state` (`#[ignore]`, release, a scratch clone, consent declined): manual, owner; not run.

spec-writer:

- [x] SW-1 -- this spec.
- [x] SW-2 -- ADR-0041, `architecture.md` `#apply` and `#layout` in the same change.
- [x] SW-3 -- `docs/canon/spec-lock.md`, `spec-lock-accept.md` (new); `architecture-code.md`, `plugin.md`, `plugin-files.md`, `code-bindings.md`, `code-identity.md`, `node-revision.md`; core, store, CLI, code READMEs; 05, 06, 07, 08 lines above; `CLAUDE.md`.
- [x] SW-4 -- `export index`, `check` clean.

## Out of scope

Slice 4a (`spec-verify.md`, ADR-0042): comparing, lock findings, orphans. 4b `rev-discipline` (ADR-0043): the revision rule, `spec bump`, an editorial apply writing the lock in its commit. 4c `drift-queue` (ADR-0045): a `drift` kind whose approve calls `lock_accept` with its consent and `Proposal:` trailer. 5: levels, fingerprints, the RON recipe; format 2 and `spec lock rebase` (a new ADR); anchors. 6/7: lock data in reads. 8a `verify-tests` (ADR-0044); 8b `[[tested]]` in the next format. Phase 4: history diffs. Not planned: a merge driver, a `spec.lock` gate rule, several REFs, UI staging, a queue event.

## Open

Decided under the owner's pre-approval, review open: Q1 the lock alone, from a clean HEAD, hooks run; Q2 RON and modules by `file_blob` until slice 5; Q3 writers by consent; Q4 `accept <gone ID>` drops its entries; WA-1..WA-16. From the review: strict dirty inputs (every changed or untracked walked `.rs`/`.ron`, every `Cargo.toml` at or above a root); gone only when HEAD agrees; the fourth post-commit check (committed bytes = confirmed bytes); success only a verified commit; known limits: git-ignored walked files, `id_state` on the working-tree corpus. Working answers, pinned, now canon: a header-only lock, a non-UTF-8 blob exit 2, row order, the four counts, regen's recipes and `deleted`, a `git mv` holder.

## Implementation

| Module | What it does |
|---|---|
| core `lock.rs` (new), `lib.rs` | `parse_lock`, `render_lock`, `key_problem`/`KeyProblem`, `merge_accept`, `refresh_editorial`, `drop_node`, `merge_stages`; pure |
| store `worktree.rs`, `lib.rs` | reads `blob_oid` (`:(literal)`), `unmerged_stages`, `status_entries(paths, untracked)` -> `StatusEntry`, `is_ignored`; one write `reset_path` (`git reset -q -- :(literal)<path>`, scrubbed environment, no `GIT_OPTIONAL_LOCKS`); `commit_of`, `blob_at` reused at step 10 |
| CLI `lock.rs`, `lock_regen.rs` (new) | `lock_accept` (accept, editorial, gone; a `LockConsent` closure, `LockTrailer`s), `lock_regen`; `Check::{Ours, NotAlone, Unverified}` |
| CLI `main.rs`, `lib.rs`, `refs.rs`, `code_walk.rs` | `lock accept|regen`, `show_and_ask`; `Outcome::{Lock, LockRegen}`; state constants, `MANIFEST`, `code_config` shared `pub(crate)` |
| code `hash.rs`, `items.rs`, `bind.rs`, `lib.rs`; eval `ast_hash` | `HashState::NotHashed` for the binder's error-free items, `hash_bound_items`, `MOD_KIND` exported |
| plugin | five roles, `feature`: "or the lock"; 0.7.1 |

Deviations, accepted in review. Iteration 1: four store reads, not two (state letters name each dirty input; `check-ignore` for a first lock); "kept" only with a key (a Rust `cannot_verify` keeps entries naming a now-broken item of its file; `unanchored`, RON `cannot_verify` drop); one key from several markers lockable only if all allow; exit 2 when HEAD's blob gives other items than the working tree's (names come from the walk); `--config` naming another file exit 2, `--editorial` on a gone ID exit 1; the consent closure gets the rows (stdout) before the question (stderr); `NotHashed` a new variant, as `Option<HashState>` broke more callers (code-refs' carry-over); the skill omits "(spec.lock)" for its 4 096 B body cap. Iteration 2 (the review's major: success without a check): the commit verified as `spec approve` verifies; `key_problem` shared, after a TAB path bricked the lock; step 9's exact text; the data directory before the prompt; 5(a) replaces "a holder deleted or renamed"; 5(b) over any changed `.md`; `blob_oid` literal; a gone ID's `@rev` stripped. Iteration 3: the fourth check and the restore guard (a hook rewriting the staged lock; a concurrent same-node accept; restoring over a foreign lock); advice `<commit>^`, not the old HEAD (a foreign commit underneath); the commit found through `refs/heads/<branch>`, as approve; `reset_path` where the branch did not move after git said made; branch unmoved but HEAD moved: unverified, nothing restored (the worktree may sit elsewhere); a failed commit whose moved branch cannot be verified: left as written (it may be this run's); a parser panic at HEAD exits 2.

Carried to slice 4a's developer: `lock.rs:1810-1815` "changed meanwhile" omits the index entry left (a first lock's intent-to-add, or the confirmed bytes staged); `:1701-1706` a failed commit with an unreadable branch is restored, not "unverified, left as written" (practically unreachable); `:1845` "its bytes restored" even when the restore failed; `:1779` comment wrong for a commit deleting `spec.lock` (`Unverified`, still exit 1); `store/worktree.rs:26-28` says every command but `commit` gets `GIT_OPTIONAL_LOCKS=0` ("every read"); `lock.rs:41-52` step 6 omits the two iteration-3 outcomes. Pointers still citing this spec's headings, to retarget to the two canons: core `lock.rs:1`, `lib.rs:56`; store `worktree.rs:11`; CLI `lib.rs:27,120`, `lock.rs:1-8` (also "task spec `drift-queue`"), `lock_regen.rs:1`. Test gaps: M08 survives (RD-6 enforced twice, `inputs` and `changed_code`: either alone can regress unseen; both mutated, red); unpinned: a parser panic at HEAD, two step-10 rows, `--json`.

Runs (iteration 3): fmt, clippy (workspace, MCP probes) clean; nextest code, core, store, CLI, MCP 1 855 passed, 1 skipped (`lock_pilot`); eval `build_graph`, `doc_pointers` 42/42; `spec check` clean.
