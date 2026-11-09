---
class: spec
status: shipped
shipped: 2026-11-09
scope: [ui]
ref: RM-0090; v4 step 2d (08 s2 Phase 7, "v4 scope"); the owner's decision 2 of 2026-11-08 and ADR-0072; writer-studio's follow-up 2; Q1-Q4 working answers, the owner's review of the Russian open
adrs: []
---

# ui-languages-sweep: every screen left joins the catalog, Russian in its own chunk (RM-0090)

## Why

Step (6) of the studio trial switches to Russian and back. After 1b and 2b, Inbox, Tasks, Board, Roadmap, Spec tree, Graph, Health and Questions were English, and so were `tree/LinksPanel.tsx` and `trace/CardsBlock.tsx` inside Flow's Russian panel. Measured at `406ce2a`: 1 059 literals in JSX and 327 in code, about 1 040 keys; both catalogs in the main chunk would have passed 1b's +120 kB bar by about 112 kB. So every screen joins the catalog, Russian moves to a chunk only a Russian viewer fetches (a Russian start waits up to 2 s for it, Q1), and Flow's Up names its holder by title. No ADR: ADR-0072 holds, and the bar was 1b's spec rule kept in canon (D8). How it works now: `docs/canon/ui-languages.md`, `docs/canon/spec-flow-place.md`.

## Acceptance criteria

Tests in `ui/src`.

- [x] **LS-1 Every screen scanned.** `policy.test.ts`: every `src/` directory but `api/`, `i18n/`, `mocks/`, `styles/`, `test/` (`NOT_SCANNED`), each non-test `.tsx` of the ten converted. M1 `/src/health/` excluded.
- [x] **LS-2 No word of their own.** Both rules find nothing (1 059 and 327 before). The detector reports `<Field label="Title" />`, `{ pool: "Pool" }`, `` `${n} open, ${m} done` `` and passes `t("roadmap.pool")`, `` `spec approve ${id}` ``, `x === "Release"`, `case "Escape":`, `className="a b-c"`. M2 `label="Title"` in `board/CardDialogs.tsx`, M3 `"Pool"` in `roadmap/text.ts`, M4 the code token widened to any `spec`.
- [x] **LS-3 The screens switch.** `app/screens.language.test.tsx`, stub client: on Inbox, Tasks, Board, Roadmap, Flow with a node selected, Spec tree, Graph, Health and Questions, Russian shows `ruT`'s heading and a string of the screen's group, with no client call and focus on the select; English comes back. M5 a lane name memoised with `[]`.
- [x] **LS-4 Served text stays.** Same file, in Russian: a daemon refusal or note on Inbox, Board, Roadmap and Health byte for byte under `lang="en"`; a node title with no `lang`. M6 `lang="en"` dropped in `inbox/StagedChoice.tsx`.
- [x] **LS-5 Parity and plurals.** `i18n/catalog.test.ts` over the new groups, `pnpm build` failing on a missing Russian key; `i18n/plural.test.ts`: every plural group under `ruT` `_one` at 1 and 21, `_few` at 3, `_many` at 5; `roadmap/text.test.ts`: the header and counts in Russian. M7 a `ru` key deleted, M8 `count === 1 ?`.
- [x] **LS-6 Russian on demand.** `i18n/load.test.tsx`, fresh instance, mocked `./ru`: an English start imports nothing; a Russian start renders Russian first, `lang` `ru`; past 2 000 ms English, then Russian, English kept if chosen meanwhile; a failed import gives English, the stored `ru`, `language.failed`, and the next choice imports again; two choices import once, the last wins, `lang` `en` while pending. `app/language.test.tsx`: options read `languageName`. M9 the import not memoised, M10 the switch before the bundle.
- [x] **LS-7 The split.** `policy.test.ts`: one `import("./ru")`, no static `ru` import outside tests. After `pnpm build`, Cyrillic (U+0400-U+04FF) is in exactly one file besides `MockClient-*.js`: `ru-*.js`. M11 `import { ru }` in `catalog.ts`.
- [x] **LS-8 Up by title.** `flow/FlowView.test.tsx`: Up reads `flow.up` with the holder's title from a held read, else its ID; no extra call. M12 `name: group`.
- [x] **LS-form.** `pnpm lint`, `pnpm build`, `pnpm test` (3622); eval `anonymity`, `doc_pointers`, `build_graph`, `workspace_names`, `ui_policy`; `spec check` 0 errors, 0 warnings.
- [x] **L1 Russian fits.** 1440x900 and 320x640, every LS-3 screen: `scrollWidth <= innerWidth`.
- [x] **L2 Round trip live.** `spec serve`: one `ru-*.js` fetch, no `/api` request on a switch, console clean.

## Out of scope

`src/api/http.ts`'s transport messages; served text, spec content, a pack's vocabulary (RM-0061); a third language, a lazy English, RTL; writer-studio follow-up 1, the dash order (`spec-flow-layout.md` "Forms"); any daemon, CLI, MCP or package change.

## Implementation

Review ACCEPT, minors fixed in iteration 2. English 551 values to 1 806. Build: `index-*.js` 622.62 kB (gzip 171.96; 726.99 at base), `Icon-*.js` 140.15 (44.53), `ru-*.js` 134.98 (31.13). First-load JS 768.96 kB, gzip 219.15, against 746.17 and 216.22 at `406ce2a`; the new gate and its 845 kB bar: canon "Size".

**Departures, for the owner's review.** (1) Vite put i18next and English in the preloaded `Icon-*.js`: the old index-only bar passed only by that, so the gate is now first-load JS. (2) LS-7 amended: `MockClient-*.js` keeps the 30 Cyrillic characters it had at the base. (3) R11 exceptions: Health's "None" line shows `amends` as plain text, as AC-14 of ui-tree-node keeps link types out of the catalog, one assertion removed; LS-8 changes Flow's "Up: G" and "Up: C". (4) `PROSE_ALLOWED` adds code only: CSS transforms, Graph's `Home`/`End`, the lock diff note's prefix, `main.tsx`'s rootless error; `CODE_METHODS` exempts a first argument. (5) Beyond the file list: `app/shortcuts.tsx`, `flow/labels.ts`, `i18n/fill.ts` (`around`), `overview/InboxRegion.tsx`, `palette/` (matches the agent's summary, else kind and target, in any language), `styles/app.css`, `ui/Badge.tsx` (`served`). (6) Exports reshaped: builders take `t`; `WORDING` became `verbOf`, `wordingOf`, `isStaged`; `CATALOGS`, `counted`, the `*_ARIA_LABELS` and notice constants gone; `RoundResult` holds IDs, worded at render. (7) English slips kept byte-equal ("1 links not shown", "1 tokens"): RM-0091. (8) LS-6's file is `.tsx`. (9) Iteration 2 reworded Russian calques: lock, bundle, frozen apart from locked.

**Mutations**, each red, restored byte-equal (`cmp`): M1 LS-1; M2, M3 LS-2; M4 the detector test; M5 LS-3 Roadmap once an `h2` check was added (the jump button also shows the name); M6 LS-4 Inbox; M7 `tsc` exit 2, `catalog.test.ts` 2 red; M8 `text.test.ts` 3 of 5, re-run after iteration 2; M9 `load.test` 2 red, M10 1; M11 LS-7; M12 FlowView 2 red. Iteration 2's checks: a Russian `health.drift.total` with `{{skipped}}` first reddens `AROUND`'s; `lang` dropped from `Badge`, LS-4's served badge.

**Looks, 2026-11-09**, by the orchestrator. L1 under `vite preview`, mock data, exact-size iframe: ten screens in Russian, overflow 0 at both sizes, `lang` `ru`; Latin left: data, config words, check codes, Graph's `REF`, raw unknown values. L2 on a scratch `fixtures/trace-almanac` (not `studio-lamp`): a Russian start fetches `ru-*.js` once; en, ru, en, ru on six screens with no `/api` or JS fetch; an English start fetches it only on a choice.

| Module | What it does |
|---|---|
| `i18n/index.ts`, `catalog.ts`, `main.tsx` | the lazy chunk, the gate, `languageName` |
| `i18n/en.ts`, `ru.ts`; the ten directories | the keys; text modules take `t` |
| `app/LanguageSwitcher.tsx` | `aria-busy`, `language.failed` |
| `ui/Badge.tsx`, `*/labels.ts` | served values under `lang="en"` |
| `api/queries.ts`, `flow/FlowView.tsx` | `useHeldTitle`: Up by title |
| `policy.test.ts`, `i18n/*.test.*` | scan, split, `AROUND` |
