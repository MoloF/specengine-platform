---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-09
---

# UI languages: the chrome in English and Russian

<a id="the-rule"></a>
## The rule

The chrome, every word the UI writes itself, is English or Russian, each viewer's choice: one per browser, for every project, on a local daemon and a hub alike; with none stored, a Russian browser starts in Russian, any other in English. Spec content (titles, bodies, names, vocabulary, workspace titles) shows as written, in any script. The daemon's, CLI's and MCP's text (refusals, notes, gap causes, messages, commands) stays English and verbatim: the UI never translates what it did not write.

Every chrome string is a key of one catalog, `ui/src/i18n/`: English, `en.ts`, the key source; Russian, `ru.ts`, typed against it, the only Russian file outside the two fixture corpora `anonymity.rs` exempts. Keys, comments, tests and docs stay English. The library is i18next with react-i18next at exact pins, imported only in `ui/src/i18n/`. A third language is a new decision. ADR-0072.

## Catalog

- **Files**: `en.ts` (`as const`, values byte-equal to the strings they replaced); `ru.ts` (`ru: Catalog`); `catalog.ts` (`Language`, `LANGUAGES`, `isLanguage`; `Catalog`, `en`'s shape, string leaves, each plural group gaining required `_few`, `_many`; `CATALOGS: Record<Language, Messages>`, plain nested text, as `en` lacks them); `i18next.d.ts` (`CustomTypeOptions`: keys typed by `en`); `index.ts`; `fill.ts`.
- **Keys**: `[a-z][A-Za-z0-9_]*` segments joined by `.`, the first a `src/` directory (`home` for `overview/`) or `language`; only a plural form ends `_one`, `_few`, `_many`, `_other`.
- **Values**: plain text, `{{name}}` placeholders, `{{count}}` in every plural form and nowhere else; no markup, `$t(` or format; a command to copy is code. Key caps (`Esc`, `Enter`, `Cmd-K`, letters) match in both; the arrows and Space are words, translated.
- **Elements**: `fill(t(key, {link: slot("link")}), {link: <a/>})` places a React node at each slot of the translated text; never markup.
- **Instance**: one, initialised synchronously at the first import (`initAsync: false`), both catalogs in the main chunk; `fallbackLng: "en"`, silent; `escapeValue: false` (React escapes), `skipOnVariables: true`; no backend, detector, `Trans` or `saveMissing`.
- **Reading**: nothing resolves at module load. A component reads `useTranslate()`, any other module takes a `Translate` (`taskStatusLook(status, t)`), a class component its wrapper's; a switch re-renders, remounting nothing. `switchLanguage` (stores nothing) and `translateIn` (a fixed `t`) are for the select and tests only.

## Choice

`localStorage` `specengine.language`, `en` or `ru`. Else, or on a throwing read, `navigator.languages[0]` (else `navigator.language`) with primary subtag `ru` gives Russian, any other English; nothing stored. Only the select writes; a refused write holds until a reload; another tab's change, the next load. `<html lang>` follows from init on.

## Switcher

A Language select in the shell's header beside Workspace, each option its language's own name under its `lang`. A choice applies at once: no request, focus, view state and query data kept. The root error page (`RootFailure`) has none.

## Served text

Byte for byte in either language: the daemon's `message`, notes, `reason`, gap `cause`, config notes, verdict words, commands, IDs, paths, an unknown value raw. The daemon's text carries `lang="en"` per item, never on a list whose label is translated; spec content and names carry none. Interpolated data is text. Filters and `en` sort collators are unchanged.

## Keys

`src/ui/keys.ts` `letterOf(event)`: `key` when one ASCII letter; when one non-ASCII character, `KeyA`-`KeyZ`'s letter (upper with Shift); else `null`. A Russian layout's physical key acts; Dvorak keeps its letters; ASCII punctuation (AZERTY's `,` on `KeyM`) and named keys (`Dead`) are none. Every letter hotkey compares it (`hotkeyOf`: it, else `key`): `j`/`k`, the Inbox's `a`/`r`/`c`/`d`, the Board's `m`/`n`, Graph's and Flow's `o`/`c`, the palette chord. Other keys stay on `key`.

## Dates and counts

Plural forms by `Intl.PluralRules` (Russian 1, 21 `one`; 2-4 `few`; 0, 5-20 `many`), numbers ungrouped. `ui/time.ts` `formatUtc(stored, t)` fills `ui.time.stamp` from `Intl.DateTimeFormat("en-US", {timeZone: "UTC", hourCycle: "h23", …})` parts: `2026-10-05 21:14 UTC`, Russian `05.10.2026 21:14 UTC`, unparsable raw. `formatAge(stored, now, t)`: plural keys.

## Converted

`src/app/`, `palette/`, `overview/`, `ui/`, `workspace/`; the six `labels.ts`, `tasks/groups.ts`, `main.tsx`'s mock lines, dates. Other screens stay English until 2b (`src/flow/`) and RM-0090; a slice touching a screen keys its new strings and adds its directory to the scan.

## Guards

- `src/i18n/catalog.test.ts`: key parity, plural forms, placeholder sets, `{{count}}`, no empty value or markup; `@ts-expect-error` `t("app.nope")`.
- `src/policy.test.ts`: no letter hotkey on `.key`; in a converted `.tsx`, no JSX text with a letter, no literal in a shown attribute or `TEXT_PROPS` prop (a new shared one joins); raw Cyrillic only in `ru.ts`; only `src/i18n/` imports the library (ESLint too).
- `anonymity.rs` exempts exactly `fixtures/spec-b`, `fixtures/token-calibration`, `ui/src/i18n/ru.ts`; `ui_policy.rs`: the 19 pins, the README table, the seam.

## Known limits

- Until RM-0090 a converted label sits in an English screen.
- Every page loads both catalogs, +99 kB at 1b; past +120 kB Russian goes to a lazy chunk.
- Spec content carries no `lang`; one choice per browser, no cross-tab sync.
- The Russian awaits the owner's review; rewording breaks no test.

## Tests

Russian only via `ru.ts` or `\u` escapes; `src/test/text.ts` `enT`, `ruT`. `src/test/setup.ts` imports `../i18n` in `beforeEach`, back to English, so a file's `vi.mock` of a catalog applies (`src/i18n/fallback.test.tsx`). `src/app/language.test.tsx` switches over the stub client.
