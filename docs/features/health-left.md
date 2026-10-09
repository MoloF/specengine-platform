---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
ref: health-left analysis 2026-10-09 (W1-W14, A1-A4, criteria; Q1, Q2 decided under the owner's pre-approval, review open); kind-amendment "Out of scope"; 06 s7; 07 s3; 08 s2 Phase 2; Rust 2 iterations, UI 2, reviews accepted
shipped: 2026-10-09
adrs: []
---

# Health left: unapplied amendments

## Why

An approved amendment leaves the queue adding only its own file; the node it amends stays as written until two updates absorb it (`docs/canon/proposal-amendment.md` "Unapplied"). Nothing listed such documents (`spec graph` walks from a REF, `spec inbox` reads the queue, the check reports defects), and Health's row 3 of "What is left" (06 s7) read "Not measured yet". One read-only corpus query, `spec health [--json]`, served as `GET /api/projects/:p/health`, now measures row 3. No general "What is left" engine (W1): rows 1-2 (the queue) and 7 (budgets) stay; rows 4-6 wait for Phase 3. Three states stay apart: measured (0 too), not measured, could not be measured. No ADR (ADR-0008, ADR-0012, ADR-0033, ADR-0034).

## Description and interactions

Canon: `docs/canon/health.md`: "Rule" (core `check::unapplied_amendments`), "spec health" (text, JSON, notes, exit, determinism), "Route", "UI" (row 3's three states, the refresh rules: entering Health, "Check again", after `proposal.applied` or a stream gap while on screen; no `AbortSignal`; the home reads none), "Known limits"; the document grows by keys in Phase 3 (W14). Working answers taken: W1-W14; Q1 "old" is the queue's order, no clock (W3); Q2 pilot B's own relation is mapped at its migration.

## Data

`docs/canon/health.md` "spec health" (`Health {documents, amendments, notes}`, `UnappliedAmendment {path, id, title, class, status, links}`, `links` the `ShownLink` of `show --links`; each of `id`, `title`, `class`, `status` `null` when absent), "Route", "UI" (`getHealth`, `HealthReport`, `UnappliedAmendment`, key `["health", p]`, the mocks). `fixtures/daemon-keys.json`: 44 sets, + `HealthReport` (3), `UnappliedAmendment` (6), `ShownLink` (9).

## Rules and edge cases

`docs/canon/health.md` "Rule" (W2), "spec health" (W4, W6, W8, W9: a walk gap exit 2 / 503, never a partial list or 0; a broken front-matter a note, never refused), "Known limits".

## Acceptance criteria

Git copies of `fixtures/spec-a` (`-b`, `-c`), amendments written into the copies. CLI `tests/health.rs` (9), http `tests/health.rs` (3). Mutations: Rust 18, UI 19, all red. Workspace 2249/2249, Vitest 80 files 1710.

rust-developer
- [x] AC-R1 -- `ac_r1_live_amendments_are_listed_and_nothing_else`, `ac_r1_spec_b_and_spec_c_list_their_own_amendment`.
- [x] AC-R2 -- `ac_r2_links_are_show_links_amends_entries_every_state`.
- [x] AC-R3 -- `ac_r3_the_document_keys_counts_order_and_text`, `ac_r3_many_amendments_are_listed_uncut_by_path`.
- [x] AC-R4 -- `ac_r4_a_walk_gap_is_exit_2_naming_it_stdout_empty`, `ac_r4_a_broken_front_matter_is_a_note_not_a_row`.
- [x] AC-R5 -- `ac_r5_read_only_and_byte_identical`.
- [x] AC-R6 -- http `ac_r6_every_health_is_the_clis_document_byte_for_byte`, `ac_r6_a_query_another_method_an_unknown_slug_or_route_and_the_fence`, `ac_r6_a_walk_gap_or_a_refused_config_is_a_503_never_a_document`.
- [x] AC-R7 -- `ac_r7_the_new_sources_write_no_class_status_kind_or_link_type`; `proposal_genre.rs`, http `no_domain.rs`, `door.rs` (`/health` among the endpoints that change nothing) green.

test-engineer
- [x] AC-T1 -- the tests above, 18 mutations red.
- [x] AC-T2 -- `daemon_keys.rs` `ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json` (the three sets from `GET .../health`), `kind_decision_ac_t4_*`, `kind_amendment_ac_t4_*` at 44; CLI `kind_amendment.rs` `ac_r10_the_review_is_a_new_files_46_keys` too. Mapping: `HealthReport` <- `HealthOutcome`, `ShownLink` <- the CLI's link JSON.
- [x] AC-T3 -- `turn.rs` `health_left_t3_a_health_waits_for_its_own_projects_reads_not_anothers`.
- [x] AC-T4 -- `git diff --stat -- fixtures/spec-*` empty.
- [x] AC-T5 -- "Implementation".

ui-developer
- [x] AC-U1 -- `client.test.ts`, `http.test.ts` (one path segment, no query, no signal).
- [x] AC-U2 -- `daemonKeys.test.ts` "counts health-left's three ..." and "%s cites `docs/features/health-left.md` "Data"", 44 types, unconditional.
- [x] AC-U3 -- `health.smoke.test.tsx` "normal: harbor-sim's two in path order ...", "large: 3 000 by path, the first ten rendered until Show all 3000"; `HealthView.test.tsx` "unapplied amendments, row 3".
- [x] AC-U4 -- `HealthView.test.tsx` row 3; `health.smoke.test.tsx` "error: ...", "cannot-check: ..."; `http.test.ts` 200 and refusals; `liveScreens.test.tsx` "Health's unapplied amendments over the daemon".
- [x] AC-U5 -- `health.smoke.test.tsx` "#/harbor-sim/health reads the projects, the check, the inbox and the amendments once each ..."; `HealthView.test.tsx` "what the route reads"; `queries.test.tsx`, `health.live.test.tsx`.
- [x] AC-U6 -- `mocks/health.test.ts`; `policy.test.ts` "compares no kind, class, status, state or link type in Health ..."; `ui_policy.rs`.
- [x] AC-U7 -- "holds no digit in the value of rows 4 to 6 and of task W"; `kind-amendment` AC-U2's tests replaced; `pnpm lint` 0, `build`, `test` twice; 17 packages.

spec-writer
- [x] AC-S1 -- the draft.
- [x] AC-S2 -- `docs/canon/health.md` (new); `proposal-amendment.md` "Elsewhere", "Unapplied"; `serve.md` "Concurrency"; the http, CLI, core and `ui/` READMEs; 06 s7, 07 s2-s3, 08 s2, `CLAUDE.md`; dated notes in `ui-health` ("Why", item 2, AC-01, AC-04).

## Out of scope

Rows 4-6 producers; an age threshold or clock; rows 1-2 server-side; 06 s7's metrics; an absorb action from the row; an MCP tool, plugin or skill; the task package listing them; pilot B's relation; the Home summary; other worktrees; a check finding; `null` keys for rows 4-6; the live check (W10: no ADR, config key, schema change, dependency or package).

## Open

1. The owner reviews Q1, Q2, W1-W14 and the working answers (`class` `null` when absent; the types citing "Data"; the `cannot-check` mock's 503; the text form).
2. Owner's manual check: `spec health` in a project; `spec serve`, `pnpm --dir ui dev`: `?scenario=normal#/harbor-sim/health`, `ledger-api`, `empty`, `large`, `cannot-check`, `error`, `slow`; with the daemon, row 3 after an apply.

## Implementation

| Module | What it does |
|---|---|
| core `check/unapplied.rs` (new), `check/mod.rs` | `unapplied_amendments`, `Unapplied {amendments, not_judged}` |
| CLI `health.rs` (new), `lib.rs`, `main.rs`, `export.rs`, `links.rs` | `health` -> `HealthOutcome`, `UnappliedAmendment`, text, JSON; `Outcome::Health`, `Command::Health`; `incomplete`, `node_links`, `Admission` crate-visible |
| http `app.rs` | the route (GET, no query, in the turn); `health` in the unknown-route list after `check` |
| UI `api/{client,http,provisional,queries}.ts` | `getHealth`, the types, `useHealth`, `health` among the walks |
| UI `health/AmendmentsPart.tsx` (new), `HealthView`, `HealthRegions`, `CheckRegion`, `LeftRegion`, `styles/app.css` | row 3, a boundary per part |
| UI `mocks/health.ts` (new), `MockClient.ts`, `scenario.ts`, `harbor-sim/` | the scenarios; the two documents in harbor-sim's corpus |

Deviations, accepted. Rust: 1 the links are `show --links`' out links of the model's `AMENDS` type, exactly `amended_refs` (the draft's "`AMENDS` only through `amended_refs`" held for the listing); 2 an unreadable root, its own exit-2 line; 3 a non-UTF-8 name is no gap and no note (as `walk_gap`): a known limit; 4 notes on stderr in both modes and in JSON. Iteration 2: docs in code only (`lib.rs`'s module text, the help line). UI: 1 the zero's second line "Measured over <d> documents."; 2 the failure state titled "The unapplied amendments could not be read", no `.health-value`; 3 the queue part has its own boundary; 4 changed tests: `liveScreens.test.tsx`'s stub answers `/health`, the smoke's `error`, `slow` expect two failures, two skeletons; 5 the 3 000-row test's 20 s timeout; 6 iteration 2: harbor-sim's corpus holds the two documents, daemon keys unconditional, "None" `.health-measured`, the busy flag once.

T5, release, two consumer corpora read-only, median of 5 (health / check): 1 540 documents, none listed, 0.320 / 0.347 s; 150 documents, 7 `frontmatter-yaml` notes, 0.113 / 0.125 s; 1 442 documents under the project's committed config, 0.143 / 0.183 s.

Left (nits): `node_links` also builds incoming links; "Check again" while the check is in flight does not re-read a failed health; untested: the 500, the unreadable-root line, no event through SSE.
