---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: proposal-kinds analysis 2026-10-06, all accepted; 08 s2 Phase 2, slice 6; builds `create` only
shipped: 2026-10-07
---

# Proposal kinds: create

## Why

No node, not even a first tree, could come through the queue (08 s4.1): `propose update` refuses an added `{#ID}` or `id:`, and the `propose-spec-change` skill deferred to the owner. The kind `create` adds a new spec file, or new `{#ID}` sections in a node's span, applied by `spec approve` as one commit where it was raised. No new ADR: ADR-0004, 0005, 0032, 0012, 0008, 0031, 0009, 0026, 0003, 0013, 0017.

Working answers (the owner's night rule 2026-10-06, open to his review): Q1 the proposer names each new ID, reserved while live (else: issued at approve, a placeholder rewritten); Q2 one kind for files and sections, `update` keeps its node set; Q3 `kind` on `propose_change`, no new tool; Q4 no `[decision_records]` ID by create; Q5 no schema bump; Q6 an amendment will be a file by create (`amends:`, 06 s7); Q7 a decision and its canon diff: two commits.

How it works now: `docs/canon/proposal-kinds.md`.

## Data

Moved to the canon at shipping: `docs/canon/proposal-kinds.md` "Forms", "Queue row", "MCP and plugin"; the section line-ending rule `proposal-apply.md` "Apply steps" 5; the schema `agent-intake.md` "Tools"; `INSTRUCTIONS` sizes `mcp-read.md` "Tools"; plugin 0.1.4 the root `README.md` "Claude Code plugin".

## Rules and edge cases

Moved to `docs/canon/proposal-kinds.md` "Propose" (the refusal texts as built), "Reservation", "Apply", "Completion, killed run, reject", "Genre", "Known limits".

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a`, `-b`, committed; scratch `HOME`; clock `2026-10-06T12:00:00Z`; a git identity; library calls, consent yes; T13 = `R-12.md` with `id: R-13` and its own H1. Refused: the exit named, nothing stored, written or reserved. M: the mutation turning it red. Tests: CLI `proposal_kinds.rs` unless named.

- [x] AC-01 -- spec-a `propose create docs/records/R/R-13.md` T13: `PR-0001`; review `kind` `create`, `target_id` `R-13`, `target_ids` `["R-13"]`, base `null`, `new_text` T13; `git status --porcelain` empty (`ac01_a_new_file_is_proposed_and_nothing_is_written`; M: writing at propose).
- [x] AC-02 -- a file modified, another staged; approve: one commit, one parent, exactly `A docs/records/R/R-13.md`, blob T13, four trailers; both files as before; `spec show R-13` resolves (`ac02_approve_commits_exactly_the_new_file`; M: staging the whole tree).
- [x] AC-03 -- propose `exists`: `R-12.md` without `--base`, a dangling symlink, an index-only `R-15.md`; X at the path after propose: `apply_failed` step 4; X by the consent callback: step 8, X intact (`ac03_a_path_holding_anything_is_refused_at_propose`, `ac03_a_file_at_the_path_stops_the_apply_at_step_4_or_8`; M: step 4 not re-checked. "Rename over the path" survives: "Known limits").
- [x] AC-04 -- exit 1: `id: R-12` naming its file and `R-13`; `TERM-tired` naming `TERM-exhausted`; `QST-033` naming `Q-033`; `R-7`, `R-007` naming `R-07`; `{#RULE-STAM-REGEN}` naming `docs/spec/movement/stamina.md`; an ID twice. **Corrected**: `id: \u0410-103` (U+0410 for `A`) exit 2 naming `A-103`; `id: \u0420-13` (U+0420 is a `P` look-alike) reads `P-13`, no prefix `P`: stored, `id-not-in-scheme` (`ac04_new_ids_are_checked_in_text_order_against_the_index`; M: `id:` checked, `{#...}` not).
- [x] AC-05 -- `PR-0001` open, then `approved`: `id: R-13` at `R-13-b.md` exit 1 naming `PR-0001`, `R-14`; stored after reject; a second handle -> `Reserved` (`ac05_a_live_create_reserves_its_new_ids`; store `ac05_a_live_create_holds_its_new_ids_against_every_handle`, `ac05_parallel_creates_of_one_new_id_store_one`; M: the in-transaction check dropped).
- [x] AC-06 -- section form on `RULE-STAM-REGEN` + `### Rest delay {#EDGE-STAM-REST}`: `target_ids` both; approve `M stamina.md`, parent `RULE-STAM-REGEN`; exit 1: at `##`, no new ID, `{#EDGE-STAM-ZERO}` dropped, a level changed, by `propose update` (`ac06_new_sections_are_added_below_the_target`, `ac06_a_section_create_keeps_every_id_and_level`; M: update's rule relaxed).
- [x] AC-07 -- `Base rate 10` -> `12` committed: `rebases`, both written; a line after `Exhausted` instead: `conflicts`, exit 1, `apply_failed` step 5 (`ac07_a_moved_span_rebases_or_conflicts`; M: `new_text` over the current span).
- [x] AC-08 -- T13 + `derived_from: [R-99]`: `introduced: 1`, stored, applied (`ac08_findings_are_stored_never_a_refusal`; M: findings refusing).
- [x] AC-09 -- exit 1: `class: generated`; `docs/generated/`, `templates/`, `.txt`, `../`; a section on immutable `R-12`; `id: DEC-0024` naming `spec approve` (`ac09_what_only_a_generator_or_spec_approve_writes_is_refused`, `ac09_the_generated_directory_inside_the_walk_is_named`; M: the `[decision_records]` check dropped).
- [x] AC-10 -- MCP `kind: "create"`, `base` null or absent = the `--brief --json` twin; `update` with `base: null` -> error naming `base`; schema `kind` `["update","create"]`, `base` nullable; `git status` empty (MCP `ac10_propose_change_creates_as_its_twin`, `ac10_the_schema_takes_create_and_a_nullable_base`; M: `base` required).
- [x] AC-11 -- spec-b `REQ-003` stores and applies; its Cyrillic alias prefix exit 1 naming `REQ-003`; `GLS-worktree` naming its file; genre tests green (`ac11_spec_b_creates_with_its_own_scheme`, `proposal_genre.rs` `proposal_kinds_ac11_the_create_sources_name_no_prefix_kind_or_directory`, core `genre.rs`, MCP `mcp_genre.rs`; M: a `"create"` literal outside core).
- [x] AC-12 -- committed by hand: `applied`, no commit; other bytes: `does not carry the proposal's text`; reject refused; killed between 8 and 9: step 4 names two ways out (`ac12_a_new_file_committed_by_hand_completes_it`, `ac12_other_bytes_by_hand_complete_nothing`, `ac12_a_killed_run_names_its_two_ways_out`; M: completion ignoring the blob).
- [x] AC-13 -- `--option`, `--answer`, `--canon` exit 2, no event; `--base` on a new path `drop --base`; a node without `--base` exit 1 (`ac13_decision_flags_and_a_misplaced_base_are_refused`; M: a decision flag accepted).
- [x] AC-14 -- diff `@@ -0,0 +1,<n> @@`; `applies`, a file there `unavailable`; inbox `| create |`; export, import, re-export byte-identical, schema 3, 40 columns, `user_version` 3 (`ac14_review_inbox_and_backup_carry_a_create`; store `ac14_creates_keep_schema_3_and_40_columns`; M: a column or a bump).
- [x] AC-15 -- `propose-spec-change` teaches `create`; removal only stays the owner's; `plugin.json` 0.1.4, `PINS` appended (`plugin_skills.rs`, `plugin_files.rs`; M: the skill edited, no bump).
- [x] AC-16 -- no `crates/*/src` comment cites `decision-apply`; eval `anonymity`, `doc_pointers` green; docs gate clean, worst W not above shipping's (`ac16_the_source_comments_cite_the_decision_record_canon`; M: a heading `decision-record.md` lacks).
- [x] AC-17 -- `INSTRUCTIONS` 1 740 B, 1 897 with `probes` (<= 2 048 - 140); `mcp_decision.rs`, `mcp_path.rs` re-pin 1 740 and BLAKE3 `4b08078c...1c0e` (MCP `ac17_the_instructions_name_create_within_their_budget`, `ac17_the_probes_build_sends_the_short_probe_paragraph`; M: the probe paragraph at 366 B).

## Implementation

Canon: `docs/canon/proposal-kinds.md` (new); `proposal-queue.md`, `proposal-apply.md`, `agent-intake.md`, `mcp-read.md`; core, store, CLI, MCP READMEs; root `README.md`; 05 s3.3, 07 s1.2, s2, 08 s2; `CLAUDE.md`. Two iterations; review 1 accepted (2 minor, 1 spec defect, 4 nits; m3, n1, n2, n4 fixed in iteration 2, n3 a known limit).

| Module | What it does |
|---|---|
| core `create.rs` (new), `record.rs`, `intake.rs`, `front_matter.rs` | `id_sites`, `written_id`, `add_sections`; `record_form`; `CREATE_KIND` |
| store `queue.rs` | `ProposalKind::Create`, optional base, `new_ids`, `Reservation`, `reserved()`, `Reserved` in the insert, corrupt create rows |
| CLI `create.rs` (new) | propose create, `Corpus`, `NewIds`, next free, the race refusal; new file apply steps 2-10, killed-run hint |
| CLI `propose.rs`, `preflight.rs`, `apply.rs`, `review.rs`, `proposals.rs`, `inbox.rs`, `decide.rs`, `main.rs` | `SpanRule::Sections`; section steps, step 5 line endings, `file_commit`; dispatch; preview, keys, flags |
| MCP `intake.rs`, `server.rs`; plugin | `ChangeKind`, nullable `base`; `INSTRUCTIONS`; skill, 0.1.4 |

Tests: CLI `proposal_kinds.rs` (22), store `queue_kinds.rs` (6), MCP `mcp_create.rs` (3 per build); expectations moved in 10 files. Core, store, CLI, MCP 1 143/1 143; mutations 23/24 red, the 24th (`hard_link` -> `rename`) unreachable without a product hook.

Accepted deviations, now canon: (1) CLI comments cite `` canon `decision-record`, "<Heading>" `` (CLI genre forbids `docs/`); (2) apply step 5 gives a section's merge sides a final LF, updates too; (3) next free per scope, the text's IDs counted; (4) binding before the text: an unbound root exits 2 first. Iteration 2: a section's new-ID check after step 5; the race refusal names the holder's state; the index and its shards refused; the skill's own-proposal rule.

Owner's check, open: on a scratch copy in a real terminal, `spec propose create` a new file, `spec approve PR`: the prompt ends `(new file)? [y/N]`, `git show --stat` one added file, `spec inbox --all` `| create |`; plugin 0.1.4 updated and restarted, an agent's `propose_change` with `kind: "create"` answers `target_ids`.
