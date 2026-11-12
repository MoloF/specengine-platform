---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-store, ui]
owner: owner
reviewed: 2026-11-13
---

# Agent intake: the documents it answers with

What the intake commands and tools print and return (`agent-intake.md`: tools, rules, dedup, rows): the intake document, the review document's intake keys, a collision's pair, escaping and size. Code: CLI `intake.rs` (`render_text`), `review.rs`; MCP `mirror.rs`.

## Intake document

`{id, created, hits, related, linked, diagnostics, notes}`: `id` the new `PR-...` or `null`; a match `{id, source: corpus|queue, status, path, answer, record}` (corpus: the decision's `id:` or `null`, `accepted`, its path, its indexed title; queue: `PR-...`, its status, `null`, a rejected one's reason; an applied one: `decision-record.md`; related: `answer`, `record` `null`); `linked` the update's ID, `diagnostics` its findings (20, then `<k> more introduced finding(s): spec review PR`); `notes` also `note:` lines. Text: the ID, `linked: PR-...`, a patch's `introduced: <n>` and finding lines, a `hit:`/`related: <id or path> | <status> | <path or -> | <answer's first line or ->` line each; not stored: ``not stored: name every hit in `distinct_from` to store it anyway``. Exit 0 stored or not; 1 refused (no text, `--json` the document).

```
ask_question {"node_ids": ["Q-031"], "text": "Does regeneration wait for rest?", "working_answer": "yes, 1.5 s",
  "price_of_other": "R-28 rebalanced", "author_role": "developer"}
-> {"id": null, "created": false, "hits": [{"id": "DEC-0023", "source": "corpus", "status": "accepted",
  "path": "docs/records/DEC/DEC-0023.md", "answer": "Regeneration waits for rest", "record": "DEC-0023"}], "related": [], "linked": null,
  "diagnostics": [], "notes": []}
```

## Review document

The fourteen keys after `updated_at`, of the 54 (`proposal-review.md` "Review document"; then a record's five: `decision-record.md`; stages: `decision-staging.md`): `target_ids, severity, gap_type, summary, gist, matters, working_answer, price_of_other, evidence, pair, options, recommendation, distinct_from, linked` (`gist`, `matters` a question's, text lines after `summary:`: `plain-questions.md` "Documents"; `pair`: "Pair"); `recommendation` an integer, lists `[]` when absent (an update's `target_ids` `[target_id]`), scalars `null`. Text: `key: <n>`, an indented line per item: `[i] label | effect | price` (+ ` (recommended)`), `file[:lines][ qpath] | observed | documented`. Deciding kinds: `diff`, `preview`, `conflict`, `revisions` `null`, no apply step run. `revisions` (after `conflict`): `proposal-revisions.md` "Review". After `interpretation`, `card_id` (a deciding kind's card) and `why` (a decision's), `null` where absent; text `card_id: C-0007`, `why: <text>` after `staged_at`'s line (`card-questions.md` "Store"). After `why`, `similar`: a list on a create's propose answer, `null` everywhere else, no line in `spec review`'s text (`proposal-similar.md`).

- **`--brief`** (`propose update`, `review`; `propose_change`, `get_proposal`): `base_text`, `new_text`, `diff`, `conflict` `null`; 20 `diagnostics` (`SHOW_TAIL_NAMES`), note `<k> more introduced finding(s): spec review PR`; text over `OUTPUT_CAP_CHARS`: the whole lines that fit (a longer first one cut), then `[truncated: <k> of <n> lines not shown: spec review PR]`. A dry run has no `PR`: both tails read `spec propose's dry run without --brief lists them`.
- **Inbox**: entries gain `severity`, `summary` (`null` for an update), `gist`, `rationale` `null` for the deciding kinds; their last column `<severity>: <gist, else the summary's first line>`, cut at 80 characters as the rationale.

## Pair

A two-sided collision as two documents. `pair` (core `pair.rs`, pure; CLI `round.rs` `pair_served`, `pair_of`, `review.rs` `review_pair`) is computed per read, never stored, for a `discrepancy` whose `gap_type` is `contradicts` with exactly two `target_ids` (`pair_applies`, `PAIR_TARGETS`), any status; every other item `null`. `{left, right, other}`, a side `{id, title, path, evidence}`: `left` is `target_ids[0]`, `right` `[1]`, each resolved now as round resolves a target (`round.md` "JSON": `id` `null` for a path target, `title` `null` when gone; a gone first target keeps its stored `target_path`, a gone second `null`), so a rename shows on the next read with no queue write. A side's `evidence` is the positions in `evidence` whose `file` equals its `path` byte for byte (`./docs/a.md` is not `docs/a.md`; one path on both sides: under both; `path` `null`: none), `other` the rest, each ascending: every position is in one of the three, no quote served twice.

- **Where**: review documents (`spec review`, `--brief`, `get_proposal`, `review_proposal`, `GET .../proposals/:id`) and `spec round --json` items. Propose, approve, reject and stage answers serve `null`; the intake document (`spec propose discrepancy --json`, the MCP intake tools) has no `pair` key; `InboxEntry`, `CardItem` none (no evidence there). Targets are resolved only when it applies: `subset.rs`'s seam counts `pair_targets`, 0 for every other item, 2 for a pair. An index that does not read gives `null` and the note `pair unknown: <error>`, exit 0. MCP `Pair`, `PairSide` closed, `evidence` and `other` integer arrays.
- **Text**, after the evidence lines and only when not `null`, escaped as the rest; a side `<id, else path> | <title, else -> | <path, else -> | evidence <i, j, else ->`, indices counting the evidence lines from 0. The example, `fixtures/spec-a`'s two rules with evidence in `sprint.md`, `stamina.md` and `src/stamina.rs`, filed with `distinct_from: ["DEC-0023"]` (else the corpus hit leaves it unstored):

```text
pair:
  left RULE-SPRINT-COST | Cost | docs/spec/movement/sprint.md | evidence 0
  right RULE-STAM-REGEN | Regeneration | docs/spec/movement/stamina.md | evidence 1
  other | evidence 2
```

- **UI** (`src/inbox/`): `PairSlot` stands the lazy `PairView` in a `Suspense` with a skeleton, in place of the flat evidence list whenever `pair !== null` (the key decides, never the kind): the Inbox's `ProposalCard` after the summary (a gist held: inside `AskedFold`, after the text), Questions' `RoundItemCard`, a card's Answer... (`CardItems.tsx`). `PairView` lays out what is served, never grouping, counting or matching (`architecture.md` "Interface"): one region, "What conflicts with what" (`inbox.pair.label`); two sides in a grid (`.collision-pair-*`), stacked when narrow; each headed `h3` by its title, else its ID or path (`lang="en"`), a button opening the document modal (`openDocFrom`, focus back on close), none with neither title nor path; a gone side adds "Not in the spec now" (`inbox.pair.gone`), its heading not repeated under it; the ID and path, mono; its evidence (`EvidenceItem`, shared with the flat list), or `inbox.pair.noEvidence` when no position resolves; "Other evidence" only when one of `other` resolves. Served text in `<bdi>` through `shownSlot`, `shownText`, the flat list too. Mock `?scenario=collision-pairs`.
- **Known limits**: a found target with no title (a path target with no H1) shows as gone, `PairSide` having no found flag; each pair review reads the whole index to resolve its two targets; three or more targets, or another gap type, stay a flat list.
- **Tests**: core `pair.rs`; `collision_pairs.rs` in CLI, MCP and http; http `daemon_keys.rs`; UI `inbox/PairView.test.tsx`, `mocks/collisionPairs.smoke.test.tsx`, `app/lazy.test.ts`, `api/daemonKeys.test.ts`. Criteria: `docs/features/collision-pairs.md`.

## Escaping and size

The new fields go through `escape_controls` (text, prompts, stderr: inbox, review, intake, `--input` errors), JSON raw. An update's brief drops the texts; at every cap with each byte escaping x6, a deciding kind's brief and text stay < 300 000 characters: every result <= `MAX_RESULT_CHARS`, `content` <= 48 000 (measured). Residue: ID, path, reason lengths (64 hits and 64 related at 600-byte paths pass 48 000). A pair, measured on the example: its text 186 B, its compact JSON 232 B, `"pair":null,` 12 B, the whole `--brief` 1 719 B.
