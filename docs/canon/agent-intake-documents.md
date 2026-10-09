---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-store]
owner: owner
reviewed: 2026-10-09
---

# Agent intake: the documents it answers with

What the intake commands and tools print and return (`agent-intake.md`: tools, rules, dedup, rows): the intake document, the review document's intake keys, escaping and size. Code: CLI `intake.rs` (`render_text`), `review.rs`; MCP `mirror.rs`.

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

The eleven keys after `updated_at` (then a record's five: `decision-record.md`; stages: `decision-staging.md`): `target_ids, severity, gap_type, summary, working_answer, price_of_other, evidence, options, recommendation, distinct_from, linked`; `recommendation` an integer, lists `[]` when absent (an update's `target_ids` `[target_id]`), scalars `null`. Text: `key: <n>`, an indented line per item: `[i] label | effect | price` (+ ` (recommended)`), `file[:lines][ qpath] | observed | documented`. Deciding kinds: `diff`, `preview`, `conflict` `null`, no apply step run.

- **`--brief`** (`propose update`, `review`; `propose_change`, `get_proposal`): `base_text`, `new_text`, `diff`, `conflict` `null`; 20 `diagnostics` (`SHOW_TAIL_NAMES`), note `<k> more introduced finding(s): spec review PR`; text over `OUTPUT_CAP_CHARS`: the whole lines that fit (a longer first one cut), then `[truncated: <k> of <n> lines not shown: spec review PR]`.
- **Inbox**: entries gain `severity`, `summary` (`null` for an update), `rationale` `null` for the deciding kinds; their last column `<severity>: <summary's first line>`, cut at 80 characters as the rationale.

## Escaping and size

The new fields go through `escape_controls` (text, prompts, stderr: inbox, review, intake, `--input` errors), JSON raw. An update's brief drops the texts; at every cap with each byte escaping x6, a deciding kind's brief and text stay < 300 000 characters: every result <= `MAX_RESULT_CHARS`, `content` <= 48 000 (measured). Residue: ID, path, reason lengths (64 hits and 64 related at 600-byte paths pass 48 000).
