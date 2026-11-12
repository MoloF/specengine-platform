---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp]
ref: RM-0060's follow-up, from the orchestrator's check of `spec collisions` on a scratch copy of a real game corpus, 2026-11-12; small, no plugin version
adrs: []
---

# collision-names: a pair names each side by its canonical reference, never by an ambiguous bare ID

## Why

`collision-pass` names a side by its ID as written, else its path (`docs/canon/collision-pass.md` "Known limits"). Feature-scoped IDs repeat across documents: many feature documents hold an `AC-16`. So a pair reads `AC-16 | AC-12`, three different pairs read `AC-16 | AC-18`, and:
- `get_context_bundle` on those bare names pulls in every node holding the ID, dozens of them;
- a mark is keyed by an ambiguous name, so one verdict can hide another feature's pair;
- two holders of one name are never paired, so two features' alike criteria never list.

After this slice every name the pass prints or takes is the node's canonical reference, the form intake stores in `node_ids` and `spec.lock` keys by.

## Description and interactions

- **The rule.** A side's name, each `shared_targets` entry, the name `mark_pairs` takes and the name the ledger keys by are one form:
  1. `slug/ID` for a feature-scoped ID defined in a feature document (core `check::canonical_id`, `feature_stem`, `feature_scoped`: what `propose` already stores), and for any ID that more than one live node holds when this holder is a feature document (`feat/R-97` resolves for any prefix: `docs/canon/spec-check-links.md` "Resolution");
  2. else the bare ID, when it names that one live node;
  3. else, for an ID-less document, its path.
- **Left out (W1).** A node whose ID more than one live node holds outside feature documents (an `id-taken` or `duplicate-id` error, which `spec check` already reports) has no unambiguous name: it is not paired, and one note says so (below). `skipped` keeps its meaning.
- **The read.** The side's `id` key holds the canonical reference (`null` for an ID-less document, as now); the text line and `shared_targets` print it. The never-paired rule compares canonical names, so `stamina-tuning/AC-01` and `other/AC-01` pair like any two nodes.
- **The mark.** Rule 4 of `mark_pairs` compares each name with the universe's canonical names. A bare ID that is feature-scoped, or held by several live nodes, is refused with the resolver's own reason (`Resolver`'s unresolved reason, `spec-check-links.md` "Resolution"): ``pairs[i].a: `AC-01` is feature-scoped: cite it as `stamina-tuning/AC-01` or `other/AC-01` ``, exit 1, nothing written. Any other unknown name keeps ``no live node `X` ``.
- **The ledger.** No schema step, no migration: a row keyed by an old bare name stops matching, so its pair is listed again, unmarked, and the row stays unread (as a renamed node's). Rows of project-scoped IDs held once keep their names and hold.
- **Plugin: no version.** `get_collisions` says "a side is named by its id, else its path", `mark_pairs` "names as get_collisions gives them", the skill "by ID, else by path": all stay true with `id` canonical. No description, skill, `INSTRUCTIONS`, schema or `plugin/` byte changes, so `PINS` and `SCHEMA_PINS` gain nothing. If the developer finds a text that must change, it is a PATCH (0.44.1) recorded here.

## Data

- The document's shape is unchanged; only the values of `a.id`, `b.id` and `shared_targets` change for feature-scoped and repeated IDs.
- A new note, after the norm-less one: ``<n> node(s) not paired: their ID `<ID>` is held by more than one node outside feature documents (`spec check` reports it)``, one per ID, IDs in byte order.
- The mark's input and answer are unchanged.

## Rules and edge cases

- WHEN a node's ID is feature-scoped and it lies in a feature document, the system SHALL name it `slug/ID` whether or not another document holds the ID.
- WHEN a mark names a bare ID that resolves to more than one live node, or a bare feature-scoped ID, the system SHALL refuse it naming the canonical forms, in path order.
- WHEN a pair was marked under a bare name before this slice, the system SHALL list it again with `marked` null.
- The order, scores, caps, notes' order and determinism are unchanged: byte-equal at another root and `HOME`.

## Acceptance criteria

Over scratch git copies of `fixtures/spec-a` (`AC` is feature-scoped there): its `docs/features/stamina-tuning.md` gains a `{#AC-01}` section and a new feature document `endurance-tuning.md` beside it holds one, both titled `Regen floor holds`.

- [ ] CN1 — `spec collisions --kind criterion --json` lists one pair, `a.id` `endurance-tuning/AC-01`, `b.id` `stamina-tuning/AC-01`; `spec bundle endurance-tuning/AC-01 stamina-tuning/AC-01 --json` holds exactly those two nodes. Mutations, each red: the bare ID kept as the name (no pair: one name); the never-paired rule comparing bare IDs.
- [ ] CN2 — Marking that pair `distinct` with its canonical names hides it and only it: a third document's `{#AC-01}` pair with either side stays listed. Mutation, red: the ledger matched by bare ID.
- [ ] CN3 — `mark_pairs` with `a: "AC-01"` exits 1 with ``pairs[0].a: `AC-01` is feature-scoped: cite it as `endurance-tuning/AC-01` or `stamina-tuning/AC-01` ``, and the ledger and last event `seq` are unchanged. Mutation, red: a bare name accepted as any holder.
- [ ] CN4 — Two documents of one new kind, titles sharing no trigram, each `uses: [stamina-tuning/AC-01]`, list one pair with `shared_targets` `["stamina-tuning/AC-01"]`. Mutation, red: `end_name` left bare.
- [ ] CN5 — A ledger row written before (`a: "AC-01"`) leaves the pair listed with `marked` null.
- [ ] CN6 — Two non-feature documents both defining `R-01` (an `id-taken` error) are not paired and give the new note once. Mutation, red: the note dropped.
- [ ] CN7 — `get_collisions` and `mark_pairs` content stays byte-equal to the CLI's `2>&1` and `structuredContent` to its `--json`, for CN1 and CN3; `plugin_files.rs`' `PINS`, `SCHEMA_PINS` and the `tools/list` hash are unchanged.

## Out of scope

- A name for a node whose ID repeats outside feature documents (W1: `spec check` already errs).
- Re-keying old ledger rows; a schema step.
- Signal (a)'s flood on a real corpus (one shared target qualifies a pair): the owner's open question in `docs/canon/collision-pass.md` "Known limits".

## Implementation

To be filled in after implementation. The plan:

| Role | Files |
|---|---|
| rust-developer | core `collisions.rs` (names, the note, the never-paired rule on names); store `read.rs` (`collision_input` hands each node's and end's holder path, as now); CLI `collisions.rs` (canonical names from `[ids]` and `[paths] features`, the mark's refusal) |
| test-engineer | CLI and MCP `tests/collisions.rs`; core units |

Comments cite `docs/features/collision-names.md` "Acceptance criteria". **Working answers** (the orchestrator's rule, the owner's review open): W1 above; the price of the other answer, naming such a node by its bare ID, is the ambiguity this slice removes. Ship: `docs/canon/collision-pass.md` "Universe", "Known limits".
