---
name: propose-spec-change
description: Propose a new text for a spec section or document through the SpecEngine queue, only where the project routes spec edits through it. The owner approves and a commit lands; never edit the spec file yourself.
---

# Proposing a spec change

The project's CLAUDE.md and its roles take precedence over this skill.

Use this only where the project routes spec edits through the SpecEngine queue. Where the project changes its spec another way, follow that way instead.

## Steps

1. Read the target with `get_node`: its current text and its `span_hash`, the `base` you send. The target is an ID as the spec writes it (not an alias), or the root-relative path of a spec document; a section's span holds its subsections, a document's span is its whole file. The header line (it ends with the span's hash) and the link lines that `with` adds are not part of the span.
2. If the answer is cut for size, a whole text written from it would drop the rest: target a smaller ID section instead, or read the span in the file itself, from its `line` to its `end_line`.
3. Write the span's whole new text, not a diff or a fragment: keep every ID heading and its level, add no heading at the section's own level or above it, change what must change, keep the rest as it was.
4. Call `propose_change` with the target, the `span_hash` you read as `base`, the new text inline, a `rationale` (why; it becomes the commit's body) and your `author_role` as the project names it.
5. Read the reply: the proposal's `id` and its `diagnostics`, the findings your text would introduce. Findings never refuse a proposal; when they show a mistake of yours, send a corrected text (below).

## After sending

- The owner reviews it; on approval SpecEngine writes the file and a commit lands. Until then the file stays as it was: never edit the spec file yourself, and never apply your text by hand.
- `get_proposal` with its `id` as `proposal_id` gives its `status` and the owner's `decision_note`.
- Refused for a stale `base`: the span changed since you read it. Read it again with `get_node`, redo your text on the current one, and send it again.
- Refused for the span's headings (an ID heading added, dropped or moved to another level, or a heading at the section's level or above inside it): keep the span's ID headings as they are, any new heading below the section's level; an ID section to add or remove is a question for the owner (the ask-owner skill).
- A corrected text is a new proposal: the earlier one stays open beside it. Name the one it replaces in the new `rationale` and in your report.
- Nothing waits for the owner: keep working, and name the proposal in your report where your work assumes it.

Arguments of `propose_change`:
```json
{
  "kind": "update",
  "target": "<ID>",
  "base": "<the span_hash of the target>",
  "text": "<the span's whole new text>",
  "rationale": "<why the spec should say this>",
  "author_role": "<your-role>"
}
```
