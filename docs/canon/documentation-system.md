---
class: canon
tier: 2
scope: [docs]
owner: owner
reviewed: 2026-09-28
---

# Documentation System: Constant Cost at Corpus Growth

> **The norm for this repository and for every project under SpecEngine** (ADR-0022). The convention text below is quoted verbatim; how it is applied here — `docs/README.md`.

> Portable convention — no dependency on a particular stack, repo layout or tooling.
> §1 is the only non-negotiable part; everything else is a mechanism serving it.
> This document is subject to its own budget (§4): keep it under 12 KB or it fails its own rule.

## 1. The invariant

The cost of a task is not the size of the corpus. It is **W — the working set**: the bytes that
must be read to answer one question or make one change.

    corpus |C| grows O(n)   ·   W stays O(1)

Every rule below exists to hold that line. The failure mode it prevents is the common one:
a large, sincere, well-written documentation set that nobody — human or agent — can use,
because answering anything requires reading an amount of it proportional to how long the
project has existed.

Two corollaries worth stating up front, because most conventions violate them:

- **Adding a document is free. Adding a document that must be read is not.** Budget the
  second, never the first.
- **A document that is never read is not waste** — it is correctly classified archive.
  A document that must be read to rule it out is the expensive kind.

## 2. Four classes

Every document belongs to exactly one class. The class determines its lifecycle, its budget,
and whether it enters W at all.

| Class | Answers | Written | Read | Updated | Budget |
|---|---|---|---|---|---|
| **Canon** | "how it works now" | continuously | always | rewritten in place | hard cap |
| **Decision record** | "why, and when" | once | rarely | never — superseded | per-document cap |
| **Spec** | "what we are about to do" | once per change | until merge | never | none (archived) |
| **Generated** | "what exists" | by a script | by path | every build | none |

Misclassification is the main source of rot, and it is always one of two mistakes:

- a **spec left in canon position** — it describes an intent that shipped differently, and it
  rots within weeks while looking authoritative;
- a **canon written as a decision record** — the current state is then only recoverable by
  replaying history, which is exactly the O(n) this document exists to prevent.

## 3. Tiers and read triggers

Classes say what a document is. Tiers say when it is read.

- **Tier 0** — root canon. Loaded always, by everyone, for every task.
- **Tier 1** — subtree canon. Loaded when working inside that subtree, and only then.
- **Tier 2** — index plus individually addressed documents. Read on an explicit question.
- **Tier 3** — archive. Excluded from default retrieval; reachable by id when someone asks.

    W = Tier0 + one Tier1 + index + k × (Tier2 document),  k ≈ 2–3

No term depends on n. That is the whole design; the rest is enforcement.

Locality is what makes Tier 1 work: canon lives next to the code it describes, so the read
cost of a change tracks the change's blast radius rather than the size of the project.

## 4. Budgets, and how to calibrate them for a new project

Budgets are **enforced ceilings**. An unenforced budget is a wish, and wishes lose to deadlines.

Target W ≈ 20–40 KB (roughly 5–10k tokens). Allocate it:

| Slot | Cap | Rationale |
|---|---|---|
| Tier 0 | 16 KB | read on every task by every participant |
| Tier 1 | 10 KB per subtree | only one is ever in W |
| Index | 10 KB | ~1 line per document |
| Decision record | 1.5 KB each | 2–3 in W at once |

Reference point: this allocation was calibrated on a 270 kLOC, 7-service monorepo with
~1000 commits per quarter. W came to ~39 KB and does not move as the corpus grows into megabytes.

**Calibration procedure — three measurements, one afternoon:**

1. `wc -c` the document everyone already reads first. That is Tier 0 today; set the cap at
   about 1.25× and never raise it.
2. Take the five documents you actually opened last week and average their size. That is a
   realistic per-document cap — not the one you would like to impose.
3. Count the top-level subtrees a change typically stays inside. That is the number of
   Tier 1 slots.

When a tier overflows, content **moves down a tier**. It is never appended, and the cap is
never raised. The pressure to push detail downward is the point, not a side effect.

## 5. The promotion rule

**An accepted decision record must, in the same change, produce a diff to canon.**

The record carries a `canon:` field pointing at the exact section it updated. This single rule
is what keeps decision records out of W: "how it works now" is answerable from one canon
document, never by replaying a log.

Without it, decision records quietly become the source of truth, and the cost of answering any
question grows linearly with the number of decisions ever taken on that subject.

**Test:** can a newcomer answer "how does X work today" without opening a single decision
record? If not, canon is incomplete — and no amount of well-written history will fix it.

## 6. Specs are consumables

A spec's job ends at merge. What survives a shipped change is: the decision record (if a
decision was actually made), the code, the tests, and the canon diff. Everything else is
process exhaust.

- **Keep:** the statement of intent, and a short shipped-summary (≤3 KB).
- **Archive or drop:** plans, implementation notes, review transcripts, test run logs.

Measured on a real feature pipeline, exhaust was 60–70% of the bytes produced per feature.
At a hundred features that is the difference between a corpus of megabytes and one of
hundreds of kilobytes — and none of it was ever going to be read again.

Mark shipped specs `status: shipped`. **Front-matter, not folder location, is what excludes a
document from retrieval** — moving files is not a lifecycle mechanism, because the next person
to search does not know where you moved them.

## 7. Generated documentation

Anything derivable from code is generated, never written: API contract, route and permission
maps, event and topic inventories, schema and entity maps, dependency graphs.

This class has three properties no hand-written document has: it cannot rot, it costs nothing
to grow, and it is read by path rather than wholesale. **Maximize it** — it is the only part
of the corpus whose growth is genuinely free.

A generated document that someone hand-edits has silently become canon, and will rot like
canon. Regenerate in CI and fail the build on drift.

## 8. Front-matter contract

Retrofitting metadata onto a grown corpus is expensive; requiring it from the first document
costs nothing. This is the one thing that is cheap now and expensive in six months.

Decision record:

    id: ADR-0042
    title: <what was decided, not what was discussed>
    status: accepted | superseded-by <id> | rejected
    date: 2026-09-17
    scope: [<subtree>, <subtree>]        # routing
    canon: <path>#<anchor>               # see §5 — the load-bearing field
    supersedes: [ADR-0019]
    ref: <issue key>

Spec:

    ref: <issue key>
    status: draft | in-progress | shipped | abandoned
    shipped: <date>
    adrs: [ADR-0042]
    scope: [<subtree>]

Canon (minimal):

    owner: <team or role>
    reviewed: <date>

`scope` drives routing, `status` drives exclusion, `canon` drives §5. The rest is bookkeeping.

## 9. Index

One generated entry point. One line per document: id, title, scope, status. Never
hand-maintained — it is built from front-matter, so it cannot disagree with reality.

A healthy index is about 3% of the corpus it routes. Shard it by scope when it passes roughly
500 entries; below that, sharding costs more than it saves.

The retrieval protocol, for humans and agents alike, is exactly two steps: read the index,
then read at most three documents. If anyone needs a third step, the index is wrong.

## 10. Lifecycle and compaction

On a fixed cadence — quarterly, or whenever the index crosses its budget:

1. mark superseded decisions, with the superseding id;
2. absorb settled decisions into canon and trim the record down to the "why";
3. drop archived specs older than N releases;
4. regenerate the index.

Skipping compaction does not cost storage — storage is free. It costs **truth**: statuses
drift, the index starts lying, and any retrieval layer built on top begins citing dead
decisions with complete confidence. That is strictly worse than having no retrieval layer.

## 11. Enforcement

One CI check, on every change that touches documentation:

1. byte budgets — Tier 0 and each Tier 1 within cap;
2. front-matter present and schema-valid;
3. every `accepted` record has a `canon:` path that resolves;
4. every `superseded-by` target exists;
5. index regenerated and committed — fail on drift;
6. generated documents regenerated and unchanged.

This is roughly 150 lines of script. Without it, every rule above decays within two quarters,
and the decay is invisible until someone needs an answer.

## 12. Adoption order

**Day one, before any documents exist.** The front-matter schema, the four classes, the Tier 0
budget. Free now; a migration project later.

**First week.** Sort existing documents into the four classes — expect most to be misclassified
specs. Write Tier 1 canon per subtree. Start the decision log with a template and the promotion
rule in §5.

**When it hurts, not before.** The index generator, the CI check, the generated API contract.
Building these against twelve documents is premature; against two hundred it is overdue.

**Never first: a retrieval bot or assistant over the corpus.** Its answer quality equals corpus
freshness, not model quality. Over an unenforced corpus it produces confident wrong answers,
which is measurably worse than no bot at all — a wrong answer is acted upon, a missing one is
investigated. Give the corpus two months under enforcement first.

## 13. What stays O(n)

Documentation reduces **lookup** cost, not **change** cost. Cross-cutting edits, migration
ordering, and review load over a wide API surface scale with the system no matter how well it
is described. Those are addressed by code generation, shared packages and interface design.

Say this out loud when introducing the system. Documentation programs lose credibility by
being sold as a fix for the wrong curve.
