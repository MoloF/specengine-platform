---
class: spec
status: draft
scope: [ui]
ref: RM-0099 part a, v5's UI lane, its second slice after RM-0103 `canvas-clicks` (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (part a, its census, rename table, guards G1 and G2, DS-1 to DS-11, risks R1-R16; here DS1 to DS12); the orchestrator's working answers, the owner's review open
adrs: []
---

# design-sweep: the legacy names gone, the guards closed, Inbox, Questions and the Spec tree on the primitives (RM-0099 a)

## Why

RM-0093 built the tokens, the themes and six primitives, and applied them to the shell, the Board and the work item (`docs/canon/ui-design.md`). Every other screen still reads the legacy size names through eleven aliases in `tokens.css`, and the canon schedules their end: "Other screens keep the legacy names until RM-0099". While the aliases stand, the guard cannot refuse those names, so each new slice adds more: the split dialog (RM-0100) and the mandate rules (RM-0104) both did. Inbox, Questions and the Spec tree use almost none of the primitives, so they look older than the Board beside them.

This slice is RM-0099's first half. It renames all 698 legacy-name uses and the 77 raw font weights, takes the aliases out of `tokens.css`, and turns the guard on (G1 names, G2 weights). It also puts Inbox, Questions and the Spec tree on the primitives, the two list screens with a one-item pane laid out for their width. The Overview is renamed only, since RM-0086 replaces it.

The second half, `design-sweep-b`, gets its own spec after this one ships. It covers Health, Graph, Roadmap and Agent tasks on the primitives, the last 8 exceptions, the stale header heights and guard G3, and it closes RM-0099. The owner's word of 2026-11-12 takes both now, before his pipeline trial (`08-roadmap.md` Phase 7). This slice is UI only: no crate, no plugin version, no hook or CI change. No ADR: it rests on ADR-0011 (the stack), ADR-0072 with ADR-0024 (two catalogs), ADR-0031 (served words stay strings, looks live in `labels.ts`), ADR-0008 and ADR-0076 (the owner's words in `en`).

## Description and interactions

### Decisions

- **D1. The rename is value-identical but at two named spots.** Every use takes its alias's target ("Data", the table), so every computed size stays as it is. The two exceptions are A3, where markdown headings take their steps' line heights, and A4, where the 650 weights become semibold.
- **D2. The aliases leave, and the guard closes.** The eleven alias declarations leave `tokens.css` (about lines 330-342, with their comment). G1 refuses the legacy names everywhere under `ui/src` but tests, and G2 refuses a raw `font-weight` ("Data", "The guards").
- **D3. One item beside a list is a pane.** `IssueView` gains `layout?: "item" | "pane"`, defaulting to `"item"`, so the Board, the card page and the document view are unchanged. In a pane (Inbox's proposal, the Spec tree's node) the DOM order is head, side, main at every width: today's order of facts above the body. The body is one column until the pane itself is 60rem wide, then main on the left and the side on the right. The Board keeps 44.5rem.
- **D4. Lozenge marks status in a head; Badge stays on rows.** A Lozenge marks a proposal's status in Inbox's head and a holder's raw `status:` in the Spec tree's (neutral, `lang="en"`). Every Badge on a row stays: sync, link state, finding, severity, a list row's status.
- **D5. Inbox.** The selected proposal is an `IssueView` pane, the queue's empty states are `EmptyState`s, and every button is a `Button` ("Data", "Inbox").
- **D6. Questions.** Every button is a `Button`, with Stage the one primary. The empty sheet is an `EmptyState` with its next step. The sheet is not a work item.
- **D7. The Spec tree.** The node is an `IssueView` pane, and its facts are a `PropertyList` from one row builder that the document view's `Facts` shares. Its empty states and buttons take the primitives. Its three exceptions leave the list. A direction's flat list loses the chevron's slot, and with one kind, the kind chip (the limit `docs/canon/directions.md` names for this sweep).
- **D8. Shared.** `ErrorPanel`'s Retry (`ui/src/ui/states.tsx`) becomes a `Button`. So does every raw `className="button…"` in `src/{inbox,round,tree,trace}/` (`trace/CardsBlock.tsx` serves the tree's pane), or a link takes `buttonClasses`.
- **D9. The Overview is renamed only:** 27 uses and 4 weights, no primitive. Flow, the Board, the document view, Health, Graph, Roadmap, Agent tasks, the palette and the shell are likewise renamed only.
- **D10. Nothing else moves.** No new data or endpoint, no `provisional.ts` edit, no colour, no layout of the four screens part b takes.

## Data

### The rename (`ui/src/styles/app.css`)

The census at 82a7d39 is 698 uses, all in `app.css`. Re-count before editing, because `canvas-clicks` lands first:

```sh
perl -ne 'print "$&\n" while /--(?:space-[1-6]|font-size-(?:s|m|l|xl)|line-height-tight)(?![\w-])/g' ui/src/styles/app.css | sort | uniq -c
perl -ne 'print "$1\n" while /font-weight:\s*(\d+)/g' ui/src/styles/app.css | sort | uniq -c
```

| Legacy | Uses | New | Value |
|---|---|---|---|
| `--font-size-s` | 126 | `--font-size-body` | 0.875rem |
| `--font-size-m` | 8 | `--font-size-heading-s` | 1rem |
| `--font-size-l` | 7 | `--font-size-heading-m` | 1.25rem |
| `--font-size-xl` | 1 | `--font-size-heading-l` | 1.5rem |
| `--space-1` … `--space-6` | 159, 201, 119, 56, 17, 3 | `--space-050`, `-100`, `-150`, `-200`, `-300`, `-400` | 0.25, 0.5, 0.75, 1, 1.5, 2rem |
| `--line-height-tight` | 1 | A3 | 1.25 → each level's step |

A substitution with the `(?![\w-])` boundary never touches `--space-100` or `--font-size-small`. Outside `app.css`, legacy names stand only in `tokens.css` (the 11 aliases), `scale-exceptions.ts` (`.tree-row`'s value, which leaves with D7), and tests ("Test churn").

**A3, markdown headings.** `.markdown .md-heading` loses its `line-height`. h2 takes `--font-size-heading-m` and `--line-height-heading-m` (24 px, was 25). h3 and h4 take the heading-s pair (20 px, equal). h5 and h6 take the body pair (20 px, was 17.5). The text of the document view and the tree's Text tab moves by those pixels.

**A4, weights.** `font-weight: 400` becomes `var(--font-weight-regular)` (2 uses), 500 becomes `var(--font-weight-medium)` (9), and 600 becomes `var(--font-weight-semibold)` (57). The nine 650s (Graph 2, Roadmap 2, Flow 5, about lines 2492, 2532, 4838, 4869, 5016, 5524, 5708, 5731, 5954) become `var(--font-weight-semibold)`, so Graph's, Roadmap's and Flow's marks and current items are 50 lighter.

### The guards (`ui/src/styles/scale.test.ts`)

- **G1, names.** No match of `/--(?:space-[1-6]|font-size-(?:s|m|l|xl)|line-height-tight)(?![\w-])/` in any `src/**/*.css`, `tokens.css` included, or any non-test `src/**/*.{ts,tsx}`. Both sets are read through `import.meta.glob(…, {query: "?raw"})`, as the policy tests read sources. The families become exact lists: `--space-*` the 11 steps `025` to `600`, `--font-size-*` and `--line-height-*` the 7 steps each, every one in its set. `LEGACY_RATIO` goes, and so does the assertion that the legacy names resolve (about line 198). The two `>=` counts (`--font-size-` ≥ 11, `--space-` ≥ 17) become the exact lists.
- **G2, weights.** Every `font-weight` declaration in a `src/**/*.css` outside an `@font-face` rule is exactly one `var(--font-weight-regular|medium|semibold|bold)` or `inherit`. `fonts.css`'s four `font-weight: 100 900` descriptors stand inside `@font-face` and pass. `tokens.css`'s `--font-weight-*` family is exactly those four, at 400, 500, 600 and 700. The inline-style test's property pattern gains `fontWeight`, so no non-test `.tsx` sets a weight inline.

### The pane (`ui/src/ui/IssueView.tsx`, `app.css`)

```tsx
/** "pane": one item beside a list (Inbox, the Spec tree): head, side, main in the DOM; two columns from 60rem. */
layout?: "item" | "pane";
```

With `"pane"` the root is `issue-view issue-view-pane`, and `.issue-view-side` comes before `.issue-view-main` in the body. The head is unchanged.

```css
.issue-view-pane > .issue-view-body { grid-template-columns: minmax(0, 1fr); }
@container issue-view (min-width: 60rem) {
  .issue-view-pane > .issue-view-body { grid-template-columns: minmax(var(--issue-main-min), 1fr) minmax(0, var(--issue-side-width)); }
  .issue-view-pane > .issue-view-body > .issue-view-main { grid-column: 1; grid-row: 1; }
  .issue-view-pane > .issue-view-body > .issue-view-side { grid-column: 2; grid-row: 1; }
}
```

The first rule outranks the 44.5rem rule by specificity, so a pane stays one column between 44.5rem and 60rem. The side keeps `max-width: var(--issue-side-width)`.

### Inbox (`ui/src/inbox/`)

- **The proposal** (`ProposalCard.tsx`): the `article` and its `aria-labelledby="card-title"` wrap an `IssueView layout="pane"`.
  - Breadcrumbs: Project (`homeHash(project)`, its name, else the slug), Inbox (`sectionHash(project, "inbox")`, `app.sections.inbox.label`), and the proposal's ID current (`mono`).
  - Title: today's `h2#card-title`.
  - Meta: the status `Lozenge`.
  - Actions: the decision bar, as Buttons: Accept `primary`, Reject `danger`, the rest `default`. Each keeps `aria-keyshortcuts`, its `kbd` and `aria-disabled`. Then `ActOff` and the staged choice. A drift item keeps `TerminalDecision`.
  - Side: a `PropertyList` labelled `inbox.card.details`. Its rows are every fact of today's `dl.facts` but status (kind raw `mono`, gap type, severity as its Badge, the agent task's link or none), and the board task (`card_id`'s link, today's `card-task-link` line), when there is one.
  - Main: today's `.card-body`, its `ref` and `tabIndex={-1}` kept.
- **The status lozenge** (`labels.ts`): `statusLozenge(value: ProposalStatus, t): LozengeLook & {lang?: "en"}`. Its table is `board/labels.ts`'s `ITEM_STATUSES`, moved here: open neutral, changes requested warning, approved information, applied success, rejected danger, deferred and superseded neutral, each with today's icon. An unknown value is neutral, icon `unknown`, raw, under `lang="en"`. `board/labels.ts` `itemStatusLook` returns `statusLozenge(…)`, so one table serves both.
- **Empty states** (`InboxView.tsx`): the empty queue is `EmptyState` level 2: `inbox.clearTitle`, its two lines as the description. A filter with no match is `EmptyState` level 3: `inbox.noMatch` as its title, Clear filter its primary act.
- **Buttons:** `ConfirmDialog`, `DecisionDialog`, `StagedChoice`, `OutsideStageAlert`, `InboxView`. Refs and `data-autofocus` pass through `Button`'s `ref` and rest props.

### Questions (`ui/src/round/`)

- **Buttons:** Stage is the one `primary`, Dismiss is `subtle`, Show all and an item's button are `default`.
- **The empty sheet** (`RoundScreen.tsx`) is `EmptyState` level 2: `round.emptyTitle`, then the notes, `round.emptyWhy` and the next-step line with its Inbox and Agent tasks links as the description.
- `RoundItemCard`'s severity Badge stays.

### The Spec tree (`ui/src/tree/`, `ui/src/trace/`)

- **The node** (`NodePane.tsx`): `IssueView layout="pane"`.
  - Breadcrumbs: Project, Spec tree (`sectionHash(project, "tree")`, `app.sections.tree.label`), and the REF current (`mono`).
  - Title: today's `h1`, its `id`, `ref` and `tabIndex={-1}` kept.
  - Meta: one holder's raw `status:` as a neutral `Lozenge`, `lang="en"`. Nothing for a section or several holders.
  - Actions: Open document (`Button` subtle compact, as today) and Show in Graph (a link with `buttonClasses("subtle", "compact")`).
  - Side: one holder's `PropertyList` with `foldEmpty`, labelled `tree.node.details`. With several holders, the count line, then per holder its `h2` and its `PropertyList`.
  - Main: the read's notes, the failed re-read's alert, `CardsBlock`, the tabs and the foot, in that order.
- **The facts** (`Facts.tsx`): `factsOf(holder: ShownNode, t): Property[]`, rows Kind, Where, Status, Rev, Size, Span hash, Flags, `null` where today's `Facts` draws nothing. `Facts` renders today's `dl.facts` from those rows, unchanged for the document view's Technical details.
- **Empty states:** `NodePane` (no node open; nothing found, the reason verbatim and the REF forms as its description), `TreeView` (an empty tree, two), `SearchResults` (no hit), `BundlePanel`, `LinksPanel`, `ProposalsPanel`. Each is an `EmptyState` one level below its part's heading.
- **Buttons:** `TreeView`, `SearchResults`, `BundlePanel`, `BindingsPanel`, `LockDiffPanel` (its toggle keeps `aria-expanded` and `aria-controls`), and `trace/CardsBlock.tsx`.
- **The three exceptions leave:**
  - `.text-view` `line-height: var(--line-height-body)` (was 1.6 on 14 px; the number column follows it);
  - `.tree` `gap: var(--space-025)`;
  - `.tree-row` `padding: var(--space-050) var(--space-100) var(--space-050) calc(var(--space-050) + var(--depth, 0) * var(--space-200))` (the indent 16 px a level, was 17.6).
- **A direction's list:** `TreeView` passes `flat` (the read had no root and named kinds) and `oneKind` (`moduleKinds.length === 1`) to `SpecTree`. A flat tree's class is `tree tree-flat`, whose rows drop the chevron's column and `.tree-toggle`. With `oneKind`, `RowContent` draws no kind chip.

### Strings

New keys: `inbox.card.details` "Details" and `tree.node.details` "Details", in `en.ts` and `ru.ts` (the Russian a draft for the owner, as before). Everything else reuses keys that exist: `app.sections.*.label`, and the EmptyStates' present titles and lines. A key whose last user goes leaves both catalogs.

## Rules and edge cases

- WHEN a legacy name or a raw `font-weight` enters any non-test source under `ui/src`, the guard SHALL fail and name the file and the match.
- WHEN a pane is under 60rem of its own width, its body SHALL be one column, side before main. At or above, main SHALL stand left and the side right, with the DOM order unchanged, so focus never jumps on a resize.
- WHEN an item or a dialog shows acts, at most one SHALL be primary (`ui-design.md` "Primitives").
- WHEN a status word is not in the table, the Lozenge SHALL show it raw, neutral, `lang="en"`, never mapped to a known value.
- Inbox's hotkeys (`a`, `r`, `c`, `d` in the proposal, `j`, `k` in the list), the heading's focus on selection, `useRetryFocus` and the Spec tree's roving tree keys SHALL behave as today.
- A direction with two or more kinds SHALL keep each row's kind chip, and a rooted read SHALL keep the chevrons and indent.
- Rules left with no user leave `app.css` (`.card-head`, `.card-id`, `.decision-bar` where the pane replaces them). The old `.empty-state` and `.empty-filter` rules stay until part b removes their last users (Agent tasks, Health, Graph, the Board's `CardPanel`).

### Test churn

- `board/styles.test.ts` lines 72, 95 and 155-160 pin `--space-1`, `--space-6` and `--space-3`. They take the new names.
- `styles/tokens.test.ts` (about line 248) requires the legacy names. It requires the steps instead.
- `styles/scale.test.ts`: `LEGACY_RATIO` (102-103), the legacy branch and line 198 (177-199), the `--space-` count (248), and the exception count 13 → 10 (`:318-323`, and the `toHaveLength(13)` it holds). Lines 225-226 are detector inputs and stay.
- The facts' pins: `inbox/InboxView.test.tsx` (about 372), `mocks/{amendment,decision}.smoke.test.tsx` (`.facts dt`), and `tree/NodePane.test.tsx` (about 116) read the PropertyList's rows.
- Tests that find a raw `button` by class still find `Button`'s `button` class.

## Acceptance criteria

A browser look is marked **[look]**, a counted run **[measure]**. Every other criterion is a test that the named mutation turned red. DS1-DS3 are the analysis's DS-1 to DS-3. From DS4 on they are renumbered, the pane added.

- [ ] DS1 — **G1, names.** `scale.test.ts` finds no legacy name in CSS or non-test TS and TSX, and the three families are the exact step lists. Mutations, each red: `--space-2: var(--space-100);` back in `tokens.css`; `gap: var(--space-2)` in `.queue`; `"var(--space-1)"` in a non-test `.ts`.
- [ ] DS2 — **G2, weights.** Every `font-weight` outside `@font-face` is one token `var()` or `inherit`, and `fonts.css` passes untouched. The family is exactly the four at 400-700. Mutations, each red: `font-weight: 600` in `.round-count`; `650` in `.graph-mark`; `style={{ fontWeight: 600 }}` in a non-test `.tsx`.
- [ ] DS3 — **Markdown headings carry their step.** h2 takes the heading-m pair, h3 and h4 heading-s, h5 and h6 body. No `.markdown` rule holds a unitless line height. Mutation, red: `line-height: 1.25` restored on `.markdown .md-heading`.
- [ ] DS4 — **The pane.** `IssueView.test.tsx`: `layout="pane"` gives `issue-view-pane`, with side before main in the DOM. The default `"item"` keeps main before side and no pane class. A source test of `app.css` finds the pane's one-column rule and its `@container issue-view (min-width: 60rem)` rule, and the 44.5rem rule unchanged. Mutations, each red: the pane's two columns at 44.5rem; main before side in a pane.
- [ ] DS5 — **Inbox.** The selected proposal shows Breadcrumbs (Project, Inbox, its ID current, `aria-current="page"`). The decision bar holds exactly one `button-primary`. The status is a Lozenge from `statusLozenge` (`changes_requested` is warning; `"weird"` is neutral, raw, `lang="en"`). The facts are a `PropertyList` labelled Details. The empty queue and a no-match filter are `.empty-state-box`es, and Clear filter is the filter's primary. The existing hotkey, focus and decision tests stay green. Mutations, each red: two primaries; no Breadcrumbs; the empty queue as a `div.empty-state`; a second status table in `board/labels.ts`.
- [ ] DS6 — **Questions.** Stage is the only `button-primary` on a sheet. The empty sheet is an `EmptyState` whose description holds the Inbox and Agent tasks links. Mutation, red: the empty sheet as plain text.
- [ ] DS7 — **The Spec tree.** One holder's node shows Breadcrumbs (Project, Spec tree, its REF current), its status as a neutral Lozenge, and its facts as a `PropertyList` with `foldEmpty` (a section's Status and Rev under More fields). Several holders show each holder's `h2` and list. The document view's Technical details still renders `dl.facts` with the same rows (`DocBody.test.tsx` green). A search with no hit is an `EmptyState`. Mutations, each red: the node's facts back in `dl.facts`; `factsOf` dropped from `Facts`, which then lists its own rows.
- [ ] DS8 — **The tree's exceptions leave, and the direction list.** `SCALE_EXCEPTIONS` holds 10, none of `.text-view`, `.tree` or `.tree-row`, whose rules take tokens. In `TreeView.test.tsx`, a one-kind direction's flat rows have no kind chip and the tree has `tree-flat`. A two-kind direction keeps the chips, and a rooted read keeps `tree` alone. Mutations, each red: `.tree` gap back at `1px` with its entry; the chip drawn with one kind.
- [ ] DS9 — **Primitives in the four directories.** A source test over the non-test `.tsx` of `src/{inbox,round,tree,trace}/` and `src/ui/states.tsx` finds no `className="button`, no `"button button-` literal, and no `className="empty-state"` or `"empty-filter"`. Mutation, red: one raw `<button className="button">` restored in `StagedChoice.tsx`.
- [ ] DS10 — **Strings.** Both new keys are in `en.ts` and `ru.ts`, and a key with no user is in neither. `catalog.test.ts` and `words.test.ts` pass. Mutations, each red: `tree.node.details` missing in `ru.ts`; "writer" in an `en` string.
- [ ] DS11 — **[measure] Size.** After `pnpm build`, first-load JS is at most 845 kB. It is recorded with gzip and `ru-*.js` (raw and gzip) against `canvas-clicks`'s figure, its change explained.
- [ ] DS12 — **[look]** The orchestrator's, in Chrome, in both themes and both languages, with RM-0093's DevTools measure (`design-foundation.md` DF-10). Sizes: 1440x900 and 1100x800, plus 1920x1080 for a pane's two columns.
  - Inbox: a queue with an update, a decision item and a drift item; the empty queue; a filter with no match. The pane is one column at 1440 and two at 1920, with the side right.
  - Questions: a sheet of three items, and the empty sheet.
  - The Spec tree: one holder (its Text and Bindings tabs); several holders; a direction with one kind and one with two; a search with no hit; no node open.
  - Unchanged but for A3 and A4: the Overview, the Board's work item, Flow (marks and the current item), Graph and Roadmap's Chain (marks), and the document modal (headings h2-h6).
- [ ] DS13 — **Gates.** UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green. The eval tests (`ui_policy`, `anonymity`, `doc_pointers`) and `spec check` pass.

## Out of scope

- Health, Graph, Roadmap and Agent tasks on the primitives, the other 8 exceptions, the header heights (`.app-body`, `.tree-pane`, `.tasks-pane`, `.task-column h1`) and G3: `design-sweep-b`.
- The Overview's restyle (RM-0086).
- Primitives on Flow, the Board and the document view, beyond the rename.
- New data or endpoints, any Rust, `provisional.ts`, colours, and the filter chips' design.
- `card-questions.md`'s limit "The Questions screen sends no `as` and links no card (RM-0099)": behaviour, not looks. It stays named until `design-sweep-b` builds it or re-points it.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| ui-developer | `ui/src/styles/{app,tokens}.css`, `scale-exceptions.ts`; `ui/src/ui/{IssueView,states}.tsx`; `ui/src/inbox/**`, `ui/src/round/**`, `ui/src/tree/**`, `ui/src/trace/CardsBlock.tsx`; `ui/src/board/labels.ts` (`itemStatusLook` delegating); `ui/src/i18n/{en,ru}.ts`; the tests of DS4-DS9 and "Test churn" |
| test-engineer, UI | G1 and G2 in `styles/scale.test.ts` (DS1, DS2), `tokens.test.ts`; the gates and every DS mutation; DS11's measure; `policy.test.ts` and `ui_policy.rs` only if a pin moves |

Comments cite `docs/features/design-sweep.md` "Acceptance criteria" with a criterion ID, never "Data" (`doc_pointers`; compaction drops it at ship).

### Work order

1. After `canvas-clicks` ships (both edit `app.css`), the ui-developer re-counts and runs the rename and the weights alone, with A3, then the test churn. This is one commit-sized step whose look is unchanged but for A3 and A4.
2. The test-engineer lands G1 and G2 on that step, with the aliases out. Every gate is green before any screen moves.
3. The ui-developer: the pane (DS4), then Inbox, Questions and the Spec tree (DS5-DS9), each screen's tests green before the next.
4. The test-engineer: the gates and the mutations, then DS11. Then the UI review.
5. The orchestrator's look (DS12). Ship: the spec-writer ("Canon at ship"). Then the spec for `design-sweep-b`.

It runs beside the core lane (`node-history` or whichever core slice is current), sharing only documents. The owner commits each slice on its own. At most three iterations.

### Risks

- **The restructure breaks tests** (R9): Inbox and the Spec tree move into a pane, and their hotkey, focus and decision tests are the most numerous. This is the main iteration-cap risk, which is why the rename and the guard land first, apart from it.
- **The census moves:** `canvas-clicks` lands first, and another slice may add a legacy name. The guard is the proof, not the count.
- **A3, A4 and `.text-view` change looks** on screens this slice does not restyle. DS12 covers them.
- **Width:** the 60rem split, the side's 360 px and the two scrolls are a look; jsdom lays nothing out.
- **`ui/README.md`** is at 8 191 of 8 192 B. Its Tokens line still holds after this slice, so it is untouched.

### Canon at ship

Free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/ui-design.md` [15] | "The rule": the RM-0099 sentence goes. "Type, space and size": the Legacy aliases bullet goes, and the weights name their four tokens. "Guards and exceptions": G1 and G2, and the list of 10 (two kept, 8 RM-0099's). "The work item", Width: the pane (Inbox, the Spec tree), side first in the DOM, two columns from 60rem. Net not positive |
| `docs/canon/directions.md` | "Tree": a flat list has no chevron's slot, and with one kind no chip. "Known limits": the RM-0099 line goes |
| `docs/canon/doc-view.md` | "The view", Side: `Facts` renders `factsOf`'s rows, which the Spec tree's `PropertyList` shares (net zero) |
| `CLAUDE.md`; 08 s2 Phase 7 "Owner's word, 2026-11-12", the UI lane; RM-0099 (part a shipped, part b next) | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q-A. RM-0086 replaces the Overview: should the sweep only rename its tokens? | Yes: its 27 uses and 4 weights, value-identical, no primitive; its look only confirmed unchanged | A full restyle costs 1-2 pipeline hours for looks RM-0086 throws away. Skipping the rename too keeps the aliases and G1 off, so RM-0099 cannot close |
| Q-B. One item beside a list (Inbox, the Spec tree): the work item's two columns? | One column (head, details, main) until the pane itself is 60rem wide; the Board keeps 44.5rem | At 44.5rem, main is 352-392 px at 1440x900 against one 404-444 px column at 1100x800. Diffs and verbatim text wrap at about 45 characters, and the wider screen reads worse |
| Q-C. Lozenge or Badge? | Lozenge for an item's status in its head, and for a check verdict (part b); Badge stays on rows | Every Badge a Lozenge maps about 60 tone roles onto six appearances. The canon's distinctions blur (ok against cannot-verify, the dashed unknowns), contrast must be proved again, and the diff roughly doubles |
| Q-D. Build before the owner's trial? | Yes: the trial judges the swept look | Waiting leaves the UI lane idle until the trial |

**The spec-writer's:**
- W1: the 60rem split is a `layout` prop, not a second component, so the Board's and the document view's markup is unchanged.
- W2: in a pane the side comes first in the DOM at every width. Focus order then never changes on a resize, and at 60rem and up the side, though drawn right, is read before main. Main first would put a long diff above the facts in one column.
- W3: one status table. `ITEM_STATUSES` moves to `inbox/labels.ts`, and the Board's `itemStatusLook` delegates to it.
- W4: `factsOf` feeds both renderings, so the document view's Technical details, out of scope, keeps its `dl.facts` with no second list of facts.
- W5: `directions.md`'s RM-0099 limit is the Spec tree's and closes here. `card-questions.md`'s is behaviour and is left to part b.
- W6: the rename lands as its own step before any restructure, so a red test in step 3 is never a rename's.
