---
class: spec
status: shipped
scope: [ui, crates/specengine-eval]
ref: ui-health analysis 2026-10-06, every recommendation accepted (working answers for the owner's review); 08 s2 Phase 4; Round split off as `ui-round`; iterations 1-2, review accepted
shipped: 2026-10-07
adrs: []
---

# UI health: the check on mocks

## Why

Whether the corpus passes `spec check`, and what is left to decide, the owner learned only on a terminal. Health (07 s3) renders `spec check --json` and the Inbox read, read-only, on the mock (ADR-0033); verdicts, findings, debt, budgets and W are core's, the UI computes none (`architecture.md#ui`; ADR-0012, ADR-0031). What has no producer yet (drift, task W, rows 3-6 of 06 s7) reads "Not measured yet", never 0. No ADR, no package. Round waits for `ui-round` (no `spec round --json` producer; choosing questions is core logic; the paste is a write, ADR-0035).

Working answers, 2026-10-06 (orchestrator, for the owner's review): the endpoint is `check` (`health` stays for Phase 3's composite); `blocked` spelled only on the `KnownCheckVerdict` line, in mocks and tests, labelled "Fails the check"; no W target (`docs/canon/spec-check.md` "Findings, debt, verdict": "not a check, no cap").

## Description and interactions

Route `#/<p>/health`: one `h1`, four `h2` regions, each with its own boundary, skeleton + `aria-busy`, empty text, error (verbatim + Retry of that read); regions sharing the check read show its failure, only Check announces it.

1. **Check**: verdict label + icon: `clean` "Clean", `observed` "Passes with findings", `blocked` "Fails the check", `cannot-check` "Could not check" (`cannot-verify` token); any other a neutral raw badge. `mode` raw; only the counts present; "Worst W: <n> B", on `cannot-check` "Not measured" and each cause `<path>: <message>` (`.` for no path, as the CLI); "Task W: Not measured yet (the `bundles` log)". No W target, bar or colour.
2. **What is left** (06 s7): "Proposals in the queue" from `getInbox` in `queueOrder` (`ui/src/inbox/order.ts`): counts by severity, the first five linked `sectionHash(p, "inbox", id)`, times as stored; rows 3-6 "Not measured yet: <source>"; row 7 is Budgets.
3. **Findings**: by `code` (`role="group"`), errors first, a count each, over 10 rows collapsed (rows not rendered). A row: severity; `path:line`, an `.md` path linked `sectionHash(p, "tree", path)`, `""` "No file"; `subject`; `message` verbatim; "In debt until <expires>: <reason>" or "Debt expired <expires>: <reason>"; `fix.text` as text, no apply. Severity and code chips and a text filter make no call.
4. **Debt and budgets**: findings with `debt`, expired first, then by `expires`; each stale entry "`.spec-debt.toml` line <line>: <code> on <path> matches nothing"; Budgets: exactly the `budget` findings, slot and `message` verbatim.

"Check again" refetches once, announced politely. A clean report: "The check is clean: <n> documents, no findings, no debt." + an Inbox link. Keys as Tasks (`docs/features/ui-tasks.md` "Implementation"); Enter on an `.md` row opens it in the tree. The nav's Questions: "Not built yet: arrives in slice `ui-round`".

## Data

**Client** (`ui/src/api/client.ts`):

```ts
/** MISSING ENDPOINT GET /api/projects/:p/check (= spec check --json; rust-developer, ui-live; 07 section 3's health is a later composite) */
getCheck(project: string): Promise<CheckReport>;
```

`HttpClient.getCheck`: `ClientError {status: 501, notServed: true}`, no `fetch`, its message naming this spec's "Open". Key `["check", p]` (`useCheck`): read on entering Health, never on focus, reconnect or interval, not in `READS_AFTER_DECISION`.

**Provisional types** (`ui/src/api/provisional.ts`), citing `docs/canon/spec-check.md` "Findings, debt, verdict" (core `check/report.rs`); modes its "Configuration":

```ts
CheckReport { mode: CheckMode; verdict: CheckVerdict; counts: CheckCounts; findings: CheckFinding[]; stale: DebtEntry[];
  new_debt?: NewDebtEntry[]; cannot_check: CheckCause[] }                      // 7 keys
CheckCounts { documents; errors; warnings; debt; expired; stale; introduced?; new_debt?; worst_w_bytes }  // numbers, 9
CheckFinding = Finding & { fix?: { span: { start; end }; text }; debt?: { reason; expires; expired: boolean }; introduced? }  // 9
DebtEntry { code; path; subject; reason; expires; line: number }  NewDebtEntry = DebtEntry & { head_expires? }
CheckCause { path; message }
CheckVerdict, CheckMode, FindingSeverity = Known... | Unlisted  // KnownCheckVerdict + KNOWN_CHECK_VERDICTS on one line
// `?`: omitted when absent, never null (`docs/canon/spec-check-cli.md` "spec check", W-1)
```

**Mock** (`ui/src/mocks/check.ts`, no clock; findings in core's order (path, line, code, subject, message), counts by core's rule): `normal`: `harbor-sim` `observed` (below), `ledger-api` `blocked` under `enforce`; `empty`: `clean`; `cannot-check`: two causes, `worst_w_bytes` 0; `large`: 3 000 findings over 12 codes; `error`, `slow`.

```json
{"mode":"observe","verdict":"observed","counts":{"documents":41,"errors":2,"warnings":0,"debt":1,"expired":0,"stale":1,"worst_w_bytes":61234},
"findings":[{"code":"budget","severity":"error","path":"docs/canon/tides.md","line":1,"subject":"canon","message":"12950 bytes, over the canon cap of 12288: move detail down a tier; caps are never raised"},
{"code":"ref-dangling","severity":"error","path":"docs/spec/berths/mooring.md","line":12,"subject":"RULE-TIDE-GATE","message":"..."},
{"code":"id-width","severity":"warning","path":"docs/spec/cranes.md","line":3,"subject":"CR-7","message":"...","debt":{"reason":"legacy import","expires":"2026-12-31","expired":false}}],
"stale":[{"code":"file-name","path":"docs/spec/harbor.md","subject":"","reason":"legacy import","expires":"2026-12-31","line":7}],"cannot_check":[]}
```

## Rules and edge cases

- Nothing computed (ADR-0012): no verdict, count, W or headroom derived; `message` never parsed (a budget's size lives only there); codes, slots, modes raw; `budget` the only code literal.
- A verdict, severity or mode outside its known list: a neutral badge, raw, last.
- `blocked` only on the `KnownCheckVerdict` line, in `src/mocks/` and tests; `src/health/` labels verdicts through `KNOWN_CHECK_VERDICTS`; no Health text matches `/block/i`.
- An omitted key shows nothing, never "0" or "null" (W-1); this read has no base, so no `introduced`, `new_debt`.
- Times as stored; no clock in `src/health/`; "expired" only from `debt.expired`; the report has no time: as old as its read.

## Acceptance criteria

Vitest (`ui/src/health/`, `mocks/check.test.ts`, `health.smoke`, else named), `ui_policy.rs`, the gates. M: the mutation turning it red, each applied and red.

- [x] AC-01 - `client.ts?raw` holds the `/check` `MISSING ENDPOINT` comment; `HttpClient.getCheck` 501 `notServed`, no `fetch` (`http.test.ts`); counting stub: Health one `getCheck`, one `getInbox`, nothing else; the home no `getCheck` (M: the home calls it; `getCheck` fetching).
  Amendment (ui-live, 2026-10-07): the comment names `GET /api/projects/:p/check`, every verdict a 200 document; `HttpClient.getCheck` fetches it, no 501 (`client.test.ts`, `http.test.ts`).
- [x] AC-02 - the types cite existing headings (`doc_pointers`); key records (`provisional.test.ts`) 7, 9, 9, 6, 2; no `debt`, `fix`: neither shown (M: `debt !== null`; a key dropped).
- [x] AC-03 - four verdicts, distinct labels and icons; `cannot-check` in `cannot-verify` (`tokens.test.ts`); `triage` neutral, raw, last; no `/block/i` in the view (M: the raw verdict; `cannot-check` toned clean).
- [x] AC-04 - `cannot-check`: no "0 B", causes verbatim; no introduced count; the `health-value` span of rows 3-6 and task W holds no digit (M: `?? 0`; "0 B" on `cannot-check`).
- [x] AC-05 - the five rows = the Inbox view's first five, in order; severity counts sum to its total; no `Date.now(`, `new Date(` in `src/health/` (M: an own sort; a clock read).
- [x] AC-06 - group counts sum to `findings.length`, errors first; `textContent` of `message`, `subject`, `fix.text` equals the JSON string; `<img onerror>` as text (M: `trim()`; `dangerouslySetInnerHTML`).
- [x] AC-07 - an `.md` path links the tree; `""`, `specengine.toml`, `subject` never; a stale entry never `<path>:<line>` (M: a `subject` link; a stale `path:line`).
- [x] AC-08 - Budgets = the `budget` findings; debt expired first, then `expires`; stale lines = `stale.length` (M: a regex over `message`; stale dropped).
- [x] AC-09 - "Check again" one more `getCheck`; none on `visibilitychange`, chip or filter; `slow` skeletons, `aria-busy`; `empty` the clean text; `error` verbatim, Retry one call; a failing `getInbox` leaves the other three (M: `refetchInterval`; one boundary; a re-read after a decision).
- [x] AC-10 - one row `tabindex="0"`; each key acts; Enter one history entry; no `document`/`window` listener; one `h1`; focus never on `body`; Home on the first row then typing keeps focus in the filter (Health, Tasks) (M: a `document` listener; Enter pushing twice; `useFocusLater` without a render per request).
- [x] AC-11 - R1, narrowing `ui-shell` AC-15: `ui_policy.rs` `nothing_is_worded_as_a_hold_on_work` (`hold_hits`; `policy.test.ts` alike): `blocking`, `blocker`, `unblock` fail anywhere in `ui/src`; `blocked` only on the one `provisional.ts` line starting `export type KnownCheckVerdict`, under `src/mocks/`, `src/test/`, `*.test.ts(x)`; `the_hold_scan_exempts_the_verdict_line_mocks_and_tests_only` on a temp `ui/`: a "Blocking" label and a second `blocked` line fail (M: the exemption widened to a file or the word list).
- [x] AC-12 - 17 packages; `pnpm lint` (0), `build`, `test` (62 files, 1 253, twice) green, silent; `ui_policy`, `anonymity`, `doc_pointers` 38/38; Questions names `ui-round`; docs gate clean, worst W 108 162 <= 108 468 B (M: an 18th package; `ui-health-round` left in `ui/src`).

## Owner's manual check

`pnpm --dir ui dev`: `#/harbor-sim/health`; a finding's path opens its node; debt, stale, Budgets; "Check again"; `#/ledger-api/health` "Fails the check"; `?scenario=cannot-check`, `empty`, `large`, `error`, `slow`; keyboard alone; VoiceOver on the verdict and a group; 200 %: regions stack.

## Open

- **Endpoint** (`rust-developer`, `ui-live`): `GET /api/projects/:p/check` = `spec --root R check --json` on the plain tree; a query 400; 200 for every verdict, the CLI's stdout less its final LF; 503 only on a `CliError`; `Cache-Control: no-store`; one call in flight per project; no DB, index refresh or git. It amends daemon-read (R2): "One door" lists `check` as unserved, and its exit mapping (1 -> 404, 2 -> 503) would pass `blocked` only via `HttpClient`'s GET-404 branch and drop a `cannot-check`'s causes. AC: byte-equal to the CLI on clean, debt, blocked, cannot-check fixtures, 200 each; no database in a fresh `HOME` (M: the generic exit mapping).
- **`.claude/agents/ui-developer.md`** (owner's text, ADR-0033 "Cost"): after lines 29-30 ("nothing is blocked ... (ADR-0012);") add "- `spec check`'s verdict `blocked` is a check outcome, not a hold: spelled only on the `KnownCheckVerdict` line, in mocks and tests, shown as "Fails the check";".

## Out of scope

Round; drift, `@assumes`, `unbound` (Phase 3); task W; any write; a base; a W target; Home summary; live refresh and the endpoint (`ui-live`); new packages.

## Implementation

| Module | What it does |
|---|---|
| `ui/src/health/` | `HealthView` (route, lazy `HealthRegions`), `CheckRegion`, `LeftRegion`, `FindingsRegion` (memoised rows), `DebtRegion`, `parts.tsx`; `findings.ts` (groups, filter, debt order), `labels.ts` |
| `ui/src/api/`, `mocks/` | `getCheck`, `notServed` with a `where`, the types, `useCheck`; `check.ts` builder, `cannot-check` |
| `ui/src/app/`, `ui/`, `styles/` | route, keys, `sections.ts`; `useFocusLater` (a render per request, Tasks too), `announce` on `ErrorPanel`/`ReadFailure`, icons; `--check-*`, `--finding-*` |
| `crates/specengine-eval/tests/ui_policy.rs` | `hold_hits`, `is_verdict_line`, the self-test |

Two iterations, review accepted; mutations 36 + 14, 8 on `ui_policy.rs`, all red. Vitest 62 files, 1 253; build main 492.87 kB, `HealthRegions` 20.89 kB. Iteration 2 made `ui/src/markdown/parse.test.tsx` deterministic (`act` + `waitFor`) and turned single-tick waits into waits for the observable.

Deviations, accepted: `CheckMode`, `FindingSeverity` known lists + `Unlisted`; `HealthRegions` lazy (eager main 526 kB, over Vite's 500 kB); only Check announces a shared failure; rows focusable `li`, roving `tabindex`, path links `tabIndex -1`; Enter on a non-`.md` row does nothing; the mock sorts as core (sample reordered); groups `role="group"`; neutral Debt wording on `cannot-check`; the filter folds the query once; a pathless cause `.: <message>`, as the CLI.
