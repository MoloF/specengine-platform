---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-core, crates/specengine-ra]
ref: Phase 3 plan 2026-10-10, slice 11a-2, split from 11a `layer-c.md` 2026-10-19 against 5b79b81 (drift 3-31; Q2-Q5, Q11 2026-10-10 under the owner's pre-approval; L3, L6 owner pre-approved 2026-10-19; review open); refreshed 2026-10-19 against 11a-1 as built, dc26973 (the analyst's delta; L7-L10 owner pre-approved 2026-10-19, review open); L11 owner pre-approved 2026-10-19 with review 1, review open; 05 s5.1; 07 s2, s5; 08 s2; developer 2 iterations, review 1 accepted
shipped: 2026-10-19
adrs: [ADR-0049]
---

# layer-c-resolve (11a-2): `spec resolve` and the layer C snapshot

## Why

11a-1 (`layer-c.md`, shipped) builds `specengine-ra`, which answers a moniker or a state per item; protocol v1 as built is `docs/canon/layer-c-protocol.md`. Here `spec resolve` becomes the one command that spawns it: it checks the reply and stores it as one advisory snapshot per worktree, with a library state for 11a-3 (`layer-c-reads.md`) to show after the owner's TE-6. No read changes in this part. ADR-0049.

## Description and interactions

Delivered: `spec resolve [--timeout S] [--json]`, one cold load, the only spawner of `specengine-ra` (WA-3: no read, MCP tool, daemon route, hook, gate, `index`, `check`, `verify`, `lock` or `propose drift` spawns it, and a snapshot changes none of their outputs); the three snapshot tables, `INDEX_FORMAT` 11; `layer_c_state` for 11a-3; the process-group core shared with `verify --tests` (A2); `scip` retired; R13 in `specengine-ra`. No HTTP, UI, MCP, plugin, hook or CI change; `daemon_keys.rs` stays at 56 sets. Writes: the data directory only. How it works now: `docs/canon/layer-c.md`; the protocol: `docs/canon/layer-c-protocol.md`. Live-spec lines rewritten at shipping: 05:123; 07:111, :209; 08:41, :43.

## Data

As built is canon: `docs/canon/layer-c.md` "Run", "Request", "Stdin, stdout, stderr", "Checks", "Output and exits", "Tables", "Freshness", "Join". Below, what the build refined against this spec's draft: deviations 1-12 and L11, each accepted in review 1.

### spec resolve

WA-1, WA-2, Q1-Q3, L3, L6, L7: canon "Run", "Request". `files`: the code index's `rust` rows read and UTF-8, the walk's set and byte order (deviation 7). A program found through a relative `PATH` entry is made absolute against `spec`'s cwd before the spawn (review 1, iteration 2). The timeout is checked by clap and again by the library, for bridges (deviation 12). Step 6: only a walk gap or another fingerprint is `the tree changed during the run`, its refresh's notes dropped (deviation 8).

**Group core** (A2): `group.rs`, out of `test_runner.rs`: `GRACE`, `Forwarding`, `spawn` (`process_group(0)`), `wait(child, timeout, &Forwarding, stop) -> Ended`, the caller's stop a third trigger, the first trigger the outcome; `verify --tests` keeps its I/O and bytes (`test_runner.rs` maps `group::Ended` into its own); the CLI's only `unsafe`.

**Stdin, stdout, stderr**: canon. stderr is inherited, not copied by a thread (deviation 9); risk: under `stty tostop` a background run stops until S ends it.

**Checks**: canon "Checks". An `error` after `file` lines is accepted (deviation 3); `protocol` of `hello` and `refused` is read first wherever the line stands (4); categories closed to `metadata`, `database`, `path`, `build_scripts` malformed, item `end` unchecked (5); a repeated JSON key folds (review 1, accepted).

### Tables

Q4, Q5, A3: canon "Tables", as specified. `replace_layer_c` refuses without a `code_worktrees` row (`NotIndexed`); readers `layer_c_stamp`, `layer_c_snapshot`, `code_tree_rows`.

### Freshness

WA-5: canon "Freshness". `tree_fp` hashes the bytes of each of `Cargo.lock`, `Cargo.toml`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml` (`TREE_FILES`, six) in the root and each directory from a walked file's (any `code_files` row) up to it, never above (a path with `..`, `.`, an empty part, or absolute adds the root only); deduplicated, byte-sorted (L11, iteration 2: a virtual workspace root's manifest is watched; L8's guard stays). The encoding (deviation 7): JSON-array entries, unique, byte-sorted, `\n`-joined, `b3:`; an unreadable tree file is an entry, a directory at such a name skipped. `layer_c_state` is public and refreshes both indexes first (deviation 11); `state_in` serves 11a-3 inside its read's refresh.

### Join

Canon "Join". `collisions` counts groups (deviation 6). `agrees`' A side comes from stored rows: the package's `targets` through `TargetIndex::file_role`, without a unit the primary target's name (`TargetIndex::primary()` added), the modules plus the enclosing inline `mod` names by `parent_ord`; `path_attribute`, `unrooted` rows null (deviation 10).

Per read (11a-3, `layer-c-reads.md`), a 6a row's `c_state` (WA-4), null while `absent`:

| `c_state` | When |
|---|---|
| `unseen` | file not in `code_resolve_files`, or its BLAKE3 changed (offsets moved) |
| `moniker`, `local`, `none`, `unresolved`, `not_loaded` | a row with equal `(path, start, end)` where `kind_matches` holds |
| `unmatched` | file seen unchanged, no such row |

### Output and exits

Canon "Output and exits". Beyond the draft's reasons: before the spawn `cannot create`, `open`, `lock`, `resolve <path>: <e>`, a non-UTF-8 path, `cannot write the request: <e>` (deviation 1); after it `cannot store the snapshot: <e>`, `specengine-ra cannot be waited for: <e>` (2). The counts failing after a committed store: exit 2, stdout empty (review 1, accepted).

### Config

Q11: `scip` retired as specified: `NOT_BUILT_KEYS` `["languages", "crates"]`, the unknown-key text, `code_refs_walk.rs` `EXEMPT` 3 (`code-bindings.md`). The root README has the install line.

## Rules and edge cases

- WHEN the binary is missing, hangs, crashes, overruns, refuses or is interrupted, other commands SHALL be unchanged and `resolve` SHALL keep the previous snapshot.
- WHEN the reply is not yet at its last line, `spec` SHALL keep the binary's stdin open; WHEN the binary never reads the request, S SHALL still end the run.
- WHEN any `tree_fp` input changes after a run, `layer_c_state()` SHALL be `stale`; WHEN one tree is resolved from two roots, twice or in reversed creation order, the layer C dump SHALL be byte-identical.
- Genre (`architecture.md#universal`): Cargo file names, the kind map, descriptor reading and collisions live in `specengine-code`; no SCIP or Cargo literal in store, core or the CLI (R10 names them). Protocol v1's keys, line types, categories and the install line are the CLI's own data (precedent: `serve.rs` `no_binary`). A moniker never binds, locks, judges or becomes a finding.

## Acceptance criteria

M: mutation -> the test turning red. CLI `layer_c.rs`, under `alarm`; CLI tests never build `specengine-ra` (A6). A fake: an executable named `specengine-ra` beside a copy of `spec` in a temp dir; it reads and logs the request line, writes its reply and exits with the ending's code. By default it replays `fixtures/layer-c/ra-mini.jsonl` (stamps masked: `host`, `toolchain` "masked"; `ms`, `peak_rss`, `sysroot_packages` 0) on a scratch copy of `fixtures/ra-mini` plus a `specengine.toml` whose `[code]` walk lists exactly its six `.rs` files. Files: CLI `tests/layer_c.rs` (29, TE-2 ignored), `layer_c_state.rs` (8), `layer_c_outputs.rs` (2), `layer_c_scan.rs` (3), helpers `layer_c_common`; MCP `tests/mcp_layer_c.rs` (2). Every criterion met; round 1's mutations M1-M8 all red.

- [x] X0 -- `git diff --stat 5b79b81 -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing; `Cargo.lock` equals 11a-1's (dc26973). Both empty after iterations 1 and 2.
- [x] R1 -- M: `PATH` first -> `layer_c_binary_beside_spec_wins_over_path`; missing, exit 0 -> `resolve_without_binary_is_exit_2_with_install_line` (stdout empty); a relative `PATH` entry, nothing beside `spec`: exit 0 (review 1) -> `relative_path_entry_finds_the_binary_from_the_cwd`; step 1 and the timeout bounds -> `resolve_refuses_before_the_spawn`. Green.
- [x] R2 -- M: `error`, `refused` or exit 3 taken as success -> `every_ending_but_done_and_exit_0_keeps_the_snapshot`, one case per row of "Output and exits"' table but the first (R4) (exit 2, the row's reason, stdout empty, the layer C dump byte-identical): `error` with workspace 1 and with 0, `refused` protocol 1, exit 3 with no line, a signal, `done` then exit 1, a `#!` to a missing interpreter; M: a check stricter than the canon -> `protocol_v1_variants_are_not_malformed` (`dir` `..` with null `toolchain` and `peak_rss`: stored, `toolchain unknown`, `peak unknown`; `error` N with no step N, `error` 0 right after `hello`: `load failed`); M: no version check, or keys checked before `protocol` -> `protocol_mismatch_is_exit_2_and_stores_nothing` (`hello` protocol 2; `refused` protocol 2 plus an extra key); any one check off -> `malformed_reply_is_exit_2_and_stores_nothing` (each case its line `<n>`, a line after `done` among them); no cap -> `oversized_reply_line_is_exit_2`. Green, 33 malformed cases; M4 (`error` and exit 1 taken as `Ok`) and M6 (no cap) red.
- [x] R3 -- M: only the child killed -> `no_layer_c_grandchild_outlives_the_budget` (a grandchild ignoring `SIGTERM`, `--timeout 2`: exit 2, no survivor); default SIGINT disposition -> `interrupted_resolve_kills_the_group_and_keeps_the_snapshot`. Green; M3 (`killpg` -> `kill`) red.
- [x] R4 -- M: rows written while streaming -> `budget_overrun_keeps_the_previous_snapshot` (a crash mid-stream too); no re-hash -> `tree_change_during_run_stores_nothing`; no lock -> `second_resolve_is_refused_while_one_runs`. Green; M2 (no re-hash) red.
- [x] R5 -- M: `symbols` spawns -> `reads_mcp_check_gate_verify_never_spawn_layer_c`: a logging fake beside the copy and on `PATH` stays empty across `health`, `unmapped`, `symbols`, `refs`, `bundle`, `index`, `check`, gate, `verify` (plain, `--staged`, `--tests`), `lock accept`, `propose drift`, and the MCP code tools from `mcp_code.rs`'s harness; a call from `health` -> `only_resolve_spawns_layer_c` (a source scan over every crate's `src/`: the spawn's one caller, the daemon route included). Green, the MCP half in `mcp_layer_c.rs`; M1 (`symbols.rs` calling resolve) turned both red. Uncovered: the daemon route runs no live `GET health` (the scan only); `verify --tests` stops at its refusal (no `[verify] tests`).
- [x] R6 -- `layer_c_state` fresh after a run, then M: per-file freshness -> `layer_c_state_is_stale_after_another_file_edit`; manifests out -> `manifest_edit_makes_the_snapshot_stale`; L11 -> `tree_files_from_each_walked_dir_up_to_the_root_are_inputs` (`layer_c_state.rs`): a virtual root's `Cargo.toml` edit (`members` reordered, `[workspace.dependencies]`) turns it stale; one out -> `lock_toolchain_or_cargo_config_edit_makes_the_snapshot_stale` (each of `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`); `--full` drops -> `full_reindex_keeps_the_snapshot`; no cascade -> `clear_code_drops_the_snapshot`. Green: `layer_c_state.rs` (`virtual_workspace_root_edit_makes_the_snapshot_stale`: `[workspace.dependencies]`, `[patch]`, a `members` reorder; cargo files between a walked file and the root; `unreadable_tree_file_is_an_input_and_a_directory_is_not`), the dump half in `layer_c.rs`.
- [x] R7 -- M: kept at 10 -> store `format_history_ends_with_the_current_format_and_dump_hash` (`11 <hash>`); outside `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`, covering the three; `CODE_INDEX_FORMAT` 4, `code_format_history.txt` unchanged. Green: `format_history.txt` `11 1faa605a…`, store `layer_c_pins_format_11_with_one_new_history_line`, CLI `ac16_index_format_is_eleven`, `format_change_drops_the_snapshot`.
- [x] R8 -- `golden_replay_gives_the_pinned_counts`: measured joined 29, unmatched 0, `c_only` 0, agrees 26, disagrees 0, collisions 1, as expected; text (timings masked) and every JSON key as in Data. M: a `mod` row's own `name/` compared (L10), or no `-` -> `_` -> that test. M: by line -> `attributed_item_joins_its_row`; by name -> `same_name_items_join_by_offset`; `end` ignored, kinds ignored -> `boundary_mismatch_is_unmatched_and_c_only` (hand-written replies: one item's `end` off by one byte; one item's `kind` not matching: unmatched 1, `c_only` 1 each); offsets from normalised text -> `crlf_file_joins_by_raw_bytes`. Green, the counts as expected.
- [x] R9 -- M: hash-map order -> `resolve_rows_are_byte_identical_across_runs_and_roots` (two roots, two runs, reversed creation; TE-4; the dump's `w.root` masked). Green.
- [x] R10 -- `no_scip_or_cargo_literal_outside_code_and_ra`, a string- and char-literal scan of store, core and CLI `src/` (`#[cfg(test)]` modules excluded): none holds `Cargo.lock`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml`, `build-script-build`, `build_script_build`, nor `Cargo.toml` but 6a's `code_walk.rs` `MANIFEST`; the slice's new CLI and store modules hold no literal equal to `/`, `#`, `.`, `!`, `[`, `]`, `().` or holding `impl#[`. M: `agrees` split at `/` in the CLI; a `Cargo.lock` literal in the CLI. Green, the scanner self-tested (`the_scan_reads_literals_and_skips_comments_and_test_modules`).
- [x] R11 -- M: `scip` kept in `NOT_BUILT_KEYS` -> `code_table_unknown_key_is_refused` case 13 (`scip = false`); `EXEMPT` kept at 4 -> new `every_exempt_literal_occurs_in_its_file` (drift 21: the stack-word scan cannot turn red). Green.
- [x] R12 -- `verify_tests_*` green (61 passed, 1 skipped: the pilot, both iterations); M: a moniker leaking into one -> `outputs_equal_with_and_without_a_snapshot`: `lock`, `verify`, `refs`, `bundle`, `check`, `health`, `symbols`, `unmapped` and the MCP code tools byte-identical; `daemon_keys` 56. Green; the MCP half's snapshot is written by `sqlite3`; `daemon_keys` 3/3.
- [x] R13 -- carried from 11a-1: both B6 tests in `crates/specengine-ra/tests/protocol.rs` (`binary_kills_its_group_when_stdin_closes`, `..._after_budget_and_grace`) assert SIGKILL only again, the group-gone checks unchanged; 25 runs of each under load (12 `perl -e 'alarm 120; exec "yes"'`) green. M: `exit(3)` restored after the kill. 25 of 25 runs green under load (12 `yes` × 2 waves); `protocol.rs` 17/17 in 44.7 s; M8 red in 25 of 25.
- [x] R14 -- `resolve_request_is_protocol_v1`, the fake's logged request: exactly the six keys; `root` canonical with `spec` run through a symlinked path to the copy; `files` the golden's six in byte order (`arena-derive/src/lib.rs` first), a `.ron` and a non-UTF-8 `.rs` added under a walked root absent; `budget_ms` = `--timeout` × 1000; `target_dir`, `tmp_dir` the canonical `<data dir>/<slug>.ra/target`, `.../tmp`. M: `files` re-sorted by `Path`; `root` or `<slug>.ra` not canonicalised. Green; M7 (`files` re-sorted by `Path`) red.
- [x] R15 -- M: stdin closed after the request -> `stdin_is_held_until_the_last_line`: a fake that reads the request, then stdin for 1 s: EOF -> exit 3, no line, as the binary does; held -> the golden: exit 0, `fresh`. M: the request written to its end before the timer starts -> `budget_covers_an_unread_request`: a fake that never reads stdin, a request over 128 KiB (a scratch tree of many long `.rs` names), `--timeout 1`: exit 2 `over the budget of 1 s` within 1 + 5 s plus slack, no survivor. Green; M5 (stdin dropped after the request) red.

test-engineer:

- [x] TE-1 -- fakes: ok (the golden), wrong protocol, hang, grandchild, flood, crash mid-stream, malformed, missing; `refused` protocol 1; `refused` protocol 2 plus an extra key; `error` workspace 0 right after `hello`; `error` workspace 2 with no step 2; exit 3 with no line; `done` then exit 1; null stamps with `dir` `..`; one honouring stdin EOF as the binary does; one never reading stdin. Always beside a `spec` copy: once `target.noindex/debug/specengine-ra` exists, `debug/spec` finds it beside itself. Done: `layer_c_common`'s Perl fake, one plan per case.
- [x] TE-2 -- one `#[ignore]` end-to-end, `real_binary_stores_the_golden_rows`: `target.noindex/debug` `spec` and `specengine-ra` copies on a scratch `ra-mini`, `--timeout` ≥ 120 (a debug ra-mini run takes 21-50 s): exit 0, `fresh`, the stored rows equal the golden replay's, stamps aside (A6). Passed in 21.6 s; the stored rows equal the golden replay's.
- [x] TE-6 -- the owner's, 2026-10-22, both pilots (release, scratch `HOME`, read-only; the second's `git status` unchanged). Install 47.3 s wall (411.6 s user), binary 21 485 264 B. `spec resolve` exit 0: A 9.1 s, peak 2.76 GiB, files 194 (in a crate 193), items 11 850: moniker 11 835, unresolved 15, local, none, not_loaded 0; B 10.7 s, 3.46 GiB, files 189 (189), items 7 593, all moniker. Both: every item joined, unmatched 0, `c_only` 0; agrees 11 686 and disagrees 17 (A, cause not attributed), 7 536 and 0 (B); collisions 6 and 1; one workspace each, no `..` dir (L8), no not-UTF-8 note (L9). `r#` in a moniker by substring, 108 and 45, matches type descriptors of names ending in `r`; neither tree defines a raw identifier, so the `moniker_modules` limit is not exercised. 11b's figure (`specengine-eval ra`, clones, real `HOME`, its own item scan: 11 937 and 7 599): without the proc-macro server A 11 922 with a moniker (99.9 %), cold 8.8 s, warm 0.26 s, group peak 2 558 MB; B 7 599 (100 %), 10.6 s, 0.17 s, 3 061 MB; with it A 11 921, cold 33.7 s (build scripts 21.4 s), 3 202 MB; B 7 580, 66.0 s (50.6 s), 3 673 MB. No item gains a moniker (1 and 19 lose one: 0.01 %, 0.25 %), cold 3.9 and 6.3 times: under 11b's 1 % trigger.

spec-writer:

- [x] SW-1 -- this spec, split from `layer-c.md` 2026-10-19; refreshed against dc26973 the same day.
- [x] SW-2 -- 2026-10-19, at shipping: new Tier 2 `docs/canon/layer-c.md` and its index line; `layer-c-protocol.md` (the caller line, "Endings"' exit-3 limit dropped); `symbol-index.md`, `architecture-code.md`, `code-bindings.md`, `spec-verify-tests.md` "Runner", `stack.md` (`libc`); the CLI README (the reads' open minors moved to `spec-cli-read.md` "Known limits"), the store README (the search rows moved to `spec-cli-read.md` "Search index"), the code README, the root README; 05:123, 07:111, :209, 08:41, :43; `CLAUDE.md` State.

## Out of scope

`c` lines, `symbols` keys, tier `moniker`, MCP, plugin (11a-3); rebinding, `spec.lock` keys, `verify` verdicts by moniker (after 5b/5c, own ADR); HTTP, UI; a memory guard; `deps` call targets; the binary's request checks repeated in `spec` (L7).

## Open

| Q | Working answer | The other answer costs |
|---|---|---|
| Q2 protocol | v1 exact, stdin open, JSON lines; 300 s; stored after `done`, tree unchanged | version ranges, half snapshots |
| Q3 failing | only `resolve`: exit 2, install line, snapshot kept | minutes, GiBs per agent read |
| Q4 per row | facts by `(file, start)`; marker > qpath > moniker | lock re-keyed per `ra_ap` bump |
| Q5 freshness | one snapshot; any change: all stale | a parent `mod` edit mislabels a child |
| Q11 | no key, `scip` retired | a dead toggle |
| SW | manifests, cargo config in `tree_fp`; per-read join; `c_only` per read | stored joins rerun per 6a bump |
| L3 timing | on `resolve`'s stdout, the rule's one exception | stderr `note:` lines; TE-6 captures stderr |
| L6 Ctrl-C | `Forwarding`: group `SIGTERM`, 5 s, `SIGKILL`, exit 2 `interrupted` | the stdin watchdog kills the group, no message |
| L7 `refused`, protocol 1 (a file gone after the walk, a request over 64 MiB, an unclean data path, a directory not created) | exit 2 `specengine-ra refused the request: <reason>`, verbatim; a clean request, none of the binary's checks copied | the request rules duplicated in the CLI, or a generic failure |
| L8 a workspace root above the project root | stored like any run; `tree_fp` within the root; TE-6 records it | reads above the root on every `layer_c_state()`, or such trees never resolve |
| L9 a non-UTF-8 `.rs` module of a scanned crate | `load failed: database (workspace <n>)`; 6a's "not UTF-8" notes the hint; TE-6 counts them | a pre-spawn refusal of harmless files too, or a protocol v1 change |
| L10 `agrees` on `mod` rows | A's modules against C's `name/` descriptors before the item's own last; golden: agrees 26, disagrees 0 | every module declaration disagrees (3 of 26): noise |
| L11 a virtual root's manifest | every `TREE_FILES` name (`Cargo.toml` included) in each directory from a walked file's up to the root; a manifest duplicates its package entry | an edit to `members`, `[patch]` or `[workspace.dependencies]` of a virtual root stays `fresh` |

Q-n: 2026-10-10 under the owner's pre-approval; L-n: owner pre-approved 2026-10-19 (L7-L10 with the refresh against dc26973, L11 with review 1); review open. The rest: `layer-c.md` "Open".

## Dependencies and ordering

- **Needed** (shipped): 11a-1 `layer-c` (dc26973: the binary, the golden, the kind map); 6a-1..3, 8a-1 (the runner), 10a-1, 9a-1.
- **Order**: 11a-1 -> 11a-2 (this, shipped) -> the owner's TE-6 -> 11a-3 (`layer-c.md` "Dependencies and ordering").

## Implementation

Developer 2 iterations; review 1 accepted, no blocker or major; iteration 2 re-reviewed, accepted. Iteration 2: the program found on `PATH` made absolute against `spec`'s cwd before the spawn (the child's cwd is `<data>/<slug>.ra/tmp`; `spec serve` unchanged), `by_ord` once per file, the clap range from the constants, L11. Nits accepted: inherited stderr, the counts failing after the store, duplicate keys folding, `r#name` (TE-6).

| Module | What it does |
|---|---|
| CLI `group.rs` | the process-group core of `verify --tests` and `resolve`: `spawn`, `wait` (overrun, `Forwarding`, the caller's stop), `GRACE`; the CLI's only `unsafe` |
| CLI `layer_c.rs` | steps 1-6, `LAYER_C_BIN`, `tree_fp`, `layer_c_state`, `state_in`, the snapshot, join counts, `render_text` |
| CLI `layer_c_run.rs` | one run: the spawn, the writer and reader threads, the line cap, the ending table |
| CLI `layer_c_reply.rs` | `Reply::line` -> `Fault {Malformed, Mismatch}`, `REPLY_LINE_CAP`, `PROTOCOL` |
| CLI `test_runner.rs`, `serve.rs`, `code_index.rs`, `lib.rs`, `main.rs` | `group::Ended` mapped; `find_beside_or_on_path`; `package_targets`; exports, `Outcome::Resolve`, the command |
| store `layer_c.rs`, `schema.rs`, `dump.rs` | the three tables, `INDEX_FORMAT` 11, `replace_layer_c`, the readers, the dump's `wt` |
| code `layer_c.rs`, `qpath.rs` | `TREE_FILES`, `tree_files`, the join, `agrees`, collisions; `TargetIndex::primary()` |
| core `code_toml.rs` | `scip` out of `NOT_BUILT_KEYS` |
| `specengine-ra` `src/main.rs` | R13: `terminate` parks after `kill_own_group()` |
| tests | CLI `layer_c{,_state,_outputs,_scan}.rs`, `layer_c_common`; MCP `mcp_layer_c.rs`; `code_refs_walk.rs`, `code_index.rs`, store `format_history.txt` and its pin; ra `protocol.rs` B6 SIGKILL only |

Figures (macOS, debug): the golden replay 29/0/0/26/0/1 (joined, unmatched, `c_only`, agrees, disagrees, collisions), as expected; a real ra-mini run exit 0 in 22 s, peak 0.5 GiB, the stored rows equal to the golden's, nothing under the root; a relative `PATH` entry 20.6 s. Tests: CLI 78 passed, 1 skipped, 8.2 s; store format 13/13; MCP 2/2; `verify_tests_*` 61 + 1 skipped; `daemon_keys` 3/3; `doc_pointers` and anonymity 15/15; R13 25/25 under load. Mutations, round 1: 8 of 8 red (criteria).

Follow-ups, no change now (also in `layer-c-reads.md`): `tree_files` allocates per directory level (walk deepest-first, stop at the first known directory); `layer_c_state` reads the `Tree` twice (11a-3: a `state_of` from the refreshed tree, or `state_in` inside a read).

Deviations, accepted in review 1 ("Data" above, canon): 1-2 extra reasons; 3 `error` after `file` lines; 4 `protocol` read first; 5 closed categories, `end` unchecked; 6 collisions count groups; 7 the `tree_fp` encoding and `files` from the code index; 8 step 6's gap and notes; 9 inherited stderr; 10 `agrees` from stored rows; 11 `layer_c_state` public and refreshing; 12 the timeout checked twice.
