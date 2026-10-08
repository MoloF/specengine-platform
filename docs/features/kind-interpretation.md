---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli, ui]
ref: kinds analysis 2026-10-08 (W1-W11, slice A criteria; Q1-Q5 decided under the owner's pre-approval, review open); 06 s5.4; 08 s2 Phase 2
---

# Kind interpretation

## Why

06 s5.4: when an accepted decision becomes a rule only after interpretation, the proposal has kind `interpretation` and shows "how the decision was applied and why interpretation was needed". Today it is a plain `update`: the decision and the reading taken vanish at approval.

Slice A of three (W1): this, then `kind-decision`, then `kind-amendment` ("Out of scope"). An interpretation is an `update` naming an accepted decision, with two labelled texts; applied as an update, one commit `M <path>`, the decision in a trailer. Groundwork for B and C: kind names only as core constants (the MCP enum too); behaviour keyed on `applies()`/`decides()` or the row's shape, never a kind list.

## Description and interactions

**Propose**: `spec propose interpretation TARGET --base HASH --decision REF --how T --why T --text-file F|- --rationale T [A] [--task T] [--brief]` (the rest: `docs/canon/proposal-queue.md` "Commands"). In order, the first refusal answers, nothing stored: form (clap, exit 2; all but `A`, `--task`, `--brief` required); caps (text, rationale as an update's; `--how`, `--why` not blank, <= 2 048 bytes, W6; the author); Creation 1 (a path target too); **the decision**; Creation 2-5 (one span, no `{#ID}`/`id:` added or dropped, W2; findings never refuse); `--task`. Output as `propose update`'s.

**The decision** (W3): `--decision REF` resolves as Creation 1 resolves a target (alias, legacy ID -> exit 1 naming the canonical; look-alike, mixed script, `project:` -> exit 2 with the Latin fix; `#SECTION`, `@rev` -> exit 1). Its node is, or lies in, a live `class: decision` document with `status: accepted` (the dedup test, `agent-intake.md` "Dedup"), never by `[ids]` kind (ADR-0008; pilot B's records lack `class:` until import). Else exit 1 `--decision: <why>; nothing stored`: `<ID> is superseded by X: interpret X`; `<ID> is rejected`; `<REF> is no accepted decision: <class> document <path>`; a queue ID applied: `` `PR` is recorded as <ID>: name it ``, else `` `PR` is not recorded yet: `spec approve PR` records it first ``; the target inside that document: `the target lies in <ID> itself` (W12). Stored: the document's `id:` (W13). Re-checked at apply step 4 and by the preview.

**Review, inbox**: review document 46 keys, `interpretation` (Data), kept by `--brief`; preview refused at step 4 -> `unavailable`, note `not applicable now (step 4): --decision: <why>`. Inbox `<id> | interpretation | <status> | <target_id> | <branch> | <created_at> | <rationale's first line>`; JSON entries unchanged.

**Approve** (`proposal-apply.md`): an update's consent, steps, runs, rebase, conflict, reject; stderr `interprets <ID> (<path>)` before the prompt (W14). Step 4 re-checks the decision in the refreshed index: refused -> exit 1, `apply_failed` step 4, nothing written. `--option`, `--answer`, `--canon` -> exit 2. Commit exactly `M <path>`, as Data (W5). **Completion** matches `Proposal:` only: a commit without `Interprets:` completes it, the decision not re-checked.

**Note on an update** (W7), never a refusal (ADR-0012): `spec propose update` (MCP too) and `spec review` of an open or approved update note when the target span is, encloses or lies in the `canon:` section of a live accepted decision (the section step 10 reads; a reference-form `canon:` as `spec check` resolves it): ``<target> is in the `canon:` section of <IDs>: a change carrying one is `spec propose interpretation <target> --decision <ID>` ``, IDs by path, 10, then ` and <k> more`. Step 10's note (`decision-record.md` "Steps") names `spec propose interpretation <target> --decision <ID>` for `spec propose update`.

**Elsewhere**, as an update unless said:

- Stage (`POST .../proposals/:id/decision`): `span_hash` set; `option`, `answer`, `canon` a usage refusal. No new route.
- Round (Q5): not on the sheet; counted among open change proposals; its `round answer` line -> ``is a change proposal (interpretation): `spec approve PR` decides it``; `--staged` notes it.
- Task-bound, applied at the task's compared place: the snapshot refreshed (`tasks.md` "Task-bound proposals"). Package `open_proposals` lists it; no assumption.
- Backup (`queue-backup.md` "Format"): `STATE_FORMAT` 2, header `queue_schema` 6, 44 columns; format 2 schemas 4-6 restore (later columns NULL), format 1 as today; a schema 1-5 DB exports as 6, unmigrated.

**MCP** (`agent-intake.md` "Tools"): `propose_change` adds `decision?`, `how?`, `why?` (nullable); `kind` enum `["update", "create", "interpretation"]` from core `UPDATE_KIND`, `CREATE_KIND`, `INTERPRETATION_KIND` (`ChangeKind`) = `spec propose interpretation ... --brief`. One missing for an interpretation (or `base`) -> -32602 naming it; given on another kind -> -32602 `<field>: only an interpretation takes it`. `get_proposal` mirrors the key. `INSTRUCTIONS` says `update|create|interpretation`: with `probes` 2 035 of 2 048 B today, >= 2 B cut elsewhere (`mcp-read.md` "Texts"). No new tool.

**Plugin** 0.3.0 -> 0.4.0 (MINOR, `PINS` appended): `propose-spec-change` gains "Carrying an accepted decision into a rule" (`"kind": "interpretation"`, `decision`, `how`, `why`; superseded: resend with the named successor); `spec-author`: a decision's rule change goes "as an interpretation when the decision needed one". No `.claude/` change.

**UI** (kind-agnostic): provisional `interpretation: {decision: string; how: string; why: string} | null` citing this spec's "Data"; `ProposalCard` shows a section "Interpretation" (the decision linked to its node, then how and why, plain text) iff it is not null, never by comparing `kind`; harbor-sim holds one. No exception to `.claude/agents/ui-developer.md`.

## Data

Core `intake.rs`: `INTERPRETATION_KIND` beside `CREATE_KIND`; `commit_message` holds the labels and trailer key. Store `ProposalKind::Interpretation` (AC-R1); `NewProposal.interpretation: Option<Interpretation {decision, how, why}>`, set iff that kind, else `Invalid`.

**Schema step 6**, one `Immediate` transaction: `ALTER TABLE proposals ADD COLUMN interpretation TEXT`; `QUEUE_SCHEMA_VERSION` 6, `PROPOSAL_COLUMNS` 44, `proposal_columns(5)` the first 43. Schema 5 opens as 6, the column NULL; 7 -> exit 2 `schema version 7, this build knows 6`. Older builds refuse it (`SchemaTooNew`): `spec`, `specengine-mcp`, the daemon from one build. Row: an update's columns plus `interpretation`, compact JSON, keys in this order:

```json
{"decision":"DEC-0023","how":"Rest is any speed under 40 % of walking.","why":"DEC-0023 says rest; the rule names no speed."}
```

Corrupt, named (`get`, `list` fail; inbox notes it): the column on another kind or NULL on this one; another shape (a key missing, extra, reordered, not a string); a blank value; `how`, `why` over 2 048 bytes.

**Review document**: `interpretation` after `staged_at`, the object or `null`. Text, escaped (`escape_controls`): `interpretation:`, then indented `decision: <ID>` and the blocks `how the decision was applied:`, `why interpretation was needed:` (their text indented two more); `interpretation: -` when null.

**Commit** (core `commit_message`, texts verbatim):

```
spec: apply PR-0007

<rationale>

How the decision was applied:
<how>

Why interpretation was needed:
<why>

Proposal: PR-0007
<Decided-by, Proposed-by, Base-commit: an update's>
Interprets: DEC-0023
```

**Daemon keys** (`fixtures/daemon-keys.json`): `Proposal` gains `interpretation` after `staged_at`; a new set `Interpretation: ["decision", "how", "why"]`.

## Rules and edge cases

- WHEN the decision is superseded between propose and approve THEN step 4 refuses, the proposal stays open; a new one names the successor.
- WHEN an `update` lies in an accepted decision's `canon:` section THEN a note only (W7). A new rule section from a decision is a `create`, unflagged (W2).
- No link is written into the target or the decision: provenance is the trailer and the row. `how`, `why` raw in JSON and the commit, escaped in text (`agent-intake.md` "Escaping and size").
- A malformed interpretation is refused as form; a plain update stays open (ADR-0012).
- Genre: no `"interpretation"` literal in `specengine-{store,cli,mcp}/src` (`proposal_genre.rs`, `mcp_genre.rs`).

**Working answers**: W1-W7, Q5 as above (W4's alternative, `summary`/`rationale` reused, no bump, cannot be labelled); the spec-writer's W12 a target inside the decision's document refused, W13 its `id:` stored, W14 the consent line, `Interprets:` last, the key after `staged_at`.

**Risks**:

- Name `decision`: slice B's kind, the class, an `[ids]` kind (spec-a's `DEC`, pilot A's `ADR`), the stage's key, the route, `--decision`; `proposal_genre.rs` admits it as a class name, so no literal scan guards it.
- Hidden kind branches: `decide.rs:791-810` (every deciding non-question a discrepancy), `package.rs:470-477` (assumptions) for slice B; `stage.rs:181-195`, the refresh: each `ProposalKind::(Update|Create)` site judged, listed in "Implementation".
- Canon at shipping: five of its files within 90 B of 12 288: detail moves down, the cap holds.

## Acceptance criteria

A git copy of `fixtures/spec-a` (`RULE-STAM-REGEN` in `DEC-0023`'s `canon:` section; `DEC-0007` superseded by it); `A` = `--author-role spec-author`.

rust-developer:

- [ ] AC-R1 -- `ProposalKind::Interpretation`: `ALL` 5, `applies()`, not `decides()`; `create` stores it, `create_intake` `Invalid`. M: `decides()` true.
- [ ] AC-R2 -- `spec propose interpretation RULE-STAM-REGEN --base <span_hash> --decision DEC-0023 --how H --why W --text-file - --rationale R A` -> `PR-0001`, `introduced: 0`; a `{#ID}` added or dropped: exit 1, nothing stored. M: skip Creation 3.
- [ ] AC-R3 -- `--decision DEC-0007` exit 1 naming `DEC-0023`; `RULE-STAM-REGEN` exit 1 naming class and path; an alias (its `aliases:`) exit 1 naming `DEC-0023`; a Cyrillic look-alike exit 2; an open question's `PR-` exit 1; a `DEC-` document without `class: decision` exit 1; inbox unchanged. `DEC-0023` superseded after propose: approve exit 1, one `apply_failed` step 4, nothing written. M: drop the status check.
- [ ] AC-R4 -- `--how`/`--why` absent exit 2; blank or 2 049 bytes exit 1 `--how: 2049 bytes; at most 2048`; 2 048 stored; U+202E escaped in text, raw in `--json`. M: drop the cap.
- [ ] AC-R5 -- a schema-5 queue opens as 6, the column NULL, 44 columns; `user_version` 7 exit 2, unchanged; the column on an update, NULL on an interpretation, keys reordered: `get` fails naming row and column, inbox notes it. M: omit step 6.
- [ ] AC-R6 -- approve: `git show --name-status` exactly `M docs/spec/movement/stamina.md`, body and five trailers as Data; a rebase over another section's commit; a conflict exit 1 with its text; a hand commit with `Proposal: PR-0001` alone completes it; `--option 0`, `--answer A`, `--canon X` exit 2. M: drop the trailer, or require it for completion.
- [ ] AC-R7 -- review `--json` 46 keys (`null` on an update), the text block, kept by `--brief`; `get_proposal` = `spec review PR --brief --json`; inbox `PR-0001 | interpretation | open | RULE-STAM-REGEN | ...`, the rationale's first line last. M: dropped by `--brief`.
- [ ] AC-R8 -- a staged approve holds `span_hash`; with `option` 400; `round new` notes it among open change proposals; its `round answer` line refused `is a change proposal (interpretation)`. M: `span_hash` null.
- [ ] AC-R9 -- bound to a `ready` task whose snapshot holds `RULE-STAM-REGEN`: in `open_proposals`; applied in its place: one `task.refreshed`. M: refresh limited to `Update | Create`.
- [ ] AC-R10 -- `spec propose update RULE-STAM-REGEN ...` and its review note `DEC-0023`, exit 0, stored; the file's path target noted, `EDGE-STAM-ZERO` not; spec-b: ADR-0001's non-Latin slug noted; a recorded question's step-10 note names `spec propose interpretation`. M: no note, or a refusal.
- [ ] AC-R11 -- `tools/list` `kind.enum` `["update", "create", "interpretation"]`; no `how` -32602 `how: ...`, `how` on an update -32602; a valid call answers with `interpretation`; text asserts green. M: a kind literal in `specengine-mcp/src`.
- [ ] AC-R12 -- both plugin texts name the kind and fields; `plugin.json` 0.4.0 in `PINS`; `plugin_files.rs` green. M: text changed, version kept.
- [ ] AC-R13 -- export with an interpretation: header `{"format":2,"queue_schema":6,...}`; import, export byte-identical; a schema-5 dump restores, the column NULL. M: import refuses schema 5.

test-engineer:

- [ ] AC-T1 -- R1-R13 as tests over spec-a (spec-b for R10).
- [ ] AC-T2 -- genre scans extended to `INTERPRETATION_KIND`. M: the literal in `apply.rs`.
- [ ] AC-T3 -- `mcp_door.rs`, http `door.rs` green with an interpretation raised over MCP. M: a file write in propose.
- [ ] AC-T4 -- daemon keys: 46-key review set, `Interpretation`, Rust and TS. M: a key renamed in Rust.
- [ ] AC-T5 -- store migration tests: 5 -> 6; 7 `SchemaTooNew`. M: `user_version` left 5.

ui-developer:

- [ ] AC-U1 -- the provisional type; `daemonKeys.test.ts` green. M: the key left out.
- [ ] AC-U2 -- `ProposalCard` shows "Interpretation" iff the key is not null. M: `review.kind === "interpretation"` turns `policy.test.ts` red.
- [ ] AC-U3 -- harbor-sim's interpretation renders alike in mock and HTTP clients.

spec-writer:

- [x] AC-S1 -- this spec (W1-W7, Q5).
- [ ] AC-S2 -- at shipping, canon: `proposal-kinds.md` (an "Interpretation" section), `proposal-queue`, `proposal-apply`, `decision-record`, `decision-staging`, `round`, `tasks`, `task-package`, `agent-intake`, `queue-backup`, `mcp-read`, `plugin` as cited above, the store, CLI, MCP READMEs; 05 s3.3, 06 s5.4, 07, 08 s2, `CLAUDE.md` trimmed; check 0 errors. M: a canon file still lists `["update", "create"]`.

## Out of scope

Next slices, questions decided (owner's pre-approval, review open), not re-asked:

- **`kind-decision`** (Q1, Q2, W8, W9): a decision taken during work (agent or owner), confirmed on a terminal, recorded through `[decision_records]` like a question, the decision its working answer; no flag records `{"working_answer":true}`, `--answer` overrides, `--option` exit 2; an optional `proposed_patch` a linked update; record and canon diff two commits; dedup: question and decision one family; its own MCP intake tool; on the round sheet; a package assumption.
- **`kind-amendment`** (Q3, Q4, W10): a file-form create with >= 1 `amends` link to a live node, its target never written, `A <path>`; the amended ID never in `target_ids`; applied when its document is Tier 3 by status; a producer of unapplied ones, the Health row measured (never 0 unmeasured); may slip to Phase 4/5.
- Here: interpretations on the round sheet; states `changes_requested|deferred|superseded`; follow-up tasks, `has_open_proposal`, edit-and-accept, MCP staging forms; a "decision without its canon diff" row; pilot B's amendment relation.

## Open

- The owner reviews Q1-Q5 and W12-W14.

## Implementation

Filled in after implementation.
