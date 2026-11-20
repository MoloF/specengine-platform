---
id: PRA-DEV-PRACTICES
class: canon
tier: 2
scope: [practices]
owner: dev-lead
reviewed: 2026-11-20
disciplines: [development]
---

<!--
An example practice of the gamedev preset (docs/features/department-practices.md).
Copy this file into your project under docs/spec/practices/development/, or propose
it there with `spec propose create`.
Declare its prefix in [ids]: PRA = { kind = "practice", shape = "name" }.
The rules below are examples: replace them with your department's own.
-->

# Programming: agreed rules

The programming department's agreed rules for every change to the game's code. A task card of the `development` discipline carries them as its practices; a review cites a rule by its ID when a change keeps or departs from it.

## A change cites its rule {#PRA-DEV-CITE}

A change that follows a spec node or a practice says which one: a marker on the code it implements and the node's ID in the commit or the review. A change that departs from a rule says so and why, in its card, before it is reviewed.

## Tests named for the rule {#PRA-DEV-TESTS}

A test is named for the behaviour it checks and cites the node or rule it verifies, so a failing test points at the agreement it breaks. A rule with no test says how it is checked instead.

## No hidden global state {#PRA-DEV-STATE}

Game state lives in the places the architecture names for it; no module keeps mutable state of its own that another reads behind its back. A singleton or a static that changes at run time is agreed with the lead first and recorded as a decision.
