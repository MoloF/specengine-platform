---
class: spec
status: shipped
scope: [ui]
ref: ui-home analysis 2026-10-06, Q1-Q7 recommendations accepted by the orchestrator; 08 section 2 Phase 4; iterations 1-2, review accepted
shipped: 2026-10-06
adrs: []
---

# UI home and jump palette

## Why

The owner opens SpecEngine to learn what waits for him: a task whose plan waits for approval or whose spec changed since (the only control point, 06 section 3.5, ADR-0012), and the queue. That was two screens, and reaching a task, proposal or node took the nav and a list. Slice 5: a project home `#/<p>` from two reads the client has, and a palette (Cmd-K or Ctrl-K). Read-only, on the mock (ADR-0033); no new endpoint, client member, type, query key, mock data or package.

Working answers, 2026-10-06 (orchestrator; no ADR): one global `document` keydown listener, for the chord only; no `/` trigger; node search once per explicit activation; `#/` lands on `#/<first>`; nav "Overview" first; the Inbox region counts all the read lists, by status; Tasks region first. Assumed: non-Latin keyboard layouts occur. What became true: `ui/README.md` "Screen rules" (Keyboard's one exception, the chord; Dialogs' IME Esc and opt-in scrim click; `href` from `routes.ts`; Shell routes).

## Data

No new contract; `Route` gains `{ type: "project"; project: string }`. The home reads `TaskList` (`docs/features/task-package.md` "Description and interactions") and only the daemon's `InboxEntry` keys (`docs/features/daemon-read.md` "Data": `id`, `kind`, `status`, `target_id`, `target_ids`, `created_at`, `rationale`, `severity`, `summary`; never `task_id`, `evidence`); the palette `SearchResults.hits` (`id`, `title`, `path`).

```text
#/harbor-sim, #/harbor-sim/  -> { type: "project", project: "harbor-sim" }
#/                           -> replaced by #/harbor-sim (first project)
"T-0108" Enter               -> push #/harbor-sim/tasks/T-0108
REF "MEC-TIDES#RULE-TIDE-WINDOW" -> push #/harbor-sim/tree/MEC-TIDES%23RULE-TIDE-WINDOW
```

Mock `#/harbor-sim`: waiting T-0107 (Plan review), T-0108 (Ready, stale); other open "Draft: 1", "Changes requested: 1", "Ready: 2", "In progress: 1"; note `T-0106: unreadable row (bad JSON in criteria); skipped`.

## Rules and edge cases

- Nothing computed but grouping and counting what the reads list (ADR-0012); times as stored, never relative, no clock read.
- `kind` counted by raw value, shown as text, never quoted or compared outside `src/mocks/` and tests (ADR-0031).
- Per region: loading, skeleton + `aria-busy="true"`; empty, Tasks "No tasks yet." + `spec task new --nodes REF...`, none waiting "Nothing waits for you: no plan to review, no approved spec changed.", Inbox "The queue is clear: no proposal waits for your decision." + "Next: approve the tasks that are ready for development" (Tasks link); error, verbatim + Retry refetching that read only.
- WHEN `getTasks` fails THEN Inbox and the palette's other groups work (under the daemon until `GET .../tasks` ships).
- No "block" wording (ui-shell AC-15); one `h1`; data as text.
- **Amendment** (shipped specs are never edited, convention section 2): read under `ui/README.md` "Screen rules": ui-shell AC-13 (M: a listener acting on a plain key), ui-tree-node AC-04, ui-tasks AC-12 (M: a view's `document` listener), ui-graph AC-08 (the canvas adds none); `TasksView.test.tsx` "adds no key listener to the document or the window" and `GraphView.test.tsx` "... when the canvas mounts" accept exactly the shell's.

## Acceptance criteria

Vitest (`ui-developer`), `ui_policy.rs`, the gates. Counting stub: calls per member; builder: a test-built answer; M: the mutation turning it red; 32 applied and red in iteration 1, 15 in iteration 2.

- [x] AC-01 - `#/alpha`, `#/alpha/` the project route (replaces `routes.test.ts`'s not_found); `#/alpha/nowhere` not_found; `#/`, projects alpha, beta: `#/alpha`, `history.length` unchanged, `h1` "Overview"; `#/gamma`: "There is no project gamma." (M: `#/` -> `/inbox`; `#/<p>` not_found).
- [x] AC-02 - counting stub, `#/harbor-sim` rendered: one `getProjects`, `getInbox`, `getTasks` each, no other call (M: `getTask` per waiting task; a `getTree`).
- [x] AC-03 - mock: Tasks region as "Data", links `#/harbor-sim/tasks/T-0107`, `.../T-0108`; "Spec changed" on T-0108 only; no Closed count; the note verbatim; builder, 7 waiting: 5 rows, "5 of 7 shown" (M: `stale` ignored; notes dropped; closed counted).
- [x] AC-04 - `harbor-sim`, `ledger-api`: total = the Inbox view's rows; the five = its first five, in order; status (label + icon) and kind counts each sum to it; builder kind `widget`: "widget: 1" text; status `escalated`: neutral raw badge, last (M: own sort; a kind literal; colour only).
- [x] AC-05 - entries with only the nine `InboxEntry` keys render the region as full `Proposal`s do (M: a link from `task_id`).
- [x] AC-06 - `slow`: skeletons, `aria-busy` in both; `empty`: both texts and next steps; `getTasks` rejecting 503 "x": "x" + Retry, Inbox rendered; Retry: one `getTasks`, no `getInbox`; mirrored (M: one boundary; inert Retry).
- [x] AC-07 - no `/block/i` in the home's text; times equal stored strings; no `Date.now(`, `new Date()` in `src/overview/`, `src/palette/`; one `h1`; title `<img src=x onerror=alert(1)>` as text; `policy.test.ts`'s `href` rule covers `src/overview/` (M: "2 h ago"; an `href` by concatenation).
- [x] AC-08 - seven nav links, "Overview" first, `#/harbor-sim`, `aria-current` on the home only; switcher: `#/harbor-sim` -> `#/ledger-api`, `.../tasks` -> `#/ledger-api/tasks`; `?` on a home link: the chord row, no "Next proposal"; the chord row in every list (M: switcher to Inbox; Inbox keys).
- [x] AC-09 - on `body`, each built view, the Inbox filter: `{key: "k", metaKey}`, `{key: "k", ctrlKey}` open, `defaultPrevented`; `{key: "\u043b", code: "KeyK", metaKey}` opens; `{key: "t", code: "KeyK", metaKey}`, + `altKey`, + `shiftKey`, `isComposing`: nothing; plain `k` moves the Inbox; decision dialog open: none, `defaultPrevented` (M: `key` only; `code` only; over a dialog).
- [x] AC-10 - spies before `renderApp`, every route: one key listener, `keydown` on `document`, added once, removed on unmount (same function); plain `j`, `a`, `?` on `document`: nothing; the two tests amended (M: handling `/`; a view's listener).
- [x] AC-11 - `aria-modal` dialog "Jump to", `h2`; combobox attributes; Down, Up, Home, End move `aria-activedescendant`, focus in the input; Tab, Shift-Tab inside (the field and Close); Esc to "Jump to", a task row, or `main` from `body`; "<n> options" polite, outside the inert subtree (M: focus into the list; on `body`).
- [x] AC-12 - counting stub, `#/harbor-sim`: opening, typing 20 characters: no call; `#/harbor-sim/tree`: opening <= one `getInbox`, one `getTasks`, reopening none; a Search: one `search`, `query` the text with spaces (M: a call per keystroke; a refetch per opening).
- [x] AC-13 - decomposed title, precomposed query: found; "t-0108": T-0108, Tasks first; Enter: one history entry, `#/harbor-sim/tasks/T-0108`, focus on its `h1`, also from `.../T-0107`; hit `id` null: `sectionHash(p, "tree", path)`; the REF of "Data", no call; `<img src=x onerror=alert(1)>` as text (M: no NFC; focus on `body`; concatenation).
- [x] AC-14 - `getTasks` 503 "t": "Tasks could not be read: t", Sections and Inbox listed; `search` 503 "s": "Search could not be read: s", open, text kept (M: closing on error; one error empties all).
- [x] AC-15 - 15 packages; `pnpm lint` (0 warnings), `build`, `test` green, silent; `ui_policy`, `anonymity`, `doc_pointers` green (33/33); `ui/README.md` <= 8 192 B (8 169 B); docs gate clean, worst W <= 108 506 B; `CLAUDE.md` unchanged (M: a 16th package).

## Owner's manual check

`pnpm --dir ui dev`: `#/` -> `#/harbor-sim`, Back does not return to `#/`; the regions as "Data"; Cmd-K from the Inbox filter, `T-0108`, Enter (focus on its `h1`); again from `#/harbor-sim/tasks/T-0107`; Cmd-K on a Cyrillic layout; Esc returns focus; "Jump to" by mouse; a click outside the palette and Close each close it; with a Japanese or Chinese IME, Esc during a composition cancels only the composition; `?scenario=slow`, `error`, `empty`, `large` (`T-0`: "Tasks, 10 of 329"); VoiceOver: the dialog's name, the combobox, "<n> options"; 200 %: regions stack, the palette fits.

## Open

- **Endpoints**: none new; `GET .../tasks` as `ui-tasks` "Open".
- **`.claude/agents/ui-developer.md`** (owner's text, ADR-0033 "Cost"): line 16 "Tree, Node, Graph, Queue, Tasks, Health, Round" -> "Home, Tree, Node, Graph, Queue, Tasks, Health, Round"; until applied, `ui-tasks` "Open" holds (lines 18-23 provisional types, 38 dark only, 41-43 `pnpm test` + "Gates" and the mock, 47-48 as `ui-shell`).

## Out of scope

Recent decisions, applied proposals; health, drift, budgets, W (`ui-health-round`); node counts; any `getTask`; a cross-project view; live updates (`ui-live`); a `/` trigger; fuzzy ranking; new endpoints, members, types, packages (`cmdk`, `kbar`, `fuse.js`).

## Implementation

**Shell**: the project route, `homeHash`, `navOf` (`app/routes.ts`); `#/` replaced without a history entry; the switcher keeps a section, home to home. "Jump to" carries an `aria-hidden` key hint (the Command glyph and K, or "Ctrl K"); its accessible name stays "Jump to". **Home**: the "Waiting for you" emphasis only when something waits; "No other open task." when no other group is open.

**Palette**: a quiet "Close" after the keys hint (Tab stops: the field, Close); a scrim click closes like Esc (focus to the opener, else `main`) through `Dialog`'s opt-in `closeOnScrim`, off for every other dialog. Option element IDs `<useId>-option-<encodeURIComponent(key)>`, a hit's key `hit:<path>:<line>`. `p`: the route's project while `getProjects` loads or fails; once read, the route's if listed, else the first. Tasks and Inbox answers put in screen order and folded once per answer array. Under the list also "Projects could not be read: <message>" and "The spec search found no node for '<text>'."; "<n> options" also on opening.

**Keys**: `isJumpChord` (`app/chord.ts`) behind one capture-phase listener. Every key list's intro: "without Ctrl, Alt or Cmd; Cmd-K or Ctrl-K excepted: it opens Jump to from anywhere, a text field included." `Dialog` ignores, without preventing, an Esc while `nativeEvent.isComposing`.

| Module | What it does |
|---|---|
| `ui/src/overview/` | `HomeView`, `TasksRegion`, `InboxRegion`, `parts.tsx`; `tally.ts` (waiting, other open, status and kind counts) |
| `ui/src/palette/` | `Palette.tsx` (dialog, combobox, jumps, announcements); `options.ts` (groups, matching, caps, keys, per-answer caches) |
| `ui/src/app/` | `chord.ts`; `routes.ts`; `Shell.tsx` (landing, nav, switcher, "Jump to", palette); `shortcuts.tsx` (chord row, intros) |
| `ui/src/ui/Dialog.tsx` | `closeOnScrim`; the IME Esc |
| `ui/src/api/queries.ts` | `useCachedTasks`, `useCachedInbox`; `useSearchOnActivation` (one call per activation) |
| `ui/src/` `inbox/`, `tasks/`, `styles/` | `statusRank`; `SPEC_CHANGED` in `labels.ts`; home and palette styles |

Tests: 943 Vitest in 46 files (new `chord`, `tally`, `HomeView`, `options`, `options.memo`, `Palette`, `home.smoke`, the counting stub `test/homeStub.ts`; amended App, routes, InboxView, smoke, Dialog, the TasksView and GraphView listener tests, `policy.test.ts`'s `href` rule and a no-clock rule); lint 0, build, 15 packages. Iteration 2 fixed the review's nine findings: the project fallback, Close and the scrim click, option IDs, a chord test, a test name, the waiting emphasis, the intros, IME Esc, folding once.

Deviations, accepted: the home lives in `src/overview/`, not the planned directory named after it (`anonymity.rs` rejects that path segment anywhere); added the key hint, "No other open task.", the Projects read error, the no-hit line, "<n> options" on opening.
