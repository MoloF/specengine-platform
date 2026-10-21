---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: Phase 3 slice 8b-2 (RM-0032), split from 8b on 2026-10-21 (tested-evidence.md Q-A, owner pre-approved, review open); a compact draft, refreshed by its own analysis before it starts
adrs: [ADR-0046]
---

# tested-reads (8b-2): the reads show each node's `tested` state

## Why

8b-1 (`tested-evidence.md`) makes the owner's passing test run durable evidence in `spec.lock` and judges it in plain `spec verify` and `check_binding`. An agent reading a node, a bundle, its task package or an impact radius sees each binding's `sync` (7a, 7b) but not whether the node's evidence still holds. ADR-0046 puts computed state where agents read: 8b-2 adds `tested` there, from 8b-1's `tested_state`, never a judge of its own. Health's lines stay 9b's (`health-drift.md`).

## Description and interactions

- **Order.** After RM-0008 (8b-1); its analysis refreshes this draft at that `HEAD`.
- **Reads**: `spec show REF --bindings` (MCP `get_node` `with: ["bindings"]`, HTTP `with=bindings`), `spec bundle` (`get_context_bundle`), `spec task show` (`get_task`, the daemon's task route), `spec impact` (`get_impact`). Tree counters stay RM-0011's (`code-reads.md` "Known limits").
- **Seam.** 7a's one judge, CLI `bindings.rs` `judge_set` / `set_bindings` (`code-reads.md` "The seam"), exposes each row's current digests or a per-node bound hash, so a read parses no file twice and computes the state as plain verify does (`docs/canon/spec-lock-tested.md` "Bound hash").
- **Nothing new is written**: no lock write, queue row, tool, argument or route; no exit, status, claim, approval or gate depends on `tested` (ADR-0044, ADR-0046).
- **Degrade** (`code-reads.md` "Degrade"): every cause leaves `tested` `null` beside the `bindings:` note; a degraded read never shows `verified`.
- Plain `spec show` stays byte-identical: without `--bindings` the lock is not read.

## Data

Sketch; the exact placement is its analysis'.

- **Entry**: 8b-1's plain-verify entry less `node` where the node is the context: `{state, category, rev, node_rev, commit, verified_at}`, `null` without a block.
- **`show --bindings`**: a node's `bindings` becomes `{rows, omitted, tested}` (`code-reads.md` "spec show --bindings", its JSON line); text `  tested <state>[ <category>]` under the closing `bindings` line, only with a block.
- **Bundle**: per target node (`spec-cli-bundle.md`), JSON and the bundle form; `bundle_hash` moves with it, as with `sync` (ADR-0046's "Cost").
- **Package**: a 27th key, `tested`, per node of the package's `bindings` set, by node bytes; `schema_version` stays 1 (`task-package.md`: "a new key keeps it"); the brief's Bindings section gains the line.
- **`impact`**: per reached node (`spec-impact.md` "Output", `nodes`).
- **Mirrors**: `ShowBindings`, `TaskPackage`, `ImpactDocument` and the bundle's shapes, `state` the closed enum of 8b-1; `fixtures/daemon-keys.json` (`TaskPackage`, the bundle's and `ShowBindings`' rows); the UI's types in `ui/src/api/provisional.ts` and its daemon-key pins follow the package key (types only, no screen).
- **Plugin** 0.18.0: `PINS`, `SCHEMA_PINS`; `read-spec` and `implement` name `tested` (an edit that stales evidence is the owner's to rerun, never an agent's to claim).

## Rules and edge cases

- WHEN a read and `spec verify` judge one node at one state, their `tested` SHALL be equal (one function).
- WHEN a read degrades, its `tested` SHALL be `null`, never `verified`.
- WHEN no block exists, every read's text SHALL be byte-identical to 8b-1's, and its JSON SHALL differ only by the new keys.

## Acceptance criteria

Outline; its analysis names files, counted mutations and budgets.

- [ ] RD -- M: `tested` computed without `--bindings` -> `plain_show_reads_no_evidence`; a read judging on its own -> `read_and_verify_agree_on_tested`; `verified` under a degraded read -> `degraded_read_never_verified`; `schema_version` raised -> `p2_2_the_package_schema_is_pinned_at_version_1`; a key missing -> `daemon_keys.rs` and the `mcp_*` walkers; a schema change without 0.18.0 -> `SCHEMA_PINS`.
- [ ] TE -- CLI, MCP and daemon parity per read with a block; determinism (two roots, another cwd, CRLF copies); `show_bindings_cost`, `bundle_bindings_cost` rerun (`#[ignore]`).
- [ ] UI -- the types and `daemonKeys.test.ts` follow `daemon-keys.json`.
- [ ] SW -- canon "Reads" in `docs/canon/spec-lock-tested.md` (8b-1's, 9 785 B of 12 288 at its shipping); pointers only in `code-reads.md` (12 263 B, 25 left), `spec-impact.md` (12 279, 9), `spec-cli-bundle.md` (12 278, 10), `mcp-read.md` (12 266, 22); `task-package.md` 11 783 (505 left); the HTTP README (10 239, 1 left) untouched.

## Out of scope

Health (9b); tree counters, a node screen (RM-0011); a stored state; a new tool, argument or route; evidence levels.

## Open

For its analysis: the bundle's placement (per target or per layer-5 node); the package's set (targets only, or every node of `bindings`); the brief's line; whether `get_impact`'s description (2 043 B of 2 048) needs a word, else the schema descriptions carry it.

## Implementation

Not started.
