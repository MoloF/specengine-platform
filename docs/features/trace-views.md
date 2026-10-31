---
class: spec
status: shipped
shipped: 2026-10-31
scope: [ui, crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-import, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin]
ref: RM-0048; Phase 7 step 7 (08 s2)
---

# trace-views: flow badges, `stage:`, the timeline, a node's cards (RM-0048)

## Why

This closes the first result: the owner, alone on the local daemon, sees a whole cycle. The diagram shows what is confirmed, the timeline how far each stage has come, and a node its cards, all read from `card-trace.md`'s rollup. The UI joins nothing and computes no rollup.

## Summary

What became true is in `docs/canon/`. Code comments cite the headings below, so each stays with its pointer.

### The key `stage:`

Typed, any script, never counted: `spec-flow.md` "The read", `node-revision.md` "Never counted".

### Folded start and end

`spec-flow.md` "The read".

### Flow badges

`spec-flow.md` "The read"; `spec-flow-screen.md` "Layout".

### A node's cards

The Cards block: `spec-flow-screen.md` "Panel".

### The board tile

`board-ui.md` "Screen"; `spec-trace.md` "The read".

### Timeline

`spec-flow-screen.md` "Timeline".

### Stage filter

`spec-flow-screen.md` "Stage filter".

### Data

Node keys: `spec-flow.md` "Output".

### `spec trace`, extended

`--stage`, `--summary`, lanes: `spec-trace.md` "The read".

### Routes and types

`board-ui.md` "Routes", "Types", 106 key sets.

### Fixture

`fixtures/trace-almanac`.

## Acceptance criteria

- [x] AC-01 — Writing or changing `stage:` on `fixtures/trace-almanac` leaves every `norm_hash`, `rev` and card mark as they were; no `unknown-key`; `spec flow` prints it. M: counting the key in `norm_input`.
- [x] AC-02 — `spec flow BOOK-ALMANAC --depth 1`: the part holding `STEP-CALL` is `start`, the one holding `STEP-BIND` `end`, at every depth above; gap rows byte-equal to the unfolded read. M: `steps.starts.contains(&file)` kept in `shown`.
- [x] AC-03 — `--trace` absent: bytes as before but `stage`, `rollup` and `counts` `null`. Present: each node's values equal `spec trace`'s. `get_flow`, `GET flow` agree. A newer queue: both `null`, a note, exit 0. M: `rollup` filled without the flag.
- [x] AC-04 — A stubbed `counts` of `{confirmed: 7}` draws "7 confirmed" on a closed group whose read holds none. M: a count recomputed from the group's read.
- [x] AC-05 — A spy on load sees `getTrace({summary: true})`, opening a lane `{stage}`, never `{}`. M: a whole read on load.
- [x] AC-06 — A step with no card stands in its lane as "No card"; accepting the last card moves the lane to `<n> of <n>` through a `card.accepted` event, no reload. M: hiding `no_card` steps.
- [x] AC-07 — `spec trace --stage print` lists its two steps across both parts; `--stage -` the unstaged step alone, no group; `--summary` no node row; the summary and stage lines stay whole under each and past the cap; lanes by least `distance`, the `null` stage last, summing to less than the header by the book and its two parts; two file orders byte-identical. M: every listed node put in a lane.
- [x] AC-08 — The Cards block links to the board and back; a suspect card's Clear finishes on the page; the filter dims on the Canvas, hides in the List; a child's edge to its group is in the List, not on the Canvas. `policy.test.ts` scans `src/trace/`. M: a branch on a rollup word outside `labels.ts`.
- [x] AC-09 — A card made suspect, by a `proposal.applied` event with no reload, and a departed one each show their word and icon on the tile, accepted or not; `list --marks` equals `spec trace`'s marks; a failed mark read draws no tile. M: a tile drawn without its mark.

## Verification

Review took three iterations on one rule: a person never reads "fine" while the state is unknown. Gates: fmt, clippy, probes 0; `cargo nextest run --workspace` 4388 run, 4388 passed, 71 skipped; UI lint 0, vitest 3199 passed, build 0; parity 3 of 3; 86 of 86 doc tests. Mutations 8 of 8 killed. `--summary` builds light rows, 70 reads byte-identical.

## Out of scope

Dates on the timeline: nothing here reads a clock. A `[flow]` key ordering stages. Overrides and `pack:` (RM-0058). Drawing a group's edge to its child.

## Implementation

| Module | What it does |
|---|---|
| Model, core, store, import | `stage`, `TYPED_KEYS` 31, `INDEX_FORMAT` 14; folded `start`, `end`; `flow_walk`, `stage_of` |
| CLI | `--stage`, `--summary`, `stages`; `--trace`, `TraceBadge`; `list --marks` |
| Daemon, MCP, plugin | The same keys on routes and tools; `INSTRUCTIONS` 2 048 B; 0.25.0 |
| UI | Badges, Timeline, filter, the fit; `src/trace/`; Board marks from the list |

A person never reads "fine" while the state is unknown. A failed list re-read puts `ErrorPanel` where the Board's columns stood and takes the panel's mark and Clear. A failed flow re-read drops that read's badges and counts (`withoutStates`); the Timeline and the Cards block say their numbers are from the last read. A group with no stage of its own is not judged by the stage filter (`outsideStage`, `keptInList`). `window` `focus` reaches `refetchOnWindowFocus` through `focusManager.setEventListener`.

A failed re-read leaves no state above its line: the Cards block drops its state row, and the top read's failure takes every gap row's state, items and cause ("Not read") and the top level's Gap marks. Each Retry hands focus on. An unknown mark word is a neutral badge, never a clean tile.

Beyond the spec: the Flow's fit, for the 92 px box (`spec-flow-layout.md` "Layout"). `TimelineView.tsx`: `Timeline.tsx` collides with `timeline.ts` on macOS.

### Files in scope

New: `ui/src/trace/`, `ui/src/flow/{TimelineView.tsx,timeline.ts}`, `ui/src/mocks/trace.ts`; CLI `tests/trace_views.rs`; `fixtures/trace-almanac/`; canon `spec-trace-record.md`, `spec-flow-layout.md`.
