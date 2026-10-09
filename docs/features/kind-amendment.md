---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
ref: kinds analysis 2026-10-08 (Q3, Q4, W10, slice C criteria; decided under the owner's pre-approval, review open); kind-interpretation, kind-decision "Out of scope"; 06 s7; 08 s2 Phase 2
---

# Kind amendment

## Why

An amendment is a separate document that `amends` a live node until absorbed (06 s7; slice 6 Q6). A file-form `create` takes one unchecked, the inbox calls it a create, nothing says when it stops pending. Slice C of three (W1). Decided (owner's pre-approval, review open): **Q3** an amendment touches only its own new file; **Q4** it is applied when its document goes to Tier 3 by status; **W10** it is a file-form create whose text declares >= 1 `amends` link to a live node, never writes its target, unapplied while its document is live.

**Cut**: (a) the kind end to end, here; (b) the "unapplied amendments" producer and its Health row: next slice `health-left`, as no served read lists links corpus-wide (`spec graph` walks from a REF, `spec inbox` reads the queue, the check reports defects, which a live amendment is not): (b) needs a new read, route and key set (07 s3 `health`, "to come").

## Description and interactions

`spec propose amendment PATH` raises a new file declaring `links: {amends: [ID, ...]}`; `spec approve` applies it as a file-form create (`proposal-kinds.md` "Apply"), one commit `A <path>`, every amended file untouched; the document then stays an unapplied amendment until moved to Tier 3 by status (W35). All else follows the shape "a new file", never the kind's name. MCP: `propose_change` `kind: "amendment"`; plugin `propose-spec-change`, 0.6.0; UI: the Health row's source text only (kinds are strings, ADR-0031).

## Data

```
spec propose amendment PATH --text-file F|- --rationale T [--author-role R] [--author-model M] [--run ID] [--task T] [--brief]
```

No `--base` (clap, exit 2); outputs, exits, `--json`, `--brief` as `propose create`'s. The text, whole (a git copy of spec-a, `PATH` `docs/features/regen-delay.md`):

```markdown
---
class: spec
status: draft
scope: [movement]
links:
  amends: [RULE-STAM-REGEN]
---

# Regeneration waits after a sprint

## Delay {#AC-01}

Regeneration starts 1.5 s after the last sprint, not after any stamina use.
```

**Names** (W26): core `AMENDMENT_KIND = "amendment"` beside `CREATE_KIND`, in `ChangeKind`; model `link.rs` `AMENDS = "amends"`, in `LINK_TYPES`; store `ProposalKind::Amendment`: `ALL` 7, `applies()` true, `decides()` false, `adds_nodes()` true (W34), `with_article()` `an amendment`; core `amended_refs(&ParsedFile)`: the document's front-matter `amends` values as written, in order.

**Row** (W28): no column or schema step (queue 6, 44 columns, `STATE_FORMAT` 2, backup `queue_schema` 6, `fixtures/daemon-keys.json` unchanged). A file-form create's columns (`proposal-kinds.md` "Queue row") with `kind` `amendment`: here `target_id` `docs/features/regen-delay.md`, `target_ids` `["docs/features/regen-delay.md", "regen-delay/AC-01"]`, never an amended ID; base NULL. Corrupt, named: a create's rules, plus a base (`base_hash`). The amended IDs are read from `new_text`, their one copy. Review document: a new file's 46 keys. Inbox `PR-0001 | amendment | open | docs/features/regen-delay.md | ...`.

**Consent** (W32), then the commit of a file-form create, name-status exactly `A <path>`, no trailer added (W31):

```
amends RULE-STAM-REGEN (docs/spec/movement/stamina.md)
apply PR-0001 to docs/features/regen-delay.md on main in /work/lantern-keep (new file)? [y/N]
```

**MCP** `propose_change {kind: "amendment", target: PATH, base: null, text, rationale, A, task_id?}` = `spec propose amendment PATH --text-file - --rationale R A [--task T] --brief`.

## Rules and edge cases

**Propose**, in order, the first refusal answering, exit 1 unless named, nothing stored, no ID taken:

1. **Form** (exit 2): clap, the author grammar, the place bound.
2. **Target**: create's file-form path rules ("Propose" 2); an indexed file -> `` `<p>` exists: an amendment is a new file ``; an ID -> `` `<X>`: an amendment is a new file: name its .md path ``.
3. **Text**: create's file-form rules, then (W29): no front-matter `amends` -> `` `<p>` declares no `amends` link: name the node it amends in `links: {amends: [ID]}`; a new file amending nothing is `spec propose create` ``; a `class:` other than `spec`, `decision`, or none -> `` `<p>` is `class: <c>`: an amendment is applied when its document goes to Tier 3 by status, which only a spec or a decision can ``; Tier 3 already -> `` `<p>` is archived (`status: <s>`): an amendment is written live ``.
4. **New IDs**: create's propose 4.
5. **Amended nodes** (W30), each in written order, over the recorded root's index refreshed now: Creation 1's forms (an alias, a legacy ID, `#SECTION`, `@rev`, a bare feature-scoped ID -> exit 1 naming the canonical form; a look-alike, mixed script, `project:` -> exit 2 with the Latin fix); then ``amends `X`: <why>; nothing stored``, `<why>` one of `no node X` (a reserved ID too), `X is archived (<path>, status: <s>)`, `X is generated (<path>)`, `X is defined by this text`, `X is named twice`. An `immutable_text` target passes: nothing writes it.
6. **Validation**: Creation 4 with the file added, findings never refusing (ADR-0012). 7. `--task`. 8. Insert, create's reservation (its new IDs only).

**Approve** `spec approve PR [--note T]`: create's file-form steps; step 4 checks each amended ID again as propose 5 -> exit 1 ``amends `X`: <why>``, one `apply_failed` (step 4), nothing written (review: preview `unavailable`, `not applicable now (step 4): ...`); step 6 runs propose 3 again on the stored text. `--option`, `--answer`, `--canon` -> exit 2, no event (`` ...; `PR` is an amendment: `spec approve PR` applies it as proposed ``). Completion by `Proposal:` alone; killed run, reject, orphans: create's.

**Elsewhere**:

- **Stage**: an approve, `span_hash` null; `option`, `answer`, `canon` a usage refusal (HTTP 400); one stored on an amendment row: corrupt, named.
- **Round**: never on the sheet; counted among open change proposals; its answer line -> ``is a change proposal (amendment): `spec approve PR` decides it``; `--staged` notes it unasked.
- **Task** (W33): `--task`, `task_id` bind it; `open_proposals` lists it bound, or when its `target_ids` or amended IDs meet the wanted nodes (`lists_proposal`); no assumption; applied, no `task.refreshed`.
- **Backup**: a byte-identical round trip. An older build names an `amendment` row corrupt, the inbox skips it with a note (ADR-0017): `spec`, daemon, `specengine-mcp` from one build.
- **MCP**: `kind` `["update", "create", "interpretation", "amendment"]` from core constants; with `amendment`, a `base` string -> -32602 `base: an amendment is a new file: null or absent`, `decision`, `how`, `why` -> -32602 `<field>: only an interpretation takes it`. `INSTRUCTIONS` and `CHANGE_DESCRIPTION` name it, each <= 2 048 B (`INSTRUCTIONS` with probes).
- **Create** (W29): a file-form text declaring `amends` -> exit 1 `` `<p>` declares `amends`: raise it as `spec propose amendment <p>`; nothing stored ``. The section form and `update` do not judge `amends`.

**Unapplied** (W10, W35): a document with `amended_refs` non-empty is an unapplied amendment while `check::is_live` (neither Tier 3 by status nor generated); settled once Tier 3. Absorbing it takes two updates, the target's text, then the amendment's status (a path target), never one commit (Q3); no command, note or link of its own; it never blocks (ADR-0012).

**Kind sites** (W34): `adds_nodes()`, exhaustive like `applies()`, true for `create` and `amendment`; `Proposal::new_file()` = `adds_nodes()` and no base. Each `==`/`!=` `ProposalKind::Create` (`queue.rs` `new_file`, the row's base and `new_ids` decoding, `target_ids` encoding, `new_proposal_problem`; `preflight.rs` step 4) is keyed on one of the two; `stage.rs`'s exhaustive match puts `Amendment` with the applying kinds; apply dispatches by `new_file()`. `ProposalKind::Amendment` appears only in the enum's `impl` and the CLI constructor.

**Genre**: `amends` only through `AMENDS`, `amendment` only through `AMENDMENT_KIND`; a project's own relation name (pilot B's) maps to `amends` in its importer, never the core (ADR-0008).

**Working answers**: W26-W35 where tagged; W27 file form only, the amended IDs only in the text, no flag; W31 the front-matter is the provenance; W36 plugin 0.6.0, MINOR (a newly named kind). Not taken: W28 a JSON column (schema 7, review 47 keys, a UI label); W29 any class (a canon amendment never applied).

## Risks

- **Hidden kind branches**: each `== ProposalKind::Create` left as is drops an amendment silently (the update apply path, a kept span hash, new IDs unreserved); AC-T2 pins them.
- **Caps**: `INSTRUCTIONS` 2 047 of 2 048 B with probes (the kinds list adds 10); `propose-spec-change` body 7 B under 4 096 (move plan text out); skill descriptions 30 B under 1 200; canon `proposal-queue.md` 3 B, `proposal-apply.md` 16 B, `proposal-kinds.md` 45 B under 12 288 (the split plan, items 1 and 4).
- **Cross-version**: no schema bump, yet an older build names an `amendment` row corrupt: reinstall `specengine-mcp`, restart the daemon.
- **Config drift**: an `[ids]` change since propose (a new `aliases_from`) makes step 4 refuse naming the canonical form.
- **Reach**: not in a class-less project (pilot B before import, W29), nor of an id-less document (`links:` values are IDs); this repository's `spec` contract lacks `links` (Open 2).

## Acceptance criteria

A git copy of `fixtures/spec-a` (`RULE-STAM-REGEN` in canon `stamina.md`, `DEC-0007` superseded, `R-12` `immutable_text`, `QST` aliasing `Q`, the live spec `stamina-tuning.md` with `AC-07`), the "Data" text unless named. Each mutation turns its criterion red.

rust-developer
- [ ] AC-R1 `ProposalKind::Amendment` as "Names"; `adds_nodes()` false for the other five; store `create` stores it without a base, with one `Invalid`; `create_intake` `Invalid`; a stored base: `get` fails naming `base_hash`. M: `adds_nodes()` false for it.
- [ ] AC-R2 Propose: exit 0, `PR-0001`, `introduced: 0`; the inbox line and JSON `target_ids` of "Data"; `reserved()` exactly `regen-delay/AC-01` by `PR-0001`, a create of it then refused as reserved; `git status --porcelain` empty. M (C-R2): the amended ID in `target_ids` (`reserved()` lists it).
- [ ] AC-R3 Exit 1, the queue empty: no `amends`; `class: canon`; no `class:`; `status: shipped`; `spec propose create` of the "Data" text names `spec propose amendment`. M: the class rule dropped.
- [ ] AC-R4 Nothing stored: `QST-031` exit 1 naming `Q-031`; `MEC-STAMINA#RULE-STAM-REGEN` naming `RULE-STAM-REGEN`; `AC-07` naming `stamina-tuning/AC-07`; a Cyrillic `A` in `RULE-STAM-REGEN` exit 2 with the fix; `DEC-0007` `is archived (<path>, status: superseded-by DEC-0023)`; `RULE-NOPE` `no node RULE-NOPE`; `regen-delay/AC-01` `is defined by this text`; a repeat `is named twice`. `R-12` stored. M: the live test dropped.
- [ ] AC-R5 Approve: the consent of "Data"; `y` -> `applied PR-0001 as <sha>`; `git show --name-status --format= <sha>` exactly `A docs/features/regen-delay.md`, one parent, its blob `new_text`, only an update's four trailers; `stamina.md` byte-identical; `reserved()` empty; `spec check`: no `ref-dangling` in the new file. M (C-R1): the apply writes the amended file too.
- [ ] AC-R6 Amend `stamina-tuning/AC-07`, then commit `status: shipped` there: review's preview `unavailable` naming step 4; approve exit 1, one `apply_failed` (step 4), `open`, nothing written. M: no re-check.
- [ ] AC-R7 `--option 0`, `--answer x`, `--canon y`: exit 2, no event, `` `PR-0001` is an amendment ``; a hand commit with `--trailer 'Proposal: PR-0001'` completes it; `spec reject` frees `regen-delay/AC-01`. M: completion needing another trailer.
- [ ] AC-R8 A staged approve (store, HTTP `POST .../decision`) holds `span_hash` null; `option`, `answer`, `canon` refused (HTTP 400); a stored `span_hash` named corrupt; `spec round new` notes `1 open change proposal(s) not on this sheet: PR-0001`; `PR-0001: ok` refused `is a change proposal (amendment)`. M: `new_file()` left `== ProposalKind::Create`.
- [ ] AC-R9 Bound (`--task T-0001`): in `open_proposals`, no assumption; unbound: listed by a task wanting `RULE-STAM-REGEN`, not by another; applied in the compared place: no `task.refreshed`. M: `lists_proposal` by `target_ids()` alone.
- [ ] AC-R10 Review: 46 keys, `diff` `@@ -0,0 +1,13 @@`, `preview` `applies`; `--brief`, `get_proposal` alike; `fixtures/daemon-keys.json` unchanged. M: a key added.
- [ ] AC-R11 MCP: the `kind` enum of four from constants; `amendment` with `base` null, and absent, byte-equal to the twin; a `base` string, a `how`: -32602 as "Elsewhere"; both texts within caps. M: a kind literal in `specengine-mcp/src`.
- [ ] AC-R12 Plugin: `propose-spec-change` names `kind: "amendment"`, `base` null, `links: {amends: [ID]}`, the target unwritten, applied once shipped, abandoned or superseded; body <= 4 096 B, descriptions <= 1 200 B; 0.6.0, a `PINS` row. M: the text changed, not the version.
- [ ] AC-R13 Backup: an applied and an open amendment export `"queue_schema":6`, restore byte-identical. M: export skipping the kind.

test-engineer
- [ ] AC-T1 Tests: store `queue_amendment.rs`, CLI `kind_amendment.rs`, MCP `mcp_amendment.rs`, additions where named; every mutation run red.
- [ ] AC-T2 Genre (`proposal_genre.rs`, `mcp_genre.rs`): no `"amendment"`, `"amends"` literal in `specengine-{store,cli,mcp}/src`; no `==`/`!=` with `ProposalKind::Create` or `::Amendment` there (the enum's `impl`, the CLI constructor aside). M: `|| proposal.kind == ProposalKind::Amendment` in `preflight.rs`.
- [ ] AC-T3 One door: raised over MCP, `git status --porcelain` empty (`mcp_door.rs`); http `door.rs` green. M: a file write in propose.
- [ ] AC-T4 http `daemon_keys.rs` serves an amendment's review and inbox entry with the fixture's key sets. M: a key renamed in Rust.

ui-developer
- [ ] AC-U1 harbor-sim `PR-0052`, an open amendment (new file, `base_hash` null): card diff, inbox kind, stage dialog approve or reject only, on both clients; no product file changed for it. M: `review.kind === "amendment"` (`policy.test.ts` red).
- [ ] AC-U2 Health "Unapplied amendments" stays "Not measured yet", its source the producer to come (a read of live `amends` links), not the kind; no digit (`HealthView.test.tsx`). M: the row showing `0`.

spec-writer
- [ ] AC-S1 This spec, compressed at shipping.
- [ ] AC-S2 Canon: `proposal-amendment.md` (new); `proposal-{kinds,interpretation,decision,queue}.md` (split where full); `spec-check.md` "Not checked yet" (a Health row, W10); `agent-intake`, `mcp-read`, `plugin{,-files}`, `decision-staging`, `tasks`, `task-package`, `queue-backup`; model, store, CLI, MCP READMEs; 05-08; `CLAUDE.md`; check clean, worst W <= 108 468. M: a canon page still saying `amendment`: a later slice.

## Out of scope

- **`health-left`**, next: the producer (`amended_refs`, `is_live` over the corpus), a read and `GET /api/projects/:p/health` with its key set, the Health row measured, never 0 unmeasured; may slip to Phase 4/5.
- Absorbing in one command or commit (Q3); a section-form amendment; an `--amends` flag; amending an id-less or Tier 3 document; a note on an update of an amended node; `--impact` following `amends`; pilot B's relation (its importer); states `changes_requested|deferred|superseded`; follow-up tasks; edit-and-accept; MCP staging forms.

## Open

1. The owner reviews Q3, Q4, W10, the cut ((b) to `health-left`), W26-W36.
2. This repository's closed `[classes] spec` lacks `links`: an amendment here is a `key-extra` error. Working answer: leave it (amendments here stay lines in a new spec).
3. The owner's manual check at shipping: reinstall `specengine-mcp`, restart the daemon, plugin 0.6.0; an amendment over MCP, `spec approve PR`: one added file; `?scenario=normal#/harbor-sim/inbox/PR-0052`.

## Implementation

Filled in after implementation.
