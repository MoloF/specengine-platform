---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
owner: owner
reviewed: 2026-10-14
---

# Proposal kind: amendment

Phase 2 (2026-10-09, 06 s7): a separate document amending live nodes, raised as a new spec or decision file whose front-matter declares `links: {amends: [ID, ...]}`. Approval adds only that file, `A <path>`, never an amended one; it stays an unapplied amendment while live and is applied once Tier 3 by status ("Unapplied"). Code: model `AMENDS` (in `LINK_TYPES`); core `AMENDMENT_KIND`, `create::amended_refs`, `check::front_matter_failure`; store `ProposalKind::Amendment`, `adds_nodes()`, `amends()`, `Reservation.kind`; CLI `amend.rs` (`propose_amendment[_brief]`, `text_problem`, `amended_nodes`, `amended_step`, `amended_ids`), `create.rs` (`approve_file`, `reserved_refusal`), `package.rs` (`Listing`); MCP `intake.rs` (`ChangeKind::Amendment`).

## Naming

`amendment` is the kind (`AMENDMENT_KIND`, `ChangeKind`), `amends` the link type of the model's shared table (`AMENDS`); neither is a literal in the store, CLI or MCP sources (`proposal_genre.rs`, `mcp_genre.rs`). A project's own relation name maps to `amends` in its importer, never in the core (ADR-0008). Prose: "an amendment" (`with_article()`).

## Propose

`spec propose amendment PATH --text-file F|- --rationale T [A] [--task T] [--brief]`; no `--base` (clap, exit 2); output, JSON, `--brief` as `propose create`'s (`proposal-kinds.md` "Forms"). In order; the first refusal answers, exit 1 unless named, each ending `; nothing stored`, no ID taken:

1. **Form** (exit 2): the author grammar, the place bound; the text read, `--rationale`'s cap.
2. **Target**: not a `.md` path -> `` `<X>`: an amendment is a new file: name its .md path ``; an indexed file -> `` `<p>` exists: an amendment is a new file ``; then create's file-form path rules (`proposal-kinds.md` "Propose" 2).
3. **Text**: create's file-form rules, then `text_problem`:
   - a front-matter that does not read: the parser's own finding (`front_matter_failure`: `not-utf8`, `frontmatter-unclosed`, `-yaml`, `-not-mapping`), `<p>:<line>: <code>: <message>; an amendment names the nodes it amends in its front-matter, which does not read`;
   - no `amends` -> `` `<p>` declares no `amends` link: name the node it amends in `links: {amends: [ID]}`; a new file amending nothing is `spec propose create` ``;
   - a class but `spec` or `decision` -> `` `<p>` is `class: <c>`: an amendment is applied when its document goes to Tier 3 by status, which only a spec or a decision can `` (none: `` declares no `class:` ``);
   - Tier 3 already: a spec -> `` `<p>` is archived (`status: <s>`): an amendment is written live ``; a decision -> `` `<p>` is `class: decision` with `status: <s>`: a decision document is live only while `accepted`, and an amendment is written live `` (no `status:` passes).
4. **New IDs**: create's propose 4; one held -> `` `X` is reserved by `PR-0001` (open), a live create `` (or `amendment`: the holder's kind).
5. **Amended nodes** (`amended_nodes`), each `amends` value in written order over the index this call refreshed: Creation 1's refusals prefixed `amends: ` (`` amends: `QST-031` is a legacy alias: a proposal names the canonical ID `Q-031` ``; `#SECTION`, `@rev`, `[[...]]` exit 1; a look-alike, mixed script, `project:` exit 2 with the Latin fix); then `` amends `X`: <why> ``: `no node X` (an ID only reserved is none), `X is named twice`, `X is defined by this text`, `` an amendment names the canonical ID `slug/X` `` (a bare feature-scoped ID), `X is generated (<path>)`, `X is archived (<path>, status: <s>)`. An `immutable_text` holder passes: nothing writes it.
6. **Validation**: Creation 4 with the file added; findings never refuse (ADR-0012). 7. `--task` (`tasks.md`). 8. **Insert**: create's reservation, its own new IDs only.

## Row

No column or schema step of its own (44 columns, `STATE_FORMAT` 2, `fixtures/daemon-keys.json` unchanged by it). `ProposalKind`: `ALL` 7; `Amendment` `applies()` true, `decides()` false; `adds_nodes()` (create, amendment: new IDs reserved while live, `target_ids` the target and new IDs) and `amends()` (amendment: a new file only, its text naming live nodes), each an exhaustive match; `Proposal::new_file()` = `adds_nodes()` and no base. Store `create` stores it; a base -> `Invalid` `an amendment is a new file: it is written against no base`; `create_intake` -> `Invalid`. The columns are a file-form create's (`proposal-kinds.md` "Queue row"), `target_ids` never an amended ID; corrupt, named: a create's rules, plus a base column (`an amendment is a new file: it holds no base`). The amended IDs live only in `new_text`, read when needed under the reading root's scheme, as written (`amended_ids`). `reserved()` -> `Reservation {id, by, kind, status}`, `QueueError::Reserved {id, by, kind}`: refusals say `a live create` or `a live amendment`.

Review document: a new file's (`kind` `amendment`, base `null`, `diff` `@@ -0,0 +1,<n> @@`, `preview`). Inbox: `PR-0001 | amendment | open | docs/features/regen-delay.md | ...`.

## Approve

`spec approve PR [--note T]`: a new file's steps (`proposal-create-apply.md`), plus:

- **4** the amended nodes again, as propose 5, over the recorded root's refreshed index (`amended_step`) -> exit 1 `` amends `X`: <why>; nothing changed ``, one `apply_failed`; review's preview `unavailable` naming step 4.
- **6** `text_problem` again on the stored text (a config change since, a front-matter the parser now fails) -> exit 1 at step 6.
- **1** consent: a line per amended node, as step 4 found it, then the question:

```
amends RULE-STAM-REGEN (docs/spec/movement/stamina.md)
apply PR-0001 to docs/features/regen-delay.md on main in /work/lantern-keep (new file)? [y/N]
```

- **10** one commit, name-status exactly `A <path>`, an update's four trailers, none added; completion by `Proposal:` alone; killed run, reject (frees its new IDs), orphans: a create's.

`--option`, `--answer`, `--canon` -> exit 2, no event: `` ...; `PR` is an amendment: `spec approve PR` applies it as proposed; nothing changed ``.

## Elsewhere

- **Create**: a file-form text declaring `amends` -> exit 1 `` `<p>` declares `amends`: raise it as `spec propose amendment <p>`; nothing stored ``; a front-matter that does not read is no create refusal (stored with its finding, ADR-0012). The section form and `update` do not judge `amends`.
- **Stage** (`decision-staging.md`): an approve, `span_hash` null; `option`, `answer`, `canon` a usage refusal (HTTP 400); one stored on its row: corrupt, named.
- **Round**: never on the sheet; among `open change proposal(s)`; its answer line -> ``is a change proposal (amendment): `spec approve PR` decides it``; `--staged` notes it unasked.
- **Task**: `--task`, `task_id` bind it; `open_proposals` and the round's waiting tasks list it bound, or when its `target_ids` or the IDs it amends meet the wanted nodes (`lists_proposal`; `Listing` reads a new file's text once, after the cheaper tests); no assumption; applied, no `task.refreshed` (a new file).
- **Backup**: byte-identical round trip. An older build names an `amendment` row corrupt by its kind, the inbox skipping it (ADR-0017): `spec`, the daemon, `specengine-mcp` from one build.
- **MCP** `propose_change`: `kind` `["update", "create", "interpretation", "amendment"]` from core constants; `amendment` = `spec propose amendment TARGET --text-file - --rationale R A [--task T] --brief` (`propose_amendment_brief`); a `base` string -> -32602 `base: an amendment is a new file: null or absent`; `decision`, `how`, `why` -> -32602 `<field>: only an interpretation takes it`. `CHANGE_DESCRIPTION` 2 029 B; `INSTRUCTIONS`: `mcp-read.md` "Texts".
- **Plugin** 0.6.0 (`plugin.md`): `propose-spec-change`'s amendment bullet.
- **UI**: kinds are strings (ADR-0031), no product branch; Health's "Unapplied amendments" is measured by `spec health` (`docs/canon/health.md` "UI"). Mock: harbor-sim `PR-0052`.

## Unapplied

A document whose front-matter declares >= 1 `amends` (`amended_refs` non-empty) is an **unapplied amendment** while live (`check::is_live`: neither Tier 3 by status nor generated), whatever raised it; applied, or abandoned, once Tier 3 by status: a spec `shipped` or `abandoned`, a decision no longer `accepted`. Absorbing one takes two updates, the amended text, then the amendment's status (a path target), never one commit; no command, note or link of its own; it never blocks (ADR-0012). `spec health` lists them (core `check::unapplied_amendments`), served as `GET /api/projects/:p/health` and Health's row 3, never 0 unmeasured: `docs/canon/health.md`. The check reports none (`spec-check.md` "Not checked yet").

## Known limits

Accepted at shipping (2026-10-09); none blocks.

- A mistyped `links:` the parser drops with `frontmatter-type` (a scalar `amends:`, a non-string item) reads as `declares no amends link`, the finding unnamed.
- The consent names an amended node's path from the project root, the target's from the worktree top.
- An `[ids]` change since propose (a new `aliases_from`) makes step 4 refuse naming the canonical form: reject, raise again. An id-less document cannot be amended (`links:` values are IDs); a class-less project cannot raise one.
- This repository's closed `[classes] spec` lacks `links`: an amendment here is a `key-extra` error until the owner admits it.
- `propose-spec-change` says "each ID as the spec writes it" (a feature-scoped one is `slug/ID`); the UI mock's staged option on a change kind words its refusal unlike the daemon.
- Untested: an older build reading an amendment row; a killed run or orphan of one; MCP `task_id` on one.
