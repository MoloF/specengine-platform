---
class: spec
status: shipped
shipped: 2026-11-11
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, ui, plugin, scripts, fixtures]
ref: RM-0097, v5 step 5 (08 s2 Phase 7, "v5 scope"); the owner's v5 Q5 and Q11 of 2026-11-10; the analyst's requirement analysis of 2026-11-11; the orchestrator's working answers Q1-Q5, the owner's review open
adrs: [ADR-0077]
---

# card-questions: a task asks, and the answer is a recorded decision (RM-0097)

## Why

The owner's pipeline of 2026-11-10 (`08-roadmap.md` Phase 7, "v5 scope"): development sends a task back with a question; the writers' answer becomes a decision record, or a document that proves the work; a developer who decides alone says so; who decided what, and why, stays visible. His Q5: people raise questions and decisions from a task in the UI. His Q11: a decision record names no card, and the card links back to it.

Before this slice:
- nothing tied a question, discrepancy or decision to a card;
- a decision said what, never why;
- a return kept one reason, in the move's event;
- the page raised nothing: only a hub's `/mcp` and a departing accept raised proposals through the daemon.

How it works now: `docs/canon/card-questions.md`, whose "The rule" is ADR-0077's canon (it supplements ADR-0056, ADR-0057, ADR-0074 and ADR-0075); `architecture.md#ui` widened by a card's questions and decisions.

## Description and interactions

Each decision is canon now. The numbers stay because code and tests cite them; the sections named are `card-questions.md`'s unless another file is.

- **D1** The link: "The rule", "Store". **D2** A decision's `why`: "Store", "Record".
- **D3** Queue schema 11, backup format 5: "Store", `proposal-queue.md` "Store", `queue-backup.md` "Format".
- **D4** `items` and the merged `history`: "Read", `cards.md` "Documents and events".
- **D5** `--card`, MCP `card_id` and `why`: "Raise", `agent-intake.md` "Tools", `proposal-decision.md` "Raise".
- **D6** The card door, `card_raise`: "The card door", `board-ui.md` "Routes", "Fence and trust".
- **D7** A hub's sixth plain write: "The card door", `hub-team.md` "Acts", `hub.md` "Routes".
- **D8** Answering is staging; the stage's `as`: "Stage", `decision-staging.md` "Daemon", `hub-team.md` "Stages".
- **D9** The lifecycle: "Store". **D10** The link decides nothing: "The rule", `cards.md` "Rules".
- **D11** Return can ask: "Screen", `card-flows.md` "Return". **D12** Activity: "Screen", `ui-design.md` "The work item".
- **D13** Ask a question…, Record a decision…, Answer…, the Inbox link: "Screen", `board-screen.md` "Writes and dialogs".
- **D14** The review document: "Store", `proposal-review.md` "Review document".
- **D15** Plugin 0.36.0: `plugin-files.md`, `plugin.md` "Skills". **D16** Rollout: "Known limits".

Working answers taken: the orchestrator's Q1 (`{{why}}` opt-in), Q2 (stages send `as`), Q3 (locally an answer ends at a stage), Q4 (Return's question needs a working answer and its price), Q5 (an answer is a stage); the spec-writer's W1-W10 (`decision_note` a 16th key; `{{why}}` empty under `--answer`; `card_raise`, `raise_on_card`, a missing card a 404 first; no `proposed_patch`, `task_id` or author on the route; `as` on both stage routes; no index; blocks at their `proposal.created`, "Earlier items"; Ask anyway names the listed hits; Return asks about the card's targets; the two acts on any card with a target). The owner's Q-A (the Questions screen's `as`: RM-0099), Q-B (an answered question moves no card) and Q-C (no addressee in `v5`) stay open.

## Data

`fixtures/daemon-keys.json` holds 118 sets: 3 `Proposal` 50 keys (`card_id`, `why` after `interpretation`); 92 `Card` 32 (`items` after `tasks`); 116 `CardItem` `[id, kind, status, summary, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title]`; 117 `IntakeDocument` `[id, created, hits, related, linked, diagnostics, notes]`; 118 `IntakeMatch` `[id, source, status, path, answer, record]`. MCP description bytes, stdio/hub, of 2 048: `ask_question` 2 033/2 030, `report_decision` 1 810/1 758, `report_discrepancy` 1 736/1 714; `get_card` 1 411, `get_proposal` 1 150. `ask-owner`'s body 4 095 B by the canon's measure (the file 4 374). Catalog keys: `board.panel.activity` (was `board.panel.history`), `board.activity.*`, `board.items.*`, `board.ask.*`, `board.return.ask`, `board.decide.*`, `board.answer.*`, `inbox.proposal.task`, `inbox.proposal.why`.

## Acceptance criteria

A **[look]** and a **[measure]** are recorded under "Implementation". Every other criterion is a test that its named mutation turned red; new Rust tests are `card_questions.rs` of the crate named.

- [x] S1 — **Schema step 11.** Store: a new queue at `user_version` 11, `table_info(proposals)` equal to `PROPOSAL_COLUMNS` 48, `card_id`, `why` last; `proposal_columns` 48 for 11, 46 for 8-10, `None` for 12. A schema-10 DB with a row of each kind and a card steps to 11 at its first `open`, rows byte-equal but the two `NULL`; schema 9 reaches 11 in one `migrate`; a hand-made `card_id` column fails step 11 and the DB stays at 9; `open_existing` on schema 10 reads both `None`, steps nothing. M: each step committed alone; `why` before `card_id`.
- [x] S2 — **Raise with a card.** CLI on `studio-lamp`: `--card C-0001` stores a question, a decision and a discrepancy with `card_id`; accepted and cancelled cards take items; `C-0099` exit 1, `id` null, the note, no event, the next raise the next free ID; `C-07` exit 1; a look-alike exit 2; `[ids]` claiming `C` exit 2 with `--card` only; store `CardRefused`, nothing written. M: the existence check after the insert; a closed card refused.
- [x] S3 — **`why` and the slot.** Core: blank, then 2 049 bytes refused, 2 048 read, a blank `why` beside a bad patch is `why`'s refusal. CLI: a decision's `why` stored and printed, a question's `null`, a discrepancy's `--input` holding it exit 2. On `spec-a` with `{{why}}`: approving with no flag writes the why; `--answer` and a question leave it empty; `{{why}}` in the front-matter exit 2. Store: `Intake.why` on a question `Invalid`, a question row with `why` corrupt. M: `why` filled under `--answer`; `why` accepted on a question.
- [x] S4 — **Lifecycle and corrupt rows.** `card_id` on an update, `X-7` on a question: `review` exit 2, `inbox` skips with a note. A departing accept's discrepancy names its card; approve, reject, stage, unstage and reopen keep `card_id`; `card new --corrects C` gives `items: []`; a cancelled card keeps its items. M: `departure_intake` without the card; a correction listing its original's items.
- [x] S5 — **Backup format 5.** CLI `queue_state.rs`: the eight-key header, 48 columns; a carded question and a decision with `why` round-trip byte-identical; a format-4 dump imports both `NULL` and re-exports as 5; refused, exit 2: a format-5 row without `card_id`, a format-4 row with it, format 5 at schema 10, format 6. M: `STATE_FORMAT` left 4; a format-4 row taking the new columns.
- [x] K1 — **The card read, deciding nothing.** CLI and http `cards.rs`: `items` after `tasks` in set 116's order, by ID number, `record_*` and `decision_note` as stored, `[]` with none, the same outside a worktree; `--history` with 70 moves and 70 item events gives 128 entries merged by `seq`, a linked update's absent; the text lines; a corrupt item left out with a note, exit 0; `list` and `GET .../cards` byte-equal before and after; beside an open question and decision a card moves, returns, is assigned, accepted and cancelled as its twin without, `spec gate` the same, and a question whose card was cancelled is approved and rejected as one without. `GET .../cards/C-0001` byte-equal to `show --history --json`. M: `card_move` refusing while an item is open; the two lists capped together at 64.
- [x] L1 — **CLI surface.** The three `--help`s list `--card C`; `review --json`'s keys end `interpretation, card_id, why, notes`; `inbox` lists a carded item as its twin. M: the review without `card_id`.
- [x] L2 — **MCP, stdio.** The three input schemas hold `card_id` `["string","null"]`, `report_decision` `why`, `additionalProperties: false` (presence and type: parsed JSON shows no key order); a call stores both, `structuredContent` equals the CLI's; `get_proposal`, `get_card` serve them, valid against `outputSchema`; the texts at "Data"'s bytes. M: `CardItemShape` without `decision_note`; the question text untrimmed.
- [x] L3 — **The card door.** http: a question with `as: "dev"` 200, the human author, the event `{"id":"PR-0001","as":"dev"}`, `as: null` `"as":"local"`; a decision's `why` stored; `C-0099` 404, no event; a repeat 200 `created` false, then stored with `distinct_from`; a cap 409; an unknown key, `kind: "update"`, `why` on a question 400; a look-alike 503, non-JSON 415, no same-origin 403, GET 405. `door.rs`: `CARD_CALLS` 11, `card_ask` caught, `propose*` forbidden; a raise adds one `proposal.created`, nothing else. M: the handler calling `propose_question`; the event without `as`; the card not read first.
- [x] L4 — **The stage's `as`.** A local stage with `as: "ana"` logs `proposal.staged` ending `"as":"ana"`; without it exactly `{"id","staged","staged_at"}`; a round answer's stage takes `as`; an off-grammar `as` 400; `spec approve` confirms as before. M: `"as":null` written; `as` in a column.
- [x] M1 — **Rollout on today's code.** Recorded at a62c81c: CLI `stage_confirm.rs` `ac02`, `gate.rs` `ac06`, `queue_state.rs` `ac06`, store `open_existing_refuses_a_newer_schema`, http `stage.rs` `ac02`, then moved to 12 and format 6. New, green before and after: CLI `m1_an_unknown_template_slot_exits_2_at_its_line` (stderr starting `` spec: `PR-0001` not applied (step 3): templates/decision.md:N: ``), `m1_an_unknown_decision_input_key_exits_2`, MCP `m1_an_unknown_argument_fails_report_decision`. M: the slot's line taken from the front-matter.
- [x] H1 — **A hub's sixth plain write.** `Remote-User: eve` outside `[team]`: 200, `author.login` and the event's `as` `eve`; another `as` 403; `class` Plain for `.../items`, `None` for `.../items/x` and an encoded `items`; the step-up regex unchanged. M: `items` classed Team; `acting` skipped.
- [x] H2 — **A hub's stage under the login.** A `member`'s stage ends `"as":"tomas"`; another name 403; outside `[team]` 403. M: the body's `as` over the login.
- [x] H3 — **A hub's `/mcp`.** `ask_question` with `card_id` stores it with `author.login`; `report_decision` takes `why`, no `task_id`; hub texts ASCII at "Data"'s bytes. M: `HubQuestionArgs` without `card_id`.
- [x] H4 — **Answered on a hub.** A carded decision with `why`, staged by a `member`, confirmed by a covering `lead`, commits its record with the why; the card lists it `applied` with `record_id`, its `history` ending `proposal.approved`, `.applied`. M: the record rendered without the slot.
- [x] U1 — **Activity.** `CardActivity.test.tsx`, `workItem.test.tsx` on harbor-sim's `C-0008`: Activity where History stood, served order, the return's note; `PR-0055`'s block (`question` raw under `lang="en"`, lozenge, summary, working answer, "by Noor Ainsley"); `PR-0056`'s why, `DocLink` and `decided_by`; "Earlier items"; an unknown type raw. M: entries sorted by `at`; the block chosen by `kind`.
- [x] U2 — **Return with a question.** `ReturnDialog.test.tsx`: the closed part on a card with targets, none without; the move, then the raise; a refused move sends no raise; not stored keeps the dialog, Ask anyway posts the raise alone naming the hit; status 0 offers Ask again; all hits named: the too-many line; closed, the move byte-equal to before. M: the raise first; Ask anyway resending the move.
- [x] U3 — **Ask a question…, Record a decision….** `CardAsk.test.tsx`: both after Return… on a card with a target, open or closed; `kind` question and decision, `why` or `null`; hits as U2; a 400 or 409 in the alert. M: Record sending `why: ""`.
- [x] U4 — **Answer….** `CardAnswer.test.tsx`: one `getProposal`, the three choices by structure, the stage with `updated_at` and `as`, then Staged and the command with Copy, no Confirm locally; a hub's Confirm with `confirm` alone; off outside `[team]`; none on an applied item; a 409 keeps the dialog. M: Copy for an ID off `^PR-[0-9]{4,}$`; Answer… on outside `[team]`.
- [x] U5 — **The Inbox link, words, policy.** `ProposalCard.test.tsx`: "Task C-0008" linking the board, a decision's Why, neither when `null`; `policy.test.ts`'s scan `(card|proposal)\.[a-z_]+`, `labels.ts` alone exempt, its control red; `words.test.ts`: no `writer` in `en`, no Russian stem in `ru`. M: a `"proposal.staged"` literal in `CardPanel.tsx`.
- [x] U6 — **[look]** Under "Implementation".
- [x] U7 — **[measure]** Size: under "Implementation".
- [x] D1 — **[measure]** Dedup on a real corpus, the five targets with the most accepted decisions: under "Implementation".
- [x] P1 — **Plugin 0.36.0.** `PINS`, `SCHEMA_PINS` end at 0.36.0, `spec_ids_hub.rs` and `tag-check.sh` read it; ask-owner 4 095 / 4 374 B with both sentences. M: `plugin.json` at 0.35.0; the `card_id` sentence dropped.
- [x] G1 — **Gates.** UI lint 0, `tsc` clean, the full run twice; the Rust workspace and the four eval tests; the docs check clean.

## Out of scope

- `card_id` on any other kind or a linked update; changing an item's card; two cards.
- A question's addressee; a discrepancy or a `proposed_patch` from the page.
- Item counts on tiles, a parent's rollup, Ready to accept (RM-0098).
- A card moving when its question is answered; anything waiting on an item.
- A page confirm on a local daemon (ADR-0035); the Questions screen's `as`, a card filter in the inbox (RM-0099); a record's `card` slot (Q11).

## Implementation

| Module | Does |
|---|---|
| core `intake.rs`, `record.rs` | `DecisionInput.why` and its caps; `Slot::Why` |
| store `queue.rs`, `queue/{state,cards,stage}.rs` | step 11, `card_id` and `why` judged and decoded, `CardRefused`; 48 columns; `card_items`, `item_history`; `stage_as` |
| CLI `intake.rs`, `main.rs`, `card.rs`, `proposals.rs`, `stage.rs`, `round.rs`, `state_file.rs` | `--card`; `items`, the merged `history`, `card_raise`; the review's keys; `StageBody`'s `as`; format 5 |
| MCP `intake.rs`, `hub.rs`, `mirror.rs`, `review.rs` | `card_id`, `why`, the texts; `CardItemShape` |
| http `app.rs`, `hub.rs`, `answer.rs` | `raise_on_card`, `WRITES` `items`, the stage's `as` bound on a hub |
| UI `src/board/{CardPanel,CardItems,CardDialogs,CardActs,labels}`, `inbox/ProposalCard.tsx` | Activity, Return's question, the acts, Answer…, the Inbox link |

Reviews: Rust ACCEPT; UI REWORK, then iteration 2: the card re-read on every `proposal.*` event and after a confirm, one primary button, focus off read-only fields and the body, a lasting status region, "Answer" only on a rejected item's note, the stored PR announced, `formatUtc` staged times (the looks' L1), priced choices.

**Departures**, accepted. Rust: `why` in `Intake.why`, no `Proposal.why`; refusals keep `spec: `, a bad stage `as` `spec: as: <problem>; nothing changed`; clap help drops the final period; the store also judges `card_id`'s kind and grammar and the actor. UI: `as` sent only when known; Return checks one target at least, the daemon the 16; mock differences (C-0008's return move, a hub scenario). Rejected: no cross-tab refresh. Churn: `hub_team.rs` `as_on_a_stage_or_a_round_body_is_a_400` reversed by D8, H2 (a round body's top-level `as` still 400). About 15 Rust test files beyond the table, and `shards_repo` (the `card*` shard holds 4 canons). Untested: S4's completion; S1's rows skip create, interpretation, amendment and drift.

**Numbers.** Rust: 28 mutations red; `PINS` 0.36.0 `1ee268…`, `SCHEMA_PINS` `1c47d3…`, both set back red; counts: the full gate. UI: 4 991 tests in 167 files; eval 58/58; 12 UI mutations red. U7: first-load JS 840.79 kB (+13.17, bar 845), gzip 241.97; `CardItems` 10.26 kB lazy; `ru` 147.34 kB.

**Looks** (U6), 2026-11-11, on a scratch copy of a real game corpus: Russian light at 1440, dark at 320, nothing cut; a return that asked, Activity, Answer…'s stage, the Inbox's link. The record link was seen on the mock alone. D1, through the route on that copy: the five targets' first raises gave 8, 5, 3, 2 and 2 hits and stored nothing; Ask anyway naming every hit stored each; none neared 64 hits, so the too-many line is the tests'.

**Known limits** (`card-questions.md`): the live hub (v0.35.0, schema 10, tmpfs dumps, no backup service) needs a snapshot off the host before this upgrade; a local Activity names whatever "Acting as" holds (empty goes out as `local`), so its default should be the hub login, or asked once locally; size: RM-0098.
