---
class: spec
status: draft
scope: [ui]
ref: RM-0090; v4 step 2d (08 s2 Phase 7, "v4 scope"); the owner's decision 2 of 2026-11-08 and ADR-0072; writer-studio's two follow-ups (its "Implementation", departure 1; `spec-flow-layout.md` "Forms"); Q1-Q4 working answers
adrs: []
---

# ui-languages-sweep: every screen left joins the catalog, Russian in its own chunk (RM-0090)

## Why

Step (6) of the studio trial switches to Russian and back (08 s2 Phase 7, "The studio trial"). After 1b (RM-0082) and 2b (RM-0084), Russian covers only the shell, nav, palette, Home, `src/ui/`, `workspace/` and Flow. Inbox, Tasks, Board, Roadmap, Spec tree, Graph, Health and Questions are still English, and so are `tree/LinksPanel.tsx` and `trace/CardsBlock.tsx` inside Flow's Russian panel. This slice converts all of them, so no screen mixes languages.

It was measured first ("Data"): about 1 040 keys, about twice 1b's 551. With both catalogs in the main chunk this adds about 120 kB, and only 8.8 kB are left before the canon's bar (`docs/canon/ui-languages.md` "Known limits": past +120 kB over the base before 1b, Russian goes to a lazy chunk). So Russian moves to its own chunk here, as 1b's R-size foresaw.

### Decisions

- **D1** Every `src/` directory is scanned except `api/`, `i18n/`, `mocks/`, `styles/` and `test/`. The ten here join the scan, and a new directory is scanned from the start.
- **D2** A second scan rule, prose, covers `.ts` and `.tsx`. LG-10 sees JSX positions only, so it misses `roadmap/text.ts`, `inbox/decisions.ts` and a local field's `label="Title"`.
- **D3** `ru.ts` is reached only through one `import("./ru")`. English stays in the main chunk and loads synchronously, as it does now.
- **D4** A Russian start renders nothing until the chunk lands, for at most `LANGUAGE_WAIT_MS` (2 000 ms). Past that it renders English, then Russian once the chunk lands.
- **D5** The select names each language with `Intl.DisplayNames` in that language, so the Russian option reads correctly before its chunk loads. `language.self` is removed.
- **D6** Flow's Up names its holder by the title from a held read, and falls back to the ID. This is writer-studio follow-up 2: a label's words, no extra read. Follow-up 1 stays out ("Out of scope").
- **D7** The ui-developer writes the Russian values and the owner reviews them; rewording breaks no test (1b D14).
- **D8** No ADR. ADR-0072 holds, the split is the canon's own rule, and 1b's D4 ("both catalogs in the main chunk") was a spec decision. The canon rewrite at shipping replaces it.

### Questions for the owner, with working answers

- **Q1** The wait: a Russian start shows an empty page for up to 2 s, then English until the chunk lands. Working answer: D4. The alternative is both catalogs in main with the bar raised to about +240 kB, which costs every English viewer about 80 kB.
- **Q2** The size: about 1 040 keys against RM-0090's 3-5 pipeline hours, re-estimated at 5-8. Working answer: one slice. If it runs long, cut it at a directory boundary: the chunk, the scan and the directories already converted ship, and the rest stays in RM-0090.
- **Q3** Follow-up 1 (dash order): no item before the trial. The canon keeps the limit ("Out of scope").
- **Q4** The review of about 1 040 new Russian values (about 60 kB): open, and it blocks nothing.

## Description and interactions

1b's catalog, keys, plurals, dates, served-text and hotkey rules stay as they are (`docs/canon/ui-languages.md`). This slice adds:

- **Screens.** Inbox, Tasks (a route, off the nav), Board, Roadmap, Spec tree, Graph, Health, Questions (`src/round/`) and Flow's panel parts take every word the UI writes from the catalog. Spec content, served text, IDs, paths and commands show as they do now.
- **The select and a reload.** Russian's first choice fetches `ru-*.js` once; a reload in Russian waits for it, milliseconds on a local daemon, with no flash of English. What shows meanwhile and on a failure (`language.failed`, `role="status"`): R1-R5.
- **Flow's Up.** `flow.up` with the holder's title when a held read has it.
- The daemon, CLI, MCP, packages, `vite.config.ts` and `eslint.config.js` are unchanged. The daemon serves the new chunk like every other file in `ui/dist` (rust-embed).

At shipping the spec-writer rewrites these canon parts: `docs/canon/ui-languages.md` "Catalog", "Switcher", "Converted", "Guards" and "Known limits"; `docs/canon/spec-flow-place.md` "Known limits"; and `ui/README.md`'s "Text" line. Code comments cite those canon headings, never this spec's sections, which are dropped at compaction.

## Data

### Measured at `406ce2a`

A prototype of the two rules ran over non-test files, using TypeScript's parser as `policy.test.ts` does. JSX hits are LG-10's. Prose hits are the prose rule's (below) outside JSX positions, false positives included. Bytes are the English text.

| Directory | `.tsx`/`.ts` | JSX hits (B) | Prose hits (B) |
|---|---|---|---|
| `inbox/` | 10/8 | 179 (4 453) | 56 (2 303) |
| `tasks/` | 10/5 | 122 (2 211) | 21 (702) |
| `board/` | 7/4 | 91 (1 688) | 105 (1 481) |
| `roadmap/` | 3/2 | 26 (327) | 29 (644) |
| `tree/` | 15/6 | 238 (5 501) | 37 (1 010) |
| `graph/` | 6/4 | 106 (2 145) | 21 (504) |
| `health/` | 12/3 | 184 (3 099) | 29 (405) |
| `round/` | 3/3 | 80 (2 436) | 20 (593) |
| `trace/` | 2/0 | 26 (400) | 3 (34) |
| `markdown/` | 3/2 | 7 (47) | 6 (77) |
| total | 71/37 | 1 059 (22 307) | 327 (7 753) |

`tree/LinksPanel.tsx` has 18 hits and `trace/CardsBlock.tsx` 26. The directories already converted have 0 JSX hits and 18 prose hits, all of them code: palette's group IDs (11), `ui/Dialog.tsx`'s selectors (4) and Flow's served rollup words (3).

### The cost, estimated

The estimate is calibrated on the catalogs inside today's `index-*.js` (726 995 B): `en` is 21 056 B for 551 values, `ru` 37 388 B for 607 entries. Russian values take 2.12 times the bytes of English ones, plus about 13.5 B of key per entry. Pieces join into sentences, so 1 386 pieces with 30.1 kB of English make about 1 040 keys.

| | `en` | `ru` | views | main chunk |
|---|---|---|---|---|
| both in main | +44 kB | +80 kB | about 0 (literals -22, calls +20) | about 848 kB: +232 over 615.81, past the 735.81 bar by 112 |
| Russian lazy | +44 kB | `ru-*.js` about 117 kB | about 0 | about 731 kB: +115 |

The estimate is good to plus or minus 25 %. At any point in that range, Russian alone (60-100 kB) passes the 8.8 kB left. Once Russian leaves the main chunk, the bar has done its job: the main chunk grows only with English, and each slice prints both sizes.

### Keys

Keys follow 1b's grammar (canon "Catalog"). The first segment is the directory, with `round` for Questions. `inbox`, `tasks`, `board`, `tree` and `health` grow; `roadmap`, `graph`, `round`, `trace` and `markdown` are new. A sentence around an element is one key, filled by `fill` and `slot`. Two counts on one line combine two plural groups:

```ts
roadmap: {
  open_one: "{{count}} open", open_other: "{{count}} open",
  done_one: "{{count}} done", done_other: "{{count}} done",
  counts: "{{open}}, {{done}}", // t("roadmap.counts", {open: t("roadmap.open", {count: o}), done: t("roadmap.done", {count: d})})
},
language: { label: "Language", failed: "Russian could not be loaded; choose it again to retry." }, // `self` removed (D5)
```

### `ui/src/i18n/`

```ts
// index.ts
export const LANGUAGE_WAIT_MS = 2000;
export function loadLanguage(language: Language): Promise<boolean>; // en true; ru: import("./ru") once, addResourceBundle; a failure retried next call; never rejects
export function readyLanguage(): Promise<void>; // main.tsx's gate, chained by .then, never a top-level await (main.tsx's note); never rejects
export function useLanguage(): { language: Language; pending: Language | null; failed: boolean; choose(next: Language): void };
export function languageName(language: Language): string; // Intl.DisplayNames([l], {type: "language"}).of(l), first letter upper-cased in l; else l
```

The instance starts with `lng: "en"` and both `supportedLngs`, and `readyLanguage` switches it. `switchLanguage` and `translateIn` work as now for a loaded language; `src/test/setup.ts` preloads Russian once. `catalog.ts` imports no `ru` value, so `CATALOGS` either holds English only or goes. `ru.ts` stays `export const ru: Catalog`, type-checked by `tsc`.

### The scan (`ui/src/policy.test.ts`)

- **Scanned**: every non-test `.ts` and `.tsx` under `src/` except `/src/api/`, `/src/i18n/`, `/src/mocks/`, `/src/styles/` and `/src/test/`. `CONVERTED` becomes this exclusion list.
- **The JSX rule** (LG-10 as built), `.tsx`: no JSX text containing an ASCII letter, and no literal word as a child, in `SHOWN_ATTRIBUTES` or in a `TEXT_PROPS` prop.
- **The prose rule**, `.ts` and `.tsx`: no string literal or template text that reads as English, meaning a phrase `/\b[A-Za-z][a-z]+[ ,;:]+[a-z]{2,}\b/` or a single capitalised word `/^[A-Z][a-z]+[.:]?$/`. These positions are code and never count as a hit:
  - an import or export specifier, a type, a property name, a `case` label, or an operand of `===`, `!==` or `in`;
  - `t`'s key;
  - an argument of a DOM, storage, string or collection method (`querySelector*`, `closest`, `getItem`, `startsWith`, `split`, `replace*`, `get`, `has`…);
  - the value of an attribute or property that is not shown (`className`, `key`, `id`, `href`, `lang`, `role`, `type`);
  - a code token: text starting with `spec `, or a `…COMMAND` constant, shown in `<code>`;
  - `PROSE_ALLOWED`: exact literals per file, each with its reason (palette's group IDs, `ui/Dialog.tsx`'s selectors, Flow's rollup words). The rule is a net, not a proof: LS-3 and review cover what it misses (a bare `none`).
- **Never literals**: IDs, paths, served text and spec content are data, so neither rule sees them. They render under the canon's "Served text" rule.
- **The split**: no non-test module imports `i18n/ru` statically, and `src/i18n/index.ts` holds the only `import("./ru")`.

### Files

The ui-developer may touch only `ui/**`, except `ui/README.md` and `ui/dist`: `src/i18n/{en,ru,catalog,index}.ts`, `src/app/LanguageSwitcher.tsx`, `src/main.tsx`, the ten directories, `src/flow/FlowView.tsx`, `src/api/queries.ts` (the held title), `src/test/{setup,text}.ts`, and the tests below. No change to `package.json`, the lockfile, `vite.config.ts` or `eslint.config.js`.

## Rules and edge cases

- **R1** WHEN the start language is English, the page SHALL import no Russian chunk and render at once.
- **R2** WHEN it is Russian, the root SHALL stay empty until the chunk lands, then render Russian. Past `LANGUAGE_WAIT_MS` it SHALL render English and switch when the chunk lands, unless the viewer has chosen meanwhile.
- **R3** WHEN the chunk fails, the page SHALL show English and keep the stored choice, so a reload retries. It SHALL show `language.failed` until the next choice, which SHALL import again.
- **R4** WHEN Russian is chosen before its chunk is in, the page SHALL keep its language and the select SHALL show Russian with `aria-busy`, focus kept. The last choice wins. No API request is made, and a successful fetch happens once per page.
- **R5** `<html lang>` SHALL follow the language shown, never the pending one.
- **R6** Served text, commands, IDs and paths show byte for byte. The daemon's text carries `lang="en"` per item; spec content carries no `lang` (canon "Served text"). The target directories hold no `lang="en"` today, so this slice adds it.
- **R7** A line the UI builds in the CLI's form (Roadmap's header, a review's revision line) is chrome: its words translate, and its IDs, numbers and `->` stay.
- **R8** Every count goes through a plural group's `{{count}}`, and numbers stay ungrouped. A stored time shows as the screen shows it now.
- **R9** Sorts and filters compare data, never a translated word.
- **R10** Nothing resolves at module load (1b D5): a `.ts` text module takes `t`.
- **R11** English stays byte-equal (1b D12): no existing test's expected English changes.
- **R12** The language changes no right, gate, finding or exit (ADR-0012).

## Acceptance criteria

Tests live in `ui/src` unless named otherwise. Mutations M1-M12 are each run once, and the test that turns red goes in "Implementation".

- [ ] **LS-1 Every screen scanned.** `policy.test.ts`: the scanned set holds every non-test `.tsx` under `src/inbox/`, `tasks/`, `board/`, `roadmap/`, `tree/`, `graph/`, `health/`, `round/`, `trace/` and `markdown/`, and every `src/` directory except the five excluded. M1: `/src/health/` added to the exclusions.
- [ ] **LS-2 No word of their own.** `policy.test.ts`: both rules find nothing in the scanned set (1 059 and 327 before). The prose detector reports `<Field label="Title" />`, `{ pool: "Pool" }` and `` `${n} open, ${m} done` ``. It passes `t("roadmap.pool")`, `` `spec approve ${id}` ``, `x === "Release"`, `case "Escape":` and `className="a b-c"`. M2: `label="Title"` restored in `board/CardDialogs.tsx`. M3: `"Pool"` restored in `roadmap/text.ts`. M4: the code-token exemption widened to any text containing `spec`.
- [ ] **LS-3 The screens switch.** `src/app/screens.language.test.tsx` (new), over the stub client. On Inbox, Tasks, Board, Roadmap, Flow with a node selected (its links and cards), Spec tree, Graph, Health and Questions:
  - choosing Russian shows `ruT`'s heading and one string from the screen's own group (on Roadmap, a lane name);
  - no client call is made, and focus stays on the select;
  - switching back to English shows `enT`'s strings.
  M5: a Roadmap lane name memoised with `[]` deps.
- [ ] **LS-4 Served text stays.** Same file, in Russian: a daemon refusal or note on Inbox, Board, Roadmap and Health shows byte for byte inside `lang="en"`, and a node title shows with no `lang`. M6: the `lang="en"` added to `inbox/StagedChoice.tsx`'s refusal dropped.
- [ ] **LS-5 Parity and plurals.**
  - `src/i18n/catalog.test.ts` applies LG-1's rules to the new groups, and `pnpm build` fails when a Russian key is missing.
  - `src/i18n/plural.test.ts`: every plural group in `en`, under `ruT`, gives `_one` at 1 and 21, `_few` at 3 and `_many` at 5.
  - `src/roadmap/text.test.ts` (new): the header and `countsText` in Russian at 1, 3 and 5 open.
  M7: a new group's `ru` key deleted. M8: `count === 1 ?` in `roadmap/text.ts`.
- [ ] **LS-6 Russian loads on demand.** `src/i18n/load.test.ts` (new), with a fresh instance and a mocked `./ru`:
  - R1: an English start imports nothing.
  - R2: a Russian start's first render is Russian, with `lang` `ru`. With fake timers past 2 000 ms it renders English, then Russian when the chunk lands; English is kept if chosen meanwhile.
  - R3: a failing import gives English, keeps the stored `ru` and shows `language.failed`; a second choice imports again.
  - R4: two choices import once, a choice back during the load wins, and `lang` stays `en` while pending.
  `src/app/language.test.tsx`: the options read `languageName`, with `\u` escapes in the test. M9: the import not memoised. M10: the language switched before the bundle lands.
- [ ] **LS-7 The split.** `policy.test.ts` checks the one dynamic import and that no static `ru` import exists outside tests. After `pnpm build`, a `node -e` count of the characters U+0400 to U+04FF (written as `\u` escapes) over `dist/assets/*.js` reports exactly one file, `ru-*.js`. Vite's `index-*.js` and `ru-*.js` lines (minified, gzip) go in "Implementation", against 726.99 kB. M11: `import { ru } from "./ru"` restored in `catalog.ts`.
- [ ] **LS-8 Up by title.** `src/flow/FlowView.test.tsx`: entering a group from its holder's picture, Up reads `flow.up` with the holder's title. A holder that no held read titles shows its ID. The client's call count is unchanged. M12: `name: group` restored.
- [ ] **LS-form.** `pnpm lint`, `pnpm build` and `pnpm test` pass, with no existing test's expected English changed (R11). `cargo nextest run -p specengine-eval --test anonymity --test doc_pointers --test ui_policy` passes, with `ui_policy` run last. `spec check` reports 0 errors and 0 warnings.
- [ ] **L1 Russian fits.** Run `pnpm build && pnpm preview` with `?scenario=normal` in Russian, and open each screen of LS-3 at 1440x900 and 320x640. At 320, `document.documentElement.scrollWidth <= innerWidth`; buttons and tabs wrap, and nothing is cut unseen.
- [ ] **L2 Round trip live.** Run `spec serve` over a scratch copy of `fixtures/studio-lamp`. On each screen switch Russian, English, Russian. DevTools shows one `ru-*.js` fetch and no `/api` request on a switch, and no console line or CSP report. A reload in Russian paints Russian first.

## Out of scope

- The transport messages in `src/api/http.ts`: the UI writes them, but they show as error text under `lang="en"`, like the daemon's. The mocks' data.
- Served text, spec content and a pack's vocabulary (RM-0061); a third language; a lazy English catalog or per-screen catalogs; palette matches across languages; RTL.
- **Writer-studio follow-up 1, the dash order.** `spec-flow-layout.md` "Forms" keeps the limit. The requested fix, the own types first, would name link types, which `policy.test.ts` forbids in app code (flow-canvas AC-07). A stable order needs the project-wide type set, and only Top's read (one more read per scope) or the daemon (a code change) holds it. It belongs to a Flow slice after the studio trial, if the trial shows it matters (Q3).
- Any daemon, CLI, MCP, crate, package or `.claude/` change.

## Implementation

Filled in after implementation.
