---
class: spec
status: draft
scope: [crates/specengine-ra, crates/specengine-code, crates/specengine-eval]
ref: Phase 3 plan 2026-10-10, slice 11a of layer C (WA-1..WA-13; Q1-Q13 decided under the owner's pre-approval, review open); refreshed and split 2026-10-19 against 5b79b81 (drift 1-31; L1-L6 owner pre-approved 2026-10-19, review open) into 11a-1 (this), `layer-c-resolve.md` (11a-2), `layer-c-reads.md` (11a-3); 04 s1.7; 05 s5.1; 08 s2, AC-10
adrs: [ADR-0049]
---

# layer-c (11a-1): rust-analyzer in its own executable, protocol v1

## Why

Layer A names items by a heuristic and says where it is unsure (`mod x;`, stray files, `#[path]`: `code-identity.md` "Known limits"); the marker carries the link (ADR-0020). Layer C, measured in Phase 0 (05 s5.1: cold 8-10 s, ≤ 3.2 GiB without proc macros), is called only by `specengine-eval --features ra`. Insurance, built small: on the pilots A plus 6a's discriminator leave almost nothing open (WA-13).

Slice 11a, in three parts (L1): **11a-1** (this) `specengine-ra` gains an executable speaking protocol v1, a pure function of request and disk, no proc macros, no `spec` command; **11a-2** `layer-c-resolve.md`: `spec resolve`, the only spawner, stores each item's moniker beside 6a's rows as advisory data; the owner's TE-6 on the pilots; **11a-3** `layer-c-reads.md`: `spec symbols` and `find_symbols` show it. No `ra_ap_*` in `spec`, MCP or the daemon (ADR-0049).

Follow-ups on measured triggers: **11b** `layer-c-macros` (attribute expansions; owner-started project code on a scratch copy) when TE-6 finds > 1 % of a pilot's items under attribute macros, the figure taken from `specengine-eval ra` with and without the proc-macro server (protocol v1 carries no attribute data, A9); **11c** `layer-c-registration` (Bevy registration by resolved types) after 10b (parked 2026-10-16; resumes with it), gated by `[code] frameworks` listing `bevy`.

## Description and interactions

- **Caller**: `spec resolve` (11a-2) only. By hand, for a look: `specengine-ra --protocol 1 < request.json`; not leading its process group, it exits alone (A8).
- **Crates**: `specengine-ra` a `[[bin]]` (`src/main.rs`), no `specengine-*` normal dependency, outside `default-members`; `specengine_ra::ItemRecord` gains `start`, `end` (`specengine_code` has an `ItemRecord` too: never confused). `specengine-code`: the kind map only (Data), B4's dev-dependency. Eval: graph and licence tests. **Writes**: the request's `target_dir`, `tmp_dir`; nothing under `root`.
- **Overridden live-spec lines** (dated notes 2026-10-10 in place, rewritten at shipping): 04:45 (`ra_ap_*` in the executable), 04:187 (the `scip` crate not taken: descriptors come from `ra_ap_ide`). 05:123, 07, 08 belong to 11a-2 and 11a-3.

## Data

### Protocol v1

Request, one stdin line, exactly these keys; `files` byte-sorted, unique, clean root-relative (no absolute path, `..` or `.`); `target_dir`, `tmp_dir` absolute, not under `root`. stdin then stays open: EOF is the caller gone.

```json
{"protocol":1,"root":"/abs/root","files":["arena/src/lib.rs","…","tools/probe.rs"],"target_dir":"/…/demo.ra/target","tmp_dir":"/…/demo.ra/tmp","budget_ms":300000}
```

Reply, JSON lines, key `t` first:

```json
{"t":"hello","protocol":1,"ra_ap":"0.0.352","host":"aarch64-macos"}
{"t":"step","workspace":1,"dir":".","step":"discover","ms":2140,"toolchain":"1.98.0","sysroot_packages":43,"metadata_degraded":false}
{"t":"step","workspace":1,"dir":".","step":"database","ms":6120}
{"t":"file","path":"arena/src/lib.rs","in_crate":true,"items":[{"start":318,"end":402,"kind":"impl","state":"moniker","moniker":"arena impl#[Health]"}]}
{"t":"done","files":6,"items":29,"by_state":{"moniker":26,"local":0,"none":0,"unresolved":2,"not_loaded":1},"peak_rss":2914000000,"ms":9100}
```

- **Order**: `hello`; per workspace by `dir`, its `step` lines; one `file` line per requested path, in request order, items by `start`; `done` last, its counts equal to the `file` lines'.
- **Endings**: `done`, exit 0; `{"t":"error","workspace":n,"category":"path|metadata|build_scripts|database"}`, exit 1; bad argv or request: `{"t":"refused","protocol":<own>,"reason":"…"}`, exit 2, before any discovery. `LoadError` text: stderr only.

### Binary

- **First in `main`** (single-threaded, before the watchdogs; L4, L5): removes eval's seven overrides (`crates/specengine-eval/src/ra/mod.rs`): `RUSTUP_TOOLCHAIN`, `CARGO`, `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_TARGET_DIR`, `CARGO_BUILD_TARGET_DIR`; then sets `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` to `""` (no wrapper, over a project's cargo config), inherited by every toolchain command. rust-analyzer then looks `rustc` up on `PATH`, then in `$CARGO_HOME/bin`; `cargo`, once a sysroot is known, in `$CARGO_HOME/bin` first (`ra_ap_toolchain` `prefer_proxy`); it sets `RUSTUP_AUTO_INSTALL=0` on each: rustup never installs.
- **Load** (Q6): each workspace owning a file (nearest manifest) discovered with `proc_macros: false` (no `cargo check`; `cargo metadata --locked --offline` on a copied lock), loaded, scanned, closed in turn, by `dir`, one at a time; files under no package: `scan_unloaded` (`in_crate` false, every item `not_loaded`). `cfg(test)` on.
- **Items**: `start`, `end` bytes from the first token after outer attributes and comments, as layer A's, on the raw text (load-cargo keeps CRLF; so does the code index); `line` (attributes included) stays for eval; `kind` `ItemKind::as_str`, `state` `MonikerStatus::as_str`; `moniker` crate plus SCIP descriptors, no version (`scan.rs` `render`), else null.
- **Stamp**: `host` `std::env::consts` arch-os; `peak_rss` `getrusage` `ru_maxrss`, `RUSAGE_SELF` plus `RUSAGE_CHILDREN`, in bytes (KiB × 1024 on Linux); `ms` wall time.
- **Watchdogs** (eval's): stdin EOF, or `budget_ms` + 10 s: `kill(0, SIGKILL)` when it leads its group, else exit alone (A8).

### Kind map

`specengine-code` (the join's, 11a-2's, and B4's): `method` -> `function_item`, `function_signature_item`; `type_alias` -> `type_item`; `mod` -> `mod_item`; `macro_rules`, `macro_def` -> `macro_definition`; else `<kind>_item`.

### Dependencies and install

- **Direct normal dependencies**, exactly 16: nine `ra_ap_*`, four pins (`salsa`, `salsa-macros`, `salsa-macro-rules`, `unicode-ident`), `serde`, `serde_json`, `libc`; all three already pinned in `[workspace.dependencies]`, none added; no `ra_ap_hir` (Q12). Dev: `specengine-code`. Licences: the daemon's set plus `CC0-1.0` (Q10).
- **Install** (Q8): `cargo install --locked --force --path crates/specengine-ra`, beside `spec`, from one commit; rustc ≥ 1.98 with `rust-src` (without it, answers degrade, stamped).

## Rules and edge cases

- WHEN argv or the request is wrong, the binary SHALL write one `refused` line and exit 2 before discovering anything.
- WHEN stdin closes, or `budget_ms` + 10 s passes, its process group SHALL be gone within 5 s.
- The binary SHALL run no build script, proc macro or `cargo check`, write nothing under `root`, and ignore the caller's toolchain overrides. Known limits (L5): a project's `build.rustc` and `[env]`.
- A file in two crates gets one's answer [A]. Genre (`architecture.md#universal`): SCIP text is data in `specengine-ra` and `specengine-code` only.

## Acceptance criteria

M: mutation -> the test turning red. Real binary: one test file, `crates/specengine-ra/tests/protocol.rs`, on scratch copies of `fixtures/ra-mini` (each integration test links `ra_ap`). A shim: a `cargo` and a `rustc` first on `PATH` and in a scratch `$CARGO_HOME/bin`.

- [ ] X0 -- frozen bytes (08 s2): `git diff --stat 5b79b81 -- crates/specengine-code/src/levels.rs crates/specengine-code/src/hash.rs crates/specengine-code/src/ron/` prints nothing; `git diff 5b79b81 -- Cargo.lock` is exactly four added lines in the `specengine-ra` block, `"libc"`, `"serde"`, `"serde_json"`, `"specengine-code"`: no tree-sitter line, no `version =` line. M: a `cargo update`.
- [ ] B1 -- graph, licence (eval `build_graph.rs`). M: `ra_ap_hir` added -> new `specengine_ra_direct_dependencies_are_exactly_the_pinned_set`; CC0 dropped -> new `every_package_of_the_ra_binarys_normal_graph_is_licensed_within_the_allowed_set` (red on `notify` 8.2.0; asserts `ALLOWED_LICENCES` has no CC0); a CLI dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; a default member -> `default_members_graph_has_no_rust_analyzer`.
- [ ] B2 -- `reply_follows_protocol_v1`: `hello` first, then the `step` lines; one `file` line per requested path, in request order, items by `start`; `done` last, counts equal, exit 0; the copy byte-identical, no `target/`. M: `done.items` counted before the `scan_unloaded` files. `reply_equals_the_masked_golden`: the reply, `ms`, `peak_rss`, `toolchain`, `host`, `sysroot_packages` masked, equals `fixtures/layer-c/ra-mini.jsonl`, which 11a-2's fakes replay (A6). M: items in moniker order.
- [ ] B3 -- `ra_mini_states_match_the_fixture`: `stray_helper`, `windows_overlay` `unresolved`; `tools/probe.rs` `in_crate` false, every item `not_loaded`; `mod tests` `moniker`; the two `impl Health` share one raw moniker; a lib and a same-name bin (added to the scratch copy only) both answered. M: `set_test: false`.
- [ ] B4 -- `item_bytes_equal_layer_a`: each item's `(start, end, kind)` equals layer A's row on the same bytes (kind map above), over `#[derive]`, `///`, a marker line above, a CRLF copy. M: `start` from the syntax node's start.
- [ ] B5 -- `bad_request_is_refused_before_discovery`, a logging shim: `--protocol 2`; files unsorted, repeated, absolute or with `..`; an unknown key; `target_dir` or `tmp_dir` under the root: each one `refused` line, exit 2, the shim's log empty. M: the request checked after discovery.
- [ ] B6 -- under `alarm`, a shim forking a sleeping grandchild: stdin closed -> the group gone within 5 s (M: no stdin watchdog -> `binary_kills_its_group_when_stdin_closes`); `budget_ms` 1000 -> gone by 11 s plus slack (M: no budget watchdog -> `binary_kills_its_group_after_budget_and_grace`).
- [ ] B7 -- read-only: a sentinel `build.rs` never runs, no `target/` under the root (M: `proc_macros: true` -> `no_build_script_runs`); the seven variables pointed at logging shims stay unlogged (M: scrub off -> `toolchain_overrides_are_scrubbed`); SW refinement for L5: a project `.cargo/config.toml` naming a logging shim as `build.rustc-wrapper` and `rustc-workspace-wrapper` stays unlogged (M: wrappers not set to `""` -> `project_wrappers_are_emptied`).
- [ ] B8 -- TE-3, binary level: `monikers_survive_an_unrelated_use_edit` (M: a moniker from display text that follows imports, R2); a crate-directory move recorded in "Implementation" (decides on an own impl descriptor later).
- [ ] B9 -- eval `ra_cli.rs` with `--features ra` green, once at the end; `cargo tree -p specengine-cli -e normal` has no `ra_ap`.
- [ ] Owner -- install time, binary size, `target.noindex` growth recorded.

spec-writer:

- [x] SW-1 -- this spec; ADR-0049; refreshed and split 2026-10-19; 08 s2.
- [ ] SW-2 -- at shipping: `crates/specengine-ra/README.md` (5 796 of 10 240: the binary, a protocol pointer, dependencies, tests); a new Tier 2 `docs/canon/layer-c.md` with protocol v1 (else `layer-c-protocol.md`), its line in `docs/index-canon.md` (9 166 of 10 240: measure; over the cap, a shard claim); `stack.md` (`libc`, the ra licence set); `architecture-code.md:30` an "As built" pointer; the code README (10 101: the kind map's row, room from the Bevy row and limit cut to pointers); 04:45, :187 rewritten; `CLAUDE.md` State.

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
| L5 wrappers | both set to `""` for every toolchain command; `build.rustc`, `[env]` known limits | resolve may run a program the project names |

Q1-Q13: 2026-10-10, under the owner's pre-approval, review open. L1-L6: owner pre-approved 2026-10-19, review open. Numbers shared across the three parts; each row lives in the part that owns it (L2: `layer-c-reads.md`; L3, L6, Q2-Q5, Q11: `layer-c-resolve.md`). WA-n: the 2026-10-10 analysis (`scan.rs`, `load.rs`, eval's `ra-worker`, `ra_ap_ide` `moniker.rs`); A-n, drift n: the 2026-10-19 refresh; "SW refinement": the spec-writer's.

## Risks

- **Laptop build**: only this part compiles `ra_ap`: `-p specengine-ra`, eval `--features ra` once, never `--workspace`; a clean debug build 2-4 min, each test executable 0.2-0.5 GB (one file); install 3-6 min, +1-2 GB, a 40-80 MB binary [A]; `cargo clean` at the boundary.
- **rustc**: 1.98 here against the workspace's 1.90; no `rust-toolchain.toml`. CI builds only the CLI `--locked`, so this part commits `Cargo.lock`: hold `=0.0.352`, never `cargo update`, review the lock diff.
- Licences beyond `notify` unmeasured until B1; `display` text may follow imports (R2, B8); cfg set, host, sysroot and degraded loads change answers (stamped); no memory guard (~3.2 GiB on the heavier pilot); value may be ~0 open rows (WA-13).

## Dependencies and ordering

- **Needs** (shipped): 6a-1..3, 6b-1, 8a-1, 10a-1/2, 9a-1.
- **Order**: 11a-1 (this) -> 11a-2 -> the owner's TE-6 -> 11a-3, unless the owner parks 11a-3 on TE-6's figures; only while 5a-2's TE-1 is outstanding (A1); 5b-1 takes over at any boundary. Interplay: 6b-2's plugin version (L2); 10b parked (its `INDEX_FORMAT` becomes 12).

## Implementation

Not started; when filled, ≤ 3 072 B. To record: modules, the golden, build and install figures, B8's move, deviations with reasons.
