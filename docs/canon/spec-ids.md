---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp]
owner: owner
reviewed: 2026-11-04
---

# Spec IDs: a hub mints from 100001, a terminal below it

ADR-0070, built by `spec-ids` (`docs/features/spec-ids.md`, RM-0073, release `v3`); working answers of the orchestrator, the owner's review open. A hub and a terminal never mint one spec ID, so a merge of a hub's records and creates with a developer's meets no add/add conflict and no `id-taken`. It extends a hub's `PR-` range (ADR-0063, `queue-backup.md` "ID floor") to spec IDs, refuses no apply (ADR-0012) and changes no format: queue schema 9, dump format 3, 46 columns, index format 17, the floor's grammar and `specengine.toml` as before. Code: core `record.rs`, `check/engine.rs`; store `floor.rs`, `queue.rs`; CLI `decide.rs`, `create.rs`, `amend.rs`.

## The range

- **The constant**: core `HUB_ID_START` = `HUB_PROPOSAL_FLOOR`, 100001. Two names, two meanings: `PR-` mints use the stored floor's number, spec IDs this constant, reading the file only for the side.
- **The side** (core `IdSide {Terminal, Hub}`, store `id_side(dir)`): `Hub` when `<data dir>/proposal-floor` exists and reads as a floor, whatever number it holds; `Terminal` when there is none. The file alone decides, never `--public-origin`, a flag, a key or the stored number, so `spec approve` or `spec propose` on the host under the hub's `HOME` is on the hub's side. An invalid file is `read_floor`'s error (`` <data dir>/proposal-floor: not a proposal floor (a decimal number from 1, then LF) ``): the mint stores nothing, exit 2. The side is read on each mint.
- **The rule**: every number-shape spec ID minted on a hub is `HUB_ID_START` or more, every one minted elsewhere below it, and each side counts only its own numbers. A record is minted by its issue, a created node by its propose (the proposer names it). It covers records, created nodes, feature-scoped IDs (`slug/AC-100001`) and plan items; not name-shape IDs, `T-`, `C-`, revisions or `PR-` (its own floor).
- **`IdSide::holds(n)`**: `Hub` `n >= 100001`, `Terminal` `n < 100001` (0 too). **`IdSide::next(highest)`**, `highest` the side's (0: none): `Hub` `max(highest + 1, 100001)` to `u64::MAX`; `Terminal` `highest + 1` up to 100000, then `None`; `None` on overflow.
- **Written** by `record_id`: `DEC-100001` under `width = 4`, `R-100001` under 2, `X-00100001` under 8, unpadded with no `width`.

```
proposal-floor "100001\n" -> Hub:      DEC-100001, STY-100001, stamina-tuning/AC-100001, PR-100001
proposal-floor "5\n"      -> Hub:      spec IDs as above; PR- from 5
no proposal-floor         -> Terminal: DEC-0025, STY-0002, stamina-tuning/AC-08, PR-0007
```

## Records

`n = side.next(max(c, q))`: `c` = core `highest_record_number(files, spec, side)`, the side's highest number of the prefix or its `aliases_from`, defined or in `aliases:`, in the recorded root's refreshed index; `q` = the highest `record_id` of the prefix the project's queue issued on the side (store `next_number`). The CLI reads the side for `c` at step 4 (`decide.rs`); the store reads it again in `next_record` (the preview, step 4) and in `approve_record_from` (the issue, step 7, under the write lock). A record ID already kept (a reopen, a restored row) is written as kept, whatever its side, and reads no floor.

- **A terminal past 100000**: `` `DEC` numbers reach 100000, the last below a hub's 100001: no record ID follows ``; a hub past `u64::MAX`: `` `DEC` numbers reach <n>: no record ID follows ``. `QueueError::Invalid`: exit 2 at step 4.
- **The guard**: the store does not re-filter `c`, but a nonzero `c` off the side it reads is `QueueError::Invalid`, `` `DEC`'s highest number <n> is not on this data directory's side: its `proposal-floor` came or went during the run; run it again ``: exit 2 at step 4, exit 1 at step 7 (refused, `; nothing written`), no record written.
- **An invalid floor** at step 4: exit 2, no record, no row changed; as any refusal at steps 2-10, the run logs its one `proposal.apply_failed`.
- Otherwise a step 7 result other than the preview is `Issued`, exit 1, run again, as ever.

## Created IDs

**Propose step 4's last check** (`proposal-kinds.md` "Propose"), CLI `NewIds::off_side`, run after `NewIds::checked` passed and never inside it: for each new ID of a number-shape prefix in text order (canonical, `slug/ID` in a feature document), the first off the side refuses, exit 1, nothing stored, reserved or logged. An all-digit body past `u64` counts as a hub's. Name-shape IDs are never judged; a taken or reserved ID reports as taken first. It runs in `spec propose create` (file and section forms, `--dry-run` alike), `spec propose amendment` and MCP `propose_change` of kind `create` or `amendment`.

**The side is read at propose step 4** (`id_side`, `NewIds.side`), so an invalid floor exits 2 there, nothing stored: a create's `--dry-run`, which read no floor before, now exits 2 on it; an amendment, which met the floor first at its insert, now meets it at step 4.

**Next free** (`NewIds::next_free`): `side.next` after the side's highest number of the prefix or its `aliases_from` in the ID's own scope, over the index, the live creates and the text's new IDs, written by `record_id`; none past a terminal's 100000, the clause then left out. Every refusal naming a next free ID counts it so: taken, reserved, the insert's race, apply step 6's hint, the new step.

**Apply never judges a side** (ADR-0012): steps 1-10 of either form, completion, reject and a hub's `confirm`. A create, amendment or record ID raised or kept on the other side applies as stored. `prepare_file` (apply step 6) reads the side for its hint's text alone; when the floor cannot be read there, `NewIds.side` is `None` and the hint counts every number, as before the range.

## `id-width`

A number-shape definition whose digit count differs from its `width` gets no `id-width` when its body is ASCII digits of some `n >= 100001` and the ID equals `record_id(prefix, width, n)`, no zero past the padding (core `hub_numbered`). The same on either side. Under `width = 4`, 2 and 8:
- no finding: `DEC-100001`, `R-100001`, `X-00100001`;
- `id-width`: `DEC-100000`, `R-123`, `R-7`, `R-0100001`, `X-100001`.

## Texts

- **Refusals** (the ID canonical, the line its site's, `100001` formatted from `HUB_ID_START`):
  - terminal: `` `STY-100002` (line 3): numbers from 100001 are a hub's; the next free here is `STY-0002`; nothing stored ``;
  - hub: `` `STY-0002` (line 3): a hub numbers from 100001; the next free here is `STY-100001`; nothing stored ``.
  MCP returns the same reason as an error result.
- **MCP**: the `propose_change` descriptions say `you name each new ID, below 100001 unless on a hub; a taken one is refused naming the next free ID.` (stdio `CHANGE_DESCRIPTION`, 2 044 B) and `you name each new ID, from 100001 on this hub; a taken or lower one is refused naming the next free ID.` (`HUB_CHANGE_DESCRIPTION`, 2 040 B), each tied to `HUB_ID_START` by a compile-time `holds_number` assert; no schema changes.
- **Plugin** 0.29.1 (PATCH): `propose-spec-change` names a hub's range above a terminal's and the other-range refusal, with no digit (`plugin.md` "Skills"). The hub's release tag is `v0.29.1`.

## Older binaries

A `spec` from before this release mints by the highest number of either side: after a merge it mints in the hub's range, and its check reports a hub's IDs `id-width`, red under `enforce`. So every `spec` that writes to the repository, each developer's and the host's, runs this release or later before the hub's first record or create (`hub-ops.md` "Upgrade", `scripts/deploy/README.md` "Limits").

## API

Core `HUB_ID_START`, `IdSide` (`holds`, `next`), `highest_record_number(files, spec, side)`; `check/engine.rs` `hub_numbered`. Store `id_side(dir: &Path) -> Result<IdSide, StoreError>` beside `read_floor`; `queue.rs` `next_number(conn, project, series, side)`, private, under `next_record` and `approve_record_from`; `RecordSeries` keeps its fields, `corpus_max` the side's. CLI `NewIds {scheme, decision_prefix, slug, side: Option<IdSide>}`, `off_side`, `next_free`; `lib.rs` re-exports `HUB_ID_START` for mcp's texts. mcp `hub.rs`, `intake.rs`.

## Known limits

Of `v3`; none blocks.

- **Two terminals** still share the numbers below 100001, and `PR-`: their spec IDs can meet at a merge (add/add, `id-taken`), and a person renumbers.
- **One shared range**: two hubs on one remote, a copied hub data directory, or a laptop that once ran `--public-origin` all mint in the hub's range. Deleting `<data dir>/proposal-floor` by hand makes a laptop a terminal again; its `PR-` go on from the highest stored.
- **Before the hub's first start**, a mint in its data directory is on the terminal's side; the start writes the file under `daemon.lock` before listening.
- **An older `spec`** mints in the hub's range and reports its IDs `id-width` ("Older binaries").
- **Raised before the upgrade**: creates raised then, and creates or kept record IDs restored from the other side's dump, apply with their IDs. A dump carries no floor: a terminal's restored on a hub keeps its low open creates; a hub's restored on a terminal mints below 100001 and ignores the hub's issued IDs.
- **Sorting by path** interleaves the sides: `DEC-1000.md`, `DEC-100001.md`, `DEC-1001.md`.
- **The first start's flip at `c` = 0**: 0 is on both sides, so when the CLI reads `Terminal` over a tree holding only hub-range numbers of the prefix and the floor then appears before the store's read, the guard sees nothing and the hub's number ignores the tree's. A same-path clash is still refused at step 4 (the record's path is taken); the window is the hub's first start.

## Tests

Core `tests/spec_ids.rs`, `tests/decision_records.rs`; store `tests/spec_ids.rs`; CLI `tests/spec_ids.rs`, `tests/proposal_kinds.rs`, `tests/proposal_genre.rs` (scans `amend.rs`, `floor.rs`); http `tests/spec_ids_hub.rs` (real merges over bare remotes), `tests/hub_confirm.rs`, `tests/deploy.rs`; mcp `tests/plugin_files.rs`, `plugin_skills.rs`, `plugin_roles.rs`, `mcp_rehearsal.rs`. Criteria: the spec.
