---
class: spec
status: shipped
scope: [ui]
ref: Phase 3 plan 2026-10-10, slice 9a-2, split from health-code 2026-10-16 (its Q19; WA-9, WA-10, Q11 carried; decided under the owner's pre-approval); 06 s7; 07 s3; UI 1 iteration, review 1 accepted
shipped: 2026-10-19
adrs: [ADR-0047]
---

# health-code-ui (9a-2): Health draws rows 4-5

## Why

9a-1 (`health-code.md`) makes `spec health` and its route answer rows 4-5, yet Health still drew them "Not measured yet". The owner looks at Health, not at a terminal: code built on a working answer the decision then changed, and accepted nodes no code binds, have to be on the panel, beside row 3, read the same way. Visibility, never a gate (ADR-0047).

## Description and interactions

Delivered: Health's "What is left" draws row 4 "Code assuming a decision" and row 5 "Unbound nodes" beside row 3, from the same read (`getHealth`, key `["health", p]`); "Not measured yet" keeps row 6 alone until 9b. `ui/**` only: no Rust, no daemon key, no package. How it works now: `docs/canon/health.md` "UI", "Known limits"; the body `docs/canon/health-code.md` "Document".

## Data

Each part keeps four states apart (9a's Q11): being measured; not measured (`off`, `not_configured`); could not be measured (`cannot`, or the whole read failed); measured, alone with a digit, zero as "None" and what was measured over. Values, lines, links, re-read triggers and mocks: `health.md` "UI". Types: 9a-1's provisional ones, unchanged, citing `docs/features/health-code.md` "Data" (ADR-0033's exception; no generation exists).

## Rules and edge cases

- WHEN one row cannot be measured, the other parts SHALL draw as they are; a row not measured SHALL hold no digit.
- The UI SHALL sort nothing and compare no kind, class, status, state or reason outside the label map (health-left's AC-U6, extended to the two parts, not loosened).
- An unknown state word SHALL show raw, never fail the part (9b and later slices add words).
- English interface (ADR-0014).

## Acceptance criteria

ui-developer (M: mutation -> the test turning red; 9a's UI-2..UI-6, renumbered):

- [x] UI-1 -- four states apart per row; `HealthView.test.tsx`'s "holds no digit" test narrowed to row 6. M: 0 drawn for `cannot` -> `HealthView.test.tsx`.
- [x] UI-2 -- M: a sort in the UI -> `health.smoke.test.tsx`; a state word compared outside the map -> `policy.test.ts` (AC-U6 of health-left, extended).
- [x] UI-3 -- M: the `proposal.rejected` trigger dropped -> `health.live.test.tsx` (its "never" list loses `proposal.rejected` only).
- [x] UI-4 -- the mocks of `health.md` "UI". M: a mock off its type -> `mocks/health.test.ts`.
- [x] UI-5 -- `pnpm lint` 0; `build`; `test` twice; no new package.
- [x] UI-6 -- "Show all <n>" focuses the first added row; a `PR-` ID opens its inbox card, a node ID the tree. M: focus left on the button -> `HealthView.test.tsx`.

spec-writer:

- [x] SW-1 -- this spec (2026-10-16, split from `health-code.md`, its Q19).
- [x] SW-2 -- 2026-10-19, at shipping: `health.md` "UI", "Known limits", "Tests" (11 193 of 12 288 B); `ui/README.md` (8 181) untouched; `CLAUDE.md` State; 08 s2.

## Out of scope

Row 6 (9b, `health-drift.md`); any Rust, route or daemon-key change (9a-1); a type generator; the home; sorting or filtering; a follow-up task from a row (06:103); re-reading on `proposal.created`, focus or an interval.

## Follow-ups

Closed 2026-10-19, all five, by 9a-3 (`health-ui-follow-up.md`).

- Add `proposal.approved` to Health's re-read triggers (a spec change, no ADR): it moves row 4 from `pending` to `as_assumed`/`otherwise`, which stays stale until "Check again" (`health.md` "Known limits").
- UI nits from review 1, optional: give `otherwise` its own colour alias (it and `rejected` both resolve to #ff7d88; label and icon differ); a `measured` row with null `counts`/`rows` (outside the contract) falls back to the raw-word branch, not "Not measured: Measured"; move `shownText` from `round/text.ts` to shared `ui/` code (now a health-to-round dependency); re-cite the comment at `ui/src/api/queries.ts:423`, which names a "Reads" heading ui-home's spec lacks, to its "Data".

## Implementation

UI 1 iteration; review 1 accepted, no blocker or major.

| Module (`ui/src/`) | What it does |
|---|---|
| `health/codeRows.tsx` | `CodePart` (frame, heading, boundary), `RowNotMeasured` (not measured, cannot), `CodeRowList` ("Show all <n>", focus to the first added line), `oneLine` (one line, controls `\u{..}` via `shownText`), `counted` |
| `health/AssumesPart.tsx` | row 4: the value, `AssumesLine` (badge, ID link, place, relation, the CLI's tail) |
| `health/UnboundPart.tsx` | row 5: the value, `UnboundLine` and its markers |
| `health/LeftRegion.tsx` | the parts after row 3; "Not measured yet" row 6 alone; `healthBusy` shared by rows 3-5 |
| `health/labels.ts` | `codeRowLook`, `assumedLook`, `unboundReasonLook`, `idStateLabel`, `bindingLabel`; an unlisted word raw, neutral |
| `api/queries.ts` | `useLiveQueue`: `proposal.rejected` invalidates `["health", p]` alone, `cancelRefetch: false` |
| `styles/tokens.css`, `app.css` | 14 aliased roles `--assumed-*`, `--unbound-*`, their `.tone-*`; unknown dashed, cannot-verify dotted; `.health-code-*` wraps anywhere |
| `mocks/health.ts`, `scenario.ts` | rows 4-5 per scenario; `code-cannot`, `not-configured` |
| tests | `test/builders.ts`; `HealthView`, `labels`, `mocks/health`, `health.smoke`, `MockClient`, `health.live`, `policy` (AC-U6 adds `reason`, `id_state`, `binding`, `relation`), `tokens` |

Deviations, accepted in review 1:

1. Row 4 `unknown` (no item of that ID) is plain text: there is no inbox card to open.
2. Causes and fields are one-lined and escaped as the CLI's text, not verbatim; U+2028/U+2029 too, so not byte-identical to the CLI. The read-failure message stays verbatim.
3. A failed read shows in rows 4-5 without `role=alert`: row 3 speaks it, one alert per failure.
4. Row 5 reads "Rules failed: 1, 2": a quoted "rule" is barred by `ui_policy` AC-19.
5. Row 4 stale on `proposal.approved`/`created`: a Known limit, no ADR (the owner); "Follow-ups". Closed 2026-10-19 for `approved` by 9a-3; `created` stays a Known limit.
6. `ui/README.md`'s scenario list lacks the two new ones (8 181 of 8 192 B); `health.md` "UI" names them.

Mutations, 10/10 red: M1 0 drawn for `cannot` (HealthView); M1b `cannot` labelled measured (labels; HealthView renders it equivalently); M2a row 5 sorted by ID, M2a' row 4 by path (health.smoke); M2b a state compared in `AssumesPart` (policy); M3 the `proposal.rejected` trigger off (health.live); M4 a mock key renamed, M4' an extra key (mocks/health, 7 scenarios); M6 focus left on "Show all" (HealthView); M6' an item's ID linked to the tree (HealthView, health.smoke). Gates: `pnpm lint` 0, `test` twice 84 files, 1949/1949, `build` 0.
