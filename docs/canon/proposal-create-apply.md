---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-09
---

# Proposal kinds: applying a create

How `spec approve` applies a `create` (`proposal-kinds.md`) and an amendment, a new file too (`proposal-amendment.md`, which adds its step-4, step-6 and consent lines). Code: CLI `create.rs` (`approve_file`, the completion of a new file), `preflight.rs` (the section form's steps); store `create_file`.

## Apply

`spec approve PR [--note T]`; `--option`, `--answer`, `--canon` -> exit 2, no event (`` ...; `PR` is a create: `spec approve PR` applies it as proposed ``). The trailer lookup first ("Completion"); refusals logged and reopened as an update's (`proposal-apply.md` "Runs").

**File form** (`create::approve_file`): `decision-record.md` "Steps", `new_text` the record, no ID issued:

- **2** Place (exit 2). **3** The recorded root's config; the path's propose-2 rules (exit 1).
- **4** The index refreshed; the path free as at propose (`exists`, or "Killed run"'s two ways out); the new IDs free in that index (its own reservation aside).
- **5** The text parsed, not generated. **6** Propose 4 again, no queue read, giving exactly the stored new IDs, else ``the text of `<path>` defines A under the recorded root's `[ids]` now, the proposal B``. The identity.
- **1** Consent: `apply PR-0001 to <path> on <branch> in <worktree> (new file)? [y/N]`.
- **7** `approve_from`. **8** The place and step 4 re-checked; store `create_file` (a hard link: never replaces an entry).
- **9** Intent-to-add, `git commit --only` of the path; failed: entry, file, new directories removed, `` the commit of `<path>` failed, the file removed: <git's error> ``.
- **10** One parent, the old `HEAD`, name-status exactly `A <path>`, its blob `new_text`, `Proposal:` the ID -> `applied_with`, `update_paths`.

**Section form**: an update's steps (`proposal-apply.md` "Apply steps", step 5's section line-ending rule included). Once step 5 finds the text not in place, step 4's check: each stored new ID free in the refreshed index (else exit 1 at step 4 naming the holder; a step-5 conflict is told first); a text already in place is step 5's refusal or a completion. Step 6: the section rule on the merged file, adding exactly the stored new IDs, else `` `<target>`: the merged text adds A, the proposal B ``.

## Completion, killed run, reject

**Completion** by the `Proposal:` trailer, as an update's (`proposal-completion.md`); a new file: one parent, exactly `A <target_path>`, its blob `new_text` byte for byte; else why (`changes M a, A b`, `has N parents`, `does not carry the proposal's text`). Hint: ``a commit on `<branch>` with the trailer `Proposal: PR`, one parent, adding only `<path>` with the proposal's text completes it``. **Killed run** (file form, between 8 and 9): the file holding `new_text`, its entry intent-to-add; approve refuses at step 4 naming two ways out: commit it by hand (`git commit --only --trailer 'Proposal: PR' -m 'spec: apply PR' -- <path>`), then `spec approve PR` completes it; or `git rm --cached -- <path>`, delete the file, `spec approve PR` writes it again. **Reject**, orphans: an update's (`proposal-completion.md` "Reject"); a rejected create or amendment frees its IDs.

## Known limits

- Git's index lookup is case-sensitive (`:(literal)`): on a case-insensitive file system an index-only `R-15.md` does not stop a create at `r-15.md`; the apply fails at step 9, cleaned up, exit 1, nothing lost. Decision records alike.
- Between step 8's re-check and the write only the hard link guards; untested (no product hook).
