---
class: generated
generator: gen-index
source: the plan items of [roadmap]
---

# Roadmap

<!-- Built by `gen-index`. Manual edits are overwritten on rebuild, and `spec check` rejects them. -->

roadmap: 16 open, 5 done; current beta; to beta 14-17 days, 2 unestimated; total 23-29 days, 2 unestimated

## release alpha | 0 open, 2 done | 0 days

## release beta | current | mvp cut | 8 open, 1 done | 14-17 days, 2 unestimated

1. [RM-0017](../../plan/RM-0017.md) | Save and load | draft | must | 5
2. [RM-0013](../../plan/RM-0013.md) | Dock layout pass | draft | should | -
3. [RM-0014](../../plan/RM-0014.md) | Cargo manifest screen | draft | should | 1-2 | waits on RM-0030
4. [RM-0010](../../plan/RM-0010.md) | Tide engine core | in-progress | could | 2-4
5. [RM-0011](../../plan/RM-0011.md) | Boat physics | draft | should | 3 | waits on RM-0010
6. [RM-0012](../../plan/RM-0012.md) | Harbor crane controls | draft | must | - | waits on RM-0011
7. [RM-0016](../../plan/RM-0016.md) | Gull ambience | draft | could | 2
8. [RM-0018](../../plan/RM-0018.md) | Storm warning siren | draft | urgent | 1

## release launch | 5 open, 1 done | 9-12 days

1. [RM-0030](../../plan/RM-0030.md) | Ferry routes | draft | must | 2
2. [RM-0021](../../plan/RM-0021.md) | Ferry ticket booth | draft | should | 1 | waits on RM-0020 | cycle
3. [RM-0020](../../plan/RM-0020.md) | Ferry crew roster | draft | could | 3-5 | waits on RM-0021 | cycle
4. [RM-0022](../../plan/RM-0022.md) | Ferry launch event | draft | must | 2-3 | waits on RM-0030, RM-0020
5. [RM-0060](../spec/RM-0060.md) | Weather sync | draft | could | 1

## pool | 2 open, 1 done | 5 days, 1 unestimated

- [RM-0040](../../plan/RM-0040.md) | Fishing side quest | draft | should | 5
- [RM-0041](../../plan/RM-0041.md) | Photo mode | draft | - | -

## unplaced | 1 open, 0 done | 2-6 days

- [RM-0050](../../plan/RM-0050.md) | Co-op mode | draft | must | 2-6 | release someday

## Notes

- RM-0012: estimate "4-2" is not N or N-M
- RM-0018: priority "urgent" is not in [roadmap] priorities
- RM-0050: release "someday" is not in [roadmap] releases
- RM-0060: outside [roadmap] dir plan
- depends_on cycle: RM-0020, RM-0021
