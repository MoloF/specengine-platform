---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, ui, plugin, scripts, fixtures]
ref: RM-0096, v5 step 4 (08 s2 Phase 7, "v5 scope", "The mapping"); the owner's v5 answer 4 and Q3 of 2026-11-10; the analyst's requirement analysis of 2026-11-11 (A1-A9); the orchestrator's seven working answers (W1-W7), the owner's review open; DP-1 to DP-16
adrs: [ADR-0076]
---

# departments: the owner's words, flow titles, directions and a hand-off by department (RM-0096)

## Why

On 2026-11-10 the owner named the words people should see: "Project", "Task" for a card, "Subtask" for a child card, "Agent task" for a `T-` task, and "Department" for a workspace lens (v5 answer 4). In the same request he asked for documents spread into **directions** the project configures (mechanics, models, scenes), so one sees what an object consists of, and for each department to have its own statuses, with nothing hard-coded.

Today:
- **The chrome's words are the engine's.** It says Workspace, card and Tasks (for `T-` tasks), so "Task" cannot mean a card until every `T-` task says "Agent task".
- **A module names one root.** A direction by node kind cannot be written: ADR-0073's "Cost" says a kind selector needs a corpus read.
- **Raw words leak.** The Board's tabs show flow names (`level-design`); a document's Inside groups its children by raw kind (`mechanic`); doc-modal deferred both kinds' words and "Task" for a card to this slice (`doc-modal.md` "Out of scope").
- **A hand-off asks for a discipline word.** The person must know the config's discipline names.

This slice gives the owner's words in both catalogs, an optional flow `title`, `[[module]] kinds` directions, and a hand-off that picks a department. ADR-0076 amends ADR-0073; its canon is `docs/canon/workspaces.md` "The lens".

## Description and interactions

### Decisions

- **D1. Words in the chrome only.** In both catalogs a workspace reads Department, a card Task, a child card Subtask, a `T-` task Agent task, and a `kinds` module a direction (the Tree's "Directions" group). Every value that names a `T-` task moves to "agent task" in the same change that makes a card a "task", so no screen says "task" for both. The nav's `tasks` section reads "Agent tasks". Catalog keys, section IDs (`SECTIONS`), routes, `localStorage` keys, config keys, CLI, MCP and daemon text are unchanged: no `[[department]]` key or alias (A1, A2).
- **D2. A department stays a lens** (ADR-0073, Q3). Its statuses are the columns of the flows it shows, each column's category from its place (`ui-design.md` "Status categories"); no `categories` key. It hides no work: Show all flows stays one click away.
- **D3. `[[card.flow]] title`**, optional: any script, 1-64 UTF-8 bytes, no control character; absent, the flow's `name`. Always served, right after `name`, in `list`'s `flows[]` (CLI, MCP `list_cards`, daemon); the text output is unchanged (A3). `FLOW_KEYS` 7 to 8. A table under `title`, or a string under `titles`, gets a hint naming the other key ("Data").
- **D4. A direction is a `[[module]]` with `kinds`.** A module holds exactly one of `root` or `kinds` (W5). `kinds`: 1-32 entries, each 1-64 bytes, no control character, no space at an end, each once; matched exactly against a node's `kind`, never checked against `[ids]`; a node with no kind matches none (A4). A bad `kinds` is a malformed `[[module]]`: no module is served, one note (ADR-0073's rule). Served `modules[]` `{name, title, root, kinds}`: `root` null for a direction, `kinds` null for a rooted module. Text: `module <n> | <t> | kinds <a, b>`.
- **D5. The Tree lists a direction flat.** Choosing a direction reads `getTree({kinds})` with no root, the read `spec tree --kind` already serves; its `depth` and `parent` are the whole tree's (`spec-cli-graph.md`), so `rows.ts` `buildRows` would hang rows under the wrong parent. The rows are drawn flat, in served order, one level, nothing to expand.
- **D6. Flow takes rooted modules only** (W1). Its places pane lists rooted modules; a stored direction reads as All modules, the key untouched; a hash `module=<direction>` drops by a replace, as an unserved module does. Flow's kind cut is RM-0102, in the pool.
- **D7. Inside groups by direction** (W2). In a document's Inside (`doc-view.md` "Inside"), within each holder, a child whose kind a direction lists joins that direction's group, the first such direction in served order (A8); directions come first, in served order, labelled by title; kinds no direction lists stay raw groups, as today. Only `kinds` modules group (A6). The direction titles are the words for kinds (W6); link types stay raw (ADR-0031). No new request: the Shell's held workspaces read.
- **D8. The daemon serves each department's `disciplines`** (W4, ADR-0033: the UI computes no core rule). For each workspace, its flows in its order (`null`: every flow, in config order), each flow's disciplines in written order, each once, a discipline kept only if the first flow listing it in config order (`CardConfig::flow_for`) is one of the department's flows. A `[card]` that does not read gives `[]`.
- **D9. Hand off picks a department** (W3). With two or more departments whose `disciplines` is not empty, a required Department select; a department with several disciplines adds a Discipline select defaulting to its first. With fewer than two, today's Discipline field stays. The write still sends `discipline`; nothing new is sent.
- **D10. Flow titles on the Board.** A tab, the board's accessible name, a Parent lane head, the work item's side Flow row and each Subtasks row show the flow's `title`. A raw `<flow>/<column>` pair stays raw: the hand-off's polite line (the daemon's answer) and the document view's Cards block (A9).
- **D11. The preset** titles its five flows and ships no direction, only a commented example (W7); its header names the rollout rule for `title` and `return`.
- **D12. Plugin 0.35.0.** `list_cards`' flows gain `title`, a key a tool returns: a MINOR. `read-spec` gains the vocabulary line, trimmed to stay within its 4 096-byte body ("Data").
- **D13. Genre guards** take every `kinds` entry and each preset flow's `title` ("Data", DP-6).
- **D14. Rollout.** Older binaries refuse a flow's `title` (an older `spec card` exits 2, an older daemon or hub answers 503 on cards, an older MCP server fails the card tools) and drop every module when one holds `kinds` (one note; workspaces naming them get theirs). ADR-0073's rule holds: every terminal, plugin and hub serving a project upgrades before the keys are committed.

### The contract (Rust lane)

- **Core `card_toml.rs`.** `CardFlow.title: String`, after `name`, serialized there; `FLOW_KEYS` gains `title` after `name`; `read_flow` reads it with the title checks of `workspace_toml.rs`' `title_problem`, moved here and shared (`pub(crate)`). `read_titles`' type error on a string, and `title`'s on a table, append the hints. `PLAIN_FLOW` is unchanged: `plain` is titled `plain`.
- **Core `workspace_toml.rs`.**
  - `WorkspaceModule { name, title, root: Option<String>, kinds: Option<Vec<String>> }`; `MODULE_KEYS` `["name", "title", "root", "kinds"]`; `read_modules` takes `kinds` through `strings` (the 32 cap) and judges each entry; exactly one of the two.
  - `Workspace.disciplines: Vec<String>`, after `flows`. `Judge` keeps the whole `CardConfig` it reads once (today only the names). A failed `[card]` read pushes its note only where `flows` are judged, as now; `disciplines` is then `[]`. The implicit workspace serves `disciplines` as `flows: null` would.
- **CLI `workspace.rs`.** `render_text` prints `module <name> | <title> | kinds <a, b>` for a direction; a workspace's line is unchanged (`disciplines` is JSON only). JSON follows the structs.
- **Twins.** `GET /api/projects/:p/workspaces` serves the CLI's JSON byte for byte, unchanged in code; so do the card routes for `flows[]`. MCP `mirror.rs` `CardFlowShape.title` (after `name`), so the schemas' hash moves. No new route, tool, store column or queue step.
- **Plugin** 0.35.0 (`plugin.json`, `PINS`, `SCHEMA_PINS`, `scripts/deploy/.env.example`); `skills/read-spec/SKILL.md` as "Data".
- **Preset** `scripts/presets/gamedev.toml` as "Data".

### The view (UI lane)

- **Types** (`src/api/provisional.ts` and the card types): `Workspace.disciplines`, `WorkspaceModule.root: string | null`, `.kinds: string[] | null`, `CardFlow.title`. `daemonKeys.test.ts` lists the three in `AHEAD_OF_FIXTURE` until the Rust lane's `fixtures/daemon-keys.json` lands; empty at ship.
- **Words** (`src/i18n/en.ts`, `ru.ts`): D1 across every group; the new keys of "Data".
- **Switcher** (`WorkspaceControls.tsx`): its label and hint say Department; nothing else moves.
- **Board** (`BoardView.tsx`, `CardPanel.tsx`, `swimlanes.ts`): D10.
- **Hand off** (`CardDialogs.tsx` `HandOffDialog`): D9, from the held workspaces read; the form is chosen when the dialog opens. A department with no discipline is not offered.
- **Module bar** (`ModuleBar.tsx`): group "Modules", All modules then the rooted modules' titles; group "Directions" (`workspace.directionsLabel`), each direction's title, when one is served; one choice across both, `aria-pressed`, wrapping at 320 px. Only rooted modules: no Directions group; only directions: Modules holds All modules alone.
- **Tree** (`TreeView.tsx`): a `moduleKinds` prop beside `moduleRoot`. While the view's root is the module's start and a direction is chosen, the read is `{kinds, archive}` and the rows flat (D5): each row `depth` taken as 0 before `buildRows`, so every row is `aria-level="1"`, none expandable. Show as root leaves the list for a rooted read; Whole tree returns to the start, the direction's list; choosing All modules reads the whole tree.
- **Flow** (`FlowPlaces.tsx`, `FlowView.tsx`): D6.
- **Inside** (`src/doc/inside.ts`, `DocInside.tsx`, lazy): D7, the directions from `useWorkspaces`; pending or failed, raw groups, never waiting.
- **Mocks**: harbor-sim's `src/mocks/workspaces.ts` gains the direction `mechanics` "Mechanics" `kinds: ["mechanic"]` and each workspace's `disciplines`; `src/mocks/cards.ts`' `copy` flow `title: "Copy desk"`, `plates` none (its name); `src/test/docStub.ts` a workspaces document with two directions ("Data", Inside).

## Data

### `[[card.flow]] title`

```toml
[[card.flow]]
name = "copy"
title = "Copy desk"
disciplines = ["copy"]
columns = ["draft", "edit", "proof", "filed"]
accept = { from = "proof", to = "filed" }
```

`spec card list --json`, `list_cards`, `GET .../cards`: `flows` `[{name, title, disciplines, columns: [{id, title}], moves: [{from, to, by}], accept: {from, to}, return}]`; untitled, `title` is the name (`"plates"`). The text `flow copy | draft 0 | ...` is unchanged. `[card]`'s errors, each `<config>:<line>: <message>` at the value's line, `spec card` exit 2:

```text
`title` is a string, not a table: column titles go in `titles`
`title` is a string, not <an integer | a float | a boolean | a datetime | an array>
`title` is <N> bytes long; a title has 1 to 64
`title` holds a control character
`titles` is a table, not a string: a flow's own title is `title`
```

`titles` given another type keeps today's `` `titles` is a table, not <type> ``; an unknown key's message lists the eight `FLOW_KEYS`.

### `[[module]] kinds`

```toml
[[module]]
name = "scenes"
title = "Scenes"
kinds = ["scene"]
```

Each problem makes the `[[module]]` array malformed: no module is served and one note, the first problem by line, `<config>:<line>: <problem>; no module is served`:

```text
`[[module]]` `<n>` holds both `root` and `kinds`: a module holds one        (at `kinds`' line)
`[[module]]` is required to hold `root` or `kinds`                          (at the table's line; today's "`root`")
`kinds` lists no kind; a direction lists 1 to 32                            (at the value)
a list holds at most 32 entries                                             (at the 33rd)
`kinds`: `<k>` is <N> bytes long; a kind has 1 to 64
`kinds`: a kind holds a control character
`kinds`: `<k>` has a space at an end
`kinds`: `<k>` is listed twice
```

A wrong type is the TOML reader's message. A workspace's `module` may name a direction (A7).

### `spec workspace` and `GET .../workspaces`

`fixtures/studio-lamp`, its third module the direction `scenes`; text (each line one line):

```text
workspaces config | workspaces 4, modules 3
workspace writing | Writing | landing flow | sections flow, tree, board, questions, inbox | flows narrative | module (none)
...
module shore | The shore | root CHP-SHORE
module island | The lamp island | root ISL-LAMP
module scenes | Scenes | kinds scene
```

JSON, every key, this order: `{source, notes, workspaces: [{name, title, landing, sections, flows, disciplines, module}], modules: [{name, title, root, kinds}]}`:

```json
{"name": "art", "title": "Art and levels", "landing": "board", "sections": ["board", "flow", "tree", "questions", "inbox"], "flows": ["art", "level-design"], "disciplines": ["art", "level-design"], "module": null}
{"name": "production", "title": "Production", "landing": "overview", "sections": ["inbox", "roadmap", "board", "flow", "tree", "graph", "health", "questions"], "flows": null, "disciplines": ["narrative", "development", "art", "level-design", "production"], "module": null}
{"name": "shore", "title": "The shore", "root": "CHP-SHORE", "kinds": null}
{"name": "scenes", "title": "Scenes", "root": null, "kinds": ["scene"]}
```

**The `disciplines` rule** (D8), on flows `a` `["x", "y"]`, `b` `["y", "z"]`, `c` none:

| Workspace `flows` | `disciplines` | Why |
|---|---|---|
| `["b"]` | `["z"]` | `y`'s first flow is `a`, not the department's |
| `["a", "b"]` | `["x", "y", "z"]` | |
| `["b", "a"]` | `["y", "z", "x"]` | the department's order |
| `null` | `["x", "y", "z"]` | every flow |
| `["c"]` | `[]` | `c` lists none; the department is not offered (D9) |

### Key sets and the plugin

`fixtures/daemon-keys.json`, still 115 sets: 88 `CardFlow` `[name, title, disciplines, columns, moves, accept, return]`; 112 `Workspace` `[name, title, landing, sections, flows, disciplines, module]`; 113 `WorkspaceModule` `[name, title, root, kinds]`. Plugin 0.35.0: `plugin.json`, the last `PINS` and `SCHEMA_PINS` entries, `scripts/deploy/.env.example`, `spec_ids_hub.rs`, `tag-check.sh`.

`read-spec`'s body goes from 4 094 to 4 089 bytes (the file 4 415 to 4 410). Three edits, the first two trims:

| Was | Becomes |
|---|---|
| `- The answers are deterministic: the same files and the same request give the same answer, so reading an unchanged node again only spends context.` | `- The answers are deterministic: reading an unchanged node again only spends context.` |
| `... here or above), the working directory is not a SpecEngine project: stop using these tools for this session, do not set one up yourself, and say so instead of guessing what the spec holds.` | `... here or above), stop using these tools, do not set one up, and say so rather than guess what the spec holds.` |
| the last bullet's end, ``... needs a `note`, why it goes back.`` | the same, one space, then ``People use the web UI's words: a card is a task, a child card a subtask, a task of `get_task` an agent task, a workspace a department.`` |

### The preset

`scripts/presets/gamedev.toml`: each flow gains `title` after `name`: `narrative` "Narrative", `development` "Programming" (W8), `art` "Art", `level-design` "Level design", `production` "Production". No `[[module]]` table. The header's last two lines become: a flow's `title` and `return` need a build that reads them on every terminal, plugin and hub serving the project, an older one refusing either key. After the workspaces, a commented example:

```toml
# A direction: a module chosen by node kind, with no root. The Spec tree lists
# its kinds' nodes and a document's Inside groups them under its title; Flow,
# which draws from a root, leaves it out. Use your project's own kinds, and add
# it only once every terminal, plugin and hub runs a build that reads `kinds`.
# [[module]]
# name = "scenes"
# title = "Scenes"
# kinds = ["scene"]
```

### Fixtures

- `fixtures/studio-lamp`: the preset verbatim between its markers; after them a third module, the direction `scenes` above (its `scene` nodes `SCN-ARRIVAL`, `SCN-STORM`); `expected.json`'s `workspaces` text as above.
- `fixtures/card-press`: `copy` gains `title = "Copy desk"`, `plates` stays untitled; `expected.json`'s `flows` gain `title`.

### Catalog words

Values only; keys unchanged. English values become: Department (`workspace.workspaceLabel`; the hint "for one department"), Directions (new `workspace.directionsLabel`), New task, Cancel task, Tasks (the doc view's and Flow's card blocks), Subtasks (the Hand-offs block), `<n> subtasks` (a parent's tile), Agent tasks (`app.sections.tasks.label`), and "agent task" wherever a `T-` task was "task". Hand off: Department (new `board.field.department`, required, the placeholder option "Choose a department"), its hint "The department that takes the subtask.", and the Discipline select reusing `board.field.discipline`. The act keeps its name, Hand off.

Russian, in `ru.ts` alone: the owner's words for Department, Task and Subtask (transliterated here: Otdel, Zadacha, Podzadacha); an agent task as `board.item.agentTasks` already says it; Directions the ui-developer's. Russian grammar follows each sentence, the owner's review open (`ui-languages.md` "Known limits").

### Inside, the stub

`src/test/docStub.ts`' workspaces document: `records` "Records" `kinds: ["assumption"]`, then `asked` "Asked" `kinds: ["question", "assumption"]`. Over `getTree({root: DOM-FERRY, depth: 2})`: the document's group holds raw `mechanic` (`MEC-BOARDING`); `RULE-FERRY-TIDES`' group holds "Records" (`ASM-0001`, `ASM-0002`), then "Asked" (`QST-0001`).

## Rules and edge cases

- WHEN a `[[module]]` holds both `root` and `kinds`, or neither, or a bad `kinds`, the reader SHALL serve no module and one note, the first problem by line; a workspace naming a module then gets today's "no `[[module]]` is named" note.
- WHEN a workspace's `module` names a direction, it SHALL be served; the Tree starts at the direction's list and Flow reads as All modules.
- WHEN `[card]` does not read, every `disciplines` SHALL be `[]` and the notes SHALL be today's.
- WHEN two directions list a node's kind, Inside SHALL put it under the first in served order; the Tree's list for each SHALL list it. A node whose `kind` is null SHALL match none.
- WHEN a direction's kinds match no node, the Tree SHALL show the tree read's empty answer as today, and Inside no group for it.
- WHEN the workspaces read is pending or failed, Inside SHALL group by raw kind without waiting, the module bar SHALL be absent as today, and a Hand off dialog opened then SHALL keep today's Discipline field for its life.
- WHEN the stored module names a direction and Flow opens, Flow SHALL read as All modules and leave the key; `module=<direction>` in Flow's hash SHALL drop by one replace.
- WHEN a flow has no `title`, the screens SHALL show its name. Titles need not be unique: two equal tabs are data.
- WHEN fewer than two departments serve a discipline, Hand off SHALL be today's dialog; with two or more, Submit SHALL send no request until a department is chosen.
- Config keys, CLI and MCP arguments, the daemon's notes and refusals keep `workspace`, `module`, `card` and `task` (ADR-0076). The UI shows them verbatim under `lang="en"`, as today.
- Nothing branches on a department, module or direction name, a kind, a discipline or a flow title (ADR-0008, ADR-0031): `policy.test.ts`, `workspace_names.rs`, `no_domain.rs`.
- **Scale.** No new read. The config read stays config-only (no turn). A direction's Tree read is today's whole-tree walk with a line filter (`get_tree` about 956 ms at 100k, `scale-eval.md` "Baseline"), the cost of All modules today. Inside's grouping reads nothing.

### Test churn

Only these change; every other test passes unchanged. The test-engineers add what is missing under "Implementation".

| Test | Becomes | Why |
|---|---|---|
| core `workspace_toml.rs`: module messages ("no root"; "a kind selector (D4)", an unknown `kinds` today, the both-keys message after), unknown-key lists (`MODULE_KEYS` 4, `FLOW_KEYS` 8), WS-7 | `root` or `kinds`; the lists; the preset as "Data" | D3, D4, D11 |
| CLI `workspace_cli.rs` (`ws4`'s "`root`" note too), `tests/workspaces_common`; `card-press` and `studio-lamp` `expected.json` | `disciplines`, `kinds`, nullable `root`; flows' `title`; `modules 3` | D3, D4, D8 |
| CLI `cards.rs`, `card_handoff.rs`, `card_tasks.rs`, `card_trace.rs` goldens of `flows` | `title` after `name` | D3 |
| http `workspaces.rs`, `cards.rs`, `daemon_keys.rs`; `fixtures/daemon-keys.json`; UI `api/daemonKeys.test.ts` (its `WorkspaceModule` regex too) | sets 88, 112, 113 | D3, D4, D8 |
| MCP `mcp_cards.rs` (the schema), `plugin_files.rs`, `plugin_skills.rs` (read-spec 4 410 / 4 089), `plugin_roles.rs`; http `spec_ids_hub.rs` | 0.35.0 | D12 |
| eval `workspace_names.rs`, http `no_domain.rs` | the new words | D13 |
| UI tests that query chrome by its English words: about 90 test files hold "card", "workspace", "task" or "hand-off"; those that find an element by such a name, or expect such a line | the new words | D1 |
| UI `workspace/shell.test.tsx`, `board/BoardView.test.tsx` (tabs), `board/HandOffDialog.test.tsx`, `doc/inside.test.ts` (the group shape), `tree/TreeView*.test.tsx` (module props), `flow/FlowView.test.tsx` (places), `mocks/MockClient.test.ts` (workspaces), `policy.test.ts` (the mock's names) | the new shapes and words | D4-D10 |

## Acceptance criteria

A browser look is marked **[look]**; a timed run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red.

- [ ] DP-1 — **A flow's title.** `crates/specengine-core/tests/departments.rs` (new):
  - `card_from_toml` on `copy` with `title = "Copy desk"` serves it; untitled `plates` serves `"plates"`; `PLAIN_FLOW` serves `"plain"`; a 64-byte title of `é` 32 times (two bytes each) reads; one more ASCII letter, 65 bytes in 33 characters, is refused.
  - Each of "Data"'s five messages at its value's line; a flow's unknown key lists eight keys, `title` second.

  `crates/specengine-cli/tests/departments.rs` (new), on a scratch copy of `card-press`: `spec card list --json`'s `flows[0]` keys equal set 88 in order, `title` `"Copy desk"`; the text output is byte-equal to the build before. Mutations, each red: `title` serialized after `disciplines`; the `titles` hint dropped; the 64-byte bound counted in characters.
- [ ] DP-2 — **Directions.** `departments.rs` (core):
  - `kinds = ["scene", "chapter"]` serves `root: None, kinds: Some(["scene", "chapter"])`, written order; a rooted module `kinds: None`;
  - each of "Data"'s `[[module]]` messages, at its line, with `; no module is served`, and every module dropped;
  - 32 kinds read, 33 refused; a workspace `module = "scenes"` naming a direction is served.

  Mutations, each red: both keys served; a repeated kind kept; the "required to hold `root`" message kept.
- [ ] DP-3 — **The read and `disciplines`.** `departments.rs` (core): the five rows of "Data"'s table, exactly, on the `a`, `b`, `c` config; a `[card]` that does not read gives `[]` everywhere and no new note; the implicit workspace serves every flow's disciplines. `departments.rs` (CLI), on `studio-lamp`:
  - `spec workspace` text equals "Data"'s lines; JSON keys and values as "Data";
  - `spec tree --kind scene --json` lists `SCN-ARRIVAL` and `SCN-STORM`, depth 1: the read a direction uses, unchanged.

  http `workspaces.rs`: `GET /api/projects/studio-lamp/workspaces` is byte-equal to the CLI's JSON; no project turn is taken. Mutations, each red: a discipline kept when any of the department's flows lists it (`["b"]` gives `["y", "z"]`); config order instead of the department's (`["b", "a"]`).
- [ ] DP-4 — **Twins, keys, plugin.**
  - MCP `list_cards` on `card-press`: `structuredContent` equals the CLI's JSON and validates against `outputSchema`, whose flow object requires `title`, a string.
  - HTTP `GET /api/projects/card-press/cards` byte-equal to the CLI's JSON.
  - `daemon_keys.rs` and `fixtures/daemon-keys.json`: 115 sets, 88, 112 and 113 as "Data"; UI `daemonKeys.test.ts` passes with `AHEAD_OF_FIXTURE` empty.
  - `plugin_files.rs`: `PINS` and `SCHEMA_PINS` end at 0.35.0; `spec_ids_hub.rs` and `tag-check.sh` read 0.35.0.
  - `plugin_skills.rs`: read-spec's body 4 089 bytes (at most 4 096), the file 4 410; the vocabulary sentence present; every content rule passes (`get_task` a tool name).

  Mutations, each red: the mirror without `title`; `plugin.json` left at 0.34.0; the vocabulary sentence dropped.
- [ ] DP-5 — **Rollout**, proven on today's code; no older build is run. An older binary reads `title` and `kinds` as unknown keys (its `FLOW_KEYS` 7, `MODULE_KEYS` 3), and these tests pin what an unknown key does, with `colour`, unknown before and after this slice, so each passes unchanged across it:
  - **`[[card.flow]]`.** CLI `crates/specengine-cli/tests/cards.rs` `ac05_a_bad_card_table_stops_spec_card_alone`, its `bad_tables()` case "another key" (`colour = "red"`): `spec card list`, `new`, `show` and `cancel` exit 2 (`cannot`), stdout empty, stderr `specengine.toml:<line>:` within the table and naming `colour`; `spec tree`, `check` and `task list` unchanged. MCP `crates/specengine-mcp/tests/mcp_cards.rs` `a_bad_card_table_stops_the_card_tools_alone`: the five card tools answer an error, `get_tree` answers. `ac05` pins a line of the table, not the key's own, so the test-engineer adds `dp5_an_unknown_flow_key_exits_2_at_its_line` to `cards.rs`, green on today's rule before the Rust lane lands: `colour = "red"` in a `[[card.flow]]`, `spec card list` exits 2, its stderr starting `specengine.toml:<the colour line>: unknown field `colour`, expected one of ` (the prefix only: this slice adds `title` to the list). Mutation, red: the line taken from the flow's `[[card.flow]]` header.
  - **`[[module]]`.** Core `crates/specengine-core/tests/workspace_toml.rs` `a_malformed_module_table_serves_no_module_with_one_note`, case "an unknown key" (`colour`): no module, one note at its line ending `; no module is served`, the workspaces read apart, `ProjectConfig` accepting the file. At 47ca1e0 its case "a kind selector (D4)" is exactly a direction meeting the old reader, `unknown field `kinds``; this slice rewrites that case to the both-keys message ("Test churn"). CLI `crates/specengine-cli/tests/workspace_cli.rs` `ws4_a_bad_table_fails_nothing`: a `[[module]]` without `root` serves `modules: []`, exit 0, `spec check`'s counts and `spec tree`'s bytes unchanged.
  - The header-sentence test (core `departments.rs`) pins the preset's sentence naming `title`, `return` and the upgrade rule; mutation, red: the sentence dropped.
- [ ] DP-6 — **Genre guards.**
  - eval `workspace_names.rs`: the names gain every `kinds` entry of every `[[module]]` in the repository's configs and presets, and each preset flow's `title`; the scratch-tree control: a preset `title = "Kiln work"` with `"Kiln work"` in a probe `.rs`, and a fixture `kinds = ["quarry"]` with `'quarry'` in a probe `.tsx`, are each red; a fixture flow's `title` is not read.
  - http `no_domain.rs`: the words gain every `kinds` entry, lowercased; its reader returns exactly `{"scene"}` on the repository.

  Mutations, each red: the guard reading `root` instead of `kinds`; preset titles not read.
- [ ] DP-7 — **The preset.** core `departments.rs` and `workspace_toml.rs` WS-7:
  - `gamedev.toml` parsed with the TOML reader: five flows, each `title` as "Data", in order; no `module` key; the text holds the commented example's four `# ` lines verbatim;
  - `studio-lamp` holds the preset verbatim between its markers and reads as one config: `spec card list --json`'s titles equal the preset's; `spec workspace` as DP-3.

  Mutations, each red: a live `[[module]]` in the preset; `art`'s title dropped.
- [ ] DP-8 — **Words, English.** `ui/src/i18n/words.test.ts` (new), over every leaf of `en`:
  - no leaf holds `/\bcards?\b/i` or `/\bworkspaces?\b/i`;
  - every `/\btasks?\b/i` in a leaf under `tasks.` or `app.sections.tasks`, and in each leaf of the test's `AGENT_TASK_KEYS` (the leaves outside `tasks` that named a `T-` task before this change, listed by the ui-developer's sweep), is preceded by `agent `, any case;
  - `app.sections.tasks.label` is "Agent tasks"; `workspace.directionsLabel` "Directions"; `workspace.workspaceLabel` "Department".

  `ui_policy.rs` still pins `SECTIONS` with `tasks`. Mutations, each red: `app.sections.tasks.label` back to "Tasks"; the board's New card label kept.
- [ ] DP-9 — **Words, Russian.** `words.test.ts`: no `ru` leaf holds the stem of today's word for a card or the phrase of today's workspace label (both written as `\u` escapes, no Cyrillic in the test); `workspace.workspaceLabel` equals the owner's word for Department (escaped). `catalog.test.ts` passes: parity, plurals, placeholders. `app/screens.language.test.tsx` switches the Board, the card modal and the Hand off dialog to Russian: chrome from `ru.ts`, titles verbatim, the daemon's words under `lang="en"`. Mutation, red: `ru` `workspace.workspaceLabel` restored.
- [ ] DP-10 — **The department's Board.** `BoardView.test.tsx`, `workspace/shell.test.tsx`, on harbor-sim:
  - the switcher's label is "Department"; choosing Galley shows one tab set by `title`: "Copy desk"; Foundry's tab reads "plates";
  - with every flow shown, tabs "Copy desk", "plates"; Show all flows reaches every flow from a department;
  - the board's accessible name, a Parent lane head and the side Flow row show "Copy desk"; the hand-off's polite line keeps raw `copy/draft`;
  - the document view's Cards block keeps raw `copy/<column>`.

  Mutation, red: tabs labelled by `name`.
- [ ] DP-11 — **Directions in the Tree.** `TreeView.test.tsx` and `mocks/workspaces.smoke.test.tsx`, on harbor-sim:
  - the module bar holds group "Modules" (All modules, Berths, Harbour waters) and group "Directions" (Mechanics), one pressed;
  - choosing Mechanics stores `mechanics` in `specengine.module.harbor-sim` and reads `getTree` with `kinds: ["mechanic"]` and no `root`; every row is `aria-level="1"`, none has `aria-expanded`; served order;
  - Show as root on a row reads `{root: <it>}`; Whole tree returns to the list; All modules reads the whole tree; a reload keeps Mechanics;
  - with directions only, Modules holds All modules alone; with rooted modules only, no Directions group.

  `policy.test.ts` bans the mock's direction name and kinds quoted in app code. Mutations, each red: rows built on the served depths (a level of 2); a direction's `null` root passed as `root`.
- [ ] DP-12 — **Flow, rooted only.** `FlowView.test.tsx`, on harbor-sim:
  - the places pane's modules are Berths and Harbour waters, not Mechanics;
  - stored `mechanics`: `#/harbor-sim/flow` reads as All modules, the key unchanged;
  - `#/harbor-sim/flow?module=mechanics` replaces to the hash without `module` (one replace, no push).

  Mutation, red: the places pane listing every module.
- [ ] DP-13 — **Inside by direction.** `doc/inside.test.ts` and `DocBody.test.tsx`, on `docStub`:
  - the groups are "Data"'s: raw `mechanic` under the document; "Records" then "Asked" under `RULE-FERRY-TIDES`, each `role="group"` labelled by its title in a `bdi`, no `lang`; a raw kind keeps `lang="en"`;
  - expanding makes one `getTree` call and no `getWorkspaces` call beyond the Shell's;
  - with the workspaces read failed or pending, the groups are today's raw kinds, at once.

  Mutations, each red: the last matching direction (`ASM-0001` under "Asked"); directions after raw kinds.
- [ ] DP-14 — **Hand off by department.** `HandOffDialog.test.tsx`, a workspaces document: Galley `["copy"]`, Foundry `["plates", "proofs"]`, Yard `[]`:
  - one required `combobox` "Department": Galley, Foundry, in served order, no Yard, no default; Submit without a choice sends nothing and shows the field's error;
  - Galley: no Discipline select; Submit posts `discipline: "copy"`, `parent`, `reviewer`, `as`, and no new key;
  - Foundry: a Discipline select showing `plates`; choosing `proofs` posts it;
  - with Foundry alone serving disciplines, today's Discipline field and datalist; with the read pending at open, today's field, kept when the read lands.

  Mutations, each red: the department's name sent as `discipline`; the second select starting empty.
- [ ] DP-15 — **[measure] Size and gates.**
  - After `pnpm build`, first-load JS (`ui-languages.md` "Size") is at most 845 kB; expected +2 to +6 kB on 824.13; over +10 kB, the reason is written under "Implementation" with the figure, gzip and `ru-*.js`.
  - UI: lint 0, `tsc` clean, the full test run green twice. Rust: the workspace run green. `cargo nextest run -p specengine-eval --test doc_pointers --test anonymity --test ui_policy --test workspace_names`. The docs check clean.
- [ ] DP-16 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide, on the mock and on a scratch copy of a real game corpus with two departments and one direction configured:
  - the switcher, the Board's tabs by title, a parent's Subtasks;
  - the Hand off dialog with both selects;
  - the Tree's bar with both groups, a direction's flat list;
  - a document's Inside with a direction group;
  - Flow's places pane without the direction;
  - long Russian titles wrapping; at 320 the bar wrapping and the dialog filling the screen; focus rings visible, nothing cut unseen.

## Out of scope

- Flow drawn through a direction (RM-0102, in the pool).
- Words for link types (ADR-0031; doc-modal's Q-A stays open).
- A `[[department]]` key or alias; renaming any config key, CLI flag, MCP argument or daemon text.
- A `categories` key; statuses beyond a flow's columns.
- A department that hides work or grants a right (ADR-0073).
- Directions by anything but kind (a status, a path, a tag).
- `disciplines` in `spec workspace`'s text.
- A hub login's default department (RM-0089), an Overview per department (RM-0086), a parent's progress (RM-0098), `spec init --pack` (RM-0061).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/card_toml.rs` (`CardFlow.title`, `FLOW_KEYS`, hints, `title_problem`); `crates/specengine-core/src/workspace_toml.rs` (`WorkspaceModule`, `MODULE_KEYS`, `kinds`, `Workspace.disciplines`, `Judge`); `crates/specengine-cli/src/workspace.rs`; `crates/specengine-mcp/src/mirror.rs`; `plugin/specengine/.claude-plugin/plugin.json`; `plugin/specengine/skills/read-spec/SKILL.md`; `scripts/presets/gamedev.toml`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `crates/specengine-core/tests/departments.rs`, `crates/specengine-cli/tests/departments.rs` (new); the "Test churn" Rust rows; `fixtures/daemon-keys.json`, `fixtures/studio-lamp`, `fixtures/card-press`; eval `workspace_names.rs`, http `no_domain.rs`; DP-5's `dp5_an_unknown_flow_key_exits_2_at_its_line` in CLI `cards.rs` |
| ui-developer | `src/api/provisional.ts` and the card types; `src/i18n/{en,ru}.ts`; `src/workspace/{ModuleBar,WorkspaceControls}.tsx`; `src/tree/TreeView.tsx`; `src/flow/{FlowPlaces,FlowView}.tsx`; `src/doc/{inside.ts,DocInside.tsx}`; `src/board/{BoardView,CardDialogs,CardPanel}.tsx`, `swimlanes.ts`; `src/mocks/{workspaces,cards}.ts`; `src/test/docStub.ts`; `src/i18n/words.test.ts` (new) and the UI tests of DP-8 to DP-14 |
| test-engineer, UI | gates, mutations, DP-15; `ui_policy.rs` and `doc_pointers` if a pin moves |

Comments cite `docs/features/departments.md` "Acceptance criteria" with a DP number, or an existing canon heading; never "Data" or "Description and interactions", which compaction drops at ship (doc_pointers).

### Work order

1. The rust-developer and the ui-developer start together; the contract is fixed above. The UI runs on the mock, the three keys in `AHEAD_OF_FIXTURE` until the Rust lane's `daemon-keys.json` lands.
2. The Rust test-engineer writes DP-5's `cards.rs` test first, green on today's rule, then follows the Rust lane, overlapping the UI lane (DP-1 to DP-7); then the Rust review.
3. The UI lane completes; the UI test-engineer runs gates and mutations (DP-15); then the UI review.
4. The orchestrator's looks (DP-16).
5. Ship: the spec-writer ("Canon at ship").

At most three iterations a lane. **The cut line**, if the UI lane runs out: the Directions UI (D5, D7, the bar's Directions group) goes to a follow-up item; the served `kinds` stays; the Tree's bar and Flow's places list rooted modules only, a stored direction reads as All modules everywhere, and Inside groups by raw kind. The words, flow titles and the hand-off ship.

### Risks

- **Older binaries** (D14): a project that commits `title` or `kinds` before every terminal, plugin and hub upgrades loses its Board or its modules there; the `v0.29.1` hub refuses any `[[module]]` already (503 on every read).
- **The word sweep.** Every `T-` task must say "agent task" in the same change as a card saying "task"; a missed leaf shows "task" for both. DP-8's partition catches the groups it lists; review reads the rest.
- **`title` against `titles`**: one letter apart, the hints (DP-1).
- **Size**: 20.87 kB of headroom; the Hand off dialog and the bar are in first load, Inside lazy.
- **Churn**: about 90 UI test files hold a changed word; the plugin pins; three key sets; two fixtures' `expected.json`; `studio-lamp`'s verbatim preset.
- **Anonymity**: the looks' real corpus is never named in the repository.
- **Caps** at ship: below.

### Canon at ship

New Tier 2 canon **`docs/canon/directions.md`**, scope `[crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]`, at most 12 KB: the `kinds` module (config, errors, the served shape, the text line), the Tree's flat list, Inside's groups, Flow's exclusion, overlaps, guards, Known limits (older binaries drop every module; Flow draws none: RM-0102), Tests. `docs/index-canon.md` gains one line (9 574 B plus about 150).

Edits, each within its cap (free bytes now in brackets):

| File | Edit |
|---|---|
| `docs/canon/workspaces.md` [809] | "State" dropped (about 420 B back); "Config": `kinds` a pointer to `directions.md`, `disciplines`; "Read": the module line, `disciplines` and its rule; "Screen": Department, the Directions group; "Preset": titles, the commented direction; "Guards": `kinds`, titles; "Known limits": `title` and `kinds` on older binaries |
| `docs/canon/card-flows.md` [2 841] | "Config": `FLOW_KEYS` 8, `title`, its errors and hints; "Hand-off": the Department and Discipline selects (moved here from `board-screen.md`); "Known limits": older binaries refuse `title` |
| `docs/canon/cards.md` [206] | "Documents and events": `flows` `[{name, title, ...}]` (+7 B) |
| `docs/canon/board-screen.md` [72] | "Flows, filters, lanes": tabs by title; "Writes and dialogs" Hand off: a pointer to `card-flows.md` "Hand-off" replacing the field's sentence (net not positive) |
| `docs/canon/ui-design.md` [9] | "The work item": Hand-offs becomes Subtasks (net not positive) |
| `docs/canon/doc-view.md` [2 394] | "Inside": direction groups |
| `docs/canon/spec-flow-place.md` [6 231] | "The module", "Places": rooted modules only |
| `docs/canon/ui-languages.md` [1 757] | "The rule": the owner's words; "Guards": `words.test.ts` |
| `docs/canon/plugin-files.md` [5 243] | 0.35.0, `list_cards`' flows' `title` |
| `docs/canon/plugin.md` [480] | read-spec's vocabulary line, if it lists read-spec's content |
| `docs/canon/hub-deploy.md` [5] | "0.34.0, `doc-modal`'s" becomes "0.35.0, `departments`'s" (+2 B) |
| `CLAUDE.md`; 08 s2 Phase 7 step 4; RM-0096 | shipped |
| this spec | compacted to at most 3 KB |

`mcp-read.md` [34] and the crate READMEs (each under 22 B free) are not touched.

### Working answers and the owner's questions

**The orchestrator's working answers (W1-W7), taken:** W1 Flow takes no kind cut and lists rooted modules only (D6). W2 a direction replaces the raw-kind group in Inside, unlisted kinds raw (D7). W3 the Hand off selects (D9). W4 the daemon serves `workspaces[].disciplines` by the new-card rule (D8). W5 exactly one of `root` or `kinds` (D4). W6 direction titles are the words for kinds; link types raw (D7). W7 the preset ships no directions, a commented example, and titles its five flows (D11).

**The spec-writer's:** W8 the `development` flow is titled "Programming": "Development" is a chrome word (`board.item.development`, the work item's Development summary), and the guard's exact literal scan would meet it (DP-6). W9 `disciplines` is JSON only, the text unchanged. W10 a department with no discipline is not offered. W11 the Hand-offs block reads Subtasks; the act keeps "Hand off". W12 flow titles on the Board alone; raw pairs elsewhere (A9).

**The owner's questions:**
- **Q-A.** Should the preset's development flow be titled "Development", as its department is? That needs the guard to skip `src/i18n/`. Working answer: "Programming" (W8).
- **Q-B.** Should Flow draw a direction? Working answer: no; RM-0102 in the pool (W1).
- **Q-C.** Link-type words: doc-modal's Q-A, still open.
