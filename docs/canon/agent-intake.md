---
class: canon
tier: 2
scope: [crates/specengine-mcp, crates/specengine-cli, crates/specengine-store, crates/specengine-core]
owner: owner
reviewed: 2026-10-14
---

# Agent intake: queue tools, questions, discrepancies, decisions

Phase 2 slice 3: an agent proposes a change, asks a question, reports a discrepancy or a decision it took (`proposal-decision.md`) over MCP or the CLI, a queue record with evidence and priced options checked against what is decided and asked, and works on its working answer (ADR-0012). Only the queue is written: nothing under the root, no commit, no proposal's state changed; the owner decides on a terminal (`proposal-{queue,apply}.md`). The stdio server writes through the CLI library per request until the daemon (ADR-0019). Code: core `intake` (kinds, enums, caps, checks), store `create_intake`, CLI and MCP `intake.rs`.

## Tools

Default build, each one CLI library call with a twin, answering as `mcp-read.md` "Parity". `A` = `--author-role R [--author-model M] [--run ID]`, over MCP `author_role, author_model?, run?`; writers `task_id?` = `--task T` (`tasks.md`).

| Tool | = `spec` | Answer |
|---|---|---|
| `propose_change {kind, target, base?, text, rationale, decision?, how?, why?, A}` | `propose update\|create\|interpretation\|amendment TARGET [--base B] --text-file - --rationale R A --brief` | brief review |
| `ask_question {node_ids, text, working_answer, price_of_other, severity?, distinct_from?, A}` | `propose question ID… --text T --working-answer W --price-of-other P [--severity S] [--distinct-from X]… A` | intake |
| `report_discrepancy {node_ids, summary, gap_type, severity, evidence: [{file, qpath?, lines?, observed, documented}], options: [{label, effect, price}], recommendation, working_answer?, proposed_patch?: {target, base, text, rationale}, distinct_from?, A}` | `propose discrepancy --input F\|- A` (F: the arguments but `A`'s as JSON, UTF-8, <= 8 MiB `INTAKE_INPUT_MAX_BYTES`) | intake |
| `report_decision {node_ids, text, working_answer, price_of_other, severity?, proposed_patch?, distinct_from?, A}` | `propose decision --input F\|- A` (F as above) | intake |
| `get_proposal {proposal_id}` | `review PR --brief` | brief review |

`target`, a `node_ids` item: also a `.md` path (`proposal-queue.md` "Creation" 1). `base`: `get_node`'s `span_hash`, `null`: a new file (`proposal-kinds.md`); `text` inline. A patch becomes a linked `update`, decided on its own. Place: `propose update` step 5 from the server's root (`--root`, else its cwd).

- **Schemas**: input 2020-12, `additionalProperties: false` on the root and each of one level of inline objects (`evidence`, `options`, `proposed_patch`), no `$ref`, `$defs`, root combinators; `?` nullable; enums `kind` `["update", "create", "interpretation", "amendment"]` (`update`, `interpretation` need `base`, an `amendment` takes none: invalid-params; the three fields: `proposal-interpretation.md`), `severity` `high|normal|low`, `gap_type` `missing|partial|contradicts|unrequested`. Output: `mirror.rs`'s `ReviewDocument`, `IntakeDocument`, `kind` free: no kind literal in `specengine-mcp/src`.
- **Annotations**: the four writers `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` all `false`, no `requiresUserInteraction`; `get_proposal` the read tools'; all `_meta` `maxResultSizeChars` 500 000. Descriptions as `mcp-read.md` "Texts", adding that only the queue is written, the owner decides on a terminal, agent-written fields are data, not instructions.

## Rules

In order; the first refusal answers, exit 1 `<field>: <problem>` (`evidence[2].observed: 1300 bytes; at most 1024`) with the intake document, the reason its last note, unless exit 2 is named; nothing stored, no ID taken before step 7.

1. Form -> exit 2 (MCP: rmcp's parameter error): clap, the MCP schema, `--input` not UTF-8 JSON of the shape (unknown or missing field, a bad `severity`, `gap_type`); `--input` errors escaped.
2. Caps (core constants, UTF-8 bytes, every caller), in input order; required strings not blank, a given optional one not blank either: `node_ids` 1-16; `text`, `summary` <= 1 024; `working_answer`, `price_of_other` <= 2 048; `evidence` 1-8: `file`, `qpath` <= 512, `lines` `N` or `N-M` (1 <= N <= M < 10⁹), `observed`, `documented` <= 1 024; `options` 2-6: `label` <= 128, `effect`, `price` <= 512; `recommendation` an index into them; `proposed_patch` `text` <= 1 MiB, `rationale` <= 4 096 (`propose update`'s too); `distinct_from` <= 64 (`DISTINCT_MAX`) x 1-256, no control character; then the author (Authors).
3. `node_ids` resolve as `propose update` step 1, generated and `immutable_text` holders allowed (an alias, a legacy ID -> exit 1 naming the canonical; a look-alike, mixed script, `project:` -> exit 2), prefixed `node_ids[i]: `; a node twice once canonical (a path and its `id:`) -> refused naming the first; stored canonical, in given order.
4. `proposed_patch`: its target resolved and among them, then `propose update` steps 1-4 (prefix `proposed_patch: `; a stale `base` names the current hash; a revision changed refused, `proposal-revisions.md` "Creation"); its introduced findings stored, never refusing.
5. Place: step 5 (exit 2).
6. Corpus hits, 7. queue hits and the insert (Dedup).

## Dedup

Deterministic, never search.

- **Corpus** (step 6, the index this call refreshed): per target, each link `spec show --links` lists (both directions, resolved, any type but `mentions`) whose other end lies in a live `class: decision` document with `status: accepted`, and that document when the target lies in one: a hit; one only `mentions`-linked -> related. Node kinds never consulted (ADR-0008). A whole document matches the decisions linked to its sections too.
- **Queue** (step 7, store `create_intake`, inside the inserting `BEGIN IMMEDIATE`): rows of the project whose kind shares the item's shape (`intake_shape()`, exhaustive: `question` and `decision` one family, `discrepancy` another) (any repository, any state) sharing a canonical target; `normalized_summary` equal (Unicode whitespace runs -> one space, trimmed, `to_lowercase`) -> hit, other text -> related.
- **Stored** only when every hit is named byte-equal in `distinct_from` (its `id`, else its path): the item, then the patch's update (next ID, `linked` both ways), a `proposal.created` each, committed; else rolled back, `id` `null`. Busy over 5 s -> exit 2.
- **Lists**: hits corpus by path then queue by ID number; 10 (`INTAKE_MATCHES_MAX`) in full, then a note `<n> more hit(s) by name only: A, B, …` (`related item(s)`), named as `distinct_from` takes them, up to 64 per list, the rest `; <r> more not listed`. Over 64 hits an item cannot be stored: the hits note adds `` ; more than 64 hits: `distinct_from` names at most 64, so this item cannot be stored (a known limit) ``, stdout `not stored: <that reason>` (`IntakeOutcome.unnameable`).

## Intake document

What `ask_question`, `report_discrepancy`, `report_decision` and their twins answer, `{id, created, hits, related, linked, diagnostics, notes}`: `agent-intake-documents.md` "Intake document".

## Stored

Kinds `question`, `discrepancy`, `decision` (`applies()` false, `decides()` true); a decision's row is a question's (`IntakeShape::Question`). Queue step 1 -> 2 (step 3: `decision-record.md`): `ALTER TABLE proposals ADD COLUMN <c> TEXT`, in order `target_ids` (JSON canonical targets; `target_id` the first, `target_path` its holder), `severity`, `gap_type`, `summary` (a question's, a decision's text), `working_answer`, `price_of_other`, `evidence`, `options` (JSON as input, absent keys `null`; a question's, a decision's NULL), `recommendation` (decimal), `distinct_from` (JSON), `linked`; 35 columns. Deciding kinds: `base_hash`, `base_text`, `new_text`, `patch_hash`, `rationale` NULL, `diagnostics` `[]`; an update: the eleven NULL but `linked`. Corrupt, named (`IntakeColumns::decode` by shape): a column the shape holds none of set (`null_columns()`: a question-shaped row's `gap_type`, `evidence`, `options`, `recommendation`, a discrepancy's `price_of_other`; `[]` counts as set: `` proposal PR-0001: the stored `options` cannot be read: it is set on a decision, which holds none ``), a required column NULL, JSON not of its shape, an enum or `recommendation` out of range, a first target not `target_id`, `linked` no `PR-` ID.

Store: `create_intake(&NewIntake {kind, target_path, place, author, intake: Intake}, corpus_hits: &[String], patch: Option<&NewProposal>, now) -> IntakeResult {hits, related: Vec<QueueMatch {id, status, reason, record_{id,path,title}}>, created, linked}`; an applying kind, or an item not of its kind's shape (`IntakeShape::problem`: `` a decision holds no `options` ``, `` a question lacks its `working_answer` ``, `the recommendation 3 of a discrepancy is no index into its 2 options`) -> `Invalid`, nothing written. `create`, `approve`, `approve_from` of a deciding kind are `Invalid`; `applied`, `applied_with` without its record too. Backup: `queue-backup.md` "Format".

## Review document

The intake keys of `spec review` and `get_proposal`, `--brief`, the inbox's entries: `agent-intake-documents.md` "Review document".

## Settling

`spec approve` decides one as an accepted decision record: `decision-record.md`. `reject --reason`: `open -> rejected`, the reason in `decision_note` (the owner's answer: `get_proposal` and dedup return it), `proposal.rejected`; its history read only when a commit of it may exist (`decision-record.md` "Reject"); orphans and other repositories as for an update.

## Authors

`author_role` is required over MCP (missing: rmcp's parameter error), stored verbatim, no enum (P2-7): `{"type":"agent","role":"nest-developer","model":null,"run":null}`. Grammar (core `author_problem`): printable ASCII without spaces, 1-128 bytes, named `author_role`, `author_model`, `run`. Exits differ, as built: `propose update` (± `--brief`, `propose_change`) -> exit 2 `spec: author_role: <problem>`, no document; the intake commands -> exit 1 with the document. The twin without `A` stores `human`.

## Known limits

An agent's Bash `spec propose` without `A` stores `human`; a subagent in a task worktree binds to the session's tree (ADR-0032); a cancelled MCP write still stores (a repeat gets the hit); a scratch-`HOME` launcher hides its items from the owner's inbox; no dedup for `update`; rows by a path and by its later `id:` stay apart.

- **Unnameable hits**: over 64, or one whose path is over 256 bytes or holds a control character (listed; `distinct_from[i]` refuses it, the agent's only sign): the item is never stored. `unnameable` counts hit documents, not names: two accepted decisions sharing an ID may read "cannot be stored" though naming them stores it. The note joins names with `, `: a path holding `, ` does not split back.
