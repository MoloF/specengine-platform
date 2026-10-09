---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli]
owner: owner
reviewed: 2026-10-09
---

# Question rounds: the sheet

What `spec round new` prints for one question, staged, and one decision (`round.md` "Sheet": the rules; "Answers": what each line does). A decision is laid out and answered as a question: it has no options (`proposal-decision.md`).

```
# Open questions: <slug>

project: <slug>

2 items, about 1 min of reading. Answer each on a line of its own, at the start of the line: `PR-0003: ok`. ok takes the working answer, a number takes that option, `reject: <reason>` rejects, ? or nothing leaves it open, other text is your own answer to a question. Keep the project line. Then, in a terminal: spec round answer <file>; each answer is asked again there, one at a time.

## PR-0003 - question, high

  Does regeneration wait for rest?

Working answer:
  yes, 1.5 s

Concerns: Stamina (MEC-STAMINA); Regeneration (RULE-STAM-REGEN)
Waiting: Tune stamina (T-0004)
Staged 2026-10-07T17:49:50Z: spec approve PR-0003 --answer "no"

PR-0003:

## PR-0005 - decision, normal

  Does the crane queue keep its order after a reload?

Working answer:
  Yes: the queue is saved with the dock and restored in order.

Another answer costs:
  A reload reorders waiting ships; R-31 is rewritten.

Concerns: Cargo cranes (MEC-CARGO-CRANES)

PR-0005:
```
