---
class: spec
status: draft
scope: [movement]
ref: owner request, stamina tuning
tier: two
adrs: [DEC-0023]
refs: [R-12, QST-031]
priority: high
shipped: 2026-09-28
acceptance: owner playtest on the reference machine
playtest: {ratio: 0.1, passed: true, notes: null, maps: [keep, cellar], seed: 18446744073709551615}
---

# Stamina tuning

Tune the regeneration delay of [[R-12|the regen requirement]] against
MEC-STAMINA#RULE-STAM-REGEN@3 and record the result in AC-07.

## Acceptance criteria {#acceptance}

- [ ] No mention of R-12abc, FOO-R-12 or R-12-3 counts as a citation.

### Regeneration starts 1.5 s after the last sprint {#AC-07}

Verifies R-12. Measured in the stamina test: a sprint followed by rest shows the
first regeneration tick 1.5 s later.
