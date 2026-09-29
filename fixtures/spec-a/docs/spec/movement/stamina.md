---
id: MEC-STAMINA
kind: mechanic
class: canon
tier: 2
title: Stamina
parent: DOM-MOVEMENT
status: accepted
owner: owner
reviewed: 2026-09-20
links:
  derived_from: [R-12, A-101]
  depends_on: [MEC-SPRINT]
  uses_term: [TERM-exhausted]
---

# Stamina

Stamina limits sprinting. Drained while sprinting, restored at rest. <!-- summary: first paragraph, ≤ 3 lines -->

## Regeneration {#RULE-STAM-REGEN}
- Base rate 10 units/s, delay after sprinting 1.5 s (R-12).
- While `Exhausted`, rate × 0.5.

## Depletion {#EDGE-STAM-ZERO}
- Stamina reaches 0 → `Exhausted` is applied immediately.
