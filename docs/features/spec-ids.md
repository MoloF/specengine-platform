---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, scripts/deploy]
ref: RM-0073; Phase 7, the first item of release `v3`, the hosted hub (08 s2 Phase 7, "Release scope"); S1-S7 of 2026-11-04 and the settlements D1-D12 below are the orchestrator's working answers, each awaiting the owner's review; the draft ADR `spec-ids` awaits the owner
---

# spec-ids: a hub mints spec IDs from 100001, a terminal below it (RM-0073)

## Why

A hub and a terminal mint spec IDs from the same numbers. Records (`docs/canon/decision-record.md` "ID") are numbered from the root's index and the queue's issued `record_id`. Created node IDs are named by the proposer, and the "next free" hint counts the index and the live creates (`docs/canon/proposal-kinds.md` "Reservation"). So once `hub-git` (RM-0051) pushes and `hub-mcp` (RM-0053) lets a hub raise creates and decisions, a hub and a developer can both write `DEC-0024` or `STY-0002`:
- a record or a new file at one path is an add/add conflict, and the sync is `held`;
- one ID at two paths merges cleanly and is pushed, then fails as `id-taken`, an error that turns `enforce` red;
- a person renumbers by hand, and queue rows and `Interprets:` trailers keep the old ID.

`apply-outputs` (RM-0072) gave a hub its own `PR-` range from a stored floor (ADR-0063), and named spec IDs a known limit to be cured here. This slice extends that range to spec IDs, so a hub's and a terminal's spec IDs never meet. It refuses no apply (ADR-0012) and changes no format.

Decided by the orchestrator (binding here, each awaiting the owner):
- **S1.** The cure is (A): a hub's own range from 100001, for records and for created number-shape IDs, as `PR-` has (ADR-0063). Reading the tracking ref before a mint (B) and a renumbering proposal (C) wait for `v4`.
- **S2.** One core constant for the start, equal to `HUB_PROPOSAL_FLOOR`. The side is decided by the data directory's `proposal-floor` file, so a host terminal under the hub's `HOME` is on the hub's side.
- **S3.** `id-width` exempts only numbers from 100001; an over-wide number below stays a finding.
- **S4.** A created ID on the wrong side is refused at propose only (exit 1, naming the next free ID, nothing stored). Apply steps 4 and 6 are unchanged (ADR-0012; risk R2 is the trap).
- **S5.** Feature-scoped IDs and plan items are in the range.
- **S6.** Every `spec` that writes to the repository is upgraded before the hub's first spec-ID mint. RM-0074's "Upgrade" bullet says so.
- **S7.** Two terminals colliding is a known limit of `v3`.

Settled here, each a working answer:
- **D1. The constant:** core `record.rs` `pub const HUB_ID_START: u64 = HUB_PROPOSAL_FLOOR;`, beside `record_id`. The check, the store and the CLI already import `record`. The two names keep two meanings: `PR-` readers use the stored floor's number, spec IDs use this constant and read the file only for the side.
- **D2. The side:** core `IdSide {Terminal, Hub}`; store `floor.rs` `id_side(dir)`. The side is `Hub` when `<data dir>/proposal-floor` exists and reads as a floor, whatever number it holds. With no file it is `Terminal`. An invalid file gives `read_floor`'s error, so the mint stores nothing (exit 2, as a `PR-` mint). The side is read on each mint: by the CLI for the preview and the propose step, and again by the store under the write lock when it issues a record.
- **D3. Records:** core `highest_record_number` takes the side. `RecordSeries` keeps its fields, and its `corpus_max` is now the side's highest number. The store's `next_number` counts only the issued `record_id`s on its side, then takes `IdSide::next`.
- **D4. Created IDs:** `NewIds` gains `side`, and `next_free` counts only that side's numbers. The new step `NewIds::off_side` runs in the two propose paths after `checked` returns (CLI `create.rs:345`, `amend.rs:213`), never inside `checked`. `checked` also serves apply step 6 (`create.rs:1209`), so the apply never meets the step.
- **D5. The refusal texts** are in "Data", exact.
- **D6. `id-width`:** a definition passes when it is written as `record_id` writes a number of `HUB_ID_START` or more. The check is the same on either side.
- **D7. A terminal's last number** is 100000. Past it no record or "next free" exists. A hub's numbers run to `u64::MAX`.
- **D8. The MCP texts:** the exact sentences are in "Data". Both texts drop `its document, ` from the `target` line to stay under the 2048 B cap, and each gets a compile-time assert that ties the stated number to `HUB_ID_START`.
- **D9. The plugin:** the `propose-spec-change` skill gets a line, and the plugin goes 0.29.0 -> **0.29.1**, a PATCH, because only a skill's text changes (`docs/canon/plugin-files.md` "Version"). The release tag becomes `v0.29.1` (see "Release tag").
- **D10. The canon home** is a new Tier 2 page, `docs/canon/spec-ids.md`, and the ADR's `canon:` is `#the-range`. This differs from the analysis's `queue-backup.md#id-floor`, because that page has 271 B free and the rule spans records, creates and the check. `queue-backup.md` "ID floor" points to the new page.
- **D11. One ADR,** `spec-ids` (ADR\-0070, the next free number), `scope: [architecture]`, drafted below and filed with its canon at ship, as `read-trust` did. It replaces ADR-0063's last clause ("Spec IDs ... stay minted per queue ... until RM-0073"). ADR-0063 stays `accepted` for `PR-`, as ADR-0061 narrowed ADR-0035 without superseding it. The `zone-index` draft, deferred to `v4`, now takes the next free number when it resumes. `docs/canon/architecture.md` does not change: no rule there names an ID range or a mint, `#ids` keeps Latin IDs, `#control` keeps every apply open, and a constant applied to every prefix is no domain fact (`#universal`).
- **D12. `docs/canon/import-layout-verifier.md` does not change,** though the analysis lists it. Rule 3 attributes an `id-width` finding that was raised, and none is raised from 100001.

## Description and interactions

**On a hub** (a data directory that holds `proposal-floor`; the daemon's start writes it, `docs/canon/queue-backup.md` "ID floor"):
- A confirmed question, discrepancy or decision is written as `DEC-100001`, then `DEC-100002`, whatever low numbers the tree holds.
- An agent over `/mcp` names created IDs from 100001: `STY-100001`, or `stamina-tuning/AC-100001` in a feature document. A plan item is numbered the same way.
- A lower number is refused, and the refusal names the next free ID on the hub's side.
- The same holds for `spec approve` and `spec propose` run on the host under the hub's `HOME`, because the file decides the side, not the daemon's flag.

**On a terminal** (no file):
- Numbering is as before, except that numbers from 100001 in the tree or the queue are ignored. After a merge that brought in `DEC-100001`, the next record is still `DEC-0025`.
- An agent that names `STY-100002`, because the tree's highest is `STY-100001`, is refused once, naming `STY-0002`.

**`spec check`** reports no `id-width` on `DEC-100001` under `width = 4`. Any other ID whose digit count differs from its width is still reported.

**Merges:** the hub's records and creates and a developer's no longer share a number, so the add/add conflict and `id-taken` of a same-number pair cannot occur. Two developers' terminals still share the numbers below 100001 (S7).

**Unchanged:**
- every command, flag, exit code, JSON key, MCP tool, schema, queue column and dump line;
- apply's steps, completion and reject;
- `PR-`, `T-` and `C-` numbering;
- name-shape IDs;
- `specengine.toml`.

## Data

### The constant and the side

```rust
// core record.rs
pub const HUB_ID_START: u64 = HUB_PROPOSAL_FLOOR; // 100_001
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdSide { Terminal, Hub }
impl IdSide {
    /// Hub: number >= HUB_ID_START; Terminal: number < HUB_ID_START (0 too,
    /// so a terminal's `X-0000` is judged as today).
    pub const fn holds(self, number: u64) -> bool;
    /// The next number after `highest`, the side's highest (0: none):
    /// Hub max(highest + 1, HUB_ID_START); Terminal highest + 1 while below
    /// HUB_ID_START, else None; None on overflow.
    pub const fn next(self, highest: u64) -> Option<u64>;
}
```

```
<data dir>/proposal-floor   "100001\n"  -> IdSide::Hub:      DEC-100001, STY-100001, stamina-tuning/AC-100001, R-100001 (width 2), PR-100001
<data dir>/proposal-floor   "5\n"       -> IdSide::Hub:      spec IDs as above; PR- from 5 (the stored number)
(no proposal-floor)                     -> IdSide::Terminal: DEC-0025, STY-0002, stamina-tuning/AC-08, PR-0007
<data dir>/proposal-floor   "x\n"       -> error: `<data dir>/proposal-floor: not a proposal floor (a decimal number from 1, then LF)`, exit 2
```

A prefix wider than six digits pads the hub's numbers (`width = 8`: `X-00100001`). A prefix with no `width` writes them unpadded (`record_id`'s width 1).

### Records

`n = side.next(max(c, q))`, where:
- `c` is core `highest_record_number(files, spec, side)`: the highest number on the side among the prefix and its `aliases_from`, defined or in `aliases:`, in the recorded root's refreshed index;
- `q` is the highest `record_id` of the prefix the project's queue issued on the side.

It is previewed at step 4 and issued at step 7 under the write lock, where `approve_record_from` recomputes it. A different result is `Issued`, exit 1, run again, as today. A record ID already kept (a reopened item, a restored row) is written as kept, whatever its side.

A terminal past 100000 has no next number. The error is `QueueError::Invalid`, so step 4 exits 2:

```
`DEC` numbers reach 100000, the last below a hub's 100001: no record ID follows
```

### Created IDs: the propose step

Propose step 4 gets a last check, after every check of `docs/canon/proposal-kinds.md` "Propose" step 4. For each new ID in text order (canonical, `slug/ID` for a feature-scoped prefix) of a number-shape prefix: if `side.holds(number)` is false, the propose refuses with exit 1. Nothing is stored or reserved, and no event is logged. Name-shape IDs are never judged.

It runs in `spec propose create` (both forms), in `spec propose amendment`, with `--dry-run` alike, and in MCP `propose_change` with `kind` `create` or `amendment`. It is not part of `NewIds::checked`. `prepare_file` (apply step 6), preflight's section steps (apply steps 4 and 6), completion and reject never call it.

**Next free** (`NewIds::next_free`) is one more than the highest number on the side among the prefix and its `aliases_from`, in the ID's own scope, over the index, the live creates and the text's new IDs, written by `record_id`. It is `None` past a terminal's 100000. This rule holds in every refusal that names a next free ID:
- taken in the index;
- reserved by a live create;
- the insert's race (`reserved_refusal`);
- apply step 6's taken hint (text only);
- the new step.

### Refusal texts

The ID is canonical and the line is its site's, as in the "defined twice" refusal. `100001` is formatted from `HUB_ID_START`; no prefix is written as a literal.

```
terminal: `STY-100002` (line 3): numbers from 100001 are a hub's; the next free here is `STY-0002`; nothing stored
hub:      `STY-0002` (line 3): a hub numbers from 100001; the next free here is `STY-100001`; nothing stored
feature:  `stamina-tuning/AC-100001` (line 41): numbers from 100001 are a hub's; the next free here is `stamina-tuning/AC-08`; nothing stored
```

With no next free ID, the clause `; the next free here is ...` is left out. MCP returns the same reason as an error result, with the document.

### `id-width`

In core `check/engine.rs` `ids()`, a number-shape definition whose digit count differs from `width` gets no `id-width` when its body is ASCII digits for some `n >= HUB_ID_START` and the ID equals `record_id(prefix, width, n)`, that is, with no zero past the padding. Results under `width = 4`, `width = 2` and `width = 8`:
- `DEC-100001`, `R-100001`, `X-00100001`: no finding;
- `DEC-100000`, `R-123`, `R-7`, `R-0100001`, `X-100001`: `id-width`.

### Store, core and CLI API

| Crate | Item | Change |
|---|---|---|
| core `record.rs` | `HUB_ID_START`, `IdSide` (`holds`, `next`) | new |
| core `record.rs` | `highest_record_number(files, spec, side)` | gains `side`: counts only `side.holds` numbers |
| core `check/engine.rs` | `ids()` | the `id-width` exemption above |
| store `floor.rs` | `id_side(dir: &Path) -> Result<IdSide, StoreError>` | new, exported beside `read_floor` |
| store `queue.rs` | `next_number(conn, project, series, side)`; `next_record`, `approve_record_from` read `id_side(&self.dir)` | the side's issued IDs, `IdSide::next` |
| CLI `decide.rs` | step 4: `id_side(&context.data_dir)` -> `corpus_max` | the side's corpus maximum |
| CLI `create.rs` | `NewIds {scheme, decision_prefix, slug, side}`; `next_free` by side; `NewIds::off_side(&self, sites, ids, corpus, reserved) -> Option<String>`; the call after `:345`; `prepare_file`'s `NewIds` takes the side (hint text only) | the propose step |
| CLI `amend.rs` | its `NewIds` takes the side; `off_side` after `:213` | the propose step |

### The MCP texts

The sentence `you name each new ID; a taken one is refused naming the next free one.` and the `target` line's `(its document, the whole file)` change in both texts:

| Text | New sentence | `target` line | Bytes, cap 2048 |
|---|---|---|---|
| hub `HUB_CHANGE_DESCRIPTION` (mcp `hub.rs`) | `you name each new ID, from 100001 on this hub; a taken or lower one is refused naming the next free ID.` | `(the whole file)` | 2021 -> 2040 |
| stdio `CHANGE_DESCRIPTION` (mcp `intake.rs`) | `you name each new ID, below 100001 unless on a hub; a taken one is refused naming the next free ID.` | `(the whole file)` | 2029 -> 2044 |

The new asserts: `holds_number(HUB_CHANGE_DESCRIPTION, "from ", HUB_ID_START, " on this hub")` and `holds_number(CHANGE_DESCRIPTION, "below ", HUB_ID_START, " unless on a hub")`. `INSTRUCTIONS` (stdio, 2048 B, full) and `HUB_INSTRUCTIONS` (1979 B) stay as they are. No schema changes, so `SCHEMA_PINS` gains no entry.

### The plugin

In `plugin/specengine/skills/propose-spec-change/SKILL.md` the body stays at 4092 B of 4096, with no digit (`plugin-files.md` "Content"). Five edits:
1. "A new section or file", the "You name" bullet:
   - `numbered as the project writes its IDs; look it up` becomes `numbered as the project writes its IDs, a hub's range above a terminal's; look it up`;
   - `A taken one is refused naming its holder and,` becomes `A taken or other-range one is refused naming its holder or range and,`.
2. The same section: `- Your new IDs are held while the proposal is open, in the spec only once approved; the reply` becomes `- Your new IDs are held while the proposal is open; the reply`.
3. The same section: `ask the owner to reject it, and resend once` becomes `ask the owner to reject it, resend once`.
4. Step 5: `a mistake is fixed by a corrected text (below)` becomes `fix a mistake with a corrected text (below)`.
5. "After sending": `never edit it or apply your text yourself` becomes `never edit it yourself`.

`plugin.json` `version` is `0.29.1`. `PINS` (mcp `tests/plugin_files.rs`) appends `0.29.1` with the tree's BLAKE3. `scripts/deploy/.env.example` sets `SPECENGINE_VERSION=0.29.1`, because `tag-check.sh` and `crates/specengine-http/tests/deploy.rs` require it to equal the plugin's version.

### Release tag

`image.yml` publishes only a tag that equals `v` + the plugin's version (`tag-check.sh`, `hub-deploy` Q8). No tag exists yet, so `v0.29.0` was never published. After this slice the hub's release tag is **`v0.29.1`** and its image is `ghcr.io/molof/specengine:0.29.1`. At ship, `scripts/deploy/README.md` "Install" clones `--branch v0.29.1`. If the owner prefers a MINOR step, every `0.29.1` above becomes `0.30.0` and the tag `v0.30.0`.

### Formats that do not change

`QUEUE_SCHEMA_VERSION` 9, `STATE_FORMAT` 3, `PROPOSAL_COLUMNS` 46, `INDEX_FORMAT` 17, the `proposal-floor` grammar, `RawPrefix` (core `scheme_toml.rs:138-150`) and the top-level keys of `specengine.toml` (`project_toml.rs:21-35`; S1 rejects a per-prefix key because older binaries refuse it, `deny_unknown_fields`).

## Rules and edge cases

- **I1.** WHEN a data directory holds a readable `proposal-floor`, every number-shape spec ID minted there SHALL be `HUB_ID_START` or more; WHEN it holds none, every one SHALL be below it. A record is minted by its issue; a create by its propose, its ID named by the proposer.
- **I2.** The side SHALL come from the file alone: never from `--public-origin`, a flag, a key or the stored number.
- **I3.** Both maxima a record mint reads, the corpus's and the queue's, SHALL count only the side's numbers (R3: a half fix leaks).
- **I4.** WHEN a new ID of a number-shape prefix is on the other side, propose SHALL refuse it with exit 1, naming the next free ID on its side, and store, reserve and log nothing; the dry run SHALL refuse it alike.
- **I5.** Apply (steps 1-10 of either form), completion, reject and `confirm` SHALL NOT judge a side (ADR-0012). A create, amendment or record ID raised or kept on the other side applies as stored.
- **I6.** `id-width` SHALL pass only a `record_id`-form number of 100001 or more, on any machine.
- **I7.** No queue, dump or index format, column, config key, MCP tool, key or schema SHALL change.
- **I8.** No source SHALL name a prefix, kind or fixture word (`proposal-kinds.md` "Genre"); `100001` comes from `HUB_ID_START` only.

Edge cases:
- **Taken and other side at once:** the existing checks run first, so a taken ID reports as taken, with the next free ID of its side.
- **Several new IDs on mixed sides:** the first in text order is named; the next free ID counts the text's other new IDs of the side.
- **The hub's first start** writes the file under `daemon.lock` before it listens, so the hub's first mint is in range. A host terminal that mints before that start is on the terminal's side.
- **A dump** carries no file. A terminal's dump restored on a hub keeps its rows: open creates hold low IDs, and kept record IDs stay. A hub's dump restored on a terminal mints below 100001 and ignores the hub's issued IDs.
- **A laptop that once ran `--public-origin`** holds the file and mints in the hub's range. Deleting `<data dir>/proposal-floor` by hand makes it a terminal again; its `PR-` numbers go on from the highest stored.
- **Sorting by path** puts a hub's IDs among a terminal's: `DEC-1000.md`, `DEC-100001.md`, `DEC-1001.md`.

## Acceptance criteria

Tests run on scratch copies of the fixtures, with no new fixture. A hub's side in a CLI test is a data directory where store `store_floor` wrote the file. M names the mutations that must turn each criterion red: M1-M14 from the analysis, plus M15 and M16.

- [ ] **AC-1 [default, http] Records through the real merge** (`fixtures/apply-outputs/forge`, `DEC` width 4; the bare-remote world of `crates/specengine-http/tests/hub_git.rs:1-9`).
  - A developer's clone approves a question and pushes `DEC-0001`. Before the next sync, the hub confirms a question raised on it and commits `DEC-100001`.
  - The sync then ends `ok`, with a merge commit that holds both files, and `spec check --json` on the hub's checkout reports no `id-taken` and no `id-width`.
  - The developer pulls and approves another question, which becomes `DEC-0002`.
  - M1: the hub's `next_number` without the range (both mint `DEC-0001`; `held`). M2: the terminal's corpus maximum counts numbers from 100001 (the developer mints `DEC-100002`).
- [ ] **AC-2 [default, http] Creates through the real merge** (`fixtures/card-press`, `STY` width 4, `STY-0001` in the tree).
  - The hub's `/mcp` `propose_change` create names `STY-0002` and is refused naming `STY-100001`. Resent with `STY-100001`, it is stored and confirmed.
  - A developer creates and approves `STY-0002` and pushes.
  - The sync merges cleanly, and `spec check --json` reports no `id-taken`.
  - M3: the propose step removed (the hub stores `STY-0002`; add/add or `id-taken`).
- [ ] **AC-3 [default, core, store, cli] The mint by side.**
  - (a) Core: `highest_record_number` over files holding `DEC-0024`, `DEC-100003` and `aliases: [QST-100002]` (spec-a's `Q` aliases `QST`): `DEC` is 24 for `Terminal` and 100003 for `Hub`; `Q` is 0 and 100002. `IdSide::next`: `Hub` 0 -> 100001, `Terminal` 99999 -> 100000, `Terminal` 100000 -> `None`.
  - (b) Store, with a floor: `next_record` gives `DEC-100001` with `corpus_max` 0 and a restored row issued `DEC-0030`, and `DEC-100008` with a row issued `DEC-100007`. A spec-a-shaped `R` width 2 gives `R-100001`.
  - (c) Store, with no floor: `DEC-0025` with `corpus_max` 24 and a restored row issued `DEC-100004`. With `corpus_max` 100000: the error of "Records", nothing written.
  - (d) CLI on spec-a, with a floor: `spec approve` of a question writes `docs/records/DEC/DEC-100001.md`; with `DEC-100003` added to the tree it writes `DEC-100004`. With no floor and `DEC-100003` in the tree it writes `DEC-0024`.
  - (e) An invalid `proposal-floor`: the preview stores nothing, exit 2, with the floor's text.
  - M4: the queue's issued IDs not filtered by side. M5: the side read from the daemon's flag instead of the file (the CLI's host terminal mints `DEC-0024`).
- [ ] **AC-4 [default, cli] Propose** (spec-a, card-press).
  - (a) No floor: a file-form create holding `id: STY-100002` exits 1 with the terminal text naming `STY-0002`; the queue is empty and the event count unchanged.
  - (b) A floor: a create of `STY-0002` exits 1 naming `STY-100001`.
  - (c) With `STY-0001` and `STY-100001` in the tree, the taken refusal, the reserved refusal and the insert's race (`proposal_kinds.rs` n2's seam) name `STY-0002` on a terminal and `STY-100002` on a hub.
  - (d) The section form (`--base`) and `spec propose amendment` refuse alike.
  - (e) A floor: spec-a's `stamina-tuning.md` gaining `{#AC-100001}` is stored with `target_ids` holding `stamina-tuning/AC-100001`. No floor: it is refused naming `stamina-tuning/AC-08`.
  - (f) `--dry-run` of (a) and (b): the same exit and stderr, nothing stored.
  - M6: the step removed. M7: `next_free` ignores the side. M8: the dry run skips the step.
- [ ] **AC-5 [default, http, cli] Never refuse an apply** (ADR-0012).
  - (a) A terminal's dump holding an open create of `STY-0002` and an open amendment whose new file defines `STY-0003` is restored with `spec import-state` into an empty queue whose data directory holds the floor. On the hub, each confirm answers 200 and is `applied`.
  - (b) The reverse: a hub's open create of `STY-100001`, restored with no floor, applies by `spec approve`.
  - M9: the side judged inside `NewIds::checked` (apply step 6 refuses: 409, exit 1).
- [ ] **AC-6 [default, core] `id-width`.** `check::run` over the "`id-width`" examples gives exactly the findings listed there. M10: the rule unchanged (`DEC-100001` reported). M11: every over-wide number passes (`R-123`). M15: the value alone exempts (`R-0100001` passes). M16: `>` for `>=` (`DEC-100001` reported) or a lower start (`DEC-100000` passes).
- [ ] **AC-7 [default] Formats.** The pins of `QUEUE_SCHEMA_VERSION` 9, `STATE_FORMAT` 3, `PROPOSAL_COLUMNS` 46 against `PRAGMA table_info`, and `INDEX_FORMAT` 17 (`format.rs`) stay green and unedited. `git diff` of core `scheme_toml.rs` and `project_toml.rs` is empty. M12: a column added.
- [ ] **AC-8 [build, mcp, plugin] The texts.**
  - (a) Both descriptions hold the sentences of "The MCP texts", ASCII, at 2040 and 2044 B, with the `holds_number` asserts. `tools/list` over stdio and over a hub's `/mcp` shows them.
  - (b) The skill holds the five edits, its body at 4092 B or less with no digit, and `plugin_skills.rs`, `plugin_files.rs` and `plugin_roles.rs` pin `0.29.1`.
  - (c) `PINS` ends at `0.29.1`, and `SCHEMA_PINS` still ends at `0.29.0`.
  - (d) `sh scripts/deploy/tag-check.sh v0.29.1` exits 0 and prints `0.29.1`, and `deploy.rs` is green.
  - M13: a sentence dropped (the build or the text test fails).
- [ ] **AC-9 [docs, cli] Canon and genre.**
  - After `export index`, `check` reports 0 errors and 0 warnings.
  - `queue-backup.md` names neither RM-0053 nor a spec-ID collision that "waits".
  - The "Known limits" of `decision-record.md`, `proposal-kinds.md`, `proposal-create-apply.md` and `hub-git.md` name the two-terminal and older-`spec` limits, or point to `spec-ids.md` "Known limits", which does.
  - `proposal_genre.rs` scans `crates/specengine-cli/src/amend.rs` and `crates/specengine-store/src/floor.rs`.
  - M14: a fixture prefix literal (`DEC` in `decide.rs`, `STY` in `amend.rs`'s new step) fails the scan.
- [ ] **F1 [form]** clippy with `-D warnings` and fmt are clean; no dependency is added; the default `cargo nextest` run passes; `check` reports 0 errors and 0 warnings.

## Out of scope

Known limits of `v3`, stated in the canon at ship:
- **Two terminals** collide on spec IDs and `PR-` (S7).
- **One shared range:** two hubs on one remote, a copied hub data directory, and a laptop holding a stale `proposal-floor` all mint in the hub's range.
- **Before the hub's first start**, a mint is on the terminal's side.
- **An older `spec` (R1)** mints by the highest number, so after a merge it mints in the hub's range, and its check reports the hub's IDs `id-width`, red under `enforce`; S6 answers this.
- **Raised or kept on the other side:** creates raised before the upgrade, creates and kept record IDs restored from the other side's dump, all apply with their IDs (I5).
- **Sorting by path** interleaves the two sides.

Not done here: name-shape IDs; `T-`, `C-` and revisions; reading the tracking ref (B) and renumbering (C), deferred to `v4`; a reservation on the remote (it needs the network: `architecture.md#distribution`, ADR-0064).

## Implementation

To be filled in after implementation. The plan, the risks and the ship's canon follow.

### Files in scope

| Crate | Files | What changes |
|---|---|---|
| core | `record.rs`, `check/engine.rs`, `lib.rs` | `HUB_ID_START`, `IdSide`, `highest_record_number`'s side, the `id-width` exemption |
| store | `floor.rs`, `queue.rs`, `lib.rs` | `id_side`, `next_number` by side in `next_record` and `approve_record_from` |
| CLI | `decide.rs`, `create.rs`, `amend.rs` | the side's corpus maximum; `NewIds.side`, `next_free`, `off_side` and its two calls |
| mcp | `hub.rs`, `intake.rs` | the two sentences, the `target` trims, the asserts |
| plugin | `skills/propose-spec-change/SKILL.md`, `.claude-plugin/plugin.json` | the five edits; 0.29.1 |
| scripts | `deploy/.env.example` | `SPECENGINE_VERSION=0.29.1` |
| tests (test-engineer) | core `tests/decision_records.rs` (the call takes `IdSide::Terminal`), `tests/spec_ids.rs`; store `tests/spec_ids.rs`; CLI `tests/spec_ids.rs`, `tests/proposal_kinds.rs` (n2 on a hub), `tests/proposal_genre.rs`; http `tests/spec_ids_hub.rs` beside `hub_git.rs`, `tests/deploy.rs`; mcp `tests/plugin_files.rs`, `tests/plugin_skills.rs`, `tests/plugin_roles.rs`, the hub and stdio text tests | AC-1 to AC-9 |

### Work order

1. Core: the constant, `IdSide`, the side of `highest_record_number`, the `id-width` exemption (AC-6).
2. Store: `id_side`, `next_number` (AC-3 b, c, AC-7).
3. CLI records (AC-3 d, e).
4. CLI creates: `NewIds.side`, `next_free`, `off_side` and its two calls (AC-4, AC-5 b, AC-9's scan).
5. The MCP texts, the skill, the version, `.env.example` (AC-8).
6. The http criteria through the real merge (AC-1, AC-2, AC-5 a).

If the slice has to be cut, records and `id-width` come first (steps 1-3), creates second; both land before the hub's gate (step 11b-3).

### Risks

- **R1** Older binaries go red under `enforce` and mint in the hub's range: S6, the guide's bullet, and RM-0074's "Upgrade" bullet answer it.
- **R2** The side judged inside `NewIds::checked` would reach apply (`create.rs:345`, `amend.rs:213`, `create.rs:1209`) and refuse applies against ADR-0012: D4 rules it out, AC-5 and M9 guard it.
- **R3** A half fix (the corpus maximum or the queue's alone) leaks: I3, guarded by M2 and M4.
- **R4** The MCP texts' 2048 B cap: the hub's text is measured at 2040 B, the stdio text at 2044 B; the asserts fail the build.
- **R5** An agent's first wrong guess costs one refused call; the refusal names the fix.
- **R6** Genre: the new code must stay inside the scanned files; AC-9 adds `amend.rs` and `floor.rs` to the scan.
- **R7** A stale `proposal-floor` on a laptop: a known limit, with deleting the file by hand as the way out.

### Canon at ship

Free bytes are given as of this spec.

**New page.** `docs/canon/spec-ids.md` (Tier 2; `docs/index-canon-spec.md` claims it, with 4 426 B free; scope `[crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp]`). Its sections:
- an intro: the ADR, built by `spec-ids`, the working answers;
- "The range", the ADR's anchor: I1, I2, the constant and the side;
- "Records", "Created IDs", "`id-width`";
- "Older binaries" (S6);
- "API", "Known limits", "Tests".

**Tight pages, with where their room comes from:**
- `queue-backup.md` (271 B free), "ID floor":
  - "Read" adds that the file's presence is the spec-ID side;
  - the limits paragraph's spec-ID sentence (about 330 B, ending "the cure waits for RM-0053", which is stale) becomes `Spec IDs: spec-ids.md.`;
  - net about 250 B freed.
- `proposal-kinds.md` (1 B free), about 400 B needed:
  - "Propose" step 4 gains a last bullet: the other side's refusal, never at apply;
  - "Next free" is on the queue's side;
  - "MCP and plugin" gets the texts' range sentence;
  - "Known limits" first bullet points to `spec-ids.md`;
  - room: the plugin paragraph (549 B), which `plugin.md` "Skills" already holds, becomes a pointer (about 450 B freed), and the quoted `dry_run` schema text, which the code pins, goes (about 105 B).
- `spec-check.md` (11 B free): the "IDs." bullet gets the exemption (about 80 B). Room: "Open owner questions" Q-4, answered, keeps only its answer (about 150 B freed).
- `hub-git.md` (0 B free): the "Merged-in history" bullet of "Known limits" also names the spec-ID limits and the page (about +20 B). Room: the first bullet of the same section, reworded.
- `hub-mcp.md` (5 B free): "Texts" names `propose_change`'s range sentence and its assert (about 90 B). Room: the same paragraph's read-tail sentence (about 140 B), which `spec-read-freshness.md` holds.
- `serve.md` (192 B free): the "Files" line of `proposal-floor` gains "and its spec-ID side".
- READMEs:
  - core (22 B free): `record`: `decision-record.md`, `spec-ids.md`;
  - store (9 B free): `floor.rs` `id_side` beside `read_floor`, with room from the same line's restated canon;
  - CLI (15 B free), mcp (10 B free) and http: unchanged, since they cite these modules by canon.
- `scripts/deploy/README.md` (263 B free):
  - "Install" `v0.29.0` -> `v0.29.1`;
  - a "Limits" bullet, **Developers' `spec`**: before the hub records a decision or creates a node, everyone who commits to the repository runs this release or later (about 200 B).

**Pages with room:**
- `decision-record.md` (2 527 B free): "ID" counts the side's numbers; "Known limits" first bullet.
- `proposal-create-apply.md` (8 038 B free): step 6 judges no side; "Known limits" line 37.
- `hub-confirm.md` (1 430 B free): "Where it ends" says spec IDs too start at 100001.
- `plugin.md` (447 B free): the `propose-spec-change` row adds "an other-range one alike".
- `plugin-files.md` (5 696 B free): "Version" says current 0.29.1, PATCH 0.29.1 for the other range.
- `CLAUDE.md` (7 414 B free): the state line names `spec-ids`, the ADR and the page, and says 11b-2 is next.

**Plan:**
- RM-0073 shipped and compressed;
- RM-0074 "Upgrade" (S6): before the hub's first record or create, every `spec` that writes to the repository, each developer's and the host's, runs this release or later;
- 08's `spec-ids` row shipped with the ADR's number and the estimate, and 08 "Release scope".

**Unchanged:**
- `architecture.md` (4 B free; D11);
- `import-layout-verifier.md` (D12);
- `mcp-read.md`, whose "Texts" are the read tools' and `INSTRUCTIONS`;
- ADR-0063 (accepted, narrowed by the new ADR) and ADR-0012.

### Draft ADR

This is a working answer, awaiting the owner. It takes its date at ship and is 1 519 B with its front-matter:

```markdown
---
class: decision
id: ADR-0070
title: A hub mints spec IDs from 100001 and a terminal below it; propose refuses the other side, apply never
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/spec-ids.md#the-range
ref: spec-ids (RM-0073); the orchestrator's S1-S7 of 2026-11-04, awaiting the owner's review
---

# ADR-0070

**Decision.** Accepted by the orchestrator in the owner's absence, awaiting the owner's review. Replaces ADR-0063's last clause; ADR-0063 stands for `PR-`. A data directory holding `proposal-floor` mints every number-shape spec ID, a record or a created node, from 100001 (core `HUB_ID_START`, `HUB_PROPOSAL_FLOOR`'s value); any other mints below it. Each counts only its side's numbers, in the index and the queue. Propose refuses a new ID of the other side, naming the next free one; apply judges no side (ADR-0012). `id-width` passes a number from 100001.

**Why.** Once a hub pushes, its records and creates met a terminal's on one branch: an add/add conflict held the sync, or `id-taken` followed a clean merge, and a person renumbered. A range holds offline and changes no format; a reservation needs the network, a qualified ID a new grammar, a per-prefix key fails older binaries.

**Cost.** Hub IDs take six digits and sort among a terminal's by path. Two terminals still collide. An older `spec` mints in the hub's range and fails `id-width` on its IDs, so every writer upgrades before a hub's first mint. A create raised on the other side applies with its ID.
```
