---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0088, its question half, v5 core slice 8 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section 3, RM-0088); the orchestrator's working answers, the owner's review open
adrs: []
---

# plain-questions: a question leads with its gist, why it matters and its options (RM-0088, the question half)

## Why

A question reached the owner as the agent wrote it: a wall of IDs, paths and code names, with one working answer and nothing to pick from. A discrepancy already carried priced options and a recommendation; a question did not, so the owner read code to learn what was asked and then wrote the answer himself.

Now `ask_question` takes four optional fields: a one-line `gist`, why it `matters`, two or three priced `options` and a `recommendation`. The Questions screen, the Inbox, a card's Activity and its Answer… show the gist, why it matters and the options first. The recommendation is labelled with who made it and never preselected. The agent's own text is folded below, verbatim. The engine stores and folds; it never rewrites an agent's words (no LLM inside). A soft note asks for plain words when the gist reads as code. No ADR: it relies on ADR-0012, ADR-0035, ADR-0038, ADR-0054, ADR-0061, ADR-0072, ADR-0077 and `docs/canon/architecture.md#ui`.

## Description and interactions

Each decision is canon now, in `docs/canon/plain-questions.md`: D1 optional, questions only, and D5 the note: "Fields", "The note"; D2 queue schema 13, backup format 7: "Shapes"; D3 judged by fields: "Answers"; D4 never preselected, D6 the page's dialogs unchanged, D8 the primitives: "UI"; D7 plugin 0.43.0: "MCP and plugin".

The owner's questions, open, their working answers built: **Q-A** the fields stay optional (required would break older prompts, the CLI, a hub's callers and the page's dialogs); **Q-B** RM-0088 split, its question half before RM-0060, as his word of 2026-11-12 orders; **Q-C** nothing preselected for discrepancies too. The writer's W1 to W8 (`--options F` read as `--input`; `[]` refused; the note reads the gist only and names one token; the sheet's frame words; Activity without options; D4 for discrepancies; "the asker" for any non-agent, no login lookup; `CardRaiseBody` unchanged, a deliberate divergence from the analysis's contract) are in that canon too.

## Data

In the canon: `plain-questions.md` "Fields", "The note", "Shapes", "Documents"; `proposal-queue.md` "Store" (schema 13); `queue-backup.md` "Format" (format 7); `agent-intake.md` "Tools"; `agent-intake-documents.md` "Review document", `proposal-review.md` (53 keys); `card-questions.md` "Read". **The example**, the tests' corpus `fixtures/spec-a`: `spec propose question MEC-STAMINA RULE-STAM-REGEN --text "Does RULE-STAM-REGEN's delay restart when stamina_regen() is called mid-sprint?" --working-answer "yes, the 1.5 s delay restarts" --price-of-other "R-28 rebalanced" --gist "Should resting restart the wait before stamina refills?" --matters "Sprint tuning and two tasks wait on it." --options - --recommendation 0 --distinct-from DEC-0023 --author-role developer`, stdin `[{"label": "Restart the wait", "effect": "Short rests do not refill", "price": "none: as built"}, {"label": "Keep the wait running", "effect": "Tapping rest refills faster", "price": "R-28 rebalanced"}]`; its sheet is `round-sheet.md`'s PR-0007.

### Approve, stage, form, round

Canon: `plain-questions.md` "Answers", "Form and sheet"; `decision-record-template.md` "Flags", "Template"; `review-form.md` "Form"; `round.md` "Sheet", "JSON", "Answers"; `round-sheet.md`.

### MCP

Canon: `plain-questions.md` "MCP and plugin". The description texts, 2 031 and 2 044 B after six edits, are pinned in `mcp_intake.rs` and `mcp_hub.rs`.

### The plugin

Canon: `plugin-files.md` "Changing it", `plugin.md` "Skills". `ask-owner`'s eight edits are in its `SKILL.md`, pinned in `plugin_skills.rs`.

### The UI

Canon: `plain-questions.md` "UI"; `round-ui.md` "Screen"; `decision-staging.md` "UI"; `card-questions.md` "Screen"; `ui/README.md` "Screen rules".

## Rules and edge cases

In the canon: `plain-questions.md` "Fields" (agent text is data), "Answers", "UI", "Known limits" (the rollout). Without the four fields a question stores, answers and renders as before; a decision or a discrepancy given `gist` or `matters` is refused.

## Acceptance criteria

A browser look is **[look]**, a timed or counted run **[measure]**; every other criterion is a test that the named mutation turned red. New Rust tests: `plain_questions.rs` in core, store, CLI, MCP and http, over scratch git copies of `fixtures/spec-a`; core and store units in `#[cfg(test)]`.

- [x] PQ1 — **Round trip.** The "Data" example stores `PR-0001`. `spec review PR-0001 --json` and `get_proposal` return `gist`, `matters`, both options and `recommendation` 0 byte-equal to the input. The review text has `gist:` and `matters:` after `summary:`, and the options as a discrepancy's. Mutation, red: `matters` not stored.
- [x] PQ2 — **Caps.** Each exits 1 with its line and stores nothing, the MCP answer `isError` with the document: 4 options; 1 option; `[]`; a 161-byte gist; a gist holding LF; a blank gist; a 513-byte `matters`; `--recommendation 2` with 2 options; a recommendation without options; a 129-byte label. A malformed `--options` file exits 2. Mutation, red: 4 options accepted.
- [x] PQ3 — **Nothing given, nothing changed.** The existing `propose question` pins (text, JSON, MCP) pass unedited but for the two `null` keys, and the stored row's `gist`, `matters`, `options` and `recommendation` are NULL. Mutation, red: `gist` required.
- [x] PQ4 — **Approve.** With `[decision_records]` configured in the copy, `spec approve PR-0001 --option 1` records `choice {"option":1}`, title `Keep the wait running`, `{{effect}}` and `{{cost}}` option 1's, `{{options}}` both. No flag takes the working answer, `--answer T` takes T. `--option 2` exits 1 naming `0-1`; on an option-less question `--option 0` exits 2 with the new line; a discrepancy without `--option` still exits 2. Mutation, red: `--option` on a question ignored.
- [x] PQ5 — **Shapes.** `Invalid` in store units: a decision with `options`, `gist` or `matters`; a discrepancy with `gist`. Corrupt, column named: a decision with `options`, a discrepancy with `matters`, a question's `recommendation` without options, a question's `choice {"option":5}` over 2 options. A question and a decision of equal text on a shared target still hit. Mutations, each red: one shape for question and decision; a dedup family per shape.
- [x] PQ6 — **Verbatim, and the note.** A gist, `matters` and labels holding `MEC-STAMINA`, `docs/spec/x.md`, `a::b`, U+202E and CRLF in `matters` round-trip byte-equal through JSON; text output escapes the controls. The note names the first of `MEC-STAMINA`, `docs/spec/x.md`, `stamina_regen::tick`, and is absent for `stamina` and `ZZ-99`. The item stores either way; two flagged tokens give one note. Mutations, each red: IDs stripped from the stored gist; the note refusing.
- [x] PQ7 — **Form and sheet.** `review_proposal` on `PR-0001` sends line 5 with `gist:`, `matters:`, `summary:`, both options and `working answer:`, and the choices `option-0` `Option 0: Restart the wait (recommended)`, `option-1`, `working-answer`, `reject`; accepting `option-1` stages `--option 1`. `spec round new` prints the "Data" sheet; `PR-0001: 1` stages option 1, `ok` the working answer, `PR-0001: no` `--answer no`; an option-less question still refuses `1`; the JSON item has 21 keys. Mutation, red: the sheet refusing a number on a question.
- [x] PQ8 — **Schema 13, format 7.** `QUEUE_SCHEMA_VERSION` 13 and `PROPOSAL_COLUMNS` 50 pinned to `PRAGMA table_info`. A schema 12 DB with rows migrates, every row kept. `spec export state` writes format 7, schema 13, 50 columns. A format 6 dump imports with both columns NULL and re-exports as format 7, equal but for the header and the two nulls. A format 7 header with `queue_schema` 12 is refused naming 13; a build set to 12 meets `SchemaTooNew` on a 13 DB. Mutation, red: the format 6 import path dropped.
- [x] PQ9 — **Parity, schemas, texts, the hub.** Stdio `ask_question` with all four fields: content byte-equal to the CLI's `2>&1`, `structuredContent` to its `--json`. The schema walker passes with exact key sets (input the four, nullable, `options` items closed; output `gist`, `matters`). Descriptions 2 031 and 2 044 B; `INSTRUCTIONS`, `HUB_INSTRUCTIONS` byte-equal to before. A hub's `/mcp/<slug>` stores the fields; `tools/list` holds 18. Mutations, each red: `gist` dropped from the mirror; the hub's args ignoring `options`.
- [x] PQ10 — **Daemon keys.** `GET …/proposals/PR-0001` serves 53 keys with the fields; `…/inbox` entries 13, `…/round` items 21, the card's `items` 18. `fixtures/daemon-keys.json` names them, `provisional.ts` matches, `AHEAD_OF_FIXTURE` empty at the end. Mutation, red: `matters` out of the fixture.
- [x] PQ11 — **Plugin 0.43.0.** `plugin.json` 0.43.0, `.env.example` `SPECENGINE_VERSION=0.43.0`, `tag-check.sh v0.43.0` prints it; `PINS` and `SCHEMA_PINS` end with 0.43.0. `ask-owner` carries the eight edits, each once: body 4 089 B, file 4 368, no digit outside a step number, the JSON example deserializing as `ask_question`'s arguments. Mutations, each red: the `gist` sentence dropped; the schema changed with no `SCHEMA_PINS` entry.
- [x] PQ12 — **The UI.** Vitest over the mock and builders, on the Questions screen, the Inbox card and dialog, Activity and Answer…: a gist item leads with the gist, why it matters, the options, the recommendation "Recommended by the AI" for an agent, "Recommended by the asker" for a human; no radio checked and Accept off until a pick; the asked text in a collapsed `Disclosure`, verbatim in `<bdi>`; a question with options offers options, the working answer and an own answer, a discrepancy options alone; an item without a gist as before. `policy.test.ts` passes. Mutations, each red: the recommendation preselected; `author.type` compared in `ItemLead.tsx`.
- [x] PQ13 — **Strings and size.** `en.ts` and `ru.ts` hold the five keys, `tsc` is clean, `words.test.ts` passes; first-load JS at most 845 kB, recorded below. Mutation, red: a `lead.*` key missing from `ru.ts`.
- [x] PQ14 — **[look]** At 1 200 px, both themes, in Russian: the Questions screen and the Inbox card lead with the gist, why it matters, the options and the label, nothing picked, the fold closed; an item without a gist as before.
- [x] PQ15 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli`, `-mcp`, `-http` and `-eval` green, then the full run once; the eval tests `doc_pointers`, `ui_policy`, `anonymity` and `spec check` pass; UI lint, `tsc`, `pnpm test`, `pnpm build` pass.

## Out of scope

The pair half, `collision-pairs` (after RM-0060); fields on a decision or a discrepancy; new fields in the page's dialogs or `CardRaiseBody`; a template slot for the gist; rewriting or cutting agent text; a plain-words refusal; a note on `matters` or a label.

## Implementation

Rust review ACCEPT after three iterations (the third for the tester's three reds); UI review ACCEPT with fixes; PQ14 the orchestrator's look; PQ15 his full run, ticked on his word.

| Module | Does |
|---|---|
| core `intake.rs`, `record.rs` | the caps and their order, `gist_note`; `Choice` by fields |
| store `queue.rs`, `queue/{stage,state,cards,card_counts_seam}.rs` | `STEP_13`, three `IntakeShape`s, `IntakeFamily`, choices, `Stage::problem_holding`, 50 columns |
| CLI `intake.rs`, `propose.rs`, `main.rs`, `lib.rs` | the four flags, `read_question_options`, the note's `::` test, `GistNames` |
| CLI `decide.rs`, `confirm.rs`, `package.rs`, `round.rs`, `inbox.rs`, `card.rs`, `proposals.rs`, `state_file.rs` | `first_checks` by fields, slots, the assumption, the sheet, the keys, format 7 |
| MCP `intake.rs`, `hub.rs`, `mirror.rs`, `review.rs`, `cards.rs` | args, descriptions, shapes, the form, `get_card`'s text |
| `plugin.json`, `ask-owner`, `.env.example`, `fixtures/daemon-keys.json` | 0.43.0, eight edits, the key sets |
| UI `inbox/{ItemLead (new),labels,ProposalCard,DecisionDialog,ProposalList,summary,filter}`, `round/{drafts,RoundItemCard}`, `board/CardItems`, `api/provisional`, `i18n/{en,ru}`, `styles/app.css`, mocks | the lead, `recommendedBy`, controls by structure, no preselection, strings, the scenario |

**Departures**, accepted, canon now: (1) R1: the note also flags a legacy or declared alias, named as written, trimmed; (2) core tokenises, trims, judges `.md` and keeps the order, the CLI judges `::` and nodes (guard `code_refs_walk`, symbol-index "Genre"); (3) the store judges a staged option by the row's fields; (4) `--option` with `--answer` refused, exit 2, everywhere, the store alike; (5) `get_card`'s description lists 18 item keys at 2 047 B, its hash unchanged; (6) `subtask_items` gains the keys, 19; (7) so does `propose_change`'s output; (8) `ask-owner` 4 089 B by `splitn`, the spec's 4 090 one byte more; (9) the developer's: the sheet's order, packages assuming the working answer, the `recommendation` doc's digit, corrupt `gist`/`matters` named on create, amendment and drift rows, one `IntakeOption` description; (10) UI: keys `inbox.lead.*`, neither options nor a price: today's controls, the mock's own scenario.

**Test differences**: no format 6 dump committed under `fixtures/` (queue-export AC-11): the test builds one, a hand check a real 4d52ec3c dump; the "Data" example stores only with `--distinct-from DEC-0023`; a CRLF `matters` folds to a space in text; a stored `options: []` reads fine.

**Numbers**: first load 831 994 B (bar 845 kB), gzip 244.5 kB; `ru-*.js` 159.29 kB, gzip 36.74 kB; UI 5 634 tests twice, 13 mutations red. **Look**: the mock and a scratch copy of a real game corpus, its queue migrated 12 to 13: the gist as `h2`, the AI's label, nothing preselected, Accept off until a pick, option 1 staged `{decision: approve, option: 1, answer: null}`, review JSON 53 keys, an ID in a gist drew the note. **Known limits**: `plain-questions.md` "Known limits".
