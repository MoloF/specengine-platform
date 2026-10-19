---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5b-1 (WA-1..WA-13; Q1-Q8 2026-10-12; split 2026-10-16, Q9-Q21; refreshed against 34b17b2 and split 2026-10-20, S1-S15, Q22-Q31, this file 5b-1a; decided under the owner's pre-approval, review open); 05 s3.5, s5.2; 07 s2; 08 s2, AC-2, AC-14
adrs: [ADR-0050]
---

# lock-levels (5b-1a): spec.lock format 2, its writers and readers

## Why

Format 1 (`docs/canon/spec-lock.md` "Entry") keeps one v2 `ast_hash` per item and only its file's blob for a `module` or `ron` binding: declared levels are dead, and any edit of a RON file or a module's file is drift for everything bound in it (05:137). Verify can judge by levels only once the lock holds them; the bytes freeze at the first format-2 lock (ADR-0050).

Delivered: `format = 2` (levels, every level's digest, 5a's fingerprints; no `ast_hash`), written by accept, `--editorial`, a drift approve, regen and the editorial approve, read by every reader; verify on format 2 judging as v2 did; drift evidence over the digests; format 1 read where only its spec side counts, refused elsewhere by one text naming `spec lock rebase` (Q10, Q11, Q27). **5b-1** names this slice (5b-1a) and 5b-1b `lock-levels-verify.md` (verify by levels; Q22); **5b** names both and 5b-2 `lock-rebase.md`. Order (Q22, Q23, 08 s2): this, 5b-2 at once (closing the window: "Readers"), 5b-1b, the carry-overs' XS slice, 5c. The gate is met ("Dependencies and ordering").

## Description and interactions

- **The owner** meets a format-1 lock, or one of another `code_recipe`, as exit 2 from verify (every mode), accept, `spec propose drift`, a drift approve, regen and an editorial approve that would refresh an entry, the message naming the way out (Q11) that 5b-2 ships; `spec check`, `spec bump` and the review's vouching count keep reading it. **An agent** sees no new state, detail or JSON key; an item edit v2 saw but `sig` and `body` do not (N5's, N6's reorders, a CRLF checkout's CR: `code-hash.md` "Levels") now reads `ok`; `report_drift`'s evidence carries the new `lock_code` and an item's `code` per changed level.
- **Crates** (WA-12: core sees digest strings, the four level names, `body_bytes`; `build_graph.rs` `core_and_store_have_no_tree_sitter` guards it). Core `lock.rs`: format 2 in `parse_lock`, `render_lock`, `same_pair` and the merges (`merge_accept`, `merge_stages`, `refresh_editorial`, `drop_node`); `Lock` knows its format; the texts; `lock_code`; its own copy of the level names. Core `drift.rs` `HashedEntry.code`'s doc (Q12); `verify.rs` `Verified`'s new shape, no comparison in core (5b-1b). Code (S1, Q24): one public function; `lib.rs`'s "Nothing records these digests yet"; no digest byte, `LEVEL_RECIPE` or `CODE_INDEX_FORMAT` moves. CLI `lock.rs`: accept through `item_levels`, `module_levels`, `ron::value_hashes`, `build_recipes` on `LEVEL_RECIPE`; S4: `Target::Module` gains an inline `mod`'s item index, `node_markers` keeps each marker's levels, a `ron` binding pairs with its value hash by the comment's range. CLI `verify_rows.rs` (the item level through `item_levels`, no `hash_bound_items`), `verify.rs`, `drift.rs` (`code: entry.ast_hash` -> `lock_code`), `lock_regen.rs` (Q28), `revisions.rs` (`step_3_lock`, Q29). Store, MCP, HTTP, UI, plugin, eval code unchanged; no new crate or dependency.
- **`verify --tests`** (S3): "Verifiable" is accept's predicate (CLI `committed_bindings`; `spec-verify-tests.md` "Verifiable"), so accept's new refusals (`bad_levels`, a module file with a parse error, a rejected RON file) make such a node `not verifiable <state>`. The canon stays true; TE-3 pins it.
- **Unchanged**: the gate, the index, the queue's schema and store (no row gains `levels` before 5b-1b), `daemon-keys.json` (`DriftEvidence`'s keys only; MCP `DriftShape.rows` untyped). **Nothing is blocked** (ADR-0012): `spec check` reads the lock only through the rule `rev-not-raised` with `None`, which reads format 1; the gate and the reads never read it (`health_code.rs` reads no lock). Only a project's `[verify]` CI or hook stops, on a format-1 lock; none exists (Q23).
- **Overridden at shipping** (shipped canon is not edited while built): SW-2's sections. "Base", cause (7), "A project's hook" are 5b-2's; "States", "Known limits"' module and RON CRLF half, JSON `levels` 5b-1b's.

## Data

### Format 2

WA-3..WA-6.

```toml
# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit
format = 2
norm_recipe = "specengine-norm/v1"
code_recipe = "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15"

[[binding]]
node = "R-12"
rev = 3
spec_hash = "b3:9f2c<60 hex>"
relation = "implements"
file = "crates/game/src/stamina.rs"
kind = "item"
symbol = "game::stamina::regen_system"
item_kind = "function_item"
levels = ["sig"]
sig_hash = "b3:41aa<60 hex>"
body_hash = "b3:07c3<60 hex>"
deps_hash = "b3:d1e0<60 hex>"
skeleton_hash = "b3:5b92<60 hex>"
quote_hash = "b3:a8f4<60 hex>"
body_bytes = 212
file_blob = "5f1c9e<34 hex>"
commit = "a0cea48<33 hex>"
verified_by = "owner"
verified_at = "2026-10-10"
```

- Header: `code_recipe` = `<LEVEL_RECIPE> tree-sitter-rust/<version> abi/<n>`; `norm_recipe`, the comment line and the entry key (core `EntryKey`; WA-3) unchanged.
- Keys in this order (WA-4): `node, rev, spec_hash, relation, file, kind, symbol, item_kind?, levels, sig_hash, body_hash, deps_hash, skeleton_hash?, quote_hash?, body_bytes?, file_blob, commit, verified_by, verified_at`.

| Key | Value |
|---|---|
| `levels` | non-empty, distinct, canonical order `path, sig, body, deps` (`code-identity.md` "Marker grammar"): the union of the effective levels of the markers giving the key, `Default` as `sig, body` |
| `sig_hash`, `body_hash`, `deps_hash` | every kind, every level, in `levels` or not: `item` by `item_levels`; `module` by `module_levels`, an inline `mod`'s `mod_item` by `item_levels` (`code-hash.md` "File module"); `ron` sig = body = the value hash, deps the empty set's digest (Q24; `code-hash-ron.md` "RON value") |
| `skeleton_hash`, `quote_hash`, `body_bytes` | `item` and `module` only; `body_bytes` a TOML integer, 0 to 4 294 967 295; stored for 5c, never compared, never drift evidence (WA-13) |
| `ast_hash` | gone: the default `[sig, body]` sees a change iff v2 does, N5, N6 and the CR rule aside (`code-hash.md` "Levels") |

- Bytes as format 1 (`spec-lock.md` "The file"), plus `levels = ["sig", "body"]` (basic strings, `", "` between, no trailing comma) and `body_bytes` in decimal (no sign, `_` or leading zero).

### Readers

Q10, Q11, Q27 (S7, S8). `parse_lock(text, build)`:

- **`None`**: formats 1 and 2 in full, each by its own rules, recipes never compared. Format 1: `spec-lock.md` "Entry" (`ast_hash` iff `item`; `levels`, digests unknown keys). Format 2: format 1's rules for shared keys; `levels` as above, else `spec.lock:<line>: levels: <empty|unknown <name>|duplicate <name>|order>`; the three digests always; fingerprints iff `item` or `module`; `ast_hash` unknown. Another `format`: `format <n> is newer than this build's 2` (n > 2), `format <n> is not this build's: it reads formats 1 and 2` (n < 1).
- **`Some(build)`**: format 2 at this build's recipes only, checked in this order (Q27; today's order: header, recipes, entries), the first failing one the error, as `LockError` renders it (`spec.lock:<line>: …`): (1) unreadable (the TOML, `format`, a recipe key) or `format` neither 1 nor 2, as under `None`; (2) another `norm_recipe`: today's text, no step (no rebase across it: ADR-0050); (3) format 1: `format 1 ("<its code_recipe>"), this build's is 2 ("<build's>"): run spec lock rebase on a terminal`; (4) another `code_recipe`: `` `code_recipe` is "<its>", this build's is "<build's>": run spec lock rebase on a terminal ``; (5) the entries.
- No reader upgrades a lock in memory (WA-5); no format-1 lock is rendered: a writer reading `None` refuses format 1 before rendering. Verify keeps "a run cause gives no hint" (`spec-verify-output.md` "Hints"): the step is the cause's text.

| Caller | Reads | Format 1 | Format 2, another `code_recipe` |
|---|---|---|---|
| rule `rev-not-raised` (core `check/rev_rule.rs` `rule_lock`, via store `check.rs`), `spec bump` | `None` | read | read |
| the review's vouching count (CLI `lock_for_review`: `spec review`, MCP `get_proposal`, HTTP's review) | `None` | counted | counted |
| `spec verify`, every mode (`read_lock`, `read_commit`, `read_head`) | `Some` | cause (6); `HEAD`'s lock (7) | the same |
| accept, `--editorial`, a gone node | `Some` | exit 2, the text | the same |
| `spec propose drift`, MCP `report_drift` (`read_commit`); a drift approve (accept) | `Some` | exit 2, the text | the same |
| `spec approve --editorial` (`step_3_lock`, Q29) | `None` | exit 2 before rendering, the format-1 text, only if an entry would be refreshed; else untouched | refreshed: code fields, `code_recipe` kept |
| `spec lock regen` (Q28) | `None`, per stage | exit 2, regen's text | merged as today when every stage is format 2 of one `code_recipe`; else exit 2, regen's text |
| `spec lock rebase` (5b-2) | `None` | its input | its input |

- **Regen's text** (Q28), for a format-1 stage and for stages of two `code_recipe`s: `` spec.lock: a stage is format 1, or the stages differ in `code_recipe`: run git merge --abort, spec lock rebase on the lagging branch, and merge again ``.
- A `HEAD` lock of another format or recipe beside a judged lock of this build's stays cause (7) until 5b-2's base rule (`lock-rebase.md` "The base").

### Accept

`spec-lock-accept.md` "Steps" 5-6 under format 2. Step 5 hashes each lockable binding from its `HEAD` blob: `item` by `item_levels`, `module` by `module_levels` (an inline `mod` by its `mod_item`'s `item_levels`), `ron` by `ron::value_hashes`, keyed by the comment's range (`code-hash-ron.md` "Entry point"). **Lockability** (`spec-lock.md`), rows in this order, the first refusal in marker order naming one:

| Row | Locked | Never |
|---|---|---|
| `id_state`, `rev_state` | as format 1 | as format 1 |
| `levels` (new) | `Default`, a declared list | `Invalid` (`unclosed`, `unknown`, `duplicate`, `empty`): `not locked: bad_levels` |
| binding | `item`, `module` with levels hashed; `ron` with a value hash | `ambiguous <reason>`, `unanchored`, `cannot_verify`: format 1's, plus a `module` whose file has any parse error, a `ron` value of a file `RonAnalysis` rejects (Q31) |

A refused binding with an entry keeps it (`kept`). **Merge**: `same_pair` (core `lock.rs`) compares `rev`, `spec_hash`, `levels`, the three digests and `file_blob`, never a fingerprint or `body_bytes` (Q12); equal is `unchanged`, byte-identical; a changed `deps_hash` alone refreshes. `--editorial` and 4b-1's editorial apply keep every code field byte for byte.

### Verify on format 2

All that changes in `spec verify` (`spec-verify.md` "Cascade"). E is a format-2 entry. Level 1, the blob, unchanged. Level 3, an `item` whose blob changed: its file re-parsed from the walk's text, its levels by `item_levels`, `sig` and `body` against E's `sig_hash`, `body_hash` whatever the levels; either differing: `code_changed`, no detail; `CannotVerify`: `cannot_verify item_error`, as today. `module`, `ron` rows: by blob, as today. `compared` keeps `blob`, `ast`, null; no new key.

### Drift evidence

Q7, Q12, Q13 (S9). `drift_hash` keeps `DRIFT_HASH_DOMAIN` and its 18 places (`proposal-drift-item.md` "The item"); two change:

- `lock_code` (core `HashedEntry.code`), every kind: the entry's `levels`, `sig_hash`, `body_hash`, `deps_hash` lines as `render_lock` writes them, LF-joined, no final LF (a core function beside `render_block`, working name `lock_code`); never a fingerprint or `body_bytes`, so tuning 5c's fingerprints re-keys no item.
- `code` (`HashedRow.code`), an `item` row's: `<level>:<digest>` per changed level of `sig`, `body`, this run's digest, `,`-joined in canonical order (`body:b3:07c3…`); null when equal by blob, `cannot_verify`, not compared. A `module` or `ron` row's stays its blob OID until 5b-1b.
- Stored rows unchanged (Q25). An item raised before the project's rebase meets new evidence once: an open one `evidence changed since it was raised`, a rejected one raised again (`proposal-drift.md` "Known limits").

### The empty set's `deps`

Q24 (S1). `specengine-code` gains one public function, working name `empty_deps_digest() -> Digest` (`levels.rs`, re-exported at the root): crate-private `deps_digest` over no target, `level_header() ‖ put("deps")` (`code-hash-ron.md` "RON value"). A `ron` entry's `deps_hash` renders it. No pinned constant: `level_header()` carries tree-sitter-rust's version and ABI.

### Fixtures and the golden

S10. `fixtures/spec-lock/code-a.lock` becomes accept's format-2 golden, regenerated by `lock_accept.rs`' `GOLDEN_REFS` accepts on `fixtures/code-a/`, digests never hand-written: 9 entries, `PIECE-COUNTER` 4, `LAW-BOUND` 4, `GOAL-01` 1 (`counter.rs:77` `[path, path]`: `bad_levels`). `cap-check/PASS-01`'s only binding, `bounds.rs:14` `[Sig]`, is refused: its accept exits 1, no commit. Of 3 committing accepts, `PIECE-COUNTER`'s 4 entries keep `commit` `2ba8a3b…`; `LAW-BOUND`'s 4 and `GOAL-01`'s 1 change (format 1: `82f6593…`, `85a2c98…`); `verified_at` stays `2026-10-08` (the tests' `DAY`). `GOLDEN_REFS` and `golden_accepts_ask_and_commit_once_each` follow. Today's bytes move verbatim to `fixtures/spec-lock/code-a.format-1.lock`: the format-1 readers' evidence, 5b-2's input (Q30). `rev_rule_corpora.rs`' format-1 locks stay (Q10's evidence). No other fixture holds a lock or digests; `daemon-keys.json`, `code-a/expected.json` unchanged.

### Carry-overs

Moved out of 5b-1 (Q22): listed once, in `lock-levels-verify.md` "Later".

## Rules and edge cases

- WHEN a reader needs only a lock's spec side, it SHALL read formats 1 and 2; every writer and code judge SHALL refuse format 1 and another `code_recipe` with its one text, in Q27's order; a writer SHALL refuse before rendering, and no reader SHALL upgrade a lock in memory (Q10, Q11).
- WHEN only a fingerprint or `body_bytes` differs, no `same_pair` outcome, verify row or `drift_hash` SHALL change (Q12).
- WHEN a binding's markers carry `Invalid` levels, accept SHALL NOT lock it and `verify --tests` SHALL read its node `not verifiable bad_levels` (S3).
- WHEN verify judges a format-2 item, it SHALL compare `sig` and `body` whatever the levels (5b-1b narrows it).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, accept SHALL give byte-identical locks and output.
- **The pin** (Q16, S6) `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`), per file: `lock.rs` calls `item_levels(` and `module_levels(`, never `hash_bound_items(`, `hash_item(`, `hash_node(`, names no `HashState`, reads no `.state` but a judgement's, a level list's or a row's. `verify_rows.rs` joins in 5b-1b (Q26), `lock_rebase.rs` in 5b-2.
- No stack, kind or prefix in rows, texts or the lock (WA-12).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red; M1-M5 here, M6-M8 in 5b-1b, 8 in all):

- [ ] RD-1 -- "Format 2", core `tests/lock_model.rs`: format-2 bytes and key order; `levels` canonical (`levels_out_of_canonical_order_refused`); the three digests for every kind; fingerprints iff `item` or `module`; `body_bytes` a decimal u32; format 1 under `None` (`ast_hash` iff `item`); `format = 3` newer, `format = 0` not this build's; `lock_code` the `levels` and three digest lines (`lock_code_has_no_fingerprint`). **M1** `ast_hash` accepted in format 2 -> `format_2_refuses_ast_hash`. **M2** a digest ignored by `same_pair` -> `changed_deps_digest_refreshes`.
- [ ] RD-2 -- "Readers". **M3** `parse_lock(None)` refusing format 1 -> `format_1_head_lock_is_read` (CLI `rev_rule.rs`; 4b-2's carry-over, Q18: `HEAD` holds `code-a.format-1.lock`, a staged spec change without a raise is found `rev-not-raised`, not cannot-check). **M4** one caller without the step -> `format_1_text_is_the_same_everywhere`, new CLI `tests/lock_format.rs` (every refusing caller; MCP `report_drift`'s case in `mcp_drift.rs`).
- [ ] RD-3 -- "Accept", CLI `lock_accept.rs`: the format-2 golden byte for byte from its 3 committing accepts; `levels` the markers' union (`levels_are_the_union_of_the_markers`); a `module` whose file has a parse error, a `ron` value of a rejected file -> `not locked: cannot_verify <category>`, the old entry kept. **M5** `Invalid` locked as the default -> `invalid_levels_are_not_locked`.
- [ ] RD-4 -- genre, graph, the pin, no mutation run: `code_refs_walk.rs` as "Rules"' pin; `build_graph.rs` green (`core_and_store_have_no_tree_sitter`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`); `lock_level_names_match_the_marker_grammar` (`lock_format.rs`) against code `Level::ALL`.

test-engineer:

- [ ] TE-1 -- `lock_format.rs`: each refusing caller × {format 1, another `code_recipe`, another `norm_recipe`, `format = 3`}: exit 2, the text of Q27's first failing check; `git status --porcelain=v1 -z`, `HEAD`, the lock's bytes unchanged. The `None` readers (the rule through `spec check --staged`, `spec bump`, `spec review`, MCP `get_proposal`) read format 1. Regen: a format-1 stage, stages of two `code_recipe`s give Q28's text; an editorial approve refreshing no entry passes over a format-1 lock, one refreshing exits 2 (Q29).
- [ ] TE-2 -- accept determinism (`lock_accept.rs`): byte-identical lock, stdout, stderr from two roots, another cwd, `core.autocrlf=true`, the lock's entries reversed.
- [ ] TE-3 -- `verify_tests_select.rs`: a node whose only `@verifies` marker is `[Sig]` reads `not verifiable bad_levels` (S3).
- [ ] TE-4 -- items under format 2: AC-14 (`verify_cannot.rs` on the new golden: two different broken bound items, never compared, exit 2 in every mode); a body edit's drift item stores `code` `body:<digest>` (`drift_raise.rs` `item_code_names_its_changed_levels`).

spec-writer:

- [x] SW-1 -- this spec (2026-10-12, ADR-0050 and its canon); split 2026-10-16 (Q21); refreshed (S1-S15) and split 2026-10-20 (Q22): `lock-levels-verify.md`, `lock-rebase.md` (Q30), the rebase pointers in `architecture-code.md` "As built" and `symbol-names.md`, 05 s5.2, 08 s2, the neighbours' order lines; no new ADR (ADR-0041, ADR-0042, ADR-0045, ADR-0050 hold); `export index`, `check` clean.
- [ ] SW-2 -- at shipping; bytes at `34b17b2`, caps Tier 2 12 288, Tier 1 10 240. Also 05 s3.5, s5.2; 08 s2, AC-14; `CLAUDE.md` State (5 970 of 16 384). No new canon file.

| File | Bytes | Changes | Budget |
|---|---|---|---|
| `spec-lock.md` | 7 426 | "The file", "Entry", "Lockability", `regen` (Q28), `:11`'s writers, "Known limits"' grammar line | ≤ +2.5 K; ≥ 2 K left for 8b |
| `spec-lock-accept.md` | 11 237 | "Steps" 5-6, "Exits" | ≤ +1.0 K |
| `spec-verify.md` | 11 745 | E's fields, cause (6), "Cascade" level 3, "Known limits"' CRLF item half | net ≤ 0 (5b-2's "Base") |
| `proposal-revisions.md` | 10 710 | "Editorial approve": Q29 | ≤ +150 B |
| `proposal-drift-item.md` | 4 095 | "The item": `lock_code`, an item's `code` | — |
| `spec-check-git.md` | 11 550 | `:44` "Format 1 is read; 5b adds 2" | net ≤ 0 |
| `code-hash.md` | 12 228 | `:11`, "Entry points" (Q24) | net ≤ 0 |
| `code-hash-ron.md` | 4 319 | `:11` "Nothing records it yet", `:33` | — |
| `architecture-code.md` | 11 566 | "As built" `#spec-lock`, `#verify` | ≤ +300 B |
| core, CLI, code READMEs | 10 139, 9 934, 9 780 | pointers; Q24's function | net ≤ 0, ≤ +100 B, ≤ +300 B |

## Risks

- **Frozen bytes**: the first format-2 lock freezes 5a's bytes; a later change costs a `LEVEL_RECIPE` bump, a rebase of every lock, every rejected drift item back once. TE-1 has passed (Q31). The RON lexer is not in `code_recipe`: a lexer change must bump `LEVEL_RECIPE`; 5a-1b's stream pin and the golden's RON digests catch it.
- **The window** (Q23): a format-1 lock stops verify and every writer until its rebase commit; none exists, 5b-2 follows at once.
- **Hidden churn** (S11, "Files"): the test-engineer starts on day 1 with `lock_common`'s format-2 `block` and the golden; the developer lands core `lock.rs` first.
- **8a-1's verdicts** (S3): TE-3. **Accept's new refusals**: on the pilots 0 of 386 module files; RON 67 of 6 301 anchors on pilot A, none bound; 0 on pilot B.
- **Caps**: `code-hash.md` 60 B left, `node-revision.md` 5 B, the core README 101 B, `spec-verify.md` 543 B shared with 5b-2 and 5b-1b: net ≤ 0 edits; `spec-verify-base.md` as `lock-rebase.md` plans.
- **Regen**: Q28 avoids a text loop. A base-only mismatch (both sides rebased since the merge base) exits 2 too: take one side's lock (`git checkout --ours spec.lock`), accept the other side's nodes after the merge (regen across formats: `lock-rebase.md` "Out of scope").
- Entries grow about 1.8x.

## Out of scope

Verify by levels, `module` and `ron` by digest, JSON and stored `levels`, a `module` or `ron` `code` without a blob OID, `verify_rows.rs`' pin (5b-1b); `spec lock rebase`, the base rule (5b-2); the carry-overs; 5c's anchors (`anchor-states.md`); `--deep`; reads' `sync` (7a); Health's drift row (9b); an MCP tool, UI or plugin command; 8b's format 3 (`tested-evidence.md`).

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q22 5b-1's size (owner) | split: 5b-1a (this: format 2, writers, readers, verify on format 2 as v2 judged, `lock_code`, both goldens, the `lock.rs` pin); 5b-1b `lock-levels-verify.md` (verify by levels, no lock byte); 5b-1a -> 5b-2 -> 5b-1b; carry-overs an XS slice after 5b-1b | one L review over about 28 test files; the iterations may run out |
| Q23 the window (owner) | 5b-1a ships alone; 5b-2 right after closes the window. No `spec.lock` is tracked or on disk in this repository or either pilot project (checked read-only 2026-10-20); no 5b-1a binary reaches a project holding one before 5b-2 | one commit doubles the review; a minimal rebase duplicates 5b-2's read by OID and locator |
| Q24 `ron` `deps_hash` | a public function in code `levels.rs`, the empty set's digest, re-exported; `LEVEL_RECIPE`, `CODE_INDEX_FORMAT` unchanged | a pinned constant goes stale at a grammar bump |
| Q25 `DriftRow.levels` (5b-1b) | optional, lenient: any JSON after `compared`, never validated | four tests' array rows (S9) and mixed-build queues read as corrupt |
| Q26 `verify_rows.rs`' pin (5b-1b) | requires `item_levels(`; bans `hash_item(`, `hash_node(`, `hash_bound_items(`; its errored item's `item.state` and `HashState` allowed there only | a code accessor, `unbound()` rewritten |
| Q27 `Some`'s order | unreadable or newer -> `norm_recipe` (no step) -> format 1 -> `code_recipe` | format 1 first advises a rebase that refuses across `norm_recipe` |
| Q28 regen | one text: "git merge --abort, spec lock rebase on the lagging branch, merge again" | the generic text loops with 5b-2's "spec lock regen first" |
| Q29 editorial approve | refuses format 1 only when a refresh would render | every editorial approve refused before a rebase |
| Q30 5b-2's golden | `code-a.rebased.lock`: `code-a.lock` with `code-a.format-1.lock`'s `commit`s, generated, pinned | its RD-2 can never pass |
| Q31 TE-1's policies | kept: a module with any parse error, a whole RON file refused; the fingerprint bytes; AC-15's threshold not stored | a finer policy changes frozen bytes: v4, a rebase |
| Q3 format 1 under 5b | exit 2 where code is judged or a lock written; read where only its spec side counts | a second, v2 judge |
| Q7, Q12 `lock_code` | `levels` and the three digests as rendered, no fingerprint; `same_pair` alike | all code fields: tuning 5c's fingerprints re-keys every item |
| Q9 order (owner) | after 5a-1, 5a-1b, 6a-2 and 5a-2's whole TE-1: met 2026-10-20 | a pilot shortfall costs v4 and a rebase of every lock |
| Q10 format-1 readers | `None` reads formats 1 and 2; `Some(build)` this build's only; `None` writers refuse format 1 before rendering | an unrebased project's spec commits cannot check |
| Q11 the refusal text | the reader's message carries the step, every caller alike | a `hint:` line: an exception to the hint rule, a pin per caller |
| Q13 the `code` fact | `<level>:<digest>` per changed level; items here, every kind in 5b-1b | the blob OID: a neighbour's edit is new evidence |
| Q16 the pin | per file: `lock.rs` here, `verify_rows.rs` (Q26), `lock_rebase.rs` (5b-2) | the cross-check through `hash_item(`: a tree-sitter node in the CLI |
| Q18 4b-2's deferred tests | `format_1_head_lock_is_read` here; the rebase commit through a real hook: 5b-2 | a reader change makes unrebased projects cannot-check unseen |

Decided under the owner's pre-approval, review open: Q1-Q8 2026-10-12, Q9-Q21 2026-10-16 (Q21: 5b-2 split off), Q22-Q31 2026-10-20. Q1, Q2, Q4, Q8, Q14, Q15, Q19 live in `lock-rebase.md`; Q5, Q6, Q20 in `lock-levels-verify.md`, Q17 in its "Later". S-n: the 2026-10-20 refresh's stale points; WA-n: the 2026-10-12 analysis' working answers.

## Dependencies and ordering

- **From 5a-1** (`code-hash.md` "Entry points", "File module"): `LEVEL_RECIPE`, `level_header()`, `LevelState`, `LevelHashes`, `item_levels`, `module_levels`, `Digest`; v2's `RECIPE`, `hash_bound_items` (5b-2's cross-check), `grammar_info` kept; `markers.rs` `Level`, `Levels`, `DEFAULT_LEVELS`. **From 5a-1b** (`code-hash-ron.md` "Entry point", "RON value"): `ron::value_hashes`, its whole-file refusal. **From 5a-1c**: N6.
- **Gate, met 2026-10-20**: 5a-2's TE-1 passed on both pilots, figures in `docs/canon/code-hash-eval.md` "Measured" (every judged figure 100.0, 0 unstable rows, levels 4.09 / 4.17 ms per kLOC within 6.67).
- **Order** (08 s2): 5b-1a -> 5b-2 -> 5b-1b -> the carry-overs' XS -> 5c -> 8a-2 -> 6b-2 -> (6c) -> 7a -> 7b -> 8b -> 9b. 4b-1, 4b-2, 4c-1 read the lock ("Readers"); 4c-2 does not interact.

## Files

- **rust-developer**: core `src/lock.rs`, `src/drift.rs`, `src/verify.rs`; code `src/levels.rs`, `src/lib.rs`; CLI `src/lock.rs`, `src/verify_rows.rs`, `src/verify.rs`, `src/drift.rs`, `src/lock_regen.rs`, `src/revisions.rs`.
- **test-engineer** (S11). Golden readers: CLI `verify_common/mod.rs`, `drift_common/mod.rs`, `rev_rule_common/mod.rs`, `drift_raise.rs`, `drift_approve.rs`, `rev_rule.rs`, `rev_rule_hook.rs`, `rev_editorial.rs`, `lock_accept.rs`, `verify_rows.rs` `#[cfg(test)]`; MCP `mcp_drift.rs`. Format-1 helpers and keys: CLI `lock_common/mod.rs` (`PREAMBLE`, `ENTRY_KEYS`, `block`), `lock_regen.rs` (24 helper sites), `lock_exits.rs`, `lock_commit.rs`, `symbol_twins.rs` (pins `ast_hash`); core `lock_model.rs`, `verify_judge.rs`. Verify: `verify_states.rs`, `verify_cascade.rs`, `verify_modes.rs`, `verify_staged.rs`, `verify_cannot.rs`, `verify_tests_select.rs`; `code_refs_walk.rs`; new `lock_format.rs`. Fixtures: `spec-lock/code-a.lock` regenerated, new `spec-lock/code-a.format-1.lock`. Re-run only (they commit the golden as it is): CLI `health_rows/mod.rs`, HTTP `drift.rs`, `daemon_keys.rs`, MCP `mcp_code.rs`, store `queue_drift.rs`.
- **spec-writer**: SW-2.

## Implementation

Not started. 2026-10-16 (Q21): the rebase's criteria to `lock-rebase.md`. 2026-10-20 (Q22): RD-1 <- RD-1 and RD-3's `same_pair` mutation; RD-2, RD-3 kept; RD-4 <- RD-10's pins; TE-1 <- RD-2's refusals; TE-2 <- TE-7's accept half; TE-3 new; TE-4 <- TE-3's items; RD-4 (verify by levels), RD-11, TE-2 (the matrix), TE-3's `module` and `ron` half, TE-7's verify half -> `lock-levels-verify.md`, RD-12 -> its "Later". To record: modules, deviations with reasons.
