---
name: propose-spec-change
description: Propose a new text for a spec section or document, or a new section or spec file, through the SpecEngine queue, only where the project routes spec edits through it. The owner approves and a commit lands; never edit the spec file yourself.
---

# Proposing a spec change

The project's CLAUDE.md and its roles take precedence over this skill.

Use this only where the project routes spec edits through the SpecEngine queue.

## Steps

1. Read the target with `get_node`: its text and its `span_hash`, the `base` you send. The target is an ID as the spec writes it (not an alias), or a spec document's root-relative path; a section's span holds its subsections, a document's is its whole file. The header line and the link lines `with` adds are not the span.
2. An answer cut for size would drop the rest from a whole text: target a smaller ID section, or read the span in the file, `line` to `end_line`.
3. Write the span's whole new text, not a diff: keep every ID heading and its level, add no heading at the section's level or above it, change only what must change.
4. Call `propose_change` with the target, the `span_hash` you read as `base`, the new text inline, a `rationale` (why; the commit's body) and your `author_role` as the project names it.
5. Read the reply: its `id` and `diagnostics`, the findings your text would introduce; they never refuse it, and a mistake of yours is fixed by a corrected text (below).

## A new section or file

The same call with `"kind": "create"` adds what is not in the spec yet:

- New ID sections in a node: its `span_hash` as `base`, the text its span with the new ID headings added below its own level, its own ID headings kept at their levels.
- A new spec file: `target` a root-relative .md path where nothing is yet, inside the spec's directories; `"base": null`; the text the whole file, its ID in the front-matter's `id:`, if any.
- You name each new ID: its kind's prefix (never a legacy alias), in Latin, numbered as the project writes its IDs; look it up with `get_node` first. A taken one is refused naming its holder and, for a number, the next free ID: held by the spec or another's proposal, send again with that one.
- Held by your own earlier proposal: never take the next free ID (it adds the node twice); name that proposal in your report, ask the owner to reject it, and send again once `get_proposal` shows it rejected.
- Your new IDs are held while the proposal is open, in the spec only once approved; the reply's `target_ids` lists the target and them.
- A document amending a live node: `"kind": "amendment"`, a new spec or decision file (`"base": null`) whose front-matter declares `links: {amends: [ID]}`, each ID as the spec writes it. Approval adds only your file, the node unwritten; the amendment counts as applied once your document is shipped, abandoned or superseded.

## Carrying an accepted decision into a rule

When an accepted decision becomes a rule only through your reading of it, send the update as `"kind": "interpretation"` with `decision` (its ID), `how` (how you applied it) and `why` (why it needed interpreting), a short paragraph each. A superseded decision is refused naming its successor: send again with that one.

## After sending

- On approval SpecEngine writes and commits the file; never edit the spec file or apply your text yourself.
- `get_proposal` with its `id` as `proposal_id` gives its `status` and the owner's `decision_note`.
- Refused for a stale `base`: the span changed since you read it; read it again, redo your text and resend.
- Refused for the span's headings: keep its ID headings at their levels, a new heading below the section's; an ID section to add is a create (above), one to remove a question for the owner (the ask-owner skill).
- A corrected text is a new proposal, the earlier one open beside it: name the one it replaces in its `rationale` and your report; a corrected create's earlier one holds its IDs (above).
- Nothing waits for the owner: keep working; name the proposal and its new IDs in your report where your work assumes them.
- On a task, pass its `task_id` with each proposal.

Arguments of `propose_change`:
```json
{
  "kind": "update",
  "target": "<ID>",
  "base": "<its span_hash>",
  "text": "<the span's whole new text>",
  "rationale": "<why>",
  "author_role": "<your-role>"
}
```
