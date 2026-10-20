---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 6 slice 1b of `roadmap-items.md` (its Q-7, N5); the requirement-analyst's breakdown of 2026-10-20, decided under the owner's pre-approval, review open; 08 s2
adrs: [ADR-0052]
---

# roadmap-view (Phase 6, 1b): the committed, checked roadmap view

A compact draft, refreshed against 1a as built before it starts.

## Why

After 1a the path is computed, but 08 s2 still keeps it by hand ("~N weeks", a summed "Total") and drifts. A generated view committed beside the documents makes the plan readable in any diff and on GitHub, and the check fails a hand edit as it fails a hand-edited index (ADR-0022 §7, ADR-0052: "a committed view of it is `generated`, from git alone"). 08 s2's list and "Total" become a pointer to it (Q-7).

## Description and interactions

- **Registry** (`docs/canon/spec-check-graph.md` §11.6): the `index = true` entry takes an optional key `roadmap = "<path>"` (N5). Here: `roadmap = "docs/generated/roadmap.md"`, listed in that entry's `writes`.
- **`spec export index`** writes the view with the index and its shards; **`spec check`** renders it and compares it byte for byte, like each index output (§11.5): a difference -> `index-drift` on its path, a view not walked -> `index-missing`. No new check code (`CHECK_CODES` stays 36), no new generator entry, no change to the roles' finishing command.
- **Git only**: rendered from the walked files (`--staged`: the staged blobs) through 1a's `plan`, never the queue or the index database: CI has no queue. Slice 2's task-derived progress never enters it.
- **Not indexed**: a generated document has no index line (`spec-check-graph.md` "Index render (§9)"); it carries `class: generated`, `generator:` the entry's `command`, `source: the plan items of [roadmap]`.

## Data

Registry rules, each a config error (`specengine.toml:<line>: message`, the run cannot check): `roadmap` on an entry without `index = true`; its path not in `writes`, equal to `[paths] index` or a shard's, or breaking the `[paths]` rules; `roadmap` set while `[roadmap]` is absent or bad (its own message and line). The existing rule "an index entry `writes` path neither `[paths] index` nor a shard" admits the view's path.

The view (Markdown, LF, one trailing newline; every list in 1a's order):

```text
---
class: generated
generator: <command>
source: the plan items of [roadmap]
---

# Roadmap

<!-- Built by `<command>`. Manual edits are overwritten on rebuild, and `<gate>` rejects them. -->

<o> open, <d> done; current <release or none>[; to <cut> <sum>]; total <sum>.

## <release>[ (current)][ (mvp cut)]

<o> open, <d> done; <sum>.

1. [<ID>](<link>) <title>: <status>, <priority or ->, <estimate or ->[; waits on <ID>, …][; cycle]

## Pool

## Unplaced

## Notes
```

Open items listed, done items counted only (they stay readable by ID). `<link>` relative from the view's directory. An empty group says `None.`; "Notes" lists 1a's notes or `None.`.

## Rules and edge cases

- WHEN the queue changes and no file does, the view SHALL keep its bytes.
- WHEN a project registers no `roadmap` key, `export index` and `check` SHALL behave as before.
- WHEN an item changes, its commit SHALL carry the regenerated view, as an index line today: in a consumer project an approve commits only its own path, so under an `enforce` hook an item's apply meets `index-drift` exactly as a `create` meets it on the index today (N12; 3b waits on its ADR).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- registry; core `tests/check_generated.rs`. M: the key accepted on another entry -> `roadmap_output_only_on_the_index_entry`. Unmutated: each config error above.
- [ ] RD-2 -- drift. M: the comparison skipped -> `hand_edited_view_is_index_drift`; a missing view unreported -> `missing_view_is_index_missing`.
- [ ] RD-3 -- git only. M: the render reads the queue -> `view_ignores_the_queue` (an `in_progress` task targeting an item leaves the bytes unchanged).
- [ ] RD-4 -- determinism and genre: the expected views (TE-1), two roots and reversed creation; `roadmap_genre.rs` scans the render too.

test-engineer:

- [ ] TE-1 -- `fixtures/roadmap-a/expected-view.md`, `fixtures/roadmap-b/expected-view.md` equal `export index`'s output; in this repository `export index && check` is clean.

spec-writer (bytes 2026-10-20):

- [ ] SW-1 -- `spec-check-graph.md` §11.5, §11.6 (12 273 of 12 288: first move §11.6 into a new Tier 2 `docs/canon/spec-check-generators.md` or trim "Next", "Open nits"; never a raised cap).
- [ ] SW-2 -- `spec-roadmap.md` "View".
- [ ] SW-3 -- in one change: the root config's `roadmap` key and `writes`; 08 s2's list and "Total" replaced by a pointer to the view (Q-7, 08 keeping the MVP definition); `docs/README.md` "Generated"; `CLAUDE.md` State.

## Files

rust-developer: core `check/config.rs` (the key and its rules), `check/generated.rs` (render and compare), `roadmap.rs` (`render_view`); CLI `export.rs`. test-engineer: core `tests/check_generated.rs`, `tests/roadmap_genre.rs`; CLI `tests/export.rs`; the two expected views.

## Dependencies

After 1a `roadmap-items` (its `plan`, the fixtures, the seeded items here); before 2. About 0.5 week.

## Out of scope

Task progress in the view (2); a second generator entry or `spec export roadmap`; new check codes; dates.

## Implementation

Not started.
