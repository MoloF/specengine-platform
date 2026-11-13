---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, ui]
ref: RM-0112; the owner's goal of 2026-11-13 (see what is used where; he likes Obsidian's dependency map); his Q3 answer, the whole corpus at once; the graph filters he chose that day; the analyst's analysis of 2026-11-13, slice 2; an earlier prototype's graph filters
adrs: [ADR-0080]
---

# corpus-map: the whole corpus as one map, a node's links both ways, and the graph's filters

## Why

The owner wants to see what is used where, as Obsidian's global graph shows it: every document at once, the most used ones large, orphans floating apart. The Graph needed a REF, walked one way and capped at 200 boxes. This slice adds `spec map`, its route, and the Graph section's home with a whole-corpus Map, laid out by `d3-force` on a 2D canvas in a lazy chunk (ADR-0080), sized by `usage-orphans`' counts, coloured per kind with a label, orphans marked. It adds the filters he chose (only broken, only orphans, ghost nodes, kind chips), `spec graph --both` and `--kind`, and the local graph's Both ways. No MCP tool, no plugin version. Canon: `docs/canon/spec-map.md`.

## Rules and edge cases

Canon: `docs/canon/spec-map.md` "The rule", "spec map", "Graph flags", "The UI", "Known limits". In short: a twin ID is drawn twice and warned once, its links on the first admitted holder; folded sections merge their edges and drop self-edges; Orphans only hides sections; filters never lay out again; one answer gives one picture in one engine; the UI counts, resolves and writes nothing, keys no colour on a kind's text and imports `d3-force` only in `ui/src/map/`.

## Acceptance criteria

**[look]** marks a browser look, **[measure]** a timed or counted run; every other criterion is a test its named mutation turns red. Code comments cite this heading with an ID (`doc_pointers`). "The copy" is a scratch copy of `fixtures/spec-a`. Evidence: the tests citing each ID; the Rust review ACCEPT (iteration 1, then iterations 2 and 3), the UI review's REWORK then fixes; the test-engineer's final run on iteration 3: 26 named mutations red, clippy clean.

- [x] CM1 — **The map read.** On the copy, `spec map --json` gives `documents` 12, `sections` 7, `edges` 35 (25 `mentions`; the oracle: the union over every live node X of `spec graph X --depth 1 --type T` per type and `mentions`, resolved, merged per (src, dst, type)), `parents` 3, `ghosts` 0, `orphans` 1; nodes by (path, ord), RULE-STAM-REGEN `of` MEC-STAMINA's index, each node's uses `usage-orphans`' table. Names are canonical: `AC-07` is `stamina-tuning/AC-07`; a feature-scoped ID in two features is two names, no warning; a twin in one feature or outside features warns; an end with several holders lands on the first admitted. `--archive`: `documents` 13, DEC-0007 `archived` with its `supersedes` edge, the uses unchanged. Mutations: an edge per place; self-edges kept; Tier 3 nodes without `--archive`.
- [x] CM2 — **Ghosts.** The copy's `sprint.md` gains `links: {depends_on: [MEC-GONE]}`, `stamina.md` a wiki mention of it: one ghost, `written` MEC-GONE, `sources` [MEC-SPRINT's, MEC-STAMINA's], `places` 2; `shared:DEC-0023@1` gives none. Mutations: skipped links as ghosts; a ghost per place.
- [x] CM3 — **Text, route, one answer.** The text holds the canon's lines; at a forced small cap the tail shows, the JSON whole; stderr `note:` then `warning:` lines. `GET …/map`, `?archive=true` byte-equal the CLI; `?x=1`, `archive=yes` are 400s naming it. Two roots and two `HOME`s: byte-identical text and JSON. `fixtures/daemon-keys.json` holds the six sets (155-160). Mutations: the JSON cut with the text; `notes` in input order.
- [x] CM4 — **Both ways.** `spec graph MEC-STAMINA --both --depth 1 --json` reaches MEC-SPRINT, DEC-0023 (`canon` in), A-101, R-12 (`derived_from` out) and TERM-exhausted (`uses_term` out) at distance 1; `types` `out` then `in`; `--both --impact` exits 2 with the line, a 400 on the route; `?ref=MEC-STAMINA&both=true` byte-equal. Mutation: the in direction not followed.
- [x] CM5 — **A direction start.** `spec graph --kind mechanic --depth 0`: MEC-SPRINT, MEC-STAMINA at 0, `ref` `null`, `kinds` `["mechanic"]`; `--kind nothing`: `nodes 0, edges 0`, the note, exit 0; a REF with `--kind` exits 2, a 400 on the route; `?kinds=mechanic` byte-equal. Mutation: kinds matched by prefix.
- [x] CM6 — **Layout, one picture** (`layout.test.ts`). Twice: identical positions, no `NaN`, exactly `MAP_TICKS` ticks; 1-tick slices equal one run; Sections off lays out no section. Mutations: `Math.random`; ticks counted by time.
- [x] CM7 — **Drawing and pointer** (`MapCanvas.test.tsx`, `MapStage.test.tsx`, `draw.test.ts`, a recording pen): radii follow `used_by`; the 8 most held kinds take slots 1-8 by count, ties by code point, a ninth and no kind `--map-kind-other`; orphans' dashed ring; a hover lights the node and its neighbours, the rest at 0.15; a ghost's card; a 4 px press sets `doc=<name>`, 6 px pans; a wheel event stays not `defaultPrevented`; `+`, `-`, `0`, arrows act only with the canvas focused. Mutations: the wheel zooming; slots by kind name, not count; `CLICK_DISTANCE` 0.
- [x] CM8 — **Filters and the List** (`filters.test.ts`, `MapView.test.tsx`): a kind chip hides its nodes and edges; Orphans only keeps the 2 orphans, Broken only the 3 ghosts and their sources; Unresolved targets, Sections (4, unfolded), Parents draw theirs; Mentions starts off, on draws the weak edges; Direction presses only its chips; only Sections and Include archive move a position; the List shows the kept nodes, counts and marks, then "Broken links"; notes past 3 fold; ArrowDown and Enter light and announce the second match, Enter opens it. Mutations: a filter laying out again; the List ignoring the chips.
- [x] CM9 — **The cap.** 5 001 nodes draw 5 000, the least used left out (last in served order on a tie); the count line names it; the List holds 5 001 rows. Mutation: the first 5 000 in served order.
- [x] CM10 — **Both ways on the screen** (`GraphView.test.tsx`, `settings.test.ts`): `both=true` sent, chips per (type, direction), the other modes keep their bases, a reload keeps the mode. Mutation: Both ways sending `impact`.
- [x] CM11 — **The dependency and the bundle.** `d3-force` 3.0.0 and `@types/d3-force` 3.0.10, exact, frozen; `ui_policy.rs` takes 21 packages and finds `d3-force` imported in `ui/src/map/` alone; `lazy.test.ts` finds `MapView` lazy. First load 839.87 kB, within 845 kB; `MapView` 45.18 kB raw, 15.07 kB gzip. **Missed:** the growth bar: with `scope-rings`, first load grew 4.16 kB over `usage-orphans`' 835.71 kB, past this 2.5 kB and SR8's 1.0 kB. Mutations: a static import; `d3-force` in `graph/`; a `^` pin; a 22nd package.
- [x] CM12 — **[measure]** Read (eval `map_scale.rs`, release, warm, median of 5): 1k 17 ms, 10k 105 ms (bar 1 s), 100k 1 253 ms. Hover 12-17 ms (bar 33), no long task. A scratch copy of a real game corpus (2 422 nodes, 10 546 edges): the request 0.55 s, layout and draw of 1 405 documents about 0.6 s, so 5 000 extrapolates to about 2 s, taken as met; the `large` mock's 2.6 s at idle includes generating it in the browser, its split unknown. No fallback applied.
- [x] CM13 — **Tokens and words.** `tokens.test.ts` finds the roles and contrasts in both themes; `policy.test.ts`, `words.test.ts`, `scale.test.ts` pass; `map.*` complete in `ru.ts`. Mutation: a kind role at 2.5:1.
- [x] CM14 — **[look]** Chrome, light and dark, Russian, on the mocks and a scratch copy of a real game corpus (not this repository's): the Map at rest, hover, search, legend, List. It found four defects, fixed in iteration 3: 22 holder warnings flooding the page (canonical names: 0 left), long notes pushing the map down (folded), every node grey with no direction (now per kind: 8 kinds by count, then "Other kinds"), a hairball of mentions (off by default). Not done: VoiceOver; the DOM's accessible names were checked instead.
- [x] CM15 — **Gates.** Full workspace: 6 095 run, 6 091 passed; 3 http `ui` failures were races with a mid-run `ui/dist` rebuild and pass alone; the 4th, `ui_policy`'s README table, passes after this ship's README. Eval `doc_pointers`, `anonymity`, `workspace_names`; UI lint, `tsc`, `pnpm test` (6 130), `pnpm build`; `export index`, `check`.

## Out of scope

An MCP tool; WebGL, a worker, saved positions, dragging; clusters, a time slider; arrowheads; the local graph's colours; coverage findings; a generated map; editing from the map.

## Implementation

| Module | What it does |
|---|---|
| core `map.rs`, `lib.rs` | `corpus_map` → `CorpusMap`; `MapNaming`, `node_names` |
| CLI `map.rs`, `cap.rs` | `spec map`: `MapRequest`, `MapRow`, `MapOutcome` (`text_within`, stderr); `lines_fit` |
| CLI `graph.rs`, `main.rs`, `lib.rs`, `tree.rs` | `--both` (`walk_both`), `--kind`, an optional REF, `GraphRequest::refusal()`; `Command::Map`; `parent_warning` `pub(crate)` |
| http `app.rs`, `answer.rs` | `GET …/map`; graph's `both`, `kinds`, its 400; the 404 list |
| UI `src/map/*` | layout, filters, colours, draw, hit, canvas, stage, controls, legend, List, view |
| UI `graph/`, `app/{Shell,location,shortcuts}`, `ui/useFocusLater.ts` | the home, Both ways, keys |
| UI `api/`, `mocks/`, `i18n/`, `styles/`, `package.json`, lockfile, `eslint.config.js` | seam, mock, strings, tokens, the dependency confined |
| tests | core, CLI, http `map.rs`; `graph.rs`; `daemon_keys.rs`; eval `map_scale.rs`, `ui_policy.rs`; UI `map/*.test.*`, smoke |

**Departures**, accepted: a ghost groups by the bare target; a node line's tail is `[ | of D][ | orphan]` after the counts, `[ | archived]` last; a ghost line always prints `places`; the canvas sits in a focusable labelled region; the fit frames all points; cards hold no actions; harbor-sim gains 3 map-only documents.

**Rulings (orchestrator):** iteration 2, the first admitted holder, `warning:` lines, escaped graph stderr; the UI review, a ghost card and "Broken links" (ghosts had no text twin); iteration 3, after the look, canonical names with the shared-name warning, colour per kind (the owner's choice; Direction only filters), mentions off by default, notes folded after 3.

**Known limits:** `spec-map.md` "Known limits".
