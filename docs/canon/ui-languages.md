---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-11
---

# UI languages: the chrome in English and Russian

<a id="the-rule"></a>
## The rule

The chrome, every word the UI writes itself, is English or Russian, each viewer's choice: one per browser, for every project, on a local daemon and a hub alike; with none stored, a Russian browser starts in Russian, any other in English. Spec content (titles, bodies, names, vocabulary, workspace titles) shows as written, in any script. The daemon's, CLI's and MCP's text (refusals, notes, gap causes, messages, commands) stays English and verbatim: the UI never translates what it did not write.

Every chrome string is a key of one catalog, `ui/src/i18n/`: English, `en.ts`, the key source; Russian, `ru.ts`, typed against it, the only Russian file outside the two fixture corpora `anonymity.rs` exempts. Keys, comments, tests and docs stay English. The chrome's words are the owner's (ADR-0076, `workspaces.md` "The lens"): Department for a workspace, Task for a card (New task, Cancel task, a node's Tasks block), Subtask for a child card, Agent task for a `T-` task (the nav's Agent tasks, the work item's Agent tasks block), Directions for the bar's `kinds` modules; Russian's are his too, in `ru.ts`. Catalog keys, section IDs, routes, storage, config, CLI, MCP and daemon words keep `workspace`, `card` and `task`. The library is i18next with react-i18next at exact pins, imported only in `ui/src/i18n/`. A third language is a new decision. ADR-0072.

## Catalog

- **Files**: `en.ts` (`as const`, values byte-equal to the strings they replaced); `ru.ts` (`ru: Catalog`); `catalog.ts` (`Language`, `LANGUAGES`, `isLanguage`; `Catalog`, `en`'s shape, string leaves, each plural group gaining required `_few`, `_many`; `Messages`, plain nested text, for the parity test; it imports `en` as a type and no value); `i18next.d.ts` (`CustomTypeOptions`: keys typed by `en`); `index.ts`; `fill.ts`.
- **Keys**: `[a-z][A-Za-z0-9_]*` segments joined by `.`, the first a `src/` directory (`home` for `overview/`, `round` for Questions) or `language`; only a plural form ends `_one`, `_few`, `_many`, `_other`.
- **Values**: plain text, `{{name}}` placeholders, `{{count}}` in every plural form and nowhere else; no markup, `$t(` or format; a command to copy is code. Key caps (`Esc`, `Enter`, `Cmd-K`, letters) match in both; the arrows and Space are words, translated. Two counts on one line are two plural groups joined by a third key.
- **Elements**: `fill(t(key, {link: slot("link")}), {link: <a/>})` places a React node at each slot of the translated text; never markup. `around(text, value)` cuts a translated sentence at the first `value`, so a count is drawn on its own and the words around it read out: the count's placeholder comes first in every form of both languages, no digit before it, and every key a view cuts is in `catalog.test.ts` `AROUND`.
- **Instance**: one, initialised synchronously at the first import (`initAsync: false`) with English only, `lng: "en"`, both `supportedLngs`; `fallbackLng: "en"`, silent; `escapeValue: false` (React escapes), `skipOnVariables: true`; no backend, detector, `Trans` or `saveMissing`. English loads with the page.
- **Russian, a chunk of its own** (`ru-*.js`): reached only by `index.ts`'s one `import("./ru")`, memoised, joined by `addResourceBundle`. `loadLanguage(l)`: English at once; Russian imported once per page; resolves whether it is ready, never rejects; after a failure the next call imports again. `readyLanguage()`, the page's gate, chained by `main.tsx` with `.then` (never a top-level `await`), never rejects: an English start renders at once and imports nothing; a Russian start leaves the root empty until the chunk lands, then renders Russian, or after `LANGUAGE_WAIT_MS` (2 000 ms) renders English and turns Russian when the chunk lands, unless the viewer chose meanwhile: the last choice wins. A failed chunk leaves English, the stored choice and `language.failed`. The language switches only once its catalog is in, never to Russian keys reading English.
- **Reading**: nothing resolves at module load. A component reads `useTranslate()`, any other module takes a `Translate` (`taskStatusLook(status, t)`, `roadmap/text.ts`), a class component its wrapper's; a switch re-renders, remounting nothing. `switchLanguage` (a loaded language; stores nothing) and `translateIn` (a fixed `t`) are for the select and tests only.

## Choice

`localStorage` `specengine.language`, `en` or `ru`. Else, or on a throwing read, `navigator.languages[0]` (else `navigator.language`) with primary subtag `ru` gives Russian, any other English; nothing stored. Only the select writes; a refused write holds until a reload; another tab's change, the next load. `<html lang>` follows the language shown from init on, never a pending one.

## Switcher

A Language select in the shell's header beside Department, each option under its `lang` and named in its own language by `languageName(l)`: `Intl.DisplayNames([l], {type: "language"})`, its first letter upper-cased, else the code, so Russian reads right before its chunk loads. A choice is stored and, its catalog in, applies at once: no request, focus, view state and query data kept. Not in yet: the page keeps its language and `lang`, the select shows the choice with `aria-busy`, and it applies when the chunk lands. A failed chunk: English stays, and `language.failed` shows under the select as `role="status"` until the next choice, which imports again. The root error page (`RootFailure`) has none.

## Served text

Byte for byte in either language: the daemon's `message`, notes, `reason`, gap `cause`, config notes, verdict words, commands, IDs, paths, an unknown value raw. The daemon's text carries `lang="en"` per item, never on a list whose label is translated; spec content and names carry none. A served enum value no label table knows shows raw in its `Badge` under `lang="en"`: its `Look` sets `served: true`. Interpolated data is text. Filters and `en` sort collators are unchanged; sorts and filters compare data, never a translated word.

## Keys

`src/ui/keys.ts` `letterOf(event)`: `key` when one ASCII letter; when one non-ASCII character, `KeyA`-`KeyZ`'s letter (upper with Shift); else `null`. A Russian layout's physical key acts; Dvorak keeps its letters; ASCII punctuation (AZERTY's `,` on `KeyM`) and named keys (`Dead`) are none. Every letter hotkey compares it (`hotkeyOf`: it, else `key`): `j`/`k`, the Inbox's `a`/`r`/`c`/`d`, the Board's `m`/`n`, Graph's and Flow's `o`/`c`, the palette chord. Other keys stay on `key`.

## Dates and counts

Plural forms by `Intl.PluralRules` (Russian 1, 21 `one`; 2-4 `few`; 0, 5-20 `many`), numbers ungrouped. `ui/time.ts` `formatUtc(stored, t)` fills `ui.time.stamp` from `Intl.DateTimeFormat("en-US", {timeZone: "UTC", hourCycle: "h23", …})` parts: `2026-10-05 21:14 UTC`, Russian `05.10.2026 21:14 UTC`, unparsable raw. `formatAge(stored, now, t)`: plural keys. An English `_one` reads right at 1 ("1 link not shown"; RM-0091, `i18n/plural.test.ts`).

## Converted

Every screen: Home, Inbox, Tasks, Board, Roadmap, Spec tree, Graph, Health, Questions and Flow, its panel's `tree/LinksPanel.tsx` and `trace/CardsBlock.tsx` too. Every `src/` directory but `api/`, `i18n/`, `mocks/`, `styles/` and `test/` is scanned (`policy.test.ts` `NOT_SCANNED`); a new directory is scanned from the start, and a slice keys its new strings. A line built in the CLI's form (Roadmap's header, a revision line) translates its words and keeps its IDs, numbers and `->`. `src/api/http.ts`'s transport messages stay English, shown as error text under `lang="en"`.

## Guards

- `src/i18n/catalog.test.ts`: key parity, plural forms, placeholder sets, `{{count}}`, no empty value or markup; `@ts-expect-error` `t("app.nope")`; `AROUND` is exactly the keys views pass to `around()`, each with its count first. `plural.test.ts`: every plural group under `ruT`, `_one` at 1 and 21, `_few` at 3, `_many` at 5.
- `src/policy.test.ts`, over every scanned non-test `.ts` and `.tsx`: no letter hotkey on `.key`. **JSX** (`.tsx`): no JSX text with a letter, no literal in a shown attribute or `TEXT_PROPS` prop (a new shared one joins). **Prose**: no string or template text reading as English (a phrase `/\b[A-Za-z][a-z]+[ ,;:]+[a-z]{2,}\b/` or one capitalised word) outside code: a specifier, a type, a property name, a `case` label, an operand of `===`, `!==` or `in`; `t`'s key; the first argument of a `CODE_METHODS` DOM, storage, string or collection method (a selector, a key, a separator; the others are read); a not-shown attribute (`className`, `key`, `id`, `href`, `lang`, `role`, `type`); a code token (`spec …`, a `…COMMAND`); `PROSE_ALLOWED`, exact literals per file, each code, with its reason. Raw Cyrillic only in `ru.ts`; only `src/i18n/` imports the library (ESLint too); `index.ts` holds the one `import("./ru")`, and no non-test module imports `ru` statically.
- `src/i18n/words.test.ts`: no `en` leaf holds `card(s)` or `workspace(s)`; every `task(s)` under `AGENT_TASK_GROUPS` (`tasks.`, `app.sections.tasks.`, `home.tasks.`, `app.shortcuts.tasks.`, `board.tasks.`) and in `AGENT_TASK_KEYS`' 20 leaves outside them follows `agent `; no `ru` leaf holds the old card stem or workspace phrase (`\u` escapes); no `en` leaf holds `writer(s)`, no `ru` leaf the two stems of the Russian word (the chrome names no trade, ADR-0008); the Department label pinned in both.
- `anonymity.rs` exempts exactly `fixtures/spec-b`, `fixtures/token-calibration`, `ui/src/i18n/ru.ts`; `ui_policy.rs`: the 19 pins, the README table, the seam.

## Size

The gate is **first-load JS**: the entry `<script>` of `ui/dist/index.html` plus every `<link rel="modulepreload">` it lists, raw bytes on disk, as the daemon sends them (no `content-encoding`, `serve-ui.md`). Each UI slice prints it in its "Implementation" with the first-load gzip (Vite's per-chunk figures, summed) and `ru-*.js` raw and gzip, for a hub behind a compressing proxy. Baseline, 2026-11-09 (RM-0090): 768.96 kB (768 959 B: `index-*.js` 622.62, `Icon-*.js` 140.15 holding i18next and English, four small chunks), gzip 219.15 kB; `ru-*.js` 134.98 kB, gzip 31.13 kB.

**Bar: 845 kB**, the baseline plus 10 %. A slice that would pass it moves a screen or part out of first load (a lazy chunk, as `HealthRegions`, `RoundScreen`, `Markdown` and `DocBody` are) before it ships; the bar is never raised. `ru-*.js` has none: only a Russian viewer fetches it, once a page. Vite's 500 kB warning on `index-*.js` is not the gate. At RM-0097, 840.79 kB: the next slice moves `RoadmapView` out of first load, then `GraphView` (RM-0098).

## Known limits

- A Russian start on a slow link shows an empty page for up to 2 s, then English until the chunk lands.
- The prose rule is a net, not a proof: a bare lower-case word passes; review covers it.
- Spec content carries no `lang`; one choice per browser, no cross-tab sync.
- The Russian awaits the owner's review; rewording breaks no test.

## Tests

Russian only via `ru.ts` or `\u` escapes; `src/test/text.ts` `enT`, `ruT`. `src/test/setup.ts`, in `beforeEach`, imports `../i18n`, loads Russian's chunk (once a file) and switches back to English, so a file's `vi.mock` of a catalog applies (`src/i18n/fallback.test.tsx`). `src/i18n/load.test.tsx`: the start, the wait, a failure and the last choice on a fresh instance with a mocked `./ru`. `src/app/language.test.tsx` switches over the stub client; `src/app/screens.language.test.tsx` switches each screen and keeps its served text.
