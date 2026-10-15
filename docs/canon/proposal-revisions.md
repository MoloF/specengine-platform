---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-10-15
---

# Proposal apply: revisions and the editorial approve

ADR-0043 (`architecture.md#apply`, `#layout`; the rule: `node-revision.md` "Raising"), built by slice 4b-1 (`docs/features/rev-discipline.md`, 2026-10-14). What `spec approve` adds to `proposal-apply.md`'s steps for the kinds writing an existing node, `update`, `interpretation` and a section-form `create`; a new file and a deciding kind write no revision. Code: core `raise` (pure: pairs, the raise and its refusals, the lines written; `revision_change`, `RaiseRefusal`, `write_revisions`, `RevisionRow`), `lock::{vouches, refresh_editorial}`, `proposal::commit_message_raising`; CLI `revisions.rs` (steps 3, 6, 8-10, completion, the review's rows), `apply.rs` (`approve_editorial`); store `WorktreeGit::commit_only_paths`.

## Creation

Creation's check 3 (`proposal-queue.md` "Creation"), so apply's step 6 and an intake's `proposed_patch` (`agent-intake.md` "Rules" 4), refuses a text changing a revision as core `revision()` reads it, variant and value both (`Unwritten` -> `Written(1)` too): exit 1 `` `R-12`'s revision changed (3 -> 4): keep rev as read; approval sets revisions `` (`none written` for none). Approval sets revisions; a proposal never does.

## The raise

Step 6, after the structure check. Nodes with an ID in both the file as read (step 3) and step 5's text are paired by canonical ID, the k-th holder with the k-th. A pair whose `norm_hash` input changes (both null: its own text compared) is raised to the current revision + 1, "current" from the file at apply (a rebase over a raise raises from it), never lowered; a whitespace-, heading- or move-only change keeps it (ADR-0040). Written: a section's readable `rev=N` in place, else ` rev=N` before its heading's `}`; a document's `rev:` value, else `rev: N` after its `id:` line with that line's ending. The result is parsed again and kept only when each raised node reads its new revision, every other node its old one, the structure and every meaning input as before; else the write is refused (a flow-style front-matter), never half done. `write_revisions` writes every raise together; failing, each alone, blaming the ones failing alone (all, with the joint reason, when each passes alone).

Every node is judged in one pass; refusals exit 1 before the prompt, the first in file order named:

- an unreadable current revision refuses any approve, `--editorial` too (no R to keep), the preview `unavailable`: `` `X`'s revision is unreadable (a `bad-rev`, or a front-matter that fails): write a readable revision by hand and commit it, then approve ``;
- a meaning approve only (Q15: the preview stays `applies` or `rebases`, the review lists the node): at `REV_MAX` (999 999 999) `` `X` is at rev 999999999, the largest revision: it cannot be raised; approve with --editorial ``; a document of a `closed` class listing `rev` in neither `required` nor `optional` `` `ADR-0007`: class `decision` is closed and does not admit `rev`: admit it in [classes], or approve with --editorial ``; a write that cannot land `` `X`: its revision cannot be written: <why>; approve with --editorial to keep it ``.

Question `apply PR-0007 to docs/spec/combat.md on main in /w (applies; raises R-12 3 -> 4, R-13 1 -> 2)? [y/N]` (`, staged` last). The commit adds `Revision: R-12 3 -> 4` per raised node, file order, after `Base-commit:`, `Interprets:` still last (`proposal-interpretation.md` "Apply").

## Editorial approve

`spec approve PR --editorial [--note T]` (`approve_editorial`), or a staged `editorial: true` ("Stage and UI"), keeps every revision. On a kind writing no existing node: exit 2 `` `--editorial` keeps the revisions of the nodes an apply changes; `PR` is a <kind>, which writes no existing node: `spec approve PR` without it; nothing changed ``. No node's meaning changing: the note `no node's meaning changes; --editorial changes nothing`, above the question and in the outcome's notes.

- **Step 3** also reads `<root>/spec.lock`: none, the text alone; modified, staged, deleted, untracked or not tracked -> exit 1 `` `spec.lock` <state>: --editorial sets its entries from its committed bytes; commit it first; nothing written ``; parsed by `parse_lock(text, None)`, another `norm_recipe` -> exit 2 naming both. `code_recipe` is not read, unlike accept's `read_lock`: no code field is written.
- **Step 6, the refresh**: each changed node's **vouching** entries (core `vouches`: at its revision R, `spec_hash` its old `norm_hash`) take the new one (`refresh_editorial` with that filter; `spec lock accept --editorial` passes none: every entry at R). A node with a null hash refreshes none. An entry at R with another `spec_hash` (an earlier unreviewed change), the code fields, `file_blob`, `commit`, `verified_*`, other entries and the header stay byte-identical.
- **Question** `apply … (rebases, editorial: R-12 stays 3; spec.lock: 2 entries)? [y/N]`, the lock part only when entries change; none: the text alone is written and committed, as a meaning approve.
- **Step 8**: the lock read again (changed: exit 1 `spec.lock changed while being applied; nothing written`), the text written, then the lock. A failed lock write restores the text only while it holds this run's bytes: ``cannot write spec.lock: <error>; `<path>` restored; nothing committed``.
- **Step 9**: `git commit --only … -- :(literal)<path> :(literal)<root>/spec.lock`. No commit made: each file restored only while it holds this run's bytes (accept's rule, CLI `lock.rs` `Undo`): `their bytes restored`, else per file `` `<path>` restored ``, `` … was changed meanwhile, so it is left as it is, not restored ``, `` … is not restored (see the warning) ``. A failed restore warns `` cannot restore `<top path>` in <worktree>: <error>; `git checkout -- ':(literal)<top path>'` restores it ``.
- **Step 10**: one parent, the old `HEAD`; exactly the two paths changed; a `Lock:` trailer per refreshed node; the committed `spec.lock` blob the confirmed bytes (a hook rewriting it, a commit deleting it fail); each refreshed `spec_hash` the committed text blob's `norm_hash`. Else it stays `approved`, exit 1. A commit made with branch and `HEAD` unmoved (Q16; a hook resetting it): nothing restored, the refusal ending ``; `<path>` and `spec.lock` stay as the commit left them in <worktree>: `git reset -q -- 'A' 'B'`, then `git checkout -- 'A' 'B'` puts them back``, A and B `':(literal)<top path>'`, an embedded `'` as `'\''` (`pathspec_word`); `HEAD` moved elsewhere: the one-path text.

```
Base-commit: <hex>
Revision: R-12 3 (editorial)
Lock: R-12
Lock-mode: editorial
```

`Revision: <ID> <rev> (editorial)` per changed node, file order; `Lock:` per node whose entries changed, then one `Lock-mode: editorial`.

## Completion and reject

The trailer lookup (`proposal-completion.md` "Completion") completes, and reject refuses on, a commit changing the path alone, or the path and `<root_rel>/spec.lock` when its `Lock-mode:` is `editorial`; its span equal to step 5's text with or without the raise (Q12: an apply made before 4b-1 completes). The hint keeps "changing only `<path>`".

## Review

The review document's `revisions` after `conflict` (`proposal-review.md`): `null` when the kind writes no existing node or the preview `conflicts` or is `unavailable`; else per node whose `norm_hash` changes, file order, `{id, rev, to, lock_entries, refused}` (core `RevisionRow`): `to` the meaning approve's, `null` when refused; `lock_entries` the vouching entries of the worktree's `spec.lock` an editorial approve refreshes (0 without one; unreadable: `null`, note `lock_entries unknown: <why>`); `refused` `null`, else the step-6 reason as printed after `(step 6): `. Text `revisions: R-12 3 -> 4 (2 lock entries), ADR-0007 1 -> refused (0 lock entries)`, `1 lock entry` for one, `none` for `[]`; notes `lock_entries unknown` first, then per refused node `meaning approve not applicable (step 6): <refused>`. MCP `get_proposal`'s schema: `RevisionShape`. `fixtures/daemon-keys.json`: `RevisionRow`.

## Stage and UI

The approve stage's `editorial` (`decision-staging.md` "The stage"): `true` or `null`, after `canon`; the daemon's `StageBody` reads `false` as `null`; `true` on a kind writing no existing node is a usage refusal (400) when staged, a corrupt row when found (``a staged `editorial` on <kind>, which writes no existing node``). A bare `spec approve PR` confirms it, note kept, the staged line ending in `--editorial`: ``staged <at>: spec approve PR-0004 --note "typo" --editorial``; typed flags, `--editorial` too, replace the stage whole. Queue schema 7 (`proposal-queue.md` "Store"): no column; open approve stages gain `"editorial":null`, a restored schema-6 dump's too (`queue-backup.md` "Format").

UI: Accept in the decision dialog offers "Editorial: keep the revision" only when `revisions` is a non-empty array (a refused entry too), unchecked, unchecked again when the version is revised; checked stages `editorial: true`, else `null` (every approve POST sends the key: UI and daemon ship together). The card lists `revisions` as returned in "Revisions" (`R-12 rev 3 → 4 (2 lock entries)`, a refused one with its reason; `[]`: "No node's meaning changes: approval keeps every revision."; `null`: no section); the staged card shows an "Editorial" row and copies the bare `spec approve PR-0004`. A meaning approve staged over a refused raise is not refused at the stage; step 6 refuses it. Mock: `?scenario=editorial`.

## Known limits

- A `read_classes` failure or a parser panic in the raise makes the preview `unavailable` too.
- `write_revisions` runs k+1 write passes on a failure; a failure between raises that each pass alone, beside one failing alone, is not reported (a meaning approve refuses anyway).
- `git merge-file`: a heading rename meeting an earlier raise of that line, or a raised heading directly followed by a body line the next proposal edits (adjacency), conflicts at step 5.
- The Q16 and restore commands (`spec lock accept`'s too) are worktree-top-relative: run in `<worktree>` (named); pasted below the top they name nothing and fail harmlessly (later: `:(top,literal)`). `one_line` turns a newline in a path into a space in the step-10 reason, the printed command then naming another path (a control character in a spec path is not refused).
- Step 8's restore finds the text changed or fails only through a race: untested.
- Two branches' refreshes of one entry meet in `spec lock regen`, which may drop one: verify then shows `spec_ahead same_revision`.
