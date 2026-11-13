---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-12
---

# UI design: themes, the scale, primitives, the work item

Built by `design-foundation` (RM-0093), no ADR, on the owner's answers of 2026-11-10: two themes, Inter, a mature tracker's look. The numbers follow the Atlassian Design System's public tokens (Apache-2.0); no Atlassian name, logo, font or illustration, the accent SpecEngine's own. Code: `ui/src/styles/`, `src/ui/`, `src/app/theme.ts`, `src/board/category.ts`.

## The rule

One file, `src/styles/tokens.css`, holds every colour literal, in both themes, and every size step. Any other CSS or component takes one `var()` of a token, or a listed exception. Screens build from `src/ui/`'s primitives. A status is a label and an icon, never colour alone.

## Themes

- **Roles.** A colour role is declared once in `:root` as `light-dark(<light>, <dark>)`, each side six-digit hex or a `var()`; an alias `var(--role)` is theme-free; `light-dark(` stands in no other file. The alpha roles: `--scrim`, `--shadow-key`, `--shadow-ambient`, `--graph-minimap-mask`, `--graph-selection-surface`.
- **System.** `:root` has `color-scheme: light dark`: with no choice the page follows `prefers-color-scheme` (light when it states none), no script; a change applies at once.
- **Choice.** The header's Theme select (System, Light, Dark) writes `localStorage` `specengine.theme`, `light` or `dark` (System removes it), and sets `<html data-theme>`; `:root[data-theme="light"|"dark"]` pins `color-scheme`. Another value or a throwing storage reads System, silently. Per browser profile; a hub keeps none.
- **First paint.** `src/main.tsx` runs `applyTheme(storedTheme())` before `createRoot`; `index.html` has `<meta name="color-scheme" content="light dark">`, no inline script.
- **Palette.** Neutral grey surfaces, both themes: canvas, sunken (board columns, wells), panel (header, nav), raised (tiles, `--shadow-raised`), overlay (dialogs, menus, `--shadow-overlay`), hovered, selected. Accent indigo `#4f46e5` / `#a5b4fc`, not the information blue. Six lozenge pairs `--lozenge-<a>-surface`, `-text`: neutral, information, success, warning, danger, discovery.
- **Canvases.** Flow's, Graph's and Roadmap's `--xy-*` resolve to `light-dark()` roles, `colorMode` never set; the Map's: `spec-map.md`.

## Contrast

`tokens.test.ts` resolves every role per theme and checks both. At least 4.5:1: text (primary, secondary, subtlest) and the accent on every surface, hovered and selected too; on-accent, on-warning, on-danger on their fills; each lozenge's text on its surface; primary and secondary text on lozenge, diff and highlight surfaces. At least 3:1: focus ring, control border, every status role. Roles kept distinct (states, edges, cannot-verify and ok) differ in both themes.

## Type, space and size

- **Type.** `--font-sans` is `"Inter"`, then the system stack; `--font-mono` unchanged. Steps, size/line in rem, as `--font-size-<step>` and `--line-height-<step>`: small 12/16 (labels, lozenges, badges, kbd), body 14/20 (the chrome), document 16/24 (a document's or a task's text, at most `--reading-width` 68ch), heading-s 16/20, heading-m 20/24 (`h2`), heading-l 24/28 (`h1`), code 12/20. Weights `--font-weight-regular|medium|semibold|bold`, 400 to 700.
- **Space.** `--space-025` to `--space-600`: 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48 px. Radii `--radius-xs|s|control|m|l|xl` 2, 4, 6, 8, 12, 16 px and `--radius-full` 999 px; borders 1 px, focus and selection 2 px.
- **Size.** Controls `--control-height` 32 px, compact 24 px; `--target-min` 24 px; `--nav-height` 48 px; `--issue-side-width` 360 px beside `--issue-main-min` 320 px; `--modal-width-s|m|xl` 600, 800, 1200 px; `--empty-state-width` 464 px; `--lozenge-height` 20 px, `--lozenge-max-width` 200 px.

## Guards and exceptions

`src/styles/scale.test.ts` reads every CSS file under `src` but `tokens.css`: `font-size`, `line-height`, `padding*`, `margin*`, gaps and radii take one `var()` of their family (spacing also `0`, `auto`, `calc()` of tokens); `font` only `inherit`; G2: `font-weight`, outside `@font-face`, one `--font-weight-*` or `inherit`; a last declaration with no `;`, a raw length in a custom property outside `tokens.css` and a `var()` fallback count. G1: no legacy name (`--space-1..6`, `--font-size-s|m|l|xl`, `--line-height-tight`) in any `src` CSS, `tokens.css` too, or non-test TS or TSX. Each family is exactly its steps (space 11, size and line 7, weight 4). `.button` and `.field` are `var(--control-height)` tall, `.button-compact` the compact one. No non-test `.tsx` sets a size, space, radius or weight inline.

`src/styles/scale-exceptions.ts` `SCALE_EXCEPTIONS: readonly ScaleException[]`, each `{file, selector, property, value, reason, until: null}` matching a declaration. G3: exactly two, kept: `.sr-only`'s `-1px`; `.tab`'s `-1px`, a border over a border. A new one is kept, with a reason.

## Primitives

In `src/ui/`, each with its test:
- `Button(props: ButtonProps)`: `appearance?: "default" | "primary" | "subtle" | "warning" | "danger"`, `spacing?: "default" | "compact"`, `icon?: IconName`, `className?` (layout only), `ref?`, the rest `<button>`'s, `type` `button` unless given. Classes `button` plus `button-primary|quiet|warning|danger|compact`; `buttonClasses(appearance?, spacing?, className?)` gives them to a link or a menu trigger. At most one primary per dialog, item's acts or side column; a staged choice's Confirm takes it (`hub-confirm.md` "Screen").
- `Lozenge({appearance, icon, label, lang}: LozengeLook & { lang?: "en" })`: 12/16 medium, padding 2 px 4 px, radius 4; a longer label wrapping by whole 16 px lines, never cut. It marks a status in an item's head or a Roadmap card's, and Health's verdict; a proposal's status has one table, `inbox/labels.ts` `statusLozenge` (`proposal-review.md` "Inbox").
- `PropertyList({label, items, foldEmpty?})`, `Property {key; label; value: ReactNode | null}`: a `dl`, labels small and secondary. With `foldEmpty`, `null` rows stand in order under one "More fields (n)" disclosure, collapsed, each "None".
- `Breadcrumbs({items})`, `Crumb {key; label; href?; mono?; onFollow?}`: a `nav` "Breadcrumbs" around an `ol`; all but the last link from `routes.ts`, at least 24 px; the last `aria-current="page"`, no `href`; `/` separators `aria-hidden`; `onFollow` takes a plain click in place. A pane's Project: `useProjectName` (`proposal-review.md` "Inbox").
- `IssueView({breadcrumbs, tools?, title, meta?, actions?, main, side, layout?: "item" | "pane"})`: "The work item".
- `EmptyState({title, titleId?, titleLang?: "en", level: 2 | 3 | 4, description?, primary?, secondary?})`: sentence case, never a failure, one primary at most.
- `Disclosure({summary, regionLabel?, className?, children})`: a button with `aria-expanded`, `aria-controls`; its region `hidden` while collapsed, a `section` landmark with `regionLabel`; focus stays on the button; only the chevron turns, by the duration tokens.
- Rows keep `.badge`: the lozenge's metrics, the label `--text-primary`, the tone on icon and 1 px border. Icons are hand-drawn at 16 px.

## Status categories

`src/board/category.ts` `categoryOf(flow, column)` (`todo`, `doing`, `done`, `returned`, `stranded`): the column's place in its served flow, first match winning: unserved or not in `columns` stranded; `accept.to` done; `return` returned; `columns[0]` to do; else in progress. `statusLook(flow, column)` joins `labels.ts` `categoryMark(category)` (stranded and to do neutral, done success, returned warning, in progress information) to the served title, the raw ID when stranded; `outsideLook(t)` is "Not in the flow". Cancellation keeps the category. `policy.test.ts` bans in `src/board/` the mock's column IDs and flow names quoted, and `column`, `to` or `from` compared with a quoted word but `edit`. Shown on column heads, Parent lane heads (after the flow title), the side Status and Subtasks rows; tiles carry none.

## The work item

`CardModal` (`view: (frame: {heading, tools}) => ReactNode`, by `Dialog`'s `frame`) and `CardPage` render one `CardView` (`CardPanel.tsx`) in an `IssueView`; a document's, `DocFrame` (`doc-view.md`). Routes, history, focus and writes: `board-screen.md` "The card modal and page".
- **Head.** Breadcrumbs Project (`#/<p>`), Board, the parent card (its modal, replacing the entry), the card's ID current; the tools on that line (modal: Open as a page, Close; page: Open on the board); the title, the modal's focused `h2` or the page's `h1`; the acts Accept (primary, in `accept.from` only), Hand off, Return…, Ask a question…, Record a decision…, Edit, Assign, Cancel task (subtle, last), a mandate moving some after Accept (`card-mandate.md`).
- **Main.** Mark and Clear, Changed above, return reason, acceptance, cancellation; Goal and Criteria at the document step; Source; Subtasks (`card-progress.md`), Decisions from subtasks; Evidence; Activity (`card-questions.md`).
- **Side**, at most 360 px. Status: the lozenge, then Move, compact, none on a closed card. Details: Assignee, Reviewer, Mandate, Flow, Due, Estimate, Discipline, Parent, Corrects, Created by, Created at, empty ones under More fields. Agent tasks, collapsed: `card-tasks.md` "Screen".
- **Width.** One column, main first, under 44.5rem of its own width (`@container issue-view`); DOM order main, side. The modal: 1200 px or the viewport less 2rem, its head still; at two columns Main and the side scroll apart (`overscroll-behavior: contain`, each padded for focus rings), at one the body; under 36rem the screen, scrolling as one. The page, 1200 px at most, scrolls the window. A pane (`layout="pane"`: Inbox's proposal, the Spec tree's node, an Agent task): DOM order side, main; one column until 60rem, then main left; an `:empty` side hidden, the pane one column by `:has()`.

## The focus trap

`Dialog.tsx` `focusables(root)` lists drawn stops only: `isTabStop(element, root)` skips one inside `[hidden]` or `[inert]`, `display: none`, `visibility: hidden`, or boxless while the dialog has a box. From the title focused by script, Tab goes to the next stop after it in the DOM and Shift+Tab to the last; elsewhere both wrap. `frame?: (heading) => ReactNode` lays the `h2` among the caller's parts.

## The header

One row of `--nav-height`, `flex-wrap: nowrap` from 68.75rem (a failed project read wraps), controls 32 px at 14/20: brand, Project, Department, config notes, Language, Theme, the mock indicator, Jump to, Shortcuts. A select shows its value after an icon, its label visually hidden, `flex: none`, `field-sizing: content` up to 13rem. Below 100rem the config notes show icon and count, the mock indicator "Mock data" without the scenario; below 80rem Jump to and Shortcuts are icon-only. Names stay. Nav items are 32 px, the current in accent on `--surface-selected`. The body and sticky panes offset by `--nav-height`: one row.

## Fonts

Inter 4.001 from `@fontsource-variable/inter` 5.3.0, committed in `ui/fonts/inter/` (outside `src`, which `ui_policy.rs` reads as text): `inter-{latin,latin-ext,cyrillic,cyrillic-ext}-wght-normal.woff2` (weights 100-900, upright, italic synthesised), `README.txt` (provenance, SHA-256s) and `OFL.txt`, SIL OFL 1.1, whose byte-equal copy `ui/public/assets/OFL.txt` ships in `dist/assets/`. `src/styles/fonts.css`, imported before `tokens.css`, holds the only `@font-face`s: four, `font-display: swap`, each with the package's `unicode-range`, so a page fetches only its subsets; none preloaded, no `data:` font, no remote `url()`. No package: an update replaces the files and `README.txt`.

## Known limits

- `field-sizing: content` is Chrome's; elsewhere a header select is as wide as its widest option. Past 13rem a value is cut while closed.
- Every asset is served `no-store`, so fonts load once per page load; the blank page before the entry module shows the system scheme.
- Safari scrolls a modal's column by keyboard only with focus in it.
- Sizes, the one-row header and the two scrollers are a look in Chrome (`design-foundation.md` DF-10, DF-14): jsdom lays nothing out.

## Tests

UI `src/styles/*.test.ts`, `src/ui/*.test.*`, `src/board/{category,workItem}.test.*`, `src/app/{theme,header}.test.tsx`, `graph/styles.test.ts`, `policy.test.ts`.
