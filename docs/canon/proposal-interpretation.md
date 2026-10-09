---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-10-09
---

# Proposal kind: interpretation

Phase 2 (2026-10-09): an `update` carrying an accepted decision into a rule (06 s5.4), naming the decision and saying how it was applied and why interpretation was needed. Applied as an update (`proposal-apply.md`): one commit `M <path>`, the decision in a trailer; no record, no link written. Kind `decision`: `proposal-decision.md`; `amendment`: a later slice. Code: core `intake.rs` (`INTERPRETATION_KIND`, `INTERPRETATION_TEXT_MAX` 2 048, `INTERPRETATION_FIELDS`, `Interpretation`, `with_article`), `proposal.rs` (`commit_message_interpreting`, `INTERPRETS_TRAILER`, `HOW_LABEL`, `WHY_LABEL`), `check::CANON_LINK`; store `ProposalKind::Interpretation`, step 6; CLI `interpret.rs` (`decision_named`, `decision_step`, `canon_note`, `canon_target`), `preflight.rs` (`StepRead`), `review.rs`; MCP `intake.rs`.

## Propose

`spec propose interpretation TARGET --base HASH --decision REF --how T --why T --text-file F|- --rationale T [A] [--task T] [--brief]` (`A`, `--brief`, exits: `proposal-queue.md` "Commands"). A flag missing: clap, exit 2. Then in order, the first refusal answers, nothing stored, no ID taken:

1. Caps: text, `--rationale` as an update's; `--how`, `--why` not blank, <= 2 048 bytes each (`--how: 2049 bytes; at most 2048`); the author (exit 2).
2. Creation 1 (`proposal-queue.md`): the target, a path too.
3. The decision (below).
4. Creation 2-5: base, text (one span, no `{#ID}`/`id:` added or dropped), validation (findings never refuse), binding.
5. `--task` (`tasks.md` "Task-bound proposals").

Output as `propose update`'s: the review document, `--brief` the brief one.

## The decision

`--decision REF`, trimmed. A queue ID (a look-alike: exit 2 with the Latin form) names no decision: `` `PR` is recorded as <ID>: name it `` (applied as a record), `` `PR` is an update: it records no decision `` (any non-deciding kind, with its article), `` `PR` is rejected: it records no decision ``, `` `PR` is not recorded yet: `spec approve PR` records it first ``, ``no proposal `PR` in this project's queue``. Else REF resolves as a target (an alias, a legacy ID, `#SECTION`, `@rev` -> exit 1 naming the canonical form; a look-alike, mixed script, `project:` -> exit 2 with the Latin fix); its node's document must be `class: decision`, live and `status: accepted`, never told by an `[ids]` kind (ADR-0008). Else exit 1 `--decision: <why>; nothing stored`:

- `<REF> is no accepted decision: <class> document <path>` (another class; a decision with another status or not live); `... document <path> declares no `class:``;
- `<ID> is superseded by X: interpret X`; `<ID> is rejected`;
- `` `<path>` could not be read: its decision cannot be told ``;
- `the target lies in <ID> itself` (the target in the decision's own file).

Stored: the document's `id:`, canonical; an id-less decision document by its path. Apply step 4 and review's preview check it again over the recorded root's refreshed index (`decision_step`): refused -> `--decision: <why>`.

## Row and review

`ProposalKind::Interpretation`: `applies()` true, `decides()` false; `create` stores it, `create_intake` -> `Invalid`. `NewProposal`, `Proposal` `.interpretation: Option<Interpretation {decision, how, why}>`, set iff that kind, nothing blank, `how`, `why` within the cap, else `Invalid`.

Queue step 6, one `Immediate` transaction: `ALTER TABLE proposals ADD COLUMN interpretation TEXT`; `QUEUE_SCHEMA_VERSION` 6, `PROPOSAL_COLUMNS` 44, `proposal_columns(5)` the first 43. Schema 5 opens as 6, the column NULL; 7 -> exit 2 `schema version 7, this build knows 6`. A migrated queue refuses older builds (`SchemaTooNew`): `spec`, `specengine-mcp` and the daemon come from one build. The row: an update's columns plus `interpretation`, compact JSON, keys in order: `{"decision":"DEC-0023","how":"...","why":"..."}`. Corrupt, named (`get`, `list` fail; inbox skips it, a note): the column on another kind or NULL on this one; another shape (a key missing, extra, reordered, not a string); a blank value; a text over the cap.

**Review document**, 46 keys: `interpretation` after `staged_at`, before `notes`, the object or `null`, kept by `--brief` (MCP `get_proposal`, `mirror.rs`). Text, escaped (`escape_controls`; JSON and the commit raw):

```
interpretation:
  decision: DEC-0023
  how the decision was applied:
    <how>
  why interpretation was needed:
    <why>
```

`interpretation: -` when null. **Inbox**: `<id> | interpretation | <status> | <target_id> | <branch> | <created_at> | <rationale's first line>`; entries unchanged.

## Apply

An update's consent, steps, runs, rebase, conflict and reject (`proposal-apply.md`). The line before the question: `interprets <stored ID> (<its path at step 4>)`. Step 4 refuses a decision no longer accepted: exit 1 `--decision: <why>`, one `apply_failed` (step 4), nothing written. `--option`, `--answer`, `--canon`: exit 2. The commit (`commit_message_interpreting`), exactly `M <path>`:

```
spec: apply PR-0007

<rationale>

How the decision was applied:
<how>

Why interpretation was needed:
<why>

Proposal: PR-0007
Decided-by: ... (the four trailers as an update's)
Interprets: DEC-0023
```

**Completion** by `Proposal:` alone, as an update's: `Interprets:` never required, the decision not checked again (the commit is history).

## Note on an update

Never a refusal (ADR-0012). `spec propose update` (MCP `propose_change` kind `update`) and `spec review` of an open or approved plain update (`plain_update`: applies, no new file, no new ID, no interpretation; a section-form `create` never flagged) note when the target's span is, encloses or lies in the node a live accepted decision's `canon:` names (resolved as `spec check` does): ``<target> is in the `canon:` section of <IDs>: a change carrying one is `spec propose interpretation <target> --decision <ID>` ``, decisions by path, 10 named, then ` and <k> more`. Review judges over the recorded root's step-4 index (`StepRead`); a preview stopped before step 4, or not run, refreshes the recorded root once; unreadable -> ``cannot tell whether <target> is in an accepted decision's `canon:` section: <why>``; its worktree gone -> no note (the preview names the place).

**Step 10** of a record applied without a linked update (`decision-record.md` "Steps"): ``<ID> names <canon> in `canon:`, its text unchanged: `spec propose interpretation <target> --decision <ID>` carries it there``. `<target>` (`canon_target`, over the index updated after the commit): a reference `ID` or `ID#SECTION` -> the section's ID when it resolves to its own node, else the ID part (`scope/ID` when scoped); `path#anchor` -> the nearest ID node enclosing the anchor, an id-less document by its path, the path when it lands nowhere; no `canon:` or another project's -> the `canon:` as written.

## Elsewhere

- **Stage** (`decision-staging.md`): an approve with `span_hash`; `option`, `answer`, `canon` a usage refusal (HTTP 400). No new route.
- **Round**: never on the sheet; counted among open change proposals; its `round answer` line -> ``is a change proposal (interpretation): `spec approve PR` decides it``; `--staged` notes it unasked.
- **Task-bound**: refreshes its task's snapshot as an update (`tasks.md`); the package lists it in `open_proposals`, no assumption.
- **Backup** (`queue-backup.md`): format 2, `queue_schema` 6, 44 columns.
- **MCP** `propose_change` (`agent-intake.md` "Tools"): `decision`, `how`, `why` nullable strings, no enum (the `mcp_stage` guard admits a `decision` input there only); `kind` `interpretation` = `propose_interpretation_brief`; a field missing -> -32602 `<field>: required with kind interpretation`, `base` null -> `base: an interpretation is written against its node's span_hash (get_node); null or absent only with kind create, for a new file`; one given on another kind -> `<field>: only an interpretation takes it`.
- **Plugin** 0.4.0 (`plugin.md`): `propose-spec-change` "Carrying an accepted decision into a rule"; `spec-author` names the kind.

## UI

Kind-agnostic: `ProposalCard` shows "Interpretation" (a lazy chunk) iff `interpretation` is not null: the decision read through `GET .../nodes/<ID>` (found: linked with its title; none: the ID and `No node answers this ID.` or the reason; failed: `The decision <ID> could not be read`, Retry; while read: its ID in an `aria-busy` block, no skeleton), then "How the decision was applied", "Why interpretation was needed", agent text verbatim, controls escaped. Its stage dialog adds `It interprets decision <ID>; the card's Interpretation section says how and why.` Mock: harbor-sim `PR-0049` interprets `DEC-0012`.

## Known limits

Accepted at shipping (2026-10-09); none blocks.

- An id-less decision document is stored by its path. A `canon:` ID part written as an alias or legacy ID, or another project's, makes the step-10 command one Creation 1 refuses.
- `apply` keeps the step-4 index in memory through consent, write and commit.
- Untested: the note cut after 10; `--decision` naming a section inside a decision; an `aliases_from` prefix; mixed script; completing an interrupted apply of an interpretation; `canon_target` landing nowhere or in another project; the "cannot tell" note.
- `propose-spec-change` is 7 B under its body cap: slices B and C move plan text first.
