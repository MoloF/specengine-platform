---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: proposal-kinds analysis 2026-10-06, every recommendation accepted; 08 s2 Phase 2; this slice builds `create` only
---

# Proposal kinds: create

## Why

No node, not even a first tree, comes into existence through the queue (08 s4.1: "built through the queue", IDs "agent proposes, owner confirms"). `propose update` refuses an added `{#ID}` or `id:` (`proposal-queue.md` "Creation" 3); the `propose-spec-change` skill sends the agent to the owner. This slice adds the kind `create`: a new spec file, or new `{#ID}` sections in a node's span, applied by `spec approve` as one commit where raised. **Order** (accepted 2026-10-06): `daemon-read`, this, `task-package` (schema 4, `--task` on `create`, plugin 0.1.5), `ui-live`; AC-14's schema 3 and 40 columns hold only before `task-package`. No new ADR: it rests on ADR-0004, 0005, 0032, 0012, 0008, 0031, 0009, 0026, 0003, 0013, 0017.

Working answers (the orchestrator's, accepted under the owner's night rule 2026-10-06; morning review: "Open"): Q1 the proposer names each new ID, reserved while live (Open); Q2 one kind for files and sections, `update` keeps its node set; Q3 `kind` on `propose_change`, no new tool; Q4 no `[decision_records]` ID by create; Q5 no schema bump; Q6 an amendment is a file by create (Open); Q7 a decision and its canon diff are two commits, the record then a linked update; Q8 (readiness check) the probe paragraph cut (Data), the order above.

## Description and interactions

Later kinds: `interpretation`, `amendment` (Q6), `decision` (05 s3.3; 06 s5, s7); `spec new` = create + instant apply. **File form**: `TARGET` a free `.md` path, no `--base`, the text the whole file, applied by decision-apply's new-file steps (`docs/canon/decision-record.md` "Steps"), commit `A <path>`. **Section form**: `TARGET` a node, `--base` its `span_hash`, the span's text adding `{#ID}` sections below the target, applied by the update's steps (`docs/canon/proposal-apply.md` "Apply steps"), commit `M <path>`.

## Data

**CLI** (`A` the author flags; caps, outputs as `propose update`'s, `docs/canon/proposal-queue.md` "Commands"); fixture `spec-a`, `r13.md` = `R-12.md` with `id: R-13` and its own H1:

```
spec propose create TARGET [--base HASH] --text-file F|- --rationale T [A] [--brief]
$ spec propose create docs/records/R/R-13.md --text-file r13.md --rationale "sprint needs it" --author-role writer
PR-0001
introduced: <n>
```

**MCP** `propose_change {kind, target, base?, text, rationale, author_role, author_model?, run?}`: `kind` enum `["update", "create"]` from core constants; `create` = `spec propose create TARGET [--base B] --text-file - --rationale R A --brief`; `base` absent or `null` = no `--base` (an `update`: exit 2, as the CLI). The description gains `create`; `mirror.rs` no new key.

**Instructions budget** (measured 2026-10-06: `INSTRUCTIONS` 1 675 B + `PROBE_INSTRUCTIONS` 366 = 2 041 of `server.rs`'s `TEXT_LIMIT` 2 048, asserted at compile time under `probes`, which the longer line alone breaks): the line becomes (146 B with its LF) `- propose_change = spec propose update|create: a node's new text against its span_hash; create: new ID sections in it, or a new file (base null).`; `PROBE_INSTRUCTIONS` a blank line and `Probes build, owner's checklist only: review_proposal(proposal_id) asks the owner by form; probe_output(tokens) returns filler; probe_sleep(seconds) waits.` (157 B). Then 1 740 B, 1 897 with `probes`: 151 left, 138 for `task-package`'s line. `mcp_decision.rs`, `mcp_path.rs` re-pin length and BLAKE3.

**Queue row**, `kind` `create`: schema 3, 40 columns, no column added, no `user_version` bump (Q5; schema 4 stays `task-package`'s).

| Column | File form | Section form |
|---|---|---|
| `target_id`, `target_path` | the text's `id:` canonical, else the path; the path | the target canonical (Creation 1); its holder |
| `target_ids` | `[target_id, other new IDs in text order]` | `[target_id, new IDs in text order]` |
| `base_hash`, `base_text` | `NULL` | as an update's |
| `new_text` | the file, byte for byte | the span's new text |
| `patch_hash` | `b3_hash(target_id LF LF new_text)` | as an update's |
| place, `rationale`, `author`, `diagnostics` | as an update's | as an update's |
| ten other intake columns, five record columns | `NULL` | `NULL` |

**New IDs of a row**: `target_ids` but its first, plus the first when `base_hash` is `NULL` and it is an ID; canonical as `spec show` names them (`slug/ID` if feature-scoped, ADR-0026). Corrupt, named: `target_ids` `NULL` or no JSON list of canonical IDs (first `target_id`); the base set partly; `new_text`, `rationale` `NULL`; another intake or a record column set. An older build reads the row as corrupt, `kind` (ADR-0017).

**Store**: `ProposalKind::Create` (core `CREATE_KIND`; `applies()` true, `decides()` false); `NewProposal` carries the new IDs and an optional base (none only for a create); `create` checks **inside its `Immediate` transaction** that no live (`open`, `approved`) create of the project, any repository, holds a new ID, else `QueueError::Reserved {id, by}`, nothing written; read-only `reserved()`: live creates' new IDs.

**Review document**: an update's keys, `target_ids` stored; file form `base_hash`, `base_text` `null`, `diff` against an empty base (`--- base <path>`, `+++ proposed <path>`, `@@ -0,0 +1,<n> @@`); `preview` `applies|rebases|conflicts|unavailable`. **Inbox** `<id> | create | <status> | <target_id> | ...`. **Backup**, events unchanged. **Commit**: an update's, name-status exactly `A <path>` or `M <path>`.

## Rules and edge cases

**Propose**, in order; the first refusal answers, nothing stored or reserved:

1. Form (exit 2): clap, the author grammar.
2. **Target** resolving as an update's (Creation 1) -> section form: no `--base` -> exit 1 `` `<target>` exists: a create never replaces a file; to add ID sections to it, name its span_hash with --base ``; Creation 1's refusals (generated, `immutable_text` prefix or document; alias, `#SECTION`, `@rev`) and 2 hold. A `.md` path naming nothing -> file form: `--base` -> exit 1 ``nothing at `<path>`: a new file is written against no base; drop --base``; the path clean, `.md`, in the walk (core `Paths::in_walk_scope`), not under `[paths] generated`, no symlink or non-directory on its way, nothing there (a dangling symlink too) nor in git's index, else exit 1 (`exists` as above).
3. **Text**: UTF-8, at most `TEXT_MAX_BYTES`, verbatim, parsed under the recorded root's scheme. File form: `class: generated` -> exit 1. Section form: spliced as Creation 3, every (ID, level) pair of the file kept in order; exit 1: an ID dropped, an `id:` added, a level changed, a heading at the target's level or above, no new ID (``no new ID: `spec propose update` replaces a span``). **`update`'s rule is unchanged.**
4. **New IDs**, each in text order: an `aliases_from` prefix (recognised verbatim first) -> exit 1 naming the canonical (`QST-033`: `Q-033`); a look-alike or mixed script -> exit 2 naming the Latin form (core's `fix`); a number not as core `record_id` writes it -> exit 1 naming that form (`R-7`, `R-007`: `R-07`; past `width` passes, `id-width` a finding); the `[decision_records]` prefix -> exit 1 `` `DEC-0024`: these records are made by `spec approve` of a `question` or `discrepancy`; nothing stored ``; twice in the text -> exit 1; defined or in an `aliases:` in the recorded root's refreshed index -> exit 1 naming the holder (path, `id:`) and, number shape, the next free ID; a live create's (`reserved()`) -> exit 1 naming it and the next free ID. Canonical forms compared. An `immutable_text` prefix (spec-a's `R`) is allowed: creation is how one comes to be. **Next free**: 1 + the greatest number of the prefix or its `aliases_from` in the index (defined or aliased) and among live creates, by `record_id`; gaps may be named.
5. Validation: Creation 4 with the file added or swapped, against `HEAD`: introduced findings stored, never refusing.
6. Binding: Creation 5. 7. Insert: `Reserved` (a race) -> step 4's last refusal.

Only steps 1-4, 6, 7 refuse; whatever `spec check` judges (`ref-dangling`, `file-name`, `id-scope`) is a finding (ADR-0012).

**Apply** (`spec approve PR [--note T]`): an update's flow, exits, trailer lookup, consent, runs, refusals logged and reopened by step; `--option`, `--answer`, `--canon` -> exit 2, no event.

- File form: `decision-record.md` "Steps", `new_text` the record, no ID issued: **1** `apply PR-0001 to <path> on <branch> in <worktree> (new file)? [y/N]`; **3** the path in the walk, not generated (exit 1); **4** propose 2's path checks (`exists`, "Killed run"'s hint), the new IDs free in the refreshed index; **5-6** propose 3-4 on `new_text` (not the queue check), the identity; **7** `approve_from`; **8** `create_file`; **9** intent-to-add, `commit_only`, a failure removing entry, file, directories (``the commit of `<path>` failed, the file removed: <git's error>``); **10** exactly `A <path>`, blob `new_text`.
- Section form: an update's steps; 4 also re-checks the new IDs free; 6 is propose 3's section rule on the merged file, adding exactly the stored new IDs (else exit 1).

**Completion**: the trailer, as an update's; file form: one parent, exactly `A <target_path>`, its blob `new_text` byte for byte, else why (`changes M a, A b`, `does not carry the proposal's text`). **Killed run** (file form, between 8 and 9): `decision-record.md` "Killed run"'s refusal and two ways out. **Reject**, **orphans**: an update's; a rejected create frees its IDs.

**Genre**: kind names only from core constants (`CREATE_KIND` beside `QUESTION_KIND`; `decision` is a spec-a node kind); no `[ids]` prefix, kind, `docs/`, `records/` literal in new sources; the proposer names the path (ADR-0008).

**Known limits**: one ID created on two branches -> `id-taken` at merge; a stale open create holds its IDs until rejected; a wrong number guess costs one refused call.

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a`, `-b`, committed; scratch `HOME`; clock `2026-10-06T12:00:00Z`; a git identity; library calls, consent yes; T13 = "Data"'s `r13.md`. Refused: the exit named, nothing stored, written or reserved. M: the mutation turning it red. Tests: CLI `proposal_kinds.rs` unless named.

- [ ] AC-01 -- spec-a `propose create docs/records/R/R-13.md` T13: `PR-0001`; review: `kind` `create`, `target_id` `R-13`, `target_path` the path, `target_ids` `["R-13"]`, `base_hash`, `base_text` `null`, `new_text` T13; `git status --porcelain` empty (M: writing at propose).
- [ ] AC-02 -- a file modified, another staged; `approve`: one commit, one parent, exactly `A docs/records/R/R-13.md`, blob T13, `spec: apply PR-0001`, rationale, four trailers; both files as before; `spec show R-13` resolves (M: staging the whole tree).
- [ ] AC-03 -- propose exit 1 `exists`: `docs/records/R/R-12.md` without `--base`, a dangling symlink `R-14.md`, an index-only `R-15.md`; after AC-01, X at the path: approve exit 1, `apply_failed` step 4, X intact, no commit, `open`; X by the consent callback: exit 1 at step 8, X intact (M: step 4 not re-checked; rename over the path).
- [ ] AC-04 -- new files, exit 1: `id: R-12` naming its file and `R-13`; `TERM-tired` naming `TERM-exhausted`; `QST-033` naming `Q-033`; `R-7` naming `R-07`; a `{#RULE-STAM-REGEN}` section naming `docs/spec/movement/stamina.md`; an ID twice; `\u0420-13` (U+0420 for `R`) exit 2 naming `R-13` (M: `id:` checked, `{#...}` not).
- [ ] AC-05 -- `PR-0001` open, then `approved`: `id: R-13` at `R-13-b.md` exit 1 naming `PR-0001`, `R-14`; after `reject PR-0001` it stores; store: a second handle's `create` of `R-13` -> `Reserved` (store `queue_kinds.rs`; M: the in-transaction check dropped).
- [ ] AC-06 -- section form on `RULE-STAM-REGEN`, its span + `### Rest delay {#EDGE-STAM-REST}`: `target_ids` `["RULE-STAM-REGEN","EDGE-STAM-REST"]`; approve: exactly `M docs/spec/movement/stamina.md`, `EDGE-STAM-REST`'s parent `RULE-STAM-REGEN`. Exit 1: that heading at `##`; no new ID; on `MEC-STAMINA`, `{#EDGE-STAM-ZERO}` dropped or `## Regeneration` made `###`; AC-06's text by `propose update` (M: update's rule relaxed).
- [ ] AC-07 -- after AC-06's propose, a commit `Base rate 10` -> `12`: `preview` `rebases`, both changes written; instead a line added after the `Exhausted` bullet: `conflicts`, approve exit 1, the conflict on stdout, file untouched, `apply_failed` step 5 (M: `new_text` over the current span).
- [ ] AC-08 -- T13 + `links: {derived_from: [R-99]}`: `introduced: 1`, `ref-dangling` naming `R-99`, stored; approve applies it (M: findings refusing).
- [ ] AC-09 -- exit 1: T13 `class: generated`; `docs/generated/R-13.md`, `templates/R-13.md`, `docs/records/R/R-13.txt`, `../R-13.md`; on `R-12` (`immutable_text`) adding `## Note {#RULE-R12-NOTE}`; `id: DEC-0024` naming `spec approve` (M: the `[decision_records]` check dropped).
- [ ] AC-10 -- MCP `propose_change {kind: "create", target: "docs/records/R/R-13.md", base: null, ...}`: `structuredContent` = `propose create ... --brief --json`, `content` its text; `base` absent alike; `{kind: "update", base: null}` an error naming `base`; `create` on `RULE-STAM-REGEN`, `base: null`: AC-03's refusal; schema `kind` `["update","create"]`, `base` nullable; `git status --porcelain` empty after each call (MCP `mcp_create.rs`; M: `base` required).
- [ ] AC-11 -- spec-b, the same functions: `id: REQ-003` stores and applies `A`; the Cyrillic alias prefix + `-003` exit 1 naming `REQ-003`; `GLS-merge` stores; `GLS-worktree` exit 1 naming its file; core `genre.rs`, CLI `proposal_genre.rs` (new sources listed), MCP `mcp_genre.rs` green (M: a `"create"` or prefix literal outside core).
- [ ] AC-12 -- file form `approved` at step 7, committed by hand (`new_text`, `Proposal: PR-0001`): approve -> `applied`, no commit or prompt; other bytes -> exit 1 `does not carry the proposal's text`; reject refused with that commit on the branch; killed between 8 and 9: approve exit 1 at step 4 naming two ways out (M: completion ignoring the blob).
- [ ] AC-13 -- `approve PR-0001` with `--option 0`, `--answer a`, `--canon R-12`: exit 2, no event; `propose create docs/records/R/R-13.md --base b3:<any>` exit 1 `drop --base`; `propose create RULE-STAM-REGEN` without `--base` exit 1 (M: a decision flag accepted).
- [ ] AC-14 -- `review PR-0001`: `diff` `--- base docs/records/R/R-13.md`, `@@ -0,0 +1,<n> @@`, all `+`; `preview` `applies`, a file there -> `unavailable` (step 4); `inbox` `PR-0001 | create | open | R-13 | ...`; `export state`, `import-state` fresh, re-export byte-identical, `queue_schema` 3, 40 columns, `user_version` 3 (M: a column or a schema bump).
- [ ] AC-15 -- `propose-spec-change` teaches `kind: "create"` (a new file, `base` null; ID sections with the span's hash; the named next free ID); its "an ID section to add or remove is a question for the owner" now removal only; `plugin.json` `0.1.4` (`task-package` 0.1.5), `PINS` appended; `plugin_skills.rs`, `plugin_files.rs` green (M: the skill edited, no bump).
- [ ] AC-16 -- no `crates/*/src` comment cites `decision-apply`; `cargo nextest run -p specengine-eval --test anonymity --test doc_pointers` green; `export index && check` clean, worst W not above shipping's; the full run green (M: a heading `decision-record.md` lacks).
- [ ] AC-17 -- instructions as "Data" (1 897 <= 2 048 - 140); the `probes` build compiles; `mcp_decision.rs`, `mcp_path.rs` pin 1 740 (M: the probe paragraph left at 366 B: the `probes` build fails).

## Out of scope

Kinds `interpretation`, `amendment`, `decision` (later slices, that order); `spec new`, `spec edit`; engine-issued numbers; deleting, renaming, re-targeting nodes; `has_open_proposal`; `task_id`, `--task` (`task-package`, after this slice); `[decision_records]` IDs (Q4); the UI; pilots; `.claude/`.

## Open

The owner's morning review; working answers stand until then.

- **Q1** proposer-named IDs, reserved while live: the agent cites it before review, a wrong guess costs one call; else issued at approve as decision records, a placeholder rewritten.
- **Q6** amendment = a document with `amends: <target>` by create's file form, unapplied until the target is revised (06 s7); else a row beside its target applied as an update. For the `amendment` slice.

## Implementation

Not built.

**Pointers** (developer, this slice): `crates/*/src` comments citing task spec `decision-apply` cite `` `docs/canon/decision-record.md` "<Heading>" `` instead, on one line: "Config" core `project_toml.rs:13`; "Template", "ID" core `record.rs:1`; "ID", "Steps" store `lib.rs:57`, `worktree.rs:6`; "Flags", "Steps" CLI `decide.rs:2`, `lib.rs:16`, `apply.rs:48`; "Flags" `apply.rs:95`; "Completion" `preflight.rs:46`; "Queue and documents" store `queue.rs:15`, `:146`, `queue/state.rs:26`, CLI `proposals.rs:27`, `inbox.rs:8`, `intake.rs:164`, `state_file.rs:10`. Tests' citations still resolve.

**At shipping**: new Tier 2 canon `docs/canon/proposal-kinds.md` (queue canons are full), "Data" and "Rules" as built; byte-neutral pointers: `proposal-queue.md` "Not yet", "Creation" 3; `agent-intake.md` "Schemas"; `mcp-read.md` "Tools" (1 740, 1 897 B); 05 s3.3; 07 s1.2, s2; 08 s2 Phase 2; core, store, CLI, MCP READMEs; root `README.md` `propose-spec-change` row, "Version" 0.1.4; `CLAUDE.md` state.
