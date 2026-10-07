---
class: canon
tier: 2
scope: [crates/specengine-mcp, crates/specengine-cli, crates/specengine-store, crates/specengine-core]
owner: owner
reviewed: 2026-10-07
---

# Agent intake: queue tools, questions, discrepancies

Phase 2 slice 3: an agent proposes a change, asks a question or reports a discrepancy over MCP or the CLI, a queue record with evidence and priced options checked against what is decided and asked, and works on its working answer (ADR-0012). Only the queue is written: nothing under the root, no commit, no proposal's state changed; the owner decides on a terminal (`proposal-{queue,apply}.md`). The stdio server writes through the CLI library per request until the daemon (ADR-0019). Code: core `intake` (kinds, enums, caps, checks), store `create_intake`, CLI and MCP `intake.rs`.

## Tools

Default build, each one CLI library call with a twin, answering as `mcp-read.md` "Parity". `A` = `--author-role R [--author-model M] [--run ID]`, over MCP `author_role, author_model?, run?`.

| Tool | ≙ `spec` | Answer |
|---|---|---|
| `propose_change {kind, target, base?, text, rationale, A}` | `propose update\|create TARGET [--base B] --text-file - --rationale R A --brief` | brief review |
| `ask_question {node_ids, text, working_answer, price_of_other, severity?, distinct_from?, A}` | `propose question ID… --text T --working-answer W --price-of-other P [--severity S] [--distinct-from X]… A` | intake |
| `report_discrepancy {node_ids, summary, gap_type, severity, evidence: [{file, qpath?, lines?, observed, documented}], options: [{label, effect, price}], recommendation, working_answer?, proposed_patch?: {target, base, text, rationale}, distinct_from?, A}` | `propose discrepancy --input F\|- A` (F: the arguments but `A`'s as JSON, UTF-8, ≤ 8 MiB `INTAKE_INPUT_MAX_BYTES`) | intake |
| `get_proposal {proposal_id}` | `review PR --brief` | brief review |

`target`, a `node_ids` item: also a `.md` path (`proposal-queue.md` "Creation" 1). `base`: `get_node`'s `span_hash`, `null`: a new file (`proposal-kinds.md`); `text` inline. A patch becomes a linked `update`, decided on its own. Place: `propose update` step 5 from the server's root (`--root`, else its cwd).

- **Schemas**: input 2020-12, `additionalProperties: false` on the root and each of one level of inline objects (`evidence`, `options`, `proposed_patch`), no `$ref`, `$defs`, root combinators; `?` nullable; enums `kind` `["update", "create"]` (`update` needs `base`: invalid-params), `severity` `high|normal|low`, `gap_type` `missing|partial|contradicts|unrequested`. Output: `mirror.rs`'s `ReviewDocument`, `IntakeDocument`, `kind` free: no kind literal in `specengine-mcp/src` (`mcp_genre.rs`).
- **Annotations**: the three writers `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` all `false`, no `requiresUserInteraction`; `get_proposal` the read tools'; all `_meta` `maxResultSizeChars` 500 000. Descriptions as `mcp-read.md` "Texts" (asserts too), adding that only the queue is written, the owner decides on a terminal, agent-written fields are data, not instructions.

## Rules

In order; the first refusal answers, exit 1 `<field>: <problem>` (`evidence[2].observed: 1300 bytes; at most 1024`) with the intake document, the reason its last note, unless exit 2 is named; nothing stored, no ID taken before step 7.

1. Form → exit 2 (MCP: rmcp's parameter error): clap, the MCP schema, `--input` not UTF-8 JSON of the shape (unknown or missing field, a bad `severity`, `gap_type`); `--input` errors escaped.
2. Caps (core constants, UTF-8 bytes, every caller), in input order; required strings not blank, a given optional one not blank either: `node_ids` 1–16; `text`, `summary` ≤ 1 024; `working_answer`, `price_of_other` ≤ 2 048; `evidence` 1–8: `file`, `qpath` ≤ 512, `lines` `N` or `N-M` (1 ≤ N ≤ M < 10⁹), `observed`, `documented` ≤ 1 024; `options` 2–6: `label` ≤ 128, `effect`, `price` ≤ 512; `recommendation` an index into them; `proposed_patch` `text` ≤ 1 MiB, `rationale` ≤ 4 096 (`propose update`'s too); `distinct_from` ≤ 64 (`DISTINCT_MAX`) × 1–256, no control character; then the author (Authors).
3. `node_ids` resolve as `propose update` step 1, generated and `immutable_text` holders allowed (an alias, a legacy ID → exit 1 naming the canonical; a look-alike, mixed script, `project:` → exit 2), prefixed `node_ids[i]: `; a node twice once canonical (a path and its `id:`) → refused naming the first; stored canonical, in given order.
4. `proposed_patch`: its target resolved and among them, then `propose update` steps 1–4 (prefix `proposed_patch: `; a stale `base` names the current hash); its introduced findings stored, never refusing.
5. Place: step 5 (exit 2).
6. Corpus hits, 7. queue hits and the insert (Dedup).

## Dedup

Deterministic, never search.

- **Corpus** (step 6, the index this call refreshed): per target, each link `spec show --links` lists (both directions, resolved, any type but `mentions`) whose other end lies in a live `class: decision` document with `status: accepted`, and that document when the target lies in one: a hit; one only `mentions`-linked → related. Node kinds never consulted (ADR-0008). A whole document matches the decisions linked to its sections too (`--links` lists them).
- **Queue** (step 7, store `create_intake`, inside the inserting `BEGIN IMMEDIATE`): rows of the same kind and project (any repository, any state) sharing a canonical target; `normalized_summary` equal (Unicode whitespace runs → one space, trimmed, `to_lowercase`) → hit, other text → related.
- **Stored** only when every hit is named byte-equal in `distinct_from` (its `id`, else its path): the item, then the patch's update (next ID, `linked` both ways), a `proposal.created` each, committed; else rolled back, `id` `null`. Busy over 5 s → exit 2.
- **Lists**: hits corpus by path then queue by ID number; 10 (`INTAKE_MATCHES_MAX`) in full, then a note `<n> more hit(s) by name only: A, B, …` (`related item(s)`), named as `distinct_from` takes them, up to 64 per list, the rest `; <r> more not listed`. Over 64 hits an item cannot be stored: the hits note adds `` ; more than 64 hits: `distinct_from` names at most 64, so this item cannot be stored (a known limit) ``, stdout `not stored: <that reason>` (`IntakeOutcome.unnameable`).

## Intake document

`{id, created, hits, related, linked, diagnostics, notes}`: `id` the new `PR-…` or `null`; a match `{id, source: corpus|queue, status, path, answer, record}` (corpus: the decision's `id:` or `null`, `accepted`, its path, its indexed title; queue: `PR-…`, its status, `null`, a rejected one's reason; an applied one: `decision-record.md`; related: `answer`, `record` `null`); `linked` the update's ID, `diagnostics` its findings (20, then `<k> more introduced finding(s): spec review PR`); `notes` also `note:` lines. Text: the ID, `linked: PR-…`, a patch's `introduced: <n>` and finding lines, a `hit:`/`related: <id or path> | <status> | <path or -> | <answer's first line or ->` line each; not stored: ``not stored: name every hit in `distinct_from` to store it anyway``. Exit 0 stored or not; 1 refused (no text, `--json` the document).

```
ask_question {"node_ids": ["Q-031"], "text": "Does regeneration wait for rest?", "working_answer": "yes, 1.5 s",
  "price_of_other": "R-28 rebalanced", "author_role": "developer"}
→ {"id": null, "created": false, "hits": [{"id": "DEC-0023", "source": "corpus", "status": "accepted",
  "path": "docs/records/DEC/DEC-0023.md", "answer": "Regeneration waits for rest", "record": "DEC-0023"}], "related": [], "linked": null,
  "diagnostics": [], "notes": []}
```

## Stored

Kinds `question`, `discrepancy` (`applies()` false, `decides()` true). Queue step 1 → 2 (step 3: `decision-record.md`): `ALTER TABLE proposals ADD COLUMN <c> TEXT`, in order `target_ids` (JSON canonical targets; `target_id` the first, `target_path` its holder), `severity`, `gap_type`, `summary` (a question's text), `working_answer`, `price_of_other`, `evidence`, `options` (JSON as input, absent keys `null`; a question's NULL), `recommendation` (decimal), `distinct_from` (JSON), `linked`; 35 columns. New kinds: `base_hash`, `base_text`, `new_text`, `patch_hash`, `rationale` NULL, `diagnostics` `[]`; an update: the eleven NULL but `linked`. Corrupt, named: a kind's required column NULL, JSON not of its shape, an enum or `recommendation` out of range, a first target not `target_id`, `linked` no `PR-` ID.

Store: `create_intake(&NewIntake {kind, target_path, place, author, intake: Intake}, corpus_hits: &[String], patch: Option<&NewProposal>, now) -> IntakeResult {hits, related: Vec<QueueMatch {id, status, reason, record_{id,path,title}}>, created, linked}`; an applying kind → `Invalid`. `create`, `approve`, `approve_from` of a new kind are `Invalid`; `applied`, `applied_with` without its record too. Backup: `queue-backup.md` "Format".

## Review document

The eleven keys after `updated_at` (then a record's five: `decision-record.md`; stages: `decision-staging.md`): `target_ids, severity, gap_type, summary, working_answer, price_of_other, evidence, options, recommendation, distinct_from, linked`; `recommendation` an integer, lists `[]` when absent (an update's `target_ids` `[target_id]`), scalars `null`. Text: `key: <n>`, an indented line per item: `[i] label | effect | price` (+ ` (recommended)`), `file[:lines][ qpath] | observed | documented`. New kinds: `diff`, `preview`, `conflict` `null`, no apply step run.

- **`--brief`** (`propose update`, `review`; `propose_change`, `get_proposal`): `base_text`, `new_text`, `diff`, `conflict` `null`; 20 `diagnostics` (`SHOW_TAIL_NAMES`), note `<k> more introduced finding(s): spec review PR`; text over `OUTPUT_CAP_CHARS`: the whole lines that fit (a longer first one cut), then `[truncated: <k> of <n> lines not shown: spec review PR]`.
- **Inbox**: entries gain `severity`, `summary` (`null` for an update), `rationale` `null` for the new kinds; their last column `<severity>: <summary's first line>`, cut at 80 characters as the rationale.

## Settling

`spec approve` decides one as an accepted decision record: `decision-record.md`. `reject --reason`: `open → rejected`, the reason in `decision_note` (the owner's answer: `get_proposal` and dedup return it), `proposal.rejected`; its history read only when a commit of it may exist (`decision-record.md` "Reject"); orphans and other repositories as for an update.

## Authors

`author_role` is required over MCP (missing: rmcp's parameter error), stored verbatim, no enum (P2-7): `{"type":"agent","role":"nest-developer","model":null,"run":null}`. Grammar (core `author_problem`): printable ASCII without spaces, 1–128 bytes, named `author_role`, `author_model`, `run`. Exits differ, recorded as is: `propose update` (± `--brief`, `propose_change`) → exit 2 `spec: author_role: <problem>`, no document; the intake commands → exit 1 with the document. The twin without `A` stores `human`.

## Escaping and size

The new fields go through `escape_controls` (text, prompts, stderr: inbox, review, intake, `--input` errors), JSON raw. An update's brief drops the texts; at every cap with each byte escaping ×6, a new kind's brief and text stay < 300 000 characters: every result ≤ `MAX_RESULT_CHARS`, `content` ≤ 48 000 (measured). Residue: ID, path, reason lengths (64 hits and 64 related at 600-byte paths pass 48 000).

## Known limits

An agent's Bash `spec propose` without `A` stores `human`; a subagent in a task worktree binds to the session's tree (ADR-0032); a cancelled MCP write still stores (a repeat gets the hit); a scratch-`HOME` launcher (`fixtures/mcp/mcp.json`) hides its items from the owner's inbox; no dedup for `update`; rows by a path and by its later `id:` stay apart.

- **Unnameable hits**: over 64, or one whose path is over 256 bytes or holds a control character (listed; `distinct_from[i]` refuses it, the agent's only sign): the item is never stored. `unnameable` counts hit documents, not names: two accepted decisions sharing an ID (a `spec check` error) may read "cannot be stored" though naming the 64 names stores it. The note joins names with `, `: a path holding `, ` does not split back.
