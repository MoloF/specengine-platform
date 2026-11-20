---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/documentation-system.md#2-four-classes, docs/canon/documentation-system.md#5-the-promotion-rule, docs/canon/architecture.md#layout, docs/canon/architecture.md#ids, docs/canon/node-revision.md#revision]
---

# Records, the spec tree and canon

Every document in a project belongs to one of four classes, and every statement worth citing has one Latin ID that is defined in exactly one place. This page says which class a document is, how IDs, sections and revisions work, and how the business-logic tree is laid out.

## What it solves

A rule written in two places drifts into two rules. A decision that lives only in chat is lost. A document that mixes "how it works now" with "what we once planned" looks authoritative and is wrong. Classes keep these apart, and IDs let code, cards, questions and other documents point at one exact statement, which a read then returns alone.

## When to use it

- **Adding anything others will rely on**: a requirement, a mechanic, a rule of a mechanic, an assumption, an open question, a term.
- **Deciding where a text goes.** Ask what it answers: "how it works now" is canon, "why, and when" is a decision, "what we are about to do" is a spec, "what exists" is generated.
- **Citing a statement** from code, a card, a question or another document.

## How

### CLI

```text
$ spec show MEC-STAMINA
$ spec show MEC-STAMINA#RULE-STAM-REGEN
$ spec show R-012 --links
$ spec tree DOM-MOVEMENT --depth 2
$ spec search stamina regeneration
$ spec bundle MEC-STAMINA --budget 8000
```

`spec show` takes an ID, an alias, `slug/ID`, `ID#SECTION` or a root-relative `.md` path. It prints the node and its `span_hash`, which a proposal to change it needs. `--links` adds what the node links and what uses it. `spec bundle` gathers a node with what it depends on, fitted to a character budget, for one task.

### MCP

`get_node {id, with: ["links"]}`, `get_tree {root, depth}`, `search {query}` and `get_context_bundle {node_ids, budget}` are the same reads. The resources `spec://<slug>/tree` and `spec://<slug>/node/<REF>` serve them too.

### Screen

The Spec tree lists the tree from its root. Any ID opens the document view, one modal with the text and its links; Documents lists every document with facets (`guide:documents#GD-DOCUMENTS-FIND-BY-FACET`).

## Where it lives

The tree, under `[paths] spec` (default `docs/spec/`):

```text
docs/spec/game.md                  DOM-GAME, the root
docs/spec/movement/README.md       DOM-MOVEMENT, a domain (Tier 1)
docs/spec/movement/stamina.md      MEC-STAMINA, a mechanic
docs/records/R/R-012.md            a requirement, one file
docs/records/Q/Q-031.md            an open question, one file
```

A mechanic, its rules as `{#ID}` sections:

```markdown
---
id: MEC-STAMINA
class: canon
tier: 2
title: Stamina
parent: DOM-MOVEMENT
owner: design-lead
reviewed: 2026-11-20
links:
  derived_from: [R-012]
  depends_on: [MEC-SPRINT]
---

# Stamina

Stamina limits sprinting. Drained while sprinting, restored at rest.

## Regeneration {#RULE-STAM-REGEN rev=2}

Base rate 10 units/s, starting 1.5 s after the last sprint (R-012).
```

Each prefix is declared once in `[ids]`, with its kind and shape: `R = { kind = "requirement", width = 3 }` issues `R-012`; `MEC = { kind = "mechanic", shape = "name" }` takes `MEC-STAMINA`. `aliases_from = ["QST"]` keeps a legacy prefix resolving. `immutable_text = true` marks a prefix whose records no proposal may change.

### Four classes, one per document {#GD-RECORDS-FOUR-CLASSES}

| Class | Answers | Updated |
|---|---|---|
| canon | how it works now | rewritten in place, within its cap |
| decision | why, and when | never: superseded by a new one |
| spec | what we are about to do | consumed at ship, then archived |
| generated | what exists | rebuilt by its generator |

The class is in front-matter, `class:`, and front-matter, never the folder, decides what is live. Canon carries `owner` and `reviewed`. A document with no class gets `class-missing` from `spec check`.

### One record, one file {#GD-RECORDS-ONE-FILE}

A record of a project-scoped prefix (requirement, assumption, question, decision, term) is its own file, `docs/records/<PREFIX>/<ID>.md`, with `id:` in its front-matter. Atomic files give clean diffs, merge without conflicts and cost an agent one small write. A mechanic is one document read whole, its rules `{#ID}` sections inside it, so editing one rule never makes its neighbour look changed.

### A node's place is its `parent:` {#GD-RECORDS-PARENT}

A document joins the tree by `parent: <ID>` in its front-matter, never by its folder alone. `spec tree` and the Spec tree follow `parent:`; a section is the child of the heading it sits under. Typed links go under `links:` (`derived_from`, `depends_on`, `answers`, ...); flow links are `follows`, `uses` and `supplies`.

### A revision says the meaning changed {#GD-RECORDS-REVISION}

A document's revision is its front-matter `rev:`, a section's the `rev=N` on its heading; none written is 1. A change of meaning raises it; a typo fix does not. Approving a meaning change through the queue raises it for you; `spec bump` lists the nodes changed since `HEAD` whose revision was not raised. Code markers cite `ID@rev`, so a raised revision shows which code lags (`guide:markers#GD-MARKERS-RAISE-THEN-ALIGN`).

### Canon says how it works now {#GD-RECORDS-CANON-NOW}

Canon is rewritten in place when the truth changes; it is never appended to as a log. A newcomer must be able to answer "how does X work today" from canon alone, without opening a decision. Each accepted decision changes canon in the same change (`guide:features#GD-FEATURES-PROMOTION`).

## What not to do

### Never mix scripts in an ID {#GD-RECORDS-LATIN-IDS}

IDs are Latin letters, digits and `-`. A look-alike letter from another script is refused with its Latin fix. Text may be in any language; IDs never are. An imported corpus keeps its old IDs as `aliases:`, which still resolve.

### Never reuse or renumber an ID {#GD-RECORDS-NO-REUSE}

An ID, once cited, is cited from code, cards, questions and git history. A retired record gets `status: abandoned` or is superseded; its ID is not given to a new statement. A renamed document keeps the old ID as an alias.

### Never write a statement in two places {#GD-RECORDS-ONE-PLACE}

Cite the ID instead of copying the text. A copy drifts, and a read of one copy cannot know about the other. `spec show --links` and the Documents "Used by" column show who relies on a node before you change it.
