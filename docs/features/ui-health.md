---
class: spec
status: draft
scope: [ui]
ref: ui-health analysis 2026-10-06, every recommendation accepted (working answers for the owner's morning review); 08 s2 Phase 4; Round split off as `ui-round`
adrs: []
---

# UI health: the check on mocks

## Why

Whether the corpus passes `spec check`, and what is left to decide, the owner learns today only on a terminal. Health (07 s3: "what is left", drift, budgets, W metrics) renders `spec check --json` and the Inbox the client already reads, read-only, on the mock (ADR-0033). Verdicts, findings, debt, budgets and W are core's: the UI computes no core logic (`architecture.md#ui`; ADR-0012, ADR-0031, ADR-0014). Drift, task W and rows 3-6 of 06 s7 "what is left" have no producer yet: they read "Not measured yet", never 0. No new ADR, no package; numbers shown as text.

Round leaves for `ui-round`, drafted once a round spec documents `spec round new|answer --json`: no producer (07 s2's `spec round` unbuilt; the plugin reserves the name, `crates/specengine-mcp/tests/plugin_files.rs` `RESERVED`); choosing and ordering questions is core logic (ADR-0031); the paste is a write (staged at most, confirmed on a terminal: ADR-0035; until then a `spec round answer FILE` command to copy); printing fights dark-only.

Working answers, 2026-10-06 (orchestrator, for the owner's morning review): the endpoint is `check` (`health` stays for Phase 3's composite, 07 s3); the verdict value `blocked` is spelled only on the `KnownCheckVerdict` line, in mocks and tests, labelled "Fails the check"; no W target (`docs/canon/spec-check.md` "Findings, debt, verdict": worst W "not a check, no cap"). Assumed: A1 the check reads the root's working tree; A2 it takes seconds in debug (an index walk 3.6 s: `daemon-read.md` "Rules and edge cases"); A3 times are `YYYY-MM-DDTHH:MM:SSZ` (CLI `utc_now`); A4 a debt `expires` is the last UTC day it holds; A5 `enforce-introduced` without a base reports `mode` `enforce` (its note on stderr only).

## Description and interactions

Route `#/<p>/health`: one `h1` "Health", four regions (`h2`), each with its own boundary, skeleton + `aria-busy`, empty text, and error (the message verbatim + Retry refetching that read only):

1. **Check**: the verdict as label + icon: `clean` "Clean", `observed` "Passes with findings", `blocked` "Fails the check", `cannot-check` "Could not check" in the `cannot-verify` token; any other value a neutral badge with the raw text. `mode` raw; the counts present (an omitted one not shown); "Worst W: <worst_w_bytes> B", on `cannot-check` "Not measured" and each cause `<path>: <message>` verbatim; "Task W: Not measured yet (the `bundles` log)". No W target, bar, percentage or colour.
2. **What is left** (06 s7): rows 1-2 from `getInbox` as the Inbox view orders them (`queueOrder`, `ui/src/inbox/order.ts`): counts by severity, then its first five, each linked `sectionHash(p, "inbox", id)`, times as stored; rows 3-6 "Not measured yet: <source>" (unapplied amendments: the `amendment` kind of `proposal-kinds`; `@assumes` against a decision, `unbound` accepted nodes, drift: Phase 3); row 7 is region 4's Budgets.
3. **Findings**: grouped by `code`, errors first, a count per group, a group over 10 rows collapsed. A row: severity (label + icon); `path:line`, an `.md` path linked `sectionHash(p, "tree", path)`, `""` and other paths as text; `subject`; `message` verbatim; debt "until <expires>: <reason>" or "expired"; `fix.text` as text, no apply (`spec-check.md` "Open owner questions" Q-3). Chips per severity and code and a text filter make no call.
4. **Debt and budgets**: the findings with `debt`, by stored `expires`, expired first; each stale entry "`.spec-debt.toml` line <line>: <code> on <path> matches nothing" (`line` the baseline file's: core `check/baseline.rs` `DebtEntry`); Budgets: exactly the `budget` findings, `subject` (the slot) and `message` verbatim.

"Check again" refetches `getCheck` once. A `clean` report with no finding or debt: "The check is clean: <n> documents, no findings, no debt." + an Inbox link. Keys as Tasks (`docs/features/ui-tasks.md` "Implementation": one roving row; Up/Down, `j`/`k`, Home/End, Enter, `?`). Home, Tasks, Inbox unchanged; the nav's Questions says "Not built yet: arrives in slice `ui-round`".

## Data

**Client** (`ui/src/api/client.ts`):

```ts
/** MISSING ENDPOINT GET /api/projects/:p/check (= spec check --json; rust-developer, ui-live; 07 section 3's health is a later composite) */
getCheck(project: string): Promise<CheckReport>;
```

`HttpClient.getCheck`: the not-served marker (`ClientError {status: 501, notServed: true}`; status 0 is no response), no `fetch`. Key `["check", p]`: read on entering Health, never on window focus or an interval, not in `READS_AFTER_DECISION`.

**Provisional types** (`ui/src/api/provisional.ts`), citing `docs/canon/spec-check.md` "Findings, debt, verdict" (its JSON; core `check/report.rs` `Report`):

```ts
CheckReport { mode; verdict: CheckVerdict; counts: CheckCounts; findings: CheckFinding[]; stale: DebtEntry[];
  new_debt?: NewDebtEntry[]; cannot_check: CheckCause[] }                      // 7 keys
CheckCounts { documents; errors; warnings; debt; expired; stale; introduced?; new_debt?; worst_w_bytes }  // numbers, 9
CheckFinding = Finding & { fix?: { span: { start: number; end: number }; text }; debt?: { reason; expires; expired: boolean };
  introduced?: boolean }                                                        // 9 keys
DebtEntry { code; path; subject; reason; expires; line: number }  NewDebtEntry = DebtEntry & { head_expires? }
CheckCause { path; message }
CheckVerdict = KnownCheckVerdict | Unlisted  // clean | observed | blocked | cannot-check, on one line
// `?`: omitted when absent, never null (`docs/canon/spec-check-cli.md` "spec check", W-1); unmarked: string
```

`Finding` (six keys, citing the same heading) stays the keys every finding has; `CheckFinding` adds the three optional ones (R5).

**Mock** (`ui/src/mocks/`, one builder, no clock): `normal`: `harbor-sim` `observed` (below), `ledger-api` `blocked`; `empty`: `clean`, no finding; a new scenario `cannot-check`: two causes, `worst_w_bytes` 0; `large`: `harbor-sim` 3 000 findings over 12 codes; `error`, `slow` as every read.

```json
{"mode":"observe","verdict":"observed","counts":{"documents":41,"errors":2,"warnings":0,"debt":1,"expired":0,"stale":1,"worst_w_bytes":61234},
"findings":[{"code":"budget","severity":"error","path":"docs/canon/tides.md","line":1,"subject":"canon","message":"12950 bytes, over the canon cap of 12288: move detail down a tier; caps are never raised"},
{"code":"id-width","severity":"warning","path":"docs/spec/cranes.md","line":3,"subject":"CR-7","message":"...","debt":{"reason":"legacy import","expires":"2026-12-31","expired":false}},
{"code":"ref-dangling","severity":"error","path":"docs/spec/berths/mooring.md","line":12,"subject":"RULE-TIDE-GATE","message":"..."}],
"stale":[{"code":"file-name","path":"docs/spec/harbor.md","subject":"","reason":"legacy import","expires":"2026-12-31","line":7}],"cannot_check":[]}
```

## Rules and edge cases

- Nothing computed (ADR-0012): no verdict, count, W or headroom derived; `message` never parsed (a budget's size and cap live only there: core `check/engine.rs` `fn budget`); codes, slots and modes are core's strings, shown raw; `budget` is the only code literal, no slot literal (R6).
- WHEN a verdict, severity or mode is outside its known list THEN a neutral badge with the raw text, sorted last.
- The value `blocked` appears only on the `KnownCheckVerdict` line of `provisional.ts` (with the other three, e.g. a `KNOWN_CHECK_VERDICTS` tuple), under `src/mocks/` and in tests; `src/health/` labels a verdict through that tuple, never by the literal. No Health text matches `/block/i`.
- An omitted key shows nothing, never "0", "null" or "none" (W-1); `introduced`, `new_debt` exist only with a base, which this read never has.
- Times and dates as stored; no clock read in `src/health/`; "expired" only from `debt.expired`.
- Known limits: the report carries no time (R4): an answer is as old as its read, and "Check again" re-reads; 3 000 findings render collapsed (R3).
- `ui/README.md` "Screen rules" hold.

## Acceptance criteria

Vitest (`ui-developer`), `ui_policy.rs` (`test-engineer`), the gates. Counting stub: calls per member; builder: a test-built answer; M: the mutation turning it red.

- [ ] AC-01 - `client.ts?raw` holds the `/check` `MISSING ENDPOINT` comment; `HttpClient.getCheck` rejects with the not-served marker (501, `notServed`), no `fetch`; counting stub: `#/harbor-sim/health` one `getCheck`, one `getInbox`, no other call; `#/harbor-sim` no `getCheck` (M: the home calls it; `getCheck` fetching).
- [ ] AC-02 - the check types cite an existing heading (`doc_pointers`); key records equal the cited lists: `CheckReport` 7, `CheckCounts` 9, `CheckFinding` 9, `DebtEntry` 6, `CheckCause` 2; a builder finding without `debt`, `fix` shows neither (M: `debt !== null` as the test; a key dropped).
- [ ] AC-03 - the four verdicts: distinct labels, an icon each; `cannot-check` in the `cannot-verify` token; builder verdict `triage`: neutral, raw, last; the view's text never matches `/block/i` (M: the raw verdict shown; `cannot-check` styled as clean).
- [ ] AC-04 - `cannot-check`: no "0 B", the causes verbatim; no `counts.introduced`: no introduced count; the value cells of rows 3-6 and task W hold no digit (M: `?? 0`).
- [ ] AC-05 - `harbor-sim`, `ledger-api`: the five rows = the Inbox view's first five, in order; the severity counts sum to its total; no `Date.now(`, `new Date(` in `src/health/` (M: an own sort; a clock read).
- [ ] AC-06 - group counts sum to `findings.length`, errors first; `textContent` of `message`, `subject`, `fix.text` equals the JSON string (builder: leading blank lines, trailing spaces); `<img src=x onerror=alert(1)>` as text, no `img` (M: `trim()`; an HTML sink).
- [ ] AC-07 - an `.md` path links `sectionHash(p, "tree", path)`; `""`, `specengine.toml` and every `subject` never link; a stale entry is never shown as `<path>:<line>` (M: a `subject` link; a stale `path:line`).
- [ ] AC-08 - Budgets = exactly the `budget` findings; debt by stored `expires`, expired first; stale lines = `stale.length` (M: a regex over `message`; stale dropped).
- [ ] AC-09 - "Check again": exactly one more `getCheck`; none on window focus, chip or filter; `slow`: skeletons, `aria-busy="true"`; `empty`: the clean text + Inbox link; `error`: verbatim, Retry one call; a rejecting `getInbox` leaves Check, Findings, Debt rendered (M: `refetchInterval`; one boundary).
- [ ] AC-10 - one row `tabindex="0"`; each key of the keys list acts; Enter one history entry; no `document` or `window` keydown listener added; one `h1`; focus never on `body` (M: a `document` listener).
- [ ] AC-11 - R1, amending `ui-shell` AC-15 narrowly (`test-engineer`: `crates/specengine-eval/tests/ui_policy.rs` `nothing_is_worded_as_a_hold_on_work`): `blocked` passes only on the `KnownCheckVerdict` line of `src/api/provisional.ts`, under `src/mocks/` and in tests (`*.test.ts`, `*.test.tsx`, `src/test/`); every other hit, and `blocking`, `blocker`, `unblock` anywhere, still fails; on a temp `ui/` (`ui_sources_in`) a "Blocking" label in `src/health/` and `blocked` on another `provisional.ts` line fail (M: the exemption widened to a file or the word list).
- [ ] AC-12 - 15 packages; `pnpm lint` (0 warnings), `build`, `test` green, silent; `ui_policy`, `anonymity`, `doc_pointers` green; Health built (`slice` `null`), Questions names `ui-round` (`sections.ts`, `App.test.tsx`); docs gate clean, worst W <= this draft's (M: a 16th package; `ui-health-round` left in `ui/src`).

## Owner's manual check

`pnpm --dir ui dev`: `#/harbor-sim/health`, the four regions; a finding's path opens its node; a debt row, the stale line, Budgets' message; "Check again"; `#/ledger-api/health` "Fails the check"; `?scenario=cannot-check`, `empty`, `large` (groups collapsed), `error`, `slow`; keyboard alone; VoiceOver on the verdict and a group; 200 %: regions stack.

## Open

- **Endpoint** (`rust-developer`, with `graph`, in `ui-live`): `GET /api/projects/:p/check` = `spec --root R check --json` on the root's plain tree; any query -> 400; 200 for every verdict, the body the CLI's stdout without its final LF; 503 only on a `CliError`; `Cache-Control: no-store`; one call in flight per project; no DB, no index refresh, no git (`spec-check-cli.md` "spec check"); a full walk per call. It amends daemon-read (R2): "One door" names `check` among calls no handler makes, and its exit mapping (1 -> 404, 2 -> 503) would pass `blocked` as data only through `HttpClient`'s GET-404 branch (`ui/src/api/http.ts`) and drop a `cannot-check`'s causes. Its AC: the body byte-equal to `spec --root R check --json` minus the final LF on clean, debt, blocked and cannot-check fixtures, 200 each; a query 400; no database in a fresh `HOME` (M: the generic exit mapping; an index refresh).
- **Rename** (`ui-developer`, this slice): `sections.ts` Health `slice: null`, Questions `slice: "ui-round"`; `App.test.tsx` `UNBUILT` keeps the Questions row, as `ui-round`. Docs already say `ui-health` + `ui-round` (08 s2 Phase 4); the shipped `ui-home.md` keeps its `ui-health-round`.
- **`.claude/agents/ui-developer.md`** (owner's text, ADR-0033 "Cost"): after lines 29-30 ("nothing is blocked ... (ADR-0012);") add "- `spec check`'s verdict `blocked` is a check outcome, not a hold: spelled only on the `KnownCheckVerdict` line, in mocks and tests, shown as "Fails the check";". Until applied, `ui-tasks` and `ui-home` "Open" hold.

## Out of scope

Round, its sheet and paste (`ui-round`); drift, `@assumes`, `unbound` (Phase 3); task W (the `bundles` log, `spec-cli-bundle.md` "Not yet"); any write, a fix applied; `--changed`, `--staged`, a base; a W target or headroom; a summary on Home; refresh on file change (`ui-live`); the endpoint; generated types; new packages.

## Implementation

Not built. **At shipping**: `ui/README.md` "Screen rules" ("Text": no "block" wording) gains the `blocked` exception of AC-11; `ui-tasks` "Implementation" keys re-cited if moved.
