---
class: spec
status: draft
scope: [ui]
ref: RM-0011 slice 3 of 3, split 2026-10-21 (Q-B, Q-D; owner pre-approved, review open); 07 s3 Tree row; docs/canon/health-drift.md; a compact draft, refreshed before it starts
adrs: [ADR-0047, ADR-0033]
---

# ui-tree-drift (RM-0011, 3 of 3): drift badges on the tree, from Health's row 6

## Why

The tree shows kind, status, rev, archived, mark and the inbox count, but no `sync` (07 s3 Tree row). Health's row 6 already judges every key of the tree (`docs/canon/health-drift.md` "Reading"). The tree can count those rows per node with no new read, key or route (Q-B). The CLI and MCP counters wait in RM-0034. No ADR.

## Description and interactions

- **Toggle**: a per-project "Show drift" toggle in the Shell's screen state. While it is off, the Tree triggers no Health read.
- **Read**: while it is on, the Tree observes `useHealth` (`ui/src/api/queries.ts`). That is a walk, never aborted, and its cache is shared with the Health screen. The tree draws first; "Measuring drift" shows while the walk runs.
- **Badges**: per tree row, the node's own row 6 rows by class: Fix, Check, Cannot verify (`driftClassLook`, text and icon). They are matched by `namesOf` as the inbox counter matches (`tree/matching.ts`, `TreeView.tsx`). A row never counts on its parent (Q-D). There are no badges for `tested` or row 5.
- **Row 6 `off` or `cannot`**: one line with the causes, no badge, never 0.
- **Unmatched rows**: rows that name no tree line (a bare `""`, an unresolved ID, a look-alike as written) are summarized in one line that links to Health.
- **Cost**: 59 ms at plus 1 000 files and 242 ms at plus 10 000 (`health-drift.md` "Known limits"). The first call after the daemon starts also builds the code index inside the turn (`docs/canon/serve.md` "Concurrency").

## Data

- No key, type or route.
- `ui/src/tree/drift.ts` (new): rows grouped by `node`, then matched to tree rows, giving `Map<rowKey, {fix, check, cannot}>` and an unmatched count.
- The map is memoized on the Health answer, so only rows whose counts change re-render.

## Acceptance criteria

- [ ] UI-1: `tree/drift.test.ts`: rows grouped by `node`, matched by `namesOf`, never counted on a parent. M1 (rows rolled up into the parent) turns it red.
- [ ] UI-2: `TreeView.test.tsx`: the tree draws before Health answers, with "Measuring drift" while pending. M2 (the tree gated on Health) turns it red.
- [ ] UI-3: `off` or `cannot` gives one line with the causes and no badge, never 0. M3 (a 0 badge) turns it red.
- [ ] UI-4: rows that match no tree line get one summary line linking to Health. M4 (such rows dropped) turns it red.
- [ ] UI-5: `TreeView.renders.test.tsx`: a Health answer re-renders only the rows whose counts changed. M5 (a new map built on every render) turns it red.
- [ ] UI-6: badges have text and an icon; `policy.test.ts`. M6 (colour only) turns it red.
- [ ] UI-7: with the toggle off, the Tree triggers no Health read. M7 (a read while it is off) turns it red.
- [ ] UI-8: mocks; the `large` smoke (3 108 nodes, 3 000 rows) passes within 20 s; gates as slice 1's UI-8.
- [ ] TE: the mutations run.
- [ ] SW:
  - 07 s3 Tree row (114 B): drop one genre's domain words (ADR-0008).
  - `architecture-code.md` "Sync in reads" (12 287 of 12 288: rewording must not grow) and `code-reads.md` "Known limits" already point tree counters at RM-0034.
  - RM-0011 shipped; 08 s2 Phase 4 and the Phase 6 paragraph; `drift-screens.md`; `CLAUDE.md`.

## Out of scope

- `spec tree --bindings`, `get_tree` counters (RM-0034).
- Roll-ups, `ok` counts, not-judged counts.
- Sorting or filtering by drift; new triggers.

## Open before it starts

- The toggle's default (off is proposed: the plain tree never pays for a walk) and where its state lives.
- The summary line's text.
- How look-alike rows are summarized, given that Health does not place them under their fix (`docs/canon/spec-verify-scope.md` "`--node`" does).

## Implementation

Filled in after implementation.
