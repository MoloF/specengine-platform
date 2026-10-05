---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-mcp]
ref: onboarding analysis 2026-10-05, item 5 and slice criteria; owner decisions 2026-10-05; 08 §2 Phase 2, slice 4
shipped: 2026-10-05
---

# Queue path targets

## Why

A project onboarded as an overlay (an untracked `specengine.toml` at its root, its corpus untouched) has nodes only where `id:` or `{#ID}` defines them (ADR-0026), and most of its documents carry neither: `spec show` and `spec bundle` reach them by path, the queue refused a path, so an agent could not ask about, report on or change them. Now a queue target may be the root-relative `.md` path of an indexed document, the whole file, stored under the name `--links` and dedup already use: its `id:`, else its path.

No new ADR: that naming rule exists (`spec-cli-graph.md`, dedup's `distinct_from`); no rule in `docs/canon/architecture.md` ties a proposal to an ID; ADR-0004, ADR-0005, ADR-0032 hold; the columns stay TEXT, so refusing paths again strands no row.

Working answers (the owner's, 2026-10-05): Q1 pilot A is onboarded as an overlay now, its documents migrated to IDs later, a separate task; Q2 this slice precedes `decision-apply`, `update` by path included.

How it works now: `docs/canon/proposal-queue.md` "Creation" 1, 3 and "Store"; `agent-intake.md` "Tools", "Rules" 3, "Dedup", "Known limits"; `proposal-apply.md` "Apply steps" 4, "Known limits"; the CLI README "Rules".

## Acceptance criteria

Setup as `agent-intake.md`'s: temp git repos of `fixtures/spec-a` (`spec-b` where named), scratch `HOME`, injected clock, library approve with consent yes, the default MCP build. Refused: nothing stored, no event, the next ID unchanged. M: the mutation turning it red.

- [x] AC-01 — `propose update docs/features/stamina-tuning.md`, `spec show`'s `span_hash`, a text changing `priority:` and AC-07's prose: `PR-0001`; review's `target_id`, `target_path`, `target_ids` and inbox's target column that path; `ask_question` on it: created, `DEC-0023` in `related` (M: the `is a path` refusal restored).
- [x] AC-02 — `docs/spec/movement/stamina.md` → `MEC-STAMINA`: `propose update`'s `target_id` (`patch_hash` = by ID's, same base and text); `ask_question`: in a fresh queue the intake document of `node_ids: ["MEC-STAMINA"]` (hit `DEC-0023`, not stored), with `distinct_from: ["DEC-0023"]` `target_ids` `["MEC-STAMINA"]` (M: the path stored despite an `id:`).
- [x] AC-03 — approve of AC-01's, another tracked file modified: `applied`; `HEAD` one parent, the old `HEAD`; `git diff-tree --no-commit-id --name-only -r HEAD` exactly the path; trailers `Proposal: PR-0001`, `Decided-by`, `Proposed-by`, `Base-commit`; the file byte-equal to the text; the other still modified (M: a first section as the span, the `priority:` edit lost).
- [x] AC-04 — after proposing, a commit changing another line of the file → approve `rebases`, both edits kept; one changing the same line → exit 1, the conflict on stdout, file and branch as before, `open` (M: step 5 over a section's span).
- [x] AC-05 — CLI and MCP alike: `../spec-a/docs/spec/game.md`, an absolute path, `docs/./spec/game.md`, `docs//spec/game.md` → exit 2; `docs/spec/missing.md`, `NOTES.md` at the root (outside the roots), `docs/SPEC/game.md` → exit 1 naming the field; `update` on `docs/records/R/R-12.md` (immutable), spec-b's `docs/records/GLS/GLS-task-branch.md` (generated) → exit 1, while `ask_question` stores `R-12`, `GLS-task-branch` (M: the immutability check skipped for a path).
- [x] AC-06 — texts adding `id: MEC-TUNING` to stamina-tuning or dropping `{#AC-07}`, a stale base → exit 1, refused (M: check 3 skipped for a path).
- [x] AC-07 — committed in the copy: `docs/spec/movement/climb.md` (`class: canon`, no `id:`, `# Climb`), `docs/records/DEC/DEC-0099.md` (`class: decision`, `status: accepted`, `canon: docs/spec/movement/climb.md#climb`, `# Climbing costs stamina`); ask on the path: `created: false`, corpus hit `DEC-0099`, its title the answer; with `distinct_from: ["DEC-0099"]`: stored, target the path; again: a queue hit too; spec-b `docs/spec/cli.md`: stored `MOD-CLI`, hit `ADR-0001` (M: no corpus dedup for a path; `mentions` as hits).
- [x] AC-08 — `node_ids: ["docs/spec/movement/stamina.md", "MEC-STAMINA"]` → refused naming `node_ids[1]`; a discrepancy on `MEC-STAMINA`, `distinct_from: ["DEC-0023"]`, `proposed_patch.target` the path → both stored, `linked`, the update's `target_id` `MEC-STAMINA` (M: written forms compared).
- [x] AC-09 — `propose_change`, `ask_question`, `report_discrepancy` by path, `get_proposal` of AC-01's: `content` byte-equal to the twin's `2>&1`, `structuredContent` its `--json`; `get_node`'s `span_hash` taken as `base`; descriptions name the path form, `INSTRUCTIONS` byte-unchanged (M: the MCP side refusing paths).
- [x] AC-10 — AC-01's update and AC-07's question exported, imported fresh: `dump()` equal, re-export byte-identical, `queue_schema` 2 (M: a read-time ID check on `target_id`).
- [x] AC-11 — an ask on `docs/features/a<U+202E>b.md`: inbox and review text show `\u{202e}`, JSON raw (M: the target printed raw).
- [x] AC-12 — gate clean, worst W ≤ 109 484; `mcp_genre.rs`, core `check_genre.rs`, the anonymity test green (M: a pilot's name in this spec).

## Implementation

Canon moved: `proposal-queue.md` "Creation" 1 (path form, stored canonical), 3 (an `id:` added is refused), "Store" (`target_id` canonical, unchecked when read); `agent-intake.md` "Tools", "Rules" 3, "Dedup", "Stored", "Known limits"; `proposal-apply.md` step 4, "Known limits"; the CLI README "Rules", "Exit codes and streams"; the MCP README; 08 §2 Phase 2; `decision-apply.md` (`canon:` of a path target). The three canons gained room by compaction; no cap raised. No queue schema, backup or store change (`QUEUE_SCHEMA_VERSION` 2, 35 columns, `STATE_FORMAT` 1). One iteration.

| Module | What it does |
|---|---|
| CLI `show.rs` | `unclean_path` (exit 2), `not_indexed` (exit 1): the path refusals `show` and the queue share |
| CLI `propose.rs` | `is_path_target` (`.md`); `written_reference` passes a clean path; `Named {Id, Document}` and `named()`: the walk's file byte for byte, a node required, its `id:` canonical as by ID, else the path; `holder_path`'s `at` guard; `resolved_node`, `checked_update` for a document (ord 0, `target_id` the path); the `is a path` refusal gone |
| CLI `intake.rs` | `node_of`: a path target → its holder's document node for corpus dedup |
| CLI `preflight.rs` | step 4 split: `held_by_id` (as before), `document_of` (the fresh parse's document, no holder lookup); `held_at` (completion) alike |
| CLI `main.rs` | help of `propose update ID`, `propose question ID…` names the path form |
| MCP `intake.rs` | the three writers' descriptions (1 741, 1 914, 1 590 B ≤ 2 048), `ChangeArgs.target` and `node_ids` schema docs name the path form; `INSTRUCTIONS` unchanged |

Tests: `crates/specengine-cli/tests/proposal_path.rs` (14: AC-01–08, AC-10, AC-11, deviation 3, completion by a hand commit, determinism), the rewritten `proposal_create.rs::non_canonical_targets_and_bad_inputs` (a path no longer refused); `crates/specengine-mcp/tests/mcp_path.rs` (AC-09: 23 calls against their CLI twins, `INSTRUCTIONS` pinned at 1 675 B).

Accepted deviations (additive guards, now canon): (1) a path whose document's `id:` resolves to another file → exit 1 `` `<p>` declares `<ID>`, which resolves to `<q>`; a proposal names one node ``; (2) an id-less document re-read with an `id:` or no node → exit 1 `` `<p>` is not as read in the index (an `id:` or no node now); run it again ``; (3) apply step 4 and completion: a path row whose `target_id` ≠ `target_path` → refused `` `<t>` names a document by its path, not the recorded `<p>` ``; (4) a non-canonical `id:` (a legacy alias prefix) → the by-ID refusal prefixed `` `<p>` declares `<ID>`: ``; (5) an `id:` added to an id-less document is refused by core's structure message, which does not name front-matter (kept). A file with no node: `` `<p>` could not be read: it has no node to name ``, `` `<p>` is not UTF-8: … ``.

Known limits (canon): a path target follows no rename (apply step 3 refuses; ADR-0032); queue dedup keeps rows by a path apart from rows by its document's later `id:` (apply still finds the file); `decision-apply` settles a path as `canon:`.
