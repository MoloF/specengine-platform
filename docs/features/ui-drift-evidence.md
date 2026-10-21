---
class: spec
status: shipped
scope: [ui]
ref: RM-0011 slice 2 of 3, split 2026-10-21 (Q-K, Q-L; owner pre-approved, review open); 07 s3 Queue card; review accepted, then a micro round (owner pre-approved)
shipped: 2026-10-21
adrs: [ADR-0045, ADR-0047, ADR-0033]
---

# ui-drift-evidence (RM-0011, 2 of 3): drift rows typed on the card and in Health

## Why

A drift item's evidence reached the Inbox card as raw compact JSON lines, so the owner decided without readable rows. Health's row 6 left six of verify's fields to RM-0011 (`docs/features/health-drift-ui.md`), and slice 1 left `BindingLine` drawing fields raw where row 6 used `oneLine`. Both row sets are `spec verify --json` rows, so one component now draws both. No read, route, wire key, trigger, token, cost or ADR is added; visibility, never a gate (ADR-0047, ADR-0012); provisional types (ADR-0033); the item is ADR-0045's.

## Description and interactions

As built: `docs/canon/drift-screens.md` "Drift rows" (the line, the guard, the card, the raw row, commands, mocks); Health's row 6, `docs/canon/health-drift.md` "UI"; the Bindings and Bundle tabs' words through `oneLine`, `drift-screens.md` "Bindings tab". `--editorial` is always listed (Q-K); only the card marks a row, Health never (Q-L).

## Data

- `api/provisional.ts`: `StoredDriftRow`, `DriftRow`'s 17 keys in order, the 15 core ones required, `levels?: DriftLevels | null` and `anchor?: BindingAnchor | null` optional (older items lack them); `DriftEvidence {hash, rows: StoredDriftRow[], more}`. Not a daemon key set: `fixtures/daemon-keys.json` stays at 76, `StoredDriftRow` stays out of `daemonKeys.test.ts`'s `KEYS`.
- The guard, `inbox/drift.ts` `storedRowOf(row: unknown): StoredDriftRow | null`: a JSON object (not an array, not null) whose values fit, keys checked by name from a list:

| Key | Fits |
|---|---|
| `node`, `relation`, `file`, `state` | a string |
| `kind`, `symbol`, `item_kind`, `category`, `detail`, `class`, `compared` | a string or null |
| `pre_existing` | a boolean |
| `markers` | an array of `{line, rev}`, `line` a count, `rev` a count or null |
| `node_rev` | a count or null |
| `lock` | null, or `{rev, commit, verified_at}` (a count, two strings) |
| `levels` | absent, null, or `{lock, markers, changed}` (string arrays; `changed` may be null) |
| `anchor` | absent, null, or `{state, from, candidates}`: `state` a string, `from` null or a place, `candidates` null or an array of places |

  A place: `{file, kind, symbol}` strings, `item_kind` a string or null. A count: a non-negative safe integer. Other keys ignored; anything else `null`.
- Mock PR-0011 (`DRIFT_ITEM_2`, `DRIFT_NODE_2` `POL-REFUND-WINDOW`), every scenario but `empty`, as PR-0010 but severity `high`, summary `spec_ahead same_revision, conflict`, `more` 0, hash `b3:` + `fakeHex` of its rows. Two 17-key rows, `implements`, `pre_existing`, `node_rev` 2, lock `{rev: 2, commit: LOCKED_COMMIT, verified_at: "2026-10-09"}`: `src/refunds/clock.rs` `spec_ahead same_revision` `check`, compared `blob`, levels `changed` null, anchor `exact`; `src/refunds/window.rs` `conflict` `fix`, compared `ast`, levels `changed` `body, deps`, anchor `moved` from `src/refunds/policy.rs`.

## Rules and edge cases

- WHEN a stored row fails `storedRowOf`, the card SHALL show it raw in its place and count it (M3); no row makes the card throw.
- WHEN `levels` or `anchor` is absent, the row SHALL draw as with null (M1).
- `--editorial` SHALL always be listed (M4); a `conflict` at R = `rev_L` is not marked.
- No sort, filter or cut (M8); the UI SHALL NOT compare a state, class, category or sync outside `labels.ts` (M6).
- Every word is text, wrapped, uncut, through `oneLine` (M5, M7); nothing written or staged, no request but the existing GETs.

## Acceptance criteria

ui-developer; each mutation turned its test red once and was reverted.

- [x] UI-1: `daemonKeys.test.ts`: `DriftEvidence` 3 keys, `rows: StoredDriftRow[]`; `declaredKeys("StoredDriftRow")` equals `served("DriftRow")`, `?` on `levels` and `anchor` only; the fixture 76 sets. `mocks/drift.smoke.test.tsx`: PR-0010's 15-key rows draw 2 `DriftLine`s, no `[data-raw]`. M1 (`storedRowOf` requires `levels`) red.
- [x] UI-2: `inbox/drift.test.tsx`, a 15-key, a 17-key and an 18-key row, all typed; the texts "No marker", "Markers line 41 @5, line 88 unversioned", "No node revision", "Node rev 3", "Not locked", "Locked rev 2, <40 hex>, 2026-10-09", "Not compared", "Compared ast", "Levels", "lock sig, body", "changed body", "Pre-existing", "Moved from …"; a `file` holding `\n` shows a space. M2 (`lock.commit` dropped) red.
- [x] UI-3: `null`, `"row"`, `[]`, a row without `state`, `node: 7`, `levels: "sig"`, `markers: [{line: -1, rev: null}]` each draw "Shown as sent: …" with their compact JSON, in stored order between typed rows, all counted. M3 (refused rows filtered out) red.
- [x] UI-4: the three commands with no `same_revision` row, `spec approve PR-0012 --editorial` with its exact meaning; "Editorial" on `spec_ahead same_revision` only, not `marker_behind` nor `conflict`; `isEditorialRow` true for that pair only. M4 (the editorial command only when a row is marked) red.
- [x] UI-5: `HealthView.test.tsx` "row 6: details" over stub rows; the harbor-sim checks (MEC-MOORING "Marker line 30 @2", "Node rev 2", "Locked rev 2, 4f1c9a2e…, 2026-10-02", "Compared ast", "lock sig, body", "changed body"; MEC-CARGO-CRANES "Not locked", "Not compared", no "Levels"; no "Pre-existing", no "Editorial") in `mocks/health.smoke.test.tsx`. `tree/NodePane.test.tsx`: a binding row's file, signature and detail holding `\n` show a space. M5 (`DriftPart` draws its own short line), M7 (`BindingLine` draws `row.signature` raw) red.
- [x] UI-6: `policy.test.ts` AC-U6 and the no-sort test list `/src/inbox/drift.ts` and `/src/inbox/DriftItem.tsx`; AC-U6's field list gains `category`, with detector cases `row.category === "same_revision"` and `row.category !== "marker_behind" && …`. M6 (`row.state === "spec_ahead"` in `inbox/drift.ts`) red.
- [x] UI-7: `drift.smoke.test.tsx`: two drift items, PR-0011's rows 17 keys in `DriftRow`'s order, "Editorial" on its first row only, the second "Moved from src/refunds/policy.rs | item refunds::policy::is_refundable function_item" and "changed body, deps"; the round sheet names PR-0010 and PR-0011. 17 packages, no new token.
- [x] TE: M1-M8 and the micro round's four red, then reverted; http `drift.rs`, `daemon_keys.rs` green; `fixtures/daemon-keys.json` unchanged; eval `ui_policy`, `anonymity`, `doc_pointers` green.
- [x] SW: `drift-screens.md` "Drift rows", lead, "Row", limits, tests; `proposal-drift.md` "UI"; `proposal-review.md` "Pinned"; `health-drift.md` "UI"; RM-0011; 07 s3; 08 s2; `CLAUDE.md`; this spec compacted.
- [ ] MC, the owner's (the raise judges `HEAD` only): in a scratch copy, on a scratch branch with `specengine.toml` clean, commit an edit to a locked, marker-bound node's text without raising its revision; run `spec propose drift <REF>`, then `spec serve`: the item's card shows the row `spec_ahead same_revision` marked "Editorial". Its commands only copy, nothing runs from the page; `spec approve` writes a lock commit, so only in the copy. Without a daemon, the mock `?scenario=normal`, ledger-api PR-0011, shows the same card.

## Out of scope

A new read, route, trigger, deep link, write or stage; sorting, filters; MCP typing (`mirror.rs` stays `JsonObject`); marking a `conflict` at R = `rev_L`; tree badges (slice 3); the lock diff (RM-0033).

## Implementation

| Module (`ui/src/`) | What it does |
|---|---|
| `api/provisional.ts` | `StoredDriftRow`; `DriftEvidence.rows` typed |
| `inbox/drift.ts` | `storedRowOf`; `driftCommands`, `--editorial` always with its meaning |
| `inbox/DriftItem.tsx`, `ProposalCard.tsx` | `DriftSection {project, drift, commit}`: typed rows as `DriftLine`s, the raw row |
| `health/DriftPart.tsx` | `DriftLine` exported: facts, levels, `markEditorial` |
| `health/labels.ts` | `isEditorialRow({state, category})` |
| `tree/BindingLine.tsx` | every daemon word through `oneLine`; the one `bindingOf` |
| `health/oneLine.ts` (new), `codeRows.tsx`, `tree/labels.ts` | `oneLine` moved unchanged into a leaf over `ui/text` (no cycle with the label maps), re-exported by `codeRows.tsx`; `syncLook` and `rawClass` write an unlisted word through it |
| `mocks/ledger-api/fixtures.ts` | `DRIFT_ROWS` typed; PR-0011 |
| `styles/app.css` | `.drift-raw`, the facts lines; existing tokens |
| tests | `inbox/drift.test.tsx`, `health/HealthView.test.tsx`, `mocks/health.smoke.test.tsx`, `mocks/drift.smoke.test.tsx`, `tree/NodePane.test.tsx`, `api/daemonKeys.test.ts`, `policy.test.ts` |

Differences from the draft:

1. UI-5's harbor-sim checks live in `mocks/health.smoke.test.tsx`: policy AC-05 forbids importing mocks outside `src/mocks/`; `HealthView.test.tsx` uses stub rows.
2. M8 added (`toSorted` in `DriftItem.tsx`, the no-sort test red).
3. Micro round, owner pre-approved, final: AC-U6's field list gains `category`; an unlisted state or class badge word goes through `oneLine`, which moved to `health/oneLine.ts`; the raw row's JSON through `shownText`, so bidi marks, DEL and C1 show as `\u{..}`. Each of its four changes was proven red, then reverted by hand and the files checked.

Gates: before the micro round eval 46 passed, http `daemon_keys`, `drift` green, the fixture 76; after it `pnpm lint` 0, `pnpm test` 2 403 of 2 403 twice, main chunk 469.49 kB (was 465.44), eval `ui_policy` and `doc_pointers` 38/38. Workspace: no Rust change since slice 1 (nextest 3 848 of 3 848 there); fmt, ui lint, build and test 2 403 of 2 403, export, check, parity and the docs and eval policy tests (101 of 101) clean. `ui/README.md` untouched (8 190 of 8 192 B).
