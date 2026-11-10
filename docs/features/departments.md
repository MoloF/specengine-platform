---
class: spec
status: shipped
shipped: 2026-11-11
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, ui, plugin, scripts, fixtures]
ref: RM-0096, v5 step 4 (08 s2 Phase 7, "v5 scope", "The mapping"); the owner's v5 answer 4 and Q3 of 2026-11-10; the analyst's A1-A9 of 2026-11-11; working answers W1-W12, the owner's review open; DP-1 to DP-16
adrs: [ADR-0076]
---

# departments: the owner's words, flow titles, directions and a hand-off by department (RM-0096)

## Why

On 2026-11-10 the owner named the words people should see (v5 answer 4): "Project"; "Task" for a card; "Subtask" for a child card; "Agent task" for a `T-` task; "Department" for a workspace lens. In the same request he asked for documents spread into **directions** the project configures (mechanics, models, scenes), so one sees what an object consists of. He also asked that each department have its own statuses, with nothing hard-coded.

Before this slice:
- The chrome said Workspace, card and Tasks.
- A module named one root.
- The Board's tabs showed raw flow names, and Inside grouped by raw kind.
- A hand-off asked for a config's discipline word.

How it works now: `docs/canon/workspaces.md` ("The lens", "Read", "Screen", "Preset"); `docs/canon/directions.md`; `docs/canon/card-flows.md` ("Config", "Hand-off"); the words in `docs/canon/ui-languages.md` "The rule". ADR-0076 amends ADR-0073.

## Description and interactions

Each decision is canon now. The numbers stay because code and tests cite them.

- **D1** The owner's words in the chrome only: `ui-languages.md` "The rule".
- **D2** A department stays a lens, its statuses its flows' columns: `workspaces.md` "The lens".
- **D3** `[[card.flow]] title`: `card-flows.md` "Config".
- **D4** `[[module]] kinds`: `directions.md` "Config", "Read".
- **D5** The Tree's flat list: "Tree".
- **D6** Flow takes rooted modules only: "Flow", `spec-flow-place.md` "The module".
- **D7** Inside groups by direction: "Inside", `doc-view.md` "Inside".
- **D8** The served `disciplines`: `workspaces.md` "Read".
- **D9** Hand off by department: `card-flows.md` "Hand-off".
- **D10** Flow titles on the Board: `board-screen.md` "Flows, filters, lanes".
- **D11** The preset: `workspaces.md` "Preset".
- **D12** Plugin 0.35.0: `plugin-files.md`, `plugin.md` "Skills".
- **D13** Guards: `workspaces.md` "Guards", `directions.md` "Guards".
- **D14** Rollout: `workspaces.md` "Known limits", `card-flows.md` "Known limits".

## Data

`fixtures/daemon-keys.json` holds 115 sets:
- 88 `CardFlow` `[name, title, disciplines, columns, moves, accept, return]`;
- 112 `Workspace` `[name, title, landing, sections, flows, disciplines, module]`;
- 113 `WorkspaceModule` `[name, title, root, kinds]`.

`FLOW_KEYS` holds 8 keys, `MODULE_KEYS` 4, and `MAX_KIND_BYTES` is 64. The messages are in the canon. The body of `read-spec` is 4 089 B (the file 4 410) and ends: "People use the web UI's words: a card is a task, a child card a subtask, a task of `get_task` an agent task, a workspace a department."

## Acceptance criteria

A **[look]** and a **[measure]** are recorded under "Implementation". Every other criterion is a test that its named mutation turned red (17 Rust, 14 UI).

- [x] DP-1 — **A flow's title.** Core `tests/departments.rs`:
  - `copy` serves "Copy desk", the untitled `plates` serves `"plates"`, and `PLAIN_FLOW` serves `"plain"`;
  - 64 bytes of `é` read; 65 bytes in 33 characters are refused;
  - the five messages arrive at the value's line;
  - an unknown key's message lists eight keys, `title` second.

  CLI `tests/departments.rs` on `card-press`: the keys of `flows[0]` are set 88, `title` is "Copy desk", and the text is byte-equal to 5bec07e's (`departments_before.json`). M: `title` after `disciplines`; the `titles` hint dropped; the 64-byte bound counted in characters.
- [x] DP-2 — **Directions.** Core `departments.rs`:
  - `kinds = ["scene", "chapter"]` is served with `root: None`, in written order, and a rooted module serves `kinds: None`;
  - each `[[module]]` message arrives at its line with `; no module is served`, and every module is dropped;
  - 32 kinds read and 33 are refused;
  - `module = "scenes"` is served.

  M: both keys served; a repeated kind kept; the "required to hold `root`" message kept.
- [x] DP-3 — **`disciplines` and the read.** Core: the rule's five rows on `a`, `b`, `c`. A `[card]` that does not read gives `[]` and no new note. The implicit workspace serves every flow's disciplines. When every listed flow is unknown, `flows` is null and every discipline is served. CLI on `studio-lamp`:
  - `spec workspace` prints `module scenes | Scenes | kinds scene`, and its JSON matches;
  - `spec tree --kind scene --json` lists `SCN-ARRIVAL` and `SCN-STORM` at depth 1.

  HTTP is byte-equal to the CLI, and `beyond_daemon(HOME)` is empty (no turn). M: a discipline kept when any of the department's flows lists it; config order.
- [x] DP-4 — **Twins, keys, plugin.**
  - MCP `list_cards` equals the CLI's JSON (checked in-process) and validates against `outputSchema`, which requires `title`; HTTP `cards` is byte-equal.
  - 115 sets; UI `AHEAD_OF_FIXTURE` is empty.
  - `PINS` and `SCHEMA_PINS` end at 0.35.0; `spec_ids_hub.rs` reads 0.35.0.
  - read-spec is 4 089 / 4 410 B and holds the sentence.

  M: the mirror without `title`; `plugin.json` at 0.34.0; the sentence dropped.
- [x] DP-5 — **Rollout**, proven on today's code with `colour` as the unknown key; no older build is run.
  - CLI `cards.rs`: `ac05_a_bad_card_table_stops_spec_card_alone`, and the new `dp5_an_unknown_flow_key_exits_2_at_its_line`, whose stderr starts `specengine.toml:<line>: unknown field `colour`, expected one of `.
  - MCP `a_bad_card_table_stops_the_card_tools_alone`.
  - Core `a_malformed_module_table_serves_no_module_with_one_note`, whose old case "a kind selector (D4)" now holds the both-keys message.
  - CLI `ws4_a_bad_table_fails_nothing`.
  - Core, the preset's header sentence.

  M: the line taken from the `[[card.flow]]` header; the sentence dropped.
- [x] DP-6 — **Genre guards.** Eval `workspace_names.rs` takes every `kinds` entry and each preset flow's `title`. Its controls are red: "Kiln work" in a `.rs`, and `quarry` in a `.tsx`. A fixture flow's `title` is not read. HTTP `no_domain.rs` reads exactly `{"scene"}`. M: `root` read for `kinds`; preset titles not read.
- [x] DP-7 — **The preset.** Core `departments.rs` and WS-7:
  - the five titles, in order, and no `module` key;
  - the commented example pinned verbatim: all eight lines, not the four the draft named;
  - `studio-lamp` holds the preset verbatim, and its titles are the preset's.

  M: a live `[[module]]`; `art`'s title dropped.
- [x] DP-8 — **Words, English.** `ui/src/i18n/words.test.ts` checks:
  - no `en` leaf holds `card(s)` or `workspace(s)`;
  - `task(s)` follows `agent ` in every leaf of `AGENT_TASK_GROUPS` (`tasks.`, `app.sections.tasks.`, `home.tasks.`, `app.shortcuts.tasks.`, `board.tasks.`) and in `AGENT_TASK_KEYS`' 20 leaves;
  - "Agent tasks", "Directions" and "Department" are pinned.

  M: the nav's label "Tasks"; New card kept.
- [x] DP-9 — **Words, Russian.** No `ru` leaf holds the old stem or phrase (`\u` escapes), and the label is the owner's (Otdel). `catalog.test.ts` passes. In `screens.language.test.tsx` the Board, the card modal and Hand off are in Russian, titles are verbatim, and the daemon's words stay `lang="en"`. M: the `ru` label restored.
- [x] DP-10 — **The Board**, amended: a department that shows one flow draws no tab (WS-14). `BoardView.test.tsx` and `workspace/shell.test.tsx`:
  - the switcher reads "Department";
  - Galley shows "Showing 1 of 2 flows.";
  - with every flow shown, the tabs are "Copy desk" and "plates", and the board's name, a Parent lane head and the side Flow row show "Copy desk";
  - the hand-off's line and the Cards block stay raw.

  M: tabs by `name`.
- [x] DP-11 — **The Tree.** `TreeView.test.tsx` and `mocks/workspaces.smoke.test.tsx`:
  - the bar has "Modules" (All modules, Berths, Harbour waters) and "Directions" (Mechanics);
  - Mechanics stores `mechanics` and reads `getTree` with `kinds: ["mechanic"]` and no `root`; every row is `aria-level="1"`, with no `aria-expanded`;
  - Show as root, Whole tree, All modules and a reload work as `directions.md` describes;
  - a project with directions only, and one with rooted modules only.

  M: the served depths; a null root passed.
- [x] DP-12 — **Flow, rooted only**, amended: the drop writes the module in force (`spec-flow-place.md` "The place"). `FlowView.test.tsx`:
  - the places pane lists Berths and Harbour waters, not Mechanics;
  - a stored `mechanics` reads as All modules, its key unchanged;
  - `?module=mechanics` is replaced by `?module=`, with one replace.

  M: the pane lists every module.
- [x] DP-13 — **Inside.** `doc/inside.test.ts` and `DocBody.test.tsx` on `docStub`:
  - raw `mechanic` under the document;
  - "Records" (`ASM-0001`, `ASM-0002`), then "Asked" (`QST-0001`), under `RULE-FERRY-TIDES`, in a `bdi` with no `lang`;
  - one `getTree` and no extra `getWorkspaces`;
  - a pending or failed read gives raw kinds.

  M: the last matching direction; directions after raw kinds.
- [x] DP-14 — **Hand off.** `HandOffDialog.test.tsx` with Galley `["copy"]`, Foundry `["plates", "proofs"]` and Yard `[]`:
  - a required Department select offers Galley and Foundry, with no default;
  - Galley posts `discipline: "copy"` and no new key;
  - Foundry adds a Discipline select showing `plates`, and choosing `proofs` posts it;
  - with one department serving, or the read pending, the dialog shows today's field.

  M: the department's name sent as `discipline`; the second select left empty.
- [x] DP-15 — **[measure]** Size and gates: under "Implementation".
- [x] DP-16 — **[look]** Under "Implementation".

## Out of scope

- Flow through a direction (RM-0102).
- Words for link types (Q-C).
- A `[[department]]` key, and any renamed key, flag, argument or daemon text.
- `categories`.
- A department that hides work or grants a right.
- Directions by status, path or tag.
- `disciplines` in the text output.
- RM-0086, RM-0089, RM-0098, RM-0061.

The owner's Q-A ("Programming" for the preset's development flow) and Q-B stay open.

## Implementation

Reviews: ACCEPT. Rust passed in iteration 1, UI in iteration 2.

**Departures**, all accepted. Rust:
- `disciplines_of` is `pub`, not re-exported;
- "both" is decided by the keys' presence; a kind's checks run length, control character, end space, then repeat;
- a non-string `name` is left out of the both-keys message;
- `[card]` is read once per workspaces read;
- `tag-check.sh` is unchanged.

UI:
- one flow shown, no tab;
- `?module=` (DP-10, DP-12);
- the second select was seen on the real corpus, since the mock has no two-discipline department;
- the chosen strings ("Showing 1 of 2 flows.", the agent-task leaves);
- the bar's two groups;
- `InsideGroup.sets`;
- `useHeldWorkspaces` never fetches.

Iteration 2 fixed the looks' L1 and L2:
- the side block reads "Agent tasks · …", so it cannot clash with a department named Development;
- the Reviewer is prefilled with the parent's reviewer, else the acting name unless `agent` or `local`, which the daemon refuses (`acting.ts` `handOffReviewer`);
- the Tree's flatness comes from the answer, so a placeholder is drawn as read;
- a `ru` plural;
- `AGENT_TASK_GROUPS`, five prefixes.

**Churn** beyond the plan: core `workspace_toml.rs` line re-pins. `PINS` 0.35.0 is `6f31285e…` and `SCHEMA_PINS` `51c839c0…`, both set back red.

**Numbers.** Rust:
- nextest 5 372;
- 17 mutations red.

UI:
- 4 908 tests in 164 files, twice;
- lint 0, `tsc` clean;
- 14 mutations red.

DP-15:
- first-load JS 827.62 kB (+3.49 on 824.13, bar 845), gzip 236.90;
- `DocBody` 11.87 kB (gzip 3.75), lazy;
- `ru` 144.96 kB (gzip 33.41).

**Looks**, 2026-11-11, with a debug daemon on a scratch copy of a real game corpus and on studio-lamp:
- the Department select, and the Board's words in Russian;
- the Flow row shows the flow's title;
- Hand off's Department select drives four disciplines, and the Subtasks row reads "C-0003 … <flow title> · <column> · Handed off";
- `/tree?kinds=mechanic` gives 67 flat rows;
- Inside puts direction groups first, with one read;
- at 320 wide, in dark and in Russian, there is no overflow.

For RM-0099:
- Source shows an ID;
- direction rows repeat the kind chip;
- a Board with one flow shows its title nowhere visible;
- flat rows keep the chevron's indent.

| Module | What it does |
|---|---|
| core `card_toml.rs`, `workspace_toml.rs` | `title`, `kinds`, `disciplines_of` |
| CLI `workspace.rs`; MCP `mirror.rs`; `plugin/` | the `kinds` line; `title`; 0.35.0 |
| preset; `fixtures/{card-press,studio-lamp}` | titles, a commented direction; `scenes` |
| UI `i18n/`, `workspace/`, `tree/`, `flow/`, `doc/` | words, the bar, the flat list, sets |
| UI `board/`, `api/`, `mocks/`, `test/` | titles, Hand off, types, `mechanics` |
