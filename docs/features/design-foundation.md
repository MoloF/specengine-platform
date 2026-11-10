---
class: spec
status: shipped
shipped: 2026-11-10
scope: [ui]
ref: RM-0093; v5 step 1 (08 s2 Phase 7, "v5 scope"); the owner's request and answers of 2026-11-10 (themes, Inter, words); the analyst's DF-1 to DF-11, extended here for the light theme; W1-W12 working answers, the owner's review open
---

# design-foundation: light and dark tokens, Inter, a type scale, primitives, and the task as the first work item (RM-0093)

## Why

The owner's verdict of 2026-11-10: font and block sizes were inconsistent and the UI "looks hand-made"; he wants a mature tracker's look, much like Jira. The UI had one dark theme, `system-ui` text at 13 and 15 px beside raw sizes, six spacing steps with raw gaps, 36 px controls, 999 px pills, and a card modal listing every field in one column. His answers: light and dark themes, light by default, dark when the system asks, a toggle per viewer; Inter bundled under the OFL, with Cyrillic, no font host. So this slice lays the foundation every later v5 UI slice uses: semantic tokens in both themes, a rem type scale, 8 px spacing, radii and control heights, the primitives, guards that fail on a raw size; and the task modal and page become its first work item. The working answers W1-W12 (the font as files, not a package; `light-dark()`; an indigo accent; grey dark surfaces; category precedence; status shown once, on the column head; a badge's label in primary text; the Details order; icon selects in the header; legacy aliases until RM-0099; lozenges wrap, never cut; dialog sizes) await the owner's review. How it works now: `docs/canon/ui-design.md`.

## Description and interactions

Each part is canon now; the headings stay because code comments cite them.

### Themes

`ui-design.md` "Themes", "Contrast".

### Typography

`ui-design.md` "Type, space and size", "Fonts".

### Space, shape, size

`ui-design.md` "Type, space and size".

### Primitives (`ui/src/ui/`)

`ui-design.md` "Primitives", "The focus trap".

### Status categories

`ui-design.md` "Status categories".

### The work item: the task modal and page

`ui-design.md` "The work item"; routes, history and writes `board-screen.md` "The card modal and page"; Tasks `card-tasks.md` "Screen".

### The Board

Column heads are category lozenges, Parent lane heads the flow name and a lozenge, badges and tags lozenge-sized, 32 px controls: `board-screen.md` "The grid", `ui-design.md` "Primitives".

### The header

`ui-design.md` "The header".

### Guards and staging

`ui-design.md` "Guards and exceptions". RM-0094 (Flow), RM-0095 (the document modal) and RM-0099 (every other screen) move off the legacy aliases and empty the exceptions named for them.

## Data

### Theme choice

`localStorage` `specengine.theme`, `<html data-theme>`, `src/app/theme.ts`: `ui-design.md` "Themes".

### Contrast matrix

`ui-design.md` "Contrast"; the values in `src/styles/tokens.css`.

### Size tokens

`ui-design.md` "Type, space and size"; the sets `scale.test.ts` reads.

### `fonts.css` and `ui/fonts/inter/`

`ui-design.md` "Fonts"; provenance and SHA-256s in `ui/fonts/inter/README.txt`.

### Exceptions

`src/styles/scale-exceptions.ts`; the policy in `ui-design.md` "Guards and exceptions".

## Acceptance criteria

Look = the orchestrator's dated check in Chrome DevTools on a scratch build (`vite preview`). Each named mutation turned its test red.

- [x] **DF-1 Type scale.** `src/styles/scale.test.ts` "the type scale (DF-1)". M: `font-size: 13px` in `.board-card-title`; `--font-size-body: 0.8125rem`; a canvas label's exception deleted; an exception matching nothing.
- [x] **DF-2 Spacing.** `scale.test.ts` "spacing (DF-2)", with iteration 2's probes (a last declaration with no `;`, a raw length in a custom property, a `var()` fallback). M: `gap: 3px` in `.board-card-top`; `style={{ padding: 4 }}` in `BoardCard.tsx`; `--space-150: 0.7rem`.
- [x] **DF-3 Radii and control heights.** `scale.test.ts` "radii and control heights (DF-3)". M: `border-radius: 5px` on `.lozenge`; `--radius-m: 10px`; `.button { min-height: 2.25rem }`.
- [x] **DF-4 One primary per area.** `src/ui/Button.test.tsx`; `src/board/workItem.test.tsx` "one primary per area (DF-4)", modal, page and each dialog. M: Hand off as `primary`; Button dropping `type="button"`.
- [x] **DF-5 Lozenges by category.** `src/board/category.test.ts`, `src/ui/Lozenge.test.tsx`, `BoardView.test.tsx` "the column heads' lozenges (DF-5)", `CardModal.test.tsx` (Hand-offs rows). M: `accept.to` and `return` rules swapped; a Lozenge without its icon; `"blocked"` quoted in `category.ts` (`policy.test.ts`).
- [x] **DF-6 Work-item order.** `workItem.test.tsx` "the work item's order (DF-6)", `src/ui/PropertyList.test.tsx`. M: Flow before Reviewer; empty rows left in Details; side width `22rem`.
- [x] **DF-7 Development folded.** `workItem.test.tsx` "Development, folded (DF-7)", `src/ui/EmptyState.test.tsx`. M: opened by default; the summary counting `null` rows out.
- [x] **DF-8 Breadcrumbs.** `src/ui/Breadcrumbs.test.tsx`; `workItem.test.tsx` "the breadcrumbs (DF-8)". M: `aria-current` dropped; the current item linked; a parent crumb on a card with none.
- [x] **DF-9 Contrast in both themes.** `src/styles/tokens.test.ts`, rewritten, every distinctness check per theme. M: light `--text-secondary` `#8c8f97`; dark `--lozenge-success-text` `#4c6b1f`; `--attention` a plain hex; `light-dark(` in `app.css`.
- [x] **DF-10 [look] Measured metrics.** Run 2026-11-10 in two of the eight combinations, the extremes, by the snippet, no screenshot names. 1440x900, light, English, the C-0001 modal (`?scenario=config-notes`): font sizes 12/14/16/20/24 only; controls 24/32; no target under 24x24 (inline links aside); header 48; side column 330; modal 968; lozenges 20; `color-scheme` `light dark`; body in Inter. 1100x800, dark (system), Russian: modal 12/14/16/20/24, Board 12/14/24, controls 24/32, header one 48 px row, no horizontal scroll. Re-run after iteration 2: the dark Russian 1100 modal and the light English 1440 Board on the scale, header 48, no overflow.
- [x] **DF-11 Catalogs, gates, size, churn.** `en.ts`, `ru.ts` (`tsc`); `app/screens.language.test.tsx` "the work item and the header in Russian (DF-11 of design-foundation)"; `pnpm lint`, `build`, `test` (154 files, 4 670 tests); eval `ui_policy`, `anonymity`, `doc_pointers` (58); first-load JS under 845 kB (below); churn below. M: a `ru.ts` key removed; "More fields" as a JSX literal.
- [x] **DF-12 Theme.** `src/app/theme.test.tsx` "the theme (DF-12)"; `tokens.test.ts` "the theme's colour scheme (DF-12)". M: dark with nothing stored; another storage key; `color-scheme: dark` in `:root`.
- [x] **DF-13 Inter bundled.** `src/styles/fonts.test.ts` "Inter, bundled (DF-13)", the `dist` licence too. M: a face's `src` at `https://`; `OFL.txt` removed; the cyrillic face dropped.
- [x] **DF-14 One-line header.** `src/app/header.test.tsx` "the one-line header (DF-14)"; the trap `Dialog.test.tsx` "the trap's stops", `workItem.test.tsx` "focus in the card's modal". M: a label dropped; `flex-wrap: wrap` above 68.75rem. **Look**, 2026-11-10, light/English/1440 and dark/Russian/1100: one 48 px row, every value readable, the notes as icon and count, the mock indicator short (the developer measured 1100, 1280, 1440, 1600, both languages); real key presses in the dark Russian modal: focus on the title, Shift+Tab to the collapsed Development button, Tab from it round to the first crumb, Tab from the title to Hand off.

## Out of scope

The owner's words Task, Subtask, Department and any renamed string (RM-0096); other screens and the legacy renames (RM-0099); Flow and the document modal (RM-0094, RM-0095); Ready to accept, progress bars (RM-0098); Activity (RM-0097). Never here: a `categories` key, a lozenge on tiles, Jira's combined status button, truncated lozenges, a high-contrast theme, density, a hub-wide theme, cross-tab theme sync, an italic file, avatars, packaged icons, a daemon or dependency change.

## Implementation

Code review: REWORK; its majors (the trap's hidden stops, squeezed header selects) and minors fixed in iteration 2; departures 1-9 accepted, amended. UI only.

**Departures.** (1) Theme keys `app.theme.*`, not `theme.*`. (2) Font provenance in `ui/fonts/inter/README.txt`: a `README.md` would be Tier 1. (3) `workItem.test.tsx` runs over the test stub: the policy keeps the mock's client in `src/mocks/`. (4) For the word bans, `category.ts` holds the rules and keys, `labels.ts` `categoryMark` the look. (5) Extra APIs: `Crumb.onFollow`; `Dialog`'s `frame?: (heading) => ReactNode`, `isTabStop(element, root)`, `focusables(root)`; `CardModal`'s `view: (frame: {heading, tools}) => ReactNode`; `CardView` for `CardBody`; `buttonClasses(appearance?, spacing?, className?)`; `Button`'s `ref`; `LozengeLook`, `LOZENGE_APPEARANCES`. (6) Lozenge padding 2 px 4 px, not 0 4 px. (7) The look bundle: tags at badge metrics, `.flow-box .badge` `min-height: 0`; default buttons a subtle border, danger bold, Cancel card subtle; the nav's current accent on selected, no left bar; `.card-dialog` 600 px; checkbox 24x24; the header line an inset shadow. (8) The Status lozenge replaces "In <column>"; `board.panel.in` is gone. (9) `.app-body`'s `min-height` stays, Flow's styles test pinning it: a follow-up in RM-0094 or RM-0099. (10) The header's short forms start below 100rem, not 80rem: at 1440 in Russian the full forms overflowed one row by 90 px. (11) `edit`, the Board's own act, is exempt from the column-word ban. (12) `field-sizing: content` is Chrome-only; elsewhere a header select takes its widest option, at most 13rem, a longer value cut while closed. (13) `colorMode` removed from three canvases; `.flow-edge-label`'s exception gone, 29 left. (14) The title's ring after opening is Chrome's `:focus-visible` on a scripted focus, kept.

**Extra test churn**: `CardPage.test` (act order, Move under Status), `BoardView.test`, `hubIdentity.test` (Move's place), `board.smoke` (hand-off rows), parent and corrects links scoped to Details; new cases in `Dialog.test`, `graph/styles.test`, `policy.test`.

**Mutations**: the 37 above and four on iteration 2's fixes, all red. **Size**: first-load JS 806.03 kB raw (797.95 before, bar 845), gzip 229.05 kB; CSS 130.16 kB; `ru` 139.17 kB; fonts four `woff2`, 178 032 B, none preloaded; `OFL.txt` in `dist/assets/`. **Font**: Inter 4.001 from `@fontsource-variable/inter` 5.3.0.

| Module | What it does |
|---|---|
| `ui/fonts/inter/`, `public/assets/`, `src/styles/*`, `index.html`, `src/main.tsx` | fonts, licence; both themes, the scale, 29 exceptions, every rule on tokens; the theme before the root |
| `src/app/{theme.ts,ThemeSwitcher.tsx,Shell.tsx,LanguageSwitcher.tsx}`, `workspace/WorkspaceControls.tsx` | the choice; the one-row header |
| `src/ui/*.tsx` | seven primitives, six icons, the trap |
| `src/board/{category,labels}.ts`, `Board{View,Column,Card}.tsx`, `Card{Modal,Page,Panel,Tasks}.tsx` | categories, lozenge heads and lanes; `CardView` in `IssueView` |
| `flow/`, `graph/`, `roadmap/` canvases; `i18n/{en,ru}.ts` | no `colorMode`; the new keys |
