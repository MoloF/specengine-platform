---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: Phase 3 slice 7a-2 of RM-0006 (Q-A..Q-H, the review's points owner pre-approved, review open); 05 s6; 07 s1.2; 08 s2; 2 iterations, review accepted
shipped: 2026-10-21
adrs: [ADR-0046]
---

# bundle-bindings (7a-2): the bundle's layers 5 and 9 and the package's `bindings`

## Why

Layers 5 and 9 were empty, the package's `bindings` `[]`. Now `get_context_bundle` and `get_task` give the code bound to the targets and the tests verifying them, each with its `sync` (`spec verify --node`'s state), with no `check_binding` per node. No ADR: ADR-0046 as in 7a-1; ADR-0027 (`schema_version` stays 1: a promised key filled). RM-0006 shipped with it.

## Description and interactions

Canon: `docs/canon/code-reads.md` "The bundle and the package"; `spec-cli-bundle.md` "Layers", "Fitting", "Output", "API"; `task-package.md`. Unchanged: no exit changes because of sync or degrade (the minimum moves with rows: the reserve counts them); without `[code]` every output, byte for byte; the package's 26 keys; `INSTRUCTIONS`; the HTTP sources; check, gate, CI, `INDEX_FORMAT`, the queue.

## Data

### Layers 5 and 9

Layer 5 the non-`verifies` rows of the targets and the nodes within them, layer 9 the `verifies` rows of those and of layer 4's candidates; a row is an item: its row line and `sig`, or a tail entry named by its symbol, else its file.

### Task package

`bindings`: every relation's rows of `targets`, `affected_nodes` and their sections, one read with the bundle, at most `PACKAGE_BINDINGS_MAX` 64 and one note; the brief's Bindings section 10th of 11, before Bundle, only with rows, from the row alone.

### Descriptions and plugin 0.14.0

`BUNDLE_DESCRIPTION` 1 305 B: the list and a paragraph before the errors' (lines 1, 2). `GET_TASK_DESCRIPTION` 1 754 B: the brief's list `(goal, criteria, targets, assumptions, open proposals, owner notes, plan, spec changes since approval, runs, code bindings, bundle)`, then line 3 (64 tied to `PACKAGE_BINDINGS_MAX` by `holds_number`).

```text
Markdown body: the targets, their open questions, ancestors, criteria, code bindings (each with its sync: the state spec verify --node gives), decisions, neighbours, terms and the tests bound to them or their criteria, in that order; what did not fit is named by ID (a binding by its symbol, else its file) with its cost.
Code bindings and tests need a [code] table; a code layer that cannot be read gives a note, never an error.
bindings: the code bound to the targets, the affected nodes and their sections, each with its sync (the state spec verify --node gives), at most 64 (a note names the rest); [] without a [code] table. A code layer that cannot be read gives a note, never an error.
```

Plugin 0.14.0 (MINOR): `PINS` `f183ab07…6e90`, `SCHEMA_PINS` `257a8f33…565a`; `read-spec` step 1:

```text
1. **The bundle first.** Call `get_context_bundle` with the IDs (or root-relative `.md` paths) your task names, in `node_ids`. One answer holds the targets, their open questions, ancestors, criteria, the code bound to them with each binding's `sync`, decisions, neighbours, terms and the tests bound to them or their criteria; what did not fit the budget is named by ID (a binding by its symbol, else its file) with its cost.
```

### UI and daemon keys

Q-G (ADR-0033 exceptions): types in `ui/src/api/provisional.ts` (`BindingRow`, `BundleBindingRow`; `ShownNode.bindings` always `null`); dark only.

- **Bundle tab** (`tree/BundlePanel.tsx`): layers 5 and 9 (`labels.ts` `isCodeLayer`) print each row as lines, never through `ItemRow`, nothing a link: node (`-` for `""`), relation, place; binding (keyless `No symbol`), layer, tokens; a `Sync` badge, category, detail; `Signature:`, `Moved from`, `Candidate`. React keys hold the row's index. A code tail entry is text: `<name> <title> | <path>:<line> | <n> tokens | Bindings|Tests`.
- **Sync** (`syncLook`): In sync, Spec ahead, Code ahead, Conflict, Broken binding, Unlocked, Bad marker, Predated, Cannot verify (own colour, icon); `null` Not judged (neutral, dashed, never ok's look); another word raw. Tokens `--sync-*` alias existing roles. Mocks serve no rows.
- **Daemon keys**: `fixtures/daemon-keys.json` 67 sets (`BindingRow`'s 14 keys, last); `TaskPackage` 26; `daemonKeys.test.ts` `AHEAD_OF_FIXTURE` empty.

## Acceptance criteria

M: the mutation that turns the test red; 8 of 8 killed. CLI `tests/bundle_bindings.rs` unless named; the bytes before 7a-2: `bundle_bindings_before.json`.

- [x] RD-1 -- `bundle_is_unchanged_by_unrelated_code` (1 000 unrelated files). M1: rows taken by file.
- [x] RD-2 -- `bindings_layer_holds_target_rows_only`, `tests_layer_reaches_criteria_tests`. M2: a neighbour's or `verifies` row in layer 5.
- [x] RD-3 -- `bundle_sync_equals_verify_node` on four corpora. M3: rows judged against an empty lock.
- [x] RD-4 -- `bindings_follow_criteria`, `frame_reserve_counts_binding_rows`; `bundle_fit.rs` with rows. M4: rows before layer 2.
- [x] RD-5 -- `no_code_table_bundle_is_unchanged` (text, JSON, hash, brief); `code_index_fresh.rs`. M5: cause (2)'s note printed.
- [x] RD-6 -- `bundle_survives_a_broken_code_layer`, a leg per cause, `bundle` and `task show`. M6: exit 2 propagated.
- [x] RD-7 -- `package.rs` `package_bindings_are_capped_with_a_note` (65 rows; the budget leg); the brief; `p2_2`'s 25 paths. M7: no cap.
- [x] RD-8, UI-1 (not counted) -- MCP `mcp_read.rs`, `mcp_tasks.rs`, `plugin_files.rs`; `daemon_keys.rs`; the UI's type pins.
- [x] UI-2 -- `NodePane.test.tsx`, `labels.test.ts`: rows as lines, no link, Not judged and Cannot verify each its look. M8: rows through `ItemRow` (the linked-tail half not run).
- [x] TE-1 -- RD-3's differential over every bundle and package set. TE-2 -- `bundle_and_package_are_byte_identical_across_roots_order_and_crlf`.
- [x] TE-3 -- `bundle_bindings_cost` (`#[ignore]`, release, warm, median of five; `code-a` plus 10 000 unrelated files, outputs byte-identical): `bundle` 219 ms, 7 without `[code]`, 220 at budget 10 000; `task show` 237; `show --bindings` 221; plain verify 365 (bound 372).
- [x] TE-4 -- `spec-c` unchanged (CLI, MCP); `mcp_genre`, the P2-3 scan.
- [x] TE-5 -- `mcp_size` with rows: `get_task`, 64 long rows, 489 267 characters; `get_context_bundle` at `u32::MAX` 121 817; `mcp_door`; without `[code]` outputs byte-identical (TE-4, 176 cases).
- [ ] TE-5 pilots -- the owner's: `bundle_bindings_pilot` (`#[ignore]`) both warm under 1 s; `specengine-eval w` medians with `[code]` recorded.
- [x] SW-1, SW-2 -- canon and READMEs within their caps (`spec-cli-bundle.md` 12 278, `mcp-read.md` 12 286 B).

## Out of scope

Tree counters, the node view's bindings (RM-0011); `tested` (8b); `impact` (7b); `bundle --task`; layers B, C; running tests; running counts in the fit.

## Implementation

| Module | What it does |
|---|---|
| CLI `bundle.rs` | `bundle_with(…, Option<&mut Beside {refs, rows}>)`, `BesideRows`, `beside_rows`; `CodeRead`, `CodeRows`, `code_rows` (one `set_bindings`); `prepare_row`, `Placed`; `Bundle.{bindings, tests}`, `placed`, `BundleRow`, `RowJson`; one `open_index` handle for both indexes |
| CLI `package.rs`, `main.rs` | `bundle_of` takes the rows (else `beside_rows`); the cap; `within_budget(pkg, past_cap)`, `rows_note`, `note_weight`; `SECTIONS` 11, Bindings 10th; `render_brief(&TaskPackage)`; `task show`'s help names code bindings |
| CLI `cap.rs`, `bindings.rs`, `node_set.rs`, `lib.rs` | `row_line`, `row_lines` crate-visible; `row_binding`; `named_set`; exports `BundleRow`, `PACKAGE_BINDINGS_MAX` |
| core `task.rs`, `check/bundle.rs`; eval `w.rs` | `PACKAGE_BINDINGS_MAX` 64; doc comments; `bundle.placed(layer)` |
| MCP `mirror.rs`, `read.rs`, `tasks.rs` | `BundleRowShape`, `TaskPackage.bindings` `BindingRowShape`; the two descriptions |
| `plugin/`; UI | 0.14.0; `provisional.ts`, `BundlePanel.tsx` (`BindingLine`, `TailLine`, `rowKey`), `labels.ts`, tokens, mocks |

Files per role beyond the plan: test-engineer `plugin_roles.rs`, `bindings_common/`, `bindings.rs`' `#[cfg(test)]` (`a_rows_brief_binding_is_its_key_else_its_judgement`); ui-developer `mocks/corpus.test.ts`; the UI's duplicate-key test kills the `rowKey` index revert.

Divergences (owner-approved): a keyless row's binding is verify's text in the body, `<sync> <category>`, `<sync>` or `-` in the brief (it renders the JSON alone); the brief prints `moved from`, `candidate` lines, the body `sig` only; Bindings moved from 4th to 10th (64 long rows could push Owner notes, Plan, Spec changes, Runs past 40 000); `BundleRowShape` repeats `BindingRowShape` (the walker catches drift).

Tests: nextest workspace 3 678 of 3 678 (53 skipped; one genre fix after the run, rerun green), ui 2 195 of 2 195.
