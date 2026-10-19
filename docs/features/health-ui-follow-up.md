---
class: spec
status: shipped
scope: [ui]
ref: health-code-ui.md Follow-ups (9a-2 review 1), slice 9a-3; requirement analysis 2026-10-19 against 9350c22, U1-U7 owner pre-approved 2026-10-19; 06 s7; 07 s3; 08 s2; UI 2 iterations, review 1 accepted
shipped: 2026-10-19
---

# health-ui-follow-up (9a-3): Health re-reads on an approve and a refused apply, and 9a-2's nits

## Why

Row 4 moves when the owner decides: an approve turns `pending` into `as_assumed` or `otherwise`, a refused apply turns it back. Health re-read on `proposal.applied`, `proposal.rejected` and a gap only, so after `spec approve` the panel kept the old state until "Check again".

Adding `proposal.approved` alone makes it worse. `approved` is logged at the apply's step 7 and `applied` at step 10; a walk in flight is kept, so the `applied` re-read lands during the `approved` walk and is dropped, and Health ends on the mid-apply answer. A refusal at steps 8-9 reopens the item while Health keeps "Decided as …". Hence three things together: a re-read on `approved`, one on `apply_failed`, and one trailing read when triggers land during a walk. The other answers cost more: `cancelRefetch: true` races two walks (no signal is taken), a read per event costs N walks per burst, a timer adds a clock and a delay.

The slice also closes 9a-2's review nits (`health-code-ui.md` "Follow-ups"): `otherwise` and `rejected` shared one colour; a `measured` row without counts read "Not measured: Measured"; Health imported its text escaping from the round screen; a code comment cited a heading ui-home's spec lacks. Visibility, never a gate (ADR-0047, ADR-0012); the UI computes no core logic (ADR-0033). No ADR: the trigger list is canon text.

## Description and interactions

Delivered, `ui/**` only (no crate, route, daemon key, type, package or mock scenario): with Health open, `spec approve` moves row 4 without "Check again", a refused apply moves it back, and a burst of events ends on an answer read after the last of them; a measured row without counts draws neutral words; `--assumed-otherwise` has its own colour; the text escaping is shared code. How it works now: `docs/canon/health.md` "UI", "Known limits", "Tests". 9b's row 6 (`health-drift.md`) inherits the trailing read and the fallback; 9b is refreshed after this.

## Data

No format changes. In the canon (`health.md` "UI"): the triggers (`proposal.approved`, `.rejected`, `.apply_failed` the health read alone; `.applied` and a gap with the spec reads), the trailing read, `MEASURED_WITHOUT_COUNTS = "the answer holds no counts"` (`labels.ts`), `--assumed-otherwise: var(--sync-predated)` (#f59ac2). `ui/src/ui/text.ts` holds `shownText`, `shownSlot`, `ESCAPED`, `escaped`; `round/text.ts` keeps `ROUND_COMMAND`, the caps and `utf8Bytes`, re-exporting nothing. Comments on Health's reads cite `docs/canon/health.md` "UI"; `useCachedTasks`' cites `` `docs/features/ui-home.md` "Data" ``.

## Rules and edge cases

- WHEN one or more triggers (a gap included) land during a health walk, the UI SHALL keep it and send exactly one more health read after it settles, answered or failed; none off screen; a failed trailing read SHALL schedule nothing. The rest: `health.md` "UI".
- No `AbortSignal`, no timer, no branch on `apply_failed`'s `step` or `reason` (ADR-0033); no state word compared outside `labels.ts` (health-left's AC-U6); nothing outside `src/round/` SHALL import `round/text`. English interface (ADR-0014).

## Acceptance criteria

ui-developer and test-engineer (M: the mutation turning the named test red; `ui/src/api/health.live.test.tsx` unless named):

- [x] UI-1 -- Health on screen, its entry read answered: `proposal.approved` -> exactly one more `/health`, no other spec read. M: the trigger dropped.
- [x] UI-2 -- the same for `proposal.apply_failed` (`step` 8). M: the trigger dropped.
- [x] UI-3 -- `proposal.staged`, `.unstaged`, `.created`, `task.created`, `.approved`, `.completed`, `.refreshed`, focus, a reconnect -> no `/health`. M: the health read on every `proposal.*`.
- [x] UI-4 -- the walk held: `approved` (PR-0001) starts it, `applied` (PR-0001) and `rejected` (PR-0002) land during it -> exactly two `/health`, the second sent after the first answered, neither with a signal; the second answer (`as_assumed`) drawn. M1: no trailing read. M2: one per event (three). M3: `cancelRefetch: true`.
- [x] UI-5 -- one trigger, no walk in flight -> exactly one `/health`. M: a trailing read after every walk.
- [x] UI-6 -- Health off screen: `approved`, `apply_failed` -> no `/health`; entering reads once. M: `refetchType: "all"`.
- [x] UI-7 -- `HealthView.test.tsx`: rows 4-5 `measured` with `counts: null`, then `rows: null` -> "Not measured: the answer holds no counts", the causes, no digit, no Retry; row 3 drawn. M: `look.label` shown.
- [x] UI-8 -- `tokens.test.ts`: nine `--assumed-*`, nine distinct colours, each >= 3:1 on every surface. M: `var(--sync-conflict)` restored.
- [x] UI-9 -- `ui/text.ts` exports `shownText`, `shownSlot`, `round/text.ts` neither; U-09 green on `ui/text.test.ts` (14 code points, the slot case); nothing outside `src/round/` imports `round/text`. M: a re-export; M': `codeRows.tsx` importing `../round/text` -> `policy.test.ts`.
- [x] UI-10 -- `useCachedTasks`' comment cites `` `docs/features/ui-home.md` "Data" ``. M: "Reads" -> `doc_pointers`.
- [x] UI-11 -- `pnpm lint` 0, `build` 0, `test` twice; 17 packages; `ui/README.md` unchanged; `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers` green.

spec-writer:

- [x] SW-1 -- this spec (2026-10-19), 08 s2, 9b's note; each of `health-code-ui.md` "Follow-ups" closed by a criterion (UI-1/UI-4, UI-8, UI-7, UI-9, UI-10).
- [x] SW-2 -- 2026-10-19, at shipping: `health.md` "UI", "Known limits", "Tests" (12 272 of 12 288 B); `ui-round.md`'s `text.ts` pointers; `health-code-ui.md` "Follow-ups" and deviation 5 closed; `CLAUDE.md` State; 08 s2. Unchanged: `health-code.md`, `architecture.md`, `round-ui.md`, `ui/README.md` (8 181 B).

## Out of scope

A trigger on `proposal.created` (U3: a walk per proposal an agent raises; revisit with TE-7's timings); the check's in-flight rule; re-reading the tree or the check after a step-10 `apply_failed`; the 42 other code comments citing a heading without backticks (a sweep that also makes `doc_pointers` read them: `docs/features/citation-sweep.md`); `ui/README.md`; 9b's row 6; any crate, route, daemon key, type, package or mock scenario.

## Implementation

UI 2 iterations (the second: tests only, plus `WALKS`' dead "health" removed and a comment fixed); review 1 accepted, no blocker or major.

| Module (`ui/src/`) | What it does |
|---|---|
| `api/queries.ts` | `useLiveQueue`: `HEALTH_EVENTS` (approved, rejected, apply_failed) read health alone; `readHealthAgain`, a `healthOwed` flag per subscription: a walk in flight (`query.promise`) kept (exact, `cancelRefetch: false`), on its settle (`walk.then(after, after)`) one more, active only; an apply's and a gap's health read too; the cleanup drops the debt; `WALKS` the check alone; `useCachedTasks` re-cited |
| `health/labels.ts`, `codeRows.tsx` | `MEASURED_WITHOUT_COUNTS`; `RowNotMeasured` picks it by `look.measured`, never by `row.state` |
| `styles/tokens.css` | `--assumed-otherwise: var(--sync-predated)` |
| `ui/text.ts` (new) | `shownText`, `shownSlot`, `ESCAPED`, `escaped`, moved from `round/text.ts` |
| `inbox/{ProposalCard,StagedChoice,InterpretationSection,DecisionDialog}`, `round/{RoundItemCard,RoundScreen}` | import from `../ui/text` |
| tests | `health.live` (UI-1..UI-6, a gap during a walk, Health left while a read is owed, a failed walk owing its read, a failed trailing read scheduling nothing, the cleanup drop (`HealthBesideTail`); the "never" list and both in-flight tests rewritten, a distinct second answer each, none deleted), `HealthView` (UI-7), `tokens` (UI-8), `ui/text` (U-09 moved whole), `round/text` (U-08), `policy` (UI-9: `resolvedFrom`, both export lists) |

Deviations, accepted in review 1:

1. In flight is the query's `promise`, not `fetchStatus === "fetching"`: a paused (offline) fetch counts too, as the canon says.
2. UI-4's M2 as the flag dropped alone is equivalent (the owed `after`s run in one microtask flush, the first walk joined by the rest); applied observably, a read per event chained: red, three walks.

Mutations, iteration 1, 18 of 19 red: UI-1 `approved` dropped (3 tests), UI-2 `apply_failed` (1), UI-3 every `proposal.*` (6), UI-4 M1 (3), M2 chained (1), M3 (3), a gap past `readHealthAgain` (2), UI-5 (6), UI-6 `refetchType: "all"` on the trigger (3) and the trailing read (1), 9a-2's M3 `rejected` dropped (3), UI-7 (2), UI-8 (1), UI-9 and M' (`policy`, 1 each), UI-10 (`doc_pointers`), U-09 U+2028 dropped and CR unescaped in a slot (1 each); green M2a, the flag alone (equivalent). Iteration 2, 3 of 3 red: `walk.then(after)` without the failure arm (review 1's survivor; 2 tests), `healthOwed = false` removed from the cleanup (1), `retry: 1` on `useHealth` (2).

Gates after iteration 2: `pnpm lint` 0, `test` twice 85 files 1963/1963 (iteration 1: 1960), `build` 0, eval `ui_policy`, `anonymity`, `doc_pointers` 41/41, 17 packages, `ui/README.md` 8 181 B unchanged.
