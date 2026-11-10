---
class: spec
status: draft
scope: [ui]
ref: RM-0093; v5 step 1 (08 s2 Phase 7, "v5 scope"); the owner's request and answers of 2026-11-10 (themes, Inter, words); the analyst's DF-1 to DF-11, extended here for the light theme; W1-W12 below are working answers, the owner's review open
---

# design-foundation: light and dark tokens, Inter, a type scale, primitives, and the task as the first work item (RM-0093)

## Why

The owner's verdict of 2026-11-10: font and block sizes are inconsistent, and the UI "looks hand-made". He wants the look of a mature tracker, much like Jira. Today the UI has:
- one dark theme;
- `system-ui` text at 13 and 15 px, with raw 11, 12 and 0.92em sizes beside them;
- six spacing steps, with raw 1 px and 2 px gaps;
- 36 px controls;
- pills of radius 999 px;
- a card modal that lists every field in one column, technical ones first.

His answers fix the base:
- light and dark themes, light by default, dark when the system asks, and a toggle remembered per viewer;
- Inter, bundled in the build under the OFL, with Cyrillic, and no font host.

This slice builds the foundation that every later v5 UI slice uses (Q8: the foundation comes before Flow v2's UI). The foundation is:
- semantic tokens in both themes;
- a rem type scale, spacing on an 8 px base, a radius scale, and control heights;
- six primitives;
- guards that fail on a raw size.

The task modal and page are its first application: they become a work-item view, with a breadcrumb, a main column and a side column of people's fields, where the technical side is folded.

The numbers follow the Atlassian Design System's public tokens (Apache-2.0 packages): the type scale, spacing, radii, colour roles and the lozenge's six appearances. The UI uses no Atlassian name, logo, font file or illustration, and its accent hue is SpecEngine's own (W3).

## Description and interactions

### Themes

- **Two themes, one set of roles.** Every colour role in `ui/src/styles/tokens.css` is written once as `light-dark(<light>, <dark>)`. `:root` declares `color-scheme: light dark`, so with no choice stored the page follows the system's `prefers-color-scheme`, light when the system states none. This needs no script and no reload, and a system change applies at once.
- **The choice.** A **Theme** select in the header offers System, Light and Dark. It writes `localStorage` `specengine.theme` (`light` or `dark`; System removes the key) and sets `data-theme` on `<html>`, where `:root[data-theme="light"]` and `:root[data-theme="dark"]` pin `color-scheme`.
  - **Before the first render.** `src/main.tsx` applies the stored choice before `createRoot`. Under the CSP, `ui/index.html` holds no inline script, so the only page that can show a wrong theme first is the empty one before the entry module runs.
  - **Scope.** The choice is per viewer, meaning per browser profile, as the language is. A hub keeps no theme.
- **The dark theme moves** from navy to neutral greys, so both themes share one hierarchy of surfaces (W4). A status role keeps its dark value wherever that value passes the matrix ("Data", "Contrast matrix").
- **The canvases.** Flow's and Graph's `--xy-*` variables already come from tokens, so they follow the theme. `@xyflow/react`'s `colorMode` is never set.

### Typography

- **Inter.** It is bundled as four committed `woff2` subsets of its variable weight axis (100-900, upright): `latin`, `latin-ext`, `cyrillic` and `cyrillic-ext`. They live in `ui/fonts/inter/`, beside `OFL.txt`, the SIL Open Font License 1.1 text, and are declared in `ui/src/styles/fonts.css` (W1).
  - **How Vite treats them.** Vite emits the files to `ui/dist/assets/`, which `serve-ui.md` serves as `font/woff2`, so they are neither JS nor first-load JS.
  - **What a page fetches.** Each face carries its subset's `unicode-range`, so a page fetches only the subsets its text uses: an English page takes `latin`, and a Russian one adds `cyrillic`.
  - **Fallback.** `font-display: swap` keeps text visible while a file loads. A glyph outside the four subsets falls back to the system stack, glyph by glyph. Italic is synthesised.
- **The stack.** `--font-sans` becomes `"Inter", system-ui, -apple-system, "Segoe UI", "Noto Sans", Roboto, "Helvetica Neue", Arial, sans-serif`. `--font-mono` is unchanged.
- **The scale**, in rem with sizes and line heights paired:

  | Step | Size/line | For |
  |---|---|---|
  | small | 12/16 | secondary text, field labels, lozenges, badges, kbd |
  | body | 14/20 | the chrome: lists, buttons, fields, tiles, properties |
  | document | 16/24 | a document's or a task's text: goal, criteria, at most 68ch |
  | heading-s | 16/20 | a section's title inside a view |
  | heading-m | 20/24 | `h2`: a modal's or a panel's title |
  | heading-l | 24/28 | `h1`: a view's or a page's title |
  | code | 12/20 | inline and block code in the chrome |

  Weights are 400, 500, 600 and 700. Inside components, bold body text stands in for a heading style.

### Space, shape, size

- **Spacing** is on an 8 px base: 2, 4, 6, 8, 12, 16, 20, 24, 32, 40 and 48 px. Radii are 2 px (small marks), 4 px (lozenges, badges, kbd), 6 px (buttons, fields), 8 px (tiles, panels, menus) and 12 px (dialogs), plus `full` for round dots. Borders are 1 px, and focus and selection are 2 px.
- **Controls** are 32 px tall, or 24 px compact. The 24 px target minimum stays.
- **Layout sizes:**
  - the header is 48 px;
  - the work item's side column is at most 330 px, beside a main column of at least 320 px;
  - dialogs are 600 px (the default), 800 px or 968 px wide (the card modal);
  - an empty state is at most 464 px wide;
  - reading text is at most 68ch wide.

### Primitives (`ui/src/ui/`)

- **Button.** Five appearances: default, primary, subtle, warning and danger. Two spacings: 32 px, and 24 px compact. It renders today's classes (`button`, `button-primary`, `button-quiet`, `button-danger`) plus the new `button-warning` and `button-compact`, so every existing `.button` takes the new metrics from one rule. At most one primary stands per area: a dialog, a view's header acts, a side column.
- **Lozenge.** A status as a label plus an icon, never colour alone, in six appearances: neutral, information, success, warning, danger and discovery.
  - Its look is 20 px tall, with 12/16 text at weight 500 in sentence case, radius 4, and padding 0 4 px, on the appearance's surface with its text colour. Its maximum width is 200 px.
  - A longer label wraps rather than being cut (W11).
- **PropertyList.** A `dl` of label and value rows, the labels 12/16 secondary and the values 14/20. With `foldEmpty`, rows whose value is `null` leave the list and stand under one "More fields (n)" disclosure, collapsed, each reading "None".
- **Breadcrumbs.** A `nav` labelled "Breadcrumbs" holding an `ol`. Every item but the last is a link from `routes.ts`, at least 24 px tall. The last item is the current one, text with `aria-current="page"` and no `href`. Separators are `/`, `aria-hidden`.
- **IssueView.** The work-item layout, a size container:
  - a head: breadcrumbs and tools on one line, the caller's title (`h1` on a page, `h2` in a modal), then the acts;
  - a body: a main column, then a side column at most 330 px wide.

  Under 42.5rem of its own width the body is one column, main first. The DOM order is main then side at every width, so reading and focus order never jump.
- **EmptyState.** A heading (a level the caller picks), a short description, and up to two acts, one of them primary. It is at most 464 px wide, never worded as a failure, and sentence case.
- **A disclosure helper** shared by "More fields" and Development: a `button` with `aria-expanded` and `aria-controls`, whose region is `hidden` while collapsed. It does not move focus on toggle, and it animates only through the duration tokens.

### Status categories

A card's status is its column. Its category comes from the column's position in its served flow, never from a column word (`src/board/category.ts`; `policy.test.ts` still holds). The first rule that matches wins:

| Rule | Category | Appearance | Icon |
|---|---|---|---|
| the flow is not served, or the column is not in its `columns` | stranded | neutral | `unknown` |
| the column is `accept.to` | done | success | `taskAccepted` |
| the column is the flow's `return` | returned | warning | `cardReturned` |
| the column is `columns[0]` | to do | neutral | `open` |
| any other column | in progress | information | `taskInProgress` |

The label is the column's served `title`, or the raw ID for a stranded column. Cancellation leaves the category as it is: the existing Cancelled label says it.

Example, the mock's `plates` flow: `queued` is to do, `blocked` is returned, `checked` is in progress, `struck` is done, and a row left in an unlisted `inked` column is stranded. A one-column flow is done. A flow whose `return` is its first column reads returned there.

### The work item: the task modal and page

`CardModal` and `CardPage` render one `IssueView`, the modal at 968 px. The routes, history, focus, inert background, write lock, refusals and dialogs are those of `board-screen.md` "The card modal and page", unchanged.

- **Head.**
  - Breadcrumbs: Project (the switcher's label, `#/<p>`), then Board (`app.sections.board`, `#/<p>/board`), then the parent card's ID if there is one (its modal, replacing the entry as a card link does today), then the card's ID as the current item.
  - In the modal, "Open as a page" and Close sit on the breadcrumb line as subtle buttons; on the page, "Open on the board" sits there.
  - The title: the modal's `h2` (`focusTitle`) or the page's `h1`.
  - The acts, in this order: Accept, primary, only in `accept.from`; Hand off; Return…; Edit; Assign; Cancel card, subtle and last. All are default unless named.
- **Main column, top to bottom:**
  - the mark and Clear, the latest return reason, the acceptance and the cancellation, each where the card has one;
  - Goal and Criteria at the document step;
  - Source, the targets;
  - Hand-offs, the subtasks;
  - Evidence;
  - History.

  Each keeps its current content and words.
- **Hand-offs rows.** Each row shows the child's ID link, title, flow name, a status Lozenge (the child's flow and column), the assignee and the rollup word. The raw `<flow>/<column>` text leaves the row.
- **Side column, top to bottom:**
  - **Status:** the card's Lozenge, then the existing Move menu button (compact, its name unchanged); a closed card shows no Move.
  - **Details** (`PropertyList`, labelled `board.panel.details`), in this order: Assignee, Reviewer, Flow, Due, Estimate, Discipline, Parent, Corrects, Created by, Created at. An empty field goes under "More fields (n)" in the same order.
  - **Development:** a disclosure, collapsed by default, whose button reads "Development · n agent tasks" ("No agent tasks" at 0). Expanding it shows the Tasks part of `card-tasks.md` "Screen" as it is: rows, notes, the hub's sentence. Its empty state ("No task yet.", how to make and link one, the commands with Copy) is drawn by `EmptyState`.

### The Board

- **Column heads.** Each column head's title becomes the column's category Lozenge, with the count after it as now. The stranded "Not in the flow" head is a neutral Lozenge with the `unknown` icon. Heads keep their height: no head moves on open, close, a write or a filter (`board-screen.md` "The grid").
- **Tiles.** A tile adds no status lozenge, since its column says the status once (W6). Its badges keep their words, icons, order and tones: a mark, Returned, Accepted, Cancelled, Overdue, the returned count. `.badge` takes the lozenge's metrics everywhere: 20 px, radius 4, 12/16 at weight 500. Its label is in `--text-primary`, its icon and 1 px border in the tone, and dashed or dotted borders keep their meaning (W7).
- **Lane heads.** A Parent lane head shows the parent's flow name and its status Lozenge in place of the raw `<flow>/<column>`.
- **Controls.** The toolbar's tabs, selects, the Lanes select and the Acting-as field take the 32 px control height.

### The header

The header is one row 48 px tall, with every control 32 px and its text 14/20. The brand comes first, then Project, Workspace, the config notes, Language, Theme, the mock indicator, Jump to and Shortcuts.
- **Labels.** The selects show their value, with an icon in front; their labels stay as visually hidden text, so every accessible name is unchanged (W9).
- **Narrower screens.** Below 80rem, Jump to and Shortcuts show their icon alone, and their names stay. Below 68.75rem the header may wrap, each row 48 px.
- **The nav's** items are 32 px tall, 14/20, and the current one is in accent text on `--surface-selected`.

### Guards and staging

A guard test fails on any raw font size, line height, spacing or radius in any CSS file under `ui/src`, and on any inline `style` that sets one. Only listed exceptions pass, each with its reason and the item that removes it ("Data", "Exceptions").

What each slice migrates:
- **This slice:**
  - `tokens.css`;
  - the base rules (`body`, headings, `code`, `kbd`, links, focus);
  - the global `.button`, `.field`, `.badge` and `.tab`;
  - the shell, header and nav;
  - the Board;
  - the card modal and page, and `src/ui/`.

  Every other raw value maps to its step where the mapping is mechanical (11 px or 12 px to small, a 2 px gap to `--space-025`, 999 px to `--radius-full`, 2 px to `--radius-xs`); otherwise it is listed.
- **The aliases.** The legacy names `--font-size-s|m|l|xl`, `--line-height-tight` and `--space-1..6` stay as aliases onto the scale, so the other screens' CSS text is untouched and their pinned tests hold. Their sizes still land on the scale: 13 px becomes 14, 15 becomes 16 and 18 becomes 20 (W10).
- **Later slices.** RM-0099 renames the legacy uses screen by screen, drops the aliases and empties the exceptions it is named in. RM-0094 does the same for Flow, RM-0095 for the document modal.

### Words

New strings are catalog keys in both languages ("Data", "Catalog keys"). Of the owner's words, only "agent task" is needed here: in the Development summary. The others (Task, Subtask, Department) and every rename of an existing string belong to RM-0096. "Hand-offs", "Tasks", "Workspace" and "Discipline" keep their keys and words here.

## Data

### `tokens.css`

```css
/* One block of roles; a colour role is light-dark(light, dark), a size has no theme. */
:root {
  color-scheme: light dark;
  --surface-canvas: light-dark(#ffffff, #1f1f21);
  --text-primary: light-dark(#292a2e, #cecfd2);
  --task-done: light-dark(#1f845a, #4cc995);          /* a status role: both values */
  --proposal-open: var(--accent);                      /* an alias: follows its target per theme */
  --scrim: light-dark(rgba(5, 12, 31, 0.46), rgba(16, 18, 20, 0.6));
  --shadow-raised: 0 1px 1px var(--shadow-key), 0 0 1px var(--shadow-ambient);
  --font-size-body: 0.875rem;
  --line-height-body: 1.25rem;
  --space-100: 0.5rem;
  --space-2: var(--space-100);                          /* legacy alias until RM-0099 */
  /* ... */
}
:root[data-theme="light"] { color-scheme: light; }
:root[data-theme="dark"] { color-scheme: dark; }
@media (prefers-reduced-motion: reduce) { :root { /* the durations, 0ms, as now */ } }
```

- **Where colour lives.** A declaration that holds a colour literal is `light-dark(L, D)`, each side a literal or a `var()`; a `var()` alias of a role is theme-free. `light-dark(` appears in no other file. `ui_policy.rs` keeps colour literals to this file.
- **What stays.** Every existing role name stays. The light side is new: each literal role gets a light value that passes the matrix. A role of today's tests that must differ from another (row 4's nine states, the four flow edges, the trace states, cannot-verify against ok) differs in both themes.

### Roles and starting values

The values are the starting palette. A value may move to pass a pair; a role and its pair may not.

| Role | Light | Dark | New |
|---|---|---|---|
| `--surface-canvas` | `#ffffff` | `#1f1f21` | |
| `--surface-sunken` (board columns, wells, code) | `#f8f8f8` | `#18191a` | yes |
| `--surface-panel` (header, nav) | `#ffffff` | `#1f1f21` | |
| `--surface-raised` (tiles, with `--shadow-raised`) | `#ffffff` | `#242528` | |
| `--surface-overlay` (dialogs, menus, with `--shadow-overlay`) | `#ffffff` | `#2b2c2f` | |
| `--surface-hovered` | `#f0f1f2` | `#303134` | yes |
| `--surface-selected` (the nav's current, a chosen row) | `#eef0ff` | `#2b2e4a` | yes |
| `--border-subtle` (dividers, no contrast duty) | `#dfe1e6` | `#3d3f43` | |
| `--border-control` | `#7d818a` | `#8c8f97` | |
| `--focus-ring` | `#4f46e5` | `#a5b4fc` | |
| `--text-primary` | `#292a2e` | `#cecfd2` | |
| `--text-secondary` | `#505258` | `#a9abaf` | |
| `--text-subtlest` | `#63666e` | `#96999e` | yes |
| `--accent` (links, primary, the nav's current) | `#4f46e5` | `#a5b4fc` | |
| `--accent-hovered` | `#4338ca` | `#c7d2fe` | yes |
| `--text-on-accent` | `#ffffff` | `#1f1f21` | |
| `--warning-bold`, `--text-on-warning` | `#fbc828`, `#292a2e` | `#fbc828`, `#1f1f21` | yes |
| `--danger-bold`, `--text-on-danger` | `#c9372c`, `#ffffff` | `#fd9891`, `#1f1f21` | yes |
| `--lozenge-neutral-surface`, `-text` | `#f0f1f2`, `#292a2e` | `#303134`, `#cecfd2` | yes |
| `--lozenge-information-surface`, `-text` | `#e9f2fe`, `#1558bc` | `#1c2b42`, `#8fb8f6` | yes |
| `--lozenge-success-surface`, `-text` | `#efffd6`, `#4c6b1f` | `#28311b`, `#b3df72` | yes |
| `--lozenge-warning-surface`, `-text` | `#fff5db`, `#9e4c00` | `#3a2c1f`, `#fbc828` | yes |
| `--lozenge-danger-surface`, `-text` | `#ffeceb`, `#ae2e24` | `#42221f`, `#fd9891` | yes |
| `--lozenge-discovery-surface`, `-text` | `#f8eefe`, `#803fa5` | `#35243f`, `#d8a0f7` | yes |
| `--diff-added-surface`, `--diff-removed-surface` | `#dcfff1`, `#ffeceb` | `#123628`, `#3d1c25` | |
| `--highlight-surface` | `#fff7d6` | `#4a3a10` | |
| `--scrim` | `rgba(5, 12, 31, 0.46)` | `rgba(16, 18, 20, 0.6)` | |
| `--shadow-key`, `--shadow-ambient` | `#1e1f2140`, `#1e1f214f` | `#03040480`, `#0304047a` | yes |

- **Shadows.** `--shadow-raised` is `0 1px 1px var(--shadow-key), 0 0 1px var(--shadow-ambient)`, and `--shadow-overlay` is `0 8px 12px var(--shadow-key), 0 0 1px var(--shadow-ambient)`.
- **The accent** is indigo, SpecEngine's own hue (W3). It is told apart from the information blue of an in-progress lozenge.
- **Status roles in light.** Every other literal role (attention, severity, proposal, task, run, sync, highlight line, graph, flow, card, trace) takes a light value from its family, each at least 3:1 on the five surfaces. The families: ok and done `#1f845a`; attention `#a54800`; high and conflict `#c9372c`; information `#1d6ce0`; cannot-verify and discovery `#8f46c1`; neutral and unknown `#6b6e76`; and the extra hues (cyan, pink, teal, orange) at the same contrast.
- **Status roles in dark.** These keep today's values. `--graph-minimap-node` (2.91:1 on the dark overlay) takes `#7187ad`.

### Contrast matrix

`tokens.test.ts` checks every pair in both themes, on the resolved values. "Surfaces*" means canvas, sunken, panel, raised and overlay, plus hovered and selected for text. The ratios are the starting values' lowest.

| Foreground | Background | Min | Light | Dark |
|---|---|---|---|---|
| `--text-primary` | surfaces* | 4.5 | 12.66 | 8.35 |
| `--text-secondary` | surfaces* | 4.5 | 6.90 | 5.66 |
| `--text-subtlest` | surfaces* | 4.5 | 5.07 | 4.55 |
| `--accent` | surfaces* | 4.5 | 5.55 | 6.52 |
| `--text-on-accent` | `--accent`, `--accent-hovered` | 4.5 | 6.29 | 8.25 |
| `--text-on-warning` | `--warning-bold` | 4.5 | 9.14 | 10.50 |
| `--text-on-danger` | `--danger-bold` | 4.5 | 5.16 | 7.90 |
| `--lozenge-<a>-text`, each of six | `--lozenge-<a>-surface` | 4.5 | 5.54 | 6.81 |
| `--text-primary`, `--text-secondary` | each lozenge surface (banners) | 4.5 | 6.86 | 5.66 |
| `--text-primary` | `--diff-added-surface`, `--diff-removed-surface`, `--highlight-surface` | 4.5 | 12.59 | 7.08 |
| `--focus-ring` | surfaces* | 3 | 5.55 | 6.52 |
| `--border-control` | surfaces* | 3 | 3.45 | 4.02 |
| every status role of today's `STATUS` list | the five surfaces | 3 | the developer's values | today's, but `--graph-minimap-node` |

### Size tokens

```css
--font-sans: "Inter", system-ui, -apple-system, "Segoe UI", "Noto Sans", Roboto, "Helvetica Neue", Arial, sans-serif;
--font-size-small: 0.75rem;     --line-height-small: 1rem;
--font-size-body: 0.875rem;     --line-height-body: 1.25rem;      /* was the ratio 1.5, used by body only */
--font-size-document: 1rem;     --line-height-document: 1.5rem;
--font-size-heading-s: 1rem;    --line-height-heading-s: 1.25rem;
--font-size-heading-m: 1.25rem; --line-height-heading-m: 1.5rem;
--font-size-heading-l: 1.5rem;  --line-height-heading-l: 1.75rem;
--font-size-code: 0.75rem;      --line-height-code: 1.25rem;
--font-weight-regular: 400; --font-weight-medium: 500; --font-weight-semibold: 600; --font-weight-bold: 700;
--space-025: 0.125rem; --space-050: 0.25rem; --space-075: 0.375rem; --space-100: 0.5rem; --space-150: 0.75rem;
--space-200: 1rem; --space-250: 1.25rem; --space-300: 1.5rem; --space-400: 2rem; --space-500: 2.5rem; --space-600: 3rem;
--radius-xs: 2px; --radius-s: 4px; --radius-control: 6px; --radius-m: 8px; --radius-l: 12px; --radius-full: 999px;
--control-height: 2rem; --control-height-compact: 1.5rem; --target-min: 24px; --nav-height: 3rem;
--lozenge-height: 1.25rem; --lozenge-max-width: 12.5rem; --reading-width: 68ch; --empty-state-width: 29rem;
--issue-side-width: 20.625rem; --issue-main-min: 20rem;
--modal-width-s: 37.5rem; --modal-width-m: 50rem; --modal-width-l: 60.5rem;
/* legacy aliases, removed by RM-0099 */
--font-size-s: var(--font-size-body); --font-size-m: var(--font-size-heading-s);
--font-size-l: var(--font-size-heading-m); --font-size-xl: var(--font-size-heading-l); --line-height-tight: 1.25;
--space-1: var(--space-050); --space-2: var(--space-100); --space-3: var(--space-150);
--space-4: var(--space-200); --space-5: var(--space-300); --space-6: var(--space-400);
```

**The scale sets.** These are what the guard reads, each token resolved through aliases:
- sizes: 0.75, 0.875, 1, 1.25 and 1.5 rem;
- line heights: 1, 1.25, 1.5 and 1.75 rem, with `--line-height-tight` the one legacy ratio;
- spaces: 0.125, 0.25, 0.375, 0.5, 0.75, 1, 1.25, 1.5, 2, 2.5 and 3 rem;
- radii: 2, 4, 6, 8 and 12 px, and 999 px.

### `fonts.css` and `ui/fonts/inter/`

```css
/* Inter <version> (SIL OFL 1.1, ui/fonts/inter/OFL.txt), the variable weight axis, upright:
   docs/features/design-foundation.md "Typography". */
@font-face {
  font-family: "Inter";
  font-style: normal;
  font-weight: 100 900;
  font-display: swap;
  src: url("../../fonts/inter/inter-latin-wght-normal.woff2") format("woff2");
  unicode-range: U+0000-00FF, U+0131, U+0152-0153; /* ...the package's own range, copied whole */
}
/* three more faces: latin-ext, cyrillic, cyrillic-ext */
```

- **The files.** `ui/fonts/inter/` holds `inter-{latin,latin-ext,cyrillic,cyrillic-ext}-wght-normal.woff2` and `OFL.txt`, about 180 kB in all.
- **Where they come from.** They are taken from one `@fontsource-variable/inter` release at least 7 days old (the pin rule's age), as its files. That package is the OFL build of Inter, split into subsets. Each face's `unicode-range` is copied from the package's `wght.css`.
- **Provenance.** The release's version and each file's SHA-256 go in the header comment and, at ship, in `ui/README.md`.
- **No package.** The package is downloaded once and never installed: `package.json` and the lockfile are unchanged, and the allowlist stays at 19.
- **Why outside `ui/src`.** `ui_policy.rs` reads every file under `ui/src` as text.
- **Imports.** `src/main.tsx` imports `./styles/fonts.css` before `tokens.css`. No other file declares `@font-face`. No CSS holds a `url()` to `http:`, `https:` or `//`, or an `@import url(`.

### Theme choice

| Where | What |
|---|---|
| `localStorage["specengine.theme"]` | `"light"` or `"dark"`; absent: System. Another value, or a storage that throws, reads System. |
| `<html data-theme>` | `light` or `dark` while chosen; absent under System |
| `ui/index.html` | `<meta name="color-scheme" content="light dark" />` |
| `src/app/theme.ts` | `THEMES = ["system", "light", "dark"]`, `THEME_KEY`, `storedTheme(): Theme`, `applyTheme(theme)`, `chooseTheme(theme)` (store, then apply) |
| `src/app/ThemeSwitcher.tsx` | the header's select, after Language; options in `THEMES` order |

### Component APIs

```ts
// src/ui/Button.tsx
export type ButtonAppearance = "default" | "primary" | "subtle" | "warning" | "danger";
export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className"> {
  appearance?: ButtonAppearance;      // "default"
  spacing?: "default" | "compact";    // 32 px or 24 px
  icon?: IconName;                    // before the label
  className?: string;                 // layout only (e.g. `board-move`), never a look
}                                     // type defaults to "button"

// src/ui/Lozenge.tsx
export type LozengeAppearance = "neutral" | "information" | "success" | "warning" | "danger" | "discovery";
export function Lozenge(props: { appearance: LozengeAppearance; icon: IconName; label: string; lang?: "en" }): JSX.Element;
// <span class="lozenge" data-appearance="success"><svg aria-hidden/><span class="lozenge-label">struck</span></span>

// src/ui/PropertyList.tsx
export interface Property { key: string; label: string; value: ReactNode | null }
export function PropertyList(props: { label: string; items: readonly Property[]; foldEmpty?: boolean }): JSX.Element;

// src/ui/Breadcrumbs.tsx
export interface Crumb { key: string; label: string; href?: string; mono?: boolean }   // the last: current, no href
export function Breadcrumbs(props: { items: readonly Crumb[] }): JSX.Element;

// src/ui/IssueView.tsx
export function IssueView(props: {
  breadcrumbs: ReactNode; tools?: ReactNode; title: ReactNode; actions?: ReactNode;
  main: ReactNode; side: ReactNode;
}): JSX.Element;

// src/ui/EmptyState.tsx
export function EmptyState(props: {
  title: string; level: 2 | 3 | 4; description?: ReactNode; primary?: ReactNode; secondary?: ReactNode;
}): JSX.Element;

// src/board/category.ts
export type StatusCategory = "todo" | "doing" | "done" | "returned" | "stranded";
export function categoryOf(flow: CardFlow | undefined, column: string): StatusCategory;
export function statusLook(flow: CardFlow | undefined, column: string): { appearance: LozengeAppearance; icon: IconName; label: string };
```

New icons, hand-drawn at 16 px as the others are: `themeSystem`, `themeLight`, `themeDark`, `language`, `project` and `workspace`.

### Exceptions

```ts
// src/styles/scale-exceptions.ts
export interface ScaleException {
  file: string;         // "src/styles/app.css"
  selector: string;     // the rule's selector, whitespace collapsed
  property: string;     // "font-size", "line-height", "gap", "margin-bottom", "border-radius", ...
  value: string;        // as written
  reason: string;       // why it is not a token
  until: "RM-0094" | "RM-0095" | "RM-0099" | null;   // the item that removes it; null: kept
}
export const SCALE_EXCEPTIONS: readonly ScaleException[];
```

Kept (`until: null`):
- `.sr-only`'s `margin: -1px`, the visually hidden pattern;
- a 1 px negative margin that lays a border over a border, which is not space.

Everything else names an item. Examples: Graph's and Flow's canvas labels, sized against the hand-written layout's boxes (RM-0094 for Flow, RM-0099 for Graph); the tree's `1.1rem` indent per depth (RM-0099).

### Catalog keys

New, in `en.ts` and `ru.ts`:

| Key | English |
|---|---|
| `theme.label` | Theme |
| `theme.system`, `theme.light`, `theme.dark` | System, Light, Dark |
| `ui.breadcrumbs` | Breadcrumbs |
| `ui.moreFields` | More fields ({{n}}) |
| `ui.emptyValue` | None |
| `board.item.status` | Status |
| `board.item.flow` | Flow |
| `board.item.development` | Development |
| `board.item.agentTasks_one`, `_other` | {{count}} agent task, {{count}} agent tasks |
| `board.item.noAgentTasks` | No agent tasks |

The Russian for "agent task" builds on the owner's word for a task: `\u0437\u0430\u0434\u0430\u0447\u0430 \u0430\u0433\u0435\u043d\u0442\u0430`, with its plural forms. Everything else reuses keys: `board.panel.*`, `board.modal.*`, `board.tasks.*`, `app.sections.board`, `language.label`, `board.notInTheFlow`.

## Rules and edge cases

- WHEN no theme is stored, the page SHALL take the system's scheme, light when the system states none, before any script runs; WHEN the system's scheme changes, the page SHALL follow at once.
- WHEN the viewer picks Light or Dark, the UI SHALL store it and set `data-theme` at once, and on every later load before the first render; WHEN they pick System, it SHALL remove both.
- WHEN storage throws or holds another value, the theme SHALL be System and nothing SHALL be shown or logged (a test fails on `console.error` or `console.warn`).
- WHEN a font file fails to load, or a glyph lies outside the four subsets, text SHALL render in the system stack and never be invisible.
- WHEN a column title is long, its Lozenge SHALL wrap inside 200 px and never cut, and its height SHALL grow by whole 16 px lines. WHEN a flow is not served, the Lozenge SHALL read the raw column ID.
- WHEN every optional field of a card is filled, no "More fields" SHALL show; WHEN none is, Details SHALL hold Flow, Created by and Created at.
- WHEN a card links no task, Development SHALL read "No agent tasks"; on a hub, its rows SHALL show IDs without state, as `card-tasks.md` "Screen" says.
- WHEN a dialog, a view's header acts or a side column holds buttons, at most one SHALL be primary.
- WHEN the IssueView is narrower than 42.5rem, it SHALL show one column, main then side; under 36rem the modal SHALL take the screen, as now.
- WHEN a disclosure toggles, focus SHALL stay on its button; under reduced motion nothing SHALL animate.
- **Unchanged.** Nothing here is a hold on work (ADR-0012). No status is told by colour alone. No column, flow or event word is quoted in `src/board/`. The CSP stays clean: no inline script or style in `index.html`, and no `data:` font, so Vite's 4 kB inline limit stays below every subset.

### Test churn

Accessible names, roles and catalog keys stay as they are. Only these tests are replaced; every other existing test passes unchanged.

| Test | Becomes | Why |
|---|---|---|
| `src/styles/tokens.test.ts`, whole | per-theme resolution of `light-dark()`; the matrix in both themes; today's distinctness checks per theme; the new roles required | one dark `:root` block is no longer the file's shape |
| `src/board/styles.test.ts` "keeps the card modal inside the viewport: at most 48rem wide…" | `min(var(--modal-width-l), 100vw - 2rem)` | the work item needs a main column beside 330 px |
| `src/board/CardModal.test.tsx` "lists each child with ID, title, <flow>/<column>, assignee and word…" | the flow name and a category Lozenge | the raw pair leaves the row |
| `src/board/CardModal.test.tsx` "lists each linked task after Hand-offs, in the scrolling body…" | the Tasks part inside Development, collapsed by default | folded by design |
| `src/board/CardTasks.test.tsx` "in the modal, right after Hand-offs, a row per link in link order" | rows in link order after expanding Development | as above |
| `src/board/CardPage.test.tsx` "shows the card's Tasks part, as the modal does…" | expands Development first | as above |
| `src/mocks/board.smoke.test.tsx`, the step that finds the "Tasks" region in the open modal | expands Development first | as above |
| `src/board/BoardView.test.tsx` "Parent lanes: each head names its parent… and where it stands…" | the flow name and the parent's Lozenge | the raw pair leaves the head |
| a test asserting the modal body's field order | the IssueView's order | the order is DF-6's |

Tests cite this spec's headings (`doc_pointers.rs`), not `docs/canon/ui-design.md`, which exists only from ship on.

## Acceptance criteria

A browser look is marked **[look]**: the owner's or a role's eyes in Chrome DevTools, recorded in "Implementation" with the date. Every other criterion is a test that the named mutation turns red.

- [ ] DF-1 — **Type scale.** `src/styles/scale.test.ts` (new) reads every `src/**/*.css` but `tokens.css` (`import.meta.glob`, `?raw`). Each `font-size` and `line-height` must be one `var()` of a `--font-size-*` or `--line-height-*` defined in `tokens.css`, or an entry of `SCALE_EXCEPTIONS`. A `font` shorthand may only be `inherit`. In `tokens.css`, every size and line-height token must resolve into "Data", "Size tokens"'s sets. Every exception must match at least one declaration. Mutations, each red: `font-size: 13px` in `.board-card-title`; `--font-size-body: 0.8125rem`; deleting a canvas label's exception; an exception that matches nothing.
- [ ] DF-2 — **Spacing.** The same test: `padding*`, `margin*`, `gap`, `row-gap` and `column-gap` hold no non-zero length literal outside `var()` of a `--space-*` (`0`, `auto` and `calc()` of tokens and plain numbers pass), or they are listed. Every `--space-*` resolves into the set. No `style={{…}}` in a non-test `.tsx` sets `fontSize`, `lineHeight`, `padding*`, `margin*`, `gap` or `borderRadius`. Mutations, each red: `gap: 3px` in `.board-card-top`; `style={{ padding: 4 }}` in `BoardCard.tsx`; `--space-150: 0.7rem`.
- [ ] DF-3 — **Radii and control heights.** `border-radius` and `border-*-radius` are `0` or `var(--radius-*)` only, and the tokens resolve into the set. `.button`'s and `.field`'s `min-height` is `var(--control-height)`, and `.button-compact`'s is `var(--control-height-compact)`. Mutations, each red: `border-radius: 5px` on `.lozenge`; `--radius-m: 10px`; `.button { min-height: 2.25rem }`.
- [ ] DF-4 — **Buttons, one primary per area.** `src/ui/Button.test.tsx` (new): each appearance and spacing gives exactly its classes, and `type` is `button` unless given. `src/board/workItem.test.tsx` (new), over the mock board: in the card modal, the card page, and each dialog they open (Hand off, Return, Edit, Assign, Cancel, Accept, Clear), at most one `.button-primary` per `role="dialog"`, per header acts and per side column. Accept is the primary only in `accept.from`. Mutations, each red: Hand off as `primary`; Button dropping `type="button"`.
- [ ] DF-5 — **Lozenges by category.** `src/board/category.test.ts` (new) runs the "Status categories" table over a flow `[queued, blocked, checked, struck]` with `accept {checked → struck}` and `return: blocked`: queued, blocked, checked, struck and `inked` give to do, returned, in progress, done and stranded. A one-column flow gives done; `return` equal to the first column gives returned. `src/ui/Lozenge.test.tsx` (new): a label, an `aria-hidden` icon, and `data-appearance`, with no text cut (no `text-overflow` in `.lozenge`). `BoardView.test.tsx`: each column head shows its Lozenge, with appearance and label per the table, and the stranded head is neutral with the `unknown` icon. The card's side Status and each Hand-offs row show the same look as `statusLook` gives. Mutations, each red: swapping the `accept.to` and `return` rules; a Lozenge without its icon; quoting `"blocked"` in `src/board/category.ts` (`policy.test.ts`).
- [ ] DF-6 — **Work-item order.** `workItem.test.tsx`, on a card with every field filled: the side column's labels read in DOM order Status, Assignee, Reviewer, Flow, Due, Estimate, Discipline, Parent, Corrects, Created by, Created at. The main column's sections read: the card's blocks, Goal, Criteria, Source, Hand-offs, Evidence, History. On a card with assignee, due and estimate `null`: Details lacks the three; "More fields (3)" has `aria-expanded="false"`; expanding it lists them in that order, each "None". `src/ui/PropertyList.test.tsx` (new) covers `foldEmpty`. `.issue-view-side`'s `max-width` is `var(--issue-side-width)` (20.625rem). Mutations, each red: Flow before Reviewer; empty rows left in Details; side width `22rem`.
- [ ] DF-7 — **Development folded.** `workItem.test.tsx`: the side column's Development button has `aria-expanded="false"` and reads "Development · 2 agent tasks" for two links. The Tasks region is not in the accessibility tree until the button is clicked. Then the rows of `card-tasks.md` "Screen" are there; with no link, `EmptyState` holds "No task yet." and both commands with Copy. `src/ui/EmptyState.test.tsx` (new): the heading at its level, at most two acts, at most one primary. Mutations, each red: opened by default; the summary counting rows with a `null` status out.
- [ ] DF-8 — **Breadcrumbs.** `src/ui/Breadcrumbs.test.tsx` (new): a `nav` named "Breadcrumbs" around an `ol`; every item but the last is an `a` with its `href`; the last has `aria-current="page"` and no `href`; the separators are `aria-hidden`. `workItem.test.tsx`: the modal and the page show Project (`#/<p>`), Board (`#/<p>/board`), the parent (`#/<p>/board/<parent>`, when there is one) and the card's ID as current. Mutations, each red: dropping `aria-current`; linking the current item; a parent crumb on a card with none.
- [ ] DF-9 — **Contrast in both themes.** `tokens.test.ts`, rewritten: every literal-holding role is `light-dark(L, D)`, both sides six-digit hex or `var()`. The roles that take alpha are excepted by name: `--scrim`, `--shadow-key`, `--shadow-ambient`, `--graph-minimap-mask` and `--graph-selection-surface`. `light-dark(` appears in no other CSS file. Every pair of "Data", "Contrast matrix" passes in the light and the dark theme. Today's distinctness assertions hold per theme. Mutations, each red: light `--text-secondary` to `#8c8f97`; dark `--lozenge-success-text` to `#4c6b1f`; `--attention` as a plain hex; `light-dark(` in `app.css`.
- [ ] DF-10 — **[look] Measured metrics.** In Chrome DevTools at 1440x900 and at 1100x800, in English and Russian, light and dark (eight looks), on the mock's Board, a card modal with every field and two tasks, the card page and the header, the console snippet below gives:
  - font sizes only 12, 14, 16, 20 and 24 px;
  - control heights only 24 and 32;
  - no target under 24x24, but inline links in sentences;
  - lozenges 20 px tall;
  - the side column at most 330 px;
  - the modal 968 px wide, or the viewport less 32.

  Each look is judged against the owner's words "professional, consistent" and recorded with a screenshot name. Mutation: `.button { min-height: 2.25rem }` makes the snippet report 36.
  ```js
  (() => {
    const shown = (e) => !e.closest(".sr-only") && e.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true });
    const all = [...document.querySelectorAll("body *")].filter(shown);
    const sizes = [...new Set(all.filter((e) => [...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())).map((e) => getComputedStyle(e).fontSize))].sort();
    const heights = [...new Set(all.filter((e) => e.matches("button, select, input:not([type=checkbox]):not([type=radio])")).map((e) => Math.round(e.getBoundingClientRect().height)))].sort();
    const small = all.filter((e) => e.matches("a[href], button, select, input, [tabindex='0']")).filter((e) => { const r = e.getBoundingClientRect(); return r.width < 24 || r.height < 24; }).map((e) => e.outerHTML.slice(0, 80));
    console.log({ sizes, heights, small });
  })();
  ```
- [ ] DF-11 — **Catalogs, gates, size, churn.**
  - Every new string is a key of "Data", "Catalog keys" in `en.ts` and in `ru.ts`, where `tsc` makes Russian complete.
  - `app/screens.language.test.tsx` renders the Board, the modal with Development and More fields open, and the header in Russian, with no new English left.
  - `pnpm lint`, `pnpm build` and `pnpm test` are green; so are `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers` and the docs gate.
  - First-load JS, measured as `ui-languages.md` "Size", is at most 845 kB. It is printed with gzip in "Implementation", beside the font files' bytes and first-load CSS.
  - Only the tests of "Test churn" change.

  Mutations, each red: a `ru.ts` key removed (`tsc`); "More fields" written as a JSX literal (the catalog's prose rule).
- [ ] DF-12 — **Theme.** `src/app/theme.test.tsx` (new):
  - with nothing stored, `<html>` has no `data-theme`;
  - choosing Dark in the header's Theme select sets `data-theme="dark"` and stores `"dark"` under `specengine.theme`;
  - a fresh `applyTheme(storedTheme())` restores it;
  - System removes both;
  - `"sepia"`, or a `localStorage` that throws, reads System without a console line;
  - the select's name is "Theme", and its options are System, Light and Dark in both languages.

  `tokens.test.ts`: `:root` holds `color-scheme: light dark`, and the two `[data-theme]` blocks pin `light` and `dark`. `index.html`'s meta reads `light dark`. Mutations, each red: defaulting to dark with nothing stored; another storage key; `color-scheme: dark` in `:root`.
- [ ] DF-13 — **Inter bundled.** `src/styles/fonts.test.ts` (new): `fonts.css` declares exactly four `@font-face` for `"Inter"`, each with `font-weight: 100 900`, `font-style: normal`, `font-display: swap`, a `unicode-range`, and a relative `url()` into `ui/fonts/inter/` whose `?raw` text starts with `wOF2`. `OFL.txt` contains `SIL OPEN FONT LICENSE Version 1.1`. `--font-sans` starts with `"Inter",`. No other CSS file holds `@font-face`, a `url()` to `http:`, `https:` or `//`, or `@import url(`. After `pnpm --dir ui build`, `ui/dist/assets` holds four `.woff2` files and no CSS file holds `data:font`. Mutations, each red: a face's `src` pointing at `https://`; `OFL.txt` removed; the cyrillic face dropped.
- [ ] DF-14 — **One-line header.** `src/app/header.test.tsx` (new):
  - the header's selects are named Project, Workspace, Language and Theme;
  - Jump to and Shortcuts keep their names;
  - each select's label is in the DOM, visually hidden;
  - `.app-header`'s `min-height` is `var(--nav-height)`, and it sets `flex-wrap: nowrap` above 68.75rem.

  **[look]** At 1440x900 and 1100x800, in English and Russian, light and dark, with the mock's longest project and workspace names, the config notes and the mock indicator shown, the header is one row 48 px tall. Mutations, each red: dropping a label; `flex-wrap: wrap` above 68.75rem.

## Out of scope

- **Belonging to later slices:**
  - the owner's words Task, Subtask and Department, and renaming any existing string (RM-0096);
  - other screens' layouts and primitives, legacy token renames, font weights elsewhere (RM-0099);
  - Flow and the document modal (RM-0094, RM-0095);
  - Ready to accept and progress bars (RM-0098);
  - Activity and a task's questions (RM-0097).
- **Not done here:**
  - a `categories` config key: categories are derived;
  - a status lozenge on tiles;
  - Jira's combined status button, whose visible text would not be in the Move button's name;
  - truncating lozenges;
  - a high-contrast theme;
  - a density setting;
  - a hub-wide default theme;
  - cross-tab sync of the theme;
  - an italic font file;
  - avatars;
  - icons from a package (hand-drawn, as now).
- **The daemon:** caching hashed assets (every response is `no-store`, so fonts are fetched once per page load), and any daemon or Rust change.
- **No dependency change.** The font is files, not a package (W1).

## Implementation

To be filled in after implementation. The plan, the starting point and the ship's canon follow.

### Roles and files

The **ui-developer** alone writes, in `ui/**` except `ui/README.md` and `src/api/generated/`: no package, lockfile, Rust or docs change. The **test-engineer** runs the gates and the mutations and writes nothing here (`ui_policy.rs` and `anonymity.rs` need no change: the fonts lie outside `ui/src`, and `OFL.txt` is English).

| Area | Files | What changes |
|---|---|---|
| Fonts | `fonts/inter/*.woff2`, `fonts/inter/OFL.txt`, `src/styles/fonts.css`*, `src/main.tsx` | four subsets, the licence, the faces, the import |
| Tokens | `src/styles/tokens.css`, `src/styles/scale-exceptions.ts`*, `index.html` | both themes, the scale, aliases, exceptions, the meta |
| Theme | `src/app/theme.ts`*, `src/app/ThemeSwitcher.tsx`*, `src/app/Shell.tsx`, `src/main.tsx` | choice, apply before render, the header |
| Primitives | `src/ui/{Button,Lozenge,PropertyList,Breadcrumbs,IssueView,EmptyState,Disclosure}.tsx`*, `src/ui/Icon.tsx`, `src/ui/Badge.tsx` (look only) | the six and the helper, six icons |
| Board | `src/board/category.ts`*, `BoardView.tsx`, `BoardColumn.tsx`, `BoardCard.tsx` | lozenge heads, lane heads, badges' metrics |
| Work item | `src/board/CardModal.tsx`, `CardPage.tsx`, `CardPanel.tsx`, `CardActs.tsx`, `CardTasks.tsx` | the IssueView, side column, More fields, Development |
| CSS | `src/styles/app.css` | base, controls, shell, Board, work item, primitives on tokens; mechanical raw values |
| Catalog | `src/i18n/en.ts`, `src/i18n/ru.ts` | "Data", "Catalog keys" |
| Tests | `src/styles/{scale,fonts}.test.ts`*, `tokens.test.ts` (rewritten), `src/ui/*.test.tsx`*, `src/board/{category.test.ts,workItem.test.tsx}`*, `src/app/{theme,header}.test.tsx`*, the "Test churn" rows | DF-1 to DF-14 |

\* New.

### Work order

1. Fonts, then tokens with both themes and the rewritten `tokens.test.ts` (DF-9, DF-12, DF-13).
2. The scale, the aliases, and the guard with its exceptions (DF-1 to DF-3).
3. The primitives and their tests (DF-4, DF-5, DF-8, DF-7's EmptyState).
4. The header and the theme switcher (DF-14).
5. The Board, then the card modal and page (DF-5 to DF-8), with the test churn.
6. Catalogs, gates and size (DF-11); then the looks (DF-10, DF-14).

### Before

At `61290d6`:
- first-load JS is 797.95 kB against a bar of 845;
- `tokens.css` has 171 custom properties, 54 lines with a colour literal, and the dark theme only;
- `app.css` has 6 419 lines. Its raw values: 15 font sizes (eight 0.6875rem, five 0.75rem, two 0.92em), 12 line heights (1.6, 1.5, 1.4, 1.3, 1.25, 24px), 14 radii (ten 999px, three 2px, one 36px), and 29 spacing declarations holding a non-zero length;
- `--font-size-s` has 148 uses.

### Canon at ship

- **New `docs/canon/ui-design.md`** (Tier 2, scope `[ui]`, claimed by `docs/index-canon.md`, which has about 0.9 kB free), about 8 KB. Its sections:
  - The rule (one token file, two themes, the scale, the guard);
  - Themes;
  - Type;
  - Space and shape;
  - Colour roles and the matrix;
  - Primitives;
  - Status categories;
  - The work item;
  - Guards and exceptions;
  - Known limits (the empty page before the entry module, `no-store` fonts, synthesised italic, the legacy aliases until RM-0099);
  - Tests.
- **`ui/README.md`** (8 176 B of 10 240):
  - "Screen rules", **Tokens**: "light and dark, `light-dark()` roles (`ui-design.md`)" in place of "dark only";
  - the `localStorage` list gains `theme`;
  - a **Look** bullet: primitives from `src/ui/`, sizes from the scale, `scale.test.ts`;
  - "Dependencies": a line "Bundled font, no package: Inter <version>, OFL-1.1, the owner's decision of 2026-11-10, files and licence in `ui/fonts/inter/`".
- **`docs/canon/board-screen.md`** (98 B free): "The card modal and page" points at `ui-design.md` "The work item" for the layout and keeps only the routes, history and focus; "The grid" names the lozenge heads. It is net-neutral, the layout's detail moving out.
- **`docs/canon/card-tasks.md`** (39 B free): "Screen" says the part sits under the side column's Development, folded; net-neutral.
- **`docs/canon/ui-languages.md`**: "Size" is unchanged, since fonts are not JS. Only the slice's figure goes in "Implementation".
- **`CLAUDE.md`'s state line** and the v5 table row in `08-roadmap.md`.

### Working answers

These are the spec-writer's; the owner's review is open.
- **W1. The font is files, not a package.** Committed subsets keep the allowlist and lockfile as they are, and need no ADR (ADR-0033's rule covers packages), no `ui_policy.rs` change and no second role. An `@fontsource-variable/inter` package would add a twentieth entry, an ADR amending ADR-0033, and a test-engineer edit, in exchange for updates by pin.
- **W2. `light-dark()`.** Every role is declared once, and the system choice needs no script.
- **W3. An indigo accent** (`#4f46e5` light, `#a5b4fc` dark). It is SpecEngine's own hue, told apart from the information blue of in-progress lozenges and from Jira's blue.
- **W4. Neutral grey dark surfaces** in place of navy, as in the light theme. The dark status roles keep their values where they pass.
- **W5. Category precedence:** stranded, then done, then returned, then to do, then in progress. The appearances are neutral, success, warning, neutral and information.
- **W6. Status shows once.** A column head carries the category lozenge, and tiles carry none. The tiles' badges become lozenge-sized.
- **W7. A badge's label is `--text-primary`**, since 12 px text needs 4.5:1. The tone stays on the icon and border at 3:1, and dashed or dotted borders keep their meaning.
- **W8. The Details order** follows the brief, then Discipline, Parent, Corrects, Created by and Created at. Empty fields go under More fields.
- **W9. Header selects** show icons, with visually hidden labels. Jump to and Shortcuts are icon-only below 80rem.
- **W10. Legacy aliases:** 13 px becomes 14, 15 becomes 16, 18 becomes 20 and 24 stays 24; body text becomes 14 px, from 15.
- **W11. Lozenges wrap and never cut** ("anything cut is seen whole"), unlike Jira's truncation.
- **W12. Sizes.** The card modal is 968 px, a default dialog 600 px, and a dialog's radius 12.

<!-- On shipping (status: shipped + shipped: date) the document is compacted to "Why", the criteria and a summary <= 3 KB; whatever became true moves into the canon. -->
