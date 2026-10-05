---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-mcp]
ref: onboarding analysis 2026-10-05, item 5 and slice criteria; owner decisions 2026-10-05; 08 §2 Phase 2, slice 4
---

# Queue path targets

## Why

A project onboarded as an overlay (an untracked `specengine.toml` at its root, its corpus untouched) has nodes only where `id:` or `{#ID}` defines them (ADR-0026), and most of its documents carry neither: `spec show` and `spec bundle` reach them by path (`spec-cli-bundle.md` "Command"), the queue refuses a path (`proposal-queue.md` "Creation" 1), so an agent cannot ask about, report on or change them. Here a queue target may be the root-relative `.md` path of an indexed document, stored under the name `--links` and dedup already use: its ID, else its path.

No new ADR: that naming rule exists (`spec-cli-graph.md`, dedup's `distinct_from`); no rule in `docs/canon/architecture.md` ties a proposal to an ID; ADR-0004, ADR-0005, ADR-0032 hold; the columns stay TEXT, so refusing paths again strands no row.

Working answers (the owner's, 2026-10-05): Q1 pilot A is onboarded as an overlay now, its documents migrated to IDs later, a separate task; Q2 this slice precedes `decision-apply` (paused), `update` by path included.

## Description and interactions

Wherever a queue target is named — `spec propose update TARGET`, `propose_change.target`, `propose question ID…`, `ask_question`/`report_discrepancy.node_ids`, `proposed_patch.target` (`agent-intake.md` "Tools") — a `.md` argument is a path, resolved as `spec show` resolves one (CLI README "Rules"), naming its file's document node: the whole file. Review, inbox, `get_proposal`, approve, reject, completion, export and import read a stored path as an ID. MCP adds no logic (`mcp-read.md` "Parity"): the three writers' descriptions name the path form within `DESCRIPTION_LIMIT`; `INSTRUCTIONS` unchanged (2 041 B with `probes`).

## Data

**Stored target** (`proposal-queue.md` "Store"; `QUEUE_SCHEMA_VERSION` 2, `PROPOSAL_COLUMNS` 35, `STATE_FORMAT` 1 unchanged): in `target_id` and its `target_ids` item the document's `id:`, canonical as by ID (spec-a `docs/spec/movement/stamina.md` → `MEC-STAMINA`), else the path (`docs/features/stamina-tuning.md`); `target_path` the path either way. A stored target is a path iff it ends in `.md` (`spec show`'s test), then equal to `target_path`: root-relative (the root at `root_rel` in its worktree), the walked name byte for byte, no case or Unicode folding. `patch_hash` takes it as an ID. No read-time check judges a target's grammar, so a path row is not corrupt; a dump carries it as any string. **Base** of an update by path: `spec show <path>`'s (`get_node`'s) `span_hash`, the whole file's.

```
$ spec propose update docs/features/stamina-tuning.md --base b3:<span_hash> --text-file new.md --rationale "Delay: 1.2 s"
PR-0001
introduced: 0
$ spec inbox
PR-0001 | update | open | docs/features/stamina-tuning.md | <branch> | 2026-10-05T12:00:00Z | Delay: 1.2 s
```

## Rules and edge cases

They replace the path refusal of `proposal-queue.md` "Creation" 1 and `agent-intake.md` "Rules" 3; the rest holds, field prefixes included.

1. WHEN a target ends in `.md`, the system SHALL resolve it as `spec show`: not `is_clean_relative` (`../`, a leading `/`, a `.` or empty component) → exit 2; no file of the walk this call refreshed (outside the `[paths]` roots, excluded, missing, other case) → exit 1 `` `<p>` is no indexed document: … ``; unlike `show`, a file with no node (not UTF-8) → exit 1. Tier 3 is reachable, as by ID.
2. The document's parsed `id:` → stored as that ID, canonical as by ID; none (a failed front-matter too) → the path.
3. `update`: creation 1's other refusals hold for the file (`class: generated`, an `id:` prefix with `immutable_text` → exit 1); intake allows both, as for IDs.
4. Creation 2–5 on the document node: a stale base → exit 1 with the current hash; the text verbatim; check 3 on the whole file, the document's own (ID, level) entry included: an `id:` added to an id-less document, an `{#ID}` added or dropped → exit 1 (naming a document is a later kind's).
5. Intake: a node twice once canonical (a path and its document's ID; one path twice) → refused naming the first; `proposed_patch.target` among `node_ids` once canonical.
6. Dedup (`agent-intake.md` "Dedup") on the document node: its `--links` include those landing on its sections, so a live accepted decision whose `canon:` lands in the file → hit; a `mentions` link (`adrs:`, `refs:`, a citation, a Markdown link) → related. Queue hits compare canonical targets: a path and its document's ID are one.
7. Apply (`proposal-apply.md`): step 4 for a path takes the document node of the fresh parse, no holder lookup; every other step and completion unchanged, on the whole file (merge, check 3, `commit --only` of the one path, the trailers).
8. A path is a walked name: a control character or bidi mark in it is escaped wherever agent text is (inbox, review, prompts, stderr); JSON raw.

**Known limits**: a path target does not follow a rename (apply step 3 refuses; ADR-0032, no re-targeting); a document given an `id:` later keeps its path rows, which queue dedup does not join to rows by ID (apply still finds the file); `decision-apply` (resumed after this) meets a path as first target: `canon: <path>`, a non-canon file there `canon-file`, for its spec to settle.

## Acceptance criteria

Setup as `agent-intake.md`'s: temp git repos of `fixtures/spec-a` (`spec-b` where named), scratch `HOME`, injected clock, library approve with consent yes, the default MCP build. Refused: nothing stored, no event, the next ID unchanged. M: the mutation turning it red.

- [ ] AC-01 — `propose update docs/features/stamina-tuning.md`, `spec show`'s `span_hash`, a text changing `priority:` and AC-07's prose: `PR-0001`; review's `target_id`, `target_path`, `target_ids` and inbox's target column that path; `ask_question` on it: created, `DEC-0023` in `related` (M: refuse paths again: the `is a path` refusal restored).
- [ ] AC-02 — `docs/spec/movement/stamina.md` → `MEC-STAMINA`: `propose update`'s `target_id` (`patch_hash` = by ID's, same base and text); `ask_question`: in a fresh queue the intake document of `node_ids: ["MEC-STAMINA"]` (hit `DEC-0023`, not stored), with `distinct_from: ["DEC-0023"]` `target_ids` `["MEC-STAMINA"]` (M: the path stored despite an `id:`).
- [ ] AC-03 — approve of AC-01's, another tracked file modified: `applied`; `HEAD` one parent, the old `HEAD`; `git diff-tree --no-commit-id --name-only -r HEAD` exactly the path; trailers `Proposal: PR-0001`, `Decided-by`, `Proposed-by`, `Base-commit`; the file byte-equal to the text; the other still modified (M: a first section as the span, the `priority:` edit lost).
- [ ] AC-04 — after proposing, a commit changing another line of the file → approve `rebases`, both edits kept; one changing the same line → exit 1, the conflict on stdout, file and branch as before, `open` (M: step 5 over a section's span).
- [ ] AC-05 — CLI and MCP alike: `../spec-a/docs/spec/game.md`, an absolute path, `docs/./spec/game.md`, `docs//spec/game.md` → exit 2; `docs/spec/missing.md`, `NOTES.md` at the root (outside the roots), `docs/SPEC/game.md` → exit 1 naming the field; `update` on `docs/records/R/R-12.md` (immutable), spec-b's `docs/records/GLS/GLS-task-branch.md` (generated) → exit 1, while `ask_question` stores `R-12`, `GLS-task-branch` (M: the immutability check skipped for a path).
- [ ] AC-06 — texts adding `id: MEC-TUNING` to stamina-tuning or dropping `{#AC-07}`, a stale base → exit 1, refused (M: check 3 skipped for a path).
- [ ] AC-07 — committed in the copy: `docs/spec/movement/climb.md` (`class: canon`, no `id:`, `# Climb`), `docs/records/DEC/DEC-0099.md` (`class: decision`, `status: accepted`, `canon: docs/spec/movement/climb.md#climb`, `# Climbing costs stamina`); ask on the path: `created: false`, corpus hit `DEC-0099`, its title the answer; with `distinct_from: ["DEC-0099"]`: stored, target the path; again: a queue hit too; spec-b `docs/spec/cli.md`: stored `MOD-CLI`, hit `ADR-0001` (M: no corpus dedup for a path; `mentions` as hits).
- [ ] AC-08 — `node_ids: ["docs/spec/movement/stamina.md", "MEC-STAMINA"]` → refused naming `node_ids[1]`; a discrepancy on `MEC-STAMINA`, `distinct_from: ["DEC-0023"]`, `proposed_patch.target` the path → both stored, `linked`, the update's `target_id` `MEC-STAMINA` (M: written forms compared).
- [ ] AC-09 — `propose_change`, `ask_question`, `report_discrepancy` by path, `get_proposal` of AC-01's: `content` byte-equal to the twin's `2>&1`, `structuredContent` its `--json`; `get_node`'s `span_hash` taken as `base`; descriptions name the path form, `INSTRUCTIONS` byte-unchanged (M: the MCP side refusing paths).
- [ ] AC-10 — AC-01's update and AC-07's question exported, imported fresh: `dump()` equal, re-export byte-identical, `queue_schema` 2 (M: a read-time ID check on `target_id`).
- [ ] AC-11 — an ask on `docs/features/a<U+202E>b.md`: inbox and review text show `\u{202e}`, JSON raw (M: the target printed raw).
- [ ] AC-12 — gate clean, worst W ≤ 109 484; `mcp_genre.rs`, core `check_genre.rs`, the anonymity test green (M: a pilot's name in this spec).

## Out of scope

Giving documents IDs (migration, the importer's after-tree, `create`); re-targeting after a rename or a new `id:`; `path#anchor`, `@rev`, directories, globs, non-`.md` files; `decision-apply`'s `canon:` for a path; the overlay setup, pilot runs; daemon, `.claude/`, `ui/`.

## Implementation

Pending. At shipping: `proposal-queue.md` "Creation" 1, "Store" (`target_ids`: canonical targets); `agent-intake.md` "Tools", "Rules" 3, "Dedup"; `proposal-apply.md` step 4; the CLI and MCP READMEs. Those canons sit within 20 B of the 12 288 cap: room by compaction, never a raised cap.
