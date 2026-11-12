---
name: write-flow
description: Write a flow of spec documents
---

# Writing a flow

The project's CLAUDE.md and its roles take precedence over this skill.

Use this to write a flow: steps in reading order, with forks, loops, reuse and groups.

## Steps

1. **Configure.** Add the block below to the project's config; the owner commits it: a proposal needs a commit. Name steps by `paths`: your own key in a rule's `when` selector warns on each document holding it. `[[flow.step]]` is optional.
2. **Rehearse.** Send the first proposal with `dry_run` true: the real refusals and findings, nothing stored.
3. **Propose.** One proposal per document, no `base` for a new file; a later edit goes by the propose-spec-change skill. A document may name one still open. One not yet sent, as a loop needs, reads `ref-dangling` until it lands. A reply with `id` null names a gating `similar` entry: reuse it, or resend naming it in `distinct_from`.
4. **Approve.** The owner runs `spec approve --open` on a terminal, one key each.
5. **Read.** Call `get_flow` until each row is measured and finds nothing; a row not measured is no pass.

## Grammar

- `parent:` groups. `stage:` stands on each step, is never inherited, and cuts across groups.
- `follows` stands on the later step. A plain fork says nothing: mark it `kind: parallel` (all run, then join) or `kind: alternative` (one runs); a link's `when` is a prose label. `kind: repeat` closes a loop.
- `uses` reuses a node; a definition wants two users.
- `supplies` stands on the producer, naming what it feeds.

## Example

```toml
[ids]
NODE = { kind = "node", shape = "name" }
[[flow.start]]
paths = ["docs/spec/open.md"]
[[flow.end]]
paths = ["docs/spec/done.md"]
[[flow.definition]]
paths = ["docs/spec/rule.md"]
```

`propose_change`:
```json
{"kind":"create","target":"docs/spec/root.md","text":"---\nid: NODE-root\nclass: canon\nowner: owner\nreviewed: 2026-01-01\n---\n\n# Flow\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/rule.md","text":"---\nid: NODE-rule\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\n---\n\n# Rule\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/open.md","text":"---\nid: NODE-open\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\nstage: a\n---\n\n# Open\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/left.md","text":"---\nid: NODE-left\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\nstage: a\nlinks:\n  follows:\n    - {to: NODE-open, kind: parallel}\n  uses: [NODE-rule]\n---\n\n# Left\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/right.md","text":"---\nid: NODE-right\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\nstage: a\nlinks:\n  follows:\n    - {to: NODE-open, kind: parallel}\n  uses: [NODE-rule]\n---\n\n# Right\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/join.md","text":"---\nid: NODE-join\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\nstage: b\nlinks:\n  follows:\n    - NODE-left\n    - NODE-right\n    - {to: NODE-lost, kind: repeat, when: retried}\n  supplies: [NODE-done]\n---\n\n# Join\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/done.md","text":"---\nid: NODE-done\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\nstage: b\nlinks:\n  follows:\n    - {to: NODE-join, kind: alternative, when: won}\n---\n\n# Done\n","rationale":"flow","author_role":"<your-role>"}
```
`propose_change`:
```json
{"kind":"create","target":"docs/spec/lost.md","text":"---\nid: NODE-lost\nclass: canon\nowner: owner\nreviewed: 2026-01-01\nparent: NODE-root\nstage: b\nlinks:\n  follows:\n    - {to: NODE-join, kind: alternative, when: lost}\n---\n\n# Lost\n","rationale":"flow","author_role":"<your-role>"}
```
