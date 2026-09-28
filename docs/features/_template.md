---
class: spec
status: draft               # draft | in-progress | shipped | abandoned
scope: [<crate or subtree>]
ref: <requirement in your own words / task>
adrs: []                    # decisions made by this work
---

# <Title>

## Why

Which problem of the owner or an agent we solve. A couple of paragraphs.

## Description and interactions

The whole behaviour: CLI commands, MCP tools, what the owner sees, what an agent sees. How it interacts with what already exists (links to the canon and neighbouring specs).

## Data

Formats this work fixes: front-matter, SQLite tables, JSON schemas of tools, `specengine.toml`. With real fields and an example — this section defines what can change without code.

## Rules and edge cases

Invariants and edge cases. Where possible in the form "WHEN … the system SHALL …" (ADR-0007). Links to the rules in `docs/canon/architecture.md`.

## Acceptance criteria

- [ ] AC-01 — verifiable by an action: command, input, expected output. For a check that guards against regression, name the mutation that must turn it red.

## Out of scope

What we deliberately do not do.

## Implementation

Filled in after implementation: a "module — what it does" table over the files actually changed. Deliberate deviations from the spec go here too, with the reason.

<!-- On shipping (status: shipped + shipped: date) the document is compacted to "Why", the criteria and a summary ≤ 3 KB; whatever became true moves into the canon. -->
