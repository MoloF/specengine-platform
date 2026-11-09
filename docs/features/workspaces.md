---
class: spec
status: shipped
shipped: 2026-11-09
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, ui, scripts, fixtures]
ref: RM-0081; v4 step 1a of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"), his decisions 1 and 3; Q5 and Q6 are the orchestrator's working answers, awaiting the owner's review
adrs: [ADR-0073]
---

# workspaces: a workspace per trade, modules, a gamedev preset, and Tasks off the nav (RM-0081)

## Why

Every person saw the engine's nine sections, whatever their trade. On 2026-11-08 the owner asked for scopes per trade, since a writer has no use for Health, and for modules, so that a person sees one part of the game. His decisions: (1) four trades to start, writer, developer, artist or level designer, and lead; (3) Tasks leaves the nav, while the `T-` model and the gate stay. Every later v4 slice reads the workspace: the studio (2b), the board (3), the dashboards (4).

Decided (ADR-0073; Q5 and Q6 the orchestrator's working answers):
- **D1** A lens: sections, a landing, card flows, a default module; it hides no route, entitles no one and stops nothing. **D2** Two tables read apart by a reader that never fails. **D3** The implicit workspace for a project that configures nothing. **D4** A module is one root (Q5 narrowed): a kind selector would turn the route into a corpus read.
- **D5** Served by the daemon with a CLI twin, no turn, no MCP tool. **D6** Named "Workspace", not "View as", which reads as acting as someone else. **D7** Tasks off the nav by data alone. **D8** The choice per viewer and project in `localStorage`; a hub login's default is RM-0089's (Q6).
- **D9** `#/` and a switch open the landing, no route redirected. **D10** The module bar on Tree and Flow only, never made up. **D11** The preset in `scripts/presets/`, outside every crate and the pinned plugin. **D12** Every chrome string in `WORKSPACE_TEXT`, for 1b (RM-0082).

## Description and interactions

How it works now: `docs/canon/workspaces.md`. `[card]`'s flow names judge `flows`; `[team]` is never read; the Tree and Flow reads only take the `root` they already took; the palette keeps all nine sections; a hub mounts the route for every login.

## Data

`docs/canon/workspaces.md` "Config" (the two tables, the limits, the judging, the notes' texts) and "Read"; the example tables: `fixtures/card-press/specengine.toml`, its `expected.json` `workspaces`.

### The daemon

`docs/canon/workspaces.md` "Read": `GET /api/projects/:p/workspaces`, `spec workspace --json` byte for byte, no turn; daemon key sets 111-113.

### The UI

`docs/canon/workspaces.md` "Screen": the seam `getWorkspaces`, `provisional.ts`'s `WorkspaceDocument`, `Workspace`, `WorkspaceModule`, `useWorkspaces`, the keys, the nav, the switcher, the landing, the notes, the module bar, the Board, `WORKSPACE_TEXT`. Mocks: harbor-sim serves `galley` and `foundry`, modules `berths` and `waters`; ledger-api the implicit document; `?scenario=config-notes` two notes; `error` a failed read.

### The gamedev preset

`scripts/presets/gamedev.toml`: `docs/canon/workspaces.md` "Preset".

## Rules and edge cases

**W1** a malformed table serves its default with one note, every command exiting as without it; **W2** a wrong value in a good table is left out with a note; **W3** the route reads neither `[team]` nor the index, takes no turn; **W4** a section off the nav still renders on its route, palette entry and links; **W5** no workspace or module name in code; **W6** the nav pending or failed is a skeleton or the nine, never empty. Edge cases: a module root naming nothing is the read's `reason`, the bar kept; one workspace, no switcher; every section unknown, Overview alone; a later key (RM-0089's default, a selector) is an unknown key, its table falling back; mixed scripts allowed in titles.

## Acceptance criteria

Scratch git copies of the named fixtures; UI criteria on the mock through Testing Library. All 13 Rust-side named mutations (M1, M7-M9, M12, M14, M16-M18, M22, M23, M26, M27) were killed; the UI-side ones were spot-checked by the ui-developer.

- [x] **WS-1 The implicit workspace.** On `spec-a` the JSON and text are the implicit document, the route byte-equal; ledger-api's nav is Overview and the eight, no switcher or bar. M1 `tasks` in `IMPLICIT_SECTIONS`; M2 the nav from `SECTIONS`.
- [x] **WS-2 Tasks is still a route.** On ledger-api `#/ledger-api/tasks/T-0031` renders the task (the spec's T-0107 is harbor-sim's: its "no task" reason is asserted); the palette lists Tasks; the Inbox's link renders it; ui-live-tasks AC-12 unchanged. M3, M4.
- [x] **WS-3 Choosing a workspace.** First visit `galley`; Foundry stores `foundry`, changes the nav, pushes `#/harbor-sim` (one entry); a remount keeps it, a throwing `setItem` holds it until a remount. M5, M6.
- [x] **WS-4 A bad table fails nothing.** card-press variants (a)-(g): each note, `source`, `modules` as listed; `spec workspace` exits 0, `note:` lines equal `notes`, the route byte-equal; `check --json` `counts`, `tree --json`, `/api/projects` byte-equal to the copy without the tables. M7-M9.
- [x] **WS-5 A lens hides no route.** `#/harbor-sim/health` in `galley` renders Health, no redirect, entry or `aria-current`; the palette lists the nine. M10, M11.
- [x] **WS-6 No name is code.** (a) `no_domain.rs`, (b) `workspace_names.rs`, (c) `policy.test.ts`, each red on a planted name. M12-M15.
- [x] **WS-7 The preset.** `ProjectConfig`, `WorkspaceConfig` (four workspaces, no note), `CardConfig` (five flows) accept it; studio-lamp holds its bytes between markers and equals `expected.json`; `AREAS` holds `scripts/presets`. M16, M17.
- [x] **WS-8 Every command loads the tables.** On card-press `index`, `check`, `tree`, `flow`, `card list`, `team`, `workspace`, MCP `initialize`, `tools/list` and a daemon start exit 0; `[[workspaces]]` exits 2 with the line (`spec team` behind its `spec: ` prefix; MCP a tool error result carrying it, `initialize` and `tools/list` reading no config). M18.
- [x] **WS-9 Modules root Tree and Flow.** `galley` reads `root: "DOM-BERTHS"` on both; All modules unrooted and stored `""`; `waters` DOM-WATER, kept on remount; Foundry clears it; the bar on Tree and Flow alone, none on ledger-api; other Flow reads unchanged. M19-M21.
- [x] **WS-10 A hub.** A bad `[team]` leaves the route 200 and byte-equal to a local daemon's; `GET_ROUTES` pins 28. M22, M23.
- [x] **WS-11 The landing.** `#/` replaced by `#/harbor-sim/flow`, `#/harbor-sim` on failure; a typed `#/harbor-sim` the Overview. M24, M25.
- [x] **WS-12 The route.** `?x=1` 400; HEAD, POST 405 `Allow: GET`; a refused slug 503; 200 with the turn held; the three key sets. M26, M27.
- [x] **WS-13 Notes and failures.** `config-notes` lists both; `error` the nine, the message and Retry; pending `aria-busy`. M28, M29.
- [x] **WS-14 The Board's flows.** `galley`: no tab, "Showing 1 of 2 card flows.", Show all flows toggles, Foundry resets; `null` every tab, no line. M30.
- [x] **WS-15 One place for the strings.** `text.ts` mocked to sentinels renders only sentinels and data; `sections.ts` keeps the labels. M31.
- [x] **F Form.** Clippy `-D warnings`, fmt, no dependency added; nextest 5240/5240; UI vitest 3598, lint, typecheck, build; `spec check` 0 errors, 0 warnings.

## Out of scope

Dashboards (RM-0086), static columns, swimlanes, hand-offs (RM-0085); a hub login's default (RM-0089); module filters on Board and Roadmap, a module in the hash, a kind selector; an MCP tool, a `spec check` finding; `spec init --pack` (RM-0061); translation (RM-0082); Tasks inside cards (step 3).

## Implementation

Review: ACCEPT; the nit fixed (the unknown-route 404 names `workspaces` after `events`). Where the spec was silent, now canon:
- **Reader** ("Config"): an unreadable `[card]` is noted only when a workspace lists flows; `flows = []` is `null` without reading `[card]`; unjudged lists are served as written, repeats too; non-TOML gives the implicit workspace and one note, at the parser's line or 1; a wrong-typed `name` or `root` reports the type, not "is required"; a module without `root` is noted at its header; the limit, length, control and space wordings.
- **CLI** ("Read"): notes and lines made one line, controls escaped; `sections (none)`; flows joined with ", ".
- **UI** ("Screen"): `WORKSPACE_TEXT` gained `notesButton(n)`, `navPending`, `readFailedKept`; a routed card in a hidden flow turns on Show all flows, once the cards are read; an unserved stored module falls back to the workspace's; Board, Tree and Flow wait for the read; `FlowState.root`, no `memory.remember` before the module is known; the select's hint is its `title` too.
- **Tests**: studio-lamp gained `{#CHP-SHORE-TIDE}` and `{#ISL-LAMP-ROOM}` for the store's own-text oracle; CLI `tests/store_reads.rs` lists it in `AFTER_THE_GOLDEN`, the golden of 2220d05 not re-recorded. Not asserted: no cut, `sections (none)`, escaped controls. Harness: the stale-binary guard fires after a `#[cfg(test)]`-only `src` edit, which relinks no bin; touch a bin source (`crates/specengine-http/README.md` "Tests").
- **Gate fixes.** (1) ui-markdown AC-13 (`ui/src/markdown/parse.test.tsx`) failed 2 runs in 3: the tree read now waits for the workspaces read, and its answer could land while React retried the lazy renderer's render, parsing twice. `Markdown.tsx` `parsed()` keeps the last 32 parses, keyed by text, first line and heading level: one parse per change in any read order, still ui-markdown's "parse memoised"; a StrictMode test holding the workspaces read asserts one parse and fails without it. (2) CLI `tests/read_trust.rs` `ac6g` (12e): the 1 s reconcile interval could pass before the `show` under load; now 3 s, with premises that the `show` ran within it and did not walk; T5's mutation still turns it red.

| Module | What it does |
|---|---|
| core `workspace_toml.rs`, `project_toml.rs`, `lib.rs` | the reader; `RawFile`'s two keys |
| CLI `workspace.rs`, `lib.rs`, `main.rs` | `spec workspace [--json]` |
| http `app.rs`, `answer.rs` | the route, no turn (`document` shared), the 404's list |
| `scripts/presets/gamedev.toml` | the preset |
| UI `src/workspace/`, `api/`, `app/Shell.tsx`, `routes.ts`, `board/`, `flow/FlowView.tsx`, `tree/TreeView.tsx`, `palette/options.ts`, `mocks/`; `markdown/Markdown.tsx` | seam, choice, nav, switcher, notes, bar, Board tabs; the parse cache |
| tests | core `workspace_toml.rs`; CLI `workspace_cli.rs`; mcp `mcp_workspaces.rs`; http `workspaces.rs`, `turn.rs`, `hub.rs`, `no_domain.rs`, `daemon_keys.rs`; eval `workspace_names.rs`, `ui_policy.rs`, `anonymity.rs`; fixtures `studio-lamp`, `card-press`, `daemon-keys.json` |
