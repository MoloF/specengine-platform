---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/spec-check-links.md#feature-documents, docs/canon/decision-record-template.md#config, docs/canon/decision-record.md#steps, docs/canon/documentation-system.md#5-the-promotion-rule, docs/canon/documentation-system.md#6-specs-are-consumables]
---

# Features, criteria and decisions

A feature spec says what one change will do and how to tell it is done, and a decision record says why a choice was made, while canon keeps saying how things work now. This page covers writing a feature spec with checkable criteria, recording a decision, pointing it at canon, and superseding it later.

## What it solves

A developer needs one document to implement against, and a tester needs the same document to verify against. Criteria written as opinions ("the save works") cannot be verified, and decisions taken in a chat are lost. Without a rule that a decision changes canon, decisions pile up and "how does it work now" becomes a replay of history.

## When to use it

- **A feature spec**: any change bigger than a wording fix: a new mechanic, a new screen, a changed rule, a migration.
- **A decision record**: an open question answered, a discrepancy between code and spec settled, or a choice an agent made during work confirmed.
- **Superseding**: when a later decision replaces an earlier one. The old record is kept, never edited.

## How

### CLI

A feature spec is a file under `[paths] features`. An agent raises it as a proposal; a person may also commit it by hand:

```text
$ spec propose create docs/features/stamina-tuning.md --text-file draft.md --rationale "Tune the regeneration delay"
PR-0012
introduced: 0
```

A decision is recorded when the owner approves a deciding item (a question, a discrepancy or a decision) on a terminal:

```text
$ spec approve PR-0004 --option 1 --canon MEC-STAMINA#RULE-STAM-REGEN
record DEC-0024 at docs/records/DEC/DEC-0024.md:
...
apply PR-0004 as DEC-0024 (option 1) on main in /work/lamp-game? [y/N] y
applied PR-0004 as 3f2a1c9 on main: DEC-0024 docs/records/DEC/DEC-0024.md
```

`--answer T` records a free answer instead of an option, `--note T` adds the owner's note, and no flag takes the working answer. The canon text itself changes through a second proposal, `spec propose interpretation <target> --decision DEC-0024 ...`, which names the decision it applies.

### MCP

An agent drafts a spec with `propose_change {kind: "create", target: "docs/features/<slug>.md", base: null, ...}`. It reports a choice it made with `report_decision`, asks with `ask_question` and keeps working on its working answer. It never approves (`guide:proposals#GD-PROPOSALS-ONE-DOOR`).

### Screen

Inbox lists open deciding items; the owner stages an answer there, or answers a batch on Questions, and confirms each on a terminal (or with one click on a hub). Documents shows each canon page's decisions, grouped as accepted, superseded and rejected (`guide:documents#GD-DOCUMENTS-BEHIND-DECISION`).

## Where it lives

A feature spec, `docs/features/stamina-tuning.md`, its criteria `{#ID}` sections of a `scope = "feature"` prefix:

```markdown
---
class: spec
status: draft
scope: [movement]
ref: C-0042
---

# Stamina tuning

## Acceptance criteria

### Regeneration starts 1.5 s after the last sprint {#AC-07}

Verifies R-012. Sprint, stop: the first regeneration tick is logged 1.5 s later.
```

Inside the file `AC-07` is cited bare; anywhere else it is `stamina-tuning/AC-07`. Two features may both have an `AC-07`.

A decision record, written from your template under `[decision_records] dir`:

```markdown
---
id: DEC-0024
class: decision
title: Regeneration waits for rest
status: accepted
date: 2026-11-20
canon: docs/spec/movement/stamina.md#regeneration
supersedes: [DEC-0007]
scope: [movement]
---
```

`[decision_records]` names the `prefix`, the `dir` and a tracked `template` with slots such as `{{id}}`, `{{title}}`, `{{choice}}` and `{{canon}}`. SpecEngine fixes no shape of its own.

### A criterion is verified by an action {#GD-FEATURES-CHECKABLE-CRITERIA}

Each criterion names an action and its observable result: a command and its output, a fixture and a count, a screen and what it shows. For a regression, name the change that must turn the test red. "Regeneration feels right" is no criterion; "the first tick is logged 1.5 s after the sprint ends" is.

### Criteria live in their feature {#GD-FEATURES-SCOPED-CRITERIA}

Declare the criterion prefix with `scope = "feature"`. Its IDs are defined only as `{#ID}` sections of a document directly under `[paths] features`, whose file stem is its slug. Defining one elsewhere is the error `id-scope`.

### An accepted decision changes canon in the same change {#GD-FEATURES-PROMOTION}

A decision with `status: accepted` carries `canon:`, the exact canon section it changed, as `path#anchor` or an ID. In the same change, that canon section is rewritten to say how things work now. `spec check` reports `canon-missing` when `canon:` is absent and `canon-anchor` when it names nothing. Documents marks a page "behind its decision" when an accepted decision is newer than the page.

### A shipped spec is compressed {#GD-FEATURES-SHIP}

At ship, set `status: shipped` and `shipped: <date>`, tick the criteria, and cut the spec to its "Why", its criteria and a short summary (3 KB or less). Plans and implementation notes are spent; git keeps them. The truth moves into canon. A shipped spec leaves default reads by its status, not by moving the file.

## What not to do

### Never edit an accepted decision {#GD-FEATURES-SUPERSEDE}

A decision is written once. To change it, record a new one with `supersedes: [DEC-0023]`, and set the old one's `status: superseded-by DEC-0031`. Both stay; canon follows the new one.

### Never leave a spec in canon's place {#GD-FEATURES-NO-SPEC-AS-CANON}

A spec describes intent, and intent ships differently. Once shipped, a spec is no source for "how it works now"; canon is. If behaviour diverged from the spec on purpose, write the divergence into the spec and canon before closing it, or the next reader takes it for a bug.
