---
class: spec
status: draft
scope: [ui]
ref: RM-0011 slice 2 of 3, split 2026-10-21 (Q-K, Q-L; owner pre-approved, review open); 07 s3 Queue card; docs/canon/proposal-drift.md "UI"; a compact draft, refreshed before it starts
adrs: [ADR-0045, ADR-0047, ADR-0033]
---

# ui-drift-evidence (RM-0011, 2 of 3): drift rows typed on the card and in Health

## Why

A drift item's evidence reaches the Inbox card as raw compact JSON lines (`ui/src/inbox/DriftItem.tsx` `DriftSection`, `drift.ts` `driftRowText`). The owner approves or rejects without readable rows. Health's row 6 left six of verify's fields to RM-0011 (`docs/features/health-drift-ui.md` "Description and interactions"). Both sets of rows come from verify, so one row component draws them in both places. This slice adds no read, no cost and no ADR.

## Description and interactions

- **Card**: "Drift evidence" draws each stored row as a typed line: Health's fields plus markers, node revision, lock, `compared`, levels and pre-existing. A row that does not fit the shape stays its compact JSON, and the count is unchanged. Number, commit, hash and `more` stay as now.
- **Commands**: "Decided on a terminal" always lists `spec approve PR --editorial` (Q-K). Rows that are `spec_ahead` `same_revision` are marked "Editorial", because accept judges `HEAD` now (`docs/canon/proposal-drift.md` "Approve").
- **Health**: row 6 draws through the same component, so the six fields show there too (Q-L).
- **Words**: through `health/labels.ts`, with no comparison outside it.

## Data

- **`StoredDriftRow`** (provisional, citing `docs/canon/proposal-drift-item.md` "The item"): `DriftRow`'s 17 keys, with `levels?` and `anchor?` optional. Older items, and the mock `PR-0010`, store 15. Later keys are ignored. `DriftEvidence` stays at 3 keys. Its `rows` is typed as the analysis proposes (`StoredDriftRow[]`), but a guard in `drift.ts` decides the shape at drawing time.
- **Fields**:
  - markers: `line` `@N`, or "unversioned" (rev null); `[]` reads "No marker".
  - node rev: "Node rev N" or "No revision now".
  - lock: "Locked rev N, <commit>, <date>" or "Not locked".
  - `compared`: raw.
  - levels: lock, markers, changed (null: absent).
  - pre-existing: "Pre-existing" when true.
- **Mocks**: `ledger-api` gains a second drift item whose 17-key rows include a moved anchor and changed levels. `PR-0010` keeps 15 keys.

## Rules and edge cases

- A row missing a required key, or with a wrongly typed value, SHALL show raw and still count (M3).
- The UI SHALL NOT compare a state, class or category outside `labels.ts`. The "Editorial" mark comes from a `labels.ts` predicate.
- `--editorial` SHALL always be listed (M4).

## Acceptance criteria

- [ ] UI-1: `StoredDriftRow`'s `levels` and `anchor` are optional; `daemonKeys.test.ts` keeps `DriftEvidence` at 3. M1 (`levels` made required, so `mocks/drift.smoke.test.tsx` fails on `PR-0010`) turns it red.
- [ ] UI-2: `inbox/drift.test.tsx` checks every field above. M2 (`lock.commit` dropped) turns it red.
- [ ] UI-3: a malformed row shows raw, and the count is unchanged. M3 (the row dropped) turns it red.
- [ ] UI-4: `--editorial` is always listed, and `same_revision` rows are marked. M4 (the command listed only when such a row exists) turns it red.
- [ ] UI-5: `HealthView.test.tsx` "row 6: details". M5 (Health keeping the short line) turns it red.
- [ ] UI-6: `policy.test.ts` covers `src/inbox/drift*`. M6 (a state comparison outside `labels.ts`) turns it red.
- [ ] UI-7: mocks; gates as slice 1's UI-8.
- [ ] TE: the mutations run, and `crates/specengine-http/tests/drift.rs` stays green.
- [ ] SW:
  - `proposal-drift.md` "UI" (567 B line, file 9 022) and `proposal-review.md` "Review document" both say "rows untyped until Phase 4": drop it.
  - `health-drift.md` "UI" (127 B free).
  - `drift-screens.md`.

## Out of scope

- A new read or MCP typing (`specengine-mcp` `mirror.rs` stays `JsonObject`; its comment is an optional edit for the rust-developer).
- Sorting, filters, writes.
- Tree badges (slice 3).

## Files by role

- **ui-developer**: `inbox/DriftItem.tsx`, `inbox/drift.ts`, `health/DriftPart.tsx`, `health/labels.ts`, `api/provisional.ts`, `mocks/ledger-api/fixtures.ts`, `styles/app.css`, and the tests above. The comments saying "untyped until Phase 4" in `provisional.ts`, `drift.ts`, `DriftItem.tsx`, `app.css` and `daemonKeys.test.ts` change too.
- **Shared with slice 1**: `provisional.ts` and `DriftPart.tsx`. Merge, never overwrite.

## Open before it starts

- Where the shared row component lives (`health/` or a new `drift/`).
- The guard's exact rules.
- Whether `rows` stays `unknown[]` with the guard returning `StoredDriftRow | null`, rather than the analysis' typed array.

## Implementation

Filled in after implementation.
