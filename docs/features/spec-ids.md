---
class: spec
status: shipped
shipped: 2026-11-04
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, scripts/deploy]
ref: RM-0073; Phase 7, the first item of release `v3` (08 s2 Phase 7, "Release scope"); S1-S7, D1-D12 and the iteration 2 changes are the orchestrator's working answers, awaiting the owner's review
adrs: [ADR-0070]
---

# spec-ids: a hub mints spec IDs from 100001, a terminal below it (RM-0073)

## Why

A hub and a terminal minted spec IDs from the same numbers: records from the root's index and the queue's issued `record_id` (`docs/canon/decision-record.md` "ID"), created node IDs named by the proposer against a "next free" hint (`docs/canon/proposal-kinds.md` "Reservation"). Once `hub-git` (RM-0051) pushes and `hub-mcp` (RM-0053) lets a hub raise creates and decisions, both can write `DEC-0024` or `STY-0002`:
- at one path, an add/add conflict, and the sync is `held`;
- at two paths, a clean merge, then `id-taken`, red under `enforce`;
- a person renumbers, and queue rows and `Interprets:` trailers keep the old ID.

`apply-outputs` (RM-0072) gave a hub its own `PR-` range from a stored floor (ADR-0063) and left spec IDs a known limit. This slice extends that range to spec IDs, so a hub's and a terminal's never meet. It refuses no apply (ADR-0012) and changes no format.

Decided by the orchestrator, each awaiting the owner (now: `docs/canon/spec-ids.md`):
- **S1** The cure (A): a hub's own range from 100001, as `PR-` has. Reading the tracking ref before a mint (B) and a renumbering proposal (C) wait for `v4`.
- **S2** One core constant, equal to `HUB_PROPOSAL_FLOOR`; the data directory's `proposal-floor` file decides the side, so a host terminal under the hub's `HOME` is on the hub's side.
- **S3** `id-width` exempts only numbers from 100001. **S4** A created ID on the wrong side is refused at propose only; apply steps 4 and 6 are unchanged. **S5** Feature-scoped IDs and plan items are in the range.
- **S6** Every `spec` that writes to the repository upgrades before the hub's first spec-ID mint (RM-0074 "Upgrade"). **S7** Two terminals colliding is a known limit of `v3`.
- **D1** `HUB_ID_START` in core `record.rs`; `PR-` readers keep the stored number. **D2** The side: core `IdSide`, store `id_side(dir)`; `Hub` when the file reads as a floor, whatever its number, `Terminal` with none, an invalid one `read_floor`'s error (exit 2, nothing stored); read on each mint, by the CLI and again by the store under the write lock. **D3** Records: both maxima by side. **D4** Created IDs: `NewIds.side`, `next_free` by side, `NewIds::off_side` after `checked`, never inside it, so no apply meets it. **D5** The refusal texts. **D6** `id-width`: `record_id`'s form from 100001, on either side. **D7** A terminal's last number is 100000. **D8** The MCP texts, asserted at compile time. **D9** Plugin 0.29.1, a PATCH; the release tag `v0.29.1`. **D10** The canon home, a new page. **D11** One ADR, ADR-0070; ADR-0063 stays `accepted` for `PR-`; `architecture.md` unchanged. **D12** `import-layout-verifier.md` unchanged.
- **Review iteration 2:** the store refuses an off-side corpus maximum (the floor came or went during the run); an overflowing all-digit body counts as a hub's.

## Description and interactions

How it works now: `docs/canon/spec-ids.md`. Outputs, tools, schemas, formats and exits are unchanged, but a create's `--dry-run` on an invalid `proposal-floor` (exit 2).

## Data

The canon page's sections.

### The constant and the side

"The range" (D2).

### Records

"Records" (I3).

### Created IDs: the propose step

"Created IDs".

### Refusal texts

"Texts".

### `id-width`

"`id-width`" (AC-6).

### The MCP texts

"Texts": stdio 2 044 B, hub 2 040 B.

### The plugin

0.29.0 -> 0.29.1, PATCH (`plugin-files.md`).

## Rules and edge cases

**I1** a hub mints number-shape spec IDs from `HUB_ID_START`, any other data directory below; **I2** the floor file alone decides the side; **I3** both maxima of a record mint count only the side's; **I4** propose refuses a new ID off the side, exit 1, nothing stored, the dry run alike; **I5** apply, completion, reject and `confirm` judge no side; **I6** `id-width` passes only `record_id`'s form from 100001; **I7** no format changes; **I8** no prefix, kind or fixture word in the sources, `100001` from `HUB_ID_START`. Edge cases: a taken ID reports as taken first; of mixed sides, the first new ID in text order is named; the rest: the canon's "Known limits".

## Acceptance criteria

Scratch copies of the fixtures, no new fixture; a hub's side is a data directory where store `store_floor` wrote the file. M: the mutations that must turn each red, M1-M16. Mutations: 20 named, all killed: M1-M16 (M14 and M16 in two forms each) and two for iteration 2; M1, M2, M3 and M9 also red at their named http criterion; M13 fails the build at the compile-time `holds_number` assert. Gate: the tester's last run passed 5 138 of 5 138.

- [x] **AC-1 [default, http] Records through the real merge** (`fixtures/apply-outputs/forge`, `DEC` width 4, bare remotes as `hub_git.rs`). A developer's clone pushes `DEC-0001`; the hub then commits `DEC-100001`; the sync ends `ok` with a merge commit holding both, `spec check --json` reporting no `id-taken` and no `id-width`; the developer's next is `DEC-0002`. M1: the hub's `next_number` without the range. M2: the terminal's corpus maximum counts numbers from 100001.
- [x] **AC-2 [default, http] Creates through the real merge** (`fixtures/card-press`, `STY` width 4). The hub's `/mcp` create of `STY-0002` is refused naming `STY-100001`; resent so, stored and confirmed; a developer's `STY-0002` merges cleanly, no `id-taken`. M3: the propose step removed.
- [x] **AC-3 [default, core, store, cli] The mint by side.**
  - (a) Core `highest_record_number` over `DEC-0024`, `DEC-100003`, `aliases: [QST-100002]`: `DEC` 24 `Terminal`, 100003 `Hub`; `Q` 0 and 100002. `IdSide::next`: `Hub` 0 -> 100001, `Terminal` 99999 -> 100000, 100000 -> `None`.
  - (b) Store, a floor: `DEC-100001` with `corpus_max` 0 and a restored `DEC-0030`; `DEC-100008` after `DEC-100007`; `R` width 2 gives `R-100001`.
  - (c) Store, no floor: `DEC-0025` with `corpus_max` 24 and a restored `DEC-100004`; `corpus_max` 100000: the error of "Records", nothing written.
  - (d) CLI on spec-a, a floor: a question's approve writes `DEC-100001`, with `DEC-100003` in the tree `DEC-100004`; no floor and `DEC-100003` in the tree: `DEC-0024`.
  - (e) An invalid `proposal-floor`: the preview exits 2 with the floor's text, no record and no row changed; the run logs its one `proposal.apply_failed`, as every refusal at steps 2-10.
  - M4: the queue's issued IDs not filtered by side. M5: the side read from the daemon's flag.
- [x] **AC-4 [default, cli] Propose** (spec-a, card-press).
  - (a) No floor: a file-form create of `id: STY-100002` exits 1 with the terminal text naming `STY-0002`; queue empty, event count unchanged.
  - (b) A floor: a create of `STY-0002` exits 1 naming `STY-100001`.
  - (c) With `STY-0001` and `STY-100001` in the tree, the taken, reserved and insert-race (`proposal_kinds.rs` n2's seam) refusals name `STY-0002` on a terminal, `STY-100002` on a hub.
  - (d) The section form (`--base`) and `spec propose amendment` refuse alike.
  - (e) A floor: `stamina-tuning.md` gaining `{#AC-100001}` is stored, `target_ids` holding `stamina-tuning/AC-100001`; no floor: refused naming `stamina-tuning/AC-08`.
  - (f) `--dry-run` of (a) and (b): the same exit and stderr, nothing stored.
  - M6: the step removed. M7: `next_free` ignores the side. M8: the dry run skips the step.
- [x] **AC-5 [default, http, cli] Never refuse an apply** (ADR-0012). (a) A terminal's dump with an open create of `STY-0002` and an open amendment defining `STY-0003`, restored on a floor: each hub confirm answers 200, `applied`. (b) A hub's open create of `STY-100001`, restored with no floor, applies by `spec approve`. M9: the side judged inside `NewIds::checked`.
- [x] **AC-6 [default, core] `id-width`.** `check::run` over the "`id-width`" examples gives exactly those findings. M10: the rule unchanged. M11: every over-wide number passes. M15: the value alone exempts (`R-0100001`). M16: `>` for `>=`, or a lower start.
- [x] **AC-7 [default] Formats.** The pins of `QUEUE_SCHEMA_VERSION` 9, `STATE_FORMAT` 3, `PROPOSAL_COLUMNS` 46 and `INDEX_FORMAT` 17 green and unedited; core `scheme_toml.rs`, `project_toml.rs` unchanged. M12: a column added.
- [x] **AC-8 [build, mcp, plugin] The texts.** (a) Both descriptions hold "Texts"' sentences, ASCII, 2 040 and 2 044 B, asserted; `tools/list` over stdio and a hub's `/mcp` shows them. (b) The skill's five edits, body at most 4 092 B, no digit; `plugin_skills.rs`, `plugin_files.rs`, `plugin_roles.rs` pin `0.29.1`. (c) `PINS` ends at `0.29.1`, `SCHEMA_PINS` at `0.29.0`. (d) `tag-check.sh v0.29.1` exits 0 printing `0.29.1`; `deploy.rs` green. M13: a sentence dropped.
- [x] **AC-9 [docs, cli] Canon and genre.** `check` 0 errors, 0 warnings; `queue-backup.md` names neither RM-0053 nor a waiting spec-ID cure; the "Known limits" of `decision-record.md`, `proposal-kinds.md`, `proposal-create-apply.md` and `hub-git.md` point to `spec-ids.md` "Known limits"; `proposal_genre.rs` scans `amend.rs` and `floor.rs`. M14: a fixture prefix literal fails the scan.
- [x] **F1 [form]** clippy `-D warnings`, fmt clean; no dependency; the default `cargo nextest` run passes; `check` 0 errors, 0 warnings.

## Out of scope

Name-shape IDs, `T-`, `C-`, revisions; (B) and (C), `v4`; a reservation on the remote (ADR-0064).

## Implementation

Review: ACCEPT with nits; iteration 2 ACCEPT (the guard, the overflowing body).

| Module | What it does |
|---|---|
| core `record.rs`, `check/engine.rs` | `HUB_ID_START`, `IdSide`, `highest_record_number`; `hub_numbered` |
| store `floor.rs`, `queue.rs`, `lib.rs` | `id_side`; `next_number` by side, its guard |
| CLI `decide.rs`, `create.rs`, `amend.rs`, `lib.rs`* | the side's maximum; `NewIds.side`, `next_free`, `off_side`; `HUB_ID_START` for mcp |
| mcp `hub.rs`, `intake.rs`; plugin; `.env.example` | the sentences, asserts; the skill; 0.29.1 |

\* Outside the listed scope. Deviations: `NewIds.side` is an `Option`, `None` only at apply when the floor cannot be read, the hint then counting every number; a create's `--dry-run` exits 2 on an invalid floor (an amendment has no dry run; it meets the floor at step 4, not its insert). Not covered: no direct test that a kept record ID is written as kept on either side (pre-existing reopen behaviour); no wire test of stdio `propose_change`'s off-side refusal (its library path is covered by the CLI and the hub's `/mcp`). Spec defects: AC-3(e)'s "stores nothing" means no record, no row changed; no amendment `--dry-run` exists; no estimate made.
