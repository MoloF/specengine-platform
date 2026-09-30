---
class: spec
status: draft
scope: [specengine]
ref: owner requirement of 2026-09-30 (roadmap and backlog), planned for Phase 6 of 08 §2, after Phase 5
adrs: []
---

# Roadmap and backlog (planned)

## Why

The owner (2026-09-30): set the goal of the work and trace the whole path — place work in a roadmap, set priority, build the chain, separate MVP, V1, V2 and so on, roughly estimate timelines, track where we are, see the pool of work not yet distributed or taken into development. Decorative, additional: after the main work.

Today the path is hand-written (08 §2: "~N weeks", "done" stamps, a hand-summed "Total") and drifts. Parked work is scattered: six "Open minors" sections (`docs/canon/spec-check.md`, five crate READMEs), "Next"/"Open nits" of `docs/canon/spec-check-graph.md`, 08 Phase 5 "Optional". Statuses, task `priority` (05 §3.3) and `depends-cycle` exist, unarranged; `get_task next` orders by priority only (07 §1.2). An item is one `/feature` run, not a `T-NNNN`; the pool precedes a task `draft`, which needs target nodes (06 §2).

Questions (working answer → cost of the other):

- Q-1 One git file per item; execution in tasks; position computed, survives deleting `<slug>.db` (05 §1) → (a) tasks in SQLite (08 §4.1): no ADR, ~0.5 week cheaper, lost with the DB, never in a diff (ADR-0003); (b) one planning file: `{#ID}` items need an ADR on `#layout`; (c) feature specs as items: long drafts rot.
- Q-2 After Phase 5 (the screen needs Phase 4, files need Q-8) → after Phase 2: the pool 6–8 weeks earlier, Phase 3 ~1.5 weeks later.
- Q-3 A flat ordered release list in `[roadmap]` with an MVP cut, each goal a release document → two levels: ~+0.5 week; a goal in config only cannot be cited.
- Q-4 Estimates `min-max` in one configured unit, no dates → dates: a scheduler ~+1 week, "late" noise (08 §5); sizes: a mapping table.
- Q-5 The chain is advisory: orders `next`, shows "waits on" → gating contradicts ADR-0012.
- Q-6 The owner fills the pool, agents via `propose_change`, a minor only when promoted → agent writes break ADR-0004 and 08 AC-3; copied minors drift.
- Q-7 Here 08 §2 becomes the generated roadmap in increment 2, one change → two plans drift, 08 stays in W's top three.
- Q-8 Per-item files: shard the index by scope (convention §9), with the pilot migration → items out of the index: an ADR against ADR-0022; nothing: the 10 KB cap after ~15 lines.

## Description and interactions

Pending the questions. CLI: `spec roadmap [--release R | --pool] [--json]`; writes via `spec new`/`spec edit` (07 §2). MCP, read only: `get_task next` orders by release, chain, priority; one read tool < 48 000 characters, paginated (07 §1.1); agents add items via `propose_change kind: create`; the brief places the task in the path (05 §6). `docs/generated/roadmap.md`: in `[[generators]]`, drift-checked, not indexed. After Phase 4, the decorative part: a Roadmap screen (07 §3) — release lanes, the pool, chain edges, a range timeline; a drag is an owner edit applied as a proposal.

## Data

Pending Q-1, Q-3, Q-4. An item: one file under a configured prefix, the convention's statuses, `kind` from the project vocabulary, the body its goal; the chain is `links.depends_on` (closed `LINK_TYPES`). Tasks cite items in `target_ids` (05 §3.3).

```yaml
id: RM-0012
status: draft
release: v1
priority: high
estimate: 2-4
links: {depends_on: [RM-0009]}
```

```toml
[roadmap]
prefix = "RM"
releases = ["mvp", "v1", "v2"]       # path order, Latin slugs
mvp_cut = "mvp"
priority = ["high", "normal", "low"] # like severity, 05 §2.2
unit = "days"
```

Derived, never written: done at `shipped`/`abandoned`; in progress when a linked task is past `ready` or the feature spec is `in-progress`; current release = the first with unfinished items; time left = Σ min to Σ max over unfinished items, serially. Pool: items without `release`, tasks targeting no item.

## Rules and edge cases

- Items are git files, SQLite only indexes them (`docs/canon/architecture.md#storage`), one file each (`#layout`).
- An item changes only by `apply_proposal` on an owner action, drags included (`#apply`).
- WHEN a predecessor is unfinished the system SHALL still allow `ready` and `claim_task` (`#control`).
- Releases, MVP cut, priority scale and unit come only from `[roadmap]` (`#universal`).
- Single user: no assignees, no capacity (`#distribution`); English interface (`#ui`).
- Revisit 08 §4.1: "Roadmap state and queue → SpecEngine tasks", "milestones → tasks" (pilot shapes unverified).

## Acceptance criteria

- [ ] I1-a On `spec-a` and `spec-b`, each with its `[roadmap]`, `spec roadmap --json` groups by release in config order, orders by chain then priority, sums ranges of unfinished items (`expected.json`). Red: reversed release order.
- [ ] I1-b An item without `release` appears only under `--pool`. Red: give it a release.
- [ ] I1-c A `depends_on` cycle of items → `depends-cycle`, verdict unchanged. Red: remove one edge.
- [ ] I1-d `check_genre.rs` covers the roadmap sources. Red: a hard-coded `"mvp"`.
- [ ] I1-e The generated roadmap equals its render byte for byte. Red: a hand edit → drift error.
- [ ] I2-a `spec roadmap` leaves `git status` empty.
- [ ] I2-b An item with an unfinished predecessor can go `ready` and be claimed. Red: a chain gate.
- [ ] I2-c `rm <slug>.db && spec index` restores releases, items, chain, estimates.
- [ ] I2-d The MCP read over 500 items is < 48 000 characters with a "not included" tail.
- [ ] I3-a A drag to another lane → one `spec: apply PR-…` commit with provenance, no other write.

## Out of scope

Dates, velocity, burndown; assignees (ADR-0017); hard dependencies (ADR-0012); tracker sync (export stays optional, Phase 5); cross-project roadmaps; agent write tools (ADR-0004).

## Implementation

Not started. After Q-1: an ADR "plan items are files in git, execution stays in tasks, position is computed", `#storage` diff, reversing the 08 §4.1 rows; Q-8 has its own ADR. Increments: (1) data, findings, CLI, generated view ~1 week, after the Phase 1 CLI and Phase 2 `apply_proposal`; (2) position, agents, pilot import, 08 §2 as items ~0.5–1 week, after Phase 2 tasks and Q-8; (3) the screen ~1 week, after Phase 4. Rejected homes: 08 only (no place for questions, criteria); the archive (false status, still an index line).
