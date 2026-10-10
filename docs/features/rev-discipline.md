---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4b (WA-1..WA-14; Q1-Q7 decided under the owner's pre-approval, review open); 05 s3.5; 07 s2; 08 s2, AC-13
adrs: [ADR-0043]
---

# rev-discipline: a changed meaning raises the revision, or the owner calls it editorial

## Why

Nothing raises a revision: apply writes the text as sent, an agent may change `rev=`, and a meaning approve and an editorial one make the same commit. So "rewrote the meaning, coverage stayed green" (05 s3.5) stays possible, and the drafted `spec lock accept --editorial` hashes committed bytes: it runs after a text commit whose hook the rule would fail.

Delivered, by ADR-0043 (amends ADR-0041; `architecture.md#apply`, `#layout`; `node-revision.md` "Raising"): the raise at apply; `spec approve --editorial`, refreshing the lock in the same commit; `rev-not-raised` (AC-13); `spec bump`; `spec verify --staged`. Needs slices 1, 3, 4a as drafted.

**Owner review: risk 3, "content never blocks".** The rule refuses a commit over a spec change, beside ADR-0012 and `docs/README.md` "Enforcement" ("content never does"). The reading taken: it judges whether a locked node's change was *declared* (a raise or an editorial refresh), never what the text says; ADR-0019 took it from tracey; `[check] mode` gates it (`observe` never blocks), debt covers it. Read as content, it becomes a warning and AC-13 is unmet. "Enforcement" gets the clause at shipping (SW-4).

## Description and interactions

- **The owner** approves as today, meaning by default (the prompt names the raise); `--editorial` or a staged `editorial: true` keeps revisions, the `[y/N]` confirming the lock write (ADR-0041). By hand: raise `rev` (`spec bump` prints the line); an editorial edit of a locked node: Q5.
- **An agent** never writes a revision (Q7) and says "editorial" in the rationale, text only (Q6). The `propose-spec-change` skill gains (plugin PATCH bump): `Keep every rev as you read it: approval sets revisions. If you believe the change is editorial (wording, no meaning), say "editorial" in the rationale; the owner decides.`
- **A project** in `enforce` or `enforce-introduced` gets the rule with its first committed `spec.lock`; its hook may run `spec verify --staged` on `[code]` files and `spec.lock` (documented, not shipped).
- **This repository** (WA-10): no `spec.lock`, nothing in scope: output byte-identical, `CHECK_CODES` 36; hook, CI unchanged; `verify --staged` not wired (no `[code]`). With every ID in scope, each ADR edit would need `rev:`, which the closed decision class refuses.
- **Crates** (WA-13): core the rule, the raise, bump's rows (pure); store lock blobs, a multi-path `commit_only`, queue schema 7; CLI, MCP, UI, plugin, fixtures; HTTP no code; no new crate or dependency.
- **Overridden live-spec lines**, fixed at shipping: 05:103, 07:105-106 (`bump` writing revisions, `--editorial` the lock), 05:224, 06:97, 08:44, 08:82 (Q4).

## Data

### Raising at apply

WA-1, WA-2, Q3. Kinds `update`, `interpretation`, section-form `create` (a new file writes none). Each node with an ID in both step 3's read and step 5's text, by canonical ID, after step 6's structure check: `norm_hash` changed -> revision := current + 1 ("current" from the file at apply, so a rebase over a raise raises from it), kept under `--editorial`; unchanged -> kept; never lowered. A section's readable `rev=N` replaced in place, else ` rev=N` appended before its heading's `}`; a document's `rev:` value replaced, else `rev: N` inserted after its `id:` line, with that line's ending. A current revision unreadable: exit 1 at step 6 naming the node.

```
## Cost {#R-12 rev=3}  ->  ## Cost {#R-12 rev=4}
## Range {#R-13}       ->  ## Range {#R-13 rev=2}
```

Prompt: `apply PR-0007 to docs/spec/combat.md on main in /w (applies; raises R-12 3 -> 4, R-13 1 -> 2)? [y/N]`; editorial `… (rebases, editorial: R-12 stays 3; spec.lock: 2 entries)? [y/N]`, the `spec.lock` part only when entries change. No `norm_hash` change: today's question; `--editorial` then adds `note: no node's meaning changes; --editorial changes nothing`. `--editorial` on a kind writing no existing node: exit 2.

### Creation

WA-4, Q7. Creation 3 (so apply's step 6) refuses a text changing a revision as core `revision()` reads it, compared as values (`Unwritten` -> `Written(1)` too: SW refinement): exit 1 `` `R-12`'s revision changed (3 -> 4): keep rev as read; approval sets revisions ``.

### Editorial refresh

WA-5, Q1. Under `--editorial`, step 3: no `<root>/spec.lock` -> text only; else tracked and clean (exit 1, as accept's step 4), this build's `format` and `norm_recipe` (exit 2). A changed node's entries at its revision whose `spec_hash` equals its old `norm_hash` get the new one; all else byte-identical (`ast_hash`, `file_blob`, `commit`, `verified_*`, other entries, the header with `code_recipe`). Step 8 re-reads the lock (changed: exit 1), writes both atomically; step 9 `git commit --only … -- :(literal)<path> :(literal)<root>/spec.lock`, a failure restoring both; step 10: exactly those paths, each refreshed `spec_hash` = the committed blob's `norm_hash`.

```
Base-commit: <hex>
Revision: R-12 3 (editorial)
Lock: R-12
Lock-mode: editorial
```

Trailers after `Base-commit:`, `Interprets:` last; a meaning approve's `Revision: R-12 3 -> 4` per raised node, file order; `Lock:` per node whose entries changed, then one `Lock-mode`. Completion and reject's lookup take the path alone, or with `spec.lock` and `Lock-mode: editorial`.

### The rule `rev-not-raised`

WA-6, WA-7, WA-14, Q2. One error code (`CHECK_CODES` 36), in check, not verify (WA-14), under `--staged` and `--changed` only. **In scope**: a node whose canonical ID is at `HEAD` and in the checked tree, its `HEAD` revision readable and `norm_hash` non-null, `HEAD`'s `spec.lock` holding an entry for it at that revision with `spec_hash` = that `norm_hash`. **Finding**: the checked `norm_hash` non-null and different, the checked revision unreadable or not above `HEAD`'s, and the checked lock not carrying the new hash on each vouching entry (same key, same `rev`). **Lock**: `--staged` the index's blob (`GIT_INDEX_FILE`, apply's `--only` index too), `--changed` the disk; none at `HEAD`: nothing in scope; either unparseable, another `format` or `norm_recipe`, unmerged under `--staged`: cannot check (exit 2); `code_recipe` ignored. Always introduced; `[check] mode` decides; debt covers it.

```
error  docs/spec/combat.md:14: rev-not-raised: `R-12` changed at rev 3, which spec.lock vouches for: raise it (rev=4), or have the owner approve it through the queue with --editorial
```

Line: the heading or the `id:` line; subject the ID; `fix` the raised line as apply writes it. Unlocked nodes, marker-only bindings, `@N` pins: no finding (bump's `suggested`).

### spec bump

WA-8. `spec bump [--staged] [--json]` (+ `--root`, `--config`), read-only: no file, index entry, git object, queue row or event. The disk (as `check --changed`) or `--staged` the index, against `HEAD`. A row per node with an ID at `HEAD` whose `norm_hash` changed and revision is not above `HEAD`'s, by path and line; `required` when the rule finds it, else `suggested`; `to` = `HEAD`'s + 1:

```
required | R-12 | docs/spec/combat.md:14 | rev 3 -> 4 | 2 lock entries | ## Cost {#R-12 rev=4}
suggested | R-13 | docs/spec/combat.md:30 | rev 1 -> 2 | 0 lock entries | ## Range {#R-13 rev=2}
hint: suggested: raising it makes markers naming it lag
```

`hint: required:` gives the finding's ways out. JSON `{rows: [{need, id, path, line, rev, to, lock_entries, write}]}`; an unreadable revision `rev ?`, `rev` and `write` null. None: `nothing to raise`. Exit 0 when run; 2 on check's run or the rule's lock causes; `--editorial` a usage error.

**`spec verify --staged`** (WA-9): 4a's judge over the git index (documents, `[code]` files, `spec.lock`, `specengine.toml` unless `--config`), level 1 by the index's OIDs against `file_blob` (no `hash-object`), base `HEAD`; with `--changed`: exit 2; exits by `[verify] mode`.

### Review document, stage, queue

WA-11. Review key `revisions` after `conflict` (47 keys; place: SW refinement): `null` when the kind writes no existing node or the preview `conflicts` or is `unavailable`; else per node whose `norm_hash` changes (step 3 against step 5), `to` a meaning approve's, `lock_entries` an editorial one's (0 without a lock; `null` and a note if unreadable); text `revisions: R-12 3 -> 4 (2 lock entries)`. The approve stage gains `editorial` (`true` or `null`) after `canon`, 7 keys; `true` on a kind writing no existing node: refused when staged, a corrupt row when found:

```json
"revisions": [{"id": "R-12", "rev": 3, "to": 4, "lock_entries": 2}]
{"decision":"approve","option":null,"answer":null,"canon":null,"editorial":true,"note":"typo","span_hash":"<target span hash>"}
```

A bare `spec approve PR` confirms it, printing `spec approve PR --editorial`; typed flags replace it whole. Queue schema 7 adds `"editorial":null` to open approve stages (older binaries refused). `get_proposal`'s schema gains both. No column or event: the trailers are the record.

### UI

WA-12. Accept in the decision dialog gets "Editorial: keep the revision", shown only when `revisions` is a non-empty array, staged `editorial: true`, copied `spec approve PR-0004 --editorial`; the card lists `revisions` as returned.

## Rules and edge cases

- WHEN a proposal is applied, the system SHALL keep the revision of a whitespace-, heading- or move-only change (ADR-0040); a heading rename meeting an earlier raise of that line conflicts at step 5 (exit 1).
- WHEN one commit is checked from two roots, another cwd or creation order, findings and rows SHALL be byte-identical. Genre-neutral: IDs and the lock only.
- Two branches' refreshes of one entry meet in `spec lock regen`, one may drop; it and the escapes (`--no-verify`, `observe`, no hook, CI's plain check) show in verify as `spec_ahead same_revision`.

## Acceptance criteria

rust-developer (mutation -> the test that must fail):

- [ ] RD-1 -- the rule, pure: every ID judged -> `unlocked_node_change_is_no_finding`; evidence ignored -> `refreshed_entries_pass_at_the_same_revision`; one of two refreshed -> `partial_refresh_is_a_finding`; a lower revision -> `lowered_revision_is_a_finding`; the heading hashed -> `heading_rename_is_no_finding`; by path -> `moved_section_is_no_finding`; a non-vouching entry -> `pre_existing_lock_drift_is_out_of_scope`; `std::fs` -> `model_and_core_sources_do_no_file_io`.
- [ ] RD-2 -- wiring: runs plain -> `plain_check_has_no_rev_finding`; the default index -> `rule_reads_the_lock_through_git_index_file`; `observe` blocks -> `observe_never_blocks_a_rev_finding`; `code_recipe` read -> `other_code_recipe_lock_still_checks`; an invalid lock passes -> `invalid_lock_is_cannot_check`. `CHECK_CODES` 36; `check_parity.rs` unchanged.
- [ ] RD-3 -- raising: `meaning_approve_raises_each_changed_node`, `editorial_approve_keeps_revisions`, `whitespace_only_change_keeps_revision`, `apply_never_lowers_a_revision`, `unchanged_nested_section_keeps_its_revision`, `document_rev_line_follows_id_with_its_line_ending`, `unreadable_current_revision_refuses_at_step_6`; from the base -> `rebased_apply_raises_from_the_current_revision`.
- [ ] RD-4 -- editorial: a second commit -> `editorial_apply_is_one_commit_of_text_and_lock`; `ast_hash` touched -> `editorial_refresh_keeps_code_hashes_and_commit`; `only_vouching_entries_are_refreshed`, `dirty_lock_refuses_editorial_apply`, `editorial_without_a_lock_writes_text_only`, `editorial_prompt_names_the_lock_write`, `step_10_checks_the_lock_hash_against_the_committed_blob`.
- [ ] RD-5 -- the raise removed -> `apply_commits_pass_the_revision_rule` (a real hook, a lock). RD-7 -- a revision change accepted -> `proposal_changing_a_revision_is_refused`.
- [ ] RD-6 -- `interrupted_editorial_apply_completes`, `interrupted_meaning_apply_completes_with_the_raise`, `reject_refuses_with_a_two_path_apply_commit`.
- [ ] RD-8 -- `editorial_on_a_new_file_is_a_usage_error`, `staged_editorial_is_confirmed_by_a_bare_approve`, `typed_flags_replace_a_staged_editorial`, `schema_7_rewrites_open_approve_stages`.
- [ ] RD-9 -- `review_document_every_key_present` (47), `revisions_null_for_a_new_file`, `revisions_read_the_current_file`.
- [ ] RD-10 -- any write (status, `HEAD`, index, queue) -> `bump_writes_nothing`; `bump_marks_vouched_nodes_required`, `bump_has_no_editorial_flag`.
- [ ] RD-11 -- `verify_staged_judges_the_index_not_the_disk`, `verify_staged_runs_no_hash_object`, `staged_and_changed_exclude_each_other`.
- [ ] RD-12 -- a kind literal -> the genre scan; a new dependency -> the CLI pin in `build_graph.rs`.

test-engineer:

- [ ] TE-1 -- AC-13: temp repos from `fixtures/code-a/`, slice 3's golden lock, a hook running `spec check --staged --root .` under `enforce`: a locked node edited without a raise refused, naming it and both ways out; raised by hand, whitespace-only, an unlocked node pass; `observe` passes, the finding in `--json`; `enforce-introduced` refuses. M: the rule off.
- [ ] TE-2 -- apply through the hook: meaning 3 -> 4; editorial stays 3, entries refreshed, `spec verify` `ok`; a rebase 3 -> 4 -> 5; nested sections; a document; CRLF; an interpretation's trailers. TE-3 -- interrupted before and after the commit: completed, reject refused.
- [ ] TE-4 -- `--changed` = `--staged` on equal trees; plain unchanged. TE-5 -- an invalid lock exits 2; another `code_recipe` judged; unmerged; two refreshes through `regen`.
- [ ] TE-6 -- an HTTP stage `editorial: true` -> the terminal's staged line; migration 6 -> 7; `daemon-keys.json` (stage 7, review 47). TE-7 -- `bump` rows, read-only. TE-8 -- `verify --staged`, index unlike disk.
- [ ] TE-9 -- here: an ADR edit staged, no finding; the hook untouched. TE-10 -- `fixtures/spec-c`, no finding; the source scan. TE-11 -- two roots, cwd, creation order.
- [ ] TE-12 -- pilots, manual, scratch clones: silent without a lock; with slice 3's TE-11 lock, in-scope count and timing; no pilot name committed.

ui-developer:

- [ ] UI-1 -- shown on a file-form create card -> `editorial_toggle_hidden_without_revisions`. UI-2 -- the flag dropped from stage or copied command -> the contract test against the mock.
- [ ] UI-3 -- the UI computing `rev + 1` -> the mock with `to` 9 for rev 3 fails. UI-4 -- provisional types (`StagedChoice`, the review document), `?scenario=editorial` "Mock data"; typecheck, lint, tests.

spec-writer:

- [x] SW-1 -- this spec. SW-2 -- ADR-0043; `architecture.md` `#apply`, `#layout` (detail in `node-revision.md` "Raising"), `#implementation-base`. SW-3 -- `spec-lock.md`, `spec-verify.md` amended; 08 s2 marks 4b draft. SW-5 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: "Raising" as built; `proposal-apply.md` (steps 3, 5-10, completion, the "Known limits" rev line), `proposal-queue.md`, `decision-staging.md`, `spec-check*.md`, `docs/README.md` "Enforcement" (risk 3); the overridden lines; 08 AC-13 (Q4) met; CLI, core, store READMEs; `CLAUDE.md`.

## Risks

- **Content never blocks** (risk 3): "Why". The largest to build: apply's one-path invariants (steps 9-10, completion, reject's lookup, section-form create, `commit_only`).
- Writing revisions: flow-style front-matter, CRLF, attribute blocks; a raise in a closed contract without `rev` gets `key-extra` (not used here). Schema 7 refuses older binaries; `verify --staged` parses changed code blobs (AC-10).

## Out of scope

4c `drift-queue`; slice 5 levels, lock format 2; slice 8 `verify --tests` writes. A staged lock writer (Q5); a commit-msg hook; a CI range check; `spec edit`; a per-node editorial choice in one proposal; `@rev` pins; raising markers; Health rows; a shipped hook script; a lock here.

## Open

- Q1 the editorial proof: one commit of text and the lock's `spec_hash` (ADR-0043 amends ADR-0041).
- Q2 the rule's scope: nodes a `HEAD` lock entry vouches for; none here. Q3 apply raises each node with an ID whose `norm_hash` changes, locked or not.
- Q4 AC-13 becomes "a locked node's `norm_hash` changed with neither a revision raise nor an editorial refresh of its lock entries" (08, at shipping).
- Q5 a hand editorial edit of a locked node: the queue (`propose update`, `approve --editorial`) or `--no-verify`, then `spec lock accept --editorial`. Q6 the agent's hint: rationale text only. Q7 an agent's revision change: refused at creation.

Decided under the owner's pre-approval, review open; WA-n: the analysis (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. To record: modules, TE-12's figures, deviations with reasons.
