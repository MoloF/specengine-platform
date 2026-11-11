---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0104, v5 step 6d (08 s2 Phase 7, "Owner's vision, 2026-11-11"); the analyst's requirement analysis of 2026-11-11 ("MANDATE", M1-M4, V1-V3); the orchestrator's working answers, the owner's review open
adrs: [ADR-0079]
---

# task-mandate: a subtask is binding, or its department decides (RM-0104)

## Why

The owner, 2026-11-11, translated: "a task that goes down must be approved, or stay open and be decided at the department's own discretion; or the writer sends down an approved task, and it must be refined or sent back to the writer for more work with questions." His vision asks for no shifting of responsibility (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"); his draft case is "a new female character", where the sound designer picks the voice and the rest follows the brief.

Before this slice, after `department-split` (RM-0100), a subtask said nothing about who decides; Return with a question and Record a decision… stood side by side with no sign of which one the writer expects; and the writer saw the departments' decisions only by opening each child.

This slice adds the mandate, named per subtask by the asking side: `binding` ("Binding") or `open` ("Department decides"). Only the reviewer changes it. It orders the work item's acts and frames two dialogs, and nothing else reads it. The parent gains "Decisions from subtasks", one read of its children's items. ADR-0079 supplements ADR-0056, ADR-0074 and ADR-0077. Then comes the owner's stop point.

## Description and interactions

Each decision is canon now, in `docs/canon/card-mandate.md` unless another file is named: D1 the field, D12 its words: "The field"; D2 named at creation, D3 only the reviewer: "The rule", "Messages", "Commands"; D4 per subtask: "Commands", `card-split.md` "Command", "Screen"; D5, D6 binding and open on the page: "Screen"; D7 Decisions from subtasks: "Store and backup", "Screen", `card-progress.md` "Screen"; D8 nothing reads it: "The rule"; D9, D10 schema 12 and format 6: "Store and backup", `queue-backup.md` "Format"; D11 documents and events: "Documents and events", `cards.md`; D13 no new surface: "Bodies and tools"; D14 plugin 0.38.0: `plugin-files.md`; D15 rollout: "Known limits".

Working answers taken (the owner did not answer): a mandate per subtask, `--mandate D=M` and a body's list of the same strings, no "same for all" control; `card.created` always carries it, `card.edited` only when it changes; the `list` suffix only when set; no clearing, the same value again an edit like any; `assign` leaves it; a split's new parent holds none, a stored one is never written; one argument struct for stdio and a hub, so stdio's `edit_card` always refuses it; `read-spec` unchanged; New task has no choice; Hand off and Edit reuse `SelectField` in first load; `subtask_items` serves every kind, the page shows decisions; Ask about it… raises on the child.

The owner's questions, open: **Q-A** the writer does not approve an open decision before it stands; **Q-B** per subtask, not per split; **Q-C** a binding task records a decision, never refuses it (ADR-0012). `department-split`'s Q-C, subtasks carrying the parent's title, stays open (`card-mandate.md` "Known limits").

## Data

In the canon: `card-mandate.md` "The field", "Messages", "Store and backup", "Commands", "Documents and events", "Bodies and tools", "Screen"; key sets 91, 92, 121 and 122 (`fixtures/daemon-keys.json`).

## Rules and edge cases

In the canon: `card-mandate.md` "The rule", "Known limits"; `card-flows.md` "The rule".

## Acceptance criteria

A browser look is marked **[look]**; a timed or counted run, **[measure]**. Every other criterion is a test that the named mutation turned red. New Rust tests live in `card_mandate.rs` of the crate named (core, store, CLI, http, mcp) unless another file is named; a scratch copy of `fixtures/studio-lamp` serves the CLI and http ones.

- [x] M1 — **Named at creation.** Core `Mandate::parse` takes `binding`, `open`, refuses `Binding`, ` open`, `-`, empty; `mandate_problem`. CLI: a hand-off `--reviewer "Ana Ruiz" --mandate open` stores `open` after `reviewer`, the two `card.created` carry `null` and `open`; `list` ends `| mandate open`, a card with none byte-equal to 8a43e5d's, 22 JSON keys; `--mandate binding` without `--reviewer` exit 1, nothing written, the next `new` `C-0003`; `--mandate maybe` exit 2. Store: no reviewer, `CardRefused`, no row or event. Corrupt `mandate = 'maybe'`: `show` exit 2 naming it, `list` a note. Mutations, each red: the store not asking `mandate_problem`; `card.created` carrying `mandate` only when set; the suffix printed for null.
- [x] M2 — **Per subtask in a split.** `--child art --child sound --mandate sound=open --mandate art=binding`: parent null, `art` binding, `sound` open, the three events `null`, `binding`, `open`, the children's `mandate` last and their text suffixes. Refused, exit 1, nothing written: `voice=open`, `art` twice, `art`, `Art=open`. A dry run plans `binding`, `seq` unchanged; `--mandate` without `--child` exit 2. An unnamed child `null`; a stored parent byte-equal. Mutations, each red: mandates paired by position; a child taking its neighbour's.
- [x] M3 — **Only the reviewer changes it.** The reviewer's edit: exit 0, `revision` 2, one `card.edited` `{id, fields: ["mandate"], as, mandate}`. Refused, exit 1, no event: another name, no `--as` (`local`), a title with it (unwritten). `--mandate -` exit 2. A title-only edit keeps `{id, fields, as}`; title and mandate, `["title", "mandate"]`. Reviewer cleared: the mandate stays, the no-reviewer line; a new reviewer's edit stands. A cancelled card, the closed line. Mutations, each red: the compare reading `assignee`; the check skipped beside another field; `card.edited` without `mandate`.
- [x] M4 — **Nothing reads it** (ADR-0012). Three hand-offs alike but the mandate, moved, returned, accepted, and three cancelled: `allowed`, `column`, `position` and documents equal; decisions without `why` and questions store on each; a `selective` gate's verdict unchanged; UI `progressOf` equal. Mutations, each red: `allowed` dropping a binding card's return; `create_intake` requiring `why` on an open card.
- [x] M5 — **Schema 12 and backup format 6.** Store: `user_version` 12, `table_info` equal to `CARD_COLUMNS` 26; `card_columns` 24, 25, 25, 26 for 9-12, `None` for 13; schema 11 steps on open, rows equal but `mandate`; 9 to 12 in one `migrate`; a hand-made column fails step 12 at 11; `open_existing` on 11 steps nothing. CLI `queue_state.rs`: the format-6 header, 26 columns, a round trip byte-identical; formats 5, 4, 3 import `NULL`, re-export as 6; the four refusals. Mutations, each red: `STATE_FORMAT` left 5; step 12 apart; a format-5 row read as 26 columns.
- [x] M6 — **Decisions from subtasks, one read.** Store `subtask_items`: children's items by ID, a cancelled child's, no parent's own or grandchild's, a corrupt one `unreadable`. CLI `show --json` in set 122's order, `[]` with no children, the text lines; `GET .../cards/C-0001` and `get_card` equal, valid against its schema; `list` byte-equal before and after. Mutations, each red: grandchildren taken; a read per child.
- [x] M7 — **The surfaces.** http local: create 200, no reviewer 409, `maybe` 400; edit 200 for the reviewer, 409 for another; split 200, `voice=open` 409; `CARD_CALLS` 12. A hub: any login creates and splits; the edit 200 for the reviewer's login, 409 another, an `as` naming another 403. mcp: three schemas, `additionalProperties: false`; stdio stores a mandate as `agent`, its `edit_card` refused naming `agent`; a hub's acting as the login; texts ASCII within 2 048 B. `daemon_keys.rs` 122. Mutations, each red: `mandate` missing from `CardEditBody`; the hub's edit as `local`; `CardSubtaskItemShape` without `card_id`.
- [x] R1 — **Rollout on today's code.** Recorded on a pristine export of 8a43e5d: the gate's no verdict at schema 12, `stage_confirm.rs`, `card_tasks.rs`, `drift_raise.rs` exit 2, `queue_state.rs` format 6 refused, store `SchemaTooNew`, http `stage.rs` 503; the same moved to 13 and 7 green here. Manual: `--mandate` an unexpected argument, stdio `create_card` an unknown field `mandate`. Mutation, red: the gate's newer-schema branch giving a verdict.
- [x] R2 — **The hub's upgrade, dump first.** A format-5 dump with two cards and an item imports into schema 12 with `mandate` `NULL`, re-exports as 6, re-imports byte-identical; the way back pinned by R1; the steps in `card-mandate.md` "Known limits". Mutation, red: the format-5 reader taking a 26th column.
- [x] P1 — **Plugin 0.38.0.** `PINS`, `SCHEMA_PINS` end at 0.38.0, the schemas' hash moved; `spec_ids_hub.rs` reads 0.38.0; `read-spec` byte-equal to 0.37.0's. Mutations, each red: `plugin.json` at 0.37.0; `SCHEMA_PINS` not appended.
- [x] V1 — **The choice.** Hand off: the select after Reviewer at "Choose…", none gives `board.mandate.needed`, nothing posted; `mandate: "open"` sent; a hub the same. Split: a select per checked row, Split off until each has one, `["art=binding", "sound=open"]` in row order, an unchecked row's dropped, one plan read. Edit: the reviewer's select preset, another name `reviewerOnly`; an unchanged value sends none. New task none. Mutations, each red: a default `binding`; an unchecked row's mandate sent.
- [x] V2 — **The work item.** Binding: Return… and Ask a question… right after Accept; open: Record a decision… there; null: today's acts element for element; the note, Details' row, Activity's "mandate Binding"; Return's question open on C-0013 only; Record's binding note and `why: null`, open's required why. `policy.test.ts`: `mandate ===`, `!==` only in `labels.ts`. Mutations, each red: the order compared in `CardPanel.tsx`; why required on a binding card.
- [x] V3 — **Decisions from subtasks.** `SubtaskDecisions.test.tsx` on C-0001: the part after Subtasks, two decisions in served order, `CardLink`s, the departure and Department decides lozenges, `ItemBlock`; Ask about it… on C-0019 prefilled, posting to its items; none without a decision; one `getCard` and one children read; after `pnpm build` in the items' chunk. Mutations, each red: a `getCard` per child; the part listing questions.
- [x] V4 — **[look]** 2026-11-12: see "Implementation", Looks.
- [x] V5 — **[measure] Size.** First-load JS at most 845 kB: see "Implementation", Numbers.
- [x] G1 — **Gates.** UI lint 0, `tsc` clean, the full run twice; Rust the workspace run, then `doc_pointers`, `anonymity`, `ui_policy`, `workspace_names`; the check clean.

## Out of scope

- The writer approving or confirming a department's decision (ADR-0035, ADR-0061); any refusal, wait or status by mandate (ADR-0012).
- A mandate on New task, on the Board's tile, in `list` filters, or as a `[card]` default; clearing one; one per split; grandchildren's items.
- Notifications; department Overview counts (RM-0086); a subtask told its parent changed (RM-0105); the live hub's upgrade, the owner's.

## Implementation

Both reviews ACCEPT, departures accepted, nits applied.

| Module | Does |
|---|---|
| model `card.rs`, `lib.rs` | `Mandate` (`ALL`, `as_str`, `parse`), `Card.mandate`, re-exported |
| core `card.rs` | `mandate_problem`, `mandate_change_problem`, `split_mandates`, `MANDATE_ENTRY_ECHO_MAX` |
| store `queue.rs`, `queue/{cards,state}.rs` | schema 12, `STEP_12`, `MANDATE_SCHEMA`; `CARD_COLUMNS` 26, `card_columns`; the checks under the lock; `subtask_items` |
| cli `card.rs`, `main.rs`, `state_file.rs`, `lib.rs` | the three flags, bodies, documents, text; `STATE_FORMAT` 6 |
| mcp `cards.rs`, `hub.rs`, `mirror.rs` | `MandateArg`, the texts; `CardMandateShape`, `CardSubtaskItemShape` |
| `plugin/`, `scripts/deploy/.env.example` | 0.38.0 |
| ui `api/`, `board/`, `i18n/`, `mocks/`, `ui/Icon.tsx`, `styles/app.css` | the type, lookups, choice, framing, `SubtaskDecisions`, words, mock, icons |

**Rust departures.** (1) No `JsonSchema` on the model's `Mandate` (no new dependency): MCP wraps it as `MandateArg` with a hand-written schema, enum `binding`, `open`, null. (2) Split usage puts the mandate line first. (3) The tools' header lines do not list `--mandate`. (4) A `--mandate` entry is echoed on one line, cut at 72 B. `tag-check.sh` holds no literal and is unchanged.

**UI departures** (1-9, and iteration 2): Russian words set by the orchestrator, the field "who decides", binding "the one who set the task", open "the department"; `board.split.mandateForDiscipline` when a row's discipline differs from its flow; `asksOnReturn`, `mandateMark`, `mandateWords` beside the lookups; the binding icon an anchor, open a fork; the non-reviewer line its own; "Choose…" following the value at open; Answer… and Ask about it… off while a card write is out; the words guard over verdict and mark words and the field's own word.

**Numbers.** Rust: `card_mandate.rs` core 7, store 12, CLI 18, http 6, mcp 4, CLI `queue_state.rs` two more; full run 5 523, two failures green alone (a re-pin, a 3 s load flake); 22 mutations red; daemon keys 122; `PINS` 0.38.0 `af7a7419…`, `SCHEMA_PINS` `57a26d3e…`. The churn went far past "Test churn", about 35 files. UI: 5 198 tests in 177 files, green twice; 17 mutations red; first load 823.45 kB (gzip 237.96) from 818.68, `CardItems` 11.95 kB, `SplitDialog` 8.02 kB, `ru` 152.30 kB.

**Settled.** R2's format-5 dump came from a format-6 unmigrated export, byte-equal to 8a43e5d's own; M4 compared per copy; M3's `revision` 2 holds without the assign step.

**Looks**, 2026-11-12, a scratch copy of a real game corpus, light, Russian, 1440: a CLI split with a binding and an open subtask; the open one's acts led by Record a decision…, its note, the Details row with the fork and the department's word, the Activity word; a decision with its why; the parent's Decisions from subtasks with flow, lozenge, the item, Answer… and Ask about it…. Light at 1440 only; dark and 320 by earlier looks of the dialog and the work item, the rest by the mock tests.

**Known limits**: `card-mandate.md`.
