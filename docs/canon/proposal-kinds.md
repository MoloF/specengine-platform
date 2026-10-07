---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
owner: owner
reviewed: 2026-10-07
---

# Proposal kinds: create

Phase 2 slice 6 (2026-10-07): a node comes into existence through the queue. Kind `create` adds a new spec file (**file form**) or new `{#ID}` sections in one node's span (**section form**), applied by `spec approve` as one commit where it was raised (ADR-0004, ADR-0005, ADR-0032). The proposer names the path and every new ID (ADR-0008); the queue reserves them while the proposal is live. `update` keeps its node set (`proposal-queue.md` "Creation" 3); decision kinds: `agent-intake.md`, `decision-record.md`; `interpretation`, `amendment`, `decision`: later slices. Code: core `create.rs` (`id_sites`, `written_id` -> `NoId|Alias|LookAlike|Id`, `add_sections`), `record_form`; store `ProposalKind::Create`, `reserved()`; CLI `create.rs` (propose, the new file's apply), `propose.rs` (`SpanRule::Sections`), `preflight.rs` (section steps, `file_commit`); MCP `intake.rs`.

## Forms

`spec propose create TARGET [--base HASH] --text-file F|- --rationale T [--author-role R] [--author-model M] [--run ID] [--brief]`: caps, outputs (`PR-0001`, `introduced: <n>`), `--brief`, JSON as `propose update`'s (`proposal-queue.md` "Commands").

- **File form**: `TARGET` a `.md` path no indexed file has, no `--base`; the text is the whole file, byte for byte; the commit `A <path>`.
- **Section form**: `TARGET` resolving as an update's (Creation 1, its refusals), `--base` its span hash (Creation 2); the text is the span with new `{#ID}` sections below the target's level; the commit `M <path>`.

## Propose

In order; the first refusal answers, exit 1 unless named; nothing stored or reserved:

1. **Form** (exit 2): clap, the author grammar; the place bound (Creation 5) before the text is read, so an unbound root exits 2 first.
2. **Target**. A path no indexed file has: `--base` -> ``nothing at `<path>`: a new file is written against no base; drop --base``; the path clean, `.md`, in the walk (core `Paths::in_walk_scope`; else `` `<p>` is no file the walk would list ``), not under `[paths] generated`, not `[paths] index` nor a shard (live or archive) of the `index = true` `[[generators]]` entry (read when the root's check tables load): `` `<p>` is the project's index (`[paths] index`) | the index's archive shard | a shard of the index: only `spec export index` writes it, never a proposal ``; no symlink or non-directory on its way, nothing there (a dangling symlink too) nor in git's index: `` `<p>` exists: a create never replaces a file; to add ID sections to it, name its span_hash with --base ``. Else the section form: Creation 1-2; no `--base` -> that `exists`.
3. **Text**: UTF-8, <= `TEXT_MAX_BYTES`, verbatim, parsed under the recorded root's scheme (a parser panic: refused). File form: `class: generated` refused. Section form: Creation 3's splice under the section rule (core `add_sections`): the file's (ID, heading level) pairs kept in order; refused: an ID dropped or moved, the `id:` changed, a level changed, a heading at the target's level or above, `` no new ID: the text adds no `{#ID}` section; `spec propose update` replaces a span ``. `update`'s rule is unchanged: any added ID refused.
4. **New IDs**: the file form's `id:` and `{#ID}`s; the section form's added sections and any ID-shaped `{#...}` the parser took as no definition (an alias prefix, a look-alike) the span did not hold. Each in text order, canonical (`slug/ID` for a feature-scoped prefix in a feature document, ADR-0026), checked in order:
   - an `aliases_from` prefix -> `` `QST-033` (line N) is written with a legacy `aliases_from` prefix: a new ID is written with its canonical prefix, `Q-033` ``;
   - a look-alike or mixed script -> exit 2 `line N: <the Latin fix>` (core `fix`, ADR-0009);
   - a number not as core `record_id` writes it (`record_form`) -> `` `R-7` (line N): its prefix's numbers are written as `R-07` `` (past `width` passes: `id-width` is a finding);
   - the `[decision_records]` prefix -> `` `DEC-0024`: these records are made by `spec approve` of a `question` or `discrepancy` ``;
   - twice -> `` `X` is defined twice in the text (lines a and b) ``;
   - taken: defined or aliased in the recorded root's index, refreshed now -> `` `X` is defined in `<path>` (`id: Y` | no `id:`) `` (`is an alias in`); else held by a live create -> `` `X` is reserved by `PR-0001` (open), a live create ``; a number shape adds ``; the next free is `Z` ``.
   An `immutable_text` prefix may be created in a new file; its sections are no section-form target (Creation 1).
5. **Validation**: Creation 4 with the file added (or swapped) against `HEAD`; introduced findings stored: `ref-dangling`, `file-name`, `id-scope`, `id-width`, `id-not-in-scheme` never refuse (ADR-0012).
6. **Insert** ("Reservation").

## Reservation

A live (`open`, `approved`) create of the project, from any worktree or repository, holds its new IDs. The store's `create` reads them **inside its `Immediate` transaction**, under the write lock it inserts under: two creates of one ID never both store. The loser gets `QueueError::Reserved {id, by}`, nothing written; the CLI answers with step 4's refusal, the holder's state read again (left out if no longer live) and the next free ID. Read-only `reserved()` -> `[Reservation {id, by, status}]` by proposal number, then text order; a row whose targets do not read holds none. Reject or apply frees them (applied: the corpus's); an orphan holds them until rejected.

**Next free** (number shapes): one more than the highest number of the prefix or its `aliases_from` **in the ID's own scope** (bare, or one feature slug), over the index (defined or aliased), the live creates and the text's own new IDs, written by `record_id`; gaps may be named.

## Queue row

No schema bump of its own (now 4, 41 columns: `tasks.md`). File form: `target_id` the text's `id:` canonical, else the path; `target_path` the path; `target_ids` the new IDs in text order when `target_id` heads them, else `[path, new IDs]`; `base_hash`, `base_text` `NULL`; `new_text` the file; `patch_hash` `b3_hash(target_id LF LF new_text)`. Section form: an update's columns, `target_ids` `[target_id, new IDs]` (an id-less feature document: `[<path>, <slug>/AC-08]`). Both: place, `rationale`, `author`, `diagnostics` as an update's; intake and record columns `NULL`.

New IDs of a row: `target_ids` but the first, plus the first with no base when it is an ID. Corrupt, named: `target_ids` `NULL` or not a JSON list of IDs headed by `target_id`; the base set in part; `new_text`, `rationale` `NULL`; an intake or record column set. An older build reads it as corrupt by its `kind` (ADR-0017).

Store: `ProposalKind::Create` (`applies()` true, `decides()` false); `NewProposal {base_hash?, base_text?, new_ids}`; `Proposal::new_file()` (no base), `target_ids()`. `create` -> `Invalid`: an update without its base or with new IDs, a base in part, an empty base hash, a new ID empty or repeated, a new file's `target_id` neither its path nor its first new ID.

**Review document**: an update's keys; a new file: `base_hash`, `base_text` `null`, `diff` against an empty base (`--- base <path>`, `+++ proposed <path>`, `@@ -0,0 +1,<n> @@`), `preview` `applies` or `unavailable`. **Inbox** `<id> | create | <status> | <target_id> | ...`. Backup (`queue-backup.md`), events unchanged.

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

**Completion** by the `Proposal:` trailer, as an update's; a new file: one parent, exactly `A <target_path>`, its blob `new_text` byte for byte; else why (`changes M a, A b`, `has N parents`, `does not carry the proposal's text`). Hint: ``a commit on `<branch>` with the trailer `Proposal: PR`, one parent, adding only `<path>` with the proposal's text completes it``. **Killed run** (file form, between 8 and 9): the file holding `new_text`, its entry intent-to-add; approve refuses at step 4 naming two ways out: commit it by hand (`git commit --only --trailer 'Proposal: PR' -m 'spec: apply PR' -- <path>`), then `spec approve PR` completes it; or `git rm --cached -- <path>`, delete the file, `spec approve PR` writes it again. **Reject**, orphans: an update's (`proposal-apply.md` "Reject"); a rejected create frees its IDs.

## MCP and plugin

`propose_change {kind, target, base?, text, rationale, author_role, author_model?, run?}`: `kind` required, enum `["update", "create"]` from core `UPDATE_KIND`, `CREATE_KIND` (`ChangeKind`); `base` nullable, not required; `create` = `spec propose create TARGET [--base B] --text-file - --rationale R A --brief` (`propose_create_brief`); an `update` with `base` `null` or absent -> invalid-params (-32602) `base: ...`. `INSTRUCTIONS`: `mcp-read.md` "Texts".

Plugin (since 0.1.4), `propose-spec-change` "A new section or file": `kind: "create"`; a new file with `base: null`, its ID in `id:`; ID sections against the span's `span_hash`; the agent names each new ID. Taken by the spec or another's proposal: resend with the named next free ID. **Held by the agent's own earlier proposal** (a corrected create): never the next free ID, which would add the node twice; name that proposal, ask the owner to reject it, resend once `get_proposal` shows it rejected. Removing an ID section stays a question for the owner.

## Genre

Kind names from core constants only (`CREATE_KIND` beside `QUESTION_KIND`); no `[ids]` prefix, kind, `docs/` or `records/` literal in the create sources (`proposal_genre.rs`).

## Known limits

Accepted at shipping (2026-10-07); none blocks (ADR-0012).

- An ID created outside the queue on another branch: `id-taken` at merge. A stale open create holds its IDs until rejected; a wrong number guess costs one refused call.
- Git's index lookup is case-sensitive (`:(literal)`): on a case-insensitive file system an index-only `R-15.md` does not stop a create at `r-15.md`; the apply fails at step 9 (pathspec did not match), cleaned up, exit 1, nothing lost. Decision records alike.
- Between step 8's re-check and the write only the hard link guards: untestable without a product hook, so `rename` there is untested.
- A generated directory or index outside the walk gets the walk's wording, not its own (exit 1 either way).
