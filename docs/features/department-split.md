---
class: spec
status: shipped
shipped: 2026-11-11
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, ui, plugin, scripts, fixtures]
ref: RM-0100 re-scoped, v5 step 6c (08 s2 Phase 7, "Owner's vision, 2026-11-11"); the analyst's requirement analysis of 2026-11-11 ("SPLIT", S1-S9, U1-U3); the orchestrator's working answers, the owner's review open
adrs: [ADR-0078]
---

# department-split: a task and a subtask per department, in one act (RM-0100)

## Why

The owner's target workflow of 2026-11-11 (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"): a writer describes the game; it is split into subtasks per department; questions go back up; each subtask is binding, or its department decides. His draft case is "a new female character": narrative, art and sound each get a subtask, and the sound designer picks the voice.

Before it, a split took a New task and then one Hand off per department, each its own write and dialog, so a forgotten department left a hole nobody saw; no config tied a kind to the departments it needs; each hand-off targeted what the person typed; and an agent could make one card a call.

This slice makes the split one act: a parent task and one subtask per chosen discipline, all or nothing, from the document, Flow's panel or a task, by the CLI, MCP and the daemon. `[[card.split]]` rows say which disciplines a kind usually needs and which nodes each subtask targets; they only pre-select. ADR-0078 supplements ADR-0056, ADR-0074 and ADR-0076. The mandate (binding, or the department decides) is the next slice, RM-0104 `task-mandate`.

## Description and interactions

Each decision is canon now, in `docs/canon/card-split.md` unless another file is named: D1 one act, D2 all or none, D3 the reviewer: "The rule", "Command"; D4 templates: "Config"; D5 the plan: "The plan"; D6 `--dry-run`: "Command"; D7 the route and a hub's seventh plain write: "Route and hub" (`board-ui.md`, `hub-team.md` "Acts"); D8 `split_card`, 30 and 18 tools: "MCP" (`hub-mcp.md` "Tools"); D9, D10 the word and the dialog: "Screen"; D11 the preset's `sound` flow and department, its commented rows: `workspaces.md` "Preset"; D12 no storage change: "Command"; D13 no cascade: `card-flows.md` "The rule"; D14 plugin 0.37.0: `plugin-files.md`; D15 rollout: "Known limits".

Working answers taken (the owner did not answer): rows under `[card]`, never a top-level key; a child targets its rows' kinds' nodes, else the parent's targets; a missing parent is a 409; the reviewer chain given, stored parent's, acting name; a dry run judges every rule but the persons and, with no `--child`, plans every discipline; a child takes the parent's title and goal alone; the scope is each target and its tree rows to depth 2; past 64 nodes, the first 64 with a note; a row's unlisted discipline is a config error, a kind has one row; a stored parent in any state is taken; `card.created` unchanged; no dedup; at most 32 children and 64 rows.

The owner's questions, open: **Q-A** rows name disciplines, not departments; **Q-B** subtasks wait on nothing (ADR-0074, ADR-0012); **Q-C** a subtask carries the parent's title (the look found two subtasks reading alike on the Board apart from their discipline).

## Data

In the canon: `card-split.md` "Config", "The plan", "Command", "The document", "Route and hub", "MCP"; the preset `workspaces.md` "Preset"; key sets 119-121 `CardSplit`, `CardSplitParent`, `CardSplitChild` (`fixtures/daemon-keys.json`).

## Rules and edge cases

In the canon: `card-split.md` "The rule", "Known limits"; `card-flows.md` "The rule".

## Acceptance criteria

A browser look is marked **[look]**; a timed or counted run, **[measure]**. Every other criterion is a test that the named mutation turned red. New Rust tests live in `card_split.rs` of the crate named (core, store, CLI, http, mcp), unless another file is named; scratch copies of `fixtures/card-press` (two live rows) and `fixtures/studio-lamp` (the preset) serve them.

- [x] S1 — **Config.** Core: `card-press` reads `splits` `[{broadsheet, [copy, plates]}, {print-run, [plates]}]`; no key, `splits` empty and every flow as before; each message of `card-split.md` "Config" at its line, a row naming `voice` (no flow lists it) the listing error; the preset with its rows uncommented reads six flows and two rows. CLI: `spec card list` on the `voice` copy exits 2 with that line; `spec show PRN-WEEKLY`, `spec check` as before. Mutations, each red: the listing check dropped; a kind compared trimmed; `split` judged before every flow reads.
- [x] S2 — **The plan.** `split_plan`: scope `[(A, scene), (A-VO, voice), (A-X, None), (B, voice)]`, rows `scene: [narrative, sound]`, `voice: [sound]`, chosen none: `suggested` `[narrative, sound]`, `sound` `[A, A-VO, B]` and `narrative` `[A]` by `kinds`, a third flow's `art` the parent's targets, not suggested; `Scene` matches no `scene`, `None` none; 70 nodes give 64 and the note; chosen `[art, sound]` keeps its order; `[]`, a 33rd, a repeat, `nowhere` each the canon's line. Mutations, each red: kinds case-insensitive; `suggested` in discipline order; a kindless child given `[]`; the cap at 65.
- [x] S3 — **A split from a document.** CLI, `card-press`: `spec card split --target PRN-WEEKLY --title "The weekly run" --discipline copy --child copy --child plates --reviewer "Ana Ruiz" --as "Ana Ruiz" --json` exits 0: `C-0001` `copy/draft` on `[PRN-WEEKLY]`; `C-0002` `copy/draft` (`SHT-BACK`, `SHT-FRONT`, `SHT-INNER`, no `PRN-`) and `C-0003` `plates/queued`, each `parent` `C-0001`, reviewer Ana Ruiz, the parent's title and goal; `list --parent C-0001` lists both, each with a snapshot per target; exactly three `card.created`, `C-0001` first, `as` Ana Ruiz; the text the canon's lines. Mutations, each red: children before the parent; a child's reviewer null; its `goal` dropped.
- [x] S4 — **Under a stored task.** `split --parent C-0001 --child plates`: one child; `C-0001`'s row, `revision`, `updated_at` byte-equal, no event naming it, `created` false; `--parent C-0001 --title X` exit 2; `--parent C-0099` exit 1, nothing written, the next `new` `C-0002`; a Cyrillic look-alike exit 2 naming `C-0001`; no reviewer anywhere, exit 1, `` reviewer: `local` can be no reviewer… ``. Mutations, each red: the parent's `revision` raised; the chain skipping its reviewer.
- [x] S5 — **All or none.** Store: a second child's reviewer `agent` refuses the split, no row, no event, `create_card` next `C-0001`; `Stored("C-0099")` refused, nothing written. CLI: `--child nowhere`, `--child copy --child copy` exit 1 with their lines. Mutations, each red: each card committed alone; the children's `parent` from the input.
- [x] S6 — **Dry run.** `--dry-run --json`: exit 0, every `id` null, `copy` then `plates` suggested, no row, no event, the last `seq` unchanged; on `STY-0001` none suggested, `from` `parent`; `--reviewer agent --dry-run` exit 0, without `--dry-run` exit 1; `--child nowhere` exit 1; a bad `[card]` exit 2. Mutations, each red: a dry run storing; a dry run judging the reviewer.
- [x] S7 — **The route and the hub.** S3 as a body, 200; a dry run 200, nothing stored; `nowhere` 409, the reason last; an unknown key or bad `as` 400; a bad `[card]` 503; GET 405 `Allow: POST`; `GET .../cards/C-0001` still the card. `door.rs`: `CARD_CALLS` 12; a split adds three `card.created`, a dry run none. A hub: a login outside `[team]` splits, `as` absent records it on every event, `as` another 403, nothing stored; `class` `Plain` for POST, `None` for DELETE. Mutations, each red: the arm left out of `class`; `acting` not called.
- [x] S8 — **MCP.** Stdio lists 30 with `split_card`, `additionalProperties: false`; no reviewer on a new parent, an error result, nothing stored; a person reviewer, stored, `as` `agent`; `dry_run`, the plan; an unknown key refused. A hub lists 18, its call records the login. Both texts and both instructions ASCII, at most 2 048 B, the determinism sentence, naming no tool left out; `INSTRUCTIONS` and `HUB_INSTRUCTIONS` name `split_card` (departure 1). Mutations, each red: `split_card` missing from `HUB_TOOLS` (refused by the build); the stdio tool acting as `local`.
- [x] S9 — **The preset, fixtures and guards.** The preset's `sound` flow, `sound` workspace before `production`, header sentence and commented rows verbatim, `studio-lamp`'s copy byte-equal, `spec workspace --json` five workspaces, `sound` serving `[sound]`; `no_domain.rs` takes the rows' `disciplines` as words (a planted `"plates"` red) and scans `kind = "…"` on live lines alone; `workspace_names.rs`, `anonymity.rs` pass; an unknown `[card]` key fails `spec card list` alone. Mutations, each red: the header sentence dropped; the rows' disciplines unread.
- [x] K1 — **No cascade.** After S3, moving, accepting and cancelling `C-0002` changes no other row; cancelling `C-0001` leaves both children open; `C-0001` is accepted with both open; accepting both children moves no parent. Mutation, red: a split raising the stored parent's `revision`, or an accept refusing a parent with open children.
- [x] P1 — **Plugin 0.37.0.** `PINS`, `SCHEMA_PINS` end at 0.37.0, the schemas' hash moved; `spec_ids_hub.rs`, `tag-check.sh` read 0.37.0; `read-spec` within 4 096 B naming `split_card`. Mutations, each red: `plugin.json` at 0.36.0; `split_card` dropped from `read-spec`.
- [x] U1 — **Openers and the chunk.** The act after New task in the document modal and page, after New task on Flow's rooted scope and a box, after Hand off on an open card only; pressing opens the dialog with its source; after `pnpm build` `SplitDialog` is its own chunk, outside first load and every `modulepreload`. Mutation, red: a static `import { SplitDialog }` in `CardActs.tsx`.
- [x] U2 — **The dialog.** `src/board/SplitDialog.test.tsx` on the mock: one plan read on open, `dry_run: true`, `child: null`; rows under their departments, suggested ones checked, the `parent` row "The task's targets"; from a document Title prefilled, Discipline the department's first, the document's tasks listed; from a task no Title, Goal or Discipline; Submit off with none checked; toggling reads nothing; Split posts the checked disciplines in row order; 200 closes with the line and link; the conflict 409 keeps the fields; a hub login's Reviewer read-only. Mutations, each red: every suggested row sent whatever is checked; a plan read per toggle.
- [x] U3 — **[look]** 2026-11-11: see "Implementation", Looks.
- [x] U4 — **[measure] Size.** First-load JS at most 845 kB: see "Implementation", Numbers.
- [x] R1 — **[measure] A real corpus.** A dry run under 500 ms: see "Implementation", Looks.
- [x] G1 — **Gates.** UI lint 0, `tsc` clean, the full run; Rust the workspace run, then `cargo nextest run -p specengine-eval --test doc_pointers --test anonymity --test ui_policy --test workspace_names`; the check clean.

## Out of scope

- The mandate: RM-0104 `task-mandate`, the next slice. The old item's soft `after`, any order or wait between subtasks.
- Grandchildren in one act; a template naming a department or matched by anything but a kind; editing rows from the page; editing a child's targets, title or goal in the dialog.
- Dedup; notifications; department Overview counts (RM-0086); a split across two stores.

## Implementation

Both reviews ACCEPT, every departure accepted; four refused splits left no card, event or ID live.

| Module | Does |
|---|---|
| core `card_toml.rs`, `card_split.rs` (new) | `[[card.split]]`, `CardSplit`, `MAX_SPLITS`, `MAX_SPLIT_DISCIPLINES`; `split_plan`, `ScopeNode`, `TargetsFrom`, `PlannedChild`, `MAX_SPLIT_CHILDREN` |
| store `queue/cards.rs` | `split_cards`, `SplitParent`, `NewSplit`, `SplitCards`, one transaction |
| cli `card.rs`, `main.rs`, `lib.rs`, `tree.rs` | `card_split`, `CardSplitBody`; `card split`; `SPLIT_DRY_RUN_FLAG`; `walked` shared |
| http `app.rs`, `hub.rs`, `answer.rs` | the route, `split_card`; `class`'s plain arm |
| mcp `cards.rs`, `hub.rs`, `mirror.rs`, `server.rs` | `split_card`, its two texts (1 655 / 1 670 B), the shapes; both instructions name it (`HUB_INSTRUCTIONS` 1 997 B) |
| `plugin/`, `scripts/` | 0.37.0, `read-spec` 4 086 B; the preset's `sound`, rows commented |
| ui `board/SplitDialog.tsx`, `api/splitQueries.ts` (new), `board/`, `doc/`, `flow/`, `api/`, `i18n/`, `mocks/`, `workspace/` | the dialog, openers, words; `splitCard`; `useSplitPlan`, `useCardSplit`, `useHeldTrace`, the dialog's alone |

**Rust departures.** (1) Stdio `INSTRUCTIONS` and `HUB_INSTRUCTIONS` name `split_card`; stdio stays at 2 048 B by dropping `search`'s `limit?, archive?` from its reads line (still in the schema). (2) A 33rd discipline reads `` `disciplines` lists <n>; a row lists 1 to 32 ``. (3) `--dry-run` is named by `SPLIT_DRY_RUN_FLAG` (`proposal_genre`'s guard). (4) A dry run's stored parent has `id` null. (5) A refusal before the plan carries the reason alone. (6) `SplitParent::New {card: Box<NewCard>, snapshot}`, so no lint allow is needed. (7) Small choices: a row's table errors at its header; ID'd sections in scope.

**UI departures** (1-9): openers in `FlowPanel`, `CardPanel`; the plan held; `storedWorkspace` the default department; mock rows `mechanic`, `rule`. Iteration 2: the document's tasks by `heldTraceRef`, `DocActs` alone; `aria-haspopup="dialog"` on every dialog act.

**Numbers.** Rust: `card_split.rs` core 18, store 5, CLI 15, http 7, mcp 5; full run 5 474; about 26 mutations red; daemon keys 121; `PINS` 0.37.0 `3eff9086…`, `SCHEMA_PINS` `def093f6…`, stdio `PLANNED_BLAKE3` `7de2fdaa…`. UI: 5 130 tests in 174 files, green twice; nine more mutations red; first load 818.68 kB (gzip 236.59) from 815.45, `SplitDialog` 7.30 kB (gzip 2.66) lazy, `ru` 150.78 kB.

**Looks**, 2026-11-11, a scratch copy of a real game corpus with two rows. R1: a CLI dry run on one document of 8 sections of a kind, 0.25-0.39 s wall: two disciplines by kind, the rest the task's targets; no follow-up. U3, light, 1440, Russian: the act and its chunk from the document modal, rows by department, two pre-checked, a split making a parent and two subtasks on the kind's nodes, the polite line; on the mock, dark, 320x700, the conflict: the dialog fills the screen, no overflow, the refusal's words in the alert, the fields kept. No Network panel.

**Known limits**: `card-split.md`.
