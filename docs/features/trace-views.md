---
class: spec
status: draft
scope: [ui, crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-import, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin]
ref: RM-0048; Phase 7 step 7 (08 s2)
---

# trace-views: flow badges, `stage:`, the timeline, a node's cards (RM-0048)

## Why

This closes the first result: the owner, alone on the local daemon, sees a whole cycle. The diagram shows what is confirmed, the timeline how far each stage has come, and a node its cards, all read from `card-trace.md`'s rollup. The UI joins nothing and computes no rollup.

## Description and interactions

### The key `stage:`

A document's front-matter `stage: gather`, typed `Text`, in any script: project vocabulary nothing resolves or branches on. A plan item's ID is one value a project may write, and nothing reads it as one. Never counted (`node-revision.md` "Never counted"), so re-cutting stages raises no revision and marks no card, and it cuts across groups, which `parent:` cannot. A section has none; a mistyped value is `frontmatter-type`, as any typed key.

`TYPED_KEYS` 31, `Fields.stage`, `INDEX_FORMAT` 14 with its `format_history.txt` line; today the key is `unknown-key`, kept in `extra`.

`spec flow` reads it: JSON `stage` after `group`, a node line ` | stage <s>`. A folded node shows its own.

### Folded start and end

A shown node's `start` and `end` become true when it or anything it holds is one, so the overview marks where a flow begins and ends. This reverses the depth rule of `spec-flow.md` "The read", which keeps them to the node's own. `shown` in core `flow.rs` folds them beside `distance`, over the same members. Gap rows, computed before any fold, do not move. A closed group then says Start or End at Top, Entry in a group's lane, as `FlowCanvas.tsx` already words it.

### Flow badges

`spec flow --trace` adds, per shown node, `rollup` and `counts` from `spec trace`; `null` without the flag, so every other read keeps its bytes. The CLI joins them after core's read: core `flow.rs` stays pure and reads no queue. Text ` | rollup <state>`. MCP `get_flow` gains `trace?`, the daemon `trace=true`, and the Flow section always sends it.

- **A box**: its rollup, a label and an icon from `src/flow/labels.ts`, an unknown value a neutral badge.
- **A closed group**: its served `counts`, `3 confirmed, 1 suspect, 2 no card`, zero parts left out. Counted over what it holds by the daemon, never by the UI, so folding moves nothing.

### A node's cards

A Cards block (`src/trace/CardsBlock.tsx`) in `FlowDetails.tsx` and the tree's `NodePane.tsx`, reading `getTrace(ref)`: the rollup, then each card's title and mark, a link to `#/<p>/board/<C>`; `uses` rows linking their nodes; New card. A suspect card has a Clear button, the dialog of `card-trace.md` "Clearing"; a group's block adds Clear all, no effect only. The board's panel already links each target back, and gains its rollup word.

### The board tile

`card-trace` already shows the badge on the tile from `GET trace`, which misses a card on a gone target; this section moves it onto the list. A card can never look normal on the board while suspect or departed. `spec card list --marks` adds each row's `mark`, core `card_mark`'s, `null` without the flag, so other reads keep their bytes; the Board always sends `marks=true`, MCP `list_cards` gains `marks?`. `BoardCard.tsx` shows Suspect, Departed or Correcting first on the tile, a label and an icon from `src/board/labels.ts`, naming the target, the discrepancy or the correction; an accepted card's word yields to it.

No marks, no tiles: an index that cannot be read is the read's 503, so `ErrorPanel` stands where a clean board would. `card.*` and `proposal.applied` read the list again, and so does window focus, for a hand edit.

### Timeline

A third tab of the Flow section beside Canvas and List: lanes by stage, in `stages[]` order, "No stage" last.

- **A lane** shows the stage, `<c> of <n> confirmed`, and a bar drawn from `counts`, each part labelled, never by colour alone.
- **Opening a lane** reads `getTrace({stage})` once and lists its steps by `distance`, each with its rollup. A `no card` step keeps its place, hollow, "No card": the gaps stay visible.
- **It fills** as cards are accepted: `card.*` events read `stages` and the open lanes again. Flow's gap rows stay beside it.

Never a whole read: on load `getTrace({summary: true})`, then one read per open lane.

### Stage filter

A select over the Flow section, per-project in `FlowMemory`: All, each stage, No stage. The Canvas dims other boxes, keeping the layout; the List hides them. Options come from `stages[]`, comparing nothing but the served value.

### The undrawn edge

An edge between a group and its own child stays undrawn unless the review says otherwise: a group's own read leaves the group out of `DrawnRead.boxes`, so such an edge has no second end. The List keeps it, and the panel's links.

### States

Loading, failed: as `flow-canvas.md`. No card anywhere: lanes at `0 of <n>`, and "No cards yet. Next: open a step and press New card." No `stage:` anywhere: one lane, "No stage", and "Next: give documents a `stage:` key". A bad `[flow]`: `distance` null, lanes by name, the daemon's note.

## Data

### `spec trace`, extended

```text
spec trace [REF] [--stage S] [--summary]
stage <name or -> | distance <d or -> | no card <a>, in work <b>, suspect <s>, departed <d>, confirmed <f>
```

Stage lines follow the summary line, always whole. `--stage S` keeps the node rows of one stage, `-` those with none. `--summary` prints no node rows. JSON gains `stages` `[{stage, distance, counts}]`; a node gains `stage`, `distance`. A lane's `distance` is the least of its steps', from core's walk; `stages` by (`distance`, unreached last, `stage`), the `null` stage last.

Scope as `card-trace.md`: every live document, so `no card` steps count. A section's `stage` is its document's.

### Routes and types

| Route | Adds |
|---|---|
| GET `.../flow` | `trace=true`, `spec flow --trace` |
| GET `.../trace` | `stage`, `summary` |
| GET `.../cards` | `marks=true`, `spec card list --marks` |

```ts
interface TraceCounts { no_card: number; in_work: number; suspect: number; departed: number; confirmed: number }
interface TraceStage { stage: string | null; distance: number | null; counts: TraceCounts }
// FlowNode gains stage: string | null; rollup: string | null; counts: TraceCounts | null
// TraceView gains stages: TraceStage[]; TraceNode gains stage, distance; CardRow gains mark: string | null
interface TraceOptions { ref?: string; stage?: string; summary?: boolean }
getTrace(project, options, signal?): Promise<TraceView>;
```

`rollup`, `mark` stay `string` in the UI; `labels.ts` maps them. `fixtures/daemon-keys.json`: `FlowNode` 15 keys, `CardRow`, `TraceView`, `TraceNode`, `TraceCard`, `TraceUse`, `TraceStage`, `TraceCounts`.

### Fixture

`fixtures/trace-almanac`, a community almanac: `BOOK-ALMANAC` over `PART-GATHER`, `PART-PRINT`; steps `STEP-CALL`, `STEP-SORT`, `STEP-SET`, `STEP-BIND` on `follows`, staged `gather`, `gather`, `print`, `print`, cutting across the parts; one step without a stage; `[flow]` with `[[flow.start]]`, `[[flow.end]]`. Tests cut cards through the library.

## Rules and edge cases

- WHEN a badge, a lane or a mark shows anything THEN no status, control or exit changes (ADR-0012).
- `src/flow/`, `src/trace/` branch on no rollup, stage or mark word but through `labels.ts`.
- WHEN the rollup read fails THEN the canvas still draws, boxes without badges, a line with the daemon's words.
- `--trace` with a queue of another schema: `spec trace`'s reading, `no card` and a note, never a failed flow.

## Acceptance criteria

- [ ] AC-01 — Writing or changing `stage:` on `fixtures/trace-almanac` leaves every `norm_hash`, `rev` and card mark as they were; no `unknown-key`; `spec flow` prints it. M: counting the key in `norm_input`.
- [ ] AC-02 — `spec flow BOOK-ALMANAC --depth 1`: the part holding `STEP-CALL` is `start`, the one holding `STEP-BIND` `end`, at every depth above; gap rows byte-equal to the unfolded read. M: `steps.starts.contains(&file)` kept in `shown`.
- [ ] AC-03 — `--trace` absent: bytes as before but `stage`, `rollup` and `counts` `null`. Present: each node's values equal `spec trace`'s. `get_flow`, `GET flow` agree. M: `rollup` filled without the flag.
- [ ] AC-04 — A stubbed `counts` of `{confirmed: 7}` draws "7 confirmed" on a closed group whose read holds none. M: a count recomputed from the group's read.
- [ ] AC-05 — A spy on load sees `getTrace({summary: true})`, opening a lane `{stage}`, never `{}`. M: a whole read on load.
- [ ] AC-06 — A step with no card stands in its lane as "No card"; accepting the last card moves the lane to `<n> of <n>` through a `card.accepted` event, no reload. M: hiding `no_card` steps.
- [ ] AC-07 — `spec trace --stage print` lists its two steps across both parts; `--stage -` the unstaged one; `--summary` no node row; lanes by least `distance`; two file orders byte-identical.
- [ ] AC-08 — The Cards block links to the board and back; a suspect card's Clear finishes on the page; the filter dims on the Canvas, hides in the List; a child's edge to its group is in the List, not on the Canvas. `policy.test.ts` scans `src/trace/`. M: a branch on a rollup word outside `labels.ts`.
- [ ] AC-09 — A card made suspect, by a `proposal.applied` event with no reload, and a departed one each show their word and icon on the tile, accepted or not; `list --marks` equals `spec trace`'s marks; a failed mark read draws no tile. M: a tile drawn without its mark.

## Out of scope

Dates on the timeline: nothing here reads a clock. A `[flow]` key ordering stages. Overrides and `pack:` (RM-0058). Drawing a group's edge to its child.

## Files in scope

- **New:** `ui/src/flow/{Timeline.tsx,timeline.ts,timeline.test.ts}`, `ui/src/trace/{CardsBlock.tsx,CardsBlock.test.tsx}`, `ui/src/mocks/trace.ts`; CLI `tests/trace_views.rs`; `fixtures/trace-almanac/`.
- **Rust:** model `src/node.rs`; core `src/front_matter.rs`, `src/flow.rs`; store `src/lib.rs`, `tests/format_history.txt`; import `src/layout/typed.rs`; CLI `src/flow.rs`, `src/trace.rs`, `src/card.rs`, `src/main.rs`; MCP `src/read.rs`, `src/cards.rs`, `src/mirror.rs`; http `src/app.rs`; `plugin.json`, next MINOR.
- **UI**, in `ui/src/`: `flow/{FlowView,FlowCanvas,FlowList,FlowDetails}.tsx`, `flow/labels.ts`; `tree/NodePane.tsx`; `board/{BoardCard,CardPanel}.tsx`, `board/labels.ts`, `BoardView.test.tsx`; `api/{provisional,client,http,queries}.ts`; `mocks/MockClient.ts`; `styles/app.css`; `policy.test.ts`.
- **Pins:** `norm_recipe.rs`, `front_matter.rs` tests, eval `layout_cli.rs`, `flow_read.rs`, `mcp_flow.rs`, `SCHEMA_PINS`, `daemon-keys.json`, `daemon_keys.rs`, `daemonKeys.test.ts`, genre scans.
- **Canon at shipping:** `spec-flow.md` "The read" (the reversal, `stage`, `--trace`), `node-revision.md` "Never counted", `spec-trace.md`, `spec-flow-screen.md`, `ui/README.md`.

## Implementation
