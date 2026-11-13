---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, fixtures, ui]
ref: RM-0113; the owner's choice of 2026-11-13 (scope rings, an extra beside the writer map); an earlier prototype's rings tab (rings as scope layers, sectors at equal angles, the fill a state); the analyst's analysis of 2026-11-13
adrs: []
---

# scope-rings: the project's nodes on concentric rings, the radius their scope

## Why

A writer asks what a mechanic touches: the whole world, one thing, or one actor. A tree shows where a document is filed and a map what links to what; neither shows scope at a glance. An earlier prototype drew scope as concentric rings, the widest inside, each mechanic a sector at an equal angle filled by how far its work is; the owner chose to bring it in. SpecEngine names no domain (ADR-0008), so each ring is the project's `[[map.ring]]` row, choosing nodes by a root, kinds or both, as a `[[module]]` does: no front-matter field (sections have none), no ADR, nothing stored, no dependency, no MCP tool, no plugin version. `spec rings`, its route, and the Graph home's Rings view in plain SVG, the fill the trace's rollup. Canon: `docs/canon/spec-rings.md`.

## Rules and edge cases

Canon: `docs/canon/spec-rings.md` "Config", "Membership", "The UI", "Known limits". In short: a node sits in the first ring listing it; a root's several holders each give their subtree; a kinds-only ring sees `spec tree`'s roots; a bad table serves no ring, one note, exit 0, and changes no other read; an older `spec` or hub refuses `[map]`, the gate does not; the UI computes no membership or state; one answer gives one drawing.

## Acceptance criteria

**[look]** marks a browser look; every other criterion is a test its named mutation turns red. Code comments cite this heading with an ID (`doc_pointers`). "The copy" is a scratch copy of `fixtures/spec-a` with the canon's config. Evidence: the tests citing each ID; the Rust review (iteration 1, a REWORK on ADR-0009, iteration 2) and the UI review, ACCEPT; the test-engineer's final run on iteration 3 (with `corpus-map`): 26 named mutations red, clippy clean.

- [x] SR1 — **The config** (core `tests/map_toml.rs`). The canon's config reads two rings. Each problem gives its note at its line and no ring: neither key; a 13th ring; `[map] x = 1`; a bad title, root or kinds; a wrong type and an unknown key, in serde's words (`` unknown field `colour`, expected … ``). No `[map]`: no ring, no note. `spec check` and every command load a file with `[map]`. Mutations: a bad ring dropped alone, the rest served; `map` missing from `RawFile`.
- [x] SR2 — **The rings.** On the copy, `spec rings` prints the canon's lines; `--json` gives ring 1 the two mechanics and ring 2 the four rules, in tree order. A third ring `{title = "All", root = "DOM-GAME"}` holds the tree but the six, in `spec tree DOM-GAME`'s order; moved first, it takes all and the others are empty. Mutations: a node in two rings; sectors by path.
- [x] SR3 — **States.** A card on MEC-STAMINA reads `in work`; accepted, `confirmed`; cancelled, `no card`. Mutation: a cancelled card counted.
- [x] SR4 — **Notes, exits, route.** A root of `DOM-NONE` leaves its ring empty with `` rings: ring 1 `Movement`: `DOM-NONE` resolves to no ID and no alias ``, exit 0; a look-alike, `slug/ID`, a path, a non-REF and `project:` each give the resolver's reason; `@rev` a note, the ring served. A bad table: the note, exit 0, `rings: []`. `GET …/rings` byte-equals the CLI's JSON, `?x=1` is a 400; the fixture holds the three sets (152-154); two roots and two `HOME`s give byte-identical answers. Mutation: an unresolved root exiting 1.
- [x] SR5 — **No domain.** http `no_domain.rs` and eval `workspace_names.rs` take each ring title and kind of the repository's configs (`fixtures/plant-diary` gains two rings) and presets as words, none in app code; `map_toml.rs`, `rings.rs` hold no kind or state literal. Mutation: a ring title quoted in `ui/src`.
- [x] SR6 — **Geometry** (`geometry.test.ts`). Three rings fit 460; sector k of n starts at 2πk/n from 12 o'clock; one sector is a full band; a 56-unit arc keeps its label, 55 drops it. Mutations: angles by title length; a label on a 40-unit arc.
- [x] SR7 — **The view** (`RingsView.test.tsx`, `mocks/rings.smoke.test.tsx`): sectors in served order, named in words; Tab enters once, Right, Down, Home, End walk; Enter, a bare click or a 4 px press sets `doc=<name>`, a 6 px press nothing; search dims, a chip hides its state's fill; the List shows the rows by ring; each state by icon and label; the empty and notes states; the wheel not taken. Mutations: every sector a tab stop; a fill keyed on a state word outside `labels.ts`.
- [x] SR8 — **Lazy and size.** `lazy.test.ts` finds `RingsView` lazy, not in first load; `RingsView` 12.65 kB; no dependency (`ui_policy.rs`). First load 839.87 kB with `corpus-map`, within 845 kB. **Missed:** this 1.0 kB growth bar (CM11). Mutation: a static import.
- [x] SR9 — **[look]** Chrome, light and dark, Russian, on the mock (13, 5, 1 and 0 sectors) and a scratch copy of a real game corpus with three test rings (91 sectors: 14, 67, 10, all "no card"): every sector drawn, the thin ones' labels hidden and named in the List. Not done: VoiceOver; the DOM's accessible names were checked instead. The empty and bad-table states: the smoke test over the mock.
- [x] SR10 — **Gates.** As `corpus-map` CM15 (one run): `ui_policy`, `anonymity`, `doc_pointers`, `workspace_names`, UI lint, `tsc`, `pnpm test`, `pnpm build`, `export index`, `check`.

## Out of scope

A front-matter scope field; weighted angles, zoom, pan; rings in the Map or local graph; a check finding for a bad `[map]`; an MCP tool; rings in presets; coverage findings; editing from the rings.

## Implementation

| Module | What it does |
|---|---|
| core `map_toml.rs`, `project_toml.rs`, `workspace_toml.rs`, `lib.rs` | `MapConfig`, `RingConfig`, `MapNote`, `MAP_RINGS_MAX`, `map_from_toml`; `RawFile.map`; the readers `pub(crate)` |
| core `rings.rs` | `RingsDocument`, `Ring`, `RingSector`, `ring_members`, `ring_note`, `unresolved_root` |
| CLI `rings.rs`, `lib.rs`, `main.rs`, `tree.rs` | `spec rings`, `rings_with_view`; `Command::Rings`; `lines_of` `pub(crate)` |
| http `app.rs`, `answer.rs` | `GET …/rings`, walking; the 404 list |
| UI `src/rings/*` | geometry, view helpers, `RingsPicture`, `RingsList`, `RingsView` |
| UI `graph/GraphHome.tsx`, `app/{Shell,location,shortcuts}`, `ui/useFocusLater.ts`, `api/`, `mocks/rings.ts`, `i18n/`, `styles/app.css` | the home's view, SVG openers, the `?` keys, seam, mock, 17 strings, CSS |
| tests | core `map_toml.rs`, `rings.rs`; CLI, http `rings.rs`; `no_domain.rs`, `daemon_keys.rs`, `workspace_names.rs`; UI `rings/*.test.*`, smoke |

**Departures**, accepted: with no ring, no index or queue is opened; the note type is `MapNote`; `workspace_toml.rs` and `tree.rs` helpers made `pub(crate)`; the opener type widened to SVG elements.

**Rulings (orchestrator):** a root's note is the resolver's reason (iteration 2, after the ADR-0009 REWORK); the gate is not affected by `[map]` (`GateConfig` reads its own table), older `spec` and hubs are; the TOML reader's unknown-key wording is serde's; UI fixes: `labelRoom` capped at the band's straight length, a bare click opens, the card an overlay, sectors keyed by name and occurrence, `.rings-sector:focus-visible` transparent outline with the forced-colours one kept.

**Known limits:** `spec-rings.md` "Known limits".
