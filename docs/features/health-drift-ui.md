---
class: spec
status: draft
scope: [ui]
ref: Phase 3 slice 9b-2 of RM-0009, split from health-drift 2026-10-21 (Q-F, Q-G, Q-H; owner pre-approved, review open); 06 s7; 07 s3; refreshed against 9b-1's canon before it starts
adrs: [ADR-0047]
---

# health-drift-ui (9b-2): Health draws row 6, drift

## Why

9b-1 (`docs/features/health-drift.md`, shipped 2026-10-21; canon `docs/canon/health-drift.md`) gave `spec health` and its route the `drift` key and the UI its provisional types, but draws nothing. Row 6 still says "Not measured yet" / "Phase 3". This slice draws it, so the owner sees drift in Health without a terminal. 9b-1's "Data" is frozen.

## Description and interactions

- `ui/src/health/DriftPart.tsx` draws row 6, "Drift", beside rows 4-5 (frame `codeRows.tsx`). It replaces the `UNMEASURED` block of `LeftRegion.tsx`. Task W's "Not measured yet (the bundles log)" in Check stays.
- **States**, as 9a's (`health-code-ui.md`):
  - being measured;
  - not measured: `off` or `cannot` with its causes, Retry on `cannot`;
  - the measured-without-counts fallback ("Not measured: the answer holds no counts"), where `RowNotMeasured` accepts `HealthDrift`;
  - the read failed.
- **Measured**:
  - verify's counts;
  - the rows in the daemon's order, 10, then "Show all <n>";
  - each row: its class through the map (`fix` "Fix", `check` "Check", `cannot` "Cannot verify" in the `cannot-verify` look); its state through `syncLook` (`ui/src/tree/labels.ts`); category and detail raw; the node linked to the tree; relation, file and binding plain; a moved row's `from` and an ambiguous row's candidates as sub-lines;
  - the stale evidence (`tested`) listed, its state through a map, its node linked;
  - `tested` `null`: its note shown.
- **Zero**: "None" plus "Measured over <rows> bindings, <skipped> skipped".
- **Triggers**: none new (Q-F); the health read's own triggers stay, the trailing read included, and `verify.tests` is not subscribed. Code edits, `lock accept` and evidence commits need "Check again" (Known limits).
- No actions and no commands from the UI.

## Data

No new key: the UI reads `HealthReport.drift` (`docs/features/health-drift.md` "Data"). `ui/src/health/labels.ts` gains the class map and the tested-state map; an unknown word shows raw and neutral.

Mocks (`ui/src/mocks/health.ts`, fictional names), replacing 9b-1's `off`:
- harbor-sim: measured, with every class (a `cannot_verify` row), a moved anchor, and a `stale` and an `unverifiable` entry;
- ledger-api: `off`;
- `empty`: measured, zero;
- `large`: 3 000 rows;
- `code-cannot`: row 6 `cannot` too.

## Rules and edge cases

- WHEN row 6 is `off` or `cannot`, the UI SHALL draw no digit for it.
- The UI SHALL neither sort rows nor compare a class or state outside `labels.ts` (Q-H).
- `fix` shows as "Fix". `blocked` is never written (`ui/README.md` "Text").

## Acceptance criteria

- [ ] UI-2: `DriftPart.tsx` with the states above; the row-6 test of `HealthView.test.tsx` (:258-264) is replaced by them. M7: a 0 drawn for a `cannot` or `off` row 6 → `ui/src/health/HealthView.test.tsx`.
- [ ] UI-3: no sort and no second map. M8: a UI sort, or a class or state compared outside `labels.ts` → `ui/src/mocks/health.smoke.test.tsx` or `ui/src/policy.test.ts`. Writing `blocked` stays caught by `crates/specengine-eval/tests/ui_policy.rs` `nothing_is_worded_as_a_hold_on_work`, unwidened.
- [ ] UI-4: no new trigger. In `ui/src/api/health.live.test.tsx`, a stage, `proposal.created` or a `task.*` event causes no health re-read.
- [ ] UI-5: the mocks as "Data"; `ui/src/mocks/health.test.ts` holds each one to its type.
- [ ] UI-6: `pnpm lint` reports 0, then `build`, then `test` twice; no new package. `ui/src/styles/tokens.css` and `tokens.test.ts` change only for new tones.
- [ ] SW: at shipping, `ui/README.md` :87 (the mocks), the "UI" sections of `docs/canon/health-drift.md` and `health.md`, and this spec compacted.

## Out of scope

- Per-node views, the diff inspector and typed drift-item evidence (RM-0011).
- Actions, sorting and filters.
- New triggers.

## Implementation

Not started. Files: `ui/src/health/DriftPart.tsx` (new), `LeftRegion.tsx`, `labels.ts`, `labels.test.ts`, `codeRows.tsx`, `HealthView.test.tsx`; `ui/src/mocks/health.ts`, `health.test.ts`, `health.smoke.test.tsx`; `ui/src/api/health.live.test.tsx`.
