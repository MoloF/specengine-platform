---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, fixtures, ui]
ref: RM-0113; the owner's choice of 2026-11-13 (scope rings, an extra beside the writer map); an earlier prototype's rings tab (rings as scope layers, sectors at equal angles, the fill a state); the analyst's analysis of 2026-11-13
adrs: []
---

# scope-rings: the project's nodes on concentric rings, the radius their scope

## Why

A writer asks what a mechanic touches: the whole world, one thing, or one actor. A tree shows where a document is filed, and a map shows what links to what. Neither shows scope at a glance.

An earlier prototype drew scope as concentric rings, from the widest scope inside to the narrowest outside. Each mechanic was a sector at an equal angle, and the fill showed how far along its work was. The owner chose to bring this in.

SpecEngine names no domain (ADR-0008), so the rings come from the project's config: each ring is a set of nodes chosen by a root, by kinds, or both. Nothing is stored, and no dependency is added.

## Description and interactions

### Decisions

- **D1. Rings come from config, not from a front-matter field.** `[[map.ring]]` rows each hold a `title` and a `root`, `kinds`, or both.
  - A field would not do. A section has no front-matter, and pilot A's mechanics are sections. An unknown key is an `unknown-key` finding, and a new typed key changes the model for one view.
  - The config reuses what a `[[module]]` already means: a root and its subtree, or a kinds direction (`workspaces.md` "Config", `directions.md` "Config").

  No ADR: no rule of `docs/canon/architecture.md` changes, and ADR-0008 already puts a project's words in its config, as `[flow]` and `[roadmap]` are.
- **D2. A ring's sectors are a tree read.** Ring i holds exactly the lines `spec tree [<root>] [--kind <k>]…` lists, in its order, less any node an inner ring already holds. Rings are innermost first. A node sits in one ring at most, the first that lists it.
- **D3. The fill is the work state.** It is the trace's rollup (`spec-trace.md` "Marks and rollup"), the catalog's `state`: `no card`, `in work`, `confirmed`, `suspect` or `departed`. The raw `status:` shows in the card and the List. There is no new state.
- **D4. A bad table serves no ring and fails nothing**, as `[[module]]` does: one note, the first problem by line. There is no check code. `ProjectConfig` gains `map` among its name-checked tables, so the file still loads.
- **D5. One read of its own.** `spec rings [--json]` and `GET …/rings`, computed per read. There is no MCP tool: an agent has `get_tree` with `kinds`.
- **D6. Plain SVG**, no dependency, as the Rings view of the Graph section's home (`corpus-map` D10). Whichever of the two slices lands first creates `graph/GraphHome.tsx`.
- **D7. Every sector is reachable by keyboard.** The SVG is one tab stop with roving focus. Each sector is a `role="link"` named in words, and Enter opens the document. The List twin holds the same rows.

### What the owner sees

Graph's Rings view shows one ring per `[[map.ring]]`, the first in the middle:
- each ring is cut into equal sectors, one per node, filled by its work state;
- the ring's title runs along its band, and a legend names each state with its icon and the rings in order;
- a search field dims everything but the matches, and state chips hide or show each state;
- hovering or focusing a sector shows its title, kind, state and status, and a click or Enter opens the document.

With no `[[map.ring]]`, the view explains in a few lines how to add one.

## Data

### Config (core `map_toml.rs`, new; `project_toml.rs` `RawFile` gains `map`)

```toml
[[map.ring]]
title = "Movement"
root  = "DOM-MOVEMENT"
kinds = ["mechanic"]

[[map.ring]]
title = "Rules"
kinds = ["rule"]
```

`MapConfig::from_toml(&str) -> MapConfig {rings: Vec<RingConfig>, notes: Vec<Note>}` is pure and never fails. `RingConfig` is `{title, root: Option<String>, kinds: Option<Vec<String>>, line}`. The keys follow `workspace_toml.rs`' readers:
- `title` is required, as a `[[module]]`'s title (`title_problem`: 1-64 UTF-8 bytes, no control character);
- `root` is a rooted module's: 1-256 bytes, no control character or end space, a REF in `spec show`'s forms, not resolved here;
- `kinds` is a direction's: the shared list reader, 1-32 entries, each checked as `directions.md` "Config" says.

A ring holds `root`, `kinds` or both, and there are at most 12 rings (`MAP_RINGS_MAX`). `[map]` holds `ring` alone. Any problem serves no ring and gives one note, the first by line:

```text
<config>:<line>: <problem>; no ring is served
```

`<problem>` is one of the following:
- the TOML reader's (a wrong type, or an unknown key: `` expected `title`, `root` or `kinds` ``);
- `` `[map]` holds only `ring` ``;
- `` `[[map.ring]]` is required to hold `title` ``;
- `` `[[map.ring]]` `<title>` holds neither `root` nor `kinds`: a ring holds one or both ``;
- `` at most 12 `[[map.ring]]` tables ``;
- the title, root and kinds problems as `workspaces.md` and `directions.md` word them.

### `spec rings [--json]` (core `rings.rs`, CLI `rings.rs`, both new)

The input is `spec tree`'s. For each ring in order, the read takes the lines `spec tree` gives for its root (that root's holders; none, the whole tree's roots) and its kinds (none, every kind), live only, and drops the nodes an inner ring holds. The states come from the trace's reading, built once for all rings (`node_work`, as the catalog's). A root that resolves to no node leaves its ring empty, with a note: `` rings: ring <i> `<title>`: `<REF>` resolves to no node ``.

On a copy of `fixtures/spec-a` with Data's config:

```text
rings 2 | sectors 6
ring 1 | Movement | root DOM-MOVEMENT | kinds mechanic | sectors 2
  MEC-SPRINT | mechanic | no card | Sprint | docs/spec/movement/sprint.md:1
  MEC-STAMINA | mechanic | no card | Stamina | docs/spec/movement/stamina.md:1 | status accepted
ring 2 | Rules | kinds rule | sectors 4
  RULE-CORE-LOOP | rule | no card | Core loop | docs/spec/game.md:21
  RULE-MOVE-SPEEDS | rule | no card | Speeds | docs/spec/movement/README.md:16
  RULE-SPRINT-COST | rule | no card | Cost | docs/spec/movement/sprint.md:18
  RULE-STAM-REGEN | rule | no card | Regeneration | docs/spec/movement/stamina.md:21
```

```json
{"notes": [],
 "rings": [
  {"title": "Movement", "root": "DOM-MOVEMENT", "kinds": ["mechanic"], "sectors": [
    {"name": "MEC-SPRINT", "id": "MEC-SPRINT", "kind": "mechanic", "title": "Sprint", "path": "docs/spec/movement/sprint.md", "line": 1, "status": null, "state": "no card"}]},
  {"title": "Rules", "root": null, "kinds": ["rule"], "sectors": []}]}
```

That JSON is abridged: ring 1 holds both mechanics and ring 2 its four rules. Without `[map]`, the read answers `{"notes": [], "rings": []}` and `rings 0 | sectors 0`. The config notes go to stderr as `note:` lines and into `notes`, sorted. Text values go through `one_line`. A cut at `OUTPUT_CAP_CHARS` falls at a line and keeps whole rings while they fit, with the tail `[truncated: <k> of <n> sectors not shown]`. The JSON follows the cut; the daemon's is whole. The exits are as `spec tree`'s, but an unresolved root is a note, never exit 1.

**Daemon.** `GET /api/projects/:p/rings` serves `spec rings --json` less its final LF, uncut, in the project's turn. Any query name is a 400; exit 2 is a 503. The 404 list names `rings` after `map`, or after `catalog` when `map` is not yet served. The key sets are `Rings` (2), `Ring` (4) and `RingSector` (8).

### The UI (`ui/src/rings/`, new; the lazy `RingsView.tsx`)

- **Seam.** `provisional.ts` gains `RingsDocument`, `Ring` and `RingSector`. `client.ts` gains `getRings(project, signal)`. `http.ts` and `queries.ts` follow (`useRings`, key `["rings", p]`, in `READS_OF_THE_SPEC`, read again on `card.*` as the catalog is). The mock (`mocks/rings.ts`): harbor-sim has three rings (12, 5 and 1 sectors) with every state, a sector with a long title and an empty ring; `notes` holds a bad table; `empty` has none.
- **Geometry** (`rings/geometry.ts`, pure). The `viewBox` is 1000 × 1000. Ring i's band runs from `R0 + i·W` to `R0 + (i+1)·W`, with `R0` 80 and `W` set by the ring count to fit 460. A sector's angle is 2π/n, starting at 12 o'clock and running clockwise. One sector is a full band, and a gap of 1 unit separates sectors. `sectorPath(ring, index, count)` returns the path.
- **Drawing:**
  - each sector is a `<path>` filled with its state's tone (`rollupLook`'s role, no new colour) and stroked with `--surface`;
  - a label shows only when the arc at mid-radius is at least 56 units long, cut with an ellipsis;
  - the ring's title runs along its band at 12 o'clock;
  - a dimmed sector (search or chip) shows its outline only;
  - the focused one carries the focus ring role.
- **Interaction:**
  - the `<svg role="group">` is labelled "Scope rings" and is one tab stop; its sectors are `role="link"` with a roving `tabindex`, named "<title>, <kind>, <state words>, ring <title>";
  - Left and Right move along the ring order; Up and Down go to the nearest-angle sector of the outer or inner ring; Home and End go to the ring's first and last;
  - Enter, or a click (a press of at most 5 px, `CLICK_DISTANCE`), opens the document view (`doc=<name>`);
  - hovering or focusing shows a card with the title, kind, state (label and icon), raw status (`lang="en"`) and `path:line`;
  - the wheel is never taken, and nothing pans or zooms: the drawing fits its box.
- **Controls:**
  - a search field, matched as text rules say;
  - state chips, one per state present, in `ROLLUP_WORDS`' order, each with icon, label and count, `aria-pressed`;
  - Picture | List.
- **The legend:** each state's swatch, icon and label, then the rings innermost first, with each one's sector count.
- **The List:** a section per ring (`h3` the title, then "root <REF> · kinds <a, b>" `lang="en"`), with rows in served order: a `DocLink` (title, else name), the kind raw, a state `Badge`, the status raw. Search and chips apply.
- **States:**
  - a skeleton while reading;
  - an `ErrorPanel` with Retry on a failure;
  - no ring and no note: an `EmptyState` "No rings configured", the next step "Add `[[map.ring]]` tables to `specengine.toml`, each with a `title` and a `root`, `kinds` or both", and the example in mono;
  - notes: those notes verbatim (`lang="en"`), then the same next step;
  - an empty ring: drawn as an empty band with its title and "0".
- **Strings:** about 24 leaves under `rings.*` in `en.ts` and `ru.ts`, the Russian a draft. **CSS:** `.rings-*` in `app.css`, from tokens only; no new token.

## Rules and edge cases

- WHEN a node passes two rings' filters, it SHALL be drawn once, in the inner one.
- WHEN a root has several holders, the ring SHALL take each holder's subtree, as `spec tree ROOT` does.
- WHEN a ring holds only `kinds`, it SHALL see `spec tree`'s roots (`[paths] spec`), as the catalog does. A record outside them needs a `root`.
- WHEN the config has a problem, the read SHALL exit 0 with the note and no ring, and SHALL change no other read.
- An older `spec` refuses a `specengine.toml` holding `[map]`, as `card-flows.md` "Known limits" says of `return`. That includes the gate (no verdict) and an older hub (503). Install `spec` from one commit before adding a ring.
- The UI SHALL NOT compute membership or a state. It SHALL draw sectors in served order and key no fill on a state word outside `labels.ts`.
- The same answer SHALL give the same drawing: there is no random and no clock.

## Acceptance criteria

A browser look is marked **[look]** (the orchestrator's, in Chrome). Every other criterion is a test that the named mutation turns red. Code comments cite `docs/features/scope-rings.md` "Acceptance criteria" with an ID, never "Data" (`doc_pointers`). "The copy" is a scratch copy of `fixtures/spec-a` with Data's config.

- [ ] SR1 — **The config** (core `tests/map_toml.rs`). Data's config reads two rings. Each problem in Data's list gives its note at its line, and no ring: neither key; a 13th ring; `[map] x = 1`; a bad title, root or kinds; a wrong type. Without `[map]`, the read gives no ring and no note. `spec check` and every command load a file with `[map]`. Mutations, each red: a bad ring dropped alone, the rest served; `map` missing from `RawFile` (every command exits 2).
- [ ] SR2 — **The rings.** On the copy, `spec rings` prints Data's text, and `--json` gives ring 1 the two mechanics and ring 2 the four rules, in tree order. With a third ring `{title = "All", root = "DOM-GAME"}`, that ring holds every node of the tree but the six, in `spec tree DOM-GAME`'s order. With the third ring moved first, it takes all, and the other two are empty. Mutations, each red: a node in two rings; sectors by path, not tree order.
- [ ] SR3 — **States.** With a card made on MEC-STAMINA (`spec card new … --target MEC-STAMINA`), its sector reads `in work`. Accepted, it reads `confirmed`. Cancelled, it reads `no card`. Mutation, red: a cancelled card counted.
- [ ] SR4 — **Notes, exits, route.** A root of `DOM-NONE` leaves its ring empty, with the note, exit 0. A bad table gives the note, exit 0, `rings: []`. `GET …/rings` is byte-equal to the CLI's JSON, `?x=1` is a 400, and the fixture holds the three key sets. The copy at two roots and two `HOME`s gives byte-identical answers. Mutation, red: an unresolved root exiting 1.
- [ ] SR5 — **No domain.** http `no_domain.rs` and eval `workspace_names.rs` take each `[[map.ring]]` title and kind of the repository's configs and presets as words, and find none in app code. `map_toml.rs` and `rings.rs` hold no kind or state literal. Mutation, red: a ring title quoted in `ui/src`.
- [ ] SR6 — **Geometry** (`rings/geometry.test.ts`). Three rings fit 460. Sector k of n starts at 2πk/n from 12 o'clock. One sector is a full band. A 56-unit arc keeps its label and 55 drops it. Mutations, each red: angles by title length; a label on a 40-unit arc.
- [ ] SR7 — **The view** (`RingsView.test.tsx`, `mocks/rings.smoke.test.tsx`):
  - sectors appear in served order, each named in words;
  - Tab enters once; Right moves along the ring, Down goes to the nearest-angle sector of the next ring, and Home and End go to the ring's ends;
  - Enter, or a 4 px press, sets `doc=<name>`, and a 6 px press does nothing;
  - search dims the non-matches;
  - a state chip hides its state's fill;
  - the List shows the same rows by ring;
  - each state is shown by icon and label, never by colour alone;
  - with no ring, the empty state shows its example; with notes, the notes show;
  - a wheel event is not `preventDefault`ed.

  Mutations, each red: every sector a tab stop; the fill keyed on a state word outside `labels.ts`.
- [ ] SR8 — **Lazy and size.** `lazy.test.ts` finds `RingsView` lazy. After `pnpm build` it is not in first load. First load grows at most 1.0 kB, within 845 kB, and the figure is recorded. No dependency is added (`ui_policy.rs`). Mutation, red: a static import.
- [ ] SR9 — **[look]** In Chrome, both themes and both languages, at 1440x900 and 1100x800, on the mock and on the copy through `spec serve`: three rings, a long title, search, chips, the List, the empty and bad-table states. Keyboard only, the rings are reached and walked. VoiceOver reads a sector's name.
- [ ] SR10 — **Gates.** These pass:
  - `cargo nextest run` for `specengine-core`, `-cli` and `-http`;
  - eval `doc_pointers`, `ui_policy`, `anonymity` and `workspace_names`;
  - UI lint, `tsc`, `pnpm test`, `pnpm build`, `policy.test.ts` and `words.test.ts`;
  - `export index`, then `check`.

## Out of scope

- A front-matter field for scope, and a ring per section.
- Weighted angles (equal angles only); zoom and pan.
- Rings in the Map or the local graph.
- A check finding for a bad `[map]`.
- An MCP tool.
- Rings in presets: a project writes its own.
- Coverage findings.
- Editing from the rings.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | core `src/map_toml.rs` (new), `src/rings.rs` (new), `src/project_toml.rs` (`RawFile.map`), `src/lib.rs` (two `pub mod`); CLI `src/rings.rs` (new), `src/lib.rs` (`Outcome::Rings`, the module), `src/main.rs` (`Command::Rings`); http `src/app.rs` (the route, the 404 list) |
| test-engineer, Rust | core `tests/{map_toml,rings}.rs`; CLI `tests/rings.rs`; http `tests/rings.rs`, `daemon_keys.rs` with `fixtures/daemon-keys.json` and `daemonKeys.test.ts`, `hub.rs` `GET_ROUTES`, `no_domain.rs`; eval `workspace_names.rs` (ring words) |
| ui-developer | `src/rings/*` (new); `graph/GraphHome.tsx` (create or extend, D6), `graph/GraphView.tsx` only if it creates the home; `api/{provisional,client,http,queries}.ts`; `mocks/{rings.ts,MockClient.ts}`; `test/builders.ts`; `styles/app.css`; `i18n/{en,ru}.ts` |
| test-engineer, UI | `rings/*.test.ts(x)`, the smoke test, `app/lazy.test.ts`; SR6-SR8's mutations and figure |

### Overlaps

- **`delivery-timeline`** (in flight) shares CLI `src/{lib,main}.rs`, http `src/app.rs`, the keys fixture and its tests, and the UI seam files, `MockClient.ts`, `builders.ts`, `app.css`, `en.ts`, `ru.ts` and `lazy.test.ts`. All of these wait for its commit.
- **`release-hardening`** (in flight): R2 edits CLI `main.rs`, and lane U edits `app.css`. Core `project_toml.rs` is no file of its lanes.
- **`corpus-map`** shares core `src/lib.rs`, CLI `src/{lib,main}.rs`, http `src/app.rs`, `graph/{GraphView,GraphHome}.tsx`, the UI seam files, the catalogs, `app.css`, `lazy.test.ts` and the keys fixture.
- **`usage-orphans`** shares core `src/lib.rs` and the UI seam files.

### Work order

1. **Now, beside the others:** core `map_toml.rs`, `rings.rs` and `project_toml.rs`; CLI `rings.rs` unregistered; the UI's `src/rings/*` with a built answer. None of these is an in-flight file.
2. **After `delivery-timeline` commits, and R2 for `main.rs`:** the registration (`lib.rs`, `main.rs`, `app.rs`), one lane at a time with `corpus-map`; then the UI seam files and `GraphHome.tsx` (D6).
3. Tests overlap; the keys fixture written once per lane, one lane at a time; reviews, the look (SR9) and ship. At most three iterations a lane.

### Canon at ship

| File | Edit |
|---|---|
| `docs/canon/spec-rings.md` (new, Tier 2) | the config and its notes, membership, the read and route, the UI, known limits (older binaries, kinds-only rings see `[paths] spec`), tests |
| `docs/canon/spec-map.md` | the home's Rings view, a pointer |
| `workspaces.md` "Config" or `directions.md` "Config" | `[[map.ring]]` reuses their readers, a pointer. Net ≤ +150 B |
| core, CLI and http `README.md` | by replacing text. Net ≤ 0 |
| `ui-languages.md` "Size" | the figure, and `RingsView` among the lazy chunks. Net |
| `CLAUDE.md` "State", 08 Phase 7, RM-0113 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers

- **The owner's:** scope rings, from the earlier prototype.
- **The orchestrator's:** layers from project config, plain SVG, no dependency, a legend and a List twin.
- **The spec-writer's:**
  - D1, rings by a root, kinds or both, not a front-matter field: sections hold no front-matter, and a key changes the model;
  - D2, membership as a tree read, innermost first;
  - D3, the fill as the trace's rollup, with no new state;
  - D4, no check code;
  - no ADR, by the convention's test (no rule changes);
  - roving focus over every sector, rather than the List alone, since SVG sectors are DOM.
