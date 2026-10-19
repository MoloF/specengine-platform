---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-core]
ref: Phase 3 plan 2026-10-10, slice 11a-2, split from 11a `layer-c.md` 2026-10-19 against 5b79b81 (drift 3-31; Q2-Q5, Q11 2026-10-10 under the owner's pre-approval; L3, L6 owner pre-approved 2026-10-19; review open); 05 s5.1; 07 s2, s5; 08 s2
adrs: [ADR-0049]
---

# layer-c-resolve (11a-2): `spec resolve` and the layer C snapshot

## Why

11a-1 (`layer-c.md`) builds `specengine-ra`, which answers a moniker or a state per item. Here `spec resolve` becomes the one command that spawns it: it checks the reply and stores it as one advisory snapshot per worktree, with a library state for 11a-3 (`layer-c-reads.md`) to show after the owner's TE-6. No read changes in this part. ADR-0049.

## Description and interactions

- **Owner** or an agent: `spec resolve [--timeout S] [--json]`, one cold load. Not `id_state resolved`.
- **Never implicit** (WA-3, ADR-0049): no read (`unmapped`, `health` too), no MCP tool, no daemon route (`GET health` too), no hook, gate, `index`, `check`, `verify` (any mode, `--tests`), `lock` or `propose drift` spawns the binary; a snapshot changes none of their outputs (11a-3 then changes only `spec symbols`' and `find_symbols`' keys and the `c`, `layer c:` lines).
- **Crates**: CLI `resolve` and the group core shared with `verify --tests` (A2); store the three tables, `INDEX_FORMAT` 11; `specengine-code` the `tree_fp` file names, the join, `agrees`, the collision suffix beside 11a-1's kind map; core retires `scip`. No HTTP, UI, MCP, plugin, hook or CI change; `daemon_keys.rs` stays at 56 sets. **Writes**: the data directory only.
- **Overridden live-spec lines** (dated notes in place, rewritten at shipping): 05:123; 07:111, :209; 08:41, :43.

## Data

### spec resolve

WA-1, WA-2, Q1-Q3. S: seconds 1..=3600, default 300. In order, each failure exit 2:

1. Not Unix; usage, discovery, config, no `[code]`, a walk gap (6a's reads, the code index refreshed first). `files`: the walked UTF-8 `.rs` files, none: `no Rust file to resolve` (A7); `tree_fp` computed.
2. `File::try_lock` on `<data dir>/<slug>.ra/lock`; `target/`, `tmp/` beside it.
3. Lookup as `spec serve`'s (`serve.md` "Launcher"): beside `current_exe()`, else the first `PATH` entry holding an executable regular `specengine-ra`.
4. Spawn, never exec, on the group core: `--protocol 1`, cwd and `TMPDIR` `tmp/`, own process group, `spec`'s environment (the binary scrubs it, L4); stdin held: the request, then open; stdout read as lines; stderr copied live.
5. Each line checked; at S: the group `SIGTERM`, 5 s, `SIGKILL`. `SIGINT`, `SIGTERM`, `SIGHUP` to `spec` (L6): 8a-1's `Forwarding`, the same escalation, then `interrupted`.
6. After `done` and exit 0: the code index refreshed, `tree_fp` recomputed; equal: one Immediate transaction replaces the worktree's rows in the three tables; else nothing stored.

- **Group core** (A2): 8a-1's `waitid` `WNOWAIT`, `killpg`, `SIGTERM`, the 5 s `GRACE`, `SIGKILL` and `Forwarding` (`spec-verify-tests.md` "Runner") move out of `test_runner.rs` into one CLI module both commands call. `verify --tests` keeps its I/O (stdin null, one merged pipe copied to stderr, `env_clear`) and its bytes; the CLI README's "the CLI's only `unsafe`" names the module.
- **Checks**: `hello` first, `protocol` 1; each requested file once, in request order, no other; items by `start` within a file; `done` last, counts equal; each line ≤ `REPLY_LINE_CAP` 8 388 608 B, JSON, known keys. Else `malformed reply at line <n>`.

### Tables

Q4, Q5. In `FORMAT_TABLES`: `INDEX_FORMAT` 10 -> 11, store `tests/format_history.txt` gains `11 <hash>`. `CODE_INDEX_FORMAT` stays 4, `code_format_history.txt` unchanged: no analyzer rule changes, the tables are outside `CODE_TABLES`, `code_index_common`'s `md_lines` excludes them. From a run: outside 6a's dump-equals-rebuild; `dump.rs` maps their keys.

| Table | Key | Columns |
|---|---|---|
| `code_resolve` | `(wt)` -> `code_worktrees` `ON DELETE CASCADE` | `tree_fp`, `protocol`, `ra_ap`, `host`, `toolchains` (JSON: the workspaces' distinct toolchains, byte-sorted), `degraded` (any `metadata_degraded`), `workspaces`, `files`, `items` |
| `code_resolve_files` | `(wt, path)` -> `code_resolve`, cascade | `blake3` (6a's at the run), `in_crate` |
| `code_monikers` | `(wt, path, start)` -> `code_resolve`, cascade; `moniker` indexed | `end`, `kind`, `state`, `moniker` (raw) |

Lifetime (A3): `clear_code` and an `INDEX_FORMAT` change drop the snapshot; `spec index --full`, an `exclude` change and a `CODE_INDEX_FORMAT` change keep it. No `c_only` column: per read (A10), as a 6a rule bump renames qpaths, not C's facts.

### Freshness

WA-5. `tree_fp` = BLAKE3, byte-sorted: protocol, `ra_ap`, `host`; each walked `.rs` `(path, blake3)`; each `code_packages` `(dir, manifest_b3)` (a renamed crate renames every moniker); the bytes of each `Cargo.lock`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml` in the root, a package dir or between. The names are `specengine-code` data; the CLI hashes bytes. The CLI library's `layer_c_state()`: `absent` (no snapshot), `fresh` (equal), `stale` (any change: every row stale, old values kept).

### Join

Per read (`specengine-code`), a 6a row against `code_monikers`; its `c_state` (WA-4), null while `absent`:

| `c_state` | When |
|---|---|
| `unseen` | file not in `code_resolve_files`, or its BLAKE3 changed (offsets moved) |
| `moniker`, `local`, `none`, `unresolved`, `not_loaded` | a row at its `(path, start byte)`, kind matching (11a-1's map) |
| `unmatched` | file seen unchanged, no such row |

- **`c_only`**: C rows no 6a row joins, counted.
- **Collision**: a raw moniker shared by ≥ 2 C rows of a file renders ` #<value>`: the joined rows' 6a discriminators if all distinct (`arena impl#[Health] #2`), else the ordinal by `start`. Raw text never holds ` #`.
- **`agrees`**: on `moniker` rows whose file has one target: C's crate equals A's target crate (`code_packages.targets`, `-` -> `_`) and C's leading `<name>/` descriptors equal A's modules; names, owners not compared. Null otherwise (`shared:`, `unrooted`, `path_attribute`), counted in neither agrees nor disagrees (A5).
- **Precedence**: the marker binds; the qpath names for `spec.lock`, `verify` (key `(file, kind, qpath)` unchanged); the moniker displays and finds. `agrees` false: counted, shown, never fixed or a finding.

### Output and exits

Exit 0:

```
layer c: specengine-ra protocol 1, ra_ap 0.0.352, host aarch64-macos
workspace 1 .: discover 2140 ms (toolchain 1.98.0, sysroot 43, degraded no), database 6120 ms
files 6 (in crate 4), items 29: moniker 26, local 0, none 0, unresolved 2, not_loaded 1
joined 28, unmatched 1, c_only 1; agrees 25, disagrees 1; collisions 1; stored fresh, 9.1 s, peak 2.7 GiB
```

- JSON `{protocol, ra_ap, host, workspaces: [{dir, toolchain, sysroot_packages, metadata_degraded, discover_ms, database_ms}], files, in_crate, items, by_state, joined, unmatched, c_only, agrees, disagrees, collisions, ms, peak_rss}`.
- Timing (L3): `ms`, `s`, `GiB`, `peak_rss` on `resolve`'s stdout only, the CLI's "no timing" rule's one exception; never stored or guarded (Q7); determinism covers the stored rows and the reads.
- Exit 2: stdout empty, `spec resolve: <reason>; the previous snapshot is kept`. Reasons beyond step 1's: `specengine-ra not found beside <spec> or on PATH; install it: cargo install --locked --force --path <clone>/crates/specengine-ra (rustc >= 1.98, rust-src)`; `specengine-ra speaks protocol <n>, spec needs 1: install both from one commit`; `a layer C run of <slug> is in progress`; `over the budget of <S> s`; `interrupted`; `load failed: <category>`; `specengine-ra ended (<status>) before done`; `malformed reply at line <n>`; `the tree changed during the run`; `no Rust file to resolve`.

### Config

Q11: no `[code]` key. `scip` is retired: `NOT_BUILT_KEYS` `["languages", "crates", "scip"]` -> `["languages", "crates"]`; `scip` gets the unknown-key text, "`[code]` has no key `scip`: it takes `roots` and `exclude` and `frameworks` only"; `code_toml.rs`'s module doc drops it; `code_refs_walk.rs` `EXEMPT` 4 -> 3. `crates` stays refused (6a Q2). The root README gets the install line.

## Rules and edge cases

- WHEN the binary is missing, hangs, crashes, overruns or is interrupted, other commands SHALL be unchanged and `resolve` SHALL keep the previous snapshot.
- WHEN any `tree_fp` input changes after a run, `layer_c_state()` SHALL be `stale`; WHEN one tree is resolved from two roots, twice or in reversed creation order, the layer C dump SHALL be byte-identical.
- Genre (`architecture.md#universal`): Cargo file names, the kind map, descriptor reading and collisions live in `specengine-code`; no SCIP or Cargo literal in store, core or the CLI. A moniker never binds, locks, judges or becomes a finding.

## Acceptance criteria

M: mutation -> the test turning red. CLI `layer_c.rs`: fakes replaying 11a-1's golden beside a copy of `spec` in a temp dir, under `alarm`; CLI tests never build `specengine-ra` (A6).

- [ ] X0 -- `git diff --stat 5b79b81 -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing; `Cargo.lock` equals 11a-1's.
- [ ] R1 -- M: `PATH` first -> `layer_c_binary_beside_spec_wins_over_path`; missing, exit 0 -> `resolve_without_binary_is_exit_2_with_install_line` (stdout empty).
- [ ] R2 -- M: no version check -> `protocol_mismatch_is_exit_2_and_stores_nothing`; any one check off -> `malformed_reply_is_exit_2_and_stores_nothing` (each case its line `<n>`); no cap -> `oversized_reply_line_is_exit_2`.
- [ ] R3 -- M: only the child killed -> `no_layer_c_grandchild_outlives_the_budget` (a grandchild ignoring `SIGTERM`, `--timeout 2`: exit 2, no survivor); default SIGINT disposition -> `interrupted_resolve_kills_the_group_and_keeps_the_snapshot`.
- [ ] R4 -- M: rows written while streaming -> `budget_overrun_keeps_the_previous_snapshot` (a crash mid-stream too); no re-hash -> `tree_change_during_run_stores_nothing`; no lock -> `second_resolve_is_refused_while_one_runs`.
- [ ] R5 -- M: `symbols` spawns -> `reads_mcp_check_gate_verify_never_spawn_layer_c`: a logging fake beside the copy and on `PATH` stays empty across `health`, `unmapped`, `symbols`, `refs`, `bundle`, `index`, `check`, gate, `verify` (plain, `--staged`, `--tests`), `lock accept`, `propose drift`, and the MCP code tools from `mcp_code.rs`'s harness; a call from `health` -> `only_resolve_spawns_layer_c` (a source scan over every crate's `src/`: the spawn's one caller, the daemon route included).
- [ ] R6 -- `layer_c_state` fresh after a run, then M: per-file freshness -> `layer_c_state_is_stale_after_another_file_edit`; manifests out -> `manifest_edit_makes_the_snapshot_stale`; one out -> `lock_toolchain_or_cargo_config_edit_makes_the_snapshot_stale` (each of `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`); `--full` drops -> `full_reindex_keeps_the_snapshot`; no cascade -> `clear_code_drops_the_snapshot`.
- [ ] R7 -- M: kept at 10 -> store `format_history_ends_with_the_current_format_and_dump_hash` (`11 <hash>`); outside `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`, covering the three; `CODE_INDEX_FORMAT` 4, `code_format_history.txt` unchanged.
- [ ] R8 -- `golden_replay_gives_the_pinned_counts`: joined, unmatched, `c_only`, agrees, disagrees, collisions pinned; text (timings masked) and every JSON key as in Data. M: by line -> `attributed_item_joins_its_row`; by name -> `same_name_items_join_by_offset`; kinds ignored -> `boundary_mismatch_is_unmatched_and_c_only`; offsets from normalised text -> `crlf_file_joins_by_raw_bytes`.
- [ ] R9 -- M: hash-map order -> `resolve_rows_are_byte_identical_across_runs_and_roots` (two roots, two runs, reversed creation; TE-4).
- [ ] R10 -- M: `agrees` split at `/` in the CLI, a `Cargo.lock` literal in the CLI -> `no_scip_or_cargo_literal_outside_code_and_ra` (store, core, the CLI).
- [ ] R11 -- M: `scip` kept in `NOT_BUILT_KEYS` -> `code_table_unknown_key_is_refused` case 13 (`scip = false`); `EXEMPT` kept at 4 -> new `every_exempt_literal_occurs_in_its_file` (drift 21: the stack-word scan cannot turn red).
- [ ] R12 -- `verify_tests_*` green; M: a moniker leaking into one -> `outputs_equal_with_and_without_a_snapshot`: `lock`, `verify`, `refs`, `bundle`, `check`, `health`, `symbols`, `unmapped` and the MCP code tools byte-identical; `daemon_keys` 56.

test-engineer:

- [ ] TE-1 -- fakes: ok (the golden), wrong protocol, hang, grandchild, flood, crash mid-stream, malformed, missing; always beside a `spec` copy: once `target.noindex/debug/specengine-ra` exists, `debug/spec` finds it beside itself.
- [ ] TE-2 -- one `#[ignore]` end-to-end: `target.noindex/debug` `spec` and `specengine-ra` copies on a scratch `ra-mini`: exit 0, `fresh` (A6).
- [ ] TE-6 -- the owner's, gating 11a-3: both pilots, manual (`#[ignore]`, release, `--timeout`, scratch `HOME`, read-only, unnamed): cold time, peak; counts per state; `agrees` false; collisions; `c_only`; 11b's figure: `specengine-eval ra`'s moniker counts with and without the proc-macro server; install time, size.

spec-writer:

- [x] SW-1 -- this spec, split from `layer-c.md` 2026-10-19.
- [ ] SW-2 -- at shipping: `docs/canon/layer-c.md` (11a-1's): resolve, tables, freshness; `symbol-index.md` (12 234 of 12 288: detail down first; its refreshers plus resolve, `INDEX_FORMAT` 11 with a pointer, history); `architecture-code.md` (resolve); `code-bindings.md` (10 842: `scip` beside `bevy`); the CLI README (10 219 of 10 240, room first: Commands, API, Rules, the timing exception, the group module); the store README (10 216: rows detail out); the code README (the module row); the root README install line; 05:123, 07:111, :209, 08:41, :43; `CLAUDE.md` State.

## Out of scope

`c` lines, `symbols` keys, tier `moniker`, MCP, plugin (11a-3); rebinding, `spec.lock` keys, `verify` verdicts by moniker (after 5b/5c, own ADR); HTTP, UI; a memory guard; `deps` call targets.

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

Q-n: 2026-10-10 under the owner's pre-approval; L-n: owner pre-approved 2026-10-19; review open. The rest: `layer-c.md` "Open".

## Risks

- `INDEX_FORMAT` 11 drops every index once; the group-core refactor edits shipped 8a-1 code (`verify_tests_*` guard it).
- `tree_fp` misses cargo configs above the root or in `$CARGO_HOME`, and `rustup default` changes (stamped only).
- Doc caps for SW-2: the CLI, store and code READMEs and `symbol-index.md` are within 150 B of theirs.

## Dependencies and ordering

- **Needs**: 11a-1 `layer-c` (the binary, the golden, the kind map); shipped 6a-1..3, 8a-1 (the runner), 10a-1, 9a-1.
- **Order**: 11a-1 -> 11a-2 (this) -> the owner's TE-6 -> 11a-3 (`layer-c.md` "Dependencies and ordering").

## Implementation

Not started; when filled, ≤ 3 072 B. To record: modules, the group core's name, deviations with reasons.
