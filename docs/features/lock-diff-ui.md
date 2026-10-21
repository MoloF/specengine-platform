---
class: spec
status: draft
scope: [ui]
ref: RM-0033 slice 2 of 2 (the RM-0011 breakdown's slice 5; owner pre-approved, review open); 07 s3 Node row; after `docs/features/lock-diff.md`
adrs: [ADR-0053, ADR-0046, ADR-0033]
---

# lock-diff-ui (RM-0033, 2 of 2): the lock diff in the Bindings tab

## Why

Slice 1 (`docs/features/lock-diff.md`) gives the hunks; the owner reads drift in the node's Bindings tab (`docs/canon/drift-screens.md` "Bindings tab"). This draws them there, on demand, beside each drifted row, so a drift item or an accept is decided without a terminal round trip. Slice 1 shipped 2026-10-21; the read's contract as built: `docs/canon/spec-lock-diff.md`.

## Description and interactions

- **Read.** Under the Bindings panel's holders, a closed disclosure "Changes since the lock". Its first opening sends one `GET /api/projects/:p/lock-diff?ref=<REF>` (`client.getLockDiff`, the tab's REF as the bindings read takes it); never when the tab opens, never again on closing and reopening. Key `["lock-diff", p, ref]`, added to `READS_OF_THE_SPEC` (`api/queries.ts`), so `proposal.applied` and an event gap re-read an opened panel; the tab's "Read again" re-reads it too when it is open. Cost: a `verify --node` plus a git process or two per side, in the project's turn (`docs/canon/spec-lock-diff.md` "Known limits").
- **Panel** (`tree/LockDiffPanel.tsx`): `reason` verbatim; the notes through `oneLine`, in the daemon's order; per holder as the Bindings panel places holders; per row the `DriftLine` head (`health/DriftPart.tsx`), then the spec and code sides side by side (stacked below 48em), each a `ui/DiffView` labelled "Node text since the lock: <path> at <commit>" and "Code since the lock: <file>, blob <blob>", hunks exactly as sent.
- **States.** A side `null`: "Not moved since the lock" (muted); `diff` `""`: "No change since the lock"; `diff` `null`: "No diff: the daemon's notes above say why", never "No change"; `rows` `null`: "Not judged: the daemon's note above says why", no count; `rows` `[]`: "None: no drifted binding of this node has a lock entry."; loading, failed and failed-again as the Bindings panel's states.
- **Commands.** Beside each row, `nodeCommands(row.node)` (`tree/commands.ts`, `NODE_ID`): `spec propose drift <ID>`, `spec lock accept <ID>`, `spec lock diff <ID>`; none for an ID failing `NODE_ID`, with the refusal text.

## Data

Slice 1's JSON and types, unchanged: `LockDiffView`, `LockDiffNode`, `LockDiffRow`, `LockDiffSpecSide`, `LockDiffCodeSide`; mocks `mocks/lockDiff.ts`. Words through `health/labels.ts` and `tree/labels.ts`; no state compared elsewhere (`policy.test.ts` AC-U6).

## Rules and edge cases

- The panel SHALL send GET requests only, and only its one read per opening state.
- A `null` diff SHALL never be drawn as "No change"; a `null` `rows` never as a count or "None".
- Hunks SHALL be drawn as sent: no re-diffing, sorting, trimming or re-wrapping of lines (`architecture.md#ui`).
- Every daemon word SHALL show as text, wrapped, uncut.

## Acceptance criteria

ui-developer; mutations M1-M7, each red once, then reverted.

- [ ] UI-1: `tree/NodePane.test.tsx`: opening the tab sends no `lock-diff` request; the first opening of "Changes since the lock" sends exactly one; reopening none. M1 (the read on tab open) red.
- [ ] UI-2: both sides side by side, stacked below 48em; the hunks' text equals `diff` byte for byte. M2 (hunks altered before drawing) red.
- [ ] UI-3: each state of "States" draws its own text. M3 (a `null` diff drawn as "No change since the lock") red.
- [ ] UI-4: `api/live.test.tsx`: `proposal.applied` re-reads an opened panel once; `proposal.created` nothing. M4 (the key left outside `READS_OF_THE_SPEC`) red.
- [ ] UI-5: `tree/commands.test.ts`: the row commands for `LAW-BOUND`, none for a space, `;`, `$(`, a backtick or a Cyrillic look-alike. M5 (an unvalidated ID) red.
- [ ] UI-6: each `DiffView` is a labelled figure; screen readers hear "Added:" and "Removed:"; `policy.test.ts` scans the panel. M6 (an unlabelled figure) red.
- [ ] UI-7: mocks (conflict, `code_ahead`, a gone commit, ledger-api's `null`); `pnpm lint` 0, build under Vite's 500 kB warning, `pnpm test` twice; 17 packages, tokens unchanged. M7 (`row.state === "conflict"` in the panel) red.

test-engineer: M1-M7 red then reverted; eval `ui_policy`, `anonymity`, `doc_pointers` green.

spec-writer, at shipping: the panel's canon in a new `docs/canon/lock-diff-panel.md` (`spec-lock-diff.md` holds 12 163 of 12 288), one pointer to it there and in `drift-screens.md` "Bindings tab" (12 225: a cut if over 63 B); 07 s3's Node row; RM-0033 shipped; 08 s2; `CLAUDE.md` state.

## Out of scope

Any write or stage; a deep link to the panel; a word diff or editable merge (`@codemirror/merge`); diffs against `HEAD`; a new SSE trigger; the panel elsewhere than the Bindings tab.

## Implementation

Filled in after implementation.
