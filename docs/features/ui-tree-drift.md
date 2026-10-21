---
class: spec
status: shipped
scope: [ui]
ref: RM-0011 slice 3 of 3 (Q-B, Q-D; owner pre-approved, review open); 07 s3 Tree row; accepted, then iteration 2 (pre-approved)
shipped: 2026-10-21
adrs: [ADR-0046, ADR-0047, ADR-0033]
---

# ui-tree-drift (RM-0011, 3 of 3): drift badges on the tree, from Health's row 6

## Why

The tree showed no drift (07 s3 Tree row's `sync`): the owner saw it on Health, one node's Bindings tab or a terminal. Health's row 6 already judges every lock entry and marker, each row naming its `node` (`docs/canon/health-drift.md` "Reading"); counting those rows per tree line needs no new read, key, route or trigger (Q-B; CLI and MCP counters: RM-0034). That read is a whole-tree walk, so the badges are opt-in. Visibility only, never a gate (ADR-0046, ADR-0047, ADR-0012). No ADR.

## Description and interactions

- **Toggle** "Show drift", after "Include archive", off by default, per project and viewer ("Toggle"); off hides the badges and the line and reads nothing, whatever the cache holds.
- **Read**: `useHealth(project, enabled)`, `enabled` = the toggle on and the tree's read answered (`tree.data` defined: a cached answer counts, `keepPreviousData` keeps it over an archive or root change), so the tree read goes first in the project's turn. Health's key and cache `["health", p]`: entering the Tree or turning the toggle on reads once more (a walk in flight kept); while on, Health's triggers reach it (`docs/canon/health.md` "UI"), none of its own.
- **Badges**: a line's own row 6 rows by class after the inbox count, never rolled up (Q-D). **Line**: `tree/DriftStatus.tsx`, `RegionBoundary name="drift"` ("Line"). An answer re-renders only the lines whose counts changed (`RowContent` memoised).

## Data

No route, wire or query key, type, package, token or trigger; `fixtures/daemon-keys.json` stays at 76. New: one storage key, `useHealth`'s optional `enabled`.

### Matching

`tree/drift.ts`, the inbox counter's pattern (`tree/matching.ts` `namesOf`):

- Fields read by name (`fieldOf`): `driftOf(report)` row 6, `driftWords` its state and causes as text. `measuredRows(drift)`: the rows when the state is text and `codeRowLook(state).measured`, `counts` an object and `rows` an array; else null.
- `treeDrift(rows, lines, previous)` → `{byRow: ReadonlyMap<string, NodeDrift>, unmatched}`: rows grouped by `node`, bytes as sent (no trim, case fold, NFC or alias), classed by `health/labels.ts` `driftClassOf` (`fix`, `check`, `cannot`; else, or not text, `DriftClassCounts.unlisted`); a non-object row skipped, a non-string `node` filed under `""`. A line sums its `namesOf` (`<ID>`, `<stem>/<ID>`, an ID-less document's path), so two holders both count; never an ancestor's or descendant's rows; none, absent from `byRow`. Unchanged counts keep `previous`'s object; every line kept, `previous.byRow` itself. `unmatched` counts the rows naming no line, never dropped.

Rows `MEC-TIDES` `fix` and `check` give its line `Fix 1`, `Check 1`; `RULE-TIDE-WINDOW` `cannot` lands on its own line, not its parent MEC-TIDES's; `<stem>/AC-07` on `AC-07` in `<stem>.md`; `""` and `RULE-BASIN-N\u0410ME` (a Cyrillic A) make `unmatched` 2.

### Badges

`health/labels.ts` `driftBadgeLooks(counts)`: Fix, Check, Cannot verify (`DRIFT_CLASS`), then `Another class` (neutral, icon `unknown`), each only above 0, label `<label> <count>`; `tree/RowContent.tsx` draws each in a `tree-drift-badge` span as `<Badge name="Drift">`: icon and `Fix 2`, accessible `Drift: Fix 2`.

### Line

`DriftStatus {project, health, failure, drift, treeShown}`: `<div class="tree-drift">`, `aria-busy` only while Health fetches; `note tree-drift-line` paragraphs with `data-drift-state`; words through `labels.ts`, causes through `oneLine` joined `; `.

| Case | `data-drift-state` | Shown |
|---|---|---|
| off; the tree's read pending or failed | | nothing, no Retry |
| no answer, reading | `measuring` | `Measuring drift` |
| no answer, failed | | `Drift could not be read`, the message, Retry |
| `off`, `not_configured`, `cannot`, an unlisted word | `not-measured` | `No drift badges: <label>: <causes>` (`Code layer off`, `Could not be measured`) |
| `measured` without counts or rows; a malformed answer | `not-measured` | `No drift badges: the answer holds no counts`; the Tree stays up |
| `measured`, no row | `none` | `No drift: every binding spec verify judges is in sync.` |
| `unmatched` > 0, lines drawn | `unmatched` | `<n> drift row(s) name(s) no line of this tree.`, `See them in Health` |
| reading again, an answer shown | | the last paragraph and badges in place, dimmed (opacity 0.75: text 5.2:1, links 4.9:1); nothing inserted |
| read again failed | | `Drift could not be read again: <message>` (`role="alert"`); badges kept |

### Toggle

A checkbox `Show drift` in an `archive-toggle` div, `sr-only` hint: `Badges on each node for its own bindings in Health's drift row: Fix, Check, Cannot verify. While on, the tree reads Health.` `recallShowDrift(project)` reads `specengine.tree.show-drift.<project>` as on only when it holds `on`; `rememberShowDrift` writes `on` or removes the key. Each access caught: no storage, a refusal or a throw reads off; a failed write keeps the choice for the view. Per browser origin: another port starts off.

### Mocks

`mocks/health.ts`'s row 6: harbor-sim 12 rows, 11 on lines, the look-alike unmatched; ledger-api `Code layer off`; `code-cannot` `Could not be measured`; `slow` `Measuring drift` 1.5 s; `empty` `No drift`; `large` 3 000 rows, 2 988 each `Check 1`.

## Acceptance criteria

Each mutation red once, then reverted. Paths under `ui/src/`.

- [x] UI-1 `tree/drift.test.ts`: counts per class and line; not on a parent; `<ID>`, `<stem>/<ID>`, a path; two holders; `""`, an unresolved ID, a look-alike unmatched; an unlisted class apart; unchanged lines and answers keep their objects; `measuredRows` null for `off`, `cannot`, an unlisted state, no counts, a bad shape. M1 (descendants' rows added) red.
- [x] UI-2 `tree/TreeView.test.tsx`: `getHealth` held, rows and `Measuring drift`, no badge; `getHealth` called after `getTree` answered; no line or Retry while the tree is pending or failed; a cached Health error and a failed tree read leave `getHealth` at 1. M2 (the tree waits for Health) red.
- [x] UI-3: `off`, `cannot`: one `not-measured` line, causes, no badge or digit; a malformed answer the line, the Tree up; a failed read the message, Retry, rows kept; a re-read inserts no paragraph. M3 (`Fix 0` unmeasured) red.
- [x] UI-4: one summary with the count, linking to `#/<p>/health`; none, none. M4 (unmatched dropped) red.
- [x] UI-5 `tree/TreeView.renders.test.tsx`: an answer changing one line's counts renders it alone. M5 (`previous` ignored) red.
- [x] UI-6: icon, `<label> <count>`, `Drift: Fix 2`; AC-U6 and no-sort (`policy.test.ts`) scan `tree/drift.ts`, `RowContent.tsx`, `DriftStatus.tsx`. M6 (a toned span) red.
- [x] UI-7 (storage cleared per test): off at entry, no `getHealth`, nor on an approval or a gap; on, one read, key `on`; off again, key gone, nothing on an approval or a gap; another project off; on on return; refused storage off, nothing thrown. M7 (`useHealth(project, true)`), M8 (uncaught `recallShowDrift`) red.
- [x] UI-8 `mocks/tree.smoke.test.tsx`: "Mocks", `large` within 20 s. Lint 0; main chunk 474.46 kB; `pnpm test` 2 438 of 2 438, twice; 17 packages; `tokens.css` unchanged.
- [x] TE: M1-M8, P1, P2, then iteration 2's I1 (the gate), I2a, I2b, I6 red, reverted; eval `ui_policy`, `anonymity`, `doc_pointers`, `identity_literals` green; with parity, `shards_repo`, `check_parity` and `build_graph`, 106 of 106; Rust unchanged since slice 1 (nextest 3 848 of 3 848 there); no Rust or fixture change.
- [x] SW: `drift-screens.md`, `health.md` "UI", `ui/README.md` "Shell", 07 s3, 08 s2, RM-0011, `CLAUDE.md`; this spec compacted.

## Out of scope

CLI, MCP and daemon tree counters (RM-0034); roll-ups; `ok`, `tested` or row 5 per line; sorting or filtering by drift; a deep link to Health's Drift part; a trigger of its own.

## Risks

- **Cost**: while on, one whole Health walk (rows 3-6) per Tree entry, toggle-on and `proposal.approved`, `.rejected`, `.apply_failed`, `.applied` or gap event, node and search reads waiting behind it (`docs/canon/drift-screens.md` "Known limits"); hence off by default, after the tree's read.
- **Unlocked flood**: before a first `spec lock accept`, every binding reads Check.

## Implementation

| Module (`ui/src/`) | What it does |
|---|---|
| `tree/drift.ts` (new) | `driftOf`, `driftWords`, `measuredRows`, `treeDrift`, `recallShowDrift`, `rememberShowDrift` |
| `tree/DriftStatus.tsx`, `RowContent.tsx` (new) | the line; a line's content, memoised, with its badges |
| `tree/TreeView.tsx`, `SpecTree.tsx` | the toggle, the gated `useHealth`, `useTreeDrift`; the `drift` prop |
| `api/queries.ts` | `useHealth(project, enabled = true)` |
| `health/labels.ts` | `DriftClass`, `driftClassOf`, `DriftClassCounts`, `driftBadgeLooks` |
| `styles/app.css` | `.tree-drift`, `.tree-drift-line` (dimmed when busy), `.tree-drift-badge` |
| `test/setup.ts` | `localStorage` cleared per test |
| tests | `tree/drift.test.ts`, `TreeView.test.tsx`, `TreeView.renders.test.tsx`, `mocks/tree.smoke.test.tsx`, `policy.test.ts` |

Accepted divergences from the draft (iteration 2): nothing drawn while the tree is pending or failed; a re-read dims the last line in place, no `Measuring drift` above it; `DriftStatus` in its own file; a malformed answer degrades to the line; a non-object row is skipped, not counted unmatched.
