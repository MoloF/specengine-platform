---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-store]
ref: Phase 3 slice 6b-2a of RM-0005 (with 6b-2b `check-binding.md`); split from code-tools 2026-10-16 (its Q28), split again and refreshed at ce6065b 2026-10-21; Q5 2026-10-10, Q28-Q34 2026-10-16, Q-A..Q-H 2026-10-21, under the owner's pre-approval, review open; 05 s5.3; 08 s2; developer 1 iteration, review 1 accepted with nits
shipped: 2026-10-21
adrs: []
---

# verify-node (6b-2a): `spec verify --node`

## Why

Before an agent or the owner hands in work, they ask whether one node's bindings still hold. Plain `spec verify` judges every lock key against the whole tree (`docs/canon/spec-verify.md`). `--task` keeps a task's rows, but it still walks and parses the whole tree. A per-node check has to cost what the node's own files cost, so that its MCP twin `check_binding` (6b-2b, `check-binding.md`) fits MCP's 1 s (`docs/canon/mcp-read.md` "Latency").

Delivered: `spec verify --node REF [--symbol QPATH]`, plain verify's rows kept to REF's own bindings, judged only over the files the code index selects (Q5, Q30, Q-A); for those rows the result is plain verify's. Canon: `docs/canon/spec-verify-scope.md` "`--node`", `spec-verify-output.md`. RM-0005 flips to shipped with 6b-2b. No ADR: ADR-0042 (exits; a finding is never `ok`), ADR-0046 (`architecture-code.md#sync-in-reads`), ADR-0012/0039 (nothing reads a finding), ADR-0004/0005/0035 (nothing written), ADR-0050 (format 2) and ADR-0008, applied.

## Data

### spec verify --node

The forms, the order, the selection, the partial tree, rows, `--symbol`, notes and causes, text and JSON: the canon above. Plain, `--changed`, `--staged`, `--task` and `--tests` keep their rows, text and exits, and none refreshes the code index.

**Files** (Q30, Q-A): the code index selects them, never a whole walk. They are the files of the stored markers `spec refs REF` lists under a holder, plus those of the `spec.lock` entries whose `node` can be a holder's. The lock is cut to those entries. Each file is read once, BLAKE3-checked against the store and bound with its stored names. A base would read all of `HEAD`'s code, so `--node` takes none (`verify-task.md` P2).

**JSON**: `node` and `symbol` after `task`, `omitted` last (`TOP_KEYS` 10 -> 13); `null` in every other mode. No row key is added (Q29).

## Acceptance criteria

**rust-developer** (M = the mutation that must turn the named test red; M1-M7, all in the CLI's new `tests/verify_node.rs`):

- [x] RD-1 -- forms. `verify_node_forms_are_usage`:
  - `--node X` with `--task T`, `--changed`, `--staged` or `--tests`, and `--symbol Q` alone, each exit 2 with stdout empty, `--json` too, and stderr naming both flags.
  - The library's four refusals give their exact lines.
  - M1: one of these forms accepted.
- [x] RD-2 -- rows. `verify_node_rows_equal_plain_rows_filtered`, over `golden` with drift edits.
  - `--node PIECE-COUNTER` holds none of its sections' rows (LAW-BOUND, LAW-STEP).
  - `--node docs/spec/counter.md` gives the same rows.
  - `--node LAW-BOUND` keeps a lock-only row, a row reached through an alias REF and a look-alike marker's row (under its fix's node).
  - Every kept row equals plain's row with the same key, `levels` and `anchor` included.
  - M2: a section's row kept, or a lock-only, alias or look-alike row dropped.
- [x] RD-3 -- counts. `verify_node_counts_its_rows_only`, under `[verify] mode = "enforce"`, with another node's Fix row and the node's rows Check or `ok`.
  - Plain exits 1; `--node` exits 0.
  - The counts are the kept rows', and `skipped` is 0.
  - The summary starts `verify --node <REF>: `.
  - The text prints the `ok` rows as `- | ok | …`, and a moved `ok` row with its `moved from` line.
  - M3: counts, `delta`, verdict or exit taken over plain's rows.
- [x] RD-4 -- REF. `verify_node_naming_nothing_is_exit_2`.
  - An unknown ID, a look-alike (its Latin fix), `other:ID` and an unindexed `.md` path each give exit 2, stdout empty, `--json` too, and `spec: …` on stderr.
  - A REF with two holders gives one `warning:` line and both holders' rows.
  - M4: a REF naming nothing at exit 1, or run as plain.
- [x] RD-5 -- `--symbol`. `verify_symbol_is_byte_exact`.
  - The exact qpath keeps its rows.
  - A suffix or a bare name keeps none: `clean`, plus the note.
  - A RON row matches by `root.cap` and by `data/limits.ron#root.cap`.
  - A moved row matches by its old key and by its new one.
  - M5: a suffix or tier match, or a moved row matched only on its new key.
- [x] RD-6 -- freshness. `verify_node_is_fresh`.
  - (a) After `spec index`, a new `.rs` file with a marker of the node: its `unlocked` row shows with no second `spec index`.
  - (b) The code index's level digest columns tampered in the scratch data directory: `--node`'s rows are unchanged.
  - M6: the code refresh skipped.
  - M7: the index's level columns read.

Pins, no mutation counted:
- `verify_output.rs` `verify_json_every_key_present` with `verify_common` `TOP_KEYS` 13; `verify_task.rs`' `TOP_KEYS` asserts follow; plain JSON gives `node`, `symbol` and `omitted` as `null`.
- `verify_task.rs`' `VerifyRequest` literals gain `node: None, symbol: None`.
- `code_index_fresh.rs` `verify_leaves_the_code_index_untouched` stays green.
- `verify_node_cut_names_narrowing`: a node whose rows pass 40 000 characters gives the tail, `omitted.rows` = k and the JSON `rows` = the printed ones. M: `--node` left uncut.

**test-engineer**:

- [x] TE-1 -- differential. `verify_node.rs` `verify_node_equals_plain_on_every_corpus` runs every node of each corpus below. For each, `--node`'s rows, counts, cascade, `delta`, verdict, exit and `cannot` equal plain's kept by "Rows" (not `notes`). Across the corpora, every state, category and anchor state is met. The corpora:
  - `verify_common::golden`, with drift edits;
  - `fixtures/anchor-states`, all 18 scenarios;
  - `fixtures/verify-tests`: a document, an alias, a `slug/ID`;
  - `verify_common::ecs`;
  - a `duplicate` twin across two files;
  - a walk gap, which gives the same `cannot:` line, `rows` `[]` and exit 2.
- [x] TE-3 (CLI half) -- the single door. `verify_node_writes_nothing_under_the_root`, on the `code-a` git repository with the golden lock. Run `--node` valid, with zero rows, naming nothing, and as usage. After them these are all unchanged:
  - `git status --porcelain=v1 -z --untracked-files=all`;
  - the `.git/index` mtime;
  - the `spec.lock` bytes;
  - the queue rows and events.
- [x] TE-4 -- cost. `verify_node_cost`, `#[ignore]`, release: `code-a` plus 1 000 unrelated `.rs` files, warm index. Record the median of five warm runs of `--node` and of plain in "Implementation". `--node` must be below plain.
- [ ] TE-5 -- the owner's pilots. `verify_pilot.rs` gains `pilot_verify_node_latency`: `#[ignore]`, release, run in the foreground on a scratch clone, as `pilot_verify_counts` does. It measures warm `--node` per sampled marker ID against 1 s, with plain alongside, and prints no pilot name or path. It decides 6c. The MCP run is 6b-2b's.

**spec-writer**:

- [x] SW-1 -- this spec, refreshed and split 2026-10-21: `check-binding.md`; RM-0005's body; 08 s2.
- [x] SW-2 -- at shipping (2026-10-21), bytes before -> after (budget; caps: Tier 2 12 288, Tier 1 10 240, Tier 0 16 384): `spec-verify-scope.md` 2 493 -> 5 996 (6 000: the opening reworded, `## --node`, its known limits); `spec-verify.md` 12 156 -> 12 212 (12 250: the opening's form, "Cascade"); `spec-verify-output.md` 10 610 -> 11 097 (11 100: "Text" the prefix, `ok` rows, the tail; "JSON" `node`, `symbol`, `omitted`); `symbol-index.md` 12 284 -> 12 272 (net ≤ 0); `architecture-code.md` 12 128 -> 12 215 (12 218: "As built (6b-2a)"); store README 10 118 -> 10 221 (`code_selection`, `CodeSelection`, `SelectedFile`); CLI README 10 173 -> 10 174 (10 190; `DriftDocument` left to its canon); `CLAUDE.md` 6 329 -> 6 344; 07 s1.2 :38, s2; 08 s2 :41, :42, :63; `check-binding.md`'s two pointers re-pointed at the canon; this spec compacted.
- [x] SW-3 -- no ADR; `export index` and `check` clean.

## Decided

| Q | Answer (else: the other answer's cost) |
|---|---|
| 5 | twin of plain verify kept to one node (else: a judge of its own drifts) |
| 29 row keys | none (else: every verify row and stored drift item wider) |
| 30, Q-A files | the index selects (else: a whole walk per call, MCP 1 s; 7a needs the path anyway) |
| 32 canon home | `## --node` in `spec-verify-scope.md` (8a-2's Q4) |
| 33 fixtures | `code-a` with the format-2 golden, no new fixture |
| 34 moved `ok` rows | their `moved from` line under `--node` (5c prints it under non-`ok` rows only) |
| Q-B split | 6b-2a CLI, 6b-2b MCP and plugin, one item (else: a third iteration at risk) |
| Q-E REF order | as T: after discovery, exit 2, stdout empty (else: a bad REF answers a report in one config, a bare error in another) |
| Q-F unknown framework | cause (1), a named divergence (else: bypass `refresh_code`'s one reader of the key) |
| Q-G `ok` rows printed | kept (else: the JSON outgrows the text's bound, or a second cut rule) |

Q-C, Q-D and Q-H are 6b-2b's.

## Out of scope

`check_binding`, `INSTRUCTIONS` and the plugin (6b-2b `check-binding.md`); `--node --changed` and any base; tiers on `--symbol`; HTTP and UI; `delta.symbols` (7b); `tests_reaching` and `sync` (7a); writing markers.

## Later

The review's nits left (cost: TE-4's and TE-5's figures decide). Each call builds `Corpus` and `NodeSet::of` twice. `code_selection` reads every file, marker and mention through `refs_in`, yet its one caller never uses `CodeSelection.refs`: reuse the first corpus and leave mentions out if MCP's 1 s is tight. A Rust grammar failing to load in `partial_tree` reads `the code walk is incomplete: the Rust grammar cannot be loaded: …`, where plain has no prefix; it is unreachable, since `refresh_code` fails first with cause (8). A `PartialGap` variant of its own would keep plain's wording.

## Implementation

| Module | What it does |
|---|---|
| CLI `src/verify_node.rs` (new) | `verify_node`: REF (`classify`, `indexed_in`, `target_nodes`; holders sorted, deduplicated), the note, the warning; `plain_reads`' causes plus `known_frameworks` (Q-F); `refreshed_code`; `judge_node`: `code_selection`, `cut_lock`, `fresh_tree` (BLAKE3 recheck, one more refresh, cause (3)), plain's judge, `row_set_nodes`; `on_symbol`; the zero-rows note |
| CLI `src/verify.rs` | `VerifyRequest.{node, symbol}`, `VerifyNode`, `VerifyOutcome.node`; the library refusals; `plain_reads` and `row_set_nodes` (shared with `--task`) extracted, unchanged; `printed` (the cut), `row_block`, `ok` rows, the prefix; JSON `node`, `symbol`, `omitted` |
| CLI `src/code_walk.rs` | `partial_tree(files, names)`, `PartialGap {Walk, Names}` |
| CLI `src/main.rs`, `src/lib.rs` | the flags (`conflicts_with_all`, `requires`); `VerifyNode` re-exported |
| store `src/code.rs`, `src/lib.rs` | `code_selection(select) -> Option<CodeSelection {refs, files}>`, one read transaction; `SelectedFile {path, names, holds(bytes)}`; `stored_names` over the shared `selected_in` |
| tests | new CLI `verify_node.rs`; `verify_common/mod.rs` (`TOP_KEYS` 13), `verify_output.rs`, `verify_task.rs`, `verify_pilot.rs` (TE-5), `layer_c_scan.rs` (R10's `LATER` list) |

Divergences, kept (owner pre-approved, review open):
- REF is trimmed in the `@rev` note, the prefix and `VerifyNode.reference`.
- Holders are sorted and deduplicated by (path, position).
- The cut counts characters before escaping; one block over 40 000 prints zero rows and the tail.
- Missing stored names, a count not one per fresh item or an unknown ambiguity word read as "changed": one more refresh, then cause (3).
- The frameworks cause (1) prints after a `[verify]` cause (1); plain puts `[code]` first.
- `--symbol Q --changed`: clap's `requires` does not fire; the library's refusal does (exit 2, stdout empty).

After the tests, a micro developer pass (review nits, no behaviour): the `verify()` doc names `--node`'s REF and index; `CAUSE_UNMERGED` private again.

Tests: nextest core, code, CLI, MCP, HTTP, store, model, import 3 175 passed, 14 skipped; eval `build_graph`, `doc_pointers`, `anonymity` 58 passed; clippy `--all-targets -D warnings`, fmt clean. `verify_node.rs`, oracle plain verify: TE-1 (golden in two states, `anchor-states` 18, `verify-tests`, ecs, a walk gap) meets 9 states, 4 anchor states, 16 categories; TE-3 nothing written. M1-M7 killed by their named tests, variants too. TE-4, release, median of 5 warm runs, three runs: plain 65/61/61 ms, `--node LAW-BOUND` 51/51/47 ms. Uncovered: a second BLAKE3 mismatch (racy), cause (8) faults, `--root`/`--config`, two-root determinism, some categories; a moved RON row cannot occur (RON is never followed).
