---
class: spec
status: shipped
shipped: 2026-11-09
scope: [ui, crates/specengine-eval]
ref: RM-0082; v4 step 1b; the owner's decision 2 of 2026-11-08 and his answers to Q1 and Q2 of 2026-11-09 (08 s2 Phase 7, "v4 scope"); Q3 a working answer, his review open
adrs: [ADR-0072]
---

# ui-languages: the chrome in English and Russian, one catalog, letter keys under any layout (RM-0082)

## Why

The owner's decision 2 of 2026-11-08: the UI in Russian and English, switchable; his writers work in Russian. His answers of 2026-11-09: Q1, Russian text only in the UI's catalog, keys, comments, tests and docs English; Q2, "use any good and reliable library". ADR-0072 records both. Strings sat in their views, and letter hotkeys compared `event.key`, dead under a Russian layout. This slice laid the catalog, the choice, the switcher and the key rule, and converted the shell, nav, palette, Home, `src/ui/`, the six `labels.ts` and 1a's strings; 2b (RM-0084) converts Flow, the sweep (RM-0090) the rest.

Decided: **D1** the chrome only (Q3); **D2** one catalog, `ui/src/i18n/`, `en.ts` the key source, `ru.ts` typed against it and the one new Cyrillic file (Q1); **D3** i18next with react-i18next, imported only there (Q2); **D4** both catalogs in the main chunk, loaded before the first render; **D5** no string resolved at module load; **D6** the choice per browser (`specengine.language`), a first visit following the browser; **D7** a Language select in the shell's header, each option its own name; **D8** `<html lang>` follows, the daemon's text carries `lang="en"`; **D9** counts by i18next's plural forms; **D10** a stamp's digits from `Intl`, its order from the catalog; **D11** a letter hotkey compares `letterOf(event)`: `key` when one ASCII letter, the `KeyA`-`KeyZ` code only when `key` is one non-ASCII character, else none (amended by review: ASCII punctuation and named keys give none); **D12** English byte-equal; **D13** tests reach Russian through `ru.ts` or `\u` escapes; **D14** the ui-developer writes the Russian, the owner reviews it.

## Description and interactions

How it works now: `docs/canon/ui-languages.md`. A Language select beside Workspace switches the converted chrome at once; an unconverted screen keeps English chrome; served text stays English. The daemon, CLI and MCP are unchanged and a switch reads nothing.

## Data

`docs/canon/ui-languages.md` "Catalog", "Choice", "Dates and counts", "Keys".

### The catalog, `ui/src/i18n/`

`en.ts`, `ru.ts`, `catalog.ts`, `index.ts`, `i18next.d.ts`, `fill.ts`; key and value grammar: canon "Catalog". `workspace/text.ts` became the `workspace` group and was removed; `sections.ts` gives `sectionLabel(id, t)`, `aboutOf(id, t)`; each `labels.ts` function takes a last `t`.

### The instance (`index.ts`)

`useTranslate`, `useLanguage`, `startLanguage`, `LANGUAGE_KEY`, `switchLanguage`, `translateIn`; its options: canon "Catalog". Test helpers: `src/test/text.ts` `enT`, `ruT`.

### The choice

Canon "Choice": `localStorage` `specengine.language`, else the browser's first language.

### Plurals and dates

Canon "Dates and counts": `formatUtc(stored, t)`, `formatAge(stored, now, t)`.

### Keys (`src/ui/keys.ts`)

`letterOf(event: Pick<KeyboardEvent, "key" | "code" | "shiftKey">): string | null` and `hotkeyOf`, as D11; the callers: canon "Keys".

### Dependencies

| Package | List | Pin | Published | Licence | Peers |
|---|---|---|---|---|---|
| `i18next` | `dependencies` | 26.5.11 | 2026-10-25 | MIT | `typescript ^5 \|\| ^6 \|\| ^7`, optional |
| `react-i18next` | `dependencies` | 17.1.9 | 2026-10-25 | MIT | `react >= 16.8.0`, `i18next >= 26.2.0` |

Pinned 2026-11-09, 15 days old; no override, no `peerDependencyRules`, no install script. Lockfile-only, all MIT: `html-parse-stringify` 4.0.1 (new; it no longer depends on `void-elements`, which this spec named wrongly), `@babel/runtime` 7.29.7 and `use-sync-external-store` 1.7.0 (already present).

## Rules and edge cases

Canon "Served text" and "Switcher": served text byte for byte, interpolated data as text, a switch issuing no request and keeping focus and state, a missing key falling back to English silently, an unknown value raw. The language is a lens: it changes no right, gate, finding or exit (ADR-0012). Russian runs longer: the header wraps, no sideways scroll at 320 px (L2).

## Acceptance criteria

Tests in `ui/src` unless named; mutations M1-M28.

- [x] **LG-1 Catalog parity.** `src/i18n/catalog.test.ts`: keys in both but plural forms; plural groups `_one`/`_other` in `en`, exactly `_one`/`_few`/`_many`/`_other` in `ru`; equal placeholder sets; `{{count}}` in plural forms alone; no empty value, markup, `$t(` or stray `{{`. `pnpm build` fails on a Russian key missing or added. M1 a `ru` key deleted, M2 a placeholder renamed, M3 a `_many` dropped, M4 `{{count}}` outside a plural.
- [x] **LG-2 The switch.** `src/app/language.test.tsx`: choosing Russian sets `lang` and the key to `ru` and shows `ruT`'s nav, palette options, Home titles, Workspace label and a badge per `labels.ts`; no client call, focus on the select; a fresh render starts in Russian, with a throwing `setItem` in English; English restores all. M5 `lang` left `en`, M6 `key={language}` on the shell, M7 the choice in state only, M8 the language in a query key.
- [x] **LG-3 Labels and sections.** Each closed table of the six `labels.ts` and the nine sections under `ruT` equal `ru`'s values. M9 a label left a literal.
- [x] **LG-4 Counts and dates.** `src/i18n/plural.test.ts`: Russian `one` at 1, 21, 101, `few` at 2-4, 22, `many` at 0, 5, 11, 12, 14, 25, 111; English `one` at 1. `src/ui/time.test.ts`: `2026-10-05 21:14 UTC`, Russian `05.10.2026 21:14 UTC`, `00:05Z` hour `00`, `formatAge` forms at 1, 3, 5, 21 min, unparsable raw. M10 `count === 1`, M11 `toLocaleString()`, M12 `hour12: false`.
- [x] **LG-5 Served text verbatim.** In Russian, `ErrorPanel`'s message byte for byte under `lang="en"` with a Russian title; Home's and the config notes verbatim; `a&b <c> {{n}} $t(x)` as written; an unknown status raw. M13 the message through `t`, M14 `lang` dropped, M15 `escapeValue: true`.
- [x] **LG-6 English the default, unchanged.** `["en-US", "en"]` English, nothing stored; `["ru-RU", "en"]` Russian; `["uk", "ru"]` English; empty `languages`, `language` `ru`, Russian; a stored `ru-RU` is none. No base test's expected English changed. M16 Russian the default, M17 `Spec tree` edited in `en.ts`.
- [x] **LG-7 Letter keys under any layout.** `src/ui/keys.test.ts`: `j`, Shift `J`; Russian `KeyJ` gives `j`/`J`; Dvorak `{key: "h", code: "KeyJ"}` gives `h`; `?`, `1` and `{key: "\u0441", code: "Unidentified"}` give `null`; AZERTY's `,` on `KeyM` and `Dead` give `null`. Screen tests under a Russian layout: `c` centres Flow, `n` New card, `m` Move, `j` down in the Spec tree, Tasks, Inbox, Health and Graph; `?` lists; `chord.test.ts` unchanged. `policy.test.ts`: no `.key` against a one-letter literal or `case`. M18 `event.key === "c"` in `FlowCanvas.tsx`, M19 `code` first, M20 a `case "x":`.
- [x] **LG-8 Cyrillic only in `ru.ts`.** `crates/specengine-eval/tests/anonymity.rs`: the exemption is exactly `fixtures/spec-b`, `fixtures/token-calibration` and the file `ui/src/i18n/ru.ts`; `index.ts`, `ru/x.ts`, `ru.tsx` scanned. M21 Cyrillic in `Shell.tsx`, M22 the exemption widened to `ui/src/i18n/`, M23 Cyrillic in a test.
- [x] **LG-9 Typed keys.** A `// @ts-expect-error` `t("app.nope")` in `catalog.test.ts`. M24 `CustomTypeOptions` removed.
- [x] **LG-10 Converted directories hold no literal.** `policy.test.ts` over `src/app/`, `palette/`, `overview/`, `ui/`, `workspace/` (2b adds `src/flow/`, RM-0090 the rest): no JSX text with an ASCII letter, no literal in `aria-label`, `aria-description`, `aria-roledescription`, `aria-valuetext`, `title`, `placeholder`, `alt` or a `TEXT_PROPS` prop. M25 `Mock data` restored in `DataSource.tsx`.
- [x] **LG-11 The library's place and pins.** 19 packages, the two exact in `dependencies`, at least 14 days old (a hand check: "Dependencies"); `pnpm install --frozen-lockfile` with strict peers; `ui_policy.rs`'s `ALLOWLIST` the 19, the README table naming them; only `ui/src/i18n/` imports either (ESLint, `ui_policy.rs`). M26 `react-i18next` in `Shell.tsx`, M27 a `^`, M28 a 20th package.
- [x] **LG-12 The cost, measured.** `index-*.js` +98.99 kB minified, under the 120 kB cap; Russian only in `index-*.js`.
- [x] **LG-form.** `pnpm lint`, `pnpm build`, `pnpm test`; `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers`, `ui_policy` last; `spec check` 0 errors, 0 warnings.
- [ ] **L1** `spec serve` over a scratch `fixtures/card-press`: Russian, English, reload; no console line or CSP report, no request on a switch, the choice kept.
- [ ] **L2** Russian at 1440x900 and 320x640, Home and the palette: `scrollWidth <= innerWidth` at 320, the header wraps, nothing cut unseen.

## Out of scope

Translating served text, spec content or a pack's vocabulary (RM-0061); a hub's per-login language; a third language; RTL, number grouping, `Intl.RelativeTimeFormat`; cross-tab sync, lazy catalogs, ICU, `Trans`; the screens left to 2b and RM-0090; a UI licence check.

## Implementation

Review ACCEPT, its minors fixed. UI vitest 3676/3676 twice, lint and build clean; eval `ui_policy`, `anonymity`, `doc_pointers`, `workspace_names` green, `ui_policy` last.

**Bundle (LG-12).** `index-*.js` 615.81 kB (gzip 175.20) at the base, 714.80 kB (gzip 204.73) after.

**Spec defects corrected.** `CATALOGS` is `Record<Language, Messages>`: `Record<Language, Catalog>` cannot type-check, `en` lacking `_few`/`_many`, which `Catalog` keeps required for `ru`. The hotkey list missed the Inbox's `a`/`r`/`c`/`d` and Graph's and Flow's `o`/`c`, all on `letterOf` now. D11 amended by review, as above: AZERTY's `,` on `KeyM` no longer opens Move, Dvorak keeps its letters. `void-elements` ("Dependencies").

**Departures, for the owner's review.** (1) Key caps: in `ru.ts` the arrows and Space are Russian words (the shortcut list's `app.keys.{down,up,left,right,space}`, the palette hint's `palette.keys.{up,down}`), against "Latin caps in both languages"; Home, End, Enter, Esc, Cmd-K, Ctrl-K and letters stay Latin. The orchestrator's call, for Russian readers. (2) `RootFailure` has no Language select: D7's "always shown" holds for the shell. (3) A stamp's year is not padded to 4 digits: out of range, accepted.

**Additions.** `fill.ts` `fill`/`slot`: an element placed in a translated sentence as a React node, never markup. `switchLanguage` (the select's `choose` after storing; stores nothing) and `translateIn` for the select and tests only; `hotkeyOf`. Served text carries `lang="en"` per item, not on a list whose label is translated (D8). Also converted: `tasks/groups.ts` (Home shows the groups), `main.tsx`'s mock lines, `ui/DiffView.tsx`. `src/test/setup.ts` imports `../i18n` dynamically in `beforeEach`, so a file's `vi.mock` of a catalog applies; the missing-key fallback test is `src/i18n/fallback.test.tsx`, alone since `vi.mock` is per file.

**Mutations.** M22 run for real on `anonymity.rs`: two tests red. M21, M23 and M26-M28 run on every run as built-in controls on scratch trees. LG-11's 14-day age stays a hand check; L1 and L2 are open for the owner.

**Open for the owner.** Review the Russian in `ui/src/i18n/ru.ts`: a value edit is a plain UI change, no ADR, breaking no test. Apply to `.claude/agents/ui-developer.md` its "Language" paragraph:

> **Language.** The chrome is English or Russian, each viewer's choice (ADR-0072, `docs/canon/ui-languages.md`). Every string the UI writes is a key of `ui/src/i18n/`: English in `en.ts` is the key source; Russian in `ru.ts`, the only file where Cyrillic may stand. Keys, comments and tests stay English; a test reaches Russian through `ru.ts` or `\u` escapes. Only `src/i18n/` imports i18next. Spec content and the daemon's text are shown as is and may be in any script, including Cyrillic: fonts, search, line breaking and column widths must handle non-Latin text.

| Module | What it does |
|---|---|
| `ui/src/i18n/`: `en.ts`, `ru.ts`; `catalog.ts`, `i18next.d.ts`; `index.ts`; `fill.ts` | the catalogs; their types and typed keys; the instance and choice; elements in a sentence |
| `app/LanguageSwitcher.tsx`, `Shell.tsx`, `styles/app.css` | the select |
| `ui/keys.ts`; `SpecTree`, `SearchResults`, `TaskList`, `InboxView`, `FindingsRegion`, `GraphCanvas`, `BoardView`, `FlowCanvas`, `chord.ts` | `letterOf`, `hotkeyOf`; their callers |
| `ui/time.ts`, six `labels.ts`, `app/sections.ts`, `tasks/groups.ts` | take `t` |
| `app/`, `palette/`, `overview/`, `ui/`, `workspace/` (`text.ts` removed), `main.tsx`; `roadmap/`, `round/`, `trace/`, `markdown/` | converted; callers pass `t` |
| `package.json`, `pnpm-lock.yaml`, `eslint.config.js` | the pins; the import seam |
| tests | `i18n/*.test.*`, `app/language.test.tsx`, `ui/keys.test.ts`, `policy.test.ts`, `test/{setup,text}.ts`, labels, time and screen tests; eval `anonymity.rs`, `ui_policy.rs` |
