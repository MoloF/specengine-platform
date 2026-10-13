---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4b-1 (4b split 2026-10-13, Q8; WA-1..WA-14); refreshed 2026-10-13 against shipped spec-lock and spec-verify; 05 s3.5; 07 s2; 08 s2, AC-13
adrs: [ADR-0043]
---

# rev-discipline (4b-1): approval raises a changed meaning's revision, or the owner calls it editorial

## Why

Nothing raises a revision: apply writes the text as sent, an agent may change `rev=`, and a meaning approve and an editorial one make the same commit. So "rewrote the meaning, coverage stayed green" (05 s3.5) stays possible. The shipped `spec lock accept --editorial` (`docs/canon/spec-lock-accept.md` "`--editorial`") reads the node's file, config and lock clean at `HEAD`, so it runs after the text commit, whose hook 4b-2's rule fails.

Delivered here, by ADR-0043 (amends ADR-0041; `architecture.md#apply`, `#layout`; `node-revision.md` "Raising"): the raise at apply; an agent's revision change refused at creation; `spec approve --editorial`, refreshing the vouching lock entries in the apply commit; its stage, schema 7, the review's `revisions`; UI; the skill. 4b's other half, 4b-2 (`docs/features/rev-rule.md`): `rev-not-raised` (AC-13), `spec bump`, `spec verify --staged`. 4b-1 adds no check. Built on the canon of slices 1, 3 and 4a: `code-bindings.md`, `node-revision.md`, `spec-lock.md`, `spec-lock-accept.md`, `spec-verify.md`, `spec-verify-output.md`.

## Description and interactions

- **The owner** approves as today, meaning by default (the prompt names the raise); `--editorial` or a staged `editorial: true` keeps revisions, the `[y/N]` confirming the lock write (ADR-0041). By hand: raise `rev` (4b-2's `spec bump` prints the line); an editorial edit of a locked node: Q5.
- **An agent** never writes a revision (Q7) and says "editorial" in the rationale, text only (Q6). The skill `plugin/specengine/skills/propose-spec-change/SKILL.md` gains (plugin 0.7.1 -> 0.7.2, PATCH, `plugin-files.md` "Changing it"): `Keep every rev as you read it: approval sets revisions. If you believe the change is editorial (wording, no meaning), say "editorial" in the rationale; the owner decides.`
- **This repository** (WA-10): no `spec.lock`, so no refresh. The raise reaches only nodes with an ID, here ADRs, whose closed `decision` class admits no `rev` (`specengine.toml` `[classes]`): an update changing an ADR's meaning is refused at step 6 (Q11), as "never edited, only superseded" already asks. Hook, CI unchanged.
- **Crates** (WA-13): core the raise's rows and line writes (pure), one vouching predicate (an entry of the node at R whose `spec_hash` is the given `norm_hash`; with 8b its `[[tested]]` block), `refresh_editorial` taking it as an optional filter (Q10), the stage's key; store `commit_only` over several paths (`worktree.rs`, one path today), queue schema 7; CLI apply's steps 3, 5-10, completion and reject's lookup; MCP `get_proposal`'s schema; UI; plugin; fixtures. HTTP no code; no new crate or dependency.
- **Later slices**: 4c takes schema 7, `approve --editorial` on a `drift` item (running `lock_accept --editorial`), `revisions` null for a kind writing no existing node; 5b code fields kept byte for byte; 4b-2, 8b the one predicate, 8b the one `refresh_editorial`.
- **Overridden live-spec lines**, fixed at shipping: 05:99 ("never by `apply_proposal`"), 05:222 ("To come: hashes recomputed"), 06:97 ("to come: `spec_hash` recomputed"), 07:94 (`spec approve … [--editorial]`).

## Data

### Raising at apply

WA-1, WA-2, Q3. Kinds `update`, `interpretation`, section-form `create` (a new file writes none). Each node with an ID in both step 3's read and step 5's text, by canonical ID, after step 6's structure check: `norm_hash` changed -> revision := current + 1 ("current" from the file at apply, so a rebase over a raise raises from it), kept under `--editorial`; unchanged -> kept; never lowered. A section's readable `rev=N` replaced in place, else ` rev=N` appended before its heading's `}`; a document's `rev:` value replaced, else `rev: N` inserted after its `id:` line, with that line's ending.

Refused at step 6, before the prompt, exit 1 naming the node: a current revision unreadable; one at `REV_MAX` (999 999 999, core `lock.rs`); a document's raise into a `closed` class listing `rev` in neither `required` nor `optional` (Q11): `` `ADR-0007`: class `decision` is closed and does not admit `rev`: admit it in [classes], or approve with --editorial ``.

```
## Cost {#R-12 rev=3}  ->  ## Cost {#R-12 rev=4}
## Range {#R-13}       ->  ## Range {#R-13 rev=2}
```

Prompt: `apply PR-0007 to docs/spec/combat.md on main in /w (applies; raises R-12 3 -> 4, R-13 1 -> 2)? [y/N]`; editorial `… (rebases, editorial: R-12 stays 3; spec.lock: 2 entries)? [y/N]`, the `spec.lock` part only when entries change. No `norm_hash` change: today's question; `--editorial` then adds `note: no node's meaning changes; --editorial changes nothing`. `--editorial` on a kind writing no existing node: exit 2, but on a `drift` item (4c, ADR-0045), whose approve runs `spec lock accept --editorial` for its node in a lock-only commit and raises no revision (`drift-queue.md` "Approve").

### Creation

WA-4, Q7. Creation's check 3 (so apply's step 6) refuses a text changing a revision as core `revision()` reads it: the `Revision` variant and its value both compared (`node-revision.md` "Revision"), so `Unwritten` <-> `Written(_)` is refused, `Written(1)` too: exit 1 `` `R-12`'s revision changed (3 -> 4): keep rev as read; approval sets revisions ``.

### Editorial refresh

WA-5, Q1, Q10. Under `--editorial`, step 3 also reads `<root>/spec.lock`: none -> text only; else tracked and equal to `HEAD` (exit 1, as accept's inputs, `spec-lock-accept.md` "Steps" 4), parsed by core `parse_lock(text, None)`, its `format` and `norm_recipe` then compared with this build's (exit 2 naming both; under 5b, format 1 exits 2 with the rebase hint, `lock-levels.md` "Old locks and the base"). `code_recipe` is ignored: a deliberate difference from accept's `read_lock`, which refuses any other recipe; the refresh writes no code field.

**Refreshed** (Q10): each changed node's vouching entries, at its revision R with `spec_hash` equal to its old `norm_hash` (step 3's), take the new one, through `refresh_editorial` with the predicate as filter; `spec lock accept --editorial` keeps "every entry at R". An entry at R with another `spec_hash` (an earlier unreviewed change: a regen leftover, a `--no-verify` commit) is left, still a Fix for verify. With 8b, a vouching `[[tested]]` block too (`tested-evidence.md` Q3). All else byte-identical: the code fields (format 1's `ast_hash`; format 2's `levels` through `body_bytes`, `lock-levels.md` "Accept"), `file_blob`, `commit`, `verified_*`, other entries, the header.

**Write and commit**: step 8 re-reads the lock (changed: exit 1 `spec.lock changed while being applied; nothing written`), then replaces each file atomically, the text first, the lock second. Step 9: `git commit --only … -- :(literal)<path> :(literal)<root>/spec.lock`. A failure restores both by accept's table and rules (`spec-lock-accept.md` "The lock commit"; CLI `lock.rs` `Undo`): each file only while it holds this run's bytes, a failed restore a warning naming its git command, never "restored". Step 10: one parent, the old `HEAD`; exactly those two paths changed; `Proposal:` the ID; one `Lock:` trailer per refreshed node; the committed `spec.lock` blob the bytes confirmed (accept's fourth check, `check_lock_commit`: a hook rewriting the lock fails it, a commit deleting it too); each refreshed `spec_hash` the committed text blob's `norm_hash`. No entry refreshed: the path alone, as today.

```
Base-commit: <hex>
Revision: R-12 3 (editorial)
Lock: R-12
Lock-mode: editorial
```

Trailers after `Base-commit:`, `Interprets:` last; a meaning approve's `Revision: R-12 3 -> 4` per raised node, file order; `Lock:` per node whose entries changed, then one `Lock-mode`. Completion and reject's lookup take the path alone, or with `spec.lock` and `Lock-mode: editorial`. Completion accepts step 5's text with or without the raise (Q12), so an apply interrupted before 4b-1 still completes.

### Review document, stage, queue

WA-11. Review key `revisions` after `conflict` (47 keys; `decision-staging.md` has 46): `null` when the kind writes no existing node or the preview `conflicts` or is `unavailable`; else per node whose `norm_hash` changes (step 3 against step 5), `to` a meaning approve's, `lock_entries` the vouching entries an editorial one refreshes (0 without a lock; `null` and a note if unreadable); text `revisions: R-12 3 -> 4 (2 lock entries)`. The approve stage gains `editorial` (`true` or `null`) after `canon`, 7 keys; `true` on a kind writing no existing node: refused when staged, a corrupt row when found:

```json
"revisions": [{"id": "R-12", "rev": 3, "to": 4, "lock_entries": 2}]
{"decision":"approve","option":null,"answer":null,"canon":null,"editorial":true,"note":"typo","span_hash":"<target span hash>"}
```

A bare `spec approve PR` confirms it, note kept, its staged line (`decision-staging.md`) ending in `--editorial`: ``staged <at>: spec approve PR-0004 --note "typo" --editorial``; typed flags, `--editorial` too, replace it whole, its note dropped (Q14). Queue schema 7 (store `QUEUE_SCHEMA_VERSION` 6 today; 4c takes 8) adds `"editorial":null` to open approve stages (older binaries refused). `get_proposal`'s schema gains both. No column or event: the trailers are the record.

### UI

WA-12, Q14. Accept in the decision dialog gets "Editorial: keep the revision", shown only when `revisions` is a non-empty array, staged `editorial: true`; a version revised while the dialog is open unchecks it (the default is a meaning approve). The staged card copies the bare `spec approve PR-0004` (the stage confirms, its note kept) and shows an "Editorial: keep the revision" row; the card lists `revisions` as returned. The UI and daemon ship together: every approve POST sends `editorial` (`null` unless checked); an older daemon refuses the unknown key.

## Rules and edge cases

- WHEN a proposal is applied, the system SHALL keep the revision of a whitespace-, heading- or move-only change (ADR-0040); a heading rename meeting an earlier raise of that line conflicts at step 5 (exit 1).
- WHEN an editorial approve meets entries at R that do not vouch for the old text, the system SHALL leave them byte-identical (Q10).
- Two branches' refreshes of one entry meet in `spec lock regen`, one may drop: verify shows it as `spec_ahead same_revision`. Genre-neutral: the raise keys only on canonical IDs, `norm_hash`, `[classes]` and the lock.

## Acceptance criteria

rust-developer (mutation -> the test that must fail):

- [ ] RD-1 -- raising: `meaning_approve_raises_each_changed_node`, `editorial_approve_keeps_revisions`, `whitespace_only_change_keeps_revision`, `apply_never_lowers_a_revision`, `unchanged_nested_section_keeps_its_revision`, `document_rev_line_follows_id_with_its_line_ending`, `unreadable_current_revision_refuses_at_step_6`, `raise_at_max_revision_refuses`; from the base -> `rebased_apply_raises_from_the_current_revision`; a raise into a closed class without `rev` written -> `raise_into_a_closed_contract_refuses_at_step_6`.
- [ ] RD-2 -- editorial: a second commit -> `editorial_apply_is_one_commit_of_text_and_lock`; a code field touched (`ast_hash`; under 5b, `levels` through `body_bytes`) -> `editorial_refresh_keeps_code_hashes_and_commit`; an unfiltered refresh -> `only_vouching_entries_are_refreshed`; accept's `read_lock` -> `editorial_refresh_ignores_code_recipe`; `dirty_lock_refuses_editorial_apply`, `editorial_without_a_lock_writes_text_only`, `editorial_prompt_names_the_lock_write`, `step_10_checks_the_lock_hash_against_the_committed_blob`, `hook_rewriting_the_lock_fails_step_10`, `editorial_commit_deleting_the_lock_fails_step_10`, `failed_editorial_commit_restores_text_and_lock`; a block at R skipped -> `editorial_apply_refreshes_tested` (the later of 4b-1 and 8b, `tested-evidence.md`).
- [ ] RD-3 -- creation: a revision change accepted -> `proposal_changing_a_revision_is_refused`; compared by `number()` -> `unwritten_to_written_one_is_refused`.
- [ ] RD-4 -- completion, reject: `interrupted_editorial_apply_completes`, `interrupted_meaning_apply_completes_with_the_raise`, `reject_refuses_with_a_two_path_apply_commit`; the raise required -> `unraised_apply_commit_completes` (Q12).
- [ ] RD-5 -- stage, schema: `editorial_on_a_new_file_is_a_usage_error`, `staged_editorial_is_confirmed_by_a_bare_approve`, `typed_flags_replace_a_staged_editorial`, `schema_7_rewrites_open_approve_stages`.
- [ ] RD-6 -- review: `review_document_every_key_present` (47), `revisions_null_for_a_new_file`, `revisions_read_the_current_file`.
- [ ] RD-7 -- a kind literal -> the genre scan; a new dependency -> the CLI pin in `build_graph.rs`.
- [ ] RD-8 -- carry-overs, code only, the code-reviewer checks: 4a's (`docs/features/spec-verify.md` "Implementation", last paragraph): CLI `verify.rs` conflict hint "raise the marker to its revision" -> "set the marker to its revision"; store `git.rs` module doc and `Staged::blob`'s "each OID is requested once": requested again once `read_files` has moved it out; `verify_git.rs` module doc "each OID once" -> "once per `read_files` call"; code pointers to that feature retargeted to canon `docs/canon/spec-verify.md`, `spec-verify-output.md`, headings verbatim ("The `[verify]` table", "Rows", "States", "Cascade", "Base", "Verdict and exit", "Hints"). After Q7 no proposal raises `rev`: CLI `verify.rs` `SAME_REVISION_STEP` and accept's refusal (`lock.rs`) say "raise its `rev` by hand, commit, then accept; or `--editorial`" -> `same_revision_hint_names_a_raise_and_editorial` (4b-2 adds `spec bump`).

test-engineer:

- [ ] TE-1 -- apply through a hook running `spec check --staged --root .`, a lock from `fixtures/spec-lock/code-a.lock` or CLI `tests/verify_common/`'s four accepted `verify-ecs` nodes: meaning 3 -> 4; editorial stays 3, vouching entries refreshed, a non-vouching one byte-identical, `spec verify` `ok`; a rebase 3 -> 4 -> 5; nested sections; a document; CRLF; an interpretation's trailers; a closed class without `rev` refused at step 6, `--editorial` passes.
- [ ] TE-2 -- interrupted before and after the commit: completed, reject refused; a pre-4b-1 apply commit (no raise) completes.
- [ ] TE-3 -- an HTTP stage `editorial: true` -> the terminal's staged line; migration 6 -> 7; `daemon-keys.json` (stage 7, review 47).
- [ ] TE-4 -- carry-overs: `crates/specengine-code/tests/code_bind.rs`'s "Binding rules" -> `code-bindings.md` "Rust binding"; `code_refs_walk.rs`'s accept pointer -> `spec-lock-accept.md` "Steps". Expected churn: apply tests committing a meaning change now carry the raise (`proposal_apply.rs`, `proposal_content.rs`, `proposal_kinds.rs`).

ui-developer:

- [ ] UI-1 -- shown on a file-form create card -> `editorial_toggle_hidden_without_revisions`. UI-2 -- the flag dropped from the stage, or the staged card losing its Editorial row -> the contract test against the mock; the copied command stays bare.
- [ ] UI-3 -- the UI computing `rev + 1` -> the mock with `to` 9 for rev 3 fails. UI-4 -- provisional types (`StagedChoice`, the review document), `?scenario=editorial` "Mock data"; typecheck, lint, tests.

spec-writer:

- [x] SW-1 -- this spec, refreshed and split 2026-10-13 (`rev-rule.md`, 08 s2, the drafts pointing at either half). SW-2 -- ADR-0043; `architecture.md` `#apply`, `#layout` (detail in `node-revision.md` "Raising"), `#implementation-base`. SW-3 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: "Raising"'s approval bullet as built; `proposal-apply.md` (steps 3, 5-10, completion, reject, the "Known limits" rev line), `proposal-queue.md`, `decision-staging.md` (stage 7 keys, review 47, schema 7, the staged line and the card's Editorial row, Q14); `spec-lock-accept.md` "Steps" 6 and `spec-verify-output.md` "Hints" (no "proposal raising `rev`"; "set the marker"); `architecture-code.md#spec-lock` (the approve writer built); `plugin-files.md` (0.7.2); the overridden lines; CLI, core, store READMEs; `CLAUDE.md`.

## Risks

- The largest to build: apply's one-path invariants (steps 3, 8-10, completion, reject's lookup, section-form create, `commit_only`).
- Raises on unlocked nodes (Q3) reach every project using the queue, `[code]` or not: apply diffs gain `rev=` lines; closed classes meet Q11. Writing revisions: flow-style front-matter, CRLF, attribute blocks. Schema 7 refuses older binaries.

## Out of scope

4b-2 `rev-rule` (the rule, `spec bump`, `verify --staged`); 4c `drift-queue` (ADR-0045); slice 5 levels, lock format 2; 8a `verify-tests`, 8b `tested-evidence`. A staged lock writer (Q5); `spec edit`; a per-node editorial choice in one proposal; `@rev` pins; raising markers; a lock here.

## Open

- Q1 the editorial proof: one commit of text and the lock's `spec_hash` (ADR-0043 amends ADR-0041). Q3 apply raises each node with an ID whose `norm_hash` changes, locked or not.
- Q5 a hand editorial edit of a locked node: the queue (`propose update`, `approve --editorial`) or `--no-verify`, then `spec lock accept --editorial`, or, once 4c ships, `spec propose drift <ID>` and `spec approve PR --editorial` on its `spec_ahead` item (ADR-0045). Q6 the agent's hint: rationale text only. Q7 an agent's revision change: refused at creation.
- Q8 split under ADR-0043, no new ADR: 4b-1 this file (identifier `rev-discipline`), 4b-2 `rev-rule.md`; 4c may start after 4b-1; 4b-2's rule after 4b-1 (else every meaning apply on a locked node fails its hook), its `verify --staged` needs only 4a.
- Q10 an editorial approve refreshes only vouching entries (`spec_hash` equals the old `norm_hash`, at R) via `refresh_editorial` with an optional filter; `spec lock accept --editorial` keeps "all at R".
- Q11 a raise into a closed class without `rev` refuses at step 6, before the prompt, exit 1, naming the class and both ways out (admit `rev` in `[classes]`, or `--editorial`).
- Q12 completing an apply made before 4b accepts step 5's text with or without the raise.
- Q14 the staged card copies the bare `spec approve PR-0004` (the stage confirms, its note kept) with an "Editorial: keep the revision" row; the terminal's staged line ends in `--editorial`; a typed `--editorial` would replace the stage, dropping its note. A version revised while the dialog is open unchecks the toggle. UI and daemon ship together: every approve POST sends `editorial`, `null` by default; an older daemon refuses the key. Decided 2026-10-13 under the owner's pre-approval, review open.
- Q2, Q4, Q9, Q13 and `--staged` beside `--changed`: `rev-rule.md` "Open".

Q1-Q7 decided 2026-10-10, Q8-Q14 2026-10-13, under the owner's pre-approval, review open; WA-n: the analysis (2026-10-10).

## Implementation

Not started. To record: modules, deviations with reasons. Criteria renumbered at the split (2026-10-13), old -> new: RD-3 -> 1, RD-4 -> 2, RD-7 -> 3, RD-6 -> 4, RD-8 -> 5, RD-9 -> 6, RD-12 -> 7 (in `rev-rule.md` too), RD-13 -> 8; TE-2 -> 1, TE-3 -> 2, TE-6 -> 3, TE-4 new; SW-1, SW-2 kept, SW-3 dropped (its drafts shipped), SW-5 -> SW-3, SW-4's approval part kept; the rest in `rev-rule.md`.
