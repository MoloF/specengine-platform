---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4b-1 (4b split 2026-10-13, Q8; WA-1..WA-14; Q1-Q16 decided under the owner's pre-approval, review open, "Open"); 05 s3.5; 07 s2; 08 s2, AC-13; Rust 3 iterations, review accepted with minors
shipped: 2026-10-14
adrs: [ADR-0043]
---

# rev-discipline (4b-1): approval raises a changed meaning's revision, or the owner calls it editorial

## Why

Nothing raised a revision: apply wrote the text as sent, an agent could change `rev=`, and a meaning approve and an editorial one made the same commit. So "rewrote the meaning, coverage stayed green" (05 s3.5) stayed possible, and `spec lock accept --editorial`, reading the node's file and the lock clean at `HEAD`, ran only after the text commit, whose hook 4b-2's rule fails.

Delivered, by ADR-0043 (amends ADR-0041): the raise at apply; an agent's revision change refused at creation; `spec approve --editorial`, refreshing the vouching lock entries in the apply commit; its stage, queue schema 7, the review's `revisions`; the UI toggle; the skill sentence (plugin 0.7.2). 4b's other half, 4b-2 (`docs/features/rev-rule.md`): `rev-not-raised` (AC-13), `spec bump`, `spec verify --staged`. 4b-1 adds no check.

## Description and interactions

Canon: `docs/canon/proposal-revisions.md` (new: creation, the raise, the editorial approve, completion, review, stage, UI, known limits), `node-revision.md` "Raising"; the rest: SW-4.

- **The owner** approves as before, meaning by default (the question names the raise); `--editorial` or a staged `editorial: true` keeps revisions, the `[y/N]` confirming the lock write. A hand editorial edit of a locked node: Q5.
- **An agent** never writes a revision (Q7) and says "editorial" in the rationale, text only (Q6): `propose-spec-change` gained `Keep every rev as you read it: approval sets revisions. If you believe the change is editorial (wording, no meaning), say "editorial" in the rationale; the owner decides.`
- **This repository** (WA-10): no `spec.lock`, so no refresh; the raise reaches only nodes with an ID, here ADRs, whose closed `decision` class admits no `rev`: a meaning update of an ADR is refused at step 6 (Q11), `--editorial` passes. Hook, CI unchanged.
- **Crates** (WA-13): no new crate or dependency; HTTP no code. **Later slices**: 4c-1 takes schema 8, `approve --editorial` on a `drift` item; 4b-2, 8b reuse `lock::vouches` and `refresh_editorial`'s filter; 5b keeps code fields byte for byte.

## Data

As built in the canon; the headings stay for the pointers in code, tests and neighbouring specs.

### Raising at apply

`docs/canon/proposal-revisions.md` "The raise" (WA-1, WA-2, Q3, Q11, Q15).

### Creation

`docs/canon/proposal-revisions.md` "Creation" (WA-4, Q7).

### Editorial refresh

`docs/canon/proposal-revisions.md` "Editorial approve", "Completion and reject" (WA-5, Q1, Q10, Q12, Q16).

### Review document, stage, queue

`docs/canon/proposal-revisions.md` "Review", "Stage and UI" (WA-11, Q14, Q15); `decision-staging.md` "The stage", "Documents".

### UI

`docs/canon/proposal-revisions.md` "Stage and UI" (WA-12, Q14).

## Rules and edge cases

In `docs/canon/proposal-revisions.md`: a whitespace-, heading- or move-only change keeps its revision (ADR-0040); entries at R not vouching for the old text stay byte-identical (Q10); two branches' refreshes meet in `regen` ("Known limits"). Genre-neutral: the raise keys only on canonical IDs, `norm_hash`, `[classes]` and the lock.

## Acceptance criteria

rust-developer (mutation -> the test that must fail):

- [x] RD-1 -- raising: `meaning_approve_raises_each_changed_node`, `editorial_approve_keeps_revisions`, `whitespace_only_change_keeps_revision`, `apply_never_lowers_a_revision`, `unchanged_nested_section_keeps_its_revision`, `document_rev_line_follows_id_with_its_line_ending`, `unreadable_current_revision_refuses_at_step_6`, `raise_at_max_revision_refuses`; from the base -> `rebased_apply_raises_from_the_current_revision`; a closed class -> `raise_into_a_closed_contract_refuses_at_step_6`.
- [x] RD-2 -- editorial: `editorial_apply_is_one_commit_of_text_and_lock`, `editorial_refresh_keeps_code_hashes_and_commit`, `only_vouching_entries_are_refreshed`, `editorial_refresh_ignores_code_recipe`, `dirty_lock_refuses_editorial_apply`, `editorial_without_a_lock_writes_text_only`, `editorial_prompt_names_the_lock_write`, `step_10_checks_the_lock_hash_against_the_committed_blob`, `hook_rewriting_the_lock_fails_step_10`, `editorial_commit_deleting_the_lock_fails_step_10`, `failed_editorial_commit_restores_text_and_lock`; `editorial_apply_refreshes_tested` is the later of 4b-1 and 8b's: 8b (`tested-evidence.md`).
- [x] RD-3 -- creation: `proposal_changing_a_revision_is_refused`; compared by `number()` -> `unwritten_to_written_one_is_refused`.
- [x] RD-4 -- `interrupted_editorial_apply_completes`, `interrupted_meaning_apply_completes_with_the_raise`, `reject_refuses_with_a_two_path_apply_commit`, `unraised_apply_commit_completes` (Q12).
- [x] RD-5 -- `editorial_on_a_new_file_is_a_usage_error`, `staged_editorial_is_confirmed_by_a_bare_approve`, `typed_flags_replace_a_staged_editorial`, `schema_7_rewrites_open_approve_stages`.
- [x] RD-6 -- the pins `REVIEW_KEYS` in CLI `tests/proposal_create.rs` (47) and `rev_raise::review_document_every_key_present`; `revisions_null_for_a_new_file`, `revisions_read_the_current_file`; the preview made `unavailable` on a raise refusal -> `revisions_name_a_refused_raise`.
- [x] RD-7 -- a kind literal -> the genre scan; a new dependency -> the CLI pin in `build_graph.rs`.
- [x] RD-8 -- carry-overs from 4a (code only): "set the marker to its revision"; store `git.rs`, `verify_git.rs` docs; pointers to `docs/canon/spec-verify.md` headings; the same-revision hint and accept's refusal "raise its `rev` by hand, commit, then accept; or `--editorial`" -> `same_revision_hint_names_a_raise_and_editorial`.

test-engineer:

- [x] TE-1 -- through a hook running `spec check --staged --root .`, the golden code-a lock and `verify-ecs`: meaning 3 -> 4, a rebase 4 -> 5 with a nested section kept; editorial with vouching entries refreshed, a non-vouching one byte-identical, `spec verify` clean; CRLF; an interpretation's trailers; a closed class refused at step 6, `--editorial` passes.
- [x] TE-2 -- interrupted before and after the commit: completed, reject refused; a pre-4b-1 apply commit completes.
- [x] TE-3 -- an HTTP stage `editorial: true` -> the terminal's staged line; migration 6 -> 7; `fixtures/daemon-keys.json`: stage 7, review 47, 45 sets with `RevisionRow`.
- [x] TE-4 -- `code_bind.rs` -> `code-bindings.md` "Rust binding"; `code_refs_walk.rs` -> `spec-lock-accept.md` "Steps"; apply tests carry the raise.

ui-developer:

- [x] UI-1 -- `editorial_toggle_hidden_without_revisions`, `editorial_toggle_shown_for_a_refused_raise`. UI-2 -- the flag dropped from the stage or the Editorial row lost -> the contract tests against the mock and `HttpClient`; the copied command stays bare.
- [x] UI-3 -- the UI computing `rev + 1` -> the mock with `to` 9 for rev 3 fails. UI-4 -- provisional types (`RevisionChange`, `StagedApprove.editorial`), `?scenario=editorial`; lint, tests, build.

spec-writer:

- [x] SW-1 -- this spec, refreshed and split 2026-10-13. SW-2 -- ADR-0043 and its canon. SW-3 -- `export index`, `check` clean.
- [x] SW-4 -- at shipping, 2026-10-14: `proposal-revisions.md` (new); `node-revision.md`, `proposal-{apply,queue}.md`, `decision-staging.md`, `queue-backup.md`, `tasks.md` "Backup", `agent-intake{,-documents}.md`, `spec-lock{,-accept}.md`, `spec-verify-output.md` "Hints", `architecture-code.md#spec-lock`, `plugin{,-files}.md`; core, store, CLI, HTTP READMEs, `ui/README.md`, `CLAUDE.md`; live-spec lines 05:99, 05:222, 06:97, 07:94, 08:41, 08 AC-13; `rev-rule.md`, `drift-queue.md`, `tested-evidence.md`.

## Out of scope

4b-2 `rev-rule`; 4c `drift-queue` (ADR-0045); slice 5 levels, lock format 2; 8a, 8b. A staged lock writer (Q5); `spec edit`; a per-node editorial choice in one proposal; `@rev` pins; raising markers; a lock here.

## Open

Decided 2026-10-10 (Q1-Q7), 2026-10-13 (Q8-Q14), 2026-10-14 (Q15, Q16), under the owner's pre-approval, review open; WA-n: the analysis (2026-10-10).

- Q1 the editorial proof: one commit of text and the lock's `spec_hash` (ADR-0043 amends ADR-0041). Q3 apply raises each node with an ID whose `norm_hash` changes, locked or not.
- Q5 a hand editorial edit of a locked node: the queue (`propose update`, `approve --editorial`) or `--no-verify`, then `spec lock accept --editorial`, or, once 4c ships, `spec propose drift <ID>` and `spec approve PR --editorial`. Q6 the agent's hint: rationale text only. Q7 an agent's revision change: refused at creation, an intake's `proposed_patch` too.
- Q8 split under ADR-0043, no new ADR: 4b-1 this file, 4b-2 `rev-rule.md`; 4c-1 may start after 4b-1; 4b-2's rule after 4b-1.
- Q10 an editorial approve refreshes only vouching entries; `spec lock accept --editorial` keeps "all at R". Q11 a raise into a closed class without `rev` refuses at step 6, naming both ways out. Q12 completing an apply made before 4b accepts step 5's text with or without the raise.
- Q14 the staged card copies the bare `spec approve PR-0004` with an Editorial row, the staged line ending in `--editorial`; typed flags replace the stage; the toggle unchecks on a revised version.
- Q15 a raise refused at step 6 for a meaning approve only (`REV_MAX`, a closed class, a write that cannot land) leaves the preview `applies`/`rebases`: `revisions` lists it, `to` `null`, `refused` the reason; the toggle shows, unchecked. Only an unreadable revision keeps `unavailable`. "First refused" is file order; notes: `lock_entries unknown` first, then one per refused node, file order.
- Q16 a commit made, branch and `HEAD` unmoved: apply stays `approved` and restores nothing, as for one path; its step-10 refusal names both files and quoted `:(literal)` commands to put them back. `HEAD` moved elsewhere: the old message.
- Q2, Q4, Q9, Q13, `--staged` beside `--changed`: `rev-rule.md` "Open".

## Implementation

| Module | What it does |
|---|---|
| core `raise.rs` (new), `front_matter.rs`, `lib.rs` | `paired`, `revision_change`, `meaning_changes`, `raised`, `closed_against_rev`, `write_revisions` (`Written`, `RevisionWriteError`), `RaiseRefusal` (`Unreadable`, `AtMax`, `ClosedClass`, `Unwritable`), `RevisionRow`; `rev_sites` locates the YAML spans |
| core `lock.rs`, `proposal.rs` | `vouches`; `refresh_editorial(…, Option<&str>)`; `commit_message_raising`, `ApplyTrailers`, `Revised`, the `Revision`/`Lock`/`Lock-mode` trailers, `EDITORIAL` |
| CLI `revisions.rs` (new) | `step_3_lock`, `step_6` (one pass), `editorial_refresh`, `lock_unchanged`, `restore`, `pathspec_word`, `restored_clause`, `lock_in_commit`, `raised_span`, `lock_for_review`, `rows`, `rows_text` |
| CLI `apply.rs`, `preflight.rs`, `main.rs`, `lib.rs` | `approve_editorial`, `editorial_refusal`; steps 3, 6, 8-10, Q16's clause; `trailer_commit` takes two paths under `Lock-mode: editorial`; `Held::holds_raised` |
| CLI `propose.rs`, `review.rs`, `stage.rs`, `proposals.rs`, `round.rs`, `state_file.rs` | Q7 at creation; `revisions` and its notes; `StageBody.editorial`; staged line `--editorial` last; dump schema 7 |
| CLI `verify.rs`, `lock.rs`, `code_walk.rs`, `verify_rows.rs` | RD-8's hints and pointers; accept passes no filter |
| store `queue.rs`, `queue/stage.rs`, `queue/state.rs`, `worktree.rs`, `git.rs`, `verify_git.rs`, `lib.rs` | `QUEUE_SCHEMA_VERSION` 7, `upgrade_stages` (migration and restore); `Stage::Approve.editorial`; `commit_only_paths` (`commit_only` wraps it); RD-8's docs |
| MCP `mirror.rs`; plugin; UI | `RevisionShape`, `StagedApprove.editorial`; skill and 0.7.2; `revisions.ts`, `RevisionList.tsx`, `DecisionDialog`, `StagedChoice`, `ProposalCard`, mock `editorial.ts` |

Deviations, accepted (iteration 1's numbers): 1 CLI pointers in the canon-name form (``canon `spec-verify`, "Rows"``): CLI `tests/genre.rs` forbids `docs/` and `ADR` in CLI sources; 2 `approve_editorial` a separate function, `ApproveFlags` kept the decision flags; 3 `commit_only` a one-path wrapper of `commit_only_paths`; 4 an unreadable revision refuses under `--editorial` too; 5 -> Q15; 6 a schema-6 dump's open approve stages upgraded on restore (idempotent); 7 the skill trimmed (4 263 -> 4 092 B of 4 096, no pinned phrase); 8 -> Q16; 9 the completion hint unchanged; 10 `editorial: false` from the daemon read as `null`. Iteration 2: Q15 in one pass, per-node write blame; `to` from the plan, never `rev + 1`; step 8 restores the text only while it holds this run's bytes; one `refreshable()` for rows and refresh; the `--editorial` no-change note above the question and in the notes, as the canon's other notes. Iteration 3: `pathspec_word` (`':(literal)<path>'`, `'` as `'\''`), byte-exact under sh, bash and zsh for a space, `[`, `*`, `?`, `'`, `$`, a backtick, `\`, `!`, non-ASCII and a newline; "stay as the commit left them"; the restore warning names the worktree.

Tests: CLI `tests/rev_raise.rs` (21), `rev_editorial.rs` (23, `restore_commands_quote_literal_pathspecs` included), `rev_complete.rs` (4), `rev_stage.rs` (3), `rev_common/`; store `queue_stage.rs` (+2); HTTP `stage.rs` (+1); UI `revisions.test.tsx`, `editorial.smoke.test.tsx`, `daemonKeys.test.ts`, `provisional.test.ts`; churn across the apply, stage, schema and plugin pins. Mutations: iteration 1, 12 named and 19 extra red; iteration 2, 7 of 9 red, two equivalent (the first raise blamed: only a document can be unwritable, always the first raise; step 8 restoring unconditionally: the text always holds this run's bytes there); UI 4 + 6 + 1 red.

Known limits: `proposal-revisions.md` "Known limits" (a `read_classes` failure or parser panic makes the preview `unavailable`; `write_revisions`' k+1 passes and the unreported pairwise failure; `git merge-file` adjacency; worktree-top-relative commands and `one_line`'s newline; step 8's untested race branches). Carried: to 8b, `editorial_apply_refreshes_tested`; to 4c-1 (`drift-queue.md` "Implementation"), `spec lock`'s raw undo pathspecs and the "left written and staged" docs in CLI `apply.rs`. Code pointers to this spec's "Data" headings may move to `docs/canon/proposal-revisions.md` when their files are next touched.
