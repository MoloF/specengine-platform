---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5c (WA-1..WA-8; Q1-Q6 decided under the owner's pre-approval, review open); 05 s5.2 (anchor states, triviality), :160, :183; 08 s2, AC-15
adrs: []
---

# anchor-states: a renamed or moved binding judged against its old lock entry

## Why

Today (4a, `docs/canon/spec-verify.md` "States") a function renamed or moved together with its marker gives two rows: the old key `broken_binding symbol_gone|file_gone` (Fix) and the new key `unlocked` (Check). So under `enforce-introduced` a pure rename exits 1 ("Verdict and exit"); 4c-1 raises a `high` drift item (`docs/canon/proposal-drift.md` "Raise"); Health row 6 lists both; the agent gets a Fix ("broken by the change", ADR-0042) for a refactor that broke nothing; the owner cannot tell a rename from a deletion plus new code. A manifest change renames qpaths with no code edit (`code-identity.md` "Known limits": adding a lib renames `main.rs` items), and every bound item of that file breaks.

Delivered: a read-only pairing inside verify's judge. A lock entry whose key no marker gives any more is paired with a marker binding of the same node and relation that has no entry; equal digests, unique both ways, and the two become one row at the new key, judged against the old entry by 5b's levels: a rename only `ok`, a rename with edits `code_ahead code_changed <levels>` or `conflict`, a `[path]` binding `code_changed path`. The anchor answers "where is it", `sync` "does it still match" (05:149). No writer, no new `state`, nothing stored, no ADR: it relies on ADR-0020 (the marker is the identity, `architecture.md` :74), ADR-0018, ADR-0042 (nothing unverified is `ok`), ADR-0046 (never stored, `architecture-code.md#sync-in-reads`); ADR-0041, ADR-0050 stand (5c writes nothing); ADR-0012 is helped (fewer Fix rows). Following code with no marker would amend ADR-0020: 12b's ground (08:41). After 4a, 5a-1, 5a-2, 6a-1, 5b (08 s2); before 6b, 7a.

## Description and interactions

- **An agent** running `spec verify --changed` after a rename sees nothing (an `ok` row, `11 ok (1 moved)`), or one Check row with a `moved from` line when it also edited. **The owner** sees the move, re-keys at leisure with `spec lock accept <ID>`, and sees an `ambiguous` Fix row's candidates instead of a bare `symbol_gone`.
- **Re-keying stays with accept** (WA-2): its merge already writes the new key `new` and the vanished one `dropped` (`spec-lock-accept.md` "Steps" 6); a rebase keeps keys byte for byte (`lock-rebase.md` "`spec lock rebase`" step 6). 5c has no write door.
- **Crates** (WA-7: core sees keys, digests, `body_bytes` only, `hash-levels.md` "Description"). Core: the pure pairing ("The pairing"), its own copy of 124 pinned to the code crate's `TRIVIAL_BODY_BYTES` (as 5b pins its level names, `lock-levels.md` "Description"). CLI: both verify sides (disk, `HEAD`) and the output. Code: reuses 5a-1's `item_levels`, `module_levels` (`hash-levels.md` "Entry points", Q6), no new API. Fixtures. **Unchanged**: store, MCP, HTTP, UI, plugin, the gate, `spec check`, the index, the queue, `spec.lock` format 2, `specengine.toml`. No new crate or dependency.
- **4a is not edited while built** (as `lock-levels.md` "Description"). Its lines overridden at 5c's shipping: "Command and rows" (a moved pair is one row at the binding's key); "States" (that row's code comparison against the other key's entry; `path` changed iff moved); "Cascade and spec side" (a merged row skips the blob step, `compared` `ast`; SW refinement); "Output" (`anchor`, sub-lines, the summary's `moved`, the `moved` hint; `delta.bindings.moved`, `ambiguous`, `lost`); "Out of scope"'s 5c line. 5b's "Level-aware verify" "`path` is never compared" too. For 4a's and 5b's implementation, no text change: `moved` stays null; optionally `"anchor": null` reserved per JSON row.
- **Neighbours**, each refreshed 2026-10-12 pointing here: 4c-1, shipped (`docs/canon/proposal-drift-item.md` "The item": `drift_hash` ends with `moved_from`; later row keys are no corruption, Q6); 6a `symbol-index.md` (`symbols` store `skeleton_hash`, `quote_hash`, `body_bytes`, so reads pair as verify does); 7a `bundle-bindings.md` (the binding row gains `anchor`, `moved_from`); 6b `code-tools.md` (the mirrors' `anchor` enum; `--symbol` matching a moved row's `from`); `hash-levels.md`, `lock-levels.md` "Out of scope"; 08:41. **9b, 8b: no edit.** 9b passes rows verbatim and shows an unknown word raw (`health-drift.md` "`drift`", "Route and UI"); whichever of 9b and 5c ships second adds `anchor` to `daemon-keys.json`. 8b: a move stales evidence (its key lines are in `bound_hash`, `tested-evidence.md` "Bound hash"): no follow of evidence.

## Data

No file, table, key or schema changes but verify's output and 4c-1's reserved slot.

### The pairing

WA-1, WA-4, WA-5, Q2. On each side (disk; with `--changed` also `HEAD`), before the cascade, per **group** (node, relation, kind, item_kind):

- **E**: the group's lock entries whose node resolves (not `node_gone`), whose key no marker gives and whose code fact is `file_gone` or `symbol_gone` (4a "States"), of kind `item` or `module`, with stored `body_bytes ≥ 124`. Its `body_hash`, `skeleton_hash`, `quote_hash` are format 2's (`lock-levels.md` "Format 2"; WA-8).
- **B**: the group's marker rows with no entry at their key, lockable now on the code side (5b's "Accept" rows `levels` and binding: levels valid, unambiguous, hashed; no `key_problem`; WA-5), with `body_bytes ≥ 124`; digests and fingerprints from 5a-1's `item_levels`, `module_levels` over that side's bytes, as accept's. The markers' revisions do not matter: a lagging marker is a candidate (SW refinement; the merged row then reads `spec_ahead marker_behind`).

1. C(e) = every b with an equal `body`; none: every b with b.file = e.file and an equal `skeleton` (05:156's "same file").
2. |C(e)| > 1: keep those with an equal `quote`, only when exactly one is left.
3. R(b) = {e : b ∈ C(e)}, with the same quote rule.
4. **MOVED** iff C(e) = {b} and R(b) = {e}. Else **AMBIGUOUS** if C(e) ≠ ∅ (candidates C(e)), else **LOST**.

- **EXACT**: an entry and a marker binding share the key.
- **Deterministic**: over sets, output in 4a's key order ("Command and rows"); keys root-relative; no line, path string, time or map order goes in.
- **Cost** (WA-1): candidates are verify's own marker rows: no tree-wide search, no dependency on 6a (05:156-158's suffix index and discriminator serve tree-wide lookups). A file is parsed only if it holds a B of a group with an unpaired E, once, shared with the cascade's re-parse; 4a's Q4 holds.

### Anchors per row

| `anchor.state` | Row | `from` | `candidates` |
|---|---|---|---|
| `exact` | an entry and a marker binding at its key, any state (the null cases below excepted) | null | null |
| `moved` | the merged row ("The merged row") | e's `file`, `kind`, `symbol`, `item_kind` | null |
| `ambiguous` | e's row, 4a's `broken_binding file_gone\|symbol_gone`, Fix (Q3) | null | C(e), the same four fields each, key order |
| `lost` | an entry's `broken_binding` neither moved nor ambiguous: C(e) = ∅, or never followed (RON, `body_bytes < 124`, `marker_gone`; WA-1, WA-4) | null | null |

`anchor` null: a row without an entry (`unlocked`, keyless, a marker-only Fix, an unpaired or contested B), `node_gone` (Q4), and the cannot-verify code facts `hidden_by_error`, `unreadable`, `item_error`: nothing was looked for.

### The merged row

Key, markers from b; entry e; `compared` `ast` (b was hashed for the pairing; no blob step). 4a's matrix and 5b's "Level-aware verify" steps 1, 3, 4 judge it with b's digests against e's, plus: `path` is changed iff the anchor is `moved` and `path` ∈ L_lock ∪ L_mark (`hash-levels.md` "Levels"), first in canonical order (`code_changed path,body`). e's own row is gone; `counts.rows` drops by one per pair. Q1: unchanged levels read `ok`.

### Output

WA-6: no new state; the anchor sits beside 5b's judgement, so 4c-1's triggers and 9b's label map stand.

- **JSON**: every row gains `anchor` after 5b's `levels`, `{"state":…,"from":{…}|null,"candidates":[{…}]|null}` or null. `delta` (over rows not Pre-existing, as 4a): `bindings.moved` `[{"from":<e's six key fields>,"to":<the row's>}]`; `ambiguous` = rows with an `ambiguous` anchor ∪ 4a's `ambiguous_*` rows; `lost` = `broken_binding` rows not `ambiguous`. `moved`, `ambiguous`, `lost` are disjoint; a moved `code_ahead` or `conflict` row is in `stale` too (SW refinement). `counts` unchanged.
- **Text** (Q5): a non-`ok` moved row gets the line `  moved from <file> | <kind> <symbol>[ <item_kind>]` under it; an ambiguous row one `  candidate <file> | <kind> <symbol>[ <item_kind>]` per candidate; `ok` rows stay unprinted, the summary's `<n> ok` gains ` (<m> moved)`, `ok` rows whose anchor is `moved`, omitted at 0. When any row is moved, once after 4a's per-state hints, before the drift hint: `hint: moved: commit, then spec lock accept <ID> on a terminal moves its entry to the new key` (SW refinement).

```
check | code_ahead code_changed body | R-12 | implements | crates/game/src/stamina.rs | item game::stamina::regen_tick function_item
  moved from crates/game/src/stamina.rs | item game::stamina::regen_system function_item
fix | broken_binding symbol_gone | R-20 | implements | crates/game/src/movement.rs | item game::movement::step function_item
  candidate crates/game/src/movement.rs | item game::movement::step_a function_item
  candidate crates/game/src/movement.rs | item game::movement::step_b function_item
hint: moved: commit, then spec lock accept <ID> on a terminal moves its entry to the new key
verify: blocked [enforce]: 1 fix, 1 check, 0 cannot, 11 ok (1 moved); 9 by blob, 3 by ast
```

(4a's per-state and drift hints elided.) The first row in JSON:

```json
{"node":"R-12","relation":"implements","file":"crates/game/src/stamina.rs","kind":"item",
 "symbol":"game::stamina::regen_tick","item_kind":"function_item","state":"code_ahead",
 "category":"code_changed","detail":"body","class":"check","pre_existing":false,
 "markers":[{"line":41,"rev":2}],"node_rev":2,"lock":{"rev":2,"commit":"a0cea48<33 hex>","verified_at":"2026-10-10"},
 "compared":"ast","levels":{"lock":["sig","body"],"markers":["sig","body"],"changed":["body"]},
 "anchor":{"state":"moved","from":{"file":"crates/game/src/stamina.rs","kind":"item",
   "symbol":"game::stamina::regen_system","item_kind":"function_item"},"candidates":null}}
```

- **4c-1's slot** (Q6, reserved now): `drift_hash`'s per-row array ends with `moved_from` = the row's `anchor.from` as `[file, kind, symbol, item_kind]`, `null` otherwise (always, until 5c ships); the hash tag stays `specengine-drift/v1`.

## Rules and edge cases

- WHEN an entry's key is given by no marker, the system SHALL follow it only to a marker binding of its group by a mutual unique match, and SHALL never follow a trivial body (either side), RON, `marker_gone`, `node_gone` or a cannot-verify fact (05:156: "a missed follow heals as STALE, a false follow lies forever").
- WHEN a pair is MOVED, verify SHALL give one row at the binding's key judged against the entry, and SHALL write nothing: no lock byte, index row, queue row or file (WA-3, ADR-0046).
- WHEN a row cannot be verified, its anchor SHALL be null and it SHALL never be `ok` (AC-14, unchanged).
- WHEN run from two roots, another cwd, `core.autocrlf=true` or over reversed entries, rows, anchors and output SHALL be byte-identical.
- Genre-neutral: no stack, kind or prefix in the pairing, rows or hints; no Bevy rule (one-line `build` fns fall under 124).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- the pairing, pure (core, "The pairing"). M: skeleton matched across files -> `skeleton_step_stays_in_the_entry_file`; uniqueness checked one way -> `contested_candidate_moves_nothing`; quote ignored -> `twins_with_distinct_quotes_each_move`; two left after the quote taken -> `twins_with_equal_quotes_stay_ambiguous`; skeleton tried after a body match -> `body_match_wins_over_skeleton`; `<=` for `<` -> `body_of_123_bytes_is_never_followed`, `body_of_124_bytes_moves`; a trivial candidate -> `trivial_candidate_is_never_followed`; another relation or kind -> `candidates_share_node_relation_and_kind`; order-dependent -> `pairing_ignores_input_order` (a property test over permutations).
- [ ] RD-2 -- never followed. M: RON -> `ron_entry_is_lost`; `marker_gone` -> `lost_marker_is_not_followed`; `node_gone` -> `gone_node_is_not_followed`; `hidden_by_error` -> `hidden_symbol_stays_cannot_verify`.
- [ ] RD-3 -- verify ("The merged row"). M: two rows kept -> `moved_binding_is_one_row`; `path` ignored -> `path_marker_reads_a_move_as_code_changed`; `path` changed on an exact row -> `exact_path_binding_stays_ok`; an edit judged without the entry -> `renamed_and_edited_is_code_ahead`; `HEAD` side not paired -> `committed_move_is_pre_existing`.
- [ ] RD-4 -- output. M: `moved` left null -> `delta_lists_moved_pairs`; a row in two of `moved`, `ambiguous`, `lost` -> `delta_lists_are_disjoint`; `anchor` or a sub-line missing -> verify's JSON and text goldens.
- [ ] RD-5 -- cost, read-only. M: every unlocked binding hashed -> `no_lone_entry_parses_no_extra_file` (a parse counter, as `health-drift.md` RD-13); an anchor persisted -> `verify_writes_nothing`.
- [ ] RD-6 -- pins. M: core's 124 drifts -> `trivial_threshold_is_the_code_crates`; a new dependency or tree-sitter in core -> `crates/specengine-eval/tests/build_graph.rs`.

test-engineer:

- [ ] TE-1 -- `fixtures/anchor-states/` with `expected.json`, one locked-then-changed scenario each, every row's state, class and anchor pinned:

| Scenario | Expected rows |
|---|---|
| rename within the file | one `ok`, `moved` |
| move to another file | one `ok`, `moved` |
| rename plus a local renamed (skeleton step) | one `code_ahead code_changed body`, `moved` |
| move across files plus an edit | `broken_binding` `lost`; `unlocked` null |
| bodies of 123 B and 124 B, renamed | 123: `broken_binding` `lost` + `unlocked`; 124: `ok` `moved` |
| twins, distinct quotes, both renamed | two `ok`, `moved` |
| twins, equal quotes, both renamed | two `broken_binding` `ambiguous`, 2 candidates each; two `unlocked` |
| a contested candidate (two entries, one binding) | two `broken_binding` `ambiguous`, 1 candidate; one `unlocked` |
| a marker moved, the old item stays | `broken_binding marker_gone` `lost`; `unlocked` |
| a locked impl gaining an in-file twin (6a-1: `impl Tally` -> `impl Tally #1`) | one `ok`, `moved`, paired by `body` |
| an ordinal shift: an unmarked twin inserted above two locked twins of one node | never paired: the old `#1` `broken_binding marker_gone` `lost`, the old `#2` compared with the twin now there; `#3` `unlocked` |
| a RON path renamed | `broken_binding symbol_gone` `lost`; `unlocked` |
| a `//!` file renamed | `ok`, `moved`, kind `module` |
| a `[path]` binding moved | `code_ahead code_changed path`, `moved` |
| a rename by the manifest only (a lib added) | every bound item of `main.rs` `ok`, `moved` |
| a lagging marker, renamed | `spec_ahead marker_behind`, `moved` |
| an ID renamed to an alias | `broken_binding node_gone`, null; `unlocked`, null |

- [ ] TE-2 -- AC-15 end to end: a pure rename, uncommitted, `spec verify --changed` under `enforce-introduced` exits 0 (4a exits 1). M: the pairing turned off.
- [ ] TE-3 -- byte-identical JSON and text across two roots, the lock's entries reversed, LF vs `core.autocrlf=true` checkouts.
- [ ] TE-4 -- read-only: after every TE-1 scenario `spec.lock` bytes, `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs unchanged.

spec-writer:

- [x] SW-1 -- this spec, no ADR; `export index`, `check` clean.
- [x] SW-2 -- drafts refreshed 2026-10-12 ("Neighbours"); 4a overridden here, not edited; 9b, 8b unedited.
- [ ] SW-3 -- at shipping: 4a's overridden lines in verify's canon (`architecture-code.md#verify` and what 4a creates), 5b's `path` line, 05:145 ("marker lost" -> "marker found at another key"), :160, :183, 08 AC-15 met, the procedure as canon.

## Risks

- **Delayed alarm**: an `ok` move leaves the old key in the lock; an edit after a move across files then cannot match (the skeleton step stays in the entry's file), and `broken_binding` names a long-gone symbol. Mitigations: the `moved` hint, Q1.
- **Reads vs verify**: 6a-2 stores the fingerprint columns before 5b (`symbol-index.md` Q11); without them reads and verify give two answers (`health-drift.md` Q7).
- **Weak quote tie-break**: `quote`'s `before` is often the marker comment, so twins bound by one node share it.
- 4c-1's corruption rule must accept new row keys (refreshed); 8b's evidence goes stale on a move.
- Pilots hold no marker (`hash-levels-eval.md` "Risks"): proven on fixtures only.

## Out of scope

Following a lost marker (`marker_gone`, `hash-levels.md` Q4) or code without a marker (12b); ID renames (`node_gone`: accept drops the gone node); RON (never followed, `hash-levels-ron.md` "RON value"); storing anchors; the pairing in accept or rebase rows; evidence following a move (8b); `--deep`; per-branch ratios; UI.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 a moved binding, levels unchanged | `ok` (AC-15 "rename is not drift"; `[path]` the opt-in, 05:137) | a Check, Health row and drift item per rename, `path` meaningless; the lock re-keyed sooner |
| Q2 where candidates come from | marker bindings of the same node and relation | anything in the tree (05:145 literally): whole-tree hashing each run, the twin trap, an ADR-0020 amendment |
| Q3 an AMBIGUOUS row's class | 4a's `broken_binding` (Fix), with candidates | as Check, an `enforce` project passes copies verify cannot vouch for |
| Q4 follow `node_gone` (an ID now an alias) | no | a second identity path inside 5c |
| Q5 moved `ok` rows in text | the summary's count only | every rename printed |
| Q6 the `drift_hash` slot | reserved in 4c now | `specengine-drift/v2` later, an "evidence changed" note per open item |

Q1-Q6 (2026-10-12) decided under the owner's pre-approval, review open. WA-n: the analysis' working answers (2026-10-12), glossed where cited; WA-5..WA-7 assumed, the rest proven from the sources; "SW refinement": the spec-writer's. Order: 4a, 5a-1, 5a-2, 6a-1, 6a-2, 5b -> 5c -> 6b, 7a (2026-10-13, `symbol-index.md` Q11); 4c, 8b, 9b do not depend on 5c in code.

## Implementation

Not started. To record: modules, the parse counter's figures, deviations with reasons.
