---
class: spec
status: draft
scope: [ui]
ref: RM-0082; v4 step 1b; the owner's decision 2 of 2026-11-08 and his answers to Q1 and Q2 of 2026-11-09 (08 s2 Phase 7, "v4 scope"); Q3 a working answer, his review open
adrs: [ADR-0072]
---

# ui-languages: the chrome in English and Russian, one catalog, letter keys under any layout (RM-0082)

## Why

### Requirement

The owner's decision 2 of 2026-11-08: the UI in Russian and English, switchable; his writers work in Russian. His answers of 2026-11-09: Q1, Russian text in the repository only in the UI's translation catalog, keys, comments, tests and docs English; Q2, "use any good and reliable library". ADR-0072 records both, superseding the English-only interface.

Today every string sits in the view that shows it, but 1a's, which `WORKSPACE_TEXT` gathered for this slice (`docs/canon/workspaces.md` "Screen"). Letter hotkeys compare `event.key`, so under a Russian layout `j`, `k`, `m`, `n` and `c` do nothing; only `chord.ts` falls back to the physical key, for Cmd-K.

This slice lays the catalog, the choice, the switcher and the key rule. It converts the shell, the nav, the palette, Home, the shared `src/ui/` parts, the six `labels.ts` and 1a's strings. 2b (RM-0084) follows and converts Flow; the sweep (RM-0090) converts every screen left before the studio trial.

### Decisions

- **D1 The chrome only.** The chrome is every word the UI writes itself. Spec content and anything the daemon, the CLI or MCP wrote is shown as served (Q3, a working answer).
- **D2 One catalog,** `ui/src/i18n/`. `en.ts` is the source of the keys; `ru.ts` is typed against it. `ru.ts` is the repository's one new file holding Cyrillic (Q1).
- **D3 The library** is i18next with react-i18next, the orchestrator's pick (Q2, ADR-0072's "Why"). Only `src/i18n/` imports them, as only `src/markdown/` imports the markdown packages (ADR-0036), so changing the library touches one directory.
- **D4 Both catalogs ship in the main chunk,** loaded before the first render: no flash of English, no request, no load to fail. The cost is measured (LG-12).
- **D5 No string is resolved at module load.** A component reads `t` from `useTranslate()`; any other module takes a `Translate` argument. A switch then re-renders what shows text, remounts nothing and keeps focus and state.
- **D6 The choice is the viewer's,** one per browser for every project, local or hub (`specengine.language`); a first visit follows the browser's first language.
- **D7 The switcher** is a Language select in the header, always shown. Each option is a language's own name.
- **D8 `<html lang>`** follows the choice. An element showing served text carries `lang="en"`.
- **D9 Counts** use i18next's plural forms. `{{count}}` appears in plural keys only.
- **D10 Dates.** A stamp's digits come from `Intl.DateTimeFormat` and its order from the catalog. The English stamp stays byte-equal to today's.
- **D11 A letter hotkey compares `letterOf(event)`**: `chord.ts`'s rule, extended to every letter. `key` is used when it is an ASCII letter, else the `KeyA`-`KeyZ` code. Under a Russian layout the physical key acts, and a Dvorak layout keeps its own letters.
- **D12 English stays byte-equal.** Each English value equals today's string, so every existing test keeps its expected text.
- **D13 Tests reach Russian through `ru.ts`** or `\u` escapes, never a Cyrillic literal: Q1's "tests stay English", which `anonymity.rs` already enforces. An owner's edit to a Russian value breaks no test.
- **D14 The ui-developer writes the Russian and the owner reviews it.** The review is an open item, not a blocker.

### Assumptions

- **LA1** The pinned i18next resolves plurals through `Intl.PluralRules`: `_one`, `_few`, `_many`, `_other` for `ru`; `_one`, `_other` for `en`.
- **LA2** react-i18next's `useTranslation` re-renders its component on `languageChanged` and hands back a new `t`, so a `useMemo` taking `t` recomputes.
- **LA3** There is a pair of stable versions, each at least 14 days old, whose peers admit `react` 19.3.0 and `typescript` 6.0.3 under `strict-peer-dependencies=true`. If there is none, the slice stops and asks the owner: no override and no `peerDependencyRules`.
- **LA4** i18next needs no `eval`, so the page's `default-src 'self'` policy holds (`docs/canon/serve-ui.md` "Routes").
- **LA5** jsdom's `navigator.languages` is `["en-US", "en"]`.

## Description and interactions

**What a viewer sees.** A Language select beside Workspace: English, and Russian under its own name. Choosing Russian switches the converted chrome at once; a screen not yet converted keeps English chrome around Russian labels; refusals, notes, gap causes and commands stay English. The choice holds for the browser until changed.

**Converted now:**

| Where | What |
|---|---|
| `src/app/` | the shell (brand, project select, Jump to, Keyboard shortcuts, the nav and its wait); the sections' labels and abouts (`sections.ts`); the shortcut list's words (key caps stay Latin); "Mock data"; the error boundaries; the 501 words |
| `src/palette/` | the dialog, its placeholder and list label, the option words, the failure lines' region names |
| `src/overview/` | Home: region titles, links, counts, skeleton and empty words |
| `src/ui/` | `states.tsx` (Retry, the skeleton's and error panel's words), `Dialog.tsx`, `CopyButton.tsx`, `Badge.tsx`'s spoken prefix, `Tabs.tsx`, `announcer.tsx`, `time.ts` |
| `src/workspace/` | `WORKSPACE_TEXT` (below) |
| six `labels.ts` | board, flow, health, inbox, tasks, tree: every word they export; tones, icons, order and the raw fallback stay |
| every screen | letter hotkeys by `letterOf` (D11) |

**Not now.** Each screen's own strings in `src/inbox/`, `tasks/`, `board/` (the hub's "Acting as" too), `roadmap/` (`text.ts`), `tree/`, `graph/`, `health/`, `round/`, `trace/` and `markdown/` stay English. 2b converts `src/flow/`, and the sweep RM-0090 converts the rest before the studio trial. A slice that touches a screen keeps its new strings in the catalog.

**Existing string modules.**

| Module | Becomes |
|---|---|
| `workspace/text.ts` `WORKSPACE_TEXT` | `workspace.*`, same names. `notesButton(n)` is `notesButton` with `{{n}}`; `flowsShown(shown, all)` is plural `flowsShown` with `{{shown}}` and `{{count}}` (all). The file is removed |
| `app/sections.ts` | `app.sections.<id>.label` and `.about`. `SECTIONS` keeps its `id: "…"` entries in order (`ui_policy.rs` reads them); `sectionLabel(id, t)` and `aboutOf(id, t)` |
| six `labels.ts` | `<dir>.*` keys; each function gains a last `t: Translate` (`taskStatusLook(status, t)`), and a `Look` or line constant becomes a function of `t` (`SPEC_CHANGED` becomes `specChanged(t)`) |
| `ui/time.ts` | `ui.time.*`: `formatAge(stored, now, t)`, `formatUtc(stored, t)`. Their callers in Board, Inbox, Tasks and Tree pass `t`, so those dates follow the choice now |
| `round/text.ts`, `ui/text.ts` | unchanged: a command, limits and escaping, no words (`roadmap/text.ts`: RM-0090) |

**Interactions.** The daemon, CLI and MCP are unchanged, and a switch reads nothing. 1a's Workspace select, module bar and notes keep their behaviour; workspace and module titles are data. On a hub the choice is the browser's, never the login's. The `?` list names keys by their Latin caps in both languages. `aria-keyshortcuts` values stay as they are.

## Data

### The catalog, `ui/src/i18n/`

| File | Holds |
|---|---|
| `en.ts` | `export const en = {…} as const`: the key source, the English values, a comment per group |
| `ru.ts` | `export const ru: Catalog = {…}`: the Russian values, the repository's only Cyrillic outside the two fixture corpora `anonymity.rs` exempts |
| `catalog.ts` | `Language = "en" \| "ru"`, `LANGUAGES = ["en", "ru"]`, `isLanguage`, `CATALOGS: Record<Language, Catalog>`, and the type `Catalog`: `en`'s shape with every leaf `string`, and every plural group gaining `_few` and `_many` |
| `index.ts` | the one i18next instance, `useTranslate`, `useLanguage`, `startLanguage`, `LANGUAGE_KEY` |
| `i18next.d.ts` | `CustomTypeOptions`: `defaultNS: "translation"`, `resources: {translation: typeof en}` |

**Keys.** A path of segments `[a-z][A-Za-z0-9_]*` joined by `.`; the first segment is a `src/` directory (`app`, `palette`, `home` for `overview/`, `ui`, `workspace`, `board`, `flow`, `health`, `inbox`, `tasks`, `tree`; later `roadmap`, `round`, `graph`, `trace`, `markdown`) or `language`. Only a plural form ends in `_one`, `_few`, `_many` or `_other`; no `_zero`, `_two` or ordinal key. Keys are English (Q1).

**Values.** Plain text. Placeholders are `{{name}}`, with `name` matching `[a-z][A-Za-z0-9]*`; `{{count}}` appears in a plural form alone, and in every form of it. A value holds no markup (`<` before a letter or `/`), no `$t(` nesting and no `{{…, format}}`. A command to copy is code, never a catalog value. A key cap (`Esc`, `Enter`, `Ctrl K`, the `⌘` sign) is a value, the same in both catalogs.

```ts
// en.ts: the source of the keys (values byte-equal to today's strings)
export const en = {
  language: { label: "Language", self: "English" },
  app: {
    sections: {
      inbox: { label: "Inbox", about: "Proposals waiting for your decision." },
      // … the nine, in sections.ts's order
    },
    mockData: "Mock data",
    jumpTo: "Jump to",
  },
  ui: {
    retry: "Retry",
    time: {
      justNow: "just now",
      minutesAgo_one: "{{count}} min ago",
      minutesAgo_other: "{{count}} min ago",
      stamp: "{{year}}-{{month}}-{{day}} {{hour}}:{{minute}} UTC",
    },
  },
  workspace: {
    workspaceLabel: "Workspace",
    notesButton: "Config notes ({{n}})",
    flowsShown_one: "Showing {{shown}} of {{count}} card flows.",
    flowsShown_other: "Showing {{shown}} of {{count}} card flows.",
  },
  tasks: { status: { draft: "Draft", in_review: "Work in review" } },
} as const;

// ru.ts: the same paths; every plural group has _one, _few, _many, _other
export const ru: Catalog = {
  language: { label: "…", self: "…" },            // self: Russian's own name
  ui: { time: { stamp: "{{day}}.{{month}}.{{year}} {{hour}}:{{minute}} UTC", /* … */ } },
  workspace: { flowsShown_one: "…{{shown}}…{{count}}…", flowsShown_few: "…", flowsShown_many: "…", flowsShown_other: "…" },
  // …
};
```

"…" stands for Russian: `anonymity.rs` scans `docs/`.

### The instance (`index.ts`)

`i18next.use(initReactI18next).init`, run once, synchronously, before `createRoot` renders, with:
- both resources under `translation`, `lng: startLanguage()`, `fallbackLng: "en"`, `supportedLngs: ["en", "ru"]`;
- `keySeparator: "."`, `nsSeparator: false`;
- `interpolation: {escapeValue: false, skipOnVariables: true}`: React escapes, and a placeholder's value is never read for `{{` or `$t(`;
- `returnNull: false`, `returnEmptyString: false`, `returnObjects: false`;
- `react: {useSuspense: false}`, plus the pinned version's synchronous-init option (`initAsync: false`, or `initImmediate: false` before it);
- no debug, no `saveMissing`, no support notice and no plugin but `initReactI18next`: no backend, no detector, no `Trans`, no `withTranslation`.

```ts
export type Language = "en" | "ru";
export type Translate = TFunction;                     // i18next's, typed by the catalog
export const LANGUAGE_KEY = "specengine.language";
export function useTranslate(): Translate;             // useTranslation().t
export function useLanguage(): { language: Language; choose(next: Language): void };
export function startLanguage(): Language;             // stored, else browser, else "en"
```

`choose` stores the language (a refused write is ignored) and calls `changeLanguage`; one `languageChanged` listener sets `<html lang>`, which `startLanguage` sets before the first render. A class component, such as an error boundary, takes `t` as a prop from a function wrapper. Test helpers in `src/test/text.ts` export `enT` and `ruT` (fixed `t`s). `src/test/setup.ts` returns the instance and `<html lang>` to English before each test.

### The choice

| `localStorage` `specengine.language` | Start |
|---|---|
| `en` or `ru` | that language |
| absent, any other value, or a read that throws | the browser's first language: `navigator.languages[0]`, else `navigator.language`. Its primary subtag, before the first `-` and lower-cased, equal to `ru` gives Russian; anything else, an empty value included, gives English. Nothing is stored |

Only a choice in the select writes the key. A write that throws keeps the choice until a reload. Another tab's change is not heard; it applies on the next load.

### Plurals and dates

- **Plurals.** `t(key, {count})` picks the form by `Intl.PluralRules`. In Russian, 1, 21 and 101 are `one`; 2-4, 22 and 34 are `few`; 0, 5-20, 25 and 111 are `many`. In English, 1 is `one` and everything else `other`. A number prints as `String(n)`, ungrouped in both languages.
- **Dates.** `formatUtc(stored, t)` takes `year`, `month`, `day`, `hour` and `minute` from `new Intl.DateTimeFormat("en-US", {timeZone: "UTC", year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hourCycle: "h23"}).formatToParts(at)`. It fills them into `ui.time.stamp`; `hour12: false` is never used, since it gives `24` at midnight on some engines. English stays `2026-10-05 21:14 UTC`, Russian is `05.10.2026 21:14 UTC`, and an unparsable value is shown raw. `formatAge` keeps its thresholds; its words are `ui.time.justNow` and the plural `minutesAgo`, `hoursAgo` and `daysAgo`.

### Keys (`src/ui/keys.ts`)

```ts
/** The letter a Latin layout gives: `key` when one ASCII letter; else `KeyA`..`KeyZ`'s letter, upper case with Shift; else null. */
export function letterOf(event: Pick<KeyboardEvent, "key" | "code" | "shiftKey">): string | null;
```

The letter hotkeys today: `j` and `k` in `tree/SpecTree.tsx`, `tree/SearchResults.tsx`, `tasks/TaskList.tsx`, `inbox/InboxView.tsx`, `health/FindingsRegion.tsx` and `graph/GraphCanvas.tsx`; `m` and `n` in `board/BoardView.tsx`; `c` in `flow/FlowCanvas.tsx`. Each compares `letterOf(event)`. `?`, Escape, Enter, Space, the arrows, Home and End stay on `key`. `chord.ts`'s `isJumpChord` keeps its rule and may call `letterOf`.

### Dependencies

| Package | List | Pin | Licence |
|---|---|---|---|
| `i18next` | `dependencies` | the newest stable at least 14 days old on the pin day (`npm view i18next time`) | MIT |
| `react-i18next` | `dependencies` | the same, its `i18next` peer admitting the pin | MIT |

Their dependencies stay lockfile-only (`@babel/runtime`; `html-parse-stringify`, `void-elements` if still declared), each licence recorded in "Implementation"; no install script. `minimumReleaseAge` stays 7 days: the owner's 14 for these two is checked by hand. No UI licence check exists (`build_graph.rs` pins Rust crates only), and none is added.

## Rules and edge cases

- WHEN the chrome is Russian, the system SHALL show every served text byte for byte: the daemon's `message`, notes, `reason`, a gap's `cause`, config notes, a check verdict's words, commands, IDs, paths, an unknown status raw. It SHALL also show spec content, project names and workspace and module titles as served. The element holding served text carries `lang="en"` when its text is the daemon's; spec content carries none, since its language is the project's.
- WHEN a value interpolates data (`{{project}}`), the data SHALL be text: `a&b <c> {{n}} $t(x)` shows as written, in either language.
- WHEN the language changes, the system SHALL issue no request, remount no view, keep focus on the select, keep every view's state and every query's data, and set `<html lang>`.
- WHEN a key has no Russian value at run time (impossible past `tsc`, LG-1), i18next SHALL fall back to English and write nothing to the console.
- WHEN a known value's label is shown, it SHALL be the catalog's; an unknown value is its raw text in either language (`ui/README.md` "Screen rules", Status).
- WHEN a hotkey's letter is read, the system SHALL use `letterOf`, never `event.key` against a letter or `event.code` alone.
- Filters keep comparing `normalize("NFC").toLowerCase()`; sort collators stay `en`, since they sort data, not chrome.
- Russian runs longer: the header and nav wrap, nothing is cut that cannot be seen whole, and there is no sideways scroll at 320 px (`ui/README.md` "Screen rules", Text).
- The language is a lens, as a workspace is: it changes no right, gate, finding or exit (ADR-0012).

## Acceptance criteria

Tests in `ui/src` unless named. "In Russian" means after choosing Russian in the select. Mutations are numbered M1 to M28.

- [ ] **LG-1 Catalog parity.** `src/i18n/catalog.test.ts`, over the flattened `en` and `ru`:
  - every key but a plural form is in both;
  - every plural group is `_one` and `_other` in `en`, and exactly `_one`, `_few`, `_many` and `_other` in `ru`;
  - every value's placeholder set equals its English one (a plural form's equals `en`'s `_other`; in `en`, `_one`'s equals `_other`'s);
  - `{{count}}` stands in every plural form and nowhere else;
  - no value is empty, holds markup or `$t(`, or holds a `{{` that does not open a placeholder.

  `pnpm build` fails on a Russian key missing or added. M1 a `ru` key deleted. M2 a `ru` placeholder renamed. M3 a `ru` `_many` dropped. M4 `{{count}}` in a key that is not plural.
- [ ] **LG-2 The switch.** `src/app/language.test.tsx`, over the stub client:
  - Choosing Russian sets `document.documentElement.lang` to `ru` and `specengine.language` to `ru`. The nav (Overview and the workspace's sections), the palette's section options, Home's region titles, the Workspace select's label and one known badge per `labels.ts` then show their `ruT` values.
  - The stub client sees no call from the switch, and `document.activeElement` is still the select.
  - A fresh render, as after a reload, starts in Russian. With `setItem` throwing, the switch still shows Russian, and a fresh render is English.
  - Choosing English restores every string checked above to its base text.

  M5 `<html lang>` left `en`. M6 `key={language}` on the shell (focus lost). M7 the choice in component state only. M8 the language in a query key.
- [ ] **LG-3 Labels and sections.** For each closed table of the six `labels.ts`, every known value's label under `ruT` equals `ru`'s value at its key; so does each of the nine sections' label and about. M9 one label left a string literal.
- [ ] **LG-4 Counts and dates.**
  - `src/i18n/plural.test.ts`: in Russian, `workspace.flowsShown` takes `_one` at 1, 21 and 101, `_few` at 2, 3, 4 and 22, and `_many` at 0, 5, 11, 12, 14, 25 and 111. In English it takes `_one` at 1 and `_other` at 0, 2 and 21.
  - `src/ui/time.test.ts`, its base cases unchanged but for `t`: `formatUtc("2026-10-05T21:14:03Z", enT)` is `2026-10-05 21:14 UTC`; with `ruT` it is the `ru` stamp filled, `05.10.2026 21:14 UTC`; `00:05Z` gives hour `00`. `formatAge` at 1, 3, 5 and 21 minutes takes `one`, `few`, `many` and `one` in Russian. An unparsable value is raw in both.

  M10 `count === 1 ? one : other`. M11 a stamp from `toLocaleString()`. M12 `hour12: false`.
- [ ] **LG-5 Served text verbatim.** In Russian:
  - `ErrorPanel` shows a stub daemon's `message` byte for byte, its element carrying `lang="en"`, while its title and Retry are Russian;
  - Home's daemon notes and the workspace config notes are verbatim with `lang="en"`;
  - a project name `a&b <c> {{n}} $t(x)` in Home's skeleton label shows as written;
  - an unknown task status is raw.

  M13 the message passed through `t`. M14 `lang="en"` dropped from the message. M15 `escapeValue: true` (it shows `&amp;`).
- [ ] **LG-6 English is the default and unchanged.**
  - With no stored choice and the browser at `["en-US", "en"]`, the UI is English with `lang="en"` and nothing stored.
  - `["ru-RU", "en"]` gives Russian, nothing stored. `["uk", "ru"]` gives English. Empty `languages` with `language` `ru` gives Russian. A stored `ru-RU` counts as none.
  - Every UI test of the base passes. In `git diff <base> -- 'ui/src/**/*.test.ts' 'ui/src/**/*.test.tsx'`, no expected English string changes: a changed line only hands a `t` or reaches a string through `en`.

  M16 Russian as the default. M17 an English value edited in `en.ts` (`Spec tree` to `Tree`), which turns a base test red.
- [ ] **LG-7 Letter keys under any layout.**
  - `src/ui/keys.test.ts`, for `letterOf`:
    - `{key: "j"}` gives `j`; `{key: "J", shiftKey}` gives `J`;
    - Russian `{key: "\u043e", code: "KeyJ"}` gives `j`, and with Shift `J`;
    - Dvorak `{key: "h", code: "KeyJ"}` gives `h`;
    - `{key: "?", code: "Digit7", shiftKey}`, `{key: "1", code: "Digit1"}` and `{key: "\u0441", code: "Unidentified"}` give `null`.
  - Screen tests with Russian-layout events:
    - `{key: "\u0441", code: "KeyC"}` centres the Flow canvas;
    - `{key: "\u0442", code: "KeyN"}` opens New card on the Board;
    - `{key: "\u044c", code: "KeyM"}` opens a card's move menu;
    - `{key: "\u043e", code: "KeyJ"}` moves down in the Spec tree, Tasks, Inbox, Health findings and the Graph;
    - `?` still opens each list;
    - `chord.test.ts` passes unchanged.
  - `policy.test.ts`: no non-test file compares `.key` with a one-letter literal, and no `switch` on `.key` has a one-letter `case`.

  M18 `event.key === "c"` kept in `FlowCanvas.tsx`. M19 `letterOf` reading `code` first (Dvorak's `h` then acts as `j`). M20 a new `case "x":` under `switch (event.key)`.
- [ ] **LG-8 Cyrillic only in `ru.ts`.** `crates/specengine-eval/tests/anonymity.rs`: the exemption is exactly `fixtures/spec-b`, `fixtures/token-calibration` and the file `ui/src/i18n/ru.ts`, whole paths. The exemption test asserts that each of the three holds Cyrillic, and that `ui/src/i18n/index.ts`, `ui/src/i18n/ru/x.ts` and `ui/src/i18n/ru.tsx` are scanned. M21 a Cyrillic literal in `src/app/Shell.tsx`. M22 the exemption widened to `ui/src/i18n/`, with a Cyrillic letter in `index.ts`. M23 a Cyrillic literal in a test.
- [ ] **LG-9 Typed keys.** `src/i18n/catalog.test.ts` holds a `// @ts-expect-error` call `t("app.nope")`. `pnpm build`'s `tsc --noEmit` passes only while keys are typed. M24 the `CustomTypeOptions` declaration removed (the directive goes unused and `tsc` fails).
- [ ] **LG-10 Converted directories hold no literal.** `policy.test.ts`, over the `.tsx` files of a listed set (`src/app/`, `src/palette/`, `src/overview/`, `src/ui/`, `src/workspace/`; 2b adds `src/flow/`, RM-0090 the rest), tests excepted:
  - no JSX text holds an ASCII letter;
  - no string or template literal holding one is the value of `aria-label`, `aria-description`, `aria-roledescription`, `aria-valuetext`, `title`, `placeholder` or `alt`, or of a shared component's text prop the test lists (`Skeleton` `label`, `Notes` `label`, `Stored` `label`, `ErrorPanel` `title`, `Badge` `name`; a new one joins the list).

  M25 the literal `Mock data` restored in `DataSource.tsx`.
- [ ] **LG-11 The library's place and pins.**
  - `package.json` names 19 packages, `i18next` and `react-i18next` in `dependencies` at exact `x.y.z`, each published at least 14 days before the pin day; both dates are recorded in "Implementation".
  - `pnpm install --frozen-lockfile` passes with `strict-peer-dependencies=true`: no override, no `peerDependencyRules`, no install script.
  - `ui_policy.rs`'s `ALLOWLIST` is the 19, and the README table names them.
  - Only `ui/src/i18n/` imports either package: ESLint `no-restricted-imports` `^(i18next|react-i18next)(/|$)` outside it, and `ui_policy.rs` as for `src/markdown/`. Something there does import it.

  M26 `import { useTranslation } from "react-i18next"` in `Shell.tsx`. M27 a `^` on a pin. M28 a 20th package.
- [ ] **LG-12 The cost, measured.** `pnpm build` at the base commit and after: Vite's printed `index-*.js` line grows by at most 120 kB minified, and both lines, gzip included, go in "Implementation". The base already warns: `ui/dist` of 2026-11-09 holds a 615 816 B `index-*.js`, over Vite's 500 kB. `ru.ts`'s values are in `index-*.js` only. A measurement, no mutation; over the cap, the slice stops for the owner.
- [ ] **LG-form.** `pnpm lint`, `pnpm build` and `pnpm test` pass, as do `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers` and `spec check` (0 errors, 0 warnings). The `ui_policy` run comes last, after the README pass of "Canon at ship".

### Looks

By hand or a headless browser on the built UI, with real input; each dated in "Implementation".

- [ ] **L1** `spec serve` over a scratch copy of `fixtures/card-press`, which has workspaces. Choose Russian, then English, then reload. The console stays empty, with no CSP report; the network panel shows no request on a switch; the reload keeps the choice.
- [ ] **L2** In Russian at 1440x900 and 320x640, Home and the open palette: at 320, `scrollWidth <= innerWidth`; the header wraps, and no label is cut without a way to see it whole.

**Tests**: as each criterion names them; LG-2, LG-5 and LG-6 in a new `app/language.test.tsx`, the six `labels.test.ts` and `workspace/*.test.*` extended.

## Out of scope

Translating served text, spec content, project data or a pack's vocabulary (RM-0061); a hub's per-login language; a third language (a new decision); RTL, number grouping, `Intl.RelativeTimeFormat`; cross-tab sync, lazy catalogs, ICU syntax, `Trans`; palette matches across languages; the screens left to 2b and RM-0090; a UI licence check; `.claude/` (the owner's: below).

### Risks

- **R-mixed.** Until RM-0090, a converted label sits inside an English screen. Accepted: the trial waits for the sweep.
- **R-peers.** TypeScript 6 against an optional `typescript` peer of `^5` fails a strict install (LA3). The ui-developer checks `npm view <pkg>@<v> peerDependencies` before pinning.
- **R-size.** Every viewer pays for both catalogs (D4), growing with each screen converted. Past LG-12's cap at a later slice, Russian moves to a lazy chunk, at a first-render wait.
- **R-review.** The owner reviews Russian he did not write; until he does, a writer may meet a clumsy word. Not a blocker.

### The owner's review (open, not a blocker)

The owner reads `ui/src/i18n/ru.ts`, or the UI in Russian, and edits values: a plain UI change, through the pipeline or on his own instruction, with no ADR. Tests read the catalog, so a reworded value breaks none; LG-1 guards the placeholders.

### For the owner: `.claude/agents/ui-developer.md`

This slice's exception (ADR-0033): the ui-developer writes Russian values into `ui/src/i18n/ru.ts`. Once it ships, the role's "Language" paragraph reads:

> **Language.** The chrome is English or Russian, each viewer's choice (ADR-0072, `docs/canon/ui-languages.md`). Every string the UI writes is a key of `ui/src/i18n/`: English in `en.ts` is the key source; Russian in `ru.ts`, the only file where Cyrillic may stand. Keys, comments and tests stay English; a test reaches Russian through `ru.ts` or `\u` escapes. Only `src/i18n/` imports i18next. Spec content and the daemon's text are shown as is and may be in any script, including Cyrillic: fonts, search, line breaking and column widths must handle non-Latin text.

## Canon at ship

Each file touched moves `reviewed:` to the ship date. Byte counts are today's.

- **`docs/canon/ui-languages.md`** is rewritten whole, at most 6 144 B (1 645 today).
  - "State" goes, and "The rule" stays under its anchor.
  - New sections: "Catalog" (files, key and value grammar, plurals, the instance's options), "Choice" (the key, first visit, a refused store, no cross-tab sync), "Switcher", "Served text" (verbatim, `lang="en"`), "Keys" (`letterOf`), "Dates and counts", "Converted" (LG-10's list, each later slice adding its own), "Guards" (LG-1, LG-7 to LG-11), "Known limits" (R-mixed, R-size, untagged spec content, one choice per browser, the owner's review), "Tests".
  - Its index line stays in `docs/index-canon.md`.
- **`ui/README.md`**, net -1 to -7 B by the pins' length, at most 8 189 B after: the test-enforced cap is 8 192.
  - "Dependencies": "the 17 below (2026-10-05; the last two ADR-0036)." becomes "the 19 below (2026-10-05; the last four ADR-0036, ADR-0072)." (+11).
  - After `remark-gfm`, two rows: "| `i18next` | <pin> | strings, `src/i18n/` only |" and "| `react-i18next` | <pin> | React for it |" (+93 to +99).
  - "Text": "data is text," becomes "chrome from `src/i18n/` (`ui-languages.md`); data is text," (+45).
  - "Keyboard": "unmodified (WCAG 2.1.4); `?` lists them." becomes "unmodified (WCAG 2.1.4), letters by `letterOf`; `?` lists them." (+23).
  - "Shell": `` `…,module.<p>}` `` becomes `` `…,module.<p>,language}` `` (+9).
  - "Owner's manual steps" 2: the scenario list, already incomplete (`health-code-ui.md`), becomes "The mock: `?scenario=<name>` before the `#`, names and projects in `src/mocks/scenario.ts`." (-188).
- **`docs/canon/workspaces.md`** "Screen", the Strings bullet becomes: "every chrome string is a key of the catalog's `workspace` group, named as `WORKSPACE_TEXT`'s members were (`ui-languages.md`); section labels: `app.sections`." (-178 B).
- **`docs/canon/architecture.md`** `#ui` and `#language`, `07-interfaces.md` s3, RM-0061 and `CLAUDE.md`'s rules were changed with ADR-0072 on 2026-11-09. At ship, `CLAUDE.md`'s UI rule drops "Russian from `ui-languages`", and its "State" adds `ui-languages` to `v4`'s shipped list.
- **`08-roadmap.md`**: row 1b shipped. **RM-0082**: `status: shipped`.
- **No change**: `serve-ui.md`, `hub.md`, `board-ui.md`, crate READMEs.

## Implementation

Filled in after implementation.
