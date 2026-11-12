---
class: spec
status: shipped
shipped: 2026-11-12
scope: [ui]
ref: RM-0099 part a, v5's UI lane, its second slice after RM-0103 `canvas-clicks` (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (DS-1 to DS-11, here DS1 to DS13); the orchestrator's working answers, the owner's review open; one iteration, then the review's iteration 2
adrs: []
---

# design-sweep: the legacy names gone, the guards closed, Inbox, Questions and the Spec tree on the primitives (RM-0099 a)

## Why

RM-0093 built the tokens, the themes and six primitives for the shell, the Board and the work item. Every other screen still read the legacy size names through eleven aliases in `tokens.css`. While the aliases stood, the guard could not refuse those names, so each new slice added more. Inbox, Questions and the Spec tree used almost none of the primitives, so they looked older than the Board beside them.

RM-0099's first half renames every legacy use and raw weight, takes the aliases out and turns the guards on (G1 names, G2 weights). It puts Inbox, Questions and the Spec tree on the primitives, with a list's one item laid out as a pane. The Overview is renamed only, since RM-0086 replaces it. The second half is `design-sweep-b`. UI only, no ADR: it rests on ADR-0011, ADR-0072 with ADR-0024, ADR-0031, ADR-0008 and ADR-0076.

## Description and interactions

The decisions are canon now:
- D1 and D2, the rename and the guards: `docs/canon/ui-design.md` "Type, space and size", "Guards and exceptions".
- D3, the pane: "The work item".
- D4 and D5, Inbox and its one status table: `proposal-review.md` "Inbox". The staged Confirm: `hub-confirm.md` "Screen".
- D6, Questions: `round-ui.md` "Screen".
- D7, the Spec tree: `directions.md` "Tree".
- D8 to D10: `ErrorPanel`'s Retry is a `Button`. The Overview and every other screen are renamed only. No data, endpoint, colour or `provisional.ts` change.

The owner's questions are open, and their working answers are built:
- **Q-A**: the Overview is renamed only.
- **Q-B**: a pane is one column until 60rem of its own width. The Board keeps 44.5rem.
- **Q-C**: a Lozenge marks an item's status in its head; Badges stay on rows.
- **Q-D**: the sweep is built before his trial.

## Data

The rename keeps every value but at A3 and A4:
- **A3**: markdown h2 takes heading-m, h3 and h4 heading-s, h5 and h6 body.
- **A4**: the nine 650 weights become semibold.

G1, G2 and the pane's rules are in `ui-design.md`. The new keys are `inbox.card.details`, `tree.node.details` and `tree.node.detailsAt` (`Details at {{place}}`). Their Russian is a draft for the owner.

## Rules and edge cases

In the canon:
- `ui-design.md` "Primitives" (at most one primary) and "The work item" (the pane).
- `directions.md` "Tree" (chips, chevrons, the outline) and "Known limits".

## Acceptance criteria

A browser look is marked **[look]**, a counted run **[measure]**. Every other criterion is a test that the named mutation turned red.

- [x] DS1 — **G1, names.** `scale.test.ts` finds no legacy name in CSS or in non-test TS and TSX, and the three families are their exact step lists. Mutations, each red: `--space-2` back in `tokens.css`; `gap: var(--space-2)` in `.queue`; `"var(--space-1)"` in `inbox/labels.ts`.
- [x] DS2 — **G2, weights.** Every `font-weight` outside `@font-face` is one token `var()` or `inherit`, and `fonts.css` passes untouched. The family is exactly the four, 400 to 700. Mutations, each red: `font-weight: 600` in `.round-count`; `650` in `.graph-mark`; `style={{ fontWeight: 600 }}` in `Lozenge.tsx`.
- [x] DS3 — **Markdown headings carry their step.** No `.markdown` rule holds a unitless line height. Mutation, red: `line-height: 1.25` back on `.markdown .md-heading`.
- [x] DS4 — **The pane.** `layout="pane"` gives `issue-view-pane`, with side before main in the DOM. The default keeps main first and has no pane class. `app.css` holds the pane's one-column rule and its `@container issue-view (min-width: 60rem)` rule, and the 44.5rem rule is unchanged. Mutations, each red: the pane's two columns at 44.5rem; main before side in a pane.
- [x] DS5 — **Inbox.** Amended at ship from "exactly one primary".
  - The selected proposal shows Breadcrumbs: Project, Inbox, and its ID with `aria-current="page"`.
  - The decision bar holds at most one `button-primary`. It is Accept, or Confirm on a hub with a staged choice confirmable here, and then Accept is default.
  - The status is a Lozenge from `statusLozenge`: `changes_requested` is warning, and `"weird"` is neutral, raw, `lang="en"`.
  - The facts are a `PropertyList` labelled Details.
  - The empty queue and a no-match filter are `.empty-state-box`es, and Clear filter is the filter's primary.
  - The hotkey, focus and decision tests stay green.
  - Mutations, each red: two primaries; Accept staying primary beside a staged Confirm; no Breadcrumbs; the empty queue as a `div.empty-state`; a second status table in `board/labels.ts`; `useProjectName` reading the projects itself.
- [x] DS6 — **Questions.** Stage is the only `button-primary` on a sheet. The empty sheet is an `EmptyState` whose description holds the Inbox and Agent tasks links. Mutations, each red: the empty sheet as plain text; the sheet's Confirm as primary.
- [x] DS7 — **The Spec tree.** One holder's node shows Breadcrumbs (Project, Spec tree, its REF current), its status as a neutral Lozenge, and its facts as a `PropertyList` with `foldEmpty` (a section's Status and Rev under More fields). Several holders show each holder's `h2` and list. The document view's Technical details still renders `dl.facts` with the same rows. A search with no hit is an `EmptyState`. Mutations, each red: the node's facts back in `dl.facts`; `Facts` listing its own rows instead of `factsOf`.
- [x] DS8 — **The tree's exceptions leave, and the direction list.** `SCALE_EXCEPTIONS` holds 10, none of them `.text-view`, `.tree` or `.tree-row`. A one-kind direction's flat rows have no kind chip, and the tree has `tree-flat`. A two-kind direction keeps the chips, and a rooted read keeps `tree` alone. Mutations, each red: the `.tree` gap back at `1px` with its entry; the chip drawn with one kind.
- [x] DS9 — **Primitives in the four directories.** `ui/primitives.test.ts` checks the non-test `.tsx` of `src/{inbox,round,tree,trace}/` and `src/ui/states.tsx`. It finds no `className="button`, no `"button button-` literal, and no `className="empty-state"` or `"empty-filter"`. Mutation, red: a raw `<button className="button">` in `StagedChoice.tsx`.
- [x] DS10 — **Strings.** The three new keys are in `en.ts` and `ru.ts`, and every key still has a user. `catalog.test.ts` and `words.test.ts` pass. Mutations, each red: `tree.node.details` missing in `ru.ts`; "writer" in an `en` string.
- [x] DS11 — **[measure] Size.** First-load JS is at most 845 kB. It was 824.12 kB in the worktree and 825.27 kB on `main` with RM-0035; the figures are in "Implementation".
- [x] DS12 — **[look]** The orchestrator's, 2026-11-12, in Chrome on the worktree's build under `vite preview`, with the mock's `normal` scenario.
  - Inbox PR-0042 at 1440x900, light, English: the trail, an Open lozenge, Accept the only primary, Reject danger, and Details in one column above main.
  - The same at 1920x1080, dark, English: two columns with Details on the right. Contrast was right after a reload. The first shot, taken mid-switch, was a transition artifact.
  - The Spec tree's RULE-TIDE-WINDOW at 1100x800, light, English: the trail, the focused heading, the acts, Details with More fields (2), the tasks block and the tabs.
  - Questions at 1100x800, light, Russian: one primary, and the recommended options marked.
  - Not looked at (tests cover them): the empty states, the hub's Confirm case, a decision item, a drift item, several holders, a direction's list, and the Text and Bindings tabs. Nor the screens A3 and A4 alone change: the Overview, the Board, Flow, Graph, Roadmap's Chain and the document modal. The mock has no two-kind direction.
- [x] DS13 — **Gates.** On `main` after the merge:
  - UI: lint has 0 problems, the build passes, and `pnpm test` is green with 185 files and 5,307 tests.
  - The eval tests `ui_policy`, `anonymity` and `doc_pointers` pass, 98 of 98, and `spec check` passes.

## Out of scope

- Health, Graph, Roadmap and Agent tasks, the other 8 exceptions, the header heights and G3: `design-sweep-b`.
- The Overview's restyle (RM-0086).
- Primitives on Flow, the Board and the document view beyond the rename.
- `card-questions.md`'s limit that Questions sends no `as`: that is behaviour, for part b.

## Implementation

Built in a worktree, reviewed ACCEPT, then iteration 2; applied onto `main` beside `node-history`, no conflict. 48 files, under `ui/src/`:

| Module | Does |
|---|---|
| `styles/{app,tokens}.css` | the rename and the weights (A3, A4); the 11 aliases out; the pane's rules; the tree's three exceptions on tokens; `.tree-flat` |
| `styles/{scale.test,scale-exceptions,tokens.test}.ts`, `board/styles.test.ts` | G1, G2 and DS3; the exceptions from 13 to 10 |
| `ui/{IssueView,states}.tsx`, `ui/primitives.test.ts` (new) | `layout="pane"`; `ErrorPanel`'s Retry as a `Button`; DS9 |
| `api/queries.ts` | `useProjectName` |
| `inbox/*`, `board/labels.ts` | the proposal pane; `statusLozenge`, which `itemStatusLook` delegates to; `confirmLook`; EmptyStates and Buttons |
| `round/{RoundScreen,RoundItemCard}.tsx` | Buttons; the empty sheet |
| `tree/*` (11 files), `trace/CardsBlock.tsx` | the node pane; `factsOf`; the tree pane's `h2`; EmptyStates and Buttons; `flat`, `oneKind` |
| `i18n/{en,ru}.ts`, tests | the three keys; the DS tests; `policy.test.ts`'s tree links from 16 to 17 (the node's trail) |

**Census**, before (0f13522) and after:
- `app.css` legacy names: 698 before (`space-1..6` 159/201/119/56/17/3, `font-size-s/m/l/xl` 126/8/7/1, `line-height-tight` 1), 0 after.
- Raw weights: 77 before (600×57, 500×9, 650×9, 400×2), 0 after, with 117 token uses.
- Aliases: 11 before, 0 after. `scale-exceptions.ts` legacy names: 4 before, 0 after.
- Tests: 14 before, 15 after, all G1's own inputs.

**Departures**, accepted by the review:
1. When Confirm shows on a hub, it is the item's one primary and Accept turns default. `StagedChoice` takes `confirmLook`.
2. `useProjectName` is passive, because `useProjects` refetched on mount.
3. An `:empty` side is hidden and the pane drops to one column by `:has()`.
4. `.decision-bar` and `.node-head` are kept. `.card-head`, `.card-id`, `.card-task-link` and `.node-action` are gone.
5. EmptyStates reuse existing titles. "No tree from that root:" and "No bundle for this REF:" keep their colon, and the no-hit query loses its monospace. These wait on the owner's look.
6. `oneKind` is read from the answer's kinds, not `moduleKinds`.

**Review**: it verified that the rename keeps values (by a scratch diff), that G1 and G2 are exact, the pane's specificity, focus and hotkeys, `useProjectName`, and one status table with no cycle. **Iteration 2** made these changes:
- Inbox's no-match state is an `h2` (the draft said level 3).
- The tree pane's sr-only `h2` comes first, with `h3` empty states and an outline test. This relaxes `ui-tree-node` AC-14: the `h1` no longer leads the outline.
- Each holder gets "Details at <place>".
- A comment and a citation fixed.

**Numbers**: in the worktree, 181 files and 5,265 tests, green twice. Every DS mutation and three extras were red. First load was 823.97 kB in iteration 1 (gzip 239.26 by Vite's chunks) and 824.12 kB in iteration 2, against 823.88. `ru-*.js` was 152.41 kB (+0.11). On `main`: 825.27 kB.
