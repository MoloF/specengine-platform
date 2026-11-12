---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0088, its question half, v5 core slice 8 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section 3, RM-0088); the orchestrator's working answers, the owner's review open
adrs: []
---

# plain-questions: a question leads with its gist, why it matters and its options (RM-0088, the question half)

## Why

A question reaches the owner as the agent wrote it: a wall of IDs, paths and code names, with one working answer and nothing to pick from. A discrepancy already carries priced options and a recommendation (`docs/canon/agent-intake.md` "Tools"); a question does not, so the owner reads code to learn what is asked and then writes the answer himself.

After this slice `ask_question` takes four optional fields: a one-line `gist`, why it `matters`, two or three priced `options` and a `recommendation`. The Questions screen, the Inbox, a card's Activity and its Answer… show the gist, why it matters and the options first. The recommendation is labelled with who made it ("Recommended by the AI" for an agent) and is never preselected. The agent's own text is folded below, verbatim. The engine stores and folds; it never rewrites an agent's words (no LLM inside). A soft note asks for plain words when the gist reads as code.

No ADR. It relies on ADR-0012 (the note never refuses), ADR-0035, ADR-0038, ADR-0054 and ADR-0061 (a question's option is staged and confirmed as a discrepancy's is), ADR-0072 (both catalogs) and ADR-0077, and on `docs/canon/architecture.md#ui` (the UI renders what the daemon serves). The pair view for collisions is the other half, `collision-pairs` (Out of scope).

## Description and interactions

### Decisions

- **D1. Optional, questions only.** All four fields are optional. A decision and a discrepancy keep their shape. `price_of_other` stays required. A question without the fields stores, reads and answers exactly as today.
- **D2. Queue schema 13, backup format 7.** The schema adds `gist` and `matters`; a question may then hold `options` and `recommendation`, and a decision still may not. If this slice does not ship right after RM-0059 and RM-0061, it takes the next free numbers and records them here.
- **D3. Judged by fields.** Approve, the stage, the form, the round and the UI choose answers by the item's fields, never its kind (`docs/canon/decision-record-template.md` "Flags"). With options, each option can be chosen. With `price_of_other`, the working answer and an own answer can be chosen too. So a question with options takes `--option N`, its working answer or `--answer T`, and a discrepancy still takes only `--option N`.
- **D4. Never preselected, everywhere.** No screen preselects a recommendation, the Inbox's decision dialog included (today it preselects a discrepancy's).
- **D5. The plain-words note** is advice in `notes`, on the stored answer or the not-stored one. It never refuses.
- **D6. The page's dialogs are unchanged.** Return, Ask a question… and Record a decision… send no new field, and `CardRaiseBody` stays as it is. Only agents (stdio and a hub's `/mcp`) and the CLI send the fields.
- **D7. Plugin 0.43.0**, the MINOR after RM-0059's 0.42.0. `ask_question` takes four keys, and `get_proposal` and `get_card` return two. `ask-owner` teaches it.
- **D8. The UI follows design-sweep (RM-0099 a).** It is built on the primitives (`docs/canon/ui-design.md` "Primitives"), with CSS scale tokens only, strings in both catalogs, and first-load JS within 845 kB.

## Data

### Fields and caps (core `intake.rs`)

| Field | CLI | Rule (exit 1, nothing stored) |
|---|---|---|
| `gist` | `--gist G` | given: not blank, no LF or CR (`gist: holds a line break; give one line`), at most 160 bytes (`GIST_MAX`) |
| `matters` | `--matters M` | given: not blank, at most 512 bytes (`MATTERS_MAX`) |
| `options` | `--options F` (a JSON array `[{label, effect, price}]`, a file or `-`, read as `--input` is: UTF-8, at most 8 MiB; a bad shape exits 2) | given: 2 to 3 (`QUESTION_OPTIONS_MAX` 3; `options: 4 option(s); give 2 to 3, each priced`, `[]` too), each option as a discrepancy's (`label` <= 128, `effect`, `price` <= 512, not blank) |
| `recommendation` | `--recommendation N` | needs `options` (`recommendation: given without options`), and is an index into them (the discrepancy's line) |

- **Input order** for the caps (rule 2): `node_ids`, `text`, `gist`, `matters`, `options`, `recommendation`, `working_answer`, `price_of_other`, `distinct_from`, `card_id`, the author.
- **Types.** `QuestionInput` gains `gist: Option<&str>`, `matters: Option<&str>`, `options: Option<&[IntakeOption]>` and `recommendation: Option<u64>`. `Intake` gains `gist`, `matters`. The CLI's `QuestionRequest` gains the same four, owned.
- **Unchanged.** The dedup compares `text` as today (`agent-intake.md` "Dedup"): two questions that differ only in gist still hit.

### The plain-words note

When the item passes rules 1-5, its gist is cut into tokens at Unicode whitespace. A token is flagged when, in this order:
1. it holds `::`;
2. once leading and trailing ``` `"'()[]{}<>,.;:!? ``` are trimmed, it ends in `.md` (ASCII case-insensitive);
3. or it resolves as rule 3 resolves a `node_ids` item (an ID, alias or `slug/ID`) in the index the call refreshed. A resolution error, a look-alike's included, is no flag.

The first flagged token adds the last note, once per item:

```text
gist: `RULE-STAM-REGEN` reads as code to the owner: say it in plain words; IDs belong in node_ids
```

The note is escaped as notes are. It appears whether the item is stored or not, and never on a refusal.

### Store: queue schema 13

- **Step 12 -> 13** (`STEP_13`): `ALTER TABLE proposals ADD COLUMN gist TEXT`, then `matters`, in `migrate`'s one transaction with every due step. This gives `QUEUE_SCHEMA_VERSION` 13 and `PROPOSAL_COLUMNS` 50 (`proposal_columns(12)` is 48).
- **Shapes.** `IntakeShape` becomes `{Question, Decision, Discrepancy}`. `ProposalKind::intake_shape()` maps each kind to its own shape. The dedup compares `IntakeShape::family() -> IntakeFamily {Asks, Reports}`: question and decision are `Asks`, a discrepancy is `Reports`, the same families as today. `null_columns()` per shape:

| Shape | Holds none of (set: corrupt, named; given: `Invalid`) | Required |
|---|---|---|
| `Question` | `gap_type`, `evidence` | `working_answer`, `price_of_other` |
| `Decision` | `gap_type`, `evidence`, `options`, `recommendation`, `gist`, `matters` | as a question |
| `Discrepancy` | `price_of_other`, `gist`, `matters` | `gap_type`, `recommendation` |

- **Question rows.** A question's `recommendation` with `options` NULL is corrupt (`` the stored `recommendation` cannot be read: it is set without options ``). Its range and the options' JSON are checked as a discrepancy's.
- **Choices.** `choice_fits` and the `choice` decode accept a question's `{"option":N}` in range of its options, as well as `{"working_answer":true}` and `{"answer":"…"}`. A decision takes the last two only, and a discrepancy `option` only.

### Approve, stage, form, round

- **`first_checks`**, by fields: an item without options and with `--option` exits 2 with `` `--option` names an option, and `PR-0001` is a question with none: its working answer, or `--answer T`, answers it; nothing changed `` (`a decision with none` alike). An item without `price_of_other` and with `--answer`, or an open one without `--option`, keeps today's lines. Everything else is unchanged.
- **The record.** An option chosen on a question fills `title`, `choice`, `effect` and `cost` as a discrepancy's option does. `{{options}}` lists a question's options whenever it holds them, and `{{summary}}` is its text. No slot is added, so older `spec approve` builds read every template.
- **`review_proposal`'s form** (`review-form.md` "Form"). Line 5 for a deciding item becomes: `gist: <gist>` and `matters: <matters>` when held, then `summary: <summary>`, each option, then `working answer: <working_answer>` when the item holds `price_of_other` and a working answer. The choices list each option (`` (recommended)`` on the recommendation), then `working-answer` when `price_of_other` is held, then `reject`.
- **`spec round`**: the JSON item has 21 keys, `gist` and `matters` after `summary`. In the text sheet, a held gist is printed as `Gist: <gist>` right after the heading, then `Why it matters:` when held, and then the text gets the frame word `As asked:`. Without a gist the text is printed as today. Held options are printed as a discrepancy's (`round.md` "Sheet"), before `Working answer:`. Answers: `N` on a question with options approves `--option N`; `ok` takes the working answer; other text is `--answer`. An option-less question refuses `N` as today.

```text
## PR-0007 - question, normal

Gist: Should resting restart the wait before stamina refills?

Why it matters:
  Sprint tuning and two tasks wait on it.

As asked:
  Does RULE-STAM-REGEN's delay restart when stamina_regen() is called mid-sprint?

Options:
  - [0] Restart the wait | Short rests do not refill | none: as built (recommended)
  - [1] Keep the wait running | Tapping rest refills faster | R-28 rebalanced

Working answer:
  yes, the 1.5 s delay restarts

Another answer costs:
  R-28 rebalanced

Concerns: Stamina (MEC-STAMINA); Regeneration (RULE-STAM-REGEN)

PR-0007:
```

### Documents and daemon keys

| Shape | Gains | Keys |
|---|---|---|
| review document, daemon `Proposal`, MCP `ReviewDocument` | `gist`, `matters` after `summary` (text lines `gist:`, `matters:` after `summary:`) | 53: RM-0059's 51 plus two |
| `spec inbox` JSON, `InboxEntry` | `gist` after `summary`; the text's last column is `<severity>: <gist, else the summary's first line>` | 13 |
| round item, `RoundItem` | `gist`, `matters` after `summary` | 21 |
| card `items`, `CardItem`, MCP `CardItemShape` | `gist`, `matters` after `summary`; the text `items:` line shows the gist, else the summary | 18 |

A question's `options` and `recommendation` fill the existing keys. Every other kind serves `gist` and `matters` as `null`. No route, event or key set is added.

The example (`fixtures/spec-a`): `spec propose question MEC-STAMINA RULE-STAM-REGEN --text "Does RULE-STAM-REGEN's delay restart when stamina_regen() is called mid-sprint?" --working-answer "yes, the 1.5 s delay restarts" --price-of-other "R-28 rebalanced" --gist "Should resting restart the wait before stamina refills?" --matters "Sprint tuning and two tasks wait on it." --options - --recommendation 0 --author-role developer`, with stdin:

```json
[{"label": "Restart the wait", "effect": "Short rests do not refill", "price": "none: as built"},
 {"label": "Keep the wait running", "effect": "Tapping rest refills faster", "price": "R-28 rebalanced"}]
```

### Backup format 7

`STATE_FORMAT` 7, `{"format":7,"queue_schema":13,…}`, with the eight header keys of formats 3-6 and 50 proposal columns, `gist` and `matters` last. Formats 1-6 import with both NULL. Format 6 holds schema 12. Format 7 holds schema 13; RM-0060 widens it to 13-14. A schema 1-12 DB exports as format 7, schema 13, unmigrated.

### MCP

- **`ask_question`**, stdio `QuestionArgs` and hub `HubQuestionArgs`, gains after `text` the nullable fields `gist`, `matters`, `options` (items closed `{label, effect, price}`, the `mirror::IntakeOption` schema) and `recommendation` (an integer). Their property docs carry no digits:
  - `gist`: "The question in one line of plain words for the owner: no IDs, paths or code names."
  - `matters`: "Why it matters to the owner, in plain words."
  - `options`: "Two or three answers the owner may pick, each with its effect and price."
  - `recommendation`: "The index of the option you recommend, from 0."
- **The descriptions**, `QUESTION_DESCRIPTION` (2 033 -> 2 031 B) and `HUB_QUESTION_DESCRIPTION` (2 030 -> 2 044 B), ASCII, within 2 048. Every `holds_number` assert stays, and new ones tie 160, 512, 2 and 3 to `GIST_MAX`, `MATTERS_MAX`, `OPTIONS_MIN` and `QUESTION_OPTIONS_MAX`. The edits:
  1. after `[--severity S] `, insert `[--gist G] [--matters M] [--options F] [--recommendation N] `;
  2. after the `text:` line, insert `gist (160 bytes), matters (512), options (2-3) and recommendation: what the owner reads first; plain words, no IDs, paths or code names.` and an LF; the hub's reads `what a person reads first`;
  3. `node_ids: 1 to 16 IDs (or slug/ID) it is about, or root-relative .md paths naming their files' documents, each node once;` -> `node_ids: 1 to 16 IDs (or slug/ID) or root-relative .md paths it is about, each once;`;
  4. `A hit: an accepted decision document linked to a node (its id, path and title as the answer), or` -> `A hit: an accepted decision linked to a node (its id, path, title as the answer), or`;
  5. drop ` Related: decision documents only mentioning a node, queued items with other text.`;
  6. stdio: drop ` (a decision record: a queue hit's record, path, title as the answer)`. Hub: drop ` (a queue hit's record, path, title as the answer)`, and change `with a Confirm on the hub` to `with Confirm on the hub`.
- `get_proposal`'s and `review_proposal`'s output gains `gist` and `matters`, and `get_card`'s `items` gains them too. `INSTRUCTIONS` and `HUB_INSTRUCTIONS` are unchanged. The stdio server keeps 30 tools and the hub 18.

### The plugin

- `plugin.json` and `scripts/deploy/.env.example` go to 0.43.0, and `PINS` and `SCHEMA_PINS` (`plugin_files.rs`) append 0.43.0 entries.
- **`ask-owner`**: body 4 096 -> 4 090 B, file 4 374 -> 4 368. Its description is unchanged.
  1. `Use this when the spec is silent or ambiguous, or contradicts the code. Raise a record` -> `Raise a record`;
  2. after the question bullet's `what another answer would cost.`, add `` Lead with a one-line `gist` and why it `matters`, in plain words, no IDs, paths or code names; then two or three priced `options` and your `recommendation` when known.``;
  3. `- The `related` items are context only.` and the refusal bullet become one bullet: ``- The `related` items are context only; a refusal names the field: fix it, send again.``;
  4. `` gives its `status` and the owner's `decision_note`; on a rejected record the note is the owner's answer; on an applied one, `` -> `` gives its `status` and `decision_note`, the owner's answer on a rejected record; on an applied one, ``;
  5. `` the owner decided it, and that record is the owner's decision. Read it with `get_node` on its `record`; off this branch it is not there yet: `get_proposal` `` -> `` the owner's decision. Read it with `get_node` on its `record`; off this branch, `get_proposal` ``;
  6. `` its `answer` is only the decision's title. Read the decision with `get_node` `` -> `` its `answer` is only its title. Read it with `get_node` ``;
  7. `; it takes no evidence;` -> `; no evidence;`, and `the owner rejected that record;` -> `the owner rejected it;`;
  8. in the JSON example, after `"text"`, add the line `  "gist": "<the question in plain words>",`.

### The UI

- **Types**: `ui/src/api/provisional.ts` gains `Proposal.gist`, `matters`, `InboxEntry.gist`, `RoundItem.gist`, `matters` and `CardItem.gist`, `matters`, each `string | null` and each after `summary`. `ui/src/test/builders.ts`, `ui/src/mocks/build.ts` and `daemonKeys.test.ts` follow, with `AHEAD_OF_FIXTURE` holding them until the fixture does. The mock gains an agent's question with all four fields and a person's without them.
- **The lead**: `ui/src/inbox/ItemLead.tsx` (new). For an item holding `gist`:
  - the gist leads on one line, followed by "Why it matters" with `matters` when held, then the options or controls;
  - everything else the item shows today (its text, working answer, price of another answer, why) moves into a `Disclosure` collapsed by default, summary `lead.asked` "The question as asked";
  - agent text stays verbatim in `<bdi>` through `shownSlot`.

  An item without `gist` renders exactly as today.
- **Who recommends**: `ui/src/inbox/labels.ts` `recommendedBy(author, t)` is the one place the author's type is compared. An `agent` gets `lead.recommendedByAgent` "Recommended by the AI"; any other author gets `lead.recommendedByAsker` "Recommended by the asker". It replaces "Recommended" in `RoundItemCard.tsx`, `ProposalCard.tsx`, `DecisionDialog.tsx` and `CardItems.tsx`. A key left without users leaves both catalogs.
- **Controls by structure** (`round/drafts.ts` `offers`, `board/CardItems.tsx` `picksOf`, `RoundItemCard.tsx`):
  - each option when held;
  - the working answer when `price_of_other` and `working_answer` are both non-null;
  - an own answer when `price_of_other` is non-null;
  - Reject, and Leave open on the Questions screen.
- **The Inbox** (`ProposalCard.tsx`, `DecisionDialog.tsx`, the list's summary):
  - the lead, as above;
  - the decision dialog offers each option and, with `price_of_other`, the working answer; nothing is preselected, and Accept stays off until a pick (`lead.pickFirst` "Pick an answer first");
  - a list row shows the gist, else the summary.
- **Activity** (`CardItems.tsx`): an item's block leads with the gist and why it matters; the rest is folded as above.
- **Strings**: the five `lead.*` keys in `en.ts` and `ru.ts`; the Russian is a draft for the owner. CSS uses scale tokens only.

## Rules and edge cases

- WHEN `ask_question` carries none of the four fields, the system SHALL store, answer and render the item byte-equal to today.
- WHEN a question holds options, the system SHALL let the owner choose an option, the working answer or his own answer on every surface but the Inbox, which offers no own answer (`decision-staging.md` "UI").
- WHEN a decision or a discrepancy is given `gist` or `matters`, the system SHALL refuse it (the store's `Invalid`; MCP's schema has no such field on those tools; `--input` an unknown field, exit 2).
- WHEN the gist reads as code, the system SHALL add the note and still store as the dedup decides.
- Agent text is data: the fields are stored as given, JSON raw, escaped in text output (`agent-intake-documents.md` "Escaping and size"), and shown verbatim in `<bdi>`. Nothing is trimmed, rewritten or cut.
- No kind, `gap_type` or author type is compared outside `labels.ts` (`card-questions.md` "Screen", `policy.test.ts`).

### Rollout

- Schema 13: older binaries refuse the queue (`SchemaTooNew`), and the hook gives no verdict until reinstall. Every terminal, plugin and hub upgrades from one commit; a hub dumps first (`card-mandate.md` "Known limits").
- An older hub's `/mcp` refuses the four keys (`unknown field`) until its upgrade, the owner's call. An older build refuses a format 7 dump.

### Test churn

The following re-pin: review key lists (CLI, the MCP walkers, http `daemon_keys.rs`); the round JSON; the inbox and card item keys; the `QUEUE_SCHEMA_VERSION`, `PROPOSAL_COLUMNS` and `STATE_FORMAT` pins; and backup fixtures and dumps. So do `first_checks`' `--option` line (`decision_apply` tests), the form's choice lists, `DecisionDialog`'s preselection tests, the description and plugin pins (`plugin_skills.rs`, `plugin_files.rs`, `plugin_roles.rs`), and http `spec_ids_hub.rs` (`v0.43.0`).

## Acceptance criteria

A browser look is **[look]**, a timed or counted run **[measure]**; every other criterion is a test that the named mutation turns red. New Rust tests: `plain_questions.rs` in `crates/specengine-cli/tests` and `crates/specengine-mcp/tests`, over scratch git copies of `fixtures/spec-a`; core and store units in `#[cfg(test)]`.

- [ ] PQ1 — **Round trip.** The "Data" example stores `PR-0001`. `spec review PR-0001 --json` and `get_proposal` return `gist`, `matters`, both options and `recommendation` 0 byte-equal to the input. The review text has `gist:` and `matters:` after `summary:`, and the options as a discrepancy's. Mutation, red: `matters` not stored.
- [ ] PQ2 — **Caps.** Each exits 1 with its line and stores nothing, the MCP answer `isError` with the document: 4 options; 1 option; `[]`; a 161-byte gist; a gist holding LF; a blank gist; a 513-byte `matters`; `--recommendation 2` with 2 options; a recommendation without options; a 129-byte label. A malformed `--options` file exits 2. Mutation, red: 4 options accepted.
- [ ] PQ3 — **Nothing given, nothing changed.** The existing `propose question` pins (text, JSON, MCP) pass unedited but for the two `null` keys, and the stored row's `gist`, `matters`, `options` and `recommendation` are NULL. Mutation, red: `gist` required.
- [ ] PQ4 — **Approve.** With `[decision_records]` configured in the copy, `spec approve PR-0001 --option 1` records `choice {"option":1}`. The record's title is `Keep the wait running`, `{{effect}}` and `{{cost}}` are option 1's, and `{{options}}` lists both. `spec approve` with no flag takes the working answer and `--answer T` takes T. `--option 2` exits 1 naming `0-1`. On an option-less question, `--option 0` exits 2 with the new line. A discrepancy without `--option` still exits 2. Mutation, red: `--option` on a question ignored (the working answer recorded).
- [ ] PQ5 — **Shapes.** Each of these is `Invalid` in store units: a decision with `options`, `gist` or `matters`; a discrepancy with `gist`. Each of these hand-edited rows reads corrupt with its column named: a decision with `options`, a discrepancy with `matters`, a question's `recommendation` without options, a question's `choice {"option":5}` over 2 options. A question and a decision with equal text on a shared target still hit each other. Mutations, each red: one shape for question and decision; a dedup family per shape.
- [ ] PQ6 — **Verbatim, and the note.** A gist, a `matters` and labels holding `MEC-STAMINA`, `docs/spec/x.md`, `a::b`, U+202E and CRLF in `matters` round-trip byte-equal through JSON, and the text output escapes the controls. The note names the first of `MEC-STAMINA`, `docs/spec/x.md` and `stamina_regen::tick` in a gist, and is absent for `stamina` and `ZZ-99` (unresolvable). The item stores either way, and with two flagged tokens there is one note. Mutations, each red: IDs stripped from the stored gist; the note refusing.
- [ ] PQ7 — **Form and sheet.** `review_proposal` on `PR-0001` sends line 5 with `gist:`, `matters:`, `summary:`, both options and `working answer:`, and the choices `option-0` `Option 0: Restart the wait (recommended)`, `option-1`, `working-answer`, `reject`. Accepting `option-1` stages `spec approve PR-0001 --option 1`. `spec round new` prints the "Data" sheet for it; `PR-0001: 1` stages option 1, `ok` the working answer, `PR-0001: no` `--answer no`. An option-less question still refuses `1`. The JSON item has 21 keys. Mutation, red: the sheet refusing a number on a question.
- [ ] PQ8 — **Schema 13, format 7.** `QUEUE_SCHEMA_VERSION` 13 and `PROPOSAL_COLUMNS` 50 are pinned to `PRAGMA table_info`. A schema 12 DB with rows migrates, every row kept. `spec export state` writes format 7, schema 13, 50 columns. A new format 6 fixture dump imports with both columns NULL, and re-exports as format 7, equal but for the header and the two nulls. A format 7 header with `queue_schema` 12 is refused naming 13. A build set to 12 meets `SchemaTooNew` on a 13 DB. Mutation, red: the format 6 import path dropped.
- [ ] PQ9 — **Parity, schemas, texts, the hub.** The stdio `ask_question` with all four fields gives content byte-equal to the CLI's `2>&1`, and `structuredContent` equal to its `--json`. The schema walker passes with exact key sets: input gains the four, nullable, `options` items closed; output gains `gist` and `matters`. The descriptions are as "MCP" edits them (2 031 and 2 044 B), and `INSTRUCTIONS` and `HUB_INSTRUCTIONS` are byte-equal to before. On a hub's `/mcp/<slug>` the call stores the fields, and `tools/list` holds 18. Mutations, each red: `gist` dropped from the mirror; the hub's args ignoring `options`.
- [ ] PQ10 — **Daemon keys.** `GET …/proposals/PR-0001` serves 53 keys with the fields. `…/inbox` entries have 13 keys, `…/round` items 21, and the card's `items` 18. `fixtures/daemon-keys.json` names them, `provisional.ts` matches, and `AHEAD_OF_FIXTURE` is empty at the end. Mutation, red: `matters` out of the fixture.
- [ ] PQ11 — **Plugin 0.43.0.** `plugin.json` is 0.43.0, `.env.example` has `SPECENGINE_VERSION=0.43.0`, and `tag-check.sh v0.43.0` prints it. `PINS` and `SCHEMA_PINS` end with 0.43.0. `ask-owner` carries "The plugin"'s edits, each once: body 4 090 B, file 4 368, no digit outside a step number, the JSON example deserializing as `ask_question`'s arguments. Mutations, each red: the `gist` sentence dropped; the schema changed with no `SCHEMA_PINS` entry.
- [ ] PQ12 — **The UI.** Vitest, over the mock and builders, on the Questions screen, the Inbox card and dialog, Activity and Answer…:
  - a gist item leads with the gist, then why it matters, then the options, the recommendation marked "Recommended by the AI" for an agent and "Recommended by the asker" for a human;
  - no radio is checked and Accept is off until a pick;
  - the asked text sits in a collapsed `Disclosure`, verbatim in `<bdi>`;
  - a question with options offers options, the working answer and an own answer, and a discrepancy options alone;
  - an item without a gist renders as before (its tests unedited).

  `policy.test.ts` passes. Mutations, each red: the recommendation preselected; `author.type` compared in `ItemLead.tsx`.
- [ ] PQ13 — **Strings and size.** `en.ts` and `ru.ts` hold the five keys, `tsc` is clean and `words.test.ts` passes. `pnpm build`'s first-load JS is at most 845 kB, recorded in "Implementation" with gzip and `ru-*.js`. Mutation, red: a `lead.*` key missing from `ru.ts`.
- [ ] PQ14 — **[look]** The mock in the browser, at 1 200 px, in both themes and in Russian, shows the Questions screen and the Inbox card with the gist leading, why it matters, the options, the label, nothing picked and the fold closed. An item without a gist looks as before.
- [ ] PQ15 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli`, `-mcp`, `-http` and `-eval` is green, then the full run once. The eval tests `doc_pointers`, `ui_policy` and `anonymity` pass, and `spec check` passes. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

- The pair half, `collision-pairs` (v5 core slice 10, after RM-0060 `collision-pass`): a served `pair {left, right}` for a `contradicts` discrepancy with exactly two targets, each side's title and path with the evidence grouped by target path, shown side by side and loaded lazily.
- Fields on a decision or a discrepancy.
- New fields in the page's dialogs or in `CardRaiseBody`.
- A template slot for the gist.
- Rewriting or cutting agent text.
- A plain-words refusal.
- A `matters` or label note.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | core `intake.rs`, `record.rs`; store `queue.rs` (step 13, shapes, choices, decode); CLI `main.rs`, `intake.rs`, `decide.rs`, `round.rs`, `inbox.rs`, `card.rs`, `review.rs`, `state_file.rs` (format 7), `lib.rs`; MCP `intake.rs`, `hub.rs`, `mirror.rs`, `review.rs` (the form); `plugin.json`, `ask-owner/SKILL.md`, `scripts/deploy/.env.example` |
| test-engineer, Rust | `plain_questions.rs` (CLI, MCP); core and store units; the churn; a format 6 dump under `fixtures/`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`); http `daemon_keys.rs`, `spec_ids_hub.rs`; plugin tests |
| ui-developer | "The UI"'s files and their tests |

Comments cite `docs/features/plain-questions.md` "Acceptance criteria" with a criterion ID, or a canon heading; never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after RM-0061 `pack`: one Rust slice at a time on `main`. RM-0060 follows this slice.
2. The ui-developer adds the types behind `AHEAD_OF_FIXTURE` once design-sweep (a) has merged, then the screens.
3. The rust-developer works through core, store, the CLI (approve, round, inbox, card, backup), MCP, then the texts. The Rust test-engineer overlaps: PQ1-PQ8, then PQ9-PQ11 with the churn. Then the review.
4. Ship: the spec-writer ("Canon at ship").

### Risks

- The schema step: a hub needs a dump, an upgrade and the owner's OK.
- Text room: `ask-owner` 4 090 of 4 096, the descriptions 2 031 and 2 044 of 2 048.
- The Inbox dialog's preselection goes for discrepancies too (D4): those tests change.
- First-load: about 2-5 kB on the 823.88 kB of RM-0103, plus design-sweep's.

### Canon at ship

Free bytes now are in brackets; "net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/plain-questions.md` (new, Tier 2) | fields, caps, the note, shapes, choices, the form's and sheet's lines, the UI lead, Known limits, Tests |
| `agent-intake.md` [562] | "Tools": `ask_question`'s four fields; "Stored": three shapes, step 13, a pointer |
| `decision-record-template.md`, `decision-record.md` | "Flags": by fields (D3); "Queue and documents": a question's `option` |
| `proposal-queue.md` [5], `queue-backup.md` [52], `card-questions.md` [225] | the row for 13; format 7 and 6's import; the `items` keys, the Screen's lead (each net not positive) |
| `review-form.md`, `round.md` [351], `round-sheet.md`, `round-ui.md`, `proposal-review.md`, `agent-intake-documents.md` | line 5 and the choices; the JSON, sheet and answers; the example; controls by structure; the keys |
| `ui-languages.md` [9], `ui/README.md` | "Size": the figure (net not positive); the lead |
| `plugin-files.md`, `plugin.md` [316] | **Version** 0.43.0 with its keys; `ask-owner`'s row |
| crate READMEs | where they list `IntakeShape`, the formats or the schema, net not positive |
| `CLAUDE.md`; 08 s2 "Owner's word, 2026-11-12" row 8; RM-0088 | shipped (the half) |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner did not answer: D1, D2, D5 and D6, plus a `--option` answering a question as `choice {"option":N}`.

**The spec-writer's:**
- W1: the CLI takes `--options F` as JSON, as `--input`;
- W2: `[]` options are refused;
- W3: the note reads the gist only, and names one token;
- W4: the round sheet's `Gist:`, `Why it matters:` and `As asked:` frame words;
- W5: Activity shows the gist and why it matters; the options stay in Answer…, so `CardItem` gains no options;
- W6: D4 for discrepancies in the Inbox;
- W7: "Recommended by the asker" for any non-agent, with no login lookup;
- W8: `CardRaiseBody` unchanged, a deliberate divergence from the analysis's contract (D6).

**The owner's questions:**
- **Q-A.** Should the fields be required? Working answer: no. The cost of yes: older prompts, the CLI, the hub's callers and the page's dialogs all break.
- **Q-B.** Should RM-0088 be split, with its question half before RM-0060? Working answer: yes, as the owner's word of 2026-11-12 orders. The cost of no: writers' questions wait 8-14 h on collision work.
- **Q-C.** Should nothing be preselected in the Inbox for discrepancies too (D4)? Working answer: yes. The cost of keeping it for discrepancies: two rules for one control, and a structure check per kind.
