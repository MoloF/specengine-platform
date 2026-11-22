---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-22
summarises: [docs/canon/card-practices.md#the-rule, docs/canon/card-practices.md#disciplines, docs/canon/card-practices.md#caps, docs/canon/card-practices.md#the-card, docs/canon/card-practices.md#the-task-and-the-proposal, docs/canon/hub-team.md#team, docs/canon/auto-mode.md#rule]
---

# Department practices

A practice is a department's agreed rules, such as polygon budgets for 3D models or code review rules, kept as one document that every card and agent task of that department carries. This page covers where practices live, how `disciplines:` ties them to cards, how they reach a card's context and an agent's task within budget, how to cite a rule, and who may change one.

## What it solves

Each trade has rules it agreed on: realism level, normal maps, level of detail, naming, audio loudness, code review. Kept in people's heads or a wiki, they never reach the agent doing the work, and a reviewer cannot point at the rule a result breaks. A practice puts the rules in the spec, delivers them with every card of the trade, and makes each rule citable by ID.

## When to use it

- **A department agrees on a rule** every piece of its work should follow.
- **An agent or a person starts a card or a task** of that discipline: the rules come with it, unasked.
- **Reviewing a result**: cite the rule it keeps or departs from.

## How

### CLI

```text
$ spec card show C-0012 --practices
...
practices: PRA-ART-MODELS
cited: PRA-ART-NORMALS | Normal maps | docs/spec/practices/art/3d-models.md
```

`--context` implies `--practices` and fits the practices into the card's bundle. `spec bundle MEC-LAMP --budget 10000 --discipline art` adds a discipline's practices to any bundle, as its last layer. `spec task show T-0042` prints a `Practices: <n>` section in the brief when a card linking the task has a discipline with practices. `spec library` lists each document's `disciplines`.

### MCP

`get_card {card_id, context: true}` returns `practices` (each practice's name, title, path and rule count) and `cited` (the rule IDs the card's texts mention, resolved). `get_context_bundle {node_ids, budget, disciplines: ["art"]}` and the `get_task` package carry them too, and `get_proposal` resolves the IDs a proposal cites.

### Screen

A card's modal and page have a Practices part after Targets: each practice by title, with its rule count, opening the document. A rule ID written in a card, a question or a proposal links to the current rule. Documents has a Discipline facet.

## Where it lives

One folder per trade, under the default walk:

```text
docs/spec/practices/art/3d-models.md
docs/spec/practices/sound/mixing.md
docs/spec/practices/development/code-review.md
```

A practice names the card disciplines it serves; its rules are `{#ID}` sections:

```markdown
---
id: PRA-ART-MODELS
class: canon
tier: 2
scope: [practices]
owner: art-lead
reviewed: 2026-11-20
disciplines: [art]
---

# 3D models: agreed rules

## Normal maps {#PRA-ART-NORMALS}

Every model with surface detail ships a tangent-space normal map baked from a high-poly source.

## Polygon budget {#PRA-ART-POLYS}

A hero prop stays under 15 000 triangles at LOD 0.
```

The config that goes with it:

```toml
[ids]
PRA = { kind = "practice", shape = "name" }

[zones]
code          = ["src/**", "Cargo.toml"]
practices     = ["docs/spec/practices/**"]
art-practices = ["docs/spec/practices/art/**"]

[team.ann]
role  = "lead"
zones = ["art-practices"]
email = "ann@example.org"

[auto]
mode          = "both"
zones         = ["code"]
exclude_zones = ["practices"]
```

The gamedev preset ships four example practices (3D models, development, sound, production) in `scripts/presets/practices/gamedev/`. Copy one into your project, or propose it with `spec propose create`; `spec init` never writes them.

### A practice is a document naming its disciplines {#GD-PRACTICES-WHERE}

Any live document whose front-matter lists `disciplines:` is a practice for those card disciplines. Use the exact names your `[[card.flow]] disciplines` use; a name no flow lists gets the warning `practice-discipline`, and no card receives the practice. A document keeps at most 16 disciplines.

### One rule, one section {#GD-PRACTICES-RULES-ARE-SECTIONS}

Each rule is a direct `{#ID}` section of the practice, short enough to quote: one requirement and its reason. A practice with no ID sections is one rule, itself. Rules keep their IDs for good, so a card or a commit citing one stays readable.

### How practices reach the work {#GD-PRACTICES-REACH}

A card's context carries the practices of the card's discipline. An agent task carries the practices of every card that lists it with `--task`. They are read fresh from the index on each call, never cached, and fitted within budget as the bundle's last layer: what does not fit becomes a list of IDs and a count, with a note. A read lists at most 32 practices and 128 rules of each.

### Cite the rule, not its text {#GD-PRACTICES-CITE-RULES}

In a card's goal, a question, a proposal or a review note, write the rule's ID: `breaks PRA-ART-POLYS: 22 000 triangles`. The reader gets a link to the current rule. When a result departs from a rule on purpose, accept it with `spec card accept C-0012 --as ann --departs PRA-ART-POLYS ...`, so the departure is recorded and the owner sees it.

### Leads keep their own practices {#GD-PRACTICES-LEADS-CONFIRM}

Anyone proposes a change to a practice; the department's lead confirms it on a hub, inside the zones `[team]` gives them; the owner confirms anywhere. Give each lead a zone for their trade's folder.

## What not to do

### Never treat a practice as a gate {#GD-PRACTICES-NOT-A-GATE}

A practice informs; it refuses nothing. No card, task or file is blocked, failed or judged by a practice, and the one finding about practices is a warning. Disagreement goes through a question or a departure, not a stopped pipeline.

### Never let auto mode change a practice {#GD-PRACTICES-NO-AUTO}

Put the practices' zone in `[auto] exclude_zones`, as the example does. Practices are what a department agreed; a delegated apply must not rewrite them overnight (`guide:auto#GD-AUTO-NEVER-DELEGATED`).

### Never paste the rules into each card {#GD-PRACTICES-NO-COPY}

A pasted copy goes stale the day the practice changes and costs every read its bytes. The card already carries the current practice; cite a rule's ID where it matters.
