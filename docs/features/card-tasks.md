---
class: spec
status: shipped
shipped: 2026-11-10
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures, docs]
ref: RM-0092, v4 step 3b (08 s2 Phase 7, "v4 scope"); the analyst's requirement analysis of 2026-11-09; the owner's decision 3 of 2026-11-08, Q9 of 2026-11-08 and Q2 of 2026-11-09; W1-W7 working answers, the owner's review open
adrs: [ADR-0075]
---

# card-tasks: the `T-` tasks behind a card (RM-0092, 3b)

## Why

The owner's decision 3 of 2026-11-08: an agent's task shows inside its card, with ADR-0056's `T-` model and the gate kept. A card and a task shared nothing: whoever opened a card could not see whether an agent had a task for it, whether a plan awaited the owner, or whether the spec had moved under it, and had to leave for Tasks, which the default workspaces no longer list (1a). So a card links tasks on its own row, behind queue schema step 10 and backup format 4, its reads serve each task's state read now, and the card modal and page show a Tasks block. The link decides nothing (ADR-0075). It is local only (Q2): a hub shows the IDs without state. Working answers W1-W7 (only `edit` links; another repository's task links but reads `null`; only `review` awaits the owner; no Edit-dialog field; no task on tiles or rows; no command on a hub; shell-safe targets else `REF`) await the owner's review. How it works now: `docs/canon/card-tasks.md`; the card's command, store and document: `docs/canon/cards.md`.

## Data

### Documents

The `show --json` and `get_card` document carries `tasks` after `evidence`; a task's notes follow the card's own, in link order. `fixtures/daemon-keys.json`: `Card` gains `"tasks"` after `"evidence"`; set 114 `CardTask` is `["id", "status", "title", "stale"]`. A task with no snapshot, before its approve, has `stale` `null` and `task list`'s note:

```json
"tasks": [
  {"id": "T-0001", "status": "review", "title": "Light the north lamp", "stale": null},
  {"id": "T-0002", "status": "in_progress", "title": "Wire the switch", "stale": true},
  {"id": "T-0003", "status": null, "title": null, "stale": null}
],
"notes": ["T-0001: stale unknown: no snapshot (the owner's `spec task approve` freezes one)",
          "T-0003: a task of another repository of the project `studio-lamp`; its state is read there (`spec task show T-0003`)"]
```

Every note case, the flag, the messages, the hub and the store: `docs/canon/card-tasks.md` "Link", "Read", "MCP and hub", "Store and backup".

## Acceptance criteria

Look = the orchestrator's dated check in Chrome. Each M named turned its test red.

- [x] **CT-1 Link.** CLI `card_tasks.rs` (`ct1_*`: order and one `card.edited`, refusals with a byte-equal dump, 32 stored and 33 refused, `T` in `[ids]`, a re-estimate takes no task); store `queue_cards.rs`; mcp `mcp_cards.rs`; http `cards.rs` (`card_tasks_ct1_an_edit_body_links_as_the_cli_does`). M1, M2.
- [x] **CT-2 Read.** `card_tasks.rs` (`ct2_*`: status and title read now, `stale` after an approve and an edit, a corrupt row, another repository, a deleted row, a root outside git, an unlinked card outside git); mcp `get_card` byte-equal to the library; http `cards.rs` byte-equal to `card show --history --json`; `daemon_keys.rs` (114 sets); `list` rows keep 21 keys. M3, M4.
- [x] **CT-3 The gate reads no link.** CLI `gate.rs` `card_tasks_ct3_the_gate_reads_no_link` (five faces, refused and passing, no card, linked, corrupt, byte-equal); store `the_gate_reads_tasks_alone`. M5.
- [x] **CT-4 Store and backup.** Store `queue_cards.rs` (9 -> 10, 8 -> 10), `queue_gate.rs` (11 newer), `queue_state.rs`; CLI `card_tasks.rs` `ct4_a_queue_of_schema_11_is_refused_naming_11_and_10`, `queue_state.rs` (format 4 round trip, format 3 import, a schema-9 DB as format 4, the four refusals). M6, M7.
- [x] **CT-5 The Tasks part.** `src/board/CardTasks.test.tsx`, `CardModal.test.tsx`, `CardPage.test.tsx`, `src/tasks/actions.test.ts`, `src/api/queries.test.tsx`, `daemonKeys.test.ts`, `i18n/catalog.test.ts`, `policy.test.ts`, `mocks/board.smoke.test.tsx`. M8, M9, M10. **Look, run 2026-11-10** by the orchestrator at 1440x900, a debug daemon on a scratch git copy of `studio-lamp` with a scratch `HOME` and a scratch UI build: C-0001's modal shows the block after Hand-offs; `T-0001` (review) its link, "Awaiting you", `spec task approve T-0001` with Copy and its no-snapshot note under `lang="en"`; `T-0002` In progress and Spec changed; `T-0003` (a corrupt row) no link, "State not read here", its note there and not in the top notes; opening reads no task. Planning `T-0004` in a terminal turned the open row to Plan review with no reload. C-0002's empty state shows both commands; the page shows the block; Russian chrome, English notes. Not in the record: the header rects across opening and the pasted approve's `[y/N]`.
- [x] **CT-6 It decides nothing.** `card_tasks.rs` `ct6_the_link_decides_nothing_on_twin_projects`. M11, M12.
- [x] **CT-7 Hub.** http `hub.rs` `card_tasks_ct7_a_hub_keeps_the_ids_as_written_and_reads_no_task`, `hub_mcp.rs` (both eras), mcp `mcp_hub.rs` (texts ASCII, at most 2 048 B, 17 tools). M13, M14.
- [x] **F Form.** Rust: clippy, rustfmt and `cargo check` clean; the full workspace run, 5 296 of 5 300 passed, the four (a `deploy_ops` format re-pin, three http UI tests racing a `ui/dist` rebuild) passing alone; `PINS`, `SCHEMA_PINS` 0.32.0 (`as_of_0_31_0`), `plugin_roles.rs`, `plugin_skills.rs`, `spec_ids_hub.rs`, `.env.example`; `read-spec` byte-equal. Eval `doc_pointers`, `ui_policy`, `anonymity`, `build_graph`, `workspace_names`. UI: `pnpm test` 3 771 in 143 files; `pnpm build` (below). The shard pins moved with the split. `spec check` 0 errors, 0 warnings.

## Out of scope

Making or approving a task from the UI (Q9); a Tasks field in Edit; a link on `new`; tasks on tiles, rows or `list --task`; state on a hub, hub-unique `T-` IDs; a task naming its card; any rule reading the link; dashboards (RM-0086); timing the read at scale.

## Implementation

Code review: ACCEPT with minors; every departure below accepted. Fixed after it: `CardEditRequest` derives no `Default`, `TASK_LINKS_SCHEMA` has one copy, `--task -` asks no `missing_task`, `queue.rs`'s Backup bullet names format 4, and the UI re-reads the open card on `proposal.applied`.

**D10, amended** (Rust departure 1): on a hub `--task` is checked by grammar, repeat and cap alone and kept as written, while `[ids]` claiming `T` exits 2 whenever `--task` is given, a config error that reads no task. Core's checks run before existence.

**Rust departures.** (1) Above. (2) Any failure to set up the task read is a per-task note `<id>: no state shown: <line>`. (3) Store calls beyond the spec: `read_task`, `missing_task`, `card_columns`, `list_existing_cards` reading a schema-9 queue. (4) `CardShowRequest`, `CardEditRequest` derive no `Default`; `CardEditBody::request(id, now, tasks_here)`. (5) A linked card's `show` refreshes the reading root's index once, as `task list`.

**UI departures.** (1) A task's notes show only on its row, not in the card's notes. (2) The hub sentence also stands under linked rows. (3) The empty state has a how-to line and each command's meaning. (4) "Awaiting you" is the command's meaning, no badge. (5) Reused keys `tasks.untitled`, `copyContext`, `copyFailed`. (6) No `CardEditBody.task` in the UI. (7) A `task.*` event invalidates every per-card read. (8) The mock adds `task list`'s no-snapshot note. (9) Files beyond the table: `test/builders.ts`, `mocks/MockClient.ts`, `mocks/tasks.ts`, `daemonKeys.test.ts`, `live.test.tsx`, the modal and page tests, `board.smoke`. (10) The test-engineer regenerated the daemon-keys fixture (114 sets, `Card` 31 keys).

**Mutations**, each red: Rust M1-M7, M11-M14 (M13, M14 on rebuilt binaries); UI M8-M10, two extras, and one for the `proposal.applied` re-read. **Size**: first-load JS 797.95 kB against 845 (794.35 after 3a); `ru-*.js` 138.61 kB.

| Module | What it does |
|---|---|
| model `card.rs`; core `card.rs` | `Card.tasks`, `CARD_TASKS_MAX`; `task_links_problem` |
| store `queue.rs`, `queue/{cards,state,tasks}.rs` | `STEP_10`, schema 10; the column, its decode, the edit's `COALESCE`; `CARD_COLUMNS` 25, `card_columns`; `read_task`, `missing_task` |
| cli `card.rs`, `main.rs` | `--task`, `tasks_here`, `git`; `tasks` in the document and lines; `CardEditBody.task` |
| cli `task.rs`, `proposals.rs`, `state_file.rs` | `LinkedTask`, `linked_tasks`; `repository_of`; format 4 |
| mcp `cards.rs`, `hub.rs`, `mirror.rs`; http `app.rs` | `edit_card.task`, the texts, `CardTaskShape`; `tasks_here` per server |
| `plugin/`, `scripts/deploy/.env.example`, `fixtures/daemon-keys.json` | 0.32.0; `CardTask` |
| ui `board/CardTasks.tsx`, `CardPanel.tsx`, `tasks/actions.ts` | the part, `ownNotes`; `awaitsOwner`, `approveCommand` |
| ui `api/`, `i18n/`, `mocks/`, `styles/` | `CardTask`, the re-reads; ten keys each; linked mock cards |
