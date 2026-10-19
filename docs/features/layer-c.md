---
class: spec
status: draft
scope: [crates/specengine-ra, crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-core, crates/specengine-eval]
ref: Phase 3 plan 2026-10-10, slice 11a of layer C (WA-1..WA-13; Q1-Q13 decided under the owner's pre-approval, review open); 04 s1.7; 05 s5.1; 07 s2, s5; 08 s2, AC-10
adrs: [ADR-0049]
---

# layer-c: rust-analyzer in its own executable, monikers as advisory index data

## Why

Layer A names items by a heuristic and says where it is unsure (`mod x;`, stray files, `#[path]`: `code-identity.md` "Known limits"); the marker carries the link (ADR-0020). Layer C, measured in Phase 0 (05 s5.1: cold 8-10 s, ≤ 3.2 GiB without proc macros), is called only by `specengine-eval --features ra`.

**11a** (this): `specengine-ra` gains an executable speaking protocol v1, no proc macros; only `spec resolve` spawns it; `spec` stores each item's moniker beside 6a's rows as advisory data, shown by `spec symbols`. No `ra_ap_*` in `spec`, MCP or the daemon. ADR-0049. Insurance, built small: on the pilots A plus 6a's discriminator leave almost nothing open (WA-13).

Follow-ups on measured triggers: **11b** `layer-c-macros` (attribute expansions; owner-started project code on a scratch copy) when TE-6 finds > 1 % of a pilot's items under attribute macros; **11c** `layer-c-registration` (Bevy registration by resolved types) after 10b, for what the dump cannot give, gated by `[code] frameworks` listing `bevy` (10a-1).

## Description and interactions

- **Owner** or an agent: `spec resolve [--timeout S] [--json]`, one cold load; `spec symbols` then shows a `c` line per row. Not `id_state resolved`.
- **Never implicit** (WA-3): no read, MCP tool, daemon route, hook, gate, `index`, `check`, `verify`, `lock` spawns the binary; outputs unchanged but `symbols`' JSON keys.
- **Crates**: `specengine-ra` a `[[bin]]`, no `specengine-*` dependency, outside `default-members`; CLI `resolve`, `symbols`; store tables; `specengine-code` moniker functions; core drops `scip`; eval tests. **Writes**: the data directory only.
- **Overridden live-spec lines** (dated notes 2026-10-10, rewritten at shipping): 05:123 ("In the daemon … debounce"), 04:45 ("Call the CLI, do not link `ra_ap_*`"), 07:205 (`scip`); 07 s2 lists `spec resolve`.

## Data

### spec resolve

WA-1, WA-2, Q1-Q3. S: seconds 1..=3600, default 300. In order, each failure exit 2:

1. Not Unix; usage, discovery, config, no `[code]`, a walk gap (6a's reads, which refresh the indexes first). `files`: the walked UTF-8 `.rs` files; `tree_fp` computed.
2. `flock` on `<data dir>/<slug>.ra/lock`, try-exclusive; `target/`, `tmp/` beside it.
3. Lookup (`spec serve`'s): beside `current_exe()`, else the first `PATH` entry with an executable regular `specengine-ra`.
4. Spawn, never exec: `--protocol 1`, cwd and `TMPDIR` `tmp/`, own process group, env minus eval's list (`RUSTUP_TOOLCHAIN`, `RUSTC`, `RUSTC_WRAPPER`, `CARGO_TARGET_DIR`, …); stderr copied live; stdin gets the request, stays open.
5. stdout read to `done`, each line checked; at S: `SIGTERM` to the group, 5 s, `SIGKILL` (8a-1's runner, `spec-verify-tests.md` "Runner").
6. After `done` and exit 0: code index refreshed, `tree_fp` recomputed; equal: one Immediate transaction replaces the worktree's rows; else nothing stored.

### Protocol v1

Request, one stdin line; `files` byte-sorted, unique, clean root-relative:

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

- **Binary**, a pure function of request and disk (Q6): each workspace owning a file (nearest manifest) discovered with `proc_macros: false` (no `cargo check`; `cargo metadata --locked --offline` on a copied lock), loaded, scanned, closed in turn, by `dir`; files under no package: `scan_unloaded`. Items in start order; `start`, `end` bytes from the first token after outer attributes and comments, as tree-sitter's (`ItemRecord` gains them; its `line`, attributes included, stays for eval); `kind` `ItemKind::as_str`, `state` `MonikerStatus::as_str`; `host` `std::env::consts` arch-os; `moniker` crate plus SCIP descriptors, no version (`scan.rs` `render`), else null. `LoadError` text: stderr only.
- **Endings**: `done`, exit 0; `{"t":"error","workspace":n,"category":"path|metadata|build_scripts|database"}`, exit 1; bad argv, request or protocol: `{"t":"refused","protocol":<own>,"reason":"…"}`, exit 2. Watchdogs (eval's): stdin EOF or `budget_ms` + 10 s: `SIGKILL` to its group.
- **`spec` checks**: `hello` first, `protocol` 1; each requested file once, no other; items ordered, within the file; `done` last, counts equal; lines ≤ `REPLY_LINE_CAP` 8 388 608 B, JSON, known keys. Else malformed.

### Tables

Q4, Q5. Keyed by worktree, in `FORMAT_TABLES`; `INDEX_FORMAT` +1, its `format_history.txt` line (a format change drops rows, `--full` keeps them). From a run: outside 6a's dump-equals-rebuild; `dump.rs` maps their keys.

| Table | Key | Columns |
|---|---|---|
| `code_resolve` | `(wt)` | `tree_fp`, `protocol`, `ra_ap`, `host`, `toolchains` (JSON), `degraded`, `workspaces`, `files`, `items`, `c_only` |
| `code_resolve_files` | `(wt, path)` | `blake3` (6a's at the run), `in_crate` |
| `code_monikers` | `(wt, path, start)`; `moniker` indexed | `end`, `kind`, `state`, `moniker` (raw) |

Join, `agrees`, disambiguator: per read (SW refinement: a 6a rule bump renames qpaths, not C's facts).

### Freshness and states

WA-5. `tree_fp` = BLAKE3, byte-sorted: protocol, `ra_ap`, `host`; each walked `.rs` `(path, blake3)`; each `code_packages` `(dir, manifest_b3)` (SW refinement: a renamed crate renames every moniker); the bytes of each `Cargo.lock`, `rust-toolchain(.toml)`, `.cargo/config(.toml)` in the root, a package dir or between. Snapshot `absent`, `fresh` (equal), `stale` (any change: every row stale, old values shown).

| 6a row's `c_state` (WA-4) | When |
|---|---|
| `absent` | no snapshot |
| `unseen` | file not in `code_resolve_files` or its BLAKE3 changed (SW refinement: offsets moved) |
| `moniker`, `local`, `none`, `unresolved`, `not_loaded` | a `code_monikers` row at its `(path, start byte)`, kind matching |
| `unmatched` | file seen unchanged, no such row |

- **Kinds** (`specengine-code`): `method` -> `function_item`, `function_signature_item`; `type_alias` -> `type_item`; `mod` -> `mod_item`; `macro_rules`, `macro_def` -> `macro_definition`; else `<kind>_item`. Unjoined C rows: `c_only`, counted.
- **Moniker**: raw; shared by ≥ 2 C rows of a file: ` #<value>`, the joined rows' 6a discriminators if all distinct (`arena impl#[Health] #2`), else the ordinal by start. Raw text never holds ` #`.
- **`agrees`** (`moniker` rows, else null): C's crate equals A's target crate (`code_packages.targets`, `-` -> `_`) and C's leading `<name>/` descriptors equal A's modules; names, owners not compared.
- **Precedence**: the marker binds; the qpath names for `spec.lock`, `verify` (key `(file, kind, qpath)` unchanged); the moniker displays and finds. `agrees` false: counted, shown, never fixed or a finding.

### Output and exits

`spec resolve`, exit 0:

```
layer c: specengine-ra protocol 1, ra_ap 0.0.352, host aarch64-macos
workspace 1 .: discover 2140 ms (toolchain 1.98.0, sysroot 43, degraded no), database 6120 ms
files 6 (in crate 4), items 29: moniker 26, local 0, none 0, unresolved 2, not_loaded 1
joined 28, unmatched 1, c_only 1; agrees 25, disagrees 1; collisions 1; stored fresh, 9.1 s, peak 2.7 GiB
```

JSON `{protocol, ra_ap, host, workspaces: [{dir, toolchain, sysroot_packages, metadata_degraded, discover_ms, database_ms}], files, in_crate, items, by_state, joined, unmatched, c_only, agrees, disagrees, collisions, ms, peak_rss}`; time and peak never stored or guarded (Q7). Exit 2: stdout empty, `spec resolve: <reason>; the previous snapshot is kept`; reasons beyond step 1's: `specengine-ra not found beside <spec> or on PATH; install it: cargo install --locked --force --path <clone>/crates/specengine-ra (rustc >= 1.98, rust-src)`; `specengine-ra speaks protocol <n>, spec needs 1: install both from one commit`; `a layer C run of <slug> is in progress`; `over the budget of <S> s`; `load failed: <category>`; `specengine-ra ended (<status>) before done`; `malformed reply at line <n>`; `the tree changed during the run`.

`spec symbols`, while a snapshot exists:

```
arena::impl Health #2 | impl_item | arena/src/lib.rs:23-27 | layer a | mentions 0 | exact
  sig impl Health
  c moniker | arena impl#[Health] #2 | agrees | stale
symbols: 1 of 1; code files 6 (.rs 6, .ron 0), skipped 0
layer c: stale (ra_ap 0.0.352); spec resolve refreshes it
```

- After `sig`: `c <c_state>[ | <moniker>][ | agrees|disagrees][ | stale]`; none when `absent`, so text without a snapshot is 6a's byte for byte.
- Tier `moniker`, after `exact`: the query equals a rendered moniker (raw lookup, ` #<value>` dropped, then filtered); stale answers too.
- JSON: `symbols[]` gains `c_state`, `moniker`, `agrees`; top `layer_c` after `code`: `null`, or the stamp with `state`.

### Config and install

- Q11: no `[code]` key. `scip` (07:205) is retired as 10a-1 retires `bevy`: `NOT_BUILT_KEYS` loses it (`["languages", "crates", "scip"]` -> `["languages", "crates"]` after 10a-1); `scip` takes the unknown-key arm and text (`KEYS` as then); the module doc drops it; `code_refs_walk.rs` `EXEMPT` drops `(…code_toml.rs, "scip")`. `crates` stays refused (6a Q2).
- Q8: `cargo install --locked --force --path crates/specengine-ra`, beside `spec`, same commit; rustc ≥ 1.98, `rust-src`. Direct dependencies: nine `ra_ap_*`, four pins, `serde`, `serde_json`, `libc`; no `ra_ap_hir` (Q12).

## Rules and edge cases

- WHEN the binary is missing, hangs, crashes or overruns, other commands SHALL be unchanged and `resolve` SHALL keep the previous snapshot.
- WHEN any `tree_fp` input changes after a run, every row SHALL read `stale` (each `c` line, `layer_c.state`); WHEN one tree is resolved from two roots, twice or in reversed creation order, rows and `symbols` SHALL be byte-identical.
- A moniker never binds, locks, judges or becomes a finding (ADR-0049). Genre (`architecture.md#universal`): SCIP text is data outside `specengine-code`, `specengine-ra`. A file in two crates gets one's answer [A].

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (CLI `layer_c.rs` on fakes; real binary `-p specengine-ra`):

- [ ] RD-1 -- graph, licence. M: a CLI dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; a default member -> `default_members_graph_has_no_rust_analyzer`; `ra_ap_hir` added -> new `specengine_ra_direct_dependencies_are_exactly_the_pinned_set`; CC0 dropped -> new `every_package_of_the_ra_binarys_normal_graph_is_licensed_within_the_allowed_set` (red on `notify`; asserts `ALLOWED_LICENCES` has no CC0).
- [ ] RD-2 -- lookup. M: `PATH` first -> `layer_c_binary_beside_spec_wins_over_path`; missing exit 0 -> `resolve_without_binary_is_exit_2_with_install_line`.
- [ ] RD-3 -- protocol. M: no version check -> `protocol_mismatch_is_exit_2_and_stores_nothing`; a bad, unknown, repeated or unrequested line, unequal counts -> `malformed_reply_is_exit_2_and_stores_nothing`; no cap -> `oversized_reply_line_is_exit_2`.
- [ ] RD-4 -- group. M: only the child killed -> `no_layer_c_grandchild_outlives_the_budget`; stdin EOF ignored -> `binary_kills_its_group_when_stdin_closes`; no own budget -> `binary_kills_its_group_after_budget_and_grace`.
- [ ] RD-5 -- atomic. M: stored while streaming -> `budget_overrun_keeps_the_previous_snapshot`; no re-hash -> `tree_change_during_run_stores_nothing`; no flock -> `second_resolve_is_refused_while_one_runs`.
- [ ] RD-6 -- M: a read, MCP tool, `index`, `check`, gate, `verify`, `lock` spawning it -> `reads_mcp_check_gate_verify_never_spawn_layer_c` (a logging fake beside a `spec` copy and on `PATH`).
- [ ] RD-7 -- freshness. M: per file -> `edit_in_another_file_marks_every_row_stale`; manifests out -> `manifest_edit_makes_the_snapshot_stale`; `Cargo.lock` out -> `lock_file_edit_makes_the_snapshot_stale`; offsets on a changed file -> `changed_file_rows_are_unseen`; `--full` drops -> `full_reindex_keeps_the_snapshot`.
- [ ] RD-8 -- join. M: attributes in `start`, or by line -> `attributed_item_joins_its_row`; by name -> `same_name_items_join_by_offset`; kinds ignored -> `boundary_mismatch_is_unmatched_and_c_only`.
- [ ] RD-9 -- M: a moniker in lock, verify, refs, bundle, check -> `lock_verify_refs_bundle_check_equal_with_and_without_a_snapshot`.
- [ ] RD-10 -- M: no suffix -> `adjacent_inherent_impls_get_distinct_monikers`; a unique one suffixed -> `unique_moniker_is_unchanged`.
- [ ] RD-11 -- read-only. M: target under the root -> `resolve_leaves_the_worktree_untouched`; `proc_macros: true` -> `no_build_script_runs` (sentinel `build.rs`); env kept -> `resolve_scrubs_toolchain_overrides`.
- [ ] RD-12 -- M: outside `FORMAT_TABLES` -> 6a's `format_change_recreates_code_tables_and_keeps_queue_rows`.
- [ ] RD-13 -- M: hash-map order -> `resolve_rows_are_byte_identical_across_runs_and_roots`.
- [ ] RD-14 -- M: a descriptor split in store or CLI -> `no_scip_or_rust_syntax_outside_code_and_ra`.
- [ ] RD-15 -- output. M: a key dropped -> 6a's `symbols_json_every_key_present`; a `c` line without a snapshot -> `symbols_text_is_unchanged_without_a_snapshot`; no tier -> `moniker_query_finds_its_symbol`; names compared -> `agrees_compares_crate_and_modules_only`.
- [ ] RD-16 -- M: `scip` kept not-built -> `code_table_unknown_key_is_refused` (case `scip = false`), the stack-word scan.

test-engineer:

- [ ] TE-1 -- fakes under `alarm` (ok, wrong protocol, hang, grandchild, flood, crash mid-stream, malformed, missing) beside a hard-linked `spec` copy in a temp dir.
- [ ] TE-2 -- real run on a temp copy of `fixtures/ra-mini`: `stray_helper` (A a module path, C `unresolved`), `windows_overlay` `unresolved` on macOS, `tools/probe.rs` `not_loaded`, `cfg(test)` resolved, two `impl Health` (`#1`, `#2`), a lib and bin of one name; fixture byte-identical, no `target/`.
- [ ] TE-3 -- stability across an unrelated `use` edit and a crate-directory move (decides on an own impl descriptor later).
- [ ] TE-4 -- two roots, two runs, reversed creation: rows, `symbols` byte-identical.
- [ ] TE-5 -- RD-1; eval `ra_cli.rs` green; `cargo tree -p specengine-cli -e normal` has no `ra_ap`.
- [ ] TE-6 -- pilots, manual (`#[ignore]`, release, `--timeout`, scratch `HOME`, read-only, unnamed): cold time, peak; rows per `c_state`; `agrees` false; collisions; `c_only`; items under attribute macros (11b); install time, size.
- [ ] TE-7 -- binary missing, hanging, crashing: `check`, gate, `verify`, `symbols`, `refs`, `bundle`, MCP byte-identical to no binary.

spec-writer:

- [x] SW-1 -- this spec; ADR-0049 with both `#code-identity` sections; the dated notes, 08 :41, :43; `symbol-index.md`'s `layer` note; `export index`, `check` clean.
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/layer-c.md` from `architecture-code.md`; ra, CLI, store READMEs (the last two full: detail down first); `code-bindings.md` (`scip` unknown; full); `stack.md`; root README install; `code-identity.md` "Open"; overridden lines rewritten; 08 AC-10; `CLAUDE.md` State.
- [x] SW-3 -- ADR-0049 ≤ 1 536 B, layer C only, beside ADR-0048's layer B.

## Risks

R1 licences beyond `notify` and the release build time unmeasured until RD-1, TE-6. R2 `display` text may follow imports (TE-3); cfg set, host, sysroot, degraded loads change answers (stamped). R3 `cargo metadata` reads the project's cargo config; rustup may fetch its toolchain [A]. R4 a real binary beside a test's `spec` wins: tests use a copy. R5 no memory guard. R6 value ≈ 0 pilot rows against high scope creep (08:125).

## Out of scope

Rebinding, `spec.lock` keys, `verify` verdicts by moniker (after 5b/5c, own ADR); proc macros, expansion mapping, generated items (11b); resolved-type registration, inference (11c); daemon, background, watcher, warm process; Windows; memory guard; `[code] crates`; `deps` call targets; MCP, HTTP, UI beyond `--json` keys; plugin text.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 process | `resolve` spawns `specengine-ra`, a pure function; `spec` stores | exec: the binary links the CLI, writes the index |
| Q2 protocol | v1 exact, stdin open, JSON lines; 300 s; stored after `done`, tree unchanged | version ranges, half snapshots |
| Q3 failing | only `resolve`: exit 2, install line, snapshot kept | minutes, GiBs per agent read |
| Q4 per row | `c_state`, `moniker`, `agrees` by (file, start); marker > qpath > moniker | lock re-keyed per `ra_ap` bump |
| Q5 freshness | one snapshot; any change: all stale | a parent `mod` edit mislabels a child |
| Q6 project code | none in 11a; 11b owner-started, scratch | project code under SpecEngine's name |
| Q7 cost | 8-10 s, ≤ 3.2 GiB, cold; reported | memory probes; a warm daemon process |
| Q8 install | optional third binary, same commit | `ra_ap_*`, rustc 1.98 for every user |
| Q9, Q13 | 11a small; 11b, 11c on triggers | 3x unmeasured surface, or nothing |
| Q10 licence | a test: daemon set + `CC0-1.0` (new dependencies owner-approved in principle) | `notify` fails, or drift unseen |
| Q11, Q12 | no key, `scip` retired; no `ra_ap_hir` | a dead toggle; an unused edge |
| SW | `unseen`; manifests, cargo config in `tree_fp`; per-read join; tier after `exact` | stored joins rerun per 6a bump |

## Working answers

WA-n: the analysis' (2026-10-10), checked against `scan.rs`, `load.rs`, eval's `ra-worker`, `ra_ap_ide` `moniker.rs`; [A] boundaries align once attributes are skipped, the 1 % trigger. "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs**: 6a `symbol-index` (hard: rows, start bytes, discriminator, `code_packages`, `symbols`), on `code-refs` (shipped). Soft: 8a-1 `verify-tests` (runner, `libc`; else 11a builds both); 6b-1 `code-tools` (MCP mirror); 10a-1 (the second to ship edits `NOT_BUILT_KEYS`, `EXEMPT` from the other's state).
- **Order**: ADR-0049, this spec -> binary and real protocol tests -> CLI spawn on fakes -> tables -> display, lookup -> TE-6. 11b after TE-6's trigger; 11c after 10b; moniker locking after 5b/5c, own ADR. **Independent**: 4a, 5a, 6b, 9a, 9b, 10b.

## Implementation

Not started. To record: modules, TE-6's figures, the line cap's basis, deviations with reasons.
