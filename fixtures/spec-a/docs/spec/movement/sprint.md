---
id: MEC-SPRINT
class: canon
tier: 2
parent: DOM-MOVEMENT
links:
  depends_on: [MEC-STAMINA]
  constrains: [RULE-STAM-REGEN]
---

# Sprint

Hold the sprint key to run; each second drains stamina (see
[[MEC-STAMINA#RULE-STAM-REGEN|regeneration]]).

## Cost {#RULE-SPRINT-COST rev=3}

Sprinting costs 12 units/s, set by R-12@2 and bounded by A-101.

### Empty tank {#EDGE-SPRINT-EMPTY}

At zero stamina the sprint ends; see `EDGE-STAM-ZERO` and Q-031.

```text
RULE-SPRINT-COST is not a reference inside a fence.
```

## Tuning notes

Numbers come from the feature stamina-tuning/AC-07 and from shared:DEC-0023@1.
