---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-http, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 9b of health-code (WA-1..WA-4, WA-8..WA-12; Q1, Q2, Q6-Q9, owner pre-approved, review open); 05 s5.3; 06 s4, s7; 08 s2
adrs: [ADR-0047]
---

# health-drift: Health shows drift between spec.lock and the tree

## Why

Health's row 6 (06 s7: drift) reads "Not measured yet". `spec verify` (4a) shows drift only to whoever runs it, 4c-1 queues it only when raised (`docs/canon/proposal-drift.md`); Health is where the owner looks without asking. ADR-0047 lets it list plain verify's findings, read-only, never a verdict (`architecture-code.md#health`, `#verify`).

Slice 9 split (Q1): 9a `health-code.md` (rows 4-5, the rule table, the document's row shape), 9b this. **In scope:** the `drift` key of `spec health [--json]` and its route; verify's judge run in-process over 9a's walk; 8b's non-`verified` `tested` entries if 8b has shipped; Health's row 6, provisional types, mocks.

## Description and interactions

- **The owner** sees each drifting binding's class, state, node, file and symbol without running `spec verify`; settling it stays `spec lock accept` or 4c-1's queue, on a terminal.
- **Crates**: CLI `health.rs` calls verify's library judge (4a) in plain mode over 9a's code walk, so a call walks code once (WA-4); http tests; ui the part. No core or store change, no new crate or dependency.
- **Unchanged**: `spec verify`'s text, JSON and exits, byte for byte; rows 3-5; `spec check`, hook, CI, gate, `spec.lock`, the queue. Git: verify's own reads only (`hash-object --no-filters`, never `-w`); no `status` or `diff`.
- **Overridden lines**, fixed at shipping (SW-4): `health.md` "UI" (row 6 "Not measured yet"); `spec-verify.md` "Out of scope" (9b).

## Data

### `drift`

WA-8, Q7. A row of 9a's shape (`health-code.md` "The document"), its `rows` and `counts` from plain `spec verify --json` (`docs/canon/spec-verify-output.md` "JSON"), plus `notes`:

```json
"drift":{"state":"measured","causes":[],
 "counts":{"rows":9,"ok":7,"fix":1,"check":1,"cannot":0,"pre_existing":0,"skipped":1},
 "rows":[{"node":"LAW-STEP","relation":"implements","file":"crates/tally/src/counter.rs","kind":"item",
   "symbol":"crates/tally::counter::Tally::step","item_kind":"function_item","state":"conflict","category":null,
   "detail":null,"class":"fix","pre_existing":false,"markers":[{"line":23,"rev":1}],"node_rev":2,
   "lock":{"rev":1,"commit":"a0cea48<33 hex>","verified_at":"2026-10-10"},"compared":"ast"}],
 "notes":[]}
```

- `counts`: verify's, verbatim (plain: `pre_existing` 0). `rows`: verify's without `ok`, its key order and objects verbatim; a `cannot_verify` row (class `cannot`) is a row, not the row state `cannot`.
- `notes`: verify's, but the mode note (Health has no mode).
- Left out (WA-8): `mode`, `verdict`, `base`, `cascade`, `delta`, the `hint:` lines; the word `blocked` never appears.
- `tested` (8b, `tested-evidence.md` "Surfaces"): plain verify's `tested` entries not `verified`, verbatim, by node bytes; present only once 8b has shipped; added by whichever of 8b and 9b ships second, with `daemon-keys.json`.
- State: `measured` when verify answers without a run cause (no `spec.lock`: its `unlocked` rows); `off` with no `[code]` (code-refs' sentence less `spec: `); `cannot` on any other run cause of plain verify (`docs/canon/spec-verify.md` "Verdict and exit": a config error, `[verify]`'s too; a walk gap; no worktree, no git, a git failure; `spec.lock` invalid, unmerged, another format or recipe; HOME, the data directory), `causes` its `cannot:` lines verbatim, `counts` and `rows` null, never 0.

### Text

Rows' lines in row order 4, 5, 6, their summaries in that order, `code:`, the last line unchanged (9a "Text"). Row 6: each row as verify's text line prefixed `drift | ` (never `pre-existing `); with 8b, verify's `tested` lines likewise (`drift | tested | <state>[ <category>] | <node>`); the summary `drift: <f> fix, <c> check, <k> cannot, <o> ok; <s> skipped` (verify's, less verdict, mode, cascade):

```text
drift | fix | conflict | LAW-STEP | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::step function_item
drift | check | unlocked | LAW-BOUND | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::bound function_item
drift: 1 fix, 1 check, 0 cannot, 7 ok; 1 skipped
```

Not measured: `drift: off|cannot: <cause>` per cause, `cannot` also on stderr as `note: drift: <cause>`.

### Failures and exits

WA-2, Q6. 9a's list gains row 6: no `[code]` -> rows 4-6 `off`; a bad `[code]`, a walk gap, HOME or the data directory -> rows 4-6 `cannot`; a bad `[verify]`, no worktree or git, a git failure, an invalid or unmerged lock, another format or recipe -> row 6 `cannot` alone; the queue and `[health]` never touch row 6. Exit as 9a: never 1, whatever `[verify] mode`.

### One walk

WA-4, Q8. 9a's `code_walk::read_tree` result feeds rows 4-5 and verify's judge; 6a's code index is neither read nor refreshed. 4a's CLI judge takes the walk (`spec-verify.md` RD-10) and re-parses only the bound files whose blob changed (its Q4), `spec verify` byte-identical (RD-13).

**Note 2026-10-16** (9a's refresh, `health-code.md` Q8, Q18; ADR-0051): 9a-1 reads rows 4-5 from the code index the call refreshes; there is no 9a walk to share. Working answer: row 6 runs 7a's `sync` judge over that index (`docs/canon/architecture-code.md#sync-in-reads`), never a walk of its own. This section, "Why"' and "Description"'s "9a's walk", RD-13's `health_calls_read_tree_once` and Q8, Q9 below are restated at 9b's refresh, which decides; 9b then follows 7a.

### Route and UI

WA-9, WA-10.

- Route as 9a: any row state a 200; the body grows by `drift`.
- `ui/src/health/`: row 6 "Drift" becomes a part beside rows 4-5, 9a's four states, "None" and "over <rows> bindings, <skipped> skipped" at zero. Rows in the daemon's order, 10 then "Show all <n>"; class and state through the label map (`labels.ts`: `fix` "Fix", `check` "Check", `cannot` "Cannot verify"; an unknown word raw, so 5b's and 5c's states pass); the node linked to the tree, file and symbol plain.
- No new trigger: drift moves with code edits and lock commits that fire no event; a 4c-1 approve's `proposal.applied` already re-reads; "Check again" re-reads.
- **Exceptions to `.claude/agents/ui-developer.md`** (ADR-0033): provisional types in `ui/src/api/provisional.ts` citing `docs/features/health-drift.md` "Data"; no type generation (none exists).

## Rules and edge cases

- WHEN plain `spec verify --json` answers without a run cause, `drift.rows` SHALL equal its `rows` without `ok`, and `drift.counts` its `counts`.
- WHEN it has a run cause, `drift` SHALL be `off` or `cannot` with its lines and no count.
- WHEN `[verify] mode` is `enforce`, Health SHALL still exit 0 and carry no verdict.
- Read-only (verify writes nothing: `spec-verify.md` RD-8); determinism, genre: 9a's rules.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (CLI `tests/health.rs` unless named; RD-1..RD-6, RD-8..RD-12: `health-code.md`):

- [ ] RD-7 -- M: `ok` rows listed -> `drift_lists_no_ok_row`; an own judge -> `drift_equals_verify_rows_minus_ok`; a run cause read as zero -> `verify_run_cause_is_drift_cannot_never_zero`.
- [ ] RD-13 -- M: the shared walk changes verify -> 4a's golden `verify` tests; a second walk per call -> `health_calls_read_tree_once` (counts `read_tree` calls; verify's re-parse of changed bound files is no walk, `spec-verify.md` Q4).
- [ ] RD-14 -- M: exit 1 under `enforce` -> `drift_never_exits_1_under_enforce`; `mode`, `verdict`, `blocked` or a `hint:` in Health -> `health_carries_no_verdict_mode_or_hint`.
- [ ] RD-15 -- 9a's tests extended to `drift`: `health_code_keys_present_without_code_table`, `health_writes_no_lock_queue_row_or_event` (lock bytes too), `health_code_rows_are_byte_identical_across_roots_and_order`, http `a_row_that_cannot_be_measured_is_still_a_200` (an invalid lock). M: each key or scenario dropped -> its test.
- [ ] RD-16 -- 8b shipped first: M: a `verified` entry listed -> `drift_tested_lists_only_unverified`; else 8b's spec takes it.

ui-developer (row 6; rows 4-5: `health-code-ui.md`):

- [ ] UI-1 -- the provisional `drift` types cite "Data". M: a key missing -> `daemonKeys.test.ts`.
- [ ] UI-2 -- row 6's four states; `HealthView.test.tsx`'s "no digit in row 6" replaced by them. M: 0 drawn for `cannot` -> `HealthView.test.tsx`.
- [ ] UI-3 -- M: a sort in the UI -> `health.smoke.test.tsx`; a class or state compared outside the map -> `policy.test.ts`; `blocked` written -> `ui_policy.rs` `nothing_is_worded_as_a_hold_on_work`, unwidened.
- [ ] UI-4 -- no new trigger. M: a re-read on a stage, `.created` or `task.*` -> `health.live.test.tsx`.
- [ ] UI-5 -- mocks: harbor-sim rows of each class (a `cannot_verify` row); ledger-api `off`; `empty` measured zero; `large` (3 000 rows); `code-cannot` row 6 `cannot` too. M: a mock off its type -> `mocks/health.test.ts`.
- [ ] UI-6 -- `pnpm lint` 0; `build`; `test` twice; no new package.

test-engineer:

- [ ] TE-1 -- temp git copies of `fixtures/code-a` with slice 3's golden lock: a scenario per verify state and category, no lock, an unmerged lock, a bad `[verify]`, no git on `PATH`; `git diff --stat -- fixtures/` empty.
- [ ] TE-2 -- parity with `spec verify --json` in each TE-1 scenario, robust to 6a's discriminator and to 5b's, 5c's states once shipped.
- [ ] TE-3 -- 9a's failure matrix with row 6's causes: states, causes, stderr, exit 0; under each `[verify] mode`.
- [ ] TE-4 -- 9a's read-only probe (`spec.lock`'s bytes, `.git/index` mtime, object count) and determinism over TE-1.
- [ ] TE-5 -- `fixtures/spec-c` row 6 `off`; source scans as 9a's TE-5.
- [ ] TE-6 -- `daemon_keys.rs`: `HealthReport` + `drift`, exactly 5 new sets (the row, `counts`, a drift row, its marker, its `lock`), more with 8b's `tested`.
- [ ] TE-7 -- pilots, manual, as 9a's TE-7: verify's share of a Health entry, the whole against AC-10's 2 s (08:79).
- [ ] TE-8 -- every M above turns red.

spec-writer:

- [x] SW-1 -- this spec, with `health-code.md`'s SW-1..SW-3 (ADR-0047, the canon, the dated notes).
- [ ] SW-4 -- at shipping: row 6 into `docs/canon/health-code.md` (9a's); `health.md`; verify's canon names Health as a reader; CLI, http, `ui/` READMEs; `CLAUDE.md` State.

## Risks

Cost: a Health entry runs verify too (`hash-object` over the bound files, the walk shared, changed bound files re-parsed), in the project's turn; TE-7 measures. Vocabulary: verify's `fix` class shows, `blocked` never. Flood: before any lock accept every binding is `unlocked`; the class counts keep it readable. Stale: code edits fire no event; the row is as of its read. `daemon-keys.json` changes again if 8b ships after.

## Out of scope

`verify --changed` (base, Pre-existing) in Health; `mode`, verdicts, hints; diffs, the diff inspector, `lock accept` or 4c-1's raise from the UI (Phase 4 drift screens, 08:50); 6a's store; a watcher; other worktrees; MCP; `verified` elsewhere in the UI (Phase 4).

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 split | 9b after 4a, with 8b's `tested` if shipped | rows 4-5 wait for 4a…8b |
| Q2 ADR | ADR-0047, shared with 9a | `#control` edited undecided |
| Q6 exits | row 6 fails alone, exit 0 | a lock error hides rows 3-5 |
| Q7 row 6 | plain verify's rows but `ok`, counts, notes | the whole document: thousands of `ok`, `blocked`; a second judge: two answers |
| Q8, Q9 | one walk, 6a unread; read-only, git via verify | a refresh per read; row 6 impossible |

WA-n: the analysis' working answers (2026-10-10), [A] assumed: WA-1, -2, -3 (git only through verify), -4, -8, -10; [P] settled: WA-9, -11, -12.

## Dependencies and ordering

- **Needs**: 9a-1 and 9a-2 (row shape, parts; the walk: "One walk"' note); slice 3 `spec-lock` (lock reader); 4a `spec-verify` (the judge, run causes). Soft: 8b (`tested`).
- **Row 5** aligned with 6a-3's X3 at 9a's refresh (2026-10-16): a `cannot_verify` marker binds nothing (`health-code.md` "What binds").
- **Order**: 1 -> 3 -> 4a -> … -> 9b, after 9a; independent of 4b, 4c, 6, 7. 5b, 5c reach row 6 through verify (label map: unknown words raw). **Later**: Phase 4's drift screens.

## Implementation

Not started. To record: modules, TE-7's figures, deviations with reasons.
