---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
owner: owner
reviewed: 2026-10-09
---

# Proposal kinds: create

Phase 2 slice 6 (2026-10-07): a node comes into existence through the queue. Kind `create` adds a new spec file (**file form**) or new `{#ID}` sections in one node's span (**section form**), applied by `spec approve` as one commit where it was raised (ADR-0004, ADR-0005, ADR-0032). The proposer names the path and every new ID (ADR-0008); the queue reserves them while the proposal is live. `update` keeps its node set (`proposal-queue.md` "Creation" 3); deciding kinds: `agent-intake.md`, `decision-record.md`; `interpretation`, `decision`, `amendment` (a new file too, sharing this page's rules): `proposal-<kind>.md`. Code: core `create.rs` (`id_sites`, `written_id` -> `NoId|Alias|LookAlike|Id`, `add_sections`), `record_form`; store `ProposalKind::Create`, `reserved()`; CLI `create.rs` (propose, the new file's apply), `propose.rs` (`SpanRule::Sections`), `preflight.rs` (section steps, `file_commit`); MCP `intake.rs`.

## Forms

`spec propose create TARGET [--base HASH] --text-file F|- --rationale T [--author-role R] [--author-model M] [--run ID] [--brief]`: caps, outputs (`PR-0001`, `introduced: <n>`), `--brief`, JSON as `propose update`'s (`proposal-queue.md` "Commands").

- **File form**: `TARGET` a `.md` path no indexed file has, no `--base`; the text is the whole file, byte for byte; the commit `A <path>`.
- **Section form**: `TARGET` resolving as an update's (Creation 1, its refusals), `--base` its span hash (Creation 2); the text is the span with new `{#ID}` sections below the target's level; the commit `M <path>`.

## Propose

In order; the first refusal answers, exit 1 unless named; nothing stored or reserved:

1. **Form** (exit 2): clap, the author grammar; the place bound (Creation 5) before the text is read: an unbound root exits 2 first.
2. **Target**. A path no indexed file has: `--base` -> ``nothing at `<path>`: a new file is written against no base; drop --base``; the path clean, `.md`, in the walk (core `Paths::in_walk_scope`; else `` `<p>` is no file the walk would list ``), not under `[paths] generated`, not `[paths] index` nor a shard (live or archive) of the `index = true` `[[generators]]` entry: `` `<p>` is the project's index (`[paths] index`) | the index's archive shard | a shard of the index: only `spec export index` writes it, never a proposal ``; no symlink or non-directory on its way, nothing there (a dangling symlink too) nor in git's index: `` `<p>` exists: a create never replaces a file; to add ID sections to it, name its span_hash with --base ``. Else the section form: Creation 1-2; no `--base` -> that `exists`.
3. **Text**: UTF-8, <= `TEXT_MAX_BYTES`, verbatim, parsed under the recorded root's scheme (a parser panic: refused). File form: `class: generated` refused; a front-matter declaring `amends` -> `` `<p>` declares `amends`: raise it as `spec propose amendment <p>`; nothing stored ``; one that does not read is no refusal (stored with its finding, step 5). Section form: Creation 3's splice under the section rule (core `add_sections`): the file's (ID, heading level) pairs kept in order; refused: an ID dropped or moved, the `id:` changed, a level changed, a heading at the target's level or above, `` no new ID: the text adds no `{#ID}` section; `spec propose update` replaces a span ``. `update`'s rule is unchanged: any added ID refused.
4. **New IDs**: the file form's `id:` and `{#ID}`s; the section form's added sections and any ID-shaped `{#...}` the parser took as no definition (an alias prefix, a look-alike) the span did not hold. Each in text order, canonical (`slug/ID` for a feature-scoped prefix in a feature document, ADR-0026), checked in order:
   - an `aliases_from` prefix -> `` `QST-033` (line N) is written with a legacy `aliases_from` prefix: a new ID is written with its canonical prefix, `Q-033` ``;
   - a look-alike or mixed script -> exit 2 `line N: <the Latin fix>` (core `fix`, ADR-0009);
   - a number not as core `record_id` writes it (`record_form`) -> `` `R-7` (line N): its prefix's numbers are written as `R-07` `` (past `width` passes: `id-width` is a finding);
   - the `[decision_records]` prefix -> `` `DEC-0024`: these records are made by `spec approve` of a `question`, `discrepancy` or `decision` (`spec propose decision` raises one); nothing stored `` (`deciding_kinds()`, by `decides()`);
   - twice -> `` `X` is defined twice in the text (lines a and b) ``;
   - taken: defined or aliased in the recorded root's index, refreshed now -> `` `X` is defined in `<path>` (`id: Y` | no `id:`) `` (`is an alias in`); else held by a live create or amendment -> `` `X` is reserved by `PR-0001` (open), a live create `` (the holder's kind); a number shape adds ``; the next free is `Z` ``.
   An `immutable_text` prefix may be created in a new file; its sections are no section-form target (Creation 1).
5. **Validation**: Creation 4 with the file added (or swapped) against `HEAD`; introduced findings stored: `ref-dangling`, `file-name`, `id-scope`, `id-width`, `id-not-in-scheme` never refuse (ADR-0012).
6. **Insert** ("Reservation").

## Reservation

A live (`open`, `approved`) create or amendment (`adds_nodes()`) of the project, from any worktree or repository, holds its new IDs. The store's `create` reads them **inside its `Immediate` transaction**, under the write lock it inserts under: two creates of one ID never both store. The loser gets `QueueError::Reserved {id, by, kind}`, nothing written; the CLI answers with step 4's refusal, the holder's state read again (left out if no longer live) and the next free ID. Read-only `reserved()` -> `[Reservation {id, by, kind, status}]` by proposal number, then text order; a row whose targets do not read holds none. Reject or apply frees them (applied: the corpus's); an orphan holds them until rejected.

**Next free** (number shapes): one more than the highest number of the prefix or its `aliases_from` **in the ID's own scope** (bare, or one feature slug), over the index (defined or aliased), the live creates and the text's own new IDs, written by `record_id`; gaps may be named.

## Queue row

No schema bump of its own (now 6, 44 columns). File form: `target_id` the text's `id:` canonical, else the path; `target_path` the path; `target_ids` the new IDs in text order when `target_id` heads them, else `[path, new IDs]`; `base_hash`, `base_text` `NULL`; `new_text` the file; `patch_hash` `b3_hash(target_id LF LF new_text)`. Section form: an update's columns, `target_ids` `[target_id, new IDs]` (an id-less feature document: `[<path>, <slug>/AC-08]`). Both: place, `rationale`, `author`, `diagnostics` as an update's; intake and record columns `NULL`.

New IDs of a row: `target_ids` but the first, plus the first with no base when it is an ID. Corrupt, named: `target_ids` `NULL` or not a JSON list of IDs headed by `target_id`; the base set in part; `new_text`, `rationale` `NULL`; an intake or record column set. An older build reads it as corrupt by its `kind` (ADR-0017).

Store: `ProposalKind::Create` (`applies()`, `adds_nodes()` true; `decides()`, `amends()` false; each an exhaustive match; `proposal_genre.rs` refuses `==` on `Create` or `Amendment`); `NewProposal {base_hash?, base_text?, new_ids}`; `Proposal::new_file()` (`adds_nodes()`, no base), `target_ids()`. `create` -> `Invalid`: an update without its base or with new IDs, a base in part, an empty base hash, a new ID empty or repeated, a new file's `target_id` neither its path nor its first new ID.

**Review document**: an update's keys; a new file: `base_hash`, `base_text` `null`, `diff` against an empty base (`--- base <path>`, `+++ proposed <path>`, `@@ -0,0 +1,<n> @@`), `preview` `applies` or `unavailable`. **Inbox** `<id> | create | <status> | <target_id> | ...`. Backup (`queue-backup.md`), events unchanged.

## Apply

`spec approve PR [--note T]`: the file form's steps (`create::approve_file`, one commit `A <path>`), the section form's (an update's, plus the new-ID checks), completion, killed run, reject: `proposal-create-apply.md`.

## MCP and plugin

`propose_change {kind, target, base?, text, rationale, author_role, author_model?, run?}`: `kind` required, enum `["update", "create", "interpretation", "amendment"]` from core `*_KIND` (`ChangeKind`); `base` nullable, not required; `create` = `spec propose create TARGET [--base B] --text-file - --rationale R A --brief` (`propose_create_brief`); an `update` with `base` `null` or absent -> -32602 `base: ...`. `INSTRUCTIONS`: `mcp-read.md` "Texts".

Plugin (since 0.1.4), `propose-spec-change` "A new section or file": `kind: "create"`; a new file with `base: null`, its ID in `id:`; ID sections against the `span_hash`; the agent names each new ID. Taken by the spec or another's proposal: resend with the named next free ID. **Held by the agent's own earlier proposal** (a corrected create): never the next free ID, which would add the node twice; name that proposal, ask the owner to reject it, resend once `get_proposal` shows it rejected. Removing an ID section stays a question for the owner.

## Genre

Kind names from core constants only (`CREATE_KIND` beside `QUESTION_KIND`); no `[ids]` prefix, kind, `docs/` or `records/` literal in the create sources (`proposal_genre.rs`).

## Known limits

Accepted at shipping (2026-10-07); none blocks.

- An ID created outside the queue on another branch: `id-taken` at merge. A stale open create holds its IDs until rejected; a wrong guess costs one refused call.
- Apply's limits: `proposal-create-apply.md`.
- A generated directory or index outside the walk gets the walk's wording, not its own (exit 1 either way).
