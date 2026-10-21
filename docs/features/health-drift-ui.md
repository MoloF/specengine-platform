---
class: spec
status: shipped
scope: [ui]
ref: Phase 3 slice 9b-2 of RM-0009, split from health-drift 2026-10-21 (Q-F, Q-G, Q-H; owner pre-approved, review open); 06 s7; 07 s3; UI 1 iteration and a cleanup round, review 1 accepted
shipped: 2026-10-21
adrs: [ADR-0047, ADR-0033]
---

# health-drift-ui (9b-2): Health draws row 6, drift

## Why

9b-1 (`docs/features/health-drift.md`; canon `docs/canon/health-drift.md`) gave `spec health` the `drift` key; Health still showed row 6 "Not measured yet". The owner reads Health, not a terminal: out-of-sync bindings, verify's counts and stale test evidence go beside rows 3-5. Visibility, not a gate (ADR-0047, ADR-0012); provisional types over 9b-1's frozen "Data" (ADR-0033).

## Description and interactions

Delivered: `ui/src/health/DriftPart.tsx` draws row 6 "Drift" after "Unbound nodes" in rows 4-5's frame (`codeRows.tsx`), replacing the "Not measured yet" block; task W's "Not measured yet (the bundles log)" stays. Now: `docs/canon/health-drift.md` "UI"; frame, states, Retry, triggers `docs/canon/health.md` "UI".

- **States**: "Measuring the drift"; "The drift could not be read" (an answer shown: "... could not be read again: <message>"), verbatim, unspoken, Retry; "Not measured: Code layer off" (an unlisted word raw) or "Not measured: the answer holds no counts" (`counts` or `rows` null), causes, no Retry; `cannot` "Could not be measured", causes, Retry (`getHealth` alone).
- **Measured**: notes; "31 of 39 bindings, 2 skipped", the tally (four classes, zeros too); zero rows "None: every binding `spec verify` judges is in sync." then "Measured over <bindings>, <skipped> skipped.". Rows in the daemon's order, 10 then "Show all <n>": class and state badges, the node linked to the tree (`""` "No ID"), relation, category, detail, file, `<kind> <symbol>[ <item_kind>]` or "No symbol", "Moved from", "Candidate"; `markers`, `node_rev`, `lock`, `compared`, `levels`, `pre_existing` left to RM-0011.
- **Test evidence**, zero rows too: entries by node (badge, node, category, "Vouched for rev", "Now rev"/"No revision now", "Commit", "Tests passed" `<time>`); `[]` "None: no `[[tested]]` entry is stale, unverifiable or without its node."; `null` "Not judged: the test evidence could not be judged on this read."

## Data

No new key, type or token; the UI reads `HealthReport.drift` (`docs/features/health-drift.md` "Data"). `ui/src/health/labels.ts` holds the only class and tested-state tables, label (tone, icon) on existing `.tone-*` roles:

- `driftClassLook(string | null)`: `fix` Fix (`finding-error`, `findingError`); `check` Check (`finding-warning`, `findingWarning`); `cannot` Cannot verify (`sync-cannot-verify`, `cannotCheck`); `null` "No class", another word raw (`sync-unknown`, `unknown`).
- `testedLook(TestedState)`: `stale` Stale (`stale-changed`, `deferred`); `unverifiable` Unverifiable (`sync-cannot-verify`, `cannotCheck`); `node_gone` Node gone (`sync-broken-binding`, `linkDangling`); `verified` Verified (`sync-ok`, `checkClean`); another raw (`sync-unknown`, `unknown`).
- `driftTally(counts)`: `fix`, `check`, `cannot`, then `ok` as `syncLook("ok")`; `DRIFT_CLASSES`, `TESTED_STATES` the orders. `CodePart`'s `row` gains `"drift"`; `RowNotMeasured` takes `HealthDrift`.

**Mocks** (`ui/src/mocks/health.ts`): harbor-sim measured in `normal`, `conflict`, `editorial`, `error`, `slow`, `not-configured`: 12 rows (every class, verify's eight non-`ok` states, two keyless, a moved anchor, an ambiguous one with two candidates, `unlocked` without `lock`) in verify's key order, which no sort by class, state or file gives; `tested` `stale`, `node_gone`, `unverifiable`. ledger-api `off`, in `not-configured` measured with zero rows. `empty`: both zero rows, `tested` `[]`. `large`: harbor-sim 3 000 (the 12 and `unlocked` flood rows). `code-cannot`: both `cannot` (`CODE_GAP_CAUSE`; `LOCK_CAUSE`). Counts match the rows per class; `DriftRow`'s 17 keys in order.

## Rules and edge cases

- WHEN row 6 is `off`, `cannot`, an unlisted word, or `measured` without `counts` or `rows`, the part SHALL hold no `.health-value` digit, tally or count: never 0 (M7).
- The UI SHALL NOT sort, nor compare a class, state or tested state outside `labels.ts` (`!== null` is fine); category, detail, relation, kind, symbol and file show raw (Q-H; health-left's AC-U6, extended; M8).
- An unlisted word SHALL show raw and neutral; `cannot` and `unverifiable` SHALL take cannot-verify's look, never ok's.
- `fix` shows as "Fix"; never `blocked`, a verdict, a mode, a hint or "block" wording.
- A failed read SHALL draw in rows 3-6 alike, one alert (row 3's); row 6's boundary leaves rows 3-5 standing; rows 4-5's own causes (queue, `[health]`, `code_refs`) leave row 6 measured.

## Acceptance criteria

ui-developer (UI-1 was 9b-1's). Mutations M7a, M7b, M8a, M8b, all killed; with 9b-1's M1-M6, RM-0009 has 8 counted.

- [x] UI-2 -- `HealthView.test.tsx` "row 6: drift": each state's text ("read again" too), Retry (one `/health` per click), value, tally, "Show all <n>" focusing row 11, each row field, zero rows, `tested` listed, `[]`, `null`. M7a (a 0 for an unmeasured row 6), M7b (unmeasured drawn as measured, zero counts) -> red.
- [x] UI-3 -- M8a (a sort by class, state or file) -> `health.smoke.test.tsx` "normal: harbor-sim's row 6 in the daemon's order" red; M8b (`row.class === "fix"`, `entry.state === "stale"` outside `labels.ts`) -> `policy.test.ts` AC-U6 (gains `DriftPart.tsx`) red. `ui_policy.rs:938` (`blocked`) unwidened; `labels.test.ts` every word, an unknown one.
- [x] UI-4 -- `health.live.test.tsx` :213 (the never-list) passes unedited; added: row 6 measured, `proposal.applied` -> one `/health`, the new row 6 drawn. `queries.ts` comments only.
- [x] UI-5 -- mocks as "Data" (`mocks/health.test.ts`: keys, agreements); smoke `normal`, `large` (Show all 3000, 20 s), `code-cannot`, `empty`; `error`, `slow`, `cannot-check` count row 6.
- [x] UI-6 -- `pnpm lint` 0, `build` clean, `test` 2 315 of 2 315, run twice; 17 packages; `tokens.css`, `tokens.test.ts` unchanged; eval `ui_policy`, `anonymity`, `doc_pointers` green.

spec-writer:

- [x] SW-1 -- this spec (2026-10-21).
- [x] SW-2 -- 2026-10-21: canon `health-drift.md` (intro, "UI", "Tests"), `health.md` "UI"; `ui/README.md` scenarios (8 190 of its own 8 192 B, ui-shell AC-20); 07 s3; 08 s2; `RM-0009` shipped; `CLAUDE.md` State.

## Out of scope

RM-0011's views, counters, bindings tab, diff inspector, typed evidence; actions, sorting, filters, hints; new triggers; any crate, route, key, type, package or token; the home.

## Risks

- **Shared looks**: "Fix" takes the check's "Error" look, "Stale" a task's "Spec changed" tone; Fix and Stale have their own labels; `cannot` shares `cannot_verify`'s look and label.

## Implementation

| Module (`ui/src/`) | What it does |
|---|---|
| `health/DriftPart.tsx` (new) | `DriftPart` (states via `CodePart`, `RowNotMeasured`), value and tally, `DriftLine`, `TestedPart`, `TestedLine`, `NodeLink`; `.health-drift-evidence` a test hook, no CSS |
| `health/codeRows.tsx`, `LeftRegion.tsx` | `CodePart` row `"drift"`; `RowNotMeasured` takes `HealthDrift`; row 6 after row 5, `healthBusy` over rows 3-6 |
| `health/labels.ts` | `DRIFT_CLASSES`, `driftClassLook`, `driftTally`, `TESTED_STATES`, `testedLook` |
| `mocks/health.ts`, `scenario.ts` | row 6 per scenario ("Data") |
| `styles/app.css` | dead `.health-unmeasured*` dropped |
| `api/queries.ts`, `provisional.ts`, `health/AmendmentsPart.tsx` | comments only |
| tests | `test/builders.ts` (`aDriftRow`, `aTestedRow`, `measuredDrift`, `aDriftIn`); `HealthView`, `labels`, `mocks/health`, `health.smoke`, `health.live`, `policy` |

`policy.test.ts` gains a Health-wide test: nothing in `src/health/` sorts but `findings.ts`; a `Record<` or `satisfies` table lives only in `labels.ts` (a heuristic; M8b's scan covers the rest).

Divergences, accepted in review 1:

1. A smoke case reading every `.health-tally-count` is scoped to "Open proposals by severity": row 6 has its own tally.
2. harbor-sim's row 6 is measured in the `error` and `slow` mocks too.
3. Two mock nodes lie outside harbor-sim's corpus on purpose: a look-alike ID (`LOOKALIKE_BASIN`, a Cyrillic capital A written escaped) and a gone node (`RULE-QUAY-LIGHTS`, `node_gone`).
4. The `large` rows name generated `src/gen/*.rs` files, not harbor-sim's code.
