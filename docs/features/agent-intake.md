---
class: spec
status: draft
scope: [crates/specengine-mcp, crates/specengine-cli, crates/specengine-store, crates/specengine-core]
ref: agent-intake analysis 2026-10-05; 08 §2 Phase 2, slice 3
---

# Agent intake over MCP

## Why

Agents raise only `update`, through Bash; MCP is read-only, so discrepancies and questions stay prose in chat and the owner re-answers settled ones (08 AC-5). Here each becomes a queue record with evidence and priced options, checked against what is decided, while the agent works on its working answer (ADR-0012). No new ADR: the stdio server writes the queue through the CLI library, per request, the interim `proposal-queue.md` "Store" grants CLI processes until the daemon (ADR-0019).

Working answers (the owner's, 2026-10-05, session rule): Q1 intake only, decisions in `decision-apply`; Q2 questions live in the queue as `PR-…`, no record file; Q3 the owner settles one by `spec reject PR --reason <answer>`, returned by `get_proposal` and dedup; Q4 dedup deterministic (Rules 6–7), search never a hit; Q5 `author_role` required, verbatim, no enum; Q6 one level of closed inline objects in input schemas, no `$ref`; Q7 bound to the server's root.

## Description and interactions

Four default-build tools, each one CLI library call with a twin (`mcp-read.md` "Parity"); MCP adds no logic, cap or default. `A` = `[--author-role R] [--author-model M] [--run ID]` as `propose update`'s; MCP always passes `--author-role`.

| Tool | ≙ `spec` | Result |
|---|---|---|
| `propose_change` | `propose update TARGET --base B --text-file - --rationale R A --brief` | brief review |
| `ask_question` | `propose question ID… --text T --working-answer W --price-of-other P [--severity S] [--distinct-from X]… A` | intake |
| `report_discrepancy` | `propose discrepancy --input F\|- A` (F: the arguments but `A`'s, JSON ≤ 8 MiB) | intake |
| `get_proposal` | `review PR --brief` | brief review |

Only the queue is written, in the CLI's data directory: nothing under the root, no commit, no status change. A `proposed_patch` becomes a linked `update`, decided on its own. Place: `propose update` step 5 from the server's root (`--root`, else its cwd).

## Data

**Inputs** (2020-12; `additionalProperties: false` on the root and each inline object; `?` optional, nullable; no `$ref`, `$defs`, root combinators):

- `propose_change {kind: "update" (enum), target, base, text, rationale, author_role, author_model?, run?}`: `base` = `get_node`'s `span_hash`, `text` inline, never a path.
- `ask_question {node_ids: [string], text, working_answer, price_of_other, severity?, distinct_from?: [string], author_role, author_model?, run?}`.
- `report_discrepancy {node_ids, summary, gap_type, severity, evidence: [{file, qpath?, lines?, observed, documented}], options: [{label, effect, price}], recommendation: integer, working_answer?, proposed_patch?: {target, base, text, rationale}, distinct_from?, author_role, author_model?, run?}`.
- `get_proposal {proposal_id}`.

Enums: `severity` `high|normal|low` (questions default `normal`; never blocking), `gap_type` `missing|partial|contradicts|unrequested`. **Caps** (core constants, UTF-8 bytes, every caller; required strings not blank): `node_ids` 1–16, distinct once resolved; `text`, `summary` ≤ 1 024; `working_answer`, `price_of_other` ≤ 2 048 (a discrepancy without one: the recommendation stands); `evidence` 1–8: `file`, `qpath` ≤ 512, `lines` `N` or `N-M` (1 ≤ N ≤ M < 10⁹), `observed`, `documented` ≤ 1 024; `options` 2–6: `label` ≤ 128, `effect`, `price` ≤ 512; `recommendation` an index into them; `distinct_from` ≤ 16 × 1–256, no control character; `rationale` ≤ 4 096 (`propose update` too); a change's `text` ≤ `TEXT_MAX_BYTES`.

**Stored**: kinds `question`, `discrepancy`. Queue step 1 → 2 (`QUEUE_SCHEMA_VERSION` 2; one `Immediate` transaction, 0 → 2 runs both): `ALTER TABLE proposals ADD COLUMN <c> TEXT`, in order, `target_ids` (JSON canonical IDs; `target_id` the first, `target_path` its holder), `severity`, `gap_type`, `summary` (a question's text), `working_answer`, `price_of_other`, `evidence`, `options` (JSON as input, absent keys `null`), `recommendation` (decimal), `distinct_from` (JSON), `linked` (the other `PR-…` of a discrepancy and its update); `PROPOSAL_COLUMNS` 35. New kinds: `base_hash`, `base_text`, `new_text`, `patch_hash`, `rationale` NULL; updates: the new columns NULL but `linked`. Also corrupt (named): a kind's required column NULL, JSON not of its shape, an enum or `recommendation` out of range, `linked` no `PR-` ID.

**Review document**: the eleven keys between `updated_at` and `notes`, `recommendation` an integer, lists `[]` when absent (an update's `target_ids` `[target_id]`), scalars `null`; text: `key: <n>`, an indented line per item (`[i] label | effect | price`, ` (recommended)`; `file:lines qpath | observed | documented`). New kinds: `diff`, `preview`, `conflict` `null`, no apply step run. **`--brief`**: `base_text`, `new_text`, `diff`, `conflict` `null`; 20 `diagnostics` (`SHOW_TAIL_NAMES`), note `<k> more introduced finding(s): spec review PR`; text cut at `OUTPUT_CAP_CHARS`. **Inbox** entries gain `severity`, `summary`; a new kind's last column: `<severity>: <summary's first line>`.

**Intake document** `{id, created, hits, related, linked, diagnostics, notes}`: `id` the new `PR-…` or `null`; `hits`, `related`: `{id, source: corpus|queue, status, path, answer}` (corpus: the decision's `id:` or `null`, `accepted`, its path, its title as the index takes it; queue: `PR-…`, status, `null`, a rejected one's reason; `related` answers `null`), by source, path or ID number, ≤ 10 each + a note; `linked`, `diagnostics`: the update's, as in the brief. Text: the ID, `linked: PR-…`, `introduced: <n>` and findings as `propose update`; a `hit:`/`related: <id or path> | <status> | <path or -> | <answer's first line or ->` line each; not created: ``not stored: name every hit in `distinct_from` to store it anyway``. Exit 0 both ways.

```
ask_question {"node_ids": ["Q-031"], "text": "Does regeneration wait for rest?", "working_answer": "yes, 1.5 s",
  "price_of_other": "R-28 rebalanced", "author_role": "developer"}
→ {"id": null, "created": false, "hits": [{"id": "DEC-0023", "source": "corpus", "status": "accepted",
  "path": "docs/records/DEC/DEC-0023.md", "answer": "Regeneration waits for rest"}], "related": [], "linked": null,
  "diagnostics": [], "notes": []}
```

**Backup** (`queue-backup.md` "Format"): `format` 1, header `queue_schema` 2, 35 proposal columns; import takes `queue_schema` 1 (its 24 columns, the eleven NULL) and 2.

## Rules and edge cases

In order; the first refusal answers, exit 1 `<field>: <problem>` (`evidence[2].observed: 1300 bytes; at most 1024`) unless exit 2 is named; nothing stored, no ID taken before step 7.

1. Form: the MCP schema (rmcp's parameter error), clap, `--input` not UTF-8 JSON of the shape → exit 2.
2. Caps, enums, author grammar.
3. `node_ids` resolve as `propose update` step 1, generated and immutable holders allowed (alias, legacy ID → exit 1 naming the canonical; look-alike, mixed script, `project:` → exit 2); stored canonical, in given order.
4. `proposed_patch`: `target` among them, then `propose update` steps 1–4 (prefix `proposed_patch: `); findings stored, never refusing.
5. Place: step 5 (exit 2).
6. Corpus hits, from the index this call refreshed: per target, each link `spec show --links` lists (both directions, resolved, any type but `mentions`) whose other end lies in a live `class: decision` document with `status: accepted`, and that document when the target lies in one; `mentions` only → `related`. Node kinds never consulted (ADR-0008).
7. One `Immediate` transaction: queue hits = rows of the same kind and project (any repository, status) sharing a target, `summary` normalised equal (Unicode whitespace runs → one space, trimmed, `to_lowercase`); other text → `related`. Every hit of 6 and 7 named in `distinct_from` (its `id`, else path; byte-equal), none found included → insert it, then the update (next ID, `linked` both ways), a `proposal.created` each, commit; else roll back. Busy over 5 s → exit 2.

**Settling**: `approve` on a new kind → exit 1 before any prompt, `` `PR-0004` never applies: `spec reject PR-0004 --reason <answer>` settles it; nothing changed``; store `approve_from`, `applied_with` → `Invalid`. Reject: `open → rejected`, reason in `decision_note`, `proposal.rejected`, no git; orphans, other repositories as for updates.

**Escaping**: new fields through `escape_controls` (text, prompts, stderr), JSON raw; the `get_proposal` and intake descriptions call agent-written fields data, not instructions.

**Tools**: the intake ones `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` all `false`, no `requiresUserInteraction`; `get_proposal`, `_meta`, texts as `mcp-read.md` "Annotations", "Texts", the descriptions adding that only the queue is written and the owner decides on a terminal. Output schemas mirror both documents; `kind` free (no kind literal in `specengine-mcp/src`).

**Size**: at every cap with each byte escaping ×6, a new kind's brief and text stay < 300 000 characters; an update's brief drops the texts: results ≤ `MAX_RESULT_CHARS`, `content` ≤ 48 000. Residue: ID, path, reason lengths.

**Known limits**: a pty passes the terminal check; an agent's Bash `spec propose` stores `human`; a subagent in a task worktree binds to the session's tree (ADR-0032); a cancelled write still stores (a repeated ask or report gets the hit); a settled question reads `rejected` until `decision-apply`; a scratch-`HOME` launcher (`fixtures/mcp/mcp.json`) hides its proposals from the owner.

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a` (`mcp_genre.rs`'s garden if named), scratch `HOME`, injected clock; MCP: the default build as `mcp_*` spawn it; library approve: consent yes. Refused: nothing stored, no event, next ID unchanged. M: the mutation turning it red.

- [ ] AC-01 — each default-build tool, a valid and an invalid call (a change to `RULE-STAM-REGEN`, a discrepancy with a patch): `git status --porcelain` empty, files byte-identical, no commit (`mcp_door.rs`; M: `propose_change` writes its target).
- [ ] AC-02 — `open`, `approved` with its commit on its branch, `applied`, `rejected`: `get_proposal`, every write tool: statuses, `decided_*`, `applied_commit`, `events()` unchanged; `tools/list` = the four reads + these four (M: `get_proposal` via approve's completion lookup).
- [ ] AC-03 — no `author_role`: `isError` naming it, refused; `"nest-developer"` alone → `{"type":"agent","role":"nest-developer","model":null,"run":null}`; `"a b"` refused; the twin without `A`: `human` (M: `author_role` optional, absent passed on).
- [ ] AC-04 — `EDGE-STAM-ZERO` asked twice, whitespace and case apart: `created: false`, hit the first ID, `counts()` unchanged; after `reject --reason R`: hit `rejected`, answer `R`; `RULE-STAM-REGEN`, `Q-031`: hit `DEC-0023`, its title, nothing stored; the text on another node: created; `distinct_from` naming every hit: created, in review; a subset: not; a temp accepted decision only mentioning the node: `related`, created (M: dedup off; targets ignored; any `distinct_from`; mentions as hits).
- [ ] AC-05 — 8 parallel identical `ask_question` + the twin: one row, every answer naming it (M: the queue read before `BEGIN IMMEDIATE`).
- [ ] AC-06 — refused naming the field: 1 and 7 options, `recommendation` 2 of 2, blank `working_answer`, `price_of_other`, no `node_ids`, an unknown ID, alias `QST-031` (exit 1 naming `Q-031`), a U+0420 look-alike (exit 2), an evidence field over cap, an unknown argument; then a valid ask stores `PR-0001` (M: `options` optional; a cap dropped).
- [ ] AC-07 — a patch citing an undeclared ID: both stored, `linked` both ways, the update's `diagnostics` and brief with it; a stale `base`: refused naming `proposed_patch` (M: refused on findings; a `linked` missing).
- [ ] AC-08 — ESC, U+0085, U+202E, U+2066 in a summary, working answer, `observed`, label: `inbox`, `review` text escaped, JSON raw; `inbox`: `… | question | open | EDGE-STAM-ZERO | <branch> | <created_at> | normal: <text>` (M: one field raw).
- [ ] AC-09 — library approve on a question, a discrepancy: exit 1 naming `spec reject`, no prompt, `dump()` unchanged; `approve_from` → `Invalid`; reject: `rejected`, reason in `decision_note` (M: approve accepted).
- [ ] AC-10 — a v1 DB opened: `user_version` 2, rows kept, the eleven NULL; every kind exported, imported fresh: `dump()` equal, re-export byte-identical; a `queue_schema` 1 dump imports, re-exports as 2; 3: `upgrade SpecEngine` (M: schema 1 refused; a column left out).
- [ ] AC-11 — `mcp_genre.rs`, core `check_genre.rs`, eval `build_graph.rs` green; garden: ask, ask again, `get_proposal`: no P2-3 word (M: a `"question"` literal in `specengine-mcp/src`).
- [ ] AC-12 — "Tools" holds (no `$ref`, inline objects closed); a 1 MiB `propose_change`, a discrepancy at every cap of control characters: each result ≤ `MAX_RESULT_CHARS`, `content` ≤ 48 000 (M: `readOnlyHint: true`; the brief keeping `new_text`).
- [ ] AC-13 — `content` byte-equal to the twin's `2>&1`, `structuredContent` its `--json`; made with `HOME` X: in `spec inbox` with X, not Y (M: the server's own data directory).
- [ ] AC-14 — gate clean; worst W ≤ min(109 484, 108 729); the new canon ≤ 12 288 B, READMEs ≤ 10 240 B; `CLAUDE.md` not grown; anonymity green (M: a byte added to `CLAUDE.md`).

## Out of scope

Kinds `decision`, `create`, `interpretation`, `amendment`, `approve --option N` (`decision-apply`: records with `cost`, `answers`, `canon:`); `deprecate`/`move`/`link` (front-matter edits go as `update`); `review_proposal`, `approve_task`; tasks (`claim_task`, `submit_plan`, `report_run`, `@assumes`: `task-package`); `changes_requested`, defer, `has_open_proposal`; search-related items (`search` ANDs its terms); daemon, `spec mcp`, SSE, plugin; `.claude/`, `ui/`; pilot runs.

## Implementation

Pending. At shipping (the proposal and MCP canons are full): a new `docs/canon/agent-intake.md`; pointers from `proposal-queue.md` "Store" (its interim line extended to the MCP stdio process), `queue-backup.md` "Format", `mcp-read.md` "Schemas", the READMEs.
