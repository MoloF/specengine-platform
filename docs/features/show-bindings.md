---
class: spec
status: shipped
scope: [crates/specengine-model, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, plugin]
ref: Phase 3 slice 7a-1 of RM-0006, split 2026-10-21 (Q-A..Q-H and the review's points owner pre-approved, review open); 05 s5.2; 07 s1.2; 08 s2; developer 2 iterations, review accepted
shipped: 2026-10-21
adrs: [ADR-0046]
---

# show-bindings (7a-1): `spec show --bindings`, `get_node` `with: ["bindings"]`, `delta.tests_reaching`

## Why

`get_node` gave a node's text and links, never its code: an agent grepped or called `check_binding` per node to learn where it is implemented and whether that still matches. 7a-1 puts each binding, with its `sync` (`spec verify --node`'s state by construction), into that read, and no read fails on the code layer. No ADR: ADR-0046 covers the `--node` path (Q-B). 7a-2 and 7b reuse row and seam.

## Description and interactions

How it works now: `docs/canon/code-reads.md`, `architecture-code.md#sync-in-reads`, `spec-cli-read.md`, `spec-verify-scope.md` "`--node`". Unchanged: every exit, `verify --node`'s output, check, gate, CI, `INDEX_FORMAT`, the queue, the package's bytes.

## Data

### The binding row

`docs/canon/code-reads.md` "The binding row": 14 keys; `PackageBinding` replaced; verify's anchor object (Q-D).

### Degrade

`docs/canon/code-reads.md` "Degrade". Decided at shipping: (8), the code refresh failing on the data directory, ends the bindings read at once like (1)-(3), its note naming it even with git or the lock faulty (rare; rows `null` either way).

### spec show --bindings

`docs/canon/code-reads.md` "spec show --bindings". Below the cap the spec part is `spec show REF`'s; near it the JSON follows the text's cut; without `[code]`, exactly `spec show REF` plus the note.

### MCP get_node

`NODE_DESCRIPTION` 1 264 -> 1 527 B: `[--links] [--bindings] [--archive]` and the line:

```text
with: ["bindings"] lists each node's code bindings, its sections' too, each with its sync (the state spec verify --node gives), after its links; without a [code] table they are null, and a code layer that cannot be read gives a note, never an error.
```

### HTTP

`with=bindings`, the 400: `docs/canon/code-reads.md` "Twins".

### delta.tests_reaching

All modes but `--node` (`null`, Q-E): `docs/canon/spec-verify-output.md` "JSON".

### INSTRUCTIONS

Q-F: `, --bindings is with ["bindings"]` (33 B) in, ` (no specengine.toml: `spec init`)` (34 B) out: 1 977 B (`0be8933f…61f4`), 1 839 less the tasks line (`54d160f1…eb72`), 2 036 with `probes` (`mcp-read.md` **Texts**).

### Plugin 0.13.0

MINOR: `plugin.json` 0.13.0, `PINS` `c80be32c…c046`, `SCHEMA_PINS` `3570759a…8f10`; `read-spec` step 2's first bullet:

```text
   - one node's current text: `get_node` with its `id`; add `with: ["links"]` for its links when you need what it points at and what points at it, `with: ["bindings"]` for the code bound to it and whether that code still matches the spec (each binding's `sync`);
```

## Acceptance criteria

M is the mutation that must turn the named test red; 8 of 8 killed.

- [x] RD-1 -- `show_bindings_sync_equals_verify_node`: rows the union of `verify --node` over the node and its sections. M1 run as its variant (the state from the marker's `@rev`): the exact one needs a second judge path; the tamper leg stands for it.
- [x] RD-2 -- `show_bindings_rows_are_own_and_sections`. M2.
- [x] RD-3 -- `show_bindings_survive_a_broken_code_layer`, a leg per "Degrade" row. M3.
- [x] RD-4 -- `code_index_fresh.rs` `show_bindings_refresh_the_code_index`, `show_search_graph_never_refresh_code`; `show_bindings_without_code_is_show`. M4.
- [x] RD-5 -- `show_bindings_cut_keeps_json_to_printed_rows`. M5.
- [x] RD-6 -- `verify_output.rs` `tests_reaching_lists_bound_tests`: the linked test is a record document's (`GOAL-02`, `links: verifies: [LAW-BOUND]`), not a feature criterion's: a section has no front-matter (`spec-check-links.md` Q-I). M6.
- [x] RD-7 -- twins: `mcp_read.rs` `get_node_with_bindings_equals_show_bindings`; HTTP `query.rs` `nodes_with_bindings_equals_show_bindings` and the 400; `mcp_code.rs` at 1 977 B; `plugin_files.rs` 0.13.0. M7.
- [x] RD-8 -- `show_bindings_writes_nothing_under_the_root`; `no_workspace_dependency_is_added_against_main`. M8.
- [x] Pins -- `verify --node`'s output unchanged; the `verify_node.rs` `delta_of` oracle takes `tests_reaching` as `null` under `--node`, `verify_task.rs`'s computes it before `--task`'s filter. Re-pinned: the `show`, `bundle`, `norm_show` key sets (`bindings`); `graph.rs`'s unknown flag (`--proposals`); `mcp_door` ac08 and `mcp_read` ac05 (`history` the invalid `with`); `plugin_skills`, `plugin_roles`, `plugin_files` 0.13.0; `INSTRUCTIONS` in five files; 22 request literals.
- [x] TE-1 -- `show_bindings_equals_verify_node_on_every_corpus`. TE-2 -- `show_bindings_are_byte_identical_across_roots_order_and_crlf`.
- [x] TE-3 -- `show_bindings_cost`: below plain verify ("Implementation").
- [x] TE-4 -- `show_bindings_leave_spec_c_unchanged`, MCP `get_node_with_bindings_on_spec_c_is_get_node_and_the_note`; genre, the P2-3 scan.
- [x] TE-5 -- `mcp_size`: `get_node` with 3 000 rows 101 440 characters; `mcp_door`.
- [ ] TE-6 -- the owner's pilots: `verify_pilot.rs` `pilot_show_bindings_latency`, the `mcp_code_pilot.rs` form (`#[ignore]`, against 1 s; a breach plans 6c).
- [x] SW-1, SW-2 -- `code-reads.md` new, 8 937 B (planned 6 000: it holds the twins, `tests_reaching`, the limits too); the pointers within their caps (`architecture-code.md` 12 286, `symbol-index.md` 12 284, `mcp-read.md` 12 283; HTTP README 10 239); 05, 07, 08 s2, RM-0006, RM-0011.

## Out of scope

Bundle layers and the package's rows (7a-2); tree counters (RM-0011, Q-C); `tested` (8b); `impact`, `delta.symbols` (7b); `with: history | proposals`; the UI; `--bindings` on `search`, `graph`.

## Implementation

| Module | What it does |
|---|---|
| CLI `bindings.rs` (new) | `judge_set` (`--node`'s judge over any set), `set_bindings` (the degrade), `binding_row` |
| CLI `verify_node.rs`, `verify_rows.rs`, `verify.rs`, `node_set.rs` | `--node` via `judge_set`; `list_rows`; `reaching_tests`, `key_set_nodes`; `contained` |
| CLI `show.rs`, `cap.rs` | `ShowRequest {bindings, git}`, `ShownNode.bindings`, `add_bindings`; the block, cut, tail |
| CLI `main.rs`, `bundle.rs`, `package.rs`, `lib.rs` | `--bindings`; `BundleRequest.git` (read by 7a-2); `process_git` |
| model `task.rs`, `lib.rs`; store `code.rs` | `BindingRow` and its parts; `SelectedFile.symbols` (store `lib.rs` unchanged) |
| MCP `read.rs`, `mirror.rs`, `server.rs`, `resources.rs` | `With::Bindings`, `NODE_DESCRIPTION`; `ShowBindings`, `BindingRowShape`; the swap |
| HTTP `args.rs`, `app.rs`; eval `src/w.rs`; `plugin/` | `with() -> (links, bindings)`, the 400; request literals; 0.13.0, `read-spec` |

Roles: rust-developer `src` and its request literals (eval `w.rs`, MCP `resources.rs`, CLI `package.rs`); test-engineer the tests, the 22 test literals (CLI 11, MCP 9, eval 2), the re-pins.

Tests, iteration 1: nextest over eight crates green; touched binaries 164/164, eval 91/91; clippy (both builds), fmt clean. Final full run: nextest workspace 3 654 of 3 654 passed, 50 skipped. TE-3 (release, warm, median of five, `code-a` plus 1 000 files): `show --bindings` 51 ms, `verify --node` 52, plain verify 64, plain `show` 6.

Iteration 2, after the ACCEPT: the git session's end a cause only when none is listed; a node whose ID the second read lacks gets no rows (no fallback to the document); `[code]` checked before the spec files' bytes are read. Kept (owner pre-approved, review open): (8) ending the read at once; a spec graph built per verify run for `tests_reaching`.
