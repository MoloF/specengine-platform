---
class: spec
status: shipped
scope: [crates/specengine-ra, crates/specengine-code, crates/specengine-eval]
ref: Phase 3 plan 2026-10-10, slice 11a of layer C (WA-1..WA-13; Q1-Q13 decided under the owner's pre-approval, review open); refreshed and split 2026-10-19 against 5b79b81 (drift 1-31; L1-L6 owner pre-approved 2026-10-19, review open) into 11a-1 (this), `layer-c-resolve.md` (11a-2), `layer-c-reads.md` (11a-3); 04 s1.7; 05 s5.1; 08 s2, AC-10; developer 2 iterations, review 1 accepted
shipped: 2026-10-19
adrs: [ADR-0049]
---

# layer-c (11a-1): rust-analyzer in its own executable, protocol v1

## Why

Layer A names items by a heuristic and says where it is unsure (`mod x;`, stray files, `#[path]`: `code-identity.md` "Known limits"); the marker carries the link (ADR-0020). Layer C, measured in Phase 0 (05 s5.1: cold 8-10 s, ≤ 3.2 GiB without proc macros), was called only by `specengine-eval --features ra`. Insurance, built small: on the pilots A plus 6a's discriminator leave almost nothing open (WA-13).

Slice 11a, in three parts (L1): **11a-1** (this) `specengine-ra` gains an executable speaking protocol v1, a pure function of request and disk, no proc macros, no `spec` command; **11a-2** `layer-c-resolve.md`: `spec resolve`, the only spawner, stores each item's moniker beside 6a's rows as advisory data; the owner's TE-6 on the pilots; **11a-3** `layer-c-reads.md`: `spec symbols` and `find_symbols` show it. No `ra_ap_*` in `spec`, MCP or the daemon (ADR-0049).

Follow-ups on measured triggers: **11b** `layer-c-macros` (attribute expansions; owner-started project code on a scratch copy) when TE-6 finds > 1 % of a pilot's items under attribute macros, the figure taken from `specengine-eval ra` with and without the proc-macro server (protocol v1 carries no attribute data, A9); **11c** `layer-c-registration` (Bevy registration by resolved types) after 10b (parked 2026-10-16; resumes with it), gated by `[code] frameworks` listing `bevy`.

## Description and interactions

Delivered: `specengine-ra --protocol 1`, the `[[bin]]` of `crates/specengine-ra` (no `specengine-*` normal dependency, outside `default-members`); `specengine_ra::ItemRecord` gains `start`, `end` (`specengine_code` has an `ItemRecord` too: never confused); `specengine-code` the kind map only, B4's dev-dependency; eval the graph and licence tests. Writes: the request's `target_dir`, `tmp_dir`; nothing under `root`. How it works now: `docs/canon/layer-c-protocol.md`; the library: `crates/specengine-ra/README.md`.

- **Caller**: `spec resolve` (11a-2) only. By hand, for a look: `{ cat request.json; cat; } | specengine-ra --protocol 1`, stdin held open (`< request.json` alone reaches EOF, the caller-gone watchdog: the run ends after `hello`). A8: a watchdog ends the group only when the binary leads it: started by a shell with job control alone or as a pipeline's first command, it leads its job; later in a pipeline, or without job control, it exits alone, 3.
- **Live-spec lines** rewritten at shipping: 04:45 (`ra_ap_*` in the executable), 04:187 (the `scip` crate not taken: descriptors come from `ra_ap_ide`). 05:123, 07, 08 belong to 11a-2 and 11a-3.

## Data

Protocol v1 as built is canon: `docs/canon/layer-c-protocol.md` "Run", "Request", "Reply", "Order", "Endings", "Environment", "Watchdogs", "Read-only", "Kind map". Below, what the build refined against this spec's draft, each accepted in review 1.

### Protocol v1

- **Request**: also a clean absolute `root` of a directory, canonicalised; "under the root" judged as written and resolved; each file a regular file; `budget_ms` ≥ 1; a line ≤ 64 MiB; both directories created, else refused; `files: []` allowed.
- **Order**: workspaces by the byte order of the files' nearest-manifest directories, each claiming its members, numbered from 1; `dir` may hold `..`; the `file` lines after all workspaces.
- **Endings**: exit 3 outside the protocol, no line (a watchdog or a closed stdout while not leading the group, an internal failure); once `done` or `error` is written, a firing watchdog exits with its code, silently (while it is being written, it waits up to 1 s); `path` for a requested file unreadable or not UTF-8, inside a loaded workspace too: all of a workspace's requested files are read before any is scanned, the first bad one in request order named (`workspace` N after its steps); `workspace` 0 for a file under no manifest, right after `hello` only when no requested file has a manifest, else after every workspace's steps; a rust-analyzer panic `metadata` while discovering, `database` while loading or scanning; `peak_rss` null where `ru_maxrss`'s unit is unknown.

### Binary

The scrub, `TMPDIR` = `tmp_dir`, the load, the item spans, the stamp and the watchdogs: canon "Environment", "Read-only", "Reply", "Watchdogs". Known limits (L5), canon "Read-only": a project's `build.rustc` and `[env]`; `CARGO_BUILD_RUSTC` in the caller's environment. No watchdog waits on the request line: the caller's budget covers it. Also known limits there: membership by cargo's spelling of member paths (a symlinked alias directory: cargo's `metadata` refusal); a non-UTF-8 module file that is not requested: `database`, its stderr naming the scanned file and a `FileId`, not the culprit.

### Kind map

`specengine-code` `layer_c` (the join's, 11a-2's, and B4's): `method` -> `function_item`, `function_signature_item`; `type_alias` -> `type_item`; `mod` -> `mod_item`; `macro_rules`, `macro_def` -> `macro_definition`; else `<kind>_item`. Canon "Kind map".

### Dependencies and install

Exactly 16 direct normal dependencies: nine `ra_ap_*`, four pins (`salsa`, `salsa-macros`, `salsa-macro-rules`, `unicode-ident`), `serde`, `serde_json`, `libc`, all three already pinned, none added; no `ra_ap_hir` (Q12). Dev: `specengine-code`. Licences: the daemon's set plus `CC0-1.0` (Q10, `stack.md`). Install (Q8): canon "Run".

## Rules and edge cases

- WHEN argv or the request is wrong, the binary SHALL write one `refused` line and exit 2 before discovering anything.
- WHEN stdin closes, or `budget_ms` + 10 s passes, its process group SHALL be gone within 5 s when it leads the group; else it exits alone (A8); after `done` or `error`, with that line's code.
- The binary SHALL run no build script, proc macro or `cargo check`, write nothing under `root`, and ignore the caller's toolchain overrides. Known limits (L5): a project's `build.rustc` and `[env]`, `CARGO_BUILD_RUSTC`.
- A file in two crates gets one's answer [A]. Genre (`architecture.md#universal`): SCIP text is data in `specengine-ra` and `specengine-code` only.

## Acceptance criteria

M: mutation -> the test turning red. Real binary: one test file, `crates/specengine-ra/tests/protocol.rs`, on scratch copies of `fixtures/ra-mini` (each integration test links `ra_ap`). A shim: a `cargo` and a `rustc` first on `PATH` and in a scratch `$CARGO_HOME/bin`.

- [x] X0 -- frozen bytes (08 s2): `git diff --stat 5b79b81 -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing; `git diff 5b79b81 -- Cargo.lock` is exactly four added lines in the `specengine-ra` block, `"libc"`, `"serde"`, `"serde_json"`, `"specengine-code"`: no tree-sitter line, no `version =` line. M: a `cargo update`.
- [x] B1 -- graph, licence (eval `build_graph.rs`). M: `ra_ap_hir` added -> new `specengine_ra_direct_dependencies_are_exactly_the_pinned_set`; CC0 dropped -> new `every_package_of_the_ra_binarys_normal_graph_is_licensed_within_the_allowed_set` (red on `notify` 8.2.0; asserts `ALLOWED_LICENCES` has no CC0); a CLI dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; a default member -> `default_members_graph_has_no_rust_analyzer`.
- [x] B2 -- `reply_follows_protocol_v1`: `hello` first, then the `step` lines; one `file` line per requested path, in request order, items by `start`; `done` last, counts equal, exit 0; the copy byte-identical, no `target/`. M: `done.items` counted before the `scan_unloaded` files. `reply_equals_the_masked_golden`: the reply, `ms`, `peak_rss`, `toolchain`, `host`, `sysroot_packages` masked, equals `fixtures/layer-c/ra-mini.jsonl`, which 11a-2's fakes replay (A6). M: items in moniker order.
- [x] B3 -- `ra_mini_states_match_the_fixture`: `stray_helper`, `windows_overlay` `unresolved`; `tools/probe.rs` `in_crate` false, every item `not_loaded`; `mod tests` `moniker`; the two `impl Health` share one raw moniker; a lib and a same-name bin (added to the scratch copy only) both answered. M: `set_test: false`.
- [x] B4 -- `item_bytes_equal_layer_a`: each item's `(start, end, kind)` equals layer A's row on the same bytes (kind map above), over `#[derive]`, `///`, a marker line above, a CRLF copy. M: `start` from the syntax node's start.
- [x] B5 -- `bad_request_is_refused_before_discovery`, a logging shim: `--protocol 2`; files unsorted, repeated, absolute or with `..`; an unknown key; `target_dir` or `tmp_dir` under the root: each one `refused` line, exit 2, the shim's log empty. M: the request checked after discovery.
- [x] B6 -- under `alarm`, a shim forking a sleeping grandchild: stdin closed -> the group gone within 5 s (M: no stdin watchdog -> `binary_kills_its_group_when_stdin_closes`); `budget_ms` 1000 -> gone by 11 s plus slack (M: no budget watchdog -> `binary_kills_its_group_after_budget_and_grace`); the status assertion accepts SIGKILL or exit 3 when it leads its group (on macOS the group kill can land after `kill` returns; fixed in 11a-2, R13), every group-gone check strict.
- [x] B7 -- read-only: a sentinel `build.rs` never runs, no `target/` under the root (M: a `run_build_scripts()` call added before `into_workspace` -> `no_build_script_runs`); the seven variables pointed at logging shims stay unlogged (M: scrub off -> `toolchain_overrides_are_scrubbed`); SW refinement for L5: a project `.cargo/config.toml` naming a logging shim as `build.rustc-wrapper` and `rustc-workspace-wrapper` stays unlogged (M: wrappers not set to `""` -> `project_wrappers_are_emptied`).
- [x] B8 -- TE-3, binary level: `monikers_survive_an_unrelated_use_edit` (M: a moniker from display text that follows imports, R2).
- [ ] B8 move -- a crate-directory move recorded in "Implementation" (decides on an own impl descriptor later): open, not run.
- [x] B9 -- eval `ra_cli.rs` with `--features ra` green, once at the end; `cargo tree -p specengine-cli -e normal` has no `ra_ap`.
- [x] B10 -- reviews 1 and 2, iterations 2-3. The guard: `closing_stdin_after_done_exits_0` (100 rounds of `files: []`), `closing_stdin_after_error_exits_1` (20 rounds): stdin closed right after the final line -> exit 0, 1, never 3 or a signal (M9: the guard off, 89 of 100 and 7 of 20 rounds red: caught in practice, not by construction). `path`: `non_utf8_file_in_a_loaded_workspace_ends_with_a_path_error` (`arena/src/latin1.rs`, outside every module tree) and `non_utf8_module_after_a_requested_sibling_ends_with_a_path_error` (`mod zlatin;` in `arena/src/lib.rs`, files `arena/src/lib.rs`, `arena/src/zlatin.rs`): each `{"t":"error","workspace":1,"category":"path"}`, exit 1, no `done`. M10: the UTF-8 check in `run.rs` `read_text` removed -> both red; M11: one read-then-scan loop per file -> the sibling test red, the other green as control.
- [ ] Owner -- install time, binary size, `target.noindex` growth recorded: open (debug figures in "Implementation").

spec-writer:

- [x] SW-1 -- this spec; ADR-0049; refreshed and split 2026-10-19; 08 s2.
- [x] SW-2 -- 2026-10-19, at shipping: `crates/specengine-ra/README.md`; new Tier 2 `docs/canon/layer-c-protocol.md` (protocol v1; `layer-c.md` left to 11a-2's resolve, tables, freshness) and its line in `docs/index-canon.md`; `stack.md` (`libc`, the ra licence set); `architecture-code.md#code-identity` an "As built" pointer; the code README (the `layer_c` row; the Bevy row and limit cut to pointers); 04:45, :187; 08 s2; `CLAUDE.md` State.

## Out of scope

Every `spec` command, the store, MCP, plugin (11a-2, 11a-3); proc macros, expansions, attribute data in the protocol (11b); resolved-type registration (11c); daemon, background, watcher, warm process; Windows; a memory guard; `[code] crates`.

## Open

| Q | Working answer | The other answer costs |
|---|---|---|
| Q1 process | `resolve` spawns `specengine-ra`, a pure function; `spec` stores | exec: the binary links the CLI, writes the index |
| Q6 project code | none in 11a; 11b owner-started, scratch | project code under SpecEngine's name |
| Q7 cost | 8-10 s, ≤ 3.2 GiB, cold; reported | memory probes; a warm daemon process |
| Q8 install | optional third binary, same commit | `ra_ap_*`, rustc 1.98 for every user |
| Q9, Q13 | 11a small; 11b, 11c on triggers | 3x unmeasured surface, or nothing |
| Q10 licence | daemon set + `CC0-1.0` (new dependencies owner-approved in principle) | `notify` fails, or drift unseen |
| Q12 | no `ra_ap_hir` | an unused edge |
| L1 split | three parts, TE-6 between 11a-2 and 11a-3 | two: 11a-2 with ~13 of 16 criteria, MCP and plugin churn before TE-6 |
| L4 scrub | the binary, first in `main`; `spec` passes its environment plus `TMPDIR` | seven Cargo literals in the CLI (a genre exemption); by-hand runs keep overrides |
| L5 wrappers | both set to `""` for every toolchain command; `build.rustc`, `[env]`, `CARGO_BUILD_RUSTC` known limits | resolve may run a program the project names |

Q1-Q13: 2026-10-10, under the owner's pre-approval, review open. L1-L6: owner pre-approved 2026-10-19, review open. Numbers shared across the three parts; each row lives in the part that owns it (L2: `layer-c-reads.md`; L3, L6, Q2-Q5, Q11: `layer-c-resolve.md`). WA-n: the 2026-10-10 analysis; A-n, drift n: the 2026-10-19 refresh; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needed** (shipped): 6a-1..3, 6b-1, 8a-1, 10a-1/2, 9a-1.
- **Order**: 11a-1 (this, shipped) -> 11a-2 -> the owner's TE-6 -> 11a-3, unless the owner parks 11a-3 on TE-6's figures; only while 5a-2's TE-1 is outstanding (A1); 5b-1 takes over at any boundary. Interplay: 6b-2's plugin version (L2); 10b parked (its `INDEX_FORMAT` becomes 12).

## Implementation

Developer 3 iterations; reviews 1-3 accepted, no blocker or major. Iteration 2: the last-line guard, `path` for non-UTF-8 files in a loaded workspace; iteration 3: every requested file read before any scan, the pipeline wording; the guard-CAS and private-`Ending` nits left optional.

| Module | What it does |
|---|---|
| `specengine-ra` `src/main.rs` | the scrub, argv, request, `TMPDIR`, the watchdogs, `terminate`, `hello`, the analysis thread (64 MiB stack) |
| `src/request.rs` | `check_argv`, `read_line`, `parse`, `create_dirs` |
| `src/reply.rs` | the line types, `t` first; `emit`; `emit_done`, `emit_error` behind the `LAST_LINE` mark; `last_line` |
| `src/run.rs` | nearest manifests, workspaces in turn, `read_text` of every claimed file before `scan_in`, files parsed alone, `file` lines, `done`; panics to `metadata`/`database` |
| `src/sys.rs` | `peak_rss`, `leads_its_group`, `kill_own_group` (`libc`) |
| `src/scan.rs`, `load.rs` | `ItemRecord.start`/`end` (`LEADING`, `Span`); `Project::workspace_root`, `member_manifests`, `Workspace::file_at` |
| `specengine-code` `src/layer_c.rs` | `KIND_MAP`, `layer_a_kinds`, `kind_matches` |
| manifests, `Cargo.lock` | `[[bin]]`; `serde`, `serde_json`, `libc`; dev `specengine-code`; four lock lines |
| tests | `specengine-ra/tests/protocol.rs`; `fixtures/layer-c/ra-mini.jsonl` (10 lines, 6 files, 29 items; `ms`, `peak_rss`, `sysroot_packages` 0, `toolchain`, `host` "masked"); eval `build_graph.rs` +2 |

Figures (macOS, debug): binary 120 MB; first build 22 s, 175 crates (32 `ra_ap_*`); `target.noindex` 1.2 → 4.0 GiB (6.3 after the tests); a ra-mini run ≈ 21 s quiet, 50 s loaded, `peak_rss` 545-593 MB. `protocol.rs` 17 tests, all on the real binary but the masker's, 87.5 s (3 threads); `build_graph` 40, `doc_pointers` and `anonymity` 15; B9 12 tests, 105 s. Release install: the owner's, open.

Deviations, accepted in review ("Data" above, canon):

1. The by-hand form holds stdin open: EOF is the caller gone.
2. Exit 3 outside the protocol; a leader dies by SIGKILL; after `done`/`error`, that code.
3. Workspace order by nearest-manifest directory, `dir` with `..`.
4. More request checks, each a refusal.
5. `path` for unreadable or non-UTF-8 files; panics `metadata`/`database`.
6. `peak_rss` null off macOS and Linux. 7. `TMPDIR` = `tmp_dir`.
8. B7's draft mutation `proc_macros: true` could not turn red (no build-script call exists); reworded.
9. Symlinked manifest dirs not canonicalised: cargo judges membership by the listed spelling; a known limit.
10. Found in final verification, iterations exhausted: a leading binary's watchdog may end with exit 3, not SIGKILL (its `exit` beats the group kill: 18 of 25 runs under load, 6 of 6 alone); the group is gone either way. Fix carried to 11a-2 (R13); B6 accepts both meanwhile.

Mutations: 11 distinct over 12 runs, all red: M1-M8 (B2-B7), M9 the guard, M10 the UTF-8 check (iterations 2 and 3), M11 read-then-scan. Not run: wrappers not emptied, `ra_ap_hir`, CC0 dropped, golden item order, `cargo update`.
