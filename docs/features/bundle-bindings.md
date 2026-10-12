---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 7a of impact-bundle (WA-1..WA-9, WA-12, WA-13, WA-15..WA-17; Q1-Q6, Q9-Q12, Q14 decided under the owner's pre-approval, review open); 05 s3.4, s5.3, s6; 07 s1.2, s2; 08 s2
adrs: [ADR-0046]
---

# bundle-bindings: an agent's reads carry its nodes' code bindings and sync

## Why

A bundle says nothing about code: layers 5 and 9 are empty (`docs/canon/spec-cli-bundle.md` "Layers"), the package's `bindings` is an uninhabited type (model `task.rs`), drift shows only in `spec verify` on a terminal. An agent greps files to learn where a node is implemented and whether that code still matches: a cost growing with the project.

Delivered: each binding of the nodes read, with its `sync`, in the call the agent already makes. Slice 7 split (Q1): 7a this; 7b `impact` (`impact.md`), the radius over 7a's rows. ADR-0046: reads show verify's state and never fail on code (`architecture.md#control`, `architecture-code.md#sync-in-reads`).

**In scope:** the row (WA-6); layers 5, 9; `show --bindings` and its MCP, HTTP twins; tree counters; package `bindings` and a brief section; `delta.tests_reaching`; the code-index refresh in these reads; mirrors; UI types; fixtures.

## Description and interactions

- **An agent** gets, in `get_context_bundle`, `get_node`, `get_tree` or `get_task`, the code bound to its nodes and each binding's `sync`. **The owner**: the same in the daemon's JSON (screens: Phase 4).
- **Freshness** (WA-2): a read printing code data (bundle, `show --bindings`, tree, `task show`, 7b's `impact`) refreshes the md index, then 6a's code index (`symbol-index.md` "Cache key and cross-file pass"), when `[code]` exists; `show` without `--bindings`, `search`, `graph` never walk code. Amends `symbol-index.md` WA-6, RD-11.
- **Sync** (Q5; WA-3..WA-5): 4a's pure core judge (`spec-verify.md` "States") over the refreshed index rows, `spec.lock` (slice 3's reader) and the read's own parse for R, H. An item compares the indexed `ast_hash` with the lock's (no git); a locked `module` or `ron` row takes its blob OID from one `git hash-object --no-filters --stdin-paths` per read (`GitEnv::child_vars`). 2026-10-12, `lock-levels.md` "Level-aware verify": after 5b an item compares the indexed level digests of (the lock's ∪ the markers' levels) minus `path` with the lock's, a `module` or `ron` row whose blob differs is judged by levels too (its digests from 6a's index or the read's own parse), and a format-1 lock gives `sync` unknown with the rebase note. Plain state: no base, no Pre-existing; a per-node `tested` (`tested-evidence.md` "Surfaces", no level number) added by whichever of 7a and 8b ships second (Q10).
- **Crates**: core the pure selection, order and fit, 4a's judge reused; store queries over 6a's tables by node ID (never a walk) and the blob call; CLI `bundle`, `show`, `tree`, `package`, verify's delta; model the row type, replacing `PackageBinding {}`; MCP `with`, mirrors; HTTP `with=bindings`; UI types only (Q14). No new crate or dependency. Relations are the marker vocabulary, never kinds (WA-17).
- **Unchanged**: every read's exit; verify's rows and verdict; `spec check`, gate, hook, CI; `INDEX_FORMAT`, queue schema; `TASK_PACKAGE_SCHEMA_VERSION` 1 (a promised key filled); `graph`, `search`. **Writes**: the data directory only.
- **Overridden lines**, fixed at shipping (SW-2): `spec-cli-bundle.md` layers 5, 9 and "Not yet"; `task-package.md`'s "`bindings` `[]` until Phase 3"; `mcp-read.md` `with`'s enum; 05:215, 07:26, 07:78.

## Data

### The binding row

WA-6, Q2. One object in every read (and 7b's): the lock key (`spec-verify.md` "Command and rows": `node`, `relation`, `file`, `kind`, `symbol`, `item_kind`; markers sharing a key are one row; an `unanchored` marker's `kind`, `symbol`, `item_kind` null) plus `line`, `end_line` (the indexed symbol's; RON `line` only; module, unanchored, gone: null), `signature` (6a's or null), `layer` (6a's `"a"`; null without a symbol row), `sync` (4a's state; null when unknown), `category`, `detail` (4a's), `anchor` (5c's `anchor.state`; null when unknown), `moved_from` (its `from`, or null; 2026-10-12, `anchor-states.md`). `node` is the canonical ID the marker or entry resolves to (`spec show`'s resolver). A lock entry without a marker is a row, so `broken_binding marker_gone` shows. Never: mentions, `queue` or `other_project` IDs.

```json
{"node":"LAW-BOUND","relation":"implements","file":"crates/tally/src/counter.rs","kind":"item",
 "symbol":"crates/tally::counter::impl Tally","item_kind":"impl_item","line":22,"end_line":30,
 "signature":"impl Tally","layer":"a","sync":"code_ahead","category":"code_changed","detail":null}
```

Text, one line per row, then `  sig <signature>` when not null (`node` first, SW refinement: a document target holds its sections' rows):

```
LAW-BOUND | implements | crates/tally/src/counter.rs:22-30 | item crates/tally::counter::impl Tally impl_item | layer a | sync code_ahead code_changed
  sig impl Tally
cap-check/PASS-01 | verifies | crates/tally/tests/bounds.rs:16-22 | item <qpath> function_item | layer a | sync unlocked
```

Location `<file>:<line>-<end_line>`, `<file>:<line>` (RON), `<file>` (no line); `layer -`, `sync -` when null. **Order** everywhere: the bound node's (path, ord), relation (`implements`, `configures`, `assumes`, `verifies`), file bytes, the first marker's line and column (lock-only rows last), never rowid.

### Layers 5 and 9

Q2-Q4. Core `bundle_layers` gains the two layers by node, never by file:

| # | Key, heading | Rows | Form |
|---|---|---|---|
| 5 | `bindings`, Bindings | relation ≠ `verifies`; node a target or within one (`SpecGraph::within`) | the row's line(s) |
| 9 | `tests`, Tests | relation `verifies`; node a target, within one, or a layer-4 candidate (placed or not) | the row's line(s) |

- **Fit**: the canon order 1-4, 5, 6-8, 9 and the same greedy (`spec-cli-bundle.md` "Fitting"); one form per row, in or out, never degraded; the reserve's n counts every row; running counts for rows (a document target can hold hundreds, "Risks").
- **Left out**: a not-included entry `{name: <symbol, else file>, title: "<relation> <node>", path: <file>, line, tokens_est, layer: "bindings" | "tests"}` (SW refinement).
- **JSON** `layers.bindings`, `layers.tests`: the placed rows, each the row object plus `tokens_est`.
- `bundle_hash` names the body, so it moves with code drift, and the package's `bundle.bundle_hash` with it (intended: "it names the output").
- No `[code]`: both layers empty, no heading, no note.

### spec show --bindings

WA-9. `spec show REF --bindings [--links] [--archive]`: each shown node's block (its rows and its nested sections', both relations) between its header line and its bytes, after the links block, lines indented two spaces, closed by `  bindings <n>[, <f> fix][, <c> check][, <k> cannot]`. JSON: a node's `bindings`, an array of rows, null without the flag or `[code]`. The block counts toward show's cap; rows past it close as `  bindings <n> (<k> not shown)`. Without `[code]`: exit 0, ``note: specengine.toml has no [code] table: the code layer is off (add [code] with roots)`` (`code-refs.md`'s sentence). MCP `get_node`: `with` enum `["links","bindings"]`; HTTP `GET …/nodes/{*ref}?with=bindings` (the key repeated for both).

### Tree counters

Q12. When `[code]` exists, a `spec tree` line of a node with rows of its own (never its descendants') ends ` | code <n>[, <f> fix][, <c> check][, <k> cannot]`, after the `status`, `archived`, `parent` parts; none for 0 rows (SW refinement: 40 000 characters hold more lines). JSON node `bindings` `{rows, ok, check, fix, cannot}` (4a's classes), null without `[code]`; under a lock or git cause ("Degrade") the four class counts are null and the line ends ` | code <n> sync -`. `get_tree` and the daemon mirror it.

### Task package

Q11, WA-8. `bindings`: the rows (both relations) of the targets and `affected_nodes`, from the reading root (as `bundle_hash`, `task-package.md` "Package"), in row order, at most `PACKAGE_BINDINGS_MAX` 64; past it one note `bindings: <n> more row(s) left out; spec show <REF> --bindings lists them`. Rows count in `PACKAGE_BUDGET`: after diff and reference texts, rows leave from the last, the same note (SW refinement). The brief: a Bindings section after Targets, the row lines, none when `[]`. `schema_version` 1; the mirror's item `{"type":"object"}` becomes the row's schema, every key required. No `[code]`: `[]`, no note.

### delta.tests_reaching

`spec verify --json`, plain and `--changed`: the lock keys of every `verifies` row whose node is in `affected_nodes` or holds a resolved `verifies` link into one, by key; `[]` when none, never null. `delta.symbols` stays null until 7b.

### Degrade

Q6, WA-7, ADR-0046. A read of nodes never changes its exit or its spec part because of code:

| Cause | Rows | Note |
|---|---|---|
| no `[code]` | none, silent | none (`show --bindings`: the sentence above) |
| a bad `[code]` key; a walk gap | none: the index is not refreshed, older rows never shown | `note: bindings: <cause>` |
| `spec.lock` invalid or unmerged; no `git`; a git failure | listed, `sync -` (JSON null), tree class counts null | `note: bindings: <cause>` |
| a row's own `cannot_verify` | listed, `sync cannot_verify <category>` | none |

`<cause>`: 4a's `cannot:` text (`spec-verify.md` "Verdict and exit"); one note per read, the first cause in 4a's order. A row not judged is never `ok`.

## Rules and edge cases

- WHEN a read runs from two roots, another cwd, in reverse creation order or on CRLF copies, its text, JSON and `bundle_hash` SHALL be byte-identical.
- WHEN `[code]` is absent, bundle text and `bundle_hash`, show and tree text SHALL equal the bytes before 7a; JSON SHALL differ only by the new null keys.
- WHEN a state is judged, a read's `sync` SHALL equal `spec verify --json`'s plain `state` for that key.
- Door: git only `hash-object` without `-w`, never `status` or `diff` (they refresh `.git/index`). Genre: `item_kind`, qpaths, `layer` travel as data; no stack, kind or fixture literal in new sources (P2-3 scan); `spec-c` byte-identical.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red:

- [ ] RD-1 -- constant cost: 1 000 code files with markers on non-target nodes leave `spec bundle R` byte-identical. M: rows taken by the target's file -> `bundle_is_unchanged_by_unrelated_code`.
- [ ] RD-2 -- selection. M: a neighbour's rows in layer 5 -> `bindings_layer_holds_target_rows_only`; `verifies` in 5 -> `verifies_rows_go_to_tests`; criteria's tests dropped -> `tests_layer_reaches_criteria_tests`; rowid order -> `binding_rows_ignore_insertion_order`.
- [ ] RD-3 -- parity, every `code-a` state. M: the marker's rev alone -> `read_sync_equals_verify_plain`; `cannot_verify` as `ok` -> `unverifiable_row_is_never_ok_in_reads`; a lock-only row dropped -> `broken_binding_shows_in_bundle`.
- [ ] RD-4 -- no `[code]`: on `spec-a`, `spec-b` bundle text and `bundle_hash`, show and tree text equal pinned goldens. M: an empty heading or a note -> `no_code_table_reads_are_unchanged`.
- [ ] RD-5 -- each "Degrade" cause: exit 0, the same spec part, the table's rows, one note. M: exit 2 propagated -> `bundle_survives_a_broken_code_layer`; older rows after a bad `[code]` -> `bad_code_table_shows_no_stale_rows`.
- [ ] RD-6 -- fit. M: rows before layer 2 -> `bindings_follow_criteria`; the reserve without rows -> `frame_reserve_counts_binding_rows`; `bundle_fit.rs` asserts the budget bound with rows present.
- [ ] RD-7 -- M: `show` without `--bindings` refreshes code -> `show_never_walks_code` (6a's RD-11, reworded); `with` ignored -> `get_node_with_bindings_equals_show_bindings`.
- [ ] RD-8 -- tree. M: descendants counted -> `tree_counts_own_rows`; counters without `[code]` -> `tree_without_code_has_null_bindings`.
- [ ] RD-9 -- package. M: `schema_version` 2 -> `p2_2_the_package_schema_is_pinned_at_version_1` (its key paths gain the row's); no cap -> `package_bindings_are_capped_with_a_note`; a stack literal -> the P2-3 scan.
- [ ] RD-10 -- M: `tests_reaching` left null -> `tests_reaching_lists_bound_tests`.
- [ ] RD-11 -- mirrors. M: a CLI key without its mirror -> the schema walker (`mcp-read.md` "Schemas"); HTTP `with=bindings` ignored -> `nodes_with_bindings_equals_show_bindings` (HTTP `query.rs`); `fixtures/daemon-keys.json` follows; `server.rs`'s 2 048 B `INSTRUCTIONS` asserts, `probes` included, stay.
- [ ] RD-12 -- door. M: `git status` used -> `reads_leave_git_index_and_objects_untouched` (`.git/index` mtime, object count).
- [ ] RD-13 -- M: a hash crate added -> `no_workspace_dependency_is_added_against_main`; `std::fs` in core -> `model_and_core_sources_do_no_file_io`.

test-engineer:

- [ ] TE-1 -- `code-a` plus a git repository built at test time: a lock covering `ok`, `code_ahead`, `spec_ahead`, `conflict`, `broken_binding` (`marker_gone`, `symbol_gone`), `cannot_verify`, `unlocked`; a module and a RON binding; `@verifies` on a target and on a feature criterion; a non-git copy; `expected.json` with each read's rows.
- [ ] TE-2 -- two roots, another cwd, reverse creation order, CRLF: bundle, `show --bindings`, tree, `task show` give identical text, JSON and hash.
- [ ] TE-3 -- scale: 1k and 10k unrelated code files leave the bundle body identical; warm latency recorded ([A] under 1 s; over it, 6c first).
- [ ] TE-4 -- genre: the full `spec-c` cycle unchanged in the CLI and MCP; no fixture prefix, kind or stack word in the new sources.
- [ ] TE-5 -- MCP: parity, the schema walker, `mcp_size` maxima re-measured with rows, `mcp_door` over every tool.
- [ ] TE-6 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground, scratch `HOME`, config copies outside the repository): bundle, tree, `get_task` latency with `[code]`; `specengine-eval w` medians unchanged; no pilot name or path committed.

ui-developer:

- [ ] UI-1 -- `ui/src/api/provisional.ts`: the row type; bundle `bindings`, `tests` as rows (not `BundleItem[]`); node `bindings`; the tree's counters; package `bindings`; their key pins. M: a key missing -> `ui/src/api/daemonKeys.test.ts`.

spec-writer:

- [x] SW-1 -- this spec and `impact.md`; ADR-0046 with `architecture.md#control` and `architecture-code.md#sync-in-reads`; 08 s2 lists 7a, 7b; `spec-verify.md` (`delta`, "Out of scope"), `symbol-index.md` (WA-6, RD-11, "Risks", "Out of scope", Q6), `verify-tests.md` (`verified` in reads to 8b); `export index`, `check` clean.
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/code-reads.md` (≤ 12 288 B); pointer rows in `spec-cli-bundle.md` (layers 5, 9, "Not yet"), `task-package.md`, `spec-cli-graph.md`, `mcp-read.md`, each ≤ 12 288 B; CLI, MCP, HTTP READMEs ≤ 10 240 B; 05:215, 07:26, 07:78; `CLAUDE.md` State.

## Risks

The fit re-estimates the whole body per candidate (3 000: 1.13 s, `spec-cli-bundle.md` "Open"): rows need running counts. The JSON gains a row per placed row (signature ≤ 512 B, qpath unbounded): `mcp_size` re-measured, package rows capped to keep `PACKAGE_BUDGET + OUTPUT_CAP_CHARS` ≤ 500 000. Every bundle, tree, `show --bindings`, `get_task` (the daemon per request) refreshes the code index and may spawn git, against MCP's 1 s (`mcp-read.md` "Latency"). Package rows show the reading root's code to an agent in another worktree (accepted, as `bundle_hash`). 7a stands on drafts 3, 4a, 6a and unshipped 1: a change in their row shapes reopens the row object.

## Out of scope

evidence levels (none: `tested-evidence.md` Q5); `tested` (Q10, the later of 7a and 8b); `bindings.moved` (5c); level-aware sync (5b); layers B, C; a call-graph radius; the `spec index` delta; open-proposal counters; `show --history`; `--task`, `get_context_bundle {task_id}`, the `bundles` log, `compact`; 6b's tools; UI screens (Phase 4); `unbound`, Health (9); writing markers; the gate's file-to-node map. 7b: `impact`, `delta.symbols`.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 split | 7a this, then 7b `impact` | 7 crates, UI, a git reader, a tool in one: three iterations at risk |
| Q2 layer 5 | targets' and within rows, not `verifies`, lock-only included | markers only hide `marker_gone` |
| Q3 layer 9 | `verifies` rows, layer 4's criteria too; never run (ADR-0044) | misses a feature criterion's test |
| Q4 priority | canon order 1-4, 5, 6-8, 9; a row in or out | reordering the canon on a guess |
| Q5 sync | 4a's plain state over index, lock, parse | the marker's rev hides `code_ahead` |
| Q6 code fails | degrade ("Degrade") | exit 2: one bad file breaks every read |
| Q9 ADR | ADR-0046 | the canon contradicts four interfaces |
| Q10 `verified` | per-node `tested`, added by whichever of 7a and 8b ships second (`tested-evidence.md`) | 7 waits two slices |
| Q11 package | "Task package" | a second worktree walked per `get_task` |
| Q12 tree | own rows with `[code]` | drift needs a second call |
| Q14 UI | provisional types only | rows typed `BundleItem[]` |

WA-n: the analysis' working answers (2026-10-10), below; "SW refinement": the spec-writer's.

## Working answers

[P] checked, [A] assumed. WA-1 [P] layers 5, 9 are empty placeholders (core `check/bundle.rs`, MCP `mirror.rs`). WA-2 [P/A] the bundle refreshes the md index only; 6a's no-other-read rule amended. WA-3 [P/A] `sync` is computed only (05 s3.4), 4a's judge pure, index rows pure functions of the bytes: a read's sync can equal verify's (RD-3). WA-4 [P/A] 6a's tables hold every judge input; its "`ast_hash` display only" amended. WA-5 [P/A] items need no git; only locked `module`, `ron` rows need a blob OID. WA-6 [P/A] the row key is the lock key, one object everywhere. WA-7 [P] reads never change their exit on freshness (CLI README); the bundle already tolerates unrelated broken config. WA-8 [P] package `bindings` always `[]`, schema `{"type":"object"}`, a new key keeps `schema_version`. WA-9 [P] `with` is the closed enum `["links"]`; HTTP `nodes` takes `with`. WA-12, WA-13 [P] slice 7 owns `tests_reaching` and read sync; `verified` needs 8b. WA-15 [A] the bundle, MCP and package canons near 12 288 B: detail to `code-reads.md`. WA-16 [P/A] pilots hold no markers: W unmoved, value shown on `code-a`. WA-17 [P] relations are the marker vocabulary (ADR-0010), not kinds.

## Dependencies and ordering

- **Needs**: 1 `code-refs` (built, unshipped) -> 3 `spec-lock` (the reader) -> 4a `spec-verify` (judge, row key, blob call) -> 6a `symbol-index` (tables, refresh, signature); [6b optional, spends `INSTRUCTIONS` room first] -> 7a -> 7b.
- **Order**: this spec and ADR-0046 now, code after 6a; 6c first if AC-10's no-op timing fails. **Independent**: 4b, 4c, 8a. **Later**: 5b, 5c, Phase 4. **Either side**: 8b (`tested` here, by the later).

## Implementation

Not started. To record: modules, TE-3's and TE-6's figures, deviations with reasons.
