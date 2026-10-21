---
class: spec
status: shipped
scope: [ui]
ref: RM-0033 slice 2 of 2 (the RM-0011 breakdown's slice 5, the last; owner pre-approved, review open); 07 s3 Node row; accepted at iteration 1, iteration 2 (a11y, test tightening) pre-approved
shipped: 2026-10-21
adrs: [ADR-0053, ADR-0033]
---

# lock-diff-ui (RM-0033, 2 of 2): the lock diff in the Bindings tab

## Why

A drifted row in the node's Bindings tab says that the spec, the code or both moved since the binding was locked, not what moved. Slice 1 (`docs/features/lock-diff.md`, shipped 2026-10-21) serves the hunks (`client.getLockDiff`, `mocks/lockDiff.ts`); the contract: `docs/canon/spec-lock-diff.md`. This slice draws them in the tab, on demand, under each drifted row with its commands, so a drift item or an accept is decided without a terminal round trip. Read-only, gating nothing (ADR-0053, ADR-0012); UI only (ADR-0033).

## Description and interactions

As built: `docs/canon/lock-diff-panel.md`. `tree/LockDiffPanel.tsx {project, nodeRef, onFollow?}`, the disclosure "Changes since the lock", sits in the Bindings tab after the holders and before "Commands for a terminal". Collapsed at mount; the first expanding reads `lock-diff?ref=<REF>` once, kept while the pane shows the node; `proposal.applied` and a gap read it again; its own "Read again" sends one GET per click. Each kept row is Health's `DriftLine` carrying its sides (spec then code: two side by side, a lone one spanning, stacked below 48em; hunks in `DiffView` as sent; `""` "No change since the lock"; `null` "No diff: <why>" from its note) and its commands (`nodeCommands(row.node)` in the shared `tree/CommandList.tsx`).

## Data

No new route, daemon key set, type, package, token or scenario: slice 1's `LockDiffView {ref, reason, notes, nodes}`, `LockDiffNode`, `LockDiffRow {row, spec, code}` and the two sides. A side `null` did not move; `diff` `""` is equal, `null` not shown (one note); `rows` `null` is not judged (N0), `[]` none.

- **Query** (`api/queries.ts`): `queryKeys.lockDiff(project, ref)` = `["lock-diff", project, ref]`; `useLockDiff(project, ref, enabled)`; `"lock-diff"` in `READS_OF_THE_SPEC`, not `WALKS`.
- **Helpers** (`tree/lockDiff.ts`): `rowKeyOf(row)`, moved from the mock, which imports it back; `sideNoteOf(notes, row, side)`, the <why>, raw.
- **Mocks**: `mocks/lockDiff.ts`'s four harbor-sim rows (canon "Mocks"); RULE-HIGH-WATER gained a spec side, `diff` `null` with its N7 note, at `HIGH_WATER_FILE`.
- **Texts, states, layout, a11y**: canon "Texts", "States", "Layout", "a11y".

## Acceptance criteria

Paths under `ui/src/`. M1-M7: 7 of 7 red, then reverted (tests hit: M1 1, M2 2, M3 2, M4 1, M5 7, M6 3, M7 1).

- [x] UI-1 `tree/NodePane.test.tsx`: opening Bindings sends no `lock-diff` request; the first expanding sends exactly one `GET /api/projects/<p>/lock-diff?ref=<REF>`; collapsing, expanding again and reselecting the tab, none; the panel sits after the holders, before the commands, an h2. M1 (the read enabled at mount) red.
- [x] UI-2, split at review because AC-05 forbids importing mocks outside `src/mocks/`. In `tree/LockDiffPanel.test.tsx`, stub views copying the mock: a conflict's two figures, spec first; a lone code side spanning; each figure's `<code>` text, `.sr-only` removed, equals its `diff` byte for byte (tabs, trailing spaces, a CR, a non-Latin line, two hunks); `app.css?raw` holds the two-column, lone-side and 48em rules. In `mocks/tree.smoke.test.tsx` "normal: changes since the lock", the real mock: the four "Mocks" rows on their nodes. M2 (hunks through `oneLine`) red.
- [x] UI-3 `tree/LockDiffPanel.test.tsx`, stub views: `""`; `null` with N1, with N7, with no note; a `null` side, both `null`; `rows` `null` (no list, "None" or digit); `[]`; `reason` verbatim; notes on one line; loading, failed with Retry, failed again; several holders. M3 (a `null` diff drawn as "No change since the lock") red.
- [x] UI-4 `api/live.test.tsx`: expanded once, then collapsed, `proposal.applied` sends one `lock-diff` GET, `proposal.created` none; never expanded, none; Read again one GET per click, GETs only. M4 (`"lock-diff"` outside `READS_OF_THE_SPEC`) red.
- [x] UI-5 `tree/LockDiffPanel.test.tsx`: a `LAW-BOUND` row lists the three commands in order with Copy, meanings compared with `nodeCommands`; a `node` of `""`, `A B`, `X;Y`, `$(x)`, a backtick, a quote or a Cyrillic look-alike gets the refusal and no `.command-text`. M5 (commands from `row.node` without `nodeCommands`) red.
- [x] UI-6 same file: `getByRole("figure", {name})` finds each figure by its title, none "Section diff"; lines speak "Added: " and "Removed: "; `aria-expanded` false then true, `aria-controls` only expanded; focus stays on the button through expanding, rendering and collapsing. M6 (the `label` dropped) red.
- [x] UI-7 `policy.test.ts`: `tree/LockDiffPanel.tsx`, `tree/lockDiff.ts` in `ROW_SOURCES`, `tree/CommandList.tsx` in the tab's; `mocks/lockDiff.test.ts` the four rows, the binary pair; `mocks/tree.smoke.test.tsx` ledger-api not judged and none; 17 packages, `tokens.css` unchanged; Gates below. M7 (`row.state === "conflict"` in the panel) red.
- [x] TE-1: M1-M7 as above. The tester's O1 (`notes.find` to `findLast` in `sideNoteOf`) survived the developer's tests; `tree/lockDiff.test.ts` (3 tests: the first of two notes taken; a note holding the prefix but not starting with it, not taken) kills it. Eval `ui_policy`, `anonymity`, `doc_pointers` 46 of 46; `daemon_keys` 4 of 4, at 81.
- [x] SW-1: `docs/canon/lock-diff-panel.md`; `drift-screens.md` "Panel" and "Line"; `spec-lock-diff.md`'s intro; 07 s3, 08 s2; RM-0033 shipped; `CLAUDE.md`; this spec compacted; `export index && check` clean.

**Owner's manual check** (not run by the roles): `spec serve` over a scratch `fixtures/code-a` after `spec lock accept LAW-BOUND`, its sentence and `CAP` edited: no lock diff read until expanding; then CAP's `conflict` with both sides as the canon's example; stacked under 48em.

## Out of scope

Any write or stage; a `spec lock diff` row command; a deep link; a word diff or an editable merge; cutting long hunks; diffs against `HEAD`; a new trigger or scenario.

## Implementation

| Module (`ui/src/`) | What it does |
|---|---|
| `tree/LockDiffPanel.tsx` (new) | the disclosure, its gated read, Read again (`aria-describedby` the disclosure), states; per holder the rows as `DriftLine`s carrying `Sides` and `RowCommands` |
| `tree/lockDiff.ts` (new) | `rowKeyOf` (moved from the mock), `sideNoteOf` |
| `tree/CommandList.tsx` (new) | meaning, text and `CopyButton` per command; the tab's block and the rows |
| `tree/BindingsPanel.tsx` | mounts the panel after the holders; its commands through `CommandList` |
| `health/DriftPart.tsx` | `DriftLine` gains `onFollow?` (its node link) and `children?` (drawn last) |
| `api/queries.ts` | `queryKeys.lockDiff`, `useLockDiff`, `"lock-diff"` in `READS_OF_THE_SPEC` |
| `styles/app.css` | `.lock-diff*`: the grid, the lone side, 48em; existing tokens |
| `mocks/lockDiff.ts`, `mocks/harbor-sim/fixtures.ts` | RULE-HIGH-WATER's spec side (N7), `HIGH_WATER_FILE`; `rowKeyOf` imported |
| tests | `tree/LockDiffPanel.test.tsx`, `tree/lockDiff.test.ts` (new, TE-1), `tree/NodePane.test.tsx`, `api/live.test.tsx`, `mocks/lockDiff.test.ts`, `mocks/tree.smoke.test.tsx`, `policy.test.ts` |

Divergences from the draft, accepted at review: UI-2 split (above); props `{project, nodeRef, onFollow?}`, the tree view's `onFollow` threaded to `DriftLine`; `reason` drawn verbatim like the tab's, not through `oneLine` (the other daemon words are); apostrophes U+2019 in markup text, straight in attributes and in this spec; the panel's Read again described by the disclosure, as two share the tab. Iteration 2: that `aria-describedby`, a real focus check, meanings compared with `nodeCommands`, a CSS guard (the base `.lock-diff-side` has `min-width: 0`, no `grid-column`), the `CommandList` comment; re-checks R1-R3 red, restored.

Gates, iteration 1: lint 0; main chunk 480.11 kB (474.46 before); `pnpm test` 2 542 twice, 2 545 after TE-1. Iteration 2: lint 0; 480.13 kB; `pnpm test` 2 546 of 2 546 twice; eval quartet 51 of 51. Final run: `pnpm test` 2 546 of 2 546, build: main chunk 480.13 kB, lint 0, fmt clean, docs and eval tests (parity, shards_repo, check_parity, doc_pointers, ui_policy, anonymity, build_graph): 101 of 101.

Untested: the re-read on an event gap (only `proposal.applied`); the body's `aria-busy` during a re-read; the real layout (jsdom matches only the CSS text).
