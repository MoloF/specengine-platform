---
class: spec
status: draft
scope: [ui]
ref: RM-0084; v4 step 2b of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"), his Flow layout and the defects he hit; Q9 and Q10 are working answers, awaiting the owner's review
adrs: []
---

# writer-studio: Flow in three panes, one scope at a time, the place in the URL (RM-0084)

## Why

### Requirement

On 2026-11-08 the owner could not work on Flow. The causes, found in the code and confirmed in his browser:
- **An edge click falls through.** Edges have no handler and no hit area (`FlowCanvas.tsx` `interactionWidth={0}`), so the click lands on the open group's frame below. `onNodeClick` then selects the group, pushes a hash and re-centres.
- **The panel stretches the page.** It borrows Graph's `clamp(22rem, 68vh, 46rem)` (`.graph-details`) beside a canvas 12-24rem tall, `align-items: start`. One of his flows made the page 8 839 px tall in a 1 086 px viewport.
- **The Stage filter only dims** (`--flow-dimmed`) and judges no group, so at Top it does nothing.
- **The List** prints `path:line` rows no one can click.
- **The header** (about text, notes) **and the always-open legend** take the screen.

His layout: a left pane by stage with the scope's sub-groups, bound to the URL (reload, Back and Forward keep the place); a canvas of the current scope only; a right pane always visible, with detail, description and actions, nothing jumping; few canvas actions; the legend a thin bottom bar expanding over the content; a one-line header; one screen, no page scroll.

This slice rebuilds Flow in place, over 2a's stage read (RM-0083) and 1a's module (RM-0081). It follows 1b (RM-0082): its strings are keys of 1b's catalog and its letter keys go through 1b's `letterOf`. The sweep (RM-0090) and the studio trial follow (08 s2 Phase 7).

### Decisions

- **D1 One scope at a time.** The canvas draws one read: the scope's children, or one stage's documents within it. A group is *entered*, a scope change, never opened in place. That leaves no frames, no open-group set and no nested lanes, so the frame an edge click fell through to is gone.
- **D2 The read shape.** Each picture is one of three `getFlow` forms, always with `trace`: Top `{depth: 0}`, a scope `{root: R, depth: 1}`, a stage `{root: R, stage: S}`. A stage read always carries the scope's root: without one it admits every document of the stage, a whole read in payload. The stages come from `getTrace({ref: R, summary: true})`, the scope's text from `getNode(R)`, a subset read; `getTree` only seeks a name no read holds. No daemon change: `stage` is 2a's query.
- **D3 The lens root**, the scope when the hash names none: the chosen module's `root`; else Top's one node, when Top holds exactly one and it holds documents; else the unrooted Top, offering no stage (D2), only its groups.
- **D4 The place is in the hash:** `#/<p>/flow[/<name>][?module=&scope=&stage=&edge=]`; the old `#/<p>/flow/<name>` still selects. With the module in it, a reload, a shared link and Back draw one picture whatever the viewer stored; 1a's `specengine.module.<p>` is the default for a hash without one, written only by the bar.
- **D5 History.** A scope, stage or module change pushes. A selection, a seek's fix and a correction replace. In the Shell's life, a bare `#/<p>/flow` (the nav link) is replaced by the project's last place.
- **D6 Three fixed panes.** Places 15rem, panel 22rem, canvas at least 20rem. Above 64rem the studio fills the viewport below the shell and the page never scrolls. From 64 to 80rem the places pane folds into a disclosure; under 64rem the panes stack.
- **D7 The panel is always mounted.** With nothing selected it shows the scope, its actions and its gap rows. Retired with it: the gaps column, Hide gaps, `specengine.flow.gaps-hidden`, the Stage filter, the Timeline tab (folded into the places pane) and `--flow-dimmed`.
- **D8 An edge click selects the edge.** `onEdgeClick`, a 12 px hit area; the inspector shows from, to, type, kind, when, count, `path:line`. Mouse only: no tab stop on edges; the keyboard goes through the List (D10).
- **D9 The canvas holds still.** The wheel pans; a pinch or xyflow's zoom key zooms. A picture fits once, when its read lands; then only the owner, `c` and a gap activation move it. No `onlyRenderVisibleElements`: a read holds at most 200 boxes, which the 200-box trial draws, and it would unmount boxes the arrow keys reach.
- **D10 The List is an outline** (cut 2c): every node and edge of the drawing read, each row a link that selects it, `path:line` in the panel only (Q10).
- **D11 A gap goes to the AI as a card** (cut 2c). "Hand to your AI" is New card with the item's members as targets and its `cause` as goal. It never makes a `T-` task (Q9, an owner stop).
- **D12 Keys by 1b's rule.** A letter key compares `letterOf(event)` (`ui/src/ui/keys.ts`), so `c` acts under a Russian layout. No new letter key: Space, which opened a group, now enters it.
- **D13 Strings in the catalog.** Every chrome string of a `src/flow/` view is a `flow.` key of 1b's catalog (`ui/src/i18n/en.ts`, its Russian in `ru.ts`) or a `labels.ts` word, data a placeholder; `src/flow/` joins 1b's scan (`ui-languages.md` LG-10).
- **D14 Graph and the Roadmap Chain are untouched**, `.graph-details` and its clamp included.

### Assumptions

- **SA1** A rooted read holds its root as a node, with `group` set to its holder; `mocks/flow.test.ts` pins this for DOM-HARBOR. Up reads it.
- **SA2** A rooted `spec trace --summary` lists the scope's lanes by the scope's walk (`docs/canon/spec-trace.md` "The read").
- **SA3** xyflow 12.12 clicks an edge once `onEdgeClick` is set (else edges take `selectable: true`, the hash still the store); jsdom keeps hash history (else tests spy on `pushState` and `replaceState`).

## Description and interactions

Today's canon: `spec-flow-screen.md`, `spec-flow-layout.md`; the reads, `spec-flow.md`, `spec-trace.md` (`docs/canon/`).

**Header**, one line: the `h1` "Flow"; the scope's title (` · <stage>` when chosen; an ellipsis when cut); the Canvas and List tabs; one disclosure, `aboutButton(n)`, holding the about text and each note of every read shown, once.

**Places pane.** A `<nav>` named "Flow places":
- 1a's module bar, moved here, when modules are served.
- The scope: Up, then its title (`aria-current="location"`). Up names the scope node's `group`, else the lens root; none at the lens root; in a stage view, shown once the scope's read is held.
- Stages: All stages, then the summary's lanes in served order, No stage last. Each lane shows `<c> of <n> confirmed` and its bar, the Timeline's lane folded in.
- Under the current item: for All stages, the scope's groups (`holds > 0`); for a stage, the distinct `group`s its nodes name, first seen, the scope itself left out.

Every item is an `<a>` with a `flowHash` `href`, so a click pushes. At an unrooted Top the pane shows `stagesAtTop` and Top's groups, and reads no stages.

**Canvas.** A `region` named "Flow canvas", the boxes as today. A click selects a box; a double-click, its caret, Space or the panel's Open enters a group; a click on the pane does nothing.

**Bottom bar.** One line: the Legend toggle (`aria-expanded`) and, past the cap, `notDrawn`. Open, the legend lies over the canvas from its bottom edge, at most half its height, and scrolls inside.

**Panel.** `role="complementary"` named "Details", at `--flow-panel-width`, scrolling inside:
- **The scope**, with nothing selected: title, name, `kind`, counts and bar (the node's `counts`); its text and links (`getNode(R, {with: ["links"]})`); Open in Spec tree; New card (the scope as target); the gap rows. At Top: `topTitle` and the rows.
- **A box**: today's panel (`docs/canon/spec-flow-screen.md` "Panel"), plus `group` with Open, and `backTo(scope)` at its top, acting as Esc.
- **An edge**: From and To, each a link selecting that end (a group end adds Open); Type, raw; Kind, raw or `none`; When, or `none`; Links, the `count`; `path:line`, the first written.

**Gap items.** Activating one selects its first member (a seek pushes its scope if undrawn), marks every drawn member, centres the first, focus to the panel heading. Beside each measured item, "Hand to your AI" opens D11's New card; Create is one `createCard`. A `no-start` item hands the scope's root when rooted, nothing at Top.

**List** (2c), in served order: each box an `<a>` selecting it (a group adds `<holds> documents` and Open); under it, its outgoing edges, each a link selecting the edge: `<type> <to name>, <to title>[, <kind>][, when <when>][, ×<count>]`; an edge from the scope's own node first, under the scope's name; in a stage view, boxes under their `group`.

No row prints `path:line`. Under 48rem the List is the default tab unless another tab is remembered.

**Keys.** As `docs/canon/spec-flow-screen.md` "Keyboard, motion, width", but arrows move among boxes only, Space enters a group, Enter selects (focus to the panel heading), Esc closes the legend if open, else clears the selection (focus back where it was made), and `?` lists Space. One tab stop on the canvas; none on edges.

**What it meets.** `CardsBlock` is unchanged; the Tree's links into Flow keep the old form, which the seek places; live events as today (`READS_OF_THE_SPEC`).

## Data

### The place (`ui/src/app/routes.ts`)

```text
#/<p>/flow[/<name>][?<key>=<value>[&<key>=<value>]...]
module=<m>                         a served module's name; empty: All modules
scope=<ref>                        the read's root: a group's name, a module's root
stage=<s>                          a served lane's stage; `-`: No stage
edge=<from>,<to>,<type>[,<kind>]   no fourth part: kind null
```

```ts
interface FlowPlace {
  module: string | null; // null: not in the hash, 1a's stored choice; "": All modules
  scope: string | null; // null: the lens root (D3)
  stage: string | null; // null: All stages; "-": No stage
  select: { node: string } | { edge: FlowEdgeKey } | null;
}
interface FlowEdgeKey { from: string; to: string; type: string; kind: string | null }
```

- **Splitting.** `parseHash` splits off the first raw `?` for `flow` only; a `?` in a Tree id stays in it.
- **`flowHash(project, place)`.** Each value and each `edge` part goes through `encodeURIComponent`; the parts are joined by a raw `,`. Keys run module, scope, stage, edge, each written only when non-null. The name is the path segment, as `sectionHash` writes it.
- **`parseFlowPlace(route)`.** Decodes with `decodeURIComponent`, never `URLSearchParams` (`+` is no space). A key drops when its value fails to decode, when the key is unknown, when `scope` or `stage` is empty, or when `edge` has fewer than three parts or an empty one. The first of a repeated key wins, and a name wins over an `edge`.
- **Written hashes** name the effective module whenever `[[module]]` serves one (`module=` for All), never otherwise. A literal stage `-` cannot be asked, as in `spec trace`.

Examples:
- `#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&stage=quayside`
- `#/harbor-sim/flow?module=&edge=MEC-FAIRWAY,DOM-BERTHS,follows,alternative`, a lifted edge
- `#/harbor-sim/flow/MEC-MOORING`, an old link, which seeks

### The reads

| Picture or pane | Call | When |
|---|---|---|
| Top | `getFlow({depth: 0, trace: true})` | no module, no scope: once, finding the lens root; drawn if it is one |
| scope R | `getFlow({root: R, depth: 1, trace: true})` | All stages |
| stage S in R | `getFlow({root: R, stage: S, trace: true})` | `-` for No stage |
| stages | `getTrace({ref: R, summary: true})` | each rooted scope; never at Top |
| scope view | `getNode(R, {with: ["links"]})` | each rooted scope |
| box N | `getNode(N, {with: ["links"]})`, `getTrace({ref: N})` | N selected |
| seek | `getTree({})`, on demand | a name no drawing read holds |

Drawing keys: `staleTime: Infinity`, `refetchOnMount: "always"`: a place read before draws from its kept answer, read again on a live event that marks it or on the next entry. `useTraceLanes` leaves Flow. A scope change costs two whole-work reads, a stage change one (`docs/canon/scale-eval.md` "Baseline": flow about 1 s, the summary 0.54 s at 100k).

### State, client, tokens

- **`FlowState`**, per project in the Shell, for the page's life: `{tab: "canvas" | "list" | null, place: FlowPlace | null}`. `root`, `open`, `stage` and `lanes` retire. The hash is the store, and `place` only replaces a bare hash.
- **Client.** `FlowOptions` gains `stage?: string`. `HttpClient.getFlow` sends `root`, `depth`, `stage`, `trace`, citing `docs/canon/spec-flow.md` "The read". No `provisional.ts` type and no route change.
- **`NewCardDialog`** gains `preset?: {title: string; goal: string; targets: readonly string[]}`, one target per line in the form. `target` stays for the Cards block.
- **Tokens.** `--flow-places-width: 15rem`, `--flow-panel-width: 22rem` and `--flow-canvas-min: 20rem` are added. `--flow-dimmed` goes, with its `tokens.test.ts` pin. `geometry.ts` gains `EDGE_HIT = 12`.
- **Classes.** `.flow-studio`, `-head`, `-places`, `-centre`, `-canvas`, `-bar`, `-legend` and `-panel`; no `graph-details` class in Flow's DOM.
- **Dashes.** `FLOW_DASHES` and `--flow-edge-*` stay (a read fills three type places); a test pinning a fourth served type pins the constant.

### Strings (`flow.` in `ui/src/i18n/en.ts`)

At least these, each with its Russian in `ru.ts`; the views' strings move in. `f(a)` is a key with `{{a}}`; a count is `{{count}}`, making it plural.

- `placesLabel` `Flow places`, `placesToggle` `Places`, `up(name)` `Up: <name>`, `topTitle` `The whole project`
- `stagesHeading` `Stages`, `allStages` `All stages`, `stagesAtTop` `Stages show inside a group: open one.`, `laneCounts(c, n)` `<c> of <n> confirmed`, `stagesFailed` `Stages could not be read.`
- `aboutButton(n)` `About` at 0, else `About and notes (<n>)`; `canvasLabel` `Flow canvas`, `panelLabel` `Details`, `legendButton` `Legend`
- `notDrawn(k, n)` `<k> of <n> boxes not drawn: choose a stage, enter a group, or read the List`
- `emptyStage(s)` `No step here carries <s>.` (`-`: `Every step here has a stage.`), `emptyScope(name)` `Nothing is inside <name>.`
- `edgeGone` `That arrow is not in this view.`, `edgeHeading` `Arrow`; the inspector's `From`, `To`, `Type`, `Kind`, `When`, `Links`, `none`
- `openGroup(name)` `Open <name>`, `backTo(name)` `Back to <name>`
- `handToAi` `Hand to your AI`; `handTitle(code, members)` `Gap <code>: <m1> + <m2>…`, and past 120 characters `Gap <code>: <m1> and <k> more`
- `showing(scope, n, stage)`, announced politely: `<scope>, <n> boxes[, <stage>]`

### Mocks (`ui/src/mocks/`)

- **harbor-sim** gains `DOM-PILOTS` under `DOM-HARBOR`, in the corpus, tree and trace, so `landfall` and `quayside` each span two groups: `MEC-PILOT-CALL` (`landfall`, following `RULE-SPRING-WINDOW`); `MEC-HARBOUR-LOG` (`passage`, following it; a 160-character title, an 8 KB body with a 300-character token); `MEC-PILOT-RETURN` (`quayside`, following `MEC-MOORING`).
- **The `depends_on` flow edges leave**, moved here from 2a: `MEC-FAIRWAY -> RULE-HIGH-WATER`, and ledger-api's `EP-REFUNDS -> POL-WRITES`. ledger-api keeps no `[flow]`.
- **`getFlow` takes `stage`**, the cut after the fold of `spec-flow.md`. Under a ROOT, the mock serves Top's rows, keeping the items with a member in scope; this is the mock's reading. **`getTrace`** takes `ref` with `summary`.
- Other scenarios keep their roles.

## Rules and edge cases

- **R1** The screen SHALL issue only "The reads". A `getFlow` holding `stage` holds `root`; none lacks both `root` and `depth`.
- **R2** WHEN the hash names a stage at an unrooted Top, or a module that is not served, the screen SHALL drop it by a replace.
- **R3** WHEN the selected name is not in the drawing read, a seek SHALL run once per place: `getTree`, then a replace with `{module, scope: parent, stage: null, select}` (a gap's seek pushes). A parent equal to the lens root gives `scope: null`, a top document `module: ""`. A name the tree lacks: a note and a Tree link. A name equal to the scope's own node is the scope view; its selection is dropped by a replace.
- **R4** WHEN the selected edge is not drawn, the screen SHALL drop it by a replace and announce `edgeGone`.
- **R5** WHEN the scope names nothing, the canvas SHALL show the read's `reason`, the places pane the lens root; a scope holding nothing shows `emptyScope`, a stage of 0 nodes `emptyStage`; the rows stand.
- **R6** The cap: the first 200 (`CANVAS_LIMIT`) nodes in served order are drawn, with the edges among them; the bar counts the rest, and the List holds them all.
- **R7** With no `[flow]` start, no read changes: the walk starts at derived starts; with none, `distance` is null, stages keep the summary's order, boxes stand "Not on the walk", and `no-start` and `unreachable` say why. With no `[flow]`, rows needing it read `Not configured`; the canvas is drawn.
- **R8** A failed drawing read shows `ErrorPanel` in the canvas, verbatim, with Retry; the rows and marks read "Not read" (`withoutStates`). A failed summary shows `stagesFailed` and Retry in the places pane; the canvas stands.
- **R9** Focus stays on the link that changed the scope while it stands; after Space, it goes to the new picture's first box; lost, to the canvas region, never `body`.
- **R10** An open legend closes on a focus or click in the canvas, so nothing focused is under it. Above 64rem the bar is the centre's last grid row; under 64rem it is sticky, `scroll-padding-bottom` its height.
- **R11** Under 64rem `panOnScroll` is off and `preventScrolling` false: the wheel scrolls the page, and a drag pans.
- **R12** Nothing is blocked (ADR-0012): no hold word; a hand-off changes no status; the UI writes cards only (`docs/canon/architecture.md#ui`); no link type is quoted in app code, `type` shows raw.

## Acceptance criteria

The tests run on the mocks, the looks on the built UI. Each named mutation must turn its check red.

- [ ] **ST-1 The place round-trips** (`app/routes.test.ts`).
  - For 200 places or more, `parseFlowPlace(parseHash(flowHash("harbor-sim", x))) == x`, crossing module (null, `""`, `berths`), scope (null, `DOM-BERTHS`, a path with `/ # ? & = , + %` and a space), stage (null, `-`, `quayside`, `a b+c&d=e/f?#,%`, a Cyrillic word) and select (null, two names, edges with kind null or `a,b`).
  - `#/harbor-sim/flow/MEC-MOORING` parses as `{module: null, scope: null, stage: null, select: {node: "MEC-MOORING"}}`.
  - A name beside an `edge` wins; other sections parse as before. M1 `URLSearchParams`. M2 the old form not found. M3 edge parts unencoded.
- [ ] **ST-2 Back and reload.** From `#/harbor-sim/flow?module=`, enter DOM-BERTHS and choose `quayside` (pushes), then select MEC-MOORING and MEC-OLD-QUAYS (replaces): `history.length` grows by 2; Back shows DOM-BERTHS with no stage or selection, Back again the lens root. A fresh mount on `#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&stage=quayside` draws that stage with MEC-MOORING selected; in the same Shell, the bare `#/harbor-sim/flow` is replaced by the last place. M4 a selection pushing. M5 `FlowState` as the only store. M6 the module bar replacing.
- [ ] **ST-3 An edge click selects it.** A click on the interaction path of an edge in the lens root's picture (`.react-flow__edge-interaction`, stroke width 12 or more) fills the panel with its served from, to, type, kind or `none`, when or `none`, count and `path:line`. The hash gains `edge=` by a replace, no name; no read is made; the viewport transform stays. Plus L2. M7 `interactionWidth={0}`. M8 no `onEdgeClick`. M9 the click also reaching the pane or a box.
- [ ] **ST-4 The page holds still.** A CSS pin over `app.css?raw`: `.flow-studio` takes the viewport (`dvh`) less the shell, `overflow: hidden`; `.flow-places` and `.flow-panel` take `overflow: auto` and `min-height: 0`, the panel fixed at `--flow-panel-width`; no `.flow-*` rule holds `clamp(`, `68vh` or `align-items: start`; Flow's DOM has no `graph-details` class. Plus L1. M10 the clamp and `align-items: start` restored.
- [ ] **ST-5 The panel is always there.** With nothing selected, "Details" is mounted with the scope's title, counts, Open in Spec tree, New card and ten rows; selecting a box, then Esc, swaps its content in the same element. No gaps column, no Hide gaps: a stored `specengine.flow.gaps-hidden=on` hides nothing, and a Storage spy sees the key neither read nor written. M11 the panel unmounted with no selection. M12 the key read.
- [ ] **ST-6 A stage narrows.** Under DOM-HARBOR, the link `landfall` pushes and issues `getFlow({root: "DOM-HARBOR", stage: "landfall", trace: true})`. The canvas holds exactly RULE-HIGH-WATER, RULE-SPRING-WINDOW, MEC-PILOTAGE and MEC-PILOT-CALL, MEC-FAIRWAY absent from the DOM; the pane lists their distinct `group`s but DOM-HARBOR, two or more. No stage gives the steps with none. `--flow-dimmed` is in neither CSS file. M13 dimming restored. M14 the stage read without `root`.
- [ ] **ST-7 One-line header.** The `h1` "Flow", the scope's title, the two tabs and one disclosure; the about text and each note of every read shown, once, only inside it, `aria-expanded` following. M15 the about text inline.
- [ ] **ST-8 Legend at the bottom.** The bar is the centre's last row; the open legend is absolute at `bottom: 0` (CSS pin). The first Esc closes it, focus to its toggle, the second clears the selection; a focus in the canvas closes it. M16 Esc clearing the selection while the legend is open. M17 the legend in the layout's flow.
- [ ] **ST-9 The List outline (2c).** On the lens root and on `landfall`, the List holds every node and edge of the read, past the cap too; each row an `<a>` with a `flowHash` selection `href`, a click replacing; edge rows read `<type> <to name>, <to title>…`; no row holds `path:line`, the panel does. M18 a box missing. M19 a plain-text row. M20 `path:line` in a row.
- [ ] **ST-10 Landmarks and keys.** The places pane is a `nav` "Flow places", apart from the app nav, its items `<a>` with `flowHash` `href`s; the canvas a `region`, the panel a `complementary`; one `h1`. The canvas has one tab stop, no edge is focusable, and the keys act as "Description" says. M21 places items as `div` or `button` with handlers. M22 `edgesFocusable`.
- [ ] **ST-11 Narrow widths.** A CSS pin: under 64rem, one column (header, places disclosure closed with `aria-expanded`, canvas, panel); from 64 to 80rem, the places pane is the disclosure, with canvas and panel side by side. A stubbed `matchMedia` under 64rem sees `panOnScroll` off. Plus L3 and L4. M23 a fixed places width under 64rem.
- [ ] **ST-12 Hand to your AI (2c).** Under DOM-HARBOR, on `cycle-no-repeat`'s item MEC-FAIRWAY + MEC-PILOTAGE, New card opens with both targets, the `cause` verbatim as goal and the title `Gap cycle-no-repeat: MEC-FAIRWAY + MEC-PILOTAGE`. Create sends one `createCard`, `target` both members in order, `goal` the cause; MEC-FAIRWAY's Cards block then shows the card; no task call is made. M24 the first member only. M25 the cause rewritten.
- [ ] **ST-13 The reads stay bounded.** A counting client over the mock, the workspaces read aside:
  - (a) `#/harbor-sim/flow?module=` issues exactly `getFlow {depth: 0, trace: true}`, `getFlow {root: "DOM-HARBOR", depth: 1, trace: true}`, `getTrace {ref: "DOM-HARBOR", summary: true}` and `getNode "DOM-HARBOR"`;
  - (b) `quayside` adds its stage read alone; (c) selecting MEC-MOORING its `getNode` and `getTrace {ref}`; (d) entering DOM-BERTHS its scope read, summary and `getNode`; (e) Back, Back, Forward nothing;
  - (f) with All modules stored, the old link `#/harbor-sim/flow/MEC-PILOT-RETURN` adds one `getTree {}` and DOM-PILOTS' three reads, and is replaced by `#/harbor-sim/flow/MEC-PILOT-RETURN?module=&scope=DOM-PILOTS`;
  - (g) `module=berths` issues no `{depth: 0}`.

  Throughout: every `getFlow` is one of D2's forms, no `getTrace` is `{}` or has `stage`, and `getTree` appears in (f) alone. M26 a `getTree` drawing the places pane. M27 a stage read with no root. M28 an unrooted summary under a scope, or one summary per stage change.
- [ ] **ST-14 Strings in the catalog.** 1b's scan (`ui-languages.md` LG-10) covers `src/flow/`: no JSX text with a letter, no string-literal text attribute; LG-1 holds for every `flow.` key. M29 a literal `Open` in a view.
- [ ] **ST-form.**
  - `pnpm lint`, `pnpm build` and `pnpm test` pass, as do `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers` and `spec check` (0 errors, 0 warnings).
  - `GraphCanvas.tsx`, `RoadmapChain.tsx`, `src/graph/` and the `.graph-details*` and `.roadmap-*` rules are byte-equal to the base.

### Looks

`ui/` has no browser harness: by hand or a headless browser on the built UI and mock, with real CDP input, never `element.click()`; each dated in "Implementation".

- [ ] **L1** At 1440x900 and 1920x1080, on `?scenario=normal#/harbor-sim/flow/MEC-HARBOUR-LOG?module=&scope=DOM-PILOTS`: `scrollHeight <= innerHeight`, `scrollWidth <= innerWidth`; the canvas's `getBoundingClientRect()` within 0.5 px before selecting, after it, after scrolling the panel to its end and after opening the legend; the header one line high.
- [ ] **L2** At 1440x900, a real mouse click at a drawn edge's midpoint, and another 5 px off it, selects that edge; `history.length` and the viewport stay unchanged.
- [ ] **L3** At 320x640, `scrollWidth <= 320`, and the places pane is a closed disclosure.
- [ ] **L4** At 1100x800, the page does not scroll, the canvas is 20rem wide or more, and the places pane is the disclosure.
- [ ] **L5** `spec serve` over a scratch copy of `fixtures/trace-almanac` lands on BOOK-ALMANAC (D3); `gather` draws exactly STEP-CALL and STEP-SORT, PART-GATHER and PART-PRINT under it; the console stays clean.

**Tests to add** (`ui/src`): `app/routes.test.ts` ST-1; `flow/FlowView.test.tsx`, rewritten, ST-2, ST-3, ST-5 to ST-13; a new `flow/styles.test.ts`, as the Board's, ST-4, ST-8, ST-11; `policy.test.ts` ST-14; `mocks/flow.test.ts` (the stage cut, no `depends_on`, rows under a ROOT) and `mocks/flow.smoke.test.tsx`; `styles/tokens.test.ts` loses the `--flow-dimmed` pin. No Rust test or fixture changes.

## Out of scope

Canvas editing; a `T-` task; a card's discipline from the workspace (RM-0085); Graph and the Chain; any daemon, CLI or MCP change; the other screens' strings (RM-0090); a parent chain beyond Up; exit steps; a lifted edge's members; journeys; module filters on the Board and Roadmap; `[[flow.exclude]]`; RM-0070.

### Risks

- **R-cost.** At 100k a scope change waits about 1.5 s on two O(N) reads, each taking a hub's project turn: measured, not fixed, a known limit.
- **R-size.** `FlowView.test.tsx`, 2 362 lines, assumes open groups. On an overrun, cut 2c takes ST-9 and ST-12: the List keeps today's rows; the rows lose the hand-off.
- **R-header.** A shell header wrapping above 64rem fails L1: the studio then measures it, never a constant.

## Canon at ship

On each file touched, `reviewed:` moves to the ship date. Byte counts are today's.

- **`docs/canon/spec-flow-screen.md`** is rewritten whole, at most 11 800 B (12 282 today).
  - D1-D14 and R1-R12 become its text, R1 as "No whole read"; "Timeline", "Stage filter", frames, open groups and Hide gaps go; "Panel" stays (`board-ui.md` cites it).
  - "Known limits" gains R-cost, Up after the scope's read, edges mouse only, one sought member, 320 px by a look.
- **`docs/canon/spec-flow-place.md`**, new, Tier 2, `scope: [ui]`, at most 4 096 B.
  - Sections: "The place" (grammar, encoding, written hashes, examples), "History" (D5), "The module" (D4, R2), "The seek" (R3, R4), "Memory" (`FlowState`).
  - Its index line goes in `docs/index-canon-spec.md`.
- **`docs/canon/spec-flow-layout.md`** (2 860 B, at most 3 100 after):
  - "Stacking and view": "The canvas takes the height of the fitted picture and its controls, within the Graph's measure;" becomes "The canvas fills the studio's centre pane; the wheel pans it, a pinch or the zoom key zooms it, and a selection moves nothing;" (+31 B).
  - "Layout": "a lane per read, nested." becomes "one read, one lane." if the nesting code goes.
- **`docs/canon/spec-flow-readings.md`**: no change (0 B): it reads the CLI's cases; the studio adds no read.
- **`ui/README.md`**: net 0 B over 1b's. Any further word goes to the Flow canon.
  - "Canvas": "the wheel scrolls the page;" becomes "the wheel scrolls the page, Flow's pans it;" (+16).
  - "Shell": "(bare: its home)" becomes "(bare: its home; Flow's place: `spec-flow-place.md`)" (+36).
  - "Shell": "(Questions: `round-ui.md`; `FlowState` with `stage`, `lanes`)" becomes "(Questions: `round-ui.md`)" (−35).
  - "Shell": `` `specengine.{flow.gaps-hidden,board.as,workspace.<p>,module.<p>}` `` drops `flow.gaps-hidden,` (−17).
- **`docs/canon/workspaces.md`** "Screen", Module bar: "Flow's Top read takes `root` (`FlowState.root`, no `memory.remember` until known)" becomes "Flow's lens root, the module also in its hash (`spec-flow-place.md`)" (−13 B).
- **`07-interfaces.md`** s3, the Flow row: three panes, one scope's canvas, a panel always shown (the scope's rows or the selection), the List as its text twin, the place in the hash, cards (a gap's too) written from the panel.
- **`08-roadmap.md`**: row 2b shipped. **RM-0084**: `status: shipped`. **`CLAUDE.md`** "State": `writer-studio` joins v4's shipped items.
- **No change**: `architecture.md` (`#ui` and `#control` hold), `spec-flow.md`, `spec-trace.md`, `board-ui.md`, `cards.md`.

## Implementation

Filled in after implementation.
