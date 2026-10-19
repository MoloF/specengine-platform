---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-core, crates/specengine-ra]
ref: Phase 3 plan 2026-10-10, slice 11a-2, split from 11a `layer-c.md` 2026-10-19 against 5b79b81 (drift 3-31; Q2-Q5, Q11 2026-10-10 under the owner's pre-approval; L3, L6 owner pre-approved 2026-10-19; review open); refreshed 2026-10-19 against 11a-1 as built, dc26973 (the analyst's delta; L7-L10 owner pre-approved 2026-10-19, review open); 05 s5.1; 07 s2, s5; 08 s2
adrs: [ADR-0049]
---

# layer-c-resolve (11a-2): `spec resolve` and the layer C snapshot

## Why

11a-1 (`layer-c.md`, shipped) builds `specengine-ra`, which answers a moniker or a state per item; protocol v1 as built is `docs/canon/layer-c-protocol.md`. Here `spec resolve` becomes the one command that spawns it: it checks the reply and stores it as one advisory snapshot per worktree, with a library state for 11a-3 (`layer-c-reads.md`) to show after the owner's TE-6. No read changes in this part. ADR-0049.

## Description and interactions

- **Owner** or an agent: `spec resolve [--timeout S] [--json]`, one cold load. Not `id_state resolved`.
- **Never implicit** (WA-3, ADR-0049): no read (`unmapped`, `health` too), no MCP tool, no daemon route (`GET health` too), no hook, gate, `index`, `check`, `verify` (any mode, `--tests`), `lock` or `propose drift` spawns the binary; a snapshot changes none of their outputs (11a-3 then changes only `spec symbols`' and `find_symbols`' keys and the `c`, `layer c:` lines).
- **Crates**: CLI `resolve` and the group core shared with `verify --tests` (A2); store the three tables, `INDEX_FORMAT` 11; `specengine-code` the `tree_fp` file names, the join, `agrees`, the collision suffix beside 11a-1's kind map; core retires `scip`; `specengine-ra` one fix carried from 11a-1 (R13): `src/main.rs` `terminate` parks after `kill_own_group()` and never calls `exit` while leading its group (`layer-c-protocol.md` "Endings": exit 3 can beat the group SIGKILL on macOS). No HTTP, UI, MCP, plugin, hook or CI change; `daemon_keys.rs` stays at 56 sets. **Writes**: the data directory only.
- **Overridden live-spec lines** (dated notes in place, rewritten at shipping): 05:123; 07:111, :209; 08:41, :43.

## Data

### spec resolve

WA-1, WA-2, Q1-Q3. S: seconds 1..=3600, default 300. Protocol v1 is canon (`layer-c-protocol.md` "Run" to "Watchdogs"); below, the caller's side. In order, each failure exit 2:

1. Not Unix; usage, discovery, config, no `[code]`, a walk gap (6a's reads, the code index refreshed first). `files`: the walked `.rs` files whose content is UTF-8, root-relative, in the walk's byte order (`code_walk.rs`: `arena-derive/…` before `arena/…`), never re-sorted by `Path`; none: `no Rust file to resolve` (A7); `tree_fp` computed.
2. `File::try_lock` on `<data dir>/<slug>.ra/lock`; `target/`, `tmp/` beside it; D: that `<slug>.ra` directory, canonicalised.
3. Lookup as `spec serve`'s (`serve.md` "Launcher"): beside `current_exe()`, else the first `PATH` entry holding an executable regular `specengine-ra`.
4. Spawn, never exec, on the group core: exactly `--protocol 1`, cwd `D/tmp`, the leader of its own process group (canon "Run": a binary not leading its group exits 3 alone); `spec`'s environment unchanged: the binary scrubs it and sets `TMPDIR` itself (11a-1 deviation 7; L4's "plus `TMPDIR`" no longer applies). Then the request, under the stdin contract below.
5. Each reply line checked as it arrives; nothing is written before step 6. At S since the spawn: the group `SIGTERM`, 5 s, `SIGKILL`; a failed check or a protocol mismatch: the same escalation at once, stdin closed. `SIGINT`, `SIGTERM`, `SIGHUP` to `spec` (L6): 8a-1's `Forwarding`, the same escalation, then `interrupted`. The binary's own budget watchdog (S + 10 s) is only a fallback.
6. After `done` and exit 0: the code index refreshed, `tree_fp` recomputed; equal: one Immediate transaction replaces the worktree's rows in the three tables; else nothing stored.

**Request**: one line, exactly the six keys:

```json
{"protocol":1,"root":"/abs/root","files":["arena-derive/src/lib.rs","arena/build.rs","arena/src/lib.rs","arena/src/stray.rs","arena/src/systems.rs","tools/probe.rs"],"target_dir":"/abs/data/ra-mini.ra/target","tmp_dir":"/abs/data/ra-mini.ra/tmp","budget_ms":300000}
```

`root`: `ProjectRoot.root`, already canonical (`project.rs`); `files`: step 1's; `target_dir`, `tmp_dir`: `D/target`, `D/tmp`; `budget_ms`: S × 1000. `spec` builds a clean request and copies none of the binary's checks: a refusal is reported, never pre-empted (L7).

**Stdin, stdout, stderr**:

- the request is written on its own thread, and S runs while it is written: a binary that never reads it is an overrun;
- a broken pipe while writing is not reported: the reply and the exit decide;
- stdin stays open until the reply's last line (`done`, `error`, `refused`) is read, then is closed (the binary's B10 guard then exits 0 or 1); closed earlier, the binary SIGKILLs its group and no `done` comes;
- stdout is read as lines; stderr is copied live to `spec`'s stderr.

**Group core** (A2): 8a-1's `waitid` `WNOWAIT`, `killpg`, `SIGTERM`, the 5 s `GRACE`, `SIGKILL` and `Forwarding` (`spec-verify-tests.md` "Runner") move out of `test_runner.rs` into one CLI module both commands call: it spawns a prepared `Command` as a group leader and waits; the I/O is each caller's. Its wait gains a third trigger beside the overrun and `Forwarding`: the caller's stop (a failed check), the same escalation. The first trigger is the outcome: an overrun or a forwarded signal wins over any exit seen afterwards, as in 8a-1's wait. `verify --tests` keeps its I/O (stdin null, one merged pipe copied to stderr, `env_clear`) and its bytes; the CLI README's "the CLI's only `unsafe`" names the module.

**Checks**, per line (canon "Reply", "Order", "Endings"); any fault: `malformed reply at line <n>`:

- the reply is `refused` alone, or begins with `hello`; the `protocol` of `hello` and `refused` is read before the line's other checks, so ≠ 1 gives the mismatch reason, never malformed;
- `step` lines: workspaces numbered from 1, no gap; each `discover` then `database`, both with one `dir` (a lone `discover` only before an `error`); `discover` carries `toolchain` (a string or null), `sysroot_packages`, `metadata_degraded`; `dir` a string (`.`, `a/b`, or with `..` segments), neither unique nor sorted;
- the end is either an `error` whose `workspace` is 0 ..= the last step's number + 1 (a failed discover writes no step), or, after all steps, one `file` line per requested path in request order, then `done`;
- a `file` line: items strictly by `start`; `kind` one of the 13 of `specengine_code::layer_c::KIND_MAP`; `state` one of the 5; `moniker` a string exactly when `state` is `moniker`;
- `done`: `files`, `items` and `by_state` (its 5 keys) equal the tallies; `peak_rss` an integer or null; `ms`;
- every line: none after the last; each ≤ `REPLY_LINE_CAP` 8 388 608 B, JSON, exactly its type's keys. A last line cut off by EOF is not judged: the exit decides.

### Tables

Q4, Q5. In `FORMAT_TABLES`: `INDEX_FORMAT` 10 -> 11, store `tests/format_history.txt` gains `11 <hash>`. `CODE_INDEX_FORMAT` stays 4, `code_format_history.txt` unchanged: no analyzer rule changes, the tables are outside `CODE_TABLES`, `code_index_common`'s `md_lines` excludes them. From a run: outside 6a's dump-equals-rebuild; `dump.rs` maps their keys.

| Table | Key | Columns |
|---|---|---|
| `code_resolve` | `(wt)` -> `code_worktrees` `ON DELETE CASCADE` | `tree_fp`, `protocol`, `ra_ap`, `host`, `toolchains` (JSON: the workspaces' distinct non-null toolchains, byte-sorted), `degraded` (any `metadata_degraded`), `workspaces`, `files`, `items` |
| `code_resolve_files` | `(wt, path)` -> `code_resolve`, cascade | `blake3` (6a's at the run), `in_crate` |
| `code_monikers` | `(wt, path, start)` -> `code_resolve`, cascade; `moniker` indexed | `end`, `kind`, `state`, `moniker` (raw) |

Lifetime (A3): `clear_code` and an `INDEX_FORMAT` change drop the snapshot; `spec index --full`, an `exclude` change and a `CODE_INDEX_FORMAT` change keep it. No `c_only` column: per read (A10), as a 6a rule bump renames qpaths, not C's facts.

### Freshness

WA-5. `tree_fp` = BLAKE3, byte-sorted: protocol, `ra_ap`, `host` (from `hello`; `layer_c_state()` takes the stored ones); each requested file `(path, blake3)`; each `code_packages` `(dir, manifest_b3)` (a renamed crate renames every moniker); the bytes of each `Cargo.lock`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml` in the root, a package dir or between, never above the root (a workspace root above it, `dir` with `..`: L8). The names are `specengine-code` data; the CLI hashes bytes. The CLI library's `layer_c_state()`: `absent` (no snapshot), `fresh` (equal), `stale` (any change: every row stale, old values kept).

### Join

Per read (`specengine-code`), a 6a row against `code_monikers`; its `c_state` (WA-4), null while `absent`:

| `c_state` | When |
|---|---|
| `unseen` | file not in `code_resolve_files`, or its BLAKE3 changed (offsets moved) |
| `moniker`, `local`, `none`, `unresolved`, `not_loaded` | a row with equal `(path, start, end)` where `kind_matches` (11a-1's map) holds; `code_monikers` keeps its key `(wt, path, start)` |
| `unmatched` | file seen unchanged, no such row |

- **`c_only`**: C rows no 6a row joins, counted.
- **Collision**: a raw moniker shared by ≥ 2 C rows of a file renders ` #<value>`: the joined rows' 6a discriminators if all distinct (`arena impl#[Health] #2`), else the ordinal by `start`. Raw text never holds ` #`.
- **`agrees`**: on `moniker` rows whose file has one target: C's crate equals A's target crate (`code_packages.targets`, `-` -> `_`: `build_script_build main().` agrees) and C's leading `<name>/` descriptors before the item's own last descriptor equal A's modules (L10: a `mod` row's own `name/` is not a module); names, owners not compared. Null otherwise (`shared:`, `unrooted`, `path_attribute`), counted in neither agrees nor disagrees (A5).
- **Precedence**: the marker binds; the qpath names for `spec.lock`, `verify` (key `(file, kind, qpath)` unchanged); the moniker displays and finds. `agrees` false: counted, shown, never fixed or a finding.

### Output and exits

Exit 0, on ra-mini (the canon's reply figures):

```
layer c: specengine-ra protocol 1, ra_ap 0.0.352, host aarch64-macos
workspace 1 .: discover 235 ms (toolchain 1.98.1, sysroot 26, degraded no), database 313 ms
files 6 (in crate 4), items 29: moniker 26, local 0, none 0, unresolved 2, not_loaded 1
joined 29, unmatched 0, c_only 0; agrees 26, disagrees 0; collisions 1; stored fresh, 20.5 s, peak 0.5 GiB
```

- The golden replay's expected counts: joined 29, unmatched 0, `c_only` 0, agrees 26, disagrees 0 (L10), collisions 1 (`arena impl#[Health]` at 341 and 453); measured at implementation (R8). unmatched and `c_only` above 0 need hand-written replies.
- Null stamps: text `toolchain unknown`, `peak unknown`; JSON null. `dir` printed as given, `..` included.
- JSON `{protocol, ra_ap, host, workspaces: [{dir, toolchain, sysroot_packages, metadata_degraded, discover_ms, database_ms}], files, in_crate, items, by_state, joined, unmatched, c_only, agrees, disagrees, collisions, ms, peak_rss}`.
- Timing (L3): `ms`, `s`, `GiB`, `peak_rss` on `resolve`'s stdout only, the CLI's "no timing" rule's one exception; never stored or guarded (Q7); determinism covers the stored rows and the reads.

Exit 2: stdout empty, `spec resolve: <reason>; the previous snapshot is kept`, the snapshot untouched. By the binary's ending:

| The binary ends | Reason |
|---|---|
| `done`, exit 0 | none: step 6 stores, exit 0; or `the tree changed during the run` |
| `error`, exit 1 | `load failed: <category> (workspace <n>)`; n = 0: `load failed: <category> (files under no manifest)` |
| `refused` or `hello` with `protocol` ≠ 1 | `specengine-ra speaks protocol <n>, spec needs 1: install both from one commit` |
| `refused` with `protocol` 1, exit 2 | `specengine-ra refused the request: <reason>`, the reason verbatim (L7) |
| exit 3, a signal, or a last line with another exit | `specengine-ra ended (exit <c>) before done`, `(signal <s>)` for a signal; `after done` once `done` was read |
| spawn fails | `specengine-ra cannot be started: <error>` |

Also: `malformed reply at line <n>` (a failed check); `over the budget of <S> s`; `interrupted`; these win over any exit seen afterwards. Before the spawn, beyond step 1's: `no Rust file to resolve`; `a layer C run of <slug> is in progress`; `specengine-ra not found beside <spec> or on PATH; install it: cargo install --locked --force --path <clone>/crates/specengine-ra (rustc >= 1.98, rust-src)`.

### Config

Q11: no `[code]` key. `scip` is retired: `NOT_BUILT_KEYS` `["languages", "crates", "scip"]` -> `["languages", "crates"]`; `scip` gets the unknown-key text, "`[code]` has no key `scip`: it takes `roots` and `exclude` and `frameworks` only"; `code_toml.rs`'s module doc drops it; `code_refs_walk.rs` `EXEMPT` 4 -> 3. `crates` stays refused (6a Q2). The root README gets the install line.

## Rules and edge cases

- WHEN the binary is missing, hangs, crashes, overruns, refuses or is interrupted, other commands SHALL be unchanged and `resolve` SHALL keep the previous snapshot.
- WHEN the reply is not yet at its last line, `spec` SHALL keep the binary's stdin open; WHEN the binary never reads the request, S SHALL still end the run.
- WHEN any `tree_fp` input changes after a run, `layer_c_state()` SHALL be `stale`; WHEN one tree is resolved from two roots, twice or in reversed creation order, the layer C dump SHALL be byte-identical.
- Genre (`architecture.md#universal`): Cargo file names, the kind map, descriptor reading and collisions live in `specengine-code`; no SCIP or Cargo literal in store, core or the CLI (R10 names them). Protocol v1's keys, line types, categories and the install line are the CLI's own data (precedent: `serve.rs` `no_binary`). A moniker never binds, locks, judges or becomes a finding.

## Acceptance criteria

M: mutation -> the test turning red. CLI `layer_c.rs`, under `alarm`; CLI tests never build `specengine-ra` (A6). A fake: an executable named `specengine-ra` beside a copy of `spec` in a temp dir; it reads and logs the request line, writes its reply and exits with the ending's code. By default it replays `fixtures/layer-c/ra-mini.jsonl` (stamps masked: `host`, `toolchain` "masked"; `ms`, `peak_rss`, `sysroot_packages` 0) on a scratch copy of `fixtures/ra-mini` plus a `specengine.toml` whose `[code]` walk lists exactly its six `.rs` files.

- [ ] X0 -- `git diff --stat 5b79b81 -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing; `Cargo.lock` equals 11a-1's (dc26973).
- [ ] R1 -- M: `PATH` first -> `layer_c_binary_beside_spec_wins_over_path`; missing, exit 0 -> `resolve_without_binary_is_exit_2_with_install_line` (stdout empty).
- [ ] R2 -- M: `error`, `refused` or exit 3 taken as success -> `every_ending_but_done_and_exit_0_keeps_the_snapshot`, one case per row of "Output and exits"' table but the first (R4) (exit 2, the row's reason, stdout empty, the layer C dump byte-identical): `error` with workspace 1 and with 0, `refused` protocol 1, exit 3 with no line, a signal, `done` then exit 1, a `#!` to a missing interpreter; M: a check stricter than the canon -> `protocol_v1_variants_are_not_malformed` (`dir` `..` with null `toolchain` and `peak_rss`: stored, `toolchain unknown`, `peak unknown`; `error` N with no step N, `error` 0 right after `hello`: `load failed`); M: no version check, or keys checked before `protocol` -> `protocol_mismatch_is_exit_2_and_stores_nothing` (`hello` protocol 2; `refused` protocol 2 plus an extra key); any one check off -> `malformed_reply_is_exit_2_and_stores_nothing` (each case its line `<n>`, a line after `done` among them); no cap -> `oversized_reply_line_is_exit_2`.
- [ ] R3 -- M: only the child killed -> `no_layer_c_grandchild_outlives_the_budget` (a grandchild ignoring `SIGTERM`, `--timeout 2`: exit 2, no survivor); default SIGINT disposition -> `interrupted_resolve_kills_the_group_and_keeps_the_snapshot`.
- [ ] R4 -- M: rows written while streaming -> `budget_overrun_keeps_the_previous_snapshot` (a crash mid-stream too); no re-hash -> `tree_change_during_run_stores_nothing`; no lock -> `second_resolve_is_refused_while_one_runs`.
- [ ] R5 -- M: `symbols` spawns -> `reads_mcp_check_gate_verify_never_spawn_layer_c`: a logging fake beside the copy and on `PATH` stays empty across `health`, `unmapped`, `symbols`, `refs`, `bundle`, `index`, `check`, gate, `verify` (plain, `--staged`, `--tests`), `lock accept`, `propose drift`, and the MCP code tools from `mcp_code.rs`'s harness; a call from `health` -> `only_resolve_spawns_layer_c` (a source scan over every crate's `src/`: the spawn's one caller, the daemon route included).
- [ ] R6 -- `layer_c_state` fresh after a run, then M: per-file freshness -> `layer_c_state_is_stale_after_another_file_edit`; manifests out -> `manifest_edit_makes_the_snapshot_stale`; one out -> `lock_toolchain_or_cargo_config_edit_makes_the_snapshot_stale` (each of `Cargo.lock`, `rust-toolchain.toml`, `.cargo/config.toml`); `--full` drops -> `full_reindex_keeps_the_snapshot`; no cascade -> `clear_code_drops_the_snapshot`.
- [ ] R7 -- M: kept at 10 -> store `format_history_ends_with_the_current_format_and_dump_hash` (`11 <hash>`); outside `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`, covering the three; `CODE_INDEX_FORMAT` 4, `code_format_history.txt` unchanged.
- [ ] R8 -- `golden_replay_gives_the_pinned_counts`: the measured counts pinned (expected joined 29, unmatched 0, `c_only` 0, agrees 26, disagrees 0, collisions 1; a difference recorded in "Implementation" with its reason); text (timings masked) and every JSON key as in Data. M: a `mod` row's own `name/` compared (L10), or no `-` -> `_` -> that test. M: by line -> `attributed_item_joins_its_row`; by name -> `same_name_items_join_by_offset`; `end` ignored, kinds ignored -> `boundary_mismatch_is_unmatched_and_c_only` (hand-written replies: one item's `end` off by one byte; one item's `kind` not matching: unmatched 1, `c_only` 1 each); offsets from normalised text -> `crlf_file_joins_by_raw_bytes`.
- [ ] R9 -- M: hash-map order -> `resolve_rows_are_byte_identical_across_runs_and_roots` (two roots, two runs, reversed creation; TE-4).
- [ ] R10 -- `no_scip_or_cargo_literal_outside_code_and_ra`, a string- and char-literal scan of store, core and CLI `src/` (`#[cfg(test)]` modules excluded): none holds `Cargo.lock`, `rust-toolchain`, `rust-toolchain.toml`, `.cargo/config`, `.cargo/config.toml`, `build-script-build`, `build_script_build`, nor `Cargo.toml` but 6a's `code_walk.rs` `MANIFEST`; the slice's new CLI and store modules hold no literal equal to `/`, `#`, `.`, `!`, `[`, `]`, `().` or holding `impl#[`. M: `agrees` split at `/` in the CLI; a `Cargo.lock` literal in the CLI.
- [ ] R11 -- M: `scip` kept in `NOT_BUILT_KEYS` -> `code_table_unknown_key_is_refused` case 13 (`scip = false`); `EXEMPT` kept at 4 -> new `every_exempt_literal_occurs_in_its_file` (drift 21: the stack-word scan cannot turn red).
- [ ] R12 -- `verify_tests_*` green; M: a moniker leaking into one -> `outputs_equal_with_and_without_a_snapshot`: `lock`, `verify`, `refs`, `bundle`, `check`, `health`, `symbols`, `unmapped` and the MCP code tools byte-identical; `daemon_keys` 56.
- [ ] R13 -- carried from 11a-1: both B6 tests in `crates/specengine-ra/tests/protocol.rs` (`binary_kills_its_group_when_stdin_closes`, `..._after_budget_and_grace`) assert SIGKILL only again, the group-gone checks unchanged; 25 runs of each under load (12 `perl -e 'alarm 120; exec "yes"'`) green. M: `exit(3)` restored after the kill.
- [ ] R14 -- `resolve_request_is_protocol_v1`, the fake's logged request: exactly the six keys; `root` canonical with `spec` run through a symlinked path to the copy; `files` the golden's six in byte order (`arena-derive/src/lib.rs` first), a `.ron` and a non-UTF-8 `.rs` added under a walked root absent; `budget_ms` = `--timeout` × 1000; `target_dir`, `tmp_dir` the canonical `<data dir>/<slug>.ra/target`, `.../tmp`. M: `files` re-sorted by `Path`; `root` or `<slug>.ra` not canonicalised.
- [ ] R15 -- M: stdin closed after the request -> `stdin_is_held_until_the_last_line`: a fake that reads the request, then stdin for 1 s: EOF -> exit 3, no line, as the binary does; held -> the golden: exit 0, `fresh`. M: the request written to its end before the timer starts -> `budget_covers_an_unread_request`: a fake that never reads stdin, a request over 128 KiB (a scratch tree of many long `.rs` names), `--timeout 1`: exit 2 `over the budget of 1 s` within 1 + 5 s plus slack, no survivor.

test-engineer:

- [ ] TE-1 -- fakes: ok (the golden), wrong protocol, hang, grandchild, flood, crash mid-stream, malformed, missing; `refused` protocol 1; `refused` protocol 2 plus an extra key; `error` workspace 0 right after `hello`; `error` workspace 2 with no step 2; exit 3 with no line; `done` then exit 1; null stamps with `dir` `..`; one honouring stdin EOF as the binary does; one never reading stdin. Always beside a `spec` copy: once `target.noindex/debug/specengine-ra` exists, `debug/spec` finds it beside itself.
- [ ] TE-2 -- one `#[ignore]` end-to-end: `target.noindex/debug` `spec` and `specengine-ra` copies on a scratch `ra-mini`, `--timeout` ≥ 120 (a debug ra-mini run takes 21-50 s): exit 0, `fresh`, the stored rows equal the golden replay's, stamps aside (A6).
- [ ] TE-6 -- the owner's, gating 11a-3: both pilots, manual (`#[ignore]`, release, `--timeout`, scratch `HOME`, read-only, unnamed): cold time, peak; counts per state; `agrees` false; collisions; `c_only`; a workspace `dir` with `..` (L8); the "not UTF-8" notes (L9); 11b's figure: `specengine-eval ra`'s moniker counts with and without the proc-macro server; install time, size.

spec-writer:

- [x] SW-1 -- this spec, split from `layer-c.md` 2026-10-19; refreshed against dc26973 the same day.
- [ ] SW-2 -- at shipping: a new Tier 2 `docs/canon/layer-c.md`: resolve, tables, freshness; `layer-c-protocol.md` (12 218 of 12 288 B): line 11's "Its one caller is to be `spec resolve` (…, not built)" rewritten shorter, pointing at `layer-c.md`, and "Endings"' macOS exit-3 known limit dropped (R13); `symbol-index.md` (12 234 of 12 288: detail down first; its refreshers plus resolve, `INDEX_FORMAT` 11 with a pointer, history); `architecture-code.md` (resolve); `code-bindings.md` (10 842: `scip` beside `bevy`); the CLI README (10 219 of 10 240, room first: Commands, API, Rules, the timing exception, the group module); the store README (10 216: rows detail out); the code README (the module row); the root README install line; 05:123, 07:111, :209, 08:41, :43; `CLAUDE.md` State.

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
| L8 a workspace root above the project root | stored like any run; `tree_fp` within the root; Risks; TE-6 records it | reads above the root on every `layer_c_state()`, or such trees never resolve |
| L9 a non-UTF-8 `.rs` module of a scanned crate | `load failed: database (workspace <n>)`; 6a's "not UTF-8" notes the hint; Risks; TE-6 counts them | a pre-spawn refusal of harmless files too, or a protocol v1 change |
| L10 `agrees` on `mod` rows | A's modules against C's `name/` descriptors before the item's own last; golden: agrees 26, disagrees 0 | every module declaration disagrees (3 of 26): noise |

Q-n: 2026-10-10 under the owner's pre-approval; L-n: owner pre-approved 2026-10-19 (L7-L10 with the refresh against dc26973); review open. The rest: `layer-c.md` "Open".

## Risks

- `INDEX_FORMAT` 11 drops every index once; the group-core refactor edits shipped 8a-1 code (`verify_tests_*` guard it).
- `tree_fp` misses cargo configs above the root or in `$CARGO_HOME`, and `rustup default` changes (stamped only); a workspace root above the project root (`dir` with `..`): its manifests, lock and cargo configs are outside the walk and `tree_fp` (L8).
- `ra_ap` and `host` come only from `hello`: a reinstalled binary never makes a snapshot stale, only stamped.
- A non-UTF-8 module file of a scanned crate fails every run with `load failed: database`, the culprit unnamed (L9).
- Doc caps for SW-2: `layer-c-protocol.md` has 70 B left; the CLI, store and code READMEs and `symbol-index.md` are within 150 B of theirs.

## Dependencies and ordering

- **Needs**: 11a-1 `layer-c` (shipped, dc26973: the binary, the golden, the kind map); shipped 6a-1..3, 8a-1 (the runner), 10a-1, 9a-1.
- **Order**: 11a-1 -> 11a-2 (this) -> the owner's TE-6 -> 11a-3 (`layer-c.md` "Dependencies and ordering").

## Implementation

Not started; when filled, ≤ 3 072 B. To record: modules, the group core's name, the measured golden counts, deviations with reasons.
