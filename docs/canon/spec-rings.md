---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# Scope rings: the project's nodes on concentric rings, the radius their scope

Built by `scope-rings` (RM-0113, 2026-11-13), no ADR: a ring is the project's words in its config (ADR-0008), as `[flow]` and `[roadmap]` are. The Graph home's Rings view (`spec-map.md` "The UI"). Nothing is stored, no dependency is added, and there is no MCP tool: an agent has `get_tree` with `kinds`.

## Config

```toml
[[map.ring]]
title = "Movement"
root  = "DOM-MOVEMENT"
kinds = ["mechanic"]

[[map.ring]]
title = "Rules"
kinds = ["rule"]
```

Core `map_toml.rs`: `MapConfig::from_toml(&str)` (`map_from_toml`) `-> MapConfig {rings: Vec<RingConfig>, notes: Vec<MapNote>}`, pure, never failing; `RingConfig {title, root?, kinds?, line}`; `MapNote {line, message}`, `at(file)`. `ProjectConfig` only name-checks `map`, so a bad table fails no other read, command, check or gate. The readers and messages are `workspace_toml.rs`' (`workspaces.md` "Config"): `title` a `[[module]]`'s (1-64 bytes, no control character), `root` a rooted module's (1-256 bytes, a REF in `spec show`'s forms, resolved by the read), `kinds` a direction's (1-32, `directions.md` "Config"). A ring holds `root`, `kinds` or both; at most 12 (`MAP_RINGS_MAX`); `[map]` holds `ring` alone.

Any problem serves no ring and gives one note, the first by line (the value's line, a missing key's table header's, else 1), and no check code:

```text
<config>:<line>: <problem>; no ring is served
```

`<problem>`: the TOML reader's, as serde words it (`` invalid type: integer `3`, expected a string ``, `` unknown field `colour`, expected one of `title`, `root`, `kinds` ``); `` `[map]` holds only `ring` ``; `` `[[map.ring]]` is required to hold `title` ``; `` `[[map.ring]]` `<title>` holds neither `root` nor `kinds`: a ring holds one or both ``; `` at most 12 `[[map.ring]]` tables ``; the title, root and kinds problems as `workspaces.md` words them.

## Membership

`spec rings [--json]`: core `rings.rs` (`RingsDocument`, `Ring`, `RingSector`, `ring_members`, `ring_note`, `unresolved_root`), CLI `rings.rs` (`rings`, `rings_with_view`). The config is read once; with no ring nothing else is read, no index or queue opened. Else one index update and read, then per ring in order the lines `spec tree [<root>] [--kind <k>]…` lists, live files only, less the nodes an inner ring holds: innermost first, a node in the first ring listing it, once. A root with several holders gives each holder's subtree and one `warning:`; no root, `spec tree`'s roots (`[paths] spec`), as the catalog. The state is the trace's rollup (`spec-trace.md` "Marks and rollup"), built once for every sector (`node_work`, as `spec catalog`): `no card`, `in work`, `confirmed`, `suspect`, `departed`.

A root that names no node, or is refused as a REF, leaves its ring empty with `` rings: ring <i> `<title>`: <reason> ``, `spec tree ROOT`'s reason: a look-alike's Latin fix (`` …; write `DOM-MOVEMENT` ``), `` `DOM-NONE` resolves to no ID and no alias ``, a `slug/ID`'s, a path's, a non-REF's, `project:`'s; one located at no node, `` `<REF>` resolves to no node ``. An `@rev` on a root is a note with the same prefix; the ring is served.

JSON `{notes, rings}`; a ring `{title, root, kinds, sectors}`; a sector `{name, id, kind, title, path, line, status, state}`, `name` the ID else the path, `status` a document's raw `status:` (`null` for a section). Text, each value through `one_line`:

```text
rings <n> | sectors <m>
ring <i> | <title>[ | root <REF>][ | kinds <a, b>] | sectors <k>
  <name> | <kind or -> | <state> | <title or -> | <path>:<line>[ | status <s>]
[truncated: <k> of <n> sectors not shown]
```

On a copy of `fixtures/spec-a` with the config above: ring 1 MEC-SPRINT, MEC-STAMINA; ring 2 the four rules, in tree order, all `no card`. No `[map]`: `{"notes": [], "rings": []}`, `rings 0 | sectors 0`. The cut at `OUTPUT_CAP_CHARS` is flat, at a line, rings whole until it; the JSON drops what the text drops; the daemon's (`View::Browser`) is whole. Notes (the config's as `<config>:<line>:`, a root's, the index's, the tree's, the trace's) go to stderr as `note:` lines and to `notes`, sorted, once; the tree's warnings to stderr alone. Exit 0 whatever is listed; 2: no project, a config `ProjectConfig` refuses, an index or a queue that cannot be read; never 1.

**Route.** `GET /api/projects/:p/rings`: `spec rings --json` less its final LF, uncut, in the project's turn, walking as `catalog` does (trace data). Any query name is a 400; exit 2 a 503. Key sets `Rings` (2), `Ring` (4), `RingSector` (8).

## The UI

`ui/src/rings/`, the lazy `RingsView.tsx` (12.65 kB), plain SVG.
- **Seam.** `RingsDocument`, `Ring`, `RingSector`; `getRings(project, signal)`; `useRings`, key `["rings", p]`, in `READS_OF_THE_SPEC`, read again on `card.*`. Mock `mocks/rings.ts`: harbor-sim four rings of 13, 5, 1 and 0 sectors, every state, a long title, a bad table in `notes`; `empty` none.
- **Geometry** (`geometry.ts`, pure): `viewBox` 1000 × 1000; ring i's band from `80 + i·W` to `80 + (i+1)·W`, W fitting 460; sector k of n from 2πk/n clockwise from 12 o'clock; one sector a full band; a gap of 1. A label shows when the mid-radius arc is at least 56 units, cut with an ellipsis to `labelRoom`, at most the band's straight length; the ring's title runs along its band at 12 o'clock.
- **Drawing.** A sector is a `<path>` filled with its state's tone (`rollupLook`, no new colour), stroked `--surface`; dimmed (search, chip): outline only; focused: the focus ring role (`.rings-sector:focus-visible` hides the outline, the forced-colours one stays).
- **Keyboard and pointer.** `<svg role="group">` "Scope rings", one tab stop, roving `tabindex` over `role="link"` sectors named "<title>, <kind>, <state>, ring <title>", keyed by name and occurrence. Left, Right along the ring; Up, Down the nearest-angle sector outward or inward; Home, End the ring's ends; `?` lists them. Enter or a click opens `doc=<name>`; only a recorded press moving past `CLICK_DISTANCE` opens nothing. Hover or focus shows an overlay card: title, kind, state (label, icon), raw status `lang="en"`, `path:line`. No wheel, pan or zoom: the drawing fits its box.
- **Controls.** Search (`ui/README.md` "Screen rules", Text); state chips in `ROLLUP_WORDS`' order, icon, label, count, `aria-pressed`; Picture | List. Legend: the states, then the rings innermost first with their counts.
- **List.** A section per ring: `h3` the title, "root <REF> · kinds <a, b>" `lang="en"`; rows `DocLink`, raw kind, state `Badge`, raw status `lang="en"`; search and chips apply.
- **States.** Skeleton; `ErrorPanel` with Retry; no ring and no note: `EmptyState` "No rings configured", the step "Add `[[map.ring]]` tables to `specengine.toml`, each with a `title` and a `root`, `kinds` or both" and the example; notes verbatim `lang="en"`, then the step; an empty ring a band with its title and 0.
- `rings.*` strings, `.rings-*` CSS from tokens only. The UI computes no membership or state and keys no fill on a state word outside `labels.ts`; one answer gives one drawing.

## Known limits

- An older `spec` refuses a `specengine.toml` holding `[map]` (a closed top level: exit 2), and so does an older hub (503), as `card-flows.md` "Known limits" says of `return`. The gate is not affected: `GateConfig` reads its own table. Install `spec` from one commit before adding a ring.
- A kinds-only ring sees `spec tree`'s roots (`[paths] spec`); a record outside them needs a `root`.
- Thin sectors hide their labels (67 in one ring of a scratch copy of a real game corpus); the List names them.
- Equal angles only. The Russian strings are a draft.

## Tests

Core `tests/map_toml.rs`, `tests/rings.rs`; CLI `tests/rings.rs`; http `tests/rings.rs`, `no_domain.rs`, `daemon_keys.rs` (sets 152-154), `hub.rs`; eval `workspace_names.rs` (ring words: `fixtures/plant-diary` holds two rings); UI `src/rings/*.test.ts(x)`, `mocks/rings.smoke.test.tsx`, `mocks/rings.test.ts`. Criteria: `docs/features/scope-rings.md`.
