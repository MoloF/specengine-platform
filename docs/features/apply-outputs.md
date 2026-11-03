---
class: spec
status: shipped
shipped: 2026-11-03
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http]
ref: RM-0072; Phase 7 step 10a (08 s2); the hub-git analysis and the orchestrator's decisions of 2026-11-03, with iterations 2 and 3 on the review's findings
adrs: [ADR-0062, ADR-0063]
---

# apply-outputs: the index in every apply commit, and a hub's own `PR-` range (RM-0072)

## Why

Since `hub-confirm` a confirm commits in the hub's checkout, and 10b (RM-0051) will push it. Two defects would then reach everyone. An apply committed its path alone, so a create or a title or status change left `index-drift`, an error: an `enforce` hook stopped the next person's Markdown commit (N12, which parked RM-0019). And a queue restored from a dump went on from its source's highest number, so a hub and a terminal minted the same `Proposal:` IDs, one's commit on the branch blocking the other's reject. This slice fixes both with no network verb. Decided by the orchestrator: the cut; terminals and hubs alike; render from the committed tree; only SpecEngine-rendered outputs; a gap commits the target alone with a warning, an output never refusing; a stored `PR-` floor from `PR-100001`; spec-ID collisions a known limit.

## Description and interactions

Canon: `docs/canon/proposal-outputs.md` (ADR-0062), `docs/canon/queue-backup.md` "ID floor" (ADR-0063).

### The committed tree

`HEAD` at step 2's commit under the recorded root, its config included, each path the apply writes at the bytes it writes: the tree `git commit --only` records and a staged hook judges (`proposal-outputs.md` "The committed tree").

### A hub's `PR-` floor

`specengine-http --public-origin` stores `proposal-floor` once in its data directory; every mint there goes no lower (`queue-backup.md` "ID floor").

## Data

The outputs, `never_output`'s list, the messages and causes: `proposal-outputs.md` "Never an output", "Messages". The floor file: `100001` and LF, written atomically (sibling, sync, hard link). **Bumps:** none; no queue schema, index format, event, daemon key or plugin change.

## Rules and edge cases

`proposal-outputs.md` "Steps", "Completion", "Known limits"; `queue-backup.md` "ID floor".

## Acceptance criteria

- [x] O1 — A terminal file-form create commits `A` the file and `M docs/index.md`, a note naming it; `spec check` then has no `index-drift`. M: skip the render.
- [x] O2 — The same create confirmed on a hub (PR-100001): the same commit. M: render on the terminal path only.
- [x] O3 — With an untracked document and an unstaged title edit, and a `check --staged` hook: the committed index is the committed tree's; a refusing hook puts target and index back, `open`, exit 1. M: render from the disk walk; restore the target only.
- [x] O4 — No `index = true` entry, a `touch marker.txt` command: today's commit, no marker. M: render whenever `[[generators]]` exists; run the command.
- [x] O5 — A section-body update commits its path alone. M: stage every output.
- [x] O6 — An interrupted apply completes from its commit; one more path does not. M: keep "only the target's path"; accept any extra path.
- [x] O7 — A record commits `A <record>` beside `M docs/index.md`, and completes. M: keep "exactly `A <path>`".
- [x] O8 — A root absent from `HEAD`'s tree, a symlinked index: the target alone, a warning naming the gap and the command; on a hub in `notes`. M: render the partial walk; refuse.
- [x] O9 — A dump of PR-0001..0040 into a hub's data directory: the hub PR-100001, the host PR-100002, a hub-less one PR-0041; restarts keep the file. M: no floor in `next_id`; the floor on every queue.
- [x] O10 — Both queues from one dump, both commits on one branch: the terminal's reject succeeds. M: no floor.
- [x] O11 — An ignored untracked output, a case-variant config path, a failed staging: the target alone, a warning, no entry left. M: refuse at step 9.
- [x] O12 — `HEAD`'s config naming the target or `spec.lock` as a shard narrows. M: write it.
- [x] O13 — A hand commit whose own config adds `specengine.toml` or `docs/extra.md` as a shard, or adds a `spec.lock` its parent names but lacks, completes nothing. M: accept the four paths as outputs. Reading the commit's config alone is equivalent here, no test telling it apart.
- [x] O14 — A hook rewriting `docs/index.md`: step 10 warns, `applied`. M: check names only.
- [x] O15 — Paused after each step of the floor's write, the file is absent or whole; a killed start recovers. M: `create_new` then write.
- [x] O16 — Another walked document, tracked or untracked, named as a shard keeps its bytes and narrows; a hand commit editing it does not complete. M: exclude only the four paths and the walk scope; judge `HEAD`'s tree alone.

## Implementation

Three iterations: review REWORK (two majors), then ACCEPT with one minor, then ACCEPT. Past the draft, in canon: `[paths] index` unset, an output that cannot be written and git unable to say how it holds a path narrow too; the floor's start lives in CLI `store_hub_floor`, which the daemon calls; `--editorial` carries outputs through `revisions.rs`. Tests: 4750 of 4750, `apply_outputs` 31; clippy clean. Mutations killed: 24, then 10 and O1-O10 again, then O16's two halves and the rollback guard, O11-O15 again and O13 strengthened by `o13_a_commit_adding_a_spec_lock_its_parent_names_but_lacks_completes_nothing`. Survivors: O13's commit's-config-only, equivalent; `DiskName::CannotTell` read as an exact match, which no file system stages honestly (a known limit).

### Files in scope

| Module | What it does |
|---|---|
| core `proposal.rs` | `HUB_PROPOSAL_FLOOR`, `FLOOR_PROBLEM`, `parse_floor`, `floor_text` |
| store `outputs.rs` (new) | `committed_tree`, no lazy fetch |
| store `floor.rs` (new), `queue.rs`, `queue/{cards,state}.rs` | `read_floor`, `store_floor`; every mint `max(highest + 1, floor)` |
| store `worktree.rs`, `lib.rs` | `is_skip_worktree`, `is_intent_to_add`, `local_blob_at`, `disk_name` -> `DiskName` |
| CLI `outputs.rs` (new) | `Outputs` (render, judge, write, stage, restore, compare), `never_output`, `parent_outputs`, `Beside` |
| CLI `apply.rs`, `create.rs`, `decide.rs`, `revisions.rs`, `preflight.rs` | steps 8-10 with the outputs; completion by the parent's config; the refusal's words |
| CLI `location.rs`, `lib.rs`, `proposals.rs` | `store_hub_floor`; a hub's gap into `notes` |
| http `main.rs` | the floor at a hub's start |
| Tests | CLI `apply_outputs.rs`; http `hub_ids.rs`, `hub_confirm.rs`, `hub_team.rs`, `hub.rs`; `fixtures/apply-outputs/` |
