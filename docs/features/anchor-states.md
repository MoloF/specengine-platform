---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5c (WA-1..WA-8; Q1-Q6 2026-10-12; Q7-Q16 2026-10-16); refreshed 2026-10-20 against 55709ac, 5b-1a, 5b-2 and 5b-1b shipped (S1-S20, Q17-Q23, the R(b) rule, M1-M8); all decided under the owner's pre-approval, review open; Q1-Q16's alternatives in this file at 55709ac; 05 s5.2, s5.3; 08 s2, AC-15
adrs: []
---

# anchor-states: a renamed or moved binding judged against its old lock entry

## Why

Today (`docs/canon/spec-verify.md` "States") a function renamed or moved with its marker gives two rows: the old key `broken_binding symbol_gone|file_gone` (Fix), the new key `unlocked` (Check). Under `enforce-introduced` a pure rename exits 1, on disk and in the advised `--staged` hook alike; `spec propose drift` raises a `high` item; the agent gets a Fix ("broken by the change", ADR-0042) for a refactor that broke nothing; the owner cannot tell a rename from a deletion plus new code. A manifest change renames qpaths with no code edit (`code-identity.md` "Known limits"), and so does an in-file twin (`symbol-names.md` "Frozen names").

Delivered: a read-only pairing inside verify's one judge, `judge_side`, on every side (Q7). A lock entry whose key no marker gives any more is paired, by fingerprints and one to one, with a marker binding of its node and relation that has no entry; the two become one row at the new key, judged against the old entry by 5b-1b's levels: a pure rename `ok`, a rename with edits `code_ahead code_changed <levels>` or `conflict`, a `[path]` binding `code_changed path`. The anchor answers "where is it", `sync` "does it still match" (05 s5.2). No writer, no new `state`, no lock byte, nothing stored but a drift item's evidence, no ADR: ADR-0020 (only marker bindings are candidates), ADR-0042 (nothing unverifiable is followed or `ok`), ADR-0046; ADR-0041, ADR-0050 stand (accept re-keys); ADR-0012 is helped. Following code with no marker would amend ADR-0020 (12b).

Order (08 s2): 5b-1a, 5b-2, 5b-1b (shipped 2026-10-20) -> **5c** -> 8a-2 -> 6b-2 -> (6c) -> 7a; size M, one slice, at most 3 iterations (Q23).

## Description and interactions

- **An agent** running `spec verify --changed` or a `--staged` hook after a rename sees an `ok` row (`11 ok (1 moved)`), or one Check row with a `moved from` line when it also edited; `report_drift` at a pure-rename commit raises nothing. **The owner** sees the move, re-keys at leisure with `spec lock accept <ID>`, and sees an `ambiguous` Fix row's candidates.
- **Re-keying stays with accept** (WA-2): its merge writes the new key `new`, the vanished one `dropped` (`spec-lock-accept.md` "Steps" 6). For an AMBIGUOUS row accept locks every lockable candidate, or the owner removes a marker first. 5b-2's rebase keeps keys byte for byte, so it never pairs.
- **Crates** (WA-7; core sees keys, digests and `body_bytes` only, ADR-0008): core and CLI, files in "Implementation". **Unchanged**: code (`code-hash.md` "Entry points" exports what 5c reads), store, MCP (drift rows pass untyped), HTTP (`fixtures/daemon-keys.json` pins only `DriftEvidence`'s `hash`, `rows`, `more`), UI (a drift card prints each row as JSON: `proposal-drift.md` "UI"), plugin, the gate, `spec check`, the index, the queue's schema, `spec.lock` (format 2 holds what 5c reads: `spec-lock.md` "Entry"; 8b's format 3 is independent), `specengine.toml`, every existing fixture, `Cargo.toml`.
- **Canon 5c makes false** (S8): SW-2's rows, rewritten at shipping, not while built.
- **Neighbours**: 4c-1 reserved place 18 (`proposal-drift-item.md` "The item", ADR-0045); 7a `bundle-bindings.md` (`anchor`, `moved_from`, through this core pairing, Q13); 7b `impact.md` (Q14); 6b-2 `verify-node.md` (`anchor`, `--symbol` on a moved row's `from`, its Q34) inherits the pairing through `judge_side`, before its filter. 9b, 8b: no edit.

## Data

Only verify's output (`anchor`, `delta.bindings.moved`) and the drift hash's place 18 change.

### The pairing

**Where** (S2, S3): `judge_side` (CLI `verify_rows.rs:313`), called for the disk (`judge_walk`, :205), `HEAD` (`mark_pre_existing`, :228; the current lock) and the index or one commit (`judge_commit`, :1613: `--staged`, `spec propose drift`, MCP `report_drift`). Each side pairs alone. Today it joins entries to rows (:405-410), runs `cascade` (:412), judges (:416-473). With 5c:

1. Rows built as today.
2. **E**: the entry-only rows the judge reads `broken_binding file_gone|symbol_gone` (their code fact, `unbound()`, needs no cascade), of kind `item` or `module`: never `node_gone`, `marker_gone`, RON, `hidden_by_error`, `unreadable`, `item_error`, `rev_unknown`, `hash_null`.
3. **B**: rows with no entry, `CodeRef::Bound` (an item, an inline `mod`, a file module), in a group holding an E, lockable on the code side (`spec-lock.md` "Lockability" rows levels and binding: no `bad_levels`, `LevelState::Hashed`; no `key_problem`), chosen by a pure function (RD-4). A lagging marker is a candidate (the merged row reads `spec_ahead marker_behind`).
4. B's units join the cascade's in one `made()` (:941; its one `.parse(` :976), rendered by CLI `lock.rs` `level_code` (:1389) as the cascade does (:900). E's digests and fingerprints are its entry's (core `lock.rs` `LevelCode.fingerprints` :208).
5. Core pairs E with B; each MOVED pair merges ("The merged row").
6. Judged as today.

**Group**: (node, relation, kind, item_kind), an opaque key to core (Q13). T = `TRIVIAL_BODY_BYTES` (124, `code-hash.md` "Triviality"; kept by TE-1's sweep, `code-hash-eval.md` "Measured"), passed by the CLI as a parameter (Q18). Per group, over the E and B with `body_bytes ≥ T` (a trivial body on either side never takes part):

1. C0(e) = every b with an equal `body`; none: every b with b.file = e.file and an equal `skeleton` (05 s5.2: "the same skeleton appeared in the same file").
2. **C(e)** = C0(e); when |C0(e)| > 1, those with an equal `quote` instead, only when exactly one is left.
3. **R(b)**: R0(b) = {e : b ∈ C(e)}; when one of them matched b by `body`, only the `body` matches stay (a body match outranks a skeleton match); then step 2's quote rule.
4. **MOVED** iff C(e) = {b} and R(b) = {e}. Else **AMBIGUOUS** if C(e) ≠ ∅ (candidates C(e)), else **LOST**.

The R(b) rule (2026-10-20): without step 3's precedence a quote could pick a skeleton match over a body-equal entry.

- **Fingerprints** (`code-hash.md` "Fingerprints"): `body` excludes the name, so a rename or a move to any file keeps it; `skeleton` masks identifiers and literals, same file only; `quote` only breaks ties. A skeleton follow differs in `body`: at worst a Fix becomes a Check.
- **Twins**: equal bodies pair by quote, both ways; equal quotes stay AMBIGUOUS. A new unmarked twin is never a candidate; an ordinal shift (`marker_gone`) never pairs.
- **Deterministic**: over sets, output in key order; no line, map order or time goes in. **Cost** (WA-1): only the side's own marker rows; no tree-wide search, no read of 6a's index.

**Core API** (Q13, Q18; names the developer's, the tests follow them):

```rust
// specengine-core/src/anchor.rs: pure, no I/O, no node or relation type
pub struct Print<'a, G> { pub group: G, pub file: &'a str, pub body: &'a str,
    pub skeleton: &'a str, pub quote: &'a str, pub body_bytes: u32 }
pub enum Follow { Moved(usize), Ambiguous(Vec<usize>), Lost } // into `bindings`, ascending
pub fn pair<G: Ord>(entries: &[Print<'_, G>], bindings: &[Print<'_, G>], trivial: u32) -> Vec<Follow>
```

One `Follow` per entry, a trivial one `Lost`; 7a and 7b call it as is.

### Anchors per row

| `anchor.state` | Row | `from` | `candidates` |
|---|---|---|---|
| `exact` | an entry and a marker binding at one key, any state but `cannot_verify` | null | null |
| `moved` | the merged row | e's `file`, `kind`, `symbol`, `item_kind` | null |
| `ambiguous` | e's row, `broken_binding file_gone\|symbol_gone`, Fix (Q3) | null | C(e), the same four fields each, key order |
| `lost` | every other entry row with no binding at its key: `broken_binding` of any category (`node_gone`, `marker_gone`, RON, trivial, C(e) = ∅) | null | null |

`anchor` null (Q20): a row without an entry (`unlocked`, keyless, a marker-only Fix, an unpaired or contested B) and every `cannot_verify` row (AC-14).

### The merged row

At b's key with b's markers; entry e; `compared` `ast`, no blob step (b's digests come from step 4), b's digests against e's over C ("Cascade" step 4); `path` changed iff `path` ∈ L_lock ∪ L_mark, first in canonical order: `code_changed path,body`, and in `levels.changed` too (Q19). Core's judge needs no change: `level_change` (core `verify.rs:1232`) renders any level of `Comparison::Digests.changed`; the CLI adds `path`, which `RowLevels::compared` (:791) leaves out. e's row is gone (`counts.rows` one less per pair). Unchanged levels read `ok` (Q1).

**The `code` fact** (S7): digest levels only, else null. `path` added to `changed` before `cascade` builds `code` (`verify_rows.rs:904-913`) gives `Some("")`: it must be `None`.

### Drift evidence

Q6, Q8. `drift_hash` keeps `specengine-drift/v1` and its 18 places (`proposal-drift-item.md` "The item"); `lock_*` are e's; `code` as above (a `path,body` row's `body:<digest>`). Place 18 `moved_from` = `anchor.from` as `[file, kind, symbol, item_kind]`, else `null` (today a literal `Value::Null`, core `drift.rs:397`), through a new `HashedRow.moved_from` (:322-347; literals in CLI `drift.rs:632`, tests `verify_levels.rs:474`, `drift_raise.rs:643`). Stored rows are verify's (CLI `drift.rs:555-567`): `anchor` is stored as evidence (S9). Core `DriftRow` never reads it (Q22), so the foreign `anchor`s hand-written in `mcp_drift.rs:321`, `drift_approve.rs:1438`, `drift_stored.rs:55`, `queue_drift.rs:908,1106` stay green. An open item over a now-followed rename re-keys once (`evidence changed`).

### Output

WA-6: no new state; 4c-1's triggers (`drift::raises`) and 9b's label map stand. CLI `verify.rs` (S10):

- **JSON**: `anchor`, the 17th key, after `levels` (`VerifyRow` :119-153), `{"state":…,"from":{…}|null,"candidates":[{…}]|null}` or null. `delta` (rows not Pre-existing): `bindings.moved` (`BindingsJson` :1077, now `Option<()>`) `[{"from":<e's six key fields>,"to":<the row's>}]`, `[]` when none, a committed unaccepted move on every run (Q21); `ambiguous` (:1144-1150) adds the `ambiguous` anchors; `lost` (:1151) the `broken_binding` rows not `ambiguous`. The three are disjoint; a moved `code_ahead` or `conflict` row is in `stale` too. `counts` unchanged.
- **Text** (Q5, Q16): under a non-`ok` moved row `  moved from <file> | <kind> <symbol>[ <item_kind>]`; under an ambiguous row one `  candidate <file> | <kind> <symbol>[ <item_kind>]` per candidate; the summary's (:1020) `<n> ok` gains ` (<m> moved)` (`ok` rows moved), omitted at 0. When any row is moved, `ok` too, once in `hints` (:877) after the per-state hints, before `DRIFT_HINT`: `hint: moved: commit, then spec lock accept <ID> on a terminal locks the new key and drops the old`.

```
check | code_ahead code_changed body | R-12 | implements | crates/game/src/stamina.rs | item game::stamina::regen_tick function_item
  moved from crates/game/src/stamina.rs | item game::stamina::regen_system function_item
fix | broken_binding symbol_gone | R-20 | implements | crates/game/src/movement.rs | item game::movement::step function_item
  candidate crates/game/src/movement.rs | item game::movement::step_a function_item
  candidate crates/game/src/movement.rs | item game::movement::step_b function_item
hint: moved: commit, then spec lock accept <ID> on a terminal locks the new key and drops the old
verify: blocked [enforce]: 1 fix, 1 check, 0 cannot, 11 ok (1 moved); 9 by blob, 3 by ast; 0 skipped
```

(The per-state and drift hints elided.) The first row's JSON ends:

```json
 "compared":"ast","levels":{"lock":["sig","body"],"markers":["sig","body"],"changed":["body"]},
 "anchor":{"state":"moved","from":{"file":"crates/game/src/stamina.rs","kind":"item",
   "symbol":"game::stamina::regen_system","item_kind":"function_item"},"candidates":null}}
```

### Fixtures

Q11, Q12. `fixtures/anchor-states/` with `expected.json`: each TE-1 scenario runs in a temp git repo copied from it, locked by `spec lock accept <ID>` (format 2), committed, then changed; no digest hand-written. Bodies of T-1 and T `body_bytes` are written by the test and asserted through `item_levels` against `specengine_code::TRIVIAL_BODY_BYTES`; no file, test name or expectation holds the number. Core cannot read the constant (no `specengine-code` in its graph): `anchor_pairing.rs` passes its own `t`.

**Churn** (S14, S15): `verify_common/mod.rs:66` `ROW_KEYS` 16 -> 17 (read by `verify_output.rs:414,478`, `drift_raise.rs:79`); `verify_output.rs:397` `moved` null -> `[]`; the `HashedRow` literals. Green unedited only while their bodies stay trivial: `verify_states.rs` `renamed_symbol_is_symbol_gone`, `drift_raise.rs` `renamed_symbol_raises_symbol_gone`, `symbol_twins.rs` `twin_added_after_lock_breaks_the_entry`, `ecs_system_cfg_twins_lock_and_verify_clean`.

## Rules and edge cases

- WHEN an entry's key is given by no marker, the system SHALL follow it only to a marker binding of its group by a mutual unique match, and SHALL never follow a trivial body (either side), RON, `marker_gone`, `node_gone` or a cannot-verify fact (05 s5.2: "a missed follow heals as STALE, a false follow lies forever").
- WHEN verify judges a side, it SHALL pair on that side alone inside `judge_side`, so a committed pure rename reads the same under `--changed`, `--staged` and a sweep (Q7); `is_pre_existing` (core `verify.rs:1307`: key, state, category) is unchanged (S20).
- WHEN a pair is MOVED, verify SHALL give one row at the binding's key judged against the entry, and SHALL write nothing: no lock byte, index row, queue row or file (WA-3, ADR-0046).
- WHEN only `path` changed on a moved row, its drift `code` SHALL be null, the move carried by place 18 alone (Q8).
- WHEN a row cannot be verified, its anchor SHALL be null and it SHALL never be `ok` (AC-14).
- WHEN a side has no E, verify SHALL parse no file beyond the cascade's.
- WHEN run from two roots, another cwd, `core.autocrlf=true` or over reversed entries, rows, anchors and output SHALL be byte-identical.
- **The pin** (S12) `nothing_outside_the_binder_reads_an_items_state` (`code_refs_walk.rs`) stays green unedited: B hashed in `verify_rows.rs` by `item_levels(`, `module_levels(` (pinned :1400); the anchor never read as `<x>.state` there (allowed: `judged`, `levels`, `item`, :1448-1452); a new hashing file would join the list.
- **Genre-neutral** (ADR-0008, S19): no stack, kind or prefix in the pairing, rows or hints; core gains no tree-sitter; CLI sources cite canon by name (``canon `spec-verify-anchors` ``), never `docs/` (`genre.rs`, AC-17).

## Acceptance criteria

rust-developer (M1-M8, cap 8: the mutation that must turn the named test red; tests by the test-engineer):

- [ ] RD-1 -- the pairing, pure; new core `tests/anchor_pairing.rs` over "Core API". **M1** uniqueness checked one way -> `contested_candidate_moves_nothing`. **M2** `<=` for `<` -> `body_at_the_threshold_moves` (t), `body_under_the_threshold_is_never_followed` (t-1). **M3** the skeleton matched across files -> `skeleton_step_stays_in_the_entry_file`. **M4** a candidate taken while several remain after the quote -> `twins_with_equal_quotes_stay_ambiguous`. Unmutated: `body_match_wins_over_skeleton` (both sides: an e with a body and a skeleton match; a b with a body-equal and a skeleton-equal e, the R(b) rule), `twins_with_distinct_quotes_each_move`, `trivial_candidate_is_never_followed`, `candidates_share_the_group_key`, `pairing_ignores_input_order` (every permutation of a 3 + 3 input, indexes mapped back).
- [ ] RD-2 -- the merged row; new CLI `tests/verify_anchors.rs`. **M5** e's row kept beside the merged one -> `moved_binding_is_one_row`. **M6** `path` not added -> `path_marker_reads_a_move_as_code_changed`. Unmutated: `exact_path_binding_stays_ok`, `renamed_and_edited_is_code_ahead`, `path_change_adds_no_code_fact` (null, never `""`), the never-followed cells (RON, `marker_gone`, `node_gone`, `hidden_by_error`).
- [ ] RD-3 -- evidence and output. **M7** place 18 left null -> `moved_row_hash_carries_moved_from` (`drift_raise.rs`, the hash rebuilt by hand, `regen_drift_hash`'s way). Unmutated: `delta_lists_moved_pairs`, `delta_lists_are_disjoint`; text and JSON goldens in `verify_anchors.rs` (the sub-lines, ` (m moved)`, the hint's place, 17 keys).
- [ ] RD-4 -- cost. **M8** the selection takes every unlocked binding -> `lone_entries_select_no_binding` (`#[cfg(test)]` in `verify_rows.rs`, over the pure B selection). Guards: one `.parse(` in `verify_rows.rs` outside `#[cfg(test)]`, a static scan as `code_refs_walk.rs` `one_parse_per_rederived_file` (no runtime counter exists: S13); `verify_output.rs` `verify_never_writes_the_lock`, `verify_writes_no_queue_row` green.
- [ ] RD-5 -- guards, no mutation: the pin green unedited; eval `build_graph.rs` `core_and_store_have_no_tree_sitter`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; core `genre.rs`; CLI `genre.rs`; every other `verify_*` suite green but the churn above.

test-engineer:

- [ ] TE-1 -- `fixtures/anchor-states/` ("Fixtures"), run by `verify_anchors.rs`; per scenario every row's state, class and anchor pinned (untouched bindings `ok` `exact`):

| Scenario | Expected rows |
|---|---|
| rename within the file | one `ok`, `moved` |
| move to another file, the old file kept | one `ok`, `moved` |
| rename plus a local renamed (skeleton step) | one `code_ahead code_changed body`, `moved` |
| move across files plus an edit | `broken_binding symbol_gone` `lost`; `unlocked` null |
| bodies of T-1 and T `body_bytes`, renamed | T-1: `broken_binding symbol_gone` `lost`, `unlocked` null; T: `ok` `moved` |
| twins, distinct quotes, both renamed | two `ok`, `moved` |
| twins, equal quotes, both renamed | two `broken_binding` `ambiguous`, 2 candidates each; two `unlocked` null |
| a contested candidate (two entries, one binding, no quote equal) | two `broken_binding` `ambiguous`, 1 candidate each; one `unlocked` null |
| a marker moved, the old item stays | `broken_binding marker_gone` `lost`; `unlocked` null |
| a locked impl gaining an unmarked in-file twin (`impl Tally` -> `impl Tally #1`) | one `ok`, `moved`, by `body` |
| an ordinal shift: an unmarked twin inserted above two locked twins of one node | never paired: the old `#1` `broken_binding marker_gone` `lost`; the old `#2` compared with the twin now there, `exact`; `#3` `unlocked` null |
| a RON path renamed | `broken_binding marker_gone` `lost`; `unlocked` null |
| a `//!` file renamed | `ok`, `moved`, kind `module` |
| a `[path]` binding moved | `code_ahead code_changed path`, `moved`, `levels.changed` `["path"]`, drift `code` null |
| a rename by the manifest only (a lib added) | every bound item of `main.rs` (bodies ≥ T) `ok`, `moved` |
| a lagging marker, renamed | `spec_ahead marker_behind`, `moved` |
| an ID renamed to an alias | `broken_binding node_gone` `lost` (Q20); `unlocked` null |
| a symbol gone from a file with an orphan parse error, its marker on a renamed copy | `cannot_verify hidden_by_error` null; `unlocked` null |

- [ ] TE-2 -- AC-15 end to end under `enforce-introduced`, every side: a pure rename uncommitted, `spec verify --changed` exits 0 (today 1); staged, a `--staged` hook exits 0; committed, `spec propose drift` stores nothing (MCP `report_drift` runs the same command); a committed rename plus an edit: the moved `code_ahead` row is Pre-existing under `--changed` and `--staged`. In `verify_anchors.rs`, the staged cases may sit in `verify_staged.rs`.
- [ ] TE-3 -- `verify_output.rs` `verify_is_byte_identical_across_roots_and_order` gains a moved and an ambiguous scenario: two roots, the lock's entries reversed, `core.autocrlf=true`; JSON and text byte-identical.
- [ ] TE-4 -- read-only after every TE-1 scenario: `spec.lock` bytes, `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs unchanged.

spec-writer:

- [x] SW-1 -- this spec (2026-10-12), refreshed 2026-10-16 and 2026-10-20 (against 55709ac); `code-hash.md` "Triviality"'s 5c pointer (Q18); 08 s2.
- [ ] SW-2 -- at shipping; bytes measured 2026-10-20 (caps: Tier 2 12 288, Tier 1 10 240):

| File | Bytes -> at shipping | Edit |
|---|---|---|
| new `docs/canon/spec-verify-anchors.md` | ≤ 3 584 | the pairing with the R(b) rule, anchors, the merged row, the evidence, the `[path]` exception to Q42; its line in `docs/index-canon-spec.md` (3 033) |
| `spec-verify.md` | 11 958 -> ≤ 12 100 | "Rows" points there; "States" C; "Cascade" step 3 (Q42) and its last sentence (fingerprints, `body_bytes`) |
| `spec-verify-output.md` | 9 603 -> ≤ 10 900 | "Text" sub-lines, ` (<m> moved)`; "Hints" `moved`; "JSON" `anchor` (17 keys), `delta.bindings` |
| `proposal-drift-item.md` | 4 458 -> ≤ 4 600 | "The item" place 18 |
| `proposal-drift.md` | 8 944 -> ≤ 9 050 | "Known limits": a followed rename re-keys once |
| `code-hash.md` | 12 223, net ≤ 0 | "Fingerprints" ("verify compares none") |
| `architecture-code.md` | 11 956 -> ≤ 12 080 | "As built (5c)" in "Drift: `spec verify`" |
| `spec-lock.md` | 10 121, ≤ +40 | "Entry" ("never compared"), or untouched; ≥ 2 048 B free for 8b |
| CLI README | 9 957 -> ≤ 10 040 | a pointer |
| core README | 10 139, net ≤ 0 | the `anchor` module |
| `symbol-names.md` | 8 667, net ≤ 0 | "Frozen names" (:54, :56-57: exact keys, "Never all `ok`", "by `body`") |
| `code-identity.md` | 10 471, net ≤ 0 | "Known limits" (:80): a lib's renames followed |
| `CLAUDE.md` | 6 022 | State |
| 05 | 29 054, net ≤ 0 (W) | s5.2 the `MOVED` row, the fingerprint paragraph, "Reply to every write"'s `moved`; s5.3 the `broken_binding` row |
| 08 | | s2; AC-15 (Q17): "copies the fingerprints cannot tell apart (one body or skeleton, one quote) yield `AMBIGUOUS`" |

- [x] SW-3 -- no ADR ("Why"); `export index`, `check` clean.

## Risks

- **The `Some("")` code fact** (S7) -> `path_change_adds_no_code_fact`.
- **The R(b) precedence**, an order-dependent follow -> the rule, `body_match_wins_over_skeleton`, `pairing_ignores_input_order`.
- **Four sides through one edited `judge_side`** -> the no-E path; the existing `verify_*` suites.
- **Rename tests under T** (S15, the analyst's hand count): a longer body flips them -> the tester asserts their `body_bytes < TRIVIAL_BODY_BYTES` through `item_levels`.
- **Delayed alarm**: an `ok` move keeps the old key; a later edit across files cannot match, `broken_binding` naming a long-gone symbol; an open pre-5c item for a committed rename "reads ok at HEAD now" (`proposal-drift-item.md` "Dedup") -> the `moved` hint, `delta.bindings.moved`.
- **Reads vs verify**: unless 7a calls this core pairing (Q13), they give two answers.
- **Weak quote**: `before` is often the marker comment twins share; a miss fails safe (AMBIGUOUS or LOST). Pilots hold no marker (`code-hash-eval.md` "Known limits"): proven on fixtures only.

## Out of scope

Following a lost marker (`marker_gone`) or code without a marker (12b); ID renames (`node_gone`: accept drops the gone node); RON; storing anchors but as evidence; the pairing in accept or rebase; `symbols.renamed` (7b's, Q14); `moved from` in `spec review`'s text (`proposals.rs:894` `drift_row_line`) and follow-up goals (Q22); evidence following a move (8b); `--deep`; UI.

## Open

Decided 2026-10-20 under the owner's pre-approval, review open:

| Point | Answer | The other answer's cost |
|---|---|---|
| Q17 AC-15's "two copies with the same skeleton yield AMBIGUOUS" vs the quote step (05 s5.2: quote "tells copies apart") | keep the quote step; AC-15 reworded at shipping (SW-2) | twins renamed together stay Fix until accepted |
| Q18 the threshold in core | a parameter, the CLI passing `specengine_code::TRIVIAL_BODY_BYTES` | a second constant and a pin test |
| Q19 `levels.changed` on a moved row | includes `path`, as the detail does | the two disagree; the canon explains it |
| Q20 the anchor of `node_gone`, cannot-verify rows | `lost` for every entry row with no binding, not moved or ambiguous; null without an entry or on `cannot_verify` | two rules (`marker_gone` lost, `node_gone` null) |
| Q21 a committed, unaccepted move | in `delta.bindings.moved` on every run (`ok` is never Pre-existing) | a Pre-existing rule for `ok` rows |
| Q22 `moved from` in the review text, follow-up goals | out of scope; the JSON and the UI card show `anchor` | `DriftRow` reads `anchor`; 4 tests and goal texts change |
| Q23 size | one slice, ≤ 3 iterations; the split a contingency | two reviews; AC-15 met only by the second |
| AC-15's T | 124 (TE-1's sweep) | a new T, re-measured on both pilots |

Q1-Q16 (2026-10-12, 2026-10-16) are answered where cited, their costs in this file at 55709ac; S-n: the 2026-10-20 analysis' stale points, fixed in place; WA-n: the 2026-10-12 working answers. Cited from outside: **Q13** core API, an opaque group key and digest records, no node or relation type, so 7a and 7b call it as is; **Q14** 5c fills `bindings.moved` only, `symbols.renamed` stays 7b's on Q13's function. Q15 now needs no pin edit (S12).

## Dependencies and ordering

- **Built on** 5b-1a (`spec-lock.md` "Entry"), 5b-1b (`spec-verify.md` "Cascade" 1-4, `levels`, `HashedRow.code`, the pin), 5b-2 (`spec-lock-rebase.md` "The base": `HEAD` with the current lock).
- **TE-1** (`code-hash-eval.md` "Measured", 2026-10-20, pilots A / B): trivial 48.9 % / 41.1 %; in-file skeleton groups 611 / 288; `quote_separates` 757 / 357; skeleton kept under rename 100 %; followable 4 666 / 3 717: both steps stay.
- **Contingency** (Q23), only if RD-1 is red after iteration 2: 5c-1, the body step, uniqueness, triviality, the merged row, `path`, every output, place 18, every TE-1 row (AC-15 met literally: |C| > 1 is AMBIGUOUS); 5c-2, core only: the skeleton and quote steps, M3, M4; TE-1's skeleton and twin rows flip to the table's.
- **Independent in code**: 4c, 8b, 9b. **Assumed**: no real format-1 lock, no pilot marker.

## Implementation

Not started. Files per role:

- rust-developer: core `src/anchor.rs` (new, `pub mod anchor` in `lib.rs`), `src/drift.rs` (`HashedRow.moved_from`, place 18); CLI `src/verify_rows.rs` (`judge_side`, `made`, `cascade`'s `code`, `Row`'s anchor, the pure B selection), `src/verify.rs` (`VerifyRow`, `shown` :670, `hints` :877, `render_text` :996, `BindingsJson` :1076-1154), `src/drift.rs` (`hashed_row` :619).
- test-engineer: new core `tests/anchor_pairing.rs`, CLI `tests/verify_anchors.rs`, `fixtures/anchor-states/`; edited CLI `tests/verify_common/mod.rs`, `verify_output.rs`, `drift_raise.rs`, `verify_levels.rs`, `verify_staged.rs`, the `#[cfg(test)]` of `src/verify_rows.rs`.
- spec-writer: SW-2.
