---
class: spec
status: shipped
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
ref: kinds analysis 2026-10-08 (Q3, Q4, W10, slice C criteria; decided under the owner's pre-approval, review open); kind-interpretation, kind-decision "Out of scope"; 06 s7; 08 s2 Phase 2
shipped: 2026-10-09
---

# Kind amendment

## Why

An amendment is a separate document that `amends` a live node until absorbed (06 s7; slice 6 Q6). A file-form `create` took one unchecked, the inbox called it a create, nothing said when it stopped pending. Slice C of three (W1). Decided (owner's pre-approval, review open): **Q3** an amendment touches only its own new file; **Q4** it is applied when its document goes to Tier 3 by status; **W10** it is a file-form create whose text declares >= 1 `amends` link to a live node, never writes its target, unapplied while its document is live.

**Cut**: (a) the kind end to end, here; (b) the "unapplied amendments" producer and its Health row: next slice `health-left`, as no served read lists links corpus-wide (`spec graph` walks from a REF, `spec inbox` reads the queue, the check reports defects, which a live amendment is not): (b) needs a new read, route and key set (07 s3 `health`, "to come").

## Description and interactions

Canon: `docs/canon/proposal-amendment.md` (naming, propose, row, approve, elsewhere, "Unapplied", known limits); the split made room: `proposal-create-apply.md` (a new file's apply, completion, killed run, reject), `proposal-kinds.md` keeping create's forms, propose, reservation and row.

## Data

`docs/canon/proposal-amendment.md` "Propose" (`spec propose amendment PATH`, the text with `links: {amends: [ID]}`), "Row" (no column, queue 6, `adds_nodes()`, `amends()`, `Reservation.kind`), "Approve" (the consent lines, `A <path>`); `fixtures/daemon-keys.json` unchanged. The example of the tests: spec-a's `docs/features/regen-delay.md` amending `RULE-STAM-REGEN`, defining `regen-delay/AC-01`.

## Rules and edge cases

`docs/canon/proposal-amendment.md` "Propose" (refusal order and texts), "Elsewhere" (create, stage, round, task, backup, MCP), "Unapplied" (W35), "Known limits". Working answers, all taken: W26 the names, no MCP tool; W27 file form only, amended IDs only in the text; W28 derived from `new_text`, no column; W29 a live `spec` or `decision`, create refusing `amends`; W30 canonical, live, not its own, once, `immutable_text` allowed; W31 no new trailer; W32 the consent names each amended node; W33 the package lists it by amended IDs; W34 kind sites by `adds_nodes()`, `amends()`, `new_file()`, never `==` on a kind; W35 absorbed by two updates; W36 plugin 0.6.0.

## Acceptance criteria

A git copy of `fixtures/spec-a` (spec-b, spec-c where named). Tests: store `queue_amendment.rs` (7), CLI `kind_amendment.rs` (24), MCP `mcp_amendment.rs` (3), core `front_matter_failure.rs` (3), additions named below. Mutations: 28 Rust (19 + 9), 8 UI, all red. Workspace 2236/2236, UI 1646.

rust-developer
- [x] AC-R1 -- store `ac_r1_the_kind_is_named_appended_applies_and_adds_nodes`, `ac_r1_an_amendment_with_a_base_and_the_intake_door_are_invalid`, `ac_r1_a_corrupt_amendment_row_names_its_column`; `ac_r1_amended_refs_reads_the_front_matter_as_written_in_order`.
- [x] AC-R2 -- `ac_r2_propose_stores_pr_0001_holding_only_its_own_new_id`, `ac_r2_r5_spec_b_and_spec_c_amend_their_own_nodes`; store `ac_r1_r2_the_row_is_a_new_files_and_holds_its_own_new_ids`.
- [x] AC-R3 -- `ac_r3_text_and_target_refusals_store_nothing`, `ac_r3_base_and_a_bad_author_are_exit_2`, `edge_a_broken_front_matter_is_refused_naming_the_file_and_line`, `edge_a_create_of_a_broken_front_matter_is_stored_with_its_finding`, `iteration2_a_decision_amendment_is_live_only_while_accepted`, `an_update_adding_amends_is_not_judged`.
- [x] AC-R4 -- `ac_r4_amended_ids_are_canonical_live_and_named_once`, `ac_r4_spec_b_names_the_canonical_id_and_refuses_a_generated_node`.
- [x] AC-R5 -- `ac_r5_approve_adds_only_the_new_file`, `ac_r5_the_consent_names_each_amended_node_in_order`.
- [x] AC-R6 -- `ac_r6_an_amended_node_no_longer_live_fails_step_4`, `edge_a_stored_text_whose_front_matter_does_not_read_fails_step_6`.
- [x] AC-R7 -- `ac_r7_a_decision_flag_on_an_amendment_is_exit_2`, `ac_r7_a_hand_commit_with_proposal_completes_it`, `ac_r7_reject_frees_the_reservation`, `iteration2_reservation_refusals_name_the_holders_kind`; store `ac_r2_r7_an_amendment_holds_its_ids_until_applied_or_rejected`.
- [x] AC-R8 -- `ac_r8_the_stage_and_the_round_take_it_as_a_change_proposal`; store `ac_r8_an_amendment_stages_an_approve_without_a_span_hash`.
- [x] AC-R9 -- `ac_r9_tasks_list_it_bound_or_by_the_nodes_it_amends`, `iteration2_listings_are_unchanged_by_the_lazy_read`.
- [x] AC-R10 -- `ac_r10_the_review_is_a_new_files_46_keys`.
- [x] AC-R11 -- `ac_r11_propose_change_amends_as_its_twin`, `ac_r11_a_base_or_an_interpretation_field_is_invalid_params`, `ac_r11_the_enum_of_four_and_the_texts_name_amendment`; the `INSTRUCTIONS` pins (`mcp_decision.rs`, `mcp_create.rs`, `mcp_tasks.rs`).
- [x] AC-R12 -- `plugin_skills.rs` `kind_amendment_ac_r12_propose_spec_change_teaches_the_amendment`; `plugin_files.rs` `PINS` 0.6.0.
- [x] AC-R13 -- `ac_r13_a_backup_with_amendments_round_trips_byte_identical`; store `ac_r13_amendments_keep_schema_6_and_round_trip_byte_identical`.

test-engineer
- [x] AC-T1 -- the tests above.
- [x] AC-T2 -- `proposal_genre.rs` `kind_amendment_ac_t2_no_amendment_or_amends_literal_outside_core`, `kind_amendment_ac_t2_no_kind_compared_with_create_or_amendment`; `mcp_genre.rs` `kind_amendment_the_four_change_kinds_and_amends_are_no_literal_of_the_server`.
- [x] AC-T3 -- `mcp_door.rs` `ac08_reads_write_nothing_under_the_project_root` (an amendment stored, one refused); http `door.rs` green.
- [x] AC-T4 -- http `daemon_keys.rs` `kind_amendment_ac_t4_an_amendment_is_served_with_the_fixture_key_sets`.

ui-developer
- [x] AC-U1 -- harbor-sim `PR-0052` (`mocks/harbor-sim/fixtures.ts`), `mocks/round.ts`, `mocks/tasks.ts`; `mocks/amendment.smoke.test.tsx` (8), `policy.test.ts`, `kinds.test.ts`; no product file changed for it.
- [x] AC-U2 -- `health/LeftRegion.tsx` source `a read of live amends links (slice health-left)`; `HealthView.test.tsx` "unapplied amendments" (2).

spec-writer
- [x] AC-S1 -- this spec.
- [x] AC-S2 -- `proposal-amendment.md`, `proposal-create-apply.md` (new); `proposal-{kinds,queue,apply,interpretation,decision}.md`, `spec-check.md`, `agent-intake.md`, `mcp-read.md`, `plugin{,-files}.md`, `decision-staging.md`, `tasks.md`, `task-package.md`; the model, core, store, CLI, MCP READMEs; 05-08; `CLAUDE.md`.

## Owner's manual check

Reinstall `specengine-mcp`, restart the daemon, update the plugin to 0.6.0. In a project: `propose_change` with `kind: "amendment"` from a session, `spec inbox`, `spec approve PR` (the consent names the amended node), `git show --stat`: one added file. `spec serve`, `pnpm --dir ui dev`: `?scenario=normal#/harbor-sim/inbox/PR-0052`, and `#/harbor-sim/health`'s row text.

## Out of scope

- **`health-left`**, next: the producer (`amended_refs`, `is_live` over the corpus), a read and `GET /api/projects/:p/health` (07 s3) with its key set, the Health row measured, never 0 unmeasured; may slip to Phase 4.
- Absorbing in one command or commit (Q3); a section-form amendment; an `--amends` flag; amending an id-less or Tier 3 document; a note on an update of an amended node; `--impact` following `amends`; pilot B's relation (its importer); states `changes_requested|deferred|superseded`; follow-up tasks; edit-and-accept; MCP staging forms.

## Open

1. The owner reviews Q3, Q4, W10, the cut ((b) to `health-left`), W26-W36.
2. This repository's closed `[classes] spec` lacks `links`: an amendment here is a `key-extra` error. Working answer: leave it (amendments here stay lines in a new spec) until the owner admits `links`.
3. The owner's manual check.

## Implementation

Rust two iterations, UI one; reviews accepted each (iteration 2 for two minors).

| Module | What it does |
|---|---|
| model `link.rs` | `AMENDS`, in `LINK_TYPES` |
| core `intake.rs`, `create.rs`, `check/text.rs` | `AMENDMENT_KIND`; `amended_refs`; `front_matter_failure` (`front_matter_failed` calls it) |
| store `queue.rs`, `queue/stage.rs` | `ProposalKind::Amendment` (`ALL` 7), `adds_nodes()`, `amends()`, `new_file()`; row decoding, `target_ids`, `new_proposal_problem`, the live reservations keyed on them; `Reservation.kind`, `QueueError::Reserved {id, by, kind}`; the stage arm |
| CLI `amend.rs` (new) | `AmendmentRequest`, `propose_amendment[_brief]`, `text_problem`, `amended_nodes`, `amended_step`, `amended_ids` |
| CLI `create.rs`, `preflight.rs`, `package.rs`, `round.rs`, `main.rs` | create's `amends` refusal; `approve_file`'s step 4 and 6 checks and consent lines; `reserved_by` naming the holder's kind; `adds_nodes()`; `Listing` (amended IDs read once, last) for packages and the round's waiting tasks; `Propose::Amendment` |
| MCP `intake.rs`, `server.rs` | `ChangeKind::Amendment`, the `base` refusal, `CHANGE_DESCRIPTION` 2 029 B; `INSTRUCTIONS` 1 885 B (BLAKE3 `2e89ff1208d7e622...`), 1 747 less the tasks line (`15142f63116f4e78...`), 2 042 with probes |
| plugin | `propose-spec-change`'s amendment bullet (body 4 092 B); 0.6.0, `PINS` `e741c8fd7383af6f...` |
| UI | `LeftRegion.tsx` source text; mocks `PR-0052`, round and task mocks |

Deviations, all accepted: 1 a second predicate `amends()` beside `adds_nodes()`; 2 the listing by shape: `amended_ids` reads any new file's text (a create declares none); 3 refusal texts as built (Creation 1's prefixed `amends: `; `an amendment names the canonical ID`; each ends `; nothing stored`); 4 reservation refusals name the holder's kind (iteration 2); 5 cap trims: `INSTRUCTIONS` drops get_tree's ": the node tree", `CHANGE_DESCRIPTION` its examples, the skill "; elsewhere follow the project's way", its header parenthetical and the "On a task" tool list (taught by `analyze`, `implement`, the roles); 6 iteration 2: a front-matter that does not read is refused at propose 3 and apply 6 with the parser's finding; a decision amendment is live only while `accepted`, its own refusal; a create with such a front-matter stays stored with its finding (ADR-0012). Left (nits, "Known limits"): a mistyped `links` reads as "declares no amends link"; the consent's amended path from the project root; `SKILL.md`'s "as the spec writes it"; the UI mock's staged-option text.
