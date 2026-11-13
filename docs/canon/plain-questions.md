---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-11-13
---

# Plain questions: a question's gist, why it matters, its options

RM-0088's question half. A question may lead with a one-line `gist`, why it `matters`, two or three priced `options` and a `recommendation`; every surface shows them first and folds the agent's own text below, verbatim. The engine stores and folds, never rewrites (no LLM inside); a soft note asks for plain words and never refuses (ADR-0012). A question's option is staged and confirmed as a discrepancy's is (ADR-0035, ADR-0038, ADR-0054, ADR-0061). A decision and a discrepancy keep their shape; a question without the fields stores, reads and answers as before. Code: core `intake.rs` (caps, `gist_note`), `record.rs` (`Choice`); store `queue.rs` (step 13, shapes), `queue/stage.rs`; CLI `intake.rs`, `propose.rs` (`GistNames`), `decide.rs`, `round.rs`, `inbox.rs`, `card.rs`, `state_file.rs`; MCP `intake.rs`, `hub.rs`, `mirror.rs`, `review.rs`; UI `src/inbox/ItemLead.tsx`.

## Fields

| Field | `spec propose question` | Rule (exit 1 `<field>: <problem>`, nothing stored) |
|---|---|---|
| `gist` | `--gist G` | not blank, no LF or CR (`gist: holds a line break; give one line`), <= 160 bytes (`GIST_MAX`) |
| `matters` | `--matters M` | not blank, <= 512 (`MATTERS_MAX`) |
| `options` | `--options F`: a JSON array `[{label, effect, price}]`, a file or `-`, read as `--input` (UTF-8, <= 8 MiB; a bad shape exit 2) | 2-3 (`OPTIONS_MIN`, `QUESTION_OPTIONS_MAX`: `options: 4 option(s); give 2 to 3, each priced`, `[]` too), each as a discrepancy's (`label` <= 128, `effect`, `price` <= 512) |
| `recommendation` | `--recommendation N` | needs options (`recommendation: given without options`), an index into them (`recommendation: 2 is no index into the 2 options (0 to 1)`) |

Input order (`agent-intake.md` "Rules" 2): `node_ids`, `text`, `gist`, `matters`, `options`, `recommendation`, `working_answer`, `price_of_other`, `distinct_from`, `card_id`, the author; `QuestionInput`, `QuestionRequest`, `Intake` carry them. The dedup compares `text` alone (`agent-intake.md` "Dedup"): two questions differing only in gist hit. Agent text is data: stored as given, JSON raw, escaped in text (`agent-intake-documents.md` "Escaping and size"), never trimmed or cut; in text a CRLF `matters` folds to one line. The page's dialogs and `CardRaiseBody` send none of the four: agents and the CLI do.

## The note

Once the item passes rules 1-5, its gist is cut at Unicode whitespace, and the first flagged token adds the last note, once, stored or not, never on a refusal, escaped as notes are:

```text
gist: `RULE-STAM-REGEN` reads as code to the owner: say it in plain words; IDs belong in node_ids
```

Core `gist_note(gist, flags)` tokenises, trims, keeps the order and judges a path; the CLI's predicate judges the rest (core holds no language's syntax). A token is flagged when:
1. shed of `` `"'()[]{}<>,.;:!? `` at both ends, it ends in `.md`, ASCII case-insensitive (core);
2. as written, it holds `::` (CLI);
3. shed, it names a node of the index the call refreshed (CLI `GistNames`): rule 3 resolves it, or it is a legacy alias (an `aliases_from` prefix) whose canonical ID resolves, or an alias its document declares. A look-alike, mixed script, `project:`, an unclean path or anything unknown is no flag.

It is named shed, else as written.

## Shapes

Queue step 12 -> 13 (`STEP_13`): `ADD COLUMN gist TEXT`, then `matters`, in `migrate`'s one transaction: `QUEUE_SCHEMA_VERSION` 13, `PROPOSAL_COLUMNS` 50 (`proposal_columns(12)` 48). `IntakeShape` is `{Question, Decision, Discrepancy}`, each kind its own (`ProposalKind::intake_shape()`); the dedup reads `family()`: `Asks` (question, decision) or `Reports` (discrepancy), as before.

| Shape | Holds none of (stored: corrupt, named; given: `Invalid`) | Requires |
|---|---|---|
| `Question` | `gap_type`, `evidence` | `working_answer`, `price_of_other` |
| `Decision` | those, `options`, `recommendation`, `gist`, `matters` | as a question |
| `Discrepancy` | `price_of_other`, `gist`, `matters` | `gap_type`, `recommendation` |

A question's `recommendation` without options is corrupt (`` the stored `recommendation` cannot be read: it is set without options ``); its range and the options' JSON are checked as a discrepancy's. `choice_fits` and the `choice` decode take a question's `{"option":N}` in range, `{"working_answer":true}` and `{"answer":"…"}`; a decision the last two; a discrepancy `option` alone. A stage is judged by the row's fields (`Stage::problem_holding`): `` a staged `option` on a question, which holds no options ``, `` a staged `option` 5 on a question, which holds options 0-1 `` (a discrepancy's range too); `Stage::problem`: `` a staged `option` and `answer` on a question, which takes one of them ``. Backup format 7: `queue-backup.md` "Format".

## Answers

By the item's fields, never its kind (`decision-record-template.md` "Flags"): options give each option; `price_of_other` gives the working answer and an own answer. So a question with options takes `--option N`, its working answer (no flag) or `--answer T`; a decision the last two; a discrepancy `--option` alone. Approve, stage, form, round and the UI go through `first_checks`:
- `--option` on one without options, exit 2: `` `--option` names an option, and `PR-0001` is a question with none: its working answer, or `--answer T`, answers it; nothing changed `` (`a decision with none` alike);
- `--option` with `--answer`, exit 2: ``spec: `--option` and `--answer` each name the owner's choice, and `PR-0014` takes one: give either; nothing changed``;
- past the options, exit 1 naming `0-1`; `--answer` without `price_of_other`, an open discrepancy without `--option`: as before.

A question's chosen option fills `title`, `choice`, `effect`, `cost` as a discrepancy's does; `{{options}}` lists a question's options when held, `{{summary}}` is its text. No slot is added, so older `spec approve` builds read every template. A task package assumes the working answer of an item with `price_of_other` (`task-package.md`).

## Form and sheet

- **`review_proposal`** (`review-form.md` "Form"): a deciding item's line 5 is `gist: …` and `matters: …` when held, `summary: …`, each option, then `working answer: …` when `price_of_other` and a working answer are held. Choices: each option (`` (recommended)``), `working-answer` when `price_of_other` is held, `reject`.
- **`spec round`** (`round-sheet.md`): a held gist prints `Gist: <gist>` after the heading, `Why it matters:` when held, then the text under `As asked:`; without one, the text, then why it matters. An item with `price_of_other` shows its options before `Working answer:`, a discrepancy after, as before. On a question with options `N` approves `--option N`, `ok` the working answer, other text `--answer`; an option-less one refuses `N` as before. The JSON item has 21 keys.

## Documents

| Shape | Gains, after `summary` | Keys |
|---|---|---|
| review document, daemon `Proposal`, MCP `ReviewDocument` (`get_proposal`, `review_proposal`, `propose_change`'s output) | `gist`, `matters`; text lines `gist:`, `matters:` after `summary:` | 53 |
| `spec inbox` JSON, `InboxEntry` | `gist`; the text's last column `<severity>: <gist, else the summary's first line>` | 13 |
| round item, `RoundItem` | `gist`, `matters` | 21 |
| card `items`, `CardItem`, MCP `CardItemShape` | `gist`, `matters`; the text `items:` line the gist, else the summary | 18 |
| a parent's `subtask_items`, `CardSubtaskItem` | `card_id`, then `items`' 18 | 19 |

A question's `options`, `recommendation` fill the existing keys; every other kind serves `gist`, `matters` `null`. No route, event or key set is added; `fixtures/daemon-keys.json` names them.

## MCP and plugin

- `ask_question` (stdio `QuestionArgs`, hub `HubQuestionArgs`) takes `gist`, `matters`, `options` (items closed, `mirror::IntakeOption`) and `recommendation` after `text`, nullable, property docs without digits. `QUESTION_DESCRIPTION` 2 031 B, `HUB_QUESTION_DESCRIPTION` 2 044, `get_card`'s 2 047 (its 18 item keys), ASCII, within 2 048; `holds_number` ties 160, 512, 2 and 3 to the constants. `INSTRUCTIONS`, `HUB_INSTRUCTIONS` unchanged; 30 stdio tools, 18 on a hub.
- Plugin 0.43.0 (`plugin-files.md` "Changing it"): `ask-owner` teaches a question to lead with a plain `gist` and why it `matters`, then priced `options` and a `recommendation` (`plugin.md` "Skills").

## UI

`src/inbox/ItemLead.tsx` (`ItemLead`, `LeadMatters`, `AskedFold`, `AskedText`, `RecommendedMark`), on the Inbox card and its dialogs, the Questions screen, a card's Activity and Answer…:
- **The lead**, for an item holding `gist`: the gist on one line in `<bdi>` through `shownSlot` (also the Inbox `h2`, a list row, the not-staged line; the filter searches it), "Why it matters" (`shownText`, line feeds kept), then the options and controls; the text, working answer, price and why in a `Disclosure`, "The question as asked", collapsed. A question with no options shows its working answer and price after "Why it matters", outside the fold, which keeps the text, the pair and the why. An item without a gist renders as before. Every list draws an entry through `inbox/ProposalSummary.tsx`: a held gist so, else the summary (the Inbox list and a card's others, the Spec tree's proposals, Health's left region, the palette).
- **Who recommends**: `labels.ts` `recommendedBy(author, t)`, the one place an author's type is compared (`policy.test.ts`): an agent "Recommended by the AI", anyone else "Recommended by the asker". Never preselected, the Inbox's decision dialog included, discrepancies too: Accept stays off with "Pick an answer first" until a pick, which a revision clears.
- **Controls by structure** (`round/drafts.ts` `answersOf`, shared by `offers`, `RoundItemCard`, `CardItems` `picksOf`): each option; the working answer when `price_of_other` and `working_answer` are held; an own answer with `price_of_other` (not in the Inbox: `decision-staging.md` "UI"); Reject; Leave open on the Questions screen. An item with neither options nor a price offers the working answer and an own answer, as before.
- **Strings**: `inbox.lead.{matters, asked, recommendedByAgent, recommendedByAsker, pickFirst}` in both catalogs, the Russian a draft for the owner; `.lead-*` CSS on scale tokens. Mock: `?scenario=plain-questions` (`src/mocks/plainQuestions.ts`), the approve lines in `mocks/approveLines.ts`.

## Known limits

- **Rollout**: older binaries refuse a schema 13 queue (`SchemaTooNew`; the hook gives no verdict until reinstall) and a format 7 dump; an older hub's `/mcp` refuses the four keys (`unknown field`). Every terminal, plugin and hub upgrades from one commit, a hub dumping first (`card-mandate.md` "Known limits"), the owner's call.
- The Inbox's `DecisionDialog` and `ConfirmDialog` still draw their summary line raw, outside `ProposalSummary`: a follow-up.
- A stored `options: []` on a question reads as fine, though the intake refuses `[]`.
- The note flags `QST-031@1` but not `MEC-STAMINA@2`; `../x.md` is named `/x.md`, the trim shedding the dots.

## Tests

`plain_questions.rs` in core, store, CLI, MCP and http; store `format.rs`, `queue_stage.rs`; CLI `decision_apply.rs`, `queue_state.rs`, `round.rs`; MCP `mcp_review_form.rs`, `mcp_intake.rs`, `mcp_hub.rs`, `mcp_cards.rs`, `plugin_files.rs`, `plugin_skills.rs`; http `daemon_keys.rs`; UI `src/inbox/ItemLead.test.tsx`, `mocks/plainQuestions.smoke.test.tsx`, `api/daemonKeys.test.ts`, `policy.test.ts`. Criteria: `docs/features/plain-questions.md`.
