---
class: spec
status: draft
scope: [ui]
ref: Phase 3 plan 2026-10-10, slice 9a-2, split from health-code 2026-10-16 (its Q19; WA-9, WA-10, Q11 carried; decided under the owner's pre-approval, review open); 06 s7; 07 s3
adrs: [ADR-0047]
---

# health-code-ui (9a-2): Health draws rows 4-5

## Why

9a-1 (`health-code.md`) makes `spec health` and its route answer rows 4-5, yet Health still draws them "Not measured yet" (`docs/canon/health.md` "UI"). The owner looks at Health, not at a terminal: code built on a working answer the decision then changed, and accepted nodes no code binds, have to be on the panel, beside row 3, read the same way. Visibility, never a gate (ADR-0047).

## Description and interactions

- **The owner**, in Health's "What is left": row 4 "Code assuming a decision" and row 5 "Unbound nodes" become parts beside row 3's (`ui/src/health/AmendmentsPart.tsx`, the pattern), from the same read (`getHealth`, key `["health", p]`, `useHealth`); `LeftRegion.tsx`'s "Not measured yet" list (`:24-43`, `:157-163`) keeps row 6 alone until 9b.
- **ui-developer only**, `ui/**`: two parts in `ui/src/health/`, the state words in `labels.ts`, `mocks/health.ts`, tests. No Rust, no daemon key, no new package. Types: 9a-1's provisional ones (`ui/src/api/provisional.ts`); a change they need goes back to `health-code.md` "Data", never made here alone.
- **Order** (08 s2): built on mocks once 9a-1's "Data" is frozen at its review; ships right after 9a-1, before 12a.

## Data

The body: `health-code.md` "The document", "Row 4: `assumes`", "Row 5: `unbound`". Each part keeps four states apart (9a's Q11):

- **being measured**: a skeleton, `aria-busy`, no digit;
- **not measured** (`off`, `not_configured`): each cause verbatim, no digit, no Retry;
- **could not be measured** (`cannot`; or the whole read failed: a 503, an older daemon's 404, status 0): the causes, else the daemon's message, verbatim, a Retry reading `getHealth` alone; no `.health-value`, never 0;
- **measured**: alone with a digit. Zero: "None", then what was measured over: "over <counts.markers> markers in <code.files> code files" (row 4), "over <counts.selected> selected nodes" (row 5).

| Part | Value | A row | Beneath it |
|---|---|---|---|
| Row 4 | `<counts.rows>` rows of `<counts.markers>` markers | the state's label, the ID (a `PR-` ID linked to its inbox card, a node ID to the tree), `path:line:column`, the relation, the tail of `health-code.md` "Text" | — |
| Row 5 | `<counts.unbound>` of `<counts.selected>` selected nodes | the ID linked to the tree, kind and title raw, `path:line`, the failed rules, the mentions | each marker: `path:line:column`, relation, `id_state`, `binding`, the reason's label |

- Rows in the daemon's order; over 10 (`SHOWN_OPEN`) the first 10 and "Show all <n>", which focuses the first added row; paths plain.
- Every state word (row 4's eight, row 5's `reason`, `id_state`, `binding`) through one map in `labels.ts`; an unknown word shown raw.
- **Re-read** while Health is on screen also on `proposal.rejected` (row 4's `rejected`); as row 3 otherwise (entering Health, "Check again", `proposal.applied`, a stream gap); never `proposal.created`, focus, a reconnect, an interval, a stage or `task.*`.
- **Mocks** (`mocks/health.ts`): harbor-sim every row-4 state and row 5 measured with each reason; ledger-api both `off`; `empty` (both measured, zero), `large` (3 000 unbound), `cannot-check`, `error`, `slow`; new `code-cannot` (rows 4-5 `cannot`, row 3 measured) and `not-configured` (row 5).
- **Exceptions to `.claude/agents/ui-developer.md`** (ADR-0033): provisional types citing `docs/features/health-code.md` "Data"; no type generation (none exists).

## Rules and edge cases

- WHEN one row cannot be measured, the other parts SHALL draw as they are; a row not measured SHALL hold no digit.
- The UI SHALL sort nothing and compare no kind, class, status, state or reason outside the label map (health-left's AC-U6, extended to the two parts, not loosened).
- An unknown state word SHALL show raw, never fail the part (9b and later slices add words).
- English interface (ADR-0014).

## Acceptance criteria

ui-developer (M: mutation -> the test turning red; 9a's UI-2..UI-6, renumbered):

- [ ] UI-1 -- four states apart per row; `HealthView.test.tsx`'s "holds no digit in the value of rows 4 to 6" narrowed to row 6. M: 0 drawn for `cannot` -> `HealthView.test.tsx`.
- [ ] UI-2 -- M: a sort in the UI -> `health.smoke.test.tsx`; a state word compared outside the map -> `policy.test.ts` (`:274`, AC-U6 of health-left, extended).
- [ ] UI-3 -- M: the `proposal.rejected` trigger dropped -> `health.live.test.tsx` (its "never" list, `:157`, loses `proposal.rejected` only).
- [ ] UI-4 -- the mocks above. M: a mock off its type -> `mocks/health.test.ts`.
- [ ] UI-5 -- `pnpm lint` 0; `build`; `test` twice; no new package.
- [ ] UI-6 -- "Show all <n>" focuses the first added row; a `PR-` ID opens its inbox card, a node ID the tree. M: focus left on the button -> `HealthView.test.tsx`.

spec-writer:

- [x] SW-1 -- this spec (2026-10-16, split from `health-code.md`, its Q19).
- [ ] SW-2 -- at shipping: `health.md` "UI" (8 530 of 12 288 before 9a-1's SW-2: rows 4-5's parts, mocks, the `proposal.rejected` trigger), else `docs/canon/health-code.md` gains "UI"; `ui/README.md` (8 181) untouched; `CLAUDE.md` State.

## Out of scope

Row 6 (9b, `health-drift.md`); any Rust, route or daemon-key change (9a-1); a type generator; the home; sorting or filtering; a follow-up task from a row (06:103); re-reading on `proposal.created`, focus or an interval.

## Implementation

Not started. To record: components, deviations with reasons.
